#!/usr/bin/env bash
# Build docs/novel-encryption-whitepaper.pdf from docs/WHITEPAPER.md.
# Needs Google Chrome and Python with playwright + pypdf. The PDF is
# committed, so the site build does not need these tools.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
WORK="$ROOT/target/whitepaper-print"
rm -rf "$WORK" && mkdir -p "$WORK"
cp -R "$ROOT/site/fonts" "$WORK/fonts"
cd "$ROOT"
cargo run -q --release -p novel-encryption --example render_whitepaper -- --print \
  "$ROOT/docs/print/whitepaper.html" "$WORK/whitepaper.html"
python3 -I "$ROOT/scripts/print_pdf.py" "$WORK/whitepaper.html" "$ROOT/docs/novel-encryption-whitepaper.pdf"
