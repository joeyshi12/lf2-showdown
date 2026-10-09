//! The 1 on 1 championship screens: `championship_1on1_menus` (0x432ab0), match_phase
//! 0x14..0x1d.
//!
//! 0x15 choose the eight contestants (fighter, human or computer), 0x16 confirm,
//! 0x17 shuffle the bracket, 0x18/0x19 the menu, 0x1a pick the next pair from the
//! bracket, 0x1b the humans of the pair choose who plays, 0x1c after a match, 0x1d the
//! champion.

use crate::menu::main_menu::{KEY_ATTACK, KEY_DOWN, KEY_JUMP, KEY_LEFT, KEY_RIGHT, KEY_UP, MOUSE_CLICK};
use crate::world::{World, EXISTS, MAX_OBJECTS};

const RANDOM: u32 = 0x451344;
const PICKS: u32 = 0x44d0c0;
const HUMAN: u32 = 0x451364;
const ORDER: u32 = 0x44d0e0;
const HUMANS: u32 = 0x451340;
const HP: u32 = 0x44d080;
const LAST_CAUGHT: u32 = 0x44d0a0;
const PROGRESS: u32 = 0x451384;
const TEAM: u32 = 0x44d100;
const ROUND: u32 = 0x44d318;
const ASSIGN: u32 = 0x4512d0;
const BOX: u32 = 0x44d158;
const BOX_SIZE: u32 = 0x44d120;
pub const CLIP4: u32 = 0x45116c;

/// The globals that differ between the 1 on 1 (0x432ab0) and 2 on 2 (0x434ab0)
/// screens.
#[derive(Clone, Copy)]
pub(crate) struct Ch {
    pub cur: u32,
    pub sub: u32,
    pub blink: u32,
    pub menu: u32,
    pub timer: u32,
    pub winner: u32,
    pub pair: u32,
    pub pair_count: u32,
    pub label: u32,
}

const C1: Ch = Ch {
    cur: 0x4513e8,
    sub: 0x4513e4,
    blink: 0x4513e0,
    menu: 0x4513dc,
    timer: 0x4513d8,
    winner: 0x4513d4,
    pair: 0x4513c8,
    pair_count: 0x4513d0,
    label: 0x44d31c,
};

const C2: Ch = Ch {
    cur: 0x451414,
    sub: 0x451410,
    blink: 0x45140c,
    menu: 0x451408,
    timer: 0x451404,
    winner: 0x451400,
    pair: 0x4513ec,
    pair_count: 0x4513fc,
    label: 0x44d320,
};

impl World {
    /// A 1 on 1 phase number in the current championship (2 on 2 adds 0x64).
    fn ph(&self, p: i32) -> i32 {
        p + if self.g.game_mode() == 3 { 0x64 } else { 0 }
    }

    pub(crate) fn ch(&self) -> Ch {
        if self.g.game_mode() == 3 { C2 } else { C1 }
    }

    fn r(&self, a: u32) -> i32 {
        self.g.m.i32(a)
    }

    fn w(&mut self, a: u32, v: i32) {
        self.g.m.set_i32(a, v)
    }

    fn ri(&self, a: u32, i: i32) -> i32 {
        self.g.m.i32(a.wrapping_add((4 * i) as u32))
    }

    fn wi(&mut self, a: u32, i: i32, v: i32) {
        self.g.m.set_i32(a.wrapping_add((4 * i) as u32), v)
    }

    fn champ_pickable(&self, k: i32) -> bool {
        let d = &self.gd.objects[k as usize];
        d.kind == 0 && ((d.id / 10 != 5 && d.id / 10 != 3) || self.g.u_458428() == 1)
    }

    /// Fill the bracket boxes of contestant `p` up to `n` in `rgb`.
    pub(crate) fn bracket_boxes(&mut self, p: i32, n: i32, rgb: u32) {
        self.bracket_boxes_from(p, 0, n, rgb);
    }

    /// Boxes `from..n` of contestant `p`.
    pub(crate) fn bracket_boxes_from(&mut self, p: i32, from: i32, n: i32, rgb: u32) {
        for k in from..n {
            let (x, y) = (self.ri(BOX, p * 0xe + 2 * k), self.ri(BOX, p * 0xe + 2 * k + 1));
            let (bw, bh) = (self.ri(BOX_SIZE, 2 * k), self.ri(BOX_SIZE, 2 * k + 1));
            self.fill(x - 0x33, y + 0x7e, bw, bh, rgb);
        }
    }

    fn label(&mut self, c: u8, bk: u32, color: u32, x: i32, y: i32) {
        self.g.m.set_u8(self.ch().label, c);
        let t: Vec<u8> = (0..8).map(|k| self.g.m.u8(self.ch().label + k)).take_while(|&b| b != 0).collect();
        self.text(&t, bk, color, x, y);
    }

    fn reset_assignments(&mut self) {
        for i in 0..MAX_OBJECTS as u32 {
            self.g.m.set_u8(EXISTS + i, 0);
        }
        for i in 0..20 {
            self.wi(ASSIGN, i, -1);
        }
    }

    fn champ_blink(&self, on: bool) -> u32 {
        if !on {
            return 0xffffff;
        }
        let v = (self.r(self.ch().blink) % 6) * 0x1e;
        let (r, g) = ((v + 0x19) as u32 & 0xff, (v + 0x46) as u32 & 0xff);
        0xff0000 | (g << 8) | r
    }

    fn face_of(&mut self, pick: i32, x: i32, y: i32) {
        if pick < 0 {
            self.blit(crate::menu::pick::RFACE, x, y, -1, false);
        } else {
            self.blit_sheet(crate::draw::Sheet::Face(pick as usize), x, y, -1, false);
        }
    }

