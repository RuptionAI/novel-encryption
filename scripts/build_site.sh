#!/usr/bin/env bash
# Build the static site for novelencryption.com into dist/.
#
#   scripts/build_site.sh            # build
#   python3 -m http.server -d dist   # serve locally
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DIST="$ROOT/dist"
WASM="$ROOT/target/wasm32-unknown-unknown/release/novel_encryption_wasm.wasm"

cd "$ROOT"
cargo build --release --target wasm32-unknown-unknown -p novel-encryption-wasm

rm -rf "$DIST"
mkdir -p "$DIST/pkg" "$DIST/novels"
wasm-bindgen --target web --out-dir "$DIST/pkg" "$WASM"

if command -v wasm-opt >/dev/null 2>&1; then
  wasm-opt -O3 --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext \
    "$DIST/pkg/novel_encryption_wasm_bg.wasm" -o "$DIST/pkg/novel_encryption_wasm_bg.wasm"
fi

cp -R "$ROOT/site/." "$DIST/"
cp "$ROOT/catalog/catalog.json" "$DIST/novels/catalog.json"
cp "$ROOT"/catalog/texts/*.txt "$DIST/novels/"
cp "$ROOT/docs/WHITEPAPER.md" "$DIST/WHITEPAPER.md"
# Render the white paper to HTML now, so the page needs no script to show it.
cargo run -q --release -p novel-encryption --example render_whitepaper -- \
  "$ROOT/site/whitepaper.html" "$DIST/whitepaper.html"

echo "built $DIST ($(du -sh "$DIST" | cut -f1))"
