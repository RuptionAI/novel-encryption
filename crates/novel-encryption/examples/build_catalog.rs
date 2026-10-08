//! Write `catalog/catalog.json` from `catalog/sources.json` and the pinned texts.
//!
//!     cargo run --release -p novel-encryption --example build_catalog
//!
//! The site and clients verify a downloaded text against `fingerprint` before
//! using it, so a changed text can never silently produce different keys.

use std::path::PathBuf;

use novel_encryption::Novel;
use serde_json::{json, Value};

fn main() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let sources: Vec<Value> =
        serde_json::from_str(&std::fs::read_to_string(root.join("catalog/sources.json")).unwrap()).unwrap();

    let mut out = Vec::new();
    for src in sources {
        let slug = src["slug"].as_str().unwrap();
        let file = format!("catalog/texts/{slug}.txt");
        let bytes = std::fs::read(root.join(&file)).unwrap();
        let novel = Novel::from_text(std::str::from_utf8(&bytes).unwrap()).unwrap();
        let s = novel.stats();
        let mut entry = src.clone();
        let gid = src["gutenberg_id"].as_u64().unwrap();
        entry["file"] = json!(file);
        entry["bytes"] = json!(bytes.len());
        entry["source_url"] = json!(format!("https://www.gutenberg.org/ebooks/{gid}"));
        entry["fingerprint"] = json!(s.fingerprint);
        entry["total_words"] = json!(s.total_words);
        entry["key_vocabulary"] = json!(s.key_vocabulary);
        entry["chain_bits_per_word"] = json!((s.chain_bits_per_word * 100.0).round() / 100.0);
        entry["words_for_128_bits"] = json!(s.words_for_128_bits);
        // Narrative keys: expected words to reach 128 / 256 bits (before
        // finishing the sentence), from the walk's average entropy per word.
        let nm = novel.narrative();
        let start = (nm.start_count() as f64).log2();
        let rate = nm.bits_per_word();
        entry["narrative_bits_per_word"] = json!((rate * 100.0).round() / 100.0);
        entry["narrative_words_for_128_bits"] = json!(2 + ((128.0 - start) / rate).ceil() as usize);
        entry["narrative_words_for_256_bits"] = json!(2 + ((256.0 - start) / rate).ceil() as usize);
        eprintln!("{slug:24} {} vocab={}", &s.fingerprint[..16], s.key_vocabulary);
        out.push(entry);
    }
    let json = serde_json::to_string_pretty(&json!({ "version": 1, "novels": out })).unwrap();
    std::fs::write(root.join("catalog/catalog.json"), json + "\n").unwrap();
}