    /// 0x432ab0.
    pub(crate) fn championship_1on1(&mut self) {
        self.menu_keys();
        let mode = self.g.game_mode();
        if self.g.match_phase() == 0x14 {
            self.g.set_match_phase(0x15);
            self.w(self.ch().cur, 0);
            self.w(self.ch().sub, 0);
            for i in 0..8 {
                self.wi(RANDOM, i, 1);
                self.wi(PICKS, i, -1);
                self.wi(HUMAN, i, 0);
                self.wi(ORDER, i, i);
            }
        }
        if 0x14 < self.g.match_phase() {
            self.w(self.ch().blink, (self.r(self.ch().blink) + 1) % 0x1e);
            let (ox, oy) = if self.g.match_phase() < 0x18 {
                self.blit(CLIP4, 0x1b, 0x6a, 0, false);
                self.fill(0x2f, 0x180, 0x83, 0x29, 0x324d9a);
                (0x1b, 0x6a)
            } else {
                self.blit(CLIP4, 0x77, 0x7e, 9, false);
                (-0x33, 0x7e)
            };
            if self.r(self.ch().cur) < 8 {
                self.choose_contestant(ox, oy);
            }
            let cur = self.r(self.ch().cur);
            let mut n = 0;
            for i in 0..cur.max(0) {
                if 0 < self.ri(HUMAN, i) {
                    n += 1;
                    self.wi(HUMAN, i, n);
                }
            }
            self.w(HUMANS, n);
            for i in 0..cur.max(0) {
                let idx = self.ri(ORDER, i);
                let x = ox + 0xd8 + i * 0x3c;
                self.small_face(self.ri(PICKS, idx), x, oy + 0x104);
                let h = self.ri(HUMAN, idx);
                if h == 0 {
                    self.label(b'C', 0x8f472f, 0x9679c8, x + 0xd, oy + 0x133);
                } else {
                    self.label(h as u8 + b'0', 0x8f472f, 0xe6794d, x + 0xd, oy + 0x133);
                }
            }
            if cur < 8 {
                let idx = self.ri(ORDER, cur);
                self.small_face(self.ri(PICKS, cur), ox + 0xd8 + cur * 0x3c, oy + 0x104);
                if 0 < self.r(self.ch().sub) {
                    let x = ox + cur * 0x3c + 0xe5;
                    if self.ri(HUMAN, idx) == 0 {
                        self.label(b'C', 0x8f472f, 0x9679c8, x, oy + 0x133);
                    } else {
                        let c = self.r(HUMANS) as u8 + b'1';
                        self.label(c, 0x8f472f, 0xe6794d, x, oy + 0x133);
                    }
                }
            }
        }
        let phase = self.g.match_phase();
        if phase == 0x16 {
            if self.confirm_box(3) {
                return;
            }
        } else if phase == 0x17 {
            if self.shuffle_bracket() {
                return;
            }
        } else if phase == 0x18 {
            self.resolve_contestants();
            if self.champ_menu(mode) {
                return;
            }
        } else if phase == 0x19 && self.champ_menu(mode) {
            return;
        }
        if 0x19 < self.g.match_phase() {
            for p in 0..8 {
                let mut prog = self.ri(PROGRESS, p);
                if 8 <= prog {
                    prog = 7;
                }
                if prog == 7 && self.g.match_phase() < 0x1d {
                    self.g.set_match_phase(0x1d);
                    self.w(self.ch().winner, p);
                    self.w(self.ch().timer, 0);
                }
                let color = if prog < 7 && prog % 2 == 1 { 0x102860 } else { 0xffffff };
                self.bracket_boxes(p, prog, color);
            }
        }
        if self.g.match_phase() == 0x1a {
            self.next_pair();
        }
        if self.g.match_phase() == 0x1b {
            self.champ_assign();
        }
        if self.g.match_phase() == 0x1c {
            self.after_champ_match();
        }
        if self.g.match_phase() == 0x1d {
            self.champion_screen();
        }
    }

    fn small_face(&mut self, pick: i32, x: i32, y: i32) {
        if pick < 0 {
            self.blit(CLIP4, x, y, 1, false);
        } else {
            self.blit_sheet(crate::draw::Sheet::Small(pick as usize), x, y, -1, false);
        }
    }

    /// Phase 0x15: the fighter and human/computer of contestant self.ch().cur.
    fn choose_contestant(&mut self, ox: i32, oy: i32) {
        let cur = self.r(self.ch().cur);
        let n = self.gd.objects.len() as i32;
        if self.r(self.ch().sub) == 0 {
            if self.r(KEY_UP) != 0 {
                self.wi(PICKS, cur, -1);
                self.wi(RANDOM, cur, 1);
            }
            if self.r(KEY_RIGHT) != 0 {
                let mut p = self.ri(PICKS, cur) + 1;
                self.wi(PICKS, cur, p);
                self.wi(RANDOM, cur, 0);
                let mut found = false;
                while p < n {
                    if self.champ_pickable(p) {
                        found = true;
                        break;
                    }
                    p += 1;
                    self.wi(PICKS, cur, p);
                }
                if !found {
                    self.wi(PICKS, cur, -1);
                    self.wi(RANDOM, cur, 1);
                }
            }
            if self.r(KEY_LEFT) != 0 {
                let mut p = self.ri(PICKS, cur) - 1;
                self.wi(PICKS, cur, p);
                self.wi(RANDOM, cur, 0);
                let mut found = false;
                if p != -1 {
                    loop {
                        if p < -1 {
                            p = n - 1;
                            self.wi(PICKS, cur, p);
                        }
                        let q = p;
                        if self.champ_pickable(q) {
                            found = true;
                            break;
                        }
                        p = q - 1;
                        self.wi(PICKS, cur, p);
                        if q == 0 {
                            break;
                        }
                    }
                }
                if !found {
                    self.wi(RANDOM, cur, 1);
                }
            }
            if self.r(KEY_ATTACK) != 0 {
                self.w(KEY_ATTACK, 0);
                self.buffer_sounds.push("data\\m_join.wav");
                self.w(self.ch().sub, 1);
            } else if self.r(KEY_JUMP) != 0 {
                self.w(KEY_JUMP, 0);
                self.buffer_sounds.push("data\\m_cancel.wav");
                if cur < 1 {
                    self.g.set_match_phase(10);
                    self.end_match();
                    self.w(MOUSE_CLICK, 0);
                } else {
                    self.w(self.ch().cur, cur - 1);
                    self.w(self.ch().sub, 1);
                }
            }
        }
        let cur = self.r(self.ch().cur);
        let color = self.champ_blink(self.r(self.ch().sub) == 0);
        let p = self.ri(PICKS, cur);
        self.face_of(p, ox + 0x19, oy + 0x35);
        let tx = ox + 0x3b;
        if p < 0 {
            self.text(b" Random", 0, color, tx, oy + 0xce);
        } else {
            let name = self.gd.objects[p as usize].name.clone();
            self.text(&name, 0, color, tx, oy + 0xce);
        }
        let mut draw_kind = false;
        if self.r(self.ch().sub) == 1 {
            if self.r(KEY_RIGHT) != 0 || self.r(KEY_LEFT) != 0 {
                let h = (self.ri(HUMAN, cur) < 1) as i32;
                self.wi(HUMAN, cur, h);
            }
            if self.r(KEY_ATTACK) != 0 {
                self.w(KEY_ATTACK, 0);
                self.buffer_sounds.push("data\\m_join.wav");
                self.w(self.ch().cur, cur + 1);
                self.w(self.ch().sub, 0);
                if cur + 1 == 8 {
                    self.g.set_match_phase(self.ph(0x16));
                    self.w(self.ch().menu, 0);
                }
            } else if self.r(KEY_JUMP) != 0 {
                self.w(KEY_JUMP, 0);
                self.buffer_sounds.push("data\\m_cancel.wav");
                self.w(self.ch().sub, self.r(self.ch().sub) - 1);
                draw_kind = 0 < self.r(self.ch().sub);
            } else {
                draw_kind = true;
            }
        } else if 0 < self.r(self.ch().sub) {
            draw_kind = true;
        }
        if draw_kind {
            let cur = self.r(self.ch().cur);
            let color = self.champ_blink(self.r(self.ch().sub) == 1);
            let t: &[u8] = if self.ri(HUMAN, cur) == 0 { b"Computer" } else { b" Human" };
            self.text(t, 0, color, tx, oy + 0xfd);
        }
    }

