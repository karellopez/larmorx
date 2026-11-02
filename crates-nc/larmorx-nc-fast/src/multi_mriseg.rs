// SPDX-License-Identifier: LicenseRef-FSL
// Translated to Rust from FSL 6.0.7.23: fsl-fast4 2111.3 (multi_mriseg_two.h,
// multi_mriseg_two.cc). FAST4 - FMRIB's Automated Segmentation Tool v4, John Vickers, Mark
// Jenkinson and Steve Smith, FMRIB Image Analysis Group; Copyright (C) 2005-2007 University of
// Oxford. Rust translation Copyright 2026 Karel Lopez Vilaret; modified from the originals.
// Distributed under the FSL Licence (see LICENSE).
//! `ZMRIMULTISegmentation`: FAST's multi-channel segmentation (`-S n`, n of 2 or more), as FSL's
//! x86-64 build computes it.
//!
//! Each class is a multivariate Gaussian over the channels' log intensities (`log(v + 1)`), its
//! mean a row of `m_mean` and its covariance an `n × n` NEWMAT matrix (Armadillo 14.2.1 under
//! fsl-armawrap 0.7.0, `larmorx_nc_core::arma`). The expressions are evaluated on Armadillo's
//! paths: `row * inv(Σ) * col` is `(row Σ⁻¹) col` (`gemv_emul_tinysq` of `Σ⁻¹ᵀ` with the row
//! up to four channels, BLAS `dgemv('T', n, n)` above, then BLAS `dgemv_t` of the column with
//! it: OpenBLAS's Haswell kernel, whose tail rows fuse into multiply-adds), `row * Σ.i() * col`
//! is `row * solve(Σ, col)` (LAPACK `dgesv`, then the same `dgemv_t`), `Σ.i()` and
//! `Σ.Determinant()` Armadillo's `inv()` and `det()` (closed forms for 2 × 2 and 3 × 3, LAPACK
//! above). Up to eight channels are translated: LAPACK's paths are ported up to 8 × 8
//! (`larmorx_nc_core::lapack::MAX_N`).
//!
//! Unlike the single-channel segmentation, the main loop's MRF sweeps start from the current
//! posteriors (no random draw), the bias field is estimated per channel from the current
//! (bias-corrected) intensities and the classes are put in order by their means in the first
//! channel (the image given last on the command line) with FSL's own permutation (see
//! [`MultiSegmentation::qsort`]).
//!
//! The 4D volumes (`m_post`, `m_prob`, `m_pve`, the priors) hold `nClasses + 1` volumes, the
//! 0th unused, as FSL's; `members` holds `nClasses`.

use larmorx_core::defects::UpstreamDefects;
use larmorx_nc_core::arma::{self, Mat};
use larmorx_nc_core::cnum;
use larmorx_nc_core::glibc_double::{exp, log_compat as log};
use larmorx_nc_core::glibc_float::{expf, logf_compat as logf};
use larmorx_nc_core::lapack;
use larmorx_nc_core::miscmaths::{INV_ERROR, SOLVE_ERROR};
use larmorx_nc_core::newimagefns;
use larmorx_nc_core::volume::{FslError, Volume};

use crate::mriseg::{Io, read_floats};

/// `const float PI=3.14159265;`: the literal rounds to the float nearest to pi.
const PI: f32 = std::f32::consts::PI;

/// The most channels translated (LAPACK's paths are ported up to 8 × 8).
pub const MAX_CHANNELS: usize = 8;

/// What `TanakaCreate` is given (`segmentMultiChannel`'s call, in its own argument order).
#[derive(Clone, Debug)]
pub struct MultiParams {
    /// `-n`.
    pub nclasses: i32,
    /// `-I`: main-loop iterations with the bias field.
    pub nbiter: i32,
    /// `-l`: the bias field's smoothing (FWHM, mm).
    pub nblowpass: f32,
    /// `-f`: the initial MRF weight `beta`.
    pub fbeta: f32,
    /// 0: no priors; 1: priors to initialise; 2: priors throughout (`-P`).
    pub bapused: i32,
    /// Not `-N`.
    pub bias: bool,
    /// `-O`: main-loop iterations after the bias field.
    pub initfixed: i32,
    pub verbose: bool,
    /// The partial-volume steps (100; 0 with `--nopve`).
    pub iterationspve: i32,
    /// `-W`: segmentation-initialisation iterations.
    pub inititerations: i32,
    /// `-R`.
    pub mixeltype_mrf: f32,
    /// `-H`.
    pub hyper: f32,
    /// `-s`: the file of initial class intensities (`nClasses` per channel).
    pub mansegfile: String,
    /// `-t`.
    pub imagetype: i32,
    /// Threads for the bias field's convolutions (the results do not depend on them).
    pub threads: usize,
    /// D13's mode (FSL-98: the classes' sort).
    pub mode: UpstreamDefects,
    /// FSL-104 reproduced: the channels after the first were larger than channel 1 (each now
    /// its corner of channel 1's size); the first percentile of another channel throws
    /// newimage's size error, as FAST's does.
    pub size_error: bool,
}

/// An exception of Armadillo's (`std::runtime_error`), which nothing in FAST catches.
fn arma_error(what: &str) -> FslError {
    FslError {
        printed: Vec::new(),
        type_name: "std::runtime_error",
        what: what.to_owned(),
    }
}

/// `ZMRIMULTISegmentation` after `TanakaCreate`.
pub struct MultiSegmentation {
    p: MultiParams,
    nx: i64,
    ny: i64,
    nz: i64,
    nvox: usize,
    nc: usize,
    /// The number of channels.
    nch: usize,
    xdim: f32,
    ydim: f32,
    zdim: f32,
    /// `m_Mricopy[1..]`, `m_Mri[1..]`: one image per channel.
    mricopy: Vec<Vec<f32>>,
    mri: Vec<Vec<f32>>,
    /// `m_mask`: every channel above 0.
    mask: Vec<bool>,
    /// `m_post`, `m_prob`, `talpriors`: `nc + 1` volumes of `nvox` values.
    post: Vec<f32>,
    prob: Vec<f32>,
    talpriors: Vec<f32>,
    /// `members`: `nc` volumes.
    members: Vec<f32>,
    /// `m_pve`: `nc + 1` volumes.
    pve: Vec<f32>,
    /// `p_bias[1..]` (`m_Finalbias` after `takeexpo`).
    bias: Vec<Vec<f32>>,
    pub segment: Vec<i32>,
    pub pve_segment: Vec<i32>,
    pub hard_pv: Vec<i32>,
    /// FSL-98: the sort of the classes by channel 1's means was a permutation that is not its
    /// own inverse (FAST then gives each sorted class another class's statistics).
    pub sort_not_involution: bool,
    pv_prob: Vec<f32>,
    /// `m_mean(c, i)`: `nc` rows (classes 1 to nc) of `nch` values.
    mean: Vec<f64>,
    /// `m_co_variance[c]`, `m_inv_co_variance[c]` (index 0 unused).
    cov: Vec<Mat>,
    inv_cov: Vec<Mat>,
    weight: Vec<f32>,
    /// `rhs`: FSL's `new float[100]`, read only by `TanakaHyper` (a NaN `-H`), never written
    /// (`TanakaPriorHyper` fills it only for a negative `-H`, which `main` refuses): zeros here.
    rhs: Vec<f32>,
    volumequant: Vec<f64>,
    /// `MRFWeightsAM(l, m, n)` for the 27 neighbours in FSL's loop order (n, then m, then l).
    am: [f64; 27],
    beta: f32,
    kernels: [Vec<f64>; 3],
}

