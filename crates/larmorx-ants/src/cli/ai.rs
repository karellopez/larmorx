// SPDX-License-Identifier: Apache-2.0
//! `antsAI` with its original arguments (ANTs v2.6.5, `Examples/antsAI.cxx`), through ANTs'
//! command-line parser: the options read, converted and checked in ANTs' order, with its
//! messages on its streams and its exit statuses; the images read where ANTs reads them; then
//! the initial alignment and the search ([`crate::ai`]) and the output transform.
//!
//! As in ANTs:
//! - most errors are explained only with `--verbose 1`; larmorx always explains them
//!   (ANTS-45), on ANTs' stream;
//! - a value the parser cannot convert (`-c 1o`), an ITK exception `antsAI` does not catch
//!   (a zero image's moments, a file it cannot write), or a missing image (ANTs then
//!   dereferences a null pointer) aborts ANTs; larmorx prints what ANTs' exception says (or
//!   what ANTs would do) and exits 1;
//! - the option values are read newest first (`GetFunction(0)` is the last one given).

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

use super::menu::{MenuOption, print_menu};
use super::parser::{self, OptionValue, ParseFailure, Parsed};
use super::registration::{Conv, param, to_image};
use crate::ai::{
    AiError, AiMetric, AiSampling, AiSettings, AiTransform, SearchOutcome, initial, search,
    without_search,
};
use crate::image::{FileStore, ImageStore, read_with};
use crate::registration::helper::Log;

/// What ends a run early: the exit status (the messages are already printed).
type Exit = u8;

/// The text libstdc++ prints when an ITK exception reaches `std::terminate`
/// (`terminate called after throwing ...`, `what()`: the file, the line and the
/// description); object addresses are `0x0`.
pub fn terminate_text(e: &ItkException) -> String {
    let description = if e.class.is_empty() {
        e.description.clone()
    } else {
        format!("ITK ERROR: {}(0x0): {}", e.class, e.description)
    };
    format!(
        "terminate called after throwing an instance of 'itk::ExceptionObject'\n  what():  {}:{}:\n{}\n",
        e.file, e.line, description
    )
}

/// An ITK exception `antsAI` does not catch: ANTs aborts; larmorx prints what ANTs prints
/// and exits 1.
fn uncaught(log: &mut Log<'_>, e: &ItkException) -> Exit {
    log.cerr(&terminate_text(e));
    log.cerr(
        "larmorx: antsAI: ANTs aborts here (an uncaught exception); larmorx stops with exit 1\n",
    );
    1
}

/// A case ANTs runs with undefined behaviour or never finishes.
fn refuse(log: &mut Log<'_>, message: &str) -> Exit {
    log.cerr(&format!("larmorx: antsAI: {message}\n"));
    1
}

fn ai_failure(log: &mut Log<'_>, e: AiError) -> Exit {
    match e {
        AiError::Uncaught(e) => uncaught(log, &e),
        AiError::Refused(m) => refuse(log, &m),
        AiError::Exit(c) => c,
        AiError::Terminated(text) => {
            log.cerr(&text);
            log.cerr(
                "larmorx: antsAI: ANTs aborts here (an uncaught exception); larmorx stops with exit 1\n",
            );
            1
        }
    }
}

const SPECS: &[parser::OptionSpec] = &specs();

const fn specs() -> [parser::OptionSpec; OPTIONS.len()] {
    let mut out = [parser::OptionSpec {
        long: "",
        short: None,
    }; OPTIONS.len()];
    let mut i = 0;
    while i < OPTIONS.len() {
        out[i] = OPTIONS[i].spec();
        i += 1;
    }
    out
}

/// Runs `antsAI` on files, in the D13 mode of `LARMORX_UPSTREAM_DEFECTS`; returns the exit
/// status.
pub fn main(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    match UpstreamDefects::from_env() {
        Ok(mode) => run(
            args,
            &mut FileStore,
            super::itk_threads(),
            mode,
            std::env::var("ANTS_RANDOM_SEED").ok(),
            out,
            err,
        ),
        Err(e) => {
            let _ = writeln!(err, "larmorx: {e}");
            EXIT_BAD_MODE
        }
    }
}

