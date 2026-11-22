// SPDX-License-Identifier: Apache-2.0
//! Random and constant volumes, frames, orders, fractions and difference lists (spec §38, part 8):
//! `MRIrandn`, `MRIconst`, the frame normalisations, `MRIsort`, `MRIreverseSliceOrder`,
//! `MRIorderIndices`, `MRIcomputeVoxelPermutation`, `MRInonMaxSuppress`, `MRIextractRegionAndPad`,
//! `MRIrms`, `MRIcomputeVolumeFractions`, `MRIcombineDistanceTransforms`, `MRIdiff2Vect`,
//! `MRIapplyDiffVect`, `MRIestimateTIV` and `MRIdimMismatch`; FS-472 to FS-479.

use larmorx_core::defects::{FS_472, FS_473, FS_474, FS_475, FS_476, FS_477, FS_478, FS_479};
use larmorx_core::glibc_qsort::qsort;

use super::ltaread::TransformInputs;
use super::sample::{FLT_EPSILON, cvtt};
use super::{Dst, VolOut, Vols};
use crate::maths::Returned;
use crate::maths::matrix::{MATRIX_REAL, Matrix, real_point_f};
use crate::mri::{Mri, Region, types};
use crate::process::{Process, Stop, codes};
use crate::sse;

/// `CompareDoubles` (§38): -1 when `a < b`, +1 when `a > b`, else 0 (a NaN equal to all).
fn compare_doubles(a: &f64, b: &f64) -> i32 {
    if a < b {
        -1
    } else if a > b {
        1
    } else {
        0
    }
}

/// The four dimensions of two volumes equal.
fn same_dims(a: &Mri, b: &Mri) -> bool {
    a.geom.width == b.geom.width
        && a.geom.height == b.geom.height
        && a.geom.depth == b.geom.depth
        && a.nframes == b.nframes
}

/// The three extents of two volumes equal.
fn same_extent(a: &Mri, b: &Mri) -> bool {
    a.geom.width == b.geom.width && a.geom.height == b.geom.height && a.geom.depth == b.geom.depth
}

impl Process {
    /// A new volume of the extent, type and frames given, with `src`'s header
    /// (`MRIallocSequence`, `MRIcopyHeader`).
    pub(crate) fn alloc_with_header(
        &mut self,
        src: &Mri,
        (w, h, d): (i32, i32, i32),
        kind: i32,
        nframes: i32,
    ) -> Result<Mri, Stop> {
        let mut m = self.mri_alloc_sequence(w, h, d, kind, nframes)?;
        self.mri_copy_header(src, Some(&mut m))?;
        Ok(m)
    }

    /// FS-472: an integer stored as a byte, short or unsigned short: its low bits with
    /// `reproduce`, clamped to the type's range by default (noted where they differ); other
    /// types' elements as C stores them.
    pub(crate) fn store_int(&mut self, kind: i32, v: i32, function: &str) -> Vec<u8> {
        let range = match kind {
            types::UCHAR => Some((0, 255)),
            types::SHORT => Some((-32768, 32767)),
            types::USHRT => Some((0, 65535)),
            _ => None,
        };
        let mut v = i64::from(v);
        if let Some((lo, hi)) = range
            && (v < lo || v > hi)
        {
            if self.mode().reproduce() {
                self.note(
                    &FS_472,
                    format!("{function}: a value past the type's range stored as its low bits"),
                );
            } else {
                self.note(
                    &FS_472,
                    format!(
                        "{function}: a value past the type's range clamped (FreeSurfer keeps its low bits)"
                    ),
                );
                v = v.clamp(lo, hi);
            }
        }
        match kind {
            types::UCHAR => vec![v as u8],
            types::SHORT => (v as i16).to_le_bytes().to_vec(),
            types::USHRT => (v as u16).to_le_bytes().to_vec(),
            types::INT => (v as i32).to_le_bytes().to_vec(),
            _ => v.to_le_bytes().to_vec(),
        }
    }

    /// The given volume of `MRIrandn` and `MRIconst` checked: its four dimensions (then, for
    /// `MRIrandn`, its type), the messages on standard output.
    fn check_given(&mut self, m: &Mri, dims: [i32; 4], name: &str, float: bool) -> bool {
        let g = &m.geom;
        if [g.width, g.height, g.depth, m.nframes] != dims {
            self.put_out(format!("ERROR: {name}: dimension mismatch\n").as_bytes());
            return false;
        }
        if float && m.kind != types::FLOAT {
            self.put_out(format!("ERROR: {name}: structure passed is not MRI_FLOAT\n").as_bytes());
            return false;
        }
        true
    }

