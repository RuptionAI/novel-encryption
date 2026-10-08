# Every book is a key

This morning I had an idea. By tonight it was live at novelencryption.com.

Here is how it happened, including the part where my first idea turned out to be breakable.

## A question from the library board

I serve on a library board. Like libraries everywhere, we watch patrons check out fewer and fewer physical books. Libraries have adapted well: makerspaces, classes, community rooms. But the book itself still does one thing. It gets read, and then it goes back on the shelf.

So this morning I asked myself a simple question. What else could a book do? Could a book you love be useful in a brand-new way, and still be loved for being a book?

## The first idea

My first thought was encryption. Take a novel and break it into words. Replace each letter of a secret message with a randomly chosen word from the book that starts with that letter, and chain more words together by their last letters, as deep as you like. The novel plus the table of chosen words would be the key.

It felt clever. It was also broken.

Write "HELLO" with Moby-Dick and you get something like: **harpoon, ever, leviathan, long, ocean.** Now read the first letters. Anyone can.

This is one of the oldest lessons in cryptography. Benedict Arnold sent messages in a book cipher in 1779. In 1883, Auguste Kerckhoffs wrote that a system has to stay secure even when everything except the key is public. A library catalog is about as public as it gets. **The book can't be the secret.**

## Keeping what was good

What was good about the idea was never the cipher. It was the book: shared, familiar, delightful. So I kept the book and replaced the cipher.

- **The book supplies the words.** Your key is a passage drawn at random from the book, and it reads like the book, because every three words in a row appear together somewhere in it. One I drew from War and Peace: *"Denisov came into the passage of the commander in chief. Malasha, who had settled into three armies…"*
- **The randomness is the secret, and it's measured exactly.** Every key is at least 128 bits, or 256 for secrets that need to last decades, even against future quantum computers.
- **The lock is standard and proven:** Argon2id and XChaCha20-Poly1305, the same building blocks used by serious security tools. Everything runs in your browser. Your book, key and message never leave your device.

## Lost chapters

Then came the fun part. Seal a note with your book and key, and it comes back as a **lost chapter**: a brand-new passage in the book's own voice that secretly carries your encrypted message.

> MOBY-DICK: A LOST CHAPTER
>
> But when you come back to it. But what is the great Sperm whale, and the other end of it, I say, we good Presbyterian Christians should be the…

Fan fiction that only your key can read. Anyone else just holds a strange new chapter of Moby-Dick.

## From idea to launch in a day

By tonight there was a Rust library and command-line tool, a website that runs entirely in your browser, a byte-exact specification, and an 18-page white paper. The paper includes the section on why my first idea fails. I built it with Claude Code as my pair programmer.

The shelf has twelve public-domain classics and the King James Bible, which gives the shortest keys of all. You can also bring your own book.

It's open source. It hasn't been independently audited yet, and reviews are welcome.

## What's next

- **Libraries:** "Encrypt with a Classic" workshops, book clubs trading lost chapters, a key bookmark at the circulation desk. If your library wants to try it, my DMs are open.
- **Wallet backups (already built):** write a crypto wallet's 24-word backup phrase as a passage from a book you love, or as a short chain of its words, and convert it back whenever you need it. It runs offline, in a command-line tool or a single page that can't touch the network.
- **Fan fiction with a plot:** lost chapters with a real storyline.

Books get read, then shelved. Now they can keep a secret.

**Try it:** novelencryption.com
**White paper:** novelencryption.com/whitepaper.html
**Code:** github.com/RuptionAI/novel-encryption