/// Runs `antsAI` with images from `store`, ANTs' thread count `n_threads` (larmorx's result
/// does not depend on it), the D13 mode `mode` and the value of `ANTS_RANDOM_SEED`.
#[allow(clippy::too_many_arguments)]
pub fn run(
    args: &[String],
    store: &mut dyn ImageStore,
    n_threads: usize,
    mode: UpstreamDefects,
    env_seed: Option<String>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let mut notices = Notices::new(mode);
    let mut warnings = Vec::new();
    let code = {
        let mut log = Log::new(out, err, false);
        let code = match antsai(
            args,
            store,
            n_threads,
            &env_seed,
            &mut log,
            &mut notices,
            &mut warnings,
        ) {
            Ok(code) | Err(code) => code,
        };
        for w in &warnings {
            log.cerr(w);
        }
        code
    };
    notices.print(err);
    code
}

/// [`run`] for in-memory images without `ANTS_RANDOM_SEED` (callers pass `--random-seed`):
/// the Python wrapper's entry point.
pub fn run_store(
    args: &[String],
    store: &mut dyn ImageStore,
    n_threads: usize,
    mode: UpstreamDefects,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    run(args, store, n_threads, mode, None, out, err)
}

/// What the dimension-specific part reads from.
struct Context<'a, 'n> {
    p: &'a Parsed,
    store: &'a mut dyn ImageStore,
    n_threads: usize,
    env_seed: &'a Option<String>,
    notices: &'n mut Notices,
    warnings: &'n mut Vec<String>,
}

/// `antsAI`'s `main` (`antsAI.cxx:1767-1891`).
fn antsai(
    args: &[String],
    store: &mut dyn ImageStore,
    n_threads: usize,
    env_seed: &Option<String>,
    log: &mut Log<'_>,
    notices: &mut Notices,
    warnings: &mut Vec<String>,
) -> Result<Exit, Exit> {
    let p = match parser::parse_ants(args, SPECS) {
        Ok(p) => p,
        Err(ParseFailure::Regroup(message)) => {
            let line = if message.contains("leftDelimiterPosition") {
                238
            } else if message.contains("rightDelimiterPosition") {
                246
            } else {
                276
            };
            let e = ItkException::new(
                "Utilities/antsCommandLineParser.cxx",
                line,
                "CommandLineParser",
                message,
            );
            return Err(uncaught(log, &e));
        }
        Err(ParseFailure::InvalidFlags(flags)) => {
            for f in flags {
                log.cout(&format!("ERROR:  Invalid flag provided {f}\n"));
            }
            log.cerr("ERROR:  Invalid command line flags found! Aborting execution.\n");
            return Err(1);
        }
    };
    let verbose = match p.last("verbose") {
        Some(v) => Conv::bool(&v.name).map_err(|e| uncaught(log, &e))?,
        None => false,
    };
    log.verbose = verbose;
    if args.is_empty() {
        print_menu(log.out, "antsAI", DESCRIPTION, OPTIONS, Some(&p), 5, false);
        return Err(1);
    }
    if let Some(h) = p.last("help")
        && Conv::bool(&h.name).map_err(|e| uncaught(log, &e))?
    {
        print_menu(log.out, "antsAI", DESCRIPTION, OPTIONS, Some(&p), 5, false);
        return Ok(0);
    }
    if let Some(h) = p.last("")
        && Conv::bool(&h.name).map_err(|e| uncaught(log, &e))?
    {
        print_menu(log.out, "antsAI", DESCRIPTION, OPTIONS, Some(&p), 5, true);
        return Ok(0);
    }
    let Some(d) = p.last("dimensionality") else {
        log.cerr("Image dimensionality not specified.  See command line option --dimensionality\n");
        return Err(1);
    };
    let dimension = Conv::u32(&d.name).map_err(|e| uncaught(log, &e))?;
    let mut ctx = Context {
        p: &p,
        store,
        n_threads,
        env_seed,
        notices,
        warnings,
    };
    match dimension {
        2 => ants_ai::<2>(&mut ctx, log),
        3 => ants_ai::<3>(&mut ctx, log),
        _ => {
            log.cerr("Unrecognized dimensionality.  Please see help menu.\n");
            Err(1)
        }
    }
}

