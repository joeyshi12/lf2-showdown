#!/usr/bin/env python3
"""Pack a recorded trace (version 1, see snapshot.py) into the version 2 format kept in
the repository.

usage: pack_trace.py IN.lf2t OUT.lf2t     (LF2_DIR must point at the game folder)

Version 2 differs in two ways:
- The globals of the first frame are XORed with lf2.exe's initialised .data section
  (file offset 0x4d000, 0x2000 bytes, loaded at 0x44d000), so the file holds only
  what changed at run time and none of the executable's own bytes. Readers XOR again.
- Later frames store only the 4-byte words that changed, as (index, value) pairs over
  the state space globals ++ object 0 ++ ... ++ object 399.
Version 3 adds the harness's pokes (see snapshot.py) to every frame, after the keys:
    u32 n, n * (i32 object, u32 offset, u32 len, bytes).

Layout (little-endian, whole file zlib-compressed):
    b"LF2T" u32 2, u32 frames, u32 globals_base, u32 globals_len
    50 bytes: valid bitmap of the 400 objects (constant over the trace)
    frame 0: 256 key bytes, globals (XORed), then each valid object's 0x420 bytes
    frames 1..: 256 key bytes, u32 n, n * (u32 word_index, u32 value)
"""
import os, struct, sys, zlib

DATA_FILE_OFF = 0x4D000
DATA_RAW_LEN = 0x2000


def exe_data():
    exe = open(os.path.join(os.environ["LF2_DIR"], "lf2.exe"), "rb").read()
    return exe[DATA_FILE_OFF:DATA_FILE_OFF + DATA_RAW_LEN]


def read_v1(path):
    b = zlib.decompress(open(path, "rb").read())
    assert b[:4] == b"LF2T" and struct.unpack_from("<I", b, 4)[0] == 1
    n = struct.unpack_from("<I", b, 8)[0]
    o = 12
    frames = []
    for _ in range(n):
        kl = struct.unpack_from("<I", b, o)[0]
        o += 4
        keys = b[o:o + kl]
        o += kl
        pokes = b""
        if b[o:o + 4] != b"LF2S":
            np_ = struct.unpack_from("<I", b, o)[0]
            start = o
            o += 4
            for _ in range(np_):
                _, _, ln = struct.unpack_from("<iII", b, o)
                o += 12 + ln
            pokes = b[start:o]
        assert b[o:o + 4] == b"LF2S"
        o += 8
        gb, gl = struct.unpack_from("<II", b, o)
        o += 8
        glob = b[o:o + gl]
        o += gl
        cnt = struct.unpack_from("<I", b, o)[0]
        o += 4
        objs = []
        for _ in range(cnt):
            v = b[o]
            o += 1
            if v:
                objs.append(b[o:o + 0x420])
                o += 0x420
            else:
                objs.append(None)
        frames.append((keys, gb, glob, objs, pokes or struct.pack("<I", 0)))
    return frames


def main(src, dst):
    frames = read_v1(src)
    keys0, gb, glob0, objs0, pokes0 = frames[0]
    valid = [o is not None for o in objs0]
    for _, _, _, objs, _ in frames:
        assert [o is not None for o in objs] == valid, "object slots changed during the trace"
    bitmap = bytearray(50)
    for i, v in enumerate(valid):
        if v:
            bitmap[i // 8] |= 1 << (i % 8)
    ed = exe_data()
    x = bytes(a ^ (ed[i] if i < len(ed) else 0) for i, a in enumerate(glob0))
    out = bytearray(b"LF2T" + struct.pack("<IIII", 3, len(frames), gb, len(glob0)))
    out += bitmap
    out += keys0 + pokes0 + x + b"".join(o for o in objs0 if o is not None)

    def space(glob, objs):
        return glob + b"".join(o for o in objs if o is not None)

    prev = space(glob0, objs0)
    for keys, _, glob, objs, pokes in frames[1:]:
        cur = space(glob, objs)
        changes = [(i // 4, cur[i:i + 4]) for i in range(0, len(cur), 4) if cur[i:i + 4] != prev[i:i + 4]]
        out += keys + pokes + struct.pack("<I", len(changes))
        for w, v in changes:
            out += struct.pack("<I", w) + v
        prev = cur
    open(dst, "wb").write(zlib.compress(bytes(out), 9))
    print(f"{dst}: {len(frames)} frames, {os.path.getsize(dst)} bytes")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
