// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Volume-to-volume resampling of `utils/mri2.cpp` (3.1-37's piece d): `MRIvol2Vol`,
//! `MRIvol2VolTLKernel`, `MRIvol2VolVSM`, `MRIvol2VolDelta` and `MRImaskAndUpsample`, with
//! `transform.cpp`'s `TransformRegDat2LTA` (3.1-40, taken early: `MRImaskAndUpsample` returns its
//! transform).
//!
//! Each target voxel `(ct, rt, st)` maps to the source point `Vt2s (ct, rt, st, 1)`: in
//! `MRIvol2Vol` and `MRIvol2VolTLKernel` each row is summed in `float` (`r1 ct + r2 rt + r3 st +
//! r4`, the products and sums rounded to `float`, the order of the official build's sums for NaN
//! operands: [`row`]); in `MRIvol2VolVSM` by `MatrixMultiply`. A point whose nearest voxel (`nint`
//! of each coordinate; `nint2` in `MRIvol2Vol` for a source one voxel thick in some direction) is
//! outside the source leaves the target voxel as it was.
//!
//! Defect (`larmorx_core::defects`; the findings in `docs/findings/freesurfer-volume.md` §15):
//! - **FS-482** (memory): `MRIvol2VolVSM`'s voxel shift is the last one the shift map's sampler
//!   wrote; a shift map of a type the sampler refuses (`MRIsampleSeqVolume: unsupported type`)
//!   leaves it as it was, at the first such voxel never written. Both modes take 0 there, noted.
//!
//! Cubic B-spline sampling (`SAMPLE_CUBIC_BSPLINE`) needs `fs.bspline` (wave 3.2), and a voxel-shift
//! transform that is not voxel-to-voxel needs `LTAchangeType` (3.1-39): port errors until they are
//! ported.

use std::io::Write;

use larmorx_core::defects::FS_482;
use larmorx_freesurfer_core::diag::DIAG_VERBOSE;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_UNSUPPORTED, Fatal};
use larmorx_freesurfer_core::matrix::{
    MATRIX_REAL, Matrix, matrix_alloc, matrix_copy, matrix_multiply, matrix_multiply_d,
};
use larmorx_freesurfer_core::matrix_ops::{matrix_identity, matrix_inverse, matrix_print};
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_core::sse::{add, mul, sub};
use larmorx_freesurfer_core::utils::{nint, nint2};
use larmorx_freesurfer_mri::lta::{LINEAR_VOX_TO_VOX, Lta, lta_alloc, lta_copy};
use larmorx_freesurfer_mri::mri::{MRI_FLOAT, Mri, mri_alloc_sequence, mri_copy_header};
use larmorx_freesurfer_mri::tkreg::{tkreg_mtx, xfm_crs2xyz_tkreg};
use larmorx_freesurfer_mri::vol_geom::{VolGeom, xfm_crs2xyz};
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};

use crate::arith::mri_extract_region;
use crate::coords::index_not_in_volume;
use crate::labels::int_of;
use crate::region::{region_get_bounding_box, region_print};
use crate::resize::{mri_upsample_n, mri_upsample_n_conserve};
use crate::sample::{
    SAMPLE_CUBIC_BSPLINE, SAMPLE_NEAREST, SAMPLE_SINC, SAMPLE_TRILINEAR, sampled_type,
    sinc_sample_volume,
};
use crate::seq::{sample_seq_volume, trilin_kernel};
use crate::vgeom::get_vol_geom;

/// How a row's three products are summed (the official build's operands; only NaN payloads
/// tell them apart): `(p1 + p2) + p3`, `p3 + (p1 + p2)` or `p3 + (p2 + p1)`, then `+ r4`.
#[derive(Clone, Copy)]
enum Sum {
    P12P3,
    P3P12,
    P3P21,
}

/// Row `r` of `Vt2s (ct, rt, st, 1)` in `float`.
fn row(m: &Matrix, r: i32, ct: i32, rt: i32, st: i32, sum: Sum) -> f32 {
    let p1 = mul(m.relt(r, 1), ct as f32);
    let p2 = mul(m.relt(r, 2), rt as f32);
    let p3 = mul(m.relt(r, 3), st as f32);
    let s = match sum {
        Sum::P12P3 => add(add(p1, p2), p3),
        Sum::P3P12 => add(p3, add(p1, p2)),
        Sum::P3P21 => add(p3, add(p2, p1)),
    };
    add(s, m.relt(r, 4))
}

