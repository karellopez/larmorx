// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `CostFunctions` (queue 3.2-02, `fs.robust` 1 of 2): the image moments of Martin Reuter's
//! robust registration that both `mri_robust_register` and `mri_robust_template` reach (the
//! registration's initial alignment), translated from FreeSurfer 8.2.0's
//! `mri_robust_register/CostFunctions.cpp`: [`centroid`] and [`orientation`].
//!
//! The cost functions proper serve only `mri_robust_register`'s Powell registration (`--cost`
//! other than the robust default, queue 3.2-13): [`least_squares`] (`LS`), [`abs_diff`] (`SAD`),
//! [`tukey_biweight`] (`TB`), [`ncc`] (`NCC`), [`local_ncc`] (`LNCC`), with [`map_mri`], [`mean`]
//! and [`minmax`] (`MyMRI::MRInorm255`'s range). Each samples the source at `Msi * (x, y, z)` and
//! the target at `Mti * (x, y, z)` (trilinear, `MRIsampleVolumeFrame`) over the target's grid,
//! every `d`-th voxel, with both images' outside value set to -1 (a sample of -1 is skipped). The
//! sums run in FreeSurfer's order at one thread (its OpenMP reductions add the threads' partial
//! sums in the order the threads finish: one thread is the order defined by the source).
//! FreeSurfer's other statistics in the file (`var`, `median`, `mad`, `moment`, ...) have no live
//! caller in either program; `SBCost` and the joint-histogram costs come later.
//!
//! Defects (`larmorx_core::defects`):
//! - **FS-966** (memory): [`orientation`] sorts the three eigenvalues by finding the largest and
//!   the smallest; when no eigenvalue is above another (all equal, or NaN) both are the first,
//!   and the middle one's index comes out as 3: FreeSurfer reads a fourth column of the 3 x 3
//!   eigenvector matrix (two of its numbers are the first column's, the third lies past the
//!   matrix). By default the eigenvectors keep EISPACK's order (0, 1, 2); `reproduce` reads the
//!   two numbers and fills the one past the matrix with 0 (noted).
//! - **FS-967** (rule B): [`orientation`] of an image with an infinite or NaN voxel has no
//!   covariance; VNL's eigensystem then never returns (FreeSurfer hangs, ITK-115) or stops
//!   without eigenvectors (`vnl_symmetric_eigensystem: ierr = <i>`, the vectors memory never
//!   written). By default a clear error and the end of the program (exit 1, as [`centroid`] ends
//!   on such an image); `reproduce` gives FreeSurfer's result where VNL returns with eigenvectors
//!   (the same error where it hangs or leaves them unwritten).

use std::io::Write;

use larmorx_core::cfmt;
use larmorx_core::defects::{FS_966, FS_967, ITK_115};
use larmorx_core::vnl_eigen::symmetric_eigensystem;
use larmorx_core::vnl_fixed;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::sse::{add64, div64, mul, mul64, sqrt64, sub64};
use larmorx_freesurfer_mri::mri::{MRI_FLOAT, MRI_UCHAR, Mri, mri_alloc_sequence};
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};
use larmorx_freesurfer_volume::sample::{ByteSampler, sample_volume_frame};

use crate::debug::Cout;
use crate::mymatrix::Mat4;
use crate::mymri::wide;

/// `CostFunctions::centroid(i)`: the centre of mass of frame 0 in 1-based voxel coordinates,
/// `(sum (w + 1) v / sum v, ...)`, a voxel within `outside_val / 255` of `outside_val` counted as 0
/// (`fabs(v - outside_val) < eps`; never with an outside value of 0). When the sum of the three is
/// NaN: `CostFunctions::centroid is NAN (empty image? n = <n> )` on standard error and `exit(1)`.
pub fn centroid(g: &mut Globals, err: &mut dyn Write, i: &Mri) -> Result<[f64; 3], Fatal> {
    let mut dd = [0.0f64; 3];
    let mut n = 0.0f64;
    let ov = i.outside_val;
    let eps = div64(ov, 255.0);
    for d in 0..i.vg.depth {
        for h in 0..i.vg.height {
            for w in 0..i.vg.width {
                let mut val = wide(mri_get_vox_val(g, err, i, w, h, d, 0, 0)?);
                if sub64(val, ov).abs() < eps {
                    val = 0.0;
                }
                n = add64(n, val);
                dd[0] = add64(dd[0], mul64(f64::from(w + 1), val));
                dd[1] = add64(dd[1], mul64(f64::from(h + 1), val));
                dd[2] = add64(dd[2], mul64(f64::from(d + 1), val));
            }
        }
    }
    for x in &mut dd {
        *x = div64(*x, n);
    }
    if add64(add64(dd[0], dd[1]), dd[2]).is_nan() {
        let _ = writeln!(
            err,
            "CostFunctions::centroid is NAN (empty image? n = {} )",
            cfmt::ostream(n)
        );
        return Err(Fatal::Exit(1));
    }
    Ok(dd)
}

