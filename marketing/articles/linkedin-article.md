# Every Book Is a Key: How a Library Board Question Became Novel Encryption

This morning I had an idea. By tonight it was live at novelencryption.com, published by Ruption AI and me. This is the story of that day, including the part where my first idea turned out to be breakable, and what I learned from it.

## The question

I serve on a library board. Like libraries across the country, we see patrons checking out fewer and fewer physical books. Libraries have responded well, adding makerspaces, classes and community programs. But the book itself still has one job: it is read, and then it is shelved.

So I asked a simple question this morning: **what else can a book do?** Could a book people love become useful in a new way, without becoming anything less than a book?

## The first idea, and why it failed

My first answer was encryption. Break a novel into words. Replace each letter of a secret message with a randomly chosen word from the book that starts with that letter, and chain further words together by their last letters. The novel and the table of chosen words would form the key.

It was a satisfying idea, and it did not work. Encode "HELLO" with Moby-Dick and you get something like *harpoon, ever, leviathan, long, ocean*. The first letters spell the message for anyone who looks.

That failure is centuries old. Book ciphers were used in the American Revolution, and in 1883 Auguste Kerckhoffs set out the principle that still governs the field: a system must remain secure even when everything about it except the key is public. A public-domain library is, by definition, public. **The book cannot be the secret.**

## Keeping the book, replacing the cipher

The valuable part of the idea was never the cipher. It was the book: shared, familiar and genuinely delightful. So Novel Encryption keeps the book and puts the security where it belongs.

1. **The book supplies the words.** A key is a passage drawn at random from the book, so it reads like the book: every three consecutive words appear together somewhere in the text. An example from War and Peace: *"Denisov came into the passage of the commander in chief. Malasha, who had settled into three armies…"*
2. **The randomness is the secret, and it is measured exactly.** Every key carries at least 128 bits of strength by default, or 256 bits for long-term secrets, which also preserves a comfortable margin against future quantum computers.
3. **The protection is standard.** Data is sealed with Argon2id and XChaCha20-Poly1305, well-studied algorithms used throughout modern security software. Everything runs in the user's browser; no book, key or message is sent to a server.

## Lost chapters

The most enjoyable part came next. A sealed message can be written as a **lost chapter**: a new passage in the author's voice whose word choices carry the encrypted message. Readers with the right book and key recover the message. Everyone else holds an unfamiliar chapter of Moby-Dick.

It is a small piece of fan fiction that doubles as a sealed letter.

## Shipped in a day, in the open

In a single day the project went from idea to launch:

- an open-source Rust library and command-line tool;
- a website that runs entirely in the browser, with a catalog of twelve public-domain classics and the King James Bible;
- a byte-exact specification with test vectors for other implementations;
- a 17-page white paper covering the design, measurements and security analysis, including a section on why the original idea fails.

I built it with Claude Code as my pair programmer. The project has not yet been independently audited, and I welcome review from the security community.

## An invitation

**For libraries and educators:** I see Novel Encryption as a program, not just a tool. Picture an "Encrypt with a Classic" workshop that opens by breaking the naive cipher by hand, which makes a wonderful first lesson in why cryptography is hard. Picture book clubs trading lost chapters, or a key bookmark handed out at the circulation desk. If your library or classroom would like to pilot it, please reach out.

**For security professionals:** the specification, test vectors and white paper are public. I would value your critique.

**Coming next:** a converter that writes a crypto wallet's 24-word backup phrase as a passage from a book you love, and back again.

Books are read and then shelved. Now they can also keep a secret.

**Try it:** novelencryption.com
**White paper (web and PDF):** novelencryption.com/whitepaper.html
**Source code:** github.com/RuptionAI/novel-encryption

#Libraries #Cryptography #OpenSource
