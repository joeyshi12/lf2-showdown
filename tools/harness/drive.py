"""Driving the original through its menus by writing the mouse and key state that the
window procedure would write. Coordinates are client-area pixels (794x550 window)."""
import time

KEYS = 0x455378        # u8['d'|'u'][256], indexed by virtual-key code
MOUSE_X = 0x4546F0
MOUSE_Y = 0x453CDC
MOUSE_CLICK = 0x457580

VK = {"enter": 0x0D, "esc": 0x1B, "f4": 0x73}
# control.txt defaults for player 1: up W, down S, left A, right D, attack J, jump K, defend L
CONTROLS = 0x44FB20     # per player p (1..4) at CONTROLS + p*0x50: int kind, then 7 VKs
# Player 2 shares W/A/D with player 1 in the default control.txt; the harness gives it
# the numeric keypad instead (written into the loaded controls, not the file).
P2 = {"up": 0x68, "down": 0x62, "left": 0x64, "right": 0x66, "attack": 0x61, "jump": 0x63, "defend": 0x60}
ORDER = ["up", "down", "left", "right", "attack", "jump", "defend"]


def set_controls(game, player, keys):
    import struct
    base = CONTROLS + player * 0x50
    game.write(base, struct.pack("<i", 0) + b"".join(struct.pack("<i", keys[k]) for k in ORDER))


P1 = {"up": 0x57, "down": 0x53, "left": 0x41, "right": 0x44, "attack": 0x4A, "jump": 0x4B, "defend": 0x4C}


class Driver:
    def __init__(self, game, lockstep):
        self.g = game
        self.L = lockstep

    def frames(self, n):
        for _ in range(n):
            self.L.step()

    def click(self, x, y, hold=2):
        self.g.put_i32(MOUSE_X, x)
        self.g.put_i32(MOUSE_Y, y)
        self.frames(2)
        self.g.put_i32(MOUSE_CLICK, 1)
        self.frames(hold)

    def key(self, vk, down=True):
        self.g.put_u8(KEYS + vk, ord("d") if down else ord("u"))

    def tap(self, vk, hold=2, after=2):
        self.key(vk, True)
        self.frames(hold)
        self.key(vk, False)
        self.frames(after)

    def release_all(self):
        self.g.write(KEYS, b"u" * 256)
