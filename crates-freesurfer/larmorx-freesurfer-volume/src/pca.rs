// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Frame statistics of `utils/mri2.cpp` (3.1-37's piece d): `MRIcovarianceMatrix`, `MRIpca`,
//! `PrintPCAStats`, `WritePCAStats` and `MRIfdr2vwth`, with `utils/sig.cpp`'s `fdr2vwth` and
//! `doublecompar` (no queue item of their own; reached only through `MRIfdr2vwth`).
//!
//! Defect (`larmorx_core::defects`; the findings in `docs/findings/freesurfer-volume.md` §15):
//! - **FS-483**: with a mask, the covariance and the PCA count the voxels whose mask value is
//!   above 0.5 but use every voxel whose mask value is not below 0.5: a mask voxel of exactly 0.5
//!   (or NaN) enters the sums and not the count. With `reproduce` as FreeSurfer; by default the
//!   count takes the voxels the sums use, noted where they differ.
//! - **FS-484** (crash): the PCA of a volume without frames crashes in the SVD of its 0 x 0
//!   covariance (VXL's `ssvdc`); both modes give an error (rule B).
//!
//! `log10` has no correctly rounded version in larmorx yet: glibc 2.35's
//! ([`larmorx_core::math::log10_glibc`]) in both modes.

use std::io::Write;
use std::path::Path;

use larmorx_core::defects::{FS_483, FS_484};
use larmorx_core::glibc_qsort::qsort;
use larmorx_core::math::{Libm, log10_glibc};
use larmorx_freesurfer_core::cstd::note_glibc07;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, Fatal};
use larmorx_freesurfer_core::matrix::{
    MATRIX_REAL, Matrix, matrix_alloc, matrix_copy, matrix_multiply,
};
use larmorx_freesurfer_core::matrix_decomp::matrix_svd;
use larmorx_freesurfer_core::sse::{add64, div, div64, mul, mul64, sqrt, sqrt64};
use larmorx_freesurfer_mri::mri::{MRI_FLOAT, Mri, mri_alloc_sequence, mri_copy_header};
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};

use crate::rows::{self, Num};

/// `(double)` of a `float` as `cvtss2sd` makes it (a signalling NaN quieted).
fn widen(x: f32) -> f64 {
    if x.is_nan() {
        f64::from(f32::from_bits(x.to_bits() | 0x0040_0000))
    } else {
        f64::from(x)
    }
}

/// The voxel count of a mask: FreeSurfer's (above 0.5) with `reproduce`, else the voxels the sums
/// use (not below 0.5); FS-483 noted where the two differ.
fn mask_count(g: &mut Globals, err: &mut dyn Write, mri: &Mri, mask: &Mri) -> Result<i32, Fatal> {
    let (mut above, mut used) = (0i32, 0i32);
    for c in 0..mri.vg.width {
        for r in 0..mri.vg.height {
            for s in 0..mri.vg.depth {
                let v = f64::from(mri_get_vox_val(g, err, mask, c, r, s, 0, 0)?);
                if v > 0.5 {
                    above += 1;
                }
                if !(v < 0.5) {
                    used += 1;
                }
            }
        }
    }
    if above != used {
        g.notices.add(
            &FS_483,
            "a mask voxel of exactly 0.5 (or NaN) enters the frame sums but not FreeSurfer's voxel count",
        );
        if !g.notices.reproduce() {
            return Ok(used);
        }
    }
    Ok(above)
}

/// Whether a voxel is outside the mask (its value below 0.5).
fn masked_out(
    g: &mut Globals,
    err: &mut dyn Write,
    mask: Option<&Mri>,
    c: i32,
    r: i32,
    s: i32,
) -> Result<bool, Fatal> {
    Ok(match mask {
        Some(m) => f64::from(mri_get_vox_val(g, err, m, c, r, s, 0, 0)?) < 0.5,
        None => false,
    })
}

