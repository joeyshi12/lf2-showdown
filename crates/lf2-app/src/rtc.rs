//! A two-computer link over a WebRTC data channel, set up through an icebreaker
//! rendezvous server (https://github.com/joeyshi12/icebreaker): the host reserves a
//! room and gets a four-character code, the joiner sends an offer with the code, the
//! host answers. After that the server is out of the way and the bytes go peer to
//! peer over UDP. The link carries the same byte stream the TCP game does (see
//! lf2_sim::net), as messages that the receiver joins back together.
//!
//! Candidates are not trickled (icebreaker passes only the offer and the answer), so
//! both SDPs carry them: the host candidate of the UDP socket and the server-reflexive
//! one a STUN server reports for it.

use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use std::time::{Duration, Instant};
use str0m::change::{SdpAnswer, SdpOffer};
use str0m::channel::ChannelId;
use str0m::net::{Protocol, Receive};
use str0m::{Candidate, Event, IceConnectionState, Input, Output, Rtc};
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Message, WebSocket};

/// The rendezvous server, overridable with LF2_RENDEZVOUS.
pub const DEFAULT_URL: &str = "wss://rv.lf2-showdown.online/ws?app=lf2-showdown";
const DEFAULT_STUN: &str = "stun.l.google.com:19302";

pub fn rendezvous_url() -> String {
    std::env::var("LF2_RENDEZVOUS").unwrap_or_else(|_| DEFAULT_URL.into())
}

/// What the link reports.
#[derive(Debug)]
pub enum LinkEvent {
    /// Host: the room code to give the other player.
    Code(String),
    /// The data channel is open.
    Connected,
    /// Bytes from the other computer.
    Data(Vec<u8>),
    /// The link failed or closed, with a message for the player.
    Closed(String),
}

/// The game side of a link: send bytes, poll events. The link runs on its own thread.
pub struct Link {
    tx: Sender<Vec<u8>>,
    rx: Receiver<LinkEvent>,
}

impl Link {
    pub fn host(url: String) -> Link {
        Self::spawn(move |ev, out| host_thread(&url, ev, out))
    }

    pub fn join(url: String, code: String) -> Link {
        Self::spawn(move |ev, out| join_thread(&url, &code, ev, out))
    }

    fn spawn(f: impl FnOnce(&Sender<LinkEvent>, Receiver<Vec<u8>>) -> Result<(), String> + Send + 'static) -> Link {
        let (tx, out_rx) = channel();
        let (ev_tx, rx) = channel();
        std::thread::spawn(move || {
            if let Err(e) = f(&ev_tx, out_rx) {
                let _ = ev_tx.send(LinkEvent::Closed(e));
            }
        });
        Link { tx, rx }
    }

    pub fn send(&self, bytes: &[u8]) {
        let _ = self.tx.send(bytes.to_vec());
    }

