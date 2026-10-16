//! The game loop of the desktop program and the Android app.

use crate::touch::TouchPad;
use crate::{assets, audio, platform, render, window::Display};

use lf2_sim::setup::{Fighter, VsSetup, DEFAULT_CONTROLS};
use lf2_sim::World;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

pub struct Options {
    /// The game folder (the one with data/data.txt).
    pub dir: PathBuf,
    /// With either fighter given, start a match of the two instead of the title.
    pub p1: Option<String>,
    pub p2: Option<String>,
    pub bg: Option<i32>,
    pub seed: Option<u32>,
    /// 60 or 30 pictures a second.
    pub fps: u32,
    pub fullscreen: bool,
    /// Show the on-screen controls (`touch::TouchPad`).
    pub touch: bool,
    /// Run this many ticks without a window and write the last picture to this file.
    pub dump: Option<(u32, String)>,
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

fn fighter_index(gd: &lf2_data::GameData, name: &str) -> Option<usize> {
    gd.objects.iter().position(|o| o.kind == 0 && String::from_utf8_lossy(&o.name).eq_ignore_ascii_case(name))
}

/// Run the game until it quits or its window is closed. `display` opens the window
/// (with full screen or not).
pub fn run(o: Options, display: impl FnOnce(bool) -> Display) {
    let Options { dir, p1, p2, bg, seed, fps, fullscreen, touch, dump } = o;
    let mut tween = render::Tween::new();
    let exe: Vec<u8> = match crate::exe_image::built_in() {
        Some(img) => img.to_vec(),
        None => {
            let exe = lf2_data::read_exe(&dir).unwrap_or_default();
            if !crate::exe_image::is_original(&exe) {
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
    frame.hide_cursor = touch;
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
    let mut window = display(fullscreen);
    if touch {
        window.set_touch_controls(Some(TouchPad::keys_of(&file_controls)));
    }
    let mut hidden = false;
    let mut keys = [b'u'; 256];
    let mut next = Instant::now();
    let mut warned = 0;
    let mut mouse_was_down = false;
    let mut net = crate::net::Net::default();
    let mut controllers = crate::gamepad::Controllers::new();
    let mut joysticks = crate::gamepad::Joysticks::default();
    let mut events: Vec<(u8, bool)> = Vec::new();
    let take_events = |window: &mut Display, events: &mut Vec<(u8, bool)>| {
        window.pump();
        window.take_keys(events);
    };
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
        take_events(&mut window, &mut events);
        if !window.is_visible() {
            // an Android app in the back: no ticks, no sound
            if !hidden {
                audio.set_paused(true);
                hidden = true;
            }
            std::thread::sleep(Duration::from_millis(50));
            next = Instant::now();
            continue;
        }
        if hidden {
            audio.set_paused(false);
            hidden = false;
        }
        let mut stepped = false;
        let over = quick && (world.g.match_phase() != 0 || world.g.end_timer() > 0x15d);
        net.pump(&mut world);
        {
            if window.ctrl_down() && events.contains(&(b'V', true)) {
                if let Some(text) = platform::clipboard_text() {
                    lf2_sim::menu::net_menu::paste_room_code(&mut world, &text);
                }
            }
            apply_events(&mut events, &mut keys, &mut world.g);
            joysticks.write(&mut world.g, &controllers.poll());
            if let Some((mx, my)) = window.mouse() {
                world.g.m.set_i32(lf2_sim::menu::main_menu::MOUSE_X, mx);
                world.g.m.set_i32(lf2_sim::menu::main_menu::MOUSE_Y, my);
                frame.cursor = Some((mx, my));
            }
            let down = window.mouse_down();
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
                if net.in_game() {
                    net.advance(&mut world, &mut keys);
                } else {
                    world.set_keys(&keys);
                    world.step();
                    // the frame writes 'u' over keys it consumed (the F-keys)
                    keys.copy_from_slice(&world.g.m.bytes[(0x455378 - 0x44d000)..(0x455378 - 0x44d000 + 256)]);
                }
                stepped = true;
            }
        }
        if stepped {
            if fps == 60 {
                tween.after_tick(&world);
            }
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
                MenuRequest::NetStart => net.start(&mut world),
                MenuRequest::CopyText(t) => platform::copy_text(&t),
                MenuRequest::OpenRecording => {
                    open_recording(&mut world, &dir, &exe);
                    tween.clear();
                }
                MenuRequest::SaveControls(text) => {
                    if let Err(e) = std::fs::write(lf2_data::resolve(&dir, b"data\\control.txt"), &text) {
                        eprintln!("cannot write data/control.txt: {}", e);
                    }
                    if touch {
                        let c = lf2_data::controls::parse(&text);
                        window.set_touch_controls(Some(TouchPad::keys_of(&c)));
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
        window.present(&frame.px);
        if smooth {
            let half = next + Duration::from_micros(period * 500);
            let now = Instant::now();
            if half > now {
                std::thread::sleep(half - now);
            }
            // the keys wait in the window for the next tick's take_keys
            window.pump();
            if let Some((mx, my)) = window.mouse() {
                frame.cursor = Some((mx, my));
            }
            renderer.draw_tween(&mut tween, 0.5, false, &assets, &mut frame);
            over_text(&mut frame);
            window.present(&frame.px);
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
