//! The battle mode set-up screens: `battle_menus` (0x438b40), match_phase 200..219.
//!
//! 200 the army screen (defense, troops in screen and in reserve, army type, OK),
//! 201 resolve random fighters, 202 the menu (Fight!, ...), 210..219 choose an army
//! type and size for one side.
//!
//! Per side t (0 team 1, 1 team 2) and kind k (n = `DAT_0044d37c` kinds in
//! `DAT_0044d350`): troops in screen `IN[t*n+k]`, in reserve `RES[t*n+k]`.

use crate::menu::main_menu::{
    KEY_ATTACK, KEY_DEFEND, KEY_DOWN, KEY_JUMP, KEY_LEFT, KEY_RIGHT, KEY_UP, MENU_CLIP, MOUSE_CLICK,
};
use crate::menu::pick::{BG_RANDOM, PICK, PICK_STATE, SLOT_STATE, WAS_RANDOM};
use crate::world::{World, EXISTS, MAX_OBJECTS};

const N: u32 = 0x44d37c;
const KINDS: u32 = 0x44d350;
const ARMY: u32 = 0x44d380;
const PRESET_TOTAL: u32 = 0x44d388;
const PRESET_IN: u32 = 0x44d468;
const BACKUP_RES: u32 = 0x44d548;
const BACKUP_IN: u32 = 0x44d5a0;
const IN: u32 = 0x44d5f8;
const RES: u32 = 0x44d650;
const RESERVE: u32 = 0x44d6a8;
const MAX_ALIVE: u32 = 0x44d700;
const DEFENSE: u32 = 0x44d758;
const ARMY_CUR: u32 = 0x44d760;
const X0: u32 = 0x44d764;
const Y0: u32 = 0x44d768;
const COL: u32 = 0x44d76c;
const ROW: u32 = 0x44d770;
const INIT: u32 = 0x44d774;
const SIZE: u32 = 0x451b74;
const SIDE: u32 = 0x451ba8;
const MENU: u32 = 0x451b84;
const KIND_DATA: u32 = 0x451b38;
const TYPE_SEL: u32 = 0x451b90;
const SIZE_SEL: u32 = 0x451b98;
const SAVED_TYPE: u32 = 0x451b88;
const SAVED_SIZE: u32 = 0x451b8c;
const COLS: u32 = 0x451b9c;
const FRAME_BLINK: u32 = 0x451b80;
pub const BATTLEMODE: u32 = 0x451bb0;
pub const BATTLETROOPS: u32 = 0x451bac;

impl World {
    fn b(&self, a: u32) -> i32 {
        self.g.m.i32(a)
    }

    fn bw(&mut self, a: u32, v: i32) {
        self.g.m.set_i32(a, v)
    }

    fn bi(&self, a: u32, i: i32) -> i32 {
        self.g.m.i32(a.wrapping_add((4 * i) as u32))
    }

    fn bwi(&mut self, a: u32, i: i32, v: i32) {
        self.g.m.set_i32(a.wrapping_add((4 * i) as u32), v)
    }

    /// `battle_aux` (0x4389a0): a blinking selection frame.
    fn battle_frame(&mut self, x: i32, y: i32, w: i32, h: i32, rgb: u32) {
        let t = (self.b(FRAME_BLINK) + 1) % 4;
        self.bw(FRAME_BLINK, t);
        if t < 2 {
            self.battle_frame2(x, y, w, h, rgb);
            return;
        }
        self.fill(x, y - 1, w, 1, rgb);
        self.fill(x, y + h, w, 1, rgb);
        self.fill(x - 1, y, 1, h, rgb);
        self.fill(x + w, y, 1, h, rgb);
        self.fill(x, y, w, 1, rgb);
        self.fill(x, y + h - 1, w, 1, rgb);
        self.fill(x, y, 1, h, rgb);
        self.fill(x + w - 1, y, 1, h, rgb);
    }

    /// `battle_aux2` (0x438ad0).
    fn battle_frame2(&mut self, x: i32, y: i32, w: i32, h: i32, rgb: u32) {
        self.fill(x + 1, y, w - 2, 1, rgb);
        self.fill(x + 1, y + h - 1, w - 2, 1, rgb);
        self.fill(x, y + 1, 1, h - 2, rgb);
        self.fill(x + w - 1, y + 1, 1, h - 2, rgb);
    }

