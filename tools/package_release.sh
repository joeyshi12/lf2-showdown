#!/bin/sh
# Make a download archive: a folder named after the archive with the program and the
# game files from game/ next to it, so it runs after unpacking. lf2_image.bin is left
# out because the build puts it into the program. The format follows OUT's extension.
# usage: tools/package_release.sh PROGRAM OUT.zip|OUT.tar.gz
set -eu
prog="$1"; out="$2"
here=$(cd "$(dirname "$0")/.." && pwd)
case "$out" in
  *.zip) name=$(basename "$out" .zip) ;;
  *.tar.gz) name=$(basename "$out" .tar.gz) ;;
  *) echo "OUT must end in .zip or .tar.gz" >&2; exit 1 ;;
esac
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
mkdir "$tmp/$name"
cp -r "$here/game/." "$tmp/$name/"
rm -f "$tmp/$name/lf2_image.bin"
cp "$prog" "$tmp/$name/"
chmod +x "$tmp/$name/$(basename "$prog")"
out=$(cd "$(dirname "$out")" && pwd)/$(basename "$out")
rm -f "$out"
case "$out" in
  *.zip) (cd "$tmp" && zip -qr "$out" "$name") ;;
  *) tar -czf "$out" -C "$tmp" "$name" ;;
esac
echo "$out: $(du -h "$out" | cut -f1)"
