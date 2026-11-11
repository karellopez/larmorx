// SPDX-License-Identifier: Apache-2.0
//! `larmorx fsl applyxfm4D`: FSL's `applyxfm4D` (`specs/flirt.md` §15.7), every volume of a
//! series resampled onto a reference's grid through one matrix or one per volume, on FLIRT's
//! shared resampling ([`crate::flirt::output::resample`]).

use std::io::Write;

use larmorx_core::defects::{self, Notice, UpstreamDefects};
use larmorx_core::element::DataType;
use larmorx_mri::hmc::volume::Volume;

use super::Settings;
use super::common::{self, TRUNCATED, abort, print_notices};
use crate::bet::image;
use crate::flirt::engine::{self, Ctx, End, Look};
use crate::flirt::mat::{self, Mat4};
use crate::flirt::output::{self, FlirtSpline, Resampling, SincKernel};
use crate::flirt::{Interp, SincWindow, pyramid};

/// The usage text (observed: standard error; the program names itself as it was called).
pub const USAGE: &str = "Usage: applyxfm4D <input volume> <ref volume> <output volume> <transformation matrix file/dir>\n\n\t--interp, -interp <nearestneighbour (or nn), trilinear, spline, sinc (default)>\n\t--singlematrix, -singlematrix (flag option, do not provide an argument)\n\t--fourdigit, -fourdigit (flag option, do not provide an argument)\n\t--userprefix, -userprefix <prefix>\n\n";

/// FSL's string of null (an option's value missing).
const NULL_STRING: &str = "basic_string: construction from null is not valid";

/// The most volumes FSL resamples.
const MAX_VOLUMES: usize = 10000;

struct Opts {
    single: bool,
    four: bool,
    prefix: Option<String>,
    interp: Interp,
    method: String,
    three_d: bool,
    verbose: bool,
}

/// The words after the fourth (§15.7); `Err` with the exit status after FSL's message.
fn parse(words: &[String], err: &mut dyn Write) -> Result<Opts, u8> {
    let mut o = Opts {
        single: false,
        four: false,
        prefix: None,
        interp: Interp::Sinc,
        method: "sinc".to_owned(),
        three_d: false,
        verbose: false,
    };
    let mut i = 0;
    while i < words.len() {
        let w = &words[i];
        let name = w.strip_prefix("--").or_else(|| w.strip_prefix('-'));
        i += 1;
        let mut value = |err: &mut dyn Write| -> Result<String, u8> {
            match words.get(i) {
                Some(v) => {
                    i += 1;
                    Ok(v.clone())
                }
                None => Err(abort(err, "applyxfm4D", "std::logic_error", NULL_STRING)),
            }
        };
        match name {
            Some("singlematrix") => o.single = true,
            Some("fourdigit") => o.four = true,
            Some("userprefix") => o.prefix = Some(value(err)?),
            Some("interp") => {
                let m = value(err)?;
                o.interp = match m.as_str() {
                    "nearestneighbour" | "nn" => Interp::NearestNeighbour,
                    "trilinear" => Interp::Trilinear,
                    "sinc" => Interp::Sinc,
                    "spline" => Interp::Spline,
                    _ => {
                        let _ = writeln!(
                            err,
                            "interpolation method: {m} is not a valid choice. Please choose from nearestneighbour, trilinear, sinc, or spline."
                        );
                        return Err(1);
                    }
                };
                o.method = m;
            }
            Some("3D" | "3d") => o.three_d = true,
            Some("v" | "verbose") => o.verbose = true,
            _ => {}
        }
    }
    Ok(o)
}

