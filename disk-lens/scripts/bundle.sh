#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
export MACOSX_DEPLOYMENT_TARGET=12.0
cargo build --locked --release
version=$(sed -n 's/^version = "\([^"]*\)"$/\1/p' Cargo.toml)
test -n "$version"
app="dist/DiskLens.app"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
swift scripts/make-icon.swift dist/DiskLens.iconset
cp assets/disk-lens.svg "$app/Contents/Resources/DiskLens.svg"
iconutil --convert icns --output "$app/Contents/Resources/DiskLens.icns" dist/DiskLens.iconset
cp target/release/disk-lens "$app/Contents/MacOS/disk-lens"
cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleName</key><string>DiskLens</string>
<key>CFBundleDisplayName</key><string>DiskLens</string>
<key>CFBundleDevelopmentRegion</key><string>en</string>
<key>CFBundleLocalizations</key><array><string>en</string><string>ko</string><string>zh-Hans</string><string>ja</string><string>de</string><string>fr</string><string>it</string><string>es</string><string>ar</string><string>pl</string><string>ru</string></array>
<key>CFBundleIconFile</key><string>DiskLens.icns</string>
<key>CFBundleIdentifier</key><string>local.jongwoo.disklens</string>
<key>CFBundleVersion</key><string>$version</string>
<key>CFBundleShortVersionString</key><string>$version</string>
<key>CFBundleExecutable</key><string>disk-lens</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>LSMinimumSystemVersion</key><string>12.0</string>
<key>NSHighResolutionCapable</key><true/>
</dict></plist>
PLIST
for language in en ko zh-Hans ja de fr it es ar pl ru; do
    mkdir -p "$app/Contents/Resources/$language.lproj"
    printf '"CFBundleName" = "DiskLens";\n"CFBundleDisplayName" = "DiskLens";\n' > "$app/Contents/Resources/$language.lproj/InfoPlist.strings"
done
codesign --force --deep --sign - "$app"
printf '%s\n' "Built: $app"
