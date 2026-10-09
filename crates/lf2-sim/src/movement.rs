//! Movement, friction, gravity and landing for one object. Ported from `obj_move`
//! (0x40e490).

use crate::mem::ftol;
use crate::world::World;

/// 0x40e490.
pub fn obj_move(w: &mut World, i: usize) {
    let gd = w.gd.clone();
    let tick_mod12 = w.g.tick_mod12();
    let o = &mut w.objs[i];
    // hit pause (0xb4): count towards 0 and skip the tick
    let pause = o.m.i32(0xb4);
    if pause != 0 {
        if pause > 0 {
            o.m.set_i32(0xb4, pause - 1);
        }
        if o.m.i32(0xb4) < 0 {
            o.m.add_i32(0xb4, 1);
        }
        return;
    }
    if o.weapon_type() < 0 {
        return; // held by someone
    }
    let d = &gd.objects[o.m.i32(0x368) as usize];
    let st = |f: i32| d.frames[f as usize].state;
    if d.frames[o.frame() as usize].cpoint.kind == 2 {
        return; // being held by a catcher
    }
    // 0x3e8..0x3f4: blocked in +z, -z, -x, +x this tick
    if !((o.vx() > 0.0 && o.m.i32(0x3f4) == 1) || (o.vx() < 0.0 && o.m.i32(0x3f0) == 1)) {
        o.set_x(o.x() + o.vx());
    }
    if d.kind == 4 || d.id == 0x78 {
        o.set_x(o.vx() * 0.2 + o.x());
    }
    if d.id == 0x65 {
        o.set_x(o.x() - 0.2 * o.vx());
    }
    if !((o.vz() > 0.0 && o.m.i32(0x3ec) == 1) || (o.vz() < 0.0 && o.m.i32(0x3e8) == 1)) {
        o.set_z(o.z() + o.vz());
    }
    for a in [0x3ecu32, 0x3e8, 0x3f0, 0x3f4] {
        o.m.set_i32(a, 0);
    }
    if d.kind == 3 && d.frames[o.frame() as usize].hit_j > 0 {
        o.set_z((d.frames[o.frame() as usize].hit_j - 0x32) as f64 + o.z());
    }
    // friction on the ground
    if o.iy() >= 0 {
        if 0.0001 < o.vx() {
            o.set_vx(o.vx() - 1.0);
            if o.vx() < 0.0001 {
                o.set_vx(0.0);
            }
        }
        if o.vx() < -0.0001 {
            o.set_vx(o.vx() + 1.0);
            if 0.0001 < o.vx() {
                o.set_vx(0.0);
            }
        }
        if 0.0001 < o.vz() {
            o.set_vz(o.vz() - 1.0);
            if o.vz() < 0.0001 {
                o.set_vz(0.0);
            }
        }
        if o.vz() < -0.0001 {
            o.set_vz(o.vz() + 1.0);
            if 0.0001 < o.vz() {
                o.set_vz(0.0);
            }
        }
    }
    if (d.kind == 4 || d.kind == 6) && st(o.frame()) == 1000 && (9.0 < o.vx() || o.vx() < -9.0) {
        o.set_frame(0x28);
    }
    let y = o.y() + o.vy();
    o.set_y(y);
    // a sound to queue once the object is no longer borrowed: (system, x, id)
    let mut snd: Option<(bool, i32, i32)> = None;

    if y < -0.0001 {
        // in the air: gravity
        if d.kind != 3 {
            let g = if d.kind == 6 {
                1.1333333333333333
            } else if d.kind == 4 {
                0.85
            } else if st(o.frame()) == 0x3ea {
                match d.id {
                    0x7c => 0.16999999999999998,
                    0x78 => 0.425,
                    0x65 => 1.1333333333333333,
                    _ => 0.5666666666666667,
                }
            } else {
                1.7
            };
            o.set_vy(o.vy() + g);
        }
        if d.kind == 0 {
            let f = o.frame();
            if st(f) == 0xc {
                let vy = o.vy();
                if f < 0xb9 {
                    o.set_frame(if vy < -8.0 {
                        0xb4
                    } else if vy < 1.0 {
                        0xb5
                    } else if vy < 8.0 {
                        0xb6
                    } else {
                        0xb7
                    });
                    if o.m.i32(0x320) < 0 {
                        o.set_frame(if vy < 12.0 && 5 < tick_mod12 { 0xb6 } else { 0xb5 });
                    }
                } else if f < 0xbf && 0xb9 < f {
                    o.set_frame(if vy < -8.0 {
                        0xba
                    } else if vy < 1.0 {
                        0xbb
                    } else if vy < 8.0 {
                        0xbc
                    } else {
                        0xbd
                    });
                }
            }
            if st(o.frame()) == 0x12 && o.frame() < 0xcd && 1.0 < o.vy() {
                o.set_frame(0xcd);
            }
        }
    } else if d.frames[o.frame() as usize].cpoint.kind != 2 {
        let state = st(o.frame());
        let drop_sound = gd.weapon_sound[o.m.i32(0x368) as usize][1];
        match d.kind {
            0 => {
                if land_character(o, d, y) {
                    snd = Some((true, o.ix(), 6));
                }
            }
            1 => {
                if y > 0.0001 && o.vy() > 0.0001 {
                    o.set_y(0.0);
                    o.add_weapon_hp(-d.weapon_drop_hurt);
                    let mut halve = true;
                    if o.vy() <= 9.9 {
                        if state == 0x3ea {
                            o.set_frame(0x46);
                        } else {
                            o.set_frame(0x3c);
                        }
                    } else if state == 0x3ea {
                        o.set_frame(7);
                        o.set_vy(-8.0);
                        o.set_facing(1 - o.facing());
                        o.set_vx(o.vx() * 0.5);
                        halve = false;
                        snd = Some((false, o.ix(), drop_sound));
                    } else {
                        o.set_frame(0x3c);
                    }
                    if halve {
                        o.set_vy(0.0);
                        o.set_u_88(0);
                        o.set_vx(o.vx() * 0.5);
                    }
                }
            }
            4 | 6 => {
                if y > 0.0001 && o.vy() > 0.0001 {
                    o.add_weapon_hp(-d.weapon_drop_hurt);
                    if d.kind == 6 && o.hp() < 1 {
                        o.set_weapon_hp(-1);
                    }
                    o.set_y(0.0);
                    let vx = o.vx();
                    if (o.vy() <= 8.5 && -10.0 <= vx && vx <= 10.0) || (state != 0x3ea && state != 1000) {
                        o.set_vy(0.0);
                        o.set_u_88(0);
                        o.set_vx(vx * 0.7);
                        o.set_frame(if state == 0x3ea { 0x46 } else { 0x3c });
                    } else {
                        o.set_frame(0);
                        let vy = o.vy() * -0.7;
                        o.set_vy(if vy < -10.0 { -10.0 } else { vy });
                        o.set_vx(vx * 0.7);
                        snd = Some((false, o.ix(), drop_sound));
                    }
                }
            }
            2 => {
                if y > 0.0001 {
                    o.add_weapon_hp(-1);
                    o.set_y(0.0);
                    if 9.0 < o.vy() {
                        snd = Some((false, o.ix(), drop_sound));
                        o.set_vy(-5.0);
                        o.set_facing(1 - o.facing());
                        o.set_vx(o.vx() * 0.5);
                    } else {
                        let hp = o.weapon_hp() - d.weapon_drop_hurt;
                        o.set_weapon_hp(if hp < 0 { 0 } else { hp });
                        o.set_frame(0x14);
                        o.set_vy(0.0);
                        o.set_u_88(0);
                        o.set_vx(o.vx() * 0.5);
                    }
                }
            }
            _ => {
                if d.id == 999 && -0.0001 < y {
                    o.set_y(0.0);
                    o.set_frame(0x65);
                    o.set_vy(0.0);
                    o.set_u_88(0);
                    o.set_vx(0.0);
                }
            }
        }
    }
    o.set_ix(ftol(o.x()));
    o.set_iy(ftol(o.y()));
    o.set_iz(ftol(o.z()));
    if st(o.frame()) != 0xc {
        o.m.set_i32(0x320, 0);
    }
    match snd {
        Some((true, x, id)) => w.play_sound(x, id),
        Some((false, x, id)) if id > -1 => w.play_sound_id(x, id),
        _ => {}
    }
}

