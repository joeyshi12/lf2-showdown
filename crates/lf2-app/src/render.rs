//! Software renderer for a match frame: a port of the drawing done inside
//! `match_frame` (background 0x41a250, objects 0x41a5a0 / 0x40de30, HUD 0x41ae60).
//! Pixels are 0x00RRGGBB; black (0) in a sprite is transparent, as the original sets a
//! black colour key on every surface.

use crate::assets::{Assets, Surface};
use lf2_sim::World;

pub const W: i32 = 794;
pub const H: i32 = 550;

pub struct Frame {
    pub px: Vec<u32>,
    /// The mouse position (game pixels) to draw the menus' cursor at, when the
    /// platform has a newer one than the tick that drew the picture: the cursor then
    /// follows the mouse on every picture, not only at ticks. None draws it where
    /// the tick put it.
    pub cursor: Option<(i32, i32)>,
}

impl Frame {
    pub fn new() -> Self {
        Frame { px: vec![0; (W * H) as usize], cursor: None }
    }

    pub fn fill(&mut self, x: i32, y: i32, w: i32, h: i32, rgb: u32) {
        for yy in y.max(0)..(y + h).min(H) {
            for xx in x.max(0)..(x + w).min(W) {
                self.px[(yy * W + xx) as usize] = rgb;
            }
        }
    }

    /// `blit_sprite` (0x43f010) without mirroring: cell `cell` (or the whole image when
    /// `cell` < 0 or the surface has no cells) at (x, y); `key` skips black pixels.
    pub fn blit(&mut self, s: &Surface, x: i32, y: i32, cell: i32, key: bool) {
        let (sx, sy, w, h) = if cell < 0 || cell as usize >= s.cells.len() {
            if cell >= 0 && !s.cells.is_empty() {
                return;
            }
            (0, 0, s.img.w as i32, s.img.h as i32)
        } else {
            let c = s.cells[cell as usize];
            (c.x, c.y, c.w, c.h)
        };
        self.blit_rect(s, sx, sy, w, h, x, y, key);
    }

    /// Copy a rectangle of `s` to (dx, dy), clipped to the screen and to the image.
    pub fn blit_rect(&mut self, s: &Surface, sx: i32, sy: i32, w: i32, h: i32, dx: i32, dy: i32, key: bool) {
        let (iw, ih) = (s.img.w as i32, s.img.h as i32);
        for yy in 0..h {
            let (py, ty) = (sy + yy, dy + yy);
            if py < 0 || py >= ih || ty < 0 || ty >= H {
                continue;
            }
            for xx in 0..w {
                let (px, tx) = (sx + xx, dx + xx);
                if px < 0 || px >= iw || tx < 0 || tx >= W {
                    continue;
                }
                let v = s.img.px[(py * iw + px) as usize];
                if key && v == 0 {
                    continue;
                }
                self.px[(ty * W + tx) as usize] = v;
            }
        }
    }

    /// Text in the WORDS font (8x16 cells, 9 pixels apart).
    pub fn text(&mut self, a: &Assets, font: &str, x: i32, y: i32, s: &str) {
        let Some(f) = a.res.get(font) else { return };
        for (i, c) in s.bytes().enumerate() {
            self.blit(f, x + 9 * i as i32, y, c as i32, true);
        }
    }
}

/// Objects that moved further than this many pixels between two ticks (teleports,
/// respawns) are not moved between the ticks; the same holds for the camera.
const MAX_STEP: i32 = 80;

fn in_match(w: &World) -> bool {
    w.g.match_phase() == 0 && w.g.m.i32(lf2_sim::world::GAME) == 2
}

/// Object positions (ix, iy, iz) and the camera after one tick.
pub struct Positions {
    in_match: bool,
    cam: i32,
    objs: Vec<Option<[i32; 3]>>,
}

impl Positions {
    pub fn of(w: &World) -> Self {
        let objs = (0..w.objs.len()).map(|i| w.exists(i).then(|| [w.objs[i].ix(), w.objs[i].iy(), w.objs[i].iz()])).collect();
        Positions { in_match: in_match(w), cam: w.g.m.i32(lf2_sim::camera::CAMERA_X), objs }
    }
}

