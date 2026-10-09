//! The demo recordings in the game folder, made by the original, replay in the
//! simulation to the end of their match: the hp sums the recording holds every 150
//! frames all match (a mismatch ends the replay with "Recording file error!").
use std::sync::Arc;

#[test]
fn demo_recordings_replay_to_the_end() {
    let Some(dir) = lf2_data::game_dir_from_env() else { return };
    let exe = std::fs::read(dir.join("lf2.exe")).unwrap();
    let gd = Arc::new(lf2_data::GameData::load(&dir).unwrap());
    let controls = lf2_data::controls::load(&dir).unwrap_or_default();
    let ads = lf2_data::ads::load(&dir);
    let mut failures = Vec::new();
    let mut n_files = 0;
    for e in std::fs::read_dir(dir.join("recording")).unwrap() {
        let p = e.unwrap().path();
        if p.extension().map_or(true, |x| x != "lfr") {
            continue;
        }
        n_files += 1;
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        let rec = lf2_data::lfr::decode(&std::fs::read(&p).unwrap(), &exe).unwrap();
        let frames = i32::from_le_bytes(rec[0x144..0x148].try_into().unwrap());
        let mut w = lf2_sim::World::new_boot(gd.clone(), &exe, &controls, ads.as_ref(), 0);
        if let Err(e) = w.load_replay(rec) {
            failures.push(format!("{name}: {e:?}"));
            continue;
        }
        let mut n = 0;
        let mut failed = false;
        while w.g.match_phase() == 0 && w.g.end_timer() < 0x90 && n < 200_000 {
            w.set_keys(&[b'u'; 256]);
            w.step();
            if w.requests.iter().any(|r| matches!(r, lf2_sim::menu::main_menu::MenuRequest::Message(_))) {
                failures.push(format!("{name}: hp sums differ at frame {}", w.g.match_frames()));
                failed = true;
                break;
            }
            n += 1;
        }
        if !failed && w.g.end_timer() < 0x90 {
            failures.push(format!("{name}: the match did not end ({n} frames, recording has {frames})"));
        }
    }
    assert!(n_files >= 10);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