    /// Fill the troops of side `t` from the preset of army type `ty` at size `sz` (1..3).
    fn battle_preset(&mut self, t: i32, ty: i32, sz: i32) {
        let n = self.b(N);
        for k in 0..n {
            let total = (self.bi(PRESET_TOTAL, ty * n + k) * sz) / 3;
            let mut inn = (self.bi(PRESET_IN, ty * n + k) * sz) / 3;
            if inn < 1 && 0 < total {
                inn = 1;
            }
            self.bwi(IN, t * n + k, inn);
            self.bwi(RES, t * n + k, (total - inn).max(0));
        }
    }

    fn battle_backup(&mut self, save: bool) {
        let n = 2 * self.b(N);
        for i in 0..n.max(0) {
            if save {
                let (r, v) = (self.bi(RES, i), self.bi(IN, i));
                self.bwi(BACKUP_IN, i, r);
                self.bwi(BACKUP_RES, i, v);
            } else {
                let (r, v) = (self.bi(BACKUP_IN, i), self.bi(BACKUP_RES, i));
                self.bwi(RES, i, r);
                self.bwi(IN, i, v);
            }
        }
    }

    /// 0x438b40.
    pub(crate) fn battle_menus(&mut self) {
        self.menu_keys();
        let n = self.b(N);
        if self.b(INIT) == 1 {
            let mut t = 0;
            while t <= n {
                for k in 0..n {
                    let total = self.bi(PRESET_TOTAL, k) / 3;
                    let mut inn = self.bi(PRESET_IN, k) / 3;
                    if inn < 1 && 0 < total {
                        inn = 1;
                    }
                    self.bwi(IN, t + k, inn);
                    self.bwi(RES, t + k, (total - inn).max(0));
                }
                if n <= 0 {
                    break;
                }
                t += n;
            }
            // the two surfaces; markers for "loaded"
            self.bw(BATTLEMODE, 1);
            self.bw(BATTLETROOPS, 1);
            for k in 0..n {
                let id = self.bi(KINDS, k);
                if let Some(d) = self.data_index(id) {
                    // a data pointer in the original; a non-zero marker here
                    self.bwi(KIND_DATA, k, d as i32 + 1);
                }
            }
        }
        if self.b(INIT) == 1 || self.b(INIT) == -1 {
            self.bw(ROW, 6);
            self.bw(COL, 2);
            self.bw(SIDE, 0);
            self.bw(INIT, 0);
        }
        for a in [0x451b68, 0x451b64, 0x451b70, 0x451b6c] {
            self.bw(a, 0);
        }
        let phase = self.g.match_phase();
        let x0 = self.b(X0);
        let y0 = if (0xc9..0xd2).contains(&phase) {
            0x50
        } else if (0xd2..=0xdb).contains(&phase) {
            0x3c
        } else {
            0x21
        };
        self.bw(Y0, y0);
        if y0 == 0x21 {
            self.blit(BATTLEMODE, x0, 0x21, -1, false);
        } else {
            self.blit(BATTLEMODE, x0, y0, 0, false);
            self.blit(BATTLEMODE, x0, y0 + 400, 1, false);
        }
        self.battle_overview(phase, x0, y0, n);
        if self.g.match_phase() == 200 {
            self.battle_army_screen(x0, y0, n);
        }
        let p = self.g.match_phase();
        if p == 0xc9 {
            for s in 0..8 {
                if self.bi(WAS_RANDOM, s) != 1 {
                    continue;
                }
                let picks: Vec<i32> = (0..8).map(|j| self.bi(PICK, j)).collect();
                let list: Vec<i32> = (1..self.gd.objects.len() as i32)
                    .filter(|&k| {
                        let d = &self.gd.objects[k as usize];
                        d.kind == 0 && !picks.contains(&k) && d.id < 0x1e
                    })
                    .collect();
                let r = self.random(0x122, list.len() as i32);
                let k = list.get(r as usize).copied().unwrap_or(0);
                self.bwi(PICK, s, k);
                self.objs[s as usize].m.set_i32(0x368, k);
            }
            self.g.set_match_phase(0xca);
            self.bw(MENU, 2);
            self.battle_menu();
        } else {
            if (0xd2..=0xdb).contains(&p) {
                self.battle_army_type(n);
            } else if self.g.match_phase() == 0xca {
                self.battle_menu();
            }
        }
        if self.b(KEY_ATTACK) != 0 || self.b(KEY_DEFEND) != 0 {
            self.buffer_sounds.push("data\\m_join.wav");
        }
        if self.b(KEY_JUMP) != 0 {
            self.buffer_sounds.push("data\\m_cancel.wav");
        }
    }

