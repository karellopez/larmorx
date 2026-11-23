// SPDX-License-Identifier: Apache-2.0
//! Frame covariance, principal components and the false discovery rate (spec §42):
//! `MRIcovarianceMatrix` and `MRIpca` (FS-483, FS-484), `PrintPCAStats`, `WritePCAStats`,
//! `fdr2vwth` and `MRIfdr2vwth` (GLIBC-07's `pow` and `log10`).

use larmorx_core::defects::{FS_483, FS_484, GLIBC_07};
use larmorx_core::glibc_qsort::qsort;
use larmorx_core::math;

use crate::clib::printf::sprintf_f64;
use crate::maths::Returned;
use crate::maths::matrix::{MATRIX_REAL, Matrix};
use crate::mri::{Mri, types};
use crate::process::{Process, Stop, codes};
use crate::sse;

/// `FLT_EPSILON` (§42's stop).
const FLT_EPSILON: f32 = f32::EPSILON;

/// The value of a `Returned` matrix, or none.
fn returned(r: Returned<Matrix>) -> Option<Matrix> {
    match r {
        Returned::New(m) => Some(m),
        _ => None,
    }
}

/// `fdr2vwth`'s comparator (§42): -1 below, 0 equal, else +1 (a NaN compares greater).
fn compare_p(a: &f64, b: &f64) -> i32 {
    if a < b {
        -1
    } else if a == b {
        0
    } else {
        1
    }
}

/// An output of `MRIfdr2vwth` (§42): none, the input itself, or a volume of its own.
pub enum FdrOut {
    None,
    Itself,
    Vol(Box<Mri>),
}

/// `!(x < t)` in C: true at or above `t`, and for a NaN.
fn not_below(x: f32, t: f32) -> bool {
    x.partial_cmp(&t) != Some(std::cmp::Ordering::Less)
}

/// `!(x > t)` in C: true at or below `t`, and for a NaN.
fn not_above<T: PartialOrd>(x: T, t: T) -> bool {
    x.partial_cmp(&t) != Some(std::cmp::Ordering::Greater)
}

/// `PrintPCAStats`' lines (§42) of a row vector of singular values.
fn pca_stats_lines(s: &Matrix) -> Vec<u8> {
    let n = s.cols.max(0);
    let v = |i: i32| sse::widen(sse::mul32(s.at(1, i), s.at(1, i)));
    let mut total = 0f64;
    for i in 1..=n {
        total = sse::add64(total, v(i));
    }
    let mut out = Vec::new();
    let mut sum = 0f64;
    for i in 1..=n {
        let x = v(i);
        sum = sse::add64(sum, x);
        out.extend_from_slice(format!("{i:3}   ").as_bytes());
        out.extend_from_slice(&sprintf_f64(b"%8.2f", x));
        out.push(b' ');
        out.extend_from_slice(&sprintf_f64(b"%9.2f", sum));
        out.extend_from_slice(b"   ");
        out.extend_from_slice(&sprintf_f64(
            b"%6.2f",
            sse::div64(sse::mul64(100.0, x), total),
        ));
        out.extend_from_slice(b"  ");
        out.extend_from_slice(&sprintf_f64(
            b"%6.2f",
            sse::div64(sse::mul64(100.0, sum), total),
        ));
        out.extend_from_slice(b" \n");
    }
    out
}

