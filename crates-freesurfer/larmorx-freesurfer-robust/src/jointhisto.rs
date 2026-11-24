// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `JointHisto` (queue 3.2-13): the joint histogram of two byte images that `mri_robust_register`'s
//! Powell registration builds for its information-theoretic costs (`--cost MI`, `NMI`, `ECC`,
//! `SCR`), translated from FreeSurfer 8.2.0's `mri_robust_register/JointHisto.cpp` (`create` with
//! the two half-way maps, `smooth`, `normalize`, `computeRCsums`, `computeMI`, `computeNMI`,
//! `computeECC`, `computeSCR`; the others have no caller in the program).
//!
//! [`JointHisto::create`]: 256 x 256 bins; over the target's grid (every `d`-th voxel, from 0 while
//! below `extent - d + 1`), the source sampled at `Msi * (x, y, z)` and the target at
//! `Mti * (x, y, z)` (trilinear, frame 0, both outside values set to -1 and left so; a sample of -1
//! skips the voxel), each pair's weight `(1 - s)(1 - t)`, `s(1 - t)`, `(1 - s)t`, `st` (`s`, `t` the
//! samples' fractional parts) added to its bin and the next ones (none past bin 255); a sample
//! whose bin is outside 0..255 prints `ERROR at ( x y z )`, the two points and the bins on standard
//! output and ends the program (status 1); no sample at all prints ` WARNING JointHisto::create :
//! histogram is empty (no overlap?) `. [`JointHisto::smooth`]: a Gaussian of 7 bins' full width at
//! half maximum (sigma `7 / sqrt(8 ln 2)`, 9 bins each side, `exp(-i^2 / sigma^2)` normalised)
//! along the rows, then the columns, the sum recomputed. The costs normalise the histogram (unless
//! its sum is 0 or 1), take its row and column sums and give mutual information (`sum P log2(P /
//! (Pr Pc))`), the normalised one (`(H(r) + H(c)) / H(r, c)`, -100 for an empty histogram), the
//! entropy correlation coefficient (`-2 MI / (sum Pr log2 Pr + sum Pc log2 Pc)`) and the symmetric
//! correlation ratio. Every sum runs in FreeSurfer's order; `log2` and `exp` are the call's
//! [`Libm`]'s (glibc's bits with `reproduce`, correctly rounded by default: GLIBC-07, quiet).

use std::io::Write;

use larmorx_core::math::Libm;
use larmorx_freesurfer_core::cstd::double_to_int_x86;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::sse::{add64, div64, mul64, sub64};
use larmorx_freesurfer_mri::mri::Mri;
use larmorx_freesurfer_volume::sample::ByteSampler;

use crate::costs::sample;
use crate::debug::Cout;
use crate::mymatrix::Mat4;

/// The number of bins per axis (`256 / dim`, `dim` 1).
const N: usize = 256;

/// `JointHisto`'s state: the histogram (row `i` the source's bin, column `j` the target's), its
/// sum and the row and column sums of the last cost.
pub struct JointHisto {
    histo: Vec<f64>,
    sum: f64,
    rowsum: Vec<f64>,
    colsum: Vec<f64>,
}

impl Default for JointHisto {
    fn default() -> JointHisto {
        JointHisto::new()
    }
}

/// `Msi * (u, v, w)`'s row `k` in the source's order: `((m0 u + m1 v) + m2 w) + m3`.
#[inline(always)]
fn row(m: &Mat4, k: usize, u: f64, v: f64, w: f64) -> f64 {
    add64(
        add64(
            add64(mul64(m[k][0], u), mul64(m[k][1], v)),
            mul64(m[k][2], w),
        ),
        m[k][3],
    )
}

/// `(int) floor(v)` as x86-64 converts it: a number from 0 below 2^31 truncated straight (its
/// floor), any other through `floor` and `cvttsd2si` (no call of libm's `floor` for the samples of
/// byte images, all at or above 0).
#[inline]
fn floor_int(v: f64) -> i32 {
    if (0.0..2_147_483_648.0).contains(&v) {
        v as i32
    } else {
        double_to_int_x86(v.floor())
    }
}

