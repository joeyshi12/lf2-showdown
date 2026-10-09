#!/usr/bin/env python3
"""Write traces/scenarios/ai_<name>.txt for each hero: a computer player with the hero
fights a computer Davis and an idle human Davis (difficult), with the computers' mp
refilled every 150 frames so the special-move branches of the AI run.

usage: gen_ai.py OUT_DIR
"""
import os, sys
from gen_specials import HEROES


def scenario(idx, name):
    out = [f"# ai_{name}: computer {name} and computer Davis fight, human Davis stands still.",
           "p1 23", "p2 none", f"com {idx}", "com 23", "difficulty 2", "rng zero",
           "place p1 200 300 0", "place o11 400 300 1", "place o12 600 350 1"]
    for _ in range(8):
        out += ["set o11 mp 500", "set o12 mp 500", "wait 150"]
    return "\n".join(out) + "\n"


if __name__ == "__main__":
    for idx, name in HEROES.items():
        with open(os.path.join(sys.argv[1], f"ai_{name}.txt"), "w") as f:
            f.write(scenario(idx, name))
