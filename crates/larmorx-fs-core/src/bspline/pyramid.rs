// SPDX-License-Identifier: Apache-2.0
//! The factor-of-two pyramids of the centred cubic spline (spec §6, after Brigger, Müller,
//! Illgner and Unser's "Centered pyramids"): reducing and expanding a line, halving and doubling
//! a volume's resolution; FS-962, FS-963.

use larmorx_core::defects::{FS_962, FS_963};

use super::Axis;
use crate::maths::matrix::real_point_f;
use crate::mri::{Mri, types};
use crate::process::{Process, Stop};
use crate::sse;
use crate::volume::{Dst, VolOut, Vols};

/// The reduce filter `g` (21 taps).
const G: [f64; 21] = [
    0.708792,
    0.328616,
    -0.165157,
    -0.114448,
    0.0944036,
    0.0543881,
    -0.05193,
    -0.0284868,
    0.0281854,
    0.0152877,
    -0.0152508,
    -0.00825077,
    0.00824629,
    0.00445865,
    -0.0044582,
    -0.00241009,
    0.00241022,
    0.00130278,
    -0.00130313,
    -0.000704109,
    0.000704784,
];

/// The expand filter `h` (16 taps).
const H: [f64; 16] = [
    1.13726,
    0.625601,
    -0.0870191,
    -0.159256,
    0.0233167,
    0.0426725,
    -0.00624769,
    -0.0114341,
    0.00167406,
    0.00306375,
    -0.000448564,
    -0.000820929,
    0.000120192,
    0.000219967,
    -0.0000322054,
    -0.00005894,
];

/// Reduces a line of `n_in` samples to `n_in / 2` (§6.1).
pub fn reduce_line(input: &[f64]) -> Vec<f64> {
    let n_in = input.len();
    let m = 2 * (n_in / 2);
    let k2 = (2 * m) as i64;
    let mi = m as i64;
    let mut y = vec![0f64; m];
    for (k, yk) in y.iter_mut().enumerate() {
        let k = k as i64;
        let mut s = sse::mul64(input[k as usize], G[0]);
        for (i, &g) in G.iter().enumerate().skip(1) {
            let i = i as i64;
            let mut i1 = k - i;
            let mut i2 = k + i;
            if i1 < 0 {
                i1 = (2 * mi - 1 - i1) % k2;
                if i1 >= mi {
                    i1 = k2 - i1 - 1;
                }
            }
            if i2 > mi - 1 {
                i2 %= k2;
                if i2 >= mi {
                    i2 = k2 - i2 - 1;
                }
            }
            let t = sse::add64(input[i1 as usize], input[i2 as usize]);
            s = sse::add64(s, sse::mul64(t, g));
        }
        *yk = s;
    }
    (0..m / 2)
        .map(|q| sse::mul64(sse::add64(y[2 * q + 1], y[2 * q]), 0.5))
        .collect()
}

/// Expands a line of `n` samples to `2n` (§6.2).
pub fn expand_line(input: &[f64]) -> Vec<f64> {
    let n = input.len() as i64;
    let k2 = 2 * n;
    let mut out = vec![0f64; 2 * input.len()];
    for i in 0..n {
        let j = (2 * i) as usize;
        let mut s = sse::mul64(input[i as usize], H[0]);
        for k in (2..=14).step_by(2) {
            let mut i1 = i - k / 2;
            let mut i2 = i + k / 2;
            if i1 < 0 {
                i1 = (2 * n - 1 - i1) % k2;
                if i1 >= n {
                    i1 = k2 - i1 - 1;
                }
            }
            if i2 >= n {
                i2 %= k2;
                if i2 >= n {
                    i2 = k2 - i2 - 1;
                }
            }
            let t = sse::add64(input[i1 as usize], input[i2 as usize]);
            s = sse::add64(s, sse::mul64(t, H[k as usize]));
        }
        out[j] = s;
        let mut s = 0.0f64;
        for k in (-15i64..=15).step_by(2) {
            let mut i1 = i + (k + 1) / 2;
            if i1 < 0 {
                i1 = (2 * n - 1 - i1) % k2;
                if i1 > n - 1 {
                    i1 = k2 - i1 - 1;
                }
            }
            if i1 >= n {
                i1 %= k2;
                if i1 >= n {
                    i1 = k2 - i1 - 1;
                }
            }
            s = sse::add64(
                s,
                sse::mul64(H[k.unsigned_abs() as usize], input[i1 as usize]),
            );
        }
        out[j + 1] = s;
    }
    let len = out.len();
    for j in (2..len).rev() {
        out[j] = sse::mul64(sse::add64(out[j], out[j - 1]), 0.5);
    }
    if len > 1 {
        out[1] = sse::mul64(sse::add64(out[0], out[1]), 0.5);
    }
    out[0] = sse::mul64(0.5, out[0]);
    out
}

