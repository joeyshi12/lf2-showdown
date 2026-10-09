//! The demo recordings decode, and encoding then decoding gives the same struct.
#[test]
fn demo_recordings_round_trip() {
    let Some(dir) = lf2_data::game_dir_from_env() else { return };
    let exe = std::fs::read(dir.join("lf2.exe")).unwrap();
    let mut n = 0;
    for e in std::fs::read_dir(dir.join("recording")).unwrap() {
        let p = e.unwrap().path();
        if p.extension().map_or(true, |x| x != "lfr") {
            continue;
        }
        let rec = lf2_data::lfr::decode(&std::fs::read(&p).unwrap(), &exe).unwrap();
        let mode = i32::from_le_bytes(rec[0x148..0x14c].try_into().unwrap());
        assert!((0..5).contains(&mode), "{}: mode {}", p.display(), mode);
        let again = lf2_data::lfr::decode(&lf2_data::lfr::encode(&rec, &exe), &exe).unwrap();
        assert!(again == rec, "{}", p.display());
        n += 1;
    }
    assert!(n >= 7);
}
