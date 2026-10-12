//! The main menu (match_phase 10): `recording_menus` (0x431d10) with `key_name`
//! (0x422b00), `load_menu_background` (0x423840) and `draw_main_menu` (0x423b00).
//!
//! The menu cursor is the game mode global (0x451160): 0 VS, 1 stage, 2 1 on 1
//! championship, 3 2 on 2 championship, 4 battle, 5 demo, 6 playback, 7 quit.

use crate::world::World;

pub const MOUSE_X: u32 = 0x4546f0;
pub const MOUSE_Y: u32 = 0x453cdc;
pub const MOUSE_CLICK: u32 = 0x457580;
/// Vertical offset of the main menu (room for the ad banner).
pub const MENU_Y: u32 = 0x453da4;
/// Menu key presses of the frame from any of objects 0..7 (set by `menu_keys`).
pub const KEY_UP: u32 = 0x4513a4;
pub const KEY_DOWN: u32 = 0x4513a8;
pub const KEY_LEFT: u32 = 0x4513ac;
pub const KEY_RIGHT: u32 = 0x4513b0;
pub const KEY_ATTACK: u32 = 0x4513b4;
pub const KEY_JUMP: u32 = 0x4513b8;
pub const KEY_DEFEND: u32 = 0x4513bc;
/// Frames left to show the key settings over the main menu.
const SHOW_KEYS: u32 = 0x4513c0;
/// The mouse button as it was in the previous menu frame.
const PREV_CLICK: u32 = 0x4513c4;
const MENU_BACK: u32 = 0x4511ac;
/// Which ad shows: a position in the ad timeline (-1: pick one from the clock).
const AD_INDEX: u32 = 0x44d780;
/// The ad picture (from `sprite\\sys\\adN.bmp`), 0 when there is none.
pub const AD_SHEET: u32 = 0x458420;
const AD_SHOWN: u32 = 0x458418;
const AD_ANIM: u32 = 0x45841c;
/// The `ta` lines of adN.txt (pictures on the right): start, end, length, url.
const TA_START: u32 = 0x452928;
const TA_END: u32 = 0x4546d0;
const TA_DUR: u32 = 0x453f50;
pub const TA_URL: u32 = 0x4546f8;
/// The `ba` lines (link boxes on the bottom strip): x, y, w, h, url.
const BA_X: u32 = 0x4583b8;
const BA_Y: u32 = 0x453c08;
const BA_W: u32 = 0x453f70;
const BA_H: u32 = 0x453ce0;
pub const BA_URL: u32 = 0x454a18;
/// The `un` line: the newest version and its url, and this exe's version.
const NEW_VERSION: u32 = 0x4554bc;
pub const NEW_VERSION_URL: u32 = 0x453da8;
const EXE_VERSION: u32 = 0x44d03c;
/// Nonzero in a network game.
pub const NETWORK: u32 = 0x44f1af;

/// Surfaces (globals holding them).
pub const MENU_CLIP: u32 = 0x4511a0;
pub const MENU_CLIP3: u32 = 0x451178;
pub const MENU_CLIP5: u32 = 0x45117c;
pub const CURSOR: u32 = 0x451170;

