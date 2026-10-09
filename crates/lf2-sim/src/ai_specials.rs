//! `ai_use_specials` (0x403a40): the per-character special-move decisions of the
//! character AI. Each branch tests the object's id; a chosen move is started by setting
//! its input counter (0xd4..0xdc) to 3, and the function then returns 1.

use crate::ai::abs;
use crate::mem::ftol;
use crate::world::{World, MAX_OBJECTS};

const DFA_R: u32 = 0xd4;
const DFA_L: u32 = 0xd5;
const DUA: u32 = 0xd6;
const DDA: u32 = 0xd7;
const DFJ_R: u32 = 0xd8;
const DFJ_L: u32 = 0xd9;
const DUJ: u32 = 0xda;
const DDJ: u32 = 0xdb;
const DJA: u32 = 0xdc;

impl World {
    fn ai_seq(&mut self, s: usize, field: u32) -> i32 {
        self.objs[s].m.set_i8(field, 3);
        1
    }

    /// Start the move whose input points towards `t` (right or left version).
    fn ai_seq_dir(&mut self, s: usize, t: usize, right: u32, left: u32) -> i32 {
        let f = if self.objs[s].ix() < self.objs[t].ix() { right } else { left };
        self.ai_seq(s, f)
    }

    fn ai_dx(&self, s: usize, t: usize) -> i32 {
        abs(self.objs[t].ix() - self.objs[s].ix())
    }

    fn ai_dz(&self, s: usize, t: usize) -> i32 {
        abs(self.objs[t].iz() - self.objs[s].iz())
    }

    /// Facing the target: facing right (0) with the target to the right, or the reverse.
    fn ai_facing_target(&self, s: usize, t: usize) -> bool {
        let (o, p) = (&self.objs[s], &self.objs[t]);
        (o.facing() == 0 && o.ix() < p.ix()) || (o.facing() == 1 && p.ix() < o.ix())
    }

    /// Distance to the target one tick ahead of the object's own x speed.
    fn ai_dx_ahead(&self, s: usize, t: usize) -> i32 {
        let (o, p) = (&self.objs[s], &self.objs[t]);
        abs(ftol(o.vx()) - o.ix() + p.ix())
    }

    fn ai_low_hp(&self, i: usize, margin: i32) -> bool {
        let o = &self.objs[i];
        (o.hp() < o.dark_hp() - margin && o.hp() < 0x8c) || (o.hp() < (o.dark_hp() * 3) / 5 && 0x8b < o.hp())
    }

    /// The healing branch shared by ids 2 and 0x22 (0x403ca1, 0x405618): go to a
    /// hurt teammate among objects 0..19 and start D^J when facing it or on it.
    fn ai_heal_teammate(&mut self, s: usize, near_dist: i32, near_z: i32) -> Option<i32> {
        let o = &self.objs[s];
        let healthy = (o.dark_hp() - 0x46 <= o.hp() || 0x8b < o.hp())
            && ((o.dark_hp() * 3) / 5 <= o.hp() || o.hp() < 0x8c);
        if !((o.weapon_type() == 0 || o.frame() < 9) && healthy && near_z == 0) {
            return None;
        }
        for j in 0..20 {
            if j == s || !self.exists(j) {
                continue;
            }
            let (p, o) = (&self.objs[j], &self.objs[s]);
            if p.team() == 0 || p.team() != o.team() {
                continue;
            }
            let dx = abs(p.ix() - o.ix());
            if dx >= 0xfa {
                continue;
            }
            let dz = abs(p.iz() - o.iz());
            if dz >= 0x3c || o.mp() <= 0x15e {
                continue;
            }
            if self.ai_low_hp(j, 0x5a) && 0 < p.hp() && dz + dx < near_dist / 3 {
                let px = p.ix();
                let o = &mut self.objs[s];
                if o.ix() < px {
                    o.set_right(1);
                    o.set_left(0);
                } else {
                    o.set_right(0);
                    o.set_left(1);
                }
                let sx = o.ix();
                if (sx < px && o.facing() == 0) || (px < sx && o.facing() == 1) || abs(px - sx) < 5 {
                    o.set_seq_duj(3);
                }
                return Some(1);
            }
        }
        None
    }

