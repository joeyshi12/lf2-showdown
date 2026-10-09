//! The computer player AI for characters: `ai_character` (0x4094b0) and its helpers
//! `ai_use_weapon` (0x408cb0), `ai_basic_attacks` (0x4034f0) and `between` (0x4061a0).
//! `ai_use_specials` (0x403a40) is in `ai_specials.rs`.
//!
//! The AI writes the object's key flags (0xcd..0xd3) and the special-input counters
//! (0xd4..0xdc; 3 starts the move), after copying the previous keys to 0xc6..0xcc.

use crate::mem::ftol;
use crate::world::{World, MAX_OBJECTS};

/// `abs_int` (0x4034e0).
#[inline]
pub(crate) fn abs(v: i32) -> i32 {
    v.wrapping_abs()
}

/// 0x4061a0: 1 when `b` lies strictly between `a` and `c`, in either order.
fn between(a: i32, b: i32, c: i32) -> bool {
    !((b <= a || c <= b) && (b <= c || a <= b))
}

const NO_CMD: i32 = -1000;

impl World {
    /// Copy the current keys to the previous keys and clear the current keys.
    fn ai_shift_keys(&mut self, s: usize) {
        let o = &mut self.objs[s];
        o.set_prev_up(o.up());
        o.set_prev_down(o.down());
        o.set_prev_left(o.left());
        o.set_prev_right(o.right());
        o.set_prev_attack(o.attack());
        o.set_prev_jump(o.jump());
        o.set_prev_defend(o.defend());
        ai_clear_keys(o);
    }

    /// The test the AI repeats before moving towards `t`: true unless the object was
    /// told to hold its position (0x404) and `t` is outside 151 by 241.
    pub(crate) fn ai_in_range(&self, s: usize, t: usize) -> bool {
        let (o, p) = (&self.objs[s], &self.objs[t]);
        o.cmd_hold() == 0 || (abs(o.iz() - p.iz()) < 0x97 && abs(o.ix() - p.ix()) < 0xf1)
    }

    fn ai_is_enemy(&self, mode: i32, s: usize, j: usize) -> bool {
        let st = self.objs[s].team();
        let ot = self.objs[j].team();
        (ot != st && (mode != 1 || st == 5)) || (ot == 5 && mode == 1 && st != 5)
    }

