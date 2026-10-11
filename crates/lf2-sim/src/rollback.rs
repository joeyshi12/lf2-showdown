//! Rollback for the network game: every computer runs the frames as soon as its
//! own input is there, guessing the inputs of the others from the last ones that
//! arrived. When an input arrives that differs from the guess, the world goes back
//! to the frame it belongs to and runs the frames since then again with it.
//!
//! A frame here is one call of `World::step` from the network game's start (the
//! "loading" frame) on. The match frame reads keys only every second frame
//! (half_frame 0); there the packet holds every computer's keys of that frame and
//! the F-keys of that frame and the one before, so no F-key press is lost.
//!
//! The state saved before each frame is everything the simulation reads: the
//! globals, the objects, and the parts of `World` outside them (stage spawn counters,
//! the recording, the menu background).

use crate::fields::{Globals, Obj};
use crate::menu::main_menu::{MOUSE_CLICK, MOUSE_X, MOUSE_Y};
use crate::world::{World, EXISTS, MAX_OBJECTS};
use std::collections::VecDeque;

pub const MAX_SEATS: usize = 4;
/// One computer's input for one frame: its player's keys as the packet byte (up
/// 0x80, down 0x40, left 0x20, right 0x10, attack 8, jump 4, defend 2) and its
/// F-keys as packet bytes 10 and 12.
pub type Input = [u8; 3];
/// The most frames a computer runs past the last frame whose inputs it has from
/// every computer (8 frames, about a quarter of a second).
pub const MAX_PREDICT: u32 = 8;
/// A computer's input is used this many frames after it is read, so that on a
/// short connection it arrives at the others before it is needed.
pub const INPUT_DELAY: u32 = 1;
/// The game state is hashed every this many frames, to find computers whose games
/// differ.
pub const HASH_EVERY: u32 = 16;
/// The frame period the network game's clock (`World::time_ms`) counts in.
const FRAME_MS: u32 = 33;

/// The state of the world before one frame.
pub struct State {
    g: Globals,
    objs: Vec<Obj>,
    spawn_rt: Vec<crate::stage::SpawnRt>,
    menu_back: i32,
    control_txt: Option<Vec<u8>>,
    recording: Option<Vec<u8>>,
    replay: Option<Vec<u8>>,
    rec_frame: [u8; 10],
    replay_frame: [u8; 10],
    time_ms: u32,
}

impl State {
    fn hash(&self) -> u64 {
        state_hash(&self.g, &self.objs)
    }
}

