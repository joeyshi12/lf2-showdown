//! The per-object loop at the end of a tick (0x41f550..0x4214c6 in `match_frame`):
//! transformations, hp and mp regeneration, `obj_post_update`, falling to the floor
//! with no hp, opoint spawning, broken weapons, the team commands, ice and fire
//! particles.

use crate::world::{World, MAX_OBJECTS};

/// `dark_hp` loses a third of the damage: `trunc((floor(d*0x55555555 / 2^32) - d) / 2)`,
/// the compiler's division sequence kept as is.
pub fn dark_hp_change(d: i32) -> i32 {
    let q = ((d as i64 * 0x55555555i64) >> 32) as i32;
    let x = q.wrapping_sub(d);
    (x >> 1) - (x >> 31)
}

impl World {
    /// First free slot from 50 up, or `None`.
    pub fn free_slot(&self) -> Option<usize> {
        (50..MAX_OBJECTS).find(|&i| !self.exists(i))
    }

    /// Spawn object data `data` in `slot` with the standard reset, mark it existing.
    fn spawn_at(&mut self, slot: usize, data: usize) {
        self.spawn_reset(slot, data);
        self.set_exists(slot, true);
    }

    /// 0x41f550.
    pub(crate) fn post_loop(&mut self, mode: i32) {
        for i in 0..MAX_OBJECTS {
            if !self.exists(i) {
                continue;
            }
            self.post_one(i, mode);
        }
    }

