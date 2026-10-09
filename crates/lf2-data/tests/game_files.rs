//! Checks against the files of an installed game. Set `LF2_DIR` to the folder that
//! holds `lf2.exe`; without it these tests print a note and pass.

use lf2_data::*;
use std::path::PathBuf;

fn dir() -> Option<PathBuf> {
    let d = game_dir_from_env();
    if d.is_none() {
        eprintln!("LF2_DIR not set; skipping");
    }
    d
}

fn dat_files(d: &std::path::Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(d.join("data"))
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().map_or(false, |x| x.eq_ignore_ascii_case("dat")))
        .collect();
    for e in std::fs::read_dir(d.join("bg")).unwrap().flatten() {
        for e2 in std::fs::read_dir(e.path()).unwrap().flatten() {
            let p = e2.path().join("bg.dat");
            if p.exists() {
                v.push(p);
            }
        }
    }
    v.sort();
    v
}

#[test]
fn every_dat_file_reencodes_to_the_same_bytes() {
    let Some(d) = dir() else { return };
    let files = dat_files(&d);
    assert!(files.len() > 70, "found only {} .dat files", files.len());
    for p in files {
        let raw = std::fs::read(&p).unwrap();
        let plain = crypt::decode_file(&raw);
        let header: [u8; crypt::HEADER_LEN] = raw[..crypt::HEADER_LEN].try_into().unwrap();
        assert_eq!(crypt::encode_file(&header, &plain), raw, "{}", p.display());
    }
}

#[test]
fn every_object_parses_and_round_trips() {
    let Some(d) = dir() else { return };
    let gd = GameData::load(&d).unwrap();
    assert_eq!(gd.objects.len(), gd.manifest.objects.len());
    for o in &gd.objects {
        let text = write::write_object(o).unwrap_or_else(|e| panic!("id {}: {}", o.id, e));
        let (o2, _) = object::parse_object(text.as_bytes(), o.id, o.kind);
        assert_eq!(&o2, o, "id {} does not round-trip", o.id);
        assert!(o.frames.iter().any(|f| f.exists), "id {} has no frames", o.id);
    }
}

#[test]
fn every_background_and_stage_round_trips() {
    let Some(d) = dir() else { return };
    let gd = GameData::load(&d).unwrap();
    assert!(!gd.backgrounds.is_empty());
    for b in &gd.backgrounds {
        let (b2, _) = bg::parse_background(write::write_background(b).as_bytes(), b.id);
        assert_eq!(&b2, b, "background {}", b.id);
        assert!(!b.layers.is_empty(), "background {} has no layers", b.id);
    }
    assert!(!gd.stages.is_empty());
    let st2 = stage::parse_stages(write::write_stages(&gd.stages).as_bytes());
    assert_eq!(st2, gd.stages);
}

#[test]
fn manifest_lists_objects_and_backgrounds() {
    let Some(d) = dir() else { return };
    let gd = GameData::load(&d).unwrap();
    let davis = gd.objects.iter().find(|o| o.name == b"Davis").expect("Davis");
    let entry = gd.manifest.objects.iter().find(|e| e.file.eq_ignore_ascii_case(b"data\\davis.dat")).unwrap();
    assert_eq!(davis.id, entry.id);
    assert_eq!(gd.backgrounds.len(), gd.manifest.backgrounds.len());
}

#[test]
fn every_bitmap_decodes() {
    let Some(d) = dir() else { return };
    let mut n = 0;
    let mut stack = vec![d.join("sprite"), d.join("bg")];
    while let Some(p) = stack.pop() {
        for e in std::fs::read_dir(&p).unwrap().flatten() {
            let q = e.path();
            if q.is_dir() {
                stack.push(q);
            } else if q.extension().map_or(false, |x| x.eq_ignore_ascii_case("bmp")) {
                let im = image::decode_bmp(&std::fs::read(&q).unwrap()).unwrap_or_else(|| panic!("{}", q.display()));
                assert!(im.w > 0 && im.h > 0, "{}", q.display());
                n += 1;
            }
        }
    }
    assert!(n > 400, "only {} bitmaps", n);
    let exe = std::fs::read(d.join("lf2.exe")).unwrap();
    let res = image::pe_bitmaps(&exe);
    for name in ["FRAME", "WORDS0", "BARS", "SPARK", "SHADOW1", "PAUSE"] {
        let (_, dib) = res.iter().find(|(n, _)| n == name).unwrap_or_else(|| panic!("resource {}", name));
        let im = image::decode_dib(dib, None).unwrap();
        assert!(im.w > 0, "{}", name);
    }
}
