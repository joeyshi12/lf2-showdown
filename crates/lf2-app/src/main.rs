//! Desktop game. Alt+Enter toggles full screen; `--touch` shows on-screen
//! controls for touch screens.
//!
//! `lf2 [--game DIR] [--p1 NAME --p2 NAME] [--bg N] [--seed N] [--fps 60|30]
//!      [--fullscreen] [--touch] [--dump FRAMES OUT.ppm]`

use lf2_app::game::{run, Options};
use lf2_app::window::Display;

fn is_game_dir(d: &std::path::Path) -> bool {
    lf2_data::resolve(d, b"data\\data.txt").is_file()
}

fn main() {
    let mut args = std::env::args().skip(1);
    let mut dir = lf2_data::game_dir_from_env();
    let (mut p1, mut p2, mut bg, mut seed) = (None::<String>, None::<String>, None, None);
    let mut dump: Option<(u32, String)> = None;
    let mut fps = 60;
    let mut fullscreen = false;
    let mut touch = false;
    while let Some(a) = args.next() {
        let mut val = || args.next().unwrap_or_else(|| panic!("{} needs a value", a));
        match a.as_str() {
            "--game" => dir = Some(val().into()),
            "--p1" => p1 = Some(val()),
            "--p2" => p2 = Some(val()),
            "--bg" => bg = val().parse().ok(),
            "--seed" => seed = val().parse().ok(),
            "--fps" => {
                fps = val().parse().unwrap_or(0);
                if fps != 30 && fps != 60 {
                    panic!("--fps is 60 or 30");
                }
            }
            "--fullscreen" => fullscreen = true,
            "--touch" => touch = true,
            "--dump" => {
                let n = val().parse().expect("frames");
                dump = Some((n, val()));
            }
            "-h" | "--help" => {
                println!("usage: lf2 [--game DIR] [--p1 NAME] [--p2 NAME] [--bg N] [--seed N] [--fps 60|30] [--fullscreen] [--touch] [--dump FRAMES OUT.ppm]");
                return;
            }
            other => panic!("unknown argument {}", other),
        }
    }
    let dir = dir
        .or_else(|| {
            let here = std::env::current_exe().ok()?.parent()?.to_path_buf();
            is_game_dir(&here).then_some(here)
        })
        .or_else(|| is_game_dir(std::path::Path::new(".")).then(|| ".".into()))
        .expect("put this program in the Little Fighter 2 folder, or set LF2_DIR or pass --game DIR (the folder with data/data.txt)");
    run(Options { dir, p1, p2, bg, seed, fps, fullscreen, touch, dump }, |fullscreen| Display::new("Little Fighter 2", fullscreen));
}