    /// 0x4094b0.
    pub fn ai_character(&mut self, s: usize, mode: i32) {
        let bg = self.g.background();
        if mode == 1 && self.g.u_450bac() == 1 {
            self.ai_shift_keys(s);
            let o = &mut self.objs[s];
            o.set_right(1);
            o.set_prev_right(0);
            if o.blocked_xpos() != 1 {
                return;
            }
            o.set_prev_attack(0);
            o.set_attack(1);
            return;
        }
        if NO_CMD < self.objs[s].cmd_x() {
            self.ai_walk_to_command(s);
            return;
        }
        let limit = if self.g.u_450bb4() < 1 { self.background().width } else { self.g.u_450bb4() };
        self.g.set_ai_x_limit(limit);
        let forced_easy = self.g.u_450c2c() == 1
            || (mode == 1 && self.objs[s].team() != 5 && (s < 0x14 || self.data_of(s).id < 0x1e));
        let level = if forced_easy || self.g.difficulty() < 0 { 0 } else { self.g.difficulty() };
        self.g.set_ai_level(level);
        self.g.set_ai_level3(level * 3);
        self.g.set_ai_level5(level * 5);
        self.g.set_ai_level15(level * 15);
        self.g.set_ai_level20(level * 20);
        let mut near_dist = 10000; // local_28
        let mut fallen_dist = 10000; // local_18
        let mut item_dist = 10000; // local_10
        let mut idx: i32 = -1;
        let mut near_z = 0; // local_2c
        let mut allies = 0; // local_50
        let mut want_milk = false; // bVar19
        let mut no_milk = false; // bVar20
        self.g.set_ai_hold_back(0);
        if (mode == 1 || mode == 4) && self.objs[s].team() != 5 {
            let team = self.objs[s].team();
            let (hp, max_hp) = (self.objs[s].hp(), self.objs[s].max_hp());
            want_milk = !((max_hp * 4) / 5 < hp || max_hp - 0x82 < hp);
            for j in 0..MAX_OBJECTS {
                if j != s && self.exists(j) {
                    let o = &self.objs[j];
                    if 0 < o.hp() && self.data_of(j).kind == 0 && o.team() == team {
                        if o.hp() < self.objs[s].hp() {
                            want_milk = false;
                        }
                        allies += 1;
                    }
                }
            }
            let hp = self.objs[s].hp();
            no_milk = 0x1ae < hp || self.objs[s].max_hp() - 0x82 < hp;
            for j in 0..MAX_OBJECTS {
                if j != s && self.exists(j) {
                    let o = &self.objs[j];
                    if 0 < o.hp() && self.data_of(j).kind == 0 && o.team() == team && o.hp() < hp - 200 {
                        no_milk = true;
                    }
                }
            }
            if mode == 1 {
                // the right-most living character among objects 0..9
                let mut max_x = -1;
                let mut max_z = 0;
                for j in 0..10 {
                    if j != s && self.exists(j) {
                        let o = &self.objs[j];
                        if 0 < o.hp() && self.data_of(j).kind == 0 && max_x < o.ix() {
                            max_z = o.iz();
                            max_x = o.ix();
                        }
                    }
                }
                if -1 < max_x {
                    let o = &self.objs[s];
                    if max_x < o.ix() && 200 < (abs(o.iz() - max_z) / 2 - max_x) + o.ix() {
                        self.g.set_ai_hold_back(1);
                    }
                    if max_x + 400 < self.objs[s].ix() {
                        self.g.set_ai_hold_back(2);
                    }
                }
            }
            if allies == 0 {
                want_milk = false;
            }
        }
        let o = &self.objs[s];
        let summoned = -1 < o.m.i32(0x2f4);
        if summoned {
            no_milk = true;
        }
        let mut no_beer = 0xfa < o.mp() || summoned;
        if mode == 1 && o.team() == 1 {
            no_beer = true;
        }
        if 0x13 < s && mode == 4 {
            no_beer = true;
        }

        // nearest enemy character (or incoming ball, state 3000) that can be hit
        for j in 0..MAX_OBJECTS {
            if j == s || !self.exists(j) {
                continue;
            }
            let p = &self.objs[j];
            let candidate = if self.data_of(j).kind == 0 {
                true
            } else if self.state(j) == 3000 {
                let sx = self.objs[s].ix();
                (sx < p.ix() && p.vx() < 0.0) || (p.ix() < sx && 0.0 < p.vx())
            } else {
                false
            };
            if candidate
                && self.ai_is_enemy(mode, s, j)
                && 0 < p.hp()
                && self.state(j) != 0xe
                && abs(p.u_08()) < 3
            {
                let o = &self.objs[s];
                let d = abs(p.iz() - o.iz()) + abs(p.ix() - o.ix());
                if d < near_dist {
                    idx = j as i32;
                    near_dist = d;
                }
            }
        }
        if -1 < idx && abs(self.objs[idx as usize].iz() - self.objs[s].iz()) < 0xf {
            near_z = 1;
        }
        // otherwise the nearest enemy lying down or invulnerable, within 250 by 40
        if self.state(s) != 9 {
            for j in 0..MAX_OBJECTS {
                if j == s || !self.exists(j) {
                    continue;
                }
                let p = &self.objs[j];
                if self.ai_is_enemy(mode, s, j) && 0 < p.hp() && (self.state(j) == 0xe || 2 < abs(p.u_08())) {
                    let o = &self.objs[s];
                    let dz = abs(p.iz() - o.iz());
                    let dx = abs(p.ix() - o.ix());
                    if dx + dz < fallen_dist && dz < 0x28 && dx < 0xfa {
                        idx = j as i32;
                        fallen_dist = dx + dz;
                    }
                }
            }
        }
        let prev = self.objs[s].ai_target();
        if !(0..=399).contains(&prev) {
            self.objs[s].set_ai_target(idx);
        } else if !self.exists(prev as usize)
            || self.objs[prev as usize].hp() < 1
            || self.random(0x13, 0x1e) < 1
            || self.data_of(self.objs[s].ai_target() as usize).kind != 0
        {
            self.objs[s].set_ai_target(idx);
        } else {
            idx = self.objs[s].ai_target();
        }
        self.ai_shift_keys(s);
        let mut danger_down = false; // bVar5
        let mut block_left = 0; // local_4c
        let mut block_right = 0; // local_40
        let mut danger_up = false; // bVar6
        let mut danger = 0; // local_30
        let mut avoid = false; // bVar4
        let mut heal_target = false; // bVar3
        if idx < 0 {
            self.ai_no_target(s, idx, MAX_OBJECTS);
            return;
        }
        let enemy = idx;
        for j in 20..MAX_OBJECTS {
            if !self.exists(j) {
                continue;
            }
            let id = self.data_of(j).id;
            let (pf, pteam) = (self.objs[j].frame(), self.objs[j].team());
            if id == 200 {
                let f10 = pf / 10;
                let o = &self.objs[s];
                let hit = if f10 == 6 && pteam != o.team() {
                    true
                } else if f10 == 5 {
                    let sid = self.data_of(s).id;
                    (sid == 2 || sid == 0x22)
                        && (o.max_hp() - 0x46 <= o.hp() || o.max_hp() - 200 <= o.hp())
                        && ((o.max_hp() * 3) / 5 <= o.hp() || o.hp() < o.max_hp() - 200)
                        && pteam == o.team()
                } else {
                    false
                };
                if hit {
                    let p = &self.objs[j];
                    let dz = abs(p.iz() - o.iz());
                    avoid = true;
                    if dz < 0x19 {
                        let dx = abs(p.ix() - o.ix());
                        if dx < 0x96 {
                            danger = 1;
                            if dz < 0x14 {
                                if dx < 0xb4 {
                                    if o.iz() < p.iz() {
                                        danger_up = true;
                                    } else {
                                        danger_down = true;
                                    }
                                }
                                if o.ix() < p.ix() {
                                    block_right = 1;
                                } else {
                                    block_left = 1;
                                }
                            }
                        }
                    }
                }
            }
            if (id == 0xd3 && self.state(j) == 0x12) || (id == 0xd4 && 0x95 < pf && pf < 0xab) {
                let (p, o) = (&self.objs[j], &self.objs[s]);
                let px = p.ix();
                if abs(px - o.ix()) < 0x50 {
                    if o.iz() + 0x14 < p.iz() {
                        danger_up = true;
                    } else if p.iz() < o.iz() - 0x14 {
                        danger_down = true;
                    }
                }
                if abs(p.iz() - o.iz()) < 0x14 {
                    if o.ix() + 100 < px {
                        block_right = 1;
                    } else if px < o.ix() - 100 {
                        block_left = 1;
                    }
                }
            }
            if !heal_target && near_z == 0 && !avoid {
                let (p, o) = (&self.objs[j], &self.objs[s]);
                let d = abs(p.iz() - o.iz()) + abs(p.ix() - o.ix());
                let st = self.state(j);
                if o.weapon_type() == 0
                    && d < near_dist * 2
                    && d < item_dist
                    && (id / 100 == 1 || id == 0xd5)
                    && p.weapon_type() == 0
                    && (st == 0x3ec || st == 0x7d4)
                    && (!no_milk || id != 0x7a)
                    && (!no_beer || id != 0x7b)
                    && (o.cmd_hold() != 1 || id == 0x7a)
                {
                    idx = j as i32;
                    item_dist = d;
                }
            }
            if id == 200 && pf / 10 == 5 {
                let (p, o) = (&self.objs[j], &self.objs[s]);
                if abs(p.ix() - o.ix()) < 300 && abs(p.iz() - o.iz()) < 0x5a && pteam == o.team() {
                    if (o.hp() < o.dark_hp() - 0x46 && o.hp() < 0x8c)
                        || (o.hp() < (o.dark_hp() * 3) / 5 && 0x8b < o.hp())
                    {
                        idx = j as i32;
                    }
                    heal_target = true;
                }
            }
            if want_milk && id == 0x7a && self.state(j) == 0x3ec && self.objs[s].weapon_type() == 0 {
                idx = j as i32;
                heal_target = true;
            }
        }
        if avoid {
            idx = enemy;
        }
        if idx < 0 {
            self.ai_no_target(s, idx, MAX_OBJECTS);
            return;
        }
        let t = idx as usize;
        let ts = self.state(t);
        let ss = self.state(s);
        let lvl3 = self.g.ai_level3();
        if self.random(0x14, self.g.ai_level5() + 8) == 0 {
            let o = &self.objs[s];
            if o.blocked_zneg() == 1 || o.blocked_zpos() == 1 || o.blocked_xneg() == 1 || o.blocked_xpos() == 1 {
                self.objs[s].set_prev_attack(0);
                self.objs[s].set_attack(1);
            }
        }
        if ts == 3000 {
            if ss != 7 && self.random(0x15, lvl3) == 0 {
                let (p, sx) = (&self.objs[t], self.objs[s].ix());
                let px = p.ix();
                if (sx < px && px < sx + 200 && p.vx() < 0.0) || (px < sx && sx - 200 < px && 0.0 < p.vx()) {
                    self.objs[s].set_prev_defend(0);
                    self.objs[s].set_defend(1);
                }
            }
            let tx = self.objs[t].ix();
            let o = &mut self.objs[s];
            if o.ix() < tx && o.facing() == 1 {
                o.set_right(1);
            }
            if o.ix() <= tx || o.facing() != 0 {
                return;
            }
            o.set_left(1);
            return;
        }
        if self.objs[s].cmd_hold() == 1 {
            let o = &self.objs[s];
            if 0 < o.weapon_type() {
                let wid = self.data_of(o.weapon_held() as usize).id;
                if wid == 0x7a || wid == 0x7b {
                    self.objs[s].set_prev_attack(0);
                    self.objs[s].set_attack(1);
                    return;
                }
            }
            let o = &self.objs[s];
            let near_cmd = |o: &crate::fields::Obj| abs(o.cmd_z() - o.iz()) < 0x5b && abs(o.cmd_x() - o.ix()) < 0x5b;
            if o.cmd_x() == NO_CMD || near_cmd(o) {
                if near_cmd(o) {
                    self.objs[s].set_cmd_x(NO_CMD);
                    self.objs[s].set_cmd_z(NO_CMD);
                }
                let tx = self.objs[t].ix();
                let o = &mut self.objs[s];
                if o.ix() < tx && o.facing() == 1 {
                    o.set_right(1);
                }
                if tx < o.ix() && o.facing() == 0 {
                    o.set_left(1);
                }
                if ss == 2 {
                    if o.facing() == 1 {
                        o.set_right(1);
                    }
                    if o.facing() == 0 {
                        o.set_left(1);
                    }
                }
            }
        }
        if ts == 0x3ec || ts == 0x7d4 {
            // walk to the item and pick it up
            let tid = self.data_of(t).id;
            if self.objs[s].cmd_hold() == 0 || self.ai_in_range(s, t) || tid == 0x7a || tid == 0x7b {
                let tx = self.objs[t].ix();
                let hold_back = self.g.ai_hold_back();
                if tx + 6 < self.objs[s].ix() {
                    self.objs[s].set_left(1);
                    if tx + 0xfa < self.objs[s].ix() && self.random(0x16, lvl3 + 3) == 0 {
                        self.objs[s].set_prev_left(0);
                    }
                    let o = &mut self.objs[s];
                    if o.ix() < tx + 100 && ss == 2 && o.facing() == 1 {
                        o.set_right(1);
                    }
                } else if self.objs[s].ix() < tx - 6 {
                    if hold_back == 0 {
                        self.objs[s].set_right(1);
                    }
                    if self.objs[s].ix() < tx - 0xfa && self.random(0x17, lvl3 + 3) == 0 && hold_back == 0 {
                        self.objs[s].set_prev_right(0);
                    }
                    let o = &mut self.objs[s];
                    if tx - 100 < o.ix() && ss == 2 && o.facing() == 0 {
                        o.set_left(1);
                    }
                }
                let tz = self.objs[t].iz();
                let o = &mut self.objs[s];
                if o.iz() < tz - 3 {
                    o.set_down(1);
                } else if tz + 3 < o.iz() {
                    o.set_up(1);
                }
            }
            let (tx, tz) = (self.objs[t].ix(), self.objs[t].iz());
            let o = &mut self.objs[s];
            if 3 < abs(tz - o.iz()) || 6 < abs(tx - o.ix()) {
                return;
            }
            o.set_prev_attack(0);
            o.set_attack(1);
            return;
        }
        if self.ai_in_range(s, t) && (ts == 0xe || 2 < abs(self.objs[t].u_08())) {
            // the target cannot be hit: keep away from it
            let tx = self.objs[t].ix();
            if self.g.ai_x_limit() - 0x1e < tx {
                self.objs[s].set_left(1);
                self.objs[s].set_prev_left(0);
                return;
            }
            if tx < 0x1e {
                self.objs[s].set_right(1);
                self.objs[s].set_prev_right(0);
                return;
            }
            let o = &self.objs[s];
            if 0x2d < abs(self.objs[t].iz() - o.iz()) && 0x15e < abs(tx - o.ix()) {
                return;
            }
            let l20 = self.g.ai_level20();
            if self.objs[s].ix() < tx {
                self.objs[s].set_left(1);
                if self.random(0x18, l20 + 0x23) == 0 {
                    self.objs[s].set_prev_left(0);
                }
            } else {
                self.objs[s].set_right(1);
                if self.random(0x19, l20 + 0x23) == 0 {
                    self.objs[s].set_prev_right(0);
                }
            }
            let tz = self.objs[t].iz();
            let zmin = self.gd.bg(bg).zboundary.0;
            let o = &mut self.objs[s];
            if tz < o.iz() || tz < zmin + 10 {
                o.set_down(1);
            } else {
                o.set_up(1);
            }
            return;
        }
        if self.data_of(t).id == 200 {
            let (tx, tz) = (self.objs[t].ix(), self.objs[t].iz());
            let o = &mut self.objs[s];
            if o.ix() + 7 < tx {
                o.set_right(1);
            } else if tx < o.ix() - 7 {
                o.set_left(1);
            }
            if o.iz() + 2 < tz {
                o.set_down(1);
            } else if tz < o.iz() - 2 {
                o.set_up(1);
            }
            return;
        }
        if self.ai_use_specials(t, s, ss, ts, danger, near_dist, near_z) != 0 {
            return;
        }
        if self.ai_in_range(s, t) {
            let hold_back = self.g.ai_hold_back();
            let l20 = self.g.ai_level20();
            {
                let o = &mut self.objs[s];
                if (block_right == 1 || hold_back == 1) && ss == 2 && o.facing() == 0 {
                    o.set_left(1);
                }
                if block_left == 1 && ss == 2 && o.facing() == 1 {
                    o.set_right(1);
                }
            }
            let sid = self.data_of(s).id;
            let keep_distance = [4, 5, 0x1f].contains(&sid) || {
                let (o, p) = (&self.objs[s], &self.objs[t]);
                (o.hp() * 2 < p.hp() || (o.hp() < 0x65 && 100 < o.max_hp()))
                    && mode == 1
                    && self.data_of(t).kind == 0
                    && 0x13 < s
                    && o.team() != 5
            };
            if keep_distance {
                let (sx, tx, f) = (self.objs[s].ix(), self.objs[t].ix(), self.objs[s].facing());
                if (sx + 0xaa < tx || ((sx + 0x96 < tx || (ss == 7 && sx < tx)) && f == 1))
                    && block_right == 0
                    && hold_back == 0
                {
                    self.objs[s].set_right(1);
                    if self.random(0x1a, l20 + 0x23) == 0 {
                        self.objs[s].set_prev_right(0);
                    }
                }
                let (sx, tx, f) = (self.objs[s].ix(), self.objs[t].ix(), self.objs[s].facing());
                if (tx < sx - 0xaa || ((tx < sx - 0x96 || (ss == 7 && tx < sx)) && f == 0)) && block_left == 0 {
                    self.objs[s].set_left(1);
                    if self.random(0x1b, l20 + 0x23) == 0 {
                        self.objs[s].set_prev_left(0);
                    }
                }
            } else if ss != 0x13 {
                let (sx, tx, f) = (self.objs[s].ix(), self.objs[t].ix(), self.objs[s].facing());
                if (sx + 0x3c < tx || (sx < tx && f == 1)) && block_right == 0 && (hold_back == 0 || f == 1) {
                    self.objs[s].set_right(1);
                    if self.random(0x1c, l20 + 0x23) == 0 {
                        self.objs[s].set_prev_right(0);
                    }
                }
                let (sx, tx, f) = (self.objs[s].ix(), self.objs[t].ix(), self.objs[s].facing());
                if (tx < sx - 0x3c || (tx < sx && f == 0)) && block_left == 0 {
                    self.objs[s].set_left(1);
                    if self.random(0x1d, l20 + 0x23) == 0 {
                        self.objs[s].set_prev_left(0);
                    }
                }
            }
            let (sz, tz) = (self.objs[s].iz(), self.objs[t].iz());
            let blocked = block_right == 1 || block_left == 1;
            if ((sz + 3 < tz && danger == 0) || (blocked && danger_down)) && !danger_up && ss != 0x13 {
                self.objs[s].set_down(1);
            }
            if ((tz < sz - 3 && danger == 0) || (blocked && danger_up)) && !danger_down && ss != 0x13 {
                self.objs[s].set_up(1);
            }
        }
        if 0 < self.objs[s].weapon_type() && self.ai_use_weapon(t, s, ss, ts, near_z, danger) == 0 {
            return;
        }
        if self.random(0x1e, self.g.ai_level() * 7 + 10) == 0 && (ts == 3 || ts / 100 == 3) {
            let (p, o) = (&self.objs[t], &self.objs[s]);
            if abs(p.iz() - o.iz()) < 9 && ((p.facing() == 0 && p.ix() < o.ix()) || (p.facing() == 1 && o.ix() < p.ix())) {
                self.objs[s].set_defend(1);
            }
        }
        if self.ai_in_range(s, t)
            && self.random(0x1f, (self.g.ai_level() * 5 + 10) * 2) < 3
            && self.random(0x20, 0x14) < 3
            && ts != 0xe
        {
            self.objs[s].set_jump(1);
        }
        let sid = self.data_of(s).id;
        let sid_far = [4, 5, 0x1f].contains(&sid);
        if !sid_far || ts == 0x10 {
            let (o, p) = (&self.objs[s], &self.objs[t]);
            if abs(p.ix() + ftol(o.vx()) * -2 - o.ix()) < 0x50
                && abs(p.iz() - o.iz()) < 5
                && self.random(0x21, lvl3 + 3) == 0
                && ts != 0xe
            {
                self.objs[s].set_attack(1);
            }
        }
        // 0x40b4e0..0x40b78f
        let mut attack_x: Option<i32> = None; // LAB_0040b590 with the target's x
        let mut retreat: Option<i32> = None; // the random() site for the retreat jump
        let mut check_retreat = false; // LAB_0040b5c8
        if self.objs[s].weapon_type() == 0 {
            if ts == 0x10 && sid_far {
                let (o, p) = (&self.objs[s], &self.objs[t]);
                if !(0x15d < abs(p.ix() + ftol(o.vx()) * -2 - o.ix())
                    || 4 < abs(p.iz() - o.iz())
                    || self.random(0x22, lvl3 + 3) != 0)
                {
                    attack_x = Some(self.objs[t].ix());
                }
            } else if ts == 0x10 || ![4, 5, 0x1f, 0x24].contains(&sid) {
                check_retreat = true;
            } else {
                let (o, p) = (&self.objs[s], &self.objs[t]);
                let close = p.ix() - o.ix() < 100 && abs(p.iz() - o.iz()) < 0x50 && self.random(0x23, lvl3 + 2) == 0;
                if close && ss != 7 {
                    retreat = Some(0x24);
                } else {
                    let (o, p) = (&self.objs[s], &self.objs[t]);
                    if abs(p.ix() + ftol(o.vx()) * -2 - o.ix()) < 300
                        && abs(p.iz() - o.iz()) < 5
                        && self.random(0x25, lvl3 + 3) == 0
                        && ts != 0xe
                    {
                        attack_x = Some(self.objs[t].ix());
                    }
                }
            }
        } else {
            check_retreat = true;
        }
        if check_retreat {
            let (o, p) = (&self.objs[s], &self.objs[t]);
            if (o.hp() * 2 < p.hp() || (o.hp() < 0x65 && 100 < o.max_hp()))
                && mode == 1
                && self.data_of(t).kind == 0
                && 0x13 < s
                && o.team() != 5
            {
                let close = p.ix() - o.ix() < 100 && abs(p.iz() - o.iz()) < 0x50 && self.random(0x26, lvl3 + 2) == 0;
                if close && ss != 7 {
                    retreat = Some(0x27);
                }
            }
        }
        if let Some(site) = retreat {
            if self.ai_in_range(s, t) {
                let tx = self.objs[t].ix();
                let limit = self.g.ai_x_limit();
                let o = &mut self.objs[s];
                if (tx < 0xfa || tx < o.ix()) && tx <= limit - 0xfa {
                    o.set_right(1);
                    o.set_prev_right(0);
                } else if limit - 0xfa < tx || o.ix() < tx {
                    o.set_left(1);
                    o.set_prev_left(0);
                }
            }
            if self.ai_in_range(s, t) && self.random(site, 0x11) == 0 {
                self.objs[s].set_jump(1);
            }
        }
        if let Some(tx) = attack_x {
            let o = &mut self.objs[s];
            let facing_target = if o.ix() < tx { o.facing() == 0 } else { o.facing() == 1 };
            if facing_target {
                o.set_attack(1);
            }
        }
        self.ai_basic_attacks(t, s, ss, ts, near_z, block_right, block_left);
    }

