#!/usr/bin/env python3
"""Render NYRA editable SVG TUI concepts into PNG previews.

These are raster exports of the repository's SVG wireframes, NOT exact original
chat-generated PNG screenshots. Deterministic sizes and names for GitHub README.
"""
from pathlib import Path
from io import BytesIO
import cairosvg
from PIL import Image, ImageOps, ImageDraw

ROOT = Path(__file__).resolve().parents[1]
FOLDER = ROOT / "assets" / "mockups"
THEMES = ("cyber-cyan", "neo-tokyo", "acid-grid", "crimson-noir")

def main():
    imgs = []
    for theme in THEMES:
        src = FOLDER / f"nyra-{theme}.svg"
        dst = FOLDER / f"nyra-{theme}.png"
        data = cairosvg.svg2png(url=str(src), output_width=1440, output_height=850)
        with Image.open(BytesIO(data)) as im:
            rgb = im.convert("RGB")
            rgb.save(dst, format="PNG", optimize=True)
            imgs.append(rgb.copy())
        print(f"Rendered: {dst.relative_to(ROOT)} ({dst.stat().st_size} bytes)")

    # Wide 2x2 overview for README / decisions.
    thumbs = []
    for im in imgs:
        copy = im.resize((960, 567), Image.Resampling.LANCZOS)
        thumbs.append(copy)
    board = Image.new("RGB", (1920 + 36, 1134 + 36), "#0C1019")
    coords = [(12, 12), (984, 12), (12, 591), (984, 591)]
    for img, (x, y) in zip(thumbs, coords):
        board.paste(img, (x, y))
    out = FOLDER / "nyra-four-themes.png"
    board.save(out, format="PNG", optimize=True)
    print(f"Rendered: {out.relative_to(ROOT)} ({out.stat().st_size} bytes)")

if __name__ == "__main__":
    main()
