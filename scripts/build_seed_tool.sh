#!/usr/bin/env bash
# Build tools/novel-seed-offline.html: the wallet-backup converter as one
# self-contained file (WebAssembly embedded, no network). Not served on the site.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
WORK="$ROOT/target/seed-tool"
rm -rf "$WORK" && mkdir -p "$WORK"
cd "$ROOT"
cargo build --release --target wasm32-unknown-unknown -p novel-encryption-wasm
wasm-bindgen --target no-modules --out-dir "$WORK" "$ROOT/target/wasm32-unknown-unknown/release/novel_encryption_wasm.wasm"
if command -v wasm-opt >/dev/null 2>&1; then
  wasm-opt -O3 --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext \
    "$WORK/novel_encryption_wasm_bg.wasm" -o "$WORK/novel_encryption_wasm_bg.wasm"
fi
python3 -I - "$ROOT" "$WORK" <<'PY'
import base64, json, sys
from pathlib import Path
root, work = Path(sys.argv[1]), Path(sys.argv[2])
page = (root / "tools/seed/page.html").read_text()
glue = (work / "novel_encryption_wasm.js").read_text()
wasm = base64.b64encode((work / "novel_encryption_wasm_bg.wasm").read_bytes()).decode()
catalog = [{k: n[k] for k in ("title", "author", "fingerprint")} for n in json.loads((root / "catalog/catalog.json").read_text())["novels"]]
for marker in ("/*GLUE*/", "/*CATALOG*/", "__WASM_BASE64__"):
    assert page.count(marker) == 1, marker
page = page.replace("/*GLUE*/", glue).replace("/*CATALOG*/", json.dumps(catalog)).replace("__WASM_BASE64__", wasm)
out = root / "tools/novel-seed-offline.html"
out.write_text(page)
print(f"wrote {out.relative_to(root)} ({out.stat().st_size // 1024} KiB)")
PY
shasum -a 256 "$ROOT/tools/novel-seed-offline.html"