/// The neighbours `(l, m, n)` in the order of FSL's loops: `n` outermost, `l` innermost.
fn neighbours() -> [(i64, i64, i64); 27] {
    let mut out = [(0, 0, 0); 27];
    let mut k = 0;
    for n in -1..=1 {
        for m in -1..=1 {
            for l in -1..=1 {
                out[k] = (l, m, n);
                k += 1;
            }
        }
    }
    out
}

/// `gemv_emul_tinysq<true>`: `y = Mᵀ d` for a square `M` of `n` (1 to 4) rows, each element a
/// left-to-right sum of products (`(row * M)` for a row vector `d`).
#[inline]
fn tinysq_t(m: &[f64], n: usize, d: &[f64; MAX_CHANNELS]) -> [f64; MAX_CHANNELS] {
    let mut y = [0f64; MAX_CHANNELS];
    for r in 0..n {
        let mut acc = m[r * n] * d[0];
        for c in 1..n {
            acc += m[c + r * n] * d[c];
        }
        y[r] = acc;
    }
    y
}

/// OpenBLAS's `dgemv_t` (Haswell) for an `m × 1` matrix `a` and `alpha = 1`, `beta = 0`
/// (`larmorx_nc_core::lapack::blas::dgemv_t`, written out for one column): `y = aᵀ x`, the rows
/// in blocks of four summed in two lanes of two (`(a0 x0 + a2 x2) + (a1 x1 + a3 x3)`), the last
/// `m mod 4` rows fused (`fma(a0, x0, a1 x1)`, `fma(a2, x2, fma(a0, x0, a1 x1))`).
#[inline]
fn dgemv_t_col(a: &[f64; MAX_CHANNELS], x: &[f64; MAX_CHANNELS], m: usize) -> f64 {
    let mut y = 0f64;
    let m1 = m & !3;
    if m1 > 0 {
        let (mut l10, mut l9) = ([0f64; 2], [0f64; 2]);
        for r in 0..m1 {
            let p = a[r] * x[r];
            if r & 2 == 0 {
                l10[r & 1] += p;
            } else {
                l9[r & 1] += p;
            }
        }
        let s0 = l10[0] + l9[0];
        let s1 = l10[1] + l9[1];
        y = (s0 + s1).mul_add(1.0, y);
    }
    match m & 3 {
        0 => y,
        1 => a[m1].mul_add(x[m1], y),
        2 => y + a[m1].mul_add(x[m1], a[m1 + 1] * x[m1 + 1]),
        _ => {
            let s = a[m1].mul_add(x[m1], a[m1 + 1] * x[m1 + 1]);
            y + a[m1 + 2].mul_add(x[m1 + 2], s)
        }
    }
}

/// `(row * M * col).AsScalar()` with `row` and `col` the same vector `d` (Armadillo's
/// `(A B) C`).
#[inline]
fn quad(d: &[f64; MAX_CHANNELS], m: &[f64], n: usize) -> f64 {
    let y = if n <= 4 {
        tinysq_t(m, n, d)
    } else {
        // more than four channels: gemv<true> is BLAS dgemv('T', n, n)
        let mut y = [0f64; MAX_CHANNELS];
        let am = lapack::blas::MatRef { off: 0, ld: n };
        lapack::blas::dgemv_t(n, n, 1.0, m, am, &d[..n], 1, 0.0, &mut y[..n]);
        y
    };
    dgemv_t_col(d, &y, n)
}

/// `M_2PI(numberofchan)`: `(2 PI)^n` in `float`, one factor at a time.
fn m_2pi(n: usize) -> f32 {
    let mut val = 1.0f32;
    for _ in 0..n {
        val = val * 2.0 * PI;
    }
    val
}

/// What one class's `logGaussian` needs: `m_inv_co_variance[c]` and
/// `log(sqrt(abs(m_co_variance[c].Determinant() * M_2PI(n))))`.
struct ClassTerm {
    inv: Vec<f64>,
    lnorm: f64,
}

/// A mixel type's covariance at one fraction `delta` (`sig`, built in `double`), with what
/// `logpveGaussian` (its inverse and `log(sqrt(abs(det * M_2PI)))` in `float`) and `PVEnergy`
/// (its LU factors for `sig.i() * col`, and `log(det)` in `float`) need.
struct MixStep {
    delta: f32,
    mu: [f64; MAX_CHANNELS],
    sig: Mat,
    detsig: f32,
}

impl MixStep {
    fn new(seg: &MultiSegmentation, delta: f32, a: usize, b: usize) -> MixStep {
        let n = seg.nch;
        let mut mu = [0f64; MAX_CHANNELS];
        let mut sig = Mat::zeros(n, n);
        let w = 1.0f32 - delta;
        for i in 0..n {
            mu[i] = f64::from(delta) * seg.mean_at(a, i) + f64::from(w) * seg.mean_at(b, i);
            for j in 0..n {
                let v = f64::from(delta * delta) * seg.cov[a].at(i, j)
                    + f64::from(w * w) * seg.cov[b].at(i, j);
                sig.set(i, j, v);
            }
        }
        let detsig = arma::det(&sig) as f32;
        MixStep {
            delta,
            mu,
            sig,
            detsig,
        }
    }
}

/// The fractions `delta` of FSL's partial-volume loops: `for(float delta=0.0; delta<=1.0;
/// delta+=step)`, `step` in `float`.
fn deltas(step: f32) -> Vec<f32> {
    let mut out = Vec::new();
    let mut delta = 0f32;
    while f64::from(delta) <= 1.0 {
        out.push(delta);
        delta += step;
        if out.len() > 1 << 24 {
            break;
        }
    }
    out
}

/// The two classes of a mixel type of FSL's PV loops (1-based class indices).
fn mixel_pair(nc: usize, t: usize) -> (usize, usize) {
    if nc == 3 {
        match t {
            3 => (1, 2),
            4 => (1, 3),
            _ => (2, 3),
        }
    } else {
        (t - nc + 1, t - nc + 2)
    }
}

