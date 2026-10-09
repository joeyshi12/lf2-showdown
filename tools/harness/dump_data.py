#!/usr/bin/env python3
"""Dump the loaded data files from the running original for comparison with lf2-data.

usage: dump_data.py OUT_DIR

Writes OUT_DIR/obj_NNN.bin for each object in the file manager: the 0x25360-byte object
followed, for each frame in order, by its itr array (count*0x50) and bdy array
(count*0x28). Also writes manager.bin: the stage and background area of the manager.
Pointer fields are left as they are; the comparison ignores them.
"""
import os, sys, struct
sys.path.insert(0, os.path.dirname(__file__))
from lf2client import Game

GAME = 0x458B00
MANAGER_PTR = GAME + 0x7D4
OBJ_SIZE = 0x25360
FRAMES = 0x7A4
FRAME_SIZE = 0x178


def main(out):
    g = Game()
    os.makedirs(out, exist_ok=True)
    m = g.u32(MANAGER_PTR)
    count = g.i32(m + 0x4D82380)
    nbg = g.i32(m + 0x4D82384)
    print("manager", hex(m), "objects", count, "backgrounds", nbg)
    for i in range(count):
        p = g.u32(m + 4 * i)
        blob = bytearray(g.read(p, OBJ_SIZE))
        for f in range(400):
            fb = FRAMES + f * FRAME_SIZE
            ni, nb = struct.unpack_from("<ii", blob, fb + 0x128)
            pi, pb = struct.unpack_from("<II", blob, fb + 0x130)
            if ni > 0:
                blob += g.read(pi, ni * 0x50)
            if nb > 0:
                blob += g.read(pb, nb * 0x28)
        open(os.path.join(out, f"obj_{i:03}.bin"), "wb").write(blob)
    # stages: 60 * 0x149b08 at +0x7d0; backgrounds at +0x4d45db0, 0x990 each
    open(os.path.join(out, "stages.bin"), "wb").write(g.read(m + 0x7D0, 60 * 0x149B08))
    open(os.path.join(out, "backgrounds.bin"), "wb").write(g.read(m + 0x4D45DB0, nbg * 0x990))
    open(os.path.join(out, "globals.bin"), "wb").write(
        struct.pack("<ii", g.i32(0x44F620), g.i32(0x458438)) + g.read(0x455638, 0x14 * 300))


if __name__ == "__main__":
    main(sys.argv[1])
