// SPDX-License-Identifier: Apache-2.0
//! `antsAffineInitializer` with its original arguments (ANTs v2.6.5,
//! `Examples/antsAffineInitializer.cxx`):
//!
//! ```text
//! antsAffineInitializer ImageDimension <Image1.ext> <Image2.ext> TransformOutput.mat Optional-SearchFactor Optional-Radian-Fraction Optional-bool-UsePrincipalAxes Optional-uint-UseLocalSearch Optional-Image1Mask
//! ```
//!
//! As in ANTs:
//! - fewer than two arguments print the usage on standard error and exit 0;
//! - the dimension is `std::stoi` of the first argument; 2, 3 and 4 run as such, any other
//!   number as 2 (ANTS-56; corrected by default: an error);
//! - the search factor (degrees) and the radian fraction are read with `atof` (the fraction
//!   clamped to `[0.01, 1]`), the principal-axes flag with `std::stoi`; the local search's
//!   iterations are skipped unread (ANTs always runs 20, ANTS-54);
//! - a missing image is reported and then aborts ANTs (` zero image1 error `, then the moments
//!   calculator's exception), as does an image whose values sum to 0;
//! - the initial alignment is written first, then (2D and 3D) replaced by the search's best
//!   transform; in 4D only the initial alignment is computed.
//!
//! Where ANTs aborts (an exception reaches `std::terminate`), larmorx prints what ANTs prints
//! and exits 1; where it crashes or never finishes, larmorx refuses with a message (exit 1).

use std::io::Write;
use std::path::Path;
use std::sync::Arc;

use larmorx_core::defects::{self, EXIT_BAD_MODE, Notices, UpstreamDefects};
use larmorx_io::itk_transform::{
    ItkTransformParts, Parameters, TransformFileError, write_itk_transform_parts,
};
use larmorx_registration::mask::Mask;
use larmorx_registration::transform::LinearXfm;
use larmorx_registration::{Image, ItkException};

use super::ai::terminate_text;
use super::cstd::{atof, stoi};
use super::registration::to_image;
use crate::affine_initializer::{AffineInitializerSettings, image_moments, initial, search};
use crate::ai::AiError;
use crate::image::{FileStore, ImageStore, read_with};
use crate::registration::helper::Log;

/// What ends a run early: the exit status (the messages are already printed).
type Exit = u8;

/// The usage text (`antsAffineInitializer.cxx:592-608`), on standard error.
pub const USAGE: &str = concat!(
    "\nUsage: antsAffineInitializer ImageDimension <Image1.ext> <Image2.ext> TransformOutput.mat ",
    "Optional-SearchFactor Optional-Radian-Fraction Optional-bool-UsePrincipalAxes ",
    "Optional-uint-UseLocalSearch Optional-Image1Mask \n",
    " Optional-SearchFactor is in degrees --- e.g. 10 = search in 10 degree increments .\n",
    " Radian-Fraction should be between 0 and 1 --- will search this arc +/- around principal axis.\n",
    " Optional-bool-UsePrincipalAxes determines whether the rotation is searched around an initial ",
    "principal axis alignment.  Default = false. \n",
    " Optional-uint-UseLocalSearch determines if a local optimization is run at each search point ",
    "for the set number of iterations. Default = 20.\n",
);

/// Runs `antsAffineInitializer` on files, in the D13 mode of `LARMORX_UPSTREAM_DEFECTS`;
/// returns the exit status.
pub fn main(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    match UpstreamDefects::from_env() {
        Ok(mode) => run(args, &mut FileStore, super::itk_threads(), mode, out, err),
        Err(e) => {
            let _ = writeln!(err, "larmorx: {e}");
            EXIT_BAD_MODE
        }
    }
}

/// Runs `antsAffineInitializer` with images from `store`, ANTs' thread count `n_threads`
/// (larmorx's result does not depend on it) and the D13 mode `mode`.
pub fn run(
    args: &[String],
    store: &mut dyn ImageStore,
    n_threads: usize,
    mode: UpstreamDefects,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let mut notices = Notices::new(mode);
    let mut warnings = Vec::new();
    let code = {
        let mut log = Log::new(out, err, false);
        let code = match affine_initializer(
            args,
            store,
            n_threads,
            &mut log,
            &mut notices,
            &mut warnings,
        ) {
            Ok(code) | Err(code) => code,
        };
        for w in warnings.drain(..) {
            log.cerr(&w);
        }
        code
    };
    notices.print(err);
    code
}

/// An exception `antsAffineInitializer` does not catch: ANTs aborts; larmorx prints what
/// libstdc++ prints and exits 1.
fn abort(log: &mut Log<'_>, text: &str) -> Exit {
    log.cerr(text);
    log.cerr("larmorx: antsAffineInitializer: ANTs aborts here (an uncaught exception); larmorx stops with exit 1\n");
    1
}

