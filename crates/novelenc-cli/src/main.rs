use std::io::{IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand, ValueEnum};
use novel_encryption::{
    armor, check_key, decrypt, encrypt, generate, Armor, KdfParams, KeyLength, KeyPhrase, KeyStyle,
    Novel, SealOptions, DEFAULT_KEY_BITS, LONG_TERM_KEY_BITS,
};
use novel_encryption::seed::{self, SeedStyle};
use zeroize::Zeroizing;

/// Novel Encryption: keys drawn from a novel as a chain of words, data sealed
/// with Argon2id + XChaCha20-Poly1305.
#[derive(Parser)]
#[command(name = "novelenc", version, about)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Show a novel's fingerprint and key-space statistics.
    Inspect { novel: PathBuf },
    /// Generate a new key from a novel.
    Keygen {
        novel: PathBuf,
        /// narrative: a passage that follows the book's own word sequences
        /// (default). chain: shorter words linked by letters, easier to memorize.
        #[arg(long, value_enum, default_value_t = StyleArg::Narrative)]
        style: StyleArg,
        /// Minimum strength in bits (words are added until it is reached).
        #[arg(long, default_value_t = DEFAULT_KEY_BITS, conflicts_with_all = ["words", "long_term"])]
        bits: f64,
        /// Long-term key: 256 bits, for secrets that must stay safe for decades
        /// (keeps a 128-bit margin against quantum search).
        #[arg(long, conflicts_with = "words")]
        long_term: bool,
        /// Exact length in words, instead of --bits.
        #[arg(long)]
        words: Option<usize>,
        /// Permit keys under 128 bits (not recommended).
        #[arg(long)]
        allow_weak: bool,
        /// Print the bare lowercase words instead of the display form.
        #[arg(long)]
        plain: bool,
    },
    /// Check that a key fits a novel and report its strength.
    Check {
        novel: PathBuf,
        #[command(flatten)]
        key: KeySource,
    },
    /// Encrypt data with a novel and key.
    Encrypt {
        novel: PathBuf,
        #[command(flatten)]
        key: KeySource,
        #[command(flatten)]
        io: Io,
        /// Output format [default: compact to stdout, binary to a file].
        #[arg(long, value_enum)]
        armor: Option<ArmorArg>,
        /// Do not compress before encrypting (compression is used only when it helps).
        #[arg(long)]
        no_compress: bool,
        /// Argon2id memory in MiB.
        #[arg(long, default_value_t = 64)]
        kdf_memory_mib: u32,
        /// Argon2id iterations.
        #[arg(long, default_value_t = 3)]
        kdf_iterations: u32,
    },
    /// Decrypt data (any armor is detected automatically).
    Decrypt {
        novel: PathBuf,
        #[command(flatten)]
        key: KeySource,
        #[command(flatten)]
        io: Io,
    },
    /// Wallet backups: write a BIP-39 recovery phrase as a book passage, and back.
    #[command(subcommand)]
    Seed(SeedCmd),
    /// Detailed key-space and typo-detection analysis (used for the white paper).
    Analyze {
        #[arg(required = true)]
        novels: Vec<PathBuf>,
    },
}

#[derive(Subcommand)]
enum SeedCmd {
    /// Recovery phrase → passage. The phrase is read from a hidden prompt (or stdin).
    To {
        novel: PathBuf,
        /// narrative: reads like the book (~125–270 words for 24 words).
        /// chain: ~35–50 linked words, practical to copy by hand.
        #[arg(long, value_enum, default_value_t = StyleArg::Narrative)]
        style: StyleArg,
    },
    /// Passage → recovery phrase. The passage is read from -i FILE or stdin.
    From {
        novel: PathBuf,
        #[arg(short, long)]
        input: Option<PathBuf>,
    },
}

#[derive(Args)]
struct KeySource {
    /// Read the key from a file.
    #[arg(long, conflicts_with = "key")]
    key_file: Option<PathBuf>,
    /// The key itself (visible in shell history; prefer --key-file or the prompt).
    #[arg(long, env = "NOVELENC_KEY", hide_env_values = true)]
    key: Option<String>,
}

#[derive(Args)]
struct Io {
    /// Input file [default: stdin].
    #[arg(short, long)]
    input: Option<PathBuf>,
    /// Output file [default: stdout].
    #[arg(short, long)]
    output: Option<PathBuf>,
}

#[derive(Clone, Copy, PartialEq, ValueEnum)]
enum StyleArg {
    Narrative,
    Chain,
}

#[derive(Clone, Copy, ValueEnum)]
enum ArmorArg {
    /// Raw bytes (smallest).
    Binary,
    /// One base64url line, for chat, SMS and QR codes.
    Compact,
    /// Base64 block with BEGIN/END lines, for email.
    Text,
    /// Words from the novel.
    Novel,
    /// A new passage in the book's voice: a "lost chapter".
    Story,
}

impl From<ArmorArg> for Armor {
    fn from(a: ArmorArg) -> Self {
        match a {
            ArmorArg::Binary => Armor::Binary,
            ArmorArg::Compact => Armor::Compact,
            ArmorArg::Text => Armor::Text,
            ArmorArg::Novel => Armor::Novel,
            ArmorArg::Story => Armor::Story,
        }
    }
}

