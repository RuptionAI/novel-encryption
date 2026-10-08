use std::path::PathBuf;

use novel_encryption::{
    armor, check_key, decrypt, encrypt, generate, parse_header, Armor, Error, KdfParams, KeyLength,
    KeyPhrase, KeyStyle, Novel, SealOptions, LONG_TERM_KEY_BITS,
};

const FAST: SealOptions = SealOptions { kdf: KdfParams { m_kib: 1024, t: 1, p: 1 }, compress: true };

fn catalog(slug: &str) -> Novel {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("../../catalog/texts/{slug}.txt"));
    Novel::from_text(&std::fs::read_to_string(p).unwrap()).unwrap()
}

#[test]
fn generated_keys_chain_and_meet_strength() {
    let novel = catalog("alice-in-wonderland");
    for _ in 0..50 {
        let k = generate(&novel, KeyStyle::Chain, KeyLength::default(), false).unwrap();
        assert!(k.bits >= 128.0);
        assert_eq!(check_key(&novel, &k.phrase).unwrap().bits, k.bits);
        let w = k.phrase.words();
        for pair in w.windows(2) {
            assert_eq!(pair[0].as_bytes().last(), pair[1].as_bytes().first());
        }
    }
}

#[test]
fn fixed_depth_keys() {
    let novel = catalog("moby-dick");
    let k = generate(&novel, KeyStyle::Chain, KeyLength::Words(20), false).unwrap();
    assert_eq!(k.phrase.len(), 20);
    assert!(matches!(generate(&novel, KeyStyle::Chain, KeyLength::Words(3), false), Err(Error::WeakKey { .. })));
    let weak = generate(&novel, KeyStyle::Chain, KeyLength::Words(3), true).unwrap();
    assert_eq!(weak.phrase.len(), 3);
}

#[test]
fn roundtrip_every_armor() {
    let novel = catalog("frankenstein");
    let key = generate(&novel, KeyStyle::Narrative, KeyLength::default(), false).unwrap().phrase;
    let msg = b"You will rejoice to hear that no disaster has accompanied the commencement.";
    let sealed = encrypt(&novel, &key, msg, FAST).unwrap();
    for a in [Armor::Binary, Armor::Compact, Armor::Text, Armor::Novel, Armor::Story] {
        let carried = armor::encode(&novel, &sealed, a).unwrap();
        let (bytes, detected) = armor::decode_any(&novel, &carried).unwrap();
        assert_eq!(detected, a);
        assert_eq!(decrypt(&novel, &key, &bytes).unwrap(), msg);
    }
    assert_eq!(decrypt(&novel, &key, &encrypt(&novel, &key, b"", FAST).unwrap()).unwrap(), b"");
}

#[test]
fn key_parsing_is_forgiving() {
    let novel = catalog("dracula");
    let key = generate(&novel, KeyStyle::Narrative, KeyLength::default(), false).unwrap().phrase;
    let shouty = key.words().iter().map(|w| w.to_uppercase()).collect::<Vec<_>>().join(",  \n");
    assert_eq!(KeyPhrase::parse(&shouty).unwrap(), key);
}

#[test]
fn wrong_key_or_wrong_novel_fails() {
    let novel = catalog("pride-and-prejudice");
    let key = generate(&novel, KeyStyle::Narrative, KeyLength::default(), false).unwrap().phrase;
    let other = generate(&novel, KeyStyle::Narrative, KeyLength::default(), false).unwrap().phrase;
    let sealed = encrypt(&novel, &key, b"secret", FAST).unwrap();
    assert_eq!(decrypt(&novel, &other, &sealed), Err(Error::DecryptFailed));

    // Same words, different novel: the key usually does not even fit, and if
    // it does, the fingerprint changes the derived key.
    let alice = catalog("alice-in-wonderland");
    assert!(decrypt(&alice, &key, &sealed).is_err());
}