    /// The fighters, defenses and troop counts drawn on every battle screen.
    fn battle_overview(&mut self, phase: i32, x0: i32, y0: i32, n: i32) {
        self.bwi(COLS, 1, x0 + 0xb3);
        self.bwi(COLS, 2, x0 + 0x20e);
        for s in 0..8 {
            let st = self.bi(SLOT_STATE, s);
            let t = self.objs[s as usize].team();
            if (st == 3 || st == 0xd) && 0 < t && t < 3 {
                self.bwi(COLS, t, self.bi(COLS, t) - 0x14);
            }
        }
        let mut x = 0;
        for s in 0..8 {
            let st = self.bi(SLOT_STATE, s);
            if st != 3 && st != 0xd {
                continue;
            }
            let t = self.objs[s as usize].team();
            if 0 < t && t < 3 {
                x = self.bi(COLS, t);
                self.bwi(COLS, t, x + 0x28);
            }
            if self.bi(WAS_RANDOM, s) == 0 || (200 < phase && phase < 0xd2) {
                let pick = self.bi(PICK, s);
                self.blit_sheet(crate::draw::Sheet::Small(pick.max(0) as usize), x, y0 + 0x56, -1, false);
            } else {
                self.blit(crate::menu::champ1::CLIP4, x, y0 + 0x56, 1, false);
            }
            let c = if st == 3 { b'1' + s as u8 } else { b'C' };
            let color = if t == 1 { 0xff9b4f } else { 0x4f4fff };
            self.text(&[c], 0x8f472f, color, x + 0xf, y0 + 0x83);
        }
        for t in 0..2 {
            let d = self.bi(DEFENSE, t);
            let s = format!("x {}.{}", d / 100, (d % 100) / 10);
            self.text(s.as_bytes(), 0, 0xffffff, t * 0x15b + 0xf0 + x0, y0 + 0x9a);
        }
        self.bwi(COLS, 1, x0 + 0x27);
        self.bwi(COLS, 2, x0 + 0x182);
        for t in 0..2 {
            let base = self.bi(COLS, t + 1);
            for k in 0..n {
                let (x, y) = if k < 6 { (base + k * 0x30, y0 + 0xd7) } else { (base - 0x108 + k * 0x30, y0 + 0x132) };
                if k == n - 1 {
                    self.blit(crate::menu::champ1::CLIP4, x, y, 0x16, false);
                } else if k == n - 2 {
                    self.blit(crate::menu::champ1::CLIP4, x, y, 0x17, false);
                } else {
                    let d = self.bi(KIND_DATA, k) - 1;
                    self.blit_sheet(crate::draw::Sheet::Small(d.max(0) as usize), x, y, -1, false);
                }
                self.fill(x + 3, y + 0x2e, 0x22, 0x10, 0);
                self.fill(x + 3, y + 0x40, 0x22, 0x10, 0);
                let inn = self.bi(IN, t * n + k);
                if inn == 0 {
                    self.text(b"0", 0, 0x9a4d32, x + 10, y + 0x2e);
                    self.text(b"--", 0, 0x9a4d32, x + 10, y + 0x40);
                } else {
                    self.text(inn.to_string().as_bytes(), 0, 0xffffff, x + 10, y + 0x2e);
                    let r = self.bi(RES, t * n + k);
                    let c = if r == 0 { 0x9a4d32 } else { 0xffffff };
                    self.text(r.to_string().as_bytes(), 0, c, x + 10, y + 0x40);
                }
            }
            let a = self.bi(ARMY, t);
            if a != -1 {
                let w = |c: i32| {
                    crate::menu::sprites::SHEETS
                        .iter()
                        .find(|s| s.global == MENU_CLIP)
                        .and_then(|s| s.clips.get(c as usize))
                        .map_or(0, |c| c.2)
                };
                if a == 0 || a == 6 {
                    let x = (t * 0x15b - w(a + 0x12)) + 0x159 + x0;
                    self.blit(MENU_CLIP, x, y0 + 0xc0, a + 0x12, true);
                } else {
                    let sz = self.bi(SIZE, t);
                    let w2 = w(sz + 0xf);
                    self.blit(MENU_CLIP, (t * 0x15b - w(a + 0x12) - w2) + 0x159 + x0, y0 + 0xc0, a + 0x12, true);
                    self.blit(MENU_CLIP, x0 + 0x159 + t * 0x15b - w2, y0 + 0xc0, sz + 0xf, true);
                }
            }
        }
    }

