// SPDX-License-Identifier: LicenseRef-FSL
// Translated to Rust from FSL 6.0.7.23: fsl-warpfns 2501.0 (warpfns.h: raw_general_transform,
// RGT_UTILS, affine_transform, general_transform, displacement_transform_1D, apply_warp,
// raw_apply_warp; warpfns.cc: rows_per_thread; fnirt_file_reader.cpp: deffield2jacobian,
// deffield2jacobian_matrix). Copyright (C) 2007-2025 University of Oxford (Jesper Andersson, Mark
// Jenkinson et al.). Rust translation Copyright 2026 Karel Lopez Vilaret; modified from the
// originals. Distributed under the FSL Licence (see LICENSE).
//! fsl-warpfns' resampling of an image through an affine transform and displacement fields
//! (`raw_general_transform` and its wrappers), with the image's partial derivatives and the
//! mask of voxels inside it, and the Jacobian determinants of B-spline displacement fields
//! (`deffield2jacobian`), as topup and applytopup use them.
//!
//! The resampling maps each output voxel `(i, j, k)` to the input's voxels: an affine-only
//! transform by `iM · A⁻¹ · S_out · iT` (12 `float` coefficients stepped along `i`); with
//! displacement fields, `x = A⁻¹ · S_d · (i, j, k)` plus the displacements (in millimetres) at
//! the voxel (or interpolated in the field's space when it differs from the output's), then
//! `iM = S_in⁻¹ (· M⁻¹)`. FSL splits the output's rows among threads; each voxel is computed
//! alone, so the thread count never changes a value (the replica splits its slices).
//!
//! Not translated (no replica needs them yet): `affine2warp`, `shift2warp`, `convertwarp_*`,
//! `concat_warps`, `calc_sigloss`, `jacobian_check`, `constrain_topology`,
//! `NewimageCoord2NewimageCoord`, `inv_coord`, `get_displacement_fields`, `FnirtFileReader`,
//! `FnirtFileWriter`, `PointList`.

use crate::arma::{self, Mat};
use crate::basisfield::{FieldIndex, SplineField};
use crate::interp_partial::{ExtrapParams, Interpolator, valid_f, value_ep};
use crate::miscmaths::{INV_ERROR, SOLVE_ERROR};
use crate::nifti::XFORM_UNKNOWN;
use crate::volume::{Extrapolation, FslError, Volume, mirrorclamp, periodicclamp};

/// `NiftiIO::NIFTI_XFORM_ALIGNED_ANAT`.
const XFORM_ALIGNED_ANAT: i32 = 2;

/// `WarpFnsException(msg)`: `what()` is `"warpfns::" + msg`.
pub fn warpfns_error(msg: &str) -> FslError {
    FslError {
        printed: Vec::new(),
        type_name: "NEWIMAGE::WarpFnsException",
        what: format!("warpfns::{msg}"),
    }
}

/// `FnirtFileReaderException(msg)`: `what()` is `"FnirtFileReader:: msg=" + msg`.
pub fn fnirt_file_reader_error(msg: &str) -> FslError {
    FslError {
        printed: Vec::new(),
        type_name: "NEWIMAGE::FnirtFileReaderException",
        what: format!("FnirtFileReader:: msg={msg}"),
    }
}

fn runtime(what: &str) -> FslError {
    FslError {
        printed: Vec::new(),
        type_name: "std::runtime_error",
        what: what.to_owned(),
    }
}

/// An error of larmorx's own where FSL's behaviour is not defined (see the call site).
fn refused(what: String) -> FslError {
    FslError {
        printed: Vec::new(),
        type_name: "larmorx",
        what,
    }
}

/// `RGT_UTILS::rows_per_thread(nrows, nthr)`: the first row of each thread's share, and
/// `nrows` last.
pub fn rows_per_thread(nrows: u32, nthr: u32) -> Vec<u32> {
    let rpt = f64::from(nrows) / f64::from(nthr);
    let mut nr = vec![0u32; nthr as usize + 1];
    for i in 1..nthr as usize {
        nr[i] = (i as f64 * rpt).floor() as u32;
    }
    nr[nthr as usize] = nrows;
    nr
}

/// An empty `volume4D<float>` (no displacement fields, no derivatives).
pub fn empty4d() -> Volume {
    Volume::new7([0, 0, 0, 0, 1, 1, 1])
}

fn samesize3<S: crate::volume::Elem, D: crate::volume::Elem>(a: &Volume<S>, b: &Volume<D>) -> bool {
    a.xsize() == b.xsize() && a.ysize() == b.ysize() && a.zsize() == b.zsize()
}

