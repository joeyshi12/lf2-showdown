//! The `.dat` obfuscation.
//!
//! Decoding is `FUN_004148a0` (to `data\temporary.txt`) and `FUN_00414a30` (to a given
//! path); both skip the first 123 bytes of the file, then subtract the key byte from each
//! following byte. The key index advances once per skipped byte as well, so the first
//! real byte uses key index `123 % 37 = 12`.
//!
//! Encoding is `FUN_004146b0`: it writes 123 bytes of `rand() % 25 + 'A' + key[i]` as a
//! header (their values do not matter for decoding) and then adds the key byte to each
//! byte of the plain text.
//!
//! The original opens every file with `fopen(.., "r")`/`"w"`, which are MSVC text mode.
//! Reading in text mode turns CR LF into LF and stops at the first 0x1A byte; writing
//! turns LF into CR LF. The game decodes to `data\temporary.txt` and then reads that file
//! back in text mode, so the text its parser sees is
//! `text_read(text_write(decode(text_read(raw))))`. [`decode_file`] reproduces that.

pub const KEY: &[u8; 37] = b"SiuHungIsAGoodBearBecauseHeIsVeryGood";
pub const HEADER_LEN: usize = 123;

/// MSVC 2005 text-mode read of a whole disk file: CR LF becomes LF, a lone CR is kept,
/// and a 0x1A byte ends the file.
pub fn text_mode_read(raw: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(raw.len());
    let mut i = 0;
    while i < raw.len() {
        let c = raw[i];
        if c == 0x1a {
            break;
        }
        if c == b'\r' && raw.get(i + 1) == Some(&b'\n') {
            out.push(b'\n');
            i += 2;
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}

/// MSVC 2005 text-mode write: every LF is written as CR LF.
pub fn text_mode_write(text: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len() + text.len() / 16);
    for &c in text {
        if c == b'\n' {
            out.push(b'\r');
        }
        out.push(c);
    }
    out
}

/// The byte transform of `FUN_00414a30`, applied to bytes already read in text mode.
pub fn decode_bytes(input: &[u8]) -> Vec<u8> {
    input
        .iter()
        .enumerate()
        .skip(HEADER_LEN)
        .map(|(i, &c)| c.wrapping_sub(KEY[i % KEY.len()]))
        .collect()
}

/// The byte transform of `FUN_004146b0` with a caller-chosen header.
pub fn encode_bytes(header: &[u8; HEADER_LEN], plain: &[u8]) -> Vec<u8> {
    let mut out = header.to_vec();
    out.extend(
        plain
            .iter()
            .enumerate()
            .map(|(i, &c)| c.wrapping_add(KEY[(i + HEADER_LEN) % KEY.len()])),
    );
    out
}

/// What the game's parser reads after decoding `raw` (the bytes of a `.dat` file on
/// disk) through `data\temporary.txt`.
pub fn decode_file(raw: &[u8]) -> Vec<u8> {
    let decoded = decode_bytes(&text_mode_read(raw));
    text_mode_read(&text_mode_write(&decoded))
}

/// The bytes the encoder would write to disk for `plain`, given the 123 header bytes.
pub fn encode_file(header: &[u8; HEADER_LEN], plain: &[u8]) -> Vec<u8> {
    text_mode_write(&encode_bytes(header, &text_mode_read(plain)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_inverts_encode() {
        let header = [b'A'; HEADER_LEN];
        let plain = b"<bmp_begin>\nname: Test\n<bmp_end>\n".to_vec();
        let raw = encode_bytes(&header, &plain);
        assert_eq!(decode_bytes(&raw), plain);
    }

    #[test]
    fn first_plain_byte_uses_key_index_12() {
        let header = [0u8; HEADER_LEN];
        let raw = encode_bytes(&header, b"\0");
        assert_eq!(raw[HEADER_LEN], KEY[12]);
    }

    #[test]
    fn text_mode_read_collapses_crlf_and_stops_at_ctrl_z() {
        assert_eq!(text_mode_read(b"a\r\nb\rc\x1ad"), b"a\nb\rc");
    }
}