    /// Phase 200: the army screen.
    fn battle_army_screen(&mut self, x0: i32, y0: i32, n: i32) {
        if self.b(KEY_UP) != 0 {
            self.bw(ROW, self.b(ROW) - 1);
        } else if self.b(KEY_DOWN) != 0 {
            self.bw(ROW, self.b(ROW) + 1);
        }
        let mut row = self.b(ROW);
        if 7 <= row {
            row = 0;
            self.bw(ROW, 0);
        } else if row < 0 {
            row = 6;
            self.bw(ROW, 6);
        }
        let lr = self.b(KEY_LEFT) != 0 || self.b(KEY_RIGHT) != 0;
        if row == 0 {
            let side = self.b(SIDE);
            self.battle_frame(side * 0x15b + 0x50 + x0, y0 + 0x96, 0xd0, 0x19, 0xffffff);
            if lr {
                self.bw(SIDE, 1 - side);
            }
            let s = self.b(SIDE);
            if self.b(KEY_ATTACK) != 0 {
                let mut d = self.bi(DEFENSE, s) + 0x32;
                if 300 < d {
                    d = 100;
                }
                self.bwi(DEFENSE, s, d);
            }
            if self.b(KEY_JUMP) != 0 {
                let mut d = self.bi(DEFENSE, s) - 0x32;
                if d < 100 {
                    d = 300;
                }
                self.bwi(DEFENSE, s, d);
            }
        }
        let row = self.b(ROW);
        if 0 < row && row < 5 {
            let maxk = (row < 3) as i32 + 4;
            let mut side = self.b(SIDE);
            if self.b(KEY_RIGHT) != 0 {
                let c = self.b(COL) + 1;
                self.bw(COL, c);
                if maxk < c {
                    side = 1 - side;
                    self.bw(COL, 0);
                    self.bw(SIDE, side);
                }
            }
            if self.b(KEY_LEFT) != 0 {
                let c = self.b(COL).min(maxk) - 1;
                self.bw(COL, c);
                if c < 0 {
                    side = 1 - side;
                    self.bw(COL, maxk);
                    self.bw(SIDE, side);
                }
            }
            let col = self.b(COL);
            let (a, j, d) = (self.b(KEY_ATTACK) != 0, self.b(KEY_JUMP) != 0, self.b(KEY_DEFEND) != 0);
            if a || j || d {
                self.bwi(ARMY, side, -1);
                self.bwi(SIZE, side, -1);
                let (addr, max) = match row {
                    1 => (IN + 4 * (col + side * n) as u32, 10),
                    2 => (RES + 4 * (col + side * n) as u32, 30),
                    3 if col < 4 => (0x44d610 + 4 * (col + side * n) as u32, 10),
                    3 => (0x44d620 + 4 * (side * n) as u32, 10),
                    4 if col < 4 => (0x44d668 + 4 * (col + side * n) as u32, 30),
                    _ => (0x44d678 + 4 * (side * n) as u32, 30),
                };
                let mut v = self.b(addr);
                if a {
                    v += 1;
                }
                if d {
                    v += 5;
                    if max < v && v < max + 5 {
                        v = max;
                    }
                }
                if j {
                    v -= 1;
                }
                if v < 0 {
                    v = max;
                } else if max < v {
                    v = 0;
                }
                self.bw(addr, v);
            }
            let c = col.min(maxk);
            let (x, y) = if row < 3 {
                (c * 0x30 + 0x2a, y0 + 0xf3 + row * 0x12)
            } else {
                (c * 0x30 + 0x42, y0 + 0x12a + row * 0x12)
            };
            let side = self.b(SIDE);
            self.battle_frame(x + side * 0x15b + x0, y, 0x22, 0x10, 0xffffff);
        }
        if self.b(ROW) == 5 {
            let side = self.b(SIDE);
            self.battle_frame(side * 0x15b + 0x2b + x0, y0 + 0x18c, 0x110, 0x19, 0xffffff);
            if lr {
                self.bw(SIDE, 1 - side);
            }
            if self.b(KEY_ATTACK) != 0 {
                let s = self.b(SIDE);
                self.g.set_match_phase(0xd2);
                self.bw(ARMY_CUR, 10);
                self.bw(KEY_ATTACK, 0);
                self.battle_backup(true);
                self.bw(SAVED_SIZE, self.bi(SIZE_SEL, s));
                self.bw(SAVED_TYPE, self.bi(TYPE_SEL, s));
            }
        }
        if self.b(ROW) != 6 {
            return;
        }
        self.battle_frame(x0 + 0x12d, y0 + 0x1b9, 0x6a, 0x18, 0xffffff);
        if self.b(KEY_ATTACK) != 0 {
            self.g.set_match_phase(0xca);
            self.bw(MENU, 2);
            self.bw(KEY_ATTACK, 0);
            self.buffer_sounds.push("data\\m_join.wav");
        }
        if self.b(KEY_LEFT) != 0 {
            self.bw(ROW, self.b(ROW) - 1);
            self.bw(SIDE, 0);
        } else if self.b(KEY_RIGHT) != 0 {
            self.bw(ROW, self.b(ROW) - 1);
            self.bw(SIDE, 1);
        }
        if self.b(KEY_JUMP) != 0 {
            self.g.set_match_phase(3);
            self.bw(PICK_STATE, 0);
        }
    }

