# LF2 Showdown

Little Fighter 2 v2.0a rebuilt in Rust. Private study only.

## Play

- Desktop: `cargo run --release -p lf2-app`. Or copy the built program into the game
  folder and run it there. Alt+Enter switches between a window and full screen;
  `--fullscreen` starts full screen. The window can be resized.
- Browser: see "Web" below.
- Touch screens: `--touch` shows on-screen controls for player 1: the direction pad
  at the bottom left, attack (A), jump (J) and defend (D) at the bottom right, and F4
  (back to the menu), F1 (pause) and the on-screen keyboard at the top. A touch
  anywhere else is the mouse, for the menus.
- Android: install the APK from "Build" below. It runs full screen in landscape with
  the touch controls above; the back button is F4. A controller (a gamepad, or the
  built-in controls of an Android handheld) plays player 1 without changing the
  control settings: the touch controls hide while it is used and come back when the
  screen is touched. Start is F1 (pause) and Select is F4.
- Game controllers (desktop and browser): under control settings, click a player's
  picture to switch from the keyboard to joystick 1 or 2, the first and second
  controller connected. The direction pad and left stick move; the bottom, right,
  left and top face buttons are buttons 0 to 3.
- Music: `game/bgm/` has the music as Ogg. For another copy of the game, run
  `tools/convert_bgm.sh GAME_DIR` once to turn its WMA music into Ogg files. The
  browser needs them; the desktop can also play the WMA files.
- Network game: up to four computers, one player each. On the title screen choose
  "network game".

Differences from the original: no ads, no forum link, a smoother 60 fps
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
- Android, from Linux: `tools/android/build-apk.sh [OUT.apk]` (default
  `target/lf2-showdown.apk`). Needs the Android SDK with build-tools and a platform
  (`ANDROID_HOME`, default `~/Android/Sdk`), the NDK, a JDK, and the
  `aarch64-linux-android` Rust target; `ABIS="arm64-v8a x86_64"` also builds for the
  emulator. Install with `adb install target/lf2-showdown.apk`. The APK carries the
  game folder, which the app copies to its own storage on the first start.
- Web: `wasm-pack build --release --target web --out-dir OUT crates/lf2-web`, then
  `tools/web/package.sh OUT DIST GAME_DIR`, and serve `DIST` with any web server.

On Linux and the web, text uses Wine's copy of the Windows System font
(`/usr/share/wine/fonts/vgasys.fon`, or `LF2_SYSTEM_FONT`), built into the program.

## Test

`cargo test --workspace` with `LF2_DIR` set. The main test replays every recording in
`traces/` and checks that the game state matches the original's on every frame.
