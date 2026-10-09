//! The byte layout the original uses in memory for loaded data, built from the parsed
//! values. Used to compare this crate's loaders with memory dumps of the running game.
//!
//! Each image comes with a mask of the bytes this crate determines; pointers, surface
//! handles and bytes past a string's terminating NUL are left out of the mask.

use crate::bg::Background;
use crate::object::{ObjectData, MAX_FRAMES};

pub const OBJECT_SIZE: usize = 0x25360;
pub const FRAME_BASE: usize = 0x7a4;
pub const FRAME_SIZE: usize = 0x178;
pub const BG_SIZE: usize = 0x990;

pub struct Image {
    pub bytes: Vec<u8>,
    pub mask: Vec<bool>,
}

impl Image {
    fn new(n: usize) -> Self {
        Image { bytes: vec![0; n], mask: vec![false; n] }
    }
    fn i32(&mut self, at: usize, v: i32) {
        self.raw(at, &v.to_le_bytes());
    }
    fn f64(&mut self, at: usize, v: f64) {
        self.raw(at, &v.to_le_bytes());
    }
    fn u8(&mut self, at: usize, v: u8) {
        self.raw(at, &[v]);
    }
    fn raw(&mut self, at: usize, b: &[u8]) {
        if self.bytes.len() < at + b.len() {
            self.bytes.resize(at + b.len(), 0);
            self.mask.resize(at + b.len(), false);
        }
        self.bytes[at..at + b.len()].copy_from_slice(b);
        for m in &mut self.mask[at..at + b.len()] {
            *m = true;
        }
    }
    /// A NUL-terminated string; only the string and its NUL are compared.
    fn cstr(&mut self, at: usize, s: &[u8]) {
        self.raw(at, s);
        self.raw(at + s.len(), &[0]);
    }
    /// Offsets of masked bytes that differ from `other`.
    pub fn diff(&self, other: &[u8]) -> Vec<usize> {
        (0..self.bytes.len())
            .filter(|&i| self.mask[i] && other.get(i) != Some(&self.bytes[i]))
            .collect()
    }
}

