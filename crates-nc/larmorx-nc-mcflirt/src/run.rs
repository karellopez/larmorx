// SPDX-License-Identifier: LicenseRef-FSL
// Translated to Rust from FSL 6.0.7.17: fsl-mcflirt 2111.0 (mcflirt.cc, Log.{h,cc}). Copyright
// (C) 1999-2001 University of Oxford (Peter Bannister). Rust translation Copyright 2026 Karel
// Lopez Vilaret; modified from the original. Distributed under the FSL Licence (see LICENSE).
//! mcflirt's `main`: the stages of line searches, the final resampling and the output files,
//! with larmorx's corrections of mcflirt's defects (D13) unless they are reproduced.

use std::io::Write;

use larmorx_core::defects::{self, Notice, UpstreamDefects};
use larmorx_nc_core::arma::{self, Mat};
use larmorx_nc_core::cnum;
use larmorx_nc_core::costfns::{CostType, Costfn};
use larmorx_nc_core::kernel;
use larmorx_nc_core::miscmaths::{self, MatrixFileError};
use larmorx_nc_core::newimagefns::{self, affine_transform, isotropic_resample};
use larmorx_nc_core::nifti;
use larmorx_nc_core::optimise;
use larmorx_nc_core::settings::Settings;
use larmorx_nc_core::volume::{
    self, Conversion, FslError, Interpolation, SaveError, SaveType, Volume, Extrapolation,
};
use rayon::prelude::*;

use crate::options::{Options, Parsed, parse};

/// The `descrip` field of the images the replica writes (mcflirt writes FSL's build string).
pub const DESCRIP: &[u8] = b"larmorx-nc fsl mcflirt";

/// The prefix of the replica's own lines on standard error.
pub const PREFIX: &str = "larmorx-nc";

/// How a run ends.
enum End {
    /// An uncaught C++ exception in mcflirt (std::terminate, then abort): what it printed.
    Abort(FslError),
    /// `exit(code)` with the lines printed before it.
    Exit(u8, Vec<String>),
    /// A refusal of the corrected command line: exit 1 with this message.
    Refuse(String),
}

impl From<FslError> for End {
    fn from(e: FslError) -> End {
        End::Abort(e)
    }
}

/// Standard error with the state of C++'s `std::cerr` that matters: armawrap's matrix printing
/// switches the stream to fixed notation and never switches it back.
struct Cerr {
    text: String,
    fixed: bool,
}

impl Cerr {
    fn new(fixed: bool) -> Cerr {
        Cerr {
            text: String::new(),
            fixed,
        }
    }

    fn s(&mut self, t: &str) {
        self.text.push_str(t);
    }

    /// `cerr << double` (a float is promoted).
    fn num(&mut self, v: f64) {
        let t = if self.fixed {
            cnum::fmt_f(v, 6)
        } else {
            cnum::stream(v)
        };
        self.text.push_str(&t);
    }

    /// `cerr << Matrix` (armawrap's `doPrint`: fixed, precision 6, a blank after each value).
    fn matrix(&mut self, m: &Mat) {
        self.fixed = true;
        for r in 0..m.rows() {
            for c in 0..m.cols() {
                self.text.push_str(&cnum::fmt_f(m.at(r, c), 6));
                self.text.push(' ');
            }
            self.text.push('\n');
        }
    }
}

/// The mutable part of `Globaloptions` during a run.
#[derive(Clone)]
struct State {
    refnum: i32,
    no_volumes: i32,
    twodcorrect: bool,
    smoothsize: f32,
    maincostfn: CostType,
}

/// What the cost evaluations of one volume need.
struct Search<'a> {
    o: &'a Options,
    st: &'a State,
}

/// The result of registering one volume: its matrix and what it printed.
struct VolumeResult {
    mat: Mat,
    err: String,
    fixed: bool,
}

/// `vector2affine(inparams, n, centre, aff)` (Euler angles).
fn vector2affine(params: &Mat, n: i32, centre: &Mat, twod: bool) -> Mat {
    if n <= 0 {
        return Mat::zeros(4, 4);
    }
    let p = if twod {
        // params = 0; scales 1; rz, tx, ty from the three parameters.
        let mut p = Mat::zeros(12, 1);
        p.put(6, 1.0);
        p.put(7, 1.0);
        p.put(8, 1.0);
        p.put(2, params.get(0));
        p.put(3, params.get(1));
        p.put(4, params.get(2));
        p
    } else {
        params.clone()
    };
    miscmaths::compose_aff(&p, n, centre)
}

/// `set_param_tols(param_tol, 12)` times `new_tolerance`.
fn param_tols(rot_param: f32, new_tolerance: f32) -> Mat {
    let diagonal: [f32; 12] = [
        0.005, 0.005, 0.005, 0.2, 0.2, 0.2, 0.002, 0.002, 0.002, 0.001, 0.001, 0.001,
    ];
    let mut t = Mat::zeros(12, 1);
    for (i, d) in diagonal.iter().enumerate() {
        t.put(i, f64::from(*d));
    }
    for i in 0..3 {
        t.put(i, t.get(i) / f64::from(rot_param));
    }
    t.scale(f64::from(new_tolerance))
}