/// FSL's image layer's exception for an image it cannot read (observed: `#63`, then the C++
/// runtime's lines).
fn image_error(err: &mut dyn Write, name: &str, e: &image::ImageError) -> u8 {
    let text = e.to_string();
    let missing = text.starts_with("No image files match") || text.contains("No such file");
    let stem = larmorx_mri::hmc::files::strip_image_extension(name);
    let what = if missing {
        format!("No image files match: {stem}")
    } else if text.starts_with("Multiple possible") {
        format!("Multiple possible filenames detected for basename: {stem}")
    } else {
        format!("{name}: {text}")
    };
    let _ = writeln!(err, "Image Exception : #63 :: {what}");
    abort(err, "applyxfm4D", "std::runtime_error", &what)
}

/// A matrix file as FSL reads it here: its message and an empty matrix when it cannot be opened,
/// which stops the run at its product with the volume's 4 × 4 map.
fn matrix(settings: &Settings, name: &str, err: &mut dyn Write) -> Result<Mat4, u8> {
    let rows = match common::read_rows(settings, name, err) {
        Ok(r) => r,
        Err(m) => return Err(abort(err, "applyxfm4D", "std::runtime_error", &m)),
    };
    // the matrix is inverted, then multiplied with the volume's 4 × 4 map (observed)
    let (r, c) = common::size(&rows);
    if r != c {
        return Err(abort(
            err,
            "applyxfm4D",
            "std::logic_error",
            "inv(): given matrix must be square sized",
        ));
    }
    let Some(m) = common::exact4(&rows) else {
        return Err(abort(
            err,
            "applyxfm4D",
            "std::logic_error",
            &format!("matrix multiplication: incompatible matrix dimensions: {r}x{c} and 4x4"),
        ));
    };
    if mat::inverse(&m).is_none() {
        return Err(abort(
            err,
            "applyxfm4D",
            "std::runtime_error",
            mat::SINGULAR,
        ));
    }
    Ok(m)
}

/// `applyxfm4D` with FSL's arguments; returns the exit status.
pub fn main(args: &[String], settings: &Settings, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let mode = match settings.defects() {
        Ok(m) => m,
        Err(e) => {
            let _ = writeln!(err, "larmorx fsl applyxfm4D: {e}");
            return 2;
        }
    };
    if args.len() < 4 {
        let _ = write!(err, "{USAGE}");
        return 255;
    }
    let o = match parse(&args[4..], err) {
        Ok(o) => o,
        Err(c) => return c,
    };
    if o.verbose {
        let k = match o.interp {
            Interp::NearestNeighbour => 0,
            Interp::Trilinear => 1,
            Interp::Sinc => 2,
            Interp::Spline => 4,
        };
        let _ = writeln!(
            out,
            "using interpolation method (enum, string): {k}, {}",
            o.method
        );
    }
    let ctx = Ctx {
        settings,
        fsl09: &engine::FSL_09_APPLYXFM4D,
        mode,
        n_threads: settings.threads(),
        libm: std::sync::Arc::new(larmorx_core::math::Libm::quiet(mode)),
    };
    let result = larmorx_core::parallel::with_threads(ctx.n_threads, || {
        let (mut o_buf, mut e_buf, mut notices) = (Vec::new(), Vec::new(), Vec::new());
        let code = run(&args[..4], &o, &ctx, &mut o_buf, &mut e_buf, &mut notices);
        (code, o_buf, e_buf, notices)
    });
    match result {
        Ok((code, o_buf, e_buf, notices)) => {
            let _ = out.write_all(&o_buf);
            let _ = err.write_all(&e_buf);
            print_notices(err, &notices);
            code
        }
        Err(e) => {
            let _ = writeln!(err, "larmorx fsl applyxfm4D: {e}");
            1
        }
    }
}

/// One volume of a series as the resampler reads it, with its doubles for a double image.
struct Source {
    vols: Vec<Volume>,
    doubles: Option<Vec<f64>>,
    input: image::Input,
}

