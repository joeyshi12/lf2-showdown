//! The network game's title screens. The original's boxes (MENU_CLIP pictures)
//! ask for an IP address and wait for one opponent; this port has its own, drawn
//! with fills and labels in the same colours:
//! - 1: the network menu (host a game, join a game, back);
//! - 2: the host's room: the room code, the four seats, start and cancel;
//! - 3: typing the room code;
//! - 4: a joiner connecting, then waiting in the room for the host to start;
//! - 5: a message when the connection failed, with "try again" and "back".

use crate::menu::main_menu::{MenuRequest, MENU_CLIP, MOUSE_X, MOUSE_Y};
use crate::menu::title::SCREEN;
use crate::net::{set_room_code, LOCAL_IP, SERVER_IP};
use crate::world::{World, KEYS};

const PAGE: u32 = 0x10206c;
const PANEL: u32 = 0x122565;
const LINE: u32 = 0x5a77d8;
const HI: u32 = 0xa9bcff;
const HOVER: u32 = 0x1a3080;
const DIM: u32 = 0x3d55a8;
const OFF_LINE: u32 = 0x2d438f;
const RULE: u32 = 0x23398a;
const WHITE: u32 = 0xffffff;
const ERR: u32 = 0xff9b8a;
const CODE_BG: u32 = 0x0b1a4d;
/// The colour of each seat's number.
const SEAT: [u32; 4] = [0xf2c14e, 0x7ad0f0, 0xef7d7d, 0x8fe08a];
/// Characters typed into the room code so far (0x4511dc, the original's count of
/// typed address characters).
const TYPED: u32 = 0x4511dc;
pub const CODE_LEN: i32 = 4;
/// The panel's left edge and width; it is centred on the screen.
const PX: i32 = 237;
const PW: i32 = 320;
const MID: i32 = PX + PW / 2;

/// Paste `text` into the room code on the code screen: letters as capitals, digits,
/// nothing else, up to the code's four characters. Nothing happens on other screens.
pub fn paste_room_code(w: &mut World, text: &str) {
    if w.g.m.i32(SCREEN) != 3 {
        return;
    }
    for c in text.bytes().filter(|c| c.is_ascii_alphanumeric()) {
        w.type_code_char(c);
    }
}

impl World {
    fn type_code_char(&mut self, c: u8) {
        let n = self.g.m.i32(TYPED);
        if n >= CODE_LEN {
            return;
        }
        self.g.m.set_u8(SERVER_IP + n as u32, c.to_ascii_uppercase());
        self.g.m.set_u8(SERVER_IP + n as u32 + 1, 0);
        self.g.m.set_i32(TYPED, n + 1);
    }

    fn mouse_in(&self, x: i32, y: i32, w: i32, h: i32) -> bool {
        let (mx, my) = (self.g.m.i32(MOUSE_X), self.g.m.i32(MOUSE_Y));
        x <= mx && mx < x + w && y <= my && my < y + h
    }

    /// A box with rounded corners: a 2-pixel `line` around `fill`.
    fn rounded(&mut self, x: i32, y: i32, w: i32, h: i32, line: u32, fill: u32) {
        let shape = |s: &mut World, x: i32, y: i32, w: i32, h: i32, r: i32, c: u32| {
            for i in 0..r.min(h / 2) {
                let d = r as f64 - i as f64 - 0.5;
                let inset = r - ((r * r) as f64 - d * d).max(0.0).sqrt().round() as i32;
                s.fill(x + inset, y + i, w - 2 * inset, 1, c);
                s.fill(x + inset, y + h - 1 - i, w - 2 * inset, 1, c);
            }
            s.fill(x, y + r, w, h - 2 * r, c);
        };
        shape(self, x, y, w, h, 7, line);
        shape(self, x + 2, y + 2, w - 4, h - 4, 5, fill);
    }

    /// The panel with its title on the top line, as the original's "network game".
    fn panel(&mut self, y: i32, h: i32, title: &str) {
        self.rounded(PX, y, PW, h, LINE, PANEL);
        self.draw.push(crate::draw::Draw::Label { text: title.as_bytes().to_vec(), rgb: LINE, x: PX + 24, y: y - 8, align: -1, scale: 1, pad: Some(PANEL) });
    }