    /// 0x403a40. Returns 1 when it decided the keys for this tick.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn ai_use_specials(
        &mut self,
        t: usize,
        s: usize,
        _ss: i32,
        ts: i32,
        danger: i32,
        near_dist: i32,
        near_z: i32,
    ) -> i32 {
        if 0 < self.random(0x3c, self.g.ai_level5() + 1) {
            return 0;
        }
        let mp = |w: &World| w.objs[s].mp();
        let id = |w: &World| w.data_of(s).id;

        if id(self) == 2 {
            if self.random(0x3d, 10) == 0 && 0x15e < mp(self) && self.ai_low_hp(s, 0x46) {
                return self.ai_seq(s, DDJ);
            }
            if near_dist < 10000 && self.random(0x3e, 0x1e) == 0 && 0xfa < mp(self) {
                return self.ai_seq(s, DUA);
            }
            let tid = self.data_of(t).id;
            if [2, 9, 10, 11, 0x21, 0x22].contains(&tid) {
                if self.random(0x3f, 0xf) == 0 {
                    let d = self.ai_dx(s, t);
                    if 100 < d
                        && d < 500
                        && self.ai_dz(s, t) < 0x1e
                        && 100 < mp(self)
                        && 0xdc < self.objs[t].mp()
                        && danger == 0
                    {
                        return self.ai_seq_dir(s, t, DFJ_R, DFJ_L);
                    }
                }
            } else if self.random(0x40, 0xf) == 0 {
                let d = self.ai_dx(s, t);
                if !(d < 0x65
                    || 0xf9 < d
                    || 0x1d < self.ai_dz(s, t)
                    || mp(self) < 0x65
                    || self.objs[t].mp() < 0xab
                    || danger != 0)
                {
                    return self.ai_seq_dir(s, t, DFJ_R, DFJ_L);
                }
            }
            if let Some(r) = self.ai_heal_teammate(s, near_dist, near_z) {
                return r;
            }
        }

        if id(self) == 1 {
            let f = self.objs[s].frame();
            if 0x103 < f && f < 0x122 && self.ai_dx(s, t) < 100 && self.ai_dz(s, t) < 7 {
                let (ty, sy) = (self.objs[t].iy(), self.objs[s].iy());
                if (ty != 0 || sy != 0 || self.random(0x41, 3) != 0)
                    && (-1 < self.objs[t].iy() || -1 < self.objs[s].iy() || self.random(0x42, 7) != 0)
                {
                    if (self.objs[t].iy() < 0 && self.random(0x43, 5) == 0) || self.random(0x44, 0x1e) == 0 {
                        if self.ai_facing_target(s, t) {
                            self.objs[s].set_jump(1);
                        }
                        self.objs[s].set_prev_jump(0);
                    }
                    return 1;
                }
                self.objs[s].set_attack(1);
                self.objs[s].set_prev_attack(0);
                return 1;
            }
            if self.random(0x45, 7) == 0 {
                let far = 0x95 < self.ai_dx(s, t)
                    || 7 < self.ai_dz(s, t)
                    || mp(self) < 0x97
                    || ((self.random(0x46, 10) != 0 || ts == 3)
                        && (self.random(0x47, 3) < 1 || (ts != 0x10 && ts != 8 && ts != 0xb)));
                if !far {
                    return self.ai_seq_dir(s, t, DFJ_R, DFJ_L);
                }
            }
            if self.random(0x48, 7) == 0 && self.ai_dx(s, t) < 100 && self.ai_dz(s, t) < 7 && 0x4b < mp(self) {
                if 0x96 < mp(self)
                    && ((self.random(0x49, 10) == 0 && ts != 3) || (0 < self.random(0x4a, 3) && ts == 0x10))
                {
                    return self.ai_seq_dir(s, t, DFJ_R, DFJ_L);
                }
                return self.ai_seq(s, DDA);
            }
        }

        if id(self) == 4 {
            if 0x168 < mp(self)
                && self.ai_dx(s, t) < 100
                && self.ai_dz(s, t) < 0x46
                && self.random(0x4b, self.objs[s].hp() / 5 + 10) == 0
            {
                return self.ai_seq(s, DUJ);
            }
            if self.random(0x4c, 0x2d) == 0 {
                let d = self.ai_dx(s, t);
                if 100 < d && d < 0x226 && self.ai_dz(s, t) < 0x14 && 0xaa < mp(self) {
                    return self.ai_seq_dir(s, t, DFJ_R, DFJ_L);
                }
            }
            if self.random(0x4d, 0x1e) == 0 && 200 < mp(self) {
                let d = self.ai_dx(s, t);
                if 100 < d && d < 0xa0 && self.ai_dz(s, t) < 0x37 && self.ai_facing_target(s, t) {
                    return self.ai_seq(s, DJA);
                }
            }
        }

        if id(self) == 5 {
            if 0x1c2 < mp(self) && 100 < self.ai_dx(s, t) && 0x32 < self.ai_dz(s, t) && self.random(0x4e, 3) == 0 {
                if self.random(0x4f, 2) == 0 {
                    return self.ai_seq(s, DUJ);
                }
                return self.ai_seq(s, DDJ);
            }
            if 0x46 < mp(self) {
                let d = self.ai_dx(s, t);
                if 100 < d && d < 0xa0 && self.ai_dz(s, t) < 8 && self.random(0x50, 10) == 0 {
                    return self.ai_seq_dir(s, t, DFJ_R, DFJ_L);
                }
            }
            if self.random(0x51, 0x1e) == 0 && 200 < mp(self) {
                let d = self.ai_dx(s, t);
                if 100 < d && d < 0xa0 && self.ai_dz(s, t) < 0x37 {
                    let (o, tx) = (&self.objs[s], self.objs[t].ix());
                    if o.facing() == 0 && o.ix() < tx {
                        return self.ai_seq(s, DFA_R);
                    }
                    if o.facing() == 1 && tx < o.ix() {
                        return self.ai_seq(s, DFA_L);
                    }
                }
            }
        }

        if id(self) == 6 {
            if 100 < mp(self) {
                let d = self.ai_dx(s, t);
                if 0x50 < d && d < 0x82 && self.ai_dz(s, t) < 0x1e && self.random(0x52, 10) == 0 {
                    return self.ai_seq_dir(s, t, DFJ_R, DFJ_L);
                }
            }
            if mp(self) < 0x65 || 0x2c < self.ai_dx(s, t) || 4 < self.ai_dz(s, t) || self.random(0x53, 3) != 0 {
                if self.state(s) == 9 && self.random(0x54, 8) == 0 {
                    self.objs[s].set_jump(1);
                    self.objs[s].set_prev_jump(0);
                }
            } else {
                return self.ai_seq(s, DUJ);
            }
        }

        if id(self) == 7 {
            if let Some(r) = self.ai_specials_7(s, t, ts) {
                return r;
            }
        }

        if id(self) == 8 {
            let mut try_duj = ts == 0xd;
            if ts != 0xd {
                if 200 < mp(self)
                    && self.ai_dx(s, t) < 400
                    && self.ai_dz(s, t) < 0xaa
                    && self.random(0x59, 0xfa) == 0
                {
                    return self.ai_seq_dir(s, t, DFJ_R, DFJ_L);
                }
                if ts != 0xe {
                    if 200 < mp(self) {
                        let d = self.ai_dx(s, t);
                        if 0x3c < d && d < 0x118 && self.ai_dz(s, t) < 0x41 && self.random(0x5a, 0xf) == 0 {
                            return self.ai_seq_dir(s, t, DFJ_R, DFJ_L);
                        }
                    }
                    try_duj = true;
                }
            }
            if try_duj && ts != 0xe && 0x140 < mp(self) {
                let d = self.ai_dx(s, t);
                let dz = self.ai_dz(s, t);
                if (0x32 < d || 7 < dz || ts == 0xd)
                    && d < 0x7d
                    && dz < 0x19
                    && self.random(0x5b, 3) == 0
                    && self.ai_facing_target(s, t)
                {
                    return self.ai_seq(s, DUJ);
                }
            }
            if self.random(0x5c, 0x32) == 0
                && self.objs[s].weapon_type() == 0
                && 200 < mp(self)
                && 200 < self.ai_dx(s, t)
                && 0x32 < self.ai_dz(s, t)
            {
                return self.ai_seq(s, DDJ);
            }
        }

        if id(self) == 0xb || id(self) == 10 {
            let eleven = id(self) == 0xb;
            let (mp_min, dz_max, site) = if eleven { (0x96, 0x1e, 0x5d) } else { (100, 0x19, 0x5f) };
            if mp_min < mp(self) && self.ai_dx(s, t) < 0x118 && self.ai_dz(s, t) < dz_max && self.random(site, 10) == 0 {
                let (o, tx) = (&self.objs[s], self.objs[t].ix());
                if o.facing() == 0 && o.ix() < tx {
                    return self.ai_seq(s, DDA);
                }
                // facing left with the target to the left: return without a move
                if o.facing() == 1 && tx < o.ix() {
                    return 1;
                }
            }
            if eleven {
                if self.frame_of(s).hit_j == 0x122 && self.objs[t].iy() < 0 {
                    self.objs[s].set_prev_jump(0);
                    self.objs[s].set_jump(1);
                }
                if (self.random(0x5e, 5) == 0 || ts == 0x10 || ts == 8)
                    && self.ai_dx_ahead(s, t) < 100
                    && self.ai_dz(s, t) < 7
                    && 200 < mp(self)
                    && self.ai_facing_target(s, t)
                {
                    return self.ai_seq(s, DUA);
                }
            } else {
                if self.objs[s].frame() == 0x10f && self.objs[t].iy() < 0 && ts == 0xc {
                    return self.ai_seq(s, DUA);
                }
                if (self.random(0x60, 10) == 0 || ts == 0x10 || ts == 8)
                    && self.ai_dx_ahead(s, t) < 0x50
                    && self.ai_dz(s, t) < 7
                    && self.ai_facing_target(s, t)
                {
                    return self.ai_seq(s, DUA);
                }
                let mut dfj = false;
                if 0xc9 <= mp(self) {
                    let d = self.ai_dx(s, t);
                    let skip = d < 0x3d
                        || 0x117 < d
                        || 0x40 < self.ai_dz(s, t)
                        || (self.random(0x61, 0xf) != 0
                            && (self.random(0x62, 4) != 0
                                || (ts != 0x10 && ts != 8 && (ts != 0xc || -0x29 < self.objs[t].iy()))));
                    dfj = !skip;
                }
                if dfj {
                    return self.ai_seq_dir(s, t, DFJ_R, DFJ_L);
                }
                let hp = self.objs[s].hp();
                if hp < 0xfa && hp < self.objs[t].hp() + 0x32 && self.random(99, 0x14) == 0 && 0x4b < mp(self) {
                    let mut best = -1;
                    let mut found: Option<usize> = None;
                    for j in 0..MAX_OBJECTS {
                        if j == s || !self.exists(j) || self.data_of(j).kind != 0 {
                            continue;
                        }
                        let (p, o) = (&self.objs[j], &self.objs[s]);
                        if p.team() == o.team() && self.objs[t].hp() < p.hp() {
                            let d = abs(p.iz() - o.iz()) + abs(p.ix() - o.ix());
                            if best < d {
                                best = d;
                                found = Some(j);
                            }
                        }
                    }
                    if found.is_some() && 300 < best && self.objs[s].weapon_type() == 0 {
                        self.objs[s].set_seq_ddj(3);
                    }
                }
                if self.objs[t].hp() < self.objs[s].hp() && self.random(100, 0x46) == 0 && 500 < mp(self) {
                    self.objs[s].set_seq_duj(3);
                }
            }
        }

        if id(self) == 9 {
            if (self.random(0x65, 10) == 0 || ts == 0x10 || ts == 8)
                && self.ai_dx_ahead(s, t) < 0x78
                && self.ai_dz(s, t) < 7
                && self.ai_facing_target(s, t)
            {
                return self.ai_seq(s, DDA);
            }
            if ts != 0x12 && ts != 0xe && ts != 0xc && 200 < mp(self) {
                let d = self.ai_dx(s, t);
                if 0x4b < d && d < 0x172 && self.ai_dz(s, t) < 0x3c && self.random(0x66, 0xd) == 0 {
                    return self.ai_seq_dir(s, t, DFJ_R, DFJ_L);
                }
            }
            if self.random(0x67, self.objs[t].hp() / 4 + 0x28) == 0 {
                let d = self.ai_dx(s, t);
                if 0x96 < d && d < 400 {
                    return self.ai_seq_dir(s, t, DFJ_R, DFJ_L);
                }
            }
            if near_dist < 10000 && self.random(0x68, 0x1e) == 0 && 0x96 < mp(self) {
                return self.ai_seq(s, DUA);
            }
        }

        if id(self) == 0x20 {
            let first = ts != 0x12
                && ts != 0xe
                && ts != 0xc
                && 200 < mp(self)
                && self.ai_dx(s, t) < 0x10e
                && self.ai_dz(s, t) < 0x3c
                && self.random(0x69, 0x3c) == 0;
            if first || (self.random(0x6a, self.objs[t].hp() / 4 + 0x28) == 0 && {
                let d = self.ai_dx(s, t);
                0x96 < d && d < 400
            }) {
                return self.ai_seq_dir(s, t, DFJ_R, DFJ_L);
            }
            if self.ai_dx(s, t) < 0x96 && self.ai_dz(s, t) < 0x28 && self.random(0x6b, 0xf) == 0 {
                return self.ai_seq_dir(s, t, DFA_R, DFA_L);
            }
        }

        if id(self) == 0x21
            && (self.random(0x6c, 5) == 0 || ts == 0x10 || ts == 8)
            && self.ai_dx_ahead(s, t) < 0x3c
            && self.ai_dz(s, t) < 7
            && 0x96 < mp(self)
            && self.ai_facing_target(s, t)
        {
            return self.ai_seq(s, DUA);
        }

        if id(self) == 0x22 {
            if self.random(0x6d, 10) == 0 && 0x15e < mp(self) && self.ai_low_hp(s, 0x46) {
                return self.ai_seq(s, DDJ);
            }
            if let Some(r) = self.ai_heal_teammate(s, near_dist, near_z) {
                return r;
            }
        }

        if id(self) == 0x32 {
            let mut dfa = false;
            if self.random(0x6e, 7) == 0 {
                let d = self.ai_dx(s, t);
                if !(499 < d || d < 0x5b || 3 < self.ai_dz(s, t) || mp(self) < 0x97) {
                    let tf = self.objs[t].frame();
                    if tf == 0x107 || tf == 0x108 {
                        self.objs[s].set_prev_attack(0);
                        self.objs[s].set_attack(1);
                    } else {
                        dfa = true;
                    }
                }
            }
            if dfa {
                return self.ai_seq_dir(s, t, DFA_R, DFA_L);
            }
            if self.random(0x6f, 7) == 0 && self.ai_dx(s, t) < 100 && self.ai_dz(s, t) < 7 && 0x4b < mp(self) {
                return self.ai_seq(s, DDA);
            }
        }

        if id(self) == 0x23 && self.random(0x70, 7) == 0 {
            let d = self.ai_dx(s, t);
            if d < 0x28a && 0x28 < d && self.ai_dz(s, t) < 4 && 0x78 < mp(self) {
                return self.ai_seq_dir(s, t, DFA_R, DFA_L);
            }
        }

        if id(self) == 0x24 {
            if 200 < mp(self) && self.random(0x71, 5) == 0 {
                let team = self.objs[s].team();
                for j in 0..100 {
                    if self.exists(j) && self.data_of(j).kind == 0 && self.objs[j].team() == team {
                        let p = &self.objs[j];
                        if p.hp() < p.dark_hp() - 200 || (p.hp() < 200 && p.hp() < p.dark_hp() - 100) {
                            return self.ai_seq(s, DUJ);
                        }
                    }
                }
                return 1;
            }
            if 0x104 < mp(self) && self.random(0x72, 10) == 0 && self.ai_dx(s, t) < 0x28a && self.ai_dz(s, t) < 0xf0 {
                return self.ai_seq(s, DUA);
            }
        }

        if id(self) == 0x26 {
            if 0x96 < mp(self) && self.random(0x73, 5) == 0 {
                let d = self.ai_dx(s, t);
                if d < 0xfa && 0x82 < d && self.ai_dz(s, t) < 10 {
                    return self.ai_seq_dir(s, t, DFJ_R, DFJ_L);
                }
            }
            if 200 < mp(self) && self.random(0x74, 10) == 0 && self.ai_dz(s, t) < 10 {
                return self.ai_seq_dir(s, t, DFA_R, DFA_L);
            }
            if 200 < mp(self) && self.random(0x75, 10) == 0 && (200 < self.ai_dx(s, t) || self.ai_dz(s, t) < 0xfa) {
                return self.ai_seq(s, DUJ);
            }
        }

        if id(self) == 0x27 {
            if 100 < mp(self)
                && self.random(0x76, 3) == 0
                && self.ai_dx(s, t) < 0x78
                && self.ai_facing_target(s, t)
                && self.ai_dz(s, t) < 10
            {
                return self.ai_seq(s, DDA);
            }
            if 100 < mp(self) && self.random(0x77, 7) == 0 && self.ai_dx(s, t) < 0xfa && self.ai_dz(s, t) < 10 {
                return self.ai_seq_dir(s, t, DFA_R, DFA_L);
            }
        }

        if id(self) == 0x34 {
            if ts == 3 && 0x7d < mp(self) && self.random(0x78, 10) == 0 && self.ai_dx(s, t) < 0x78 && self.ai_dz(s, t) < 10 {
                return self.ai_seq(s, DJA);
            }
            if 0x7d < mp(self) && self.random(0x7b, 5) == 0 && self.ai_dx(s, t) < 100 && self.ai_dz(s, t) < 0x1e {
                if self.objs[t].ix() <= self.objs[s].ix() {
                    return 1;
                }
                return self.ai_seq(s, DUJ);
            }
            if 0x7d < mp(self) && self.random(0x7c, 0xe) == 0 && self.ai_dx(s, t) < 700 && self.ai_dz(s, t) < 0x96 {
                return self.ai_seq_dir(s, t, DFA_R, DFA_L);
            }
            if 0x7d < mp(self) && self.random(0x7d, 5) == 0 && self.ai_dz(s, t) < 0x14 {
                return self.ai_seq_dir(s, t, DFJ_R, DFJ_L);
            }
            if (self.random(0x7e, 5) == 0 || ts == 0x10 || ts == 8)
                && self.ai_dx_ahead(s, t) < 100
                && self.ai_dz(s, t) < 7
                && mp(self) < 100
                && self.ai_facing_target(s, t)
            {
                return self.ai_seq(s, DUA);
            }
        }

        if id(self) != 0x33 {
            return 0;
        }
        let f = self.objs[s].frame();
        if 0x109 < f && f < 0x118 && (0xd < self.ai_dz(s, t) || self.data_of(t).kind != 0) {
            self.objs[s].set_prev_defend(0);
            self.objs[s].set_defend(1);
            return 1;
        }
        if 300 < mp(self) && self.random(0x7f, 10) == 0 && self.ai_dx(s, t) < 300 && self.ai_dz(s, t) < 200 {
            return self.ai_seq(s, DUJ);
        }
        if mp(self) < 0x12d || self.random(0x80, 10) != 0 || 0x3b5 < self.ai_dx(s, t) {
            if self.random(0x81, 5) == 0 && 0xfa < mp(self) {
                let d = self.ai_dx(s, t);
                if d < 0x4b0 && 0x28 < d && self.ai_dz(s, t) < 0xd {
                    return self.ai_seq_dir(s, t, DFJ_R, DFJ_L);
                }
            }
            return 0;
        }
        self.ai_seq(s, DUA)
    }

    /// The id 7 branch of 0x403a40 (0x404606..0x40498d).
    fn ai_specials_7(&mut self, s: usize, t: usize, ts: i32) -> Option<i32> {
        let defend = |w: &mut World| {
            w.objs[s].set_defend(1);
            w.objs[s].set_prev_defend(0);
            Some(1)
        };
        let f = self.objs[s].frame();
        if 0x10b < f && f < 0x11b {
            if ts == 0xc || ts == 0xb {
                return defend(self);
            }
            if 0x96 < self.ai_dx(s, t) || 0x19 < self.ai_dz(s, t) || self.ai_facing_target(s, t) {
                return defend(self);
            }
        }
        let mp = self.objs[s].mp();
        if ts != 0x12 && ts != 0xe {
            if ts != 0xc && 0x46 < self.objs[s].hp() && 0x140 < mp {
                let d = self.ai_dx(s, t);
                let dz = self.ai_dz(s, t);
                if (0x32 < d || 10 < dz)
                    && d < 0x55
                    && self.objs[t].hp() < self.objs[s].hp()
                    && dz < 0x23
                    && self.random(0x55, 5) == 0
                {
                    return Some(self.ai_seq(s, DUJ));
                }
            }
            if ts != 0xc && 200 < self.objs[s].mp() {
                let d = self.ai_dx(s, t);
                if 100 < d && d < 0x172 && self.ai_dz(s, t) < 0x3c && self.random(0x56, 0x14) == 0 {
                    return Some(self.ai_seq_dir(s, t, DFJ_R, DFJ_L));
                }
            }
        }
        if self.random(0x57, 100) == 0 {
            let d = self.ai_dx(s, t);
            if 0xf0 < d && d < 400 {
                return Some(self.ai_seq_dir(s, t, DFJ_R, DFJ_L));
            }
        }
        if ts != 0x12 && ts != 0xe && ts != 0xc && 200 < self.objs[s].mp() {
            let d = self.ai_dx(s, t);
            if 0x3c < d
                && d < 0x118
                && self.ai_dz(s, t) < 0x3c
                && self.random(0x58, 0xf) == 0
                && self.ai_facing_target(s, t)
            {
                return Some(self.ai_seq(s, DDJ));
            }
        }
        let o = &self.objs[s];
        if o.frame() < 0xff || 0x105 < o.frame() {
            return None;
        }
        let (tx, tz) = (self.objs[t].ix(), self.objs[t].iz());
        let limit = self.g.ai_x_limit();
        if (o.facing() == 0 && (tx + 0x78 < o.ix() || limit - 0x1e < o.ix()))
            || (o.facing() == 1 && (o.ix() < tx - 0x78 || o.ix() < 0x1e))
        {
            return defend(self);
        }
        if 0x46 < abs(tz - o.iz()) || self.g.ai_hold_back() == 1 {
            return defend(self);
        }
        let toward = self.ai_facing_target(s, t);
        let o = &mut self.objs[s];
        let up = if toward { tz <= o.iz() } else { o.iz() <= tz };
        if up {
            o.set_up(1);
        } else {
            o.set_down(1);
        }
        None
    }
}
