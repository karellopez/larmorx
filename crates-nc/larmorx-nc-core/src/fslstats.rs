// SPDX-License-Identifier: LicenseRef-FSL
// Translated to Rust from FSL 6.0.7.23: fsl-avwutils 2209.6 (fslstats.cc: print_usage,
// nonzerocount, nonzeromean, nonzerostddev, generateNonZeroMask, generate_masks,
// fmrib_main_float, main), with fsl-newimage 2601.0's statistics it calls (newimage.cc: mean,
// variance, min, max with coordinates, robustlimits, percentile, histogram, cog) and
// fsl-armawrap 0.7.0's matrix printing (operator_stream.hpp). Copyright (C) 2003-2009 University
// of Oxford (Mark Jenkinson, Matthew Webster). Rust translation Copyright 2026 Karel Lopez
// Vilaret; modified from the originals. Distributed under the FSL Licence (see LICENSE).
//! `fslstats [preoptions] <input> [options]`: statistics of an image, the options applied in
//! order.
//!
//! - **Preoptions**: `-t` (a line, or a JSON entry, per volume of a 4D image), `-K <index
//!   mask>` (one per label 1 to the mask's maximum), `-json` (nlohmann's JSON at the end).
//! - **Options**: thresholds and masks (`-l`, `-u`, `-k`), transformations (`-a`, `-n`, `-d`)
//!   and outputs (`-r`, `-R`, `-e`, `-E`, `-v`, `-V`, `-m`, `-M`, `-s`, `-S`, `-w`, `-x`, `-X`,
//!   `-c`, `-C`, `-p`, `-P`, `-h`, `-H` and the hidden `-D`), printed fixed with 6 decimals,
//!   each followed by a blank, a line per volume and label.
//! - **Messages and statuses**: the usage (status 1), the arguments' messages (1, 2 or 3), an
//!   exception's text (status 255, `return -1`).
//! - **D13**: FSL-181 (NaN in the order statistics), FSL-182 (`-s` and `-S` without a mask),
//!   FSL-183 (`-t` with a 4D `-k` or `-d` image), FSL-184 (`-x`, `-X` of an empty mask: a
//!   crash), FSL-185 (`-e`, `-E` of an empty range), FSL-186 (`-H` without a range), FSL-187
//!   (histogram bins of values beyond `int`), FSL-188 (`-w` of an empty 4D image), FSL-189
//!   (`-json`'s voxel counts in single precision): corrected by default, reproduced with
//!   `LARMORX_UPSTREAM_DEFECTS=reproduce` (FSL-184's crash refused in both modes).
//!
//! BET's script runs `fslstats <image> -l 0.001 -C` through this program
//! ([`crate::avwutils::fslstats_cog`]).

use std::io::Write;

use larmorx_core::defects::{self, Defect, Notices, UpstreamDefects};

use crate::arma::{self, Mat};
use crate::cnum;
use crate::glibc_double;
use crate::json::Value;
use crate::settings::Settings;
use crate::sse;
use crate::stdsort;
use crate::volops::{self, Op};
use crate::volume::{self, FslError, Volume};
use crate::vstats;

/// The usage text (`print_usage`), on standard output.
pub const USAGE: &str = "Usage: fslstats [preoptions] <input> [options]

preoption -json: output in JSON format to standard out
preoption -t will give a separate output line for each 3D volume of a 4D timeseries
preoption -K < indexMask > will generate seperate n submasks from indexMask, for indexvalues 1..n where n is the maximum index value in indexMask, and generate statistics for each submask
Note - options are applied in order, e.g. -M -l 10 -M will report the non-zero mean, apply a threshold and then report the new nonzero mean

-l <lthresh> : set lower threshold
-u <uthresh> : set upper threshold
-r           : output <robust min intensity> <robust max intensity>
-R           : output <min intensity> <max intensity>
-e           : output mean entropy ; mean(-i*ln(i))
-E           : output mean entropy (of nonzero voxels)
-v           : output <voxels> <volume>
-V           : output <voxels> <volume> (for nonzero voxels)
-m           : output mean
-M           : output mean (for nonzero voxels)
-s           : output standard deviation
-S           : output standard deviation (for nonzero voxels)
-w           : output smallest ROI <xmin> <xsize> <ymin> <ysize> <zmin> <zsize> <tmin> <tsize> containing nonzero voxels
-x           : output co-ordinates of maximum voxel
-X           : output co-ordinates of minimum voxel
-c           : output centre-of-gravity (cog) in mm coordinates
-C           : output centre-of-gravity (cog) in voxel coordinates
-p <n>       : output nth percentile (n between 0 and 100)
-P <n>       : output nth percentile (for nonzero voxels)
-a           : use absolute values of all image intensities
-n           : treat NaN or Inf as zero for subsequent stats
-k <mask>    : use the specified image (filename) for masking - overrides lower and upper thresholds
-d <image>   : take the difference between the base image and the image specified here
-h <nbins>   : output a histogram (for the thresholded/masked voxels only) with nbins
-H <nbins> <min> <max>   : output a histogram (for the thresholded/masked voxels only) with nbins and histogram limits of min and max
Note - thresholds are not inclusive ie lthresh<allowed<uthresh
";

/// How the program runs: FSL's environment settings and D13's mode.
pub struct Run<'a> {
    pub fsl: &'a Settings,
    pub mode: UpstreamDefects,
}

/// How a run ends early: an exit status (what was printed already written), or an exception
/// (`main` catches every `std::exception`: its text, status 255).
#[derive(Debug)]
enum Stop {
    Exit(u8),
    Exception(FslError),
}

impl From<FslError> for Stop {
    fn from(e: FslError) -> Stop {
        Stop::Exception(e)
    }
}

type Res<T> = Result<T, Stop>;

struct Ctx<'a, 'w> {
    run: &'a Run<'a>,
    out: &'w mut dyn Write,
    err: &'w mut dyn Write,
    notices: Notices,
    /// With `-json`, the notices of the option being run: kept with its value, so that a value
    /// a later option of the same name replaces names nothing.
    collecting: Option<Vec<(&'static Defect, String)>>,
}

impl Ctx<'_, '_> {
    fn reproduce(&self) -> bool {
        self.run.mode.reproduce()
    }

