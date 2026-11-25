// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_coreg`'s registration (`mri_coreg.cpp`'s `COREG` and its functions, 1162-2149):
//! `COREGvolIndex`, `COREGsamp` (trilinear interpolation of a byte vector), `COREGhist` (the
//! joint histogram, after `spm_hist2.c`), `MRIrescaleToUChar`, `MRItoUCharVect`,
//! `MRIgetVoxelToVoxelXformBase`, `conv1d`, `conv1dmat`, `NMICost`,
//! `COREGoptSchema2MatrixPar`, `COREGmatrixPar2OptSchema`, `COREGcost`, `COREGcostPowell`,
//! `COREGMinPowell`, `COREGpreproc`, `COREGfwhm`, `MRIgetPercentile`, `COREGprint`,
//! `MRIconformNoScale` and `COREGoptBruteForce`; and `randomfields.cpp`'s `RFsynth` and
//! `RFdrawVal` for the uniform field FreeSurfer draws (`RFspecInit`'s RANLUX389: larmorx-core's
//! [`Ranlux`], the clean room of `specs/ranlux.md`).
//!
//! The joint histogram: the reference sampled every `sep` voxels (with the coordinate dither,
//! `sep` times a uniform draw in [0, 1) per axis, clamped to the volume), each point mapped by
//! the voxel-to-voxel matrix into the moving volume (outside it: skipped, or a moving value 0
//! with `--mov-oob` and during the brute-force search); the moving value's bin and the next
//! share the weight by its fraction, the reference's bin is its value rounded. FreeSurfer splits
//! the reference's columns into 32 chunks, each with its own histogram, summed in chunk order:
//! the same here, the chunks on the run's threads. The cost: both histogram axes smoothed by a
//! Gaussian of 7 bins' full width (29 taps), `FLT_EPSILON` added to every bin, normalised, the
//! normalised mutual information `-(H(r) + H(c)) / (H(r, c) + FLT_EPSILON)` (`log2`). Every sum
//! in FreeSurfer's order; the convolutions skip zero bins (their terms are zeros) and redo a sum
//! that ends as a NaN with x86-64's NaN rules. `exp`, `log`, `log2`, `pow` are the run's
//! [`Libm`]'s (glibc's bits with `reproduce`, correctly rounded by default: GLIBC-07, quiet).
//!
//! Defects: FS-1319 (the brute-force search's step does not advance the parameter: FreeSurfer
//! never ends), FS-1320 (`MRIgetPercentile` reads past its sorted values for a percentile that
//! rounds to the last place or beyond: memory contents), FS-1321 (a sample whose coordinates
//! are not numbers: FreeSurfer reads outside its vector).

use std::cell::RefCell;
use std::fs::File;
use std::io::Write;

use larmorx_core::cfmt;
use larmorx_core::defects::{FS_1319, FS_1320, FS_1321, FS_1323};
use larmorx_core::glibc_qsort::qsort;
use larmorx_core::math::Libm;
use larmorx_core::ranlux::Ranlux;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::matrix::{Matrix, matrix_multiply_d};
use larmorx_freesurfer_core::matrix_ops::matrix_inverse;
use larmorx_freesurfer_core::numerics::{NrMatrix, open_powell2};
use larmorx_freesurfer_core::utils::{compare_floats, nint};
use larmorx_freesurfer_filter::conv::{Smni, mri_gaussian_smooth_ni};
use larmorx_freesurfer_mri::mri::{MRI_FLOAT, Mri, mri_alloc, mri_copy_header};
use larmorx_freesurfer_mri::vol_geom::xfm_crs2xyz;
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};
use larmorx_freesurfer_volume::arith::Pair;
use larmorx_freesurfer_volume::rotmat::tranform_affine_params2_matrix;

use super::main::Run;

/// The histograms' bins per axis.
const NB: usize = 256;
/// `COREGhist`'s chunks of the reference's columns.
const NCHUNKS: usize = 32;
/// `FLT_EPSILON` as a `double`.
const FLT_EPSILON: f64 = f32::EPSILON as f64;

/// A uniform random field FreeSurfer draws from (`RFS` named `uniform`): RANLUX389 seeded with
/// the spec's seed, and its two parameters (`calloc` leaves them 0).
pub struct UniformField {
    pub rng: Ranlux,
    pub lo: f64,
    pub hi: f64,
}

impl UniformField {
    /// `RFspecInit(seed, NULL)`: FreeSurfer's `unsigned long` of the program's `int` seed
    /// (`-1` is `2^64 - 1`); 0 is the time of day's (FS-29: here fs.core's fixed seed, noted).
    pub fn new(g: &mut Globals, seed: i32, lo: f64, hi: f64) -> UniformField {
        let s = if seed == 0 {
            larmorx_freesurfer_core::pdf::pdf_tod_seed(g)
        } else {
            i64::from(seed) as u64
        };
        UniformField {
            rng: Ranlux::new(s, g.mode()),
            lo,
            hi,
        }
    }

    /// `RFdrawVal(rfs)`: `sc_ran_flat(rng, params[0], params[1])`.
    pub fn draw(&mut self) -> f64 {
        self.rng.flat(self.lo, self.hi)
    }
}

