#!/bin/bash
# macOS 에서 배포용 ZIP 두 개를 만든다.
#   dist/DiskLens-<버전>-macOS-arm64.zip, dist/DiskLens-<버전>-windows-x64.zip (+ .sha256)
# 필요: Xcode Command Line Tools, rustup target add x86_64-pc-windows-gnu, brew install mingw-w64
# Windows 에서 직접 만들 때는 scripts/bundle-windows.ps1 을 쓴다.
set -euo pipefail
cd "$(dirname "$0")/.."

VERSION=$(sed -n 's/^version = "\([^"]*\)"$/\1/p' Cargo.toml)
test -n "$VERSION"
WIN_TARGET=x86_64-pc-windows-gnu
MAC_ZIP="DiskLens-$VERSION-macOS-arm64.zip"
WIN_ZIP="DiskLens-$VERSION-windows-x64.zip"
STAGE=$(mktemp -d)
trap 'rm -rf "$STAGE"' EXIT

# 실행 파일에 들어가는 크레이트의 라이선스 원문을 한 파일로 모은다.
notices() { # $1 = cargo --target, $2 = 출력 파일
    cargo tree --locked --target "$1" -e normal --prefix none --format '{p}' | sed 's/ (.*//' | sort -u |
    while read -r name version; do
        [ "$name" = disk-lens ] && continue
        version=${version#v}
        src=$(find "$HOME/.cargo/registry/src" -maxdepth 2 -type d -name "$name-$version" | head -1)
        test -n "$src" || { echo "크레이트 원본을 찾지 못했습니다: $name $version" >&2; exit 1; }
        files=$(find "$src" -maxdepth 1 -type f \( -iname 'LICEN[CS]E*' -o -iname 'COPYING*' -o -iname 'UNLICENSE*' -o -iname 'NOTICE*' \) | sort)
        # 원본에 라이선스 파일이 없는 크레이트는 licenses/ 에 따로 받아 둔 것을 쓴다.
        [ -n "$files" ] || files=$(find "licenses/$name-$version" -type f 2>/dev/null | sort)
        [ -n "$files" ] || { echo "라이선스 원문이 없습니다: $name $version" >&2; exit 1; }
        license=$(sed -n 's/^license = "\(.*\)"$/\1/p' "$src/Cargo.toml" | head -1)
        printf '\n=== %s %s (%s) ===\n' "$name" "$version" "$license"
        echo "$files" | while read -r file; do cat "$file"; echo; done
        if [ "$name" = epaint_default_fonts ]; then
            find "$src/fonts" -type f -name '*.txt' | sort | while read -r file; do cat "$file"; echo; done
        fi
    done > "$2"
    test -s "$2"
}

./scripts/bundle.sh
cargo build --locked --release --target "$WIN_TARGET"
mkdir -p "$STAGE/mac" "$STAGE/win"
cp "target/$WIN_TARGET/release/disk-lens.exe" "$STAGE/win/DiskLens.exe"
cargo build --locked --release --target "$WIN_TARGET" --features console
cp "target/$WIN_TARGET/release/disk-lens.exe" "$STAGE/win/disk-lens-cli.exe"

cp -R dist/DiskLens.app "$STAGE/mac/DiskLens.app"
cp ../LICENSE "$STAGE/mac/LICENSE.txt"
# macOS 쪽 크레이트는 원본에 라이선스 파일이 없는 것이 많아, 따로 모아 둔 고지 파일을 쓴다.
# 의존성이 바뀌어 빠진 크레이트가 생기면 여기서 멈춘다.
MAC_NOTICES=licenses/THIRD-PARTY-NOTICES-macOS.txt
cargo tree --locked --target aarch64-apple-darwin -e normal --prefix none --format '{p}' | sed 's/ (.*//' | sort -u |
while read -r name version; do
    [ "$name" = disk-lens ] && continue
    grep -q "^## $name ${version#v}\$" "$MAC_NOTICES" || { echo "$MAC_NOTICES 에 없습니다: $name $version" >&2; exit 1; }
done
cp "$MAC_NOTICES" "$STAGE/mac/THIRD-PARTY-NOTICES.txt"

cp WINDOWS.md "$STAGE/win/README.md"
cp ../LICENSE "$STAGE/win/LICENSE.txt"
notices "$WIN_TARGET" "$STAGE/win/THIRD-PARTY-NOTICES.txt"

rm -f "dist/$MAC_ZIP" "dist/$WIN_ZIP"
ditto -c -k --norsrc --noextattr "$STAGE/mac" "dist/$MAC_ZIP"
ditto -c -k --norsrc --noextattr "$STAGE/win" "dist/$WIN_ZIP"
(cd dist && for f in "$MAC_ZIP" "$WIN_ZIP"; do shasum -a 256 "$f" > "$f.sha256"; done && ls -lh "$MAC_ZIP" "$WIN_ZIP" && cat "$MAC_ZIP.sha256" "$WIN_ZIP.sha256")
