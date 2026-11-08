// SPDX-License-Identifier: Apache-2.0
//! The registration of one volume to a base (`specs/3dvolreg.md` §7): the basis (the weight,
//! its edging and trimming box, the six derivative images, the seven-vector weighted least
//! squares) and the iterations.
//!
//! The basis is built once per pass; the derivative images' twelve resamplings run in
//! parallel. A fit reads the trimming box straight from the resampled volume, its sums in
//! double in storage order (as the spec's), so a volume's fit does not depend on the thread
//! count.

use rayon::prelude::*;

use crate::automask::{cluster, morph};
use crate::cfmt;

use super::blur::{self, BlurSettings};
use super::rotate::{Resampler, Transform, Work};

/// The weight of a basis (§7.1).
#[derive(Clone, Debug)]
pub enum Weight {
    /// `|base|`, blurred, clipped at 2.5 % of its maximum, edged.
    Default,
    /// A weight volume used as given; `edged` for the first pass's (§8.5).
    Given { w: Vec<f32>, edged: bool },
    /// `-wtinp`: the input's first volume, processed as the default (no edging).
    Input(Vec<f32>),
}

/// The settings of a basis.
#[derive(Clone, Debug)]
pub struct BasisSettings {
    pub resampler: Resampler,
    /// The edge widths `ex, ey, ez` (§8.3).
    pub edging: [usize; 3],
    /// The derivative step `dfac` (voxels).
    pub dfac: f32,
    pub sinit: f32,
    /// The internal angle axes `a1, a2, a3`.
    pub axes: [usize; 3],
    pub blur: BlurSettings,
    /// `-verbose -verbose`: the basis' messages.
    pub chatty: bool,
}

/// A basis that cannot fit: the messages AFNI prints before it fails (§7.3).
#[derive(Clone, Debug, PartialEq)]
pub enum BasisError {
    /// One slice: AFNI's "cannot use nD images".
    NotThreeD,
    /// The least squares could not be set up (an all-zero reference vector, a failed Cholesky).
    LeastSquares,
}

/// What the basis setup printed, in order (`edt_blur` is reported separately: it is printed
/// once per run).
#[derive(Clone, Debug, Default)]
pub struct SetupLog {
    pub lines: Vec<String>,
    /// The `edt_blur` line of the weight's blur, if this basis blurred one.
    pub blur_line: Option<String>,
    /// Where the blur line falls among `lines` (it is printed at most once per run).
    pub blur_at: usize,
}

/// The inclusive box of the fit (§7.1, trimming).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bounds {
    pub lo: [usize; 3],
    pub hi: [usize; 3],
}

impl Bounds {
    fn full(n: [usize; 3]) -> Bounds {
        Bounds {
            lo: [0; 3],
            hi: [n[0] - 1, n[1] - 1, n[2] - 1],
        }
    }

    fn count(&self) -> usize {
        (0..3).map(|a| self.hi[a] + 1 - self.lo[a]).product()
    }

    /// The box's voxels of `v` (a volume of `n`), in storage order.
    fn cut(&self, v: &[f32], n: [usize; 3]) -> Vec<f32> {
        let mut out = Vec::with_capacity(self.count());
        for k in self.lo[2]..=self.hi[2] {
            for j in self.lo[1]..=self.hi[1] {
                let at = n[0] * (j + n[1] * k);
                out.extend_from_slice(&v[at + self.lo[0]..=at + self.hi[0]]);
            }
        }
        out
    }
}

/// A basis ready to fit volumes.
pub struct Basis {
    pub n: [usize; 3],
    pub sizes: [f32; 3],
    pub bounds: Bounds,
    /// The seven reference vectors times the weight, over the box.
    refs: Vec<Vec<f32>>,
    /// The Cholesky factor of the normal matrix (lower, row by row).
    chol: [[f64; 7]; 7],
    pub settings: BasisSettings,
}