/// `ReadImage<itk::Image<float, D>>`: the image, or ANTs' message and `None` (ANTs goes on
/// with a null image).
fn read_float<const D: usize>(
    ctx: &mut Context<'_, '_>,
    log: &mut Log<'_>,
    name: &str,
) -> Option<Image<f32, D>> {
    match read_with::<f32, _>(ctx.store, name, D, ctx.n_threads, ctx.notices) {
        Ok(img) => match to_image(img) {
            Ok(i) => Some(i),
            Err(e) => {
                log.cerr(&format!("larmorx: antsAI: {name}: {}\n", e.description));
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

/// `GetFunction(0)` of an option: the last value given.
fn last<'p>(p: &'p Parsed, long: &str) -> Option<&'p OptionValue> {
    p.last(long)
}

/// `antsAI<D>(parser)` (`antsAI.cxx:610-1577`).
fn ants_ai<const D: usize>(ctx: &mut Context<'_, '_>, log: &mut Log<'_>) -> Result<Exit, Exit> {
    let p = ctx.p;
    let conv_err = |log: &mut Log<'_>, e: ItkException| uncaught(log, &e);
    let mut s = AiSettings {
        verbose: log.verbose,
        ..Default::default()
    };
    // The metric and the images.
    let Some(m) = last(p, "metric") else {
        log.cerr("Input metric not specified.\n");
        return Err(1);
    };
    let metric_name = m.name.to_ascii_lowercase();
    if m.parameters.len() < 2 {
        log.cerr("Fixed and/or moving images not specified.\n");
        return Err(1);
    }
    let fixed = read_float::<D>(ctx, log, param(m, 0));
    let moving = read_float::<D>(ctx, log, param(m, 1));
    if m.parameters.len() > 2 {
        s.bins = Conv::u32(param(m, 2)).map_err(|e| conv_err(log, e))?;
    }
    if m.parameters.len() > 3 {
        s.sampling = match param(m, 3).to_ascii_lowercase().as_str() {
            "none" => AiSampling::None,
            "regular" => AiSampling::Regular,
            "random" => AiSampling::Random,
            _ => {
                log.cerr("Unrecognized sampling strategy.\n");
                return Err(1);
            }
        };
    }
    if m.parameters.len() > 4 {
        s.sampling_percentage = Conv::f64(param(m, 4)).map_err(|e| conv_err(log, e))?;
    }
    s.metric = AiMetric::from_name(&metric_name);
    // The convergence.
    if let Some(c) = last(p, "convergence") {
        if c.parameters.is_empty() {
            s.iterations = Conv::u32(&c.name).map_err(|e| conv_err(log, e))?;
        } else {
            s.iterations = Conv::u32(param(c, 0)).map_err(|e| conv_err(log, e))?;
            if c.parameters.len() > 1 {
                s.convergence_threshold = Conv::f64(param(c, 1)).map_err(|e| conv_err(log, e))?;
            }
            if c.parameters.len() > 2 {
                s.convergence_window = Conv::u32(param(c, 2)).map_err(|e| conv_err(log, e))?;
            }
        }
    }
    // The search factor (degrees * pi / 180, as ANTs computes it).
    let pi = std::f64::consts::PI;
    if let Some(f) = last(p, "search-factor") {
        if f.parameters.is_empty() {
            s.search_factor = Conv::f64(&f.name).map_err(|e| conv_err(log, e))? * pi / 180.0;
        }
        if !f.parameters.is_empty() {
            s.search_factor = Conv::f64(param(f, 0)).map_err(|e| conv_err(log, e))? * pi / 180.0;
        }
        if f.parameters.len() > 1 {
            s.arc_fraction = Conv::f64(param(f, 1)).map_err(|e| conv_err(log, e))?;
        }
    }
    // The translation search grid.
    s.translation_grid = vec![0.0; D];
    if let Some(g) = last(p, "translation-search-grid") {
        s.translation_step = Conv::f64(param(g, 0)).map_err(|e| conv_err(log, e))?;
        if g.parameters.len() > 1 {
            s.translation_grid =
                Conv::vec_real::<f64>(param(g, 1)).map_err(|e| conv_err(log, e))?;
        }
    }
    // The transform.
    let transform_name = match last(p, "transform") {
        Some(t) => {
            if !t.parameters.is_empty() {
                s.learning_rate = Conv::f64(param(t, 0)).map_err(|e| conv_err(log, e))?;
            }
            t.name.to_ascii_lowercase()
        }
        None => String::new(),
    };
    let Some(transform) = AiTransform::from_name(&transform_name) else {
        log.cerr("Unrecognized transform option.\n");
        return Err(1);
    };
    s.transform = transform;
    // The images are used from here on: ANTs dereferences a missing one.
    // The images as read are the ones the search uses (float, as in ANTs; no copies).
    let images = |log: &mut Log<'_>,
                  blobs: bool,
                  fixed: Option<Image<f32, D>>,
                  moving: Option<Image<f32, D>>|
     -> Result<(Image<f32, D>, Image<f32, D>), Exit> {
        match (fixed, moving) {
            (Some(f), Some(m)) => Ok((f, m)),
            // The blob detector's Update with a null input.
            _ if blobs => Err(uncaught(
                log,
                &ItkException::new(
                    "Modules/Core/Common/src/itkProcessObject.cxx",
                    1339,
                    "MultiScaleLaplacianBlobDetectorImageFilter",
                    "Input Primary is required but not set.",
                ),
            )),
            (fixed, _) if transform.from_images() => Err(uncaught(
                log,
                &ItkException::new(
                    "Modules/Registration/Common/include/itkCenteredTransformInitializer.hxx",
                    if fixed.is_none() { 31 } else { 35 },
                    "CenteredTransformInitializer",
                    if fixed.is_none() {
                        "Fixed Image has not been set"
                    } else {
                        "Moving Image has not been set"
                    },
                ),
            )),
            _ => Err(uncaught(
                log,
                &ItkException::new(
                    "Modules/Filtering/ImageStatistics/include/itkImageMomentsCalculator.hxx",
                    225,
                    "ImageMomentsCalculator",
                    "GetCenterOfGravity() invoked, but the moments have not been computed. Call Compute() first.",
                ),
            )),
        }
    };
    if !transform.from_images() {
        if let Some(b) = last(p, "align-blobs") {
            if b.parameters.is_empty() {
                let n = Conv::u32(&b.name).map_err(|e| conv_err(log, e))?;
                s.blobs = (n, n);
            }
            if !b.parameters.is_empty() {
                let n = Conv::u32(param(b, 0)).map_err(|e| conv_err(log, e))?;
                s.blobs = (n, n);
            }
            if b.parameters.len() > 1 {
                s.blobs.1 = Conv::u32(param(b, 1)).map_err(|e| conv_err(log, e))?;
            }
            if s.blobs.0 < 3 {
                log.cerr("Please specify a greater number of blobs (>=3).\n");
                return Err(1);
            }
        }
        if s.blobs.0 < 3
            && let Some(a) = last(p, "align-principal-axes")
        {
            s.align_principal_axes = Conv::bool(&a.name).map_err(|e| conv_err(log, e))?;
        }
    }
    let (fixed32, moving32) = images(log, s.blobs.0 >= 3, fixed, moving)?;
    let init = initial(&s, &fixed32, &moving32, log, ctx.notices, ctx.n_threads)
        .map_err(|e| ai_failure(log, e))?;
    // The output's name.
    let output = last(p, "output").map(|o| {
        if o.parameters.is_empty() {
            o.name.clone()
        } else {
            param(o, 0).to_owned()
        }
    });
    // ANTS-53: the two alignments with iterations run the search, which lists no start
    // point, and write no transform. Corrected, the iterations are ignored (the metric is not
    // used, as the help says).
    let align_with_iterations = transform.from_images() && s.iterations > 0;
    if align_with_iterations {
        ctx.notices.add(
            &defects::ANTS_53,
            if ctx.notices.fix() {
                "AlignGeometricCenters and AlignCentersOfMass ignore --convergence: the alignment is written as with 0 iterations (ANTs writes no transform)"
            } else {
                "AlignGeometricCenters and AlignCentersOfMass with iterations write no transform, as in ANTs"
            },
        );
    }
    if s.iterations == 0 || (align_with_iterations && ctx.notices.fix()) {
        if let Some(name) = output {
            let t = without_search(&s, &init, ctx.warnings).map_err(|e| {
                flush(ctx, log);
                ai_failure(log, e)
            })?;
            flush(ctx, log);
            write_output(ctx, log, &name, Some(&t), true)?;
        }
        return Ok(0);
    }
    // The masks.
    let (mut fixed_mask, mut moving_mask) = (None, None);
    if let Some(x) = last(p, "masks") {
        match x.parameters.len() {
            0 => fixed_mask = read_mask::<D>(ctx, log, &x.name),
            1 => fixed_mask = read_mask::<D>(ctx, log, param(x, 0)),
            2 => {
                fixed_mask = read_mask::<D>(ctx, log, param(x, 0));
                moving_mask = read_mask::<D>(ctx, log, param(x, 1));
            }
            _ => {}
        }
    }
    // The sampling seed, read by the search after the metric's message.
    let mut seed_failure = None;
    if s.sampling != AiSampling::None {
        match last(p, "random-seed") {
            Some(r) => match Conv::i32(&r.name) {
                Ok(v) => s.random_seed = v,
                Err(e) => seed_failure = Some(AiError::Uncaught(e)),
            },
            None => {
                if let Some(v) = ctx.env_seed {
                    match super::cstd::stoi(v) {
                        Some(seed) => s.random_seed = seed,
                        None => {
                            // std::stoi throws invalid_argument without digits, else
                            // out_of_range.
                            let t = v.trim_start_matches([' ', '\t', '\n', '\x0b', '\x0c', '\r']);
                            let t = t.strip_prefix(['+', '-']).unwrap_or(t);
                            let kind = if t.starts_with(|c: char| c.is_ascii_digit()) {
                                "std::out_of_range"
                            } else {
                                "std::invalid_argument"
                            };
                            seed_failure = Some(AiError::Terminated(format!(
                                "terminate called after throwing an instance of '{kind}'\n  what():  stoi\n"
                            )));
                        }
                    }
                }
            }
        }
    }
    let outcome = search(
        &s,
        &init,
        Arc::new(fixed32),
        Arc::new(moving32),
        fixed_mask,
        moving_mask,
        log,
        ctx.notices,
        ctx.warnings,
        ctx.n_threads,
        seed_failure,
    );
    flush(ctx, log);
    let outcome = outcome.map_err(|e| ai_failure(log, e))?;
    let Some(name) = output else {
        return Ok(0);
    };
    match outcome {
        SearchOutcome::Best(best) => write_output(ctx, log, &name, Some(&best), false)?,
        // The two alignments: the writer gets no transform.
        SearchOutcome::NoStartPoint if transform.from_images() => {
            write_output::<D>(ctx, log, &name, None, false)?
        }
        SearchOutcome::NoStartPoint => {
            return Err(refuse(
                log,
                "no start point (a negative arc fraction or translation grid): ANTs reads the best parameters from an empty list and crashes",
            ));
        }
    }
    Ok(0)
}

/// Prints the warnings gathered so far (ITK prints them as they happen, before what follows
/// on the same stream).
fn flush(ctx: &mut Context<'_, '_>, log: &mut Log<'_>) {
    for w in ctx.warnings.drain(..) {
        log.cerr(&w);
    }
}

/// `TransformFileWriter::Update()` of `t` (`SetUseCompression(true)` when `compress`): the
/// class with its parameters and fixed parameters in double. `None`: the writer has no
/// transform (the two alignments with iterations, ANTS-53): ITK's MATLAB writer opens the
/// file and writes nothing; its text and HDF5 writers read the first transform of the empty
/// list and crash (refused).
///
/// ITK's exceptions are not caught (ANTs aborts): larmorx prints them as ANTs does.
fn write_output<const D: usize>(
    ctx: &mut Context<'_, '_>,
    log: &mut Log<'_>,
    name: &str,
    t: Option<&LinearXfm<f64, D>>,
    compress: bool,
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
        return Err(uncaught(log, &unknown_transform_type(name)));
    };
    let Some(t) = t else {
        if kind != "Matlab" {
            return Err(refuse(
                log,
                "the transform writer has no transform: ITK's text and HDF5 writers read the first transform of an empty list and crash (ANTS-53)",
            ));
        }
        return std::fs::File::create(path)
            .map(|_| ())
            .map_err(|_| uncaught(log, &open_failure(kind, name)));
    };
    let parts = vec![ItkTransformParts {
        name: format!("{}_double_{D}_{D}", t.kind.class()),
        parameters: Parameters::F64(t.parameters()),
        fixed_parameters: t.fixed_parameters(),
    }];
    let options = larmorx_io::itk_hdf5::WriteOptions {
        compression: compress,
        ..Default::default()
    };
    write_itk_transform_parts(path, &parts, &options, ctx.notices).map_err(|e| match e {
        TransformFileError::Io { .. } if kind != "HDF5" => uncaught(log, &open_failure(kind, name)),
        e => {
            match e.ants_report() {
                Some(r) => log.cerr(&r),
                None => log.cerr(&format!("{e}\n")),
            }
            refuse(
                log,
                "ITK's transform writer throws and antsAI does not catch it (ANTs aborts)",
            )
        }
    })
}

/// `TransformFileWriterTemplate::Update`'s exception for a name no transform writer takes.
pub(crate) fn unknown_transform_type(name: &str) -> ItkException {
    ItkException::new(
        "Modules/IO/TransformBase/src/itkTransformFileWriterSpecializations.cxx",
        153,
        "TransformFileWriterTemplate",
        format!(
            "Could not create Transform IO object for writing file {name}\n  Tried to create one of the following:\n    HDF5TransformIOTemplate\n    HDF5TransformIOTemplate\n    MatlabTransformIOTemplate\n    MatlabTransformIOTemplate\n    TxtTransformIOTemplate\n    TxtTransformIOTemplate\n  You probably failed to set a file suffix, or\n    set the suffix to an unsupported type.\n"
        ),
    )
}

/// `TransformIOBaseTemplate::OpenStream`'s exception for a file it cannot open.
pub(crate) fn open_failure(kind: &str, name: &str) -> ItkException {
    ItkException::new(
        "Modules/IO/TransformBase/src/itkTransformIOBase.cxx",
        88,
        match kind {
            "HDF5" => "HDF5TransformIOTemplate",
            "Matlab" => "MatlabTransformIOTemplate",
            _ => "TxtTransformIOTemplate",
        },
        format!("Failed opening file{name}"),
    )
}

/// The command's description (`antsAI.cxx:1820-1821`, the two strings joined as ANTs joins
/// them).
pub const DESCRIPTION: &str =
    "Program to calculate the optimallinear transform parameters for aligning two images.";

/// The options, as `InitializeCommandLineOptions` declares them (`antsAI.cxx:1579-1765`).
pub const OPTIONS: &[MenuOption] = &[
    MenuOption {
        long: "dimensionality",
        short: Some('d'),
        usage: &["2/3"],
        description: "This option forces the image to be treated as a specified-dimensional image.  If not specified, we try to infer the dimensionality from the input image.",
        defaults: &[],
    },
    MenuOption {
        long: "metric",
        short: Some('m'),
        usage: &[
            "",
            "Mattes[fixedImage,movingImage,<numberOfBins=32>,<samplingStrategy={None,Regular,Random}>,<samplingPercentage=[0,1]>]",
            "GC[fixedImage,movingImage,<radius=NA>,<samplingStrategy={None,Regular,Random}>,<samplingPercentage=[0,1]>]",
            "MI[fixedImage,movingImage,<numberOfBins=32>,<samplingStrategy={None,Regular,Random}>,<samplingPercentage=[0,1]>]",
        ],
        description: "These image metrics are available:  Mattes: Mattes mutual information (recommended), GC:  global correlation,  MI:  joint histogram mutual information",
        defaults: &[],
    },
    MenuOption {
        long: "transform",
        short: Some('t'),
        usage: &[
            "Rigid[gradientStep]",
            "Affine[gradientStep]",
            "Similarity[gradientStep]",
            "AlignGeometricCenters",
            "AlignCentersOfMass",
        ],
        description: "Several transform options are available.  For the rigid, affine, and similarity transforms, the gradientStep characterizes the gradient descent optimization and is scaled appropriately for each transform using the shift scales estimator. The other two transform types finds the simple translation transform which aligns the specified image feature.  Note that the images are read from the similarity metric option although the metric isn't actually used.",
        defaults: &[],
    },
    MenuOption {
        long: "align-principal-axes",
        short: Some('p'),
        usage: &[],
        description: "Boolean indicating alignment by principal axes.  Alternatively, one can align using blobs (see -b option).",
        defaults: &[],
    },
    MenuOption {
        long: "align-blobs",
        short: Some('b'),
        usage: &[
            "numberOfBlobsToExtract",
            "[numberOfBlobsToExtract,<numberOfBlobsToMatch=numberOfBlobsToExtract>]",
        ],
        description: "Boolean indicating alignment by a set of blobs.  Alternatively, one can align using blobs (see -p option).",
        defaults: &[],
    },
    MenuOption {
        long: "search-factor",
        short: Some('s'),
        usage: &["searchFactor", "[searchFactor=20,<arcFraction=1.0>]"],
        description: "Incremental search factor (in degrees) which will sample the arc fraction around the principal axis or default axis.",
        defaults: &[],
    },
    MenuOption {
        long: "translation-search-grid",
        short: Some('g'),
        usage: &["[stepSize=25, AxBxC=0x0x0]"],
        description: "Translation search grid in mm, which will translate the moving image in each dimension in increments of the step size.",
        defaults: &[],
    },
    MenuOption {
        long: "convergence",
        short: Some('c'),
        usage: &[
            "numberOfIterations",
            "[numberOfIterations,<convergenceThreshold=1e-6>,<convergenceWindowSize=10>]",
        ],
        description: "Number of iterations.",
        defaults: &[],
    },
    MenuOption {
        long: "masks",
        short: Some('x'),
        usage: &["fixedImageMask", "[fixedImageMask,movingImageMask]"],
        description: "Image masks to limit voxels considered by the metric.",
        defaults: &[],
    },
    MenuOption {
        long: "output",
        short: Some('o'),
        usage: &["outputFileName"],
        description: "Specify the output transform (output format an ITK .mat file). ",
        defaults: &[],
    },
    MenuOption {
        long: "random-seed",
        short: None,
        usage: &["seedValue"],
        description: "Use a fixed seed for random number generation. The default fixed seed is overwritten by this value. The fixed seed can be any nonzero int value. If the specified seed is zero, the system time will be used.",
        defaults: &[],
    },
    MenuOption {
        long: "verbose",
        short: Some('v'),
        usage: &["(0)/1"],
        description: "Verbose output.",
        defaults: &[],
    },
    MenuOption {
        long: "",
        short: Some('h'),
        usage: &[],
        description: "Print the help menu (short version).",
        defaults: &[],
    },
    MenuOption {
        long: "help",
        short: None,
        usage: &[],
        description: "Print the help menu.",
        defaults: &[],
    },
];