    /// `MRIrandn(ncols, nrows, nslices, nframes, avg, stddev, mri)` (§38): each voxel `stddev g +
    /// avg` (`double`, `g` a new `PDFgaussian` draw; frames, slices, rows, columns) stored as a
    /// `float`, into a new `float` volume or the given one (its dimensions, then its type, checked).
    #[allow(clippy::too_many_arguments)]
    pub fn mri_randn(
        &mut self,
        ncols: i32,
        nrows: i32,
        nslices: i32,
        nframes: i32,
        avg: f32,
        stddev: f32,
        mri: Option<&mut Mri>,
    ) -> Result<VolOut, Stop> {
        let mut own = None;
        let given = mri.is_some();
        let m: &mut Mri = match mri {
            Some(m) => {
                if !self.check_given(m, [ncols, nrows, nslices, nframes], "MRIrandn", true) {
                    return Ok(VolOut::Nothing);
                }
                m
            }
            None => {
                own.insert(self.mri_alloc_sequence(ncols, nrows, nslices, types::FLOAT, nframes)?)
            }
        };
        let n = (ncols.max(0) as usize)
            * (nrows.max(0) as usize)
            * (nslices.max(0) as usize)
            * (nframes.max(0) as usize);
        let (s, a) = (sse::widen(stddev), sse::widen(avg));
        if n > 0 && m.voxels.is_none() {
            self.no_voxels("MRIrandn")?;
        }
        for i in 0..n {
            let g = self.pdf_gaussian();
            let v = sse::narrow(sse::add64(sse::mul64(s, g), a));
            if let Some(b) = m.voxels.as_mut() {
                b[4 * i..4 * i + 4].copy_from_slice(&v.to_le_bytes());
            }
        }
        Ok(match own {
            Some(m) => VolOut::New(m),
            None if given => VolOut::Given,
            None => VolOut::Nothing,
        })
    }

    /// `MRIconst(ncols, nrows, nslices, nframes, val, mri)` (§38): each voxel `nint(val)` as a
    /// byte, short, unsigned short, `int` or `long` (FS-472 for the first three), `val` for
    /// `float`, nothing for the other types; into a new `float` volume or the given one (any type,
    /// its dimensions checked).
    #[allow(clippy::too_many_arguments)]
    pub fn mri_const(
        &mut self,
        ncols: i32,
        nrows: i32,
        nslices: i32,
        nframes: i32,
        val: f32,
        mri: Option<&mut Mri>,
    ) -> Result<VolOut, Stop> {
        let mut own = None;
        let given = mri.is_some();
        let m: &mut Mri = match mri {
            Some(m) => {
                if !self.check_given(m, [ncols, nrows, nslices, nframes], "MRIconst", false) {
                    return Ok(VolOut::Nothing);
                }
                m
            }
            None => {
                own.insert(self.mri_alloc_sequence(ncols, nrows, nslices, types::FLOAT, nframes)?)
            }
        };
        let bytes = match m.kind {
            types::FLOAT => Some(val.to_le_bytes().to_vec()),
            types::UCHAR | types::SHORT | types::USHRT | types::INT | types::LONG => {
                let k = self.nint(sse::widen(val));
                Some(self.store_int(m.kind, k, "MRIconst"))
            }
            _ => None,
        };
        let n = m.vox_total as usize;
        if let Some(b) = bytes
            && n > 0
        {
            if m.voxels.is_none() {
                self.no_voxels("MRIconst")?;
            }
            if let Some(v) = m.voxels.as_mut() {
                for c in v.chunks_exact_mut(b.len()).take(n) {
                    c.copy_from_slice(&b);
                }
            }
        }
        Ok(match own {
            Some(m) => VolOut::New(m),
            None if given => VolOut::Given,
            None => VolOut::Nothing,
        })
    }

    /// `MRInormalizeSequence(mri, target)` (§38): each voxel's frames divided by `sqrt(Σ v²) /
    /// target` (1 when its magnitude is below `FLT_EPSILON`); bytes, shorts and unsigned shorts
    /// through a 32-bit integer (FS-12, FS-472), floats as `float`s. Another type: the error at
    /// the first voxel and `ERROR_UNSUPPORTED`.
    pub fn mri_normalize_sequence(&mut self, m: &mut Mri, target: f32) -> Result<i32, Stop> {
        let g = m.geom.clone();
        let t = sse::widen(target);
        for x in 0..g.width {
            for y in 0..g.height {
                for z in 0..g.depth {
                    let mut norm = 0.0f64;
                    for f in 0..m.nframes {
                        let mut v = 0.0;
                        self.mri_sample_volume_frame(
                            m,
                            f64::from(x),
                            f64::from(y),
                            f64::from(z),
                            f,
                            &mut v,
                        )?;
                        norm = sse::add64(norm, sse::mul64(v, v));
                    }
                    norm = sse::div64(sse::sqrt64(norm), t);
                    if norm.abs() < FLT_EPSILON {
                        norm = 1.0;
                    }
                    let kind = m.kind;
                    for f in 0..m.nframes {
                        if !matches!(
                            kind,
                            types::UCHAR | types::SHORT | types::USHRT | types::FLOAT
                        ) {
                            self.error_printf(
                                codes::UNSUPPORTED,
                                format!("MRInormalizeSequence: unsupported input type {kind}")
                                    .as_bytes(),
                            );
                            return Ok(codes::UNSUPPORTED);
                        }
                        let e = self.direct_get(
                            m,
                            kind,
                            i64::from(x),
                            i64::from(y),
                            i64::from(z),
                            i64::from(f),
                            "MRInormalizeSequence",
                        )?;
                        let q = sse::div64(sse::widen(e.to_f32()), norm);
                        let b = if kind == types::FLOAT {
                            sse::narrow(q).to_le_bytes().to_vec()
                        } else {
                            let k = self.int_fs12(q);
                            self.store_int(kind, k, "MRInormalizeSequence")
                        };
                        self.direct_put(
                            m,
                            i64::from(x),
                            i64::from(y),
                            i64::from(z),
                            i64::from(f),
                            &b,
                            "MRInormalizeSequence",
                        )?;
                    }
                }
            }
        }
        Ok(codes::NONE)
    }

