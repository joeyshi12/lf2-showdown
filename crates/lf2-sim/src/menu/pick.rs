//! The character selection screen and the menu after it (match_phase 1..), the part of
//! `game_menus` (0x429730) that runs for VS, stage and the championships, with
//! `draw_music_menu` (0x402130) and the "Fight!" set-up.
//!
//! Per pick slot s (objects 0..7): `SLOT_STATE[s]` is 0 not joined, 1 choosing a
//! fighter, 2 choosing a team, 3 ready; computer players use 11 (fighter), 12 (team)
//! and 13 (ready). `PICK[s]` is the chosen file-manager index or -1 for random.
//! `PICK_STATE` is 0 while players join, 1 in the "How many computer players?"
//! dialog, 2 while the computer players are chosen, 3 in the menu.

use crate::world::{World, EXISTS, MAX_OBJECTS};

pub const PICK_STATE: u32 = 0x4512c8;
pub const SLOT_STATE: u32 = 0x451288;
pub const HELD: u32 = 0x451268;
pub const PICK: u32 = 0x451248;
pub const WAS_RANDOM: u32 = 0x451228;
const BLINK: u32 = 0x451224;
const COM_RANDOM: u32 = 0x451220;
const COM_LIST: u32 = 0x451200;
const COM_CUR: u32 = 0x4511fc;
pub const COUNTDOWN: u32 = 0x44d078;
const PREV_PICK_STATE: u32 = 0x44d074;
pub const COM_COUNT: u32 = 0x44d070;
pub const MENU_CUR: u32 = 0x44d06c;
pub const BG_RANDOM: u32 = 0x44d028;
pub const MUSIC: u32 = 0x44f18c;
const MUSIC_ON: u32 = 0x44d010;
const MUSIC_NAME: u32 = 0x44f128;
const MUSIC_FILE: u32 = 0x44eed0;
const MUSIC_LINE: u32 = 0x44f060;

// surfaces game_menus loads (0x429730, first entry)
pub const CHARMENU: u32 = 0x4512c4;
pub const CM1: u32 = 0x4512b0;
pub const CMA: u32 = 0x4512ac;
pub const CMA2: u32 = 0x4512a8;
pub const CMC: u32 = 0x44fd88;
pub const RFACE: u32 = 0x44fd84;

/// Surfaces of game_menus (no clip tables): global, resource.
pub const SHEETS: &[(u32, &str)] = &[
    (CHARMENU, "CHARMENU"),
    (0x4512b0, "CM1"),
    (0x4512b4, "CM2"),
    (0x4512b8, "CM3"),
    (0x4512bc, "CM4"),
    (0x4512c0, "CM5"),
    (CMA, "CMA"),
    (CMA2, "CMA2"),
    (CMC, "CMC"),
    (RFACE, "RFACE"),
];

const WHITE: u32 = 0xffffff;
const COM_COLOR: u32 = 0x9b9bff;

fn cstr(w: &World, at: u32, max: u32) -> Vec<u8> {
    (0..max).map(|k| w.g.m.u8(at + k)).take_while(|&c| c != 0).collect()
}

fn put_cstr(w: &mut World, at: u32, s: &[u8]) {
    for (k, &b) in s.iter().chain(std::iter::once(&0)).enumerate() {
        w.g.m.set_u8(at + k as u32, b);
    }
}

impl World {
    fn rd(&self, a: u32) -> i32 {
        self.g.m.i32(a)
    }

    fn wr(&mut self, a: u32, v: i32) {
        self.g.m.set_i32(a, v)
    }

    fn arr(&self, base: u32, i: usize) -> i32 {
        self.g.m.i32(base + 4 * i as u32)
    }

    fn set_arr(&mut self, base: u32, i: usize, v: i32) {
        self.g.m.set_i32(base + 4 * i as u32, v)
    }

    /// The colour of blinking texts (COLORREF).
    fn blink_color(&self) -> u32 {
        let t = self.rd(BLINK);
        let c = ((t / 3) * 76 + t * 30) as u8;
        0xff0000 | ((c.wrapping_add(0x46) as u32) << 8) | c.wrapping_add(0x19) as u32
    }

    /// A fighter the pick screen offers: a character whose id is not 30..39 or
    /// 50..59 (soldiers and bosses), unless 0x458428 (F9 cheat) is set.
    fn pickable(&self, k: usize) -> bool {
        let d = &self.gd.objects[k];
        d.kind == 0 && ((d.id / 10 != 5 && d.id / 10 != 3) || self.g.u_458428() == 1)
    }

    fn pick_next(&mut self, s: usize) {
        let n = self.gd.objects.len() as i32;
        loop {
            let p = self.arr(PICK, s) + 1;
            self.set_arr(PICK, s, p);
            if n <= p {
                self.set_arr(PICK, s, -1);
                return;
            }
            if p == -1 {
                return;
            }
            if self.pickable(p as usize) {
                self.objs[s].m.set_i32(0x368, p);
                return;
            }
        }
    }

    fn pick_prev(&mut self, s: usize) {
        let n = self.gd.objects.len() as i32;
        let mut p = self.arr(PICK, s) - 1;
        self.set_arr(PICK, s, p);
        if p == -1 {
            return;
        }
        loop {
            if p < -1 {
                p = n - 1;
                self.set_arr(PICK, s, p);
            }
            if self.pickable(p as usize) {
                self.objs[s].m.set_i32(0x368, p);
                return;
            }
            p -= 1;
            self.set_arr(PICK, s, p);
            if p + 1 == 0 {
                return;
            }
        }
    }

    /// Replace each random pick (WAS_RANDOM) with a fighter of id below 30 that nobody
    /// has picked, using random() site `site`.
    pub(crate) fn resolve_random_picks(&mut self, site: i32) {
        for s in 0..8 {
            if self.arr(WAS_RANDOM, s) == 0 {
                continue;
            }
            let picks: Vec<i32> = (0..8).map(|j| self.arr(PICK, j)).collect();
            let list: Vec<i32> = (1..self.gd.objects.len() as i32)
                .filter(|&k| {
                    let d = &self.gd.objects[k as usize];
                    d.kind == 0 && !picks.contains(&k) && d.id < 0x1e
                })
                .collect();
            let r = self.random(site, list.len() as i32);
            // the original indexes a stack array; an empty list reads garbage
            let k = list.get(r as usize).copied().unwrap_or(0);
            self.set_arr(PICK, s, k);
            self.objs[s].m.set_i32(0x368, k);
        }
    }

