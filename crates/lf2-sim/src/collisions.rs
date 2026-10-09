//! Collision detection between objects: which itr of an attacker touches which bdy of
//! a victim this tick (`collisions` 0x419380, `itr_touches_bdy` 0x417200,
//! `register_hit` 0x417400), plus the fusion check that runs right after
//! (`after_collisions` 0x4064d0) and the knockback application (0x4196f0).
//!
//! An attacker's hits are collected in its hit list: victims at 0x280 (int[20]), itr
//! index at 0x2d0 (u8[20]), count at 0x2e4. `obj_update` applies them later.

use crate::fields::Obj;
use crate::world::{World, MAX_OBJECTS};

/// 0x4171c0.
pub fn rects_overlap(x1: i32, y1: i32, w1: i32, h1: i32, x2: i32, y2: i32, w2: i32, h2: i32) -> bool {
    x2.wrapping_sub(x1) < w1 && x1.wrapping_sub(x2) < w2 && y2.wrapping_sub(y1) < h1 && y1.wrapping_sub(y2) < h2
}

fn is_weapon_type(t: i32) -> bool {
    t == 1 || t == 2 || t == 4 || t == 6
}

impl World {
    #[inline]
    pub fn o(&self, i: usize) -> &Obj {
        &self.objs[i]
    }
    #[inline]
    pub fn om(&mut self, i: usize) -> &mut Obj {
        &mut self.objs[i]
    }
    /// Frame `f` of object `i`'s data.
    #[inline]
    pub fn fr(&self, i: usize, f: i32) -> &lf2_data::Frame {
        &self.data_of(i).frames[f as usize]
    }
    /// State of object `i`'s current frame.
    #[inline]
    pub fn state(&self, i: usize) -> i32 {
        self.frame_of(i).state
    }
    /// State of the frame object `i` had at collision time (0x7c).
    #[inline]
    pub fn state7c(&self, i: usize) -> i32 {
        self.fr(i, self.objs[i].m.i32(0x7c)).state
    }
    /// State of the frame object `i` had at the end of the previous tick (0x78).
    #[inline]
    pub fn state78(&self, i: usize) -> i32 {
        self.fr(i, self.objs[i].m.i32(0x78)).state
    }

    /// 0x419380.
    pub fn collisions(&mut self, mode: i32) {
        for i in 0..MAX_OBJECTS {
            if !self.exists(i) {
                continue;
            }
            let f = self.objs[i].frame();
            self.objs[i].m.set_i32(0x7c, f);
            let fr = self.fr(i, f);
            let mut clear = fr.itrs.is_empty();
            if !clear && fr.state == 0x3e9 {
                let h = self.objs[i].weapon_holder() as usize;
                clear = self.frame_of(h).wpoint.attacking == 0;
            }
            if clear {
                self.objs[i].m.set_i32(0xec, 0);
            }
        }
        for a in 0..MAX_OBJECTS {
            if !self.exists(a) {
                continue;
            }
            for b in a + 1..MAX_OBJECTS {
                if !self.exists(b) {
                    continue;
                }
                let x = self.objs[a].m.i8(0xf0 + b as u32);
                if x > 0 {
                    self.objs[a].m.set_i8(0xf0 + b as u32, x - 1);
                }
                let x = self.objs[b].m.i8(0xf0 + a as u32);
                if x > 0 {
                    self.objs[b].m.set_i8(0xf0 + a as u32, x - 1);
                }
                if self.itr_touches_bdy(a, b) {
                    self.register_hit(a, b, mode);
                }
                if self.itr_touches_bdy(b, a) {
                    self.register_hit(b, a, mode);
                }
            }
        }
        self.after_collisions();
    }

    /// World rectangle of a box given in frame coordinates, for an object facing
    /// `facing` at (ix, iy) with frame centre (cx, cy).
    fn box_world(o: &Obj, cx: i32, cy: i32, x: i32, y: i32, w: i32) -> (i32, i32) {
        let wx = if o.facing() == 0 { x + (o.ix() - cx) } else { ((cx + o.ix()) - w) - x };
        (wx, y + (o.iy() - cy))
    }

