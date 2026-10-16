//! Original title menu at 0x4246b0 while game state 0x458b00 is 0. SCREEN
//! (`DAT_0044d064`) selects it; network screens 1..5 are in `menu::net_menu`.

use crate::menu::main_menu::{MenuRequest, MENU_CLIP, MENU_CLIP5, MENU_Y, MOUSE_CLICK, MOUSE_X, MOUSE_Y};
use crate::world::{World, KEYS};

pub(crate) const SCREEN: u32 = 0x44d064;
pub(crate) const PREV_CLICK: u32 = 0x44d060;
const ADS: u32 = 0x450be8;
const ADS_ANIM: u32 = 0x4511f4;
const KEY_SLOT: u32 = 0x4511e0;
const NAME_SLOT: u32 = 0x4511c8;
const TEXT_FIELD: u32 = 0x4511c4;
const CONTROLS: u32 = 0x44fb70;
const MENU_CLIP2: u32 = 0x451168;
const MENU_CLIP6: u32 = 0x4511a4;
const MENU_CLIP7: u32 = 0x451188;

/// Set the title screen state (`DAT_0044d064`).
pub fn set_screen(w: &mut World, v: i32) {
    w.tw(SCREEN, v);
}

/// `key_char` (0x422f60): the character a key types (caps lock taken as off).
pub fn key_char(vk: i32, shift: bool) -> u8 {
    let c = vk as u8;
    if vk == 0x20 {
        return b' ';
    }
    let mut ch = c;
    if (0x41..=0x5a).contains(&vk) {
        if !shift {
            ch = c + 0x20;
        }
    } else if (0x60..=0x69).contains(&vk) {
        ch = c - 0x30;
    } else {
        match vk {
            0x6b => return b'+',
            0x6d => return b'-',
            0x6a => return b'*',
            0x6f => return b'/',
            0x6e => return b'.',
            _ => {}
        }
        let shifted: &[(i32, u8)] = &[
            (0xbd, b'_'), (0xbb, b'+'), (0xdb, b'{'), (0xdd, b'}'), (0xba, b':'), (0xde, b'"'), (0xdc, b'|'),
            (0xbc, b'<'), (0xbe, b'>'), (0xbf, b'?'), (0xc0, b'~'), (0x31, b'!'), (0x32, b'@'), (0x33, b'#'),
            (0x34, b'$'), (0x35, b'%'), (0x36, b'^'), (0x37, b'&'), (0x38, b'*'), (0x39, b'('), (0x30, b')'),
        ];
        let plain: &[(i32, u8)] = &[
            (0xbd, b'-'), (0xbb, b'='), (0xdb, b'['), (0xdd, b']'), (0xba, b';'), (0xde, b'\''), (0xdc, b'\\'),
            (0xbc, b','), (0xbe, b'.'), (0xbf, b'/'), (0xc0, b'`'),
        ];
        if shift {
            if let Some(&(_, r)) = shifted.iter().find(|x| x.0 == vk) {
                return r;
            }
            ch = 0;
        } else if !(0x30..=0x39).contains(&vk) {
            if let Some(&(_, r)) = plain.iter().find(|x| x.0 == vk) {
                return r;
            }
            ch = 0;
        }
    }
    match vk {
        0x21 => b'9',
        0x22 => b'3',
        0x23 => b'1',
        0x24 => b'7',
        0x25 => b'4',
        0x26 => b'8',
        0x27 => b'6',
        0x28 => b'2',
        0xc => b'5',
        0x2d => b'0',
        _ => ch,
    }
}

impl World {
    fn t(&self, a: u32) -> i32 {
        self.g.m.i32(a)
    }

    fn tw(&mut self, a: u32, v: i32) {
        self.g.m.set_i32(a, v)
    }

    fn tstr(&self, at: u32, max: u32) -> Vec<u8> {
        (0..max).map(|k| self.g.m.u8(at + k)).take_while(|&c| c != 0).collect()
    }

