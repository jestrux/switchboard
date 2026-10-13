#!/bin/sh
# Build Switchboard, install it into /Applications and relaunch it, so you can iterate on
# the real app the way you would use it. Sessions are stopped by the quit, like any quit.
#
#   scripts/package.sh                 # Debug build (has File > Install Sample Project)
#   scripts/package.sh Release
#   scripts/package.sh --no-build      # just reinstall dist/Switchboard.app and relaunch
set -eu
cd "$(dirname "$0")/.."

CONFIG=Debug
BUILD=1
for a in "$@"; do
  case "$a" in
    Release|Debug) CONFIG=$a ;;
    --no-build) BUILD=0 ;;
    *) echo "usage: scripts/package.sh [Debug|Release] [--no-build]"; exit 2 ;;
  esac
done

if [ "$BUILD" = 1 ]; then
  scripts/build-app.sh "$CONFIG"
fi
[ -d dist/Switchboard.app ] || { echo "dist/Switchboard.app missing; run without --no-build"; exit 1; }

if pgrep -x Switchboard > /dev/null; then
  echo "==> quitting the running app (this stops its sessions)"
  osascript -e 'tell application "Switchboard" to quit' 2>/dev/null || true
  for _ in $(seq 1 100); do pgrep -x Switchboard > /dev/null || break; sleep 0.1; done
  pkill -x Switchboard 2>/dev/null || true
fi

rm -rf /Applications/Switchboard.app
cp -R dist/Switchboard.app /Applications/Switchboard.app
echo "==> installed /Applications/Switchboard.app ($CONFIG)"
open /Applications/Switchboard.app