    fn post_one(&mut self, i: usize, mode: i32) {
        let kind = self.data_of(i).kind;
        if kind == 0 && self.state(i) == 0x270b {
            if let Some(k) = self.data_index(0x32) {
                self.objs[i].m.set_i32(0x368, k as i32);
            }
            self.objs[i].set_frame(0);
        }
        let st = self.state(i);
        if (8000..9000).contains(&st) {
            if let Some(k) = self.data_index(st - 8000) {
                self.objs[i].m.set_i32(0x368, k as i32);
            }
            self.objs[i].set_frame(0);
            self.objs[i].m.set_i32(0x318, 0x8c);
        }
        if self.data_of(i).kind == 0 && self.state(i) == 0x270c && self.objs[i].u_88() == 1 {
            self.armour_pieces(i);
        }
        if self.data_of(i).kind == 0 {
            let m12 = self.g.tick_mod12();
            let o = &mut self.objs[i];
            if 0 < o.hp() && o.hp() < o.dark_hp() && m12 == 0 {
                o.add_hp(1);
            }
            if o.m.i32(0x320) < 0 && m12 == 0 {
                let def = o.m.i32(0x340);
                let d = if def > 0 { 900 / def } else { 9 };
                o.add_hp(-d);
                o.add_dark_hp(dark_hp_change(d));
                if o.hp() < 0 {
                    o.set_hp(0);
                }
                if o.dark_hp() < 0 {
                    o.set_dark_hp(0);
                }
                o.m.add_i32(0x34c, 9);
            }
            let m3 = self.g.tick_mod3();
            let id = self.data_of(i).id;
            let o = &mut self.objs[i];
            if (o.m.i32(0x2f4) == -1 || o.mp() < 0x96) && o.mp() < 500 && m3 == 0 && o.u_08() > -1 {
                let mut h = o.hp().min(500);
                if id == 0x33 || id == 0x34 {
                    h /= 2;
                }
                o.add_mp((500 - h) / 100 + 1);
            }
        }
        self.obj_post_update(i, mode);

        let f = self.objs[i].frame();
        if f / 100 == 0xb || f / 100 == 0xc {
            // frames 1100-1299: become invisible for (frame - 1100) ticks, with
            // everything this object created
            let v = 0x44c - f;
            for j in 0..MAX_OBJECTS {
                if self.exists(j) && self.objs[j].m.i32(0x2f4) == i as i32 {
                    self.objs[j].set_u_08(v);
                }
            }
            self.objs[i].set_u_08(v);
            self.objs[i].set_frame(0);
            return;
        }
        if !(0..400).contains(&f) {
            self.objs[i].set_frame(0);
            self.set_exists(i, false);
            return;
        }
        let kind = self.data_of(i).kind;
        {
            let o = &mut self.objs[i];
            let f = o.frame();
            let ko = kind == 0 && o.hp() < 1 && (f < 0xc || f == 0x6e || f == 0x6f);
            let still = kind == 0
                && o.iy() == 0
                && o.y() == 0.0
                && o.vy() == 0.0
                && o.m.f64(0x30) == 0.0
                && ((0xb3 < f && f < 0xbe && f != 0xb8) || (0xd3 < f && f < 0xd7));
            if ko || still {
                o.set_frame(0xba);
                o.set_vy(-3.0);
                o.m.set_f64(0x30, -3.0);
                o.set_y(-1.0);
                o.set_iy(-1);
            }
        }
        let op = self.frame_of(i).opoint.clone();
        let o = &self.objs[i];
        if op.kind < 1 || op.oid < 1 || o.u_88() != 0 || (o.m.i32(0xb4) != 0 && kind == 0) {
            if (kind == 1 || kind == 2 || kind == 4 || kind == 6) && o.weapon_hp() < 0 {
                self.objs[i].set_weapon_hp(0);
                self.weapon_debris(i);
                self.set_exists(i, false);
            } else if i < 10 && o.hp() > 0 && kind == 0 {
                self.team_command(i);
            }
        } else {
            self.spawn_opoint(i, &op);
        }
        // leaving the frozen state: ice shards
        let (s78, f78) = (self.state78(i), self.objs[i].m.i32(0x78));
        if (s78 == 0xd || f78 == 200) && self.state(i) != 0xd && self.objs[i].frame() != 200 {
            let x = self.objs[i].ix();
            self.play_sound(x, 0xf);
            if let Some(k) = self.data_index(999) {
                for n in 0..15 {
                    let Some(s) = self.free_slot() else { break };
                    self.spawn_at(s, k);
                    let (pix, piy, piz, pz, py, px, p28) = {
                        let p = &self.objs[i];
                        (p.ix(), p.iy(), p.iz(), p.z(), p.y(), p.x(), p.m.f64(0x28))
                    };
                    {
                        let o = &mut self.objs[s];
                        o.set_ix(pix);
                        o.set_iy(piy);
                        o.set_iz(piz);
                        o.set_z(pz);
                    }
                    let r = self.random(199, 0x1d);
                    self.objs[s].set_y(py - r as f64);
                    let r = self.random(200, 0x27) - 0x13;
                    self.objs[s].set_x(r as f64 + px);
                    let r = self.random(0xc9, 0x14);
                    self.objs[s].set_vy(-(r / 2) as f64 - 8.0);
                    let r = self.random(0xca, 0xb);
                    self.objs[s].set_vx(p28 * 0.5 + (r as f64 - 5.0));
                    let f = if n < 2 {
                        Some(0x78)
                    } else if n < 5 {
                        Some(0x82)
                    } else if n < 9 {
                        Some(0x7d)
                    } else if n < 0xf {
                        Some(0x87)
                    } else {
                        None
                    };
                    if let Some(f) = f {
                        self.objs[s].set_frame(f);
                    }
                }
            }
        }
        // burning: fire particles
        let s78 = self.state78(i);
        if s78 == 0x12 || s78 == 0x13 {
            let s = self.state(i);
            let n = if s == 0x12 || s == 0x13 {
                if self.random(0xcb, 4) == 0 {
                    1
                } else {
                    0
                }
            } else {
                7
            };
            for _ in 0..n {
                let Some(s) = self.free_slot() else { break };
                let Some(k) = self.data_index(999) else { break };
                self.spawn_at(s, k);
                let (pix, piy, piz, pz, py, px, pvx) = {
                    let p = &self.objs[i];
                    (p.ix(), p.iy(), p.iz(), p.z(), p.y(), p.x(), p.vx())
                };
                {
                    let o = &mut self.objs[s];
                    o.set_ix(pix);
                    o.set_iy(piy);
                    o.set_iz(piz);
                    o.set_z(pz);
                }
                let r = self.random(0xcc, 0x1d);
                self.objs[s].set_y(py - r as f64);
                let r = self.random(0xcd, 0x3b) - 0x1d;
                self.objs[s].set_x(r as f64 + px);
                self.objs[s].set_vy(-1.0);
                let r = self.random(0xce, 0xb);
                self.objs[s].set_vx((r as f64 - 5.0) + pvx);
                let r = self.random(0xcf, 1);
                self.objs[s].set_frame(r + 0x8c);
            }
        }
        let f = self.objs[i].frame();
        self.objs[i].m.set_i32(0x78, f);
    }

