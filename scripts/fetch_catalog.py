#!/usr/bin/env python3
"""Download the public-domain novel catalog from Project Gutenberg.

Raw downloads (with Gutenberg's license header/footer) are cached outside the
repo; the stripped, public-domain body of each work is written to
catalog/texts/<slug>.txt. Those stripped texts are the pinned source of truth:
keys are bound to a novel's word fingerprint, so a text must never change once
published. Run `novelenc catalog` afterwards to (re)compute catalog.json.

Usage: scripts/fetch_catalog.py [--raw-dir DIR] [--force]
"""

import argparse
import json
import os
import re
import sys
import time
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SOURCES = ROOT / "catalog" / "sources.json"
TEXTS = ROOT / "catalog" / "texts"
DEFAULT_RAW = Path(
    os.environ.get(
        "NOVELENC_RAW_DIR",
        "/Volumes/SSD_2/HottubBuildStorage/jeffpittman/user-data/novelencryption/gutenberg-raw",
    )
)
URL = "https://www.gutenberg.org/cache/epub/{id}/pg{id}.txt"

START = re.compile(r"^\*\*\* ?START OF (THE|THIS) PROJECT GUTENBERG EBOOK.*$", re.M | re.I)
END = re.compile(r"^\*\*\* ?END OF (THE|THIS) PROJECT GUTENBERG EBOOK.*$", re.M | re.I)


def strip_gutenberg(raw: str) -> str:
    start = START.search(raw)
    end = END.search(raw)
    if not start or not end:
        raise ValueError("Gutenberg START/END markers not found")
    body = raw[start.end() : end.start()]
    body = body.replace("\r\n", "\n").strip("\n")
    return body + "\n"


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--raw-dir", type=Path, default=DEFAULT_RAW)
    ap.add_argument("--force", action="store_true", help="overwrite existing stripped texts")
    args = ap.parse_args()

    sources = json.loads(SOURCES.read_text())
    args.raw_dir.mkdir(parents=True, exist_ok=True)
    TEXTS.mkdir(parents=True, exist_ok=True)

    for src in sources:
        out = TEXTS / f"{src['slug']}.txt"
        if out.exists() and not args.force:
            print(f"keep  {out.name}")
            continue
        raw_path = args.raw_dir / f"pg{src['gutenberg_id']}.txt"
        if not raw_path.exists():
            url = URL.format(id=src["gutenberg_id"])
            print(f"fetch {url}")
            req = urllib.request.Request(url, headers={"User-Agent": "NovelEncryption catalog builder"})
            with urllib.request.urlopen(req, timeout=60) as resp:
                raw_path.write_bytes(resp.read())
            time.sleep(1)  # be polite to gutenberg.org
        raw = raw_path.read_text(encoding="utf-8-sig")
        head = raw[:4000].lower()
        if src["title"].lower().split(";")[0][:12] not in head:
            print(f"WARN  title {src['title']!r} not found in header of pg{src['gutenberg_id']}", file=sys.stderr)
        out.write_text(strip_gutenberg(raw), encoding="utf-8")
        print(f"wrote {out.relative_to(ROOT)} ({out.stat().st_size // 1024} KiB)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
