//! Recording and replaying `.lfr` files: `recording_start` (0x43d2c0, header part),
//! `recording_write_frame` (0x43db40), `replay_read_frame` (0x43dc50), `replay_step`
//! (0x4197a0), `replay_load` (0x43dfa0), `replay_restore` (0x43df00), and the match
//! summary written at end_timer 101 (0x421d00).
//!
//! The recording struct (`lf2_data::lfr`) lives on the heap in the original; the port
//! keeps it in `World::recording` (being written) and `World::replay` (being played).
//! The globals 0x4588a8 / 0x4588ac hold markers for "allocated".

use crate::world::{World, EXISTS, MAX_OBJECTS};
use lf2_data::lfr::REC_SIZE;

const REC: u32 = 0x4588a8;
const PLAY: u32 = 0x4588ac;
const KEYS_AT: usize = 0x2b38;
const HP_SUMS_AT: usize = 0x14b8;

fn rd(r: &[u8], at: usize) -> i32 {
    i32::from_le_bytes(r[at..at + 4].try_into().unwrap())
}

fn wr(r: &mut [u8], at: usize, v: i32) {
    r[at..at + 4].copy_from_slice(&v.to_le_bytes());
}

fn cstr_at(r: &[u8], at: usize) -> Vec<u8> {
    r[at..].iter().copied().take_while(|&c| c != 0).collect()
}

/// Why a recording cannot be played (the messages of 0x431d10).
#[derive(Debug, Clone, PartialEq)]
pub enum ReplayError {
    Corrupt,
    DataFiles,
    Version(i32),
}

impl World {
    fn g_cstr(&self, at: u32, max: u32) -> Vec<u8> {
        (0..max).map(|k| self.g.m.u8(at + k)).take_while(|&c| c != 0).collect()
    }

    fn g_put_cstr(&mut self, at: u32, s: &[u8]) {
        for (k, &b) in s.iter().chain(std::iter::once(&0)).enumerate() {
            self.g.m.set_u8(at + k as u32, b);
        }
    }

    /// The header of `recording_start` (0x43d2c0), written when a match starts.
    pub(crate) fn recording_header(&mut self) {
        let mut r = vec![0u8; REC_SIZE];
        wr(&mut r, 0, self.g.difficulty());
        wr(&mut r, 8, self.g.u_458428());
        wr(&mut r, 0xc, self.g.m.i32(0x45842c));
        wr(&mut r, 4, self.g.m.i32(0x450b94));
        wr(&mut r, 0x148, self.g.game_mode());
        for p in 0..8u32 {
            let n = self.g_cstr(0x44fcc0 + 0xb * p, 11);
            let at = 0x14c + 0xb * p as usize;
            r[at..at + n.len()].copy_from_slice(&n);
        }
        for (at, from, max) in [(0x630bc0usize, 0x44fd18u32, 100u32), (0x630c24, 0x44f900, 0x1f3), (0x630db4, 0x44f890, 100)] {
            let n = self.g_cstr(from, max);
            let n = &n[..n.len().min(REC_SIZE - at - 1)];
            r[at..at + n.len()].copy_from_slice(n);
        }
        wr(&mut r, 0x1a4, self.g.background());
        for i in 0..18usize {
            let o = &self.objs[i];
            let d = self.gd.objects[o.m.i32(0x368).max(0) as usize].id;
            let b = 4 * i;
            wr(&mut r, 0x1a8 + b, o.team());
            wr(&mut r, 0x1f0 + b, d);
            wr(&mut r, 0x238 + b, self.g.m.u8(EXISTS + i as u32) as i8 as i32);
            wr(&mut r, 0x280 + b, o.u_08());
            wr(&mut r, 0x2c8 + b, o.ix());
            wr(&mut r, 0x310 + b, o.iy());
            wr(&mut r, 0x358 + b, o.iz());
            wr(&mut r, 0x3a0 + b, o.mp());
            wr(&mut r, 0x3e8 + b, o.m.i32(0x354));
            wr(&mut r, 0x430 + b, o.max_hp());
            wr(&mut r, 0x478 + b, o.m.i32(0x33c));
            wr(&mut r, 0x4c0 + b, o.m.i32(0x344));
            wr(&mut r, 0x508 + b, o.m.i32(0x340));
        }
        let m = self.g_cstr(0x44eed0, 0x30);
        r[0x550..0x550 + m.len()].copy_from_slice(&m);
        wr(&mut r, 0x744, self.g.m.i32(0x44f620));
        wr(&mut r, 0x748, self.g.m.i32(0x44d03c));
        for k in 0..0x16usize {
            for (dst, src) in [(0x74c, 0x44d5f8u32), (0x7a4, 0x44d650), (0x7fc, 0x44d6a8), (0x854, 0x44d700)] {
                wr(&mut r, dst + 4 * k, self.g.m.i32(src + 4 * k as u32));
            }
        }
        wr(&mut r, 0x8c4, self.g.rng_bcc());
        for k in 0..0xbb9u32 {
            r[0x8c8 + k as usize] = self.g.m.u8(0x44ff90 + k);
        }
        for k in 0..11usize {
            wr(&mut r, 0x1488 + 4 * k, self.g.m.i32(0x44d324 + 4 * k as u32));
        }
        wr(&mut r, 0x14b4, self.g.half_frame());
        self.recording = Some(r);
    }

