// Web Worker: holds the prepared Novel and runs the slow parts (tokenizing a
// whole book, 64 MiB Argon2id) off the main thread.
import init, { Novel, defaultKdf } from "./pkg/novel_encryption_wasm.js";

const ready = init();
let novel = null;

const ops = {
  async load({ text, expectedFingerprint }) {
    if (novel) { novel.free(); novel = null; }
    const n = new Novel(text);
    if (expectedFingerprint && n.fingerprint !== expectedFingerprint) {
      const got = n.fingerprint;
      n.free();
      throw new Error(`This text's fingerprint (${got.slice(0, 16)}…) does not match the catalog. The download may be damaged.`);
    }
    novel = n;
    return novel.stats();
  },
  generateKey({ style, bits, words, allowWeak }) {
    return need().generateKey(style, bits, words, allowWeak);
  },
  checkKey({ key }) {
    return need().checkKey(key);
  },
  encrypt({ key, data, armor, title }) {
    const [m, t] = defaultKdf();
    const out = need().encrypt(key, data, armor, m, t, true, title || ""); // compress when it helps
    return { result: out, transfer: [out.buffer] };
  },
  decrypt({ key, data }) {
    const out = need().decrypt(key, data);
    return { result: out, transfer: [out.buffer] };
  },
};

function need() {
  if (!novel) throw new Error("Choose a book first.");
  return novel;
}

self.onmessage = async ({ data: { id, op, args } }) => {
  try {
    await ready;
    let r = await ops[op](args);
    let transfer = [];
    if (r && typeof r === "object" && "transfer" in r) { transfer = r.transfer; r = r.result; }
    self.postMessage({ id, ok: true, result: r }, transfer);
  } catch (e) {
    self.postMessage({ id, ok: false, error: e && e.message ? e.message : String(e) });
  }
};
