#!/usr/bin/env python3
"""Print the NUL-terminated strings (or N raw bytes with -n N) at virtual addresses of
lf2.exe. usage: rstr.py [-n N] ADDR..."""
import os, struct, sys
exe = open(os.path.join(os.environ.get("LF2_DIR", "."), "lf2.exe"), "rb").read()
pe = struct.unpack_from("<I", exe, 0x3c)[0]
nsec = struct.unpack_from("<H", exe, pe + 6)[0]
opt = struct.unpack_from("<H", exe, pe + 20)[0]
secs = []
for i in range(nsec):
    o = pe + 24 + opt + i * 40
    secs.append((struct.unpack_from("<I", exe, o + 12)[0], struct.unpack_from("<I", exe, o + 8)[0],
                 struct.unpack_from("<I", exe, o + 20)[0]))


def rd(a, k):
    a -= 0x400000
    for va, sz, raw in secs:
        if va <= a < va + sz:
            return exe[raw + a - va:raw + a - va + k]


args = sys.argv[1:]
n = None
if args and args[0] == "-n":
    n = int(args[1]); args = args[2:]
for a in args:
    b = rd(int(a, 16), n or 200)
    if n is None:
        b = b[:b.index(0)]
    print(a, repr(b))