    pub fn poll(&self) -> Option<LinkEvent> {
        match self.rx.try_recv() {
            Ok(e) => Some(e),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(LinkEvent::Closed("the connection closed".into())),
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

fn ws_send(ws: &mut Ws, v: serde_json::Value) -> Result<(), String> {
    ws.send(Message::Text(v.to_string().into())).map_err(|e| format!("rendezvous server: {}", e))
}

/// The next JSON message, None on a read timeout.
fn ws_recv(ws: &mut Ws) -> Result<Option<serde_json::Value>, String> {
    match ws.read() {
        Ok(Message::Text(t)) => serde_json::from_str(t.as_str()).map(Some).map_err(|e| e.to_string()),
        Ok(Message::Close(_)) => Err("the rendezvous server closed the connection".into()),
        Ok(_) => Ok(None),
        Err(tungstenite::Error::Io(e)) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => Ok(None),
        Err(e) => Err(format!("rendezvous server: {}", e)),
    }
}

/// Wait for a message of one of these types; an "error" message fails.
fn ws_wait(ws: &mut Ws, types: &[&str], timeout: Duration) -> Result<serde_json::Value, String> {
    let end = Instant::now() + timeout;
    while Instant::now() < end {
        if let Some(m) = ws_recv(ws)? {
            let ty = m["type"].as_str().unwrap_or("");
            if ty == "error" {
                return Err(match m["reason"].as_str() {
                    Some("no_room") => "There is no game with that code.".into(),
                    Some("full") => "That game already has two players.".into(),
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

/// The STUN servers of an ice_servers list, as host:port.
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

fn host_thread(url: &str, ev: &Sender<LinkEvent>, out: Receiver<Vec<u8>>) -> Result<(), String> {
    let mut ws = ws_connect(url)?;
    ws_send(&mut ws, serde_json::json!({"type": "host"}))?;
    let hosted = ws_wait(&mut ws, &["hosted"], Duration::from_secs(10))?;
    let code = hosted["code"].as_str().unwrap_or("").to_string();
    let _ = ev.send(LinkEvent::Code(code));
    let stun = stun_servers(&hosted["ice_servers"]);
    // wait for a joiner's offer, keeping the room alive
    let mut last_ping = Instant::now();
    let offer = loop {
        if let Some(m) = ws_recv(&mut ws)? {
            match m["type"].as_str() {
                Some("offer") => break m,
                Some("error") => return Err(m["message"].as_str().unwrap_or("rendezvous error").to_string()),
                _ => {}
            }
        }
        if last_ping.elapsed() > Duration::from_secs(25) {
            let _ = ws.send(Message::Ping(Vec::new().into()));
            last_ping = Instant::now();
        }
        // the game cancelled (the link was dropped)
        if let Err(TryRecvError::Disconnected) = out.try_recv() {
            let _ = ws_send(&mut ws, serde_json::json!({"type": "close"}));
            return Ok(());
        }
    };
    let (sock, local, mut rtc) = new_rtc(&stun)?;
    let sdp: SdpOffer = serde_json::from_value(offer["offer"].clone()).map_err(|e| format!("bad offer: {}", e))?;
    let answer = rtc.sdp_api().accept_offer(sdp).map_err(|e| e.to_string())?;
    let seat = offer["seat"].clone();
    ws_send(&mut ws, serde_json::json!({"type": "answer", "seat": seat, "answer": answer}))?;
    let r = run(sock, local, rtc, None, ev, out);
    // the room is not needed any more
    let _ = ws_send(&mut ws, serde_json::json!({"type": "close"}));
    r
}

fn join_thread(url: &str, code: &str, ev: &Sender<LinkEvent>, out: Receiver<Vec<u8>>) -> Result<(), String> {
    let mut ws = ws_connect(url)?;
    // the ICE servers come with "joined", after the offer; ask for them first
    ws_send(&mut ws, serde_json::json!({"type": "ice"}))?;
    let stun = match ws_wait(&mut ws, &["ice_servers"], Duration::from_secs(3)) {
        Ok(m) => stun_servers(&m["ice_servers"]),
        Err(_) => vec![DEFAULT_STUN.to_string()],
    };
    let (sock, local, mut rtc) = new_rtc(&stun)?;
    let mut api = rtc.sdp_api();
    let cid = api.add_channel("lf2".into());
    let (offer, pending) = api.apply().ok_or("no offer")?;
    let code = code.trim().to_ascii_uppercase();
    ws_send(&mut ws, serde_json::json!({"type": "join", "code": code, "offer": offer}))?;
    ws_wait(&mut ws, &["joined"], Duration::from_secs(10))?;
    let m = ws_wait(&mut ws, &["answer"], Duration::from_secs(30))?;
    let answer: SdpAnswer = serde_json::from_value(m["answer"].clone()).map_err(|e| format!("bad answer: {}", e))?;
    rtc.sdp_api().accept_answer(pending, answer).map_err(|e| e.to_string())?;
    drop(ws);
    run(sock, local, rtc, Some(cid), ev, out)
}

/// Drive the Rtc until the link closes: UDP in and out, the channel's bytes to the
/// game, the game's bytes to the channel.
fn run(sock: UdpSocket, local: SocketAddr, mut rtc: Rtc, mut cid: Option<ChannelId>, ev: &Sender<LinkEvent>, out: Receiver<Vec<u8>>) -> Result<(), String> {
    let mut buf = vec![0u8; 2000];
    let mut open = false;
    let mut pending: std::collections::VecDeque<Vec<u8>> = Default::default();
    let start = Instant::now();
    loop {
        // the game's bytes
        loop {
            match out.try_recv() {
                Ok(b) => pending.push_back(b),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return Ok(()),
            }
        }
        if open {
            if let Some(mut ch) = cid.and_then(|c| rtc.channel(c)) {
                while let Some(b) = pending.front() {
                    match ch.write(true, b) {
                        Ok(true) => {
                            pending.pop_front();
                        }
                        _ => break,
                    }
                }
            }
        }
        let deadline = loop {
            match rtc.poll_output().map_err(|e| e.to_string())? {
                Output::Timeout(t) => break t,
                Output::Transmit(t) => {
                    let _ = sock.send_to(&t.contents, t.destination);
                }
                Output::Event(e) => match e {
                    Event::ChannelOpen(id, _) => {
                        cid = Some(id);
                        open = true;
                        let _ = ev.send(LinkEvent::Connected);
                    }
                    Event::ChannelData(d) => {
                        let _ = ev.send(LinkEvent::Data(d.data));
                    }
                    Event::ChannelClose(_) => return Err("The other player left.".into()),
                    Event::IceConnectionStateChange(IceConnectionState::Disconnected) => {
                        return Err("Connection Lost!".into())
                    }
                    _ => {}
                },
            }
        };
        if !open && start.elapsed() > Duration::from_secs(20) {
            return Err("Could not connect to the other computer (no direct route between the two networks).".into());
        }
        let wait = deadline.saturating_duration_since(Instant::now()).clamp(Duration::from_millis(1), Duration::from_millis(5));
        let _ = sock.set_read_timeout(Some(wait));
        let input = match sock.recv_from(&mut buf) {
            Ok((n, source)) => match buf[..n].try_into() {
                Ok(contents) => Input::Receive(Instant::now(), Receive { proto: Protocol::Udp, source, destination: local, contents }),
                Err(_) => Input::Timeout(Instant::now()),
            },
            Err(_) => Input::Timeout(Instant::now()),
        };
        rtc.handle_input(input).map_err(|e| e.to_string())?;
        if !rtc.is_alive() {
            return Err("Connection Lost!".into());
        }
    }
}
