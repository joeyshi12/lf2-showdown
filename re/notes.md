# Reverse-engineering notes

Facts found so far, with where they come from. "Verified" means checked against the
running game's memory; everything else is read from the decompile.

## Program structure

- `WinMain` = `0x43cf40`. `srand(timeGetTime())`, init, then a loop: when no message
  is waiting it calls the frame function `0x43e9a0` every 33 ms (`0x21`), or every 3 ms
  while `DAT_0044d02c == 0`. It catches up at most 100 ms.
- Frame function `0x43e9a0`: watches the key array for A, B, C (in that order) to
  unlock tool modes (F1 game, F2 data editor `0x4151d0`, F3 encrypt tool `0x414b70`).
  Mode 0 calls `0x4246b0` with `this = 0x458b00` (the game object).
- `0x4246b0` (menus): `*(int*)this` is the top state: 1 = start match setup,
  2 = in match (`0x41bc90`). Main menu substate `DAT_0044d064`. The menus are mouse
  driven: mouse x `DAT_004546f0`, y `DAT_00453cdc`, click `DAT_00457580`, menu y
  offset `DAT_00453da4`. Verified: writing these starts "game start".
- `0x41bc90` = match frame (thiscall on the game object). Loads all data on its first
  call (`DAT_0044d05c == 1`) through `0x4122f0`.
