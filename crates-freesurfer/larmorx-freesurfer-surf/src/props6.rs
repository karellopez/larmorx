// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Queue item 3.1-51, the rest (relay 20), second group: the second fundamental form of
//! `utils/mrisurf_metricProperties.cpp` (lines 10423-10866): `MRIScomputeSecondFundamentalForm`
//! and `MRIScomputeSecondFundamentalFormThresholded` (mris_curvature's principal curvatures, with
//! its outlier removal), each vertex's quadratic fit `z = A u² + 2 B u v + C v²` to its neighbours
//! in the tangent plane solved by fs.maths' matrices, the principal curvatures the eigenvalues of
//! its Hessian.
//!
//! Arithmetic through fs.core's `sse` operations in the official build's operand order (the unit
//! oracle's object code; `docs/findings/freesurfer-surfio.md` §22). Defects
//! (`larmorx_core::defects`):
//! - **FS-377** (crash): the outlier removal (a threshold of 0 or more) of a surface whose
//!   unripped `k1` (or `k2`) values are all equal, or one of which is not finite, writes far
//!   outside its 1,000-bin histogram. The default leaves such a vertex out of the histogram
//!   (noted); `reproduce` ends as FreeSurfer's crash (exit status 139).
//! - **FS-378** (memory): the outlier removal of a surface without unripped vertices reads the
//!   histogram one place before its start (no effect on the result: no vertex is clipped).
//!   Both modes skip the read (noted).

use std::io::Write;
use std::path::Path;

use larmorx_core::defects::{FS_377, FS_378};
use larmorx_freesurfer_core::cstd::fmt_f;
use larmorx_freesurfer_core::diag::{DIAG_SHOW, DIAG_WRITE, verbose_on};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::histo::{
    Histogram, histo_alloc, histo_find_highest_peak_in_region, histo_make_pdf_in_place, histo_plot,
};
use larmorx_freesurfer_core::log::{Fatal, NO_ERROR};
use larmorx_freesurfer_core::matrix::{
    MATRIX_REAL, Matrix, matrix_alloc, matrix_alloc2, matrix_multiply,
};
use larmorx_freesurfer_core::matrix_decomp::{
    matrix_condition_number, matrix_eigen_system, matrix_svd_eigen_values, matrix_svd_inverse,
};
use larmorx_freesurfer_core::matrix_ops::{matrix_identity, matrix_transpose};
use larmorx_freesurfer_core::sse::{add, add64, div, div64, mul, mul64, sub, sub64};
use larmorx_freesurfer_core::utils::nint;

use crate::aaa::{MrisStatus, SurfGlobals};
use crate::coords::Io;
use crate::mris::Mris;
use crate::props::{compute_tangent_planes, compute_total_vertex_spacing_stats};

fn nverts(mris: &Mris) -> usize {
    usize::try_from(mris.nvertices).unwrap_or(0)
}

/// `ILL_CONDITIONED` (`mrisurf_base.h`).
const ILL_CONDITIONED: f32 = 500000.0;

fn alloc(g: &mut Globals, io: &mut Io, rows: i32, cols: i32) -> Result<Matrix, Fatal> {
    Ok(matrix_alloc(g, io.out, io.err, rows, cols, MATRIX_REAL)?.expect("a matrix"))
}

fn alloc2(g: &mut Globals, io: &mut Io, rows: i32, cols: i32) -> Result<Matrix, Fatal> {
    Ok(matrix_alloc2(g, io.out, io.err, rows, cols, MATRIX_REAL)?.expect("a matrix"))
}

/// `MRIScomputeSecondFundamentalForm(mris)`: [`compute_second_fundamental_form_thresholded`]
/// with no outlier removal (-1).
pub fn compute_second_fundamental_form(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &mut Mris,
) -> Result<i32, Fatal> {
    compute_second_fundamental_form_thresholded(g, sg, io, dir, mris, -1.0)
}

