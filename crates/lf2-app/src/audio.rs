//! Sound effects: the game's wave files mixed in software, played with the volume and
//! pan the original gives DirectSound (0x419e60). Each sound id has one voice, and
//! playing it again restarts it, as with a DirectSound buffer.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use lf2_sim::sound::SoundEvent;
use std::sync::{Arc, Mutex};

pub use crate::sound_names::{BUFFER_SOUNDS, SYSTEM_SOUNDS};

#[derive(Clone)]
struct Wave {
    rate: u32,
    /// mono samples, -1..1
    data: Arc<Vec<f32>>,
}

struct Voice {
    wave: Wave,
    pos: f64,
    left: f32,
    right: f32,
}

/// The bgm: stereo samples, looped (music_event 0x401e90 rewinds on EC_COMPLETE).
struct Music {
    rate: u32,
    data: Arc<Vec<[f32; 2]>>,
    pos: f64,
    playing: bool,
}

struct Mixer {
    voices: Vec<Option<Voice>>,
    music: Option<Music>,
    /// Counts the files asked for, so a slow decode of an older file is dropped.
    music_gen: u64,
    music_gain: f32,
    out_rate: u32,
}

pub struct Audio {
    mixer: Arc<Mutex<Mixer>>,
    game_dir: std::path::PathBuf,
    /// The bgm file loaded (as the game names it).
    music_file: Vec<u8>,
    system: Vec<Option<Wave>>,
    /// The waves `play_buffer` plays, in `BUFFER_SOUNDS` order.
    buffers: Vec<Option<Wave>>,
    data: Vec<Option<Wave>>,
    _stream: Option<cpal::Stream>,
}

fn parse_wav(b: &[u8]) -> Option<Wave> {
    if b.get(0..4)? != b"RIFF" || b.get(8..12)? != b"WAVE" {
        return None;
    }
    let mut p = 12;
    let (mut ch, mut rate, mut bits) = (1u16, 22050u32, 16u16);
    while p + 8 <= b.len() {
        let id = &b[p..p + 4];
        let len = u32::from_le_bytes(b[p + 4..p + 8].try_into().ok()?) as usize;
        let body = b.get(p + 8..(p + 8 + len).min(b.len()))?;
        if id == b"fmt " {
            ch = u16::from_le_bytes(body.get(2..4)?.try_into().ok()?);
            rate = u32::from_le_bytes(body.get(4..8)?.try_into().ok()?);
            bits = u16::from_le_bytes(body.get(14..16)?.try_into().ok()?);
        } else if id == b"data" {
            let ch = ch.max(1) as usize;
            let mut out = Vec::new();
            match bits {
                8 => {
                    for f in body.chunks(ch) {
                        out.push((f[0] as f32 - 128.0) / 128.0);
                    }
                }
                16 => {
                    for f in body.chunks(2 * ch) {
                        if f.len() >= 2 {
                            out.push(i16::from_le_bytes([f[0], f[1]]) as f32 / 32768.0);
                        }
                    }
                }
                _ => return None,
            }
            return Some(Wave { rate, data: Arc::new(out) });
        }
        p += 8 + len + (len & 1);
    }
    None
}

/// DirectSound volume and pan (hundredths of a decibel) to per-channel gains.
fn gains(volume_cb: i32, pan_cb: i32) -> (f32, f32) {
    let amp = 10f32.powf(volume_cb.min(0) as f32 / 2000.0);
    let l = if pan_cb > 0 { 10f32.powf(-(pan_cb as f32) / 2000.0) } else { 1.0 };
    let r = if pan_cb < 0 { 10f32.powf(pan_cb as f32 / 2000.0) } else { 1.0 };
    (amp * l, amp * r)
}

