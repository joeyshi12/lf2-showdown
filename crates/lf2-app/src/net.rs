//! The network game on the desktop: a WebRTC link (rtc.rs) carrying the byte stream
//! of lf2_sim::net::NetSession. The original blocks inside a frame while it waits
//! for the other computer; here the main loop stops stepping the world instead and
//! keeps the window alive.

use crate::rtc::{rendezvous_url, Link, LinkEvent};
use lf2_sim::net::NetSession;
use lf2_sim::World;

#[derive(Default)]
pub struct Net {
    link: Option<(Link, NetSession)>,
    error: Option<String>,
}

impl Net {
    /// MenuRequest::NetHost.
    pub fn host(&mut self) {
        self.link = Some((Link::host(rendezvous_url()), NetSession::host()));
    }

    /// MenuRequest::NetConnect: join the room whose code the player typed.
    pub fn connect(&mut self, world: &mut World) {
        self.link = Some((Link::join(rendezvous_url(), world.room_code_typed()), NetSession::join()));
    }

    /// MenuRequest::NetClose.
    pub fn close(&mut self) {
        self.link = None;
    }

    /// A message for the player since the last call (a failed connection).
    pub fn take_error(&mut self) -> Option<String> {
        self.error.take()
    }

    /// Pass the link's events to the session and its bytes to the link.
    fn pump(&mut self, world: &mut World) -> Option<&mut NetSession> {
        let (link, s) = self.link.as_mut()?;
        while let Some(e) = link.poll() {
            match e {
                LinkEvent::Code(c) => s.on_code(world, &c),
                LinkEvent::Connected => s.on_connected(),
                LinkEvent::Data(d) => s.on_data(&d),
                LinkEvent::Closed(msg) => {
                    s.on_closed(world, &msg);
                    break;
                }
            }
        }
        if let Some(e) = s.error.take() {
            self.error = Some(e);
        }
        Some(s)
    }

    fn flush(&mut self) {
        if let Some((link, s)) = self.link.as_mut() {
            for b in s.take_outgoing() {
                link.send(&b);
            }
        }
    }

    /// Between frames, before the game runs: move the handshake on. True while the
    /// world must not run.
    pub fn handshake_busy(&mut self, world: &mut World) -> bool {
        let busy = self.pump(world).map(|s| s.handshake_busy(world)).unwrap_or(false);
        self.flush();
        busy
    }

    /// Call when `World::step` stopped at the exchange: send this computer's packet.
    pub fn begin_exchange(&mut self, world: &mut World) {
        if let Some((_, s)) = self.link.as_mut() {
            s.begin_exchange(world);
        }
        self.flush();
    }

    /// The other computer's packet: Some(Some(packet)) when it has come, Some(None)
    /// when the connection closed, None while waiting.
    pub fn poll_exchange(&mut self, world: &mut World) -> Option<Option<Vec<u8>>> {
        match self.pump(world) {
            Some(s) => s.poll_exchange(),
            None => Some(None),
        }
    }
}
