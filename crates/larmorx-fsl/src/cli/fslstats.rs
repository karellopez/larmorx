// SPDX-License-Identifier: Apache-2.0
//! `larmorx fsl fslstats`: FSL's image statistics on FSL's arguments (`specs/fslstats.md`),
//! `fslstats [preoptions] <input> [options]`, written from its spec, FSL's usage text and
//! black-box runs of FSL 6.0.7.23's program, without access to FSL's source code or its replica.
//!
//! A pass (each volume with `-t`, each label with `-K`, else the whole image) starts from the
//! image in single precision (FSL's radiological frame), no mask and the thresholds at
//! `±FLT_MAX`; the options apply in order, each changing what the later ones see (thresholds,
//! masks, `-d`, `-a`, `-n`) or printing numbers (§5). The defects FSL-181 to FSL-189 are
//! corrected by default and reproduced with `reproduce` (§8), each named where the printed
//! output changes.
//!
//! Built for speed: the masks are bytes, the statistics single passes over the values (the
//! masked sums in FSL's blocks give its bits), the percentiles by selection, the counts and
//! extremes by blocks combined in order.

use std::collections::BTreeMap;
use std::io::Write;

use larmorx_core::defects::{self, Notices};
use larmorx_core::math;
use larmorx_io::nifti::NiftiHeader;
use rayon::prelude::*;

use super::options::atof;
use super::{Settings, common, fmt};
use crate::flirt::schedule::std_sort_checked;
use crate::maths::image;
use crate::maths::num::{self, cvtt};
use crate::maths::ops;
use crate::maths::reduce::{block_sum, sqrt64};

/// The usage text (§2.3), exactly as FSL prints it on standard output.
pub const USAGE: &str = include_str!("fslstats_usage.txt");

/// How a run ends early: its exit status (the messages are printed).
struct Exit(u8);

/// The status of a refusal that stands for FSL's crash (its buffered output lost), status 1.
const CRASHED: u8 = 200;

const EMPTY_MASK: &str = "ERROR:: Empty mask image";
const EXTREMA_EMPTY: &str = "calculateExtrema: mask is empty";
const HISTOGRAM_EMPTY: &str = "findHistogram: mask is empty";
const ROBUST_EMPTY: &str = "WARNING:: Empty mask image in calc_robustlimits";
const COG_ZERO: &str = "WARNING::in calculating COG, total = 0.0";

/// A JSON value of the output (§6.2).
#[derive(Clone, Debug)]
enum J {
    F(f64),
    I(i64),
    A(Vec<J>),
}

/// An image read in single precision, in FSL's frame.
struct Image {
    data: Vec<f32>,
    dims: [usize; 4],
    header: NiftiHeader,
    flipped: bool,
}

impl Image {
    fn nvox(&self) -> usize {
        self.dims[0] * self.dims[1] * self.dims[2]
    }
}

/// The state of a pass (§4).
struct Pass {
    img: Vec<f32>,
    /// The pass's x, y, z sizes and volumes.
    dims: [usize; 4],
    mask: Option<Vec<bool>>,
    nonzero: Option<Vec<bool>>,
    lthr: f32,
    uthr: f32,
}

/// What a pass prints: its text (`flushed`: how much of it FSL's buffer had written when it
/// crashed), its JSON entries, and the JSON counts single precision rounds (FSL-189).
struct Printed {
    text: String,
    flushed: usize,
    crash: bool,
    json: BTreeMap<String, J>,
    rounded: BTreeMap<String, usize>,
}

impl Printed {
    fn number(&mut self, key: &str, v: f64) {
        self.text.push_str(&fmt::f(v));
        self.text.push(' ');
        self.json.insert(key.into(), J::F(v));
    }
    fn numbers(&mut self, key: &str, v: &[f64]) {
        for &x in v {
            self.text.push_str(&fmt::f(x));
            self.text.push(' ');
        }
        self.json
            .insert(key.into(), J::A(v.iter().map(|&x| J::F(x)).collect()));
    }
    fn ints(&mut self, key: &str, v: &[i64]) {
        for &x in v {
            self.text.push_str(&format!("{x} "));
        }
        self.json
            .insert(key.into(), J::A(v.iter().map(|&x| J::I(x)).collect()));
    }
}

/// Runs fslstats.
pub fn main(args: &[String], settings: &Settings, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let mode = match settings.defects() {
        Ok(m) => m,
        Err(e) => {
            let _ = writeln!(err, "larmorx fsl fslstats: {e}");
            return defects::EXIT_BAD_MODE;
        }
    };
    let n_threads = settings.threads();
    let (code, o, e, notices) = crate::bet::pooled(n_threads, || {
        let mut notices = Notices::new(mode);
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let code = match run(args, settings, &mut notices, &mut o, &mut e) {
            Ok(()) => 0,
            Err(Exit(c)) => c,
        };
        (code, o, e, notices)
    });
    let _ = out.write_all(&o);
    let _ = err.write_all(&e);
    notices.print(err);
    code
}

/// The image layer's failure to read `name`: its lines, then the exception's text (status 255).
fn read_failure(err: &mut dyn Write, name: &str, e: &larmorx_mri::hmc::image::ImageError) -> Exit {
    let (lines, what) = common::image_exception(name, e);
    let _ = err.write_all(lines.as_bytes());
    let _ = writeln!(err, "{what}");
    Exit(255)
}

fn read(
    settings: &Settings,
    name: &str,
    notices: &mut Notices,
    err: &mut dyn Write,
) -> Result<(image::Stored, Vec<f32>), Exit> {
    let mut ns = Vec::new();
    let s = image::read_stored(settings, name, notices.mode(), &mut ns, settings.threads());
    notices.extend(ns);
    let mut s = s.map_err(|e| read_failure(err, name, &e))?;
    // the stored values moved (or dropped once converted): one copy of the image
    let (data, _) = s.take_values::<f32>(notices.fix());
    Ok((s, data))
}

fn read_image(
    settings: &Settings,
    name: &str,
    notices: &mut Notices,
    err: &mut dyn Write,
) -> Result<Image, Exit> {
    let (s, data) = read(settings, name, notices, err)?;
    Ok(Image {
        data,
        dims: s.dims,
        header: s.header,
        flipped: s.flipped,
    })
}