impl Audio {
    pub fn new(gd: &lf2_data::GameData) -> Audio {
        let load = |name: &str| std::fs::read(lf2_data::resolve(&gd.dir, name.as_bytes())).ok().and_then(|b| parse_wav(&b));
        let system = SYSTEM_SOUNDS.iter().map(|n| load(&format!("data\\{}.wav", n))).collect();
        let data = gd.sounds.iter().map(|n| load(&String::from_utf8_lossy(n))).collect();
        let buffers = BUFFER_SOUNDS.iter().map(|n| load(n)).collect();
        let mixer = Arc::new(Mutex::new(Mixer {
            voices: (0..480 + BUFFER_SOUNDS.len()).map(|_| None).collect(),
            music: None,
            music_gen: 0,
            music_gain: 1.0,
            out_rate: 44100,
        }));
        let stream = Self::open(mixer.clone());
        Audio { mixer, game_dir: gd.dir.clone(), music_file: Vec::new(), system, data, buffers, _stream: stream }
    }

    fn open(mixer: Arc<Mutex<Mixer>>) -> Option<cpal::Stream> {
        let dev = cpal::default_host().default_output_device()?;
        let cfg = dev.default_output_config().ok()?;
        let channels = cfg.channels() as usize;
        let rate = cfg.sample_rate().0;
        mixer.lock().ok()?.out_rate = rate;
        let m = mixer.clone();
        let stream = dev
            .build_output_stream(
                &cfg.into(),
                move |out: &mut [f32], _| {
                    let mut m = m.lock().unwrap();
                    let out_rate = m.out_rate as f64;
                    for frame in out.chunks_mut(channels) {
                        let (mut l, mut r) = (0f32, 0f32);
                        for v in m.voices.iter_mut() {
                            let Some(voice) = v else { continue };
                            let i = voice.pos as usize;
                            if i >= voice.wave.data.len() {
                                *v = None;
                                continue;
                            }
                            let s = voice.wave.data[i];
                            l += s * voice.left;
                            r += s * voice.right;
                            voice.pos += voice.wave.rate as f64 / out_rate;
                        }
                        let gain = m.music_gain;
                        if let Some(mu) = m.music.as_mut() {
                            if mu.playing && !mu.data.is_empty() {
                                let i = mu.pos as usize % mu.data.len();
                                l += mu.data[i][0] * gain;
                                r += mu.data[i][1] * gain;
                                mu.pos = (mu.pos + mu.rate as f64 / out_rate) % mu.data.len() as f64;
                            }
                        }
                        frame[0] = l.clamp(-1.0, 1.0);
                        if channels > 1 {
                            frame[1] = r.clamp(-1.0, 1.0);
                        }
                        // the buffer is not cleared for us: silence any other channels
                        for c in frame.iter_mut().skip(2) {
                            *c = 0.0;
                        }
                    }
                },
                |e| eprintln!("audio: {}", e),
                None,
            )
            .ok()?;
        stream.play().ok()?;
        Some(stream)
    }

    /// Apply the frame's music changes (`World::music`): Some(file) plays a bgm file
    /// (or carries on with the loaded one), None stops it.
    pub fn music(&mut self, events: &[Option<Vec<u8>>], master: i32) {
        // music_play (0x401f30): IBasicAudio volume master * 34 - 3900 (hundredths
        // of a decibel), silent at 0
        if let Ok(mut m) = self.mixer.lock() {
            m.music_gain = if master < 1 { 0.0 } else { 10f32.powf((master * 0x22 - 0xf3c).min(0) as f32 / 2000.0) };
        }
        for e in events {
            match e {
                None => {
                    if let Ok(mut m) = self.mixer.lock() {
                        if let Some(mu) = m.music.as_mut() {
                            mu.playing = false;
                        }
                    }
                }
                Some(f) if *f == self.music_file => {
                    if let Ok(mut m) = self.mixer.lock() {
                        if let Some(mu) = m.music.as_mut() {
                            mu.playing = true;
                        }
                    }
                }
                Some(f) => {
                    self.music_file = f.clone();
                    let Ok(mut m) = self.mixer.lock() else { continue };
                    m.music = None;
                    m.music_gen += 1;
                    let gen = m.music_gen;
                    drop(m);
                    let path = lf2_data::resolve(&self.game_dir, f);
                    let mixer = self.mixer.clone();
                    std::thread::spawn(move || {
                        let Some((rate, data)) = decode_music(&path) else {
                            eprintln!("cannot play {}: convert the bgm to Ogg with tools/convert_bgm.sh, or install ffmpeg", path.display());
                            return;
                        };
                        if let Ok(mut m) = mixer.lock() {
                            if m.music_gen == gen {
                                m.music = Some(Music { rate, data: Arc::new(data), pos: 0.0, playing: true });
                            }
                        }
                    });
                }
            }
        }
    }

