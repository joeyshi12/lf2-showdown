#!/usr/bin/env python3
"""Start the original (through lf2d.py) and get to a VS match between player 1 and
player 2, both human, with the fighters given as file-manager indices (the order of
data.txt). Leaves the game paused in lockstep at the first frame of the match.

usage: start_vs.py P1_INDEX P2_INDEX [COMPUTER_INDEX...]

P2_INDEX may be "none" for a single human. Computer players take the pick-screen
slots after the humans and play as objects slot + 10.
"""
import os, subprocess, sys, time
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from lf2client import Game
from lockstep import Lockstep
from drive import Driver, P1, P2, MOUSE_X, MOUSE_Y, MOUSE_CLICK, set_controls

MATCH_STATE = 0x44D020   # 0 in a match, 1 on the pick screen, 10 in the main menu
PICK = 0x451248          # int[8] chosen file index per pick-screen slot, -1 random


def wait_for(cond, timeout=60):
    t = time.time()
    while not cond():
        if time.time() - t > timeout:
            raise TimeoutError
        time.sleep(0.2)


DIFFICULTY = 0x450C30


def main(p1, p2, coms=(), difficulty=None, mode="vs"):
    here = os.path.dirname(os.path.abspath(__file__))
    subprocess.run([sys.executable, os.path.join(here, "lf2d.py"), "stop"], capture_output=True)
    time.sleep(1)
    subprocess.run([sys.executable, os.path.join(here, "lf2d.py"), "start"], check=True)
    time.sleep(3)
    g = Game()
    L = Lockstep(g)
    L.install()
    L.pause()
    d = Driver(g, L)
    set_controls(g, 2, P2)
    if mode == "title":
        d.frames(3)
        print("at the title screen; frames entered:", L.entered())
        return
    # main menu "game start" (loads all data in one long frame)
    g.put_i32(MOUSE_X, 0x150)
    g.put_i32(MOUSE_Y, g.i32(0x453DA4) + 0xE0)
    g.put_i32(MOUSE_CLICK, 1)
    L.free_run()
    wait_for(lambda: g.u32(0x458B00 + 0x7D4) != 0 and g.i32(MATCH_STATE) == 10)
    time.sleep(1)
    L.pause()
    g.put_i32(MOUSE_X, 700)
    g.put_i32(MOUSE_Y, 500)
    if mode == "menu":
        d.frames(2)
        print("in the main menu; frames entered:", L.entered())
        return
    for _ in range({"vs": 0, "stage": 1, "battle": 4}[mode]):
        d.tap(P1["down"], 4, 10)
    d.tap(P1["attack"], 4, 10)            # VS mode or stage mode
    assert g.i32(MATCH_STATE) == 1
    humans = [(P1, 0, p1)] + ([(P2, 1, p2)] if p2 is not None else [])

    def choose(slot, idx):
        # the pick screen keeps the choice in the slot's object: objects[slot]->data
        g.put_i32(PICK + 4 * slot, idx)
        mgr = g.u32(0x458B00 + 0x7D4)
        g.write(g.u32(0x458C94 + 4 * slot) + 0x368, g.read(mgr + 4 * idx, 4))

    for keys, slot, idx in humans:
        d.tap(keys["attack"], 4, 6)       # join
        choose(slot, idx)
        d.tap(keys["attack"], 4, 6)       # fighter
        d.tap(keys["attack"], 4, 6)       # team: independent
    d.frames(200)                         # countdown
    # in VS mode the dialog starts at 0 with two humans and at 1 with one; in stage
    # mode it starts at 0
    start = 0 if mode == "stage" else 2 - len(humans)
    for _ in range(len(coms) - start):
        d.tap(P1["right"], 4, 6)
    d.tap(P1["attack"], 4, 20)            # number of computer players
    for k, idx in enumerate(coms):
        choose(len(humans) + k, idx)
        d.tap(P1["attack"], 4, 6)         # fighter
        d.tap(P1["attack"], 4, 6)         # team: independent
    if coms:
        d.frames(100)
    if mode == "battle":
        d.tap(P1["attack"], 4, 10)        # "OK" on the army setup screen (defaults)
    if difficulty is not None:
        g.put_i32(DIFFICULTY, difficulty)
    d.tap(P1["up"], 4, 6)
    d.tap(P1["up"], 4, 6)
    d.tap(P1["attack"], 4, 6)             # Fight!
    for _ in range(600):
        if g.i32(MATCH_STATE) == 0:
            break
        d.frames(1)
    assert g.i32(MATCH_STATE) == 0, g.i32(MATCH_STATE)
    print("in match; frames entered:", L.entered())


if __name__ == "__main__":
    a = sys.argv[1:]
    main(int(a[0]), None if a[1] == "none" else int(a[1]), [int(x) for x in a[2:]])
