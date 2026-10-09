//! `obj_update` (0x42e100): apply the hits an attacker registered this tick (its hit
//! list at 0x280/0x2d0/0x2e4) to the victims: damage, knockback, falling, defending,
//! catching, picking up weapons, reflecting balls, ice and fire, and hit sparks.

use crate::world::World;
use lf2_data::object::Itr;

/// Object ids of the balls that can be turned into ice by Freeze (and back).
const BALLS: [i32; 7] = [200, 0xcb, 0xcd, 0xce, 0xcf, 0xd7, 0xd8];

fn is_weapon_type(t: i32) -> bool {
    t == 1 || t == 2 || t == 4 || t == 6
}

/// `dark_hp` change for `d` damage (see post_loop.rs).
use crate::post_loop::dark_hp_change;

impl World {
    /// MSVC `rand()`; the seed lives at 0x459f08 (moved there by the harness patch, see
    /// tools/harness/lockstep.py).
    pub fn crt_rand(&mut self) -> i32 {
        let s = self.g.m.u32(CRT_SEED).wrapping_mul(214013).wrapping_add(2531011);
        self.g.m.set_i32(CRT_SEED, s as i32);
        ((s >> 16) & 0x7fff) as i32
    }

    /// Damage `dmg` (already scaled by defence) to `v` from attacker `a`, with the kill
    /// and damage statistics the original keeps.
    fn deal_damage(&mut self, a: usize, v: usize, dmg: i32, kill_threshold: i32) {
        let vt = self.data_of(v).kind;
        let (vhp, v2f4, v344) = (self.objs[v].hp(), self.objs[v].m.i32(0x2f4), self.objs[v].m.i32(0x344));
        if 0 < vhp && vhp <= kill_threshold && vt == 0 && v2f4 == -1 {
            let owner = self.objs[a].m.i32(0x354) as usize;
            self.objs[owner].m.add_i32(0x358, 1);
            if 0 < v344 && v344 < 3 {
                self.g.m.add_i32(0x451b60 + 4 * v344 as u32, 1);
            }
        }
        let o = &mut self.objs[v];
        o.add_hp(-dmg);
        o.add_dark_hp(dark_hp_change(dmg));
        o.m.add_i32(0x34c, dmg);
        if self.data_of(v).kind == 0 && self.objs[v].m.i32(0x2f4) == -1 {
            let owner = self.objs[a].m.i32(0x354) as usize;
            self.objs[owner].m.add_i32(0x348, dmg);
        }
        let v344 = self.objs[v].m.i32(0x344);
        if 0 < v344 && v344 < 3 {
            self.g.m.add_i32(0x451b68 + 4 * v344 as u32, dmg);
        }
    }

    /// Defence-scaled injury (`injury * 100 / defence` when defence > 0).
    fn scaled_injury(&self, v: usize, injury: i32) -> i32 {
        let def = self.objs[v].m.i32(0x340);
        if 0 < def {
            (injury * 100) / def
        } else {
            injury
        }
    }

    /// Drop the heavy weapon `v` holds, if it holds one (used by kind 0 and kind 16 hits).
    fn knock_heavy_weapon_loose(&mut self, a: usize, v: usize, site: i32) {
        if self.objs[v].weapon_type() != 2 {
            return;
        }
        let w = self.objs[v].weapon_held() as usize;
        if self.objs[w].weapon_holder() == v as i32 && self.objs[w].weapon_type() == -2 {
            self.objs[a].m.set_u8(0xf0 + w as u32, 0x2d);
            self.objs[v].set_weapon_type(0);
            self.objs[v].m.set_u8(0xf0 + w as u32, 0x1e);
            self.objs[w].set_weapon_type(0);
            let r = self.random(site, 6);
            self.objs[w].set_frame(r);
            self.objs[w].set_vy(-1.0);
        }
    }

    /// 0x42e100.
    pub fn obj_update(&mut self, a: usize) {
        if self.objs[a].m.i32(0x2e4) <= 0 {
            return;
        }
        let a_f7c = self.objs[a].m.i32(0x7c);
        let a_data = self.objs[a].m.i32(0x368) as usize;
        let mut k: u32 = 0;
        loop {
            let itr_i = self.objs[a].m.i8(0x2d0 + k) as i32;
            let itrs_len = self.gd.objects[a_data].frames[a_f7c as usize].itrs.len() as i32;
            if itrs_len - 1 < itr_i {
                return;
            }
            let v = self.objs[a].m.i32(0x280 + 4 * k) as usize;
            if self.objs[v].m.i8(0xf0 + a as u32) < 1 {
                if self.objs[a].m.u8(0xeb) != 0 && self.data_of(v).kind == 0 {
                    return;
                }
                let v7c = self.objs[v].m.i32(0x7c);
                let caught_and_safe = {
                    let c = self.objs[v].m.i32(0x90) as usize;
                    self.fr(v, v7c).cpoint.kind == 2
                        && self.objs[c].m.i32(0x8c) == v as i32
                        && self.fr(c, self.objs[c].m.i32(0x7c)).cpoint.hurtable == 0
                };
                if !caught_and_safe && !self.apply_hit(a, v, a_data, a_f7c, itr_i as usize) {
                    return;
                }
            }
            k += 1;
            if self.objs[a].m.i32(0x2e4) <= k as i32 {
                return;
            }
        }
    }