impl JointHisto {
    /// `JointHisto()`: 256 x 256 zeros.
    pub fn new() -> JointHisto {
        JointHisto {
            histo: vec![0.0; N * N],
            sum: 0.0,
            rowsum: vec![0.0; N],
            colsum: vec![0.0; N],
        }
    }

    #[inline(always)]
    fn at(&self, i: usize, j: usize) -> f64 {
        self.histo[i * N + j]
    }

    #[inline(always)]
    fn bump(&mut self, i: usize, j: usize, w: f64) {
        let h = &mut self.histo[i * N + j];
        *h = add64(*h, w);
    }

    /// `create(mriS, mriT, Msi, Mti, d1, d2, d3)`; `cout` is the standard output's state (its
    /// error lines print the points in it).
    #[allow(clippy::too_many_arguments)]
    pub fn create(
        &mut self,
        g: &mut Globals,
        out: &mut dyn Write,
        err: &mut dyn Write,
        s: &mut Mri,
        t: &mut Mri,
        msi: &Mat4,
        mti: &Mat4,
        d: [i32; 3],
        cout: Cout,
    ) -> Result<(), Fatal> {
        self.histo.iter_mut().for_each(|h| *h = 0.0);
        self.sum = 0.0;
        let nm1 = N - 1;
        let dt = [
            f64::from(t.vg.width),
            f64::from(t.vg.height),
            f64::from(t.vg.depth),
        ];
        s.outside_val = -1.0;
        t.outside_val = -1.0;
        let (bs, bt) = (ByteSampler::new(s), ByteSampler::new(t));
        let lim = |k: usize| dt[k] - f64::from(d[k]) + 1.0;
        let mut z = 0i32;
        while f64::from(z) < lim(2) {
            let mut y = 0i32;
            while f64::from(y) < lim(1) {
                let mut x = 0i32;
                while f64::from(x) < lim(0) {
                    let (ux, uy, uz) = (f64::from(x), f64::from(y), f64::from(z));
                    let xt = row(mti, 0, ux, uy, uz);
                    let yt = row(mti, 1, ux, uy, uz);
                    let zt = row(mti, 2, ux, uy, uz);
                    let xs = row(msi, 0, ux, uy, uz);
                    let ys = row(msi, 1, ux, uy, uz);
                    let zs = row(msi, 2, ux, uy, uz);
                    x += d[0];
                    let mut vs = 0.0;
                    sample(&bs, g, err, s, xs, ys, zs, 0, &mut vs);
                    if vs == -1.0 {
                        continue;
                    }
                    let ivs = floor_int(vs);
                    let mut vt = 0.0;
                    sample(&bt, g, err, t, xt, yt, zt, 0, &mut vt);
                    if vt == -1.0 {
                        continue;
                    }
                    let ivt = floor_int(vt);
                    if ivs < 0 || ivt < 0 || ivs >= N as i32 || ivt >= N as i32 {
                        let p = |v: f64| cout.os(v);
                        let _ = writeln!(out, "ERROR at ( {} {y} {z} ) ", x - d[0]);
                        let _ = writeln!(
                            out,
                            " PS: ( {} {} {} )  PT: ( {} {} {} )",
                            p(xs),
                            p(ys),
                            p(zs),
                            p(xt),
                            p(yt),
                            p(zt)
                        );
                        let _ = writeln!(out, " ivs: {ivs}  ivt: {ivt}  n: {N}");
                        return Err(Fatal::Exit(1));
                    }
                    let sdiff = sub64(vs, f64::from(ivs));
                    let tdiff = sub64(vt, f64::from(ivt));
                    let (is, it) = (ivs as usize, ivt as usize);
                    self.bump(is, it, mul64(sub64(1.0, sdiff), sub64(1.0, tdiff)));
                    if is < nm1 {
                        self.bump(is + 1, it, mul64(sdiff, sub64(1.0, tdiff)));
                    }
                    if it < nm1 {
                        self.bump(is, it + 1, mul64(sub64(1.0, sdiff), tdiff));
                    }
                    if is < nm1 && it < nm1 {
                        self.bump(is + 1, it + 1, mul64(sdiff, tdiff));
                    }
                    self.sum += 1.0;
                }
                y += d[1];
            }
            z += d[2];
        }
        if self.sum == 0.0 {
            let _ = writeln!(
                out,
                " WARNING JointHisto::create : histogram is empty (no overlap?) "
            );
        }
        Ok(())
    }

