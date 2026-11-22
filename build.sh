#!/usr/bin/env bash
# Build ModlandSearch (GUI + CLI) and put the libopenmpt DLLs next to them,
# which is what the README tells you to do by hand.
#
# The libopenmpt DLLs are not in git: on the first run this fetches the pinned
# Windows dev package via scripts/fetch-libopenmpt.sh (cached in target/).
#
# Usage:
#   ./build.sh                                   host build (Windows, Git Bash/WSL)
#   TARGET=x86_64-pc-windows-gnu ./build.sh      cross-compile from Linux
#
# The crate is Windows-targeted (egui/glow plus Win32 resources), so a plain
# `cargo build` on Linux only works once a Windows target and mingw-w64 are
# installed; see "Building on Linux" in the README.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$HERE"

DLLS=(libopenmpt.dll openmpt-mpg123.dll openmpt-ogg.dll openmpt-vorbis.dll openmpt-zlib.dll)
DLL_SRC="$HERE/third_party/libopenmpt/bin/amd64"
TARGET="${TARGET:-}"

command -v cargo >/dev/null || {
  echo "error: cargo not found. Install Rust (https://rustup.rs) - this machine has no toolchain." >&2
  exit 1
}

if [ -n "$TARGET" ]; then
  echo "=== cargo build --release --target $TARGET"
  cargo build --release --target "$TARGET"
  OUT="$HERE/target/$TARGET/release"
else
  echo "=== cargo build --release"
  cargo build --release
  OUT="$HERE/target/release"
fi

if [ ! -f "$DLL_SRC/libopenmpt.dll" ]; then
  echo "=== libopenmpt DLLs missing, fetching them (not stored in git)"
  "$HERE/scripts/fetch-libopenmpt.sh"
fi

missing=0
for dll in "${DLLS[@]}"; do
  if [ ! -f "$DLL_SRC/$dll" ]; then
    echo "error: missing $DLL_SRC/$dll - remove third_party/libopenmpt/bin and run scripts/fetch-libopenmpt.sh again" >&2
    missing=1
  fi
done
[ "$missing" = 0 ] || exit 1

for dll in "${DLLS[@]}"; do
  cp -f "$DLL_SRC/$dll" "$OUT/$dll"
done

echo "=== done: $OUT/ModlandZoeker.exe, $OUT/modland-cli.exe (+ ${#DLLS[@]} libopenmpt DLLs)"
echo "    installer: powershell -ExecutionPolicy Bypass -File build-installer.ps1"