/// What is needed to show more pictures than ticks. The pictures run one tick behind
/// the simulation: between tick N and tick N+1 the picture is tick N's (its sprites,
/// texts and everything else), with the objects and the camera moved part of the way
/// towards their positions in tick N+1. At the start of a tick that is exactly tick
/// N's picture, and the motion on the screen always goes forward.
pub struct Tween {
    /// The state of tick N (only what drawing reads).
    shown: Option<World>,
    /// Positions in tick N+1.
    next: Option<Positions>,
}

impl Tween {
    pub fn new() -> Self {
        Tween { shown: None, next: None }
    }

    /// Call before `World::step`: keeps what tick N draws.
    pub fn before_tick(&mut self, w: &World) {
        let mut c = World::with_state(w.g.clone(), w.objs.clone(), w.gd.clone());
        c.draw = w.draw.clone();
        self.shown = Some(c);
    }

    /// Call after `World::step`.
    pub fn after_tick(&mut self, w: &World) {
        self.next = Some(Positions::of(w));
    }

    /// Forget the kept tick (after loading a recording or starting a new match).
    pub fn clear(&mut self) {
        self.shown = None;
        self.next = None;
    }

    pub fn ready(&self) -> bool {
        self.shown.is_some() && self.next.is_some()
    }
}

/// Run `f` on `w` with every object that exists in `w` and in `next`, and the camera,
/// moved by `t` (0..1) of the way from its position in `w` to its position in `next`;
/// then put the positions back. Nothing is moved outside a match, or when the camera
/// jumped.
fn with_moved<R>(w: &mut World, next: &Positions, t: f64, f: impl FnOnce(&World) -> R) -> R {
    use lf2_sim::camera::CAMERA_X;
    let lerp = |a: i32, b: i32| a + ((b - a) as f64 * t).round() as i32;
    let cam = w.g.m.i32(CAMERA_X);
    if !next.in_match || !in_match(w) || (next.cam - cam).abs() > MAX_STEP || t <= 0.0 {
        return f(w);
    }
    let mut moved = Vec::new();
    for (i, n) in next.objs.iter().enumerate() {
        let Some(n) = n else { continue };
        if !w.exists(i) {
            continue;
        }
        let o = &mut w.objs[i];
        let now = [o.ix(), o.iy(), o.iz()];
        if (0..3).any(|k| (n[k] - now[k]).abs() > MAX_STEP) {
            continue;
        }
        o.set_ix(lerp(now[0], n[0]));
        o.set_iy(lerp(now[1], n[1]));
        o.set_iz(lerp(now[2], n[2]));
        moved.push((i, now));
    }
    w.g.m.set_i32(CAMERA_X, lerp(cam, next.cam));
    let r = f(w);
    w.g.m.set_i32(CAMERA_X, cam);
    for (i, [x, y, z]) in moved {
        let o = &mut w.objs[i];
        o.set_ix(x);
        o.set_iy(y);
        o.set_iz(z);
    }
    r
}

/// Per-layer animation counters (`+0x4d465d4`), which live in the file manager in the
/// original and are only used for drawing.
pub struct Renderer {
    pub layer_counters: Vec<Vec<i32>>,
    /// Player names under the fighters when the globals at 0x44fcc0 hold none (from
    /// control.txt; "1".."4" by default).
    pub names: [String; 10],
}

impl Renderer {
    pub fn new(gd: &lf2_data::GameData) -> Self {
        Renderer {
            layer_counters: gd.backgrounds.iter().map(|b| vec![0; b.layers.len()]).collect(),
            names: std::array::from_fn(|i| if i < 4 { (i + 1).to_string() } else { String::new() }),
        }
    }

    /// One frame's picture, as `frame` (0x43e9a0) makes it: the back buffer is
    /// filled with black (0x401250, a DirectDraw colour-fill Blt), then the match
    /// is drawn while match_phase is 0, then the frame's draw list.
    pub fn draw_frame(&mut self, w: &World, a: &Assets, f: &mut Frame) {
        f.px.fill(0);
        if w.g.match_phase() == 0 && w.g.m.i32(lf2_sim::world::GAME) == 2 {
            self.draw(w, a, f);
        }
        f.draw_list(a, &w.draw);
    }

