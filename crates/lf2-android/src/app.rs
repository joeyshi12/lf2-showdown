//! android_main: copy the game files, then run the game.

use lf2_app::game::{run, Options};
use crate::input::AndroidInput;
use lf2_app::window::Display;
use std::ffi::CString;
use std::io::Read;
use std::path::Path;
use winit::event_loop::EventLoop;
use winit::platform::android::activity::{AndroidApp, WindowManagerFlags};
use winit::platform::android::EventLoopBuilderExtAndroid;

/// The list of game files in the assets: one line per file, its SHA-256, two spaces
/// and its path inside `game/`.
const FILE_LIST: &str = "game/files.txt";
/// The copy of `FILE_LIST` the files in internal storage were copied from.
const COPIED_LIST: &str = ".files.txt";

#[no_mangle]
fn android_main(app: AndroidApp) {
    log_stderr();
    let dir = app.internal_data_path().expect("no internal storage").join("game");
    if let Err(e) = copy_game_files(&app, &dir) {
        panic!("cannot copy the game files to {}: {}", dir.display(), e);
    }
    app.set_window_flags(WindowManagerFlags::KEEP_SCREEN_ON | WindowManagerFlags::FULLSCREEN, WindowManagerFlags::empty());
    let input = AndroidInput::new(app.clone());
    let event_loop = EventLoop::builder().with_android_app(app).build().expect("event loop");
    let options = Options { dir, p1: None, p2: None, bg: None, seed: None, fps: 60, fullscreen: true, touch: true, dump: None };
    run(options, |fullscreen| {
        let mut d = Display::with_event_loop(event_loop, "Little Fighter 2", fullscreen);
        d.set_input_source(Box::new(input));
        d
    });
    // Returning finishes the activity; MainActivity.onDestroy then ends the process.
}

fn read_asset(app: &AndroidApp, path: &str) -> std::io::Result<Vec<u8>> {
    let name = CString::new(path).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
    let mut asset = app
        .asset_manager()
        .open(&name)
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, path.to_string()))?;
    let mut bytes = Vec::new();
    asset.read_to_end(&mut bytes)?;
    Ok(bytes)
}

/// Copy the game files from the assets into `dir`, unless they were copied from the
/// same list before. The player's own files are kept: data/control.txt (the control
/// settings) when it exists, and the recordings, which the assets do not have.
fn copy_game_files(app: &AndroidApp, dir: &Path) -> std::io::Result<()> {
    let list = read_asset(app, FILE_LIST)?;
    if std::fs::read(dir.join(COPIED_LIST)).is_ok_and(|old| old == list) {
        return Ok(());
    }
    eprintln!("copying the game files to {}", dir.display());
    for line in String::from_utf8_lossy(&list).lines() {
        let Some((_, rel)) = line.split_once("  ") else { continue };
        let to = dir.join(rel);
        if rel.eq_ignore_ascii_case("data/control.txt") && to.exists() {
            continue;
        }
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&to, read_asset(app, &format!("game/{}", rel))?)?;
    }
    std::fs::create_dir_all(dir.join("recording"))?;
    std::fs::write(dir.join(COPIED_LIST), list)
}

/// Send what the game writes to stderr (its messages, and panics) to the system log,
/// under the tag "lf2".
fn log_stderr() {
    #[link(name = "log")]
    extern "C" {
        fn __android_log_write(prio: libc::c_int, tag: *const libc::c_char, text: *const libc::c_char) -> libc::c_int;
    }
    const ANDROID_LOG_INFO: libc::c_int = 4;
    let mut fds = [0; 2];
    // SAFETY: plain libc calls on descriptors this function owns
    unsafe {
        if libc::pipe(fds.as_mut_ptr()) != 0 || libc::dup2(fds[1], libc::STDERR_FILENO) < 0 {
            return;
        }
        libc::close(fds[1]);
    }
    let read_end = fds[0];
    std::thread::spawn(move || {
        use std::io::BufRead;
        use std::os::fd::FromRawFd;
        // SAFETY: the read end of the pipe made above, owned by this thread from here on
        let file = unsafe { std::fs::File::from_raw_fd(read_end) };
        let tag = CString::new("lf2").unwrap();
        for line in std::io::BufReader::new(file).lines() {
            let Ok(line) = line else { break };
            let Ok(text) = CString::new(line) else { continue };
            // SAFETY: both strings are NUL-terminated and live across the call
            unsafe { __android_log_write(ANDROID_LOG_INFO, tag.as_ptr(), text.as_ptr()) };
        }
    });
}
