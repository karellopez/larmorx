// SPDX-License-Identifier: LicenseRef-FSL
// Translated to Rust from FSL 6.0.7.17: fsl-newimage 2203.12 (newimage.{h,cc}, generalio.cc,
// newimageio.h). Copyright (C) 1999-2018 University of Oxford (Mark Jenkinson, Matthew Webster
// et al.). Rust translation Copyright 2026 Karel Lopez Vilaret; modified from the originals.
// Distributed under the FSL Licence (see LICENSE).
//! FSL's `volume<float>`: an image of up to seven dimensions with the properties newimage
//! carries (voxel sizes, sform and qform, orientation, interpolation and extrapolation
//! settings), read and saved with FSL's rules.
//!
//! - [`read_volume4d`] and [`read_volume`] read as FSL does (`readGeneralVolume`): the file
//!   found by FSL's name rules, values converted to float with `scl_slope`/`scl_inter`, voxel
//!   sizes made positive, and a neurological image swapped in memory to radiological order.
//! - [`Volume::save`] and [`Volume::save_dtype`] write as `save_volume` and `save_volume_dtype`
//!   do: the header FSL builds (`set_fsl_hdr`), swapped back to neurological order when the
//!   file was, in the format `FSLOUTPUTTYPE` names, converted to the output type with C's casts
//!   as x86-64 performs them.
//! - The properties used by registration: [`Volume::sampling_mat`], [`Volume::cog_scaled_mm`],
//!   [`Volume::backgroundval`], [`Volume::min`] and [`Volume::max`].
//!
//! Errors are [`FslError`]s with FSL's text: the lines printed before an exception, and the
//! exception's type and `what()` (an uncaught exception ends an FSL program).

use std::path::Path;

use crate::arma::{self, Mat};
use crate::cnum;
use crate::nifti::{self, NiftiExtension, NiftiHeader, dt};
use crate::settings::Settings;
use crate::sse;

/// `FSL_RADIOLOGICAL`, `FSL_NEUROLOGICAL`, `FSL_INCONSISTENT`, `FSL_ZERODET`.
pub const RADIOLOGICAL: i32 = -1;
pub const NEUROLOGICAL: i32 = 1;
pub const INCONSISTENT: i32 = 0;
pub const ZERODET: i32 = -101;

/// `extrapolation` (newimage.h).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Extrapolation {
    Zeropad,
    Constpad,
    Extraslice,
    Mirror,
    Periodic,
    BoundsAssert,
    BoundsException,
}

/// `interpolation` (newimage.h).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Interpolation {
    NearestNeighbour,
    Trilinear,
    Sinc,
    Spline,
}

/// An error that ends an FSL program: the lines it printed on standard error before throwing
/// (`imthrow`'s `Image Exception : #n :: msg`), and the exception: its C++ type name and
/// `what()` (std::terminate prints both).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FslError {
    pub printed: Vec<String>,
    pub type_name: &'static str,
    pub what: String,
}

impl FslError {
    /// An abort by the C library itself (no C++ exception): it prints `msg` and raises
    /// SIGABRT, as glibc's allocator does when it finds its heap corrupted.
    pub fn library_abort(msg: impl Into<String>) -> FslError {
        FslError {
            printed: vec![msg.into()],
            type_name: "",
            what: String::new(),
        }
    }

    /// `imthrow(msg, n)`: prints `Image Exception : #n :: msg`, throws an `std::runtime_error`.
    pub fn imthrow(msg: impl Into<String>, n: i32) -> FslError {
        let msg = msg.into();
        FslError {
            printed: vec![format!("Image Exception : #{n} :: {msg}")],
            type_name: "std::runtime_error",
            what: msg,
        }
    }

    /// A `NiftiIO::NiftiException` thrown without `imthrow`.
    pub fn nifti(e: nifti::NiftiError) -> FslError {
        FslError {
            printed: Vec::new(),
            type_name: "NiftiIO::NiftiException",
            what: e.0,
        }
    }

    /// What std::terminate adds on standard error.
    pub fn terminate_text(&self) -> String {
        if self.type_name.is_empty() {
            return String::new();
        }
        format!(
            "terminate called after throwing an instance of '{}'\n  what():  {}\n",
            self.type_name, self.what
        )
    }

    /// Everything the program writes on standard error from the throw on.
    pub fn stderr_text(&self) -> String {
        let mut s = String::new();
        for l in &self.printed {
            s.push_str(l);
            s.push('\n');
        }
        s.push_str(&self.terminate_text());
        s
    }
}

impl std::fmt::Display for FslError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.what)
    }
}

impl std::error::Error for FslError {}

/// FSL's `volume<float>` (a `volume4D` is the same type).
#[derive(Clone, Debug)]
pub struct Volume {
    /// The values, x fastest, then y, z, t and the higher dimensions.
    pub data: Vec<f32>,
    /// `ColumnsX, RowsY, SlicesZ, dim4, dim5, dim6, dim7`.
    pub dims: [i64; 7],
    /// `Xdim, Ydim, Zdim, p_TR, pxdim5, pxdim6, pxdim7`.
    pub pixdims: [f32; 7],
    pub sform: Mat,
    pub qform: Mat,
    pub sform_code: i32,
    pub qform_code: i32,
    pub radiological_file: bool,
    pub intent_code: i32,
    pub intent_params: [f32; 3],
    pub slice_ordering_code: i32,
    pub interp: Interpolation,
    pub extrap: Extrapolation,
    pub padvalue: f32,
    pub splineorder: i32,
    pub ep_valid: [bool; 3],
    pub display_max: f32,
    pub display_min: f32,
    pub aux_file: Vec<u8>,
    pub extensions: Vec<NiftiExtension>,
    /// The sinc kernel of `setinterpolationmethod(sinc)` (`interpkernel`), if defined.
    pub kernel: Option<std::sync::Arc<crate::kernel::Kernel>>,
}

impl Volume {
    /// `volume(x, y, z, t)` (values 0) with `setdefaultproperties()`.
    pub fn new(x: i64, y: i64, z: i64, t: i64) -> Volume {
        Volume::new7([x, y, z, t, 1, 1, 1])
    }

    pub fn new7(dims: [i64; 7]) -> Volume {
        let n: i64 = dims.iter().product();
        Volume {
            data: vec![0.0; n.max(0) as usize],
            dims,
            pixdims: [1.0; 7],
            sform: Mat::eye(4),
            qform: Mat::eye(4),
            sform_code: nifti::XFORM_UNKNOWN,
            qform_code: nifti::XFORM_UNKNOWN,
            radiological_file: true,
            intent_code: 0,
            intent_params: [0.0; 3],
            slice_ordering_code: 0,
            interp: Interpolation::Trilinear,
            extrap: Extrapolation::Zeropad,
            padvalue: 0.0,
            splineorder: 3,
            ep_valid: [false; 3],
            display_max: 0.0,
            display_min: 0.0,
            aux_file: Vec::new(),
            extensions: Vec::new(),
            kernel: None,
        }
    }

