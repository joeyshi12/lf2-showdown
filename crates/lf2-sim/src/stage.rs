//! Stage mode: `stage_mode_tick` (0x437860), `stage_phase` (0x437400, spawn one
//! object of a spawn slot) and `stage_spawn` (0x436fc0, start the next stage).
//!
//! The original keeps a counter block in front of each spawn slot of stage.dat in the
//! file manager (`+0x810 + slot*0xe0` of the phase): objects spawned so far, objects to
//! spawn in total, objects alive at once, and the object index of each of those. Only
//! the current phase's blocks are read, and they are set when the phase starts, so the
//! port keeps one set of 60 in `World::spawn_rt`.
//!
//! The fades, the "GO" arrow and the texts (stage number, "Man:" and "HP:" lines) are
//! drawn by the same function; that part is left to the renderer.

use crate::mem::ftol;
use crate::world::{World, MAX_OBJECTS};
use lf2_data::stage::{Phase, Spawn, MAX_SPAWNS};
use crate::menu::main_menu::MENU_CLIP3;

/// MENU_CLIP2 (the "Survival" stage title is its cell 3).
const MENU_CLIP2: u32 = 0x451168;

/// The counter block in front of a spawn slot (file manager phase `+0x810 + k*0xe0`).
#[derive(Clone, Debug, PartialEq)]
pub struct SpawnRt {
    /// Objects of this slot spawned so far (`[0]`).
    pub count: i32,
    /// Objects to spawn in total (`[1]`).
    pub total: i32,
    /// Objects alive at once (`[2]`, at most 40).
    pub alive: i32,
    /// Object index of each of those, -1 for none (`[3..43]`).
    pub objs: [i32; 40],
}

impl Default for SpawnRt {
    fn default() -> Self {
        SpawnRt { count: 0, total: 0, alive: 0, objs: [-1; 40] }
    }
}

const RESERVED: u32 = 0x4514e0;
const RANDOM_IDS: u32 = 0x44d324;

fn empty_spawn(ph: &Phase) -> Spawn {
    Spawn { x: ph.default_x, ..Default::default() }
}

impl World {
    /// Phase count of stage `n`, -1 for a stage the file does not have.
    fn stage_phase_count(&self, n: i32) -> i32 {
        self.gd.stages.iter().find(|s| s.id == n).map_or(-1, |s| s.phases.len() as i32)
    }

    /// Phase `p` of stage `n` as the file manager holds it (defaults past the end).
    fn stage_phase_data(&self, n: i32, p: i32) -> Phase {
        let st = self.gd.stages.iter().find(|s| s.id == n);
        match st {
            Some(st) => st.phases.get(p as usize).cloned().unwrap_or_default(),
            // a stage slot the file did not fill is all zeros after the count
            None => Phase { bound: 0, when_clear_goto_phase: 0, ..Default::default() },
        }
    }

    fn stage_spawn_data(ph: &Phase, k: usize) -> Spawn {
        ph.spawns.get(k).cloned().unwrap_or_else(|| empty_spawn(ph))
    }

    fn reserved(&self, j: usize) -> bool {
        self.g.m.i32(RESERVED + 4 * j as u32) != 0
    }