/// `COREG`'s state and Powell's file-scope values (`COREGcostPowell`'s statics).
pub struct Coreg {
    pub ref_vol: Mri,
    pub mov: Mri,
    pub seplist: Vec<i32>,
    pub sep: i32,
    pub sepmin: i32,
    pub sat_pct: f64,
    pub refsat: f64,
    pub movsat: f64,
    /// The reference's and the moving volume's bytes (`g`, `f`), columns fastest.
    pub gvec: Vec<u8>,
    pub fvec: Vec<u8>,
    /// RefRAS-to-MovRAS and RefVox-to-MovVox.
    pub m: Option<Matrix>,
    pub v2v: Option<Matrix>,
    pub reffwhm: [f64; 3],
    pub refgstd: [f64; 3],
    pub movfwhm: [f64; 3],
    pub movgstd: [f64; 3],
    pub histfwhm: [f64; 2],
    pub params: [f64; 12],
    pub nparams: i32,
    pub mparams: [f64; 12],
    /// `H0`, row `i` the moving volume's bin, column `j` the reference's.
    pub h0: Vec<f64>,
    pub cost: f64,
    pub n_cost_evaluations: i32,
    pub ftol: f64,
    pub linmintol: f64,
    pub fret: f32,
    pub nitersmax: i32,
    pub niters: i32,
    pub startmin: bool,
    pub nhits: i32,
    pub nvoxref: i32,
    pub pcthits: f64,
    pub do_coord_dither: i32,
    /// The coordinate dither (`cdither`, a 3-frame float volume of the reference's extent):
    /// frame `f`'s voxel `(c, r, s)` at `((f * depth + s) * height + r) * width + c`.
    pub cdither: Vec<f32>,
    pub do_intensity_dither: i32,
    pub refirfs: Option<UniformField>,
    pub movirfs: Option<UniformField>,
    pub movidither: Option<Mri>,
    pub do_smoothing: i32,
    pub fplogcost: Option<File>,
    pub mov_oob_flag: i32,
    pub optschema: i32,
    pub debug: i32,
    pub seed: i32,
    /// The threads the chunks of the histogram run on (the results never depend on them).
    pub threads: usize,
    /// `COREGcostPowell`'s `initcost`, `mincost` and `ppmin`.
    pub initcost: f32,
    pub mincost: f32,
    pub ppmin: [f32; 100],
    /// The run's C library functions.
    pub libm: Libm,
}

/// C's `round` (half away from zero) through the integer part, without the C library.
pub fn c_round(v: f64) -> f64 {
    if v.is_nan() || v.abs() >= 4_503_599_627_370_496.0 {
        return v;
    }
    let t = v as i64 as f64;
    let d = v - t;
    if d >= 0.5 {
        t + 1.0
    } else if d <= -0.5 {
        t - 1.0
    } else {
        t
    }
}

/// `(int) x` of a `double` as x86-64 converts it (`cvttsd2si`: out of range and NaN give
/// `INT_MIN`).
pub fn to_int_x86(x: f64) -> i32 {
    if x.is_nan() || x >= 2_147_483_648.0 || x < -2_147_483_648.0 {
        i32::MIN
    } else {
        x as i32
    }
}

/// `COREGsamp(f, c, r, s, ncols, nrows, nslices)`: trilinear interpolation of the byte vector
/// `f` (columns fastest), the eight corners' weights `((1 - dc) or dc) * ... ` in the source's
/// order. `None` where a corner falls outside the vector (FreeSurfer reads outside it, FS-1321).
#[inline]
fn coreg_samp(f: &[u8], c: f64, r: f64, s: f64, ncols: i32, nrows: i32) -> Option<f64> {
    let (cm, rm, sm) = (c.floor(), r.floor(), s.floor());
    let (cp, rp, sp) = (c.ceil(), r.ceil(), s.ceil());
    let cmd = c - cm;
    let rmd = r - rm;
    let smd = s - sm;
    let cpd = 1.0 - cmd;
    let rpd = 1.0 - rmd;
    let spd = 1.0 - smd;
    let idx = |c: f64, r: f64, s: f64| -> Option<usize> {
        let (c, r, s) = (to_int_x86(c), to_int_x86(r), to_int_x86(s));
        let i = i64::from(c)
            + i64::from(r) * i64::from(ncols)
            + i64::from(s) * i64::from(nrows) * i64::from(ncols);
        usize::try_from(i).ok().filter(|&i| i < f.len())
    };
    let v = |c: f64, r: f64, s: f64| -> Option<f64> { idx(c, r, s).map(|i| f64::from(f[i])) };
    let val = cpd * rpd * spd * v(cm, rm, sm)?
        + cpd * rpd * smd * v(cm, rm, sp)?
        + cpd * rmd * spd * v(cm, rp, sm)?
        + cpd * rmd * smd * v(cm, rp, sp)?
        + cmd * rpd * spd * v(cp, rm, sm)?
        + cmd * rpd * smd * v(cp, rm, sp)?
        + cmd * rmd * spd * v(cp, rp, sm)?
        + cmd * rmd * smd * v(cp, rp, sp)?;
    Some(val)
}

/// One chunk's histogram (its non-zero bins, `(bin, value)` in the order of the bins' first
/// weight) and hits, or the coordinates of a sample FreeSurfer reads outside its vector at.
type ChunkResult = Result<(Vec<(usize, f64)>, i64), (f64, f64, f64)>;

/// A chunk's histogram: the bins' values and the bins touched (reused from chunk to chunk; the
/// touched bins cleared after each).
struct Scratch {
    h: Vec<f64>,
    seen: Vec<bool>,
    touched: Vec<usize>,
}

impl Scratch {
    fn new() -> Scratch {
        Scratch {
            h: vec![0.0; NB * NB],
            seen: vec![false; NB * NB],
            touched: Vec::new(),
        }
    }

    #[inline]
    fn add(&mut self, k: usize, w: f64) {
        if !self.seen[k] {
            self.seen[k] = true;
            self.touched.push(k);
        }
        self.h[k] += w;
    }

    /// The touched bins and their values, the scratch cleared.
    fn take(&mut self) -> Vec<(usize, f64)> {
        let out = self.touched.iter().map(|&k| (k, self.h[k])).collect();
        for &k in &self.touched {
            self.h[k] = 0.0;
            self.seen[k] = false;
        }
        self.touched.clear();
        out
    }
}

impl Coreg {
    /// The coordinate dither's value at `(c, r, s)`, frame `f` (`MRIgetVoxVal(cdither, ...)`).
    #[inline]
    fn dither(&self, c: i32, r: i32, s: i32, f: i32) -> f32 {
        let (w, h, d) = (
            self.ref_vol.vg.width as usize,
            self.ref_vol.vg.height as usize,
            self.ref_vol.vg.depth as usize,
        );
        self.cdither[((f as usize * d + s as usize) * h + r as usize) * w + c as usize]
    }

