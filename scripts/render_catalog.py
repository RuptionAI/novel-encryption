#!/usr/bin/env python3
"""Write the catalog cards into dist/index.html at build time, so the page
paints its final layout at once instead of shifting when catalog.json loads.

Usage: render_catalog.py <catalog.json> <index.html>   (edits index.html in place)
"""
import html
import json
import sys
from pathlib import Path

MARKER = "<!-- CATALOG -->"


def card(i: int, n: dict) -> str:
    e = lambda v: html.escape(str(v), quote=True)
    return f"""<button type="button" class="card" role="listitem" aria-pressed="false" data-slug="{e(n['slug'])}">
          <span class="stamp">CHECKED OUT</span>
          <span class="card-call">NE {i:03d} · PG {e(n['gutenberg_id'])}</span>
          <span class="card-title">{e(n['title'])}</span>
          <span class="card-author">{e(n['author'])}, {e(n['year'])}</span>
          <span class="card-meta">{n['key_vocabulary']:,} words · 128-bit key:<br>
            ~{n['narrative_words_for_128_bits']}-word passage or {n['words_for_128_bits']}-word chain</span>
        </button>"""


def main() -> int:
    novels = json.loads(Path(sys.argv[1]).read_text())["novels"]
    page = Path(sys.argv[2])
    text = page.read_text()
    assert MARKER in text, f"{page} lacks {MARKER}"
    cards = "\n        ".join(card(i, n) for i, n in enumerate(novels, 1))
    page.write_text(text.replace(MARKER, cards))
    return 0


if __name__ == "__main__":
    sys.exit(main())