    /// The frames' values of one voxel (`MRIgetVoxVal`, as `double`s).
    fn frames_of(&mut self, m: &Mri, p: (i32, i32, i32)) -> Result<Vec<f64>, Stop> {
        let mut v = Vec::with_capacity(m.nframes.max(0) as usize);
        for f in 0..m.nframes {
            v.push(sse::widen(self.vf(m, p, f)?));
        }
        Ok(v)
    }

    /// The three frame normalisations of §38: `k` 0 `MRInormalizeFrames` (`(v - mean) /
    /// sqrt(var)`), 1 `MRInormalizeFramesMean` (`v / mean`, a mean below 0.000001 in magnitude
    /// taken as 1), 2 `MRInormalizeFramesFirst` (`v / frame 0`); without a volume the error and
    /// `ERROR_BADPARM`.
    pub fn mri_normalize_frames_kind(&mut self, m: Option<&mut Mri>, k: i32) -> Result<i32, Stop> {
        let Some(m) = m else {
            self.error_printf(codes::BAD_PARM, b"MRInormalize: no MRI\n");
            return Ok(codes::BAD_PARM);
        };
        let g = m.geom.clone();
        let n = f64::from(m.nframes);
        for x in 0..g.width {
            for y in 0..g.height {
                for z in 0..g.depth {
                    let v = self.frames_of(m, (x, y, z))?;
                    let (mut sum, mut sum2) = (0.0f64, 0.0f64);
                    for &a in &v {
                        sum = sse::add64(sum, a);
                        sum2 = sse::add64(sum2, sse::mul64(a, a));
                    }
                    let mean = sse::div64(sum, n);
                    let q: Box<dyn Fn(f64) -> f64> = match k {
                        0 => {
                            let var = sse::sub64(sse::div64(sum2, n), sse::mul64(mean, mean));
                            let sd = sse::sqrt64(var);
                            Box::new(move |a| sse::div64(sse::sub64(a, mean), sd))
                        }
                        1 => {
                            let mean = if mean.abs() < 0.000001 { 1.0 } else { mean };
                            Box::new(move |a| sse::div64(a, mean))
                        }
                        _ => {
                            let first = v.first().copied().unwrap_or(0.0);
                            Box::new(move |a| sse::div64(a, first))
                        }
                    };
                    for (f, &a) in v.iter().enumerate() {
                        self.mri_set_vox_val(m, x, y, z, f as i32, sse::narrow(q(a)), 0)?;
                    }
                }
            }
        }
        Ok(codes::NONE)
    }

    /// `MRInormalizeFrames(mri)` (§38).
    pub fn mri_normalize_frames(&mut self, m: Option<&mut Mri>) -> Result<i32, Stop> {
        self.mri_normalize_frames_kind(m, 0)
    }

    /// `MRInormalizeFramesMean(mri)` (§38).
    pub fn mri_normalize_frames_mean(&mut self, m: Option<&mut Mri>) -> Result<i32, Stop> {
        self.mri_normalize_frames_kind(m, 1)
    }

    /// `MRInormalizeFramesFirst(mri)` (§38).
    pub fn mri_normalize_frames_first(&mut self, m: Option<&mut Mri>) -> Result<i32, Stop> {
        self.mri_normalize_frames_kind(m, 2)
    }

    /// `MRIsort(in, mask, sorted)` (§38): within the mask (a voxel below 0.5 skipped), each voxel's
    /// frames as `double`s in glibc's `qsort` order with `CompareDoubles`, into a new volume of
    /// `in`'s extent, type, frames and header or the given one (its four dimensions checked); in
    /// place allowed.
    pub fn mri_sort(
        &mut self,
        src: &mut Mri,
        mask: Option<&mut Mri>,
        dst: Dst,
    ) -> Result<VolOut, Stop> {
        let has_mask = mask.is_some();
        let mut v = Vols::new(src, mask, dst);
        if !v.has_dst() {
            let a = v.a();
            let g = &a.geom;
            let m = self.alloc_with_header(a, (g.width, g.height, g.depth), a.kind, a.nframes)?;
            v.set_dst(m);
        }
        if !same_dims(v.a(), v.d()) {
            self.put_out(b"ERROR: MRIsort: dimension mismatch\n");
            return Ok(VolOut::Nothing);
        }
        let g = v.a().geom.clone();
        for s in 0..g.depth {
            for r in 0..g.height {
                for c in 0..g.width {
                    if has_mask && self.v0(v.b(), (c, r, s))? < 0.5 {
                        continue;
                    }
                    let mut f = self.frames_of(v.a(), (c, r, s))?;
                    qsort(&mut f, compare_doubles);
                    for (k, &x) in f.iter().enumerate() {
                        self.mri_set_vox_val(v.d_mut(), c, r, s, k as i32, sse::narrow(x), 0)?;
                    }
                }
            }
        }
        Ok(v.out())
    }

