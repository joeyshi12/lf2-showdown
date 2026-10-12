//! Starting a VS match without the original's menus: the state the pick screen and
//! the "Fight!" code (in `game_menus`, 0x42d0xx) leave behind.

use crate::fields::{Globals, Obj};
use crate::world::{World, MAX_OBJECTS, PLAYER_OF_SLOT, RNG_TABLE};
use lf2_data::GameData;
use std::sync::Arc;

/// One fighter at the start of a VS match.
#[derive(Clone, Debug)]
pub struct Fighter {
    /// Pick-screen slot 0..7. Humans use object `slot`, computer players `slot + 10`.
    pub slot: usize,
    /// File-manager index of the character's data.
    pub data: usize,
    /// 0 for "Independent" (the game then uses the object index + 10), else 1..4.
    pub team: i32,
    pub human: bool,
    /// Start position; `None` draws it from the RNG as the original does.
    pub pos: Option<(i32, i32)>,
}

#[derive(Clone, Debug)]
pub struct VsSetup {
    pub fighters: Vec<Fighter>,
    /// Index into the background list (data.txt order).
    pub background: i32,
    /// 0 easy, 1 normal, 2 difficult (`DAT_00450c30`).
    pub difficulty: i32,
    /// Seed for the 3000-byte random table (the original seeds with timeGetTime).
    pub seed: u32,
}

/// The CRT's `rand()` sequence for a seed.
pub struct CrtRand(pub u32);

impl CrtRand {
    pub fn next(&mut self) -> i32 {
        self.0 = self.0.wrapping_mul(214013).wrapping_add(2531011);
        ((self.0 >> 16) & 0x7fff) as i32
    }
}

/// Keys of player 1..4 as written in the default data\control.txt.
pub const DEFAULT_CONTROLS: [[i32; 7]; 4] = [
    [87, 83, 65, 68, 74, 75, 76],
    [87, 88, 65, 68, 83, 9, 192],
    [38, 40, 37, 39, 13, 16, 17],
    [73, 188, 74, 76, 75, 32, 190],
];

impl World {
    /// The globals as lf2.exe starts: its initialised .data section, the rest zero.
    pub fn initial_globals(exe: &[u8]) -> Globals {
        let mut g = Globals::new();
        let n = crate::trace::EXE_DATA_LEN;
        let off = crate::trace::EXE_DATA_OFFSET;
        if let Some(d) = exe.get(off..off + n) {
            g.m.bytes[..n].copy_from_slice(d);
        }
        // sound_init (0x401970) at start-up: the DirectSound object exists, so
        // flush_sounds plays sounds (only its null-ness matters)
        g.m.set_i32(crate::sound::SOUND_ON, 1);
        g
    }

