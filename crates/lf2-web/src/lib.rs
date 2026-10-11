//! The web build: the simulation and the renderer compiled to WebAssembly. The page
//! (`web/index.html`) hands over the game folder's files, then each frame the keys
//! and the mouse; it draws the picture and plays the sounds this module reports.

use lf2_app::{assets::Assets, render};
use lf2_sim::World;
use std::cell::RefCell;
use std::sync::Arc;
use wasm_bindgen::prelude::*;

thread_local! {
    static FILES: RefCell<Vec<(String, Vec<u8>)>> = const { RefCell::new(Vec::new()) };
}

/// Give one file of the game folder (`path` relative to it, e.g. "data/davis.dat").
#[wasm_bindgen]
pub fn add_file(path: &str, bytes: &[u8]) {
    FILES.with(|f| f.borrow_mut().push((path.to_string(), bytes.to_vec())));
}

#[wasm_bindgen]
pub struct Game {
    world: World,
    assets: Assets,
    renderer: render::Renderer,
    frame: render::Frame,
    rgba: Vec<u8>,
    exe: Vec<u8>,
    mouse_was_down: bool,
    keys: [u8; 256],
    /// Key events (virtual-key code, down) since the last frame, given to the key
    /// array at the start of the next one.
    events: Vec<(u8, bool)>,
    /// The last tick kept for pictures between ticks (see `render::Tween`).
    tween: render::Tween,
    /// The network game; the page carries its messages over WebRTC.
    net: Option<lf2_sim::net::NetSession>,
}

impl Game {
    fn to_rgba(&mut self) {
        for (i, p) in self.frame.px.iter().enumerate() {
            self.rgba[4 * i..4 * i + 4].copy_from_slice(&[(p >> 16) as u8, (p >> 8) as u8, *p as u8, 255]);
        }
    }
}

#[wasm_bindgen]
impl Game {
    /// Load the files given with `add_file` and start at the title screen.
    #[wasm_bindgen(constructor)]
    pub fn new(seed: u32) -> Result<Game, JsError> {
        let files = FILES.with(|f| std::mem::take(&mut *f.borrow_mut()));
        lf2_data::vfs::install(files);
        let dir = std::path::Path::new("");
        let exe = match lf2_app::exe_image::built_in() {
            Some(img) => img.to_vec(),
            None => lf2_data::vfs::read(&lf2_data::resolve(dir, b"lf2.exe")).map_err(|e| JsError::new(&format!("lf2.exe: {}", e)))?,
        };
        let gd = Arc::new(lf2_data::GameData::load(dir).map_err(|e| JsError::new(&format!("{:?}", e)))?);
        let assets = Assets::load(&gd, &exe);
        let controls = lf2_data::controls::load(dir).unwrap_or_default();
        // no advertisements (see the desktop app)
        let mut world = World::new_title(gd.clone(), &exe, &controls, None, seed);
        world.ads_removed = true;
        let renderer = render::Renderer::new(&gd);
        Ok(Game {
            world,
            assets,
            renderer,
            frame: render::Frame::new(),
            rgba: vec![0; (render::W * render::H * 4) as usize],
            exe,
            mouse_was_down: false,
            keys: [b'u'; 256],
            events: Vec::new(),
            tween: render::Tween::new(),
            net: None,
        })
    }

