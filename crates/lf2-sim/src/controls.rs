//! Key handling for one object: key timers, the nine special-move inputs, and
//! walking, running, jumping and dashing started from keys. Ported from
//! `obj_controls` (0x413080) and the helpers it calls.

use crate::fields::Obj;
use crate::world::World;
use lf2_data::ObjectData;

/// 0x40e450: push a key code into the five-entry history at 0x408..0x418.
pub fn push_key(o: &mut Obj, key: i32) {
    for a in [0x408u32, 0x40c, 0x410, 0x414] {
        let v = o.m.i32(a + 4);
        o.m.set_i32(a, v);
    }
    o.m.set_i32(0x418, key);
}

/// Key timer offsets in the order 0x40e170 tests them.
const TIMERS: [(u8, u32); 7] = [
    (b'a', 0xbe),
    (b'j', 0xbf),
    (b'd', 0xc0),
    (b'L', 0xc3),
    (b'U', 0xc4),
    (b'D', 0xc5),
    (b'R', 0xc2),
];

/// 0x40e170: whether a key other than `kind` was pressed this tick (its timer is 5).
/// With `any == 0` every key counts; with any other value but 1 nothing does.
pub fn other_key_pressed(o: &Obj, kind: u8, any: i32) -> bool {
    match any {
        0 => TIMERS.iter().any(|&(_, t)| o.m.i8(t) == 5),
        1 => {
            if !TIMERS.iter().any(|&(k, _)| k == kind) {
                return false;
            }
            TIMERS.iter().any(|&(k, t)| k != kind && o.m.i8(t) == 5)
        }
        _ => false,
    }
}

/// 0x40e2d0: switch to frame `target` (negative: also turn around, when specials cost
/// mp). Frame 999 means frame 0. Specials that cost mp only start if the object can pay:
/// `mp` field = hp_cost*1000 + mp_cost.
pub fn goto_frame(w: &mut World, i: usize, target: i32) {
    let back = target < 0;
    let mut f = if back { target.wrapping_neg() } else { target };
    if f == 999 {
        f = 0;
    }
    let data = w.data_of(i);
    let Some(fr) = data.frames.get(f as usize) else { return };
    if !fr.exists {
        return;
    }
    let cost = fr.mp;
    let o = &mut w.objs[i];
    if w.g.mp_cost_on() == 0 {
        o.set_frame(f);
        clear_key_timers(o);
        return;
    }
    let hp_cost = (cost / 1000) * 10;
    if cost % 1000 <= o.mp() && hp_cost < o.hp() {
        o.add_hp(-hp_cost);
        o.add_mp(-(cost % 1000));
        o.m.add_i32(0x34c, hp_cost);
        o.set_frame(f);
        o.m.add_i32(0x350, cost % 1000);
        if back {
            o.set_facing(1 - o.facing());
        }
        clear_key_timers(o);
    }
}

fn clear_key_timers(o: &mut Obj) {
    o.set_t_jump(0);
    o.set_t_attack(0);
    o.set_t_defend(0);
    o.set_t_down(0);
    o.set_t_up(0);
    o.set_t_left(0);
    o.set_t_right(0);
}

/// Which frame field a special-move input starts.
#[derive(Clone, Copy)]
enum Hit {
    Fa,
    Ua,
    Da,
    Fj,
    Uj,
    Dj,
}

fn hit_of(f: &lf2_data::Frame, h: Hit) -> i32 {
    match h {
        Hit::Fa => f.hit_fa,
        Hit::Ua => f.hit_ua,
        Hit::Da => f.hit_da,
        Hit::Fj => f.hit_fj,
        Hit::Uj => f.hit_uj,
        Hit::Dj => f.hit_dj,
    }
}

