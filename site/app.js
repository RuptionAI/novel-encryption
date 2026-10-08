// novelencryption.com UI. All cryptography runs in worker.js (Rust → WebAssembly).

const $ = (id) => document.getElementById(id);
const worker = new Worker(new URL("./worker.js", import.meta.url), { type: "module" });

let nextId = 1;
const pending = new Map();
worker.onmessage = ({ data }) => {
  const p = pending.get(data.id);
  if (!p) return;
  pending.delete(data.id);
  data.ok ? p.resolve(data.result) : p.reject(new Error(data.error));
};
worker.onerror = (e) => console.error("worker error", e);
function call(op, args = {}, transfer = []) {
  const id = nextId++;
  return new Promise((resolve, reject) => {
    pending.set(id, { resolve, reject });
    worker.postMessage({ id, op, args }, transfer);
  });
}

const state = {
  book: null,       // { title, slug?, fingerprint, stats }
  keyOk: false,
  sealFile: null,   // { name, bytes }
  openFile: null,
  sealOutput: null, // { bytes, armor, name }
  openOutput: null, // { bytes, name }
};

const fmt = new Intl.NumberFormat("en-US");
const esc = (s) => String(s).replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]));
const spin = (msg) => `<span class="spinner" aria-hidden="true"></span>${esc(msg)}`;

/* ---------- I. Shelf ---------- */

async function loadCatalog() {
  const box = $("catalog");
  try {
    const res = await fetch("novels/catalog.json");
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    const { novels } = await res.json();
    box.innerHTML = "";
    novels.forEach((n, i) => {
      const card = document.createElement("button");
      card.type = "button";
      card.className = "card";
      card.setAttribute("role", "listitem");
      card.setAttribute("aria-pressed", "false");
      card.dataset.slug = n.slug;
      card.innerHTML = `
        <span class="stamp">CHECKED OUT</span>
        <span class="card-call">NE ${String(i + 1).padStart(3, "0")} · PG ${esc(n.gutenberg_id)}</span>
        <span class="card-title">${esc(n.title)}</span>
        <span class="card-author">${esc(n.author)}, ${esc(n.year)}</span>
        <span class="card-meta">${fmt.format(n.key_vocabulary)} words · 128-bit key:<br>
          ~${n.narrative_words_for_128_bits}-word passage or ${n.words_for_128_bits}-word chain</span>`;
      card.addEventListener("click", () => chooseCatalog(n, card));
      box.appendChild(card);
    });
  } catch (e) {
    box.innerHTML = `<p class="status bad">Couldn't open the catalog (${esc(e.message)}). You can still bring your own book.</p>`;
  }
}

function markCard(card) {
  document.querySelectorAll(".catalog .card").forEach((c) => c.setAttribute("aria-pressed", String(c === card)));
}

async function chooseCatalog(n, card) {
  markCard(card);
  bookStatus(spin(`Fetching ${n.title} from the shelf…`));
  try {
    const res = await fetch(`novels/${n.slug}.txt`);
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    const text = await res.text();
    bookStatus(spin(`Reading all ${fmt.format(n.total_words)} words of ${n.title}…`));
    const stats = await call("load", { text, expectedFingerprint: n.fingerprint });
    setBook({ title: n.title, author: n.author, slug: n.slug, stats, entry: n });
  } catch (e) {
    markCard(null);
    bookStatus(esc(e.message), true);
    setBook(null);
  }
}

$("own-file").addEventListener("change", async (ev) => {
  const f = ev.target.files[0];
  ev.target.value = "";
  if (!f) return;
  markCard(null);
  bookStatus(spin(`Reading ${f.name}…`));
  try {
    const text = await f.text();
    const stats = await call("load", { text });
    setBook({ title: f.name, author: "your own copy", stats });
  } catch (e) {
    bookStatus(esc(e.message), true);
    setBook(null);
  }
});

function bookStatus(html, bad = false) {
  const el = $("book-status");
  el.hidden = false;
  el.classList.toggle("bad", bad);
  el.innerHTML = html;
}

