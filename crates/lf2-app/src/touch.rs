//! On-screen controls for touch screens: a direction pad at the bottom left, the
//! attack, jump and defend buttons at the bottom right, and buttons for F4 (back to
//! the menu), F1 (pause) and the on-screen keyboard at the top. They press player 1's
//! keys from control.txt. A touch that starts away from the controls is the mouse,
//! for the menus.
//!
//! Positions and sizes are in screen pixels and follow the screen height, so the
//! controls are the same physical size on any screen of the same height. When the
//! black bars beside the picture are wide enough the controls sit in them; otherwise
//! they are drawn over the picture, half transparent.
//!
//! A controller can press player 1's keys through the same path (`set_controller`).
//! The controls are hidden while a controller is in use (`set_visible`) and come back
//! when the screen is touched.
//!
//! `changes` is called once a tick. A key stays pressed for at least `MIN_TICKS`
//! calls however short the touch was, because the game copies the keys into the
//! players' objects only every other tick (0x419a60) and would miss a shorter tap.

use lf2_sim::input::Joystick;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Start,
    Move,
    End,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    Key(u8),
    Keyboard,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Shape {
    Circle,
    /// A rectangle with rounded corners; `r` is half the height and `half_w` half
    /// the width.
    Bar { half_w: i32 },
}

#[derive(Clone, Debug)]
struct Button {
    action: Action,
    label: &'static str,
    x: i32,
    y: i32,
    r: i32,
    shape: Shape,
}

