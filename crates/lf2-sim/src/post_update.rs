//! `obj_post_update` (0x40d960): per-object timers and frame advance (wait/next).

use crate::world::World;

/// `DAT_00450c30`: difficulty (0 easy, 1 normal, 2 difficult; -1 crazy).
pub const DIFFICULTY: u32 = 0x450c30;

impl World {
    /// 0x40d960.
    pub fn obj_post_update(&mut self, i: usize, mode: i32) {
        let gd = self.gd.clone();
        let mp_cost_on = self.g.mp_cost_on() != 0;
        let difficulty = self.g.m.i32(DIFFICULTY);
        let o = &mut self.objs[i];
        let d = &gd.objects[o.m.i32(0x368) as usize];
        if o.m.i32(0xb4) != 0 && d.kind != 3 {
            return; // hit pause
        }
        if o.m.i32(0xec) > 0 {
            o.m.add_i32(0xec, -1);
        }
        if o.weapon_type() < 0 {
            return;
        }
        let fr = |f: i32| &d.frames[f as usize];
        if fr(o.frame()).cpoint.kind == 2 {
            return;
        }
        if d.kind == 3 {
            let cost = fr(o.frame()).hit_a;
            if cost > 0 {
                o.add_hp(-cost);
                if o.hp() < 1 {
                    o.set_hp(0);
                    o.set_frame(fr(o.frame()).hit_d);
                }
            }
        }
        if o.u_08() > 0 {
            o.set_u_08(o.u_08() - 1);
        }
        if o.u_08() < 0 {
            o.set_u_08(o.u_08() + 1);
        }
        if o.m.i32(0xb0) > 0 {
            o.m.add_i32(0xb0, -1);
        }
        if o.m.i32(0xb8) > 0 {
            o.m.add_i32(0xb8, -1);
        }
        if o.m.i8(0xea) > 0 {
            o.m.set_i8(0xea, o.m.i8(0xea) - 1);
        }
        if o.frame() != o.m.i32(0x74) {
            let snd = gd.frame_sound[o.m.i32(0x368) as usize][o.frame() as usize];
            if snd > -1 {
                let ix = o.ix();
                self.play_sound_id(ix, snd);
            }
            self.objs[i].set_u_88(0);
        }
        let o = &mut self.objs[i];
        o.set_u_88(o.u_88() + 1);
        if d.kind >= 0 && fr(o.frame()).state == 0 && o.iy() < 0 {
            o.set_frame(0xd4);
        }
        if d.kind == 2 && fr(o.frame()).state == 2000 && o.iy() == 0 && o.vx() < 0.1 && -0.1 < o.vx() {
            o.set_frame(0x14);
        }
        if fr(o.frame()).state == 0xe && o.hp() < 1 {
            if (o.m.i32(0x2f4) > -1 || o.team() == 5 || 0x13 < i) && o.u_08() < 1 {
                o.set_u_08(0x1e);
            }
            o.set_u_88(0);
        }
        if fr(o.frame()).state == 2000 {
            o.set_facing(if o.vx() <= 0.0 { 1 } else { 0 });
        }
        'advance: {
            if o.u_88() <= fr(o.frame()).wait {
                break 'advance;
            }
            o.set_u_88(0);
            let next = fr(o.frame()).next;
            if next == 0 {
                break 'advance;
            }
            o.set_frame(next);
            if next < 0 {
                o.set_facing(1 - o.facing());
                o.set_frame(-o.frame());
            }
            let mut landing_jump = false;
            if o.frame() == 999 {
                if o.iy() == 0 || d.kind != 0 {
                    o.set_frame(0);
                } else {
                    landing_jump = true;
                    o.set_frame(0xd4);
                }
            }
            let f = o.frame();
            if !(0..400).contains(&f) {
                return; // the caller deletes the object
            }
            let was_lying = fr(o.m.i32(0x74)).state == 0xe;
            let p344 = o.m.i32(0x344);
            if was_lying
                && fr(f).state != 0xd
                && ((o.team() != 5 && p344 == 0) || difficulty != 2)
                && (((mode != 1 && mode != 4) || (o.team() != 5 && p344 == 0)) || (d.id / 10 != 3 || d.id == 0x26))
            {
                o.set_u_08(0xf); // blinking after getting up
            }
            if o.frame() == 0xd4 && !landing_jump {
                o.set_vy(d.jump_height.unwrap_or(0.0));
                let jd = d.jump_distance.unwrap_or(0.0);
                if o.right() == 1 && o.left() == 0 {
                    o.set_vx(jd);
                } else if o.left() == 1 && o.right() == 0 {
                    o.set_vx(-jd);
                }
                let jz = d.jump_distancez.unwrap_or(0.0);
                if o.up() != 0 && o.down() == 0 {
                    o.set_vz(-jz);
                } else if o.up() == 0 && o.down() != 0 {
                    o.set_vz(jz);
                }
            }
            if o.frame() < 400 {
                let snd = gd.frame_sound[o.m.i32(0x368) as usize][o.frame() as usize];
                if snd > -1 {
                    let ix = o.ix();
                    self.play_sound_id(ix, snd);
                }
            }
            let o = &mut self.objs[i];
            let cost = fr(o.frame()).mp;
            if cost < 0 && mp_cost_on {
                if o.mp() < cost {
                    o.set_frame(fr(o.frame()).hit_d);
                } else {
                    o.set_mp(cost + o.mp());
                    o.m.add_i32(0x350, -fr(o.frame()).mp);
                }
                let hd = fr(o.frame()).hit_d;
                if hd > 0 {
                    if o.left() == 1 && o.right() == 0 && o.iy() == 0 && o.facing() == 0 {
                        o.set_frame(hd);
                    }
                    if o.left() == 0 && o.right() == 1 && o.iy() == 0 && o.facing() == 1 {
                        o.set_frame(fr(o.frame()).hit_d);
                    }
                }
            }
        }
        let o = &mut self.objs[i];
        if o.frame() == 0x6e || o.frame() == 0x72 {
            o.set_t_c1(3);
        }
        if o.frame() == 0xca {
            o.set_u_08(0x14);
        }
        let f = o.frame();
        o.m.set_i32(0x74, f);
    }
}
