//! Four-computer rollback netplay. Computer k plays slot k; joiners connect only to
//! the host, which forwards inputs. The original's packet locations remain the input
//! boundary (`IN` at 0x44f198; `read_remote_inputs` at 0x4198f0).
//!
//! Room messages use a reliable channel. INPUT, PING and PONG use an unordered
//! channel without retransmits; INPUT repeats unacknowledged inputs, so losing one
//! costs nothing. A reliable channel would hold later messages until it resent the
//! loss. Joining checks the protocol and data; confirmed-state hashes replace the
//! original packet's counter, HP and checksum checks.

use crate::menu::main_menu::MenuRequest;
use crate::rollback::{Input, Rollback, MAX_SEATS};
use crate::world::{World, PLAYER_OF_SLOT};

/// `DAT_0044f1af`: 0 no network game, 1 joiner, 2 host.
pub const ROLE: u32 = 0x44f1af;
/// `DAT_0044f1ae`: 1 once the network game has started.
pub const JOINED: u32 = 0x44f1ae;
/// The packet the original sends (`DAT_0044d040`); match_frame still resets it every
/// frame, as the original does.
pub const OUT: u32 = 0x44d040;
/// The packet received from the other computer (`DAT_0044f198`).
pub const IN: u32 = 0x44f198;
pub const PACKET: usize = 0x16;
/// The local address shown on the network screens (`DAT_0044f340`, 200 bytes).
pub const LOCAL_IP: u32 = 0x44f340;
/// The text typed on the client screen (game object + 0x7d8): the room code.
pub const SERVER_IP: u32 = 0x4592d8;
/// The random table the host sends: 3000 bytes at 0x44ff90 and the byte after it.
pub const TABLE: usize = 0xbb9;
const TABLE_AT: u32 = 0x44ff90;
const CHECKSUM: u32 = 0x44f620;
const NAMES: u32 = 0x44fcc0;
/// The version of the messages below. Computers with different versions refuse
/// each other.
pub const PROTOCOL: u16 = 3;

/// The F-keys in the packet, in the order the original applies them: key, packet
/// byte (10 or 12), bit.
pub const FKEY_BITS: [(u32, usize, u8); 11] = [
    (0x70, 10, 0x02),
    (0x71, 10, 0x40),
    (0x73, 10, 0x04),
    (0x74, 10, 0x08),
    (0x72, 12, 0x02),
    (0xf9, 12, 0x08),
    (0xf8, 12, 0x10),
    (0x75, 10, 0x10),
    (0x76, 10, 0x20),
    (0x77, 12, 0x04),
    (0x78, 10, 0x80),
];

#[derive(Clone, Debug, Default, PartialEq)]
pub struct NetPlayer {
    pub name: String,
    /// The round-trip time to it in milliseconds, where this computer measures it
    /// (the host to each joiner, a joiner to the host).
    pub ping: Option<u32>,
    pub me: bool,
}

#[derive(Clone, Debug, Default)]
pub struct NetUi {
    pub players: Vec<NetPlayer>,
    pub error: Option<String>,
    pub hosting: bool,
    pub copied_at: Option<u32>,
}

/// The name a computer is listed with: its player 1 name from control settings,
/// or "Player N" when that is empty or only the default digit.
fn display_name(raw: &[u8; 11], seat: usize) -> [u8; 11] {
    let n = name_string(raw);
    if !n.is_empty() && !n.bytes().all(|c| c.is_ascii_digit()) {
        return *raw;
    }
    let mut out = [0u8; 11];
    for (k, b) in format!("Player {}", seat + 1).bytes().take(10).enumerate() {
        out[k] = b;
    }
    out
}

pub fn set_room_code(w: &mut World, code: &str) {
    for k in 0..200 {
        w.g.m.set_u8(LOCAL_IP + k, 0);
    }
    for (k, b) in code.bytes().take(199).enumerate() {
        w.g.m.set_u8(LOCAL_IP + k as u32, b);
    }
}