    /// A picture `t` (0..1) of the way from the kept tick N towards tick N+1 (see
    /// `Tween`). `first` is true for the first picture after a tick: the background
    /// layer counters advance once per tick, as with `draw_frame`. Returns false when
    /// no tick is kept yet.
    pub fn draw_tween(&mut self, tw: &mut Tween, t: f64, first: bool, a: &Assets, f: &mut Frame) -> bool {
        let (Some(w), Some(next)) = (tw.shown.as_mut(), tw.next.as_ref()) else { return false };
        let counters = self.layer_counters.clone();
        with_moved(w, next, t.clamp(0.0, 1.0), |w| self.draw_frame(w, a, f));
        if !first {
            self.layer_counters = counters;
        }
        true
    }

    pub fn draw(&mut self, w: &World, a: &Assets, f: &mut Frame) {
        self.draw_background(w, a, f);
        self.draw_objects(w, a, f);
        draw_hud(w, a, f);
        if w.g.m.i32(0x450bfc) == 1 {
            if let Some(p) = a.res.get("PAUSE") {
                f.blit(p, 0x168, 0x120, -1, true);
            }
        }
    }

    /// 0x41a250.
    fn draw_background(&mut self, w: &World, a: &Assets, f: &mut Frame) {
        let bi = w.g.background() as usize;
        let cam = w.g.m.i32(0x450bc4);
        if bi == 99 {
            draw_lee_on_road(cam, a, f);
            return;
        }
        let Some(bg) = w.gd.backgrounds.get(bi) else { return };
        for (li, l) in bg.layers.iter().enumerate() {
            if l.rect != 0 {
                let c = match l.rect {
                    0x175317 => 0x104f10,
                    0x575347 => 0x5a4e4b,
                    0x977757 => 0x9a6e5a,
                    0x473f1f => 0x423818,
                    v => v,
                } as u32;
                f.fill(l.x, l.y, l.width, l.height, c);
                continue;
            }
            if l.cc > 0 {
                let c = &mut self.layer_counters[bi][li];
                *c = (*c + 1) % l.cc;
                if *c < l.c1 || l.c2 < *c {
                    continue;
                }
            }
            let Some(Some(s)) = a.backgrounds[bi].layers.get(li) else { continue };
            let key = l.transparency != 0;
            if l.loop_ == 0 {
                let off = if 0x31a < bg.width { -(((l.width - 0x31a) * cam) / (bg.width - 0x31a)) } else { 0 };
                f.blit(s, l.x + off, l.y, -1, key);
            } else {
                let d = bg.width - 0x31a;
                let shift = if d != 0 { ((l.width - 0x31a) * cam) / d } else { 0 };
                let mut x = l.x;
                while x < l.width {
                    f.blit(s, x - shift, l.y, -1, key);
                    x += l.loop_;
                }
            }
        }
    }

