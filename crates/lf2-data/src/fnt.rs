//! Windows bitmap fonts (.fon: an NE file holding FNT resources), for drawing text
//! the way GDI's TextOutA does with the System font.

/// One FNT font: glyph bitmaps for the characters first..=last.
#[derive(Clone, Debug)]
pub struct BitmapFont {
    pub height: i32,
    pub ascent: i32,
    first: u8,
    last: u8,
    default: u8,
    /// Per character: width and rows of bits (bit 7 of byte 0 is the leftmost pixel).
    glyphs: Vec<(i32, Vec<Vec<u8>>)>,
}

fn u16le(d: &[u8], o: usize) -> Option<usize> {
    Some(u16::from_le_bytes(d.get(o..o + 2)?.try_into().ok()?) as usize)
}
fn u32le(d: &[u8], o: usize) -> Option<usize> {
    Some(u32::from_le_bytes(d.get(o..o + 4)?.try_into().ok()?) as usize)
}

impl BitmapFont {
    /// The first FNT resource of a .fon file.
    pub fn from_fon(d: &[u8]) -> Option<BitmapFont> {
        let ne = u32le(d, 0x3c)?;
        if d.get(ne..ne + 2)? != b"NE" {
            return None;
        }
        let rt = ne + u16le(d, ne + 0x24)?;
        let shift = u16le(d, rt)?;
        let mut p = rt + 2;
        loop {
            let ty = u16le(d, p)?;
            if ty == 0 {
                return None;
            }
            let n = u16le(d, p + 2)?;
            p += 8;
            for _ in 0..n {
                let (off, len) = (u16le(d, p)? << shift, u16le(d, p + 2)? << shift);
                p += 12;
                if ty == 0x8008 {
                    return Self::from_fnt(d.get(off..off + len)?);
                }
            }
        }
    }

    /// An FNT resource (versions 2 and 3).
    pub fn from_fnt(f: &[u8]) -> Option<BitmapFont> {
        let version = u16le(f, 0)?;
        let ascent = u16le(f, 0x4a)? as i32;
        let height = u16le(f, 0x58)? as i32;
        let (first, last, default) = (*f.get(0x5f)?, *f.get(0x60)?, *f.get(0x61)?);
        let (table, entry) = if version == 0x300 { (148, 6) } else { (118, 4) };
        let mut glyphs = Vec::new();
        for i in 0..=(last as usize - first as usize) {
            let e = table + entry * i;
            let w = u16le(f, e)? as i32;
            let off = if version == 0x300 { u32le(f, e + 2)? } else { u16le(f, e + 2)? };
            let cols = ((w + 7) / 8) as usize;
            let mut rows = vec![vec![0u8; cols]; height as usize];
            for c in 0..cols {
                for (r, row) in rows.iter_mut().enumerate() {
                    row[c] = *f.get(off + c * height as usize + r)?;
                }
            }
            glyphs.push((w, rows));
        }
        Some(BitmapFont { height, ascent, first, last, default, glyphs })
    }

    fn glyph(&self, c: u8) -> &(i32, Vec<Vec<u8>>) {
        let i = if (self.first..=self.last).contains(&c) { c - self.first } else { self.default };
        &self.glyphs[(i as usize).min(self.glyphs.len() - 1)]
    }

    /// Width of a string in pixels.
    pub fn width(&self, text: &[u8]) -> i32 {
        text.iter().map(|&c| self.glyph(c).0).sum()
    }

    /// Call `pixel(x, y, on)` for every pixel of every character cell of `text`
    /// drawn with its top left corner at (x, y): `on` for glyph pixels, else the
    /// background (TextOutA in opaque mode fills the whole cell).
    pub fn draw(&self, text: &[u8], x: i32, y: i32, mut pixel: impl FnMut(i32, i32, bool)) {
        let mut cx = x;
        for &c in text {
            let (w, rows) = self.glyph(c);
            for (r, row) in rows.iter().enumerate() {
                for px in 0..*w {
                    let on = row[(px / 8) as usize] & (0x80 >> (px % 8)) != 0;
                    pixel(cx + px, y + r as i32, on);
                }
            }
            cx += w;
        }
    }
}