    /// One chunk of `COREGhist`'s loop: the reference's columns `[begin, end)` every `sep`.
    fn hist_chunk(&self, sc: &mut Scratch, v2v: &[f64; 16], begin: i32, end: i32) -> ChunkResult {
        let mut nhits = 0i64;
        let (rw, rh, rd) = (
            self.ref_vol.vg.width,
            self.ref_vol.vg.height,
            self.ref_vol.vg.depth,
        );
        let (mw, mh, md) = (self.mov.vg.width, self.mov.vg.height, self.mov.vg.depth);
        let sep = self.sep;
        let fsep = sep as f32;
        let z_only = self.optschema != 2 && self.optschema != 4 && self.optschema != 5;
        let mut cref = begin;
        while cref < end {
            let mut rref = 0;
            while rref < rh {
                let mut sref = 0;
                while sref < rd {
                    let (mut dc, mut dr, mut ds) =
                        (f64::from(cref), f64::from(rref), f64::from(sref));
                    if self.do_coord_dither != 0 {
                        dc += f64::from(fsep * self.dither(cref, rref, sref, 0));
                        dr += f64::from(fsep * self.dither(cref, rref, sref, 1));
                        ds += f64::from(fsep * self.dither(cref, rref, sref, 2));
                        if dc > f64::from(rw - 1) {
                            dc = f64::from(rw - 1);
                        }
                        if dr > f64::from(rh - 1) {
                            dr = f64::from(rh - 1);
                        }
                        if ds > f64::from(rd - 1) {
                            ds = f64::from(rd - 1);
                        }
                    }
                    let dcmov = v2v[0] * dc + v2v[4] * dr + v2v[8] * ds + v2v[12];
                    let drmov = v2v[1] * dc + v2v[5] * dr + v2v[9] * ds + v2v[13];
                    let mut oob = dcmov < 0.0 || dcmov > f64::from(mw - 1);
                    if drmov < 0.0 || drmov > f64::from(mh - 1) {
                        oob = true;
                    }
                    let mut dsmov = 0.0;
                    if z_only {
                        dsmov = v2v[2] * dc + v2v[6] * dr + v2v[10] * ds + v2v[14];
                        if dsmov < 0.0 || dsmov > f64::from(md - 1) {
                            oob = true;
                        }
                    }
                    let vf = if !oob {
                        nhits += 1;
                        coreg_samp(&self.fvec, dcmov, drmov, dsmov, mw, mh)
                            .ok_or((dcmov, drmov, dsmov))?
                    } else if self.mov_oob_flag != 0 {
                        0.0
                    } else {
                        sref += sep;
                        continue;
                    };
                    let vg = coreg_samp(&self.gvec, dc, dr, ds, rw, rh).ok_or((dc, dr, ds))?;
                    let ivf = to_int_x86(vf.floor());
                    let ivg = to_int_x86((vg + 0.5).floor());
                    let k = (i64::from(ivf) + i64::from(ivg) * NB as i64) as usize;
                    let frac = vf - f64::from(ivf);
                    sc.add(k, 1.0 - frac);
                    if ivf < 255 {
                        sc.add(k + 1, frac);
                    }
                    sref += sep;
                }
                rref += sep;
            }
            cref += sep;
        }
        Ok((sc.take(), nhits))
    }

    /// `COREGhist(coreg)`: the joint histogram into `h0`, the hits and their percentage.
    fn hist(&mut self, g: &mut Globals, r: &mut Run) -> Result<i64, Fatal> {
        let v = self.v2v.as_ref().expect("the voxel-to-voxel matrix");
        let mut v2v = [0.0f64; 16];
        for j in 0..4 {
            for i in 0..3 {
                v2v[4 * j + i] = f64::from(v.relt(i as i32 + 1, j as i32 + 1));
            }
        }
        let width = self.ref_vol.vg.width;
        let sep = self.sep;
        if sep == 0 {
            // FreeSurfer's integer division by 0 (SIGFPE): only `reproduce` reaches it
            g.notices.add(
                &FS_1323,
                "mri_coreg --sep 0: FreeSurfer divides by 0 at the separation's first histogram (SIGFPE)",
            );
            return Err(Fatal::Exit(136));
        }
        let niters = (width + sep - 1) / sep;
        let chunk_size = (niters + NCHUNKS as i32 - 1) / NCHUNKS as i32;
        let ranges: Vec<(i32, i32)> = (0..NCHUNKS as i32)
            .map(|ch| {
                let b = ch * chunk_size * sep;
                let e = ((ch + 1) * chunk_size * sep).min(width);
                (b, e)
            })
            .collect();
        if sep < 0 && ranges.iter().any(|&(b, e)| b < e) {
            // a negative step from below the end: FreeSurfer's loop never ends (it reads outside
            // the volume until it crashes); only `reproduce` reaches it
            g.notices.add(
                &FS_1323,
                format!("mri_coreg --sep {sep}: FreeSurfer's sampling loop never ends"),
            );
            let _ = writeln!(
                r.err,
                "{}: --sep {sep}: FreeSurfer's sampling loop never ends here",
                super::main::NAME
            );
            return Err(Fatal::Exit(1));
        }
        let nthreads = self.threads.clamp(1, NCHUNKS);
        let this = &*self;
        let results: Vec<ChunkResult> = if nthreads == 1 {
            let mut sc = Scratch::new();
            ranges
                .iter()
                .map(|&(b, e)| this.hist_chunk(&mut sc, &v2v, b, e))
                .collect()
        } else {
            std::thread::scope(|scope| {
                let per = NCHUNKS.div_ceil(nthreads);
                let hs: Vec<_> = ranges
                    .chunks(per)
                    .map(|part| {
                        scope.spawn(move || {
                            let mut sc = Scratch::new();
                            part.iter()
                                .map(|&(b, e)| this.hist_chunk(&mut sc, &v2v, b, e))
                                .collect::<Vec<_>>()
                        })
                    })
                    .collect();
                hs.into_iter()
                    .flat_map(|h| h.join().expect("a histogram chunk"))
                    .collect()
            })
        };
        // the chunks collected in their order (`H01d[k] = sum_n HH[n][k]`, each sum from 0: a
        // chunk that left a bin at 0 adds nothing to it), then `H0[r][c] = H01d[c * 256 + r]`
        let mut h01d = vec![0.0f64; NB * NB];
        let mut nhits = 0i64;
        for res in results {
            match res {
                Ok((bins, n)) => {
                    nhits += n;
                    for (k, v) in bins {
                        h01d[k] += v;
                    }
                }
                Err((c, rr, s)) => {
                    return Err(super::standalone::refuse(
                        g,
                        r,
                        &FS_1321,
                        &format!(
                            "a sample at ({}, {}, {}): no voxel there (FreeSurfer reads outside its volume)",
                            cfmt::g(c, 6),
                            cfmt::g(rr, 6),
                            cfmt::g(s, 6)
                        ),
                    ));
                }
            }
        }
        for c in 0..NB {
            for rr in 0..NB {
                self.h0[rr * NB + c] = h01d[c * NB + rr];
            }
        }
        self.nhits = nhits as i32;
        self.pcthits = self.libm.pow(f64::from(self.sep), 3.0) * 100.0 * nhits as f64
            / f64::from(self.nvoxref);
        Ok(nhits)
    }