    /// The OK / Back box of phases 0x16 and 0x1b (`title` is its title clip). Returns
    /// true when the frame ends.
    fn confirm_box(&mut self, title: i32) -> bool {
        self.blit(CLIP4, 0x116, 0xa2, 2, false);
        self.blit(CLIP4, 0x13a, 0xb3, title, false);
        self.blit(CLIP4, 0x142, 0xe8, 5, false);
        self.blit(CLIP4, 0x1aa, 0xe8, 6, false);
        if self.r(self.ch().menu) == 0 {
            self.blit(CLIP4, 0x13f, 0xe5, 7, false);
        } else {
            self.blit(CLIP4, 0x1a4, 0xe6, 8, false);
        }
        if self.r(KEY_LEFT) != 0 || self.r(KEY_RIGHT) != 0 {
            self.w(self.ch().menu, 1 - self.r(self.ch().menu));
        }
        if self.r(KEY_ATTACK) == 0 {
            if self.r(KEY_JUMP) != 0 {
                self.buffer_sounds.push("data\\m_cancel.wav");
                self.w(KEY_JUMP, 0);
                self.g.set_match_phase(self.ph(0x15));
                self.w(self.ch().sub, 1);
                self.w(self.ch().cur, 7);
                for i in 0..8 {
                    self.wi(ORDER, i, i);
                    if self.g.game_mode() == 3 {
                        self.wi(TEAM, i, i / 2 + 1);
                    }
                }
            }
            return false;
        }
        self.w(KEY_ATTACK, 0);
        self.buffer_sounds.push("data\\m_join.wav");
        if self.r(self.ch().menu) == 0 {
            self.g.set_match_phase(self.ph(0x17));
            self.w(self.ch().timer, 0);
            return self.shuffle_bracket();
        }
        self.w(self.ch().menu, 0);
        self.g.set_match_phase(self.ph(0x18));
        self.resolve_contestants();
        let m = self.g.game_mode();
        self.champ_menu(m)
    }

    /// Phase 0x17: shuffle the bracket for ten frames. Returns true when the frame ends.
    fn shuffle_bracket(&mut self) -> bool {
        if self.g.game_mode() == 3 {
            for _ in 0..0x32 {
                let a = self.random(0x101, 4);
                let b = self.random(0x102, 4);
                for (base, k) in [(ORDER, 0), (ORDER, 1), (TEAM, 0), (TEAM, 1)] {
                    let va = self.ri(base, 2 * a + k);
                    let vb = self.ri(base, 2 * b + k);
                    self.wi(base, 2 * a + k, vb);
                    self.wi(base, 2 * b + k, va);
                }
            }
        } else {
            for _ in 0..0x32 {
                let a = self.random(0xf7, 8);
                let b = self.random(0xf8, 8);
                let va = self.ri(ORDER, a);
                let vb = self.ri(ORDER, b);
                self.wi(ORDER, a, vb);
                self.wi(ORDER, b, va);
            }
        }
        let t = self.r(self.ch().timer) + 1;
        self.w(self.ch().timer, t);
        if t % 5 == 0 {
            self.buffer_sounds.push("data\\m_ok.wav");
        }
        if 10 < t {
            self.g.set_match_phase(self.ph(0x16));
            self.w(self.ch().menu, 0);
            return true;
        }
        false
    }

    /// LAB_00433550: random contestants get a fighter of id below 30 nobody has.
    fn resolve_contestants(&mut self) {
        for i in 0..8 {
            if self.ri(RANDOM, i) != 1 {
                continue;
            }
            let picks: Vec<i32> = (0..8).map(|j| self.ri(PICKS, j)).collect();
            let list: Vec<i32> = (1..self.gd.objects.len() as i32)
                .filter(|&k| {
                    let d = &self.gd.objects[k as usize];
                    d.kind == 0 && !picks.contains(&k) && d.id < 0x1e
                })
                .collect();
            let site = if self.g.game_mode() == 3 { 0x103 } else { 0xf9 };
            let r = self.random(site, list.len() as i32);
            self.wi(PICKS, i, list.get(r as usize).copied().unwrap_or(0));
        }
        self.g.set_match_phase(self.ph(0x19));
        self.w(self.ch().menu, 2);
    }

