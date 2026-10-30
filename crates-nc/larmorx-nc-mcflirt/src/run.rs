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
use larmorx_nc_core::sse;
use larmorx_nc_core::volume::{
    self, Background, Conversion, Extrapolation, FslError, Interpolation, NanOrder, SaveError,
    SaveOptions, SaveType, Volume,
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
    /// Where mcflirt crashes (FSL-66 reproduced: a stage's negative number of bins): what it
    /// prints from the crash on, how it ends (the replica's own line says it, then exits 1) and
    /// the notice naming the defect.
    Crash {
        text: String,
        how: String,
        notice: Notice,
    },
}

/// How mcflirt ends when a stage starts with `bins` histogram bins (`Costfn::set_no_bins`, which
/// every cost function calls): `None` while `bins >= 0`; with −1 the size of its table of
/// `p ln p` values divides by `bins + 1` (an integer division by zero: signal 8); with −2 or less
/// the first histogram it allocates has a negative length (`std::bad_array_new_length`,
/// signal 6). FSL-66.
fn bins_crash(bins: i32) -> Option<BinsCrash> {
    match bins {
        -1 => Some(BinsCrash::DivisionByZero),
        b if b <= -2 => Some(BinsCrash::NegativeLength),
        _ => None,
    }
}

/// How mcflirt crashes on a stage's negative number of bins (FSL-66).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BinsCrash {
    /// −1 bins: an integer division by zero, signal 8 (`SIGFPE`), no message of its own.
    DivisionByZero,
    /// −2 or fewer: `std::bad_array_new_length`, uncaught (abort, signal 6).
    NegativeLength,
}

impl BinsCrash {
    fn what(self) -> &'static str {
        match self {
            BinsCrash::DivisionByZero => {
                "mcflirt is killed by signal 8 (SIGFPE: an integer division by zero)"
            }
            BinsCrash::NegativeLength => "mcflirt aborts with std::bad_array_new_length (signal 6)",
        }
    }
}