    fn put(&mut self, s: &str) {
        let _ = self.out.write_all(s.as_bytes());
    }

    fn eput(&mut self, s: &str) {
        let _ = self.err.write_all(s.as_bytes());
    }

    fn notice(&mut self, d: &'static Defect, detail: impl Into<String>) {
        match &mut self.collecting {
            Some(v) => v.push((d, detail.into())),
            None => self.notices.add(d, detail),
        }
    }

    fn flush(&mut self, collected: Vec<(&'static Defect, String)>) {
        for (d, s) in collected {
            self.notices.add(d, s);
        }
    }

    /// A crash of FSL's (both modes refuse): the message and exit status 1.
    fn refuse(&mut self, d: &'static Defect, what: &str) -> Stop {
        self.notice(d, what);
        let line = format!(
            "larmorx-nc: error: fslstats: {what} ({}: FSL crashes here)\n",
            d.id
        );
        self.eput(&line);
        Stop::Exit(1)
    }

    /// An input the default refuses (rule B): the message and exit status 1.
    fn reject(&mut self, d: &'static Defect, what: &str) -> Stop {
        self.notice(d, what);
        let line = format!("larmorx-nc: error: fslstats: {what} ({})\n", d.id);
        self.eput(&line);
        Stop::Exit(1)
    }
}

/// `fslstats` with `argv` (`argv[0]` the program's name): the exit status. Exceptions are
/// caught as `main` catches them (the text of `what()`, status 255).
pub fn main(argv: &[String], run: &Run, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let mut ctx = Ctx {
        run,
        out,
        err,
        notices: Notices::new(run.mode),
        collecting: None,
    };
    let code = match main_inner(argv, &mut ctx) {
        Ok(c) | Err(Stop::Exit(c)) => c,
        Err(Stop::Exception(e)) => {
            let mut s = e.printed_text();
            if e.type_name.is_empty() {
                // the C library's abort: no exception to catch
                ctx.eput(&s);
                134
            } else {
                s.push_str(&e.what);
                s.push('\n');
                ctx.eput(&s);
                255
            }
        }
    };
    let lines: Vec<String> = ctx.notices.as_slice().iter().map(|n| n.line()).collect();
    for l in lines {
        ctx.eput(&format!("larmorx-nc: {l}\n"));
    }
    let _ = ctx.out.flush();
    let _ = ctx.err.flush();
    code
}

fn main_inner(argv: &[String], ctx: &mut Ctx) -> Res<u8> {
    let mut first = 0usize;
    let mut argc = argv.len();
    let (mut ts, mut json) = (false, false);
    let mut index_name = String::new();
    while argc > 2 && matches!(argv[first + 1].as_str(), "-t" | "-K" | "-json") {
        match argv[first + 1].as_str() {
            "-t" => ts = true,
            "-json" => json = true,
            _ => {
                index_name = argv[first + 2].clone();
                first += 1;
                argc -= 1;
            }
        }
        first += 1;
        argc -= 1;
    }
    if argc < 3 {
        ctx.put(USAGE);
        return Ok(1);
    }
    fmrib_main_float(&argv[first..first + argc], ts, &index_name, json, ctx)
}

/// What an option prints: the text (each number followed by a blank) and its JSON value.
#[derive(Clone, Debug, PartialEq)]
struct Out {
    text: String,
    json: Value,
}

fn f6(v: f64) -> String {
    cnum::fmt_f(v, 6)
}

fn num(v: f64) -> Out {
    Out {
        text: format!("{} ", f6(v)),
        json: Value::Float(v),
    }
}

fn nums(v: &[f64]) -> Out {
    let text = v.iter().map(|x| format!("{} ", f6(*x))).collect();
    Out {
        text,
        json: Value::floats(v),
    }
}

fn ints(v: &[i64]) -> Out {
    let text = v.iter().map(|x| format!("{x} ")).collect();
    Out {
        text,
        json: Value::ints(v),
    }
}

/// A `ColumnVector` printed by armawrap (each value, a blank and a newline) and then the
/// option's blank.
fn column(v: &[f64]) -> Out {
    let mut text: String = v.iter().map(|x| format!("{} \n", f6(*x))).collect();
    text.push(' ');
    Out {
        text,
        json: Value::floats(v),
    }
}

/// The state of one volume and label: the image, the masks (`nvoxels() > 0`: `Some`) and the
/// thresholds.
struct State {
    vol: Volume,
    mask: Option<Volume>,
    masknz: Option<Volume>,
    lthr: f32,
    uthr: f32,
}

fn fmrib_main_float(
    argv: &[String],
    ts: bool,
    index_name: &str,
    json: bool,
    ctx: &mut Ctx,
) -> Res<u8> {
    let fsl = ctx.run.fsl;
    let argc = argv.len();
    let mut master: Option<Volume> = None;
    let mut base: Option<Volume> = None;
    if ts || !index_name.is_empty() {
        master = Some(volume::read_volume4d::<f32>(&argv[1], fsl)?.0);
    } else {
        base = Some(volume::read_volume4d::<f32>(&argv[1], fsl)?.0);
    }
    let index_mask = if index_name.is_empty() {
        None
    } else {
        Some(volume::read_volume4d::<i32>(index_name, fsl)?.0)
    };
    let n_tp = if ts {
        master.as_ref().map_or(1, |m| m.tsize())
    } else {
        1
    };
    let n_idx = match &index_mask {
        Some(im) => i64::from(im.extrema_masked(None)?.0.0),
        None => 1,
    };
    let mut js = Value::object();
    js.set("volumes", Value::array());
    for timepoint in 0..n_tp {
        let mut tp = Value::object();
        tp.set("indices", Value::array());
        let mut index = 1i64;
        while index <= n_idx {
            let mut idx = Value::object();
            // without -t or -K the image is read straight into `vol` (one pass of the loops)
            let vol = match (base.take(), ts) {
                (Some(b), _) => b,
                (None, true) => master.as_ref().unwrap().sub_volume(timepoint)?,
                (None, false) => Volume::new(0, 0, 0, 0),
            };
            let mut st = State {
                vol,
                mask: None,
                masknz: None,
                lthr: -f32::MAX,
                uthr: f32::MAX,
            };
            if let Some(im) = &index_mask {
                if im.dimensionality() > 3 {
                    ctx.eput("Index mask must be 3D\n");
                    return Ok(3);
                }
                let mut m = Volume::<f32>::copyconvert(im);
                m.binarise(
                    (index - 1) as i32 as f32,
                    (index + 1) as i32 as f32,
                    false,
                    false,
                );
                // vol = indexMaster * mask (indexMaster: the time point's volume with -t)
                st.vol = if ts {
                    mul_volumes(&st.vol, &m)?
                } else {
                    mul_volumes(master.as_ref().unwrap(), &m)?
                };
                if m.extrema_masked(None)?.0.0 < 1.0 {
                    ctx.put(&format!("missing label: {index} in mask\n"));
                    index += 1;
                    continue;
                }
                st.masknz = Some(nonzero_mask(&st.vol, &m)?);
                st.mask = Some(m);
            }
            let mut narg = 2usize;
            let mut pending = std::collections::BTreeMap::new();
            while narg < argc {
                let sarg = argv[narg].as_str();
                if json {
                    ctx.collecting = Some(Vec::new());
                }
                let r = option(ctx, &mut st, argv, &mut narg, timepoint, ts, json, &master);
                let collected = ctx.collecting.take().unwrap_or_default();
                match r {
                    Err(e) => {
                        ctx.flush(collected);
                        return Err(e);
                    }
                    Ok(Some(o)) if json => {
                        idx.set(sarg, o.json);
                        pending.insert(sarg.to_owned(), collected);
                    }
                    Ok(Some(o)) => ctx.put(&o.text),
                    Ok(None) => ctx.flush(collected),
                }
                narg += 1;
            }
            for (_, collected) in pending {
                ctx.flush(collected);
            }
            if let Some(a) = tp.get_mut("indices") {
                a.push(idx);
            }
            if !json {
                ctx.put("\n");
            }
            index += 1;
        }
        if let Some(a) = js.get_mut("volumes") {
            a.push(tp);
        }
    }
    if json {
        let text = format!("{}\n", js.dump(4));
        ctx.put(&text);
    }
    Ok(0)
}

/// `v1 * v2` (newimage's `operator*`): a copy of the one of higher dimensionality multiplied
/// by the other, cycled.
fn mul_volumes(a: &Volume, b: &Volume) -> Res<Volume> {
    let (mut tmp, other) = if a.dimensionality() >= b.dimensionality() {
        (a.clone(), b)
    } else {
        (b.clone(), a)
    };
    let _ = tmp.op_volume(Op::Mul, other)?;
    Ok(tmp)
}

/// `generateNonZeroMask(mask, masknz, input)`: `binarise(input, 0, 0, inclusive, true) * mask`
/// (NaN counts as nonzero).
fn nonzero_mask(input: &Volume, mask: &Volume) -> Res<Volume> {
    let mut nz = input.clone();
    nz.binarise(0.0, 0.0, true, true);
    mul_volumes(&nz, mask)
}

/// `generate_masks(mask, masknz, vol, lthr, uthr)` then `vol *= mask`.
fn threshold_masks(st: &mut State) -> Res<()> {
    let mut m = st.vol.clone();
    m.binarise(st.lthr, st.uthr, false, false);
    st.masknz = Some(nonzero_mask(&st.vol, &m)?);
    let _ = st.vol.op_volume(Op::Mul, &m)?;
    st.mask = Some(m);
    Ok(())
}

/// `maskedIterator`'s check: `samesize(vol, mask, SUBSET)` or the runtime_error.
fn check_mask(vol: &Volume, mask: Option<&Volume>, source: &str) -> Res<()> {
    if let Some(m) = mask
        && !m.data.is_empty()
        && !volops::samesize_subset(vol, m)
    {
        return Err(Stop::Exception(FslError {
            printed: Vec::new(),
            type_name: "std::runtime_error",
            what: format!("{source}maskedIterator: mask and volume must be the same size"),
        }));
    }
    Ok(())
}

fn mask_data(m: Option<&Volume>) -> &[f32] {
    m.map_or(&[], |m| m.data.as_slice())
}

/// The voxels of `mask` (every voxel without one) whose value is not NaN, as a mask; `None`
/// when no masked value is NaN (FSL-181 does not apply).
fn nan_free(vol: &Volume, mask: Option<&Volume>) -> Option<Volume> {
    let m = mask_data(mask);
    let any = vol
        .data
        .iter()
        .enumerate()
        .any(|(k, v)| v.is_nan() && vstats::in_mask(m, k));
    if !any {
        return None;
    }
    let mut keep = vol.clone();
    for (k, d) in keep.data.iter_mut().enumerate() {
        let inside = vstats::in_mask(m, k) && !vol.data[k].is_nan();
        *d = if inside { 1.0 } else { 0.0 };
    }
    Some(keep)
}

/// `ptrToCoord(offset)`'s x, y and z.
fn coords(v: &Volume, offset: usize) -> [i64; 3] {
    let o = offset as i64;
    let (xs, ys, zs) = (v.xsize(), v.ysize(), v.zsize());
    [o % xs, (o / xs) % ys, (o / (xs * ys)) % zs]
}

/// `niftivox2newimagevox_mat().i() * [x y z 1]` (Armadillo's `solve`), each coordinate.
fn to_nifti(v: &Volume, c: [f64; 3]) -> [f64; 3] {
    let m = v.niftivox2newimagevox_mat();
    arma::solve(&m, &Mat::col(&[c[0], c[1], c[2], 1.0]))
        .map(|r| [r.get(0), r.get(1), r.get(2)])
        .unwrap_or(c)
}

/// The value of an option's argument, or the program's message and exit status 2.
fn arg<'a>(ctx: &mut Ctx, argv: &'a [String], narg: &mut usize, msg: &str) -> Res<&'a str> {
    *narg += 1;
    if *narg >= argv.len() {
        ctx.eput(&format!("{msg}\n"));
        return Err(Stop::Exit(2));
    }
    Ok(argv[*narg].as_str())
}