/// 0x412800..0x412e60: defend, then a direction, then attack or jump.
/// `state` is the progress byte, `dir`/`dir_kind` the direction's timer and code,
/// `last`/`last_kind` the final key's, `face` the facing to set when it fires.
fn special_dir(w: &mut World, i: usize, state: u32, dir: u32, dir_kind: u8, last: u32, last_kind: u8, hit: Hit, face: Option<i8>) {
    let mut adv = 0;
    {
        let o = &mut w.objs[i];
        if o.m.i8(state) == 0 && o.t_defend() == 5 {
            o.m.set_i8(state, 1);
            adv = 1;
        }
        if o.m.i8(state) == 1 {
            if o.m.i8(dir) == 5 {
                o.m.set_i8(state, 2);
                adv = 1;
            } else if other_key_pressed(o, b'd', adv) {
                o.m.set_i8(state, 0);
            }
        }
        if o.m.i8(state) == 2 {
            if o.m.i8(last) == 5 {
                o.m.set_i8(state, 3);
                adv = 1;
            } else if other_key_pressed(o, dir_kind, adv) {
                o.m.set_i8(state, 0);
            }
        }
    }
    if w.objs[i].m.i8(state) == 3 {
        let target = hit_of(w.frame_of(i), hit);
        if target != 0 && w.objs[i].weapon_type() != 2 {
            goto_frame(w, i, target);
            let o = &mut w.objs[i];
            if let Some(f) = face {
                o.set_facing(f);
            }
            o.m.set_i8(state, 0);
            return;
        }
        let o = &mut w.objs[i];
        if other_key_pressed(o, last_kind, adv) {
            o.m.set_i8(state, 0);
        }
    }
}

/// 0x412f40: defend, jump, attack (D^J... the "DJA" input, frame field hit_ja).
fn special_dja(w: &mut World, i: usize) {
    const S: u32 = 0xdc;
    let mut adv = 0;
    {
        let o = &mut w.objs[i];
        if o.m.i8(S) == 0 && o.t_defend() == 5 {
            o.m.set_i8(S, 1);
            adv = 1;
        }
        if o.m.i8(S) == 1 {
            if o.t_jump() == 5 {
                o.m.set_i8(S, 2);
                adv = 1;
            } else if other_key_pressed(o, b'd', adv) {
                o.m.set_i8(S, 0);
            }
        }
        if o.m.i8(S) == 2 {
            if o.t_attack() == 5 {
                o.m.set_i8(S, 3);
                adv = 1;
            } else if other_key_pressed(o, b'j', adv) {
                o.m.set_i8(S, 0);
            }
        }
    }
    if w.objs[i].m.i8(S) != 3 {
        return;
    }
    let hit_ja = w.frame_of(i).hit_ja;
    let id = w.data_of(i).id;
    // Louis turns into LouisEX (hit_ja 300) only below 178 hp, unless 0x458428 is set.
    let blocked = id == 6 && hit_ja == 300 && w.objs[i].hp() >= 0xb2 && w.g.u_458428() == 0;
    if blocked {
        return;
    }
    if hit_ja != 0 && w.objs[i].m.i32(0x324) == -1 && w.objs[i].weapon_type() != 2 {
        goto_frame(w, i, hit_ja);
        w.objs[i].m.set_i8(S, 0);
        return;
    }
    if w.objs[i].m.i32(0x328) == 1 {
        w.objs[i].m.set_i32(0x338, 0);
        return;
    }
    let o = &mut w.objs[i];
    if other_key_pressed(o, b'a', adv) {
        o.m.set_i8(S, 0);
    }
}

fn wfr(d: &ObjectData) -> i32 {
    d.walking_frame_rate.unwrap_or(0)
}
fn rfr(d: &ObjectData) -> i32 {
    d.running_frame_rate.unwrap_or(0)
}
fn v(x: Option<f64>) -> f64 {
    x.unwrap_or(0.0)
}

/// The walking frame cycle: frames base..base+3 then back down, `rate` ticks each.
fn walk_step(o: &mut Obj, rate: i32, base: i32) -> bool {
    let c = (o.walk_counter() + 1) % (rate * 6);
    o.set_walk_counter(c);
    if c < rate * 4 {
        o.set_frame(c / rate + base);
        true
    } else {
        o.set_frame(base + 6 - c / rate);
        false
    }
}