impl MultiSegmentation {
    /// `TanakaCreate(images, ...)`: `images[0]` is the first channel (the image given last).
    /// The channels share the first one's grid (the caller checks it).
    pub fn create(images: &[Volume], p: MultiParams) -> MultiSegmentation {
        let first = &images[0];
        let (nx, ny, nz) = (first.xsize(), first.ysize(), first.zsize());
        let nvox = (nx * ny * nz) as usize;
        let nc = p.nclasses.max(0) as usize;
        let nch = images.len();
        let mricopy: Vec<Vec<f32>> = images.iter().map(|v| v.data[..nvox].to_vec()).collect();
        let tal = if p.bapused > 0 { nvox * (nc + 1) } else { 0 };
        let beta = p.fbeta;
        MultiSegmentation {
            nx,
            ny,
            nz,
            nvox,
            nc,
            nch,
            xdim: first.xdim(),
            ydim: first.ydim(),
            zdim: first.zdim(),
            mri: Vec::new(),
            mricopy,
            mask: vec![false; nvox],
            post: vec![0.0; nvox * (nc + 1)],
            prob: vec![0.0; nvox * (nc + 1)],
            talpriors: vec![0.0; tal],
            members: vec![0.0; nvox * nc],
            pve: Vec::new(),
            bias: vec![vec![0.0; nvox]; nch],
            segment: vec![0; nvox],
            pve_segment: vec![0; nvox],
            hard_pv: Vec::new(),
            sort_not_involution: false,
            pv_prob: Vec::new(),
            mean: vec![0.0; nc * nch],
            cov: vec![Mat::zeros(nch, nch); nc + 1],
            inv_cov: vec![Mat::zeros(nch, nch); nc + 1],
            weight: Vec::new(),
            rhs: vec![0.0; 100],
            volumequant: vec![0.0; nc + 1],
            am: [0.0; 27],
            beta,
            kernels: [Vec::new(), Vec::new(), Vec::new()],
            p,
        }
    }

    /// `m_mean(c, i + 1)`.
    #[inline]
    fn mean_at(&self, c: usize, i: usize) -> f64 {
        self.mean[(c - 1) * self.nch + i]
    }

    #[inline]
    fn mean_mut(&mut self, c: usize, i: usize) -> &mut f64 {
        &mut self.mean[(c - 1) * self.nch + i]
    }

    /// `m_mask(x, y, z) == 1`, with the zero padding outside the image.
    #[inline]
    fn in_mask(&self, x: i64, y: i64, z: i64) -> bool {
        x >= 0
            && y >= 0
            && z >= 0
            && x < self.nx
            && y < self.ny
            && z < self.nz
            && self.mask[((z * self.ny + y) * self.nx + x) as usize]
    }

    /// The voxel indices of the 27 neighbours of `(x, y, z)` in FSL's order, `None` where the
    /// mask (zero outside the image) is not 1.
    fn neighbourhood(&self, x: i64, y: i64, z: i64) -> [Option<usize>; 27] {
        let mut out = [None; 27];
        for (k, (l, m, n)) in neighbours().into_iter().enumerate() {
            let (a, b, c) = (x + l, y + m, z + n);
            if self.in_mask(a, b, c) {
                out[k] = Some(((c * self.ny + b) * self.nx + a) as usize);
            }
        }
        out
    }

    /// `MRFWeightsInner(x, y, z, c)` over a neighbourhood: the `double` sum, in FSL's order,
    /// of `MRFWeightsAM(l, m, n) * m_post(neighbour, c)`.
    #[inline]
    fn mrf_inner(&self, nb: &[Option<usize>; 27], c: usize) -> f64 {
        let base = c * self.nvox;
        let mut inner = 0f64;
        for (k, idx) in nb.iter().enumerate() {
            if let Some(i) = *idx {
                inner += self.am[k] * f64::from(self.post[base + i]);
            }
        }
        inner
    }

    /// The channels' values at voxel `k` minus `mu` (`m_Mri[i](x, y, z) - mu(i)` in `double`).
    #[inline]
    fn centred(&self, k: usize, mu: impl Fn(usize) -> f64) -> [f64; MAX_CHANNELS] {
        let mut d = [0f64; MAX_CHANNELS];
        for i in 0..self.nch {
            d[i] = f64::from(self.mri[i][k]) - mu(i);
        }
        d
    }

    /// Each class's `logGaussian` terms (its inverse covariance and normalisation).
    fn class_terms(&self) -> Vec<ClassTerm> {
        let m2pi = f64::from(m_2pi(self.nch));
        (0..=self.nc)
            .map(|c| ClassTerm {
                inv: self.inv_cov[c].as_slice().to_vec(),
                lnorm: log((arma::det(&self.cov[c]) * m2pi).abs().sqrt()),
            })
            .collect()
    }

    /// `logGaussian(c, x, y, z)` at a voxel of the mask.
    #[inline]
    fn log_gaussian(&self, terms: &[ClassTerm], c: usize, k: usize) -> f32 {
        let d = self.centred(k, |i| self.mean_at(c, i));
        let sum = quad(&d, &terms[c].inv, self.nch) as f32;
        (0.5 * f64::from(sum) + terms[c].lnorm) as f32
    }

    /// `logpveGaussian(x, y, z, mu, invsig, detsig)` with `lnorm` its normalisation,
    /// `log(sqrt(abs(detsig * M_2PI(n))))` in `float`.
    #[inline]
    fn log_pve_gaussian(&self, k: usize, mu: &[f64; MAX_CHANNELS], inv: &[f64], lnorm: f32) -> f32 {
        let d = self.centred(k, |i| mu[i]);
        let sum = quad(&d, inv, self.nch) as f32;
        (0.5 * f64::from(sum) + f64::from(lnorm)) as f32
    }

    /// `log(sqrt(abs(detsig * M_2PI(n))))` in `float` (`logpveGaussian`'s normalisation).
    fn pve_lnorm(&self, detsig: f32) -> f32 {
        logf((detsig * m_2pi(self.nch)).abs().sqrt())
    }

