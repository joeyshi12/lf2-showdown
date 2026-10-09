//! One frame of a match (`match_frame`, 0x41bc90), the parts that change the
//! simulation. Drawing, sound, network play, recording files and the F-key menus are
//! not here.
//!
//! Sections that are not ported yet call [`World::unported`], which records the
//! section so a differential test can say where a difference may come from.

use crate::controls::obj_controls;
use crate::movement::obj_move;
use crate::world::{World, CONTROLS, KEYS, MAX_OBJECTS, PLAYER_OF_SLOT};

/// Joystick state, 0x30 bytes per joystick: up, down, right, left, then buttons.
const JOYSTICKS: u32 = 0x453ff0;

/// Key array entry of a virtual key.
fn key_at(vk: u32) -> u32 {
    KEYS + vk
}

/// `DAT_004593a4`: progress through the A, B, C key sequence that unlocks the tool
/// modes; `DAT_00450bec` is set once it is complete.
const ABC_STATE: u32 = 0x4593a4;
const ABC_DONE: u32 = 0x450bec;
/// `DAT_004593a0`: 0 game, 1 data editor, 2 encryption tool.
const TOOL_MODE: u32 = 0x4593a0;

impl World {
    /// One call of the frame function (0x43e9a0) while a match is running: the tool-mode
    /// key check, then `match_frame`.
    pub fn step(&mut self) {
        self.buffer_sounds.clear();
        // sounds are per frame: a frame that does not reach flush_sounds plays none
        self.sounds.clear();
        self.draw.clear();
        // requests stay until the platform takes them (some come between frames)
        self.music.clear();
        for vk in 0..0xfau32 {
            if self.g.m.u8(KEYS + vk) != b'd' {
                continue;
            }
            let st = self.g.m.i32(ABC_STATE);
            let next = match (st, vk) {
                (0, 0x41) => 1,
                (0, _) => 0,
                (1, 0x42) => 2,
                (1, 0x41) => 1,
                (2, 0x43) => {
                    self.g.m.set_i32(ABC_DONE, 1);
                    3
                }
                (2, 0x42) => 2,
                (3, 0x43) => 3,
                _ => 0,
            };
            self.g.m.set_i32(ABC_STATE, next);
        }
        if self.g.m.i32(ABC_DONE) != 0 {
            for (vk, mode) in [(0x70u32, 0), (0x71, 1), (0x72, 2)] {
                if self.g.m.u8(KEYS + vk) == b'd' {
                    self.g.m.set_i32(TOOL_MODE, mode);
                }
            }
        }
        if self.g.m.i32(0x44dce4) == 1 || self.g.m.i32(TOOL_MODE) != 0 {
            self.unported("tool modes (0x43ea6x)");
            return;
        }
        match self.g.m.i32(crate::world::GAME) {
            0 => {
                self.draw.clear();
                self.title_menus();
                return;
            }
            1 => {
                // 0x4246b0, state 1: the "loading" picture, then the match frames
                self.free_menu_background();
                self.blit(0x45118c, 0, 0, -1, false);
                self.g.m.set_i32(crate::world::GAME, 2);
                self.recording_messages();
                return;
            }
            _ => {}
        }
        if self.g.match_first_frame() == 1 {
            // the first match frame loads all data, then goes on (the main menu)
            self.data_loaded();
        }
        self.match_frame();
        if !self.net_waiting {
            self.g.m.set_i32(0x457580, 0);
        }
    }

    pub fn unported(&mut self, section: &'static str) {
        if !self.unported_hits.contains(&section) {
            self.unported_hits.push(section);
        }
    }