type Res<T> = Result<T, String>;

fn load_novel(p: &Path) -> Res<Novel> {
    let text = std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))?;
    Novel::from_text(&text).map_err(|e| format!("{}: {e}", p.display()))
}

fn load_key(src: &KeySource) -> Res<KeyPhrase> {
    let raw = Zeroizing::new(match (&src.key_file, &src.key) {
        (Some(f), _) => std::fs::read_to_string(f).map_err(|e| format!("{}: {e}", f.display()))?,
        (None, Some(k)) => k.clone(),
        (None, None) => rpassword::prompt_password("Key: ").map_err(|e| format!("reading key: {e}"))?,
    });
    KeyPhrase::parse(&raw).map_err(|e| e.to_string())
}

fn read_input(io: &Io) -> Res<Vec<u8>> {
    match &io.input {
        Some(p) => std::fs::read(p).map_err(|e| format!("{}: {e}", p.display())),
        None => {
            let mut buf = Vec::new();
            std::io::stdin().read_to_end(&mut buf).map_err(|e| e.to_string())?;
            Ok(buf)
        }
    }
}

fn write_output(io: &Io, data: &[u8]) -> Res<()> {
    match &io.output {
        Some(p) => std::fs::write(p, data).map_err(|e| format!("{}: {e}", p.display())),
        None => std::io::stdout().write_all(data).map_err(|e| e.to_string()),
    }
}

fn run(cli: Cli) -> Res<()> {
    match cli.cmd {
        Cmd::Inspect { novel } => {
            let s = load_novel(&novel)?.stats();
            println!("fingerprint          {}", s.fingerprint);
            println!("words in text        {}", s.total_words);
            println!("distinct words       {}", s.distinct_words);
            println!("key vocabulary       {}", s.key_vocabulary);
            println!("first word           {:.2} bits", s.first_word_bits);
            println!("each chained word    {:.2} bits (long-run average)", s.chain_bits_per_word);
            println!("words for 128 bits   ~{}", s.words_for_128_bits);
        }
        Cmd::Keygen { novel, style, bits, long_term, words, allow_weak, plain } => {
            let novel = load_novel(&novel)?;
            let bits = if long_term { LONG_TERM_KEY_BITS } else { bits };
            let length = words.map(KeyLength::Words).unwrap_or(KeyLength::Bits(bits));
            let style = if style == StyleArg::Chain { KeyStyle::Chain } else { KeyStyle::Narrative };
            let k = generate(&novel, style, length, allow_weak).map_err(|e| e.to_string())?;
            println!("{}", if plain { k.phrase.words().join(" ") } else { k.display.clone() });
            eprintln!("{} words, {:.1} bits", k.phrase.len(), k.bits);
        }
        Cmd::Check { novel, key } => {
            let novel = load_novel(&novel)?;
            let key = load_key(&key)?;
            let c = check_key(&novel, &key).map_err(|e| e.to_string())?;
            let bits = c.bits;
            println!("ok: {:?} key, {} words, {bits:.1} bits if randomly generated", c.style, key.len());
            if bits < DEFAULT_KEY_BITS {
                eprintln!("warning: below the recommended {DEFAULT_KEY_BITS:.0} bits");
            }
        }
        Cmd::Encrypt { novel, key, io, armor: a, no_compress, kdf_memory_mib, kdf_iterations } => {
            let novel_path = novel;
            let novel = load_novel(&novel_path)?;
            let key = load_key(&key)?;
            let bits = check_key(&novel, &key).map_err(|e| e.to_string())?.bits;
            if bits < DEFAULT_KEY_BITS {
                eprintln!("warning: key is only {bits:.1} bits; {DEFAULT_KEY_BITS:.0}+ recommended");
            }
            let kdf = KdfParams { m_kib: kdf_memory_mib.saturating_mul(1024), t: kdf_iterations, p: 1 };
            let opts = SealOptions { kdf, compress: !no_compress };
            let data = read_input(&io)?;
            let sealed = encrypt(&novel, &key, &data, opts).map_err(|e| e.to_string())?;
            let a = a.map(Armor::from).unwrap_or(if io.output.is_some() { Armor::Binary } else { Armor::Compact });
            if a == Armor::Binary && io.output.is_none() && std::io::stdout().is_terminal() {
                return Err("refusing to write binary to a terminal; use -o or --armor compact".into());
            }
            let title = novel_path.file_stem().map(|s| s.to_string_lossy().replace('-', " "));
            let out = match a {
                Armor::Story => armor::story_encode(&novel, &sealed, title.as_deref()).map(String::into_bytes),
                _ => armor::encode(&novel, &sealed, a),
            }
            .map_err(|e| e.to_string())?;
            write_output(&io, &out)?;
        }
        Cmd::Decrypt { novel, key, io } => {
            let novel = load_novel(&novel)?;
            let key = load_key(&key)?;
            let (bytes, _) = armor::decode_any(&novel, &read_input(&io)?).map_err(|e| e.to_string())?;
            let plain = Zeroizing::new(decrypt(&novel, &key, &bytes).map_err(|e| e.to_string())?);
            write_output(&io, &plain)?;
        }
        Cmd::Seed(SeedCmd::To { novel, style }) => {
            let novel = load_novel(&novel)?;
            let phrase = Zeroizing::new(if std::io::stdin().is_terminal() {
                rpassword::prompt_password("Recovery phrase (hidden): ").map_err(|e| e.to_string())?
            } else {
                let mut s = String::new();
                std::io::stdin().read_to_string(&mut s).map_err(|e| e.to_string())?;
                s
            });
            let style = if style == StyleArg::Chain { SeedStyle::Chain } else { SeedStyle::Narrative };
            let p = seed::to_passage(&novel, &phrase, style).map_err(|e| e.to_string())?;
            println!("{}", p.display);
            eprintln!(
                "{} words carrying your {}-word recovery phrase. Anyone with this passage and the book can \
                 take your funds: store it like the phrase itself. Test a restore before relying on it.",
                p.words.len(),
                p.phrase_words
            );
        }
        Cmd::Seed(SeedCmd::From { novel, input }) => {
            let novel = load_novel(&novel)?;
            let text = Zeroizing::new(read_input(&Io { input, output: None })?);
            let text = std::str::from_utf8(&text).map_err(|_| "the passage is not UTF-8 text".to_string())?;
            let phrase = seed::from_passage(&novel, text).map_err(|e| e.to_string())?;
            println!("{}", *phrase);
        }
        Cmd::Analyze { novels } => {
            for p in novels {
                analyze(&p, &load_novel(&p)?);
            }
        }
    }
    Ok(())
}

