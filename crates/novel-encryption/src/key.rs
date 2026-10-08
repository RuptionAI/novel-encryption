//! Keys drawn from a novel: generation, parsing, validation and strength.
//!
//! Two styles share one format and one key derivation:
//!
//! * **Narrative** (the default): a passage walked through the book's own
//!   word sequences, so every three consecutive words appear together in the
//!   book. Longer, and reads like the book.
//! * **Chain**: words in which each starts with the last letter of the one
//!   before. Shorter and easier to memorize.

use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::narrative::tokenize_key;
use crate::novel::Novel;
use crate::{rng, Error, Result};

/// Default and recommended minimum key strength, in bits.
pub const DEFAULT_KEY_BITS: f64 = 128.0;

/// Long-term key strength: keeps a 128-bit margin even against a quantum
/// adversary running Grover's search. For secrets that must last decades.
pub const LONG_TERM_KEY_BITS: f64 = 256.0;

/// Upper bound on key length, to catch nonsensical requests.
pub const MAX_KEY_WORDS: usize = 1024;

/// How a key is drawn from the book.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum KeyStyle {
    /// A passage that follows the book's own word sequences.
    #[default]
    Narrative,
    /// Words linked by letters: each starts with the last letter of the one before.
    Chain,
}

/// A key's words, canonical (lowercase letters only). Wiped from memory on drop.
#[derive(Clone, PartialEq, Eq, Zeroize, ZeroizeOnDrop)]
pub struct KeyPhrase {
    words: Vec<String>,
}

impl std::fmt::Debug for KeyPhrase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "KeyPhrase({} words, redacted)", self.words.len())
    }
}

impl KeyPhrase {
    /// Parse a key typed or pasted in any reasonable form. Case, punctuation,
    /// hyphens and line breaks are ignored, and apostrophes inside words are
    /// optional ("it's" and "its" are the same word).
    pub fn parse(s: &str) -> Result<KeyPhrase> {
        let words = tokenize_key(s);
        if words.is_empty() {
            return Err(Error::EmptyKey);
        }
        Ok(KeyPhrase { words })
    }

    pub fn words(&self) -> &[String] {
        &self.words
    }

    pub fn len(&self) -> usize {
        self.words.len()
    }

    pub fn is_empty(&self) -> bool {
        self.words.is_empty()
    }

    /// Canonical words joined by hyphens.
    pub fn to_hyphenated(&self) -> String {
        self.words.join("-")
    }

    /// Canonical bytes fed to the KDF: words joined by single spaces.
    pub(crate) fn canonical_bytes(&self) -> Vec<u8> {
        self.words.join(" ").into_bytes()
    }
}

/// How long a generated key should be.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum KeyLength {
    /// Keep adding words until the key reaches at least this many bits.
    /// Every key produced this way has probability at most 2^-bits.
    Bits(f64),
    /// Exactly this many words (the user's chosen depth).
    Words(usize),
}

impl Default for KeyLength {
    fn default() -> Self {
        KeyLength::Bits(DEFAULT_KEY_BITS)
    }
}

/// A freshly generated key and its strength.
#[derive(Debug, Clone)]
pub struct GeneratedKey {
    pub phrase: KeyPhrase,
    pub style: KeyStyle,
    /// Exact surprisal of this key: -log2 of the probability the generator
    /// had of producing it.
    pub bits: f64,
    /// The key as people should see it: prose with the book's punctuation
    /// and capitals (narrative), or hyphenated words (chain). Parsing the
    /// display form gives back the same key.
    pub display: String,
}

/// Generate a key from OS randomness. Keys below [`DEFAULT_KEY_BITS`] are
/// refused unless `allow_weak` is set.
pub fn generate(novel: &Novel, style: KeyStyle, length: KeyLength, allow_weak: bool) -> Result<GeneratedKey> {
    match length {
        KeyLength::Bits(b) if !(1.0..=4096.0).contains(&b) => {
            return Err(Error::InvalidLength(format!("bits must be 1..=4096, got {b}")))
        }
        KeyLength::Bits(b) if b < DEFAULT_KEY_BITS && !allow_weak => {
            return Err(Error::WeakKey { bits: b, required: DEFAULT_KEY_BITS })
        }
        KeyLength::Words(n) if n == 0 || n > MAX_KEY_WORDS => {
            return Err(Error::InvalidLength(format!("words must be 1..={MAX_KEY_WORDS}, got {n}")))
        }
        KeyLength::Words(n) if style == KeyStyle::Narrative && n < 3 => {
            return Err(Error::InvalidLength("narrative keys need at least 3 words".into()))
        }
        _ => {}
    }

    let (words, bits) = match style {
        KeyStyle::Chain => generate_chain(novel, length)?,
        KeyStyle::Narrative => {
            let (bits, exact) = match length {
                KeyLength::Bits(b) => (Some(b), None),
                KeyLength::Words(n) => (None, Some(n)),
            };
            novel.narrative().generate(bits, exact, MAX_KEY_WORDS)?
        }
    };
    if bits < DEFAULT_KEY_BITS && !allow_weak {
        return Err(Error::WeakKey { bits, required: DEFAULT_KEY_BITS });
    }
    let display = match style {
        KeyStyle::Chain => words.join("-"),
        KeyStyle::Narrative => novel.narrative().render(&words),
    };
    Ok(GeneratedKey { phrase: KeyPhrase { words }, style, bits, display })
}