function setBook(book) {
  state.book = book;
  const on = !!book;
  $("key").setAttribute("aria-disabled", String(!on));
  $("seal").setAttribute("aria-disabled", String(!on));
  $("gen-btn").disabled = !on;
  if (book) {
    const s = book.stats;
    bookStatus(
      `<strong>${esc(book.title)}</strong> is on your desk${book.author ? ` (${esc(book.author)})` : ""}.
       ${fmt.format(s.totalWords)} words, ${fmt.format(s.keyVocabulary)} usable for keys.
       A 128-bit key is ${book.entry ? `a ~${book.entry.narrative_words_for_128_bits}-word passage or ` : ""}a ~${s.wordsFor128Bits}-word chain.<br>
       Fingerprint <code class="fp">${esc(s.fingerprint)}</code>`
    );
  }
  $("chain").innerHTML = "";
  checkKeyNow();
  updateButtons();
}

/* ---------- II. Key ---------- */

const styleMode = () => document.querySelector('input[name="style"]:checked').value;
const SLIDER = { narrative: { min: 20, max: 300, value: 80 }, chain: { min: 4, max: 60, value: 16 } };
function applySlider() {
  const s = SLIDER[styleMode()], el = $("depth-words");
  Object.assign(el, { min: s.min, max: s.max });
  el.value = s.value;
  $("depth-words-out").textContent = `${s.value} words`;
}
document.querySelectorAll('input[name="style"]').forEach((r) => r.addEventListener("change", applySlider));
applySlider();
const depthRadios = document.querySelectorAll('input[name="depth"]');
depthRadios.forEach((r) => r.addEventListener("change", () => {
  document.querySelector(".words-ctl").hidden = depthMode() !== "words";
}));
const depthMode = () => document.querySelector('input[name="depth"]:checked').value;
$("depth-words").addEventListener("input", (e) => { $("depth-words-out").textContent = `${e.target.value} words`; });

$("gen-btn").addEventListener("click", async () => {
  const words = depthMode() === "words" ? Number($("depth-words").value) : 0;
  const style = styleMode();
  const args = { style, bits: depthMode() === "long" ? 256 : 128, words, allowWeak: false };
  try {
    let k;
    try {
      k = await call("generateKey", args);
    } catch (e) {
      if (!/too weak/.test(e.message)) throw e;
      if (!confirm(`${words} words from this book falls short of the recommended 128 bits. Use it anyway?`)) return;
      k = await call("generateKey", { ...args, allowWeak: true });
    }
    if (k.style === "narrative") renderPassage(k.phrase); else renderChain(k.words);
    $("key-input").value = k.phrase;
    checkKeyNow();
  } catch (e) {
    showKeyCheck(e.message, false);
  }
});

function renderPassage(text) {
  const box = $("chain");
  box.innerHTML = "";
  const q = document.createElement("blockquote");
  q.className = "passage";
  q.textContent = text;
  box.appendChild(q);
}

function renderChain(words) {
  const box = $("chain");
  box.innerHTML = "";
  words.forEach((w, i) => {
    if (i > 0) {
      const a = document.createElement("span");
      a.className = "arrow";
      a.textContent = "→";
      a.setAttribute("aria-hidden", "true");
      a.style.animationDelay = `${i * 45 - 20}ms`;
      box.appendChild(a);
    }
    const span = document.createElement("span");
    span.className = "w";
    span.style.animationDelay = `${i * 45}ms`;
    // Highlight the letters that link the chain: first letter (except the
    // first word) and last letter (except the last word).
    const first = i > 0, last = i < words.length - 1;
    if (w.length === 1) {
      span.innerHTML = first || last ? `<b>${esc(w)}</b>` : esc(w);
    } else {
      span.innerHTML =
        (first ? `<b>${esc(w[0])}</b>` : esc(w[0])) +
        esc(w.slice(1, -1)) +
        (last ? `<b>${esc(w.at(-1))}</b>` : esc(w.at(-1)));
    }
    box.appendChild(span);
  });
}

let checkTimer = 0;
$("key-input").addEventListener("input", () => {
  clearTimeout(checkTimer);
  checkTimer = setTimeout(checkKeyNow, 200);
});

