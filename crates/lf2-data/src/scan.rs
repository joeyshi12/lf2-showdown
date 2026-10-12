//! An emulation of the parts of MSVC 2005 `fscanf` that the loaders use: `%s`, `%d`
//! and `%lf`, plus `feof`.
//!
//! The differences from a plain tokenizer matter for matching the original:
//! - `feof` only becomes true after a read has tried to go past the end. A file whose
//!   last token is followed by whitespace therefore makes the loaders run one extra
//!   `fscanf("%s")` that fails at the end.
//! - A failed `%s` leaves the destination buffer unchanged, so the loaders process the
//!   previous token a second time. [`Scanner::token`] keeps that behaviour by not touching
//!   its buffer on failure.

pub struct Scanner<'a> {
    buf: &'a [u8],
    pos: usize,
    eof: bool,
}

/// `isspace` in the "C" locale.
fn is_space(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r')
}

impl<'a> Scanner<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        Scanner { buf, pos: 0, eof: false }
    }

    /// What has not been read yet.
    pub fn rest(&self) -> &'a [u8] {
        &self.buf[self.pos.min(self.buf.len())..]
    }

    /// `feof(file)`.
    pub fn eof(&self) -> bool {
        self.eof
    }

    fn peek(&mut self) -> Option<u8> {
        match self.buf.get(self.pos) {
            Some(&c) => Some(c),
            None => {
                self.eof = true;
                None
            }
        }
    }

    fn skip_space(&mut self) {
        while let Some(c) = self.peek() {
            if !is_space(c) {
                break;
            }
            self.pos += 1;
        }
    }

    /// `fscanf(f, "%s", buf)`. Returns false at end of file and leaves `out` unchanged.
    pub fn token(&mut self, out: &mut Vec<u8>) -> bool {
        self.skip_space();
        if self.peek().is_none() {
            return false;
        }
        out.clear();
        while let Some(c) = self.peek() {
            if is_space(c) {
                break;
            }
            out.push(c);
            self.pos += 1;
        }
        true
    }

    /// `fscanf(f, "%d", &v)`. `None` on a matching failure or end of file.
    pub fn int(&mut self) -> Option<i32> {
        self.skip_space();
        let mut neg = false;
        match self.peek()? {
            b'-' => {
                neg = true;
                self.pos += 1;
            }
            b'+' => self.pos += 1,
            _ => {}
        }
        let mut v: i64 = 0;
        let mut any = false;
        while let Some(c) = self.peek() {
            if !c.is_ascii_digit() {
                break;
            }
            any = true;
            v = v.wrapping_mul(10).wrapping_add((c - b'0') as i64);
            self.pos += 1;
        }
        if !any {
            return None;
        }
        // The CRT accumulates in an unsigned long and negates; the result is the low 32 bits.
        let v = if neg { v.wrapping_neg() } else { v };
        Some(v as i32)
    }

    fn digits(&mut self, out: &mut String) -> usize {
        let start = self.pos;
        while let Some(c) = self.peek().filter(u8::is_ascii_digit) {
            out.push(c as char);
            self.pos += 1;
        }
        self.pos - start
    }

    /// `fscanf(f, "%lf", &v)`.
    pub fn float(&mut self) -> Option<f64> {
        self.skip_space();
        let mut s = String::new();
        if let Some(c @ (b'-' | b'+')) = self.peek() {
            s.push(c as char);
            self.pos += 1;
        }
        let mut digits = self.digits(&mut s);
        if self.peek() == Some(b'.') {
            s.push('.');
            self.pos += 1;
            digits += self.digits(&mut s);
        }
        if digits == 0 {
            return None;
        }
        if let Some(b'e' | b'E') = self.peek() {
            let save = self.pos;
            let mut e = String::from("e");
            self.pos += 1;
            if let Some(c @ (b'-' | b'+')) = self.peek() {
                e.push(c as char);
                self.pos += 1;
            }
            let ed = self.digits(&mut e);
            if ed > 0 {
                s.push_str(&e);
            } else {
                self.pos = save;
            }
        }
        s.parse().ok()
    }
}

/// The data checksum `DAT_0044f620`: every loader adds `sum(token[i] * i)` (with signed
/// chars) for each token it reads in its top-level loop. The network code compares it
/// between peers.
pub fn token_checksum(tok: &[u8]) -> i32 {
    tok.iter()
        .enumerate()
        .fold(0i32, |acc, (i, &c)| acc.wrapping_add((c as i8 as i32).wrapping_mul(i as i32)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_token_keeps_buffer_and_sets_eof() {
        let mut s = Scanner::new(b"a b \n");
        let mut t = Vec::new();
        assert!(s.token(&mut t));
        assert!(s.token(&mut t));
        assert_eq!(t, b"b");
        assert!(!s.eof());
        assert!(!s.token(&mut t));
        assert_eq!(t, b"b");
        assert!(s.eof());
    }

    #[test]
    fn token_at_end_without_newline_sets_eof() {
        let mut s = Scanner::new(b"a");
        let mut t = Vec::new();
        assert!(s.token(&mut t));
        assert!(s.eof());
    }

    #[test]
    fn int_stops_at_non_digit() {
        let mut s = Scanner::new(b" -12.5x");
        assert_eq!(s.int(), Some(-12));
        let mut t = Vec::new();
        s.token(&mut t);
        assert_eq!(t, b".5x");
    }

    #[test]
    fn float_parses_plain_decimals() {
        let mut s = Scanner::new(b"5.000000 .5 -3");
        assert_eq!(s.float(), Some(5.0));
        assert_eq!(s.float(), Some(0.5));
        assert_eq!(s.float(), Some(-3.0));
    }
}