/// The `edt_blur` line of a blur (§6.3).
pub fn blur_line(sigma: f32, delta: f32, factor: f64, m: usize) -> String {
    format!(
        "edt_blur: sigx {}, dx {}, fwhm {}, sfac {}, firx {m}",
        cfmt::g32(sigma),
        cfmt::g32(delta),
        cfmt::g(f64::from(sigma) / 0.4246609),
        cfmt::g(factor),
    )
}

/// The default weight's processing of `v` (§7.1 items 1 and 3): absolute value, the blur of
/// σ = 3 voxels per axis, values below 2.5 % of the maximum cleared. Returns the blur's line.
fn default_weight(v: &mut [f32], n: [usize; 3], sizes: [f32; 3], s: &BlurSettings) -> String {
    for x in v.iter_mut() {
        *x = x.abs();
    }
    let sigma = sizes.map(|d| (3.0 * f64::from(d)) as f32);
    let factor = s.main_factor();
    let m = blur::half_width(sigma[0], sizes[0], factor);
    let line = blur_line(sigma[0], sizes[0], factor, m);
    blur::fir(v, n, sigma, sizes, factor, false, s);
    clip_small(v, 0.025);
    line
}

/// Clears every value below `frac` of the maximum (the threshold in double, rounded to
/// float32; a value equal to it stays).
pub fn clip_small(v: &mut [f32], frac: f64) {
    let max = v.iter().fold(f32::NEG_INFINITY, |m, &x| m.max(x));
    let thr = (frac * f64::from(max)) as f32;
    for x in v.iter_mut() {
        if *x < thr {
            *x = 0.0;
        }
    }
}

/// Zeroes the weight within `e` planes of every face (§7.1, edging).
fn edge(w: &mut [f32], n: [usize; 3], e: [usize; 3]) {
    let [nx, ny, nz] = n;
    for k in 0..nz {
        for j in 0..ny {
            for i in 0..nx {
                let near = |x: usize, e: usize, n: usize| x < e || x + e >= n;
                if near(i, e[0], nx) || near(j, e[1], ny) || near(k, e[2], nz) {
                    w[i + nx * (j + ny * k)] = 0.0;
                }
            }
        }
    }
}

/// The trimming box of a weight (§7.1, trimming), with AFNI's messages.
fn trim_box(w: &[f32], n: [usize; 3], log: &mut Vec<String>) -> Bounds {
    let mut mask: Vec<u8> = w.iter().map(|&x| u8::from(x != 0.0)).collect();
    cluster::keep_largest(&mut mask, n);
    let stencil = morph::Stencil::new(n, morph::Neighbourhood::Edges.offsets());
    morph::peel_and_restore(&mut mask, &stencil, 1, 17);
    cluster::keep_largest(&mut mask, n);
    let [nx, ny, nz] = n;
    let mut lo = [usize::MAX; 3];
    let mut hi = [0usize; 3];
    let mut any = false;
    for k in 0..nz {
        for j in 0..ny {
            for i in 0..nx {
                if mask[i + nx * (j + ny * k)] != 0 {
                    any = true;
                    for (a, c) in [i, j, k].into_iter().enumerate() {
                        lo[a] = lo[a].min(c);
                        hi[a] = hi[a].max(c);
                    }
                }
            }
        }
    }
    if !any || hi == [0, 0, 0] {
        let lo = if any { lo } else { [0; 3] };
        log.push(format!(
            " + skipping empty trim, {{xyz}}{{a,b}} = {},{}, {},{}, {},{}\n",
            lo[0], hi[0], lo[1], hi[1], lo[2], hi[2]
        ));
        return Bounds::full(n);
    }
    let b = Bounds { lo, hi };
    let t = 100.0f32 * b.count() as f32 / (nx * ny * nz) as f32;
    if t < 90.0 {
        b
    } else {
        log.push(" + skipping use of trim - too little savings\n".into());
        Bounds::full(n)
    }
}

fn range(v: &[f32]) -> (f32, f32) {
    v.iter()
        .fold((f32::INFINITY, f32::NEG_INFINITY), |(a, b), &x| {
            (a.min(x), b.max(x))
        })
}

