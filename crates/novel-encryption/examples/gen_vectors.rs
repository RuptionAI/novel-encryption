//! Regenerate `vectors/v1.json`, the cross-language conformance vectors.
//!
//!     cargo run -p novel-encryption --example gen_vectors > vectors/v1.json
//!
//! Vectors are pinned: only regenerate when deliberately changing the spec.

use std::path::PathBuf;

use novel_encryption::{armor, check_key, encrypt_with_salt, tokenize, KdfParams, KeyPhrase, Novel, SealOptions};

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn main() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let text = std::fs::read_to_string(root.join("catalog/texts/alice-in-wonderland.txt")).unwrap();
    let novel = Novel::from_text(&text).unwrap();
    let stats = novel.stats();

    // A fixed chain built deterministically from the vocabulary: the first
    // word, then repeatedly the first successor, 16 words long.
    let mut words = vec![novel.vocabulary()[0].clone()];
    while words.len() < 16 {
        let last = *words.last().unwrap().as_bytes().last().unwrap();
        words.push(novel.successors(last).next().unwrap().to_string());
    }
    let key = KeyPhrase::parse(&words.join(" ")).unwrap();
    let bits = check_key(&novel, &key).unwrap().bits;

    let params = KdfParams { m_kib: 1024, t: 1, p: 1 };
    let salt: [u8; 16] = core::array::from_fn(|i| i as u8);
    let plaintext = "Curiouser and curiouser!";
    // Reproducible vectors store the payload uncompressed (deflate output varies by library).
    let opts = SealOptions { kdf: params, compress: false };
    let sealed = encrypt_with_salt(&novel, &key, plaintext.as_bytes(), opts, &salt).unwrap();

    // Compressed payload: decrypt-only, since deflate encoders differ.
    let long = "Alice was beginning to get very tired of sitting by her sister on the bank. ".repeat(8);
    let salt2: [u8; 16] = core::array::from_fn(|i| 0x10 + i as u8);
    let compressed = encrypt_with_salt(&novel, &key, long.as_bytes(), SealOptions { compress: true, ..opts }, &salt2).unwrap();
    assert!(compressed.len() < long.len());

    // Standard profile, for the 34-byte overhead (slow: 64 MiB Argon2id).
    let std_sealed =
        encrypt_with_salt(&novel, &key, plaintext.as_bytes(), SealOptions { compress: false, ..SealOptions::default() }, &salt).unwrap();

    let tok_in = "Café naïve, DON'T stop—1984 times! Ahab’s x2y";
    let key_in = "It's, AHAB'S—ship. Café";
    let nm = novel.narrative();
    let walk = nm.first_walk(30);
    let walk_key = KeyPhrase::parse(&walk.join(" ")).unwrap();
    let walk_bits = check_key(&novel, &walk_key).unwrap().bits;
    let v = serde_json::json!({
        "spec": "NovelEncryption v1",
        "tokenize": [{ "input": tok_in, "output": tokenize(tok_in) }],
        "novel": {
            "file": "catalog/texts/alice-in-wonderland.txt",
            "fingerprint": novel.fingerprint_hex(),
            "total_words": stats.total_words,
            "distinct_words": stats.distinct_words,
            "key_vocabulary": stats.key_vocabulary,
            "armor_table_first_8": novel_armor_prefix(&novel),
        },
        "key": { "phrase": key.to_hyphenated(), "bits": format!("{bits:.6}") },
        "key_tokenize": [{ "input": key_in, "output": novel_encryption::narrative::tokenize_key(key_in) }],
        "narrative": {
            "start_contexts": nm.start_count(),
            "live_trigrams": nm.trigram_count(),
            "first_walk_30": walk.join(" "),
            "first_walk_30_bits": format!("{walk_bits:.6}"),
            "first_walk_30_display": nm.render(&walk),
        },
        "seal": {
            "params": { "m_kib": params.m_kib, "t": params.t, "p": params.p, "compress": false },
            "salt": hex(&salt),
            "plaintext": plaintext,
            "sealed": hex(&sealed),
            "compact_armor": armor::compact_encode(&sealed),
            "text_armor": armor::text_encode(&sealed),
            "novel_armor": armor::novel_encode(&novel, &sealed),
            "story_armor": armor::story_encode(&novel, &sealed, Some("Alice")).unwrap(),
        },
        "seal_standard_profile": { "salt": hex(&salt), "plaintext": plaintext, "sealed": hex(&std_sealed) },
        "decrypt_only": [{ "note": "deflate-compressed payload", "plaintext": long, "sealed": hex(&compressed) }]
    });
    println!("{}", serde_json::to_string_pretty(&v).unwrap());
}

fn novel_armor_prefix(novel: &Novel) -> Vec<String> {
    (0u8..8).map(|b| armor::novel_encode(novel, &[b]).trim().trim_end_matches('.').to_lowercase()).collect()
}
