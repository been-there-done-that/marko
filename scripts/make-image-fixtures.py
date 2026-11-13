#!/usr/bin/env python3
"""Generates the large pictures `fixtures/image-heavy.md` decodes.

They are solid colours on purpose: the file is tiny, but the decoded RGBA is 48 MB per picture,
which is what the memory measurement needs. Run from the repository root:

    python3 scripts/make-image-fixtures.py
"""

from pathlib import Path

from PIL import Image

OUT = Path(__file__).resolve().parent.parent / "fixtures" / "images"

PNG = [(200, 30, 30), (30, 150, 80), (30, 80, 200), (180, 140, 20)]
JPG = [(120, 40, 160), (20, 140, 160)]


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    for i, rgb in enumerate(PNG, 1):
        Image.new("RGBA", (4000, 3000), (*rgb, 255)).save(OUT / f"photo-{i}.png")
    for i, rgb in enumerate(JPG, len(PNG) + 1):
        Image.new("RGB", (4000, 3000), rgb).save(OUT / f"photo-{i}.jpg", quality=85)


if __name__ == "__main__":
    main()
