// SPDX-License-Identifier: Apache-2.0
//! A FLIRT run (`specs/flirt.md` §3 to §9): the images read and prepared, the reference
//! pyramid, the schedule interpreted command by command, and the outputs.

use std::collections::BTreeMap;
use std::io::Write;
use std::sync::Arc;

use larmorx_core::defects::{self, Notice, UpstreamDefects};
use larmorx_core::element::DataType;
use larmorx_io::nifti::NiftiHeader;

use larmorx_mri::hmc::rigid;
use larmorx_mri::hmc::volume::Volume;

use super::bbr::BbrSource;
use super::cost::{Cost, Log, Pair};
use super::mat::{self, IDENTITY, Mat4, Stream};
use super::optimise::{self, Subset};
use super::output::{self, Resampling, SincKernel};
use super::param::{self, AngleRep};
use super::paxes;
use super::pyramid;
use super::schedule::{self, Var};
use super::search::{self, Ranges};
use super::{Interp, Options, OutType};
use crate::bet::image::{self, Input, Output, Values};
use crate::cli::Settings;

/// How a run ends early.
#[derive(Debug)]
pub enum End {
    /// An exit status, its messages already printed.
    Exit(u8),
    /// FSL's exception (FSL-68): the message; exit status 1 by default, 0 with `reproduce`.
    Error(String),
    /// A case whose result FSL leaves undefined (it crashes, never finishes, reads memory it
    /// never set): refused in both modes with the message, exit status 1.
    Refuse(String),
}

/// One row of a matrix variable (§7.1): a cost and a matrix.
#[derive(Clone, Copy, Debug)]
pub struct Row {
    pub cost: f64,
    pub m: Mat4,
}

/// What a run reads besides its options.
pub struct Ctx<'a> {
    pub settings: &'a Settings,
    pub mode: UpstreamDefects,
    pub n_threads: usize,
}

/// An image as FLIRT holds it: its first volume (or every volume), its header and order.
struct Image {
    vols: Vec<Volume>,
    input: Input,
}

fn to_volume(v: &crate::bet::Vol) -> Volume {
    Volume::new(v.shape, v.voxel.map(|d| d as f32), v.data.clone())
}

/// Reads `name` as FSL's image layer does (every volume when `all`).
fn read(ctx: &Ctx<'_>, name: &str, all: bool, notices: &mut Vec<Notice>) -> Result<Image, End> {
    let files = crate::cli::bet2::resolve_image(ctx.settings, name, ctx.mode, notices)
        .map_err(|e| End::Error(read_message(name, &e)))?;
    if all {
        let (vols, input) =
            image::read(&files, ctx.n_threads).map_err(|e| End::Error(read_message(name, &e)))?;
        Ok(Image {
            vols: vols.iter().map(to_volume).collect(),
            input,
        })
    } else {
        let input = image::read_first(&files, ctx.n_threads)
            .map_err(|e| End::Error(read_message(name, &e)))?;
        Ok(Image {
            vols: vec![to_volume(&input.vol)],
            input,
        })
    }
}

/// FSL's messages for an image that cannot be read (§2.2).
fn read_message(name: &str, e: &image::ImageError) -> String {
    let text = e.to_string();
    let missing = text.starts_with("No image files match") || text.contains("No such file");
    if missing || text.starts_with("Multiple possible") {
        let stem = larmorx_mri::hmc::files::strip_image_extension(name);
        let what = if missing {
            format!("No image files match: {stem}")
        } else {
            format!("Multiple possible filenames detected for basename: {stem}")
        };
        format!(
            "Image Exception : #63 :: {what}\nImage Exception : #22 :: Failed to read volume {name}\nError : {what}\nFailed to read volume {name}\nError : {what}"
        )
    } else {
        format!("{name}: {text}")
    }
}

/// A single slice made three (§4.1): the slice repeated above and below, 8 mm thick.
fn triple(v: &Volume) -> Volume {
    if v.shape[2] != 1 {
        return v.clone();
    }
    let mut data = Vec::with_capacity(v.len() * 3);
    for _ in 0..3 {
        data.extend_from_slice(&v.data);
    }
    Volume::new(
        [v.shape[0], v.shape[1], 3],
        [v.voxel_size[0], v.voxel_size[1], 8.0],
        data,
    )
}

/// Clamping to the robust range (§4.1).
fn clamp(v: &Volume) -> Volume {
    let vol = crate::bet::Vol::new(v.shape, v.voxel_size.map(f64::from), v.data.clone());
    let (lo, hi) = crate::bet::params::robust_range(&vol);
    let (lo, hi) = (lo as f32, hi as f32);
    let data = v
        .data
        .iter()
        .map(|&x| {
            if x > hi {
                hi
            } else if x < lo {
                lo
            } else {
                x
            }
        })
        .collect();
    Volume::new(v.shape, v.voxel_size, data)
}

fn scaled(v: &Volume, b: f32) -> Volume {
    if (b - 1.0).abs() > 1e-5 {
        Volume::new(v.shape, v.voxel_size.map(|d| d / b), v.data.clone())
    } else {
        v.clone()
    }
}

/// An image's voxel-to-world matrix in FSL's memory order: its sform when set, else its
/// qform, else the voxel sizes (§8.5); `None` when neither code is set.
fn world(input: &Input) -> Option<Mat4> {
    let h = &input.header;
    if h.sform_code + h.qform_code <= 0 {
        return None;
    }
    let a = if h.sform_code > 0 {
        *h.sform().rows()
    } else {
        *h.qform().ok()?.rows()
    };
    let mut f = IDENTITY;
    if input.flipped {
        f[0][0] = -1.0;
        f[0][3] = input.vol.shape[0] as f64 - 1.0;
    }
    Some(mat::mul(&a, &f))
}

/// The scaled-mm conversion of a matrix (§3): `S M S⁻¹`, `S = diag(1/b)`.
fn scale_in(m: &Mat4, b: f32) -> Mat4 {
    if (b - 1.0).abs() <= 1e-5 {
        return *m;
    }
    let r = f64::from(1.0f32 / b);
    let s = mat::diagonal([r; 3]);
    let si = mat::diagonal([1.0 / r; 3]);
    mat::mul(&mat::mul(&s, m), &si)
}

/// The sform/qform initialisation (§8.5), in unscaled mm.
fn qsform(reference: &Input, input: &Input) -> Mat4 {
    let (Some(wr), Some(wi)) = (world(reference), world(input)) else {
        return IDENTITY;
    };
    let sr = mat::diagonal(reference.vol.voxel);
    let si = mat::diagonal(input.vol.voxel);
    let (Some(wri), Some(sii)) = (mat::inverse(&wr), mat::inverse(&si)) else {
        return IDENTITY;
    };
    mat::mul(&mat::mul(&mat::mul(&sr, &wri), &wi), &sii)
}

/// The base scale (§4.2).
fn base_scale(o: &Options, a: &Volume, b: &Volume) -> f32 {
    if let Some(s) = o.base_scale {
        return s;
    }
    if o.apply_xfm || o.apply_iso.is_some() {
        return 1.0;
    }
    let all: Vec<f32> = a
        .voxel_size
        .iter()
        .chain(&b.voxel_size)
        .map(|d| d.abs())
        .collect();
    let mx = all.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let mn = all.iter().copied().fold(f32::INFINITY, f32::min);
    let mut s = 1.0;
    if mx > 12.0 {
        s = mx / 8.0;
    }
    if mn < 0.75 {
        s = mn;
    }
    s
}

/// Reads a matrix file (§2.2): its rows, `Could not open matrix file` (an empty matrix) or the
/// numbers' error.
fn read_matrix(
    settings: &Settings,
    name: &str,
    err: &mut dyn Write,
) -> Result<Vec<Vec<f64>>, String> {
    match std::fs::read_to_string(settings.path(name)) {
        Ok(text) => mat::parse_matrix_text(&text),
        Err(_) => {
            let _ = writeln!(err, "Could not open matrix file {name}");
            Ok(Vec::new())
        }
    }
}

/// A 4 × 4 matrix where FLIRT multiplies by one (§2.2): any other size is the product's error.
fn as_mat4(rows: &[Vec<f64>]) -> Result<Mat4, String> {
    let r = rows.len();
    let c = rows.first().map_or(0, Vec::len);
    mat::mat4_of(rows)
        .filter(|_| r == 4 && c == 4)
        .ok_or_else(|| {
            format!("matrix multiplication: incompatible matrix dimensions: 4x4 and {r}x{c}")
        })
}

