//! The desktop window: winit makes the window and reports keys and the mouse,
//! softbuffer shows the picture. The picture is the game's 794 x 550 screen, scaled
//! to the window by the largest factor that fits (nearest neighbour, with black bars
//! where the shape differs), so the window can be resized and made full screen.
//! Alt+Enter toggles full screen, as in the original.
//!
//! The event loop is pumped from the game's own loop (`Display::pump`) instead of
//! owning it.

use crate::render::{H, W};
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Duration;
use winit::application::ApplicationHandler;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, ModifiersState, PhysicalKey};
use winit::platform::pump_events::EventLoopExtPumpEvents;
use winit::window::{Fullscreen, Window, WindowId};

/// The Windows virtual-key code that the original's window procedure (0x43b3d0) gets
/// in WM_KEYDOWN/WM_KEYUP for a key; None for keys it never gets there. Both Shift keys
/// are VK_SHIFT (0x10) and both Ctrl keys VK_CONTROL (0x11), as in wParam. Alt and F10
/// come as WM_SYSKEYDOWN/WM_SYSKEYUP, which the original does not write into the key
/// array, so they have no code here. The keypad keys are the NumLock-on codes. Keys are
/// the physical ones, so the controls do not move with the keyboard layout.
pub const KEY_CODES: &[(KeyCode, u8)] = &[
    (KeyCode::KeyA, 0x41),
    (KeyCode::KeyB, 0x42),
    (KeyCode::KeyC, 0x43),
    (KeyCode::KeyD, 0x44),
    (KeyCode::KeyE, 0x45),
    (KeyCode::KeyF, 0x46),
    (KeyCode::KeyG, 0x47),
    (KeyCode::KeyH, 0x48),
    (KeyCode::KeyI, 0x49),
    (KeyCode::KeyJ, 0x4a),
    (KeyCode::KeyK, 0x4b),
    (KeyCode::KeyL, 0x4c),
    (KeyCode::KeyM, 0x4d),
    (KeyCode::KeyN, 0x4e),
    (KeyCode::KeyO, 0x4f),
    (KeyCode::KeyP, 0x50),
    (KeyCode::KeyQ, 0x51),
    (KeyCode::KeyR, 0x52),
    (KeyCode::KeyS, 0x53),
    (KeyCode::KeyT, 0x54),
    (KeyCode::KeyU, 0x55),
    (KeyCode::KeyV, 0x56),
    (KeyCode::KeyW, 0x57),
    (KeyCode::KeyX, 0x58),
    (KeyCode::KeyY, 0x59),
    (KeyCode::KeyZ, 0x5a),
    (KeyCode::Digit0, 0x30),
    (KeyCode::Digit1, 0x31),
    (KeyCode::Digit2, 0x32),
    (KeyCode::Digit3, 0x33),
    (KeyCode::Digit4, 0x34),
    (KeyCode::Digit5, 0x35),
    (KeyCode::Digit6, 0x36),
    (KeyCode::Digit7, 0x37),
    (KeyCode::Digit8, 0x38),
    (KeyCode::Digit9, 0x39),
    (KeyCode::Numpad0, 0x60),
    (KeyCode::Numpad1, 0x61),
    (KeyCode::Numpad2, 0x62),
    (KeyCode::Numpad3, 0x63),
    (KeyCode::Numpad4, 0x64),
    (KeyCode::Numpad5, 0x65),
    (KeyCode::Numpad6, 0x66),
    (KeyCode::Numpad7, 0x67),
    (KeyCode::Numpad8, 0x68),
    (KeyCode::Numpad9, 0x69),
    (KeyCode::NumpadMultiply, 0x6a),
    (KeyCode::NumpadAdd, 0x6b),
    (KeyCode::NumpadSubtract, 0x6d),
    (KeyCode::NumpadDecimal, 0x6e),
    (KeyCode::NumpadDivide, 0x6f),
    (KeyCode::ArrowLeft, 0x25),
    (KeyCode::ArrowUp, 0x26),
    (KeyCode::ArrowRight, 0x27),
    (KeyCode::ArrowDown, 0x28),
    (KeyCode::Enter, 0x0d),
    (KeyCode::NumpadEnter, 0x0d),
    (KeyCode::Space, 0x20),
    (KeyCode::Tab, 0x09),
    (KeyCode::Escape, 0x1b),
    (KeyCode::Backspace, 0x08),
    (KeyCode::ShiftLeft, 0x10),
    (KeyCode::ShiftRight, 0x10),
    (KeyCode::ControlLeft, 0x11),
    (KeyCode::ControlRight, 0x11),
    (KeyCode::Pause, 0x13),
    (KeyCode::CapsLock, 0x14),
    (KeyCode::NumLock, 0x90),
    (KeyCode::ScrollLock, 0x91),
    (KeyCode::SuperLeft, 0x5b),
    (KeyCode::SuperRight, 0x5c),
    (KeyCode::ContextMenu, 0x5d),
    (KeyCode::Backquote, 0xc0),
    (KeyCode::Comma, 0xbc),
    (KeyCode::Period, 0xbe),
    (KeyCode::Slash, 0xbf),
    (KeyCode::Semicolon, 0xba),
    (KeyCode::Quote, 0xde),
    (KeyCode::BracketLeft, 0xdb),
    (KeyCode::BracketRight, 0xdd),
    (KeyCode::Backslash, 0xdc),
    (KeyCode::Minus, 0xbd),
    (KeyCode::Equal, 0xbb),
    (KeyCode::F1, 0x70),
    (KeyCode::F2, 0x71),
    (KeyCode::F3, 0x72),
    (KeyCode::F4, 0x73),
    (KeyCode::F5, 0x74),
    (KeyCode::F6, 0x75),
    (KeyCode::F7, 0x76),
    (KeyCode::F8, 0x77),
    (KeyCode::F9, 0x78),
    (KeyCode::F11, 0x7a),
    (KeyCode::F12, 0x7b),
    (KeyCode::F13, 0x7c),
    (KeyCode::F14, 0x7d),
    (KeyCode::F15, 0x7e),
    (KeyCode::Insert, 0x2d),
    (KeyCode::Delete, 0x2e),
    (KeyCode::Home, 0x24),
    (KeyCode::End, 0x23),
    (KeyCode::PageUp, 0x21),
    (KeyCode::PageDown, 0x22),
];

