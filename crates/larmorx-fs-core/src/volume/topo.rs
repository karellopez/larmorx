// SPDX-License-Identifier: Apache-2.0
//! Voxel topology (spec §59): `QuadEulerCharChange`, `MRIvolTopoErodeOne` and
//! `MRIvolTopoDilateOne`.

use super::Dst;
use super::VolOut;
use super::setops::ext;
use crate::mri::Mri;
use crate::process::{Process, Stop};
use crate::sse;

/// `NEAREST_NEIGHBOR_FACE`, the six-face neighbourhood of `MRIerodeNN`.
const NEAREST_NEIGHBOR_FACE: i32 = 1;

impl Process {
    /// Whether voxel `(x, y, z)` is set (value above 0.5, frame 0; a NaN is not): outside the
    /// volume on an axis of `tested` it is unset; the other axes are read through the voxel
    /// functions (§59).
    fn topo_set(&mut self, m: &Mri, at: [i32; 3], tested: [bool; 3]) -> Result<bool, Stop> {
        let e = [m.geom.width, m.geom.height, m.geom.depth];
        for k in 0..3 {
            if tested[k] && (at[k] < 0 || at[k] >= e[k]) {
                return Ok(false);
            }
        }
        let v = self.mri_get_vox_val(m, at[0], at[1], at[2], 0, 0)?;
        Ok(v > 0.5)
    }

    /// `QuadEulerCharChange(vol, mask, c, r, s)` (§59): the change of the Euler characteristic
    /// of the quad mesh of the set voxels if voxel `(c, r, s)` were added, with the debugging
    /// voxel's lines on standard output.
    pub fn quad_euler_char_change(
        &mut self,
        vol: &Mri,
        mask: Option<&Mri>,
        c: i32,
        r: i32,
        s: i32,
    ) -> Result<i32, Stop> {
        let debug = [c, r, s] == self.gvoxel;
        if debug {
            self.put_out(format!("  debug vox {c} {r} {s}\n").as_bytes());
        }
        let mut dec = 0i32;
        let p = [c, r, s];
        for dc in -1..=1i32 {
            for dr in -1..=1i32 {
                for ds in -1..=1i32 {
                    let o = [dc, dr, ds];
                    let at = [
                        c.wrapping_add(dc),
                        r.wrapping_add(dr),
                        s.wrapping_add(ds),
                    ];
                    if let Some(mk) = mask {
                        let mv = self.mri_get_vox_val(mk, at[0], at[1], at[2], 0, 0)?;
                        if mv < 0.5 {
                            continue;
                        }
                    }
                    let nnz = o.iter().filter(|&&d| d != 0).count();
                    if nnz == 0 {
                        continue;
                    }
                    // the face voxels along the offset's axes, each tested on its own axis
                    let face = |k: usize| {
                        let mut q = p;
                        q[k] = q[k].wrapping_add(o[k]);
                        let mut t = [false; 3];
                        t[k] = true;
                        (q, t)
                    };
                    let (kind, change) = if nnz == 1 {
                        let k = o.iter().position(|&d| d != 0).expect("one offset");
                        let (q, t) = face(k);
                        if self.topo_set(vol, q, t)? {
                            ("face", -1)
                        } else {
                            ("face", 1)
                        }
                    } else {
                        // the diagonal voxel, tested on all three axes, then the others
                        let mut set = 0;
                        let mut total = 0;
                        if self.topo_set(vol, at, [true; 3])? {
                            set += 1;
                        }
                        total += 1;
                        let axes: Vec<usize> = (0..3).filter(|&k| o[k] != 0).collect();
                        for &k in &axes {
                            let (q, t) = face(k);
                            if self.topo_set(vol, q, t)? {
                                set += 1;
                            }
                            total += 1;
                        }
                        if nnz == 3 {
                            // the three edge voxels (two of the offsets), each tested on its two axes
                            for skip in [2usize, 1, 0] {
                                let mut q = p;
                                let mut t = [false; 3];
                                for k in 0..3 {
                                    if k != skip {
                                        q[k] = q[k].wrapping_add(o[k]);
                                        t[k] = true;
                                    }
                                }
                                if self.topo_set(vol, q, t)? {
                                    set += 1;
                                }
                                total += 1;
                            }
                        }
                        if nnz == 2 {
                            if set == 0 {
                                ("edge", -1)
                            } else if set == total {
                                ("edge", 1)
                            } else {
                                continue;
                            }
                        } else if set == 0 {
                            ("vertex", 1)
                        } else if set == total {
                            ("vertex", -1)
                        } else {
                            continue;
                        }
                    };
                    dec = dec.wrapping_add(change);
                    if debug {
                        // `added` for a change that adds the element, `removed` otherwise:
                        // faces and vertices add with +1, edges with -1
                        let added = match kind {
                            "edge" => change < 0,
                            _ => change > 0,
                        };
                        let word = if added { "added" } else { "removed" };
                        self.put_out(
                            format!("{kind} {dc:2} {dr:2} {ds:2} {word} dEC={dec}\n").as_bytes(),
                        );
                    }
                }
            }
        }
        if debug {
            self.put_out(format!("  debug vox {c} {r} {s} {dec}\n").as_bytes());
        }
        Ok(dec)
    }

