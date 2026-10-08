//! Wallet backups as book passages.
//!
//! A BIP-39 recovery phrase (12–24 words) encodes 128–256 bits of entropy.
//! This module writes that same entropy as a passage from a book, using the
//! narrative model's story encoding, and converts it back. Because both forms
//! carry identical bits, the passage always restores the original phrase, so
//! it works with every standard wallet.
//!
//! The passage also carries a 32-bit check tied to the book's fingerprint.
//! A passage copied out of the book by hand, or a real passage read with the
//! wrong book, fails the check: this tool never turns a person-chosen text
//! into a wallet (the "brain wallet" mistake).

use sha2::{Digest, Sha256};
use zeroize::{Zeroize, Zeroizing};

use crate::narrative::tokenize_key;
use crate::novel::Novel;
use crate::{Error, Result};

/// The official BIP-39 English word list (MIT licensed), SHA-256
/// 2f5eed53a4727b4bf8880d8f3f199efc90e58503646d9ff8eff3a2ed3b24dbda.
const WORDLIST: &str = include_str!("bip39-english.txt");

/// Bits carried by the passage's opening, and by each later word.
pub const SEED_START_BITS: u32 = 8;
pub const SEED_STEP_BITS: u32 = 3;

const FORMAT_V1: u8 = 1;
const CHECK_DOMAIN: &[u8] = b"NovelEncryption/v1/seed\n";
const CHECK_LEN: usize = 4;

fn words() -> Vec<&'static str> {
    WORDLIST.lines().collect()
}

fn seed_err(msg: impl Into<String>) -> Error {
    Error::Seed(msg.into())
}

/// Parse and verify a BIP-39 English phrase; returns its entropy.
/// Words may be given in full or as their unique first four letters.
pub fn mnemonic_to_entropy(mnemonic: &str) -> Result<Zeroizing<Vec<u8>>> {
    let list = words();
    let given: Vec<String> = mnemonic.split_whitespace().map(|w| w.to_lowercase()).collect();
    if ![12, 15, 18, 21, 24].contains(&given.len()) {
        return Err(seed_err(format!("a recovery phrase has 12, 15, 18, 21 or 24 words; this has {}", given.len())));
    }
    let mut bits: Vec<bool> = Vec::with_capacity(given.len() * 11);
    for (i, w) in given.iter().enumerate() {
        let index = list.iter().position(|x| x == w).or_else(|| {
            let hits: Vec<usize> =
                (0..list.len()).filter(|&j| w.len() >= 4 && list[j].starts_with(w.as_str())).collect();
            (hits.len() == 1).then(|| hits[0])
        });
        let Some(index) = index else {
            return Err(seed_err(format!("word {} (\"{w}\") is not in the BIP-39 English list", i + 1)));
        };
        bits.extend((0..11).rev().map(|b| (index >> b) & 1 == 1));
    }
    let cs_len = bits.len() / 33;
    let ent_len = bits.len() - cs_len;
    let mut entropy = Zeroizing::new(vec![0u8; ent_len / 8]);
    for (i, &bit) in bits[..ent_len].iter().enumerate() {
        if bit {
            entropy[i / 8] |= 0x80 >> (i % 8);
        }
    }
    let hash = Sha256::digest(&*entropy);
    let expected = (0..cs_len).all(|i| bits[ent_len + i] == ((hash[i / 8] >> (7 - i % 8)) & 1 == 1));
    bits.zeroize();
    if !expected {
        return Err(seed_err("the recovery phrase's checksum is wrong (a word is mistyped or out of order)"));
    }
    Ok(entropy)
}

/// Write entropy (16, 20, 24, 28 or 32 bytes) as a BIP-39 English phrase.
pub fn entropy_to_mnemonic(entropy: &[u8]) -> Result<Zeroizing<String>> {
    if ![16, 20, 24, 28, 32].contains(&entropy.len()) {
        return Err(seed_err(format!("entropy must be 16–32 bytes in steps of 4, not {}", entropy.len())));
    }
    let list = words();
    let hash = Sha256::digest(entropy);
    let cs_len = entropy.len() * 8 / 32;
    let bit = |i: usize| -> usize {
        let b = if i < entropy.len() * 8 { entropy[i / 8] } else { hash[(i - entropy.len() * 8) / 8] };
        let j = if i < entropy.len() * 8 { i } else { i - entropy.len() * 8 };
        ((b >> (7 - j % 8)) & 1) as usize
    };
    let total = entropy.len() * 8 + cs_len;
    let mut out = Zeroizing::new(String::new());
    for w in 0..total / 11 {
        let index = (0..11).fold(0usize, |acc, k| (acc << 1) | bit(w * 11 + k));
        if w > 0 {
            out.push(' ');
        }
        out.push_str(list[index]);
    }
    Ok(out)
}