/// `CostFunctions::orientation(i)`: the principal axes of frame 0's intensity distribution. The
/// moments in `double` from `float` terms (`n += v`; `M_100 += (w + 1) v`, `M_200 += (w + 1)^2 v`,
/// `M_110 += (w + 1)(h + 1) v`, ...: the integer coordinate products times the `float` voxel in
/// `float`); the covariance `M_ij / n - mean_i mean_j` (symmetric); VNL's symmetric eigensystem;
/// its eigenvectors as columns from the largest eigenvalue to the smallest; the first column
/// negated when the determinant is negative. FS-966, FS-967.
pub fn orientation(g: &mut Globals, err: &mut dyn Write, i: &Mri) -> Result<[[f64; 3]; 3], Fatal> {
    let mut dd = [0.0f64; 3];
    let mut n = 0.0f64;
    let mut cov = [[0.0f64; 3]; 3];
    for d in 0..i.vg.depth {
        let dp1 = d + 1;
        for h in 0..i.vg.height {
            let hp1 = h + 1;
            for w in 0..i.vg.width {
                let wp1 = w + 1;
                let val = mri_get_vox_val(g, err, i, w, h, d, 0, 0)?;
                let f = |k: i32| mul(k as f32, val);
                n = add64(n, wide(val));
                dd[0] = add64(dd[0], wide(f(wp1)));
                dd[1] = add64(dd[1], wide(f(hp1)));
                dd[2] = add64(dd[2], wide(f(dp1)));
                cov[0][0] = add64(cov[0][0], wide(f(wp1.wrapping_mul(wp1))));
                cov[1][1] = add64(cov[1][1], wide(f(hp1.wrapping_mul(hp1))));
                cov[2][2] = add64(cov[2][2], wide(f(dp1.wrapping_mul(dp1))));
                cov[0][1] = add64(cov[0][1], wide(f(wp1.wrapping_mul(hp1))));
                cov[1][2] = add64(cov[1][2], wide(f(hp1.wrapping_mul(dp1))));
                cov[0][2] = add64(cov[0][2], wide(f(wp1.wrapping_mul(dp1))));
            }
        }
    }
    for x in &mut dd {
        *x = div64(*x, n);
    }
    for k in 0..3 {
        cov[k][k] = sub64(div64(cov[k][k], n), mul64(dd[k], dd[k]));
    }
    cov[0][1] = sub64(div64(cov[0][1], n), mul64(dd[0], dd[1]));
    cov[1][2] = sub64(div64(cov[1][2], n), mul64(dd[1], dd[2]));
    cov[0][2] = sub64(div64(cov[0][2], n), mul64(dd[0], dd[2]));
    cov[1][0] = cov[0][1];
    cov[2][1] = cov[1][2];
    cov[2][0] = cov[0][2];
    let flat: Vec<f64> = cov.iter().flatten().copied().collect();
    let eig = symmetric_eigensystem(&flat, 3);
    if flat.iter().any(|x| !x.is_finite()) {
        let hangs = eig.never_returns();
        let unwritten = eig.ierr != 0;
        g.notices.add(
            &FS_967,
            if hangs {
                "CostFunctions::orientation of an image with an infinite or NaN voxel: FreeSurfer hangs (VNL's eigensystem); an error"
            } else if unwritten {
                "CostFunctions::orientation of an image with an infinite or NaN voxel: VNL leaves the eigenvectors unwritten; an error"
            } else {
                "CostFunctions::orientation of an image with an infinite or NaN voxel"
            },
        );
        if hangs {
            g.notices.add(
                &ITK_115,
                "vnl_symmetric_eigensystem would never return on the covariance",
            );
        }
        if hangs || unwritten || !g.notices.reproduce() {
            if unwritten && !hangs {
                let _ = writeln!(err, "vnl_symmetric_eigensystem: ierr = {}", eig.ierr);
            }
            let _ = writeln!(
                err,
                "CostFunctions::orientation: the image has an infinite or NaN voxel (no covariance)"
            );
            return Err(Fatal::Exit(1));
        }
    }
    let dval = &eig.values;
    let (mut smallest, mut largest) = (0usize, 0usize);
    for k in 1..3 {
        if dval[largest] < dval[k] {
            largest = k;
        }
        if dval[smallest] > dval[k] {
            smallest = k;
        }
    }
    let mut order = [
        largest,
        3usize.wrapping_sub(smallest).wrapping_sub(largest),
        smallest,
    ];
    let col = |c: usize, r: usize| eig.vectors[r * 3 + c];
    let mut evec = [[0.0f64; 3]; 3];
    if largest == smallest {
        g.notices.add(
            &FS_966,
            "CostFunctions::orientation: no eigenvalue above another: FreeSurfer reads a fourth eigenvector column",
        );
        if g.notices.reproduce() {
            // column 3 of the row-major 3 x 3 block: (V[1][0], V[2][0], the number past it)
            for (c, &k) in order.iter().enumerate() {
                for (r, row) in evec.iter_mut().enumerate() {
                    row[c] = if k == 3 {
                        if r < 2 { col(0, r + 1) } else { 0.0 }
                    } else {
                        col(k, r)
                    };
                }
            }
        } else {
            order = [0, 1, 2];
        }
    }
    if !(largest == smallest && g.notices.reproduce()) {
        for (c, &k) in order.iter().enumerate() {
            for (r, row) in evec.iter_mut().enumerate() {
                row[c] = col(k, r);
            }
        }
    }
    let d = vnl_fixed::det3(&evec);
    if d < 0.0 {
        for row in &mut evec {
            row[0] = -row[0];
        }
    }
    Ok(evec)
}

