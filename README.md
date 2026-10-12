# LF2 Showdown

Little Fighter 2 v2.0a rebuilt in Rust. The game logic matches the original `lf2.exe`
frame for frame. Private study only.

The game files the port needs are in `game/`: `data/`, `sprite/`, `bg/`, the demo
recordings, the music as Ogg files, and `lf2_image.bin`, which holds the parts of the
original `lf2.exe` that the port uses (its starting data and the menu pictures). Point
the programs at it with `LF2_DIR=$PWD/game`.

## Play

- Desktop: `cargo run --release -p lf2-app`. Or copy the built program into the game
  folder and run it there. Alt+Enter switches between a window and full screen;
  `--fullscreen` starts full screen. The window can be resized.
- Browser: see "Web" below.
- Music: `game/bgm/` has the music as Ogg. For another copy of the game, run
  `tools/convert_bgm.sh GAME_DIR` once to turn its WMA music into Ogg files. The
  browser needs them; the desktop can also play the WMA files.
- Network game: up to four computers, one player each. On the title screen choose
  "network game". The host picks "Host a game" and gets a room code; the others
  pick "Join a game" and type it. The host presses Enter to start
  once everyone is listed. Desktop and browser can play each other. Each computer
  runs ahead with its own input and corrects the game when another computer's
  input arrives late (rollback), so a slow connection does not slow the game down.
  The connections are peer to peer (WebRTC), each joiner to the host, and are set up
  through `wss://rv.lf2-showdown.online/ws?app=lf2-showdown` (change it with
  `LF2_RENDEZVOUS` on the desktop or `?rendezvous=` on the web page).

Differences from the original: no ads, no forum link, a smoother 60 pictures a second
(`--fps 30` for the original's 30), and the network game takes up to four computers
with rollback and room codes instead of two with lockstep and IP addresses, so it
cannot play against the original program.

## Build

The build puts `lf2_image.bin` (or the parts of `lf2.exe` it uses, from a full game
folder) from `LF2_DIR` into the program, so the program can replace `lf2.exe`.

- Linux: `cargo build --release -p lf2-app`
- Windows, on Windows: `cargo build --release -p lf2-app`
- Windows, from Linux: `tools/cross/build-windows.sh` (needs the `x86_64-pc-windows-gnu`
  Rust target and zig: `pip install ziglang`)
- Web: `wasm-pack build --release --target web --out-dir OUT crates/lf2-web`, then
  `tools/web/package.sh OUT DIST GAME_DIR`, and serve `DIST` with any web server.

On Linux and the web, text uses Wine's copy of the Windows System font
(`/usr/share/wine/fonts/vgasys.fon`, or `LF2_SYSTEM_FONT`), built into the program.

## Test

`cargo test --workspace` with `LF2_DIR` set. The main test replays every recording in
`traces/` and checks that the game state matches the original's on every frame.
