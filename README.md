# Novel Encryption

**Turn a book into a key, and your messages into lost chapters.** Pick a novel and draw
a key from it: a passage chosen at random that reads like the book, because every three
words in a row appear together somewhere in it. Seal your data with the book and the key,
and the result can itself be written as a new chapter in the book's voice. Only the same
book and the same key open it.

> Denisov came into the passage of the commander in chief. Malasha, who had settled into
> three armies. First the vicomte to ask so naive a question for a moment on a low voice,
> evidently an infantry officer who said he hoped to find out everything very thoroughly
> and accurately as every German has to. *(135 bits, War and Peace. Example only.)*

Prefer a key you can memorize? `--style chain` draws a short letter chain instead:
`doubt → talisman → nobly → yea → avatar → …` (14 words for 128 bits).

The book supplies the words and is bound into every key and message. The security
comes from a randomly generated chain of measured strength (128 bits by default) and
standard cryptography — Argon2id and XChaCha20-Poly1305 — never from word
substitution. Read the [white paper](docs/WHITEPAPER.md) for why.

Published by **Ruption AI** and **Jeff Pittman** · [novelencryption.com](https://novelencryption.com)

## Try it

```sh
cargo install --path crates/novelenc-cli      # installs `novelenc`

novelenc inspect catalog/texts/moby-dick.txt  # key-space stats for a book
novelenc keygen  catalog/texts/moby-dick.txt  # new 128-bit narrative key (--style chain, --words N)
novelenc keygen  catalog/texts/moby-dick.txt --long-term   # 256-bit key for secrets that must last decades

# encrypt / decrypt (prompts for the key; or --key-file, or NOVELENC_KEY)
echo "Call me Ishmael." | novelenc encrypt catalog/texts/moby-dick.txt --armor story > chapter.txt
novelenc decrypt catalog/texts/moby-dick.txt -i chapter.txt
```

A sealed message is only **34 bytes** larger than its contents, and the book never
travels with it. Data is compressed first when that helps, so sealed text files are
usually smaller than the originals. Armor: `story` writes the message as a lost chapter
of the book, `novel` as one book word per byte, `compact` as one base64url line (default
on stdout), `text` as a BEGIN/END block, and `binary` as raw bytes (default with `-o`).

## Wallet backups

Write a crypto wallet's BIP-39 recovery phrase (12–24 words) into a book, and back:

```sh
novelenc seed to   catalog/texts/king-james-bible.txt                # passage (~124 words for 24)
novelenc seed to   catalog/texts/king-james-bible.txt --style chain  # chain (~35 words; 34–48 by book)
novelenc seed from catalog/texts/king-james-bible.txt -i backup.txt  # back to the phrase
```

The book form carries exactly the phrase's entropy plus a 32-bit check bound to the book,
so it always restores the original phrase and rejects the wrong book, changed words, or
text copied from the book. For people who prefer a browser, `tools/novel-seed-offline.html`
is one self-contained file that cannot reach the network; open it with your computer
offline. Treat the book-form backup exactly like the phrase itself.

## The catalog

Thirteen public-domain books from Project Gutenberg, pinned by fingerprint in
[`catalog/catalog.json`](catalog/catalog.json): *Moby-Dick*, *Pride and Prejudice*,
*Frankenstein*, *Dracula*, *A Tale of Two Cities*, *Great Expectations*, *Jane Eyre*,
*The Adventures of Sherlock Holmes*, *Treasure Island*, *Alice's Adventures in
Wonderland*, *War and Peace*, *Middlemarch* and the King James Bible (which gives the
shortest narrative keys: about 44 words for 128 bits). The King James Version is in the
public domain in the United States; in the United Kingdom, rights in it are held by the
Crown. Any text with at least 256 usable words
works, including your own.

## Repository

| Path | |
|---|---|
| `crates/novel-encryption` | Rust reference implementation (library) |
| `crates/novelenc-cli` | `novelenc` command-line tool |
| `crates/novel-encryption-wasm` | WebAssembly bindings for the website |
| `site/` | novelencryption.com source (`scripts/build_site.sh` → `dist/`) |
| `catalog/` | public-domain texts, sources and fingerprints |
| `infra/aws/` | hosting on AWS: S3 + CloudFront + Route 53 (`./deploy.sh`) |
| `tools/` | offline wallet-backup page (`scripts/build_seed_tool.sh`) |
| `docs/SPEC.md` | byte-exact specification |
| `docs/WHITEPAPER.md` | design, measurements and security analysis ([PDF](docs/novel-encryption-whitepaper.pdf); rebuild with `scripts/build_pdf.sh`) |
| `vectors/v1.json` | conformance vectors for other-language ports |

```sh
cargo test --release --workspace
cargo run --release -p novel-encryption --example build_catalog   # refresh catalog.json
scripts/fetch_catalog.py                                          # (re)download texts
```

## Status

Version 1 draft. **Not yet independently audited.** See [SECURITY.md](SECURITY.md).

## License

Code: MIT OR Apache-2.0, at your option. White paper: CC BY 4.0. Catalog texts are in
the public domain (sourced from Project Gutenberg; license headers removed).
