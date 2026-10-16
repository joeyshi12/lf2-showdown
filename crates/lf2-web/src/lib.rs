//! WebAssembly interface used by `web/index.html`.

use lf2_app::{assets::Assets, render};
use lf2_sim::World;
use std::cell::RefCell;
use std::sync::Arc;
use wasm_bindgen::prelude::*;

thread_local! {
    static FILES: RefCell<Vec<(String, Vec<u8>)>> = const { RefCell::new(Vec::new()) };
}

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
    events: Vec<(u8, bool)>,
    tween: render::Tween,
    net: Option<lf2_sim::net::NetSession>,
    /// The controllers as `set_gamepad` last gave them.
    pads: [lf2_sim::input::Joystick; lf2_sim::input::JOYSTICK_COUNT],
    joysticks: lf2_app::gamepad::Joysticks,
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
            pads: Default::default(),
            joysticks: Default::default(),
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
        self.joysticks.write(&mut w.g, &self.pads);
        w.time_ms = time_ms;
        if local_time.len() >= 6 {
            w.local_time.copy_from_slice(&local_time[..6]);
        }
        if smooth {
            self.tween.before_tick(w);
        }
        if let Some(n) = self.net.as_mut() {
            n.set_clock(time_ms);
        }
        match self.net.as_mut().filter(|n| n.in_game()) {
            Some(n) => {
                n.advance(w, &mut self.keys);
            }
            None => {
                w.set_keys(&self.keys);
                w.step();
                // The frame writes `u` over consumed F-keys.
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

    /// Controller `n` (0 for joystick 1, 1 for joystick 2) for the next ticks: bits 0
    /// to 3 up, down, left and right, bits 4 to 7 buttons 0 to 3 (see
    /// `lf2_app::gamepad`).
    pub fn set_gamepad(&mut self, n: u32, bits: u32) {
        let Some(p) = self.pads.get_mut(n as usize) else { return };
        let b = |k: u32| bits & (1 << k) != 0;
        *p = lf2_sim::input::Joystick { up: b(0), down: b(1), left: b(2), right: b(3), buttons: std::array::from_fn(|k| b(4 + k as u32)) };
    }

    pub fn net_host(&mut self) {
        self.net = Some(lf2_sim::net::NetSession::host());
    }

    pub fn net_join(&mut self) {
        self.net = Some(lf2_sim::net::NetSession::join());
    }

    pub fn net_drop(&mut self) {
        self.net = None;
    }

    pub fn net_code(&mut self, code: &str) {
        if let Some(n) = self.net.as_mut() {
            n.on_code(&mut self.world, code);
        }
    }

    /// The host numbers joiners from 1; a joiner calls the host 0.
    pub fn net_connected(&mut self, peer: u32) {
        if let Some(n) = self.net.as_mut() {
            n.on_connected(&mut self.world, peer);
        }
    }

    pub fn net_data(&mut self, peer: u32, bytes: &[u8]) {
        if let Some(n) = self.net.as_mut() {
            n.on_message(&mut self.world, peer, bytes);
        }
    }

    pub fn net_failed(&mut self, message: &str) {
        let Some(n) = self.net.as_mut() else { return };
        if n.is_host() {
            if !n.in_game() {
                self.world.net_connect_failed(message);
            }
        } else {
            n.on_closed(&mut self.world, 0, message);
        }
    }

    pub fn net_closed(&mut self, peer: u32, message: &str) {
        if let Some(n) = self.net.as_mut() {
            n.on_closed(&mut self.world, peer, message);
        }
    }

    /// The messages to send since the last call, each as the peer (1 byte), 1 if it
    /// goes on the channel that may lose messages else 0 (1 byte), the length (4
    /// bytes, little-endian) and the message.
    pub fn net_outgoing(&mut self) -> Vec<u8> {
        let mut out = Vec::new();
        for (peer, m) in self.net.as_mut().map(|n| n.take_outgoing()).unwrap_or_default() {
            out.push(peer as u8);
            out.push(lf2_sim::net::is_unreliable(&m) as u8);
            out.extend_from_slice(&(m.len() as u32).to_le_bytes());
            out.extend_from_slice(&m);
        }
        out
    }

    pub fn net_stats(&self) -> String {
        match self.net.as_ref().and_then(|n| n.rollback()) {
            Some(r) => format!("{} {} {} {}", r.frame(), r.rollbacks, r.resimulated, r.stalls),
            None => String::new(),
        }
    }

    pub fn net_players(&self) -> u32 {
        self.world.net_ui.players.len() as u32
    }

    pub fn net_log(&self) -> String {
        self.net.as_ref().map(|n| n.log.iter().map(|(f, h)| format!("{} {:016x}", f, h)).collect::<Vec<_>>().join("\n")).unwrap_or_default()
    }

    /// With `smooth` ticks: draw the picture `t` (0..1) of the way from the tick
    /// before the last one towards the last one (pictures run one tick behind the
    /// simulation, see `render::Tween`). `first` is true for the first picture after
    /// a tick; the mouse position is where the menus' cursor is drawn. Returns false
    /// when no tick is kept yet; `picture` then holds nothing new.
    pub fn draw_smooth(&mut self, t: f64, first: bool, mouse_x: i32, mouse_y: i32) -> bool {
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

    pub fn paste(&mut self, text: &str) {
        lf2_sim::menu::net_menu::paste_room_code(&mut self.world, text);
    }

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

    pub fn music(&self) -> String {
        self.world.music.iter().map(|m| m.as_ref().map(|f| String::from_utf8_lossy(f).into_owned()).unwrap_or_default() + "\n").collect()
    }

    pub fn volume(&self) -> i32 {
        self.world.g.m.i32(0x44d000)
    }

    /// Lines consumed by the page: `url`, `message`, `quit`, `open-recording`,
    /// `save-recording`, `save-controls`, `net-*` and `copy`.
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
                MenuRequest::NetHost => out += "net-host\n",
                MenuRequest::NetConnect => out += &format!("net-join {}\n", self.world.room_code_typed()),
                MenuRequest::NetClose => out += "net-close\n",
                MenuRequest::CopyText(t) => out += &format!("copy {}\n", t),
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

    pub fn written_file(&self, path: &str) -> Vec<u8> {
        lf2_data::vfs::read(std::path::Path::new(path)).unwrap_or_default()
    }

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
