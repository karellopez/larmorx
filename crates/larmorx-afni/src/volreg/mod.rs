// SPDX-License-Identifier: Apache-2.0
//! `3dvolreg`: rigid-body registration of every volume of a series to a base volume, the
//! clean-room original (`specs/3dvolreg.md`), written from the spec, AFNI's documentation and
//! black-box runs of AFNI 26.3.05's program.
//!
//! [`run`] is the program of §8 on a series held as AFNI holds it ([`Bricks`]) on its grid
//! ([`grid::Grid`]): the base, the `-maxdisp` mask, the first pass of `-twopass` with its coarse
//! searches, the final pass, the parameters, matrices and displacements, and the registered
//! series. The command line (`crate::cli::volreg`) reads and writes the files and prints the
//! messages [`run`] returns, in AFNI's order.
//!
//! Built for speed (`docs/porting-guide.md`, "Clean-room originals"): the volumes of each pass
//! are registered one after another, each one's work parallel inside (the shears by planes,
//! the coarse search's rotations and scores, the blurs), so the memory is one volume's work
//! whatever the thread count and every result is computed by the same sequence of operations
//! whatever the thread count; the series is registered in place (each output volume replaces
//! its input volume) and the basis is built once per pass.

pub mod blur;
pub mod coarse;
pub mod fit;
pub mod fourier;
pub mod grid;
pub mod notices;
pub mod report;
pub mod rotate;
pub mod shift;
pub mod sweep;

use larmorx_core::defects::{self, Notice, UpstreamDefects};

use crate::automask::{self, MaskSettings};
use crate::cfmt;
use crate::dataset::{BrickData, Bricks};
use crate::msg::Message;

use blur::BlurSettings;
use coarse::{CoarseSettings, Found};
use fit::{Basis, BasisError, BasisSettings, Fitted, Iteration, SetupLog, Weight};
use grid::Grid;
use report::MaxDisp;
use rotate::{Resampler, Trig, Work};

pub use shift::Method;

/// The base of the registration.
#[derive(Clone, Debug)]
pub enum Base {
    /// A volume of the series (`-base n`).
    Index(usize),
    /// Volume 0 of another dataset (`-base dset`): its volume as float32 and its grid.
    External { volume: Vec<f32>, grid: Grid },
}

/// `-edging`: a percentage of each axis or a number of voxels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Edging {
    Percent(f64),
    Voxels(f64),
}

impl Edging {
    /// The edge widths (§8.3) for a grid of `n` voxels.
    pub fn widths(self, n: [usize; 3]) -> [usize; 3] {
        n.map(|d| match self {
            Edging::Percent(p) => (0.01 * p * d as f64 + 0.5).trunc() as usize,
            Edging::Voxels(e) => (0.25 * d as f64).min(e).trunc().max(0.0) as usize,
        })
    }
}

/// The parameters of a run (§3), as the command line sets them.
#[derive(Clone, Debug)]
pub struct VolregParams {
    pub base: Base,
    pub method: Method,
    pub final_method: Method,
    pub clip: bool,
    pub zpad: usize,
    pub max_iter: i32,
    pub x_thresh: f32,
    pub rot_thresh: f32,
    pub delta: f32,
    /// `-weight`: a volume (float32) of the input's grid.
    pub weight: Option<Vec<f32>>,
    pub wtinp: bool,
    pub edging: Edging,
    pub twopass: bool,
    pub twoblur: f32,
    pub coarse: (i32, i32),
    pub coarse_rotation: bool,
    pub sinit: bool,
    pub maxdisp: bool,
    /// The clip fraction of the coarse search's automask (0.333, or 0.5 with `-nomaxdisp` in
    /// `reproduce`: AFNI-92).
    pub coarse_clfrac: f32,
    /// Compute the registered series (not with `-prefix NULL`).
    pub output: bool,
    /// The registered series as float32 (`-float`, or an input with brick factors).
    pub float_output: bool,
    /// `-savedisp`'s displacements.
    pub savedisp: bool,
    /// `-verbose` (0, 1, 2).
    pub verbose: u8,
    pub firfac: Option<f64>,
    pub firold: bool,
    pub n_threads: usize,
    pub mode: UpstreamDefects,
}

impl Default for VolregParams {
    fn default() -> VolregParams {
        VolregParams {
            base: Base::Index(0),
            method: Method::Heptic,
            final_method: Method::Heptic,
            clip: true,
            zpad: 4,
            max_iter: 23,
            x_thresh: 0.01,
            rot_thresh: 0.02,
            delta: 0.7,
            weight: None,
            wtinp: false,
            edging: Edging::Percent(5.0),
            twopass: false,
            twoblur: 2.0,
            coarse: (10, 2),
            coarse_rotation: true,
            sinit: true,
            maxdisp: true,
            coarse_clfrac: 0.333,
            output: true,
            float_output: false,
            savedisp: false,
            verbose: 0,
            firfac: None,
            firold: false,
            n_threads: 0,
            mode: UpstreamDefects::Fix,
        }
    }
}