/// `CostFunctions::minmax(i)` of a byte image (the only type the Powell registration passes):
/// the smallest and largest voxel over every frame, as `float`.
pub fn minmax(i: &Mri) -> (f32, f32) {
    assert_eq!(i.mtype, MRI_UCHAR, "minmax of a byte image");
    let c = i.chunk.as_deref().unwrap_or(&[]);
    let mut it = c.iter().map(|&b| f32::from(b));
    let first = it.next().unwrap_or(0.0);
    let (mut min, mut max) = (first, first);
    for v in it {
        if v < min {
            min = v;
        }
        if v > max {
            max = v;
        }
    }
    (min, max)
}

/// [`sample_volume_frame`] of `m`, through its byte sampler when it has one (the same numbers).
#[allow(clippy::too_many_arguments)]
#[inline(always)]
pub(crate) fn sample(
    b: &Option<ByteSampler>,
    g: &mut Globals,
    err: &mut dyn Write,
    m: &Mri,
    x: f64,
    y: f64,
    z: f64,
    f: i32,
    v: &mut f64,
) {
    match b {
        Some(b) => b.sample(g, err, m, x, y, z, f, v),
        None => sample_volume_frame(g, err, m, x, y, z, f, v),
    };
}

/// A `float` image whose voxels lie in one buffer as its row pointers step them (the images
/// [`map_mri`] makes): a voxel inside it read and written straight, as `MRIgetVoxVal` and
/// `MRIsetVoxVal` read and write it.
#[derive(Clone, Copy)]
struct Floats {
    w: usize,
    h: usize,
    d: usize,
}

impl Floats {
    fn of(m: &Mri) -> Option<Floats> {
        let c = m.chunk.as_deref()?;
        let (w, h, d, nf) = (m.vg.width, m.vg.height, m.vg.depth, m.nframes);
        if m.mtype != MRI_FLOAT || w < 1 || h < 1 || d < 1 || nf < 1 {
            return None;
        }
        let (w, h, d, nf) = (w as usize, h as usize, d as usize, nf as usize);
        let wh = w.checked_mul(h)?;
        let whd = wh.checked_mul(d)?;
        let need = whd.checked_mul(nf)?.checked_mul(4)?;
        let ok = m.vox_per_row == w as u64
            && m.vox_per_slice == wh as u64
            && m.vox_per_vol == whd as u64
            && c.len() >= need
            && need as u64 <= m.bytes_total;
        ok.then_some(Floats { w, h, d })
    }

    #[inline(always)]
    fn at(&self, x: i32, y: i32, z: i32, f: i32) -> usize {
        4 * (((f as usize * self.d + z as usize) * self.h + y as usize) * self.w + x as usize)
    }
}