/// The object image followed by each frame's itr then bdy arrays, the same order
/// `tools/harness/dump_data.py` writes.
pub fn object_image(o: &ObjectData) -> Image {
    let mut im = Image::new(OBJECT_SIZE);
    if let Some(v) = o.walking_frame_rate {
        im.i32(0x00, v);
    }
    if let Some(v) = o.walking_speed {
        im.f64(0x08, v);
    }
    if let Some(v) = o.walking_speedz {
        im.f64(0x10, v);
    }
    if let Some(v) = o.running_frame_rate {
        im.i32(0x18, v);
    }
    for (i, v) in [
        o.running_speed,
        o.running_speedz,
        o.heavy_walking_speed,
        o.heavy_walking_speedz,
        o.heavy_running_speed,
        o.heavy_running_speedz,
        o.jump_height,
        o.jump_distance,
        o.jump_distancez,
        o.dash_height,
        o.dash_distance,
        o.dash_distancez,
        o.rowing_height,
        o.rowing_distance,
    ]
    .iter()
    .enumerate()
    {
        if let Some(v) = v {
            im.f64(0x20 + 8 * i, *v);
        }
    }
    im.i32(0x90, o.weapon_hp);
    im.i32(0x94, o.weapon_drop_hurt);
    for (e, st) in o.strength.iter().enumerate() {
        itr_fields(&mut im, 0xb0 + e * 0x50, &st.itr, true);
        if !st.name.is_empty() {
            im.cstr(0x3d0 + e * 0x1e, &st.name);
        }
    }
    im.i32(0x498, o.sheet_count as i32);
    for (k, sh) in o.sheets.iter().enumerate().skip(1) {
        im.i32(0x628 + k * 4, sh.first_pic);
        // The object is not zeroed when allocated, so fields of unused sheets hold
        // whatever was in memory.
        if k > o.sheet_count {
            continue;
        }
        im.cstr(0x474 + k * 0x28, &sh.file);
        im.i32(0x650 + k * 4, sh.w);
        im.i32(0x678 + k * 4, sh.h);
        im.i32(0x6a0 + k * 4, sh.row);
        im.i32(0x6c8 + k * 4, sh.col);
    }
    im.i32(0x6f4, o.id);
    im.i32(0x6f8, o.kind);
    im.cstr(0x700, &o.head);
    if !o.small.is_empty() {
        im.cstr(0x72c, &o.small);
    }
    im.cstr(0x25324, &o.name);

    let mut tail = OBJECT_SIZE;
    for (n, f) in o.frames.iter().enumerate().take(MAX_FRAMES) {
        let b = FRAME_BASE + n * FRAME_SIZE;
        im.u8(b, f.exists as u8);
        if !f.exists {
            continue;
        }
        for (off, v) in [
            (0x04, f.pic), (0x08, f.state), (0x0c, f.wait), (0x10, f.next), (0x14, f.dvx),
            (0x18, f.dvy), (0x1c, f.dvz), (0x24, f.hit_a), (0x28, f.hit_d), (0x2c, f.hit_j),
            (0x30, f.hit_fa), (0x34, f.hit_ua), (0x38, f.hit_da), (0x3c, f.hit_fj),
            (0x40, f.hit_uj), (0x44, f.hit_dj), (0x48, f.hit_ja), (0x4c, f.mp),
            (0x50, f.centerx), (0x54, f.centery),
            (0x58, f.opoint.kind), (0x5c, f.opoint.x), (0x60, f.opoint.y), (0x64, f.opoint.action),
            (0x68, f.opoint.dvx), (0x6c, f.opoint.dvy), (0x70, f.opoint.oid), (0x74, f.opoint.facing),
            (0x80, f.bpoint.0), (0x84, f.bpoint.1),
            (0x88, f.cpoint.kind), (0x8c, f.cpoint.x), (0x90, f.cpoint.y), (0x94, f.cpoint.injury),
            (0x98, f.cpoint.cover), (0x9c, f.cpoint.vaction), (0xa0, f.cpoint.aaction),
            (0xa4, f.cpoint.jaction), (0xa8, f.cpoint.daction), (0xac, f.cpoint.throwvx),
            (0xb0, f.cpoint.throwvy), (0xb4, f.cpoint.hurtable), (0xb8, f.cpoint.decrease),
            (0xbc, f.cpoint.dircontrol), (0xc0, f.cpoint.taction),
            (0xd8, f.wpoint.kind), (0xdc, f.wpoint.x), (0xe0, f.wpoint.y), (0xe4, f.wpoint.weaponact),
            (0xe8, f.wpoint.attacking), (0xec, f.wpoint.cover), (0xf0, f.wpoint.dvx),
            (0xf4, f.wpoint.dvy), (0xf8, f.wpoint.dvz),
            (0x128, f.itrs.len() as i32), (0x12c, f.bdys.len() as i32),
            (0x138, f.itr_box.x), (0x13c, f.itr_box.y), (0x140, f.itr_box.w), (0x144, f.itr_box.h),
            (0x148, f.bdy_box.x), (0x14c, f.bdy_box.y), (0x150, f.bdy_box.w), (0x154, f.bdy_box.h),
        ] {
            im.i32(b + off, v);
        }
        if let Some(v) = f.cpoint.throwinjury {
            im.i32(b + 0xc4, v);
        }
        if let Some(v) = f.cpoint.throwvz {
            im.i32(b + 0xc8, v);
        }
        im.cstr(b + 0x15c, &f.name);
        for it in &f.itrs {
            itr_fields(&mut im, tail, it, false);
            tail += 0x50;
        }
        for bd in &f.bdys {
            for (k, v) in [bd.kind, bd.x, bd.y, bd.w, bd.h].iter().enumerate() {
                im.i32(tail + 4 * k, *v);
            }
            // the loader zeroes all ten fields of a new bdy
            for k in 5..10 {
                im.i32(tail + 4 * k, 0);
            }
            tail += 0x28;
        }
    }
    // An itr's last field (@0x4c) is never written, so pad to the full length.
    im.bytes.resize(tail, 0);
    im.mask.resize(tail, false);
    im
}