    fn mark_random_picks(&mut self) {
        for s in 0..8 {
            let v = (self.arr(PICK, s) < 0) as i32;
            self.set_arr(WAS_RANDOM, s, v);
        }
    }

    fn slot_xy(s: usize) -> (i32, i32) {
        ((s as i32 % 4) * 0x99, (s as i32 / 4) * 0xd4)
    }

    fn draw_face_and_name(&mut self, s: usize, color: u32) {
        let (x0, y0) = Self::slot_xy(s);
        if self.arr(PICK, s) < 0 {
            self.blit(RFACE, x0 + 0x93, y0 + 0x5e, -1, false);
            self.text(b" Random", 0, color, x0 + 0xb1, y0 + 0xf1);
        } else {
            let d = self.objs[s].m.i32(0x368) as usize;
            self.blit_sheet(crate::draw::Sheet::Face(d), x0 + 0x93, y0 + 0x5e, -1, false);
            let name = self.gd.objects[d].name.clone();
            self.text(&name, 0, color, x0 + 0xb1, y0 + 0xf1);
        }
    }

    fn draw_team(&mut self, s: usize, color: u32) {
        let (x0, y0) = Self::slot_xy(s);
        match self.objs[s].team() {
            0 => self.text(b"Independent", 0, color, x0 + 0xa7, y0 + 0x107),
            t @ 1..=4 => {
                let txt = format!("Team {}", t);
                self.text(txt.as_bytes(), 0, color, x0 + 0xbb, y0 + 0x107)
            }
            _ => {}
        }
    }

    /// The team nobody else may leave: when all the others are ready and in one team,
    /// the last one cannot join it too (VS and battle mode). -1 when there is none.
    fn forbidden_team(&self, s: usize) -> i32 {
        let mut acc = -1;
        for j in 0..8 {
            if self.arr(SLOT_STATE, j) % 10 != 3 || j == s {
                continue;
            }
            let t = self.objs[j].team();
            if t == 0 {
                acc = -2;
            } else if acc == -1 {
                acc = t;
            } else if acc >= 0 && t != acc {
                acc = -2;
            }
        }
        acc
    }

    fn team_invalid(&self, mode: i32, t: i32, forbidden: i32) -> bool {
        t == forbidden || (mode == 4 && (t == 0 || 2 < t))
    }

    /// The pressed key of object `s` as the pick screen tests them, in its order.
    fn pick_key(&self, s: usize, order: &[u32]) -> Option<u32> {
        order.iter().copied().find(|&k| self.objs[s].m.u8(k) != 0)
    }

    /// One frame of the pick screen and menu (0x429e9b..0x42e0d2).
    pub(crate) fn pick_screen(&mut self) {
        let mode = self.g.game_mode();
        let prev_state = self.rd(PICK_STATE);
        let phase = self.g.match_phase();
        if phase == 3 {
            for s in 0..8 {
                self.set_arr(SLOT_STATE, s, 0);
                self.set_arr(PICK, s, -1);
                self.set_arr(HELD, s, 1);
            }
            if mode == 1 {
                for i in 0..20 {
                    self.objs[i].set_team(1);
                }
            }
            if mode == 4 {
                for i in 0..8 {
                    let t = if i < 4 { 1 } else { 2 };
                    self.objs[i].set_team(t);
                    self.objs[i + 10].set_team(t);
                }
            }
            self.wr(PICK_STATE, 0);
            self.g.set_match_phase(1);
        } else if phase == 2 {
            self.wr(PICK_STATE, 3);
            self.g.set_match_phase(1);
        }
        let prev_state = if phase == 3 || phase == 2 { 0 } else { prev_state };
        if mode == 4 && self.rd(PICK_STATE) == 3 {
            if self.g.match_phase() < 200 {
                self.g.set_match_phase(200);
            }
            self.battle_menus();
            return;
        }
        let p = self.g.match_phase();
        if (200..300).contains(&p) {
            self.battle_menus();
            return;
        }
        self.wr(BLINK, (self.rd(BLINK) + 1) % 0x1e);
        for o in self.objs.iter_mut() {
            if !(0..=4).contains(&o.team()) {
                o.set_team(0);
            }
        }
        self.fill(0, 0, 0x31a, 0x226, 0);
        self.blit(CHARMENU, 0x28, 0x21, -1, false);
        let state = self.rd(PICK_STATE);
        let unjoined = if state == 0 { (0..8).filter(|&s| self.arr(SLOT_STATE, s) == 0).count() as i32 } else { -1 };
        let (mut faster, mut ready, mut idle_or_ready) = (false, 0, 0);
        for s in 0..8usize {
            self.pick_slot(s, mode, state, unjoined, &mut faster);
            if self.arr(SLOT_STATE, s) == 3 {
                ready += 1;
                self.g.m.set_u8(EXISTS + s as u32, 1);
            } else {
                self.g.m.set_u8(EXISTS + s as u32, 0);
            }
            let st = self.arr(SLOT_STATE, s);
            if st == 0 || st == 3 {
                idle_or_ready += 1;
            }
            if self.g.match_phase() == 10 {
                // the jump key on an empty pick screen went back to the main menu;
                // the loop goes on in the original, so it does here
            }
        }
        let cd = self.rd(COUNTDOWN);
        if ready < 1 || idle_or_ready != 8 {
            self.wr(COUNTDOWN, 0x96);
        } else {
            self.wr(COUNTDOWN, cd - 1);
            if faster {
                self.wr(COUNTDOWN, cd - 0x1f);
            }
            if ready == 8 {
                self.wr(COUNTDOWN, 0);
            }
        }
        self.wr(PREV_PICK_STATE, self.rd(PICK_STATE));
        if self.rd(PICK_STATE) == 1 {
            self.com_count_dialog(mode, prev_state);
        }
        let st = self.rd(PICK_STATE);
        if st == 2 || st == 3 {
            if !self.choose_computers(mode, prev_state) {
                self.pick_end();
                return;
            }
            if mode != 4 {
                self.pick_menu(mode, prev_state);
            }
        }
        self.pick_end();
    }

    fn pick_end(&mut self) {
        if self.rd(COUNTDOWN) < 1 && self.rd(PICK_STATE) == 0 {
            self.wr(PICK_STATE, 1);
        }
    }

