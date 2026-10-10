//! Take what the game needs from the original lf2.exe at build time, so the program
//! does not read lf2.exe when it runs and can itself be named lf2.exe. Kept: the PE
//! headers, the initial .data section (globals, the .lfr key) and .rsrc (the bitmaps
//! the menus use), at their file offsets. The code sections (.text, .rdata) are left
//! as zeros. The original comes from LF2_EXE, LF2_DIR/lf2.exe or LF2_DIR/lf2_image.bin;
//! without those the image is empty and the program reads lf2.exe from the game
//! folder instead.
use std::path::PathBuf;

fn u16le(d: &[u8], o: usize) -> usize {
    u16::from_le_bytes([d[o], d[o + 1]]) as usize
}
fn u32le(d: &[u8], o: usize) -> usize {
    u32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]]) as usize
}

/// The System font for drawing text without GDI (Linux, the web): LF2_SYSTEM_FONT,
/// or Wine's vgasys.fon (LGPL, the font Wine's GDI uses for "System"). Empty when
/// neither is found; the renderer then falls back to the game's WORDS0 font.
fn system_font() {
    println!("cargo:rerun-if-env-changed=LF2_SYSTEM_FONT");
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("system_font.fon");
    let candidates: Vec<PathBuf> = std::env::var_os("LF2_SYSTEM_FONT")
        .map(PathBuf::from)
        .into_iter()
        .chain(["/usr/share/wine/fonts/vgasys.fon", "/usr/local/share/wine/fonts/vgasys.fon", "/opt/wine/share/wine/fonts/vgasys.fon"].map(PathBuf::from))
        .collect();
    for p in candidates {
        if let Ok(b) = std::fs::read(&p) {
            println!("cargo:rerun-if-changed={}", p.display());
            std::fs::write(&out, b).unwrap();
            return;
        }
    }
    println!("cargo:warning=no System font (set LF2_SYSTEM_FONT to a .fon, e.g. Wine's vgasys.fon); text uses the game's WORDS0 font");
    std::fs::write(&out, []).unwrap();
}

fn main() {
    system_font();
    println!("cargo:rerun-if-env-changed=LF2_EXE");
    println!("cargo:rerun-if-env-changed=LF2_DIR");
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("lf2_image.bin");
    let src = std::env::var_os("LF2_EXE")
        .map(PathBuf::from)
        .or_else(|| {
            // a game folder's lf2.exe, or the repository's game/lf2_image.bin, which
            // already holds only the parts kept here
            let d = PathBuf::from(std::env::var_os("LF2_DIR")?);
            [d.join("lf2.exe"), d.join("lf2_image.bin")].into_iter().find(|p| p.exists())
        });
    let Some(exe) = src.and_then(|p| {
        println!("cargo:rerun-if-changed={}", p.display());
        std::fs::read(p).ok()
    }) else {
        println!("cargo:warning=no original lf2.exe (set LF2_DIR or LF2_EXE); the program will read lf2.exe at run time");
        std::fs::write(&out, []).unwrap();
        return;
    };
    let pe = u32le(&exe, 0x3c);
    let nsec = u16le(&exe, pe + 6);
    let sec = pe + 24 + u16le(&exe, pe + 20);
    let mut img = vec![0u8; exe.len()];
    img[..sec + 40 * nsec].copy_from_slice(&exe[..sec + 40 * nsec]);
    for i in 0..nsec {
        let o = sec + 40 * i;
        let name = &exe[o..o + 8];
        let (size, raw) = (u32le(&exe, o + 16), u32le(&exe, o + 20));
        if name.starts_with(b".data") || name.starts_with(b".rsrc") {
            let end = (raw + size).min(exe.len());
            img[raw..end].copy_from_slice(&exe[raw..end]);
        }
    }
    std::fs::write(&out, img).unwrap();
}