    /// 0x41bc90 from the top: everything a match frame does to the simulation.
    pub fn match_frame(&mut self) {
        // 0x41bd02: half_frame toggles; on 0 the network flags are copied
        let hf = (self.g.half_frame() + 1) & 1;
        self.g.set_half_frame(hf);
        if hf == 0 {
            let v = self.g.m.i32(0x44fb60);
            self.g.m.set_i32(0x450bfc, v);
            let v = self.g.m.i32(0x44fcb0);
            self.g.m.set_i32(0x44fb60, v);
        }
        // `local_60c`: a network frame with nothing to simulate
        let net_wait = self.g.m.i32(0x450bfc) == 1;
        if self.g.replaying() != 0 {
            self.replay_camera_keys();
        }
        self.rec_frame = [0; 10];
        self.replay_frame = [0; 10];
        if self.g.replaying() != 0 && !net_wait {
            self.replay_read_frame();
        }
        // 0x41c583: the outgoing network packet starts from a constant every frame
        for k in 0..20u32 {
            self.g.m.set_u8(crate::net::OUT + k, 1);
        }
        self.g.m.set_u8(crate::net::OUT + 20, 0);
        if !net_wait {
            self.read_inputs_and_ai(hf);
        }
        // F-keys (pause, single step, cheats) and network exchange (0x41bf36)
        if hf == 0 {
            if self.net_role() > 0 {
                // the frame stops here until the platform has exchanged the packets
                self.net_out = Some(self.net_build_packet());
                self.net_waiting = true;
                return;
            } else if self.g.demo() != 0 {
                self.fkeys_replay();
            } else {
                self.fkeys();
            }
        }
        self.match_frame_after_exchange();
    }

    /// The rest of match_frame after the F-keys or the network exchange (0x41cf12).
    pub(crate) fn match_frame_after_exchange(&mut self) {
        let hf = self.g.half_frame();
        if hf == 0 && self.net_role() > 0 {
            self.net_apply_packet();
        }
        let net_wait = self.g.m.i32(0x450bfc) == 1;
        if !net_wait {
            self.read_remote_inputs(hf);
            self.replay_and_record(hf);
        }
        // 0x41d6b9: frame counter (recording a .lfr also happens here)
        if !net_wait {
            let n = self.g.match_frames();
            if n < 0x9e33f {
                self.g.set_match_frames((n + 1).min(0x9e33f));
            } else {
                self.g.set_match_frames(0x9e33f);
            }
        }
        let two = (self.g.m.i32(0x450bfc) == 2) as i32;
        self.g.m.set_i32(0x450c00, two);
        if net_wait {
            // paused (F1) or waiting for the network peer: only drawing happens
            self.g.m.set_i32(0x44d02c, 1);
            return;
        }
        self.tick();
    }

    /// 0x419a60: copy the keys of players 1-4 into objects 0-7 (only on frames where
    /// `half_frame` is 0), then run the computer players.
    fn read_inputs_and_ai(&mut self, half: i32) {
        if self.g.replaying() == 0 {
            for slot in 0..8usize {
                let player = self.g.m.i32(PLAYER_OF_SLOT + 4 * slot as u32);
                if player <= 0 {
                    continue;
                }
                let o = &mut self.objs[slot];
                for k in 0..7u32 {
                    let cur = o.m.u8(0xcd + k);
                    o.m.set_u8(0xc6 + k, cur);
                }
                if half != 0 {
                    continue;
                }
                for k in 0..7u32 {
                    o.m.set_u8(0xcd + k, 0);
                }
                if (player as u32).wrapping_sub(1) < 4 {
                    let base = CONTROLS + player as u32 * 0x50;
                    if self.g.m.i32(base) == 0 {
                        for k in 0..7u32 {
                            let vk = self.g.m.i32(base + 4 + 4 * k) as u32;
                            if self.g.m.u8(KEYS + (vk & 0xff)) == b'd' {
                                self.objs[slot].m.set_u8(0xcd + k, 1);
                            }
                        }
                    } else if 0 < self.g.m.i32(base) {
                        // joystick `kind`: direction bytes, then buttons chosen in
                        // control.txt (+0x20, +0x24, +0x28)
                        let off = ((self.g.m.i32(base) * 3 - 3) * 0x10) as u32;
                        let j = JOYSTICKS + off;
                        let pressed = [
                            self.g.m.u8(j) != 0,
                            self.g.m.u8(j + 1) != 0,
                            self.g.m.u8(j + 3) != 0,
                            self.g.m.u8(j + 2) != 0,
                            self.g.m.u8((JOYSTICKS + 4).wrapping_add(self.g.m.i32(base + 0x20) as u32) + off) != 0,
                            self.g.m.u8((JOYSTICKS + 4).wrapping_add(self.g.m.i32(base + 0x24) as u32) + off) != 0,
                            self.g.m.u8((JOYSTICKS + 4).wrapping_add(self.g.m.i32(base + 0x28) as u32) + off) != 0,
                        ];
                        for (k, p) in pressed.iter().enumerate() {
                            if *p {
                                self.objs[slot].m.set_u8(0xcd + k as u32, 1);
                            }
                        }
                    }
                    self.net_put_keys(slot);
                    if self.g.recording() != 0 {
                        // the frame's record: up down left right attack jump defend
                        let o = &self.objs[slot];
                        let mut b = 0u8;
                        for (k, bit) in [0x80u8, 0x40, 0x20, 0x10, 8, 4, 2].iter().enumerate() {
                            if o.m.u8(0xcd + k as u32) != 0 {
                                b |= bit;
                            }
                        }
                        self.rec_frame[slot] |= b;
                    }
                }
            }
        }
        let mode = self.g.game_mode();
        for i in 10..MAX_OBJECTS {
            if !self.exists(i) {
                continue;
            }
            if self.data_of(i).kind == 0 {
                self.ai_character(i, mode);
            } else if self.frame_of(i).hit_fa > 0 {
                self.ai_projectile(i);
            }
        }
    }

