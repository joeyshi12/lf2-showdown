#!/bin/sh
# Put the web build together: OUT/index.html, OUT/pkg (wasm-pack output) and, with
# GAME_DIR, a copy of the game folder in OUT/game with OUT/game/files.txt, so the
# page can be opened as index.html?game=game from a web server.
# usage: tools/web/package.sh PKG_DIR OUT [GAME_DIR]
set -e
pkg="$1"; out="$2"; game="$3"
here=$(cd "$(dirname "$0")/../.." && pwd)
mkdir -p "$out"
cp "$here/web/index.html" "$out/"
rm -rf "$out/pkg" && cp -r "$pkg" "$out/pkg"
if [ -n "$game" ]; then
  rm -rf "$out/game" && cp -r "$game" "$out/game"
  # the web build has the parts of lf2.exe it needs built in
  rm -f "$out/game/lf2.exe" "$out/game/lf2_image.bin" "$out/game/lf2"
  # browsers cannot play WMA: keep only the Ogg files of tools/convert_bgm.sh
  for f in "$out"/game/bgm/*.wma; do
    [ -f "${f%.wma}.ogg" ] && rm -f "$f"
  done
  (cd "$out/game" && find . -type f ! -name files.txt | sed 's|^\./||' | sort > files.txt)
fi