    /// `MRIreverseSliceOrder(in, out)` (§38): a clone of `in` with its slices reversed and its
    /// b-value and b-vector tables (new, when `in` has them) copied row by row (FS-473: rows past
    /// a shorter table left 0, noted); in place refused.
    pub fn mri_reverse_slice_order(&mut self, src: &mut Mri, dst: Dst) -> Result<VolOut, Stop> {
        match dst {
            Dst::Src => {
                self.put_out(b"ERROR: MRIreverseSliceOrder: cannot be done in-place\n");
                Ok(VolOut::Nothing)
            }
            Dst::Given(w) => {
                if matches!(self.mri_clone(src, Dst::Given(&mut *w))?, VolOut::Nothing) {
                    return Ok(VolOut::Nothing);
                }
                self.reverse_slices_into(src, w)?;
                Ok(VolOut::Given)
            }
            _ => match self.mri_clone(src, Dst::New)? {
                VolOut::New(mut m) => {
                    self.reverse_slices_into(src, &mut m)?;
                    Ok(VolOut::New(m))
                }
                r => Ok(r),
            },
        }
    }

    fn reverse_slices_into(&mut self, src: &Mri, out: &mut Mri) -> Result<(), Stop> {
        let nf = src.nframes;
        if src.bvals.is_some() {
            out.bvals = Some(Matrix::zeros(nf, 1, MATRIX_REAL));
        }
        if src.bvecs.is_some() {
            out.bvecs = Some(Matrix::zeros(nf, 3, MATRIX_REAL));
        }
        let g = src.geom.clone();
        for s in 0..g.depth {
            for c in 0..g.width {
                for r in 0..g.height {
                    for f in 0..nf {
                        let v = self.vf(src, (c, r, s), f)?;
                        self.mri_set_vox_val(out, c, r, g.depth - 1 - s, f, v, 0)?;
                    }
                }
            }
        }
        for (from, to, cols) in [
            (&src.bvals, &mut out.bvals, 1),
            (&src.bvecs, &mut out.bvecs, 3),
        ] {
            let (Some(a), Some(b)) = (from, to.as_mut()) else {
                continue;
            };
            for f in 1..=nf {
                for j in 1..=cols {
                    if f <= a.rows && j <= a.cols {
                        b.set(f, j, a.at(f, j));
                    } else {
                        self.note(
                            &FS_473,
                            "MRIreverseSliceOrder: the b-table entries past the source's rows left 0 (FreeSurfer reads past its table)",
                        );
                    }
                }
            }
        }
        out.bvec_space = src.bvec_space;
        Ok(())
    }

    /// `MRIorderIndices(mri, xs, ys, zs)` (§38): the voxels (x outermost) ordered by their frame-0
    /// value as a byte (glibc's `qsort`: equal values keep their order), their coordinates into
    /// the three arrays (one entry per voxel). FS-474: coordinates and values kept in bytes with
    /// `reproduce`; by default whole coordinates and values clamped (NaN 0).
    pub fn mri_order_indices(
        &mut self,
        m: &Mri,
        xs: &mut [i16],
        ys: &mut [i16],
        zs: &mut [i16],
    ) -> Result<i32, Stop> {
        let reproduce = self.mode().reproduce();
        let g = m.geom.clone();
        let mut t: Vec<(i32, i32, i32, u8)> = Vec::new();
        let mut differs = false;
        for x in 0..g.width {
            for y in 0..g.height {
                for z in 0..g.depth {
                    let v = self.v0(m, (x, y, z))?;
                    let low = cvtt(f64::from(v)) as u8;
                    let clamped = if v.is_nan() {
                        0
                    } else {
                        v.trunc().clamp(0.0, 255.0) as u8
                    };
                    differs |= low != clamped || x > 255 || y > 255 || z > 255;
                    if reproduce {
                        t.push((x & 255, y & 255, z & 255, low));
                    } else {
                        t.push((x, y, z, clamped));
                    }
                }
            }
        }
        if differs {
            if reproduce {
                self.note(
                    &FS_474,
                    "MRIorderIndices: coordinates and values kept in bytes (wrapped)",
                );
            } else {
                self.note(
                    &FS_474,
                    "MRIorderIndices: whole coordinates and clamped values (FreeSurfer keeps them in bytes)",
                );
            }
        }
        qsort(&mut t, |a, b| i32::from(a.3) - i32::from(b.3));
        for (i, &(x, y, z, _)) in t.iter().enumerate() {
            if i < xs.len() && i < ys.len() && i < zs.len() {
                xs[i] = x as i16;
                ys[i] = y as i16;
                zs[i] = z as i16;
            }
        }
        Ok(codes::NONE)
    }

    /// `MRIcomputeVoxelPermutation(mri, xs, ys, zs)` (§38): the voxels' coordinates in order, each
    /// entry `i` from 0 exchanged with entry `(int)randomNumber(0, n - 0.0001)`.
    pub fn mri_compute_voxel_permutation(
        &mut self,
        m: &Mri,
        xs: &mut [i16],
        ys: &mut [i16],
        zs: &mut [i16],
    ) -> Result<i32, Stop> {
        let g = &m.geom;
        let (w, h, d) = (i64::from(g.width), i64::from(g.height), i64::from(g.depth));
        let n = (w * h * d) as i32;
        let n = (n.max(0) as usize)
            .min(xs.len())
            .min(ys.len())
            .min(zs.len());
        for i in 0..n as i64 {
            let k = i as usize;
            xs[k] = (i % w) as i16;
            ys[k] = ((i / w) % h) as i16;
            zs[k] = ((i / (w * h)) % d) as i16;
        }
        for i in 0..n {
            let j = cvtt(self.random_number(0.0, n as f64 - 0.0001)?);
            if let Ok(j) = usize::try_from(j)
                && j < n
            {
                xs.swap(i, j);
                ys.swap(i, j);
                zs.swap(i, j);
            }
        }
        Ok(codes::NONE)
    }