/// `MRIScomputeSecondFundamentalFormThresholded(mris, pct_thresh)`: nothing for a plane
/// (`MRIS_PLANE`). With `pct_thresh` 0 or more, the vertex spacing's mean first
/// (`MRIScomputeTotalVertexSpacingStats`, which recomputes the metric properties) gives the squared
/// tangent-plane distance a neighbour needs (`min(mean / 4, 1)²`; 0 otherwise). The tangent planes
/// are recomputed (`mrisComputeTangentPlanes`); `Kmin`, `Hmin` start at 10000, `Kmax`, `Hmax` at
/// -10000, `Ktotal` at 0. For each unripped vertex with neighbours (`vtotal`): each unripped
/// neighbour's offset `y` gives `u = e1 . y`, `v = e2 . y` (`float`, then `double`), the row
/// `u², 2 u v, v²` (rounded to `float`) and the height `n . y`, written at the next free row of a
/// `vtotal` x 3 matrix whatever the neighbour's distance, the row taken only when `u² + v²` (as
/// `float`) is `FLT_EPSILON` or more and above the threshold (its `height / (u² + v²)` kept for
/// the extremes `kmin`, `kmax`); with fewer than 4 rows taken the pass repeats with a quarter of
/// the threshold (at most 102 passes). The fit `c = (Uᵀ U)⁻¹ Uᵀ z` (fs.maths' `float` matrices,
/// `MatrixSVDInverse`) gives the Hessian `[[2 c1, 2 c2], [2 c2, 2 c3]]`; a singular `Uᵀ U` gives
/// curvatures 0 and the tangent plane kept; a condition number of 500000 or more gives `k1 = kmax`,
/// `k2 = kmin` (nothing else changes); else the Hessian's eigenvalues (`MatrixEigenSystem`, or
/// `MatrixSVDEigenValues` when it fails, nothing else changing then) are `k1`, `k2`, with `K = k1
/// k2`, `H = (k1 + k2) / 2`, the extremes, `Ktotal += k1 k2 area` (`double`) and the tangent
/// plane turned to the eigenvectors. With `DIAG_WRITE` and `DIAG_VERBOSE` each such vertex's
/// `%d %f %f %f %f` line goes to `curv.dat` (in `dir`); with `DIAG_SHOW` the vertex `Gdiag_no`'s
/// `v %d: k1=%2.3f, k2=%2.3f, K=%2.3f, H=%2.3f`, with `DIAG_VERBOSE` too `max H error=-1.000 at -1`
/// and `total area = %2.3f`, and `%d ill-conditioned points` when some were (standard output).
///
/// With `pct_thresh` 0 or more the outliers are removed: a 1,000-bin histogram of each principal
/// curvature over its unripped range (made a PDF; `k1.plt`, `k2.plt` with `DIAG_WRITE` and
/// `DIAG_VERBOSE`), from its highest peak upwards and downwards the bin where the running sum
/// first passes `pct_thresh` of that side's total gives the limits; each unripped vertex's `k1`,
/// `k2` clipped to them, `K`, `H` and the extremes recomputed, and `%d vertices thresholded to be
/// in k1 ~ [%2.2f %2.2f], k2 ~ [%2.2f %2.2f]` on standard error when any was (FS-377, FS-378).
pub fn compute_second_fundamental_form_thresholded(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &mut Mris,
    pct_thresh: f64,
) -> Result<i32, Fatal> {
    if mris.status == MrisStatus::PLANE {
        return Ok(NO_ERROR);
    }
    let orig_rsq_thresh = if pct_thresh >= 0.0 {
        let vmean = compute_total_vertex_spacing_stats(g, sg, io, mris)?.mean;
        let x = mul64(vmean, 0.25);
        let m = if x < 1.0 { x } else { 1.0 };
        mul64(m, m)
    } else {
        0.0
    };
    compute_tangent_planes(g, io, mris);
    let mut v_c = alloc(g, io, 3, 1)?;
    let mut m_q = alloc(g, io, 2, 2)?;
    let mut m_eigen = alloc(g, io, 2, 2)?;
    mris.Kmin = 10000.0;
    mris.Hmin = 10000.0;
    mris.Kmax = -10000.0;
    mris.Hmax = -10000.0;
    mris.Ktotal = 0.0;
    let mut fp = if g.gdiag & DIAG_WRITE != 0 && verbose_on(g) {
        let path = match dir {
            Some(d) => d.join("curv.dat"),
            None => "curv.dat".into(),
        };
        std::fs::File::create(path).ok()
    } else {
        None
    };
    let mut nbad = 0;
    let mut total_area = 0.0f64;
    for vno in 0..nverts(mris) {
        if mris.vertex(vno).ripflag != 0 {
            continue;
        }
        let v = mris.vertex(vno);
        let nrm = [v.nx, v.ny, v.nz];
        let e1 = [v.e1x, v.e1y, v.e1z];
        let e2 = [v.e2x, v.e2y, v.e2z];
        let p = [v.x, v.y, v.z];
        let vt = mris.topology(vno);
        let vtotal = i32::from(vt.vtotal);
        if vtotal <= 0 {
            continue;
        }
        let nbrs: Vec<usize> = vt.v.as_deref().unwrap_or(&[])[..vtotal as usize]
            .iter()
            .map(|&x| x as usize)
            .collect();
        let mut m_u = alloc(g, io, vtotal, 3)?;
        let mut v_z = alloc(g, io, vtotal, 1)?;
        let mut rsq_thresh = orig_rsq_thresh;
        let mut niter = 0;
        let (mut kmin, mut kmax);
        loop {
            kmin = 10000.0f32;
            kmax = -10000.0f32;
            let mut n = 0i32;
            for &nb in &nbrs {
                let vn = mris.vertex(nb);
                if vn.ripflag != 0 {
                    continue;
                }
                let y = [sub(vn.x, p[0]), sub(vn.y, p[1]), sub(vn.z, p[2])];
                let uf = add(add(mul(e1[0], y[0]), mul(e1[1], y[1])), mul(e1[2], y[2]));
                let vf = add(add(mul(y[1], e2[1]), mul(y[0], e2[0])), mul(y[2], e2[2]));
                let (ui, vi) = (f64::from(uf), f64::from(vf));
                let uu = mul64(ui, ui);
                let vv = mul64(vi, vi);
                m_u.set_relt(n + 1, 1, uu as f32);
                m_u.set_relt(n + 1, 2, mul64(add64(ui, ui), vi) as f32);
                m_u.set_relt(n + 1, 3, vv as f32);
                let z = add(add(mul(nrm[0], y[0]), mul(nrm[1], y[1])), mul(nrm[2], y[2]));
                v_z.set_relt(n + 1, 1, z);
                let rsq = add64(vv, uu) as f32;
                if !(rsq < f32::EPSILON) && f64::from(rsq) > rsq_thresh {
                    let k = div(z, rsq);
                    if k > kmax {
                        kmax = k;
                    }
                    if k < kmin {
                        kmin = k;
                    }
                    n += 1;
                }
            }
            rsq_thresh = mul64(0.25, rsq_thresh);
            let last = niter > 100;
            niter += 1;
            if last || n >= 4 {
                break;
            }
        }
        let mut m_ut = alloc2(g, io, m_u.cols, m_u.rows)?;
        matrix_transpose(g, io.out, io.err, &m_u, Some(&mut m_ut))?;
        let mut m_tmp2 = alloc2(g, io, m_ut.rows, m_u.cols)?;
        matrix_multiply(
            g,
            io.out,
            io.err,
            Some(&m_ut),
            Some(&m_u),
            Some(&mut m_tmp2),
        )?;
        let cond_no = matrix_condition_number(g, io.out, io.err, &m_tmp2)?;
        let mut m_inverse = alloc2(g, io, m_tmp2.cols, m_tmp2.rows)?;
        let inv = matrix_svd_inverse(g, io.out, io.err, &m_tmp2, Some(&mut m_inverse))?;
        let mut evalues = [0.0f32; 3];
        if inv.is_null() {
            nbad += 1;
            let rows = m_eigen.rows;
            matrix_identity(g, io.out, io.err, rows, Some(&mut m_eigen))?;
        } else {
            let mut m_tmp1 = alloc2(g, io, m_ut.rows, v_z.cols)?;
            matrix_multiply(
                g,
                io.out,
                io.err,
                Some(&m_ut),
                Some(&v_z),
                Some(&mut m_tmp1),
            )?;
            matrix_multiply(
                g,
                io.out,
                io.err,
                Some(&m_inverse),
                Some(&m_tmp1),
                Some(&mut v_c),
            )?;
            let (c1, c2, c3) = (v_c.relt(1, 1), v_c.relt(2, 1), v_c.relt(3, 1));
            m_q.set_relt(1, 1, add(c1, c1));
            let q12 = add(c2, c2);
            m_q.set_relt(2, 1, q12);
            m_q.set_relt(1, 2, q12);
            m_q.set_relt(2, 2, add(c3, c3));
            if cond_no >= ILL_CONDITIONED {
                set_curvatures(mris, vno, kmax, kmin);
                continue;
            }
            let r = matrix_eigen_system(g, io.out, io.err, &m_q, &mut evalues, Some(&mut m_eigen))?;
            if r.is_null() {
                nbad += 1;
                matrix_svd_eigen_values(g, io.out, io.err, &m_q, &mut evalues)?;
                set_curvatures(mris, vno, evalues[0], evalues[1]);
                continue;
            }
        }
        let (k1, k2) = (evalues[0], evalues[1]);
        set_curvatures(mris, vno, k1, k2);
        let v = mris.vertex(vno);
        let (kk, hh, area) = (v.K, v.H, v.area);
        if let Some(f) = fp.as_mut() {
            let _ = writeln!(
                f,
                "{vno} {} {} {} {}",
                fmt_f(f64::from(k1), 0, 6),
                fmt_f(f64::from(k2), 0, 6),
                fmt_f(f64::from(kk), 0, 6),
                fmt_f(f64::from(hh), 0, 6)
            );
        }
        if i64::try_from(vno).ok() == Some(i64::from(g.gdiag_no)) && g.gdiag & DIAG_SHOW != 0 {
            let _ = writeln!(
                io.out,
                "v {vno}: k1={}, k2={}, K={}, H={}",
                fmt_f(f64::from(k1), 2, 3),
                fmt_f(f64::from(k2), 2, 3),
                fmt_f(f64::from(kk), 2, 3),
                fmt_f(f64::from(hh), 2, 3)
            );
        }
        if kk < mris.Kmin {
            mris.Kmin = kk;
        }
        if hh < mris.Hmin {
            mris.Hmin = hh;
        }
        if kk > mris.Kmax {
            mris.Kmax = kk;
        }
        if hh > mris.Hmax {
            mris.Hmax = hh;
        }
        let area = f64::from(area);
        mris.Ktotal = add64(
            mul64(mul64(f64::from(k1), f64::from(k2)), area),
            mris.Ktotal,
        );
        total_area = add64(area, total_area);
        let (a11, a12) = (m_eigen.relt(1, 1), m_eigen.relt(1, 2));
        let (a21, a22) = (m_eigen.relt(2, 1), m_eigen.relt(2, 2));
        let v = mris.vertex_mut(vno);
        v.e1x = add(mul(e2[0], a21), mul(e1[0], a11));
        v.e1y = add(mul(e1[1], a11), mul(e2[1], a21));
        v.e1z = add(mul(a21, e2[2]), mul(a11, e1[2]));
        v.e2x = add(mul(e1[0], a12), mul(e2[0], a22));
        v.e2y = add(mul(e1[1], a12), mul(e2[1], a22));
        v.e2z = add(mul(a12, e1[2]), mul(a22, e2[2]));
    }
    drop(fp);
    let show = g.gdiag & DIAG_SHOW != 0;
    if show && verbose_on(g) {
        let _ = writeln!(io.out, "max H error=-1.000 at -1");
        let _ = writeln!(io.out, "total area = {}", fmt_f(total_area, 2, 3));
    }
    if show && nbad > 0 {
        let _ = writeln!(io.out, "{nbad} ill-conditioned points");
    }
    if pct_thresh < 0.0 {
        return Ok(NO_ERROR);
    }
    remove_outliers(g, io, dir, mris, pct_thresh)?;
    Ok(NO_ERROR)
}