    /// 0x41a5a0 and 0x40de30.
    fn draw_objects(&mut self, w: &World, a: &Assets, f: &mut Frame) {
        let cam = w.g.m.i32(0x450bc4);
        let parity = w.g.tick_parity();
        let mut order: Vec<usize> = (0..400).filter(|&i| w.exists(i)).collect();
        // bubble sort by z, as the original (stable)
        let n = order.len();
        for pass in (0..n).rev() {
            for k in 0..pass {
                if w.objs[order[k + 1]].iz() < w.objs[order[k]].iz() {
                    order.swap(k, k + 1);
                }
            }
        }
        let bi = w.g.background() as usize;
        let (sw, sh) = w.gd.backgrounds.get(bi).map(|b| b.shadow_size).unwrap_or((0, 0));
        for &i in &order {
            let o = &w.objs[i];
            let d = w.data_of(i);
            let state = w.state(i);
            let blink = o.u_08().wrapping_abs() % 4 >= 2;
            if o.weapon_type() >= 0 && state != 0xbbd && state != 0x270d && d.id != 0xdf && d.id != 0xe0 && o.u_08() > -0x46 && !blink {
                if let Some(s) = &a.backgrounds.get(bi).and_then(|b| b.shadow.as_ref()) {
                    f.blit(s, (o.m.i32(0x1c) - sw / 2) + o.ix() - cam, o.iz() - sh / 2, -1, true);
                }
            }
            if !blink && o.u_08() > -0x19 {
                draw_object(w, a, f, i, cam, parity);
            }
            let lives = o.m.i32(0x30c);
            if lives > 1 {
                let s = format!("x{}", lives % 100);
                let x = (o.m.i32(0x1c) - (s.len() as i32 * 9) / 2) + o.ix() - cam;
                let y = (o.iz() - w.frame_of(i).centery) - 7 + o.iy();
                f.text(a, "WORDS0", x, y, &s);
            }
            if (i < 0x14 || (o.team() != 5 && d.kind == 0)) && o.u_08() > -0x19 {
                let name = if i < 10 {
                    // the names at 0x44fcc0, 11 bytes per slot (0x41a92f), which
                    // control.txt and, in a network game, the other computer fill
                    let at = 0x44fcc0 + 11 * i as u32;
                    let b: Vec<u8> = (0..11).map(|k| w.g.m.u8(at + k)).take_while(|&c| c != 0).collect();
                    // (a match started with World::new_vs_match has none there)
                    let n = if b.is_empty() { self.names[i].clone() } else { String::from_utf8_lossy(&b).into_owned() };
                    if w.g.m.i32(0x450b4c + 4 * i as u32) == -1 {
                        format!("[{}]", n)
                    } else {
                        n
                    }
                } else {
                    "Com".to_string()
                };
                let len = name.len() as i32;
                let mut x = (o.m.i32(0x1c) - (len * 9) / 2) + o.ix() - cam;
                x = x.max(0).min(len * -9 + 0x31a);
                let font = match o.team() {
                    1 => "WORDS1",
                    2 => "WORDS2",
                    3 => "WORDS3",
                    4 => "WORDS4",
                    _ => "WORDS0",
                };
                f.text(a, font, x, o.iz() + 3, &name);
            }
            // hit sparks (state changes are in lf2_sim::hits::advance_sparks)
            if let Some(sp) = a.res.get("SPARK") {
                for j in 0..o.m.i32(0x36c).clamp(0, 10) {
                    let k = o.m.i32(0x3c0 + 4 * j as u32);
                    let (sx, sy) = (o.m.i32(0x370 + 4 * j as u32) + o.m.i32(0x1c) - cam, o.m.i32(0x398 + 4 * j as u32));
                    if k < 5 {
                        f.blit(sp, sx - 0x33, sy - 0x28, k, true);
                    } else if (10..15).contains(&k) {
                        f.blit(sp, sx - 0x1e, sy - 0x18, k - 5, true);
                    } else if (20..29).contains(&k) {
                        f.blit(sp, sx - 0x33, sy - 0x28, (k - 0x14) / 2 + 10, true);
                    } else if (30..39).contains(&k) {
                        f.blit(sp, sx - 0x1e, sy - 0x18, (k - 0x1e) / 2 + 0xf, true);
                    }
                }
            }
        }
    }
}

/// 0x40de30: one object's sprite (and its weapon, which is a separate object).
fn draw_object(w: &World, a: &Assets, f: &mut Frame, i: usize, cam: i32, parity: i32) {
    let o = &w.objs[i];
    let shake = if o.m.i32(0xb4) < 0 { parity * 6 - 3 } else { 0 };
    let fr = w.frame_of(i);
    let pic = fr.pic + o.m.i32(0x318);
    let data = o.m.i32(0x368) as usize;
    let Some(sheets) = a.objects.get(data) else { return };
    let Some((first, _, normal, mirrored)) = sheets.sheets.iter().find(|(first, n, _, _)| *first <= pic && pic < first + n) else { return };
    let cell = pic - first;
    let y = (o.iz() - fr.centery) + o.iy();
    let base = o.m.i32(0x1c) + o.ix() - cam + shake;
    let state9997 = fr.state == 0x270d;
    if o.facing() == 0 {
        let mut x = base - fr.centerx;
        if state9997 {
            x = x.clamp(0, 0x2ca);
        }
        f.blit(normal, x, y, cell, true);
    } else {
        let cw = normal.cells.get(cell as usize).map_or(0, |c| c.w);
        let mut x = fr.centerx + base - cw;
        if state9997 {
            x = x.clamp(0, 0x2ca);
        }
        f.blit(mirrored, x, y, cell, true);
    }
    // a red marker at the bpoint (bleeding) when hp is below a third
    if !state9997 && o.hp() < o.max_hp() / 3 && fr.bpoint.0 > 0 {
        if let Some(bars) = a.res.get("BARS") {
            let dx = if o.facing() == 0 { fr.bpoint.0 - fr.centerx } else { fr.centerx - fr.bpoint.0 };
            f.blit_rect(bars, 0, 0x14, 1, 3, dx + base, (fr.bpoint.1 - fr.centery) + o.iz() + o.iy(), false);
        }
    }
}