    /// `TanakaMain(pcsf, pgm, pwm)`: the segmentation. `Ok(1)` is the k-means exception (its
    /// message printed); `Err` an exception nothing in FAST catches.
    pub fn main(&mut self, priors: Option<[&Volume; 3]>, io: &mut Io) -> Result<i32, FslError> {
        for k in 0..self.nvox {
            self.mask[k] = true;
            for i in 0..self.nch {
                let v = self.mricopy[i][k];
                if v > 0.0 {
                    self.mricopy[i][k] = log(f64::from(v) + 1.0) as f32;
                } else {
                    self.mricopy[i][k] = 0.0;
                    self.mask[k] = false;
                }
            }
        }
        self.dimensions();
        self.init_kernel();
        self.mri = self.mricopy.clone();
        self.weight = vec![1.0f32 / self.nc as f32; self.nc];
        if self.p.bapused == 0 {
            if !self.initialise(io)? {
                let _ = writeln!(
                    io.out,
                    "Exception: Not enough classes detected to init KMeans"
                );
                return Ok(1);
            }
        } else {
            let pr = priors.expect("the priors are read when they are used");
            self.init_simple(pr, io)?;
        }
        // Hyper < 0 (TanakaPriorHyper) is refused by fast's main.
        self.bias_removal()?;
        for iter in 0..self.p.nbiter {
            if self.p.verbose {
                let _ = writeln!(
                    io.out,
                    "Tanaka Iteration {iter} bias field {}",
                    self.p.nbiter
                );
            }
            self.tanaka_iterations(io);
            self.update_members_post();
            self.bias_removal()?;
            self.means_variances()?;
        }
        for iter in 0..self.p.initfixed {
            if self.p.verbose {
                let _ = writeln!(
                    io.out,
                    "Tanaka Iteration {iter} hyperparameter {}",
                    self.p.initfixed
                );
            }
            self.tanaka_iterations(io);
            self.update_members_post();
            if self.p.hyper >= 0.0 {
                self.beta = self.p.hyper;
            } else {
                self.tanaka_hyper();
            }
            if self.p.verbose {
                let _ = writeln!(io.out, " BETA {}", cnum::stream(f64::from(self.beta)));
            }
            self.means_variances()?;
        }
        // m_Finalbias = p_bias; takeexpo()
        for b in &mut self.bias {
            for v in b.iter_mut() {
                *v = expf(-*v);
            }
        }
        self.qsort();
        self.classification();
        self.update_members_post();
        if self.p.iterationspve > 0 {
            if self.p.verbose {
                let _ = writeln!(io.out, "Partial Volume Estimation ");
            }
            self.pv_classification_step()?;
            self.pv_estimation()?;
            self.volumes_quant(io);
            self.pve_classification();
        }
        Ok(0)
    }

    /// `Dimensions()`: the MRF weights, `1 / distance` with the distances in `float`.
    fn dimensions(&mut self) {
        let (x, y, z) = (self.xdim, self.ydim, self.zdim);
        let inv = |d: f32| (1.0 / f64::from(d)) as f32;
        let amz = inv(z);
        let amy = inv(y);
        let amzy = inv((z * z + y * y).sqrt());
        let amx = inv(x);
        let amzx = inv((z * z + x * x).sqrt());
        let amxy = inv((x * x + y * y).sqrt());
        for (k, (l, m, n)) in neighbours().into_iter().enumerate() {
            // MRFWeightsAM(l, m, n)
            let mut am = 0f32;
            if l == 0 {
                if m == 0 && n != 0 {
                    am = amz;
                }
                if m != 0 && n == 0 {
                    am = amy;
                }
                if m != 0 && n != 0 {
                    am = amzy;
                }
            }
            if m == 0 {
                if l != 0 && n == 0 {
                    am = amx;
                }
                if l != 0 && n != 0 {
                    am = amzx;
                }
            }
            if n == 0 && m != 0 && l != 0 {
                am = amxy;
            }
            self.am[k] = f64::from(am);
        }
    }

    /// `InitKernel()`: the bias field's Gaussian kernels, `sigma = sqrt(lowpass / dim)`.
    fn init_kernel(&mut self) {
        let lp = self.p.nblowpass;
        for (axis, dim) in [self.xdim, self.ydim, self.zdim].into_iter().enumerate() {
            let sigma = (lp / dim).sqrt();
            let radius = 2i32.wrapping_mul(cnum::f32_to_i32(sigma));
            self.kernels[axis] = newimagefns::gaussian_kernel_1d(sigma, radius);
        }
    }

    /// `Initialise()`: `WeightedKMeans`, the means, `Initclass`, `Classification`. `Ok(false)`
    /// is the k-means exception (two initial means of a channel equal).
    fn initialise(&mut self, io: &mut Io) -> Result<bool, FslError> {
        if !self.weighted_kmeans(io)? {
            return Ok(false);
        }
        self.update_members_post();
        self.means_variances()?;
        self.initclass();
        self.post.copy_from_slice(&self.prob);
        self.update_members_post();
        self.classification();
        Ok(true)
    }

    /// `WeightedKMeans()`.
    fn weighted_kmeans(&mut self, io: &mut Io) -> Result<bool, FslError> {
        let nc = self.nc;
        let nch = self.nch;
        let mut input_means: Vec<f32> = Vec::new();
        if !self.p.mansegfile.is_empty() {
            input_means = read_floats(&self.p.mansegfile);
        }
        self.post.fill(0.0);
        self.prob.fill(0.0);
        // m_mean.ReSize(noclasses + 1, numberofchannels); m_mean = 0.0
        self.mean = vec![0.0; nc * nch];
        for c in 1..=nc {
            self.cov[c] = Mat::zeros(nch, nch);
            self.inv_cov[c] = Mat::zeros(nch, nch);
        }
        let mut maskc = Volume::new(self.nx, self.ny, self.nz, 1);
        for (m, &b) in maskc.data.iter_mut().zip(&self.mask) {
            *m = if b { 1.0 } else { 0.0 };
        }
        let perc = (1.0 / f64::from((nc as f64 + 1.0) as f32)) as f32;
        for n in 0..nch {
            let mut chan = Volume::new(self.nx, self.ny, self.nz, 1);
            chan.data.copy_from_slice(&self.mricopy[n]);
            for c in 1..=nc {
                let v = if input_means.len() == nc * nch {
                    f64::from(logf(input_means[c + nc * n - 1]))
                } else if self.p.size_error && n > 0 {
                    return Err(FslError::imthrow(
                        "mask and vol have different sizes in calc_percentiles",
                        3,
                    ));
                } else {
                    let (v, _) = chan.percentile_masked(perc * c as f32, Some(&maskc))?;
                    f64::from(v)
                };
                *self.mean_mut(c, n) = v;
                if self.p.verbose {
                    let _ = writeln!(io.out, "{} {c} {}", n + 1, cnum::stream(v));
                }
            }
        }
        drop(maskc);
        for n in 0..nch {
            for c in 1..=nc {
                for c2 in 1..c {
                    if self.mean_at(c, n) == self.mean_at(c2, n) {
                        return Ok(false);
                    }
                }
            }
        }
        for k in 0..self.nvox {
            if !self.mask[k] {
                continue;
            }
            let mut minclass = 0;
            let mut mindist = 1e31f32;
            for c in 1..=nc {
                let mut sum = 0f32;
                for n in 0..nch {
                    for m in 0..nch {
                        let a = f64::from(self.mricopy[n][k]) - self.mean_at(c, n);
                        let b = f64::from(self.mricopy[m][k]) - self.mean_at(c, m);
                        sum = (f64::from(sum) + a * b) as f32;
                    }
                }
                if sum < mindist {
                    mindist = sum;
                    minclass = c;
                }
            }
            self.post[minclass * self.nvox + k] = 1.0;
        }
        for it in 0..self.p.inititerations {
            self.update_members_post();
            self.means_variances()?;
            if self.p.verbose {
                let _ = writeln!(io.out, "KMeans Iteration {it}");
            }
            self.initclass();
            self.post.copy_from_slice(&self.prob);
        }
        Ok(true)
    }