/// Chooses between FSL's output and the corrected one, naming `d` where they differ in the
/// mode's output.
fn pick(ctx: &mut Ctx, json: bool, fsl: Out, fixed: Out, d: &'static Defect, what: &str) -> Out {
    let differ = if json {
        fsl.json != fixed.json
    } else {
        fsl.text != fixed.text
    };
    if !differ {
        return fsl;
    }
    ctx.notice(d, what);
    if ctx.reproduce() { fsl } else { fixed }
}

/// One option at `argv[*narg]`: what it prints (`None` for the options that print nothing).
#[allow(clippy::too_many_arguments)]
fn option(
    ctx: &mut Ctx,
    st: &mut State,
    argv: &[String],
    narg: &mut usize,
    timepoint: i64,
    ts: bool,
    json: bool,
    master: &Option<Volume>,
) -> Res<Option<Out>> {
    let fsl = ctx.run.fsl;
    let sarg = argv[*narg].as_str();
    let out = match sarg {
        "-n" => {
            for x in st.vol.data.iter_mut() {
                if !x.is_finite() {
                    *x = 0.0;
                }
            }
            None
        }
        "-m" => {
            if st.mask.is_some() {
                check_mask(&st.vol, st.mask.as_ref(), "mean: ")?;
                let (m, empty) = vstats::mean_masked(&st.vol.data, mask_data(st.mask.as_ref()));
                if empty {
                    ctx.eput("ERROR:: Empty mask image\n");
                }
                Some(num(m))
            } else {
                Some(num(vstats::mean(&st.vol.data)))
            }
        }
        "-M" => {
            if st.masknz.is_some() {
                check_mask(&st.vol, st.masknz.as_ref(), "mean: ")?;
                let (m, empty) = vstats::mean_masked(&st.vol.data, mask_data(st.masknz.as_ref()));
                if empty {
                    ctx.eput("ERROR:: Empty mask image\n");
                }
                Some(num(m))
            } else {
                Some(num(nonzero_mean(&st.vol.data)))
            }
        }
        "-X" | "-x" => Some(extremum_coords(ctx, st, sarg == "-x", json)?),
        "-w" => {
            if st.masknz.is_none() {
                threshold_masks(st)?;
            }
            Some(roi(ctx, st.masknz.as_ref().unwrap(), json))
        }
        "-e" | "-E" => {
            if st.mask.is_none() {
                threshold_masks(st)?;
            }
            let m = if sarg == "-e" {
                st.mask.as_ref()
            } else {
                st.masknz.as_ref()
            };
            Some(entropy(ctx, &st.vol, m, json, sarg)?)
        }
        "-k" => {
            let name = arg(ctx, argv, narg, "Must specify an argument to -k")?;
            let mut m = volume::read_volume4d::<f32>(name, fsl)?.0;
            let sizes = m.xsize() == st.vol.xsize()
                && m.ysize() == st.vol.ysize()
                && m.zsize() == st.vol.zsize();
            let series = master.as_ref().map_or(1, |v| v.tsize());
            if ts && m.tsize() != 1 && sizes && m.tsize() <= series {
                // FSL-183: FSL refuses the 4D mask; the time point's volume is meant
                let t = timepoint.min(m.tsize() - 1);
                ctx.notice(
                    &defects::FSL_183,
                    format!("-t -k: the mask's volume {t} used for time point {timepoint}"),
                );
                if !ctx.reproduce() {
                    m = m.sub_volume(t)?;
                }
            }
            if !sizes || m.tsize() > st.vol.tsize() {
                ctx.eput("Mask and image must be the same size\n");
                return Err(Stop::Exit(3));
            }
            if m.tsize() != st.vol.tsize() && m.tsize() != 1 {
                pad_volumes(&mut m, st.vol.tsize())?;
            }
            m.binarise(0.5, f32::MAX, true, false);
            st.masknz = Some(nonzero_mask(&st.vol, &m)?);
            let _ = st.vol.op_volume(Op::Mul, &m)?;
            st.mask = Some(m);
            None
        }
        "-d" => {
            let name = arg(ctx, argv, narg, "Must specify an argument to -d")?;
            let mut image2 = volume::read_volume4d::<f32>(name, fsl)?.0;
            let same = image2.xsize() == st.vol.xsize()
                && image2.ysize() == st.vol.ysize()
                && image2.zsize() == st.vol.zsize();
            if !same {
                ctx.eput("Image used with -d must be the same size as the base image\n");
                return Err(Stop::Exit(3));
            }
            if ts {
                let series = master.as_ref().map_or(1, |v| v.tsize());
                let fails = timepoint >= image2.tsize() || image2.tsize() > 1;
                if fails && image2.tsize() <= series {
                    // FSL-183: FSL stops (an index exception) or refuses the 4D image
                    let t = timepoint.min(image2.tsize() - 1);
                    ctx.notice(
                        &defects::FSL_183,
                        format!("-t -d: the image's volume {t} used for time point {timepoint}"),
                    );
                    if !ctx.reproduce() {
                        image2 = image2.sub_volume(t)?;
                    }
                }
                if ctx.reproduce() {
                    // image3 = image2[timepoint]: the index checked
                    image2.sub_volume(timepoint)?;
                }
            }
            if image2.tsize() > st.vol.tsize() {
                ctx.eput("Image must be the same size as the base image\n");
                return Err(Stop::Exit(3));
            }
            if image2.tsize() != st.vol.tsize() && image2.tsize() != 1 {
                pad_volumes(&mut image2, st.vol.tsize())?;
            }
            let _ = st.vol.op_volume(Op::Sub, &image2)?;
            None
        }
        "-l" | "-u" => {
            let what = if sarg == "-l" {
                "Must specify an argument to -l"
            } else {
                "Must specify an argument to -u"
            };
            let a = arg(ctx, argv, narg, what)?;
            let v = cnum::atof(a) as f32;
            if sarg == "-l" {
                st.lthr = v;
            } else {
                st.uthr = v;
            }
            threshold_masks(st)?;
            None
        }
        "-a" => {
            for x in st.vol.data.iter_mut() {
                *x = volops::abs_value(*x);
            }
            None
        }
        "-v" => Some(voxels(ctx, st, json, false)),
        "-V" => Some(voxels(ctx, st, json, true)),
        "-D" => Some(num(vstats::sums(&st.vol.data).0)),
        "-s" => {
            if st.mask.is_some() {
                check_mask(&st.vol, st.mask.as_ref(), "variance: ")?;
                let (v, lines) = vstats::variance_masked(&st.vol.data, mask_data(st.mask.as_ref()));
                for _ in 0..lines {
                    ctx.eput("ERROR:: Empty mask image\n");
                }
                Some(num(sse::sqrt_f64(v)))
            } else {
                let f = num(vstats::stddev(&st.vol.data, false));
                let x = num(vstats::stddev(&st.vol.data, true));
                Some(pick(
                    ctx,
                    json,
                    f,
                    x,
                    &defects::FSL_182,
                    "-s: the variance below 0 or of one value",
                ))
            }
        }
        "-S" => {
            if st.masknz.is_some() {
                check_mask(&st.vol, st.masknz.as_ref(), "variance: ")?;
                let (v, lines) =
                    vstats::variance_masked(&st.vol.data, mask_data(st.masknz.as_ref()));
                for _ in 0..lines {
                    ctx.eput("ERROR:: Empty mask image\n");
                }
                Some(num(sse::sqrt_f64(v)))
            } else {
                let f = num(nonzero_stddev(&st.vol.data));
                let d = &st.vol.data;
                let (v, _) = vstats::variance_where(d, |k| d[k] != 0.0);
                let x = num(sse::sqrt_f64(v));
                Some(pick(
                    ctx,
                    json,
                    f,
                    x,
                    &defects::FSL_182,
                    "-S: the nonzero voxels' squares in single precision",
                ))
            }
        }
        "-r" => Some(robust(ctx, st, json)?),
        "-R" => Some(range(ctx, st, json)?),
        "-c" | "-C" => {
            let mut warnings = Vec::new();
            let c = st.vol.calc_cog(&mut warnings);
            for w in warnings {
                ctx.eput(&format!("{w}\n"));
            }
            let r = if sarg == "-c" {
                let m = st.vol.newimagevox2mm_mat();
                let r = arma::mul(&m, &Mat::col(&[c[0], c[1], c[2], 1.0]), false);
                [r.get(0), r.get(1), r.get(2)]
            } else {
                to_nifti(&st.vol, c)
            };
            Some(nums(&r))
        }
        "-p" | "-P" => {
            let what = if sarg == "-p" {
                "Must specify an argument to -p"
            } else {
                "Must specify an argument to -P"
            };
            let a = arg(ctx, argv, narg, what)?;
            let n = cnum::atof(a) as f32;
            if n < 0.0 || n > 100.0 {
                ctx.eput("Percentile must be between 0 and 100\n");
                return Err(Stop::Exit(1));
            }
            if sarg == "-P" && st.mask.is_none() {
                threshold_masks(st)?;
            }
            let m = if sarg == "-p" {
                st.mask.as_ref()
            } else {
                st.masknz.as_ref()
            };
            let p = (f64::from(n) / 100.0) as f32;
            Some(percentile(ctx, &st.vol, m, p, json, sarg)?)
        }
        "-h" | "-H" => {
            let a = arg(ctx, argv, narg, "Must specify the number of bins")?;
            let nbins = cnum::f32_to_i32(cnum::atof(a) as f32);
            if nbins < 1 {
                ctx.eput("Must specify at least 1 bin\n");
                return Err(Stop::Exit(1));
            }
            let limits = if sarg == "-H" {
                let lo = arg(
                    ctx,
                    argv,
                    narg,
                    "Must specify the histogram minimum intensity",
                )?;
                let lo = cnum::atof(lo) as f32;
                let hi = arg(
                    ctx,
                    argv,
                    narg,
                    "Must specify the histogram maximum intensity",
                )?;
                let hi = cnum::atof(hi) as f32;
                if hi.partial_cmp(&lo) != Some(std::cmp::Ordering::Greater) {
                    // FSL-186: no histogram is printed
                    if !ctx.reproduce() {
                        return Err(ctx.reject(
                            &defects::FSL_186,
                            "-H: the histogram's maximum is not above its minimum",
                        ));
                    }
                    ctx.notice(&defects::FSL_186, "-H: no histogram printed");
                }
                Some((lo, hi))
            } else {
                None
            };
            Some(histogram(ctx, st, nbins, limits, json)?)
        }
        _ => {
            ctx.eput(&format!("Unrecognised option: {sarg}\n"));
            return Err(Stop::Exit(3));
        }
    };
    Ok(out)
}

