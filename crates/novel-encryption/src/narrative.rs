//! Narrative keys: passages walked through the book's own word sequences.
//!
//! The book is read as a stream of words. A narrative key starts at a word
//! pair that begins a sentence somewhere in the book, then repeatedly picks
//! the next word given the previous two, with probability proportional to how
//! often the book continues that pair with it. Every three consecutive words
//! of a key therefore appear together in the book, so keys read like the book.
//!
//! Strength is exact: the generator adds -log2(probability) of every choice,
//! and stops only after reaching the target, so every possible key has
//! probability at most 2^-target.

use std::collections::{HashMap, HashSet};

use unicode_normalization::char::is_combining_mark;
use unicode_normalization::UnicodeNormalization;

use crate::{rng, Error, Result};

/// Punctuation classes recorded after each word, for display only.
const P_NONE: u8 = 0;
const P_HYPHEN: u8 = 8;
const PUNCT: [&str; 9] = ["", ",", ";", ":", ".", "!", "?", " —", "-"];

fn is_terminal(p: u8) -> bool {
    (4..=6).contains(&p)
}

/// Words a sentence may run on for, past the strength target, while the
/// generator looks for a natural sentence end.
pub const SENTENCE_GRACE_WORDS: usize = 12;

/// Story armor: bits carried by the choice of opening (among the most common
/// sentence openings) and by each later word (among the most common
/// continuations). Smaller caps read more naturally but make longer stories.
pub const STORY_START_BITS: u32 = 8;
pub const STORY_STEP_BITS: u32 = 2;
/// Words the story may run on, after the message is spent, to end a sentence.
pub const STORY_GRACE_WORDS: usize = 40;
/// Largest message story armor will carry.
pub const STORY_MAX_BYTES: usize = 8192;
/// Bytes carried before the length prefix.
const STORY_LEAD: usize = 16;

/// Is this line a heading (chapter title, all-caps line) that keys should skip?
pub(crate) fn is_heading(line: &str) -> bool {
    let t = line.trim();
    let letters = t.chars().filter(|c| c.is_alphabetic()).count();
    if letters >= 2 && !t.chars().any(|c| c.is_lowercase()) {
        return true;
    }
    let lower = t.to_lowercase();
    t.len() < 60
        && ["chapter ", "book ", "part ", "volume ", "letter "]
            .iter()
            .any(|p| lower.starts_with(p))
}

/// Remove `[...]` editorial insertions such as "[Illustration]".
fn strip_brackets(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(open) = rest.find('[') {
        match rest[open..].find(']') {
            Some(close) => {
                out.push_str(&rest[..open]);
                rest = &rest[open + close + 1..];
            }
            None => break,
        }
    }
    out.push_str(rest);
    out
}

/// A word as it appears in the text: canonical lowercase letters (apostrophes
/// dropped) plus the surface form with its original capitals and apostrophes.
struct Token {
    canonical: String,
    surface: String,
    punct_after: u8,
    paragraph_before: bool,
}

/// Tokenize prose for narrative keys. Like [`crate::tokenize`], except that an
/// apostrophe between letters joins them ("it's" → `its`), headings and
/// bracketed insertions are skipped, and punctuation is recorded for display.
fn tokenize_prose(text: &str) -> Vec<Token> {
    let mut body = String::with_capacity(text.len());
    for line in strip_brackets(text).lines() {
        if !is_heading(line) {
            body.push_str(line);
        }
        body.push('\n');
    }

    let mut out: Vec<Token> = Vec::new();
    let mut canon = String::new();
    let mut surface = String::new();
    let mut gap = String::new(); // characters since the last word
    let mut paragraph_before = true;
    let mut chars = body.nfkd().filter(|c| !is_combining_mark(*c)).peekable();
    while let Some(c) = chars.next() {
        if c.is_ascii_alphabetic() {
            if canon.is_empty() {
                // A word begins: the gap behind it decides the previous word's
                // punctuation and whether this word opens a paragraph.
                if let Some(prev) = out.last_mut() {
                    prev.punct_after = classify(&gap);
                    paragraph_before = gap.matches('\n').count() >= 2;
                }
                gap.clear();
            }
            canon.push(c.to_ascii_lowercase());
            surface.push(c);
        } else if (c == '\'' || c == '\u{2019}')
            && !canon.is_empty()
            && chars.peek().is_some_and(|n| n.is_ascii_alphabetic())
        {
            surface.push('\'');
        } else {
            if !canon.is_empty() {
                out.push(Token {
                    canonical: std::mem::take(&mut canon),
                    surface: std::mem::take(&mut surface),
                    punct_after: P_NONE,
                    paragraph_before,
                });
            }
            gap.push(c);
        }
    }
    if !canon.is_empty() {
        out.push(Token { canonical: canon, surface, punct_after: P_NONE, paragraph_before });
    }
    if let Some(prev) = out.last_mut() {
        prev.punct_after = classify(&gap);
    }
    out
}

