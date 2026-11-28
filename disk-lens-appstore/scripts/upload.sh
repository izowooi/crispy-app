#!/bin/sh
# dist/DiskZ-<버전>.pkg 를 App Store Connect 에 올린다.
# 인증은 iOS 업로드 도구와 같은 설정 파일(ASC_KEY_PATH, ASC_KEY_ID, ASC_ISSUER_ID)을 쓴다.
set -eu
cd "$(dirname "$0")/.."
CONFIG="${TESTFLIGHT_CONFIG:-$HOME/git/.auth/testflight.env}"
[ -f "$CONFIG" ] || { echo "설정 파일이 없습니다: $CONFIG" >&2; exit 1; }
. "$CONFIG"
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
PKG="dist/DiskZ-$VERSION.pkg"
[ -f "$PKG" ] || { echo "$PKG 가 없습니다. 먼저 서명 빌드를 만드세요." >&2; exit 1; }
export API_PRIVATE_KEYS_DIR="$(dirname "$ASC_KEY_PATH")"
for step in --validate-app --upload-app; do
    xcrun altool "$step" -f "$PKG" -t macos --apiKey "$ASC_KEY_ID" --apiIssuer "$ASC_ISSUER_ID"
done