    fn pick_slot(&mut self, s: usize, mode: i32, state: i32, unjoined: i32, faster: &mut bool) {
        let (x0, y0) = Self::slot_xy(s);
        let st = self.arr(SLOT_STATE, s);
        if st < 1 {
            if state != 0 {
                self.text(b"     ----", 0, WHITE, x0 + 0xb1, y0 + 0xdb);
            } else {
                let c = self.blink_color();
                self.text(b"   Join?", 0, c, x0 + 0xb1, y0 + 0xdb);
            }
        } else if st < 0xb {
            let name = cstr(self, 0x44fcc0 + 0xb * s as u32, 11);
            self.text(&name, 0, WHITE, x0 + 0xb1, y0 + 0xdb);
        } else {
            self.text(b"Computer", 0, COM_COLOR, x0 + 0xb1, y0 + 0xdb);
        }
        if self.arr(SLOT_STATE, s) == 0 && state == 0 {
            let cd = self.rd(COUNTDOWN);
            if cd == 0x96 {
                let sh = if self.rd(BLINK) % 5 < 3 { CMA } else { CMA2 };
                self.blit(sh, x0 + 0x93, y0 + 0x78, -1, false);
            } else if -1 < cd {
                self.blit(CM1 + 4 * (cd / 0x1e) as u32, x0 + 0xb5, y0 + 0x7d, -1, false);
            }
            let o = &self.objs[s];
            if o.attack() != 0 {
                if self.arr(HELD, s) == 0 && self.rd(PREV_PICK_STATE) == 0 {
                    self.set_arr(SLOT_STATE, s, 1);
                    self.buffer_sounds.push("data\\m_join.wav");
                }
                self.set_arr(HELD, s, 1);
            } else if o.jump() != 0 && unjoined == 8 {
                if self.arr(HELD, s) == 0 {
                    self.buffer_sounds.push("data\\m_cancel.wav");
                    self.g.set_match_phase(10);
                    self.wr(crate::menu::main_menu::MOUSE_CLICK, 0);
                    self.end_match();
                }
                self.set_arr(HELD, s, 1);
            } else {
                self.set_arr(HELD, s, 0);
            }
        }
        if self.arr(SLOT_STATE, s) == 1 {
            let c = self.blink_color();
            self.draw_face_and_name(s, c);
            let held = self.arr(HELD, s) != 0;
            match self.pick_key(s, &[0xd0, 0xcf, 0xcd, 0xd1, 0xd2]) {
                Some(0xd0) => {
                    if !held {
                        self.pick_next(s);
                    }
                }
                Some(0xcf) => {
                    if !held {
                        self.pick_prev(s);
                    }
                }
                Some(0xcd) => {
                    if !held {
                        self.set_arr(PICK, s, -1);
                    }
                }
                Some(0xd1) => {
                    if !held {
                        self.buffer_sounds.push("data\\m_join.wav");
                        self.set_arr(SLOT_STATE, s, 3 - (mode != 1) as i32);
                    }
                }
                Some(_) => {
                    if !held {
                        self.buffer_sounds.push("data\\m_cancel.wav");
                        self.set_arr(SLOT_STATE, s, 0);
                    }
                }
                None => {
                    self.set_arr(HELD, s, 0);
                    return self.pick_slot_ready(s, state, mode, faster);
                }
            }
            self.set_arr(HELD, s, 1);
        }
        if self.arr(SLOT_STATE, s) == 2 {
            let forbidden = if (0..8).filter(|&j| self.arr(SLOT_STATE, j) == 3).count() == 7 && (mode == 0 || mode == 4) {
                self.forbidden_team(s)
            } else {
                -1
            };
            self.draw_face_and_name(s, WHITE);
            while self.team_invalid(mode, self.objs[s].team(), forbidden) {
                let t = (self.objs[s].team() + 1) % 5;
                self.objs[s].set_team(t);
            }
            let c = self.blink_color();
            self.draw_team(s, c);
            if mode == 2 {
                self.objs[s].set_team(0);
                self.objs[s].set_attack(1);
                self.set_arr(HELD, s, 0);
            }
            let held = self.arr(HELD, s) != 0;
            match self.pick_key(s, &[0xd0, 0xcf, 0xd1, 0xd2]) {
                Some(0xd0) => {
                    if !held {
                        loop {
                            loop {
                                let t = (self.objs[s].team() + 1) % 5;
                                self.objs[s].set_team(t);
                                if t != forbidden {
                                    break;
                                }
                            }
                            let t = self.objs[s].team();
                            if !(mode == 4 && (t == 0 || 2 < t)) {
                                break;
                            }
                        }
                    }
                    self.set_arr(HELD, s, 1);
                }
                Some(0xcf) => {
                    if !held {
                        let dec = |w: &mut World| {
                            let mut t = w.objs[s].team() - 1;
                            if t < 0 {
                                t = 4;
                            }
                            w.objs[s].set_team(t);
                        };
                        dec(self);
                        while self.team_invalid(mode, self.objs[s].team(), forbidden) {
                            dec(self);
                        }
                    }
                    self.set_arr(HELD, s, 1);
                }
                Some(0xd1) => {
                    if !held {
                        self.buffer_sounds.push("data\\m_join.wav");
                        self.set_arr(SLOT_STATE, s, 3);
                    }
                    self.set_arr(HELD, s, 1);
                }
                Some(_) => {
                    if !held {
                        self.set_arr(SLOT_STATE, s, 1);
                        self.buffer_sounds.push("data\\m_cancel.wav");
                    }
                    self.set_arr(HELD, s, 1);
                }
                None => self.set_arr(HELD, s, 0),
            }
        }
        self.pick_slot_ready(s, state, mode, faster);
    }

    fn pick_slot_ready(&mut self, s: usize, state: i32, mode: i32, faster: &mut bool) {
        if self.arr(SLOT_STATE, s) != 3 {
            return;
        }
        self.draw_face_and_name(s, WHITE);
        self.draw_team(s, WHITE);
        if state == 0 {
            if self.objs[s].jump() == 0 {
                self.set_arr(HELD, s, 0);
            } else if self.rd(COUNTDOWN) == 0x96 {
                if self.arr(HELD, s) == 0 {
                    self.set_arr(SLOT_STATE, s, (mode != 1) as i32 + 1);
                    self.buffer_sounds.push("data\\m_cancel.wav");
                }
                self.set_arr(HELD, s, 1);
            } else if 0x95 < self.rd(COUNTDOWN) {
                self.set_arr(HELD, s, 0);
            } else {
                if self.arr(HELD, s) == 0 {
                    *faster = true;
                }
                self.set_arr(HELD, s, 1);
            }
        }
    }

