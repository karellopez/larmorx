// SPDX-License-Identifier: LicenseRef-FSL
// Translated to Rust from FSL 6.0.7.23: fsl-fast4 2111.3 (mriseg_two.h, mriseg_two.cc).
// FAST4 - FMRIB's Automated Segmentation Tool v4, John Vickers, Mark Jenkinson, Matthew
// Webster and Steve Smith, FMRIB Image Analysis Group; Copyright (C) 2005-2012 University of
// Oxford. Rust translation Copyright 2026 Karel Lopez Vilaret; modified from the originals.
// Distributed under the FSL Licence (see LICENSE).
//! `ZMRISegmentation`: FAST's single-channel segmentation (a hidden Markov random field fitted
//! by expectation-maximisation, with a bias field and partial-volume estimation), as FSL's
//! x86-64 build computes it: every `float` and `double` of the source in its own type (SSE, no
//! fused multiply-add), the C library's `exp`, `log`, `expf`, `logf` and `rand` as FSL's binary
//! binds them (`larmorx_nc_core::glibc_double`, `glibc_float`, `crand`), the loops in FSL's
//! order (the MRF and the mixel-type sweeps update in place, so a voxel sees the neighbours
//! already updated in the same sweep).
//!
//! The 4D volumes (`m_post`, `m_prob`, `m_pve`, the priors) hold `nClasses + 1` volumes, the
//! 0th unused, as FSL's; `members` holds `nClasses`.

use std::io::Write;

use larmorx_core::defects::UpstreamDefects;
use larmorx_nc_core::cnum;
use larmorx_nc_core::crand::{RAND_MAX, Rand};
use larmorx_nc_core::exception;
use larmorx_nc_core::glibc_double::{exp, log_compat as log};
use larmorx_nc_core::glibc_float::{expf, logf_compat as logf};
use larmorx_nc_core::newimagefns;
use larmorx_nc_core::stats::{self, EMPTY_MASK};
use larmorx_nc_core::volume::{FslError, Volume};

/// `const float PI=3.14159265;`: the literal rounds to the float nearest to pi.
const PI: f32 = std::f32::consts::PI;

/// What `TanakaCreate` is given (fast_two.cc's options).
#[derive(Clone, Debug)]
pub struct Params {
    /// `-f`: the initial MRF weight `beta`.
    pub fbeta: f32,
    /// `-n`.
    pub nclasses: i32,
    /// `-l`: the bias field's smoothing (FWHM, mm).
    pub nblowpass: f32,
    /// Not `-N`.
    pub bias: bool,
    /// The partial-volume steps (100; 0 with `--nopve`).
    pub iterationspve: i32,
    /// `-R`.
    pub mixeltype_mrf: f32,
    /// `-I`: main-loop iterations with the bias field.
    pub nbiter: i32,
    /// `-O`: main-loop iterations after the bias field.
    pub initfixed: i32,
    /// `-W`: segmentation-initialisation iterations.
    pub inititerations: i32,
    /// 0: no priors; 1: priors to initialise; 2: priors throughout (`-P`).
    pub bapused: i32,
    /// `-H`.
    pub hyper: f32,
    pub verbose: bool,
    /// `-s`: the file of initial class intensities.
    pub mansegfile: String,
    /// `-t`.
    pub imagetype: i32,
    /// Threads for the bias field's convolutions (the results do not depend on them).
    pub threads: usize,
    /// D13's mode (FSL-103: the mask's brightest voxels).
    pub mode: UpstreamDefects,
}

/// The standard output and error FAST writes to.
pub struct Io<'a> {
    pub out: &'a mut dyn Write,
    pub err: &'a mut dyn Write,
}