    /// State 9996 on the frame's second tick: five armour pieces fly off (ids 217, 218).
    fn armour_pieces(&mut self, i: usize) {
        let mut data = None;
        for n in 0..5 {
            let Some(s) = self.free_slot() else { break };
            let id = if n == 4 { 0xda } else { 0xd9 };
            if let Some(k) = self.data_index(id) {
                data = Some(k);
            }
            let Some(k) = data else { break };
            self.spawn_at(s, k);
            let (pix, piy, piz) = (self.objs[i].ix(), self.objs[i].iy(), self.objs[i].iz());
            let r = self.random(0x9b, 7);
            self.objs[s].set_ix(r - 3 + pix);
            let r = self.random(0x9c, 7);
            self.objs[s].set_iy(r - 9 + piy);
            {
                let o = &mut self.objs[s];
                o.set_iz(piz + 1);
                o.set_z(o.iz() as f64);
                o.set_y(o.iy() as f64);
                o.set_x(o.ix() as f64);
            }
            let r = self.random(0x9d, 0xf);
            self.objs[s].set_vy(-(r / 2) as f64 - 5.0);
            self.objs[s].m.set_i32(0xec, 6);
            if n == 0 || n == 2 {
                let r = self.random(0x9e, 2);
                self.objs[s].set_vz(r as f64 + 3.0);
            } else if n == 1 || n == 3 {
                let r = self.random(0x9f, 2);
                self.objs[s].set_vz(-3.0 - r as f64);
            } else {
                self.objs[s].set_vz(1.0);
            }
            if n < 2 {
                let r = self.random(0xa0, 3);
                self.objs[s].set_vx(-10.0 - r as f64);
            } else if n < 4 {
                let r = self.random(0xa1, 3);
                self.objs[s].set_vx(r as f64 + 10.0);
            } else {
                let r = self.random(0xa2, 7);
                self.objs[s].set_vx(r as f64 - 3.0);
            }
            let r = self.random(0xa4, 4);
            self.objs[s].set_frame(r);
            let r = self.random(0xa5, 2);
            self.objs[s].set_facing(r as i8);
        }
    }

    /// A weapon with hp below 0 breaks into pieces (object 999).
    fn weapon_debris(&mut self, i: usize) {
        let id = self.data_of(i).id;
        let mut n = 0;
        if id == 0x65 || id == 0xda {
            n = 7;
        }
        if id == 100 || id == 0xd5 || id == 0xd9 {
            n = 5;
        }
        if id == 0xc9 {
            n = 3;
        }
        if id == 0x96 {
            n = 0xd;
        }
        if id == 0x97 {
            n = 0xf;
        }
        if id == 0x78 || id == 0x7c {
            n = 3;
        }
        if id == 0x79 {
            n = 4;
        }
        if id == 0x7a || id == 0x7b {
            n = 9;
        }
        let snd = self.gd.weapon_sound[self.objs[i].m.i32(0x368) as usize][2];
        if snd > -1 {
            let x = self.objs[i].ix();
            self.play_sound_id(x, snd);
        }
        let Some(k) = self.data_index(999) else { return };
        for piece in 0..n {
            let Some(s) = self.free_slot() else { break };
            self.spawn_at(s, k);
            let (pix, piy, piz) = (self.objs[i].ix(), self.objs[i].iy(), self.objs[i].iz());
            let r = self.random(0xa6, 7);
            self.objs[s].set_ix(r - 3 + pix);
            let r = self.random(0xa7, 7);
            self.objs[s].set_iy(r - 3 + piy);
            {
                let o = &mut self.objs[s];
                o.set_iz(piz);
                o.set_z(o.iz() as f64);
                o.set_y(o.iy() as f64);
                o.set_x(o.ix() as f64);
            }
            if id == 0x96 || id == 0x97 || id == 0xd5 {
                let r = self.random(0xa8, 0x14);
                self.objs[s].set_vy(-(r / 2) as f64 - 8.0);
            }
            if [100, 0xc9, 0x65, 0x78, 0x79, 0x7a, 0x7b, 0x7c, 0xd9, 0xda].contains(&id) {
                let r = self.random(0xa9, 8);
                self.objs[s].set_vy(-(r / 2) as f64 - 6.0);
            }
            let r = self.random(0xaa, 0xb);
            self.objs[s].set_vx(r as f64 - 5.0);
            let set = |w: &mut World, site: i32, add: i32| {
                let r = w.random(site, 4);
                w.objs[s].set_frame(r + add);
            };
            match id {
                0x96 => set(self, if piece < 5 { 0xab } else { 0xac }, if piece < 5 { 0 } else { 4 }),
                100 => set(self, if piece < 2 { 0xad } else { 0xae }, if piece < 2 { 10 } else { 0xe }),
                0xd5 => set(self, if piece < 2 { 0xaf } else { 0xb0 }, if piece < 2 { 0x96 } else { 0x9a }),
                0x65 => {
                    if piece < 5 {
                        let a = self.random(0xb1, 2) * 4 + 0x14;
                        let b = self.random(0xb2, 4);
                        self.objs[s].set_frame(a + b);
                    } else {
                        set(self, 0xb3, 0x1e);
                    }
                }
                0x97 => {
                    if piece < 2 {
                        set(self, 0xb4, 0x28)
                    } else if piece < 5 {
                        set(self, 0xb5, 0x2c)
                    } else if piece < 8 {
                        set(self, 0xb6, 0x32)
                    } else {
                        set(self, 0xb7, 0x36)
                    }
                }
                0x78 => {
                    if piece < 2 {
                        set(self, 0xb8, 0x36)
                    } else if piece < 5 {
                        set(self, 0xb9, 0x1e)
                    }
                }
                0x7c => set(self, 0xba, 0xaa),
                0x79 => set(self, 0xbb, 0x3c),
                0x7a | 0x7b => {
                    let base = if id == 0x7a { (0xbc, 0x46, 0xbd, 0x50, 0xbe, 0xbf) } else { (0xc0, 0xa0, 0xc1, 0xa4, 0xc2, 0xc3) };
                    if piece < 1 {
                        set(self, base.0, base.1)
                    } else if piece < 3 {
                        set(self, base.2, base.3)
                    } else {
                        set(self, base.4, 0x4a);
                        let r = self.random(base.5, 0x12);
                        self.objs[s].set_vy(-(r / 2) as f64 - 4.0);
                    }
                }
                0xd9 | 0xda => set(self, 0xc4, 0xae),
                _ => {}
            }
        }
    }