/// `MRIgetVoxVal(m, x, y, z, f)` of a voxel inside `m` ([`Floats`] when it has them).
#[allow(clippy::too_many_arguments)]
#[inline(always)]
fn fget(
    l: &Option<Floats>,
    g: &mut Globals,
    err: &mut dyn Write,
    m: &Mri,
    x: i32,
    y: i32,
    z: i32,
    f: i32,
) -> Result<f32, Fatal> {
    match (l, m.chunk.as_deref()) {
        (Some(l), Some(c)) => {
            let o = l.at(x, y, z, f);
            Ok(f32::from_le_bytes([c[o], c[o + 1], c[o + 2], c[o + 3]]))
        }
        _ => mri_get_vox_val(g, err, m, x, y, z, f, 0),
    }
}

/// The row constants of one slice or row of the sampling grid: `M[k][a] * v + base[k]`, `k` the
/// three rows (the source's order: column `a` times the coordinate, plus the next level's sum).
#[inline(always)]
fn lane(m: &Mat4, a: usize, v: f64, base: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|k| add64(mul64(m[k][a], v), base[k]))
}

/// The sampling loop the half-way costs share: the target's grid from 0 below `extent - d + 1`
/// in steps of `d` (slices, rows, columns), each frame; `term(z, vs, vt)` (`z` the slice) for every
/// pair of samples neither of which is -1.
#[allow(clippy::too_many_arguments)]
fn half_way_samples(
    g: &mut Globals,
    err: &mut dyn Write,
    s: &Mri,
    t: &Mri,
    msi: &Mat4,
    mti: &Mat4,
    d: [i32; 3],
    mut term: impl FnMut(i32, f64, f64),
) {
    let dt = [
        t.vg.width - d[0] + 1,
        t.vg.height - d[1] + 1,
        t.vg.depth - d[2] + 1,
        t.nframes,
    ];
    let col3 = |m: &Mat4| [m[0][3], m[1][3], m[2][3]];
    let (bs, bt) = (ByteSampler::new(s), ByteSampler::new(t));
    let mut z = 0;
    while z < dt[2] {
        let tz = lane(mti, 2, f64::from(z), col3(mti));
        let sz = lane(msi, 2, f64::from(z), col3(msi));
        let mut y = 0;
        while y < dt[1] {
            let ty = lane(mti, 1, f64::from(y), tz);
            let sy = lane(msi, 1, f64::from(y), sz);
            let mut x = 0;
            while x < dt[0] {
                let pt = lane(mti, 0, f64::from(x), ty);
                let ps = lane(msi, 0, f64::from(x), sy);
                for f in 0..dt[3] {
                    let mut vs = 0.0;
                    sample(&bs, g, err, s, ps[0], ps[1], ps[2], f, &mut vs);
                    if vs == -1.0 {
                        continue;
                    }
                    let mut vt = 0.0;
                    sample(&bt, g, err, t, pt[0], pt[1], pt[2], f, &mut vt);
                    if vt == -1.0 {
                        continue;
                    }
                    term(z, vs, vt);
                }
                x += d[0];
            }
            y += d[1];
        }
        z += d[2];
    }
}

/// `CostFunctions::leastSquares(mriS, mriT, Msi, Mti, d1, d2, d3, s1, s2)`: the sum of `((s1 vs)
/// - (s2 vt))^2` over the half-way samples (both outside values restored afterwards).
#[allow(clippy::too_many_arguments)]
pub fn least_squares(
    g: &mut Globals,
    err: &mut dyn Write,
    s: &mut Mri,
    t: &mut Mri,
    msi: &Mat4,
    mti: &Mat4,
    d: [i32; 3],
    s1: f64,
    s2: f64,
) -> f64 {
    let (sout, tout) = (s.outside_val, t.outside_val);
    s.outside_val = -1.0;
    t.outside_val = -1.0;
    let mut sum = 0.0f64;
    half_way_samples(g, err, s, t, msi, mti, d, |_, vs, vt| {
        let dd = sub64(mul64(s1, vs), mul64(s2, vt));
        sum = add64(sum, mul64(dd, dd));
    });
    s.outside_val = sout;
    t.outside_val = tout;
    // OpenMP's reduction: the shared 0 plus the one thread's sum
    add64(0.0, sum)
}

/// `CostFunctions::absDiff(mriS, mriT, Msi, Mti, d1, d2, d3, s1, s2)`: the sum of `|(s1 vs) - (s2
/// vt)|` over the half-way samples (both outside values left at -1).
#[allow(clippy::too_many_arguments)]
pub fn abs_diff(
    g: &mut Globals,
    err: &mut dyn Write,
    s: &mut Mri,
    t: &mut Mri,
    msi: &Mat4,
    mti: &Mat4,
    d: [i32; 3],
    s1: f64,
    s2: f64,
) -> f64 {
    s.outside_val = -1.0;
    t.outside_val = -1.0;
    let mut sum = 0.0f64;
    half_way_samples(g, err, s, t, msi, mti, d, |_, vs, vt| {
        let dd = sub64(mul64(s1, vs), mul64(s2, vt));
        sum = add64(sum, dd.abs());
    });
    add64(0.0, sum)
}

