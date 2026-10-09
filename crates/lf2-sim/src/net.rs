//! The network game (two computers, four players each). The original runs it over
//! TCP port 12345; this port carries the same bytes over a WebRTC data channel set up
//! with a room code (lf2-app rtc.rs, the web page), so it no longer plays against the
//! original lf2.exe.
//!
//! What the original does (see re/notes.md, "Network game"):
//! - The title screen's "network game" (0x4246b0 state 1). Choosing "server" hosts a
//!   room and waits for the other computer (state 2); "client" asks for the room code
//!   (state 3) and connects (state 4). (The original listens on a socket and asks for
//!   the server's IP address.)
//! - Handshake (server in `net_receive` 0x402ec0 on FD_ACCEPT, client in 0x4246b0):
//!   the server sends "u can connect" (14 bytes with the terminator); the client
//!   sends a 77-byte hello, the server answers with a 77-byte reply and then 3001
//!   bytes from 0x44ff90: the random table and the low byte of its index. Hello and
//!   reply hold 8 slot flags ('1': the slot is played on the other computer) and,
//!   from byte 0x20, the four player names, 11 bytes each, padded with '_'. The server
//!   plays slots 0-3, the client slots 4-7.
//! - Every second frame (half_frame 0) of match_frame (0x41bc90) the two exchange a
//!   22-byte packet: bytes 0-7 the keys of each slot (0x01 plus up 0x80, down 0x40,
//!   left 0x20, right 0x10, attack 8, jump 4, defend 2), byte 9 a counter mod 50,
//!   bytes 10 and 12 the F-keys, byte 11 the version, byte 13 the sum of the hp of
//!   objects 0-19 mod 100 plus 1, bytes 14-15 the data checksum. The client sends
//!   first, the server receives first; both wait for the other's packet. Mismatched
//!   counters, version, hp sums or checksums end the game with a message.
//!
//! The simulation stays free of sockets: `World::step` stops at the exchange with
//! the packet to send in `World::net_out`; the platform sends it, receives the other
//! computer's packet and calls `World::net_resume`. The handshake is done by the
//! platform with the bytes from `net_hello`/`net_reply`/`net_table` and the results
//! given to `net_client_joined`/`net_server_joined`.

use crate::menu::main_menu::MenuRequest;
use crate::world::{World, EXISTS, KEYS, PLAYER_OF_SLOT};

/// `DAT_0044f1af`: 0 no network game, 1 client, 2 server.
pub const ROLE: u32 = 0x44f1af;
/// `DAT_0044f1ae`: 1 once a client has joined the server.
pub const JOINED: u32 = 0x44f1ae;
/// The packet this computer sends (`DAT_0044d040`).
pub const OUT: u32 = 0x44d040;
/// The packet received from the other computer (`DAT_0044f198`).
pub const IN: u32 = 0x44f198;
pub const PACKET: usize = 0x16;
/// The local address shown on the network screens (`DAT_0044f340`, 200 bytes).
pub const LOCAL_IP: u32 = 0x44f340;
/// The text typed on the client screen (game object + 0x7d8): the room code.
pub const SERVER_IP: u32 = 0x4592d8;
/// What the server sends first.
pub const GREETING: &[u8; 14] = b"u can connect\0";
/// Hello and reply length.
pub const HELLO: usize = 0x4d;
/// Random table bytes the server sends.
pub const TABLE: usize = 0xbb9;
const COUNTER: u32 = 0x450bf0;
const HP_CHECK: u32 = 0x450bf4;
const VERSION: u32 = 0x44d03c;
const CHECKSUM: u32 = 0x44f620;
const NAMES: u32 = 0x44fcc0;

