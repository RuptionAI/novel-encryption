import sys
from pathlib import Path
from playwright.sync_api import sync_playwright
here = Path(__file__).resolve().parent
with sync_playwright() as p:
    b = p.chromium.launch(channel="chrome", headless=True)
    for scale, name in [(1, "x-header-ruptionai.png"), (2, "x-header-ruptionai@2x.png")]:
        pg = b.new_page(viewport={"width": 1500, "height": 500}, device_scale_factor=scale)
        pg.goto((here / "x-header-ruptionai.html").as_uri()); pg.evaluate("document.fonts.ready"); pg.wait_for_timeout(500)
        pg.screenshot(path=str(here / name))
    # Safe-zone preview: avatar circle (desktop) and the phone crop band.
    pg = b.new_page(viewport={"width": 1500, "height": 500})
    pg.goto((here / "x-header-ruptionai.html").as_uri()); pg.evaluate("document.fonts.ready"); pg.wait_for_timeout(300)
    pg.evaluate("""() => {
      const d = document.createElement('div');
      d.style.cssText = 'position:absolute;left:58px;top:330px;width:270px;height:270px;border-radius:50%;background:rgba(255,255,255,.85);outline:4px solid #f33';
      document.body.appendChild(d);
      for (const y of [0, 440]) { const s = document.createElement('div'); s.style.cssText = `position:absolute;left:0;right:0;top:${y}px;height:60px;background:rgba(255,0,0,.25)`; document.body.appendChild(s); }
    }""")
    pg.screenshot(path="/Volumes/SSD_2/HottubBuildStorage/jeffpittman/user-caches/claude-tmp/claude-501/-Users-jeffpittman-Developer/9fb58d46-2f5d-4e55-a07c-bbc384950407/scratchpad/header_safezone.png")
    b.close()