    #[inline]
    pub fn xsize(&self) -> i64 {
        self.dims[0]
    }
    #[inline]
    pub fn ysize(&self) -> i64 {
        self.dims[1]
    }
    #[inline]
    pub fn zsize(&self) -> i64 {
        self.dims[2]
    }
    #[inline]
    pub fn tsize(&self) -> i64 {
        self.dims[3]
    }
    #[inline]
    pub fn xdim(&self) -> f32 {
        self.pixdims[0]
    }
    #[inline]
    pub fn ydim(&self) -> f32 {
        self.pixdims[1]
    }
    #[inline]
    pub fn zdim(&self) -> f32 {
        self.pixdims[2]
    }

    /// `nvoxels()`: voxels of one 3D volume.
    pub fn nvoxels(&self) -> i64 {
        self.dims[0] * self.dims[1] * self.dims[2]
    }

    /// `totalElements()`.
    pub fn total_elements(&self) -> usize {
        self.data.len()
    }

    /// `dimensionality()`.
    pub fn dimensionality(&self) -> i32 {
        let mut d = 0;
        if self.dims[0] >= 1 {
            d = 1;
        }
        for (k, &n) in self.dims.iter().enumerate().skip(1) {
            if n > 1 {
                d = k as i32 + 1;
            }
        }
        d
    }

    /// `setxdim` .. `setzdim` (absolute values) and `settdim`.
    pub fn setdims(&mut self, x: f32, y: f32, z: f32) {
        self.pixdims[0] = x.abs();
        self.pixdims[1] = y.abs();
        self.pixdims[2] = z.abs();
    }

    /// Index of `(x, y, z)` in volume 0 (`basicptr`).
    #[inline]
    pub fn idx(&self, x: i64, y: i64, z: i64) -> usize {
        ((z * self.dims[1] + y) * self.dims[0] + x) as usize
    }

    /// `value(x, y, z)`: no bounds handling.
    #[inline]
    pub fn value(&self, x: i64, y: i64, z: i64) -> f32 {
        self.data[self.idx(x, y, z)]
    }

    /// `in_bounds(x, y, z)` for integers.
    #[inline]
    pub fn in_bounds(&self, x: i64, y: i64, z: i64) -> bool {
        x >= 0 && y >= 0 && z >= 0 && x < self.dims[0] && y < self.dims[1] && z < self.dims[2]
    }

    /// `operator()(x, y, z) const`: the value, or the extrapolated one outside the volume.
    #[inline]
    pub fn get(&self, x: i64, y: i64, z: i64) -> f32 {
        if self.in_bounds(x, y, z) {
            self.value(x, y, z)
        } else {
            self.extrapolate(x, y, z)
        }
    }

    /// `extrapolate(x, y, z)` for the methods mcflirt's paths use.
    pub fn extrapolate(&self, x: i64, y: i64, z: i64) -> f32 {
        match self.extrap {
            Extrapolation::Zeropad => 0.0,
            Extrapolation::Constpad => self.padvalue,
            Extrapolation::Periodic => {
                let nx = periodicclamp(x, 0, self.dims[0] - 1);
                let ny = periodicclamp(y, 0, self.dims[1] - 1);
                let nz = periodicclamp(z, 0, self.dims[2] - 1);
                self.value(nx, ny, nz)
            }
            Extrapolation::Mirror => {
                let nx = mirrorclamp(x, 0, self.dims[0] - 1);
                let ny = mirrorclamp(y, 0, self.dims[1] - 1);
                let nz = mirrorclamp(z, 0, self.dims[2] - 1);
                self.value(nx, ny, nz)
            }
            Extrapolation::Extraslice => {
                let mut nx = x;
                let mut ny = y;
                let mut nz = z;
                if nx == -1 {
                    nx = 0;
                } else if nx == self.dims[0] {
                    nx = self.dims[0] - 1;
                }
                if ny == -1 {
                    ny = 0;
                } else if ny == self.dims[1] {
                    ny = self.dims[1] - 1;
                }
                if nz == -1 {
                    nz = 0;
                } else if nz == self.dims[2] {
                    nz = self.dims[2] - 1;
                }
                if self.in_bounds(nx, ny, nz) {
                    self.value(nx, ny, nz)
                } else {
                    self.padvalue
                }
            }
            // boundsassert/boundsexception: not reached on mcflirt's paths (affine_transform
            // switches them to constpad first).
            Extrapolation::BoundsAssert | Extrapolation::BoundsException => self.padvalue,
        }
    }

    /// `copybasicproperties(source, *this)` (and `copyproperties`).
    pub fn copy_properties(&mut self, s: &Volume) {
        self.pixdims = s.pixdims;
        self.sform = s.sform.clone();
        self.qform = s.qform.clone();
        self.sform_code = s.sform_code;
        self.qform_code = s.qform_code;
        self.radiological_file = s.radiological_file;
        self.intent_code = s.intent_code;
        self.intent_params = s.intent_params;
        self.slice_ordering_code = s.slice_ordering_code;
        self.kernel = s.kernel.clone();
        self.interp = s.interp;
        self.extrap = s.extrap;
        self.padvalue = s.padvalue;
        self.splineorder = s.splineorder;
        self.ep_valid = s.ep_valid;
        self.display_max = s.display_max;
        self.display_min = s.display_min;
        self.aux_file = s.aux_file.clone();
        self.extensions = s.extensions.clone();
    }

    /// `timeseries[t]` copied into a 3D volume (`testvol = timeseries[t]`): the values of
    /// volume `t` with every property of the series. `Err` is FSL's "Invalid t index".
    pub fn sub_volume(&self, t: i64) -> Result<Volume, FslError> {
        if t < 0 || t >= self.dims[3] {
            return Err(FslError::imthrow("Invalid t index in [] operator", 61));
        }
        let n = self.nvoxels() as usize;
        let mut v = Volume::new(self.dims[0], self.dims[1], self.dims[2], 1);
        v.data
            .copy_from_slice(&self.data[t as usize * n..(t as usize + 1) * n]);
        v.copy_properties(self);
        Ok(v)
    }

    /// `timeseries[t] = vol` (`ShadowVolume::equals`: the data only). `Err` is FSL's
    /// "Attempted to copydata with non-matching sizes".
    pub fn set_sub_volume(&mut self, t: i64, vol: &Volume) -> Result<(), FslError> {
        let n = self.nvoxels() as usize;
        if vol.data.len() != n {
            return Err(FslError::imthrow(
                "Attempted to copydata with non-matching sizes",
                2,
            ));
        }
        self.data[t as usize * n..(t as usize + 1) * n].copy_from_slice(&vol.data);
        Ok(())
    }

