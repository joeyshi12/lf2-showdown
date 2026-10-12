//! Loaders for the data files of Little Fighter 2 v2.0a.
//!
//! Each loader is a port of the original routine named in its module and reproduces its
//! quirks, so that the numbers the simulation sees are the numbers the original sees.

pub mod ads;
pub mod bg;
pub mod controls;
pub mod vfs;
pub mod crypt;
pub mod flate;
pub mod fnt;
pub mod lfr;
pub mod image;
pub mod layout;
pub mod object;
pub mod scan;
pub mod stage;
pub mod write;

use std::path::{Path, PathBuf};

pub use bg::Background;
pub use object::{Frame, ObjectData};
pub use stage::Stage;

#[derive(Debug)]
pub enum Error {
    Io(PathBuf, std::io::Error),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Io(p, e) => write!(f, "{}: {}", p.display(), e),
        }
    }
}

impl std::error::Error for Error {}

/// Resolve a path as written in the game's files (`data\davis.dat`) against the game
/// folder. Windows paths are case-insensitive, so each component is matched ignoring
/// ASCII case when the exact name does not exist.
pub fn resolve(game_dir: &Path, win_path: &[u8]) -> PathBuf {
    let s = String::from_utf8_lossy(win_path).replace('\\', "/");
    let mut p = game_dir.to_path_buf();
    if vfs::installed() {
        for comp in s.split('/').filter(|c| !c.is_empty()) {
            p = p.join(comp);
        }
        return p;
    }
    for comp in s.split('/').filter(|c| !c.is_empty()) {
        let exact = p.join(comp);
        if exact.exists() {
            p = exact;
            continue;
        }
        let found = std::fs::read_dir(&p).ok().and_then(|rd| {
            rd.filter_map(|e| e.ok())
                .find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(comp))
                .map(|e| e.path())
        });
        p = found.unwrap_or(exact);
    }
    p
}

fn read(path: &Path) -> Result<Vec<u8>, Error> {
    vfs::read(path).map_err(|e| Error::Io(path.to_path_buf(), e))
}