/// Key-space statistics plus the fraction of single-edit typos in a chained
/// key word that the chain rule catches, versus vocabulary membership alone.
fn analyze(path: &Path, novel: &Novel) {
    let s = novel.stats();
    let name = path.file_stem().unwrap().to_string_lossy();
    let vocab = novel.vocabulary();

    let (mut total, mut in_vocab, mut undetected) = (0u64, 0u64, 0u64);
    for w in vocab {
        for t in single_edits(w) {
            total += 1;
            if novel.contains(&t) {
                in_vocab += 1;
                // A middle word keeps the chain only if its first and last
                // letters are unchanged.
                if t.as_bytes()[0] == w.as_bytes()[0] && t.as_bytes().last() == w.as_bytes().last() {
                    undetected += 1;
                }
            }
        }
    }
    let pct = |n: u64| 100.0 * n as f64 / total as f64;
    let nm = novel.narrative();
    let start_bits = (nm.start_count() as f64).log2();
    let rate = nm.bits_per_word();
    let est = |t: f64| 2 + ((t - start_bits) / rate).ceil() as usize;
    let sample = |t: f64| {
        let n = 40;
        (0..n)
            .map(|_| generate(novel, KeyStyle::Narrative, KeyLength::Bits(t), false).unwrap().phrase.len())
            .sum::<usize>() as f64
            / n as f64
    };
    println!(
        "{name}\tfp={}\ttokens={}\tdistinct={}\tvocab={}\tfirst={:.2}\tsecond={:.2}\tchain={:.2}\twords128={}\t\
         typos={total}\tmissed_vocab_only={:.2}%\tmissed_with_chain={:.2}%\t\
         narr_starts={}\tnarr_trigrams={}\tnarr_start_bits={:.2}\tnarr_bits_per_word={:.2}\t\
         narr_est128={}\tnarr_mean128={:.1}\tnarr_est256={}\tnarr_mean256={:.1}",
        &s.fingerprint[..16],
        s.total_words,
        s.distinct_words,
        s.key_vocabulary,
        s.first_word_bits,
        s.second_word_bits,
        s.chain_bits_per_word,
        s.words_for_128_bits,
        pct(in_vocab),
        pct(undetected),
        nm.start_count(),
        nm.trigram_count(),
        start_bits,
        rate,
        est(128.0),
        sample(128.0),
        est(256.0),
        sample(256.0),
    );
}

/// All distinct lowercase strings one substitution, deletion, insertion or
/// adjacent transposition away from `w`.
fn single_edits(w: &str) -> Vec<String> {
    let b = w.as_bytes();
    let mut out = std::collections::BTreeSet::new();
    for i in 0..b.len() {
        for c in b'a'..=b'z' {
            if c != b[i] {
                let mut v = b.to_vec();
                v[i] = c;
                out.insert(v);
            }
        }
        let mut v = b.to_vec();
        v.remove(i);
        out.insert(v);
        if i + 1 < b.len() && b[i] != b[i + 1] {
            let mut v = b.to_vec();
            v.swap(i, i + 1);
            out.insert(v);
        }
    }
    for i in 0..=b.len() {
        for c in b'a'..=b'z' {
            let mut v = b.to_vec();
            v.insert(i, c);
            out.insert(v);
        }
    }
    out.remove(b);
    out.into_iter().filter(|v| !v.is_empty()).map(|v| String::from_utf8(v).unwrap()).collect()
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("novelenc: {e}");
            ExitCode::FAILURE
        }
    }
}