    /// One entry of the hit list. Returns false where the original returns from
    /// `obj_update` altogether.
    fn apply_hit(&mut self, a: usize, v: usize, a_data: usize, a_f7c: i32, itr_i: usize) -> bool {
        let orig = self.gd.objects[a_data].frames[a_f7c as usize].itrs[itr_i].clone();
        if orig.kind == 0 && orig.effect == 0x15 {
            let s = self.state(v);
            if s == 0x12 || s == 0x13 {
                return false;
            }
        }
        // The itr used for the hit: a copy, or for a held weapon (kind 5) the copy with
        // the holder's weapon strength entry. When the weapon is held but not
        // attacking, the original works on the itr in the data file itself.
        let mut in_data = false;
        let mut it: Itr = orig.clone();
        if orig.kind == 5 && self.objs[a].weapon_type() < 0 {
            let h = self.objs[a].weapon_holder() as usize;
            if self.objs[h].weapon_held() == a as i32 {
                let att = self.fr(h, self.objs[h].m.i32(0x7c)).wpoint.attacking;
                if 0 < att && h != v {
                    let s = self.gd.objects[a_data].strength[att as usize].itr.clone();
                    it = Itr { kind: 0, x: orig.x, y: orig.y, w: orig.w, h: orig.h, ..s };
                } else {
                    in_data = true;
                }
            }
        }
        if 0 < self.objs[a].m.i32(0x320) && it.kind == 4 {
            it.kind = 0;
            let o = &self.objs[a];
            if (0.0 < o.vx() && o.facing() == 1) || (o.vx() < 0.0 && o.facing() == 0) {
                it.dvx = -it.dvx;
            }
        }
        if it.kind == 0 {
            self.knock_heavy_weapon_loose(a, v, 0xec);
        }
        if self.data_of(v).kind == 2 {
            it.dvx /= 2;
            it.dvy /= 2;
        }
        if in_data && it != orig {
            let gd = std::sync::Arc::make_mut(&mut self.gd);
            gd.objects[a_data].frames[a_f7c as usize].itrs[itr_i] = it.clone();
        }
        let mut spark_kind = 0;
        let vs = self.state(v);
        let v_type = self.data_of(v).kind;
        let mut special = true;
        if it.kind == 9 && (v_type == 0 || vs == 0x3ea || vs == 2000) {
            it.kind = 0;
            if v_type == 0 {
                self.objs[a].set_hp(0);
            }
            special = false;
        } else if it.kind == 0 {
            special = false;
        }
        if !special {
            let r = self.hit_kind0(a, v, &it, &mut spark_kind);
            if r == Hit::Return {
                return false;
            }
            if r == Hit::Special {
                self.hit_special(a, v, &it);
            }
        } else {
            self.hit_special(a, v, &it);
        }
        self.hit_spark(a, v, &it, spark_kind);
        true
    }
}

#[derive(PartialEq)]
enum Hit {
    Done,
    Return,
    /// `LAB_0043056e`: continue with the special-kind chain (which does nothing for
    /// kind 0, but keeps the original's path).
    Special,
}

/// The CRT seed address (see `World::crt_rand`).
pub const CRT_SEED: u32 = 0x459f08;

impl World {
    /// `LAB_0042e61c`: a kind 0 hit; decides between a normal hit and a defended one.
    fn hit_kind0(&mut self, a: usize, v: usize, it: &Itr, spark: &mut i32) -> Hit {
        let v7c = self.objs[v].m.i32(0x7c);
        let vs7c = self.fr(v, v7c).state;
        let mut normal = it.bdefend == 100 || [8, 0xb, 0xc, 0xd, 0xe, 0x10, 0x12].contains(&vs7c);
        if !normal {
            // armour: Knight (37), Louis (6), Julian (52) block weak hits
            let vid = self.data_of(v).id;
            let aid = self.data_of(a).id;
            let vb8 = self.objs[v].m.i32(0xb8);
            let e = it.effect;
            let pierces = e / 10 == 2 || e / 10 == 3 || e == 2 || e == 3 || aid == 0xd6 || aid == 0xd0;
            let blocked = if vid == 0x25 && vb8 < 0x10 {
                !pierces
            } else if vid == 6 && vb8 < 2 && !pierces && {
                let vf = self.objs[v].frame();
                let s = self.state(v);
                vf < 0x14 || s == 5 || s == 4 || s == 7
            } {
                true
            } else {
                vid == 0x34 && vb8 <= 0xf && aid != 0xd6 && aid != 0xd0
            };
            normal = !blocked;
        }
        if normal {
            let (af, vf) = (self.objs[a].facing(), self.objs[v].facing());
            let aid = self.data_of(a).id;
            let defending = vs7c == 7
                && it.bdefend < 0x3d
                && (af != vf || it.dvx < 0 || [0x7c, 0xdc, 0xdd, 0xde].contains(&aid))
                && 0 < self.objs[v].hp();
            if !defending {
                return self.normal_hit(a, v, it, spark);
            }
        }
        // LAB_0042fdf2: defended
        if it.kind != 0 || self.data_of(v).kind != 0 {
            return Hit::Special;
        }
        self.defended_hit(a, v, it, vs7c);
        Hit::Done
    }

