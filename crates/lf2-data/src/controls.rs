//! `data\control.txt` as `load_controls` (0x423480) reads it: per player 11 numbers
//! (input kind, up down left right attack jump defend, then three more), the four
//! player names (`` ` `` stands for a space), the ads option, the recording option,
//! then the name, e-mail and info lines of the network dialog.

use crate::scan::Scanner;
use std::path::Path;

#[derive(Clone, Debug, PartialEq)]
pub struct Controls {
    /// Per player: kind (0 keyboard), 7 keys, 3 more numbers.
    pub players: [[i32; 11]; 4],
    pub names: [Vec<u8>; 4],
    /// 1: show ads, 0: no ads, -1: ask (`DAT_00450be8`).
    pub ads: i32,
    /// `DAT_00450be4`: record every match.
    pub record: i32,
    pub name: Vec<u8>,
    pub email: Vec<u8>,
    /// Everything after the e-mail line.
    pub info: Vec<u8>,
    /// How many line-end and trailing-space bytes fgets read after `name` and
    /// `email` (load_controls sets them to 0).
    pub name_end: usize,
    pub email_end: usize,
}

impl Default for Controls {
    fn default() -> Self {
        Controls {
            players: [
                [0, 87, 83, 65, 68, 74, 75, 76, 0, 1, 3],
                [1, 87, 88, 65, 68, 83, 9, 192, 0, 1, 3],
                [1, 38, 40, 37, 39, 13, 16, 17, 0, 1, 2],
                [1, 73, 188, 74, 76, 75, 32, 190, 0, 1, 3],
            ],
            names: [b"1".to_vec(), b"2".to_vec(), b"3".to_vec(), b"4".to_vec()],
            ads: 1,
            record: 0,
            name: b"<No name>".to_vec(),
            email: b"<No email>".to_vec(),
            info: b"<No info>\n".to_vec(),
            name_end: 0,
            email_end: 0,
        }
    }
}

fn trim_end(v: &mut Vec<u8>) {
    while matches!(v.last(), Some(b' ') | Some(b'\r') | Some(b'\n')) {
        v.pop();
    }
}

/// Read control.txt; None when the file is missing (the original then reads nothing
/// and keeps zeros).
pub fn load(dir: &Path) -> Option<Controls> {
    Some(parse(&crate::vfs::read(&crate::resolve(dir, b"data\\control.txt")).ok()?))
}

/// Parse the text of control.txt as `load_controls` (0x423480) reads it.
pub fn parse(text: &[u8]) -> Controls {
    let text: Vec<u8> = text.iter().copied().filter(|&c| c != b'\r').collect();
    let mut c = Controls::default();
    let mut s = Scanner::new(&text);
    for p in 0..4 {
        for k in 0..11 {
            if let Some(v) = s.int() {
                c.players[p][k] = v;
            }
        }
    }
    for n in c.names.iter_mut() {
        let mut t = Vec::new();
        if s.token(&mut t) {
            *n = t.iter().map(|&ch| if ch == b'`' { b' ' } else { ch }).collect();
        }
    }
    if let Some(v) = s.int() {
        c.ads = v;
    }
    if let Some(v) = s.int() {
        c.record = v;
    }
    // "%d\n" skips the whitespace after the number; then fgets reads whole lines
    let rest = s.rest();
    let skip = rest.iter().take_while(|ch| ch.is_ascii_whitespace()).count();
    let mut lines = rest[skip..].split_inclusive(|&ch| ch == b'\n');
    c.name = lines.next().unwrap_or(b"").to_vec();
    c.email = lines.next().unwrap_or(b"").to_vec();
    let (n, e) = (c.name.len(), c.email.len());
    trim_end(&mut c.name);
    trim_end(&mut c.email);
    c.name_end = n - c.name.len();
    c.email_end = e - c.email.len();
    c.info = lines.flat_map(|l| l.iter().copied()).collect();
    c
}