/// `Vt2s` when the caller gave none: `inv(vox2ras(src)) * vox2ras(targ)` (both scanner
/// voxel-to-RAS matrices, `MatrixInverse` then `MatrixMultiply`).
fn vox2vox(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &VolGeom,
    targ: &VolGeom,
) -> Result<Option<Matrix>, Fatal> {
    let v2r_src = xfm_crs2xyz(g, out, err, src, 0, false)?;
    let inv = matrix_inverse(g, out, err, Some(&v2r_src), None)?.into_new();
    let v2r_targ = xfm_crs2xyz(g, out, err, targ, 0, false)?;
    Ok(matrix_multiply(g, out, err, inv.as_ref(), Some(&v2r_targ), None)?.into_new())
}

/// `MatrixPrint(stdout, m)` (`% 2.5f` elements).
fn print_matrix(g: &mut Globals, out: &mut dyn Write, err: &mut dyn Write, m: Option<&Matrix>) {
    let mut buf = Vec::new();
    matrix_print(
        g,
        Some(&mut buf as &mut dyn Write),
        &mut std::io::sink(),
        err,
        m,
        5,
    );
    let _ = out.write_all(&buf);
}

/// The port error for cubic B-spline sampling (`MRItoBSpline`: `fs.bspline`, wave 3.2).
fn no_bspline(g: &mut Globals, err: &mut dyn Write, what: &str) -> Result<(), Fatal> {
    log::error_exit(
        g,
        err,
        ERROR_UNSUPPORTED,
        &format!("{what}: cubic B-spline sampling needs fs.bspline (not yet ported)\n"),
    )
}

/// One target voxel's values sampled from the source at `(fcs, frs, fss)` (nearest voxel
/// `(ics, irs, iss)`) into `valvect`: trilinear for every frame at once, else frame by frame;
/// sinc samples frame 0 for every frame. Another method prints `ERROR: <who>: interpolation
/// method <m> unknown` and ends the program (`exit(1)`).
#[allow(clippy::too_many_arguments)]
fn sample_voxel(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    (fcs, frs, fss): (f32, f32, f32),
    (ics, irs, iss): (i32, i32, i32),
    interp: i32,
    sinchw: i32,
    valvect: &mut [f32],
    who: &str,
) -> Result<(), Fatal> {
    let (x, y, z) = (f64::from(fcs), f64::from(frs), f64::from(fss));
    if interp == SAMPLE_TRILINEAR {
        sample_seq_volume(g, err, src, x, y, z, valvect, 0, src.nframes - 1)?;
        return Ok(());
    }
    for f in 0..src.nframes {
        valvect[f as usize] = match interp {
            SAMPLE_NEAREST => mri_get_vox_val(g, err, src, ics, irs, iss, f, 0)?,
            SAMPLE_SINC => {
                let mut rval = 0.0f64;
                sinc_sample_volume(g, err, src, x, y, z, sinchw, &mut rval)?;
                rval as f32
            }
            _ => {
                let _ = writeln!(out, "ERROR: {who}: interpolation method {interp} unknown");
                return Err(Fatal::Exit(1));
            }
        };
    }
    Ok(())
}

