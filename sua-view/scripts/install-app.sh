#!/bin/zsh
set -euo pipefail
cd "${0:A:h}/.."
source_app="$PWD/dist/Sua View.app"
[[ -x "$source_app/Contents/MacOS/SuaView" ]] || ./scripts/build-app.sh
destination="${1:-$HOME/Applications}/Sua View.app"
mkdir -p "${destination:h}"
# Keep a previous installation recoverable rather than deleting it.
if [[ -e "$destination" ]]; then
  backup="${destination}.previous-$(date +%Y%m%d-%H%M%S)"
  mv "$destination" "$backup"
fi
ditto "$source_app" "$destination"
/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister -f "$destination"
/System/Library/CoreServices/pbs -update
print "Installed $destination"
print 'Finder: select a comic file or folder, then press ⇧⌘J (Services → Sua View로 열기).'
print 'If macOS has not enabled the service: System Settings → Keyboard → Keyboard Shortcuts → Services → Sua View로 열기.'