/// The run's state while the schedule runs.
struct State<'a> {
    o: &'a Options,
    ctx: &'a Ctx<'a>,
    log: Arc<Log>,
    stream: Stream,
    /// The reference and input as read and prepared (clamped, tripled, scaled).
    reference: Volume,
    input0: Volume,
    ref_weight: Option<Volume>,
    in_weight0: Option<Volume>,
    /// The pyramid's levels (1, 2, 4, 8 mm) and their weights.
    levels: [Volume; 4],
    level_weights: [Option<Volume>; 4],
    one_replaced: bool,
    bbr: Arc<BbrSource>,
    pair: Option<Arc<Pair>>,
    lists: BTreeMap<Var, Vec<Row>>,
    scale: f32,
    opt: optimise::Settings,
    subset: Option<Subset>,
    smoothing: f32,
    fuzzy: f32,
    cost: Cost,
    search_cost: Cost,
    bbr_step: i64,
    min_sampling: f32,
    maxdof: usize,
    init: Mat4,
    qsform: Mat4,
    fine_counts: [usize; 3],
}

impl State<'_> {
    fn pair(&self) -> Result<&Arc<Pair>, End> {
        self.pair.as_ref().ok_or_else(|| {
            End::Error("no image pair: the schedule has no setscale before its first use".into())
        })
    }

    fn check(&self) -> Result<(), End> {
        match self.log.fatal_error() {
            Some(m) if m.starts_with("larmorx: ") => Err(End::Refuse(m)),
            Some(m) => Err(End::Error(m)),
            None => Ok(()),
        }
    }

    fn flush(&self, err: &mut dyn Write) {
        for line in self.log.take() {
            let _ = writeln!(err, "{line}");
        }
    }

    fn list(&self, v: Var) -> &[Row] {
        self.lists.get(&v).map_or(&[], Vec::as_slice)
    }

    fn rows(&self, word: &str) -> Result<(Var, Vec<Row>), End> {
        let (v, r1, r2) =
            schedule::parse_ref(word, self.maxdof as f64, f64::from(self.min_sampling))
                .map_err(|m| refuse_name(&m))?;
        let l = self.list(v);
        let lo = (r1 as usize).saturating_sub(1).min(l.len());
        let hi = (r2 as usize).min(l.len()).max(lo);
        Ok((v, l[lo..hi].to_vec()))
    }

    fn var(&self, word: &str) -> Result<Var, End> {
        schedule::parse_var(word.split(':').next().unwrap_or(word)).map_err(|m| refuse_name(&m))
    }

    fn push(&mut self, v: Var, row: Row) {
        self.lists.entry(v).or_default().push(row);
    }

    fn scalar(&self, w: &str) -> f64 {
        schedule::scalar(w, self.maxdof as f64, f64::from(self.min_sampling))
    }

    fn eval(&self, t: &Mat4, cost: Cost) -> Result<f32, End> {
        let p = self.pair()?;
        Ok(evaluate(p, t, cost, &self.log))
    }

    fn deltas(&self, dof: usize) -> [f64; 12] {
        let mut d = [0.0; 12];
        for (k, dk) in d.iter_mut().enumerate().take(3) {
            *dk = if self.fine_counts[k] > 1 {
                f64::from(self.o.fine) / 2.0
            } else {
                3.0 * f64::from(self.opt.tolerance(k).unwrap_or(0.0))
            };
        }
        for dk in d.iter_mut().take(6).skip(3) {
            *dk = 0.05;
        }
        for dk in d.iter_mut().take(9).skip(6) {
            *dk = if self.o.dof >= 7 { 0.1 } else { 0.0 };
        }
        for dk in d.iter_mut().skip(9) {
            *dk = 0.05;
        }
        let _ = dof;
        d
    }

    /// The perturbed matrix of a row (§7.1).
    fn perturb(&self, m: &Mat4, p: &[f64], abs: bool, dof: usize) -> Result<Mat4, End> {
        let pair = self.pair()?;
        let c = pair.centre;
        let mut x = param::decompose(self.opt.rep, m, c);
        let d = self.deltas(dof);
        for (i, &pi) in p.iter().enumerate() {
            if i >= 12 {
                return Err(End::Error("Mat::operator(): index out of bounds".into()));
            }
            x[i] += if abs { pi } else { pi * d[i] };
        }
        Ok(param::compose(self.opt.rep, &x, dof.max(6), c, &self.log))
    }
}

fn refuse_name(m: &str) -> End {
    End::Refuse(format!(
        "{m}\nlarmorx: an undefined matrix name in the schedule: FSL reads an uninitialised list there; refused"
    ))
}

/// The cost of the trial matrix `t` with `cost`; evaluations of large grids sample their
/// slices in parallel.
fn evaluate(pair: &Pair, t: &Mat4, cost: Cost, log: &Log) -> f32 {
    let parallel = pair.reference.len() > 16_384;
    pair.cost_of(t, cost, parallel, log)
}

fn wrong(cmd: &str, err: &mut dyn Write) -> End {
    let _ = writeln!(err, "Wrong number of args to {}", cmd.to_uppercase());
    End::Exit(255)
}

/// Runs FLIRT (§2 to §9) with options `o`. Returns the exit status.
pub fn run(o: &Options, ctx: &Ctx<'_>, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let mut notices: Vec<Notice> = Vec::new();
    let result = larmorx_core::parallel::with_threads(ctx.n_threads, || {
        let mut o_buf: Vec<u8> = Vec::new();
        let mut e_buf: Vec<u8> = Vec::new();
        let mut n = Vec::new();
        let r = run_inner(o, ctx, &mut o_buf, &mut e_buf, &mut n);
        (r, o_buf, e_buf, n)
    });
    let (r, o_buf, e_buf, n) = match result {
        Ok(x) => x,
        Err(e) => {
            let _ = writeln!(err, "larmorx fsl flirt: {e}");
            return 1;
        }
    };
    let _ = out.write_all(&o_buf);
    let _ = err.write_all(&e_buf);
    for x in n {
        defects::push_once(&mut notices, x);
    }
    let code = match r {
        Ok(()) => 0,
        Err(End::Exit(c)) => c,
        Err(End::Refuse(m)) => {
            let _ = writeln!(err, "{m}");
            1
        }
        Err(End::Error(m)) => {
            let _ = writeln!(err, "{m}");
            let detail = match ctx.mode {
                UpstreamDefects::Fix => {
                    "FLIRT's error ends the run with exit status 1 (FSL exits 0 after its message)"
                }
                UpstreamDefects::Reproduce => {
                    "FLIRT's error ends the run with exit status 0, as FSL does"
                }
            };
            defects::push_once(
                &mut notices,
                Notice::new(&defects::FSL_68, ctx.mode, detail),
            );
            u8::from(ctx.mode.fix())
        }
    };
    for x in &notices {
        let _ = writeln!(err, "larmorx: {}", x.line());
    }
    code
}

fn run_inner(
    o: &Options,
    ctx: &Ctx<'_>,
    out: &mut dyn Write,
    err: &mut dyn Write,
    notices: &mut Vec<Notice>,
) -> Result<(), End> {
    let in_name = o.input.clone().unwrap_or_default();
    let ref_name = o.reference.clone().unwrap_or_default();
    let applying = o.apply_xfm || o.apply_iso.is_some();
    let reference = read(ctx, &ref_name, false, notices)?;
    if reference.input.volumes > 1 && applying {
        let _ = writeln!(
            err,
            "Warning: An input intended to be a single 3D volume has multiple timepoints. Input will be truncated to first volume, but this functionality is deprecated and will be removed in a future release."
        );
    }
    let input = read(ctx, &in_name, applying, notices)?;
    let ref_sform = reference.input.header.sform_code > 0;
    let in_sform = input.input.header.sform_code > 0;
    if o.verbose >= 1 {
        if ref_sform {
            let _ = writeln!(
                out,
                "The output image will use the sform from the reference image"
            );
        }
        if in_sform && !(applying && ref_sform) {
            let _ = writeln!(
                out,
                "The output image will use the transformed sform from the input image"
            );
        }
    }
    if ref_sform && in_sform && o.verbose >= 1 {
        let _ = writeln!(
            err,
            "WARNING: Both reference and input images have an sform matrix set"
        );
    }
    // the initial matrix
    let q_unscaled = qsform(&reference.input, &input.input);
    let init_unscaled = if let Some(name) = &o.init {
        let rows = read_matrix(ctx.settings, name, err).map_err(End::Error)?;
        as_mat4(&rows).map_err(End::Error)?
    } else if o.use_sqform {
        q_unscaled
    } else {
        IDENTITY
    };
    let mut stream = Stream::default();
    if o.verbose >= 1 || o.print_init {
        let _ = write!(out, "Init Matrix = \n{}\n", stream.matrix(&init_unscaled));
    }
    if applying {
        return apply(
            o,
            ctx,
            &reference,
            &input,
            &init_unscaled,
            out,
            err,
            notices,
        );
    }
    register(
        o,
        ctx,
        reference,
        input,
        init_unscaled,
        q_unscaled,
        stream,
        out,
        err,
        notices,
    )
}