    /// Phase 0x19 (LAB_00433660): the menu. Returns true when the frame ends.
    fn champ_menu(&mut self, _mode: i32) -> bool {
        use crate::menu::main_menu::MENU_CLIP3;
        let (l, r) = (self.r(KEY_LEFT) != 0, self.r(KEY_RIGHT) != 0);
        let m = self.g.game_mode();
        self.draw_music_menu(m, l, r);
        self.blit(MENU_CLIP3, 3, 3, 8, false);
        self.blit(MENU_CLIP3, 3, 0x9f, 0xf, true);
        let hl = [(0x5c, 0x10, 9), (0x40, 0x27, 10), (0x28, 0x40, 0xb), (0xf, 0x57, 0xc), (0x25, 0x6f, 0xd), (0x65, 0x89, 0xe)];
        let m = self.r(self.ch().menu);
        if (0..6).contains(&m) {
            let (x, y, c) = hl[m as usize];
            self.blit(MENU_CLIP3, x, y, c, false);
        }
        if self.r(crate::menu::pick::BG_RANDOM) == 1 {
            self.g.set_background(100);
        }
        let name = self.gd.bg_name(self.g.background()).to_vec();
        self.text(&name, 0, 0xff9b9b, 0xae, 0x5b);
        let d: Option<&[u8]> = match self.g.difficulty() {
            2 => Some(b"Easy"),
            1 => Some(b"Normal"),
            0 | -1 => Some(b"Difficult"),
            _ => None,
        };
        if let Some(d) = d {
            self.text(d, 0, 0xff9b9b, 0xae, 0x73);
        }
        if self.r(KEY_UP) != 0 {
            let v = self.r(self.ch().menu) - 1;
            self.w(self.ch().menu, if v == -1 { 5 } else { v });
        }
        if self.r(KEY_DOWN) != 0 {
            self.w(self.ch().menu, (self.r(self.ch().menu) + 1) % 6);
        }
        if self.r(KEY_ATTACK) == 0 {
            if self.r(KEY_JUMP) != 0 {
                self.w(KEY_JUMP, 0);
                self.buffer_sounds.push("data\\m_cancel.wav");
                self.g.set_match_phase(self.ph(0x16));
                self.w(self.ch().menu, 0);
                for i in 0..8 {
                    if self.ri(RANDOM, i) == 1 {
                        self.wi(PICKS, i, -1);
                    }
                }
            }
            return false;
        }
        self.w(KEY_ATTACK, 0);
        self.buffer_sounds.push("data\\m_ok.wav");
        match self.r(self.ch().menu) {
            0 => {
                self.g.set_match_phase(self.ph(0x1a));
                for i in 0..8 {
                    self.wi(HP, i, 500);
                    self.wi(LAST_CAUGHT, i, -1);
                    let start = if self.g.game_mode() == 3 { 2 } else { 0 };
                    self.wi(PROGRESS, i, start);
                }
                self.w(self.ch().timer, 0x19);
                self.w(ROUND, 0);
                let f: Vec<u8> = (0..60).map(|k| self.g.m.u8(0x44eed0 + k)).take_while(|&c| c != 0).collect();
                if !f.is_empty() {
                    self.play_music(&f);
                }
            }
            1 => {
                self.g.set_match_phase(self.ph(0x14));
                return true;
            }
            2 => {
                self.g.set_match_phase(self.ph(0x18));
                return true;
            }
            3 => {
                self.buffer_sounds.push("data\\m_ok.wav");
                self.w(crate::menu::pick::BG_RANDOM, 0);
                let n = self.gd.backgrounds.len() as i32;
                let next = match self.g.background() {
                    100 => 99,
                    99 => 0,
                    b => b + 1,
                };
                self.g.set_background(next);
                if next == n {
                    self.g.set_background(100);
                    self.w(crate::menu::pick::BG_RANDOM, 1);
                }
            }
            4 => {
                let mut d = self.g.difficulty() - 1;
                if d < 0 {
                    d = 2;
                }
                self.w(crate::post_update::DIFFICULTY, d);
                self.buffer_sounds.push("data\\m_ok.wav");
            }
            5 => {
                self.g.set_match_phase(10);
                self.end_match();
                self.w(MOUSE_CLICK, 0);
            }
            _ => {}
        }
        false
    }

    /// Phase 0x1a: find the next pair; computer pairs are decided at once.
    fn next_pair(&mut self) {
        let round = self.r(ROUND);
        let mut n = 0;
        for p in 0..8 {
            if self.ri(PROGRESS, p) == round {
                self.wi(self.ch().pair, n, p);
                n += 1;
                self.w(self.ch().pair_count, n);
                if n == 2 {
                    break;
                }
            }
        }
        self.w(self.ch().pair_count, n);
        if n == 0 {
            if round < 6 {
                self.w(ROUND, round + 2);
            }
            return;
        }
        if n != 2 {
            return;
        }
        if 0x1e < self.r(self.ch().timer) {
            let p = if 4 < self.r(self.ch().blink) % 10 { self.ri(self.ch().pair, 1) } else { self.ri(self.ch().pair, 0) };
            let k = (self.ri(PROGRESS, p) + 2).min(7);
            self.bracket_boxes(p, k, 0xffa0a0);
        }
        let t = self.r(self.ch().timer) + 1;
        self.w(self.ch().timer, t);
        if self.r(KEY_JUMP) != 0 {
            self.w(self.ch().timer, 0x50);
        } else if t != 0x50 {
            return;
        }
        let (a, b) = (self.ri(self.ch().pair, 0), self.ri(self.ch().pair, 1));
        let (ha, hb) = (self.ri(HUMAN, self.ri(ORDER, a)), self.ri(HUMAN, self.ri(ORDER, b)));
        if ha == 0 && hb == 0 {
            self.buffer_sounds.push("data\\m_join.wav");
            self.w(self.ch().timer, 0);
            let boss = |w: &World, p: i32| w.gd.objects[w.ri(PICKS, w.ri(ORDER, p)) as usize].id / 10 == 5;
            let w = match (boss(self, a), boss(self, b)) {
                (true, true) => self.random(0xfb, 2),
                (true, false) => 0,
                (false, true) => 1,
                (false, false) => self.random(0xfa, 2),
            };
            let win = self.ri(self.ch().pair, w);
            let lose = self.ri(self.ch().pair, 1 - w);
            self.wi(PROGRESS, win, self.ri(PROGRESS, win) + 2);
            self.wi(PROGRESS, lose, self.ri(PROGRESS, lose) + 1);
            if self.ri(PROGRESS, win) == 6 {
                self.wi(PROGRESS, win, 7);
            }
            let d = self.g.difficulty();
            let range = if d < -1 { 0 } else { d };
            let r = self.random(0xfc, range * 0xc + 0xf);
            let hp = ((r + d * -0xc + 0x55) * self.ri(HP, win)) / 100;
            self.wi(HP, win, hp);
        } else {
            self.g.set_match_phase(0x1b);
            self.w(self.ch().timer, 0);
            self.reset_assignments();
            self.wi(PROGRESS, a, self.ri(PROGRESS, a) + 1);
            self.wi(PROGRESS, b, self.ri(PROGRESS, b) + 1);
        }
    }

