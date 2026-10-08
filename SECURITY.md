# Security

## Status

Novel Encryption v1 is a **draft that has not been independently audited**. The
construction uses only standard primitives (Argon2id, XChaCha20-Poly1305, SHA-256)
from the RustCrypto project, but the composition, the key generator and the
implementation are new and deserve review.

## What it protects, and what it does not

* Protects the confidentiality and integrity of data sealed under a **generated** key
  (128 bits by default), even against an attacker who knows which novel was used.
* The novel is **not** a secret. Strength figures never count it.
* Keys you make up yourself are weak, whatever their bit count says. Always generate.
* Novel armor is an encoding, not steganography: it does not hide that a message exists
  or how long it is.
* No public-key exchange, sender authentication or forward secrecy.
* The website runs entirely in your browser, but a web page is only as trustworthy as
  the code it serves. For high-stakes use, run the CLI from source.

## Reporting a vulnerability

Please report privately through a GitHub security advisory on
[RuptionAI/novel-encryption](https://github.com/RuptionAI/novel-encryption/security/advisories/new)
rather than in a public issue. We aim to acknowledge within three business days.
