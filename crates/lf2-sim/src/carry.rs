//! `bound_and_carry` (0x417f80, called twice per tick): keep characters inside the
//! background's z range, and move held weapons to their holder's wpoint; throwing,
//! dropping and drinking happen here too.

use crate::mem::ftol;
use crate::world::{World, MAX_OBJECTS};

impl World {
    /// Background `n` of the file manager (`DAT_0044d024` is the current one).
    pub fn background(&self) -> &lf2_data::Background {
        self.gd.bg(self.g.background())
    }

    /// 0x417f80.
    pub fn bound_and_carry(&mut self) {
        let (zmin, zmax) = {
            let b = self.background();
            (b.zboundary.0 as f64, b.zboundary.1 as f64)
        };
        for i in 0..MAX_OBJECTS {
            if self.exists(i) && self.data_of(i).kind == 0 {
                let o = &mut self.objs[i];
                if o.z() < zmin {
                    o.set_z(zmin);
                }
                if zmax < o.z() {
                    o.set_z(zmax);
                }
                o.set_iz(ftol(o.z()));
            }
        }
        for i in 0..MAX_OBJECTS {
            if !self.exists(i) || self.objs[i].weapon_type() >= 0 {
                continue;
            }
            let h = self.objs[i].weapon_holder() as u32;
            if !(h < 400 && self.exists(h as usize) && self.objs[h as usize].weapon_held() == i as i32) {
                self.objs[i].set_weapon_type(0);
                continue;
            }
            let h = h as usize;
            if self.state(h) == 0x11 && self.drink(i, h) {
                continue;
            }
            let wp = self.frame_of(h).wpoint.clone();
            let (hf, hix, hiy, hiz, hb4) = {
                let o = &self.objs[h];
                (o.facing(), o.ix(), o.iy(), o.iz(), o.m.i32(0xb4))
            };
            let hfr = self.frame_of(h);
            let (hcx, hcy) = (hfr.centerx, hfr.centery);
            {
                let o = &mut self.objs[i];
                o.set_frame(wp.weaponact);
                o.set_facing(hf);
                o.m.set_i32(0xb4, hb4);
            }
            let wx = if hf == 0 { (hix - hcx) + wp.x } else { (hcx - wp.x) + hix };
            let wy = (hiy - hcy) + wp.y;
            let wfr = self.frame_of(i);
            let (cx, cy, gx, gy) = (wfr.centerx, wfr.centery, wfr.wpoint.x, wfr.wpoint.y);
            let o = &mut self.objs[i];
            if o.facing() == 0 {
                o.set_ix((cx - gx) + wx);
            } else {
                o.set_ix((gx - cx) + wx);
            }
            o.set_iy((cy - gy) + wy);
            o.set_iz(hiz);
            if wp.cover == 0 {
                o.add_iz(1);
                o.add_iy(-1);
            } else {
                o.add_iz(-1);
                o.add_iy(1);
            }
            o.set_z(o.iz() as f64);
            o.set_x(o.ix() as f64);
            o.set_y(o.iy() as f64);
            let hs = self.state(h);
            if hs == 0xc || hs == 10 {
                // the holder is hit: drop it
                self.objs[h].set_weapon_type(0);
                self.objs[i].set_weapon_type(0);
                let r = self.random(0x8a, 0x10);
                let (n, acx, acy, hvx, hvy) = {
                    let p = &self.objs[h];
                    (p.m.i32(0x20), p.m.f64(0x28), p.m.f64(0x30), p.vx(), p.vy())
                };
                let o = &mut self.objs[i];
                o.set_frame(r);
                if n == 1 {
                    o.set_vy(acy);
                    o.set_vx(acx / 3.0);
                } else {
                    o.set_vy(hvy);
                    o.set_vx(hvx / 3.0);
                }
                if -2.0 < o.y() {
                    o.set_y(-2.0);
                }
            }
            if wp.dvx != 0 {
                let (up, down) = (self.objs[h].up(), self.objs[h].down());
                let t = self.data_of(i).kind;
                let vx = if hf == 0 { wp.dvx } else { -wp.dvx };
                let vz = if up != 0 && down == 0 {
                    Some(-wp.dvz)
                } else if up == 0 && down != 0 {
                    Some(wp.dvz)
                } else {
                    None
                };
                if t == 1 || t == 4 || t == 6 {
                    let o = &mut self.objs[i];
                    o.m.set_i32(0x2f8, h as i32);
                    o.set_frame(0x28);
                    o.set_vx(vx as f64);
                    o.set_vy(wp.dvy as f64);
                    self.objs[h].set_weapon_type(0);
                    self.objs[i].set_weapon_type(0);
                    if let Some(z) = vz {
                        self.objs[i].set_vz(z as f64);
                    }
                }
                if t == 2 {
                    let r = self.random(0x8b, 6);
                    let o = &mut self.objs[i];
                    o.set_frame(r);
                    o.set_vx(vx as f64);
                    o.set_vy(wp.dvy as f64);
                    self.objs[h].set_weapon_type(0);
                    self.objs[i].set_weapon_type(0);
                    if let Some(z) = vz {
                        self.objs[i].set_vz(z as f64);
                    }
                }
            }
            if wp.kind == 3 {
                self.objs[i].set_weapon_type(0);
                self.objs[h].set_weapon_type(0);
                let f = self.random(0x8c, 6);
                let vx = self.random(0x8d, 7) - 3;
                let vy = -self.random(0x8e, 4);
                let vz = self.random(0x8f, 5) - 2;
                let o = &mut self.objs[i];
                o.set_frame(f);
                o.set_vx(vx as f64);
                o.set_vy(vy as f64);
                o.set_vz(vz as f64 / 5.0);
            }
        }
    }

