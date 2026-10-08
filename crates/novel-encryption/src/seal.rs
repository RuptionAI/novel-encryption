//! Sealing data: Argon2id key derivation, optional compression, and
//! XChaCha20-Poly1305 authenticated encryption.
//!
//! Message layout (integers little-endian):
//!
//! ```text
//! 1 byte   format: 0xE1 = v1 with the standard KDF profile,
//!                  0xE0 = v1 with explicit KDF parameters, followed by
//!                         m_kib (u32), t (u32), p (u8)
//! 16 bytes salt (random, fresh for every message)
//! n+1+16   XChaCha20-Poly1305( flags (1 byte) || payload ) and tag
//! ```
//!
//! The header (format byte, any parameters, salt) is authenticated as
//! associated data. Argon2id yields 56 bytes: the 32-byte key and the 24-byte
//! nonce. Because every message has a fresh salt, every message has its own
//! key, so the nonce never needs to be stored. With the standard profile the
//! total overhead is 34 bytes.

use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use zeroize::{Zeroize, Zeroizing};

use crate::key::{check, KeyPhrase};
use crate::novel::Novel;
use crate::{rng, Error, Result};

/// Format byte: version 1, standard KDF profile.
pub const FORMAT_V1_STANDARD: u8 = 0xE1;
/// Format byte: version 1, explicit KDF parameters follow.
pub const FORMAT_V1_CUSTOM: u8 = 0xE0;
pub const SALT_LEN: usize = 16;
pub const TAG_LEN: usize = 16;
/// Smallest possible message: standard header, flags byte, tag.
pub const MIN_LEN: usize = 1 + SALT_LEN + 1 + TAG_LEN;
/// Overhead of a standard-profile message over its (possibly compressed) payload.
pub const OVERHEAD: usize = MIN_LEN;

const FLAG_DEFLATE: u8 = 0x01;
const KDF_DOMAIN: &[u8] = b"NovelEncryption/v1/key\n";

/// Bounds enforced when decrypting, so a hostile header cannot demand
/// unbounded memory or time.
pub const MAX_M_KIB: u32 = 1 << 20; // 1 GiB
pub const MAX_T: u32 = 64;
pub const MAX_P: u8 = 16;
/// Largest plaintext a compressed message may expand to.
pub const MAX_INFLATED: usize = 1 << 30;

/// Argon2id cost parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KdfParams {
    pub m_kib: u32,
    pub t: u32,
    pub p: u8,
}

impl KdfParams {
    /// The standard profile: 64 MiB, 3 passes, 1 lane. About a second in a
    /// browser, and expensive for an attacker to run at scale.
    pub const STANDARD: KdfParams = KdfParams { m_kib: 64 * 1024, t: 3, p: 1 };

    fn validate(&self) -> Result<()> {
        if self.p == 0 || self.p > MAX_P {
            return Err(Error::InvalidParams(format!("parallelism {} not in 1..={MAX_P}", self.p)));
        }
        if self.t == 0 || self.t > MAX_T {
            return Err(Error::InvalidParams(format!("iterations {} not in 1..={MAX_T}", self.t)));
        }
        if self.m_kib < 8 * self.p as u32 || self.m_kib > MAX_M_KIB {
            return Err(Error::InvalidParams(format!(
                "memory {} KiB not in {}..={MAX_M_KIB}",
                self.m_kib,
                8 * self.p as u32
            )));
        }
        Ok(())
    }
}

impl Default for KdfParams {
    fn default() -> Self {
        KdfParams::STANDARD
    }
}

/// Options for sealing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SealOptions {
    pub kdf: KdfParams,
    /// Deflate the data first, keeping the result only if it is smaller.
    pub compress: bool,
}

impl Default for SealOptions {
    fn default() -> Self {
        SealOptions { kdf: KdfParams::STANDARD, compress: true }
    }
}

/// Derive the 32-byte key and 24-byte nonce.
///
/// password = "NovelEncryption/v1/key\n" || novel fingerprint (32 bytes) || key words joined by " "
/// output   = Argon2id(password, salt, m, t, p, 56 bytes) = key (32) || nonce (24)
fn derive(novel: &Novel, key: &KeyPhrase, params: KdfParams, salt: &[u8; SALT_LEN]) -> Result<Zeroizing<[u8; 56]>> {
    params.validate()?;
    let mut password = Zeroizing::new(Vec::with_capacity(128));
    password.extend_from_slice(KDF_DOMAIN);
    password.extend_from_slice(novel.fingerprint());
    let mut phrase = key.canonical_bytes();
    password.extend_from_slice(&phrase);
    phrase.zeroize();

    let argon_params = Params::new(params.m_kib, params.t, params.p as u32, Some(56))
        .map_err(|e| Error::InvalidParams(e.to_string()))?;
    let mut out = Zeroizing::new([0u8; 56]);
    Argon2::new(Algorithm::Argon2id, Version::V0x13, argon_params)
        .hash_password_into(&password, salt, out.as_mut())
        .map_err(|e| Error::InvalidParams(e.to_string()))?;
    Ok(out)
}