    /// The "How many computer players?" dialog (PICK_STATE 1).
    fn com_count_dialog(&mut self, mode: i32, prev_state: i32) {
        self.wr(COM_RANDOM, 0);
        self.blit(CMC, 0xda, 0xd7, -1, false);
        let mut empty = 0;
        let mut teams = [0i32; 5];
        for s in 0..8 {
            if self.g.m.u8(EXISTS + s as u32) == 0 {
                empty += 1;
            } else {
                let t = self.objs[s].team().clamp(0, 4) as usize;
                if t == 0 || teams[t] == 0 {
                    teams[t] += 1;
                }
            }
        }
        if empty == 0 {
            self.wr(PICK_STATE, 3);
        }
        let min = if teams.iter().sum::<i32>() < 2 && mode != 1 { 1 } else { 0 };
        let n = self.rd(COM_COUNT);
        if n == -100 || n < min || empty < n {
            self.wr(COM_COUNT, min);
        }
        for s in 0..8usize {
            if self.g.m.u8(EXISTS + s as u32) == 0 {
                continue;
            }
            let held = self.arr(HELD, s) != 0;
            let o = &self.objs[s];
            let (r, l, a) = (o.right() != 0, o.left() != 0, o.attack() != 0);
            let mut confirm = false;
            if r {
                if !held && prev_state == 1 {
                    let n = self.rd(COM_COUNT) + 1;
                    self.wr(COM_COUNT, if empty < n { min } else { n });
                }
            } else if l {
                if !held && prev_state == 1 {
                    let n = self.rd(COM_COUNT) - 1;
                    self.wr(COM_COUNT, if n < min { empty } else { n });
                }
            } else if a || mode == 2 || mode == 3 {
                if mode == 2 || mode == 3 {
                    self.set_arr(HELD, s, 0);
                    self.wr(COM_COUNT, empty);
                }
                confirm = true;
            } else {
                self.set_arr(HELD, s, 0);
                continue;
            }
            if confirm {
                if self.arr(HELD, s) == 0 && prev_state == 1 {
                    if mode != 2 && mode != 3 {
                        self.buffer_sounds.push("data\\m_ok.wav");
                    }
                    self.wr(PICK_STATE, 2);
                    let n = self.rd(COM_COUNT);
                    if n == 0 {
                        self.wr(MENU_CUR, 2);
                        self.mark_random_picks();
                        self.wr(PICK_STATE, 3);
                        self.resolve_random_picks(0xd7);
                    } else {
                        let mut k = 0;
                        for j in 0..8usize {
                            if self.g.m.u8(EXISTS + j as u32) == 0 && k < n {
                                self.set_arr(COM_LIST, k as usize, j as i32);
                                self.set_arr(SLOT_STATE, j, 0xb);
                                k += 1;
                            }
                        }
                        self.wr(COM_CUR, 0);
                        self.set_arr(HELD, s, 1);
                    }
                    break;
                }
                // a held attack in modes 0/1/4 counts as held; mode 2/3 cleared it
            }
            self.set_arr(HELD, s, 1);
        }
        let (min, n) = (min, self.rd(COM_COUNT));
        for k in 0..8 {
            let color = if k < min || empty < k { 0xc06850 } else { WHITE };
            let x = 0x118 + 0x1e * k;
            self.text(k.to_string().as_bytes(), 0x9f472f, color, x + 6, 0x11e);
            if n == k {
                self.fill(x, 0x11b, 0x15, 1, WHITE);
                self.fill(x, 0x11b, 1, 0x15, WHITE);
                self.fill(x, 0x12f, 0x15, 1, WHITE);
                self.fill(x + 0x14, 0x11b, 1, 0x15, WHITE);
            }
        }
    }

    /// Choosing the computer players (PICK_STATE 2). Returns false when the frame
    /// ends here (the menu does not run).
    fn choose_computers(&mut self, mode: i32, prev_state: i32) -> bool {
        let (mut up, mut right, mut left, mut attack, mut jump) = (false, false, false, false, false);
        if self.rd(PICK_STATE) == 2 {
            for s in 0..8usize {
                if self.g.m.u8(EXISTS + s as u32) == 0 {
                    continue;
                }
                let first = self.arr(HELD, s) == 0 && prev_state == 2;
                match self.pick_key(s, &[0xcd, 0xce, 0xd0, 0xcf, 0xd1, 0xd2]) {
                    Some(0xcd) => up |= first,
                    Some(0xce) => {}
                    Some(0xd0) => right |= first,
                    Some(0xcf) => left |= first,
                    Some(0xd1) => attack |= first,
                    Some(_) => jump |= first,
                    None => {
                        self.set_arr(HELD, s, 0);
                        continue;
                    }
                }
                self.set_arr(HELD, s, 1);
            }
        }
        let n = self.rd(COM_COUNT);
        for k in 0..n.max(0) as usize {
            let s = self.arr(COM_LIST, k) as usize & 7;
            if 0xb < self.arr(SLOT_STATE, s) {
                self.draw_face_and_name(s, COM_COLOR);
            }
            if 0xc < self.arr(SLOT_STATE, s) {
                self.draw_team(s, COM_COLOR);
            }
        }
        let mut cur = self.rd(COM_CUR);
        let s = self.arr(COM_LIST, cur as usize) as usize & 7;
        if self.arr(SLOT_STATE, s) == 0xb {
            let c = self.blink_color();
            self.draw_face_and_name(s, c);
            if right {
                self.pick_next(s);
            }
            if left {
                self.pick_prev(s);
            }
            if up {
                self.set_arr(PICK, s, -1);
            }
            if self.rd(COM_RANDOM) == 1 {
                self.set_arr(PICK, s, -1);
                if mode != 2 {
                    self.set_arr(SLOT_STATE, s, 0xb);
                }
                if mode == 1 {
                    self.set_arr(SLOT_STATE, s, 0xb);
                }
                self.wr(COM_RANDOM, 0);
            }
            if attack {
                self.buffer_sounds.push("data\\m_join.wav");
                self.set_arr(SLOT_STATE, s, 0xc);
                attack = false;
                if mode == 1 {
                    let cs = self.arr(COM_LIST, cur as usize) as usize & 7;
                    self.set_arr(SLOT_STATE, s, 0xd);
                    self.wr(COM_RANDOM, (self.arr(PICK, cs) < 0) as i32);
                    cur += 1;
                    self.wr(COM_CUR, cur);
                    if n <= cur {
                        self.all_computers_chosen(0xd8);
                        jump = false;
                        return self.computer_team(mode, cur, attack, jump, right, left);
                    }
                    let next = self.arr(COM_LIST, cur as usize) as usize & 7;
                    self.set_arr(SLOT_STATE, next, 0xb);
                }
            }
            if jump {
                self.buffer_sounds.push("data\\m_cancel.wav");
                jump = false;
                if self.rd(COM_CUR) == 0 {
                    self.wr(PICK_STATE, 1);
                    for j in 0..8 {
                        if self.arr(SLOT_STATE, j) == 0xb {
                            self.set_arr(SLOT_STATE, j, 0);
                        }
                    }
                    cur = 0;
                } else {
                    let c = self.rd(COM_CUR);
                    let prev = self.arr(COM_LIST, (c - 1) as usize) as usize & 7;
                    self.set_arr(SLOT_STATE, prev, 0xc);
                    cur = c - 1;
                    self.wr(COM_CUR, cur);
                    if mode == 1 || mode == 2 {
                        self.set_arr(SLOT_STATE, prev, 0xb);
                    }
                }
            }
        }
        self.computer_team(mode, cur, attack, jump, right, left)
    }

