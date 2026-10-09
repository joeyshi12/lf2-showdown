//! Catching: `catch_controls` (0x418c30) handles the catcher's keys, throws and the
//! grip timer; `catch_hold` (0x4187b0) puts the caught object at the catcher's cpoint
//! and applies cpoint injury.

use crate::post_loop::dark_hp_change;
use crate::world::{World, MAX_OBJECTS};

impl World {
    /// 0x418c30.
    pub fn catch_controls(&mut self) {
        // `local_4`: the last victim index the loop saw, kept across objects as the
        // original does. Starts uninitialised in the original.
        let mut last: Option<usize> = None;
        for i in 0..MAX_OBJECTS {
            if !self.exists(i) {
                continue;
            }
            let f7c = self.objs[i].m.i32(0x7c);
            let cp = self.fr(i, f7c).cpoint.clone();
            if cp.kind == 1 && self.objs[i].m.i32(0xb4) >= 0 {
                let v = self.objs[i].m.i32(0x8c) as usize;
                let linked = self.objs[v].m.i32(0x90) == i as i32 && {
                    last = Some(v);
                    self.fr(v, self.objs[v].m.i32(0x7c)).cpoint.kind == 2
                };
                let mut released = !linked;
                if linked {
                    if cp.decrease > 0 {
                        self.objs[i].m.add_i32(0x94, -cp.decrease);
                    }
                    if cp.decrease < 0 {
                        self.objs[i].m.add_i32(0x94, cp.decrease);
                        if self.objs[i].m.i32(0x94) < 0 {
                            self.objs[i].set_frame(0);
                            self.objs[v].set_frame(0);
                            self.objs[i].m.set_i32(0x20, 1);
                            self.objs[v].m.set_i32(0x20, 1);
                            let x = if self.objs[v].ix() < self.objs[i].ix() { -4.0 } else { 4.0 };
                            let o = &mut self.objs[v];
                            o.m.set_f64(0x28, x);
                            o.m.set_f64(0x30, -3.0);
                            o.set_frame(0xb5);
                            released = true;
                        }
                    }
                    if !released {
                        let take = |w: &mut World, act: i32| {
                            w.objs[i].set_frame(act);
                            if w.objs[i].frame() < 0 {
                                let o = &mut w.objs[i];
                                o.set_facing(1 - o.facing());
                                o.set_frame(-o.frame());
                            }
                            let va = w.frame_of(i).cpoint.vaction;
                            w.objs[v].set_frame(va);
                            w.objs[v].set_u_88(0);
                            w.objs[i].set_u_88(0);
                        };
                        let o = &self.objs[i];
                        if o.attack() != 0
                            && o.t_attack() > 0
                            && cp.aaction != 0
                            && ((o.left() == 0 && o.right() == 0) || cp.taction == 0)
                        {
                            take(self, cp.aaction);
                        }
                        let o = &self.objs[i];
                        if o.attack() != 0
                            && o.t_attack() > 0
                            && (o.left() != 0 || o.right() != 0 || o.up() != 0 || o.down() != 0)
                            && cp.taction != 0
                        {
                            take(self, cp.taction);
                        }
                        let o = &self.objs[i];
                        if o.jump() != 0 && o.t_jump() > 0 && cp.jaction != 0 {
                            take(self, cp.jaction);
                        }
                    }
                }
                if released {
                    self.objs[i].set_frame(0);
                }
                if cp.throwvx != 0 {
                    let Some(v) = (if linked { Some(v) } else { last }) else {
                        self.unported("throw from a broken catch with no earlier victim (0x418dxx)");
                        continue;
                    };
                    self.throw_caught(i, v, &cp);
                }
                let o = &mut self.objs[i];
                if o.u_88() == 2 {
                    let (r, l) = (o.right() != 0, o.left() != 0);
                    if cp.dircontrol == 1 {
                        if r && !l {
                            o.set_facing(0);
                        }
                        if !r && l {
                            o.set_facing(1);
                        }
                    }
                    if cp.dircontrol == -1 {
                        if r && !l {
                            o.set_facing(1);
                        }
                        if !r && l {
                            o.set_facing(0);
                        }
                    }
                }
            } else if self.frame_of(i).cpoint.kind == 2 {
                let c = self.objs[i].m.i32(0x90) as usize;
                let broken = self.objs[c].m.i32(0x8c) != i as i32 || {
                    last = Some(c);
                    self.frame_of(c).cpoint.kind != 1
                };
                if broken {
                    let o = &mut self.objs[i];
                    o.set_frame(0xd4);
                    o.set_vy(-3.0);
                    if -2.0 < o.y() {
                        o.set_y(-2.0);
                    }
                }
            }
        }
    }