impl Process {
    /// A mask's two counts over the volume's extent (§42): FreeSurfer's (values above 0.5) and
    /// the voxels the sums take (values not below 0.5); FS-483 noted where they differ; the
    /// mode's count.
    fn pca_mask_count(&mut self, mri: &Mri, mask: &Mri) -> Result<i64, Stop> {
        let (w, h, d) = (mri.geom.width, mri.geom.height, mri.geom.depth);
        let (mut theirs, mut taken) = (0i64, 0i64);
        for c in 0..w {
            for r in 0..h {
                for s in 0..d {
                    let m = self.vf(mask, (c, r, s), 0)?;
                    if m > 0.5 {
                        theirs += 1;
                    }
                    if not_below(m, 0.5) {
                        taken += 1;
                    }
                }
            }
        }
        if theirs != taken {
            if self.mode().reproduce() {
                self.note(
                    &FS_483,
                    "MRIcovarianceMatrix: the voxels counted above 0.5 while the sums take those not below 0.5 (FreeSurfer's)",
                );
            } else {
                self.note(
                    &FS_483,
                    "MRIcovarianceMatrix: the count of the voxels the sums take (FreeSurfer counts those above 0.5 only)",
                );
            }
        }
        Ok(if self.mode().reproduce() {
            theirs
        } else {
            taken
        })
    }

    /// The count of `MRIcovarianceMatrix` and `MRIpca` (§42): the mask's in the mode, or the
    /// volume's voxels (32-bit).
    fn pca_count(&mut self, mri: &Mri, mask: Option<&Mri>) -> Result<i64, Stop> {
        match mask {
            Some(m) => self.pca_mask_count(mri, m),
            None => Ok(i64::from(
                mri.geom
                    .width
                    .wrapping_mul(mri.geom.height)
                    .wrapping_mul(mri.geom.depth),
            )),
        }
    }

    /// `MRIcovarianceMatrix(mri, mask)` (§42): the frames x frames matrix of the mean products of
    /// the frames over the voxels (with a mask those whose value is not below 0.5), or none
    /// (`ERROR: no voxels in mask`).
    pub fn mri_covariance_matrix(
        &mut self,
        mri: &Mri,
        mask: Option<&Mri>,
    ) -> Result<Option<Matrix>, Stop> {
        let count = self.pca_count(mri, mask)?;
        if count == 0 {
            self.put_out(b"ERROR: no voxels in mask\n");
            return Ok(None);
        }
        let nf = mri.nframes.max(0);
        let Some(mut m) = self.matrix_alloc(nf, nf, MATRIX_REAL)? else {
            return Ok(None);
        };
        let (w, h, d) = (mri.geom.width, mri.geom.height, mri.geom.depth);
        // the voxels the sums take
        let mut taken = Vec::new();
        for c in 0..w {
            for r in 0..h {
                for s in 0..d {
                    let ok = match mask {
                        Some(mk) => not_below(self.vf(mk, (c, r, s), 0)?, 0.5),
                        None => true,
                    };
                    if ok {
                        taken.push((c, r, s));
                    }
                }
            }
        }
        let n = count as f64;
        for f1 in 0..nf {
            for f2 in f1..nf {
                let mut sum = 0f64;
                for &(c, r, s) in &taken {
                    let v1 = f64::from(self.vf(mri, (c, r, s), f1)?);
                    let v2 = f64::from(self.vf(mri, (c, r, s), f2)?);
                    sum = sse::add64(sse::mul64(v2, v1), sum);
                }
                let x = sse::narrow(sse::div64(sum, n));
                m.set(f1 + 1, f2 + 1, x);
                m.set(f2 + 1, f1 + 1, x);
            }
        }
        Ok(Some(m))
    }