    fn tput(&mut self, at: u32, s: &[u8]) {
        for (k, &b) in s.iter().chain(std::iter::once(&0)).enumerate() {
            self.g.m.set_u8(at + k as u32, b);
        }
    }

    pub(crate) fn clicked(&self) -> bool {
        self.t(PREV_CLICK) == 0 && self.t(MOUSE_CLICK) == 1
    }

    /// `save_controls` (0x423230): the text of data\control.txt, for the platform to
    /// write. Names get ` for spaces in the file only.
    fn save_controls(&mut self) {
        let mut out = String::new();
        for p in 0..4u32 {
            for k in 0..11u32 {
                out += &format!("{} ", self.t(CONTROLS + 0x50 * p + 4 * k));
            }
            out += "\n";
        }
        let mut names = Vec::new();
        for p in 0..4u32 {
            let mut n = self.tstr(0x44fcc0 + 0xb * p, 11);
            for c in n.iter_mut() {
                if *c == b'`' {
                    *c = b'\'';
                } else if *c == b' ' {
                    *c = b'`';
                }
            }
            if n.is_empty() {
                n = vec![b'1' + p as u8];
            }
            self.tput(0x44fcc0 + 0xb * p, &n.iter().map(|&c| if c == b'`' { b' ' } else { c }).collect::<Vec<u8>>());
            names.push(String::from_utf8_lossy(&n).into_owned());
        }
        out += &format!("{}\n", names.join(" "));
        for (at, def) in [(0x44fd18u32, &b"<No name>"[..]), (0x44f900, b"<No info>"), (0x44f890, b"<No email>")] {
            if self.g.m.u8(at) == 0 {
                self.tput(at, def);
            }
        }
        out += &format!("{}\n{}\n", self.t(ADS), self.t(0x450be4));
        out += &format!("{}\n", String::from_utf8_lossy(&self.tstr(0x44fd18, 100)));
        out += &format!("{}\n", String::from_utf8_lossy(&self.tstr(0x44f890, 100)));
        out += &String::from_utf8_lossy(&self.tstr(0x44f900, 0x400));
        let out = out.into_bytes();
        self.control_txt = Some(out.clone());
        self.requests.push(MenuRequest::SaveControls(out));
    }

    /// `load_controls` (0x423480) again (Cancel). A missing file changes nothing.
    fn reload_controls(&mut self) {
        if let Some(t) = self.control_txt.clone() {
            self.apply_controls(&lf2_data::controls::parse(&t));
        }
    }

    /// Apply data\control.txt (what `load_controls` reads) to the globals.
    pub fn apply_controls(&mut self, c: &lf2_data::controls::Controls) {
        for (p, v) in c.players.iter().enumerate() {
            for (k, &x) in v.iter().enumerate() {
                self.tw(CONTROLS + 0x50 * p as u32 + 4 * k as u32, x);
            }
        }
        for (p, n) in c.names.iter().enumerate() {
            let n = n[..n.len().min(10)].to_vec();
            self.tput(0x44fcc0 + 0xb * p as u32, &n);
        }
        self.tw(ADS, c.ads);
        self.tw(0x450be4, c.record);
        // fgets keeps the line end, then trailing ' ', '\r', '\n' become 0
        for (at, v, end) in [(0x44fd18u32, &c.name, c.name_end), (0x44f890, &c.email, c.email_end)] {
            let n = v.len().min(99);
            self.tput(at, &v[..n]);
            for k in 0..end.min(99 - n) as u32 {
                self.g.m.set_u8(at + n as u32 + 1 + k, 0);
            }
        }
        self.tput(0x44f900, &c.info[..c.info.len().min(0x3ff)]);
    }