    /// The `throwvx != 0` part of 0x418c30.
    fn throw_caught(&mut self, i: usize, v: usize, cp: &lf2_data::object::Cpoint) {
        let ti = cp.throwinjury.unwrap_or(0);
        if ti < 1 {
            if ti == -1 {
                // throwinjury -1: become the thrown character (Rudolf's transform)
                let (my_id, v_id, vd) = (self.data_of(i).id, self.data_of(v).id, self.objs[v].m.i32(0x368));
                let o = &mut self.objs[i];
                o.m.set_i32(0x324, my_id);
                o.m.set_i32(0x33c, v_id);
                o.m.set_i32(0x368, vd);
                o.set_frame(0);
                for j in 0..MAX_OBJECTS {
                    if self.exists(j) && self.objs[j].m.i32(0x2f4) == i as i32 {
                        self.objs[j].m.set_i32(0x368, vd);
                    }
                }
            }
            if cp.throwinjury.is_none() {
                self.unported("throw with an uninitialised throwinjury (0x418dd0)");
            }
        } else {
            self.objs[v].m.set_i32(0x320, ti);
        }
        let fr = self.frame_of(i);
        let (cx, cy) = (fr.centerx, fr.centery);
        let (ix, iy, f) = (self.objs[i].ix(), self.objs[i].iy(), self.objs[i].facing());
        {
            let o = &mut self.objs[v];
            o.set_iy((iy - cy) + cp.y);
            o.set_y(o.iy() as f64);
            o.set_ix(if f == 0 { (ix - cx) + cp.x } else { (cx - cp.x) + ix });
            o.set_x(o.ix() as f64);
        }
        let next = self.frame_of(i).next;
        {
            let o = &mut self.objs[i];
            o.set_frame(next);
            o.m.set_i32(0x7c, next);
            o.set_u_88(0);
        }
        let f = self.objs[i].facing();
        let (up, down) = (self.objs[i].up(), self.objs[i].down());
        let o = &mut self.objs[v];
        o.set_vx(if f == 0 { cp.throwvx } else { -cp.throwvx } as f64);
        o.set_frame(cp.vaction);
        o.m.set_i32(0x7c, cp.vaction);
        o.set_vy(cp.throwvy as f64);
        let tz = cp.throwvz.unwrap_or(0);
        if up != 0 && down == 0 {
            o.set_vz(-tz as f64);
        } else if up == 0 && down != 0 {
            o.set_vz(tz as f64);
        }
        if cp.throwvz.is_none() && (up != 0) != (down != 0) {
            self.unported("throw with an uninitialised throwvz (0x419221)");
        }
    }

    /// 0x4187b0.
    pub fn catch_hold(&mut self) {
        for i in 0..MAX_OBJECTS {
            if !self.exists(i) {
                continue;
            }
            let f = self.objs[i].frame();
            let cp = self.fr(i, f).cpoint.clone();
            if cp.kind != 1 || self.fr(i, f).state != 9 {
                continue;
            }
            let v = self.objs[i].m.i32(0x8c) as usize;
            if !(self.objs[v].m.i32(0x90) == i as i32 && self.frame_of(v).cpoint.kind == 2) {
                continue;
            }
            if (self.objs[v].m.i32(0xb4) == 0 && cp.hurtable == 1) || cp.hurtable == 0 {
                self.objs[v].set_frame(cp.vaction);
            }
            if self.objs[v].frame() < 0 {
                let o = &mut self.objs[v];
                o.set_facing(1 - o.facing());
                o.set_frame(-o.frame());
            }
            let inj = cp.injury;
            if inj != 0 && self.objs[i].u_88() == 0 {
                if inj < 1 {
                    let o = &mut self.objs[v];
                    o.add_hp(inj);
                    o.add_dark_hp(inj / 3);
                    self.objs[i].set_u_88(1);
                } else {
                    let def = self.objs[v].m.i32(0x340);
                    let dmg = if 0 < def { (inj * 100) / def } else { inj };
                    let o = &self.objs[v];
                    if 0 < o.hp() && o.hp() <= dmg && o.m.i32(0x2f4) == -1 {
                        let owner = self.objs[i].m.i32(0x354) as usize;
                        self.objs[owner].m.add_i32(0x358, 1);
                        let k = self.objs[v].m.i32(0x344);
                        if 0 < k && k < 3 {
                            self.g.m.add_i32(0x451b60 + 4 * k as u32, 1);
                        }
                    }
                    let o = &mut self.objs[v];
                    o.add_hp(-dmg);
                    o.add_dark_hp(dark_hp_change(dmg));
                    self.objs[i].set_u_88(1);
                    self.objs[i].m.set_i32(0xb4, 2);
                    self.objs[v].m.set_i32(0xb4, -3);
                    self.objs[v].m.add_i32(0x34c, dmg);
                    let owner = self.objs[i].m.i32(0x354) as usize;
                    self.objs[owner].m.add_i32(0x348, dmg);
                    let k = self.objs[v].m.i32(0x344);
                    if 0 < k && k < 3 {
                        self.g.m.add_i32(0x451b68 + 4 * k as u32, dmg);
                    }
                }
            }
            // place the victim's cpoint on the catcher's cpoint
            let fr = self.frame_of(i);
            let (cx, cy) = (fr.centerx, fr.centery);
            let (ix, iy, f, iz) = (self.objs[i].ix(), self.objs[i].iy(), self.objs[i].facing(), self.objs[i].iz());
            let px = if f == 0 { (ix - cx) + cp.x } else { (cx - cp.x) + ix };
            let py = (iy - cy) + cp.y;
            if cp.vaction < 0 {
                self.unported("catch with a negative vaction reads before the frame table (0x418a5x)");
                continue;
            }
            let vcp = self.fr(v, cp.vaction).cpoint.clone();
            let vfr = self.frame_of(v);
            let (vcx, vcy) = (vfr.centerx, vfr.centery);
            let o = &mut self.objs[v];
            if o.facing() == 0 {
                o.set_ix((vcx - vcp.x) + px);
                o.set_iy((vcy - vcp.y) + py);
            } else {
                o.set_ix((vcp.x - vcx) + px);
                o.set_iy((vcy - vcp.y) + py);
            }
            o.set_iz(iz);
            if cp.cover % 10 == 0 {
                o.add_iz(-1);
                o.add_iy(1);
            } else {
                o.add_iz(1);
                o.add_iy(-1);
            }
            match cp.cover / 10 {
                1 => o.set_facing(f),
                2 => o.set_facing(1 - f),
                _ => {}
            }
            o.set_z(o.iz() as f64);
            o.set_x(o.ix() as f64);
            o.set_y(o.iy() as f64);
        }
    }
}
