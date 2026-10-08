//! WebAssembly bindings used by novelencryption.com. Everything runs in the
//! visitor's browser; novels, keys and data never leave the page.

use novel_encryption as ne;
use wasm_bindgen::prelude::*;

fn js_err(e: ne::Error) -> JsError {
    JsError::new(&e.to_string())
}

/// A prepared novel. Construct once per text; preparation tokenizes the whole book.
#[wasm_bindgen]
pub struct Novel(ne::Novel);

#[wasm_bindgen]
impl Novel {
    #[wasm_bindgen(constructor)]
    pub fn new(text: &str) -> Result<Novel, JsError> {
        ne::Novel::from_text(text).map(Novel).map_err(js_err)
    }

    #[wasm_bindgen(getter)]
    pub fn fingerprint(&self) -> String {
        self.0.fingerprint_hex()
    }

    /// Key-space statistics as a plain object.
    pub fn stats(&self) -> Result<JsValue, JsError> {
        let s = self.0.stats();
        let o = js_sys_object(&[
            ("fingerprint", JsValue::from_str(&s.fingerprint)),
            ("totalWords", JsValue::from_f64(s.total_words as f64)),
            ("distinctWords", JsValue::from_f64(s.distinct_words as f64)),
            ("keyVocabulary", JsValue::from_f64(s.key_vocabulary as f64)),
            ("firstWordBits", JsValue::from_f64(s.first_word_bits)),
            ("chainBitsPerWord", JsValue::from_f64(s.chain_bits_per_word)),
            ("wordsFor128Bits", JsValue::from_f64(s.words_for_128_bits as f64)),
        ])?;
        Ok(o)
    }

    /// Generate a key. `style`: "narrative" (default) or "chain". Pass
    /// `words` > 0 for an exact length, otherwise `bits` sets the minimum
    /// strength. Returns `{ phrase, words, bits, style }`, where `phrase` is the
    /// display form (prose for narrative keys, hyphenated for chain keys).
    #[wasm_bindgen(js_name = generateKey)]
    pub fn generate_key(&self, style: &str, bits: f64, words: u32, allow_weak: bool) -> Result<JsValue, JsError> {
        let style = parse_style(style)?;
        let length = if words > 0 { ne::KeyLength::Words(words as usize) } else { ne::KeyLength::Bits(bits) };
        let k = ne::generate(&self.0, style, length, allow_weak).map_err(js_err)?;
        let list = k.phrase.words().iter().map(|w| JsValue::from_str(w)).collect::<js_sys::Array>();
        js_sys_object(&[
            ("phrase", JsValue::from_str(&k.display)),
            ("words", list.into()),
            ("bits", JsValue::from_f64(k.bits)),
            ("style", JsValue::from_str(style_name(k.style))),
        ])
    }

    /// Validate a key against this novel; returns `{ style, bits, words }` or
    /// throws an error naming the first bad word.
    #[wasm_bindgen(js_name = checkKey)]
    pub fn check_key(&self, key: &str) -> Result<JsValue, JsError> {
        let k = ne::KeyPhrase::parse(key).map_err(js_err)?;
        let c = ne::check_key(&self.0, &k).map_err(js_err)?;
        js_sys_object(&[
            ("style", JsValue::from_str(style_name(c.style))),
            ("bits", JsValue::from_f64(c.bits)),
            ("words", JsValue::from_f64(k.len() as f64)),
        ])
    }

    /// Words that can follow a word ending in `letter` (for key autocomplete).
    pub fn successors(&self, letter: &str, limit: u32) -> Vec<String> {
        let Some(&b) = letter.as_bytes().first() else { return vec![] };
        self.0.successors(b).take(limit as usize).map(String::from).collect()
    }

    /// Encrypt bytes. `armor`: "binary" | "compact" | "text" | "novel" | "story".
    /// Returns bytes (UTF-8 text for the text armors). `compress` deflates
    /// first when that makes the message smaller. `title` heads a story.
    #[allow(clippy::too_many_arguments)]
    pub fn encrypt(&self, key: &str, data: &[u8], armor: &str, m_kib: u32, t: u32, compress: bool, title: &str) -> Result<Vec<u8>, JsError> {
        let k = ne::KeyPhrase::parse(key).map_err(js_err)?;
        let a = match armor {
            "binary" => ne::Armor::Binary,
            "compact" => ne::Armor::Compact,
            "text" => ne::Armor::Text,
            "novel" => ne::Armor::Novel,
            "story" => ne::Armor::Story,
            other => return Err(JsError::new(&format!("unknown armor {other:?}"))),
        };
        let opts = ne::SealOptions { kdf: ne::KdfParams { m_kib, t, p: 1 }, compress };
        let sealed = ne::encrypt(&self.0, &k, data, opts).map_err(js_err)?;
        if a == ne::Armor::Story {
            let t = (!title.is_empty()).then_some(title);
            return ne::armor::story_encode(&self.0, &sealed, t).map(String::into_bytes).map_err(js_err);
        }
        ne::armor::encode(&self.0, &sealed, a).map_err(js_err)
    }

    /// Decrypt any armor of a sealed message.
    pub fn decrypt(&self, key: &str, input: &[u8]) -> Result<Vec<u8>, JsError> {
        let k = ne::KeyPhrase::parse(key).map_err(js_err)?;
        let (bytes, _) = ne::armor::decode_any(&self.0, input).map_err(js_err)?;
        ne::decrypt(&self.0, &k, &bytes).map_err(js_err)
    }
}

/// Default Argon2id memory (KiB) and iterations, for the UI.
#[wasm_bindgen(js_name = defaultKdf)]
pub fn default_kdf() -> Vec<u32> {
    let p = ne::KdfParams::STANDARD;
    vec![p.m_kib, p.t]
}

/// Default and long-term key strengths in bits, for the UI.
#[wasm_bindgen(js_name = keyStrengths)]
pub fn key_strengths() -> Vec<f64> {
    vec![ne::DEFAULT_KEY_BITS, ne::LONG_TERM_KEY_BITS]
}

fn parse_style(s: &str) -> Result<ne::KeyStyle, JsError> {
    match s {
        "" | "narrative" => Ok(ne::KeyStyle::Narrative),
        "chain" => Ok(ne::KeyStyle::Chain),
        other => Err(JsError::new(&format!("unknown key style {other:?}"))),
    }
}

fn style_name(s: ne::KeyStyle) -> &'static str {
    match s {
        ne::KeyStyle::Narrative => "narrative",
        ne::KeyStyle::Chain => "chain",
    }
}

fn js_sys_object(fields: &[(&str, JsValue)]) -> Result<JsValue, JsError> {
    let o = js_sys::Object::new();
    for (k, v) in fields {
        js_sys::Reflect::set(&o, &JsValue::from_str(k), v).map_err(|_| JsError::new("Reflect.set failed"))?;
    }
    Ok(o.into())
}
