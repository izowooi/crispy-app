#!/bin/bash
# DeepZ 빌드(Apple Silicon) → ~/Applications 에 설치 → 실행
#   ./mac/build.sh              빌드·설치·실행
#   ./mac/build.sh --no-install 빌드만 (mac/build/DeepZ.app)
set -euo pipefail
cd "$(dirname "$0")"

APP="build/DeepZ.app"
INSTALLED="$HOME/Applications/DeepZ.app"
SUPPORT="$HOME/Library/Application Support/DeepZ"
VERSION=$(sed -n 's/^version = "\([^"]*\)"$/\1/p' ../windows/Cargo.toml)
test -n "$VERSION"

rm -rf build
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp AppIcon.icns "$APP/Contents/Resources/AppIcon.icns"
# 무료 번역 서버의 서명용 값. 저장소에 넣지 않고 빌드할 때만 읽는다. 없으면 무료 번역이 빠진 앱이 된다.
SECRET="${DEEPZ_APP_SECRET:-$(sed -n 's/^DEEPZ_APP_SECRET=//p' "$HOME/git/.auth/deepz.env" 2>/dev/null | head -1)}"
MASKED=""
for byte in $(printf '%s' "$SECRET" | od -An -v -tu1); do MASKED="$MASKED$((byte ^ 90)), "; done
printf 'enum AppSecret {\n    static let pad: UInt8 = 90\n    static let masked: [UInt8] = [%s]\n}\n' "$MASKED" > build/AppSecret.swift
[ -n "$SECRET" ] || echo "알림: DEEPZ_APP_SECRET 이 없어 무료 번역 없이 빌드합니다." >&2
swiftc -O -swift-version 5 -target arm64-apple-macos13.0 Sources/main.swift build/AppSecret.swift -o "$APP/Contents/MacOS/DeepZ"
rm build/AppSecret.swift

cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleIdentifier</key><string>local.deepz</string>
    <key>CFBundleName</key><string>DeepZ</string>
    <key>CFBundleExecutable</key><string>DeepZ</string>
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

pkill -x DeepZ 2>/dev/null || true
mkdir -p "$HOME/Applications"
rm -rf "$INSTALLED"
cp -R "$APP" "$INSTALLED"
open "$INSTALLED"
echo "설치 및 실행 완료: $INSTALLED"