    /// `smooth(fwhm)`: see the module's notes (`RegPowell` passes 7).
    pub fn smooth(&mut self, libm: &Libm, fwhm: f64) {
        // `sqrt(8 * log(2))`: a constant GCC folds (log, product and root correctly rounded)
        let mut sm = fwhm / (8.0 * std::f64::consts::LN_2).sqrt();
        if sm < 0.001 {
            sm = 0.001;
        }
        let t = double_to_int_x86((3.0 * sm + 0.5).floor());
        let len = (2 * t + 1).max(0) as usize;
        let mut filter = vec![0.0f64; len];
        let mut fsum = 0.0f64;
        let sm2 = -sm * sm;
        for (k, i) in (-t..=t).enumerate() {
            filter[k] = libm.exp(f64::from(i * i) / sm2);
            fsum = add64(fsum, filter[k]);
        }
        for f in &mut filter {
            *f = div64(*f, fsum);
        }
        let t = t as isize;
        let n = N as isize;
        let mut htmp = vec![0.0f64; N * N];
        for i in 0..N {
            for j in 0..n {
                let mut dtmp = 0.0;
                for (count, k) in (j - t..=j + t).enumerate() {
                    if k < 0 || k >= n {
                        continue;
                    }
                    dtmp = add64(dtmp, mul64(self.at(i, k as usize), filter[count]));
                }
                htmp[i * N + j as usize] = dtmp;
            }
        }
        self.sum = 0.0;
        for i in 0..n {
            for j in 0..N {
                let mut dtmp = 0.0;
                for (count, k) in (i - t..=i + t).enumerate() {
                    if k < 0 || k >= n {
                        continue;
                    }
                    dtmp = add64(dtmp, mul64(htmp[k as usize * N + j], filter[count]));
                }
                self.histo[i as usize * N + j] = dtmp;
                self.sum = add64(self.sum, dtmp);
            }
        }
    }

    /// `normalize()`: the histogram divided by its sum, unless that is 0 or 1.
    fn normalize(&mut self) {
        if self.sum == 0.0 || self.sum == 1.0 {
            return;
        }
        let s = self.sum;
        for h in &mut self.histo {
            *h = div64(*h, s);
        }
        self.sum = 1.0;
    }

    /// `computeRCsums()`: `histo * ones(n, 1)` and `ones(1, n) * histo` (vnl's products: each
    /// sum from 0 in the index order).
    fn compute_rc_sums(&mut self) {
        for i in 0..N {
            let mut r = 0.0;
            for k in 0..N {
                r = add64(r, mul64(self.at(i, k), 1.0));
            }
            self.rowsum[i] = r;
        }
        for j in 0..N {
            let mut c = 0.0;
            for k in 0..N {
                c = add64(c, mul64(1.0, self.at(k, j)));
            }
            self.colsum[j] = c;
        }
    }

    /// `computeMI()`: `sum_{ij} P log2(P / (Pr_i Pc_j))` over the bins that are not 0.
    pub fn compute_mi(&mut self, libm: &Libm) -> f64 {
        self.normalize();
        self.compute_rc_sums();
        let mut mi = 0.0;
        for i in 0..N {
            for j in 0..N {
                let d = self.at(i, j);
                if d != 0.0 {
                    let q = div64(d, mul64(self.rowsum[i], self.colsum[j]));
                    mi = add64(mi, mul64(d, libm.log2(q)));
                }
            }
        }
        mi
    }

    /// `computeECC()`: `-2 MI / (sum_i Pr_i log2 Pr_i + sum_i Pc_i log2 Pc_i)`, the two sums and MI
    /// accumulated row by row as the source does.
    pub fn compute_ecc(&mut self, libm: &Libm) -> f64 {
        let mut msum = 0.0;
        let mut mi = 0.0;
        self.normalize();
        self.compute_rc_sums();
        for i in 0..N {
            if self.rowsum[i] != 0.0 {
                msum = add64(msum, mul64(self.rowsum[i], libm.log2(self.rowsum[i])));
            }
            if self.colsum[i] != 0.0 {
                msum = add64(msum, mul64(self.colsum[i], libm.log2(self.colsum[i])));
            }
            for j in 0..N {
                let d = self.at(i, j);
                if d != 0.0 {
                    let q = div64(d, mul64(self.rowsum[i], self.colsum[j]));
                    mi = add64(mi, mul64(d, libm.log2(q)));
                }
            }
        }
        div64(mul64(-2.0, mi), msum)
    }