/// A vertex's principal curvatures `k1`, `k2`, with `K = k1 k2` and `H = (k1 + k2) / 2`.
fn set_curvatures(mris: &mut Mris, vno: usize, k1: f32, k2: f32) {
    let v = mris.vertex_mut(vno);
    v.k1 = k1;
    v.k2 = k2;
    v.K = mul(k1, k2);
    v.H = mul(add(k1, k2), 0.5);
}

/// The limits from a histogram's highest peak `zbin` (FreeSurfer's loops): upwards, the bin where
/// the running sum first passes `pct` of the total from `zbin` up (the last but one bin at most);
/// downwards likewise (bin 0 at least).
fn limits(h: &Histogram, zbin: i32, pct: f64) -> (f64, f64) {
    let nbins = h.nbins;
    let count = |b: i32| f64::from(h.counts[b as usize]);
    let mut total = 0.0f64;
    let mut bin = zbin;
    while bin < nbins {
        total = add64(total, count(bin));
        bin += 1;
    }
    let thresh = mul64(total, pct);
    total = 0.0;
    bin = zbin;
    while bin < nbins - 1 {
        total = add64(total, count(bin));
        if total > thresh {
            break;
        }
        bin += 1;
    }
    let max = f64::from(h.bins[bin as usize]);
    total = 0.0;
    bin = zbin;
    while bin > 0 {
        total = add64(total, count(bin));
        bin -= 1;
    }
    let thresh = mul64(total, pct);
    total = 0.0;
    bin = zbin;
    while bin > 0 {
        total = add64(total, count(bin));
        if total > thresh {
            break;
        }
        bin -= 1;
    }
    (f64::from(h.bins[bin as usize]), max)
}

