#!/usr/bin/env python3
"""Write traces/scenarios/specials_<name>.txt for each hero: the hero (player 1) does
every special-move input once (D>A, D^A, DvA, D>J, D^J, DvJ, DJA), with mp refilled
before each, against Davis standing in front (player 2).

usage: gen_specials.py OUT_DIR
"""
import os, sys

HEROES = {  # file-manager index (data.txt order) -> name
    1: "julian", 2: "firzen", 3: "louisex", 4: "bat", 5: "justin", 6: "knight", 7: "jan",
    8: "monk", 9: "sorcerer", 10: "jack", 11: "mark", 12: "hunter", 13: "bandit",
    14: "deep", 15: "john", 16: "henry", 17: "rudolf", 18: "louis", 19: "firen",
    20: "freeze", 21: "dennis", 22: "woody", 23: "davis",
}
INPUTS = [("right", "attack"), ("up", "attack"), ("down", "attack"),
          ("right", "jump"), ("up", "jump"), ("down", "jump"), ("jump", "attack")]


def scenario(idx, name):
    lines = [f"# {name}: every special input once, mp refilled before each, against Davis.",
             f"p1 {idx}", "p2 23", "rng zero", "place p1 300 300 0", "place p2 420 300 1", "wait 70"]
    for a, b in INPUTS:
        lines += ["set p1 mp 500", "set p1 x 300", "set p2 x 420", "set p2 hp 500",
                  "hold 2 p1.defend", "wait 2", f"hold 2 p1.{a}", "wait 2", f"hold 2 p1.{b}", "wait 84"]
    return "\n".join(lines) + "\n"


if __name__ == "__main__":
    out = sys.argv[1]
    for idx, name in HEROES.items():
        open(os.path.join(out, f"specials_{name}.txt"), "w").write(scenario(idx, name))
    print("wrote", len(HEROES), "scenarios")
