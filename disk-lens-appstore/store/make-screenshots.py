#!/usr/bin/env python3
"""store/captures 의 앱 화면에 바탕과 문구를 입혀 Mac App Store 용 스크린샷(2560x1600, 알파 없음)을 만든다.

사용: python3 store/make-screenshots.py   (disk-lens-appstore 폴더에서, Pillow 필요)
captures/<언어> 는 창 하나를 Retina 로 찍은 것이다: screencapture -o -l <창 번호> (2560x1704).
"""
from pathlib import Path
import io

from PIL import Image, ImageCms, ImageDraw, ImageFilter, ImageFont

ROOT = Path(__file__).resolve().parent.parent
W, H = 2560, 1600
WINDOW_WIDTH = 2000
FONT = "/System/Library/Fonts/AppleSDGothicNeo.ttc"
SHOTS = {
    "ko": [
        ("treemap.png", "무엇이 용량을 차지하는지 한눈에", "큰 것은 크게, 작은 것은 작게 보여 줍니다"),
        ("folder.png", "눌러서 폴더 안으로", "궁금한 칸을 누르면 그 안을 다시 펼칩니다"),
        ("sunburst.png", "방사형 차트로도", "안쪽 고리가 상위 폴더, 바깥 고리가 그 안의 내용입니다"),
        ("details.png", "큰 파일이 어디 있는지 바로", "이름·크기·경로를 확인하고 Finder 에서 엽니다"),
        ("welcome.png", "직접 고른 폴더만, 읽기만", "아무것도 지우거나 옮기지 않고, 인터넷으로 보내지도 않습니다"),
    ],
    "en": [
        ("treemap.png", "See what fills your disk", "Big things look big, small things look small"),
        ("folder.png", "Click to go inside", "Click any block to open that folder"),
        ("sunburst.png", "Or view it as a sunburst", "Inner rings are parent folders, outer rings are what they hold"),
        ("details.png", "Find the big files fast", "Check the name, size and location, then reveal it in Finder"),
        ("welcome.png", "Only what you choose, read-only", "Nothing is deleted, moved or sent anywhere"),
    ],
}


def font(size, bold):
    wanted = "Bold" if bold else "Regular"
    for index in range(12):
        try:
            candidate = ImageFont.truetype(FONT, size, index=index)
        except OSError:
            break
        if candidate.getname() == ("Apple SD Gothic Neo", wanted):
            return candidate
    raise SystemExit(f"Apple SD Gothic Neo {wanted} 를 찾지 못했습니다")


def background():
    # 아이콘과 같은 청록 대각선 그러데이션
    stops = [(0.0, (31, 167, 160)), (0.55, (15, 85, 104)), (1.0, (7, 34, 47))]
    small = Image.new("RGB", (256, 160))
    pixels = small.load()
    for y in range(160):
        for x in range(256):
            t = (x / 255 + y / 159) / 2
            for (t0, c0), (t1, c1) in zip(stops, stops[1:]):
                if t <= t1:
                    k = (t - t0) / (t1 - t0)
                    pixels[x, y] = tuple(round(a + (b - a) * k) for a, b in zip(c0, c1))
                    break
    return small.resize((W, H), Image.BICUBIC)


title_font, sub_font = font(104, True), font(46, False)
for language, shots in SHOTS.items():
  for number, (name, title, subtitle) in enumerate(shots, 1):
      canvas = background().convert("RGBA")
      shot = Image.open(ROOT / "store/captures" / language / name)
      # 화면 캡처는 디스플레이 색 프로파일(P3)로 저장된다. sRGB 로 바꾸지 않으면 색이 바랜다.
      profile = shot.info.get("icc_profile")
      shot = shot.convert("RGB")
      if profile:
          shot = ImageCms.profileToProfile(
              shot, ImageCms.ImageCmsProfile(io.BytesIO(profile)), ImageCms.createProfile("sRGB")
          )
      shot = shot.resize((WINDOW_WIDTH, round(shot.height * WINDOW_WIDTH / shot.width)), Image.LANCZOS)
      x, y, radius = (W - WINDOW_WIDTH) // 2, 330, 24

      shadow = Image.new("RGBA", (W, H), (0, 0, 0, 0))
      ImageDraw.Draw(shadow).rounded_rectangle((x, y + 24, x + shot.width, y + 24 + shot.height), radius, fill=(0, 0, 0, 150))
      canvas.alpha_composite(shadow.filter(ImageFilter.GaussianBlur(40)))

      mask = Image.new("L", shot.size, 0)
      ImageDraw.Draw(mask).rounded_rectangle((0, 0, shot.width - 1, shot.height - 1), radius, fill=255)
      canvas.paste(shot, (x, y), mask)

      draw = ImageDraw.Draw(canvas)
      draw.text((W / 2, 126), title, font=title_font, fill=(255, 255, 255), anchor="mm")
      draw.text((W / 2, 236), subtitle, font=sub_font, fill=(200, 240, 232), anchor="mm")
      canvas.convert("RGB").save(ROOT / "store/screenshots" / f"{language}-{number}-{Path(name).stem}.png", optimize=True)
print("만들었습니다: store/screenshots/")
