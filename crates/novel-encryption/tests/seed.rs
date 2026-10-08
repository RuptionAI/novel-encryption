use std::path::PathBuf;

use novel_encryption::seed::{entropy_to_mnemonic, from_passage, to_passage, SeedStyle};
use novel_encryption::{rng, Error, Novel};

fn catalog(slug: &str) -> Novel {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("../../catalog/texts/{slug}.txt"));
    Novel::from_text(&std::fs::read_to_string(p).unwrap()).unwrap()
}

fn random_phrase(bytes: usize) -> String {
    let mut e = vec![0u8; bytes];
    rng::fill(&mut e).unwrap();
    entropy_to_mnemonic(&e).unwrap().to_string()
}

#[test]
fn every_phrase_length_round_trips_in_both_styles() {
    let novel = catalog("king-james-bible");
    for bytes in [16, 20, 24, 28, 32] {
        for style in [SeedStyle::Narrative, SeedStyle::Chain] {
            for _ in 0..5 {
                let phrase = random_phrase(bytes);
                let p = to_passage(&novel, &phrase, style).unwrap();
                // The display form (punctuation, capitals, line breaks) decodes too.
                assert_eq!(*from_passage(&novel, &p.display).unwrap(), phrase, "{style:?} {bytes}");
                assert_eq!(*from_passage(&novel, &p.words.join(" ")).unwrap(), phrase);
            }
        }
    }
}

#[test]
fn chains_are_short_and_passages_read_like_the_book() {
    let novel = catalog("moby-dick");
    let phrase = random_phrase(32);
    let chain = to_passage(&novel, &phrase, SeedStyle::Chain).unwrap();
    assert!((30..=40).contains(&chain.words.len()), "{}", chain.words.len());
    for pair in chain.words.windows(2) {
        assert_eq!(pair[0].as_bytes().last(), pair[1].as_bytes().first());
    }
    let passage = to_passage(&novel, &phrase, SeedStyle::Narrative).unwrap();
    assert!(passage.display.ends_with(['.', '!', '?']));
    assert!(novel_encryption::narrative::tokenize_key(&passage.display).len() == passage.words.len());
}

#[test]
fn wrong_book_copied_text_and_edits_are_rejected() {
    let moby = catalog("moby-dick");
    let alice = catalog("alice-in-wonderland");
    let phrase = random_phrase(32);
    for style in [SeedStyle::Narrative, SeedStyle::Chain] {
        let p = to_passage(&moby, &phrase, style).unwrap();
        assert!(matches!(from_passage(&alice, &p.display), Err(Error::Seed(_))), "{style:?} wrong book");
        let mut words = p.words.clone();
        let i = words.len() / 2;
        words[i] = if words[i] == "whale" { "ship".into() } else { "whale".into() };
        assert!(from_passage(&moby, &words.join(" ")).is_err(), "{style:?} edited word");
    }
    // A passage copied straight out of the book must never become a wallet.
    let text = std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../catalog/texts/moby-dick.txt")).unwrap();
    for start in [5_000, 50_000, 200_000, 600_000] {
        let copied: String = text[start..start + 1500].to_string();
        assert!(from_passage(&moby, &copied).is_err(), "copied text at {start} decoded");
    }
}