    /// Run one tick. With `smooth`, the tick is kept for `draw_smooth` and nothing is
    /// drawn; without it the tick's picture is drawn for `picture`. `time_ms` is the
    /// time since start, `local_time` year, month, day, hour, minute, second. Returns
    /// the frame period in milliseconds (33, or 3 in fast mode). In a network game
    /// the frame may run no tick, or two (see `NetSession::advance`).
    pub fn step(&mut self, smooth: bool, mouse_x: i32, mouse_y: i32, mouse_down: bool, time_ms: u32, local_time: &[u16]) -> u32 {
        use lf2_sim::menu::main_menu::{MOUSE_CLICK, MOUSE_X, MOUSE_Y};
        let w = &mut self.world;
        w.g.m.set_i32(MOUSE_X, mouse_x);
        w.g.m.set_i32(MOUSE_Y, mouse_y);
        if mouse_down && !self.mouse_was_down {
            w.g.m.set_i32(MOUSE_CLICK, 1);
        }
        if !mouse_down && self.mouse_was_down {
            w.g.m.set_i32(MOUSE_CLICK, 0);
        }
        self.mouse_was_down = mouse_down;
        let kb = lf2_sim::world::KEYS;
        for (i, &k) in self.keys.iter().enumerate() {
            w.g.m.set_u8(kb + i as u32, k);
        }
        for (vk, down) in self.events.drain(..) {
            if down {
                lf2_sim::input::key_down(&mut w.g, vk);
            } else {
                lf2_sim::input::key_up(&mut w.g, vk);
            }
        }
        for (i, k) in self.keys.iter_mut().enumerate() {
            *k = w.g.m.u8(kb + i as u32);
        }
        w.time_ms = time_ms;
        if local_time.len() >= 6 {
            w.local_time.copy_from_slice(&local_time[..6]);
        }
        if smooth {
            self.tween.before_tick(w);
        }
        match self.net.as_mut().filter(|n| n.in_game()) {
            Some(n) => {
                n.advance(w, &mut self.keys);
            }
            None => {
                w.set_keys(&self.keys);
                w.step();
                // the frame writes 'u' over keys it consumed (the F-keys)
                for (i, k) in self.keys.iter_mut().enumerate() {
                    *k = w.g.m.u8(kb + i as u32);
                }
            }
        }
        if smooth {
            self.tween.after_tick(w);
        } else {
            self.tween.clear();
        }
        let period = if w.g.m.i32(0x44d02c) == 0 { 3 } else { 0x21 };
        if !smooth {
            self.renderer.draw_frame(w, &self.assets, &mut self.frame);
            self.to_rgba();
        }
        period
    }

    /// Network game: the page asked the rendezvous server for a room (after the
    /// "net-host" request).
    pub fn net_host(&mut self) {
        self.net = Some(lf2_sim::net::NetSession::host());
    }

    /// Network game: the page joins a room (after "net-join CODE").
    pub fn net_join(&mut self) {
        self.net = Some(lf2_sim::net::NetSession::join());
    }

    /// Network game: the link is gone (after "net-close", or the page gave up).
    pub fn net_drop(&mut self) {
        self.net = None;
    }

    /// The rendezvous server gave the room code.
    pub fn net_code(&mut self, code: &str) {
        if let Some(n) = self.net.as_mut() {
            n.on_code(&mut self.world, code);
        }
    }

    /// A data channel opened. `peer` is the page's number for the link: the
    /// host's joiners count from 1, a joiner's host is 0.
    pub fn net_connected(&mut self, peer: u32) {
        if let Some(n) = self.net.as_mut() {
            n.on_connected(&mut self.world, peer);
        }
    }

    /// A message from a link.
    pub fn net_data(&mut self, peer: u32, bytes: &[u8]) {
        if let Some(n) = self.net.as_mut() {
            n.on_message(&mut self.world, peer, bytes);
        }
    }

    /// The rendezvous server failed, or a joiner lost its link to the host.
    pub fn net_failed(&mut self, message: &str) {
        let Some(n) = self.net.as_mut() else { return };
        if n.is_host() {
            if !n.in_game() {
                n.error = Some(message.to_string());
                self.world.net_connect_failed();
            }
        } else {
            n.on_closed(&mut self.world, 0, message);
        }
    }

    /// A link failed or closed.
    pub fn net_closed(&mut self, peer: u32, message: &str) {
        if let Some(n) = self.net.as_mut() {
            n.on_closed(&mut self.world, peer, message);
        }
    }