    /// A button; true when it was clicked.
    fn button(&mut self, x: i32, y: i32, w: i32, label: &str, enabled: bool) -> bool {
        let hover = enabled && self.mouse_in(x, y, w, 24);
        let (line, fill, text) = match (enabled, hover) {
            (false, _) => (OFF_LINE, PANEL, DIM),
            (true, true) => (LINE, HOVER, HI),
            (true, false) => (LINE, PANEL, LINE),
        };
        self.rounded(x, y, w, 24, line, fill);
        self.ui_text(label, text, x + w / 2, y + 4, 0);
        hover && self.clicked()
    }

    /// A menu item: text that gets a box when the mouse is on it.
    fn item(&mut self, y: i32, label: &str) -> bool {
        let hover = self.mouse_in(PX + 20, y, PW - 40, 26);
        if hover {
            self.rounded(PX + 20, y, PW - 40, 26, LINE, HOVER);
        }
        self.ui_text(label, if hover { HI } else { LINE }, MID, y + 5, 0);
        hover && self.clicked()
    }

    /// The four boxes of the room code; `cursor` blinks in that box.
    fn code_boxes(&mut self, y: i32, code: &[u8], cursor: Option<i32>) {
        let x0 = MID - (4 * 42 + 3 * 8) / 2;
        for i in 0..CODE_LEN {
            let x = x0 + i * 50;
            let here = cursor == Some(i);
            self.rounded(x, y, 42, 48, if here { HI } else { LINE }, CODE_BG);
            if let Some(&c) = code.get(i as usize) {
                let s = (c as char).to_string();
                self.draw.push(crate::draw::Draw::Label { text: s.into_bytes(), rgb: WHITE, x: x + 21, y: y + 8, align: 0, scale: 2, pad: None });
            } else if here && self.time_ms / 500 % 2 == 0 {
                self.fill(x + 20, y + 10, 2, 28, HI);
            }
        }
    }

    /// The seats: number, name, "host" or "you", round-trip time.
    fn seats(&mut self, y: i32) {
        let players = self.net_ui.players.clone();
        self.fill(PX + 18, y, PW - 36, 1, RULE);
        for k in 0..4usize {
            let ry = y + 1 + 30 * k as i32;
            let p = players.get(k);
            self.fill(PX + 22, ry + 6, 18, 18, if p.is_some() { SEAT[k] } else { OFF_LINE });
            self.ui_text(&(k + 1).to_string(), if p.is_some() { CODE_BG } else { LINE }, PX + 31, ry + 7, 0);
            match p {
                Some(p) => {
                    self.ui_text(&p.name, WHITE, PX + 50, ry + 7, -1);
                    let tag = if k == 0 { "host" } else if p.me { "you" } else { "" };
                    self.ui_text(tag, LINE, PX + PW - 82, ry + 7, 1);
                    if let Some(ms) = p.ping {
                        let c = if ms < 80 { 0x7fd18b } else if ms < 160 { 0xe2c35a } else { 0xe0705f };
                        self.ui_text(&format!("{} ms", ms), c, PX + PW - 22, ry + 7, 1);
                    }
                }
                None => {
                    let dots = ".".repeat((self.time_ms / 350 % 4) as usize);
                    self.ui_text(&format!("open{}", dots), DIM, PX + 50, ry + 7, -1);
                }
            }
            self.fill(PX + 18, ry + 29, PW - 36, 1, RULE);
        }
    }

    /// Eight bars that light up one after another.
    fn pulse(&mut self, y: i32) {
        let lit = (self.time_ms / 150 % 10) as i32;
        for i in 0..8 {
            let c = if i == lit || i + 1 == lit { 0x8aa6ff } else { OFF_LINE };
            self.fill(MID - 68 + 18 * i, y, 6, 12, c);
        }
    }

    fn key_pressed(&mut self, vk: u32) -> bool {
        if self.g.m.u8(KEYS + vk) == b'd' {
            self.g.m.set_u8(KEYS + vk, b'u');
            return true;
        }
        false
    }