    /// `COREGoptSchema2MatrixPar(coreg, par)`: the optimised parameters into their places of the
    /// twelve.
    pub fn opt_schema2matrix_par(&self, par: &mut [f64; 12]) {
        let p = &self.params;
        match self.optschema {
            1 => {
                let n = self.nparams.clamp(0, 12) as usize;
                par[..n].copy_from_slice(&p[..n]);
            }
            2 => {
                par[0] = p[0];
                par[2] = p[1];
                par[4] = p[2];
            }
            3 => {
                par[..6].copy_from_slice(&p[..6]);
                par[8] = p[6];
            }
            4 => {
                par[0] = p[0];
                par[1] = p[1];
                par[5] = p[2];
            }
            5 => {
                par[0] = p[0];
                par[1] = p[1];
                par[5] = p[2];
                par[6] = p[3];
                par[7] = p[4];
                par[9] = p[5];
            }
            _ => {}
        }
    }

    /// `COREGmatrixPar2OptSchema(coreg, par)`.
    pub fn matrix_par2opt_schema(&mut self, par: &[f64; 12]) {
        let p = &mut self.params;
        match self.optschema {
            1 => {
                let n = self.nparams.clamp(0, 12) as usize;
                p[..n].copy_from_slice(&par[..n]);
            }
            2 => {
                p[0] = par[0];
                p[1] = par[2];
                p[2] = par[4];
            }
            3 => {
                p[..6].copy_from_slice(&par[..6]);
                p[6] = par[8];
            }
            4 => {
                p[0] = par[0];
                p[1] = par[1];
                p[2] = par[5];
            }
            5 => {
                p[0] = par[0];
                p[1] = par[1];
                p[2] = par[5];
                p[3] = par[6];
                p[4] = par[7];
                p[5] = par[9];
            }
            _ => {}
        }
    }

    /// The Gaussian of `fwhm` bins' full width, normalised (`COREGcost`'s filters).
    fn hist_filter(&self, fwhm: f64) -> Vec<f64> {
        let std = fwhm / self.libm.log(256.0).sqrt();
        let lim = to_int_x86((2.0 * fwhm).ceil());
        let ng = (2 * lim + 1).max(0) as usize;
        let mut gk = vec![0.0f64; ng];
        let mut sum = 0.0f64;
        for n in -lim..=lim {
            let v = self.libm.exp(f64::from(-(n * n)) / (2.0 * (std * std)))
                / (std * (2.0 * std::f64::consts::PI).sqrt());
            gk[(n + lim) as usize] = v;
            sum += v;
        }
        for v in &mut gk {
            *v /= sum;
        }
        gk
    }

    /// `COREGcost(coreg)`: the cost of the current parameters (into `cost`), its line in the
    /// cost log, the evaluations counted.
    pub fn cost(&mut self, g: &mut Globals, r: &mut Run) -> Result<f64, Fatal> {
        let mut mp = self.mparams;
        self.opt_schema2matrix_par(&mut mp);
        self.mparams = mp;
        let mut m = self.m.take();
        match m.as_mut() {
            Some(mm) => {
                tranform_affine_params2_matrix(g, r.out, r.err, &self.mparams, Some(mm))?;
            }
            None => {
                m = tranform_affine_params2_matrix(g, r.out, r.err, &self.mparams, None)?
                    .into_new();
            }
        }
        self.m = m;
        self.v2v = Some(self.vox2vox_base(g, r)?);
        self.hist(g, r)?;
        let g1 = self.hist_filter(self.histfwhm[0]);
        let g2 = self.hist_filter(self.histfwhm[1]);
        // the rows (the moving volume's bins) convolved with g2, then the columns with g1
        let cols1 = NB + g2.len() - 1;
        let mut h1 = vec![0.0f64; NB * cols1];
        for rr in 0..NB {
            conv1d(
                &self.h0[rr * NB..(rr + 1) * NB],
                &g2,
                &mut h1[rr * cols1..(rr + 1) * cols1],
            );
        }
        // the smoothed histogram column by column (`hc[c * rows + r]`: the passes below run
        // over the columns, the rows inside)
        let rows = NB + g1.len() - 1;
        let mut hc = vec![0.0f64; rows * cols1];
        let mut col = vec![0.0f64; NB];
        for c in 0..cols1 {
            for (rr, v) in col.iter_mut().enumerate() {
                *v = h1[rr * cols1 + c];
            }
            conv1d(&col, &g1, &mut hc[c * rows..(c + 1) * rows]);
        }
        for v in &mut hc {
            *v += FLT_EPSILON;
        }
        let mut sum = 0.0f64;
        for v in &hc {
            sum += *v;
        }
        for v in &mut hc {
            *v /= sum;
        }
        self.cost = nmi_cost(&self.libm, &hc, cols1, rows);
        if let Some(fp) = self.fplogcost.as_mut() {
            let mut line = format!("{} {}  ", pad(self.sep, 2), pad(self.n_cost_evaluations, 4));
            for v in &self.params[..self.nparams.clamp(0, 12) as usize] {
                line.push_str(&format!("{} ", cfmt::pad(&cfmt::f(*v, 5), 7)));
            }
            line.push_str(&format!("  {}\n", cfmt::pad(&cfmt::f(self.cost, 7), 9)));
            let _ = fp.write_all(line.as_bytes());
        }
        self.n_cost_evaluations = self.n_cost_evaluations.wrapping_add(1);
        Ok(self.cost)
    }