    /// From 0x41d70e: one simulation tick.
    fn tick(&mut self) {
        if self.g.m.i32(0x450bfc) == 2 {
            self.g.m.set_i32(0x450bfc, 1);
        }
        self.g.set_tick_parity(1 - self.g.tick_parity());
        self.g.set_tick_mod12((self.g.tick_mod12() + 1) % 12);
        self.g.set_tick_mod3((self.g.tick_mod3() + 1) % 3);
        self.match_end_checks();
        if self.g.end_timer() == 0x91 {
            if self.g.u_450c2c() == 0 {
                self.g.set_end_timer(0x90);
            }
        } else if self.g.end_timer() > 0x15d {
            // the frame ends here, without drawing or flushing sounds
            self.end_of_match();
            return;
        }
        if self.g.match_phase() != 0 {
            self.game_menus();
            // the first menu frame of a network game (0x44d058 starts at 1)
            if 0 < self.g.m.i32(0x44d058) && 0 < self.net_role() {
                self.fill(0, 0, 0x31a, 0x226, 0);
                self.blit(crate::menu::main_menu::MENU_CLIP3, 0xe6, 0xdd, 0, false);
                self.text(b"Waiting for opponent...", 0x652512, 0xd8775a, 0x10f, 0x127);
                self.g.m.set_i32(0x44d058, self.g.m.i32(0x44d058) - 1);
            }
            self.recording_messages();
            return;
        }
        if self.g.end_timer() > 0x8f {
            self.continue_after_match();
        }
        for i in 0..MAX_OBJECTS {
            if !self.exists(i) {
                continue;
            }
            obj_controls(self, i);
            let state = self.frame_of(i).state;
            if state == 400 && self.g.tick_parity() == 0 {
                self.teleport(i, 1);
            }
            if self.frame_of(i).state == 401 && self.g.tick_parity() == 0 {
                self.teleport(i, 2);
            }
            let o = &self.objs[i];
            if state == 500 && (o.m.i32(0x33c) == -1 || o.m.i32(0x324) >= 0) {
                self.objs[i].set_frame(0);
            }
            let o = &self.objs[i];
            if self.frame_of(i).state == 0x1f5 && o.m.i32(0x33c) >= 0 {
                self.transform_501(i);
            }
        }
        for i in 0..MAX_OBJECTS {
            if !self.exists(i) {
                continue;
            }
            obj_move(self, i);
            if self.frame_of(i).state == 0x270e {
                self.set_exists(i, false);
            }
            let o = &self.objs[i];
            let st = self.frame_of(i).state;
            if st == 0xe
                && o.hp() < 1
                && (o.m.i32(0x2f4) >= 0 || o.team() == 5 || i > 0x13)
                && 0 < o.u_08()
                && o.u_08() < 5
            {
                self.death_and_respawn(i);
            }
            let o = &self.objs[i];
            if o.m.i32(0x324) >= 0 && o.m.i8(0xdc) == 3 && o.iy() == 0 && o.hp() > 0 {
                self.transform_back(i);
            }
        }
        self.unported_rest_of_tick();
    }

