//! `ai_projectile` (0x406ba0): behaviour of non-character objects whose current frame
//! has `hit_Fa > 0`. The value selects the behaviour: 1, 2, 3, 4, 12 and 14 steer
//! towards a target (0x3f8), 5, 6, 8, 9, 11 and 13 spawn other objects and disappear,
//! 7 leaves a trail and steers, 10 speeds up.
//!
//! Which object uses which value (from the data files): 1 John's biscuit, 2 Dennis's
//! chase ball, 3 object 124 frame 40, 4/5 Jan's healing angel, 6/7 Jan's devil, 7/9/11
//! Firzen's chase balls and explosion, 8/12 bat chase, 13/14 Julian's balls.

use crate::world::{World, MAX_OBJECTS};

/// Parameters of one spawn made with `ai_projectile_aux` (0x406a20).
struct Spawn {
    id: i32,
    owner: i32,
    frame: i32,
    team: i32,
    pos: (i32, i32, i32),
    v: (f64, f64, f64),
    facing: i8,
}

impl World {
    /// Allocate a slot from 50 and spawn object `id` with the standard reset; returns
    /// the slot. Does not mark it existing.
    fn spawn_new(&mut self, id: i32) -> Option<usize> {
        let slot = self.free_slot()?;
        let data = self.data_index(id)?;
        self.spawn_reset(slot, data);
        Some(slot)
    }

    /// 0x406a20.
    fn spawn_aux(&mut self, s: Spawn) {
        let Some(slot) = self.spawn_new(s.id) else { return };
        let o = &mut self.objs[slot];
        o.m.set_i32(0x354, s.owner);
        o.set_ix(s.pos.0);
        o.set_iy(s.pos.1);
        o.set_iz(s.pos.2);
        o.set_team(s.team);
        o.set_x(s.pos.0 as f64);
        o.set_y(s.pos.1 as f64);
        o.set_z(s.pos.2 as f64);
        o.set_frame(s.frame);
        o.set_vx(s.v.0);
        o.set_vy(s.v.1);
        o.set_vz(s.v.2);
        o.set_facing(s.facing);
        self.set_exists(slot, true);
    }

    /// Copy owner, integer and double position and team from `src` to the new object.
    fn copy_place(&mut self, slot: usize, src: usize) {
        let (owner, ix, iy, iz, team, x, y, z) = {
            let p = &self.objs[src];
            (p.m.i32(0x354), p.ix(), p.iy(), p.iz(), p.team(), p.x(), p.y(), p.z())
        };
        let o = &mut self.objs[slot];
        o.m.set_i32(0x354, owner);
        o.set_ix(ix);
        o.set_iy(iy);
        o.set_iz(iz);
        o.set_team(team);
        o.set_x(x);
        o.set_y(y);
        o.set_z(z);
    }

    /// Living characters of other teams, in index order.
    fn enemies_of(&self, i: usize) -> Vec<i32> {
        let team = self.objs[i].team();
        (0..MAX_OBJECTS)
            .filter(|&j| self.exists(j) && self.data_of(j).kind == 0 && self.objs[j].team() != team && self.objs[j].hp() > 0)
            .map(|j| j as i32)
            .collect()
    }

    /// 0x406ba0.
    pub fn ai_projectile(&mut self, i: usize) {
        let kind = self.frame_of(i).hit_fa;
        match kind {
            // falls through into the target search (0x40704d), which sets hp to 0
            // when there is no target
            0xb => self.firzen_explosion(i),
            8 => return self.bat_spawn(i),
            0xd => return self.julian_spawn(i),
            5 => return self.heal_angels(i),
            6 | 9 => return self.chasers(i, kind),
            7 => self.trail_copy(i),
            10 => {
                let o = &mut self.objs[i];
                o.set_vx(if 0.0 <= o.vx() { o.vx() + 1.1 } else { o.vx() - 1.1 });
                if 30.0 < o.vx() {
                    o.set_vx(30.0);
                }
                if o.vx() < -30.0 {
                    o.set_vx(-30.0);
                }
                if 3.0 < o.y() {
                    o.set_y(3.0);
                }
                o.set_facing(if 0.0 < o.vx() { 0 } else { 1 });
                return;
            }
            _ => {}
        }
        if !self.choose_target(i, kind) {
            return;
        }
        let t = self.objs[i].m.i32(0x3f8);
        if !(0..MAX_OBJECTS as i32).contains(&t) {
            // the original reads objects[t] out of bounds here
            self.unported("ai_projectile with no target (0x407fcf)");
            return;
        }
        let t = t as usize;
        match kind {
            1 => self.steer_biscuit(i, t),
            4 => {
                let (o, p) = (&self.objs[i], &self.objs[t]);
                if p.hp() > 0
                    && p.ix() - 0x1e < o.ix()
                    && o.ix() < p.ix() + 0x1e
                    && p.iy() - 0x50 < o.iy()
                    && o.iy() < p.iy()
                    && p.iz() - 10 < o.iz()
                    && o.iz() < p.iz() + 10
                {
                    let o = &mut self.objs[i];
                    o.set_vx(0.0);
                    o.set_vy(0.0);
                    o.set_vz(0.0);
                    o.set_frame(0x3c);
                    self.objs[t].m.set_i32(0xe4, 100);
                    return;
                }
                self.steer_chaser(i, t, kind);
            }
            0xe | 0xc | 2 | 7 => self.steer_chaser(i, t, kind),
            3 => self.steer_kind3(i, t),
            _ => {}
        }
    }