fn run(
    args: &[String],
    settings: &Settings,
    notices: &mut Notices,
    out: &mut dyn Write,
    err: &mut Vec<u8>,
) -> Result<(), Exit> {
    // the preoptions, while at least two arguments remain (§2.1)
    let (mut k, mut series, mut json) = (0usize, false, false);
    let mut index_name: Option<&str> = None;
    while args.len() - k > 1 {
        match args[k].as_str() {
            "-t" => series = true,
            "-json" => json = true,
            "-K" => {
                index_name = Some(args[k + 1].as_str());
                k += 1;
            }
            _ => break,
        }
        k += 1;
    }
    if args.len() - k < 2 {
        let _ = out.write_all(USAGE.as_bytes());
        return Err(Exit(1));
    }
    let input = read_image(settings, &args[k], notices, err)?;
    let opts = &args[k + 1..];
    let index = match index_name {
        Some(n) => {
            let mut ns = Vec::new();
            let s = image::read_stored(settings, n, notices.mode(), &mut ns, settings.threads());
            notices.extend(ns);
            let mut s = s.map_err(|e| read_failure(err, n, &e))?;
            let (v, changed) = s.take_values::<i32>(notices.fix());
            if changed {
                image::fsl151(notices, "the index image's labels");
            }
            Some((v, s.dims))
        }
        None => None,
    };
    let nvox = input.nvox();
    let nt = input.dims[3];
    let volumes: Vec<Option<usize>> = if series {
        (0..nt).map(Some).collect()
    } else {
        vec![None]
    };
    let labels: Vec<Option<i32>> = match &index {
        Some((v, _)) => {
            let n = v.iter().copied().max().unwrap_or(0);
            (1..=n.max(0)).map(Some).collect()
        }
        None => vec![None],
    };
    let mut ctx = Ctx {
        settings,
        input: &input,
        cache: BTreeMap::new(),
        json,
    };
    let mut doc: Vec<Vec<BTreeMap<String, J>>> = Vec::new();
    for &t in &volumes {
        let mut indices = Vec::new();
        for &label in &labels {
            let mut img = match t {
                Some(t) => input.data[t * nvox..(t + 1) * nvox].to_vec(),
                None => input.data.clone(),
            };
            let pnt = if t.is_some() { 1 } else { nt };
            let mut pass = Pass {
                img: Vec::new(),
                dims: [input.dims[0], input.dims[1], input.dims[2], pnt],
                mask: None,
                nonzero: None,
                lthr: -f32::MAX,
                uthr: f32::MAX,
            };
            if let (Some(l), Some((iv, idims))) = (label, &index) {
                if idims[3] > 1 {
                    let _ = writeln!(err, "Index mask must be 3D");
                    return Err(Exit(3));
                }
                if idims[..3] != input.dims[..3] {
                    let what = "Attempted to multiply images of different sizes";
                    let _ = writeln!(err, "Image Exception : #3 :: {what}\n{what}");
                    return Err(Exit(255));
                }
                let lm: Vec<bool> = iv.par_iter().map(|&v| v == l).collect();
                if !lm.par_iter().any(|&b| b) {
                    let _ = writeln!(out, "missing label: {l} in mask");
                    continue;
                }
                let mask: Vec<bool> = (0..img.len()).map(|i| lm[i % nvox]).collect();
                img.par_iter_mut()
                    .zip(&mask)
                    .for_each(|(v, &m)| *v = keep(*v, m));
                pass.nonzero = Some(nonzero_of(&img, &mask));
                pass.mask = Some(mask);
            }
            pass.img = img;
            let mut p = Printed {
                text: String::new(),
                flushed: 0,
                crash: false,
                json: BTreeMap::new(),
                rounded: BTreeMap::new(),
            };
            let r = options(&mut ctx, &mut pass, t, opts, notices, &mut p, err);
            if !json {
                let text = if p.crash {
                    &p.text[..p.flushed]
                } else {
                    &p.text
                };
                let _ = out.write_all(text.as_bytes());
            }
            r?;
            if let Some(n) = p.rounded.values().find(|&&n| n > 0) {
                notices.add(
                    &defects::FSL_189,
                    if notices.fix() {
                        format!("-json's voxel count {n} written exactly (FSL writes it in single precision)")
                    } else {
                        "-json's voxel counts written in single precision, as FSL does".to_owned()
                    },
                );
            }
            if !json {
                let _ = writeln!(out);
            }
            indices.push(p.json);
        }
        doc.push(indices);
    }
    if json {
        let _ = out.write_all(json_document(&doc).as_bytes());
    }
    Ok(())
}

/// What the options need besides the pass: the input, the images `-k` and `-d` name (read
/// once).
struct Ctx<'a> {
    settings: &'a Settings,
    input: &'a Image,
    cache: BTreeMap<String, Image>,
    json: bool,
}

impl Ctx<'_> {
    fn operand(
        &mut self,
        name: &str,
        notices: &mut Notices,
        err: &mut dyn Write,
    ) -> Result<&Image, Exit> {
        if !self.cache.contains_key(name) {
            let img = read_image(self.settings, name, notices, err)?;
            self.cache.insert(name.to_owned(), img);
        }
        Ok(&self.cache[name])
    }
}

/// `v` where the mask keeps it, else `v · 0` (NaN for NaN and the infinities, −0 for negative
/// values; x86-64's NaN).
#[inline]
fn keep(v: f32, m: bool) -> f32 {
    if m { v } else { num::nan32(v, 0.0, v * 0.0) }
}

/// The nonzero mask: the mask where the value is not 0 (NaN is not 0).
fn nonzero_of(img: &[f32], mask: &[bool]) -> Vec<bool> {
    img.par_iter()
        .zip(mask)
        .map(|(&v, &m)| m && v != 0.0)
        .collect()
}

/// `-l`, `-u` (and the options that make the masks when there are none): both masks from the
/// thresholds (strict), the image multiplied by the mask.
fn apply_thresholds(pass: &mut Pass) {
    let (lo, hi) = (pass.lthr, pass.uthr);
    let mask: Vec<bool> = pass.img.par_iter().map(|&v| lo < v && v < hi).collect();
    pass.img
        .par_iter_mut()
        .zip(&mask)
        .for_each(|(v, &m)| *v = keep(*v, m));
    pass.nonzero = Some(nonzero_of(&pass.img, &mask));
    pass.mask = Some(mask);
}

fn ensure_masks(pass: &mut Pass) {
    if pass.mask.is_none() {
        apply_thresholds(pass);
    }
}

/// The values a mask keeps (every value without one), in order.
fn kept(img: &[f32], mask: Option<&Vec<bool>>) -> Vec<f32> {
    match mask {
        Some(m) => img
            .par_iter()
            .zip(m)
            .filter(|(_, k)| **k)
            .map(|(v, _)| *v)
            .collect(),
        None => img.to_vec(),
    }
}