impl Button {
    /// How far the point is outside the button, in pixels (0 or less is inside).
    fn distance(&self, x: f64, y: f64) -> f64 {
        let (dx, dy) = (x - self.x as f64, y - self.y as f64);
        match self.shape {
            Shape::Circle => (dx * dx + dy * dy).sqrt() - self.r as f64,
            Shape::Bar { half_w } => {
                let ox = (dx.abs() - half_w as f64).max(0.0);
                let oy = (dy.abs() - self.r as f64).max(0.0);
                (ox * ox + oy * oy).sqrt()
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Role {
    /// The touch started on the direction pad; it keeps steering it wherever it moves.
    Pad { x: f64, y: f64 },
    /// The touch started on a button; it presses whichever button it is over now.
    Button(Option<usize>),
    /// The touch started away from the controls.
    Mouse,
}

/// What a touch did besides pressing keys.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TouchResult {
    /// The touch that is the mouse: its position in screen pixels and whether it is
    /// down.
    pub mouse: Option<(f64, f64, bool)>,
    /// The keyboard button was tapped.
    pub toggle_keyboard: bool,
}

pub struct TouchPad {
    /// Player 1's up, down, left, right, attack, jump and defend keys.
    keys: [u8; 7],
    size: (u32, u32),
    pad: (i32, i32, i32),
    buttons: Vec<Button>,
    touches: Vec<(u64, Role)>,
    /// The keys a touch pressed since the last call of `changes`.
    pressed: [bool; 256],
    /// The keys as last reported by `changes`.
    reported: [bool; 256],
    /// For each key reported pressed, the number of calls of `changes` it has been
    /// pressed for.
    ticks: [u8; 256],
    /// What the controller pressing player 1's keys is pressing now.
    controller: Joystick,
    visible: bool,
}


/// The fewest ticks a touch keeps a key pressed.
pub const MIN_TICKS: u8 = 2;

const UP: usize = 0;
const DOWN: usize = 1;
const LEFT: usize = 2;
const RIGHT: usize = 3;
const ATTACK: usize = 4;
const JUMP: usize = 5;
const DEFEND: usize = 6;

/// The virtual-key codes of F1 and F4.
const VK_F1: u8 = 0x70;
const VK_F4: u8 = 0x73;

impl TouchPad {
    /// `keys`: player 1's up, down, left, right, attack, jump and defend keys.
    pub fn new(keys: [u8; 7]) -> TouchPad {
        TouchPad { keys, size: (0, 0), pad: (0, 0, 0), buttons: Vec::new(), touches: Vec::new(), pressed: [false; 256], reported: [false; 256], ticks: [0; 256], controller: Joystick::default(), visible: true }
    }

    /// Player 1's keys from control.txt (the second to eighth numbers of the
    /// player's line).
    pub fn keys_of(controls: &lf2_data::controls::Controls) -> [u8; 7] {
        std::array::from_fn(|k| controls.players[0][k + 1] as u8)
    }

    /// Change the keys the controls press. Touches that are down keep the keys they
    /// pressed until they are lifted.
    pub fn set_keys(&mut self, keys: [u8; 7]) {
        self.keys = keys;
    }

    /// Place the controls for a screen of `w` x `h` pixels.
    pub fn resize(&mut self, w: u32, h: u32) {
        if self.size == (w, h) {
            return;
        }
        self.size = (w, h);
        let (w, h) = (w as i32, h as i32);
        let unit = w.min(h);
        let margin = unit / 25;
        // the width of each black bar beside the picture
        let (_, bar, _, _, _) = crate::window::layout(w.max(1) as u32, h.max(1) as u32);
        let bar = bar as i32;

        // centred in the bar, or a little way from the screen's edge when the bar is
        // narrower
        let side = unit / 50;
        let from_edge = |half_w: i32| (bar / 2).max(side + half_w);

        let pad_r = unit * 9 / 50;
        self.pad = (from_edge(pad_r), h - margin - pad_r, pad_r);

        // attack at the left, jump at the right, defend below between them
        let b = unit * 17 / 200;
        let half_w = b * 215 / 100;
        let cx = w - from_edge(half_w);
        let cy = h - margin - b * 225 / 100;
        let circle = |action, label, x, y| Button { action, label, x, y, r: b, shape: Shape::Circle };
        self.buttons = vec![
            circle(Action::Key(ATTACK as u8), "A", cx - b * 115 / 100, cy - b * 60 / 100),
            circle(Action::Key(JUMP as u8), "J", cx + b * 115 / 100, cy - b * 60 / 100),
            circle(Action::Key(DEFEND as u8), "D", cx, cy + b * 125 / 100),
        ];

        // F4 at the top left; F1 and the keyboard at the top right
        let s = text_scale(h as u32) as i32;
        let r = 6 * s;
        let bar_w = |label: &str| text_width(label, s as u32) as i32 / 2 + 3 * s;
        let top = margin + r;
        let left = |label: &'static str, action, y| {
            let half_w = bar_w(label);
            Button { action, label, x: from_edge(half_w), y, r, shape: Shape::Bar { half_w } }
        };
        let right = |label: &'static str, action, y| {
            let half_w = bar_w(label);
            Button { action, label, x: w - from_edge(half_w), y, r, shape: Shape::Bar { half_w } }
        };
        self.buttons.push(left("F4 MENU", Action::Key(VK_F4), top));
        self.buttons.push(right("F1 PAUSE", Action::Key(VK_F1), top));
        self.buttons.push(right("KEYBOARD", Action::Keyboard, top + 2 * r + margin / 2));
    }

    /// Show or hide the controls. Hidden, they are not drawn and a touch shows them
    /// again (and does what it does when they are shown).
    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    pub fn visible(&self) -> bool {
        self.visible
    }

    /// A controller is pressing `j`: the directions are player 1's up, down, left and
    /// right, the bottom face button attack, the right one jump, and the left and
    /// top ones defend (the buttons control.txt gives a joystick by default are 0,
    /// 1 and 3).
    pub fn set_controller(&mut self, j: Joystick) {
        self.controller = j;
        let touched = self.touched_keys();
        for (p, t) in self.pressed.iter_mut().zip(touched) {
            *p |= t;
        }
    }

    /// The button a touch at (x, y) is on: the nearest one within a little more than
    /// its size.
    fn button_at(&self, x: f64, y: f64) -> Option<usize> {
        let near = |b: &Button| b.distance(x, y) <= b.r as f64 * 0.2;
        self.buttons
            .iter()
            .enumerate()
            .filter(|(_, b)| near(b))
            .min_by(|(_, a), (_, b)| a.distance(x, y).total_cmp(&b.distance(x, y)))
            .map(|(i, _)| i)
    }

    fn on_pad(&self, x: f64, y: f64) -> bool {
        let (cx, cy, r) = self.pad;
        let (dx, dy) = (x - cx as f64, y - cy as f64);
        (dx * dx + dy * dy).sqrt() <= r as f64 * 1.25
    }

    /// The directions (UP, DOWN, LEFT, RIGHT) the pad gives for a touch at (x, y):
    /// one, or two on the diagonals, or none near the middle.
    fn directions(&self, x: f64, y: f64) -> [bool; 4] {
        let (cx, cy, r) = self.pad;
        let (dx, dy) = (x - cx as f64, y - cy as f64);
        let mut d = [false; 4];
        if (dx * dx + dy * dy).sqrt() < r as f64 * 0.25 {
            return d;
        }
        // eight sectors of 45 degrees; each direction covers three of them
        let a = (-dy).atan2(dx).to_degrees();
        d[RIGHT] = a.abs() < 67.5;
        d[LEFT] = a.abs() > 112.5;
        d[UP] = a > 22.5 && a < 157.5;
        d[DOWN] = a < -22.5 && a > -157.5;
        d
    }

    /// A touch went down, moved or was lifted at (x, y) in screen pixels.
    pub fn touch(&mut self, id: u64, phase: Phase, x: f64, y: f64) -> TouchResult {
        self.visible = true;
        let mut out = TouchResult::default();
        let at = self.touches.iter().position(|(t, _)| *t == id);
        let role = match (phase, at) {
            (Phase::Start, _) => {
                if let Some(i) = at {
                    self.touches.remove(i);
                }
                let role = if self.on_pad(x, y) {
                    Role::Pad { x, y }
                } else if let Some(b) = self.button_at(x, y) {
                    if self.buttons[b].action == Action::Keyboard {
                        out.toggle_keyboard = true;
                    }
                    Role::Button(Some(b))
                } else if self.touches.iter().any(|(_, r)| *r == Role::Mouse) {
                    // a second finger away from the controls does nothing
                    Role::Button(None)
                } else {
                    Role::Mouse
                };
                self.touches.push((id, role));
                Some(role)
            }
            (Phase::Move, Some(i)) => {
                let role = match self.touches[i].1 {
                    Role::Pad { .. } => Role::Pad { x, y },
                    // sliding from one button onto another presses the other
                    Role::Button(_) => Role::Button(self.button_at(x, y).filter(|&b| self.buttons[b].action != Action::Keyboard)),
                    Role::Mouse => Role::Mouse,
                };
                self.touches[i].1 = role;
                Some(role)
            }
            (Phase::End, Some(i)) => Some(self.touches.remove(i).1),
            (_, None) => None,
        };
        if role == Some(Role::Mouse) {
            out.mouse = Some((x, y, phase != Phase::End));
        }
        let touched = self.touched_keys();
        for (p, t) in self.pressed.iter_mut().zip(touched) {
            *p |= t;
        }
        out
    }

    /// Press the key `vk` for `MIN_TICKS` ticks.
    pub fn tap(&mut self, vk: u8) {
        self.pressed[vk as usize] = true;
    }

    /// The keys the touches and the controller are on now.
    fn touched_keys(&self) -> [bool; 256] {
        let mut held = [false; 256];
        let c = &self.controller;
        let b = c.buttons;
        for (k, on) in [c.up, c.down, c.left, c.right, b[0], b[1], b[2] || b[3]].into_iter().enumerate() {
            if on {
                held[self.keys[k] as usize] = true;
            }
        }
        for (_, role) in &self.touches {
            match *role {
                Role::Pad { x, y } => {
                    for (i, &d) in self.directions(x, y).iter().enumerate() {
                        if d {
                            held[self.keys[i] as usize] = true;
                        }
                    }
                }
                Role::Button(Some(b)) => match self.buttons[b].action {
                    Action::Key(k) if (k as usize) < self.keys.len() => held[self.keys[k as usize] as usize] = true,
                    Action::Key(vk) => held[vk as usize] = true,
                    Action::Keyboard => {}
                },
                Role::Button(None) | Role::Mouse => {}
            }
        }
        held
    }

    /// The key presses and releases since the last call, as (virtual-key code,
    /// down). Call it once a tick.
    pub fn changes(&mut self, out: &mut Vec<(u8, bool)>) {
        let mut held = self.touched_keys();
        let pressed = std::mem::replace(&mut self.pressed, [false; 256]);
        for k in 0..256 {
            held[k] |= pressed[k] || (self.reported[k] && self.ticks[k] < MIN_TICKS);
            self.ticks[k] = if held[k] { self.ticks[k].saturating_add(1) } else { 0 };
        }
        for (k, (&now, &before)) in held.iter().zip(&self.reported).enumerate() {
            if now != before {
                out.push((k as u8, now));
            }
        }
        self.reported = held;
    }

    /// Release every key (when the window loses the focus).
    pub fn release_all(&mut self, out: &mut Vec<(u8, bool)>) {
        self.touches.clear();
        self.controller = Joystick::default();
        self.pressed = [false; 256];
        self.ticks = [0; 256];
        for k in 0..256 {
            if std::mem::take(&mut self.reported[k]) {
                out.push((k as u8, false));
            }
        }
    }

    /// Draw the controls over the screen's pixels (`buf` is `w` x `h`, 0x00RRGGBB).
    pub fn draw(&self, buf: &mut [u32], w: u32, h: u32) {
        if !self.visible || self.size != (w, h) || buf.len() < (w * h) as usize {
            return;
        }
        let mut c = Canvas { buf, w: w as i32, h: h as i32 };
        let held = self.reported;
        let (cx, cy, r) = self.pad;
        c.disc(cx, cy, r, 0xffffff, 40);
        c.ring(cx, cy, r, (r / 40).max(2), 0xffffff, 110);
        // the arrows, lit while their key is held
        let arrow = r * 26 / 100;
        let reach = r * 62 / 100;
        for (dir, (ux, uy)) in [(UP, (0, -1)), (DOWN, (0, 1)), (LEFT, (-1, 0)), (RIGHT, (1, 0))] {
            let on = held[self.keys[dir] as usize];
            let (ax, ay) = (cx + ux * reach, cy + uy * reach);
            c.triangle(ax, ay, (ux, uy), arrow, if on { 230 } else { 120 });
        }
        if let Some((x, y)) = self.touches.iter().find_map(|(_, r)| match r {
            Role::Pad { x, y } => Some((*x as i32, *y as i32)),
            _ => None,
        }) {
            c.disc(x, y, r * 22 / 100, 0xffffff, 90);
        }
        let s = text_scale(h);
        for b in &self.buttons {
            let on = match b.action {
                Action::Key(k) if (k as usize) < self.keys.len() => held[self.keys[k as usize] as usize],
                Action::Key(vk) => held[vk as usize],
                Action::Keyboard => false,
            };
            let alpha = if on { 150 } else { 55 };
            match b.shape {
                Shape::Circle => {
                    c.disc(b.x, b.y, b.r, 0xffffff, alpha);
                    c.ring(b.x, b.y, b.r, (b.r / 20).max(2), 0xffffff, 140);
                    let big = (b.r as u32 / 9).max(1);
                    c.text(b.x, b.y, b.label, big, 0xffffff, 220);
                }
                Shape::Bar { half_w } => {
                    c.rounded(b.x, b.y, half_w, b.r, 0xffffff, alpha);
                    c.text(b.x, b.y, b.label, s, 0xffffff, 220);
                }
            }
        }
    }
}

/// The size of a pixel of the small labels' font, in screen pixels.
fn text_scale(h: u32) -> u32 {
    (h / 270).max(1)
}

fn text_width(text: &str, s: u32) -> u32 {
    (text.len() as u32 * 6).saturating_sub(1) * s
}

struct Canvas<'a> {
    buf: &'a mut [u32],
    w: i32,
    h: i32,
}