    fn all_computers_chosen(&mut self, site: i32) {
        self.mark_random_picks();
        self.resolve_random_picks(site);
        self.wr(MENU_CUR, 2);
        self.wr(PICK_STATE, 3);
    }

    /// The team choice of the current computer player (LAB_0042c3bf).
    fn computer_team(&mut self, mode: i32, cur: i32, attack: bool, jump: bool, right: bool, left: bool) -> bool {
        let s = self.arr(COM_LIST, cur as usize) as usize & 7;
        if self.arr(SLOT_STATE, s) != 0xc {
            return self.rd(PICK_STATE) == 3;
        }
        let n = self.rd(COM_COUNT);
        let forbidden = if cur == n - 1 && (mode == 0 || mode == 4) { self.forbidden_team(s) } else { -1 };
        while self.team_invalid(mode, self.objs[s].team(), forbidden) {
            let t = (self.objs[s].team() + 1) % 5;
            self.objs[s].set_team(t);
        }
        let c = self.blink_color();
        self.draw_team(s, c);
        if right {
            let mut t = (self.objs[s].team() + 1) % 5;
            if t == forbidden {
                t = (t + 1) % 5;
            }
            self.objs[s].set_team(t);
        }
        if left {
            let dec = |w: &mut World| {
                let mut t = w.objs[s].team() - 1;
                if t < 0 {
                    t = 4;
                }
                w.objs[s].set_team(t);
            };
            dec(self);
            while self.team_invalid(mode, self.objs[s].team(), forbidden) {
                dec(self);
            }
        }
        let confirm = if mode == 2 {
            self.objs[s].set_team(0);
            true
        } else {
            attack
        };
        let mut check_jump = true;
        if confirm {
            self.buffer_sounds.push("data\\m_join.wav");
            let cs = self.arr(COM_LIST, self.rd(COM_CUR) as usize) as usize & 7;
            let r = self.arr(PICK, cs) < 0;
            self.set_arr(SLOT_STATE, s, 0xd);
            self.wr(COM_RANDOM, r as i32);
            let c = self.rd(COM_CUR) + 1;
            self.wr(COM_CUR, c);
            if c < n {
                let next = self.arr(COM_LIST, c as usize) as usize & 7;
                self.set_arr(SLOT_STATE, next, 0xb);
            } else {
                self.all_computers_chosen(0xd9);
                check_jump = false;
            }
        }
        if check_jump && jump {
            self.buffer_sounds.push("data\\m_cancel.wav");
            self.set_arr(SLOT_STATE, s, 0xb);
            self.wr(COM_RANDOM, 0);
        }
        self.rd(PICK_STATE) == 3
    }

    /// `draw_music_menu` (0x402130).
    pub(crate) fn draw_music_menu(&mut self, mode: i32, left: bool, right: bool) {
        let mut m = self.rd(MUSIC);
        let on = if mode == 1 {
            if !left && !right {
                m != -1
            } else if m == -1 {
                self.wr(MUSIC, 0);
                true
            } else {
                self.wr(MUSIC, -1);
                false
            }
        } else {
            if right {
                m += 1;
            }
            if 8 < m {
                m = -1;
            }
            if left {
                m -= 1;
            }
            if m < -1 {
                m = 8;
            }
            self.wr(MUSIC, m);
            let names: [&[u8]; 9] =
                [b"Random", b"Main Theme", b"Stage 1", b"Stage 2", b"Stage 3", b"Stage 4", b"Stage 5", b"Boss", b"Final Boss"];
            if (0..=8).contains(&m) {
                put_cstr(self, MUSIC_NAME, names[m as usize]);
            }
            let k = if m == 0 { self.random(1, 8) + 1 } else { m };
            self.set_music_file(k);
            m != -1
        };
        if on {
            if mode == 1 {
                put_cstr(self, MUSIC_NAME, b"ON");
            }
            self.wr(MUSIC_ON, 1);
        } else {
            put_cstr(self, MUSIC_NAME, b"OFF");
            self.wr(MUSIC_ON, 0);
            self.music_stop();
        }
        let line = [b"Music: ".as_slice(), &cstr(self, MUSIC_NAME, 20)].concat();
        put_cstr(self, MUSIC_LINE, &line);
        self.text(&line, 0, 0xd8775a, 0x25b, 3);
        self.text(b"(Press Left/Right to change)", 0, 0xa03f22, 0x25b, 0x14);
    }

