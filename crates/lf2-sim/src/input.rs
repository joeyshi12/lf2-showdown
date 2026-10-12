//! Keyboard effects of the original window procedure (`wndproc`, 0x43b3d0).
//! WM_KEYDOWN/UP writes `d`/`u` at 0x455378 + vk, including repeats. WM_SYSKEY
//! leaves the array unchanged; a key released while Alt is held therefore stays
//! `d` (the platform handles Alt+Enter). Esc's key-up is hidden by its message box.
//! Key-down advances the `lf2.net` and `herofighter.com` detectors at
//! 0x45857c/0x458578. Repeating the last matched letter keeps progress; repeating the
//! final letter presses 0xf9/0xf8 again, toggling 0x458428/0x45842c.

use crate::fields::Globals;
use crate::world::KEYS;

/// State of the "lf2.net" detector (`DAT_0045857c`).
pub const WORD_LF2NET: u32 = 0x45857c;
/// State of the "herofighter.com" detector (`DAT_00458578`).
pub const WORD_HEROFIGHTER: u32 = 0x458578;

/// Virtual-key codes of "lf2.net" (0xbe is the period key).
const LF2NET: [u8; 7] = [0x4c, 0x46, 0x32, 0xbe, 0x4e, 0x45, 0x54];
/// Virtual-key codes of "herofighter.com".
const HEROFIGHTER: [u8; 15] = [0x48, 0x45, 0x52, 0x4f, 0x46, 0x49, 0x47, 0x48, 0x54, 0x45, 0x52, 0xbe, 0x43, 0x4f, 0x4d];

/// What the platform layer has to do after a key event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyAction {
    None,
    /// Esc: the original shows the message box "Are you sure to quit?" (Yes/No) and
    /// quits on Yes.
    AskQuit,
}

/// One detector step: `state` is the number of letters matched so far.
fn word_step(g: &mut Globals, at: u32, word: &[u8], fire: u8, vk: u8) {
    let s = g.m.i32(at);
    let last = word.len() as i32 - 1;
    if !(0..=last).contains(&s) {
        g.m.set_i32(at, 0);
        return;
    }
    if vk == word[s as usize] {
        if s == last {
            g.m.set_u8(KEYS + fire as u32, b'd');
        } else {
            g.m.set_i32(at, s + 1);
        }
    } else if !(s > 0 && vk == word[s as usize - 1]) {
        g.m.set_i32(at, 0);
    }
}

/// WM_KEYDOWN with virtual-key code `vk` (also each autorepeat message).
pub fn key_down(g: &mut Globals, vk: u8) -> KeyAction {
    g.m.set_u8(KEYS + vk as u32, b'd');
    word_step(g, WORD_LF2NET, &LF2NET, 0xf9, vk);
    word_step(g, WORD_HEROFIGHTER, &HEROFIGHTER, 0xf8, vk);
    if vk == 0x1b {
        KeyAction::AskQuit
    } else {
        KeyAction::None
    }
}

/// WM_KEYUP with virtual-key code `vk`.
pub fn key_up(g: &mut Globals, vk: u8) {
    g.m.set_u8(KEYS + vk as u32, b'u');
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh() -> Globals {
        let mut g = Globals::new();
        for k in 0..256 {
            g.m.set_u8(KEYS + k, b'u');
        }
        g
    }

    fn typed(g: &mut Globals, s: &[u8]) {
        for &c in s {
            key_down(g, c);
            key_up(g, c);
        }
    }

    #[test]
    fn key_down_and_up_write_d_and_u() {
        let mut g = fresh();
        assert_eq!(key_down(&mut g, 0x41), KeyAction::None);
        assert_eq!(g.m.u8(KEYS + 0x41), b'd');
        // a repeated WM_KEYDOWN writes 'd' again, after the game consumed the key
        g.m.set_u8(KEYS + 0x41, b'u');
        key_down(&mut g, 0x41);
        assert_eq!(g.m.u8(KEYS + 0x41), b'd');
        key_up(&mut g, 0x41);
        assert_eq!(g.m.u8(KEYS + 0x41), b'u');
    }

    #[test]
    fn esc_writes_its_key_and_asks_to_quit() {
        let mut g = fresh();
        assert_eq!(key_down(&mut g, 0x1b), KeyAction::AskQuit);
        assert_eq!(g.m.u8(KEYS + 0x1b), b'd');
    }

    #[test]
    fn typed_lf2_net_writes_key_f9() {
        let mut g = fresh();
        typed(&mut g, b"LF2\xbeNE");
        assert_eq!(g.m.u8(KEYS + 0xf9), b'u');
        assert_eq!(g.m.i32(WORD_LF2NET), 6);
        typed(&mut g, b"T");
        assert_eq!(g.m.u8(KEYS + 0xf9), b'd');
        // the state stays on the last letter: another T writes the key again
        assert_eq!(g.m.i32(WORD_LF2NET), 6);
        g.m.set_u8(KEYS + 0xf9, b'u');
        typed(&mut g, b"T");
        assert_eq!(g.m.u8(KEYS + 0xf9), b'd');
        typed(&mut g, b"X");
        assert_eq!(g.m.i32(WORD_LF2NET), 0);
    }

    #[test]
    fn typed_herofighter_com_writes_key_f8() {
        let mut g = fresh();
        typed(&mut g, b"HEROFIGHTER\xbeCO");
        assert_eq!(g.m.u8(KEYS + 0xf8), b'u');
        typed(&mut g, b"M");
        assert_eq!(g.m.u8(KEYS + 0xf8), b'd');
        assert_eq!(g.m.u8(KEYS + 0xf9), b'u');
    }

    #[test]
    fn repeats_of_the_last_matched_letter_keep_the_state() {
        let mut g = fresh();
        // "LLFF2..NNEET": each letter repeated, as autorepeat of a held key would
        typed(&mut g, b"LLFF22\xbe\xbeNNEET");
        assert_eq!(g.m.u8(KEYS + 0xf9), b'd');
        // "HEE" keeps state 2, "HEH" restarts at 0 because H is neither R nor E
        let mut g = fresh();
        typed(&mut g, b"HEE");
        assert_eq!(g.m.i32(WORD_HEROFIGHTER), 2);
        typed(&mut g, b"H");
        assert_eq!(g.m.i32(WORD_HEROFIGHTER), 0);
    }

    #[test]
    fn a_wrong_letter_restarts_the_word() {
        let mut g = fresh();
        typed(&mut g, b"LF2X\xbeNET");
        assert_eq!(g.m.u8(KEYS + 0xf9), b'u');
        // a word that starts again in the middle is not found: the L after "LF" sends
        // the detector back to 0 (it is neither 2 nor F), it does not count as a new start
        typed(&mut g, b"LFLF2\xbeNET");
        assert_eq!(g.m.u8(KEYS + 0xf9), b'u');
    }

    #[test]
    fn both_words_are_followed_at_once() {
        let mut g = fresh();
        // "herofighter.com" then "lf2.net"; letters of one word reset the other
        typed(&mut g, b"HEROFIGHTER\xbeCOMLF2\xbeNET");
        assert_eq!(g.m.u8(KEYS + 0xf8), b'd');
        assert_eq!(g.m.u8(KEYS + 0xf9), b'd');
    }
}
