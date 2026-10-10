//! Two worlds play a network game against each other in one process: one as the
//! server, one as the client, with the handshake and the packets passed directly
//! instead of over sockets. They must go through the title screens, the main menu,
//! the pick screen and a VS match together, with the same objects on every frame
//! and no check failing (counter, version, hp sum, data checksum).
use lf2_sim::menu::main_menu::{MenuRequest, MOUSE_CLICK, MOUSE_X, MOUSE_Y};
use lf2_sim::World;
use std::sync::Arc;

struct Pair {
    s: World,
    c: World,
    s_keys: [u8; 256],
    c_keys: [u8; 256],
    frames: usize,
}

fn messages(w: &mut World) -> Vec<String> {
    let mut out = Vec::new();
    for r in std::mem::take(&mut w.requests) {
        match r {
            MenuRequest::Message(m) => out.push(m.to_string()),
            MenuRequest::NetHost => lf2_sim::net::set_room_code(w, "AB2C"),
            _ => {}
        }
    }
    out
}

impl Pair {
    /// One frame on both computers; the exchange frames pass the packets across.
    fn frame(&mut self) {
        self.s.set_keys(&self.s_keys);
        self.c.set_keys(&self.c_keys);
        self.s.step();
        self.c.step();
        assert_eq!(self.s.net_waiting, self.c.net_waiting, "frame {}: only one side exchanges", self.frames);
        if self.s.net_waiting {
            let (ps, pc) = (self.s.net_out.take().unwrap(), self.c.net_out.take().unwrap());
            self.c.net_resume(Some(&ps));
            self.s.net_resume(Some(&pc));
        }
        // the frames write 'u' over keys they consumed
        for i in 0..256u32 {
            self.s_keys[i as usize] = self.s.g.m.u8(lf2_sim::world::KEYS + i);
            self.c_keys[i as usize] = self.c.g.m.u8(lf2_sim::world::KEYS + i);
        }
        let (ms, mc) = (messages(&mut self.s), messages(&mut self.c));
        assert!(ms.is_empty() && mc.is_empty(), "frame {}: server {:?} client {:?}", self.frames, ms, mc);
        self.frames += 1;
    }

    fn frames(&mut self, n: usize) {
        for _ in 0..n {
            self.frame();
        }
    }

    fn click(w: &mut World, x: i32, y: i32) {
        w.g.m.set_i32(MOUSE_X, x);
        w.g.m.set_i32(MOUSE_Y, y);
        w.g.m.set_i32(MOUSE_CLICK, 1);
    }

    /// A key held for 3 frames, then let go for 7, on the server (`server`) or client.
    fn tap(&mut self, server: bool, vk: usize) {
        for k in 0..10 {
            let v = if k < 3 { b'd' } else { b'u' };
            if server {
                self.s_keys[vk] = v;
            } else {
                self.c_keys[vk] = v;
            }
            self.frame();
        }
    }
}