pub fn vk(k: KeyCode) -> Option<u8> {
    KEY_CODES.iter().find(|(c, _)| *c == k).map(|(_, v)| *v)
}

/// Where the picture goes in a window of `w` x `h` pixels: the scale, and the left and
/// top edge and size of the picture. The largest scale at which the whole picture fits.
pub fn layout(w: u32, h: u32) -> (f64, u32, u32, u32, u32) {
    let s = (w as f64 / W as f64).min(h as f64 / H as f64);
    let (dw, dh) = (
        ((W as f64 * s).round() as u32).clamp(1, w.max(1)),
        ((H as f64 * s).round() as u32).clamp(1, h.max(1)),
    );
    (s, (w - dw.min(w)) / 2, (h - dh.min(h)) / 2, dw, dh)
}

/// Scale the picture `src` (W x H) into `dst` (`w` x `h`) as `layout` says: nearest
/// neighbour, black around it.
pub fn scale_into(src: &[u32], dst: &mut [u32], w: u32, h: u32) {
    let (_, ox, oy, dw, dh) = layout(w, h);
    let (w, ox, oy, dw, dh) = (
        w as usize,
        ox as usize,
        oy as usize,
        dw as usize,
        dh as usize,
    );
    let xmap: Vec<usize> = (0..dw)
        .map(|x| (x * W as usize / dw).min(W as usize - 1))
        .collect();
    let mut last_sy = usize::MAX;
    for y in 0..h as usize {
        let row = &mut dst[y * w..(y + 1) * w];
        if y < oy || y >= oy + dh {
            row.fill(0);
            continue;
        }
        let sy = ((y - oy) * H as usize / dh).min(H as usize - 1);
        if sy == last_sy && y > oy {
            // the same source row as the line above: copy it
            let (above, this) = dst.split_at_mut(y * w);
            this[..w].copy_from_slice(&above[(y - 1) * w..y * w]);
            continue;
        }
        last_sy = sy;
        row[..ox].fill(0);
        row[ox + dw..].fill(0);
        let s = &src[sy * W as usize..(sy + 1) * W as usize];
        if dw == W as usize {
            row[ox..ox + dw].copy_from_slice(s);
        } else {
            for (d, &sx) in row[ox..ox + dw].iter_mut().zip(&xmap) {
                *d = s[sx];
            }
        }
    }
}