/// 0x41ae60: the eight status panels.
/// `draw_background_layer` (0x41a050): background 99, "Lee On Road", drawn from
/// lf2.exe's bitmaps back99_1..3 and filled rectangles.
fn draw_lee_on_road(cam: i32, a: &Assets, f: &mut Frame) {
    let res = |n: &str| a.res.get(n).or_else(|| a.res.get(&n.to_ascii_uppercase()));
    if let Some(s) = res("back99_2") {
        f.blit(s, 0xfa - cam / 100, 0x78, -1, false);
    }
    if let Some(s) = res("back99_3") {
        for k in (0..4000).step_by(500) {
            f.blit(s, (k - (cam * 7) / 10) + 0x1e, 0xaf, -1, true);
        }
    }
    f.fill(0, 0x146, 0x31a, 0x14, 0x3f3f3f);
    f.fill(0, 0x159, 0x31a, 0x9c, 0x575757);
    f.fill(0, 0x1d7, 0x31a, 0x1e, 0x3f3f3f);
    f.fill(0, 0x148, 0x31a, 2, 0x373737);
    let mut x = (900 - cam) % 0x46;
    while x < 0x319 {
        f.fill(x, 0x124, 1, 0x25, 0x577fa7);
        f.fill(x + 1, 0x124, 1, 0x25, 0x2f4357);
        x += 0x46;
    }
    f.fill(0, 0x136, 0x31a, 1, 0x577fa7);
    f.fill(0, 0x137, 0x31a, 1, 0x2f4357);
    f.fill(0, 0x122, 0x31a, 1, 0x577fa7);
    f.fill(0, 0x123, 0x31a, 1, 0x2f4357);
    if let Some(s) = res("back99_1") {
        for k in (0..0xc80).step_by(0x140) {
            f.blit(s, (k - cam) + 10, 0x186, -1, false);
        }
    }
}

fn draw_hud(w: &World, a: &Assets, f: &mut Frame) {
    let (Some(frame), Some(bars)) = (a.res.get("FRAME"), a.res.get("BARS")) else { return };
    for k in 0..8usize {
        let (px, py) = ((k as i32 & 3) * 0xc6, (k as i32 >> 2) * 0x36);
        f.blit(frame, px, py, -1, false);
        let i = if w.exists(k) {
            k
        } else if w.exists(k + 10) {
            k + 10
        } else {
            continue;
        };
        let o = &w.objs[i];
        if let Some(s) = a.objects.get(o.m.i32(0x368) as usize).and_then(|s| s.small.as_ref()) {
            f.blit(s, px + 9, py + 7, -1, false);
        }
        if o.hp() > 0 {
            let x = px + 0x39;
            f.blit_rect(bars, 0, 0x1e, (o.dark_hp() * 0x1f) / 0x7d, 10, x, py + 0x10, false);
            f.blit_rect(bars, 0, 0x14, (o.hp() * 0x1f) / 0x7d, 10, x, py + 0x10, false);
            if (o.m.i32(0xe0) / 1000 == 1 || o.m.i32(0xe4) > 0) && w.g.tick_mod12() % 2 == 0 {
                f.blit_rect(bars, 0, 0x28, (o.hp() * 0x1f) / 0x7d, 10, x, py + 0x10, false);
            }
            f.blit_rect(bars, 0, 10, 0x7c, 10, x, py + 0x24, false);
            f.blit_rect(bars, 0, 0, (o.mp() * 0x1f) / 0x7d, 10, x, py + 0x24, false);
        }
        let font = match o.team() {
            1 => "WORDS1",
            2 => "WORDS2",
            3 => "WORDS3",
            4 => "WORDS4",
            _ => "WORDS0",
        };
        if let Some(s) = a.res.get(font) {
            f.blit(s, px + 5, py, 0xfe, true);
        }
    }
}

