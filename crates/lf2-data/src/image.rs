//! BMP and PE bitmap-resource decoding.
/// An image as 0x00RRGGBB pixels.
#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    pub w: usize,
    pub h: usize,
    pub px: Vec<u32>,
}

fn u16le(b: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(o..o + 2)?.try_into().ok()?))
}
fn u32le(b: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(o..o + 4)?.try_into().ok()?))
}

/// Decode a DIB (a BITMAPINFOHEADER followed by the palette and pixels). `bits_at`
/// is the offset of the pixel data within `dib` when known (from a file header).
pub fn decode_dib(dib: &[u8], bits_at: Option<usize>) -> Option<Image> {
    let hsize = u32le(dib, 0)? as usize;
    let w = u32le(dib, 4)? as i32;
    let h = u32le(dib, 8)? as i32;
    let bpp = u16le(dib, 14)?;
    let comp = u32le(dib, 16)?;
    let mut ncolors = u32le(dib, 32).unwrap_or(0) as usize;
    if ncolors == 0 && bpp <= 8 {
        ncolors = 1 << bpp;
    }
    let (w, flip) = (w.unsigned_abs() as usize, h > 0);
    let h = h.unsigned_abs() as usize;
    let pal_at = hsize;
    let palette: Vec<u32> = (0..ncolors)
        .map(|i| {
            let o = pal_at + 4 * i;
            let (b, g, r) = (dib.get(o).copied().unwrap_or(0), dib.get(o + 1).copied().unwrap_or(0), dib.get(o + 2).copied().unwrap_or(0));
            (r as u32) << 16 | (g as u32) << 8 | b as u32
        })
        .collect();
    let data_at = bits_at.unwrap_or(pal_at + 4 * ncolors);
    let data = dib.get(data_at..)?;
    // RLE pixels skipped by a delta or an early end of line keep palette entry 0, as
    // when GDI decodes the bitmap.
    let mut px = vec![palette.first().copied().unwrap_or(0); w * h];
    let mut put = |x: usize, y: usize, v: u32| {
        if x < w && y < h {
            let row = if flip { h - 1 - y } else { y };
            px[row * w + x] = v;
        }
    };
    match (bpp, comp) {
        (8, 0) => {
            let stride = (w + 3) & !3;
            for y in 0..h {
                for x in 0..w {
                    let i = *data.get(y * stride + x)? as usize;
                    put(x, y, palette.get(i).copied().unwrap_or(0));
                }
            }
        }
        (8, 1) => {
            // RLE8
            let (mut x, mut y, mut p) = (0usize, 0usize, 0usize);
            while p + 1 < data.len() {
                let (n, c) = (data[p] as usize, data[p + 1]);
                p += 2;
                if n > 0 {
                    for _ in 0..n {
                        put(x, y, palette.get(c as usize).copied().unwrap_or(0));
                        x += 1;
                    }
                    continue;
                }
                match c {
                    0 => {
                        x = 0;
                        y += 1;
                    }
                    1 => break,
                    2 => {
                        x += *data.get(p)? as usize;
                        y += *data.get(p + 1)? as usize;
                        p += 2;
                    }
                    k => {
                        let k = k as usize;
                        for i in 0..k {
                            let ci = *data.get(p + i)? as usize;
                            put(x, y, palette.get(ci).copied().unwrap_or(0));
                            x += 1;
                        }
                        p += (k + 1) & !1;
                    }
                }
            }
        }
        (24, 0) | (32, 0) => {
            let bytes = (bpp / 8) as usize;
            let stride = (w * bytes + 3) & !3;
            for y in 0..h {
                for x in 0..w {
                    let o = y * stride + x * bytes;
                    let (b, g, r) = (*data.get(o)?, *data.get(o + 1)?, *data.get(o + 2)?);
                    put(x, y, (r as u32) << 16 | (g as u32) << 8 | b as u32);
                }
            }
        }
        _ => return None,
    }
    Some(Image { w, h, px })
}

/// Decode a .bmp file.
pub fn decode_bmp(file: &[u8]) -> Option<Image> {
    if file.get(0..2)? != b"BM" {
        return None;
    }
    let bits = u32le(file, 10)? as usize;
    decode_dib(file.get(14..)?, Some(bits.checked_sub(14)?))
}

/// Named RT_BITMAP resources of a PE file (lf2.exe), as DIBs.
pub fn pe_bitmaps(exe: &[u8]) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    let Some(pe) = u32le(exe, 0x3c).map(|v| v as usize) else { return out };
    let (Some(nsec), Some(optsize)) = (u16le(exe, pe + 6), u16le(exe, pe + 20)) else { return out };
    let sec = pe + 24 + optsize as usize;
    let mut rsrc = None;
    for i in 0..nsec as usize {
        let s = sec + i * 40;
        if exe.get(s..s + 5) == Some(b".rsrc") {
            rsrc = Some((u32le(exe, s + 12).unwrap_or(0) as usize, u32le(exe, s + 20).unwrap_or(0) as usize));
        }
    }
    let Some((va, raw)) = rsrc else { return out };
    let entries = |off: usize| -> Vec<(u32, u32)> {
        let base = raw + off;
        let (Some(nn), Some(ni)) = (u16le(exe, base + 12), u16le(exe, base + 14)) else { return Vec::new() };
        (0..(nn + ni) as usize)
            .filter_map(|k| Some((u32le(exe, base + 16 + k * 8)?, u32le(exe, base + 20 + k * 8)?)))
            .collect()
    };
    let name_of = |n: u32| -> Option<String> {
        if n & 0x8000_0000 == 0 {
            return Some(n.to_string());
        }
        let p = raw + (n & 0x7fff_ffff) as usize;
        let len = u16le(exe, p)? as usize;
        let units: Vec<u16> = (0..len).filter_map(|i| u16le(exe, p + 2 + 2 * i)).collect();
        Some(String::from_utf16_lossy(&units))
    };
    for (t, toff) in entries(0) {
        if t != 2 || toff & 0x8000_0000 == 0 {
            continue;
        }
        for (n, noff) in entries((toff & 0x7fff_ffff) as usize) {
            let Some(name) = name_of(n) else { continue };
            // one language entry, then the data entry
            let Some(&(_, loff)) = entries((noff & 0x7fff_ffff) as usize).first() else { continue };
            let d = raw + (loff & 0x7fff_ffff) as usize;
            let (Some(rva), Some(size)) = (u32le(exe, d), u32le(exe, d + 4)) else { continue };
            let start = (rva as usize).wrapping_sub(va).wrapping_add(raw);
            if let Some(b) = exe.get(start..start + size as usize) {
                out.push((name, b.to_vec()));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rle8_runs_and_literals() {
        // 2x2, palette [black, red], rows bottom-up: row 0 = run of 2 red; row 1 = runs of 1
        let mut dib = vec![0u8; 40];
        dib[0] = 40;
        dib[4] = 2;
        dib[8] = 2;
        dib[12] = 1;
        dib[14] = 8;
        dib[16] = 1;
        dib[32] = 2;
        dib.extend_from_slice(&[0, 0, 0, 0, 0, 0, 255, 0]);
        dib.extend_from_slice(&[2, 1, 0, 0, 1, 0, 1, 1, 0, 1]);
        let im = decode_dib(&dib, None).unwrap();
        assert_eq!(im.px, vec![0, 0xff0000, 0xff0000, 0xff0000]);
    }
}
