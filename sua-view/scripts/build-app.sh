#!/bin/zsh
set -euo pipefail
cd "${0:A:h}/.."

if [[ "$(uname -s)" != Darwin ]]; then
  print -u2 'The native app requires macOS and Xcode Command Line Tools.'
  exit 1
fi

cargo build --release --locked
app="$PWD/dist/Sua View.app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources" local/AppIcon.iconset
xcrun swift scripts/make-icon.swift "$PWD/local/AppIcon.iconset"
iconutil -c icns local/AppIcon.iconset -o "$app/Contents/Resources/AppIcon.icns"
cp native/Info.plist "$app/Contents/Info.plist"
cp assets/ReaderTheme.json "$app/Contents/Resources/ReaderTheme.json"
arch="$(uname -m)"
[[ "$arch" == arm64 || "$arch" == x86_64 ]]
xcrun swiftc -swift-version 5 -O -whole-module-optimization \
  -target "$arch-apple-macosx12.0" \
  -import-objc-header include/sua_view.h \
  native/SuaView.swift target/release/libsua_view_core.a \
  -framework AppKit -framework ImageIO -framework UniformTypeIdentifiers \
  -framework QuartzCore -larchive \
  -o "$app/Contents/MacOS/SuaView"
xcrun strip -x "$app/Contents/MacOS/SuaView"
codesign --force --sign - "$app"
codesign --verify --deep --strict "$app"
plutil -lint "$app/Contents/Info.plist"
print "Built $app"
du -sh "$app"