    /// `computeNMI()`: `(sum Pr log2 Pr + sum Pc log2 Pc) / sum P log2 P`; -100 for an empty
    /// histogram.
    pub fn compute_nmi(&mut self, libm: &Libm) -> f64 {
        let mut s1 = 0.0;
        let mut s2 = 0.0;
        if self.sum == 0.0 {
            return -100.0;
        }
        self.normalize();
        self.compute_rc_sums();
        for i in 0..N {
            if self.rowsum[i] != 0.0 {
                s1 = add64(s1, mul64(self.rowsum[i], libm.log2(self.rowsum[i])));
            }
            if self.colsum[i] != 0.0 {
                s1 = add64(s1, mul64(self.colsum[i], libm.log2(self.colsum[i])));
            }
            for j in 0..N {
                let d = self.at(i, j);
                if d != 0.0 {
                    s2 = add64(s2, mul64(d, libm.log2(d)));
                }
            }
        }
        div64(s1, s2)
    }

    /// `computeSCR()`: the symmetric correlation ratio `CR(t | s) + CR(s | t)` (Roche et al.,
    /// 1998), each `1 - sum_i sum_j P (j - m_i)^2 / sigma^2` over the columns (rows) of non-zero
    /// sum.
    pub fn compute_scr(&mut self) -> f64 {
        self.normalize();
        self.compute_rc_sums();
        let (mut m1, mut m2) = (0.0, 0.0);
        for i in 0..N {
            let fi = i as f64;
            m1 = add64(m1, mul64(self.rowsum[i], fi));
            m2 = add64(m2, mul64(self.colsum[i], fi));
        }
        let (mut sig1, mut sig2) = (0.0, 0.0);
        for i in 0..N {
            let fi = i as f64;
            sig1 = add64(
                sig1,
                mul64(mul64(self.rowsum[i], sub64(fi, m1)), sub64(fi, m1)),
            );
            sig2 = add64(
                sig2,
                mul64(mul64(self.colsum[i], sub64(fi, m2)), sub64(fi, m2)),
            );
        }
        let mut sum1 = 0.0;
        for i in 0..N {
            if self.colsum[i] == 0.0 {
                continue;
            }
            let mut mi1 = 0.0;
            for j in 0..N {
                mi1 = add64(mi1, mul64(j as f64, self.at(j, i)));
            }
            mi1 = div64(mi1, self.colsum[i]);
            let mut sigi1 = 0.0;
            for j in 0..N {
                let dj = sub64(j as f64, mi1);
                sigi1 = add64(sigi1, mul64(mul64(self.at(j, i), dj), dj));
            }
            sum1 = add64(sum1, sigi1);
        }
        sum1 = div64(sum1, sig1);
        sum1 = sub64(1.0, sum1);
        let mut sum2 = 0.0;
        for i in 0..N {
            if self.rowsum[i] == 0.0 {
                continue;
            }
            let mut mi2 = 0.0;
            for j in 0..N {
                mi2 = add64(mi2, mul64(j as f64, self.at(i, j)));
            }
            mi2 = div64(mi2, self.rowsum[i]);
            let mut sigi2 = 0.0;
            for j in 0..N {
                let dj = sub64(j as f64, mi2);
                sigi2 = add64(sigi2, mul64(mul64(self.at(i, j), dj), dj));
            }
            sum2 = add64(sum2, sigi2);
        }
        sum2 = div64(sum2, sig2);
        sum2 = sub64(1.0, sum2);
        add64(sum1, sum2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use larmorx_core::defects::UpstreamDefects;

    /// A histogram of one bin: every cost's closed form.
    #[test]
    fn one_bin() {
        let libm = Libm::new(UpstreamDefects::Reproduce);
        let mut h = JointHisto::new();
        h.histo[3 * N + 5] = 2.0;
        h.sum = 2.0;
        assert_eq!(h.compute_mi(&libm), 0.0);
        let mut h = JointHisto::new();
        assert_eq!(h.compute_nmi(&libm), -100.0);
    }
}
