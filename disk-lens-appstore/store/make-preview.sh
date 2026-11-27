#!/bin/sh
# 창을 녹화한 영상(store/preview/raw-window-<언어>.mov)을 App Store 미리보기 규격으로 만든다.
#   규격: 1920x1080, 30fps, H.264, 15~30초, 스테레오 AAC (소리는 무음)
#   녹화: screencapture -v -C -V 27 -R<창 x,y,너비,높이> store/preview/raw-window-ko.mov
# 사용: ./store/make-preview.sh   (disk-lens-appstore 폴더에서, ffmpeg 와 Pillow 필요)
set -eu
cd "$(dirname "$0")/.."
background=$(mktemp).png
cover=$(mktemp).png
trap 'rm -f "$background" "$cover"' EXIT
python3 - "$background" "$cover" <<'PY'
import sys
from PIL import Image
stops = [(0.0, (31, 167, 160)), (0.55, (15, 85, 104)), (1.0, (7, 34, 47))]
small = Image.new("RGB", (192, 108))
pixels = small.load()
for y in range(108):
    for x in range(192):
        t = (x / 191 + y / 107) / 2
        for (t0, c0), (t1, c1) in zip(stops, stops[1:]):
            if t <= t1:
                k = (t - t0) / (t1 - t0)
                pixels[x, y] = tuple(round(a + (b - a) * k) for a, b in zip(c0, c1))
                break
background = small.resize((1920, 1080), Image.BICUBIC)
background.save(sys.argv[1])
# 창의 둥근 모서리 밖에 찍힌 바탕 화면을 가리는 덮개: 창 자리만 뚫린 바탕
from PIL import ImageDraw
width, height = 1472, 980
x, y = (1920 - width) // 2, (1080 - height) // 2
hole = Image.new("L", (1920, 1080), 255)
ImageDraw.Draw(hole).rounded_rectangle((x + 1, y + 1, x + width - 2, y + height - 2), 13, fill=0)
cover = background.convert("RGBA")
cover.putalpha(hole)
cover.save(sys.argv[2])
PY
for language in ko en; do
ffmpeg -v error -y -loop 1 -framerate 30 -i "$background" -i "store/preview/raw-window-$language.mov" -i "$cover" \
    -f lavfi -i anullsrc=channel_layout=stereo:sample_rate=48000 \
    -filter_complex "[1:v]fps=30,scale=1472:980:flags=lanczos[w];[0:v][w]overlay=(W-w)/2:(H-h)/2:shortest=1[f];[f][2:v]overlay=0:0,format=yuv420p[v]" \
    -map "[v]" -map 3:a -c:v libx264 -profile:v high -level 4.2 -b:v 10M -maxrate 12M -bufsize 20M \
    -colorspace bt709 -color_primaries bt709 -color_trc bt709 \
    -c:a aac -b:a 256k -shortest -movflags +faststart "store/preview/$language-preview.mp4"
done
echo "만들었습니다: store/preview/ko-preview.mp4, en-preview.mp4"