fn check(novel: &Novel, entropy: &[u8]) -> [u8; CHECK_LEN] {
    let mut h = Sha256::new();
    h.update(CHECK_DOMAIN);
    h.update(novel.fingerprint());
    h.update(entropy);
    h.finalize()[..CHECK_LEN].try_into().unwrap()
}

/// How a recovery phrase is written into the book.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SeedStyle {
    /// A passage that reads like the book (about 125–270 words for a 24-word phrase).
    #[default]
    Narrative,
    /// A letter chain (about 35–50 words for a 24-word phrase): practical to copy by hand.
    Chain,
}

/// A recovery phrase written as a book passage.
#[derive(Debug, Clone)]
pub struct SeedPassage {
    pub style: SeedStyle,
    /// Canonical lowercase words (what is decoded).
    pub words: Vec<String>,
    /// The passage with the book's punctuation and capitals, for writing down.
    pub display: String,
    /// Number of words in the original recovery phrase.
    pub phrase_words: usize,
}

impl Drop for SeedPassage {
    fn drop(&mut self) {
        self.words.zeroize();
        self.display.zeroize();
    }
}

/// Write a recovery phrase into `novel` in the given style.
///
/// Payload = entropy ‖ format (1) ‖ SHA-256("NovelEncryption/v1/seed\n" ‖
/// fingerprint ‖ entropy)[..4].
///
/// * Narrative: story-encoded with [`SEED_START_BITS`] and [`SEED_STEP_BITS`];
///   the entropy comes first, so passages open differently.
/// * Chain: `len(entropy)` (1 byte) ‖ payload, read as bits most significant
///   first. The first word is entry `v` of the sorted key vocabulary and each
///   next word is entry `v` of the words starting with the previous word's
///   last letter, where `v` is the next ⌊log2 n⌋ bits (n = number of
///   choices). Bits past the end read as 0; the chain stops once every bit
///   has been written.
pub fn to_passage(novel: &Novel, mnemonic: &str, style: SeedStyle) -> Result<SeedPassage> {
    let entropy = mnemonic_to_entropy(mnemonic)?;
    let mut payload = Zeroizing::new(entropy.to_vec());
    payload.push(FORMAT_V1);
    payload.extend_from_slice(&check(novel, &entropy));
    let (words, display) = match style {
        SeedStyle::Narrative => {
            let words = novel.narrative().story_encode_with(&payload, SEED_START_BITS, SEED_STEP_BITS)?;
            let display = novel.narrative().render(&words);
            (words, display)
        }
        SeedStyle::Chain => {
            let mut framed = Zeroizing::new(vec![entropy.len() as u8]);
            framed.extend_from_slice(&payload);
            let words = chain_encode(novel, &framed);
            let display = words.chunks(6).map(|c| c.join("-")).collect::<Vec<_>>().join("\n");
            (words, display)
        }
    };
    Ok(SeedPassage { style, words, display, phrase_words: mnemonic.split_whitespace().count() })
}

fn chain_choices<'a>(novel: &'a Novel, prev: Option<&str>) -> Vec<&'a str> {
    match prev {
        None => novel.vocabulary().iter().map(String::as_str).collect(),
        Some(w) => novel.successors(*w.as_bytes().last().unwrap()).collect(),
    }
}

fn chain_encode(novel: &Novel, data: &[u8]) -> Vec<String> {
    let total = data.len() * 8;
    let bit = |i: usize| -> usize { data.get(i / 8).map_or(0, |b| ((b >> (7 - i % 8)) & 1) as usize) };
    let (mut pos, mut out) = (0usize, Vec::new());
    while pos < total {
        let choices = chain_choices(novel, out.last().map(String::as_str));
        let m = choices.len().ilog2() as usize;
        let v = (0..m).fold(0usize, |acc, k| (acc << 1) | bit(pos + k));
        pos += m;
        out.push(choices[v].to_string());
    }
    out
}

