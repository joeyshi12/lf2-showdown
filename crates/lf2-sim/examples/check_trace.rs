//! Run a recorded trace through the simulation and print where it first differs.
//! usage: LF2_DIR=... cargo run --example check_trace -- TRACE.lf2t [max_diffs]

use lf2_sim::trace::Trace;
use lf2_sim::{compare, World};
use std::sync::Arc;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let path = std::path::Path::new(&args[1]);
    let max: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(20);
    let dir = lf2_data::game_dir_from_env().expect("set LF2_DIR");
    let gd = Arc::new(lf2_data::GameData::load(&dir).unwrap());
    let exe = std::fs::read(dir.join("lf2.exe")).unwrap();
    let t = Trace::load(path, &exe).unwrap();
    let mut w = World::from_snapshot(&t.frames[0].state, gd);
        // state outside the snapshot (stage-mode spawn counters) comes as pokes
        w.apply_pokes(&t.frames[0].pokes);
    for n in 0..t.frames.len() - 1 {
        w.set_keys(&t.frames[n].keys);
        w.step();
        w.apply_pokes(&t.frames[n + 1].pokes);
        let d = compare::compare(&w, &t.frames[n + 1].state);
        if !d.is_empty() {
            println!("frame {} -> {}: {} differences", n, n + 1, d.len());
            for x in d.iter().take(max) {
                println!("  {:<28} ours {:<30} game {}", x.what, x.ours, x.theirs);
            }
            println!("unported sections reached: {:?}", w.unported_hits);
            std::process::exit(1);
        }
    }
    println!("{} frames match; unported sections reached: {:?}", t.frames.len() - 1, w.unported_hits);
}