impl Search<'_> {
    /// `costfn(params)`: the cost of the parameters, printing the matrix with `-verbose 20`.
    /// `no_params` is `Globaloptions::no_params`, which `optimise_strategy1` sets to the
    /// corrected dof before every search (`-dof` sets it too, but no cost is evaluated before).
    fn cost(&self, cf: &mut Costfn, params: &Mat, no_params: i32, cerr: &mut Cerr) -> f32 {
        let centre = Mat::col(&cf.test_cog);
        let aff = vector2affine(params, no_params, &centre, self.st.twodcorrect);
        // affmat = uninitaffmat * initmat (the identity): a 4x4 product.
        let affmat = arma::mul_tinysq(&aff, &Mat::eye(4));
        if self.o.verbose >= 20 {
            cerr.s("Cost::affmat = \n");
            cerr.matrix(&affmat);
            cerr.s("\n");
        }
        cf.costtype = self.st.maincostfn;
        cf.cost(&affmat)
    }

    /// `usroptimise(matresult, dof, 1, new_tolerance)` → `optimise_strategy1`.
    fn usroptimise(&self, cf: &mut Costfn, matresult: &Mat, new_tolerance: f32, cerr: &mut Cerr) -> Mat {
        // usroptimise(matresult, globalopts.dof, ...): Min(globalopts.dof, usrdof), the same.
        let mut dof = self.o.dof;
        if dof < 6 {
            cerr.s(&format!("Erroneous dof {dof} : using 6 instead\n"));
            dof = 6;
        }
        if dof > 12 {
            cerr.s(&format!("Erroneous dof {dof} : using 12 instead\n"));
            dof = 12;
        }
        let param_tol = param_tols(self.o.rot_param, new_tolerance);
        let centre = Mat::col(&cf.test_cog);
        let p12 = miscmaths::decompose_aff(matresult, &centre);
        let mut params = Mat::col(&p12);
        let twod = self.st.twodcorrect;
        let mut messages: Vec<String> = Vec::new();
        // mcflirt's optimise(): the in-plane mode searches rz, tx, ty only.
        let (mut pvec, tols) = if twod {
            (
                Mat::col(&[params.get(2), params.get(3), params.get(4)]),
                Mat::col(&[param_tol.get(2), param_tol.get(3), param_tol.get(4)]),
            )
        } else {
            (params.clone(), param_tol.clone())
        };
        let no_params = if twod { 3 } else { dof as usize };
        // powell_opt: parameters all below 0.001 in magnitude are reset to the identity's.
        if pvec.max_abs() < 0.001 {
            let init = [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0];
            for i in 0..pvec.len() {
                pvec.put(i, init[i]);
            }
        }
        {
            let mut f = |p: &Mat| -> f32 { self.cost(cf, p, dof.max(6), cerr) };
            let _ = optimise::optimise(
                &mut pvec,
                no_params,
                &tols,
                &mut f,
                1,
                &[10.0, 1.0],
                &mut messages,
            );
        }
        for m in &messages {
            cerr.s(m);
            cerr.s("\n");
        }
        if twod {
            cerr.s("Params: ");
            cerr.num(pvec.get(0));
            cerr.s(" : ");
            cerr.num(pvec.get(1));
            cerr.s(" : ");
            cerr.num(pvec.get(2));
            cerr.s("\n");
            cerr.s("Tolerances: ");
            cerr.num(tols.get(0));
            cerr.s(" : ");
            cerr.num(tols.get(1));
            cerr.s(" : ");
            cerr.num(tols.get(2));
            cerr.s("\n");
            params = pvec;
        } else {
            params = pvec;
        }
        vector2affine(&params, dof, &centre, twod)
    }
}

/// `double_end_slices(testvol)`: one replicated slice added at each end, slice thickness 8 mm,
/// the sform and qform not carried over.
fn double_end_slices(v: &Volume) -> Volume {
    let (nx, ny, nz) = (v.xsize(), v.ysize(), v.zsize());
    let mut n = Volume::new(nx, ny, nz + 2, 1);
    n.setdims(v.xdim(), v.ydim(), 8.0);
    for z in 0..nz + 2 {
        let mut ez = z - 1;
        if ez < 0 {
            ez = 0;
        }
        if ez >= nz {
            ez = nz - 1;
        }
        for y in 0..ny {
            for x in 0..nx {
                let i = n.idx(x, y, z);
                n.data[i] = v.value(x, y, ez);
            }
        }
    }
    n
}

/// `fix2D(vol)`: the in-plane mode for thin images (sets it for the rest of the run).
fn fix2d(vol: &mut Volume, st: &mut State, fov: f32) {
    let thin = vol.zsize() < 3 || ((vol.zsize() as f32) * vol.zdim()) < fov || st.twodcorrect;
    if thin {
        st.twodcorrect = true;
        st.smoothsize = 0.1;
        *vol = double_end_slices(vol);
    }
}

/// The run's settings and outputs.
struct Run<'a> {
    o: Options,
    st: State,
    mode: UpstreamDefects,
    settings: &'a Settings,
    n_threads: usize,
    out: &'a mut dyn Write,
    err: &'a mut dyn Write,
    fixed: bool,
    notices: Vec<Notice>,
}