/// The outcome of a run.
#[derive(Clone, Debug, Default)]
pub struct VolregOutput {
    /// The registered series (`None` with `-prefix NULL`).
    pub registered: Option<Bricks>,
    /// The internal parameters of each volume.
    pub fitted: Vec<Fitted>,
    /// `roll, pitch, yaw, dS, dL, dP` of each volume.
    pub reported: Vec<[f32; 6]>,
    /// `-1Dmatrix_save`'s 12 numbers of each volume.
    pub matrices: Vec<[f64; 12]>,
    /// `rmsold`, `rmsnew` of each volume.
    pub rms: Vec<[f32; 2]>,
    /// The maximum displacements (`None` without `-maxdisp` or without a mask).
    pub maxdisp: Option<MaxDisp>,
    /// `-savedisp`'s three series (dataset axes 0, 1, 2), float32.
    pub displacements: Option<[Vec<f32>; 3]>,
    /// AFNI's messages of the run, in its order.
    pub messages: Vec<Message>,
    /// The messages of the maximum displacements (printed after `-1Dmatrix_save` is opened).
    pub maxdisp_messages: Vec<Message>,
    /// The upstream defects that changed (or would have changed) the result.
    pub notices: Vec<Notice>,
}

/// A run that failed: the messages printed before, the notices, and the error (AFNI's plain
/// `** ` line, or a `** FATAL ERROR: ` line when `fatal_error`).
#[derive(Clone, Debug)]
pub struct VolregError {
    pub messages: Vec<Message>,
    pub notices: Vec<Notice>,
    pub fatal: String,
    pub fatal_error: bool,
}

/// The values of volume `t` of `bricks` as float32: `factor·x` when its factor is neither 0
/// nor 1 (§8.2).
pub fn volume_f32(bricks: &Bricks, t: usize) -> Vec<f32> {
    let [nx, ny, nz, _] = bricks.shape;
    let n = nx * ny * nz;
    let f = bricks.factors[t];
    let scale = f != 0.0 && f != 1.0;
    let conv = |x: f32| if scale { f * x } else { x };
    match &bricks.data {
        BrickData::Byte(v) => v[t * n..(t + 1) * n]
            .iter()
            .map(|&x| conv(f32::from(x)))
            .collect(),
        BrickData::Short(v) => v[t * n..(t + 1) * n]
            .iter()
            .map(|&x| conv(f32::from(x)))
            .collect(),
        BrickData::Float(v) => v[t * n..(t + 1) * n].iter().map(|&x| conv(x)).collect(),
    }
}

fn nonzero(v: &[f32]) -> usize {
    v.iter().filter(|&&x| x != 0.0).count()
}

/// The `-maxdisp` mask's edge voxels (§8.4): their positions `o + i·Δ` (float32, dataset
/// order), in storage order, with `-verbose`'s lines.
fn maxdisp_positions(
    bricks: &Bricks,
    grid: &Grid,
    mode: UpstreamDefects,
    verbose: bool,
    messages: &mut Vec<Message>,
) -> Vec<[f32; 3]> {
    if verbose {
        messages.push(Message::Info("Creating mask for -maxdisp".into()));
    }
    let [nx, ny, nz, _] = bricks.shape;
    let dims = [nx, ny, nz];
    let (avg, _) = automask::average_abs(bricks, mode);
    let settings = MaskSettings {
        clfrac: 0.333,
        ..MaskSettings::default()
    };
    let mut log = Vec::new();
    let Ok(mask) = automask::form_mask(&avg, dims, &settings, &mut log) else {
        messages.push(Message::Warning("Can't create -maxdisp mask?!".into()));
        return Vec::new();
    };
    if verbose {
        messages.push(Message::More(format!(
            "Automask has {} voxels",
            mask.iter().filter(|&&m| m != 0).count()
        )));
    }
    let mut positions = Vec::new();
    for k in 0..nz {
        for j in 0..ny {
            for i in 0..nx {
                let p = i + nx * (j + ny * k);
                if mask[p] == 0 {
                    continue;
                }
                let face = i == 0 || j == 0 || k == 0 || i + 1 == nx || j + 1 == ny || k + 1 == nz;
                let outside = face
                    || mask[p - 1] == 0
                    || mask[p + 1] == 0
                    || mask[p - nx] == 0
                    || mask[p + nx] == 0
                    || mask[p - nx * ny] == 0
                    || mask[p + nx * ny] == 0;
                if outside {
                    let at = [i, j, k];
                    positions.push(std::array::from_fn(|a| {
                        grid.origin[a] + at[a] as f32 * grid.delta[a]
                    }));
                }
            }
        }
    }
    if verbose {
        messages.push(Message::More(format!(
            "{} voxels left in -maxdisp mask after erosion",
            positions.len()
        )));
    }
    positions
}

