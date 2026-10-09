//! Parts of the tick (0x41bc90) that change an object's data file or bring it back:
//! Rudolf's transform (frame state 501), transforming back with D^J^A, and the death
//! of objects that have lives (0x30c lives, 0x310 and 0x314 lives and hp of the next
//! stage of a boss).

use crate::world::{World, MAX_OBJECTS};

impl World {
    /// Give object `i` data file `data` and do the same for its living clones
    /// (objects whose 0x2f4 is `i`), which restart at frame 0, or 212 in the air.
    fn set_data_with_clones(&mut self, i: usize, data: usize) {
        for j in 0..MAX_OBJECTS {
            if self.exists(j) && self.objs[j].m.i32(0x2f4) == i as i32 && 0 < self.objs[j].hp() {
                let o = &mut self.objs[j];
                o.m.set_i32(0x368, data as i32);
                o.set_frame(if o.iy() < 0 { 0xd4 } else { 0 });
            }
        }
    }

    /// 0x41e3f0: frame state 501 turns the object into the character with id 0x33c
    /// (the last one Rudolf caught), remembering its own id in 0x324.
    pub(crate) fn transform_501(&mut self, i: usize) {
        let id = self.objs[i].m.i32(0x33c);
        let Some(k) = self.data_index(id) else { return };
        let own = self.data_of(i).id;
        let o = &mut self.objs[i];
        o.m.set_i32(0x324, own);
        o.m.set_i32(0x368, k as i32);
        o.set_frame(0);
        self.set_data_with_clones(i, k);
    }

    /// 0x41ec7x: D^J^A on the ground turns a transformed object back.
    pub(crate) fn transform_back(&mut self, i: usize) {
        self.objs[i].set_seq_dja(0);
        let id = self.objs[i].m.i32(0x324);
        let Some(k) = self.data_index(id) else { return };
        let o = &mut self.objs[i];
        o.m.set_i32(0x368, k as i32);
        o.set_frame(0xf5);
        o.m.set_i32(0x324, -1);
        self.set_data_with_clones(i, k);
    }

    /// 0x41dd8x (end_timer past 349): put back the characters that changed during the
    /// match, leave the match for the result screen, and clear the players' keys.
    pub(crate) fn end_of_match(&mut self) {
        for i in 0..MAX_OBJECTS {
            if !self.exists(i) {
                continue;
            }
            let back = self.objs[i].m.i32(0x324);
            if 0 <= back {
                if let Some(k) = self.data_index(back) {
                    self.objs[i].m.set_i32(0x368, k as i32);
                    self.objs[i].m.set_i32(0x324, -1);
                }
            } else if self.data_of(i).id == 0x32 && self.g.u_458428() == 0 {
                // Louis EX becomes Louis
                if let Some(k) = self.data_index(6) {
                    self.objs[i].m.set_i32(0x368, k as i32);
                }
            } else if -1 < self.objs[i].m.i32(0x328) {
                self.split_fusion(i);
            }
        }
        let mode = self.g.game_mode();
        let phase = match mode {
            2 => 0x1c,
            3 => 0x80,
            4 => 0xca,
            _ => 2,
        };
        self.g.set_match_phase(phase);
        self.g.set_end_timer(0);
        self.end_match();
        if self.g.demo() != 0 && self.g.match_phase() != 10 {
            self.g.set_game_mode(6);
            if self.replay.is_some() {
                self.replay_restore();
            }
            self.g.m.set_i32(0x450b88, 0);
            self.g.set_replaying(0);
            self.g.set_match_phase(10);
        }
    }

    /// The fusion split of `end_of_match`: object `i` (a fused character, 0x328 >= 0)
    /// becomes 0x330 again and object 0x32c comes back as 0x334, each with half the hp.
    fn split_fusion(&mut self, i: usize) {
        let other = self.objs[i].m.i32(0x32c) as usize;
        self.objs[i].m.set_i32(0x328, -1);
        self.objs[i].m.set_i32(0x338, 900);
        for k in 0..self.gd.objects.len() {
            let id = self.gd.objects[k].id;
            if id == self.objs[i].m.i32(0x330) {
                self.objs[i].m.set_i32(0x368, k as i32);
            } else if id == self.objs[i].m.i32(0x334) {
                self.spawn_reset(other, k);
                self.set_exists(other, true);
            }
        }
        let src = self.objs[i].clone();
        let o = &mut self.objs[other];
        o.set_hp(src.hp() / 2);
        o.set_dark_hp(src.dark_hp() / 2);
        o.set_ix(src.ix());
        o.set_iz(src.iz());
        o.set_iy(0);
        o.set_x(o.ix() as f64);
        o.set_z(o.iz() as f64);
        o.set_y(0.0);
        o.set_vx(0.0);
        o.m.set_i8(0x80, 1 - src.facing());
        o.set_frame(0x70);
        o.set_mp(0);
        o.set_team(src.team());
        let o = &mut self.objs[i];
        o.set_hp(src.hp() / 2);
        o.set_dark_hp(src.dark_hp() / 2);
        o.set_vx(0.0);
        o.set_frame(0x70);
        o.set_mp(0);
    }

