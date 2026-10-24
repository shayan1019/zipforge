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
# Release artifacts drop the dot: 26.04 -> 7z2604-linux-x64.tar.xz
VERNUM="${VER//./}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DST="$ROOT/src-tauri/resources/7z"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

case "$(uname -s)-$(uname -m)" in
  Linux-x86_64)
    FILE="7z${VERNUM}-linux-x64.tar.xz"
    SHA="fc0327ba27e89bd086cf426dff17d77de582953cdbbc10a6576540a06853ffcd"
    SUB="linux-x64"
    ;;
  Darwin-*)
    # Universal: contains both x86_64 and arm64 slices.
    FILE="7z${VERNUM}-mac.tar.xz"
    SHA="bee04358cbcbc7106273cee0e8d72916db2696c48067a3538c34d8cd6fd16578"
    SUB="mac-universal"
    ;;
  *)
    echo "Unsupported platform: $(uname -s) $(uname -m)" >&2
    exit 1
    ;;
esac

# The upstream project publishes each release to both hosts. GitHub is tried
# first because 7-zip.org answers 404 to requests coming from cloud build IPs,
# which would otherwise break CI. Both serve byte-identical archives.
SOURCES=(
  "https://github.com/ip7z/7zip/releases/download/${VER}/${FILE}"
  "https://www.7-zip.org/a/${FILE}"
)

download() {
  local out="$1" url
  for url in "${SOURCES[@]}"; do
    echo "Downloading ${url}"
    if curl -fsSL --proto '=https' --retry 3 --retry-delay 2 --retry-connrefused --connect-timeout 20 "$url" -o "$out"; then
      return 0
    fi
    echo "  mirror unavailable, trying the next one" >&2
  done
  echo "All download mirrors failed for ${FILE}" >&2
  return 1
}

download "$TMP/${FILE}"

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

# Smoke check: write to a file instead of piping into `head`, because SIGPIPE
# plus `set -o pipefail` makes the script exit non-zero after a successful run.
"$DST/${SUB}/7zz" i >"$TMP/version.txt"
echo "7-Zip engine ready: src-tauri/resources/7z/${SUB}/7zz"
head -n 1 "$TMP/version.txt"