    /// 0x41d73a..0x41dd60: in VS mode, decide when the match is over.
    fn match_end_checks(&mut self) {
        let mode = self.g.game_mode();
        if mode == 1 {
            self.stage_end_checks();
            return;
        }
        // count living characters per team (teams 1..39 except 5)
        let mut per_team = [0i32; 40];
        for i in 0..MAX_OBJECTS {
            if !self.exists(i) {
                continue;
            }
            let o = &self.objs[i];
            if o.hp() > 0 && self.data_of(i).kind == 0 {
                let t = o.team();
                if 0 < t && t < 0x28 && t != 5 {
                    per_team[t as usize] += 1;
                }
            }
        }
        let mut teams = 0;
        for (t, &n) in per_team.iter().enumerate() {
            if n > 0 {
                self.g.m.set_i32(0x450bf8, t as i32);
                teams += 1;
            }
        }
        if teams < 2 && self.g.match_phase() == 0 && (self.g.m.i32(0x451b7c) == 1 || mode != 4) {
            if teams == 0 {
                self.g.m.set_i32(0x450bf8, -1);
            }
            self.g.set_end_timer(self.g.end_timer() + 1);
            if self.g.end_timer() == 0x50 {
                if mode != 0 && mode != 2 && mode != 3 && mode != 4 && mode != 5 {
                    self.music_stop();
                }
                if mode != 5 {
                    self.g.set_slow_frames(1);
                }
                self.buffer_sounds.push("data\\m_end.wav");
            }
        } else if mode != 2 && mode != 3 {
            self.g.m.set_i32(0x450bf8, -1);
        }
    }

    /// From 0x41eed1: everything after the movement loop.
    fn unported_rest_of_tick(&mut self) {
        self.bound_and_carry();
        if self.g.match_first_frame() == 2 {
            self.g.set_match_first_frame(0);
        } else {
            let mode = self.g.game_mode();
            self.collisions(mode);
        }
        let mut weapons = 0;
        for i in 0..MAX_OBJECTS {
            if !self.exists(i) {
                continue;
            }
            let k = self.data_of(i).kind;
            if k == 1 || k == 2 || k == 4 || k == 6 {
                weapons += 1;
            }
            if k == 0 {
                self.obj_update(i);
            }
        }
        self.g.set_weapon_count(weapons);
        if weapons < 4 && self.random(0x92, 200) == 0 {
            self.drop_random_weapon();
        }
        for i in 0..MAX_OBJECTS {
            if self.exists(i) && self.data_of(i).kind > 0 {
                self.obj_update(i);
            }
        }
        self.catch_controls();
        self.catch_hold();
        // 0x41f2b8: forget holder links that no longer point back
        for i in 0..MAX_OBJECTS {
            if !self.exists(i) {
                continue;
            }
            let o = &self.objs[i];
            if o.weapon_type() > 0 {
                let j = o.weapon_held();
                if j < 0 || 399 < j || !self.exists(j as usize) || self.objs[j as usize].weapon_holder() != i as i32 {
                    self.objs[i].set_weapon_type(0);
                }
            }
        }
        self.bound_and_carry();
        let mode = self.g.game_mode();
        self.bound_x_and_camera(mode);
        // 0x41a5a0 draws the objects and advances the hit sparks.
        self.advance_sparks();
        match self.g.game_mode() {
            1 => self.stage_mode_tick(),
            4 => self.battle_mode_tick(),
            _ => {}
        }
        self.apply_knockback();
        let mode = self.g.game_mode();
        self.post_loop(mode);
        self.after_post_loop();
        if ((self.g.end_timer() - 0x65) as u32) < 0xf9 {
            self.draw_summary();
        }
        self.draw_replay_info();
        self.draw_mode_label();
        self.recording_messages();
        self.sounds = self.flush_sounds();
    }