fn missing(err: &mut dyn Write, what: &str) -> Exit {
    let _ = writeln!(err, "{what}");
    Exit(2)
}

/// The options of a pass, in order. FSL's standard output is buffered, flushed by each line
/// on its standard error and at its exit: a crash loses what was printed after the last one
/// (the original's refusals of FSL's crashes print alike).
fn options(
    ctx: &mut Ctx<'_>,
    pass: &mut Pass,
    t: Option<usize>,
    opts: &[String],
    notices: &mut Notices,
    p: &mut Printed,
    err: &mut Vec<u8>,
) -> Result<(), Exit> {
    let mut k = 0;
    while k < opts.len() {
        let (before, start) = (err.len(), p.text.len());
        match one_option(ctx, pass, t, opts, k, notices, p, err) {
            Ok(used) => {
                if err.len() > before {
                    p.flushed = start;
                }
                k += 1 + used;
            }
            Err(Exit(CRASHED)) => {
                if !err[before..].starts_with(b"larmorx: error:") {
                    p.flushed = start;
                }
                p.crash = true;
                return Err(Exit(1));
            }
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

/// The option at `opts[k]`; returns how many arguments it took.
#[allow(clippy::too_many_arguments)]
fn one_option(
    ctx: &mut Ctx<'_>,
    pass: &mut Pass,
    t: Option<usize>,
    opts: &[String],
    k: usize,
    notices: &mut Notices,
    p: &mut Printed,
    err: &mut Vec<u8>,
) -> Result<usize, Exit> {
    let fix = notices.fix();
    {
        let o = opts[k].as_str();
        let arg = |j: usize| opts.get(k + j).map(String::as_str);
        let need = |j: usize, err: &mut dyn Write| -> Result<&str, Exit> {
            arg(j).ok_or_else(|| missing(err, &format!("Must specify an argument to {o}")))
        };
        let mut used = 0;
        match o {
            "-l" | "-u" => {
                let v = atof(need(1, err)?) as f32;
                if o == "-l" {
                    pass.lthr = v;
                } else {
                    pass.uthr = v;
                }
                apply_thresholds(pass);
                used = 1;
            }
            "-k" => {
                let name = need(1, err)?.to_owned();
                mask_option(ctx, pass, t, &name, notices, err)?;
                used = 1;
            }
            "-d" => {
                let name = need(1, err)?.to_owned();
                difference(ctx, pass, t, &name, notices, err)?;
                used = 1;
            }
            "-a" => pass
                .img
                .par_iter_mut()
                .for_each(|v| *v = f32::from_bits(v.to_bits() & 0x7fff_ffff)),
            "-n" => pass.img.par_iter_mut().for_each(|v| {
                if !v.is_finite() {
                    *v = 0.0;
                }
            }),
            "-v" | "-V" => {
                let n = count(pass, o == "-V");
                let [dx, dy, dz] = voxel(ctx.input);
                let vol = n as f32 * dx * dy * dz;
                p.text.push_str(&format!("{n} {} ", fmt::f(f64::from(vol))));
                let single = f64::from(n as f32);
                if ctx.json {
                    // FSL-189: named when the rounded count survives into the document
                    p.rounded
                        .insert(o.into(), if single != n as f64 { n } else { 0 });
                }
                let c = if fix { n as f64 } else { single };
                p.json
                    .insert(o.into(), J::A(vec![J::F(c), J::F(f64::from(vol))]));
            }
            "-m" | "-M" | "-s" | "-S" => {
                let v = mean_std(pass, o, notices, ctx.json, err);
                p.number(o, v);
            }
            "-D" => {
                let n = pass.img.len();
                let s = block_sum(pass.img.iter().map(|&v| f64::from(v)), n);
                p.number(o, s);
            }
            "-R" => {
                let mask = pass.mask.as_ref();
                match extremes(&pass.img, mask, fix, notices, o, ctx.json) {
                    Some(e) => p.numbers(o, &[f64::from(e.lo), f64::from(e.hi)]),
                    None => {
                        let _ = writeln!(err, "{EXTREMA_EMPTY}\n{EXTREMA_EMPTY}");
                        p.numbers(o, &[0.0, 0.0]);
                    }
                }
            }
            "-x" | "-X" => {
                let mask = pass.mask.as_ref();
                let Some(e) = extremes(&pass.img, mask, fix, notices, o, ctx.json) else {
                    let _ = writeln!(err, "{EXTREMA_EMPTY}");
                    notices.add(
                        &defects::FSL_184,
                        format!("{o} with an empty mask is refused (FSL crashes)"),
                    );
                    let _ = writeln!(
                        err,
                        "larmorx: error: fslstats: {o}: the mask selects no voxel, so none has \
                         an extreme value (FSL-184: FSL crashes here)"
                    );
                    return Err(Exit(CRASHED));
                };
                let at = if o == "-x" { e.hi_at } else { e.lo_at };
                let [x, y, z] = file_coords(ctx.input, at);
                p.ints(o, &[x, y, z]);
            }
            "-r" => {
                let vals = kept(&pass.img, pass.mask.as_ref());
                let r = ops::robust_range::<f32>(&vals, None, fix);
                let active = if fix { r.fix } else { r.rep };
                let Some((lo, hi)) = active else {
                    let _ = writeln!(err, "{ROBUST_EMPTY}");
                    p.numbers(o, &[0.0, 0.0]);
                    return Ok(0);
                };
                if fix && !r.causes.is_empty() {
                    let rep = r.rep.unwrap_or((0.0, 0.0));
                    if shown(ctx.json, &[lo, hi], &[rep.0, rep.1]) {
                        for (d, what) in &r.causes {
                            notices.add(d, format!("-r: {what}"));
                        }
                    }
                }
                p.numbers(o, &[f64::from(lo), f64::from(hi)]);
            }
            "-p" | "-P" => {
                let n = atof(need(1, err)?) as f32;
                used = 1;
                if n < 0.0 || n > 100.0 {
                    let _ = writeln!(err, "Percentile must be between 0 and 100");
                    return Err(Exit(1));
                }
                if o == "-P" {
                    ensure_masks(pass);
                }
                let m = if o == "-P" {
                    pass.nonzero.as_ref()
                } else {
                    pass.mask.as_ref()
                };
                let vals = kept(&pass.img, m);
                let v = percentile(vals, n / 100.0, notices, o, ctx.json, err)?;
                p.number(o, f64::from(v));
            }
            "-h" => {
                let Some(a) = arg(1) else {
                    return Err(missing(err, "Must specify the number of bins"));
                };
                used = 1;
                let nbins = cvtt(atof(a));
                if nbins < 1 {
                    let _ = writeln!(err, "Must specify at least 1 bin");
                    return Err(Exit(1));
                }
                let rep = ops::extremes(&pass.img, false).unwrap_or((0.0, 0.0));
                let ours = if fix {
                    ops::extremes(&pass.img, true).unwrap_or((0.0, 0.0))
                } else {
                    rep
                };
                let c = HistCall {
                    img: &pass.img,
                    mask: pass.mask.as_ref(),
                    nbins: nbins as usize,
                };
                print_histogram(p, o, &c, ours, rep, notices, err);
            }
            "-H" => {
                let Some(a) = arg(1) else {
                    return Err(missing(err, "Must specify the number of bins"));
                };
                let Some(l) = arg(2) else {
                    return Err(missing(err, "Must specify the histogram minimum intensity"));
                };
                let Some(h) = arg(3) else {
                    return Err(missing(err, "Must specify the histogram maximum intensity"));
                };
                used = 3;
                let nbins = cvtt(atof(a));
                if nbins < 1 {
                    let _ = writeln!(err, "Must specify at least 1 bin");
                    return Err(Exit(1));
                }
                let (lo, hi) = (atof(l) as f32, atof(h) as f32);
                if hi <= lo && fix {
                    notices.add(
                        &defects::FSL_186,
                        "-H with a maximum not above its minimum is refused (FSL prints nothing)",
                    );
                    let _ = writeln!(
                        err,
                        "larmorx: error: fslstats: -H: the histogram's maximum ({h}) is not above \
                         its minimum ({l}) (FSL-186: FSL prints no histogram and no message)"
                    );
                    return Err(Exit(1));
                }
                let c = HistCall {
                    img: &pass.img,
                    mask: pass.mask.as_ref(),
                    nbins: nbins as usize,
                };
                print_histogram(p, o, &c, (lo, hi), (lo, hi), notices, err);
            }
            "-e" | "-E" => {
                ensure_masks(pass);
                let m = if o == "-E" {
                    pass.nonzero.as_ref()
                } else {
                    pass.mask.as_ref()
                };
                let v = entropy(&pass.img, m, o, notices, ctx.json, err)?;
                p.number(o, v);
            }
            "-w" => {
                ensure_masks(pass);
                let b = bounding_box(ctx.input, pass, fix, notices);
                p.ints(o, &b);
            }
            "-c" | "-C" => {
                let c = centre(ctx.input, pass, o == "-c", err);
                p.numbers(o, &c);
            }
            _ => {
                let _ = writeln!(err, "Unrecognised option: {o}");
                return Err(Exit(3));
            }
        }
        Ok(used)
    }
}

/// Whether two results print differently (the text's six decimals, or JSON's full precision).
fn shown(json: bool, a: &[f32], b: &[f32]) -> bool {
    a.iter().zip(b).any(|(&x, &y)| {
        if json {
            x.to_bits() != y.to_bits()
        } else {
            fmt::f(f64::from(x)) != fmt::f(f64::from(y))
        }
    })
}

fn shown64(json: bool, x: f64, y: f64) -> bool {
    if json {
        fmt::json(x) != fmt::json(y)
    } else {
        fmt::f(x) != fmt::f(y)
    }
}

/// The voxel sizes in single precision.
fn voxel(img: &Image) -> [f32; 3] {
    [1, 2, 3].map(|k| (img.header.pixdim[k] as f32).abs())
}

/// `-v`'s (`-V`'s) count: the mask's (nonzero mask's) voxels, else every value (every value
/// that is not 0, NaN counted).
fn count(pass: &Pass, nonzero: bool) -> usize {
    let m = if nonzero { &pass.nonzero } else { &pass.mask };
    match m {
        Some(m) => m.par_iter().filter(|&&b| b).count(),
        None if nonzero => pass.img.par_iter().filter(|&&v| v != 0.0).count(),
        None => pass.img.len(),
    }
}

/// The masked sums of §5.2 in FSL's blocks (`b` from the image's value count): `(S, S2, n)`.
fn masked_sums(vals: &[f32], n_all: usize) -> (f64, f64, usize) {
    let b = (((n_all as f64).sqrt().floor()) as usize).max(10000);
    // each block's sums in parallel, added in order: FSL's bits
    let blocks: Vec<(f64, f64)> = vals
        .par_chunks(b + 1)
        .map(|c| {
            let (mut s, mut q) = (0.0f64, 0.0f64);
            for &v in c {
                let x = f64::from(v);
                s = num::nan64(s, x, s + x);
                let x2 = num::nan64(x, x, x * x);
                q = num::nan64(q, x2, q + x2);
            }
            (s, q)
        })
        .collect();
    let (mut s, mut q) = (0.0f64, 0.0f64);
    for (bs, bq) in blocks {
        s = num::nan64(s, bs, s + bs);
        q = num::nan64(q, bq, q + bq);
    }
    (s, q, vals.len())
}

/// The masked mean and the clamped variance of §5.2.
fn masked_moments(vals: &[f32], n_all: usize) -> (f64, f64) {
    let (s, q, n) = masked_sums(vals, n_all);
    let nf = n as f64;
    let m = num::nan64(s, nf.max(1.0), s / nf.max(1.0));
    let r = (nf / ((n as f64) - 1.0).max(1.0)) * (q / nf - m * m);
    let var = if r.is_nan() {
        num::nan64(q, m, r)
    } else {
        r.max(0.0)
    };
    (m, var)
}

/// `-m`, `-M`, `-s`, `-S` (§5.2).
fn mean_std(pass: &Pass, o: &str, notices: &mut Notices, json: bool, err: &mut dyn Write) -> f64 {
    let fix = notices.fix();
    let n_all = pass.img.len();
    let nonzero = o == "-M" || o == "-S";
    let std = o == "-s" || o == "-S";
    let m = if nonzero { &pass.nonzero } else { &pass.mask };
    if let Some(m) = m {
        let vals = kept(&pass.img, Some(m));
        if vals.is_empty() {
            let _ = writeln!(err, "{EMPTY_MASK}");
            return 0.0;
        }
        let (mean, var) = masked_moments(&vals, n_all);
        return if std { sqrt64(var) } else { mean };
    }
    match o {
        "-m" => {
            let s = block_sum(pass.img.iter().map(|&v| f64::from(v)), n_all);
            num::nan64(s, n_all as f64, s / n_all as f64)
        }
        "-M" => {
            // the nonzero values' plain sum in order (rule C: the masked mean adds in blocks)
            let (mut s, mut n) = (0.0f64, 0usize);
            for &v in &pass.img {
                if v != 0.0 {
                    let x = f64::from(v);
                    s = num::nan64(s, x, s + x);
                    n += 1;
                }
            }
            if n == 0 {
                0.0
            } else {
                num::nan64(s, n as f64, s / n as f64)
            }
        }
        "-s" => {
            // FSL-182: the unmasked variance, not clamped (NaN for one value)
            let (s, q, n) = masked_sums(&pass.img, n_all);
            let nf = n as f64;
            let mean = num::nan64(s, nf, s / nf);
            let a = num::nan64(nf, nf - 1.0, nf / (nf - 1.0));
            let d = num::nan64(q / nf, mean * mean, q / nf - mean * mean);
            let theirs = sqrt64(num::nan64(a, d, a * d));
            let ours = sqrt64(masked_moments(&pass.img, n_all).1);
            fsl182(notices, o, json, ours, theirs)
        }
        _ => {
            // FSL-182: the nonzero values' squares in single precision, not clamped
            let (mut s, mut q, mut n) = (0.0f64, 0.0f64, 0usize);
            for &v in &pass.img {
                if v != 0.0 {
                    let x = f64::from(v);
                    s = num::nan64(s, x, s + x);
                    let sq = f64::from(num::nan32(v, v, v * v));
                    q = num::nan64(q, sq, q + sq);
                    n += 1;
                }
            }
            let theirs = if n < 2 {
                0.0
            } else {
                let nf = n as f64;
                let mean = s / nf;
                let r = num::nan64(q, nf * mean * mean, q - nf * mean * mean);
                sqrt64(num::nan64(r, nf - 1.0, r / (nf - 1.0)))
            };
            let ours = if fix {
                let vals: Vec<f32> = pass.img.par_iter().copied().filter(|&v| v != 0.0).collect();
                if vals.is_empty() {
                    0.0
                } else {
                    sqrt64(masked_moments(&vals, n_all).1)
                }
            } else {
                theirs
            };
            fsl182(notices, o, json, ours, theirs)
        }
    }
}

fn fsl182(notices: &mut Notices, o: &str, json: bool, ours: f64, theirs: f64) -> f64 {
    if notices.reproduce() {
        return theirs;
    }
    if shown64(json, ours, theirs) {
        notices.add(
            &defects::FSL_182,
            format!(
                "{o} without a mask: the masked form's variance (FSL's prints {})",
                fmt::f(theirs)
            ),
        );
    }
    ours
}

/// The extremes and their first positions.
#[derive(Clone, Copy, Debug)]
struct Extremes {
    lo: f32,
    hi: f32,
    lo_at: usize,
    hi_at: usize,
}

fn extremes_pass(img: &[f32], mask: Option<&Vec<bool>>, skip_nan: bool) -> Option<Extremes> {
    const B: usize = 1 << 15;
    let parts: Vec<Option<Extremes>> = img
        .par_chunks(B)
        .enumerate()
        .map(|(c, chunk)| {
            let base = c * B;
            let mut e: Option<Extremes> = None;
            for (j, &v) in chunk.iter().enumerate() {
                let i = base + j;
                if mask.is_some_and(|m| !m[i]) || (skip_nan && v.is_nan()) {
                    continue;
                }
                match &mut e {
                    None => {
                        e = Some(Extremes {
                            lo: v,
                            hi: v,
                            lo_at: i,
                            hi_at: i,
                        })
                    }
                    Some(x) => {
                        if v > x.hi {
                            x.hi = v;
                            x.hi_at = i;
                        } else if v < x.lo {
                            x.lo = v;
                            x.lo_at = i;
                        }
                    }
                }
            }
            e
        })
        .collect();
    let mut it = parts.into_iter().flatten();
    let mut e = it.next()?;
    for x in it {
        if x.hi > e.hi {
            e.hi = x.hi;
            e.hi_at = x.hi_at;
        }
        if x.lo < e.lo {
            e.lo = x.lo;
            e.lo_at = x.lo_at;
        }
    }
    Some(e)
}

/// `-R`, `-x`, `-X`'s extremes from the first value (FSL-181: NaN values left out by default).
fn extremes(
    img: &[f32],
    mask: Option<&Vec<bool>>,
    fix: bool,
    notices: &mut Notices,
    o: &str,
    json: bool,
) -> Option<Extremes> {
    let rep = extremes_pass(img, mask, false);
    if !fix {
        return rep;
    }
    let ours = extremes_pass(img, mask, true);
    if let (Some(a), Some(b)) = (ours, rep) {
        let differ = match o {
            "-R" => shown(json, &[a.lo, a.hi], &[b.lo, b.hi]),
            "-x" => a.hi_at != b.hi_at,
            _ => a.lo_at != b.lo_at,
        };
        if differ {
            notices.add(
                &defects::FSL_181,
                format!(
                    "{o}: NaN values left out of the extremes (FSL's start from a NaN first value)"
                ),
            );
        }
    }
    ours
}

/// The file's voxel coordinates of the value at `i` (FSL's frame's x mapped back).
fn file_coords(img: &Image, i: usize) -> [i64; 3] {
    let [nx, ny, nz, _] = img.dims;
    let v = i % (nx * ny * nz);
    let (x, y, z) = (v % nx, (v / nx) % ny, v / (nx * ny));
    let x = if img.flipped { nx - 1 - x } else { x };
    [x as i64, y as i64, z as i64]
}

/// `-p`, `-P`: the value at `⌊count · p⌋` of the sorted values (FSL-181: NaN values left out by
/// default; with `reproduce` FSL's sort is replayed, refused where it would leave the values).
fn percentile(
    mut vals: Vec<f32>,
    p: f32,
    notices: &mut Notices,
    o: &str,
    json: bool,
    err: &mut dyn Write,
) -> Result<f32, Exit> {
    let fix = notices.fix();
    let pick = |v: &mut Vec<f32>| -> Option<f32> {
        let n = v.len();
        if n == 0 {
            return Some(0.0);
        }
        let x = (n as f32 * p).floor();
        let k = if x.is_nan() {
            0
        } else {
            (cvtt(f64::from(x)).max(0) as usize).min(n - 1)
        };
        let signed_zeros = v.iter().any(|z| *z == 0.0 && z.is_sign_negative())
            && v.iter().any(|z| *z == 0.0 && z.is_sign_positive());
        if v.iter().any(|z| z.is_nan()) || signed_zeros {
            // libstdc++'s order (NaN, or ±0 whose order the comparison leaves open)
            if !std_sort_checked(v, &|a: &f32, b: &f32| a < b) {
                return None;
            }
            return Some(v[k]);
        }
        let (_, x, _) = v.select_nth_unstable_by(k, f32::total_cmp);
        Some(*x)
    };
    let has_nan = vals.iter().any(|v| v.is_nan());
    if fix && has_nan {
        let mut theirs = vals.clone();
        let rep = pick(&mut theirs);
        vals.retain(|v| !v.is_nan());
        let ours = pick(&mut vals).unwrap_or(0.0);
        if rep.is_none_or(|r| shown(json, &[ours], &[r])) {
            notices.add(
                &defects::FSL_181,
                format!("{o}: the percentile of the values that are not NaN (FSL sorts the NaN values with them)"),
            );
        }
        return Ok(ours);
    }
    match pick(&mut vals) {
        Some(v) => Ok(v),
        None => {
            let _ = writeln!(
                err,
                "larmorx: error: fslstats: {o}: values holding NaN, whose sort reads outside \
                 them in FSL (FSL-181: FSL's result is undefined here)"
            );
            Err(Exit(CRASHED))
        }
    }
}

/// A histogram's image, mask and bins.
struct HistCall<'a> {
    img: &'a [f32],
    mask: Option<&'a Vec<bool>>,
    nbins: usize,
}

/// What a histogram gives: no kept value, no histogram (`hi <= lo`), or its counts.
#[derive(Clone, Debug, PartialEq)]
enum Hist {
    Empty,
    None,
    Counts(Vec<u64>),
}

/// The histogram of the kept values over `[lo, hi]` (§5.5): bin `max(0, min((int)(A v + B),
/// nbins − 1))`, `A = nbins / (hi − lo)` (the width in single precision); `skip_nan`: NaN values
/// left out (FSL-181's correction), else x86-64's conversion puts them in the first bin;
/// `last_bin`: a bin beyond `int` the last (FSL-187's correction), else the first.
fn histogram(c: &HistCall<'_>, lo: f32, hi: f32, skip_nan: bool, last_bin: bool) -> Hist {
    let nbins = c.nbins;
    let any = match c.mask {
        Some(m) => m.par_iter().any(|&b| b),
        None => !c.img.is_empty(),
    };
    if !any {
        return Hist::Empty;
    }
    if hi <= lo {
        return Hist::None;
    }
    let w = f64::from(num::nan32(hi, lo, hi - lo));
    let nb = nbins as f64;
    let a = nb / w;
    let b = nb * (-f64::from(lo)) / w;
    const B: usize = 1 << 15;
    let parts: Vec<Vec<u64>> = c
        .img
        .par_chunks(B)
        .enumerate()
        .map(|(k, chunk)| {
            let mut h = vec![0u64; nbins];
            for (j, &v) in chunk.iter().enumerate() {
                if c.mask.is_some_and(|m| !m[k * B + j]) || (skip_nan && v.is_nan()) {
                    continue;
                }
                let x = a * f64::from(v) + b;
                let bin = if last_bin && x >= 2_147_483_648.0 {
                    nbins - 1
                } else {
                    (cvtt(x).min(nbins as i32 - 1)).max(0) as usize
                };
                h[bin] += 1;
            }
            h
        })
        .collect();
    let mut counts = vec![0u64; nbins];
    for h in parts {
        for (x, y) in counts.iter_mut().zip(h) {
            *x += y;
        }
    }
    Hist::Counts(counts)
}

/// Whether a kept value is NaN.
fn kept_nan(c: &HistCall<'_>) -> bool {
    c.img
        .par_iter()
        .enumerate()
        .any(|(i, v)| v.is_nan() && c.mask.is_none_or(|m| m[i]))
}

/// Names the defects that part a corrected histogram (or a value from it) from FSL's.
fn name_histogram_defects(notices: &mut Notices, o: &str, nan_changes: bool, far_changes: bool) {
    if nan_changes {
        notices.add(
            &defects::FSL_181,
            format!("{o}: NaN values left out (FSL counts them in its histogram's first bin)"),
        );
    }
    if far_changes {
        notices.add(
            &defects::FSL_187,
            format!(
                "{o}: values far above the histogram's range counted in its last bin (FSL counts \
                 them in its first)"
            ),
        );
    }
}

/// `-h`, `-H`: the histogram over `ours` (FSL's range `theirs`, which a NaN first value makes
/// NaN for `-h`), one bin a line, then the option's blank.
fn print_histogram(
    p: &mut Printed,
    o: &str,
    c: &HistCall<'_>,
    ours: (f32, f32),
    theirs: (f32, f32),
    notices: &mut Notices,
    err: &mut dyn Write,
) {
    let fix = notices.fix();
    let h = histogram(c, ours.0, ours.1, fix, fix);
    if fix && h != Hist::Empty {
        let rep = histogram(c, theirs.0, theirs.1, false, false);
        if rep != h {
            // the NaN values (with the range they part) alone, then the far values alone
            let nan_only = histogram(c, ours.0, ours.1, true, false);
            let nan_changes = kept_nan(c) || ours.0.to_bits() != theirs.0.to_bits();
            name_histogram_defects(notices, o, nan_changes && nan_only != rep, nan_only != h);
        }
    }
    // an empty mask: FSL's message, then its histogram of zeros (random testing, findings 28)
    let h = match h {
        Hist::Empty => {
            let _ = writeln!(err, "{HISTOGRAM_EMPTY}");
            Hist::Counts(vec![0; c.nbins])
        }
        other => other,
    };
    match h {
        Hist::None | Hist::Empty => {
            if o == "-H" {
                notices.add(
                    &defects::FSL_186,
                    "-H with a maximum not above its minimum prints nothing, as FSL does",
                );
            }
            // no histogram: no counts printed, an empty list in the JSON (findings 28)
            p.json.insert(o.into(), J::A(Vec::new()));
        }
        Hist::Counts(counts) => {
            for &n in &counts {
                p.text.push_str(&format!("{} \n", fmt::f(n as f64)));
            }
            p.json.insert(
                o.into(),
                J::A(counts.iter().map(|&n| J::F(n as f64)).collect()),
            );
        }
    }
    p.text.push(' ');
}

/// `-e`, `-E`'s value from a 1000-bin histogram over `range`: `None` for an empty range.
fn entropy_of(c: &HistCall<'_>, range: (f32, f32), skip_nan: bool, last_bin: bool) -> Option<f64> {
    let (lo, hi) = range;
    if hi == lo {
        return None;
    }
    let Hist::Counts(counts) = histogram(c, lo, hi, skip_nan, last_bin) else {
        return Some(0.0);
    };
    let n: u64 = counts.iter().sum();
    let mut e = 0.0f64;
    for &k in &counts {
        if k > 0 {
            let q = k as f64 / n as f64;
            e -= q * math::log(q);
        }
    }
    Some(e / math::log(1000.0))
}

/// `-e`, `-E` (§5.5): the entropy of a 1000-bin histogram over the robust range of every value
/// (FSL-181, FSL-185, FSL-187).
fn entropy(
    img: &[f32],
    mask: Option<&Vec<bool>>,
    o: &str,
    notices: &mut Notices,
    json: bool,
    err: &mut dyn Write,
) -> Result<f64, Exit> {
    let fix = notices.fix();
    let range = ops::robust_range::<f32>(img, None, fix);
    let c = HistCall {
        img,
        mask,
        nbins: 1000,
    };
    if mask.is_some_and(|m| !m.par_iter().any(|&b| b)) {
        let _ = writeln!(err, "{HISTOGRAM_EMPTY}");
    }
    let active = if fix { range.fix } else { range.rep }.unwrap_or((0.0, 0.0));
    let Some(ours) = entropy_of(&c, active, fix, fix) else {
        // FSL-185: an empty robust range
        if fix {
            notices.add(
                &defects::FSL_185,
                format!("{o} of an image whose robust range is empty: the entropy 0 (FSL crashes)"),
            );
            return Ok(0.0);
        }
        notices.add(
            &defects::FSL_185,
            format!("{o}: refused, as FSL crashes here"),
        );
        let _ = writeln!(
            err,
            "larmorx: error: fslstats: {o}: the robust range is empty (FSL-185: FSL crashes \
             here, an integer division by zero)"
        );
        return Err(Exit(CRASHED));
    };
    if fix {
        let theirs = entropy_of(&c, range.rep.unwrap_or((0.0, 0.0)), false, false);
        if theirs.is_none_or(|t| shown64(json, ours, t)) {
            let nan_only = entropy_of(&c, active, true, false);
            let nan_changes = kept_nan(&c) || range.causes.iter().any(|x| x.0.id == "FSL-153");
            let far_changes = nan_only.is_none_or(|v| shown64(json, ours, v))
                || range.causes.iter().any(|x| x.0.id == "FSL-187");
            name_histogram_defects(notices, o, nan_changes, far_changes);
        }
    }
    Ok(ours)
}

/// `-w` (§5.6): the smallest box holding the nonzero mask's voxels, in the file's coordinates.
fn bounding_box(input: &Image, pass: &Pass, fix: bool, notices: &mut Notices) -> [i64; 8] {
    let [nx, ny, nz, nt] = pass.dims;
    let nvox = nx * ny * nz;
    let m = pass.nonzero.as_ref();
    let found = m.map(|m| {
        m.par_iter()
            .enumerate()
            .filter(|(_, b)| **b)
            .map(|(i, _)| {
                let v = i % nvox;
                let c = [v % nx, (v / nx) % ny, v / (nx * ny), i / nvox];
                (c, c)
            })
            .reduce_with(|(a, b), (c, d)| {
                (
                    [0, 1, 2, 3].map(|k| a[k].min(c[k])),
                    [0, 1, 2, 3].map(|k| b[k].max(d[k])),
                )
            })
    });
    let Some(Some((lo, hi))) = found else {
        // no voxel: x, y, z the whole image; t FSL-188's
        let (t0, ts) = if fix {
            (0, nt as i64)
        } else {
            (nt as i64 - 1, 2 - nt as i64)
        };
        if nt > 1 {
            notices.add(
                &defects::FSL_188,
                if fix {
                    "-w without nonzero voxels: the whole time range (FSL's is a first volume of \
                     tsize - 1 with a size of 2 - tsize)"
                } else {
                    "-w without nonzero voxels: FSL's time range"
                },
            );
        }
        return [0, nx as i64, 0, ny as i64, 0, nz as i64, t0, ts];
    };
    let (mut x0, mut x1) = (lo[0], hi[0]);
    if input.flipped {
        (x0, x1) = (nx - 1 - hi[0], nx - 1 - lo[0]);
    }
    let size = |a: usize, b: usize| (b - a + 1) as i64;
    [
        x0 as i64,
        size(x0, x1),
        lo[1] as i64,
        size(lo[1], hi[1]),
        lo[2] as i64,
        size(lo[2], hi[2]),
        lo[3] as i64,
        size(lo[3], hi[3]),
    ]
}

/// A double sum in blocks of `b + 1` values, `b = max(⌊√n⌋, min_b)`.
fn blocks(vals: impl Iterator<Item = f64>, n: usize, min_b: usize) -> f64 {
    let b = (((n as f64).sqrt().floor()) as usize).max(min_b);
    let (mut total, mut run) = (0.0f64, 0.0f64);
    let mut k = 0usize;
    for x in vals {
        run = num::nan64(run, x, run + x);
        k += 1;
        if k == b + 1 {
            total = num::nan64(total, run, total + run);
            run = 0.0;
            k = 0;
        }
    }
    num::nan64(total, run, total + run)
}

/// `-c`, `-C` (§5.7): the centre of gravity of the first volume, every value minus the image's
/// minimum, in the file's voxel coordinates or in mm.
fn centre(input: &Image, pass: &Pass, mm: bool, err: &mut dyn Write) -> [f64; 3] {
    let [nx, ny, nz, _] = pass.dims;
    let n3 = nx * ny * nz;
    let min = ops::extremes(&pass.img, false).map_or(0.0, |e| e.0);
    let w: Vec<f64> = pass.img[..n3]
        .par_iter()
        .map(|&v| f64::from(num::nan32(v, min, v - min)))
        .collect();
    let at = |axis: usize| {
        blocks(
            w.iter().enumerate().map(|(i, &x)| {
                let c = match axis {
                    0 => i % nx,
                    1 => (i / nx) % ny,
                    _ => i / (nx * ny),
                } as f64;
                num::nan64(x, c, x * c)
            }),
            n3,
            1000,
        )
    };
    let mut total = blocks(w.iter().copied(), n3, 1000);
    if total.abs() < 1e-5 {
        let _ = writeln!(err, "{COG_ZERO}");
        total = 1.0;
    }
    let mut c = [0, 1, 2].map(|k| {
        let s = at(k);
        num::nan64(s, total, s / total)
    });
    if input.flipped {
        c[0] = (nx as f64 - 1.0) - c[0];
    }
    if !mm {
        return c;
    }
    let h = &input.header;
    let m = if h.sform_code > 0 {
        Some(*h.sform().rows())
    } else if h.qform_code > 0 {
        h.qform().ok().map(|q| *q.rows())
    } else {
        None
    };
    match m {
        Some(m) => [0, 1, 2].map(|r| m[r][0] * c[0] + m[r][1] * c[1] + m[r][2] * c[2] + m[r][3]),
        None => {
            let v = voxel(input);
            [0, 1, 2].map(|k| f64::from(v[k]) * c[k])
        }
    }
}

/// `-k` (§4): the mask image binarised (`0.5 <= m <= FLT_MAX`), replacing both masks; FSL-183
/// for a 4D mask with `-t`.
fn mask_option(
    ctx: &mut Ctx<'_>,
    pass: &mut Pass,
    t: Option<usize>,
    name: &str,
    notices: &mut Notices,
    err: &mut dyn Write,
) -> Result<(), Exit> {
    let fix = notices.fix();
    let m = ctx.operand(name, notices, err)?;
    let [nx, ny, nz, nt] = pass.dims;
    let nvox = nx * ny * nz;
    let mt = m.dims[3];
    let series_4d = t.is_some() && mt > 1;
    if m.dims[..3] != [nx, ny, nz] || (mt > nt && !(series_4d && fix)) {
        let _ = writeln!(err, "Mask and image must be the same size");
        return Err(Exit(3));
    }
    if series_4d {
        notices.add(
            &defects::FSL_183,
            "-t with a 4D -k mask: each time point's own mask volume (FSL refuses the mask)",
        );
    }
    let vol_of = |tv: usize| match t {
        Some(tp) if series_4d => tp.min(mt - 1),
        _ => tv.min(mt - 1),
    };
    let data = &m.data;
    let mask: Vec<bool> = (0..pass.img.len())
        .into_par_iter()
        .map(|i| {
            let v = data[vol_of(i / nvox) * nvox + i % nvox];
            (0.5..=f32::MAX).contains(&v)
        })
        .collect();
    pass.img
        .par_iter_mut()
        .zip(&mask)
        .for_each(|(v, &k)| *v = keep(*v, k));
    pass.nonzero = Some(nonzero_of(&pass.img, &mask));
    pass.mask = Some(mask);
    Ok(())
}

/// `-d` (§4): `I − image`; FSL-183 with `-t`.
fn difference(
    ctx: &mut Ctx<'_>,
    pass: &mut Pass,
    t: Option<usize>,
    name: &str,
    notices: &mut Notices,
    err: &mut dyn Write,
) -> Result<(), Exit> {
    let fix = notices.fix();
    let d = ctx.operand(name, notices, err)?;
    let [nx, ny, nz, nt] = pass.dims;
    let nvox = nx * ny * nz;
    let dt = d.dims[3];
    if d.dims[..3] != [nx, ny, nz] {
        let _ = writeln!(
            err,
            "Image used with -d must be the same size as the base image"
        );
        return Err(Exit(3));
    }
    let vol = match t {
        Some(tp) => {
            // FSL-183: FSL picks the time point's volume but never uses it
            if !fix {
                if dt > 1 {
                    let _ = writeln!(err, "Image must be the same size as the base image");
                    return Err(Exit(3));
                }
                if tp >= dt {
                    let what = "Invalid t index in [] operator";
                    let _ = writeln!(err, "Image Exception : #61 :: {what}\n{what}");
                    return Err(Exit(255));
                }
            } else if dt > 1 || tp >= dt {
                notices.add(
                    &defects::FSL_183,
                    "-t with -d: each time point's own volume of the difference image (FSL stops)",
                );
            }
            Some(tp.min(dt - 1))
        }
        None => {
            if dt > nt {
                let _ = writeln!(err, "Image must be the same size as the base image");
                return Err(Exit(3));
            }
            None
        }
    };
    let data = &d.data;
    pass.img.par_iter_mut().enumerate().for_each(|(i, v)| {
        let dv = match vol {
            Some(tv) => data[tv * nvox + i % nvox],
            None => data[(i / nvox).min(dt - 1) * nvox + i % nvox],
        };
        *v = num::nan32(*v, dv, *v - dv);
    });
    Ok(())
}

// ------------------------------------------------------------------------------------------
// JSON (§6.2)

fn json_document(doc: &[Vec<BTreeMap<String, J>>]) -> String {
    let mut s = String::from("{\n    \"volumes\": [");
    if doc.is_empty() {
        s.push_str("]\n}\n");
        return s;
    }
    s.push('\n');
    for (k, indices) in doc.iter().enumerate() {
        s.push_str("        {\n            \"indices\": [");
        if indices.is_empty() {
            s.push(']');
        } else {
            s.push('\n');
            for (j, entry) in indices.iter().enumerate() {
                s.push_str("                ");
                if entry.is_empty() {
                    s.push_str("{}");
                } else {
                    s.push_str("{\n");
                    for (q, (key, v)) in entry.iter().enumerate() {
                        s.push_str(&format!("                    \"{key}\": "));
                        value(&mut s, v, 5);
                        s.push_str(if q + 1 < entry.len() { ",\n" } else { "\n" });
                    }
                    s.push_str("                }");
                }
                s.push_str(if j + 1 < indices.len() { ",\n" } else { "\n" });
            }
            s.push_str("            ]");
        }
        s.push_str("\n        }");
        s.push_str(if k + 1 < doc.len() { ",\n" } else { "\n" });
    }
    s.push_str("    ]\n}\n");
    s
}

fn value(s: &mut String, v: &J, depth: usize) {
    match v {
        J::F(x) => s.push_str(&fmt::json(*x)),
        J::I(x) => s.push_str(&x.to_string()),
        J::A(items) => {
            if items.is_empty() {
                s.push_str("[]");
                return;
            }
            s.push_str("[\n");
            let pad = "    ".repeat(depth + 1);
            for (k, x) in items.iter().enumerate() {
                s.push_str(&pad);
                value(s, x, depth + 1);
                s.push_str(if k + 1 < items.len() { ",\n" } else { "\n" });
            }
            s.push_str(&"    ".repeat(depth));
            s.push(']');
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_and_json() {
        let v: Vec<f64> = (0..3000).map(f64::from).collect();
        assert_eq!(blocks(v.iter().copied(), 3000, 1000), 3000.0 * 2999.0 / 2.0);
        let doc = vec![vec![BTreeMap::from([(
            "-r".to_owned(),
            J::A(vec![J::F(-1.0), J::F(2.5)]),
        )])]];
        assert_eq!(
            json_document(&doc),
            "{\n    \"volumes\": [\n        {\n            \"indices\": [\n                {\n                    \"-r\": [\n                        -1.0,\n                        2.5\n                    ]\n                }\n            ]\n        }\n    ]\n}\n"
        );
    }
}
