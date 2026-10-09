#!/bin/sh
# Convert the game's bgm (WMA) to Ogg Vorbis, which LF2 Showdown decodes itself on every
# platform (Windows, Linux, the browser). Needs ffmpeg with libvorbis.
# usage: tools/convert_bgm.sh GAME_DIR [OUT_DIR]   (OUT_DIR defaults to GAME_DIR/bgm)
set -e
src="$1/bgm"
out="${2:-$src}"
[ -d "$src" ] || { echo "no bgm folder in $1" >&2; exit 1; }
mkdir -p "$out"
for f in "$src"/*.wma "$src"/*.WMA; do
  [ -f "$f" ] || continue
  n=$(basename "$f"); n=${n%.*}
  # the sources are 64 kbps; quality 6 (about 190 kbps) keeps the second lossy
  # encoding inaudible
  ffmpeg -v error -y -i "$f" -c:a libvorbis -q:a 6 "$out/$n.ogg"
  echo "$out/$n.ogg"
done
