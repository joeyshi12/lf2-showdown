//! Byte images with the original's layout.
//!
//! The simulation keeps each game object, and the global variables the tick uses, as
//! byte arrays laid out exactly as in `lf2.exe`'s memory. Code ported from the decompile
//! reads and writes them at the same offsets, and the differential tests compare them
//! byte for byte with the running original. Named offsets live in `fields.rs`.

/// A little-endian byte image.
#[derive(Clone, PartialEq, Eq)]
pub struct Mem {
    pub base: u32,
    pub bytes: Vec<u8>,
}

impl Mem {
    pub fn new(base: u32, len: usize) -> Self {
        Mem { base, bytes: vec![0; len] }
    }

    #[inline]
    fn at(&self, addr: u32) -> usize {
        (addr - self.base) as usize
    }

    #[inline]
    pub fn u8(&self, a: u32) -> u8 {
        self.bytes[self.at(a)]
    }
    #[inline]
    pub fn i8(&self, a: u32) -> i8 {
        self.u8(a) as i8
    }
    #[inline]
    pub fn i32(&self, a: u32) -> i32 {
        let i = self.at(a);
        i32::from_le_bytes(self.bytes[i..i + 4].try_into().unwrap())
    }
    #[inline]
    pub fn u32(&self, a: u32) -> u32 {
        self.i32(a) as u32
    }
    #[inline]
    pub fn f64(&self, a: u32) -> f64 {
        let i = self.at(a);
        f64::from_le_bytes(self.bytes[i..i + 8].try_into().unwrap())
    }
    #[inline]
    pub fn set_u8(&mut self, a: u32, v: u8) {
        let i = self.at(a);
        self.bytes[i] = v;
    }
    #[inline]
    pub fn set_i8(&mut self, a: u32, v: i8) {
        self.set_u8(a, v as u8)
    }
    #[inline]
    pub fn set_i32(&mut self, a: u32, v: i32) {
        let i = self.at(a);
        self.bytes[i..i + 4].copy_from_slice(&v.to_le_bytes());
    }
    #[inline]
    pub fn set_f64(&mut self, a: u32, v: f64) {
        let i = self.at(a);
        self.bytes[i..i + 8].copy_from_slice(&v.to_le_bytes());
    }
    #[inline]
    pub fn add_i32(&mut self, a: u32, d: i32) {
        let v = self.i32(a).wrapping_add(d);
        self.set_i32(a, v);
    }
}

/// `_ftol2` as the game uses it: truncate to a 64-bit integer and keep the low 32 bits.
/// NaN and values outside the 64-bit range give the x87 "integer indefinite"
/// 0x8000000000000000, whose low half is 0.
#[inline]
pub fn ftol(v: f64) -> i32 {
    if v.is_nan() || v >= 9.223372036854775808e18 || v < -9.223372036854775808e18 {
        0
    } else {
        (v as i64) as i32
    }
}