/// `CostFunctions::rhoTukeyBiweight(d, sat)`: `sat^2 / 2` beyond `sat`, else `(sat^2 / 2) (1 -
/// (1 - (d / sat)^2)^3)`.
pub fn rho_tukey_biweight(d: f64, sat: f64) -> f64 {
    let half = div64(mul64(sat, sat), 2.0);
    if d > sat || d < -sat {
        half
    } else {
        let a = div64(d, sat);
        let b = sub64(1.0, mul64(a, a));
        mul64(half, sub64(1.0, mul64(mul64(b, b), b)))
    }
}

/// `CostFunctions::tukeyBiweight(mriS, mriT, Msi, Mti, d1, d2, d3, sat)`: the half-way
/// differences `vs - vt` stored as `float` slice by slice (slice `z / d3` from `(z / d3) ((width +
/// 1) / d1) ((height + 1) / d2) nframes`, the rest of a `calloc`'s zeros), then the sum of
/// [`rho_tukey_biweight`] over all of them in order (both outside values left at -1).
#[allow(clippy::too_many_arguments)]
pub fn tukey_biweight(
    g: &mut Globals,
    err: &mut dyn Write,
    s: &mut Mri,
    t: &mut Mri,
    msi: &Mat4,
    mti: &Mat4,
    d: [i32; 3],
    sat: f64,
) -> f64 {
    s.outside_val = -1.0;
    t.outside_val = -1.0;
    // unsigned int arithmetic, as FreeSurfer's
    let q = |e: i32, dd: i32| ((e + 1) / dd) as u32;
    let nf = t.nframes as u32;
    let pn = q(t.vg.width, d[0])
        .wrapping_mul(q(t.vg.height, d[1]))
        .wrapping_mul(nf);
    let n = pn.wrapping_mul(q(t.vg.depth, d[2]));
    let mut diff = vec![0.0f32; n as usize];
    let (mut slice, mut count) = (-1i32, 0usize);
    half_way_samples(g, err, s, t, msi, mti, d, |z, vs, vt| {
        if z != slice {
            slice = z;
            count = 0;
        }
        let zpos = ((z / d[2]) as u32).wrapping_mul(pn) as usize;
        diff[zpos + count] = sub64(vs, vt) as f32;
        count += 1;
    });
    let mut sum = 0.0f64;
    for &v in &diff {
        sum = add64(sum, rho_tukey_biweight(f64::from(v), sat));
    }
    add64(0.0, sum)
}

/// `CostFunctions::mapMRI(mri, Mi, d1, d2, d3)`: a `float` image of `((extent - 1) / d) + 1`
/// voxels per axis (the source's frames and outside value), voxel `(ix, iy, iz)` the trilinear
/// sample of `mri` at `Mi * (ix d1, iy d2, iz d3)` (`MRIsampleVolumeFrame`, stored as `float`).
pub fn map_mri(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mri: &Mri,
    mi: &Mat4,
    d: [i32; 3],
) -> Result<Mri, Fatal> {
    let width = ((mri.vg.width - 1) / d[0]) + 1;
    let height = ((mri.vg.height - 1) / d[1]) + 1;
    let depth = ((mri.vg.depth - 1) / d[2]) + 1;
    let mut nmri = mri_alloc_sequence(g, out, err, width, height, depth, MRI_FLOAT, mri.nframes)?;
    nmri.outside_val = mri.outside_val;
    let col3 = [mi[0][3], mi[1][3], mi[2][3]];
    let b = ByteSampler::new(mri);
    let l = Floats::of(&nmri);
    for iz in 0..depth {
        let tz = lane(mi, 2, f64::from(iz * d[2]), col3);
        for iy in 0..height {
            let ty = lane(mi, 1, f64::from(iy * d[1]), tz);
            for ix in 0..width {
                let p = lane(mi, 0, f64::from(ix * d[0]), ty);
                for f in 0..mri.nframes {
                    let mut v = 0.0;
                    sample(&b, g, err, mri, p[0], p[1], p[2], f, &mut v);
                    match (&l, nmri.chunk.as_mut()) {
                        (Some(l), Some(c)) => {
                            let o = l.at(ix, iy, iz, f);
                            c[o..o + 4].copy_from_slice(&(v as f32).to_le_bytes());
                        }
                        _ => {
                            mri_set_vox_val(g, err, &mut nmri, ix, iy, iz, f, v as f32, 0)?;
                        }
                    }
                }
            }
        }
    }
    Ok(nmri)
}

