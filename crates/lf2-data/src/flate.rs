//! zlib (RFC 1950/1951): a small decoder, and an encoder that writes fixed-Huffman
//! blocks with run-length matches, so the crates have no dependencies.


pub fn inflate_zlib(src: &[u8], out: &mut Vec<u8>) -> Result<(), String> {
    if src.len() < 2 || (src[0] & 0x0f) != 8 {
        return Err("not zlib data".into());
    }
    inflate(&src[2..], out)
}

struct Bits<'a> {
    s: &'a [u8],
    p: usize,
    bit: u32,
    n: u32,
}

impl<'a> Bits<'a> {
    fn need(&mut self, k: u32) -> Result<(), String> {
        while self.n < k {
            let b = *self.s.get(self.p).ok_or("deflate stream ended early")? as u32;
            self.p += 1;
            self.bit |= b << self.n;
            self.n += 8;
        }
        Ok(())
    }
    fn get(&mut self, k: u32) -> Result<u32, String> {
        if k == 0 {
            return Ok(0);
        }
        self.need(k)?;
        let v = self.bit & ((1u32 << k) - 1);
        self.bit >>= k;
        self.n -= k;
        Ok(v)
    }
}

struct Huff {
    counts: [u16; 16],
    syms: Vec<u16>,
}

impl Huff {
    fn new(lens: &[u8]) -> Huff {
        let mut counts = [0u16; 16];
        for &l in lens {
            counts[l as usize] += 1;
        }
        counts[0] = 0;
        let mut offs = [0u16; 16];
        for i in 1..16 {
            offs[i] = offs[i - 1] + counts[i - 1];
        }
        let mut syms = vec![0u16; lens.len()];
        for (s, &l) in lens.iter().enumerate() {
            if l != 0 {
                syms[offs[l as usize] as usize] = s as u16;
                offs[l as usize] += 1;
            }
        }
        Huff { counts, syms }
    }
    fn decode(&self, b: &mut Bits) -> Result<u16, String> {
        let (mut code, mut first, mut index) = (0i32, 0i32, 0i32);
        for len in 1..16 {
            code |= b.get(1)? as i32;
            let count = self.counts[len] as i32;
            if code - count < first {
                return Ok(self.syms[(index + (code - first)) as usize]);
            }
            index += count;
            first += count;
            first <<= 1;
            code <<= 1;
        }
        Err("bad huffman code".into())
    }
}

const LBASE: [u16; 29] = [3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195, 227, 258];
const LEXT: [u8; 29] = [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0];
const DBASE: [u16; 30] = [1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537, 2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577];
const DEXT: [u8; 30] = [0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13];