    /// Before the tick (0x41c6xx..0x41d6b9): apply the replayed keys, check the hp sums
    /// every 150 frames, and write the frame into the recording.
    pub(crate) fn replay_and_record(&mut self, half: i32) {
        let n = self.g.match_frames();
        if self.g.replaying() != 0 {
            self.replay_step(half);
            if n % 0x96 == 0 {
                let sum = self.hp_sum();
                let want = self.replay.as_ref().map(|r| rd(r, HP_SUMS_AT + 4 * (n / 0x96) as usize));
                if want != Some(sum) {
                    self.end_match();
                    if self.g.replaying() != 0 && self.g.match_phase() != 10 {
                        self.g.set_game_mode(6);
                        if self.replay.is_some() && self.g.demo() != 0 {
                            self.replay_restore();
                        }
                        self.g.m.set_i32(0x450b88, 0);
                        self.g.set_replaying(0);
                        self.g.set_match_phase(10);
                    }
                    self.requests.push(crate::menu::main_menu::MenuRequest::Message(
                        "Recording file error! Replaying canceled!",
                    ));
                }
            }
        }
        if self.g.recording() != 0 {
            let frame = if self.g.replaying() == 0 { self.rec_frame } else { self.replay_frame };
            let sum = if n % 0x96 == 0 { Some(self.hp_sum()) } else { None };
            if let Some(r) = self.recording.as_mut() {
                let at = KEYS_AT + 10 * n as usize;
                if at + 10 <= r.len() {
                    r[at..at + 10].copy_from_slice(&frame);
                }
                if let Some(s) = sum {
                    wr(r, HP_SUMS_AT + 4 * (n / 0x96) as usize, s);
                }
            }
        }
    }

    fn hp_sum(&self) -> i32 {
        (0..20).filter(|&i| self.g.m.u8(EXISTS + i as u32) == 1).map(|i| self.objs[i].hp()).sum()
    }

    /// `replay_read_frame` (0x43dc50).
    pub(crate) fn replay_read_frame(&mut self) {
        let n = self.g.match_frames() as usize;
        if let Some(r) = &self.replay {
            let at = KEYS_AT + 10 * n;
            if at + 10 <= r.len() {
                self.replay_frame.copy_from_slice(&r[at..at + 10]);
            }
        }
    }

    /// `replay_step` (0x4197a0): the keys of objects 0..7 from the replayed frame.
    fn replay_step(&mut self, half: i32) {
        for i in 0..8 {
            let b = self.replay_frame[i];
            let o = &mut self.objs[i];
            o.shift_keys();
            if half == 0 {
                o.set_up((b & 0x80 != 0) as i8);
                o.set_down((b & 0x40 != 0) as i8);
                o.set_left((b & 0x20 != 0) as i8);
                o.set_right((b & 0x10 != 0) as i8);
                o.set_attack((b & 8 != 0) as i8);
                o.set_jump((b & 4 != 0) as i8);
                o.set_defend((b & 2 != 0) as i8);
            }
        }
    }