impl Basis {
    /// The basis of `base` (a volume of `n` voxels of sizes `sizes`) with `weight` (§7.1).
    #[allow(clippy::needless_range_loop)]
    pub fn new(
        base: &[f32],
        n: [usize; 3],
        sizes: [f32; 3],
        weight: Weight,
        settings: BasisSettings,
        log: &mut SetupLog,
    ) -> Result<Basis, BasisError> {
        if n[2] < 2 {
            return Err(BasisError::NotThreeD);
        }
        let chatty = settings.chatty;
        let mut lines = Vec::new();
        let mut blur_at = None;
        let mut w = match weight {
            Weight::Default => {
                if chatty {
                    lines.push("  initializing weight".to_owned());
                }
                let mut w = base.to_vec();
                blur_at = Some(lines.len());
                log.blur_line = Some(default_weight(&mut w, n, sizes, &settings.blur));
                if chatty {
                    lines.push(":".to_owned());
                }
                edge(&mut w, n, settings.edging);
                w
            }
            Weight::Input(mut w) => {
                if chatty {
                    lines.push("  processing weight".to_owned());
                }
                blur_at = Some(lines.len());
                log.blur_line = Some(default_weight(&mut w, n, sizes, &settings.blur));
                w
            }
            Weight::Given { mut w, edged } => {
                if edged {
                    edge(&mut w, n, settings.edging);
                }
                w
            }
        };
        let total = n[0] * n[1] * n[2];
        let all_zero = w.iter().all(|&x| x == 0.0);
        let bounds = trim_box(&w, n, &mut lines);
        if all_zero {
            lines.push("** error: weights are all zero\n".to_owned());
        }
        let wb = bounds.cut(&w, n);
        drop(std::mem::take(&mut w));
        if chatty {
            let kept = bounds.count();
            lines.push(format!(
                " + wtrim: [{}..{}]x[{}..{}]x[{}..{}] = {kept} voxels kept, out of {total} ({}%)\n",
                bounds.lo[0],
                bounds.hi[0],
                bounds.lo[1],
                bounds.hi[1],
                bounds.lo[2],
                bounds.hi[2],
                cfmt::f1(f64::from(100.0f32 * kept as f32 / total as f32)),
            ));
            let (lo, hi) = range(&wb);
            lines.push(format!(
                "  weight range: min={}  max={}\n",
                cfmt::g32(lo),
                cfmt::g32(hi)
            ));
            let zeros = wb.iter().filter(|&&x| x == 0.0).count();
            lines.push(format!(
                " # zero weights={zeros} out of {kept} ({}%)\n",
                cfmt::f1(f64::from(100.0f32 * zeros as f32 / kept as f32))
            ));
            lines.push(format!(
                "  mri_3dalign: using {} Mbytes of workspace\n",
                (10 * kept * 4) / (1 << 20)
            ));
        }
        // The derivative images, in the order θ1, θ2, θ3, δx, δy, δz.
        let dfac = settings.dfac;
        let eps_rot = (2.0 * f64::from(dfac) / (n[0] + n[1] + n[2]) as f64) as f32;
        let tfs: Vec<(Transform, Transform)> = (0..6)
            .map(|p| {
                let mk = |sign: f32| {
                    let mut theta = [0f32; 3];
                    let mut shift = [0f32; 3];
                    if p < 3 {
                        theta[p] = sign * eps_rot;
                    } else {
                        shift[p - 3] = sign * (dfac * sizes[p - 3]);
                    }
                    Transform::Angles {
                        theta,
                        axes: settings.axes,
                        shift,
                    }
                };
                (mk(1.0), mk(-1.0))
            })
            .collect();
        let r = settings.resampler;
        let names = ["d/d(th1)", "d/d(th2)", "d/d(th3)", "d/dx", "d/dy", "d/dz"];
        // Each derivative image from its two resamplings (parallel inside), trimmed at once:
        // one derivative at a time, so at most two full volumes in flight.
        let mut work = Work::default();
        let derivatives: Vec<(Vec<f32>, f32, f32)> = (0..6)
            .map(|p| {
                let work = &mut work;
                let (plus_tf, minus_tf) = &tfs[p];
                let mut plus = vec![0f32; total];
                r.apply(base, n, sizes, plus_tf, &mut plus, work);
                let mut minus = vec![0f32; total];
                r.apply(base, n, sizes, minus_tf, &mut minus, work);
                let kappa = if p < 3 {
                    (f64::from(settings.sinit) * 0.5 * (std::f64::consts::PI / 180.0)
                        / f64::from(eps_rot)) as f32
                } else {
                    settings.sinit * 0.5f32 / (dfac * sizes[p - 3])
                };
                for (pv, &qv) in plus.iter_mut().zip(&minus) {
                    *pv = kappa * (qv - *pv);
                }
                drop(minus);
                // The range over the fit's box (observed: AFNI reports the trimmed image's).
                let cut = bounds.cut(&plus, n);
                let (lo, hi) = if chatty { range(&cut) } else { (0.0, 0.0) };
                (cut, lo, hi)
            })
            .collect();
        let mut refs: Vec<Vec<f32>> = Vec::with_capacity(7);
        refs.push(bounds.cut(base, n));
        for (p, (d, lo, hi)) in derivatives.into_iter().enumerate() {
            if chatty {
                let eps = if p < 3 { eps_rot } else { dfac * sizes[p - 3] };
                lines.push(format!(
                    "  initializing {}[delta={}]",
                    names[p],
                    cfmt::g32(eps)
                ));
                lines.push(":".to_owned());
                lines.push(":".to_owned());
                lines.push(format!(
                    "  {} range: min={}  max={}\n",
                    names[p],
                    cfmt::g32(lo),
                    cfmt::g32(hi)
                ));
            }
            refs.push(d);
        }
        if chatty {
            lines.push("  initializing least squares\n".to_owned());
        }
        // §7.1 step 1: all-zero reference vectors.
        let zero = refs.iter().filter(|r| r.iter().all(|&x| x == 0.0)).count();
        let fail = |lines: &mut Vec<String>| {
            lines
                .push("** ERROR: mri_startup_lsqfit: bad call to startup_lsqfit!\x07\n".to_owned());
        };
        log.blur_at = blur_at.unwrap_or(0);
        if zero > 0 {
            let s = if zero == 1 { "" } else { "s" };
            lines.push(format!(
                "** ERROR: lsqfit :: {zero} vector{s} are all 0 for data fitting\n"
            ));
            fail(&mut lines);
            log.lines = lines;
            return Err(BasisError::LeastSquares);
        }
        // Step 2: the normal matrix, its sums in double in storage order.
        let mut c = [[0f64; 7]; 7];
        let pairs: Vec<(usize, usize)> = (0..7).flat_map(|i| (i..7).map(move |j| (i, j))).collect();
        let sums: Vec<f64> = pairs
            .par_iter()
            .map(|&(i, j)| {
                let (ri, rj) = (&refs[i], &refs[j]);
                let mut s = 0.0f64;
                for v in 0..wb.len() {
                    s += f64::from((ri[v] * rj[v]) * wb[v]);
                }
                s
            })
            .collect();
        for (&(i, j), &s) in pairs.iter().zip(&sums) {
            c[i][j] = s;
            c[j][i] = s;
        }
        // Step 3: Cholesky (the spec's index form).
        let mut l = [[0f64; 7]; 7];
        for i in 0..7 {
            for j in 0..i {
                let mut s = c[i][j];
                for k in 0..j {
                    s -= l[i][k] * l[j][k];
                }
                l[i][j] = s / l[j][j];
            }
            let mut s = c[i][i];
            for k in 0..i {
                s -= l[i][k] * l[i][k];
            }
            if s <= 0.0 {
                lines.push(format!(
                    "** ERROR: Choleski factorization failure in startup_lsqfit [row={i} sum={}]\n",
                    cfmt::g(s)
                ));
                fail(&mut lines);
                log.lines = lines;
                return Err(BasisError::LeastSquares);
            }
            l[i][i] = s.sqrt();
        }
        // Step 4: the references times the weight.
        refs.par_iter_mut().for_each(|r| {
            for (x, &wv) in r.iter_mut().zip(&wb) {
                *x *= wv;
            }
        });
        log.lines = lines;
        Ok(Basis {
            n,
            sizes,
            bounds,
            refs,
            chol: l,
            settings,
        })
    }

