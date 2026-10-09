#!/usr/bin/env python3
"""Record a trace of the original: start a VS match, play a scenario, and save the
key array and a snapshot at the start of every frame.

usage: record.py SCENARIO OUT.lf2t

Scenario file:
    p1 23            # fighter of player 1 (file-manager index, data.txt order)
    p2 22            # fighter of player 2
    rng zero         # optional: overwrite the RNG table and indices (see below)
    place p1 400 300 0   # optional: put player 1 at x=400, z=300 facing right (1: left)
    wait 5           # frames with no key held
    hold 20 p1.right p2.left    # frames with these keys held
    set p1 mp 500    # write a field of a player's object before the next frame
                     # (fields: mp, hp, x, z, facing, or a hex offset for an int)
    teamhp 5 1       # set the hp of every object of team 5 above 1 to 1
    gset 450bb8 1    # write an int global (hex address) before the next frame
    p2 none          # no second human
    com 5            # add a computer player with this fighter (repeatable)
    difficulty 2     # 0 easy, 1 normal, 2 difficult
    mode stage       # stage mode (stage 1-1) or battle mode (default armies), not VS;
                     # "mode menu" starts in the main menu instead of a match
    mouse 400 300    # move the mouse (client pixels), click: press the left button
    tap p1.down      # hold the keys 4 frames, then release for 6
"place" and "set" also take oN for object N (computer players are pick slot + 10).
Lines are run in order; '#' starts a comment.

`rng zero` sets the 3000-byte table to 1..255 repeating, both indices to 0 and the CRT
rand seed to 0 before the first recorded frame, so a trace does not depend on the seed
from timeGetTime.
"""
import os, sys, struct
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from lf2client import Game
from lockstep import Lockstep
from drive import P1, P2, KEYS
import snapshot
import start_vs

PLAYERS = {"p1": P1, "p2": P2}


def parse(path):
    cfg = {"p1": 23, "p2": 22, "rng": None, "steps": [], "place": [], "com": [], "difficulty": None, "mode": "vs"}

    def obj(name):
        return int(name[1:]) if name[0] == "o" else int(name[1:]) - 1

    for line in open(path):
        f = line.split("#")[0].split()
        if not f:
            continue
        if f[0] in ("p1", "p2"):
            cfg[f[0]] = None if f[1] == "none" else int(f[1])
        elif f[0] == "com":
            cfg["com"].append(int(f[1]))
        elif f[0] == "mode":
            cfg["mode"] = f[1]
        elif f[0] == "difficulty":
            cfg["difficulty"] = int(f[1])
        elif f[0] == "place":
            cfg["place"].append((obj(f[1]), float(f[2]), float(f[3]), int(f[4]) if len(f) > 4 else None))
        elif f[0] == "rng":
            cfg["rng"] = f[1]
        elif f[0] == "set":
            cfg["steps"].append(("set", obj(f[1]), f[2], f[3]))
        elif f[0] == "teamhp":
            cfg["steps"].append(("teamhp", int(f[1]), int(f[2])))
        elif f[0] == "mouse":
            cfg["steps"].append(("gset", 0x4546F0, int(f[1])))
            cfg["steps"].append(("gset", 0x453CDC, int(f[2])))
        elif f[0] == "click":
            # button down for two frames, then up (the window procedure 0x43b3d0
            # writes 1 on WM_LBUTTONDOWN and 0 on WM_LBUTTONUP)
            cfg["steps"].append(("gset", 0x457580, 1))
            cfg["steps"].append((2, []))
            cfg["steps"].append(("gset", 0x457580, 0))
        elif f[0] == "tap":
            # tap KEY... : held 4 frames, then 6 frames released
            cfg["steps"].append((4, f[1:]))
            cfg["steps"].append((6, []))
        elif f[0] == "gset":
            cfg["steps"].append(("gset", int(f[1], 16), int(f[2])))
        elif f[0] == "wait":
            cfg["steps"].append((int(f[1]), []))
        elif f[0] == "hold":
            cfg["steps"].append((int(f[1]), f[2:]))
        else:
            raise ValueError(line)
    return cfg


