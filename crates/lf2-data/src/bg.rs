//! Backgrounds (`bg/**/bg.dat`), a port of `FUN_0040c160`.
//!
//! The original keeps backgrounds in the file manager at `+0x4d45db0 + n*0x990`; the
//! offsets below are relative to that. Per-layer fields are arrays of 30.

use crate::scan::{token_checksum, Scanner};

pub const MAX_LAYERS: usize = 30;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Layer {
    pub file: Vec<u8>,     // @0x20 + i*0x1e
    pub transparency: i32, // @0x3ec
    pub width: i32,        // @0x464
    pub x: i32,            // @0x4dc
    pub y: i32,            // @0x554
    pub height: i32,       // @0x5cc
    pub loop_: i32,        // @0x644
    pub c1: i32,           // @0x6bc
    pub c2: i32,           // @0x734
    pub cc: i32,           // @0x7ac
    /// `rect:` (a 16-bit RGB565 value, widened by the loader) or `rect32:`.
    pub rect: i32,         // @0x89c
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Background {
    pub id: i32,
    pub name: Vec<u8>,        // @0x3cc, '_' replaced by ' ', at most 29 bytes
    pub width: i32,           // @0x00
    pub zboundary: (i32, i32),   // @0x04, @0x08
    pub perspective: (i32, i32), // @0x0c, @0x10
    pub shadow: Vec<u8>,      // @0x3a4
    pub shadow_size: (i32, i32), // @0x14, @0x18
    pub layers: Vec<Layer>,   // count @0x1c
}

/// `rect:` widening: RGB565 to 0x00RRGGBB with the low three bits of each channel set.
pub fn widen_rgb565(v: i32) -> i32 {
    let v = v as u32;
    let r = (v >> 11) & 0x1f;
    let g = (v >> 5) & 0x3f;
    let b = v & 0x1f;
    ((((r * 0x200 + g) * 0x80 + b) * 8).wrapping_add(0x70707)) as i32
}

fn int(s: &mut Scanner, into: &mut i32) {
    if let Some(v) = s.int() {
        *into = v;
    }
}

pub fn parse_background(text: &[u8], id: i32) -> (Background, i32) {
    let mut bg = Background { id, ..Default::default() };
    let mut sum = 0i32;
    let mut s = Scanner::new(text);
    let mut tok = Vec::new();
    while !s.eof() {
        s.token(&mut tok);
        sum = sum.wrapping_add(token_checksum(&tok));
        match tok.as_slice() {
            b"name:" => {
                let mut n = Vec::new();
                s.token(&mut n);
                n.truncate(29);
                for c in n.iter_mut() {
                    if *c == b'_' {
                        *c = b' ';
                    }
                }
                bg.name = n;
            }
            b"width:" => int(&mut s, &mut bg.width),
            b"zboundary:" => {
                int(&mut s, &mut bg.zboundary.0);
                int(&mut s, &mut bg.zboundary.1);
            }
            b"perspective:" => {
                int(&mut s, &mut bg.perspective.0);
                int(&mut s, &mut bg.perspective.1);
            }
            b"shadow:" => {
                s.token(&mut bg.shadow);
                let mut ignored = Vec::new();
                s.token(&mut ignored);
                int(&mut s, &mut bg.shadow_size.0);
                int(&mut s, &mut bg.shadow_size.1);
            }
            b"layer:" => {
                let mut l = Layer::default();
                s.token(&mut l.file);
                s.token(&mut tok);
                // The end marker is compared over 10 bytes, so it has no colon.
                while tok != b"layer_end" {
                    match tok.as_slice() {
                        b"transparency:" => int(&mut s, &mut l.transparency),
                        b"width:" => int(&mut s, &mut l.width),
                        b"x:" => int(&mut s, &mut l.x),
                        b"y:" => int(&mut s, &mut l.y),
                        b"height:" => int(&mut s, &mut l.height),
                        b"rect:" => {
                            int(&mut s, &mut l.rect);
                            l.rect = widen_rgb565(l.rect);
                        }
                        b"rect32:" => int(&mut s, &mut l.rect),
                        b"loop:" => int(&mut s, &mut l.loop_),
                        b"cc:" => int(&mut s, &mut l.cc),
                        b"c1:" => int(&mut s, &mut l.c1),
                        b"c2:" => int(&mut s, &mut l.c2),
                        _ => {}
                    }
                    s.token(&mut tok);
                    if s.eof() {
                        break;
                    }
                }
                if bg.layers.len() < MAX_LAYERS {
                    bg.layers.push(l);
                }
            }
            _ => {}
        }
    }
    (bg, sum)
}

/// Background 99, "Lee On Road", which the loader (0x4122f0) builds in: width 2400, z
/// 350..470, shadow from the exe's `shadow1` resource. It has no layers; the match
/// draws it with `draw_background_layer` (0x41a050) from back99_1..3.
pub fn lee_on_road() -> &'static Background {
    static B: std::sync::OnceLock<Background> = std::sync::OnceLock::new();
    B.get_or_init(|| Background {
        id: -1,
        name: b"Lee On Road".to_vec(),
        width: 0x960,
        zboundary: (0x15e, 0x1d6),
        perspective: (0, 0),
        shadow: b"shadow1".to_vec(),
        shadow_size: (0x25, 9),
        layers: Vec::new(),
    })
}

/// An unused background slot (all zeros), such as index 100 ("Random" only has a name).
pub fn empty_background() -> &'static Background {
    static B: std::sync::OnceLock<Background> = std::sync::OnceLock::new();
    B.get_or_init(Background::default)
}