    /// 0x4094b0, the part after the target search when there is no target
    /// (LAB_0040b7b5). `last` is the loop counter the original reads the object from.
    fn ai_no_target(&mut self, s: usize, idx: i32, last: usize) {
        if idx != -1 {
            return;
        }
        if self.g.ai_hold_back() == 1 {
            // with cmd_hold the original measures against objects[400], which is the
            // file-manager pointer after the array: its "iz" and "ix" are data
            // pointers, far out of range, so only cmd_hold == 0 walks left
            let _ = last;
            if self.objs[s].cmd_hold() == 0 {
                self.objs[s].set_left(1);
            }
        }
        let o = &mut self.objs[s];
        let (lo, hi) = match self.gd.objects[o.m.i32(0x368) as usize].id {
            7 => (0xff, 0x105),
            9 => (0x118, 0x122),
            0x20 => (0xf0, 0xf5),
            _ => return,
        };
        if o.frame() < lo {
            return;
        }
        if o.frame() <= hi {
            o.set_defend(1);
        }
    }

    /// 0x4094b0, the start: walk to the point a team command gave (0x3fc, 0x400).
    fn ai_walk_to_command(&mut self, s: usize) {
        self.ai_shift_keys(s);
        let st = self.state(s);
        let lvl3 = self.g.ai_level3();
        let (ix, cx) = (self.objs[s].ix(), self.objs[s].cmd_x());
        if cx + 6 < ix {
            self.objs[s].set_left(1);
            if cx + 0xfa < ix && self.random(0x11, lvl3 + 3) == 0 {
                self.objs[s].set_prev_left(0);
            }
            let o = &mut self.objs[s];
            if o.ix() < o.cmd_x() + 100 && st == 2 && o.facing() == 1 {
                o.set_right(1);
            }
        } else if ix < cx - 6 {
            self.objs[s].set_right(1);
            if ix < cx - 0xfa && self.random(0x12, lvl3 + 3) == 0 {
                self.objs[s].set_prev_right(0);
            }
            let o = &mut self.objs[s];
            if o.cmd_x() - 100 < o.ix() && st == 2 && o.facing() == 0 {
                o.set_left(1);
            }
        }
        let o = &mut self.objs[s];
        if o.iz() < o.cmd_z() - 3 {
            o.set_down(1);
        } else if o.cmd_z() + 3 < o.iz() {
            o.set_up(1);
        }
        if o.blocked_xpos() == 1 || o.blocked_xneg() == 1 {
            o.set_prev_attack(0);
            o.set_attack(1);
        }
        if 0x5a < abs(o.cmd_z() - o.iz()) || 0x5a < abs(o.cmd_x() - o.ix()) {
            return;
        }
        o.set_cmd_x(NO_CMD);
        o.set_cmd_z(NO_CMD);
    }