fn classify(gap: &str) -> u8 {
    for c in gap.chars() {
        match c {
            '.' => return 4,
            '!' => return 5,
            '?' => return 6,
            _ => {}
        }
    }
    if gap.contains(';') {
        2
    } else if gap.contains(':') {
        3
    } else if gap.contains(',') {
        1
    } else if gap.contains('—') || gap.contains("--") {
        7
    } else if gap == "-" {
        P_HYPHEN
    } else if gap.matches('\n').count() >= 2 {
        4 // a paragraph break ends a sentence
    } else {
        P_NONE
    }
}

/// Canonical form of a typed narrative key word list (apostrophes joined).
pub fn tokenize_key(text: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut cur = String::new();
    let mut chars = text.nfkd().filter(|c| !is_combining_mark(*c)).peekable();
    while let Some(c) = chars.next() {
        if c.is_ascii_alphabetic() {
            cur.push(c.to_ascii_lowercase());
        } else if (c == '\'' || c == '\u{2019}') && !cur.is_empty() && chars.peek().is_some_and(|n| n.is_ascii_alphabetic()) {
            // joined, dropped
        } else if !cur.is_empty() {
            words.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        words.push(cur);
    }
    words
}

/// The book as an order-2 word model.
#[derive(Clone)]
pub struct NarrativeModel {
    words: Vec<String>,
    ids: HashMap<String, u32>,
    display: Vec<String>,
    stream: Vec<u32>,
    punct_after: Vec<u8>,
    /// Sorted (a, b, c, count) for every live trigram.
    trigrams: Vec<(u32, u32, u32, u32)>,
    /// Sorted distinct live contexts that begin a sentence.
    starts: Vec<(u32, u32)>,
    /// Sorted distinct pairs (a, b) that end a sentence somewhere in the book.
    finals: Vec<(u32, u32)>,
    /// Starts ranked by how often they open a sentence (desc), then by pair.
    starts_ranked: Vec<(u32, u32)>,
}

impl NarrativeModel {
    pub fn from_text(text: &str) -> NarrativeModel {
        let tokens = tokenize_prose(text);
        let mut ids: HashMap<String, u32> = HashMap::new();
        let mut words: Vec<String> = Vec::new();
        let mut surfaces: Vec<HashMap<String, u32>> = Vec::new();
        let mut stream = Vec::with_capacity(tokens.len());
        let mut punct_after = Vec::with_capacity(tokens.len());
        let mut sentence_start = Vec::with_capacity(tokens.len());

        let mut prev_terminal = true;
        for t in &tokens {
            let id = *ids.entry(t.canonical.clone()).or_insert_with(|| {
                words.push(t.canonical.clone());
                surfaces.push(HashMap::new());
                (words.len() - 1) as u32
            });
            let starts_sentence = prev_terminal || t.paragraph_before;
            // Casing seen mid-sentence is the word's natural casing.
            if !starts_sentence {
                *surfaces[id as usize].entry(t.surface.clone()).or_default() += 1;
            }
            stream.push(id);
            punct_after.push(t.punct_after);
            sentence_start.push(starts_sentence);
            prev_terminal = is_terminal(t.punct_after);
        }

        let display = words
            .iter()
            .zip(&surfaces)
            .map(|(w, s)| {
                s.iter()
                    .max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(a.0)))
                    .map(|(k, _)| k.clone())
                    .unwrap_or_else(|| w.clone())
            })
            .collect();

        let mut counts: HashMap<(u32, u32, u32), u32> = HashMap::new();
        for w in stream.windows(3) {
            *counts.entry((w[0], w[1], w[2])).or_default() += 1;
        }
        let mut trigrams: Vec<(u32, u32, u32, u32)> =
            counts.into_iter().map(|((a, b, c), n)| (a, b, c, n)).collect();
        // Prune dead ends: a trigram is live only if its last pair can continue.
        loop {
            let ctx: HashSet<(u32, u32)> = trigrams.iter().map(|t| (t.0, t.1)).collect();
            let before = trigrams.len();
            trigrams.retain(|t| ctx.contains(&(t.1, t.2)));
            if trigrams.len() == before {
                break;
            }
        }
        trigrams.sort_unstable();
        let live: HashSet<(u32, u32)> = trigrams.iter().map(|t| (t.0, t.1)).collect();

        let mut start_counts: HashMap<(u32, u32), u32> = HashMap::new();
        for i in 0..stream.len().saturating_sub(1) {
            let p = (stream[i], stream[i + 1]);
            if sentence_start[i] && live.contains(&p) {
                *start_counts.entry(p).or_default() += 1;
            }
        }
        let mut starts: Vec<(u32, u32)> = start_counts.keys().copied().collect();
        starts.sort_unstable();
        let mut starts_ranked = starts.clone();
        starts_ranked.sort_by(|x, y| start_counts[y].cmp(&start_counts[x]).then(x.cmp(y)));

        let mut finals: Vec<(u32, u32)> = (1..stream.len())
            .filter(|&i| is_terminal(punct_after[i]))
            .map(|i| (stream[i - 1], stream[i]))
            .collect();
        finals.sort_unstable();
        finals.dedup();

        NarrativeModel { words, ids, display, stream, punct_after, trigrams, starts, finals, starts_ranked }
    }

    /// Fraction of consecutive word triples of `words` that occur in the book.
    pub fn triple_share(&self, words: &[String]) -> f64 {
        if words.len() < 3 {
            return 0.0;
        }
        let id = |w: &String| self.ids.get(w).copied();
        let hits = words
            .windows(3)
            .filter(|t| match (id(&t[0]), id(&t[1]), id(&t[2])) {
                (Some(a), Some(b), Some(c)) => self.continuations(a, b).iter().any(|x| x.2 == c),
                _ => false,
            })
            .count();
        hits as f64 / (words.len() - 2) as f64
    }

    /// Continuations of (a, b) ranked by count (desc), then by word id.
    fn ranked(&self, a: u32, b: u32) -> Vec<u32> {
        let mut c: Vec<(u32, u32)> = self.continuations(a, b).iter().map(|t| (t.2, t.3)).collect();
        c.sort_by(|x, y| y.1.cmp(&x.1).then(x.0.cmp(&y.0)));
        c.into_iter().map(|x| x.0).collect()
    }

    /// Write `data` (at least 16 bytes) as a new passage in the book's voice.
    ///
    /// The payload is `data[..16] || LEB128(len - 16) || data[16..]`: the first
    /// 16 bytes come first so that, when they are random (a salt), every story
    /// opens differently. It is read as bits, most significant first; each
    /// choice among the 2^m most common options carries m bits. After the data
    /// is spent the story takes the most common path to a sentence end.
    pub fn story_encode(&self, data: &[u8]) -> Result<Vec<String>> {
        self.story_encode_with(data, STORY_START_BITS, STORY_STEP_BITS)
    }

    /// [`Self::story_encode`] with explicit capacities: bits carried by the
    /// opening and by each later word (larger means shorter but less natural).
    pub fn story_encode_with(&self, data: &[u8], start_bits: u32, step_bits: u32) -> Result<Vec<String>> {
        if data.len() < STORY_LEAD {
            return Err(Error::Armor(format!("story armor needs at least {STORY_LEAD} bytes")));
        }
        if data.len() > STORY_MAX_BYTES {
            return Err(Error::Armor(format!(
                "story armor carries at most {STORY_MAX_BYTES} bytes; use compact or binary for larger data"
            )));
        }
        if self.starts_ranked.is_empty() {
            return Err(Error::NovelTooSmall { found: 0, required: 1 });
        }
        let mut payload = data[..STORY_LEAD].to_vec();
        payload.extend_from_slice(&leb128((data.len() - STORY_LEAD) as u32));
        payload.extend_from_slice(&data[STORY_LEAD..]);
        let mut bits = BitReader { data: &payload, pos: 0 };

        let m0 = capacity(self.starts_ranked.len(), start_bits);
        let (a, b) = self.starts_ranked[bits.read(m0) as usize];
        let mut key = vec![a, b];
        let mut grace = 0;
        loop {
            let (x, y) = (key[key.len() - 2], key[key.len() - 1]);
            if bits.done() {
                if self.ends_sentence(x, y) || grace >= STORY_GRACE_WORDS {
                    break;
                }
                grace += 1;
            }
            let ranked = self.ranked(x, y);
            let m = capacity(ranked.len(), step_bits);
            key.push(ranked[bits.read(m) as usize]);
        }
        Ok(key.iter().map(|&i| self.words[i as usize].clone()).collect())
    }

    /// Recover the data carried by a story (inverse of [`Self::story_encode`]).
    pub fn story_decode(&self, words: &[String]) -> Result<Vec<u8>> {
        self.story_decode_with(words, STORY_START_BITS, STORY_STEP_BITS)
    }

    /// Inverse of [`Self::story_encode_with`]; the capacities must match.
    pub fn story_decode_with(&self, words: &[String], start_bits: u32, step_bits: u32) -> Result<Vec<u8>> {
        let bad = |i: usize, w: &str| {
            Error::Armor(format!("word {} (\"{w}\") is not how this book would continue the story", i + 1))
        };
        let mut ids = Vec::with_capacity(words.len());
        for (i, w) in words.iter().enumerate() {
            ids.push(*self.ids.get(w).ok_or_else(|| bad(i, w))?);
        }
        if ids.len() < 2 {
            return Err(Error::Armor("story is too short".into()));
        }
        let mut out = BitWriter::default();
        let m0 = capacity(self.starts_ranked.len(), start_bits);
        let first = self.starts_ranked[..1 << m0]
            .iter()
            .position(|p| *p == (ids[0], ids[1]))
            .ok_or_else(|| bad(1, &words[1]))?;
        out.write(first as u32, m0);
        for i in 2..ids.len() {
            if let Some(data) = out.message() {
                return Ok(data);
            }
            let ranked = self.ranked(ids[i - 2], ids[i - 1]);
            let m = capacity(ranked.len(), step_bits);
            let p = ranked[..1 << m].iter().position(|&c| c == ids[i]).ok_or_else(|| bad(i, &words[i]))?;
            out.write(p as u32, m);
        }
        out.message().ok_or_else(|| Error::Armor("story ends before its message does".into()))
    }

    /// A deterministic walk (first start, then always the first continuation),
    /// for conformance vectors only. Never use it for real keys.
    pub fn first_walk(&self, len: usize) -> Vec<String> {
        let (a, b) = self.starts[0];
        let mut key = vec![a, b];
        while key.len() < len {
            let c = self.continuations(key[key.len() - 2], key[key.len() - 1])[0].2;
            key.push(c);
        }
        key.iter().map(|&i| self.words[i as usize].clone()).collect()
    }

    /// Whether the word occurs anywhere in the book's prose.
    pub fn knows(&self, word: &str) -> bool {
        self.ids.contains_key(word)
    }

    pub fn start_count(&self) -> usize {
        self.starts.len()
    }

    pub fn trigram_count(&self) -> usize {
        self.trigrams.len()
    }

    fn continuations(&self, a: u32, b: u32) -> &[(u32, u32, u32, u32)] {
        let lo = self.trigrams.partition_point(|t| (t.0, t.1) < (a, b));
        let hi = self.trigrams.partition_point(|t| (t.0, t.1) <= (a, b));
        &self.trigrams[lo..hi]
    }

    fn ends_sentence(&self, a: u32, b: u32) -> bool {
        self.finals.binary_search(&(a, b)).is_ok()
    }

    /// Generate a key. `min_bits`: stop once reached (then finish the
    /// sentence, within [`SENTENCE_GRACE_WORDS`]). `exact_words`: stop at
    /// exactly that many words instead.
    pub(crate) fn generate(&self, min_bits: Option<f64>, exact_words: Option<usize>, max_words: usize) -> Result<(Vec<String>, f64)> {
        if self.starts.is_empty() {
            return Err(Error::NovelTooSmall { found: 0, required: 1 });
        }
        let (a, b) = self.starts[rng::uniform(self.starts.len() as u32)? as usize];
        let mut key = vec![a, b];
        let mut bits = (self.starts.len() as f64).log2();
        let mut grace = 0usize;
        loop {
            let (x, y) = (key[key.len() - 2], key[key.len() - 1]);
            match (min_bits, exact_words) {
                (_, Some(n)) if key.len() >= n => break,
                (Some(t), None) if bits >= t => {
                    if self.ends_sentence(x, y) || grace >= SENTENCE_GRACE_WORDS {
                        break;
                    }
                    grace += 1;
                }
                _ => {}
            }
            if key.len() >= max_words {
                return Err(Error::InvalidLength(format!("more than {max_words} words needed")));
            }
            let cont = self.continuations(x, y);
            let total: u64 = cont.iter().map(|t| t.3 as u64).sum();
            // Unbiased pick in 0..total, then walk the cumulative counts.
            let mut r = rng::uniform(u32::try_from(total).expect("count fits u32"))? as u64;
            let mut chosen = cont[0];
            for t in cont {
                if r < t.3 as u64 {
                    chosen = *t;
                    break;
                }
                r -= t.3 as u64;
            }
            bits += (total as f64 / chosen.3 as f64).log2();
            key.push(chosen.2);
        }
        Ok((key.iter().map(|&i| self.words[i as usize].clone()).collect(), bits))
    }

    /// Validate a narrative key; returns its bits if generated by this model.
    /// On failure returns the 1-based position of the first word that breaks it.
    pub(crate) fn check(&self, words: &[String]) -> core::result::Result<f64, (usize, String)> {
        let mut ids = Vec::with_capacity(words.len());
        for (i, w) in words.iter().enumerate() {
            match self.ids.get(w) {
                Some(&id) => ids.push(id),
                None => return Err((i + 1, w.clone())),
            }
        }
        if ids.len() < 3 {
            return Err((ids.len() + 1, String::new()));
        }
        let mut bits = (self.starts.len() as f64).log2();
        for i in 2..ids.len() {
            let cont = self.continuations(ids[i - 2], ids[i - 1]);
            let Some(t) = cont.iter().find(|t| t.2 == ids[i]) else {
                return Err((i + 1, words[i].clone()));
            };
            let total: u64 = cont.iter().map(|t| t.3 as u64).sum();
            bits += (total as f64 / t.3 as f64).log2();
        }
        Ok(bits)
    }

    /// Render a key as prose, with the book's usual capitals and the
    /// punctuation the book most often puts between each pair of words.
    pub fn render(&self, words: &[String]) -> String {
        let ids: Vec<Option<u32>> = words.iter().map(|w| self.ids.get(w).copied()).collect();
        let mut tally: HashMap<(u32, u32), [u32; 9]> = HashMap::new();
        for p in ids.windows(2) {
            if let [Some(a), Some(b)] = p {
                tally.insert((*a, *b), [0; 9]);
            }
        }
        for i in 1..self.stream.len() {
            if let Some(t) = tally.get_mut(&(self.stream[i - 1], self.stream[i])) {
                t[self.punct_after[i - 1] as usize] += 1;
            }
        }
        let mut out = String::new();
        let mut cap = true;
        let mut joined = false;
        for (i, w) in words.iter().enumerate() {
            let shown = ids[i].map(|id| self.display[id as usize].as_str()).unwrap_or(w);
            if i > 0 && !joined {
                out.push(' ');
            }
            if cap {
                let mut cs = shown.chars();
                if let Some(f) = cs.next() {
                    out.push(f.to_ascii_uppercase());
                    out.push_str(cs.as_str());
                }
            } else {
                out.push_str(shown);
            }
            let p = match (ids[i], ids.get(i + 1).copied().flatten()) {
                (Some(a), Some(b)) => tally
                    .get(&(a, b))
                    .map(|t| (0..9).max_by(|&x, &y| t[x].cmp(&t[y]).then(y.cmp(&x))).unwrap() as u8)
                    .unwrap_or(P_NONE),
                _ => P_NONE,
            };
            if i + 1 == words.len() {
                out.push_str(if is_terminal(p) { PUNCT[p as usize] } else { "." });
            } else {
                out.push_str(PUNCT[p as usize]);
            }
            cap = is_terminal(p);
            joined = p == P_HYPHEN && i + 1 < words.len();
        }
        out
    }

    /// Long-run expected bits per word, from the stationary distribution of
    /// the walk over contexts (power iteration). Used for statistics only.
    pub fn bits_per_word(&self) -> f64 {
        let mut ctxs: Vec<(u32, u32)> = self.trigrams.iter().map(|t| (t.0, t.1)).collect();
        ctxs.dedup();
        let index = |c: (u32, u32)| ctxs.binary_search(&c).unwrap();
        let n = ctxs.len();
        let src: Vec<usize> = self.trigrams.iter().map(|t| index((t.0, t.1))).collect();
        let dst: Vec<usize> = self.trigrams.iter().map(|t| index((t.1, t.2))).collect();
        let mut total = vec![0f64; n];
        for (t, &i) in self.trigrams.iter().zip(&src) {
            total[i] += t.3 as f64;
        }
        let prob: Vec<f64> = self.trigrams.iter().zip(&src).map(|(t, &i)| t.3 as f64 / total[i]).collect();
        let mut entropy = vec![0f64; n];
        for (&p, &i) in prob.iter().zip(&src) {
            entropy[i] -= p * p.log2();
        }
        let mut dist = vec![0f64; n];
        for s in &self.starts {
            dist[index(*s)] += 1.0 / self.starts.len() as f64;
        }
        // Average over the walk's first 200 steps (keys are walks of that order).
        let steps = 200;
        let mut acc = 0f64;
        for _ in 0..steps {
            acc += dist.iter().zip(&entropy).map(|(d, h)| d * h).sum::<f64>();
            let mut next = vec![0f64; n];
            for k in 0..prob.len() {
                next[dst[k]] += dist[src[k]] * prob[k];
            }
            dist = next;
        }
        acc / steps as f64
    }
}

