"""Render the original Omen Hub++ application icon.

Build-time tool only: `python scripts/generate-icon.py` (Pillow) rewrites
crates/app/assets/omen-hub-pp.ico and the documentation preview
docs/assets/app-icon.png. Small sizes are drawn separately with
fewer details instead of being downscaled from the 256-pixel image.
Colors follow DESIGN.md: flat panel surface, mint accent, light foreground.
"""

import io
import math
import struct
import sys
from pathlib import Path

from PIL import Image, ImageDraw

SIZES = (16, 20, 24, 32, 40, 48, 64, 96, 128, 256)
SUPERSAMPLE = 8
PANEL = (0x19, 0x1D, 0x22)
RIM = (0x3A, 0x45, 0x50)
TRACK = (0x2B, 0x3B, 0x38)
ACCENT = (0x68, 0xDB, 0xBF)
ACCENT_LIGHT = (0x9C, 0xF0, 0xDA)
FOREGROUND = (0xED, 0xF2, 0xF7)
# Gauge opening at the bottom; PIL angles run clockwise from three o'clock.
ARC_START, ARC_END, VALUE_END = 140.0, 400.0, 340.0
NEEDLE_ANGLE = VALUE_END


def polar(cx, cy, radius, degrees):
    angle = math.radians(degrees)
    return cx + radius * math.cos(angle), cy + radius * math.sin(angle)


def round_line(draw, start, end, width, color):
    draw.line([start, end], fill=color, width=round(width))
    for x, y in (start, end):
        draw.ellipse([x - width / 2, y - width / 2, x + width / 2, y + width / 2], fill=color)


def arc(draw, cx, cy, radius, width, start, end, color, caps=True):
    box = [cx - radius - width / 2, cy - radius - width / 2,
           cx + radius + width / 2, cy + radius + width / 2]
    draw.arc(box, start, end, fill=color, width=round(width))
    if caps:
        for angle in (start, end):
            x, y = polar(cx, cy, radius, angle)
            draw.ellipse([x - width / 2, y - width / 2, x + width / 2, y + width / 2], fill=color)


def hub_label(draw, unit, color):
    """'Hub' drawn as rounded strokes in the 256-unit grid, centred below the gauge."""
    width = 11 * unit
    top, middle, x_top, base = 176 * unit, 198 * unit, 190 * unit, 222 * unit
    x = 71 * unit
    # H
    round_line(draw, (x, top), (x, base), width, color)
    round_line(draw, (x + 30 * unit, top), (x + 30 * unit, base), width, color)
    round_line(draw, (x, middle), (x + 30 * unit, middle), width, color)
    # u
    x += 44 * unit
    bowl = 13 * unit
    round_line(draw, (x, x_top), (x, base - bowl), width, color)
    arc(draw, x + bowl, base - bowl, bowl, width, 0, 180, color, caps=False)
    round_line(draw, (x + 2 * bowl, x_top), (x + 2 * bowl, base), width, color)
    # b
    x += 26 * unit + 14 * unit
    round_line(draw, (x, top), (x, base), width, color)
    arc(draw, x + bowl + 2 * unit, base - bowl, bowl + 2 * unit, width, 0, 360, color, caps=False)


def render(size):
    scale = size * SUPERSAMPLE
    unit = scale / 256.0
    image = Image.new("RGBA", (scale, scale), (0, 0, 0, 0))

    # Flat rounded panel tile with a one-pixel rim, like the sensor panels.
    inset = scale / size * 0.5 if size <= 24 else 8 * unit
    radius = 58 * unit
    rim = max(scale / size, 3 * unit)
    draw = ImageDraw.Draw(image)
    draw.rounded_rectangle([inset, inset, scale - inset, scale - inset], radius,
                           fill=PANEL + (255,), outline=RIM + (255,), width=round(rim))

    detailed = size >= 32
    labelled = size >= 48
    cx, cy = 128 * unit, (106 if labelled else 136 if detailed else 132) * unit
    ring = (64 if labelled else 70 if detailed else 76) * unit
    stroke = (22 if detailed else 30) * unit
    if detailed:
        arc(draw, cx, cy, ring, stroke, ARC_START, ARC_END, TRACK)
        arc(draw, cx, cy, ring, stroke, ARC_START, VALUE_END, ACCENT)
        tip = polar(cx, cy, ring - stroke * 0.85, NEEDLE_ANGLE)
        round_line(draw, (cx, cy), tip, 14 * unit, ACCENT_LIGHT)
        hub = 17 * unit
        draw.ellipse([cx - hub, cy - hub, cx + hub, cy + hub], fill=FOREGROUND)
    else:
        arc(draw, cx, cy, ring, stroke, ARC_START, ARC_END, ACCENT)
        hub = 26 * unit
        draw.ellipse([cx - hub, cy - hub, cx + hub, cy + hub], fill=FOREGROUND)

    if labelled:
        hub_label(draw, unit, FOREGROUND)

    return image.resize((size, size), Image.Resampling.LANCZOS)


def ico(images):
    header = struct.pack("<HHH", 0, 1, len(images))
    entries, payloads = b"", b""
    offset = 6 + 16 * len(images)
    for image in images:
        buffer = io.BytesIO()
        image.save(buffer, "PNG", optimize=True)
        data = buffer.getvalue()
        side = image.width % 256
        entries += struct.pack("<BBBBHHII", side, side, 0, 0, 1, 32, len(data), offset)
        payloads += data
        offset += len(data)
    return header + entries + payloads


def main():
    root = Path(__file__).resolve().parent.parent
    images = [render(size) for size in SIZES]
    target = root / "crates" / "app" / "assets" / "omen-hub-pp.ico"
    target.write_bytes(ico(images))
    preview = root / "docs" / "assets" / "app-icon.png"
    preview.parent.mkdir(exist_ok=True)
    images[-1].save(preview, "PNG", optimize=True)
    if len(sys.argv) == 2:
        sheet = Image.new("RGBA", (sum(SIZES) + 16 * len(SIZES), 256), (0x10, 0x12, 0x15, 255))
        x = 0
        for image in images:
            sheet.alpha_composite(image, (x, 256 - image.height))
            x += image.width + 16
        sheet.save(sys.argv[1])
    print(target)


if __name__ == "__main__":
    main()
