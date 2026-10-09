//! `data/stage.dat`, a port of `FUN_0040c910`.
//!
//! The original stores 60 stages of 0x149b08 bytes at file manager `+0x7d0`; each stage
//! has a phase count at `+0` and 100 phases of 0x34c0 bytes at `+8`; each phase has
//! `bound` at `+0`, `music` at `+4`, `when_clear_goto_phase` at `+0x34b8` and 60 spawn
//! slots of 0xe0 bytes at `+0xe4`.

use crate::scan::Scanner;

pub const MAX_STAGES: usize = 60;
pub const MAX_PHASES: usize = 100;
pub const MAX_SPAWNS: usize = 60;

#[derive(Clone, Debug, PartialEq)]
pub struct Spawn {
    pub id: i32,           // @0x00
    pub x: i32,            // @0x04
    pub hp: i32,           // @0x08
    pub times: i32,        // @0x0c
    pub reserve: i32,      // @0x10
    pub join: i32,         // @0x14
    pub join_reserve: i32, // @0x18
    pub act: i32,          // @0x1c
    pub y: i32,            // @0x20
    pub ratio: f64,        // @0x24
    /// 0 normal, 1 `<soldier>`, 2 `<boss>`.
    pub role: i32,         // @0x2c
}

impl Default for Spawn {
    fn default() -> Self {
        Spawn {
            id: -1,
            x: 500,
            hp: 500,
            times: 1,
            reserve: 0,
            join: 0,
            join_reserve: 0,
            act: 9,
            y: 0,
            ratio: 0.0,
            role: 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Phase {
    pub bound: i32,
    pub music: Vec<u8>,
    pub when_clear_goto_phase: i32,
    /// Only the spawns that had an `id:`; the original's unused slots keep the defaults.
    pub spawns: Vec<Spawn>,
    /// The `x` that unused slots hold (`500`, or `bound + 80` after `bound:`).
    pub default_x: i32,
}

impl Default for Phase {
    fn default() -> Self {
        Phase { bound: -1, music: Vec::new(), when_clear_goto_phase: -1, spawns: Vec::new(), default_x: 500 }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Stage {
    pub id: i32,
    pub phases: Vec<Phase>,
}

fn int(s: &mut Scanner, into: &mut i32) {
    if let Some(v) = s.int() {
        *into = v;
    }
}

/// Parse decoded stage.dat text. Stages are returned in file order. Stage ids outside
/// 0..60 are kept although the original would write out of bounds.
pub fn parse_stages(text: &[u8]) -> Vec<Stage> {
    let mut out: Vec<Stage> = Vec::new();
    let mut s = Scanner::new(text);
    let mut tok = Vec::new();
    while !s.eof() {
        s.token(&mut tok);
        if tok != b"<stage>" {
            continue;
        }
        let mut stage = Stage { id: -1, ..Default::default() };
        loop {
            if !s.token(&mut tok) && s.eof() {
                break;
            }
            if tok == b"id:" {
                // the original writes into the slot of the id it reads: a stage
                // without <stage_end> ends where the next id: starts a new slot
                if stage.id != -1 || !stage.phases.is_empty() {
                    let done = std::mem::take(&mut stage);
                    out.retain(|x: &Stage| x.id != done.id);
                    out.push(done);
                }
                stage.id = -1;
                int(&mut s, &mut stage.id);
                stage.phases.clear();
            }
            if tok == b"<phase>" {
                let mut ph = Phase::default();
                loop {
                    if !s.token(&mut tok) && s.eof() {
                        break;
                    }
                    match tok.as_slice() {
                        b"bound:" => {
                            int(&mut s, &mut ph.bound);
                            ph.default_x = ph.bound.wrapping_add(80);
                            for sp in ph.spawns.iter_mut() {
                                sp.x = ph.default_x;
                            }
                        }
                        b"id:" => {
                            let mut sp = Spawn { x: ph.default_x, ..Default::default() };
                            int(&mut s, &mut sp.id);
                            ph.spawns.push(sp);
                        }
                        b"music:" => {
                            s.token(&mut ph.music);
                        }
                        b"when_clear_goto_phase:" => int(&mut s, &mut ph.when_clear_goto_phase),
                        _ => {
                            if let Some(sp) = ph.spawns.last_mut() {
                                match tok.as_slice() {
                                    b"x:" => int(&mut s, &mut sp.x),
                                    b"hp:" => int(&mut s, &mut sp.hp),
                                    b"times:" => int(&mut s, &mut sp.times),
                                    b"reserve:" => int(&mut s, &mut sp.reserve),
                                    b"<boss>" => sp.role = 2,
                                    b"join:" => int(&mut s, &mut sp.join),
                                    b"join_reserve:" => int(&mut s, &mut sp.join_reserve),
                                    b"<soldier>" => {
                                        sp.role = 1;
                                        sp.times = 50;
                                    }
                                    b"ratio:" => {
                                        if let Some(v) = s.float() {
                                            sp.ratio = v;
                                        }
                                    }
                                    b"y:" => int(&mut s, &mut sp.y),
                                    b"act:" => int(&mut s, &mut sp.act),
                                    _ => {}
                                }
                            }
                        }
                    }
                    if tok == b"<phase_end>" {
                        break;
                    }
                }
                stage.phases.push(ph);
            }
            if tok == b"<stage_end>" {
                break;
            }
        }
        out.retain(|x| x.id != stage.id);
        out.push(stage);
    }
    out
}
