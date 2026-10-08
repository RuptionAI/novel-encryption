# Novel Encryption — Specification, version 1

Status: draft, 2026-10-08. Reference implementation: `crates/novel-encryption` (Rust).
Conformance vectors: `vectors/v1.json`. An implementation conforms if it reproduces
every value in the vectors and interoperates with the reference implementation.

The key words MUST, MUST NOT, SHOULD and MAY are used as in RFC 2119.

## 1. Overview

A **novel** is any text. From it we derive:

* a **fingerprint** that binds keys and messages to that novel (§3),
* a **narrative model** (§5.2) and a **chain graph** (§4) from which keys are drawn,
* an **armor table** and the narrative model, used to write ciphertext as the novel's
  words or as a new passage in its voice (§7).

A **key** is a sequence of words drawn at random from the novel in one of two styles (§5):
a **narrative** key (the default) follows the novel's own word sequences, so every three
consecutive words occur together in the novel; a **chain** key links words by letters,
each word beginning with the last letter of the word before it. Data is sealed with
Argon2id and XChaCha20-Poly1305 under a key derived from the novel's fingerprint and
the key words (§6).

## 2. Tokenization

`tokenize(text) -> [word]`:

1. Decode `text` as UTF-8 and apply Unicode normalization form **NFKD**.
2. Scan the code points in order, keeping a current word (initially empty):
   * If the code point is a combining mark (`General_Category` = Mn, Mc or Me) it is
     **skipped**: it neither joins nor ends a word.
   * If it is an ASCII letter `A–Z` or `a–z`, append its lowercase form to the current word.
   * Otherwise (digits, punctuation, whitespace, apostrophes, letters of other scripts),
     if the current word is non-empty, emit it and start a new one.
3. At the end, emit the current word if non-empty.

Every word therefore matches `^[a-z]+$`. Numerals never appear. Examples:
`"Café naïve, DON'T stop—1984 times! Ahab’s x2y"` →
`cafe naive don t stop times ahab s x y`.

> Implementations in languages whose Unicode tables differ slightly may disagree on
> rare characters. Catalog texts are verified by fingerprint (§3), so disagreement is
> detected, never silent.

## 3. Fingerprint

Let `D` be the set of distinct words produced by `tokenize(novel)`, sorted by byte
order ascending.

```
fingerprint = SHA-256( "NovelEncryption/v1/novel\n" || for w in D: w || "\n" )
```

The fingerprint depends only on vocabulary: re-wrapping, punctuation, case and word
order do not change it; adding, removing or altering any word does.

## 4. Key vocabulary and chain graph

1. `V0` = words of `D` whose length is between **3 and 15** inclusive.
2. **Prune dead ends**: repeat until no word is removed — let `S` be the set of
   initial letters of words in `V`; remove every word whose last letter is not in `S`.
3. The result `V` (sorted ascending) is the **key vocabulary**. If `|V| < 256` the
   novel MUST be rejected.
4. For each letter `c`, `V_c` is the (sorted) list of words of `V` beginning with `c`.

A word `w` may be followed by any word of `V_{last(w)}`. Pruning guarantees every
word has at least one successor.

## 5. Keys

All key generation uses a cryptographically secure random number generator and
**unbiased** integer choices (e.g. rejection sampling). `bits` is always the exact
surprisal of the generated key (−log2 of the probability the generator had of producing
it).

**Stopping rules.** *By strength* (default): stop as soon as `bits ≥ target` (default
**128**). *By length*: stop after exactly `n` words. Implementations SHOULD refuse keys
under 128 bits unless the user explicitly opts in, and SHOULD offer a **long-term**
target of **256 bits** for secrets that must withstand a quantum adversary (Grover's
search) for decades. Because the stopping decision depends only on the words generated
so far, the set of possible keys is prefix-free, and under the strength rule every
possible key has probability ≤ 2^−target.

### 5.1 Key parsing

`tokenize_key(s)` is `tokenize` (§2) except that an apostrophe (`'` U+0027 or `’`
U+2019) immediately between two ASCII letters is **dropped and joins** them
(`"It's"` → `its`). Keys are parsed with `tokenize_key`, so case, punctuation, hyphens,
apostrophes and line breaks are all accepted. The parsed words are the key's
**canonical words**, used for validation and key derivation (§6.1).

### 5.2 Narrative keys

**Prose tokenization.** From the novel text:

