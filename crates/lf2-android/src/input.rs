//! The app's input, read from Android's input queue before winit sees it, so that
//! each event's source is known: winit reports controllers' sticks as touches and
//! does not report their buttons.
//!
//! - Touchscreens, mice and styluses are touches (`Input::touch`).
//! - Controllers (gamepads, joysticks and their direction pads) are joysticks 1 and
//!   2 (`Input::controller`): the direction pad, hat and left stick are the
//!   directions, and the A, B, X and Y buttons (bottom, right, left and top on
//!   Android) are buttons 0 to 3. Start is F1 (pause) and Select is F4 (the menu).
//! - Keyboards, including the on-screen one, press keys by their virtual-key codes.
//! - The back button is F4.
//!
//! The touch controls start hidden when MainActivity reports a controller connected,
//! hide when one is connected later and show when the last one is gone.

use crate::codes::{vk, Pad};
use lf2_app::window::{Input, InputSource, Phase};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU8, Ordering};
use winit::platform::android::activity::input::{Axis, InputEvent, KeyAction, Keycode, MotionAction};
use winit::platform::android::activity::{AndroidApp, InputStatus};

/// What MainActivity last reported: 0 nothing new, 1 no controller, 2 a controller.
static CONTROLLER_NOTICE: AtomicU8 = AtomicU8::new(0);

/// MainActivity reports whether a controller is connected, when it starts and when
/// a device is added or removed.
#[no_mangle]
extern "C" fn Java_xyz_joeyshi_lf2showdown_MainActivity_nativeControllerConnected(
    _env: *mut libc::c_void,
    _this: *mut libc::c_void,
    connected: u8,
) {
    CONTROLLER_NOTICE.store(if connected != 0 { 2 } else { 1 }, Ordering::Release);
}

// AINPUT_SOURCE_* bits
const SOURCE_DPAD: u32 = 0x201;
const SOURCE_GAMEPAD: u32 = 0x401;
const SOURCE_JOYSTICK: u32 = 0x0100_0010;
/// AINPUT_SOURCE_CLASS_POINTER: touchscreens, mice, styluses.
const CLASS_POINTER: u32 = 0x2;

fn has(source: u32, bits: u32) -> bool {
    source & bits == bits
}

fn is_controller(source: u32) -> bool {
    has(source, SOURCE_GAMEPAD) || has(source, SOURCE_JOYSTICK) || has(source, SOURCE_DPAD)
}

pub struct AndroidInput {
    app: AndroidApp,
    pads: HashMap<i32, Pad>,
}

impl AndroidInput {
    pub fn new(app: AndroidApp) -> AndroidInput {
        AndroidInput { app, pads: HashMap::new() }
    }
}

impl InputSource for AndroidInput {
    fn read(&mut self, input: &mut Input<'_>) {
        match CONTROLLER_NOTICE.swap(0, Ordering::AcqRel) {
            1 => input.show_touch_controls(true),
            2 => input.show_touch_controls(false),
            _ => {}
        }
        let Ok(mut events) = self.app.input_events_iter() else { return };
        let pads = &mut self.pads;
        while events.next(|e| handle(e, pads, input)) {}
    }
}

fn handle(e: &InputEvent<'_>, pads: &mut HashMap<i32, Pad>, input: &mut Input<'_>) -> InputStatus {
    match e {
        InputEvent::MotionEvent(m) => {
            let source: u32 = m.source().into();
            if has(source, SOURCE_JOYSTICK) {
                let p = m.pointer_at_index(0);
                let pad = pads.entry(m.device_id()).or_default();
                pad.stick = (p.axis_value(Axis::X), p.axis_value(Axis::Y));
                pad.hat = (p.axis_value(Axis::HatX), p.axis_value(Axis::HatY));
                input.controller(m.device_id(), pad.joystick());
                return InputStatus::Handled;
            }
            if source & CLASS_POINTER == 0 {
                return InputStatus::Unhandled;
            }
            let id = |i: usize| m.pointer_at_index(i).pointer_id() as u64;
            let at = |i: usize| {
                let p = m.pointer_at_index(i);
                (p.x() as f64, p.y() as f64)
            };
            match m.action() {
                MotionAction::Down | MotionAction::PointerDown => {
                    let i = m.pointer_index();
                    let (x, y) = at(i);
                    input.touch(id(i), Phase::Start, x, y);
                }
                MotionAction::Up | MotionAction::PointerUp => {
                    let i = m.pointer_index();
                    let (x, y) = at(i);
                    input.touch(id(i), Phase::End, x, y);
                }
                MotionAction::Move => {
                    for i in 0..m.pointer_count() {
                        let (x, y) = at(i);
                        input.touch(id(i), Phase::Move, x, y);
                    }
                }
                MotionAction::Cancel => {
                    for i in 0..m.pointer_count() {
                        let (x, y) = at(i);
                        input.touch(id(i), Phase::End, x, y);
                    }
                }
                _ => return InputStatus::Unhandled,
            }
            InputStatus::Handled
        }
        InputEvent::KeyEvent(k) => {
            let down = match k.action() {
                KeyAction::Down => true,
                KeyAction::Up => false,
                _ => return InputStatus::Unhandled,
            };
            let first = down && k.repeat_count() == 0;
            let code = k.key_code();
            let source: u32 = k.source().into();
            match code {
                Keycode::Back => {
                    if first {
                        input.tap(0x73);
                    }
                    return InputStatus::Handled;
                }
                Keycode::VolumeUp | Keycode::VolumeDown | Keycode::VolumeMute => return InputStatus::Unhandled,
                Keycode::ButtonStart | Keycode::ButtonSelect => {
                    if first {
                        input.tap(if code == Keycode::ButtonStart { 0x70 } else { 0x73 });
                    }
                    return InputStatus::Handled;
                }
                _ => {}
            }
            if is_controller(source) {
                let pad = pads.entry(k.device_id()).or_default();
                let b = &mut pad.keys;
                let slot = match code {
                    Keycode::DpadUp => &mut b.up,
                    Keycode::DpadDown => &mut b.down,
                    Keycode::DpadLeft => &mut b.left,
                    Keycode::DpadRight => &mut b.right,
                    Keycode::ButtonA => &mut b.buttons[0],
                    Keycode::ButtonB => &mut b.buttons[1],
                    Keycode::ButtonX => &mut b.buttons[2],
                    Keycode::ButtonY => &mut b.buttons[3],
                    _ => return InputStatus::Unhandled,
                };
                *slot = down;
                input.controller(k.device_id(), pad.joystick());
                return InputStatus::Handled;
            }
            match vk(code.into()) {
                Some(v) => {
                    input.key(v, down);
                    InputStatus::Handled
                }
                None => InputStatus::Unhandled,
            }
        }
        _ => InputStatus::Unhandled,
    }
}
