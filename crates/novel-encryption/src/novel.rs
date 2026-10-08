//! Turning a novel into word lists: tokenizing, fingerprinting, and the
//! chain graph keys are drawn from.

use std::collections::HashMap;

use sha2::{Digest, Sha256};
use unicode_normalization::char::is_combining_mark;
use unicode_normalization::UnicodeNormalization;

use crate::narrative::NarrativeModel;
use crate::{Error, Result};

/// Shortest word eligible for keys and novel armor.
pub const KEY_WORD_MIN_LEN: usize = 3;
/// Longest word eligible for keys and novel armor.
pub const KEY_WORD_MAX_LEN: usize = 15;
/// A novel must leave at least this many key words after pruning.
pub const MIN_KEY_VOCAB: usize = 256;
/// Number of words in the novel-armor byte table (one per byte value).
pub const ARMOR_TABLE_SIZE: usize = 256;

const FINGERPRINT_DOMAIN: &[u8] = b"NovelEncryption/v1/novel\n";

/// Split text into lowercase ASCII words.
///
/// The text is NFKD-normalized and combining marks are dropped, so "Café" and
/// "naïve" become `cafe` and `naive`. Every other character that is not an
/// ASCII letter (digits, punctuation, apostrophes, other scripts) ends the
/// current word. Numerals therefore never appear in the output.
pub fn tokenize(text: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut cur = String::new();
    for c in text.nfkd() {
        if is_combining_mark(c) {
            continue;
        }
        if c.is_ascii_alphabetic() {
            cur.push(c.to_ascii_lowercase());
        } else if !cur.is_empty() {
            words.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        words.push(cur);
    }
    words
}

fn initial(w: &str) -> usize {
    (w.as_bytes()[0] - b'a') as usize
}

fn final_letter(w: &str) -> usize {
    (w.as_bytes()[w.len() - 1] - b'a') as usize
}

/// A novel prepared for key generation, encryption and armor.
#[derive(Clone)]
pub struct Novel {
    fingerprint: [u8; 32],
    total_words: usize,
    distinct_words: usize,
    /// Sorted, pruned key vocabulary.
    vocab: Vec<String>,
    index: HashMap<String, u32>,
    /// For each letter a..z, indices into `vocab` of words starting with it.
    by_initial: [Vec<u32>; 26],
    armor_table: Vec<String>,
    armor_index: HashMap<String, u8>,
    narrative: NarrativeModel,
}

impl std::fmt::Debug for Novel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Novel")
            .field("fingerprint", &self.fingerprint_hex())
            .field("total_words", &self.total_words)
            .field("key_vocabulary", &self.vocab.len())
            .finish()
    }
}

impl Novel {
    /// Prepare a novel from its full text.
    pub fn from_text(text: &str) -> Result<Novel> {
        let tokens = tokenize(text);
        let mut counts: HashMap<&str, u64> = HashMap::new();
        for t in &tokens {
            *counts.entry(t.as_str()).or_default() += 1;
        }

        // Fingerprint: SHA-256 over the sorted set of distinct words, so line
        // wrapping, punctuation and word order do not matter -- only vocabulary.
        let mut distinct: Vec<&str> = counts.keys().copied().collect();
        distinct.sort_unstable();
        let mut h = Sha256::new();
        h.update(FINGERPRINT_DOMAIN);
        for w in &distinct {
            h.update(w.as_bytes());
            h.update(b"\n");
        }
        let fingerprint: [u8; 32] = h.finalize().into();

        // Key vocabulary: length-bounded words, then repeatedly prune words
        // whose last letter no remaining word starts with (chain dead ends).
        let mut vocab: Vec<&str> = distinct
            .iter()
            .copied()
            .filter(|w| (KEY_WORD_MIN_LEN..=KEY_WORD_MAX_LEN).contains(&w.len()))
            .collect();
        loop {
            let mut starts = [false; 26];
            for w in &vocab {
                starts[initial(w)] = true;
            }
            let before = vocab.len();
            vocab.retain(|w| starts[final_letter(w)]);
            if vocab.len() == before {
                break;
            }
        }
        if vocab.len() < MIN_KEY_VOCAB {
            return Err(Error::NovelTooSmall { found: vocab.len(), required: MIN_KEY_VOCAB });
        }

        let mut by_initial: [Vec<u32>; 26] = Default::default();
        let mut index = HashMap::with_capacity(vocab.len());
        for (i, w) in vocab.iter().enumerate() {
            by_initial[initial(w)].push(i as u32);
            index.insert(w.to_string(), i as u32);
        }

        // Armor table: the 256 most frequent key words (ties alphabetical).
        let mut ranked = vocab.clone();
        ranked.sort_by(|a, b| counts[b].cmp(&counts[a]).then(a.cmp(b)));
        let armor_table: Vec<String> =
            ranked.iter().take(ARMOR_TABLE_SIZE).map(|w| w.to_string()).collect();
        let armor_index =
            armor_table.iter().enumerate().map(|(i, w)| (w.clone(), i as u8)).collect();

        Ok(Novel {
            fingerprint,
            total_words: tokens.len(),
            distinct_words: distinct.len(),
            vocab: vocab.into_iter().map(String::from).collect(),
            index,
            by_initial,
            armor_table,
            armor_index,
            narrative: NarrativeModel::from_text(text),
        })
    }

