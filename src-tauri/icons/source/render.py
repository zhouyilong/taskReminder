"""Render app icons from the SVG masters.

Usage (from src-tauri/icons): python3 source/render.py
Requires: pip install cairosvg pillow
"""
import io
import struct
from pathlib import Path

import cairosvg
from PIL import Image

HERE = Path(__file__).resolve().parent
OUT = HERE.parent
FULL = HERE / "icon.svg"
SMALL = HERE / "icon-small.svg"  # simplified glyph for <= 32px


def render(size: int) -> bytes:
    src = SMALL if size <= 32 else FULL
    png = cairosvg.svg2png(url=str(src), output_width=size, output_height=size)
    # Re-encode through Pillow for a compact, consistent RGBA PNG.
    buf = io.BytesIO()
    Image.open(io.BytesIO(png)).convert("RGBA").save(buf, "PNG", optimize=True)
    return buf.getvalue()


def write_ico(path: Path, sizes: list[int]) -> None:
    # Tauri uses the first entry as the default window/tray icon, so 32px goes first.
    images = [render(s) for s in sizes]
    header = struct.pack("<HHH", 0, 1, len(images))
    offset = len(header) + 16 * len(images)
    entries, data = b"", b""
    for size, png in zip(sizes, images):
        dim = 0 if size >= 256 else size
        entries += struct.pack("<BBBBHHII", dim, dim, 0, 0, 1, 32, len(png), offset)
        offset += len(png)
        data += png
    path.write_bytes(header + entries + data)


for name, size in {
    "32x32.png": 32,
    "128x128.png": 128,
    "128x128@2x.png": 256,
    "icon.png": 512,
}.items():
    (OUT / name).write_bytes(render(size))

write_ico(OUT / "icon.ico", [32, 16, 24, 48, 64, 128, 256])