    /// `MRInonMaxSuppress(src, sup, thresh, dir)` (§38): a copy in which each voxel of frame 0
    /// (x outermost) at or below `thresh` (times `dir`) is 0, and each other one is 0 when a sample
    /// along its gradient, at `u` times the gradient on either side, lies above it.
    pub fn mri_non_max_suppress(
        &mut self,
        src: &mut Mri,
        dst: Dst,
        thresh: f32,
        dir: i32,
    ) -> Result<VolOut, Stop> {
        let mut v = Vols::new(src, None, dst);
        if v.has_dst() {
            if !v.d_is_a() {
                let s = v.a().clone();
                if self.mri_copy(&s, Some(v.d_mut()))?.is_nothing() {
                    return Ok(VolOut::Nothing);
                }
            }
        } else {
            match self.mri_copy(v.a(), None)? {
                Returned::New(m) => v.set_dst(m),
                _ => return Ok(VolOut::Nothing),
            }
        }
        let g = v.a().geom.clone();
        let (k, t) = (f64::from(dir), sse::widen(thresh));
        let dirs = [(0.0, 0.0, 1.0), (0.0, 1.0, 0.0), (1.0, 0.0, 0.0)];
        for x in 0..g.width {
            for y in 0..g.height {
                for z in 0..g.depth {
                    let val = sse::mul64(sse::widen(self.v0(v.a(), (x, y, z))?), k);
                    if val <= t {
                        self.mri_set_vox_val(v.d_mut(), x, y, z, 0, 0.0, 0)?;
                        continue;
                    }
                    let (fx, fy, fz) = (f64::from(x), f64::from(y), f64::from(z));
                    let (mut ix, mut iy, mut iz) = (0.0, 0.0, 0.0);
                    self.mri_sample_volume_gradient(v.a(), fx, fy, fz, &mut ix, &mut iy, &mut iz)?;
                    let dot = |(nx, ny, nz): (f64, f64, f64)| {
                        sse::add64(
                            sse::add64(sse::mul64(nx, ix), sse::mul64(ny, iy)),
                            sse::mul64(nz, iz),
                        )
                        .abs()
                    };
                    let mut n = dirs[0];
                    let mut best = dot(n);
                    for &e in &dirs[1..] {
                        let a = dot(e);
                        if a > best {
                            best = a;
                            n = e;
                        }
                    }
                    let (nx, ny, nz) = n;
                    let along = |(px, py, pz): (f64, f64, f64)| {
                        sse::add64(
                            sse::add64(
                                sse::mul64(nx, sse::sub64(px, fx)),
                                sse::mul64(ny, sse::sub64(py, fy)),
                            ),
                            sse::mul64(nz, sse::sub64(pz, fz)),
                        )
                    };
                    let numer = along((sse::add64(fx, nx), sse::add64(fy, ny), sse::add64(fz, nz)));
                    let denom = along((sse::add64(fx, ix), sse::add64(fy, iy), sse::add64(fz, iz)));
                    if denom.abs() < 1e-15 {
                        continue;
                    }
                    let u = sse::div64(numer, denom);
                    for side in [1.0f64, -1.0] {
                        let at = |p: f64, i: f64| {
                            if side > 0.0 {
                                sse::add64(p, sse::mul64(u, i))
                            } else {
                                sse::sub64(p, sse::mul64(u, i))
                            }
                        };
                        let mut s = 0.0;
                        self.mri_sample_volume(v.a(), at(fx, ix), at(fy, iy), at(fz, iz), &mut s)?;
                        if sse::mul64(s, k) > val {
                            self.mri_set_vox_val(v.d_mut(), x, y, z, 0, 0.0, 0)?;
                            break;
                        }
                    }
                }
            }
        }
        Ok(v.out())
    }