    /// Keep or replace the target at 0x3f8 (0x40704d). Returns false when no target
    /// was found (the object's hp is set to 0).
    fn choose_target(&mut self, i: usize, kind: i32) -> bool {
        let thrower = self.objs[i].m.i32(0x2f8);
        let thrower_team = if thrower >= 0 && self.exists(thrower as usize) {
            self.objs[thrower as usize].team()
        } else {
            -1
        };
        if [4, 5, 6, 7].contains(&kind) {
            return true;
        }
        let my_team = self.objs[i].team();
        let t = self.objs[i].m.i32(0x3f8);
        let keep = t != -1 && self.exists(t as usize) && {
            let p = &self.objs[t as usize];
            p.hp() > 0
                && self.state(t as usize) != 0xe
                && p.u_08().wrapping_abs() <= 2
                && p.team() != my_team
                && thrower_team != p.team()
        };
        if keep {
            return true;
        }
        let mut best = 10000;
        for j in 0..MAX_OBJECTS {
            if j == i || !self.exists(j) || self.data_of(j).kind != 0 {
                continue;
            }
            let p = &self.objs[j];
            if p.team() == my_team || thrower_team == p.team() {
                continue;
            }
            let usable = (self.state(j) != 0xe && p.u_08().wrapping_abs() <= 2) || self.objs[i].m.i32(0x3f8) == -1;
            if !usable || p.hp() <= 0 {
                continue;
            }
            let o = &self.objs[i];
            let d = (p.iz() - o.iz()).wrapping_abs() + (p.ix() - o.ix()).wrapping_abs();
            if d < best {
                self.objs[i].m.set_i32(0x3f8, j as i32);
                best = d;
            }
        }
        if self.objs[i].m.i32(0x3f8) == -1 {
            self.objs[i].set_hp(0);
            return false;
        }
        true
    }

    /// Kind 1 (0x407fe2).
    fn steer_biscuit(&mut self, i: usize, t: usize) {
        let (tx, tz, ty, t_char) = {
            let p = &self.objs[t];
            (p.ix(), p.iz(), p.y(), self.data_of(t).kind == 0)
        };
        let o = &mut self.objs[i];
        if o.ix() < tx {
            o.set_vx(o.vx() + 0.85);
        }
        if tx < o.ix() {
            o.set_vx(o.vx() - 0.85);
        }
        if o.iz() + 7 < tz {
            o.set_vz(o.vz() + 0.3);
        }
        if tz < o.iz() - 7 {
            o.set_vz(o.vz() - 0.3);
        }
        o.set_vy(o.vy() / 1.4);
        if t_char {
            if o.y() + 10.0 < ty {
                o.set_y(o.y() + 1.2);
            }
            if ty < o.y() + 10.0 {
                o.set_y(o.y() - 1.2);
            }
        } else if 0.0 < o.y() {
            o.set_y(o.y() + 1.0);
        }
        if 13.0 < o.vx() {
            o.set_vx(13.0);
        }
        if o.vx() < -13.0 {
            o.set_vx(-13.0);
        }
        if 2.0 < o.vz() {
            o.set_vz(2.0);
        }
        if o.vz() < -2.0 {
            o.set_vz(-2.0);
        }
        if 1.0 < o.y() {
            o.set_y(1.0);
        }
        o.set_facing(if 0.0 < o.vx() { 0 } else { 1 });
    }

