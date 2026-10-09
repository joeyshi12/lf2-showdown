# LF2 Showdown

Little Fighter 2 v2.0a rebuilt in Rust. The game logic matches the original `lf2.exe`
frame for frame. Private study only.

The game's files are not included. You need your own copy of the game folder (with
`data/`, `sprite/`, `bg/` and `bgm/`). Point the programs at it with
`LF2_DIR=/path/to/LittleFighter`.

## Play

- Desktop: `cargo run --release -p lf2-app`. Or copy the built program into the game
  folder and run it there.
- Browser: see "Web" below.
- Music: run `tools/convert_bgm.sh GAME_DIR` once to turn the WMA music into Ogg
  files. The browser needs them. The desktop can also play the WMA files directly.
- Network game: on the title screen choose "network game". The host picks "server"
  and gets a room code; the other player picks "client" and types it. Desktop and
  browser can play each other. The connection is peer to peer (WebRTC) and is set up
  through `wss://rv.joeyshi.xyz/ws?app=lf2-showdown` (change it with `LF2_RENDEZVOUS` on the
  desktop or `?rendezvous=` on the web page).

Differences from the original: no ads, no forum link, a smoother 60 pictures a second
(`--fps 30` for the original's 30), and the network game uses room codes instead of
IP addresses, so it cannot play against the original program.

## Build

The build copies some data out of the original `lf2.exe` (found through `LF2_DIR`) into
the program, so the program can replace `lf2.exe`.

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
