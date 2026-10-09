//! The ads shown in the main menus: `data\adinfo.txt` (`ads_load_info`, 0x43c4a0)
//! names which `data\adN.txt` to use, and that file (`ads_load`, 0x43c780) lists the
//! link boxes of the bottom strip (`ba`), the pictures on the right (`ta`), the newest
//! version (`un`) and the menu offset (`y`). The pictures are in `sprite\sys\adN.bmp`.
//! The original downloads newer files in a thread; the port only reads the local ones.

use std::path::Path;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Box {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub url: Vec<u8>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Picture {
    pub start: i32,
    pub end: i32,
    pub len: i32,
    pub url: Vec<u8>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Ads {
    /// The first token of adinfo.txt (a date, "now" or "dont_update").
    pub stamp: Vec<u8>,
    /// N of adN.txt / adN.bmp, and the second number of adinfo.txt.
    pub index: i32,
    pub info2: i32,
    /// The first number of adN.txt (its date).
    pub date: i32,
    /// 24 link boxes; x is -99 for unused ones.
    pub boxes: Vec<Box>,
    /// 8 pictures; start is -99 for unused ones.
    pub pictures: Vec<Picture>,
    /// The newest version (compared with the exe's 30) and where to get it.
    pub new_version: i32,
    pub new_version_url: Vec<u8>,
    /// Menu y offset and the second `y` number.
    pub menu_y: i32,
    pub y2: i32,
}

/// One `sscanf` conversion over whitespace-separated tokens: `%s` takes a token,
/// `%d` the leading integer of one (and fails without digits).
struct Scan<'a> {
    toks: std::vec::IntoIter<&'a [u8]>,
    ok: bool,
}

impl<'a> Scan<'a> {
    fn new(line: &'a [u8]) -> Self {
        let toks: Vec<&[u8]> = line.split(|c| c.is_ascii_whitespace()).filter(|t| !t.is_empty()).collect();
        Scan { toks: toks.into_iter(), ok: true }
    }

    fn s(&mut self, into: &mut Vec<u8>) {
        if !self.ok {
            return;
        }
        match self.toks.next() {
            Some(t) => *into = t.to_vec(),
            None => self.ok = false,
        }
    }

    fn d(&mut self, into: &mut i32) {
        if !self.ok {
            return;
        }
        let Some(t) = self.toks.next() else {
            self.ok = false;
            return;
        };
        let (neg, digits) = match t.first() {
            Some(b'-') => (true, &t[1..]),
            Some(b'+') => (false, &t[1..]),
            _ => (false, t),
        };
        let n: Vec<u8> = digits.iter().copied().take_while(|c| c.is_ascii_digit()).collect();
        if n.is_empty() {
            self.ok = false;
            return;
        }
        let v = std::str::from_utf8(&n).unwrap().parse::<i64>().unwrap_or(0) as i32;
        *into = if neg { v.wrapping_neg() } else { v };
        // the rest of the token is left for the next conversion
        if n.len() < digits.len() {
            self.ok = false;
        }
    }
}

fn url_ok(u: &[u8]) -> bool {
    matches!(u.first(), Some(b'?') | Some(b'h') | Some(b'H'))
}

/// Read adinfo.txt and adN.txt from the game folder. None when the original would
/// show no ads (a file missing or malformed).
pub fn load(dir: &Path) -> Option<Ads> {
    let info = crate::vfs::read(&crate::resolve(dir, b"data\\adinfo.txt")).ok()?;
    let mut a = Ads::default();
    let (mut b, mut c, mut d) = (Vec::new(), Vec::new(), Vec::new());
    let mut sc = Scan::new(&info);
    sc.s(&mut a.stamp);
    sc.s(&mut b);
    sc.s(&mut c);
    sc.s(&mut d);
    if d != b"<end>" {
        return None;
    }
    let date_ok = {
        let p: Vec<&[u8]> = a.stamp.split(|&ch| ch == b'/').collect();
        p.len() == 6 && p.iter().all(|x| !x.is_empty() && x.iter().all(|ch| ch.is_ascii_digit()))
    };
    if !(a.stamp == b"now" || a.stamp == b"dont_update" || date_ok) {
        return None;
    }
    a.index = -99;
    a.info2 = -99;
    Scan::new(&b).d(&mut a.index);
    Scan::new(&c).d(&mut a.info2);
    if a.index == -99 || a.info2 == -99 {
        return None;
    }
    let text = crate::vfs::read(&crate::resolve(dir, format!("data\\ad{}.txt", a.index).as_bytes())).ok()?;
    let mut lines = text.split(|&ch| ch == b'\n');
    let mut next = || lines.next().unwrap_or(b"");
    a.date = -99;
    Scan::new(next()).d(&mut a.date);
    if a.date == -99 {
        return None;
    }
    for _ in 0..24 {
        let mut bx = Box { x: -99, y: -99, w: -99, h: -99, url: b"-".to_vec() };
        let (mut tag, mut end) = (Vec::new(), Vec::new());
        let mut sc = Scan::new(next());
        sc.s(&mut tag);
        sc.d(&mut bx.x);
        sc.d(&mut bx.y);
        sc.d(&mut bx.w);
        sc.d(&mut bx.h);
        sc.s(&mut bx.url);
        sc.s(&mut end);
        let ok = tag == b"ba" && end == b"bae" && url_ok(&bx.url) && bx.x != -99 && bx.y != -99 && bx.w != -99 && bx.h != -99;
        if !ok {
            return None;
        }
        a.boxes.push(bx);
    }
    for _ in 0..8 {
        let mut p = Picture { start: -99, end: -99, len: -99, url: b"-".to_vec() };
        let (mut tag, mut end) = (Vec::new(), Vec::new());
        let mut sc = Scan::new(next());
        sc.s(&mut tag);
        sc.d(&mut p.start);
        sc.d(&mut p.end);
        sc.d(&mut p.len);
        sc.s(&mut p.url);
        sc.s(&mut end);
        if !(tag == b"ta" && end == b"tae" && url_ok(&p.url) && p.start != -99 && p.end != -99 && p.len != -99) {
            return None;
        }
        a.pictures.push(p);
    }
    let (mut tag, mut end) = (Vec::new(), Vec::new());
    a.new_version = -99;
    a.new_version_url = b"-".to_vec();
    let mut sc = Scan::new(next());
    sc.s(&mut tag);
    sc.d(&mut a.new_version);
    sc.s(&mut a.new_version_url);
    sc.s(&mut end);
    if !(tag == b"un" && end == b"une" && url_ok(&a.new_version_url) && a.new_version != -99) {
        return None;
    }
    let (mut tag, mut end) = (Vec::new(), Vec::new());
    a.menu_y = -99;
    a.y2 = -99;
    let mut sc = Scan::new(next());
    sc.s(&mut tag);
    sc.d(&mut a.menu_y);
    sc.d(&mut a.y2);
    sc.s(&mut end);
    if !(tag == b"y" && end == b"ye" && a.menu_y != -99 && a.y2 != -99) {
        return None;
    }
    let mut tag = Vec::new();
    Scan::new(next()).s(&mut tag);
    if tag != b"<end>" {
        return None;
    }
    Some(a)
}