impl Canvas<'_> {
    fn blend(&mut self, x: i32, y: i32, rgb: u32, alpha: u32) {
        if x < 0 || y < 0 || x >= self.w || y >= self.h {
            return;
        }
        let p = &mut self.buf[(y * self.w + x) as usize];
        let mix = |shift: u32| {
            let (a, b) = ((*p >> shift) & 0xff, (rgb >> shift) & 0xff);
            ((a * (255 - alpha) + b * alpha) / 255) << shift
        };
        *p = mix(16) | mix(8) | mix(0);
    }

    fn span(&mut self, x0: i32, x1: i32, y: i32, rgb: u32, alpha: u32) {
        for x in x0.max(0)..x1.min(self.w) {
            self.blend(x, y, rgb, alpha);
        }
    }

    fn disc(&mut self, cx: i32, cy: i32, r: i32, rgb: u32, alpha: u32) {
        for dy in -r..=r {
            let dx = ((r * r - dy * dy) as f64).sqrt() as i32;
            self.span(cx - dx, cx + dx + 1, cy + dy, rgb, alpha);
        }
    }

    fn ring(&mut self, cx: i32, cy: i32, r: i32, width: i32, rgb: u32, alpha: u32) {
        let inner = r - width;
        for dy in -r..=r {
            let outer = ((r * r - dy * dy) as f64).sqrt() as i32;
            if dy.abs() >= inner {
                self.span(cx - outer, cx + outer + 1, cy + dy, rgb, alpha);
            } else {
                let i = ((inner * inner - dy * dy) as f64).sqrt() as i32;
                self.span(cx - outer, cx - i, cy + dy, rgb, alpha);
                self.span(cx + i + 1, cx + outer + 1, cy + dy, rgb, alpha);
            }
        }
    }

    /// A rectangle of half width `hw` and half height `hh` with round ends.
    fn rounded(&mut self, cx: i32, cy: i32, hw: i32, hh: i32, rgb: u32, alpha: u32) {
        let r = hh;
        let straight = (hw - r).max(0);
        for dy in -hh..=hh {
            let dx = ((r * r - dy * dy) as f64).sqrt() as i32;
            self.span(cx - straight - dx, cx + straight + dx + 1, cy + dy, rgb, alpha);
        }
    }

    /// A white triangle pointing along (ux, uy) (one of the four directions) with its
    /// tip `size` beyond (x, y) and its base `size` behind it.
    fn triangle(&mut self, x: i32, y: i32, (ux, uy): (i32, i32), size: i32, alpha: u32) {
        for t in 0..=2 * size {
            // t runs from the base to the tip; the half width shrinks to 0 at the tip
            let half = (2 * size - t) / 2;
            let along = t - size;
            for s in -half..=half {
                let (px, py) = (x + ux * along + uy * s, y + uy * along + ux * s);
                self.blend(px, py, 0xffffff, alpha);
            }
        }
    }

    /// Text in the 5x7 font, each font pixel `s` x `s`, centred on (cx, cy).
    fn text(&mut self, cx: i32, cy: i32, text: &str, s: u32, rgb: u32, alpha: u32) {
        let s = s as i32;
        let x0 = cx - text_width(text, s as u32) as i32 / 2;
        let y0 = cy - 7 * s / 2;
        for (i, ch) in text.chars().enumerate() {
            let Some(rows) = glyph(ch) else { continue };
            for (row, bits) in rows.iter().enumerate() {
                for col in 0..5 {
                    if bits & (0x10 >> col) == 0 {
                        continue;
                    }
                    let (px, py) = (x0 + (i as i32 * 6 + col) * s, y0 + row as i32 * s);
                    for yy in py..py + s {
                        self.span(px, px + s, yy, rgb, alpha);
                    }
                }
            }
        }
    }
}