/// `RGT_UTILS::validate_input(...)`: the first problem's text.
#[allow(clippy::too_many_arguments)]
fn validate_input(
    a: &Mat,
    tt: Option<&Mat>,
    m: Option<&Mat>,
    d: &Volume,
    defdir: &[i32],
    derivdir: &[i32],
    out: &Volume,
    deriv: &Volume,
    valid: Option<&Volume<i8>>,
) -> Result<(), &'static str> {
    let is4 = |x: &Mat| x.rows() == 4 && x.cols() == 4;
    if !is4(a) {
        return Err("A must be 4x4 matrix");
    }
    if tt.is_some_and(|t| !is4(t)) {
        return Err("If specified TT must be a 4x4 matrix");
    }
    if m.is_some_and(|t| !is4(t)) {
        return Err("If specified M must be a 4x4 matrix");
    }
    if defdir.len() as i64 != d.tsize() {
        return Err("Mismatch in input. defdir.size() must equal d.tsize()");
    }
    if defdir.iter().any(|&dir| !(0..=2).contains(&dir)) {
        return Err("Displacements can only be specified for directions 0,1 or 2");
    }
    if derivdir.len() as i64 != deriv.tsize() {
        return Err("Mismatch in input. derivdir.size() must equal deriv.tsize()");
    }
    if derivdir.iter().any(|&dir| !(0..=2).contains(&dir)) {
        return Err("Derivatives can only be specified for directions 0,1 or 2");
    }
    if out.nvoxels() <= 0 {
        return Err("Size of vout must be set");
    }
    // FSL checks the slices only when there are none (`if (slices.size()==0)`), so it never
    // checks them: see raw_general_transform.
    if !derivdir.is_empty() && !samesize3(out, deriv) {
        return Err("vout and deriv must have same dimensions");
    }
    if valid.is_some_and(|v| !samesize3(out, v)) {
        return Err("vout and valid must have same dimensions");
    }
    Ok(())
}

/// `RGT_UTILS::calculate_extrapolation(f, d, infext, indext)`: the input image's parameters
/// are always the default (the volume's own: `infext` is not passed on), the displacement
/// fields' are `extraslice` (`periodic` when the image's method is periodic and there are
/// fields) with the image's validity.
fn calculate_extrapolation(
    f: &Volume,
    d: &Volume,
    infext: &ExtrapParams,
) -> (ExtrapParams, ExtrapParams) {
    let fmeth = infext.method(f);
    let dmeth = if d.tsize() != 0 && fmeth == Extrapolation::Periodic {
        Extrapolation::Periodic
    } else {
        Extrapolation::Extraslice
    };
    let dext = ExtrapParams {
        method: Some(dmeth),
        padvalue: None,
        validity: Some(infext.validity(f)),
    };
    (ExtrapParams::DEFAULT, dext)
}

/// `S.i() * B`: Armadillo's solve.
fn solve(a: &Mat, b: &Mat) -> Result<Mat, FslError> {
    arma::solve(a, b).map_err(|_| runtime(SOLVE_ERROR))
}

/// `X.i()`: Armadillo's inverse.
fn inv(x: &Mat) -> Result<Mat, FslError> {
    arma::inv(x).map_err(|_| runtime(INV_ERROR))
}

/// `RGT_UTILS::make_iT(d, out, TT)`: the mapping of the output's voxels onto the displacement
/// fields' (or the output's own), and whether it differs from the identity by more than 1e-6.
fn make_it(d: &Volume, out: &Volume, tt: Option<&Mat>) -> Result<(Mat, bool), FslError> {
    let sout = out.sampling_mat();
    let it = match tt {
        // inv(A) * inv(TT) * C: solve(A, inv(TT) * C)
        Some(t) => {
            let a = if d.tsize() != 0 {
                d.sampling_mat()
            } else {
                sout.clone()
            };
            solve(&a, &arma::mul(&inv(t)?, &sout, false))?
        }
        None => {
            if d.tsize() != 0 {
                solve(&d.sampling_mat(), &sout)?
            } else {
                Mat::eye(4)
            }
        }
    };
    let use_it = it.minus(&Mat::eye(4)).max_abs() > 1e-6;
    Ok((it, use_it))
}

/// The 12 coefficients of a 4 × 4 matrix's first three rows, as `float`s.
fn f12(m: &Mat) -> [f32; 12] {
    std::array::from_fn(|k| m.at(k / 4, k % 4) as f32)
}

/// What one slice of the output computes from.
struct Ctx<'a> {
    f: Interpolator<'a>,
    d: &'a Volume,
    /// The displacement fields' interpolators (`d[di]`, when the output's space is not theirs).
    dvol: Vec<Interpolator<'a>>,
    defdir: &'a [i32],
    derivdir: &'a [i32],
    fext: ExtrapParams,
    dext: ExtrapParams,
    /// The path: affine only (iB), displacements in the output's space (iA, iM), or not (iT,
    /// iA, iM).
    kind: Kind,
    nx: i64,
    ny: i64,
}

#[derive(Clone, Copy)]
enum Kind {
    Affine([f32; 12]),
    NoIt {
        a: [f32; 12],
        m: [f32; 12],
    },
    WithIt {
        t: [f32; 12],
        a: [f32; 12],
        m: [f32; 12],
    },
}

/// One output slice's arrays: the values, one array per derivative, the mask.
struct SliceOut<'s> {
    k: i64,
    out: &'s mut [f32],
    deriv: Vec<&'s mut [f32]>,
    valid: Option<&'s mut [i8]>,
}