/// `MRIcovarianceMatrix(mri, mask)`: the frames-by-frames matrix whose element `(f1, f2)` is the
/// sum over the voxels (columns outermost, then rows, slices; with a mask only those whose mask
/// value is not below 0.5) of `v2 v1` in `double` (each product added to the running sum), divided
/// by the voxel count (all voxels, or with a mask those above 0.5: FS-483), stored as `float` in
/// both mirrored places. A mask with no voxel above 0.5 prints `ERROR: no voxels in mask` and gives
/// `None`.
pub fn mri_covariance_matrix(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mri: &Mri,
    mask: Option<&Mri>,
) -> Result<Option<Matrix>, Fatal> {
    let nmask = match mask {
        Some(m) => {
            let n = mask_count(g, err, mri, m)?;
            if n == 0 {
                let _ = writeln!(out, "ERROR: no voxels in mask");
                return Ok(None);
            }
            n
        }
        None => mri
            .vg
            .width
            .wrapping_mul(mri.vg.height)
            .wrapping_mul(mri.vg.depth),
    };
    let nf = mri.nframes;
    let Some(mut m) = matrix_alloc(g, out, err, nf, nf, MATRIX_REAL)? else {
        return Ok(None);
    };
    let n = f64::from(nmask);
    for f1 in 0..nf {
        for f2 in f1..nf {
            let mut sum = 0.0f64;
            for c in 0..mri.vg.width {
                for r in 0..mri.vg.height {
                    for s in 0..mri.vg.depth {
                        if masked_out(g, err, mask, c, r, s)? {
                            continue;
                        }
                        let v1 = widen(mri_get_vox_val(g, err, mri, c, r, s, f1, 0)?);
                        let v2 = widen(mri_get_vox_val(g, err, mri, c, r, s, f2, 0)?);
                        sum = add64(mul64(v2, v1), sum);
                    }
                }
            }
            let e = div64(sum, n) as f32;
            m.set_relt(f1 + 1, f2 + 1, e);
            m.set_relt(f2 + 1, f1 + 1, e);
        }
    }
    Ok(Some(m))
}

/// What [`mri_pca`] returns: `U` (the covariance's left singular vectors, in place), the singular
/// values `S` (a row vector of the frame count) and the spatial vectors `V` (a `float` volume with
/// a frame per kept dimension).
pub struct Pca {
    pub u: Matrix,
    pub s: Matrix,
    pub v: Mri,
}