    /// 0x4214d5: F7/F8 effects, healing timers, clearing hit lists, the game clock.
    fn after_post_loop(&mut self) {
        let bb8 = self.g.m.i32(0x450bb8);
        if bb8 == 1 {
            self.weapon_rain();
        }
        let bc0 = self.g.m.i32(0x450bc0);
        let mode = self.g.game_mode();
        for i in 0..MAX_OBJECTS {
            if !self.exists(i) {
                continue;
            }
            if bb8 == 2 {
                let k = self.data_of(i).kind;
                let id = self.data_of(i).id;
                let o = &mut self.objs[i];
                if k == 1 || k == 2 || k == 4 || k == 6 {
                    o.set_weapon_hp(-1);
                } else if mode == 1 && o.team() == 5 && k == 0 && id != 300 {
                    o.set_hp(0);
                    o.set_mp(0);
                }
            }
            if bc0 == 1 && (mode != 1 || (i < 8 && self.objs[i].team() == 1)) {
                let o = &mut self.objs[i];
                if o.max_hp() < 500 {
                    o.set_max_hp(500);
                }
                o.set_dark_hp(o.max_hp());
                o.set_hp(o.dark_hp());
                o.set_mp(500);
            }
            // 0xe0 = 1000 + n: heal 8 every 8 ticks for n ticks
            let o = &mut self.objs[i];
            if o.m.i32(0xe0) / 1000 == 1 && o.hp() > 0 {
                o.m.add_i32(0xe0, -1);
                if o.m.i32(0xe0) % 8 == 0 {
                    if o.hp() < o.dark_hp() {
                        if o.hp() < o.dark_hp() - 8 {
                            o.add_hp(8);
                        } else {
                            o.set_hp(o.dark_hp());
                        }
                    } else {
                        o.m.set_i32(0xe0, 0);
                    }
                }
                if o.m.i32(0xe0) % 1000 == 0 {
                    o.m.set_i32(0xe0, 0);
                }
            }
            if o.m.i32(0xe4) > 0 && o.hp() > 0 {
                o.m.add_i32(0xe4, -1);
                if o.m.i32(0xe4) % 8 == 0 && o.hp() < o.dark_hp() {
                    o.add_hp(8);
                    if o.dark_hp() < o.hp() {
                        o.set_hp(o.dark_hp());
                        o.m.set_i32(0xe4, 0);
                    }
                }
            }
            if self.state(i) == 0x6a4 {
                self.objs[i].m.set_i32(0xe0, 0x44c);
            }
            if self.exists(i) {
                let o = &mut self.objs[i];
                o.m.set_i32(0x2e8, 1000);
                o.m.set_i32(0x2ec, 1000);
                o.m.set_i32(0x2f0, 1000);
                o.m.set_i32(0x2e4, 0);
                o.m.set_u8(0xeb, 0);
            }
        }
        self.g.m.set_i32(0x450bc0, 0);
        self.g.m.set_i32(0x450bb8, 0);
        if self.g.end_timer() < 100 {
            self.g.m.add_i32(0x450bbc, 1);
        }
        // at end_timer 101 a .lfr being recorded gets the match summary
        if self.g.end_timer() == 0x65
            && self.g.m.i32(0x450be4) != 0
            && self.g.recording() != 0
            && self.g.replaying() == 0
        {
            self.recording_summary();
        }
        if self.g.end_timer() == 0x65 {
            self.g.set_replaying(0);
        }
    }

}

impl World {
    /// 0x41f04b: a random weapon falls from the sky (y = -500) somewhere on the
    /// background.
    /// 0x4214e3 (F8 in tool mode): drop one of every weapon and item (ids 100..199;
    /// milk only with probability 1/2) at random places from the sky.
    fn weapon_rain(&mut self) {
        let mut list = Vec::new();
        for k in 0..self.gd.objects.len() {
            let id = self.gd.objects[k].id;
            if 99 < id && id < 200 && (id != 0x7a || self.random(0xd0, 2) != 0) {
                list.push(k);
            }
        }
        for &data in &list {
            let slot = self.free_slot();
            let (bw, z0, z1) = {
                let b = self.background();
                (b.width, b.zboundary.0, b.zboundary.1)
            };
            let r1 = self.random(0xd1, 0x1e);
            let r2 = self.random(0xd2, 0x1e);
            let x = r2 + 0x1e + r1 * ((bw - 0x3c) / 0x1e);
            let r3 = self.random(0xd3, 0x1e);
            let r4 = self.random(0xd4, 0x1e);
            let z = r4 + z0 + 0x1e + r3 * (((z1 - z0) - 0x3c) / 0x1e);
            let Some(slot) = slot else {
                // the original reuses the slot of the previous drop
                self.unported("F8 weapon rain with no free slot (0x42156x)");
                return;
            };
            let hp = self.gd.objects[data].weapon_hp;
            let o = &mut self.objs[slot];
            crate::collisions::obj_reset(o);
            o.m.set_i32(0x368, data as i32);
            o.set_x(x as f64);
            o.set_weapon_hp(hp);
            o.set_y(-500.0);
            o.set_z(z as f64);
            self.set_exists(slot, true);
            for j in 0..MAX_OBJECTS {
                if self.exists(j) {
                    self.objs[j].m.set_u8(0xf0 + slot as u32, 0);
                }
            }
            let id = self.gd.objects[data].id;
            let o = &mut self.objs[slot];
            o.set_vx(0.0);
            o.set_vy(0.0);
            o.set_vz(0.0);
            if id == 0x7a {
                o.set_hp(200);
            }
            o.set_ix(crate::mem::ftol(o.x()));
            o.set_iy(crate::mem::ftol(o.y()));
            o.set_iz(crate::mem::ftol(o.z()));
        }
    }

