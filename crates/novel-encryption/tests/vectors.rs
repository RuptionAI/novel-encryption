//! The implementation must keep reproducing the pinned conformance vectors.

use std::path::PathBuf;

use novel_encryption::{armor, decrypt, encrypt_with_salt, tokenize, KdfParams, KeyPhrase, Novel, SealOptions};
use serde_json::Value;

#[test]
fn v1_vectors() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let v: Value = serde_json::from_str(&std::fs::read_to_string(root.join("vectors/v1.json")).unwrap()).unwrap();

    for t in v["tokenize"].as_array().unwrap() {
        let out: Vec<String> = serde_json::from_value(t["output"].clone()).unwrap();
        assert_eq!(tokenize(t["input"].as_str().unwrap()), out);
    }

    let text = std::fs::read_to_string(root.join(v["novel"]["file"].as_str().unwrap())).unwrap();
    let novel = Novel::from_text(&text).unwrap();
    assert_eq!(novel.fingerprint_hex(), v["novel"]["fingerprint"]);
    assert_eq!(novel.stats().key_vocabulary as u64, v["novel"]["key_vocabulary"].as_u64().unwrap());

    for t in v["key_tokenize"].as_array().unwrap() {
        let out: Vec<String> = serde_json::from_value(t["output"].clone()).unwrap();
        assert_eq!(novel_encryption::narrative::tokenize_key(t["input"].as_str().unwrap()), out);
    }
    let n = &v["narrative"];
    let nm = novel.narrative();
    assert_eq!(nm.start_count() as u64, n["start_contexts"].as_u64().unwrap());
    assert_eq!(nm.trigram_count() as u64, n["live_trigrams"].as_u64().unwrap());
    let walk = nm.first_walk(30);
    assert_eq!(walk.join(" "), n["first_walk_30"].as_str().unwrap());
    let wk = KeyPhrase::parse(&walk.join(" ")).unwrap();
    let c = novel_encryption::check_key(&novel, &wk).unwrap();
    assert_eq!(c.style, novel_encryption::KeyStyle::Narrative);
    assert_eq!(format!("{:.6}", c.bits), n["first_walk_30_bits"].as_str().unwrap());
    assert_eq!(nm.render(&walk), n["first_walk_30_display"].as_str().unwrap());

    let s = &v["seal"];
    let key = KeyPhrase::parse(v["key"]["phrase"].as_str().unwrap()).unwrap();
    let params = KdfParams {
        m_kib: s["params"]["m_kib"].as_u64().unwrap() as u32,
        t: s["params"]["t"].as_u64().unwrap() as u32,
        p: s["params"]["p"].as_u64().unwrap() as u8,
    };
    let salt: [u8; 16] = hex::decode(s["salt"].as_str().unwrap()).unwrap().try_into().unwrap();
    let plaintext = s["plaintext"].as_str().unwrap().as_bytes();
    let sealed = encrypt_with_salt(&novel, &key, plaintext, SealOptions { kdf: params, compress: false }, &salt).unwrap();

    assert_eq!(hex::encode(&sealed), s["sealed"].as_str().unwrap());
    assert_eq!(armor::compact_encode(&sealed), s["compact_armor"].as_str().unwrap());
    assert_eq!(armor::text_encode(&sealed), s["text_armor"].as_str().unwrap());
    assert_eq!(armor::novel_encode(&novel, &sealed), s["novel_armor"].as_str().unwrap());
    assert_eq!(armor::story_encode(&novel, &sealed, Some("Alice")).unwrap(), s["story_armor"].as_str().unwrap());
    assert_eq!(armor::story_decode(&novel, s["story_armor"].as_str().unwrap()).unwrap(), sealed);
    assert_eq!(decrypt(&novel, &key, &sealed).unwrap(), plaintext);

    let std = &v["seal_standard_profile"];
    let std_sealed = encrypt_with_salt(&novel, &key, plaintext, SealOptions { compress: false, ..SealOptions::default() }, &salt).unwrap();
    assert_eq!(hex::encode(&std_sealed), std["sealed"].as_str().unwrap());
    assert_eq!(std_sealed.len(), plaintext.len() + 34);

    for d in v["decrypt_only"].as_array().unwrap() {
        let bytes = hex::decode(d["sealed"].as_str().unwrap()).unwrap();
        assert_eq!(decrypt(&novel, &key, &bytes).unwrap(), d["plaintext"].as_str().unwrap().as_bytes());
    }
}
