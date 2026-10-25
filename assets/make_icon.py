#!/usr/bin/env python3
"""Generates the xpaint icon, a rainbow-filled X, as SVG and PNG from one set of shapes.

Usage: python3 assets/make_icon.py   (needs Pillow; writes into assets/)
"""
import math
from pathlib import Path

from PIL import Image, ImageDraw

OUT = Path(__file__).resolve().parent
SIZE = 512

BG = "#1E2A38"
BG_RING = "#2C3D52"
SHADOW = "#121A23"
# Gradient stops from left to right, so both bars run through every color.
RAINBOW = ["#E05A47", "#F08A3C", "#F3D36B", "#5BB55B", "#3D8BD9", "#9B6BD6"]

# The X is two rounded bars crossing in the middle of the frame.
CENTER = 256
ARM = 150  # distance from the center to each bar end along the axes
HALF_WIDTH = 52
SHADOW_OFFSET = 12


def bar(direction, dx=0.0, dy=0.0, steps=24):
    """A stadium (rounded bar) from one corner of the X to the opposite one."""
    a = (CENTER - ARM + dx, CENTER - ARM * direction + dy)
    b = (CENTER + ARM + dx, CENTER + ARM * direction + dy)
    angle = math.atan2(b[1] - a[1], b[0] - a[0])
    pts = []
    for end, start_angle in ((b, angle - math.pi / 2), (a, angle + math.pi / 2)):
        for i in range(steps + 1):
            t = start_angle + math.pi * i / steps
            pts.append((end[0] + HALF_WIDTH * math.cos(t), end[1] + HALF_WIDTH * math.sin(t)))
    return pts


def x_shape(dx=0.0, dy=0.0):
    return [bar(1, dx, dy), bar(-1, dx, dy)]


def svg_path(polygons):
    return " ".join("M" + " L".join(f"{x:.1f},{y:.1f}" for x, y in p) + " Z" for p in polygons)


def write_svg():
    lo, hi = CENTER - ARM - HALF_WIDTH, CENTER + ARM + HALF_WIDTH
    stops = "".join(
        f'<stop offset="{i / (len(RAINBOW) - 1):.2f}" stop-color="{c}"/>'
        for i, c in enumerate(RAINBOW)
    )
    parts = [
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {SIZE} {SIZE}">',
        f'<defs><linearGradient id="paint" gradientUnits="userSpaceOnUse" '
        f'x1="{lo}" y1="0" x2="{hi}" y2="0">{stops}</linearGradient></defs>',
        f'<rect x="8" y="8" width="496" height="496" rx="116" fill="{BG}" stroke="{BG_RING}" stroke-width="10"/>',
        f'<path d="{svg_path(x_shape(0, SHADOW_OFFSET))}" fill="{SHADOW}"/>',
        f'<path d="{svg_path(x_shape())}" fill="url(#paint)"/>',
        "</svg>",
    ]
    (OUT / "icon.svg").write_text("\n".join(parts) + "\n")


def hex_rgb(c):
    return tuple(int(c[i : i + 2], 16) for i in (1, 3, 5))


def gradient(size, scale):
    """A horizontal gradient through RAINBOW, matching the SVG's linearGradient."""
    lo, hi = (CENTER - ARM - HALF_WIDTH) * scale, (CENTER + ARM + HALF_WIDTH) * scale
    n = 1024
    stops = [hex_rgb(c) for c in RAINBOW]
    strip = []
    for i in range(n):
        t = i / (n - 1) * (len(stops) - 1)
        k = min(int(t), len(stops) - 2)
        f = t - k
        strip.append(tuple(round(stops[k][j] + (stops[k + 1][j] - stops[k][j]) * f) for j in range(3)))
    row = Image.new("RGB", (size, 1))
    for x in range(size):
        t = (x - lo) / (hi - lo)
        row.putpixel((x, 0), strip[min(n - 1, max(0, round(t * (n - 1))))])
    return row.resize((size, size), Image.NEAREST)


def render_png(size):
    ss = 2  # supersampling factor for anti-aliasing
    full = size * ss
    scale = full / SIZE
    img = Image.new("RGBA", (full, full), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    d.rounded_rectangle(
        [8 * scale, 8 * scale, 504 * scale, 504 * scale],
        radius=116 * scale, fill=BG, outline=BG_RING, width=round(10 * scale),
    )

    def mask(polygons):
        m = Image.new("L", (full, full), 0)
        md = ImageDraw.Draw(m)
        for p in polygons:
            md.polygon([(x * scale, y * scale) for x, y in p], fill=255)
        return m

    img.paste(SHADOW, (0, 0), mask(x_shape(0, SHADOW_OFFSET)))
    img.paste(gradient(full, scale), (0, 0), mask(x_shape()))
    return img.resize((size, size), Image.LANCZOS)


if __name__ == "__main__":
    write_svg()
    for size in (256, 512):
        render_png(size).save(OUT / f"icon-{size}.png")
    print("wrote", OUT)