/// Chain generation: the first word is uniform over the key vocabulary, and
/// each next word is uniform over words starting with the previous word's
/// last letter.
fn generate_chain(novel: &Novel, length: KeyLength) -> Result<(Vec<String>, f64)> {
    let vocab_len = novel.vocabulary().len() as u32;
    let first = rng::uniform(vocab_len)?;
    let mut words = vec![novel.word(first).to_string()];
    let mut bits = (vocab_len as f64).log2();
    let done = |words: &Vec<String>, bits: f64| match length {
        KeyLength::Bits(b) => bits >= b,
        KeyLength::Words(n) => words.len() >= n,
    };
    while !done(&words, bits) {
        if words.len() >= MAX_KEY_WORDS {
            return Err(Error::InvalidLength(format!("more than {MAX_KEY_WORDS} words needed")));
        }
        let candidates = novel.candidates_after(words.last().unwrap());
        let pick = candidates[rng::uniform(candidates.len() as u32)? as usize];
        bits += (candidates.len() as f64).log2();
        words.push(novel.word(pick).to_string());
    }
    Ok((words, bits))
}

/// Result of validating a key against a novel.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KeyCheck {
    pub style: KeyStyle,
    /// Strength in bits *if the key was generated randomly*; a person-chosen
    /// phrase is far weaker than this number.
    pub bits: f64,
}

/// Check that a key fits the novel in either style. Errors name the first
/// word that breaks the key, which makes typos easy to find.
pub fn check(novel: &Novel, key: &KeyPhrase) -> Result<KeyCheck> {
    if key.is_empty() {
        return Err(Error::EmptyKey);
    }
    let narrative = check_narrative(novel, key);
    if let Ok(bits) = narrative {
        return Ok(KeyCheck { style: KeyStyle::Narrative, bits });
    }
    let chain = check_chain(novel, key);
    if let Ok(bits) = chain {
        return Ok(KeyCheck { style: KeyStyle::Chain, bits });
    }
    // Report the error for the style the key mostly follows: the share of
    // letter links that hold versus the share of word triples in the book.
    let words = key.words();
    let chain_share = if words.len() < 2 {
        0.0
    } else {
        let links = words.windows(2).filter(|p| p[0].as_bytes().last() == p[1].as_bytes().first()).count();
        links as f64 / (words.len() - 1) as f64
    };
    let narrative_share = novel.narrative().triple_share(words);
    Err(if chain_share > narrative_share { chain.unwrap_err() } else { narrative.unwrap_err() })
}

fn check_narrative(novel: &Novel, key: &KeyPhrase) -> Result<f64> {
    novel.narrative().check(key.words()).map_err(|(position, word)| {
        if word.is_empty() {
            Error::InvalidLength("narrative keys have at least 3 words".into())
        } else if novel.narrative().knows(&word) {
            Error::KeyNarrativeBroken { position, word }
        } else {
            Error::KeyWordUnknown { position, word }
        }
    })
}

fn check_chain(novel: &Novel, key: &KeyPhrase) -> Result<f64> {
    let words = key.words();
    let mut bits = (novel.vocabulary().len() as f64).log2();
    for (i, w) in words.iter().enumerate() {
        if !novel.contains(w) {
            return Err(Error::KeyWordUnknown { position: i + 1, word: w.clone() });
        }
        if i > 0 {
            let prev = &words[i - 1];
            let expected = *prev.as_bytes().last().unwrap();
            if w.as_bytes()[0] != expected {
                return Err(Error::KeyChainBroken {
                    position: i + 1,
                    previous: prev.clone(),
                    word: w.clone(),
                    expected: expected as char,
                });
            }
            bits += (novel.branching_after(prev) as f64).log2();
        }
    }
    Ok(bits)
}
