//! Game controllers as joysticks 1 and 2 of the control settings: the first
//! controller connected is joystick 1, the second joystick 2. The direction pad and
//! the left stick are the directions; the bottom, right, left and top face buttons
//! are buttons 0, 1, 2 and 3 (by position, so A, B, X, Y on an Xbox controller and
//! B, A, Y, X on a Nintendo one).
//!
//! `Joysticks` writes them into the game once a tick. The desktop reads controllers
//! with gilrs (`Controllers`); the web build reads the browser's Gamepad API in
//! web/index.html.

use lf2_sim::fields::Globals;
use lf2_sim::input::{set_joystick, Joystick, JOYSTICK_BUTTONS, JOYSTICK_COUNT};

/// The fewest ticks a press reaches the game for, however short it was: the game
/// copies the joysticks into the players' objects only every other tick (0x419a60).
pub const MIN_TICKS: u8 = 2;

/// How far a stick has to move from the middle (of 1) to press a direction.
pub const STICK_THRESHOLD: f32 = 0.5;

const INPUTS: usize = 4 + JOYSTICK_BUTTONS;

fn to_inputs(j: &Joystick) -> [bool; INPUTS] {
    let mut a = [false; INPUTS];
    a[..4].copy_from_slice(&[j.up, j.down, j.left, j.right]);
    a[4..].copy_from_slice(&j.buttons);
    a
}

fn from_inputs(a: &[bool; INPUTS]) -> Joystick {
    Joystick { up: a[0], down: a[1], left: a[2], right: a[3], buttons: std::array::from_fn(|k| a[4 + k]) }
}

/// The directions a stick at (x, y) presses, x to the right and y up, each from -1
/// to 1: up, down, left, right.
pub fn stick_directions(x: f32, y: f32) -> [bool; 4] {
    [y > STICK_THRESHOLD, y < -STICK_THRESHOLD, x < -STICK_THRESHOLD, x > STICK_THRESHOLD]
}

/// The joysticks as the game last saw them, for `MIN_TICKS`.
#[derive(Default)]
pub struct Joysticks {
    /// Per joystick and input, the ticks it has been pressed for (0 when it is not).
    ticks: [[u8; INPUTS]; JOYSTICK_COUNT],
}

impl Joysticks {
    /// Write the joysticks for this tick. `pressed[n]` is what controller n is
    /// pressing now, or pressed at all since the last call.
    pub fn write(&mut self, g: &mut Globals, pressed: &[Joystick; JOYSTICK_COUNT]) {
        for (n, (ticks, p)) in self.ticks.iter_mut().zip(pressed).enumerate() {
            let p = to_inputs(p);
            let mut down = [false; INPUTS];
            for i in 0..INPUTS {
                down[i] = p[i] || (0 < ticks[i] && ticks[i] < MIN_TICKS);
                ticks[i] = if down[i] { ticks[i].saturating_add(1) } else { 0 };
            }
            set_joystick(g, n, &from_inputs(&down));
        }
    }
}

/// The controllers connected to this computer, through gilrs.
#[cfg(feature = "native")]
pub struct Controllers {
    gilrs: Option<gilrs::Gilrs>,
    /// The buttons pressed since the last `poll`, per controller.
    since: std::collections::HashMap<gilrs::GamepadId, Joystick>,
}

#[cfg(feature = "native")]
impl Controllers {
    pub fn new() -> Controllers {
        let gilrs = match gilrs::Gilrs::new() {
            Ok(g) => Some(g),
            Err(e) => {
                eprintln!("no game controllers: {}", e);
                None
            }
        };
        if let Some(g) = &gilrs {
            for (_, pad) in g.gamepads() {
                eprintln!("game controller: {}", pad.name());
            }
        }
        Controllers { gilrs, since: Default::default() }
    }