    /// 0x4246b0 while the game state is 0.
    pub(crate) fn title_menus(&mut self) {
        self.tw(0x4511f8, 1 - self.t(0x4511f8));
        if self.t(SCREEN) == -2 {
            if self.t(ADS) == -1 && !self.ads_removed {
                self.tw(SCREEN, -3);
            } else {
                self.tw(SCREEN, 0);
            }
        }
        self.fill(0, 0, 0x31a, 0x226, 0x10206c);
        if self.t(0x4511ac) == 0 {
            self.load_menu_background();
        }
        let y0 = if self.t(SCREEN) != 0 { 0 } else { self.t(MENU_Y) };
        self.draw_menu_background(y0);
        if self.t(SCREEN) == 0 {
            self.draw_credits(PREV_CLICK);
            if self.ads_removed {
            } else if self.t(ADS) == 0 {
                self.blit(MENU_CLIP7, 0x2d5, 5, 6, true);
                if 0x2d4 < self.t(MOUSE_X) && self.t(MOUSE_Y) < 0x12 {
                    self.blit(MENU_CLIP7, 0x2d5, 5, 7, true);
                    if self.clicked() {
                        self.tw(MOUSE_CLICK, 0);
                        self.tw(SCREEN, -3);
                        self.buffer_sounds.push("data\\m_ok.wav");
                    }
                }
            } else {
                self.blit(MENU_CLIP7, 0x2d5, 5, 8, true);
                if 0x2d4 < self.t(MOUSE_X) && self.t(MOUSE_Y) < 0x12 {
                    self.blit(MENU_CLIP7, 0x2d5, 5, 9, true);
                    if self.clicked() {
                        self.tw(MOUSE_CLICK, 0);
                        self.tw(SCREEN, -3);
                        self.buffer_sounds.push("data\\m_ok.wav");
                    }
                }
            }
            self.blit(MENU_CLIP, 0x9b, self.t(MENU_Y) + 0x60, 1, true);
        }
        match self.t(SCREEN) {
            -3 => self.title_ads_question(),
            0 => self.title_main(),
            6 => self.title_controls(),
            7 => self.title_recording(),
            8 => self.title_challenge(),
            1..=5 => self.net_screens(),
            _ => {}
        }
        let x = self.t(MOUSE_X).min(0x307);
        let y = (self.t(MOUSE_Y) + 2).min(0x217);
        self.blit(crate::menu::main_menu::CURSOR, x, y, -1, true);
        self.recording_messages();
        if self.t(MOUSE_X) < 0x3f && 0x201 < self.t(MOUSE_Y) && self.clicked() {
            self.requests.push(MenuRequest::Quit);
        }
        self.tw(PREV_CLICK, self.t(MOUSE_CLICK));
    }

    fn title_ads_question(&mut self) {
        let a = self.t(ADS_ANIM);
        let y = if a == 0 { 0x60 } else { a + (a + 0x5a) / -6 };
        if a != 0 {
            self.tw(ADS_ANIM, y);
        }
        self.blit(MENU_CLIP, 0x9b, y, 1, true);
        self.blit(MENU_CLIP7, 0x4b, y + 100, 0, true);
        let (mx, my) = (self.t(MOUSE_X), self.t(MOUSE_Y));
        if ((mx - 0xd1) as u32) < 0xea && y + 0xae < my && my < y + 0xcb {
            self.blit(MENU_CLIP7, 0xd1, y + 0xae, 3, true);
            if self.clicked() {
                self.tw(MOUSE_CLICK, 0);
                self.tw(ADS, 1);
                self.save_controls();
                self.tw(SCREEN, 0);
                self.tw(ADS_ANIM, 0);
                self.buffer_sounds.push("data\\m_ok.wav");
            }
        }
        if ((mx - 0x1d1) as u32) < 0x58 && y + 0xae < my && my < y + 0xcb {
            self.blit(MENU_CLIP7, 0x1d1, y + 0xae, 4, true);
            if self.clicked() {
                self.tw(MOUSE_CLICK, 0);
                self.tw(ADS, 0);
                self.save_controls();
                self.tw(SCREEN, 0);
                self.tw(ADS_ANIM, 0);
                self.buffer_sounds.push("data\\m_cancel.wav");
            }
        }
        if self.t(ADS_ANIM) == 0 {
            if ((mx - 0xcb) as u32) < 0x167 && y + 0xf0 < my && my < y + 0x106 {
                self.blit(MENU_CLIP7, 0xca, y + 0xf0, 5, true);
                if self.clicked() {
                    self.tw(MOUSE_CLICK, 0);
                    self.tw(ADS_ANIM, -0x11);
                    self.buffer_sounds.push("data\\m_cancel.wav");
                }
                if self.t(ADS_ANIM) != 0 {
                    self.blit(MENU_CLIP7, 0x4b, y + 0x106, 2, true);
                    self.blit(MENU_CLIP7, 0x4b, y + 0x21e, 1, true);
                    return;
                }
            }
            self.blit(MENU_CLIP7, 0x4b, y + 0x105, 1, true);
        } else {
            self.blit(MENU_CLIP7, 0x4b, y + 0x106, 2, true);
            self.blit(MENU_CLIP7, 0x4b, y + 0x21e, 1, true);
        }
    }

