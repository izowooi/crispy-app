#!/bin/sh
# Mac App Store 용 샌드박스 앱을 만든다.
#
#   ./scripts/bundle.sh
#       임시 서명(ad-hoc)으로 dist/DeepZ.app 을 만든다. 샌드박스는 켜져 있어 내 Mac 에서 동작을 확인할 수 있다.
#
#   SIGN_APP="Apple Distribution: 이름 (팀ID)" \
#   SIGN_PKG="3rd Party Mac Developer Installer: 이름 (팀ID)" \
#   PROFILE=~/Downloads/deepz_release.provisionprofile \
#   BUILD=1 ./scripts/bundle.sh
#       배포용 인증서로 서명하고 dist/DeepZ-<버전>.pkg 까지 만든다.
#       ~/git/tools/mac-appstore-upload/upload.sh 로 올린다. BUILD 는 올릴 때마다 1씩 키운다.
set -eu
cd "$(dirname "$0")/.."
VERSION=1.0.0
BUNDLE_ID=com.izowooi.app.deepz
BUILD=${BUILD:-1}
app="dist/DeepZ.app"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp AppIcon.icns "$app/Contents/Resources/AppIcon.icns"

# 무료 번역 서버의 서명용 값. 저장소에 넣지 않고 빌드할 때만 읽는다. 직접 다운로드판과 다른, 스토어판 전용 값이다.
SECRET="${DEEPZ_STORE_SECRET:-$(sed -n 's/^DEEPZ_STORE_SECRET=//p' "$HOME/git/.auth/deepz.env" 2>/dev/null | head -1)}"
if [ -z "$SECRET" ]; then
    [ -z "${SIGN_APP:-}" ] || { echo "DEEPZ_STORE_SECRET 이 없습니다. 스토어에 올릴 앱은 무료 번역이 들어 있어야 합니다." >&2; exit 1; }
    echo "알림: DEEPZ_STORE_SECRET 이 없어 무료 번역 없이 빌드합니다." >&2
fi
MASKED=""
for byte in $(printf '%s' "$SECRET" | od -An -v -tu1); do MASKED="$MASKED$((byte ^ 90)), "; done
generated=$(mktemp -d)
entitlements="$generated/entitlements.plist"
trap 'rm -rf "$generated"' EXIT
printf 'enum AppSecret {\n    static let pad: UInt8 = 90\n    static let masked: [UInt8] = [%s]\n}\n' "$MASKED" > "$generated/AppSecret.swift"
swiftc -O -swift-version 5 -target arm64-apple-macos13.0 Sources/main.swift "$generated/AppSecret.swift" -o "$app/Contents/MacOS/DeepZ"

cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleName</key><string>DeepZ</string>
<key>CFBundleDisplayName</key><string>DeepZ</string>
<key>CFBundleDevelopmentRegion</key><string>ko</string>
<key>CFBundleLocalizations</key><array><string>ko</string></array>
<key>CFBundleIconFile</key><string>AppIcon</string>
<key>CFBundleIdentifier</key><string>$BUNDLE_ID</string>
<key>CFBundleVersion</key><string>$BUILD</string>
<key>CFBundleShortVersionString</key><string>$VERSION</string>
<key>CFBundleExecutable</key><string>DeepZ</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
<key>CFBundleSupportedPlatforms</key><array><string>MacOSX</string></array>
<key>LSApplicationCategoryType</key><string>public.app-category.productivity</string>
<key>LSMinimumSystemVersion</key><string>13.0</string>
<key>LSUIElement</key><true/>
<key>NSHighResolutionCapable</key><true/>
<key>ITSAppUsesNonExemptEncryption</key><false/>
</dict></plist>
PLIST
# App Store 는 lproj 폴더를 보고 지원 언어를 표시한다. 지금은 한국어만 지원한다.
mkdir -p "$app/Contents/Resources/ko.lproj"
printf '"CFBundleName" = "DeepZ";\n"CFBundleDisplayName" = "DeepZ";\n' > "$app/Contents/Resources/ko.lproj/InfoPlist.strings"

if [ -n "${SIGN_APP:-}" ]; then
    : "${PROFILE:?배포용 서명에는 PROFILE(프로비저닝 프로파일 경로)이 필요합니다}"
    team=$(printf '%s' "$SIGN_APP" | sed -n 's/.*(\([A-Z0-9]*\))$/\1/p')
    test -n "$team"
    cp "$PROFILE" "$app/Contents/embedded.provisionprofile"
    identity="<key>com.apple.application-identifier</key><string>$team.$BUNDLE_ID</string>
<key>com.apple.developer.team-identifier</key><string>$team</string>"
else
    identity=""
fi
# 샌드박스 + 나가는 네트워크 연결(번역 요청)만 쓴다.
cat > "$entitlements" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>com.apple.security.app-sandbox</key><true/>
<key>com.apple.security.network.client</key><true/>
$identity
</dict></plist>
PLIST
/usr/bin/xattr -cr "$app"
codesign --force --options runtime --entitlements "$entitlements" --sign "${SIGN_APP:--}" "$app"
codesign --verify --strict "$app"
echo "앱: $PWD/$app"

if [ -n "${SIGN_PKG:-}" ]; then
    pkg="dist/DeepZ-$VERSION.pkg"
    rm -f "$pkg"
    productbuild --component "$app" /Applications --sign "$SIGN_PKG" "$pkg"
    pkgutil --check-signature "$pkg" | sed -n 1,2p
    echo "pkg: $PWD/$pkg (버전 $VERSION, 빌드 $BUILD)"
fi