/// `MRIvol2Vol(src, targ, Vt2s, InterpCode, param)`: each target voxel (columns outermost) gets
/// the source sampled at its point (module documentation): `SAMPLE_NEAREST` the nearest voxel's
/// frames, `SAMPLE_TRILINEAR` `MRIsampleSeqVolume`'s, `SAMPLE_SINC` `MRIsincSampleVolume` with
/// half-width `nint(param)` (frame 0, for every frame), stored with `MRIsetVoxVal`; the values a
/// voxel's sampler did not write are the previous voxel's (0 before any). `Vt2s` `None`: computed
/// from the two volumes' scanner voxel-to-RAS matrices. Different frame counts print `ERROR:
/// MRIvol2vol: source and target have different number of frames` and return 1; else 0. With
/// `Gdiag_no > 0` the matrix is printed first (`MRIvol2Vol: Vt2s Matrix (<computed>)`).
#[allow(clippy::too_many_arguments)]
pub fn mri_vol2vol(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    targ: &mut Mri,
    vt2s: Option<&Matrix>,
    interp: i32,
    param: f32,
) -> Result<i32, Fatal> {
    let thin = src.vg.width == 1 || src.vg.height == 1 || src.vg.depth == 1;
    let nintf = |g: &mut Globals, x: f32| {
        if thin {
            nint2(g, f64::from(x))
        } else {
            nint(g, f64::from(x))
        }
    };
    if src.nframes != targ.nframes {
        let _ = writeln!(
            out,
            "ERROR: MRIvol2vol: source and target have different number of frames"
        );
        return Ok(1);
    }
    let computed;
    let (m, free) = match vt2s {
        Some(m) => (Some(m), 0),
        None => {
            computed = vox2vox(g, out, err, &src.vg, &targ.vg)?;
            (computed.as_ref(), 1)
        }
    };
    if g.gdiag_no > 0 {
        let _ = writeln!(out, "MRIvol2Vol: Vt2s Matrix ({free})");
        print_matrix(g, out, err, m);
    }
    let sinchw = nint(g, f64::from(param));
    if interp == SAMPLE_CUBIC_BSPLINE {
        no_bspline(g, err, "MRIvol2Vol")?;
    }
    let mut valvect = vec![0.0f32; src.nframes.max(0) as usize];
    let Some(m) = m else {
        return no_matrix(g, err, "MRIvol2Vol");
    };
    for ct in 0..targ.vg.width {
        for rt in 0..targ.vg.height {
            for st in 0..targ.vg.depth {
                let fcs = row(m, 1, ct, rt, st, Sum::P12P3);
                let ics = nintf(g, fcs);
                if ics < 0 || ics >= src.vg.width {
                    continue;
                }
                let frs = row(m, 2, ct, rt, st, Sum::P12P3);
                let irs = nintf(g, frs);
                if irs < 0 || irs >= src.vg.height {
                    continue;
                }
                let fss = row(m, 3, ct, rt, st, Sum::P3P21);
                let iss = nintf(g, fss);
                if iss < 0 || iss >= src.vg.depth {
                    continue;
                }
                sample_voxel(
                    g,
                    out,
                    err,
                    src,
                    (fcs, frs, fss),
                    (ics, irs, iss),
                    interp,
                    sinchw,
                    &mut valvect,
                    "MRIvol2vol",
                )?;
                for f in 0..src.nframes {
                    mri_set_vox_val(g, err, targ, ct, rt, st, f, valvect[f as usize], 0)?;
                }
            }
        }
    }
    Ok(0)
}

/// FreeSurfer dereferences a matrix its products failed to make (a crash): a port error.
fn no_matrix(g: &mut Globals, err: &mut dyn Write, what: &str) -> Result<i32, Fatal> {
    log::error_exit(
        g,
        err,
        ERROR_UNSUPPORTED,
        &format!("{what}: no voxel-to-voxel matrix (FreeSurfer crashes here)\n"),
    )?;
    Ok(1)
}