    /// While a recording plays: the replay sign, and with F-key info on (0x44d030)
    /// `draw_replay_info` (0x41b390): author, info and the time played / total.
    pub(crate) fn draw_replay_info(&mut self) {
        if self.g.replaying() == 0 {
            return;
        }
        self.blit(0x45116c, 0x43, 0x216, 0x18, false);
        if self.g.m.i32(0x44d030) == 0 {
            return;
        }
        let total = self.replay.as_ref().map_or(0, |r| rd(r, 0x144));
        let mut y = if self.g.game_mode() == 4 { 0x9b } else { 0x85 };
        let put = |w: &mut World, at: u32, s: &[u8]| {
            for (k, &b) in s.iter().chain(std::iter::once(&0)).enumerate() {
                w.g.m.set_u8(at + k as u32, b);
            }
        };
        if self.g_cstr(0x44fd18, 100) != b"<No name>" {
            put(self, 0x450f60, b"Author:");
            self.draw_number_shadow(0x450f60, 10, y, 0x40, 1, 0, false);
            self.draw_number_shadow(0x44fd18, 0x4b, y, 0x40, 1, 0, false);
            y += 0x16;
        }
        if self.g_cstr(0x44f900, 0x400) != b"<No info>" {
            put(self, 0x450f60, b"  Info:");
            self.draw_number_shadow(0x450f60, 10, y, 0x40, 4, 0, false);
            self.draw_number_shadow(0x44f900, 0x4b, y, 0x40, 4, 0, false);
        }
        let clock = |f: i32| {
            let t = (f + 0xf) / 0x1e;
            if t < 0xe10 {
                format!("{:02}:{:02}", t / 0x3c, t % 0x3c)
            } else {
                format!("{:02}:{:02}:{:02}", t / 0xe10, (t % 0xe10) / 0x3c, t % 0x3c)
            }
        };
        let tail = format!(" / {}", clock(total));
        put(self, 0x450e30, tail.as_bytes());
        let line = clock(self.g.m.i32(0x450bbc)) + &tail;
        put(self, 0x450e98, line.as_bytes());
        self.draw_number_shadow(0x450e98, 5, 0x1fe, 0x40, 4, 0, false);
    }

    /// `recording_messages` (0x4028a0), every frame: F11/F12 change the volume
    /// (`message_aux` 0x402810), then the volume or the recording message is drawn.
    pub(crate) fn recording_messages(&mut self) {
        let keys = crate::world::KEYS;
        let mut d = 0;
        if self.g.m.u8(keys + 0x7a) == b'd' {
            d = -1;
        }
        if self.g.m.u8(keys + 0x7b) == b'd' {
            d = 1;
        }
        let show = if d != 0 {
            let v = (self.g.m.i32(0x44d000) + d).clamp(0, 100);
            self.g.m.set_i32(0x44d000, v);
            self.g.m.set_i32(0x44f190, 100);
            true
        } else {
            0 < self.g.m.i32(0x44f190)
        };
        let state = self.g.m.i32(0x450b70);
        if show && state == 0 {
            let t = format!("Volume: {}", self.g.m.i32(0x44d000));
            self.text(t.as_bytes(), 0, 0xff00, 3, 0x213);
            self.g.m.set_i32(0x44f190, self.g.m.i32(0x44f190) - 1);
        }
        if 0 < state && self.g.m.i32(0x450bfc) == 0 {
            let n = self.g.m.i32(0x450b6c) + 1;
            self.g.m.set_i32(0x450b6c, n);
            if 0xf0 < n {
                self.g.m.set_i32(0x450b6c, 0);
                self.g.m.set_i32(0x450b70, 0);
                return;
            }
        }
        let name = self.recording_file_name();
        match state {
            1 => self.text(format!("Start recording '{}'...", name).as_bytes(), 0, 0xff7800, 3, 0x213),
            2 => self.text(format!("Recording file '{}' saved!", name).as_bytes(), 0, 0xff00, 3, 0x213),
            3 => {
                let n = self.g.m.i32(0x450b6c) + 1;
                self.g.m.set_i32(0x450b6c, n);
                self.text(b"Recording canceled!", 0, 0xff, 3, 0x213);
            }
            _ => {}
        }
    }

    /// The name of the next recording at 0x44fd98: "YYYYMMDD_hhmmss" + suffix + ".lfr".
    pub(crate) fn recording_name(&mut self, suffix: &str) {
        let t = self.local_time;
        let s = format!("{:4}{:02}{:02}_{:02}{:02}{:02}{}.lfr", t[0], t[1], t[2], t[3], t[4], t[5], suffix);
        for (k, b) in s.bytes().chain(std::iter::once(0)).enumerate() {
            self.g.m.set_u8(0x44fd98 + k as u32, b);
        }
    }

