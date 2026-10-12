//! WebRTC links set up through icebreaker. Joiners connect only to the host.
//! `new_rtc` gathers host and server-reflexive candidates before creating an SDP;
//! icebreaker does not support trickling. str0m handles ICE, DTLS and SCTP; this
//! module owns the UDP sockets.

use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use std::time::{Duration, Instant};
use str0m::change::{SdpAnswer, SdpOffer};
use str0m::channel::{ChannelConfig, ChannelId, Reliability};
use str0m::net::{Protocol, Receive};
use str0m::{Candidate, Event, IceConnectionState, Input, Output, Rtc};
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Message, WebSocket};

pub const DEFAULT_URL: &str = "wss://rv.lf2-showdown.online/ws?app=lf2-showdown";
const DEFAULT_STUN: &str = "stun.l.google.com:19302";

pub fn rendezvous_url() -> String {
    std::env::var("LF2_RENDEZVOUS").unwrap_or_else(|_| DEFAULT_URL.into())
}

/// The host numbers joiners from 1 in arrival order; a joiner calls the host 0.
#[derive(Debug)]
pub enum LinkEvent {
    Code(String),
    Connected(u32),
    Data(u32, Vec<u8>),
    /// A link failed or closed, with a message for the player. `None`: all of them
    /// (the rendezvous server could not be reached, or the room closed).
    Closed(Option<u32>, String),
}

pub struct Link {
    tx: Sender<(u32, Vec<u8>)>,
    rx: Receiver<LinkEvent>,
}

impl Link {
    pub fn host(url: String) -> Link {
        Self::spawn(move |ev, out| host_thread(&url, ev, out))
    }

    pub fn join(url: String, code: String) -> Link {
        Self::spawn(move |ev, out| join_thread(&url, &code, ev, out))
    }

    fn spawn(f: impl FnOnce(&Sender<LinkEvent>, Receiver<(u32, Vec<u8>)>) -> Result<(), String> + Send + 'static) -> Link {
        let (tx, out_rx) = channel();
        let (ev_tx, rx) = channel();
        std::thread::spawn(move || {
            if let Err(e) = f(&ev_tx, out_rx) {
                let _ = ev_tx.send(LinkEvent::Closed(None, e));
            }
        });
        Link { tx, rx }
    }

    pub fn send(&self, peer: u32, bytes: &[u8]) {
        let _ = self.tx.send((peer, bytes.to_vec()));
    }

    pub fn poll(&self) -> Option<LinkEvent> {
        match self.rx.try_recv() {
            Ok(e) => Some(e),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(LinkEvent::Closed(None, "the connection closed".into())),
        }
    }
}

type Ws = WebSocket<MaybeTlsStream<std::net::TcpStream>>;

fn ws_connect(url: &str) -> Result<Ws, String> {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let (mut ws, _) = tungstenite::connect(url).map_err(|e| format!("cannot reach the rendezvous server: {}", e))?;
    let t = Some(Duration::from_millis(50));
    match ws.get_mut() {
        MaybeTlsStream::Plain(s) => s.set_read_timeout(t),
        MaybeTlsStream::Rustls(s) => s.sock.set_read_timeout(t),
        _ => Ok(()),
    }
    .map_err(|e| e.to_string())?;
    Ok(ws)
}

fn ws_timeout(ws: &mut Ws, t: Duration) {
    let _ = match ws.get_mut() {
        MaybeTlsStream::Plain(s) => s.set_read_timeout(Some(t)),
        MaybeTlsStream::Rustls(s) => s.sock.set_read_timeout(Some(t)),
        _ => Ok(()),
    };
}

fn ws_send(ws: &mut Ws, v: serde_json::Value) -> Result<(), String> {
    ws.send(Message::Text(v.to_string().into())).map_err(|e| format!("rendezvous server: {}", e))
}

