"""Lockstep control of the original: a patch that makes the game wait at the start of
each call of its frame function `FUN_0043e9a0` until the harness allows it.

The patch writes a small routine into the zero padding at the end of .text and replaces
the first 7 bytes of `FUN_0043e9a0` (`push -1; push 0x4462a8`) with a jump to it:

    inc  [ENTERED]
  wait:
    mov  eax, [ALLOWED]
    cmp  eax, [ENTERED]
    jae  done                ; run while ALLOWED >= ENTERED
    push 1
    call [Sleep]
    jmp  wait
  done:
    push -1
    push 0x4462a8
    jmp  0x43e9a7

The routine also stores the x87 control word into FPU_CW at the start of every frame, so
the harness can check that the game runs at 53-bit precision throughout.

ENTERED and ALLOWED live at the end of the .data/.bss range, past the last address any
code refers to (0x459720). ALLOWED = 0xffffffff means run freely.

The patch also points the import table entries of msvcr80 `rand` and `srand` at two
small routines that do exactly what the CRT does (seed = seed * 214013 + 2531011;
return (seed >> 16) & 0x7fff), but keep the seed at CRT_SEED inside lf2.exe's .bss.
The CRT keeps its seed in per-thread data outside the game's memory; moving it makes
the seed part of every snapshot, so traces can record and set it.
"""
import struct, time

FRAME_FN = 0x43E9A0
CAVE = 0x446400
ENTERED = 0x459F00
ALLOWED = 0x459F04
SLEEP_IAT = 0x447098
RAND_IAT = 0x447198
SRAND_IAT = 0x4470EC
CRT_SEED = 0x459F08
FPU_CW = 0x459F0C   # x87 control word at the start of the latest frame
RAND_CAVE = 0x446440
SRAND_CAVE = 0x446460
FREE = 0xFFFFFFFF


def rand_bytes():
    c = b"\xa1" + struct.pack("<I", CRT_SEED)              # mov eax, [seed]
    c += b"\x69\xc0" + struct.pack("<I", 214013)           # imul eax, eax, 214013
    c += b"\x05" + struct.pack("<I", 2531011)               # add eax, 2531011
    c += b"\xa3" + struct.pack("<I", CRT_SEED)              # mov [seed], eax
    c += b"\xc1\xe8\x10"                                    # shr eax, 16
    c += b"\x25" + struct.pack("<I", 0x7FFF)                # and eax, 0x7fff
    c += b"\xc3"
    return c


def srand_bytes():
    return b"\x8b\x44\x24\x04" + b"\xa3" + struct.pack("<I", CRT_SEED) + b"\xc3"


def _rel(src_next, dst):
    return struct.pack("<i", dst - src_next)


TIME_SITES = (0x423896, 0x423BE0)
FAKE_TIME = 0x459F10


def cave_bytes():
    c = bytearray()
    c += b"\xd9\x3d" + struct.pack("<I", FPU_CW)       # fnstcw [FPU_CW]
    c += b"\xff\x05" + struct.pack("<I", ENTERED)
    wait = CAVE + len(c)
    c += b"\xa1" + struct.pack("<I", ALLOWED)
    c += b"\x3b\x05" + struct.pack("<I", ENTERED)
    c += b"\x73\x0a"
    c += b"\x6a\x01"
    c += b"\xff\x15" + struct.pack("<I", SLEEP_IAT)
    c += b"\xeb" + struct.pack("<b", wait - (CAVE + len(c) + 2))
    c += b"\x6a\xff"
    c += b"\x68" + struct.pack("<I", 0x4462A8)
    c += b"\xe9" + _rel(CAVE + len(c) + 5, FRAME_FN + 7)
    return bytes(c)


class Lockstep:
    def __init__(self, game):
        self.g = game

    def install(self):
        g = self.g
        orig = g.read(FRAME_FN, 7)
        if orig[0] == 0xE9:
            return  # already installed
        assert orig == b"\x6a\xff\x68\xa8\x62\x44\x00", orig.hex()
        assert g.read(CAVE, 0x40) == bytes(0x40)
        g.write(ALLOWED, struct.pack("<I", FREE))
        g.write(ENTERED, struct.pack("<I", 0))
        g.write(CAVE, cave_bytes())
        g.write(FRAME_FN, b"\xe9" + _rel(FRAME_FN + 5, CAVE) + b"\x90\x90")
        assert g.read(RAND_CAVE, 0x40) == bytes(0x40)
        g.write(CRT_SEED, struct.pack("<I", 0))
        g.write(RAND_CAVE, rand_bytes())
        g.write(SRAND_CAVE, srand_bytes())
        g.write(RAND_IAT, struct.pack("<I", RAND_CAVE))
        g.write(SRAND_IAT, struct.pack("<I", SRAND_CAVE))
        # timeGetTime() picks the menu background (0x423896) and the first ad
        # (0x423be0); read a fixed value instead (0 at FAKE_TIME), so traces of the
        # menus do not depend on the clock. "call [timeGetTime]" -> "mov eax,[FAKE_TIME]".
        for site in TIME_SITES:
            assert g.read(site, 6) == b"\xff\x15\x50\x72\x44\x00", hex(site)
            g.write(site, b"\xa1" + struct.pack("<I", FAKE_TIME) + b"\x90")

    def entered(self):
        return self.g.u32(ENTERED)

    def free_run(self):
        self.g.write(ALLOWED, struct.pack("<I", FREE))

    def pause(self, timeout=5.0):
        """Stop at the next frame start. Returns the number of frames entered."""
        n = self.entered()
        self.g.write(ALLOWED, struct.pack("<I", n))
        return self.wait_blocked(n + 1, timeout)

    def wait_blocked(self, n, timeout=5.0):
        t = time.time()
        while self.entered() < n:
            if time.time() - t > timeout:
                raise TimeoutError(f"frame {n} not reached")
            time.sleep(0.0005)
        # ENTERED is incremented before the wait loop, and ALLOWED is n - 1, so the
        # game is now waiting at the start of frame n.
        return n

    def step(self, timeout=30.0):
        """Run the blocked frame and stop at the start of the following one."""
        n = self.entered()
        self.g.write(ALLOWED, struct.pack("<I", n))
        return self.wait_blocked(n + 1, timeout)