    /// `LAB_0042e812` when not defended.
    fn normal_hit(&mut self, a: usize, v: usize, it: &Itr, spark: &mut i32) -> Hit {
        self.objs[v].m.set_i32(0xb8, 0x2d);
        if self.data_of(v).id == 300 {
            let bd = self.frame_of(v).bdys.first().map(|b| b.kind);
            let Some(bk) = bd else { return Hit::Return };
            if bk < 0x3e9 {
                return Hit::Return;
            }
            self.objs[v].set_team(1);
            self.objs[v].set_frame(bk - 1000);
            self.objs[a].m.set_i32(0xb4, 3);
            self.objs[v].m.set_i32(0xb4, -3);
            return Hit::Return;
        }
        let vt = self.data_of(v).kind;
        if vt != 6 {
            let dmg = self.scaled_injury(v, it.injury);
            self.deal_damage(a, v, dmg, dmg);
        }
        if self.objs[v].hp() < 1 || it.effect == 4 {
            self.objs[v].m.set_i32(0xb0, 0x50);
        }
        let vt = self.data_of(v).kind;
        if is_weapon_type(vt) {
            self.objs[v].add_weapon_hp(-it.injury);
            if it.bdefend == 100 {
                self.objs[v].set_weapon_hp(-1);
            }
        }
        if vt != 2 || 0x28 < it.fall {
            self.objs[v].m.add_i32(0x20, 1);
        }
        self.objs[v].m.add_i32(0xb0, if it.fall == 0 { 0x14 } else { it.fall });
        if self.state78(v) == 0xd || self.state7c(v) == 0xc || is_weapon_type(vt) {
            self.objs[v].m.set_i32(0xb0, 0x50);
        }
        let b0 = self.objs[v].m.i32(0xb0);
        let t3 = vt == 3;
        let same_facing = (self.objs[v].facing() == self.objs[a].facing()) as i32;
        {
            let o = &mut self.objs[v];
            if b0 >= 0x3d && !t3 {
                o.m.set_i32(0xb0, 0x50);
            } else if b0 >= 0x29 && !t3 {
                o.set_frame(0xe2);
                o.m.set_i32(0xb0, 0x3c);
                if o.iy() < 0 {
                    o.m.set_i32(0xb0, 0x50);
                }
            } else if b0 >= 0x15 && !t3 {
                o.set_frame(same_facing * 2 + 0xde);
                o.m.set_i32(0xb0, 0x28);
                if o.iy() < 0 {
                    o.m.set_i32(0xb0, 0x50);
                }
            } else if 0 < b0 && !t3 {
                o.set_frame(0xdc);
                o.m.set_i32(0xb0, 0x14);
                if o.iy() < 0 {
                    o.set_frame(same_facing * 2 + 0xde);
                }
            }
        }
        // sounds
        if self.data_of(a).kind == 3 {
            let snd = self.gd.weapon_sound[self.objs[a].m.i32(0x368) as usize][2];
            if snd > -1 {
                let x = self.objs[a].ix();
                self.play_sound_id(x, snd);
            }
        }
        let vx_ = self.objs[v].ix();
        let fell = self.objs[v].m.i32(0xb0) == 0x50;
        if vt == 0 {
            if it.effect == 0 {
                self.play_sound(vx_, if fell { 2 } else { 0 });
            }
            if it.effect == 1 {
                *spark = 1;
                self.play_sound(vx_, if fell { 0xc } else { 0xb });
                self.play_sound(vx_, if fell { 2 } else { 0 });
            }
        } else if 0 < vt {
            let snd = self.gd.weapon_sound[self.objs[v].m.i32(0x368) as usize][0];
            if snd > -1 {
                self.play_sound_id(vx_, snd);
            }
        }
        // knockback in x
        let a_state = self.state(a);
        let aid = self.data_of(a).id;
        let af = self.objs[a].facing();
        let dvx = it.dvx as f64;
        let mut bat_sound = false;
        {
            let (aix, a_wt) = (self.objs[a].ix(), self.objs[a].weapon_type());
            let o = &mut self.objs[v];
            let k28 = o.m.f64(0x28);
            if o.m.i32(0xb0) != 0x50 || 5.0 <= o.vx() || o.vx() <= -5.0 || it.dvx != 0 {
                if a_state == 2000 {
                    if aix < o.ix() {
                        o.m.set_f64(0x28, dvx + k28);
                    } else {
                        o.m.set_f64(0x28, k28 - dvx);
                    }
                } else if vt == 4 || vt == 6 {
                    let mut d = o.vx() * 0.55;
                    if o.vx() < 0.0 {
                        d = -(o.vx() * 0.55);
                    }
                    if d < dvx || (af == 0 && 0.0 < k28) || (af == 1 && k28 < 0.0) {
                        if af == 0 {
                            o.m.set_f64(0x28, dvx + o.m.f64(0x28));
                        }
                        if af == 1 {
                            o.m.set_f64(0x28, o.m.f64(0x28) - dvx);
                        }
                    } else if (af == 1 && 0.0 < o.vx()) || (af == 0 && o.vx() < 0.0) {
                        o.m.set_f64(0x28, -(o.vx() * 0.55));
                    }
                    if aid == 100 && a_wt < 0 {
                        bat_sound = true;
                        o.m.set_f64(0x28, o.m.f64(0x28) * 2.5);
                        let x = o.m.f64(0x28);
                        if 0.0 < x && x < 10.0 {
                            o.m.set_f64(0x28, 10.0);
                        }
                        let x = o.m.f64(0x28);
                        if x < 0.0 && -10.0 < x {
                            o.m.set_f64(0x28, -10.0);
                        }
                    }
                } else if it.effect == 0x16 || it.effect == 0x17 {
                    if o.ix() <= aix {
                        o.m.set_f64(0x28, dvx + k28);
                    } else {
                        o.m.set_f64(0x28, k28 - dvx);
                    }
                } else {
                    if af == 0 {
                        o.m.set_f64(0x28, dvx + o.m.f64(0x28));
                    }
                    if af == 1 {
                        o.m.set_f64(0x28, o.m.f64(0x28) - dvx);
                    }
                }
            } else if a_state == 2000 {
                o.m.set_f64(0x28, 5.0 + k28);
            } else {
                o.m.set_f64(0x28, ((af as i32) * -2 + 1) as f64 * 5.0 + k28);
            }
        }
        if bat_sound {
            let x = self.objs[v].ix();
            self.play_sound(x, 0xd);
        }
        if a_state == 3000 {
            let vd = self.data_of(v);
            let (vk, vid) = (vd.kind, vd.id);
            let vf = self.objs[v].frame();
            if vk == 0 || aid != 0xd1 || (!BALLS.contains(&vid) && (vid != 0xd1 || vf != 0x28)) {
                let o = &mut self.objs[a];
                o.set_frame(10);
                o.set_u_88(0);
                o.set_vx(0.0);
                let dvy = self.gd.objects[o.m.i32(0x368) as usize].frames[10].dvy;
                self.objs[a].set_vz(dvy as f64);
            }
        }
        if self.objs[v].m.i32(0xb0) == 0x50 {
            let vt = self.data_of(v).kind;
            let o = &mut self.objs[v];
            if it.dvy == 0 {
                if (vt != 2 && vt != 3) || 0x28 < it.fall {
                    o.m.set_f64(0x30, o.m.f64(0x30) - 7.0);
                }
            } else {
                if (vt != 2 && vt != 3) || 0x28 < it.fall {
                    o.m.set_f64(0x30, it.dvy as f64 + o.m.f64(0x30));
                }
                if 0 < crate::mem::ftol(o.iy() as f64 + o.m.f64(0x30)) {
                    o.m.set_f64(0x30, 12.0);
                }
            }
            let x = o.m.f64(0x28);
            let f = o.facing();
            o.set_frame(if (f == 0 && x <= 0.0) || (f == 1 && x >= 0.0) { 0xb4 } else { 0xba });
            if 0 < o.weapon_type() {
                let w = o.weapon_held() as usize;
                if self.objs[w].weapon_holder() == v as i32 {
                    self.objs[a].m.set_u8(0xf0 + w as u32, 0x2d);
                    self.objs[v].m.set_u8(0xf0 + w as u32, 0x1e);
                }
            }
        }
        if -1 < self.objs[a].m.i32(0xb4) {
            self.objs[a].m.set_i32(0xb4, 3);
        }
        self.objs[v].m.set_i32(0xb4, -3);
        if it.arest < 4 && it.vrest == 0 {
            self.objs[a].m.set_i32(0xec, 4);
        } else {
            self.objs[a].m.set_i32(0xec, it.arest);
        }
        if 0 < it.vrest {
            self.objs[v].m.set_u8(0xf0 + a as u32, it.vrest as u8);
        }
        {
            let v7c = self.objs[v].m.i32(0x7c);
            let cp = self.fr(v, v7c).cpoint.clone();
            let c = self.objs[v].m.i32(0x90) as usize;
            if cp.kind == 2 && self.objs[c].m.i32(0x8c) == v as i32 && self.objs[v].m.i32(0xb0) != 0x50 && cp.injury != 0 {
                let f = if self.objs[v].facing() == self.objs[a].facing() { cp.cover } else { cp.injury };
                self.objs[v].set_frame(f);
            }
        }
        if self.objs[v].m.i32(0xb0) == 0x50 {
            self.objs[v].m.set_i32(0xb0, 0);
        }
        if self.objs[a].weapon_type() < 0 {
            let h = self.objs[a].weapon_holder() as usize;
            let b4 = self.objs[a].m.i32(0xb4);
            self.objs[h].m.set_i32(0xb4, b4);
        }
        if self.state(a) == 0x3ea {
            let r = self.random(0xee, 0x10);
            let v28 = self.objs[v].m.f64(0x28);
            let o = &mut self.objs[a];
            o.set_frame(r);
            o.set_vx(-(v28 * 0.5));
            o.set_vy(-4.0);
            if self.data_of(a).kind == 4 && self.data_of(v).kind == 4 {
                self.objs[a].m.set_f64(0x28, -v28);
            }
        }
        let vt = self.data_of(v).kind;
        let ateam = self.objs[a].team();
        if vt == 1 {
            self.objs[v].m.set_u8(0xeb, 1);
            let r = self.random(0xef, 0x10);
            self.objs[v].set_frame(r);
            self.objs[v].set_team(ateam);
        }
        let vt = self.data_of(v).kind;
        if vt == 4 || vt == 6 {
            self.objs[a].m.set_u8(0xf0 + v as u32, 0x1e);
            self.objs[v].m.set_u8(0xeb, 1);
            let r = self.random(0xf0, 0x10);
            self.objs[v].set_frame(r);
            self.objs[v].set_team(ateam);
        }
        if self.data_of(v).kind == 2 {
            self.objs[v].m.set_u8(0xeb, 1);
            let rest = if it.fall < 0x29 && it.effect != 4 { 3 } else { 0x13 };
            if self.objs[a].weapon_type() == -2 {
                let h = self.objs[a].weapon_holder() as usize;
                self.objs[h].m.set_u8(0xf0 + v as u32, rest);
            } else if self.data_of(a).kind != 2 {
                self.objs[a].m.set_u8(0xf0 + v as u32, rest);
            }
            let af = self.objs[a].facing();
            self.objs[v].set_facing(af);
            if it.fall < 0x29 && -1 < self.objs[v].iy() && it.effect != 4 {
                self.objs[v].set_frame(0x14);
            } else {
                let r = self.random(0xf1, 6);
                self.objs[v].set_frame(r);
            }
            self.objs[v].set_team(ateam);
        }
        if self.data_of(a).id == 0xc9 && self.data_of(v).kind == 0 {
            self.set_exists(a, false);
        }
        if self.data_of(a).id == 0xd6 && self.data_of(v).kind == 0 {
            self.objs[a].set_hp(0);
        }
        if self.data_of(v).kind == 3 {
            self.ball_hit(a, v, it);
        }
        let vt = self.data_of(v).kind;
        if (it.effect == 3 || it.effect == 0x1e) && vt == 0 && self.state78(v) != 0xd {
            self.objs[v].set_frame(200);
            self.objs[v].set_u_88(0);
            let x = self.objs[v].ix();
            self.play_sound(x, 0xe);
        }
        let e = it.effect;
        if (e == 2 || e == 0x15 || e == 0x16 || (e == 0x14 && self.state78(v) != 0x12)) && self.data_of(v).kind == 0 {
            let o = &mut self.objs[v];
            o.set_frame(0xcb);
            o.set_u_88(0);
            let x = o.ix();
            self.play_sound(x, 0x10);
            let o = &mut self.objs[v];
            o.set_facing(if o.m.f64(0x28) < 0.0 { 0 } else { 1 });
        }
        if it.effect == 0x17 {
            let x = self.objs[v].ix();
            self.play_sound(x, 0x10);
        }
        Hit::Done
    }