/// `MRIvol2VolTLKernel(src, targ, Vt2s)`: for each target voxel (columns outermost) whose nearest
/// source voxel (`nint`) lies in the source, the eight trilinear weights of its source point
/// (`MRItrilinKernel`) go into the TARGET's frames 0 to 7 (`MRIsetVoxVal`, as `float`s): the
/// caller passes an 8-frame target. A work volume (8 `float` frames, the target's header) is made
/// and dropped; the result is always `NULL` (here `Ok(())`). `Vt2s` `None`: as
/// [`mri_vol2vol`]'s.
pub fn mri_vol2vol_tl_kernel(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    targ: &mut Mri,
    vt2s: Option<&Matrix>,
) -> Result<(), Fatal> {
    let mut kernel = mri_alloc_sequence(
        g,
        out,
        err,
        targ.vg.width,
        targ.vg.height,
        targ.vg.depth,
        MRI_FLOAT,
        8,
    )?;
    mri_copy_header(g, out, err, targ, Some(&mut kernel))?;
    let computed;
    let m = match vt2s {
        Some(m) => Some(m),
        None => {
            computed = vox2vox(g, out, err, &src.vg, &targ.vg)?;
            computed.as_ref()
        }
    };
    let Some(m) = m else {
        no_matrix(g, err, "MRIvol2VolTLKernel")?;
        return Ok(());
    };
    for ct in 0..targ.vg.width {
        for rt in 0..targ.vg.height {
            for st in 0..targ.vg.depth {
                let fcs = row(m, 1, ct, rt, st, Sum::P12P3);
                let ics = nint(g, f64::from(fcs));
                if ics < 0 || ics >= src.vg.width {
                    continue;
                }
                let frs = row(m, 2, ct, rt, st, Sum::P3P12);
                let irs = nint(g, f64::from(frs));
                if irs < 0 || irs >= src.vg.height {
                    continue;
                }
                let fss = row(m, 3, ct, rt, st, Sum::P12P3);
                let iss = nint(g, f64::from(fss));
                if iss < 0 || iss >= src.vg.depth {
                    continue;
                }
                let k = trilin_kernel(src, f64::from(fcs), f64::from(frs), f64::from(fss));
                for (f, w) in k.iter().enumerate() {
                    mri_set_vox_val(g, err, targ, ct, rt, st, f as i32, *w as f32, 0)?;
                }
            }
        }
    }
    Ok(())
}