    /// 0x437400: spawn object `k` of spawn slot `slot` (data `sp`).
    fn stage_phase(&mut self, slot: usize, sp: &Spawn, k: usize) {
        let Some(j) = (0x14..MAX_OBJECTS).find(|&j| !self.exists(j) && !self.reserved(j)) else {
            self.spawn_rt[slot].objs[k] = -1;
            return;
        };
        let mut hp = sp.hp;
        let diff = self.g.difficulty();
        if diff == -1 && sp.id != 300 {
            hp = ftol(hp as f64 * 1.5);
        } else if diff == 2 && sp.id != 300 {
            hp = ftol(hp as f64 * 0.75);
        }
        let id = match sp.id {
            1000 => {
                let next = self.g.stage_random_next();
                if next == 10 || next == -1 {
                    for _ in 0..50 {
                        let a = self.random(0x10f, 10) as u32;
                        let b = self.random(0x110, 10) as u32;
                        let va = self.g.m.i32(RANDOM_IDS + 4 * a);
                        let vb = self.g.m.i32(RANDOM_IDS + 4 * b);
                        self.g.m.set_i32(RANDOM_IDS + 4 * a, vb);
                        self.g.m.set_i32(RANDOM_IDS + 4 * b, va);
                    }
                    self.g.set_stage_random_next(0);
                }
                let n = self.g.stage_random_next();
                self.g.set_stage_random_next(n + 1);
                self.g.m.i32(RANDOM_IDS + 4 * n as u32)
            }
            3000 => self.random(0x111, 2) + 0x1e,
            3001 => {
                if self.random(0x112, 7) == 0 {
                    hp *= 4;
                    0x20
                } else {
                    self.random(0x113, 2) + 0x1e
                }
            }
            id => id,
        };
        let Some(data) = self.data_index(id) else {
            self.spawn_rt[slot].objs[k] = -1;
            return;
        };
        self.spawn_rt[slot].objs[k] = j as i32;
        self.set_exists(j, true);
        let weapon_hp = self.gd.objects[data].weapon_hp;
        let kind = self.gd.objects[data].kind;
        let (z0, z1) = self.background().zboundary;
        {
            let o = &mut self.objs[j];
            crate::collisions::obj_reset(o);
            o.m.set_i32(0x368, data as i32);
            o.set_x(350.0);
            o.set_weapon_hp(weapon_hp);
            o.set_y(0.0);
            o.set_z(300.0);
        }
        let r = self.random(0x114, z1 - z0);
        self.objs[j].set_iz(r + z0);
        let ix = if sp.x == -1000 {
            if self.random(0x116, 2) == 0 {
                self.random(0x117, 300) + 0x96 + self.g.u_450bb4()
            } else {
                -0x96 - self.random(0x118, 300)
            }
        } else {
            self.random(0x115, 300) + sp.x
        };
        let bb4 = self.g.u_450bb4();
        let o = &mut self.objs[j];
        o.set_ix(ix);
        o.m.set_i32(0x30c, sp.reserve);
        o.m.set_i32(0x314, sp.join);
        o.m.set_i32(0x310, sp.join_reserve);
        o.set_x(o.ix() as f64);
        o.set_z(o.iz() as f64);
        o.set_mp(500);
        o.set_dark_hp(hp);
        o.set_hp(hp);
        o.set_max_hp(hp);
        o.m.set_i32(0x354, j as i32);
        if kind == 0 || kind == 5 {
            o.set_u_08(0x14);
            o.m.set_i8(0x80, (bb4 - 0x31a < o.ix()) as i8);
            o.set_frame(sp.act);
            o.set_team(5);
            o.set_iy(sp.y);
            o.set_y(o.iy() as f64);
            o.set_vy(0.0);
        } else {
            o.set_u_08(0);
            o.set_team(0);
            o.set_iy(-300);
            o.set_y(o.iy() as f64);
        }
        if id == 0x7a {
            o.set_hp(200);
        }
        self.spawn_rt[slot].count += 1;
    }

    /// 0x436fc0: go on to the next stage.
    fn stage_spawn(&mut self) {
        self.g.m.add_i32(0x450b94, 1);
        let mut per_owner = [0i32; 20];
        self.g.set_stage_go_timer(0x46);
        self.g.set_stage_state(0);
        self.g.set_u_450bac(0);
        self.g.set_stage_phase(-1);
        self.g.set_survival_phase(-1);
        self.g.m.set_i32(crate::camera::CAMERA_X + 4, 0);
        self.g.m.set_i32(crate::camera::CAMERA_X, 0);
        let (z0, z1) = self.background().zboundary;
        let diff = self.g.difficulty();
        for i in 0..MAX_OBJECTS {
            if !self.exists(i) || self.data_of(i).kind != 0 {
                continue;
            }
            let r = self.random(0x10d, 0x1e);
            {
                let o = &mut self.objs[i];
                o.set_ix(r + 0x32);
                o.set_x(o.ix() as f64);
                o.set_dark_hp(o.dark_hp() + (diff + 2) * 0x32);
                if o.max_hp() < o.dark_hp() {
                    o.set_dark_hp(o.max_hp());
                }
                o.set_hp(o.dark_hp());
                o.set_mp(500);
                if 8 < o.frame() && o.frame() < 0xc {
                    o.set_frame(0);
                }
                if 0xf < o.frame() && o.frame() < 0x13 {
                    o.set_frame(0xc);
                }
            }
            let r = self.random(0x10e, z1 - z0);
            let o = &mut self.objs[i];
            o.set_iz(r + z0);
            o.set_z(o.iz() as f64);
        }
        for o in self.objs.iter_mut() {
            o.set_cmd_x(-1000);
            o.set_cmd_z(-1000);
        }
        // keep characters, held items and two summoned objects per owner
        for j in 20..MAX_OBJECTS {
            let keep = !self.exists(j) || self.data_of(j).kind < 1 || self.objs[j].weapon_type() < 0;
            if keep {
                let owner = self.objs[j].m.i32(0x2f4);
                if (0..20).contains(&owner) {
                    if 2 <= per_owner[owner as usize] {
                        self.set_exists(j, false);
                    } else {
                        per_owner[owner as usize] += 1;
                    }
                }
            } else {
                self.set_exists(j, false);
            }
        }
    }