fn ws_recv(ws: &mut Ws) -> Result<Option<serde_json::Value>, String> {
    match ws.read() {
        Ok(Message::Text(t)) => serde_json::from_str(t.as_str()).map(Some).map_err(|e| e.to_string()),
        Ok(Message::Close(_)) => Err("the rendezvous server closed the connection".into()),
        Ok(_) => Ok(None),
        Err(tungstenite::Error::Io(e)) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => Ok(None),
        Err(e) => Err(format!("rendezvous server: {}", e)),
    }
}

fn ws_wait(ws: &mut Ws, types: &[&str], timeout: Duration) -> Result<serde_json::Value, String> {
    let end = Instant::now() + timeout;
    while Instant::now() < end {
        if let Some(m) = ws_recv(ws)? {
            let ty = m["type"].as_str().unwrap_or("");
            if ty == "error" {
                return Err(match m["reason"].as_str() {
                    Some("no_room") => "There is no game with that code.".into(),
                    Some("full") => "That game is full.".into(),
                    _ => m["message"].as_str().unwrap_or("rendezvous error").to_string(),
                });
            }
            if ty == "closed" {
                return Err("The other player left.".into());
            }
            if types.contains(&ty) {
                return Ok(m);
            }
        }
    }
    Err("the rendezvous server did not answer".into())
}

fn stun_servers(ice: &serde_json::Value) -> Vec<String> {
    let mut out = Vec::new();
    for s in ice.as_array().into_iter().flatten() {
        let urls = match &s["urls"] {
            serde_json::Value::String(u) => vec![u.clone()],
            serde_json::Value::Array(a) => a.iter().filter_map(|u| u.as_str().map(String::from)).collect(),
            _ => vec![],
        };
        for u in urls {
            if let Some(rest) = u.strip_prefix("stun:") {
                out.push(rest.split('?').next().unwrap_or(rest).to_string());
            }
        }
    }
    if out.is_empty() {
        out.push(DEFAULT_STUN.into());
    }
    out
}

/// The address a STUN server sees for `sock` (RFC 5389 binding request,
/// XOR-MAPPED-ADDRESS), None if no server answers in time.
fn stun_mapped(sock: &UdpSocket, servers: &[String]) -> Option<SocketAddr> {
    let mut req = vec![0u8; 20];
    req[0..2].copy_from_slice(&1u16.to_be_bytes());
    req[4..8].copy_from_slice(&0x2112a442u32.to_be_bytes());
    let id: [u8; 12] = std::array::from_fn(|i| (Instant::now().elapsed().subsec_nanos() as u8).wrapping_add(i as u8 * 37));
    req[8..20].copy_from_slice(&id);
    let targets: Vec<SocketAddr> =
        servers.iter().filter_map(|s| s.to_socket_addrs().ok()?.find(|a| a.is_ipv4())).collect();
    let _ = sock.set_read_timeout(Some(Duration::from_millis(300)));
    let mut buf = [0u8; 512];
    for _ in 0..4 {
        for t in &targets {
            let _ = sock.send_to(&req, t);
        }
        while let Ok((n, _)) = sock.recv_from(&mut buf) {
            let b = &buf[..n];
            if n < 20 || b[0..2] != [0x01, 0x01] || b[8..20] != id {
                continue;
            }
            let mut p = 20;
            while p + 4 <= n {
                let ty = u16::from_be_bytes([b[p], b[p + 1]]);
                let len = u16::from_be_bytes([b[p + 2], b[p + 3]]) as usize;
                let v = b.get(p + 4..p + 4 + len)?;
                if (ty == 0x0020 || ty == 0x0001) && len >= 8 && v[1] == 1 {
                    let mut port = u16::from_be_bytes([v[2], v[3]]);
                    let mut ip = [v[4], v[5], v[6], v[7]];
                    if ty == 0x0020 {
                        port ^= 0x2112;
                        for (k, x) in ip.iter_mut().enumerate() {
                            *x ^= [0x21, 0x12, 0xa4, 0x42][k];
                        }
                    }
                    return Some(SocketAddr::from((ip, port)));
                }
                p += 4 + (len + 3) / 4 * 4;
            }
        }
    }
    None
}