    /// `UpdateMembers(m_post)`: the weighted posteriors, normalised (0 where they sum to 0).
    fn update_members_post(&mut self) {
        let nc = self.nc;
        let nvox = self.nvox;
        for k in 0..nvox {
            if !self.mask[k] {
                continue;
            }
            let mut sum = 0f32;
            for c in 0..nc {
                let v = self.weight[c] * self.post[(c + 1) * nvox + k];
                self.members[c * nvox + k] = v;
                sum += v;
            }
            for c in 0..nc {
                let i = c * nvox + k;
                self.members[i] = if sum > 0.0 {
                    self.members[i] / sum
                } else {
                    0.0
                };
            }
        }
    }

    /// `MeansVariances(noclasses)`: each class's mean (`double` sums of `float` products) and
    /// covariance matrix from `members`, and the covariance's inverse (Armadillo's `inv()`,
    /// which throws on a singular matrix).
    fn means_variances(&mut self) -> Result<(), FslError> {
        let nc = self.nc;
        let nch = self.nch;
        let nvox = self.nvox;
        let mut normtemp = vec![0f32; nc + 1];
        for c in 1..=nc {
            self.cov[c] = Mat::zeros(nch, nch);
            for i in 0..nch {
                *self.mean_mut(c, i) = 0.0;
            }
        }
        for k in 0..nvox {
            if !self.mask[k] {
                continue;
            }
            for c in 1..=nc {
                let ppp = self.members[(c - 1) * nvox + k];
                for i in 0..nch {
                    let v = ppp * self.mri[i][k];
                    *self.mean_mut(c, i) += f64::from(v);
                }
                normtemp[c] += ppp;
            }
        }
        for c in 1..=nc {
            if normtemp[c] != 0.0 {
                for i in 0..nch {
                    let m = self.mean_at(c, i) / f64::from(normtemp[c]);
                    *self.mean_mut(c, i) = m;
                }
            }
            self.cov[c] = self.covariance_matrix(c);
            self.inv_cov[c] = arma::inv(&self.cov[c]).map_err(|_| arma_error(INV_ERROR))?;
        }
        Ok(())
    }

    /// `covariancematrix(c - 1, members)`: `Σ p (x_i - μ_i)(x_j - μ_j) / Σ p` (the products
    /// in `double`, the weights' sum in `float`).
    fn covariance_matrix(&self, c: usize) -> Mat {
        let nch = self.nch;
        let nvox = self.nvox;
        let mut cov = vec![0f64; nch * nch];
        let mut tot = 0f32;
        let p = &self.members[(c - 1) * nvox..c * nvox];
        for k in 0..nvox {
            if !self.mask[k] {
                continue;
            }
            let d = self.centred(k, |i| self.mean_at(c, i));
            let pk = f64::from(p[k]);
            for i in 0..nch {
                for j in 0..nch {
                    cov[i + j * nch] += pk * d[i] * d[j];
                }
            }
            tot += p[k];
        }
        let t = f64::from(tot);
        for v in &mut cov {
            *v /= t;
        }
        Mat::from_col_major(nch, nch, &cov)
    }

    /// `Initclass(noclasses)` into `m_prob`: each class's likelihood, normalised over the
    /// classes (0 outside the mask or where they sum to 0).
    fn initclass(&mut self) {
        let nc = self.nc;
        let nvox = self.nvox;
        let terms = self.class_terms();
        let mut vals = vec![0f32; nc + 1];
        for k in 0..nvox {
            let mut tot = 0f32;
            if self.mask[k] {
                for c in 1..=nc {
                    let v = exp(-f64::from(self.log_gaussian(&terms, c, k))) as f32;
                    vals[c] = v;
                    tot += v;
                }
            }
            for c in 1..=nc {
                self.prob[c * nvox + k] = if tot > 0.0 && self.mask[k] {
                    vals[c] / tot
                } else {
                    0.0
                };
            }
        }
    }

    /// `Classification()`: each voxel's class of largest weighted posterior (0 outside the
    /// mask).
    fn classification(&mut self) {
        for k in 0..self.nvox {
            let mut max = -1e-10f32;
            self.segment[k] = 0;
            if self.mask[k] {
                for c in 1..=self.nc {
                    let v = self.weight[c - 1] * self.post[c * self.nvox + k];
                    if v > max {
                        max = v;
                        self.segment[k] = c as i32;
                    }
                }
            }
        }
    }

    /// `mresmatrix()` and `BiasRemoval()`: each channel's bias field, the smoothed weighted
    /// residuals over the smoothed weights, 0 outside the mask; the channels corrected.
    fn bias_removal(&mut self) -> Result<(), FslError> {
        if !self.p.bias {
            return Ok(());
        }
        let nvox = self.nvox;
        let threads = self.p.threads;
        for ch in 0..self.nch {
            let mut resmean = Volume::new(self.nx, self.ny, self.nz, 1);
            let mut meaninvcov = Volume::new(self.nx, self.ny, self.nz, 1);
            for k in 0..nvox {
                if !self.mask[k] {
                    continue;
                }
                let (mut r, mut w) = (0f32, 0f32);
                for c in 1..=self.nc {
                    let temp_m = (f64::from(self.members[(c - 1) * nvox + k])
                        / self.cov[c].at(ch, ch)) as f32;
                    r = (f64::from(r)
                        + f64::from(temp_m) * (f64::from(self.mri[ch][k]) - self.mean_at(c, ch)))
                        as f32;
                    w += temp_m;
                }
                resmean.data[k] = r;
                meaninvcov.data[k] = w;
            }
            let [kx, ky, kz] = &self.kernels;
            let resmean = newimagefns::convolve_separable(&resmean, kx, ky, kz, threads);
            let meaninvcov = newimagefns::convolve_separable(&meaninvcov, kx, ky, kz, threads);
            let bias = &mut self.bias[ch];
            for k in 0..nvox {
                let (mut r, mut w) = (resmean.data[k], meaninvcov.data[k]);
                if !self.mask[k] || w == 0.0 {
                    r = 0.0;
                    w = 1.0;
                }
                bias[k] = if self.mask[k] { r / w } else { 0.0 };
            }
            for k in 0..nvox {
                self.mri[ch][k] = self.mricopy[ch][k] - bias[k];
            }
        }
        Ok(())
    }

    /// `MRFWeightsTotal()`.
    fn mrf_weights_total(&self) -> f32 {
        let mut total = 0f64;
        for z in 0..self.nz {
            for y in 0..self.ny {
                for x in 0..self.nx {
                    let k = ((z * self.ny + y) * self.nx + x) as usize;
                    if self.mask[k] {
                        let nb = self.neighbourhood(x, y, z);
                        for c in 1..=self.nc {
                            total +=
                                self.mrf_inner(&nb, c) * f64::from(self.post[c * self.nvox + k]);
                        }
                    }
                }
            }
        }
        total as f32
    }

