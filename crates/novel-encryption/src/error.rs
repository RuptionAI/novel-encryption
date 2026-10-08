use thiserror::Error;

/// Everything that can go wrong in Novel Encryption.
#[derive(Debug, Error, Clone, PartialEq)]
pub enum Error {
    #[error("novel is too small: {found} usable key words after pruning, need at least {required}")]
    NovelTooSmall { found: usize, required: usize },

    #[error("key is empty")]
    EmptyKey,

    #[error("key word {position} (\"{word}\") does not appear in this novel")]
    KeyWordUnknown { position: usize, word: String },

    #[error(
        "key word {position} (\"{word}\") breaks the chain: it must start with '{expected}', \
         the last letter of \"{previous}\""
    )]
    KeyChainBroken { position: usize, previous: String, word: String, expected: char },

    #[error("key word {position} (\"{word}\") never follows the two words before it in this novel")]
    KeyNarrativeBroken { position: usize, word: String },

    #[error("key is too weak: {bits:.1} bits, at least {required:.0} required")]
    WeakKey { bits: f64, required: f64 },

    #[error("invalid key length request: {0}")]
    InvalidLength(String),

    #[error("not a Novel Encryption message (bad magic or truncated)")]
    NotNovelEncryption,

    #[error("unsupported format version {0}")]
    UnsupportedVersion(u8),

    #[error("unsupported key-derivation function id {0}")]
    UnsupportedKdf(u8),

    #[error("key-derivation parameters out of bounds: {0}")]
    InvalidParams(String),

    #[error("decryption failed: wrong novel, wrong key, or the message was altered")]
    DecryptFailed,

    #[error("armor: {0}")]
    Armor(String),

    #[error("system randomness unavailable: {0}")]
    Rng(String),
}

pub type Result<T> = core::result::Result<T, Error>;