    /// Phase 0x1b: each human of the pair presses attack to take a player slot; then
    /// the match starts.
    fn champ_assign(&mut self) {
        let t = self.r(self.ch().timer);
        if t < 2 {
            let p = self.ri(self.ch().pair, t);
            if self.r(self.ch().blink) % 10 < 5 {
                let k = (self.ri(PROGRESS, p) + 1).min(7);
                self.bracket_boxes(p, k, 0xffa0a0);
            }
            let h = self.ri(HUMAN, self.ri(ORDER, p));
            if h < 1 {
                self.w(self.ch().timer, t + 1);
                self.w(self.ch().menu, 1);
            } else {
                self.blit(CLIP4, 10, 10, 10, false);
                self.blit(CLIP4, 0xa9, 0x18, h + 10, false);
                let pick = self.ri(PICKS, self.ri(ORDER, p));
                self.face_of(pick, 0x5a, 0x31);
                for i in 0..8usize {
                    let o = &self.objs[i];
                    if o.attack() == 0 || o.prev_attack() != 0 {
                        continue;
                    }
                    if self.g.m.u8(EXISTS + i as u32) != 1 {
                        self.w(KEY_ATTACK, 0);
                        self.objs[i].set_prev_attack(1);
                        self.buffer_sounds.push("data\\m_join.wav");
                        self.wi(ASSIGN, i as i32, p);
                        self.g.m.set_u8(EXISTS + i as u32, 1);
                        self.w(self.ch().timer, t + 1);
                        self.objs[i].m.set_i32(0x368, pick);
                        break;
                    }
                    self.buffer_sounds.push("data\\m_cancel.wav");
                }
            }
        }
        if self.r(self.ch().timer) == 2 {
            self.blit(CLIP4, 0x116, 0xa2, 2, false);
            self.blit(CLIP4, 0x13a, 0xb3, 4, false);
            self.blit(CLIP4, 0x142, 0xe8, 5, false);
            self.blit(CLIP4, 0x1aa, 0xe8, 6, false);
            if self.r(self.ch().menu) == 0 {
                self.blit(CLIP4, 0x13f, 0xe5, 7, false);
            } else {
                self.blit(CLIP4, 0x1a4, 0xe6, 8, false);
            }
            if self.r(KEY_LEFT) != 0 || self.r(KEY_RIGHT) != 0 {
                self.w(self.ch().menu, 1 - self.r(self.ch().menu));
            }
            if self.r(KEY_ATTACK) == 0 {
                return;
            }
            self.w(KEY_ATTACK, 0);
            self.buffer_sounds.push("data\\m_join.wav");
            if self.r(self.ch().menu) != 0 {
                self.w(self.ch().timer, 0);
                self.reset_assignments();
                return;
            }
            self.w(self.ch().timer, 3);
        }
        if self.r(self.ch().timer) != 3 {
            return;
        }
        for k in 0..2 {
            let p = self.ri(self.ch().pair, k);
            if self.ri(HUMAN, self.ri(ORDER, p)) != 0 {
                continue;
            }
            for i in 0..8u32 {
                if self.g.m.u8(EXISTS + i) == 0 {
                    self.buffer_sounds.push("data\\m_join.wav");
                    self.wi(ASSIGN + 0x28, i as i32, p);
                    self.g.m.set_u8(EXISTS + 10 + i, 1);
                    let pick = self.ri(PICKS, self.ri(ORDER, p));
                    self.objs[10 + i as usize].m.set_i32(0x368, pick);
                    break;
                }
            }
        }
        self.g.set_match_phase(0);
        self.start_championship_match(0xfd, (0xfe, 0xff));
    }