    /// Kinds 2, 4, 7, 12 and 14 (0x408263), and the straight flight when the object or
    /// its target is gone (0x40877f).
    fn steer_chaser(&mut self, i: usize, t: usize, kind: i32) {
        let alive = self.objs[i].hp() > 0 && self.exists(i);
        if !alive {
            return self.fly_straight(i, kind);
        }
        let (tx, tz, ty, t_char) = {
            let p = &self.objs[t];
            (p.ix(), p.iz(), p.y(), self.data_of(t).kind == 0)
        };
        let o = &mut self.objs[i];
        for _ in 0..if kind == 7 { 2 } else { 1 } {
            if o.ix() < tx {
                o.set_vx(o.vx() + 0.7);
            }
            if tx < o.ix() {
                o.set_vx(o.vx() - 0.7);
            }
        }
        if o.iz() + 5 < tz {
            o.set_vz(o.vz() + 0.4);
        }
        if tz < o.iz() - 5 {
            o.set_vz(o.vz() - 0.4);
        }
        if kind == 2 || kind == 4 || kind == 0xc {
            o.set_vy(o.vy() / 1.4);
            if t_char {
                if o.y() + 40.0 < ty {
                    o.set_y(o.y() + 1.0);
                }
                if ty < o.y() + 40.0 {
                    o.set_y(o.y() - 1.0);
                }
            } else if 0.0 < o.y() {
                o.set_y(o.y() + 1.0);
            }
        } else if kind == 7 {
            if o.vy() < 4.0 {
                o.set_vy(o.vy() + 0.4);
            }
            o.set_y(o.vy() + o.y());
            if -0x19 < o.iy() {
                o.set_frame(0x3c);
                o.set_vz(0.0);
                o.set_vx(0.0);
                o.set_vy(0.0);
            }
        }
        if 14.0 < o.vx() {
            o.set_vx(14.0);
        }
        if o.vx() < -14.0 {
            o.set_vx(-14.0);
        }
        if 1.4 < o.y() {
            o.set_y(1.4);
        }
        let (hi, lo) = if kind == 0xe { (1.5, -1.5) } else { (2.2, -2.2) };
        if hi < o.vz() {
            o.set_vz(hi);
        }
        if o.vz() < lo {
            o.set_vz(lo);
        }
        o.set_facing(if 0.0 < o.vx() { 0 } else { 1 });
        let speed = o.vx().abs();
        if kind == 2 {
            set_chase_frame(o, speed);
        } else if kind == 0xe {
            if speed < 8.0 {
                if o.frame() < 10 {
                    o.set_frame(o.frame() + 0x32);
                }
            } else if 0x28 < o.frame() {
                o.set_frame(o.frame() - 0x32);
            }
        }
    }

    /// 0x40877f.
    fn fly_straight(&mut self, i: usize, kind: i32) {
        let o = &mut self.objs[i];
        o.set_vx(if 0.0 <= o.vx() { o.vx() + 2.0 } else { o.vx() - 2.0 });
        if 17.0 < o.vx() {
            o.set_vx(17.0);
        }
        if o.vx() < -17.0 {
            o.set_vx(-17.0);
        }
        if [2, 4, 0xe, 0xc].contains(&kind) {
            if 1.4 < o.y() {
                o.set_y(1.4);
            }
        } else if kind == 7 {
            if o.vy() < 4.0 {
                o.set_vy(o.vy() + 0.4);
            }
            o.set_y(o.vy() + o.y());
            if -0x19 < o.iy() {
                o.set_frame(0x3c);
                o.set_iy(-0x19);
                o.set_vz(0.0);
                o.set_vx(0.0);
                o.set_vy(0.0);
            }
        }
        o.set_facing(if 0.0 < o.vx() { 0 } else { 1 });
        if kind == 2 {
            let speed = o.vx().abs();
            set_chase_frame(o, speed);
        }
    }

    /// Kind 3 (0x408677).
    fn steer_kind3(&mut self, i: usize, t: usize) {
        let (tx, tz) = (self.objs[t].ix(), self.objs[t].iz());
        let o = &mut self.objs[i];
        if o.ix() < tx {
            o.set_vx(o.vx() + 0.7);
        }
        if tx < o.ix() {
            o.set_vx(o.vx() - 0.7);
        }
        if o.iz() + 10 < tz {
            o.set_vz(o.vz() + 0.17);
        }
        if tz < o.iz() - 10 {
            o.set_vz(o.vz() - 0.17);
        }
        if 16.0 < o.vx() {
            o.set_vx(16.0);
        }
        if o.vx() < -16.0 {
            o.set_vx(-16.0);
        }
        if 2.4 < o.vz() {
            o.set_vz(2.4);
        }
        if o.vz() < -2.4 {
            o.set_vz(-2.4);
        }
    }