    /// 0x417200: could `a` hit `b` at all (bounding boxes of all itrs against all bdys).
    pub fn itr_touches_bdy(&self, a: usize, b: usize) -> bool {
        let (oa, ob) = (&self.objs[a], &self.objs[b]);
        if self.frame_of(a).itrs.is_empty() || self.frame_of(b).bdys.is_empty() {
            return false;
        }
        if oa.m.i32(0xec) >= 1 || ob.m.i8(0xf0 + a as u32) >= 1 {
            return false;
        }
        let fa = self.fr(a, oa.m.i32(0x7c));
        let fb = self.fr(b, ob.m.i32(0x7c));
        let bb = fb.bdy_box;
        let (bx, by) = Self::box_world(ob, fb.centerx, fb.centery, bb.x, bb.y, bb.w);
        let ib = fa.itr_box;
        let (ax, ay) = Self::box_world(oa, fa.centerx, fa.centery, ib.x, ib.y, ib.w);
        rects_overlap(bx, by, bb.w, bb.h, ax, ay, ib.w, ib.h)
    }

    /// 0x417400: test each itr of `a` against each bdy of `b` and add hits to `a`'s list.
    pub fn register_hit(&mut self, a: usize, b: usize, mode: i32) {
        let gd = self.gd.clone();
        let da = &gd.objects[self.objs[a].m.i32(0x368) as usize];
        let db = &gd.objects[self.objs[b].m.i32(0x368) as usize];
        let fa = &da.frames[self.objs[a].m.i32(0x7c) as usize];
        let fb = &db.frames[self.objs[b].m.i32(0x7c) as usize];
        let a_id = da.id;
        for (itr_idx, it) in fa.itrs.iter().enumerate() {
            for bd in fb.bdys.iter() {
                let kind = it.kind;
                let eff = it.effect;
                let (b_type, b_id) = (db.kind, db.id);
                let st78b = self.state78(b);
                let st78a = self.state78(a);
                let bf = self.objs[b].frame();
                // victims an itr never touches: stop testing this itr
                let skip = ([200, 0xcb, 0xcd, 0xce, 0xcf, 0xd7, 0xd8].contains(&a_id) && b_id == 0xd1 && kind != 9)
                    || (kind == 3 && b_type != 0)
                    || (kind == 0
                        && ((eff == 4 && b_type == 0)
                            || (eff == 0x14 && (b_type != 0 || st78b == 0x12 || st78b == 0x13))
                            || (eff == 0x15 && (st78b == 0x12 || st78b == 0x13))
                            || (eff == 0x1e && 199 < bf && bf < 0xcb)
                            || (eff == 2 && st78a == 0x13 && st78b == 0x12)))
                    || (kind == 8 && b_type != 0)
                    || (self.objs[b].u_08() != 0 && kind != 8 && kind != 0xe);
                if skip {
                    break;
                }
                let st_b = self.state(b);
                let id_212_ok = b_id != 0xd4 || (a_id == 0xd4 && (bf % 10 != 5 || self.objs[a].frame() % 10 != 0));
                if (kind < 4 || kind == 6 || (kind == 9 && b_type == 0) || kind == 10 || kind == 11 || kind == 0xf || kind == 0x10)
                    && st_b != 0xd
                    && st_b != 10
                    && id_212_ok
                {
                    let t = self.objs[a].team();
                    if t == self.objs[b].team()
                        && t != 0
                        && kind != 8
                        && (self.state(a) != 0x12 || eff == 0x15 || eff == 0x16)
                        && (da.kind != 0 || db.kind != 3 || self.objs[a].facing() == self.objs[b].facing())
                        && !is_weapon_type(db.kind)
                    {
                        break; // same team
                    }
                }
                if kind == 5 {
                    let t = self.objs[self.objs[a].weapon_holder() as usize].team();
                    if t == self.objs[b].team() && t != 0 && st_b != 0xd && !is_weapon_type(db.kind) && id_212_ok {
                        break;
                    }
                }
                // exact test of this itr against this bdy
                let ob = &self.objs[b];
                let (bx, by) = Self::box_world(ob, fb.centerx, fb.centery, bd.x, bd.y, bd.w);
                let oa = &self.objs[a];
                let (ax, ay) = Self::box_world(oa, fa.centerx, fa.centery, it.x, it.y, it.w);
                let zw = if it.zwidth == 0 { 0xf } else { it.zwidth };
                let dz = oa.iz() - ob.iz();
                let z_ok = -zw < dz && dz < zw;
                if !(rects_overlap(bx, by, bd.w, bd.h, ax, ay, it.w, it.h) && z_ok) {
                    continue;
                }
                // falling victims ignore weak hits; in stage mode bdy kinds >= 1000 only
                // take hits from characters
                let mut ignore = st_b == 0xc && it.fall < 0x29 && kind != 10 && kind != 0xb;
                if mode == 1 {
                    let mut from_player = (da.kind == 0 || a_id == 0xc9 || a_id == 0xca) && self.objs[a].team() != 5;
                    if self.objs[a].weapon_type() < 0 {
                        let h = self.objs[a].weapon_holder() as usize;
                        if self.exists(h) && self.data_of(h).kind == 0 && self.objs[h].team() != 5 {
                            from_player = true;
                        }
                    }
                    if bd.kind > 999 && !from_player {
                        ignore = true;
                    }
                }
                let st7c_b = self.state7c(b);
                let multi = it.vrest != 0
                    || kind == 1
                    || kind == 2
                    || kind == 7
                    || (st7c_b == 0x3ec && (da.kind < 1 || self.objs[a].weapon_type() < 0))
                    || ignore;
                if multi {
                    if self.objs[a].m.i32(0x2e4) > 0x13 || ignore {
                        continue;
                    }
                    let mut r = 0;
                    if st7c_b == 0x3ec
                        && (da.kind < 1 || self.objs[a].weapon_type() < 0)
                        && kind != 2
                        && kind != 7
                        && kind != 10
                    {
                        r = 2;
                    }
                    if kind == 1 {
                        let dist = (self.objs[a].ix() - self.objs[b].ix()).wrapping_abs();
                        let best = self.objs[b].m.i32(0x2ec);
                        if dist < best || (dist == best && self.random(0x86, 2) == 0) {
                            self.objs[b].m.set_i32(0x2ec, dist);
                        } else {
                            r = 2;
                        }
                    }
                    if kind == 4 && self.objs[a].m.i32(0x320) != 0 && r != 2 {
                        r = 1;
                    }
                    let (oa, ob) = (&self.objs[a], &self.objs[b]);
                    if kind == 1 {
                        if ((oa.right() != 0 && oa.ix() < ob.ix()) || (oa.left() != 0 && ob.ix() <= oa.ix()))
                            && self.state(b) == 0x10
                            && r != 2
                        {
                            r = 1;
                        }
                    } else if kind == 2 {
                        let st_b = self.state(b);
                        let mut go_on = true;
                        if oa.weapon_type() == 0 {
                            if oa.attack() == 0 {
                                go_on = false;
                            } else if oa.prev_attack() == 0 && st_b == 0x3ec && r != 2 {
                                r = 1;
                            }
                        }
                        if go_on && oa.attack() != 0 && oa.prev_attack() == 0 && st_b == 0x7d4 && r != 2 {
                            r = 1;
                        }
                    } else if kind != 7 && r != 2 {
                        r = 1;
                    }
                    let oa = &self.objs[a];
                    if (kind == 7 && oa.attack() != 0 && oa.prev_attack() == 0 && self.state(b) == 0x3ec && r != 2) || r == 1 {
                        let n = oa.m.i32(0x2e4);
                        let o = &mut self.objs[a];
                        o.m.set_i32(0x280 + 4 * n as u32, b as i32);
                        o.m.set_u8(0x2d0 + n as u32, itr_idx as u8);
                        o.m.set_i32(0x2e4, n + 1);
                    }
                } else {
                    // single-target itr: keep only the nearest victim
                    let oa = &self.objs[a];
                    let bx = self.objs[b].ix();
                    let dist = if oa.weapon_type() < 0 {
                        let h = oa.weapon_holder();
                        if b as i32 != h {
                            (self.objs[h as usize].ix() - bx).wrapping_abs()
                        } else {
                            2000
                        }
                    } else {
                        (oa.ix() - bx).wrapping_abs()
                    };
                    let best = self.objs[a].m.i32(0x2e8);
                    if dist < best || (dist == best && self.random(0x85, 2) == 0) {
                        let o = &mut self.objs[a];
                        o.m.set_i32(0x2e8, dist);
                        o.m.set_i32(0x280, b as i32);
                        o.m.set_u8(0x2d0, itr_idx as u8);
                        o.m.set_i32(0x2e4, 1);
                    }
                }
            }
        }
    }

