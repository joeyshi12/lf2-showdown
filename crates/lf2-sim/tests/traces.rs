//! Every trace in `traces/` must run through the simulation with no difference from
//! the original on any frame. Needs `LF2_DIR`; without it the test prints a note and
//! passes.

use lf2_sim::trace::Trace;
use std::sync::Arc;

#[test]
fn recorded_traces_match_the_original() {
    let Some(dir) = lf2_data::game_dir_from_env() else {
        eprintln!("LF2_DIR not set; skipping");
        return;
    };
    let gd = Arc::new(lf2_data::GameData::load(&dir).unwrap());
    let exe = lf2_data::read_exe(&dir).unwrap();
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
        let mut w = t.start_world(gd.clone());
        for n in 0..t.frames.len() - 1 {
            let d = t.step_and_compare(&mut w, n);
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