    /// `TanakaHyper()`: the MRF weight whose total is closest to `rhs` (reached only with a
    /// NaN `-H`).
    fn tanaka_hyper(&mut self) {
        let total = self.mrf_weights_total();
        let mut betahtemp = 0f32;
        self.beta = betahtemp;
        let mut min = (total - self.rhs[0]).abs();
        for b in 1..15 {
            betahtemp += 0.05f32;
            if (total - self.rhs[b]).abs() < min {
                self.beta = betahtemp;
                min = (total - self.rhs[b]).abs();
            }
        }
    }

    /// `TanakaIterations()`: five sweeps of the MRF over the current posteriors (in place).
    fn tanaka_iterations(&mut self, io: &mut Io) {
        let nc = self.nc;
        let nvox = self.nvox;
        let terms = self.class_terms();
        let beta = self.beta;
        let mut vals = vec![0f32; nc + 1];
        for _ in 0..5 {
            for z in 0..self.nz {
                for y in 0..self.ny {
                    for x in 0..self.nx {
                        let k = ((z * self.ny + y) * self.nx + x) as usize;
                        if !self.mask[k] {
                            continue;
                        }
                        let nb = self.neighbourhood(x, y, z);
                        let mut sum = 0f32;
                        for c in 1..=nc {
                            let post = self.mrf_inner(&nb, c) as f32;
                            let mut v = expf(beta * post - self.log_gaussian(&terms, c, k));
                            if self.p.bapused >= 2 {
                                v *= self.talpriors[c * nvox + k];
                            }
                            vals[c] = v;
                            self.post[c * nvox + k] = v;
                            sum += v;
                        }
                        for c in 1..=nc {
                            self.post[c * nvox + k] = if sum > 0.0 { vals[c] / sum } else { 0.0 };
                        }
                    }
                }
            }
        }
        if self.p.verbose {
            for ch in 0..self.nch {
                let mut line = format!(" CHANNEL {}", ch + 1);
                for c in 1..=nc {
                    let m = self.mean_at(c, ch);
                    let s = self.cov[c].at(ch, ch).sqrt();
                    let sd = (exp(m + s) - exp(m - s)) / 2.0;
                    line.push_str(&format!(
                        " CLASS {c} MEAN {} STDDEV {}",
                        cnum::stream(exp(m)),
                        cnum::stream(sd)
                    ));
                }
                let _ = writeln!(io.out, "{line}");
            }
        }
    }

    /// The number of mixel types: 6 for three classes, `2 nClasses - 1` for two or more than
    /// three, none for one.
    fn mixnum(&self) -> usize {
        let nc = self.nc;
        if nc == 3 {
            6
        } else if nc == 2 || nc > 3 {
            2 * nc - 1
        } else {
            0
        }
    }

    /// The partial-volume step, `(float)(1.0f / (float)iterationspve)`.
    fn step(&self) -> f32 {
        1.0f32 / self.p.iterationspve as f32
    }

    /// `PVClassificationStep()`: the likelihood of each mixel type (the pure classes'
    /// Gaussians, the mixed types' integrated over the fraction), then `ICMPV()`.
    fn pv_classification_step(&mut self) -> Result<(), FslError> {
        let nc = self.nc;
        let nvox = self.nvox;
        let mixnum = self.mixnum();
        let step = self.step();
        self.pv_prob = vec![0.0; nvox * mixnum];
        // the pure classes
        if nc == 3 {
            let terms = self.class_terms();
            for k in 0..nvox {
                if self.mask[k] {
                    for t in 0..3 {
                        let lg = self.log_gaussian(&terms, t + 1, k);
                        self.pv_prob[t * nvox + k] = exp(-f64::from(lg)) as f32;
                    }
                }
            }
        } else if nc > 3 || nc == 2 {
            let npure = if nc == 2 { 2 } else { nc };
            for t in 0..npure {
                let c = t + 1;
                let mut mu = [0f64; MAX_CHANNELS];
                for (i, m) in mu.iter_mut().enumerate().take(self.nch) {
                    *m = self.mean_at(c, i);
                }
                let sig = self.cov[c].clone();
                let inv = arma::inv(&sig).map_err(|_| arma_error(INV_ERROR))?;
                let detsig = arma::det(&sig) as f32;
                let lnorm = self.pve_lnorm(detsig);
                for k in 0..nvox {
                    if self.mask[k] {
                        let lg = self.log_pve_gaussian(k, &mu, inv.as_slice(), lnorm);
                        self.pv_prob[t * nvox + k] = exp(-f64::from(lg)) as f32;
                    }
                }
            }
        }
        // the mixed types
        let first_mixed = if nc == 3 { 3 } else { nc };
        if nc >= 2 {
            let ds = deltas(step);
            for t in first_mixed..mixnum {
                let (a, b) = mixel_pair(nc, t);
                for &delta in &ds {
                    let s = MixStep::new(self, delta, a, b);
                    let inv = arma::inv(&s.sig).map_err(|_| arma_error(INV_ERROR))?;
                    let lnorm = self.pve_lnorm(s.detsig);
                    let base = t * nvox;
                    for k in 0..nvox {
                        if self.mask[k] {
                            let lg = self.log_pve_gaussian(k, &s.mu, inv.as_slice(), lnorm);
                            let g = exp(-f64::from(lg)) * f64::from(step);
                            self.pv_prob[base + k] = (f64::from(self.pv_prob[base + k]) + g) as f32;
                        }
                    }
                }
            }
        }
        self.icmpv();
        Ok(())
    }