1. Remove every `[` … `]` span (non-nested, up to the next `]`).
2. Split into lines; drop **heading lines**: a line whose trimmed form has at least two
   letters and no lowercase letter, or whose trimmed form is shorter than 60 bytes and
   starts (case-insensitively) with `chapter `, `book `, `part `, `volume ` or `letter `.
3. Tokenize the remaining lines (joined by `\n`) with `tokenize_key` (§5.1), recording
   for each word the **gap** of characters before the next word.

A word **starts a sentence** if it is the first word, if the gap before it contains a
blank line (two or more `\n`), or if the gap after the previous word contains `.`, `!` or
`?`.

**Model.** Let `T` be the word stream. For every `i`, `(T[i], T[i+1], T[i+2])` is a
**trigram**; `n(a,b,c)` is its number of occurrences. A **context** is a pair `(a,b)`
that begins some trigram. Prune dead ends: repeatedly remove every trigram `(a,b,c)` for
which `(b,c)` is not a context, until none is removed. **Starts** are the distinct pairs
`(T[i], T[i+1])` where `T[i]` starts a sentence and `(T[i], T[i+1])` is a context.

**Generation.**

1. Choose `(w1, w2)` uniformly from the starts; `bits = log2 |starts|`.
2. Repeat: with context `(a,b)` = the last two words, let `N = Σ_c n(a,b,c)`. Choose `c`
   with probability `n(a,b,c) / N` (draw `r` uniformly in `0..N`, and walk the
   continuations in ascending order of `c`'s first appearance in the stream, subtracting
   counts). Append `c`; `bits += log2(N / n(a,b,c))`.
3. **Finishing the sentence** (strength rule only): once `bits ≥ target`, continue until
   the last two words `(a,b)` occur somewhere in the novel with `b` followed by `.`, `!`
   or `?`, or until 12 further words have been added, whichever comes first.

Narrative keys have at least 3 words.

**Display** (not part of the key). Each word is shown in its most frequent surface form
among occurrences that do not start a sentence (original capitals and apostrophes). After
each word, show the punctuation the novel most often places between that word and the
next: one of none, `,` `;` `:` `.` `!` `?` ` —` or `-` (a single hyphen, written with no
following space). Capitalize the first word and any word after `.`, `!` or `?`; end with
`.` unless the last mark is already terminal. Parsing the display form with
`tokenize_key` yields the canonical words.

### 5.3 Chain keys

1. `w1` = uniform choice from `V` (§4); `bits = log2 |V|`.
2. While the stopping rule is not met: `w_{i+1}` = uniform choice from `V_{last(w_i)}`;
   `bits += log2 |V_{last(w_i)}|`. Words MAY repeat.

Chain keys are displayed as words joined by `-` (e.g. `whale-empty-yonder`).

### 5.4 Validation

A key is valid for a novel if it is valid in **either** style:

* **Narrative:** at least 3 words, and every consecutive triple is a live trigram.
* **Chain:** every word is in `V`, and every word after the first begins with the last
  letter of its predecessor.

If neither holds, implementations SHOULD report the first failing word for the style the
key most resembles, comparing the share of consecutive triples that are live trigrams with
the share of consecutive pairs that link by letter.

## 6. Sealed message format

### 6.1 Key derivation

```
password = "NovelEncryption/v1/key\n" || fingerprint (32 bytes) || join(words, " ")
okm      = Argon2id v1.3 (password, salt, m = m_kib, t, p, output 56 bytes)
key      = okm[0..32]      nonce = okm[32..56]
```

No Argon2 secret or associated data is used. Every message has a fresh random salt and
therefore its own key and nonce, so the nonce is never stored.

**KDF profiles.** The *standard* profile is `m_kib = 65536`, `t = 3`, `p = 1`. Any other
parameters are written explicitly in the header.

### 6.2 Layout

All integers little-endian.

| size | field |
|---|---|
| 1 | format byte: `0xE1` = v1, standard profile; `0xE0` = v1, explicit parameters |
| 9 | *only if `0xE0`:* `m_kib` (u32), `t` (u32), `p` (u8) |
| 16 | salt (random, fresh for every message) |
| n+1+16 | XChaCha20-Poly1305 encryption of `flags ‖ payload`, then the 16-byte tag |

The AEAD associated data is the header: the format byte, any parameters, and the salt.

`flags` is one byte inside the ciphertext: `0x00` means the payload is the plaintext;
`0x01` means the payload is the plaintext compressed with raw DEFLATE (RFC 1951).
Encoders SHOULD compress only when the result is smaller. Because DEFLATE encoders
differ, compressed output is not reproducible across implementations; decoders MUST
accept any valid DEFLATE stream.