/// `ZMRISegmentation` after `TanakaCreate`.
pub struct Segmentation {
    p: Params,
    nx: i64,
    ny: i64,
    nz: i64,
    nvox: usize,
    nc: usize,
    xdim: f64,
    ydim: f64,
    zdim: f64,
    /// The image's properties (every output carries them).
    props: Volume,
    mricopy: Vec<f32>,
    mri: Vec<f32>,
    mask: Vec<f32>,
    /// `m_post`, `m_prob`, `talpriors`: `nc + 1` volumes of `nvox` values.
    post: Vec<f32>,
    prob: Vec<f32>,
    talpriors: Vec<f32>,
    /// `members`: `nc` volumes.
    pub members: Vec<f32>,
    /// `m_pve`: `nc + 1` volumes.
    pub pve: Vec<f32>,
    pub bias_field: Vec<f32>,
    pub segment: Vec<i32>,
    pub pve_segment: Vec<i32>,
    pub hard_pv: Vec<i32>,
    /// FSL-103: the voxels of the image's maximum FAST's mask leaves out (when the maximum
    /// plus 1 rounds to the maximum in `float`, at 2^24 and above): kept in by default.
    pub max_left_out: usize,
    /// FSL-96: whether `members` was ever written (FAST writes it in the k-means iterations and
    /// before the partial volumes only: with priors and `--nopve`, or no k-means iteration and
    /// `--nopve`, `-p` saves memory it never wrote, in the mask too).
    pub members_written: bool,
    pv_prob: Vec<f32>,
    mean: Vec<f32>,
    variance: Vec<f32>,
    weight: Vec<f32>,
    rhs: Vec<f32>,
    volumequant: Vec<f64>,
    /// `MRFWeightsAM(l, m, n)` for the 27 neighbours in FSL's loop order (n, then m, then l).
    am: [f64; 27],
    beta: f32,
    kernels: [Vec<f64>; 3],
    rand: Rand,
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

impl Segmentation {
    /// `TanakaCreate(image, ...)`.
    pub fn create(image: &Volume, p: Params) -> Segmentation {
        let (nx, ny, nz) = (image.xsize(), image.ysize(), image.zsize());
        let nvox = (nx * ny * nz) as usize;
        let nc = p.nclasses.max(0) as usize;
        let data = image.data[..nvox].to_vec();
        // m_mask = image; m_mask.binarise(0, m_mask.max() + 1, exclusive): every voxel above 0,
        // except (FSL-103) the maximum's when max + 1 rounds to max (2^24 and above); infinite
        // voxels are left out (max + 1 is infinite then, and so is max)
        let max = image.max();
        let upper = max + 1.0;
        let max_left_out = if upper == max && max.is_finite() {
            data.iter().filter(|&&v| v == max).count()
        } else {
            0
        };
        let keep = |v: f32| {
            if p.mode.fix() {
                v > 0.0 && v.is_finite()
            } else {
                v > 0.0 && v < upper
            }
        };
        let mask: Vec<f32> = data
            .iter()
            .map(|&v| if keep(v) { 1.0 } else { 0.0 })
            .collect();
        let mut props = image.clone();
        props.data = Vec::new();
        let tal = if p.bapused > 0 { nvox * (nc + 1) } else { 0 };
        let pve = if p.iterationspve > 0 { nvox } else { 0 };
        let beta = p.fbeta;
        Segmentation {
            nx,
            ny,
            nz,
            nvox,
            nc,
            xdim: f64::from(image.xdim()),
            ydim: f64::from(image.ydim()),
            zdim: f64::from(image.zdim()),
            props,
            mri: data.clone(),
            mricopy: data,
            mask,
            post: vec![0.0; nvox * (nc + 1)],
            prob: vec![0.0; nvox * (nc + 1)],
            talpriors: vec![0.0; tal],
            members: vec![0.0; nvox * nc],
            pve: Vec::new(),
            bias_field: vec![0.0; nvox],
            segment: vec![0; nvox],
            pve_segment: vec![0; pve],
            hard_pv: Vec::new(),
            max_left_out,
            members_written: false,
            pv_prob: Vec::new(),
            mean: Vec::new(),
            variance: Vec::new(),
            weight: Vec::new(),
            rhs: Vec::new(),
            volumequant: Vec::new(),
            am: [0.0; 27],
            beta,
            kernels: [Vec::new(), Vec::new(), Vec::new()],
            rand: Rand::new(u32::MAX),
            p,
        }
    }

    /// The image's properties, which every output carries.
    pub fn properties(&self) -> &Volume {
        &self.props
    }

    pub fn nvox(&self) -> usize {
        self.nvox
    }

