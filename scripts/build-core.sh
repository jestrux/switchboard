#!/bin/sh
# Build the Rust engine as an XCFramework plus generated Swift bindings for the app.
#
#   scripts/build-core.sh            # release, universal (arm64 + x86_64 when installed)
#   scripts/build-core.sh --debug    # debug, host arch only (fast iteration)
#
# Outputs:
#   app/Frameworks/SwitchboardCore.xcframework
#   app/Generated/SwitchboardCore.swift
set -eu
cd "$(dirname "$0")/.."
export PATH="$HOME/.cargo/bin:$PATH"

PROFILE=release
CARGO_FLAGS="--release"
if [ "${1:-}" = "--debug" ]; then PROFILE=debug; CARGO_FLAGS=""; fi

HOST_TARGET=$(rustc -vV | sed -n 's/^host: //p')
TARGETS="$HOST_TARGET"
if [ "$PROFILE" = "release" ]; then
  for t in aarch64-apple-darwin x86_64-apple-darwin; do
    if [ "$t" != "$HOST_TARGET" ] && rustup target list --installed | grep -q "^$t$"; then
      TARGETS="$TARGETS $t"
    fi
  done
fi

LIBS=""
for t in $TARGETS; do
  echo "==> cargo build ($PROFILE, $t)"
  cargo build -p sb-ffi $CARGO_FLAGS --target "$t" --quiet
  LIBS="$LIBS target/$t/$PROFILE/libsb_ffi.a"
done
HOST_DYLIB="target/$HOST_TARGET/$PROFILE/libsb_ffi.dylib"

echo "==> uniffi-bindgen (swift)"
cargo build -p uniffi-bindgen --quiet
GEN=app/Generated
rm -rf "$GEN" && mkdir -p "$GEN"
target/debug/uniffi-bindgen generate --library "$HOST_DYLIB" --language swift --out-dir "$GEN" --no-format

echo "==> xcframework"
OUT=app/Frameworks
BUILD=target/xcframework
rm -rf "$BUILD" "$OUT/SwitchboardCore.xcframework"
mkdir -p "$BUILD/include" "$OUT"
cp "$GEN/SwitchboardCoreFFI.h" "$BUILD/include/"
cp "$GEN/SwitchboardCoreFFI.modulemap" "$BUILD/include/module.modulemap"
rm "$GEN/SwitchboardCoreFFI.h" "$GEN/SwitchboardCoreFFI.modulemap"
if [ "$(echo $LIBS | wc -w)" -gt 1 ]; then
  lipo -create $LIBS -output "$BUILD/libsb_ffi.a"
else
  cp $LIBS "$BUILD/libsb_ffi.a"
fi
xcodebuild -create-xcframework \
  -library "$BUILD/libsb_ffi.a" -headers "$BUILD/include" \
  -output "$OUT/SwitchboardCore.xcframework" > /dev/null
echo "==> done: $OUT/SwitchboardCore.xcframework, $GEN/SwitchboardCore.swift"