    /// A ball (type 3) is hit: it changes owner and is sent back.
    fn ball_hit(&mut self, a: usize, v: usize, it: &Itr) {
        let (vs, as_) = (self.state(v), self.state(a));
        if vs != 0xbbd && (vs != 0xbbe || as_ == 0xbbd) {
            let src = if self.objs[a].weapon_type() < 0 { self.objs[a].weapon_holder() as usize } else { a };
            let (t, o354) = (self.objs[src].team(), self.objs[src].m.i32(0x354));
            {
                let o = &mut self.objs[v];
                o.set_team(t);
                o.m.set_i32(0x354, o354);
                o.m.set_u8(0xeb, 1);
                o.set_u_88(0);
                o.m.set_f64(0x30, 0.0);
                o.m.set_f64(0x28, 0.0);
                o.m.set_f64(0x38, 0.0);
            }
            let aid = self.data_of(a).id;
            let vid = self.data_of(v).id;
            let ice = self.data_index(0xd1);
            if aid == 0xd1 && BALLS.contains(&vid) {
                let (t, o354, ad) = (self.objs[a].team(), self.objs[a].m.i32(0x354), self.objs[a].m.i32(0x368));
                let o = &mut self.objs[v];
                o.set_team(t);
                o.m.set_i32(0x354, o354);
                o.m.set_i32(0x368, ad);
                o.set_frame(0x28);
                o.m.set_i32(0x74, 0x28);
                o.m.set_i32(0x78, 0x28);
            } else if (self.data_of(a).kind == 0 || self.objs[a].weapon_type() < 0) && it.effect != 2 && it.effect != 0x14 {
                self.objs[v].set_frame(0x1e);
                if aid == 8 && BALLS.contains(&vid) {
                    if let Some(k) = ice {
                        let o = &mut self.objs[v];
                        o.m.set_i32(0x368, k as i32);
                        let f = o.frame();
                        o.m.set_i32(0x74, f);
                        o.m.set_i32(0x78, f);
                    }
                }
            } else {
                self.objs[v].set_frame(0x14);
            }
            if self.objs[a].weapon_type() < 0 {
                let h = self.objs[a].weapon_holder();
                self.objs[v].set_walk_counter(h);
                let vid = self.data_of(v).id;
                if aid == 0xd5 && BALLS.contains(&vid) {
                    if let Some(k) = ice {
                        let o = &mut self.objs[v];
                        o.m.set_i32(0x368, k as i32);
                        let f = o.frame();
                        o.m.set_i32(0x74, f);
                        o.m.set_i32(0x78, f);
                    }
                    let h = h as usize;
                    let (t, o354) = (self.objs[h].team(), self.objs[h].m.i32(0x354));
                    self.objs[v].set_team(t);
                    self.objs[v].m.set_i32(0x354, o354);
                }
            } else {
                self.objs[v].set_walk_counter(a as i32);
            }
        }
        let (vs, as_) = (self.state(v), self.state(a));
        if (vs == 0xbbd && as_ == 0xbbd) || (vs == 0xbbe && as_ == 0xbbe) {
            for x in [v, a] {
                let o = &mut self.objs[x];
                o.set_frame(0x14);
                o.set_u_88(0);
                o.m.set_f64(0x30, 0.0);
                o.m.set_f64(0x28, 0.0);
                o.m.set_f64(0x38, 0.0);
            }
        }
        let p = if self.objs[a].weapon_type() < 0 { self.objs[a].weapon_holder() as usize } else { a };
        let b4 = self.objs[p].m.i32(0xb4);
        if 0 < b4 {
            self.objs[p].m.set_i32(0xb4, -b4);
        }
    }