#[test]
fn tampering_is_detected() {
    let novel = catalog("treasure-island");
    let key = generate(&novel, KeyStyle::Narrative, KeyLength::default(), false).unwrap().phrase;
    let sealed = encrypt(&novel, &key, b"fifteen men on the dead man's chest", FAST).unwrap();
    for i in [0, 1, 9, 10, 26, 30, sealed.len() - 1] {
        let mut bad = sealed.clone();
        bad[i] ^= 1;
        assert!(decrypt(&novel, &key, &bad).is_err(), "flip at {i} accepted");
    }
}

#[test]
fn chain_typos_are_located() {
    let novel = catalog("moby-dick");
    let key = generate(&novel, KeyStyle::Chain, KeyLength::default(), false).unwrap().phrase;
    let mut words = key.words().to_vec();
    words[2] = "zzzq".into();
    match check_key(&novel, &KeyPhrase::parse(&words.join(" ")).unwrap()) {
        Err(Error::KeyWordUnknown { position: 3, .. }) => {}
        other => panic!("{other:?}"),
    }
    // A real word that breaks the chain.
    let mut words = key.words().to_vec();
    let last = *words[0].as_bytes().last().unwrap();
    let breaker = novel.vocabulary().iter().find(|w| w.as_bytes()[0] != last).unwrap();
    words[1] = breaker.clone();
    match check_key(&novel, &KeyPhrase::parse(&words.join(" ")).unwrap()) {
        Err(Error::KeyChainBroken { position: 2, .. }) => {}
        other => panic!("{other:?}"),
    }
}

#[test]
fn hostile_headers_are_bounded() {
    let novel = catalog("alice-in-wonderland");
    let key = generate(&novel, KeyStyle::Narrative, KeyLength::default(), false).unwrap().phrase;
    let mut sealed = encrypt(&novel, &key, b"x", FAST).unwrap();
    sealed[1..5].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(matches!(parse_header(&sealed), Err(Error::InvalidParams(_))));
    assert!(matches!(decrypt(&novel, &key, &sealed), Err(Error::InvalidParams(_))));
}

#[test]
fn every_catalog_novel_is_usable() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../catalog/texts");
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        let novel = Novel::from_text(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let s = novel.stats();
        assert!(s.key_vocabulary >= 1000, "{path:?}: {s:?}");
        assert!(s.words_for_128_bits <= 20, "{path:?}: {s:?}");
    }
}

#[test]
fn compression_shrinks_text_and_is_skipped_when_useless() {
    let novel = catalog("moby-dick");
    let key = generate(&novel, KeyStyle::Narrative, KeyLength::default(), false).unwrap().phrase;
    let text = std::fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../catalog/texts/alice-in-wonderland.txt"),
    )
    .unwrap();
    let sealed = encrypt(&novel, &key, &text, FAST).unwrap();
    assert!(sealed.len() < text.len() / 2, "{} vs {}", sealed.len(), text.len());
    assert_eq!(decrypt(&novel, &key, &sealed).unwrap(), text);

    // Short or incompressible data is stored as-is: overhead is exactly 34 bytes
    // with the standard profile (custom profiles add 9 bytes of parameters).
    let raw = SealOptions { compress: false, ..FAST };
    assert_eq!(encrypt(&novel, &key, b"hi", raw).unwrap().len(), 2 + 34 + 9);
    assert_eq!(encrypt(&novel, &key, b"hi", FAST).unwrap().len(), 2 + 34 + 9);
    assert_eq!(novel_encryption::seal::OVERHEAD, 34);
}

#[test]
fn long_term_keys() {
    let novel = catalog("war-and-peace");
    let k = generate(&novel, KeyStyle::Chain, KeyLength::Bits(LONG_TERM_KEY_BITS), false).unwrap();
    assert!(k.bits >= 256.0);
    assert!(k.phrase.len() >= 25 && k.phrase.len() <= 32, "{}", k.phrase.len());
}