fn uncaught(log: &mut Log<'_>, e: &ItkException) -> Exit {
    abort(log, &terminate_text(e))
}

/// A case ANTs runs with undefined behaviour or never finishes.
fn refuse(log: &mut Log<'_>, message: &str) -> Exit {
    log.cerr(&format!("larmorx: antsAffineInitializer: {message}\n"));
    1
}

fn failure(log: &mut Log<'_>, e: AiError) -> Exit {
    match e {
        AiError::Uncaught(e) => uncaught(log, &e),
        AiError::Refused(m) => refuse(log, &m),
        AiError::Exit(c) => c,
        AiError::Terminated(text) => abort(log, &text),
    }
}

/// `std::stoi`'s exception for `s` (no digits: `invalid_argument`, else `out_of_range`).
fn stoi_abort(log: &mut Log<'_>, s: &str) -> Exit {
    let t = s.trim_start_matches([' ', '\t', '\n', '\x0b', '\x0c', '\r']);
    let t = t.strip_prefix(['+', '-']).unwrap_or(t);
    let kind = if t.starts_with(|c: char| c.is_ascii_digit()) {
        "std::out_of_range"
    } else {
        "std::invalid_argument"
    };
    abort(
        log,
        &format!("terminate called after throwing an instance of '{kind}'\n  what():  stoi\n"),
    )
}

/// `antsAffineInitializer`'s `main` (`antsAffineInitializer.cxx:568-626`).
fn affine_initializer(
    args: &[String],
    store: &mut dyn ImageStore,
    n_threads: usize,
    log: &mut Log<'_>,
    notices: &mut Notices,
    warnings: &mut Vec<String>,
) -> Result<Exit, Exit> {
    // argc counts the program's name.
    if args.len() + 1 < 3 {
        log.cerr(USAGE);
        return Ok(0);
    }
    let Some(dimension) = stoi(&args[0]) else {
        return Err(stoi_abort(log, &args[0]));
    };
    let dimension = match dimension {
        2..=4 => dimension,
        d => {
            if notices.fix() {
                notices.add(
                    &defects::ANTS_56,
                    format!("the dimension {d} is not 2, 3 or 4: an error (ANTs runs it as 2D)"),
                );
                log.cerr(&format!(
                    "larmorx: antsAffineInitializer: unsupported dimension {d} (2, 3 or 4)\n"
                ));
                return Err(1);
            }
            notices.add(
                &defects::ANTS_56,
                format!("the dimension {d} is not 2, 3 or 4: run as 2D, as in ANTs"),
            );
            2
        }
    };
    let mut ctx = Context {
        args,
        store,
        n_threads,
        notices,
        warnings,
    };
    match dimension {
        2 => imp::<2>(&mut ctx, log),
        3 => imp::<3>(&mut ctx, log),
        _ => imp::<4>(&mut ctx, log),
    }
}

/// What the dimension-specific part reads from.
struct Context<'a, 'n> {
    args: &'a [String],
    store: &'a mut dyn ImageStore,
    n_threads: usize,
    notices: &'n mut Notices,
    warnings: &'n mut Vec<String>,
}

/// `ReadImage<itk::Image<float, D>>`: the image, or ANTs' message and `None`.
fn read_float<const D: usize>(
    ctx: &mut Context<'_, '_>,
    log: &mut Log<'_>,
    name: &str,
) -> Option<Image<f32, D>> {
    match read_with::<f32, _>(ctx.store, name, D, ctx.n_threads, ctx.notices) {
        Ok(img) => match to_image(img) {
            Ok(i) => Some(i),
            Err(e) => {
                log.cerr(&format!(
                    "larmorx: antsAffineInitializer: {name}: {}\n",
                    e.description
                ));
                None
            }
        },
        Err(e) => {
            if let Some(m) = e.ants_message() {
                log.cerr(&format!("{m}\n"));
            }
            None
        }
    }
}

/// `ReadImage<ImageMaskSpatialObject<D>::ImageType>` (`unsigned char`).
fn read_mask<const D: usize>(
    ctx: &mut Context<'_, '_>,
    log: &mut Log<'_>,
    name: &str,
) -> Option<Mask<D>> {
    match read_with::<u8, _>(ctx.store, name, D, ctx.n_threads, ctx.notices) {
        Ok(img) => to_image(img).ok().map(Mask::new),
        Err(e) => {
            if let Some(m) = e.ants_message() {
                log.cerr(&format!("{m}\n"));
            }
            None
        }
    }
}