    /// `MRIpca(D, &U, &S, &V, mask)` (§42): the decomposed covariance `U`, the singular values
    /// `S` (a row vector) and the projections `V` (normalised per frame); 1 when the covariance
    /// is none. FS-484: a volume without frames ends the program in both modes.
    #[allow(clippy::type_complexity)]
    pub fn mri_pca(
        &mut self,
        d: &Mri,
        mask: Option<&Mri>,
    ) -> Result<(i32, Option<(Matrix, Matrix, Mri)>), Stop> {
        let count = self.pca_count(d, mask)?;
        let Some(cov) = self.mri_covariance_matrix(d, mask)? else {
            return Ok((1, None));
        };
        let nf = d.nframes.max(0);
        if nf == 0 {
            self.note(
                &FS_484,
                "MRIpca: a volume without frames: the program ended (FreeSurfer crashes in the SVD)",
            );
            self.error_exit(
                codes::BAD_PARM,
                b"MRIpca: a volume without frames (FreeSurfer crashes in the SVD here)\n",
            )?;
            return Ok((1, None));
        }
        let mut u = cov;
        let Some(mut s2) = self.matrix_alloc(1, nf, MATRIX_REAL)? else {
            return Ok((1, None));
        };
        self.matrix_svd(&mut u, &mut s2, None)?;
        let nvox = d
            .geom
            .width
            .wrapping_mul(d.geom.height)
            .wrapping_mul(d.geom.depth);
        let dim = nvox.min(nf);
        let Some(mut s) = self.matrix_alloc(1, nf, MATRIX_REAL)? else {
            return Ok((1, None));
        };
        let cf = count as f32;
        let mut k = dim;
        for c in 1..=dim {
            let x = sse::mul32(s2.at(1, c), cf);
            s2.set(1, c, x);
            s.set(1, c, sse::sqrt32(x));
            if x < FLT_EPSILON {
                k = c - 1;
                break;
            }
        }
        let k = k.max(1);
        let Some(mut wm) = self.matrix_alloc(nf, k, MATRIX_REAL)? else {
            return Ok((1, None));
        };
        for f in 1..=nf {
            for c in 1..=k {
                wm.set(f, c, sse::div32(u.at(f, c), sse::sqrt32(s2.at(1, c))));
            }
        }
        let ext = (d.geom.width, d.geom.height, d.geom.depth);
        let mut v = self.mri_alloc_sequence(ext.0, ext.1, ext.2, types::FLOAT, k)?;
        self.mri_copy_header(d, Some(&mut v))?;
        let mut sums = vec![0f64; k as usize];
        let mut row = Matrix::zeros(1, nf, MATRIX_REAL);
        let mut inside = Vec::new();
        for c in 0..ext.0 {
            for r in 0..ext.1 {
                for z in 0..ext.2 {
                    if let Some(mk) = mask
                        && self.vf(mk, (c, r, z), 0)? < 0.5
                    {
                        continue;
                    }
                    for f in 0..nf {
                        let x = self.vf(d, (c, r, z), f)?;
                        row.set(1, f + 1, x);
                    }
                    let Some(p) = returned(self.matrix_multiply(Some(&row), Some(&wm), None)?)
                    else {
                        continue;
                    };
                    for f in 0..k {
                        let x = p.at(1, f + 1);
                        self.mri_set_vox_val(&mut v, c, r, z, f, x, 0)?;
                        sums[f as usize] =
                            sse::add64(sse::widen(sse::mul32(x, x)), sums[f as usize]);
                    }
                    inside.push((c, r, z));
                }
            }
        }
        for f in 0..k {
            let root = sse::sqrt64(sums[f as usize]);
            for &(c, r, z) in &inside {
                let x = self.vf(&v, (c, r, z), f)?;
                let y = sse::narrow(sse::div64(f64::from(x), root));
                self.mri_set_vox_val(&mut v, c, r, z, f, y, 0)?;
            }
        }
        Ok((0, Some((u, s, v))))
    }

    /// `PrintPCAStats(stdout, S)` (§42): each value's square, the running sum and both as
    /// percentages of the total; 0.
    pub fn print_pca_stats(&mut self, s: &Matrix) -> i32 {
        let b = pca_stats_lines(s);
        self.put_out(&b);
        0
    }