/// `MRIvol2VolVSM(src, targ, Vt2s, InterpCode, param, vsm, vsmlta, pedir)`: [`mri_vol2vol`]'s
/// sampling (always `nint`; the target voxel's point `Vt2s (ct, rt, st, 1)` by `MatrixMultiply`;
/// one value vector for the whole call) with a voxel-shift map `vsm` in the source's voxels:
/// where the point's four in-plane neighbours (`floor` of the column and row, and the next ones,
/// all inside the source) have shifts of magnitude at least `FLT_MIN`, the trilinear shift
/// (`MRIsampleSeqVolume` of `vsm`'s frame 0; 0 skips the voxel) times the sign of `pedir` moves
/// the point along axis `|pedir|` (1 column, 2 row, 3 slice), then `vsmlta`'s voxel-to-voxel
/// matrix (`MatrixMultiplyD`) maps it, and its nearest voxel must again lie in the source.
/// Messages: `ERROR: MRIvol2volVSM(): source and target have different number of frames` (1);
/// `ERROR: MRIvol2volVSM: pedir=<p>, must be +/-1, +/-2, +/-3` with a map, then `exit(1)`; with
/// `DIAG_VERBOSE`, `Using MRIvol2VolVSM` and `MRIvol2VolVSM interp=<i>, param=<p>, pedir=<d>`
/// first; with `Gdiag_no > 0` the matrix as [`mri_vol2vol`]. 0 otherwise. FS-482.
#[allow(clippy::too_many_arguments)]
pub fn mri_vol2vol_vsm(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    src: &Mri,
    targ: &mut Mri,
    vt2s: Option<&Matrix>,
    interp: i32,
    param: f32,
    vsm: Option<&Mri>,
    vsmlta: Option<&Lta>,
    pedir: i32,
) -> Result<i32, Fatal> {
    if g.gdiag & DIAG_VERBOSE != 0 {
        let _ = writeln!(out, "Using MRIvol2VolVSM");
        let p = larmorx_core::cfmt::g(f64::from(param), 6);
        let _ = writeln!(
            out,
            "MRIvol2VolVSM interp={interp}, param={p}, pedir={pedir}"
        );
    }
    if src.nframes != targ.nframes {
        let _ = writeln!(
            out,
            "ERROR: MRIvol2volVSM(): source and target have different number of frames"
        );
        return Ok(1);
    }
    let mut vsm_v2v = None;
    if vsm.is_some() {
        let ap = pedir.wrapping_abs();
        if ap != 1 && ap != 2 && ap != 3 {
            let _ = writeln!(
                out,
                "ERROR: MRIvol2volVSM: pedir={pedir}, must be +/-1, +/-2, +/-3"
            );
            return Err(Fatal::Exit(1));
        }
        if let Some(l) = vsmlta {
            let copy = lta_copy(g, out, err, l, None)?
                .into_new()
                .expect("LTAcopy makes a new transform");
            if copy.ltype != LINEAR_VOX_TO_VOX {
                log::error_exit(
                    g,
                    err,
                    ERROR_UNSUPPORTED,
                    "MRIvol2VolVSM: a voxel-shift transform that is not voxel-to-voxel needs LTAchangeType (3.1-39, not yet ported)\n",
                )?;
            }
            let m = copy.xforms.first().and_then(|x| x.m_l.as_ref());
            vsm_v2v = matrix_copy(g, out, err, m, None)?.into_new();
        }
    }
    let computed;
    let (m, free) = match vt2s {
        Some(m) => (Some(m), 0),
        None => {
            computed = vox2vox(g, out, err, &src.vg, &targ.vg)?;
            (computed.as_ref(), 1)
        }
    };
    if g.gdiag_no > 0 {
        let _ = writeln!(out, "MRIvol2Vol: Vt2s Matrix ({free})");
        print_matrix(g, out, err, m);
    }
    let sinchw = nint(g, f64::from(param));
    let mut valvect = vec![0.0f32; src.nframes.max(0) as usize];
    if interp == SAMPLE_CUBIC_BSPLINE {
        no_bspline(g, err, "MRIvol2VolVSM")?;
    }
    let Some(m) = m else {
        return no_matrix(g, err, "MRIvol2VolVSM");
    };
    let mut crs_t = matrix_alloc(g, out, err, 4, 1, MATRIX_REAL)?.expect("a 4 x 1 vector");
    crs_t.set_relt(4, 1, 1.0);
    let mut crs_s = matrix_alloc(g, out, err, 4, 1, MATRIX_REAL)?.expect("a 4 x 1 vector");
    // the voxel shift: FreeSurfer's `float dvsm`, kept from voxel to voxel (None: never written)
    let mut dvsm: Option<f32> = None;
    let (w, h, d) = (src.vg.width, src.vg.height, src.vg.depth);
    let tiny = f64::from(f32::MIN_POSITIVE);
    for ct in 0..targ.vg.width {
        for rt in 0..targ.vg.height {
            for st in 0..targ.vg.depth {
                crs_t.set_relt(1, 1, ct as f32);
                crs_t.set_relt(2, 1, rt as f32);
                crs_t.set_relt(3, 1, st as f32);
                if matrix_multiply(g, out, err, Some(m), Some(&crs_t), Some(&mut crs_s))?.is_null()
                {
                    no_matrix(g, err, "MRIvol2VolVSM")?;
                    return Ok(1);
                }
                let (mut fcs, mut frs, mut fss) =
                    (crs_s.relt(1, 1), crs_s.relt(2, 1), crs_s.relt(3, 1));
                let mut ics = nint(g, f64::from(fcs));
                let mut irs = nint(g, f64::from(frs));
                let mut iss = nint(g, f64::from(fss));
                if ics < 0 || ics >= w || irs < 0 || irs >= h || iss < 0 || iss >= d {
                    continue;
                }
                if let Some(vsm) = vsm {
                    const WHAT: &str = "MRIvol2VolVSM";
                    let cvsm = int_of(g, WHAT, f64::from(fcs.floor()));
                    let rvsm = int_of(g, WHAT, f64::from(frs.floor()));
                    if cvsm < 0 || cvsm.wrapping_add(1) >= w {
                        continue;
                    }
                    if rvsm < 0 || rvsm.wrapping_add(1) >= h {
                        continue;
                    }
                    let mut skip = false;
                    for (c, r) in [
                        (cvsm, rvsm),
                        (cvsm + 1, rvsm),
                        (cvsm, rvsm + 1),
                        (cvsm + 1, rvsm + 1),
                    ] {
                        let v = mri_get_vox_val(g, err, vsm, c, r, iss, 0, 0)?;
                        if f64::from(v).abs() < tiny {
                            skip = true;
                            break;
                        }
                    }
                    if skip {
                        continue;
                    }
                    let mut one = [dvsm.unwrap_or(0.0)];
                    let (x, y, z) = (f64::from(fcs), f64::from(frs), f64::from(fss));
                    let written = sampled_type(vsm.mtype) || index_not_in_volume(vsm, x, y, z) == 1;
                    sample_seq_volume(
                        g,
                        err,
                        vsm,
                        f64::from(fcs),
                        f64::from(frs),
                        f64::from(fss),
                        &mut one,
                        0,
                        0,
                    )?;
                    if dvsm.is_none() && !written {
                        g.notices.add(
                            &FS_482,
                            "MRIvol2VolVSM uses a voxel shift its sampler never wrote (memory contents in FreeSurfer)",
                        );
                    }
                    if written || dvsm.is_some() {
                        dvsm = Some(one[0]);
                    }
                    let shift = one[0];
                    if shift == 0.0 {
                        continue;
                    }
                    let sign = if pedir < 0 { -1.0f32 } else { 1.0f32 };
                    let delta = mul(sign, shift);
                    match pedir.wrapping_abs() {
                        1 => fcs = add(fcs, delta),
                        2 => frs = add(frs, delta),
                        _ => fss = add(fss, delta),
                    }
                    if let Some(v2v) = vsm_v2v.as_ref() {
                        let mut p = Matrix::real(4, 1);
                        p.set_relt(1, 1, fcs);
                        p.set_relt(2, 1, frs);
                        p.set_relt(3, 1, fss);
                        p.set_relt(4, 1, 1.0);
                        let Some(q) =
                            matrix_multiply_d(g, out, err, Some(v2v), Some(&p), None)?.into_new()
                        else {
                            no_matrix(g, err, WHAT)?;
                            return Ok(1);
                        };
                        (fcs, frs, fss) = (q.relt(1, 1), q.relt(2, 1), q.relt(3, 1));
                    }
                    ics = nint(g, f64::from(fcs));
                    if ics < 0 || ics >= w {
                        continue;
                    }
                    irs = nint(g, f64::from(frs));
                    if irs < 0 || irs >= h {
                        continue;
                    }
                    iss = nint(g, f64::from(fss));
                    if iss < 0 || iss >= d {
                        continue;
                    }
                }
                sample_voxel(
                    g,
                    out,
                    err,
                    src,
                    (fcs, frs, fss),
                    (ics, irs, iss),
                    interp,
                    sinchw,
                    &mut valvect,
                    "MRIvol2volVSM",
                )?;
                for f in 0..src.nframes {
                    mri_set_vox_val(g, err, targ, ct, rt, st, f, valvect[f as usize], 0)?;
                }
            }
        }
    }
    Ok(0)
}