    /// The bgm file for a music choice (1 main .. 8 boss2; -1 none).
    pub(crate) fn set_music_file(&mut self, k: i32) {
        let f: &[u8] = match k {
            1 => b"bgm\\main.wma",
            2 => b"bgm\\stage1.wma",
            3 => b"bgm\\stage2.wma",
            4 => b"bgm\\stage3.wma",
            5 => b"bgm\\stage4.wma",
            6 => b"bgm\\stage5.wma",
            7 => b"bgm\\boss1.wma",
            8 => b"bgm\\boss2.wma",
            -1 => {
                self.g.m.set_u8(MUSIC_FILE, 0);
                return;
            }
            _ => return,
        };
        put_cstr(self, MUSIC_FILE, f);
    }

    /// The menu after the selection (PICK_STATE 3): Fight!, Reset All, Reset Random,
    /// Stage/Background, Difficulty, Exit.
    fn pick_menu(&mut self, mode: i32, prev_state: i32) {
        self.blit(crate::menu::main_menu::MENU_CLIP3, 3, 3, 8, false);
        self.blit(crate::menu::main_menu::MENU_CLIP3, 3, 0x9f, 0xf, true);
        let hl = [(0x5c, 0x10, 9), (0x40, 0x27, 10), (0x28, 0x40, 0xb), (0xf, 0x57, 0xc), (0x25, 0x6f, 0xd), (0x65, 0x89, 0xe)];
        let cur = self.rd(MENU_CUR);
        if (0..6).contains(&cur) {
            let (x, y, c) = hl[cur as usize];
            self.blit(crate::menu::main_menu::MENU_CLIP3, x, y, c, false);
        }
        if self.rd(BG_RANDOM) == 1 {
            self.g.set_background(100);
        }
        let (label, lx): (Vec<u8>, i32) = if mode == 1 {
            let st = self.g.stage();
            if st != (st / 10) * 10 {
                self.g.set_stage((st / 10) * 10);
            }
            self.blit(crate::menu::main_menu::MENU_CLIP3, 0xf, 0x57, 0x17, false);
            if self.rd(MENU_CUR) == 3 {
                self.blit(crate::menu::main_menu::MENU_CLIP3, 0xf, 0x57, 0x16, false);
            }
            let st = self.g.stage();
            if st / 10 < 5 {
                (vec![b'1' + (st / 10) as u8], 0xc2)
            } else {
                (b"Survival".to_vec(), 0xae)
            }
        } else {
            (self.gd.bg_name(self.g.background()).to_vec(), 0xae)
        };
        self.text(&label, 0, 0xff9b9b, lx, 0x5b);
        let d: Option<&[u8]> = match self.g.difficulty() {
            2 => Some(b"Easy"),
            1 => Some(b"Normal"),
            0 => Some(b"Difficult"),
            -1 => Some(b"CRAZY!"),
            _ => None,
        };
        if let Some(d) = d {
            self.text(d, 0, 0xff9b9b, 0xae, 0x73);
        }
        let (mut up, mut down, mut right, mut left, mut attack) = (false, false, false, false, false);
        for s in 0..8usize {
            if self.g.m.u8(EXISTS + s as u32) == 0 {
                continue;
            }
            let first = self.arr(HELD, s) == 0 && prev_state == 3;
            match self.pick_key(s, &[0xcd, 0xce, 0xd0, 0xcf, 0xd1]) {
                Some(0xcd) => up |= first,
                Some(0xce) => down |= first,
                Some(0xd0) => right |= first,
                Some(0xcf) => left |= first,
                Some(_) => attack |= first,
                None => {
                    let j = (self.objs[s].jump() != 0) as i32;
                    self.set_arr(HELD, s, j);
                    continue;
                }
            }
            self.set_arr(HELD, s, 1);
        }
        if up {
            let mut c = self.rd(MENU_CUR) - 1;
            if c < 0 {
                c = 5;
            }
            self.wr(MENU_CUR, c);
        }
        if down {
            self.wr(MENU_CUR, (self.rd(MENU_CUR) + 1) % 6);
        }
        self.draw_music_menu(mode, left, right);
        let fight = self.g.m.i32(0x450b98) != 0 || (attack && self.rd(MENU_CUR) == 0);
        if fight {
            self.fight(mode);
        }
        if attack {
            match self.rd(MENU_CUR) {
                4 => {
                    let mut d = self.g.difficulty() - 1;
                    if (d < 0 && self.g.u_458428() == 0) || d < -1 {
                        d = 2;
                    }
                    self.wr(crate::post_update::DIFFICULTY, d);
                    self.buffer_sounds.push("data\\m_ok.wav");
                }
                5 => {
                    self.buffer_sounds.push("data\\m_ok.wav");
                    self.g.set_match_phase(10);
                    self.wr(crate::menu::main_menu::MOUSE_CLICK, 0);
                    self.end_match();
                }
                _ => {}
            }
        }
        if self.g.u_450c2c() == 1 {
            self.demo_setup();
        }
        if !attack {
            return;
        }
        let cur = self.rd(MENU_CUR);
        if cur == 1 {
            self.buffer_sounds.push("data\\m_ok.wav");
            for s in 0..8 {
                self.set_arr(SLOT_STATE, s, 0);
                self.set_arr(HELD, s, 1);
                if self.arr(WAS_RANDOM, s) == 1 {
                    self.set_arr(PICK, s, -1);
                }
            }
            self.wr(COUNTDOWN, 0x96);
            self.wr(PICK_STATE, 0);
        }
        if cur == 3 {
            if mode == 1 {
                self.buffer_sounds.push("data\\m_ok.wav");
                let mut st = self.g.stage() + 10;
                if st == 0x3c {
                    st = 0;
                }
                self.g.set_stage(st);
                return;
            }
            self.buffer_sounds.push("data\\m_ok.wav");
            self.wr(BG_RANDOM, 0);
            let bg = self.g.background();
            let n = self.gd.backgrounds.len() as i32;
            let next = match bg {
                100 => 99,
                99 => 0,
                b => b + 1,
            };
            self.g.set_background(next);
            if next == n {
                self.g.set_background(100);
                self.wr(BG_RANDOM, 1);
            }
        }
        if cur == 2 {
            self.buffer_sounds.push("data\\m_ok.wav");
            self.resolve_random_picks(0xea);
        }
    }

