#!/usr/bin/env python3
"""Make the copy of the game that the Wine harness runs.

usage: prepare_game.py SRC_GAME_DIR DST_GAME_DIR

Two changes are needed for the original to run under this Wine, and both are kept away
from the simulation:

1. bgm/*.wma are replaced by a one-second silent AIFF. Wine's DirectShow goes through
   GStreamer, which here has no WMA (or WAV) decoder; RenderFile fails and the game shows
   a modal "Could not create a filter graph" box each time the music changes. AIFF is
   decoded by gst-plugins-bad's aiffparse, which is installed.

2. Each .dat file gets one extra encoded '\\n' at the end. Wine's msvcr80 `fscanf("%c")`
   reads one character ahead, so `feof` turns true one character early and the decoder
   loop in FUN_004148a0 / FUN_00414a30 drops the last byte of every file. For most files
   that byte is the '>' of the final `<frame_end>`, and the frame loop (which never checks
   feof) then spins forever. With a trailing newline the dropped byte is that newline.
   Windows would keep it; the only effect is one extra whitespace character at the end,
   which changes nothing the loaders store.
"""
import os, shutil, struct, sys

KEY = b"SiuHungIsAGoodBearBecauseHeIsVeryGood"


def silent_aiff(path, rate=22050, seconds=1):
    import math
    n = rate * seconds
    e = int(math.floor(math.log2(rate)))
    ext = struct.pack(">HQ", 16383 + e, int(rate / 2 ** e * 2 ** 63))
    comm = struct.pack(">hIh", 1, n, 16) + ext
    ssnd = struct.pack(">II", 0, 0) + b"\0\0" * n
    body = b"AIFF" + b"COMM" + struct.pack(">I", len(comm)) + comm + b"SSND" + struct.pack(">I", len(ssnd)) + ssnd
    with open(path, "wb") as f:
        f.write(b"FORM" + struct.pack(">I", len(body)) + body)


def main(src, dst):
    if os.path.exists(dst):
        shutil.rmtree(dst)
    shutil.copytree(src, dst)
    for root, _, files in os.walk(dst):
        for name in files:
            p = os.path.join(root, name)
            low = name.lower()
            if low.endswith(".wma"):
                silent_aiff(p)
            elif low.endswith(".dat"):
                raw = open(p, "rb").read()
                if len(raw) <= 123:
                    continue
                i = len(raw)
                open(p, "ab").write(bytes([(ord("\n") + KEY[i % len(KEY)]) & 0xFF]))
    print("prepared", dst)


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