/// `MRIvol2VolDelta(mov, targ, Rt2s)`: a new 3-frame `float` volume with the target's extent and
/// header whose frames at each target voxel hold `tkRAS(targ) - Rt2s tkRAS(targ)` (the target's
/// tkregister RAS of the voxel minus its image under the RAS-to-RAS `Rt2s`; each product
/// `MatrixMultiply`'s, the difference in `float`). `Rt2s` `None`: `MRItkRegMtx(targ, mov)`.
pub fn mri_vol2vol_delta(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mov: &Mri,
    targ: &Mri,
    rt2s: Option<&Matrix>,
) -> Result<Option<Mri>, Fatal> {
    let mut delta = mri_alloc_sequence(
        g,
        out,
        err,
        targ.vg.width,
        targ.vg.height,
        targ.vg.depth,
        MRI_FLOAT,
        3,
    )?;
    mri_copy_header(g, out, err, targ, Some(&mut delta))?;
    let computed;
    let r = match rt2s {
        Some(r) => r,
        None => {
            computed = tkreg_mtx(g, out, err, &targ.vg, &mov.vg, None)?;
            &computed
        }
    };
    let t_v2r = xfm_crs2xyz_tkreg(g, out, err, &targ.vg)?;
    let t_v2m = matrix_multiply(g, out, err, Some(r), Some(&t_v2r), None)?.into_new();
    let mut crs = matrix_alloc(g, out, err, 4, 1, MATRIX_REAL)?.expect("a 4 x 1 vector");
    crs.set_relt(4, 1, 1.0);
    let mut t_ras: Option<Matrix> = None;
    let mut m_ras: Option<Matrix> = None;
    for ct in 0..targ.vg.width {
        for rt in 0..targ.vg.height {
            for st in 0..targ.vg.depth {
                crs.set_relt(1, 1, ct as f32);
                crs.set_relt(2, 1, rt as f32);
                crs.set_relt(3, 1, st as f32);
                product_into(g, out, err, Some(&t_v2r), &crs, &mut t_ras)?;
                product_into(g, out, err, t_v2m.as_ref(), &crs, &mut m_ras)?;
                let (Some(a), Some(b)) = (t_ras.as_ref(), m_ras.as_ref()) else {
                    no_matrix(g, err, "MRIvol2VolDelta")?;
                    return Ok(None);
                };
                for k in 1..=3 {
                    let dv = sub(a.relt(k, 1), b.relt(k, 1));
                    mri_set_vox_val(g, err, &mut delta, ct, rt, st, k - 1, dv, 0)?;
                }
            }
        }
    }
    Ok(Some(delta))
}