/// The type-0 (character) part of landing in 0x40e490. Returns true when the landing
/// sound (system sound 6) plays.
fn land_character(o: &mut crate::fields::Obj, d: &lf2_data::ObjectData, y: f64) -> bool {
    let st = |f: i32| d.frames[f as usize].state;
    if 0.0001 < y && 0.0001 < o.vy() && st(o.frame()) == 0xd {
        if 17.0 < o.vy() || 9.0 < o.vx() || o.vx() < -9.0 {
            let def = o.m.i32(0x340);
            if def == 0 {
                o.add_hp(-10);
            } else {
                o.add_hp(-1000 / def);
            }
            o.set_y(0.0);
            o.set_vy(-3.5);
            o.set_vx(o.vx().min(7.0));
            if o.vx() < -7.0 {
                o.set_vx(-7.0);
            }
            o.set_frame(0xb9);
        } else {
            o.set_y(0.0);
            o.set_vy(0.0);
            o.set_vx(o.vx() / 3.0);
        }
    }
    let landing_frame;
    if o.y() <= 0.0 || o.vy() != 0.0 || o.frame() != 0xd4 {
        if !(0.0001 < o.y() && 0.0001 < o.vy()) {
            return false;
        }
        let s = st(o.frame());
        if s == 0xc || s == 0x12 {
            // landing from a fall
            let mut dmg = o.m.i32(0x320);
            if dmg != 0 {
                if dmg < 0 {
                    dmg = -dmg;
                    o.m.set_i32(0x320, dmg);
                }
                let def = o.m.i32(0x340);
                if def > 0 {
                    dmg = dmg * 100 / def;
                }
                o.add_hp(-dmg);
                o.add_dark_hp(-dmg);
                o.m.set_i32(0x320, 0);
            }
            if o.vy() <= 11.0 && o.vx() <= 9.0 && -9.0 <= o.vx() && s != 0x12 {
                o.set_y(0.0);
                o.set_vy(0.0);
                o.set_u_88(0);
                let f = (0xb9 < o.frame()) as i32 + 0xe6;
                o.set_vx(o.vx() / 3.0);
                o.set_frame(f);
                return true;
            }
            o.set_y(0.0);
            o.set_vy(-3.5);
            if 7.0 < o.vx() {
                o.set_vx(7.0);
            }
            if o.vx() < -7.0 {
                o.set_vx(-7.0);
            }
            if o.frame() < 0xba || st(o.frame()) == 0x12 {
                o.set_frame(0xb9);
            } else {
                o.set_frame(0xbf);
            }
            return true;
        }
        landing_frame = o.frame();
    } else {
        landing_frame = 0xd4;
    }
    // an ordinary landing
    o.set_y(0.0);
    o.set_vy(0.0);
    o.set_vx(o.vx() / 3.0);
    let s = st(landing_frame);
    if s == 100 {
        o.set_frame(0x5e);
    } else if landing_frame == 0xd4 || s == 6 {
        o.set_frame(0xd7);
    } else {
        o.set_frame(0xdb);
    }
    o.set_u_88(0);
    false
}