    /// `MRIextractRegionAndPad(src, dst, region, pad)` (§38): a new volume of the region plus `2
    /// pad`, the region at `(pad, pad, pad)`, its geometry the source's scanner vox2ras times a
    /// translation by `-pad`; without a region the whole volume (its line printed).
    pub fn mri_extract_region_and_pad(
        &mut self,
        src: &mut Mri,
        region: Option<Region>,
        pad: i32,
    ) -> Result<Option<Mri>, Stop> {
        let g = src.geom.clone();
        let r = match region {
            Some(r) => r,
            None => {
                self.put_out(
                    format!(
                        "(box.dx, box.dy, box.dz) = ({}, {}, {})\n",
                        g.width, g.height, g.depth
                    )
                    .as_bytes(),
                );
                Region {
                    x: 0,
                    y: 0,
                    z: 0,
                    dx: g.width,
                    dy: g.height,
                    dz: g.depth,
                }
            }
        };
        let line = format!(
            "(region->dx, region->dy, region->dz) = ({}, {}, {})\n",
            r.dx, r.dy, r.dz
        );
        self.put_out(line.as_bytes());
        let p2 = pad.wrapping_mul(2);
        let (kind, nf) = (src.kind, src.nframes);
        let mut out = self.alloc_with_header(
            src,
            (
                r.dx.wrapping_add(p2),
                r.dy.wrapping_add(p2),
                r.dz.wrapping_add(p2),
            ),
            kind,
            nf,
        )?;
        self.put_out(line.as_bytes());
        if let VolOut::New(mut tmp) =
            self.mri_extract(src, Dst::New, r.x, r.y, r.z, r.dx, r.dy, r.dz)?
        {
            self.mri_extract_into(
                &mut tmp,
                Dst::Given(&mut out),
                [0, 0, 0],
                [r.dx, r.dy, r.dz],
                [pad, pad, pad],
            )?;
        }
        let v = self.vox2ras_scanner(&src.geom, 0, false)?;
        let mut t = Matrix::zeros(4, 4, MATRIX_REAL);
        for k in 1..=4 {
            t.set(k, k, 1.0);
        }
        for k in 1..=3 {
            t.set(k, 4, -(pad as f32));
        }
        if let Returned::New(m) = self.matrix_multiply(Some(&v), Some(&t), None)? {
            self.set_vox2ras_from_matrix(&mut out.geom, &m)?;
        }
        Ok(Some(out))
    }

    /// `MRIrms(in, out)` (§38): frame 0 of `out` holds the running sum of the squares between
    /// frames (`vin vin + vout`, the last frame `vout + vin vin`, divided by the frame count and
    /// square-rooted), stored as `out`'s type stores it.
    pub fn mri_rms(&mut self, src: &Mri, out: &mut Mri) -> Result<(), Stop> {
        let g = src.geom.clone();
        let nf = src.nframes;
        let n = f64::from(nf.max(1));
        for f in 0..nf {
            for z in 0..g.depth {
                for y in 0..g.height {
                    for x in 0..g.width {
                        let vin = sse::widen(self.vf(src, (x, y, z), f)?);
                        let vout = if f == 0 {
                            0.0
                        } else {
                            sse::widen(self.v0(out, (x, y, z))?)
                        };
                        let sq = sse::mul64(vin, vin);
                        let v = if f == nf - 1 {
                            sse::sqrt64(sse::div64(sse::add64(vout, sq), n))
                        } else {
                            sse::add64(sq, vout)
                        };
                        self.mri_set_vox_val(out, x, y, z, 0, sse::narrow(v), 0)?;
                    }
                }
            }
        }
        Ok(())
    }

    /// `MRIcomputeVolumeFractions(src, vox2vox, seg, fractions)` (§38): each segmentation voxel
    /// mapped into the source counts there and adds 1 to its label's frame (`label - 1`); the
    /// counted voxels' fractions divided by the count, the others mapped back into the
    /// segmentation and given 1 in their label's frame (FS-476: the last label too by default);
    /// FS-475: label 0 left out. No inverse of the matrix ends the program (status 252).
    pub fn mri_compute_volume_fractions(
        &mut self,
        src: &Mri,
        vox2vox: Option<&Matrix>,
        seg: &Mri,
        fr: &mut Mri,
    ) -> Result<i32, Stop> {
        let inv = match self.matrix_inverse(vox2vox, None)? {
            Returned::New(m) => m,
            _ => {
                self.matrix_print(Some(false), vox2vox, 5);
                self.error_exit(
                    codes::BAD_PARM,
                    b"MRIcomputePartialVolumeFractions: non-invertible vox2vox matrix",
                )?;
                return Ok(codes::BAD_PARM);
            }
        };
        let m = vox2vox.expect("an invertible matrix");
        let mut count = self.mri_clone_different_type(src, types::INT)?;
        let (sg, g) = (seg.geom.clone(), src.geom.clone());
        let nf = fr.nframes;
        let reproduce = self.mode().reproduce();
        let inside = |(x, y, z): (i32, i32, i32), g: &crate::mri::VolGeom| {
            x >= 0 && x < g.width && y >= 0 && y < g.height && z >= 0 && z < g.depth
        };
        for x in 0..sg.width {
            for y in 0..sg.height {
                for z in 0..sg.depth {
                    let q = real_point_f(m, [x as f32, y as f32, z as f32, 1.0]);
                    let p = (
                        self.nint(sse::widen(q[0])),
                        self.nint(sse::widen(q[1])),
                        self.nint(sse::widen(q[2])),
                    );
                    if !inside(p, &g) {
                        continue;
                    }
                    let c = self.v0(&count, p)?;
                    self.mri_set_vox_val(&mut count, p.0, p.1, p.2, 0, c + 1.0, 0)?;
                    let sv = self.v0(seg, (x, y, z))?;
                    let label = self.int_fs12(sse::widen(sv));
                    if label == 0 {
                        self.note(
                            &FS_475,
                            "MRIcomputeVolumeFractions: a voxel of label 0 left out (FreeSurfer counts it outside the volume)",
                        );
                    } else if label >= 1 && label <= nf {
                        let v = self.vf(fr, p, label - 1)?;
                        self.mri_set_vox_val(fr, p.0, p.1, p.2, label - 1, v + 1.0, 0)?;
                    }
                }
            }
        }
        for x in 0..g.width {
            for y in 0..g.height {
                for z in 0..g.depth {
                    let c = self.v0(&count, (x, y, z))?;
                    if c >= 1.0 {
                        for f in 0..nf {
                            let v = self.vf(fr, (x, y, z), f)?;
                            self.mri_set_vox_val(fr, x, y, z, f, sse::div32(v, c), 0)?;
                        }
                        continue;
                    }
                    let q = real_point_f(&inv, [x as f32, y as f32, z as f32, 1.0]);
                    let p = (
                        self.nint(sse::widen(q[0])),
                        self.nint(sse::widen(q[1])),
                        self.nint(sse::widen(q[2])),
                    );
                    if !inside(p, &sg) {
                        continue;
                    }
                    let sv = self.v0(seg, p)?;
                    let label = self.int_fs12(sse::widen(sv));
                    if label == 0 {
                        self.note(
                            &FS_475,
                            "MRIcomputeVolumeFractions: a voxel of label 0 left out (FreeSurfer marks it outside the volume)",
                        );
                        continue;
                    }
                    if label == nf && nf >= 1 {
                        if reproduce {
                            self.note(
                                &FS_476,
                                "MRIcomputeVolumeFractions: the last label not marked in the second pass",
                            );
                            continue;
                        }
                        self.note(
                            &FS_476,
                            "MRIcomputeVolumeFractions: the last label marked in the second pass (FreeSurfer leaves it out)",
                        );
                    }
                    if label >= 1 && label <= nf {
                        self.mri_set_vox_val(fr, x, y, z, label - 1, 1.0, 0)?;
                    }
                }
            }
        }
        Ok(codes::NONE)
    }