    /// D J D J / D D D D / A D A in the key history (0x40c..0x418): commands for
    /// computer teammates, shown with an object 998.
    fn team_command(&mut self, i: usize) {
        let o = &self.objs[i];
        let h = |a: u32| o.m.i32(a);
        let mut cmd = 0;
        if h(0x40c) == 9 {
            if h(0x410) == 0 && h(0x414) == 9 && h(0x418) == 0 {
                cmd = 100;
            }
            if h(0x410) == 9 && h(0x414) == 9 && h(0x418) == 9 {
                cmd = 0x66;
            }
            if h(0x410) == 5 && h(0x414) == 9 && h(0x418) == 5 {
                cmd = 0x68;
            }
        }
        if cmd == 0 {
            return;
        }
        for a in [0x418u32, 0x414, 0x410, 0x40c, 0x408] {
            self.objs[i].m.set_i32(a, 0);
        }
        let (Some(s), Some(k)) = (self.free_slot(), self.data_index(0x3e6)) else { return };
        self.spawn_at(s, k);
        let (pix, piz, team) = (self.objs[i].ix(), self.objs[i].iz(), self.objs[i].team());
        {
            let o = &mut self.objs[s];
            o.set_ix(pix);
            o.set_iy(0);
            o.set_iz(piz);
            o.set_frame(cmd - 100);
            o.set_z(o.iz() as f64);
            o.set_y(o.iy() as f64);
            o.set_x(o.ix() as f64);
            o.set_vx(0.0);
            o.set_vy(0.0);
        }
        for j in 0..MAX_OBJECTS {
            if self.exists(j) && self.objs[j].hp() > 0 && self.data_of(j).kind == 0 && self.objs[j].team() == team {
                match cmd {
                    100 => {
                        let r = self.random(0xc5, 0x51);
                        let x = r - 0x28 + self.objs[s].ix();
                        self.objs[j].m.set_i32(0x3fc, x);
                        let r = self.random(0xc6, 0x51);
                        let z = r - 0x28 + self.objs[s].iz();
                        self.objs[j].m.set_i32(0x400, z);
                    }
                    0x66 => self.objs[j].m.set_i32(0x404, 1),
                    _ => self.objs[j].m.set_i32(0x404, 0),
                }
            }
        }
    }

