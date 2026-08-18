#!/usr/bin/env python3
"""Pixel-level regression check for the compact Metra bubble screenshot."""

from __future__ import annotations

import argparse
from pathlib import Path

from PIL import Image


def normalized_bubble(path: Path) -> Image.Image:
    image = Image.open(path).convert("RGB")
    foreground = Image.new("1", image.size)
    foreground.putdata([0 if min(pixel) >= 245 else 1 for pixel in image.get_flattened_data()])
    bounds = foreground.getbbox()
    if bounds is None:
        raise ValueError("screenshot does not contain a visible bubble")
    return image.crop(bounds).resize((100, 100), Image.Resampling.LANCZOS)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("screenshot", type=Path)
    args = parser.parse_args()
    bubble = normalized_bubble(args.screenshot)

    percent_region = bubble.crop((40, 58, 87, 80))
    white_text_pixels = sum(
        1
        for red, green, blue in percent_region.get_flattened_data()
        if min(red, green, blue) >= 180 and max(red, green, blue) - min(red, green, blue) <= 50
    )

    bottom_center = bubble.crop((46, 87, 57, 97))
    bottom_dot_luminance = max(
        (red + green + blue) / 3
        for red, green, blue in bottom_center.get_flattened_data()
        if max(red, green, blue) - min(red, green, blue) <= 50
    )

    failures: list[str] = []
    if white_text_pixels > 8:
        failures.append(f"lower percentage contains {white_text_pixels} white text pixels")
    if bottom_dot_luminance > 80:
        failures.append(f"bottom-center indicator luminance is {bottom_dot_luminance:.1f}")

    if failures:
        print("FAIL: " + "; ".join(failures))
        return 1
    print(
        "PASS: lower percentage is consistently colored and the bottom-center indicator is absent "
        f"(white_pixels={white_text_pixels}, bottom_luminance={bottom_dot_luminance:.1f})"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
