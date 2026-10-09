//! Ogg Vorbis decoding (pure Rust, with symphonia) for the bgm converted by
//! tools/convert_bgm.sh.

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/// Decode an Ogg Vorbis file to stereo f32: (sample rate, frames).
pub fn decode(bytes: Vec<u8>) -> Option<(u32, Vec<[f32; 2]>)> {
    let mss = MediaSourceStream::new(Box::new(std::io::Cursor::new(bytes)), Default::default());
    let mut hint = Hint::new();
    hint.with_extension("ogg");
    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
        .ok()?;
    let mut format = probed.format;
    let track = format.default_track()?;
    let id = track.id;
    let rate = track.codec_params.sample_rate?;
    let mut dec = symphonia::default::get_codecs().make(&track.codec_params, &DecoderOptions::default()).ok()?;
    let mut out = Vec::new();
    let mut buf: Option<SampleBuffer<f32>> = None;
    while let Ok(packet) = format.next_packet() {
        if packet.track_id() != id {
            continue;
        }
        let Ok(decoded) = dec.decode(&packet) else { continue };
        let spec = *decoded.spec();
        let ch = spec.channels.count().max(1);
        let b = buf.get_or_insert_with(|| SampleBuffer::new(decoded.capacity() as u64, spec));
        if b.capacity() < decoded.capacity() * ch {
            *b = SampleBuffer::new(decoded.capacity() as u64, spec);
        }
        b.copy_interleaved_ref(decoded);
        for f in b.samples().chunks_exact(ch) {
            out.push(if ch == 1 { [f[0], f[0]] } else { [f[0], f[1]] });
        }
    }
    (!out.is_empty()).then_some((rate, out))
}
