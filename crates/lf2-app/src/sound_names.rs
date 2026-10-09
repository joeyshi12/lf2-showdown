//! The names of the sound files the game plays by number.

/// Waves the game keeps in their own DirectSound buffers and plays with
/// `play_buffer` (0x401a30), unchanged volume and pan.
pub const BUFFER_SOUNDS: [&str; 5] =
    ["data\\m_join.wav", "data\\m_ok.wav", "data\\m_cancel.wav", "data\\m_pass.wav", "data\\m_end.wav"];

/// The system sounds loaded by match_frame (0x41bdxx), in id order.
pub const SYSTEM_SOUNDS: [&str; 18] = [
    "001", "002", "006", "010", "011", "004", "016", "017", "020", "021", "025", "032", "033", "039", "065", "066", "068", "085",
];
