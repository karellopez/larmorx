// SPDX-License-Identifier: LicenseRef-FSL
// Translated to Rust from FSL 6.0.7.23: fsl-avwutils 2209.6 (fslmaths.cc: main, inputParser,
// loadNewImage, check_for_output_name). Copyright (C) 2000-2008 University of Oxford (Steve
// Smith, David Flitney, Mark Jenkinson, Stuart Clare, Thomas Nichols and Matthew Webster). Rust
// translation Copyright 2026 Karel Lopez Vilaret; modified from the originals. Distributed under
// the FSL Licence (see LICENSE).
//! `fslmaths`: the input read in the internal type, the operations applied left to right, the
//! output written in the output type.
//!
//! - **Types.** `-dt char|short|int|float|double|input` picks the internal type (default
//!   `float`, `double` for a double input); `-odt` the output type (default `float`, `double`
//!   for a double input). `float` and `double` results are rounded (half away from zero)
//!   before an integer output type; every conversion is C's (x86-64's).
//! - **Operations.** The binary operations with a number or an image (a 3D image cloned over
//!   a 4D one's time points), the thresholds (absolute, of the robust range, of the positive
//!   voxels' robust range), the voxelwise functions, the random numbers (`-seed`, `-rand`,
//!   `-randn`), `-save`, `-restart`, `-range` and the undocumented `-pow` and `-invbin`.
//! - **Kernels and spatial filters.** `-kernel` (3D, 2D, box, boxv, boxv3, gauss, sphere,
//!   file), the dilations, erosions and median (`morphfilter`), `-dilall`, the convolutions
//!   (`-s`, `-fmean`, `-fmeanu`: separable, direct or by the FFT), `-subsamp2`, `-subsamp2offc`,
//!   `-edge`, `-dog_edge`, `-fillh`, `-fillh26`, `-inm`, `-ing`, `-tensor_decomp`
//!   (`larmorx_nc_core::filters`, `kissfft`).
//! - **Along dimensions and statistics** (`stats`): `-T`, `-X`, `-Y`, `-Z` reductions, `-pval`,
//!   `-pval0`, `-cpval`, `-ztop`, `-ptoz`, `-rank`, `-ranknorm`, `-roi`, `-bptf`.
//! - **Threshold-free cluster enhancement and ROC** (`tfce`): `-tfce`, `-tfceS`, `-tfce_minT`,
//!   `-tfce_delta`, `-roc`.
//! - **D13.** FSL-151 (conversions to integers), FSL-152 (the `max() + 1` bound), FSL-153 (a
//!   NaN first voxel), FSL-154 (uint8 in the signed `char` template), FSL-155 (`-dt input` of a
//!   type without its own), FSL-156 (integer division traps), FSL-157 (`-odt input` on int8 and
//!   uint16), FSL-158 (arguments read past the command line), FSL-159 (images of
//!   another size), FSL-160 (a zero-width Gaussian), FSL-161 (a kernel without voxels), FSL-162
//!   (a NaN in a sorted window), FSL-163 (4D subsampling), FSL-164 (4D `-dog_edge`), FSL-165
//!   (`-fillh` of one slice), FSL-166 (a non-finite tensor), FSL-167 to FSL-172 (`stats`'
//!   defects), FSL-173 to FSL-180 (`tfce`'s), FSL-187 (the robust range's bins), GLIBC-07
//!   (glibc's double functions: correctly rounded by default): corrected by default, reproduced
//!   with `LARMORX_UPSTREAM_DEFECTS=reproduce` (crashes are refused in both modes).

use std::io::Write;

use larmorx_core::defects::{self, Defect, Notices, UpstreamDefects};
use larmorx_core::math;
use larmorx_nc_core::arma::Mat;
use larmorx_nc_core::cnum;
use larmorx_nc_core::cprob::{self, Drand};
use larmorx_nc_core::crand::Rand;
use larmorx_nc_core::filters::{self, ConvStop, Kern, Morph};
use larmorx_nc_core::glibc_double;
use larmorx_nc_core::miscmaths;
use larmorx_nc_core::nifti::{self, dt};
use larmorx_nc_core::settings::Settings;
use larmorx_nc_core::volops::{self, Arith, Op};
use larmorx_nc_core::volume::{self, Elem, FslError, SaveError, SaveOptions, Volume};

use crate::usage;

mod stats;
mod tfce;

/// How the program runs: FSL's environment settings and D13's mode.
pub struct Run<'a> {
    pub fsl: &'a Settings,
    pub mode: UpstreamDefects,
}

/// How the program ends early: an exit status (with what was printed already written), or an
/// exception nothing catches (`std::terminate`).
#[derive(Debug)]
pub enum Stop {
    Exit(u8),
    Exception(FslError),
}

impl From<FslError> for Stop {
    fn from(e: FslError) -> Stop {
        Stop::Exception(e)
    }
}

type Res<T> = Result<T, Stop>;

/// The program's streams, D13's notices and the random-number state.
pub struct Ctx<'a, 'w> {
    run: &'a Run<'a>,
    out: &'w mut dyn Write,
    err: &'w mut dyn Write,
    notices: Notices,
    drand: Drand,
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
        self.notices.add(d, detail);
    }

    /// A crash of FSL's (both modes refuse): the message and exit status 1.
    fn refuse(&mut self, d: &'static Defect, what: &str) -> Stop {
        self.notice(d, what);
        let line = format!(
            "larmorx-nc: error: fslmaths: {what} ({}: FSL crashes here)\n",
            d.id
        );
        self.eput(&line);
        Stop::Exit(1)
    }

    /// `(T) x` of a double: C's conversion, undefined for an integer type out of its range or
    /// for NaN (FSL-151: clamped, NaN as 0, by default).
    fn conv<T: Arith>(&mut self, x: f64, what: &str) -> T {
        if T::IS_FLOAT {
            return from_f64_nan(x);
        }
        let (lo, hi) = int_range::<T>();
        let defined = !x.is_nan() && x > lo - 1.0 && x < hi + 1.0;
        if defined {
            return T::from_f64(x);
        }
        self.notice(
            &defects::FSL_151,
            format!(
                "{what}: {} outside the internal type's range",
                cnum::fmt_g(x, 6)
            ),
        );
        if self.reproduce() {
            T::from_f64(x)
        } else if x.is_nan() {
            T::from_int(0)
        } else {
            T::from_f64(x.clamp(lo, hi))
        }
    }

    /// `(int) v` of a value: undefined out of `int`'s range or for NaN (FSL-151).
    fn int_of<T: Arith>(&mut self, v: T, what: &str) -> i32 {
        if T::int_defined(v) {
            return T::to_int(v);
        }
        self.notice(
            &defects::FSL_151,
            format!("{what}: a value outside int's range or NaN"),
        );
        if self.reproduce() {
            T::to_int(v)
        } else {
            T::to_int_clamped(v)
        }
    }
}

/// [`Ctx::conv`] without its notice: `(T) x` as `reproduce` (C's conversion) or the default
/// (clamped, NaN as 0) gives it.
fn quiet_conv<T: Arith>(x: f64, reproduce: bool) -> T {
    if T::IS_FLOAT {
        return from_f64_nan(x);
    }
    let (lo, hi) = int_range::<T>();
    if reproduce || (!x.is_nan() && x > lo - 1.0 && x < hi + 1.0) {
        T::from_f64(x)
    } else if x.is_nan() {
        T::from_int(0)
    } else {
        T::from_f64(x.clamp(lo, hi))
    }
}

/// `(T) x` for the floating types, a NaN keeping its sign and payload's top bits as
/// `cvtsd2ss` keeps them (on every platform).
fn from_f64_nan<T: Arith>(x: f64) -> T {
    if x.is_nan() && T::DTYPE == dt::FLOAT {
        let b = x.to_bits();
        let sign = ((b >> 63) as u32) << 31;
        let payload = ((b >> 29) as u32) & 0x003f_ffff;
        return T::from_f32(f32::from_bits(sign | 0x7fc0_0000 | payload));
    }
    T::from_f64(x)
}

/// The range of an integer element type as `double`.
fn int_range<T: Arith>() -> (f64, f64) {
    match T::DTYPE {
        dt::UNSIGNED_CHAR => (-128.0, 127.0),
        dt::SIGNED_SHORT => (-32768.0, 32767.0),
        _ => (-2_147_483_648.0, 2_147_483_647.0),
    }
}

/// An argument as FSL's `argv` gives it: a string, the terminating null pointer, or beyond it
/// (the environment's strings).
enum Arg<'s> {
    Str(&'s str),
    Null,
    Beyond,
}

fn arg(argv: &[String], k: usize) -> Arg<'_> {
    match k.cmp(&argv.len()) {
        std::cmp::Ordering::Less => Arg::Str(&argv[k]),
        std::cmp::Ordering::Equal => Arg::Null,
        std::cmp::Ordering::Greater => Arg::Beyond,
    }
}

/// `std::string(argv[k])`: a null pointer throws `std::logic_error`; past it FSL reads the
/// environment (FSL-158: refused).
fn string_arg<'s>(ctx: &mut Ctx, argv: &'s [String], k: usize) -> Res<&'s str> {
    match arg(argv, k) {
        Arg::Str(s) => Ok(s),
        Arg::Null => {
            ctx.notice(&defects::FSL_158, "an argument is missing");
            Err(Stop::Exception(FslError {
                printed: Vec::new(),
                type_name: "std::logic_error",
                what: "basic_string: construction from null is not valid".into(),
            }))
        }
        Arg::Beyond => Err(ctx.refuse(&defects::FSL_158, "arguments are missing at the end")),
    }
}

/// `atof(argv[k])` / `atoi(argv[k])`: a null pointer or what lies past it is a crash
/// (FSL-158: refused).
fn number_arg<'s>(ctx: &mut Ctx, argv: &'s [String], k: usize) -> Res<&'s str> {
    match arg(argv, k) {
        Arg::Str(s) => Ok(s),
        _ => Err(ctx.refuse(&defects::FSL_158, "an operation's arguments are missing")),
    }
}

