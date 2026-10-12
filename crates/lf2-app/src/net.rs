//! The network game on the desktop: WebRTC links (rtc.rs) carrying the messages of
//! lf2_sim::net::NetSession.

use crate::rtc::{rendezvous_url, Link, LinkEvent};
use lf2_sim::net::NetSession;
use lf2_sim::World;

pub struct Net {
    link: Option<(Link, NetSession)>,
    start: std::time::Instant,
}

impl Default for Net {
    fn default() -> Net {
        Net { link: None, start: std::time::Instant::now() }
    }
}

impl Net {
    pub fn host(&mut self) {
        self.link = Some((Link::host(rendezvous_url()), NetSession::host()));
    }

    pub fn connect(&mut self, world: &mut World) {
        self.link = Some((Link::join(rendezvous_url(), world.room_code_typed()), NetSession::join()));
    }

    pub fn close(&mut self) {
        self.link = None;
    }

    pub fn start(&mut self, world: &mut World) {
        if let Some((_, s)) = self.link.as_mut() {
            s.start(world);
        }
        self.flush();
    }

    pub fn in_game(&self) -> bool {
        self.link.as_ref().is_some_and(|(_, s)| s.in_game())
    }

    pub fn session(&self) -> Option<&NetSession> {
        self.link.as_ref().map(|(_, s)| s)
    }

    pub fn pump(&mut self, world: &mut World) {
        let Some((link, s)) = self.link.as_mut() else { return };
        s.set_clock(self.start.elapsed().as_millis() as u32);
        while let Some(e) = link.poll() {
            match e {
                LinkEvent::Code(c) => s.on_code(world, &c),
                LinkEvent::Connected(p) => s.on_connected(world, p),
                LinkEvent::Data(p, d) => s.on_message(world, p, &d),
                LinkEvent::Closed(p, msg) => {
                    // all links: the host lost the rendezvous server, or a joiner
                    // its only link
                    s.on_closed(world, p.unwrap_or(0), &msg);
                    if p.is_none() && s.is_host() && !s.in_game() {
                        world.net_connect_failed(&msg);
                    }
                    if p.is_none() || !s.is_host() {
                        break;
                    }
                }
            }
        }
        self.flush();
    }

    fn flush(&mut self) {
        if let Some((link, s)) = self.link.as_mut() {
            for (peer, b) in s.take_outgoing() {
                link.send(peer, &b);
            }
        }
    }

    pub fn settle(&mut self, world: &mut World) {
        if let Some((_, s)) = self.link.as_mut() {
            s.settle(world);
        }
        self.flush();
    }

    pub fn advance(&mut self, world: &mut World, keys: &mut [u8; 256]) -> u32 {
        let n = match self.link.as_mut() {
            Some((_, s)) => s.advance(world, keys),
            None => 0,
        };
        self.flush();
        n
    }
}