/// `MRIpca(D, &U, &S, &V, mask)`: the covariance ([`mri_covariance_matrix`]; its `None` gives 1)
/// decomposed by `MatrixSVD` (a copy becomes `U`, the squared singular values `S2`); for `c` from 1
/// to `min(voxels, frames)`, `S2[c]` times the mask count (in `float`) and `S[c] = sqrt(S2[c])`,
/// stopping after the first `S2[c]` below `FLT_EPSILON` (the kept dimensions: those before it, at
/// least one). `U S^-1` (`float`: `U[f][c] / sqrt(S2[c])`), then at each voxel the row of its
/// frames times it (`MatrixMultiply`) into `V`'s frames, each squared (`float`) and summed in
/// `double`; then each voxel of `V` divided by the square root of its frame's sum. With a mask,
/// only the voxels whose mask value is not below 0.5. 0 with the result.
pub fn mri_pca(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    d: &Mri,
    mask: Option<&Mri>,
) -> Result<(i32, Option<Pca>), Fatal> {
    let nvox =
        d.vg.width
            .wrapping_mul(d.vg.height)
            .wrapping_mul(d.vg.depth);
    let nf = d.nframes;
    let dim = if nvox < nf { nvox } else { nf };
    let nmask = match mask {
        Some(m) => mask_count(g, err, d, m)?,
        None => nvox,
    };
    let Some(m) = mri_covariance_matrix(g, out, err, d, mask)? else {
        return Ok((1, None));
    };
    if nf < 1 {
        // FS-484 (rule B): FreeSurfer's SVD of the 0 x 0 covariance crashes (VXL's ssvdc)
        g.notices.add(
            &FS_484,
            "MRIpca of a volume without frames: FreeSurfer crashes in the SVD",
        );
        log::error_exit(
            g,
            err,
            ERROR_BADPARM,
            "MRIpca: a volume without frames (FreeSurfer crashes in the SVD here)\n",
        )?;
        return Ok((1, None));
    }
    let mut s2 = matrix_alloc(g, out, err, 1, nf, MATRIX_REAL)?.expect("a row vector");
    let mut u = matrix_copy(g, out, err, Some(&m), None)?.unwrap_new();
    let _vv = matrix_svd(g, out, err, &mut u, &mut s2, None)?;
    let mut ps = matrix_alloc(g, out, err, 1, nf, MATRIX_REAL)?.expect("a row vector");
    let nm = nmask as f32;
    let mut c = 1;
    while c <= dim {
        let v = mul(s2.relt(1, c), nm);
        s2.set_relt(1, c, v);
        ps.set_relt(1, c, sqrt(v));
        if v < f32::EPSILON {
            break;
        }
        c += 1;
    }
    let dim_real = (c - 1).max(1);
    let mut uinvs = matrix_alloc(g, out, err, nf, dim_real, MATRIX_REAL)?.expect("U S^-1");
    for c in 1..=dim_real {
        for f in 1..=nf {
            let root = sqrt(s2.relt(1, c));
            uinvs.set_relt(f, c, div(u.relt(f, c), root));
        }
    }
    let (w, h, dd) = (d.vg.width, d.vg.height, d.vg.depth);
    let mut pv = mri_alloc_sequence(g, out, err, w, h, dd, MRI_FLOAT, dim_real)?;
    mri_copy_header(g, out, err, d, Some(&mut pv))?;
    let mut sum2 = vec![0.0f64; dim_real as usize];
    let mut fd = matrix_alloc(g, out, err, 1, nf, MATRIX_REAL)?.expect("a row vector");
    let mut fv = matrix_alloc(g, out, err, 1, dim_real, MATRIX_REAL)?.expect("a row vector");
    for c in 0..w {
        for r in 0..h {
            for s in 0..dd {
                if masked_out(g, err, mask, c, r, s)? {
                    continue;
                }
                for f in 0..nf {
                    let v = mri_get_vox_val(g, err, d, c, r, s, f, 0)?;
                    fd.set_relt(1, f + 1, v);
                }
                matrix_multiply(g, out, err, Some(&fd), Some(&uinvs), Some(&mut fv))?;
                for f in 0..dim_real {
                    let x = fv.relt(1, f + 1);
                    mri_set_vox_val(g, err, &mut pv, c, r, s, f, x, 0)?;
                    sum2[f as usize] = add64(widen(mul(x, x)), sum2[f as usize]);
                }
            }
        }
    }
    for c in 0..w {
        for r in 0..h {
            for s in 0..dd {
                if masked_out(g, err, mask, c, r, s)? {
                    continue;
                }
                for f in 0..dim_real {
                    let v = widen(mri_get_vox_val(g, err, &pv, c, r, s, f, 0)?);
                    let q = div64(v, sqrt64(sum2[f as usize])) as f32;
                    mri_set_vox_val(g, err, &mut pv, c, r, s, f, q, 0)?;
                }
            }
        }
    }
    Ok((0, Some(Pca { u, s: ps, v: pv })))
}

/// `PrintPCAStats(fp, Spca)`: for each singular value `s_n` (n from 1), a line `%3d   %8.2f %9.2f
/// %6.2f  %6.2f ` of `n`, `v = s_n^2` (in `float`), the running sum of the `v` (`double`), and both
/// as percentages of the total (`100 v / total`).
pub fn print_pca_stats(fp: &mut dyn Write, spca: &Matrix) -> i32 {
    use larmorx_core::cfmt::{f, pad};
    let mut tot = 0.0f64;
    for n in 1..=spca.cols {
        let x = spca.relt(1, n);
        tot = add64(tot, widen(mul(x, x)));
    }
    let mut vsum = 0.0f64;
    for n in 1..=spca.cols {
        let x = spca.relt(1, n);
        let v = widen(mul(x, x));
        vsum = add64(vsum, v);
        let pv = div64(mul64(100.0, v), tot);
        let ps = div64(mul64(100.0, vsum), tot);
        let line = format!(
            "{}   {} {}   {}  {} \n",
            pad(&n.to_string(), 3),
            pad(&f(v, 2), 8),
            pad(&f(vsum, 2), 9),
            pad(&f(pv, 2), 6),
            pad(&f(ps, 2), 6)
        );
        let _ = fp.write_all(line.as_bytes());
    }
    0
}