    /// `sampling_mat()`: `diag(xdim, ydim, zdim, 1)`.
    pub fn sampling_mat(&self) -> Mat {
        let mut s = Mat::eye(4);
        s.set(0, 0, f64::from(self.xdim()));
        s.set(1, 1, f64::from(self.ydim()));
        s.set(2, 2, f64::from(self.zdim()));
        s
    }

    /// `min()` (`calculateExtrema`): from the first element, each strictly smaller element that
    /// is not also a new maximum; NaN elements never replace one.
    pub fn min(&self) -> f32 {
        self.extrema().1
    }

    /// `max()` (`calculateExtrema`).
    pub fn max(&self) -> f32 {
        self.extrema().0
    }

    /// `(max, min)` as `calculateExtrema` finds them.
    pub fn extrema(&self) -> (f32, f32) {
        let Some(&first) = self.data.first() else {
            return (0.0, 0.0);
        };
        let (mut mx, mut mn) = (first, first);
        for &v in &self.data[1..] {
            if v > mx {
                mx = v;
            } else if v < mn {
                mn = v;
            }
        }
        (mx, mn)
    }

    /// `cog("scaled_mm")` of volume 0 (`calc_cog`, then times the sampling matrix): the centre of
    /// the values minus the minimum, partial sums flushed every `max(1000, √nvoxels)` voxels,
    /// in voxel coordinates scaled by the voxel sizes. Prints FSL's warning when the total is 0.
    pub fn cog_scaled_mm(&self, warnings: &mut Vec<String>) -> [f64; 3] {
        let c = self.calc_cog(warnings);
        // v << cog << 1.0; v = sampling_mat() * v: a tiny 4x4 matrix-vector product.
        let v = arma::gemv_tinysq(&self.sampling_mat(), &[c[0], c[1], c[2], 1.0], false);
        [v[0], v[1], v[2]]
    }

    /// `calc_cog(vol, false)`: voxel coordinates.
    pub fn calc_cog(&self, warnings: &mut Vec<String>) -> [f64; 3] {
        let vmin = self.min();
        let mut v_cog = [0.0f64; 3];
        let (mut total, mut vx, mut vy, mut vz, mut tot) = (0.0f64, 0.0f64, 0.0f64, 0.0f64, 0.0f64);
        let mut n: i64 = 0;
        let mut nlim = (self.nvoxels() as f64).sqrt() as i64;
        if nlim < 1000 {
            nlim = 1000;
        }
        for z in 0..self.zsize() {
            for y in 0..self.ysize() {
                for x in 0..self.xsize() {
                    let val = f64::from(self.get(x, y, z) - vmin);
                    vx += val * x as f64;
                    vy += val * y as f64;
                    vz += val * z as f64;
                    tot += val;
                    n += 1;
                    if n > nlim {
                        n = 0;
                        total += tot;
                        v_cog[0] += vx;
                        v_cog[1] += vy;
                        v_cog[2] += vz;
                        tot = 0.0;
                        vx = 0.0;
                        vy = 0.0;
                        vz = 0.0;
                    }
                }
            }
        }
        total += tot;
        v_cog[0] += vx;
        v_cog[1] += vy;
        v_cog[2] += vz;
        if v_cog.iter().any(|c| c.is_nan()) || total.is_nan() {
            // NaN or infinite voxels: the same sums with SSE's NaNs.
            (v_cog, total) = self.cog_sums_sse(vmin, nlim);
        }
        if total.abs() < 1e-5 {
            warnings.push("WARNING::in calculating COG, total = 0.0".to_owned());
            total = 1.0;
        }
        for c in v_cog.iter_mut() {
            *c = sse::div_f64(*c, total);
        }
        v_cog
    }

    /// [`Volume::calc_cog`]'s sums (`v_cog`, `total`) with every operation's NaN as SSE gives
    /// it, in the source's order (`vx += val*x`: `vx` first).
    #[cold]
    fn cog_sums_sse(&self, vmin: f32, nlim: i64) -> ([f64; 3], f64) {
        use sse::{add_f64 as add, mul_f64 as mul};
        let mut v_cog = [0.0f64; 3];
        let (mut total, mut vx, mut vy, mut vz, mut tot) = (0.0f64, 0.0f64, 0.0f64, 0.0f64, 0.0f64);
        let mut n: i64 = 0;
        for z in 0..self.zsize() {
            for y in 0..self.ysize() {
                for x in 0..self.xsize() {
                    let val = f64::from(sse::sub_f32(self.get(x, y, z), vmin));
                    vx = add(vx, mul(val, x as f64));
                    vy = add(vy, mul(val, y as f64));
                    vz = add(vz, mul(val, z as f64));
                    tot = add(tot, val);
                    n += 1;
                    if n > nlim {
                        n = 0;
                        total = add(total, tot);
                        v_cog[0] = add(v_cog[0], vx);
                        v_cog[1] = add(v_cog[1], vy);
                        v_cog[2] = add(v_cog[2], vz);
                        (tot, vx, vy, vz) = (0.0, 0.0, 0.0, 0.0);
                    }
                }
            }
        }
        total = add(total, tot);
        v_cog[0] = add(v_cog[0], vx);
        v_cog[1] = add(v_cog[1], vy);
        v_cog[2] = add(v_cog[2], vz);
        (v_cog, total)
    }

    /// `backgroundval()` (`calc_bval(vol, 2)`): the 10th percentile of the values in a 2-voxel
    /// shell at the edges of volume 0 (an element of the sorted shell, not interpolated).
    pub fn backgroundval(&self) -> f32 {
        let (zb, yb, xb) = (self.zsize(), self.ysize(), self.xsize());
        let (mut ewx, mut ewy, mut ewz) = (2i64, 2i64, 2i64);
        if ewx >= xb {
            ewx = xb - 1;
        }
        if ewy >= yb {
            ewy = yb - 1;
        }
        if ewz >= zb {
            ewz = zb - 1;
        }
        let numbins = 2 * (xb - 2 * ewx) * (yb - 2 * ewy) * ewz
            + 2 * (xb - 2 * ewx) * zb * ewy
            + 2 * yb * zb * ewx;
        let mut hist: Vec<f32> = Vec::with_capacity(numbins.max(0) as usize);
        for e in 0..ewz {
            for x in ewx..xb - ewx {
                for y in ewy..yb - ewy {
                    hist.push(self.value(x, y, e));
                    hist.push(self.value(x, y, zb - 1 - e));
                }
            }
        }
        for e in 0..ewy {
            for x in ewx..xb - ewx {
                for z in 0..zb {
                    hist.push(self.value(x, e, z));
                    hist.push(self.value(x, yb - 1 - e, z));
                }
            }
        }
        for e in 0..ewx {
            for y in 0..yb {
                for z in 0..zb {
                    hist.push(self.value(e, y, z));
                    hist.push(self.value(xb - 1 - e, y, z));
                }
            }
        }
        sort_like_std(&mut hist);
        let p10 = (numbins / 10).max(0) as usize;
        hist.get(p10).copied().unwrap_or(0.0)
    }

