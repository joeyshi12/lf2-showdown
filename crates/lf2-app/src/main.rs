//! lf2: play Little Fighter 2 v2.0a with the Rust simulation.
//!
//! usage: lf2 [--game DIR] [--p1 NAME --p2 NAME] [--bg N] [--seed N]
//!            [--fps 60|30] [--dump FRAMES OUT.ppm]
//!
//! The game folder (the one with data/, sprite/, bg/) comes from --game, LF2_DIR, or
//! else is the folder this program is in. What the game needs from the original
//! lf2.exe is built into the program (build.rs), so this program can replace it. The game starts at the title screen; with --p1 and --p2 it starts a VS match
//! between two human players straight away. Keys come from data/control.txt (default:
//! player 1 W A S D J K L, player 2 arrows, Enter, Shift, Ctrl). F1 pauses, F2 steps
//! one frame, F4 leaves, F5 toggles speed, F6-F9 are the original's cheats.
//!
//! The simulation runs one tick every 33 ms, as the original. By default (--fps 60)
//! two pictures are shown per tick, one tick behind the simulation: tick N's picture,
//! then tick N's picture with the objects and the camera halfway to their positions in
//! tick N+1. --fps 30 shows one picture per tick, as the original, without the delay.
//!
//! On Windows the window is scaled by a whole number (1, 2 or 4) chosen from the
//! display scaling, and the program tells Windows it handles the scaling itself, so
//! Windows does not stretch the picture by 125% or 150% and blur it.
//!
//! --dump runs FRAMES frames with no keys and no window, writes the last picture as a
//! PPM file and exits (for comparing pictures with the original).

use lf2_app::{assets, audio, platform, render};

use lf2_sim::setup::{Fighter, VsSetup, DEFAULT_CONTROLS};
use lf2_sim::World;
use minifb::{Key, KeyRepeat, Window, WindowOptions};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// The Windows virtual-key code that the original's window procedure (0x43b3d0) gets
/// in WM_KEYDOWN/WM_KEYUP for a key; None for keys it never gets there. Both Shift keys
/// are VK_SHIFT (0x10) and both Ctrl keys VK_CONTROL (0x11), as in wParam. Alt and F10
/// come as WM_SYSKEYDOWN/WM_SYSKEYUP, which the original does not write into the key
/// array, so they have no code here. The keypad keys are the NumLock-on codes (minifb
/// does not report the NumLock state).
fn vk(k: Key) -> Option<u8> {
    use Key::*;
    Some(match k {
        A => 0x41, B => 0x42, C => 0x43, D => 0x44, E => 0x45, F => 0x46, G => 0x47, H => 0x48,
        I => 0x49, J => 0x4a, K => 0x4b, L => 0x4c, M => 0x4d, N => 0x4e, O => 0x4f, P => 0x50,
        Q => 0x51, R => 0x52, S => 0x53, T => 0x54, U => 0x55, V => 0x56, W => 0x57, X => 0x58,
        Y => 0x59, Z => 0x5a,
        Key0 => 0x30, Key1 => 0x31, Key2 => 0x32, Key3 => 0x33, Key4 => 0x34, Key5 => 0x35,
        Key6 => 0x36, Key7 => 0x37, Key8 => 0x38, Key9 => 0x39,
        NumPad0 => 0x60, NumPad1 => 0x61, NumPad2 => 0x62, NumPad3 => 0x63, NumPad4 => 0x64,
        NumPad5 => 0x65, NumPad6 => 0x66, NumPad7 => 0x67, NumPad8 => 0x68, NumPad9 => 0x69,
        NumPadAsterisk => 0x6a, NumPadPlus => 0x6b, NumPadMinus => 0x6d, NumPadDot => 0x6e,
        NumPadSlash => 0x6f,
        Left => 0x25, Up => 0x26, Right => 0x27, Down => 0x28,
        Enter | NumPadEnter => 0x0d, Space => 0x20, Tab => 0x09, Escape => 0x1b, Backspace => 0x08,
        LeftShift | RightShift => 0x10, LeftCtrl | RightCtrl => 0x11,
        Pause => 0x13, CapsLock => 0x14, NumLock => 0x90, ScrollLock => 0x91,
        LeftSuper => 0x5b, RightSuper => 0x5c, Menu => 0x5d,
        Backquote => 0xc0, Comma => 0xbc, Period => 0xbe, Slash => 0xbf, Semicolon => 0xba,
        Apostrophe => 0xde, LeftBracket => 0xdb, RightBracket => 0xdd, Backslash => 0xdc,
        Minus => 0xbd, Equal => 0xbb,
        F1 => 0x70, F2 => 0x71, F3 => 0x72, F4 => 0x73, F5 => 0x74, F6 => 0x75, F7 => 0x76,
        F8 => 0x77, F9 => 0x78, F11 => 0x7a, F12 => 0x7b, F13 => 0x7c, F14 => 0x7d, F15 => 0x7e,
        Insert => 0x2d, Delete => 0x2e, Home => 0x24, End => 0x23, PageUp => 0x21, PageDown => 0x22,
        _ => return None,
    })
}