/// Reads `name`: every volume when `all`; the input (`input`) names only the missing files when
/// it cannot be read, the reference the volume too (observed).
fn read(
    ctx: &Ctx<'_>,
    name: &str,
    all: bool,
    input: bool,
    notices: &mut Vec<Notice>,
    err: &mut dyn Write,
) -> Result<Source, u8> {
    let failed = |e: &image::ImageError, err: &mut dyn Write| {
        if input {
            image_error(err, name, e)
        } else {
            let (lines, what) = common::image_exception(name, e);
            let _ = write!(err, "{lines}");
            abort(err, "applyxfm4D", "std::runtime_error", &what)
        }
    };
    let files = match super::bet2::resolve_image(ctx.settings, name, ctx.mode, notices) {
        Ok(f) => f,
        Err(e) => return Err(failed(&e, err)),
    };
    let r = if all {
        image::read(&files, ctx.n_threads)
    } else {
        image::read_first(&files, ctx.n_threads).map(|i| (vec![i.vol.clone()], i))
    };
    let (vols, mut image) = match r {
        Ok(x) => x,
        Err(e) => return Err(failed(&e, err)),
    };
    if !all && image.volumes > 1 {
        let _ = writeln!(err, "{TRUNCATED}");
    }
    // a double image's values; an int32 image's exactly when some lie beyond single precision's
    // integers (2^24): FSL holds them as stored, so its background value (the padding) is
    // written exactly (observed, random testing's 20261103-03732 of 2026-11-05: an image of
    // 2147483647, whose padding FSL writes as 2147483647 while its interpolated voxels wrap
    // through single precision, FSL-09)
    let doubles = match image.header.data_type() {
        Ok(DataType::F64) => image::read_f64_series(&files, ctx.n_threads),
        Ok(DataType::I32)
            if vols
                .iter()
                .any(|v| v.data.iter().any(|x| x.abs() > 16_777_216.0)) =>
        {
            image::read_i32_series(&files, ctx.n_threads)
        }
        _ => None,
    };
    image.vol.data = Vec::new();
    let vols = vols
        .into_iter()
        .map(|v| Volume::new(v.shape, v.voxel.map(|d| d as f32), v.data))
        .collect();
    Ok(Source {
        vols,
        doubles,
        input: image,
    })
}

/// `v` converted to the integer type `t` as x86-64 converts a double to it (FSL-79 reproduced:
/// each spline pass stored as `(T)(c + 0.5)` and each value as `(T)v`): truncated toward zero
/// through a 32-bit integer (NaN and values beyond it give its smallest value), then wrapped to
/// the type's bits (observed, 2026-11-02: a coefficient of 160 in a char image reads -96).
fn x86_convert(v: f64, t: DataType) -> f64 {
    let i = if v.is_nan() || !(-2_147_483_648.0..2_147_483_648.0).contains(&v) {
        i32::MIN
    } else {
        v as i32
    };
    match t {
        DataType::U8 => f64::from(i as u8),
        DataType::I8 => f64::from(i as i8),
        DataType::I16 => f64::from(i as i16),
        _ => f64::from(i),
    }
}

/// A uint8 volume's values as FSL's applyxfm4D reads them into its char images (FSL-111, x86-64's
/// signed char: 128 to 255 read as `v - 256`) when `signed`, or as stored (0 to 255) otherwise.
fn char_reading(vol: &Volume, signed: bool) -> Volume {
    let data = vol
        .data
        .iter()
        .map(|&v| match (signed, v >= 128.0, v < 0.0) {
            (true, true, _) => v - 256.0,
            (false, _, true) => v + 256.0,
            _ => v,
        })
        .collect();
    Volume::new(vol.shape, vol.voxel_size, data)
}

