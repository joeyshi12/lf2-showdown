//! Object data files (`data/*.dat` listed under `<object>` in `data.txt`).
//!
//! A port of `FUN_0040ef70`, the loader. The in-memory layout of the original
//! (0x25360 bytes per file) is recorded next to each field as `@offset`; frames are
//! 0x178 bytes each, 400 of them starting at `@0x7a4`.

use crate::scan::{token_checksum, Scanner};

pub const MAX_FRAMES: usize = 400;
/// The original stores up to ten sprite sheets, at indices 1..=10 (index 0 is unused).
pub const MAX_SHEETS: usize = 10;
pub const MAX_STRENGTH: usize = 10;

/// An `itr:` block, 0x50 bytes. Also the layout of a `<weapon_strength_list>` entry.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Itr {
    pub kind: i32,       // @0x00
    pub x: i32,          // @0x04
    pub y: i32,          // @0x08
    pub w: i32,          // @0x0c
    pub h: i32,          // @0x10
    pub dvx: i32,        // @0x14
    pub dvy: i32,        // @0x18
    pub fall: i32,       // @0x1c
    pub arest: i32,      // @0x20
    pub vrest: i32,      // @0x24
    pub respond: i32,    // @0x28
    pub effect: i32,     // @0x2c
    pub catchingact: [i32; 2], // @0x30; `pickingact:` writes [0]
    pub caughtact: [i32; 2],   // @0x38; `pickedact:` writes catchingact[1] (@0x34)
    pub bdefend: i32,    // @0x40
    pub injury: i32,     // @0x44
    pub zwidth: i32,     // @0x48
    pub unknown_4c: i32, // @0x4c, never written by the loader
}

