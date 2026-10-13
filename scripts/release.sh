#!/bin/sh
# Build a distributable Switchboard: universal Release build, signed with Developer ID,
# notarized by Apple and stapled, then zipped.
#
#   scripts/release.sh              # -> dist/Switchboard-<version>.zip
#   scripts/release.sh --publish    # also create the GitHub release v<version> with the zip
#
# Signing and notarization use the developer account Xcode is signed in to
# (Xcode › Settings › Accounts), so no certificate has to be installed and no password is
# stored. Apple creates and holds the Developer ID certificate. If a step fails with
# "PLA Update available" or "A required agreement is missing", accept the pending
# agreement at developer.apple.com/account and run it again a minute later.
#
# The version comes from CFBundleShortVersionString in app/project.yml.
set -eu
cd "$(dirname "$0")/.."
PUBLISH=0
[ "${1:-}" = "--publish" ] && PUBLISH=1

VERSION=$(sed -n 's/.*CFBundleShortVersionString: *"\(.*\)".*/\1/p' app/project.yml)
TEAM=${SB_TEAM_ID:-$(security find-certificate -c "Apple Development" -p 2>/dev/null | openssl x509 -noout -subject 2>/dev/null | sed -n 's/.*OU *= *\([A-Z0-9]\{10\}\).*/\1/p')}
[ -n "$TEAM" ] || { echo "No team ID found. Set SB_TEAM_ID."; exit 1; }
WORK=target/release-app
ARCHIVE=$WORK/Switchboard.xcarchive
ZIP=dist/Switchboard-$VERSION.zip
echo "==> Switchboard $VERSION, team $TEAM"

scripts/build-core.sh
command -v xcodegen >/dev/null || { echo "xcodegen missing: brew install xcodegen"; exit 1; }
( cd app && xcodegen generate -q )
rm -rf "$WORK" && mkdir -p "$WORK" dist

echo "==> archive (Release, hardened runtime)"
xcodebuild -project app/Switchboard.xcodeproj -scheme Switchboard -configuration Release \
  -destination 'generic/platform=macOS' -derivedDataPath app/build/DerivedData \
  -skipPackagePluginValidation -skipMacroValidation \
  -archivePath "$ARCHIVE" ENABLE_HARDENED_RUNTIME=YES -quiet archive

cat > "$WORK/upload.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>method</key><string>developer-id</string>
  <key>teamID</key><string>$TEAM</string>
  <key>signingStyle</key><string>automatic</string>
  <key>destination</key><string>upload</string>
</dict></plist>
PLIST

echo "==> sign with Developer ID and submit for notarization"
xcodebuild -exportArchive -archivePath "$ARCHIVE" -exportOptionsPlist "$WORK/upload.plist" \
  -exportPath "$WORK/upload" -allowProvisioningUpdates > "$WORK/upload.log" 2>&1 \
  || { grep "error:" "$WORK/upload.log"; exit 1; }

echo "==> waiting for Apple's notary service"
i=0
until xcodebuild -exportNotarizedApp -archivePath "$ARCHIVE" -exportPath "$WORK/notarized" > "$WORK/notarized.log" 2>&1; do
  i=$((i + 1))
  [ "$i" -lt 40 ] || { echo "Not notarized after 20 minutes:"; grep "error:" "$WORK/notarized.log"; exit 1; }
  rm -rf "$WORK/notarized"
  sleep 30
done

APP=$WORK/notarized/Switchboard.app
xcrun stapler validate "$APP" > /dev/null
spctl -a -vv "$APP" 2>&1 | grep -q "Notarized Developer ID" || { echo "Gatekeeper does not accept the build"; exit 1; }
rm -f "$ZIP"
ditto -c -k --keepParent "$APP" "$ZIP"
echo "==> $ZIP ($(du -h "$ZIP" | cut -f1 | tr -d ' ')), notarized and stapled"

if [ "$PUBLISH" = 1 ]; then
  echo "==> GitHub release v$VERSION"
  gh release create "v$VERSION" "$ZIP" --title "Switchboard $VERSION" \
    --notes "Download the zip, unzip it, and drag Switchboard to Applications. Requires macOS 14 or later. Signed and notarized."
fi