impl Process {
    /// A line along `axis` of `m` from `at` (the axis's own index 0), `n` samples, through
    /// `MRIgetVoxVal`, widened.
    fn read_line(
        &mut self,
        m: &Mri,
        axis: Axis,
        at: [usize; 3],
        f: usize,
        n: usize,
    ) -> Result<Vec<f64>, Stop> {
        let mut out = Vec::with_capacity(n);
        for k in 0..n {
            let mut p = at;
            match axis {
                Axis::X => p[0] = k,
                Axis::Y => p[1] = k,
                Axis::Z => p[2] = k,
            }
            let v = self.mri_get_vox_val(m, p[0] as i32, p[1] as i32, p[2] as i32, f as i32, 0)?;
            out.push(sse::widen(v));
        }
        Ok(out)
    }

    /// FS-963 (both modes, FreeSurfer's geometry): an axis of one voxel takes a new voxel size.
    fn note_fs963(&mut self, src: &Mri, function: &str) {
        let g = &src.geom;
        if g.width == 1 || g.height == 1 || g.depth == 1 {
            self.note(
                &FS_963,
                format!("{function}: an axis of one voxel, not resampled, takes the new voxel size (FreeSurfer's geometry)"),
            );
        }
    }

    /// The destination of a pyramid (§6.3 step 3, §6.4): none gives a volume of the out extent,
    /// the source's type and frames, with the source's header; a destination of another extent
    /// or frame count ends the program.
    fn pyramid_dst(&mut self, v: &mut Vols, out: [i32; 3]) -> Result<(), Stop> {
        if !v.has_dst() {
            let (kind, nframes) = (v.a().kind, v.a().nframes);
            let mut d = self.mri_alloc_sequence(out[0], out[1], out[2], kind, nframes)?;
            self.mri_copy_header(v.a(), Some(&mut d))?;
            v.set_dst(d);
        }
        let (d, a) = (v.d(), v.a());
        let g = &d.geom;
        if g.width != out[0] || g.height != out[1] || g.depth != out[2] || d.nframes != a.nframes {
            return Err(
                self.bspline_end("ERROR MRIupsample2BSpline: MRI Dest dimensions not correct!")
            );
        }
        Ok(())
    }

    /// The centre of a pyramid's destination (§6.3 step 5): the source's voxel-to-RAS matrix
    /// times the point in float; the destination's cached matrices recomputed.
    fn pyramid_centre(&mut self, v: &mut Vols, q: [f32; 3]) -> Result<(), Stop> {
        let i2r = self.extract_i_to_r(&v.a().geom)?;
        let c = real_point_f(&i2r, [q[0], q[1], q[2], 1.0]);
        let d = v.d_mut();
        d.geom.c = [c[0], c[1], c[2]];
        let mut g = d.geom.clone();
        self.reinit_cache(&mut g)?;
        v.d_mut().geom = g;
        Ok(())
    }

