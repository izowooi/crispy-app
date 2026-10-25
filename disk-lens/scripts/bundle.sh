#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
export MACOSX_DEPLOYMENT_TARGET=12.0
cargo build --locked --release
version=$(sed -n 's/^version = "\([^"]*\)"$/\1/p' Cargo.toml)
test -n "$version"
app="dist/Disk Lens.app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources/en.lproj" "$app/Contents/Resources/ko.lproj"
swift scripts/make-icon.swift dist/DiskLens.iconset assets/disk-lens.svg
cp assets/disk-lens.svg "$app/Contents/Resources/DiskLens.svg"
iconutil --convert icns --output "$app/Contents/Resources/DiskLens.icns" dist/DiskLens.iconset
cp target/release/disk-lens "$app/Contents/MacOS/disk-lens"
cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleName</key><string>Disk Lens</string>
<key>CFBundleDisplayName</key><string>Disk Lens</string>
<key>CFBundleDevelopmentRegion</key><string>en</string>
<key>CFBundleLocalizations</key><array><string>en</string><string>ko</string></array>
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
cat > "$app/Contents/Resources/en.lproj/InfoPlist.strings" <<'ENGLISH'
"CFBundleName" = "Disk Lens";
"CFBundleDisplayName" = "Disk Lens";
ENGLISH
cat > "$app/Contents/Resources/ko.lproj/InfoPlist.strings" <<'KOREAN'
"CFBundleName" = "디스크 렌즈";
"CFBundleDisplayName" = "디스크 렌즈";
KOREAN
codesign --force --deep --sign - "$app"
printf '%s\n' "Built: $app"
