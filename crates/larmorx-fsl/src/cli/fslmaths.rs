// SPDX-License-Identifier: Apache-2.0
//! `larmorx fsl fslmaths`: FSL's image calculator on FSL's arguments (`specs/fslmaths.md` §2):
//! `fslmaths [-dt <datatype>] <first_input> [operations and inputs] <output> [-odt <datatype>]`.
//!
//! The chain is read left to right; each operation takes its own arguments and acts on the
//! current image at once, in the computing type `-dt` sets (the whole chain is generic over it:
//! [`Chain`]). The operations of this file are §5-§8's (the binary operations, thresholds,
//! masks, voxelwise functions, random numbers, the index and the grid) and the chain's own
//! (`-seed`, `-restart`, `-save`, `-range`), the kernels and spatial filters (§9, §10), the
//! reductions (§11), the statistical transforms (§12), `-roi` and `-bptf` (§13), TFCE (§14),
//! the ROC analysis (§15) and `-tensor_decomp` (§16): every operation of the usage text and the
//! hidden ones the spec lists.

use std::io::Write;

use larmorx_core::defects::{self, Notices, UpstreamDefects};
use larmorx_core::element::DataType;
use larmorx_mri::hmc::image::ImageError;

use super::Settings;
use super::common;
use super::options::strtod;
use crate::flirt::mat::atoi;
use crate::maths::filters::{self, Morph};
use crate::maths::image::{self, Img, OutType, Stored, WriteError, WriteSpec};
use crate::maths::kernel::Kernel;
use crate::maths::num::{Kind, Num};
use crate::maths::ops::{self, Binary, Layout, Stop, Unary, WichmannHill};
use crate::maths::reduce::{self, Stat};
use crate::maths::stats::{self, PvalKind};
use crate::maths::{regions, roc, spatial, tensor, tfce};

/// The usage text (§2.5), exactly as FSL prints it on standard output.
pub const USAGE: &str = include_str!("fslmaths_usage.txt");

/// The datatype names of `-dt` and `-odt`.
const DATATYPES: [&str; 6] = ["char", "short", "int", "float", "double", "input"];

/// How a run ends early: its exit status (the messages are printed).
struct Exit(u8);

/// The C++ runtime's lines for a null string (FSL-158 with `reproduce`).
const NULL_STRING: &str = "basic_string: construction from null is not valid";

/// Runs fslmaths.
pub fn main(args: &[String], settings: &Settings, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let mode = match settings.defects() {
        Ok(m) => m,
        Err(e) => {
            let _ = writeln!(err, "larmorx fsl fslmaths: {e}");
            return defects::EXIT_BAD_MODE;
        }
    };
    let mut notices = Notices::new(mode);
    let code = match run(args, settings, &mut notices, out, err) {
        Ok(()) => 0,
        Err(Exit(c)) => c,
    };
    notices.print(err);
    code
}

fn usage(out: &mut dyn Write) {
    let _ = out.write_all(USAGE.as_bytes());
}

/// FSL-158: an argument read past the end of the command line. `string`: FSL reads it as a
/// string (the C++ runtime's `logic_error`, reproduced), else as a number (a crash).
fn past_end(notices: &mut Notices, err: &mut dyn Write, what: &str, string: bool) -> Exit {
    if string && notices.reproduce() {
        let _ = writeln!(
            err,
            "{}",
            common_abort_text("std::logic_error", NULL_STRING)
        );
        let _ = writeln!(
            err,
            "larmorx fsl fslmaths: {what} reads a null argument past the end of the command line \
             (FSL-158 reproduced); FSL aborts here (signal 6); exit status 1 (larmorx)"
        );
        return Exit(1);
    }
    let _ = writeln!(
        err,
        "larmorx: error: fslmaths: {what} reads past the end of the command line (FSL-158: FSL \
         crashes or reads the environment here)"
    );
    Exit(1)
}

fn common_abort_text(kind: &str, what: &str) -> String {
    format!("terminate called after throwing an instance of '{kind}'\n  what():  {what}")
}

/// The image layer's lines for an image that cannot be read, then FSL's abort.
fn read_failure(err: &mut dyn Write, name: &str, e: &ImageError, input: bool) -> Exit {
    let text = e.to_string();
    let short = text.contains("bytes of voxel data")
        || text.contains("failed to fill whole buffer")
        || text.contains("unexpected end")
        || text.contains("early eof")
        || text.contains("incomplete")
        || text.contains("truncated");
    let missing = text.starts_with("No image files match") || text.contains("No such file");
    let what = if short {
        format!("Failed to read volume {name}\nError : Error: short read, file may be truncated")
    } else if input && (missing || text.starts_with("Multiple possible")) {
        // the input: fslmaths' own words (observed), the #63 line and the exception, no #22 line
        let stem = larmorx_mri::hmc::files::strip_image_extension(name);
        let what = if missing {
            format!("No image files match: {stem}")
        } else {
            format!("Multiple possible filenames detected for basename: {stem}")
        };
        let _ = writeln!(err, "Image Exception : #63 :: {what}");
        common::abort(err, "fslmaths", "std::runtime_error", &what);
        return Exit(1);
    } else if missing || text.starts_with("Multiple possible") {
        // an operand: the image layer's #63, #22 and Error lines (observed)
        let (lines, what) = common::image_exception(name, e);
        let _ = err.write_all(lines.as_bytes());
        common::abort(err, "fslmaths", "std::runtime_error", &what);
        return Exit(1);
    } else {
        format!("Failed to read volume {name}\nError : {text}")
    };
    let _ = writeln!(err, "Image Exception : #22 :: {what}");
    common::abort(err, "fslmaths", "std::runtime_error", &what);
    Exit(1)
}