    /// `left_right_order()`: from the determinants of the qform and sform (4 × 4, Armadillo's
    /// `det`), as floats.
    pub fn left_right_order(&self) -> i32 {
        let mut order;
        let mut dets: f32 = -1.0;
        let mut detq: f32 = -1.0;
        let mut det: f32 = -1.0;
        if self.qform_code != nifti::XFORM_UNKNOWN {
            detq = arma::det(&self.qform) as f32;
            det = detq;
        }
        if self.sform_code != nifti::XFORM_UNKNOWN {
            dets = arma::det(&self.sform) as f32;
            det = dets;
        }
        order = if det < 0.0 {
            RADIOLOGICAL
        } else {
            NEUROLOGICAL
        };
        if self.sform_code != nifti::XFORM_UNKNOWN && self.qform_code != nifti::XFORM_UNKNOWN {
            if dets * detq < 0.0 {
                order = INCONSISTENT;
            }
            if f64::from((dets * detq).abs()) < 1e-12 {
                order = ZERODET;
            }
        }
        if f64::from(det.abs()) < 1e-12 {
            order = ZERODET;
        }
        order
    }

    /// `swapmat(-1, 2, 3)`: x reversed about the field of view.
    pub fn swapmat_lr(&self) -> Mat {
        let mut m = Mat::zeros(4, 4);
        m.set(3, 3, 1.0);
        // setrow(affmat, 1, -1): row 1 is (-1, 0, 0, fov) with fov = (xsize-1)*xdim in float.
        m.set(0, 0, -1.0);
        m.set(0, 1, 0.0);
        m.set(0, 2, 0.0);
        let fov: f32 = (self.xsize() - 1) as f32 * self.xdim();
        m.set(0, 3, f64::from(fov));
        m.set(1, 1, 1.0);
        m.set(2, 2, 1.0);
        m
    }

    /// `swapLRorder()` (`basic_swapdimensions(-1, 2, 3, false)`): every 3D volume reversed along
    /// x, and the sform and qform made to describe the same positions:
    /// `S * inv(samp) * inv(swap) * samp'` (Armadillo's four-matrix product).
    pub fn swap_lr_order(&mut self) {
        let (nx, ny, nz) = (self.dims[0], self.dims[1], self.dims[2]);
        let nvol = (self.data.len() as i64 / (nx * ny * nz).max(1)) as usize;
        if self.data.len() > 1 {
            let nv = (nx * ny * nz) as usize;
            for t in 0..nvol {
                let block = &mut self.data[t * nv..(t + 1) * nv];
                let mut swapped = vec![0.0f32; nv];
                for z in 0..nz {
                    for y in 0..ny {
                        for x in 0..nx {
                            let src = ((z * ny + y) * nx + x) as usize;
                            let dst = ((z * ny + y) * nx + (nx - x - 1)) as usize;
                            swapped[dst] = block[src];
                        }
                    }
                }
                block.copy_from_slice(&swapped);
            }
        }
        let dx = -self.xdim();
        let dy = self.ydim();
        let dz = self.zdim();
        let mut swapped_samp = Mat::eye(4);
        swapped_samp.set(0, 0, f64::from(dx.abs()));
        swapped_samp.set(1, 1, f64::from(dy.abs()));
        swapped_samp.set(2, 2, f64::from(dz.abs()));
        let samp_inv = arma::inv(&self.sampling_mat()).expect("diagonal sampling matrix");
        let swap_inv = arma::inv(&self.swapmat_lr()).expect("swap matrix");
        let new_s = arma::mul4(&self.sform, &samp_inv, &swap_inv, &swapped_samp);
        let new_q = arma::mul4(&self.qform, &samp_inv, &swap_inv, &swapped_samp);
        self.setdims(dx, dy, dz);
        self.sform = new_s;
        self.qform = new_q;
    }

    /// `makeradiological()`.
    pub fn make_radiological(&mut self) {
        if self.left_right_order() == NEUROLOGICAL {
            self.swap_lr_order();
        }
    }

    /// `makeneurological()`.
    pub fn make_neurological(&mut self) {
        if self.left_right_order() == RADIOLOGICAL {
            self.swap_lr_order();
        }
    }
}

/// `std::sort` of floats as libstdc++ orders them with `operator<`: for values without NaN
/// any correct sort gives the same sequence (equal values are interchangeable). A shell with
/// NaN has no defined order in C++; NaN values are put last here.
fn sort_like_std(v: &mut [f32]) {
    v.sort_by(|a, b| {
        a.partial_cmp(b)
            .unwrap_or_else(|| a.is_nan().cmp(&b.is_nan()))
    });
}

/// `periodicclamp` (miscmaths.cc).
pub fn periodicclamp(x: i64, x1: i64, x2: i64) -> i64 {
    if x2 < x1 {
        return periodicclamp(x, x2, x1);
    }
    let d = x2 - x1 + 1;
    let xp = x - x1;
    if xp >= 0 {
        (xp % d) + x1
    } else {
        let xp = xp + d + (xp / d).abs() * d;
        periodicclamp(xp + d + (xp / d).abs() * d, x1, x2)
    }
}

/// `mirrorclamp` (newimage.cc).
pub fn mirrorclamp(x: i64, x1: i64, x2: i64) -> i64 {
    if x2 < x1 {
        return mirrorclamp(x, x2, x1);
    }
    if x1 == x2 {
        return x1;
    }
    let x3 = 2 * x2 - x1 + 1;
    let mut nx = periodicclamp(x, x1, x3);
    if nx > x2 {
        nx = 2 * x2 + 1 - nx;
    }
    nx
}

// ------------------------------------------------------------------------------------------------
// File names (generalio.cc)

/// The image extensions FSL knows: (header-bearing, data-bearing).
const EXTENSIONS: [(&str, &str); 4] = [
    (".nii.gz", ".nii.gz"),
    (".nii", ".nii"),
    (".hdr", ".img"),
    (".hdr.gz", ".img.gz"),
];

/// `make_basename(filename)`: one image extension removed, if the name ends with one.
pub fn make_basename(name: &str) -> String {
    for (first, second) in EXTENSIONS {
        if let Some(stem) = name.strip_suffix(first) {
            return stem.to_owned();
        }
        if let Some(stem) = name.strip_suffix(second) {
            return stem.to_owned();
        }
    }
    name.to_owned()
}

/// `valid_imagefilename`: an existing regular file.
fn is_regular_file(name: &str) -> bool {
    std::fs::metadata(name)
        .map(|m| m.is_file())
        .unwrap_or(false)
}

