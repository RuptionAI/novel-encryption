//! Ways to carry a sealed message as text.
//!
//! * **Compact armor**: one unbroken base64url string, the smallest text form,
//!   for chat, SMS and QR codes.
//! * **Text armor**: base64 between BEGIN/END lines, for email.
//! * **Novel armor**: every byte becomes one of the novel's 256 most common
//!   words, so the message reads like words from the book.
//! * **Story armor**: the message is written as a new passage in the book's
//!   voice, a "lost chapter": each word choice carries a few bits.
//!
//! Novel and story armor are *encodings* of ciphertext, not extra secrecy, and
//! decoding them needs the same novel.

use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use base64::Engine;

use crate::novel::{tokenize, Novel};
use crate::seal::is_sealed;
use crate::{Error, Result};

pub const BEGIN: &str = "-----BEGIN NOVEL ENCRYPTION MESSAGE-----";
pub const END: &str = "-----END NOVEL ENCRYPTION MESSAGE-----";

/// Output formats for sealed messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Armor {
    Binary,
    Compact,
    Text,
    Novel,
    Story,
}

const STORY_HEADING_TAIL: &str = "a lost chapter";

/// Write a sealed message as a new passage of the book, under an all-caps
/// heading (headings are ignored when decoding).
pub fn story_encode(novel: &Novel, data: &[u8], title: Option<&str>) -> Result<String> {
    let model = novel.narrative();
    // Move the format byte behind the salt so the story opens with random
    // bits: every chapter starts differently.
    if data.len() < 17 {
        return Err(Error::NotNovelEncryption);
    }
    let mut lead_salt = data[1..].to_vec();
    lead_salt.insert(16, data[0]);
    let words = model.story_encode(&lead_salt)?;
    let prose = model.render(&words);
    let heading = match title {
        Some(t) if !t.trim().is_empty() => format!("{}: A LOST CHAPTER", t.trim().to_uppercase()),
        _ => "A LOST CHAPTER".to_string(),
    };
    let mut out = format!("{heading}\n\n");
    // Paragraph every six sentences, lines wrapped at 72 columns.
    let (mut line, mut sentences) = (0usize, 0usize);
    for w in prose.split(' ') {
        if line > 0 && line + 1 + w.len() > 72 {
            out.push('\n');
            line = 0;
        } else if line > 0 {
            out.push(' ');
            line += 1;
        }
        out.push_str(w);
        line += w.len();
        if w.ends_with(['.', '!', '?']) {
            sentences += 1;
            if sentences % 6 == 0 {
                out.push_str("\n\n");
                line = 0;
            }
        }
    }
    out.truncate(out.trim_end().len());
    out.push('\n');
    Ok(out)
}

pub fn story_decode(novel: &Novel, s: &str) -> Result<Vec<u8>> {
    // Skip the heading, even if a chat app re-cased it or joined its line.
    let lower = s.to_ascii_lowercase();
    let s = match lower.find(STORY_HEADING_TAIL) {
        Some(i) if i < 300 => &s[i + STORY_HEADING_TAIL.len()..],
        _ => s,
    };
    let body: String = s.lines().filter(|l| !crate::narrative::is_heading(l)).collect::<Vec<_>>().join("\n");
    let mut r = novel.narrative().story_decode(&crate::narrative::tokenize_key(&body))?;
    if r.len() < 17 {
        return Err(Error::NotNovelEncryption);
    }
    let format = r.remove(16);
    r.insert(0, format);
    Ok(r)
}

pub fn compact_encode(data: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(data)
}

pub fn compact_decode(s: &str) -> Result<Vec<u8>> {
    URL_SAFE_NO_PAD.decode(s.trim()).map_err(|e| Error::Armor(format!("bad compact string: {e}")))
}