impl Run<'_> {
    fn e(&mut self, t: &str) {
        let _ = self.err.write_all(t.as_bytes());
    }

    fn report(&mut self, t: &str) {
        if !self.o.no_reporting {
            self.e(t);
        }
    }

    /// `cerr << float` with the stream's current notation.
    fn num(&self, v: f64) -> String {
        if self.fixed {
            cnum::fmt_f(v, 6)
        } else {
            cnum::stream(v)
        }
    }

    fn notice(&mut self, defect: &'static defects::Defect, detail: impl Into<String>) {
        defects::push_once(&mut self.notices, Notice::new(defect, self.mode, detail));
    }

    /// `correct(direction, reference_volume, timeseries, scaling, new_tolerance, in, out,
    /// mean_cond)`.
    #[allow(clippy::too_many_arguments)]
    fn correct(
        &mut self,
        direction: i32,
        reference_volume: &Volume,
        timeseries: &Volume,
        scaling: f32,
        new_tolerance: f32,
        mat_in: &mut [Mat],
        mat_out: &mut [Mat],
        mean_cond: i32,
    ) -> Result<(), End> {
        let mut i = self.st.refnum + direction;
        let mut stop = -1 - mean_cond;
        let mut refvol = reference_volume.clone();
        fix2d(&mut refvol, &mut self.st, self.o.fov);
        if !self.o.no_reporting && self.st.twodcorrect {
            self.e("restricting optimization to R_z, T_x and T_y\n");
        }
        if i == -2 && stop == -1 {
            stop = -2;
        }
        let n = self.st.no_volumes;
        let end = if direction == 1 { n } else { stop };
        // The volumes this pass visits, in order.
        let mut order = Vec::new();
        let mut k = i;
        while k != end {
            order.push(k);
            k += direction;
            if order.len() > (n as usize + 4) {
                break;
            }
        }
        let chained = scaling == 8.0 && !self.o.fudgeflag;
        let last_init = if self.mode.fix() { n } else { n - 1 };
        let mut pos = 0;
        while pos < order.len() {
            i = order[pos];
            if !chained && pos > 0 && self.n_threads != 1 {
                // The rest of an unchained pass: independent volumes, in parallel, their messages
                // printed in order.
                let rest: Vec<i32> = order[pos..].to_vec();
                let st = self.st.clone();
                let o = self.o.clone();
                let mode = self.mode;
                let fixed = self.fixed;
                let results: Vec<Result<VolumeResult, FslError>> = with_pool(self.n_threads, || {
                    rest.par_iter()
                        .map(|&vi| {
                            register_volume(&o, &st, mode, &refvol, timeseries, vi, scaling, new_tolerance, &mat_in[vi as usize], fixed)
                        })
                        .collect()
                });
                for (r, &vi) in results.into_iter().zip(&rest) {
                    if !self.o.no_reporting {
                        self.e(&format!("[{vi}]"));
                    }
                    let r = r?;
                    self.e(&r.err);
                    self.fixed = r.fixed;
                    mat_out[vi as usize] = r.mat;
                }
                return Ok(());
            }
            if !self.o.no_reporting {
                self.e(&format!("[{i}]"));
            }
            if i < 0 || i >= n {
                return Err(FslError::imthrow("Invalid t index in [] operator", 61).into());
            }
            let r = register_volume_mut(
                &self.o,
                &mut self.st,
                self.mode,
                &refvol,
                timeseries,
                i,
                scaling,
                new_tolerance,
                &mat_in[i as usize],
                self.fixed,
            )?;
            self.e(&r.err);
            self.fixed = r.fixed;
            mat_out[i as usize] = r.mat.clone();
            let next = i + direction;
            if scaling == 8.0 && next < last_init && next > -1 && !self.o.fudgeflag {
                if self.mode.fix() && next == n - 1 {
                    self.notice(
                        &defects::FSL_01,
                        "the last volume starts its first stage from its predecessor's result; \
                         mcflirt starts it from the identity",
                    );
                }
                mat_in[next as usize] = r.mat;
            } else if self.mode.reproduce()
                && scaling == 8.0
                && next == n - 1
                && !self.o.fudgeflag
                && next > -1
            {
                self.notice(
                    &defects::FSL_01,
                    "the last volume starts its first stage from the identity, as in mcflirt",
                );
            }
            pos += 1;
        }
        Ok(())
    }
}

/// A default-constructed `volume<float>`: no voxels, voxel sizes 1.
fn empty_volume() -> Volume {
    Volume::new(0, 0, 0, 1)
}

/// Runs `f` in a pool of `n` threads (0: all logical CPUs).
fn with_pool<R: Send>(n: usize, f: impl FnOnce() -> R + Send) -> R {
    match rayon::ThreadPoolBuilder::new().num_threads(n).build() {
        Ok(pool) => pool.install(f),
        Err(_) => f(),
    }
}

/// One volume of `correct()` when the state cannot change (later volumes of a pass).
#[allow(clippy::too_many_arguments)]
fn register_volume(
    o: &Options,
    st: &State,
    mode: UpstreamDefects,
    refvol: &Volume,
    timeseries: &Volume,
    i: i32,
    scaling: f32,
    new_tolerance: f32,
    start: &Mat,
    fixed: bool,
) -> Result<VolumeResult, FslError> {
    let mut st = st.clone();
    register_volume_mut(o, &mut st, mode, refvol, timeseries, i, scaling, new_tolerance, start, fixed)
}

/// One volume of `correct()`: the test volume (in-plane padded if needed), its cost function and
/// the line searches from `start`.
#[allow(clippy::too_many_arguments)]
fn register_volume_mut(
    o: &Options,
    st: &mut State,
    mode: UpstreamDefects,
    refvol: &Volume,
    timeseries: &Volume,
    i: i32,
    scaling: f32,
    new_tolerance: f32,
    start: &Mat,
    fixed: bool,
) -> Result<VolumeResult, FslError> {
    let mut cerr = Cerr::new(fixed);
    let mut testvol = timeseries.sub_volume(i64::from(i))?;
    if o.gdtflag {
        // -gdt (reached only when reproducing, FSL-04): the test volume becomes its gradient
        // image; the reference's gradient is computed and discarded.
        if !o.no_reporting {
            cerr.s(&format!("Calculating gradient image for volume [{i}]\n"));
        }
        testvol = newimagefns::gradient(&testvol);
    }
    fix2d(&mut testvol, st, o.fov);
    let offsettrans = start.clone();
    // usrsetscale(refvol, testvol, (int) scaling)
    let mut warnings = Vec::new();
    let mut cf = new_costfn(refvol.clone(), testvol, &mut warnings, mode.fix());
    for w in &warnings {
        cerr.s(w);
        cerr.s("\n");
    }
    cf.set_no_bins(o.no_bins / scaling as i32);
    cf.smoothsize = st.smoothsize;
    cf.skip_nonfinite = mode.fix();
    let search = Search { o, st };
    let res = search.usroptimise(&mut cf, &offsettrans, new_tolerance, &mut cerr);
    // finalmat = offsettrans * initmat (the identity).
    let finalmat = arma::mul_tinysq(&res, &Mat::eye(4));
    Ok(VolumeResult {
        mat: finalmat,
        err: cerr.text,
        fixed: cerr.fixed,
    })
}