/// `return_validimagefilename(filename)`: the header-bearing file of an image, or FSL's error.
pub fn return_validimagefilename(filename: &str) -> Result<String, FslError> {
    let bname = make_basename(filename);
    let mut validname = String::new();
    if bname != filename {
        for (first, second) in EXTENSIONS {
            if format!("{bname}{first}") == filename || format!("{bname}{second}") == filename {
                validname = format!("{bname}{first}");
            }
        }
        if is_regular_file(&validname) {
            return Ok(validname);
        }
    }
    let mut count = 0;
    for (first, _) in EXTENSIONS {
        let cand = format!("{bname}{first}");
        if is_regular_file(&cand) {
            validname = cand;
            count += 1;
        }
    }
    if count > 1 {
        return Err(FslError::imthrow(
            format!("Multiple possible filenames detected for basename: {bname}"),
            61,
        ));
    }
    if count == 0 {
        return Err(FslError::imthrow(
            format!("No image files match: {bname}"),
            63,
        ));
    }
    Ok(validname)
}

/// `find_pathname(filename)`: everything up to and including the last `/` (unchanged when there
/// is none; the first character is never removed).
pub fn find_pathname(filename: &str) -> String {
    let b = filename.as_bytes();
    if b.is_empty() {
        return String::new();
    }
    let mut indx = b.len() - 1;
    while b[indx] != b'/' && indx != 0 {
        indx -= 1;
    }
    if indx < b.len() - 1 {
        filename[..=indx].to_owned()
    } else {
        filename.to_owned()
    }
}

// ------------------------------------------------------------------------------------------------
// Reading

/// `NiftiGetLeftRightOrder(header)`: from the float determinants of the header's 3x3 matrices.
fn header_left_right_order(h: &NiftiHeader) -> i32 {
    let mut order;
    let mut dets: f32 = -1.0;
    let mut detq: f32 = -1.0;
    let mut det: f32 = -1.0;
    if h.qform_code != nifti::XFORM_UNKNOWN {
        detq = nifti::mat33_determ(&nifti::mat44_to_mat33(&h.get_qform()));
        det = detq;
    }
    if h.sform_code != nifti::XFORM_UNKNOWN {
        dets = nifti::mat33_determ(&nifti::mat44_to_mat33(&h.get_sform()));
        det = dets;
    }
    order = if det < 0.0 {
        RADIOLOGICAL
    } else {
        NEUROLOGICAL
    };
    if h.sform_code != nifti::XFORM_UNKNOWN && h.qform_code != nifti::XFORM_UNKNOWN {
        if dets * detq < 0.0 {
            order = INCONSISTENT;
        }
        if f64::from((dets * detq).abs()) < 1e-12 {
            order = ZERODET;
        }
    }
    if f64::from(det.abs()) < 1e-12 {
        order = ZERODET;
    }
    order
}

fn mat44_to_mat(m: &nifti::Mat44) -> Mat {
    let mut out = Mat::zeros(4, 4);
    for (i, row) in m.iter().enumerate() {
        for (j, v) in row.iter().enumerate() {
            out.set(i, j, f64::from(*v));
        }
    }
    out
}

/// `newmat2mat44`: doubles stored as floats.
pub fn mat_to_mat44(m: &Mat) -> nifti::Mat44 {
    std::array::from_fn(|i| std::array::from_fn(|j| m.at(i, j) as f32))
}

/// Values of a raw buffer of type `S` converted as `convertbuffer(src, dst, n, slope, inter)`
/// with a `float` destination.
fn convert_buffer(raw: &[u8], datatype: i16, n: usize, slope: f32, inter: f32) -> Option<Vec<f32>> {
    let plain = slope == 1.0 && inter == 0.0;
    macro_rules! conv {
        ($t:ty, $w:expr, $to_f32:expr, $scaled:expr) => {{
            let mut out = Vec::with_capacity(n);
            for chunk in raw.chunks_exact($w).take(n) {
                let v = <$t>::from_le_bytes(chunk.try_into().expect("width"));
                out.push(if plain { $to_f32(v) } else { $scaled(v) });
            }
            Some(out)
        }};
    }
    match datatype {
        // (S) * float + float: integers of up to 32 bits become float first.
        dt::SIGNED_SHORT => conv!(i16, 2, |v: i16| f32::from(v), |v: i16| f32::from(v) * slope
            + inter),
        dt::UNSIGNED_CHAR => conv!(u8, 1, |v: u8| f32::from(v), |v: u8| f32::from(v) * slope
            + inter),
        dt::SIGNED_INT => conv!(i32, 4, |v: i32| v as f32, |v: i32| (v as f32) * slope
            + inter),
        dt::FLOAT => conv!(f32, 4, |v: f32| v, |v: f32| v * slope + inter),
        // double * float is double, + float is double, then (float).
        dt::DOUBLE => conv!(f64, 8, |v: f64| v as f32, |v: f64| (v * f64::from(slope)
            + f64::from(inter))
            as f32),
        dt::INT8 => conv!(i8, 1, |v: i8| f32::from(v), |v: i8| f32::from(v) * slope
            + inter),
        dt::UINT16 => conv!(u16, 2, |v: u16| f32::from(v), |v: u16| f32::from(v) * slope
            + inter),
        dt::UINT32 => conv!(u32, 4, |v: u32| v as f32, |v: u32| (v as f32) * slope
            + inter),
        dt::INT64 => conv!(i64, 8, |v: i64| v as f32, |v: i64| (v as f32) * slope
            + inter),
        dt::UINT64 => conv!(u64, 8, |v: u64| v as f32, |v: u64| (v as f32) * slope
            + inter),
        _ => None,
    }
}