    /// `MRIdownsample2BSpline(src, dst)` (§6.3): every axis of more than one voxel halved with
    /// the reduce filter (x, then y into `float` volumes, then z into the destination).
    pub fn mri_downsample2_bspline(&mut self, src: &mut Mri, dst: Dst) -> Result<VolOut, Stop> {
        let g = &src.geom;
        let (w, h, d, nf) = (g.width, g.height, g.depth, src.nframes);
        let out = [(w / 2).max(1), (h / 2).max(1), (d / 2).max(1)];
        let (wu, hu, du, nfu) = (w as usize, h as usize, d as usize, nf.max(0) as usize);
        let (wo, ho) = (out[0] as usize, out[1] as usize);
        let pass = |l: Vec<f64>| if l.len() > 1 { reduce_line(&l) } else { l };
        // pass x
        let mut t1 = self.mri_alloc_sequence(out[0], h, d, types::FLOAT, nf)?;
        for f in 0..nfu {
            for z in 0..du {
                for y in 0..hu {
                    let l = self.read_line(src, Axis::X, [0, y, z], f, wu)?;
                    self.store_line(&mut t1, Axis::X, [0, y, z], f, &pass(l))?;
                }
            }
        }
        // pass y
        let mut t2 = self.mri_alloc_sequence(out[0], out[1], d, types::FLOAT, nf)?;
        for f in 0..nfu {
            for z in 0..du {
                for x in 0..wo {
                    let l = self.read_line(&t1, Axis::Y, [x, 0, z], f, hu)?;
                    self.store_line(&mut t2, Axis::Y, [x, 0, z], f, &pass(l))?;
                }
            }
        }
        drop(t1);
        let mut v = Vols::new(src, None, dst);
        self.pyramid_dst(&mut v, out)?;
        // pass z
        for f in 0..nfu {
            for y in 0..ho {
                for x in 0..wo {
                    let l = self.read_line(&t2, Axis::Z, [x, y, 0], f, du)?;
                    self.store_line(v.d_mut(), Axis::Z, [x, y, 0], f, &pass(l))?;
                }
            }
        }
        // the header
        self.note_fs963(v.a(), "MRIdownsample2BSpline");
        let a = v.a();
        let (imnr0, sizes, thick, ps) = (
            a.imnr0,
            [a.geom.xsize, a.geom.ysize, a.geom.zsize],
            a.thick,
            a.ps,
        );
        let dm = v.d_mut();
        dm.imnr0 = imnr0;
        dm.imnr1 = imnr0 + out[2] - 1;
        dm.geom.xsize = sse::mul32(sizes[0], 2.0);
        dm.geom.ysize = sse::mul32(sizes[1], 2.0);
        dm.geom.zsize = sse::mul32(sizes[2], 2.0);
        dm.thick = sse::mul32(thick, 2.0);
        dm.ps = sse::mul32(ps, 2.0);
        let q = [
            (w / 2) as f32 + 0.5,
            (h / 2) as f32 + 0.5,
            (d / 2) as f32 + 0.5,
        ];
        self.pyramid_centre(&mut v, q)?;
        Ok(v.out())
    }

