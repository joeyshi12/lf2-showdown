//! Game files held in memory, for platforms without a file system (the web build).
//! When files are installed, `read` looks paths up in them ignoring ASCII case;
//! otherwise it reads the disk.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

static FILES: OnceLock<Mutex<HashMap<String, Vec<u8>>>> = OnceLock::new();

fn key(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/").trim_start_matches("./").trim_start_matches('/').to_ascii_lowercase()
}

/// Use these files (paths relative to the game folder, `/` or `\` separated)
/// instead of the disk. The game folder is then the empty path.
pub fn install(files: Vec<(String, Vec<u8>)>) {
    let map = files.into_iter().map(|(p, b)| (key(Path::new(&p)), b)).collect();
    let cell = FILES.get_or_init(|| Mutex::new(HashMap::new()));
    *cell.lock().unwrap() = map;
}

pub fn installed() -> bool {
    FILES.get().is_some()
}

/// Read a file (from memory when files are installed).
pub fn read(p: &Path) -> std::io::Result<Vec<u8>> {
    match FILES.get() {
        Some(m) => m
            .lock()
            .unwrap()
            .get(&key(p))
            .cloned()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, p.to_string_lossy().into_owned())),
        None => std::fs::read(p),
    }
}

/// Write a file (kept in memory when files are installed).
pub fn write(p: &Path, data: &[u8]) -> std::io::Result<()> {
    match FILES.get() {
        Some(m) => {
            m.lock().unwrap().insert(key(p), data.to_vec());
            Ok(())
        }
        None => std::fs::write(p, data),
    }
}