    /// 0x437860. `phase` is `DAT_0044d020`, which the function can set to 300.
    pub(crate) fn stage_mode_tick(&mut self) {
        for j in 0..MAX_OBJECTS as u32 {
            self.g.m.set_i32(RESERVED + 4 * j, 0);
        }
        let state = self.g.stage_state();
        if 2 < state {
            if state == 3 {
                self.g.set_end_timer(0);
                let fade = self.g.stage_fade() as u32;
                if fade < 0x15 {
                    // the screen below the status panels goes black in 10 steps
                    if fade < 10 {
                        self.stage_fade_bars(fade as i32 * 0x2b / 10);
                    } else {
                        self.fill(0, 0x6d, 0x31a, 0x1b8, 0);
                    }
                    self.g.set_stage_fade(fade as i32 + 1);
                    if fade + 1 == 0x15 {
                        let b94 = self.g.stage();
                        if b94 / 10 == 4 {
                            if self.g.demo() == 0 || self.g.match_phase() == 10 {
                                self.g.set_match_phase(300);
                                return;
                            }
                        } else {
                            self.g.set_stage(((b94 / 10) * 5 + 5) * 2);
                            self.g.set_u_450b98(1);
                        }
                        self.g.set_end_timer(0x15e);
                    }
                }
            }
            return;
        }
        let b94 = self.g.stage();
        if self.g.stage_phase() == -1 && b94 == (b94 / 10) * 10 {
            self.g.set_stage_random_next(-1);
        }
        let count = self.stage_phase_count(b94);
        let cur = self.g.stage_phase();
        let cur_phase = self.stage_phase_data(b94, cur);
        let has_next = cur < count - 1 || (-1 < cur && cur_phase.when_clear_goto_phase != -1);
        if has_next && (cur == -1 || self.g.u_451b34() == 1) {
            self.g.set_u_451b34(0);
            self.g.set_u_451b30(0);
            self.g.set_u_450bac(0);
            let mut players = 0;
            for j in 0..20 {
                if self.exists(j) && self.data_of(j).kind == 0 {
                    players += match self.data_of(j).id {
                        0x33 => 2,
                        0x34 => 3,
                        _ => 1,
                    };
                }
            }
            if self.g.difficulty() == -1 {
                players = ftol(players as f64 * 1.5 + 1.0);
            }
            self.g.set_survival_phase(self.g.survival_phase() + 1);
            if b94 / 10 == 5 {
                self.g.set_survival_blink(1);
            }
            let next = if cur < 0 || cur_phase.when_clear_goto_phase == -1 { cur + 1 } else { cur_phase.when_clear_goto_phase };
            self.g.set_stage_phase(next);
            let ph = self.stage_phase_data(b94, next);
            if !ph.music.is_empty() {
                let m = ph.music.clone();
                self.play_music(&m);
            }
            self.g.set_u_450bb0(ph.bound - 0x31a);
            self.g.set_u_450bb4(ph.bound);
            if b94 / 10 == 5 {
                for j in 0..20 {
                    let o = &mut self.objs[j];
                    if o.hp() < 1 {
                        if o.dark_hp() < 5 {
                            o.set_dark_hp(5);
                        }
                        o.set_hp(5);
                        o.set_mp(500);
                        o.set_ix(100);
                        o.set_x(100.0);
                    }
                }
            }
            for k in 0..MAX_SPAWNS {
                let sp = Self::stage_spawn_data(&ph, k);
                self.spawn_rt[k].count = 0;
                if sp.id < 0 {
                    continue;
                }
                if sp.ratio <= 0.0 {
                    self.spawn_rt[k].total = sp.times;
                    self.spawn_rt[k].alive = 1;
                } else {
                    self.spawn_rt[k].total = ftol(sp.times as f64 * players as f64 * sp.ratio);
                    self.spawn_rt[k].alive = ftol(players as f64 * sp.ratio).min(0x28);
                }
                for n in 0..self.spawn_rt[k].alive.max(0) as usize {
                    self.stage_phase(k, &sp, n);
                }
            }
        }
        self.g.set_u_451b34(1);
        self.g.set_u_451b30(0);
        let cur = self.g.stage_phase();
        if -1 < cur {
            let ph = self.stage_phase_data(self.g.stage(), cur);
            for k in 0..MAX_SPAWNS {
                let sp = Self::stage_spawn_data(&ph, k);
                if sp.id < 0 || sp.role <= 1 {
                    continue;
                }
                for n in 0..self.spawn_rt[k].alive.max(0) as usize {
                    let o = self.spawn_rt[k].objs[n];
                    if o < 0 {
                        continue;
                    }
                    self.g.m.set_i32(RESERVED + 4 * o as u32, 1);
                    let o = o as usize;
                    if self.exists(o) && self.objs[o].team() == 5 && self.data_of(o).kind == 0 {
                        self.g.set_u_451b30(1);
                    }
                }
            }
            for pass in 0..2 {
                for k in 0..MAX_SPAWNS {
                    let sp = Self::stage_spawn_data(&ph, k);
                    if sp.id < 0 {
                        continue;
                    }
                    let mut n = 0;
                    while n < self.spawn_rt[k].alive.max(0) as usize {
                        let o = self.spawn_rt[k].objs[n];
                        if -1 < o && !self.exists(o as usize) {
                            let rt = &mut self.spawn_rt[k];
                            if pass == 0 {
                                if sp.role == 1 {
                                    if self.g.u_451b30() == 0 || rt.total <= rt.count {
                                        rt.objs[n] = -1;
                                        rt.count = rt.total;
                                    }
                                } else if rt.total <= rt.count {
                                    rt.objs[n] = -1;
                                    rt.count = rt.total;
                                }
                            } else if rt.count < rt.total {
                                self.stage_phase(k, &sp, n);
                                self.g.set_u_451b34(0);
                            }
                        }
                        n += 1;
                    }
                }
            }
        }
        let enemies_left = (0x14..MAX_OBJECTS).any(|j| self.exists(j) && self.data_of(j).kind == 0 && self.objs[j].team() == 5);
        if enemies_left {
            self.g.set_u_451b34(0);
        }
        self.stage_after_spawns(enemies_left);
        self.stage_lives();
    }