    /// `ICMPV()`: the mixel types, by largest likelihood, then one sweep of iterated
    /// conditional modes with the mixel-type MRF (in place).
    fn icmpv(&mut self) {
        let nc = self.nc;
        let nvox = self.nvox;
        self.hard_pv = vec![0; nvox];
        let mixnum = self.mixnum();
        for k in 0..nvox {
            if self.mask[k] {
                let mut max = -1f32;
                for t in 0..mixnum {
                    let v = self.pv_prob[t * nvox + k];
                    if max < v {
                        self.hard_pv[k] = t as i32;
                        max = v;
                    }
                }
            }
        }
        let mut clique = vec![0f32; mixnum];
        let nbs = neighbours();
        let ncl = nc as i32;
        let mixl = mixnum as i32;
        for z in 0..self.nz {
            for y in 0..self.ny {
                for x in 0..self.nx {
                    let k = ((z * self.ny + y) * self.nx + x) as usize;
                    if !self.mask[k] {
                        continue;
                    }
                    clique.fill(0.0);
                    for (j, &(l, m, n)) in nbs.iter().enumerate() {
                        if !self.in_mask(x + l, y + m, z + n) {
                            continue;
                        }
                        let am = self.am[j] as f32;
                        let h = self.hard_pv
                            [(((z + n) * self.ny + (y + m)) * self.nx + (x + l)) as usize];
                        if nc == 3 {
                            for t in 0..6i32 {
                                let c = &mut clique[t as usize];
                                if t == h {
                                    *c += am * 2.0;
                                } else if (t == 0 && (h == 3 || h == 4))
                                    || (t == 1 && (h == 3 || h == 5))
                                    || (t == 2 && (h == 4 || h == 5))
                                    || (t == 3 && (h == 0 || h == 1))
                                    || (t == 4 && (h == 0 || h == 2))
                                    || (t == 5 && (h == 1 || h == 2))
                                {
                                    *c += am;
                                } else {
                                    *c -= am;
                                }
                            }
                        }
                        if nc > 3 {
                            for t in 0..mixl {
                                let c = &mut clique[t as usize];
                                if t == h {
                                    *c += am * 2.0;
                                } else if (0 < t
                                    && t < ncl - 1
                                    && (h == ncl + t - 1 || h == ncl + t))
                                    || (t == 0 && h == ncl)
                                    || (t == ncl - 1 && h == mixl - 1)
                                    || (t > ncl - 1 && (h == t - ncl || h == t - ncl + 1))
                                {
                                    *c += am;
                                } else {
                                    *c -= am;
                                }
                            }
                        }
                        if nc == 2 {
                            for t in 0..3i32 {
                                let c = &mut clique[t as usize];
                                if t == h {
                                    *c += am * 2.0;
                                } else if (t == 0 && h == 2)
                                    || (t == 1 && h == 2)
                                    || (t == 2 && (h == 0 || h == 1))
                                {
                                    *c += am;
                                } else {
                                    *c -= am;
                                }
                            }
                        }
                    }
                    let mut max = -1f32;
                    for t in 0..mixnum {
                        let p = self.pv_prob[t * nvox + k] * expf(self.p.mixeltype_mrf * clique[t]);
                        if max < p {
                            self.hard_pv[k] = t as i32;
                            max = p;
                        }
                    }
                }
            }
        }
    }

    /// `PVestimation()`: each voxel's partial volumes from its mixel type: 1 for a pure class,
    /// else the fraction of least `PVEnergy` (`row * sig.i() * col + log(det)`: Armadillo's
    /// `row * solve(sig, col)`).
    fn pv_estimation(&mut self) -> Result<(), FslError> {
        let nc = self.nc;
        let nvox = self.nvox;
        let n = self.nch;
        self.pve = vec![0.0; nvox * (nc + 1)];
        let mixnum = self.mixnum();
        let first_mixed = if nc == 3 { 3 } else { nc };
        if nc < 2 {
            // FSL's loops have no branch for one class: every partial volume stays 0.
            return Ok(());
        }
        let ds = deltas(self.step());
        // per mixed type and fraction: mu, LU factors of sig (None: singular), log(det)
        struct Energy {
            delta: f32,
            mu: [f64; MAX_CHANNELS],
            lu: Option<(Vec<f64>, [i32; MAX_CHANNELS])>,
            logdet: f32,
        }
        let mut table: Vec<Vec<Energy>> = Vec::new();
        for t in first_mixed..mixnum {
            let (a, b) = mixel_pair(nc, t);
            let row = ds
                .iter()
                .map(|&delta| {
                    let s = MixStep::new(self, delta, a, b);
                    let mut lu = s.sig.as_slice().to_vec();
                    let mut ipiv = [0i32; MAX_CHANNELS];
                    let ok = lapack::getrf(&mut lu, n, &mut ipiv) == 0;
                    Energy {
                        delta: s.delta,
                        mu: s.mu,
                        lu: ok.then_some((lu, ipiv)),
                        logdet: logf(s.detsig),
                    }
                })
                .collect();
            table.push(row);
        }
        for k in 0..nvox {
            if !self.mask[k] {
                continue;
            }
            let h = self.hard_pv[k];
            if h >= 0 && (h as usize) < nc {
                let h = h as usize;
                for c in 0..nc {
                    self.pve[(c + 1) * nvox + k] = if c == h { 1.0 } else { 0.0 };
                }
                continue;
            }
            let h = h as usize;
            if h >= mixnum {
                continue;
            }
            let (a, b) = mixel_pair(nc, h);
            let mut min = 1.0e13f32;
            for e in &table[h - first_mixed] {
                let d = self.centred(k, |i| e.mu[i]);
                let Some((lu, ipiv)) = &e.lu else {
                    return Err(arma_error(SOLVE_ERROR));
                };
                let mut x = d;
                lapack::getrs(lu, n, ipiv, &mut x[..n], 1);
                let val = (dgemv_t_col(&x, &d, n) + f64::from(e.logdet)) as f32;
                if min > val {
                    for c in 1..=nc {
                        self.pve[c * nvox + k] = 0.0;
                    }
                    self.pve[a * nvox + k] = e.delta;
                    self.pve[b * nvox + k] = (1.0 - f64::from(e.delta)) as f32;
                    min = val;
                }
            }
        }
        Ok(())
    }

    /// `Volumesquant(m_pve)`: each class's volume (mm^3), printed with `-v`.
    fn volumes_quant(&mut self, io: &mut Io) {
        let nvox = self.nvox;
        let mut tot = 0f64;
        for c in 1..=self.nc {
            let mut v = 0f64;
            for k in 0..nvox {
                if self.mask[k] {
                    v += f64::from(self.pve[c * nvox + k]);
                }
            }
            v *= f64::from(self.xdim);
            v *= f64::from(self.ydim);
            v *= f64::from(self.zdim);
            self.volumequant[c] = v;
            tot += v;
            if self.p.verbose {
                let _ = write!(io.out, "\n tissue {c} {}", cnum::stream(v));
            }
        }
        if self.p.verbose {
            let _ = write!(io.out, "\n total tissue {}\n", cnum::stream(tot));
        }
    }

    /// `pveClassification()`: each voxel's class of largest partial volume.
    fn pve_classification(&mut self) {
        for k in 0..self.nvox {
            let mut max = -1e-10f32;
            self.pve_segment[k] = 0;
            if self.mask[k] {
                for c in 1..=self.nc {
                    let v = self.pve[c * self.nvox + k];
                    if max < v {
                        max = v;
                        self.pve_segment[k] = c as i32;
                    }
                }
            }
        }
    }