    /// The match start of the championships (0x4345xx): every existing object of 0..19
    /// gets the hp its contestant has left.
    pub(crate) fn start_championship_match(&mut self, bg_site: i32, sites: (i32, i32)) {
        self.wr_lfr_flags();
        let n = self.gd.backgrounds.len() as i32;
        if self.r(crate::menu::pick::BG_RANDOM) == 1 {
            let mut b = self.random(bg_site, n - 2);
            if b == n - 3 {
                b = 99;
            }
            self.g.set_background(b);
        }
        let bg = self.g.background();
        let (bw, (z0, z1)) = {
            let b = self.gd.bg(bg);
            (b.width, b.zboundary)
        };
        for i in 0..20usize {
            if self.g.m.u8(EXISTS + i as u32) == 0 {
                continue;
            }
            let a = self.ri(ASSIGN, i as i32);
            let hp = self.ri(HP, a);
            let caught = self.ri(LAST_CAUGHT, a);
            {
                let data = self.objs[i].m.i32(0x368);
                let whp = self.gd.objects[data as usize].weapon_hp;
                let o = &mut self.objs[i];
                crate::collisions::obj_reset(o);
                o.m.set_i32(0x368, data);
                o.set_x(350.0);
                o.set_weapon_hp(whp);
                o.set_y(0.0);
                o.set_z(300.0);
                o.set_mp(500);
                o.set_hp(hp);
                o.set_dark_hp(hp);
                o.set_max_hp(hp);
                o.m.set_i32(0x33c, caught);
                o.set_team(a + 10);
                o.set_u_08(0x4b);
            }
            let x = self.random(sites.0, bw / 2) + bw / 4;
            let z = self.random(sites.1, z1 - z0) + z0;
            let o = &mut self.objs[i];
            o.set_ix(x);
            o.set_iz(z);
            o.set_iy(0);
            o.set_x(x as f64);
            o.set_z(z as f64);
            o.set_y(0.0);
            o.m.set_i32(0x354, i as i32);
        }
        for a in (0x450c04..=0x450c28).step_by(4) {
            self.w(a, 0);
        }
        self.w(0x450bbc, 0);
        self.g.set_mp_cost_on(1);
        for j in 20..MAX_OBJECTS {
            if self.g.m.u8(EXISTS + j as u32) == 0 {
                crate::collisions::obj_reset(&mut self.objs[j]);
            }
        }
        self.end_match();
        self.recording_start();
    }

    fn wr_lfr_flags(&mut self) {
        // the file name is not compared (the time differs)
        let mut suffix = if self.g.game_mode() == 3 { "_2on2_" } else { "_1on1_" }.to_string();
        suffix += match self.r(ROUND) {
            0 => "Prelminar",
            2 => "SemiFinal",
            4 => "Final",
            _ => "",
        };
        self.recording_name(&suffix);
        self.w(0x450b6c, 0);
        if self.r(0x450be4) != 0 {
            self.w(0x450b70, 1);
        }
    }

    /// Phase 0x1c: keep the hp of the pair and move the winner on.
    fn after_champ_match(&mut self) {
        for i in 0..20 {
            let a = self.ri(ASSIGN, i);
            if -1 < a {
                self.wi(HP, a, self.objs[i as usize].dark_hp());
                self.wi(LAST_CAUGHT, a, self.objs[i as usize].m.i32(0x33c));
            }
        }
        let team = self.g.m.i32(0x450bf8);
        let p = if team < 10 {
            let r = self.random(0x100, 2);
            self.ri(self.ch().pair, r)
        } else {
            team - 10
        };
        self.wi(PROGRESS, p, self.ri(PROGRESS, p) + 1);
        if self.ri(PROGRESS, p) == 6 {
            self.wi(PROGRESS, p, 7);
        }
        self.g.set_match_phase(0x1a);
        self.reset_assignments();
    }

    /// Phase 0x1d: the champion.
    fn champion_screen(&mut self) {
        if self.r(self.ch().timer) < 0x32 {
            self.w(self.ch().timer, self.r(self.ch().timer) + 1);
        }
        if self.r(self.ch().timer) <= 0x14 {
            return;
        }
        if self.r(self.ch().timer) == 0x15 {
            self.music_stop();
            self.buffer_sounds.push("data\\m_pass.wav");
        }
        self.blit(CLIP4, 10, 10, 10, false);
        let wnr = self.r(self.ch().winner);
        let h = self.ri(HUMAN, self.ri(ORDER, wnr));
        if h == 0 {
            self.blit(CLIP4, 0x5d, 0x16, 0x14, false);
        } else {
            self.blit(CLIP4, 0xa9, 0x18, h + 10, false);
        }
        let pick = self.ri(PICKS, self.ri(ORDER, wnr));
        self.face_of(pick, 0x5a, 0x31);
        self.blit(CLIP4, 0x17, 0xaf, 0x13, false);
        if self.r(KEY_ATTACK) != 0 && self.r(self.ch().timer) == 0x32 {
            self.g.set_match_phase(10);
            self.end_match();
            self.w(MOUSE_CLICK, 0);
        }
    }
}