    /// 0x438124..0x438614: the "GO" timer, the move to the next stage and the fades.
    fn stage_after_spawns(&mut self, enemies_left: bool) {
        enum At {
            Go,       // LAB_00438236
            L8311,    // LAB_00438311
            L831e,    // LAB_0043831e
            L854d,    // LAB_0043854d
            FadeIn,   // LAB_004385c0
            Done,     // LAB_00438614
        }
        let mut at;
        if !enemies_left && self.g.u_451b34() == 1 && self.g.stage_go_timer() == 0 {
            if self.g.u_450bac() != 0 {
                at = At::L8311;
            } else {
                self.g.set_stage_go_timer(0xa9);
                let last = self.stage_phase_count(self.g.stage()) - 1;
                if self.g.stage_phase() as u32 == last as u32 {
                    self.g.set_u_450bac(1);
                    self.g.set_u_450bb4(0);
                }
                at = At::Go;
            }
        } else {
            let t = self.g.stage_go_timer();
            if 0 < t && t < 100 {
                // the stage title: "STAGE n-m" in MENU_CLIP3 cells, or MENU_CLIP2 cell 3
                let b94 = self.g.stage();
                if b94 / 10 < 5 {
                    self.blit(MENU_CLIP3, 0x109, 299, 0x19, true);
                    self.blit(MENU_CLIP3, 0x1ea, 299, 0x24, true);
                    self.blit(MENU_CLIP3, 0x1cc, 299, b94 / 10 + 0x1b, true);
                    self.blit(MENU_CLIP3, 0x208, 299, b94 % 10 + 0x1b, true);
                } else {
                    self.blit(MENU_CLIP2, 0xa5, 299, 3, true);
                }
                self.g.set_stage_go_timer(t - 1);
            }
            let t = self.g.stage_go_timer();
            at = if 100 < t && t < 300 { At::Go } else { At::L8311 };
        }
        loop {
            match at {
                At::Go => {
                    let cur = self.g.stage_phase();
                    if (cur as u32) < 99 && self.g.u_450bac() != 1 {
                        let b94 = self.g.stage();
                        if self.stage_phase_data(b94, cur).bound == self.stage_phase_data(b94, cur + 1).bound {
                            self.g.set_stage_go_timer(0);
                        }
                    }
                    let t = self.g.stage_go_timer();
                    if (t / 10) % 2 == 1 {
                        if t % 10 == 0 && t < 200 {
                            self.buffer_sounds.push("data\\m_ok.wav");
                        }
                        // the "GO" sign, shown every other 10 frames
                        self.blit(MENU_CLIP3, 0x294, 299, 0x18, true);
                    }
                    let t = t - 1;
                    self.g.set_stage_go_timer(t);
                    if t != 0x65 && t != 0xc9 {
                        at = At::L8311;
                    } else if self.g.u_450bac() != 1 {
                        self.g.set_stage_go_timer(0);
                        at = At::L8311;
                    } else {
                        self.g.set_stage_go_timer(0x118);
                        at = At::L831e;
                    }
                }
                At::L8311 => {
                    at = if self.g.u_450bac() == 1 { At::L831e } else { At::L854d };
                }
                At::L831e => {
                    if self.g.stage_fade() != 0 {
                        at = At::L854d;
                        continue;
                    }
                    let b94 = self.g.stage();
                    let to_clear = b94 % 10 == 9 || self.stage_phase_count(b94 + 1) == -1;
                    if !to_clear {
                        let bound = self.stage_phase_data(b94, self.g.stage_phase()).bound;
                        let behind = (0..20).any(|j| {
                            self.exists(j) && self.data_of(j).kind == 0 && 0 < self.objs[j].hp() && self.objs[j].ix() < bound
                        });
                        if behind {
                            at = At::Done;
                        } else {
                            self.g.set_stage_fade(1);
                            at = At::FadeIn;
                        }
                        continue;
                    }
                    // all stages of the chapter cleared
                    if self.g.end_timer() == 1 {
                        self.g.set_slow_frames(1);
                        self.music_stop();
                        self.buffer_sounds.push("data\\m_pass.wav");
                    }
                    if self.g.end_timer() < 0x5a {
                        // "STAGE CLEAR"
                        self.blit(MENU_CLIP3, 0xd7, 0x12a, 0x25, true);
                    }
                    if self.g.stage_state() == 0 {
                        self.g.set_stage_state(1);
                    }
                    self.g.set_u_450bac(0);
                    self.g.set_stage_go_timer(0);
                    self.g.set_stage_fade(0);
                    at = At::Done;
                }
                At::L854d => {
                    let f = self.g.stage_fade();
                    if (f.wrapping_sub(0xb) as u32) < 10 {
                        // the black bars shrink again
                        self.stage_fade_bars((0x15 - f) * 0x2b / 10);
                        self.g.set_stage_fade(f + 1);
                        if f + 1 == 0x15 {
                            self.g.set_stage_fade(0);
                            at = At::Done;
                            continue;
                        }
                    }
                    let f = self.g.stage_fade();
                    at = if 0 < f && f < 0xb { At::FadeIn } else { At::Done };
                }
                At::FadeIn => {
                    // black bars grow across the screen; the next phase spawns behind them
                    self.stage_fade_bars(self.g.stage_fade() * 0x2b / 10);
                    let f = self.g.stage_fade() + 1;
                    self.g.set_stage_fade(f);
                    if f == 0xb {
                        self.stage_spawn();
                    }
                    at = At::Done;
                }
                At::Done => return,
            }
        }
    }