/// The blur of the two-pass scheme (σ = `twoblur` voxels; FIR up to 14, else FFT).
fn twopass_blur(v: &mut [f32], n: [usize; 3], twoblur: f32, s: &BlurSettings) {
    if blur::half_width(twoblur, 1.0, s.main_factor()) > 35 {
        blur::fft_blur(v, n, twoblur, 1.0, s.trig);
    } else {
        blur::fir(v, n, [twoblur; 3], [1.0; 3], s.main_factor(), false, s);
    }
}

/// The run's one `edt_blur` line, printed by its first blur.
struct BlurOnce(bool);

impl BlurOnce {
    fn emit(&mut self, line: Option<String>, messages: &mut Vec<Message>) {
        if let Some(l) = line
            && !self.0
        {
            self.0 = true;
            messages.push(Message::Info(l));
        }
    }
}

/// Pushes a basis' setup lines, with the run's `edt_blur` line where it falls. AFNI's
/// `** ERROR: ` lines are errors (labelled, coloured with `reproduce` as AFNI colours them).
fn setup_lines(log: SetupLog, once: &mut BlurOnce, messages: &mut Vec<Message>) {
    let at = log.blur_at.min(log.lines.len());
    let (before, after) = log.lines.split_at(at);
    let push = |messages: &mut Vec<Message>, l: &String| {
        messages.push(match l.strip_prefix("** ERROR: ") {
            Some(rest) => Message::Error(rest.to_owned()),
            None => Message::Text(l.clone()),
        });
    };
    for l in before {
        push(messages, l);
    }
    once.emit(log.blur_line, messages);
    for l in after {
        push(messages, l);
    }
}

/// `-verbose`'s line of a coarse search.
fn coarse_line(k: usize, found: &Found, rotations: bool) -> String {
    let s = found.shift;
    if rotations {
        let a = found.angles;
        format!(
            "Getting best coarse rot+shift [{k}]:{} {} {} {} : {} {} {}",
            found.marks,
            cfmt::g32(a[0]),
            cfmt::g32(a[1]),
            cfmt::g32(a[2]),
            s[0],
            s[1],
            s[2]
        )
    } else {
        format!(
            "Getting best coarse shift [{k}]: {} {} {}",
            s[0], s[1], s[2]
        )
    }
}

/// Runs 3dvolreg on `bricks` (the input series, after `-tshift`) on `grid`.
pub fn run(
    bricks: Bricks,
    grid: &Grid,
    params: &VolregParams,
) -> Result<VolregOutput, VolregError> {
    larmorx_core::parallel::with_threads(params.n_threads, || run_in_pool(bricks, grid, params))
        .unwrap_or_else(|e| {
            Err(VolregError {
                messages: Vec::new(),
                notices: Vec::new(),
                fatal: format!("larmorx: can't start {} threads: {e}", params.n_threads),
                fatal_error: true,
            })
        })
}

