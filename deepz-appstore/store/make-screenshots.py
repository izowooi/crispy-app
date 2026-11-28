#!/usr/bin/env python3
"""store/captures 의 앱 화면에 바탕과 문구를 입혀 Mac App Store 용 스크린샷(2560x1600, 알파 없음)을 만든다.

사용: python3 store/make-screenshots.py   (deepz-appstore 폴더에서, Pillow 필요)
captures 는 Retina 화면을 그대로 찍은 것이다. 번역 장면은 1280x800 포인트 영역(screencapture -R), 메뉴와 키 창은 그 부분만.
"""
from pathlib import Path
import io

from PIL import Image, ImageCms, ImageDraw, ImageFilter, ImageFont

ROOT = Path(__file__).resolve().parent
W, H = 2560, 1600
FONT = "/System/Library/Fonts/AppleSDGothicNeo.ttc"
# (파일, 화면에 놓을 너비, 모서리 반지름, 제목, 부제)
SHOTS = [
    ("en-ko.png", 2000, 36, "선택하고 ⌘C 두 번, 번역이 바로 옆에", "어떤 앱에서든 커서 옆에 한국어 번역이 뜹니다"),
    ("ja-ko.png", 2000, 36, "어떤 언어든 한국어로", "영어·일본어·중국어… 원문 언어를 고를 필요가 없습니다"),
    ("ko-en.png", 2000, 36, "한국어는 영어로", "한국어 글을 선택하면 자연스러운 영어로 옮깁니다"),
    ("menu.png", 1500, 34, "가입 없이 하루 10회 무료", "계정도 로그인도 없이, 설치하면 바로 씁니다"),
    ("keys.png", 1380, 30, "내 API 키를 넣으면 제한 없이", "Claude · OpenAI · Gemini 키는 이 Mac 의 키체인에만 저장됩니다"),
]


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
    # 아이콘과 같은 남색 대각선 그러데이션
    stops = [(0.0, (42, 59, 214)), (0.55, (22, 32, 110)), (1.0, (9, 13, 43))]
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
for number, (name, width, radius, title, subtitle) in enumerate(SHOTS, 1):
    canvas = background().convert("RGBA")
    shot = Image.open(ROOT / "captures" / name)
    # 화면 캡처는 디스플레이 색 프로파일(P3)로 저장된다. sRGB 로 바꾸지 않으면 색이 바랜다.
    profile = shot.info.get("icc_profile")
    # 창 하나만 찍은 화면은 둥근 모서리 바깥이 투명하다. 그 모양을 그대로 쓴다.
    alpha = shot.getchannel("A") if shot.mode == "RGBA" and shot.getextrema()[3][0] < 255 else None
    shot = shot.convert("RGB")
    if profile:
        shot = ImageCms.profileToProfile(
            shot, ImageCms.ImageCmsProfile(io.BytesIO(profile)), ImageCms.createProfile("sRGB")
        )
    shot = shot.resize((width, round(shot.height * width / shot.width)), Image.LANCZOS)
    if alpha:
        alpha = alpha.resize(shot.size, Image.LANCZOS)
    # 큰 화면은 문구 바로 아래에, 작은 화면(메뉴, 키 창)은 남는 공간의 가운데에 둔다.
    x = (W - width) // 2
    y = 330 if shot.height > 1100 else 330 + (H - 330 - shot.height) // 2 - 40

    shadow = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    ImageDraw.Draw(shadow).rounded_rectangle((x, y + 24, x + shot.width, y + 24 + shot.height), radius, fill=(0, 0, 0, 150))
    canvas.alpha_composite(shadow.filter(ImageFilter.GaussianBlur(40)))

    mask = alpha or Image.new("L", shot.size, 0)
    if not alpha:
        ImageDraw.Draw(mask).rounded_rectangle((0, 0, shot.width - 1, shot.height - 1), radius, fill=255)
    canvas.paste(shot, (x, y), mask)

    draw = ImageDraw.Draw(canvas)
    draw.text((W / 2, 126), title, font=title_font, fill=(255, 255, 255), anchor="mm")
    draw.text((W / 2, 236), subtitle, font=sub_font, fill=(190, 225, 255), anchor="mm")
    canvas.convert("RGB").save(ROOT / "screenshots" / f"ko-{number}-{Path(name).stem}.png", optimize=True)
print("만들었습니다: store/screenshots/")