    /// The defended part of 0x42fdf2 for a character victim.
    fn defended_hit(&mut self, a: usize, v: usize, it: &Itr, vs7c: i32) {
        if self.data_of(a).kind == 3 {
            let snd = self.gd.weapon_sound[self.objs[a].m.i32(0x368) as usize][2];
            if snd > -1 {
                let x = self.objs[a].ix();
                self.play_sound_id(x, snd);
            }
        } else {
            let vid = self.data_of(v).id;
            let x = self.objs[v].ix();
            self.play_sound(x, if vid == 0x25 || vid == 6 { 0x11 } else { 1 });
        }
        let raw = self.scaled_injury(v, it.injury);
        self.deal_damage(a, v, raw / 10, raw / 10);
        if self.objs[v].hp() < 1 {
            self.objs[v].m.set_i32(0xb0, 0x50);
        }
        {
            let o = &mut self.objs[v];
            o.set_u_88(0);
            o.m.add_i32(0xb8, it.bdefend);
            o.m.add_i32(0x20, 1);
        }
        self.objs[a].m.set_i32(0xb4, 3);
        self.objs[v].m.set_i32(0xb4, -5);
        let a_state = self.state(a);
        let (aix, af) = (self.objs[a].ix(), self.objs[a].facing());
        let dvx = it.dvx as f64;
        let e16 = it.effect == 0x16 || it.effect == 0x17;
        let o = &mut self.objs[v];
        let k28 = o.m.f64(0x28);
        if o.iy() == 0 {
            if o.m.i32(0xb8) >= 0x1f && vs7c == 7 {
                o.set_frame(0x70);
            } else if o.frame() == 0x6e {
                o.set_frame(0x6f);
            }
            if o.m.i32(0xb0) != 0x50 || 3.0 <= o.vx() || o.vx() <= -3.0 || it.dvx != 0 {
                if a_state == 2000 {
                    if aix < o.ix() {
                        o.m.set_f64(0x28, dvx + k28);
                    } else {
                        o.m.set_f64(0x28, k28 - dvx);
                    }
                } else if e16 {
                    if o.ix() <= aix {
                        o.m.set_f64(0x28, dvx + k28);
                    } else {
                        o.m.set_f64(0x28, k28 - dvx);
                    }
                } else {
                    if af == 0 {
                        o.m.set_f64(0x28, ((it.dvx / 2) as f64) + o.m.f64(0x28));
                    }
                    if af == 1 {
                        o.m.set_f64(0x28, (-(it.dvx / 2)) as f64 + o.m.f64(0x28));
                    }
                }
            } else if a_state == 2000 {
                if aix < o.ix() {
                    o.m.set_f64(0x28, k28 + 6.0);
                } else {
                    o.m.set_f64(0x28, k28 - 6.0);
                }
            } else {
                o.m.set_f64(0x28, ((af as i32) * -2 + 1) as f64 * 3.0 + k28);
            }
        } else if o.m.i32(0xb0) != 0x50 || 6.0 <= o.vx() || o.vx() <= -6.0 || 5 < it.dvx {
            if !e16 {
                if af == 0 {
                    o.m.set_f64(0x28, dvx + o.m.f64(0x28));
                }
                if af == 1 {
                    o.m.set_f64(0x28, o.m.f64(0x28) - dvx);
                }
            } else if o.ix() <= aix {
                o.m.set_f64(0x28, dvx + k28);
            } else {
                o.m.set_f64(0x28, k28 - dvx);
            }
        } else {
            o.m.set_f64(0x28, ((af as i32) * -2 + 1) as f64 * 6.0 + k28);
        }
        // LAB_0043030f
        if it.arest < 4 && it.vrest == 0 {
            self.objs[a].m.set_i32(0xec, 4);
        } else {
            self.objs[a].m.set_i32(0xec, it.arest);
            if 0xc < self.objs[a].m.i32(0xec) {
                self.objs[a].m.set_i32(0xec, 0xc);
            }
        }
        if 0 < it.vrest {
            if it.vrest < 5 {
                self.objs[v].m.set_u8(0xf0 + a as u32, 4);
            } else {
                self.objs[v].m.set_u8(0xf0 + a as u32, it.vrest as u8);
                if 0xc < self.objs[v].m.i8(0xf0 + a as u32) {
                    self.objs[v].m.set_u8(0xf0 + a as u32, 0xc);
                }
            }
        }
        if self.objs[a].weapon_type() < 0 {
            let h = self.objs[a].weapon_holder() as usize;
            let b4 = self.objs[a].m.i32(0xb4);
            self.objs[h].m.set_i32(0xb4, b4);
        }
        if self.state(a) == 0x3ea {
            let r = self.random(0xf3, 0x10);
            let v28 = self.objs[v].m.f64(0x28);
            let o = &mut self.objs[a];
            o.set_frame(r);
            o.set_vx(-(v28 * 0.5));
            o.set_vy(-4.0);
            o.set_vz(o.vz() / -1.5);
        }
        if self.state(a) == 2000 {
            let (vi, ai, avx) = (self.objs[v].ix(), self.objs[a].ix(), self.objs[a].vx());
            if (vi < ai && avx < 0.0) || (ai < vi && avx > 0.0) {
                let o = &mut self.objs[a];
                o.set_vx(o.vx() / 2.5);
                o.set_vz(o.vz() / 2.5);
            }
        }
        if self.state(a) == 3000 {
            let o = &mut self.objs[a];
            o.set_frame(10);
            o.set_u_88(0);
            o.set_vx(0.0);
        }
    }