    /// Set up a VS match. `exe` is the contents of lf2.exe (for the initial .data).
    pub fn new_vs_match(gd: Arc<GameData>, exe: &[u8], s: &VsSetup, controls: &[[i32; 7]; 4]) -> World {
        let mut g = Self::initial_globals(exe);
        // menu and match state
        g.m.set_i32(crate::world::GAME, 2);
        g.set_match_phase(0);
        g.set_background(s.background);
        g.set_mp_cost_on(1);
        g.set_match_first_frame(0);
        g.set_game_mode(0);
        g.m.set_i32(crate::post_update::DIFFICULTY, s.difficulty);
        g.m.set_i32(0x44d02c, 1); // 33 ms frames
        g.m.set_i32(0x44dce4, 2); // the frame function's first-run setup is done
        for (p, keys) in controls.iter().enumerate() {
            let base = crate::world::CONTROLS + (p as u32 + 1) * 0x50;
            g.m.set_i32(base, 0);
            for (k, &vk) in keys.iter().enumerate() {
                g.m.set_i32(base + 4 + 4 * k as u32, vk);
            }
        }
        for slot in 0..8u32 {
            g.m.set_i32(PLAYER_OF_SLOT + 4 * slot, 0);
        }
        for (slot, p) in [1, 2, 3, 4].iter().enumerate() {
            g.m.set_i32(PLAYER_OF_SLOT + 4 * slot as u32, *p);
        }
        // the random table, filled when "game start" is clicked (0x422ac0)
        let mut r = CrtRand(s.seed);
        for i in 0..3000u32 {
            let v = r.next();
            g.m.set_u8(RNG_TABLE + i, ((v % 0xff) + 1) as u8);
        }
        g.m.set_i32(0x450b48, 0);
        g.set_crt_seed(r.0 as i32);
        // every object starts as obj_reset with the first data file (0x41bd??)
        let mut objs = Vec::with_capacity(MAX_OBJECTS);
        for _ in 0..MAX_OBJECTS {
            let mut o = Obj::new();
            crate::collisions::obj_reset(&mut o);
            o.m.set_i32(0x368, 0);
            o.set_x(200.0);
            o.set_y(0.0);
            o.set_z(300.0);
            objs.push(o);
        }
        let mut w = World::with_state(g, objs, gd);
        let (bw, z0, z1) = {
            let b = &w.gd.backgrounds[s.background as usize];
            (b.width, b.zboundary.0, b.zboundary.1)
        };
        // "Fight!" (0x42d0xx): place each fighter
        for f in &s.fighters {
            let i = if f.human { f.slot } else { f.slot + 10 };
            let hp = w.gd.objects[f.data].weapon_hp;
            let o = &mut w.objs[i];
            crate::collisions::obj_reset(o);
            o.m.set_i32(0x368, f.data as i32);
            o.set_x(if f.human { 580.0 } else { 0.0 });
            o.set_y(-200.0);
            o.set_weapon_hp(hp);
            o.set_z(300.0);
            o.set_team(f.team);
            o.set_u_08(0x4b);
            w.set_exists(i, true);
            let (x, z) = match f.pos {
                Some(p) => p,
                None => {
                    let x = w.random(if f.human { 0xdd } else { 0xdb }, bw / 2) + bw / 4;
                    let z = w.random(if f.human { 0xde } else { 0xdc }, z1 - z0) + z0;
                    (x, z)
                }
            };
            let o = &mut w.objs[i];
            o.set_ix(x);
            o.set_iz(z);
            o.set_iy(0);
            o.set_x(x as f64);
            o.set_z(z as f64);
            o.set_y(0.0);
            o.set_mp(200);
            o.m.set_i32(0x354, i as i32);
        }
        for i in 0..MAX_OBJECTS {
            if w.exists(i) && w.objs[i].team() == 0 {
                w.objs[i].set_team(i as i32 + 10);
            }
        }
        w.g.set_end_timer(0);
        w.g.m.set_i32(0x450bbc, 0);
        w
    }
}

/// Write a C string into the globals (with its terminator).
fn put_cstr(g: &mut Globals, at: u32, s: &[u8]) {
    for (k, &b) in s.iter().chain(std::iter::once(&0)).enumerate() {
        g.m.set_u8(at + k as u32, b);
    }
}