    /// `MRIgetVoxelToVoxelXformBase(ref, mov, M, V2V, 0)`: `inverse(vox2ras(mov)) * M *
    /// vox2ras(ref)` (`MatrixMultiplyD`).
    pub fn vox2vox_base(&mut self, g: &mut Globals, r: &mut Run) -> Result<Matrix, Fatal> {
        let src = xfm_crs2xyz(g, r.out, r.err, &self.ref_vol.vg, 0, false)?;
        let dst = xfm_crs2xyz(g, r.out, r.err, &self.mov.vg, 0, false)?;
        let ras2vox_dst = matrix_inverse(g, r.out, r.err, Some(&dst), None)?.into_new();
        let mut v = match self.v2v.take() {
            Some(v) => v,
            None => Matrix::real(4, 4),
        };
        matrix_multiply_d(
            g,
            r.out,
            r.err,
            ras2vox_dst.as_ref(),
            self.m.as_ref(),
            Some(&mut v),
        )?;
        let v1 = v.clone();
        matrix_multiply_d(g, r.out, r.err, Some(&v1), Some(&src), Some(&mut v))?;
        Ok(v)
    }

    /// `COREGcostPowell(pPowel)`: the parameters from Powell's vector (`p[1..=n]`), the cost, the
    /// best point so far (`#@# <sep> <evaluations> <parameters> <cost>` when it improves).
    fn cost_powell(&mut self, g: &mut Globals, r: &mut Run, p: &[f32]) -> Result<f32, Fatal> {
        let n = self.nparams.clamp(0, 12) as usize;
        for k in 0..n {
            self.params[k] = f64::from(p[k + 1]);
        }
        let curcost = self.cost(g, r)? as f32;
        let mut newmin = false;
        if self.startmin {
            newmin = true;
            self.initcost = curcost;
            self.mincost = curcost;
            for k in 0..n {
                self.ppmin[k] = self.params[k] as f32;
            }
            let _ = writeln!(
                r.out,
                "InitialCost {} ",
                cfmt::pad(&cfmt::f(f64::from(self.initcost), 10), 20)
            );
            self.startmin = false;
        }
        if self.mincost > curcost {
            newmin = true;
            self.mincost = curcost;
            for k in 0..n {
                self.ppmin[k] = self.params[k] as f32;
            }
        }
        if newmin {
            let mut line = format!(
                "#@# {} {}  ",
                pad(self.sep, 2),
                pad(self.n_cost_evaluations, 4)
            );
            for v in &self.ppmin[..n] {
                line.push_str(&format!("{} ", cfmt::pad(&cfmt::f(f64::from(*v), 5), 7)));
            }
            line.push_str(&format!(
                "  {}\n",
                cfmt::pad(&cfmt::f(f64::from(self.mincost), 7), 9)
            ));
            let _ = r.out.write_all(line.as_bytes());
            let _ = r.out.flush();
        }
        Ok(curcost)
    }