    /// The name at 0x44fd98.
    pub fn recording_file_name(&self) -> String {
        let b: Vec<u8> = (0..0x40).map(|k| self.g.m.u8(0x44fd98 + k)).take_while(|&c| c != 0).collect();
        String::from_utf8_lossy(&b).into_owned()
    }

    /// Load a recording to play (`replay_load` 0x43dfa0 and the checks of 0x431d10).
    /// On success the match starts at the next frame.
    pub fn load_replay(&mut self, rec: Vec<u8>) -> Result<(), ReplayError> {
        if rec.len() != REC_SIZE {
            return Err(ReplayError::Corrupt);
        }
        self.replay = Some(rec);
        self.g.m.set_i32(PLAY, 1);
        self.replay_load();
        let r = self.replay.as_ref().unwrap();
        if rd(r, 0x744) != self.g.m.i32(0x44f620) {
            self.replay_failed();
            return Err(ReplayError::DataFiles);
        }
        if rd(r, 0x748) != self.g.m.i32(0x44d03c) {
            let v = rd(r, 0x748);
            self.replay_failed();
            return Err(ReplayError::Version(v));
        }
        let a = rd(r, 0x8c0);
        self.g.m.set_i32(0x44d380, a / 10000000 - 1);
        self.g.m.set_i32(0x451b74, (a % 10000000) / 1000000 - 1);
        self.g.m.set_i32(0x44d758, ((a % 100000) / 10000 + ((a % 1000000) / 100000) * 10) * 10);
        self.g.m.set_i32(0x44d384, (a % 10000) / 1000 - 1);
        self.g.m.set_i32(0x451b78, (a % 1000) / 100 - 1);
        self.g.m.set_i32(0x44d75c, (a % 10 + ((a % 100) / 10) * 10) * 10);
        self.g.set_match_phase(0);
        Ok(())
    }

    fn replay_failed(&mut self) {
        self.g.m.set_i32(0x450b88, 0);
        self.g.set_replaying(0);
        self.g.set_game_mode(6);
    }