/// `addvolume(v[v.tsize() - 1])` until `t` volumes.
fn pad_volumes(v: &mut Volume, t: i64) -> Res<()> {
    while v.tsize() < t {
        let last = v.sub_volume(v.tsize() - 1)?;
        v.data.extend_from_slice(&last.data);
        v.dims[3] += 1;
    }
    Ok(())
}

/// `nonzeromean(vol)`: the nonzero values (NaN included) added in order, divided by their
/// number.
fn nonzero_mean(values: &[f32]) -> f64 {
    let mut total = 0f64;
    let mut n: i64 = 0;
    for &v in values {
        if v != 0.0 {
            total = sse::add_f64(total, f64::from(v));
            n += 1;
        }
    }
    if n > 0 {
        total = sse::div_f64(total, n as f64);
    }
    total
}

/// `nonzerostddev(vol)`: the squares in single precision (`(double)((*it) * (*it))`), the
/// variance `(Σv² - n m²) / (n - 1)` (0 for one value), its square root (FSL-182).
fn nonzero_stddev(values: &[f32]) -> f64 {
    let (mut totv, mut totvv) = (0f64, 0f64);
    let mut n: i64 = 0;
    for &v in values {
        if v != 0.0 {
            totvv = sse::add_f64(totvv, f64::from(sse::mul_f32(v, v)));
            totv = sse::add_f64(totv, f64::from(v));
            n += 1;
        }
    }
    let mut var = 0f64;
    if n > 1 {
        let nf = n as f64;
        let m = sse::div_f64(totv, nf);
        let a = sse::mul_f64(sse::mul_f64(nf, m), m);
        var = sse::div_f64(sse::sub_f64(totvv, a), nf - 1.0);
    }
    sse::sqrt_f64(var)
}