    /// Kind 11: Firzen's final explosion, 14 pieces in a fixed pattern.
    fn firzen_explosion(&mut self, i: usize) {
        let (owner, team, ix, iy, iz, vx, vy, vz, f) = {
            let o = &self.objs[i];
            (o.m.i32(0x354), o.team(), o.ix(), o.iy(), o.iz(), o.vx(), o.vy(), o.vz(), o.facing())
        };
        // (id, frame, dx, dy, dz, dvz, facing; facing -1 = the object's own)
        let parts: [(i32, i32, i32, i32, i32, f64, i8); 14] = [
            (0xd3, 0x6d, 0, 0, 0, 0.0, -1),
            (0xdd, 0x51, 0, -100, 0, 0.0, -1),
            (0xd4, 0x64, 80, -3, 0, -7.0, 0),
            (0xd4, 0x64, 100, -3, 0, 0.0, 0),
            (0xd4, 0x64, 80, -3, 0, 7.0, 0),
            (0xd4, 0x64, -80, -3, 0, -7.0, 1),
            (0xd4, 0x64, -100, -3, 0, 0.0, 1),
            (0xd4, 0x64, -80, -3, 0, 7.0, 1),
            (0xd3, 0x32, -30, -1, -5, 0.0, 1),
            (0xd3, 0x32, 30, -1, -5, 0.0, 1),
            (0xd3, 0x32, -30, -1, 2, 0.0, 0),
            (0xd3, 0x32, 30, -1, 2, 0.0, 0),
            (0xd3, 0x32, 0, -1, -9, 0.0, 1),
            (0xd3, 0x32, 0, -1, 6, 0.0, 0),
        ];
        for (id, frame, dx, dy, dz, dvz, face) in parts {
            self.spawn_aux(Spawn {
                id,
                owner,
                frame,
                team,
                pos: (ix + dx, iy + dy, iz + dz),
                v: (vx, vy, vz + dvz),
                facing: if face < 0 { f } else { face },
            });
        }
        self.set_exists(i, false);
    }

    /// Kind 8: the bat's chase balls (object 225), one per two enemies, at least three.
    fn bat_spawn(&mut self, i: usize) {
        let enemies = self.enemies_of(i);
        let n = enemies.len() as i32;
        let count = if 4 < n { (n - 3) / 2 + 3 } else { 3 };
        for _ in 0..count {
            let Some(slot) = self.free_slot() else { continue };
            let Some(data) = self.data_index(0xe1) else { continue };
            self.spawn_reset(slot, data);
            self.copy_place(slot, i);
            self.objs[slot].set_frame(0);
            let r = self.random(3, 0x15);
            self.objs[slot].set_vx((r - 0xb) as f64);
            let r = self.random(5, 0x18);
            self.objs[slot].set_vy(3.0 - r as f64 * 0.25);
            let r = self.random(6, 0x18);
            self.objs[slot].set_vz(3.0 - r as f64 * 0.25);
            let f = self.objs[i].facing();
            self.objs[slot].set_facing(f);
            let target = if n == 0 { i as i32 } else { enemies[self.random(7, n) as usize] };
            self.objs[slot].m.set_i32(0x3f8, target);
            self.set_exists(slot, true);
        }
        self.set_exists(i, false);
    }

    /// Kind 13: Julian's ball turns into object 228 with a target.
    fn julian_spawn(&mut self, i: usize) {
        let enemies = self.enemies_of(i);
        let n = enemies.len() as i32;
        if let (Some(slot), Some(data)) = (self.free_slot(), self.data_index(0xe4)) {
            self.spawn_reset(slot, data);
            let (owner, ix, iy) = (self.objs[i].m.i32(0x354), self.objs[i].ix(), self.objs[i].iy());
            self.objs[slot].m.set_i32(0x354, owner);
            self.objs[slot].set_ix(ix);
            let r = self.random(8, 7);
            self.objs[slot].set_iy(r - 3 + iy);
            let (iz, team, x, y, z, vx, vz, f) = {
                let p = &self.objs[i];
                (p.iz(), p.team(), p.x(), p.y(), p.z(), p.vx(), p.vz(), p.facing())
            };
            let o = &mut self.objs[slot];
            o.set_iz(iz);
            o.set_team(team);
            o.set_x(x);
            o.set_y(y);
            o.set_z(z);
            o.set_frame(0);
            o.set_vx(vx);
            let r = self.random(0xb, 0x18);
            let o = &mut self.objs[slot];
            o.set_vz((3.0 - r as f64 * 0.25) + vz);
            o.set_facing(f);
            let target = if n == 0 { i as i32 } else { enemies[self.random(0xc, n) as usize] };
            self.objs[slot].m.set_i32(0x3f8, target);
            self.set_exists(slot, true);
        }
        self.set_exists(i, false);
    }