/// The outlier removal of [`compute_second_fundamental_form_thresholded`].
fn remove_outliers(
    g: &mut Globals,
    io: &mut Io,
    dir: Option<&Path>,
    mris: &mut Mris,
    pct: f64,
) -> Result<(), Fatal> {
    let (mut min_k1, mut min_k2) = (1000.0f64, 1000.0f64);
    let (mut max_k1, mut max_k2) = (-1000.0f64, -1000.0f64);
    for vno in 0..nverts(mris) {
        let v = mris.vertex(vno);
        if v.ripflag != 0 {
            continue;
        }
        let (k1, k2) = (f64::from(v.k1), f64::from(v.k2));
        if k1 > max_k1 {
            max_k1 = k1;
        }
        if k2 > max_k2 {
            max_k2 = k2;
        }
        if k1 < min_k1 {
            min_k1 = k1;
        }
        if k2 < min_k2 {
            min_k2 = k2;
        }
    }
    let k1_scale = div64(999.0, sub64(max_k1, min_k1));
    let k2_scale = div64(999.0, sub64(max_k2, min_k2));
    // HISTOalloc(1000) twice, then the counts (FreeSurfer counts k1 and k2 of each vertex in turn:
    // a bin outside either histogram ends the program at that vertex)
    let mut h1 = histo_alloc(g, io.err, 1000)?;
    let mut h2 = histo_alloc(g, io.err, 1000)?;
    h1.bin_size = div64(1.0, k1_scale) as f32;
    h2.bin_size = div64(1.0, k2_scale) as f32;
    for vno in 0..nverts(mris) {
        let v = mris.vertex(vno);
        if v.ripflag != 0 {
            continue;
        }
        for (h, k, min, scale, name) in [
            (&mut h1, v.k1, min_k1, k1_scale, "k1"),
            (&mut h2, v.k2, min_k2, k2_scale, "k2"),
        ] {
            let bin = nint(g, mul64(sub64(f64::from(k), min), scale));
            if !(0..h.nbins).contains(&bin) {
                g.notices.add(
                    &FS_377,
                    format!("MRIScomputeSecondFundamentalFormThresholded: vertex {vno}'s {name} falls in bin {bin}, outside the histogram's 1,000 (not counted; FreeSurfer writes outside it)"),
                );
                if g.notices.reproduce() {
                    return Err(Fatal::Exit(139));
                }
                continue;
            }
            let c = &mut h.counts[bin as usize];
            *c = add(1.0, *c);
        }
    }
    for (h, min, scale) in [(&mut h1, min_k1, k1_scale), (&mut h2, min_k2, k2_scale)] {
        for bin in 0..h.nbins {
            h.bins[bin as usize] = add64(div64(f64::from(bin), scale), min) as f32;
        }
    }
    // the first zbins, overwritten (their conversions note FS-12 for a NaN)
    let _ = nint(g, mul64(sub64(0.0, min_k1), k1_scale));
    let _ = nint(g, mul64(sub64(0.0, min_k2), k2_scale));
    histo_make_pdf_in_place(&mut h1);
    histo_make_pdf_in_place(&mut h2);
    if g.gdiag & DIAG_WRITE != 0 && verbose_on(g) {
        histo_plot(&h1, dir, "k1.plt");
        histo_plot(&h2, dir, "k2.plt");
    }
    let zbin1 = histo_find_highest_peak_in_region(&h1, 0, h1.nbins);
    let zbin2 = histo_find_highest_peak_in_region(&h2, 0, h2.nbins);
    let lims = if zbin1 < 0 || zbin2 < 0 {
        g.notices.add(
            &FS_378,
            "MRIScomputeSecondFundamentalFormThresholded: no curvature counted, so no peak (FreeSurfer reads the histogram one place before its start; no unripped vertex to clip)",
        );
        None
    } else {
        Some((limits(&h1, zbin1, pct), limits(&h2, zbin2, pct)))
    };
    mris.Kmin = 10000.0;
    mris.Hmin = 10000.0;
    mris.Kmax = -10000.0;
    mris.Hmax = -10000.0;
    let mut nthresh = 0;
    let ((lo1, hi1), (lo2, hi2)) = lims.unwrap_or(((0.0, 0.0), (0.0, 0.0)));
    for vno in 0..nverts(mris) {
        if mris.vertex(vno).ripflag != 0 {
            continue;
        }
        let v = mris.vertex_mut(vno);
        if f64::from(v.k1) > hi1 {
            v.k1 = hi1 as f32;
            nthresh += 1;
        }
        if f64::from(v.k2) > hi2 {
            v.k2 = hi2 as f32;
            nthresh += 1;
        }
        if f64::from(v.k1) < lo1 {
            v.k1 = lo1 as f32;
            nthresh += 1;
        }
        if f64::from(v.k2) < lo2 {
            v.k2 = lo2 as f32;
            nthresh += 1;
        }
        v.K = mul(v.k1, v.k2);
        v.H = mul(add(v.k1, v.k2), 0.5);
        let (kk, hh) = (v.K, v.H);
        if kk < mris.Kmin {
            mris.Kmin = kk;
        }
        if hh < mris.Hmin {
            mris.Hmin = hh;
        }
        if kk > mris.Kmax {
            mris.Kmax = kk;
        }
        if hh > mris.Hmax {
            mris.Hmax = hh;
        }
    }
    if nthresh > 0 {
        let _ = writeln!(
            io.err,
            "{nthresh} vertices thresholded to be in k1 ~ [{} {}], k2 ~ [{} {}]",
            fmt_f(lo1, 2, 2),
            fmt_f(hi1, 2, 2),
            fmt_f(lo2, 2, 2),
            fmt_f(hi2, 2, 2)
        );
    }
    Ok(())
}

