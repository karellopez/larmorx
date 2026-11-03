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
//! - **D13.** FSL-151 (conversions to integers), FSL-152 (the `max() + 1` bound), FSL-153 (a
//!   NaN first voxel), FSL-155 (`-dt input` of a type without its own), FSL-156 (integer
//!   division traps), FSL-158 (arguments read past the command line), FSL-159 (images of
//!   another size): corrected by default, reproduced with `LARMORX_UPSTREAM_DEFECTS=reproduce`
//!   (crashes are refused in both modes).

use std::io::Write;

use larmorx_core::defects::{self, Defect, Notices, UpstreamDefects};
use larmorx_nc_core::arma::Mat;
use larmorx_nc_core::cnum;
use larmorx_nc_core::cprob::{self, Drand};
use larmorx_nc_core::crand::Rand;
use larmorx_nc_core::glibc_double;
use larmorx_nc_core::miscmaths;
use larmorx_nc_core::nifti::{self, dt};
use larmorx_nc_core::settings::Settings;
use larmorx_nc_core::volops::{self, Arith, Op};
use larmorx_nc_core::volume::{self, Elem, FslError, SaveError, SaveOptions, Volume};

use crate::usage;

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
        let line = format!("larmorx-nc: error: fslmaths: {what} ({}: FSL crashes here)\n", d.id);
        self.eput(&line);
        Stop::Exit(1)
    }

    /// `(T) x` of a double: C's conversion, undefined for an integer type out of its range or
    /// for NaN (FSL-151: clamped, NaN as 0, by default).
    fn to_t<T: Arith>(&mut self, x: f64, what: &str) -> T {
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
            format!("{what}: {} outside the internal type's range", cnum::fmt_g(x, 6)),
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
    fn to_int<T: Arith>(&mut self, v: T, what: &str) -> i32 {
        if T::int_defined(v) {
            return T::to_int(v);
        }
        self.notice(&defects::FSL_151, format!("{what}: a value outside int's range or NaN"));
        if self.reproduce() {
            T::to_int(v)
        } else {
            T::to_int_clamped(v)
        }
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

/// The operations of later sub-tasks (refused until they are translated).
const NOT_YET: &[&str] = &[
    "-roc", "-kernel", "-s", "-subsamp2", "-subsamp2offc", "-pval", "-pval0", "-cpval", "-ztop",
    "-ptoz", "-rank", "-ranknorm", "-fillh", "-fillh26", "-dilall", "-dilM", "-dilD", "-dilF",
    "-ero", "-eroF", "-fmedian", "-fmean", "-fmeanu", "-edge", "-dog_edge", "-tfce_minT",
    "-tfce_delta", "-tfce", "-tfceS", "-roi", "-inm", "-ing", "-tensor_decomp", "-bptf",
];

fn read4d<T: Arith>(ctx: &Ctx, name: &str) -> Result<Volume<T>, FslError> {
    Ok(volume::read_volume4d::<T>(name, ctx.run.fsl)?.0)
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

/// `max()` and `min()` as fslmaths takes them (FSL-153: a NaN first voxel makes both NaN;
/// corrected, the NaN voxels are left out).
fn extremes<T: Arith>(ctx: &mut Ctx, v: &Volume<T>, what: &str) -> (T, T) {
    let fsl = v.extrema();
    if !T::IS_FLOAT {
        return fsl;
    }
    let fixed = v.extrema_without_nan().unwrap_or(fsl);
    let same = fsl.0.to_f64().to_bits() == fixed.0.to_f64().to_bits()
        && fsl.1.to_f64().to_bits() == fixed.1.to_f64().to_bits();
    if !same {
        ctx.notice(&defects::FSL_153, format!("{what}: the first voxel is NaN"));
        if !ctx.reproduce() {
            return fixed;
        }
    }
    fsl
}

/// The range test of `threshold`/`binarise` as FSL bounds it with `max() + 1` (or `min() - 1`)
/// and as intended (FSL-152: no bound beyond the values): `(lower, upper)` of FSL, and whether
/// the bound on each side is FSL's.
struct Bounds<T> {
    lower: T,
    upper: T,
    /// FSL's `min() - 1` lower bound (else the operation's own).
    lower_open: bool,
    /// FSL's `max() + 1` upper bound.
    upper_open: bool,
    inclusive: bool,
}

impl<T: Arith> Bounds<T> {
    fn fsl(&self, x: T) -> bool {
        if self.inclusive {
            x >= self.lower && x <= self.upper
        } else {
            x > self.lower && x < self.upper
        }
    }

    fn intended(&self, x: T) -> bool {
        let lo = if self.lower_open {
            !T::is_nan(x)
        } else if self.inclusive {
            x >= self.lower
        } else {
            x > self.lower
        };
        let hi = if self.upper_open {
            !T::is_nan(x)
        } else if self.inclusive {
            x <= self.upper
        } else {
            x < self.upper
        };
        lo && hi
    }
}

/// Which voxels are inside the range: FSL's test, or the intended one by default (FSL-152,
/// named where they differ).
fn inside<T: Arith>(ctx: &mut Ctx, v: &Volume<T>, b: &Bounds<T>, what: &str) -> Vec<bool> {
    let fsl: Vec<bool> = v.data.iter().map(|&x| b.fsl(x)).collect();
    if !(b.lower_open || b.upper_open) {
        return fsl;
    }
    let want: Vec<bool> = v.data.iter().map(|&x| b.intended(x)).collect();
    let n = fsl.iter().zip(&want).filter(|(a, b)| a != b).count();
    if n == 0 {
        return fsl;
    }
    ctx.notice(
        &defects::FSL_152,
        format!("{what}: {n} voxels on the far side of the image's maximum plus 1 or minimum minus 1"),
    );
    if ctx.reproduce() { fsl } else { want }
}

/// `binarise(lower, upper, tt)` with FSL-152's bounds.
fn binarise_b<T: Arith>(ctx: &mut Ctx, v: &mut Volume<T>, b: &Bounds<T>, what: &str) {
    let ins = inside(ctx, v, b, what);
    let (one, zero) = (T::from_int(1), T::from_int(0));
    for (x, i) in v.data.iter_mut().zip(ins) {
        *x = if i { one } else { zero };
    }
}

/// `threshold(lower, upper, tt)` with FSL-152's bounds.
fn threshold_b<T: Arith>(ctx: &mut Ctx, v: &mut Volume<T>, b: &Bounds<T>, what: &str) {
    let ins = inside(ctx, v, b, what);
    let zero = T::from_int(0);
    for (x, i) in v.data.iter_mut().zip(ins) {
        if !i {
            *x = zero;
        }
    }
}

/// `binarise(0, max() + 1, exclusive)` (`-bin`, `-mas`'s mask).
fn bin_positive<T: Arith>(ctx: &mut Ctx, v: &mut Volume<T>, what: &str) {
    let (mx, _) = extremes(ctx, v, what);
    let b = Bounds {
        lower: T::from_int(0),
        upper: T::plus_int(mx, 1),
        lower_open: false,
        upper_open: true,
        inclusive: false,
    };
    binarise_b(ctx, v, &b, what);
}

/// The robust range's limits (`robustmin()`/`robustmax()`, or with the mask of the positive
/// voxels): FSL-153 corrected leaves the NaN voxels out.
fn robust<T: Arith>(ctx: &mut Ctx, v: &Volume<T>, mask: Option<&Volume<T>>, what: &str) -> Res<(T, T)> {
    let ((lo, hi), warning) = v.robust_limits_masked(mask)?;
    if let Some(w) = warning {
        ctx.eput(&format!("{w}\n"));
    }
    if !T::IS_FLOAT || !v.data.iter().any(|&x| T::is_nan(x)) {
        return Ok((lo, hi));
    }
    // NaN voxels: counted in the histogram's lowest bin (and a NaN first voxel makes the
    // extremes NaN); corrected, they are left out of the mask.
    let mut keep = Volume::<T>::new7(v.dims);
    let one = T::from_int(1);
    let m: &[T] = mask.map_or(&[], |m| m.data.as_slice());
    for (k, (d, &x)) in keep.data.iter_mut().zip(&v.data).enumerate() {
        let in_m = m.is_empty() || m[k % m.len()].to_f64() > volops::MASK_THRESHOLD;
        if in_m && !T::is_nan(x) {
            *d = one;
        }
    }
    let fixed = if keep.no_mask_voxels() == 0 {
        (T::from_int(0), T::from_int(0))
    } else {
        volops::find_thresholds(v, Some(&keep))?
    };
    let same = fixed.0.to_f64().to_bits() == lo.to_f64().to_bits()
        && fixed.1.to_f64().to_bits() == hi.to_f64().to_bits();
    if same {
        return Ok((lo, hi));
    }
    ctx.notice(&defects::FSL_153, format!("{what}: NaN voxels in the robust range"));
    Ok(if ctx.reproduce() { (lo, hi) } else { fixed })
}

/// `robustmin + (p / 100) * (robustmax - robustmin)` converted to `T`.
fn robust_limit<T: Arith>(ctx: &mut Ctx, lo: T, hi: T, val: &str, what: &str) -> T {
    let p = cnum::atof(val) / 100.0;
    let x = lo.to_f64() + larmorx_nc_core::sse::mul_f64(p, T::diff(hi, lo));
    ctx.to_t::<T>(x, what)
}

/// An integer division by zero, as FSL crashes on it (FSL-156: refused).
fn trap(ctx: &mut Ctx, what: &str) -> Stop {
    ctx.refuse(&defects::FSL_156, &format!("{what}: an integer division by zero"))
}

/// FSL-159: an image of another size than the current one for an operation that indexes it
/// voxel by voxel. Corrected: refused as the other binary operations refuse it.
fn other_size<T: Arith>(ctx: &mut Ctx, cur: &Volume<T>, img: &Volume<T>, verb: &str, opname: &str) -> Res<()> {
    let same_xyz = cur.dims[..3] == img.dims[..3];
    let t_ok = img.tsize() == 1 || img.tsize() == cur.tsize();
    if same_xyz && t_ok {
        return Ok(());
    }
    ctx.notice(&defects::FSL_159, format!("{opname} with an image of another size"));
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
    let mut set_display_range = false;
    let mut modified = false;
    let mut i = 2usize;
    while i + 1 < argc {
        modified = true;
        let op = argv[i].as_str();
        let b = op.as_bytes();
        if b.len() >= 2 && b[1].is_ascii_uppercase() && b[0] == b'-' {
            return Err(not_yet(ctx, op));
        }
        match op {
            "-mas" => {
                i += 1;
                let name = string_arg(ctx, argv, i)?;
                let mut m = load_new_image(ctx, &mut input, name)?;
                bin_positive(ctx, &mut m, "-mas");
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
                    let val = ctx.to_t::<T>(cnum::atof(s), op);
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
                let t = ctx.to_t::<T>(cnum::atof(val), op);
                let (mx, mn) = extremes(ctx, &input, op);
                let b = if op == "-thr" {
                    Bounds {
                        lower: t,
                        upper: T::plus_int(mx, 1),
                        lower_open: false,
                        upper_open: true,
                        inclusive: true,
                    }
                } else {
                    Bounds {
                        lower: T::plus_int(mn, -1),
                        upper: t,
                        lower_open: true,
                        upper_open: false,
                        inclusive: true,
                    }
                };
                threshold_b(ctx, &mut input, &b, op);
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
                let (lo, _) = robust(ctx, &input, mask.as_ref(), op)?;
                let (_, hi) = robust(ctx, &input, mask.as_ref(), op)?;
                let _ = robust(ctx, &input, mask.as_ref(), op)?;
                let limit = robust_limit(ctx, lo, hi, val, op);
                let (mx, mn) = extremes(ctx, &input, op);
                let b = if op.starts_with("-thr") {
                    Bounds {
                        lower: limit,
                        upper: T::plus_int(mx, 1),
                        lower_open: false,
                        upper_open: true,
                        inclusive: true,
                    }
                } else {
                    Bounds {
                        lower: T::plus_int(mn, -1),
                        upper: limit,
                        lower_open: true,
                        upper_open: false,
                        inclusive: true,
                    }
                };
                threshold_b(ctx, &mut input, &b, op);
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
                let n = looped_len(&input);
                for k in 0..n {
                    let r = glibc_double::pow(input.data[k].to_f64(), p);
                    input.data[k] = ctx.to_t::<T>(r, "-pow");
                }
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
                let n = looped_len(&input);
                for k in 0..n {
                    let v = input.data[k];
                    if op == "-log" && !(v > T::from_int(0)) {
                        continue;
                    }
                    let x = v.to_f64();
                    let r = match op {
                        "-exp" => glibc_double::exp(x),
                        "-log" => glibc_double::log(x),
                        "-cos" => larmorx_core::math::cos(x),
                        "-sin" => larmorx_core::math::sin(x),
                        // libm (FreeBSD's msun): glibc's tan is not yet ported
                        "-tan" => libm::tan(x),
                        "-asin" => larmorx_core::math::asin(x),
                        "-acos" => larmorx_core::math::acos(x),
                        _ => larmorx_core::math::atan2(x, 1.0),
                    };
                    // a NaN argument passed on quieted; a NaN made from a number is SSE's default
                    let r = if x.is_nan() {
                        larmorx_nc_core::sse::quiet_f64(x)
                    } else if r.is_nan() {
                        larmorx_nc_core::sse::DEFAULT_NAN_F64
                    } else {
                        r
                    };
                    input.data[k] = ctx.to_t::<T>(r, op);
                }
            }
            "-abs" => {
                for k in 0..input.data.len() {
                    let v = input.data[k];
                    input.data[k] = if T::IS_FLOAT {
                        volops::abs_value(v)
                    } else {
                        ctx.to_t::<T>(v.to_f64().abs(), "-abs")
                    };
                }
            }
            "-bin" => bin_positive(ctx, &mut input, "-bin"),
            "-binv" => {
                bin_positive(ctx, &mut input, "-binv");
                let _ = input.op_scalar(Op::Mul, T::from_int(-1));
                let _ = input.op_scalar(Op::Add, T::from_int(1));
                let (mx, _) = extremes(ctx, &input, "-binv");
                let b = Bounds {
                    lower: T::from_f64(0.5),
                    upper: T::plus_int(mx, 1),
                    lower_open: false,
                    upper_open: true,
                    inclusive: false,
                };
                binarise_b(ctx, &mut input, &b, "-binv");
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
                let value = ctx.to_t::<T>(gv, "-grid");
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
                        ctx.to_t::<T>(rnd - 1.0, op)
                    } else {
                        let (z, msg) = cprob::ndtri(rnd - 1.0);
                        if let Some(m) = msg {
                            printed.push_str(&m);
                        }
                        ctx.to_t::<T>(z, op)
                    };
                    input.data[k] = T::add(input.data[k], add);
                }
                if !printed.is_empty() {
                    ctx.put(&printed);
                }
            }
            _ if NOT_YET.contains(&op) => return Err(not_yet(ctx, op)),
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
    // SIGNED_INT: the datatypes below DT_FLOAT)
    if T::IS_FLOAT && output_dt < dt::FLOAT {
        round_values(ctx, &mut input);
    }
    if modified {
        input.display_max = 0.0;
        input.display_min = 0.0;
    }
    if set_display_range {
        let (mx, mn) = extremes(ctx, &input, "-range");
        input.display_max = mx.to_f32();
        input.display_min = mn.to_f32();
    }
    let out_name = argv[argc - 1].clone();
    save_dtype(ctx, &input, &out_name, output_dt)?;
    Ok(0)
}

fn not_yet(ctx: &mut Ctx, op: &str) -> Stop {
    ctx.eput(&format!(
        "larmorx-nc: error: fslmaths: {op} is not translated yet in this build\n"
    ));
    Stop::Exit(1)
}

/// `sqrt` of a positive value in the type's own function (`sqrtf` for `float`, `sqrt` of the
/// `double` value for the others).
fn sqrt_t<T: Arith>(ctx: &mut Ctx, v: T) -> T {
    match T::DTYPE {
        dt::FLOAT => T::from_f32(v.to_f32().sqrt()),
        _ => ctx.to_t::<T>(v.to_f64().sqrt(), "-sqrt"),
    }
}

/// `-rem <number|image>`: `(int) value % (int) divisor`, converted back.
fn rem<T: Arith>(ctx: &mut Ctx, input: &mut Volume<T>, s: &str) -> Res<()> {
    let n = looped_len(input);
    if is_number(s) {
        let d = cnum::atof(s);
        let denom = ctx.to_int::<f64>(d, "-rem's divisor");
        for k in 0..n {
            let v = ctx.to_int(input.data[k], "-rem");
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
                        let v = ctx.to_int(input.data[k], "-rem");
                        let dd = ctx.to_int(d, "-rem's divisor");
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
            let val = ctx.to_t::<T>(d, "-div");
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
        let p = ctx.to_t::<T>(cnum::atof(s), if is_max { "-max" } else { "-min" });
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
            let fixed = if f.is_nan() { 0.0 } else { f.trunc().clamp(lo, hi) };
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
