#!/usr/bin/env python3
"""Generate the 1024x1024 AbsoluteTouch app icon.

Requires Pillow (`python3 -m pip install pillow`). The icon is committed, so
this script is only needed when the design changes.

Usage:
    python3 tools/gen_icon.py
"""

from __future__ import annotations

from pathlib import Path

from PIL import Image, ImageDraw

SIZE = 1024
SCALE = 2
CANVAS = SIZE * SCALE

BACKGROUND_TOP = (10, 30, 45)
BACKGROUND_BOTTOM = (13, 74, 86)
ACCENT = (94, 234, 212)
STYLUS = (247, 250, 252)


def lerp(start: int, end: int, t: float) -> int:
    """Interpolate one color channel."""
    return round(start + (end - start) * t)


def build() -> Image.Image:
    """Render the icon at the final size."""
    image = Image.new("RGB", (CANVAS, CANVAS))
    draw = ImageDraw.Draw(image)

    for y in range(CANVAS):
        t = y / (CANVAS - 1)
        color = tuple(
            lerp(BACKGROUND_TOP[i], BACKGROUND_BOTTOM[i], t) for i in range(3)
        )
        draw.line([(0, y), (CANVAS, y)], fill=color)

    overlay = Image.new("RGBA", (CANVAS, CANVAS), (0, 0, 0, 0))
    od = ImageDraw.Draw(overlay)

    ring_x = CANVAS * 0.46
    ring_y = CANVAS * 0.57
    ring_r = CANVAS * 0.29
    od.ellipse(
        (ring_x - ring_r, ring_y - ring_r, ring_x + ring_r, ring_y + ring_r),
        outline=(255, 255, 255, 55),
        width=7 * SCALE,
    )

    spacing = ring_r * 0.5
    dot_r = 2.5 * SCALE
    for grid_x in range(-2, 3):
        for grid_y in range(-2, 3):
            x = ring_x + grid_x * spacing
            y = ring_y + grid_y * spacing
            if (x - ring_x) ** 2 + (y - ring_y) ** 2 <= (ring_r * 0.9) ** 2:
                od.ellipse(
                    (x - dot_r, y - dot_r, x + dot_r, y + dot_r),
                    fill=(255, 255, 255, 38),
                )

    body_width = 52 * SCALE
    body_start = (CANVAS * 0.33, CANVAS * 0.75)
    body_end = (CANVAS * 0.64, CANVAS * 0.35)
    shadow_offset = 10 * SCALE
    od.line(
        [
            (body_start[0] + shadow_offset, body_start[1] + shadow_offset),
            (body_end[0] + shadow_offset, body_end[1] + shadow_offset),
        ],
        fill=(0, 0, 0, 55),
        width=body_width,
    )
    od.line(
        [body_start, body_end],
        fill=(*STYLUS, 255),
        width=body_width,
    )
    cap_r = body_width / 2
    for point in (body_start, body_end):
        od.ellipse(
            (point[0] - cap_r, point[1] - cap_r, point[0] + cap_r, point[1] + cap_r),
            fill=(*STYLUS, 255),
        )

    tip_end = (CANVAS * 0.705, CANVAS * 0.265)
    od.line(
        [body_end, tip_end],
        fill=(*STYLUS, 255),
        width=18 * SCALE,
    )
    for point in (body_end, tip_end):
        radius = 9 * SCALE
        od.ellipse(
            (point[0] - radius, point[1] - radius, point[0] + radius, point[1] + radius),
            fill=(*STYLUS, 255),
        )

    tip_glow_r = 26 * SCALE
    od.ellipse(
        (
            tip_end[0] - tip_glow_r,
            tip_end[1] - tip_glow_r,
            tip_end[0] + tip_glow_r,
            tip_end[1] + tip_glow_r,
        ),
        fill=(*ACCENT, 70),
    )
    tip_r = 13 * SCALE
    od.ellipse(
        (
            tip_end[0] - tip_r,
            tip_end[1] - tip_r,
            tip_end[0] + tip_r,
            tip_end[1] + tip_r,
        ),
        fill=(*ACCENT, 255),
    )

    image = Image.alpha_composite(image.convert("RGBA"), overlay)
    return image.convert("RGB").resize((SIZE, SIZE), Image.Resampling.LANCZOS)


def main() -> int:
    """Write the app icon PNG."""
    root = Path(__file__).resolve().parent.parent
    target = root / "ios" / "App" / "Assets.xcassets" / "AppIcon.appiconset"
    target.mkdir(parents=True, exist_ok=True)
    path = target / "AppIcon-1024.png"

    build().save(path, "PNG", optimize=True)
    print(f"wrote {path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