/// The output type `-odt` names (`None`: unknown), given the input's type for `input`.
fn out_type(name: &str, input: DataType, mode: UpstreamDefects) -> Option<OutType> {
    Some(match name {
        "char" => OutType::Char,
        "short" => OutType::Short,
        "int" => OutType::Int,
        "float" => OutType::Float,
        "double" => OutType::Double,
        "input" => match input {
            DataType::U8 => OutType::Char,
            DataType::I8 if mode.fix() => OutType::SChar,
            DataType::I8 => OutType::Char,
            DataType::I16 => OutType::Short,
            DataType::I32 | DataType::U16 => OutType::Int,
            DataType::F64 => OutType::Double,
            _ => OutType::Float,
        },
        _ => return None,
    })
}

fn datatype_error(out: &mut dyn Write, name: &str) -> Exit {
    let _ = writeln!(
        out,
        "Error: Unknown datatype \"{name}\" - Possible datatypes are: char short int float double input"
    );
    Exit(1)
}

fn run(
    args: &[String],
    settings: &Settings,
    notices: &mut Notices,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<(), Exit> {
    if args.is_empty() {
        usage(out);
        return Err(Exit(1));
    }
    if args[0] == "-h" || args[0] == "--help" {
        usage(out);
        return Ok(());
    }
    let mut words: Vec<String> = args.to_vec();
    let mut odt: Option<String> = None;
    let n = words.len();
    if n >= 2 && (words[n - 2] == "-odt" || words[n - 2] == "-output_datatype") {
        odt = Some(words[n - 1].clone());
        words.truncate(n - 2);
    }
    if let Some(o) = &odt
        && !DATATYPES.contains(&o.as_str())
    {
        return Err(datatype_error(out, o));
    }
    let mut first = 0;
    let mut dt: Option<String> = None;
    if words
        .first()
        .is_some_and(|w| w == "-dt" || w == "-datatype")
    {
        match words.get(1) {
            Some(d) => dt = Some(d.clone()),
            None => return Err(past_end(notices, err, "-dt", true)),
        }
        first = 2;
        if let Some(d) = &dt
            && !DATATYPES.contains(&d.as_str())
        {
            return Err(datatype_error(out, d));
        }
    }
    let Some(input) = words.get(first).cloned() else {
        return Err(past_end(notices, err, "the input", true));
    };
    let n_threads = settings.threads();
    let mode = notices.mode();
    let mut raw = Vec::new();
    let stored = crate::bet::pooled(n_threads, || {
        let mut ns = Vec::new();
        let r = image::read_stored(settings, &input, mode, &mut ns, n_threads);
        (r, ns)
    });
    let (stored, ns) = stored;
    raw.extend(ns);
    notices.extend(raw);
    let stored = stored.map_err(|e| read_failure(err, &input, &e, true))?;
    let effective = stored.effective_type();
    // the computing type (§2.4)
    let kind = match dt.as_deref() {
        Some("char") => {
            if stored.stored == DataType::U8 && stored.scale.is_none() && mode.fix() {
                Kind::UChar
            } else {
                Kind::Char
            }
        }
        Some("short") => Kind::Short,
        Some("int") => Kind::Int,
        Some("float") => Kind::Float,
        Some("double") => Kind::Double,
        Some("input") => match effective {
            DataType::U8 if mode.fix() => Kind::UChar,
            DataType::U8 => Kind::Char,
            DataType::I16 => Kind::Short,
            DataType::I32 => Kind::Int,
            DataType::F32 => Kind::Float,
            DataType::F64 => Kind::Double,
            other => {
                // FSL-155: FSL computes nothing, writes nothing and exits 0
                let closest = match other {
                    DataType::I8 => Kind::Char,
                    DataType::U16 => Kind::Int,
                    _ => Kind::Float,
                };
                let detail = match mode {
                    UpstreamDefects::Fix => format!(
                        "-dt input of an image stored as {other:?}: computed in {} (FSL computes nothing, writes no output and exits 0)",
                        closest.name()
                    ),
                    UpstreamDefects::Reproduce => format!(
                        "-dt input of an image stored as {other:?}: nothing computed, no output, exit 0, as FSL does"
                    ),
                };
                notices.add(&defects::FSL_155, detail);
                if mode.reproduce() {
                    return Ok(());
                }
                closest
            }
        },
        _ => {
            if effective == DataType::F64 {
                Kind::Double
            } else {
                Kind::Float
            }
        }
    };
    let odt_name = odt.as_deref().unwrap_or(if effective == DataType::F64 {
        "double"
    } else {
        "float"
    });
    let out_t = out_type(odt_name, effective, mode).unwrap_or(OutType::Float);
    // FSL-157: -odt input on int8 and uint16 truncates; int8 written as uint8
    let fsl157 = odt.as_deref() == Some("input")
        && matches!(effective, DataType::I8 | DataType::U16)
        && !kind.is_int();
    let round = !(fsl157 && mode.reproduce());
    let ctx = Ctx {
        settings,
        words: &words,
        first,
        n_threads,
        out_t,
        round,
        fsl157,
        stored_type: stored.stored,
    };
    let stored = std::sync::Mutex::new(stored);
    // the chain runs on the thread pool of the run's thread count; its messages are gathered
    // and printed after it, each stream in its order
    let (r, o, e, ns) = crate::bet::pooled(n_threads, || {
        let mut o: Vec<u8> = Vec::new();
        let mut e: Vec<u8> = Vec::new();
        let mut ns = Notices::new(mode);
        // the input moved into the chain (the closure runs once)
        let mut guard = stored
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let stored = &mut *guard;
        let r = match kind {
            Kind::Char => chain::<i8>(&ctx, stored, &mut ns, &mut o, &mut e),
            Kind::UChar => chain::<u8>(&ctx, stored, &mut ns, &mut o, &mut e),
            Kind::Short => chain::<i16>(&ctx, stored, &mut ns, &mut o, &mut e),
            Kind::Int => chain::<i32>(&ctx, stored, &mut ns, &mut o, &mut e),
            Kind::Float => chain::<f32>(&ctx, stored, &mut ns, &mut o, &mut e),
            Kind::Double => chain::<f64>(&ctx, stored, &mut ns, &mut o, &mut e),
        };
        (r, o, e, ns)
    });
    let _ = out.write_all(&o);
    let _ = err.write_all(&e);
    notices.extend(ns.into_vec());
    r
}

/// What every step of the chain needs.
struct Ctx<'a> {
    settings: &'a Settings,
    words: &'a [String],
    first: usize,
    n_threads: usize,
    out_t: OutType,
    round: bool,
    fsl157: bool,
    stored_type: DataType,
}