    /// `replay_load` (0x43dfa0).
    fn replay_load(&mut self) {
        let r = self.replay.take().unwrap();
        self.g.m.set_i32(0x45877c, self.g.difficulty());
        self.g.set_match_frames(0);
        self.g.set_replaying(1);
        self.g.m.set_i32(0x450b88, 1);
        self.g.m.set_i32(crate::post_update::DIFFICULTY, rd(&r, 0));
        let mut r = r;
        wr(&mut r, 0x630bb8, self.g.u_458428());
        wr(&mut r, 0x630bbc, self.g.m.i32(0x45842c));
        self.g.set_u_458428(rd(&r, 8));
        self.g.m.set_i32(0x45842c, rd(&r, 0xc));
        self.g.m.set_i32(0x450b94, rd(&r, 4));
        self.g.set_game_mode(rd(&r, 0x148));
        for p in 0..8u32 {
            let cur = self.g_cstr(0x44fcc0 + 0xb * p, 11);
            self.g_put_cstr(0x458850 + 0xb * p, &cur);
            let n = cstr_at(&r, 0x14c + 0xb * p as usize);
            self.g_put_cstr(0x44fcc0 + 0xb * p, &n);
        }
        for (save, live, at) in [(0x4587e8u32, 0x44fd18u32, 0x630bc0usize), (0x458588, 0x44f900, 0x630c24), (0x458780, 0x44f890, 0x630db4)] {
            let cur = self.g_cstr(live, 0x200);
            self.g_put_cstr(save, &cur);
            let n = cstr_at(&r, at);
            self.g_put_cstr(live, &n);
        }
        let last = r.len() - 1;
        r[last] = 0;
        self.g.set_background(rd(&r, 0x1a4));
        for i in 0..MAX_OBJECTS as u32 {
            self.g.m.set_u8(EXISTS + i, 0);
        }
        for i in 0..18usize {
            let b = 4 * i;
            if rd(&r, 0x238 + b) == 0 {
                continue;
            }
            let id = rd(&r, 0x1f0 + b);
            if let Some(k) = self.data_index(id) {
                let whp = self.gd.objects[k].weapon_hp;
                let o = &mut self.objs[i];
                crate::collisions::obj_reset(o);
                o.m.set_i32(0x368, k as i32);
                o.set_x(350.0);
                o.set_weapon_hp(whp);
                o.set_y(0.0);
                o.set_z(300.0);
            }
            let o = &mut self.objs[i];
            o.set_team(rd(&r, 0x1a8 + b));
            self.g.m.set_u8(EXISTS + i as u32, rd(&r, 0x238 + b) as u8);
            let o = &mut self.objs[i];
            o.set_u_08(rd(&r, 0x280 + b));
            o.set_ix(rd(&r, 0x2c8 + b));
            o.set_iy(rd(&r, 0x310 + b));
            o.set_iz(rd(&r, 0x358 + b));
            o.set_x(o.ix() as f64);
            o.set_z(o.iz() as f64);
            o.set_mp(rd(&r, 0x3a0 + b));
            o.m.set_i32(0x354, rd(&r, 0x3e8 + b));
            o.set_max_hp(rd(&r, 0x430 + b));
            o.set_dark_hp(o.max_hp());
            o.set_hp(o.dark_hp());
            o.m.set_i32(0x33c, rd(&r, 0x478 + b));
            o.m.set_i32(0x344, rd(&r, 0x4c0 + b));
            o.m.set_i32(0x340, rd(&r, 0x508 + b));
        }
        for a in (0x450c04..=0x450c28).step_by(4) {
            self.g.m.set_i32(a, 0);
        }
        let m = cstr_at(&r, 0x550);
        self.g_put_cstr(0x44eed0, &m);
        for k in 0..0x16usize {
            for (src, dst) in [(0x74c, 0x44d5f8u32), (0x7a4, 0x44d650), (0x7fc, 0x44d6a8), (0x854, 0x44d700)] {
                self.g.m.set_i32(dst + 4 * k as u32, rd(&r, src + 4 * k));
            }
        }
        self.g.set_rng_bcc(rd(&r, 0x8c4));
        for k in 0..0xbb9usize {
            self.g.m.set_u8(0x44ff90 + k as u32, r[0x8c8 + k]);
        }
        for k in 0..11usize {
            self.g.m.set_i32(0x44d324 + 4 * k as u32, rd(&r, 0x1488 + 4 * k));
        }
        self.g.set_half_frame(rd(&r, 0x14b4));
        self.g.set_mp_cost_on(1);
        self.g.m.set_i32(0x450bbc, 0);
        let mode = self.g.game_mode();
        if mode != 1 && !m.is_empty() {
            self.play_music(&m);
        }
        self.g.set_u_450bb0(0);
        self.g.set_u_450bb4(0);
        if mode == 1 {
            self.g.set_stage_go_timer(0x46);
            self.g.set_stage_state(0);
            self.g.set_u_450bac(0);
            self.g.set_stage_fade(0);
            self.g.set_stage_phase(-1);
            self.g.set_survival_phase(-1);
            self.g.m.set_i32(crate::camera::CAMERA_V, 0);
            self.g.m.set_i32(crate::camera::CAMERA_X, 0);
        }
        for i in 0..MAX_OBJECTS {
            if self.g.m.u8(EXISTS + i as u32) == 0 {
                crate::collisions::obj_reset(&mut self.objs[i]);
            }
        }
        for i in 0..8usize {
            if self.g.m.u8(EXISTS + 10 + i as u32) == 0 && self.g.m.u8(EXISTS + i as u32) == 0 {
                let t = self.objs[10 + i].team();
                self.objs[i].set_team(t);
            }
        }
        self.g.set_rng_c34(0);
        self.g.set_tick_mod12(0);
        self.g.set_tick_mod3(0);
        self.g.set_tick_parity(0);
        self.replay = Some(r);
    }