/// `-x` (the maximum's coordinates) or `-X` (the minimum's): `vol.min(mask, coords)`, the
/// voxel mapped to the file's voxel coordinates and rounded.
fn extremum_coords(ctx: &mut Ctx, st: &State, max: bool, json: bool) -> Res<Out> {
    check_mask(&st.vol, st.mask.as_ref(), "calculateExtrema: ")?;
    let m = mask_data(st.mask.as_ref());
    let e = vstats::extrema_masked(&st.vol.data, m);
    let opt = if max { "-x" } else { "-X" };
    let Some(e) = e else {
        ctx.eput("calculateExtrema: mask is empty\n");
        return Err(ctx.refuse(
            &defects::FSL_184,
            &format!("{opt}: no voxel in the mask (the coordinates of none are read)"),
        ));
    };
    let at = |e: &vstats::Extrema<f32>| if max { e.imax } else { e.imin };
    let make = |k: usize| {
        let c = coords(&st.vol, k);
        let c = to_nifti(&st.vol, [c[0] as f64, c[1] as f64, c[2] as f64]);
        ints(&c.map(|x| i64::from(volume::round_f64(x))))
    };
    let fsl = make(at(&e));
    if st.vol.data[e.imax].is_nan()
        && let Some(keep) = nan_free(&st.vol, st.mask.as_ref())
    {
        // FSL-181: a NaN first value is the maximum and the minimum
        let fixed = match vstats::extrema_masked(&st.vol.data, &keep.data) {
            Some(f) => make(at(&f)),
            None => {
                if ctx.reproduce() {
                    ctx.notice(&defects::FSL_181, format!("{opt}: every value NaN"));
                    return Ok(fsl);
                }
                return Err(ctx.reject(&defects::FSL_181, &format!("{opt}: every value is NaN")));
            }
        };
        return Ok(pick(
            ctx,
            json,
            fsl,
            fixed,
            &defects::FSL_181,
            &format!("{opt}: the first value is NaN"),
        ));
    }
    Ok(fsl)
}