/// This computer's address on its network: the source address of a route to the
/// internet (nothing is sent), else the loopback address.
fn local_ip() -> String {
    UdpSocket::bind("0.0.0.0:0")
        .and_then(|s| {
            s.connect("8.8.8.8:80")?;
            s.local_addr()
        })
        .map(|a| a.ip().to_string())
        .unwrap_or_else(|_| "127.0.0.1".into())
}

/// A UDP socket and an Rtc with its host and server-reflexive candidates.
fn new_rtc(stun: &[String]) -> Result<(UdpSocket, SocketAddr, Rtc), String> {
    static CRYPTO: std::sync::Once = std::sync::Once::new();
    CRYPTO.call_once(|| str0m::crypto::from_feature_flags().install_process_default());
    let sock = UdpSocket::bind("0.0.0.0:0").map_err(|e| e.to_string())?;
    let port = sock.local_addr().map_err(|e| e.to_string())?.port();
    let ip = local_ip().parse().map_err(|e: std::net::AddrParseError| e.to_string())?;
    let local = SocketAddr::new(ip, port);
    let mut rtc = Rtc::builder().build(Instant::now());
    let host = Candidate::host(local, "udp").map_err(|e| e.to_string())?;
    rtc.add_local_candidate(host);
    if let Some(mapped) = stun_mapped(&sock, stun) {
        if mapped != local {
            if let Ok(c) = Candidate::server_reflexive(mapped, local, "udp") {
                rtc.add_local_candidate(c);
            }
        }
    }
    Ok((sock, local, rtc))
}

const RELIABLE: &str = "lf2";
const FAST: &str = "lf2-fast";

fn fast_config() -> ChannelConfig {
    ChannelConfig { label: FAST.into(), ordered: false, reliability: Reliability::MaxRetransmits { retransmits: 0 }, ..Default::default() }
}

/// Test settings, from the environment: LF2_TEST_DROP is the percentage of UDP
/// datagrams dropped on arrival once the link is open, LF2_TEST_ALL_RELIABLE sends
/// every message on the reliable channel.
fn test_drop() -> u32 {
    std::env::var("LF2_TEST_DROP").ok().and_then(|s| s.parse().ok()).unwrap_or(0)
}

struct Conn {
    peer: u32,
    sock: UdpSocket,
    local: SocketAddr,
    rtc: Rtc,
    reliable: Option<ChannelId>,
    fast: Option<ChannelId>,
    open: bool,
    fast_open: bool,
    pending: std::collections::VecDeque<Vec<u8>>,
    /// Messages for the fast channel; the oldest are dropped when it is not
    /// keeping up, since the next INPUT message repeats them.
    pending_fast: std::collections::VecDeque<Vec<u8>>,
    start: Instant,
    buf: Vec<u8>,
    drop_percent: u32,
    all_reliable: bool,
    rng: u64,
}

impl Conn {
    fn new(peer: u32, (sock, local, rtc): (UdpSocket, SocketAddr, Rtc)) -> Result<Conn, String> {
        sock.set_nonblocking(true).map_err(|e| e.to_string())?;
        Ok(Conn {
            peer,
            sock,
            local,
            rtc,
            reliable: None,
            fast: None,
            open: false,
            fast_open: false,
            pending: Default::default(),
            pending_fast: Default::default(),
            start: Instant::now(),
            buf: vec![0; 2000],
            drop_percent: test_drop(),
            all_reliable: std::env::var_os("LF2_TEST_ALL_RELIABLE").is_some(),
            rng: 0x9e3779b97f4a7c15 ^ (peer as u64 + 1) ^ std::process::id() as u64,
        })
    }

    /// Queue a message from the game on the channel it needs.
    fn queue(&mut self, b: Vec<u8>) {
        if lf2_sim::net::is_unreliable(&b) && !self.all_reliable && self.fast_open {
            self.pending_fast.push_back(b);
            while self.pending_fast.len() > 8 {
                self.pending_fast.pop_front();
            }
        } else {
            self.pending.push_back(b);
        }
    }