/// `readGeneralVolume(target, filename, dtype, true)` for a whole image: the volume and the
/// file's datatype.
fn read_general(filename: &str, settings: &Settings) -> Result<(Volume, i16, i32), FslError> {
    let fail = |what: &str| {
        FslError::imthrow(
            format!("Failed to read volume {filename}\nError : {what}"),
            22,
        )
    };
    let valid = match return_validimagefilename(filename) {
        Ok(v) => v,
        Err(e) => {
            // The imthrow inside the try block printed its line; the catch prints FSL's #22.
            let mut out = fail(&e.what);
            out.printed.splice(0..0, e.printed);
            return Err(out);
        }
    };
    let loaded = nifti::load_image(Path::new(&valid)).map_err(|e| fail(&e.0))?;
    let mut header = loaded.header;
    let keep_ext = settings
        .get("FSL_LOAD_NIFTI_EXTENSIONS")
        .is_some_and(|v| crate::settings::atoi(v) != 0);
    let extensions = if keep_ext {
        loaded.extensions
    } else {
        Vec::new()
    };
    if header.is_analyze() {
        header.s_x[0] = header.pixdim[1];
        header.s_y[1] = header.pixdim[2];
        header.s_z[2] = header.pixdim[3];
        // -(origin()[i]-1)*pixdim: the short is promoted to int before the subtraction.
        let o = header.legacy.origin();
        header.s_x[3] = f64::from(-(i32::from(o[0]) - 1)) * header.pixdim[1];
        header.s_y[3] = f64::from(-(i32::from(o[1]) - 1)) * header.pixdim[2];
        header.s_z[3] = f64::from(-(i32::from(o[2]) - 1)) * header.pixdim[3];
        let s = header.get_sform();
        header.set_qform(&s);
        header.qform_code = nifti::XFORM_ALIGNED_ANAT;
        header.sform_code = nifti::XFORM_ALIGNED_ANAT;
    }
    for i in 1..=(header.dim[0].clamp(0, 7) as usize) {
        header.pixdim[i] = if header.pixdim[i] == 0.0 {
            1.0
        } else {
            header.pixdim[i].abs()
        };
    }
    // ConvertAndScaleNewNiftiBuffer
    let n = header.n_elements();
    let mut slope = header.scl_slope as f32;
    let mut inter = header.scl_inter as f32;
    if f64::from(slope.abs()) < 1e-30 {
        slope = 1.0;
        inter = 0.0;
    }
    let doscaling = (f64::from(slope) - 1.0).abs() > 1e-30 || f64::from(inter.abs()) > 1e-30;
    let data = if header.datatype == dt::FLOAT && !doscaling {
        convert_buffer(&loaded.data, dt::FLOAT, n, 1.0, 0.0)
    } else {
        convert_buffer(&loaded.data, header.datatype, n, slope, inter)
    };
    let Some(data) = data else {
        return Err(FslError::imthrow(
            format!("Fslread: DT {} not supported", header.datatype),
            8,
        ));
    };
    let dims = [
        header.dim[1],
        header.dim[2],
        header.dim[3],
        header.dim[4],
        header.dim[5],
        header.dim[6],
        header.dim[7],
    ];
    let mut v = Volume::new7(dims);
    v.data = data;
    v.extensions = extensions;
    // set_volume_properties
    for k in 0..7 {
        v.pixdims[k] = (header.pixdim[k + 1] as f32).abs();
    }
    // setd5dim .. setd7dim keep their sign.
    for k in 4..7 {
        v.pixdims[k] = header.pixdim[k + 1] as f32;
    }
    v.sform = mat44_to_mat(&header.get_sform());
    v.qform = mat44_to_mat(&header.get_qform());
    v.sform_code = header.sform_code;
    v.qform_code = header.qform_code;
    v.radiological_file = header_left_right_order(&header) == RADIOLOGICAL;
    v.intent_code = header.intent_code;
    v.intent_params = [
        header.intent_p1 as f32,
        header.intent_p2 as f32,
        header.intent_p3 as f32,
    ];
    v.display_min = header.cal_min as f32;
    v.display_max = header.cal_max as f32;
    let mut aux = header.aux_file.clone();
    aux.truncate(24);
    v.aux_file = aux;
    if !v.radiological_file {
        v.make_radiological();
    }
    Ok((v, header.datatype, header.units))
}

/// `read_volume4D(target, filename)`: the volume, the file's datatype and its `xyzt_units`.
pub fn read_volume4d(filename: &str, settings: &Settings) -> Result<(Volume, i16, i32), FslError> {
    read_general(filename, settings)
}

/// `read_volume(target, filename)` (the legacy reader): a 4D file is cut to its first volume
/// with FSL's warning, which is returned.
pub fn read_volume(
    filename: &str,
    settings: &Settings,
) -> Result<(Volume, Option<String>), FslError> {
    let (v, _, _) = read_general(filename, settings)?;
    if v.tsize() > 1 {
        let warning = "Warning: An input intended to be a single 3D volume has multiple \
                       timepoints. Input will be truncated to first volume, but this \
                       functionality is deprecated and will be removed in a future release."
            .to_owned();
        let first = v.sub_volume(0)?;
        return Ok((first, Some(warning)));
    }
    Ok((v, None))
}

/// `dtype(filename)`: the file's datatype, or `DT_FLOAT` when it is scaled (unless the slope is
/// 0 or the type is double).
pub fn file_dtype(filename: &str) -> Result<i16, FslError> {
    if filename.is_empty() {
        return Ok(-1);
    }
    let valid = return_validimagefilename(filename)?;
    let h = nifti::load_header(Path::new(&valid)).map_err(FslError::nifti)?;
    if h.scl_slope != 1.0 || h.scl_inter != 0.0 {
        if h.scl_slope == 0.0 || h.datatype == dt::DOUBLE {
            return Ok(h.datatype);
        }
        return Ok(dt::FLOAT);
    }
    Ok(h.datatype)
}

/// `closestTemplatedType(type)`; `Err` is the message FSL prints for unsupported types.
pub fn closest_templated_type(t: i16) -> Result<i16, String> {
    match t {
        dt::UNSIGNED_CHAR | dt::INT8 => Ok(dt::UNSIGNED_CHAR),
        dt::SIGNED_SHORT => Ok(dt::SIGNED_SHORT),
        dt::SIGNED_INT | dt::UINT16 => Ok(dt::SIGNED_INT),
        dt::FLOAT | dt::UINT32 | dt::INT64 | dt::UINT64 => Ok(dt::FLOAT),
        dt::DOUBLE | dt::FLOAT128 => Ok(dt::DOUBLE),
        dt::COMPLEX => Err("COMPLEX not supported as an independent type".to_owned()),
        _ => Err(format!(
            "Datatype {t} is NOT supported - please check your image"
        )),
    }
}

// ------------------------------------------------------------------------------------------------
// Saving

/// `FSL_TYPE_*`.
pub mod filetype {
    pub const ANALYZE: i32 = 10;
    pub const NIFTI: i32 = 1;
    pub const NIFTI2: i32 = 2;
    pub const NIFTI_PAIR: i32 = 11;
    pub const NIFTI2_PAIR: i32 = 12;
    pub const ANALYZE_GZ: i32 = 100;
    pub const NIFTI_GZ: i32 = 101;
    pub const NIFTI2_GZ: i32 = 102;
    pub const NIFTI_PAIR_GZ: i32 = 111;
    pub const NIFTI2_PAIR_GZ: i32 = 112;
}

/// Why `FslGetEnvOutputType` stopped the program: the lines it printed and its exit status
/// (`EXIT_FAILURE`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutputTypeExit {
    pub printed: Vec<String>,
}

