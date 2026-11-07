#!/usr/bin/env python3
"""Draws Matty's app icon, "Overlap" (#65): a lowercase m made of two solid
arches, two voices, in blue and pink that mix where they overlap.

Writes the SVG masters here and the macOS 26 icon source,
packaging/macos/AppIcon.icon (Icon Composer's format: one full-bleed
layer per shape; the system draws the tile, its glass and the dark and
tinted looks, and scripts/macos-icon compiles it). Then renders PNGs with Playwright's Chromium
(`node`, `require('playwright')`) and packs AppIcon.icns, matty.ico, the
Linux icon, the web favicon, the tray icon and the sign-in mark with
Pillow:

    python3 packaging/icons/make.py
"""
import json
import subprocess
from pathlib import Path

from PIL import Image

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent

# The mark on a 100-unit grid: two arches of radius 22 on a baseline at 72,
# overlapping by 20 units. OVERLAP is their intersection, drawn as its own
# shape so renderers without blend modes agree.
LEFT = "M16 72V50a22 22 0 0 1 44 0v22z"
RIGHT = "M40 72V50a22 22 0 0 1 44 0v22z"
OVERLAP = "M40 72V50A22 22 0 0 1 50 31.56A22 22 0 0 1 60 50V72z"
# The arches without their overlap, so the three Icon Composer layers do not
# depend on their stacking order.
LEFT_ONLY = "M16 72V50a22 22 0 0 1 34-18.44A22 22 0 0 0 40 50v22z"
RIGHT_ONLY = "M60 72V50A22 22 0 0 0 50 31.56A22 22 0 0 1 84 50v22z"

# Light: multiply of blue and pink. Dark: screen of the lighter pair.
LIGHT = ("#3D5AFE", "#FF4F7B", "#3D1C7A")
DARK = ("#5C76FF", "#FF5C86", "#FF96FF")


def glyph(colors, dy=-2):
    blue, pink, deep = colors
    return (f'<g transform="translate(0 {dy})"><path d="{LEFT}" fill="{blue}"/>'
            f'<path d="{RIGHT}" fill="{pink}"/><path d="{OVERLAP}" fill="{deep}"/></g>')


def tile(top, bottom, edge, colors):
    """Full-bleed rounded square, for Linux, Windows and the web."""
    return f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
  <defs><linearGradient id="t" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="{top}"/><stop offset="1" stop-color="{bottom}"/></linearGradient></defs>
  <rect width="100" height="100" rx="22.5" fill="url(#t)"/>
  <rect x=".5" y=".5" width="99" height="99" rx="22" fill="none" stroke="{edge}"/>
  {glyph(colors)}
</svg>
'''


def tile_macos(top, bottom, edge, colors):
    """Apple's grid: an 824-point tile centred on a 1024 canvas, with shadow."""
    return f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024">
  <defs>
    <linearGradient id="t" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="{top}"/><stop offset="1" stop-color="{bottom}"/></linearGradient>
    <filter id="s" x="-20%" y="-20%" width="140%" height="140%"><feDropShadow dx="0" dy="12" stdDeviation="14" flood-color="#000" flood-opacity=".28"/></filter>
  </defs>
  <rect x="100" y="100" width="824" height="824" rx="185" fill="url(#t)" filter="url(#s)"/>
  <rect x="102" y="102" width="820" height="820" rx="183" fill="none" stroke="{edge}" stroke-width="4"/>
  <g transform="translate(100 100) scale(8.24)">{glyph(colors)}</g>
</svg>
'''


SVGS = {
    "matty-icon.svg": tile("#FFFFFF", "#EDEDF0", "rgba(0,0,0,.08)", LIGHT),
    "matty-icon-dark.svg": tile("#2A2B30", "#131417", "rgba(255,255,255,.08)", DARK),
    "matty-icon-macos.svg": tile_macos("#FFFFFF", "#EDEDF0", "rgba(0,0,0,.06)", LIGHT),
    # The mark alone, for lockups, the tray and the favicon.
    "matty-mark.svg": f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="12 14 76 76">\n  {glyph(LIGHT, 2)}\n</svg>\n',
    "matty-mark-mono.svg": f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="12 14 76 76">\n  <g transform="translate(0 2)"><path d="{LEFT}"/><path d="{RIGHT}" fill-opacity=".55"/></g>\n</svg>\n',
}

def srgb(hex_color):
    r, g, b = (int(hex_color[i:i + 2], 16) / 255 for i in (1, 3, 5))
    return f"srgb:{r:.5f},{g:.5f},{b:.5f},1.00000"