    /// 0x408cb0: use the held weapon. Returns 0 to end the AI call.
    fn ai_use_weapon(&mut self, t: usize, s: usize, ss: i32, ts: i32, near_z: i32, danger: i32) -> i32 {
        let bg = self.g.background() as usize;
        let lvl3 = self.g.ai_level3();
        if 0 < self.random(0x28, lvl3 + 1) {
            return 0;
        }
        let wid = self.data_of(self.objs[s].weapon_held() as usize).id;
        let mut blocked = false;
        for j in 0..20 {
            if j == s || !self.exists(j) {
                continue;
            }
            let (p, o, q) = (&self.objs[j], &self.objs[s], &self.objs[t]);
            if p.team() != 0
                && q.team() == o.team()
                && 0 < p.hp()
                && self.state(j) != 0xe
                && abs(p.u_08()) < 3
                && abs(p.iz() - o.iz()) < 0xf
                && between(o.ix(), p.ix(), q.ix())
            {
                blocked = true;
            }
        }
        if ss == 2 && self.random(0x29, lvl3 + 5) == 0 {
            if blocked {
                self.objs[s].set_jump(1);
            } else {
                self.objs[s].set_attack(1);
            }
        }
        if [100, 0x65, 0x78, 0x79, 0x7c].contains(&wid) {
            let (o, p) = (&self.objs[s], &self.objs[t]);
            if abs(p.ix() + ftol(o.vx()) * -2 - o.ix()) < 0x73
                && abs(p.iz() - o.iz()) < 6
                && self.random(0x2a, lvl3 + 3) == 0
                && ts != 0xe
            {
                self.objs[s].set_attack(1);
            }
            if wid == 0x7c && self.random(0x2b, self.g.ai_level15() + 0x1e) == 0 {
                self.objs[s].set_attack(1);
            }
            if self.random(0x2c, lvl3 + 5) == 0 && self.ai_in_range(s, t) {
                let (o, p) = (&self.objs[s], &self.objs[t]);
                let tx = p.ix();
                if abs(tx - o.ix()) < 600 && abs(p.iz() - o.iz()) < 0x14 {
                    let hold_back = self.g.ai_hold_back();
                    let o = &mut self.objs[s];
                    if o.ix() < tx && hold_back == 0 {
                        o.set_right(1);
                        o.set_prev_right(0);
                    }
                    if tx < o.ix() {
                        o.set_left(1);
                        o.set_prev_left(0);
                    }
                }
            }
        }
        if (wid == 0x96 || wid == 0x97) && !blocked {
            let (o, p) = (&self.objs[s], &self.objs[t]);
            if abs(p.ix() + ftol(o.vx()) * -2 - o.ix()) < 300
                && abs(p.iz() - o.iz()) < 6
                && self.random(0x2d, self.g.ai_level5() + 7) == 0
                && ts != 0xe
            {
                self.objs[s].set_attack(1);
            }
        }
        if wid != 0x7a && wid != 0x7b {
            return 1;
        }
        // holding a drink: get away from the target and drink
        ai_clear_keys(&mut self.objs[s]);
        if ss == 0x11 && near_z == 1 && danger == 0 && self.objs[s].u_08() != 0 {
            self.objs[s].set_defend(1);
            return 0;
        }
        {
            let (o, p) = (&self.objs[s], &self.objs[t]);
            if o.cmd_hold() != 0 && (0x96 < abs(o.iz() - p.iz()) || 0xf0 < abs(o.ix() - p.ix())) {
                return 0;
            }
        }
        let (zmin, zmax) = self.gd.bg(bg as i32).zboundary;
        let tz = self.objs[t].iz();
        {
            let o = &mut self.objs[s];
            if zmin + 0x1e <= tz && (tz < zmax - 0x1e || o.iz() < tz) {
                o.set_up(1);
            } else {
                o.set_down(1);
            }
        }
        let tx = self.objs[t].ix();
        let limit = self.g.ai_x_limit();
        if tx < 400 && self.objs[s].ix() < 200 {
            self.objs[s].set_right(1);
            if self.random(0x2e, lvl3 + 7) == 0 {
                self.objs[s].set_prev_right(0);
            }
            if self.random(0x2f, lvl3 + 5) == 0 && ss == 2 {
                self.objs[s].set_jump(1);
                return 0;
            }
        } else if limit - 400 < tx && limit - 200 < self.objs[s].ix() {
            self.objs[s].set_left(1);
            if self.random(0x30, lvl3 + 7) == 0 {
                self.objs[s].set_prev_left(0);
            }
            if self.random(0x31, lvl3 + 5) == 0 && ss == 2 {
                self.objs[s].set_jump(1);
                return 0;
            }
        } else {
            let (o, p) = (&self.objs[s], &self.objs[t]);
            if abs(tx - o.ix()) < 0x15e && abs(p.iz() - o.iz()) < 0x46 {
                if o.ix() < tx {
                    self.objs[s].set_left(1);
                    if self.random(0x32, lvl3 + 4) == 0 {
                        self.objs[s].set_prev_left(0);
                    }
                }
                if self.objs[s].ix() < self.objs[t].ix() {
                    return 0;
                }
                self.objs[s].set_right(1);
                if self.random(0x33, lvl3 + 4) == 0 {
                    self.objs[s].set_prev_right(0);
                }
                return 0;
            }
            if ss == 2 {
                let o = &mut self.objs[s];
                if o.facing() == 0 {
                    o.set_left(1);
                }
                if o.facing() == 1 {
                    o.set_right(1);
                    return 0;
                }
            } else if self.random(0x34, 5) == 0 {
                if danger == 0 {
                    let sid = self.data_of(s).id;
                    if (sid == 2 || sid == 0x22) && 0x96 < self.objs[s].mp() && 0 < self.random(0x35, lvl3 + 3) {
                        let tx = self.objs[t].ix();
                        let o = &mut self.objs[s];
                        if tx <= o.ix() {
                            o.set_seq_dfj_l(3);
                        } else {
                            o.set_seq_dfj_r(3);
                        }
                        return 1;
                    }
                }
                self.objs[s].set_attack(1);
            }
        }
        0
    }