fn inflate(src: &[u8], out: &mut Vec<u8>) -> Result<(), String> {
    let mut b = Bits { s: src, p: 0, bit: 0, n: 0 };
    loop {
        let last = b.get(1)?;
        match b.get(2)? {
            0 => {
                // drop to a byte boundary, giving back whole buffered bytes
                b.p -= (b.n / 8) as usize;
                b.bit = 0;
                b.n = 0;
                let len = u16::from_le_bytes([src[b.p], src[b.p + 1]]) as usize;
                b.p += 4;
                out.extend_from_slice(&src[b.p..b.p + len]);
                b.p += len;
            }
            t @ (1 | 2) => {
                let (lit, dist) = if t == 1 {
                    let mut l = [0u8; 288];
                    for (i, v) in l.iter_mut().enumerate() {
                        *v = if i < 144 { 8 } else if i < 256 { 9 } else if i < 280 { 7 } else { 8 };
                    }
                    (Huff::new(&l), Huff::new(&[5u8; 30]))
                } else {
                    let hlit = b.get(5)? as usize + 257;
                    let hdist = b.get(5)? as usize + 1;
                    let hclen = b.get(4)? as usize + 4;
                    const ORD: [usize; 19] = [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15];
                    let mut cl = [0u8; 19];
                    for &o in ORD.iter().take(hclen) {
                        cl[o] = b.get(3)? as u8;
                    }
                    let ch = Huff::new(&cl);
                    let mut lens = vec![0u8; hlit + hdist];
                    let mut i = 0;
                    while i < hlit + hdist {
                        let s = ch.decode(&mut b)?;
                        match s {
                            0..=15 => {
                                lens[i] = s as u8;
                                i += 1;
                            }
                            16 => {
                                let prev = *lens.get(i.wrapping_sub(1)).ok_or("bad lengths")?;
                                for _ in 0..3 + b.get(2)? {
                                    lens[i] = prev;
                                    i += 1;
                                }
                            }
                            17 => i += 3 + b.get(3)? as usize,
                            _ => i += 11 + b.get(7)? as usize,
                        }
                    }
                    (Huff::new(&lens[..hlit]), Huff::new(&lens[hlit..]))
                };
                loop {
                    let s = lit.decode(&mut b)? as usize;
                    if s < 256 {
                        out.push(s as u8);
                    } else if s == 256 {
                        break;
                    } else {
                        let s = s - 257;
                        let len = LBASE[s] as usize + b.get(LEXT[s] as u32)? as usize;
                        let d = dist.decode(&mut b)? as usize;
                        let dd = DBASE[d] as usize + b.get(DEXT[d] as u32)? as usize;
                        let start = out.len() - dd;
                        for k in 0..len {
                            let c = out[start + k];
                            out.push(c);
                        }
                    }
                }
            }
            _ => return Err("bad block type".into()),
        }
        if last == 1 {
            return Ok(());
        }
    }
}

/// Adler-32 of `data`.
pub fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for c in data.chunks(5552) {
        for &x in c {
            a += x as u32;
            b += a;
        }
        a %= 65521;
        b %= 65521;
    }
    (b << 16) | a
}

struct BitWriter {
    out: Vec<u8>,
    acc: u64,
    n: u32,
}

impl BitWriter {
    fn put(&mut self, v: u32, bits: u32) {
        self.acc |= (v as u64) << self.n;
        self.n += bits;
        while self.n >= 8 {
            self.out.push(self.acc as u8);
            self.acc >>= 8;
            self.n -= 8;
        }
    }

    /// A Huffman code, most significant bit first.
    fn code(&mut self, c: u32, len: u32) {
        let mut r = 0;
        for i in 0..len {
            r |= ((c >> i) & 1) << (len - 1 - i);
        }
        self.put(r, len);
    }

    fn lit(&mut self, v: u32) {
        match v {
            0..=143 => self.code(0x30 + v, 8),
            144..=255 => self.code(0x190 + v - 144, 9),
            256..=279 => self.code(v - 256, 7),
            _ => self.code(0xc0 + v - 280, 8),
        }
    }
}

const LEN_BASE: [u32; 29] = [3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195, 227, 258];
const LEN_EXTRA: [u32; 29] = [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0];

/// zlib-compress `data` (one fixed-Huffman block; repeats of the previous byte are
/// coded as matches at distance 1).
pub fn deflate_zlib(data: &[u8]) -> Vec<u8> {
    let mut w = BitWriter { out: vec![0x78, 0x01], acc: 0, n: 0 };
    w.put(1, 1);
    w.put(1, 2);
    let mut i = 0;
    while i < data.len() {
        let mut run = 0;
        if i > 0 {
            while i + run < data.len() && run < 258 && data[i + run] == data[i - 1] {
                run += 1;
            }
        }
        if run >= 3 {
            let k = (0..29).rev().find(|&k| LEN_BASE[k] <= run as u32).unwrap();
            w.lit(257 + k as u32);
            w.put(run as u32 - LEN_BASE[k], LEN_EXTRA[k]);
            w.code(0, 5); // distance code 0: distance 1
            i += run;
        } else {
            w.lit(data[i] as u32);
            i += 1;
        }
    }
    w.lit(256);
    if w.n > 0 {
        w.put(0, 8 - w.n);
    }
    let mut out = w.out;
    out.extend_from_slice(&adler32(data).to_be_bytes());
    out
}
