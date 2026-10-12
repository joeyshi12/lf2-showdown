//! The simulation state and the match frame.

use crate::fields::{Globals, Obj};
use crate::trace::Snapshot;
use lf2_data::{Frame, GameData, ObjectData};
use std::sync::Arc;

pub const MAX_OBJECTS: usize = 400;
/// The game object (`this` of the match functions) lives at this address.
pub const GAME: u32 = 0x458b00;
pub const EXISTS: u32 = GAME + 4;
pub const RNG_TABLE: u32 = 0x44ff90;
pub const KEYS: u32 = 0x455378;
/// Player slot per object 0..7 (`DAT_00450b4c`): 1..4 local players, -1 remote.
pub const PLAYER_OF_SLOT: u32 = 0x450b4c;
/// Controls per player p at `CONTROLS + p*0x50`: kind (0 keyboard), then 7 key codes.
pub const CONTROLS: u32 = 0x44fb20;

impl Obj {
    pub(crate) fn shift_keys(&mut self) {
        self.set_prev_up(self.up());
        self.set_prev_down(self.down());
        self.set_prev_left(self.left());
        self.set_prev_right(self.right());
        self.set_prev_attack(self.attack());
        self.set_prev_jump(self.jump());
        self.set_prev_defend(self.defend());
    }
}

pub struct World {
    pub g: Globals,
    pub objs: Vec<Obj>,
    pub gd: Arc<GameData>,
    pub unported_hits: Vec<&'static str>,
    pub control_txt: Option<Vec<u8>>,
    /// Leave out the advertisements (the ad switch and the ad question on the title
    /// screen); a deliberate difference from the original, off in the traces. The
    /// ad banners themselves are not drawn when no ads are loaded (`new_title` with
    /// `ads` None).
    pub ads_removed: bool,
    pub net_in: [u8; crate::net::PACKET],
    pub net_ui: crate::net::NetUi,
    pub local_time: [u16; 6],
    pub sounds: Vec<crate::sound::SoundEvent>,
    /// Wave files played directly with `play_buffer` (0x401a30) in the last frame, at
    /// full volume and centred, such as `data\\m_end.wav`.
    pub buffer_sounds: Vec<&'static str>,
    /// Stage mode: the counter blocks of the current phase's spawn slots.
    pub spawn_rt: Vec<crate::stage::SpawnRt>,
    pub draw: Vec<crate::draw::Draw>,
    /// Milliseconds from the platform clock (`timeGetTime`); only used for cosmetic
    /// choices such as which menu background to show.
    pub time_ms: u32,
    /// The `MENU_BACKn` loaded by `load_menu_background` (0x423840).
    pub menu_back: i32,
    pub requests: Vec<crate::menu::main_menu::MenuRequest>,
    pub music: Vec<Option<Vec<u8>>>,
    pub recording: Option<Vec<u8>>,
    pub replay: Option<Vec<u8>>,
    /// This frame's keys as recorded (`local_210` of 0x41bc90): 8 players, 2 F-key bytes.
    pub rec_frame: [u8; 10],
    /// This frame's keys read from the replay (`local_204`).
    pub replay_frame: [u8; 10],
}

impl World {
    pub fn from_snapshot(s: &Snapshot, gd: Arc<GameData>) -> World {
        let objs = s.objects.iter().map(|o| o.clone().unwrap_or_else(Obj::new)).collect();
        World::with_state(s.globals.clone(), objs, gd)
    }

    pub fn with_state(g: Globals, objs: Vec<Obj>, gd: Arc<GameData>) -> World {
        World {
            g,
            objs,
            control_txt: gd.control_txt.clone(),
            local_time: [0; 6],
            ads_removed: false,
            net_in: [0; crate::net::PACKET],
            net_ui: Default::default(),
            gd,
            unported_hits: Vec::new(),
            sounds: Vec::new(),
            buffer_sounds: Vec::new(),
            spawn_rt: vec![Default::default(); lf2_data::stage::MAX_SPAWNS],
            draw: Vec::new(),
            time_ms: 0,
            menu_back: 1,
            requests: Vec::new(),
            music: Vec::new(),
            recording: None,
            replay: None,
            rec_frame: [0; 10],
            replay_frame: [0; 10],
        }
    }

    #[inline]
    pub fn exists(&self, i: usize) -> bool {
        self.g.m.u8(EXISTS + i as u32) != 0
    }

    #[inline]
    pub fn set_exists(&mut self, i: usize, v: bool) {
        self.g.m.set_u8(EXISTS + i as u32, v as u8)
    }

    /// The object's data file (field 0x368 holds the file-manager index).
    #[inline]
    pub fn data_of(&self, i: usize) -> &ObjectData {
        &self.gd.objects[self.objs[i].m.i32(0x368) as usize]
    }

    #[inline]
    pub fn frame_of(&self, i: usize) -> &Frame {
        &self.data_of(i).frames[self.objs[i].frame() as usize]
    }

    /// 0x417170: the game's random number in 0..n. `site` is the call-site number the
    /// original passes; it does not affect the result.
    pub fn random(&mut self, _site: i32, n: i32) -> i32 {
        if n < 1 {
            return 0;
        }
        let c34 = (self.g.rng_c34() + 1) % 0x4d2;
        self.g.set_rng_c34(c34);
        let bcc = (self.g.rng_bcc() + 1) % 3000;
        self.g.set_rng_bcc(bcc);
        let t = self.g.m.u8(RNG_TABLE + bcc as u32) as i32;
        (t + c34) % n
    }

    /// `play_music` (0x402020): play a bgm file. The file loaded is kept at 0x44ef04;
    /// for that same file the original only calls IMediaControl::Run, which carries
    /// on after `music_stop`. The platform decodes and plays the file, looping it.
    pub fn play_music(&mut self, file: &[u8]) {
        if self.g.m.i32(0x44d010) == 0 {
            return;
        }
        let cur: Vec<u8> = (0..60).map(|k| self.g.m.u8(0x44ef04 + k)).take_while(|&c| c != 0).collect();
        if cur == file {
            self.music.push(Some(file.to_vec()));
            return;
        }
        for (k, &b) in file.iter().chain(std::iter::once(&0)).enumerate() {
            self.g.m.set_u8(0x44ef04 + k as u32, b);
        }
        self.music.push(Some(file.to_vec()));
    }

    /// `music_stop` (0x402100).
    pub fn music_stop(&mut self) {
        self.music.push(None);
    }

    pub fn set_keys(&mut self, keys: &[u8; 256]) {
        self.g.m.bytes[(KEYS - crate::fields::GLOBALS_BASE) as usize..][..256].copy_from_slice(keys);
    }
}