/// Runs mcflirt with `args` (without the program name). Returns the exit status.
pub fn mcflirt(
    args: &[String],
    settings: &Settings,
    mode: UpstreamDefects,
    n_threads: usize,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let mut notices = Vec::new();
    let (parsed, perr) = parse(args, mode, &mut notices);
    let _ = err.write_all(perr.as_bytes());
    let o = match parsed {
        Parsed::Exit(e) => {
            let _ = out.write_all(e.stdout.as_bytes());
            let _ = err.write_all(e.stderr.as_bytes());
            print_notices(err, &notices);
            return e.code;
        }
        Parsed::Run(o) => *o,
    };
    print_notices(err, &notices);
    let st = State {
        refnum: o.refnum,
        no_volumes: 0,
        twodcorrect: o.twodcorrect,
        smoothsize: o.smoothsize,
        maincostfn: o.maincostfn,
    };
    let mut run = Run {
        o,
        st,
        mode,
        settings,
        n_threads,
        out,
        err,
        fixed: false,
        notices: Vec::new(),
    };
    let end = run.main();
    let notices = std::mem::take(&mut run.notices);
    let code = match end {
        Ok(()) => 0,
        Err(End::Exit(code, lines)) => {
            for l in lines {
                run.e(&l);
                run.e("\n");
            }
            code
        }
        Err(End::Abort(e)) => {
            run.e(&e.stderr_text());
            run.e(&format!(
                "{PREFIX}: mcflirt ends here with an uncaught exception (abort, signal 6); the \
                 replica exits with status 1\n"
            ));
            1
        }
        Err(End::Refuse(msg)) => {
            run.e(&format!("Error: {msg}\n"));
            1
        }
    };
    print_notices(run.err, &notices);
    code
}

fn print_notices(err: &mut dyn Write, notices: &[Notice]) {
    for n in notices {
        let _ = writeln!(err, "{PREFIX}: {}", n.line());
    }
}