    /// 0x4034f0: attack, defend and dash attack when close to the target.
    #[allow(clippy::too_many_arguments)]
    fn ai_basic_attacks(&mut self, t: usize, s: usize, _ss: i32, ts: i32, _near_z: i32, block_right: i32, block_left: i32) {
        let lvl3 = self.g.ai_level3();
        {
            let (o, p) = (&self.objs[s], &self.objs[t]);
            if abs(p.ix() + ftol(o.vx()) * -2 - o.ix()) < 0x50
                && abs(p.iz() - o.iz()) < 5
                && self.random(0x37, lvl3 + 3) == 0
                && ts != 0xe
            {
                self.objs[s].set_attack(1);
            }
        }
        if block_right != 0 && self.objs[s].ix() < self.objs[t].ix() {
            return;
        }
        if block_left != 0 && self.objs[t].ix() < self.objs[s].ix() {
            return;
        }
        if self.random(0x38, lvl3 + 1) != 0 {
            return;
        }
        const RUNNERS: [i32; 10] = [2, 4, 6, 9, 10, 11, 8, 7, 0x21, 0x22];
        let sid = self.data_of(s).id;
        let lead = |w: &World| {
            let p = &w.objs[t];
            abs(p.ix() + ftol(p.vx()) * 2 - w.objs[s].ix())
        };
        if RUNNERS.contains(&sid) {
            let d = lead(self);
            if 100 < d
                && d < 900
                && abs(self.objs[t].iz() - self.objs[s].iz()) < 5
                && self.random(0x39, lvl3 + 10) == 0
                && ts != 0xe
            {
                self.objs[s].set_defend(1);
            }
        }
        let dash = |w: &mut World, dz_max: i32, monk: bool| {
            let (o, p) = (&w.objs[s], &w.objs[t]);
            let toward = (o.facing() == 0 && o.ix() < p.ix()) || (o.facing() == 1 && p.ix() < o.ix());
            if toward && (o.frame() == 0x6e || 0xea < o.frame()) && abs(p.iz() - o.iz()) < dz_max && ts != 0xe {
                let tx = p.ix();
                let o = &mut w.objs[s];
                o.set_prev_right(0);
                o.set_prev_left(0);
                o.set_prev_attack(0);
                if o.ix() < tx {
                    o.set_right(1);
                } else {
                    o.set_left(1);
                }
                if monk && w.random(0x3a, 2) == 0 {
                    w.objs[s].set_jump(1);
                } else {
                    w.objs[s].set_attack(1);
                }
            }
        };
        if RUNNERS.contains(&self.data_of(s).id) && 0x5a < lead(self) {
            let monk = self.data_of(s).id == 0x22;
            dash(self, 0xd, monk);
        }
        if self.data_of(s).id == 1 {
            let d = lead(self);
            if 100 < d
                && d < 300
                && abs(self.objs[t].iz() - self.objs[s].iz()) < 5
                && self.random(0x3b, self.g.ai_level5() + 10) == 0
                && ts != 0xe
            {
                self.objs[s].set_defend(1);
            }
        }
        if self.data_of(s).id == 1 && 0x5a < lead(self) {
            dash(self, 7, false);
        }
    }
}

pub(crate) fn ai_clear_keys(o: &mut crate::fields::Obj) {
    o.set_defend(0);
    o.set_jump(0);
    o.set_attack(0);
    o.set_down(0);
    o.set_up(0);
    o.set_left(0);
    o.set_right(0);
}