impl World {
    pub fn net_role(&self) -> i8 {
        self.g.m.i8(ROLE)
    }

    pub fn room_code_typed(&self) -> String {
        let b: Vec<u8> = (0..0x33).map(|k| self.g.m.u8(SERVER_IP + k)).take_while(|&c| c != 0).collect();
        String::from_utf8_lossy(&b).into_owned()
    }

    pub fn net_local_name(&self) -> [u8; 11] {
        let mut n = [0u8; 11];
        for (k, b) in n.iter_mut().take(10).enumerate() {
            *b = self.g.m.u8(NAMES + k as u32);
            if *b == 0 {
                break;
            }
        }
        n
    }

    /// The data checksum the original compares in its packet.
    pub fn net_data_checksum(&self) -> i32 {
        self.g.m.i32(CHECKSUM)
    }

    /// What the host sends when the game starts besides the names: the random
    /// table, and the seed of the C library's rand() (the pick screen's random
    /// fighters use it), which the original does not send.
    pub fn net_table(&self) -> Vec<u8> {
        let mut t: Vec<u8> = (0..TABLE as u32).map(|k| self.g.m.u8(TABLE_AT + k)).collect();
        t.extend_from_slice(&self.g.crt_seed().to_le_bytes());
        t
    }

    /// Start the network game: `names[k]` plays slot `k`, this computer is `me`
    /// (0 the host), `table` is the host's `net_table`. As the original after its
    /// handshake: the role, the random table, the names, the slots of the players,
    /// then the "loading" frame.
    pub fn net_start(&mut self, names: &[[u8; 11]], me: usize, table: &[u8]) {
        self.g.m.set_u8(ROLE, if me == 0 { 2 } else { 1 });
        self.g.m.set_u8(JOINED, 1);
        for s in 0..8u32 {
            let v = if (s as usize) < names.len() { -1 } else { 0 };
            self.g.m.set_i32(PLAYER_OF_SLOT + 4 * s, v);
        }
        for (s, n) in names.iter().enumerate() {
            for k in 0..11u32 {
                self.g.m.set_u8(NAMES + 11 * s as u32 + k, n[k as usize]);
            }
        }
        for (k, &b) in table.iter().take(TABLE).enumerate() {
            self.g.m.set_u8(TABLE_AT + k as u32, b);
        }
        if let Some(seed) = table.get(TABLE..TABLE + 4) {
            self.g.set_crt_seed(i32::from_le_bytes(seed.try_into().unwrap()));
        }
        self.net_ui.players.clear();
        crate::menu::title::set_screen(self, 0);
        self.g.m.set_i32(crate::world::GAME, 1);
    }

    pub fn net_connect_failed(&mut self, message: &str) {
        crate::menu::title::set_screen(self, 5);
        set_room_code(self, "");
        self.net_ui.players.clear();
        self.net_ui.error = Some(message.to_string());
        self.requests.push(MenuRequest::NetClose);
    }

    /// 0x41cc27..0x41cf12 without the checks: run the F-keys any computer pressed.
    pub(crate) fn net_apply_packet(&mut self) {
        for (vk, byte, bit) in FKEY_BITS {
            if self.g.m.u8(IN + byte as u32) & bit != 0 {
                self.fkey(vk);
            }
        }
    }

    /// `read_remote_inputs` (0x4198f0): the keys of the slots played through the
    /// network (player -1), from the received packet on half_frame 0.
    pub(crate) fn read_remote_inputs(&mut self, half: i32) {
        for slot in 0..8usize {
            if self.g.m.i32(PLAYER_OF_SLOT + 4 * slot as u32) != -1 {
                continue;
            }
            let o = &mut self.objs[slot];
            o.shift_keys();
            if half != 0 {
                continue;
            }
            let b = self.g.m.u8(IN + slot as u32);
            let o = &mut self.objs[slot];
            for (k, bit) in [0x80u8, 0x40, 0x20, 0x10, 8, 4, 2].iter().enumerate() {
                o.m.set_u8(0xcd + k as u32, (b & bit != 0) as u8);
            }
            if self.g.recording() != 0 {
                self.rec_frame[slot] = b;
            }
        }
    }
}