pub fn text_encode(data: &[u8]) -> String {
    let b64 = STANDARD.encode(data);
    let mut out = String::with_capacity(b64.len() + b64.len() / 64 + BEGIN.len() + END.len() + 4);
    out.push_str(BEGIN);
    out.push('\n');
    for chunk in b64.as_bytes().chunks(64) {
        out.push_str(std::str::from_utf8(chunk).unwrap());
        out.push('\n');
    }
    out.push_str(END);
    out.push('\n');
    out
}

pub fn text_decode(s: &str) -> Result<Vec<u8>> {
    let start = s.find(BEGIN).ok_or_else(|| Error::Armor("missing BEGIN line".into()))? + BEGIN.len();
    let end = s[start..].find(END).ok_or_else(|| Error::Armor("missing END line".into()))? + start;
    let body: String = s[start..end].chars().filter(|c| !c.is_whitespace()).collect();
    STANDARD.decode(body).map_err(|e| Error::Armor(format!("bad base64: {e}")))
}

/// Encode bytes as words from the novel, dressed as sentences. Capitals,
/// periods and line breaks are cosmetic and ignored when decoding.
pub fn novel_encode(novel: &Novel, data: &[u8]) -> String {
    let table = novel.armor_table();
    let mut out = String::new();
    let mut line_len = 0;
    let mut sentence_left = 0usize;
    for (i, &b) in data.iter().enumerate() {
        let w = &table[b as usize];
        let starts_sentence = sentence_left == 0;
        if starts_sentence {
            sentence_left = 6 + (b as usize % 9);
        }
        sentence_left -= 1;
        let ends_sentence = sentence_left == 0 || i + 1 == data.len();

        if i > 0 {
            if line_len + 1 + w.len() + 1 > 72 {
                out.push('\n');
                line_len = 0;
            } else {
                out.push(' ');
                line_len += 1;
            }
        }
        if starts_sentence {
            let mut cs = w.chars();
            let first = cs.next().unwrap().to_ascii_uppercase();
            out.push(first);
            out.push_str(cs.as_str());
        } else {
            out.push_str(w);
        }
        line_len += w.len();
        if ends_sentence {
            out.push('.');
            line_len += 1;
        }
    }
    out.push('\n');
    out
}

pub fn novel_decode(novel: &Novel, s: &str) -> Result<Vec<u8>> {
    tokenize(s)
        .iter()
        .map(|w| {
            novel.armor_byte(w).ok_or_else(|| {
                Error::Armor(format!("\"{w}\" is not an armor word of this novel (wrong novel?)"))
            })
        })
        .collect()
}

/// Encode a sealed message in the requested armor. `Binary` returns it unchanged.
pub fn encode(novel: &Novel, data: &[u8], armor: Armor) -> Result<Vec<u8>> {
    Ok(match armor {
        Armor::Binary => data.to_vec(),
        Armor::Compact => compact_encode(data).into_bytes(),
        Armor::Text => text_encode(data).into_bytes(),
        Armor::Novel => novel_encode(novel, data).into_bytes(),
        Armor::Story => story_encode(novel, data, None)?.into_bytes(),
    })
}

/// Detect the armor of `input` and return the binary message.
pub fn decode_any(novel: &Novel, input: &[u8]) -> Result<(Vec<u8>, Armor)> {
    if is_sealed(input) {
        return Ok((input.to_vec(), Armor::Binary));
    }
    let s = std::str::from_utf8(input).map_err(|_| Error::NotNovelEncryption)?;
    if s.contains(BEGIN) {
        return Ok((text_decode(s)?, Armor::Text));
    }
    let t = s.trim();
    if !t.is_empty() && t.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_') {
        if let Ok(bytes) = compact_decode(t) {
            if is_sealed(&bytes) {
                return Ok((bytes, Armor::Compact));
            }
        }
    }
    if let Ok(bytes) = novel_decode(novel, s) {
        if is_sealed(&bytes) {
            return Ok((bytes, Armor::Novel));
        }
    }
    let bytes = story_decode(novel, s)?;
    if !is_sealed(&bytes) {
        return Err(Error::NotNovelEncryption);
    }
    Ok((bytes, Armor::Story))
}