    /// `COREGMinPowell()`: Powell's minimisation from the current parameters (`OpenPowell2`,
    /// the identity's directions), its report, the final cost.
    pub fn min_powell(&mut self, g: &mut Globals, r: &mut Run) -> Result<(), Fatal> {
        let start = std::time::Instant::now();
        let dof = self.nparams;
        let n = dof.clamp(0, 12) as usize;
        let _ = write!(r.out, "\n\n---------------------------------\n");
        let _ = write!(r.out, "Init Powel Params dof = {dof}: ");
        let mut p = vec![0.0f32; n + 1];
        for v in &self.params[..n] {
            let _ = write!(r.out, "{} ", cfmt::f(*v, 6));
        }
        let _ = writeln!(r.out);
        for k in 0..n {
            p[k + 1] = self.params[k] as f32;
        }
        let mut xi = NrMatrix::new(1, i64::from(dof), 1, i64::from(dof));
        for rr in 1..=i64::from(dof) {
            for c in 1..=i64::from(dof) {
                xi[(rr, c)] = if rr == c { 1.0 } else { 0.0 };
            }
        }
        let _ = writeln!(r.out, "Starting OpenPowel2(), sep = {}", self.sep);
        // the cost function's lines and Powell's own go to one stream each, in their order
        let buf_out = RefCell::new((Vec::new(), Vec::new()));
        let buf_err = RefCell::new((Vec::new(), Vec::new()));
        let mut failed: Option<Fatal> = None;
        let (mut niters, mut fret) = (self.niters, self.fret);
        let (ftol, linmintol, nitersmax) =
            (self.ftol as f32, self.linmintol as f32, self.nitersmax);
        {
            let env = r.env.clone();
            let (dir, threads) = (r.dir, r.threads);
            let mut pg = Globals::new(g.mode());
            pg.gdiag_no = g.gdiag_no;
            let mut f = |x: &[f32]| -> f32 {
                if failed.is_some() {
                    return 0.0;
                }
                let (mut so, mut se) = (Shared(&buf_out), Shared(&buf_err));
                let mut sr = Run {
                    out: &mut so,
                    err: &mut se,
                    env: env.clone(),
                    dir,
                    threads,
                };
                match self.cost_powell(g, &mut sr, x) {
                    Ok(v) => v,
                    Err(e) => {
                        // FreeSurfer's exit inside the cost function: the rest of Powell's calls
                        // are not made
                        failed = Some(e);
                        0.0
                    }
                }
            };
            let (mut so, mut se) = (Shared(&buf_out), Shared(&buf_err));
            open_powell2(
                &mut pg,
                &mut so,
                &mut se,
                &mut p,
                Some(&mut xi),
                dof,
                ftol,
                linmintol,
                nitersmax,
                &mut niters,
                &mut fret,
                &mut f,
            );
        }
        replay(r.out, buf_out.into_inner());
        replay(r.err, buf_err.into_inner());
        if let Some(e) = failed {
            return Err(e);
        }
        self.niters = niters;
        self.fret = fret;
        let secs = start.elapsed().as_secs_f64();
        let _ = writeln!(r.out, "Powell done niters total = {}", self.niters);
        let _ = writeln!(r.out, "OptTimeSec {} sec", cfmt::pad(&cfmt::f(secs, 1), 4));
        let _ = writeln!(
            r.out,
            "OptTimeMin {} min",
            cfmt::pad(&cfmt::f(secs / 60.0, 2), 5)
        );
        let _ = writeln!(r.out, "nEvals {}", self.n_cost_evaluations);
        let _ = r.out.flush();
        let _ = write!(r.out, "Final optimized parameters ");
        for k in 0..n {
            self.params[k] = f64::from(p[k + 1]);
            let _ = write!(r.out, "{} ", cfmt::pad(&cfmt::f(self.params[k], 8), 12));
        }
        let _ = writeln!(r.out);
        let _ = write!(r.out, "Final matrix parameters ");
        for v in &self.mparams {
            let _ = write!(r.out, "{} ", cfmt::pad(&cfmt::f(*v, 4), 7));
        }
        let _ = writeln!(r.out);
        self.cost(g, r)?;
        let _ = write!(
            r.out,
            "Final cost {}\n ",
            cfmt::pad(&cfmt::f(self.cost, 15), 20)
        );
        let _ = write!(r.out, "\n\n---------------------------------\n");
        Ok(())
    }

    /// `COREGoptBruteForce(coreg, lim0, niters, n1d)`: for each of the first six parameters (or
    /// all, when fewer) the costs at `n1d + 1` steps over `[p - lim, p + lim]` (the moving
    /// volume's outside counted as 0 meanwhile), the best kept; `lim` divided by `n1d` after each
    /// pass. FS-1319: a step that does not advance the parameter (a limit or count that gives a
    /// step of 0 or less, or one below the parameter's precision): FreeSurfer never ends; a clear
    /// error in both modes.
    pub fn opt_brute_force(
        &mut self,
        g: &mut Globals,
        r: &mut Run,
        lim0: f64,
        niters: i32,
        n1d: i32,
    ) -> Result<(), Fatal> {
        let _ = writeln!(
            r.out,
            "COREGoptBruteForce() {} {niters} {n1d}",
            cfmt::g(lim0, 6)
        );
        let dof = if self.nparams < 6 { self.nparams } else { 6 };
        let bak = self.mov_oob_flag;
        if self.mov_oob_flag == 0 {
            self.mov_oob_flag = 1;
            let _ = writeln!(r.out, "Turning on MovOOB for BruteForce Search");
        }
        let nshow = self.nparams.clamp(0, 12) as usize;
        let mut mincost = 10e10f64;
        let mut lim = lim0;
        let mut pdelta = 0.0f64;
        for iter in 0..niters {
            for nthp in 0..dof.max(0) as usize {
                let pmin = self.params[nthp] - lim;
                let pmax = self.params[nthp] + lim;
                pdelta = (pmax - pmin) / f64::from(n1d);
                let mut popt = self.params[nthp];
                if self.debug != 0 {
                    let _ = writeln!(
                        r.out,
                        "iter={iter} n1d={n1d} pno={nthp} nom={} min={} max={} delta={}",
                        cfmt::g(popt, 6),
                        cfmt::g(pmin, 6),
                        cfmt::g(pmax, 6),
                        cfmt::g(pdelta, 6)
                    );
                }
                let mut nth1d = 0;
                let mut p = pmin;
                while p <= pmax {
                    self.params[nthp] = p;
                    let curcost = self.cost(g, r)?;
                    if mincost > curcost {
                        mincost = curcost;
                        popt = p;
                    }
                    if self.debug != 0 {
                        let mut line =
                            format!("{} {} {}", pad(iter, 2), pad(nthp as i32, 2), pad(nth1d, 3));
                        for v in &self.params[..nshow] {
                            line.push_str(&format!("  {} ", cfmt::pad(&cfmt::f(*v, 5), 9)));
                        }
                        line.push_str(&format!(
                            "  {} {}\n",
                            cfmt::pad(&cfmt::f(curcost, 7), 9),
                            cfmt::pad(&cfmt::f(mincost, 7), 9)
                        ));
                        let _ = r.out.write_all(line.as_bytes());
                        let _ = r.out.flush();
                    }
                    nth1d += 1;
                    let next = p + pdelta;
                    if next <= pmax && (next == p || pdelta < 0.0) {
                        g.notices.add(
                            &FS_1319,
                            format!(
                                "mri_coreg's brute-force search: the step {} does not advance the parameter {} (FreeSurfer never ends)",
                                cfmt::g(pdelta, 6),
                                cfmt::g(p, 6)
                            ),
                        );
                        let _ = writeln!(
                            r.err,
                            "{}: the brute-force search's step {} does not advance the parameter {} (FreeSurfer never ends here)",
                            super::main::NAME,
                            cfmt::g(pdelta, 6),
                            cfmt::g(p, 6)
                        );
                        return Err(Fatal::Exit(1));
                    }
                    p = next;
                }
                self.params[nthp] = popt;
            }
            let mut line = format!(
                "#BF# sep={} iter={iter} lim={} delta={} ",
                pad(self.sep, 2),
                cfmt::pad(&cfmt::f(lim, 1), 4),
                cfmt::pad(&cfmt::f(pdelta, 2), 4)
            );
            for v in &self.params[..nshow] {
                line.push_str(&format!("{} ", cfmt::pad(&cfmt::f(*v, 5), 9)));
            }
            line.push_str(&format!("  {}\n", cfmt::pad(&cfmt::f(mincost, 7), 9)));
            let _ = r.out.write_all(line.as_bytes());
            let _ = r.out.flush();
            lim /= f64::from(n1d);
        }
        if bak == 0 {
            let _ = writeln!(r.out, "Turning  MovOOB back off after brute force search");
        }
        self.mov_oob_flag = bak;
        Ok(())
    }
}