/// miscmaths' `isNumber`: not empty, and `strtod` reads all of it.
pub fn is_number(s: &str) -> bool {
    !s.is_empty() && cnum::strtod(s).1 == s.len()
}

/// `dtype(filename)`: -1 for an empty name.
fn file_dtype(ctx: &Ctx, name: &str) -> Result<i16, FslError> {
    volume::file_dtype(name, ctx.run.fsl)
}

/// fslmaths' `main`: `argv[0]` is the program's name.
pub fn main(argv: &[String], run: &Run, out: &mut dyn Write, err: &mut dyn Write) -> Res<u8> {
    let mut ctx = Ctx {
        run,
        out,
        err,
        notices: Notices::new(run.mode),
        drand: Drand::default(),
    };
    let r = main_inner(argv, &mut ctx);
    let lines: Vec<String> = ctx.notices.as_slice().iter().map(|n| n.line()).collect();
    for l in lines {
        ctx.eput(&format!("larmorx-nc: {l}\n"));
    }
    let _ = ctx.out.flush();
    let _ = ctx.err.flush();
    r
}

fn main_inner(argv: &[String], ctx: &mut Ctx) -> Res<u8> {
    let argc0 = argv.len();
    if argc0 < 2 {
        ctx.put(usage::FSLMATHS);
        return Ok(1);
    }
    if argv[1] == "-h" || argv[1] == "--help" {
        ctx.put(usage::FSLMATHS);
        return Ok(0);
    }
    let is_dt = argv[1] == "-datatype" || argv[1] == "-dt";
    let mut input_type = if is_dt {
        let name = string_arg(ctx, argv, 3)?;
        file_dtype(ctx, name)?
    } else {
        file_dtype(ctx, &argv[1])?
    };
    // FSL-154: the char template of a uint8 image, corrected, computes in unsigned bytes
    let unsigned = input_type == dt::UNSIGNED_CHAR && !ctx.reproduce();
    let mut output_type = dt::FLOAT;
    if input_type == dt::DOUBLE {
        output_type = dt::DOUBLE;
    }
    let mut argc = argc0;
    if argv[argc - 2] == "-output_datatype" || argv[argc - 2] == "-odt" {
        output_type = match argv[argc - 1].as_str() {
            "input" => input_type,
            "char" => dt::UNSIGNED_CHAR,
            "short" => dt::SIGNED_SHORT,
            "int" => dt::SIGNED_INT,
            "float" => dt::FLOAT,
            "double" => dt::DOUBLE,
            other => {
                ctx.put(&format!(
                    "Error: Unknown datatype \"{other}\" - Possible datatypes are: char short int float double input\n"
                ));
                return Ok(1);
            }
        };
        argc -= 2;
    }
    if is_dt {
        // inputParser<T>(argc-2, argv+2): the type's name stands where the program's was
        let name = argv[2].as_str();
        if name != "input" {
            input_type = -1;
        }
        let sub = Args {
            argv: &argv[2..],
            argc: argc - 2,
        };
        let chosen = match name {
            "char" => dt::UNSIGNED_CHAR,
            "short" => dt::SIGNED_SHORT,
            "int" => dt::SIGNED_INT,
            "float" => dt::FLOAT,
            "double" => dt::DOUBLE,
            _ => match input_type {
                dt::UNSIGNED_CHAR | dt::SIGNED_SHORT | dt::SIGNED_INT | dt::FLOAT | dt::DOUBLE => {
                    input_type
                }
                -1 => {
                    ctx.put(&format!(
                        "Error: Unknown datatype \"{name}\" - Possible datatypes are: char short int float double input\n"
                    ));
                    return Ok(1);
                }
                other => {
                    // FSL-155: no branch takes the type; main ends without a word
                    ctx.notice(
                        &defects::FSL_155,
                        format!("-dt input of an image of datatype {other}"),
                    );
                    if ctx.reproduce() {
                        return Ok(0);
                    }
                    match volume::closest_templated_type(other) {
                        Ok(t) => t,
                        Err(msg) => {
                            ctx.eput(&format!("{msg}\n"));
                            return Ok(1);
                        }
                    }
                }
            },
        };
        if chosen == dt::UNSIGNED_CHAR && unsigned {
            return input_parser::<u8>(&sub, output_type, ctx);
        }
        return dispatch(chosen, &sub, output_type, ctx);
    }
    let sub = Args { argv, argc };
    if file_dtype(ctx, &argv[1])? == dt::DOUBLE {
        dispatch(dt::DOUBLE, &sub, output_type, ctx)
    } else {
        dispatch(dt::FLOAT, &sub, output_type, ctx)
    }
}

/// The parser's `argc` and `argv` (an `argv` longer than `argc` after `-odt` is taken off: FSL
/// reads past `argc` into it).
struct Args<'s> {
    argv: &'s [String],
    argc: usize,
}

fn dispatch(t: i16, a: &Args, output_dt: i16, ctx: &mut Ctx) -> Res<u8> {
    match t {
        dt::UNSIGNED_CHAR => input_parser::<i8>(a, output_dt, ctx),
        dt::SIGNED_SHORT => input_parser::<i16>(a, output_dt, ctx),
        dt::SIGNED_INT => input_parser::<i32>(a, output_dt, ctx),
        dt::DOUBLE => input_parser::<f64>(a, output_dt, ctx),
        _ => input_parser::<f32>(a, output_dt, ctx),
    }
}

fn read4d<T: Arith>(ctx: &mut Ctx, name: &str) -> Result<Volume<T>, FslError> {
    let v = volume::read_volume4d::<T>(name, ctx.run.fsl)?.0;
    if !T::IS_FLOAT && T::MAXV.to_f64() == 255.0 {
        // the unsigned template of FSL-154's correction: the values FSL's char reads as
        // negative, and every intermediate value of the chain held as an unsigned byte where
        // FSL's signed char holds it otherwise (a difference below 0, a sum above 127: a
        // difference of Gaussians' signs, random testing 2026-11-03), so named for every image
        // the unsigned template reads
        let n = v.data.iter().filter(|x| x.to_f64() >= 128.0).count();
        ctx.notice(
            &defects::FSL_154,
            format!(
                "{name}: computed as unsigned bytes ({n} values from 128 to 255, which FSL's \
                 signed char reads as negative; intermediate values below 0 or above 127 are \
                 held as FSL's signed char does not hold them)"
            ),
        );
    }
    Ok(v)
}

/// `loadNewImage(oldI, newI, filename)`: the image read; a 3D current image cloned over the
/// new one's time points; the orientations compared (a warning beyond 0.5 mm RMS).
fn load_new_image<T: Arith>(ctx: &mut Ctx, old: &mut Volume<T>, name: &str) -> Res<Volume<T>> {
    let new = read4d::<T>(ctx, name)?;
    if old.tsize() == 1 && old.tsize() < new.tsize() {
        let nvox = old.nvoxels() as usize;
        let mut v = Volume::<T>::new(old.xsize(), old.ysize(), old.zsize(), new.tsize());
        v.copy_properties(old);
        for t in 0..new.tsize() as usize {
            v.data[t * nvox..(t + 1) * nvox].copy_from_slice(&old.data[..nvox]);
        }
        *old = v;
    }
    let known = |v: &Volume<T>| {
        v.sform_code != nifti::XFORM_UNKNOWN || v.qform_code != nifti::XFORM_UNKNOWN
    };
    if known(&new) && known(old) {
        let centre = Mat::col(&[0.0, 0.0, 0.0]);
        let rms = miscmaths::rms_deviation(
            &new.newimagevox2mm_mat(),
            &old.newimagevox2mm_mat(),
            &centre,
            80.0,
        )
        .map_err(|e| {
            Stop::Exception(FslError {
                printed: Vec::new(),
                type_name: "std::runtime_error",
                what: e.to_string(),
            })
        })?;
        if rms > 0.5 {
            ctx.eput(
                "\nWARNING:: Inconsistent orientations for individual images in pipeline!\n          Will use voxel-based orientation which is probably incorrect - *PLEASE CHECK*!\n\n",
            );
        }
    }
    Ok(new)
}

/// The values of the first `tsize` volumes (fslmaths' explicit x, y, z, t loops).
fn looped_len<T: Elem>(v: &Volume<T>) -> usize {
    (v.nvoxels() * v.tsize()).max(0) as usize
}

/// `max()` and `min()` as FSL takes them (from the first voxel: a NaN first voxel makes both
/// NaN, FSL-153), and with the NaN voxels left out (larmorx's correction).
struct Extremes<T> {
    fsl: (T, T),
    fixed: (T, T),
}

impl<T: Arith> Extremes<T> {
    fn of(v: &Volume<T>) -> Extremes<T> {
        let fsl = v.extrema();
        let fixed = if T::IS_FLOAT {
            v.extrema_without_nan().unwrap_or(fsl)
        } else {
            fsl
        };
        Extremes { fsl, fixed }
    }

    /// Whether FSL's extremes are NaN where the corrected ones are not.
    fn nan_first(&self) -> bool {
        bits(self.fsl.0) != bits(self.fixed.0) || bits(self.fsl.1) != bits(self.fixed.1)
    }
}

fn bits<T: Elem>(x: T) -> u64 {
    x.to_f64().to_bits()
}

/// The range `threshold` and `binarise` test: `[lower, upper]` inclusive or `(lower, upper)`
/// exclusive; an `open` side is FSL's `min() - 1` or `max() + 1`, meant to bound nothing.
#[derive(Clone, Copy)]
struct Range<T> {
    lower: T,
    upper: T,
    inclusive: bool,
    lower_open: bool,
    upper_open: bool,
}

impl<T: Arith> Range<T> {
    /// FSL's test.
    fn fsl(&self, x: T) -> bool {
        if self.inclusive {
            x >= self.lower && x <= self.upper
        } else {
            x > self.lower && x < self.upper
        }
    }

