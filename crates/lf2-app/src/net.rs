//! The network game on the desktop: WebRTC links (rtc.rs) carrying the messages of
//! lf2_sim::net::NetSession.

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

    /// MenuRequest::NetStart: the host starts the game.
    pub fn start(&mut self, world: &mut World) {
        if let Some((_, s)) = self.link.as_mut() {
            s.start(world);
        }
        self.flush();
    }

    /// A message for the player since the last call (a failed connection).
    pub fn take_error(&mut self) -> Option<String> {
        self.error.take()
    }

    /// True while the game runs over the network: call `advance` instead of
    /// `World::step`.
    pub fn in_game(&self) -> bool {
        self.link.as_ref().is_some_and(|(_, s)| s.in_game())
    }

    pub fn session(&self) -> Option<&NetSession> {
        self.link.as_ref().map(|(_, s)| s)
    }

    /// Pass the links' events to the session and its messages to the links. Call
    /// once per frame.
    pub fn pump(&mut self, world: &mut World) {
        let Some((link, s)) = self.link.as_mut() else { return };
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
                        self.error = Some(msg);
                        world.net_connect_failed();
                    }
                    if p.is_none() || !s.is_host() {
                        break;
                    }
                }
            }
        }
        if let Some(e) = s.error.take() {
            self.error = Some(e);
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

    /// Send the inputs again and run the frames whose guesses were wrong, without
    /// new frames (for tests that stop at a frame).
    pub fn settle(&mut self, world: &mut World) {
        if let Some((_, s)) = self.link.as_mut() {
            s.settle(world);
        }
        self.flush();
    }

    /// One frame of the game over the network (NetSession::advance): the number
    /// of new frames.
    pub fn advance(&mut self, world: &mut World, keys: &mut [u8; 256]) -> u32 {
        let n = match self.link.as_mut() {
            Some((_, s)) => s.advance(world, keys),
            None => 0,
        };
        self.flush();
        n
    }
}