    /// The least-squares coefficients `(c0, …, c6)` of the volume `y` (untrimmed).
    pub fn fit(&self, y: &[f32]) -> [f32; 7] {
        let n = self.n;
        let b = self.bounds;
        let mut rho = [0f64; 7];
        let row_len = b.hi[0] + 1 - b.lo[0];
        let mut v = 0;
        for k in b.lo[2]..=b.hi[2] {
            for j in b.lo[1]..=b.hi[1] {
                let at = n[0] * (j + n[1] * k) + b.lo[0];
                let yr = &y[at..at + row_len];
                for (r, acc) in self.refs.iter().zip(rho.iter_mut()) {
                    let rr = &r[v..v + row_len];
                    let mut s = *acc;
                    for (&a, &x) in rr.iter().zip(yr) {
                        s += f64::from(a * x);
                    }
                    *acc = s;
                }
                v += row_len;
            }
        }
        let l = &self.chol;
        let mut z = [0f64; 7];
        for i in 0..7 {
            let mut s = rho[i];
            for k in 0..i {
                s -= l[i][k] * z[k];
            }
            z[i] = s / l[i][i];
        }
        let mut c = [0f64; 7];
        for i in (0..7).rev() {
            let mut s = z[i];
            for k in i + 1..7 {
                s -= l[k][i] * c[k];
            }
            c[i] = s / l[i][i];
        }
        c.map(|x| x as f32)
    }
}