    /// The intended test: an open side bounds nothing (but a NaN voxel is outside, as in FSL).
    fn intended(&self, x: T) -> bool {
        if T::is_nan(x) {
            return false;
        }
        let lo = self.lower_open
            || if self.inclusive {
                x >= self.lower
            } else {
                x > self.lower
            };
        let hi = self.upper_open
            || if self.inclusive {
                x <= self.upper
            } else {
                x < self.upper
            };
        lo && hi
    }
}

/// Which voxels are inside: FSL's range `fsl`; corrected, the intended test of `fixed` (the
/// same range with corrected limits). FSL-152 is named where the open sides change voxels,
/// FSL-153 where NaN extremes (`nan_first`) do, and each defect of `causes` (the corrections
/// of the limits: FSL-153's NaN voxels, FSL-187's bins) where the corrected limits do.
fn select<T: Arith>(
    ctx: &mut Ctx,
    v: &Volume<T>,
    fsl: &Range<T>,
    fixed: &Range<T>,
    nan_first: bool,
    causes: &[&'static Defect],
    what: &str,
) -> Vec<bool> {
    let r_fsl: Vec<bool> = v.data.iter().map(|&x| fsl.fsl(x)).collect();
    let r_mid: Vec<bool> = v.data.iter().map(|&x| fsl.intended(x)).collect();
    let r_fix: Vec<bool> = v.data.iter().map(|&x| fixed.intended(x)).collect();
    let n_open = r_fsl.iter().zip(&r_mid).filter(|(a, b)| a != b).count();
    let n_lim = r_mid.iter().zip(&r_fix).filter(|(a, b)| a != b).count();
    if n_open > 0 {
        if nan_first {
            ctx.notice(
                &defects::FSL_153,
                format!("{what}: the first voxel is NaN ({n_open} voxels)"),
            );
        } else {
            ctx.notice(
                &defects::FSL_152,
                format!(
                    "{what}: {n_open} voxels beyond the image's maximum plus 1 or minimum minus 1"
                ),
            );
        }
    }
    if n_lim > 0 {
        for &d in causes {
            let why = if d.id == defects::FSL_187.id {
                "values far above a pass's histogram in the robust range"
            } else {
                "NaN voxels in the robust range"
            };
            ctx.notice(d, format!("{what}: {why} ({n_lim} voxels)"));
        }
    }
    if ctx.reproduce() { r_fsl } else { r_fix }
}

/// `binarise(range)`: 1 inside, 0 outside.
fn binarise_r<T: Arith>(v: &mut Volume<T>, ins: Vec<bool>) {
    let (one, zero) = (T::from_int(1), T::from_int(0));
    for (x, i) in v.data.iter_mut().zip(ins) {
        *x = if i { one } else { zero };
    }
}

/// `threshold(range)`: 0 outside.
fn threshold_r<T: Arith>(v: &mut Volume<T>, ins: Vec<bool>) {
    let zero = T::from_int(0);
    for (x, i) in v.data.iter_mut().zip(ins) {
        if !i {
            *x = zero;
        }
    }
}

/// `binarise(lower, max() + 1, exclusive)` (`-bin`, `-binv`, `-mas`'s mask).
fn bin_above<T: Arith>(ctx: &mut Ctx, v: &mut Volume<T>, lower: T, what: &str) {
    let e = Extremes::of(v);
    let r = Range {
        lower,
        upper: T::plus_int(e.fsl.0, 1),
        inclusive: false,
        lower_open: false,
        upper_open: true,
    };
    let ins = select(ctx, v, &r, &r, e.nan_first(), &[], what);
    binarise_r(v, ins);
}

/// `threshold(lower, max() + 1, inclusive)` (`upper = false`) or
/// `threshold(min() - 1, upper, inclusive)`: `limit` FSL's bound and the corrected one.
fn threshold_at<T: Arith>(
    ctx: &mut Ctx,
    v: &mut Volume<T>,
    limit: (T, T),
    upper: bool,
    causes: &[&'static Defect],
    what: &str,
) {
    let e = Extremes::of(v);
    let make = |l: T| {
        if upper {
            Range {
                lower: T::plus_int(e.fsl.1, -1),
                upper: l,
                inclusive: true,
                lower_open: true,
                upper_open: false,
            }
        } else {
            Range {
                lower: l,
                upper: T::plus_int(e.fsl.0, 1),
                inclusive: true,
                lower_open: false,
                upper_open: true,
            }
        }
    };
    let (rf, rx) = (make(limit.0), make(limit.1));
    let ins = select(ctx, v, &rf, &rx, e.nan_first(), causes, what);
    threshold_r(v, ins);
}

/// FSL's robust limits, the corrected ones and the defects whose corrections changed them.
type Limits<T> = ((T, T), (T, T), Vec<&'static Defect>);

/// The robust range's limits (`robustmin()`, `robustmax()`, or with the mask of the positive
/// voxels) as FSL computes them and corrected: the NaN voxels left out (FSL-153) and values far
/// above a pass's histogram counted in its last bin (FSL-187); with the warning FSL prints for
/// an empty mask, and the defects whose corrections changed the limits.
fn robust<T: Arith>(ctx: &mut Ctx, v: &Volume<T>, mask: Option<&Volume<T>>) -> Res<Limits<T>> {
    let (r, warning) = v.robust_limits_masked(mask)?;
    if let Some(w) = warning {
        ctx.eput(&format!("{w}\n"));
    }
    let (lo, hi) = r.fsl;
    let bins = |r: &volops::RobustRange<T>| -> Vec<&'static Defect> {
        if r.differs() {
            vec![&defects::FSL_187]
        } else {
            Vec::new()
        }
    };
    if !T::IS_FLOAT || !v.data.iter().any(|&x| T::is_nan(x)) {
        return Ok(((lo, hi), r.fixed, bins(&r)));
    }
    let mut keep = Volume::<T>::new7(v.dims);
    let one = T::from_int(1);
    let m: &[T] = mask.map_or(&[], |m| m.data.as_slice());
    for (k, (d, &x)) in keep.data.iter_mut().zip(&v.data).enumerate() {
        let in_m = m.is_empty() || m[k % m.len()].to_f64() > volops::MASK_THRESHOLD;
        if in_m && !T::is_nan(x) {
            *d = one;
        }
    }
    let (fixed, mut causes) = if keep.no_mask_voxels() == 0 {
        ((T::from_int(0), T::from_int(0)), Vec::new())
    } else {
        let k = volops::robust_range(v, Some(&keep))?;
        (k.fixed, bins(&k))
    };
    causes.insert(0, &defects::FSL_153);
    Ok(((lo, hi), fixed, causes))
}

/// `robustmin + (p / 100) * (robustmax - robustmin)` converted to `T`.
fn robust_limit<T: Arith>(ctx: &mut Ctx, lo: T, hi: T, val: &str, what: &str) -> T {
    let p = cnum::atof(val) / 100.0;
    let x = lo.to_f64() + larmorx_nc_core::sse::mul_f64(p, T::diff(hi, lo));
    ctx.conv::<T>(x, what)
}

/// The bits a value converted to `T` is stored with (no notice: for comparisons only).
fn stored<T: Arith>(x: f64) -> u64 {
    let v: T = if T::IS_FLOAT {
        from_f64_nan(x)
    } else {
        T::from_f64(x)
    };
    v.to_f64().to_bits()
}

/// GLIBC-07's notice: `moved` voxels of `op` whose stored value glibc's last bit changes.
fn last_bit_notice(ctx: &mut Ctx, op: &str, moved: usize) {
    if moved > 0 {
        ctx.notice(
            &defects::GLIBC_07,
            format!("{op}: {moved} voxels whose value glibc's last bit changes"),
        );
    }
}

/// An integer division by zero, as FSL crashes on it (FSL-156: refused).
fn trap(ctx: &mut Ctx, what: &str) -> Stop {
    ctx.refuse(
        &defects::FSL_156,
        &format!("{what}: an integer division by zero"),
    )
}

/// FSL-159: an image of another size than the current one for an operation that indexes it
/// voxel by voxel. Corrected: refused as the other binary operations refuse it.
fn other_size<T: Arith>(
    ctx: &mut Ctx,
    cur: &Volume<T>,
    img: &Volume<T>,
    verb: &str,
    opname: &str,
) -> Res<()> {
    let same_xyz = cur.dims[..3] == img.dims[..3];
    let t_ok = img.tsize() == 1 || img.tsize() == cur.tsize();
    if same_xyz && t_ok {
        return Ok(());
    }
    ctx.notice(
        &defects::FSL_159,
        format!("{opname} with an image of another size"),
    );
    if ctx.reproduce() {
        return Ok(());
    }
    Err(Stop::Exception(FslError::imthrow(
        format!("Attempted to {verb} images of different sizes"),
        3,
    )))
}

fn input_parser<T: Arith>(a: &Args, output_dt: i16, ctx: &mut Ctx) -> Res<u8> {
    let argv = a.argv;
    let argc = a.argc;
    let mut input: Volume<T> = read4d::<T>(ctx, &argv[1])?;
    // the kernel of the spatial filters (default box_kernel(3,3,3)) and whether -fmean filters
    // separably; FSL-160: the kernel is a Gaussian of zero width
    let mut kernel = filters::box_kernel_vox(3, 3, 3);
    let mut separable = false;
    let mut zero_width = false;
    let mut set_display_range = false;
    // -tfce_minT and -tfce_delta
    let mut tfce_params = tfce::TfceParams::default();
    let mut modified = false;
    let mut i = 2usize;
    while i + 1 < argc {
        modified = true;
        let op = argv[i].as_str();
        let b = op.as_bytes();
        if b.len() >= 2 && b[1].is_ascii_uppercase() && b[0] == b'-' {
            if let Some(code) = stats::reduce(ctx, &mut input, argv, &mut i)? {
                return Ok(code);
            }
            i += 1;
            continue;
        }
        match op {
            "-pval" | "-pval0" => stats::pval(ctx, &mut input, op == "-pval0", op),
            "-cpval" => stats::cpval(ctx, &mut input),
            "-ztop" => stats::ztop(ctx, &mut input),
            "-ptoz" => stats::ptoz(ctx, &mut input),
            "-rank" | "-ranknorm" => stats::rank(ctx, &mut input, op == "-ranknorm")?,
            "-roi" => {
                stats::roi(ctx, &mut input, argv, i)?;
                i += 8;
            }
            "-bptf" => {
                stats::bptf(ctx, &mut input, argv, i)?;
                i += 2;
            }
            "-tfce_minT" => {
                i += 1;
                tfce_params.min_t = cnum::atof(number_arg(ctx, argv, i)?) as f32;
            }
            "-tfce_delta" => {
                i += 1;
                tfce_params.delta = cnum::atof(number_arg(ctx, argv, i)?) as f32;
            }
            "-tfce" => {
                if let Some(code) = tfce::tfce(ctx, &mut input, argv, i, tfce_params)? {
                    return Ok(code);
                }
                i += 3;
            }
            "-tfceS" => {
                tfce::tfce_support(ctx, &mut input, argv, i, tfce_params)?;
                i += 7;
            }
            "-roc" => i = tfce::roc(ctx, &input, argv, i)?,
            "-mas" => {
                i += 1;
                let name = string_arg(ctx, argv, i)?;
                let mut m = load_new_image(ctx, &mut input, name)?;
                bin_above(ctx, &mut m, T::from_int(0), "-mas");
                input.op_volume(Op::Mul, &m)?.ok();
            }
            "-add" | "-sub" | "-mul" => {
                i += 1;
                let opk = match op {
                    "-add" => Op::Add,
                    "-sub" => Op::Sub,
                    _ => Op::Mul,
                };
                let s = string_arg(ctx, argv, i)?;
                if is_number(s) {
                    let val = ctx.conv::<T>(cnum::atof(s), op);
                    let _ = input.op_scalar(opk, val);
                } else {
                    let img = load_new_image(ctx, &mut input, s)?;
                    input.op_volume(opk, &img)?.ok();
                }
            }
            "-rem" => {
                i += 1;
                let s = string_arg(ctx, argv, i)?;
                rem(ctx, &mut input, s)?;
            }
            "-div" => {
                i += 1;
                let s = string_arg(ctx, argv, i)?;
                div(ctx, &mut input, s)?;
            }
            "-max" | "-min" => {
                i += 1;
                let s = string_arg(ctx, argv, i)?;
                max_min(ctx, &mut input, s, op == "-max")?;
            }
            "-kernel" => {
                let (xd, yd, zd) = (input.xdim(), input.ydim(), input.zdim());
                let kind = string_arg(ctx, argv, i + 1)?;
                zero_width = false;
                match kind {
                    "2D" => kernel = filters::box_kernel_vox(3, 3, 1),
                    "3D" => kernel = filters::box_kernel_vox(3, 3, 3),
                    _ => {
                        let size_s = number_arg(ctx, argv, i + 2)?;
                        let size = cnum::atof(size_s) as f32;
                        separable = matches!(kind, "box" | "boxv" | "boxv3" | "gauss");
                        kernel = match kind {
                            "box" => filters::box_kernel_mm(size, xd, yd, zd),
                            "boxv" => {
                                let n = ctx.int_of::<f32>(size, "-kernel boxv");
                                filters::box_kernel_vox(n, n, n)
                            }
                            "boxv3" => {
                                let a = cnum::atof(number_arg(ctx, argv, i + 2)?);
                                let b = cnum::atof(number_arg(ctx, argv, i + 3)?);
                                let c = cnum::atof(number_arg(ctx, argv, i + 4)?);
                                i += 2;
                                filters::box_kernel_vox(
                                    ctx.int_of::<f64>(a, "-kernel boxv3"),
                                    ctx.int_of::<f64>(b, "-kernel boxv3"),
                                    ctx.int_of::<f64>(c, "-kernel boxv3"),
                                )
                            }
                            "gauss" => {
                                let g = filters::gaussian_kernel3d_mm(size, xd, yd, zd, 4.0, false);
                                zero_width = g.zero_width;
                                g.kernel
                            }
                            "sphere" => filters::spherical_kernel(size, xd, yd, zd),
                            "file" => {
                                let name = string_arg(ctx, argv, i + 2)?;
                                let (v, warning) = volume::read_volume::<f32>(name, ctx.run.fsl)?;
                                if let Some(w) = warning {
                                    ctx.eput(&format!("{w}\n"));
                                }
                                Kern::from_volume(&v)
                            }
                            // kernel.destroy() and nothing built
                            _ => Kern::filled(0, 0, 0, 0.0),
                        };
                        let known =
                            matches!(kind, "box" | "boxv" | "boxv3" | "gauss" | "sphere" | "file");
                        if !known || kernel.has_no_voxels() {
                            no_voxels(ctx, &format!("-kernel {kind}"), known)?;
                        }
                        i += 1;
                    }
                }
                i += 1;
            }
            "-s" => {
                i += 1;
                let sigma = cnum::atof(string_arg(ctx, argv, i)?) as f32;
                let (xd, yd, zd) = (input.xdim(), input.ydim(), input.zdim());
                let g = filters::gaussian_kernel3d_mm(sigma, xd, yd, zd, 4.0, false);
                kernel = g.kernel;
                zero_width = g.zero_width;
                separable = true;
                if kernel.has_no_voxels() {
                    no_voxels(ctx, "-s", true)?;
                }
                convolve(ctx, &mut input, &kernel, zero_width, true, true, op)?;
            }
            "-fmean" => convolve(ctx, &mut input, &kernel, zero_width, separable, true, op)?,
            "-fmeanu" => convolve(ctx, &mut input, &kernel, zero_width, false, false, op)?,
            "-dilM" | "-dilD" | "-dilF" | "-ero" | "-eroF" | "-fmedian" => {
                let kind = match op {
                    "-dilM" => Morph::DilateM,
                    "-dilD" => Morph::DilateD,
                    "-dilF" => Morph::Dilate,
                    "-ero" => Morph::ErodeS,
                    "-eroF" => Morph::Erode,
                    _ => Morph::Median,
                };
                morph(ctx, &mut input, &kernel, kind, op)?;
            }
            "-dilall" => {
                let nvox = input.nvoxels().max(0) as usize;
                let dims = [input.xsize(), input.ysize(), input.zsize()];
                for t in 0..input.tsize().max(0) as usize {
                    let src = input.data[t * nvox..(t + 1) * nvox].to_vec();
                    let out = filters::dilall(&src, dims, &mut |x| ctx.conv::<T>(x, op));
                    input.data[t * nvox..(t + 1) * nvox].copy_from_slice(&out);
                }
            }
            "-subsamp2" | "-subsamp2offc" => {
                let centred = op == "-subsamp2";
                subsample(ctx, &mut input, centred, op)?;
            }
            "-edge" => {
                input = filters::edge_strengthen(&input, &mut |x| ctx.conv::<T>(x, op));
            }
            "-dog_edge" => {
                i += 1;
                let s1 = cnum::atof(number_arg(ctx, argv, i)?) as f32;
                i += 1;
                let s2 = cnum::atof(number_arg(ctx, argv, i)?) as f32;
                if cnum::f32_to_i32(4.0 * s2) < 0 {
                    no_voxels(ctx, "-dog_edge", true)?;
                }
                let every = input.tsize() > 1 && !ctx.reproduce();
                let out =
                    filters::log_edge_detect(&input, s1, s2, every, &mut |x| ctx.conv::<T>(x, op));
                if input.tsize() > 1 {
                    // FSL-164 named where filtering every volume changes a voxel (the other
                    // mode's result, its conversions without notices)
                    let reproduce = ctx.reproduce();
                    let other = filters::log_edge_detect(&input, s1, s2, !every, &mut |x| {
                        quiet_conv::<T>(x, reproduce)
                    });
                    let moved = out
                        .data
                        .iter()
                        .zip(&other.data)
                        .filter(|(a, b)| a.to_f64().to_bits() != b.to_f64().to_bits())
                        .count();
                    if moved > 0 {
                        ctx.notice(
                            &defects::FSL_164,
                            format!(
                                "-dog_edge of a 4D image: {moved} voxels of volumes 1 to {} \
                                 differ when every volume is filtered",
                                input.tsize() - 1
                            ),
                        );
                    }
                }
                input = out;
            }
            "-fillh" | "-fillh26" => {
                let conn = if op == "-fillh" { 6 } else { 26 };
                bin_above(ctx, &mut input, T::from_int(0), op);
                let nvox = input.nvoxels().max(0) as usize;
                let dims = [input.xsize(), input.ysize(), input.zsize()];
                let traps = filters::fill_holes_traps(dims);
                if traps {
                    if ctx.reproduce() {
                        return Err(ctx.refuse(
                            &defects::FSL_165,
                            &format!(
                                "{op} of an image with a size of 1: an integer division by zero"
                            ),
                        ));
                    }
                    ctx.notice(
                        &defects::FSL_165,
                        format!("{op} of an image with a size of 1"),
                    );
                }
                for t in 0..input.tsize().max(0) as usize {
                    let r = &mut input.data[t * nvox..(t + 1) * nvox];
                    let out = filters::fill_holes(r, dims, conn, traps);
                    r.copy_from_slice(&out);
                }
                bin_above(ctx, &mut input, T::from_int(0), op);
            }
            "-inm" | "-ing" => {
                i += 1;
                let target = cnum::atof(number_arg(ctx, argv, i)?);
                let mut mask = input.clone();
                bin_above(ctx, &mut mask, T::from_int(0), op);
                let nvox = input.nvoxels().max(0) as usize;
                let nt = input.tsize().max(0) as usize;
                let blocks: Vec<(usize, usize)> = if op == "-inm" {
                    (0..nt).map(|t| (t * nvox, (t + 1) * nvox)).collect()
                } else {
                    vec![(0, nt * nvox)]
                };
                for (a, b) in blocks {
                    let (mean, empty) = filters::masked_mean(&input.data[a..b], &mask.data[a..b]);
                    if empty {
                        ctx.eput("ERROR:: Empty mask image\n");
                    }
                    let tmean = larmorx_nc_core::sse::div_f64(target, mean);
                    for k in a..b {
                        let v = larmorx_nc_core::sse::mul_f64(input.data[k].to_f64(), tmean);
                        input.data[k] = ctx.conv::<T>(v, op);
                    }
                }
            }
            "-tensor_decomp" => {
                if input.tsize() != 6 {
                    ctx.put("Error: input to tensor option is not a tensor!\n");
                    return Err(Stop::Exit(1));
                }
                let out = argv[argc - 1].clone();
                tensor_decomp(ctx, &mut input, &out)?;
            }
            "-seed" => {
                i += 1;
                let s = number_arg(ctx, argv, i)?;
                let mut r = Rand::new(cnum::atoi(s) as u32);
                // sdrand(rand(), rand(), rand()): GCC evaluates the arguments right to left
                let (c, b2, a1) = (r.rand(), r.rand(), r.rand());
                ctx.drand.sdrand(a1, b2, c);
            }
            "-restart" => {
                i += 1;
                let name = string_arg(ctx, argv, i)?;
                input = read4d::<T>(ctx, name)?;
            }
            "-save" => {
                i += 1;
                let name = string_arg(ctx, argv, i)?;
                save(ctx, &input, name)?;
            }
            "-thr" | "-uthr" => {
                i += 1;
                let val = string_arg(ctx, argv, i)?;
                if !is_number(val) {
                    ctx.eput(&format!("Error: {op} requires a number\n"));
                    return Err(Stop::Exit(1));
                }
                let t = ctx.conv::<T>(cnum::atof(val), op);
                threshold_at(ctx, &mut input, (t, t), op == "-uthr", &[], op);
            }
            "-range" => set_display_range = true,
            "-thrp" | "-thrP" | "-uthrp" | "-uthrP" => {
                i += 1;
                let val = string_arg(ctx, argv, i)?;
                if !is_number(val) {
                    ctx.eput(&format!("Error: {op} requires a number\n"));
                    return Err(Stop::Exit(1));
                }
                let positive = op.ends_with('P');
                let mask = if positive {
                    // mask.binarise(): [smallestPositiveT, numeric_limits<T>::max()], inclusive
                    let mut m = input.clone();
                    m.binarise(T::SMALLEST_POSITIVE, T::MAXV, true, false);
                    Some(m)
                } else {
                    None
                };
                // robustmin() + p * (robustmax() - robustmin()): three evaluations, each with
                // its warning
                let (lo, fix_lo, causes) = robust(ctx, &input, mask.as_ref())?;
                let (hi, fix_hi, _) = robust(ctx, &input, mask.as_ref())?;
                let _ = robust(ctx, &input, mask.as_ref())?;
                let limit = robust_limit(ctx, lo.0, hi.1, val, op);
                let fixed = robust_limit(ctx, fix_lo.0, fix_hi.1, val, op);
                let upper = op.starts_with("-u");
                threshold_at(ctx, &mut input, (limit, fixed), upper, &causes, op);
            }
            "-sqrt" => {
                let n = looped_len(&input);
                for k in 0..n {
                    let v = input.data[k];
                    input.data[k] = if v > T::from_int(0) {
                        sqrt_t(ctx, v)
                    } else {
                        T::from_int(0)
                    };
                }
            }
            "-pow" => {
                let s = number_arg(ctx, argv, i + 1)?;
                let p = cnum::atof(s);
                // correctly rounded by default, glibc's bits with `reproduce` (GLIBC-07; its
                // NaN as glibc makes it in both modes)
                let n = looped_len(&input);
                let reproduce = ctx.reproduce();
                let mut moved = 0usize;
                for k in 0..n {
                    let x = input.data[k].to_f64();
                    let (g, cr) = (glibc_double::pow(x, p), math::pow(x, p));
                    let cr = if g.is_nan() && cr.is_nan() { g } else { cr };
                    if g.to_bits() != cr.to_bits() && stored::<T>(g) != stored::<T>(cr) {
                        moved += 1;
                    }
                    let r = if reproduce { g } else { cr };
                    input.data[k] = ctx.conv::<T>(r, "-pow");
                }
                last_bit_notice(ctx, "-pow", moved);
                i += 1;
            }
            "-invbin" => {
                let n = looped_len(&input);
                for k in 0..n {
                    let nz = T::nonzero(input.data[k]);
                    input.data[k] = T::from_int(i32::from(!nz));
                }
            }
            "-sqr" => input.square(),
            "-recip" => {
                let n = looped_len(&input);
                let one = T::from_int(1);
                for k in 0..n {
                    let v = input.data[k];
                    if T::nonzero(v) {
                        input.data[k] = T::div(one, v).unwrap_or(v);
                    }
                }
            }
            "-exp" | "-log" | "-cos" | "-sin" | "-tan" | "-asin" | "-acos" | "-atan" => {
                // Correctly rounded by default (larmorx_core::math, CORE-MATH's); glibc 2.35's
                // functions are not (GLIBC-07). Its bits with `reproduce` where permissive code
                // has them (exp, log: Arm's Optimized Routines, the code glibc 2.35 ships),
                // named where they change a voxel; sin, cos, tan, asin, acos and atan (IBM's
                // code in glibc, LGPL) are correctly rounded in both modes.
                let n = looped_len(&input);
                let reproduce = ctx.reproduce();
                let mut moved = 0usize;
                for k in 0..n {
                    let v = input.data[k];
                    let positive = v > T::from_int(0);
                    if op == "-log" && !positive {
                        continue;
                    }
                    let x = v.to_f64();
                    let (cr, glibc) = match op {
                        "-exp" => (math::exp(x), Some(glibc_double::exp(x))),
                        "-log" => (math::log(x), Some(glibc_double::log(x))),
                        "-cos" => (math::cos(x), None),
                        "-sin" => (math::sin(x), None),
                        "-tan" => (math::tan(x), None),
                        "-asin" => (math::asin(x), None),
                        "-acos" => (math::acos(x), None),
                        _ => (math::atan2(x, 1.0), None),
                    };
                    // a NaN argument passed on quieted; a NaN made from a number is SSE's default
                    let nan = |r: f64| {
                        if x.is_nan() {
                            larmorx_nc_core::sse::quiet_f64(x)
                        } else if r.is_nan() {
                            // glibc's asin and acos give a positive NaN outside [-1, 1]; the
                            // others' NaN (sin, cos, tan of an infinity) is SSE's default
                            if matches!(op, "-asin" | "-acos") {
                                f64::NAN
                            } else {
                                larmorx_nc_core::sse::DEFAULT_NAN_F64
                            }
                        } else {
                            r
                        }
                    };
                    let r = match glibc {
                        Some(g) if reproduce => g,
                        _ => cr,
                    };
                    if let Some(g) = glibc
                        && g.to_bits() != cr.to_bits()
                        && stored::<T>(nan(g)) != stored::<T>(nan(cr))
                    {
                        moved += 1;
                    }
                    input.data[k] = ctx.conv::<T>(nan(r), op);
                }
                last_bit_notice(ctx, op, moved);
            }
            "-abs" => {
                for k in 0..input.data.len() {
                    let v = input.data[k];
                    input.data[k] = if T::IS_FLOAT {
                        volops::abs_value(v)
                    } else {
                        ctx.conv::<T>(v.to_f64().abs(), "-abs")
                    };
                }
            }
            "-bin" => bin_above(ctx, &mut input, T::from_int(0), "-bin"),
            "-binv" => {
                bin_above(ctx, &mut input, T::from_int(0), "-binv");
                let _ = input.op_scalar(Op::Mul, T::from_int(-1));
                let _ = input.op_scalar(Op::Add, T::from_int(1));
                // (T) 0.5: to cope with a bit of rounding error
                bin_above(ctx, &mut input, T::from_f64(0.5), "-binv");
            }
            "-index" => {
                let n = looped_len(&input);
                let mut index: i32 = 0;
                for k in 0..n {
                    if input.data[k] > T::from_int(0) {
                        input.data[k] = T::from_int(index);
                        index = index.wrapping_add(1);
                    }
                }
            }
            "-grid" => {
                i += 1;
                let gv = cnum::atof(number_arg(ctx, argv, i)?);
                i += 1;
                let spacing = cnum::atoi(number_arg(ctx, argv, i)?);
                if spacing == 0 {
                    return Err(trap(ctx, "-grid with a spacing of 0"));
                }
                let value = ctx.conv::<T>(gv, "-grid");
                let (nx, ny, nz) = (input.xsize(), input.ysize(), input.zsize());
                let nt = input.tsize();
                let s = i64::from(spacing);
                let mut k = 0usize;
                for _t in 0..nt {
                    for z in 0..nz {
                        for y in 0..ny {
                            for x in 0..nx {
                                if x % s == 0 || y % s == 0 || z % s == 0 {
                                    input.data[k] = value;
                                }
                                k += 1;
                            }
                        }
                    }
                }
            }
            "-nanm" | "-nan" => {
                let n = looped_len(&input);
                for k in 0..n {
                    let finite = input.data[k].to_f64().is_finite();
                    if op == "-nanm" {
                        input.data[k] = T::from_int(i32::from(!finite));
                    } else if !finite {
                        input.data[k] = T::from_int(0);
                    }
                }
            }
            "-rand" | "-randn" => {
                let n = looped_len(&input);
                let mut printed = String::new();
                for k in 0..n {
                    let rnd = ctx.drand.drand();
                    let add = if op == "-rand" {
                        ctx.conv::<T>(rnd - 1.0, op)
                    } else {
                        let (z, msg) = cprob::ndtri(rnd - 1.0);
                        if let Some(m) = msg {
                            printed.push_str(&m);
                        }
                        ctx.conv::<T>(z, op)
                    };
                    input.data[k] = T::add(input.data[k], add);
                }
                if !printed.is_empty() {
                    ctx.put(&printed);
                }
            }
            _ => {
                ctx.put(&format!(
                    "\n Error in command line: unknown option \"{op}\"\n\n"
                ));
                ctx.put(usage::FSLMATHS);
                return Ok(1);
            }
        }
        i += 1;
    }
    // check_for_output_name(i, argc-1)
    if i + 1 > argc {
        ctx.eput("Error: no output filename specified!\n");
        return Err(Stop::Exit(1));
    }
    // a float result rounded before an integer output type (DT_UNSIGNED_CHAR, SIGNED_SHORT,
    // SIGNED_INT: the datatypes below DT_FLOAT); FSL-157: int8 and uint16 (-odt input), whose
    // closest templates are char and int, are truncated by FSL, rounded by default
    let above = matches!(output_dt, dt::INT8 | dt::UINT16);
    if T::IS_FLOAT && output_dt < dt::FLOAT {
        round_values(ctx, &mut input);
    } else if T::IS_FLOAT && above {
        // the values whose rounding (half away from zero, as round_values) is not their
        // truncation
        let n = looped_len(&input);
        let moved = (0..n)
            .filter(|&k| {
                let x = input.data[k].to_f64();
                let r = if x > 0.0 { x + 0.5 } else { x - 0.5 };
                x.is_finite() && r.trunc() != x.trunc()
            })
            .count();
        if moved > 0 {
            ctx.notice(
                &defects::FSL_157,
                format!(
                    "-odt input: {moved} values rounded to the integer output type, not truncated"
                ),
            );
        }
        if !ctx.reproduce() {
            round_values(ctx, &mut input);
        }
    }
    if modified {
        input.display_max = 0.0;
        input.display_min = 0.0;
    }
    if set_display_range {
        let e = Extremes::of(&input);
        let (mut mx, mut mn) = e.fsl;
        if e.nan_first() {
            ctx.notice(&defects::FSL_153, "-range: the first voxel is NaN");
            if !ctx.reproduce() {
                (mx, mn) = e.fixed;
            }
        }
        input.display_max = mx.to_f32();
        input.display_min = mn.to_f32();
    }
    let out_name = argv[argc - 1].clone();
    save_dtype(ctx, &input, &out_name, output_dt)?;
    Ok(0)
}

/// FSL-161: a kernel without voxels (or of an unknown type), with which FSL filters anyway.
/// Corrected (rule B): refused.
fn no_voxels(ctx: &mut Ctx, what: &str, known: bool) -> Res<()> {
    let why = if known {
        "the kernel has no voxels"
    } else {
        "an unknown kernel type"
    };
    ctx.notice(&defects::FSL_161, format!("{what}: {why}"));
    if ctx.reproduce() {
        return Ok(());
    }
    ctx.eput(&format!(
        "larmorx-nc: error: fslmaths: {what}: {why} ({}; LARMORX_UPSTREAM_DEFECTS=reproduce \
         filters with it as FSL does)\n",
        defects::FSL_161.id
    ));
    Err(Stop::Exit(1))
}

/// `generic_convolve(input, kernel, separable, renormalise)` (`-s`, `-fmean`, `-fmeanu`), with
/// FSL-160's correction: a zero-width Gaussian (its centre NaN) filters as the unit impulse.
fn convolve<T: Arith>(
    ctx: &mut Ctx,
    input: &mut Volume<T>,
    kernel: &Kern,
    zero_width: bool,
    separable: bool,
    renormalise: bool,
    op: &str,
) -> Res<()> {
    let impulse;
    let mut k = kernel;
    if zero_width {
        ctx.notice(
            &defects::FSL_160,
            format!("{op}: a Gaussian kernel of zero width"),
        );
        if !ctx.reproduce() {
            let mut c = kernel.clone();
            for v in &mut c.data {
                if v.is_nan() {
                    *v = 1.0;
                }
            }
            impulse = c;
            k = &impulse;
        }
    }
    let mut msgs = Vec::new();
    let r = filters::generic_convolve(input, k, separable, renormalise, &mut msgs, &mut |x| {
        ctx.conv::<T>(x, op)
    });
    for m in msgs {
        ctx.eput(&format!("{m}\n"));
    }
    match r {
        Ok(v) => {
            *input = v;
            Ok(())
        }
        Err(ConvStop::Trap) => Err(trap(ctx, op)),
        Err(ConvStop::Exception(e)) => Err(Stop::Exception(e)),
    }
}

/// `-tensor_decomp`: each voxel's tensor (volumes 0 to 5: xx, xy, xz, yy, yz, zz, as `float`)
/// decomposed by Armadillo's `eig_sym` (LAPACK `dsyevd`); where the largest eigenvalue is
/// positive, the eigenvalues L1 >= L2 >= L3, FA, MD, the mode MO and the eigenvectors V1, V2, V3
/// (in `float`, FSL's expressions), else 0 (FSL-166: a tensor holding a non-finite value, where
/// FSL stops, is 0 too). The ten images are written at once as
/// `<output>_L1` ... `<output>_V3` (`float`, with their display ranges); the pipeline goes on with
/// the FA image (3D).
fn tensor_decomp<T: Arith>(ctx: &mut Ctx, input: &mut Volume<T>, out: &str) -> Res<()> {
    let (nx, ny, nz) = (input.xsize(), input.ysize(), input.zsize());
    let nvox = input.nvoxels().max(0) as usize;
    let new3 = |t: i64| {
        let mut v = Volume::<f32>::new(nx, ny, nz, t);
        v.copy_properties(input);
        v
    };
    let (mut l1, mut l2, mut l3) = (new3(1), new3(1), new3(1));
    let (mut fa, mut md, mut mo) = (new3(1), new3(1), new3(1));
    let (mut v1, mut v2, mut v3) = (new3(3), new3(3), new3(3));
    let mut warnings = Vec::new();
    for z in 0..nz {
        for y in 0..ny {
            for x in 0..nx {
                let i = input.idx(x, y, z);
                let c = |t: usize| f64::from(input.data[t * nvox + i].to_f32());
                let mut m = Mat::zeros(3, 3);
                let entries = [
                    (0, 0, 0),
                    (1, 0, 1),
                    (1, 1, 3),
                    (2, 0, 2),
                    (2, 1, 4),
                    (2, 2, 5),
                ];
                for (r, col, t) in entries {
                    m.set(r, col, c(t));
                    m.set(col, r, c(t));
                }
                let Some((vals, vecs)) = larmorx_nc_core::arma::eig_sym(&m, &mut warnings) else {
                    // the decomposition failed (a non-finite value): D is empty and FSL reads its
                    // third element (Armadillo's bounds check). FSL-166: the voxel left at 0.
                    ctx.notice(
                        &defects::FSL_166,
                        format!("-tensor_decomp: voxel ({x}, {y}, {z}) holds a non-finite value"),
                    );
                    if ctx.reproduce() {
                        return Err(Stop::Exception(FslError {
                            printed: Vec::new(),
                            type_name: "std::out_of_range",
                            what: "Mat::operator(): index out of bounds".into(),
                        }));
                    }
                    input.data[i] = T::from_int(0);
                    continue;
                };
                if vals[2] > 0.0 {
                    let (a, b, cc) = (vals[2] as f32, vals[1] as f32, vals[0] as f32);
                    l1.data[i] = a;
                    l2.data[i] = b;
                    l3.data[i] = cc;
                    let lmean = (a + b + cc) / 3.0;
                    let lnum = 3.0
                        * ((a - lmean) * (a - lmean)
                            + (b - lmean) * (b - lmean)
                            + (cc - lmean) * (cc - lmean));
                    let lden = 2.0 * (a * a + b * b + cc * cc);
                    fa.data[i] = (lnum / lden).sqrt();
                    md.data[i] = lmean;
                    let (e1, e2, e3) = (a - lmean, b - lmean, cc - lmean);
                    let n = (e1 + e2 - 2.0 * e3) * (2.0 * e1 - e2 - e3) * (e1 - 2.0 * e2 + e3);
                    let mut d = e1 * e1 + e2 * e2 + e3 * e3 - e1 * e2 - e2 * e3 - e1 * e3;
                    d = (if 0.0 > d { 0.0 } else { d }).sqrt();
                    d = 2.0 * d * d * d;
                    let q = if d != 0.0 { f64::from(n / d) } else { 0.0 };
                    let q = if q > -1.0 { q } else { -1.0 };
                    mo.data[i] = (if q < 1.0 { q } else { 1.0 }) as f32;
                    for k in 0..3 {
                        v1.data[k * nvox + i] = vecs.at(k, 2) as f32;
                        v2.data[k * nvox + i] = vecs.at(k, 1) as f32;
                        v3.data[k * nvox + i] = vecs.at(k, 0) as f32;
                    }
                }
                input.data[i] = ctx.conv::<T>(f64::from(fa.data[i]), "-tensor_decomp");
            }
        }
    }
    // deletevolume(5) .. deletevolume(1): the FA image alone
    input.data.truncate(nvox);
    input.dims[3] = 1;
    let range = |v: &mut Volume<f32>, mx: f32, mn: f32| {
        v.display_max = mx;
        v.display_min = mn;
    };
    for (vol, suffix) in [(&mut l1, "_L1"), (&mut l2, "_L2"), (&mut l3, "_L3")] {
        let (mx, mn) = vol.extrema();
        range(vol, mx, mn);
        save(ctx, vol, &format!("{out}{suffix}"))?;
    }
    range(&mut fa, 1.0, 0.0);
    save(ctx, &fa, &format!("{out}_FA"))?;
    let (mx, mn) = md.extrema();
    range(&mut md, mx, mn);
    save(ctx, &md, &format!("{out}_MD"))?;
    range(&mut mo, 1.0, -1.0);
    save(ctx, &mo, &format!("{out}_MO"))?;
    for (vol, suffix) in [(&mut v1, "_V1"), (&mut v2, "_V2"), (&mut v3, "_V3")] {
        range(vol, 1.0, -1.0);
        save(ctx, vol, &format!("{out}{suffix}"))?;
    }
    Ok(())
}

/// `subsample_by_2(input, centred)` with extraslice extrapolation; FSL-163: a 4D image (FSL's
/// exception wherever the smoothing reads outside the volume, else its first volume only) is
/// subsampled volume by volume.
fn subsample<T: Arith>(ctx: &mut Ctx, input: &mut Volume<T>, centred: bool, op: &str) -> Res<()> {
    let dims = [input.xsize(), input.ysize(), input.zsize()];
    let nt = input.tsize().max(0) as usize;
    // FSL-75: subsample_by_2 fills its new volume with the background value first (`halfvol =
    // refvol.backgroundval()`), whose 2-voxel border buffer is too small for some sizes (a size
    // of 3 along x or y): FSL writes past it (heap corruption: it may abort, as glibc's `free`
    // does) or, at 1 x 1 x 1, reads an empty buffer (a crash). Every voxel is then overwritten,
    // so the value is never used: the result is the same in both modes.
    let [x, y, z] = dims;
    match input.background_buffer() {
        volume::BorderBuffer::Exact => {}
        volume::BorderBuffer::Empty => {
            let what = format!(
                "{op}: the background value of a {x} x {y} x {z} image reads the first value of \
                 an empty buffer (FSL-75)"
            );
            if ctx.reproduce() {
                return Err(ctx.refuse(&defects::FSL_75, &what));
            }
            ctx.notice(&defects::FSL_75, format!("{what}; the value is never used"));
        }
        volume::BorderBuffer::Overflow { numbins, written } => ctx.notice(
            &defects::FSL_75,
            format!(
                "{op}: the background value of a {x} x {y} x {z} image: fslmaths writes \
                 {written} border values into a buffer of {numbins} (heap corruption; fslmaths \
                 may abort or crash); the value is never used (every voxel is overwritten)"
            ),
        ),
    }
    if input.dimensionality() > 3 {
        ctx.notice(&defects::FSL_163, format!("{op} of a 4D image"));
        if ctx.reproduce() {
            if filters::subsample_reads_outside(dims, centred) {
                return Err(Stop::Exception(FslError::imthrow(
                    "3D only method called by higher-dimensional volume.",
                    75,
                )));
            }
        } else {
            let halves: Vec<Volume<T>> = (0..nt)
                .map(|t| filters::subsample_by_2(input, t, centred, &mut |x| ctx.conv::<T>(x, op)))
                .collect();
            let h = &halves[0];
            let mut out = Volume::<T>::new(h.xsize(), h.ysize(), h.zsize(), nt as i64);
            out.copy_properties(h);
            out.data = halves.iter().flat_map(|v| v.data.iter().copied()).collect();
            *input = out;
            return Ok(());
        }
    }
    *input = filters::subsample_by_2(input, 0, centred, &mut |x| ctx.conv::<T>(x, op));
    Ok(())
}

/// `morphfilter(input[t], kernel, type)` for each volume, with FSL's messages before each, and
/// FSL-162's correction (a NaN in a sorted vector left out instead of Armadillo's exception).
fn morph<T: Arith>(
    ctx: &mut Ctx,
    input: &mut Volume<T>,
    kernel: &Kern,
    kind: Morph,
    op: &str,
) -> Res<()> {
    let nvox = input.nvoxels().max(0) as usize;
    let dims = [input.xsize(), input.ysize(), input.zsize()];
    let msgs = filters::morph_messages(kernel);
    for t in 0..input.tsize().max(0) as usize {
        for m in &msgs {
            ctx.eput(&format!("{m}\n"));
        }
        let mut issues = filters::MorphIssues::default();
        let src = input.data[t * nvox..(t + 1) * nvox].to_vec();
        let out = filters::morphfilter(
            &src,
            dims,
            kernel,
            kind,
            &mut |x| ctx.conv::<T>(x, op),
            &mut issues,
        );
        if issues.overflow {
            // more values under the kernel than its values in [0.5, FLT_MAX] (a +inf kernel
            // value): Armadillo's bounds check (`std::out_of_range`)
            return Err(Stop::Exception(FslError {
                printed: Vec::new(),
                type_name: "std::out_of_range",
                what: "Mat::operator(): index out of bounds".into(),
            }));
        }
        if issues.nan_sorted {
            ctx.notice(&defects::FSL_162, format!("{op}: a NaN under the kernel"));
            if ctx.reproduce() {
                return Err(Stop::Exception(filters::sort_nan_error()));
            }
        }
        input.data[t * nvox..(t + 1) * nvox].copy_from_slice(&out);
    }
    Ok(())
}

/// `sqrt` of a positive value in the type's own function (`sqrtf` for `float`, `sqrt` of the
/// `double` value for the others).
fn sqrt_t<T: Arith>(ctx: &mut Ctx, v: T) -> T {
    match T::DTYPE {
        dt::FLOAT => T::from_f32(v.to_f32().sqrt()),
        _ => ctx.conv::<T>(v.to_f64().sqrt(), "-sqrt"),
    }
}

/// `-rem <number|image>`: `(int) value % (int) divisor`, converted back.
fn rem<T: Arith>(ctx: &mut Ctx, input: &mut Volume<T>, s: &str) -> Res<()> {
    let n = looped_len(input);
    if is_number(s) {
        let d = cnum::atof(s);
        let denom = ctx.int_of::<f64>(d, "-rem's divisor");
        for k in 0..n {
            let v = ctx.int_of(input.data[k], "-rem");
            match v.checked_rem(denom) {
                Some(r) => input.data[k] = T::from_int(r),
                None => return Err(trap(ctx, "-rem")),
            }
        }
        return Ok(());
    }
    let img = load_new_image(ctx, input, s)?;
    other_size(ctx, input, &img, "divide", "-rem")?;
    let nvox = input.nvoxels() as usize;
    let (nx, ny, nz) = (input.xsize(), input.ysize(), input.zsize());
    for t in 0..input.tsize() {
        let t2 = t % img.tsize();
        for z in 0..nz {
            for y in 0..ny {
                for x in 0..nx {
                    let d = get4(&img, x, y, z, t2);
                    if T::nonzero(d) {
                        let k = t as usize * nvox + input.idx(x, y, z);
                        let v = ctx.int_of(input.data[k], "-rem");
                        let dd = ctx.int_of(d, "-rem's divisor");
                        match v.checked_rem(dd) {
                            Some(r) => input.data[k] = T::from_int(r),
                            None => return Err(trap(ctx, "-rem")),
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

/// `vol(x, y, z, t)` (bounds-checked: zero padding outside, as newimage's default
/// extrapolation gives).
fn get4<T: Arith>(v: &Volume<T>, x: i64, y: i64, z: i64, t: i64) -> T {
    if v.in_bounds(x, y, z) {
        v.data[t as usize * v.nvoxels() as usize + v.idx(x, y, z)]
    } else {
        v.extrapolate(x, y, z)
    }
}

/// `-div <number|image>`.
fn div<T: Arith>(ctx: &mut Ctx, input: &mut Volume<T>, s: &str) -> Res<()> {
    if is_number(s) {
        let d = cnum::atof(s);
        if d != 0.0 {
            let val = ctx.conv::<T>(d, "-div");
            if input.op_scalar(Op::Div, val).is_err() {
                return Err(trap(ctx, "-div"));
            }
        }
        return Ok(());
    }
    let img = load_new_image(ctx, input, s)?;
    other_size(ctx, input, &img, "divide", "-div")?;
    let nvox = input.nvoxels() as usize;
    let (nx, ny, nz) = (input.xsize(), input.ysize(), input.zsize());
    let ivox = img.nvoxels() as usize;
    for t in 0..input.tsize() {
        let t2 = t % img.tsize();
        for z in 0..nz {
            for y in 0..ny {
                for x in 0..nx {
                    let k = t as usize * nvox + input.idx(x, y, z);
                    if T::nonzero(get4(&img, x, y, z, t2)) {
                        // value(x, y, z, t2): in bounds here
                        let d = img.data[t2 as usize * ivox + img.idx(x, y, z)];
                        match T::div(input.data[k], d) {
                            Some(q) => input.data[k] = q,
                            None => return Err(trap(ctx, "-div")),
                        }
                    } else {
                        input.data[k] = T::from_int(0);
                    }
                }
            }
        }
    }
    Ok(())
}

/// `-max`/`-min <number|image>`: `MAX(a, b)` is `a > b ? a : b`, `MIN(a, b)` `a < b ? a : b`.
fn max_min<T: Arith>(ctx: &mut Ctx, input: &mut Volume<T>, s: &str, is_max: bool) -> Res<()> {
    let pick = |a: T, b: T| {
        if is_max {
            if a > b { a } else { b }
        } else if a < b {
            a
        } else {
            b
        }
    };
    let n = looped_len(input);
    if is_number(s) {
        let p = ctx.conv::<T>(cnum::atof(s), if is_max { "-max" } else { "-min" });
        for k in 0..n {
            input.data[k] = pick(input.data[k], p);
        }
        return Ok(());
    }
    let opname = if is_max { "-max" } else { "-min" };
    let img = load_new_image(ctx, input, s)?;
    other_size(ctx, input, &img, "compare", opname)?;
    // temp_volume.value(x, y, z, t % tsize): the image's own index, never checked
    let (tx, ty, tz) = (img.xsize(), img.ysize(), img.zsize());
    let (nx, ny, nz) = (input.xsize(), input.ysize(), input.zsize());
    let mut k = 0usize;
    for t in 0..input.tsize() {
        let t2 = t % img.tsize();
        for z in 0..nz {
            for y in 0..ny {
                for x in 0..nx {
                    let j = ((tz * t2 + z) * ty + y) * tx + x;
                    if j < 0 || j as usize >= img.data.len() {
                        return Err(ctx.refuse(
                            &defects::FSL_159,
                            &format!("{opname} reads past the end of a smaller image"),
                        ));
                    }
                    input.data[k] = pick(input.data[k], img.data[j as usize]);
                    k += 1;
                }
            }
        }
    }
    Ok(())
}

/// `(T) MISCMATHS::round(value)` before an integer output (`(int)(v + 0.5)` above 0,
/// `(int)(v - 0.5)` otherwise; FSL-151 out of `int`'s range or NaN).
fn round_values<T: Arith>(ctx: &mut Ctx, v: &mut Volume<T>) {
    let n = looped_len(v);
    let mut undefined = 0usize;
    for k in 0..n {
        let x = v.data[k].to_f64();
        let shifted = if x > 0.0 { x + 0.5 } else { x - 0.5 };
        let defined = !shifted.is_nan() && shifted > -2_147_483_649.0 && shifted < 2_147_483_648.0;
        if defined {
            v.data[k] = T::from_int(cnum::f64_to_i32(shifted));
        } else {
            undefined += 1;
            v.data[k] = if ctx.reproduce() {
                T::from_int(i32::MIN)
            } else if x.is_nan() {
                T::from_int(0)
            } else {
                // rounded as FSL rounds, without the conversion to int (the output clamps it)
                T::from_f64(shifted.trunc())
            };
        }
    }
    if undefined > 0 {
        ctx.notice(
            &defects::FSL_151,
            format!("{undefined} values outside int's range or NaN rounded for the output"),
        );
    }
}

/// `save_volume(vol, name)` in the volume's own type: `FSLOUTPUTTYPE`'s failure prints its
/// lines and exits with status 1 (`FslGetEnvOutputType`), any other is an exception.
fn save<T: Arith>(ctx: &mut Ctx, v: &Volume<T>, name: &str) -> Res<()> {
    let opts = SaveOptions::new(volume::BUILDSTRING.as_bytes()).reproducing_offset(ctx.reproduce());
    match v.save_volume(name, ctx.run.fsl, &opts) {
        Ok(_) => Ok(()),
        Err(SaveError::OutputType(o)) => {
            for l in &o.printed {
                ctx.eput(&format!("{l}\n"));
            }
            Err(Stop::Exit(1))
        }
        Err(SaveError::Exception(e)) => Err(Stop::Exception(e)),
    }
}

/// `save_volume_dtype(vol, name, datatype)`: converted to the closest templated type of
/// `datatype` (FSL-151: integer types clamped to the values their files hold, NaN as 0).
fn save_dtype<T: Arith>(ctx: &mut Ctx, v: &Volume<T>, name: &str, datatype: i16) -> Res<()> {
    if datatype == dt::INT8 {
        return save_int8(ctx, v, name);
    }
    match volume::closest_templated_type(datatype) {
        Ok(dt::UNSIGNED_CHAR) => save_as::<T, i8>(ctx, v, name),
        Ok(dt::SIGNED_SHORT) => save_as::<T, i16>(ctx, v, name),
        Ok(dt::SIGNED_INT) => save_as::<T, i32>(ctx, v, name),
        Ok(dt::DOUBLE) => save_as::<T, f64>(ctx, v, name),
        Ok(_) => save_as::<T, f32>(ctx, v, name),
        Err(msg) => {
            ctx.eput(&format!(
                "{msg}\nNEWIMAGE::save_volume_dtype: DT {datatype} not supported: Success\n"
            ));
            Ok(())
        }
    }
}

/// An int8 output (`-odt input` of an int8 image): FSL writes it in its closest template, `char`,
/// whose files are unsigned bytes, so a negative value becomes another (FSL-157, with FSL-151's
/// clamp to 0 by default); corrected, written as int8 with the values clamped to -128..127.
fn save_int8<T: Arith>(ctx: &mut Ctx, v: &Volume<T>, name: &str) -> Res<()> {
    if ctx.reproduce() {
        let n = v.data.iter().filter(|x| x.to_f64() < 0.0).count();
        if n > 0 {
            ctx.notice(
                &defects::FSL_157,
                format!("-odt input: {n} negative values of an int8 output written as uint8"),
            );
        }
        return save_as::<T, i8>(ctx, v, name);
    }
    let mut out = Volume::<i8>::copyconvert(v);
    let mut changed = 0usize;
    let mut negative = 0usize;
    for (o, &x) in out.data.iter_mut().zip(&v.data) {
        let f = x.to_f64();
        let c = if f.is_nan() {
            0.0
        } else {
            f.trunc().clamp(-128.0, 127.0)
        };
        if c != f.trunc() || f.is_nan() {
            changed += 1;
        }
        if c < 0.0 {
            negative += 1;
        }
        *o = c as i8;
    }
    if changed > 0 {
        ctx.notice(
            &defects::FSL_151,
            format!("{changed} values outside the output type's range or NaN"),
        );
    }
    ctx.notice(
        &defects::FSL_157,
        format!(
            "-odt input: an int8 output written as int8 ({negative} negative values), not uint8"
        ),
    );
    let opts = SaveOptions::new(volume::BUILDSTRING.as_bytes()).reproducing_offset(ctx.reproduce());
    match out.save_volume_coded(name, ctx.run.fsl, &opts, dt::INT8) {
        Ok(_) => Ok(()),
        Err(SaveError::OutputType(o)) => {
            for l in &o.printed {
                ctx.eput(&format!("{l}\n"));
            }
            Err(Stop::Exit(1))
        }
        Err(SaveError::Exception(e)) => Err(Stop::Exception(e)),
    }
}

fn save_as<T: Arith, D: Arith>(ctx: &mut Ctx, v: &Volume<T>, name: &str) -> Res<()> {
    if T::DTYPE == D::DTYPE {
        return save(ctx, v, name);
    }
    let mut out = Volume::<D>::copyconvert(v);
    if !D::IS_FLOAT {
        // the values the integer file holds: char is written as unsigned bytes
        let (lo, hi) = match D::DTYPE {
            dt::UNSIGNED_CHAR => (0.0, 255.0),
            _ => int_range::<D>(),
        };
        let mut changed = 0usize;
        for (o, &x) in out.data.iter_mut().zip(&v.data) {
            let f = x.to_f64();
            let fixed = if f.is_nan() {
                0.0
            } else {
                f.trunc().clamp(lo, hi)
            };
            let fixed: D = if D::DTYPE == dt::UNSIGNED_CHAR {
                D::from_int(fixed as i32 as u8 as i8 as i32)
            } else {
                D::from_f64(fixed)
            };
            if fixed.to_f64().to_bits() != o.to_f64().to_bits() {
                changed += 1;
                if !ctx.reproduce() {
                    *o = fixed;
                }
            }
        }
        if changed > 0 {
            ctx.notice(
                &defects::FSL_151,
                format!("{changed} values outside the output type's range or NaN"),
            );
        }
    }
    save(ctx, &out, name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings() -> Settings {
        Settings::new().with("FSLOUTPUTTYPE", "NIFTI_GZ")
    }

    fn run(args: &[&str], mode: UpstreamDefects) -> (Res<u8>, String, String) {
        let settings = settings();
        let run = Run {
            fsl: &settings,
            mode,
        };
        let argv: Vec<String> = std::iter::once("fslmaths")
            .chain(args.iter().copied())
            .map(str::to_owned)
            .collect();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let r = main(&argv, &run, &mut out, &mut err);
        (
            r,
            String::from_utf8(out).unwrap(),
            String::from_utf8(err).unwrap(),
        )
    }

    #[test]
    fn numbers_as_miscmaths_reads_them() {
        assert!(is_number("2.5"));
        assert!(is_number("-1e3"));
        assert!(is_number("nan"));
        assert!(is_number(" 7"));
        assert!(!is_number(""));
        assert!(!is_number("5mm"));
        assert!(!is_number("image.nii.gz"));
    }

    #[test]
    fn usage_and_help() {
        let (r, out, _) = run(&[], UpstreamDefects::Fix);
        assert_eq!(r.unwrap(), 1);
        assert!(out.starts_with("\nUsage: fslmaths [-dt <datatype>]"));
        let (r, out, _) = run(&["--help"], UpstreamDefects::Fix);
        assert_eq!(r.unwrap(), 0);
        assert!(out.contains(" -kernel 3D : 3x3x3 box"));
    }

    #[test]
    fn a_chain_on_a_written_image() {
        let d = tempfile::tempdir().unwrap();
        let p = |n: &str| d.path().join(n).to_string_lossy().into_owned();
        let mut v = Volume::<f32>::new(4, 3, 2, 1);
        for (k, x) in v.data.iter_mut().enumerate() {
            *x = k as f32 - 5.0;
        }
        let opts = SaveOptions::new(volume::BUILDSTRING.as_bytes());
        v.save_volume(&p("in"), &settings(), &opts).unwrap();
        let (r, _, err) = run(
            &[&p("in"), "-thr", "0", "-mul", "2", "-bin", &p("out")],
            UpstreamDefects::Fix,
        );
        assert_eq!(r.unwrap(), 0, "{err}");
        let (o, _, _) = volume::read_volume4d::<f32>(&p("out"), &settings()).unwrap();
        let want: Vec<f32> = (0..24).map(|k| if k > 5 { 1.0 } else { 0.0 }).collect();
        assert_eq!(o.data, want);
        // an integer division by zero is refused (FSL-156)
        let (r, _, err) = run(&[&p("in"), "-rem", "0", &p("out2")], UpstreamDefects::Fix);
        assert!(matches!(r, Err(Stop::Exit(1))), "{err}");
        assert!(err.contains("FSL-156"), "{err}");
    }
}