    /// The messages to send since the last call, each as the peer (1 byte), the
    /// length (4 bytes, little-endian) and the message.
    pub fn net_outgoing(&mut self) -> Vec<u8> {
        let mut out = Vec::new();
        for (peer, m) in self.net.as_mut().map(|n| n.take_outgoing()).unwrap_or_default() {
            out.push(peer as u8);
            out.extend_from_slice(&(m.len() as u32).to_le_bytes());
            out.extend_from_slice(&m);
        }
        out
    }

    /// For tests: "frames rollbacks resimulated waits" of the network game.
    pub fn net_stats(&self) -> String {
        match self.net.as_ref().and_then(|n| n.rollback()) {
            Some(r) => format!("{} {} {} {}", r.frame(), r.rollbacks, r.resimulated, r.stalls),
            None => String::new(),
        }
    }

    /// The number of computers in the room the waiting screen lists.
    pub fn net_players(&self) -> u32 {
        self.world.net_players.len() as u32
    }

    /// For tests: the state hashes of the network game so far, "frame hash" per
    /// line (examples/net_peer prints the same).
    pub fn net_log(&self) -> String {
        self.net.as_ref().map(|n| n.log.iter().map(|(f, h)| format!("{} {:016x}", f, h)).collect::<Vec<_>>().join("\n")).unwrap_or_default()
    }

    /// A message for the player when the connection failed, else "".
    pub fn net_error(&mut self) -> String {
        self.net.as_mut().and_then(|n| n.error.take()).unwrap_or_default()
    }

    /// With `smooth` ticks: draw the picture `t` (0..1) of the way from the tick
    /// before the last one towards the last one (pictures run one tick behind the
    /// simulation, see `render::Tween`). `first` is true for the first picture after
    /// a tick; the mouse position is where the menus' cursor is drawn. Returns false
    /// when no tick is kept yet; `picture` then holds nothing new.
    pub fn draw_smooth(&mut self, t: f64, first: bool, mouse_x: i32, mouse_y: i32) -> bool {
        // the menus' cursor follows the mouse on every picture
        self.frame.cursor = Some((mouse_x, mouse_y));
        if !self.tween.ready() {
            self.renderer.draw_frame(&self.world, &self.assets, &mut self.frame);
            self.to_rgba();
            return true;
        }
        self.renderer.draw_tween(&mut self.tween, t, first, &self.assets, &mut self.frame);
        self.to_rgba();
        true
    }

    /// A key went down (also for the browser's key repeat), by virtual-key code.
    /// As the window procedure does (lf2_sim::input): also follows the typed words
    /// "lf2.net" and "herofighter.com".
    pub fn key_down(&mut self, vk: u8) {
        self.events.push((vk, true));
    }

    pub fn key_up(&mut self, vk: u8) {
        self.events.push((vk, false));
    }

    /// Text pasted from the clipboard: goes into the room code on the client screen.
    pub fn paste(&mut self, text: &str) {
        lf2_sim::menu::title::paste_room_code(&mut self.world, text);
    }

    /// The picture last drawn (by `step` or `draw_smooth`): 794 x 550 RGBA.
    pub fn picture(&self) -> Vec<u8> {
        self.rgba.clone()
    }

    /// The sounds of the last frame, as "file|volume|pan" (volume and pan in
    /// hundredths of a decibel, as the original gives DirectSound), one per line.
    pub fn sounds(&self) -> String {
        let w = &self.world;
        let master = w.g.m.i32(0x44d000);
        let mut out = String::new();
        for e in &w.sounds {
            let name = if e.system {
                format!("data\\{}.wav", lf2_app::sound_names::SYSTEM_SOUNDS.get(e.id).copied().unwrap_or(""))
            } else {
                w.gd.sounds.get(e.id).map(|n| String::from_utf8_lossy(n).into_owned()).unwrap_or_default()
            };
            let vol = (e.left + e.right).min(100);
            if vol < 1 || master < 1 {
                continue;
            }
            let volume_cb = ((master - 100) * 0xed8) / 100 + ((vol - 100) * 2000) / 100;
            let pan_cb = ((e.right - e.left) * 0x5dc) / 100;
            out += &format!("{}|{}|{}\n", name, volume_cb, pan_cb);
        }
        for n in &w.buffer_sounds {
            out += &format!("{}|0|0\n", n);
        }
        out
    }