/// `WritePCAStats(fname, Spca)`: [`print_pca_stats`] into the file `fname` (relative names in the
/// program's working directory `dir`), which FreeSurfer opens and never closes (its lines reach the
/// file when the program ends). A file that cannot be opened prints `ERROR: opening <fname>` on
/// standard output and gives 1; else 0.
pub fn write_pca_stats(out: &mut dyn Write, dir: Option<&Path>, fname: &str, spca: &Matrix) -> i32 {
    let path = match dir {
        Some(d) if !Path::new(fname).is_absolute() => d.join(fname),
        _ => Path::new(fname).to_path_buf(),
    };
    match std::fs::File::create(&path) {
        Ok(mut f) => {
            let mut buf = Vec::new();
            print_pca_stats(&mut buf, spca);
            let _ = f.write_all(&buf);
            0
        }
        Err(_) => {
            let _ = writeln!(out, "ERROR: opening {fname}");
            1
        }
    }
}

/// `doublecompar(a, b)` (`sig.cpp`): -1 when `a < b`, 0 when equal, else +1 (a NaN is greater than
/// everything, itself included).
pub fn doublecompar(a: &f64, b: &f64) -> i32 {
    if a < b {
        -1
    } else if a == b {
        0
    } else {
        1
    }
}

/// `fdr2vwth(p, np, fdr)` (`sig.cpp`): `p` sorted ascending (the C library's `qsort` with
/// [`doublecompar`]); the last `p[n]` (from the end) below `(fdr / np) (n + 1)`, else `0.9 p[0]`.
pub fn fdr2vwth(p: &mut [f64], fdr: f64) -> f64 {
    let np = p.len();
    let r = div64(fdr, np as i32 as f64);
    qsort(p, doublecompar);
    for n in (0..np).rev() {
        if p[n] < mul64(r, (n as i32 + 1) as f64) {
            return p[n];
        }
    }
    mul64(0.9, p[0])
}

/// Where `MRIfdr2vwth` writes a volume's thresholded values: nowhere, a volume of the caller's,
/// or the input itself.
pub enum FdrOut<'a> {
    None,
    Given(&'a mut Mri),
    Same,
}

/// One input of `MRIfdr2vwth`: the volume, its frame, its mask and its output.
pub struct FdrVol<'a> {
    pub vol: &'a mut Mri,
    pub frame: i32,
    pub mask: Option<&'a Mri>,
    pub out: FdrOut<'a>,
}

/// A voxel's value as `MRIfdr2vwth` tests it: `None` when its sign is not the one asked for
/// (`signid` -1: not above 0; +1: not below 0), else its magnitude, or `10^-|v|` with `log10flag`.
fn fdr_value(libm: &Libm, v: f64, signid: i32, log10flag: bool) -> Option<f64> {
    if (signid == -1 && v > 0.0) || (signid == 1 && v < 0.0) {
        return None;
    }
    let a = v.abs();
    Some(if log10flag { libm.pow(10.0, -a) } else { a })
}

