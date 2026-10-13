#!/bin/sh
# Build Switchboard.app end to end: Rust core -> XCFramework -> Xcode project -> dist/Switchboard.app
#
#   scripts/build-app.sh            # Debug build (includes File > Install Sample Project)
#   scripts/build-app.sh Release
set -eu
cd "$(dirname "$0")/.."
CONFIG=${1:-Debug}
if [ "$CONFIG" = "Release" ]; then scripts/build-core.sh; else scripts/build-core.sh --debug; fi
command -v xcodegen >/dev/null || { echo "xcodegen missing: brew install xcodegen"; exit 1; }
( cd app && xcodegen generate -q )
xcodebuild -project app/Switchboard.xcodeproj -scheme Switchboard -configuration "$CONFIG" \
  -destination 'platform=macOS' -derivedDataPath app/build/DerivedData \
  -skipPackagePluginValidation -skipMacroValidation -quiet build
rm -rf dist/Switchboard.app && mkdir -p dist
cp -R "app/build/DerivedData/Build/Products/$CONFIG/Switchboard.app" dist/
echo "==> dist/Switchboard.app ($CONFIG)"