/// The images of a registration, prepared (§4).
#[allow(clippy::too_many_arguments)]
fn register(
    o: &Options,
    ctx: &Ctx<'_>,
    reference: Image,
    input: Image,
    init_unscaled: Mat4,
    q_unscaled: Mat4,
    stream: Stream,
    out: &mut dyn Write,
    err: &mut dyn Write,
    notices: &mut Vec<Notice>,
) -> Result<(), End> {
    let mode = ctx.mode;
    let weighting = o.ref_weight.is_some() || o.in_weight.is_some();
    let prep = |v: &Volume| -> Volume {
        let t = triple(v);
        if o.no_clamp { t } else { clamp(&t) }
    };
    let ref0 = prep(&reference.vols[0]);
    let in0 = prep(&input.vols[0]);
    // from the headers, before the single-slice tripling (observed)
    let b = base_scale(o, &reference.vols[0], &input.vols[0]);
    let ref0 = scaled(&ref0, b);
    let in0 = scaled(&in0, b);
    let read_weight = |name: &Option<String>,
                       like: &Volume,
                       notices: &mut Vec<Notice>|
     -> Result<Option<Volume>, End> {
        match name {
            Some(n) => {
                let w = read(ctx, n, false, notices)?;
                Ok(Some(scaled(&triple(&w.vols[0]), b)))
            }
            None if weighting => Ok(Some(Volume::new(
                like.shape,
                like.voxel_size,
                vec![1.0; like.len()],
            ))),
            None => Ok(None),
        }
    };
    let ref_weight = read_weight(&o.ref_weight, &ref0, notices)?;
    let in_weight0 = read_weight(&o.in_weight, &in0, notices)?;
    // the boundary-based cost's inputs
    let mut bbr = BbrSource {
        pedir: o.pedir,
        echo_spacing: o.echo_spacing,
        bbr_type: o.bbr_type.clone(),
        slope: o.bbr_slope,
        ..BbrSource::default()
    };
    if let Some(n) = &o.wmseg {
        let w = read(ctx, n, false, notices)?;
        bbr.wmseg = Some(scaled(&triple(&w.vols[0]), b));
    }
    if let (Some(c), Some(nm)) = (&o.wmcoords, &o.wmnorms) {
        let cr = read_matrix(ctx.settings, c, err).map_err(End::Error)?;
        let nr = read_matrix(ctx.settings, nm, err).map_err(End::Error)?;
        if cr.is_empty() || cr.len() != nr.len() {
            let _ = writeln!(
                err,
                "Coordinate and Normal matrices {c} and {nm} are either zero size or different sizes"
            );
            return Err(End::Exit(1));
        }
        let three = |r: &Vec<f64>| {
            [
                r[0],
                r.get(1).copied().unwrap_or(0.0),
                r.get(2).copied().unwrap_or(0.0),
            ]
        };
        bbr.coords = Some((
            cr.iter().map(three).collect(),
            nr.iter().map(three).collect(),
        ));
    }
    if let Some(n) = &o.fieldmap {
        if o.pedir == 0 {
            return Err(End::Refuse(
                "larmorx: -fieldmap needs -pedir (FSL's result is undefined without it): refused"
                    .into(),
            ));
        }
        let f = read(ctx, n, false, notices)?;
        let fm = scaled(&triple(&f.vols[0]), b);
        let mask = match &o.fieldmap_mask {
            Some(mn) => {
                let m = read(ctx, mn, false, notices)?;
                scaled(&triple(&m.vols[0]), b)
            }
            None => Volume::new(
                fm.shape,
                fm.voxel_size,
                fm.data.iter().map(|&v| 0.0 * v + 1.0).collect(),
            ),
        };
        bbr.fieldmap = Some((fm, mask));
    }
    // the minimum sampling (§4.3)
    let vmin = |v: &Volume| {
        v.voxel_size
            .iter()
            .map(|d| d.abs())
            .fold(f32::INFINITY, f32::min)
    };
    let s_min = vmin(&ref0).max(vmin(&in0)).ceil();
    let min_sampling = if o.force_scaling {
        o.min_sampling
    } else {
        o.min_sampling.max(s_min)
    };
    // the reference pyramid (§5)
    let filt = |v: &Volume,
                w: &Option<Volume>,
                f: &dyn Fn(&Volume) -> Volume|
     -> (Volume, Option<Volume>) {
        match w {
            Some(w) => {
                let (a, b) = pyramid::weighted(v, w, f);
                (a, Some(b))
            }
            None => (f(v), None),
        }
    };
    let (levels, level_weights) = if o.no_resample {
        (
            [ref0.clone(), ref0.clone(), ref0.clone(), ref0.clone()],
            [
                ref_weight.clone(),
                ref_weight.clone(),
                ref_weight.clone(),
                ref_weight.clone(),
            ],
        )
    } else {
        let s = min_sampling;
        let (l1, w1) = filt(&ref0, &ref_weight, &|v| {
            pyramid::isotropic(&pyramid::blur(v, s), s)
        });
        let halve = |v: &Volume,
                     w: &Option<Volume>,
                     notices: &mut Vec<Notice>|
         -> Result<(Volume, Option<Volume>), End> {
            match pyramid::background(v, mode) {
                Some(bg) => {
                    for n in
                        pyramid::background_notices(&bg, "a level of the reference pyramid", mode)
                    {
                        defects::push_once(notices, n);
                    }
                }
                None => {
                    return Err(End::Refuse(
                        "larmorx: a level of the reference pyramid is 1 x 1 x 1 voxels: its background value (the 10th percentile of its border) has no border to read (FSL reads an empty buffer, FSL-75); refused"
                            .into(),
                    ));
                }
            }
            Ok(filt(v, w, &pyramid::halve))
        };
        let (l2, w2) = if s < 1.9 {
            halve(&l1, &w1, notices)?
        } else {
            (l1.clone(), w1.clone())
        };
        let (l4, w4) = if s < 3.9 {
            halve(&l2, &w2, notices)?
        } else {
            (l2.clone(), w2.clone())
        };
        let (l8, w8) = if s < 7.9 {
            halve(&l4, &w4, notices)?
        } else {
            (l4.clone(), w4.clone())
        };
        ([l1, l2, l4, l8], [w1, w2, w4, w8])
    };
    if o.debug {
        // §9.4: the pyramid's levels (development: the volumes, single precision)
        for (name, v) in [
            ("refvol_8", &levels[3]),
            ("refvol_4", &levels[2]),
            ("refvol_2", &levels[1]),
            ("refvol", &levels[0]),
        ] {
            let shape = v.shape;
            let mut h = reference.input.header.clone();
            h.pixdim[1] = f64::from(v.voxel_size[0]);
            h.pixdim[2] = f64::from(v.voxel_size[1]);
            h.pixdim[3] = f64::from(v.voxel_size[2]);
            h.sform_code = 0;
            h.qform_code = 0;
            let like = Input {
                vol: crate::bet::Vol::new(shape, v.voxel_size.map(f64::from), v.data.clone()),
                header: h,
                flipped: reference.input.flipped,
                volumes: 1,
            };
            write_image(
                ctx,
                name,
                &like,
                shape,
                1,
                None,
                v.data.clone(),
                DataType::F32,
                err,
                notices,
            )?;
        }
    }
    let init = scale_in(&init_unscaled, b);
    let qs = scale_in(&q_unscaled, b);
    let (maxdof, dof_msg) = optimise::clamp_dof(i64::from(o.dof));
    let _ = dof_msg;
    let ranges = o.search;
    let fine_counts = [0, 1, 2].map(|k| search::samples(ranges[k], o.fine).len());
    let mut st = State {
        o,
        ctx,
        log: Arc::new(Log::default()),
        stream,
        reference: ref0,
        input0: in0,
        ref_weight,
        in_weight0,
        levels,
        level_weights,
        one_replaced: false,
        bbr: Arc::new(bbr),
        pair: None,
        lists: BTreeMap::new(),
        scale: 1.0,
        opt: optimise::Settings::default(),
        subset: None,
        smoothing: 1.0,
        fuzzy: 0.5,
        cost: o.cost,
        search_cost: o.search_cost,
        bbr_step: 1,
        min_sampling,
        maxdof: o.dof.max(0) as usize,
        init,
        qsform: qs,
        fine_counts,
    };
    let _ = maxdof;
    st.opt.rep = if o.quaternion {
        AngleRep::Quaternion
    } else {
        AngleRep::Euler
    };
    st.opt.log = Some(st.log.clone());
    // the schedule
    let text = match &o.schedule {
        Some(name) => match std::fs::read_to_string(ctx.settings.path(name)) {
            Ok(t) => t,
            Err(_) => {
                let _ = writeln!(err, "Could not open file{name}");
                return Err(End::Exit(255));
            }
        },
        None if o.two_d => schedule::DEFAULT_2D.to_owned(),
        None => schedule::DEFAULT_3D.to_owned(),
    };
    let lines: Vec<&str> = if o.schedule.is_some() {
        text.split('\n').collect()
    } else {
        text.lines().collect()
    };
    // the first pair: the 8 mm level and the input blurred to 8 mm (§5), before any setscale
    set_scale(&mut st, 8.0, true)?;
    st.flush(err);
    let mut skip = false;
    for line in lines {
        if o.verbose >= 1 {
            let _ = writeln!(out, " >> {line}");
        }
        if skip {
            skip = false;
            continue;
        }
        let w = schedule::words(line);
        let r = command(&mut st, &w, out, err, &mut skip);
        st.flush(err);
        r?;
        st.check()?;
    }
    // the result (§9.1)
    let Some(best) = st.list(Var::U(0)).first().copied() else {
        let _ = writeln!(err, "Failed to calculate any transformation matrix");
        return Ok(());
    };
    let mut result = mat::mul(&best.m, &st.init);
    if (b - 1.0).abs() > 1e-5 {
        for row in result.iter_mut().take(3) {
            row[3] *= f64::from(b);
        }
    }
    match &o.omat {
        Some(name) if !name.is_empty() => {
            if std::fs::write(ctx.settings.path(name), mat::file_text(&result)).is_err() {
                let _ = writeln!(err, "Could not open file {name} for writing");
            }
        }
        Some(_) => {}
        None => {
            let _ = write!(out, "\nFinal result: \n{}\n", st.stream.matrix(&result));
        }
    }
    if let Some(stem) = &o.out {
        write_registered(o, ctx, &reference, &input, &result, stem, err, notices)?;
    }
    Ok(())
}

