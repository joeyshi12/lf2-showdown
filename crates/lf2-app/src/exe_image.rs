//! The parts of the original lf2.exe the game uses (see build.rs), built into the
//! program.

static IMAGE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/lf2_image.bin"));

/// The built-in image, or None when the program was built without the original.
pub fn built_in() -> Option<&'static [u8]> {
    (!IMAGE.is_empty()).then_some(IMAGE)
}

/// True when `exe` is the original lf2.exe v2.0a (its .data section starts where
/// the port expects and holds the .lfr key), not this program.
pub fn is_original(exe: &[u8]) -> bool {
    exe.len() == 3424256 && exe.get(0x4d7a0..0x4d7a4).is_some_and(|k| k.iter().all(|&c| c != 0))
}

static SYSTEM_FONT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/system_font.fon"));

/// The System font built into the program (see build.rs), parsed once.
pub fn system_font() -> Option<&'static lf2_data::fnt::BitmapFont> {
    static F: std::sync::OnceLock<Option<lf2_data::fnt::BitmapFont>> = std::sync::OnceLock::new();
    F.get_or_init(|| lf2_data::fnt::BitmapFont::from_fon(SYSTEM_FONT)).as_ref()
}