    /// `LAB_0043056e`: itr kinds other than 0.
    fn hit_special(&mut self, a: usize, v: usize, it: &Itr) {
        match it.kind {
            6 => self.objs[v].m.set_i8(0xea, 3),
            1 | 3 => self.catch(a, v, it),
            7 if self.objs[a].weapon_type() == 0 => {
                self.objs[a].set_weapon_type(1);
                self.objs[v].set_weapon_type(-1);
                let t = self.objs[a].team();
                self.objs[v].set_team(t);
                self.objs[a].set_weapon_held(v as i32);
                self.objs[v].set_weapon_holder(a as i32);
                self.objs[v].m.set_i32(0x354, a as i32);
                self.objs[a].m.add_i32(0x35c, 1);
                let vid = self.data_of(v).id;
                if vid == 0x78 || vid == 0x7c {
                    self.objs[a].set_weapon_type(0x65);
                }
                let vt = self.data_of(v).kind;
                if vt == 4 {
                    self.objs[a].set_weapon_type(4);
                    self.objs[v].set_weapon_type(-4);
                }
                if vt == 6 {
                    if self.objs[v].hp() < 1 {
                        self.objs[a].set_weapon_type(4);
                        self.objs[v].set_weapon_hp(0);
                    } else {
                        self.objs[a].set_weapon_type(6);
                    }
                    let t = self.objs[a].weapon_type();
                    self.objs[v].set_weapon_type(-t);
                }
            }
            7 => {}
            2 => {
                let vt = self.data_of(v).kind;
                let vid = self.data_of(v).id;
                let t = self.objs[a].team();
                let link = |w: &mut World, at: i32, vt_: i32| {
                    w.objs[a].set_weapon_type(at);
                    w.objs[v].set_weapon_type(vt_);
                    w.objs[a].set_weapon_held(v as i32);
                    w.objs[v].set_weapon_holder(a as i32);
                    w.objs[v].set_team(t);
                    w.objs[v].m.set_i32(0x354, a as i32);
                    w.objs[a].m.add_i32(0x35c, 1);
                };
                if vt == 1 {
                    self.objs[a].set_frame(0x73);
                    let at = if vid == 0x78 || vid == 0x7c { 0x65 } else { 1 };
                    link(self, at, -1);
                }
                if vt == 4 {
                    self.objs[a].set_frame(0x73);
                    link(self, 4, -4);
                }
                if vt == 6 {
                    self.objs[a].set_frame(0x73);
                    let at = if self.objs[v].hp() < 1 {
                        self.objs[v].set_weapon_hp(0);
                        4
                    } else {
                        6
                    };
                    link(self, at, -at);
                }
                if vt == 2 {
                    self.objs[a].set_frame(0x74);
                    link(self, 2, -2);
                }
                self.objs[a].set_u_88(0);
            }
            8 => {
                self.objs[v].m.set_i32(0xe0, it.injury + 1000);
                let (vx, vz) = (self.objs[v].x(), self.objs[v].z());
                let o = &mut self.objs[a];
                o.set_frame(it.dvx);
                o.set_x(vx);
                o.set_z(vz + 1.0);
            }
            9 => {
                let vt = self.data_of(v).kind;
                if vt == 3 {
                    let snd = self.gd.weapon_sound[self.objs[v].m.i32(0x368) as usize][2];
                    if snd > -1 {
                        let x = self.objs[v].ix();
                        self.play_sound_id(x, snd);
                    }
                    if self.state(v) == 0xbbd {
                        self.objs[a].m.set_i32(0xb4, -3);
                        self.objs[v].m.set_u8(0xeb, 1);
                        self.objs[v].set_frame(0x28);
                    } else {
                        self.objs[a].m.set_i32(0xb4, -3);
                        let (t, o354) = (self.objs[a].team(), self.objs[a].m.i32(0x354));
                        let o = &mut self.objs[v];
                        o.set_team(t);
                        o.m.set_i32(0x354, o354);
                        o.m.set_u8(0xeb, 1);
                        o.set_frame(0x1e);
                        o.set_u_88(0);
                        o.m.set_f64(0x30, 0.0);
                        o.set_vy(0.0);
                        o.m.set_f64(0x28, 0.0);
                        o.set_vx(0.0);
                        o.m.set_f64(0x38, 0.0);
                        o.set_vz(0.0);
                        o.set_walk_counter(a as i32);
                    }
                } else if vt == 0 {
                    self.objs[a].set_hp(0);
                }
            }
            k @ (10 | 11) if k == 10 || self.objs[v].m.i32(0x320) < 0 => self.freeze_hit(a, v),
            10 | 11 => {}
            0xe => {
                let (aix, aiz) = (self.objs[a].ix(), self.objs[a].iz());
                let o = &mut self.objs[v];
                if !(aix <= o.ix() + 5 || (o.vx() <= 0.0 && o.m.f64(0x28) <= 0.0)) {
                    o.m.set_i32(0x3f4, 1);
                } else if aix < o.ix() - 5 && (o.vx() < 0.0 || o.m.f64(0x28) < 0.0) {
                    o.m.set_i32(0x3f0, 1);
                }
                if !(aiz <= o.iz() + 2 || (o.vz() <= 0.0 && o.m.f64(0x38) <= 0.0)) {
                    o.m.set_i32(0x3ec, 1);
                } else if aiz < o.iz() - 2 && (o.vz() < 0.0 || o.m.f64(0x38) < 0.0) {
                    o.m.set_i32(0x3e8, 1);
                }
            }
            0xf | 0x10 => self.whirlwind_hit(a, v, it),
            _ => {}
        }
    }

