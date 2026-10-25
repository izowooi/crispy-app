#!/usr/bin/env python3
"""Generate original, numbered QA pages; no books or downloads are used."""
import argparse
import io
import random
import zipfile
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont


def page(number, size=(1600, 2400)):
    rng = random.Random(number)
    w, h = size
    image = Image.new("RGB", size, "#f2efe7")
    draw = ImageDraw.Draw(image)
    font_path = next((str(p) for p in [
        Path("/System/Library/Fonts/Supplemental/Arial Bold.ttf"),
        Path("C:/Windows/Fonts/arialbd.ttf"),
        Path("/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf"),
    ] if p.exists()), "DejaVuSans.ttf")
    font = ImageFont.truetype(font_path, int(min(w * .055, h * .04)))
    small = ImageFont.truetype(font_path, int(min(w * .023, h * .02)))
    draw.text((w * .065, h * .04), "THE QUIET ORBIT", font=font, fill="#20272d")
    draw.text((w * .067, h * .095), f"SUA VIEW / ORIGINAL TEST PAGES / {number:03}", font=small, fill="#42565b")
    panels = [(0.065, .15, .935, .49), (.065, .51, .49, .86), (.51, .51, .935, .86)]
    for n, box in enumerate(panels):
        x0, y0, x1, y1 = [int(value * (w if i % 2 == 0 else h)) for i, value in enumerate(box)]
        draw.rectangle((x0, y0, x1, y1), fill="#23343e", outline="#121b21", width=8)
        for _ in range(100):
            x, y = rng.randint(x0+10, x1-10), rng.randint(y0+10, y1-10)
            r = rng.choice([1, 2, 3])
            draw.ellipse((x-r, y-r, x+r, y+r), fill="#afcac5")
        radius = int(min(x1-x0, y1-y0) * (.25 if n == 0 else .2))
        cx, cy = (x0+x1)//2, (y0+y1)//2
        draw.ellipse((cx-radius, cy-radius, cx+radius, cy+radius), fill="#69d6c5")
        draw.line((x0+25, y1-30, x1-25, y1-30), fill="#e1e6de", width=5)
    draw.text((w * .065, h * .91), "A little space to read.", font=small, fill="#42565b")
    label = f"{number:03}"
    draw.text((w * .8, h * .9), label, font=font, fill="#20272d")
    output = io.BytesIO()
    image.save(output, "JPEG", quality=88)
    return output.getvalue()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--large", action="store_true", help="Also generate 200-page benchmark archive")
    parser.add_argument("--output", type=Path, default=Path("local/fixtures"))
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    folder = args.output / "pages"
    folder.mkdir(exist_ok=True)
    with zipfile.ZipFile(args.output / "sample.cbz", "w", compression=zipfile.ZIP_DEFLATED) as archive:
        # Intentionally unordered file names exercise natural sorting.
        for i in [12, 2, 10, 1, 7, 6, 3, 4, 9, 5, 8, 11]:
            data = page(i, (2400, 1600) if i == 6 else (1600, 2400))
            archive.writestr(f"page{i}.jpg", data)
            (folder / f"page{i}.jpg").write_bytes(data)
        archive.writestr("__MACOSX/._page1.jpg", b"ignored")
        archive.writestr(".hidden.jpg", b"ignored")
        archive.writestr("ComicInfo.xml", "<ComicInfo><Title>QA</Title></ComicInfo>")
    with zipfile.ZipFile(args.output / "broken-page.cbz", "w") as archive:
        archive.writestr("page1.jpg", b"invalid jpeg")
        archive.writestr("page2.jpg", page(2))
    (args.output / "broken.cbz").write_bytes(b"this is not an archive")
    if args.large:
        with zipfile.ZipFile(args.output / "large.cbz", "w", compression=zipfile.ZIP_DEFLATED) as archive:
            for i in range(1, 201):
                archive.writestr(f"page{i}.jpg", page(i))
    print(args.output.resolve())


if __name__ == "__main__":
    main()