/// Reads an image operand in the computing type.
fn operand<T: Num>(
    ctx: &Ctx<'_>,
    name: &str,
    notices: &mut Notices,
    err: &mut dyn Write,
) -> Result<Img<T>, Exit> {
    let mut ns = Vec::new();
    let s = image::read_stored(ctx.settings, name, notices.mode(), &mut ns, ctx.n_threads);
    notices.extend(ns);
    let mut s = s.map_err(|e| read_failure(err, name, &e, false))?;
    Ok(typed(&mut s, notices))
}

/// A stored image in the computing type (FSL-151, FSL-154 noted); its stored values are moved
/// or dropped (one copy of the image held).
fn typed<T: Num>(s: &mut Stored, notices: &mut Notices) -> Img<T> {
    let fix = notices.fix();
    let (data, changed) = s.take_values::<T>(fix);
    let uint8_as_char = s.stored == DataType::U8 && s.scale.is_none() && T::KIND == Kind::Char;
    if uint8_as_char {
        // FSL-154 reproduced: the bytes 128-255 read as negative (C's conversion)
        if changed {
            notices.add(
                &defects::FSL_154,
                "the uint8 values from 128 to 255 are read as negative signed bytes, as FSL does",
            );
        }
    } else if changed {
        image::fsl151(notices, "the image's values in the computing type");
    }
    if T::KIND == Kind::UChar
        && s.stored == DataType::U8
        && data.iter().any(|&v| v.to_f64() >= 128.0)
    {
        notices.add(
            &defects::FSL_154,
            "the uint8 image's char computing type is unsigned: its values from 128 to 255 stay \
             positive (FSL reads them as negative)",
        );
    }
    let cal = (s.header.cal_min, s.header.cal_max);
    Img {
        dims: s.dims,
        data,
        header: s.header.clone(),
        flipped: s.flipped,
        cal,
    }
}

/// The chain's state.
struct State<T> {
    img: Img<T>,
    rng: WichmannHill,
    range: bool,
    /// The current kernel (§9; `-restart` keeps it).
    kernel: Kernel,
    /// `-tfce_minT` and `-tfce_delta` (§14).
    tfce: tfce::Settings,
}