/// `MRIfdr2vwth(vollist, nvols, framelist, fdr, signid, log10flag, masklist, &vwth, ovollist,
/// debug)`: the voxel-wise threshold for the false discovery rate `fdr` over the `float` voxels of
/// every input's frame (read directly) inside its mask (not below 0.5) and of the sign asked for
/// ([`fdr_value`]), by [`fdr2vwth`]; with `log10flag` the values are `-log10(p)` and `vwth` comes
/// back as `-log10` of the threshold. Each output's frame 0 then gets the input's value where it
/// passes (mask, sign, `p` not above the threshold), else 1 (0 with `log10flag`). Messages
/// (standard output, then 1): `ERROR: MRIfdr2vwth: frame = <f>, must be < nframes = <n>`, `...:
/// input volume is not of type MRI_FLOAT`, `...: output volume is not of type MRI_FLOAT`, `...:
/// output/input dimension mismatch`, `...: mask/input dimension mismatch`, `...: no matching
/// voxels found`; with `debug`, `MRIfdr2vwth: np = <np>, nv = <nv>  sign=<s>` and `MRIfdr2vwth:
/// vwth = <%lf>, log10(vwhth) = <%lf>`. 0 otherwise.
#[allow(clippy::too_many_arguments)]
pub fn mri_fdr2vwth(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    vols: &mut [FdrVol],
    fdr: f64,
    signid: i32,
    log10flag: bool,
    vwth: &mut f64,
    debug: bool,
) -> Result<i32, Fatal> {
    const WHAT: &str = "MRIfdr2vwth";
    let same_dims = |a: &Mri, b: &Mri| {
        a.vg.width == b.vg.width && a.vg.height == b.vg.height && a.vg.depth == b.vg.depth
    };
    let mut nv = 0i32;
    for it in vols.iter() {
        let vol = &*it.vol;
        if vol.nframes <= it.frame {
            let _ = writeln!(
                out,
                "ERROR: MRIfdr2vwth: frame = {}, must be < nframes = {}",
                it.frame, vol.nframes
            );
            return Ok(1);
        }
        if vol.mtype != MRI_FLOAT {
            let _ = writeln!(
                out,
                "ERROR: MRIfdr2vwth: input volume is not of type MRI_FLOAT"
            );
            return Ok(1);
        }
        if let FdrOut::Given(o) = &it.out {
            if o.mtype != MRI_FLOAT {
                let _ = writeln!(
                    out,
                    "ERROR: MRIfdr2vwth: output volume is not of type MRI_FLOAT"
                );
                return Ok(1);
            }
            if !same_dims(vol, o) {
                let _ = writeln!(out, "ERROR: MRIfdr2vwth: output/input dimension mismatch");
                return Ok(1);
            }
        }
        if let Some(m) = it.mask
            && !same_dims(vol, m)
        {
            let _ = writeln!(out, "ERROR: MRIfdr2vwth: mask/input dimension mismatch");
            return Ok(1);
        }
        nv = nv.wrapping_add(
            vol.vg
                .width
                .wrapping_mul(vol.vg.height)
                .wrapping_mul(vol.vg.depth),
        );
    }
    let libm = Libm::new(g.mode());
    let mut p = Vec::new();
    for it in vols.iter() {
        let vol = &*it.vol;
        let d = i64::from(vol.vg.depth);
        for c in 0..vol.vg.width {
            for r in 0..vol.vg.height {
                for s in 0..vol.vg.depth {
                    if masked_out(g, err, it.mask, c, r, s)? {
                        continue;
                    }
                    let sl = i64::from(s) + i64::from(it.frame) * d;
                    let v = widen(
                        rows::read(g, WHAT, vol, sl, i64::from(r), i64::from(c), MRI_FLOAT).f(),
                    );
                    if let Some(x) = fdr_value(&libm, v, signid, log10flag) {
                        p.push(x);
                    }
                }
            }
        }
    }
    let np = p.len() as i32;
    if debug {
        let _ = writeln!(out, "MRIfdr2vwth: np = {np}, nv = {nv}  sign={signid}");
    }
    if np == 0 {
        let _ = writeln!(out, "ERROR: MRIfdr2vwth: no matching voxels found");
        note_glibc07(g, &libm, "pow");
        return Ok(1);
    }
    *vwth = fdr2vwth(&mut p, fdr);
    if debug {
        let l = -log10_glibc(*vwth);
        let _ = writeln!(
            out,
            "MRIfdr2vwth: vwth = {}, log10(vwhth) = {}",
            larmorx_core::cfmt::f(*vwth, 6),
            larmorx_core::cfmt::f(l, 6)
        );
    }
    let valnull: f32 = if log10flag { 0.0 } else { 1.0 };
    let th = *vwth;
    for it in vols.iter_mut() {
        let d = i64::from(it.vol.vg.depth);
        let (w, h, dd) = (it.vol.vg.width, it.vol.vg.height, it.vol.vg.depth);
        if matches!(it.out, FdrOut::None) {
            continue;
        }
        for c in 0..w {
            for r in 0..h {
                for s in 0..dd {
                    let (rr, cc) = (i64::from(r), i64::from(c));
                    let sl = i64::from(s) + i64::from(it.frame) * d;
                    let keep = if masked_out(g, err, it.mask, c, r, s)? {
                        None
                    } else {
                        let raw = rows::read(g, WHAT, it.vol, sl, rr, cc, MRI_FLOAT).f();
                        match fdr_value(&libm, widen(raw), signid, log10flag) {
                            Some(x) if !(x > th) => Some(raw),
                            _ => None,
                        }
                    };
                    let v = Num::F(keep.unwrap_or(valnull));
                    let o: &mut Mri = match &mut it.out {
                        FdrOut::Given(o) => &mut **o,
                        _ => &mut *it.vol,
                    };
                    rows::write(g, WHAT, o, i64::from(s), rr, cc, MRI_FLOAT, v);
                }
            }
        }
    }
    if log10flag {
        *vwth = -log10_glibc(*vwth);
    }
    note_glibc07(g, &libm, "pow");
    Ok(0)
}