    /// Kinds 1 and 3: `a` catches `v`.
    fn catch(&mut self, a: usize, v: usize, it: &Itr) {
        let (ca, cv) = (it.catchingact[0], it.caughtact[0]);
        self.objs[v].set_vx(0.0);
        self.objs[a].set_vx(0.0);
        let af = (self.objs[v].ix() < self.objs[a].ix()) as i8;
        self.objs[a].set_facing(af);
        self.objs[v].set_facing(1 - af);
        self.objs[a].set_frame(ca);
        self.objs[v].set_frame(cv);
        let cx_a = self.fr(a, ca).cpoint.x;
        let cx_v = self.fr(v, cv).cpoint.x;
        // the original reads frame `caughtact` of the catcher's own data here
        let (a_cx, a_cx2, a_cy) = (self.fr(a, ca).centerx, self.fr(a, cv).centerx, self.fr(a, ca).centery);
        let v_cy = self.fr(v, cv).centery;
        let (aix, aiy) = (self.objs[a].ix(), self.objs[a].iy());
        self.objs[a].set_x(aix as f64);
        self.objs[a].set_y(aiy as f64);
        let vx = if af == 0 { ((aix - a_cx) - a_cx2) + cx_v + cx_a } else { ((a_cx + a_cx2 + aix) - cx_v) - cx_a };
        let o = &mut self.objs[v];
        o.set_x(vx as f64);
        o.set_y(((v_cy - a_cy) + aiy) as f64);
        let d = (o.ix() as f64 - o.x()) * 0.5;
        o.set_x(o.x() + d);
        let o = &mut self.objs[a];
        o.set_x(d + o.x());
        let vx = crate::mem::ftol(self.objs[v].x());
        self.objs[v].set_ix(vx);
        let ax = crate::mem::ftol(self.objs[a].x());
        self.objs[a].set_ix(ax);
        self.objs[a].m.set_i32(0x8c, v as i32);
        self.objs[v].m.set_i32(0x90, a as i32);
        self.objs[a].m.set_i32(0x94, 300);
        self.objs[v].m.set_i32(0xb0, 0);
    }

    /// The rise used by kinds 10/11/15/16: lift off the ground with vy -6, then slow.
    fn lift(&mut self, v: usize, slow: f64) {
        let o = &mut self.objs[v];
        if -3 < o.iy() {
            o.set_iy(-2);
            o.set_vy(-6.0);
        }
        if o.vy() <= -6.0 {
            return;
        }
        o.set_vy(o.vy() - slow);
        let vy = o.vy();
        o.m.set_f64(0x30, vy);
    }

    /// Kinds 10 and 11 (11 only while burning): frozen (Freeze's whirlwind).
    fn freeze_hit(&mut self, a: usize, v: usize) {
        let vt = self.data_of(v).kind;
        let vid = self.data_of(v).id;
        if vt == 0 {
            self.objs[v].m.set_i32(0x320, -20);
            if self.data_of(v).kind == 0 && self.objs[v].m.i32(0x2f4) == -1 && self.g.tick_mod12() == 0 {
                let owner = self.objs[a].m.i32(0x354) as usize;
                self.objs[owner].m.add_i32(0x348, 0xb);
            }
            let v344 = self.objs[v].m.i32(0x344);
            if 0 < v344 && v344 < 3 {
                self.g.m.add_i32(0x451b68 + 4 * v344 as u32, 0xb);
            }
            let o = &mut self.objs[v];
            o.m.set_f64(0x28, o.vx() / 1.07);
            o.set_vx(o.m.f64(0x28));
            o.m.set_f64(0x38, o.vz() / 1.07);
            o.set_vz(o.m.f64(0x38));
            o.set_frame(0xb6);
            self.lift(v, 3.0);
        } else if (vt == 1 || vt == 4 || vt == 6) && vid != 0xc9 && vid != 0xca {
            if self.state(v) != 1000 {
                self.objs[v].set_frame(0);
            }
            let o = &mut self.objs[v];
            o.m.set_f64(0x28, o.vx() / 1.07);
            o.set_vx(o.m.f64(0x28));
            o.m.set_f64(0x38, o.vz() / 1.07);
            o.set_vz(o.m.f64(0x38));
            self.lift(v, 3.0);
        } else if vt == 2 {
            if self.state(v) != 2000 {
                self.objs[v].set_frame(0);
            }
            let o = &mut self.objs[v];
            o.m.set_f64(0x28, o.vx() / 1.07);
            o.set_vx(o.m.f64(0x28));
            o.m.set_f64(0x38, o.vz() / 1.07);
            o.set_vz(o.m.f64(0x38));
            self.lift(v, 2.3);
        }
    }