    #[inline]
    fn at(&self, c: usize, k: usize) -> usize {
        c * self.nvox + k
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
            && self.mask[((z * self.ny + y) * self.nx + x) as usize] == 1.0
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

    /// `logGaussian(y, mu, sigmasq)`.
    #[inline]
    fn log_gaussian(y: f32, mu: f32, sigmasq: f32) -> f32 {
        let scaletemp = Self::log_scale(sigmasq);
        Self::log_gaussian_with(y, mu, sigmasq, scaletemp)
    }

    /// `logGaussian`'s first term, `1.0 * log(sqrt(2.0 * PI * sigmasq))` rounded to `float`.
    #[inline]
    fn log_scale(sigmasq: f32) -> f32 {
        (1.0 * log((2.0 * f64::from(PI) * f64::from(sigmasq)).sqrt())) as f32
    }

    /// `logGaussian` with its first term given.
    #[inline]
    fn log_gaussian_with(y: f32, mu: f32, sigmasq: f32, scaletemp: f32) -> f32 {
        let g = y - mu;
        let g = (0.5 * f64::from(g) * f64::from(g) / f64::from(sigmasq)) as f32;
        scaletemp + g
    }

    /// `TanakaMain(pcsf, pgm, pwm)`: the segmentation. `Ok(1)` is the k-means exception (its
    /// message printed); `Err` an exception nothing in FAST catches.
    pub fn main(&mut self, priors: Option<[&Volume; 3]>, io: &mut Io) -> Result<i32, FslError> {
        for v in &mut self.mricopy {
            *v = if *v > 0.0 {
                log(f64::from(*v) + 1.0) as f32
            } else {
                0.0
            };
        }
        self.mri = self.mricopy.clone();
        self.dimensions();
        self.rand = Rand::new(u32::MAX); // long seed=-1; srand(seed);
        let nc = self.nc;
        self.rhs = vec![0.0; 100];
        // volumequant.resize(nClasses + 1), then weight.resize(nClasses, 1): a negative size is
        // a huge one for std::vector
        let length_error = |what: &str| FslError {
            printed: Vec::new(),
            type_name: "std::length_error",
            what: what.to_owned(),
        };
        if self.p.nclasses < -1 {
            return Err(length_error("vector::_M_default_append"));
        }
        if self.p.nclasses < 0 {
            return Err(length_error("vector::_M_fill_insert"));
        }
        self.volumequant = vec![0.0; nc + 1];
        self.mean = vec![0.0; nc + 1];
        self.variance = vec![0.0; nc + 1];
        self.weight = vec![1.0; nc];
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
        // Hyper < 0 is refused by fast's main, so TanakaPriorHyper never runs.
        self.init_kernel();
        self.bias_removal(io)?;
        for n in 1..=nc {
            if self.mean[n].is_nan() || self.variance[n].is_nan() {
                let _ = writeln!(io.out, "MeaNsK variance nan");
            }
        }
        for iter in 0..self.p.nbiter {
            if self.p.verbose {
                let _ = writeln!(
                    io.out,
                    "Tanaka Iteration {iter} bias field {}",
                    self.p.nbiter
                );
            }
            self.tanaka_iterations(io);
            self.bias_removal(io)?;
            self.means_variances(io, false);
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
            if self.p.hyper >= 0.0 {
                self.beta = self.p.hyper;
            } else {
                self.tanaka_hyper(io);
            }
            if self.p.verbose {
                let _ = writeln!(io.out, " BETA {}", cnum::stream(f64::from(self.beta)));
            }
            self.means_variances(io, false);
        }
        self.qsort();
        self.classification();
        if self.p.iterationspve > 0 {
            if self.p.verbose {
                let _ = writeln!(io.out, " Starting Partial Volume Estimation ");
            }
            self.update_members_post();
            for k in 0..self.nvox {
                if self.mask[k] == 1.0 {
                    self.mri[k] = expf(self.mri[k]);
                }
            }
            self.means_variances(io, false);
            self.pv_classification_step();
            self.pv_estimation()?;
            self.calculate_volumes(io, true);
            self.pve_classification();
        } else {
            self.calculate_volumes(io, false);
        }
        if self.p.verbose {
            self.print_volume_totals(io);
        }
        for b in &mut self.bias_field {
            *b = expf(-*b);
        }
        Ok(0)
    }

    /// `Dimensions()`: the MRF weights, `1 / distance` as `float`.
    fn dimensions(&mut self) {
        let (x, y, z) = (self.xdim, self.ydim, self.zdim);
        let amz = (1.0 / z) as f32;
        let amy = (1.0 / y) as f32;
        let amzy = (1.0 / (z * z + y * y).sqrt()) as f32;
        let amx = (1.0 / x) as f32;
        let amzx = (1.0 / (z * z + x * x).sqrt()) as f32;
        let amxy = (1.0 / (x * x + y * y).sqrt()) as f32;
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

    /// `Initialise()`: `WeightedKMeans`, `Initclass`, `Classification`. `Ok(false)` is the
    /// k-means exception (two initial means equal).
    fn initialise(&mut self, io: &mut Io) -> Result<bool, FslError> {
        if !self.weighted_kmeans(io)? {
            return Ok(false);
        }
        self.initclass();
        self.classification();
        Ok(true)
    }

    /// `WeightedKMeans()`.
    fn weighted_kmeans(&mut self, io: &mut Io) -> Result<bool, FslError> {
        let nc = self.nc;
        let mut input_means: Vec<f32> = Vec::new();
        if !self.p.mansegfile.is_empty() {
            input_means = read_floats(&self.p.mansegfile);
        }
        self.post.fill(0.0);
        self.prob.fill(0.0);
        let perc = (1.0 / f64::from((nc as f64 + 1.0) as f32)) as f32;
        let mut mricopy = self.props.clone();
        mricopy.dims = [self.nx, self.ny, self.nz, 1, 1, 1, 1];
        mricopy.data = self.mricopy.clone();
        let mut maskv = mricopy.clone();
        maskv.data = self.mask.clone();
        for c in 1..=nc {
            if input_means.len() == nc {
                self.mean[c] = logf(input_means[c - 1]);
            } else {
                let (v, _) = mricopy.percentile_masked(perc * c as f32, Some(&maskv))?;
                self.mean[c] = v;
            }
            if self.p.verbose {
                let _ = writeln!(io.out, "{c} {}", cnum::stream(f64::from(self.mean[c])));
            }
        }
        drop(mricopy);
        drop(maskv);
        for c in 1..=nc {
            for c2 in 1..c {
                if self.mean[c] == self.mean[c2] {
                    return Ok(false);
                }
            }
        }
        for k in 0..self.nvox {
            if self.mask[k] == 1.0 {
                let mut minclass = 0;
                let mut mindist = 1e31f32;
                let v = self.mricopy[k];
                for c in 1..=nc {
                    let d = (v - self.mean[c]) * (v - self.mean[c]);
                    if d < mindist {
                        mindist = d;
                        minclass = c;
                    }
                }
                let i = self.at(minclass, k);
                self.post[i] = 1.0;
            }
        }
        for it in 0..(self.p.initfixed + self.p.inititerations) {
            self.update_members_post();
            self.means_variances(io, false);
            if self.p.verbose {
                let _ = writeln!(io.out, "KMeans Iteration {it}");
            }
            self.initclass();
        }
        Ok(true)
    }

    /// `UpdateMembers(m_post)`.
    fn update_members_post(&mut self) {
        self.members_written = true;
        let nc = self.nc;
        // FSL loops x outermost; each voxel is independent
        for k in 0..self.nvox {
            if self.mask[k] == 1.0 {
                let mut sum = 0f32;
                for c in 0..nc {
                    let v = fmax0(self.weight[c] * self.post[(c + 1) * self.nvox + k]);
                    self.members[c * self.nvox + k] = v;
                    sum += v;
                }
                if sum > 0.0 {
                    for c in 0..nc {
                        self.members[c * self.nvox + k] /= sum;
                    }
                }
            }
        }
    }

    /// `MeansVariances(nClasses, m_post)`: each class's mean and variance from the sums over
    /// the mask (FSL's blocks) of `post * mri` and `post * mri * mri` in `float`. With
    /// `use_prob` the probabilities are `m_prob` (not used by FAST's single channel).
    fn means_variances(&mut self, io: &mut Io, use_prob: bool) {
        let nvox = self.nvox;
        let src = if use_prob { &self.prob } else { &self.post };
        let mri = &self.mri;
        let mask = Some(self.mask.as_slice());
        for c in 1..=self.nc {
            let p = &src[c * nvox..(c + 1) * nvox];
            let (s_mp, e1) = stats::masked_sum_by(nvox, |k| p[k] * mri[k], mask);
            let (s_p, e2) = stats::masked_sum(p, mask);
            self.mean[c] = (s_mp / s_p) as f32;
            let (s_mpm, e3) = stats::masked_sum_by(nvox, |k| (p[k] * mri[k]) * mri[k], mask);
            let (s_p2, e4) = stats::masked_sum(p, mask);
            self.variance[c] = (s_mpm / s_p2 - f64::from(self.mean[c] * self.mean[c])) as f32;
            for e in [e1, e2, e3, e4] {
                if e {
                    let _ = writeln!(io.err, "{EMPTY_MASK}");
                }
            }
        }
    }

    /// `Initclass()`.
    fn initclass(&mut self) {
        let nc = self.nc;
        self.prob.fill(0.0);
        let scales: Vec<f32> = (0..=nc)
            .map(|c| Self::log_scale(self.variance[c]))
            .collect();
        for k in 0..self.nvox {
            if self.mask[k] == 1.0 {
                let mut tot = 0f32;
                let y = self.mri[k];
                for c in 1..=nc {
                    let lg = Self::log_gaussian_with(y, self.mean[c], self.variance[c], scales[c]);
                    let v = fmax0(expf(-lg));
                    self.prob[c * self.nvox + k] = v;
                    tot += v;
                }
                if tot > 0.0 {
                    for c in 1..=nc {
                        self.prob[c * self.nvox + k] /= tot;
                    }
                }
            }
        }
        self.post.copy_from_slice(&self.prob);
    }

    /// `Classification()`: each voxel's class of largest posterior (0 outside the mask).
    fn classification(&mut self) {
        for k in 0..self.nvox {
            let mut max = -1e-10f32;
            self.segment[k] = 0;
            if self.mask[k] == 1.0 {
                for c in 1..=self.nc {
                    let v = self.post[c * self.nvox + k];
                    if v > max {
                        max = v;
                        self.segment[k] = c as i32;
                    }
                }
            }
        }
    }

    /// `InitKernel(m_nbLowpass)`: the bias field's Gaussian kernels.
    fn init_kernel(&mut self) {
        let lp = f64::from(self.p.nblowpass);
        for (axis, dim) in [self.xdim, self.ydim, self.zdim].into_iter().enumerate() {
            let sigma = (0.51 * lp / dim) as f32;
            let radius = 2i32.wrapping_mul(cnum::f32_to_i32(sigma));
            self.kernels[axis] = newimagefns::gaussian_kernel_1d(sigma, radius);
        }
    }

    /// A volume of this image's grid and properties holding `data`.
    fn volume_of(&self, data: Vec<f32>) -> Volume {
        let mut v = self.props.clone();
        v.dims = [self.nx, self.ny, self.nz, 1, 1, 1, 1];
        v.data = data;
        v
    }

    /// `BiasRemoval()`.
    fn bias_removal(&mut self, io: &mut Io) -> Result<(), FslError> {
        self.bias_field.fill(0.0);
        if !self.p.bias {
            return Ok(());
        }
        let nvox = self.nvox;
        let mut meaninvcov = vec![0f32; nvox];
        let mut resmean = vec![0f32; nvox];
        for k in 0..nvox {
            if self.mask[k] == 1.0 {
                for c in 1..=self.nc {
                    let tempf = self.post[c * nvox + k] / self.variance[c];
                    meaninvcov[k] += tempf;
                    resmean[k] += tempf * (self.mricopy[k] - self.mean[c]);
                }
            }
        }
        let [kx, ky, kz] = &self.kernels;
        let threads = self.p.threads;
        let meaninvcov =
            newimagefns::convolve_separable(&self.volume_of(meaninvcov), kx, ky, kz, threads);
        let resmean =
            newimagefns::convolve_separable(&self.volume_of(resmean), kx, ky, kz, threads);
        let maskv = self.volume_of(self.mask.clone());
        let mut field = newimagefns::divide(&resmean, &meaninvcov, &maskv)?;
        let (m, empty) = field.mean_masked(&maskv)?;
        if empty {
            let _ = writeln!(io.err, "{EMPTY_MASK}");
        }
        let m = m as f32;
        for v in &mut field.data {
            *v -= m;
        }
        for (v, &w) in field.data.iter_mut().zip(&self.mask) {
            *v *= w;
        }
        self.bias_field = field.data;
        for k in 0..nvox {
            self.mri[k] = self.mricopy[k] - self.bias_field[k];
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
                    if self.mask[k] == 1.0 {
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
    /// NaN `-H`: the table is then all zeros).
    fn tanaka_hyper(&mut self, io: &mut Io) {
        let total = self.mrf_weights_total();
        let mut betahtemp = 0f32;
        self.beta = betahtemp;
        let mut min = (total - self.rhs[0]).abs();
        let mut line = format!(" TOTAL {} RHS ", cnum::stream(f64::from(total)));
        for b in 0..15 {
            line.push_str(&cnum::stream(f64::from(self.rhs[b])));
            line.push(' ');
        }
        let _ = writeln!(io.out, "{line}");
        for b in 1..15 {
            betahtemp += 0.05f32;
            let s = cnum::stream(f64::from(betahtemp));
            let _ = writeln!(io.err, "{s} {s}");
            if (total - self.rhs[b]).abs() < min {
                self.beta = betahtemp;
                min = (total - self.rhs[b]).abs();
            }
        }
    }

    /// `TanakaIterations()`: random initial posteriors, then five sweeps of the MRF.
    fn tanaka_iterations(&mut self, io: &mut Io) {
        let nc = self.nc;
        let nvox = self.nvox;
        self.post.fill(0.0);
        for k in 0..nvox {
            if self.mask[k] == 1.0 {
                let mut sum = 0f32;
                for c in 1..=nc {
                    let v = self.rand.rand() as f32 / RAND_MAX as f32;
                    self.post[c * nvox + k] = v;
                    sum += v;
                }
                if sum > 0.0 {
                    for c in 1..=nc {
                        self.post[c * nvox + k] /= sum;
                    }
                }
            }
        }
        let scales: Vec<f32> = (0..=nc)
            .map(|c| Self::log_scale(self.variance[c]))
            .collect();
        let beta = f64::from(self.beta);
        let mut vals = vec![0f32; nc + 1];
        for iteration in 0..5 {
            for z in 0..self.nz {
                for y in 0..self.ny {
                    for x in 0..self.nx {
                        let k = ((z * self.ny + y) * self.nx + x) as usize;
                        if self.mask[k] != 1.0 {
                            continue;
                        }
                        let nb = self.neighbourhood(x, y, z);
                        let mut sum = 0f64;
                        let yv = self.mri[k];
                        for c in 1..=nc {
                            let lg = Self::log_gaussian_with(
                                yv,
                                self.mean[c],
                                self.variance[c],
                                scales[c],
                            );
                            let mut v = exp(beta * self.mrf_inner(&nb, c) - f64::from(lg)) as f32;
                            if self.p.bapused >= 2 {
                                v *= self.talpriors[c * nvox + k];
                            }
                            vals[c] = v;
                            self.post[c * nvox + k] = v;
                            sum += f64::from(v);
                        }
                        for c in 1..=nc {
                            self.post[c * nvox + k] = if sum > 0.0 {
                                (f64::from(vals[c]) / sum) as f32
                            } else {
                                0.0
                            };
                        }
                    }
                }
            }
            if self.p.verbose {
                let _ = writeln!(
                    io.out,
                    "Tanaka-inner-loop-iteration={iteration} MRFWeightsTotal={} beta={}",
                    cnum::stream(f64::from(self.mrf_weights_total())),
                    cnum::stream(f64::from(self.beta))
                );
            }
        }
        if self.p.verbose {
            let mut line = String::new();
            for c in 1..=nc {
                let m = self.mean[c];
                let s = self.variance[c].sqrt();
                let sd = (expf(m + s) - expf(m - s)) / 2.0;
                line.push_str(&format!(
                    " CLASS {c} MEAN {} STDDEV {}",
                    cnum::stream(f64::from(expf(m))),
                    cnum::stream(f64::from(sd))
                ));
            }
            let _ = writeln!(io.out, "{line}");
        }
    }

    /// The number of mixel types (`PVClassificationStep`'s and `ICMPV`'s): 6 for three classes,
    /// `2 nClasses - 1` for two or more than three, none for one.
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

    /// `PVClassificationStep()`: the likelihood of each mixel type, then `ICMPV()`.
    fn pv_classification_step(&mut self) {
        let nc = self.nc;
        let nvox = self.nvox;
        let mixnum = self.mixnum();
        self.pv_prob = vec![0.0; nvox * mixnum];
        let scales: Vec<f32> = (0..=nc)
            .map(|c| Self::log_scale(self.variance[c]))
            .collect();
        for k in 0..nvox {
            if self.mask[k] != 1.0 {
                continue;
            }
            let y = self.mri[k];
            for t in 0..mixnum {
                let i = t * nvox + k;
                if t < nc {
                    let lg = Self::log_gaussian_with(
                        y,
                        self.mean[t + 1],
                        self.variance[t + 1],
                        scales[t + 1],
                    );
                    // -1.0 * x: GCC folds it into a negation
                    self.pv_prob[i] = exp(-f64::from(lg)) as f32;
                } else {
                    let (a, b) = if nc == 3 {
                        match t {
                            3 => (1, 2),
                            4 => (1, 3),
                            _ => (2, 3),
                        }
                    } else {
                        (t - nc + 1, t - nc + 2)
                    };
                    let mut delta = 0f32;
                    while f64::from(delta) <= 1.0 {
                        let mu = delta * self.mean[a] + (1.0 - delta) * self.mean[b];
                        let sigsq = delta * delta * self.variance[a]
                            + (1.0 - delta) * (1.0 - delta) * self.variance[b];
                        let g = exp(-f64::from(Self::log_gaussian(y, mu, sigsq)));
                        self.pv_prob[i] = (f64::from(self.pv_prob[i]) + g * 0.01) as f32;
                        delta = (f64::from(delta) + 0.01) as f32;
                    }
                }
            }
        }
        self.icmpv();
    }

    /// `ICMPV()`: the mixel types, by largest likelihood, then one sweep of iterated
    /// conditional modes with the mixel-type MRF (in place).
    fn icmpv(&mut self) {
        let nc = self.nc;
        let nvox = self.nvox;
        self.hard_pv = vec![0; nvox];
        let mixnum = self.mixnum();
        for k in 0..nvox {
            if self.mask[k] == 1.0 {
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
                    if self.mask[k] != 1.0 {
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

    /// `PVestimation()`: each voxel's partial volumes, from its mixel type: 1 for a pure
    /// class, else the fraction (in steps of `1 / iterationspve`) of least energy.
    fn pv_estimation(&mut self) -> Result<(), FslError> {
        let nc = self.nc;
        let nvox = self.nvox;
        self.pve = vec![0.0; nvox * (nc + 1)];
        let step = 1.0f32 as f64 / f64::from(self.p.iterationspve);
        for k in 0..nvox {
            if self.mask[k] <= 0.0 {
                continue;
            }
            let h = self.hard_pv[k];
            if (h as i64) < nc as i64 {
                self.pve[(h as usize + 1) * nvox + k] = 1.0;
            } else {
                let (a, b) = mixel_type(nc as i32, h)?;
                let y = self.mri[k];
                let mut min = 1.0e13f32;
                let mut delta = 0f32;
                while f64::from(delta) <= 1.0 {
                    let mu = delta * self.mean[a] + (1.0 - delta) * self.mean[b];
                    let sigsq = delta * delta * self.variance[a]
                        + (1.0 - delta) * (1.0 - delta) * self.variance[b];
                    let val = pv_energy(y, mu, sigsq);
                    if min > val {
                        self.pve[a * nvox + k] = delta;
                        self.pve[b * nvox + k] = (1.0 - f64::from(delta)) as f32;
                        min = val;
                    }
                    delta = (f64::from(delta) + step) as f32;
                }
            }
        }
        Ok(())
    }

    /// `calculateVolumes(probs)`: each class's volume (mm^3) for the verbose totals, from
    /// `m_pve` (`pve`) or `m_post`.
    fn calculate_volumes(&mut self, io: &mut Io, pve: bool) {
        let nvox = self.nvox;
        let mask = Some(self.mask.as_slice());
        let size = self.xdim * self.ydim * self.zdim;
        for c in 1..=self.nc {
            let src = if pve { &self.pve } else { &self.post };
            let (s, empty) = stats::masked_sum(&src[c * nvox..(c + 1) * nvox], mask);
            if empty {
                let _ = writeln!(io.err, "{EMPTY_MASK}");
            }
            self.volumequant[c] = s * size;
        }
    }

    /// `printVolumeTotals()`.
    fn print_volume_totals(&self, io: &mut Io) {
        let mut tot = 0f64;
        for c in 1..=self.nc {
            tot += self.volumequant[c];
            let _ = write!(
                io.out,
                "\n tissue {c} {}",
                cnum::stream(self.volumequant[c])
            );
        }
        let _ = write!(io.out, "\n total tissue {}\n", cnum::stream(tot));
    }

    /// `pveClassification()`: each voxel's class of largest partial volume.
    fn pve_classification(&mut self) {
        for k in 0..self.nvox {
            let mut max = -1e-10f32;
            self.pve_segment[k] = 0;
            if self.mask[k] == 1.0 {
                for c in 1..=self.nc {
                    let v = self.pve[c * self.nvox + k];
                    if v > max {
                        max = v;
                        self.pve_segment[k] = c as i32;
                    }
                }
            }
        }
    }

    /// `qsort()`: the classes ordered by mean (CSF, grey, white for a T1-weighted image; the
    /// order reversed for `-t 2`), their variances and probabilities moved with them.
    fn qsort(&mut self) {
        let nc = self.nc;
        let nvox = self.nvox;
        let meancopy = self.mean.clone();
        let varcopy = self.variance.clone();
        let postcopy = self.post.clone();
        let probcopy = self.prob.clone();
        let mut sorted = self.mean[1..].to_vec();
        if larmorx_nc_core::stdsort::sort_f32_checked(&mut sorted).is_err() {
            sorted.sort_by(|a, b| match (a.is_nan(), b.is_nan()) {
                (false, false) => a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal),
                (x, y) => x.cmp(&y),
            });
        }
        self.mean[1..].copy_from_slice(&sorted);
        if self.p.imagetype == 2 {
            self.mean[1..].reverse();
        }
        for n in 1..=nc {
            for m in 1..=nc {
                if self.mean[n] == meancopy[m] {
                    self.variance[n] = varcopy[m];
                    self.post[n * nvox..(n + 1) * nvox]
                        .copy_from_slice(&postcopy[m * nvox..(m + 1) * nvox]);
                    self.prob[n * nvox..(n + 1) * nvox]
                        .copy_from_slice(&probcopy[m * nvox..(m + 1) * nvox]);
                }
            }
        }
    }

    /// `InitSimple(pcsf, pgm, pwm)`: the initial posteriors from the tissue priors (2 or 3
    /// classes).
    fn init_simple(&mut self, pr: [&Volume; 3], io: &mut Io) -> Result<(), FslError> {
        if self.p.verbose {
            let _ = writeln!(io.out, "Beginning prior-based initialisation");
        }
        let nvox = self.nvox;
        self.post.fill(0.0);
        self.prob.fill(0.0);
        self.talpriors.fill(0.0);
        let nc = self.nc;
        let [csf, gm, wm] = pr;
        for v in [csf, gm, wm] {
            if v.data.len() < nvox {
                return Err(FslError::imthrow("Out of Bounds", 5));
            }
        }
        if nc == 3 || nc == 2 {
            for k in 0..nvox {
                if self.mask[k] != 1.0 {
                    continue;
                }
                let norm2 = csf.data[k] + gm.data[k] + wm.data[k];
                let vals: Vec<f32> = if norm2 > 0.0 {
                    if nc == 3 {
                        vec![csf.data[k] / norm2, gm.data[k] / norm2, wm.data[k] / norm2]
                    } else {
                        vec![csf.data[k] / norm2, (gm.data[k] + wm.data[k]) / norm2]
                    }
                } else {
                    let u = if nc == 3 {
                        (1.0 / f64::from(3.0f32)) as f32
                    } else {
                        (1.0 / f64::from(2.0f32)) as f32
                    };
                    vec![u; nc]
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
        self.means_variances(io, false);
        Ok(())
    }

    /// `m_pve[c]`, `members[c]` as image volumes.
    pub fn pve_volume(&self, c: usize) -> Vec<f32> {
        self.pve[c * self.nvox..(c + 1) * self.nvox].to_vec()
    }

    /// The voxels outside the mask (FSL-96: `members` is never written there).
    pub fn outside_mask(&self) -> usize {
        self.mask.iter().filter(|&&m| m != 1.0).count()
    }

    pub fn member_volume(&self, c: usize) -> Vec<f32> {
        self.members[c * self.nvox..(c + 1) * self.nvox].to_vec()
    }
}

/// `std::max(v, 0.0f)`: `v`, unless `v < 0` (a NaN stays).
#[inline]
fn fmax0(v: f32) -> f32 {
    if v < 0.0 { 0.0 } else { v }
}

/// `PVEnergy(x, y, z, mu, sigmasq)`.
#[inline]
fn pv_energy(y: f32, mu: f32, sigmasq: f32) -> f32 {
    let temp = y - mu;
    let cond = (temp * temp) / sigmasq + logf(sigmasq);
    (f64::from(cond) / 2.0) as f32
}

/// `mixelType(nClasses, type)`: the two classes of a mixel type (indices of the class
/// volumes). `Err` is its `std::invalid_argument`.
fn mixel_type(nc: i32, t: i32) -> Result<(usize, usize), FslError> {
    let limit = if nc == 3 { 5 } else { 2 * nc - 1 };
    let fail = |what: &str| FslError {
        printed: Vec::new(),
        type_name: "std::invalid_argument",
        what: what.to_owned(),
    };
    if t > limit {
        return Err(fail("Mixeltype exceeds allowed types"));
    }
    if t < 0 {
        return Err(fail("Mixeltype cannot be negative"));
    }
    let (mut a, mut b) = ((t - nc + 1) as u32, (t - nc + 2) as u32);
    if nc == 3 && t == 4 {
        a = 1;
        b = 3;
    }
    if nc == 3 && t == 5 {
        a = 2;
        b = 3;
    }
    Ok((a as usize, b as usize))
}

/// The floats of a text file (`copy(istream_iterator<float>(file), ...)`): read until the first
/// word that is not a number.
pub(crate) fn read_floats(path: &str) -> Vec<f32> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for w in text.split_whitespace() {
        let (v, end) = cnum::strtof(w);
        if end == 0 {
            break;
        }
        out.push(v);
        if end != w.len() {
            break;
        }
    }
    out
}

/// Throws `e` (unwinds to the program's entry, which prints C++'s text for an uncaught
/// exception).
pub fn throw(e: FslError) -> ! {
    exception::throw(e)
}

impl Segmentation {
    /// Whether FAST's mask holds no voxel (FSL-101: with the priors unregistered FAST then
    /// runs on, as no voxel reads them).
    pub fn mask_is_empty(&self) -> bool {
        self.mask.iter().all(|&m| m != 1.0)
    }
}
