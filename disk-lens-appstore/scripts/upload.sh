#!/bin/sh
# 서명된 macOS 설치 패키지(.pkg)를 App Store Connect 에 올린다(검증 후 업로드).
# 공용 도구 ~/git/tools/mac-appstore-upload/upload.sh 의 원본이다. 고치면 그쪽에 복사한다.
# 인증: ~/git/.auth/testflight.env 의 ASC_KEY_PATH, ASC_KEY_ID, ASC_ISSUER_ID (iOS 업로드와 같은 키).
set -eu
PKG="${1:-}"
[ -n "$PKG" ] || { echo "사용법: upload.sh PATH/TO/App.pkg [--validate-only]" >&2; exit 2; }
[ -f "$PKG" ] || { echo "파일이 없습니다: $PKG" >&2; exit 1; }
CONFIG="${TESTFLIGHT_CONFIG:-$HOME/git/.auth/testflight.env}"
if [ -f "$CONFIG" ]; then . "$CONFIG"; fi
: "${ASC_KEY_PATH:?ASC_KEY_PATH 가 없습니다 ($CONFIG)}" "${ASC_KEY_ID:?ASC_KEY_ID 가 없습니다}" "${ASC_ISSUER_ID:?ASC_ISSUER_ID 가 없습니다}"
[ -f "$ASC_KEY_PATH" ] || { echo "API 키 파일이 없습니다 (ASC_KEY_PATH)" >&2; exit 1; }
pkgutil --check-signature "$PKG" | grep -q "3rd Party Mac Developer Installer" \
    || { echo "Mac App Store 용 Installer 인증서로 서명된 pkg 가 아닙니다: $PKG" >&2; exit 1; }
export API_PRIVATE_KEYS_DIR="$(dirname "$ASC_KEY_PATH")"
STEPS="--validate-app --upload-app"
[ "${2:-}" = "--validate-only" ] && STEPS="--validate-app"
for step in $STEPS; do
    xcrun altool "$step" -f "$PKG" -t macos --apiKey "$ASC_KEY_ID" --apiIssuer "$ASC_ISSUER_ID"
done