    /// `MRIcombineDistanceTransforms(a, b, dst)` (§38): `a`'s voxel, replaced by `b`'s where `b < 0
    /// < a`, `0 < b < a` or `a < b < 0`, into `dst` (a clone of `a` when none).
    pub fn mri_combine_distance_transforms(
        &mut self,
        a: &mut Mri,
        b: &mut Mri,
        dst: Dst,
    ) -> Result<VolOut, Stop> {
        let mut v = Vols::new(a, Some(b), dst);
        self.clone_dst(&mut v)?;
        if !v.has_dst() {
            return Ok(VolOut::Nothing);
        }
        let g = v.d().geom.clone();
        for f in 0..v.d().nframes {
            for x in 0..g.width {
                for y in 0..g.height {
                    for z in 0..g.depth {
                        let va = self.vf(v.a(), (x, y, z), f)?;
                        let vb = self.vf(v.b(), (x, y, z), f)?;
                        let take = (vb < 0.0 && 0.0 < va)
                            || (0.0 < vb && vb < va)
                            || (va < vb && vb < 0.0);
                        let r = if take { vb } else { va };
                        self.mri_set_vox_val(v.d_mut(), x, y, z, f, r, 0)?;
                    }
                }
            }
        }
        Ok(v.out())
    }

    /// `MRIdiff2Vect(m1, m2, thresh, mask)` (§38): the rows `[c, r, s, v1, v2]` (frame 0; columns
    /// outermost) of the voxels in the mask whose difference is not below `thresh`.
    pub fn mri_diff2vect(
        &mut self,
        m1: Option<&Mri>,
        m2: Option<&Mri>,
        thresh: f64,
        mask: Option<&Mri>,
    ) -> Result<Option<Vec<[f64; 5]>>, Stop> {
        let Some(a) = m1 else {
            self.put_out(b"MRIdiff2Vect(): ERROR: mri1 is NULL\n");
            return Ok(None);
        };
        let Some(b) = m2 else {
            self.put_out(b"MRIdiff2Vect(): ERROR: mri2 is NULL\n");
            return Ok(None);
        };
        if !same_extent(a, b) {
            self.put_out(b"MRIdiff2Vect(): ERROR: dimension mismatch\n");
            return Ok(None);
        }
        if let Some(m) = mask
            && !same_extent(a, m)
        {
            self.put_out(b"MRIdiff2Vect(): ERROR: dimension mismatch with mask\n");
            return Ok(None);
        }
        let g = a.geom.clone();
        let mut rows = Vec::new();
        for c in 0..g.width {
            for r in 0..g.height {
                for s in 0..g.depth {
                    if let Some(m) = mask
                        && self.v0(m, (c, r, s))? < 0.5
                    {
                        continue;
                    }
                    let v1 = sse::widen(self.v0(a, (c, r, s))?);
                    let v2 = sse::widen(self.v0(b, (c, r, s))?);
                    if sse::sub64(v1, v2).abs() < thresh {
                        continue;
                    }
                    rows.push([f64::from(c), f64::from(r), f64::from(s), v1, v2]);
                }
            }
        }
        Ok(Some(rows))
    }