/// The text a loader sees for a file: `.dat` files (by the last three letters, any case)
/// go through the decoder and `data\temporary.txt`; anything else is read in text mode.
pub fn load_text(path: &Path) -> Result<Vec<u8>, Error> {
    let raw = read(path)?;
    let name = path.to_string_lossy();
    let n = name.len();
    if n >= 3 && name[n - 3..].eq_ignore_ascii_case("dat") {
        Ok(crypt::decode_file(&raw))
    } else {
        Ok(crypt::text_mode_read(&raw))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ObjectEntry {
    pub id: i32,
    pub kind: i32,
    pub file: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BackgroundEntry {
    pub id: i32,
    pub file: Vec<u8>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Manifest {
    pub objects: Vec<ObjectEntry>,
    pub backgrounds: Vec<BackgroundEntry>,
    /// Checksum contribution of data.txt's own top-level tokens.
    pub checksum: i32,
}

/// Parse data.txt, a port of the token loop in `FUN_004122f0`.
pub fn parse_manifest(text: &[u8]) -> Manifest {
    let mut m = Manifest::default();
    let mut s = scan::Scanner::new(text);
    let mut tok = Vec::new();
    while !s.eof() {
        s.token(&mut tok);
        m.checksum = m.checksum.wrapping_add(scan::token_checksum(&tok));
        if tok == b"<object>" {
            s.token(&mut tok);
            while tok != b"<object_end>" {
                if tok == b"id:" {
                    // fscanf("%d %s %d %s %s", &id, tok, &type, tok, file)
                    let id = s.int().unwrap_or(0);
                    s.token(&mut tok);
                    let kind = s.int().unwrap_or(0);
                    s.token(&mut tok);
                    let mut file = Vec::new();
                    s.token(&mut file);
                    m.objects.push(ObjectEntry { id, kind, file });
                }
                s.token(&mut tok);
                if s.eof() {
                    break;
                }
            }
        }
        if tok == b"<background>" {
            s.token(&mut tok);
            while tok != b"<background_end>" {
                if tok == b"id:" {
                    // fscanf("%d %s %s", &id, tok, file)
                    let id = s.int().unwrap_or(0);
                    s.token(&mut tok);
                    let mut file = Vec::new();
                    s.token(&mut file);
                    m.backgrounds.push(BackgroundEntry { id, file });
                }
                s.token(&mut tok);
                if s.eof() {
                    break;
                }
            }
        }
    }
    m
}

/// All data the game loads at the start of a match (`FUN_004122f0`).
#[derive(Clone)]
pub struct GameData {
    pub dir: PathBuf,
    pub manifest: Manifest,
    /// In data.txt order; the index is what the original stores in its file manager.
    pub objects: Vec<ObjectData>,
    pub backgrounds: Vec<Background>,
    pub stages: Vec<Stage>,
    /// Global sound table: names in the order the loaders first saw them.
    pub sounds: Vec<Vec<u8>>,
    /// `DAT_0044f620` after loading (starting from 0).
    pub checksum: i32,
    /// Per object, per frame: the frame's index in `sounds`, or -1 (frame `+0x174`).
    pub frame_sound: Vec<Vec<i32>>,
    /// Per object: hit, drop and broken sound indices, or -1 (`+0xa4`, `+0xa8`, `+0xac`).
    pub weapon_sound: Vec<[i32; 3]>,
    /// data\\control.txt as read at start-up (None when missing).
    pub control_txt: Option<Vec<u8>>,
}

impl GameData {
    /// Background `i` as the file manager holds it: the loaded ones, the built-in 99,
    /// or an empty slot.
    pub fn bg(&self, i: i32) -> &Background {
        if i == 99 {
            return bg::lee_on_road();
        }
        if i >= 0 {
            if let Some(b) = self.backgrounds.get(i as usize) {
                return b;
            }
        }
        bg::empty_background()
    }

    /// The name the pick screen shows for background `i` ("Random" for 100).
    pub fn bg_name(&self, i: i32) -> &[u8] {
        if i == 100 {
            return b"Random";
        }
        &self.bg(i).name
    }

    pub fn load(dir: &Path) -> Result<GameData, Error> {
        let manifest = parse_manifest(&load_text(&dir.join("data").join("data.txt"))?);
        let mut checksum = manifest.checksum;
        let mut sounds: Vec<Vec<u8>> = Vec::new();
        let mut objects = Vec::new();
        for e in &manifest.objects {
            let text = load_text(&resolve(dir, &e.file))?;
            let (o, info) = object::parse_object(&text, e.id, e.kind);
            checksum = checksum.wrapping_add(info.checksum);
            for snd in info.sounds {
                if !sounds.contains(&snd) {
                    sounds.push(snd);
                }
            }
            objects.push(o);
        }
        let mut backgrounds = Vec::new();
        for e in &manifest.backgrounds {
            let text = load_text(&resolve(dir, &e.file))?;
            let (b, sum) = bg::parse_background(&text, e.id);
            checksum = checksum.wrapping_add(sum);
            backgrounds.push(b);
        }
        let stages = stage::parse_stages(&load_text(&dir.join("data").join("stage.dat"))?);
        let idx = |n: &Option<Vec<u8>>| n.as_ref().and_then(|n| sounds.iter().position(|s| s == n)).map_or(-1, |i| i as i32);
        let frame_sound = objects.iter().map(|o| o.frames.iter().map(|f| idx(&f.sound)).collect()).collect();
        let weapon_sound = objects
            .iter()
            .map(|o| [idx(&o.weapon_hit_sound), idx(&o.weapon_drop_sound), idx(&o.weapon_broken_sound)])
            .collect();
        Ok(GameData { dir: dir.to_path_buf(), manifest, objects, backgrounds, stages, sounds, checksum, frame_sound, weapon_sound, control_txt: vfs::read(&resolve(dir, b"data\\control.txt")).ok() })
    }

    pub fn object(&self, id: i32) -> Option<&ObjectData> {
        self.objects.iter().find(|o| o.id == id)
    }
}

pub fn game_dir_from_env() -> Option<PathBuf> {
    std::env::var_os("LF2_DIR").map(PathBuf::from).filter(|p| p.join("lf2.exe").exists() || p.join(EXE_IMAGE).exists())
}

/// The parts of lf2.exe the port uses, at their file offsets in lf2.exe: the PE
/// headers, `.data` and `.rsrc`, with the code sections zeroed. The repository's
/// `game/` holds this file instead of lf2.exe.
pub const EXE_IMAGE: &str = "lf2_image.bin";

/// lf2.exe from the game folder, or `EXE_IMAGE` when the folder has no lf2.exe.
pub fn read_exe(dir: &Path) -> std::io::Result<Vec<u8>> {
    std::fs::read(dir.join("lf2.exe")).or_else(|_| std::fs::read(dir.join(EXE_IMAGE)))
}