/// This computer's input for one frame from the platform's key array, as player 1
/// (control settings) presses it. F-keys pressed are consumed (written back as 'u'),
/// as the original's frame does.
pub fn local_input(w: &World, keys: &mut [u8; 256]) -> Input {
    let base = crate::world::CONTROLS + 0x50;
    let mut b = 0u8;
    if w.g.m.i32(base) == 0 {
        for (k, bit) in [0x80u8, 0x40, 0x20, 0x10, 8, 4, 2].iter().enumerate() {
            let vk = w.g.m.i32(base + 4 + 4 * k as u32) as usize & 0xff;
            if keys[vk] == b'd' {
                b |= bit;
            }
        }
    }
    let mut input = [b, 0, 0];
    for (vk, byte, bit) in FKEY_BITS {
        // F5 changes the frame period, which the computers do not share
        if vk == 0x74 {
            continue;
        }
        if keys[vk as usize] == b'd' {
            keys[vk as usize] = b'u';
            input[if byte == 10 { 1 } else { 2 }] |= bit;
        }
    }
    input
}

/// The volume keys (F11, F12) during a network game, where the frames do not read
/// the key array: change the volume at 0x44d000 directly.
fn volume_keys(w: &mut World, keys: &[u8; 256]) {
    let d = (keys[0x7b] == b'd') as i32 - (keys[0x7a] == b'd') as i32;
    if d != 0 {
        let v = (w.g.m.i32(0x44d000) + d).clamp(0, 100);
        w.g.m.set_i32(0x44d000, v);
    }
}

mod tag {
    pub const HELLO: u8 = 1;
    pub const ROSTER: u8 = 2;
    pub const INPUT: u8 = 3;
    pub const START: u8 = 4;
    pub const REFUSE: u8 = 5;
    pub const LEFT: u8 = 6;
    pub const PING: u8 = 7;
    pub const PONG: u8 = 8;
}

/// True for the messages the platform sends on a channel that may lose and reorder
/// messages (INPUT, which repeats everything not yet acknowledged, and the pings).
/// All others need a reliable, ordered channel.
pub fn is_unreliable(m: &[u8]) -> bool {
    matches!(m.first(), Some(&tag::INPUT | &tag::PING | &tag::PONG))
}

/// How often each computer measures the round-trip time to the others.
const PING_EVERY_MS: u32 = 500;
/// How long the notice that a player left stays on the screen.
const NOTICE_MS: u32 = 3000;

/// The most inputs of one seat an INPUT message carries.
const MAX_BLOCK: u32 = 64;

struct Peer {
    id: u32,
    name: [u8; 11],
    seat: usize,
    /// For each seat, how many inputs from frame 0 on this joiner has (from its
    /// acknowledgements): the host sends it the rest.
    has: [u32; MAX_SEATS],
    frame: u32,
    gone: bool,
    ping: Option<u32>,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Stage {
    Lobby,
    Playing,
    Over,
}

/// The network game between the platform's links and the world. The platform
/// reports what its links do (a room code, a link opened, a message, a link
/// closed) and sends what `take_outgoing` gives, each message to the link it names.
/// Peer ids are the platform's: the host numbers its joiners' links, a joiner has
/// one link, to the host, with id 0. Messages must arrive whole; INPUT messages may
/// be lost, repeated or reordered, the others must arrive in order.
pub struct NetSession {
    host: bool,
    stage: Stage,
    peers: Vec<Peer>,
    connected: bool,
    rb: Option<Rollback>,
    out: Vec<(u32, Vec<u8>)>,
    /// F-keys pressed while no frame ran, for the next frame.
    pending_fk: [u8; 2],
    /// Joiner: the host's frame and its frame advantage over this computer, from
    /// the host's last message, for the time sync.
    host_frame: u32,
    host_adv: i32,
    /// Joiner: how many of each seat's inputs the host has, from its last message.
    host_has: [u32; MAX_SEATS],
    since_sync: u32,
    /// Hashes of the game state (frame, hash) this computer made, and the last one
    /// from each other computer (by seat), compared when both are known.
    hashes: Vec<(u32, u64)>,
    peer_hash: [(u32, u64); MAX_SEATS],
    frames_run: u32,
    pub log: Vec<(u32, u64)>,
    clock: u32,
    last_ping: u32,
    host_ping: Option<u32>,
    names: Vec<String>,
    notice: Option<(String, u32)>,
}

impl NetSession {
    pub fn host() -> NetSession {
        NetSession {
            host: true,
            stage: Stage::Lobby,
            peers: Vec::new(),
            connected: false,
            rb: None,
            out: Vec::new(),
            pending_fk: [0; 2],
            host_frame: 0,
            host_adv: 0,
            host_has: [0; MAX_SEATS],
            since_sync: 0,
            hashes: Vec::new(),
            peer_hash: [(u32::MAX, 0); MAX_SEATS],
            frames_run: 0,
            log: Vec::new(),
            clock: 0,
            last_ping: 0,
            host_ping: None,
            names: Vec::new(),
            notice: None,
        }
    }