struct App {
    title: String,
    fullscreen: bool,
    window: Option<Arc<Window>>,
    surface: Option<softbuffer::Surface<Arc<Window>, Arc<Window>>>,
    surface_size: (u32, u32),
    open: bool,
    events: Vec<(u8, bool)>,
    held: [bool; 256],
    mods: ModifiersState,
    /// The mouse in window pixels, and whether the left button is down.
    mouse: Option<(f64, f64)>,
    mouse_down: bool,
    /// The button went down since the game last looked: a click shorter than a frame
    /// still counts.
    pressed: bool,
    /// Geometry to restore after full screen (some platforms do not restore it
    /// themselves when `set_fullscreen(None)` is called).
    windowed_size: PhysicalSize<u32>,
    windowed_pos: Option<PhysicalPosition<i32>>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title(self.title.clone())
            .with_inner_size(PhysicalSize::new(W as u32, H as u32))
            .with_min_inner_size(PhysicalSize::new(W as u32 / 2, H as u32 / 2));
        let Ok(window) = el.create_window(attrs) else {
            self.open = false;
            return;
        };
        let window = Arc::new(window);
        // a whole number of screen pixels per game pixel, from the display scaling
        // (the window was asked for in physical pixels so that this is exact)
        let k = window.scale_factor().floor().max(1.0) as u32;
        self.windowed_size = PhysicalSize::new(W as u32 * k, H as u32 * k);
        let _ = window.request_inner_size(self.windowed_size);
        if self.fullscreen {
            window.set_fullscreen(Some(Fullscreen::Borderless(None)));
            window.set_cursor_visible(false);
        }
        let ctx = softbuffer::Context::new(window.clone()).expect("display context");
        self.surface =
            Some(softbuffer::Surface::new(&ctx, window.clone()).expect("window surface"));
        self.window = Some(window);
    }

    fn window_event(&mut self, _el: &ActiveEventLoop, _id: WindowId, ev: WindowEvent) {
        match ev {
            WindowEvent::CloseRequested | WindowEvent::Destroyed => self.open = false,
            WindowEvent::Resized(size) => {
                if self
                    .window
                    .as_ref()
                    .is_some_and(|w| w.fullscreen().is_none())
                {
                    self.windowed_size = size;
                }
            }
            WindowEvent::Moved(pos) => {
                if self
                    .window
                    .as_ref()
                    .is_some_and(|w| w.fullscreen().is_none())
                {
                    self.windowed_pos = Some(pos);
                }
            }
            WindowEvent::ModifiersChanged(m) => self.mods = m.state(),
            WindowEvent::KeyboardInput { event, .. } => {
                let PhysicalKey::Code(code) = event.physical_key else {
                    return;
                };
                let down = event.state == ElementState::Pressed;
                if down
                    && !event.repeat
                    && matches!(code, KeyCode::Enter | KeyCode::NumpadEnter)
                    && self.mods.alt_key()
                {
                    self.toggle_fullscreen();
                    return;
                }
                if let Some(v) = vk(code) {
                    self.held[v as usize] = down;
                    // the repeats are key downs too, as the WM_KEYDOWN messages of a held key
                    self.events.push((v, down));
                }
            }
            WindowEvent::Focused(false) => {
                // keys released while another window had the focus are never seen
                for (v, h) in self.held.iter_mut().enumerate() {
                    if std::mem::take(h) {
                        self.events.push((v as u8, false));
                    }
                }
                self.mouse_down = false;
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.mouse = Some((position.x, position.y))
            }
            WindowEvent::CursorLeft { .. } => {}
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => {
                self.mouse_down = state == ElementState::Pressed;
                self.pressed |= self.mouse_down;
            }
            _ => {}
        }
    }
}

impl App {
    fn toggle_fullscreen(&mut self) {
        let Some(w) = &self.window else { return };
        self.fullscreen = w.fullscreen().is_none();
        if self.fullscreen {
            self.windowed_size = w.inner_size();
            self.windowed_pos = w.outer_position().ok();
            w.set_fullscreen(Some(Fullscreen::Borderless(None)));
        } else {
            w.set_fullscreen(None);
            let _ = w.request_inner_size(self.windowed_size);
            if let Some(pos) = self.windowed_pos {
                w.set_outer_position(pos);
            }
        }
        // the game draws its own cursor
        w.set_cursor_visible(!self.fullscreen);
    }
}