/// 0x413080.
pub fn obj_controls(w: &mut World, i: usize) {
    {
        let o = &mut w.objs[i];
        for t in [0xc2u32, 0xc3, 0xc4, 0xc5, 0xbf, 0xbe, 0xc0, 0xc1] {
            let x = o.m.i8(t);
            if x > 0 {
                o.m.set_i8(t, x - 1);
            }
        }
        for (prev, cur, timer, code) in [
            (0xc9u32, 0xd0u32, 0xc2u32, 6),
            (0xc8, 0xcf, 0xc3, 4),
            (0xc6, 0xcd, 0xc4, 8),
            (0xc7, 0xce, 0xc5, 2),
            (0xcc, 0xd3, 0xc0, 9),
            (0xcb, 0xd2, 0xbf, 0),
            (0xca, 0xd1, 0xbe, 5),
        ] {
            if o.m.i8(prev) == 0 && o.m.i8(cur) == 1 {
                o.m.set_i8(timer, 5);
                push_key(o, code);
            }
        }
    }
    special_dir(w, i, 0xd4, 0xc2, b'R', 0xbe, b'a', Hit::Fa, Some(0));
    special_dir(w, i, 0xd5, 0xc3, b'L', 0xbe, b'a', Hit::Fa, Some(1));
    special_dir(w, i, 0xd6, 0xc4, b'U', 0xbe, b'a', Hit::Ua, None);
    special_dir(w, i, 0xd7, 0xc5, b'D', 0xbe, b'a', Hit::Da, None);
    special_dir(w, i, 0xd8, 0xc2, b'R', 0xbf, b'j', Hit::Fj, Some(0));
    special_dir(w, i, 0xd9, 0xc3, b'L', 0xbf, b'j', Hit::Fj, Some(1));
    special_dir(w, i, 0xda, 0xc4, b'U', 0xbf, b'j', Hit::Uj, None);
    special_dir(w, i, 0xdb, 0xc5, b'D', 0xbf, b'j', Hit::Dj, None);
    special_dja(w, i);

    // hit_a / hit_d / hit_j: the most recently pressed of attack, defend, jump wins.
    {
        let f = w.frame_of(i).clone();
        let o = &w.objs[i];
        let (ta, td, tj) = (o.t_attack(), o.t_defend(), o.t_jump());
        if f.hit_a != 0 && ta > td && ta > tj {
            goto_frame(w, i, f.hit_a);
            w.objs[i].set_t_attack(0);
        } else if f.hit_d != 0 && td > ta && td > tj {
            goto_frame(w, i, f.hit_d);
            w.objs[i].set_t_defend(0);
        } else if f.hit_j != 0 && ta < tj && td < tj {
            goto_frame(w, i, f.hit_j);
            w.objs[i].set_t_jump(0);
        }
    }

    let gd = w.gd.clone();
    let mp_cost_on = w.g.mp_cost_on() != 0;
    let o = &mut w.objs[i];
    let d: &ObjectData = &gd.objects[o.m.i32(0x368) as usize];
    let st = |o: &Obj| d.frames[o.frame() as usize].state;

    if o.frame() == 0x6e {
        if o.right() == 1 {
            o.set_facing(0);
        }
        if o.left() == 1 {
            o.set_facing(1);
        }
    }
    // states 301 and 19: up/down set vz to the running z speed
    let s = st(o);
    if s == 0x12d || s == 0x13 {
        if o.up() == 1 && o.down() == 0 && o.iy() == 0 {
            o.set_vz(-v(d.running_speedz));
        }
        if o.down() == 1 && o.up() == 0 && o.iy() == 0 {
            o.set_vz(v(d.running_speedz));
        }
    }

    let wt = o.weapon_type();
    // standing or walking, not holding a heavy weapon
    if wt != 2 && (s == 0 || s == 1) {
        if o.run_counter() > 0 {
            o.add_run_counter(-1);
        }
        if o.run_counter() < 0 {
            o.add_run_counter(1);
        }
        let rate = wfr(d);
        if o.right() == 1 && o.left() == 0 && o.iy() == 0 {
            if o.facing() == 1 {
                o.set_run_counter(0);
            }
            o.set_facing(0);
            walk_step(o, rate, 5);
            o.set_vx(v(d.walking_speed));
            if o.prev_right() == 0 {
                o.add_run_counter(10);
            }
            if o.run_counter() > 10 {
                o.set_frame(9);
                o.set_walk_counter(0);
                o.set_run_counter(0);
            }
        }
        if o.right() == 0 && o.left() == 1 && o.iy() == 0 {
            if o.facing() == 0 {
                o.set_run_counter(0);
            }
            o.set_facing(1);
            walk_step(o, rate, 5);
            o.set_vx(-v(d.walking_speed));
            if o.prev_left() == 0 {
                o.add_run_counter(-10);
            }
            if o.run_counter() < -10 {
                o.set_frame(9);
                o.set_walk_counter(0);
                o.set_run_counter(0);
            }
        }
        // up/down: step the walk cycle only if left and right are equal (both or neither)
        if o.up() == 1 && o.down() == 0 && o.iy() == 0 {
            if (o.left() == 0) == (o.right() == 0) {
                walk_step(o, rate, 5);
            }
            o.set_vz(-v(d.walking_speedz));
            o.set_vx(o.vx() / 1.4);
        }
        if o.down() == 1 && o.up() == 0 && o.iy() == 0 {
            if (o.left() == 0) == (o.right() == 0) {
                walk_step(o, rate, 5);
            }
            o.set_vz(v(d.walking_speedz));
            o.set_vx(o.vx() / 1.4);
        }
        // attack: a random punch (needs the RNG, so the frame is set below)
        let mut punch: Option<(i32, i32, bool)> = None; // (site, base, costs mp)
        if o.attack() == 1 && o.t_attack() > 0 {
            o.set_run_counter(0);
            o.set_u_88(0);
            if wt == 0 {
                if o.m.i8(0xea) < 1 {
                    punch = Some((0x82, 0xc, true));
                } else {
                    o.set_frame(0x46);
                }
            } else if wt % 100 == 1 {
                if wt == 0x65 && (o.right() != 0 || o.left() != 0 || o.up() != 0 || o.down() != 0) {
                    o.set_frame(0x2d);
                } else {
                    punch = Some((if wt == 0x65 { 0x83 } else { 0x84 }, 4, false));
                }
            } else if wt == 4 {
                o.set_frame(0x2d);
            } else if wt == 6 {
                o.set_frame(0x37);
            }
        }
        if let Some((site, base, costs)) = punch {
            let r = w.random(site, 2);
            let o = &mut w.objs[i];
            o.set_frame((r + base) * 5);
            if costs && mp_cost_on {
                let cost = d.frames[(r * 5 + 0x3c) as usize].mp;
                o.add_mp(-cost);
                if o.mp() < 0 {
                    o.set_mp(0);
                } else {
                    o.m.add_i32(0x350, cost);
                }
            }
        }
        obj_controls_jump_defend(&mut w.objs[i]);
    }
    obj_controls_rest(w, i, wt)
}