    /// Place a fighter for the match (`obj` = slot, or slot + 10 for a computer).
    fn place_fighter(&mut self, obj: usize, data: usize, team: i32, human: bool, mode: i32, bg: i32, sites: (i32, i32)) {
        let hp = self.gd.objects[data].weapon_hp;
        let (bw, (z0, z1)) = {
            let b = self.gd.bg(bg);
            (b.width, b.zboundary)
        };
        {
            let o = &mut self.objs[obj];
            crate::collisions::obj_reset(o);
            o.m.set_i32(0x368, data as i32);
            o.set_x(if human { 400.0 } else { 350.0 });
            o.set_y(if human { -50.0 } else { 0.0 });
            o.set_weapon_hp(hp);
            o.set_z(300.0);
            o.set_team(team);
            o.set_u_08(0x4b);
        }
        self.g.m.set_u8(EXISTS + obj as u32, 1);
        let x = self.random(sites.0, bw / 2) + bw / 4;
        let z = self.random(sites.1, z1 - z0) + z0;
        let o = &mut self.objs[obj];
        o.set_ix(x);
        o.set_iz(z);
        o.set_iy(0);
        o.set_x(x as f64);
        o.set_z(z as f64);
        o.set_y(0.0);
        o.set_mp(if mode == 1 { 500 } else { 200 });
        o.m.set_i32(0x354, obj as i32);
    }

    /// "Fight!" (0x42cf8a): start the match.
    fn fight(&mut self, mode: i32) {
        self.g.set_u_450bb0(0);
        self.g.set_u_450bb4(0);
        if mode == 1 {
            self.g.set_stage_go_timer(0x46);
            self.g.set_stage_state(0);
            self.g.set_u_450bac(0);
            self.g.set_stage_fade(0);
            self.wr(BG_RANDOM, 0);
            let bg = match self.g.stage() {
                0 => Some(1),
                10 => Some(2),
                20 => Some(3),
                30 => Some(4),
                40 => Some(5),
                50 => Some(6),
                _ => None,
            };
            if let Some(b) = bg {
                self.g.set_background(b);
            }
            self.g.set_stage_phase(-1);
            self.g.set_survival_blink(0);
            self.g.set_survival_phase(-1);
            self.wr(crate::camera::CAMERA_V, 0);
            self.wr(crate::camera::CAMERA_X, 0);
        }
        self.g.set_match_phase(0);
        // the file name is not compared (the time differs)
        let suffix = match mode {
            1 if self.g.stage() / 10 < 5 => format!("_Stage_{}", self.g.stage() / 10 + 1),
            1 => "_Survival".to_string(),
            0 => "_VS".to_string(),
            _ => String::new(),
        };
        self.recording_name(&suffix);
        self.wr(0x450b6c, 0);
        if self.rd(0x450be4) != 0 {
            self.wr(0x450b70, 1);
        }
        if self.g.m.i32(0x450b98) == 0 {
            self.buffer_sounds.push("data\\m_ok.wav");
        } else {
            self.fill(0, 0, 0x31a, 0x226, 0);
            self.wr(0x450b98, 0);
        }
        for i in 10..MAX_OBJECTS {
            self.g.m.set_u8(EXISTS + i as u32, 0);
        }
        let nbg = self.gd.backgrounds.len() as i32;
        if self.rd(BG_RANDOM) == 1 {
            let mut b = self.random(0xda, nbg - 2);
            if b == nbg - 3 {
                b = 99;
            }
            self.g.set_background(b);
        }
        let bg = self.g.background();
        for s in 0..8usize {
            let st = self.arr(SLOT_STATE, s);
            if st < 0xb {
                if 0 < st {
                    let team = self.objs[s].team();
                    let data = self.objs[s].m.i32(0x368) as usize;
                    self.place_fighter(s, data, team, true, mode, bg, (0xdd, 0xde));
                }
            } else {
                let team = self.objs[s].team();
                let data = self.objs[s].m.i32(0x368) as usize;
                self.place_fighter(s + 10, data, team, false, mode, bg, (0xdb, 0xdc));
            }
        }
        self.match_start_common(mode);
    }

    /// The end of "Fight!" shared with the demo set-up: independent teams, clear the
    /// per-match counters, start the recording.
    fn match_start_common(&mut self, mode: i32) {
        for j in 0..MAX_OBJECTS {
            if self.g.m.u8(EXISTS + j as u32) != 0 && self.objs[j].team() == 0 {
                self.objs[j].set_team(j as i32 + 10);
            }
        }
        for a in (0x450c04..=0x450c28).step_by(4) {
            self.wr(a, 0);
        }
        self.g.set_mp_cost_on(1);
        self.wr(0x450bbc, 0);
        self.g.set_end_timer(0);
        if mode == 1 {
            for j in 0..MAX_OBJECTS {
                if self.g.m.u8(EXISTS + j as u32) != 0 && self.data_of(j).kind == 0 {
                    let r = self.random(0xdf, 0x1e);
                    self.objs[j].set_ix(r + 0x32);
                    self.objs[j].set_x((r + 0x32) as f64);
                }
            }
        }
        if mode == 0 {
            // play_music_menu (0x4025b0)
            let f = cstr(self, MUSIC_FILE, 60);
            if !f.is_empty() {
                self.play_music(&f);
            }
        }
        for j in 20..MAX_OBJECTS {
            if self.g.m.u8(EXISTS + j as u32) == 0 {
                crate::collisions::obj_reset(&mut self.objs[j]);
            }
        }
        self.end_match();
        self.recording_start();
    }

    /// `recording_start` (0x43d2c0): the recording itself is kept on the heap; these
    /// are the globals it resets.
    pub(crate) fn recording_start(&mut self) {
        self.recording = None;
        self.g.set_match_frames(0);
        self.wr(0x450b80, 1);
        // the recording buffer (heap); a marker for "allocated"
        self.wr(0x4588a8, 1);
        self.recording_header();
        self.g.set_rng_c34(0);
        self.g.set_tick_mod12(0);
        self.g.set_tick_mod3(0);
        self.g.set_tick_parity(0);
    }

