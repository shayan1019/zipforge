#!/usr/bin/env bash
# Downloads the 7-Zip console engine (7zz) for the current OS and drops it into
# src-tauri/resources/7z so `npm run tauri build` can bundle it.
#
# Pinned to an exact release + SHA-256: the engine is a release artifact, so the
# tarball must never change underneath a build.
#
#   Linux x86_64  -> resources/7z/linux-x64/7zz
#   macOS (uni.)  -> resources/7z/mac-universal/7zz
set -euo pipefail

VER="26.04"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DST="$ROOT/src-tauri/resources/7z"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

case "$(uname -s)-$(uname -m)" in
  Linux-x86_64)
    FILE="7z${VER}-linux-x64.tar.xz"
    SHA="fc0327ba27e89bd086cf426dff17d77de582953cdbbc10a6576540a06853ffcd"
    SUB="linux-x64"
    ;;
  Darwin-*)
    # Universal: contains both x86_64 and arm64 slices.
    FILE="7z${VER}-mac.tar.xz"
    SHA="bee04358cbcbc7106273cee0e8d72916db2696c48067a3538c34d8cd6fd16578"
    SUB="mac-universal"
    ;;
  *)
    echo "Unsupported platform: $(uname -s) $(uname -m)" >&2
    exit 1
    ;;
esac

echo "Downloading 7-Zip ${VER} (${SUB})"
curl -fsSL "https://www.7-zip.org/a/${FILE}" -o "$TMP/${FILE}"

if command -v sha256sum >/dev/null 2>&1; then
  echo "${SHA}  ${TMP}/${FILE}" | sha256sum -c -
else
  echo "${SHA}  ${TMP}/${FILE}" | shasum -a 256 -c -
fi

mkdir -p "$TMP/out"
tar -xJf "$TMP/${FILE}" -C "$TMP/out"

install -d "$DST/${SUB}"
install -m 0755 "$TMP/out/7zz" "$DST/${SUB}/7zz"
cp "$TMP/out/License.txt" "$DST/License-posix.txt"

echo "7-Zip engine ready: src-tauri/resources/7z/${SUB}/7zz"
"$DST/${SUB}/7zz" i | head -n 3