/// `std::string(argv[i])`: a missing argument is a null pointer, and libstdc++ throws.
fn argument<'a>(ctx: &Context<'a, '_>, log: &mut Log<'_>, i: usize) -> Result<&'a str, Exit> {
    match ctx.args.get(i) {
        Some(a) => Ok(a.as_str()),
        None => Err(abort(
            log,
            "terminate called after throwing an instance of 'std::logic_error'\n  what():  basic_string: construction from null is not valid\n",
        )),
    }
}

fn flush(ctx: &mut Context<'_, '_>, log: &mut Log<'_>) {
    for w in ctx.warnings.drain(..) {
        log.cerr(&w);
    }
}

/// `antsAffineInitializerImp<D>` (`antsAffineInitializer.cxx:200-565`).
fn imp<const D: usize>(ctx: &mut Context<'_, '_>, log: &mut Log<'_>) -> Result<Exit, Exit> {
    // argv[k] is args[k - 1].
    let fixed_name = argument(ctx, log, 1)?;
    let moving_name = argument(ctx, log, 2)?;
    let output = argument(ctx, log, 3)?;
    let mut s = AffineInitializerSettings::default();
    let mut k = 4;
    if let Some(a) = ctx.args.get(k) {
        s.search_factor = atof(a);
        k += 1;
    }
    if let Some(a) = ctx.args.get(k) {
        s.radian_fraction = atof(a);
        k += 1;
    }
    if let Some(a) = ctx.args.get(k) {
        match stoi(a) {
            Some(v) => s.use_principal_axes = v != 0,
            None => return Err(stoi_abort(log, a)),
        }
        k += 1;
    }
    if ctx.args.get(k).is_some() {
        // ANTS-54 (questionable, reproduced): the local search's iterations are not read;
        // every start point runs LOCAL_SEARCH_ITERATIONS.
        k += 1;
    }
    let fixed = read_float::<D>(ctx, log, fixed_name);
    let moving = read_float::<D>(ctx, log, moving_name);
    let mut mask = None;
    if let Some(a) = ctx.args.get(k) {
        mask = read_mask::<D>(ctx, log, a);
    }
    let fixed64 = fixed.as_ref().map(widened);
    let moving64 = moving.as_ref().map(widened);
    let (mf, mm) =
        image_moments(fixed64.as_ref(), moving64.as_ref(), log).map_err(|e| failure(log, e))?;
    let init = initial(&s, &mf, &mm, log).map_err(|e| failure(log, e))?;
    write_output(ctx, log, output, &init.transform)?;
    if D > 3 {
        return Ok(0);
    }
    let (Some(fixed64), Some(moving64), Some(moving32)) = (fixed64, moving64, moving) else {
        return Err(refuse(log, "an image is missing"));
    };
    let best = search(
        &s,
        &init,
        &Arc::new(fixed64),
        &Arc::new(moving64),
        &moving32,
        mask,
        ctx.notices,
        ctx.warnings,
        ctx.n_threads,
    );
    flush(ctx, log);
    let best = best.map_err(|e| failure(log, e))?;
    write_output(ctx, log, output, &best)?;
    Ok(0)
}

/// The float image's values as doubles (exact).
fn widened<const D: usize>(img: &Image<f32, D>) -> Image<f64, D> {
    img.with_data(img.data.iter().map(|&v| f64::from(v)).collect())
}

/// `TransformFileWriter::Update()` of `t` with `SetUseCompression(true)`; ITK's exceptions
/// are not caught (ANTs aborts).
fn write_output<const D: usize>(
    ctx: &mut Context<'_, '_>,
    log: &mut Log<'_>,
    name: &str,
    t: &LinearXfm<f64, D>,
) -> Result<(), Exit> {
    let path = Path::new(name);
    let lower = name.to_ascii_lowercase();
    let kind = if larmorx_io::itk_transform::is_hdf5_name(path) {
        "HDF5"
    } else if lower.ends_with(".mat") {
        "Matlab"
    } else if lower.ends_with(".txt") || lower.ends_with(".tfm") {
        "Txt"
    } else {
        return Err(uncaught(log, &super::ai::unknown_transform_type(name)));
    };
    let parts = vec![ItkTransformParts {
        name: format!("{}_double_{D}_{D}", t.kind.class()),
        parameters: Parameters::F64(t.parameters()),
        fixed_parameters: t.fixed_parameters(),
    }];
    let options = larmorx_io::itk_hdf5::WriteOptions {
        compression: true,
        ..Default::default()
    };
    write_itk_transform_parts(path, &parts, &options, ctx.notices).map_err(|e| match e {
        TransformFileError::Io { .. } if kind != "HDF5" => {
            uncaught(log, &super::ai::open_failure(kind, name))
        }
        e => {
            match e.ants_report() {
                Some(r) => log.cerr(&r),
                None => log.cerr(&format!("{e}\n")),
            }
            refuse(
                log,
                "ITK's transform writer throws and antsAffineInitializer does not catch it (ANTs aborts)",
            )
        }
    })
}