    /// `WritePCAStats(fname, S)` (§42): `PrintPCAStats`' lines into the file (created or
    /// emptied); a file that cannot be opened prints `ERROR: opening <fname>` and gives 1.
    pub fn write_pca_stats(&mut self, fname: &[u8], s: &Matrix) -> i32 {
        let path = crate::files::path_of(fname);
        match std::fs::write(&path, pca_stats_lines(s)) {
            Ok(()) => 0,
            Err(e) => {
                self.errno = match e.kind() {
                    std::io::ErrorKind::NotFound => 2,
                    std::io::ErrorKind::PermissionDenied => 13,
                    _ => e.raw_os_error().unwrap_or(2),
                };
                let mut m = b"ERROR: opening ".to_vec();
                m.extend_from_slice(fname);
                m.push(b'\n');
                self.put_out(&m);
                1
            }
        }
    }

    /// `fdr2vwth(p, n, fdr)` (§42): `p` sorted ascending (glibc's merge order); from the last
    /// down, the first `p[i] < (fdr / n) (i + 1)`; none: `0.9 p[0]`.
    pub fn fdr2vwth(&mut self, p: &mut [f64], fdr: f64) -> f64 {
        let r = sse::div64(fdr, p.len() as f64);
        qsort(p, compare_p);
        for i in (0..p.len()).rev() {
            if p[i] < sse::mul64(r, (i + 1) as f64) {
                return p[i];
            }
        }
        sse::mul64(0.9, p.first().copied().unwrap_or(0.0))
    }

    /// A voxel's value as `MRIfdr2vwth` takes it (§42): `|v|`, or `pow(10, -|v|)` with `log10flag`.
    fn fdr_value(&mut self, v: f32, log10flag: bool) -> f64 {
        let a = f64::from(v.abs());
        if log10flag {
            self.libm_pow(10.0, -a)
        } else {
            a
        }
    }

    /// `-log10(x)` (§44's `log10`: glibc's with `reproduce`, correctly rounded by default;
    /// GLIBC-07 noted where the two differ).
    fn fdr_minus_log10(&mut self, x: f64) -> f64 {
        -self.libm("log10", x, math::log10, math::log10_glibc)
    }