/// A writer into a shared buffer (the cost function's lines among Powell's), its flushes'
/// places kept.
struct Shared<'a>(&'a RefCell<(Vec<u8>, Vec<usize>)>);

impl Write for Shared<'_> {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.0.borrow_mut().0.extend_from_slice(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        let mut s = self.0.borrow_mut();
        let n = s.0.len();
        s.1.push(n);
        Ok(())
    }
}

/// A shared buffer written to `w` with its flushes where they were.
fn replay(w: &mut dyn Write, buf: (Vec<u8>, Vec<usize>)) {
    let (data, flushes) = buf;
    let mut at = 0;
    for p in flushes {
        let _ = w.write_all(&data[at..p]);
        let _ = w.flush();
        at = p;
    }
    let _ = w.write_all(&data[at..]);
}

/// `%<w>d`.
fn pad(v: i32, w: usize) -> String {
    cfmt::pad(&v.to_string(), w)
}

/// `conv1d(v1, n1, v2, n2, vc)`: `vc[n] = sum_k v1[k] v2[n - k]` over the overlap, `k`
/// increasing, each sum from 0; computed by spreading each non-zero `v1[k]`'s terms (a zero's
/// terms are zeros, which leave a sum as it is), a sum that ends as a NaN redone with x86-64's
/// NaN rules.
fn conv1d(v1: &[f64], v2: &[f64], vc: &mut [f64]) {
    use larmorx_freesurfer_core::sse::{add64, mul64};
    let (n1, n2) = (v1.len(), v2.len());
    vc.iter_mut().for_each(|v| *v = 0.0);
    let finite = v2.iter().all(|v| v.is_finite());
    for (k, &a) in v1.iter().enumerate() {
        if a == 0.0 && finite {
            continue;
        }
        for (d, b) in vc[k..k + n2].iter_mut().zip(v2) {
            *d += a * *b;
        }
    }
    for (n, d) in vc.iter_mut().enumerate() {
        if d.is_nan() {
            let kmin = (n + 1).saturating_sub(n2);
            let kmax = if n + 1 < n1 { n } else { n1 - 1 };
            let mut s = 0.0;
            for k in kmin..=kmax {
                s = add64(s, mul64(v1[k], v2[n - k]));
            }
            *d = s;
        }
    }
}

/// `NMICost(H, cols, rows)`: `-(sum_r s1 log2 s1 + sum_c s2 log2 s2) / (sum H log2 H +
/// FLT_EPSILON)`, `s1` the row sums and `s2` the column sums (each from 0 in the index order);
/// `H`'s sum column by column. `hc` holds `H` column by column (`hc[c * rows + r]`).
fn nmi_cost(libm: &Libm, hc: &[f64], cols: usize, rows: usize) -> f64 {
    let mut s1 = vec![0.0f64; rows];
    for c in 0..cols {
        for (s, v) in s1.iter_mut().zip(&hc[c * rows..(c + 1) * rows]) {
            *s += *v;
        }
    }
    let mut s2 = vec![0.0f64; cols];
    for (c, s) in s2.iter_mut().enumerate() {
        for v in &hc[c * rows..(c + 1) * rows] {
            *s += *v;
        }
    }
    // `log2` of the bins in order; the bins of no sample all hold the same value, whose
    // logarithm is taken once (the same call on the same number: the same bits)
    let mut den = 0.0f64;
    let mut last: Option<(u64, f64)> = None;
    for &v in hc {
        let l = match last {
            Some((b, l)) if b == v.to_bits() => l,
            _ => {
                let l = libm.log2(v);
                last = Some((v.to_bits(), l));
                l
            }
        };
        den += v * l;
    }
    let mut s1sum = 0.0f64;
    for v in &s1 {
        s1sum += v * libm.log2(*v);
    }
    let mut s2sum = 0.0f64;
    for v in &s2 {
        s2sum += v * libm.log2(*v);
    }
    -(s1sum + s2sum) / (den + FLT_EPSILON)
}

/// `MRIgetPercentile(mri, Pct, frame)`: the values (slices outermost, then columns, rows)
/// sorted (`qsort`, `compare_floats`), the place `round(nvox * Pct / 100)`; there the value, or
/// at the first or last places the value alone, else the mean of it and its neighbours (`float`
/// sums, divided as `double`). FS-1320: a place at or past the last value (`Pct` within
/// `50 / nvox` of 100 or above it, `nvox` below 5,000 at the default 99.99) reads past the
/// sorted values; the last value by default, 0 (the memory FreeSurfer reads, filled) with
/// `reproduce`.
pub fn mri_get_percentile(
    g: &mut Globals,
    r: &mut Run,
    mri: &Mri,
    pct: f64,
    frame: i32,
) -> Result<f32, Fatal> {
    let (w, h, d) = (mri.vg.width, mri.vg.height, mri.vg.depth);
    let nvox = w * h * d;
    let mut vect: Vec<f32> = Vec::with_capacity(nvox.max(0) as usize);
    for s in 0..d {
        for c in 0..w {
            for rr in 0..h {
                vect.push(mri_get_vox_val(g, r.err, mri, c, rr, s, frame, 0)?);
            }
        }
    }
    qsort(&mut vect, |a, b| compare_floats(*a, *b));
    let n = to_int_x86(c_round(f64::from(nvox) * pct / 100.0));
    let get = |g: &mut Globals, k: i32| -> f32 {
        if k >= 0 && k < nvox {
            vect[k as usize]
        } else {
            g.notices.add(
                &FS_1320,
                format!(
                    "mri_coreg's saturation percentile {}: the place {k} of {nvox} values is past the sorted values (FreeSurfer reads memory outside them)",
                    cfmt::g(pct, 6)
                ),
            );
            if g.notices.reproduce() {
                0.0
            } else {
                vect[(k.clamp(0, (nvox - 1).max(0))) as usize]
            }
        }
    };
    if nvox <= 0 {
        return Ok(0.0);
    }
    let val = if n == 0 || n >= nvox - 1 {
        get(g, n)
    } else {
        let s = vect[n as usize] + vect[n as usize - 1] + vect[n as usize + 1];
        (f64::from(s) / 3.0) as f32
    };
    Ok(val)
}