/// Bits carried by a choice among `k` options, capped.
fn capacity(k: usize, cap: u32) -> u32 {
    if k <= 1 {
        0
    } else {
        k.ilog2().min(cap)
    }
}

fn leb128(mut n: u32) -> Vec<u8> {
    let mut out = Vec::new();
    loop {
        let b = (n & 0x7f) as u8;
        n >>= 7;
        if n == 0 {
            out.push(b);
            return out;
        }
        out.push(b | 0x80);
    }
}

/// Reads bits most-significant first; past the end it yields zeros.
struct BitReader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl BitReader<'_> {
    fn done(&self) -> bool {
        self.pos >= self.data.len() * 8
    }

    fn read(&mut self, m: u32) -> u32 {
        let mut v = 0;
        for _ in 0..m {
            let bit = self.data.get(self.pos / 8).map_or(0, |b| (b >> (7 - self.pos % 8)) & 1);
            v = (v << 1) | bit as u32;
            self.pos += 1;
        }
        v
    }
}

#[derive(Default)]
struct BitWriter {
    bytes: Vec<u8>,
    nbits: usize,
}

impl BitWriter {
    fn write(&mut self, v: u32, m: u32) {
        for i in (0..m).rev() {
            if self.nbits.is_multiple_of(8) {
                self.bytes.push(0);
            }
            if (v >> i) & 1 == 1 {
                *self.bytes.last_mut().unwrap() |= 1 << (7 - self.nbits % 8);
            }
            self.nbits += 1;
        }
    }