fn chain_decode(novel: &Novel, words: &[String]) -> Result<Vec<u8>> {
    let mut bits: Vec<bool> = Vec::new();
    for (i, w) in words.iter().enumerate() {
        let choices = chain_choices(novel, i.checked_sub(1).map(|j| words[j].as_str()));
        let m = choices.len().ilog2();
        let v = choices.iter().position(|c| c == w).filter(|&v| v < 1 << m).ok_or_else(|| {
            seed_err(format!("word {} (\"{w}\") does not continue this chain in this book", i + 1))
        })?;
        bits.extend((0..m).rev().map(|k| (v >> k) & 1 == 1));
    }
    let bytes: Vec<u8> =
        bits.chunks(8).filter(|c| c.len() == 8).map(|c| c.iter().fold(0u8, |a, &b| (a << 1) | b as u8)).collect();
    bits.zeroize();
    let Some(&len) = bytes.first() else { return Err(seed_err("this chain is too short to hold a recovery phrase")) };
    let need = 1 + len as usize + 1 + CHECK_LEN;
    if bytes.len() < need {
        return Err(seed_err("this chain is too short to hold a recovery phrase"));
    }
    Ok(bytes[1..need].to_vec())
}

/// Recover the recovery phrase from a passage or chain written by
/// [`to_passage`]; the style is detected.
pub fn from_passage(novel: &Novel, passage: &str) -> Result<Zeroizing<String>> {
    let mut words = tokenize_key(passage);
    let links = words.windows(2).filter(|p| p[0].as_bytes().last() == p[1].as_bytes().first()).count();
    let is_chain = !words.is_empty() && links * 10 >= (words.len() - 1) * 9;
    let decoded = if is_chain {
        chain_decode(novel, &words)
    } else {
        novel
            .narrative()
            .story_decode_with(&words, SEED_START_BITS, SEED_STEP_BITS)
            .map_err(|e| match e {
                Error::Armor(m) => seed_err(m.replace("continue the story", "continue this passage")),
                other => other,
            })
    };
    words.zeroize();
    let payload = Zeroizing::new(decoded?);
    let n = payload.len();
    if n < CHECK_LEN + 1 + 16 {
        return Err(seed_err("this passage is too short to hold a recovery phrase"));
    }
    let (entropy, tail) = payload.split_at(n - CHECK_LEN - 1);
    if tail[0] != FORMAT_V1 || tail[1..] != check(novel, entropy) {
        return Err(seed_err(
            "this passage was not written by the wallet converter with this book \
             (wrong book, a changed word, or text copied from the book itself)",
        ));
    }
    entropy_to_mnemonic(entropy)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Official BIP-39 vectors (trezor/python-mnemonic, English).
    const VECTORS: &[(&str, &str)] = &[
        ("00000000000000000000000000000000", "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"),
        ("7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f", "legal winner thank year wave sausage worth useful legal winner thank yellow"),
        ("ffffffffffffffffffffffffffffffff", "zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo wrong"),
        ("0000000000000000000000000000000000000000000000000000000000000000", "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon art"),
        ("ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff", "zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo vote"),
        ("9e885d952ad362caeb4efe34a8e91bd2", "ozone drill grab fiber curtain grace pudding thank cruise elder eight picnic"),
    ];

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
    }

    #[test]
    fn bip39_vectors() {
        assert_eq!(words().len(), 2048);
        for (ent, phrase) in VECTORS {
            assert_eq!(&*entropy_to_mnemonic(&hex(ent)).unwrap(), phrase);
            assert_eq!(&*mnemonic_to_entropy(phrase).unwrap(), &hex(ent));
        }
    }

    #[test]
    fn bad_phrases_are_rejected() {
        let typo = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon";
        assert!(matches!(mnemonic_to_entropy(typo), Err(Error::Seed(m)) if m.contains("checksum")));
        assert!(mnemonic_to_entropy("zoo zoo zoo").is_err());
        assert!(mnemonic_to_entropy("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abuot").is_err());
        // Unique four-letter prefixes are accepted.
        assert_eq!(&*mnemonic_to_entropy("lega winn than year wave saus wort usef lega winn than yell").unwrap(), &hex("7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f"));
    }
}
