#!/bin/sh
# M2 check: compile the generated Swift bindings against the XCFramework and run a
# tiny program that starts a command and receives its output and status events.
set -eu
cd "$(dirname "$0")/.."
FW=app/Frameworks/SwitchboardCore.xcframework/macos-arm64
[ -d "$FW" ] || FW=$(ls -d app/Frameworks/SwitchboardCore.xcframework/macos-* | head -1)
OUT=target/swift-smoke
rm -rf "$OUT" && mkdir -p "$OUT"
swiftc -O -module-name SwitchboardCore -emit-module -emit-module-path "$OUT/SwitchboardCore.swiftmodule" \
  -emit-library -o "$OUT/libSwitchboardCore.dylib" -I "$FW/Headers" app/Generated/SwitchboardCore.swift "$FW/libsb_ffi.a" 2>&1 | grep -v "^.*warning" || true
swiftc -o "$OUT/smoke" -I "$OUT" -I "$FW/Headers" crates/sb-ffi/swift-smoke/main.swift \
  -L "$OUT" -lSwitchboardCore -Xlinker -rpath -Xlinker "$PWD/$OUT" 2>&1 | grep -v "^.*warning" || true
"$OUT/smoke"