#[test]
fn narrative_keys_follow_the_book() {
    let novel = catalog("pride-and-prejudice");
    for _ in 0..20 {
        let k = generate(&novel, KeyStyle::Narrative, KeyLength::default(), false).unwrap();
        assert!(k.bits >= 128.0, "{}", k.bits);
        let c = check_key(&novel, &k.phrase).unwrap();
        assert_eq!(c.style, KeyStyle::Narrative);
        assert!((c.bits - k.bits).abs() < 1e-9);
        // The punctuated display form parses back to the same key.
        assert_eq!(KeyPhrase::parse(&k.display).unwrap(), k.phrase);
        assert!(k.display.ends_with(['.', '!', '?']), "{}", k.display);
    }
    let exact = generate(&novel, KeyStyle::Narrative, KeyLength::Words(40), true).unwrap();
    assert_eq!(exact.phrase.len(), 40);
}

#[test]
fn narrative_typos_are_located() {
    let novel = catalog("moby-dick");
    let key = generate(&novel, KeyStyle::Narrative, KeyLength::default(), false).unwrap().phrase;
    let mut words = key.words().to_vec();
    words[5] = "zzzq".into();
    match check_key(&novel, &KeyPhrase::parse(&words.join(" ")).unwrap()) {
        Err(Error::KeyWordUnknown { position: 6, .. }) => {}
        other => panic!("{other:?}"),
    }
    // Swapping two words leaves only real words but breaks the sequence.
    let mut words = key.words().to_vec();
    words.swap(10, 11);
    if words[10] != words[11] {
        match check_key(&novel, &KeyPhrase::parse(&words.join(" ")).unwrap()) {
            Err(Error::KeyNarrativeBroken { position, .. }) => assert!((11..=13).contains(&position)),
            Ok(_) => {} // the swapped order also occurs in the book; harmless
            other => panic!("{other:?}"),
        }
    }
}

#[test]
fn long_term_narrative_keys() {
    let novel = catalog("moby-dick");
    let k = generate(&novel, KeyStyle::Narrative, KeyLength::Bits(LONG_TERM_KEY_BITS), false).unwrap();
    assert!(k.bits >= 256.0);
    let sealed = encrypt(&novel, &k.phrase, b"for decades", FAST).unwrap();
    assert_eq!(decrypt(&novel, &KeyPhrase::parse(&k.display).unwrap(), &sealed).unwrap(), b"for decades");
}

#[test]
fn story_armor_is_a_lost_chapter() {
    let novel = catalog("moby-dick");
    let key = generate(&novel, KeyStyle::Narrative, KeyLength::default(), false).unwrap().phrase;
    for msg in [&b"Meet me at the library at noon."[..], &[0u8; 300][..], &[0xffu8; 7][..]] {
        let sealed = encrypt(&novel, &key, msg, FAST).unwrap();
        let story = armor::story_encode(&novel, &sealed, Some("Moby Dick")).unwrap();
        assert!(story.starts_with("MOBY DICK: A LOST CHAPTER\n\n"), "{story}");
        let (bytes, a) = armor::decode_any(&novel, story.as_bytes()).unwrap();
        assert_eq!(a, Armor::Story);
        assert_eq!(decrypt(&novel, &key, &bytes).unwrap(), msg);
        // Re-wrapping, re-casing and stripping punctuation do not matter.
        let flat = story.replace(['\n', '.', ',', ';', '!', '?'], " ").to_lowercase();
        assert_eq!(armor::story_decode(&novel, &flat).unwrap(), sealed);
    }
    // Editing a word is caught (or, if it happens to fit, the AEAD rejects it).
    let sealed = encrypt(&novel, &key, b"x", FAST).unwrap();
    let story = armor::story_encode(&novel, &sealed, None).unwrap();
    let tampered = story.replacen(" the ", " a ", 1);
    if tampered != story {
        let r = armor::decode_any(&novel, tampered.as_bytes()).and_then(|(b, _)| decrypt(&novel, &key, &b));
        assert!(r.is_err());
    }
}