fn run_in_pool(
    mut bricks: Bricks,
    grid: &Grid,
    p: &VolregParams,
) -> Result<VolregOutput, VolregError> {
    let mode = p.mode;
    let trig = Trig::of(mode);
    let [nx, ny, nz, nt] = bricks.shape;
    let n = [nx, ny, nz];
    let total = nx * ny * nz;
    let sizes = grid.sizes();
    let (axes, signs) = grid.angle_axes();
    let mut messages: Vec<Message> = Vec::new();
    let mut notices: Vec<Notice> = Vec::new();
    let fail = |messages: Vec<Message>, fatal: String| VolregError {
        messages,
        notices: Vec::new(),
        fatal,
        fatal_error: false,
    };
    // AFNI-89: a basis whose least squares failed; the error comes at the first fit.
    let afni_89 = |messages: Vec<Message>| VolregError {
        messages,
        notices: vec![notices::afni_89(mode)],
        fatal: notices::AFNI_89_FATAL.into(),
        fatal_error: true,
    };
    let blur = BlurSettings {
        firfac: p.firfac,
        old: p.firold,
        trig,
    };
    let verbose = p.verbose >= 1;
    let chatty = p.verbose >= 2;

    // §8.2: the base.
    let (base, base_index, base_grid) = match &p.base {
        Base::Index(i) => (volume_f32(&bricks, *i), Some(*i), grid.clone()),
        Base::External { volume, grid: g } => (volume.clone(), None, g.clone()),
    };
    let k = nonzero(&base);
    if k < 100 {
        let s = if k == 1 { "" } else { "s" };
        return Err(fail(
            messages,
            format!("3dvolreg fails: base image has {k} nonzero voxel{s} (< 100)"),
        ));
    }
    // §8.3: edging.
    let edging = p.edging.widths(n);
    if verbose {
        messages.push(Message::Info(format!(
            "Edging: x={} y={} z={}",
            edging[0], edging[1], edging[2]
        )));
    }
    // §8.4: the -maxdisp mask.
    let positions = if p.maxdisp {
        maxdisp_positions(&bricks, grid, mode, verbose, &mut messages)
    } else {
        Vec::new()
    };
    let mut once = BlurOnce(false);
    let iteration = Iteration {
        x_thresh: p.x_thresh,
        rot_thresh: p.rot_thresh,
        max_iter: p.max_iter,
    };
    let mut sinit = 1.0f32;
    let empty_ish = |v: &[f32]| nonzero(v) < 66;
    let skipped = |t: usize| base_index == Some(t);

    // §8.5: the first pass.
    let mut first: Vec<Option<[f32; 6]>> = vec![None; nt];
    let coarse_on = p.coarse.0 > 0 && p.coarse.1 > 0;
    // AFNI-86: -nocoarserot read as -noclip leaves the rotations on (the command line passes
    // AFNI's reading in `coarse_rotation` with reproduce).
    let coarse_settings = CoarseSettings {
        del: p.coarse.0,
        num: p.coarse.1,
        rotations: p.coarse_rotation,
        clfrac: p.coarse_clfrac,
        zpad: p.zpad,
        trig,
        axes,
        signs,
        blur,
        take_empty: mode.reproduce(),
    };
    let mut empty_region = false;
    if p.twopass {
        if verbose {
            messages.push(Message::Info(
                "Start of first pass alignment on all sub-bricks".into(),
            ));
        }
        let mut b1 = base.clone();
        once.emit(
            Some(fit::blur_line(
                p.twoblur,
                1.0,
                blur.main_factor(),
                blur::half_width(p.twoblur, 1.0, blur.main_factor()),
            )),
            &mut messages,
        );
        twopass_blur(&mut b1, n, p.twoblur, &blur);
        let searcher = coarse_on.then(|| coarse::Searcher::new(&b1, n, sizes, &coarse_settings));
        let search = |v: &[f32]| -> Found {
            match &searcher {
                Some(s) => s.search(v),
                None => Found::default(),
            }
        };
        let initial = |f: &Found| -> Option<[f32; 6]> {
            let a = f.angles;
            let s = f.shift;
            Some([
                a[0] * signs[0],
                a[1] * signs[1],
                a[2] * signs[2],
                s[0] as f32 * sizes[0],
                s[1] as f32 * sizes[1],
                s[2] as f32 * sizes[2],
            ])
        };
        let mut found0: Option<Found> = None;
        let weight = match &p.weight {
            Some(w) => Weight::Given {
                w: w.clone(),
                edged: false,
            },
            None => {
                if verbose {
                    messages.push(Message::Info(
                        "Computing first pass weight as sum of base and brick".into(),
                    ));
                }
                let mut v0 = volume_f32(&bricks, 0);
                twopass_blur(&mut v0, n, p.twoblur, &blur);
                let f = search(&v0);
                if verbose && coarse_on {
                    messages.push(Message::Info(coarse_line(0, &f, coarse_settings.rotations)));
                }
                empty_region |= f.empty_region;
                let s = f.shift;
                let inside = |i: usize, a: usize| {
                    let d = s[a].unsigned_abs() as usize;
                    i >= d && i + d < n[a]
                };
                let src = |i: usize, j: usize, k: usize| {
                    let (a, b, c) = (
                        (i as i64 - i64::from(s[0])) as usize,
                        (j as i64 - i64::from(s[1])) as usize,
                        (k as i64 - i64::from(s[2])) as usize,
                    );
                    v0[a + nx * (b + ny * c)]
                };
                if p.sinit {
                    let (mut fs, mut gs) = (0f32, 0f32);
                    for k in 0..nz {
                        for j in 0..ny {
                            for i in 0..nx {
                                if inside(i, 0) && inside(j, 1) && inside(k, 2) {
                                    fs += src(i, j, k);
                                    gs += b1[i + nx * (j + ny * k)];
                                }
                            }
                        }
                    }
                    if gs > 0.0 {
                        let sc = fs / gs;
                        if sc > 0.005 && sc < 2000.0 {
                            sinit = sc;
                            if sc < 200.0 {
                                if verbose {
                                    messages.push(Message::Info(format!(
                                        "Scale init = {}",
                                        cfmt::g32(sc)
                                    )));
                                }
                            } else {
                                messages.push(Message::Info(format!(
                                    "Warning: Scale init = {} is large. Check output.",
                                    cfmt::g32(sc)
                                )));
                            }
                        } else {
                            messages.push(Message::Text(format!(
                                "-- Large scale difference between datasets.\n   Scale init = {}\n   3dvolreg might not converge.",
                                cfmt::g32(sc)
                            )));
                        }
                    }
                }
                let mut w1 = b1.clone();
                for k in 0..nz {
                    for j in 0..ny {
                        for i in 0..nx {
                            if inside(i, 0) && inside(j, 1) && inside(k, 2) {
                                w1[i + nx * (j + ny * k)] += src(i, j, k);
                            }
                        }
                    }
                }
                if verbose {
                    messages.push(Message::Info("Blurring first pass weight".into()));
                }
                twopass_blur(&mut w1, n, p.twoblur, &blur);
                fit::clip_small(&mut w1, 0.025);
                found0 = Some(f);
                Weight::Given { w: w1, edged: true }
            }
        };
        let settings = BasisSettings {
            resampler: Resampler {
                method: Method::Linear,
                zpad: p.zpad,
                trig,
            },
            edging,
            dfac: p.twoblur * p.delta,
            sinit,
            axes,
            blur,
            chatty,
        };
        let mut log = SetupLog::default();
        let basis = Basis::new(&b1, n, sizes, weight, settings, &mut log);
        setup_lines(log, &mut once, &mut messages);
        let basis = match basis {
            Ok(b) => Some(b),
            Err(BasisError::NotThreeD) => {
                messages.push(Message::Raw(
                    "\n*** mri_3dalign_setup: cannot use nD images!\x07".into(),
                ));
                return Err(fail(
                    messages,
                    "Can't initialize first pass alignment algorithm".into(),
                ));
            }
            Err(BasisError::LeastSquares) => None,
        };
        let it1 = Iteration {
            x_thresh: p.twoblur * p.x_thresh,
            rot_thresh: p.twoblur * p.rot_thresh,
            max_iter: p.max_iter,
        };
        // One volume at a time, each one's resamplings and searches parallel inside: the
        // memory is one volume's work whatever the thread count.
        let mut work = Work::default();
        type FirstPass = (Option<[f32; 6]>, Option<String>, bool, Option<String>);
        let mut results: Vec<FirstPass> = Vec::with_capacity(nt);
        for t in 0..nt {
            if skipped(t) {
                results.push((Some([0.0; 6]), None, false, None));
                continue;
            }
            let mut v = volume_f32(&bricks, t);
            if empty_ish(&v) {
                results.push((Some([0.0; 6]), None, false, None));
                continue;
            }
            twopass_blur(&mut v, n, p.twoblur, &blur);
            let (f, line) = match (t, &found0) {
                (0, Some(f)) => (f.clone(), None),
                _ => {
                    let f = search(&v);
                    let line = (verbose && coarse_on)
                        .then(|| coarse_line(t, &f, coarse_settings.rotations));
                    (f, line)
                }
            };
            let init = initial(&f);
            let Some(basis) = &basis else {
                return Err(afni_89(messages));
            };
            let mut chat = chatty.then(String::new);
            let r = basis.register(&v, init, &it1, &mut work, chat.as_mut());
            let deg = r.radians.map(fit::to_degrees);
            results.push((
                Some([
                    deg[0],
                    deg[1],
                    deg[2],
                    r.params[3],
                    r.params[4],
                    r.params[5],
                ]),
                line,
                f.empty_region,
                chat,
            ));
        }
        for (t, (r, line, empty, chat)) in results.into_iter().enumerate() {
            first[t] = r;
            if let Some(l) = line {
                messages.push(Message::Info(l));
            }
            if let Some(c) = chat {
                messages.push(Message::Text(c));
            }
            empty_region |= empty;
        }
        if verbose {
            messages.push(Message::Info("CPU time for first pass=0 s".into()));
        }
    }
    if empty_region {
        defects::push_once(&mut notices, notices::afni_93(mode));
    }

    // §8.7: the final pass.
    let weight = match (&p.weight, p.wtinp) {
        (Some(w), _) => Weight::Given {
            w: w.clone(),
            edged: false,
        },
        (None, true) => Weight::Input(volume_f32(&bricks, 0)),
        (None, false) => Weight::Default,
    };
    if verbose {
        messages.push(Message::Info("Initializing alignment base".into()));
    }
    let settings = BasisSettings {
        resampler: Resampler {
            method: p.method,
            zpad: p.zpad,
            trig,
        },
        edging,
        dfac: p.delta,
        sinit,
        axes,
        blur,
        chatty,
    };
    let mut log = SetupLog::default();
    let basis = Basis::new(&base, n, sizes, weight, settings, &mut log);
    setup_lines(log, &mut once, &mut messages);
    let basis = match basis {
        Ok(b) => Some(b),
        Err(BasisError::NotThreeD) => {
            messages.push(Message::Raw(
                "\n*** mri_3dalign_setup: cannot use nD images!\x07".into(),
            ));
            return Err(fail(
                messages,
                "Can't initialize base image for alignment".into(),
            ));
        }
        Err(BasisError::LeastSquares) => None,
    };
    // AFNI-89: the first volume to be fitted (not the base, not empty-ish).
    if basis.is_none() {
        let fitted_one = (0..nt).any(|t| !skipped(t) && !empty_ish(&volume_f32(&bricks, t)));
        if fitted_one {
            return Err(afni_89(messages));
        }
    }
    let out_resampler = Resampler {
        method: p.final_method,
        zpad: p.zpad,
        trig,
    };
    if verbose {
        messages.push(Message::Text(format!(
            "++ Starting final pass on {nt} sub-bricks: "
        )));
    }
    let float_out = p.float_output || matches!(bricks.data, BrickData::Float(_));
    let mut out_float: Option<Vec<f32>> =
        (p.output && float_out && !matches!(bricks.data, BrickData::Float(_)))
            .then(|| vec![0f32; total * nt]);
    struct One {
        fitted: Fitted,
        rms: [f32; 2],
        warning: Option<String>,
        chat: Option<String>,
        stretched: bool,
    }
    let base_ref = &base;
    let register_one = |t: usize, y: &[f32], work: &mut Work, out: &mut [f32]| -> One {
        let mut chat = chatty.then(String::new);
        if skipped(t) {
            out.copy_from_slice(base_ref);
            return One {
                fitted: Fitted::default(),
                rms: [0.0, 0.0],
                warning: None,
                chat: None,
                stretched: false,
            };
        }
        let rms_of = |v: &[f32]| -> f32 {
            let mut s = 0f32;
            for (&b, &x) in base_ref.iter().zip(v) {
                let d = b - x;
                s += d * d;
            }
            (f64::from(s / total as f32)).sqrt() as f32
        };
        let rmsold = rms_of(y);
        if empty_ish(y) {
            out.copy_from_slice(y);
            return One {
                fitted: Fitted::default(),
                rms: [rmsold, rmsold],
                warning: Some(format!(
                    "skipped registration for #{t} because image is empty-ish"
                )),
                chat: None,
                stretched: false,
            };
        }
        let init = if p.twopass { first[t] } else { None };
        let basis = basis.as_ref().expect("checked above (AFNI-89)");
        let fitted = basis.register(y, init, &iteration, work, chat.as_mut());
        if p.output {
            let tf = basis.transform(&fitted.params);
            let tf = match tf {
                rotate::Transform::Angles { axes, shift, .. } => rotate::Transform::Angles {
                    theta: fitted.radians,
                    axes,
                    shift,
                },
                other => other,
            };
            out_resampler.apply(y, n, sizes, &tf, out, work);
            if p.clip && p.final_method.clips() {
                let (lo, hi) = y
                    .iter()
                    .fold((f32::INFINITY, f32::NEG_INFINITY), |(a, b), &x| {
                        (a.min(x), b.max(x))
                    });
                for v in out.iter_mut() {
                    if *v < lo {
                        *v = lo;
                    } else if *v > hi {
                        *v = hi;
                    }
                }
            }
        } else {
            out.copy_from_slice(y);
        }
        let rmsnew = if p.output { rms_of(out) } else { rmsold };
        One {
            fitted,
            rms: [rmsold, rmsnew],
            warning: None,
            chat,
            stretched: false,
        }
    };
    let factors = bricks.factors.clone();
    // One volume at a time (each one's resamplings parallel inside), registered in place.
    let mut work = Work::default();
    let mut buf = vec![0f32; total];
    let scaled = |t: usize| factors[t] != 0.0 && factors[t] != 1.0;
    let mut results: Vec<One> = Vec::with_capacity(nt);
    match (&mut bricks.data, out_float.as_mut()) {
        (BrickData::Float(v), _) => {
            for (t, slot) in v.chunks_mut(total).enumerate() {
                let r = if scaled(t) {
                    let f = factors[t];
                    let y: Vec<f32> = slot.iter().map(|&x| f * x).collect();
                    register_one(t, &y, &mut work, &mut buf)
                } else {
                    // The volume read in place: the output goes to `buf`, then back.
                    register_one(t, slot, &mut work, &mut buf)
                };
                if p.output {
                    slot.copy_from_slice(&buf);
                }
                results.push(r);
            }
        }
        (data, Some(of)) => {
            let view = Bricks {
                shape: [nx, ny, nz, nt],
                data: std::mem::replace(data, BrickData::Float(Vec::new())),
                factors: factors.clone(),
            };
            for (t, slot) in of.chunks_mut(total).enumerate() {
                let y = volume_f32(&view, t);
                results.push(register_one(t, &y, &mut work, slot));
            }
            *data = view.data;
        }
        (BrickData::Short(v), None) => {
            for (t, slot) in v.chunks_mut(total).enumerate() {
                let f = factors[t];
                let y: Vec<f32> = slot
                    .iter()
                    .map(|&x| {
                        if scaled(t) {
                            f * f32::from(x)
                        } else {
                            f32::from(x)
                        }
                    })
                    .collect();
                let r = register_one(t, &y, &mut work, &mut buf);
                if p.output {
                    for (s, &x) in slot.iter_mut().zip(buf.iter()) {
                        *s = f64::from(x).clamp(-32767.0, 32767.0).round_ties_even() as i16;
                    }
                }
                results.push(r);
            }
        }
        (BrickData::Byte(v), None) => {
            for (t, slot) in v.chunks_mut(total).enumerate() {
                let f = factors[t];
                let y: Vec<f32> = slot
                    .iter()
                    .map(|&x| {
                        if scaled(t) {
                            f * f32::from(x)
                        } else {
                            f32::from(x)
                        }
                    })
                    .collect();
                let mut r = register_one(t, &y, &mut work, &mut buf);
                if p.output {
                    let (lo, hi) = buf
                        .iter()
                        .fold((f32::INFINITY, f32::NEG_INFINITY), |(a, b), &x| {
                            (a.min(x), b.max(x))
                        });
                    let rounded: Vec<u8> = buf
                        .iter()
                        .map(|&x| f64::from(x).clamp(0.0, 255.0).round_ties_even() as u8)
                        .collect();
                    // AFNI-87: each volume stretched over 0..255.9 from its own range.
                    let hi2 = if hi <= lo { lo + 1.0 } else { hi };
                    let s = 255.9 / (f64::from(hi2) - f64::from(lo));
                    let stretched: Vec<u8> = buf
                        .iter()
                        .map(|&x| (s * f64::from(x - lo)).trunc() as u8)
                        .collect();
                    r.stretched = stretched != rounded;
                    slot.copy_from_slice(if mode.reproduce() {
                        &stretched
                    } else {
                        &rounded
                    });
                }
                results.push(r);
            }
        }
    }
    let mut fitted = Vec::with_capacity(nt);
    let mut rms = Vec::with_capacity(nt);
    let mut stretched = false;
    for (t, r) in results.into_iter().enumerate() {
        if verbose {
            messages.push(Message::Text(format!("{t}")));
        }
        if let Some(w) = r.warning {
            messages.push(Message::Warning(w));
        }
        if let Some(c) = r.chat {
            messages.push(Message::Text(c));
        }
        if verbose {
            messages.push(Message::Text("..".into()));
        }
        stretched |= r.stretched;
        fitted.push(r.fitted);
        rms.push(r.rms);
    }
    if stretched {
        defects::push_once(&mut notices, notices::afni_87(mode));
    }
    // §8.7 step 5: the reported parameters.
    let reported: Vec<[f32; 6]> = fitted
        .iter()
        .map(|f| report::reported(grid, f.radians, [f.params[3], f.params[4], f.params[5]]))
        .collect();
    if verbose {
        messages.push(Message::Raw(format!(
            "\n++ CPU time for realignment=0 s{}",
            if nt > 1 { "  [=0 s/sub-brick]" } else { "" }
        )));
        let names = ["roll", "pitch", "yaw", "dS", "dL", "dP"];
        let mut mn = reported[0];
        let mut mx = reported[0];
        let mut sum = [0f32; 6];
        for r in &reported {
            for q in 0..6 {
                mn[q] = mn[q].min(r[q]);
                mx[q] = mx[q].max(r[q]);
                sum[q] += r[q];
            }
        }
        let mean = sum.map(|s| s / nt as f32);
        for (label, v) in [("Min :", mn), ("Mean:", mean), ("Max :", mx)] {
            let parts: Vec<String> = names
                .iter()
                .zip(v)
                .map(|(nm, x)| format!("{nm}={}", cfmt::fixed_signed(f64::from(x), 3)))
                .collect();
            messages.push(Message::Info(format!("{label} {}", parts.join("  "))));
        }
    }
    // §8.8: the matrices and the maximum displacements.
    let cb = base_grid.centre();
    let ci = grid.centre();
    let matrices: Vec<[f64; 12]> = reported
        .iter()
        .map(|r| report::matrix_line(r, cb, ci, trig))
        .collect();
    let mut maxdisp = None;
    let mut maxdisp_messages = Vec::new();
    if !positions.is_empty() {
        let signed = mode.reproduce();
        let md = report::max_displacements(grid, &positions, &reported, signed, trig);
        let other = report::max_displacements(grid, &positions, &reported, !signed, trig);
        let shown = |m: &MaxDisp| -> (Vec<String>, String) {
            (
                m.per_volume
                    .iter()
                    .map(|(a, b)| {
                        format!(
                            "{}{}",
                            cfmt::fixed(f64::from(*a), 3),
                            cfmt::fixed(f64::from(*b), 3)
                        )
                    })
                    .collect(),
                format!(
                    "{} {} {} {}",
                    cfmt::fixed(f64::from(m.max.0), 2),
                    m.max.1,
                    cfmt::fixed(f64::from(m.max_delta.0), 2),
                    m.max_delta.1
                ),
            )
        };
        if shown(&md) != shown(&other) {
            defects::push_once(&mut notices, notices::afni_99_maxdisp(mode));
        }
        if verbose {
            let mut line = String::from("++ Max displacements (mm) for each sub-brick:\n");
            for (a, b) in &md.per_volume {
                line.push_str(&format!(
                    " {}({})",
                    cfmt::fixed(f64::from(*a), 2),
                    cfmt::fixed(f64::from(*b), 2)
                ));
            }
            maxdisp_messages.push(Message::Raw(line));
        }
        maxdisp_messages.push(Message::Info(format!(
            "Max displacement in automask = {} (mm) at sub-brick {}",
            cfmt::fixed(f64::from(md.max.0), 2),
            md.max.1
        )));
        maxdisp_messages.push(Message::Info(format!(
            "Max delta displ  in automask = {} (mm) at sub-brick {}",
            cfmt::fixed(f64::from(md.max_delta.0), 2),
            md.max_delta.1
        )));
        maxdisp = Some(md);
    }
    let displacements = p.savedisp.then(|| {
        let mine = displacement_series(grid, &reported, mode.reproduce(), trig);
        let other = displacement_series(grid, &reported, mode.fix(), trig);
        let differs = mine
            .iter()
            .zip(&other)
            .any(|(a, b)| a.iter().zip(b).any(|(x, y)| x.to_bits() != y.to_bits()));
        if differs {
            defects::push_once(&mut notices, notices::afni_99_savedisp(mode));
        }
        mine
    });
    let registered = p.output.then(|| match out_float {
        Some(v) => Bricks {
            shape: bricks.shape,
            data: BrickData::Float(v),
            factors: vec![0.0; nt],
        },
        None => Bricks {
            shape: bricks.shape,
            data: bricks.data,
            factors: vec![0.0; nt],
        },
    });
    Ok(VolregOutput {
        registered,
        fitted,
        reported,
        matrices,
        rms,
        maxdisp,
        displacements,
        messages,
        maxdisp_messages,
        notices,
    })
}

/// `-savedisp`'s displacements (§10.3) on the input's grid: three float32 series.
fn displacement_series(
    grid: &Grid,
    reported: &[[f32; 6]],
    signed: bool,
    trig: Trig,
) -> [Vec<f32>; 3] {
    let [nx, ny, nz] = grid.n;
    let total = nx * ny * nz;
    let nt = reported.len();
    let c = grid.axis_centre().map(f64::from);
    let mut out = [
        vec![0f32; total * nt],
        vec![0f32; total * nt],
        vec![0f32; total * nt],
    ];
    for (t, p) in reported.iter().enumerate() {
        let (rd, td) = report::dataset_transform(grid, p, signed, trig);
        for k in 0..nz {
            for j in 0..ny {
                for i in 0..nx {
                    let at = [i, j, k];
                    let x: [f64; 3] = std::array::from_fn(|a| {
                        f64::from(grid.origin[a] + at[a] as f32 * grid.delta[a])
                    });
                    let u = report::displacement(&rd, &td, &c, x);
                    let q = t * total + i + nx * (j + ny * k);
                    for a in 0..3 {
                        out[a][q] = u[a] as f32;
                    }
                }
            }
        }
    }
    out
}