fn header(params: KdfParams, salt: &[u8; SALT_LEN]) -> Vec<u8> {
    let mut h = Vec::with_capacity(1 + 9 + SALT_LEN);
    if params == KdfParams::STANDARD {
        h.push(FORMAT_V1_STANDARD);
    } else {
        h.push(FORMAT_V1_CUSTOM);
        h.extend_from_slice(&params.m_kib.to_le_bytes());
        h.extend_from_slice(&params.t.to_le_bytes());
        h.push(params.p);
    }
    h.extend_from_slice(salt);
    h
}

/// Encrypt `plaintext` under a novel and key with a fresh random salt.
pub fn encrypt(novel: &Novel, key: &KeyPhrase, plaintext: &[u8], opts: SealOptions) -> Result<Vec<u8>> {
    let mut salt = [0u8; SALT_LEN];
    rng::fill(&mut salt)?;
    encrypt_with_salt(novel, key, plaintext, opts, &salt)
}

/// Deterministic encryption for test vectors and cross-language ports.
///
/// **Never reuse a salt** with the same novel and key: it would reuse the
/// key and nonce. Use [`encrypt`] for real data.
pub fn encrypt_with_salt(
    novel: &Novel,
    key: &KeyPhrase,
    plaintext: &[u8],
    opts: SealOptions,
    salt: &[u8; SALT_LEN],
) -> Result<Vec<u8>> {
    check(novel, key)?;
    let mut body = Zeroizing::new(Vec::with_capacity(plaintext.len() + 1));
    let deflated = opts
        .compress
        .then(|| miniz_oxide::deflate::compress_to_vec(plaintext, 9))
        .filter(|d| d.len() < plaintext.len());
    match deflated {
        Some(d) => {
            body.push(FLAG_DEFLATE);
            body.extend_from_slice(&d);
        }
        None => {
            body.push(0);
            body.extend_from_slice(plaintext);
        }
    }

    let k = derive(novel, key, opts.kdf, salt)?;
    let cipher = XChaCha20Poly1305::new(k[..32].into());
    let mut out = header(opts.kdf, salt);
    let ct = cipher
        .encrypt(XNonce::from_slice(&k[32..]), Payload { msg: &body, aad: &out })
        .map_err(|_| Error::DecryptFailed)?;
    out.extend_from_slice(&ct);
    Ok(out)
}

/// Parsed message header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    pub params: KdfParams,
    pub salt: [u8; SALT_LEN],
    /// Header length in bytes (the associated data).
    pub len: usize,
}

/// True if `data` starts like a binary sealed message.
pub fn is_sealed(data: &[u8]) -> bool {
    matches!(data.first(), Some(&FORMAT_V1_STANDARD | &FORMAT_V1_CUSTOM))
}

/// Parse and bounds-check a message header without decrypting.
pub fn parse_header(data: &[u8]) -> Result<Header> {
    let (params, at) = match data.first() {
        Some(&FORMAT_V1_STANDARD) => (KdfParams::STANDARD, 1),
        Some(&FORMAT_V1_CUSTOM) if data.len() >= 10 => {
            let u32_at = |i: usize| u32::from_le_bytes(data[i..i + 4].try_into().unwrap());
            (KdfParams { m_kib: u32_at(1), t: u32_at(5), p: data[9] }, 10)
        }
        Some(&b) if b & 0xF0 == 0xE0 => return Err(Error::UnsupportedVersion(b)),
        _ => return Err(Error::NotNovelEncryption),
    };
    if data.len() < at + SALT_LEN + 1 + TAG_LEN {
        return Err(Error::NotNovelEncryption);
    }
    params.validate()?;
    Ok(Header { params, salt: data[at..at + SALT_LEN].try_into().unwrap(), len: at + SALT_LEN })
}

/// Decrypt a binary message. Fails with [`Error::KeyWordUnknown`] /
/// [`Error::KeyChainBroken`] if the key does not fit the novel, or
/// [`Error::DecryptFailed`] if the novel, key or message is wrong.
pub fn decrypt(novel: &Novel, key: &KeyPhrase, data: &[u8]) -> Result<Vec<u8>> {
    let h = parse_header(data)?;
    check(novel, key)?;
    let k = derive(novel, key, h.params, &h.salt)?;
    let cipher = XChaCha20Poly1305::new(k[..32].into());
    let body = Zeroizing::new(
        cipher
            .decrypt(XNonce::from_slice(&k[32..]), Payload { msg: &data[h.len..], aad: &data[..h.len] })
            .map_err(|_| Error::DecryptFailed)?,
    );
    match body[0] {
        0 => Ok(body[1..].to_vec()),
        FLAG_DEFLATE => miniz_oxide::inflate::decompress_to_vec_with_limit(&body[1..], MAX_INFLATED)
            .map_err(|_| Error::InvalidParams("compressed payload is corrupt or too large".into())),
        f => Err(Error::InvalidParams(format!("unknown payload flags {f:#04x}"))),
    }
}