/// FNV-1a over what decides how the game goes on: which objects exist, the
/// objects themselves, the random numbers' positions and the match counters.
/// (Not the whole globals: the room code, the mouse and the volume differ between
/// computers and do not matter.)
pub fn state_hash(g: &Globals, objs: &[Obj]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    let mut eat = |b: &[u8]| {
        for &x in b {
            h ^= x as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
    };
    for i in 0..MAX_OBJECTS {
        let e = g.m.u8(EXISTS + i as u32);
        eat(&[e]);
        if e != 0 {
            eat(&objs[i].m.bytes);
        }
    }
    for v in [g.rng_c34(), g.rng_bcc(), g.crt_seed(), g.match_phase(), g.match_frames(), g.half_frame(), g.game_mode()] {
        eat(&v.to_le_bytes());
    }
    h
}

impl World {
    pub fn save_state(&self) -> State {
        State {
            g: self.g.clone(),
            objs: self.objs.clone(),
            spawn_rt: self.spawn_rt.clone(),
            menu_back: self.menu_back,
            control_txt: self.control_txt.clone(),
            recording: self.recording.clone(),
            replay: self.replay.clone(),
            rec_frame: self.rec_frame,
            replay_frame: self.replay_frame,
            time_ms: self.time_ms,
        }
    }

    /// Save into `s`, reusing its buffers.
    fn save_state_into(&self, s: &mut State) {
        s.g.m.bytes.copy_from_slice(&self.g.m.bytes);
        for (a, b) in s.objs.iter_mut().zip(&self.objs) {
            a.m.bytes.copy_from_slice(&b.m.bytes);
        }
        s.spawn_rt.clone_from(&self.spawn_rt);
        s.menu_back = self.menu_back;
        s.control_txt.clone_from(&self.control_txt);
        s.recording.clone_from(&self.recording);
        s.replay.clone_from(&self.replay);
        s.rec_frame = self.rec_frame;
        s.replay_frame = self.replay_frame;
        s.time_ms = self.time_ms;
    }

    pub fn load_state(&mut self, s: &State) {
        self.g.m.bytes.copy_from_slice(&s.g.m.bytes);
        for (a, b) in self.objs.iter_mut().zip(&s.objs) {
            a.m.bytes.copy_from_slice(&b.m.bytes);
        }
        self.spawn_rt.clone_from(&s.spawn_rt);
        self.menu_back = s.menu_back;
        self.control_txt.clone_from(&s.control_txt);
        self.recording.clone_from(&s.recording);
        self.replay.clone_from(&s.replay);
        self.rec_frame = s.rec_frame;
        self.replay_frame = s.replay_frame;
        self.time_ms = s.time_ms;
    }

    pub fn state_hash(&self) -> u64 {
        state_hash(&self.g, &self.objs)
    }

    /// One frame of the network game. `now[k]` is computer `k`'s input for this
    /// frame, `before[k]` its input for the frame before. The keys, the mouse and
    /// the clock are the same on every computer.
    pub fn net_step(&mut self, now: &[Input], before: &[Input], frame: u32) {
        let mut p = [0u8; crate::net::PACKET];
        for (s, i) in now.iter().enumerate() {
            p[s] = i[0] | 1;
            p[10] |= i[1] | before[s][1];
            p[12] |= i[2] | before[s][2];
        }
        self.net_in = p;
        self.set_keys(&[b'u'; 256]);
        self.g.m.set_i32(MOUSE_X, 0);
        self.g.m.set_i32(MOUSE_Y, 0);
        self.g.m.set_i32(MOUSE_CLICK, 0);
        self.time_ms = frame.wrapping_mul(FRAME_MS);
        self.step();
    }
}

/// The inputs and saved states of a network game on one computer.
pub struct Rollback {
    seats: usize,
    me: usize,
    /// The frames run: the world is the state before frame `frame`.
    frame: u32,
    /// Each seat's inputs from frame 0 on, as far as they have arrived in order.
    inputs: Vec<Vec<Input>>,
    /// A seat whose computer left presses nothing from this frame on.
    left: [Option<u32>; MAX_SEATS],
    /// The inputs each frame from `base` on was run with.
    used: VecDeque<[Input; MAX_SEATS]>,
    /// The state before each frame from `base` on.
    states: VecDeque<State>,
    /// States no longer needed, kept to reuse their buffers.
    spare: Vec<State>,
    base: u32,
    /// The earliest frame run with a guess that turned out wrong.
    redo_from: Option<u32>,
    /// Frames up to here have been hashed.
    hashed: u32,
    new_hashes: Vec<(u32, u64)>,
    /// Statistics: frames run again, rollbacks, frames waited.
    pub resimulated: u64,
    pub rollbacks: u64,
    pub stalls: u64,
}

impl Rollback {
    pub fn new(seats: usize, me: usize) -> Rollback {
        let mut inputs = vec![Vec::new(); seats];
        // the first INPUT_DELAY frames of every computer press nothing
        for i in inputs.iter_mut() {
            i.resize(INPUT_DELAY as usize, [0; 3]);
        }
        Rollback {
            seats,
            me,
            frame: 0,
            inputs,
            left: [None; MAX_SEATS],
            used: VecDeque::new(),
            states: VecDeque::new(),
            spare: Vec::new(),
            base: 0,
            redo_from: None,
            hashed: 0,
            new_hashes: Vec::new(),
            resimulated: 0,
            rollbacks: 0,
            stalls: 0,
        }
    }

    pub fn seats(&self) -> usize {
        self.seats
    }

    pub fn me(&self) -> usize {
        self.me
    }

    pub fn frame(&self) -> u32 {
        self.frame
    }

    /// How many of the seat's inputs from frame 0 on are known (all from the frame
    /// its computer left on).
    pub fn known(&self, seat: usize) -> u32 {
        if self.left[seat].is_some() {
            return u32::MAX;
        }
        self.inputs[seat].len() as u32
    }

    /// How many of the seat's inputs have arrived.
    pub fn input_count(&self, seat: usize) -> u32 {
        self.inputs[seat].len() as u32
    }

    /// The frames whose inputs are all known.
    pub fn confirmed(&self) -> u32 {
        (0..self.seats).map(|s| self.known(s)).min().unwrap_or(0)
    }

    /// A known input (`f` < `input_count`).
    pub fn input(&self, seat: usize, f: u32) -> Input {
        self.inputs[seat][f as usize]
    }

    /// The input to run frame `f` with: the one that arrived, or the guess (the
    /// last keys that arrived, no F-keys).
    fn input_for(&self, seat: usize, f: u32) -> Input {
        if let Some(i) = self.inputs[seat].get(f as usize) {
            return *i;
        }
        if self.left[seat].is_some_and(|at| f >= at) {
            return [0; 3];
        }
        match self.inputs[seat].last() {
            Some(i) => [i[0], 0, 0],
            None => [0; 3],
        }
    }

    fn inputs_for(&self, f: u32) -> [Input; MAX_SEATS] {
        let mut a = [[0; 3]; MAX_SEATS];
        for (s, i) in a.iter_mut().enumerate().take(self.seats) {
            *i = self.input_for(s, f);
        }
        a
    }

    /// An input from another computer. Inputs that are not the next one of their
    /// seat are left out (they come again).
    pub fn add_input(&mut self, seat: usize, f: u32, input: Input) {
        if self.left[seat].is_some() || f as usize != self.inputs[seat].len() {
            return;
        }
        if f < self.frame && f >= self.base {
            if self.used[(f - self.base) as usize][seat] != input {
                self.redo_from = Some(self.redo_from.map_or(f, |r| r.min(f)));
            }
        }
        self.inputs[seat].push(input);
    }

    /// The seat's computer left: from frame `at` on it presses nothing.
    pub fn leave(&mut self, seat: usize, at: u32) {
        if self.left[seat].is_some() {
            return;
        }
        let at = at.max(self.inputs[seat].len() as u32);
        for f in self.inputs[seat].len() as u32..at {
            // inputs that never arrived count as nothing pressed
            self.add_input(seat, f, [0; 3]);
        }
        if at < self.frame && at >= self.base {
            for f in at..self.frame {
                if self.used[(f - self.base) as usize][seat] != [0; 3] {
                    self.redo_from = Some(self.redo_from.map_or(f, |r| r.min(f)));
                    break;
                }
            }
        }
        self.left[seat] = Some(at);
    }

    /// Hashes of the state before frames that every computer's inputs are known
    /// for, since the last call.
    pub fn take_hashes(&mut self) -> Vec<(u32, u64)> {
        std::mem::take(&mut self.new_hashes)
    }

    fn run(&mut self, w: &mut World, f: u32, clock: &mut u32) {
        let now = self.inputs_for(f);
        let before = if f == 0 { [[0; 3]; MAX_SEATS] } else { self.inputs_for(f - 1) };
        let mut s = self.spare.pop().unwrap_or_else(|| w.save_state());
        w.save_state_into(&mut s);
        self.states.push_back(s);
        self.used.push_back(now);
        w.net_step(&now[..self.seats], &before[..self.seats], f);
        *clock = clock.wrapping_add(1);
    }

    /// Run frames: first again from the earliest wrong guess, then up to `want`
    /// new frames with `local` as this computer's input, while no computer's inputs
    /// are more than MAX_PREDICT frames behind. Only the new frames' sounds, music
    /// and requests are kept. Returns the number of new frames.
    pub fn advance(&mut self, w: &mut World, local: Input, want: u32, clock: &mut u32) -> u32 {
        if let Some(from) = self.redo_from.take() {
            let from = from.max(self.base);
            let end = self.frame;
            let k = (from - self.base) as usize;
            w.load_state(&self.states[k]);
            while self.states.len() > k {
                self.spare.push(self.states.pop_back().unwrap());
                self.used.pop_back();
            }
            let requests = w.requests.len();
            for f in from..end {
                self.run(w, f, clock);
            }
            w.requests.truncate(requests);
            self.resimulated += (end - from) as u64;
            self.rollbacks += 1;
        }
        let mut sounds = Vec::new();
        let mut buffer_sounds = Vec::new();
        let mut music = Vec::new();
        let mut ran = 0;
        while ran < want && self.frame - self.confirmed().min(self.frame) < MAX_PREDICT {
            let f = self.frame;
            // this computer's input for the frame INPUT_DELAY ahead
            if self.inputs[self.me].len() as u32 == f + INPUT_DELAY {
                self.inputs[self.me].push(local);
            }
            self.run(w, f, clock);
            self.frame += 1;
            sounds.append(&mut w.sounds);
            buffer_sounds.append(&mut w.buffer_sounds);
            music.append(&mut w.music);
            ran += 1;
        }
        if ran == 0 {
            self.stalls += (want > 0) as u64;
            w.sounds.clear();
            w.buffer_sounds.clear();
            w.music.clear();
        } else {
            w.sounds = sounds;
            w.buffer_sounds = buffer_sounds;
            w.music = music;
        }
        // hash the states every computer's inputs are known for, then drop them
        let confirmed = self.confirmed().min(self.frame);
        while self.hashed <= confirmed {
            let f = self.hashed;
            if f % HASH_EVERY == 0 && f >= self.base {
                let h = if f == self.frame { w.state_hash() } else { self.states[(f - self.base) as usize].hash() };
                self.new_hashes.push((f, h));
            }
            self.hashed += 1;
        }
        while self.base < confirmed && !self.states.is_empty() {
            self.spare.push(self.states.pop_front().unwrap());
            self.used.pop_front();
            self.base += 1;
        }
        self.spare.truncate(4);
        ran
    }
}
