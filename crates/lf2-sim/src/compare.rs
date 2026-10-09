//! Differences between the simulation and a snapshot of the original.

use crate::fields::{GLOBAL_NAMES, OBJECT_FIELDS};
use crate::trace::Snapshot;
use crate::world::World;

/// Global byte ranges that are not simulation state: the key array (input), the
/// harness's own counters, and the frame counter in WinMain.
pub const IGNORED_GLOBALS: &[(u32, u32)] = &[
    (0x455378, 0x455478), // key array
    (0x459f00, 0x459f08), // lockstep patch counters
    (0x459f0c, 0x459f14), // x87 control word and fixed clock of the harness patch
    (0x458580, 0x458584), // WinMain loop counter
    (0x451154, 0x451158), // timeGetTime() when a match starts
    (0x44fd98, 0x44fdc0), // the .lfr file name, from the local time
];

/// Globals that hold heap pointers: only whether they are null is compared.
pub const POINTER_GLOBALS: &[u32] = &[
    0x4511ac, // menu background surface (MENU_BACKn)
    0x44f040, 0x44f044, 0x44f048, 0x44f04c, // DirectShow interfaces of the music
    0x4588a8, // the recording being written
    0x4588ac, // the recording being played
    0x451b38, 0x451b3c, 0x451b40, 0x451b44, 0x451b48, 0x451b4c, 0x451b50, 0x451b54, 0x451b58,
    0x451b5c, 0x451b60, // data of the battle troop kinds
    0x451bac, 0x451bb0, // the BATTLETROOPS and BATTLEMODE surfaces
];

#[derive(Debug, Clone)]
pub struct Diff {
    pub what: String,
    pub ours: String,
    pub theirs: String,
}

fn field_name(off: u32) -> String {
    for &(o, ty, name) in OBJECT_FIELDS {
        let len = match ty {
            "double" => 8,
            "char" | "uchar" | "byte" => 1,
            _ => 4,
        };
        if off >= o && off < o + len {
            return if off == o { name.to_string() } else { format!("{}+{}", name, off - o) };
        }
    }
    format!("@{:#x}", off)
}

fn global_name(a: u32) -> String {
    for &(g, name) in GLOBAL_NAMES {
        if a >= g && a < g + 4 {
            return if a == g { name.to_string() } else { format!("{}+{}", name, a - g) };
        }
    }
    format!("DAT_{:08x}", a)
}

/// Compare objects (only those the original has) and globals, by 4-byte word.
pub fn compare(w: &World, s: &Snapshot) -> Vec<Diff> {
    let mut out = Vec::new();
    let base = crate::fields::GLOBALS_BASE;
    let ours = &w.g.m.bytes;
    let theirs = &s.globals.m.bytes;
    let mut a = 0usize;
    while a < ours.len() {
        if ours[a..a + 4] == theirs[a..a + 4] {
            a += 4;
            continue;
        }
        let addr = base + a as u32;
        let ignored = IGNORED_GLOBALS.iter().any(|&(lo, hi)| addr >= lo && addr < hi)
            || (POINTER_GLOBALS.contains(&addr) && (ours[a..a + 4] == [0; 4]) == (theirs[a..a + 4] == [0; 4]));
        if !ignored && ours[a..a + 4] != theirs[a..a + 4] {
            out.push(Diff {
                what: global_name(addr),
                ours: format!("{}", i32::from_le_bytes(ours[a..a + 4].try_into().unwrap())),
                theirs: format!("{}", i32::from_le_bytes(theirs[a..a + 4].try_into().unwrap())),
            });
        }
        a += 4;
    }
    for (i, so) in s.objects.iter().enumerate() {
        let Some(so) = so else { continue };
        let (ob, tb) = (&w.objs[i].m.bytes, &so.m.bytes);
        if ob == tb {
            continue;
        }
        let mut off = 0usize;
        while off < ob.len() {
            let is_f64 = OBJECT_FIELDS.iter().any(|&(o, ty, _)| o as usize == off && ty == "double");
            let n = if is_f64 { 8 } else { 4 };
            if ob[off..off + n] != tb[off..off + n] {
                let fmt = |b: &[u8]| {
                    if is_f64 {
                        format!("{:?}", f64::from_le_bytes(b.try_into().unwrap()))
                    } else {
                        format!("{} ({:02x?})", i32::from_le_bytes(b.try_into().unwrap()), b)
                    }
                };
                let name = field_name(off as u32);
                out.push(Diff { what: format!("obj[{}].{}", i, name), ours: fmt(&ob[off..off + n]), theirs: fmt(&tb[off..off + n]) });
            }
            off += n;
        }
    }
    out
}