    fn lose(&mut self) -> bool {
        if self.drop_percent == 0 || !self.open {
            return false;
        }
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        self.rng % 100 < self.drop_percent as u64
    }

    fn poll(&mut self, ev: &Sender<LinkEvent>) -> Result<(), String> {
        if self.open {
            if let Some(mut ch) = self.reliable.and_then(|c| self.rtc.channel(c)) {
                while let Some(b) = self.pending.front() {
                    match ch.write(true, b) {
                        Ok(true) => {
                            self.pending.pop_front();
                        }
                        _ => break,
                    }
                }
            }
        }
        if self.fast_open {
            if let Some(mut ch) = self.fast.and_then(|c| self.rtc.channel(c)) {
                while let Some(b) = self.pending_fast.front() {
                    match ch.write(true, b) {
                        Ok(true) => {
                            self.pending_fast.pop_front();
                        }
                        _ => break,
                    }
                }
            }
        }
        loop {
            match self.rtc.poll_output().map_err(|e| e.to_string())? {
                Output::Timeout(_) => break,
                Output::Transmit(t) => {
                    let _ = self.sock.send_to(&t.contents, t.destination);
                }
                Output::Event(e) => match e {
                    Event::ChannelOpen(id, label) => {
                        if label == FAST {
                            self.fast = Some(id);
                            self.fast_open = true;
                        } else {
                            self.reliable = Some(id);
                            self.open = true;
                            let _ = ev.send(LinkEvent::Connected(self.peer));
                        }
                    }
                    Event::ChannelData(d) => {
                        let _ = ev.send(LinkEvent::Data(self.peer, d.data));
                    }
                    Event::ChannelClose(id) => {
                        if Some(id) == self.fast {
                            self.fast_open = false;
                            self.pending_fast.clear();
                        } else {
                            return Err("The other player left.".into());
                        }
                    }
                    Event::IceConnectionStateChange(IceConnectionState::Disconnected) => return Err("Connection Lost!".into()),
                    _ => {}
                },
            }
        }
        if !self.open && self.start.elapsed() > Duration::from_secs(20) {
            return Err("Could not connect to the other computer (no direct route between the two networks).".into());
        }
        let mut got = false;
        while let Ok((n, source)) = self.sock.recv_from(&mut self.buf) {
            got = true;
            if self.lose() {
                continue;
            }
            if let Ok(contents) = self.buf[..n].try_into() {
                let r = Receive { proto: Protocol::Udp, source, destination: self.local, contents };
                self.rtc.handle_input(Input::Receive(Instant::now(), r)).map_err(|e| e.to_string())?;
            }
        }
        if !got {
            self.rtc.handle_input(Input::Timeout(Instant::now())).map_err(|e| e.to_string())?;
        }
        if !self.rtc.is_alive() {
            return Err("Connection Lost!".into());
        }
        Ok(())
    }
}

fn take_outgoing(out: &Receiver<(u32, Vec<u8>)>, conns: &mut [Conn]) -> bool {
    loop {
        match out.try_recv() {
            Ok((peer, b)) => {
                if let Some(c) = conns.iter_mut().find(|c| c.peer == peer) {
                    c.queue(b);
                }
            }
            Err(TryRecvError::Empty) => return true,
            Err(TryRecvError::Disconnected) => return false,
        }
    }
}

fn poll_all(conns: &mut Vec<Conn>, ev: &Sender<LinkEvent>) {
    conns.retain_mut(|c| match c.poll(ev) {
        Ok(()) => true,
        Err(e) => {
            let _ = ev.send(LinkEvent::Closed(Some(c.peer), e));
            false
        }
    });
}