/// `MRIScomputeSecondFundamentalFormAtVertex(mris, vno, vertices, vnum)` (no FreeSurfer program
/// calls it): the fit of [`compute_second_fundamental_form_thresholded`] at one vertex over the
/// given neighbours, without thresholds: every unripped neighbour's row taken (its `height / (u² +
/// v²)` kept for the extremes when `u² + v²` is `FLT_EPSILON` or more). 0 (nothing) for a plane;
/// -4 (`ERROR_BADPARM`) for a ripped vertex or `vnum` 0 or less; a singular `Uᵀ U` gives curvatures
/// 0 with the tangent plane turned by the eigenvectors of the last call that found them (the
/// function's own matrix: zeros before; `DIAG_SHOW`: `1 ill-conditioned points`); a condition
/// number of 500000 or more gives `k1 = kmax`, `k2 = kmin` and `K`, `H` the surface's `Kmin`,
/// `Hmin` when `k1 k2 < 0`, else `Kmax`, `Hmax`, and -4; an eigensystem that fails gives the
/// singular values' curvatures and -4. The operand orders of its own compiled code (`k2 k1 area`
/// for `Ktotal`, the `z` components of `e1` and `e2`).
pub fn compute_second_fundamental_form_at_vertex(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    io: &mut Io,
    mris: &mut Mris,
    vno: usize,
    vertices: &[i32],
) -> Result<i32, Fatal> {
    const ERROR_BADPARM: i32 = -4;
    if mris.status == MrisStatus::PLANE {
        return Ok(NO_ERROR);
    }
    let mut eigen = sg.sff_at_vertex_eigen.unwrap_or([0.0; 4]);
    sg.sff_at_vertex_eigen = Some(eigen);
    if mris.vertex(vno).ripflag != 0 {
        return Ok(ERROR_BADPARM);
    }
    let v = mris.vertex(vno);
    let nrm = [v.nx, v.ny, v.nz];
    let e1 = [v.e1x, v.e1y, v.e1z];
    let e2 = [v.e2x, v.e2y, v.e2z];
    let p = [v.x, v.y, v.z];
    let vnum = i32::try_from(vertices.len()).unwrap_or(i32::MAX);
    if vnum <= 0 {
        return Ok(ERROR_BADPARM);
    }
    let mut m_u = alloc(g, io, vnum, 3)?;
    let mut v_z = alloc(g, io, vnum, 1)?;
    let (mut kmin, mut kmax) = (10000.0f32, -10000.0f32);
    let mut n = 0i32;
    for &nb in vertices {
        let vn = mris.vertex(nb as usize);
        if vn.ripflag != 0 {
            continue;
        }
        let y = [sub(vn.x, p[0]), sub(vn.y, p[1]), sub(vn.z, p[2])];
        let uf = add(add(mul(e1[0], y[0]), mul(e1[1], y[1])), mul(e1[2], y[2]));
        let vf = add(add(mul(y[1], e2[1]), mul(y[0], e2[0])), mul(y[2], e2[2]));
        let (ui, vi) = (f64::from(uf), f64::from(vf));
        let uu = mul64(ui, ui);
        let vv = mul64(vi, vi);
        m_u.set_relt(n + 1, 1, uu as f32);
        m_u.set_relt(n + 1, 2, mul64(add64(ui, ui), vi) as f32);
        m_u.set_relt(n + 1, 3, vv as f32);
        let z = add(add(mul(nrm[0], y[0]), mul(nrm[1], y[1])), mul(nrm[2], y[2]));
        v_z.set_relt(n + 1, 1, z);
        let rsq = add64(vv, uu) as f32;
        if !(rsq < f32::EPSILON) {
            let k = div(z, rsq);
            if k > kmax {
                kmax = k;
            }
            if k < kmin {
                kmin = k;
            }
        }
        n += 1;
    }
    let m_ut = matrix_transpose(g, io.out, io.err, &m_u, None)?.unwrap_new();
    let m_tmp2 = matrix_multiply(g, io.out, io.err, Some(&m_ut), Some(&m_u), None)?.unwrap_new();
    let cond_no = matrix_condition_number(g, io.out, io.err, &m_tmp2)?;
    let inv = matrix_svd_inverse(g, io.out, io.err, &m_tmp2, None)?;
    let mut nbad = 0;
    let mut evalues = [0.0f32; 3];
    match inv.into_new() {
        None => nbad += 1,
        Some(m_inverse) => {
            let m_tmp1 =
                matrix_multiply(g, io.out, io.err, Some(&m_ut), Some(&v_z), None)?.unwrap_new();
            let mut v_c = alloc(g, io, 3, 1)?;
            matrix_multiply(
                g,
                io.out,
                io.err,
                Some(&m_inverse),
                Some(&m_tmp1),
                Some(&mut v_c),
            )?;
            let mut m_q = alloc(g, io, 2, 2)?;
            let (c1, c2, c3) = (v_c.relt(1, 1), v_c.relt(2, 1), v_c.relt(3, 1));
            m_q.set_relt(1, 1, add(c1, c1));
            let q12 = add(c2, c2);
            m_q.set_relt(2, 1, q12);
            m_q.set_relt(1, 2, q12);
            m_q.set_relt(2, 2, add(c3, c3));
            if cond_no >= ILL_CONDITIONED {
                let (kmn, kmx, hmn, hmx) = (mris.Kmin, mris.Kmax, mris.Hmin, mris.Hmax);
                let v = mris.vertex_mut(vno);
                v.k1 = kmax;
                v.k2 = kmin;
                if mul(kmin, kmax) < 0.0 {
                    v.K = kmn;
                    v.H = hmn;
                } else {
                    v.K = kmx;
                    v.H = hmx;
                }
                return Ok(ERROR_BADPARM);
            }
            let mut m_eigen = alloc(g, io, 2, 2)?;
            for (k, x) in eigen.iter().enumerate() {
                m_eigen.set_relt(k as i32 / 2 + 1, k as i32 % 2 + 1, *x);
            }
            let r = matrix_eigen_system(g, io.out, io.err, &m_q, &mut evalues, Some(&mut m_eigen))?;
            eigen = [
                m_eigen.relt(1, 1),
                m_eigen.relt(1, 2),
                m_eigen.relt(2, 1),
                m_eigen.relt(2, 2),
            ];
            sg.sff_at_vertex_eigen = Some(eigen);
            if r.is_null() {
                matrix_svd_eigen_values(g, io.out, io.err, &m_q, &mut evalues)?;
                set_curvatures(mris, vno, evalues[0], evalues[1]);
                return Ok(ERROR_BADPARM);
            }
        }
    }
    let (k1, k2) = (evalues[0], evalues[1]);
    set_curvatures(mris, vno, k1, k2);
    let v = mris.vertex(vno);
    let (kk, hh, area) = (v.K, v.H, v.area);
    if i64::try_from(vno).ok() == Some(i64::from(g.gdiag_no)) && g.gdiag & DIAG_SHOW != 0 {
        let _ = writeln!(
            io.out,
            "v {vno}: k1={}, k2={}, K={}, H={}",
            fmt_f(f64::from(k1), 2, 3),
            fmt_f(f64::from(k2), 2, 3),
            fmt_f(f64::from(kk), 2, 3),
            fmt_f(f64::from(hh), 2, 3)
        );
    }
    if kk < mris.Kmin {
        mris.Kmin = kk;
    }
    if hh < mris.Hmin {
        mris.Hmin = hh;
    }
    if kk > mris.Kmax {
        mris.Kmax = kk;
    }
    if hh > mris.Hmax {
        mris.Hmax = hh;
    }
    mris.Ktotal = add64(
        mul64(mul64(f64::from(k2), f64::from(k1)), f64::from(area)),
        mris.Ktotal,
    );
    let [a11, a12, a21, a22] = eigen;
    let v = mris.vertex_mut(vno);
    v.e1x = add(mul(e2[0], a21), mul(e1[0], a11));
    v.e1y = add(mul(e1[1], a11), mul(e2[1], a21));
    v.e1z = add(mul(a11, e1[2]), mul(e2[2], a21));
    v.e2x = add(mul(e1[0], a12), mul(e2[0], a22));
    v.e2y = add(mul(e1[1], a12), mul(e2[1], a22));
    v.e2z = add(mul(a12, e1[2]), mul(e2[2], a22));
    if g.gdiag & DIAG_SHOW != 0 && nbad > 0 {
        let _ = writeln!(io.out, "{nbad} ill-conditioned points");
    }
    Ok(NO_ERROR)
}
