//! Compare the loaders with a dump of the running original's memory, written by
//! `tools/harness/dump_data.py`. Set `LF2_DIR` and `LF2_MEMDUMP`; without them the test
//! prints a note and passes.

use lf2_data::*;

fn hexdiff(name: &str, offs: &[usize], ours: &[u8], theirs: &[u8]) -> String {
    let mut s = format!("{}: {} bytes differ; first:", name, offs.len());
    for &o in offs.iter().take(12) {
        s += &format!(" @{:#x} ours {:02x} game {:02x};", o, ours[o], theirs.get(o).copied().unwrap_or(0));
    }
    s
}

#[test]
fn loaded_data_matches_game_memory() {
    let (Some(dir), Some(dump)) = (game_dir_from_env(), std::env::var_os("LF2_MEMDUMP")) else {
        eprintln!("LF2_DIR or LF2_MEMDUMP not set; skipping");
        return;
    };
    let dump = std::path::PathBuf::from(dump);
    let gd = GameData::load(&dir).unwrap();
    let mut failures = Vec::new();
    for (i, o) in gd.objects.iter().enumerate() {
        let theirs = std::fs::read(dump.join(format!("obj_{:03}.bin", i))).unwrap();
        let im = layout::object_image(o);
        assert_eq!(im.bytes.len(), theirs.len(), "object {} (id {}) size, itr/bdy counts differ", i, o.id);
        let d = im.diff(&theirs);
        if !d.is_empty() {
            failures.push(hexdiff(&format!("object {} id {}", i, o.id), &d, &im.bytes, &theirs));
        }
    }
    let bgs = std::fs::read(dump.join("backgrounds.bin")).unwrap();
    for (i, b) in gd.backgrounds.iter().enumerate() {
        let theirs = &bgs[i * layout::BG_SIZE..(i + 1) * layout::BG_SIZE];
        let im = layout::background_image(b);
        let d = im.diff(theirs);
        if !d.is_empty() {
            failures.push(hexdiff(&format!("background {}", i), &d, &im.bytes, theirs));
        }
    }
    let stages = std::fs::read(dump.join("stages.bin")).unwrap();
    assert!(!gd.stages.is_empty());
    for st in &gd.stages {
        let at = st.id as usize * layout::STAGE_SIZE;
        let theirs = &stages[at..at + layout::STAGE_SIZE];
        let im = layout::stage_image(st);
        let d = im.diff(theirs);
        if !d.is_empty() {
            failures.push(hexdiff(&format!("stage {}", st.id), &d, &im.bytes, theirs));
        }
    }
    let globals = std::fs::read(dump.join("globals.bin")).unwrap();
    let n = i32::from_le_bytes(globals[4..8].try_into().unwrap()) as usize;
    let table: Vec<Vec<u8>> = (0..n)
        .map(|i| {
            let e = &globals[8 + i * 0x14..8 + (i + 1) * 0x14];
            e[..e.iter().position(|&c| c == 0).unwrap_or(0x14)].to_vec()
        })
        .collect();
    if table != gd.sounds {
        failures.push(format!(
            "sound table: game has {} entries, ours {}; first difference at {:?}",
            table.len(),
            gd.sounds.len(),
            (0..table.len().min(gd.sounds.len())).find(|&i| table[i] != gd.sounds[i])
        ));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