    /// Phases 210..219: choose the army type and size of one side.
    fn battle_army_type(&mut self, n: i32) {
        let side = self.b(SIDE);
        let x = side * -0x113;
        self.blit(BATTLETROOPS, x + 0x19a, 0x41, -1, false);
        if self.b(KEY_JUMP) != 0 {
            self.battle_backup(false);
            self.bwi(SIZE_SEL, side, self.b(SAVED_SIZE));
            self.bwi(TYPE_SEL, side, self.b(SAVED_TYPE));
            self.g.set_match_phase(200);
        }
        let v = self.bi(SIZE_SEL, side);
        if v != -1 {
            self.battle_frame2(x + 0x1c8, v * 0x16 + 0xa8, 0xa1, 0x13, 0xa7a7ff);
        }
        let v = self.bi(TYPE_SEL, side);
        if v != -1 {
            self.battle_frame2(x + 0x1c8, v * 0x16 + 0x130, 0xa1, 0x13, 0xa7a7ff);
        }
        let r = self.b(ARMY_CUR);
        if r < 10 {
            if r < 5 {
                self.battle_frame(x + 0x1c6, r * 0x16 + 0x90, 0xa5, 0x17, 0xffffff);
                if self.b(KEY_ATTACK) != 0 {
                    if ((r - 1) as u32) < 3 {
                        self.bwi(SIZE_SEL, side, r - 1);
                        if self.bi(TYPE_SEL, side) == -1 {
                            self.bwi(TYPE_SEL, side, 0);
                        }
                        let ty = self.bi(TYPE_SEL, side);
                        self.battle_preset(side, ty, r);
                        self.bwi(SIZE, side, r - 1);
                        self.bwi(ARMY, side, ty + 1);
                    }
                    if r == 0 {
                        self.bwi(SIZE_SEL, side, -1);
                        self.bwi(TYPE_SEL, side, -1);
                        for k in 0..n {
                            self.bwi(RES, side * n + k, 0);
                            self.bwi(IN, side * n + k, 0);
                        }
                        self.bwi(ARMY, side, 0);
                    } else if r == 4 {
                        self.bwi(SIZE_SEL, side, -1);
                        self.bwi(TYPE_SEL, side, -1);
                        for k in 0..n {
                            self.bwi(RES, side * n + k, 0x1e);
                            self.bwi(IN, side * n + k, 2);
                        }
                        let b = side * n;
                        self.bwi(IN, b + 7, 1);
                        self.bwi(RES, b + 7, 0xf);
                        self.bwi(IN, b + 9, 3);
                        self.bwi(IN, b + 10, 3);
                        self.bwi(ARMY, side, 6);
                    }
                }
            } else {
                self.battle_frame(x + 0x1c6, r * 0x16 + 0xc0, 0xa5, 0x17, 0xffffff);
                if self.b(KEY_ATTACK) != 0 {
                    if self.bi(SIZE_SEL, side) == -1 {
                        self.bwi(SIZE_SEL, side, 0);
                    }
                    self.bwi(ARMY, side, r - 4);
                    let sz = self.bi(SIZE_SEL, side);
                    self.bwi(SIZE, side, sz);
                    self.bwi(TYPE_SEL, side, r - 5);
                    self.battle_preset(side, r - 5, sz + 1);
                }
            }
            if self.b(KEY_UP) != 0 {
                self.bw(ARMY_CUR, self.b(ARMY_CUR) - 1);
            }
            if self.b(KEY_DOWN) != 0 {
                self.bw(ARMY_CUR, self.b(ARMY_CUR) + 1);
            }
            if self.b(ARMY_CUR) < 0 {
                self.bw(ARMY_CUR, 10);
            }
            return;
        }
        if r == 10 {
            self.battle_frame(x + 0x1b0, 0x1b1, 0x53, 0x18, 0xffffff);
            if self.b(KEY_RIGHT) != 0 {
                self.bw(ARMY_CUR, self.b(ARMY_CUR) + 1);
            }
            if self.b(KEY_UP) != 0 {
                self.bw(ARMY_CUR, self.b(ARMY_CUR) - 1);
            }
            if self.b(KEY_DOWN) != 0 {
                self.bw(ARMY_CUR, 0);
            }
            if self.b(KEY_ATTACK) != 0 {
                self.g.set_match_phase(200);
            }
            return;
        }
        if r == 0xb {
            self.battle_frame(x + 0x214, 0x1b1, 0x6a, 0x18, 0xffffff);
            if self.b(KEY_UP) != 0 {
                self.bw(ARMY_CUR, self.b(ARMY_CUR) - 2);
            }
            if self.b(KEY_DOWN) != 0 {
                self.bw(ARMY_CUR, 0);
            }
            if self.b(KEY_LEFT) != 0 {
                self.bw(ARMY_CUR, self.b(ARMY_CUR) - 1);
            }
            if self.b(KEY_ATTACK) != 0 {
                self.battle_backup(false);
                self.g.set_match_phase(200);
                self.bw(KEY_ATTACK, 0);
                self.buffer_sounds.push("data\\m_cancel.wav");
            }
        }
    }