#[test]
fn server_and_client_stay_in_step() {
    let Some(dir) = lf2_data::game_dir_from_env() else { return };
    let exe = lf2_data::read_exe(&dir).unwrap();
    let gd = Arc::new(lf2_data::GameData::load(&dir).unwrap());
    let controls = lf2_data::controls::load(&dir).unwrap_or_default();
    let mut s = World::new_title(gd.clone(), &exe, &controls, None, 11);
    let mut c = World::new_title(gd.clone(), &exe, &controls, None, 22);
    s.ads_removed = true;
    c.ads_removed = true;
    let mut p = Pair { s, c, s_keys: [b'u'; 256], c_keys: [b'u'; 256], frames: 0 };
    // both: title screen, "network game" (x 0x114..0x208, y y0+0xf7..y0+0x110)
    for w in [&mut p.s, &mut p.c] {
        for _ in 0..5 {
            w.step();
        }
        let y0 = w.g.m.i32(lf2_sim::menu::main_menu::MENU_Y);
        Pair::click(w, 0x150, y0 + 0x100);
        w.step();
        messages(w);
        w.g.m.set_i32(MOUSE_CLICK, 0);
        w.step();
        assert_eq!(w.g.m.i32(0x44d064), 1, "network screen");
    }
    // server: "server" (hosts a room); client: "client", type the room code, Enter
    Pair::click(&mut p.s, 0x150, 0x118);
    p.s.step();
    p.s.g.m.set_i32(MOUSE_CLICK, 0);
    assert!(p.s.requests.iter().any(|r| *r == MenuRequest::NetHost), "server hosts a room");
    messages(&mut p.s);
    p.s.step();
    assert_eq!(p.s.g.m.i32(0x44d064), 2, "server waits");
    assert_eq!(p.s.g.m.u8(lf2_sim::net::LOCAL_IP), b'A', "the room code is shown");
    Pair::click(&mut p.c, 0x150, 0x138);
    p.c.step();
    p.c.g.m.set_i32(MOUSE_CLICK, 0);
    p.c.step();
    assert_eq!(p.c.g.m.i32(0x44d064), 3, "client asks for the address");
    let mut keys = [b'u'; 256];
    for &vk in b"AB2C" {
        keys[vk as usize] = b'd';
        p.c.set_keys(&keys);
        p.c.step();
        keys[vk as usize] = b'u';
        p.c.set_keys(&keys);
        p.c.step();
    }
    assert_eq!(p.c.room_code_typed().to_ascii_uppercase(), "AB2C");
    keys[0x0d] = b'd';
    p.c.set_keys(&keys);
    p.c.step();
    keys[0x0d] = b'u';
    p.c.set_keys(&keys);
    p.c.step();
    assert!(p.c.requests.iter().any(|r| *r == MenuRequest::NetConnect), "client connects");
    p.c.requests.clear();
    // the handshake
    let hello = p.c.net_hello();
    p.s.net_server_joined(&hello);
    let (reply, table) = (p.s.net_reply(), p.s.net_table());
    p.c.net_client_joined(&reply, &table);
    // the server's next frame starts the game; the client's state 4 does it too
    p.s.step();
    p.c.step();
    assert_eq!(p.s.g.m.i32(lf2_sim::world::GAME), 1);
    assert_eq!(p.c.g.m.i32(lf2_sim::world::GAME), 1);
    // from here both run in step: loading, the main menu
    p.frames(20);
    assert_eq!(p.s.g.match_phase(), 10, "main menu");
    // VS mode (player 1 of the server presses attack), the pick screen: player 1 of
    // the server and player 1 of the client join and pick a fighter
    let j = 0x4a;
    p.tap(true, j);
    p.frames(10);
    for _ in 0..4 {
        p.tap(true, j);
    }
    for _ in 0..4 {
        p.tap(false, j);
    }
    p.frames(260);

    // no computer players, then the menu: up twice to "Fight!" (on the client) and
    // attack (on the server)
    p.tap(true, j);
    p.frames(10);
    p.tap(false, 0x57);
    p.tap(false, 0x57);
    p.tap(true, j);
    p.frames(100);

    assert_eq!(p.s.g.match_phase(), 0, "the match runs");
    // fight for a while: the server's player runs right and punches, the client's
    // player walks left and jumps
    for n in 0..600 {
        // walk towards each other (each computer's player 1 is in its own slots)
        let (sx, cx) = (p.s.objs[0].ix(), p.s.objs[4].ix());
        p.s_keys[0x44] = if sx < cx - 60 { b'd' } else { b'u' };
        p.s_keys[0x41] = if sx > cx + 60 { b'd' } else { b'u' };
        p.c_keys[0x41] = if cx > sx + 60 { b'd' } else { b'u' };
        p.c_keys[0x44] = if cx < sx - 60 { b'd' } else { b'u' };
        let (sz, cz) = (p.s.objs[0].iz(), p.s.objs[4].iz());
        p.s_keys[0x53] = if sz < cz - 5 { b'd' } else { b'u' };
        p.s_keys[0x57] = if sz > cz + 5 { b'd' } else { b'u' };
        p.s_keys[j] = if n % 6 < 2 { b'd' } else { b'u' };
        p.c_keys[j] = if n % 9 < 2 { b'd' } else { b'u' };
        p.c_keys[0x4b] = if n % 37 < 2 { b'd' } else { b'u' };
        p.frame();
        for i in 0..400 {
            assert_eq!(p.s.exists(i), p.c.exists(i), "frame {}: object {} exists on one side", p.frames, i);
            if p.s.exists(i) {
                let (a, b) = (&p.s.objs[i], &p.c.objs[i]);
                assert!(
                    a.x() == b.x() && a.y() == b.y() && a.z() == b.z() && a.hp() == b.hp() && a.frame() == b.frame(),
                    "frame {}: object {} differs",
                    p.frames,
                    i
                );
            }
        }
    }
    let hurt = (0..8).any(|i| p.s.exists(i) && p.s.objs[i].hp() < p.s.objs[i].max_hp());
    assert!(hurt, "the fighters hit each other");
}