    /// 0x438614: lives of the players' characters, and the HUD lines. While a player
    /// of team != 5 is alive, characters waiting with negative lives come back; when
    /// none is, the lives of those with 2 or more are made negative.
    fn stage_lives(&mut self) {
        let any = (0..20).any(|j| {
            self.exists(j) && self.data_of(j).kind == 0 && 0 < self.objs[j].hp() && self.objs[j].team() != 5
        });
        let (mut men, mut hp, mut reserve, mut foes, mut foe_hp) = (0, 0, 0, 0, 0);
        for j in 0..MAX_OBJECTS {
            if !self.exists(j) || self.data_of(j).kind != 0 {
                continue;
            }
            let o = &self.objs[j];
            if o.team() == 5 {
                if 0 < o.hp() {
                    foe_hp += o.hp();
                    foes += 1;
                }
                continue;
            }
            if 0 < o.hp() {
                hp += o.hp();
                men += 1;
            }
            let lives = o.m.i32(0x30c);
            if 2 <= lives && any {
                reserve += lives - 1;
            }
            let negate = if lives < 2 { any && lives < 0 } else { !any };
            if negate {
                self.objs[j].m.set_i32(0x30c, -lives);
            }
        }
        // the lines are sprintf'd into 0x451418 and drawn (colours are COLORREFs)
        let b94 = self.g.stage();
        if b94 < 0x32 {
            self.hud_text(&format!("STAGE {}-{}", b94 / 10 + 1, b94 % 10 + 1));
            self.hud_draw(0xc8c8c8, 0x168);
        } else if 0 <= self.g.survival_phase() {
            let blink = self.g.survival_blink();
            if 0 < blink {
                self.g.set_survival_blink(blink + 1);
            }
            if 0x45 < self.g.survival_blink() {
                self.g.set_survival_blink(0);
            }
            self.hud_text(&format!("Survival Stage: {}", self.g.survival_phase()));
            // a new survival stage blinks red
            let blink = self.g.survival_blink();
            let color = if blink == 0 || (blink / 10) % 2 == 1 { 0xc8c8c8 } else { 0xc85a5a };
            self.hud_draw(color, 0x154);
        }
        if reserve == 0 {
            self.hud_text(&format!("Man: {men:3}      HP: {hp:4}"));
        } else {
            self.hud_text(&format!("Man: {men:3}      HP: {hp:4}     Reserve: {reserve:3}"));
        }
        self.hud_draw(0xff7878, 10);
        self.hud_text(&format!("Man: {foes:3}      HP: {foe_hp:4}"));
        self.hud_draw(0xff00ff, 0x285);
    }

