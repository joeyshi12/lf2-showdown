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
    /// The last tick kept for pictures between ticks (see `render::Tween`).
    tween: render::Tween,
    /// The network game's byte stream; the page carries it over WebRTC.
    net: Option<lf2_sim::net::NetSession>,
    /// The tick waiting for the other computer's packet was a smooth one.
    net_smooth: bool,
    net_log: Vec<String>,
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
            tween: render::Tween::new(),
            net: None,
            net_smooth: false,
            net_log: Vec::new(),
        })
    }

    /// Run one tick. With `smooth`, the tick is kept for `draw_smooth` and nothing is
    /// drawn; without it the tick's picture is drawn for `picture`. `time_ms` is the
    /// time since start, `local_time` year, month, day, hour, minute, second. Returns
    /// the frame period in milliseconds (33, or 3 in fast mode), or 0 when no tick
    /// ran because a network game waits for the other computer: call again later.
    pub fn step(&mut self, smooth: bool, mouse_x: i32, mouse_y: i32, mouse_down: bool, time_ms: u32, local_time: &[u16]) -> u32 {
        use lf2_sim::menu::main_menu::{MOUSE_CLICK, MOUSE_X, MOUSE_Y};
        // a tick waiting for the other computer's packet
        if self.world.net_waiting {
            let Some(p) = self.net.as_mut().and_then(|n| n.poll_exchange()) else { return 0 };
            self.world.net_resume(p.as_deref());
            self.log_exchange();
            return self.finish(self.net_smooth);
        }
        if let Some(n) = self.net.as_mut() {
            if n.handshake_busy(&mut self.world) {
                return 0;
            }
        }
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
        w.set_keys(&self.keys);
        w.time_ms = time_ms;
        if local_time.len() >= 6 {
            w.local_time.copy_from_slice(&local_time[..6]);
        }
        if smooth {
            self.tween.before_tick(w);
        }
        w.step();
        if w.net_waiting {
            if let Some(n) = self.net.as_mut() {
                n.begin_exchange(w);
            }
            self.net_smooth = smooth;
            return 0;
        }
        self.finish(smooth)
    }

    /// For tests (examples/net_peer prints the same): a digest of every object's
    /// position, hp and frame after each packet exchange.
    fn log_exchange(&mut self) {
        let w = &self.world;
        let mut h: u64 = 0;
        for i in 0..400 {
            if w.exists(i) {
                let o = &w.objs[i];
                for v in [o.x().to_bits(), o.y().to_bits(), o.z().to_bits(), o.hp() as u64, o.frame() as u64] {
                    h = h.wrapping_mul(1_000_003).wrapping_add(v ^ i as u64);
                }
            }
        }
        let n = self.net_log.len();
        self.net_log.push(format!("{} phase {} digest {:016x}", n, w.g.match_phase(), h));
    }

    fn finish(&mut self, smooth: bool) -> u32 {
        let w = &mut self.world;
        if smooth {
            self.tween.after_tick(w);
        } else {
            self.tween.clear();
        }
        // the frame writes 'u' over keys it consumed (the F-keys)
        for (i, k) in self.keys.iter_mut().enumerate() {
            *k = w.g.m.u8(lf2_sim::world::KEYS + i as u32);
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

    /// The data channel opened.
    pub fn net_connected(&mut self) {
        if let Some(n) = self.net.as_mut() {
            n.on_connected();
        }
    }

    /// Bytes from the other computer.
    pub fn net_data(&mut self, bytes: &[u8]) {
        if let Some(n) = self.net.as_mut() {
            n.on_data(bytes);
        }
    }

    /// The link failed or closed.
    pub fn net_closed(&mut self, message: &str) {
        if let Some(n) = self.net.as_mut() {
            n.on_closed(&mut self.world, message);
        }
    }

    /// Bytes to send to the other computer (all of them since the last call).
    pub fn net_outgoing(&mut self) -> Vec<u8> {
        self.net.as_mut().map(|n| n.take_outgoing().concat()).unwrap_or_default()
    }

    /// The exchange digests so far, one per line (see `log_exchange`).
    pub fn net_log(&self) -> String {
        self.net_log.join("\n")
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
        let kb = lf2_sim::world::KEYS;
        for (i, &k) in self.keys.iter().enumerate() {
            self.world.g.m.set_u8(kb + i as u32, k);
        }
        lf2_sim::input::key_down(&mut self.world.g, vk);
        for (i, k) in self.keys.iter_mut().enumerate() {
            *k = self.world.g.m.u8(kb + i as u32);
        }
    }

    pub fn key_up(&mut self, vk: u8) {
        self.keys[vk as usize] = b'u';
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
