//! `bound_x_and_camera` (0x41b5d0): clamp objects to the background in x and z, remove
//! projectiles and items that leave it, and move the camera.

use crate::mem::ftol;
use crate::world::{World, MAX_OBJECTS, PLAYER_OF_SLOT};

/// Camera x (`DAT_00450bc4`) and its speed (`DAT_00450bc8`).
pub const CAMERA_X: u32 = 0x450bc4;
pub const CAMERA_V: u32 = 0x450bc8;

impl World {
    /// 0x41b5d0. `mode` is the game mode.
    pub fn bound_x_and_camera(&mut self, mode: i32) {
        let (width, zmin, zmax) = {
            let b = self.background();
            (b.width, b.zboundary.0, b.zboundary.1)
        };
        let w = width as f64;
        let bb4 = self.g.u_450bb4();
        let stage_milk = mode == 1 && self.g.m.i32(0x450b94) / 10 == 5;
        for i in 0..MAX_OBJECTS {
            if !self.exists(i) {
                continue;
            }
            let t = self.data_of(i).kind;
            let id = self.data_of(i).id;
            let o = &mut self.objs[i];
            // z: characters [zmin, zmax], everything else one more on each side
            let (lo, hi) = if t == 0 { (zmin as f64, zmax as f64) } else { (zmin as f64 - 1.0, zmax as f64 + 1.0) };
            if lo > o.z() {
                o.set_z(lo);
            }
            if hi < o.z() {
                o.set_z(hi);
            }
            o.set_iz(ftol(o.z()));
            let mut remove = false;
            if t == 3 {
                if -300.0 > o.x() {
                    remove = true;
                }
                if w + 300.0 < o.x() {
                    remove = true;
                }
            } else if t == 0 {
                if i < 20 {
                    let lo = if o.team() == 5 { -300.0 } else { 0.0 };
                    if lo > o.x() {
                        o.set_x(lo);
                    }
                    if w < o.x() {
                        o.set_x(w);
                    }
                } else {
                    if -100.0 > o.x() {
                        o.set_x(-100.0);
                    }
                    if w + 100.0 < o.x() {
                        o.set_x(w + 100.0);
                    }
                }
                if bb4 > 0 && (bb4 as f64) < o.x() && o.team() != 5 && o.u_08() == 0 {
                    o.set_x(bb4 as f64);
                }
            } else if (id == 0x7a || id == 0x7b) && (o.m.i32(0x344) > 0 || stage_milk) {
                if 10.0 > o.x() {
                    o.set_x(10.0);
                }
                if w - 10.0 < o.x() {
                    o.set_x(w - 10.0);
                }
            } else {
                if 0.0 > o.x() && o.iy() == 0 {
                    remove = true;
                }
                if w < o.x() && o.iy() == 0 {
                    remove = true;
                }
            }
            let o = &mut self.objs[i];
            o.set_ix(ftol(o.x()));
            if remove {
                self.set_exists(i, false);
            }
        }

        // camera target: the players' average, a little ahead of where they face
        let mut sum = 0i32;
        let mut n = 0i32;
        for s in 0..8usize {
            if self.exists(s) && self.g.m.i32(PLAYER_OF_SLOT + 4 * s as u32) > 0 && self.objs[s].hp() > 0 {
                let o = &self.objs[s];
                if self.state(s) == 0xe {
                    sum += o.ix();
                } else {
                    sum += 0x82 + o.ix() + o.facing() as i32 * -0x104;
                }
                n += 1;
            }
        }
        if n == 0 {
            for i in 0..MAX_OBJECTS {
                if self.exists(i) && self.data_of(i).kind == 0 && self.objs[i].hp() > 0 {
                    sum += self.objs[i].ix();
                    n += 1;
                }
            }
            if n == 0 {
                sum = 800;
                n = 1;
            }
        }
        let mut target = sum / n - 0x18d;
        if target < 0 {
            target = 0;
        }
        if width - 0x31a < target {
            target = width - 0x31a;
        }
        let cap = self.g.m.i32(0x450bb0);
        if cap != 0 && cap < target {
            target = cap;
        }
        let cx = self.g.m.i32(CAMERA_X);
        let mut v = ((target - cx) / 0xe + self.g.m.i32(CAMERA_V) * 6) / 7;
        if v == 0 {
            if cx < target {
                v = 1;
            } else if target < cx {
                v = -1;
            }
        }
        self.g.m.set_i32(CAMERA_V, v);
        let mut cx = cx + v;
        let replay_cam = self.g.m.i32(0x450b74) != 0 && self.g.replaying() != 0;
        if replay_cam {
            cx = self.g.m.i32(0x450b7c);
        }
        if cx < 0 {
            cx = 0;
        }
        if width - 0x31a < cx {
            cx = width - 0x31a;
        }
        self.g.m.set_i32(CAMERA_X, cx);
        if replay_cam {
            self.g.m.set_i32(0x450b7c, cx);
        }
        // 0x41a250 (called here) draws the background; it changes no state.
    }
}