    fn drop_random_weapon(&mut self) {
        let mode = self.g.game_mode();
        let slot = self.free_slot();
        let mut list = Vec::new();
        for k in 0..self.gd.objects.len() {
            let id = self.gd.objects[k].id;
            if 99 < id && id < 200 {
                let ok = (id != 0x7a && id != 0x7b) || {
                    let r = self.random(0x93, 2);
                    r != 0 && mode != 2 && mode != 3 && mode != 1 && mode != 4
                };
                if ok {
                    list.push(k);
                }
            }
        }
        let (bw, z0, z1) = {
            let b = self.background();
            (b.width, b.zboundary.0, b.zboundary.1)
        };
        let bb4 = self.g.u_450bb4();
        let (r1, w, site) = if bb4 == 0 { (self.random(0x94, 0x1e), bw, 0x95) } else { (self.random(0x96, 0x1e), bb4, 0x97) };
        let r2 = self.random(site, 0x1e);
        let x = r2 + 0x1e + r1 * ((w - 0x3c) / 0x1e);
        let r3 = self.random(0x98, 0x1e);
        let r4 = self.random(0x99, 0x1e);
        let z = r4 + z0 + 0x1e + r3 * (((z1 - z0) - 0x3c) / 0x1e);
        let pick = self.random(0x9a, list.len() as i32) as usize;
        let (Some(slot), Some(&data)) = (slot, list.get(pick)) else {
            self.unported("weapon drop with no free slot or no weapon data (0x41f0xx)");
            return;
        };
        let hp = self.gd.objects[data].weapon_hp;
        let o = &mut self.objs[slot];
        crate::collisions::obj_reset(o);
        o.m.set_i32(0x368, data as i32);
        o.set_x(x as f64);
        o.set_weapon_hp(hp);
        o.set_y(-500.0);
        o.set_z(z as f64);
        self.set_exists(slot, true);
        for j in 0..MAX_OBJECTS {
            if self.exists(j) {
                self.objs[j].m.set_u8(0xf0 + slot as u32, 0);
            }
        }
        let id = self.gd.objects[data].id;
        let o = &mut self.objs[slot];
        o.set_vx(0.0);
        o.set_vy(0.0);
        o.set_vz(0.0);
        if id == 0x7a {
            o.set_hp(200);
        }
        o.set_ix(crate::mem::ftol(o.x()));
        o.set_iy(crate::mem::ftol(o.y()));
        o.set_iz(crate::mem::ftol(o.z()));
        o.m.set_i32(0x354, 99);
    }
}

impl World {
    /// 0x403270: state 400 moves next to the nearest enemy (120 pixels in front of
    /// it), state 401 next to the farthest teammate (60 pixels).
    fn teleport(&mut self, i: usize, kind: i32) {
        let (team, ix, iz) = (self.objs[i].team(), self.objs[i].ix(), self.objs[i].iz());
        let mut best: Option<usize> = None;
        let mut best_d = if kind == 1 { 10000 } else { -1 };
        for j in 0..MAX_OBJECTS {
            if j == i || !self.exists(j) || self.data_of(j).kind != 0 {
                continue;
            }
            let o = &self.objs[j];
            let enemy = o.team() != team;
            if (kind == 1) != enemy || o.hp() <= 0 {
                continue;
            }
            let d = (o.iz() - iz).wrapping_abs() + (o.ix() - ix).wrapping_abs();
            if (kind == 1 && d < best_d) || (kind == 2 && best_d < d) {
                best = Some(j);
                best_d = d;
            }
        }
        match best {
            None => {
                let o = &mut self.objs[i];
                o.set_iy(0);
                o.set_y(0.0);
            }
            Some(j) => {
                let (tx, tz) = (self.objs[j].ix(), self.objs[j].iz());
                let gap = if kind == 1 { 0x78 } else { 0x3c };
                let o = &mut self.objs[i];
                o.set_iy(0);
                o.set_iz(tz + 1);
                o.set_ix(if o.facing() == 0 { tx - gap } else { tx + gap });
                o.set_x(o.ix() as f64);
                o.set_y(o.iy() as f64);
                o.set_z(o.iz() as f64);
            }
        }
        let o = &mut self.objs[i];
        o.set_vz(0.0);
        o.set_vy(0.0);
        o.set_vx(0.0);
    }
}

