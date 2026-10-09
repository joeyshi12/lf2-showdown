//! Drawing requests. Code that both updates state and draws (the menus, some texts
//! in a match) records what it draws here; the app's renderer executes the list.
//! The original draws straight onto the back buffer with `blit_sprite` (0x43f010),
//! `fill_rect` (0x415160) and `draw_text` (0x401290).

/// A surface the original keeps a pointer to.
#[derive(Clone, Debug, PartialEq)]
pub enum Sheet {
    /// The surface whose pointer is stored at this global (see `menu::sprites`).
    Global(u32),
    /// The menu background `MENU_BACKn` (n = 1..13).
    MenuBack(i32),
    /// The `head:` picture of an object data file (file-manager index).
    Face(usize),
    /// The `small:` picture of an object data file.
    Small(usize),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Draw {
    /// `fill_rect(x, y, w, h, rgb)`; rgb is 0x00RRGGBB.
    Fill { x: i32, y: i32, w: i32, h: i32, rgb: u32 },
    /// `blit_sprite(sheet, x, y, clip, key, mirror)`: clip -1 is the whole surface;
    /// `key` skips black pixels.
    Blit { sheet: Sheet, x: i32, y: i32, clip: i32, key: bool, mirror: bool },
    /// `draw_text`: TextOutA with the DC's default font (System), opaque background.
    /// Colours are COLORREF values (0x00BBGGRR).
    Text { text: Vec<u8>, bk: u32, color: u32, x: i32, y: i32 },
}

impl crate::World {
    pub fn fill(&mut self, x: i32, y: i32, w: i32, h: i32, rgb: u32) {
        self.draw.push(Draw::Fill { x, y, w, h, rgb });
    }

    /// `blit_sprite` on a surface held by a global.
    pub fn blit(&mut self, global: u32, x: i32, y: i32, clip: i32, key: bool) {
        self.draw.push(Draw::Blit { sheet: Sheet::Global(global), x, y, clip, key, mirror: false });
    }

    pub fn blit_sheet(&mut self, sheet: Sheet, x: i32, y: i32, clip: i32, key: bool) {
        self.draw.push(Draw::Blit { sheet, x, y, clip, key, mirror: false });
    }

    pub fn text(&mut self, text: &[u8], bk: u32, color: u32, x: i32, y: i32) {
        self.draw.push(Draw::Text { text: text.to_vec(), bk, color, x, y });
    }
}

impl crate::World {
    /// `draw_number` (0x423940): text in the 8x16 WORDS fonts (font 0..2), at most
    /// `cols` characters per line and `rows` lines; it writes a terminator into the
    /// string where it stopped (the string is in the globals at `at`), and `cursor`
    /// adds an underscore.
    pub fn draw_number(&mut self, at: u32, x: i32, y: i32, cols: i32, rows: i32, font: i32, cursor: bool) {
        let len = (0..0x400).take_while(|&k| self.g.m.u8(at + k) != 0).count() as i32;
        let sheet = match font {
            0 => Some(0x44faf4),
            1 => Some(0x44f888),
            2 => Some(0x44fcbc),
            _ => None,
        };
        let (mut cx, mut cy, mut row, mut col, mut k) = (x, y, 0, 0, 0);
        while k < len {
            if rows <= row {
                break;
            }
            let c = self.g.m.u8(at + k as u32);
            if c == b'\n' {
                if rows - 1 <= row {
                    break;
                }
                cy += 0x10;
                row += 1;
                col = 0;
                cx = x;
            } else {
                col += 1;
                if let Some(s) = sheet {
                    self.blit(s, cx, cy, c as i8 as i32, true);
                }
                cx += 8;
                if cols <= col {
                    if row < rows - 1 {
                        cy += 0x10;
                        row += 1;
                        col = 0;
                        cx = x;
                    } else {
                        k += 1;
                        break;
                    }
                }
            }
            k += 1;
        }
        if k < 0x400 {
            self.g.m.set_u8(at + k as u32, 0);
        }
        if cursor {
            self.blit(0x44faf4, cx, cy, 0x5f, true);
        }
    }

    /// `draw_number_wrapper` (0x423a70): the text with a shadow (drawn 4 times).
    pub fn draw_number_shadow(&mut self, at: u32, x: i32, y: i32, cols: i32, rows: i32, font: i32, cursor: bool) {
        self.draw_number(at, x - 1, y + 1, cols, rows, font, cursor);
        self.draw_number(at, x - 1, y, cols, rows, font, cursor);
        self.draw_number(at, x, y + 1, cols, rows, font, cursor);
        self.draw_number(at, x, y, cols, rows, font, cursor);
    }

    /// `draw_mode_label` (0x41b130): "VS mode (Difficult)" and the like at 0x450c38.
    pub fn draw_mode_label(&mut self) {
        const AT: u32 = 0x450c38;
        let put = |w: &mut crate::World, from: usize, s: &[u8]| {
            for (k, &b) in s.iter().chain(std::iter::once(&0)).enumerate() {
                w.g.m.set_u8(AT + (from + k) as u32, b);
            }
        };
        let mode = self.g.game_mode();
        match mode {
            0 => put(self, 0, b"VS mode "),
            1 => put(self, 0, if self.g.stage() / 10 == 5 { b"Survival Stage " } else { b"Stage mode " }),
            2 => put(self, 0, b"1 on 1 "),
            3 => put(self, 0, b"2 on 2 "),
            4 => put(self, 0, b"Battle mode "),
            _ => {}
        }
        let d: Option<&[u8]> = match self.g.difficulty() {
            0 => Some(b"(Difficult)"),
            1 => Some(b"(Normal)"),
            2 => Some(b"(Easy)"),
            -1 => Some(b"(CRAZY!)"),
            _ => None,
        };
        if let Some(d) = d {
            let end = (0..0x400).take_while(|&k| self.g.m.u8(AT + k) != 0).count();
            put(self, end, d);
        }
        let len = (0..0x400).take_while(|&k| self.g.m.u8(AT + k) != 0).count() as i32;
        let y = if self.g.replaying() != 0 { 0x1fe } else { 0x213 };
        self.draw_number_shadow(AT, len * -8 + 0x316, y, 0x40, 4, 0, false);
    }
}