/// `CostFunctions::mean(mri, frame)`: the sum of the frame's voxels that are not -1 divided by
/// their count (`width height depth` less those, unsigned).
pub fn mean(g: &mut Globals, err: &mut dyn Write, mri: &Mri, frame: i32) -> Result<f64, Fatal> {
    let mut d = 0.0f64;
    let mut ocount = 0u32;
    let l = Floats::of(mri).filter(|_| (0..mri.nframes).contains(&frame));
    for z in 0..mri.vg.depth {
        for y in 0..mri.vg.height {
            for x in 0..mri.vg.width {
                let v = wide(fget(&l, g, err, mri, x, y, z, frame)?);
                if v == -1.0 {
                    ocount = ocount.wrapping_add(1);
                    continue;
                }
                d = add64(d, v);
            }
        }
    }
    let n = (mri
        .vg
        .width
        .wrapping_mul(mri.vg.height)
        .wrapping_mul(mri.vg.depth) as u32)
        .wrapping_sub(ocount);
    Ok(div64(add64(0.0, d), f64::from(n)))
}

/// `CostFunctions::NCC(mriS, mriT, Msi, Mti, d1, d2, d3)`: both images mapped ([`map_mri`], every
/// `d`-th voxel), each frame's [`mean`], then `|sum ds dt / (sqrt(sum ds^2) sqrt(sum dt^2))|`
/// over the voxels neither of which is -1 (`ds = vs - mean_s`, `dt = vt - mean_t`); both outside
/// values left at -1.
#[allow(clippy::too_many_arguments)]
pub fn ncc(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    s: &mut Mri,
    t: &mut Mri,
    msi: &Mat4,
    mti: &Mat4,
    d: [i32; 3],
) -> Result<f64, Fatal> {
    s.outside_val = -1.0;
    t.outside_val = -1.0;
    let ns = map_mri(g, out, err, s, msi, d)?;
    let nt = map_mri(g, out, err, t, mti, d)?;
    let mut mean_s = Vec::new();
    let mut mean_t = Vec::new();
    for f in 0..t.nframes {
        mean_s.push(mean(g, err, &ns, f)?);
        mean_t.push(mean(g, err, &nt, f)?);
    }
    let (mut dsum, mut sig_s, mut sig_t) = (0.0f64, 0.0f64, 0.0f64);
    // the mapped images have the same extent and frames (both on the target's grid): every voxel
    // read inside both
    let (ls, lt) = (Floats::of(&ns), Floats::of(&nt));
    let same = ns.vg.width == nt.vg.width
        && ns.vg.height == nt.vg.height
        && ns.vg.depth == nt.vg.depth
        && ns.nframes == nt.nframes;
    let (ls, lt) = if same { (ls, lt) } else { (None, None) };
    for z in 0..nt.vg.depth {
        for y in 0..nt.vg.height {
            for x in 0..nt.vg.width {
                for f in 0..nt.nframes {
                    let vs = wide(fget(&ls, g, err, &ns, x, y, z, f)?);
                    if vs == -1.0 {
                        continue;
                    }
                    let vt = wide(fget(&lt, g, err, &nt, x, y, z, f)?);
                    if vt == -1.0 {
                        continue;
                    }
                    let ds = sub64(vs, mean_s[f as usize]);
                    let dt = sub64(vt, mean_t[f as usize]);
                    dsum = add64(dsum, mul64(ds, dt));
                    sig_s = add64(sig_s, mul64(ds, ds));
                    sig_t = add64(sig_t, mul64(dt, dt));
                }
            }
        }
    }
    let dsum = add64(0.0, dsum);
    let sig_s = sqrt64(sig_s);
    let sig_t = sqrt64(sig_t);
    Ok(div64(dsum, mul64(sig_s, sig_t)).abs())
}