async function checkKeyNow() {
  const key = $("key-input").value.trim();
  const hasKey = key.length > 0;
  $("key-copy").disabled = !hasKey;
  $("key-save").disabled = !hasKey;
  if (!hasKey) { showKeyCheck("", null); setMeter(0); state.keyOk = false; updateButtons(); return; }
  if (!state.book) { showKeyCheck("Choose a book to check this key against.", null); state.keyOk = false; updateButtons(); return; }
  try {
    const { style, bits, words } = await call("checkKey", { key });
    const what = style === "narrative" ? "passage" : "chain";
    const strong = bits >= 128;
    showKeyCheck(
      strong
        ? `✓ The ${what} holds: ${words} words, ${bits.toFixed(1)} bits${bits >= 256 ? " (long-term, quantum-resistant)" : ""}.`
        : `✓ The ${what} holds, but at ${bits.toFixed(1)} bits it's below the recommended 128.`,
      true
    );
    setMeter(bits);
    state.keyOk = true;
  } catch (e) {
    showKeyCheck(`✗ ${e.message}`, false);
    setMeter(0);
    state.keyOk = false;
  }
  updateButtons();
}

function showKeyCheck(msg, ok) {
  const el = $("key-check");
  el.textContent = msg;
  el.className = "key-check" + (ok === true ? " ok" : ok === false ? " bad" : "");
}

function setMeter(bits) {
  const el = $("meter-fill");
  el.style.width = `${Math.min(100, (bits / 256) * 100)}%`;
  el.className = bits >= 128 ? "strong" : bits > 0 ? "weak" : "";
}

$("key-copy").addEventListener("click", () => copy($("key-input").value.trim(), $("key-copy")));
$("key-save").addEventListener("click", () => {
  const book = state.book ? `${state.book.title}${state.book.stats ? `\nBook fingerprint: ${state.book.stats.fingerprint}` : ""}` : "";
  const body = `Novel Encryption key\nBook: ${book}\n\n${$("key-input").value.trim()}\n`;
  download(new TextEncoder().encode(body), "novel-encryption-key.txt", "text/plain");
});

/* ---------- III. Seal & open ---------- */

["seal", "open"].forEach((name) => {
  $(`tab-${name}`).addEventListener("click", () => {
    ["seal", "open"].forEach((n) => {
      $(`tab-${n}`).setAttribute("aria-selected", String(n === name));
      $(`pane-${n}`).hidden = n !== name;
    });
  });
});

function updateButtons() {
  const ready = !!state.book && state.keyOk;
  $("seal-btn").disabled = !ready || (!state.sealFile && !$("seal-text").value);
  $("open-btn").disabled = !ready || (!state.openFile && !$("open-text").value.trim());
}
$("seal-text").addEventListener("input", updateButtons);
$("open-text").addEventListener("input", updateButtons);

function wireFile(prefix) {
  $(`${prefix}-file`).addEventListener("change", async (ev) => {
    const f = ev.target.files[0];
    ev.target.value = "";
    if (!f) return;
    state[`${prefix}File`] = { name: f.name, bytes: new Uint8Array(await f.arrayBuffer()) };
    $(`${prefix}-file-name`).textContent = `${f.name} (${fmt.format(f.size)} bytes)`;
    $(`${prefix}-file-clear`).hidden = false;
    $(`${prefix}-text`).disabled = true;
    updateButtons();
  });
  $(`${prefix}-file-clear`).addEventListener("click", () => {
    state[`${prefix}File`] = null;
    $(`${prefix}-file-name`).textContent = "";
    $(`${prefix}-file-clear`).hidden = true;
    $(`${prefix}-text`).disabled = false;
    updateButtons();
  });
}
wireFile("seal");
wireFile("open");

function setStatus(id, html, cls = "") {
  const el = $(id);
  el.className = "status" + (cls ? ` ${cls}` : "");
  el.innerHTML = html;
}

