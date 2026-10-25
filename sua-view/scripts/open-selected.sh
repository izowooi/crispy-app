#!/bin/zsh
set -euo pipefail
# Suitable for Raycast/Alfred/Shortcuts: pass selected Finder paths as arguments.
app="${SUA_VIEW_APP:-$HOME/Applications/Sua View.app}"
if [[ ! -d "$app" ]]; then
  app="${0:A:h}/../dist/Sua View.app"
fi
if [[ ! -d "$app" ]]; then
  print -u2 'Build Sua View first with scripts/build-app.sh.'
  exit 1
fi
open -a "$app" "$@"
