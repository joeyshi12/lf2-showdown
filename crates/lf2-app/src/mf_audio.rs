//! On Windows, the bgm (WMA) is decoded with Media Foundation, the system's own
//! decoders, as the original plays it through DirectShow.

use windows::core::HSTRING;
use windows::Win32::Media::MediaFoundation::*;
use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};

/// Decode an audio file to interleaved stereo f32. Returns (sample rate, frames),
/// or None when Media Foundation cannot read the file.
pub fn decode(path: &std::path::Path) -> Option<(u32, Vec<[f32; 2]>)> {
    decode_or_error(path).ok()
}

/// As `decode`, with the step that failed.
pub fn decode_or_error(path: &std::path::Path) -> Result<(u32, Vec<[f32; 2]>), String> {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        MFStartup(MF_VERSION, MFSTARTUP_FULL).map_err(|e| format!("MFStartup: {e}"))?;
        let out = read(path);
        let _ = MFShutdown();
        out
    }
}

unsafe fn read(path: &std::path::Path) -> Result<(u32, Vec<[f32; 2]>), String> {
    let step = |name: &'static str| move |e: windows::core::Error| format!("{name}: {e}");
    let url = HSTRING::from(path.as_os_str());
    let reader = MFCreateSourceReaderFromURL(&url, None).map_err(step("MFCreateSourceReaderFromURL"))?;
    let stream = MF_SOURCE_READER_FIRST_AUDIO_STREAM.0 as u32;
    let want = MFCreateMediaType().map_err(step("MFCreateMediaType"))?;
    want.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio).map_err(step("SetGUID major"))?;
    want.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_Float).map_err(step("SetGUID subtype"))?;
    reader.SetCurrentMediaType(stream, None, &want).map_err(step("SetCurrentMediaType"))?;
    let got = reader.GetCurrentMediaType(stream).map_err(step("GetCurrentMediaType"))?;
    let rate = got.GetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND).map_err(step("sample rate"))?;
    let channels = got.GetUINT32(&MF_MT_AUDIO_NUM_CHANNELS).map_err(step("channels"))?.max(1) as usize;
    let mut out = Vec::new();
    loop {
        let mut flags = 0u32;
        let mut sample: Option<IMFSample> = None;
        reader.ReadSample(stream, 0, None, Some(&mut flags), None, Some(&mut sample)).map_err(step("ReadSample"))?;
        if let Some(s) = sample {
            let buf = s.ConvertToContiguousBuffer().map_err(step("ConvertToContiguousBuffer"))?;
            let mut p: *mut u8 = std::ptr::null_mut();
            let mut len = 0u32;
            buf.Lock(&mut p, None, Some(&mut len)).map_err(step("Lock"))?;
            let f = std::slice::from_raw_parts(p as *const f32, len as usize / 4);
            for fr in f.chunks_exact(channels) {
                out.push(if channels == 1 { [fr[0], fr[0]] } else { [fr[0], fr[1]] });
            }
            let _ = buf.Unlock();
        }
        if flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
            break;
        }
    }
    Ok((rate, out))
}