/// The stage (1 to 4) where mcflirt crashes on `-bins` (FSL-66), and how, for a run of `stages`
/// stages: stage 1 has `bins / 8` bins, stages 2 to 4 `bins / 4` (truncated toward zero), each
/// prepared when the stage registers its first volume. `None` when no stage crashes (a cost
/// with no bins in a stage and FSL-21 aside).
fn bins_crash_stage(bins: i32, stages: i32) -> Option<(i32, i32, BinsCrash)> {
    (1..=stages.min(4)).find_map(|k| {
        let b = bins / if k == 1 { 8 } else { 4 };
        bins_crash(b).map(|c| (k, b, c))
    })
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
    /// The length of the volume's messages when a cost evaluation corrupted FSL's heap
    /// (FSL-21), where mcflirt's run ends.
    corrupted_at: std::cell::Cell<Option<usize>>,
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
        let c = cf.cost(&affmat);
        if cf.heap_corrupted && self.corrupted_at.get().is_none() {
            self.corrupted_at.set(Some(cerr.text.len()));
        }
        c
    }

    /// `usroptimise(matresult, dof, 1, new_tolerance)` → `optimise_strategy1`.
    fn usroptimise(
        &self,
        cf: &mut Costfn,
        matresult: &Mat,
        new_tolerance: f32,
        cerr: &mut Cerr,
    ) -> Mat {
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
                let results: Vec<Result<VolumeResult, End>> = with_pool(self.n_threads, || {
                    rest.par_iter()
                        .map(|&vi| {
                            register_volume(
                                &o,
                                &st,
                                mode,
                                &refvol,
                                timeseries,
                                vi,
                                scaling,
                                new_tolerance,
                                &mat_in[vi as usize],
                                fixed,
                            )
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
) -> Result<VolumeResult, End> {
    let mut st = st.clone();
    register_volume_mut(
        o,
        &mut st,
        mode,
        refvol,
        timeseries,
        i,
        scaling,
        new_tolerance,
        start,
        fixed,
    )
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
) -> Result<VolumeResult, End> {
    let mut cerr = Cerr::new(fixed);
    let mut testvol = timeseries.sub_volume(i64::from(i))?;
    if o.gdtflag {
        // -gdt (reached only when reproducing, FSL-04; named in `main`): the test volume becomes its gradient
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
    let bins = o.no_bins / scaling as i32;
    if let Some(crash) = bins_crash(bins) {
        // FSL-66, reproduced (the corrected run refuses a negative -bins first): the stage's
        // histogram tables cannot be prepared, and mcflirt crashes here.
        let mut text = cerr.text.clone();
        if crash == BinsCrash::NegativeLength {
            text.push_str(
                "terminate called after throwing an instance of 'std::bad_array_new_length'\n  \
                 what():  std::bad_array_new_length\n",
            );
        }
        return Err(End::Crash {
            text,
            how: crash.what().into(),
            notice: Notice::new(
                &defects::FSL_66,
                mode,
                format!(
                    "-bins {}: this stage has {bins} bins, where {}: an error (exit status 1), \
                     nothing written",
                    o.no_bins,
                    crash.what()
                ),
            ),
        });
    }
    cf.set_no_bins(bins);
    cf.smoothsize = st.smoothsize;
    cf.skip_nonfinite = mode.fix();
    let search = Search {
        o,
        st,
        corrupted_at: std::cell::Cell::new(None),
    };
    let res = search.usroptimise(&mut cf, &offsettrans, new_tolerance, &mut cerr);
    if let Some(len) = search.corrupted_at.get() {
        // FSL-21, reproduced (the corrected run refuses such -bins first): the correlation
        // ratio with no bins wrote before its arrays, and glibc aborts when they are released.
        return Err(FslError::library_abort(format!(
            "{}double free or corruption (out)",
            &cerr.text[..len]
        ))
        .into());
    }
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
        Err(End::Crash { text, how, notice }) => {
            run.e(&text);
            run.e(&format!(
                "{PREFIX}: {how} here; the replica exits with status 1\n"
            ));
            let mut notices = notices;
            defects::push_once(&mut notices, notice);
            print_notices(run.err, &notices);
            return 1;
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
        if self.mode.reproduce() && self.o.gdtflag {
            self.notice(
                &defects::FSL_04,
                "-gdt registers each volume's gradient image to the reference's plain image, as in \
                 mcflirt",
            );
        }
        if self.mode.fix() && self.o.gdtflag {
            self.notice(
                &defects::FSL_04,
                "-gdt is refused: mcflirt registers each volume's gradient image to the \
                 reference's plain image",
            );
            return Err(End::Refuse(
                "-gdt (registration of gradient images) is not supported: mcflirt computes the \
                 reference's gradient and discards it (FSL-04)"
                    .into(),
            ));
        }
        self.report("Reading time series... \n");
        let (mut timeseries, _, in_units) =
            volume::read_volume4d(&self.o.inputfname, self.settings)?;
        if self.mode.reproduce() {
            // FSL-22: the background value with libstdc++'s order of NaN values
            timeseries.nan_order = NanOrder::Libstdcxx;
        }
        let datatype = volume::file_dtype(&self.o.inputfname, self.settings)?;
        self.st.no_volumes = timeseries.tsize() as i32;
        let n = self.st.no_volumes;
        let original_refvol = self.o.refnum;
        if self.st.refnum == -1 {
            self.st.refnum = n / 2;
        }
        if self.mode.fix() {
            self.precheck(&timeseries)?;
        } else if self.o.no_stages >= 1 {
            self.reproduced_bins_notice();
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
                    let reference = anisorefvol.clone().expect("reference");
                    self.non_finite_notice(&timeseries, &reference);
                } else {
                    // The second pass: the mean of the first pass's corrected volumes.
                    let mut mv = timeseries.sub_volume(0)?;
                    mv.data.iter_mut().for_each(|v| *v = 0.0);
                    timeseries.extrap = Extrapolation::Extraslice;
                    timeseries.interp = Interpolation::Trilinear;
                    self.background_nan_notice(&timeseries)?;
                    for i in 0..n {
                        let src = timeseries.sub_volume(i64::from(i))?;
                        let mut testvol = src.clone();
                        affine_transform(&src, &mut testvol, &mat1[i as usize], 1.0)?;
                        for (m, t) in mv.data.iter_mut().zip(&testvol.data) {
                            *m = sse::add_f32(*m, *t);
                        }
                    }
                    let nf = n as f32;
                    for m in mv.data.iter_mut() {
                        *m = sse::div_f32(*m, nf);
                    }
                    self.mean_reg_notice();
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
                self.correct(
                    1,
                    &rv,
                    &timeseries,
                    current_scale,
                    new_tolerance,
                    &mut m0,
                    &mut m1,
                    mean_cond,
                )?;
                self.correct(
                    -1,
                    &rv,
                    &timeseries,
                    current_scale,
                    new_tolerance,
                    &mut m0,
                    &mut m1,
                    mean_cond,
                )?;
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
                self.correct(
                    1,
                    &rv,
                    &timeseries,
                    current_scale,
                    new_tolerance,
                    &mut m1,
                    &mut m2,
                    mean_cond,
                )?;
                self.correct(
                    -1,
                    &rv,
                    &timeseries,
                    current_scale,
                    new_tolerance,
                    &mut m1,
                    &mut m2,
                    mean_cond,
                )?;
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
                self.correct(
                    1,
                    &rv,
                    &timeseries,
                    current_scale,
                    new_tolerance,
                    &mut m2,
                    &mut m1,
                    mean_cond,
                )?;
                self.correct(
                    -1,
                    &rv,
                    &timeseries,
                    current_scale,
                    new_tolerance,
                    &mut m2,
                    &mut m1,
                    mean_cond,
                )?;
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
            self.correct(
                1,
                &rv,
                &timeseries,
                current_scale,
                0.1,
                &mut m1,
                &mut m0,
                mean_cond,
            )?;
            self.correct(
                -1,
                &rv,
                &timeseries,
                current_scale,
                0.1,
                &mut m1,
                &mut m0,
                mean_cond,
            )?;
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
                    self.e(&format!(
                        "Could not open matrix file {}\n",
                        self.o.init_transform
                    ));
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
        let requested = if self.o.sinc_final {
            Interpolation::Sinc
        } else if self.o.nn_final {
            Interpolation::NearestNeighbour
        } else if self.o.spline_final {
            Interpolation::Spline
        } else {
            Interpolation::Trilinear
        };
        timeseries.interp = requested;
        if self.o.sinc_final && timeseries.kernel.is_none() {
            let mut msgs = Vec::new();
            timeseries.kernel = Some(std::sync::Arc::new(kernel::sinc_kernel(
                "blackman", 7, 1201, &mut msgs,
            )));
        }
        // With a reference file the output grid is the reference's; mcflirt reads it in the
        // first stage only, so with `-stages 0` it is still the empty volume (`volume<float>
        // extrefvol;`) and the resampling throws (the corrected run refused this earlier).
        let template = if self.o.reffileflag {
            Some(extrefvol.clone().unwrap_or_else(empty_volume))
        } else {
            None
        };
        let mats: Vec<Mat> = mat0
            .iter()
            .map(|m| arma::mul_tinysq(m, &init_trans))
            .collect();
        self.background_nan_notice(&timeseries)?;
        let resample = |interp: Interpolation| -> Vec<Result<Volume, FslError>> {
            with_pool(self.n_threads, || {
                (0..n)
                    .into_par_iter()
                    .map(|i| {
                        let mut src = timeseries.sub_volume(i64::from(i))?;
                        src.interp = interp;
                        let mut testvol = template.clone().unwrap_or_else(|| src.clone());
                        affine_transform(&src, &mut testvol, &mats[i as usize], 1.0)?;
                        Ok(testvol)
                    })
                    .collect()
            })
        };
        let mut resampled = resample(requested);
        // FSL-59: FSL 6.0.7.23's mcflirt resamples each volume through the series' cached
        // ShadowVolume (newimage 2601.0), which keeps the interpolation method the volume had
        // when the registration first read it (trilinear): -sinc_final and -nn_final resample
        // with trilinear interpolation. Corrected by default (the method asked for, as FSL
        // 6.0.7.17 did); reproduced with `reproduce`; named where the two differ.
        if matches!(
            requested,
            Interpolation::Sinc | Interpolation::NearestNeighbour
        ) {
            let trilinear = resample(Interpolation::Trilinear);
            let differs = resampled.iter().zip(&trilinear).any(|(a, b)| match (a, b) {
                (Ok(a), Ok(b)) => a
                    .data
                    .iter()
                    .zip(&b.data)
                    .any(|(x, y)| x.to_bits() != y.to_bits()),
                _ => true,
            });
            let option = if self.o.sinc_final {
                "-sinc_final"
            } else {
                "-nn_final"
            };
            if differs {
                if self.mode.reproduce() {
                    self.notice(
                        &defects::FSL_59,
                        format!(
                            "{option}: the series resampled with trilinear interpolation, as FSL \
                             6.0.7.23's mcflirt does (newimage keeps each volume's first \
                             interpolation method)"
                        ),
                    );
                    resampled = trilinear;
                } else {
                    self.notice(
                        &defects::FSL_59,
                        format!(
                            "{option}: the series resampled with the interpolation asked for; \
                             FSL 6.0.7.23's mcflirt resamples with trilinear interpolation \
                             (newimage keeps each volume's first interpolation method)"
                        ),
                    );
                }
            }
        }
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

    /// FSL-14's notice: the volumes registered (and the reference) with NaN or infinite voxels.
    fn non_finite_notice(&mut self, ts: &Volume, reference: &Volume) {
        let per = ts.nvoxels() as usize;
        let skip = if self.o.reffileflag {
            -1
        } else {
            i64::from(self.st.refnum)
        };
        let vols: Vec<usize> = (0..ts.tsize() as usize)
            .filter(|&t| t as i64 != skip)
            .filter(|&t| {
                ts.data[t * per..(t + 1) * per]
                    .iter()
                    .any(|v| !v.is_finite())
            })
            .collect();
        let in_reference = reference.data.iter().any(|v| !v.is_finite());
        if vols.is_empty() && !in_reference {
            return;
        }
        let mut names: Vec<String> = vols.iter().take(5).map(usize::to_string).collect();
        if vols.len() > 5 {
            names.push(format!("... ({} volumes)", vols.len()));
        }
        let which = match (vols.len(), in_reference) {
            (0, _) => "the reference has".to_owned(),
            (1, false) => format!("volume {} has", names[0]),
            (_, false) => format!("volumes {} have", names.join(", ")),
            (1, true) => format!("volume {} and the reference have", names[0]),
            (_, true) => format!("volumes {} and the reference have", names.join(", ")),
        };
        let detail = if self.mode.fix() {
            format!(
                "{which} NaN or infinite voxels: they are left out of the centre of mass and the \
                 cost; mcflirt's matrices for such volumes are NaN"
            )
        } else {
            format!(
                "{which} NaN or infinite voxels: as in mcflirt, they make the centre of mass, and \
                 so the matrix, of such volumes NaN"
            )
        };
        self.notice(&defects::FSL_14, detail);
    }

    /// FSL-22: the volumes whose 2-voxel border holds a NaN voxel, where `calc_bval`'s
    /// `std::sort` has no defined order in C++ and the background value (the resampling's
    /// padding) is arbitrary in mcflirt. The correction sorts NaN values last (a defined order:
    /// every other value is FSL's); `reproduce` gives them libstdc++'s order, FSL's build's,
    /// except where libstdc++'s loops would leave the vector (sorted last there, and named).
    fn background_nan_notice(&mut self, timeseries: &Volume) -> Result<(), FslError> {
        if !timeseries.data.iter().any(|v| v.is_nan()) {
            return Ok(());
        }
        let (mut names, mut outside) = (Vec::new(), Vec::new());
        for i in 0..timeseries.tsize() {
            match timeseries.sub_volume(i)?.background_value().1 {
                Background::Defined => {}
                Background::Outside => {
                    names.push(i.to_string());
                    outside.push(i.to_string());
                }
                Background::NanLast | Background::Libstdcxx => names.push(i.to_string()),
            }
        }
        if names.is_empty() {
            return Ok(());
        }
        let which = |names: &[String], verb: [&str; 2]| {
            if names.len() == 1 {
                format!("volume {}{}", names[0], verb[0])
            } else {
                format!("volumes {}{}", names.join(", "), verb[1])
            }
        };
        let detail = if self.mode.fix() {
            format!(
                "{} NaN voxels in the 2-voxel border: they are sorted last for the background \
                 value (the resampling's padding); C++ leaves the order of mcflirt's sort undefined",
                which(&names, [" has", " have"])
            )
        } else if outside.is_empty() {
            format!(
                "{} NaN voxels in the 2-voxel border, whose order mcflirt's sort leaves \
                 undefined in C++: the background value (the resampling's padding) has them in \
                 libstdc++'s order, as FSL's build sorts them",
                which(&names, [" has", " have"])
            )
        } else {
            format!(
                "{} NaN voxels in the 2-voxel border, whose order mcflirt's sort leaves \
                 undefined in C++: the background value (the resampling's padding) has them in \
                 libstdc++'s order, except for {}: there libstdc++'s sort reads memory outside \
                 the vector of border values, so they are sorted last; everything else is \
                 mcflirt's",
                which(&names, [" has", " have"]),
                which(&outside, ["", ""])
            )
        };
        self.notice(&defects::FSL_22, detail);
        Ok(())
    }

    /// FSL-21 and FSL-66 reproduced, where mcflirt does not crash on them (the crashes of FSL-66
    /// name it where they end the run): a histogram cost with a stage of no bins (FSL-21: its
    /// cost is degenerate, or, with corratio, it corrupts the heap), and a negative `-bins` that
    /// gives every stage none with a cost that uses none (FSL-66).
    fn reproduced_bins_notice(&mut self) {
        let (bins, stages) = (self.o.no_bins, self.o.no_stages);
        let histogram = matches!(
            self.st.maincostfn,
            CostType::CorrRatio | CostType::MutualInfo | CostType::NormMI | CostType::Woods
        );
        let crash = bins_crash_stage(bins, stages).map(|(k, _, _)| k);
        let zero = (1..=stages.min(4)).find(|&k| bins / if k == 1 { 8 } else { 4 } == 0);
        if histogram && let Some(k) = zero.filter(|&k| crash.is_none_or(|c| k < c)) {
            let then = if self.st.maincostfn == CostType::CorrRatio {
                "the correlation ratio then writes before its arrays, and glibc aborts when they \
                 are released"
            } else {
                "its cost is degenerate and the matrices arbitrary"
            };
            self.notice(
                &defects::FSL_21,
                format!("-bins {bins}: stage {k} has no histogram bins, as in mcflirt; {then}"),
            );
        } else if !histogram && bins < 0 && crash.is_none() {
            // -3 to -1: every stage has no bins, which normcorr and leastsquares do not use.
            self.notice(
                &defects::FSL_66,
                format!(
                    "-bins {bins} (a negative number of bins) gives every stage none, as in \
                     mcflirt; this cost uses none"
                ),
            );
        }
    }

    /// Checks of the corrected command line before any work (FSL-12): the inputs mcflirt
    /// accepts and then aborts on.
    fn precheck(&mut self, timeseries: &Volume) -> Result<(), End> {
        let n = self.st.no_volumes;
        let histogram = matches!(
            self.st.maincostfn,
            CostType::CorrRatio | CostType::MutualInfo | CostType::NormMI | CostType::Woods
        );
        if histogram && self.o.no_stages >= 1 && self.o.no_bins / 8 == 0 {
            self.notice(
                &defects::FSL_21,
                format!(
                    "-bins {} is refused with this cost: the first stage's histogram would have \
                     no bins",
                    self.o.no_bins
                ),
            );
            return Err(End::Refuse(format!(
                "-bins {}: the histogram costs need at least 8 bins (the first stage uses -bins / \
                 8 of them; mcflirt computes them with none, FSL-21)",
                self.o.no_bins
            )));
        }
        if self.o.no_stages >= 1 && self.o.no_bins < 0 {
            let how = match bins_crash_stage(self.o.no_bins, self.o.no_stages) {
                Some((k, b, crash)) => format!("in stage {k} ({b} bins) {}", crash.what()),
                None => format!(
                    "mcflirt gives every stage {} bins, which this cost does not use",
                    self.o.no_bins / 4
                ),
            };
            self.notice(
                &defects::FSL_66,
                format!(
                    "-bins {} (a negative number of bins) is refused: {how}",
                    self.o.no_bins
                ),
            );
            return Err(End::Refuse(format!(
                "-bins {}: a negative number of bins (mcflirt prepares -bins / 8 and -bins / 4 \
                 bins for every cost and crashes on a negative count, FSL-66)",
                self.o.no_bins
            )));
        }
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
            let (r, _) = volume::read_volume::<f32>(&self.o.reffilename, self.settings)?;
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

    /// FSL-08's notice for the `-meanvol` image's name (`outputfname + "_mean_reg"`), when the
    /// correction changes it: only when `-out` ends in an image extension.
    fn mean_reg_notice(&mut self) {
        let stem = volume::make_basename(&self.o.outputfname);
        if stem == self.o.outputfname {
            return;
        }
        let detail = if self.mode.fix() {
            format!(
                "the -meanvol image is named after '{stem}'; mcflirt names it after '{}'",
                self.o.outputfname
            )
        } else {
            format!(
                "the -meanvol image is named after '{}', as in mcflirt",
                self.o.outputfname
            )
        };
        self.notice(&defects::FSL_08, detail);
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
        let opts = self.save_options(v, &settings, nifti::UNITS_MM_SEC);
        match v.save_typed(name, ty, Conversion::X86Cast, &settings, &opts, code) {
            Ok(()) => Ok(()),
            Err(SaveError::OutputType(e)) => Err(End::Exit(1, e.printed)),
            Err(SaveError::Exception(e)) => Err(End::Abort(e)),
        }
    }

    /// How an image is written: mcflirt's `descrip`, `units`, and NewNifti's data offset for
    /// several extensions (FSL-56) corrected unless reproduced, with its notice when the
    /// offset would differ.
    fn save_options(
        &mut self,
        v: &Volume,
        settings: &Settings,
        units: i32,
    ) -> SaveOptions<'static> {
        if volume::offset_defect_applies(v, settings) {
            self.notice(
                &defects::FSL_56,
                format!(
                    "{} NIfTI extensions of different sizes: the data offset counts each \
                     extension's own size (NewNifti adds the first one's once per extension)",
                    v.extensions.len()
                ),
            );
        }
        SaveOptions {
            description: DESCRIP,
            units,
            reproduce_offset: self.mode.reproduce(),
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
                (
                    SaveType::Int8,
                    Conversion::RoundClamp,
                    Some(nifti::dt::INT8),
                )
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
            let integer = matches!(ty, SaveType::Char | SaveType::Short | SaveType::Int);
            if datatype == nifti::dt::INT8 {
                self.notice(
                    &defects::FSL_09,
                    "int8 data are written as uint8, as in mcflirt",
                );
            } else if integer
                && ts
                    .data
                    .iter()
                    .any(|v| v.round_ties_even() != v.trunc() || !v.is_finite())
            {
                self.notice(
                    &defects::FSL_09,
                    "integer outputs are truncated toward zero, as in mcflirt",
                );
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
        let opts = self.save_options(ts, &settings, units);
        match ts.save_typed(&self.o.outputfname, ty, conv, &settings, &opts, code) {
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
        let (z0, z1) = if self.mode.fix() {
            (0, nz)
        } else {
            (1, nz - 1)
        };
        let nv = ts.nvoxels() as usize;
        for x in 0..nx {
            for y in 0..ny {
                for z in z0..z1 {
                    let k = meanvol.idx(x, y, z);
                    let m0 = meanvol.data[k];
                    let mut m = m0;
                    for i in 0..vmax as usize {
                        m += ts.data[i * nv + k];
                    }
                    meanvol.data[k] = m / vmax as f32;
                    if meanvol.data[k].is_nan() {
                        // NaN or infinite voxels: the same sum with SSE's NaNs, with the
                        // operands in the order of FSL's build (`addss (mean), ts`: the voxel's
                        // value is the destination, so its NaN passes on when both are NaN).
                        let mut m = m0;
                        for i in 0..vmax as usize {
                            m = sse::add_f32(ts.data[i * nv + k], m);
                        }
                        meanvol.data[k] = sse::div_f32(m, vmax as f32);
                    }
                }
            }
        }
        for x in 0..nx {
            for y in 0..ny {
                for z in z0..z1 {
                    let k = meanvol.idx(x, y, z);
                    let mean = meanvol.data[k];
                    let v0 = variancevol.data[k];
                    let mut v = v0;
                    for i in 0..vmax as usize {
                        let d = ts.data[i * nv + k] - mean;
                        v += d * d;
                    }
                    let mut var = v / (vmax - 1) as f32;
                    if var.is_nan() {
                        // NaN or infinite voxels: the same sums with SSE's NaNs, in the order
                        // of FSL's build (`subss (mean), ts`; `addss (variance), product`).
                        let mut v = v0;
                        for i in 0..vmax as usize {
                            let d = sse::sub_f32(ts.data[i * nv + k], mean);
                            v = sse::add_f32(sse::mul_f32(d, d), v);
                        }
                        var = sse::div_f32(v, (vmax - 1) as f32);
                    }
                    variancevol.data[k] = var;
                    sigmavol.data[k] = sse::sqrt_f32(var);
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
        // The -stats notice (it names the -meanvol image too) replaces a -meanvol one in place.
        let notice = Notice::new(&defects::FSL_08, self.mode, detail);
        match self
            .notices
            .iter_mut()
            .find(|n| n.defect.id == defects::FSL_08.id)
        {
            Some(n) => *n = notice,
            None => self.notices.push(notice),
        }
        self.report("Saving mean volume... \n");
        self.save(
            &meanvol,
            &self.derived_name("_meanvol"),
            SaveType::Float,
            None,
        )?;
        self.report("Saving variance volume... \n");
        self.save(
            &variancevol,
            &self.derived_name("_variance"),
            SaveType::Float,
            None,
        )?;
        self.report("Saving standard deviation volume... \n");
        self.save(
            &sigmavol,
            &self.derived_name("_sigma"),
            SaveType::Float,
            None,
        )?;
        Ok(())
    }

    /// `decompose_mats(mat_index, mat_array, refvol)`: the `.mat` directory, the `.par` and
    /// `.rms` files, in mcflirt's order: the `.par` file opened (`-plots`), the `.mat` directory
    /// made, `_rel.rms` and `_rel_mean.rms`, `_abs.rms` and `_abs_mean.rms`, then volume by
    /// volume its `MAT_` file and its `.par` line (`specs/mcflirt.md` §10.6).
    ///
    /// FSL-65: a text file that cannot be opened. Reproduced, an `.rms` file is not written and
    /// an unopenable `.par` ends the run at volume 0's line with mcflirt's message and status 0
    /// (after `MAT_0000` and the `.rms` files). Corrected (the default), every text output is
    /// opened and the `.mat` directory made before anything is written; one that cannot be is
    /// an error (exit status 1) naming the file, and what this step made is removed.
    fn decompose_mats(&mut self, mats: &[Mat], refvol: &Volume) -> Result<(), End> {
        let n = self.st.no_volumes as usize;
        let rmax: f32 = 80.0;
        let out = self.o.outputfname.clone();
        let mut center = Mat::zeros(3, 1);
        center.put(
            0,
            0.5 * (refvol.xsize() as f64 - 1.0) * f64::from(refvol.xdim()),
        );
        center.put(
            1,
            0.5 * (refvol.ysize() as f64 - 1.0) * f64::from(refvol.ydim()),
        );
        center.put(
            2,
            0.5 * (refvol.zsize() as f64 - 1.0) * f64::from(refvol.zdim()),
        );
        let mut tmpmatflag = self.o.tmpmatflag;
        let par_name = format!("{out}.par");
        let rms_names: Vec<String> = [
            (self.o.rmsrelflag, ["_rel.rms", "_rel_mean.rms"]),
            (self.o.rmsabsflag, ["_abs.rms", "_abs_mean.rms"]),
        ]
        .into_iter()
        .filter(|(on, _)| *on)
        .flat_map(|(_, names)| names.map(|x| format!("{out}{x}")))
        .collect();
        // Reproduced: `outfile.open(filename)` first (an empty file at once, where it can be
        // made).
        let mut texts = TextOutputs::default();
        if self.mode.reproduce() && self.o.plotflag {
            texts.open_reproduced(&par_name);
        }
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
                        self.notice(
                            &defects::FSL_20,
                            format!(
                                "the directory {d} cannot be created: an error; mcflirt exits \
                                 with status 0 and writes nothing more"
                            ),
                        );
                        return Err(End::Refuse(format!(
                            "cannot create the directory {d} for the matrices (FSL-20: mcflirt \
                             prints \"Error!: Unknown Log directory return value\" and exits \
                             with status 0)"
                        )));
                    }
                    self.notice(
                        &defects::FSL_20,
                        format!(
                            "the directory {d} cannot be created: mcflirt's message and exit \
                             status 0, nothing more written"
                        ),
                    );
                    return Err(End::Exit(
                        0,
                        vec!["Error!: Unknown Log directory return value".into()],
                    ));
                }
            }
        }
        if self.mode.fix() && !make_dir && (self.o.rmsrelflag || self.o.rmsabsflag) {
            // FSL-13 corrected makes no .mat directory for -rmsrel and -rmsabs alone; where
            // mcflirt's would fail for another reason than its permission (its parent missing or
            // not a directory), FSL-20 ends mcflirt's run here: the same error as with -mats.
            let d = format!("{out}.mat");
            let parent = std::path::Path::new(&d)
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(std::path::Path::new("."));
            if !parent.is_dir() {
                self.notice(
                    &defects::FSL_20,
                    format!(
                        "the directory {d} cannot be created: an error; mcflirt exits with \
                         status 0 and writes nothing more"
                    ),
                );
                return Err(End::Refuse(format!(
                    "cannot create the directory {d} for the matrices (FSL-20: mcflirt prints \
                     \"Error!: Unknown Log directory return value\" and exits with status 0)"
                )));
            }
        }
        if self.mode.fix() {
            // FSL-65 corrected: every text output opened before anything is written.
            let wanted = self
                .o
                .plotflag
                .then(|| par_name.clone())
                .into_iter()
                .chain(rms_names.iter().cloned());
            for name in wanted {
                if let Err(e) = texts.open_corrected(&name) {
                    texts.remove_made();
                    if let Some(d) = &dir {
                        let _ = std::fs::remove_dir(d);
                    }
                    let mcflirt = if name == par_name {
                        "mcflirt prints \"error: unable to open output file!\" at the first \
                         volume and exits with status 0, after the first matrix and the .rms files"
                    } else {
                        "mcflirt silently does not write it and exits with status 0"
                    };
                    self.notice(
                        &defects::FSL_65,
                        format!(
                            "{name} cannot be opened ({e}): an error (exit status 1); {mcflirt}"
                        ),
                    );
                    return Err(End::Refuse(format!(
                        "cannot open {name} for writing: {e} (FSL-65: {mcflirt})"
                    )));
                }
            }
        }
        if self.o.rmsrelflag {
            let mut mean: f32 = 0.0;
            let file = texts.take(self.mode, &format!("{out}_rel.rms"));
            let mut w = TextWriter::new(file);
            for i in 1..n {
                let r = miscmaths::rms_deviation(&mats[i - 1], &mats[i], &center, rmax)
                    .map_err(|_| rms_error())?;
                w.line(&cnum::stream(f64::from(r)));
                mean += r;
            }
            self.unwritten(w, &format!("{out}_rel.rms"));
            let m = mean / (self.st.no_volumes - 1) as f32;
            let file = texts.take(self.mode, &format!("{out}_rel_mean.rms"));
            let mut w = TextWriter::new(file);
            w.line(&cnum::stream(f64::from(m)));
            self.unwritten(w, &format!("{out}_rel_mean.rms"));
        }
        if self.o.rmsabsflag {
            let mut mean: f32 = 0.0;
            let file = texts.take(self.mode, &format!("{out}_abs.rms"));
            let mut w = TextWriter::new(file);
            for m in mats.iter().take(n) {
                let r = miscmaths::rms_deviation(&Mat::eye(4), m, &center, rmax)
                    .map_err(|_| rms_error())?;
                w.line(&cnum::stream(f64::from(r)));
                mean += r;
            }
            self.unwritten(w, &format!("{out}_abs.rms"));
            let m = mean / self.st.no_volumes as f32;
            let file = texts.take(self.mode, &format!("{out}_abs_mean.rms"));
            let mut w = TextWriter::new(file);
            w.line(&cnum::stream(f64::from(m)));
            self.unwritten(w, &format!("{out}_abs_mean.rms"));
        }
        let mut par = self.o.plotflag.then(|| {
            if self.mode.fix() {
                TextWriter::new(texts.take(self.mode, &par_name))
            } else {
                TextWriter::new(texts.par.take())
            }
        });
        let mut warnings = Vec::new();
        for (i, m) in mats.iter().enumerate().take(n) {
            let name = format!("MAT_{i:04}");
            let is_ref = i as i32 == self.st.refnum;
            let product = if is_ref { Mat::eye(4) } else { m.clone() };
            let write_mat = if is_ref { tmpmatflag } else { self.o.matflag };
            if write_mat && let Some(d) = &dir {
                write_text(&format!("{d}/{name}"), &mat_text(&product));
            }
            if let Some(w) = par.as_mut() {
                // The parameters' centre: the reference's centre of mass (of its finite values
                // when FSL-14 is corrected).
                let cog = if self.mode.fix() {
                    Mat::col(&cog_finite(refvol, &mut warnings))
                } else {
                    Mat::col(&refvol.cog_scaled_mm(&mut warnings))
                };
                let p = miscmaths::decompose_aff(&product, &cog);
                if !w.is_open() {
                    // FSL-65 reproduced: `if (!outfile) { cerr << ...; exit(0); }` at the first
                    // line.
                    for w in warnings {
                        self.e(&w);
                        self.e("\n");
                    }
                    self.notice(
                        &defects::FSL_65,
                        format!(
                            "{par_name} cannot be opened: mcflirt's message at volume 0's line \
                             and exit status 0, after MAT_0000 and the .rms files, nothing more \
                             written"
                        ),
                    );
                    return Err(End::Exit(
                        0,
                        vec!["error: unable to open output file!".into()],
                    ));
                }
                let mut line = String::new();
                for k in 0..6 {
                    line.push_str(&cnum::stream(p[k]));
                    line.push_str("  ");
                }
                w.line(&line);
            }
        }
        for w in warnings {
            self.e(&w);
            self.e("\n");
        }
        Ok(())
    }

    /// FSL-65 reproduced: the notice of an `.rms` file that could not be opened (mcflirt writes
    /// nothing into it, silently).
    fn unwritten(&mut self, w: TextWriter, name: &str) {
        if !w.is_open() {
            self.notice(
                &defects::FSL_65,
                format!("{name} cannot be opened: not written, as in mcflirt (status 0)"),
            );
        }
    }
}

/// The text outputs of `decompose_mats` (FSL-65): those opened before anything is written by
/// default, and the `.par` file mcflirt opens first.
#[derive(Default)]
struct TextOutputs {
    /// Opened by default before any is written: the name, the file, and whether this run made
    /// it (removed again if a later one cannot be opened).
    opened: Vec<(String, std::fs::File, bool)>,
    /// Reproduced: the `.par` file as `ofstream::open` left it (`None`: it could not be opened).
    par: Option<std::fs::File>,
}

impl TextOutputs {
    /// Opens `name` for writing without truncating it (an existing file keeps its contents
    /// until it is written).
    fn open_corrected(&mut self, name: &str) -> std::io::Result<()> {
        let existed = std::fs::symlink_metadata(name).is_ok();
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(name)?;
        self.opened.push((name.to_owned(), file, !existed));
        Ok(())
    }

    /// `ofstream::open(name)`: created or truncated at once; `None` where it cannot be.
    fn open_reproduced(&mut self, name: &str) {
        self.par = std::fs::File::create(name).ok();
    }

    /// Removes the files this run made (a later output could not be opened).
    fn remove_made(&mut self) {
        for (name, _, made) in self.opened.drain(..) {
            if made {
                let _ = std::fs::remove_file(name);
            }
        }
    }

    /// The file to write `name` into: by default the one opened beforehand, emptied;
    /// reproduced, `ofstream::open(name)` now (`None` where it cannot be opened).
    fn take(&mut self, mode: UpstreamDefects, name: &str) -> Option<std::fs::File> {
        if mode.reproduce() {
            return std::fs::File::create(name).ok();
        }
        let k = self.opened.iter().position(|(n, _, _)| n == name)?;
        let (_, file, _) = self.opened.remove(k);
        file.set_len(0).ok().map(|()| file)
    }
}

/// An `ofstream` written line by line, each line flushed (`<< endl`): a file that could not be
/// opened takes nothing.
struct TextWriter {
    file: Option<std::fs::File>,
}

impl TextWriter {
    fn new(file: Option<std::fs::File>) -> TextWriter {
        TextWriter { file }
    }

    fn is_open(&self) -> bool {
        self.file.is_some()
    }

    fn line(&mut self, text: &str) {
        if let Some(f) = self.file.as_mut() {
            let _ = writeln!(f, "{text}");
        }
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