impl Run<'_> {
    /// `main()` after the command line.
    fn main(&mut self) -> Result<(), End> {
        self.report("\nMcFLIRT v 2.0 - FMRI motion correction\n\n");
        if self.mode.fix() && self.o.gdtflag {
            return Err(End::Refuse(
                "-gdt (registration of gradient images) is not supported: mcflirt computes the \
                 reference's gradient and discards it (FSL-04)"
                    .into(),
            ));
        }
        self.report("Reading time series... \n");
        let (mut timeseries, _, in_units) = volume::read_volume4d(&self.o.inputfname, self.settings)?;
        let datatype = volume::file_dtype(&self.o.inputfname)?;
        self.st.no_volumes = timeseries.tsize() as i32;
        let n = self.st.no_volumes;
        let original_refvol = self.o.refnum;
        if self.st.refnum == -1 {
            self.st.refnum = n / 2;
        }
        if self.mode.fix() {
            self.precheck(&timeseries)?;
        }
        let mut mat0: Vec<Mat> = (0..n).map(|_| Mat::eye(4)).collect();
        let mut mat1: Vec<Mat> = (0..n).map(|_| Mat::eye(4)).collect();
        let mut mat2: Vec<Mat> = (0..n).map(|_| Mat::eye(4)).collect();
        let mut current_scale: f32 = 8.0;
        let mut new_tolerance: f32;
        let mut mean_cond = 0;
        let mut extrefvol: Option<Volume> = None;
        let mut anisorefvol: Option<Volume> = None;
        let mut meanvol: Option<Volume> = None;
        let mut refvol: Option<Volume> = None;
        let mean_its = if self.o.meanvol { 2 } else { 1 };
        for mean_it in 0..mean_its {
            if self.o.no_stages >= 1 {
                self.report("first iteration - 8mm scaling, set tolerance\n");
                new_tolerance = (8.0 * 0.2 * 0.5) as f32;
                current_scale = 8.0;
                if mean_it == 0 {
                    if self.o.reffileflag {
                        let (r, warning) = volume::read_volume(&self.o.reffilename, self.settings)?;
                        if let Some(w) = warning {
                            self.e(&w);
                            self.e("\n");
                        }
                        anisorefvol = Some(r.clone());
                        extrefvol = Some(r);
                        self.st.refnum = -1;
                    } else {
                        anisorefvol = Some(timeseries.sub_volume(i64::from(self.st.refnum))?);
                    }
                } else {
                    // The second pass: the mean of the first pass's corrected volumes.
                    let mut mv = timeseries.sub_volume(0)?;
                    mv.data.iter_mut().for_each(|v| *v = 0.0);
                    timeseries.extrap = Extrapolation::Extraslice;
                    timeseries.interp = Interpolation::Trilinear;
                    for i in 0..n {
                        let src = timeseries.sub_volume(i64::from(i))?;
                        let mut testvol = src.clone();
                        affine_transform(&src, &mut testvol, &mat1[i as usize], 1.0)?;
                        for (m, t) in mv.data.iter_mut().zip(&testvol.data) {
                            *m += *t;
                        }
                    }
                    let nf = n as f32;
                    for m in mv.data.iter_mut() {
                        *m /= nf;
                    }
                    self.save(&mv, &self.derived_name("_mean_reg"), SaveType::Float, None)?;
                    anisorefvol = Some(mv.clone());
                    meanvol = Some(mv);
                    self.st.refnum = -1;
                    for m in mat0.iter_mut() {
                        *m = Mat::eye(4);
                    }
                    mean_cond = 1;
                }
                let msg = format!(
                    "Rescaling reference volume [{}] to {} mm pixels\n",
                    self.st.refnum,
                    self.num(f64::from(current_scale))
                );
                self.report(&msg);
                let aniso = anisorefvol.as_ref().expect("reference");
                let mut warnings = Vec::new();
                let mut rv = isotropic_resample(aniso, current_scale, &mut warnings);
                for w in warnings {
                    self.e(&w);
                    self.e("\n");
                }
                // FSL-03: mcflirt pads the first stage's reference here and again in correct().
                if self.mode.reproduce() {
                    fix2d(&mut rv, &mut self.st, self.o.fov);
                    if self.st.twodcorrect {
                        self.notice(
                            &defects::FSL_03,
                            "the in-plane mode pads the first stage's reference grid twice, as \
                             in mcflirt",
                        );
                    }
                } else {
                    let probe = rv.zsize() < 3
                        || ((rv.zsize() as f32) * rv.zdim()) < self.o.fov
                        || self.st.twodcorrect;
                    if probe {
                        self.notice(
                            &defects::FSL_03,
                            "the in-plane mode pads the first stage's reference grid once; \
                             mcflirt pads it twice",
                        );
                    }
                }
                if self.o.gdtflag {
                    self.report("Calculating gradient image for reference volume\n");
                    self.report("Saving gradient reference volume... \n");
                    let name = format!("grefvol_{}", self.o.outputfname);
                    self.save(&rv, &name, SaveType::Float, None)?;
                }
                refvol = Some(rv.clone());
                self.report("Registering volumes ... ");
                let mut m0 = std::mem::take(&mut mat0);
                let mut m1 = std::mem::take(&mut mat1);
                self.correct(1, &rv, &timeseries, current_scale, new_tolerance, &mut m0, &mut m1, mean_cond)?;
                self.correct(-1, &rv, &timeseries, current_scale, new_tolerance, &mut m0, &mut m1, mean_cond)?;
                mat0 = m0;
                mat1 = m1;
            } else {
                mat1.clone_from(&mat0);
            }
            if self.o.no_stages >= 2 {
                self.report("\nsecond iteration - drop to 4mm scaling\n");
                new_tolerance = (4.0 * 0.2) as f32;
                current_scale = 4.0;
                let msg = format!(
                    "Rescaling reference volume [{}] to {} mm pixels\n",
                    self.st.refnum,
                    self.num(f64::from(current_scale))
                );
                self.report(&msg);
                let aniso = anisorefvol.as_ref().expect("reference");
                let mut warnings = Vec::new();
                let rv = isotropic_resample(aniso, current_scale, &mut warnings);
                for w in warnings {
                    self.e(&w);
                    self.e("\n");
                }
                if self.o.gdtflag {
                    self.report("Calculating gradient image for reference volume\n");
                    self.report("Saving gradient reference volume... \n");
                    let name = format!("grefvol_{}", self.o.outputfname);
                    self.save(&rv, &name, SaveType::Float, None)?;
                }
                refvol = Some(rv.clone());
                self.report("Registering volumes ... ");
                let mut m1 = std::mem::take(&mut mat1);
                let mut m2 = std::mem::take(&mut mat2);
                self.correct(1, &rv, &timeseries, current_scale, new_tolerance, &mut m1, &mut m2, mean_cond)?;
                self.correct(-1, &rv, &timeseries, current_scale, new_tolerance, &mut m1, &mut m2, mean_cond)?;
                mat1 = m1;
                mat2 = m2;
            } else {
                mat2.clone_from(&mat1);
            }
            if self.o.no_stages >= 3 {
                self.report("\nthird iteration - 4mm scaling, eighth tolerance\n");
                new_tolerance = 0.1;
                self.report("Registering volumes ... ");
                let rv = refvol.clone().expect("the reference of stage 2");
                let mut m2 = std::mem::take(&mut mat2);
                let mut m1 = std::mem::take(&mut mat1);
                self.correct(1, &rv, &timeseries, current_scale, new_tolerance, &mut m2, &mut m1, mean_cond)?;
                self.correct(-1, &rv, &timeseries, current_scale, new_tolerance, &mut m2, &mut m1, mean_cond)?;
                mat2 = m2;
                mat1 = m1;
            } else {
                mat1.clone_from(&mat2);
            }
        }
        mean_cond = 0;
        if self.o.no_stages >= 4 {
            self.report("\nfourth iteration - 4mm scaling, eighth tolerance, sinc interpolation\n");
            self.report("Registering volumes ... ");
            if self.st.maincostfn == CostType::NormCorr {
                self.st.maincostfn = CostType::NormCorrSinc;
            }
            let rv = refvol.clone().expect("the reference of stage 3");
            let mut m1 = std::mem::take(&mut mat1);
            let mut m0 = std::mem::take(&mut mat0);
            self.correct(1, &rv, &timeseries, current_scale, 0.1, &mut m1, &mut m0, mean_cond)?;
            self.correct(-1, &rv, &timeseries, current_scale, 0.1, &mut m1, &mut m0, mean_cond)?;
            mat0 = m0;
        } else {
            mat0.clone_from(&mat1);
        }
        let init_trans = if self.o.init_transform.is_empty() {
            Mat::eye(4)
        } else {
            match miscmaths::read_ascii_matrix(&self.o.init_transform) {
                Ok(m) => m,
                Err(MatrixFileError::CannotOpen) => {
                    self.e(&format!("Could not open matrix file {}\n", self.o.init_transform));
                    Mat::zeros(0, 0)
                }
                Err(MatrixFileError::NotEnoughNumbers) => {
                    return Err(End::Abort(FslError {
                        printed: Vec::new(),
                        type_name: "armawrap::AWException",
                        what: "Could not find enough numbers in matrix file".into(),
                    }));
                }
            }
        };
        if init_trans.rows() != 4 || init_trans.cols() != 4 {
            return Err(End::Abort(FslError {
                printed: Vec::new(),
                type_name: "std::logic_error",
                what: format!(
                    "matrix multiplication: incompatible matrix dimensions: 4x4 and {}x{}",
                    init_trans.rows(),
                    init_trans.cols()
                ),
            }));
        }
        // The final resampling.
        timeseries.extrap = Extrapolation::Extraslice;
        timeseries.interp = if self.o.sinc_final {
            Interpolation::Sinc
        } else if self.o.nn_final {
            Interpolation::NearestNeighbour
        } else if self.o.spline_final {
            Interpolation::Spline
        } else {
            Interpolation::Trilinear
        };
        if self.o.sinc_final && timeseries.kernel.is_none() {
            let mut msgs = Vec::new();
            timeseries.kernel = Some(std::sync::Arc::new(kernel::sinc_kernel("blackman", 7, 1201, &mut msgs)));
        }
        // With a reference file the output grid is the reference's; mcflirt reads it in the
        // first stage only, so with `-stages 0` it is still the empty volume (`volume<float>
        // extrefvol;`) and the resampling throws (the corrected run refused this earlier).
        let template = if self.o.reffileflag {
            Some(extrefvol.clone().unwrap_or_else(empty_volume))
        } else {
            None
        };
        let mats: Vec<Mat> = mat0.iter().map(|m| arma::mul_tinysq(m, &init_trans)).collect();
        let resampled: Vec<Result<Volume, FslError>> = with_pool(self.n_threads, || {
            (0..n)
                .into_par_iter()
                .map(|i| {
                    let src = timeseries.sub_volume(i64::from(i))?;
                    let mut testvol = template.clone().unwrap_or_else(|| src.clone());
                    affine_transform(&src, &mut testvol, &mats[i as usize], 1.0)?;
                    Ok(testvol)
                })
                .collect()
        });
        for (i, r) in resampled.into_iter().enumerate() {
            let testvol = r?;
            timeseries.set_sub_volume(i as i64, &testvol)?;
        }
        if self.o.statflag {
            self.stats(&timeseries)?;
        }
        if self.o.tmpmatflag {
            // decompose_mats takes only the reference's grid (its centre); the volumes are
            // empty when no stage ran (`-stages 0`).
            let rv = if self.o.reffileflag {
                extrefvol.clone().unwrap_or_else(empty_volume)
            } else if self.o.meanvol {
                meanvol.clone().unwrap_or_else(empty_volume)
            } else {
                if !self.o.no_reporting {
                    let _ = write!(
                        self.out,
                        "refnum = {}\nOriginal_refvol = {}\n",
                        self.st.refnum, original_refvol
                    );
                }
                if self.st.refnum < 0 {
                    return Err(End::Abort(FslError {
                        printed: Vec::new(),
                        type_name: "std::runtime_error",
                        what: "Negative refnum encountered when not in refile/meanvol mode.".into(),
                    }));
                }
                timeseries.sub_volume(i64::from(self.st.refnum))?
            };
            self.decompose_mats(&mat0, &rv)?;
        }
        self.report("\nSaving motion corrected time series... \n");
        let (mx, mn) = timeseries.extrema();
        timeseries.display_max = mx;
        timeseries.display_min = mn;
        self.save_series(&timeseries, datatype, in_units)?;
        Ok(())
    }

    /// Checks of the corrected command line before any work (FSL-12): the inputs mcflirt
    /// accepts and then aborts on.
    fn precheck(&mut self, timeseries: &Volume) -> Result<(), End> {
        let n = self.st.no_volumes;
        if !self.o.reffileflag && (self.st.refnum < 0 || self.st.refnum >= n) {
            return Err(End::Refuse(format!(
                "-refvol {}: invalid volume index for a series of {n} volumes (mcflirt aborts: \
                 \"Invalid t index in [] operator\")",
                self.o.refnum
            )));
        }
        if self.o.reffileflag {
            if self.o.no_stages <= 0 {
                return Err(End::Refuse(
                    "-stages 0 with a reference file: nothing would be registered (mcflirt aborts \
                     after the estimation)"
                        .into(),
                ));
            }
            let (r, _) = volume::read_volume(&self.o.reffilename, self.settings)?;
            if r.nvoxels() != timeseries.nvoxels() {
                return Err(End::Refuse(format!(
                    "the reference {} has {} voxels and the series' volumes {}: mcflirt aborts \
                     after the estimation (\"Attempted to copydata with non-matching sizes\")",
                    self.o.reffilename,
                    r.nvoxels(),
                    timeseries.nvoxels()
                )));
            }
        }
        Ok(())
    }

    /// `outputfname + suffix`, or, corrected (FSL-08), the stem of `-out` without its image
    /// extension plus `suffix`.
    fn derived_name(&self, suffix: &str) -> String {
        if self.mode.fix() {
            format!("{}{suffix}", volume::make_basename(&self.o.outputfname))
        } else {
            format!("{}{suffix}", self.o.outputfname)
        }
    }

    /// `save_volume(vol, name)`, with FSLOUTPUTTYPE's failure (exit 1) when it is not set and
    /// mcflirt's behaviour is reproduced; corrected, `.nii.gz` (larmorx's behaviour, FSL-11).
    fn save(&mut self, v: &Volume, name: &str, ty: SaveType, code: Option<i16>) -> Result<(), End> {
        let settings = self.output_settings();
        let units = nifti::UNITS_MM_SEC;
        match v.save_typed(name, ty, Conversion::X86Cast, &settings, DESCRIP, code, units) {
            Ok(()) => Ok(()),
            Err(SaveError::OutputType(e)) => Err(End::Exit(1, e.printed)),
            Err(SaveError::Exception(e)) => Err(End::Abort(e)),
        }
    }

    /// The settings saving reads: corrected, `FSLOUTPUTTYPE` defaults to `NIFTI_GZ`.
    fn output_settings(&self) -> Settings {
        if self.mode.fix() && self.settings.get("FSLOUTPUTTYPE").is_none() {
            self.settings.clone().with("FSLOUTPUTTYPE", "NIFTI_GZ")
        } else {
            self.settings.clone()
        }
    }

    /// `save_volume_dtype(timeseries, outputfname, datatype)`, with FSL-09 and FSL-10 corrected
    /// unless reproduced.
    fn save_series(&mut self, ts: &Volume, datatype: i16, in_units: i32) -> Result<(), End> {
        let ty = match volume::save_type_for(datatype) {
            Ok(t) => t,
            Err(msg) => {
                // closestTemplatedType printed the message; save_volume_dtype then perror()s.
                self.e(&format!("{msg}\n"));
                self.e(&format!(
                    "NEWIMAGE::save_volume_dtype: DT {datatype} not supported: Success\n"
                ));
                return Ok(());
            }
        };
        let (ty, conv, code) = if self.mode.fix() {
            let int8 = datatype == nifti::dt::INT8;
            let integer = matches!(ty, SaveType::Char | SaveType::Short | SaveType::Int);
            if int8 {
                self.notice(
                    &defects::FSL_09,
                    "int8 data are written as int8, rounded and clamped; mcflirt writes uint8 and \
                     truncates",
                );
                (SaveType::Int8, Conversion::RoundClamp, Some(nifti::dt::INT8))
            } else if integer {
                let rounded = ts
                    .data
                    .iter()
                    .any(|v| v.round_ties_even() != v.trunc() || !v.is_finite());
                if rounded {
                    self.notice(
                        &defects::FSL_09,
                        "integer outputs are rounded to the nearest and clamped; mcflirt \
                         truncates toward zero and wraps out-of-range values",
                    );
                }
                (ty, Conversion::RoundClamp, None)
            } else {
                (ty, Conversion::X86Cast, None)
            }
        } else {
            if datatype == nifti::dt::INT8 {
                self.notice(&defects::FSL_09, "int8 data are written as uint8, as in mcflirt");
            }
            (ty, Conversion::X86Cast, None)
        };
        let units = if self.mode.fix() {
            let space = in_units & 0x07;
            let time = in_units & 0x38;
            let u = (if space == 0 { 2 } else { space }) | (if time == 0 { 8 } else { time });
            if u != nifti::UNITS_MM_SEC {
                self.notice(
                    &defects::FSL_10,
                    "the output keeps the input's units; mcflirt labels them millimetres and \
                     seconds",
                );
            }
            u
        } else {
            if (in_units & 0x3f) != 0 && (in_units & 0x3f) != nifti::UNITS_MM_SEC {
                self.notice(
                    &defects::FSL_10,
                    "the output is labelled millimetres and seconds whatever the input's units, \
                     as in mcflirt",
                );
            }
            nifti::UNITS_MM_SEC
        };
        let settings = self.output_settings();
        match ts.save_typed(&self.o.outputfname, ty, conv, &settings, DESCRIP, code, units) {
            Ok(()) => Ok(()),
            Err(SaveError::OutputType(e)) => Err(End::Exit(1, e.printed)),
            Err(SaveError::Exception(e)) => Err(End::Abort(e)),
        }
    }

    /// `run_and_save_stats(timeseries)`: mean, variance and standard deviation over time; mcflirt
    /// leaves the first and last slices at 0 (FSL-08, corrected unless reproduced).
    fn stats(&mut self, ts: &Volume) -> Result<(), End> {
        let vmax = ts.tsize();
        let mut meanvol = ts.sub_volume(0)?;
        meanvol.data.iter_mut().for_each(|v| *v = 0.0);
        let mut variancevol = meanvol.clone();
        let mut sigmavol = meanvol.clone();
        let (nx, ny, nz) = (ts.xsize(), ts.ysize(), ts.zsize());
        let (z0, z1) = if self.mode.fix() { (0, nz) } else { (1, nz - 1) };
        let nv = ts.nvoxels() as usize;
        for x in 0..nx {
            for y in 0..ny {
                for z in z0..z1 {
                    let k = meanvol.idx(x, y, z);
                    let mut m = meanvol.data[k];
                    for i in 0..vmax as usize {
                        m += ts.data[i * nv + k];
                    }
                    meanvol.data[k] = m / vmax as f32;
                }
            }
        }
        for x in 0..nx {
            for y in 0..ny {
                for z in z0..z1 {
                    let k = meanvol.idx(x, y, z);
                    let mean = meanvol.data[k];
                    let mut v = variancevol.data[k];
                    for i in 0..vmax as usize {
                        let d = ts.data[i * nv + k] - mean;
                        v += d * d;
                    }
                    let var = v / (vmax - 1) as f32;
                    variancevol.data[k] = var;
                    sigmavol.data[k] = var.sqrt();
                }
            }
        }
        let detail = if self.mode.fix() {
            format!(
                "the -stats and -meanvol images cover every slice and are named after '{}'; \
                 mcflirt leaves the first and last slices at 0 and names them after '{}'",
                volume::make_basename(&self.o.outputfname),
                self.o.outputfname
            )
        } else {
            format!(
                "the -stats images leave the first and last slices at 0 and are named after '{}', \
                 as in mcflirt",
                self.o.outputfname
            )
        };
        self.notice(&defects::FSL_08, detail);
        self.report("Saving mean volume... \n");
        self.save(&meanvol, &self.derived_name("_meanvol"), SaveType::Float, None)?;
        self.report("Saving variance volume... \n");
        self.save(&variancevol, &self.derived_name("_variance"), SaveType::Float, None)?;
        self.report("Saving standard deviation volume... \n");
        self.save(&sigmavol, &self.derived_name("_sigma"), SaveType::Float, None)?;
        Ok(())
    }

    /// `decompose_mats(mat_index, mat_array, refvol)`: the `.mat` directory, the `.par` and
    /// `.rms` files.
    fn decompose_mats(&mut self, mats: &[Mat], refvol: &Volume) -> Result<(), End> {
        let n = self.st.no_volumes as usize;
        let rmax: f32 = 80.0;
        let out = self.o.outputfname.clone();
        let mut center = Mat::zeros(3, 1);
        center.put(0, 0.5 * (refvol.xsize() as f64 - 1.0) * f64::from(refvol.xdim()));
        center.put(1, 0.5 * (refvol.ysize() as f64 - 1.0) * f64::from(refvol.ydim()));
        center.put(2, 0.5 * (refvol.zsize() as f64 - 1.0) * f64::from(refvol.zdim()));
        let mut par = String::new();
        let mut tmpmatflag = self.o.tmpmatflag;
        // FSL-13: mcflirt creates the .mat directory for -rmsrel and -rmsabs without -mats.
        let make_dir = if self.mode.fix() {
            if (self.o.rmsrelflag || self.o.rmsabsflag) && !self.o.matflag {
                self.notice(
                    &defects::FSL_13,
                    "without -mats no .mat directory is created; mcflirt creates one for -rmsrel \
                     and -rmsabs",
                );
            }
            self.o.matflag
        } else {
            if (self.o.rmsrelflag || self.o.rmsabsflag) && !self.o.matflag {
                self.notice(
                    &defects::FSL_13,
                    "-rmsrel and -rmsabs create the .mat directory without -mats, as in mcflirt",
                );
            }
            self.o.matflag || self.o.rmsrelflag || self.o.rmsabsflag
        };
        let mut dir: Option<String> = None;
        if make_dir {
            match establish_dir(&format!("{out}.mat")) {
                Ok(d) => dir = Some(d),
                Err(DirError::Access(d)) => {
                    let pathname = volume::find_pathname(&self.o.inputfname);
                    self.e(&format!(
                        "Error! Could not create directory: {pathname}{d}. No write permission\n"
                    ));
                    tmpmatflag = false;
                }
                Err(DirError::Other(d)) => {
                    if self.mode.fix() {
                        return Err(End::Refuse(format!(
                            "cannot create the directory {d} for the matrices (FSL-20: mcflirt \
                             prints \"Error!: Unknown Log directory return value\" and exits \
                             with status 0)"
                        )));
                    }
                    return Err(End::Exit(
                        0,
                        vec!["Error!: Unknown Log directory return value".into()],
                    ));
                }
            }
        }
        if self.o.rmsrelflag {
            let mut text = String::new();
            let mut mean: f32 = 0.0;
            for i in 1..n {
                let r = miscmaths::rms_deviation(&mats[i - 1], &mats[i], &center, rmax)
                    .map_err(|_| rms_error())?;
                text.push_str(&cnum::stream(f64::from(r)));
                text.push('\n');
                mean += r;
            }
            write_text(&format!("{out}_rel.rms"), &text);
            let m = mean / (self.st.no_volumes - 1) as f32;
            write_text(&format!("{out}_rel_mean.rms"), &format!("{}\n", cnum::stream(f64::from(m))));
        }
        if self.o.rmsabsflag {
            let mut text = String::new();
            let mut mean: f32 = 0.0;
            for m in mats.iter().take(n) {
                let r = miscmaths::rms_deviation(&Mat::eye(4), m, &center, rmax)
                    .map_err(|_| rms_error())?;
                text.push_str(&cnum::stream(f64::from(r)));
                text.push('\n');
                mean += r;
            }
            write_text(&format!("{out}_abs.rms"), &text);
            let m = mean / self.st.no_volumes as f32;
            write_text(&format!("{out}_abs_mean.rms"), &format!("{}\n", cnum::stream(f64::from(m))));
        }
        let mut warnings = Vec::new();
        for (i, m) in mats.iter().enumerate().take(n) {
            let name = format!("MAT_{i:04}");
            let is_ref = i as i32 == self.st.refnum;
            let product = if is_ref { Mat::eye(4) } else { m.clone() };
            let write_mat = if is_ref { tmpmatflag } else { self.o.matflag };
            if write_mat && let Some(d) = &dir {
                write_text(&format!("{d}/{name}"), &mat_text(&product));
            }
            if self.o.plotflag {
                let cog = Mat::col(&refvol.cog_scaled_mm(&mut warnings));
                let p = miscmaths::decompose_aff(&product, &cog);
                for k in 0..6 {
                    par.push_str(&cnum::stream(p[k]));
                    par.push_str("  ");
                }
                par.push('\n');
            }
        }
        for w in warnings {
            self.e(&w);
            self.e("\n");
        }
        if self.o.plotflag {
            write_text(&format!("{out}.par"), &par);
        }
        Ok(())
    }
}

