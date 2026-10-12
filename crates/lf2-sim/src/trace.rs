//! Snapshots and traces recorded from the original by `tools/harness/record.py`
//! (format described in `tools/harness/snapshot.py`).

use crate::fields::{Globals, Obj, GLOBALS_BASE, GLOBALS_LEN};

pub const OBJECTS: usize = 400;
pub const OBJ_SIZE: usize = 0x420;

#[derive(Clone)]
pub struct Snapshot {
    pub globals: Globals,
    /// `None` where the original's object pointer was null.
    pub objects: Vec<Option<Obj>>,
}

/// A write the harness made to the game's memory before a frame.
#[derive(Clone, Debug)]
pub struct Poke {
    /// Object index, -1 for a global address, or -2 for a stage-mode spawn slot's
    /// counter block (offset = slot).
    pub object: i32,
    pub offset: u32,
    pub data: Vec<u8>,
}

pub struct TraceFrame {
    /// The key array (`'d'`/`'u'` per virtual key) during this frame.
    pub keys: [u8; 256],
    /// Writes the harness made just before this frame (already included in `state`;
    /// apply them to a simulation after running the previous frame).
    pub pokes: Vec<Poke>,
    /// State at the start of this frame.
    pub state: Snapshot,
}

pub struct Trace {
    pub frames: Vec<TraceFrame>,
}

struct Rd<'a> {
    b: &'a [u8],
    p: usize,
}

impl<'a> Rd<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        if self.p + n > self.b.len() {
            return Err("trace is truncated".into());
        }
        let s = &self.b[self.p..self.p + n];
        self.p += n;
        Ok(s)
    }
    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }
}

fn read_snapshot(r: &mut Rd) -> Result<Snapshot, String> {
    if r.take(4)? != b"LF2S" || r.u32()? != 1 {
        return Err("not a version 1 snapshot".into());
    }
    let base = r.u32()?;
    let len = r.u32()? as usize;
    if base != GLOBALS_BASE || len != GLOBALS_LEN {
        return Err(format!("unexpected globals range {:#x}+{:#x}", base, len));
    }
    let mut globals = Globals::new();
    globals.m.bytes.copy_from_slice(r.take(len)?);
    let n = r.u32()? as usize;
    let mut objects = Vec::with_capacity(n);
    for _ in 0..n {
        if r.u8()? != 0 {
            let mut o = Obj::new();
            o.m.bytes.copy_from_slice(r.take(OBJ_SIZE)?);
            objects.push(Some(o));
        } else {
            objects.push(None);
        }
    }
    Ok(Snapshot { globals, objects })
}

/// File offset and length of lf2.exe's initialised .data, loaded at 0x44d000. Version 2
/// traces store the first frame's globals XORed with these bytes.
pub const EXE_DATA_OFFSET: usize = 0x4d000;
pub const EXE_DATA_LEN: usize = 0x2000;

impl Trace {
    /// Load a trace. `exe` is the contents of lf2.exe, needed for version 2 traces
    /// (`tools/harness/pack_trace.py` describes the format).
    pub fn load(path: &std::path::Path, exe: &[u8]) -> Result<Trace, String> {
        let raw = std::fs::read(path).map_err(|e| format!("{}: {}", path.display(), e))?;
        let mut body = Vec::new();
        flate::inflate_zlib(&raw, &mut body)?;
        let mut r = Rd { b: &body, p: 0 };
        if r.take(4)? != b"LF2T" {
            return Err("not a trace".into());
        }
        match r.u32()? {
            1 => Self::load_v1(r),
            2 => Self::load_v2(r, exe, false),
            3 => Self::load_v2(r, exe, true),
            v => Err(format!("unknown trace version {}", v)),
        }
    }

    fn read_pokes(r: &mut Rd) -> Result<Vec<Poke>, String> {
        let n = r.u32()? as usize;
        let mut v = Vec::with_capacity(n);
        for _ in 0..n {
            let object = r.u32()? as i32;
            let offset = r.u32()?;
            let len = r.u32()? as usize;
            v.push(Poke { object, offset, data: r.take(len)?.to_vec() });
        }
        Ok(v)
    }