#[cfg(test)]
mod vk_tests {
    use super::vk;
    use minifb::Key;

    /// Every key the game reads directly: Enter, Shift, the arrows (replay camera), G
    /// (the title screen), F1-F9, F11, F12, and the keys of the default control.txt.
    #[test]
    fn keys_the_game_reads_have_codes() {
        let pairs = [
            (Key::Enter, 0x0d), (Key::LeftShift, 0x10), (Key::RightShift, 0x10), (Key::LeftCtrl, 0x11),
            (Key::RightCtrl, 0x11), (Key::Left, 0x25), (Key::Up, 0x26), (Key::Right, 0x27), (Key::Down, 0x28),
            (Key::G, 0x47), (Key::F1, 0x70), (Key::F2, 0x71), (Key::F3, 0x72), (Key::F4, 0x73), (Key::F5, 0x74),
            (Key::F6, 0x75), (Key::F7, 0x76), (Key::F8, 0x77), (Key::F9, 0x78), (Key::F11, 0x7a), (Key::F12, 0x7b),
            (Key::Escape, 0x1b), (Key::Tab, 0x09), (Key::Space, 0x20), (Key::Backquote, 0xc0),
            (Key::Comma, 0xbc), (Key::Period, 0xbe), (Key::Backspace, 0x08),
        ];
        for (k, code) in pairs {
            assert_eq!(vk(k), Some(code), "{:?}", k);
        }
        for p in lf2_sim::setup::DEFAULT_CONTROLS {
            for code in p {
                let found = (0..Key::Count as u8)
                    .map(|i| unsafe { std::mem::transmute::<u8, Key>(i) })
                    .any(|k| vk(k) == Some(code as u8));
                assert!(found, "no key gives the control.txt code {}", code);
            }
        }
    }

    /// Alt and F10 arrive as WM_SYSKEYDOWN, which the original ignores.
    #[test]
    fn alt_and_f10_are_not_keys_of_the_game() {
        for k in [Key::LeftAlt, Key::RightAlt, Key::F10, Key::Unknown] {
            assert_eq!(vk(k), None, "{:?}", k);
        }
    }

    /// No two different keys share a code, except the left and right Shift and Ctrl
    /// keys and the two Enter keys, which Windows does not tell apart in wParam either.
    #[test]
    fn codes_are_unique() {
        let shared = [0x10u8, 0x11, 0x0d];
        let mut seen = std::collections::HashMap::new();
        for i in 0..Key::Count as u8 {
            let k: Key = unsafe { std::mem::transmute(i) };
            if let Some(c) = vk(k) {
                if let Some(other) = seen.insert(c, k) {
                    assert!(shared.contains(&c), "{:?} and {:?} both give {:#x}", other, k, c);
                }
            }
        }
    }
}

