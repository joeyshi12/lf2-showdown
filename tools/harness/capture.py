#!/usr/bin/env python3
"""Capture the original's picture and state at the same frame, for comparing the
renderer with the original.

usage: capture.py SCENARIO OUT_PREFIX
Plays the scenario like record.py, then writes OUT_PREFIX.lf2t (a one-frame trace with
the state at the start of the last frame) and OUT_PREFIX.png (the window's client area
after the previous frame was drawn, which shows that state).
"""
import os, subprocess, sys, struct, time
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from lf2client import Game
from lockstep import Lockstep
from drive import KEYS
import record, snapshot, start_vs


def main(scenario, prefix):
    cfg = record.parse(scenario)
    start_vs.main(cfg["p1"], cfg["p2"], cfg["com"], cfg["difficulty"], cfg["mode"])
    g = Game()
    L = Lockstep(g)
    if cfg["rng"] == "zero":
        # as record.py
        g.write(0x44FF90, bytes((i % 255) + 1 for i in range(3000)))
        g.put_i32(0x450BCC, 0)
        g.put_i32(0x450C34, 0)
        g.put_i32(0x459F08, 0)
    for slot, x, z, facing in cfg["place"]:
        o = g.u32(0x458B00 + 0x194 + 4 * slot)
        g.write(o + 0x58, struct.pack("<d", x))
        g.write(o + 0x68, struct.pack("<d", z))
        if facing is not None:
            g.put_u8(o + 0x80, facing)
    for step in cfg["steps"]:
        if step[0] == "gset":
            g.put_i32(step[1], step[2])
            continue
        if step[0] == "set":
            _, slot, name, value = step
            fields = {"mp": (0x308, "<i"), "hp": (0x2FC, "<i"), "x": (0x58, "<d"), "z": (0x68, "<d"), "facing": (0x80, "<B")}
            off, fmt = fields[name] if name in fields else (int(name, 16), "<i")
            o = g.u32(0x458B00 + 0x194 + 4 * slot)
            g.write(o + off, struct.pack(fmt, float(value) if fmt == "<d" else int(value)))
            continue
        if step[0] == "teamhp":
            _, team, value = step
            for slot in range(400):
                o = g.u32(0x458B00 + 0x194 + 4 * slot)
                if g.read(0x458B04 + slot, 1)[0] and g.i32(o + 0x364) == team and g.i32(o + 0x2FC) > value:
                    g.write(o + 0x2FC, struct.pack("<i", value))
            continue
        n, keys = step
        arr = bytearray(b"u" * 256)
        for k in keys:
            who, name = k.split(".")
            arr[record.PLAYERS[who][name]] = ord("d")
        for _ in range(n):
            g.write(KEYS, arr)
            L.step()
    if cfg["mode"] == "menu":
        # menus draw while they update: save the state, run one frame, and the
        # picture shows that frame (compare_picture runs it too)
        w = snapshot.TraceWriter(prefix + ".lf2t")
        w.add(bytes(g.read(KEYS, 256)), snapshot.take(g))
        w.close()
        L.step()
    time.sleep(0.3)  # let the blit to the window finish
    disp = os.environ.get("LF2_DISPLAY", ":91")
    wid = subprocess.run(["xdotool", "search", "--name", "Little Fighter 2"], env=dict(os.environ, DISPLAY=disp),
                         capture_output=True, text=True).stdout.split()[0]
    subprocess.run(["import", "-display", disp, "-window", wid, prefix + ".png"], check=True)
    subprocess.run(["magick", prefix + ".png", prefix + ".ppm"], check=True)
    # background layer animation counters (file manager +0x4d465d4), drawing state only
    mgr = g.u32(0x458B00 + 0x7D4)
    bg = g.i32(0x44D024)
    n = g.i32(mgr + 0x4D45DCC + bg * 0x990)
    counters = [g.i32(mgr + 0x4D465D4 + (bg * 0x264 + i) * 4) for i in range(n)]
    open(prefix + ".layers", "w").write(" ".join(map(str, counters)))
    if cfg["mode"] != "menu":
        w = snapshot.TraceWriter(prefix + ".lf2t")
        w.add(bytes(g.read(KEYS, 256)), snapshot.take(g))
        w.close()
    print("wrote", prefix + ".png", prefix + ".lf2t")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