/// `dst = MatrixMultiply(a, b, dst)`: a new product the first time, then into it.
fn product_into(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    a: Option<&Matrix>,
    b: &Matrix,
    dst: &mut Option<Matrix>,
) -> Result<(), Fatal> {
    match dst.as_mut() {
        Some(d) => {
            if matrix_multiply(g, out, err, a, Some(b), Some(d))?.is_null() {
                *dst = None;
            }
        }
        None => *dst = matrix_multiply(g, out, err, a, Some(b), None)?.into_new(),
    }
    Ok(())
}

/// `TransformRegDat2LTA(targ, mov, R)` (`transform.cpp`, 3.1-40, taken early): a one-transform
/// `LINEAR_VOX_TO_VOX` LTA from the target's voxels to the moving volume's, `inv(Tmov) R Ttarg`
/// (`MatrixMultiply` twice, left to right), its source geometry the target's and its destination
/// the moving volume's. `R` `None`: the identity between the scanner voxel-to-RAS matrices;
/// given: between the tkregister ones. `None` when a product fails (FreeSurfer then copies a
/// `NULL` matrix: a crash).
pub fn transform_reg_dat2_lta(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    targ: &VolGeom,
    mov: &VolGeom,
    r: Option<&Matrix>,
) -> Result<Option<Lta>, Fatal> {
    let identity;
    let (t_targ, t_mov, r) = match r {
        None => {
            let a = xfm_crs2xyz(g, out, err, targ, 0, false)?;
            let b = xfm_crs2xyz(g, out, err, mov, 0, false)?;
            identity = matrix_identity(g, out, err, 4, None)?
                .into_new()
                .expect("a 4 x 4 identity");
            (a, b, &identity)
        }
        Some(r) => (
            xfm_crs2xyz_tkreg(g, out, err, targ)?,
            xfm_crs2xyz_tkreg(g, out, err, mov)?,
            r,
        ),
    };
    let inv = matrix_inverse(g, out, err, Some(&t_mov), None)?.into_new();
    let Some(v1) = matrix_multiply(g, out, err, inv.as_ref(), Some(r), None)?.into_new() else {
        return Ok(None);
    };
    let Some(v2v) = matrix_multiply(g, out, err, Some(&v1), Some(&t_targ), None)?.into_new() else {
        return Ok(None);
    };
    let mut lta = lta_alloc(g, out, err, 1, None)?;
    lta.ltype = LINEAR_VOX_TO_VOX;
    let x = &mut lta.xforms[0];
    x.ltype = LINEAR_VOX_TO_VOX;
    get_vol_geom(targ, &mut x.src);
    get_vol_geom(mov, &mut x.dst);
    x.m_l = matrix_copy(g, out, err, Some(&v2v), None)?.into_new();
    Ok(Some(lta))
}

