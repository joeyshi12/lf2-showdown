#!/bin/sh
# Linker for x86_64-pc-windows-gnu from Linux, using zig (pip install ziglang).
# Rust passes options zig's driver does not take: drop them, and give the import
# libraries of the winapi/windows crates (lib<name>.a in a -L directory) by path.
dirs=""
prev=""
for a in "$@"; do
  case "$a" in -L?*) dirs="$dirs ${a#-L}" ;; esac
  if [ "$prev" = "-L" ]; then dirs="$dirs $a"; fi
  prev="$a"
done
args=""
add() { args="$args '$(printf %s "$1" | sed "s/'/'\\\\''/g")'"; }
for a in "$@"; do
  case "$a" in
    -lgcc_eh) add -lunwind; continue ;;
    -lmsvcrt|-lgcc_s|-lgcc|-l:libpthread.a|-nodefaultlibs|-Wl,--disable-auto-image-base) continue ;;
    -l*)
      n=${a#-l}; found=""
      for d in $dirs; do
        if [ -f "$d/lib$n.a" ]; then found="$d/lib$n.a"; break; fi
      done
      if [ -n "$found" ]; then add "$found"; else add "$a"; fi ;;
    *) add "$a" ;;
  esac
done
eval exec python3 -m ziglang cc -target x86_64-windows-gnu $args