impl World {
    fn key_down_consume(&mut self, vk: u32) -> bool {
        if self.g.m.u8(key_at(vk)) == b'd' {
            self.g.m.set_u8(key_at(vk), b'u');
            true
        } else {
            false
        }
    }

    /// The F-key handlers 0x416c70..0x416f60 by the key that runs them; each also sets
    /// its bit in the frame's record.
    pub(crate) fn fkey(&mut self, vk: u32) {
        let mode = self.g.game_mode();
        let phase = self.g.match_phase();
        let cheats = |w: &World| w.g.m.i32(0x450c28) < 2 && (mode == 0 || w.g.m.i32(0x45842c) == 1) && phase == 0;
        match vk {
            0xf9 => {
                self.rec_frame[9] |= 1;
                let v = 1 - self.g.u_458428();
                self.g.set_u_458428(v);
                self.buffer_sounds.push("data\\m_pass.wav");
            }
            0xf8 => {
                self.rec_frame[9] |= 2;
                let v = 1 - self.g.m.i32(0x45842c);
                self.g.m.set_i32(0x45842c, v);
                self.buffer_sounds.push("data\\m_end.wav");
            }
            0x73 => self.f4_back_to_menu(),
            0x70 => {
                if phase == 0 {
                    // F1: pause; the pause works through the network wait flag
                    let v = 1 - self.g.m.i32(0x450bfc);
                    self.g.m.set_i32(0x44fb60, v);
                    self.g.m.set_i32(0x44fcb0, v);
                }
            }
            0x71 => {
                if phase == 0 {
                    // F2: run one frame, then pause
                    self.g.m.set_i32(0x44fcb0, 1);
                    self.g.m.set_i32(0x44fb60, 0);
                }
            }
            0x72 => {
                if phase == 0 {
                    // F3: lock the cheat keys
                    self.rec_frame[8] |= 4;
                    self.g.m.set_i32(0x450c28, 2);
                }
            }
            0x74 => {
                if phase == 0 {
                    // F5: frame period 3 ms or 33 ms
                    let v = 1 - self.g.m.i32(0x44d02c);
                    self.g.m.set_i32(0x44d02c, v);
                    self.g.m.set_u8(key_at(0x74), b'u');
                }
            }
            0x75 => {
                if cheats(self) {
                    // F6: specials cost no mp
                    self.rec_frame[8] |= 0x10;
                    let v = 1 - self.g.mp_cost_on();
                    self.g.set_mp_cost_on(v);
                    self.g.m.add_i32(0x450c18, 1);
                    self.g.m.set_i32(0x450c28, 1);
                }
            }
            0x76 | 0x77 | 0x78 => {
                if cheats(self) {
                    let bit = match vk { 0x76 => 0x20, 0x77 => 0x40, _ => 0x80 };
                    self.rec_frame[8] |= bit;
                    if self.g.end_timer() == 0 {
                        match vk {
                            0x76 => {
                                // F7: heal everybody
                                let v = 1 - self.g.m.i32(0x450bc0);
                                self.g.m.set_i32(0x450bc0, v);
                                self.g.m.add_i32(0x450c1c, 1);
                            }
                            0x77 => {
                                // F8: weapons rain
                                self.g.m.add_i32(0x450c20, 1);
                                self.g.m.set_i32(0x450bb8, 1);
                            }
                            _ => {
                                // F9: destroy weapons
                                self.g.m.add_i32(0x450c24, 1);
                                self.g.m.set_i32(0x450bb8, 2);
                            }
                        }
                        self.g.m.set_i32(0x450c28, 1);
                    }
                }
            }
            _ => {}
        }
    }

    fn key_down(&self, vk: u32) -> bool {
        self.g.m.u8(key_at(vk)) == b'd'
    }

