//! Every trace in `traces/` must run through the simulation with no difference from
//! the original on any frame. Needs `LF2_DIR`; without it the test prints a note and
//! passes.

use lf2_sim::trace::Trace;
use lf2_sim::{compare, World};
use std::sync::Arc;

#[test]
fn recorded_traces_match_the_original() {
    let Some(dir) = lf2_data::game_dir_from_env() else {
        eprintln!("LF2_DIR not set; skipping");
        return;
    };
    let gd = Arc::new(lf2_data::GameData::load(&dir).unwrap());
    let exe = std::fs::read(dir.join("lf2.exe")).unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../traces");
    let mut paths: Vec<_> = std::fs::read_dir(&root)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().map_or(false, |x| x == "lf2t"))
        .collect();
    paths.sort();
    assert!(!paths.is_empty(), "no traces in {}", root.display());
    let mut failures = Vec::new();
    for p in &paths {
        let t = Trace::load(p, &exe).unwrap();
        let mut w = World::from_snapshot(&t.frames[0].state, gd.clone());
        // state outside the snapshot (stage-mode spawn counters) comes as pokes
        w.apply_pokes(&t.frames[0].pokes);
        for n in 0..t.frames.len() - 1 {
            w.set_keys(&t.frames[n].keys);
            w.step();
            w.apply_pokes(&t.frames[n + 1].pokes);
            let d = compare::compare(&w, &t.frames[n + 1].state);
            if !d.is_empty() {
                let shown: Vec<String> =
                    d.iter().take(6).map(|x| format!("{} ours {} game {}", x.what, x.ours, x.theirs)).collect();
                failures.push(format!(
                    "{}: frame {}: {} differences: {}; unported sections reached: {:?}",
                    p.file_name().unwrap().to_string_lossy(),
                    n,
                    d.len(),
                    shown.join("; "),
                    w.unported_hits
                ));
                break;
            }
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}