    pub fn join() -> NetSession {
        NetSession { host: false, ..NetSession::host() }
    }

    pub fn is_host(&self) -> bool {
        self.host
    }

    pub fn in_game(&self) -> bool {
        self.stage == Stage::Playing
    }

    pub fn rollback(&self) -> Option<&Rollback> {
        self.rb.as_ref()
    }

    pub fn on_code(&mut self, w: &mut World, code: &str) {
        set_room_code(w, code);
        self.show_players(w);
    }

    pub fn set_clock(&mut self, ms: u32) {
        self.clock = ms;
        if self.stage == Stage::Over || ms.wrapping_sub(self.last_ping) < PING_EVERY_MS {
            return;
        }
        self.last_ping = ms;
        let mut m = vec![tag::PING];
        m.extend_from_slice(&ms.to_le_bytes());
        if self.host {
            for p in self.peers.iter().filter(|p| !p.gone) {
                self.out.push((p.id, m.clone()));
            }
        } else if self.connected {
            self.out.push((0, m));
        }
    }

    fn room_names(&self, w: &World) -> Vec<[u8; 11]> {
        let mut names = vec![display_name(&w.net_local_name(), 0)];
        for (k, p) in self.peers.iter().enumerate() {
            names.push(display_name(&p.name, k + 1));
        }
        names
    }

    fn show_players(&self, w: &mut World) {
        let names = self.room_names(w);
        let pings = std::iter::once(None).chain(self.peers.iter().map(|p| p.ping));
        w.net_ui.players = names.iter().zip(pings).enumerate().map(|(k, (n, ping))| NetPlayer { name: name_string(n), ping, me: k == 0 }).collect();
    }

    fn worst_ping(&self) -> Option<u32> {
        if self.host {
            self.peers.iter().filter(|p| !p.gone).filter_map(|p| p.ping).max()
        } else {
            self.host_ping
        }
    }

    pub fn on_connected(&mut self, w: &mut World, peer: u32) {
        if self.host {
            return;
        }
        self.connected = true;
        let mut m = vec![tag::HELLO];
        m.extend_from_slice(&PROTOCOL.to_le_bytes());
        m.extend_from_slice(&w.net_data_checksum().to_le_bytes());
        m.extend_from_slice(&w.net_local_name());
        self.out.push((peer, m));
        self.set_clock(self.clock.wrapping_add(PING_EVERY_MS));
    }

