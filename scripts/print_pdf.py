#!/usr/bin/env python3
"""Print the rendered white paper (HTML) to a PDF with Chrome, in two passes:
the first finds each section's page from the PDF outline, the second fills
those page numbers into the table of contents.

Usage: print_pdf.py <rendered.html> <out.pdf>   (needs Playwright + Google Chrome)
"""
import re
import sys
from pathlib import Path

from playwright.sync_api import sync_playwright
from pypdf import PdfReader


def slug(text: str) -> str:
    s = re.sub(r"[^a-z0-9]+", "-", text.lower())
    return s.strip("-")


def outline_pages(pdf: Path) -> dict:
    reader = PdfReader(str(pdf))
    pages = {}

    def walk(items):
        for it in items:
            if isinstance(it, list):
                walk(it)
            else:
                pages.setdefault(slug(it.title), reader.get_destination_page_number(it) + 1)

    walk(reader.outline)
    return pages


def main() -> int:
    html_path, out = Path(sys.argv[1]).resolve(), Path(sys.argv[2]).resolve()
    html = html_path.read_text()
    with sync_playwright() as p:
        browser = p.chromium.launch(channel="chrome", headless=True)
        page = browser.new_page()

        def render(source: str) -> None:
            html_path.write_text(source)
            page.goto(html_path.as_uri())
            page.evaluate("document.fonts.ready")
            page.pdf(path=str(out), prefer_css_page_size=True, print_background=True, outline=True, tagged=True)

        render(html)
        pages = outline_pages(out)
        filled = re.sub(
            r'<span class="toc-page" data-for="([^"]+)"></span>',
            lambda m: f'<span class="toc-page" data-for="{m.group(1)}">{pages.get(m.group(1), "")}</span>',
            html,
        )
        missing = sorted(set(re.findall(r'data-for="([^"]+)"', html)) - set(pages))
        if missing:
            print("warning: no page found for", ", ".join(missing), file=sys.stderr)
        render(filled)
        browser.close()
    print(f"wrote {out} ({out.stat().st_size // 1024} KiB, {len(PdfReader(str(out)).pages)} pages)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
