#!/usr/bin/env python3
"""Disassemble a range of lf2.exe with objdump, naming calls through the import table
(wsock32 ordinals get their Winsock names).
usage: disasm.py START END   (hex virtual addresses; LF2_DIR or ~/.cache/lf2-showdown/game)"""
import os, re, struct, subprocess, sys

WSOCK = {1: "accept", 2: "bind", 3: "closesocket", 4: "connect", 9: "htons", 10: "inet_addr",
         11: "inet_ntoa", 12: "inet_ntoa?", 13: "listen", 16: "recv", 19: "send", 20: "sendto",
         23: "socket", 51: "gethostbyaddr", 52: "gethostbyname", 57: "gethostname",
         101: "WSAAsyncSelect", 115: "WSAStartup", 116: "WSACleanup"}

exe = os.path.join(os.environ.get("LF2_DIR", os.path.expanduser("~/.cache/lf2-showdown/game")), "lf2.exe")
d = open(exe, "rb").read()
pe = struct.unpack_from("<I", d, 0x3c)[0]
nsec = struct.unpack_from("<H", d, pe + 6)[0]
osz = struct.unpack_from("<H", d, pe + 20)[0]
secs = [struct.unpack_from("<IIII", d, pe + 24 + osz + i * 40 + 8) for i in range(nsec)]


def off(rva):
    for vs, va, rs, ro in secs:
        if va <= rva < va + max(vs, rs):
            return rva - va + ro


names = {}
p = off(struct.unpack_from("<I", d, pe + 24 + 104)[0])
while True:
    ilt, _, _, name, iat = struct.unpack_from("<IIIII", d, p)
    p += 20
    if name == 0:
        break
    dll = d[off(name):].split(b"\0")[0].decode()
    q = off(ilt or iat)
    k = 0
    while True:
        v = struct.unpack_from("<I", d, q + 4 * k)[0]
        if v == 0:
            break
        if v & 0x80000000:
            n = WSOCK.get(v & 0xffff, f"#{v & 0xffff}") if "WSOCK" in dll.upper() else f"#{v & 0xffff}"
        else:
            n = d[off(v) + 2:].split(b"\0")[0].decode()
        names[0x400000 + iat + 4 * k] = n
        k += 1
out = subprocess.run(["objdump", "-d", "-M", "intel", f"--start-address=0x{sys.argv[1]}",
                      f"--stop-address=0x{sys.argv[2]}", exe], capture_output=True, text=True).stdout
for line in out.splitlines():
    m = re.match(r"\s*([0-9a-f]+):\t(?:[0-9a-f ]+)\t(.*)", line)
    if not m:
        continue
    a, ins = m.groups()
    ins = re.sub(r"ds:0x([0-9a-f]+)", lambda x: names.get(int(x.group(1), 16), "ds:0x" + x.group(1)), ins)
    print(f"{a}: {ins}")
