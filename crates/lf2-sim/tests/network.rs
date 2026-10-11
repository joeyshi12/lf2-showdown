//! Network games between worlds in one process, with the messages passed through a
//! simulated network that delays, reorders and drops them. Two to four computers go
//! from the title screens through the main menu and the pick screen into a VS
//! match and fight. Then:
//! - no computer reports a synchronisation error (the state hashes agree);
//! - every computer's world, once all inputs are in, is the same as a world that
//!   ran the same inputs once, without rollback, from the host's state at the start.
use lf2_sim::menu::main_menu::{MenuRequest, MOUSE_CLICK, MOUSE_X, MOUSE_Y};
use lf2_sim::net::NetSession;
use lf2_sim::rollback::Input;
use lf2_sim::world::{GAME, KEYS};
use lf2_sim::World;
use std::sync::Arc;

const J: usize = 0x4a;
const K: usize = 0x4b;
const W: usize = 0x57;
const A: usize = 0x41;
const S: usize = 0x53;
const D: usize = 0x44;

struct Profile {
    computers: usize,
    latency_ms: u64,
    jitter_ms: u64,
    /// Of every 100 messages on the unreliable channel (INPUT, PING, PONG), this many
    /// are lost. The others arrive in order, as on the reliable channel.
    loss: u64,
}

struct Peer {
    w: World,
    s: NetSession,
    keys: [u8; 256],
    /// Frames of the platform's clock since the game started.
    ticks: u32,
}

/// A message on its way: arrival time, sender (peer index), receiver, bytes.
struct InFlight {
    at: u64,
    from: usize,
    to: usize,
    m: Vec<u8>,
}

struct Lan {
    p: Profile,
    peers: Vec<Peer>,
    flight: Vec<InFlight>,
    /// The last arrival time on each (from, to) link of the messages that keep
    /// their order.
    ordered: Vec<Vec<u64>>,
    now: u64,
    rng: u64,
    /// The host's state when the game started, for the run without rollback.
    start: Option<(lf2_sim::rollback::State, Vec<[u8; 11]>, Vec<u8>)>,
}

impl Lan {
    fn rand(&mut self, n: u64) -> u64 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        if n == 0 { 0 } else { self.rng % n }
    }

    /// The peer id a computer uses for another: the host numbers its joiners
    /// 1..3, a joiner calls the host 0.
    fn peer_id(&self, me: usize, other: usize) -> u32 {
        if me == 0 { other as u32 } else { 0 }
    }

    fn send_all(&mut self, from: usize) {
        for (id, m) in self.peers[from].s.take_outgoing() {
            let to = if from == 0 { id as usize } else { 0 };
            let fast = lf2_sim::net::is_unreliable(&m);
            if fast && self.rand(100) < self.p.loss {
                continue;
            }
            let mut at = self.now + self.p.latency_ms + self.rand(self.p.jitter_ms + 1);
            if !fast {
                at = at.max(self.ordered[from][to]);
                self.ordered[from][to] = at;
            }
            self.flight.push(InFlight { at, from, to, m });
        }
    }

    fn deliver(&mut self) {
        let mut k = 0;
        while k < self.flight.len() {
            if self.flight[k].at <= self.now {
                let f = self.flight.remove(k);
                let id = self.peer_id(f.to, f.from);
                let p = &mut self.peers[f.to];
                p.s.on_message(&mut p.w, id, &f.m);
            } else {
                k += 1;
            }
        }
    }

    /// One 33 ms frame of every computer's clock.
    fn tick(&mut self) {
        self.now += 33;
        self.deliver();
        for i in 0..self.peers.len() {
            let p = &mut self.peers[i];
            if p.s.in_game() {
                script(i, p);
                p.s.advance(&mut p.w, &mut p.keys);
                p.ticks += 1;
            } else {
                p.w.set_keys(&p.keys);
                p.w.step();
                for k in 0..256u32 {
                    p.keys[k as usize] = p.w.g.m.u8(KEYS + k);
                }
            }
            let p = &mut self.peers[i];
            for r in std::mem::take(&mut p.w.requests) {
                match r {
                    MenuRequest::NetStart => {
                        let (state, table) = (p.w.save_state(), p.w.net_table());
                        p.s.start(&mut p.w);
                        let names = (0..self_count(&p.w)).map(|s| name_at(&p.w, s)).collect();
                        self.start = Some((state, names, table));
                    }
                    MenuRequest::Message(m) => panic!("computer {}: message {:?} at frame {:?}", i, m, p.s.rollback().map(|r| r.frame())),
                    MenuRequest::Quit => panic!("computer {} quit", i),
                    _ => {}
                }
            }
            if let Some(e) = self.peers[i].w.net_ui.error.take() {
                panic!("computer {}: {}", i, e);
            }
            self.send_all(i);
        }
    }

    fn click(w: &mut World, x: i32, y: i32) {
        for c in [0, 1, 0] {
            w.g.m.set_i32(MOUSE_X, x);
            w.g.m.set_i32(MOUSE_Y, y);
            w.g.m.set_i32(MOUSE_CLICK, c);
            w.step();
        }
    }
}