/// The convergence settings of a pass (§7.2).
#[derive(Clone, Copy, Debug)]
pub struct Iteration {
    /// `x_t` (voxels).
    pub x_thresh: f32,
    /// `φ_t` (degrees).
    pub rot_thresh: f32,
    /// `M`.
    pub max_iter: i32,
}

/// The result of registering one volume: the internal parameters in degrees and mm.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Fitted {
    /// θ1, θ2, θ3 (degrees, float32) and δx, δy, δz (mm).
    pub params: [f32; 6],
    /// The angles converted to radians (float32), as the result of §7.2 step 4.
    pub radians: [f32; 3],
    pub iterations: i32,
}

/// `θ·(π/180)` rounded to float32.
pub fn to_radians(theta: f32) -> f32 {
    (f64::from(theta) * (std::f64::consts::PI / 180.0)) as f32
}

/// The float32 radians times `180/π` in double, rounded to float32.
pub fn to_degrees(rad: f32) -> f32 {
    (f64::from(rad) * (180.0 / std::f64::consts::PI)) as f32
}

fn line7(c: &[f32; 7]) -> String {
    c.iter()
        .map(|&x| cfmt::g_width(f64::from(x), 13, 6))
        .collect::<Vec<_>>()
        .join(" ")
}

impl Basis {
    /// The transformation of internal parameters `p` (degrees, mm).
    pub fn transform(&self, p: &[f32; 6]) -> Transform {
        Transform::Angles {
            theta: [to_radians(p[0]), to_radians(p[1]), to_radians(p[2])],
            axes: self.settings.axes,
            shift: [p[3], p[4], p[5]],
        }
    }