fn host_thread(url: &str, ev: &Sender<LinkEvent>, out: Receiver<(u32, Vec<u8>)>) -> Result<(), String> {
    let mut ws = ws_connect(url)?;
    ws_send(&mut ws, serde_json::json!({"type": "host"}))?;
    let hosted = ws_wait(&mut ws, &["hosted"], Duration::from_secs(10))?;
    let code = hosted["code"].as_str().unwrap_or("").to_string();
    let _ = ev.send(LinkEvent::Code(code));
    let stun = stun_servers(&hosted["ice_servers"]);
    ws_timeout(&mut ws, Duration::from_millis(1));
    let mut conns: Vec<Conn> = Vec::new();
    // the server's seat numbers are reused once a joiner has left the room (a
    // joiner leaves it when its link is set up), so the joiners get their own
    let mut next_peer = 1;
    let mut last_ping = Instant::now();
    // once a joiner is connected, losing the rendezvous server only means that no
    // one else can join
    let mut ws_alive = true;
    loop {
        // the joiners' offers, keeping the room alive
        let got = if ws_alive { ws_recv(&mut ws) } else { Ok(None) };
        let got = match got {
            Ok(m) => m,
            Err(e) if conns.is_empty() => return Err(e),
            Err(_) => {
                ws_alive = false;
                None
            }
        };
        if let Some(m) = got {
            match m["type"].as_str() {
                Some("offer") => {
                    let seat = m["seat"].clone();
                    let rtc = new_rtc(&stun)?;
                    let mut c = Conn::new(next_peer, rtc)?;
                    next_peer += 1;
                    let sdp: SdpOffer = serde_json::from_value(m["offer"].clone()).map_err(|e| format!("bad offer: {}", e))?;
                    let answer = c.rtc.sdp_api().accept_offer(sdp).map_err(|e| e.to_string())?;
                    ws_send(&mut ws, serde_json::json!({"type": "answer", "seat": seat, "answer": answer}))?;
                    conns.push(c);
                }
                Some("error") => return Err(m["message"].as_str().unwrap_or("rendezvous error").to_string()),
                _ => {}
            }
        }
        if ws_alive && last_ping.elapsed() > Duration::from_secs(25) {
            let _ = ws.send(Message::Ping(Vec::new().into()));
            last_ping = Instant::now();
        }
        if !take_outgoing(&out, &mut conns) {
            // the game dropped the link: the room is not needed any more
            let _ = ws_send(&mut ws, serde_json::json!({"type": "close"}));
            return Ok(());
        }
        poll_all(&mut conns, ev);
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn join_thread(url: &str, code: &str, ev: &Sender<LinkEvent>, out: Receiver<(u32, Vec<u8>)>) -> Result<(), String> {
    let mut ws = ws_connect(url)?;
    // the ICE servers come with "joined", after the offer; ask for them first
    ws_send(&mut ws, serde_json::json!({"type": "ice"}))?;
    let stun = match ws_wait(&mut ws, &["ice_servers"], Duration::from_secs(3)) {
        Ok(m) => stun_servers(&m["ice_servers"]),
        Err(_) => vec![DEFAULT_STUN.to_string()],
    };
    let (sock, local, mut rtc) = new_rtc(&stun)?;
    let mut api = rtc.sdp_api();
    api.add_channel(RELIABLE.into());
    api.add_channel_with_config(fast_config());
    let (offer, pending) = api.apply().ok_or("no offer")?;
    let code = code.trim().to_ascii_uppercase();
    ws_send(&mut ws, serde_json::json!({"type": "join", "code": code, "offer": offer}))?;
    ws_wait(&mut ws, &["joined"], Duration::from_secs(10))?;
    let m = ws_wait(&mut ws, &["answer"], Duration::from_secs(30))?;
    let answer: SdpAnswer = serde_json::from_value(m["answer"].clone()).map_err(|e| format!("bad answer: {}", e))?;
    rtc.sdp_api().accept_answer(pending, answer).map_err(|e| e.to_string())?;
    drop(ws);
    let mut conns = vec![Conn::new(0, (sock, local, rtc))?];
    loop {
        if !take_outgoing(&out, &mut conns) {
            return Ok(());
        }
        poll_all(&mut conns, ev);
        if conns.is_empty() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}