    /// The message, once the lead bytes, the length prefix and all remaining
    /// bytes have arrived.
    fn message(&self) -> Option<Vec<u8>> {
        let complete = self.nbits / 8;
        let (mut len, mut shift, mut i) = (0usize, 0, STORY_LEAD);
        loop {
            let b = *self.bytes[..complete].get(i)?;
            len |= ((b & 0x7f) as usize) << shift;
            i += 1;
            if b & 0x80 == 0 {
                break;
            }
            shift += 7;
            if shift > 28 {
                return Some(Vec::new()); // corrupt length; decryption will reject it
            }
        }
        (complete >= i + len).then(|| [&self.bytes[..STORY_LEAD], &self.bytes[i..i + len]].concat())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prose_tokens_join_apostrophes_and_skip_headings() {
        let t = tokenize_prose("CHAPTER I.\n\nIt's Ahab’s ship. [Illustration]\nCall me Ishmael!");
        let c: Vec<&str> = t.iter().map(|t| t.canonical.as_str()).collect();
        assert_eq!(c, ["its", "ahabs", "ship", "call", "me", "ishmael"]);
        assert_eq!(t[1].surface, "Ahab's");
        assert_eq!(t[2].punct_after, 4);
        assert_eq!(tokenize_key("It's, AHAB'S—ship"), ["its", "ahabs", "ship"]);
    }

    #[test]
    fn bits_roundtrip() {
        let data: Vec<u8> = (0..=255).collect();
        let mut r = BitReader { data: &data, pos: 0 };
        let mut w = BitWriter::default();
        while !r.done() {
            w.write(r.read(3), 3);
        }
        assert_eq!(&w.bytes[..256], &data[..]);
        assert_eq!(leb128(300), [0xac, 0x02]);
    }
}