    /// `qsort()`: the classes put in order by their mean in the first channel (reversed for
    /// `-t 2`). FSL's loop matches class `c`'s mean with the `m`-th smallest and then gives
    /// slot `c` the other channels' means, the covariance and the posteriors of class `m`
    /// (the inverse of the sorting permutation, the same for a swap or the identity), while
    /// slot `c`'s first-channel mean becomes the `c`-th smallest.
    fn qsort(&mut self) {
        let nc = self.nc;
        let nvox = self.nvox;
        let meancopy = self.mean.clone();
        if self.p.imagetype == 2 {
            for c in 1..=nc {
                *self.mean_mut(c, 0) *= -1.0;
            }
        }
        let mut tempmean: Vec<f64> = (1..=nc).map(|c| self.mean_at(c, 0)).collect();
        let varcopy = self.cov.clone();
        let invvarcopy = self.inv_cov.clone();
        let mut tempvar = self.cov.clone();
        let mut tempinvvar = self.inv_cov.clone();
        let postcopy = self.post.clone();
        let probcopy = self.prob.clone();
        if larmorx_nc_core::stdsort::sort_f64_checked(&mut tempmean).is_err() {
            tempmean.sort_by(|a, b| match (a.is_nan(), b.is_nan()) {
                (false, false) => a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal),
                (x, y) => x.cmp(&y),
            });
        }
        // slot c takes class m where m's channel-1 mean is the c-th smallest (FSL-98's fix);
        // FAST pairs them the other way round: slot c takes class m where c's mean is the m-th
        // smallest (the inverse permutation)
        let means1: Vec<f64> = (1..=nc).map(|c| self.mean_at(c, 0)).collect();
        let rank = |c: usize| (1..=nc).rfind(|&m| means1[c - 1] == tempmean[m - 1]);
        let source = |c: usize| (1..=nc).rfind(|&m| means1[m - 1] == tempmean[c - 1]);
        self.sort_not_involution = (1..=nc).any(|c| rank(c) != source(c));
        let fix = self.p.mode.fix();
        for c in 1..=nc {
            for m in 1..=nc {
                let matched = if fix {
                    means1[m - 1] == tempmean[c - 1]
                } else {
                    self.mean_at(c, 0) == tempmean[m - 1]
                };
                if matched {
                    for n in 1..self.nch {
                        *self.mean_mut(c, n) = meancopy[(m - 1) * self.nch + n];
                    }
                    tempvar[c] = varcopy[m].clone();
                    tempinvvar[c] = invvarcopy[m].clone();
                    for k in 0..nvox {
                        if self.mask[k] {
                            self.post[c * nvox + k] = postcopy[m * nvox + k];
                            self.prob[c * nvox + k] = probcopy[m * nvox + k];
                        }
                    }
                }
            }
        }
        for c in 1..=nc {
            *self.mean_mut(c, 0) = tempmean[c - 1];
            self.inv_cov[c] = tempinvvar[c].clone();
            self.cov[c] = tempvar[c].clone();
        }
        if self.p.imagetype == 2 {
            for c in 1..=nc {
                *self.mean_mut(c, 0) *= -1.0;
            }
        }
    }

    /// `InitSimple(pcsf, pgm, pwm)`: the initial posteriors from the tissue priors (2 or 3
    /// classes), then the members and the means.
    fn init_simple(&mut self, pr: [&Volume; 3], io: &mut Io) -> Result<(), FslError> {
        if self.p.verbose {
            let _ = writeln!(io.out, "Beginning prior-based initialisation");
        }
        let nvox = self.nvox;
        let nc = self.nc;
        let [csf, gm, wm] = pr;
        for v in [csf, gm, wm] {
            if v.data.len() < nvox {
                return Err(FslError::imthrow("Out of Bounds", 5));
            }
        }
        if nc == 3 || nc == 2 {
            for k in 0..nvox {
                let vals: Vec<f32> = if !self.mask[k] {
                    vec![0.0; nc]
                } else {
                    let norm2 = csf.data[k] + gm.data[k] + wm.data[k];
                    if norm2 > 0.0 {
                        if nc == 3 {
                            vec![csf.data[k] / norm2, gm.data[k] / norm2, wm.data[k] / norm2]
                        } else {
                            vec![csf.data[k] / norm2, (gm.data[k] + wm.data[k]) / norm2]
                        }
                    } else {
                        vec![(1.0 / f64::from(nc as f32)) as f32; nc]
                    }
                };
                for c in 1..=nc {
                    let i = c * nvox + k;
                    self.post[i] = vals[c - 1];
                    self.prob[i] = vals[c - 1];
                    self.talpriors[i] = vals[c - 1];
                }
            }
            self.classification();
        }
        self.update_members_post();
        self.means_variances()
    }

    /// `members[c]`, `m_pve[c]` as image volumes.
    /// The voxels outside the mask (FSL-96: `members` is never written there).
    pub fn outside_mask(&self) -> usize {
        self.mask.iter().filter(|&&m| !m).count()
    }

    pub fn member_volume(&self, c: usize) -> Vec<f32> {
        self.members[c * self.nvox..(c + 1) * self.nvox].to_vec()
    }

    pub fn pve_volume(&self, c: usize) -> Vec<f32> {
        self.pve[c * self.nvox..(c + 1) * self.nvox].to_vec()
    }

    /// `m_Finalbias[ch + 1]`: the channel's multiplicative correction, `exp(-bias)`.
    pub fn final_bias(&self, ch: usize) -> &[f32] {
        &self.bias[ch]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use larmorx_nc_core::lapack::blas::{MatRef, dgemv_t};

    /// A deterministic sequence of awkward doubles.
    fn values(n: usize, seed: u64) -> Vec<f64> {
        let mut s = seed;
        (0..n)
            .map(|_| {
                s = s
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                ((s >> 11) as f64 / (1u64 << 53) as f64 - 0.5) * 3.7
            })
            .collect()
    }

    #[test]
    fn quadratic_form_follows_armadillo() {
        for n in 1..=MAX_CHANNELS {
            for seed in 0..200u64 {
                let v = values(n * n + n, seed + 17 * n as u64);
                let m = Mat::from_col_major(n, n, &v[..n * n]);
                let mut d = [0f64; MAX_CHANNELS];
                d[..n].copy_from_slice(&v[n * n..]);
                let row = Mat::from_col_major(1, n, &d[..n]);
                let col = Mat::col(&d[..n]);
                let want = arma::mul3(&row, &m, &col).get(0);
                assert_eq!(quad(&d, m.as_slice(), n).to_bits(), want.to_bits(), "n {n}");
            }
        }
    }

    #[test]
    fn column_dgemv_is_openblas() {
        for n in 1..=MAX_CHANNELS {
            for seed in 0..200u64 {
                let v = values(2 * n, seed);
                let mut a = [0f64; MAX_CHANNELS];
                let mut x = [0f64; MAX_CHANNELS];
                a[..n].copy_from_slice(&v[..n]);
                x[..n].copy_from_slice(&v[n..]);
                let mut y = [0f64; 1];
                dgemv_t(
                    n,
                    1,
                    1.0,
                    &a[..n],
                    MatRef { off: 0, ld: n },
                    &x[..n],
                    1,
                    0.0,
                    &mut y,
                );
                assert_eq!(dgemv_t_col(&a, &x, n).to_bits(), y[0].to_bits(), "n {n}");
            }
        }
    }

    #[test]
    fn deltas_step_in_float() {
        let d = deltas(1.0f32 / 100.0);
        assert_eq!(d[0], 0.0);
        assert_eq!(d[1], 0.01f32);
        assert!(d.iter().all(|&v| f64::from(v) <= 1.0));
        assert!(d.len() >= 100 && d.len() <= 101);
    }
}