/// `CostFunctions::localNCC(mriS, mriT, Msi, Mti, d1, d2, d3)` (`RegPowell` passes 5, 5, 5): both
/// images mapped at every voxel, then for the block centres from 2 below `extent - d - 2` in steps
/// of `d`, each frame: the 5^3 block's means, then `|sum ds dt| / (sqrt(sum dt^2) sqrt(sum ds^2))`
/// added to the total; a block holding a -1, or of no variance in either image, adds nothing (a
/// -1 also ends that centre's remaining frames); the total divided by the number of blocks added
/// (unsigned). A NaN total prints `d is nan <dd> <svar> <tvar>` (in the standard output's state
/// `cout`) and ends the program (status 1).
#[allow(clippy::too_many_arguments)]
pub fn local_ncc(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    s: &mut Mri,
    t: &mut Mri,
    msi: &Mat4,
    mti: &Mat4,
    d: [i32; 3],
    cout: Cout,
) -> Result<f64, Fatal> {
    const R: i32 = 2;
    const BW: i32 = 2 * R + 1;
    s.outside_val = -1.0;
    t.outside_val = -1.0;
    let ns = map_mri(g, out, err, s, msi, [1, 1, 1])?;
    let nt = map_mri(g, out, err, t, mti, [1, 1, 1])?;
    let dt = [
        t.vg.width - d[0] - R,
        t.vg.height - d[1] - R,
        t.vg.depth - d[2] - R,
        t.nframes,
    ];
    let mut total = 0.0f64;
    let mut dcount = 0u32;
    // both mapped at every voxel of their own grids (the source's and the target's extents)
    let (ls, lt) = (Floats::of(&ns), Floats::of(&nt));
    let inside = |m: &Mri, l: Option<Floats>| {
        l.filter(|_| {
            m.vg.width >= t.vg.width
                && m.vg.height >= t.vg.height
                && m.vg.depth >= t.vg.depth
                && m.nframes >= t.nframes
        })
    };
    let (ls, lt) = (inside(&ns, ls), inside(&nt, lt));
    let mut z = R;
    while z < dt[2] {
        let (nz, nzm) = (z - R, z - R + BW);
        let mut y = R;
        while y < dt[1] {
            let (ny, nym) = (y - R, y - R + BW);
            let mut x = R;
            'coord: while x < dt[0] {
                let (nx, nxm) = (x - R, x - R + BW);
                for f in 0..dt[3] {
                    let (mut smean, mut tmean) = (0.0f64, 0.0f64);
                    let mut counter = 0i32;
                    for i in nz..nzm {
                        for j in ny..nym {
                            for k in nx..nxm {
                                let vs = wide(fget(&ls, g, err, &ns, k, j, i, f)?);
                                if vs == -1.0 {
                                    x += d[0];
                                    continue 'coord;
                                }
                                let vt = wide(fget(&lt, g, err, &nt, k, j, i, f)?);
                                if vt == -1.0 {
                                    x += d[0];
                                    continue 'coord;
                                }
                                tmean = add64(tmean, vt);
                                smean = add64(smean, vs);
                                counter += 1;
                            }
                        }
                    }
                    tmean = div64(tmean, f64::from(counter));
                    smean = div64(smean, f64::from(counter));
                    let (mut dd, mut svar, mut tvar) = (0.0f64, 0.0f64, 0.0f64);
                    for i in nz..nzm {
                        for j in ny..nym {
                            for k in nx..nxm {
                                let vs = wide(fget(&ls, g, err, &ns, k, j, i, f)?);
                                if vs == -1.0 {
                                    x += d[0];
                                    continue 'coord;
                                }
                                let vt = wide(fget(&lt, g, err, &nt, k, j, i, f)?);
                                if vt == -1.0 {
                                    x += d[0];
                                    continue 'coord;
                                }
                                let tdd = sub64(vt, tmean);
                                let sdd = sub64(vs, smean);
                                dd = add64(dd, mul64(tdd, sdd));
                                svar = add64(svar, mul64(tdd, tdd));
                                tvar = add64(tvar, mul64(sdd, sdd));
                            }
                        }
                    }
                    if svar == 0.0 || tvar == 0.0 {
                        x += d[0];
                        continue 'coord;
                    }
                    let svar = sqrt64(svar);
                    let tvar = sqrt64(tvar);
                    total = add64(total, div64(dd, mul64(svar, tvar)).abs());
                    if total.is_nan() {
                        let _ = out.flush();
                        let _ = writeln!(
                            out,
                            "d is nan {} {} {}",
                            cout.os(dd),
                            cout.os(svar),
                            cout.os(tvar)
                        );
                        return Err(Fatal::Exit(1));
                    }
                    dcount = dcount.wrapping_add(1);
                }
                x += d[0];
            }
            y += d[1];
        }
        z += d[2];
    }
    Ok(div64(add64(0.0, total), f64::from(dcount)))
}