impl World {
    /// 0x434ab0: the 2 on 2 championship (match_phase 0x78..0x81). The contestant
    /// screens and the menu are shared with the 1 on 1 code; teams of two follow.
    pub(crate) fn championship_2on2(&mut self) {
        self.menu_keys();
        let c = C2;
        if self.g.match_phase() == 0x78 {
            self.g.set_match_phase(0x79);
            self.w(c.cur, 0);
            self.w(c.sub, 0);
            for i in 0..8 {
                self.wi(TEAM, i, i / 2 + 1);
                self.wi(RANDOM, i, 1);
                self.wi(PICKS, i, -1);
                self.wi(HUMAN, i, 0);
                self.wi(ORDER, i, i);
            }
        }
        if 0x78 < self.g.match_phase() {
            self.w(c.blink, (self.r(c.blink) + 1) % 0x1e);
            let (ox, oy) = if self.g.match_phase() < 0x7c {
                self.blit(CLIP4, 0x1b, 0x6a, 0, false);
                self.fill(0x2f, 0x180, 0x83, 0x29, 0x324d9a);
                (0x1b, 0x6a)
            } else {
                self.blit(CLIP4, 0x77, 0x7e, 9, false);
                (-0x33, 0x7e)
            };
            use crate::menu::main_menu::MENU_CLIP3;
            self.blit(MENU_CLIP3, ox + 0x135, oy + 0x17, 0x15, false);
            self.blit(MENU_CLIP3, ox + 0xe8, oy + 0xd7, 0x10, false);
            for t in 0..4 {
                let x = ox + 0xed + 0x78 * t;
                self.fill(x - 5, oy + 0xdd, 0x49, 0x24, 0x324d9a);
                let team = self.ri(TEAM, 2 * t);
                self.blit(MENU_CLIP3, x, oy + 0xdf, team + 0x10, false);
            }
            if self.r(c.cur) < 8 {
                self.choose_contestant(ox, oy);
            }
            let cur = self.r(c.cur);
            let mut n = 0;
            for i in 0..cur.max(0) {
                if 0 < self.ri(HUMAN, i) {
                    n += 1;
                    self.wi(HUMAN, i, n);
                }
            }
            self.w(HUMANS, n);
            let col = |i: i32| 10 - if i % 2 != 0 { 0x14 } else { 0 } + i * 0x3c;
            for i in 0..cur.max(0) {
                let idx = self.ri(ORDER, i);
                let x = col(i) + ox;
                self.small_face(self.ri(PICKS, idx), x + 0xd8, oy + 0xfc);
                let h = self.ri(HUMAN, idx);
                if h == 0 {
                    self.label(b'C', 0x8f472f, 0x9679c8, x + 0xe5, oy + 299);
                } else {
                    self.label(h as u8 + b'0', 0x8f472f, 0xe6794d, x + 0xe5, oy + 299);
                }
            }
            if cur < 8 {
                let idx = self.ri(ORDER, cur);
                let x = col(cur) + ox;
                self.small_face(self.ri(PICKS, cur), x + 0xd8, oy + 0xfc);
                if 0 < self.r(c.sub) {
                    if self.ri(HUMAN, idx) == 0 {
                        self.label(b'C', 0x8f472f, 0x9679c8, x + 0xe5, oy + 299);
                    } else {
                        let ch = self.r(HUMANS) as u8 + b'1';
                        self.label(ch, 0x8f472f, 0xe6794d, x + 0xe5, oy + 299);
                    }
                }
            }
        }
        let phase = self.g.match_phase();
        if phase == 0x7a {
            if self.confirm_box(3) {
                return;
            }
        } else if phase == 0x7b {
            if self.shuffle_bracket() {
                return;
            }
        } else if phase == 0x7c {
            self.resolve_contestants();
            if self.champ_menu(3) {
                return;
            }
        } else if phase == 0x7d && self.champ_menu(3) {
            return;
        }
        if 0x7d < self.g.match_phase() {
            for p in (0..8).step_by(2) {
                let mut prog = self.ri(PROGRESS, p);
                if 8 <= prog {
                    prog = 7;
                }
                if prog == 7 && self.g.match_phase() < 0x81 {
                    self.g.set_match_phase(0x81);
                    self.w(c.winner, p);
                    self.w(c.timer, 0);
                }
                let color = if prog < 7 && prog % 2 == 1 { 0x192f67 } else { 0xffffff };
                self.bracket_boxes_from(p, 2, prog, color);
            }
        }
        if self.g.match_phase() == 0x7e {
            self.next_pair2();
        }
        if self.g.match_phase() == 0x7f {
            self.assign2();
        }
        if self.g.match_phase() == 0x80 {
            self.after_match2();
        }
        if self.g.match_phase() == 0x81 {
            self.champion2();
        }
    }

    fn next_pair2(&mut self) {
        let c = C2;
        let round = self.r(ROUND);
        let mut n = 0;
        for p in (0..8).step_by(2) {
            if self.ri(PROGRESS, p) == round {
                self.wi(c.pair, n, p);
                self.wi(c.pair, n + 1, p + 1);
                n += 2;
                if n == 4 {
                    break;
                }
            }
        }
        self.w(c.pair_count, n);
        if n == 0 {
            if round < 6 {
                self.w(ROUND, round + 2);
            }
            return;
        }
        if n != 4 {
            return;
        }
        if 0x1e < self.r(c.timer) {
            let p = if 4 < self.r(c.blink) % 10 { self.ri(c.pair, 2) } else { self.ri(c.pair, 0) };
            let k = (self.ri(PROGRESS, p) + 2).min(7);
            self.bracket_boxes_from(p, 2, k, 0xffa7a7);
        }
        let t = self.r(c.timer) + 1;
        self.w(c.timer, t);
        if self.r(KEY_JUMP) != 0 {
            self.w(c.timer, 0x50);
        } else if t != 0x50 {
            return;
        }
        let pr: Vec<i32> = (0..4).map(|k| self.ri(c.pair, k)).collect();
        let all_com = pr.iter().all(|&p| self.ri(HUMAN, self.ri(ORDER, p)) == 0);
        if all_com {
            self.buffer_sounds.push("data\\m_join.wav");
            self.w(c.timer, 0);
            let boss = |w: &World, p: i32| w.gd.objects[w.ri(PICKS, w.ri(ORDER, p)) as usize].id / 10 == 5;
            let a = boss(self, pr[0]) || boss(self, pr[1]);
            let b = boss(self, pr[2]) || boss(self, pr[3]);
            let w = match (a, b) {
                (true, true) => self.random(0x106, 2) * 2,
                (true, false) => 0,
                (false, true) => 2,
                (false, false) => self.random(0x105, 2) * 2,
            } as usize;
            for &p in &[pr[w], pr[w + 1]] {
                self.wi(PROGRESS, p, self.ri(PROGRESS, p) + 2);
            }
            for &p in &[pr[2 - w], pr[3 - w]] {
                self.wi(PROGRESS, p, self.ri(PROGRESS, p) + 1);
            }
            for &p in &[pr[w], pr[w + 1]] {
                if self.ri(PROGRESS, p) == 6 {
                    self.wi(PROGRESS, p, 7);
                }
            }
            let d = self.g.difficulty();
            let r1 = self.random(0x107, d * 0x12 + 0xf);
            let hp = ((r1 + d * -0x12 + 0x55) * self.ri(HP, pr[w])) / 100;
            self.wi(HP, pr[w], hp);
            let r2 = self.random(0x108, d * 0x12 + 0xf);
            let hp = ((r2 + d * -0x12 + 0x55) * self.ri(HP, pr[w + 1])) / 100;
            self.wi(HP, pr[w + 1], hp);
        } else {
            self.g.set_match_phase(0x7f);
            self.w(c.timer, 0);
            self.reset_assignments();
            for &p in &pr {
                self.wi(PROGRESS, p, self.ri(PROGRESS, p) + 1);
            }
        }
    }