    /// Kind 5: Jan's healing angels (object 219), one per living teammate.
    fn heal_angels(&mut self, i: usize) {
        let team = self.objs[i].team();
        for j in 0..MAX_OBJECTS {
            if !(self.exists(j) && self.data_of(j).kind == 0 && self.objs[j].team() == team && self.objs[j].hp() > 0) {
                continue;
            }
            let Some(slot) = self.spawn_new(0xdb) else { continue };
            self.copy_place(slot, i);
            let tx = self.objs[j].ix();
            let o = &mut self.objs[slot];
            o.set_frame(0);
            o.set_vx(((tx - o.ix()) / 0x32) as f64);
            o.set_vy(0.0);
            o.set_vz(0.0);
            o.set_facing(0);
            o.m.set_i32(0x3f8, j as i32);
            self.set_exists(slot, true);
        }
        self.set_exists(i, false);
    }

    /// Kinds 6 (Jan's devils, object 220, up to 7) and 9 (Firzen's chase balls 221/222,
    /// at least 4, up to 10), one per enemy and pass.
    fn chasers(&mut self, i: usize, kind: i32) {
        let min = if kind == 9 { 4 } else { 0 };
        let max = if kind == 6 { 7 } else { 10 };
        let mut count = 0;
        let mut pass = 0;
        let mut last_slot: Option<usize> = None;
        loop {
            for j in 0..MAX_OBJECTS {
                let ok = self.exists(j)
                    && self.data_of(j).kind == 0
                    && self.objs[j].team() != self.objs[i].team()
                    && self.objs[j].hp() > 0
                    && (count < min || pass == 0);
                if !ok {
                    continue;
                }
                count += 1;
                last_slot = self.free_slot();
                if let Some(slot) = last_slot {
                    let id = if kind == 6 { 0xdc } else { self.random(0xd, 2) + 0xdd };
                    if let Some(data) = self.data_index(id) {
                        self.spawn_reset(slot, data);
                        self.copy_place(slot, i);
                        self.objs[slot].set_frame(0);
                        if kind == 6 {
                            let tx = self.objs[j].ix();
                            let sx = self.objs[slot].ix();
                            self.objs[slot].set_vx(((tx - sx) / 0x32) as f64);
                            let r = self.random(0xe, 4);
                            self.objs[slot].set_vy((-4 - r) as f64);
                        } else {
                            let r = self.random(0xf, 0x15);
                            self.objs[slot].set_vx((r - 0xb) as f64);
                            let r = self.random(0x10, 0x28);
                            self.objs[slot].set_vy(-2.0 - r as f64 / 6.0);
                        }
                        let o = &mut self.objs[slot];
                        o.set_vz(0.0);
                        o.set_facing(0);
                        o.m.set_i32(0x3f8, j as i32);
                        self.set_exists(slot, true);
                    }
                }
                if max <= count {
                    break;
                }
            }
            pass += 1;
            if min <= count || count == 0 || last_slot.is_none() || max <= count {
                break;
            }
        }
        self.set_exists(i, false);
    }

    /// Kind 7: leave a copy at frame 40 where the object is.
    fn trail_copy(&mut self, i: usize) {
        let id = self.data_of(i).id;
        let Some(slot) = self.spawn_new(id) else { return };
        self.copy_place(slot, i);
        let o = &mut self.objs[slot];
        o.set_frame(0x28);
        o.set_vx(0.0);
        o.set_vy(0.0);
        o.set_vz(0.0);
        o.set_facing(0);
        self.set_exists(slot, true);
    }
}

/// Dennis's chase ball picks its frame from its speed.
fn set_chase_frame(o: &mut crate::fields::Obj, speed: f64) {
    let f = o.frame();
    if 14.0 < speed {
        if f != 5 && f != 6 {
            o.set_frame(5);
        }
    } else if 7.0 < speed {
        if f != 3 && f != 4 {
            o.set_frame(3);
        }
    } else if f != 1 && f != 2 {
        o.set_frame(1);
    }
}