/// `FslGetEnvOutputType()` from the settings table.
pub fn env_output_type(settings: &Settings) -> Result<i32, OutputTypeExit> {
    let Some(otype) = settings.get("FSLOUTPUTTYPE") else {
        return Err(OutputTypeExit {
            printed: vec![
                "ERROR:: Environment variable FSLOUTPUTTYPE is not set!".to_owned(),
                "Please make sure that the appropriate configuration file is sourced by your \
                 shell (e.g. by putting it in .profile)."
                    .to_owned(),
                "e.g. bash or sh users add the line \". ${FSLDIR}/etc/fslconf/fsl.sh\"".to_owned(),
                "e.g. tcsh or csh users add the line \"source ${FSLDIR}/etc/fslconf/fsl.csh\""
                    .to_owned(),
            ],
        });
    };
    Ok(match otype {
        "NIFTI" => filetype::NIFTI,
        "NIFTI2" => filetype::NIFTI2,
        "NIFTI_GZ" => filetype::NIFTI_GZ,
        "NIFTI2_GZ" => filetype::NIFTI2_GZ,
        "NIFTI_PAIR" => filetype::NIFTI_PAIR,
        "NIFTI2_PAIR" => filetype::NIFTI2_PAIR,
        "NIFTI_PAIR_GZ" => filetype::NIFTI_PAIR_GZ,
        "NIFTI2_PAIR_GZ" => filetype::NIFTI2_PAIR_GZ,
        other => {
            return Err(OutputTypeExit {
                printed: vec![
                    format!(
                        "ERROR:: Unrecognised value ({other}) of environment variable FSLOUTPUTTYPE"
                    ),
                    "Legal values are: NIFTI, NIFTI_PAIR, NIFTI_GZ, NIFTI_PAIR_GZ".to_owned(),
                ],
            });
        }
    })
}

/// `outputExtension(filetype)`.
pub fn output_extension(filetype: i32) -> &'static str {
    use filetype::*;
    match filetype {
        NIFTI | NIFTI2 => ".nii",
        NIFTI_GZ | NIFTI2_GZ => ".nii.gz",
        ANALYZE | NIFTI_PAIR | NIFTI2_PAIR => ".hdr",
        NIFTI_PAIR_GZ | NIFTI2_PAIR_GZ | ANALYZE_GZ => ".hdr.gz",
        _ => "",
    }
}

/// Why saving failed: `FSLOUTPUTTYPE` (exit 1) or an exception.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SaveError {
    OutputType(OutputTypeExit),
    Exception(FslError),
}

/// The output data type of `save_volume_dtype` (the volume class FSL converts to). `Int8` is
/// not one of FSL's classes: it is larmorx's correction of FSL-09 (int8 data kept as int8).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveType {
    Char,
    Int8,
    Short,
    Int,
    Float,
    Double,
}

/// How a float value becomes the output type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Conversion {
    /// C's cast as x86-64 performs it: truncation toward zero, out-of-range values wrapped
    /// (`(char)`, `(short)`) or the integer indefinite value (`(int)`).
    X86Cast,
    /// Rounded to the nearest (ties to even) and clamped to the type's range; NaN becomes 0
    /// (larmorx's correction, FSL-09).
    RoundClamp,
}

impl Volume {
    /// `save_volume(source, filename)`: as float.
    pub fn save(
        &self,
        filename: &str,
        settings: &Settings,
        description: &[u8],
    ) -> Result<(), SaveError> {
        self.save_typed(
            filename,
            SaveType::Float,
            Conversion::X86Cast,
            settings,
            description,
            None,
            nifti::UNITS_MM_SEC,
        )
    }

    /// `save_volume_dtype(source, filename, datatype)` with the conversion `conv` (FSL's is
    /// [`Conversion::X86Cast`]); `datatype_code` overrides the header's datatype and `units` is
    /// the header's `xyzt_units` (FSL always writes [`nifti::UNITS_MM_SEC`]).
    #[allow(clippy::too_many_arguments)]
    pub fn save_typed(
        &self,
        filename: &str,
        ty: SaveType,
        conv: Conversion,
        settings: &Settings,
        description: &[u8],
        datatype_code: Option<i16>,
        units: i32,
    ) -> Result<(), SaveError> {
        if self.tsize() < 1 {
            return Ok(());
        }
        let mut v = self.clone();
        let currently_rad = v.left_right_order() == RADIOLOGICAL;
        if !v.radiological_file && currently_rad {
            v.make_neurological();
        }
        let filetype = env_output_type(settings).map_err(SaveError::OutputType)?;
        // set_fsl_hdr
        let mut h = NiftiHeader::default();
        h.dim[0] = i64::from(v.dimensionality());
        for k in 0..7 {
            h.dim[k + 1] = v.dims[k];
        }
        let (code, bytes) = convert_for_save(&v.data, ty, conv);
        h.datatype = datatype_code.unwrap_or(code);
        for k in 0..7 {
            h.pixdim[k + 1] = f64::from(v.pixdims[k]);
        }
        h.sform_code = v.sform_code;
        h.qform_code = v.qform_code;
        h.set_sform(&mat_to_mat44(&v.sform));
        h.set_qform(&mat_to_mat44(&v.qform));
        h.intent_code = v.intent_code;
        h.intent_p1 = f64::from(v.intent_params[0]);
        h.intent_p2 = f64::from(v.intent_params[1]);
        h.intent_p3 = f64::from(v.intent_params[2]);
        h.scl_slope = 1.0;
        h.scl_inter = 0.0;
        h.cal_min = f64::from(v.display_min);
        h.cal_max = f64::from(v.display_max);
        h.aux_file = v.aux_file.clone();
        h.units = units;
        h.slice_ordering = 0;
        // save_unswapped_vol
        h.description = description.to_vec();
        let version = filetype % 100 % 10 % 3;
        let single = filetype % 100 < 10;
        h.set_nifti_version(version, single);
        h.bits_per_voxel = match ty {
            SaveType::Char | SaveType::Int8 => 8,
            SaveType::Short => 16,
            SaveType::Int | SaveType::Float => 32,
            SaveType::Double => 64,
        };
        if h.is_analyze() {
            let m = if h.sform_code != nifti::XFORM_UNKNOWN {
                Some(nifti::mat44_inverse(&h.get_sform()))
            } else if h.qform_code != nifti::XFORM_UNKNOWN {
                Some(nifti::mat44_inverse(&h.get_qform()))
            } else {
                None
            };
            if let Some(inv) = m {
                for i in 0..3 {
                    let o = cnum::f32_to_i16(inv[i][3]).wrapping_add(1);
                    h.legacy.set_origin(i, o);
                }
            }
            h.pixdim[1] *= -1.0;
        }
        let name = format!("{}{}", make_basename(filename), output_extension(filetype));
        nifti::save_image(&name, &bytes, &v.extensions, &h, filetype >= 100)
            .map_err(|e| SaveError::Exception(FslError::nifti(e)))
    }
}