    /// `MRIvolTopoErodeOne(binvol, keepmask)` (§59): one layer of boundary voxels turned off in
    /// place where that keeps the Euler characteristic; returns how many.
    pub fn mri_vol_topo_erode_one(
        &mut self,
        binvol: &mut Mri,
        keep: Option<&Mri>,
    ) -> Result<i32, Stop> {
        let eroded = match self.mri_erode_nn(binvol, Dst::New, NEAREST_NEIGHBOR_FACE, 1)? {
            VolOut::New(m) => Some(m),
            _ => None,
        };
        let (w, h, d) = ext(binvol);
        let mut n = 0i32;
        for c in 1..w - 1 {
            for r in 1..h - 1 {
                for s in 1..d - 1 {
                    if let Some(k) = keep {
                        let kv = self.mri_get_vox_val(k, c, r, s, 0, 0)?;
                        if kv > 0.5 {
                            continue;
                        }
                    }
                    let v = self.mri_get_vox_val(binvol, c, r, s, 0, 0)?;
                    if self.int_fs12(sse::widen(v)) < 1 {
                        continue;
                    }
                    if let Some(e) = &eroded {
                        let ev = self.mri_get_vox_val(e, c, r, s, 0, 0)?;
                        if self.int_fs12(sse::widen(ev)) >= 1 {
                            continue;
                        }
                    }
                    if self.quad_euler_char_change(binvol, None, c, r, s)? != 0 {
                        continue;
                    }
                    self.mri_set_vox_val(binvol, c, r, s, 0, 0.0, 0)?;
                    n = n.wrapping_add(1);
                }
            }
        }
        Ok(n)
    }

    /// `MRIvolTopoDilateOne(binvol, onval)` (§59): every unset voxel that a six-neighbour
    /// dilation sets, set to `onval` where the Euler characteristic changes; returns how many.
    pub fn mri_vol_topo_dilate_one(&mut self, binvol: &mut Mri, onval: i32) -> Result<i32, Stop> {
        let dil = match self.mri_dilate6(binvol, Dst::New)? {
            VolOut::New(m) => Some(m),
            _ => None,
        };
        let (w, h, d) = ext(binvol);
        let mut n = 0i32;
        for c in 0..w {
            for r in 0..h {
                for s in 0..d {
                    let v = self.mri_get_vox_val(binvol, c, r, s, 0, 0)?;
                    if self.int_fs12(sse::widen(v)) > 0 {
                        continue;
                    }
                    if let Some(e) = &dil {
                        let dv = self.mri_get_vox_val(e, c, r, s, 0, 0)?;
                        if self.int_fs12(sse::widen(dv)) < 1 {
                            continue;
                        }
                    }
                    if self.quad_euler_char_change(binvol, None, c, r, s)? == 0 {
                        continue;
                    }
                    self.mri_set_vox_val(binvol, c, r, s, 0, onval as f32, 0)?;
                    n = n.wrapping_add(1);
                }
            }
        }
        Ok(n)
    }
}