def main(scenario, out):
    cfg = parse(scenario)
    start_vs.main(cfg["p1"], cfg["p2"], cfg["com"], cfg["difficulty"], cfg["mode"])
    g = Game()
    L = Lockstep(g)
    if cfg["rng"] == "zero":
        g.write(0x44FF90, bytes((i % 255) + 1 for i in range(3000)))
        g.put_i32(0x450BCC, 0)
        g.put_i32(0x450C34, 0)
        g.put_i32(0x459F08, 0)  # CRT rand seed (moved here by lockstep.py)
    for slot, x, z, facing in cfg["place"]:
        o = g.u32(0x458B00 + 0x194 + 4 * slot)
        g.write(o + 0x58, struct.pack("<d", x))
        g.write(o + 0x68, struct.pack("<d", z))
        g.put_i32(o + 0x10, int(x))
        g.put_i32(o + 0x18, int(z))
        if facing is not None:
            g.put_u8(o + 0x80, facing)
    w = snapshot.TraceWriter(out)
    pending = []
    if cfg["mode"] == "stage":
        # the counter blocks of the current phase's spawn slots live in the file
        # manager, outside the snapshot; pass them as pokes of object -2 (offset =
        # slot, 43 ints: spawned, total, alive at once, 40 object indices)
        phase = g.i32(0x44FB6C)
        if phase >= 0:
            base = g.u32(0x458B00 + 0x7D4) + 0x810 + g.i32(0x450B94) * 0x149B08 + phase * 0x34C0
            for k in range(60):
                pending.append((-2, k, g.read(base + k * 0xE0, 43 * 4)))
    fields = {"mp": (0x308, "<i"), "hp": (0x2FC, "<i"), "x": (0x58, "<d"), "z": (0x68, "<d"), "facing": (0x80, "<B")}
    arr = bytearray(b"u" * 256)
    for step in cfg["steps"]:
        if step[0] == "teamhp":
            _, team, value = step
            data = struct.pack("<i", value)
            for slot in range(400):
                o = g.u32(0x458B00 + 0x194 + 4 * slot)
                if g.read(0x458B04 + slot, 1)[0] and g.i32(o + 0x364) == team and g.i32(o + 0x2FC) > value:
                    g.write(o + 0x2FC, data)
                    pending.append((slot, 0x2FC, data))
            continue
        if step[0] == "gset":
            _, addr, value = step
            data = struct.pack("<i", value)
            g.write(addr, data)
            pending.append((-1, addr, data))
            continue
        if step[0] == "set":
            _, slot, name, value = step
            off, fmt = fields[name] if name in fields else (int(name, 16), "<i")
            data = struct.pack(fmt, float(value) if fmt == "<d" else int(value))
            o = g.u32(0x458B00 + 0x194 + 4 * slot)
            g.write(o + off, data)
            pending.append((slot, off, data))
            continue
        n, keys = step
        arr = bytearray(b"u" * 256)
        for k in keys:
            who, name = k.split(".")
            # vk.0x7b: a virtual-key code directly (F-keys and the like)
            arr[int(name, 16) if who == "vk" else PLAYERS[who][name]] = ord("d")
        for _ in range(n):
            g.write(KEYS, arr)
            w.add(bytes(arr), snapshot.take(g), pending)
            pending = []
            L.step()
    w.add(bytes(arr), snapshot.take(g))
    w.close()
    cw = g.u32(0x459F0C) & 0xFFFF
    # precision control (bits 8-9) must be 2 (53-bit) as set by the CRT
    assert (cw >> 8) & 3 == 2, f"x87 control word {cw:#06x}"
    print("wrote", out, len(w.frames), "frames")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
