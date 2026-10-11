//! One computer of a network game without a window, for testing the WebRTC play
//! between processes (or computers). `net_peer host N` hosts a room, prints its
//! code and starts the game once N computers are in it (itself included);
//! `net_peer join CODE` joins one. All go through the title screens as a player
//! would, then into a VS match from a fixed key script, and print the state hash
//! every rollback::HASH_EVERY frames ("hash FRAME HASH"), so the outputs of the
//! computers can be compared line by line.
//! usage: net_peer host [N] | join CODE   (needs LF2_DIR; LF2_RENDEZVOUS for another
//! server, FRAMES for the number of frames, DRIVER for the seat that goes through
//! the menus, 0 by default)
use lf2_sim::menu::main_menu::{MenuRequest, MENU_Y, MOUSE_CLICK, MOUSE_X, MOUSE_Y};
use lf2_sim::World;
use std::sync::Arc;
use std::time::{Duration, Instant};

const J: usize = 0x4a;
const W: usize = 0x57;
const A: usize = 0x41;
const S: usize = 0x53;
const D: usize = 0x44;
/// "Host a game" and "Join a game" on the network menu.
const HOST: (i32, i32) = (397, 243);
const JOIN: (i32, i32) = (397, 277);

/// The keys held at frame `t` of the game (the script of tests/network.rs, by
/// frames run instead of frames of the clock). The
/// computer in seat `driver` goes through the menus.
fn script(seat: usize, driver: usize, t: u32, w: &World) -> Vec<usize> {
    let mut down = Vec::new();
    let tap = |at: u32| (at..at + 4).contains(&t);
    if seat == driver && tap(40) {
        down.push(J);
    }
    for k in 0..4 {
        if tap(80 + 16 * k + 2 * seat as u32) {
            down.push(J);
        }
    }
    let menu = 80 + 64 + 200;
    if seat == driver {
        for (k, key) in [J, W, W, J].iter().enumerate() {
            if tap(menu + 16 * k as u32) {
                down.push(*key);
            }
        }
    }
    if t > menu + 120 && w.g.match_phase() == 0 && w.exists(seat) {
        let others: Vec<usize> = (0..4).filter(|&s| s != seat && w.exists(s) && w.objs[s].hp() > 0).collect();
        if let Some(&o) = others.iter().min_by_key(|&&o| (w.objs[o].ix() - w.objs[seat].ix()).abs()) {
            let (dx, dz) = (w.objs[o].ix() - w.objs[seat].ix(), w.objs[o].iz() - w.objs[seat].iz());
            if dx > 50 {
                down.push(D);
            } else if dx < -50 {
                down.push(A);
            }
            if dz > 6 {
                down.push(S);
            } else if dz < -6 {
                down.push(W);
            }
        }
        if t % (5 + seat as u32) < 2 {
            down.push(J);
        }
    }
    down
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let host = a.get(1).map(|s| s == "host").unwrap_or(true);
    let want: usize = if host { a.get(2).and_then(|s| s.parse().ok()).unwrap_or(2) } else { 0 };
    let frames: u32 = std::env::var("FRAMES").ok().and_then(|s| s.parse().ok()).unwrap_or(900);
    let driver: usize = std::env::var("DRIVER").ok().and_then(|s| s.parse().ok()).unwrap_or(0);
    let dir = lf2_data::game_dir_from_env().expect("set LF2_DIR");
    let exe = lf2_app::exe_image::built_in().map(|i| i.to_vec()).unwrap_or_else(|| lf2_data::read_exe(&dir).unwrap());
    let gd = Arc::new(lf2_data::GameData::load(&dir).unwrap());
    let controls = lf2_data::controls::load(&dir).unwrap_or_default();
    let mut w = World::new_title(gd, &exe, &controls, None, std::process::id());
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
                MenuRequest::NetStart => net.start(w),
                MenuRequest::Message(m) => {
                    eprintln!("message: {}", m);
                    std::process::exit(1)
                }
                MenuRequest::Quit => {
                    eprintln!("quit");
                    std::process::exit(1)
                }
                _ => {}
            }
        }
    };
    let click = |w: &mut World, net: &mut lf2_app::net::Net, x: i32, y: i32| {
        for c in [0, 1, 0] {
            w.g.m.set_i32(MOUSE_X, x);
            w.g.m.set_i32(MOUSE_Y, y);
            w.g.m.set_i32(MOUSE_CLICK, c);
            w.step();
            handle(w, net);
        }
    };
    for _ in 0..5 {
        w.step();
    }
    let y0 = w.g.m.i32(MENU_Y);
    click(&mut w, &mut net, 0x150, y0 + 0x100);
    if host {
        click(&mut w, &mut net, HOST.0, HOST.1);
    } else {
        click(&mut w, &mut net, JOIN.0, JOIN.1);
        let code = a.get(2).expect("join CODE");
        lf2_sim::menu::net_menu::paste_room_code(&mut w, code);
        keys[0x0d] = b'd';
        w.set_keys(&keys);
        w.step();
        keys[0x0d] = b'u';
        handle(&mut w, &mut net);
    }
    let mut shown_code = false;
    let mut logged = 0;
    let mut next = Instant::now();
    loop {
        if start.elapsed() > Duration::from_secs(150) {
            eprintln!("timed out");
            std::process::exit(1);
        }
        net.pump(&mut w);
        if let Some(e) = w.net_ui.error.take() {
            eprintln!("error: {}", e);
            std::process::exit(1);
        }
        if host && !shown_code && w.g.m.u8(lf2_sim::net::LOCAL_IP) != 0 {
            let code: Vec<u8> = (0..8).map(|k| w.g.m.u8(lf2_sim::net::LOCAL_IP + k)).take_while(|&c| c != 0).collect();
            println!("code {}", String::from_utf8_lossy(&code));
            shown_code = true;
        }
        if net.in_game() {
            let s = net.session().unwrap();
            let rb = s.rollback().unwrap();
            let (seat, frame) = (rb.me(), rb.frame());
            keys = [b'u'; 256];
            if frame < frames {
                for k in script(seat, driver, frame, &w) {
                    keys[k] = b'd';
                }
                net.advance(&mut w, &mut keys);
            } else if s.rollback().unwrap().confirmed() >= frames {
                break;
            } else {
                net.settle(&mut w);
            }
        } else {
            // the host presses Enter once everyone is in
            keys[0x0d] = if host && w.net_ui.players.len() >= want { b'd' } else { b'u' };
            w.set_keys(&keys);
            w.step();
        }
        handle(&mut w, &mut net);
        if let Some(s) = net.session() {
            for (f, h) in &s.log[logged..] {
                println!("hash {} {:016x}", f, h);
            }
            logged = s.log.len();
        }
        next += Duration::from_millis(33);
        if let Some(d) = next.checked_duration_since(Instant::now()) {
            std::thread::sleep(d);
        }
    }
    let rb = net.session().unwrap().rollback().unwrap();
    let hurt = (0..rb.seats()).filter(|&i| w.exists(i) && w.objs[i].hp() < w.objs[i].max_hp()).count();
    println!("done: {} fighters hurt; {} rollbacks, {} frames run again, {} waits", hurt, rb.rollbacks, rb.resimulated, rb.stalls);
    // let the others finish
    for _ in 0..240 {
        net.pump(&mut w);
        net.settle(&mut w);
        std::thread::sleep(Duration::from_millis(33));
    }
}