    /// `replay_restore` (0x43df00): the difficulty and names from before the replay.
    pub(crate) fn replay_restore(&mut self) {
        self.g.m.set_i32(crate::post_update::DIFFICULTY, self.g.m.u8(0x45877c) as i8 as i32);
        for p in 0..8u32 {
            let n = self.g_cstr(0x458850 + 0xb * p, 11);
            self.g_put_cstr(0x44fcc0 + 0xb * p, &n);
        }
        for (save, live) in [(0x4587e8u32, 0x44fd18u32), (0x458588, 0x44f900), (0x458780, 0x44f890)] {
            let n = self.g_cstr(save, 0x200);
            self.g_put_cstr(live, &n);
        }
        if let Some(r) = &self.replay {
            let (a, b) = (rd(r, 0x630bb8), rd(r, 0x630bbc));
            self.g.set_u_458428(a);
            self.g.m.set_i32(0x45842c, b);
        }
    }

    /// The match summary of 0x421d00 (end_timer 101 while recording): who played and
    /// how, then the recording is saved.
    pub(crate) fn recording_summary(&mut self) {
        let mode = self.g.game_mode();
        let winner = self.g.m.i32(0x450bf8);
        let stage_lost = mode == 1 && {
            // iStack_5e0 of the tick: no player character alive
            !(0..MAX_OBJECTS).any(|j| {
                self.exists(j) && {
                    let o = &self.objs[j];
                    let t = o.team();
                    0 < o.hp() && self.data_of(j).kind == 0 && 0 < t && t < 0x28 && t != 5
                }
            })
        };
        let mut r = match self.recording.take() {
            Some(r) => r,
            None => return,
        };
        let mut count = 0;
        for k in 0..8usize {
            let slot = 0x14 + 4 * k;
            let obj = if self.exists(k) {
                Some(k)
            } else if self.exists(k + 10) {
                Some(k + 10)
            } else {
                wr(&mut r, slot, -1);
                None
            };
            let Some(i) = obj else { continue };
            let o = &self.objs[i];
            wr(&mut r, slot + 0x20, self.data_of(i).id);
            wr(&mut r, slot, (i < 10) as i32);
            wr(&mut r, slot + 0x40, o.team());
            for (off, f) in [(0x60, 0x358), (0x80, 0x348), (0xa0, 0x34c), (0xc0, 0x350), (0xe0, 0x35c)] {
                wr(&mut r, slot + off, o.m.i32(f));
            }
            if mode == 1 {
                wr(&mut r, slot + 0x100, if stage_lost { -1 } else { (0 < o.hp()) as i32 + 1 });
            } else if -1 < winner {
                wr(&mut r, slot + 0x100, if winner == o.team() { (0 < o.hp()) as i32 + 1 } else { -1 });
            }
            count += 1;
        }
        wr(&mut r, 0x10, count);
        if mode == 4 {
            for (k, a) in [0x451b64u32, 0x451b68, 0x451b6c, 0x451b70].iter().enumerate() {
                wr(&mut r, 0x134 + 4 * k, self.g.m.i32(*a));
            }
        }
        wr(&mut r, 0x144, self.g.m.i32(0x450bbc));
        self.g.m.set_i32(0x450b6c, 0);
        self.g.m.set_i32(0x450b70, 2);
        if self.replay.is_some() && self.g.demo() != 0 {
            self.replay_restore();
        }
        wr(&mut r, 0x8ac, self.g.stage_phase());
        for (k, a) in [0x450c18u32, 0x450c1c, 0x450c20, 0x450c24].iter().enumerate() {
            wr(&mut r, 0x8b0 + 4 * k, self.g.m.i32(*a));
        }
        let alive5 = (0..20)
            .filter(|&j| self.exists(j) && self.data_of(j).kind == 0 && self.objs[j].team() == 5 && 0 < self.objs[j].hp())
            .count();
        if mode == 1 {
            wr(&mut r, 0x8c0, (alive5 < 1) as i32);
        } else if mode == 4 {
            let g = |a: u32| self.g.m.i32(a);
            let (a0, s0, d0, a1, s1, d1) = (g(0x44d380), g(0x451b74), g(0x44d758), g(0x44d384), g(0x451b78), g(0x44d75c));
            let code = (d1 % 100) / 10
                + (d1 / 100 + (s1 + 0x1adbb + (a1 + ((d0 % 100) / 10 + (d0 / 100 + (s0 + a0 * 10) * 10) * 10) * 10) * 10) * 10) * 10;
            wr(&mut r, 0x8c0, code);
        }
        self.requests.push(crate::menu::main_menu::MenuRequest::SaveRecording(r));
        self.g.m.set_i32(REC, 0);
        self.g.set_recording(0);
    }
}