def icon_composer():
    """AppIcon.icon: the mark on a 1024 canvas, opaque, with no tile of its
    own. Each arch is a layer whose fill Icon Composer sets per appearance,
    so the shapes are drawn in plain black. The arches leave a gap for the
    overlap, which gets a hairline stroke so no seam shows."""
    out = ROOT / "packaging/macos/AppIcon.icon"
    assets = out / "Assets"
    assets.mkdir(parents=True, exist_ok=True)
    layers = []
    for name, path, light, dark, stroke in (
        ("overlap", OVERLAP, LIGHT[2], DARK[2], ' stroke="#000" stroke-width=".4"'),
        ("right", RIGHT_ONLY, LIGHT[1], DARK[1], ""),
        ("left", LEFT_ONLY, LIGHT[0], DARK[0], ""),
    ):
        (assets / f"{name}.svg").write_text(
            '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024">'
            f'<g transform="translate(0 -20.48) scale(10.24)"><path d="{path}"{stroke}/></g></svg>\n')
        layers.append({
            "fill": {"solid": srgb(light)},
            "fill-specializations": [{"appearance": "dark", "value": {"solid": srgb(dark)}}],
            "glass": False,
            "hidden": False,
            "image-name": f"{name}.svg",
            "name": name,
        })
    icon = {
        "fill": {"solid": srgb("#FFFFFF")},
        "fill-specializations": [{"appearance": "dark", "value": {"solid": srgb("#1B1C20")}}],
        "groups": [{
            "layers": layers,
            "shadow": {"kind": "neutral", "opacity": 0.5},
            "translucency": {"enabled": False, "value": 0.5},
        }],
        "supported-platforms": {"squares": ["macOS"]},
    }
    (out / "icon.json").write_text(json.dumps(icon, indent=2) + "\n")


RENDER = r"""
const {chromium} = require('playwright');
const jobs = JSON.parse(process.argv[process.argv.length - 1]);
(async () => {
  const browser = await chromium.launch();
  const page = await browser.newPage();
  for (const [svg, size, out] of jobs) {
    await page.setViewportSize({width: size, height: size});
    await page.setContent(`<style>html,body{margin:0;background:transparent}img{display:block;width:${size}px;height:${size}px}</style><img src="data:image/svg+xml;base64,${require('fs').readFileSync(svg).toString('base64')}">`);
    await page.waitForFunction(() => document.images[0].complete);
    await page.screenshot({path: out, omitBackground: true});
  }
  await browser.close();
})();
"""


def main():
    for name, svg in SVGS.items():
        (HERE / name).write_text(svg)
    icon_composer()
    png = HERE / "png"
    png.mkdir(exist_ok=True)
    (ROOT / "crates/matty-app/assets/brand").mkdir(exist_ok=True)
    jobs = []
    for size in (16, 32, 48, 64, 128, 256, 512, 1024):
        jobs.append([str(HERE / "matty-icon.svg"), size, str(png / f"matty-{size}.png")])
        jobs.append([str(HERE / "matty-icon-macos.svg"), size, str(png / f"macos-{size}.png")])
    jobs.append([str(HERE / "matty-mark.svg"), 64, str(png / "mark-64.png")])
    jobs.append([str(HERE / "matty-mark.svg"), 128, str(ROOT / "crates/matty-app/assets/brand/mark-128.png")])
    subprocess.run(["node", "-e", RENDER, json.dumps(jobs)], check=True)

    # The fallback for macOS 13 to 15, where the icon still carries its own
    # tile. scripts/macos-icon replaces it with actool's own fallback when
    # Xcode 26 is there.
    macos = Image.open(png / "macos-1024.png")
    macos.save(ROOT / "packaging/macos/AppIcon.icns",
               sizes=[(16, 16), (32, 32), (64, 64), (128, 128), (256, 256), (512, 512), (1024, 1024)])
    Image.open(png / "matty-256.png").save(
        HERE / "matty.ico", sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)])
    (ROOT / "packaging/linux/dev.arya.matty.svg").write_text(SVGS["matty-icon.svg"])
    (ROOT / "crates/matty-web/web/favicon.svg").write_text(SVGS["matty-mark.svg"])
    tray = Image.open(png / "mark-64.png").convert("RGBA")
    (ROOT / "crates/matty-app/assets/tray/icon-64.rgba").write_bytes(tray.tobytes())
    print("wrote icons")


if __name__ == "__main__":
    main()