    /// `MRIapplyDiffVect(m1, mask, list, napply, m2)` (§38): `m1` copied into `m2` (with its pulse
    /// parameters and a deep copy of its colour table), then each entry's value written at its
    /// voxel (frame 0) within the mask and counted. FS-477: an index equal to the extent passes
    /// with `reproduce`; FS-478: in place with a colour table, the table lost with `reproduce`.
    pub fn mri_apply_diff_vect(
        &mut self,
        m1: &mut Mri,
        mask: Option<&mut Mri>,
        list: &[[f64; 5]],
        napply: &mut i32,
        dst: Dst,
    ) -> Result<VolOut, Stop> {
        *napply = 0;
        let has_mask = mask.is_some();
        let mut v = Vols::new(m1, mask, dst);
        if !v.d_is_a() {
            if v.has_dst() {
                let s = v.a().clone();
                if self.mri_copy(&s, Some(v.d_mut()))?.is_nothing() {
                    return Ok(VolOut::Nothing);
                }
            } else {
                match self.mri_copy(v.a(), None)? {
                    Returned::New(m) => v.set_dst(m),
                    _ => return Ok(VolOut::Nothing),
                }
            }
            let a = v.a().clone();
            self.copy_pulse_parameters(&a, v.d_mut());
        }
        if v.a().ct.is_some() {
            if v.d_is_a() {
                if self.mode().reproduce() {
                    self.note(
                        &FS_478,
                        "MRIapplyDiffVect: in place, the colour table freed before its copy (lost)",
                    );
                    v.d_mut().ct = None;
                    self.ctab_deep_copy(None);
                } else {
                    self.note(
                        &FS_478,
                        "MRIapplyDiffVect: in place, the colour table kept (FreeSurfer frees it before copying it)",
                    );
                }
            } else {
                let ct = self.ctab_deep_copy(v.a().ct.as_ref());
                v.d_mut().ct = ct;
            }
        }
        if !same_extent(v.a(), v.d()) {
            self.put_out(b"MRIapplyDiffVect(): ERROR: dimension mismatch\n");
            return Ok(VolOut::Nothing);
        }
        if has_mask && !same_extent(v.a(), v.b()) {
            self.put_out(b"MRIapplyDiffVect(): ERROR: dimension mismatch with mask\n");
            return Ok(VolOut::Nothing);
        }
        let g = v.a().geom.clone();
        let reproduce = self.mode().reproduce();
        for e in list {
            let (c, r, s) = (
                self.int_fs12(e[0]),
                self.int_fs12(e[1]),
                self.int_fs12(e[2]),
            );
            let at_extent = c == g.width || r == g.height || s == g.depth;
            let past = |k: i32, n: i32| if reproduce { k > n } else { k >= n };
            if at_extent {
                if reproduce {
                    self.note(
                        &FS_477,
                        "MRIapplyDiffVect: an index equal to the extent accepted (written past the volume)",
                    );
                } else {
                    self.note(
                        &FS_477,
                        "MRIapplyDiffVect: an index equal to the extent refused (FreeSurfer writes past the volume)",
                    );
                }
            }
            if c < 0 || r < 0 || s < 0 || past(c, g.width) || past(r, g.height) || past(s, g.depth)
            {
                self.put_out(
                    format!("MRIapplyDiffVect(): voxel index out of bounds {c} {r} {s}\n")
                        .as_bytes(),
                );
                return Ok(VolOut::Nothing);
            }
            if has_mask && self.v0(v.b(), (c, r, s))? < 0.5 {
                continue;
            }
            self.mri_set_vox_val(v.d_mut(), c, r, s, 0, sse::narrow(e[4]), 0)?;
            *napply += 1;
        }
        Ok(v.out())
    }

    /// `MRIestimateTIV(lta, scale, &det)` (§38): the transform read (none ends the program,
    /// status 255); `(scale * 1000) / det` with `det` the `float` determinant of its first linear
    /// transform's matrix. FS-479: a file of no linear transform ends the program here (FreeSurfer
    /// crashes).
    pub fn mri_estimate_tiv(
        &mut self,
        name: &[u8],
        scale: f64,
        det_out: Option<&mut f64>,
        inputs: &mut dyn TransformInputs,
    ) -> Result<f64, Stop> {
        let Some(lta) = self.lta_read_ex(name, inputs)? else {
            let mut msg = self.progname.clone();
            msg.extend_from_slice(b": could not open atlas transform file ");
            msg.extend_from_slice(name);
            self.error_exit(codes::NO_FILE, &msg)?;
            return Ok(0.0);
        };
        let Some(m) = lta.xforms.first().and_then(|t| t.m_l.as_ref()) else {
            self.note(
                &FS_479,
                "MRIestimateTIV: a transform file of no linear transform refused (FreeSurfer crashes)",
            );
            let mut msg = b"MRIestimateTIV: no linear transform in ".to_vec();
            msg.extend_from_slice(name);
            self.error_exit(codes::BAD_PARM, &msg)?;
            return Ok(0.0);
        };
        let det = f64::from(self.matrix_determinant(m));
        if let Some(d) = det_out {
            *d = det;
        }
        Ok(sse::div64(sse::mul64(scale, 1000.0), det))
    }

    /// `MRIdimMismatch(a, b, frames)` (§38): 1, 2 or 3 for another width, height or depth, 4 for
    /// another frame count (when asked), else 0.
    pub fn mri_dim_mismatch(a: &Mri, b: &Mri, frames: bool) -> i32 {
        if a.geom.width != b.geom.width {
            1
        } else if a.geom.height != b.geom.height {
            2
        } else if a.geom.depth != b.geom.depth {
            3
        } else if frames && a.nframes != b.nframes {
            4
        } else {
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compare_doubles_signs() {
        assert_eq!(compare_doubles(&1.0, &2.0), -1);
        assert_eq!(compare_doubles(&2.0, &1.0), 1);
        assert_eq!(compare_doubles(&f64::NAN, &1.0), 0);
    }
}
