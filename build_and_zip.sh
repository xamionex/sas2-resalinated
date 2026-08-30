#!/usr/bin/env bash
set -euo pipefail

PROJECT="sas2-resalinated"
OUT_DIR="out"

WIN_TARGET="x86_64-pc-windows-gnu"

WIN_BIN="target/$WIN_TARGET/release/${PROJECT}.exe"
LINUX_BIN="target/release/${PROJECT}"

WIN_ZIP="$OUT_DIR/${PROJECT}-windows.zip"
LINUX_ZIP="$OUT_DIR/${PROJECT}-linux.zip"

# Windows needs zigbuild for the cross-compile, the Linux binary is a native build
cargo zigbuild --target "$WIN_TARGET" --release &
PID_WIN=$!

cargo build --release &
PID_LINUX=$!

wait $PID_WIN
wait $PID_LINUX

strip "$LINUX_BIN"
x86_64-w64-mingw32-strip "$WIN_BIN"

mkdir -p "$OUT_DIR" staging/win staging/linux
cp "$WIN_BIN" "staging/win/"
cp "$LINUX_BIN" "staging/linux/"

(cd staging/win && 7z a -tzip -mx=9 -mpass=15 "../../$WIN_ZIP" *)
(cd staging/linux && 7z a -tzip -mx=9 -mpass=15 "../../$LINUX_ZIP" *)

rm -rf staging

echo "  - $WIN_ZIP"
echo "  - $LINUX_ZIP"