/// `-w`: the smallest box holding `masknz`'s voxels, in the file's voxel coordinates.
fn roi(ctx: &mut Ctx, masknz: &Volume, json: bool) -> Out {
    let (xs, ys, zs, tsz) = (
        masknz.xsize(),
        masknz.ysize(),
        masknz.zsize(),
        masknz.tsize(),
    );
    let (mut xmin, mut xmax, mut ymin, mut ymax) = (xs - 1, 0, ys - 1, 0);
    let (mut zmin, mut zmax, mut tmin, mut tmax) = (zs - 1, 0, tsz - 1, 0);
    let n3 = (xs * ys * zs) as usize;
    let mut any = false;
    for t in 0..tsz {
        for z in 0..zs {
            for y in 0..ys {
                for x in 0..xs {
                    let k = t as usize * n3 + ((z * ys + y) * xs + x) as usize;
                    if masknz.data[k] > 0.5 {
                        any = true;
                        xmin = xmin.min(x);
                        xmax = xmax.max(x);
                        ymin = ymin.min(y);
                        ymax = ymax.max(y);
                        zmin = zmin.min(z);
                        zmax = zmax.max(z);
                        tmin = tmin.min(t);
                        tmax = tmax.max(t);
                    }
                }
            }
        }
    }
    let lo = to_nifti(masknz, [xmin as f64, ymin as f64, zmin as f64]);
    let hi = to_nifti(masknz, [xmax as f64, ymax as f64, zmax as f64]);
    let r = |x: f64| i64::from(volume::round_f64(x));
    let (mut x0, mut y0, mut z0) = (r(lo[0]), r(lo[1]), r(lo[2]));
    let (mut x1, mut y1, mut z1) = (r(hi[0]), r(hi[1]), r(hi[2]));
    if x0 > x1 {
        std::mem::swap(&mut x0, &mut x1);
    }
    if y0 > y1 {
        std::mem::swap(&mut y0, &mut y1);
    }
    if z0 > z1 {
        std::mem::swap(&mut z0, &mut z1);
    }
    let box_of = |t0: i64, t1: i64| {
        ints(&[
            x0,
            1 + x1 - x0,
            y0,
            1 + y1 - y0,
            z0,
            1 + z1 - z0,
            t0,
            1 + t1 - t0,
        ])
    };
    let fsl = box_of(tmin, tmax);
    if !any && tsz > 1 {
        // FSL-188: the time range of no voxel
        let fixed = box_of(0, tsz - 1);
        return pick(
            ctx,
            json,
            fsl,
            fixed,
            &defects::FSL_188,
            "-w: no nonzero voxel in a 4D image",
        );
    }
    fsl
}

/// `-v` and `-V`: the number of voxels and their volume (`nvox * xdim * ydim * zdim` in single
/// precision); the JSON count in single precision (FSL-189).
fn voxels(ctx: &mut Ctx, st: &State, json: bool, nonzero: bool) -> Out {
    let v = &st.vol;
    let nvox: i64 = if nonzero {
        match &st.masknz {
            Some(m) => vstats::sums(&m.data).0 as i64,
            None => v.data.iter().filter(|&&x| x != 0.0).count() as i64,
        }
    } else {
        match &st.mask {
            Some(m) => {
                let n = vstats::sums(&m.data).0 as i64;
                if m.tsize() == 1 { n * v.tsize() } else { n }
            }
            None => v.nvoxels() * v.tsize(),
        }
    };
    let volume = sse::mul_f32(
        sse::mul_f32(sse::mul_f32(nvox as f32, v.xdim()), v.ydim()),
        v.zdim(),
    );
    let text = format!("{nvox} {} ", f6(f64::from(volume)));
    let fsl = Out {
        text: text.clone(),
        json: Value::floats(&[f64::from(nvox as f32), f64::from(volume)]),
    };
    if !json || (nvox as f32) as i64 == nvox {
        return fsl;
    }
    let fixed = Out {
        text,
        json: Value::floats(&[nvox as f64, f64::from(volume)]),
    };
    let opt = if nonzero { "-V" } else { "-v" };
    pick(
        ctx,
        json,
        fsl,
        fixed,
        &defects::FSL_189,
        &format!("{opt}: {nvox} voxels written in single precision"),
    )
}