- `DAT_00450b90` toggles 0/1 every match frame.
- WndProc `0x43b3d0` (not found by Ghidra's analysis) writes `'d'`/`'u'` into the key
  array `0x455378[vk]` on key down/up.

## Game object `0x458b00` (thiscall `this`)

- `+0x000` int top state
- `+0x004` u8 exists[400]
- `+0x194` Object* objects[400]
- `+0x7d4` FileManager* (0x4d823a8 bytes)

## File manager (data)

- `+0x0000000` ObjectData* objects[...], count at `+0x4d82380`
- `+0x00007d0` stages[60], 0x149b08 each
- `+0x4d45db0` backgrounds, 0x990 each, count at `+0x4d82384`
- Layouts of ObjectData, Frame, Itr, Bdy, Background, Stage: see `crates/lf2-data`
  (each field has its offset). Verified field by field (`tests/memdump.rs`).
- Uninitialised memory in the original: ObjectData header numbers that a file leaves
  out, unused sheet slots, and cpoint `throwinjury`/`throwvz` (frame init skips
  @0xc4..@0xd8). These hold heap leftovers.

## Random numbers

- `0x422ac0` fills a 3000-byte table `DAT_0044ff90[i] = rand() % 255 + 1` when "game
  start" is clicked and resets `DAT_00450b48`.
- `0x417170` is the game's random(n): `c34 = (c34 + 1) % 1234; bcc = (bcc + 1) % 3000;
  return (table[bcc] + c34) % n` with `c34 = DAT_00450c34`, `bcc = DAT_00450bcc`.
  Recordings store the table (`+0x8c8` in the .lfr) and network play sends it.
- `0x42e100` (probably AI) calls CRT `rand()` directly.

## Object (0x420 bytes)

- `+0x10/+0x14/+0x18` int x, y, z (truncated from the doubles each tick)
- `+0x40/+0x48/+0x50` double vx, vy, vz
- `+0x58/+0x60/+0x68` double x, y, z
- `+0x70` frame index; `+0x80` u8 facing
- `+0x2fc` hp; `+0x300` dark hp (halved together on transform)
- `+0x31c` (weapon) hp, from data `+0x90`
- `+0x364` team; `+0x368` ObjectData*
- Frame of an object: `data + 0x7a4 + frame*0x178`; state at frame `+0x08`.
- `0x40e490` = movement and gravity for one object (gravity 1.7 = `_DAT_00448348`).

## Constants (from .rdata)

0.2 @448390, 1e-4 @448388, -1e-4 @448380, 9 @448378, -9 @448370, 1.1333.. @448368,
0.85 @447a20, 0.17 @448360, 0.425 @448358, 0.5666.. @448350, 1.7 @448348, -8 @448340,
8 @447980, 12 @448338, -7 @448330, 7 @447a50, 3 @447a40, -3.5 @448328, 11 @448320,
9.9 @448318, 0.5 @448310, -5 @448308, 8.5 @448300, -10 @4482f8, -0.7 @4482f0,
10 @447a00, 0.7 @4479d0, 17 @447950.

## Running under Wine

See `tools/harness/prepare_game.py` for the two changes the run copy needs (silent
music; a trailing newline on each .dat because Wine's `fscanf("%c")` reads ahead and
sets EOF one byte early, which makes the original hang on most files).

## Harness details found while recording

- `rand()` (msvcr80) keeps its seed in thread data outside lf2.exe. The lockstep patch
  points the import entries of `rand`/`srand` at equivalent code that keeps the seed at
  0x459f08, so traces record it. It also stores the x87 control word at 0x459f0c each
  frame; `record.py` checks it is still 53-bit precision (it always was).
- Fighters start with u_08 = 75 (invulnerable, blinking); scenarios wait 70 frames.
- Computer players use objects 10..17 (slot + 10); AI runs for objects 10..399.
- Keys are read into objects only on frames where `half_frame` (0x450b90) becomes 0,
  so a key must be held for two frames to register.
- Drawing changes simulation state in one place: `draw_objects` advances hit sparks.
  The background layer counters (file manager +0x4d465d4) change on every draw but
  only affect drawing.

## Network game

Ported in `crates/lf2-sim/src/net.rs` (protocol, packet build and checks,
`read_remote_inputs` 0x4198f0), `menu/title.rs` (title states 1-4), `tick.rs`
(match_frame stops at the exchange) and `crates/lf2-app/src/net.rs` (sockets).

- TCP port 12345. The title screen's "network game" calls `net_init` (0x402b60): a
  listening socket on the address of the host name (not 0.0.0.0: under Wine it is the
  host's LAN address, e.g. 172.18.0.1), with WSAAsyncSelect for FD_ACCEPT. The address
  is shown from 0x44f340.
- Handshake. Server (`net_receive` 0x402ec0, FD_ACCEPT, inside the window procedure):
  accept, close the listening socket, send "u can connect\0" (14), Sleep 3000, recv 77
  (hello), Sleep 500, send 77 (reply), Sleep 500, send 3001 bytes from 0x44ff90 (the
  random table and the low byte of 0x450b48). Client (0x428476, in the title frame
  after Enter): close the listening socket, connect, recv(100) and compare with "u can
  connect", send hello, Sleep 500, recv 77, Sleep 500, recv 3001 into 0x44ff90.
  Hello and reply: 8 slot flags ('1' = played on the other computer), then from byte
  0x20 four names of 11 bytes padded with '_', byte 0x4c = 0. Server plays slots 0-3
  (players 1-4), client slots 4-7; the other's slots get player -1 (0x450b4c..). The
  client copies its own names to 0x44fcec and the server's into 0x44fcc0. 0x44f1af is
  1 on the client and 2 on the server; 0x44f1ae = 1 on the server once joined.
- Every frame with half_frame 0 (also in the menus and while paused), after
  read_inputs_and_ai: packet 0x44d040, 22 bytes, starting from 20 bytes of 0x01 and
  a 0: bytes 0-7 the slot keys (bits up 0x80 down 0x40 left 0x20 right 0x10 attack 8
  jump 4 defend 2, local slots only), 9 counter (0x450bf0 + 1) % 50, 10 and 12 the
  F-keys (10: F1 2, F4 4, F5 8, F6 0x10, F7 0x20, F2 0x40, F9 0x80; 12: F3 2, F8 4,
  key 0xf9 8, key 0xf8 0x10; all but 0xf9/0xf8 only while match_phase is 0), 11 the
  version (0x44d03c), 13 the hp sum of objects 0-19 % 100 + 1 (0x450bf4), 14-15 the data
  checksum 0x44f620 % 256 and / 256 % 256 (C rounding). The client sends then receives
  into 0x44f198, the server receives then sends; both block (ioctlsocket FIONBIO 0).
  Then counter, version, hp sum and checksum are checked ("Connection Lost!", "Version
  not matched!", "Synchronisation Error!", "Data Error! ..."; each ends the game), and
  every F-key whose bit either computer set is applied, in the order F1, F2, F4, F5,
  F3, 0xf9, 0xf8, F6, F7, F8, F9. read_remote_inputs then gives the remote slots the
  received keys.
- The camera follows only local players, so camera and sound panning differ between
  the two computers; the simulation does not depend on them.
- 0x44d058 starts at 1: the first menu frame of a network game draws "Waiting for
  opponent..." once.
- Checked against the original under Wine before the port moved to WebRTC (commit
  6809770, tools/harness/net_check.py): both roles, 420 exchanges each, objects equal
  at every exchange apart from heap garbage the original never initialises.
- The port now carries the same bytes (greeting, hello, reply, table, packets) over a
  WebRTC data channel set up with icebreaker room codes (lf2_sim::net::NetSession,
  lf2-app rtc.rs, web/index.html). "server" hosts a room and shows the code where the
  original shows its IP address; "client" types the code.