/// A `bdy:` block, 0x28 bytes. Only the first five fields are written by the loader.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Bdy {
    pub kind: i32, // @0x00
    pub x: i32,    // @0x04
    pub y: i32,    // @0x08
    pub w: i32,    // @0x0c
    pub h: i32,    // @0x10
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Opoint {
    pub kind: i32,   // @0x58
    pub x: i32,      // @0x5c
    pub y: i32,      // @0x60
    pub action: i32, // @0x64
    pub dvx: i32,    // @0x68
    pub dvy: i32,    // @0x6c
    pub oid: i32,    // @0x70
    pub facing: i32, // @0x74
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Cpoint {
    pub kind: i32,        // @0x88
    pub x: i32,           // @0x8c
    pub y: i32,           // @0x90
    /// `injury:` and `fronthurtact:` share this field.
    pub injury: i32,      // @0x94
    /// `cover:` and `backhurtact:` share this field.
    pub cover: i32,       // @0x98
    pub vaction: i32,     // @0x9c
    pub aaction: i32,     // @0xa0
    pub jaction: i32,     // @0xa4
    pub daction: i32,     // @0xa8
    pub throwvx: i32,     // @0xac
    pub throwvy: i32,     // @0xb0
    pub hurtable: i32,    // @0xb4
    pub decrease: i32,    // @0xb8
    pub dircontrol: i32,  // @0xbc
    pub taction: i32,     // @0xc0
    /// `throwinjury` and `throwvz` are not cleared when a frame is initialised
    /// (`FUN_0040bbf0` skips @0xc4..@0xd8), so when a cpoint leaves them out the
    /// original holds whatever was in the heap there. `None` records that.
    pub throwinjury: Option<i32>, // @0xc4
    pub throwvz: Option<i32>,     // @0xc8
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Wpoint {
    pub kind: i32,      // @0xd8
    pub x: i32,         // @0xdc
    pub y: i32,         // @0xe0
    pub weaponact: i32, // @0xe4
    pub attacking: i32, // @0xe8
    pub cover: i32,     // @0xec
    pub dvx: i32,       // @0xf0
    pub dvy: i32,       // @0xf4
    pub dvz: i32,       // @0xf8
}

/// A bounding box over a frame's itrs or bdys, computed by the loader: `x`,`y` are the
/// minimum corner, `w`,`h` the extent.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BoundBox {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

/// One frame, 0x178 bytes in the original. Offsets are from the frame start.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Frame {
    pub exists: bool,  // @0x00
    pub pic: i32,      // @0x04
    pub state: i32,    // @0x08
    pub wait: i32,     // @0x0c
    pub next: i32,     // @0x10
    pub dvx: i32,      // @0x14
    pub dvy: i32,      // @0x18
    pub dvz: i32,      // @0x1c
    pub hit_a: i32,    // @0x24
    pub hit_d: i32,    // @0x28
    pub hit_j: i32,    // @0x2c
    pub hit_fa: i32,   // @0x30
    pub hit_ua: i32,   // @0x34
    pub hit_da: i32,   // @0x38
    pub hit_fj: i32,   // @0x3c
    pub hit_uj: i32,   // @0x40
    pub hit_dj: i32,   // @0x44
    pub hit_ja: i32,   // @0x48
    pub mp: i32,       // @0x4c
    pub centerx: i32,  // @0x50
    pub centery: i32,  // @0x54
    pub opoint: Opoint,
    pub bpoint: (i32, i32), // @0x80, @0x84
    pub cpoint: Cpoint,
    pub wpoint: Wpoint,
    pub itrs: Vec<Itr>,      // count @0x128, pointer @0x130
    pub bdys: Vec<Bdy>,      // count @0x12c, pointer @0x134
    pub itr_box: BoundBox,   // @0x138
    pub bdy_box: BoundBox,   // @0x148
    pub name: Vec<u8>,       // @0x15c
    pub sound: Option<Vec<u8>>, // @0x170 (pointer), id @0x174 in the global sound table
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Sheet {
    pub file: Vec<u8>,   // @0x474 + k*0x28
    pub first_pic: i32,  // @0x628 + k*4
    pub w: i32,          // @0x650 + k*4
    pub h: i32,          // @0x678 + k*4
    pub row: i32,        // @0x6a0 + k*4
    pub col: i32,        // @0x6c8 + k*4
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Strength {
    pub itr: Itr,       // @0xb0 + e*0x50
    pub name: Vec<u8>,  // @0x3d0 + e*0x1e
}

/// The object file, 0x25360 bytes in the original. The object is allocated with
/// `operator new` and not cleared, so header fields a file leaves out (most weapons and
/// effects leave out all the movement values) hold leftover heap bytes; `None` records
/// that.
#[derive(Clone, Debug, PartialEq)]
pub struct ObjectData {
    pub id: i32,                   // @0x6f4
    pub kind: i32,                 // @0x6f8, the `type:` from data.txt
    pub walking_frame_rate: Option<i32>,   // @0x00
    pub walking_speed: Option<f64>,        // @0x08
    pub walking_speedz: Option<f64>,       // @0x10
    pub running_frame_rate: Option<i32>,   // @0x18
    pub running_speed: Option<f64>,        // @0x20
    pub running_speedz: Option<f64>,       // @0x28
    pub heavy_walking_speed: Option<f64>,  // @0x30
    pub heavy_walking_speedz: Option<f64>, // @0x38
    pub heavy_running_speed: Option<f64>,  // @0x40
    pub heavy_running_speedz: Option<f64>, // @0x48
    pub jump_height: Option<f64>,          // @0x50
    pub jump_distance: Option<f64>,        // @0x58
    pub jump_distancez: Option<f64>,       // @0x60
    pub dash_height: Option<f64>,          // @0x68
    pub dash_distance: Option<f64>,        // @0x70
    pub dash_distancez: Option<f64>,       // @0x78
    pub rowing_height: Option<f64>,        // @0x80
    pub rowing_distance: Option<f64>,      // @0x88
    pub weapon_hp: i32,            // @0x90
    pub weapon_drop_hurt: i32,     // @0x94
    pub weapon_hit_sound: Option<Vec<u8>>,    // @0x98, id @0xa4
    pub weapon_drop_sound: Option<Vec<u8>>,   // @0x9c, id @0xa8
    pub weapon_broken_sound: Option<Vec<u8>>, // @0xa0, id @0xac
    pub strength: Vec<Strength>,   // MAX_STRENGTH entries
    /// `sheets[0]` is unused, as in the original; `sheet_count` is `@0x498`.
    pub sheets: Vec<Sheet>,
    pub sheet_count: usize,
    pub head: Vec<u8>,             // @0x700
    pub small: Vec<u8>,            // @0x72c
    pub name: Vec<u8>,             // @0x25324
    pub frames: Vec<Frame>,        // MAX_FRAMES entries
}

impl ObjectData {
    pub fn new(id: i32, kind: i32) -> Self {
        let mut sheets = vec![Sheet::default(); MAX_SHEETS + 1];
        for s in sheets.iter_mut().skip(1) {
            s.first_pic = 3000;
        }
        ObjectData {
            id,
            kind,
            walking_frame_rate: None,
            walking_speed: None,
            walking_speedz: None,
            running_frame_rate: None,
            running_speed: None,
            running_speedz: None,
            heavy_walking_speed: None,
            heavy_walking_speedz: None,
            heavy_running_speed: None,
            heavy_running_speedz: None,
            jump_height: None,
            jump_distance: None,
            jump_distancez: None,
            dash_height: None,
            dash_distance: None,
            dash_distancez: None,
            rowing_height: None,
            rowing_distance: None,
            weapon_hp: 0,
            weapon_drop_hurt: 0,
            weapon_hit_sound: None,
            weapon_drop_sound: None,
            weapon_broken_sound: None,
            strength: vec![Strength::default(); MAX_STRENGTH],
            sheets,
            sheet_count: 0,
            head: b"none".to_vec(),
            small: Vec::new(),
            name: b"none".to_vec(),
            frames: vec![Frame::default(); MAX_FRAMES],
        }
    }
}

/// Everything the loader reports besides the object itself.
#[derive(Default)]
pub struct LoadInfo {
    /// Amount this file adds to the data checksum `DAT_0044f620`.
    pub checksum: i32,
    /// Sound file names in the order this file first refers to them.
    pub sounds: Vec<Vec<u8>>,
}

fn eq(t: &[u8], s: &str) -> bool {
    t == s.as_bytes()
}

fn finish_sheet(o: &mut ObjectData) {
    // At `file`/`<bmp_end>`: first_pic of the current sheet is the sum of row*col of
    // sheets 1..count-1. The loop reads row[1+i]*col[1+i] for i in 0..count-1.
    let n = o.sheet_count;
    if n >= o.sheets.len() {
        return;
    }
    let mut sum = 0i32;
    if n >= 2 {
        for i in 0..n - 1 {
            sum = sum.wrapping_add(o.sheets[1 + i].row.wrapping_mul(o.sheets[1 + i].col));
        }
    }
    o.sheets[n].first_pic = sum;
}

fn bound_box<T>(items: &[T], f: impl Fn(&T) -> (i32, i32, i32, i32)) -> BoundBox {
    let (x, y, w, h) = f(&items[0]);
    let (mut x0, mut y0, mut x1, mut y1) = (x, y, x + w, y + h);
    for it in &items[1..] {
        let (x, y, w, h) = f(it);
        x0 = x0.min(x);
        y0 = y0.min(y);
        x1 = x1.max(x + w);
        y1 = y1.max(y + h);
    }
    BoundBox { x: x0, y: y0, w: x1 - x0, h: y1 - y0 }
}

/// Parse decoded object text. `id` and `kind` come from `data.txt`.
pub fn parse_object(text: &[u8], id: i32, kind: i32) -> (ObjectData, LoadInfo) {
    let mut o = ObjectData::new(id, kind);
    let mut info = LoadInfo::default();
    let mut s = Scanner::new(text);
    let mut tok: Vec<u8> = Vec::new();
    // `iStack_4e4`: a failed `%d` after `<frame>` keeps the previous frame number.
    let mut frame_no: i32 = 0;
    let note_sound = |info: &mut LoadInfo, name: &[u8]| {
        if !info.sounds.iter().any(|n| n == name) {
            info.sounds.push(name.to_vec());
        }
    };

    while !s.eof() {
        s.token(&mut tok);
        info.checksum = info.checksum.wrapping_add(token_checksum(&tok));

        if eq(&tok, "<bmp_begin>") {
            loop {
                s.token(&mut tok);
                if eq(&tok, "<bmp_end>") {
                    break;
                }
                if tok.len() >= 4 && &tok[..4] == b"file" {
                    if o.sheet_count > 0 {
                        finish_sheet(&mut o);
                    }
                    o.sheet_count += 1;
                    let mut name = Vec::new();
                    s.token(&mut name);
                    if o.sheet_count < o.sheets.len() {
                        o.sheets[o.sheet_count].file = name;
                    }
                }
                let k = o.sheet_count.min(o.sheets.len() - 1);
                if eq(&tok, "head:") {
                    s.token(&mut o.head);
                }
                if eq(&tok, "small:") {
                    s.token(&mut o.small);
                }
                macro_rules! num {
                    ($key:literal, $field:expr, int) => {
                        if eq(&tok, $key) {
                            if let Some(v) = s.int() {
                                $field = v;
                            }
                        }
                    };
                    ($key:literal, $field:expr, float) => {
                        if eq(&tok, $key) {
                            if let Some(v) = s.float() {
                                $field = Some(v);
                            }
                        }
                    };
                    ($key:literal, $field:expr, opt_int) => {
                        if eq(&tok, $key) {
                            if let Some(v) = s.int() {
                                $field = Some(v);
                            }
                        }
                    };
                }
                num!("walking_frame_rate", o.walking_frame_rate, opt_int);
                num!("walking_speed", o.walking_speed, float);
                num!("walking_speedz", o.walking_speedz, float);
                num!("running_frame_rate", o.running_frame_rate, opt_int);
                num!("running_speed", o.running_speed, float);
                num!("running_speedz", o.running_speedz, float);
                num!("heavy_walking_speed", o.heavy_walking_speed, float);
                num!("heavy_walking_speedz", o.heavy_walking_speedz, float);
                num!("heavy_running_speed", o.heavy_running_speed, float);
                num!("heavy_running_speedz", o.heavy_running_speedz, float);
                num!("jump_height", o.jump_height, float);
                num!("jump_distance", o.jump_distance, float);
                num!("jump_distancez", o.jump_distancez, float);
                num!("dash_height", o.dash_height, float);
                num!("dash_distance", o.dash_distance, float);
                num!("dash_distancez", o.dash_distancez, float);
                num!("rowing_height", o.rowing_height, float);
                num!("rowing_distance", o.rowing_distance, float);
                if eq(&tok, "name:") {
                    s.token(&mut o.name);
                }
                num!("w:", o.sheets[k].w, int);
                num!("h:", o.sheets[k].h, int);
                num!("row:", o.sheets[k].row, int);
                num!("col:", o.sheets[k].col, int);
                num!("weapon_hp:", o.weapon_hp, int);
                num!("weapon_drop_hurt:", o.weapon_drop_hurt, int);
                for (key, slot) in [
                    ("weapon_hit_sound:", &mut o.weapon_hit_sound),
                    ("weapon_drop_sound:", &mut o.weapon_drop_sound),
                    ("weapon_broken_sound:", &mut o.weapon_broken_sound),
                ] {
                    if eq(&tok, key) {
                        let mut name = Vec::new();
                        s.token(&mut name);
                        note_sound(&mut info, &name);
                        *slot = Some(name);
                    }
                }
                if s.eof() {
                    break;
                }
            }
            finish_sheet(&mut o);
        }

        if eq(&tok, "<weapon_strength_list>") {
            let mut entry: usize = 0;
            s.token(&mut tok);
            while !eq(&tok, "<weapon_strength_list_end>") {
                if eq(&tok, "entry:") {
                    // `fscanf("%d %s", &entry, base + entry*0x1e + 0x3d0)`: the name's
                    // address is computed from the entry number before this read.
                    let prev = entry;
                    if let Some(v) = s.int() {
                        entry = v as usize;
                    }
                    let mut name = Vec::new();
                    s.token(&mut name);
                    if prev < o.strength.len() {
                        o.strength[prev].name = name;
                    }
                } else if entry < o.strength.len() {
                    let e = &mut o.strength[entry].itr;
                    let field = match tok.as_slice() {
                        b"dvx:" => Some(&mut e.dvx),
                        b"dvy:" => Some(&mut e.dvy),
                        b"fall:" => Some(&mut e.fall),
                        b"arest:" => Some(&mut e.arest),
                        b"vrest:" => Some(&mut e.vrest),
                        b"respond:" => Some(&mut e.respond),
                        b"bdefend:" => Some(&mut e.bdefend),
                        b"injury:" => Some(&mut e.injury),
                        b"zwidth:" => Some(&mut e.zwidth),
                        b"effect:" => Some(&mut e.effect),
                        _ => None,
                    };
                    if let Some(f) = field {
                        if let Some(v) = s.int() {
                            *f = v;
                        }
                    }
                }
                s.token(&mut tok);
                if s.eof() {
                    break;
                }
            }
        }

        if eq(&tok, "<frame>") {
            if let Some(v) = s.int() {
                frame_no = v;
            }
            let mut name = Vec::new();
            s.token(&mut name);
            let idx = frame_no as usize;
            let mut scratch = Frame::default();
            let f: &mut Frame = if idx < MAX_FRAMES { &mut o.frames[idx] } else { &mut scratch };
            f.name = name;
            f.itrs.clear();
            f.bdys.clear();
            f.exists = true;
            parse_frame_body(&mut s, &mut tok, f, &mut info.sounds);
            if !f.itrs.is_empty() {
                f.itr_box = bound_box(&f.itrs, |i| (i.x, i.y, i.w, i.h));
            }
            if !f.bdys.is_empty() {
                f.bdy_box = bound_box(&f.bdys, |b| (b.x, b.y, b.w, b.h));
            }
        }
    }
    (o, info)
}

fn read_opt(s: &mut Scanner, into: &mut Option<i32>) {
    if let Some(v) = s.int() {
        *into = Some(v);
    }
}

fn read_int(s: &mut Scanner, into: &mut i32) {
    if let Some(v) = s.int() {
        *into = v;
    }
}

fn parse_frame_body(s: &mut Scanner, tok: &mut Vec<u8>, f: &mut Frame, sounds: &mut Vec<Vec<u8>>) {
    s.token(tok);
    while !eq(tok, "<frame_end>") {
        match tok.as_slice() {
            b"pic:" => read_int(s, &mut f.pic),
            b"state:" => read_int(s, &mut f.state),
            b"wait:" => read_int(s, &mut f.wait),
            b"next:" => read_int(s, &mut f.next),
            b"dvx:" => read_int(s, &mut f.dvx),
            b"dvy:" => read_int(s, &mut f.dvy),
            b"dvz:" => read_int(s, &mut f.dvz),
            b"centerx:" => read_int(s, &mut f.centerx),
            b"centery:" => read_int(s, &mut f.centery),
            b"hit_a:" => read_int(s, &mut f.hit_a),
            b"hit_d:" => read_int(s, &mut f.hit_d),
            b"hit_j:" => read_int(s, &mut f.hit_j),
            b"hit_Fa:" => read_int(s, &mut f.hit_fa),
            b"hit_Ua:" => read_int(s, &mut f.hit_ua),
            b"hit_Da:" => read_int(s, &mut f.hit_da),
            b"hit_Fj:" => read_int(s, &mut f.hit_fj),
            b"hit_Uj:" => read_int(s, &mut f.hit_uj),
            b"hit_Dj:" => read_int(s, &mut f.hit_dj),
            b"hit_ja:" => read_int(s, &mut f.hit_ja),
            b"mp:" => read_int(s, &mut f.mp),
            b"sound:" => {
                let mut name = Vec::new();
                s.token(&mut name);
                if !sounds.iter().any(|n| *n == name) {
                    sounds.push(name.clone());
                }
                f.sound = Some(name);
            }
            b"opoint:" => {
                let p = &mut f.opoint;
                block(s, tok, "opoint_end:", |s, t| match t {
                    b"kind:" => read_int(s, &mut p.kind),
                    b"x:" => read_int(s, &mut p.x),
                    b"y:" => read_int(s, &mut p.y),
                    b"action:" => read_int(s, &mut p.action),
                    b"dvx:" => read_int(s, &mut p.dvx),
                    b"dvy:" => read_int(s, &mut p.dvy),
                    b"oid:" => read_int(s, &mut p.oid),
                    b"facing:" => read_int(s, &mut p.facing),
                    _ => {}
                });
            }
            b"bpoint:" => {
                let p = &mut f.bpoint;
                block(s, tok, "bpoint_end:", |s, t| match t {
                    b"x:" => read_int(s, &mut p.0),
                    b"y:" => read_int(s, &mut p.1),
                    _ => {}
                });
            }
            b"cpoint:" => {
                let p = &mut f.cpoint;
                block(s, tok, "cpoint_end:", |s, t| match t {
                    b"kind:" => read_int(s, &mut p.kind),
                    b"x:" => read_int(s, &mut p.x),
                    b"y:" => read_int(s, &mut p.y),
                    b"injury:" | b"fronthurtact:" => read_int(s, &mut p.injury),
                    b"cover:" | b"backhurtact:" => read_int(s, &mut p.cover),
                    b"vaction:" => read_int(s, &mut p.vaction),
                    b"aaction:" => read_int(s, &mut p.aaction),
                    b"jaction:" => read_int(s, &mut p.jaction),
                    b"taction:" => read_int(s, &mut p.taction),
                    b"daction:" => read_int(s, &mut p.daction),
                    b"throwvx:" => read_int(s, &mut p.throwvx),
                    b"throwvy:" => read_int(s, &mut p.throwvy),
                    b"throwvz:" => read_opt(s, &mut p.throwvz),
                    b"hurtable:" => read_int(s, &mut p.hurtable),
                    b"throwinjury:" => read_opt(s, &mut p.throwinjury),
                    b"decrease:" => read_int(s, &mut p.decrease),
                    b"dircontrol:" => read_int(s, &mut p.dircontrol),
                    _ => {}
                });
            }
            b"wpoint:" => {
                let p = &mut f.wpoint;
                block(s, tok, "wpoint_end:", |s, t| match t {
                    b"kind:" => read_int(s, &mut p.kind),
                    b"x:" => read_int(s, &mut p.x),
                    b"y:" => read_int(s, &mut p.y),
                    b"weaponact:" => read_int(s, &mut p.weaponact),
                    b"attacking:" => read_int(s, &mut p.attacking),
                    b"cover:" => read_int(s, &mut p.cover),
                    b"dvx:" => read_int(s, &mut p.dvx),
                    b"dvy:" => read_int(s, &mut p.dvy),
                    b"dvz:" => read_int(s, &mut p.dvz),
                    _ => {}
                });
            }
            b"itr:" => {
                f.itrs.push(Itr::default());
                let p = f.itrs.last_mut().unwrap();
                block(s, tok, "itr_end:", |s, t| match t {
                    b"kind:" => read_int(s, &mut p.kind),
                    b"x:" => read_int(s, &mut p.x),
                    b"y:" => read_int(s, &mut p.y),
                    b"w:" => read_int(s, &mut p.w),
                    b"h:" => read_int(s, &mut p.h),
                    b"dvx:" => read_int(s, &mut p.dvx),
                    b"dvy:" => read_int(s, &mut p.dvy),
                    b"fall:" => read_int(s, &mut p.fall),
                    b"arest:" => read_int(s, &mut p.arest),
                    b"vrest:" => read_int(s, &mut p.vrest),
                    b"respond:" => read_int(s, &mut p.respond),
                    b"bdefend:" => read_int(s, &mut p.bdefend),
                    b"injury:" => read_int(s, &mut p.injury),
                    b"zwidth:" => read_int(s, &mut p.zwidth),
                    b"effect:" => read_int(s, &mut p.effect),
                    b"catchingact:" => {
                        read_int(s, &mut p.catchingact[0]);
                        read_int(s, &mut p.catchingact[1]);
                    }
                    b"caughtact:" => {
                        read_int(s, &mut p.caughtact[0]);
                        read_int(s, &mut p.caughtact[1]);
                    }
                    b"pickingact:" => read_int(s, &mut p.catchingact[0]),
                    b"pickedact:" => read_int(s, &mut p.catchingact[1]),
                    _ => {}
                });
            }
            b"bdy:" => {
                f.bdys.push(Bdy::default());
                let p = f.bdys.last_mut().unwrap();
                block(s, tok, "bdy_end:", |s, t| match t {
                    b"kind:" => read_int(s, &mut p.kind),
                    b"x:" => read_int(s, &mut p.x),
                    b"y:" => read_int(s, &mut p.y),
                    b"w:" => read_int(s, &mut p.w),
                    b"h:" => read_int(s, &mut p.h),
                    _ => {}
                });
            }
            _ => {}
        }
        s.token(tok);
        if s.eof() {
            break;
        }
    }
}

/// The loop shape every sub-block uses: read a token, stop at `end`, otherwise handle the
/// key and read the next token.
fn block(s: &mut Scanner, tok: &mut Vec<u8>, end: &str, mut f: impl FnMut(&mut Scanner, &[u8])) {
    s.token(tok);
    while !eq(tok, end) {
        let t = tok.clone();
        f(s, &t);
        s.token(tok);
        if s.eof() {
            break;
        }
    }
}