/// `-R`: `vol.min(mask)` and `vol.max(mask)` (each prints FSL's message for an empty mask).
fn range(ctx: &mut Ctx, st: &State, json: bool) -> Res<Out> {
    check_mask(&st.vol, st.mask.as_ref(), "calculateExtrema: ")?;
    let m = mask_data(st.mask.as_ref());
    let e = vstats::extrema_masked(&st.vol.data, m);
    let (lo, hi) = match &e {
        Some(e) => (e.min, e.max),
        None => {
            ctx.eput("calculateExtrema: mask is empty\ncalculateExtrema: mask is empty\n");
            (0.0, 0.0)
        }
    };
    let fsl = nums(&[f64::from(lo), f64::from(hi)]);
    if let Some(e) = e
        && e.max.is_nan()
        && let Some(keep) = nan_free(&st.vol, st.mask.as_ref())
    {
        let fixed = match vstats::extrema_masked(&st.vol.data, &keep.data) {
            Some(f) => nums(&[f64::from(f.min), f64::from(f.max)]),
            None => nums(&[f64::NAN, f64::NAN]),
        };
        return Ok(pick(
            ctx,
            json,
            fsl,
            fixed,
            &defects::FSL_181,
            "-R: the first value is NaN",
        ));
    }
    Ok(fsl)
}

/// `-r`: `vol.robustlimits(mask)`, with the warning of an empty mask; NaN values left out by
/// default (FSL-181).
fn robust(ctx: &mut Ctx, st: &State, json: bool) -> Res<Out> {
    let ((lo, hi), warning) = st.vol.robust_limits_masked(st.mask.as_ref())?;
    if let Some(w) = warning {
        ctx.eput(&format!("{w}\n"));
    }
    let fsl = nums(&[f64::from(lo), f64::from(hi)]);
    if let Some(keep) = nan_free(&st.vol, st.mask.as_ref()) {
        let fixed = if keep.no_mask_voxels() == 0 {
            nums(&[0.0, 0.0])
        } else {
            let (a, b) = volops::find_thresholds(&st.vol, Some(&keep))?;
            nums(&[f64::from(a), f64::from(b)])
        };
        return Ok(pick(
            ctx,
            json,
            fsl,
            fixed,
            &defects::FSL_181,
            "-r: NaN values in the robust range",
        ));
    }
    Ok(fsl)
}

/// `-p` and `-P`: `vol.percentile(p, mask)`: the masked values sorted (libstdc++'s
/// `std::sort`), the value at `(unsigned)(n p)`; NaN values left out by default (FSL-181).
fn percentile(
    ctx: &mut Ctx,
    vol: &Volume,
    mask: Option<&Volume>,
    p: f32,
    json: bool,
    opt: &str,
) -> Res<Out> {
    if p > 1.0 || p < 0.0 {
        return Err(Stop::Exception(FslError::imthrow(
            "Percentiles must be in the range [0.0,1.0]",
            4,
        )));
    }
    if let Some(m) = mask
        && !m.data.is_empty()
        && !volops::samesize_subset(vol, m)
    {
        return Err(Stop::Exception(FslError::imthrow(
            "mask and vol have different sizes in calc_percentiles",
            3,
        )));
    }
    let mut values = vstats::percentile_values(&vol.data, mask_data(mask));
    let has_nan = values.iter().any(|v| v.is_nan());
    let fixed = if has_nan {
        let mut clean: Vec<f32> = values.iter().copied().filter(|v| !v.is_nan()).collect();
        num(f64::from(vstats::percentile(&mut clean, p).unwrap_or(0.0)))
    } else {
        num(0.0)
    };
    let fsl = match vstats::percentile(&mut values, p) {
        Ok(v) => num(f64::from(v)),
        Err(stdsort::Outside) => {
            if ctx.reproduce() {
                return Err(ctx.refuse(
                    &defects::FSL_181,
                    &format!("{opt}: NaN values in the sort (it leaves the values)"),
                ));
            }
            ctx.notice(&defects::FSL_181, format!("{opt}: NaN values left out"));
            return Ok(fixed);
        }
    };
    if !has_nan {
        return Ok(fsl);
    }
    Ok(pick(
        ctx,
        json,
        fsl,
        fixed,
        &defects::FSL_181,
        &format!("{opt}: NaN values left out"),
    ))
}

/// FSL's histogram and the corrected one (`None`: unsized).
type Hists = (Option<Vec<f64>>, Option<Vec<f64>>);

/// `imageHistogram` as fslstats' options call it, with FSL's message for an empty mask:
/// `None` when the range is empty (the histogram stays unsized); FSL-187's clamped bins and
/// FSL-181's NaN values left out by default.
#[allow(clippy::too_many_arguments)]
fn hist_of(
    ctx: &mut Ctx,
    vol: &Volume,
    mask: Option<&Volume>,
    nbins: i32,
    lo: f32,
    hi: f32,
    opt: &str,
    skip_nan: bool,
) -> Res<Hists> {
    check_mask(vol, mask, "imageHistogram: ")?;
    let m = mask_data(mask);
    if vstats::count_masked(vol.data.len(), m) == 0 {
        ctx.eput("findHistogram: mask is empty\n");
    }
    let fsl = vstats::image_histogram(&vol.data, m, nbins, lo, hi, false, false);
    let fixed = vstats::image_histogram(&vol.data, m, nbins, lo, hi, true, skip_nan);
    if let Some((_, _, moved)) = &fixed
        && *moved > 0
    {
        ctx.notice(
            &defects::FSL_187,
            format!("{opt}: {moved} values beyond int's range from the histogram's bins"),
        );
    }
    Ok((fsl.map(|h| h.0), fixed.map(|h| h.0)))
}