    /// A link closed. Before the game a joiner goes back to "server or client" with
    /// the message; in the game a joiner who lost the host stops the game, and the
    /// host goes on with the seat of a joiner who left pressing nothing.
    pub fn on_closed(&mut self, w: &mut World, peer: u32, msg: &str) {
        if !self.host {
            match self.stage {
                Stage::Lobby => w.net_connect_failed(msg),
                Stage::Playing => self.end_game(w, "Connection Lost!"),
                Stage::Over => {}
            }
            return;
        }
        let Some(i) = self.peers.iter().position(|p| p.id == peer && !p.gone) else { return };
        match self.stage {
            Stage::Lobby => {
                self.peers.remove(i);
                self.send_roster(w);
            }
            Stage::Playing => {
                self.peers[i].gone = true;
                let seat = self.peers[i].seat;
                self.left_notice(seat);
                let rb = self.rb.as_mut().unwrap();
                let at = rb.known(seat);
                rb.leave(seat, at);
                let mut m = vec![tag::LEFT, seat as u8];
                m.extend_from_slice(&at.to_le_bytes());
                for p in self.peers.iter().filter(|p| !p.gone) {
                    self.out.push((p.id, m.clone()));
                }
            }
            Stage::Over => {}
        }
    }

    pub fn take_outgoing(&mut self) -> Vec<(u32, Vec<u8>)> {
        std::mem::take(&mut self.out)
    }

    fn end_game(&mut self, w: &mut World, msg: &'static str) {
        self.stage = Stage::Over;
        w.requests.push(MenuRequest::Message(msg));
        w.requests.push(MenuRequest::Quit);
    }

    fn left_notice(&mut self, seat: usize) {
        if let Some(n) = self.names.get(seat) {
            self.notice = Some((format!("{} left the game", n), self.clock.wrapping_add(NOTICE_MS)));
        }
    }

    /// Host: send each joiner the list, with its own place in it.
    fn send_roster(&mut self, w: &mut World) {
        let names = self.room_names(w);
        self.show_players(w);
        for (k, p) in self.peers.iter().enumerate() {
            let mut m = vec![tag::ROSTER, names.len() as u8, k as u8 + 1];
            for n in &names {
                m.extend_from_slice(n);
            }
            self.out.push((p.id, m));
        }
    }

    pub fn players(&self) -> usize {
        1 + self.peers.len()
    }

    pub fn start(&mut self, w: &mut World) {
        if !self.host || self.stage != Stage::Lobby || self.players() < 2 {
            return;
        }
        let names = self.room_names(w);
        let table = w.net_table();
        for (k, p) in self.peers.iter_mut().enumerate() {
            p.seat = k + 1;
            let mut m = vec![tag::START, names.len() as u8, p.seat as u8];
            for n in &names {
                m.extend_from_slice(n);
            }
            m.extend_from_slice(&table);
            self.out.push((p.id, m));
        }
        self.begin(w, &names, 0, &table);
    }

    fn begin(&mut self, w: &mut World, names: &[[u8; 11]], me: usize, table: &[u8]) {
        w.net_start(names, me, table);
        self.names = names.iter().map(name_string).collect();
        self.rb = Some(Rollback::new(names.len(), me));
        self.stage = Stage::Playing;
    }