    /// Phase 202 (LAB_00439f93): the menu.
    fn battle_menu(&mut self) {
        use crate::menu::main_menu::MENU_CLIP3;
        let mode = self.g.game_mode();
        let (l, r) = (self.b(KEY_LEFT) != 0, self.b(KEY_RIGHT) != 0);
        self.draw_music_menu(mode, l, r);
        self.blit(MENU_CLIP3, 3, 3, 8, false);
        self.blit(MENU_CLIP3, 3, 0x9f, 0xf, true);
        let hl = [(0x5c, 0x10, 9), (0x40, 0x27, 10), (0x28, 0x40, 0xb), (0xf, 0x57, 0xc), (0x25, 0x6f, 0xd), (0x65, 0x89, 0xe)];
        let m = self.b(MENU);
        if (0..6).contains(&m) {
            let (x, y, c) = hl[m as usize];
            self.blit(MENU_CLIP3, x, y, c, false);
        }
        if self.b(BG_RANDOM) == 1 {
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
        if self.b(KEY_UP) != 0 {
            let v = self.b(MENU) - 1;
            self.bw(MENU, if v == -1 { 5 } else { v });
        }
        if self.b(KEY_DOWN) != 0 {
            self.bw(MENU, (self.b(MENU) + 1) % 6);
        }
        if self.b(KEY_JUMP) != 0 {
            self.g.set_match_phase(200);
            return;
        }
        if self.b(KEY_ATTACK) == 0 {
            return;
        }
        let n2 = 2 * self.b(N);
        for i in 0..n2.max(0) {
            let inn = self.bi(IN, i);
            let res = if inn < 1 { 0 } else { self.bi(RES, i) + inn };
            self.bwi(RESERVE, i, res);
            self.bwi(MAX_ALIVE, i, inn);
        }
        self.bw(KEY_ATTACK, 0);
        self.buffer_sounds.push("data\\m_ok.wav");
        match self.b(MENU) {
            0 => self.battle_fight(mode),
            1 => {
                self.bw(PICK_STATE, 0);
                self.g.set_match_phase(3);
                self.g.set_background(100);
                self.bw(BG_RANDOM, 1);
            }
            2 => self.g.set_match_phase(0xc9),
            3 => {
                self.buffer_sounds.push("data\\m_ok.wav");
                self.bw(BG_RANDOM, 0);
                let cnt = self.gd.backgrounds.len() as i32;
                let next = match self.g.background() {
                    100 => 99,
                    99 => 0,
                    b => b + 1,
                };
                self.g.set_background(next);
                if next == cnt {
                    self.g.set_background(100);
                    self.bw(BG_RANDOM, 1);
                }
            }
            4 => {
                let mut d = self.g.difficulty() - 1;
                if d < 0 {
                    d = 2;
                }
                self.bw(crate::post_update::DIFFICULTY, d);
                self.buffer_sounds.push("data\\m_ok.wav");
            }
            5 => {
                self.g.set_match_phase(10);
                self.end_match();
                self.bw(MOUSE_CLICK, 0);
            }
            _ => {}
        }
    }

    /// Battle mode "Fight!" (0x43a2xx).
    fn battle_fight(&mut self, mode: i32) {
        self.g.set_u_450bb0(0);
        self.g.set_u_450bb4(0);
        self.g.set_match_phase(0);
        self.recording_name("_Battle");
        self.bw(0x450b6c, 0);
        if self.b(0x450be4) != 0 {
            self.bw(0x450b70, 1);
        }
        for i in 0..MAX_OBJECTS as u32 {
            self.g.m.set_u8(EXISTS + i, 0);
        }
        let cnt = self.gd.backgrounds.len() as i32;
        if self.b(BG_RANDOM) == 1 {
            let mut b = self.random(0x123, cnt - 2);
            if b == cnt - 3 {
                b = 99;
            }
            self.g.set_background(b);
        }
        for a in (0x450c04..=0x450c28).step_by(4) {
            self.bw(a, 0);
        }
        let bg = self.g.background();
        let (bw, (z0, z1)) = {
            let b = self.gd.bg(bg);
            (b.width, b.zboundary)
        };
        for s in 0..8usize {
            let st = self.bi(SLOT_STATE, s as i32);
            let human = st < 0xb;
            if human && st < 1 {
                continue;
            }
            let obj = if human { s } else { s + 10 };
            let team = self.objs[s].team();
            let data = self.objs[s].m.i32(0x368);
            let whp = self.gd.objects[data as usize].weapon_hp;
            {
                let o = &mut self.objs[obj];
                crate::collisions::obj_reset(o);
                o.m.set_i32(0x368, data);
                o.set_x(if human { 400.0 } else { 350.0 });
                o.set_weapon_hp(whp);
                o.set_y(if human { -50.0 } else { 0.0 });
                o.set_z(300.0);
                o.set_team(team);
                o.m.set_i32(0x344, team);
                o.set_u_08(0x4b);
                o.set_ix(if team == 1 { 100 } else { bw - 100 });
            }
            self.g.m.set_u8(EXISTS + obj as u32, 1);
            let r = self.random(if human { 0x127 } else { 0x125 }, z1 - z0);
            let def = self.bi(0x44d754, team);
            let o = &mut self.objs[obj];
            o.set_iz(r + z0);
            if human {
                o.set_iy(0);
            }
            o.set_x(o.ix() as f64);
            o.set_z(o.iz() as f64);
            if human {
                o.set_y(0.0);
            }
            o.set_mp(if mode == 1 { 500 } else { 200 });
            o.m.set_i32(0x354, obj as i32);
            o.m.set_i32(0x340, def);
        }
        self.g.set_mp_cost_on(1);
        self.bw(0x450bbc, 0);
        self.g.set_end_timer(0);
        let f: Vec<u8> = (0..60).map(|k| self.g.m.u8(0x44eed0 + k)).take_while(|&c| c != 0).collect();
        if !f.is_empty() {
            self.play_music(&f);
        }
        for j in 20..MAX_OBJECTS {
            if self.g.m.u8(EXISTS + j as u32) == 0 {
                crate::collisions::obj_reset(&mut self.objs[j]);
            }
        }
        self.end_match();
        self.recording_start();
    }
}
