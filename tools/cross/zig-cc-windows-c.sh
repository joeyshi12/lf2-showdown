#!/bin/sh
# C compiler for the x86_64-pc-windows-gnu build of C dependencies (aws-lc-sys), from
# zig. cc-rs passes --target=x86_64-pc-windows-gnu, which zig does not parse; zig gets
# its own -target instead.
args=""
for a in "$@"; do
  case "$a" in
    --target=*) ;;
    *) args="$args '$(printf %s "$a" | sed "s/'/'\\\\''/g")'" ;;
  esac
done
eval exec python3 -m ziglang cc -target x86_64-windows-gnu $args