    pub fn on_message(&mut self, w: &mut World, peer: u32, m: &[u8]) {
        let Some(&t) = m.first() else { return };
        let body = &m[1..];
        if self.host {
            match t {
                tag::HELLO => self.on_hello(w, peer, body),
                tag::PING => self.pong(peer, body),
                tag::PONG if body.len() >= 4 => {
                    let t = u32::from_le_bytes(body[..4].try_into().unwrap());
                    if let Some(p) = self.peers.iter_mut().find(|p| p.id == peer) {
                        p.ping = Some(self.clock.wrapping_sub(t));
                    }
                    if self.stage == Stage::Lobby {
                        self.show_players(w);
                    }
                }
                tag::INPUT => {
                    if let Some(i) = self.peers.iter().position(|p| p.id == peer && !p.gone) {
                        self.on_input(w, Some(i), body);
                    }
                }
                _ => {}
            }
            return;
        }
        match t {
            tag::ROSTER if self.stage == Stage::Lobby && body.len() >= 2 => {
                let (n, me) = (body[0] as usize, body[1] as usize);
                w.net_ui.players = (0..n)
                    .filter_map(|k| body.get(2 + 11 * k..13 + 11 * k))
                    .enumerate()
                    .map(|(k, b)| NetPlayer { name: name_string(&b.try_into().unwrap()), ping: if k == 0 { self.host_ping } else { None }, me: k == me })
                    .collect();
            }
            tag::PING => self.pong(peer, body),
            tag::PONG if body.len() >= 4 => {
                let t = u32::from_le_bytes(body[..4].try_into().unwrap());
                self.host_ping = Some(self.clock.wrapping_sub(t));
                if let Some(h) = w.net_ui.players.first_mut() {
                    h.ping = self.host_ping;
                }
            }
            tag::REFUSE if self.stage == Stage::Lobby => {
                let msg = String::from_utf8_lossy(body).into_owned();
                w.net_connect_failed(&msg);
            }
            tag::START if self.stage == Stage::Lobby => {
                let (n, me) = (*body.first().unwrap_or(&0) as usize, *body.get(1).unwrap_or(&0) as usize);
                let names_end = 2 + 11 * n;
                if n < 2 || n > MAX_SEATS || me >= n || body.len() < names_end + TABLE + 4 {
                    return;
                }
                let names: Vec<[u8; 11]> = (0..n).map(|k| body[2 + 11 * k..13 + 11 * k].try_into().unwrap()).collect();
                let table = body[names_end..names_end + TABLE + 4].to_vec();
                self.begin(w, &names, me, &table);
            }
            tag::INPUT if self.stage == Stage::Playing => self.on_input(w, None, body),
            tag::LEFT if self.stage == Stage::Playing && body.len() >= 5 => {
                let seat = body[0] as usize;
                let at = u32::from_le_bytes(body[1..5].try_into().unwrap());
                if let Some(rb) = self.rb.as_mut() {
                    if seat < rb.seats() {
                        rb.leave(seat, at);
                        self.left_notice(seat);
                    }
                }
            }
            _ => {}
        }
    }

    fn on_hello(&mut self, w: &mut World, peer: u32, b: &[u8]) {
        let refuse = |s: &mut Self, why: &str| {
            let mut m = vec![tag::REFUSE];
            m.extend_from_slice(why.as_bytes());
            s.out.push((peer, m));
        };
        if self.stage != Stage::Lobby {
            return refuse(self, "That game has already started.");
        }
        if b.len() < 2 + 4 + 11 {
            return refuse(self, "That is not a Little Fighter 2 game.");
        }
        if u16::from_le_bytes([b[0], b[1]]) != PROTOCOL {
            return refuse(self, "Version not matched!");
        }
        if i32::from_le_bytes(b[2..6].try_into().unwrap()) != w.net_data_checksum() {
            return refuse(self, "Data Error! You and your opponent have different set of data files.");
        }
        if self.players() >= MAX_SEATS {
            return refuse(self, "That game is full.");
        }
        let mut name: [u8; 11] = b[6..17].try_into().unwrap();
        name[10] = 0;
        self.peers.retain(|p| p.id != peer);
        self.peers.push(Peer { id: peer, name, seat: 0, has: [0; MAX_SEATS], frame: 0, gone: false, ping: None });
        self.send_roster(w);
    }

    fn pong(&mut self, peer: u32, body: &[u8]) {
        let mut m = vec![tag::PONG];
        m.extend_from_slice(&body[..body.len().min(4)]);
        self.out.push((peer, m));
    }