/// `CostFunctions::SBCost(mriS, mriT, Msi, Mti, d1, d2, d3)`: both images mapped ([`map_mri`],
/// every `d`-th voxel); each made zero-mean ([`mean`]) and unit-norm as a vector; the voxels
/// ordered by `vs + sign(vs . vt) vt`, largest first (libstdc++'s `std::sort` of the indices,
/// which moves equal keys in its own order: [`larmorx_core::stdsort::sort_by`]); then
/// `-n max_i (Cs_i^2 + Ct_i^2) / ((i + 1)(n - 1 - i))` over the cumulative sums `Cs`, `Ct` of the
/// ordered vectors (`i < n - 1`; 0 when nothing is above 0). Images of other extents print
/// `CostFunctions::SBCost image dimensions differ!`, images of more than one frame
/// `CostFunctions::SBCost only works on single frame inputs!` on standard error and end the
/// program (status 1).
#[allow(clippy::too_many_arguments)]
pub fn sb_cost(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    s: &Mri,
    t: &Mri,
    msi: &Mat4,
    mti: &Mat4,
    d: [i32; 3],
) -> Result<f64, Fatal> {
    if s.vg.width != t.vg.width || s.vg.height != t.vg.height || s.vg.depth != t.vg.depth {
        let _ = writeln!(err, "CostFunctions::SBCost image dimensions differ!");
        return Err(Fatal::Exit(1));
    }
    if s.nframes > 1 || t.nframes > 1 {
        let _ = writeln!(
            err,
            "CostFunctions::SBCost only works on single frame inputs!"
        );
        return Err(Fatal::Exit(1));
    }
    let ns = map_mri(g, out, err, s, msi, d)?;
    let nt = map_mri(g, out, err, t, mti, d)?;
    let n = (ns.vg.width as u32)
        .wrapping_mul(ns.vg.height as u32)
        .wrapping_mul(ns.vg.depth as u32);
    let m1 = mean(g, err, &ns, 0)?;
    let m2 = mean(g, err, &nt, 0)?;
    let mut iv1 = Vec::with_capacity(n as usize);
    let mut iv2 = Vec::with_capacity(n as usize);
    let (mut norm1, mut norm2) = (0.0f64, 0.0f64);
    // both mapped on the same grid (the images' extents are equal, one frame)
    let (ls, lt) = (Floats::of(&ns), Floats::of(&nt));
    for z in 0..ns.vg.depth {
        for y in 0..ns.vg.height {
            for x in 0..ns.vg.width {
                let f1 = sub64(wide(fget(&ls, g, err, &ns, x, y, z, 0)?), m1);
                let f2 = sub64(wide(fget(&lt, g, err, &nt, x, y, z, 0)?), m2);
                iv1.push(f1);
                iv2.push(f2);
                norm1 = add64(norm1, mul64(f1, f1));
                norm2 = add64(norm2, mul64(f2, f2));
            }
        }
    }
    let norm1 = sqrt64(norm1);
    let norm2 = sqrt64(norm2);
    let mut prod = 0.0f64;
    for (a, b) in iv1.iter_mut().zip(iv2.iter_mut()) {
        *a = div64(*a, norm1);
        *b = div64(*b, norm2);
        prod = add64(prod, mul64(*a, *b));
    }
    // `int sign = std::copysign(1, prod)`
    let sign = f64::from(1.0f64.copysign(prod) as i32);
    for (a, &b) in iv1.iter_mut().zip(&iv2) {
        *a = add64(*a, mul64(sign, b));
    }
    let mut idx: Vec<usize> = (0..iv1.len()).collect();
    larmorx_core::stdsort::sort_by(&mut idx, |&i1, &i2| iv1[i1] > iv1[i2]);
    let mut c1 = Vec::with_capacity(idx.len());
    let mut c2 = Vec::with_capacity(idx.len());
    let (mut f1, mut f2) = (0.0f64, 0.0f64);
    for &k in &idx {
        f1 = add64(f1, iv1[k]);
        f2 = add64(f2, iv2[k]);
        c1.push(f1);
        c2.push(f2);
    }
    let nd = f64::from(n);
    let mut mm = 0.0f64;
    for i in 0..(n as usize).saturating_sub(1) {
        let fi = i as f64;
        let num = add64(mul64(c1[i], c1[i]), mul64(c2[i], c2[i]));
        let den = mul64(fi + 1.0, sub64(sub64(nd, 1.0), fi));
        let f = div64(num, den);
        if f > mm {
            mm = f;
        }
    }
    Ok(mul64(-mm, nd))
}