    fn load_v2(mut r: Rd, exe: &[u8], has_pokes: bool) -> Result<Trace, String> {
        let n = r.u32()? as usize;
        let base = r.u32()?;
        let len = r.u32()? as usize;
        if base != GLOBALS_BASE || len != GLOBALS_LEN {
            return Err("unexpected globals range".into());
        }
        let bitmap = r.take(50)?.to_vec();
        let valid: Vec<bool> = (0..OBJECTS).map(|i| bitmap[i / 8] & (1 << (i % 8)) != 0).collect();
        let ed = exe.get(EXE_DATA_OFFSET..EXE_DATA_OFFSET + EXE_DATA_LEN).ok_or("lf2.exe is too short")?;
        let mut keys = [0u8; 256];
        keys.copy_from_slice(r.take(256)?);
        let pokes0 = if has_pokes { Self::read_pokes(&mut r)? } else { Vec::new() };
        let mut space: Vec<u8> = r.take(len)?.to_vec();
        for (i, b) in space.iter_mut().enumerate().take(ed.len()) {
            *b ^= ed[i];
        }
        let nobj = valid.iter().filter(|&&v| v).count();
        space.extend_from_slice(r.take(nobj * OBJ_SIZE)?);
        let to_snapshot = |space: &[u8]| {
            let mut globals = Globals::new();
            globals.m.bytes.copy_from_slice(&space[..len]);
            let mut p = len;
            let objects = valid
                .iter()
                .map(|&v| {
                    if v {
                        let mut o = Obj::new();
                        o.m.bytes.copy_from_slice(&space[p..p + OBJ_SIZE]);
                        p += OBJ_SIZE;
                        Some(o)
                    } else {
                        None
                    }
                })
                .collect();
            Snapshot { globals, objects }
        };
        let mut frames = vec![TraceFrame { keys, pokes: pokes0, state: to_snapshot(&space) }];
        for _ in 1..n {
            let mut keys = [0u8; 256];
            keys.copy_from_slice(r.take(256)?);
            let pokes = if has_pokes { Self::read_pokes(&mut r)? } else { Vec::new() };
            let c = r.u32()? as usize;
            for _ in 0..c {
                let w = r.u32()? as usize * 4;
                let v = r.take(4)?;
                space.get_mut(w..w + 4).ok_or("change outside the state")?.copy_from_slice(v);
            }
            frames.push(TraceFrame { keys, pokes, state: to_snapshot(&space) });
        }
        Ok(Trace { frames })
    }

    fn load_v1(mut r: Rd) -> Result<Trace, String> {
        let n = r.u32()? as usize;
        let mut frames = Vec::with_capacity(n);
        for _ in 0..n {
            let kl = r.u32()? as usize;
            let mut keys = [b'u'; 256];
            keys[..kl.min(256)].copy_from_slice(&r.take(kl)?[..kl.min(256)]);
            let pokes = if r.b.get(r.p..r.p + 4) == Some(b"LF2S") { Vec::new() } else { Self::read_pokes(&mut r)? };
            frames.push(TraceFrame { keys, pokes, state: read_snapshot(&mut r)? });
        }
        Ok(Trace { frames })
    }

    /// State outside a trace snapshot (stage spawn counters) comes from its pokes.
    pub fn start_world(&self, gd: std::sync::Arc<lf2_data::GameData>) -> crate::World {
        let mut w = crate::World::from_snapshot(&self.frames[0].state, gd);
        w.apply_pokes(&self.frames[0].pokes);
        w
    }

    pub fn step_and_compare(&self, w: &mut crate::World, n: usize) -> Vec<crate::compare::Diff> {
        w.set_keys(&self.frames[n].keys);
        w.step();
        w.apply_pokes(&self.frames[n + 1].pokes);
        crate::compare::compare(w, &self.frames[n + 1].state)
    }
}

use lf2_data::flate;


impl crate::world::World {
    /// Apply the harness's writes for a frame.
    pub fn apply_pokes(&mut self, pokes: &[Poke]) {
        for p in pokes {
            if p.object == -2 {
                // stage mode: a spawn slot's counter block (see stage.rs)
                let v: Vec<i32> = p.data.chunks(4).map(|c| i32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
                let rt = &mut self.spawn_rt[p.offset as usize];
                rt.count = v[0];
                rt.total = v[1];
                rt.alive = v[2];
                rt.objs.copy_from_slice(&v[3..43]);
            } else if p.object < 0 {
                let at = (p.offset - GLOBALS_BASE) as usize;
                self.g.m.bytes[at..at + p.data.len()].copy_from_slice(&p.data);
            } else {
                let at = p.offset as usize;
                self.objs[p.object as usize].m.bytes[at..at + p.data.len()].copy_from_slice(&p.data);
            }
        }
    }
}