    /// The demo set-up (0x42d7a0, 0x450c2c == 1): eight random computer players.
    fn demo_setup(&mut self) {
        // music_for_stage (0x4025d0) gets whatever ECX holds after draw_music_menu,
        // not a music number, so it plays the chosen file again without random()
        if self.rd(MUSIC_ON) != 0 {
            let f = cstr(self, MUSIC_FILE, 60);
            if !f.is_empty() {
                self.play_music(&f);
            }
        }
        self.g.set_match_phase(0);
        for i in 0..MAX_OBJECTS as u32 {
            self.g.m.set_u8(EXISTS + i, 0);
        }
        self.wr(BG_RANDOM, 1);
        let nbg = self.gd.backgrounds.len() as i32;
        let mut b = self.random(0xe0, nbg - 2);
        if b == nbg - 3 {
            b = 99;
        }
        self.g.set_background(b);
        for s in 0..8 {
            let picks: Vec<i32> = (0..8).map(|j| self.arr(PICK, j)).collect();
            let list: Vec<i32> = (1..self.gd.objects.len() as i32)
                .filter(|&k| {
                    let d = &self.gd.objects[k as usize];
                    d.kind == 0 && !picks.contains(&k) && d.id < 0x1e
                })
                .collect();
            let r = self.random(0xe1, list.len() as i32);
            let k = list.get(r as usize).copied().unwrap_or(0);
            self.set_arr(PICK, s, k);
            self.objs[s].m.set_i32(0x368, k);
        }
        self.g.set_u_450c2c(1);
        let (bw, (z0, z1)) = {
            let bg = self.gd.bg(b);
            (bg.width, bg.zboundary)
        };
        for k in 0..8usize {
            let data = self.objs[k].m.i32(0x368);
            let whp = self.gd.objects[data as usize].weapon_hp;
            {
                let o = &mut self.objs[10 + k];
                crate::collisions::obj_reset(o);
                o.m.set_i32(0x368, data);
                o.set_x(350.0);
                o.set_weapon_hp(whp);
                o.set_y(0.0);
                o.set_z(300.0);
                o.set_u_08(0x4b);
            }
            let x = self.random(0xe2, bw / 2) + bw / 4;
            let z = self.random(0xe3, z1 - z0) + z0;
            let o = &mut self.objs[10 + k];
            o.set_ix(x);
            o.set_iz(z);
            o.set_x(x as f64);
            o.set_z(z as f64);
            o.set_mp(200);
            o.m.set_i32(0x354, k as i32 + 10);
        }
        let ex = |w: &mut World, k: u32, v: u8| w.g.m.set_u8(EXISTS + 10 + k, v);
        let r = self.random(0xe4, 0x1e);
        if r < 7 {
            ex(self, 0, 1);
            self.objs[10].set_team(0);
            for k in 1..8 {
                let t = self.random(0xe5, 5);
                self.objs[10 + k].set_team(t);
                ex(self, k as u32, 1);
            }
        } else if r < 0x12 {
            for k in 0..8 {
                self.objs[10 + k].set_team(if k < 4 { 1 } else { 2 });
                ex(self, k as u32, 1);
            }
            if self.random(0xe6, 3) == 0 {
                ex(self, 7, 0);
                ex(self, 3, 0);
            }
            if self.random(0xe7, 6) == 0 {
                self.objs[17].set_team(3);
                self.objs[13].set_team(3);
            }
        } else {
            let size = if r < 0x19 {
                for k in 0..8 {
                    self.objs[10 + k].set_team(k as i32 / 2 + 1);
                }
                self.random(0xe8, 3) * 2 + 4
            } else {
                for k in 0..4 {
                    self.objs[10 + k].set_team(0);
                }
                self.random(0xe9, 3) + 2
            };
            for k in 0..size.max(0) as u32 {
                ex(self, k, 1);
            }
        }
        for j in 0..MAX_OBJECTS {
            if self.g.m.u8(EXISTS + j as u32) != 0 && self.objs[j].team() == 0 {
                self.objs[j].set_team(j as i32 + 10);
            }
        }
        for a in (0x450c04..=0x450c28).step_by(4) {
            self.wr(a, 0);
        }
        self.g.set_mp_cost_on(1);
        self.wr(0x450bbc, 0);
        self.fill(0, 0, 0x31a, 0x226, 0);
        self.g.set_end_timer(0);
    }

}

impl World {
    /// `stage_aux` (0x437220, match_phase 300): the ending pictures after stage 5-10,
    /// one per attack press, then the main menu.
    pub(crate) fn ending_screen(&mut self) {
        const BLINK2: u32 = 0x451b2c;
        const PAGE: u32 = 0x451b28;
        const FADE_IN: u32 = 0x451b24;
        const FADE_OUT: u32 = 0x451b20;
        self.wr(BLINK2, (self.rd(BLINK2) + 1) % 10);
        let d = self.g.difficulty();
        if d == 1 || d == 2 {
            if self.rd(PAGE) == 1 {
                self.wr(PAGE, 2);
            }
        } else if self.rd(PAGE) == 0 {
            self.wr(PAGE, 1);
        }
        let ending = super::sprites::SHEETS.iter().find(|s| s.resource == "ENDING").unwrap();
        let page = self.rd(PAGE);
        let (w, h) = ending.clips.get(page as usize).map_or((0, 0), |c| (c.2, c.3));
        let x = (800 - w) / 2;
        let mut y = (0x208 - h) / 2;
        let bottom = h + y;
        self.blit(ending.global, x, y, page, false);
        if self.rd(FADE_IN) < 0xd {
            let f = self.rd(FADE_IN) + 1;
            self.wr(FADE_IN, f);
            for _ in 0..0xd {
                self.fill(0, y, 0x31a, 0xd - f, 0);
                y += 0xd;
            }
        } else if self.rd(FADE_OUT) == 0 {
            if (0..8).any(|i| self.objs[i].attack() != 0 && self.objs[i].prev_attack() == 0) {
                self.wr(FADE_OUT, 1);
            }
        } else if 0 < self.rd(FADE_OUT) {
            let f = self.rd(FADE_OUT);
            for _ in 0..0xd {
                self.fill(0, y, 0x31a, f, 0);
                y += 0xd;
            }
            self.wr(FADE_OUT, f + 1);
            if 0xc < f + 1 {
                self.wr(PAGE, self.rd(PAGE) + 1);
                self.wr(FADE_IN, 0);
                self.wr(FADE_OUT, 0);
            }
        }
        if self.rd(BLINK2) < 5 && self.rd(FADE_IN) == 0xd && self.rd(FADE_OUT) == 0 {
            self.blit(ending.global, w + x, bottom, 5, false);
        }
        if self.rd(PAGE) == 5 {
            self.g.set_match_phase(10);
            self.wr(PAGE, 0);
            self.wr(FADE_IN, 0);
            self.end_match();
            self.wr(crate::menu::main_menu::MOUSE_CLICK, 0);
        }
    }
}
