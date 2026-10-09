//! `.lfr` recordings: a 4-byte length, then the zlib-compressed recording struct
//! (0x630e18 bytes) whose first bytes are shifted by the key string at 0x44d7a0
//! (`replay_read_file` 0x43e620, `recording_save` 0x43dd60).
//!
//! Recording struct (offsets in bytes): +0 difficulty, +4 stage, +8/+0xc the F9/F10
//! cheat flags, +0x148 game mode, +0x14c player names (8 x 11), +0x1a4 background,
//! +0x1a8.. per object 0..17 at stride 4: team (+0x1a8), data id (+0x1f0), exists
//! (+0x238), u_08 (+0x280), ix (+0x2c8), iy (+0x310), iz (+0x358), mp (+0x3a0), 0x354
//! (+0x3e8), max hp (+0x430), 0x33c (+0x478), 0x344 (+0x4c0), 0x340 (+0x508); +0x550
//! the music file, +0x744 data checksum, +0x748 version, +0x74c.. battle troops,
//! +0x8c4 rng index, +0x8c8 the 3000-byte RNG table, +0x1488.. stage ids, +0x14b8 hp
//! check sums every 150 frames, +0x2b38 ten bytes of keys per frame.

pub const REC_SIZE: usize = 0x630e18;
/// File offset of the key string (VA 0x44d7a0) in lf2.exe.
const KEY_OFFSET: usize = 0x4d7a0;

fn key(exe: &[u8]) -> &[u8] {
    let k = exe.get(KEY_OFFSET..).unwrap_or(&[]);
    let n = k.iter().position(|&c| c == 0).unwrap_or(0);
    &k[..n]
}

/// Decode a .lfr file into the recording struct.
pub fn decode(file: &[u8], exe: &[u8]) -> Result<Vec<u8>, String> {
    if file.len() < 1000 {
        return Err("recording too short".into());
    }
    let mut d = file[4..].to_vec();
    for (i, &k) in key(exe).iter().enumerate().take(d.len()) {
        d[i] = d[i].wrapping_sub(k).wrapping_add(0x30);
    }
    let mut out = Vec::new();
    crate::flate::inflate_zlib(&d, &mut out)?;
    if out.len() != REC_SIZE {
        return Err(format!("recording has {} bytes, not {}", out.len(), REC_SIZE));
    }
    Ok(out)
}

/// Encode a recording struct as a .lfr file.
pub fn encode(rec: &[u8], exe: &[u8]) -> Vec<u8> {
    let mut d = crate::flate::deflate_zlib(rec);
    for (i, &k) in key(exe).iter().enumerate().take(d.len()) {
        d[i] = k.wrapping_add(d[i]).wrapping_sub(0x30);
    }
    let mut out = (d.len() as u32).to_le_bytes().to_vec();
    out.extend_from_slice(&d);
    out
}
