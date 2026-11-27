#!/bin/bash
# Deepz 빌드(Apple Silicon + Intel) → ~/Applications 에 설치 → 실행
#   ./mac/build.sh              빌드·설치·실행
#   ./mac/build.sh --no-install 빌드만 (mac/build/Deepz.app)
set -euo pipefail
cd "$(dirname "$0")"

APP="build/Deepz.app"
INSTALLED="$HOME/Applications/Deepz.app"
SUPPORT="$HOME/Library/Application Support/Deepz"
VERSION=$(sed -n 's/^version = "\([^"]*\)"$/\1/p' ../windows/Cargo.toml)
test -n "$VERSION"

rm -rf build
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp AppIcon.icns "$APP/Contents/Resources/AppIcon.icns"
for arch in arm64 x86_64; do
    swiftc -O -swift-version 5 -target "$arch-apple-macos13.0" Sources/main.swift -o "build/Deepz-$arch"
done
lipo -create build/Deepz-arm64 build/Deepz-x86_64 -output "$APP/Contents/MacOS/Deepz"
rm build/Deepz-arm64 build/Deepz-x86_64

cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleIdentifier</key><string>local.deepz</string>
    <key>CFBundleName</key><string>Deepz</string>
    <key>CFBundleExecutable</key><string>Deepz</string>
    <key>CFBundleIconFile</key><string>AppIcon</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleShortVersionString</key><string>$VERSION</string>
    <key>CFBundleVersion</key><string>$VERSION</string>
    <key>LSMinimumSystemVersion</key><string>13.0</string>
    <key>LSUIElement</key><true/>
</dict>
</plist>
PLIST
codesign --force --sign - "$APP"
codesign --verify --strict "$APP"

if [ "${1:-}" = "--no-install" ]; then
    echo "빌드 완료: $PWD/$APP"
    exit 0
fi

# API 키는 앱 번들에 넣지 않고, 본인만 읽을 수 있는 파일로 복사한다.
if [ -f ../.env.local ]; then
    mkdir -p "$SUPPORT"
    install -m 600 ../.env.local "$SUPPORT/env"
fi

pkill -x Deepz 2>/dev/null || true
mkdir -p "$HOME/Applications"
rm -rf "$INSTALLED"
cp -R "$APP" "$INSTALLED"
open "$INSTALLED"
echo "설치 및 실행 완료: $INSTALLED"
