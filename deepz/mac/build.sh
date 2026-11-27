#!/bin/bash
# Deepz 빌드 → ~/Applications 에 설치 → 실행
set -euo pipefail
cd "$(dirname "$0")"

APP="build/Deepz.app"
INSTALLED="$HOME/Applications/Deepz.app"
SUPPORT="$HOME/Library/Application Support/Deepz"

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp AppIcon.icns "$APP/Contents/Resources/AppIcon.icns"
swiftc -O -swift-version 5 Sources/main.swift -o "$APP/Contents/MacOS/Deepz"

cat > "$APP/Contents/Info.plist" <<'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleIdentifier</key><string>local.deepz</string>
    <key>CFBundleName</key><string>Deepz</string>
    <key>CFBundleExecutable</key><string>Deepz</string>
    <key>CFBundleIconFile</key><string>AppIcon</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleShortVersionString</key><string>1.0</string>
    <key>CFBundleVersion</key><string>1</string>
    <key>LSMinimumSystemVersion</key><string>13.0</string>
    <key>LSUIElement</key><true/>
</dict>
</plist>
EOF
codesign --force --sign - "$APP"

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
