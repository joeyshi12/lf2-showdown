//! Sound requests. The simulation only queues sounds: `play_sound` (0x417090) for the
//! system wave files and `play_sound_id` (0x416fb0) for sounds named in data files.
//! Each id keeps a left and right volume (0..100) summed over the requests of the
//! frame; `flush_sounds` (0x419e60) hands them to the audio layer at the end of a frame.

use crate::world::World;

/// System sounds: requested flag, left, right (80 entries).
const SYS_FLAG: u32 = 0x453e10;
const SYS_LEFT: u32 = 0x4527e8;
const SYS_RIGHT: u32 = 0x4554c8;
/// Data sounds: requested flag, left, right (400 entries).
const DATA_FLAG: u32 = 0x457588;
const DATA_LEFT: u32 = 0x457bc8;
const DATA_RIGHT: u32 = 0x452170;
/// Nonzero when DirectSound started.
pub const SOUND_ON: u32 = 0x44eecc;

/// One sound to play: `system` selects the table, `pan` is -1500..1500 hundredths
/// of a decibel as DirectSound takes it, `volume` 1..100.
#[derive(Clone, Debug, PartialEq)]
pub struct SoundEvent {
    pub system: bool,
    pub id: usize,
    pub left: i32,
    pub right: i32,
}

fn ear(d: i32) -> i32 {
    let d = d.wrapping_abs();
    if d < 200 {
        100
    } else if d < 400 {
        ((400 - d) * 100) / 200
    } else {
        0
    }
}

impl World {
    fn queue_sound(&mut self, x: i32, id: i32, flag: u32, left: u32, right: u32) {
        if id < 0 {
            return;
        }
        let cam = self.g.m.i32(crate::camera::CAMERA_X);
        let l = ear((x - cam) - 200);
        let r = ear((x - cam) - 600);
        let k = 4 * id as u32;
        if self.g.m.i32(flag + k) == 0 {
            self.g.m.set_i32(left + k, 0);
            self.g.m.set_i32(right + k, 0);
        }
        self.g.m.add_i32(left + k, l);
        self.g.m.add_i32(right + k, r);
        self.g.m.set_i32(flag + k, 1);
    }

    /// 0x417090.
    pub fn play_sound(&mut self, x: i32, id: i32) {
        self.queue_sound(x, id, SYS_FLAG, SYS_LEFT, SYS_RIGHT);
    }

    /// 0x416fb0.
    pub fn play_sound_id(&mut self, x: i32, id: i32) {
        self.queue_sound(x, id, DATA_FLAG, DATA_LEFT, DATA_RIGHT);
    }

    /// 0x419e60: clear the request flags and return what to play.
    pub fn flush_sounds(&mut self) -> Vec<SoundEvent> {
        let mut out = Vec::new();
        if self.g.m.i32(SOUND_ON) == 0 {
            return out;
        }
        for (system, flag, left, right, n) in
            [(false, DATA_FLAG, DATA_LEFT, DATA_RIGHT, 400u32), (true, SYS_FLAG, SYS_LEFT, SYS_RIGHT, 80)]
        {
            for id in 0..n {
                let k = 4 * id;
                if self.g.m.i32(flag + k) > 0 {
                    self.g.m.set_i32(flag + k, 0);
                    let (l, r) = (self.g.m.i32(left + k), self.g.m.i32(right + k));
                    if l + r >= 1 {
                        out.push(SoundEvent { system, id: id as usize, left: l, right: r });
                    }
                }
            }
        }
        out
    }
}
