"""Client for lf2d.py: typed memory access to the running original."""
import json, os, socket, struct

RUN = os.environ.get("LF2_RUN_DIR", os.path.expanduser("~/.cache/lf2-showdown"))
SOCK = os.path.join(RUN, "lf2d.sock")


class Game:
    def __init__(self):
        self.s = socket.socket(socket.AF_UNIX)
        self.s.connect(SOCK)
        self.f = self.s.makefile("rwb")

    def _req(self, **kw):
        self.f.write((json.dumps(kw) + "\n").encode())
        self.f.flush()
        r = json.loads(self.f.readline())
        if "error" in r:
            raise RuntimeError(r["error"])
        return r

    def read(self, addr, n):
        return bytes.fromhex(self._req(op="read", addr=addr, len=n)["hex"])

    def readmany(self, ranges):
        return bytes.fromhex(self._req(op="readmany", ranges=[list(r) for r in ranges])["hex"])

    def write(self, addr, data):
        self._req(op="write", addr=addr, hex=bytes(data).hex())

    def pid(self):
        return self._req(op="pid")["pid"]

    def i32(self, a):
        return struct.unpack("<i", self.read(a, 4))[0]

    def u32(self, a):
        return struct.unpack("<I", self.read(a, 4))[0]

    def u8(self, a):
        return self.read(a, 1)[0]

    def f64(self, a):
        return struct.unpack("<d", self.read(a, 8))[0]

    def put_i32(self, a, v):
        self.write(a, struct.pack("<i", v))

    def put_u8(self, a, v):
        self.write(a, bytes([v & 0xFF]))
