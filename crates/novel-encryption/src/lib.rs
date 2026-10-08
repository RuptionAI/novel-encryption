//! # Novel Encryption
//!
//! Pick a novel. Draw a key from it: a chain of randomly chosen words in
//! which each word starts with the last letter of the word before
//! (`whale → empty → yonder → rope → …`). Seal data with the novel and the
//! key; unseal it with the same novel and the same key.
//!
//! The novel supplies the vocabulary and is bound into every key and
//! message. The chain is the secret: it is generated from OS randomness,
//! its strength is measured exactly in bits, and its linking rule acts as a
//! checksum that catches typos. The data itself is protected by standard,
//! well-studied primitives -- Argon2id key derivation and XChaCha20-Poly1305
//! authenticated encryption -- never by the word substitution itself.
//!
//! ```no_run
//! use novel_encryption::{armor, generate, Armor, KeyLength, KeyStyle, Novel, SealOptions};
//!
//! let text = std::fs::read_to_string("moby-dick.txt").unwrap();
//! let novel = Novel::from_text(&text).unwrap();
//! let key = generate(&novel, KeyStyle::Narrative, KeyLength::default(), false).unwrap();
//! println!("key ({:.0} bits): {}", key.bits, key.display);
//!
//! let sealed = novel_encryption::encrypt(&novel, &key.phrase, b"Call me Ishmael.", SealOptions::default()).unwrap();
//! let prose = armor::encode(&novel, &sealed, Armor::Story).unwrap();
//!
//! let (bytes, _) = armor::decode_any(&novel, &prose).unwrap();
//! let opened = novel_encryption::decrypt(&novel, &key.phrase, &bytes).unwrap();
//! assert_eq!(opened, b"Call me Ishmael.");
//! ```
//!
//! See `docs/SPEC.md` for the byte-exact specification and `docs/WHITEPAPER.md`
//! for the design rationale and security analysis.

#![forbid(unsafe_code)]

pub mod armor;
mod error;
pub mod key;
pub mod narrative;
pub mod novel;
pub mod rng;
pub mod seal;
pub mod seed;

pub use armor::Armor;
pub use error::{Error, Result};
pub use key::{
    check as check_key, generate, GeneratedKey, KeyCheck, KeyLength, KeyPhrase, KeyStyle, DEFAULT_KEY_BITS,
    LONG_TERM_KEY_BITS,
};
pub use novel::{tokenize, Novel, NovelStats};
pub use seal::{decrypt, encrypt, encrypt_with_salt, is_sealed, parse_header, KdfParams, SealOptions};