    fn net_close(&mut self) {
        self.buffer_sounds.push("data\\m_cancel.wav");
        self.g.m.set_i32(SCREEN, 1);
        set_room_code(self, "");
        self.net_ui.players.clear();
        self.requests.push(MenuRequest::NetClose);
    }

    fn net_host(&mut self) {
        self.buffer_sounds.push("data\\m_ok.wav");
        self.g.m.set_i32(SCREEN, 2);
        set_room_code(self, "");
        self.net_ui = Default::default();
        self.net_ui.hosting = true;
        self.requests.push(MenuRequest::NetHost);
    }

    fn net_join(&mut self) {
        self.buffer_sounds.push("data\\m_ok.wav");
        self.g.m.set_i32(SCREEN, 4);
        self.net_ui = Default::default();
        self.requests.push(MenuRequest::NetConnect);
    }

    /// Title screen states 1 to 5.
    pub(crate) fn net_screens(&mut self) {
        if self.g.m.i32(0x4511ac) == 0 {
            self.load_menu_background();
        }
        self.fill(0, 0, 0x31a, 0x226, PAGE);
        self.draw_menu_background(0);
        self.blit(MENU_CLIP, 0x9b, 0x28, 1, true);
        match self.g.m.i32(SCREEN) {
            1 => self.net_menu(),
            2 => self.net_host_room(),
            3 => self.net_code_entry(),
            4 => self.net_joiner(),
            _ => self.net_failed_box(),
        }
    }

    fn net_menu(&mut self) {
        self.panel(210, 140, "network game");
        if self.item(230, "Host a game") {
            self.net_host();
        } else if self.item(264, "Join a game") {
            self.buffer_sounds.push("data\\m_ok.wav");
            self.g.m.set_i32(SCREEN, 3);
            self.g.m.set_i32(TYPED, 0);
            self.g.m.set_u8(SERVER_IP, 0);
        } else if self.item(306, "back") {
            self.buffer_sounds.push("data\\m_cancel.wav");
            self.g.m.set_i32(SCREEN, 0);
            self.g.m.set_i32(0x44d780, -1);
            self.free_menu_background();
            self.requests.push(MenuRequest::NetClose);
        }
    }

    fn net_host_room(&mut self) {
        let y = 150;
        self.panel(y, 316, "Hosting");
        let code: Vec<u8> = (0..8).map(|k| self.g.m.u8(LOCAL_IP + k)).take_while(|&c| c != 0).collect();
        self.ui_text("Room code", LINE, MID, y + 18, 0);
        self.code_boxes(y + 38, &code, None);
        let copied = self.net_ui.copied_at.is_some_and(|t| self.time_ms.wrapping_sub(t) < 2000);
        let hint = if code.is_empty() {
            "Getting a room code"
        } else if copied {
            "Copied"
        } else {
            "Give this code to your friends. Click to copy."
        };
        self.ui_text(hint, if copied { HI } else { DIM }, MID, y + 92, 0);
        if !code.is_empty() && self.mouse_in(MID - 104, y + 38, 208, 48) && self.clicked() {
            self.requests.push(MenuRequest::CopyText(String::from_utf8_lossy(&code).into_owned()));
            self.net_ui.copied_at = Some(self.time_ms);
            self.buffer_sounds.push("data\\m_ok.wav");
        }
        self.seats(y + 116);
        let n = self.net_ui.players.len();
        let ready = n >= 2;
        let start = self.button(PX + 20, y + 254, 134, "start", ready);
        if self.button(PX + PW - 154, y + 254, 134, "cancel", true) {
            self.net_close();
            return;
        }
        let enter = self.key_pressed(0x0d);
        if ready && (start || enter) {
            self.buffer_sounds.push("data\\m_ok.wav");
            self.requests.push(MenuRequest::NetStart);
        }
        let hint = if ready { format!("Press Enter to start with {} players", n) } else { "Waiting for at least one more player".to_string() };
        self.ui_text(&hint, DIM, MID, y + 290, 0);
    }