/// `MRImaskAndUpsample(src, mask, UpsampleFactor, nPad, DoConserve, &src2out)`: the bounding box
/// of the mask's (else the source's) nonzero voxels of frame 0, padded by `nPad`
/// (`REGIONgetBoundingBox`), cut out of the source (`MRIextractRegion`; `None` when that fails),
/// upsampled by the factor when it is above 1 (`MRIupsampleNConserve` with `DoConserve`, else
/// `MRIupsampleN`), and the voxel-to-voxel LTA from the source to the result
/// (`TransformRegDat2LTA(src, result, NULL)`). With `Gdiag_no > 0`, ` mask-and-upsample bounding
/// box <ratio> ` and the box (`REGIONprint`) on standard output first.
#[allow(clippy::too_many_arguments)]
pub fn mri_mask_and_upsample(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    src: &mut Mri,
    mask: Option<&Mri>,
    upsample: i32,
    npad: i32,
    conserve: bool,
) -> Result<Option<(Mri, Option<Lta>)>, Fatal> {
    let region = match mask {
        Some(m) => region_get_bounding_box(g, err, m, npad)?,
        None => region_get_bounding_box(g, err, src, npad)?,
    };
    if g.gdiag_no > 0 {
        let n = (src.vg.width as f32) * (src.vg.height as f32) * (src.vg.depth as f32);
        let v = region.dx.wrapping_mul(region.dy).wrapping_mul(region.dz) as f32;
        let ratio = n / v;
        let _ = write!(
            out,
            " mask-and-upsample bounding box {} ",
            larmorx_core::cfmt::g(f64::from(ratio), 6)
        );
        region_print(out, &region);
    }
    let Some(mut srcmask) = mri_extract_region(g, out, err, src, None, Some(&region))?.into_new()
    else {
        return Ok(None);
    };
    let srcus = if upsample > 1 {
        let r = if conserve {
            mri_upsample_n_conserve(g, out, err, env, &mut srcmask, None, upsample)?
        } else {
            mri_upsample_n(g, out, err, &srcmask, None, upsample)?
        };
        match r.into_new() {
            Some(m) => m,
            None => {
                log::error_exit(
                    g,
                    err,
                    ERROR_UNSUPPORTED,
                    "MRImaskAndUpsample: no upsampled volume (FreeSurfer crashes here)\n",
                )?;
                return Ok(None);
            }
        }
    } else {
        srcmask
    };
    let lta = transform_reg_dat2_lta(g, out, err, &src.vg, &srcus.vg, None)?;
    Ok(Some((srcus, lta)))
}

/// `MRIvol2VolTkRegVSM(mov, targ, Rtkreg, InterpCode, param, vsm, vsmlta, pedir)`:
/// [`mri_vol2vol_vsm`] with the vox2vox `inv(Tmov) Rtkreg Ttarg` (`T` the tkregister vox2ras of
/// each; `MatrixMultiply`), or none without a registration.
#[allow(clippy::too_many_arguments)]
pub fn mri_vol2vol_tkreg_vsm(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mov: &Mri,
    targ: &mut Mri,
    rtkreg: Option<&Matrix>,
    interp: i32,
    param: f32,
    vsm: Option<&Mri>,
    vsmlta: Option<&Lta>,
    pedir: i32,
) -> Result<i32, Fatal> {
    let vox2vox = match rtkreg {
        Some(r) => {
            let tmov = xfm_crs2xyz_tkreg(g, out, err, &mov.vg)?;
            let inv_tmov = matrix_inverse(g, out, err, Some(&tmov), None)?.into_new();
            let ttarg = xfm_crs2xyz_tkreg(g, out, err, &targ.vg)?;
            let mut v = matrix_multiply(g, out, err, inv_tmov.as_ref(), Some(r), None)?.into_new();
            // MatrixMultiply(vox2vox, Ttarg, vox2vox): FreeSurfer copies the input it writes into
            match v.as_mut() {
                Some(m) => {
                    let c = m.clone();
                    matrix_multiply(g, out, err, Some(&c), Some(&ttarg), Some(m))?;
                }
                None => {
                    matrix_multiply(g, out, err, None, Some(&ttarg), None)?;
                }
            }
            v
        }
        None => None,
    };
    mri_vol2vol_vsm(
        g,
        out,
        err,
        mov,
        targ,
        vox2vox.as_ref(),
        interp,
        param,
        vsm,
        vsmlta,
        pedir,
    )
}