/// The rows (top first, 5 bits each, the highest bit at the left) of the letters the
/// labels use.
fn glyph(c: char) -> Option<[u8; 7]> {
    Some(match c {
        'A' => [0x0e, 0x11, 0x11, 0x1f, 0x11, 0x11, 0x11],
        'B' => [0x1e, 0x11, 0x11, 0x1e, 0x11, 0x11, 0x1e],
        'D' => [0x1e, 0x11, 0x11, 0x11, 0x11, 0x11, 0x1e],
        'E' => [0x1f, 0x10, 0x10, 0x1e, 0x10, 0x10, 0x1f],
        'F' => [0x1f, 0x10, 0x10, 0x1e, 0x10, 0x10, 0x10],
        'J' => [0x07, 0x02, 0x02, 0x02, 0x02, 0x12, 0x0c],
        'K' => [0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11],
        'M' => [0x11, 0x1b, 0x15, 0x15, 0x11, 0x11, 0x11],
        'N' => [0x11, 0x11, 0x19, 0x15, 0x13, 0x11, 0x11],
        'O' => [0x0e, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0e],
        'P' => [0x1e, 0x11, 0x11, 0x1e, 0x10, 0x10, 0x10],
        'R' => [0x1e, 0x11, 0x11, 0x1e, 0x14, 0x12, 0x11],
        'S' => [0x0f, 0x10, 0x10, 0x0e, 0x01, 0x01, 0x1e],
        'U' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0e],
        'Y' => [0x11, 0x11, 0x0a, 0x04, 0x04, 0x04, 0x04],
        '1' => [0x04, 0x0c, 0x04, 0x04, 0x04, 0x04, 0x0e],
        '4' => [0x02, 0x06, 0x0a, 0x12, 0x1f, 0x02, 0x02],
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEYS: [u8; 7] = [87, 83, 65, 68, 74, 75, 76];

    fn pad(w: u32, h: u32) -> TouchPad {
        let mut p = TouchPad::new(KEYS);
        p.resize(w, h);
        p
    }

    fn changes(p: &mut TouchPad) -> Vec<(u8, bool)> {
        let mut v = Vec::new();
        p.changes(&mut v);
        v
    }

    /// Run the ticks after a press during which a released key stays pressed.
    fn settle(p: &mut TouchPad) {
        for _ in 1..MIN_TICKS {
            assert_eq!(changes(p), vec![]);
        }
    }

    fn centre(p: &TouchPad, label: &str) -> (f64, f64) {
        let b = p.buttons.iter().find(|b| b.label == label).unwrap();
        (b.x as f64, b.y as f64)
    }

    #[test]
    fn pad_presses_one_direction_or_two_on_a_diagonal() {
        let mut p = pad(2400, 1080);
        let (cx, cy, r) = p.pad;
        let (cx, cy, r) = (cx as f64, cy as f64, r as f64);
        p.touch(1, Phase::Start, cx + r * 0.7, cy);
        assert_eq!(changes(&mut p), vec![(68, true)]);
        settle(&mut p);
        // to the upper right: up as well
        p.touch(1, Phase::Move, cx + r * 0.5, cy - r * 0.5);
        assert_eq!(changes(&mut p), vec![(87, true)]);
        settle(&mut p);
        // to the upper left: left instead of right
        p.touch(1, Phase::Move, cx - r * 0.5, cy - r * 0.5);
        assert_eq!(changes(&mut p), vec![(65, true), (68, false)]);
        settle(&mut p);
        // the middle presses nothing
        p.touch(1, Phase::Move, cx, cy);
        assert_eq!(changes(&mut p), vec![(65, false), (87, false)]);
        // straight down, then past the pad's edge: the touch still steers the pad
        p.touch(1, Phase::Move, cx, cy + r * 3.0);
        assert_eq!(changes(&mut p), vec![(83, true)]);
        settle(&mut p);
        p.touch(1, Phase::End, cx, cy + r * 3.0);
        assert_eq!(changes(&mut p), vec![(83, false)]);
    }

    #[test]
    fn buttons_press_attack_jump_and_defend_together() {
        let mut p = pad(2400, 1080);
        for (id, label) in [(1, "A"), (2, "J"), (3, "D")] {
            let (x, y) = centre(&p, label);
            p.touch(id, Phase::Start, x, y);
        }
        assert_eq!(changes(&mut p), vec![(74, true), (75, true), (76, true)]);
    }

    #[test]
    fn sliding_from_one_button_to_another_moves_the_press() {
        let mut p = pad(2400, 1080);
        let (dx, dy) = centre(&p, "D");
        let (ax, ay) = centre(&p, "A");
        p.touch(1, Phase::Start, dx, dy);
        assert_eq!(changes(&mut p), vec![(76, true)]);
        settle(&mut p);
        p.touch(1, Phase::Move, ax, ay);
        assert_eq!(changes(&mut p), vec![(74, true), (76, false)]);
    }

    #[test]
    fn a_tap_between_two_ticks_is_pressed_for_min_ticks() {
        let mut p = pad(2400, 1080);
        let (x, y) = centre(&p, "A");
        p.touch(1, Phase::Start, x, y);
        p.touch(1, Phase::End, x, y);
        assert_eq!(changes(&mut p), vec![(74, true)]);
        for _ in 1..MIN_TICKS {
            assert_eq!(changes(&mut p), vec![]);
        }
        assert_eq!(changes(&mut p), vec![(74, false)]);
        assert_eq!(changes(&mut p), vec![]);
    }

    #[test]
    fn a_tap_of_a_key_is_pressed_for_min_ticks() {
        let mut p = pad(2400, 1080);
        p.tap(0x73);
        assert_eq!(changes(&mut p), vec![(0x73, true)]);
        settle(&mut p);
        assert_eq!(changes(&mut p), vec![(0x73, false)]);
    }

    #[test]
    fn a_touch_longer_than_min_ticks_is_released_when_lifted() {
        let mut p = pad(2400, 1080);
        let (x, y) = centre(&p, "J");
        p.touch(1, Phase::Start, x, y);
        for _ in 0..5 {
            changes(&mut p);
        }
        p.touch(1, Phase::End, x, y);
        assert_eq!(changes(&mut p), vec![(75, false)]);
    }

    #[test]
    fn top_buttons_press_f4_and_f1_and_ask_for_the_keyboard() {
        let mut p = pad(2400, 1080);
        let (x, y) = centre(&p, "F4 MENU");
        p.touch(1, Phase::Start, x, y);
        let (x, y) = centre(&p, "F1 PAUSE");
        p.touch(2, Phase::Start, x, y);
        assert_eq!(changes(&mut p), vec![(0x70, true), (0x73, true)]);
        let (x, y) = centre(&p, "KEYBOARD");
        let r = p.touch(3, Phase::Start, x, y);
        assert!(r.toggle_keyboard);
        assert_eq!(r.mouse, None);
    }

    #[test]
    fn a_touch_away_from_the_controls_is_the_mouse() {
        let mut p = pad(2400, 1080);
        assert_eq!(p.touch(1, Phase::Start, 1200.0, 500.0).mouse, Some((1200.0, 500.0, true)));
        assert_eq!(p.touch(1, Phase::Move, 1210.0, 505.0).mouse, Some((1210.0, 505.0, true)));
        // a second finger in the picture is not a second mouse
        assert_eq!(p.touch(2, Phase::Start, 900.0, 300.0).mouse, None);
        assert_eq!(p.touch(1, Phase::End, 1210.0, 505.0).mouse, Some((1210.0, 505.0, false)));
        assert_eq!(changes(&mut p), vec![]);
    }

    #[test]
    fn controls_sit_in_the_bars_of_a_wide_screen_and_inside_a_narrow_one() {
        for (w, h) in [(2400u32, 1080u32), (1280, 800), (1024, 768), (800, 1280)] {
            let p = pad(w, h);
            let (cx, cy, r) = p.pad;
            assert!(cx - r >= 0 && cy + r <= h as i32, "pad off a {}x{} screen", w, h);
            for b in &p.buttons {
                let half_w = match b.shape {
                    Shape::Circle => b.r,
                    Shape::Bar { half_w } => half_w,
                };
                assert!(b.x - half_w >= 0 && b.x + half_w <= w as i32, "{} off a {}x{} screen", b.label, w, h);
                assert!(b.y - b.r >= 0 && b.y + b.r <= h as i32, "{} off a {}x{} screen", b.label, w, h);
                assert!(p.button_at(b.x as f64, b.y as f64).is_some_and(|i| p.buttons[i].label == b.label));
                assert!(!p.on_pad(b.x as f64, b.y as f64), "{} is on the pad on a {}x{} screen", b.label, w, h);
            }
        }
        // on a 20:9 phone the pad and the buttons are in the bars, reaching at most a
        // tenth of their size into the picture
        let p = pad(2400, 1080);
        let (_, bar, _, _, _) = crate::window::layout(2400, 1080);
        let bar = bar as i32;
        assert!(p.pad.0 + p.pad.2 <= bar + p.pad.2 / 10);
        for b in p.buttons.iter().filter(|b| b.shape == Shape::Circle) {
            assert!(b.x - b.r >= 2400 - bar - b.r / 10, "{} is over the picture", b.label);
        }
    }

    #[test]
    fn a_controller_presses_player_1s_keys() {
        let mut p = pad(2400, 1080);
        p.set_controller(Joystick { right: true, buttons: [true, false, false, false], ..Default::default() });
        assert_eq!(changes(&mut p), vec![(68, true), (74, true)]);
        settle(&mut p);
        // the left and the top face buttons are both defend
        p.set_controller(Joystick { buttons: [false, false, true, false], ..Default::default() });
        assert_eq!(changes(&mut p), vec![(68, false), (74, false), (76, true)]);
        settle(&mut p);
        p.set_controller(Joystick::default());
        assert_eq!(changes(&mut p), vec![(76, false)]);
        // a press between two ticks still reaches the game
        p.set_controller(Joystick { buttons: [false, true, false, false], ..Default::default() });
        p.set_controller(Joystick::default());
        assert_eq!(changes(&mut p), vec![(75, true)]);
    }

    #[test]
    fn hidden_controls_are_not_drawn_and_a_touch_shows_them() {
        let (w, h) = (1200u32, 540u32);
        let mut p = pad(w, h);
        p.set_visible(false);
        let mut buf = vec![0u32; (w * h) as usize];
        p.draw(&mut buf, w, h);
        assert!(buf.iter().all(|&px| px == 0));
        // the touch on attack shows the controls and presses attack
        let (x, y) = centre(&p, "A");
        p.touch(1, Phase::Start, x, y);
        assert!(p.visible());
        assert_eq!(changes(&mut p), vec![(74, true)]);
        p.draw(&mut buf, w, h);
        assert!(buf.iter().any(|&px| px != 0));
    }

    #[test]
    fn every_label_letter_has_a_glyph() {
        let p = pad(2400, 1080);
        for b in &p.buttons {
            for c in b.label.chars().filter(|c| *c != ' ') {
                assert!(glyph(c).is_some(), "no glyph for {:?}", c);
            }
        }
    }

    #[test]
    fn drawing_changes_pixels_only_where_the_controls_are() {
        let (w, h) = (1200u32, 540u32);
        let p = pad(w, h);
        let mut buf = vec![0u32; (w * h) as usize];
        p.draw(&mut buf, w, h);
        let (cx, cy, _) = p.pad;
        assert_ne!(buf[(cy as u32 * w + cx as u32) as usize], 0);
        assert_eq!(buf[(h / 2 * w + w / 2) as usize], 0, "the middle of the screen is untouched");
    }
}