fn rms_error() -> End {
    End::Exit(
        251,
        vec!["RMS_DEVIATION ERROR:: Could not invert matrix".into()],
    )
}

/// The text of a `.mat` file: each value fixed with 6 decimals and a blank, a newline per row.
pub fn mat_text(m: &Mat) -> String {
    let mut s = String::new();
    for r in 0..m.rows() {
        for c in 0..m.cols() {
            s.push_str(&cnum::fmt_f(m.at(r, c), 6));
            s.push(' ');
        }
        s.push('\n');
    }
    s
}

/// Writes `text` to `path` as an `ofstream` would: a file that cannot be opened is silently
/// not written.
fn write_text(path: &str, text: &str) {
    let _ = std::fs::write(path, text);
}

enum DirError {
    Access(String),
    Other(String),
}

/// `Log::establishDir(name)`: `mkdir(name, 0770)`, appending `+` while the name exists.
fn establish_dir(name: &str) -> Result<String, DirError> {
    let mut dir = name.to_owned();
    loop {
        match create_dir_0770(&dir) {
            Ok(()) => return Ok(dir),
            Err(e) => match e.kind() {
                std::io::ErrorKind::AlreadyExists => dir.push('+'),
                std::io::ErrorKind::PermissionDenied => return Err(DirError::Access(dir)),
                _ => return Err(DirError::Other(dir)),
            },
        }
    }
}