    fn title_main(&mut self) {
        let y0 = self.t(MENU_Y);
        self.blit(MENU_CLIP5, 0x107, y0 + 0xca, 0, true);
        self.tw(KEY_SLOT, 0);
        let (mx, my) = (self.t(MOUSE_X), self.t(MOUSE_Y));
        if !(0x113 < mx && mx < 0x209) {
            self.draw_main_menu(PREV_CLICK);
            return;
        }
        let mut item = 0;
        if y0 + 0xd9 <= my && my <= y0 + 0xf1 {
            item = 1;
        }
        if y0 + 0xf7 <= my && my <= y0 + 0x110 {
            item = 2;
        }
        if y0 + 0x117 <= my && my <= y0 + 0x130 {
            item = 3;
        }
        if y0 + 0x135 <= my && my <= y0 + 0x14e {
            item = 4;
        }
        let link = !(my < y0 + 0x153 || y0 + 0x16c < my);
        let click = self.t(MOUSE_CLICK) == 1 && self.t(PREV_CLICK) == 0;
        if link {
            self.blit(MENU_CLIP5, 0x117, y0 + 0x155, 2, true);
            if click {
                self.buffer_sounds.push("data\\m_ok.wav");
                self.requests.push(MenuRequest::OpenUrl("http://littlefighter.com"));
            }
        } else {
            match item {
                1 => {
                    self.blit(MENU_CLIP, 0x115, y0 + 0xd7, 7, true);
                    if click {
                        // Game start
                        self.tw(MOUSE_CLICK, 0);
                        self.buffer_sounds.push("data\\m_ok.wav");
                        self.game_start();
                    }
                }
                2 => {
                    self.fill_random_table();
                    self.blit(MENU_CLIP, 0x115, y0 + 0xf7, 8, true);
                    if click {
                        // the original calls net_init (0x402b60) and listens on port 12345
                        self.tw(MOUSE_CLICK, 0);
                        self.buffer_sounds.push("data\\m_ok.wav");
                        self.tw(SCREEN, 1);
                        crate::net::set_room_code(self, "");
                    }
                }
                3 => {
                    self.blit(MENU_CLIP, 0x115, y0 + 0x116, 9, true);
                    if click {
                        self.buffer_sounds.push("data\\m_ok.wav");
                        self.tw(SCREEN, 6);
                    }
                }
                4 => {
                    self.blit(MENU_CLIP5, 0x117, y0 + 0x135, 1, true);
                    if click {
                        self.buffer_sounds.push("data\\m_ok.wav");
                        self.tw(SCREEN, 7);
                    }
                }
                _ => {}
            }
        }
        self.draw_main_menu(PREV_CLICK);
    }

    /// "Game start" on the title screen.
    pub(crate) fn game_start(&mut self) {
        self.g.m.set_u8(KEYS + 0x47, b'u');
        self.tw(SCREEN, 0);
        self.tw(crate::world::GAME, 1);
        self.fill_random_table();
        for s in 0..4u32 {
            self.tw(crate::world::PLAYER_OF_SLOT + 4 * s, s as i32 + 1);
        }
    }

