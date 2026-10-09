//! The score board at the end of a match (0x422218 in `match_frame` 0x41bc90): while
//! end_timer is 101..349 the frame draws, over the match, one row per player slot
//! with the fighter's small picture, its name ("P1".."P8", or "Com"), five counters
//! of the object and WIN_ALIVE/WIN_DEAD/LOSE_DEAD, then the footer with the match
//! time (battle mode adds the army counts). It only draws.

use crate::world::World;

pub const SCORE_BOARD1: u32 = 0x44fcb4;
pub const SCORE_BOARD2: u32 = 0x44fd8c;
pub const SCORE_BOARD3: u32 = 0x44f88c;
pub const SCORE_BOARD4: u32 = 0x44f87c;
pub const WIN_ALIVE: u32 = 0x44fd90;
pub const WIN_DEAD: u32 = 0x44fd94;
pub const LOSE_DEAD: u32 = 0x44fb64;

/// Surfaces of the score board: global, resource.
pub const SHEETS: &[(u32, &str)] = &[
    (SCORE_BOARD1, "SCORE_BOARD1"),
    (SCORE_BOARD2, "SCORE_BOARD2"),
    (SCORE_BOARD3, "SCORE_BOARD3"),
    (SCORE_BOARD4, "SCORE_BOARD4"),
    (WIN_ALIVE, "WIN_ALIVE"),
    (WIN_DEAD, "WIN_DEAD"),
    (LOSE_DEAD, "LOSE_DEAD"),
];

/// The WORDS font of a team (WORDS0 for independent fighters).
fn team_font(team: i32) -> u32 {
    match team {
        1 => 0x44f888,
        2 => 0x44fcbc,
        3 => 0x44fb68,
        4 => 0x44faf8,
        _ => 0x44faf4,
    }
}

impl World {
    /// Stage mode: no fighter of a player team (1..39 except 5) is alive, so the
    /// players lost (`iStack_5e0` of match_frame).
    fn stage_lost(&self) -> bool {
        !(0..self.objs.len()).any(|i| {
            let o = &self.objs[i];
            self.exists(i) && 0 < o.hp() && self.data_of(i).kind == 0 && (1..0x28).contains(&o.team()) && o.team() != 5
        })
    }

    pub(crate) fn draw_summary(&mut self) {
        let mode = self.g.game_mode();
        let shown = |w: &World, k: usize| if w.exists(k) { Some(k) } else if w.exists(k + 10) { Some(k + 10) } else { None };
        let rows = (0..8).filter(|&k| shown(self, k).is_some()).count() as i32;
        let height = rows * 0x2d + if mode == 4 { 0x8a } else { 0x5d };
        let y0 = (0x212 - height) / 2;
        self.blit(SCORE_BOARD1, 0x96, y0, -1, false);
        let lost = mode == 1 && self.stage_lost();
        let winner = self.g.m.i32(0x450bf8);
        let mut y = y0 + 0x10;
        for k in 0..8 {
            let Some(i) = shown(self, k) else { continue };
            y += 0x2d;
            self.blit(SCORE_BOARD2, 0x96, y, -1, false);
            let data = self.objs[i].m.i32(0x368).max(0) as usize;
            self.blit_sheet(crate::draw::Sheet::Small(data), 0xa5, y, -1, false);
            let name: [u8; 3] = if i < 10 { [b'P', b'1' + i as u8, b' '] } else { *b"Com" };
            let font = team_font(self.objs[i].team());
            let ty = y + 0xf;
            for (c, &ch) in name.iter().enumerate() {
                self.blit(font, 0xce + 9 * c as i32, ty, ch as i32, true);
            }
            for (field, color, x) in [(0x358, 0xaaaaff, 0x10f), (0x348, 0xaaaaff, 0x146), (0x34c, 0xaaf0f0, 0x186), (0x350, 0xaaf0f0, 0x1c6), (0x35c, 0xaaf5aa, 0x20f)] {
                let v = self.objs[i].m.i32(field);
                self.text(v.to_string().as_bytes(), 0, color, x, ty);
            }
            let (team, hp) = (self.objs[i].team(), self.objs[i].hp());
            let result = if mode == 1 {
                Some(if lost { LOSE_DEAD } else if 0 < hp { WIN_ALIVE } else { WIN_DEAD })
            } else if winner < 0 {
                None
            } else if winner == team {
                Some(if 0 < hp { WIN_ALIVE } else { WIN_DEAD })
            } else {
                Some(LOSE_DEAD)
            };
            if let Some(r) = result {
                self.blit(r, 0x23b, ty, -1, false);
            }
        }
        let foot = y0 + 0x3d + rows * 0x2d;
        if mode == 4 {
            self.blit(SCORE_BOARD4, 0x96, foot, -1, false);
            self.blit(SCORE_BOARD3, 0x96, foot + 0x2d, -1, false);
            let ty = foot + 0x17;
            for (at, color, x) in [(0x451b64u32, 0xffafaf, 0xf3), (0x451b68, 0xafafff, 0x1e3), (0x451b6c, 0xffafaf, 0x143), (0x451b70, 0xafafff, 0x233)] {
                let v = self.g.m.i32(at);
                self.text(v.to_string().as_bytes(), 0, color, x, ty);
            }
        } else {
            self.blit(SCORE_BOARD3, 0x96, foot, -1, false);
        }
        let t = (self.g.m.i32(0x450bbc) + 0xf) / 0x1e;
        let time = if t < 0xe10 {
            format!("{:02} : {:02}", t / 0x3c, t % 0x3c)
        } else {
            format!("{:02} : {:02} : {:02}", t / 0xe10, (t % 0xe10) / 0x3c, (t % 0xe10) % 0x3c)
        };
        let ty = y0 + if mode == 4 { 0x70 } else { 0x43 } + rows * 0x2d;
        self.text(time.as_bytes(), 0, 0xffffff, 0x244, ty);
    }
}
