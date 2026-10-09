#!/bin/sh
# Build the Windows program from Linux: tools/cross/build-windows.sh [cargo args]
# Needs the x86_64-pc-windows-gnu target (rustup) and zig (pip install ziglang),
# which links, compiles the C of aws-lc-sys and provides dlltool.
here=$(cd "$(dirname "$0")" && pwd)
export PATH="$here:$PATH"
export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER="$here/zig-cc-windows.sh"
export CC_x86_64_pc_windows_gnu="$here/zig-cc-windows-c.sh"
export AR_x86_64_pc_windows_gnu="$here/zig-ar.sh"
exec cargo build --release -p lf2-app --target x86_64-pc-windows-gnu "$@"
