#!/bin/sh
# Cut a Switchboard release: bump the version, build a universal Release build, sign it
# with Developer ID, have Apple notarize it, staple and zip it, and optionally publish it.
#
#   scripts/release.sh patch --publish     # 0.1.0 -> 0.1.1, build, commit, push, GitHub release
#   scripts/release.sh minor --publish     # 0.1.1 -> 0.2.0
#   scripts/release.sh major --publish     # 0.2.0 -> 1.0.0
#   scripts/release.sh 1.4.0 --publish     # an exact version
#   scripts/release.sh patch               # bump and build dist/Switchboard-<version>.zip only
#   scripts/release.sh                     # rebuild the current version's zip
#   scripts/release.sh minor --dry-run     # print what would happen and stop
#
# A bump rewrites the version in app/project.yml (and its build number) and in Cargo.toml.
# With --publish the bump is committed as "Release <version>" and pushed once the build is
# notarized, and the GitHub release is created on that commit with generated notes. Without
# --publish the bumped files are left uncommitted for you to review.
#
# Signing and notarization use the developer account Xcode is signed in to
# (Xcode › Settings › Accounts), so no certificate has to be installed and no password is
# stored. Apple creates and holds the Developer ID certificate. If a step fails with
# "PLA Update available" or "A required agreement is missing", accept the pending
# agreement at developer.apple.com/account and run it again a minute later.
set -eu
cd "$(dirname "$0")/.."
export PATH="$HOME/.cargo/bin:$PATH"

BUMP=""
PUBLISH=0
DRY=0
for a in "$@"; do
  case "$a" in
    --publish) PUBLISH=1 ;;
    --dry-run) DRY=1 ;;
    patch|minor|major) BUMP=$a ;;
    [0-9]*.[0-9]*.[0-9]*) BUMP=$a ;;
    *) echo "usage: scripts/release.sh [patch|minor|major|X.Y.Z] [--publish] [--dry-run]"; exit 2 ;;
  esac
done

CURRENT=$(sed -n 's/.*CFBundleShortVersionString: *"\(.*\)".*/\1/p' app/project.yml)
BUILD=$(sed -n 's/.*CFBundleVersion: *"\(.*\)".*/\1/p' app/project.yml)
MAJOR=${CURRENT%%.*}; REST=${CURRENT#*.}; MINOR=${REST%%.*}; PATCH=${REST#*.}
case "$BUMP" in
  "") VERSION=$CURRENT ;;
  patch) VERSION=$MAJOR.$MINOR.$((PATCH + 1)) ;;
  minor) VERSION=$MAJOR.$((MINOR + 1)).0 ;;
  major) VERSION=$((MAJOR + 1)).0.0 ;;
  *) VERSION=$BUMP ;;
esac
NEXT_BUILD=$BUILD
[ "$VERSION" = "$CURRENT" ] || NEXT_BUILD=$((BUILD + 1))

TEAM=${SB_TEAM_ID:-$(security find-certificate -c "Apple Development" -p 2>/dev/null | openssl x509 -noout -subject 2>/dev/null | sed -n 's/.*OU *= *\([A-Z0-9]\{10\}\).*/\1/p')}
[ -n "$TEAM" ] || { echo "No team ID found. Set SB_TEAM_ID."; exit 1; }

if [ "$PUBLISH" = 1 ] || [ "$VERSION" != "$CURRENT" ]; then
  # The release commit should hold the version bump and nothing else.
  [ -z "$(git status --porcelain)" ] || { echo "Commit or stash your changes first; the tree is not clean."; exit 1; }
fi
if [ "$PUBLISH" = 1 ]; then
  if git rev-parse -q --verify "refs/tags/v$VERSION" > /dev/null || git ls-remote --exit-code --tags origin "v$VERSION" > /dev/null 2>&1; then
    echo "v$VERSION is already released. Pass patch, minor, major or a new version."; exit 1
  fi
fi

echo "==> Switchboard $CURRENT (build $BUILD) -> $VERSION (build $NEXT_BUILD), team $TEAM$([ "$PUBLISH" = 1 ] && echo ", will publish")"
[ "$DRY" = 0 ] || exit 0

if [ "$VERSION" != "$CURRENT" ]; then
  # If anything fails from here on, put the version files back, so a rerun starts clean.
  trap 'status=$?; [ "$status" = 0 ] || { git checkout -q -- app/project.yml Cargo.toml Cargo.lock; echo "==> failed; version files restored"; }' EXIT
  sed -i '' "s/\(CFBundleShortVersionString: *\)\"$CURRENT\"/\1\"$VERSION\"/; s/\(CFBundleVersion: *\)\"$BUILD\"/\1\"$NEXT_BUILD\"/" app/project.yml
  # The first `version = ` in Cargo.toml is the workspace's; the crates inherit it.
  sed -i '' "1,/^version = \"$CURRENT\"/s/^version = \"$CURRENT\"/version = \"$VERSION\"/" Cargo.toml
  grep -q "CFBundleShortVersionString: \"$VERSION\"" app/project.yml && grep -q "^version = \"$VERSION\"" Cargo.toml \
    || { echo "Could not write the new version"; exit 1; }
fi

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
  if [ -n "$(git status --porcelain)" ]; then
    git add app/project.yml app/Switchboard/Info.plist Cargo.toml Cargo.lock
    # SB_RELEASE_TRAILERS: extra lines for the commit message, such as co-author trailers.
    git commit -q -m "Release $VERSION${SB_RELEASE_TRAILERS:+

$SB_RELEASE_TRAILERS}"
  fi
  git push -q origin HEAD
  echo "==> GitHub release v$VERSION"
  gh release create "v$VERSION" "$ZIP" --target "$(git rev-parse HEAD)" --title "Switchboard $VERSION" --generate-notes \
    --notes "Download the zip, unzip it, and drag Switchboard to Applications. Requires macOS 14 or later. Signed and notarized."
elif [ "$VERSION" != "$CURRENT" ]; then
  echo "==> version files are bumped but not committed; run with --publish to release, or commit them yourself"
fi