#[cfg(unix)]
fn create_dir_0770(dir: &str) -> std::io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    std::fs::DirBuilder::new().mode(0o770).create(dir)
}

#[cfg(not(unix))]
fn create_dir_0770(dir: &str) -> std::io::Result<()> {
    std::fs::create_dir(dir)
}

/// `Costfn(refv, inputv)`, with FSL-14's correction of the test volume's centre of mass when
/// `fix`.
fn new_costfn(refvol: Volume, testvol: Volume, warnings: &mut Vec<String>, fix: bool) -> Costfn {
    let mut cf = Costfn::new(refvol, testvol, warnings);
    if fix {
        cf.test_cog = cog_finite(&cf.testvol, warnings);
    }
    cf
}

/// FSL-14's correction: the centre of mass of the finite values (mcflirt includes NaN and
/// infinite values, which make it NaN). The same as FSL's when every value is finite.
pub fn cog_finite(v: &Volume, warnings: &mut Vec<String>) -> [f64; 3] {
    if v.data.iter().all(|x| x.is_finite()) {
        let mut w = Vec::new();
        let c = v.cog_scaled_mm(&mut w);
        let _ = w;
        return c;
    }
    let mut clean = v.clone();
    let finite: Vec<f32> = v.data.iter().copied().filter(|x| x.is_finite()).collect();
    let mn = finite.iter().copied().fold(f32::INFINITY, f32::min);
    for x in clean.data.iter_mut() {
        if !x.is_finite() {
            *x = mn;
        }
    }
    clean.cog_scaled_mm(warnings)
}