fn self_count(w: &World) -> usize {
    (0..8u32).filter(|&s| w.g.m.i32(lf2_sim::world::PLAYER_OF_SLOT + 4 * s) == -1).count()
}

fn name_at(w: &World, s: usize) -> [u8; 11] {
    std::array::from_fn(|k| w.g.m.u8(0x44fcc0 + 11 * s as u32 + k as u32))
}

/// The keys computer `i` holds, by its own frame count since the game started:
/// through the menus with taps far enough apart for the slowest profile, then a
/// fight in which each player walks to the nearest other one and attacks.
fn script(i: usize, p: &mut Peer) {
    let t = p.ticks;
    let mut down: Vec<usize> = Vec::new();
    let tap = |at: u32| (at..at + 4).contains(&t);
    if i == 0 && tap(40) {
        down.push(J); // VS mode
    }
    for k in 0..4 {
        if tap(80 + 16 * k + 2 * i as u32) {
            down.push(J); // join, fighter, team, ready
        }
    }
    let menu = 80 + 64 + 200;
    if i == 0 {
        for (k, key) in [J, W, W, J].iter().enumerate() {
            if tap(menu + 16 * k as u32) {
                down.push(*key); // no computer players, up twice to Fight!, go
            }
        }
    }
    let w = &p.w;
    let me = p.s.rollback().map_or(i, |r| r.me());
    if t > menu + 120 && w.g.match_phase() == 0 && w.exists(me) {
        let others: Vec<usize> = (0..4).filter(|&s| s != me && w.exists(s) && w.objs[s].hp() > 0).collect();
        if let Some(&o) = others.iter().min_by_key(|&&o| (w.objs[o].ix() - w.objs[me].ix()).abs()) {
            let (dx, dz) = (w.objs[o].ix() - w.objs[me].ix(), w.objs[o].iz() - w.objs[me].iz());
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
        if t % (5 + i as u32) < 2 {
            down.push(J);
        }
        if t % (41 + 3 * i as u32) < 2 {
            down.push(K);
        }
    }
    p.keys = [b'u'; 256];
    for k in down {
        p.keys[k] = b'd';
    }
}

fn run(p: Profile) {
    let Some(dir) = lf2_data::game_dir_from_env() else { return };
    let exe = lf2_data::read_exe(&dir).unwrap();
    let gd = Arc::new(lf2_data::GameData::load(&dir).unwrap());
    let controls = lf2_data::controls::load(&dir).unwrap_or_default();
    let n = p.computers;
    let mut peers = Vec::new();
    for i in 0..n {
        let mut w = World::new_title(gd.clone(), &exe, &controls, None, 11 + 7 * i as u32);
        w.ads_removed = true;
        for (k, b) in format!("p{}", i).bytes().chain([0]).enumerate() {
            w.g.m.set_u8(0x44fcc0 + k as u32, b);
        }
        let s = if i == 0 { NetSession::host() } else { NetSession::join() };
        peers.push(Peer { w, s, keys: [b'u'; 256], ticks: 0 });
    }
    let mut lan = Lan { ordered: vec![vec![0; n]; n], p, peers, flight: Vec::new(), now: 0, rng: 0x9e3779b97f4a7c15, start: None };
    // the title screens: "network game", then server or client
    for (i, peer) in lan.peers.iter_mut().enumerate() {
        for _ in 0..5 {
            peer.w.step();
        }
        let y0 = peer.w.g.m.i32(lf2_sim::menu::main_menu::MENU_Y);
        Lan::click(&mut peer.w, 0x150, y0 + 0x100);
        // "Host a game" or "Join a game"
        Lan::click(&mut peer.w, 397, if i == 0 { 243 } else { 277 });
        peer.w.requests.clear();
    }
    let host = &mut lan.peers[0];
    host.s.on_code(&mut host.w, "AB2C");
    for i in 1..n {
        let p = &mut lan.peers[i];
        p.s.on_connected(&mut p.w, 0);
        lan.send_all(i);
    }
    for _ in 0..30 {
        lan.tick();
    }
    assert_eq!(lan.peers[0].w.net_ui.players.len(), n, "the host lists every computer");
    // the host presses Enter
    lan.peers[0].keys[0x0d] = b'd';
    for _ in 0..3 {
        lan.tick();
    }
    lan.peers[0].keys[0x0d] = b'u';
    for _ in 0..30 {
        lan.tick();
    }
    for (i, peer) in lan.peers.iter().enumerate() {
        assert!(peer.s.in_game(), "computer {} is in the game", i);
        assert!(peer.w.g.m.i32(GAME) >= 1);
    }
    // the menus and a fight
    let mut fought = false;
    for _ in 0..1500 {
        lan.tick();
        let w = &lan.peers[0].w;
        fought |= w.g.match_phase() == 0 && (0..n).all(|s| w.exists(s));
    }
    assert!(fought, "the match ran with every computer's fighter");
    let readout = lan.peers[1].w.draw.iter().any(|d| matches!(d, lf2_sim::draw::Draw::Label { text, .. } if String::from_utf8_lossy(text).contains("ms  rollback")));
    assert!(readout, "the game shows the round-trip time and rollback readout");
    // stop pressing keys and let every computer's inputs arrive
    let target = lan.peers.iter().map(|p| p.s.rollback().unwrap().frame()).max().unwrap() + 4;
    for _ in 0..200 {
        lan.now += 33;
        lan.deliver();
        for i in 0..n {
            let p = &mut lan.peers[i];
            p.keys = [b'u'; 256];
            if p.s.rollback().unwrap().frame() < target {
                p.s.advance(&mut p.w, &mut p.keys);
            } else {
                p.s.settle(&mut p.w);
            }
            p.w.requests.retain(|r| !matches!(r, MenuRequest::Message(_)) || panic!("computer {}: {:?}", i, r));
            lan.send_all(i);
        }
    }
    let hurt = (0..n).filter(|&s| {
        let w = &lan.peers[0].w;
        w.exists(s) && w.objs[s].hp() < w.objs[s].max_hp()
    });
    assert!(hurt.count() >= 1, "the fighters hit each other");
    for (i, peer) in lan.peers.iter().enumerate() {
        let rb = peer.s.rollback().unwrap();
        assert_eq!(rb.frame(), target, "computer {} reached the frame", i);
        assert!(rb.confirmed() >= target, "computer {} has every input", i);
    }
    // the same inputs once, without rollback, from the host's state at the start
    let (state, names, table) = lan.start.take().unwrap();
    let rb0 = lan.peers[0].s.rollback().unwrap();
    let inputs: Vec<Vec<Input>> = (0..n).map(|s| (0..target).map(|f| rb0.input(s, f)).collect()).collect();
    let mut r = World::new_title(gd.clone(), &exe, &controls, None, 1);
    r.ads_removed = true;
    r.load_state(&state);
    r.net_start(&names, 0, &table);
    let mut prev = vec![[0u8; 3]; n];
    for f in 0..target {
        let now: Vec<Input> = (0..n).map(|s| inputs[s][f as usize]).collect();
        r.net_step(&now, &prev, f);
        prev = now;
    }
    for (i, peer) in lan.peers.iter().enumerate() {
        for o in 0..400 {
            assert_eq!(peer.w.exists(o), r.exists(o), "computer {}: object {} exists", i, o);
            if r.exists(o) {
                assert!(peer.w.objs[o].m.bytes == r.objs[o].m.bytes, "computer {}: object {} differs from the run without rollback", i, o);
            }
        }
        assert_eq!(peer.w.state_hash(), r.state_hash(), "computer {}", i);
        let rb = peer.s.rollback().unwrap();
        eprintln!("computer {}: {} frames, {} rollbacks, {} frames run again, {} waits", i, rb.frame(), rb.rollbacks, rb.resimulated, rb.stalls);
    }
}

#[test]
fn two_computers_without_delay() {
    run(Profile { computers: 2, latency_ms: 0, jitter_ms: 0, loss: 0 });
}

#[test]
fn two_computers_slow_connection() {
    run(Profile { computers: 2, latency_ms: 80, jitter_ms: 40, loss: 5 });
}

#[test]
fn four_computers_with_loss() {
    run(Profile { computers: 4, latency_ms: 60, jitter_ms: 50, loss: 10 });
}

/// The room code screen: capitals without Shift, four characters at most, nothing
/// typed while Ctrl is held, pasting, Backspace, and Enter joins once four are in.
#[test]
fn room_code_entry() {
    let Some(dir) = lf2_data::game_dir_from_env() else { return };
    let exe = lf2_data::read_exe(&dir).unwrap();
    let gd = Arc::new(lf2_data::GameData::load(&dir).unwrap());
    let controls = lf2_data::controls::load(&dir).unwrap_or_default();
    let mut w = World::new_title(gd, &exe, &controls, None, 3);
    w.ads_removed = true;
    for _ in 0..5 {
        w.step();
    }
    let y0 = w.g.m.i32(lf2_sim::menu::main_menu::MENU_Y);
    Lan::click(&mut w, 0x150, y0 + 0x100);
    Lan::click(&mut w, 397, 277);
    w.requests.clear();
    let press = |w: &mut World, vks: &[usize]| {
        let mut keys = [b'u'; 256];
        for &k in vks {
            keys[k] = b'd';
        }
        w.set_keys(&keys);
        w.step();
        w.set_keys(&[b'u'; 256]);
        w.step();
    };
    press(&mut w, &[A]);
    press(&mut w, &[0x32]);
    assert_eq!(w.room_code_typed(), "A2");
    press(&mut w, &[0x11, 0x56]);
    assert_eq!(w.room_code_typed(), "A2", "Ctrl+V types nothing");
    lf2_sim::menu::net_menu::paste_room_code(&mut w, " b-c d ");
    assert_eq!(w.room_code_typed(), "A2BC", "pasted in capitals, four at most");
    press(&mut w, &[0x08]);
    assert_eq!(w.room_code_typed(), "A2B");
    press(&mut w, &[0x0d]);
    assert!(!w.requests.contains(&MenuRequest::NetConnect), "Enter does nothing with three characters");
    press(&mut w, &[0x5a]);
    press(&mut w, &[0x0d]);
    assert!(w.requests.contains(&MenuRequest::NetConnect), "Enter joins");
    assert_eq!(w.room_code_typed(), "A2BZ");
}