    /// `MRIfdr2vwth(vols, n, frames, fdr, sign, log10flag, masks, &vwth, outs, debug)` (§42): the
    /// voxel-wise threshold of the false discovery rate over the inputs' values (masked, of the
    /// sign), into `vwth`; with outputs, each input's values that pass (else 1, 0 with
    /// `log10flag`) into frame 0 of its output. 1 after an error, else 0.
    #[allow(clippy::too_many_arguments)]
    pub fn mri_fdr2vwth(
        &mut self,
        vols: &mut [Mri],
        frames: &[i32],
        fdr: f64,
        sign: i32,
        log10flag: bool,
        masks: Option<&[Option<Mri>]>,
        vwth: &mut f64,
        outs: Option<&mut [FdrOut]>,
        debug: bool,
    ) -> Result<i32, Stop> {
        // every input checked first
        for i in 0..vols.len() {
            let v = &vols[i];
            if frames[i] >= v.nframes {
                self.put_out(
                    format!(
                        "ERROR: MRIfdr2vwth: frame = {}, must be < nframes = {}\n",
                        frames[i], v.nframes
                    )
                    .as_bytes(),
                );
                return Ok(1);
            }
            if v.kind != types::FLOAT {
                self.put_out(b"ERROR: MRIfdr2vwth: input volume is not of type MRI_FLOAT\n");
                return Ok(1);
            }
            let ext = (v.geom.width, v.geom.height, v.geom.depth);
            if let Some(o) = outs.as_deref()
                && let FdrOut::Vol(o) = &o[i]
            {
                if o.kind != types::FLOAT {
                    self.put_out(b"ERROR: MRIfdr2vwth: output volume is not of type MRI_FLOAT\n");
                    return Ok(1);
                }
                if (o.geom.width, o.geom.height, o.geom.depth) != ext {
                    self.put_out(b"ERROR: MRIfdr2vwth: output/input dimension mismatch\n");
                    return Ok(1);
                }
            }
            if let Some(ms) = masks
                && let Some(m) = &ms[i]
                && (m.geom.width, m.geom.height, m.geom.depth) != ext
            {
                self.put_out(b"ERROR: MRIfdr2vwth: mask/input dimension mismatch\n");
                return Ok(1);
            }
        }
        // the values
        let fits = |v: f32| match sign {
            -1 => not_above(v, 0.0),
            1 => not_below(v, 0.0),
            _ => true,
        };
        let mut p = Vec::new();
        let mut nv = 0i32;
        for i in 0..vols.len() {
            let v = &vols[i];
            let (w, h, d) = (v.geom.width, v.geom.height, v.geom.depth);
            nv = nv.wrapping_add(w.wrapping_mul(h).wrapping_mul(d));
            for c in 0..w {
                for r in 0..h {
                    for s in 0..d {
                        if let Some(ms) = masks
                            && let Some(m) = &ms[i]
                            && self.vf(m, (c, r, s), 0)? < 0.5
                        {
                            continue;
                        }
                        let x = self.vf(&vols[i], (c, r, s), frames[i])?;
                        if !fits(x) {
                            continue;
                        }
                        let val = self.fdr_value(x, log10flag);
                        p.push(val);
                    }
                }
            }
        }
        if debug {
            self.put_out(
                format!("MRIfdr2vwth: np = {}, nv = {nv}  sign={sign}\n", p.len()).as_bytes(),
            );
        }
        if p.is_empty() {
            self.put_out(b"ERROR: MRIfdr2vwth: no matching voxels found\n");
            return Ok(1);
        }
        let th = self.fdr2vwth(&mut p, fdr);
        *vwth = th;
        if debug {
            // GLIBC-07 only where the printed text differs
            let (a, b) = (-math::log10(th), -math::log10_glibc(th));
            let (ta, tb) = (sprintf_f64(b"%lf", a), sprintf_f64(b"%lf", b));
            let reproduce = self.mode().reproduce();
            if ta != tb {
                self.note(
                    &GLIBC_07,
                    if reproduce {
                        "glibc's log10, not correctly rounded, gave this result's last bit"
                    } else {
                        "log10 correctly rounded (glibc's differs in the last place)"
                    },
                );
            }
            let mut line = b"MRIfdr2vwth: vwth = ".to_vec();
            line.extend_from_slice(&sprintf_f64(b"%lf", th));
            line.extend_from_slice(b", log10(vwhth) = ");
            line.extend_from_slice(if reproduce { &tb } else { &ta });
            line.push(b'\n');
            self.put_out(&line);
        }
        let Some(outs) = outs else {
            if log10flag {
                *vwth = self.fdr_minus_log10(*vwth);
            }
            return Ok(0);
        };
        let other = if log10flag { 0.0 } else { 1.0 };
        for i in 0..vols.len() {
            if matches!(outs[i], FdrOut::None) {
                continue;
            }
            let (w, h, d) = (vols[i].geom.width, vols[i].geom.height, vols[i].geom.depth);
            for c in 0..w {
                for r in 0..h {
                    for s in 0..d {
                        let x = self.vf(&vols[i], (c, r, s), frames[i])?;
                        let masked = match masks {
                            Some(ms) => match &ms[i] {
                                Some(m) => self.vf(m, (c, r, s), 0)? < 0.5,
                                None => false,
                            },
                            None => false,
                        };
                        let pass = !masked && fits(x) && {
                            let val = self.fdr_value(x, log10flag);
                            not_above(val, *vwth)
                        };
                        let y = if pass { x } else { other };
                        match &mut outs[i] {
                            FdrOut::Itself => {
                                self.mri_set_vox_val(&mut vols[i], c, r, s, 0, y, 0)?;
                            }
                            FdrOut::Vol(o) => {
                                self.mri_set_vox_val(o, c, r, s, 0, y, 0)?;
                            }
                            FdrOut::None => {}
                        }
                    }
                }
            }
        }
        if log10flag {
            *vwth = self.fdr_minus_log10(*vwth);
        }
        Ok(0)
    }
}