/// `-h` (the image's range) and `-H` (the given one).
fn histogram(
    ctx: &mut Ctx,
    st: &State,
    nbins: i32,
    limits: Option<(f32, f32)>,
    json: bool,
) -> Res<Out> {
    let opt = if limits.is_some() { "-H" } else { "-h" };
    let mask = st.mask.as_ref();
    let nan = nan_free(&st.vol, None).is_some();
    let (lo, hi, fixed_range) = match limits {
        Some((a, b)) => (a, b, (a, b)),
        None => {
            let e = vstats::extrema(&st.vol.data);
            let (a, b) = e.map_or((0.0, 0.0), |e| (e.min, e.max));
            let f =
                vstats::extrema_without_nan(&st.vol.data).map_or((0.0, 0.0), |e| (e.min, e.max));
            (a, b, f)
        }
    };
    let (fsl, mut fixed) = hist_of(ctx, &st.vol, mask, nbins, lo, hi, opt, nan)?;
    if nan && fixed_range != (lo, hi) {
        let m = mask_data(mask);
        let (a, b) = fixed_range;
        fixed = vstats::image_histogram(&st.vol.data, m, nbins, a, b, true, true).map(|h| h.0);
    }
    let fsl_out = column(fsl.as_deref().unwrap_or(&[]));
    let fixed_out = column(fixed.as_deref().unwrap_or(&[]));
    if fsl_out == fixed_out {
        return Ok(fsl_out);
    }
    if nan {
        return Ok(pick(
            ctx,
            json,
            fsl_out,
            fixed_out,
            &defects::FSL_181,
            &format!("{opt}: NaN values left out"),
        ));
    }
    // FSL-187 was named by hist_of
    Ok(if ctx.reproduce() { fsl_out } else { fixed_out })
}

/// `-e` and `-E`: the entropy of a 1000-bin histogram of the masked values between the
/// image's robust limits (`histogram(nbins, mask)`: `robustmin()`, `robustmax()`),
/// `-Σ (h/n) ln(h/n) / ln(1000)`. An empty range leaves the histogram unsized, whose first
/// element's index divides by its 0 columns: FSL crashes (FSL-185; the entropy 0 by default).
fn entropy(ctx: &mut Ctx, vol: &Volume, mask: Option<&Volume>, json: bool, opt: &str) -> Res<Out> {
    const NBINS: i32 = 1000;
    let ((lo, hi), _) = vol.robust_limits_masked(None)?;
    let keep = nan_free(vol, None);
    let (flo, fhi) = match &keep {
        Some(k) if k.no_mask_voxels() > 0 => volops::find_thresholds(vol, Some(k))?,
        Some(_) => (0.0, 0.0),
        None => (lo, hi),
    };
    let (fsl, mut fixed) = hist_of(ctx, vol, mask, NBINS, lo, hi, opt, keep.is_some())?;
    if (flo, fhi) != (lo, hi) {
        fixed = vstats::image_histogram(&vol.data, mask_data(mask), NBINS, flo, fhi, true, true)
            .map(|h| h.0);
    }
    let value = |h: &[f64]| -> f64 {
        let ntot = arma::accu(h);
        let mut e = 0f64;
        for &x in h {
            if x > 0.0 {
                let q = sse::div_f64(x, ntot);
                e = sse::sub_f64(e, sse::mul_f64(q, glibc_double::log(q)));
            }
        }
        sse::div_f64(e, glibc_double::log(f64::from(NBINS)))
    };
    let fixed_out = num(fixed.as_deref().map_or(0.0, value));
    let Some(h) = fsl else {
        // hist(1) of an unsized ColumnVector divides by its 0 columns: SIGFPE
        if ctx.reproduce() {
            return Err(ctx.refuse(
                &defects::FSL_185,
                &format!("{opt}: an image whose robust range is empty"),
            ));
        }
        ctx.notice(
            &defects::FSL_185,
            format!("{opt}: an image whose robust range is empty"),
        );
        return Ok(fixed_out);
    };
    let fsl_out = num(value(&h));
    if keep.is_some() {
        return Ok(pick(
            ctx,
            json,
            fsl_out,
            fixed_out,
            &defects::FSL_181,
            &format!("{opt}: NaN values left out"),
        ));
    }
    Ok(if ctx.reproduce() { fsl_out } else { fixed_out })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(args: &[&str], dir: &std::path::Path, mode: UpstreamDefects) -> (u8, String, String) {
        let s = Settings::default();
        let r = Run { fsl: &s, mode };
        let mut argv = vec!["fslstats".to_owned()];
        argv.extend(
            args.iter()
                .map(|a| a.replace("@", &format!("{}/", dir.display()))),
        );
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let c = main(&argv, &r, &mut o, &mut e);
        (
            c,
            String::from_utf8(o).unwrap(),
            String::from_utf8(e).unwrap(),
        )
    }

    #[test]
    fn usage_and_arguments() {
        let d = tempfile::tempdir().unwrap();
        let (c, o, _) = run(&["x"], d.path(), UpstreamDefects::Fix);
        assert_eq!((c, o.as_str()), (1, USAGE));
        let (c, o, _) = run(&["-K", "m"], d.path(), UpstreamDefects::Fix);
        assert_eq!((c, o.as_str()), (1, USAGE));
        let (c, _, e) = run(&["@missing", "-m"], d.path(), UpstreamDefects::Fix);
        assert_eq!(c, 255);
        assert!(e.contains("Failed to read volume"), "{e}");
    }

    #[test]
    fn formats() {
        assert_eq!(column(&[1.0, 2.5]).text, "1.000000 \n2.500000 \n ");
        assert_eq!(ints(&[0, 10]).text, "0 10 ");
        assert_eq!(nonzero_mean(&[0.0, 1.0, 2.0]), 1.5);
        assert_eq!(nonzero_stddev(&[3.0]), 0.0);
    }
}