    /// Registers `y` (§7.2) from the initial values `init` (degrees, mm), with `chat` collecting
    /// `-verbose -verbose`'s fit lines.
    pub fn register(
        &self,
        y: &[f32],
        init: Option<[f32; 6]>,
        it: &Iteration,
        work: &mut Work,
        chat: Option<&mut String>,
    ) -> Fitted {
        let mut chat = chat;
        let thr: [f32; 6] = {
            let t = |d: f32| {
                if d == 0.0 {
                    it.x_thresh
                } else {
                    d * it.x_thresh
                }
            };
            [
                it.rot_thresh,
                it.rot_thresh,
                it.rot_thresh,
                t(self.sizes[0]),
                t(self.sizes[1]),
                t(self.sizes[2]),
            ]
        };
        let mut p;
        let iterate;
        match init {
            Some(v) if v.iter().any(|&x| x != 0.0) => {
                p = v;
                iterate = true;
                if let Some(c) = chat.as_deref_mut() {
                    let mut c7 = [1.0f32; 7];
                    c7[1..].copy_from_slice(&v);
                    c.push_str(&format!("\nFirst fit: {}", line7(&c7)));
                }
            }
            _ => {
                let c = self.fit(y);
                p = [c[1], c[2], c[3], c[4], c[5], c[6]];
                iterate = (0..6).any(|k| 10.0 * f64::from(c[k + 1].abs()) > f64::from(thr[k]));
                if let Some(s) = chat.as_deref_mut() {
                    s.push_str(&format!("\nFirst fit: {}", line7(&c)));
                }
            }
        }
        let mut iterations = 0;
        if iterate {
            let total = self.n[0] * self.n[1] * self.n[2];
            let mut moved = vec![0f32; total];
            loop {
                let tf = self.transform(&p);
                self.settings
                    .resampler
                    .apply(y, self.n, self.sizes, &tf, &mut moved, work);
                let c = self.fit(&moved);
                for k in 0..6 {
                    p[k] += c[k + 1];
                }
                iterations += 1;
                if let Some(s) = chat.as_deref_mut() {
                    s.push_str(&format!("\nDelta fit: {}", line7(&c)));
                    let mut t = [c[0]; 7];
                    t[1..].copy_from_slice(&p);
                    s.push_str(&format!("\nTotal fit: {}", line7(&t)));
                }
                let more = (0..6).any(|k| f64::from(c[k + 1].abs()) > f64::from(thr[k]));
                if iterations >= it.max_iter || !more {
                    break;
                }
            }
        }
        if let Some(s) = chat {
            s.push_str(&format!("\nIteration complete at {iterations} steps\n"));
        }
        Fitted {
            params: p,
            radians: [to_radians(p[0]), to_radians(p[1]), to_radians(p[2])],
            iterations,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::volreg::rotate::Trig;
    use crate::volreg::shift::Method;

    fn blob(n: [usize; 3], c: [f32; 3]) -> Vec<f32> {
        let mut v = vec![0f32; n[0] * n[1] * n[2]];
        for k in 0..n[2] {
            for j in 0..n[1] {
                for i in 0..n[0] {
                    let d = [i as f32 - c[0], j as f32 - c[1], k as f32 - c[2]];
                    let r2 = d[0] * d[0] / 30.0 + d[1] * d[1] / 20.0 + d[2] * d[2] / 12.0;
                    v[i + n[0] * (j + n[1] * k)] = 1000.0 / (1.0 + r2) + (i % 3) as f32;
                }
            }
        }
        v
    }

    #[test]
    fn a_shifted_blob_is_found() {
        let n = [24, 22, 16];
        let base = blob(n, [11.5, 10.5, 7.5]);
        let moved = blob(n, [12.0, 10.5, 7.5]);
        let settings = BasisSettings {
            resampler: Resampler {
                method: Method::Heptic,
                zpad: 4,
                trig: Trig::Correct,
            },
            edging: [1, 1, 1],
            dfac: 0.7,
            sinit: 1.0,
            axes: [2, 0, 1],
            blur: BlurSettings {
                firfac: None,
                old: false,
                trig: Trig::Correct,
            },
            chatty: false,
        };
        let mut log = SetupLog::default();
        let basis = Basis::new(&base, n, [2.0; 3], Weight::Default, settings, &mut log).unwrap();
        let it = Iteration {
            x_thresh: 0.01,
            rot_thresh: 0.02,
            max_iter: 23,
        };
        let f = basis.register(&moved, None, &it, &mut Work::default(), None);
        // The content moved by +0.5 voxel (1 mm) along x: registering moves it back.
        assert!((f.params[3] + 1.0).abs() < 0.05, "{:?}", f.params);
        assert!(f.params[4].abs() < 0.05 && f.params[5].abs() < 0.05);
    }
}