    /// SHA-256 vocabulary fingerprint that binds keys and ciphertexts to this novel.
    pub fn fingerprint(&self) -> &[u8; 32] {
        &self.fingerprint
    }

    pub fn fingerprint_hex(&self) -> String {
        self.fingerprint.iter().map(|b| format!("{b:02x}")).collect()
    }

    /// The order-2 word model behind narrative keys.
    pub fn narrative(&self) -> &NarrativeModel {
        &self.narrative
    }

    /// Sorted key vocabulary.
    pub fn vocabulary(&self) -> &[String] {
        &self.vocab
    }

    pub fn contains(&self, word: &str) -> bool {
        self.index.contains_key(word)
    }

    /// Words a chain may continue with after a word ending in `letter`.
    pub fn successors(&self, letter: u8) -> impl Iterator<Item = &str> {
        let i = letter.to_ascii_lowercase().wrapping_sub(b'a') as usize;
        self.by_initial
            .get(i)
            .into_iter()
            .flatten()
            .map(move |&ix| self.vocab[ix as usize].as_str())
    }

    pub(crate) fn word(&self, ix: u32) -> &str {
        &self.vocab[ix as usize]
    }

    /// Number of words a chain may continue with after `word`.
    pub(crate) fn branching_after(&self, word: &str) -> usize {
        self.by_initial[final_letter(word)].len()
    }

    pub(crate) fn candidates_after(&self, word: &str) -> &[u32] {
        &self.by_initial[final_letter(word)]
    }

    pub(crate) fn armor_table(&self) -> &[String] {
        &self.armor_table
    }

    pub(crate) fn armor_byte(&self, word: &str) -> Option<u8> {
        self.armor_index.get(word).copied()
    }

    /// Summary statistics, including the long-run entropy of each chained word.
    pub fn stats(&self) -> NovelStats {
        let n = self.vocab.len() as f64;
        let size = |l: usize| self.by_initial[l].len() as f64;

        // Markov chain over "current last letter". From letter a the next word
        // is uniform among words starting with a, contributing log2|S_a| bits,
        // and moves to that word's last letter.
        let mut trans = [[0f64; 26]; 26];
        for (a, row) in trans.iter_mut().enumerate() {
            for &ix in &self.by_initial[a] {
                row[final_letter(&self.vocab[ix as usize])] += 1.0 / size(a);
            }
        }
        // Distribution of the first word's last letter (first word uniform over vocab).
        let mut dist = [0f64; 26];
        for w in &self.vocab {
            dist[final_letter(w)] += 1.0 / n;
        }
        let step_bits = |d: &[f64; 26]| -> f64 {
            (0..26).filter(|&a| d[a] > 0.0).map(|a| d[a] * size(a).log2()).sum()
        };
        let second_word_bits = step_bits(&dist);
        for _ in 0..500 {
            let mut next = [0f64; 26];
            for a in 0..26 {
                for b in 0..26 {
                    next[b] += dist[a] * trans[a][b];
                }
            }
            dist = next;
        }
        let first_word_bits = n.log2();
        let chain_bits_per_word = step_bits(&dist);

        NovelStats {
            fingerprint: self.fingerprint_hex(),
            total_words: self.total_words,
            distinct_words: self.distinct_words,
            key_vocabulary: self.vocab.len(),
            first_word_bits,
            second_word_bits,
            chain_bits_per_word,
            unconstrained_bits_per_word: first_word_bits,
            words_for_128_bits: 1 + ((128.0 - first_word_bits) / chain_bits_per_word).ceil() as usize,
        }
    }
}

/// Statistics about a novel's key space.
#[derive(Debug, Clone, PartialEq)]
pub struct NovelStats {
    pub fingerprint: String,
    /// Tokens in the text (with repeats).
    pub total_words: usize,
    /// Distinct tokens of any length.
    pub distinct_words: usize,
    /// Distinct words usable in keys after length filtering and pruning.
    pub key_vocabulary: usize,
    /// Entropy of the first key word: log2(key_vocabulary).
    pub first_word_bits: f64,
    /// Expected entropy of the second key word.
    pub second_word_bits: f64,
    /// Long-run expected entropy of each further chained word.
    pub chain_bits_per_word: f64,
    /// Entropy per word if the chain rule were dropped (for comparison).
    pub unconstrained_bits_per_word: f64,
    /// Typical number of words to reach 128 bits.
    pub words_for_128_bits: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenize_folds_accents_and_drops_numerals() {
        assert_eq!(
            tokenize("Café naïve, DON'T stop—1984 times! Ahab’s x2y"),
            ["cafe", "naive", "don", "t", "stop", "times", "ahab", "s", "x", "y"]
        );
        assert!(tokenize("123 456 ...").is_empty());
    }

    #[test]
    fn small_text_is_rejected() {
        let err = Novel::from_text("the quick brown fox jumps over the lazy dog").unwrap_err();
        assert!(matches!(err, Error::NovelTooSmall { .. }));
    }
}