impl Ctx<'_> {
    /// The value (and derivatives) of `f` at `(x, y, z)`, written at `idx` of the slice.
    #[inline]
    fn sample(
        &self,
        s: &mut SliceOut<'_>,
        idx: usize,
        x: f32,
        y: f32,
        z: f32,
    ) -> Result<(), FslError> {
        match self.derivdir.len() {
            0 => s.out[idx] = self.f.interpolate(x, y, z, &self.fext)?,
            1 => {
                let (v, dv) = self
                    .f
                    .interp1partial(x, y, z, self.derivdir[0], &self.fext)?;
                s.out[idx] = v;
                s.deriv[0][idx] = dv;
            }
            _ => {
                let (v, dv) = self.f.interp3partial(x, y, z, &self.fext)?;
                s.out[idx] = v;
                for (di, &dir) in self.derivdir.iter().enumerate() {
                    s.deriv[di][idx] = dv[dir as usize];
                }
            }
        }
        Ok(())
    }

    fn slice(&self, s: &mut SliceOut<'_>) -> Result<(), FslError> {
        let k = s.k;
        let kf = k as f32;
        let fv = self.f.volume();
        for j in 0..self.ny {
            let jf = j as f32;
            match self.kind {
                Kind::Affine(a) => {
                    // affine_no_derivs / affine_with_derivs
                    let mut x = jf * a[1] + kf * a[2] + a[3];
                    let mut y = jf * a[5] + kf * a[6] + a[7];
                    let mut z = jf * a[9] + kf * a[10] + a[11];
                    for i in 0..self.nx {
                        let idx = (j * self.nx + i) as usize;
                        self.sample(s, idx, x, y, z)?;
                        if let Some(v) = s.valid.as_deref_mut() {
                            v[idx] = i8::from(valid_f(fv, x, y, z, 1e-8, &self.fext));
                        }
                        x += a[0];
                        y += a[4];
                        z += a[8];
                    }
                }
                Kind::NoIt { a, m } => {
                    // displacements_no_iT
                    for i in 0..self.nx {
                        let idx = (j * self.nx + i) as usize;
                        let fi = i as f32;
                        let mut x = [
                            fi * a[0] + jf * a[1] + kf * a[2] + a[3],
                            fi * a[4] + jf * a[5] + kf * a[6] + a[7],
                            fi * a[8] + jf * a[9] + kf * a[10] + a[11],
                        ];
                        for (di, &dir) in self.defdir.iter().enumerate() {
                            x[dir as usize] += value_ep(self.d, di, i, j, k, &self.dext)?;
                        }
                        let xx = [
                            x[0] * m[0] + x[1] * m[1] + x[2] * m[2] + m[3],
                            x[0] * m[4] + x[1] * m[5] + x[2] * m[6] + m[7],
                            x[0] * m[8] + x[1] * m[9] + x[2] * m[10] + m[11],
                        ];
                        self.sample(s, idx, xx[0], xx[1], xx[2])?;
                        if let Some(v) = s.valid.as_deref_mut() {
                            v[idx] = i8::from(valid_f(fv, xx[0], xx[1], xx[2], 1e-8, &self.fext));
                        }
                    }
                }
                Kind::WithIt { t, a, m } => {
                    // displacements_with_iT
                    for i in 0..self.nx {
                        let idx = (j * self.nx + i) as usize;
                        let fi = i as f32;
                        let x = [
                            fi * t[0] + jf * t[1] + kf * t[2] + t[3],
                            fi * t[4] + jf * t[5] + kf * t[6] + t[7],
                            fi * t[8] + jf * t[9] + kf * t[10] + t[11],
                        ];
                        let mut xx = [
                            x[0] * a[0] + x[1] * a[1] + x[2] * a[2] + a[3],
                            x[0] * a[4] + x[1] * a[5] + x[2] * a[6] + a[7],
                            x[0] * a[8] + x[1] * a[9] + x[2] * a[10] + a[11],
                        ];
                        for (di, &dir) in self.defdir.iter().enumerate() {
                            xx[dir as usize] +=
                                self.dvol[di].interpolate(x[0], x[1], x[2], &self.dext)?;
                        }
                        let dvalid = valid_f(self.d, x[0], x[1], x[2], 1e-8, &self.dext);
                        let y = [
                            xx[0] * m[0] + xx[1] * m[1] + xx[2] * m[2] + m[3],
                            xx[0] * m[4] + xx[1] * m[5] + xx[2] * m[6] + m[7],
                            xx[0] * m[8] + xx[1] * m[9] + xx[2] * m[10] + m[11],
                        ];
                        self.sample(s, idx, y[0], y[1], y[2])?;
                        if let Some(v) = s.valid.as_deref_mut() {
                            v[idx] =
                                i8::from(valid_f(fv, y[0], y[1], y[2], 1e-8, &self.fext) && dvalid);
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

/// `raw_general_transform(f, A, d, defdir, derivdir, slices, TT, M, out, deriv, valid, nthr,
/// infext, indext)`: `out` (whose size and voxel sizes are set) becomes `f` resampled through
/// the affine `A` (millimetres) and the displacement fields `d` (millimetres, one volume per
/// direction in `defdir`; `d` empty for an affine transform), with `TT` mapping `out` onto
/// `d`'s space and `M` mapping `f` onto the space the transform reaches (each `None` for none).
/// `deriv` (one volume per direction in `derivdir`, per voxel; empty for none) gets `f`'s
/// partial derivatives, `valid` 1 where the position falls inside `f` (or a direction whose
/// extrapolation is valid). Only `slices` of `out` are computed (empty: all). `nthr` threads
/// share the slices (the values do not depend on it). `indext` is not used (FSL computes the
/// fields' parameters itself; `infext` gives only their method and validity).
///
/// The slices are checked (FSL's check never runs, and a slice outside `out` writes through
/// `out`'s extrapolation): a slice outside `out` is refused.
#[allow(clippy::too_many_arguments)]
pub fn raw_general_transform(
    f: &Volume,
    a: &Mat,
    d: &Volume,
    defdir: &[i32],
    derivdir: &[i32],
    slices: &[u32],
    tt: Option<&Mat>,
    m: Option<&Mat>,
    out: &mut Volume,
    deriv: &mut Volume,
    valid: Option<&mut Volume<i8>>,
    nthr: u32,
    infext: &ExtrapParams,
    _indext: &ExtrapParams,
) -> Result<(), FslError> {
    validate_input(a, tt, m, d, defdir, derivdir, out, deriv, valid.as_deref())
        .map_err(|msg| warpfns_error(&format!("NEWIMAGE::raw_general_transform: {msg}")))?;
    let nz = out.zsize();
    let mut ks: Vec<i64> = if slices.is_empty() {
        (0..nz).collect()
    } else {
        slices.iter().map(|&s| i64::from(s)).collect()
    };
    if let Some(&bad) = ks.iter().find(|&&k| k >= nz) {
        return Err(refused(format!(
            "warpfns::NEWIMAGE::raw_general_transform: slice {bad} outside vout ({nz} slices)"
        )));
    }
    // A slice listed twice is computed twice with the same values.
    let mut seen = vec![false; nz as usize];
    ks.retain(|&k| !std::mem::replace(&mut seen[k as usize], true));
    let (fext, dext) = calculate_extrapolation(f, d, infext);
    let (it, use_it) = make_it(d, out, tt)?;
    let mut ia = inv(a)?;
    ia = if !defdir.is_empty() {
        arma::mul(&ia, &d.sampling_mat(), false)
    } else {
        arma::mul(&ia, &out.sampling_mat(), false)
    };
    let im = match m {
        Some(mm) => solve(&f.sampling_mat(), &inv(mm)?)?,
        None => inv(&f.sampling_mat())?,
    };
    let kind = if defdir.is_empty() {
        Kind::Affine(f12(&arma::mul3(&im, &ia, &it)))
    } else if !use_it {
        Kind::NoIt {
            a: f12(&ia),
            m: f12(&im),
        }
    } else {
        Kind::WithIt {
            t: f12(&it),
            a: f12(&ia),
            m: f12(&im),
        }
    };
    let dvol = if matches!(kind, Kind::WithIt { .. }) {
        (0..defdir.len())
            .map(|di| Interpolator::of_volume(d, di as i64))
            .collect::<Result<Vec<_>, _>>()?
    } else {
        Vec::new()
    };
    let ctx = Ctx {
        f: Interpolator::new(f)?,
        d,
        dvol,
        defdir,
        derivdir,
        fext,
        dext,
        kind,
        nx: out.xsize(),
        ny: out.ysize(),
    };
    // Every slice's arrays, taken by the slices computed.
    let plane = (out.xsize() * out.ysize()) as usize;
    let nzu = nz as usize;
    let mut outs: Vec<Option<&mut [f32]>> = out.data[..plane * nzu]
        .chunks_mut(plane)
        .map(Some)
        .collect();
    let nd = derivdir.len();
    let mut dchunks: Vec<Option<&mut [f32]>> = deriv.data[..plane * nzu * nd]
        .chunks_mut(plane)
        .map(Some)
        .collect();
    let mut valid_vol = valid;
    let mut vchunks: Vec<Option<&mut [i8]>> = match valid_vol.as_deref_mut() {
        Some(v) => v.data[..plane * nzu].chunks_mut(plane).map(Some).collect(),
        None => Vec::new(),
    };
    let mut work: Vec<SliceOut<'_>> = ks
        .iter()
        .map(|&k| {
            let ku = k as usize;
            SliceOut {
                k,
                out: outs[ku].take().expect("slice"),
                deriv: (0..nd)
                    .map(|di| dchunks[di * nzu + ku].take().expect("slice"))
                    .collect(),
                valid: if vchunks.is_empty() {
                    None
                } else {
                    vchunks[ku].take()
                },
            }
        })
        .collect();
    let nthr = (nthr.max(1) as usize).min(work.len().max(1));
    let res: Result<(), FslError> = if nthr <= 1 {
        work.iter_mut().try_for_each(|s| ctx.slice(s))
    } else {
        let per = work.len().div_ceil(nthr);
        let ctx = &ctx;
        let results: Vec<Result<(), FslError>> = std::thread::scope(|scope| {
            let handles: Vec<_> = work
                .chunks_mut(per)
                .map(|chunk| scope.spawn(move || chunk.iter_mut().try_for_each(|s| ctx.slice(s))))
                .collect();
            handles
                .into_iter()
                .map(|h| h.join().expect("warpfns thread"))
                .collect()
        });
        results.into_iter().collect()
    };
    drop(work);
    res?;
    set_sqform(f, d, !defdir.is_empty(), ia, tt, m, out, deriv)?;
    Ok(())
}

/// `RGT_UTILS::set_sqform(f, d, defdir, iA, TT, M, out, deriv)`: the output's sform and qform
/// (one copied to the other, or the fields', or the input's combined with the transform), and
/// the derivatives' the output's.
#[allow(clippy::too_many_arguments)]
fn set_sqform(
    f: &Volume,
    d: &Volume,
    has_def: bool,
    mut ia: Mat,
    tt: Option<&Mat>,
    m: Option<&Mat>,
    out: &mut Volume,
    deriv: &mut Volume,
) -> Result<(), FslError> {
    if out.sform_code == XFORM_UNKNOWN && out.qform_code != XFORM_UNKNOWN {
        out.sform_code = out.qform_code;
        out.sform = out.qform.clone();
    } else if out.qform_code == XFORM_UNKNOWN && out.sform_code != XFORM_UNKNOWN {
        out.qform_code = out.sform_code;
        out.qform = out.sform.clone();
    } else if out.qform_code == XFORM_UNKNOWN && out.sform_code == XFORM_UNKNOWN {
        if has_def {
            if d.sform_code != XFORM_UNKNOWN {
                out.sform_code = d.sform_code;
                out.sform = d.sform.clone();
                out.qform_code = d.sform_code;
                out.qform = d.sform.clone();
            } else if d.qform_code != XFORM_UNKNOWN {
                out.sform_code = d.qform_code;
                out.sform = d.qform.clone();
                out.qform_code = d.qform_code;
                out.qform = d.qform.clone();
            }
        } else {
            let base = if f.sform_code != XFORM_UNKNOWN {
                Some(&f.sform)
            } else if f.qform_code != XFORM_UNKNOWN {
                Some(&f.qform)
            } else {
                None
            };
            if let Some(b) = base {
                if let Some(t) = tt {
                    ia = arma::mul(&ia, &inv(t)?, false);
                }
                if let Some(mm) = m {
                    ia = solve(mm, &ia)?;
                }
                ia = arma::mul(b, &ia, false);
                out.sform_code = XFORM_ALIGNED_ANAT;
                out.sform = ia.clone();
                out.qform_code = XFORM_ALIGNED_ANAT;
                out.qform = ia;
            }
        }
    }
    if deriv.tsize() != 0 {
        deriv.sform_code = out.sform_code;
        deriv.sform = out.sform.clone();
        deriv.qform_code = out.qform_code;
        deriv.qform = out.qform.clone();
    }
    Ok(())
}

// ------------------------------------------------------------------------------------------------
// The wrappers

/// `affine_transform(vin, aff, vout, [inside_volume,] nthreads)` (warpfns.h): `vout` becomes
/// `vin` resampled through `aff`; `inside` (if given) 1 where the position falls inside `vin`.
pub fn affine_transform(
    vin: &Volume,
    aff: &Mat,
    vout: &mut Volume,
    inside: Option<&mut Volume<i8>>,
    nthr: u32,
) -> Result<(), FslError> {
    raw_general_transform(
        vin,
        aff,
        &empty4d(),
        &[],
        &[],
        &[],
        None,
        None,
        vout,
        &mut empty4d(),
        inside,
        nthr,
        &ExtrapParams::DEFAULT,
        &ExtrapParams::DEFAULT,
    )
}

/// `affine_transform(vin, aff, slices, vout, inside_volume, nthreads)`: only `slices`.
pub fn affine_transform_slices(
    vin: &Volume,
    aff: &Mat,
    slices: &[u32],
    vout: &mut Volume,
    inside: &mut Volume<i8>,
    nthr: u32,
) -> Result<(), FslError> {
    raw_general_transform(
        vin,
        aff,
        &empty4d(),
        &[],
        &[],
        slices,
        None,
        None,
        vout,
        &mut empty4d(),
        Some(inside),
        nthr,
        &ExtrapParams::DEFAULT,
        &ExtrapParams::DEFAULT,
    )
}

/// `affine_transform_3partial(vin, aff, vout, deriv, [invol,] nthreads)`: with the three
/// partial derivatives (`deriv` with three volumes of `vout`'s size).
pub fn affine_transform_3partial(
    vin: &Volume,
    aff: &Mat,
    vout: &mut Volume,
    deriv: &mut Volume,
    inside: Option<&mut Volume<i8>>,
    nthr: u32,
) -> Result<(), FslError> {
    raw_general_transform(
        vin,
        aff,
        &empty4d(),
        &[],
        &[0, 1, 2],
        &[],
        None,
        None,
        vout,
        deriv,
        inside,
        nthr,
        &ExtrapParams::DEFAULT,
        &ExtrapParams::DEFAULT,
    )
}

/// `df` (a 3D volume) as a one-volume `volume4D` (`pdf.addvolume(df)`).
fn as_4d(df: &Volume) -> Volume {
    let mut v = df.clone();
    v.dims[3] = 1;
    v.data.truncate(df.nvoxels() as usize);
    v
}

/// `displacement_transform_1D(vin, aff, df, defdir, vout, [invol,] nthreads)`: displacements
/// along one direction.
pub fn displacement_transform_1d(
    vin: &Volume,
    aff: &Mat,
    df: &Volume,
    defdir: i32,
    vout: &mut Volume,
    inside: Option<&mut Volume<i8>>,
    nthr: u32,
) -> Result<(), FslError> {
    raw_general_transform(
        vin,
        aff,
        &as_4d(df),
        &[defdir],
        &[],
        &[],
        None,
        None,
        vout,
        &mut empty4d(),
        inside,
        nthr,
        &ExtrapParams::DEFAULT,
        &ExtrapParams::DEFAULT,
    )
}

/// `displacement_transform_1D_3partial(vin, aff, df, dir, vout, deriv, [invol,] nthreads)`.
#[allow(clippy::too_many_arguments)]
pub fn displacement_transform_1d_3partial(
    vin: &Volume,
    aff: &Mat,
    df: &Volume,
    dir: i32,
    vout: &mut Volume,
    deriv: &mut Volume,
    inside: Option<&mut Volume<i8>>,
    nthr: u32,
) -> Result<(), FslError> {
    raw_general_transform(
        vin,
        aff,
        &as_4d(df),
        &[dir],
        &[0, 1, 2],
        &[],
        None,
        None,
        vout,
        deriv,
        inside,
        nthr,
        &ExtrapParams::DEFAULT,
        &ExtrapParams::DEFAULT,
    )
}

/// `general_transform(vin, aff, df, vout, [invol,] nthreads)`: through the affine and three
/// displacement fields (`df`, millimetres, x, y and z).
pub fn general_transform(
    vin: &Volume,
    aff: &Mat,
    df: &Volume,
    vout: &mut Volume,
    inside: Option<&mut Volume<i8>>,
    nthr: u32,
) -> Result<(), FslError> {
    raw_general_transform(
        vin,
        aff,
        df,
        &[0, 1, 2],
        &[],
        &[],
        None,
        None,
        vout,
        &mut empty4d(),
        inside,
        nthr,
        &ExtrapParams::DEFAULT,
        &ExtrapParams::DEFAULT,
    )
}

/// `general_transform_3partial(vin, aff, df, vout, deriv, [invol,] nthreads)`: with the three
/// partial derivatives (topup's resampling).
pub fn general_transform_3partial(
    vin: &Volume,
    aff: &Mat,
    df: &Volume,
    vout: &mut Volume,
    deriv: &mut Volume,
    inside: Option<&mut Volume<i8>>,
    nthr: u32,
) -> Result<(), FslError> {
    raw_general_transform(
        vin,
        aff,
        df,
        &[0, 1, 2],
        &[0, 1, 2],
        &[],
        None,
        None,
        vout,
        deriv,
        inside,
        nthr,
        &ExtrapParams::DEFAULT,
        &ExtrapParams::DEFAULT,
    )
}

/// `apply_warp(vin, A, d, TT, M, vout, [mask,] nthreads)`: `TT` and `M` used when they differ
/// from the identity by more than 1e-6. With a mask, `mask` is first made `vout`'s size and
/// properties (`reinitialize`, `copybasicproperties`).
#[allow(clippy::too_many_arguments)]
pub fn apply_warp(
    vin: &Volume,
    a: &Mat,
    d: &Volume,
    tt: &Mat,
    m: &Mat,
    vout: &mut Volume,
    mask: Option<&mut Volume<i8>>,
    nthr: u32,
) -> Result<(), FslError> {
    let eye = Mat::eye(4);
    let tptr = (tt.minus(&eye).max_abs() > 1e-6).then_some(tt);
    let mptr = (m.minus(&eye).max_abs() > 1e-6).then_some(m);
    let mask = mask.map(|mk| {
        *mk = Volume::<i8>::new(vout.xsize(), vout.ysize(), vout.zsize(), 1);
        mk.copy_properties(vout);
        mk
    });
    raw_general_transform(
        vin,
        a,
        d,
        &[0, 1, 2],
        &[],
        &[],
        tptr,
        mptr,
        vout,
        &mut empty4d(),
        mask,
        nthr,
        &ExtrapParams::DEFAULT,
        &ExtrapParams::DEFAULT,
    )
}

/// `raw_apply_warp(invol, outvol, warpvol, premat, postmat, nthreads, fext, dext)`: the warp
/// with `postmat` as `TT` and `premat` as `M` (always, identities included). As in FSL, `fext`
/// gives only the fields' extrapolation method and validity: the image is interpolated with
/// its own settings (`apply_warp(invol, outvol, warpvol, ...)` passes `{extraslice,
/// background}`, which therefore has no effect on the image's padding).
#[allow(clippy::too_many_arguments)]
pub fn raw_apply_warp(
    invol: &Volume,
    outvol: &mut Volume,
    warpvol: &Volume,
    premat: &Mat,
    postmat: &Mat,
    nthr: u32,
    fext: &ExtrapParams,
    dext: &ExtrapParams,
) -> Result<(), FslError> {
    raw_general_transform(
        invol,
        &Mat::eye(4),
        warpvol,
        &[0, 1, 2],
        &[],
        &[],
        Some(postmat),
        Some(premat),
        outvol,
        &mut empty4d(),
        None,
        nthr,
        fext,
        dext,
    )
}

// ------------------------------------------------------------------------------------------------
// Jacobians of spline displacement fields

/// The index `jac(i, j, k) = ...` writes to: the voxel, or through `jac`'s extrapolation for a
/// voxel outside it (`None`: FSL's write goes to a temporary and is lost). FSL's loops run over
/// the field's size, so a `jac` smaller than the field takes these writes.
fn write_index(jac: &Volume, i: i64, j: i64, k: i64) -> Result<Option<usize>, FslError> {
    let (nx, ny, nz) = (jac.xsize(), jac.ysize(), jac.zsize());
    let lin = |x: i64, y: i64, z: i64| Some(((z * ny + y) * nx + x) as usize);
    if jac.in_bounds(i, j, k) {
        return Ok(lin(i, j, k));
    }
    Ok(match jac.extrap {
        Extrapolation::Zeropad | Extrapolation::Constpad => None,
        Extrapolation::Periodic => lin(
            periodicclamp(i, 0, nx - 1),
            periodicclamp(j, 0, ny - 1),
            periodicclamp(k, 0, nz - 1),
        ),
        Extrapolation::Mirror => lin(
            mirrorclamp(i, 0, nx - 1),
            mirrorclamp(j, 0, ny - 1),
            mirrorclamp(k, 0, nz - 1),
        ),
        Extrapolation::Extraslice => {
            let edge = |c: i64, n: i64| {
                if c == -1 {
                    0
                } else if c == n {
                    n - 1
                } else {
                    c
                }
            };
            let (x, y, z) = (edge(i, nx), edge(j, ny), edge(k, nz));
            if jac.in_bounds(x, y, z) {
                lin(x, y, z)
            } else {
                None
            }
        }
        Extrapolation::BoundsAssert | Extrapolation::BoundsException => {
            return Err(FslError::imthrow("Invalid extrapolation method", 6));
        }
    })
}

/// The three fields' sizes must agree (FSL reads the second and third over the first's size,
/// past their arrays when they are smaller): refused when they differ.
fn same_fields(dx: &SplineField, dy: &SplineField, dz: &SplineField) -> Result<(), FslError> {
    if dx.field_size() != dy.field_size() || dx.field_size() != dz.field_size() {
        return Err(refused(
            "deffield2jacobian: the three displacement fields have different sizes".into(),
        ));
    }
    Ok(())
}

/// A field's derivative, up to date.
fn g(f: &SplineField, fi: FieldIndex) -> &[f64] {
    f.field_cache(fi).expect("updated")
}

/// The nine derivatives `d{x,y,z}/d{x,y,z}`, in FSL's order of `Get` calls.
fn update9(
    dx: &mut SplineField,
    dy: &mut SplineField,
    dz: &mut SplineField,
) -> Result<(), FslError> {
    for fld in [dx, dy, dz] {
        for fi in [FieldIndex::Dfdx, FieldIndex::Dfdy, FieldIndex::Dfdz] {
            fld.update(fi)?;
        }
    }
    Ok(())
}

/// `deffield2jacobian(dx, dy, dz, jac)`: [`deffield2jacobian_aff`] with the identity.
pub fn deffield2jacobian(
    dx: &mut SplineField,
    dy: &mut SplineField,
    dz: &mut SplineField,
    jac: &mut Volume,
) -> Result<(), FslError> {
    deffield2jacobian_aff(dx, dy, dz, &Mat::eye(4), jac)
}

/// `deffield2jacobian(dx, dy, dz, aff, jac)`: the Jacobian determinant of the mapping
/// `aff⁻¹ + displacement / voxel size` at every voxel of the fields (displacements in
/// millimetres along x, y, z), written into `jac` (`float`; each of the six terms added in
/// `double` to the `float` sum).
pub fn deffield2jacobian_aff(
    dx: &mut SplineField,
    dy: &mut SplineField,
    dz: &mut SplineField,
    aff: &Mat,
    jac: &mut Volume,
) -> Result<(), FslError> {
    same_fields(dx, dy, dz)?;
    update9(dx, dy, dz)?;
    let iaff = inv(aff)?;
    let a = |r: usize, c: usize| iaff.at(r - 1, c - 1);
    let (a11, a21, a31) = (a(1, 1), a(2, 1), a(3, 1));
    let (a12, a22, a32) = (a(1, 2), a(2, 2), a(3, 2));
    let (a13, a23, a33) = (a(1, 3), a(2, 3), a(3, 3));
    let (dxdx, dxdy, dxdz) = (
        g(dx, FieldIndex::Dfdx),
        g(dx, FieldIndex::Dfdy),
        g(dx, FieldIndex::Dfdz),
    );
    let (dydx, dydy, dydz) = (
        g(dy, FieldIndex::Dfdx),
        g(dy, FieldIndex::Dfdy),
        g(dy, FieldIndex::Dfdz),
    );
    let (dzdx, dzdy, dzdz) = (
        g(dz, FieldIndex::Dfdx),
        g(dz, FieldIndex::Dfdy),
        g(dz, FieldIndex::Dfdz),
    );
    let [xvx, xvy, xvz] = dx.voxel_size();
    let [yvx, yvy, yvz] = dy.voxel_size();
    let [zvx, zvy, zvz] = dz.voxel_size();
    let [sx, sy, sz] = dx.field_size();
    let mut indx = 0usize;
    for k in 0..i64::from(sz) {
        for j in 0..i64::from(sy) {
            for i in 0..i64::from(sx) {
                let e = |av: f64, vxs: f64, d: &[f64]| av + (1.0 / vxs) * d[indx];
                let t1 = e(a11, xvx, dxdx) * e(a22, yvy, dydy) * e(a33, zvz, dzdz);
                let t2 = e(a12, xvy, dxdy) * e(a23, yvz, dydz) * e(a31, zvx, dzdx);
                let t3 = e(a13, xvz, dxdz) * e(a21, yvx, dydx) * e(a32, zvy, dzdy);
                let t4 = e(a31, zvx, dzdx) * e(a22, yvy, dydy) * e(a13, xvz, dxdz);
                let t5 = e(a32, zvy, dzdy) * e(a23, yvz, dydz) * e(a11, xvx, dxdx);
                let t6 = e(a33, zvz, dzdz) * e(a21, yvx, dydx) * e(a12, xvy, dxdy);
                // `jac(i,j,k) = t1`, then `+=` / `-=` (each through operator(): the float read
                // back, the double term added, stored as float)
                let terms = [(t1, 0), (t2, 1), (t3, 1), (t4, -1), (t5, -1), (t6, -1)];
                for (t, op) in terms {
                    if let Some(w) = write_index(jac, i, j, k)? {
                        jac.data[w] = match op {
                            0 => t as f32,
                            1 => (f64::from(jac.data[w]) + t) as f32,
                            _ => (f64::from(jac.data[w]) - t) as f32,
                        };
                    }
                }
                indx += 1;
            }
        }
    }
    Ok(())
}

/// `deffield2jacobian(field, fi, jac)`: the Jacobian determinant of a displacement along x
/// (`fi` = [`FieldIndex::Dfdx`]) or y ([`FieldIndex::Dfdy`]) alone: `1 + d'/vxs`.
pub fn deffield2jacobian_1d(
    field: &mut SplineField,
    fi: FieldIndex,
    jac: &mut Volume,
) -> Result<(), FslError> {
    if fi != FieldIndex::Dfdx && fi != FieldIndex::Dfdy {
        return Err(fnirt_file_reader_error(
            "deffield2jacobian: fi must be 1 or 2",
        ));
    }
    field.update(fi)?;
    let vxs = if fi == FieldIndex::Dfdx {
        field.vxs_x()
    } else {
        field.vxs_y()
    };
    let df = field.field_cache(fi).expect("updated");
    let [sx, sy, sz] = field.field_size();
    let mut indx = 0usize;
    for k in 0..i64::from(sz) {
        for j in 0..i64::from(sy) {
            for i in 0..i64::from(sx) {
                if let Some(w) = write_index(jac, i, j, k)? {
                    jac.data[w] = (1.0 + (1.0 / vxs) * df[indx]) as f32;
                }
                indx += 1;
            }
        }
    }
    Ok(())
}

/// `deffield2jacobian_matrix(dx, dy, dz, [aff,] jacmat)`: the nine elements of the Jacobian
/// matrix (`jacmat` with nine volumes, in the order W_x/dx, W_y/dx, W_z/dx, W_x/dy, ...,
/// W_z/dz).
pub fn deffield2jacobian_matrix(
    dx: &mut SplineField,
    dy: &mut SplineField,
    dz: &mut SplineField,
    aff: &Mat,
    jacmat: &mut Volume,
) -> Result<(), FslError> {
    same_fields(dx, dy, dz)?;
    update9(dx, dy, dz)?;
    let iaff = inv(aff)?;
    let a = |r: usize, c: usize| iaff.at(r - 1, c - 1);
    let elems: [(f64, f64, &[f64]); 9] = [
        (a(1, 1), dx.vxs_x(), g(dx, FieldIndex::Dfdx)),
        (a(2, 1), dy.vxs_x(), g(dy, FieldIndex::Dfdx)),
        (a(3, 1), dz.vxs_x(), g(dz, FieldIndex::Dfdx)),
        (a(1, 2), dx.vxs_y(), g(dx, FieldIndex::Dfdy)),
        (a(2, 2), dy.vxs_y(), g(dy, FieldIndex::Dfdy)),
        (a(3, 2), dz.vxs_y(), g(dz, FieldIndex::Dfdy)),
        (a(1, 3), dx.vxs_z(), g(dx, FieldIndex::Dfdz)),
        (a(2, 3), dy.vxs_z(), g(dy, FieldIndex::Dfdz)),
        (a(3, 3), dz.vxs_z(), g(dz, FieldIndex::Dfdz)),
    ];
    if jacmat.tsize() < 9 {
        // FSL's 4D operator() throws "Out of Bounds (time index)" for the missing volumes.
        return Err(FslError::imthrow("Out of Bounds (time index)", 5));
    }
    let [sx, sy, sz] = dx.field_size();
    let nvox = jacmat.nvoxels() as usize;
    let mut indx = 0usize;
    for k in 0..i64::from(sz) {
        for j in 0..i64::from(sy) {
            for i in 0..i64::from(sx) {
                if let Some(w) = write_index(jacmat, i, j, k)? {
                    for (t, &(av, vxs, d)) in elems.iter().enumerate() {
                        jacmat.data[t * nvox + w] = (av + (1.0 / vxs) * d[indx]) as f32;
                    }
                }
                indx += 1;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_per_thread_is_fsls() {
        assert_eq!(rows_per_thread(10, 3), vec![0, 3, 6, 10]);
        assert_eq!(rows_per_thread(2, 4), vec![0, 0, 1, 1, 2]);
        assert_eq!(rows_per_thread(64, 1), vec![0, 64]);
    }
}
