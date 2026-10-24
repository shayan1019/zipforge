#!/usr/bin/env bash
# Verifies that the 7-Zip engine shipped inside the built packages actually runs
# on this OS. The bundles are produced by `tauri build`, so this script only
# inspects the artefacts on disk.
#
#   bash scripts/smoke-engine.sh <target-dir> [marker-file]
#
# <target-dir>   src-tauri/target (contains the cached previous builds too)
# [marker-file]  only artefacts newer than this file count as "just built".
#                rust-cache restores older bundles into target/, so a plain
#                glob would happily test a package from a previous release.
#
# Run it by hand after `npm run tauri build`:
#   bash scripts/smoke-engine.sh src-tauri/target
set -euxo pipefail

ROOT="${1:-src-tauri/target}"
MARKER="${2:-}"

if [ ! -d "$ROOT" ]; then
  echo "no such directory: $ROOT" >&2
  exit 1
fi

# `find | head` would die of SIGPIPE under pipefail, and `ls <glob>` aborts the
# script when a cached build left more than one match behind. `-print -quit`
# stops find itself, so nothing is killed and nothing is ambiguous.
find_one() {
  if [ -n "$MARKER" ] && [ -e "$MARKER" ]; then
    find "$ROOT" "$@" -newer "$MARKER" -print -quit
  else
    find "$ROOT" "$@" -print -quit
  fi
}

engine_smoke() {
  local engine="$1"
  local label="$2"
  local out

  if [ ! -f "$engine" ]; then
    echo "engine missing from the package: $engine" >&2
    exit 1
  fi
  ls -l "$engine"
  if [ ! -x "$engine" ]; then
    echo "engine is not executable inside the package: $engine" >&2
    exit 1
  fi
  out="$("$engine" i | sed -n 1p)"
  echo "$out"
  printf '%s\n' "$out" | grep -c "7-Zip 26.04" >/dev/null
  echo "$label engine ok"
}

# A real archive round-trip proves the binary is not merely able to print its
# banner: it has to allocate, compress and write.
round_trip() {
  local engine="$1"
  local work="$2"

  mkdir -p "$work/data"
  printf 'zipforge ci\n' > "$work/data/probe.txt"
  "$engine" a -t7z "$work/probe.7z" "$work/data/probe.txt" >/dev/null
  "$engine" x -y "$work/probe.7z" "-o$work/out" >/dev/null
  diff "$work/data/probe.txt" "$work/out/probe.txt"
  echo "round trip ok"
}

WORK="$(mktemp -d)"
echo "workdir: $WORK  root: $ROOT  marker: ${MARKER:-<none>}"

case "$(uname -s)" in
  Linux)
    # 1. the .deb keeps the engine's exec bit through dpkg packaging
    DEB="$(find_one -type f -name '*.deb')"
    echo "deb: ${DEB:-<none>}"
    test -n "$DEB"
    dpkg-deb -x "$DEB" "$WORK/deb"
    engine_smoke "$WORK/deb/usr/lib/zipforge/resources/7z/7zz" "deb"

    # 2. the AppImage engine survives linuxdeploy/patchelf rewriting: the
    #    bundler patches every ELF in usr/lib, so this is the risky copy
    APPIMAGE="$(find_one -type f -name '*.AppImage')"
    echo "appimage: ${APPIMAGE:-<none>}"
    test -n "$APPIMAGE"
    # find returns a relative path, and the extraction happens from $WORK
    APPIMAGE="$(cd "$(dirname "$APPIMAGE")" && pwd)/$(basename "$APPIMAGE")"
    chmod +x "$APPIMAGE"
    ( cd "$WORK" && "$APPIMAGE" --appimage-extract >/dev/null )
    engine_smoke "$WORK/squashfs-root/usr/lib/zipforge/resources/7z/7zz" "appimage"
    round_trip "$WORK/squashfs-root/usr/lib/zipforge/resources/7z/7zz" "$WORK/ai"
    ;;

  Darwin)
    # --target universal-apple-darwin puts the bundle under
    # target/universal-apple-darwin/release, so never hardcode the path.
    APP="$(find_one -maxdepth 6 -type d -name '*.app')"
    echo "app: ${APP:-<none>}"
    test -n "$APP"
    BUNDLED="$APP/Contents/Resources/resources/7z/7zz"
    ls -l "$BUNDLED"
    # the resource keeps the upstream signature, so just prove it executes
    engine_smoke "$BUNDLED" "app"
    round_trip "$BUNDLED" "$WORK/app"
    ;;

  *)
    echo "unsupported host: $(uname -s)" >&2
    exit 1
    ;;
esac

echo "all bundled engines verified"