    /// The frame's opoint, on the frame's first tick: create `oid` objects.
    fn spawn_opoint(&mut self, i: usize, op: &lf2_data::object::Opoint) {
        let (mut count, face_rule) = if op.facing > 10 { (op.facing / 10, op.facing % 10) } else { (1, op.facing) };
        if count < 0 {
            count = 0;
        }
        let mut made: Vec<usize> = Vec::new();
        for k in 0..count {
            let Some(s) = self.free_slot() else { break };
            let Some(d) = self.data_index(op.oid) else { break };
            made.push(s);
            self.spawn_reset(s, d);
            let owner354 = self.objs[i].m.i32(0x354);
            self.objs[s].m.set_i32(0x354, owner354);
            for j in 0..MAX_OBJECTS {
                if self.exists(j) {
                    self.objs[j].m.set_u8(0xf0 + s as u32, 0);
                }
            }
            self.set_exists(s, true);
            let fr = self.frame_of(i);
            let (cx, cy) = (fr.centerx, fr.centery);
            let (pix, piy, pf, team, pz) = {
                let p = &self.objs[i];
                (p.ix(), p.iy(), p.facing(), p.team(), p.z())
            };
            {
                let o = &mut self.objs[s];
                if pf == 0 {
                    o.set_ix((pix - cx) + op.x);
                } else {
                    o.set_ix((cx - op.x) + pix);
                }
                o.set_iy(piy - cy + op.y);
                o.set_team(team);
                o.set_z(pz + 1.0);
                o.set_y(o.iy() as f64);
                o.set_frame(op.action);
                o.set_vy(op.dvy as f64);
                o.set_vz(0.0);
            }
            let st = self.state(s);
            let sid = self.data_of(s).id;
            if (st == 3000 || st == 0x3ea || st == 0xbbe) && sid != 0xdf && sid != 0xe0 {
                let (up, down) = (self.objs[i].up(), self.objs[i].down());
                if up == 0 && down != 0 {
                    self.objs[s].set_vz(2.5);
                } else if up != 0 && down == 0 {
                    self.objs[s].set_vz(-2.5);
                }
                if sid == 0xd3 {
                    let v = self.objs[s].vz() * 0.25;
                    self.objs[s].set_vz(v);
                }
            }
            if self.data_of(s).kind == 0 {
                let p2f4 = self.objs[i].m.i32(0x2f4);
                let pu08 = self.objs[i].u_08();
                let o = &mut self.objs[s];
                o.m.set_i32(0x2f4, if p2f4 < 0 { i as i32 } else { p2f4 });
                o.set_u_08(pu08);
            }
            let pf = self.objs[i].facing();
            let o = &mut self.objs[s];
            o.set_facing(match face_rule {
                0 => pf,
                1 => 1 - pf,
                _ => 0,
            });
            o.set_vx(if o.facing() == 0 { op.dvx as f64 } else { -(op.dvx as f64) });
            o.set_x(o.ix() as f64);
            if count > 1 {
                let d = (k as f64 * 10.0) / (count as f64 - 1.0) - 5.0;
                o.set_vz(d + o.vz());
                let vx = o.vx();
                if vx > 0.0 && d > 0.0 {
                    o.set_vx(-d + vx);
                } else if vx < 0.0 && d < 0.0 {
                    o.set_vx(-d + vx);
                } else {
                    o.set_vx(d + vx);
                }
            }
            if self.data_of(i).kind == 3 && self.state(i) == 0xbbb {
                let w = self.objs[i].walk_counter() as usize;
                self.objs[w].m.set_u8(0xf0 + s as u32, 10);
                self.objs[s].m.set_u8(0xf0 + w as u32, 10);
            }
            let sid = self.data_of(s).id;
            let o = &mut self.objs[s];
            if sid == 5 || sid == 0x34 {
                o.set_hp(10);
                o.set_max_hp(10);
                o.set_dark_hp(10);
                o.set_mp(5);
            }
            o.set_ix(crate::mem::ftol(o.x()));
            o.set_iy(crate::mem::ftol(o.y()));
            o.set_iz(crate::mem::ftol(o.z()));
            if op.kind == 2 {
                let team = self.objs[i].team();
                self.objs[i].set_weapon_type(1);
                self.objs[s].set_weapon_type(-1);
                self.objs[i].set_weapon_held(s as i32);
                self.objs[s].set_weapon_holder(i as i32);
                self.objs[s].set_team(team);
            }
        }
        let n = made.len() as i32;
        if n > 1 {
            for (j, &s) in made.iter().enumerate() {
                let j = j as i32;
                let half = n / 2;
                if n % 2 == 0 {
                    if j < half - 1 {
                        self.objs[s].m.set_i32(0xec, (half - j) * 2 - 2);
                    } else if half < j {
                        self.objs[s].m.set_i32(0xec, (j - half) * 2);
                    }
                } else if half <= j {
                    if half < j {
                        self.objs[s].m.set_i32(0xec, (j - half) * 2);
                    }
                } else {
                    self.objs[s].m.set_i32(0xec, (half - j) * 2);
                }
                for &t in &made[..j as usize] {
                    self.objs[s].m.set_u8(0xf0 + t as u32, 0x28);
                    self.objs[t].m.set_u8(0xf0 + s as u32, 0x28);
                }
            }
        }
    }
}