    /// Music changes of the last frame: a bgm file name, or an empty line for stop.
    pub fn music(&self) -> String {
        self.world.music.iter().map(|m| m.as_ref().map(|f| String::from_utf8_lossy(f).into_owned()).unwrap_or_default() + "\n").collect()
    }

    /// The master volume (0..100, F11/F12).
    pub fn volume(&self) -> i32 {
        self.world.g.m.i32(0x44d000)
    }

    /// Requests of the last frame for the page: "url <address>", "message <text>",
    /// "quit", "open-recording", "save-recording <file name>" (then call
    /// `recording_file`), "save-controls" (then call `controls_file`).
    pub fn requests(&mut self) -> String {
        use lf2_sim::menu::main_menu::MenuRequest;
        let mut out = String::new();
        for r in std::mem::take(&mut self.world.requests) {
            match r {
                MenuRequest::Quit => out += "quit\n",
                MenuRequest::OpenUrl(u) => out += &format!("url {}\n", u),
                MenuRequest::OpenAdUrl(at) => {
                    let s: Vec<u8> = (0..100).map(|k| self.world.g.m.u8(at + k)).take_while(|&c| c != 0).collect();
                    out += &format!("url {}\n", String::from_utf8_lossy(&s));
                }
                MenuRequest::Message(m) => out += &format!("message {}\n", m),
                // the page does the WebRTC link (see net_* below)
                MenuRequest::NetHost => out += "net-host\n",
                MenuRequest::NetConnect => out += &format!("net-join {}\n", self.world.room_code_typed()),
                MenuRequest::NetClose => out += "net-close\n",
                MenuRequest::NetStart => {
                    if let Some(n) = self.net.as_mut() {
                        n.start(&mut self.world);
                    }
                }
                MenuRequest::OpenRecording => out += "open-recording\n",
                MenuRequest::SaveControls(t) => {
                    let _ = lf2_data::vfs::write(&lf2_data::resolve(std::path::Path::new(""), b"data\\control.txt"), &t);
                    out += "save-controls\n";
                }
                MenuRequest::SaveRecording(rec) => {
                    let name = self.world.recording_file_name();
                    let file = lf2_data::lfr::encode(&rec, &self.exe);
                    let _ = lf2_data::vfs::write(std::path::Path::new(&format!("recording/{}", name)), &file);
                    out += &format!("save-recording {}\n", name);
                }
            }
        }
        out
    }

    /// A file written by the game (a saved recording or data/control.txt).
    pub fn written_file(&self, path: &str) -> Vec<u8> {
        lf2_data::vfs::read(std::path::Path::new(path)).unwrap_or_default()
    }

    /// Play a .lfr recording. Returns an empty string, or the original's error text.
    pub fn play_recording(&mut self, file: &[u8]) -> String {
        use lf2_sim::replay::ReplayError;
        let rec = match lf2_data::lfr::decode(file, &self.exe) {
            Ok(r) => r,
            Err(_) => return "Loading error!  Recording file may be corrupted!!".into(),
        };
        self.tween.clear();
        match self.world.load_replay(rec) {
            Ok(()) => String::new(),
            Err(ReplayError::Corrupt) => "Loading error!  Recording file may be corrupted!!".into(),
            Err(ReplayError::DataFiles) => "Error!  Recording file are recorded in a LF2 with some data files (character or stage files) different from yours.  Your LF2 has to use the same set of data files in order in replay this recording file!".into(),
            Err(ReplayError::Version(v)) => {
                let theirs = if v == 0x1e { "v2.0" } else if v > self.world.g.m.i32(0x44d03c) { "a newer version" } else { "" };
                format!("Version error.  Recording file are recorded in {}. (your LF2 is v2.0)!", theirs)
            }
        }
    }
}