/// data/control.txt: four lines of "kind up down left right attack jump defend ...",
/// then the four player names (`` ` `` stands for a space).
fn read_controls(dir: &std::path::Path) -> ([[i32; 7]; 4], [String; 4]) {
    let mut keys = DEFAULT_CONTROLS;
    let mut names: [String; 4] = std::array::from_fn(|i| (i + 1).to_string());
    if let Ok(b) = std::fs::read(dir.join("data").join("control.txt")) {
        let t = String::from_utf8_lossy(&b);
        let toks: Vec<&str> = t.split_whitespace().collect();
        if toks.len() >= 48 {
            for p in 0..4 {
                for k in 0..7 {
                    if let Ok(v) = toks[p * 11 + 1 + k].parse() {
                        keys[p][k] = v;
                    }
                }
            }
            for p in 0..4 {
                names[p] = toks[44 + p].replace('`', " ");
            }
        }
    }
    (keys, names)
}

/// Menu item "Play recording" (0x431d10): a file dialog, then `World::load_replay`,
/// with the original's error messages.
fn open_recording(world: &mut World, dir: &std::path::Path, exe: &[u8]) {
    let Some(path) = platform::pick_recording(&lf2_data::resolve(dir, b"recording")) else { return };
    let rec = std::fs::read(&path).map_err(|e| e.to_string()).and_then(|f| lf2_data::lfr::decode(&f, exe));
    use lf2_sim::replay::ReplayError;
    let msg = match rec {
        Err(_) => Some("Loading error!  Recording file may be corrupted!!".to_string()),
        Ok(rec) => match world.load_replay(rec) {
            Ok(()) => None,
            Err(ReplayError::Corrupt) => Some("Loading error!  Recording file may be corrupted!!".to_string()),
            Err(ReplayError::DataFiles) => Some("Error!  Recording file are recorded in a LF2 with some data files (character or stage files) different from yours.  Your LF2 has to use the same set of data files in order in replay this recording file!".to_string()),
            Err(ReplayError::Version(v)) => {
                let theirs = if v == 0x1e { "v2.0" } else if v > world.g.m.i32(0x44d03c) { "a newer version" } else { "" };
                Some(format!("Version error.  Recording file are recorded in {}. (your LF2 is v2.0)!", theirs))
            }
        },
    };
    if let Some(m) = msg {
        platform::message(&m);
    }
}

/// The window's scale. On Windows: declare the program DPI-aware, so that Windows does
/// not stretch the window by the display scaling (125%, 150%, ...) with smoothing,
/// and scale by the largest whole factor not above it instead. Elsewhere 1.
fn window_scale() -> minifb::Scale {
    #[cfg(windows)]
    unsafe {
        windows_sys::Win32::UI::WindowsAndMessaging::SetProcessDPIAware();
        match windows_sys::Win32::UI::HiDpi::GetDpiForSystem() / 96 {
            0 | 1 => minifb::Scale::X1,
            2 | 3 => minifb::Scale::X2,
            _ => minifb::Scale::X4,
        }
    }
    #[cfg(not(windows))]
    minifb::Scale::X1
}

/// A Little Fighter 2 folder: it has data\\data.txt.
fn is_game_dir(d: &std::path::Path) -> bool {
    lf2_data::resolve(d, b"data\\data.txt").is_file()
}

fn fighter_index(gd: &lf2_data::GameData, name: &str) -> Option<usize> {
    gd.objects.iter().position(|o| o.kind == 0 && String::from_utf8_lossy(&o.name).eq_ignore_ascii_case(name))
}