    /// `fill_random_table` (0x422ac0): 3000 bytes of rand() % 255 + 1 (msvcr80 rand).
    fn fill_random_table(&mut self) {
        let mut r = crate::setup::CrtRand(self.g.crt_seed() as u32);
        for i in 0..3000u32 {
            let v = r.next();
            self.g.m.set_u8(crate::world::RNG_TABLE + i, ((v % 0xff) + 1) as u8);
        }
        self.g.set_crt_seed(r.0 as i32);
        self.tw(0x450b48, 0);
    }

    fn title_controls(&mut self) {
        self.blit(MENU_CLIP, 0x9b, 0x23, 1, true);
        self.blit(MENU_CLIP2, 0x2e, 0x79, 0, true);
        let (mx, my) = (self.t(MOUSE_X), self.t(MOUSE_Y));
        if mx < 0x2e || 0x21c < mx || my < 0x1db || 0x1f2 < my {
            self.blit(MENU_CLIP2, 0x2e, 0x1db, 1, true);
        } else {
            self.blit(MENU_CLIP2, 0x2e, 0x1db, 2, true);
            if self.clicked() {
                self.buffer_sounds.push("data\\m_ok.wav");
                self.requests.push(MenuRequest::OpenUrl("http://www.littlefighter.com/control_index.html"));
            }
        }
        // a key slot is waiting for a key (or a joystick button)
        let slot = self.t(KEY_SLOT);
        if 0 < slot {
            let kind = self.t(CONTROLS + 4 * ((slot / 0x14) * 0x14) as u32);
            if kind != 0 {
                let base = 0x453fc4 + kind as u32 * 0x30;
                if let Some(b) = (0..4).find(|&k| self.g.m.u8(base + k) == 1) {
                    self.tw(0x44fb7c + 4 * slot as u32, b as i32);
                    self.tw(KEY_SLOT, 0);
                }
            } else if let Some(vk) = (0..0xfa).find(|&k| self.g.m.u8(KEYS + k) == b'd') {
                self.tw(CONTROLS + 4 * slot as u32, vk as i32);
                self.tw(KEY_SLOT, 0);
            }
        }
        // a player name is being typed
        let ns = self.t(NAME_SLOT);
        if 0 < ns {
            let shift = self.g.m.u8(KEYS + 0x10) == b'd';
            for vk in 0..300u32 {
                if self.g.m.u8(KEYS + vk) != b'd' {
                    continue;
                }
                let at = 0x44fcb5 + 0xb * ns as u32;
                let mut name = self.tstr(at, 11);
                let c = key_char(vk as i32, shift);
                if c == 0 || vk == 8 {
                    if vk == 8 && !name.is_empty() {
                        name.pop();
                        self.tput(at, &name);
                        self.g.m.set_u8(KEYS + vk, b'u');
                    }
                    continue;
                }
                if 9 < name.len() || vk == 0xd {
                    continue;
                }
                name.push(c);
                self.tput(at, &name);
                self.g.m.set_u8(KEYS + vk, b'u');
            }
        }
        let click = self.clicked();
        for p in 0..4i32 {
            let x = 0xc2 + 0x8b * p;
            let kind_at = CONTROLS + 0x50 * p as u32;
            if x <= mx && mx <= x + 0x69 && click && ((my - 0xb6) as u32) < 0x5b {
                self.tw(KEY_SLOT, 0);
                self.tw(kind_at, (self.t(kind_at) + 1) % 3);
            }
            let name_at = 0x44fcc0 + 0xb * p as u32;
            let mut name = self.tstr(name_at, 11);
            let color = if self.t(NAME_SLOT) - 1 == p {
                if name.len() < 10 {
                    name.push(b'_');
                }
                0xff4619
            } else {
                0xffffff
            };
            self.text(&name, 0x2c0900, color, x + 0x15, 0x9f);
            if x <= mx && mx <= x + 0x69 && ((my - 0x9c) as u32) < 0x12 && click {
                self.tw(NAME_SLOT, p + 1);
                self.tw(KEY_SLOT, 0);
            }
            for k in 0..7 {
                let y = 0x11b + 0x16 * k;
                let slot = 0x14 * p + 1 + k;
                let kind = self.t(kind_at);
                if x <= mx && mx <= x + 0x69 && click && y - 3 <= my && my <= y + 0xf {
                    if kind == 0 || 3 < k {
                        self.tw(KEY_SLOT, slot);
                    }
                    self.tw(NAME_SLOT, 0);
                }
                let sel = self.t(KEY_SLOT) == slot;
                let color = if sel { 0xff4619 } else { 0xffffff };
                if kind == 0 {
                    let (n, off) = crate::menu::main_menu::key_name(self.t(CONTROLS + 4 * slot as u32));
                    self.text(&n, 0x2c0900, color, x + (0x29 - off), y);
                } else if k < 4 {
                    self.text(b"-", 0x2c0900, 0xffffff, x + 0x30, y);
                } else {
                    // a joystick's attack, jump and defend buttons are the three numbers
                    // after the keys (+0x20, where the slot waiting above writes them)
                    let s = format!("Button: {}", self.t(0x44fb7c + 4 * slot as u32));
                    self.text(s.as_bytes(), 0x2c0900, color, x + 0x22, y);
                }
            }
            let pic = [0x451180u32, 0x451174, 0x451164, 0x451184, 0x451194];
            let kind = self.t(kind_at);
            if (0..5).contains(&kind) {
                self.blit(pic[kind as usize], x, 0xb6, -1, false);
            }
            if 0x1b8 < my {
                if my < 0x1d2 && ((mx - 0x246) as u32) < 0x9c {
                    self.blit(MENU_CLIP, 0x244, 0x1b9, 0xc, true);
                    if click {
                        self.buffer_sounds.push("data\\m_cancel.wav");
                        self.reload_controls();
                        self.tw(SCREEN, 0);
                        self.tw(0x44d780, -1);
                        self.free_menu_background();
                    }
                }
                if ((my - 0x1b9) as u32) < 0x19 && ((mx - 0x195) as u32) < 0x9c {
                    self.blit(MENU_CLIP, 0x198, 0x1b9, 0xd, true);
                    if click {
                        self.buffer_sounds.push("data\\m_ok.wav");
                        self.save_controls();
                        self.tw(SCREEN, 0);
                        self.tw(0x44d780, -1);
                        self.free_menu_background();
                    }
                }
            }
        }
    }