    fn assign2(&mut self) {
        let c = C2;
        let t = self.r(c.timer);
        if t < 4 {
            let p = self.ri(c.pair, t);
            if self.r(c.blink) % 10 < 5 {
                let k = (self.ri(PROGRESS, p) + 1).min(7);
                self.bracket_boxes_from(p, 2, k, 0xffa7a7);
            }
            let h = self.ri(HUMAN, self.ri(ORDER, p));
            if h < 1 {
                self.w(c.timer, t + 1);
                self.w(c.menu, 1);
            } else {
                self.blit(CLIP4, 10, 10, 10, false);
                self.blit(CLIP4, 0xa9, 0x18, h + 10, false);
                let pick = self.ri(PICKS, self.ri(ORDER, p));
                self.face_of(pick, 0x5a, 0x31);
                for i in 0..8usize {
                    let o = &self.objs[i];
                    if o.attack() == 0 || o.prev_attack() != 0 {
                        continue;
                    }
                    if self.g.m.u8(EXISTS + i as u32) != 1 {
                        self.w(KEY_ATTACK, 0);
                        self.objs[i].set_prev_attack(1);
                        self.buffer_sounds.push("data\\m_join.wav");
                        self.wi(ASSIGN, i as i32, p);
                        self.g.m.set_u8(EXISTS + i as u32, 1);
                        self.w(c.timer, t + 1);
                        self.objs[i].m.set_i32(0x368, pick);
                        break;
                    }
                    self.buffer_sounds.push("data\\m_cancel.wav");
                }
            }
        }
        if self.r(c.timer) == 4 {
            self.blit(CLIP4, 0x116, 0xa2, 2, false);
            self.blit(CLIP4, 0x13a, 0xb3, 4, false);
            self.blit(CLIP4, 0x142, 0xe8, 5, false);
            self.blit(CLIP4, 0x1aa, 0xe8, 6, false);
            if self.r(c.menu) == 0 {
                self.blit(CLIP4, 0x13f, 0xe5, 7, false);
            } else {
                self.blit(CLIP4, 0x1a4, 0xe6, 8, false);
            }
            if self.r(KEY_LEFT) != 0 || self.r(KEY_RIGHT) != 0 {
                self.w(c.menu, 1 - self.r(c.menu));
            }
            if self.r(KEY_ATTACK) == 0 {
                return;
            }
            self.w(KEY_ATTACK, 0);
            self.buffer_sounds.push("data\\m_join.wav");
            if self.r(c.menu) != 0 {
                self.w(c.timer, 0);
                self.reset_assignments();
                return;
            }
            self.w(c.timer, 5);
        }
        if self.r(c.timer) != 5 {
            return;
        }
        for k in 0..4 {
            let p = self.ri(c.pair, k);
            if self.ri(HUMAN, self.ri(ORDER, p)) != 0 {
                continue;
            }
            for i in 0..8u32 {
                if self.g.m.u8(EXISTS + i) == 0 && self.g.m.u8(EXISTS + 10 + i) == 0 {
                    self.buffer_sounds.push("data\\m_join.wav");
                    self.wi(ASSIGN + 0x28, i as i32, p);
                    self.g.m.set_u8(EXISTS + 10 + i, 1);
                    let pick = self.ri(PICKS, self.ri(ORDER, p));
                    self.objs[10 + i as usize].m.set_i32(0x368, pick);
                    break;
                }
            }
        }
        self.g.set_match_phase(0);
        self.start_championship_match(0x109, (0x10a, 0x10b));
        // the 2 on 2 teams are the bracket teams, not contestant + 10
        for i in 0..20 {
            if self.g.m.u8(EXISTS + i as u32) != 0 {
                let a = self.ri(ASSIGN, i);
                let t = self.ri(TEAM, a);
                self.objs[i as usize].set_team(t);
            }
        }
    }

    fn after_match2(&mut self) {
        let c = C2;
        let winner = self.g.m.i32(0x450bf8);
        for i in 0..20usize {
            let a = self.ri(ASSIGN, i as i32);
            if a < 0 {
                continue;
            }
            if self.objs[i].hp() < 1 {
                let d = self.objs[i].dark_hp() / 2;
                self.objs[i].set_dark_hp(d);
            }
            self.wi(HP, a, self.objs[i].dark_hp());
            self.wi(LAST_CAUGHT, a, self.objs[i].m.i32(0x33c));
            if self.objs[i].team() == winner {
                self.wi(PROGRESS, a, self.ri(PROGRESS, a) + 1);
                if self.ri(PROGRESS, a) == 6 {
                    self.wi(PROGRESS, a, 7);
                }
            }
        }
        if winner == -1 {
            let r = self.random(0x10c, 2);
            for k in 0..2 {
                let p = self.ri(c.pair, 2 * r + k);
                self.wi(PROGRESS, p, self.ri(PROGRESS, p) + 1);
                if self.ri(PROGRESS, p) == 6 {
                    self.wi(PROGRESS, p, 7);
                }
            }
        }
        self.g.set_match_phase(0x7e);
        self.reset_assignments();
    }

    fn champion2(&mut self) {
        let c = C2;
        if self.r(c.timer) < 0x32 {
            self.w(c.timer, self.r(c.timer) + 1);
        }
        if self.r(c.timer) <= 0x14 {
            return;
        }
        if self.r(c.timer) == 0x15 {
            self.music_stop();
            self.buffer_sounds.push("data\\m_pass.wav");
        }
        let w = self.r(c.winner);
        self.blit(CLIP4, 10, 10, 10, false);
        self.blit(CLIP4, 0x6d, 0x16, 0x15, false);
        self.blit(CLIP4, 0xa9, 0x18, self.ri(TEAM, w) + 10, false);
        let o = self.ri(ORDER, w);
        let (p0, p1) = (self.ri(PICKS, o), self.ri(PICKS, o + 1));
        self.face_of(p0, 0x1e, 0x31);
        self.face_of(p1, 0x96, 0x31);
        self.blit(CLIP4, 0x17, 0xaf, 0x13, false);
        if self.r(KEY_ATTACK) != 0 && self.r(c.timer) == 0x32 {
            self.g.set_match_phase(10);
            self.end_match();
            self.w(MOUSE_CLICK, 0);
        }
    }
}