fn main() {
    let mut args = std::env::args().skip(1);
    let mut dir = lf2_data::game_dir_from_env();
    let (mut p1, mut p2, mut bg, mut seed) = (None::<String>, None::<String>, None, None);
    let mut dump: Option<(u32, String)> = None;
    let mut fps = 60;
    let mut tween = render::Tween::new();
    while let Some(a) = args.next() {
        let mut val = || args.next().unwrap_or_else(|| panic!("{} needs a value", a));
        match a.as_str() {
            "--game" => dir = Some(val().into()),
            "--p1" => p1 = Some(val()),
            "--p2" => p2 = Some(val()),
            "--bg" => bg = val().parse().ok(),
            "--seed" => seed = val().parse().ok(),
            "--fps" => {
                fps = val().parse().unwrap_or(0);
                if fps != 30 && fps != 60 {
                    panic!("--fps is 60 or 30");
                }
            }
            "--dump" => {
                let n = val().parse().expect("frames");
                dump = Some((n, val()));
            }
            "-h" | "--help" => {
                println!("usage: lf2 [--game DIR] [--p1 NAME] [--p2 NAME] [--bg N] [--seed N] [--fps 60|30] [--dump FRAMES OUT.ppm]");
                return;
            }
            other => panic!("unknown argument {}", other),
        }
    }
    // without --game or LF2_DIR: the folder this program is in, then the current folder
    let dir = dir
        .or_else(|| {
            let here = std::env::current_exe().ok()?.parent()?.to_path_buf();
            is_game_dir(&here).then_some(here)
        })
        .or_else(|| is_game_dir(std::path::Path::new(".")).then(|| ".".into()))
        .expect("put this program in the Little Fighter 2 folder, or set LF2_DIR or pass --game DIR (the folder with data/data.txt)");
    let exe: Vec<u8> = match lf2_app::exe_image::built_in() {
        Some(img) => img.to_vec(),
        None => {
            let exe = lf2_data::read_exe(&dir).unwrap_or_default();
            if !lf2_app::exe_image::is_original(&exe) {
                eprintln!("this build has no game data built in and the game folder has no original lf2.exe v2.0a; build it with LF2_DIR set");
                std::process::exit(1);
            }
            exe
        }
    };
    let gd = Arc::new(lf2_data::GameData::load(&dir).expect("load data"));
    eprintln!("loading images...");
    let assets = assets::Assets::load(&gd, &exe);

    let (controls, names) = read_controls(&dir);
    let seed = seed.unwrap_or_else(|| std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as u32);
    let pick = |n: &str| fighter_index(&gd, n).unwrap_or_else(|| panic!("no fighter named {}", n));
    let bg = bg.unwrap_or((seed % (gd.backgrounds.len() as u32).max(1)) as i32);
    let quick = p1.is_some() || p2.is_some();
    let setup = VsSetup {
        fighters: vec![
            Fighter { slot: 0, data: pick(p1.as_deref().unwrap_or("Davis")), team: 0, human: true, pos: None },
            Fighter { slot: 1, data: pick(p2.as_deref().unwrap_or("Woody")), team: 0, human: true, pos: None },
        ],
        background: bg,
        difficulty: 2,
        seed,
    };
    let file_controls = lf2_data::controls::load(&dir).unwrap_or_default();
    let start = std::time::Instant::now();
    // the advertisements are left out: none are loaded, and the title screen's ad
    // switch and ad question are not shown
    let boot = || {
        let mut w = World::new_title(gd.clone(), &exe, &file_controls, None, seed);
        w.ads_removed = true;
        w
    };
    let mut world = if quick { World::new_vs_match(gd.clone(), &exe, &setup, &controls) } else { boot() };
    let mut renderer = render::Renderer::new(&gd);
    for (i, n) in names.iter().enumerate() {
        renderer.names[i] = n.clone();
    }
    let mut frame = render::Frame::new();
    if let Some((n, path)) = dump {
        for _ in 0..n {
            world.set_keys(&[b'u'; 256]);
            world.step();
            renderer.draw_frame(&world, &assets, &mut frame);
        }
        let mut out = format!("P6\n{} {}\n255\n", render::W, render::H).into_bytes();
        for p in &frame.px {
            out.extend_from_slice(&[(p >> 16) as u8, (p >> 8) as u8, *p as u8]);
        }
        std::fs::write(&path, out).expect("write");
        eprintln!("unported sections reached: {:?}", world.unported_hits);
        return;
    }
    let mut audio = audio::Audio::new(&gd);
    let options = WindowOptions { scale: window_scale(), ..WindowOptions::default() };
    let mut window = Window::new("Little Fighter 2", render::W as usize, render::H as usize, options).expect("window");
    window.set_target_fps(0);
    let mut keys = [b'u'; 256];
    let mut next = Instant::now();
    let mut warned = 0;
    let mut mouse_was_down = false;
    let mut net = lf2_app::net::Net::default();
    // Key events since the window's last update. While the world waits (for the
    // other computer, or a network handshake) they are kept, as Windows keeps the
    // messages the original does not read while it waits.
    let mut events: Vec<(u8, bool)> = Vec::new();
    let take_events = |window: &Window, events: &mut Vec<(u8, bool)>| {
        for k in window.get_keys_pressed(KeyRepeat::Yes) {
            if let Some(v) = vk(k) {
                events.push((v, true));
            }
        }
        for k in window.get_keys_released() {
            if let Some(v) = vk(k) {
                events.push((v, false));
            }
        }
    };
    // The key events as the window procedure (lf2_sim::input) writes them: 'd'/'u',
    // plus the typed words "lf2.net" and "herofighter.com" (keys 0xf9 and 0xf8).
    let apply_events = |events: &mut Vec<(u8, bool)>, keys: &mut [u8; 256], g: &mut lf2_sim::fields::Globals| {
        let kb = lf2_sim::world::KEYS;
        for (i, &k) in keys.iter().enumerate() {
            g.m.set_u8(kb + i as u32, k);
        }
        for (v, down) in events.drain(..) {
            if down {
                lf2_sim::input::key_down(g, v);
            } else {
                lf2_sim::input::key_up(g, v);
            }
        }
        for (i, k) in keys.iter_mut().enumerate() {
            *k = g.m.u8(kb + i as u32);
        }
    };
    while window.is_open() {
        take_events(&window, &mut events);
        // a frame waiting for the other computer's packet, or a handshake
        let mut stepped = false;
        let over = quick && (world.g.match_phase() != 0 || world.g.end_timer() > 0x15d);
        if world.net_waiting {
            match net.poll_exchange(&mut world) {
                Some(p) => {
                    world.net_resume(p.as_deref());
                    stepped = true;
                }
                None => {
                    window.update();
                    std::thread::sleep(Duration::from_millis(1));
                    continue;
                }
            }
        } else if net.handshake_busy(&mut world) {
            window.update();
            std::thread::sleep(Duration::from_millis(5));
            continue;
        }
        if let Some(e) = net.take_error() {
            platform::message(&e);
        }
        if !stepped {
            // Ctrl+V pastes a room code on the client screen
            let ctrl = window.is_key_down(Key::LeftCtrl) || window.is_key_down(Key::RightCtrl);
            if ctrl && events.contains(&(b'V', true)) {
                if let Some(text) = platform::clipboard_text() {
                    lf2_sim::menu::title::paste_room_code(&mut world, &text);
                }
            }
            apply_events(&mut events, &mut keys, &mut world.g);
            if let Some((mx, my)) = window.get_mouse_pos(minifb::MouseMode::Clamp) {
                world.g.m.set_i32(lf2_sim::menu::main_menu::MOUSE_X, mx as i32);
                world.g.m.set_i32(lf2_sim::menu::main_menu::MOUSE_Y, my as i32);
                frame.cursor = Some((mx as i32, my as i32));
            }
            let down = window.get_mouse_down(minifb::MouseButton::Left);
            // as the window procedure (0x43b3d0): 1 on button down, 0 on button up
            if down && !mouse_was_down {
                world.g.m.set_i32(lf2_sim::menu::main_menu::MOUSE_CLICK, 1);
            }
            if !down && mouse_was_down {
                world.g.m.set_i32(lf2_sim::menu::main_menu::MOUSE_CLICK, 0);
            }
            mouse_was_down = down;
            world.time_ms = start.elapsed().as_millis() as u32;
            world.local_time = platform::local_time();
            if over {
                if keys[0x0d] == b'd' || keys[0x1b] == b'd' {
                    if keys[0x1b] == b'd' {
                        break;
                    }
                    world = World::new_vs_match(gd.clone(), &exe, &setup, &controls);
                    keys = [b'u'; 256];
                }
            } else {
                if fps == 60 {
                    tween.before_tick(&world);
                }
                world.set_keys(&keys);
                world.step();
                if world.net_waiting {
                    // the other half of the frame comes with the other computer's packet
                    net.begin_exchange(&mut world);
                    continue;
                }
                stepped = true;
            }
        }
        if stepped {
            if fps == 60 {
                tween.after_tick(&world);
            }
            // the frame writes 'u' over keys it consumed (the F-keys)
            keys.copy_from_slice(&world.g.m.bytes[(0x455378 - 0x44d000)..(0x455378 - 0x44d000 + 256)]);
            audio.play(&world.sounds, world.g.m.i32(0x44d000));
            audio.play_buffers(&world.buffer_sounds);
            audio.music(&world.music, world.g.m.i32(0x44d000));
            if world.unported_hits.len() > warned {
                for s in &world.unported_hits[warned..] {
                    eprintln!("note: reached code that is not ported yet: {}", s);
                }
                warned = world.unported_hits.len();
            }
        }
        for r in std::mem::take(&mut world.requests) {
            use lf2_sim::menu::main_menu::MenuRequest;
            match r {
                MenuRequest::Quit => return,
                MenuRequest::OpenUrl(u) => {
                    if u.starts_with("http") {
                        platform::open(u)
                    } else {
                        platform::open(&lf2_data::resolve(&dir, u.as_bytes()).to_string_lossy())
                    }
                }
                MenuRequest::OpenAdUrl(at) => {
                    let s: Vec<u8> = (0..100).map(|k| world.g.m.u8(at + k)).take_while(|&c| c != 0).collect();
                    platform::open(&String::from_utf8_lossy(&s));
                }
                MenuRequest::Message(m) => platform::message(m),
                MenuRequest::NetHost => net.host(),
                MenuRequest::NetConnect => net.connect(&mut world),
                MenuRequest::NetClose => net.close(),
                MenuRequest::OpenRecording => {
                    open_recording(&mut world, &dir, &exe);
                    tween.clear();
                }
                MenuRequest::SaveControls(text) => {
                    if let Err(e) = std::fs::write(lf2_data::resolve(&dir, b"data\\control.txt"), text) {
                        eprintln!("cannot write data/control.txt: {}", e);
                    }
                }
                MenuRequest::SaveRecording(rec) => {
                    let path = lf2_data::resolve(&dir, b"recording").join(world.recording_file_name());
                    let file = lf2_data::lfr::encode(&rec, &exe);
                    match std::fs::write(&path, file) {
                        Ok(()) => eprintln!("saved {}", path.display()),
                        Err(e) => eprintln!("cannot write {}: {}", path.display(), e),
                    }
                }
            }
        }
        let over_text = |frame: &mut render::Frame| {
            if over {
                frame.text(&assets, "WORDS0", 290, 260, "Press Enter to play again");
                frame.text(&assets, "WORDS0", 335, 280, "Esc to quit");
            }
        };
        let period = if world.g.m.i32(0x44d02c) == 0 { 3 } else { 0x21 };
        // At 60 pictures a second (not in F5 fast mode) the pictures run one tick
        // behind: tick N's picture now, and half a period later tick N's picture with
        // the objects and the camera halfway to tick N+1 (render::Tween).
        let smooth = fps == 60 && period == 0x21 && stepped && tween.ready();
        if smooth {
            renderer.draw_tween(&mut tween, 0.0, true, &assets, &mut frame);
        } else {
            renderer.draw_frame(&world, &assets, &mut frame);
        }
        over_text(&mut frame);
        window.update_with_buffer(&frame.px, render::W as usize, render::H as usize).expect("present");
        if smooth {
            let half = next + Duration::from_micros(period * 500);
            let now = Instant::now();
            if half > now {
                std::thread::sleep(half - now);
            }
            take_events(&window, &mut events);
            // the cursor follows the mouse on this picture too
            if let Some((mx, my)) = window.get_mouse_pos(minifb::MouseMode::Clamp) {
                frame.cursor = Some((mx as i32, my as i32));
            }
            renderer.draw_tween(&mut tween, 0.5, false, &assets, &mut frame);
            over_text(&mut frame);
            window.update_with_buffer(&frame.px, render::W as usize, render::H as usize).expect("present");
        }
        next += Duration::from_millis(period);
        let now = Instant::now();
        if next > now {
            std::thread::sleep(next - now);
        } else if now - next > Duration::from_millis(100) {
            next = now;
        }
    }
}