    /// Drinking milk (0x7a) or beer (0x7b) while the holder is in state 17. Returns true
    /// when the bottle is empty and was let go (the caller then skips the wpoint step).
    fn drink(&mut self, i: usize, h: usize) -> bool {
        let id = self.data_of(i).id;
        if id == 0x7a && self.objs[i].hp() > 0 {
            self.objs[i].add_hp(-1);
            let left = self.objs[i].hp();
            if left % 5 == 0 {
                let p = &mut self.objs[h];
                p.add_dark_hp(2);
                p.add_hp(4);
                if p.max_hp() < p.dark_hp() {
                    p.set_dark_hp(p.max_hp());
                }
                if p.dark_hp() < p.hp() {
                    p.set_hp(p.dark_hp());
                }
            }
            if left % 6 == 0 {
                let p = &mut self.objs[h];
                p.add_mp(5);
                if 500 < p.mp() {
                    p.set_mp(500);
                }
            }
            if self.objs[i].hp() < 1 {
                self.release_bottle(i, h, 0x88);
                return true;
            }
        }
        let id = self.data_of(i).id;
        if id == 0x7b && self.objs[i].hp() > 0 {
            self.objs[i].add_hp(-2);
            let p = &mut self.objs[h];
            p.add_mp(3);
            if 500 < p.mp() {
                p.set_mp(500);
            }
            // checks the bottle's own 0x2f4 and 0x308, as the original does
            if self.objs[i].m.i32(0x2f4) > -1 && 0x96 < self.objs[i].mp() {
                self.objs[h].set_mp(0x96);
            }
            if self.objs[i].hp() < 1 {
                self.release_bottle(i, h, 0x89);
                return true;
            }
        }
        false
    }

    fn release_bottle(&mut self, i: usize, h: usize, site: i32) {
        self.objs[h].set_weapon_type(0);
        self.objs[i].set_weapon_type(0);
        self.objs[h].set_weapon_held(0);
        self.objs[i].set_weapon_holder(0);
        self.objs[i].set_frame(0);
        self.objs[i].set_vy(-8.0);
        let r = self.random(site, 7);
        self.objs[i].set_vx((r - 3) as f64);
        self.objs[h].set_frame(0);
        self.objs[i].set_weapon_hp(0);
    }
}
