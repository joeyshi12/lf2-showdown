#!/usr/bin/env python3
"""Daemon that runs the original lf2.exe under Wine and serves memory access.

The kernel only lets a process read another's /proc/<pid>/mem if it is an ancestor
(yama ptrace_scope=1), so this daemon starts Wine itself and stays alive; clients talk
to it over a Unix socket with one JSON object per line:

  {"op": "read", "addr": 4573952, "len": 16}      -> {"hex": "..."}
  {"op": "write", "addr": 4573952, "hex": "90"}   -> {"ok": true}
  {"op": "readmany", "ranges": [[addr, len], ...]} -> {"hex": "..."} (concatenated)
  {"op": "pid"}                                   -> {"pid": 1234}
  {"op": "quit"}                                  -> stops Wine and the daemon

Usage: lf2d.py start     (detaches; env LF2_RUN_DIR, default ~/.cache/lf2-showdown)
       lf2d.py stop
"""
import json, os, socket, subprocess, sys, time

RUN = os.environ.get("LF2_RUN_DIR", os.path.expanduser("~/.cache/lf2-showdown"))
SOCK = os.path.join(RUN, "lf2d.sock")
DISPLAY = os.environ.get("LF2_DISPLAY", ":91")


def env():
    e = dict(os.environ)
    e.update(WINEPREFIX=os.path.join(RUN, "prefix"), WINEDEBUG="-all", DISPLAY=DISPLAY,
             ALSA_CONFIG_PATH=os.path.join(RUN, "asound.conf"))
    return e


def find_game_pid(root):
    """The Linux process whose command line is lf2.exe and that descends from root."""
    for _ in range(100):
        for p in os.listdir("/proc"):
            if not p.isdigit():
                continue
            try:
                cmd = open(f"/proc/{p}/cmdline", "rb").read()
                if b"lf2.exe" not in cmd or b"wine" in cmd.split(b"\0")[0][-8:]:
                    continue
                q = int(p)
                while q > 1:
                    if q == root:
                        return int(p)
                    st = open(f"/proc/{q}/stat").read()
                    q = int(st[st.rindex(")") + 2:].split()[1])
            except (OSError, ValueError):
                continue
        time.sleep(0.1)
    raise RuntimeError("lf2.exe did not start")


def serve():
    # Wine's launcher exits after starting the game; become a subreaper so the game
    # is reparented to this daemon and stays its descendant.
    import ctypes
    PR_SET_CHILD_SUBREAPER = 36
    ctypes.CDLL(None, use_errno=True).prctl(PR_SET_CHILD_SUBREAPER, 1, 0, 0, 0)
    if not os.path.exists("/tmp/.X11-unix/X" + DISPLAY.lstrip(":")):
        subprocess.Popen(["Xvfb", DISPLAY, "-screen", "0", "1024x768x24"],
                         stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        time.sleep(1)
    game = os.path.join(RUN, "game")
    proc = subprocess.Popen(["wine", "lf2.exe"], cwd=game, env=env(),
                            stdout=open(os.path.join(RUN, "wine.log"), "w"), stderr=subprocess.STDOUT)
    pid = find_game_pid(os.getpid())
    mem = open(f"/proc/{pid}/mem", "r+b", buffering=0)
    try:
        os.unlink(SOCK)
    except FileNotFoundError:
        pass
    srv = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    srv.bind(SOCK)
    srv.listen(4)
    while True:
        conn, _ = srv.accept()
        f = conn.makefile("rwb")
        quit_ = False
        for line in f:
            req = json.loads(line)
            try:
                op = req["op"]
                if op == "read":
                    mem.seek(req["addr"])
                    out = {"hex": mem.read(req["len"]).hex()}
                elif op == "readmany":
                    parts = []
                    for a, n in req["ranges"]:
                        mem.seek(a)
                        parts.append(mem.read(n))
                    out = {"hex": b"".join(parts).hex()}
                elif op == "write":
                    mem.seek(req["addr"])
                    mem.write(bytes.fromhex(req["hex"]))
                    out = {"ok": True}
                elif op == "pid":
                    out = {"pid": pid}
                elif op == "quit":
                    out = {"ok": True}
                    quit_ = True
                else:
                    out = {"error": "unknown op"}
            except Exception as e:  # report and keep serving
                out = {"error": repr(e)}
            f.write((json.dumps(out) + "\n").encode())
            f.flush()
            if quit_:
                break
        conn.close()
        if quit_ or proc.poll() is not None:
            break
    subprocess.run(["wineserver", "-k"], env=env())
    os.unlink(SOCK)


if __name__ == "__main__":
    cmd = sys.argv[1] if len(sys.argv) > 1 else "start"
    if cmd == "start":
        if os.fork() == 0:
            os.setsid()
            log = open(os.path.join(RUN, "lf2d.log"), "w")
            os.dup2(log.fileno(), 1)
            os.dup2(log.fileno(), 2)
            serve()
            os._exit(0)
        for _ in range(100):
            if os.path.exists(SOCK):
                print("started")
                break
            time.sleep(0.2)
        else:
            print("did not start; see", os.path.join(RUN, "lf2d.log"))
    elif cmd == "serve":
        serve()
    elif cmd == "stop":
        s = socket.socket(socket.AF_UNIX)
        s.connect(SOCK)
        s.sendall(b'{"op":"quit"}\n')
        print(s.recv(100).decode().strip())
