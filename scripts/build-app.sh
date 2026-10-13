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
# Sign with a real certificate when one is available, so the app keeps one identity across
# builds and macOS remembers its folder permissions. Falls back to ad-hoc signing (which
# makes macOS re-ask after every build). Override with SB_SIGN_IDENTITY / SB_TEAM_ID.
IDENTITY=${SB_SIGN_IDENTITY:-$(security find-identity -v -p codesigning 2>/dev/null | sed -n 's/.*"\(Apple Development: [^"]*\)".*/\1/p' | head -1)}
if [ -n "$IDENTITY" ]; then
  TEAM=${SB_TEAM_ID:-$(security find-certificate -c "$IDENTITY" -p 2>/dev/null | openssl x509 -noout -subject 2>/dev/null | sed -n 's/.*OU *= *\([A-Z0-9]\{10\}\).*/\1/p')}
  echo "==> signing as $IDENTITY${TEAM:+ (team $TEAM)}"
else
  echo "==> no signing certificate found; ad-hoc signing (macOS will re-ask for folder access after each build)"
fi
xcodebuild -project app/Switchboard.xcodeproj -scheme Switchboard -configuration "$CONFIG" \
  -destination 'platform=macOS' -derivedDataPath app/build/DerivedData \
  -skipPackagePluginValidation -skipMacroValidation -quiet build
rm -rf dist/Switchboard.app && mkdir -p dist
cp -R "app/build/DerivedData/Build/Products/$CONFIG/Switchboard.app" dist/
if [ -n "$IDENTITY" ]; then
  # Re-sign the finished bundle (nested code first) with the stable identity.
  find dist/Switchboard.app -name "*.dylib" -exec codesign --force --sign "$IDENTITY" --timestamp=none {} \;
  codesign --force --sign "$IDENTITY" --timestamp=none dist/Switchboard.app
fi
echo "==> dist/Switchboard.app ($CONFIG)"