/// COLORREF (0x00BBGGRR) to 0x00RRGGBB.
fn colorref(c: u32) -> u32 {
    ((c & 0xff) << 16) | (c & 0xff00) | ((c >> 16) & 0xff)
}

impl Frame {
    /// Execute a frame's drawing requests (`World::draw`).
    pub fn draw_list(&mut self, a: &Assets, list: &[lf2_sim::draw::Draw]) {
        use lf2_sim::draw::{Draw, Sheet};
        for d in list {
            match d {
                Draw::Fill { x, y, w, h, rgb } => self.fill(*x, *y, *w, *h, *rgb),
                Draw::Blit { sheet, x, y, clip, key, mirror: _ } => {
                    // the menus' cursor (0x4246b0 and 0x423b00 draw it at the mouse,
                    // x at most 0x307, y + 2 at most 0x217)
                    let (x, y) = match (sheet, self.cursor) {
                        (Sheet::Global(lf2_sim::menu::main_menu::CURSOR), Some((mx, my))) => (&mx.min(0x307), &(my + 2).min(0x217)),
                        _ => (x, y),
                    };
                    let s = match sheet {
                        Sheet::Global(g) => {
                            if *g == lf2_sim::menu::main_menu::AD_SHEET {
                                a.ad.as_ref()
                            } else {
                                lf2_sim::menu::sheet_resource(*g).and_then(|r| a.res.get(r))
                            }
                        }
                        Sheet::MenuBack(n) => a.res.get(&format!("MENU_BACK{}", n)),
                        Sheet::Face(i) => a.objects.get(*i).and_then(|o| o.face.as_ref()),
                        Sheet::Small(i) => a.objects.get(*i).and_then(|o| o.small.as_ref()),
                    };
                    if let Some(s) = s {
                        self.blit(s, *x, *y, *clip, *key);
                    }
                }
                Draw::Text { text, bk, color, x, y } => {
                    #[cfg(windows)]
                    if crate::gdi_text::text_out(&mut self.px, W, H, text, *bk, *color, *x, *y) {
                        continue;
                    }
                    self.system_text(a, text, colorref(*bk), colorref(*color), *x, *y)
                }
            }
        }
    }

    /// TextOutA with the System font in opaque mode, where GDI is not available: the
    /// System font built into the program (Wine's vgasys.fon), or without it the
    /// game's WORDS0 font. (On Windows, `gdi_text` draws with GDI itself.)
    pub fn system_text(&mut self, a: &Assets, text: &[u8], bk: u32, color: u32, x: i32, y: i32) {
        if let Some(font) = crate::exe_image::system_font() {
            let px = &mut self.px;
            font.draw(text, x, y, |tx, ty, on| {
                if (0..W).contains(&tx) && (0..H).contains(&ty) {
                    px[(ty * W + tx) as usize] = if on { color } else { bk };
                }
            });
            return;
        }
        let Some(f) = a.res.get("WORDS0") else { return };
        for (i, &c) in text.iter().enumerate() {
            let cx = x + 8 * i as i32;
            self.fill(cx, y, 8, 16, bk);
            let Some(cell) = f.cells.get(c as usize) else { continue };
            for yy in 0..cell.h {
                for xx in 0..cell.w {
                    let (sx, sy) = (cell.x + xx, cell.y + yy);
                    if sx < 0 || sy < 0 || sx >= f.img.w as i32 || sy >= f.img.h as i32 {
                        continue;
                    }
                    if f.img.px[(sy * f.img.w as i32 + sx) as usize] != 0 {
                        let (tx, ty) = (cx + xx, y + yy);
                        if (0..W).contains(&tx) && (0..H).contains(&ty) {
                            self.px[(ty * W + tx) as usize] = color;
                        }
                    }
                }
            }
        }
    }
}