    /// 0x431c70: clear the per-player counters 0x4513a4..0x4513bc and 0x451320[8],
    /// the keys of objects 0..7, and 300 bytes of the key array from 0x455378.
    pub(crate) fn end_match(&mut self) {
        for a in (0x4513a4..=0x4513bc).step_by(4) {
            self.g.m.set_i32(a, 0);
        }
        for j in 0..8 {
            self.g.m.set_i32(0x451320 + 4 * j as u32, 0);
            crate::ai::ai_clear_keys(&mut self.objs[j]);
        }
        for a in 0..300 {
            self.g.m.set_u8(crate::world::KEYS + a, b'u');
        }
    }

    /// 0x41e0xx: once end_timer passes 143, attack or jump from objects 0..7 skips to
    /// the end of the match (end_timer 350).
    pub(crate) fn continue_after_match(&mut self) {
        let mode = self.g.game_mode();
        for j in 0..8 {
            let o = &self.objs[j];
            if o.attack() != 1 && o.jump() != 1 {
                continue;
            }
            let ba8 = self.g.stage_state();
            if mode == 1 {
                if self.g.u_450c2c() == 0 && ba8 == 1 {
                    self.g.m.set_i32(0x450ba8, 3);
                    continue;
                }
                if ba8 != 2 && ba8 != 0 {
                    continue;
                }
            }
            let b94 = self.g.m.i32(0x450b94);
            self.g.m.set_i32(0x450b94, (b94 / 10) * 10);
            self.g.set_u_450c2c(0);
            self.g.set_end_timer(0x15e);
        }
    }

    /// 0x41e6b0: an object with lives (a summoned one, team 5, or object 20 and up)
    /// lies dead.
    pub(crate) fn death_and_respawn(&mut self, i: usize) {
        if self.objs[i].m.i32(0x314) < 1 {
            if self.objs[i].m.i32(0x30c) < 2 {
                self.set_exists(i, false);
                return;
            }
            self.objs[i].m.add_i32(0x30c, -1);
            // come back from the sky at the average position of the team
            let team = self.objs[i].team();
            let (mut sx, mut sz, mut n) = (0i32, 0i32, 0i32);
            for j in 0..MAX_OBJECTS {
                if self.exists(j) && j != i && self.data_of(j).kind == 0 && self.objs[j].team() == team {
                    sx = sx.wrapping_add(self.objs[j].ix());
                    sz = sz.wrapping_add(self.objs[j].iz());
                    n += 1;
                }
            }
            let r = self.random(0x90, 0x33);
            if n == 0 {
                // the original divides by zero here
                self.unported("respawn with no teammate (0x41e7xx)");
                return;
            }
            self.objs[i].set_x((sx / n) as f64 + r as f64 - 26.0);
            let r = self.random(0x91, 0x1f);
            let o = &mut self.objs[i];
            o.set_z((sz / n) as f64 + r as f64 - 16.0);
            o.set_mp(500);
            o.set_dark_hp(o.max_hp());
            o.set_hp(o.dark_hp());
            o.set_u_08(0x14);
            o.set_frame(0xd4);
            o.set_iy(-300);
            o.set_y(o.iy() as f64);
            o.set_vy(0.0);
            return;
        }
        // the next stage of a boss: join team 1 with the reserve hp and lives
        let id = self.data_of(i).id;
        let o = &mut self.objs[i];
        o.m.set_i32(0x30c, o.m.i32(0x310));
        o.set_mp(0);
        o.set_max_hp(o.m.i32(0x314));
        o.set_dark_hp(o.max_hp());
        o.set_hp(o.dark_hp());
        o.m.set_i32(0x314, 0);
        o.m.set_i32(0x310, 0);
        o.set_team(1);
        if (0x1e..=0x24).contains(&id) {
            o.m.set_i32(0x318, 0x8c);
        }
        o.set_frame(0xdb);
        o.set_u_88(0);
        o.m.set_i32(0xb4, 10);
        let Some(slot) = (0x32..MAX_OBJECTS).find(|&j| !self.exists(j)) else { return };
        let Some(data) = self.data_index(0x3e6) else { return };
        self.spawn_reset(slot, data);
        let src = self.objs[i].clone();
        let o = &mut self.objs[slot];
        o.set_ix(src.ix());
        o.set_iy(src.iy());
        o.set_iz(src.iz() + 1);
        o.set_team(src.team());
        o.set_x(src.x());
        o.set_y(src.y());
        o.set_z(src.z());
        o.set_frame(6);
        o.set_vx(0.0);
        o.set_vy(0.0);
        o.set_vz(0.0);
        o.m.set_i8(0x80, 0);
        self.set_exists(slot, true);
    }
}