    /// Prepare slot `slot` as a new object of file-manager index `data` (the inline
    /// sequence after `obj_reset` used by every spawn): position (580, -200, 300).
    pub fn spawn_reset(&mut self, slot: usize, data: usize) {
        let hp = self.gd.objects[data].weapon_hp;
        let o = &mut self.objs[slot];
        obj_reset(o);
        o.m.set_i32(0x368, data as i32);
        o.set_x(580.0);
        o.set_weapon_hp(hp);
        o.set_y(-200.0);
        o.set_z(300.0);
    }

    /// File-manager index of the object data with this id.
    pub fn data_index(&self, id: i32) -> Option<usize> {
        self.gd.objects.iter().position(|d| d.id == id)
    }

    /// 0x4064d0: Firen and Freeze running into each other become Firzen, and Firzen
    /// splits again when its time is up. Only objects 0..19 take part.
    fn after_collisions(&mut self) {
        for i in 0..20usize {
            if self.objs[i].m.i32(0x338) > 0 {
                self.objs[i].m.add_i32(0x338, -1);
            }
            if !self.exists(i) {
                continue;
            }
            let id = self.data_of(i).id;
            let o = &self.objs[i];
            let fusing = (id == 7 || id == 8)
                && o.hp() > 0
                && self.state(i) == 2
                && o.m.i32(0x338) == 0
                && (o.hp() < 0xb1 || self.g.u_458428() == 1);
            if fusing {
                for j in 0..20usize {
                    if !self.exists(j) {
                        continue;
                    }
                    let p = &self.objs[j];
                    let php = p.hp();
                    let pf = p.frame();
                    let ok = self.data_of(j).id == 0xf - id
                        && php > 0
                        && self.objs[i].team() == p.team()
                        && p.m.i32(0x338) == 0
                        && (php < 0xb1 || self.g.u_458428() == 1)
                        && (0..400).contains(&pf)
                        && {
                            let s = self.fr(j, pf).state;
                            s == 2 || (s != 0xe && p.iy() == 0 && 9 < j)
                        };
                    if !ok {
                        continue;
                    }
                    let (sx, px) = (self.objs[i].ix(), self.objs[j].ix());
                    if (sx - px).wrapping_abs() < 0x32
                        && (self.objs[i].iz() - self.objs[j].iz()).wrapping_abs() < 8
                        && (px < sx || 9 < j)
                        && i < 10
                    {
                        if let Some(fz) = self.data_index(0x33) {
                            let (pdh, pix, piz) = (self.objs[j].dark_hp(), self.objs[j].ix(), self.objs[j].iz());
                            let pid = self.data_of(j).id;
                            let my_id = self.data_of(i).id;
                            let o = &mut self.objs[i];
                            o.add_hp(php);
                            o.add_dark_hp(pdh);
                            if o.max_hp() < o.dark_hp() {
                                o.set_dark_hp(o.max_hp());
                            }
                            if o.dark_hp() < o.hp() {
                                o.set_hp(o.dark_hp());
                            }
                            o.set_frame(0x122);
                            o.m.set_i32(0x328, 1);
                            o.set_vx(0.0);
                            o.set_ix((pix + o.ix()) / 2);
                            o.set_iz((piz + o.iz()) / 2);
                            o.set_x(o.ix() as f64);
                            o.set_z(o.iz() as f64);
                            o.m.set_i32(0x32c, j as i32);
                            o.m.set_i32(0x338, 0x1194);
                            o.m.set_i32(0x330, my_id);
                            o.m.set_i32(0x334, pid);
                            o.m.set_i32(0x368, fz as i32);
                            o.set_mp(500);
                            self.objs[j].set_vy(0.0);
                            self.set_exists(j, false);
                        }
                    }
                }
            } else {
                let o = &self.objs[i];
                if id == 0x33 && o.m.i32(0x328) == 1 && (o.frame() < 9 || 0x104 < o.frame()) && o.m.i32(0x338) < 1 {
                    let other = o.m.i32(0x32c) as usize;
                    let (id_a, id_b) = (o.m.i32(0x330), o.m.i32(0x334));
                    self.objs[i].m.set_i32(0x328, -1);
                    self.objs[i].m.set_i32(0x338, 900);
                    for k in 0..self.gd.objects.len() {
                        let did = self.gd.objects[k].id;
                        if did == id_a {
                            self.objs[i].m.set_i32(0x368, k as i32);
                        } else if did == id_b {
                            self.spawn_reset(other, k);
                            self.set_exists(other, true);
                        }
                    }
                    let (hp, dhp, ix, iz, face, team) = {
                        let o = &self.objs[i];
                        (o.hp(), o.dark_hp(), o.ix(), o.iz(), o.facing(), o.team())
                    };
                    let p = &mut self.objs[other];
                    p.set_hp(hp / 2);
                    p.set_dark_hp(dhp / 2);
                    p.set_ix(ix);
                    p.set_iz(iz);
                    p.set_iy(0);
                    p.set_x(ix as f64);
                    p.set_z(iz as f64);
                    p.set_y(0.0);
                    p.set_vx(0.0);
                    p.set_facing(1 - face);
                    p.set_frame(0x70);
                    p.set_mp(0);
                    p.set_team(team);
                    let o = &mut self.objs[i];
                    o.set_hp(hp / 2);
                    o.set_dark_hp(dhp / 2);
                    o.set_vx(0.0);
                    o.set_frame(0x70);
                    o.set_mp(0);
                }
            }
        }
    }

