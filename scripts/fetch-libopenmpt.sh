#!/usr/bin/env bash
# Fetch the libopenmpt Windows DLLs that ModlandSearch loads at runtime.
#
# The 35 DLLs (~20 MB) are deliberately not stored in git. This script downloads
# the official libopenmpt Windows dev package, pinned by version and SHA-256, and
# unpacks its bin/ tree into third_party/libopenmpt/ - the path build.sh and
# src/player.rs already expect. The download is cached in target/ so a second
# build works offline.
#
# Usage:
#   ./scripts/fetch-libopenmpt.sh
#   LIBOPENMPT_CACHE=/path/to/dir ./scripts/fetch-libopenmpt.sh   # reuse a downloaded zip
#   LIBOPENMPT_FORCE=1 ./scripts/fetch-libopenmpt.sh              # re-unpack anyway
set -euo pipefail

VERSION="0.8.10"
ARCHIVE="libopenmpt-${VERSION}+release.dev.windows.vs2022.zip"
URL="https://lib.openmpt.org/files/libopenmpt/dev/${ARCHIVE}"
SHA256="52481ca47d5386616adabd8048d0e7161f8abef9063afc9b241daca1e00251db"

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEST="$HERE/third_party/libopenmpt"
CACHE_DIR="${LIBOPENMPT_CACHE:-$HERE/target/libopenmpt-cache}"
CACHE="$CACHE_DIR/$ARCHIVE"
DLLS=(libopenmpt.dll openmpt-mpg123.dll openmpt-ogg.dll openmpt-vorbis.dll openmpt-zlib.dll)

have_dlls() {
  local dll
  for dll in "${DLLS[@]}"; do
    [ -f "$DEST/bin/amd64/$dll" ] || return 1
  done
}

if have_dlls && [ "${LIBOPENMPT_FORCE:-0}" != "1" ]; then
  echo "=== libopenmpt $VERSION DLLs already in place"
  exit 0
fi

mkdir -p "$CACHE_DIR"
if [ ! -f "$CACHE" ]; then
  echo "=== downloading libopenmpt $VERSION Windows dev package"
  if command -v curl >/dev/null 2>&1; then
    curl -fL --retry 3 -o "$CACHE.part" "$URL"
  elif command -v wget >/dev/null 2>&1; then
    wget -O "$CACHE.part" "$URL"
  else
    echo "error: need curl or wget to download $URL" >&2
    echo "       (or set LIBOPENMPT_CACHE to a folder holding $ARCHIVE)" >&2
    exit 1
  fi
  mv "$CACHE.part" "$CACHE"
fi

echo "=== verifying SHA-256"
if ! echo "$SHA256  $CACHE" | sha256sum -c - >/dev/null 2>&1; then
  echo "error: checksum mismatch for $CACHE" >&2
  echo "       expected $SHA256" >&2
  rm -f "$CACHE"
  exit 1
fi

echo "=== unpacking bin/* into third_party/libopenmpt"
if command -v unzip >/dev/null 2>&1 && unzip -q -o "$CACHE" 'bin/*' -d "$DEST" 2>/dev/null; then
  :
elif command -v python3 >/dev/null 2>&1; then
  python3 - "$CACHE" "$DEST" <<'PY'
import sys, zipfile
archive, dest = sys.argv[1], sys.argv[2]
with zipfile.ZipFile(archive) as z:
    for info in z.infolist():
        if info.filename.startswith("bin/") and not info.is_dir():
            z.extract(info, dest)
PY
elif command -v tar >/dev/null 2>&1 && tar -xf "$CACHE" -C "$DEST" bin 2>/dev/null; then
  :
else
  echo "error: no unzip, python3 or zip-capable tar found to unpack $CACHE" >&2
  exit 1
fi

if ! have_dlls; then
  echo "error: unpacking $ARCHIVE did not produce third_party/libopenmpt/bin/amd64/libopenmpt.dll" >&2
  exit 1
fi

echo "=== done: libopenmpt $VERSION DLLs in third_party/libopenmpt/bin/amd64"
