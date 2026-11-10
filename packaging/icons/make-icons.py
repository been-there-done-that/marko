#!/usr/bin/env python3
"""Builds every icon file Marko ships from one square master PNG (1024 px or larger).

    python3 packaging/icons/make-icons.py path/to/master.png

Writes, next to this script: marko.png (1024), the hicolor sizes in png/ (16 to 512),
marko.ico (Windows, 16 to 256) and marko.icns (macOS). Needs Pillow.
"""
import sys
from pathlib import Path

from PIL import Image

HERE = Path(__file__).resolve().parent
SIZES = [16, 24, 32, 48, 64, 128, 256, 512]


def main() -> None:
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    master = Image.open(sys.argv[1]).convert("RGBA")
    if master.width != master.height:
        sys.exit("the master must be square")
    if master.width < 1024:
        print("warning: master is smaller than 1024 px; large sizes will be soft", file=sys.stderr)
    big = master.resize((1024, 1024), Image.LANCZOS)
    big.save(HERE / "marko.png")
    (HERE / "png").mkdir(exist_ok=True)
    for s in SIZES:
        big.resize((s, s), Image.LANCZOS).save(HERE / "png" / f"marko-{s}.png")
    big.save(HERE / "marko.ico", sizes=[(s, s) for s in [16, 24, 32, 48, 64, 128, 256]])
    big.save(HERE / "marko.icns")
    print("wrote marko.png, png/, marko.ico, marko.icns in", HERE)


if __name__ == "__main__":
    main()