    fn on_input(&mut self, w: &mut World, from: Option<usize>, b: &[u8]) {
        let Some(rb) = self.rb.as_mut() else { return };
        let mut r = Reader(b);
        let (Some(frame), Some(adv), Some(n_acks)) = (r.u32(), r.i16(), r.u8()) else { return };
        let mut acks = [0u32; MAX_SEATS];
        for a in acks.iter_mut().take(n_acks as usize) {
            *a = r.u32().unwrap_or(0);
        }
        let (Some(hash_frame), Some(hash)) = (r.u32(), r.u64()) else { return };
        let Some(blocks) = r.u8() else { return };
        for _ in 0..blocks {
            let (Some(seat), Some(first), Some(n)) = (r.u8(), r.u32(), r.u8()) else { return };
            let seat = seat as usize;
            for k in 0..n as u32 {
                let Some(i) = r.bytes(3) else { return };
                // a joiner sends only its own seat
                let allowed = match from {
                    Some(p) => seat == self.peers[p].seat,
                    None => seat != rb.me(),
                };
                if allowed && seat < rb.seats() {
                    rb.add_input(seat, first + k, [i[0], i[1], i[2]]);
                }
            }
        }
        let seat = match from {
            Some(p) => {
                let peer = &mut self.peers[p];
                for (h, a) in peer.has.iter_mut().zip(acks) {
                    *h = (*h).max(a);
                }
                peer.frame = peer.frame.max(frame);
                peer.seat
            }
            None => {
                self.host_frame = self.host_frame.max(frame);
                self.host_adv = adv as i32;
                for (h, a) in self.host_has.iter_mut().zip(acks) {
                    *h = (*h).max(a);
                }
                0
            }
        };
        if hash_frame != u32::MAX {
            self.peer_hash[seat] = (hash_frame, hash);
        }
        self.check_hashes(w);
    }

    fn check_hashes(&mut self, w: &mut World) {
        if self.stage != Stage::Playing {
            return;
        }
        for (f, h) in self.peer_hash {
            if f == u32::MAX {
                continue;
            }
            if let Some(&(_, mine)) = self.hashes.iter().find(|x| x.0 == f) {
                if mine != h {
                    self.end_game(w, "Synchronisation Error!");
                    return;
                }
            }
        }
    }

    /// One frame of the platform's clock in the game: take this computer's input
    /// from `keys`, run the frames that can run (none while the inputs of a
    /// computer are too far behind, two to catch up with the host), and send the
    /// inputs. Returns the number of new frames; the world's sounds, music and
    /// requests are those of the new frames.
    pub fn advance(&mut self, w: &mut World, keys: &mut [u8; 256]) -> u32 {
        volume_keys(w, keys);
        if self.stage != Stage::Playing {
            return 0;
        }
        let mut input = local_input(w, keys);
        input[1] |= self.pending_fk[0];
        input[2] |= self.pending_fk[1];
        let rb = self.rb.as_mut().unwrap();
        // the time sync: a joiner ahead of the host waits a frame, one behind runs
        // two; at most once every 8 frames
        let mut want = 1;
        self.since_sync += 1;
        if !self.host && self.since_sync >= 8 {
            let mine = rb.frame() as i32 - self.host_frame as i32;
            let offset = (mine - self.host_adv) / 2;
            if offset > 1 {
                want = 0;
                self.since_sync = 0;
            } else if offset < -1 {
                want = 2;
                self.since_sync = 0;
            }
        }
        let volume = w.g.m.i32(0x44d000);
        let ran = rb.advance(w, input, want, &mut self.frames_run);
        w.g.m.set_i32(0x44d000, volume);
        if ran == 0 {
            self.pending_fk = [input[1], input[2]];
        } else {
            self.pending_fk = [0; 2];
        }
        for h in rb.take_hashes() {
            self.hashes.push(h);
            self.log.push(h);
        }
        if self.hashes.len() > 64 {
            self.hashes.drain(..self.hashes.len() - 64);
        }
        self.check_hashes(w);
        self.send_inputs();
        if ran > 0 {
            self.overlay(w);
        }
        ran
    }