/// The values converted for saving, and the NIfTI code of the volume class.
fn convert_for_save(data: &[f32], ty: SaveType, conv: Conversion) -> (i16, Vec<u8>) {
    let round = |v: f32, lo: f32, hi: f32| -> f64 {
        if v.is_nan() {
            0.0
        } else {
            f64::from(v.round_ties_even().clamp(lo, hi))
        }
    };
    match ty {
        SaveType::Char => {
            let b = data
                .iter()
                .map(|&v| match conv {
                    Conversion::X86Cast => cnum::f32_to_i8(v) as u8,
                    Conversion::RoundClamp => round(v, 0.0, 255.0) as u8,
                })
                .collect();
            (dt::UNSIGNED_CHAR, b)
        }
        SaveType::Int8 => {
            let b = data
                .iter()
                .map(|&v| match conv {
                    Conversion::X86Cast => cnum::f32_to_i8(v) as u8,
                    Conversion::RoundClamp => round(v, -128.0, 127.0) as i8 as u8,
                })
                .collect();
            (dt::INT8, b)
        }
        SaveType::Short => {
            let mut b = Vec::with_capacity(data.len() * 2);
            for &v in data {
                let s = match conv {
                    Conversion::X86Cast => cnum::f32_to_i16(v),
                    Conversion::RoundClamp => round(v, -32768.0, 32767.0) as i16,
                };
                b.extend_from_slice(&s.to_le_bytes());
            }
            (dt::SIGNED_SHORT, b)
        }
        SaveType::Int => {
            let mut b = Vec::with_capacity(data.len() * 4);
            for &v in data {
                let s = match conv {
                    Conversion::X86Cast => cnum::f32_to_i32(v),
                    Conversion::RoundClamp => {
                        if v.is_nan() {
                            0
                        } else {
                            // Clamp in f64: f32 cannot hold i32::MAX.
                            f64::from(v.round_ties_even()).clamp(-2147483648.0, 2147483647.0) as i32
                        }
                    }
                };
                b.extend_from_slice(&s.to_le_bytes());
            }
            (dt::SIGNED_INT, b)
        }
        SaveType::Float => {
            let mut b = Vec::with_capacity(data.len() * 4);
            for &v in data {
                b.extend_from_slice(&v.to_le_bytes());
            }
            (dt::FLOAT, b)
        }
        SaveType::Double => {
            let mut b = Vec::with_capacity(data.len() * 8);
            for &v in data {
                b.extend_from_slice(&f64::from(v).to_le_bytes());
            }
            (dt::DOUBLE, b)
        }
    }
}

/// The volume class `save_volume_dtype` converts to for a file datatype, or the message FSL
/// prints (`perror`) when there is none.
pub fn save_type_for(datatype: i16) -> Result<SaveType, String> {
    let t = closest_templated_type(datatype)?;
    Ok(match t {
        dt::UNSIGNED_CHAR => SaveType::Char,
        dt::SIGNED_SHORT => SaveType::Short,
        dt::SIGNED_INT => SaveType::Int,
        dt::FLOAT => SaveType::Float,
        _ => SaveType::Double,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basenames_and_paths() {
        assert_eq!(make_basename("a/b.nii.gz"), "a/b");
        assert_eq!(make_basename("a/b.img.gz"), "a/b");
        assert_eq!(make_basename("x.nii.gz_meanvol"), "x.nii.gz_meanvol");
        assert_eq!(make_basename("plain"), "plain");
        assert_eq!(find_pathname("/a/b/c.nii"), "/a/b/");
        // No '/': everything after the first character is erased (FSL's loop stops at 0).
        assert_eq!(find_pathname("c.nii"), "c");
        assert_eq!(find_pathname(""), "");
    }

    #[test]
    fn extrema_and_background() {
        let mut v = Volume::new(5, 5, 5, 1);
        for (i, d) in v.data.iter_mut().enumerate() {
            *d = (i % 7) as f32;
        }
        assert_eq!(v.extrema(), (6.0, 0.0));
        let b = v.backgroundval();
        assert!((0.0..=6.0).contains(&b));
        v.data[0] = f32::NAN;
        assert!(v.max().is_nan() && v.min().is_nan());
    }

    #[test]
    fn swapping_twice_restores_the_data() {
        let mut v = Volume::new(4, 3, 2, 2);
        for (i, d) in v.data.iter_mut().enumerate() {
            *d = i as f32;
        }
        v.sform_code = 1;
        v.sform = Mat::from4([
            [2.0, 0.0, 0.0, -3.0],
            [0.0, 2.0, 0.0, 1.0],
            [0.0, 0.0, 3.0, 2.0],
            [0.0, 0.0, 0.0, 1.0],
        ]);
        let orig = v.clone();
        v.swap_lr_order();
        assert_eq!(v.data[0], orig.data[3]);
        v.swap_lr_order();
        assert_eq!(v.data, orig.data);
    }

    /// A series saved in each of FSL's output types and read back: the same values, sizes,
    /// voxel sizes and sform; the type each `FSLOUTPUTTYPE` gives; `.hdr`/`.img` pairs.
    #[test]
    fn save_and_read_every_output_type() {
        let dir = tempfile::tempdir().unwrap();
        let mut v = Volume::new(5, 4, 3, 2);
        for (i, d) in v.data.iter_mut().enumerate() {
            *d = (i as f32) * 0.5 - 7.0;
        }
        v.pixdims[0] = 2.0;
        v.pixdims[1] = 2.5;
        v.pixdims[2] = 3.0;
        v.pixdims[3] = 1.5;
        v.sform_code = 1;
        v.sform = Mat::from4([
            [-2.0, 0.0, 0.0, 4.0],
            [0.0, 2.5, 0.0, -3.0],
            [0.0, 0.0, 3.0, -2.0],
            [0.0, 0.0, 0.0, 1.0],
        ]);
        let cases = [
            ("NIFTI_GZ", ".nii.gz"),
            ("NIFTI", ".nii"),
            ("NIFTI2_GZ", ".nii.gz"),
            ("NIFTI_PAIR", ".hdr"),
            ("NIFTI_PAIR_GZ", ".hdr.gz"),
        ];
        for (ty, ext) in cases {
            let settings = Settings::default().with("FSLOUTPUTTYPE", ty);
            let base = dir.path().join(format!("v-{ty}"));
            let base = base.to_str().unwrap();
            v.save(base, &settings, b"test").unwrap();
            let path = format!("{base}{ext}");
            assert!(Path::new(&path).is_file(), "{ty}: {path}");
            let (r, dtype, _units) = read_volume4d(&path, &settings).unwrap();
            assert_eq!(dtype, dt::FLOAT, "{ty}");
            assert_eq!(r.dims[..4], v.dims[..4], "{ty}");
            assert_eq!(r.pixdims[..4], v.pixdims[..4], "{ty}");
            assert_eq!(r.data, v.data, "{ty}");
            assert_eq!(r.sform_code, 1, "{ty}");
            for i in 0..4 {
                for j in 0..4 {
                    assert_eq!(r.sform.at(i, j), v.sform.at(i, j), "{ty} sform");
                }
            }
        }
    }
}
