#!/bin/sh
# Mac App Store 용 샌드박스 앱을 만든다.
#
#   ./scripts/bundle.sh
#       임시 서명(ad-hoc)으로 dist/DiskLens.app 을 만든다. 샌드박스는 켜져 있어 내 Mac 에서 동작을 확인할 수 있다.
#
#   SIGN_APP="Apple Distribution: 이름 (팀ID)" \
#   SIGN_PKG="3rd Party Mac Developer Installer: 이름 (팀ID)" \
#   PROFILE=~/Downloads/DiskLens.provisionprofile \
#   BUILD=1 ./scripts/bundle.sh
#       배포용 인증서로 서명하고 dist/DiskLens-<버전>.pkg 까지 만든다. 이 pkg 를 Transporter 로 올린다.
#       BUILD 는 올릴 때마다 1씩 키운다.
set -eu
cd "$(dirname "$0")/.."
export MACOSX_DEPLOYMENT_TARGET=12.0
BUNDLE_ID=com.zowoo.app.disklens
BUILD=${BUILD:-1}
cargo build --locked --release
version=$(sed -n 's/^version = "\([^"]*\)"$/\1/p' Cargo.toml)
test -n "$version"
app="dist/DiskLens.app"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources/en.lproj" "$app/Contents/Resources/ko.lproj"
swift scripts/make-icon.swift dist/DiskLens.iconset
iconutil --convert icns --output "$app/Contents/Resources/DiskLens.icns" dist/DiskLens.iconset
cp target/release/disk-lens-appstore "$app/Contents/MacOS/DiskLens"
cp licenses/THIRD-PARTY-NOTICES-macOS.txt "$app/Contents/Resources/THIRD-PARTY-NOTICES.txt"
cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleName</key><string>DiskLens</string>
<key>CFBundleDisplayName</key><string>DiskLens</string>
<key>CFBundleDevelopmentRegion</key><string>en</string>
<key>CFBundleLocalizations</key><array><string>en</string><string>ko</string></array>
<key>CFBundleIconFile</key><string>DiskLens.icns</string>
<key>CFBundleIdentifier</key><string>$BUNDLE_ID</string>
<key>CFBundleVersion</key><string>$BUILD</string>
<key>CFBundleShortVersionString</key><string>$version</string>
<key>CFBundleExecutable</key><string>DiskLens</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
<key>CFBundleSupportedPlatforms</key><array><string>MacOSX</string></array>
<key>LSApplicationCategoryType</key><string>public.app-category.utilities</string>
<key>LSMinimumSystemVersion</key><string>12.0</string>
<key>NSHighResolutionCapable</key><true/>
<key>ITSAppUsesNonExemptEncryption</key><false/>
</dict></plist>
PLIST
for language in en ko; do
    printf '"CFBundleName" = "DiskLens";\n"CFBundleDisplayName" = "DiskLens";\n' > "$app/Contents/Resources/$language.lproj/InfoPlist.strings"
done

entitlements=$(mktemp)
trap 'rm -f "$entitlements"' EXIT
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
cat > "$entitlements" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>com.apple.security.app-sandbox</key><true/>
<key>com.apple.security.files.user-selected.read-only</key><true/>
$identity
</dict></plist>
PLIST
codesign --force --options runtime --entitlements "$entitlements" --sign "${SIGN_APP:--}" "$app"
codesign --verify --strict "$app"
printf '%s\n' "Built: $app"
if [ -n "${SIGN_PKG:-}" ]; then
    pkg="dist/DiskLens-$version.pkg"
    productbuild --component "$app" /Applications --sign "$SIGN_PKG" "$pkg"
    printf '%s\n' "Built: $pkg"
fi
