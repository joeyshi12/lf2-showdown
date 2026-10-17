//! The parts of the Android input that do not need Android: key codes and a
//! controller's state.

use lf2_app::gamepad::stick_directions;
use lf2_sim::input::Joystick;

/// One controller's state from its key and motion events.
#[derive(Default)]
pub struct Pad {
    /// The direction pad and the A, B, X, Y buttons as key events.
    pub keys: Joystick,
    pub hat: (f32, f32),
    pub stick: (f32, f32),
}

impl Pad {
    pub fn joystick(&self) -> Joystick {
        // Android's y axes point down
        let s = stick_directions(self.stick.0, -self.stick.1);
        let h = stick_directions(self.hat.0, -self.hat.1);
        let k = self.keys;
        Joystick {
            up: k.up || s[0] || h[0],
            down: k.down || s[1] || h[1],
            left: k.left || s[2] || h[2],
            right: k.right || s[3] || h[3],
            buttons: k.buttons,
        }
    }
}

/// The Windows virtual-key code of an Android key code (AKEYCODE_*), for the keys the
/// desktop's `window::KEY_CODES` has. F10 is left out, as there.
pub fn vk(code: u32) -> Option<u8> {
    Some(match code {
        29..=54 => 0x41 + (code - 29) as u8,  // A-Z
        7..=16 => 0x30 + (code - 7) as u8,    // 0-9
        144..=153 => 0x60 + (code - 144) as u8, // keypad 0-9
        131..=139 => 0x70 + (code - 131) as u8, // F1-F9
        141 => 0x7a,                          // F11
        142 => 0x7b,                          // F12
        19 => 0x26,                           // up
        20 => 0x28,                           // down
        21 => 0x25,                           // left
        22 => 0x27,                           // right
        62 => 0x20,                           // space
        66 | 160 => 0x0d,                     // enter, keypad enter
        67 => 0x08,                           // backspace (DEL)
        112 => 0x2e,                          // delete (FORWARD_DEL)
        61 => 0x09,                           // tab
        111 => 0x1b,                          // escape
        59 | 60 => 0x10,                      // shift
        113 | 114 => 0x11,                    // ctrl
        55 => 0xbc,                           // ,
        56 => 0xbe,                           // .
        76 => 0xbf,                           // /
        68 => 0xc0,                           // `
        69 => 0xbd,                           // -
        70 => 0xbb,                           // =
        71 => 0xdb,                           // [
        72 => 0xdd,                           // ]
        73 => 0xdc,                           // \
        74 => 0xba,                           // ;
        75 => 0xde,                           // '
        122 => 0x24,                          // home
        123 => 0x23,                          // end
        92 => 0x21,                           // page up
        93 => 0x22,                           // page down
        124 => 0x2d,                          // insert
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letters_digits_and_the_game_keys_have_their_windows_codes() {
        // AKEYCODE_A, _Z, _0, _J, _ENTER, _DEL, _DPAD_UP, _F1, _F12, _SHIFT_LEFT, _GRAVE
        let pairs = [(29, 0x41), (54, 0x5a), (7, 0x30), (38, 0x4a), (66, 0x0d), (67, 0x08), (19, 0x26), (131, 0x70), (142, 0x7b), (59, 0x10), (68, 0xc0)];
        for (code, v) in pairs {
            assert_eq!(vk(code), Some(v), "AKEYCODE {}", code);
        }
        // F10 (140) is not a key of the game, as on the desktop; the volume keys (24, 25) are the system's
        for code in [140, 24, 25] {
            assert_eq!(vk(code), None, "AKEYCODE {}", code);
        }
    }

    #[test]
    fn every_control_txt_key_has_an_android_key() {
        for p in lf2_sim::setup::DEFAULT_CONTROLS {
            for code in p {
                assert!((0..300).any(|k| vk(k) == Some(code as u8)), "no Android key gives {}", code);
            }
        }
    }

    #[test]
    fn the_direction_pad_hat_and_stick_all_press_directions() {
        let mut p = Pad::default();
        assert_eq!(p.joystick(), Joystick::default());
        // Android's y points down: a stick pushed up has a negative y
        p.stick = (0.0, -0.9);
        assert!(p.joystick().up && !p.joystick().down);
        p.stick = (0.0, 0.0);
        p.hat = (1.0, 0.0);
        assert!(p.joystick().right);
        p.hat = (0.0, 0.0);
        p.keys.left = true;
        p.keys.buttons[3] = true;
        assert_eq!(p.joystick(), Joystick { left: true, buttons: [false, false, false, true], ..Default::default() });
    }
}