    /// `MRIupsample2BSpline(src, dst)` (§6.4): every axis of more than one voxel doubled with the
    /// expand filter, each pass into the destination itself. FS-962: on an axis of one voxel
    /// FreeSurfer copies the source's voxels over the earlier passes' lines (`reproduce`); the
    /// default keeps them.
    pub fn mri_upsample2_bspline(&mut self, src: &mut Mri, dst: Dst) -> Result<VolOut, Stop> {
        let g = &src.geom;
        let (w, h, d, nf) = (g.width, g.height, g.depth, src.nframes);
        let up = |e: i32| if e > 1 { 2 * e } else { 1 };
        let out = [up(w), up(h), up(d)];
        let (wu, hu, du, nfu) = (w as usize, h as usize, d as usize, nf.max(0) as usize);
        let (wo, ho) = (out[0] as usize, out[1] as usize);
        let pass = |l: Vec<f64>| if l.len() > 1 { expand_line(&l) } else { l };
        let mut v = Vols::new(src, None, dst);
        self.pyramid_dst(&mut v, out)?;
        // pass x
        for f in 0..nfu {
            for z in 0..du {
                for y in 0..hu {
                    let l = self.read_line(v.a(), Axis::X, [0, y, z], f, wu)?;
                    self.store_line(v.d_mut(), Axis::X, [0, y, z], f, &pass(l))?;
                }
            }
        }
        // pass y
        for f in 0..nfu {
            for z in 0..du {
                for x in 0..wo {
                    if h > 1 {
                        let l = self.read_line(v.d(), Axis::Y, [x, 0, z], f, hu)?;
                        self.store_line(v.d_mut(), Axis::Y, [x, 0, z], f, &pass(l))?;
                    } else {
                        self.fs962_copy(&mut v, [x, 0, z], f, w > 1)?;
                    }
                }
            }
        }
        // pass z
        for f in 0..nfu {
            for y in 0..ho {
                for x in 0..wo {
                    if d > 1 {
                        let l = self.read_line(v.d(), Axis::Z, [x, y, 0], f, du)?;
                        self.store_line(v.d_mut(), Axis::Z, [x, y, 0], f, &pass(l))?;
                    } else {
                        self.fs962_copy(&mut v, [x, y, 0], f, w > 1 || h > 1)?;
                    }
                }
            }
        }
        // the header
        self.note_fs963(v.a(), "MRIupsample2BSpline");
        let a = v.a();
        let (imnr0, sizes) = (a.imnr0, [a.geom.xsize, a.geom.ysize, a.geom.zsize]);
        let dm = v.d_mut();
        dm.imnr0 = imnr0;
        dm.imnr1 = imnr0 + out[2] - 1;
        dm.geom.xsize = sse::narrow(sse::widen(sizes[0]) / 2.0);
        dm.geom.ysize = sse::narrow(sse::widen(sizes[1]) / 2.0);
        dm.geom.zsize = sse::narrow(sse::widen(sizes[2]) / 2.0);
        let q = [
            sse::narrow(f64::from(w) / 2.0 - 0.25),
            sse::narrow(f64::from(h) / 2.0 - 0.25),
            sse::narrow(f64::from(d) / 2.0 - 0.25),
        ];
        self.pyramid_centre(&mut v, q)?;
        Ok(v.out())
    }

    /// FS-962's copy of one destination voxel from the source's voxel at the same place (a pass
    /// of §6.4 on an axis of one voxel). `expanded`: whether an earlier pass expanded an axis
    /// (else the copy writes what is already there).
    fn fs962_copy(
        &mut self,
        v: &mut Vols,
        at: [usize; 3],
        f: usize,
        expanded: bool,
    ) -> Result<(), Stop> {
        if !expanded {
            return Ok(());
        }
        if !self.mode().reproduce() {
            if !self.noted(&FS_962) {
                self.note(
                    &FS_962,
                    "MRIupsample2BSpline: an axis of one voxel keeps the expanded lines (FreeSurfer copies the source's voxels over them)",
                );
            }
            return Ok(());
        }
        if !self.noted(&FS_962) {
            self.note(
                &FS_962,
                "MRIupsample2BSpline: an axis of one voxel takes the source's voxels at the destination's places, as FreeSurfer's",
            );
        }
        let x =
            self.mri_get_vox_val(v.a(), at[0] as i32, at[1] as i32, at[2] as i32, f as i32, 0)?;
        let p = at;
        let val = sse::widen(x);
        let stored = if v.d().kind == types::UCHAR {
            if val > 255.0 {
                255.0
            } else if val < 0.0 {
                0.0
            } else {
                self.nint(val) as f32
            }
        } else {
            sse::narrow(val)
        };
        let t = v.d_mut();
        self.mri_set_vox_val(
            t,
            p[0] as i32,
            p[1] as i32,
            p[2] as i32,
            f as i32,
            stored,
            0,
        )?;
        Ok(())
    }
}
