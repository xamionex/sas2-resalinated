#!/usr/bin/env bash
set -euo pipefail

PROJECT="sas2-save-editor"
OUT_DIR="out"

WIN_TARGET="x86_64-pc-windows-gnu"
LINUX_TARGET="x86_64-unknown-linux-gnu.2.28"

WIN_BIN="target/$WIN_TARGET/release/${PROJECT}.exe"
LINUX_BIN="target/$LINUX_TARGET/release/${PROJECT}"

WIN_ZIP="$OUT_DIR/${PROJECT}-windows.zip"
LINUX_ZIP="$OUT_DIR/${PROJECT}-linux.zip"

cargo zigbuild --target "$WIN_TARGET" --release &
PID_WIN=$!

cargo zigbuild --target "$LINUX_TARGET" --release &
PID_LINUX=$!

wait $PID_WIN
wait $PID_LINUX

mkdir -p "$OUT_DIR"

zip -j -9 "$WIN_ZIP" "$WIN_BIN"
zip -j -9 "$LINUX_ZIP" "$LINUX_BIN"

echo "  - $WIN_ZIP"
echo "  - $LINUX_ZIP"
