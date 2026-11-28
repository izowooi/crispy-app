#!/bin/bash
# 배포용 ZIP 을 만든다 → dist/DeepZ-<버전>-macOS-arm64.zip, dist/DeepZ-<버전>-windows-x64.zip (+ .sha256)
# 필요: Xcode Command Line Tools, rustup target add x86_64-pc-windows-gnu, brew install mingw-w64
set -euo pipefail
cd "$(dirname "$0")/.."

VERSION=$(sed -n 's/^version = "\([^"]*\)"$/\1/p' windows/Cargo.toml)
test -n "$VERSION"
MAC_ZIP="DeepZ-$VERSION-macOS-arm64.zip"
WIN_ZIP="DeepZ-$VERSION-windows-x64.zip"
STAGE=$(mktemp -d)
trap 'rm -rf "$STAGE"' EXIT

# 공개 배포 파일에는 무료 번역 서버의 서명용 값을 넣지 않는다. (실행 파일에서 꺼낼 수 있다.)

DEEPZ_NO_SECRET=1 ./mac/build.sh --no-install
DEEPZ_NO_SECRET=1 ./windows/build.sh
mkdir -p dist
rm -f "dist/$MAC_ZIP" "dist/$WIN_ZIP"

# macOS: 앱 + 라이선스
mkdir "$STAGE/mac"
cp -R mac/build/DeepZ.app "$STAGE/mac/DeepZ.app"
cp ../LICENSE "$STAGE/mac/LICENSE.txt"
ditto -c -k --norsrc --noextattr "$STAGE/mac" "dist/$MAC_ZIP"

# Windows: 실행 파일 + 라이선스 + 포함된 Rust 크레이트의 라이선스 고지
mkdir -p "$STAGE/win/licenses"
cp dist/DeepZ.exe "$STAGE/win/DeepZ.exe"
cp ../LICENSE "$STAGE/win/LICENSE.txt"
(cd windows && cargo tree --locked --target x86_64-pc-windows-gnu -e normal --prefix none --format '{p}' \
    | sed 's/ (.*//' | sort -u) | while read -r name version; do
    [ "$name" = deepz ] && continue
    version=${version#v}
    src=$(find "$HOME/.cargo/registry/src" -maxdepth 2 -type d -name "$name-$version" | head -1)
    test -n "$src" || { echo "라이선스 원본을 찾지 못했습니다: $name $version" >&2; exit 1; }
    mkdir -p "$STAGE/win/licenses/$name-$version"
    find "$src" -maxdepth 1 -type f \( -iname 'LICEN[CS]E*' -o -iname 'COPYING*' -o -iname 'NOTICE*' \) \
        -exec cp {} "$STAGE/win/licenses/$name-$version/" \;
    license=$(sed -n 's/^license = "\(.*\)"$/\1/p' "$src/Cargo.toml" | head -1)
    echo "$name $version — ${license:-라이선스 표기는 폴더 안 파일 참고}" >> "$STAGE/win/licenses/README.txt"
done
ditto -c -k --norsrc --noextattr "$STAGE/win" "dist/$WIN_ZIP"

(cd dist && for f in "$MAC_ZIP" "$WIN_ZIP"; do shasum -a 256 "$f" > "$f.sha256"; done && ls -lh "$MAC_ZIP" "$WIN_ZIP" && cat ./*.sha256)
