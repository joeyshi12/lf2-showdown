//! Print a few fields of objects 0 and 1 for each frame of a trace.
//! usage: cargo run --example show_trace -- TRACE.lf2t [every]
//! OBJS=0,11 picks the objects; GLOBALS=44fb6c,450b94 adds int globals (hex).
use lf2_sim::trace::Trace;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let dir = lf2_data::game_dir_from_env().expect("set LF2_DIR");
    let exe = std::fs::read(dir.join("lf2.exe")).unwrap();
    let t = Trace::load(std::path::Path::new(&args[1]), &exe).unwrap();
    let every: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(1);
    for (n, f) in t.frames.iter().enumerate().step_by(every) {
        let mut line = format!("{:4}", n);
        let ids: Vec<usize> = std::env::var("OBJS")
            .ok()
            .map(|v| v.split(',').filter_map(|x| x.parse().ok()).collect())
            .unwrap_or(vec![0, 1]);
        if let Ok(g) = std::env::var("GLOBALS") {
            for a in g.split(',').filter_map(|x| u32::from_str_radix(x, 16).ok()) {
                line += &format!(" {:x}={}", a, f.state.globals.m.i32(a));
            }
        }
        for &i in &ids {
            if let Some(o) = &f.state.objects[i] {
                line += &format!(
                    " | o{} x{:7.2} z{:6.1} y{:6.1} f{:3} hp{:4} b0 {:3} b8 {:3} b4 {:3} sparks {} u08 {}",
                    i, o.x(), o.z(), o.y(), o.frame(), o.hp(), o.m.i32(0xb0), o.m.i32(0xb8), o.m.i32(0xb4), o.m.i32(0x36c), o.u_08()
                );
            }
        }
        println!("{}", line);
    }
}