/// Runs one schedule command (§7.1).
fn command(
    st: &mut State<'_>,
    w: &[String],
    out: &mut dyn Write,
    err: &mut dyn Write,
    skip: &mut bool,
) -> Result<(), End> {
    let Some(cmd) = w.first() else {
        return Ok(());
    };
    if cmd.starts_with('#') {
        return Ok(());
    }
    let n = w.len();
    match cmd.as_str() {
        "clear" => {
            if n < 2 {
                return Err(wrong(cmd, err));
            }
            let v = st.var(&w[1])?;
            st.lists.insert(v, Vec::new());
        }
        "copy" => {
            if n < 3 {
                return Err(wrong(cmd, err));
            }
            let (src, r1, r2) =
                schedule::parse_ref(&w[1], st.maxdof as f64, f64::from(st.min_sampling))
                    .map_err(|m| refuse_name(&m))?;
            let dst = st.var(&w[2])?;
            let mut i = r1 as usize;
            loop {
                let len = st.list(src).len();
                if i > len || i > r2 as usize {
                    break;
                }
                let row = st.list(src)[i - 1];
                st.push(dst, row);
                i += 1;
                if src != dst && i > len {
                    break;
                }
            }
        }
        "print" => {
            if n < 2 {
                return Err(wrong(cmd, err));
            }
            let (_, rows) = st.rows(&w[1])?;
            st.stream.fixed = true;
            for r in rows {
                let _ = writeln!(out, "{}", row_text(&r));
            }
        }
        "save" => {
            if n < 3 {
                return Err(wrong(cmd, err));
            }
            let (_, rows) = st.rows(&w[1])?;
            let mut s = String::new();
            for r in rows {
                s.push_str(&row_text(&r));
                s.push('\n');
            }
            s.push('\n');
            let _ = std::fs::write(st.ctx.settings.path(&w[2]), s);
        }
        "read" => {
            if n < 3 {
                return Err(wrong(cmd, err));
            }
            let v = st.var(&w[1])?;
            let rows = read_rows(st.ctx.settings, &w[2], 17)?;
            let list: Vec<Row> = rows
                .into_iter()
                .map(|r| Row {
                    cost: r[0],
                    m: column_major(&r[1..17]),
                })
                .collect();
            st.lists.insert(v, list);
        }
        "printparams" | "saveparams" => {
            if n < if cmd == "printparams" { 2 } else { 3 } {
                return Err(wrong(cmd, err));
            }
            let (_, rows) = st.rows(&w[1])?;
            let c = st.pair()?.centre;
            let mut s = String::new();
            for (k, r) in rows.iter().enumerate() {
                let p = param::decompose(st.opt.rep, &r.m, c);
                let cost = if cmd == "saveparams" && k == 0 {
                    mat::g(r.cost, 6)
                } else {
                    st.stream.num(r.cost)
                };
                s.push_str(&cost);
                s.push(' ');
                for v in p {
                    s.push_str(&mat::f6(v));
                    s.push(' ');
                }
                s.push_str("\n\n");
            }
            if cmd == "printparams" {
                st.stream.fixed = true;
                let _ = write!(out, "{s}");
            } else {
                s.push('\n');
                let _ = std::fs::write(st.ctx.settings.path(&w[2]), s);
            }
        }
        "readparams" => {
            if n < 3 {
                return Err(wrong(cmd, err));
            }
            let v = st.var(&w[1])?;
            let rows = read_rows(st.ctx.settings, &w[2], 13)?;
            let c = st.pair()?.centre;
            let list: Vec<Row> = rows
                .into_iter()
                .map(|r| {
                    let mut p = [0.0; 12];
                    p.copy_from_slice(&r[1..13]);
                    Row {
                        cost: f64::from(r[0] as f32),
                        m: param::compose(st.opt.rep, &p, 12, c, &st.log),
                    }
                })
                .collect();
            st.lists.insert(v, list);
        }
        "sort" => {
            if n < 2 {
                return Err(wrong(cmd, err));
            }
            let v = st.var(&w[1])?;
            if let Some(l) = st.lists.get_mut(&v) {
                l.sort_by(|a, b| {
                    a.cost
                        .partial_cmp(&b.cost)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
            }
        }
        "dualsort" => {
            if n < 3 {
                return Err(wrong(cmd, err));
            }
            let (a, b) = (st.var(&w[1])?, st.var(&w[2])?);
            let (la, lb) = (st.list(a).to_vec(), st.list(b).to_vec());
            if la.len() != lb.len() {
                let _ = write!(err, "Cannot dual sort matrices of unequal size");
            } else {
                let mut idx: Vec<usize> = (0..la.len()).collect();
                idx.sort_by(|&i, &j| {
                    la[i]
                        .cost
                        .partial_cmp(&la[j].cost)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
                st.lists.insert(a, idx.iter().map(|&i| la[i]).collect());
                st.lists.insert(b, idx.iter().map(|&i| lb[i]).collect());
            }
        }
        "search" => {
            if n > 2 {
                return Err(wrong(cmd, err));
            }
            let dof = if n == 2 {
                schedule::int_scalar(st.scalar(&w[1]))
            } else {
                12
            };
            let dof = optimise::clamp_dof(dof).0;
            run_search(st, dof)?;
        }
        "optimise" | "measurecost" => {
            if n < 5 {
                return Err(wrong(cmd, err));
            }
            let dof_req = schedule::int_scalar(st.scalar(&w[1]));
            let (dof, msg) = optimise::clamp_dof(dof_req.min(st.maxdof as i64));
            if let Some(m) = msg {
                let _ = writeln!(err, "{m}");
            }
            let (_, rows) = st.rows(&w[2])?;
            let mut p = Vec::new();
            let mut abs = false;
            let mut k = 3;
            while k < n {
                match w[k].as_str() {
                    "rel" => {
                        k += 1;
                        break;
                    }
                    "abs" => {
                        abs = true;
                        k += 1;
                        break;
                    }
                    x => p.push(st.scalar(x)),
                }
                k += 1;
            }
            let maxit = if k < n && !w[k].is_empty() {
                schedule::int_scalar(st.scalar(&w[k])).max(0) as usize
            } else {
                4
            };
            if p.len() > 12 {
                return Err(End::Error("Mat::operator(): index out of bounds".into()));
            }
            for r in rows {
                let m = st.perturb(&r.m, &p, abs, dof)?;
                if cmd == "measurecost" {
                    let c = st.eval(&m, st.cost)?;
                    st.push(
                        Var::U(0),
                        Row {
                            cost: f64::from(c),
                            m,
                        },
                    );
                } else {
                    let (m2, c) = optimise_row(st, &m, dof, maxit)?;
                    st.push(
                        Var::U(0),
                        Row {
                            cost: f64::from(c),
                            m: m2,
                        },
                    );
                }
                st.check()?;
            }
        }
        "gridmeasurecost" => {
            if n < 7 {
                return Err(wrong(cmd, err));
            }
            let dof_req = schedule::int_scalar(st.scalar(&w[1]));
            let (dof, _) = optimise::clamp_dof(dof_req.min(st.maxdof as i64));
            let (_, rows) = st.rows(&w[2])?;
            let mut triples: Vec<[f64; 3]> = Vec::new();
            let mut k = 3;
            while k + 2 < n && w[k] != "rel" && w[k] != "abs" {
                triples.push([st.scalar(&w[k]), st.scalar(&w[k + 1]), st.scalar(&w[k + 2])]);
                k += 3;
            }
            let abs = w[k..].iter().any(|x| x == "abs");
            let counts: Vec<i64> = triples
                .iter()
                .map(|t| {
                    let mut q = (t[2] - t[0]) / t[1];
                    if q.is_nan() {
                        q = 1e-6;
                    }
                    let nc = (q.max(1e-6) + 0.000001).ceil();
                    schedule::int_scalar(nc)
                })
                .collect();
            let total: i64 = counts.iter().product();
            for r in rows {
                let mut count = vec![0i64; triples.len()];
                let mut mats = Vec::with_capacity(total.max(0) as usize);
                for _ in 0..total.max(0) {
                    // the odometer, advanced before each point
                    for (c, &nc) in count.iter_mut().zip(&counts) {
                        *c += 1;
                        if *c >= nc {
                            *c = 0;
                        } else {
                            break;
                        }
                    }
                    let p: Vec<f64> = triples
                        .iter()
                        .zip(&count)
                        .map(|(t, &c)| t[0] + c as f64 * t[1])
                        .collect();
                    mats.push(st.perturb(&r.m, &p, abs, dof)?);
                }
                let pair = st.pair()?.clone();
                let log = st.log.clone();
                let cost = st.cost;
                use rayon::prelude::*;
                let costs: Vec<f32> = mats
                    .par_iter()
                    .map(|m| pair.cost_of(m, cost, false, &log))
                    .collect();
                for (m, c) in mats.into_iter().zip(costs) {
                    st.push(
                        Var::U(0),
                        Row {
                            cost: f64::from(c),
                            m,
                        },
                    );
                }
                st.check()?;
            }
        }
        "aligncog" => {
            if n < 2 {
                return Err(wrong(cmd, err));
            }
            let (_, rows) = st.rows(&w[1])?;
            let pair = st.pair()?.clone();
            let cr = pair.reference.centre_of_mass();
            let ci = pair.centre;
            for r in rows {
                let mut m = r.m;
                for i in 0..3 {
                    m[i][3] = cr[i] - (m[i][0] * ci[0] + m[i][1] * ci[1] + m[i][2] * ci[2]);
                }
                st.push(Var::U(0), Row { cost: r.cost, m });
            }
        }
        "alignpaxes" => {
            if n < 2 {
                return Err(wrong(cmd, err));
            }
            let (_, rows) = st.rows(&w[1])?;
            let pair = st.pair()?.clone();
            let mut warn = Vec::new();
            let rr = paxes::axes(&pair.reference, &mut warn).map_err(End::Error)?;
            let ri = paxes::axes(&pair.input, &mut warn).map_err(End::Error)?;
            for line in warn {
                st.log.push(line);
            }
            let m = paxes::align(rr, ri, pair.reference.centre_of_mass(), pair.centre);
            for r in rows {
                st.push(Var::U(0), Row { cost: r.cost, m });
            }
        }
        "setscale" => {
            if n < 2 {
                return Err(wrong(cmd, err));
            }
            let s = st.scalar(&w[1]) as f32;
            let force = n > 2 && w[2] == "force";
            set_scale(st, s, force)?;
        }
        "setrow" => {
            if n < 18 {
                return Err(wrong(cmd, err));
            }
            let v = st.var(&w[1])?;
            let vals: Vec<f64> = w[2..18].iter().map(|x| st.scalar(x)).collect();
            st.push(
                v,
                Row {
                    cost: 0.0,
                    m: column_major(&vals),
                },
            );
        }
        "setrowqsform" => {
            if n < 2 {
                return Err(wrong(cmd, err));
            }
            let v = st.var(&w[1])?;
            let m = if st.o.use_sqform {
                IDENTITY
            } else {
                let inv = mat::inverse(&st.init).ok_or_else(|| End::Error(mat::SINGULAR.into()))?;
                mat::mul(&st.qsform, &inv)
            };
            st.push(v, Row { cost: 0.0, m });
        }
        "setrowparams" => {
            if n < 14 {
                return Err(wrong(cmd, err));
            }
            let v = st.var(&w[1])?;
            let mut p = [0.0; 12];
            for (k, pk) in p.iter_mut().enumerate() {
                *pk = st.scalar(&w[2 + k]);
            }
            let c = st.pair()?.centre;
            st.push(
                v,
                Row {
                    cost: 0.0,
                    m: param::compose(st.opt.rep, &p, 12, c, &st.log),
                },
            );
        }
        "setoption" => {
            if n < 3 {
                return Err(wrong(cmd, err));
            }
            set_option(st, w, err)?;
        }
        "if" => {
            if n < 4 {
                return Err(wrong(cmd, err));
            }
            let a = st.scalar(&w[1]) as f32;
            let b = st.scalar(&w[3]) as f32;
            let holds = match w[2].as_str() {
                ">" => a > b,
                "<" => a < b,
                "==" => a == b,
                "!=" => a != b,
                "<=" => a <= b,
                ">=" => a >= b,
                op => {
                    let _ = writeln!(err, "Cannot recognise operator {op} in IF statement");
                    return Err(End::Exit(255));
                }
            };
            *skip = !holds;
        }
        "exit" => return Err(End::Exit(0)),
        other => {
            let _ = writeln!(err, "Unrecognised command {other}");
            let _ = writeln!(err, " ... ignoring");
        }
    }
    Ok(())
}

/// A row as `print` and `save` write it: the cost and the matrix column by column, fixed.
fn row_text(r: &Row) -> String {
    let mut s = mat::f6(r.cost);
    s.push(' ');
    for j in 0..4 {
        for i in 0..4 {
            s.push_str(&mat::f6(r.m[i][j]));
            s.push(' ');
        }
    }
    s
}

/// The matrix whose columns are `v[0..4]`, `v[4..8]`, ... (the order rows store a matrix).
fn column_major(v: &[f64]) -> Mat4 {
    let mut m = [[0.0; 4]; 4];
    for j in 0..4 {
        for i in 0..4 {
            m[i][j] = v[4 * j + i];
        }
    }
    m
}

/// The rows of `width` numbers of a file `read` reads (§7.1): a row kept only when the end of
/// the file was not reached while reading it. A word that is not a number: FSL-72, refused.
fn read_rows(settings: &Settings, name: &str, width: usize) -> Result<Vec<Vec<f64>>, End> {
    let text = std::fs::read_to_string(settings.path(name)).unwrap_or_default();
    let ends_blank = text.ends_with(|c: char| c.is_whitespace());
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut vals = Vec::with_capacity(words.len());
    for w in &words {
        match w.parse::<f64>() {
            Ok(v) => vals.push(v),
            Err(_) => {
                return Err(End::Refuse(format!(
                    "larmorx: {name} holds a word that is not a number ({w}): FSL reads such a file for ever (FSL-72); refused"
                )));
            }
        }
    }
    let mut rows: Vec<Vec<f64>> = vals.chunks_exact(width).map(<[f64]>::to_vec).collect();
    if vals.len() % width == 0 && !ends_blank && !rows.is_empty() {
        rows.pop();
    }
    Ok(rows)
}

/// `setoption` (§7.3).
fn set_option(st: &mut State<'_>, w: &[String], err: &mut dyn Write) -> Result<(), End> {
    let vals: Vec<f64> = w[2..].iter().map(|x| st.scalar(x)).collect();
    match w[1].as_str() {
        "smoothing" => {
            st.smoothing = vals[0] as f32;
            rebuild_pair(st, |p| p.smoothing = vals[0] as f32);
        }
        "fuzzyfraction" => {
            st.fuzzy = vals[0] as f32;
            rebuild_pair(st, |p| p.fuzzy = vals[0] as f32);
        }
        "optimisationtype" => st.opt.powell = w[2] == "powell",
        "costfunction" => match Cost::from_name(&w[2]) {
            Some(c) => {
                st.cost = c;
                st.search_cost = c;
            }
            None => {
                let _ = writeln!(err, "Unrecognised cost function: {}", w[2]);
                return Err(End::Exit(255));
            }
        },
        "tolerance" => {
            st.opt.tolerances = vals.iter().map(|&t| t as f32 / st.scale).collect();
        }
        "rescaletolerance" => {
            if vals.len() != 1 {
                return Err(End::Error("Mat::operator(): index out of bounds".into()));
            }
            let k = vals[0] as f32;
            for t in &mut st.opt.tolerances {
                *t *= k;
            }
        }
        "boundguess" => st.opt.bound_guess = vals.iter().map(|&v| v as f32).collect(),
        "nosubset" => st.subset = None,
        "paramsubset" => {
            let nc = schedule::int_scalar(vals[0]).max(0) as usize;
            if vals.len() < 1 + 12 * nc {
                let _ = writeln!(
                    err,
                    "Must specify at least {} for a {nc} parameter subset mask",
                    12 * nc
                );
                return Ok(());
            }
            let cols: Vec<[f64; 12]> = (0..nc)
                .map(|c| {
                    let mut col = [0.0; 12];
                    col.copy_from_slice(&vals[1 + 12 * c..1 + 12 * (c + 1)]);
                    col
                })
                .collect();
            st.subset = Some(Subset::new(&cols).ok_or_else(|| {
                End::Refuse("larmorx: a parameter subset with a value that is not a number: FSL's decomposition fails; refused".into())
            })?);
        }
        "minsampling" => st.min_sampling = vals[0] as f32,
        "bbrstep" => {
            let k = vals[0] as i64;
            st.bbr_step = k;
            if let Some(p) = &st.pair {
                p.bbr_step
                    .store(k.max(0) as usize, std::sync::atomic::Ordering::Relaxed);
            }
        }
        other => {
            let _ = writeln!(err, "Option {other} is unrecognised - ignoring");
        }
    }
    Ok(())
}

/// The current pair with a setting changed (a pair is shared with no one else while the
/// schedule runs, so it is changed in place when it can be).
fn rebuild_pair(st: &mut State<'_>, f: impl FnOnce(&mut Pair)) {
    if let Some(m) = st.pair.as_mut().and_then(Arc::get_mut) {
        f(m);
    }
}

/// `setscale s [force]` (§7.2).
fn set_scale(st: &mut State<'_>, s: f32, force: bool) -> Result<(), End> {
    if s > 0.0 {
        st.scale = s;
        st.opt.scale = s;
    }
    let s = st.scale;
    let o = st.o;
    let input = st.input0.clone();
    let inw = st.in_weight0.clone();
    if force || o.force_scaling || st.min_sampling <= 1.25 * s {
        // the input blurred to the scale (also with -noresample, observed)
        let (inp, w) = match &inw {
            Some(wt) => {
                let (a, b) = pyramid::weighted(&input, wt, |v: &Volume| pyramid::blur(v, s));
                (a, Some(b))
            }
            None => (pyramid::blur(&input, s), None),
        };
        let level = [(1.0f32, 0usize), (2.0, 1), (4.0, 2), (8.0, 3)]
            .iter()
            .find(|(l, i)| (s - l).abs() < 0.01 && !(*i == 0 && st.one_replaced))
            .map(|&(_, i)| i);
        let (refl, refw) = match level {
            Some(i) => (st.levels[i].clone(), st.level_weights[i].clone()),
            None => {
                let f = |v: &Volume| pyramid::isotropic(&pyramid::blur(v, s), s);
                let (a, b) = match &st.ref_weight {
                    Some(w) => {
                        let (a, b) = pyramid::weighted(&st.reference, w, f);
                        (a, Some(b))
                    }
                    None => (f(&st.reference), None),
                };
                st.levels[0] = a.clone();
                st.level_weights[0] = b.clone();
                st.one_replaced = true;
                (a, b)
            }
        };
        let bins = (st.o.bins as f32 / s) as i32;
        let refusal = format!(
            "larmorx: -bins {} gives {bins} bins at the {s} mm scale, which no histogram can have (FSL-21, FSL-66: FSL's result is undefined, it aborts or crashes); refused",
            st.o.bins
        );
        // negative bins: FSL prepares them for every cost (FSL-66); none: refused where a
        // histogram cost is evaluated (FSL-21)
        if bins < 0 {
            return Err(End::Refuse(refusal));
        }
        let skip = st.ctx.mode.fix() && (inp.has_non_finite() || refl.has_non_finite());
        if cog_total(&inp) < 1e-5 {
            st.log.push("WARNING::in calculating COG, total = 0.0");
        }
        let mut pair = Pair::new(
            refl,
            refw,
            inp,
            w,
            bins.max(1) as usize,
            st.cost,
            st.smoothing,
            st.fuzzy,
            skip,
            st.init,
        );
        pair.bbr_source = Some(st.bbr.clone());
        pair.bbr_step = std::sync::atomic::AtomicUsize::new(st.bbr_step.max(0) as usize);
        if bins == 0 {
            pair.no_bins = Some(refusal);
        }
        if st.bbr_step < 1 && (st.cost == Cost::Bbr || st.search_cost == Cost::Bbr) {
            // checked again at use: FSL never finishes there (FSL-71)
        }
        st.pair = Some(Arc::new(pair));
    } else if let Some(p) = st.pair.take() {
        // only the input reset (§7.2)
        let old = Arc::try_unwrap(p).unwrap_or_else(|a| clone_pair(&a));
        let skip = st.ctx.mode.fix() && (input.has_non_finite() || old.reference.has_non_finite());
        let no_bins = old.no_bins.clone();
        let centre = old.centre;
        let mut pair = Pair::new(
            old.reference,
            old.ref_weight,
            input,
            inw,
            old.bins,
            st.cost,
            st.smoothing,
            st.fuzzy,
            skip,
            st.init,
        );
        pair.bbr_source = Some(st.bbr.clone());
        pair.bbr_step = std::sync::atomic::AtomicUsize::new(st.bbr_step.max(0) as usize);
        pair.no_bins = no_bins;
        // the existing pair keeps its centre, the previous input's centre of mass (observed:
        // the costs of a transformed matrix are FSL's only so)
        pair.centre = centre;
        st.pair = Some(Arc::new(pair));
    }
    Ok(())
}

/// The total of a centre of mass (`Σ (v − v_min)`, §6.1).
fn cog_total(v: &Volume) -> f64 {
    let m = v.min();
    v.data.iter().map(|&x| f64::from(x - m)).sum()
}

fn clone_pair(p: &Pair) -> Pair {
    Pair::new(
        p.reference.clone(),
        p.ref_weight.clone(),
        p.input.clone(),
        p.in_weight.clone(),
        p.bins,
        p.cost,
        p.smoothing,
        p.fuzzy,
        p.skip_non_finite,
        p.init,
    )
}

fn check_bbr_step(st: &State<'_>, cost: Cost) -> Result<(), End> {
    if cost == Cost::Bbr && st.bbr_step < 1 {
        return Err(End::Refuse(format!(
            "larmorx: setoption bbrstep {} with the boundary-based cost: FSL's loop over the boundary points never ends (FSL-71); refused",
            st.bbr_step
        )));
    }
    Ok(())
}

/// One optimisation of a row (§8.2) with the main cost.
fn optimise_row(st: &State<'_>, m: &Mat4, dof: usize, maxit: usize) -> Result<(Mat4, f32), End> {
    check_bbr_step(st, st.cost)?;
    let pair = st.pair()?.clone();
    let log = st.log.clone();
    let cost = st.cost;
    let eval = move |t: &Mat4| evaluate(&pair, t, cost, &log);
    let centre = st.pair()?.centre;
    optimise::optimise(&eval, m, centre, dof, maxit, &st.opt, st.subset.as_ref())
        .map_err(End::Error)
}

/// The search (§8.1), appending to `S` and `P`.
fn run_search(st: &mut State<'_>, dof: usize) -> Result<(), End> {
    check_bbr_step(st, st.search_cost)?;
    let pair = st.pair()?.clone();
    let log = st.log.clone();
    let cost = st.search_cost;
    let eval = |t: &Mat4| evaluate(&pair, t, cost, &log);
    // the search measures both centres again (observed: one more warning for an empty
    // reference or input level)
    for v in [&pair.reference, &pair.input] {
        if cog_total(v) < 1e-5 {
            st.log.push("WARNING::in calculating COG, total = 0.0");
        }
    }
    let c_ref = pair.reference.centre_of_mass();
    let c_in = pair.centre;
    let mc = mat::apply(&st.init, [c_in[0], c_in[1], c_in[2], 1.0]);
    let mut rp = rigid::IDENTITY_PARAMS;
    for k in 0..3 {
        rp[3 + k] = c_ref[k] - mc[k];
    }
    let ranges = st.o.search;
    let r = Ranges {
        ranges,
        coarse: st.o.coarse,
        fine: st.o.fine,
    };
    let min_voxel = pair
        .reference
        .voxel_size
        .iter()
        .map(|d| d.abs())
        .fold(f32::INFINITY, f32::min);
    let found = search::search(
        &eval,
        c_in,
        rp,
        dof,
        dof.min(st.maxdof),
        &r,
        &st.opt,
        min_voxel,
        true,
    )
    .map_err(End::Error)?;
    st.check()?;
    for (c, m) in found.results {
        st.push(
            Var::S,
            Row {
                cost: f64::from(c),
                m,
            },
        );
    }
    for (c, m) in found.starts {
        st.push(
            Var::P,
            Row {
                cost: f64::from(c),
                m,
            },
        );
    }
    Ok(())
}

/// The output data type (§9.2) of an input stored as `stored` (`scaled`: a scaled file).
fn output_type(
    o: &Options,
    stored: DataType,
    scaled: bool,
    values: &[f32],
    mode: UpstreamDefects,
) -> DataType {
    if let Some(t) = o.datatype {
        return match t {
            OutType::Char => DataType::U8,
            OutType::Short => DataType::I16,
            OutType::Int => DataType::I32,
            OutType::Float => DataType::F32,
            OutType::Double => DataType::F64,
        };
    }
    let t = if scaled {
        DataType::F32
    } else {
        match stored {
            DataType::U8 => DataType::U8,
            DataType::I8 => {
                if mode.fix() {
                    DataType::I8
                } else {
                    DataType::U8
                }
            }
            DataType::I16 => DataType::I16,
            DataType::U16 | DataType::I32 | DataType::U32 | DataType::I64 | DataType::U64 => {
                DataType::I32
            }
            DataType::F64 => DataType::F64,
            _ => DataType::F32,
        }
    };
    if larmorx_mri::hmc::image::integer_range(t).is_some() {
        let (lo, hi) = values
            .iter()
            .filter(|v| v.is_finite())
            .fold((f32::INFINITY, f32::NEG_INFINITY), |(l, h), &x| {
                (l.min(x), h.max(x))
            });
        if hi - lo < 1.5 {
            return DataType::F32;
        }
    }
    t
}

/// The input's stored type and whether its file scales its values.
fn stored_type(h: &NiftiHeader) -> (DataType, bool) {
    let t = h.data_type().unwrap_or(DataType::F32);
    (t, larmorx_mri::hmc::image::scaling(h).is_some())
}

/// Writes `values` (radiological order, every volume) like the reference, in type `t`.
#[allow(clippy::too_many_arguments)]
fn write_image(
    ctx: &Ctx<'_>,
    stem: &str,
    reference: &Input,
    shape: [usize; 3],
    volumes: usize,
    tr: Option<f64>,
    values: Vec<f32>,
    t: DataType,
    err: &mut dyn Write,
    notices: &mut Vec<Notice>,
) -> Result<(), End> {
    let mode = ctx.mode;
    let ot = match ctx.settings.get("FSLOUTPUTTYPE") {
        Some(v) => v.to_owned(),
        None => {
            if mode.reproduce() {
                let _ = err.write_all(crate::cli::bet2::FSLOUTPUTTYPE_UNSET.as_bytes());
                return Err(End::Exit(1));
            }
            "NIFTI_GZ".to_owned()
        }
    };
    if !image::OUTPUT_TYPES.contains(&ot.as_str()) {
        let _ = writeln!(err, "{}", image::unknown_output_type_message(&ot));
        return Err(End::Exit(1));
    }
    let full = ctx.settings.name(stem);
    let Some((name, version)) = image::output_file(&full, &ot) else {
        return Err(End::Exit(1));
    };
    let mut template = reference.header.clone();
    // FSL's image layer writes the sform's negative zeros as zeros (observed)
    for row in [
        &mut template.srow_x,
        &mut template.srow_y,
        &mut template.srow_z,
    ] {
        for v in row.iter_mut() {
            *v += 0.0;
        }
    }
    if let Some(tr) = tr {
        template.pixdim[4] = tr;
    }
    let o = Output {
        template: &template,
        flipped: reference.flipped,
        shape,
        volumes,
        cal: None,
        extensions: false,
        defects: mode,
        n_threads: ctx.n_threads,
        descrip: "larmorx fsl flirt",
    };
    let v = match larmorx_mri::hmc::image::integer_range(t) {
        Some((lo, hi)) => {
            let (data, conv) =
                larmorx_mri::hmc::image::convert_values(&values, mode, lo, hi, |i| i as f64, |f| f);
            if conv.rounded + conv.out_of_range > 0 {
                defects::push_once(
                    notices,
                    larmorx_mri::hmc::image::fsl09_notice(
                        std::path::Path::new(&name),
                        conv,
                        t,
                        mode,
                    ),
                );
            }
            match t {
                DataType::U8 => Values::U8(data.iter().map(|&x| x as i64 as u8).collect()),
                DataType::I8 => Values::I8(data.iter().map(|&x| x as i64 as i8).collect()),
                DataType::I16 => Values::I16(data.iter().map(|&x| x as i64 as i16).collect()),
                _ => Values::I32(data.iter().map(|&x| x as i64 as i32).collect()),
            }
        }
        None if t == DataType::F64 => Values::F64(values.iter().map(|&x| f64::from(x)).collect()),
        None => Values::F32(values),
    };
    match image::write(&name, version, &o, v) {
        Ok(n) => {
            for x in n {
                defects::push_once(notices, x);
            }
            // FLIRT's pair headers carry NIfTI's four extension bytes (observed: 352 bytes)
            if name.ends_with(".hdr")
                && let Ok(mut bytes) = std::fs::read(&name)
                && bytes.len() == 348
            {
                bytes.extend_from_slice(&[0, 0, 0, 0]);
                let _ = std::fs::write(&name, bytes);
            }
            Ok(())
        }
        Err(e) => Err(End::Error(format!("{name}: {e}"))),
    }
}

/// The background value of the output's input (§10a): refused for a single voxel.
fn pad_value(v: &Volume, ctx: &Ctx<'_>, notices: &mut Vec<Notice>) -> Result<f32, End> {
    match pyramid::background(v, ctx.mode) {
        Some(b) => {
            for n in pyramid::background_notices(&b, "the input's border", ctx.mode) {
                defects::push_once(notices, n);
            }
            Ok(b.value)
        }
        None => Err(End::Refuse(format!(
            "larmorx: the input is {} x {} x {} voxels: its background value (the 10th percentile of its border) has no border to read (FSL reads an empty buffer, FSL-75); refused",
            v.shape[0], v.shape[1], v.shape[2]
        ))),
    }
}

/// The registered input resampled onto the reference (§9.2).
#[allow(clippy::too_many_arguments)]
fn write_registered(
    o: &Options,
    ctx: &Ctx<'_>,
    reference: &Image,
    input: &Image,
    m: &Mat4,
    stem: &str,
    err: &mut dyn Write,
    notices: &mut Vec<Notice>,
) -> Result<(), End> {
    let src = &input.vols[0];
    let refv = &reference.vols[0];
    let values = match (&o.fieldmap, o.pedir) {
        (Some(fname), d) if d != 0 => {
            // the boundary-based mapping with the field map (§6.5), from unscaled images
            let f = read(ctx, fname, false, notices)?;
            let fm = triple(&f.vols[0]);
            let mask = match &o.fieldmap_mask {
                Some(mn) => triple(&read(ctx, mn, false, notices)?.vols[0]),
                None => Volume::new(
                    fm.shape,
                    fm.voxel_size,
                    fm.data.iter().map(|&v| 0.0 * v + 1.0).collect(),
                ),
            };
            let srcb = BbrSource {
                fieldmap: Some((fm, mask)),
                pedir: o.pedir,
                echo_spacing: o.echo_spacing,
                ..BbrSource::default()
            };
            let sh = super::bbr::field_shift(&srcb, m, src.shape, refv.voxel_size)
                .ok_or_else(|| End::Error("the field map could not be used".into()))?;
            let pad = match o.background {
                Some(b) => b,
                None => pad_value(src, ctx, notices)?,
            };
            output::resample_shifted(src, m, refv.shape, refv.voxel_size, pad, &|r| {
                (sh.axis, sh.at(r))
            })
            .ok_or_else(|| End::Error(mat::SINGULAR.into()))?
        }
        _ => resample_one(o, ctx, src, refv, m, None, notices)?,
    };
    let (stored, scaled) = stored_type(&input.input.header);
    let t = output_type(o, stored, scaled, &values, ctx.mode);
    write_image(
        ctx,
        stem,
        &reference.input,
        refv.shape,
        1,
        None,
        values,
        t,
        err,
        notices,
    )
}

/// One volume resampled as §9.2 says; `pad` the padding value when given (else the blurred
/// input's background value).
fn resample_one(
    o: &Options,
    ctx: &Ctx<'_>,
    src: &Volume,
    refv: &Volume,
    m: &Mat4,
    pad: Option<f32>,
    notices: &mut Vec<Notice>,
) -> Result<Vec<f32>, End> {
    let ref_min = refv
        .voxel_size
        .iter()
        .map(|d| d.abs())
        .fold(f32::INFINITY, f32::min);
    let blur = o.interp != Interp::NearestNeighbour && !o.no_resample_blur;
    let raw_pad = match (o.background, pad) {
        (Some(b), _) => b,
        (None, Some(p)) => p,
        (None, None) => pad_value(src, ctx, notices)?,
    };
    let blurred = if blur {
        output::blur_for_output(src, ref_min, raw_pad)
    } else {
        src.clone()
    };
    let pad_v = match (o.background, pad) {
        (Some(b), _) => b,
        (None, Some(p)) => p,
        (None, None) => {
            if blur {
                pad_value(&blurred, ctx, notices)?
            } else {
                raw_pad
            }
        }
    };
    let kernel = (o.interp == Interp::Sinc).then(|| SincKernel::new(o.sinc_width, o.sinc_window));
    let padding = if o.two_d {
        o.padding.max(1.0)
    } else {
        o.padding
    };
    let r = Resampling {
        interp: o.interp,
        sinc: kernel.as_ref(),
        pad: pad_v,
        padding,
    };
    output::resample(&blurred, m, refv.shape, refv.voxel_size, &r)
        .ok_or_else(|| End::Error(mat::SINGULAR.into()))
}

/// `-applyxfm` and `-applyisoxfm` (§9.3).
#[allow(clippy::too_many_arguments)]
fn apply(
    o: &Options,
    ctx: &Ctx<'_>,
    reference: &Image,
    input: &Image,
    init: &Mat4,
    _out: &mut dyn Write,
    err: &mut dyn Write,
    notices: &mut Vec<Notice>,
) -> Result<(), End> {
    let mut refv = reference.vols[0].clone();
    let mut ref_input = reference.input.clone();
    if let Some(s) = o.apply_iso {
        refv = pyramid::isotropic(&pyramid::blur(&refv, s), s);
        // the header follows the resampling: the voxel sizes and the dimensions
        // in FSL's memory order (x reversed for a positive determinant): `W' = W·F_n·D·F_n'`
        // with `D` the scaling `s/d` (§5), so the x offset follows the new grid (observed)
        let h = &mut ref_input.header;
        let old = [h.pixdim[1], h.pixdim[2], h.pixdim[3]];
        let flip = |n: usize| -> Mat4 {
            let mut f = IDENTITY;
            if reference.input.flipped {
                f[0][0] = -1.0;
                f[0][3] = n as f64 - 1.0;
            }
            f
        };
        let scale = mat::diagonal([0, 1, 2].map(|j| {
            if old[j] != 0.0 {
                f64::from(s) / old[j].abs()
            } else {
                1.0
            }
        }));
        let step = mat::mul(
            &mat::mul(&flip(reference.vols[0].shape[0]), &scale),
            &flip(refv.shape[0]),
        );
        // an unset sform keeps the reference's rows but for the x flips of the two grids
        // (`W·F_n·F_n'`, no scaling; observed)
        let srow = [h.srow_x, h.srow_y, h.srow_z, [0.0, 0.0, 0.0, 1.0]];
        let new_s = if h.sform_code > 0 {
            mat::mul(&srow, &step)
        } else {
            mat::mul(
                &mat::mul(&srow, &flip(reference.vols[0].shape[0])),
                &flip(refv.shape[0]),
            )
        };
        h.srow_x = new_s[0];
        h.srow_y = new_s[1];
        h.srow_z = new_s[2];
        if h.qform_code > 0
            && let Ok(q) = h.qform()
        {
            let nq = mat::mul(q.rows(), &step);
            h.qoffset_x = nq[0][3];
            h.qoffset_y = nq[1][3];
            h.qoffset_z = nq[2][3];
        }
        h.pixdim[1] = f64::from(s);
        h.pixdim[2] = f64::from(s);
        h.pixdim[3] = f64::from(s);
        ref_input.vol.shape = refv.shape;
    }
    // the image first, then the matrix (observed: an output type FSL cannot write ends the run
    // before `-omat` is written)
    let write_omat = || {
        if let Some(name) = o.omat.as_ref().filter(|n| !n.is_empty()) {
            let _ = std::fs::write(ctx.settings.path(name), mat::file_text(init));
        }
    };
    let Some(stem) = &o.out else {
        write_omat();
        return Ok(());
    };
    let first_pad = match o.background {
        Some(b) => b,
        None => pad_value(&input.vols[0], ctx, notices)?,
    };
    let mut values = Vec::with_capacity(refv.len() * input.vols.len());
    for v in &input.vols {
        let r = resample_one(o, ctx, v, &refv, init, Some(first_pad), notices)?;
        values.extend(r);
    }
    let (stored, scaled) = stored_type(&input.input.header);
    let t = output_type(o, stored, scaled, &values, ctx.mode);
    let tr = (input.vols.len() > 1).then(|| input.input.header.pixdim[4]);
    write_image(
        ctx,
        stem,
        &ref_input,
        refv.shape,
        input.vols.len(),
        tr,
        values,
        t,
        err,
        notices,
    )?;
    write_omat();
    Ok(())
}