/// `key_name` (0x422b00): the name of a virtual key for the key-settings display, and
/// the x offset the caller subtracts to centre it.
pub fn key_name(vk: i32) -> (Vec<u8>, i32) {
    let s = |t: &str, w: i32| (t.as_bytes().to_vec(), w);
    if vk == 0x20 {
        return s("Space", 10);
    }
    if vk == 0xd {
        return s("Enter", 10);
    }
    let c = vk as u8;
    let mut out = s("none", 8);
    if (0x41..=0x5a).contains(&vk) {
        out = (vec![c], 0);
    } else if (0x60..=0x69).contains(&vk) {
        out = (b"Keypad: ".iter().copied().chain([c - 0x30]).collect(), 0xf);
    } else {
        let keypad = match vk {
            0x6b => Some("Keypad: +"),
            0x6d => Some("Keypad: -"),
            0x6a => Some("Keypad: *"),
            0x6f => Some("Keypad: /"),
            0x6e => Some("Keypad: ."),
            _ => None,
        };
        if let Some(k) = keypad {
            return s(k, 0xf);
        }
    }
    if (0x30..=0x39).contains(&vk) {
        return (vec![c], 0);
    }
    let one = match vk {
        0xbd => Some(b'-'),
        0xbb => Some(b'='),
        0xdb => Some(b'['),
        0xdd => Some(b']'),
        0xba => Some(b';'),
        0xde => Some(b'\''),
        0xdc => Some(b'\\'),
        0xbc => Some(b','),
        0xbe => Some(b'.'),
        0xbf => Some(b'/'),
        0xc0 => Some(b'`'),
        _ => None,
    };
    if let Some(ch) = one {
        return (vec![ch], 0);
    }
    match vk {
        0x11 => s("Ctrl", 7),
        0x10 => s("Shift", 7),
        9 => s("Tab", 5),
        8 => s("Backspace", 0xd),
        0x2d => s("Insert", 8),
        0x2e => s("Delete", 8),
        0x24 => s("Home", 6),
        0x23 => s("End", 5),
        0x21 => s("PageUp", 10),
        0x22 => s("PageDown", 0xe),
        0x26 => s("Up", 2),
        0x25 => s("Left", 4),
        0x28 => s("Down", 5),
        0x27 => s("Right", 6),
        0x14 => s("CapsLock", 0x10),
        _ => out,
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum MenuRequest {
    OpenRecording,
    Quit,
    OpenUrl(&'static str),
    OpenAdUrl(u32),
    Message(&'static str),
    NetHost,
    NetConnect,
    NetStart,
    CopyText(String),
    NetClose,
    SaveControls(Vec<u8>),
    SaveRecording(Vec<u8>),
}

impl World {
    fn gi(&self, a: u32) -> i32 {
        self.g.m.i32(a)
    }

    fn gs(&mut self, a: u32, v: i32) {
        self.g.m.set_i32(a, v)
    }

    /// `championship_aux` (0x431b70): menu key presses. A key counts once per press
    /// of any of objects 0..7 (`0x451320[i]` remembers whether one was held).
    pub fn menu_keys(&mut self) {
        for a in (KEY_UP..=KEY_DEFEND).step_by(4) {
            self.gs(a, 0);
        }
        for i in 0..8usize {
            let o = &self.objs[i];
            let held = 0x451320 + 4 * i as u32;
            let key = [(o.up(), KEY_UP), (o.down(), KEY_DOWN), (o.right(), KEY_RIGHT), (o.left(), KEY_LEFT),
                (o.attack(), KEY_ATTACK), (o.jump(), KEY_JUMP), (o.defend(), KEY_DEFEND)]
                .into_iter()
                .find(|(k, _)| *k != 0)
                .map(|(_, a)| a);
            match key {
                Some(a) => {
                    if self.gi(held) == 0 {
                        self.gs(a, 1);
                    }
                    self.gs(held, 1);
                }
                None => self.gs(held, 0),
            }
        }
    }

    /// 0x423840: a random `MENU_BACK1..13` (from the clock).
    pub(crate) fn load_menu_background(&mut self) {
        self.menu_back = (self.time_ms % 0xd) as i32 + 1;
        self.gs(MENU_BACK, 1);
    }

    /// 0x423910.
    pub(crate) fn free_menu_background(&mut self) {
        self.gs(MENU_BACK, 0);
    }

    pub(crate) fn draw_menu_background(&mut self, y: i32) {
        let n = self.menu_back;
        self.blit_sheet(crate::draw::Sheet::MenuBack(n), 0, y, -1, true);
    }

    /// The credits and the link at the bottom right of the main menus (drawn by
    /// 0x4246b0 and 0x431d10 alike). `prev_click` is the global that holds the
    /// previous mouse button state.
    pub(crate) fn draw_credits(&mut self, prev_click: u32) {
        let y0 = self.gi(MENU_Y);
        let (mx, my) = (self.gi(MOUSE_X), self.gi(MOUSE_Y));
        let ty = y0 + 0x1eb;
        let y = y0 + 0x1ff;
        self.text(b"by Marti Wong, Starsky Wong", 0x602010, 0xd07750, 0x24f, ty);
        self.text(b"1999-2008, all rights reserved", 0x602010, 0xd07750, 0x24f, y);
        self.text(b"http://www.LittleFighter.com", 0x602010, 0xd07750, 0x24f, y0 + 0x213);
        let clicked = |w: &mut World| {
            if w.gi(prev_click) == 0 && w.gi(MOUSE_CLICK) == 1 {
                w.gs(MOUSE_CLICK, 0);
                w.buffer_sounds.push("data\\m_ok.wav");
                true
            } else {
                false
            }
        };
        if 0x24f < mx && y0 + 0x209 < my && (my < y0 + 0x227 || y0 == 0) {
            self.text(b"http://www.LittleFighter.com", 0x602010, 0xffffff, 0x24f, y0 + 0x213);
            if clicked(self) {
                self.requests.push(MenuRequest::OpenUrl("http://littlefighter.com"));
            }
        }
        if 0x263 < mx && mx < 0x2ae && ty < my && my < y {
            self.text(b"Marti Wong", 0x602010, 0xffffff, 0x263, ty);
            if clicked(self) {
                self.requests.push(MenuRequest::OpenUrl("http://martiwong.com"));
            }
        }
        if 0x2b4 < mx && ty < my && my < y {
            self.text(b"Starsky Wong", 0x602010, 0xffffff, 0x2b6, ty);
            if clicked(self) {
                self.requests.push(MenuRequest::OpenUrl("http://lf2.net/starsky"));
            }
        }
    }

    /// 0x423b00: the ads from `data\\adN.txt` and `sprite\\sys\\adN.bmp` (loaded at
    /// start-up by `ads_load`, see `ads.rs`): the sliding picture on the right with
    /// its arrows, the strip at the bottom, and the "new version" banner at the top.
    /// `prev_click` holds the previous mouse button state.
    pub(crate) fn draw_main_menu(&mut self, prev_click: u32) {
        if self.gi(AD_SHEET) == 0 {
            return;
        }
        let y0 = self.gi(MENU_Y);
        let (mx, my) = (self.gi(MOUSE_X), self.gi(MOUSE_Y));
        let top = y0 + 199;
        let mut x = 0x24e;
        let url_ok = |w: &World, base: u32, i: u32| w.g.m.u8(base + 100 * i) != b'?';
        if self.gi(AD_INDEX) == -1 {
            let mut n = 0;
            for i in 0..8u32 {
                let end = self.gi(TA_END + 4 * i);
                if n < end && url_ok(self, TA_URL, i) {
                    n = end;
                }
            }
            // timeGetTime() % n in the original (unsigned)
            let v = if n == 0 { 0 } else { (self.time_ms % n as u32) as i32 };
            self.gs(AD_INDEX, v);
            self.gs(AD_SHOWN, 0);
            self.gs(AD_ANIM, 4);
        } else {
            let cur = self.gi(AD_INDEX);
            for i in 0..8u32 {
                if !(self.gi(TA_START + 4 * i) <= cur && cur < self.gi(TA_END + 4 * i) && url_ok(self, TA_URL, i)) {
                    continue;
                }
                let dur = self.gi(TA_DUR + 4 * i);
                let mut anim = self.gi(AD_ANIM);
                if anim < dur + 10 {
                    anim += 1;
                    self.gs(AD_ANIM, anim);
                }
                if anim < 4 {
                    x = anim * -0x3c + 0x314;
                } else if dur - 10 < anim {
                    x = if anim < dur { (anim - dur) * 0x14 + 0x314 } else { 0x348 };
                } else if dur <= anim {
                    x = 0x348;
                }
                self.gs(AD_SHOWN, i as i32);
                self.blit(AD_SHEET, x, top, i as i32 + 2, true);
                if 0x24e < mx && top < my && my < y0 + 0x189 {
                    if dur < anim {
                        self.gs(AD_ANIM, 0);
                    } else if 4 < anim && anim < dur - 10 {
                        self.gs(AD_ANIM, 5);
                    }
                    if x + 0xc2 < 0x31a {
                        self.fill(x, top, 0xc6, 1, 0xffffff);
                        self.fill(x, y0 + 0x188, 0xc6, 1, 0xffffff);
                        self.fill(x, top, 1, 0xc2, 0xffffff);
                        self.fill(x + 0xc5, top, 1, 0xc2, 0xffffff);
                    }
                    if self.gi(MOUSE_CLICK) == 1 && self.gi(prev_click) == 0 {
                        self.gs(MOUSE_CLICK, 0);
                        self.buffer_sounds.push("data\\m_ok.wav");
                        self.requests.push(MenuRequest::OpenAdUrl(TA_URL + 100 * i));
                    }
                }
                break;
            }
        }
        let valid = (0..8u32).filter(|&i| url_ok(self, TA_URL, i)).count();
        if 1 < valid {
            let ax = x + 0xa0;
            // the original compares with 0x453f70 (the first strip box's width) here
            let limit = self.gi(0x453f70);
            let hover = |w: &mut World| {
                let anim = w.gi(AD_ANIM);
                if limit < anim {
                    w.gs(AD_ANIM, 0);
                } else if 4 < anim && anim < limit - 10 {
                    w.gs(AD_ANIM, 5);
                }
            };
            let step = |w: &mut World, d: i32| {
                if w.gi(MOUSE_CLICK) == 1 && w.gi(prev_click) == 0 {
                    w.gs(MOUSE_CLICK, 0);
                    loop {
                        let mut k = w.gi(AD_SHOWN) + d;
                        if k < 0 {
                            k = 7;
                        }
                        if 7 < k {
                            k = 0;
                        }
                        w.gs(AD_SHOWN, k);
                        let v = w.gi(TA_START + 4 * k as u32);
                        w.gs(AD_INDEX, v);
                        if !(v < 0 || !url_ok(w, TA_URL, k as u32)) {
                            break;
                        }
                    }
                }
            };
            if ax < mx && y0 + 0xb5 < my && mx < x + 0xb3 && my <= top {
                hover(self);
                self.blit(MENU_CLIP5, ax, y0 + 0xb9, 10, true);
                step(self, -1);
            } else {
                self.blit(MENU_CLIP5, ax, y0 + 0xb9, 8, true);
            }
            let bx = x + 0xb3;
            if bx < mx && y0 + 0xb5 < my && my <= top {
                hover(self);
                self.blit(MENU_CLIP5, bx, y0 + 0xb9, 0xb, true);
                step(self, 1);
            } else {
                self.blit(MENU_CLIP5, bx, y0 + 0xb9, 9, true);
            }
        }
        // the strip at the bottom with its link boxes
        let (mut min_x, mut min_y) = (999, 999);
        self.blit(AD_SHEET, 0, 0x1a6, 10, true);
        for i in 0..24u32 {
            let bx = self.gi(BA_X + 4 * i);
            if bx == -99 || !url_ok(self, BA_URL, i) {
                continue;
            }
            min_x = min_x.min(bx);
            let by = self.gi(BA_Y + 4 * i);
            min_y = min_y.min(by);
            let (bw, bh) = (self.gi(BA_W + 4 * i), self.gi(BA_H + 4 * i));
            if bx < mx && mx < bw + bx && by < my && (my < bh + by || i / 6 == 3) {
                self.fill(bx, by, bw, 1, 0xffffff);
                self.fill(bx, by - 1 + bh, bw, 1, 0xffffff);
                self.fill(bx, by, 1, bh, 0xffffff);
                self.fill(bw - 1 + bx, by, 1, bh, 0xffffff);
                if self.gi(MOUSE_CLICK) == 1 && self.gi(prev_click) == 0 {
                    self.gs(MOUSE_CLICK, 0);
                    self.buffer_sounds.push("data\\m_ok.wav");
                    self.requests.push(MenuRequest::OpenAdUrl(BA_URL + 100 * i));
                }
            }
        }
        if min_x < 999 && min_y < 999 {
            self.blit(0x451188, min_x, min_y - 10, 0xc, true);
        }
        // a newer version is out: the banner at the top left blinks
        if self.gi(NEW_VERSION) <= self.gi(EXE_VERSION) || self.g.m.u8(NEW_VERSION_URL) == b'?' {
            return;
        }
        let mut t = self.gi(0x4511b4) + 1;
        let mut clip = 1;
        if t < 0x3c {
            if 0x1d < t && (0x18c < mx || 0x21 < my) {
                clip = 0;
            }
        } else {
            t = 0;
        }
        self.gs(0x4511b4, t);
        self.blit(AD_SHEET, 0, 0, clip, true);
        if mx < 0x18d && my < 0x22 {
            self.blit(AD_SHEET, 0, 0, 1, true);
            if self.gi(MOUSE_CLICK) == 1 && self.gi(prev_click) == 0 {
                self.gs(MOUSE_CLICK, 0);
                self.buffer_sounds.push("data\\m_ok.wav");
                self.requests.push(MenuRequest::OpenAdUrl(NEW_VERSION_URL));
            }
        }
    }

    pub(crate) fn draw_cursor(&mut self) {
        let x = self.gi(MOUSE_X).min(0x307);
        let y = (self.gi(MOUSE_Y) + 2).min(0x217);
        self.blit(CURSOR, x, y, -1, true);
    }

    /// 0x431d10.
    pub(crate) fn recording_menus(&mut self) {
        self.gs(0x451b7c, 0);
        self.fill(0, 0, 0x31a, 0x226, 0x122565);
        if self.gi(MENU_BACK) == 0 {
            self.load_menu_background();
        }
        let y0 = self.gi(MENU_Y);
        self.draw_menu_background(y0);
        self.blit(MENU_CLIP, 0x99, y0 + 0x60, 1, true);
        self.draw_credits(PREV_CLICK);
        let net = self.g.m.i8(NETWORK) > 0;
        self.blit(MENU_CLIP3, 0xf4, y0 + 0xce, 1, true);
        self.blit(MENU_CLIP3, 0x101, y0 + 0x16e, if net { 0x32 } else { 0x33 }, true);
        let cur = self.g.game_mode();
        let hl = [(0x121, 0xcc, 2), (0x113, 0xe7, 3), (0x101, 0x102, 4), (0x101, 0x11d, 5), (0x122, 0x137, 0x26),
            (299, 0x153, 6), (0x101, 0x16e, 0x31), (0x138, 0x18a, 7)];
        if (0..8).contains(&cur) {
            let (x, y, c) = hl[cur as usize];
            self.blit(MENU_CLIP3, x, y0 + y, c, false);
        }
        self.menu_keys();
        if self.gi(KEY_UP) != 0 {
            let mut c = self.g.game_mode() - 1;
            self.gs(SHOW_KEYS, 0);
            if c < 0 {
                c = 7;
            }
            if net && c == 6 {
                c = 5;
            }
            self.g.set_game_mode(c);
        }
        if self.gi(KEY_DOWN) != 0 {
            // (n + 1) % 8 as the original computes it for signed n
            let mut c = (self.g.game_mode() + 1) % 8;
            self.gs(SHOW_KEYS, 0);
            if net && c == 6 {
                c = 7;
            }
            self.g.set_game_mode(c);
        }
        if self.gi(KEY_ATTACK) != 0 {
            self.gs(SHOW_KEYS, 0);
            let c = self.g.game_mode();
            if c < 7 {
                self.buffer_sounds.push("data\\m_ok.wav");
            }
            let start = |w: &mut World, phase: i32, bg: i32, d028: i32| {
                w.g.set_match_phase(phase);
                if bg >= 0 {
                    w.g.set_background(bg);
                    w.gs(0x44d028, d028);
                }
                w.gs(SHOW_KEYS, 0);
                w.gs(AD_INDEX, -1);
                w.free_menu_background();
            };
            match c {
                0 | 1 | 4 => {
                    self.gs(0x4512c8, 0);
                    start(self, 3, 100, 1);
                }
                2 => start(self, 0x14, 0, 0),
                3 => start(self, 0x78, 0, 0),
                5 => {
                    self.g.set_match_phase(2);
                    self.g.set_u_450c2c(1);
                    self.gs(SHOW_KEYS, 0);
                    self.gs(AD_INDEX, -1);
                    self.free_menu_background();
                }
                6 => {
                    self.end_match();
                    self.requests.push(MenuRequest::OpenRecording);
                }
                7 => self.requests.push(MenuRequest::Quit),
                _ => {}
            }
        }
        self.draw_main_menu(PREV_CLICK);
        if 0 < self.gi(SHOW_KEYS) {
            self.blit(MENU_CLIP5, 5, 0x27, 7, true);
            for p in 0..4u32 {
                for k in 0..7u32 {
                    let vk = self.gi(0x44fb74 + p * 0x50 + 4 * k);
                    let (name, off) = key_name(vk);
                    self.text(&name, 0x652512, 0xffffff, (p as i32 * 0x6c - off) + 0x82, 0x5d + 0x14 * k as i32);
                }
            }
            self.gs(SHOW_KEYS, self.gi(SHOW_KEYS) - 1);
        }
        self.draw_cursor();
        let (mx, my) = (self.gi(MOUSE_X), self.gi(MOUSE_Y));
        if 0xde < mx && y0 + 0xc2 < my && mx <= 0x23d && my < y0 + 0x1b0 && self.gi(MOUSE_CLICK) == 1 && mx != 0 {
            self.gs(MOUSE_CLICK, 0);
            self.gs(SHOW_KEYS, 0x1c2);
        }
        self.gs(PREV_CLICK, self.gi(MOUSE_CLICK));
    }

    /// 0x429730: the menus inside a match frame.
    pub(crate) fn game_menus(&mut self) {
        let phase = self.g.match_phase();
        if self.gi(0x4512cc) != 10 && phase == 10 {
            self.play_music(b"bgm\\main.wma");
        }
        self.gs(0x4512cc, phase);
        if self.gi(0x44d07c) != 0 {
            // first entry: load the surfaces, no fighter chosen yet
            for s in 0..8u32 {
                self.gs(0x451288 + 4 * s, 0);
                self.gs(0x451268 + 4 * s, 0);
                self.gs(0x451248 + 4 * s, -1);
            }
            self.gs(0x44d07c, 0);
        }
        if self.g.game_mode() == 5 && self.g.u_450c2c() == 0 {
            self.g.set_slow_frames(1);
            self.g.set_match_phase(10);
            self.recording_menus();
            return;
        }
        match phase {
            300 => self.ending_screen(),
            10 => self.recording_menus(),
            0x14..=0x32 => self.championship_1on1(),
            0x78..=0x96 => self.championship_2on2(),
            _ => self.pick_screen(),
        }
    }
}