/// The F-keys in the packet, in the order the original applies them: key, packet
/// byte (10 or 12), bit.
const FKEY_BITS: [(u32, usize, u8); 11] = [
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

/// The order the original reads the local F-keys in: key, packet byte, bit, only
/// while a match runs.
const FKEY_READ: [(u32, usize, u8, bool); 11] = [
    (0xf9, 12, 0x08, false),
    (0xf8, 12, 0x10, false),
    (0x70, 10, 0x02, true),
    (0x71, 10, 0x40, true),
    (0x72, 12, 0x02, true),
    (0x73, 10, 0x04, true),
    (0x74, 10, 0x08, true),
    (0x75, 10, 0x10, true),
    (0x76, 10, 0x20, true),
    (0x77, 12, 0x04, true),
    (0x78, 10, 0x80, true),
];

/// Set the room code the network screens show ("" for none).
pub fn set_room_code(w: &mut World, code: &str) {
    for k in 0..200 {
        w.g.m.set_u8(LOCAL_IP + k, 0);
    }
    for (k, b) in code.bytes().take(199).enumerate() {
        w.g.m.set_u8(LOCAL_IP + k as u32, b);
    }
}

/// C's `%` of 256 for the checksum bytes (the sign follows the dividend).
fn byte_c(v: i32) -> u8 {
    (v % 256) as u8
}

impl World {
    pub fn net_role(&self) -> i8 {
        self.g.m.i8(ROLE)
    }

    /// The room code the player typed on the client screen.
    pub fn room_code_typed(&self) -> String {
        let b: Vec<u8> = (0..0x33).map(|k| self.g.m.u8(SERVER_IP + k)).take_while(|&c| c != 0).collect();
        String::from_utf8_lossy(&b).into_owned()
    }

    /// The four local names, 11 bytes each, '_' for the terminator and the bytes
    /// after it, as the hello and the reply carry them (from 0x44fcc0).
    fn names_field(&self) -> [u8; 44] {
        let mut f = [b'_'; 44];
        for p in 0..4u32 {
            for k in 0..11u32 {
                let c = self.g.m.u8(NAMES + 11 * p + k);
                if c == 0 {
                    break;
                }
                f[(11 * p + k) as usize] = c;
            }
        }
        f
    }

    fn hello_with(&self, flags: &[u8; 8]) -> [u8; HELLO] {
        let mut h = [b'0'; HELLO];
        h[..8].copy_from_slice(flags);
        h[0x20..0x4c].copy_from_slice(&self.names_field());
        h[0x4c] = 0;
        h
    }

    /// The client's hello: slots 4-7 are its own.
    pub fn net_hello(&self) -> [u8; HELLO] {
        self.hello_with(b"00001111")
    }

    /// The server's reply: slots 0-3 are its own.
    pub fn net_reply(&self) -> [u8; HELLO] {
        self.hello_with(b"11110000")
    }

    /// The 3001 bytes the server sends after the reply: the random table at 0x44ff90
    /// and the byte after it.
    pub fn net_table(&self) -> Vec<u8> {
        (0..TABLE as u32).map(|k| self.g.m.u8(0x44ff90 + k)).collect()
    }

    /// The flags and names of the other computer's hello or reply.
    fn net_take_peer(&mut self, msg: &[u8], names_at: u32) {
        for k in 0..8u32 {
            if msg.get(k as usize) == Some(&b'1') {
                self.g.m.set_i32(PLAYER_OF_SLOT + 4 * k, -1);
            }
        }
        for k in 0..0x2cu32 {
            let c = msg.get(0x20 + k as usize).copied().unwrap_or(0);
            self.g.m.set_u8(names_at + k, if c == b'_' { 0 } else { c });
        }
    }

    /// The server after the handshake (`net_receive` 0x402ec0): `hello` is what the
    /// client sent.
    pub fn net_server_joined(&mut self, hello: &[u8]) {
        self.g.m.set_u8(ROLE, 2);
        for s in 0..4u32 {
            self.g.m.set_i32(PLAYER_OF_SLOT + 4 * s, s as i32 + 1);
        }
        self.net_take_peer(hello, 0x44fcec);
        self.g.m.set_u8(JOINED, 1);
    }

    /// The client after the handshake (0x428558..0x4286fa): `reply` and `table` are
    /// what the server sent.
    pub fn net_client_joined(&mut self, reply: &[u8], table: &[u8]) {
        // the client's own names move to slots 4-7 (strcpy at 0x4285c0)
        for p in 0..4u32 {
            for k in 0..11u32 {
                let c = self.g.m.u8(NAMES + 11 * p + k);
                self.g.m.set_u8(0x44fcec + 11 * p + k, c);
                if c == 0 {
                    break;
                }
            }
        }
        self.g.m.set_u8(ROLE, 1);
        for s in 0..4u32 {
            self.g.m.set_i32(PLAYER_OF_SLOT + 4 * (s + 4), s as i32 + 1);
        }
        for (k, &b) in table.iter().take(TABLE).enumerate() {
            self.g.m.set_u8(0x44ff90 + k as u32, b);
        }
        self.net_take_peer(reply, NAMES);
        crate::menu::title::set_screen(self, 4);
    }

    /// The connection to the server failed (state 3 goes back to state 1, or stays,
    /// as in the original), with the original's message.
    pub fn net_connect_failed(&mut self, message: &'static str, back: bool) {
        self.requests.push(MenuRequest::Message(message));
        if back {
            crate::menu::title::set_screen(self, 1);
        }
    }

    /// The platform cannot open sockets (the web build): back to the title screen,
    /// as the original's "back" button does.
    pub fn net_unavailable(&mut self) {
        crate::menu::title::set_screen(self, 0);
        self.g.m.set_i32(0x44d780, -1);
        self.free_menu_background();
    }

    /// Part of read_inputs_and_ai (0x419d3e): in a network game the keys of each
    /// local slot go into the outgoing packet.
    pub(crate) fn net_put_keys(&mut self, slot: usize) {
        if self.net_role() <= 0 {
            return;
        }
        let o = &self.objs[slot];
        let mut b = 0u8;
        for (k, bit) in [0x80u8, 0x40, 0x20, 0x10, 8, 4, 2].iter().enumerate() {
            if o.m.u8(0xcd + k as u32) != 0 {
                b |= bit;
            }
        }
        let at = OUT + slot as u32;
        self.g.m.set_u8(at, self.g.m.u8(at) | b);
    }

    /// 0x41c986..0x41cbd8: the local F-keys, the hp check and the counter go into the
    /// outgoing packet.
    pub(crate) fn net_build_packet(&mut self) -> [u8; PACKET] {
        let in_match = self.g.match_phase() == 0;
        for (vk, byte, bit, match_only) in FKEY_READ {
            let at = KEYS + vk;
            if self.g.m.u8(at) == b'd' && (!match_only || in_match) {
                let p = OUT + byte as u32;
                self.g.m.set_u8(p, self.g.m.u8(p) | bit);
                self.g.m.set_u8(at, b'u');
            }
        }
        let sum = (0..20u32).filter(|&i| self.g.m.u8(EXISTS + i) == 1).fold(0i32, |s, i| s.wrapping_add(self.objs[i as usize].hp()));
        let check = sum % 100 + 1;
        self.g.m.set_i32(HP_CHECK, check);
        let counter = (self.g.m.i32(COUNTER) + 1) % 0x32;
        self.g.m.set_i32(COUNTER, counter);
        let cs = self.g.m.i32(CHECKSUM);
        self.g.m.set_u8(OUT + 9, counter as u8);
        self.g.m.set_u8(OUT + 11, self.g.m.i32(VERSION) as u8);
        self.g.m.set_u8(OUT + 13, check as u8);
        self.g.m.set_u8(OUT + 14, byte_c(cs));
        self.g.m.set_u8(OUT + 15, byte_c(cs / 256));
        let mut p = [0u8; PACKET];
        for (k, b) in p.iter_mut().enumerate() {
            *b = self.g.m.u8(OUT + k as u32);
        }
        p
    }

    /// 0x41cc27..0x41cf12: check the other computer's packet, then apply the F-keys
    /// either computer pressed.
    pub(crate) fn net_apply_packet(&mut self) {
        let s8 = |w: &World, at: u32| w.g.m.i8(at) as i32;
        let mut fail: Option<&'static str> = None;
        let mut checks: Vec<&'static str> = Vec::new();
        if s8(self, IN + 9) != self.g.m.i32(COUNTER) {
            checks.push("Connection Lost!");
        }
        if s8(self, IN + 11) != self.g.m.i32(VERSION) {
            checks.push(if self.net_role() == 1 { "Version not matched! " } else { "Version not matched!" });
        }
        if s8(self, IN + 13) != self.g.m.i32(HP_CHECK) {
            checks.push("Synchronisation Error!");
        }
        let cs = self.g.m.i32(CHECKSUM);
        if self.g.m.u8(IN + 14) != byte_c(cs) || self.g.m.u8(IN + 15) != byte_c(cs / 256) {
            checks.push("Data Error! You and your opponent have different set of data files.");
        }
        for m in checks {
            self.requests.push(MenuRequest::Message(m));
            fail = Some(m);
        }
        if fail.is_some() {
            // sound_release, music_release, recording_stop, then WM_CLOSE
            self.g.set_recording(0);
            self.requests.push(MenuRequest::Quit);
        }
        for (vk, byte, bit) in FKEY_BITS {
            let remote = self.g.m.u8(IN + byte as u32) & bit != 0;
            let local = self.g.m.u8(OUT + byte as u32) & bit != 0;
            if remote || local {
                self.fkey(vk);
            }
        }
    }

    /// `read_remote_inputs` (0x4198f0): the keys of the slots played on the other
    /// computer (player -1), from the received packet on half_frame 0.
    pub(crate) fn read_remote_inputs(&mut self, half: i32) {
        for slot in 0..8usize {
            if self.g.m.i32(PLAYER_OF_SLOT + 4 * slot as u32) != -1 {
                continue;
            }
            let o = &mut self.objs[slot];
            for k in 0..7u32 {
                let cur = o.m.u8(0xcd + k);
                o.m.set_u8(0xc6 + k, cur);
            }
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

    /// The platform received the other computer's packet (None: the connection
    /// closed, and the old packet stays, as when the original's recv returns 0).
    /// Finishes the frame `step` stopped in.
    pub fn net_resume(&mut self, packet: Option<&[u8]>) {
        if let Some(p) = packet {
            for (k, &b) in p.iter().take(PACKET).enumerate() {
                self.g.m.set_u8(IN + k as u32, b);
            }
        }
        if !self.net_waiting {
            return;
        }
        self.net_waiting = false;
        self.match_frame_after_exchange();
        // the end of the game state 2 frame (0x4246b0)
        self.g.m.set_i32(0x457580, 0);
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Stage {
    /// Host: waiting for the room code, then for the other computer.
    HostWaiting,
    /// Host: connected, waiting for the hello.
    HostHello,
    /// Joiner: connecting, waiting for the greeting.
    JoinGreeting,
    /// Joiner: hello sent, waiting for reply and table.
    JoinReply,
    /// The game runs.
    Playing,
}

/// The network game's byte stream without the transport: the platform feeds it
/// what the link reports (room code, connected, data, closed) and sends what
/// `take_outgoing` gives. Both the desktop program (WebRTC with str0m) and the web
/// page (the browser's WebRTC) use it.
pub struct NetSession {
    stage: Stage,
    got: Vec<u8>,
    out: Vec<Vec<u8>>,
    closed: bool,
    /// A message for the player when the link failed before the game ran.
    pub error: Option<String>,
}

impl NetSession {
    pub fn host() -> NetSession {
        NetSession { stage: Stage::HostWaiting, got: Vec::new(), out: Vec::new(), closed: false, error: None }
    }

    pub fn join() -> NetSession {
        NetSession { stage: Stage::JoinGreeting, ..NetSession::host() }
    }

    pub fn on_code(&mut self, w: &mut World, code: &str) {
        set_room_code(w, code);
    }

    /// The data channel opened: the host greets, as the original's server does on
    /// FD_ACCEPT.
    pub fn on_connected(&mut self) {
        if self.stage == Stage::HostWaiting {
            self.out.push(GREETING.to_vec());
            self.stage = Stage::HostHello;
        }
    }

    pub fn on_data(&mut self, d: &[u8]) {
        self.got.extend_from_slice(d);
    }

    /// The link closed. Before the game ran the screen goes back to "server or
    /// client" with the message.
    pub fn on_closed(&mut self, w: &mut World, msg: &str) {
        self.closed = true;
        if self.stage != Stage::Playing {
            self.error = Some(msg.to_string());
            crate::menu::title::set_screen(w, 1);
            set_room_code(w, "");
        }
    }

    pub fn is_closed(&self) -> bool {
        self.closed
    }

    /// Bytes to send to the other computer, in order.
    pub fn take_outgoing(&mut self) -> Vec<Vec<u8>> {
        std::mem::take(&mut self.out)
    }

    fn take(&mut self, n: usize) -> Option<Vec<u8>> {
        (self.got.len() >= n).then(|| self.got.drain(..n).collect())
    }

    /// Between frames, before the game runs: move the handshake on. True while the
    /// joiner waits for the server (the world does not run then, as the original
    /// blocks in its connect).
    pub fn handshake_busy(&mut self, w: &mut World) -> bool {
        if self.closed {
            return false;
        }
        match self.stage {
            Stage::HostHello => {
                if let Some(hello) = self.take(HELLO) {
                    self.out.push(w.net_reply().to_vec());
                    self.out.push(w.net_table());
                    w.net_server_joined(&hello);
                    self.stage = Stage::Playing;
                }
                false
            }
            Stage::JoinGreeting => {
                if let Some(g) = self.take(GREETING.len()) {
                    if g != GREETING {
                        self.on_closed(w, "That is not a Little Fighter 2 game.");
                        return false;
                    }
                    self.out.push(w.net_hello().to_vec());
                    self.stage = Stage::JoinReply;
                }
                true
            }
            Stage::JoinReply => {
                if self.got.len() >= HELLO + TABLE {
                    let reply = self.take(HELLO).unwrap();
                    let table = self.take(TABLE).unwrap();
                    w.net_client_joined(&reply, &table);
                    self.stage = Stage::Playing;
                    return false;
                }
                true
            }
            _ => false,
        }
    }

    /// `World::step` stopped at the exchange: queue this computer's packet.
    pub fn begin_exchange(&mut self, w: &mut World) {
        if let Some(p) = w.net_out.take() {
            self.out.push(p.to_vec());
        }
    }

    /// The other computer's packet: Some(Some(packet)) when it has come, Some(None)
    /// when the link closed, None while waiting.
    pub fn poll_exchange(&mut self) -> Option<Option<Vec<u8>>> {
        if let Some(p) = self.take(PACKET) {
            return Some(Some(p));
        }
        self.closed.then_some(None)
    }
}