With the standard profile, a message is exactly **34 bytes** longer than its
(possibly compressed) payload.

Format bytes `0xE2`–`0xEF` are reserved for future versions.

### 6.3 Decryption

Implementations MUST reject: an unknown format byte; a message shorter than its header
plus 17 bytes; `p ∉ 1..16`, `t ∉ 1..64`, `m_kib ∉ 8p..1048576`; flags other than
`0x00`/`0x01`; and DEFLATE payloads that expand beyond 1 GiB. They MUST validate the
key against the novel (§5.3) and MUST NOT release any plaintext unless the tag verifies.

## 7. Armor

### 7.1 Compact armor

The message encoded as unpadded base64url (RFC 4648 §5) on one line, with no markers.
This is the smallest text form, suitable for chat, SMS and QR codes. Because the format
byte is `0xE0` or `0xE1`, compact strings begin with `4`.

### 7.2 Text armor

```
-----BEGIN NOVEL ENCRYPTION MESSAGE-----
<standard base64 with padding, 64 characters per line>
-----END NOVEL ENCRYPTION MESSAGE-----
```

Decoders ignore whitespace between the markers.

### 7.3 Novel armor

**Armor table**: the words of `V` sorted by occurrence count in the token stream
(descending), ties broken by byte order (ascending); the first 256 entries. Byte value
`b` is written as table entry `b`.

The encoder groups words into sentences for readability: a sentence starts at the
first byte and after each sentence ends; its length is `6 + (b mod 9)` words where `b`
is the sentence's first byte (the last sentence may be shorter). The first word of a
sentence is capitalized and the last is followed by `.`; each word after the first is
preceded by a newline if `L + 1 + len(word) + 1 > 72` (where `L` is the length of the
current line so far), and by a space otherwise. The output ends with a newline.

Decoding: `tokenize` the input and map each word back through the table; any word not
in the table is an error. Layout and punctuation are cosmetic, so only the word
sequence matters for interoperability; an encoder MAY lay words out differently, but
conformance vectors pin the reference layout.

### 7.4 Story armor (lost chapters)

Story armor writes a sealed message `M` (at least 17 bytes) as a passage generated by the
narrative model (§5.2).

1. **Lead with the salt.** `D = M[1..17] ‖ M[0] ‖ M[17..]`, so the random salt is encoded
   first.
2. **Payload.** `P = D[0..16] ‖ LEB128(len(D) − 16) ‖ D[16..]`. Encoders MUST reject
   `len(M) > 8192`.
3. **Ranked options.** Starts are ranked by the number of positions at which they begin a
   sentence (descending), ties by `(a, b)` ascending. The continuations of `(a, b)` are
   ranked by `n(a,b,c)` (descending), ties by `c`'s first appearance in the stream.
4. **Capacity.** A choice among `k` ranked options carries `m = min(⌊log2 k⌋, cap)` bits
   (0 if `k ≤ 1`), with `cap = 8` for the start and `cap = 2` for each later word. The
   option chosen is the one whose rank equals the next `m` bits of `P`, read
   most-significant bit first; after the end of `P` the bits read as 0.
5. **Ending.** After all of `P` has been read, continue (choosing rank 0) until the last
   two words end a sentence in the novel (§5.2, step 3) or 40 further words have been
   added.
6. **Layout.** Render the words with the display rules of §5.2, beneath the heading
   `<TITLE>: A LOST CHAPTER` (or `A LOST CHAPTER`) and a blank line. Encoders MAY wrap
   lines and break paragraphs freely.

**Decoding.** Discard everything up to and including the first case-insensitive
occurrence of `a lost chapter` within the first 300 bytes, and any heading lines (§5.2).
Tokenize the remainder with `tokenize_key`, recover the rank of each word among the top
`2^m` options (any other word is an error), and append its `m` bits until `P` is
complete. Then undo steps 2 and 1.

### 7.5 Detection

In order: input whose first byte is `0xE0` or `0xE1` is binary; input containing the
BEGIN marker is text armor; trimmed input consisting only of `A–Z a–z 0–9 - _` that
decodes as base64url to a sealed message is compact armor; input that decodes as novel
armor to a sealed message is novel armor; anything else is decoded as story armor and
MUST yield a sealed message.

## 8. Catalog

`catalog/catalog.json` lists public-domain novels with their pinned text file and
fingerprint. Clients MUST verify a fetched catalog text against its fingerprint before
use. A published text MUST never be modified; corrections are published as a new
catalog entry.