    /// 0x4196f0: turn the knockback collected this tick (0x28/0x30/0x38, from 0x20
    /// hits) into velocity, for objects not in hit pause.
    pub fn apply_knockback(&mut self) {
        for i in 0..MAX_OBJECTS {
            if !self.exists(i) || self.objs[i].m.i32(0xb4) != 0 {
                continue;
            }
            let o = &mut self.objs[i];
            let n = o.m.i32(0x20);
            if n != 0 {
                let c = KNOCKBACK_SCALE;
                let div = (n + 1) as f64;
                o.set_vx((o.m.f64(0x28) * c) / div);
                o.set_vy((o.m.f64(0x30) * c) / div);
                o.set_vz((o.m.f64(0x38) * c) / div);
                o.m.set_i32(0x20, 0);
            }
            o.m.set_f64(0x28, 0.0);
            o.m.set_f64(0x30, 0.0);
            o.m.set_f64(0x38, 0.0);
        }
    }
}

/// `_DAT_004479e0`, the factor in 0x4196f0.
pub const KNOCKBACK_SCALE: f64 = 2.0;

/// 0x4061d0: clear an object for reuse.
pub fn obj_reset(o: &mut Obj) {
    // generated from the decompile of 0x4061d0 (straight-line stores)
    include!("obj_reset.inc")
}