    fn net_code_entry(&mut self) {
        let y = 200;
        self.panel(y, 170, "Join a game");
        self.ui_text("Type the room code", LINE, MID, y + 18, 0);
        // typing: letters (as capitals) and digits; nothing while Ctrl is held, so
        // Ctrl+V only pastes
        let ctrl = self.g.m.u8(KEYS + 0x11) == b'd';
        for vk in 0..256u32 {
            if self.g.m.u8(KEYS + vk) != b'd' || vk == 0x0d || vk == 0x10 || vk == 0x11 {
                continue;
            }
            let c = match vk {
                0x30..=0x39 | 0x41..=0x5a => Some(vk as u8),
                0x60..=0x69 => Some(vk as u8 - 0x30),
                _ => None,
            };
            self.g.m.set_u8(KEYS + vk, b'u');
            if ctrl {
                continue;
            }
            if let Some(c) = c {
                self.type_code_char(c);
            } else if vk == 8 {
                let n = self.g.m.i32(TYPED);
                if n > 0 {
                    self.g.m.set_u8(SERVER_IP + n as u32 - 1, 0);
                    self.g.m.set_i32(TYPED, n - 1);
                }
            }
        }
        let n = self.g.m.i32(TYPED);
        let code: Vec<u8> = (0..n as u32).map(|k| self.g.m.u8(SERVER_IP + k)).collect();
        self.code_boxes(y + 38, &code, (n < CODE_LEN).then_some(n));
        self.ui_text("Ctrl+V pastes - Backspace deletes", DIM, MID, y + 94, 0);
        let full = n == CODE_LEN;
        let join = self.button(PX + 20, y + 124, 134, "join", full);
        if self.button(PX + PW - 154, y + 124, 134, "cancel", true) {
            self.net_close();
            return;
        }
        if full && (join || self.key_pressed(0x0d)) {
            self.net_join();
        }
    }

    fn net_joiner(&mut self) {
        let code = self.room_code_typed();
        if self.net_ui.players.is_empty() {
            let y = 200;
            self.panel(y, 140, "Join a game");
            self.ui_text(&format!("Connecting to {}", code), LINE, MID, y + 22, 0);
            self.pulse(y + 52);
            self.ui_text("Finding a route to the host's computer", DIM, MID, y + 74, 0);
            if self.button(MID - 67, y + 102, 134, "cancel", true) {
                self.net_close();
            }
            return;
        }
        let y = 160;
        self.panel(y, 236, &format!("Room {}", code));
        self.seats(y + 20);
        let host = self.net_ui.players[0].name.clone();
        self.ui_text(&format!("Waiting for {} to start", host), LINE, MID, y + 150, 0);
        self.pulse(y + 174);
        if self.button(MID - 67, y + 198, 134, "leave", true) {
            self.net_close();
        }
    }

    fn net_failed_box(&mut self) {
        let msg = self.net_ui.error.clone().unwrap_or_default();
        let hosting = self.net_ui.hosting;
        let lines = wrap(&msg, 36);
        let y = 210;
        let h = 112 + 18 * lines.len() as i32;
        self.panel(y, h, if hosting { "Host a game" } else { "Join a game" });
        for (k, l) in lines.iter().enumerate() {
            self.ui_text(l, ERR, MID, y + 22 + 18 * k as i32, 0);
        }
        let ly = y + 30 + 18 * lines.len() as i32;
        let hint = if hosting { "Check your internet connection and try again." } else { "Check the code with the host, or try again." };
        self.ui_text(hint, DIM, MID, ly, 0);
        let again = self.button(PX + 20, ly + 34, 134, "try again", true);
        if self.button(PX + PW - 154, ly + 34, 134, "back", true) {
            self.net_close();
            return;
        }
        if again || self.key_pressed(0x0d) {
            if hosting {
                self.net_host();
            } else {
                self.buffer_sounds.push("data\\m_ok.wav");
                self.g.m.set_i32(SCREEN, 3);
                self.net_ui.error = None;
            }
        }
    }
}

/// `text` in lines of at most about `max` characters, broken at spaces.
fn wrap(text: &str, max: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut cur = String::new();
    for word in text.split_whitespace() {
        if !cur.is_empty() && cur.len() + 1 + word.len() > max {
            lines.push(std::mem::take(&mut cur));
        }
        if !cur.is_empty() {
            cur.push(' ');
        }
        cur.push_str(word);
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines
}