fn itr_fields(im: &mut Image, at: usize, i: &crate::object::Itr, strength: bool) {
    let mut v = vec![
        (0x14, i.dvx), (0x18, i.dvy), (0x1c, i.fall), (0x20, i.arest), (0x24, i.vrest),
        (0x28, i.respond), (0x2c, i.effect), (0x40, i.bdefend), (0x44, i.injury), (0x48, i.zwidth),
    ];
    if !strength {
        v.extend([
            (0x00, i.kind), (0x04, i.x), (0x08, i.y), (0x0c, i.w), (0x10, i.h),
            (0x30, i.catchingact[0]), (0x34, i.catchingact[1]),
            (0x38, i.caughtact[0]), (0x3c, i.caughtact[1]),
        ]);
    }
    for (off, val) in v {
        im.i32(at + off, val);
    }
}

pub fn background_image(b: &Background) -> Image {
    let mut im = Image::new(BG_SIZE);
    im.i32(0x00, b.width);
    im.i32(0x04, b.zboundary.0);
    im.i32(0x08, b.zboundary.1);
    im.i32(0x0c, b.perspective.0);
    im.i32(0x10, b.perspective.1);
    im.i32(0x1c, b.layers.len() as i32);
    if !b.shadow.is_empty() {
        im.i32(0x14, b.shadow_size.0);
        im.i32(0x18, b.shadow_size.1);
        im.cstr(0x3a4, &b.shadow);
    }
    im.cstr(0x3cc, &b.name);
    for (i, l) in b.layers.iter().enumerate() {
        im.cstr(0x20 + i * 0x1e, &l.file);
        for (off, v) in [
            (0x3ec, l.transparency), (0x464, l.width), (0x4dc, l.x), (0x554, l.y), (0x5cc, l.height),
            (0x644, l.loop_), (0x6bc, l.c1), (0x734, l.c2), (0x7ac, l.cc), (0x89c, l.rect),
        ] {
            im.i32(off + 4 * i, v);
        }
    }
    im
}

pub const STAGE_SIZE: usize = 0x149b08;
pub const PHASE_SIZE: usize = 0x34c0;
pub const SPAWN_SIZE: usize = 0xe0;

/// One stage slot (file manager `+0x7d0 + id*0x149b08`).
pub fn stage_image(st: &crate::stage::Stage) -> Image {
    use crate::stage::{Phase, Spawn, MAX_PHASES, MAX_SPAWNS};
    let mut im = Image::new(STAGE_SIZE);
    im.i32(0, st.phases.len() as i32);
    let empty = Phase::default();
    for p in 0..MAX_PHASES {
        let ph = st.phases.get(p).unwrap_or(&empty);
        let b = 8 + p * PHASE_SIZE;
        im.i32(b, ph.bound);
        im.cstr(b + 4, &ph.music);
        im.i32(b + 0x34b8, ph.when_clear_goto_phase);
        for s in 0..MAX_SPAWNS {
            let def = Spawn { x: ph.default_x, ..Default::default() };
            let sp = ph.spawns.get(s).unwrap_or(&def);
            let o = b + 0xe4 + s * SPAWN_SIZE;
            for (off, v) in [
                (0x00, sp.id), (0x04, sp.x), (0x08, sp.hp), (0x0c, sp.times), (0x10, sp.reserve),
                (0x14, sp.join), (0x18, sp.join_reserve), (0x1c, sp.act), (0x20, sp.y), (0x2c, sp.role),
            ] {
                im.i32(o + off, v);
            }
            im.f64(o + 0x24, sp.ratio);
        }
    }
    im
}