fn chain<T: Num>(
    ctx: &Ctx<'_>,
    stored: &mut Stored,
    notices: &mut Notices,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<(), Exit> {
    let words = ctx.words;
    let n = words.len();
    let mut st = State {
        img: typed::<T>(stored, notices),
        rng: WichmannHill::default(),
        range: false,
        kernel: Kernel::default(),
        tfce: tfce::Settings::default(),
    };
    let mut i = ctx.first + 1;
    let mut ran = false;
    while i < n.saturating_sub(1) {
        let used = step(ctx, &mut st, i, notices, out, err)?;
        ran = true;
        i += 1 + used;
    }
    if i > n.saturating_sub(1) || n <= ctx.first + 1 {
        let _ = writeln!(err, "Error: no output filename specified!");
        return Err(Exit(1));
    }
    let output = &words[n - 1];
    // the display range (§3.3): the input's (as `-save` writes it) unless an operation ran;
    // `-range`'s at the end
    if ran {
        st.img.cal = (0.0, 0.0);
    }
    if st.range
        && let Some((lo, hi)) = range_of(&st.img.data, notices)
    {
        st.img.cal = (lo, hi);
    }
    let spec = WriteSpec {
        out: ctx.out_t,
        round: ctx.round,
        fix: notices.fix(),
    };
    // FSL-157 decided from the values before they move to the writer (named after FSL-151)
    let mut n157 = Notices::new(notices.mode());
    if ctx.fsl157 {
        note157(ctx, &st.img, &mut n157);
    }
    let (values, changed) = match image::identity_values(&mut st.img, spec) {
        Some(v) => (v, false),
        None => image::output_values(&st.img, spec),
    };
    if changed {
        image::fsl151(notices, "the output's values");
    }
    notices.extend(n157.into_vec());
    write_image(ctx, &st.img, output, values, notices, err)
}

/// FSL-157's notice where truncating changes a value, and for every int8 image (FSL writes its
/// output as uint8: the header's data type changes whatever the values; random testing,
/// 2026-11-03).
fn note157<T: Num>(ctx: &Ctx<'_>, img: &Img<T>, notices: &mut Notices) {
    let int8 = ctx.stored_type == DataType::I8;
    let differs = int8
        || ops::any(&img.data, |v| {
            let x = v.to_f64();
            let rounded = if x > 0.0 {
                (x + 0.5).trunc()
            } else {
                (x - 0.5).trunc()
            };
            rounded != x.trunc() || (int8 && x.trunc() < 0.0)
        });
    if differs {
        let detail = match notices.mode() {
            UpstreamDefects::Fix => {
                "-odt input on an int8 or uint16 image: the values rounded \
                                     (FSL truncates them) and int8 written as int8 (FSL writes \
                                     uint8)"
            }
            UpstreamDefects::Reproduce => {
                "-odt input on an int8 or uint16 image: the values \
                                           truncated (and int8 written as uint8), as FSL does"
            }
        };
        notices.add(&defects::FSL_157, detail);
    }
}

/// The display range of `-range`: the final image's extremes (FSL-153: from the first value).
fn range_of<T: Num>(data: &[T], notices: &mut Notices) -> Option<(f64, f64)> {
    let fix = notices.fix();
    let rep = ops::extremes(data, false)?;
    let got = if fix { ops::extremes(data, true)? } else { rep };
    if !(got.0.same(rep.0) && got.1.same(rep.1)) {
        notices.add(
            &defects::FSL_153,
            "-range: the display range of the values that are not NaN (FSL's is NaN: its first \
             voxel is NaN)",
        );
    }
    Some((got.0.to_f64(), got.1.to_f64()))
}

fn write_image<T: Num>(
    ctx: &Ctx<'_>,
    img: &Img<T>,
    name: &str,
    values: crate::bet::image::Values,
    notices: &mut Notices,
    err: &mut dyn Write,
) -> Result<(), Exit> {
    let mode = notices.mode();
    match image::write(
        ctx.settings,
        img,
        name,
        values,
        mode,
        notices,
        ctx.n_threads,
    ) {
        Ok(()) => Ok(()),
        Err(WriteError::OutputType(m)) => {
            let _ = err.write_all(m.as_bytes());
            Err(Exit(1))
        }
        Err(WriteError::File(m)) => {
            let _ = writeln!(err, "larmorx: error: fslmaths: cannot write {m}");
            Err(Exit(1))
        }
    }
}

/// Whether a word is a number (C's `strtod` reads it whole), and its value.
fn number_of(word: &str) -> Option<f64> {
    let (v, used) = strtod(word);
    (used > 0 && used == word.len()).then_some(v)
}

fn refuse_stop(err: &mut dyn Write, s: Stop) -> Exit {
    match s {
        Stop::DivZero(what) => {
            let _ = writeln!(
                err,
                "larmorx: error: fslmaths: {what} (FSL-156: an integer division by zero, FSL \
                 crashes here)"
            );
        }
        Stop::Exception(lines) => {
            let _ = err.write_all(lines.as_bytes());
        }
        Stop::Refuse(why) => {
            let _ = writeln!(err, "larmorx: error: fslmaths: {why}");
        }
    }
    Exit(1)
}

/// One operation at `words[i]`; returns how many arguments it took.
fn step<T: Num>(
    ctx: &Ctx<'_>,
    st: &mut State<T>,
    i: usize,
    notices: &mut Notices,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<usize, Exit> {
    let words = ctx.words;
    let op = words[i].as_str();
    let arg = |k: usize| words.get(i + k).map(String::as_str);
    if let Some(b) = Binary::of(op) {
        let a = arg(1).unwrap_or("");
        match number_of(a) {
            Some(v) => {
                let x = ops::number::<T>(v, notices);
                ops::with_number(&mut st.img, b, x, v == 0.0, notices)
                    .map_err(|s| refuse_stop(err, s))?;
            }
            None => {
                let w = operand::<T>(ctx, a, notices, err)?;
                binary_image(ctx, st, b, w, notices, err)?;
            }
        }
        return Ok(1);
    }
    if let Some(f) = Unary::of(op) {
        ops::unary(&mut st.img, f, notices);
        return Ok(0);
    }
    match op {
        "-mas" => {
            let a = arg(1).unwrap_or("");
            let mut w = operand::<T>(ctx, a, notices, err)?;
            ops::bin_values(&mut w.data, notices, "-mas");
            binary_image(ctx, st, Binary::Mul, w, notices, err)?;
            Ok(1)
        }
        "-thr" | "-uthr" | "-thrp" | "-uthrp" | "-thrP" | "-uthrP" => {
            let a = arg(1).unwrap_or("");
            let Some(v) = number_of(a) else {
                let _ = writeln!(err, "Error: {op} requires a number");
                return Err(Exit(1));
            };
            let lower = op.starts_with("-thr");
            let t = if op.len() > 4 && (op.ends_with('p') || op.ends_with('P')) {
                ops::percent_limit(&st.img.data, v, op.ends_with('P'), notices, err)
            } else {
                ops::Limit::exact(ops::number::<T>(v, notices))
            };
            ops::threshold(&mut st.img, &t, lower, notices, op);
            Ok(1)
        }
        "-bin" => {
            ops::bin_values(&mut st.img.data, notices, "-bin");
            Ok(0)
        }
        "-binv" => {
            ops::binv(&mut st.img, notices);
            Ok(0)
        }
        "-invbin" => {
            ops::invbin(&mut st.img);
            Ok(0)
        }
        "-pow" => {
            let p = strtod(arg(1).unwrap_or("")).0;
            ops::unary(&mut st.img, Unary::Pow(p), notices);
            Ok(1)
        }
        "-index" => {
            ops::index(&mut st.img);
            Ok(0)
        }
        "-grid" => {
            let v = strtod(arg(1).unwrap_or("")).0;
            let Some(s) = arg(2) else {
                return Err(past_end(notices, err, "-grid", false));
            };
            let s = atoi(s);
            if s == 0 {
                return Err(refuse_stop(err, Stop::DivZero("-grid with a spacing of 0")));
            }
            let x = ops::number::<T>(v, notices);
            ops::grid(&mut st.img, x, s);
            Ok(2)
        }
        "-rand" => {
            ops::rand(&mut st.img, &mut st.rng, notices);
            Ok(0)
        }
        "-seed" => {
            st.rng = WichmannHill::seeded(atoi(arg(1).unwrap_or("")));
            Ok(1)
        }
        "-range" => {
            st.range = true;
            Ok(0)
        }
        "-restart" => {
            let a = arg(1).unwrap_or("");
            st.img = operand::<T>(ctx, a, notices, err)?;
            Ok(1)
        }
        "-save" => {
            let a = arg(1).unwrap_or("").to_owned();
            let spec = WriteSpec {
                out: image::save_type(T::KIND),
                round: false,
                fix: notices.fix(),
            };
            let (values, ch) = image::output_values(&st.img, spec);
            if ch {
                image::fsl151(notices, "-save's char bytes");
            }
            write_image(ctx, &st.img, &a, values, notices, err)?;
            Ok(1)
        }
        "-kernel" => kernel_step(ctx, st, i, notices, err),
        "-s" => {
            let sigma = strtod(arg(1).unwrap_or("")).0;
            st.kernel = Kernel::gauss(sigma, st.img.voxel());
            filters::smooth(&mut st.img, &st.kernel, notices, err)
                .map_err(|s| refuse_stop(err, s))?;
            Ok(1)
        }
        "-fmean" | "-fmeanu" => {
            filters::fmean(&mut st.img, &st.kernel, op == "-fmean", op, notices, err)
                .map_err(|s| refuse_stop(err, s))?;
            Ok(0)
        }
        "-dilall" => {
            filters::dilall(&mut st.img, notices);
            Ok(0)
        }
        "-subsamp2" | "-subsamp2offc" => {
            spatial::subsample(&mut st.img, op == "-subsamp2offc", notices)
                .map_err(|s| refuse_stop(err, s))?;
            Ok(0)
        }
        "-edge" => {
            spatial::edge(&mut st.img, notices);
            Ok(0)
        }
        "-dog_edge" => {
            let (Some(a), Some(b)) = (arg(1), arg(2)) else {
                return Err(past_end(notices, err, "-dog_edge", false));
            };
            let (s1, s2) = (strtod(a).0, strtod(b).0);
            spatial::dog_edge(&mut st.img, s1, s2, notices).map_err(|s| refuse_stop(err, s))?;
            Ok(2)
        }
        "-fillh" | "-fillh26" => {
            spatial::fill_holes(&mut st.img, op == "-fillh26", notices)
                .map_err(|s| refuse_stop(err, s))?;
            Ok(0)
        }
        "-inm" | "-ing" => {
            let m = strtod(arg(1).unwrap_or("")).0;
            spatial::normalise(&mut st.img, m, op == "-ing", notices, err);
            Ok(1)
        }
        "-randn" => {
            stats::randn(&mut st.img, &mut st.rng, notices, out);
            Ok(0)
        }
        "-ztop" => {
            stats::ztop(&mut st.img, notices);
            Ok(0)
        }
        "-ptoz" => {
            stats::ptoz(&mut st.img, notices, out);
            Ok(0)
        }
        "-pval" | "-pval0" | "-cpval" => {
            let kind = match op {
                "-pval" => PvalKind::Pval,
                "-pval0" => PvalKind::Pval0,
                _ => PvalKind::Cpval,
            };
            stats::pval(&mut st.img, kind, notices);
            Ok(0)
        }
        "-rank" | "-ranknorm" => {
            stats::rank(&mut st.img, op == "-ranknorm", notices, out)
                .map_err(|s| refuse_stop(err, s))?;
            Ok(0)
        }
        "-roi" => {
            let mut a = [0i32; 8];
            for (k, v) in a.iter_mut().enumerate() {
                let Some(w) = arg(k + 1) else {
                    return Err(past_end(notices, err, "-roi", false));
                };
                *v = atoi(w);
            }
            regions::roi(&mut st.img, a);
            Ok(8)
        }
        "-bptf" => {
            let hp = strtod(arg(1).unwrap_or("")).0;
            let Some(l) = arg(2) else {
                return Err(past_end(notices, err, "-bptf", false));
            };
            let lp = strtod(l).0;
            regions::bptf(&mut st.img, hp, lp, notices).map_err(|s| refuse_stop(err, s))?;
            Ok(2)
        }
        "-tfce_minT" | "-tfce_delta" => {
            let v = strtod(arg(1).unwrap_or("")).0 as f32;
            if op == "-tfce_minT" {
                st.tfce.min_t = v;
            } else {
                st.tfce.delta = v;
            }
            Ok(1)
        }
        "-tfce" | "-tfceS" => {
            let k = if op == "-tfce" { 3 } else { 7 };
            let mut a: Vec<&str> = Vec::with_capacity(k);
            for j in 1..=k {
                let Some(w) = arg(j) else {
                    return Err(past_end(notices, err, op, false));
                };
                a.push(w);
            }
            let num = |w: &str| strtod(w).0 as f32;
            let (h, e, c) = (num(a[0]), num(a[1]), atoi(a[2]));
            let r = if op == "-tfce" {
                tfce::tfce(&mut st.img, h, e, c, st.tfce, notices, out)
            } else {
                let at = [atoi(a[3]), atoi(a[4]), atoi(a[5])];
                tfce::tfce_support(&mut st.img, h, e, c, at, num(a[6]), st.tfce, notices, out)
            };
            r.map_err(|s| refuse_stop(err, s))?;
            Ok(k)
        }
        "-tensor_decomp" => {
            tensor_step(ctx, st, notices, out, err)?;
            Ok(0)
        }
        "-roc" => roc_step(ctx, st, i, notices, out, err),
        _ if Morph::of(op).is_some() => {
            let f = Morph::of(op).unwrap_or(Morph::DilF);
            filters::morph(&mut st.img, &st.kernel, f, op, notices, err)
                .map_err(|s| refuse_stop(err, s))?;
            Ok(0)
        }
        _ if op.len() >= 2 && op.as_bytes()[1].is_ascii_uppercase() => {
            reduction_step(ctx, st, i, notices, err)
        }
        _ => {
            let _ = write!(out, "\n Error in command line: unknown option \"{op}\"\n\n");
            usage(out);
            Err(Exit(1))
        }
    }
}

/// `-roc L curve [noise] truth` (§15): `L < 0` takes no noise image. The curve file is created
/// first (FSL-193: one that cannot be written is an error by default; FSL skips it silently),
/// then the noise and truth images are read (single precision; a 4D truth's first volume with
/// the warning) and the truth's sizes checked; the current image is unchanged.
fn roc_step<T: Num>(
    ctx: &Ctx<'_>,
    st: &mut State<T>,
    i: usize,
    notices: &mut Notices,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<usize, Exit> {
    let words = ctx.words;
    let arg = |k: usize| words.get(i + k).map(String::as_str);
    let Some(l) = arg(1) else {
        return Err(past_end(notices, err, "-roc", false));
    };
    let limit = strtod(l).0;
    let with_noise = !(limit < 0.0);
    let used = if with_noise { 4 } else { 3 };
    let mut names = Vec::with_capacity(3);
    for k in 2..=used {
        let Some(w) = arg(k) else {
            return Err(past_end(notices, err, "-roc", true));
        };
        names.push(w);
    }
    let curve_name = names[0];
    let file = match std::fs::File::create(ctx.settings.path(curve_name)) {
        Ok(f) => Some(f),
        Err(e) if notices.fix() => {
            notices.add(
                &defects::FSL_193,
                format!(
                    "-roc's curve file {curve_name} cannot be written: an error (FSL writes no \
                     curve and says nothing)"
                ),
            );
            let _ = writeln!(
                err,
                "larmorx: error: fslmaths: -roc: cannot write the curve file {curve_name}: {e} \
                 (FSL-193: FSL skips it silently)"
            );
            return Err(Exit(1));
        }
        Err(_) => {
            notices.add(
                &defects::FSL_193,
                format!("-roc's curve file {curve_name} cannot be written: skipped silently, as FSL does"),
            );
            None
        }
    };
    let read = |name: &str, notices: &mut Notices, err: &mut dyn Write| {
        let mut ns = Vec::new();
        let s = image::read_stored(ctx.settings, name, notices.mode(), &mut ns, ctx.n_threads);
        notices.extend(ns);
        let s = s.map_err(|e| read_failure(err, name, &e, false))?;
        let (data, _) = s.values::<f32>(notices.fix());
        Ok::<_, Exit>((data, s.dims))
    };
    let mut noise = if with_noise {
        let (data, dims) = read(names[1], notices, err)?;
        Some(roc::Noise { data, dims })
    } else {
        None
    };
    let (mut truth, tdims) = read(names[used - 2], notices, err)?;
    if tdims[3] > 1 {
        let _ = writeln!(err, "{}", common::TRUNCATED);
    }
    let [nx, ny, nz, _] = st.img.dims;
    if tdims[..3] != [nx, ny, nz] {
        let what = "Attempted to multiply images of different sizes";
        let _ = writeln!(err, "Image Exception : #3 :: {what}");
        common::abort(err, "fslmaths", "std::runtime_error", what);
        return Err(Exit(1));
    }
    truth.truncate(nx * ny * nz);
    let r = roc::roc(&st.img, limit, noise.as_mut(), &truth, notices)
        .map_err(|s| refuse_stop(err, s))?;
    if let Some(mut f) = file
        && let Err(e) = f.write_all(r.curve.as_bytes())
    {
        let _ = writeln!(
            err,
            "larmorx: error: fslmaths: -roc: cannot write the curve file {curve_name}: {e}"
        );
        return Err(Exit(1));
    }
    match r.result {
        Ok(line) => {
            let _ = out.write_all(line.as_bytes());
            Ok(used)
        }
        Err(abort) => {
            let _ = err.write_all(abort.as_bytes());
            Err(Exit(1))
        }
    }
}

/// `-tensor_decomp` (§16): the nine images written at once under the output's name with
/// `_L1` ... `_V3` appended (single precision), then the chain goes on with the FA image.
fn tensor_step<T: Num>(
    ctx: &Ctx<'_>,
    st: &mut State<T>,
    notices: &mut Notices,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<(), Exit> {
    if st.img.dims[3] != 6 {
        let _ = writeln!(out, "{}", tensor::NOT_A_TENSOR);
        return Err(Exit(1));
    }
    let d = tensor::decompose(&st.img, notices).map_err(|s| refuse_stop(err, s))?;
    let [nx, ny, nz, _] = st.img.dims;
    let base = ctx.words[ctx.words.len() - 1].clone();
    let extremes = |v: &[f32]| {
        ops::extremes(v, false).map_or((0.0, 0.0), |(a, b)| (f64::from(a), f64::from(b)))
    };
    // (suffix, values, volumes, display range: the values' extremes when None)
    type Part<'a> = (&'a str, &'a Vec<f32>, usize, Option<(f64, f64)>);
    let parts: [Part<'_>; 9] = [
        ("L1", &d.l1, 1, None),
        ("L2", &d.l2, 1, None),
        ("L3", &d.l3, 1, None),
        ("FA", &d.fa, 1, Some((0.0, 1.0))),
        ("MD", &d.md, 1, None),
        ("MO", &d.mo, 1, Some((-1.0, 1.0))),
        ("V1", &d.v1, 3, Some((-1.0, 1.0))),
        ("V2", &d.v2, 3, Some((-1.0, 1.0))),
        ("V3", &d.v3, 3, Some((-1.0, 1.0))),
    ];
    for (suffix, data, nt, cal) in parts {
        let img = Img {
            dims: [nx, ny, nz, nt],
            data: data.clone(),
            header: st.img.header.clone(),
            flipped: st.img.flipped,
            cal: cal.unwrap_or_else(|| extremes(data)),
        };
        let spec = WriteSpec {
            out: OutType::Float,
            round: true,
            fix: notices.fix(),
        };
        let (values, _) = image::output_values(&img, spec);
        write_image(ctx, &img, &format!("{base}_{suffix}"), values, notices, err)?;
    }
    st.img.data = d.fa.iter().map(|&v| T::from_f64_c(f64::from(v))).collect();
    st.img.dims = [nx, ny, nz, 1];
    Ok(())
}

/// A reduction (§11): the word's second character an upper-case letter (`T`, `X`, `Y`, `Z`:
/// the axis; another: none), the rest the statistic; `perc` reads the next argument.
fn reduction_step<T: Num>(
    ctx: &Ctx<'_>,
    st: &mut State<T>,
    i: usize,
    notices: &mut Notices,
    err: &mut dyn Write,
) -> Result<usize, Exit> {
    let words = ctx.words;
    let op = words[i].as_str();
    let axis = reduce::axis_of(op.as_bytes()[1]);
    let rest = op.get(2..).unwrap_or("");
    let Some(mut stat) = Stat::of(rest) else {
        let _ = writeln!(err, "Error unknown Dimensionality operation: {op}");
        return Err(Exit(1));
    };
    let mut used = 0;
    if rest == "perc" {
        let p = strtod(words.get(i + 1).map_or("", String::as_str)).0;
        let f = (p / 100.0) as f32;
        if f < 0.0 || f > 1.0 {
            let what = reduce::PERCENTILE_RANGE;
            let _ = writeln!(err, "Image Exception : #4 :: {what}");
            common::abort(err, "fslmaths", "std::runtime_error", what);
            return Err(Exit(1));
        }
        stat = Stat::Perc(f);
        used = 1;
    }
    reduce::reduce(&mut st.img, axis, stat, op, notices).map_err(|s| refuse_stop(err, s))?;
    Ok(used)
}

/// `-kernel <type> [arguments]` (§9): the numeric argument is read before the type is looked
/// at; `3D` and `2D` take none, `boxv3` three, the others (an unknown type too) one.
fn kernel_step<T: Num>(
    ctx: &Ctx<'_>,
    st: &mut State<T>,
    i: usize,
    notices: &mut Notices,
    err: &mut dyn Write,
) -> Result<usize, Exit> {
    let words = ctx.words;
    let arg = |k: usize| words.get(i + k).map(String::as_str);
    let Some(kind) = arg(1) else {
        return Err(past_end(notices, err, "-kernel", true));
    };
    let Some(p) = arg(2) else {
        return Err(past_end(notices, err, "-kernel", false));
    };
    let x = strtod(p).0;
    let vox = st.img.voxel();
    let sep = st.kernel.separable;
    let int = |v: f64| i64::from(crate::maths::num::cvtt(v));
    let (k, used) = match kind {
        "3D" => (Kernel::ones([3, 3, 3], sep), 1),
        "2D" => (Kernel::ones([3, 3, 1], sep), 1),
        "box" => (Kernel::box_mm(x, vox), 2),
        "boxv" => {
            let n = int(x);
            (Kernel::ones([n, n, n], true), 2)
        }
        "boxv3" => {
            let (Some(y), Some(z)) = (arg(3), arg(4)) else {
                return Err(past_end(notices, err, "-kernel boxv3", false));
            };
            (
                Kernel::ones([int(x), int(strtod(y).0), int(strtod(z).0)], true),
                4,
            )
        }
        "gauss" => (Kernel::gauss(x, vox), 2),
        "sphere" => (Kernel::sphere(x, vox), 2),
        "file" => {
            let mut ns = Vec::new();
            let s = image::read_stored(ctx.settings, p, notices.mode(), &mut ns, ctx.n_threads);
            notices.extend(ns);
            let s = s.map_err(|e| read_failure(err, p, &e, false))?;
            let (data, _) = s.values::<f32>(notices.fix());
            let [nx, ny, nz, nt] = s.dims;
            if nt > 1 {
                let _ = writeln!(err, "{}", common::TRUNCATED);
            }
            let mut data = data;
            data.truncate(nx * ny * nz);
            (Kernel::from_values([nx, ny, nz], data), 2)
        }
        _ => (Kernel::none(sep), 2),
    };
    st.kernel = k;
    Ok(used)
}

/// The voxel-to-world matrix FSL compares (§3.2): the sform when set, else the qform, in the
/// radiological frame; `None` without either.
fn world<T>(img: &Img<T>) -> Option<[[f64; 4]; 4]> {
    let h = &img.header;
    let m = if h.sform_code > 0 {
        *h.sform().rows()
    } else if h.qform_code > 0 {
        *h.qform().ok()?.rows()
    } else {
        return None;
    };
    if !img.flipped {
        return Some(m);
    }
    // voxel i of the radiological frame is nx - 1 - i of the file
    let nx = img.dims[0] as f64;
    let mut r = m;
    for row in &mut r {
        row[3] += row[0] * (nx - 1.0);
        row[0] = -row[0];
    }
    Some(r)
}

/// The orientation warning when the two images' matrices differ by more than 0.5 mm (RMS over
/// a sphere of 80 mm).
fn orientation_warning<T>(a: &Img<T>, b: &Img<T>, err: &mut dyn Write) {
    let (Some(m1), Some(m2)) = (world(a), world(b)) else {
        return;
    };
    let inv = |m: [[f64; 4]; 4]| larmorx_core::affine::Affine::from_rows(m).inverse().ok();
    let Some(i2) = inv(m2) else { return };
    let i2 = *i2.rows();
    let mut d = [[0.0f64; 4]; 4];
    for r in 0..4 {
        for c in 0..4 {
            d[r][c] = (0..4).map(|k| m1[r][k] * i2[k][c]).sum::<f64>() - f64::from(r == c);
        }
    }
    let tr: f64 = (0..3)
        .map(|c| (0..3).map(|r| d[r][c] * d[r][c]).sum::<f64>())
        .sum();
    let tt: f64 = (0..3).map(|r| d[r][3] * d[r][3]).sum();
    let rms = ((80.0 * 80.0 / 5.0) * tr + tt).sqrt();
    if rms > 0.5 {
        let _ = write!(
            err,
            "\nWARNING:: Inconsistent orientations for individual images in pipeline!\n          Will use voxel-based orientation which is probably incorrect - *PLEASE CHECK*!\n\n"
        );
    }
}

/// A binary operation with an image (§3.2): the orientation warning, the current image
/// repeated over the operand's volumes, the sizes' rule.
fn binary_image<T: Num>(
    _ctx: &Ctx<'_>,
    st: &mut State<T>,
    b: Binary,
    w: Img<T>,
    notices: &mut Notices,
    err: &mut dyn Write,
) -> Result<(), Exit> {
    orientation_warning(&st.img, &w, err);
    if st.img.dims[3] == 1 && w.dims[3] > 1 {
        let nt = w.dims[3];
        let one = std::mem::take(&mut st.img.data);
        let mut data = Vec::with_capacity(one.len() * nt);
        for _ in 0..nt {
            data.extend_from_slice(&one);
        }
        st.img.data = data;
        st.img.dims[3] = nt;
    }
    let (c, o) = (st.img.dims, w.dims);
    let layout = match b {
        Binary::Add | Binary::Sub | Binary::Mul => {
            let dw = w.last_axis();
            let ok = c[0] == o[0] && st.img.last_axis() >= dw && (0..dw).all(|k| c[k] == o[k]);
            if !ok {
                let verb = match b {
                    Binary::Add => "add",
                    Binary::Sub => "subtract",
                    _ => "multiply",
                };
                let what = format!("Attempted to {verb} images of different sizes");
                let _ = writeln!(err, "Image Exception : #3 :: {what}");
                common::abort(err, "fslmaths", "std::runtime_error", &what);
                return Err(Exit(1));
            }
            Layout::Cyclic
        }
        _ => {
            let ok = c[..3] == o[..3] && (o[3] == 1 || o[3] == c[3]);
            if ok {
                Layout::Cyclic
            } else {
                let verb = if matches!(b, Binary::Div | Binary::Rem) {
                    "divide"
                } else {
                    "compare"
                };
                if notices.fix() {
                    notices.add(
                        &defects::FSL_159,
                        format!(
                            "an image of other sizes is refused (FSL {verb}s by misplaced values \
                             of it)"
                        ),
                    );
                    let _ = writeln!(
                        err,
                        "larmorx: error: fslmaths: Attempted to {verb} images of different sizes \
                         (FSL-159: FSL uses misplaced values of the second image)"
                    );
                    return Err(Exit(1));
                }
                notices.add(
                    &defects::FSL_159,
                    format!(
                        "-{} with an image of other sizes: its values at FSL's positions, as FSL \
                         takes them",
                        match b {
                            Binary::Div => "div",
                            Binary::Rem => "rem",
                            Binary::Max => "max",
                            _ => "min",
                        }
                    ),
                );
                if matches!(b, Binary::Div | Binary::Rem) {
                    Layout::Corner
                } else {
                    Layout::Strided
                }
            }
        }
    };
    ops::with_image(&mut st.img, b, &w, layout, notices).map_err(|s| refuse_stop(err, s))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_args(args: &[&str], vars: &[(&str, &str)]) -> (u8, String, String) {
        let settings = Settings {
            vars: vars
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                .collect(),
            dir: None,
            stdin: None,
        };
        let args: Vec<String> = args.iter().map(|s| (*s).to_owned()).collect();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = main(&args, &settings, &mut out, &mut err);
        (
            code,
            String::from_utf8(out).unwrap(),
            String::from_utf8(err).unwrap(),
        )
    }

    #[test]
    fn the_usage_and_the_command_line_errors() {
        let (code, out, _) = run_args(&[], &[]);
        assert_eq!(code, 1);
        assert!(out.starts_with("\nUsage: fslmaths [-dt <datatype>]"));
        assert!(out.ends_with("demeaned_4D_inputVolume\n\n"));
        let (code, out, _) = run_args(&["-h"], &[]);
        assert_eq!((code, out.len()), (0, USAGE.len()));
        let (code, out, _) = run_args(&["in", "out", "-odt", "bogus"], &[]);
        assert_eq!(code, 1);
        assert_eq!(
            out,
            "Error: Unknown datatype \"bogus\" - Possible datatypes are: char short int float \
             double input\n"
        );
        // FSL-158: -dt without an input reads a null string (the runtime's abort reproduced)
        let (code, _, err) = run_args(&["-dt", "float"], &[(defects::ENV, "reproduce")]);
        assert_eq!(code, 1);
        assert!(
            err.starts_with(
                "terminate called after throwing an instance of 'std::logic_error'\n  what():  \
                 basic_string: construction from null is not valid\n"
            ),
            "{err}"
        );
        let (code, _, err) = run_args(&["-dt", "float"], &[]);
        assert_eq!(code, 1);
        assert!(
            err.contains("larmorx: error: fslmaths: the input reads past the end"),
            "{err}"
        );
        let (code, _, err) = run_args(&["nothere", "out"], &[]);
        assert_eq!(code, 1);
        assert!(
            err.starts_with(
                "Image Exception : #63 :: No image files match: nothere\nterminate called"
            ),
            "{err}"
        );
    }
}
