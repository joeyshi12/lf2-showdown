//! Battle mode: `battle_mode_tick` (0x43a860). Two armies (team 1 starting on the
//! left, team 2 on the right) get reinforcements from their reserves while fewer than
//! the maximum of each kind are alive. The battle is decided (0x451b7c = 1) when a
//! side has no living character and no reserve left.
//!
//! The kinds are `DAT_0044d350[n]` (n = `DAT_0044d37c`); per side s and kind k the
//! reserve is `DAT_0044d6a8[s*n + k]` and the most alive at once `DAT_0044d700[s*n + k]`.
//! The function also builds the army description lines ("Balanced(S)    Defense: 1.0")
//! in 0x451c80, which the port writes as well because the buffer is in .data.

use crate::world::{World, MAX_OBJECTS};

const N_KINDS: u32 = 0x44d37c;
const KINDS: u32 = 0x44d350;
const RESERVE: u32 = 0x44d6a8;
const MAX_ALIVE: u32 = 0x44d700;
const DECIDED: u32 = 0x451b7c;
const LINE: u32 = 0x451c80;
const DEFENSE_TEXT: u32 = 0x451bb8;

impl World {
    /// 0x43a860.
    pub(crate) fn battle_mode_tick(&mut self) {
        let n = self.g.m.i32(N_KINDS);
        let kinds: Vec<i32> = (0..n.max(0) as u32).map(|k| self.g.m.i32(KINDS + 4 * k)).collect();
        // living troops of each (side, kind), counted from object 20 on
        let mut alive = vec![0i32; (2 * n.max(0)) as usize];
        let (mut men1, mut men2) = (0, 0);
        for j in 0..MAX_OBJECTS {
            if !self.exists(j) {
                continue;
            }
            let side = self.objs[j].m.i32(0x344);
            if 0 < side && side < 3 && 0x13 < j {
                let id = self.data_of(j).id;
                if ((id.wrapping_sub(0x1e) as u32) < 10 && id != 0x26) || id == 0x7a || id == 0x7b {
                    for (k, &kid) in kinds.iter().enumerate() {
                        if kid == id {
                            alive[(side as usize - 1) * n as usize + k] += 1;
                        }
                    }
                }
            }
            let o = &self.objs[j];
            if self.data_of(j).kind == 0 && 0 < o.hp() {
                if o.team() == 1 {
                    men1 += 1;
                } else {
                    men2 += 1;
                }
            }
        }
        // one reinforcement per (side, kind) and tick
        let mut base = 0;
        while base <= n {
            for k in 0..n.max(0) {
                let idx = (base + k) as u32;
                let reserve = self.g.m.i32(RESERVE + 4 * idx);
                if reserve <= 0 || self.g.m.i32(MAX_ALIVE + 4 * idx) <= alive[idx as usize] {
                    continue;
                }
                let Some(j) = (0x14..MAX_OBJECTS).find(|&j| !self.exists(j)) else { continue };
                let Some(data) = self.data_index(kinds[k as usize]) else { continue };
                self.battle_spawn(j, data, kinds[k as usize], base, n);
                self.g.m.add_i32(RESERVE + 4 * idx, -1);
            }
            if n == 0 {
                break;
            }
            base += n;
        }
        // reserves of each side, leaving out the last two kinds (the drinks)
        let mut reserves = [0i32; 2];
        for (s, r) in reserves.iter_mut().enumerate() {
            for k in 0..(n - 2).max(0) {
                *r += self.g.m.i32(RESERVE + 4 * (s as u32 * n as u32 + k as u32));
            }
        }
        let decided = !((0 < reserves[0] && 0 < reserves[1]) || (0 < men1 && 0 < men2));
        self.g.m.set_i32(DECIDED, decided as i32);
        // the "Man: ... HP: ..." lines are sprintf'd on the stack and only drawn
        for (army, size, defense) in [(0x44d380u32, 0x451b74u32, 0x44d758u32), (0x44d384, 0x451b78, 0x44d75c)] {
            self.battle_line(self.g.m.i32(army), self.g.m.i32(size), self.g.m.i32(defense));
        }
    }