    /// draw_text of the HUD line buffer at (x, 0x6e).
    fn hud_draw(&mut self, color: u32, x: i32) {
        let t: Vec<u8> = (0..0x100).map(|k| self.g.m.u8(0x451418 + k)).take_while(|&c| c != 0).collect();
        self.text(&t, 0, color, x, 0x6e);
    }

    /// Black bars of height `h` every 0x2b pixels from y 0x87 (the fades between
    /// stage phases and at the end of a stage).
    fn stage_fade_bars(&mut self, h: i32) {
        for y in (0x87..0x235).step_by(0x2b) {
            self.fill(0, y, 0x31a, h, 0);
        }
    }

    /// sprintf into the HUD line buffer 0x451418 (bytes after the terminator stay).
    pub(crate) fn hud_text(&mut self, s: &str) {
        for (k, b) in s.bytes().chain(std::iter::once(0)).enumerate() {
            self.g.m.set_u8(0x451418 + k as u32, b);
        }
    }

    /// 0x41d75b: in stage mode the match is decided when no player character is alive
    /// or the stage state is past 0.
    pub(crate) fn stage_end_checks(&mut self) {
        if self.g.match_phase() != 0 {
            return;
        }
        let none_alive = !(0..MAX_OBJECTS).any(|j| {
            if !self.exists(j) {
                return false;
            }
            let o = &self.objs[j];
            let t = o.team();
            0 < o.hp() && self.data_of(j).kind == 0 && 0 < t && t < 0x28 && t != 5
        });
        if none_alive || 0 < self.g.stage_state() {
            self.g.set_end_timer(self.g.end_timer() + 1);
            if self.g.stage_state() < 1 && self.g.end_timer() == 0x50 {
                self.g.set_slow_frames(1);
                self.music_stop();
                self.buffer_sounds.push("data\\m_end.wav");
            }
        }
    }
}