    /// Restart the `play_buffer` waves named in `names`.
    pub fn play_buffers(&mut self, names: &[&str]) {
        let Ok(mut m) = self.mixer.lock() else { return };
        for n in names {
            let Some(k) = BUFFER_SOUNDS.iter().position(|b| b == n) else { continue };
            let Some(Some(wave)) = self.buffers.get(k) else { continue };
            if let Some(v) = m.voices.get_mut(480 + k) {
                *v = Some(Voice { wave: wave.clone(), pos: 0.0, left: 1.0, right: 1.0 });
            }
        }
    }

    /// Play the frame's sounds. `master` is `DAT_0044d000` (0..100).
    pub fn play(&mut self, events: &[SoundEvent], master: i32) {
        let Ok(mut m) = self.mixer.lock() else { return };
        for e in events {
            let wave = if e.system { self.system.get(e.id) } else { self.data.get(e.id) };
            let Some(Some(wave)) = wave else { continue };
            // flush_sounds (0x419e60) skips silent sounds, and plays nothing at volume 0
            let vol = (e.left + e.right).min(100);
            if vol < 1 || master < 1 {
                continue;
            }
            let volume_cb = ((master - 100) * 0xed8) / 100 + ((vol - 100) * 2000) / 100;
            let pan_cb = ((e.right - e.left) * 0x5dc) / 100;
            let (left, right) = gains(volume_cb, pan_cb);
            let slot = if e.system { 400 + e.id } else { e.id };
            if let Some(v) = m.voices.get_mut(slot) {
                *v = Some(Voice { wave: wave.clone(), pos: 0.0, left, right });
            }
        }
    }
}

/// Decode a bgm file to stereo. An Ogg Vorbis file of the same name (made by
/// tools/convert_bgm.sh) is used first, on every platform. Otherwise the WMA file:
/// with Media Foundation on Windows, else (or when that fails) with the ffmpeg
/// program at 44.1 kHz.
fn decode_music(path: &std::path::Path) -> Option<(u32, Vec<[f32; 2]>)> {
    if let Some(d) = ogg_next_to(path).and_then(|p| std::fs::read(p).ok()).and_then(crate::ogg::decode) {
        return Some(d);
    }
    #[cfg(windows)]
    if let Some(d) = crate::mf_audio::decode(path) {
        return Some(d);
    }
    decode_ffmpeg(path).map(|d| (44100, d))
}

/// `bgm/main.wma` -> `bgm/main.ogg` when that file exists (any case).
fn ogg_next_to(path: &std::path::Path) -> Option<std::path::PathBuf> {
    let dir = path.parent()?;
    let stem = path.file_stem()?.to_string_lossy().to_ascii_lowercase();
    std::fs::read_dir(dir).ok()?.filter_map(|e| e.ok()).map(|e| e.path()).find(|p| {
        p.extension().is_some_and(|x| x.eq_ignore_ascii_case("ogg"))
            && p.file_stem().is_some_and(|s| s.to_string_lossy().to_ascii_lowercase() == stem)
    })
}

fn decode_ffmpeg(path: &std::path::Path) -> Option<Vec<[f32; 2]>> {
    let out = std::process::Command::new("ffmpeg")
        .args(["-v", "error", "-i"])
        .arg(path)
        .args(["-f", "f32le", "-ac", "2", "-ar", "44100", "-"])
        .stdin(std::process::Stdio::null())
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let b = out.stdout;
    Some(
        b.chunks_exact(8)
            .map(|c| [f32::from_le_bytes(c[0..4].try_into().unwrap()), f32::from_le_bytes(c[4..8].try_into().unwrap())])
            .collect(),
    )
}
