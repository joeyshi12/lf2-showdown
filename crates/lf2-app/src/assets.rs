use lf2_data::image::{decode_bmp, decode_dib, pe_bitmaps, Image};
use lf2_data::GameData;
use std::collections::HashMap;
use std::path::Path;

/// One cell of a sprite sheet.
#[derive(Clone, Copy, Debug, Default)]
pub struct Cell {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

/// An image with its cell table, as the original's surface objects (0x1f50 bytes:
/// surface, width, height, cell count, then x/y/w/h tables).
#[derive(Clone, Debug)]
pub struct Surface {
    pub img: Image,
    pub cells: Vec<Cell>,
}

impl Surface {
    pub fn whole(img: Image) -> Self {
        Surface { img, cells: Vec::new() }
    }
}

pub struct Sheets {
    /// (first pic, cell count, normal, mirrored) per `file(...)` line.
    pub sheets: Vec<(i32, i32, Surface, Surface)>,
    pub face: Option<Surface>,
    pub small: Option<Surface>,
}

pub struct BgAssets {
    pub layers: Vec<Option<Surface>>,
    pub shadow: Option<Surface>,
}

pub struct Assets {
    pub objects: Vec<Sheets>,
    pub backgrounds: Vec<BgAssets>,
    pub res: HashMap<String, Surface>,
    pub ad: Option<Surface>,
}

/// The cells `ads_load_images` (0x43cc60) gives the ad picture.
const AD_CELLS: [(i32, i32, i32, i32); 11] = [
    (0, 0, 0x18d, 0x22), (0x18d, 0, 0x18d, 0x22), (0, 0x22, 0xc6, 0xc2), (0xc6, 0x22, 0xc6, 0xc2),
    (0x18c, 0x22, 0xc6, 0xc2), (0x252, 0x22, 0xc6, 0xc2), (0, 0xe4, 0xc6, 0xc2), (0xc6, 0xe4, 0xc6, 0xc2),
    (0x18c, 0xe4, 0xc6, 0xc2), (0x252, 0xe4, 0xc6, 0xc2), (0, 0x1a6, 0x31a, 0x80),
];

fn load_bmp(dir: &Path, win: &[u8]) -> Option<Image> {
    decode_bmp(&lf2_data::vfs::read(&lf2_data::resolve(dir, win)).ok()?)
}

fn mirror(img: &Image) -> Image {
    let mut px = vec![0; img.px.len()];
    for y in 0..img.h {
        for x in 0..img.w {
            px[y * img.w + (img.w - 1 - x)] = img.px[y * img.w + x];
        }
    }
    Image { w: img.w, h: img.h, px }
}

/// The 8x16 font cells of the WORDS resources (0x424xxx): cell c at ((c%16)*16, (c/16)*16+1).
fn font_cells() -> Vec<Cell> {
    (0..256).map(|i| Cell { x: (i & 15) * 16, y: (i & !15) + 1, w: 8, h: 16 }).collect()
}

/// The SPARK resource's 20 cells (0x42a0xx).
fn spark_cells() -> Vec<Cell> {
    let mut v = Vec::new();
    for (y, w, h) in [(0, 102, 80), (80, 61, 48), (128, 102, 80), (208, 61, 48)] {
        for i in 0..5 {
            v.push(Cell { x: i * w, y, w, h });
        }
    }
    v
}

impl Assets {
    pub fn load(gd: &GameData, exe: &[u8]) -> Assets {
        let dir = &gd.dir;
        let mut objects = Vec::new();
        for o in &gd.objects {
            let mut sheets = Vec::new();
            for k in 1..=o.sheet_count.min(o.sheets.len() - 1) {
                let sh = &o.sheets[k];
                let Some(img) = load_bmp(dir, &sh.file) else {
                    sheets.push((sh.first_pic, 0, Surface::whole(Image { w: 0, h: 0, px: vec![] }), Surface::whole(Image { w: 0, h: 0, px: vec![] })));
                    continue;
                };
                let name = String::from_utf8_lossy(&sh.file).into_owned();
                let mname = format!("{}_mirror.bmp", &name[..name.len().saturating_sub(4)]);
                let mimg = load_bmp(dir, mname.as_bytes()).unwrap_or_else(|| mirror(&img));
                let n = sh.row * sh.col;
                let mut cells = Vec::new();
                let mut mcells = Vec::new();
                for i in 0..n.max(0) {
                    let x = (i % sh.row.max(1)) * (sh.w + 1);
                    let y = (i / sh.row.max(1)) * (sh.h + 1);
                    cells.push(Cell { x, y, w: sh.w, h: sh.h });
                    mcells.push(Cell { x: img.w as i32 - x - sh.w, y, w: sh.w, h: sh.h });
                }
                sheets.push((sh.first_pic, n, Surface { img, cells }, Surface { img: mimg, cells: mcells }));
            }
            let face = load_bmp(dir, &o.head).map(Surface::whole);
            let small = if o.small.is_empty() { None } else { load_bmp(dir, &o.small).map(Surface::whole) };
            objects.push(Sheets { sheets, face, small });
        }
        let mut backgrounds = Vec::new();
        for b in &gd.backgrounds {
            let layers = b.layers.iter().map(|l| load_bmp(dir, &l.file).map(Surface::whole)).collect();
            let shadow = load_bmp(dir, &b.shadow).map(Surface::whole);
            backgrounds.push(BgAssets { layers, shadow });
        }
        let mut res = HashMap::new();
        for (name, dib) in pe_bitmaps(exe) {
            if let Some(img) = decode_dib(&dib, None) {
                let sheet = lf2_sim::menu::sprites::SHEETS.iter().find(|sh| sh.resource == name && !sh.clips.is_empty());
                let cells = match name.as_str() {
                    n if n.starts_with("WORDS") => font_cells(),
                    "SPARK" => spark_cells(),
                    _ => sheet
                        .map(|sh| sh.clips.iter().map(|&(x, y, w, h)| Cell { x, y, w, h }).collect())
                        .unwrap_or_default(),
                };
                res.insert(name, Surface { img, cells });
            }
        }
        let ad = lf2_data::ads::load(dir).and_then(|a| {
            let img = load_bmp(dir, format!("sprite\\sys\\ad{}.bmp", a.index).as_bytes())?;
            Some(Surface { img, cells: AD_CELLS.iter().map(|&(x, y, w, h)| Cell { x, y, w, h }).collect() })
        });
        Assets { objects, backgrounds, res, ad }
    }
}