    /// Spawn reinforcement `id` (data `data`) in slot `j` for the side starting at `base`.
    fn battle_spawn(&mut self, j: usize, data: usize, id: i32, base: i32, n: i32) {
        self.set_exists(j, true);
        let weapon_hp = self.gd.objects[data].weapon_hp;
        let kind = self.gd.objects[data].kind;
        let (width, (z0, z1)) = {
            let b = self.background();
            (b.width, b.zboundary)
        };
        {
            let o = &mut self.objs[j];
            crate::collisions::obj_reset(o);
            o.m.set_i32(0x368, data as i32);
            o.set_x(350.0);
            o.set_weapon_hp(weapon_hp);
            o.set_y(0.0);
            o.set_z(300.0);
        }
        let r = self.random(0x128, z1 - z0);
        let o = &mut self.objs[j];
        o.set_iz(r + z0);
        let ix = match (base == 0, kind == 0) {
            (true, true) => -100,
            (true, false) => 50,
            (false, true) => width + 100,
            (false, false) => width - 0x32,
        };
        o.set_ix(ix);
        o.set_x(ix as f64);
        o.set_z(o.iz() as f64);
        let mut hp = 500;
        let mut tail = true; // LAB_0043ac48
        if base == 0 && kind == 0 {
            o.m.set_i32(0x318, 0x8c);
        }
        if base == 0 && kind == 0 && id == 0x25 {
            o.m.set_i32(0x318, 0x72);
            hp = 200;
        } else if id == 0x24 {
            hp = 0xfa;
            tail = false;
        } else if id == 0x25 || id == 0x23 || id == 0x20 {
            hp = 200;
        }
        if tail {
            if id == 0x27 || id == 0x21 {
                hp = 0x96;
            }
            if id == 0x22 {
                hp = 100;
            } else {
                if id == 0x1f || id == 0x1e {
                    hp = 0x32;
                }
                if id == 0x7a {
                    hp = 200;
                }
            }
        }
        o.set_mp(500);
        o.set_dark_hp(hp);
        o.set_hp(hp);
        o.set_max_hp(hp);
        o.m.set_i32(0x354, j as i32);
        let side = base / n + 1;
        if kind == 0 || kind == 5 {
            o.set_u_08(0x14);
            o.m.set_i8(0x80, (width / 2 < o.ix()) as i8);
            o.set_team(side);
            o.m.set_i32(0x344, side);
            o.set_iy(-300);
            o.set_y(o.iy() as f64);
            o.set_vy(0.0);
        } else {
            o.set_u_08(0);
            o.set_team(0);
            o.m.set_i32(0x344, side);
            o.set_iy(-300);
            o.set_y(o.iy() as f64);
        }
    }

    /// Build one army line in 0x451c80: army type, size, and "Defense: x.y" when the
    /// defense is not 100. Bytes past the terminator keep what was there.
    fn battle_line(&mut self, army: i32, size: i32, defense: i32) {
        let mut line: Vec<u8> = Vec::new();
        let name: Option<&[u8]> = match army {
            0 => Some(b"Zero"),
            1 => Some(b"Balanced"),
            2 => Some(b"Inferior"),
            3 => Some(b"Ranged attack"),
            4 => Some(b"Melee attack"),
            5 => Some(b"Giant"),
            6 => Some(b"Full"),
            _ => None,
        };
        self.g.m.set_u8(LINE, 0);
        if let Some(name) = name {
            line.extend_from_slice(name);
            if (1..=5).contains(&army) {
                match size {
                    0 => line.extend_from_slice(b"(S)"),
                    1 => line.extend_from_slice(b"(M)"),
                    2 => line.extend_from_slice(b"(L)"),
                    _ => {}
                }
            }
            self.write_cstr(LINE, &line);
        }
        if defense != 100 {
            if army != -1 {
                line.extend_from_slice(b"    ");
                self.write_cstr(LINE, &line);
            }
            let text = format!("Defense: {}.{}", defense / 100, (defense % 100) / 10);
            self.write_cstr(DEFENSE_TEXT, text.as_bytes());
            line.extend_from_slice(text.as_bytes());
            self.write_cstr(LINE, &line);
        }
    }

    fn write_cstr(&mut self, at: u32, s: &[u8]) {
        for (k, &b) in s.iter().chain(std::iter::once(&0)).enumerate() {
            self.g.m.set_u8(at + k as u32, b);
        }
    }
}
