//! On Windows, text is drawn the way the original draws it (`draw_text` 0x401290):
//! GDI TextOutA on a device context with its default font (the System font) and
//! default opaque background, in the given text and background colours.

use std::cell::RefCell;
use windows_sys::Win32::Foundation::SIZE;
use windows_sys::Win32::Graphics::Gdi::*;

struct Canvas {
    dc: HDC,
    bits: *mut u32,
    w: i32,
    h: i32,
}

thread_local! {
    static CANVAS: RefCell<Option<Canvas>> = const { RefCell::new(None) };
}

fn canvas(w: i32, h: i32) -> Option<Canvas> {
    unsafe {
        let dc = CreateCompatibleDC(0);
        if dc == 0 {
            return None;
        }
        let mut info: BITMAPINFO = std::mem::zeroed();
        info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        info.bmiHeader.biWidth = w;
        info.bmiHeader.biHeight = -h; // top-down rows
        info.bmiHeader.biPlanes = 1;
        info.bmiHeader.biBitCount = 32;
        info.bmiHeader.biCompression = BI_RGB as u32;
        let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
        let bmp = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut bits, 0, 0);
        if bmp == 0 || bits.is_null() {
            DeleteDC(dc);
            return None;
        }
        SelectObject(dc, bmp);
        SelectObject(dc, GetStockObject(SYSTEM_FONT));
        Some(Canvas { dc, bits: bits as *mut u32, w, h })
    }
}

/// Draw `text` into `px` (`w` x `h`, 0x00RRGGBB) at x, y. `bk` and `color` are
/// COLORREFs as the game passes them. Returns false when GDI is not available.
pub fn text_out(px: &mut [u32], w: i32, h: i32, text: &[u8], bk: u32, color: u32, x: i32, y: i32) -> bool {
    CANVAS.with(|c| {
        let mut c = c.borrow_mut();
        if c.is_none() {
            *c = canvas(w, h);
        }
        let Some(cv) = c.as_ref() else { return false };
        if text.is_empty() {
            return true;
        }
        unsafe {
            let mut size = SIZE { cx: 0, cy: 0 };
            GetTextExtentPoint32A(cv.dc, text.as_ptr(), text.len() as i32, &mut size);
            let (x0, y0) = (x.max(0), y.max(0));
            let (x1, y1) = ((x + size.cx).min(cv.w).min(w), (y + size.cy).min(cv.h).min(h));
            if x1 <= x0 || y1 <= y0 {
                return true;
            }
            let bits = std::slice::from_raw_parts_mut(cv.bits, (cv.w * cv.h) as usize);
            for yy in y0..y1 {
                let (a, b) = ((yy * cv.w + x0) as usize, (yy * w + x0) as usize);
                bits[a..a + (x1 - x0) as usize].copy_from_slice(&px[b..b + (x1 - x0) as usize]);
            }
            SetBkMode(cv.dc, OPAQUE as _);
            SetBkColor(cv.dc, bk);
            SetTextColor(cv.dc, color);
            TextOutA(cv.dc, x, y, text.as_ptr(), text.len() as i32);
            GdiFlush();
            for yy in y0..y1 {
                let (a, b) = ((yy * cv.w + x0) as usize, (yy * w + x0) as usize);
                for k in 0..(x1 - x0) as usize {
                    px[b + k] = bits[a + k] & 0xffffff;
                }
            }
        }
        true
    })
}