    /// Kinds 15 and 16: whirlwinds (15 pulls, 16 freezes and pulls).
    fn whirlwind_hit(&mut self, a: usize, v: usize, it: &Itr) {
        let vt = self.data_of(v).kind;
        let vid = self.data_of(v).id;
        let (aix, aiz) = (self.objs[a].ix(), self.objs[a].iz());
        let pull = |w: &mut World, toward_minus: bool, slow: f64| {
            let o = &mut w.objs[v];
            let d = if toward_minus { o.vx() - 1.0 } else { o.vx() + 1.0 };
            o.m.set_f64(0x28, d);
            o.set_vx(o.m.f64(0x28));
            let dz = if aiz < o.iz() { o.vz() - 0.5 } else { o.vz() + 0.5 };
            o.m.set_f64(0x38, dz);
            o.set_vz(o.m.f64(0x38));
            w.lift(v, slow);
        };
        if vt == 0 {
            if it.kind == 0x10 {
                let dmg = self.scaled_injury(v, it.injury);
                self.deal_damage(a, v, dmg, dmg);
                self.objs[v].set_frame(200);
                self.objs[v].set_u_88(0);
                let x = self.objs[v].ix();
                self.play_sound(x, 0xe);
                if 0 < it.vrest {
                    self.objs[v].m.set_u8(0xf0 + a as u32, it.vrest as u8);
                }
                self.knock_heavy_weapon_loose(a, v, 0xf4);
            }
            if it.kind == 0xf {
                let minus = aix < self.objs[v].ix();
                pull(self, minus, 3.0);
            }
        } else if (vt == 1 || vt == 4 || vt == 6) && vid != 0xc9 && vid != 0xca {
            if self.state(v) != 1000 {
                self.objs[v].set_frame(0);
            }
            let minus = !(self.objs[v].ix() <= aix);
            pull(self, minus, 3.0);
        } else if vt == 2 {
            if self.state(v) != 2000 {
                self.objs[v].set_frame(0);
            }
            let minus = aix < self.objs[v].ix();
            pull(self, minus, 2.3);
        }
    }

    /// `LAB_0043187a`: a hit spark for kind 0 hits, stored in the nearer object's list
    /// (count 0x36c, x 0x370.., y 0x398.., kind 0x3c0..; up to 10). Uses CRT rand().
    fn hit_spark(&mut self, a: usize, v: usize, it: &Itr, spark_kind: i32) {
        if it.kind != 0 {
            return;
        }
        let (viz, aiz) = (self.objs[v].iz(), self.objs[a].iz());
        let owner = if viz > aiz || !self.exists(a) {
            v
        } else if viz < aiz {
            a
        } else if v < a {
            a
        } else {
            v
        };
        let n = self.objs[owner].m.i32(0x36c);
        if n >= 10 {
            return;
        }
        let n = n + 1;
        self.objs[owner].m.set_i32(0x36c, n);
        let kind = if it.fall < 0x3d { spark_kind * 0x14 + 10 } else { spark_kind * 0x14 };
        let off = 4 * n as u32;
        self.objs[owner].m.set_i32(0x3bc + off, kind);
        let fr = self.frame_of(a);
        let (cx, cy) = (fr.centerx, fr.centery);
        let (aix, aiy, af) = (self.objs[a].ix(), self.objs[a].iy(), self.objs[a].facing());
        let (vix, viy) = (self.objs[v].ix(), self.objs[v].iy());
        let mut x = if af == 0 { (aix - cx) + it.w + it.x } else { ((cx - it.w) - it.x) + aix };
        if (af == 0 && vix < x) || (af != 0 && x < vix) {
            x = vix;
        }
        let mut y = (it.h / 2 + aiy + it.y) - cy;
        let t = viy - cy;
        if y < t {
            y = (y + t) / 2;
        } else if viy < y {
            y = (y + viy) / 2;
        }
        let r = self.crt_rand();
        self.objs[owner].m.set_i32(0x394 + off, r % 9 + aiz - 4 + y);
        let r = self.crt_rand();
        self.objs[owner].m.set_i32(0x36c + off, x + r % 9 - 4);
    }
}

impl World {
    /// The state change inside the object drawing routine 0x41a5a0: each hit spark
    /// advances one animation step, and a finished spark is dropped when it is the
    /// last in the list. Runs for every existing object (the drawing order does not
    /// matter here).
    pub fn advance_sparks(&mut self) {
        for i in 0..crate::world::MAX_OBJECTS {
            if !self.exists(i) {
                continue;
            }
            let o = &mut self.objs[i];
            let mut j = 0;
            while j < o.m.i32(0x36c) {
                let a = 0x3c0 + 4 * j as u32;
                let k = o.m.i32(a);
                if k < 5 || (10..15).contains(&k) || (20..29).contains(&k) || (30..39).contains(&k) {
                    o.m.set_i32(a, k + 1);
                } else if j == o.m.i32(0x36c) - 1 {
                    o.m.add_i32(0x36c, -1);
                }
                j += 1;
            }
        }
    }
}