    /// The local F-key dispatch in 0x41bf36.
    fn fkeys(&mut self) {
        for vk in [0xf9, 0xf8] {
            if self.key_down_consume(vk) {
                self.fkey(vk);
            }
        }
        if self.key_down(0x73) && self.g.match_phase() == 0 {
            self.key_down_consume(0x73);
            self.fkey(0x73);
        }
        let p = self.g.match_phase();
        if self.key_down(0x70) && self.g.match_phase() == 0 {
            self.key_down_consume(0x70);
            self.fkey(0x70);
        }
        for vk in [0x71, 0x72, 0x74] {
            if self.key_down(vk) && p == 0 {
                self.key_down_consume(vk);
                self.fkey(vk);
            }
        }
        for vk in [0x75, 0x76, 0x77, 0x78] {
            if self.key_down_consume(vk) {
                self.fkey(vk);
            }
        }
    }

    /// The F-keys while a recording plays (0x41c1xx): F1/F2/F4/F5 work live, the
    /// others come from the recorded frame.
    fn fkeys_replay(&mut self) {
        if self.key_down(0x73) {
            self.fkey(0x73);
            self.end_match();
        }
        if self.key_down_consume(0x74) {
            self.fkey(0x74);
        }
        let r = self.replay_frame;
        if r[9] & 1 != 0 {
            self.fkey(0xf9);
        }
        if r[9] & 2 != 0 {
            self.fkey(0xf8);
        }
        if self.key_down_consume(0x70) {
            self.fkey(0x70);
        }
        if self.key_down_consume(0x71) {
            self.fkey(0x71);
        }
        for (bit, vk) in [(4u8, 0x72), (0x10, 0x75), (0x20, 0x76), (0x40, 0x77), (0x80, 0x78)] {
            if r[8] & bit != 0 {
                self.fkey(vk);
            }
        }
    }

    /// 0x41bd2a: while replaying, F6 toggles the replay information and the arrow keys
    /// move the camera (down gives it back).
    fn replay_camera_keys(&mut self) {
        if self.key_down_consume(0x75) {
            let v = 1 - self.g.m.i32(0x44d030);
            self.g.m.set_i32(0x44d030, v);
        }
        let (l, r) = (self.key_down(0x25), self.key_down(0x27));
        if l || r {
            if self.g.m.i32(0x450b74) == 0 {
                let cx = self.g.m.i32(crate::camera::CAMERA_X);
                let cv = self.g.m.i32(crate::camera::CAMERA_V);
                self.g.m.set_i32(0x450b7c, cx);
                self.g.m.set_i32(0x450b74, 1);
                self.g.m.set_i32(0x450b78, cv);
            }
            if l {
                self.g.m.add_i32(0x450b78, -5);
            }
            if r {
                self.g.m.add_i32(0x450b78, 5);
            }
        }
        if self.key_down(0x28) {
            self.g.m.set_i32(0x450b74, 0);
        } else if self.g.m.i32(0x450b74) != 0 {
            let v = self.g.m.i32(0x450b78);
            self.g.m.add_i32(0x450b7c, v);
            self.g.m.set_i32(0x450b78, (v * 6) / 7);
            return;
        }
        self.g.m.set_i32(0x450b78, 0);
    }

    /// F4 (0x416cd0): leave the match.
    fn f4_back_to_menu(&mut self) {
        if self.g.match_phase() != 0 {
            return;
        }
        self.g.m.set_i32(0x450bfc, 0);
        self.g.m.set_i32(0x44fb60, 0);
        self.g.m.set_i32(0x44fcb0, 0);
        self.g.m.set_i32(0x450c2c, 0);
        self.g.m.set_i32(0x44d02c, 1);
        let mode = self.g.game_mode();
        if mode != 5 && self.g.m.i32(0x450be4) != 0 && self.g.recording() != 0 {
            self.g.set_recording(0);
            if self.g.end_timer() < 0x65 {
                self.g.m.set_i32(0x450b70, 3);
                self.g.m.set_i32(0x450b6c, 0);
            }
        }
        if self.g.demo() != 0 {
            self.g.set_game_mode(6);
            if self.replay.is_some() {
                self.replay_restore();
            }
            self.g.m.set_i32(0x450b88, 0);
            self.g.set_replaying(0);
            self.g.set_match_phase(10);
            self.g.set_end_timer(0);
            return;
        }
        if mode != 2 && mode != 3 {
            self.g.set_end_timer(0x15e);
            return;
        }
        self.g.set_end_timer(0);
        self.g.set_match_phase(10);
        self.g.m.set_i32(0x457580, 0);
    }
}