pub struct Display {
    event_loop: EventLoop<()>,
    app: App,
}

impl Display {
    /// Open the window (full screen when asked) and wait until it exists.
    pub fn new(title: &str, fullscreen: bool) -> Display {
        let event_loop = EventLoop::new().expect("event loop");
        let app = App {
            title: title.into(),
            fullscreen,
            window: None,
            surface: None,
            surface_size: (0, 0),
            open: true,
            events: Vec::new(),
            held: [false; 256],
            mods: ModifiersState::empty(),
            mouse: None,
            mouse_down: false,
            pressed: false,
            windowed_size: PhysicalSize::new(W as u32, H as u32),
            windowed_pos: None,
        };
        let mut d = Display { event_loop, app };
        for _ in 0..200 {
            if d.app.window.is_some() || !d.app.open {
                break;
            }
            d.pump();
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(d.app.window.is_some(), "no window");
        d
    }

    /// Handle the events that came in.
    pub fn pump(&mut self) {
        let _ = self
            .event_loop
            .pump_app_events(Some(Duration::ZERO), &mut self.app);
    }

    pub fn is_open(&self) -> bool {
        self.app.open
    }

    /// Key events (virtual-key code, down) since the last call, with the repeats of
    /// held keys.
    pub fn take_keys(&mut self, out: &mut Vec<(u8, bool)>) {
        out.append(&mut self.app.events);
    }

    pub fn ctrl_down(&self) -> bool {
        self.app.mods.control_key()
    }

    /// The left button is down, or went down since the last call.
    pub fn mouse_down(&mut self) -> bool {
        std::mem::take(&mut self.app.pressed) || self.app.mouse_down
    }

    /// The mouse in the game's pixels, kept inside the picture; None before it has
    /// moved over the window.
    pub fn mouse(&self) -> Option<(i32, i32)> {
        let (mx, my) = self.app.mouse?;
        let size = self.app.window.as_ref()?.inner_size();
        let (s, ox, oy, _, _) = layout(size.width, size.height);
        let x = ((mx - ox as f64) / s).floor().clamp(0.0, W as f64 - 1.0) as i32;
        let y = ((my - oy as f64) / s).floor().clamp(0.0, H as f64 - 1.0) as i32;
        Some((x, y))
    }

    /// Show the picture (W x H pixels, 0x00RRGGBB).
    pub fn present(&mut self, px: &[u32]) {
        let (Some(window), Some(surface)) = (&self.app.window, &mut self.app.surface) else {
            return;
        };
        let size = window.inner_size();
        let (Some(w), Some(h)) = (NonZeroU32::new(size.width), NonZeroU32::new(size.height)) else {
            return;
        };
        if self.app.surface_size != (size.width, size.height) {
            if surface.resize(w, h).is_err() {
                return;
            }
            self.app.surface_size = (size.width, size.height);
        }
        if let Ok(mut buf) = surface.buffer_mut() {
            scale_into(px, &mut buf, size.width, size.height);
            let _ = buf.present();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every key the game reads directly: Enter, Shift, the arrows (replay camera), G
    /// (the title screen), F1-F9, F11, F12, and the keys of the default control.txt.
    #[test]
    fn keys_the_game_reads_have_codes() {
        let pairs = [
            (KeyCode::Enter, 0x0d),
            (KeyCode::ShiftLeft, 0x10),
            (KeyCode::ShiftRight, 0x10),
            (KeyCode::ControlLeft, 0x11),
            (KeyCode::ControlRight, 0x11),
            (KeyCode::ArrowLeft, 0x25),
            (KeyCode::ArrowUp, 0x26),
            (KeyCode::ArrowRight, 0x27),
            (KeyCode::ArrowDown, 0x28),
            (KeyCode::KeyG, 0x47),
            (KeyCode::F1, 0x70),
            (KeyCode::F2, 0x71),
            (KeyCode::F3, 0x72),
            (KeyCode::F4, 0x73),
            (KeyCode::F5, 0x74),
            (KeyCode::F6, 0x75),
            (KeyCode::F7, 0x76),
            (KeyCode::F8, 0x77),
            (KeyCode::F9, 0x78),
            (KeyCode::F11, 0x7a),
            (KeyCode::F12, 0x7b),
            (KeyCode::Escape, 0x1b),
            (KeyCode::Tab, 0x09),
            (KeyCode::Space, 0x20),
            (KeyCode::Backquote, 0xc0),
            (KeyCode::Comma, 0xbc),
            (KeyCode::Period, 0xbe),
            (KeyCode::Backspace, 0x08),
        ];
        for (k, code) in pairs {
            assert_eq!(vk(k), Some(code), "{:?}", k);
        }
        for p in lf2_sim::setup::DEFAULT_CONTROLS {
            for code in p {
                assert!(
                    KEY_CODES.iter().any(|(_, c)| *c == code as u8),
                    "no key gives the control.txt code {}",
                    code
                );
            }
        }
    }

    /// Alt and F10 arrive as WM_SYSKEYDOWN, which the original ignores.
    #[test]
    fn alt_and_f10_are_not_keys_of_the_game() {
        for k in [KeyCode::AltLeft, KeyCode::AltRight, KeyCode::F10] {
            assert_eq!(vk(k), None, "{:?}", k);
        }
    }

    /// No two different keys share a code, except the left and right Shift and Ctrl
    /// keys and the two Enter keys, which Windows does not tell apart in wParam either.
    #[test]
    fn codes_are_unique() {
        let shared = [0x10u8, 0x11, 0x0d];
        let mut seen = std::collections::HashMap::new();
        for (k, c) in KEY_CODES {
            if let Some(other) = seen.insert(*c, *k) {
                assert!(
                    shared.contains(c),
                    "{:?} and {:?} both give {:#x}",
                    other,
                    k,
                    c
                );
            }
        }
    }

    #[test]
    fn layout_fits_the_picture_and_centres_it() {
        assert_eq!(layout(794, 550), (1.0, 0, 0, 794, 550));
        let (s, ox, oy, dw, dh) = layout(1920, 1080);
        assert!((s - 1080.0 / 550.0).abs() < 1e-9);
        assert_eq!((dh, oy), (1080, 0));
        assert_eq!(dw, 1559);
        assert_eq!(ox, (1920 - 1559) / 2);
        let (_, ox, oy, dw, dh) = layout(1588, 1100);
        assert_eq!((ox, oy, dw, dh), (0, 0, 1588, 1100));
        // a window narrower than the picture's shape: bars above and below
        let (_, ox, oy, dw, dh) = layout(794, 800);
        assert_eq!((ox, dw, dh), (0, 794, 550));
        assert_eq!(oy, 125);
    }

    #[test]
    fn scaling_by_two_doubles_every_pixel_and_pads_with_black() {
        let src: Vec<u32> = (0..W * H)
            .map(|i| 0x010101 * (i % 200 + 1) as u32)
            .collect();
        // bars at the sides, then bars above and below
        for (w, h) in [(1588u32 + 20, 1100u32), (1588, 1100 + 10)] {
            let mut dst = vec![7u32; (w * h) as usize];
            scale_into(&src, &mut dst, w, h);
            let (_, ox, oy, dw, dh) = layout(w, h);
            assert_eq!((dw, dh), (1588, 1100));
            assert!(ox > 0 || oy > 0);
            for y in 0..h {
                for x in 0..w {
                    let d = dst[(y * w + x) as usize];
                    if x < ox || x >= ox + dw || y < oy || y >= oy + dh {
                        assert_eq!(d, 0, "bar at {},{}", x, y);
                    } else {
                        let (sx, sy) = ((x - ox) / 2, (y - oy) / 2);
                        assert_eq!(
                            d,
                            src[(sy * W as u32 + sx) as usize],
                            "pixel at {},{}",
                            x,
                            y
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn scaling_at_one_is_a_copy() {
        let src: Vec<u32> = (0..W * H).map(|i| i as u32 | 0x100000).collect();
        let mut dst = vec![0u32; (W * H) as usize];
        scale_into(&src, &mut dst, W as u32, H as u32);
        assert!(src == dst);
    }
}