/// The jump and defend checks that end the standing/walking block.
fn obj_controls_jump_defend(o: &mut Obj) {
    if o.jump() == 1 && o.t_jump() > 0 {
        o.set_u_88(0);
        o.set_run_counter(0);
        o.set_frame(0xd2);
    }
    if o.defend() == 1 && o.t_c1() == 0 && o.t_defend() > 0 {
        o.set_run_counter(0);
        o.set_u_88(0);
        o.set_frame(0x6e);
    }
}

/// The part of 0x413080 after the standing/walking block. `wt` is the weapon type read
/// before that block (the original keeps it in a register).
fn obj_controls_rest(w: &mut World, i: usize, wt_before: i32) {
    let gd = w.gd.clone();
    let mp_cost_on = w.g.mp_cost_on() != 0;
    let d: &ObjectData = &gd.objects[w.objs[i].m.i32(0x368) as usize];
    let st = |o: &Obj| d.frames[o.frame() as usize].state;
    let _ = wt_before;

    // heavy weapon: standing or walking
    {
        let o = &mut w.objs[i];
        let s = st(o);
        if o.weapon_type() == 2 && (s == 0 || s == 1) {
            if o.run_counter() > 0 {
                o.add_run_counter(-1);
            }
            if o.run_counter() < 0 {
                o.add_run_counter(1);
            }
            if o.frame() < 0xc {
                o.set_frame(0xc);
            }
            let rate = wfr(d);
            let right = o.right();
            if right == 1 && o.left() == 0 && o.iy() == 0 {
                if o.facing() == 1 {
                    o.set_run_counter(0);
                }
                o.set_facing(0);
                walk_step(o, rate, 0xc);
                o.set_vx(v(d.heavy_walking_speed));
                if o.prev_right() == 0 {
                    o.add_run_counter(10);
                }
                if o.run_counter() > 10 {
                    o.set_frame(0x10);
                    o.set_walk_counter(0);
                    o.set_run_counter(0);
                }
            }
            if right == 0 && o.left() == 1 && o.iy() == 0 {
                if o.facing() == 0 {
                    o.set_run_counter(0);
                }
                o.set_facing(1);
                walk_step(o, rate, 0xc);
                o.set_vx(-v(d.heavy_walking_speed));
                if o.prev_left() == 0 {
                    o.add_run_counter(-10);
                }
                if o.run_counter() < -10 {
                    o.set_frame(0x10);
                    o.set_walk_counter(0);
                    o.set_run_counter(0);
                }
            }
            if o.up() == 1 && o.down() == 0 && o.iy() == 0 {
                if (o.left() == 0) == (right == 0) {
                    walk_step(o, rate, 0xc);
                }
                o.set_vz(-v(d.heavy_walking_speedz));
                o.set_vx(o.vx() / 1.4);
            }
            if o.down() == 1 && o.up() == 0 && o.iy() == 0 {
                if (o.left() == 0) == (right == 0) {
                    walk_step(o, rate, 0xc);
                }
                o.set_vz(v(d.heavy_walking_speedz));
                o.set_vx(o.vx() / 1.4);
            }
            if o.attack() == 1 && o.t_attack() > 0 {
                o.set_run_counter(0);
                o.set_u_88(0);
                o.set_frame(0x32);
            }
        }
    }

    // `iVar7` from here on: the weapon type as of the heavy-weapon check
    let mut wt = w.objs[i].weapon_type();

    // state 4 (jumping) in the air: steer, jump attack
    {
        let o = &mut w.objs[i];
        if st(o) == 4 && o.iy() < 0 {
            let right = o.right();
            if right == 1 && o.left() == 0 {
                o.set_facing(0);
            }
            if o.left() == 1 && right == 0 {
                o.set_facing(1);
            }
            if o.attack() != 0 {
                if wt == 0 {
                    o.set_u_88(0);
                    o.set_frame(0x50);
                    if mp_cost_on {
                        let cost = d.frames[0x50].mp;
                        o.add_mp(-cost);
                        if o.mp() < 0 {
                            o.set_mp(0);
                        } else {
                            o.m.add_i32(0x350, cost);
                        }
                    }
                } else if wt % 100 == 1 {
                    o.set_u_88(0);
                    wt = o.weapon_type();
                    if o.left() == 0 && right == 0 && o.up() == 0 && o.down() == 0 {
                        o.set_frame(0x1e);
                    } else {
                        o.set_frame(0x34);
                    }
                } else {
                    wt = o.weapon_type();
                    if wt == 4 || wt == 6 {
                        o.set_frame(0x34);
                    }
                }
            }
        }
    }

    // state 2 (running), not heavy
    if st(&w.objs[i]) == 2 && wt != 2 {
        let o = &mut w.objs[i];
        o.set_u_88(0);
        let rate = rfr(d);
        let c = (o.walk_counter() + 1) % (rate * 4);
        o.set_walk_counter(c);
        if c < rate * 3 {
            o.set_frame(c / rate + 9);
        } else {
            o.set_frame(10);
        }
        if o.facing() == 0 {
            o.set_vx(v(d.running_speed));
            if o.left() == 1 {
                o.set_frame(0xda);
            }
        }
        if o.facing() == 1 {
            o.set_vx(-v(d.running_speed));
            if o.right() == 1 {
                o.set_frame(0xda);
            }
        }
        let up = o.up();
        if up == 1 && o.down() == 0 && o.iy() == 0 {
            o.set_vz(-v(d.running_speedz));
            o.set_vx(o.vx() / 1.2);
        }
        if up == 0 && o.down() == 1 && o.iy() == 0 {
            o.set_vz(v(d.running_speedz));
            o.set_vx(o.vx() / 1.2);
        }
        if o.attack() == 1 && o.t_attack() > 0 {
            if wt == 0 {
                if !mp_cost_on {
                    o.set_frame(0x55);
                } else if d.frames[0x55].mp <= o.mp() {
                    let c = d.frames[0x55].mp;
                    o.add_mp(-c);
                    o.m.add_i32(0x350, c);
                    o.set_frame(0x55);
                }
            } else if wt % 100 == 1 {
                if o.left() == 0 && o.right() == 0 && up == 0 && o.down() == 0 {
                    o.set_frame(0x23);
                } else {
                    o.set_frame(0x2d);
                }
            } else if o.weapon_type() == 4 {
                o.set_frame(0x2d);
            } else if o.weapon_type() == 6 {
                if o.left() != 0 || o.right() != 0 || up != 0 || o.down() != 0 {
                    o.set_frame(0x2d);
                } else {
                    o.set_frame(0x37);
                }
            }
        }
        if o.defend() == 1 && o.t_defend() > 0 {
            o.set_frame(0x66);
        }
        if o.jump() == 1 && o.t_jump() > 0 {
            let ix = o.ix();
            w.play_sound(ix, 7);
            let o = &mut w.objs[i];
            o.set_run_counter(0);
            o.set_frame(0xd5);
            o.set_vx(((o.facing() as i32) * -2 + 1) as f64 * v(d.dash_distance));
            o.set_vy(v(d.dash_height));
            if o.up() != 0 && o.down() == 0 {
                o.set_vz(-v(d.dash_distancez));
            } else if o.up() == 0 && o.down() != 0 {
                o.set_vz(v(d.dash_distancez));
            }
        }
    }

    // state 2, heavy weapon
    {
        let o = &mut w.objs[i];
        if st(o) == 2 && o.weapon_type() == 2 {
            o.set_u_88(0);
            let rate = rfr(d);
            let c = (o.walk_counter() + 1) % (rate * 4);
            o.set_walk_counter(c);
            if c < rate * 3 {
                o.set_frame(c / rate + 0x10);
            } else {
                o.set_frame(0x11);
            }
            if o.facing() == 0 {
                o.set_vx(v(d.heavy_running_speed));
                if o.left() == 1 {
                    o.set_frame(0x13);
                }
            }
            if o.facing() == 1 {
                o.set_vx(-v(d.heavy_running_speed));
                if o.right() == 1 {
                    o.set_frame(0x13);
                }
            }
            if o.up() == 1 && o.down() == 0 && o.iy() == 0 {
                o.set_vz(-v(d.heavy_running_speedz));
                o.set_vx(o.vx() / 1.2);
            }
            if o.up() == 0 && o.down() == 1 && o.iy() == 0 {
                o.set_vz(v(d.heavy_running_speedz));
                o.set_vx(o.vx() / 1.2);
            }
            if o.attack() == 1 && o.t_attack() > 0 {
                o.set_frame(0x32);
            }
        }
    }

    // frame 215 (landing from a jump): defend, dash, z steering
    if w.objs[i].frame() == 0xd7 {
        {
            let o = &mut w.objs[i];
            if o.defend() == 1 && o.t_defend() > 0 {
                o.set_frame(0x66);
            }
        }
        if w.objs[i].jump() != 0 {
            let o = &w.objs[i];
            if (o.right() != 0 || o.vx() > 0.001) && o.t_jump() > 0 {
                let ix = o.ix();
                w.play_sound(ix, 7);
                let o = &mut w.objs[i];
                o.set_frame(o.facing() as i32 + 0xd5);
                o.set_run_counter(0);
                o.set_vx(v(d.dash_distance));
                o.set_vy(v(d.dash_height));
            }
            let o = &w.objs[i];
            if o.jump() != 0 && (o.left() != 0 || o.vx() < -0.001) && o.t_jump() > 0 {
                let ix = o.ix();
                w.play_sound(ix, 7);
                let o = &mut w.objs[i];
                o.set_frame(0xd6 - o.facing() as i32);
                o.set_run_counter(0);
                o.set_vx(-v(d.dash_distance));
                o.set_vy(v(d.dash_height));
            }
        }
        let o = &mut w.objs[i];
        if o.up() != 0 && o.down() == 0 {
            o.set_vz(-v(d.dash_distancez));
        } else if o.up() == 0 && o.down() != 0 {
            o.set_vz(v(d.dash_distancez));
        }
    }

    // frames 182/188 (falling): rowing with jump
    {
        let o = &mut w.objs[i];
        let f = o.frame();
        if (f == 0xb6 || f == 0xbc) && o.m.i32(0x320) >= 0 && o.jump() == 1 && o.t_jump() > 0 && o.hp() > 0 {
            let face = o.facing();
            let vx = o.vx();
            if (face == 0 && vx <= 0.0) || (face == 1 && vx >= 0.0) {
                o.set_frame(100);
            } else {
                o.set_frame(0x6c);
            }
            o.set_u_88(0);
            let rh = v(d.rowing_height);
            if rh < o.vy() {
                o.set_vy(rh);
            }
            let rd = v(d.rowing_distance);
            if 1.0 <= o.vx() || o.vx() <= -1.0 {
                o.set_vx(if o.vx() <= 0.0 { -rd } else { rd });
            } else if face == 1 {
                o.set_vx(rd);
            } else {
                o.set_vx(-rd);
            }
        }
    }

    // state 5 (dashing): turn, dash attack
    {
        let o = &mut w.objs[i];
        if st(o) == 5 {
            let right = o.right();
            if right == 1 && o.left() == 0 {
                o.set_facing(0);
            }
            if o.facing() == 0 && o.frame() != 0xd9 && o.vx() < 0.0 {
                o.set_frame(0xd6);
            }
            if o.facing() == 0 && o.frame() != 0xd8 && 0.0 < o.vx() {
                o.set_frame(0xd5);
            }
            if o.left() == 1 && right == 0 {
                o.set_facing(1);
            }
            let face = o.facing();
            if face == 1 {
                if o.frame() != 0xd9 && 0.0 < o.vx() {
                    o.set_frame(0xd6);
                }
                if o.frame() != 0xd8 && o.vx() < 0.0 {
                    o.set_frame(0xd5);
                }
            }
            if ((0.0 < o.vx() && face == 0) || (o.vx() < 0.0 && face == 1)) && o.attack() != 0 {
                let wt = o.weapon_type();
                if wt == 0 {
                    if !mp_cost_on {
                        o.set_frame(0x5a);
                    } else if d.frames[0x5a].mp <= o.mp() {
                        let c = d.frames[0x5a].mp;
                        o.add_mp(-c);
                        o.m.add_i32(0x350, c);
                        o.set_frame(0x5a);
                    }
                } else if wt % 100 == 1
                    || ((wt == 4 || wt == 6) && (o.left() != 0 || right != 0 || o.up() != 0 || o.down() != 0))
                {
                    o.set_frame(if wt == 4 || wt == 6 { 0x34 } else { 0x28 });
                    o.set_u_88(0);
                    o.set_vy(o.vy() - 1.0);
                }
            }
        }
    }

    // the frame's dvx, dvy, dvz
    let o = &mut w.objs[i];
    let fr = &d.frames[o.frame() as usize];
    let dvx = fr.dvx;
    if dvx < 0x1f5 {
        let face = o.facing();
        if dvx > 0 && o.vx() < dvx as f64 && face == 0 {
            o.set_vx(dvx as f64);
        }
        if dvx > 0 && -o.vx() < dvx as f64 && face == 1 {
            o.set_vx(-(dvx as f64));
        }
        if dvx < 0 && (dvx as f64) < o.vx() && face == 0 {
            o.set_vx(dvx as f64);
        }
        if dvx < 0 && (dvx as f64) < -o.vx() && face == 1 {
            o.set_vx(-(dvx as f64));
        }
    } else {
        o.set_vx((dvx - 0x226) as f64);
    }
    let dvy = fr.dvy;
    if dvy != 0 {
        if dvy < 0x1f5 {
            o.set_vy(dvy as f64 + o.vy());
        } else {
            o.set_vy((dvy - 0x226) as f64);
        }
    }
    let dvz = fr.dvz;
    if dvz != 0 {
        if dvz > 500 {
            o.set_vz((dvz - 0x226) as f64);
            return;
        }
        if o.up() == 1 && o.t_down() <= o.t_up() {
            o.set_vz(-(dvz as f64));
        }
        if o.down() == 1 && o.t_up() <= o.t_down() {
            o.set_vz(dvz as f64);
        }
    }
}

