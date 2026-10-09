//! Render the state saved by tools/harness/capture.py and compare it with the
//! original's picture. usage: compare_picture PREFIX  (reads PREFIX.lf2t, PREFIX.ppm;
//! writes PREFIX.ours.ppm and PREFIX.diff.ppm)
use lf2_app::{assets::Assets, render};
use std::sync::Arc;

fn read_ppm(p: &str) -> (usize, usize, Vec<u32>) {
    let b = std::fs::read(p).unwrap();
    let mut fields = Vec::new();
    let mut i = 0;
    while fields.len() < 4 {
        while b[i].is_ascii_whitespace() {
            i += 1;
        }
        let s = i;
        while !b[i].is_ascii_whitespace() {
            i += 1;
        }
        fields.push(String::from_utf8_lossy(&b[s..i]).to_string());
    }
    i += 1;
    let (w, h): (usize, usize) = (fields[1].parse().unwrap(), fields[2].parse().unwrap());
    let px = b[i..].chunks(3).take(w * h).map(|c| (c[0] as u32) << 16 | (c[1] as u32) << 8 | c[2] as u32).collect();
    (w, h, px)
}

fn write_ppm(p: &str, w: usize, h: usize, px: &[u32]) {
    let mut out = format!("P6\n{} {}\n255\n", w, h).into_bytes();
    for v in px {
        out.extend_from_slice(&[(v >> 16) as u8, (v >> 8) as u8, *v as u8]);
    }
    std::fs::write(p, out).unwrap();
}

fn main() {
    let prefix = std::env::args().nth(1).unwrap();
    let dir = lf2_data::game_dir_from_env().unwrap();
    let exe = std::fs::read(dir.join("lf2.exe")).unwrap();
    let gd = Arc::new(lf2_data::GameData::load(&dir).unwrap());
    let t = lf2_sim::trace::Trace::load(std::path::Path::new(&format!("{}.lf2t", prefix)), &exe).unwrap();
    let mut w = lf2_sim::World::from_snapshot(&t.frames[0].state, gd.clone());
    w.apply_pokes(&t.frames[0].pokes);
    let a = Assets::load(&gd, &exe);
    let mut r = render::Renderer::new(&gd);
    if let Ok(t) = std::fs::read_to_string(format!("{}.layers", prefix)) {
        // the counters advance once per draw, so start one step behind
        let bi = w.g.background() as usize;
        for (i, v) in t.split_whitespace().enumerate() {
            let cc = gd.backgrounds[bi].layers[i].cc;
            let v: i32 = v.parse().unwrap();
            r.layer_counters[bi][i] = if cc > 0 { (v - 1).rem_euclid(cc) } else { v };
        }
    }
    let mut f = render::Frame::new();
    if w.g.match_phase() != 0 {
        // a menu: run the frame the picture shows
        w.set_keys(&t.frames[0].keys);
        w.step();
        f.draw_list(&a, &w.draw);
    } else {
        r.draw(&w, &a, &mut f);
    }
    let (gw, gh, gpx) = read_ppm(&format!("{}.ppm", prefix));
    // the window capture may include a border: find the 794x550 client area offset
    let (ox, oy) = ((gw - 794) / 2, gh - 550 - (gw - 794) / 2);
    let mut diff = vec![0u32; 794 * 550];
    let mut n = 0;
    for y in 0..550 {
        for x in 0..794 {
            let g = gpx[(y + oy) * gw + x + ox];
            let o = f.px[y * 794 + x];
            if g != o {
                n += 1;
                diff[y * 794 + x] = 0xff0000;
            } else {
                diff[y * 794 + x] = (o >> 2) & 0x3f3f3f;
            }
        }
    }
    println!("capture {}x{}, client offset ({}, {}); {} of {} pixels differ ({:.2}%)", gw, gh, ox, oy, n, 794 * 550, 100.0 * n as f64 / (794.0 * 550.0));
    write_ppm(&format!("{}.ours.ppm", prefix), 794, 550, &f.px);
    write_ppm(&format!("{}.diff.ppm", prefix), 794, 550, &diff);
}