impl World {
    /// Title screen after WinMain 0x43cf40 loads controls and ads and calls
    /// `srand(seed)`.
    pub fn new_title(
        gd: Arc<GameData>,
        exe: &[u8],
        controls: &lf2_data::controls::Controls,
        ads: Option<&lf2_data::ads::Ads>,
        seed: u32,
    ) -> World {
        let mut g = Self::initial_globals(exe);
        g.set_crt_seed(seed as i32);
        // load_controls (0x423480)
        for (p, v) in controls.players.iter().enumerate() {
            for (k, &x) in v.iter().enumerate() {
                g.m.set_i32(0x44fb70 + 0x50 * p as u32 + 4 * k as u32, x);
            }
        }
        // the top menu's first frame names slots 1..8 "1".."8", then control.txt
        // gives the names of players 1..4
        for p in 0..8u32 {
            put_cstr(&mut g, 0x44fcc0 + 0xb * p, &[b'1' + p as u8]);
        }
        for (p, n) in controls.names.iter().enumerate() {
            put_cstr(&mut g, 0x44fcc0 + 0xb * p as u32, &n[..n.len().min(10)]);
        }
        g.m.set_i32(0x450be8, controls.ads);
        g.m.set_i32(0x450be4, controls.record);
        put_cstr(&mut g, 0x44fd18, &controls.name[..controls.name.len().min(99)]);
        put_cstr(&mut g, 0x44f890, &controls.email[..controls.email.len().min(99)]);
        put_cstr(&mut g, 0x44f900, &controls.info[..controls.info.len().min(0x3ff)]);
        // ads_load_info / ads_load (0x43c4a0, 0x43c780)
        if let Some(a) = ads {
            put_cstr(&mut g, 0x4527b0, &a.stamp);
            g.m.set_i32(0x44d784, a.index);
            g.m.set_i32(0x44d788, a.info2);
            put_cstr(&mut g, 0x453c68, format!("data\\ad{}.txt", a.index).as_bytes());
            put_cstr(&mut g, 0x453d40, format!("sprite\\sys\\ad{}.bmp", a.index).as_bytes());
            g.m.set_i32(0x44d778, a.date);
            for (i, b) in a.boxes.iter().enumerate() {
                let i = i as u32;
                g.m.set_i32(0x4583b8 + 4 * i, b.x);
                g.m.set_i32(0x453c08 + 4 * i, b.y);
                g.m.set_i32(0x453f70 + 4 * i, b.w);
                g.m.set_i32(0x453ce0 + 4 * i, b.h);
                put_cstr(&mut g, 0x454a18 + 100 * i, &b.url);
            }
            for (i, p) in a.pictures.iter().enumerate() {
                let i = i as u32;
                g.m.set_i32(0x452928 + 4 * i, p.start);
                g.m.set_i32(0x4546d0 + 4 * i, p.end);
                g.m.set_i32(0x453f50 + 4 * i, p.len);
                put_cstr(&mut g, 0x4546f8 + 100 * i, &p.url);
            }
            g.m.set_i32(0x4554bc, a.new_version);
            put_cstr(&mut g, 0x453da8, &a.new_version_url);
            g.m.set_i32(0x453da4, a.menu_y);
            g.m.set_i32(0x45757c, a.y2);
            // the ad picture surface (ads_load_images, 0x43cc60); a marker, not a pointer
            g.m.set_i32(crate::menu::main_menu::AD_SHEET, 1);
        }
        // the menu pictures are loaded on the first title frame (0x44d068)
        g.m.set_i32(0x44d068, 0);
        g.m.set_i32(0x44d060, 0);
        g.m.set_i32(0x44dce4, 2);
        for k in 0..256u32 {
            g.m.set_u8(crate::world::KEYS + k, b'u');
        }
        let objs = (0..MAX_OBJECTS).map(|_| Obj::new()).collect();
        World::with_state(g, objs, gd)
    }

    /// Main menu state after WinMain 0x43cf40, "Game start" at 0x4246b0 and the
    /// first match frame at 0x41bc90.
    pub fn new_boot(
        gd: Arc<GameData>,
        exe: &[u8],
        controls: &lf2_data::controls::Controls,
        ads: Option<&lf2_data::ads::Ads>,
        seed: u32,
    ) -> World {
        let mut w = World::new_title(gd, exe, controls, ads, seed);
        w.game_start();
        // the frame of game state 1 ("loading"), then the first match frame, which
        // loads the data and runs the main menu's first frame
        let keys = [b'u'; 256];
        w.set_keys(&keys);
        w.step();
        w.set_keys(&keys);
        w.step();
        w
    }

    /// The start of the first match frame (match_first_frame 1, 0x41bc90): the data
    /// are loaded and objects 0..7 are reset; the frame then goes on as any other.
    pub(crate) fn data_loaded(&mut self) {
        let gd = self.gd.clone();
        let g = &mut self.g;
        g.set_match_first_frame(0);
        g.m.set_i32(0x44dce4, 2);
        g.m.set_i32(0x44f620, gd.checksum);
        for i in 0..MAX_OBJECTS {
            let o = &mut self.objs[i];
            *o = Obj::new();
            crate::collisions::obj_reset(o);
            o.m.set_i32(0x368, 0);
            let x = match i {
                0 => 200.0,
                1..=3 => 210.0,
                4 | 6 => 580.0,
                5 | 7 => 570.0,
                _ => 200.0,
            };
            o.set_x(x);
            o.set_y(if i == 4 || i == 6 { -200.0 } else { 0.0 });
            o.set_z(300.0);
            if i < 8 {
                o.set_weapon_hp(gd.objects[0].weapon_hp);
            }
        }
        for i in 0..8u32 {
            g.m.set_u8(crate::world::EXISTS + i, 1);
        }
    }
}