    fn overlay(&mut self, w: &mut World) {
        let redo = self.rb.as_ref().map_or(0, |r| r.last_redo);
        let ping = self.worst_ping().map_or("-".to_string(), |p| p.to_string());
        let text = format!("{} ms  rollback {}", ping, redo);
        w.draw.push(crate::draw::Draw::Label { text: text.into_bytes(), rgb: 0xc8d2f5, x: 4, y: 532, align: -1, scale: 1, pad: Some(0) });
        if let Some((n, until)) = &self.notice {
            if (until.wrapping_sub(self.clock) as i32) > 0 {
                w.draw.push(crate::draw::Draw::Label { text: n.clone().into_bytes(), rgb: 0xffffff, x: 397, y: 140, align: 0, scale: 1, pad: Some(0x122565) });
            } else {
                self.notice = None;
            }
        }
    }

    pub fn settle(&mut self, w: &mut World) {
        if let Some(rb) = self.rb.as_mut() {
            rb.advance(w, [0; 3], 0, &mut self.frames_run);
            for h in rb.take_hashes() {
                self.hashes.push(h);
                self.log.push(h);
            }
            self.check_hashes(w);
            self.send_inputs();
        }
    }

    /// Send every link the inputs it does not have yet, with this computer's
    /// acknowledgements, frame, and last hash.
    fn send_inputs(&mut self) {
        let Some(rb) = self.rb.as_ref() else { return };
        let hash = self.hashes.last().copied().unwrap_or((u32::MAX, 0));
        let seats = rb.seats();
        let acks: Vec<u32> = (0..seats).map(|s| rb.known(s)).collect();
        let header = |adv: i32| {
            let mut m = vec![tag::INPUT];
            m.extend_from_slice(&rb.frame().to_le_bytes());
            m.extend_from_slice(&(adv.clamp(-30000, 30000) as i16).to_le_bytes());
            m.push(seats as u8);
            for a in &acks {
                m.extend_from_slice(&a.to_le_bytes());
            }
            m.extend_from_slice(&hash.0.to_le_bytes());
            m.extend_from_slice(&hash.1.to_le_bytes());
            m
        };
        let block = |m: &mut Vec<u8>, seat: usize, from: u32| {
            let to = rb.known(seat).min(rb.input_count(seat));
            let from = from.max(to.saturating_sub(MAX_BLOCK));
            if from >= to {
                return false;
            }
            m.push(seat as u8);
            m.extend_from_slice(&from.to_le_bytes());
            m.push((to - from) as u8);
            for f in from..to {
                m.extend_from_slice(&rb.input(seat, f));
            }
            true
        };
        if self.host {
            for p in self.peers.iter().filter(|p| !p.gone) {
                let adv = rb.frame() as i32 - p.frame as i32;
                let mut m = header(adv);
                let at = m.len();
                m.push(0);
                let mut n = 0u8;
                for s in (0..seats).filter(|&s| s != p.seat) {
                    if block(&mut m, s, p.has[s]) {
                        n += 1;
                    }
                }
                m[at] = n;
                self.out.push((p.id, m));
            }
        } else {
            let me = rb.me();
            let mut m = header(0);
            let at = m.len();
            m.push(0);
            if block(&mut m, me, self.host_has[me]) {
                m[at] = 1;
            }
            self.out.push((0, m));
        }
    }
}

fn name_string(n: &[u8; 11]) -> String {
    String::from_utf8_lossy(&n[..n.iter().position(|&c| c == 0).unwrap_or(11)]).into_owned()
}

struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    fn bytes(&mut self, n: usize) -> Option<&'a [u8]> {
        if self.0.len() < n {
            return None;
        }
        let (a, b) = self.0.split_at(n);
        self.0 = b;
        Some(a)
    }
    fn u8(&mut self) -> Option<u8> {
        self.bytes(1).map(|b| b[0])
    }
    fn i16(&mut self) -> Option<i16> {
        self.bytes(2).map(|b| i16::from_le_bytes([b[0], b[1]]))
    }
    fn u32(&mut self) -> Option<u32> {
        self.bytes(4).map(|b| u32::from_le_bytes(b.try_into().unwrap()))
    }
    fn u64(&mut self) -> Option<u64> {
        self.bytes(8).map(|b| u64::from_le_bytes(b.try_into().unwrap()))
    }
}
