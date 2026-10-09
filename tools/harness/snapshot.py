"""Snapshots of the original's simulation state, in the format lf2-sim reads.

A snapshot holds the global range 0x44d000..0x45a000 (.data and .bss) and the 400
object images (0x420 bytes each). Object field 0x368 (pointer to ObjectData) is
replaced by the object's index in the file manager, or -1.

Binary layout (little-endian):
    b"LF2S" u32 version=1
    u32 globals_base, u32 globals_len, globals bytes
    u32 object_count (400), then per object: u8 valid, 0x420 bytes if valid

A trace is a sequence of frames:
    b"LF2T" u32 version=1, u32 frame_count
    per frame: u32 keys_len, key array bytes (the 256-byte 'd'/'u' array as it was
    when the frame started), u32 poke_count, pokes, then a snapshot taken at the start
    of that frame (after the pokes).
A poke is a write the harness made to the game's memory before the frame:
    i32 object (-1 for a global address), u32 offset (or address), u32 len, bytes.
The file is zlib-compressed as a whole.
"""
import struct, zlib

GLOBALS_BASE = 0x44D000
GLOBALS_LEN = 0xD000
GAME = 0x458B00
OBJ_PTRS = GAME + 0x194
MANAGER = GAME + 0x7D4
OBJ_SIZE = 0x420
KEYS = 0x455378


def take(g):
    glob = g.read(GLOBALS_BASE, GLOBALS_LEN)
    ptrs = struct.unpack_from("<400I", glob, OBJ_PTRS - GLOBALS_BASE)
    mgr = struct.unpack_from("<I", glob, MANAGER - GLOBALS_BASE)[0]
    data_ptrs = {}
    if mgr:
        n = g.i32(mgr + 0x4D82380)
        for i, p in enumerate(struct.unpack("<%dI" % n, g.read(mgr, 4 * n))):
            data_ptrs[p] = i
    valid = [p != 0 for p in ptrs]
    blob = g.readmany([(p, OBJ_SIZE) for p in ptrs if p])
    out = bytearray(b"LF2S" + struct.pack("<I", 1))
    out += struct.pack("<II", GLOBALS_BASE, GLOBALS_LEN) + glob
    out += struct.pack("<I", 400)
    k = 0
    for v in valid:
        out.append(1 if v else 0)
        if v:
            img = bytearray(blob[k * OBJ_SIZE:(k + 1) * OBJ_SIZE])
            k += 1
            dp = struct.unpack_from("<I", img, 0x368)[0]
            struct.pack_into("<i", img, 0x368, data_ptrs.get(dp, -1))
            out += img
    return bytes(out)


class TraceWriter:
    def __init__(self, path):
        self.path = path
        self.frames = []

    def add(self, keys, snap, pokes=()):
        p = struct.pack("<I", len(pokes)) + b"".join(
            struct.pack("<iII", obj, off, len(data)) + data for obj, off, data in pokes)
        self.frames.append(struct.pack("<I", len(keys)) + keys + p + snap)

    def close(self):
        body = b"LF2T" + struct.pack("<II", 1, len(self.frames)) + b"".join(self.frames)
        with open(self.path, "wb") as f:
            f.write(zlib.compress(body, 6))