$("seal-btn").addEventListener("click", async () => {
  const armor = document.querySelector('input[name="armor"]:checked').value;
  const src = state.sealFile;
  const data = src ? src.bytes.slice() : new TextEncoder().encode($("seal-text").value);
  $("seal-btn").disabled = true;
  $("seal-out-wrap").hidden = true;
  setStatus("seal-status", spin("Binding your key to the book and sealing (Argon2id, 64 MiB)…"));
  try {
    const out = await call("encrypt", { key: $("key-input").value, data, armor, title: state.book.title }, [data.buffer]);
    const base = src ? src.name : "message";
    state.sealOutput = { bytes: out, armor, name: armor === "binary" ? `${base}.novl` : `${base}.${armor === "story" ? "chapter" : armor === "novel" ? "novel" : "sealed"}.txt` };
    if (armor === "binary") {
      setStatus("seal-status", `Sealed: ${fmt.format(out.length)} bytes. Your download should begin now.`, "ok");
      download(out, state.sealOutput.name, "application/octet-stream");
    } else {
      $("seal-out").value = new TextDecoder().decode(out);
      $("seal-out-wrap").hidden = false;
      setStatus("seal-status", armor === "story"
        ? `Your message is now a lost chapter of ${esc(state.book.title)}: ${fmt.format(new TextDecoder().decode(out).split(/\s+/).length)} words. Only the same book and key can read what it hides.`
        : armor === "novel"
        ? `Sealed in the words of ${esc(state.book.title)}. To open it, you need the same book and key.`
        : `Sealed: ${fmt.format(out.length)} characters.`, "ok");
    }
  } catch (e) {
    setStatus("seal-status", esc(e.message), "bad");
  } finally {
    updateButtons();
  }
});

$("seal-copy").addEventListener("click", () => copy($("seal-out").value, $("seal-copy")));
$("seal-dl").addEventListener("click", () => {
  const o = state.sealOutput;
  if (o) download(o.bytes, o.name, o.armor === "binary" ? "application/octet-stream" : "text/plain");
});

$("open-btn").addEventListener("click", async () => {
  const src = state.openFile;
  const data = src ? src.bytes.slice() : new TextEncoder().encode($("open-text").value);
  $("open-btn").disabled = true;
  $("open-out-wrap").hidden = true;
  setStatus("open-status", spin("Checking the key against the book and opening…"));
  try {
    const out = await call("decrypt", { key: $("key-input").value, data }, [data.buffer]);
    let name = "opened.txt";
    if (src) name = src.name.replace(/\.novl$/i, "") || "opened.bin";
    state.openOutput = { bytes: out, name };
    let text = null;
    try { text = new TextDecoder("utf-8", { fatal: true }).decode(out); } catch { /* binary */ }
    if (text !== null && !/[\u0000-\u0008\u000E-\u001F]/.test(text)) {
      $("open-out").value = text;
      $("open-out-wrap").hidden = false;
      $("open-copy").hidden = false;
      setStatus("open-status", "Opened.", "ok");
    } else {
      $("open-out").value = `(${fmt.format(out.length)} bytes of binary data; use Download)`;
      $("open-out-wrap").hidden = false;
      $("open-copy").hidden = true;
      setStatus("open-status", "Opened a file.", "ok");
    }
  } catch (e) {
    setStatus("open-status", esc(e.message), "bad");
  } finally {
    updateButtons();
  }
});

$("open-copy").addEventListener("click", () => copy($("open-out").value, $("open-copy")));
$("open-dl").addEventListener("click", () => {
  const o = state.openOutput;
  if (o) download(o.bytes, o.name, "application/octet-stream");
});

/* ---------- helpers ---------- */

async function copy(text, btn) {
  try {
    await navigator.clipboard.writeText(text);
    const prev = btn.textContent;
    btn.textContent = "Copied";
    setTimeout(() => (btn.textContent = prev), 1400);
  } catch {
    alert("Copy failed. Select the text and copy it manually.");
  }
}

function download(bytes, name, type) {
  const url = URL.createObjectURL(new Blob([bytes], { type }));
  const a = document.createElement("a");
  a.href = url;
  a.download = name;
  document.body.appendChild(a);
  a.click();
  a.remove();
  setTimeout(() => URL.revokeObjectURL(url), 10_000);
}

loadCatalog();