    /// What joysticks 1 and 2 press now or pressed since the last call.
    pub fn poll(&mut self) -> [Joystick; JOYSTICK_COUNT] {
        use gilrs::{Axis, Button, EventType};
        let mut out = [Joystick::default(); JOYSTICK_COUNT];
        let Some(g) = self.gilrs.as_mut() else { return out };
        let face = [Button::South, Button::East, Button::West, Button::North];
        let dpad = [Button::DPadUp, Button::DPadDown, Button::DPadLeft, Button::DPadRight];
        while let Some(ev) = g.next_event() {
            match ev.event {
                EventType::ButtonPressed(b, _) => {
                    let j = self.since.entry(ev.id).or_default();
                    if let Some(k) = face.iter().position(|&f| f == b) {
                        j.buttons[k] = true;
                    }
                    match dpad.iter().position(|&d| d == b) {
                        Some(0) => j.up = true,
                        Some(1) => j.down = true,
                        Some(2) => j.left = true,
                        Some(3) => j.right = true,
                        _ => {}
                    }
                }
                EventType::Connected => eprintln!("game controller: {}", g.gamepad(ev.id).name()),
                _ => {}
            }
        }
        let mut ids: Vec<gilrs::GamepadId> = g.gamepads().map(|(id, _)| id).collect();
        ids.sort_by_key(|&id| usize::from(id));
        for (slot, id) in out.iter_mut().zip(ids) {
            let pad = g.gamepad(id);
            let stick = stick_directions(pad.value(Axis::LeftStickX), pad.value(Axis::LeftStickY));
            let since = self.since.remove(&id).unwrap_or_default();
            let mut j = Joystick {
                up: stick[0] || pad.is_pressed(dpad[0]),
                down: stick[1] || pad.is_pressed(dpad[1]),
                left: stick[2] || pad.is_pressed(dpad[2]),
                right: stick[3] || pad.is_pressed(dpad[3]),
                buttons: face.map(|b| pad.is_pressed(b)),
            };
            j.up |= since.up;
            j.down |= since.down;
            j.left |= since.left;
            j.right |= since.right;
            for (b, s) in j.buttons.iter_mut().zip(since.buttons) {
                *b |= s;
            }
            *slot = j;
        }
        self.since.clear();
        out
    }
}

#[cfg(feature = "native")]
impl Default for Controllers {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lf2_sim::input::joystick;

    fn write(js: &mut Joysticks, g: &mut Globals, j: Joystick) -> Joystick {
        js.write(g, &[j, Joystick::default()]);
        joystick(g, 0)
    }

    #[test]
    fn a_press_shorter_than_a_tick_reaches_the_game_for_min_ticks() {
        let (mut js, mut g) = (Joysticks::default(), Globals::new());
        let a = Joystick { buttons: [true, false, false, false], ..Default::default() };
        assert_eq!(write(&mut js, &mut g, a), a);
        for _ in 1..MIN_TICKS {
            assert_eq!(write(&mut js, &mut g, Joystick::default()), a);
        }
        assert_eq!(write(&mut js, &mut g, Joystick::default()), Joystick::default());
    }

    #[test]
    fn a_held_press_is_released_when_let_go() {
        let (mut js, mut g) = (Joysticks::default(), Globals::new());
        let right = Joystick { right: true, ..Default::default() };
        for _ in 0..5 {
            assert_eq!(write(&mut js, &mut g, right), right);
        }
        assert_eq!(write(&mut js, &mut g, Joystick::default()), Joystick::default());
    }

    #[test]
    fn the_second_controller_is_joystick_2() {
        let (mut js, mut g) = (Joysticks::default(), Globals::new());
        let up = Joystick { up: true, ..Default::default() };
        js.write(&mut g, &[Joystick::default(), up]);
        assert_eq!((joystick(&g, 0), joystick(&g, 1)), (Joystick::default(), up));
    }

    #[test]
    fn a_stick_presses_a_direction_past_the_threshold_and_two_on_a_diagonal() {
        assert_eq!(stick_directions(0.3, -0.2), [false; 4]);
        assert_eq!(stick_directions(0.9, 0.0), [false, false, false, true]);
        assert_eq!(stick_directions(-0.7, 0.7), [true, false, true, false]);
        assert_eq!(stick_directions(0.0, -1.0), [false, true, false, false]);
    }
}
