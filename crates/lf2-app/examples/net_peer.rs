//! One side of a network game without a window, for testing the WebRTC play between
//! two processes (or computers): `net_peer host` hosts a room and prints its code,
//! `net_peer join CODE` joins it. Both go through the title screens as a player would,
//! then into a VS match from a fixed key script (each side's player 1 joins, Fight!,
//! a fight), and print a digest of every object's position and hp at each packet
//! exchange, so the two outputs can be compared line by line.
//! usage: net_peer host | join CODE   (needs LF2_DIR; LF2_RENDEZVOUS for another server)
use lf2_sim::menu::main_menu::{MenuRequest, MENU_Y, MOUSE_CLICK, MOUSE_X, MOUSE_Y};
use lf2_sim::World;
use std::sync::Arc;
use std::time::{Duration, Instant};

const J: usize = 0x4a;
const W: usize = 0x57;
const A: usize = 0x41;
const D: usize = 0x44;

/// The keys held from exchange `n` on (one exchange is two frames).
fn script(host: bool, n: u32) -> Vec<usize> {
    let mut taps: Vec<(u32, bool, usize)> = vec![(12, true, J)];
    let mut t = 20;
    for _ in 0..4 {
        taps.push((t, true, J));
        taps.push((t, false, J));
        t += 5;
    }
    t += 135;
    taps.push((t, true, J));
    taps.push((t + 6, false, W));
    taps.push((t + 11, false, W));
    taps.push((t + 16, true, J));
    let fight = t + 70;
    let mut keys: Vec<usize> = taps.iter().filter(|&&(at, h, _)| h == host && at <= n && n < at + 2).map(|x| x.2).collect();
    if n >= fight {
        let m = n - fight;
        if m % (if host { 4 } else { 5 }) == 0 {
            keys.push(J);
        }
    }
    keys
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let host = a.get(1).map(|s| s == "host").unwrap_or(true);
    let exchanges: u32 = std::env::var("EXCHANGES").ok().and_then(|s| s.parse().ok()).unwrap_or(400);
    let dir = lf2_data::game_dir_from_env().expect("set LF2_DIR");
    let exe = lf2_app::exe_image::built_in().map(|i| i.to_vec()).unwrap_or_else(|| lf2_data::read_exe(&dir).unwrap());
    let gd = Arc::new(lf2_data::GameData::load(&dir).unwrap());
    let controls = lf2_data::controls::load(&dir).unwrap_or_default();
    let mut w = World::new_title(gd, &exe, &controls, None, if host { 1 } else { 2 });
    w.ads_removed = true;
    let mut net = lf2_app::net::Net::default();
    let mut keys = [b'u'; 256];
    let start = Instant::now();
    let handle = |w: &mut World, net: &mut lf2_app::net::Net| {
        for r in std::mem::take(&mut w.requests) {
            match r {
                MenuRequest::NetHost => net.host(),
                MenuRequest::NetConnect => net.connect(w),
                MenuRequest::NetClose => net.close(),
                MenuRequest::Message(m) => eprintln!("message: {}", m),
                MenuRequest::Quit => {
                    eprintln!("quit");
                    std::process::exit(1)
                }
                _ => {}
            }
        }
    };
    let step = |w: &mut World, keys: &[u8; 256]| {
        w.time_ms = start.elapsed().as_millis() as u32;
        w.set_keys(keys);
        w.step();
    };
    let click = |w: &mut World, net: &mut lf2_app::net::Net, keys: &[u8; 256], x: i32, y: i32| {
        for c in [0, 1, 0] {
            w.g.m.set_i32(MOUSE_X, x);
            w.g.m.set_i32(MOUSE_Y, y);
            w.g.m.set_i32(MOUSE_CLICK, c);
            step(w, keys);
            handle(w, net);
        }
    };
    for _ in 0..5 {
        step(&mut w, &keys);
    }
    let y0 = w.g.m.i32(MENU_Y);
    click(&mut w, &mut net, &keys, 0x150, y0 + 0x100);
    if host {
        click(&mut w, &mut net, &keys, 0x150, 0x118);
    } else {
        click(&mut w, &mut net, &keys, 0x150, 0x138);
        let code = a.get(2).expect("join CODE");
        for (k, b) in code.bytes().enumerate() {
            w.g.m.set_u8(lf2_sim::net::SERVER_IP + k as u32, b);
        }
        w.g.m.set_u8(lf2_sim::net::SERVER_IP + code.len() as u32, 0);
        keys[0x0d] = b'd';
        step(&mut w, &keys);
        keys[0x0d] = b'u';
        handle(&mut w, &mut net);
        step(&mut w, &keys);
        handle(&mut w, &mut net);
    }
    let mut shown_code = false;
    let mut n = 0u32;
    while n < exchanges {
        if start.elapsed() > Duration::from_secs(120) {
            eprintln!("timed out at exchange {}", n);
            std::process::exit(1);
        }
        if let Some(e) = net.take_error() {
            eprintln!("error: {}", e);
            std::process::exit(1);
        }
        if host && !shown_code && w.g.m.u8(lf2_sim::net::LOCAL_IP) != 0 {
            let code: Vec<u8> = (0..8).map(|k| w.g.m.u8(lf2_sim::net::LOCAL_IP + k)).take_while(|&c| c != 0).collect();
            println!("code {}", String::from_utf8_lossy(&code));
            shown_code = true;
        }
        if w.net_waiting {
            match net.poll_exchange(&mut w) {
                Some(p) => {
                    w.net_resume(p.as_deref());
                    handle(&mut w, &mut net);
                    // the digest after the exchange frame
                    let mut h: u64 = 0;
                    for i in 0..400 {
                        if w.exists(i) {
                            let o = &w.objs[i];
                            for v in [o.x().to_bits(), o.y().to_bits(), o.z().to_bits(), o.hp() as u64, o.frame() as u64] {
                                h = h.wrapping_mul(1_000_003).wrapping_add(v ^ i as u64);
                            }
                        }
                    }
                    println!("{} phase {} digest {:016x}", n, w.g.match_phase(), h);
                    n += 1;
                    keys = [b'u'; 256];
                    for k in script(host, n) {
                        keys[k] = b'd';
                    }
                    // in the match: walk to the other player (own player 1 is object 0
                    // on the host, 4 on the joiner)
                    if w.g.match_phase() == 0 && w.exists(0) && w.exists(4) {
                        let (me, them) = if host { (0, 4) } else { (4, 0) };
                        let (dx, dz) = (w.objs[them].ix() - w.objs[me].ix(), w.objs[them].iz() - w.objs[me].iz());
                        if dx > 50 {
                            keys[D] = b'd';
                        } else if dx < -50 {
                            keys[A] = b'd';
                        }
                        if dz > 6 {
                            keys[0x53] = b'd';
                        } else if dz < -6 {
                            keys[W] = b'd';
                        }
                    }
                }
                None => std::thread::sleep(Duration::from_millis(1)),
            }
            continue;
        }
        if net.handshake_busy(&mut w) {
            std::thread::sleep(Duration::from_millis(2));
            continue;
        }
        step(&mut w, &keys);
        handle(&mut w, &mut net);
        if w.net_waiting {
            net.begin_exchange(&mut w);
        } else if w.g.m.i32(lf2_sim::world::GAME) == 0 {
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    let hurt = (0..8).filter(|&i| w.exists(i) && w.objs[i].hp() < w.objs[i].max_hp()).count();
    println!("done: {} fighters hurt", hurt);
    // let the other side finish its last exchange
    std::thread::sleep(Duration::from_secs(2));
}