/// `MRIrescaleToUChar(mri, NULL, sat)`: a float volume with the header copied, each value
/// (clipped at `sat`) rescaled to 0..255 from the volume's range (its maximum clipped at `sat`)
/// and rounded (`nint`, kept as an `unsigned char`).
pub fn mri_rescale_to_uchar(
    g: &mut Globals,
    r: &mut Run,
    mri: &Mri,
    sat: f64,
) -> Result<Mri, Fatal> {
    let mut uc = mri_alloc(
        g,
        r.out,
        r.err,
        mri.vg.width,
        mri.vg.height,
        mri.vg.depth,
        MRI_FLOAT,
    )?;
    mri_copy_header(g, r.out, r.err, mri, Some(&mut uc))?;
    let (mut max, mut min) = (-10e10f64, 10e10f64);
    for c in 0..mri.vg.width {
        for rr in 0..mri.vg.height {
            for s in 0..mri.vg.depth {
                let v = f64::from(mri_get_vox_val(g, r.err, mri, c, rr, s, 0, 0)?);
                if v > max {
                    max = v;
                }
                if v < min {
                    min = v;
                }
            }
        }
    }
    if max > sat {
        max = sat;
    }
    for c in 0..mri.vg.width {
        for rr in 0..mri.vg.height {
            for s in 0..mri.vg.depth {
                let mut v = f64::from(mri_get_vox_val(g, r.err, mri, c, rr, s, 0, 0)?);
                if v > sat {
                    v = sat;
                }
                let ucv = nint(g, 255.0 * (v - min) / (max - min)) as u8;
                mri_set_vox_val(g, r.err, &mut uc, c, rr, s, 0, f32::from(ucv), 0)?;
            }
        }
    }
    Ok(uc)
}

/// `MRItoUCharVect(mri, rfs, dither)`: the bytes (columns fastest), each the value plus the
/// field's draw or the dither volume's value (`float`), clipped to 0..255 and rounded.
pub fn mri_to_uchar_vect(
    g: &mut Globals,
    r: &mut Run,
    mri: &Mri,
    rfs: Option<&mut UniformField>,
    dither: Option<&Mri>,
) -> Result<Vec<u8>, Fatal> {
    let (w, h, d) = (mri.vg.width, mri.vg.height, mri.vg.depth);
    let mut a = Vec::with_capacity((w * h * d).max(0) as usize);
    let mut rfs = rfs;
    let mut dval = 0.0f32;
    for s in 0..d {
        for rr in 0..h {
            for c in 0..w {
                if let Some(f) = rfs.as_deref_mut() {
                    dval = f.draw() as f32;
                }
                if let Some(dv) = dither {
                    dval = mri_get_vox_val(g, r.err, dv, c, rr, s, 0, 0)?;
                }
                let mut val = mri_get_vox_val(g, r.err, mri, c, rr, s, 0, 0)? + dval;
                // C's two comparisons (a NaN kept)
                val = val.clamp(0.0, 255.0);
                a.push(nint(g, f64::from(val)) as u8);
            }
        }
    }
    Ok(a)
}

/// `RFsynth(rf, rfs, NULL)` of the coordinate dither: a uniform draw for each voxel (columns
/// outermost, then rows, slices, frames), stored as `float`.
pub fn rf_synth_dither(f: &mut UniformField, w: i32, h: i32, d: i32, nframes: i32) -> Vec<f32> {
    let (wu, hu, du) = (w.max(0) as usize, h.max(0) as usize, d.max(0) as usize);
    let mut v = vec![0.0f32; wu * hu * du * nframes.max(0) as usize];
    for c in 0..wu {
        for rr in 0..hu {
            for s in 0..du {
                for fr in 0..nframes.max(0) as usize {
                    v[((fr * du + s) * hu + rr) * wu + c] = f.draw() as f32;
                }
            }
        }
    }
    v
}

/// `COREGfwhm(mri, sep, fwhm)`: per axis `1.15 sqrt(sep^2 - voxsize^2)`, or 0.
pub fn coreg_fwhm(mri: &Mri, sep: f64) -> [f64; 3] {
    let vs = [
        f64::from(mri.vg.xsize),
        f64::from(mri.vg.ysize),
        f64::from(mri.vg.zsize),
    ];
    let mut fwhm = [0.0f64; 3];
    for n in 0..3 {
        let val = sep * sep - vs[n] * vs[n];
        fwhm[n] = if val > 0.0 { 1.15 * val.sqrt() } else { 0.0 };
    }
    fwhm
}

/// `MRIgaussianSmoothNI(m, c, r, s, m)` in place.
pub fn smooth_in_place(
    g: &mut Globals,
    r: &mut Run,
    m: &mut Mri,
    std: [f64; 3],
) -> Result<(), Fatal> {
    mri_gaussian_smooth_ni(
        g,
        r.out,
        r.err,
        Pair::Same(m),
        std[0],
        std[1],
        std[2],
        &Smni::default(),
    )?;
    Ok(())
}
