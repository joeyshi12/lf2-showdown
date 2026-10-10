//! Sound events belong to one frame: a frame that ends before `flush_sounds` (here,
//! paused with F1) reports none, instead of the previous frame's sounds again.
use lf2_sim::setup::{Fighter, VsSetup, DEFAULT_CONTROLS};
use std::sync::Arc;

#[test]
fn paused_frames_report_no_sounds() {
    let Some(dir) = lf2_data::game_dir_from_env() else { return };
    let exe = lf2_data::read_exe(&dir).unwrap();
    let gd = Arc::new(lf2_data::GameData::load(&dir).unwrap());
    let davis = gd.objects.iter().position(|o| o.kind == 0 && o.name == b"Davis").unwrap();
    let f = |slot, x| Fighter { slot, data: davis, team: 0, human: true, pos: Some((x, 400)) };
    let setup = VsSetup { fighters: vec![f(0, 300), f(1, 340)], background: 0, difficulty: 2, seed: 1 };
    let mut w = lf2_sim::World::new_vs_match(gd, &exe, &setup, &DEFAULT_CONTROLS);
    let attack = DEFAULT_CONTROLS[0][4] as usize;
    let mut keys = [b'u'; 256];
    // punch the other player until a frame has sounds
    let mut n = 0;
    while w.sounds.is_empty() {
        keys[attack] = if n % 4 < 2 { b'd' } else { b'u' };
        w.set_keys(&keys);
        w.step();
        n += 1;
        assert!(n < 300, "no sound while punching");
    }
    // pause straight after it (F1 sets 0x44fb60, which becomes 0x450bfc)
    keys = [b'u'; 256];
    w.g.m.set_i32(0x44fb60, 1);
    w.g.m.set_i32(0x44fcb0, 1);
    for f in 0..10 {
        w.set_keys(&keys);
        w.step();
        assert_eq!(w.g.m.i32(0x450bfc), 1, "not paused");
        assert!(w.sounds.is_empty(), "paused frame {f} reported sounds");
    }
}