    fn title_recording(&mut self) {
        self.blit(MENU_CLIP, 0x9b, 0x37, 1, true);
        self.blit(MENU_CLIP6, 0x2c, 0x93, 0, true);
        let (mx, my) = (self.t(MOUSE_X), self.t(MOUSE_Y));
        let fields = [(0x44fd18u32, 0xcf, 0xe1), (0x44f900, 0xe8, 0x12e), (0x44f890, 0x135, 0x147)];
        if self.clicked() {
            let f = if (0xd2..=0x2da).contains(&mx) {
                fields.iter().find(|f| f.1 <= my && my <= f.2).map_or(0, |f| f.0)
            } else {
                0
            };
            self.tw(TEXT_FIELD, f as i32);
        }
        let field = self.t(TEXT_FIELD) as u32;
        if field != 0 {
            let shift = self.g.m.u8(KEYS + 0x10) == b'd';
            for vk in 0..0xfau32 {
                if self.g.m.u8(KEYS + vk) != b'd' {
                    continue;
                }
                let mut s = self.tstr(field, 0x400);
                let mut c = key_char(vk as i32, shift);
                if vk == 0xd {
                    c = b'\n';
                }
                if vk == 8 {
                    if !s.is_empty() {
                        s.pop();
                        self.tput(field, &s);
                        self.g.m.set_u8(KEYS + vk, b'u');
                    }
                    continue;
                }
                if c == 0 {
                    continue;
                }
                if !s.is_empty() || c != b'\n' {
                    s.push(c);
                    self.tput(field, &s);
                    self.g.m.set_u8(KEYS + vk, b'u');
                }
            }
        }
        let f = self.t(TEXT_FIELD) as u32;
        self.draw_number_shadow(0x44fd18, 0xd5, 0xd1, 0x40, 1, (f == 0x44fd18) as i32, f == 0x44fd18);
        self.draw_number_shadow(0x44f900, 0xd5, 0xea, 0x40, 4, (f == 0x44f900) as i32, f == 0x44f900);
        self.draw_number_shadow(0x44f890, 0xd5, 0x137, 0x40, 1, (f == 0x44f890) as i32, f == 0x44f890);
        if self.t(0x450be4) != 0 {
            self.blit(MENU_CLIP6, 0x11f, 0x185, 6, true);
        }
        let click = self.clicked();
        if 0x11e < mx && 0x185 < my && mx < 0x133 && my < 0x19a {
            self.blit(MENU_CLIP6, 0x11f, 0x185, 5, true);
            if click {
                self.tw(0x450be4, 1 - self.t(0x450be4));
            }
        }
        if 0x184 < mx && 0x183 < my && mx < 0x2dd && my < 0x19b {
            self.blit(MENU_CLIP6, 0x185, 0x184, 7, true);
            if click {
                self.buffer_sounds.push("data\\m_ok.wav");
                self.requests.push(MenuRequest::OpenUrl("recording"));
            }
        }
        if mx < 0x2c || my < 0x1cd || 0x1e3 < mx || 0x1e4 < my {
            self.blit(MENU_CLIP6, 0x2c, 0x1cd, 1, true);
        } else {
            self.blit(MENU_CLIP6, 0x2c, 0x1cd, 2, true);
            if click {
                self.buffer_sounds.push("data\\m_ok.wav");
                self.requests.push(MenuRequest::OpenUrl("http://www.littlefighter.com/record"));
            }
        }
        if 0x19f < my && my < 0x1b9 {
            if 0x192 < mx && mx < 0x22f {
                self.blit(MENU_CLIP, 0x193, 0x1a0, 0xc, true);
                if click {
                    self.reload_controls();
                    self.buffer_sounds.push("data\\m_cancel.wav");
                    self.tw(SCREEN, 0);
                    self.tw(0x44d780, -1);
                    self.free_menu_background();
                }
            }
            if 0xe6 < mx && mx < 0x183 {
                self.blit(MENU_CLIP, 0xe7, 0x1a0, 0xd, true);
                if click {
                    self.save_controls();
                    self.buffer_sounds.push("data\\m_ok.wav");
                    self.tw(SCREEN, 8);
                }
            }
        }
    }

    fn title_challenge(&mut self) {
        self.blit(MENU_CLIP, 0x9b, 0x4b, 1, true);
        self.blit(MENU_CLIP6, 0x2c, 0xa7, 3, true);
        let (mx, my) = (self.t(MOUSE_X), self.t(MOUSE_Y));
        let click = self.clicked();
        if 0x5f < mx && 0x15b < my && mx < 0x280 && my < 0x175 {
            self.blit(MENU_CLIP6, 0x60, 0x15c, 4, true);
            if click {
                self.buffer_sounds.push("data\\m_ok.wav");
                self.requests.push(MenuRequest::OpenUrl("http://www.littlefighter.com/challenge"));
            }
        }
        if 0x17d < my && my < 0x197 && 0x13d < mx && mx < 0x1d9 {
            self.blit(MENU_CLIP, 0x13e, 0x17e, 0xd, true);
            if click {
                self.buffer_sounds.push("data\\m_ok.wav");
                self.tw(SCREEN, 0);
                self.tw(0x44d780, -1);
                self.free_menu_background();
            }
        }
    }

}