fn run(
    names: &[String],
    o: &Opts,
    ctx: &Ctx<'_>,
    out: &mut dyn Write,
    err: &mut dyn Write,
    notices: &mut Vec<Notice>,
) -> u8 {
    let mode = ctx.mode;
    let (inname, refname, outname, xfm) = (&names[0], &names[1], &names[2], &names[3]);
    let src = match read(ctx, inname, !o.three_d, true, notices, err) {
        Ok(s) => s,
        Err(c) => return c,
    };
    let reference = match read(ctx, refname, false, false, notices, err) {
        Ok(s) => s,
        Err(c) => return c,
    };
    let refv = &reference.vols[0];
    let (stored, scaled) = engine::stored_type(&src.input.header);
    let t = engine::grouped_type(stored, scaled, mode);
    let integer = larmorx_mri::hmc::image::integer_range(t).is_some();
    // FSL-111: FSL reads a uint8 image into its char images, signed on x86-64, so 128 to 255
    // read as v - 256 (observed, 2026-11-02); larmorx reads them as stored by default
    let unsigned_char = stored == DataType::U8 && !scaled;
    let mut src = src;
    if unsigned_char && mode.reproduce() {
        for v in &mut src.vols {
            *v = char_reading(v, true);
        }
    }
    // the type FSL-79 keeps the coefficients and values in: char (signed) for the char group
    // read signed, the image's own type otherwise
    let store_type = |signed: bool| match t {
        DataType::U8 if signed || stored == DataType::I8 => DataType::I8,
        other => other,
    };
    let double = t == DataType::F64 && src.doubles.is_some();
    // an int32 image's padding written exactly in its type (its interpolated voxels as usual)
    let exact_pad = t == DataType::I32 && src.doubles.is_some();
    let mut nv = src.vols.len();
    if !o.three_d && nv > MAX_VOLUMES {
        let _ = writeln!(
            err,
            "WARNING:: More than 10000 volumes - only doing first 10000"
        );
        nv = MAX_VOLUMES;
    }
    // the sinc kernel: blackman of width 7, or with -3D the rectangular window of width 9 it asks
    // for (FSL-80: FSL uses the first)
    let fsl80 = o.three_d && o.interp == Interp::Sinc;
    let asked = SincKernel::new(9.0, SincWindow::Rectangular, &ctx.libm);
    let first = SincKernel::new(7.0, SincWindow::Blackman, &ctx.libm);
    let (kernel, other_kernel) = if fsl80 && mode.fix() {
        (&asked, &first)
    } else {
        (&first, &asked)
    };
    // each defect is named only where it changes a value (D13): the other mode's volume is
    // computed until one differs
    let (mut fsl80_changes, mut fsl79_changes) = (false, false);
    let name80 = |notices: &mut Vec<Notice>| {
        let detail = match mode {
            UpstreamDefects::Fix => {
                "applyxfm4D -3D resamples with the rectangular sinc window of full width 9 it asks for (FSL uses the blackman window of width 7 it defined first)"
            }
            UpstreamDefects::Reproduce => {
                "applyxfm4D -3D resamples with the blackman sinc window of width 7, as FSL does (it asks for the rectangular window of width 9)"
            }
        };
        defects::push_once(
            notices,
            Notice::new(&defects::FSL_80, mode, detail.to_owned()),
        );
    };
    let fsl79 = integer && o.interp == Interp::Spline;
    let name79 = |notices: &mut Vec<Notice>| {
        let detail = match mode {
            UpstreamDefects::Fix => {
                "applyxfm4D's spline of an integer image: its coefficients and values in single precision, as for a float image (FSL keeps them in the image's type)"
            }
            UpstreamDefects::Reproduce => {
                "applyxfm4D's spline of an integer image: its coefficients and each value in the image's type, as FSL does"
            }
        };
        defects::push_once(
            notices,
            Notice::new(&defects::FSL_79, mode, detail.to_owned()),
        );
    };
    let single = o.single || o.three_d;
    let one = if single {
        match matrix(ctx.settings, xfm, err) {
            Ok(m) => Some(m),
            Err(c) => return c,
        }
    } else {
        None
    };
    let per_volume = refv.len();
    let mut values = Vec::with_capacity(per_volume * nv);
    let mut doubles: Option<Vec<f64>> =
        (double || exact_pad).then(|| Vec::with_capacity(per_volume * nv));
    let mut first_mat: Option<Mat4> = None;
    let mut fsl111_changes = false;
    let name111 = |notices: &mut Vec<Notice>| {
        let detail = match mode {
            UpstreamDefects::Fix => {
                "applyxfm4D reads the uint8 image's values 128 to 255 as stored (FSL reads them into a signed char image, as v - 256)"
            }
            UpstreamDefects::Reproduce => {
                "applyxfm4D reads the uint8 image's values 128 to 255 as v - 256, into a signed char image, as FSL does on x86-64"
            }
        };
        defects::push_once(
            notices,
            Notice::new(&defects::FSL_111, mode, detail.to_owned()),
        );
    };
    for m in 0..nv {
        let mat = match one {
            Some(x) => x,
            None => {
                let digits = format!("{m:04}");
                let name = match (&o.prefix, o.four) {
                    (Some(p), _) => format!("{xfm}/{p}{digits}"),
                    (None, true) => format!("{xfm}/MAT_{digits}"),
                    (None, false) => format!("{xfm}/MAT_0{digits}"),
                };
                let _ = writeln!(out, "{name}");
                match matrix(ctx.settings, &name, err) {
                    Ok(x) => x,
                    Err(c) => return c,
                }
            }
        };
        first_mat.get_or_insert(mat);
        let vol = &src.vols[m];
        let pad = match engine::pad_value(vol, ctx, notices) {
            Ok(p) => p,
            Err(End::Refuse(msg)) => return common::refuse(err, "applyxfm4D", &msg),
            Err(_) => return 1,
        };
        // one volume (`vol`, its background `pad`, read signed or not) resampled with FSL-79
        // reproduced or not, with `kernel`
        let resampled = |vol: &Volume,
                         pad: f32,
                         signed: bool,
                         reproduce79: bool,
                         kernel: &SincKernel|
         -> Option<Vec<f32>> {
            let st = store_type(signed);
            // §9.3's spline (§15.7): a float image's passes in single precision, a double
            // image's values read and filtered in double, an integer image's passes in its type
            // with FSL-79 reproduced
            let spline = (o.interp == Interp::Spline).then(|| {
                if fsl79 && reproduce79 {
                    let values = vol.data.iter().map(|&v| f64::from(v)).collect();
                    FlirtSpline::with_store(vol.shape, values, &|c| x86_convert(c + 0.5, st))
                } else if let (true, Some(all)) = (double, src.doubles.as_ref()) {
                    let part = all[m * vol.len()..(m + 1) * vol.len()].to_vec();
                    FlirtSpline::with_store(vol.shape, part, &|c| c)
                } else {
                    FlirtSpline::new(vol)
                }
            });
            let r = Resampling {
                interp: o.interp,
                sinc: Some(kernel),
                pad,
                padding: 0.0,
                spline: spline.as_ref(),
                constant: false,
            };
            let mut v = output::resample(vol, &mat, refv.shape, refv.voxel_size, &r)?;
            if fsl79 && reproduce79 {
                for x in &mut v {
                    *x = x86_convert(f64::from(*x), st) as f32;
                }
            }
            Some(v)
        };
        let same = |a: &[f32], b: &[f32]| a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits());
        let signed = unsigned_char && mode.reproduce();
        let Some(v) = resampled(vol, pad, signed, mode.reproduce(), kernel) else {
            return abort(err, "applyxfm4D", "std::runtime_error", mat::SINGULAR);
        };
        // (the matrix was checked invertible when it was read)
        if fsl80 && !fsl80_changes {
            fsl80_changes = resampled(vol, pad, signed, mode.reproduce(), other_kernel)
                .is_some_and(|w| !same(&v, &w));
        }
        if fsl79 && !fsl79_changes {
            fsl79_changes = resampled(vol, pad, signed, !mode.reproduce(), kernel)
                .is_some_and(|w| !same(&v, &w));
        }
        // FSL-111 named where the other reading changes a value written (the output's own
        // conversion wraps a signed char back to the stored byte)
        if unsigned_char && !fsl111_changes && vol.data.iter().any(|&x| x >= 128.0 || x < 0.0) {
            let other = char_reading(vol, !signed);
            let mut scratch = Vec::new();
            if let Ok(p) = engine::pad_value(&other, ctx, &mut scratch)
                && let Some(w) = resampled(&other, p, !signed, mode.reproduce(), kernel)
            {
                let byte = |x: f32| x86_convert(f64::from(x), DataType::U8);
                let rounded = |x: f32| f64::from(x).round_ties_even().clamp(0.0, 255.0);
                let written = |x: f32| if mode.fix() { rounded(x) } else { byte(x) };
                fsl111_changes = v
                    .iter()
                    .zip(&w)
                    .any(|(&a, &b)| written(a).to_bits() != written(b).to_bits());
            }
        }
        if let (Some(d), Some(all)) = (doubles.as_mut(), src.doubles.as_ref()) {
            let part = &all[m * vol.len()..(m + 1) * vol.len()];
            let bg = pyramid::background_f64(vol.shape, part, mode).map_or(f64::from(pad), |b| b.0);
            let mask = output::padded(
                vol.shape,
                vol.voxel_size,
                &mat,
                refv.shape,
                refv.voxel_size,
                0.0,
            )
            .unwrap_or_default();
            d.extend(
                v.iter()
                    .zip(mask.iter().chain(std::iter::repeat(&false)))
                    .map(|(&x, &p)| if p { bg } else { f64::from(x) }),
            );
        }
        values.extend(v);
    }
    if fsl80_changes {
        name80(notices);
    }
    if fsl79_changes {
        name79(notices);
    }
    if fsl111_changes {
        name111(notices);
    }
    // a reference without forms: the input's form through the first volume's matrix
    // (observed); else the reference's unset form completed from the set one (observed)
    let formless = first_mat
        .as_ref()
        .and_then(|m| engine::formless_forms(&reference.input, &src.input, m));
    let quaternion_final = formless.is_some() || engine::qform_completed(&reference.input);
    let completed = formless.unwrap_or_else(|| engine::completed_forms(&reference.input));
    let look = Look {
        reference: &completed,
        shape: refv.shape,
        volumes: nv,
        tr: Some(src.input.header.pixdim[4]),
        cal: Some((0.0, 0.0)),
        descrip: "larmorx fsl applyxfm4D",
        // the sform as stored, but recomputed (its zeros positive) for a reference stored
        // neurologically, which FSL flips (observed)
        positive_zeros: reference.input.flipped,
        quaternion_final,
    };
    match engine::write_values(ctx, outname, &look, values, doubles, t, err, notices) {
        Ok(()) => 0,
        Err(End::Exit(c)) => c,
        Err(End::Refuse(m) | End::Error(m)) => {
            let _ = writeln!(err, "{m}");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(words: &[&str]) -> (u8, String, String) {
        let args: Vec<String> = words.iter().map(|s| (*s).to_owned()).collect();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let c = main(&args, &Settings::default(), &mut out, &mut err);
        (
            c,
            String::from_utf8(out).unwrap(),
            String::from_utf8(err).unwrap(),
        )
    }

    #[test]
    fn messages() {
        assert_eq!(run(&[]), (255, String::new(), USAGE.to_owned()));
        let (c, _, err) = run(&["a", "b", "c", "d", "-interp", "cubic"]);
        assert_eq!(
            (c, err.as_str()),
            (
                1,
                "interpolation method: cubic is not a valid choice. Please choose from nearestneighbour, trilinear, sinc, or spline.\n"
            )
        );
        let (c, _, err) = run(&["a", "b", "c", "d", "-userprefix"]);
        assert_eq!(c, 1);
        assert!(err.contains(NULL_STRING), "{err}");
        let (c, _, err) = run(&["none.nii.gz", "b", "c", "d"]);
        assert_eq!(c, 1);
        assert!(
            err.starts_with("Image Exception : #63 :: No image files match: none\n"),
            "{err}"
        );
    }
}
