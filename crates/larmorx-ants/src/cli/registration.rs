// SPDX-License-Identifier: Apache-2.0
//! `antsRegistration` with its original arguments (ANTs v2.6.5,
//! `Examples/antsRegistration.cxx` and `Examples/antsRegistrationTemplateHeader.h`), through
//! ANTs' command-line parser: the options are read, checked and converted in ANTs' order,
//! with its messages on its streams and its exit statuses; the images and initial transforms
//! are read where ANTs reads them; then `RegistrationHelper::DoRegistration` runs
//! ([`crate::registration::helper`]) and the outputs are written.
//!
//! As in ANTs:
//! - most errors are explained only with `--verbose 1`; larmorx always explains them
//!   (ANTS-45), on ANTs' stream when verbose and on standard error otherwise;
//! - a value the parser cannot convert (`-c [1o]`, `-t Rigid` without its step) is an ITK
//!   exception that `main` prints ("Exception Object caught: ..."); object addresses are
//!   printed as `0x0`;
//! - the option values are read newest first (`GetFunction(0)` is the last one given), the
//!   stages are numbered from the last on the command line (`AssignStages`).
//!
//! **Scope (A4 part 2).** The linear stages (`Translation`, `Rigid`, `Affine`; `Similarity`
//! and `CompositeAffine` as far as [`crate::registration`] ports them) with the Mattes and
//! mean-squares metrics. The deformable transforms are parsed as ANTs parses them and refused
//! before the registration runs (A5). What is not ported yet is refused with a message naming
//! it.

use std::io::Write;
use std::path::Path;
use std::sync::Arc;

use larmorx_core::defects::{EXIT_BAD_MODE, Notices, UpstreamDefects};
use larmorx_core::{IndexFloor, Precision};
use larmorx_registration::initializer::{Feature, initialize};
use larmorx_registration::mask::Mask;
use larmorx_registration::transform::{Kind, LinearXfm, Xfm};
use larmorx_registration::{Image, ItkException, Space};

use super::TransformLoader;
use super::cstd::{
    atof, parser_convert_bool, parser_convert_f32, parser_convert_f64, parser_convert_i32,
    parser_convert_u16, parser_convert_u32, parser_convert_vector_f32, parser_convert_vector_f64,
    parser_convert_vector_u32, stoi,
};
use super::menu::{MenuOption, print_menu};
use super::parser::{self, OptionValue, ParseFailure, Parsed};
use crate::image::{AntsImage, FileStore, ImageStore, ReadError, read_with, write_with};
use crate::registration::helper::{
    Compute, IntervalSink, Log, Metric, RegistrationFailed, RegistrationHelper, Sampling,
    TransformMethod,
};
use crate::registration::{MetricKind, XfrmMethod};
use larmorx_io::itk_transform::{
    ItkTransformParts, Parameters, transform_from_parts, write_itk_transform_parts,
};
use larmorx_io::nifti::itk::{GeometrySource, ItkGeometry};
use larmorx_registration::transform::collapse_linear;

/// What ends a run early: the exit status (the messages are already printed).
type Exit = u8;

/// An ITK exception `main` catches: printed as "Exception Object caught: ...", exit 1.
fn caught(log: &mut Log<'_>, e: &ItkException) -> Exit {
    log.cerr(&format!("Exception Object caught: \n{}\n", e.print()));
    1
}

/// The exception `CommandLineParser::Convert<T>` throws for `text` (`antsCommandLineParser.h:133`).
fn convert_exception(text: &str, code: &str, human: &str) -> ItkException {
    ItkException::new(
        "Utilities/antsCommandLineParser.h",
        133,
        "CommandLineParser",
        format!(
            "ERROR: Parse error occured during command line argument processing\nERROR: Unable to convert '{}' to type '{code}' as {human}\n",
            text.trim_end_matches([' ', '\n', '\r', '\t'])
        ),
    )
}

/// The parser's conversions, which throw where ANTs' do.
struct Conv;

impl Conv {
    fn bool(s: &str) -> Result<bool, ItkException> {
        parser_convert_bool(s).ok_or_else(|| convert_exception(s, "b", "Unmapped Type"))
    }
    fn u32(s: &str) -> Result<u32, ItkException> {
        parser_convert_u32(s).ok_or_else(|| convert_exception(s, "j", "unsigned int"))
    }
    fn u16(s: &str) -> Result<u16, ItkException> {
        parser_convert_u16(s).ok_or_else(|| convert_exception(s, "t", "unsigned int"))
    }
    fn i32(s: &str) -> Result<i32, ItkException> {
        parser_convert_i32(s).ok_or_else(|| convert_exception(s, "i", "signed int"))
    }
    fn f32(s: &str) -> Result<f32, ItkException> {
        parser_convert_f32(s).ok_or_else(|| convert_exception(s, "f", "float"))
    }
    fn f64(s: &str) -> Result<f64, ItkException> {
        parser_convert_f64(s).ok_or_else(|| convert_exception(s, "d", "double"))
    }
    /// `Convert<RealType>`.
    fn real<R: Compute>(s: &str) -> Result<R, ItkException> {
        match R::PRECISION {
            Precision::Double => Ok(R::r(Self::f64(s)?)),
            Precision::Float => Ok(R::r(f64::from(Self::f32(s)?))),
        }
    }
    /// `ConvertVector<unsigned int>`: the first piece that throws names itself.
    fn vec_u32(s: &str) -> Result<Vec<u32>, ItkException> {
        match parser_convert_vector_u32(s) {
            Some(v) => Ok(v),
            None => Err(convert_exception(
                &first_bad(s, |p| parser_convert_u32(p).is_some()),
                "j",
                "unsigned int",
            )),
        }
    }
    fn vec_f32(s: &str) -> Result<Vec<f32>, ItkException> {
        match parser_convert_vector_f32(s) {
            Some(v) => Ok(v),
            None => Err(convert_exception(
                &first_bad(s, |p| parser_convert_f32(p).is_some()),
                "f",
                "float",
            )),
        }
    }
    fn vec_real<R: Compute>(s: &str) -> Result<Vec<R>, ItkException> {
        match R::PRECISION {
            Precision::Double => match parser_convert_vector_f64(s) {
                Some(v) => Ok(v.into_iter().map(R::r).collect()),
                None => Err(convert_exception(
                    &first_bad(s, |p| parser_convert_f64(p).is_some()),
                    "d",
                    "double",
                )),
            },
            Precision::Float => Ok(Self::vec_f32(s)?
                .into_iter()
                .map(|v| R::r(f64::from(v)))
                .collect()),
        }
    }
}

/// The first piece of `ConvertVector`'s split (`getline(·, 'x')`) that does not convert.
fn first_bad(s: &str, ok: impl Fn(&str) -> bool) -> String {
    let t = s.trim_end_matches([' ', '\n', '\r', '\t']);
    let mut pieces: Vec<&str> = t.split('x').collect();
    if t.ends_with('x') {
        pieces.pop();
    }
    pieces.into_iter().find(|p| !ok(p)).unwrap_or("").to_owned()
}

/// `GetParameter(i)`: the parameter, or "" past the end.
fn param(v: &OptionValue, i: usize) -> &str {
    v.parameters.get(i).map_or("", String::as_str)
}

/// The option's functions newest first, then its default (`GetFunction(i)`).
fn functions(p: &Parsed, long: &str, default: Option<&'static str>) -> Vec<OptionValue> {
    let mut v: Vec<OptionValue> = p.all(long).iter().rev().cloned().collect();
    if let Some(d) = default {
        v.push(OptionValue {
            name: d.to_owned(),
            parameters: Vec::new(),
        });
    }
    v
}

/// `GetFunction(0)`: the last value given, or the default.
fn function0(p: &Parsed, long: &str) -> Option<OptionValue> {
    let default = OPTIONS
        .iter()
        .find(|o| o.long == long)
        .and_then(|o| o.defaults.first().copied());
    functions(p, long, default).into_iter().next()
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

/// Runs `antsRegistration` on files, in the D13 mode of `LARMORX_UPSTREAM_DEFECTS`; returns
/// the exit status.
pub fn main(
    args: &[String],
    loader: &dyn TransformLoader,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    match UpstreamDefects::from_env() {
        Ok(mode) => run(
            args,
            &mut FileStore,
            loader,
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

/// Runs `antsRegistration` with images from `store`, ANTs' thread count `n_threads` (larmorx's
/// result does not depend on it), the D13 mode `mode` and the value of `ANTS_RANDOM_SEED`.
#[allow(clippy::too_many_arguments)]
pub fn run(
    args: &[String],
    store: &mut dyn ImageStore,
    loader: &dyn TransformLoader,
    n_threads: usize,
    mode: UpstreamDefects,
    env_seed: Option<String>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let mut notices = Notices::new(mode);
    let code = {
        let mut log = Log::new(out, err, false);
        match antsregistration(
            args,
            store,
            loader,
            n_threads,
            &env_seed,
            &mut log,
            &mut notices,
        ) {
            Ok(code) | Err(code) => code,
        }
    };
    notices.print(err);
    code
}

/// [`run`] for in-memory images: transform files read by [`super::FileLoader`], no
/// `ANTS_RANDOM_SEED` (callers pass `--random-seed`). The Python wrapper's entry point.
pub fn run_store(
    args: &[String],
    store: &mut dyn ImageStore,
    n_threads: usize,
    mode: UpstreamDefects,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    run(
        args,
        store,
        &super::FileLoader,
        n_threads,
        mode,
        None,
        out,
        err,
    )
}

/// `antsRegistration`'s `main` (`antsRegistration.cxx`, from `Parse` on).
fn antsregistration(
    args: &[String],
    store: &mut dyn ImageStore,
    loader: &dyn TransformLoader,
    n_threads: usize,
    env_seed: &Option<String>,
    log: &mut Log<'_>,
    notices: &mut Notices,
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
            return Err(caught(log, &e));
        }
        Err(ParseFailure::InvalidFlags(flags)) => {
            for f in flags {
                log.cout(&format!("ERROR:  Invalid flag provided {f}\n"));
            }
            log.cerr("ERROR:  Invalid command line flags found! Aborting execution.\n");
            return Err(1);
        }
    };
    let throw = |log: &mut Log<'_>, e: ItkException| caught(log, &e);
    let verbose = match p.last("verbose") {
        Some(v) => Conv::bool(&v.name).map_err(|e| throw(log, e))?,
        None => false,
    };
    log.verbose = verbose;
    let write_composite = Conv::bool(
        &function0(&p, "write-composite-transform")
            .expect("default")
            .name,
    )
    .map_err(|e| throw(log, e))?;
    let per_stage = Conv::bool(
        &function0(&p, "initialize-transforms-per-stage")
            .expect("default")
            .name,
    )
    .map_err(|e| throw(log, e))?;
    if per_stage && !write_composite {
        explain_err(
            log,
            "ERROR:  --initialize-transforms-per-stage requires --write-composite-transform\n        because the initializizing transform is collapsed into each stage for optimization\n",
        );
        return Err(1);
    }
    if p.has("save-state") && !write_composite {
        explain_err(
            log,
            "ERROR:  --save-state requires --write-composite-transform\n        because the the output transform will contain the this processes initializer\n",
        );
        return Err(1);
    }
    if verbose {
        log.cout("All_Command_lines_OK\n");
    }
    if args.is_empty() {
        print_menu(
            log.out,
            "antsRegistration",
            DESCRIPTION,
            OPTIONS,
            Some(&p),
            5,
            false,
        );
        return Err(1);
    }
    if let Some(h) = p.last("help")
        && Conv::bool(&h.name).map_err(|e| throw(log, e))?
    {
        print_menu(
            log.out,
            "antsRegistration",
            DESCRIPTION,
            OPTIONS,
            Some(&p),
            5,
            false,
        );
        return Ok(0);
    }
    if let Some(h) = p.last("")
        && Conv::bool(&h.name).map_err(|e| throw(log, e))?
    {
        print_menu(
            log.out,
            "antsRegistration",
            DESCRIPTION,
            OPTIONS,
            Some(&p),
            5,
            true,
        );
        return Ok(0);
    }
    if let Some(v) = p.last("version") {
        let name = v.name.to_lowercase();
        if name == "1" || name == "true" {
            log.cout(&format!(
                "larmorx {} (port of ANTs v2.6.5)\n",
                larmorx_core::VERSION
            ));
            return Ok(0);
        }
    }
    let Some(d) = p.last("dimensionality") else {
        explain_err(
            log,
            "Image dimensionality not specified.  See command line option --dimensionality\n",
        );
        return Err(1);
    };
    let dimension = Conv::u32(&d.name).map_err(|e| throw(log, e))?;
    let float = match p.last("float") {
        Some(f) => Conv::bool(&f.name).map_err(|e| throw(log, e))?,
        None => false,
    };
    if float {
        log.logger("Using single precision for computations.\n");
    } else {
        log.logger("Using double precision for computations.\n");
    }
    let ctx = Context {
        p: &p,
        store,
        loader,
        n_threads,
        env_seed,
        notices,
    };
    match (dimension, float) {
        (2, false) => do_registration::<f64, 2>(ctx, log),
        (2, true) => do_registration::<f32, 2>(ctx, log),
        (3, false) => do_registration::<f64, 3>(ctx, log),
        (3, true) => do_registration::<f32, 3>(ctx, log),
        (4, false) => do_registration::<f64, 4>(ctx, log),
        (4, true) => do_registration::<f32, 4>(ctx, log),
        _ => {
            explain_err(log, &format!("bad image dimension {dimension}\n"));
            Err(1)
        }
    }
}

/// `if (verbose) std::cerr << message`: larmorx explains it in any case (ANTS-45).
fn explain_err(log: &mut Log<'_>, message: &str) {
    log.cerr(message);
}

/// `if (verbose) std::cout << message`; without `--verbose` larmorx explains it on standard
/// error (ANTS-45).
fn explain_out(log: &mut Log<'_>, message: &str) {
    if log.verbose {
        log.cout(message);
    } else {
        log.cerr(message);
    }
}

/// What `DoRegistration` reads from.
struct Context<'a, 'n> {
    p: &'a Parsed,
    store: &'a mut dyn ImageStore,
    loader: &'a dyn TransformLoader,
    n_threads: usize,
    env_seed: &'a Option<String>,
    notices: &'n mut Notices,
}

/// A refusal for what ANTs runs and larmorx does not (yet).
fn refuse(log: &mut Log<'_>, message: &str) -> Exit {
    log.cerr(&format!("larmorx: antsRegistration: {message}\n"));
    1
}

/// `ReadImage<ImageType>`: the image, or ANTs' message and `None`.
fn read_image<R: Compute, const D: usize>(
    ctx: &mut Context<'_, '_>,
    log: &mut Log<'_>,
    name: &str,
) -> Option<Image<R, D>> {
    match read_with::<R, _>(ctx.store, name, D, ctx.n_threads, ctx.notices) {
        Ok(img) => match to_image(img) {
            Ok(i) => Some(i),
            Err(e) => {
                log.cerr(&format!(
                    "larmorx: antsRegistration: {name}: {}\n",
                    e.description
                ));
                None
            }
        },
        Err(e) => {
            if let Some(m) = e.ants_message() {
                log.cerr(&format!("{m}\n"));
            }
            if let ReadError::Unsupported { .. } = e {
                log.cerr(&format!("{e}\n"));
            }
            None
        }
    }
}

/// An image as the registration holds it.
pub fn to_image<T: Copy, const D: usize>(img: AntsImage<T>) -> Result<Image<T, D>, ItkException> {
    let g = &img.geometry;
    let space = Space::new(
        std::array::from_fn(|k| g.size[k]),
        [0; D],
        std::array::from_fn(|k| g.spacing[k]),
        std::array::from_fn(|k| g.origin[k]),
        std::array::from_fn(|i| std::array::from_fn(|j| g.direction[i][j])),
    )?;
    Ok(Image::new(space, img.data))
}

/// One transform of an initial composite, with the name ANTs lists it under.
struct Initial<R, const D: usize> {
    transform: Xfm<R, D>,
    name: String,
    #[allow(dead_code)]
    derived: bool,
}

/// `GetCompositeTransformFromParserOption`: the option's functions newest first; each a file
/// (maybe inverted), the identity, or a transform derived from two images.
fn initial_composite<R: Compute, const D: usize>(
    ctx: &mut Context<'_, '_>,
    log: &mut Log<'_>,
    long: &str,
) -> Result<(Vec<Initial<R, D>>, Vec<bool>), Exit> {
    let functions = functions(ctx.p, long, None);
    let mut out: Vec<Initial<R, D>> = Vec::new();
    let mut derived = vec![false; functions.len()];
    for (n, f) in functions.iter().enumerate() {
        if f.parameters.len() > 2 {
            let fixed_name = param(f, 0).to_owned();
            let moving_name = param(f, 1).to_owned();
            let fixed = read_image::<R, D>(ctx, log, &fixed_name);
            let moving = if fixed.is_some() {
                read_image::<R, D>(ctx, log, &moving_name)
            } else {
                None
            };
            let feature = Conv::u16(param(f, 2)).map_err(|e| caught(log, &e))?;
            let (kind, feature) = match feature {
                0 => ("Image center alignment using ", Feature::Geometry),
                1 => ("Center of mass alignment using ", Feature::Moments),
                _ => ("Image origin alignment using ", Feature::Origins),
            };
            let (Some(fixed), Some(moving)) = (fixed, moving) else {
                if feature == Feature::Origins {
                    return Err(refuse(
                        log,
                        "an image of the initial transform could not be read (ANTs crashes here: it uses the missing image)",
                    ));
                }
                let e = ItkException::new(
                    "Modules/Registration/Common/include/itkCenteredTransformInitializer.hxx",
                    if f.parameters.is_empty() { 38 } else { 32 },
                    "CenteredTransformInitializer",
                    "Fixed Image has not been set",
                );
                return Err(caught(log, &e));
            };
            let (center, translation) =
                initialize(&fixed, &moving, feature).map_err(|e| caught(log, &e))?;
            let mut t = rigid_transform::<R, D>();
            if let Xfm::Linear(l) = &mut t {
                l.set_center(center);
                l.set_translation(translation);
            }
            derived[n] = true;
            out.push(Initial {
                transform: t,
                name: format!("{kind}fixed image: {fixed_name} and moving image: {moving_name}"),
                derived: true,
            });
            continue;
        }
        let (name, invert) = if f.parameters.is_empty() {
            (f.name.clone(), false)
        } else {
            let invert = if f.parameters.len() > 1 {
                Conv::bool(param(f, 1)).map_err(|e| caught(log, &e))?
            } else {
                false
            };
            (param(f, 0).to_owned(), invert)
        };
        derived[n] = false;
        let mut transforms: Vec<Xfm<R, D>> = if name == "identity" || name == "Identity" {
            vec![Xfm::Linear(LinearXfm::new(Kind::MatrixOffset))]
        } else {
            match load_transform::<R, D>(ctx, &name) {
                Ok(t) => t,
                Err(LoadError::Report(report)) => {
                    log.cerr(&report);
                    explain_out(log, &format!("Can't read initial transform {name}\n"));
                    return Err(1);
                }
                Err(LoadError::Unsupported(why)) => {
                    explain_out(log, &format!("Can't read initial transform {name}\n"));
                    return Err(refuse(log, &why));
                }
            }
        };
        let mut shown = name.clone();
        if invert {
            let mut inverted = Vec::new();
            for t in transforms.iter().rev() {
                match invert_xfm(t) {
                    Some(i) => inverted.push(i),
                    None => {
                        explain_out(log, &format!("Inverse does not exist for {name}\n"));
                        return Err(1);
                    }
                }
            }
            transforms = inverted;
            shown = format!("inverse of {name}");
        }
        let composite = transforms.len() > 1 || is_composite_file(&name);
        for (i, t) in transforms.into_iter().enumerate() {
            let entry_name = if composite {
                format!("{shown}[{i}]")
            } else {
                shown.clone()
            };
            out.push(Initial {
                transform: t,
                name: entry_name,
                derived: false,
            });
        }
    }
    if log.verbose {
        let mut s = String::from(
            "=============================================================================\nThe composite transform comprises the following transforms (in order): \n",
        );
        for (n, t) in out.iter().enumerate() {
            s.push_str(&format!(
                "  {}. {} (type = {})\n",
                n + 1,
                t.name,
                t.transform.class()
            ));
        }
        s.push_str(
            "=============================================================================\n",
        );
        log.cout(&s);
    }
    Ok((out, derived))
}

/// The transforms derived from images among the initial ones, written as
/// `<prefix><n>DerivedInitial<Moving|Fixed>Translation.mat` (ANTs indexes the option's
/// functions with `n` and takes the composite's `n`-th transform).
fn write_derived<R: Compute, const D: usize>(
    ctx: &mut Context<'_, '_>,
    log: &mut Log<'_>,
    prefix: &str,
    which: &str,
    queue: &[Xfm<R, D>],
    derived: &[bool],
    minc: bool,
) -> Result<(), Exit> {
    if minc && derived.iter().any(|&d| d) {
        return Err(refuse(
            log,
            "MINC transform files (--minc 1) are not supported yet",
        ));
    }
    for (n, &d) in derived.iter().enumerate() {
        if let Some(t) = queue.get(n)
            && d
            && t.is_linear()
        {
            let name = format!("{prefix}{n}DerivedInitial{which}Translation.mat");
            write_transform_file(log, ctx.notices, &name, &[xfm_parts(t)]);
        }
    }
    Ok(())
}

/// `RigidTransformTraits<R, D>::TransformType::New()`: `Euler2DTransform`, `Euler3DTransform`,
/// or `AffineTransform` in 4D.
fn rigid_transform<R: Compute, const D: usize>() -> Xfm<R, D> {
    let kind = match D {
        2 => Kind::Euler2D,
        3 => Kind::Euler3D { zyx: false },
        _ => Kind::Affine,
    };
    Xfm::Linear(LinearXfm::new(kind))
}

/// Whether a file holds a composite (`.h5` composites, the transforms ANTs lists with `[i]`).
fn is_composite_file(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.ends_with(".h5") || n.ends_with(".hdf5") || n.ends_with(".hdf")
}

enum LoadError {
    /// ANTs' report for the failure.
    Report(String),
    /// Something ANTs reads that larmorx does not yet.
    Unsupported(String),
}

/// `itk::ants::ReadTransform` of `name`, flattened (a composite's transforms in order).
fn load_transform<R: Compute, const D: usize>(
    ctx: &mut Context<'_, '_>,
    name: &str,
) -> Result<Vec<Xfm<R, D>>, LoadError> {
    if D != 3 {
        return Err(LoadError::Unsupported(format!(
            "initial transform files in {D} dimensions are not supported yet"
        )));
    }
    let t = ctx
        .loader
        .load_with(Path::new(name), R::PRECISION, ctx.notices)
        .map_err(|e| match e.ants_report() {
            Some(r) => LoadError::Report(r),
            None => LoadError::Report(format!("{e}\n")),
        })?;
    let mut flat = Vec::new();
    flatten::<R, D>(&t, ctx.notices.mode(), &mut flat)?;
    Ok(flat)
}

fn flatten<R: Compute, const D: usize>(
    t: &larmorx_transform::Transform,
    mode: UpstreamDefects,
    out: &mut Vec<Xfm<R, D>>,
) -> Result<(), LoadError> {
    use larmorx_transform::itk_object::LinearObject;
    match t {
        larmorx_transform::Transform::Composite(ts) => {
            for s in ts {
                flatten::<R, D>(s, mode, out)?;
            }
            Ok(())
        }
        larmorx_transform::Transform::DisplacementField(_) => Err(LoadError::Unsupported(
            "displacement fields among the initial transforms are not supported yet".into(),
        )),
        larmorx_transform::Transform::Linear(l) => {
            let o = LinearObject::<R>::read(l, mode)
                .map_err(|e| LoadError::Report(format!("{e}\n")))?;
            let Some(state) = o.state else {
                out.push(Xfm::Identity);
                return Ok(());
            };
            if o.translation_only {
                out.push(Xfm::Translation {
                    offset: std::array::from_fn(|i| state.offset[i]),
                });
                return Ok(());
            }
            let mut x = LinearXfm::frozen(
                o.class,
                o.parameters,
                o.fixed_parameters,
                std::array::from_fn(|i| std::array::from_fn(|j| state.matrix[i][j])),
                std::array::from_fn(|i| state.offset[i]),
                std::array::from_fn(|i| state.center[i]),
            );
            // `m_Translation` as the reader set it (not recomputed from the offset).
            x.translation = std::array::from_fn(|i| state.translation[i]);
            out.push(Xfm::Linear(x));
            Ok(())
        }
    }
}

/// `GetInverseTransform()` of an initial transform.
/// `CompositeTransform::Clone()` of the initial transforms (ANTs' `SetMovingInitialTransform`,
/// `SetFixedInitialTransform`, `SetRestoreStateTransform`): each transform cloned through its
/// parameters ([`Xfm::itk_clone`]).
fn itk_clones<R: Compute, const D: usize>(
    log: &mut Log<'_>,
    queue: Vec<Xfm<R, D>>,
) -> Result<Vec<Xfm<R, D>>, Exit> {
    queue
        .iter()
        .map(|t| t.itk_clone().map_err(|e| caught(log, &e)))
        .collect()
}

fn invert_xfm<R: Compute, const D: usize>(t: &Xfm<R, D>) -> Option<Xfm<R, D>> {
    match t {
        Xfm::Identity => Some(Xfm::Identity),
        Xfm::Translation { offset } => Some(Xfm::Translation {
            offset: std::array::from_fn(|i| -offset[i]),
        }),
        Xfm::Linear(l) => l.inverse().map(Xfm::Linear),
    }
}

/// `DoRegistration<TComputeType, VImageDimension>` (`antsRegistrationTemplateHeader.h`).
fn do_registration<R: Compute, const D: usize>(
    mut ctx: Context<'_, '_>,
    log: &mut Log<'_>,
) -> Result<Exit, Exit> {
    let p = ctx.p;
    let throw = |log: &mut Log<'_>, e: ItkException| caught(log, &e);
    let minc =
        Conv::bool(&function0(p, "minc").expect("default").name).map_err(|e| throw(log, e))?;
    let mut helper = RegistrationHelper::<R, D> {
        n_threads: ctx.n_threads,
        upstream_defects: ctx.notices.mode(),
        ..RegistrationHelper::default()
    };
    if let Some(s) = p.last("random-seed") {
        helper.random_seed = Conv::i32(&s.name).map_err(|e| throw(log, e))?;
    } else if let Some(env) = ctx.env_seed {
        match stoi(env) {
            Some(v) => helper.random_seed = v,
            None => {
                return Err(refuse(
                    log,
                    &format!(
                        "ANTS_RANDOM_SEED={env:?} is not a number (ANTs aborts here: std::stoi throws std::invalid_argument)"
                    ),
                ));
            }
        }
    }
    for (long, letter) in [
        ("transform", 't'),
        ("metric", 'm'),
        ("convergence", 'c'),
        ("shrink-factors", 'f'),
        ("smoothing-sigmas", 's'),
    ] {
        if !p.has(long) {
            let what = match long {
                "transform" => "transform",
                "metric" => "metric",
                "convergence" => "convergence",
                "shrink-factors" => "shrink factors",
                _ => "smoothing sigmas",
            };
            explain_err(
                log,
                &format!(
                    "ERROR: the {what} option ('-{letter}') must be specified.  See help menu.\n"
                ),
            );
            return Err(1);
        }
    }
    if !p.has("output") {
        explain_err(
            log,
            "ERROR: the output option ('-o') must be specified.  See help menu.\n",
        );
        return Err(1);
    }
    let write_composite = Conv::bool(
        &function0(p, "write-composite-transform")
            .expect("default")
            .name,
    )
    .map_err(|e| throw(log, e))?;
    let collapse = Conv::bool(
        &function0(p, "collapse-output-transforms")
            .expect("default")
            .name,
    )
    .map_err(|e| throw(log, e))?;
    let per_stage = Conv::bool(
        &function0(p, "initialize-transforms-per-stage")
            .expect("default")
            .name,
    )
    .map_err(|e| throw(log, e))?;
    if per_stage {
        if collapse {
            explain_err(
                log,
                "ERROR: initialize-transforms-per-stage & collapse-output-transforms options are mutually exclusive.\n",
            );
            return Err(1);
        }
        helper.initialize_transforms_per_stage = true;
    }
    if p.has("print-similarity-measure-interval") {
        let v = p.last("print-similarity-measure-interval").expect("has");
        helper.print_similarity_measure_interval = Conv::u32(&v.name).map_err(|e| throw(log, e))?;
    }
    if p.has("write-interval-volumes") {
        let v = p.last("write-interval-volumes").expect("has");
        helper.write_interval_volumes = Conv::u32(&v.name).map_err(|e| throw(log, e))?;
    }
    let output = p.last("output").expect("has").clone();
    let prefix = if output.parameters.is_empty() {
        output.name.clone()
    } else {
        param(&output, 0).to_owned()
    };
    let warped_name = param(&output, 1).to_owned();
    let inverse_warped_name = param(&output, 2).to_owned();
    // The initial transforms.
    let mut moving_initial_count = 0;
    if p.has("initial-moving-transform") {
        let (initial, derived) =
            initial_composite::<R, D>(&mut ctx, log, "initial-moving-transform")?;
        moving_initial_count = p.all("initial-moving-transform").len();
        let queue: Vec<Xfm<R, D>> = initial.into_iter().map(|i| i.transform).collect();
        if !collapse {
            write_derived(&mut ctx, log, &prefix, "Moving", &queue, &derived, minc)?;
        }
        // SetMovingInitialTransform clones the composite, each transform through its
        // parameters.
        helper.composite = Some(itk_clones(log, queue)?);
    }
    if p.has("initial-fixed-transform") {
        let (initial, derived) =
            initial_composite::<R, D>(&mut ctx, log, "initial-fixed-transform")?;
        let queue: Vec<Xfm<R, D>> = initial.into_iter().map(|i| i.transform).collect();
        if !collapse {
            write_derived(&mut ctx, log, &prefix, "Fixed", &queue, &derived, minc)?;
        }
        helper.fixed_initial = Some(itk_clones(log, queue)?);
    }
    if p.has("restore-state") {
        if p.has("initial-moving-transform") || p.has("initial-fixed-transform") {
            explain_err(
                log,
                "restore-state option is mutually exclusive with initial-moving-transform & initial-fixed-transform options.\n",
            );
            return Err(1);
        }
        // SetRestoreStateTransform: without SyN's four displacement fields at its end, the
        // state's transforms become the initial moving transforms.
        if log.verbose {
            log.cout("Restoring previous registration state\n");
        }
        let (restored, _) = initial_composite::<R, D>(&mut ctx, log, "restore-state")?;
        if log.verbose {
            log.cout("+\n");
        }
        helper.composite = Some(itk_clones(
            log,
            restored.into_iter().map(|i| i.transform).collect(),
        )?);
        if log.verbose {
            log.cout("+\n");
        }
    }
    // Masks: the functions oldest first.
    if p.has("masks") {
        log.logger("  Reading mask(s).\n");
        let masks = p.all("masks").to_vec();
        for (stage, m) in masks.iter().enumerate() {
            log.logger(&format!("    Registration stage {stage}\n"));
            if m.parameters.is_empty() {
                let mask = read_mask::<D>(&mut ctx, log, &m.name);
                log.logger(&match &mask {
                    Some(_) => format!("      Fixed mask = {}\n", m.name),
                    None => "      No fixed mask\n".into(),
                });
                helper.fixed_masks.push(mask);
            } else {
                for (i, name) in m.parameters.iter().enumerate() {
                    let mask = read_mask::<D>(&mut ctx, log, name);
                    if i == 0 {
                        log.logger(&match &mask {
                            Some(_) => format!("      Fixed mask = {name}\n"),
                            None => "      No fixed mask\n".into(),
                        });
                        helper.fixed_masks.push(mask);
                    } else if i == 1 {
                        log.logger(&match &mask {
                            Some(_) => format!("      Moving mask = {name}\n"),
                            None => "      No moving mask\n".into(),
                        });
                        helper.moving_masks.push(mask);
                    }
                }
            }
        }
    }
    // Winsorizing and histogram matching.
    let mut lower = 0.0f32;
    let mut upper = 1.0f32;
    let mut winsorize = false;
    if let Some(w) = p.last("winsorize-image-intensities") {
        winsorize = true;
        if !w.parameters.is_empty() {
            lower = Conv::f32(param(w, 0)).map_err(|e| throw(log, e))?;
        }
        if w.parameters.len() > 1 {
            upper = Conv::f32(param(w, 1)).map_err(|e| throw(log, e))?;
        }
    }
    helper.winsorize = winsorize;
    helper.lower_quantile = R::r(f64::from(lower));
    helper.upper_quantile = R::r(f64::from(upper));
    helper.use_histogram_matching = p.last("use-histogram-matching").is_some_and(|h| {
        let f = h.name.to_lowercase();
        f == "1" || f == "true"
    });
    // The stages.
    let transforms = functions(p, "transform", None);
    let n_transforms = transforms.len();
    let convergence = functions(p, "convergence", None);
    let shrink = functions(p, "shrink-factors", None);
    let sigmas = functions(p, "smoothing-sigmas", None);
    if sigmas.len() != shrink.len()
        || shrink.len() != convergence.len()
        || convergence.len() != n_transforms
    {
        explain_err(
            log,
            "ERROR: smoothing sigmas, shrink factors, and convergence options must be set once for each transform.\n",
        );
        return Err(1);
    }
    let restrict = functions(p, "restrict-deformation", None);
    let metrics = functions(p, "metric", None);
    let metric_stages: Vec<usize> = {
        // GetFunction(i)->GetStageID(): stage ids in newest-first order.
        let ids = p.stage_ids("metric");
        ids.into_iter().rev().collect()
    };
    let mut deformable: Option<XfrmMethod> = None;
    for current in (0..n_transforms).rev() {
        let c = &convergence[current];
        let iterations = if c.parameters.is_empty() {
            Conv::vec_u32(&c.name)
        } else {
            Conv::vec_u32(param(c, 0))
        }
        .map_err(|e| throw(log, e))?;
        let mut threshold = R::r(1e-6);
        let mut window = 10u32;
        if c.parameters.len() > 1 {
            threshold = Conv::real::<R>(param(c, 1)).map_err(|e| throw(log, e))?;
        }
        if c.parameters.len() > 2 {
            window = Conv::u32(param(c, 2)).map_err(|e| throw(log, e))?;
            if window < 2 {
                explain_err(
                    log,
                    "Convergence Window Size must be greater than or equal to 2\n",
                );
                return Err(1);
            }
        }
        helper.iterations.push(iterations.clone());
        helper.convergence_thresholds.push(threshold);
        helper.convergence_window_sizes.push(window);
        if restrict.len() > current {
            let w = Conv::vec_real::<R>(&restrict[current].name).map_err(|e| throw(log, e))?;
            helper.restrict_deformation.push(w);
        }
        let levels = iterations.len();
        log.logger(&format!("  number of levels = {levels}\n"));
        let factors = Conv::vec_u32(&shrink[current].name).map_err(|e| throw(log, e))?;
        helper.shrink_factors.push(factors);
        let mut sigma_text = sigmas[current].name.clone();
        let physical = if let Some(i) = sigma_text.find("mm") {
            sigma_text.replace_range(i..i + 2, "");
            true
        } else if let Some(i) = sigma_text.find("vox") {
            sigma_text.replace_range(i..i + 3, "");
            false
        } else {
            false
        };
        helper.sigmas_in_physical_units.push(physical);
        let mut s = Conv::vec_f32(&sigma_text).map_err(|e| throw(log, e))?;
        if s.len() == 1 {
            s.resize(levels, s[0]);
        }
        helper.smoothing_sigmas.push(s);
        let t = &transforms[current];
        let step = Conv::f32(param(t, 0)).map_err(|e| throw(log, e))?;
        let method_name = t.name.to_lowercase();
        let Some(method) = XfrmMethod::from_name(&method_name) else {
            explain_err(
                log,
                &format!("Unknown registration method \"{method_name}\"\n"),
            );
            return Err(1);
        };
        if !method.is_linear() {
            parse_deformable(t, method, &transforms).map_err(|e| throw(log, e))?;
            deformable.get_or_insert(method);
        }
        helper
            .transforms
            .push(TransformMethod::linear(method, R::r(f64::from(step))));
    }
    helper.output_prefix = prefix.clone();
    // The metrics, oldest first.
    for (k, m) in metrics.iter().enumerate().rev() {
        let stage = metric_stages[k];
        if k == metrics.len() - 1 && stage != n_transforms - 1 {
            explain_err(
                log,
                &format!(
                    "\n\n\nError:  The number of stages does not match up with the metrics.\nThe number of transforms is {n_transforms} and the last stage ID  as determined by the metrics is {stage}.\n"
                ),
            );
            return Err(1);
        }
        let name = m.name.to_lowercase();
        let kind = MetricKind::from_name(&name);
        let mut weighting = 1.0f32;
        if m.parameters.len() > 2 {
            weighting = Conv::f32(param(m, 2)).map_err(|e| throw(log, e))?;
        }
        let mut metric = Metric::<R, D> {
            kind,
            fixed: None,
            moving: None,
            stage,
            weighting: R::r(f64::from(weighting)),
            sampling: Sampling::None,
            number_of_bins: 32,
            radius: 4,
            use_gradient_filter: false,
            use_boundary_points_only: false,
            point_set_sigma: R::ONE,
            evaluation_k_neighborhood: R::r(50.0),
            alpha: R::r(1.1),
            use_anisotropic_covariances: false,
            sampling_percentage: R::ONE,
            intensity_distance_sigma: R::ZERO,
            euclidean_distance_sigma: R::ZERO,
        };
        if kind.is_some_and(|k| k.is_point_set()) {
            return Err(refuse(
                log,
                "the point-set metrics (ICP, PSE, JHCT, IGDM) are not supported yet",
            ));
        }
        if m.parameters.len() > 5 {
            let v = Conv::f32(param(m, 5)).map_err(|e| throw(log, e))?;
            metric.sampling_percentage = R::r(f64::from(v));
        }
        if m.parameters.len() > 6 {
            metric.use_gradient_filter = Conv::bool(param(m, 6)).map_err(|e| throw(log, e))?;
        }
        let fixed_name = param(m, 0).to_owned();
        let moving_name = param(m, 1).to_owned();
        log.logger(&format!(
            "  fixed image: {fixed_name}\n  moving image: {moving_name}\n"
        ));
        let fixed = read_image::<R, D>(&mut ctx, log, &fixed_name);
        let moving = read_image::<R, D>(&mut ctx, log, &moving_name);
        if fixed.is_none() || moving.is_none() {
            // ANTs dereferences the null image (DisconnectPipeline) and crashes.
            return Err(refuse(
                log,
                "a metric image could not be read (ANTs crashes here: it uses the missing image)",
            ));
        }
        metric.fixed = fixed.map(Arc::new);
        metric.moving = moving.map(Arc::new);
        let strategy = param(m, 4).to_lowercase();
        metric.sampling = match strategy.as_str() {
            "random" => Sampling::Random,
            "regular" => Sampling::Regular,
            "none" | "" => Sampling::None,
            _ => {
                explain_err(
                    log,
                    &format!("ERROR: invalid sampling strategy specified: {strategy}\n"),
                );
                return Err(1);
            }
        };
        match kind {
            Some(MetricKind::Cc) if m.parameters.len() > 3 => {
                metric.radius = Conv::u32(param(m, 3)).map_err(|e| throw(log, e))?;
            }
            Some(MetricKind::Mattes | MetricKind::Mi) if m.parameters.len() > 3 => {
                metric.number_of_bins = Conv::u32(param(m, 3)).map_err(|e| throw(log, e))? as i32;
            }
            _ => {}
        }
        helper.metrics.push(metric);
    }
    if let Some(method) = deformable {
        return Err(refuse(
            log,
            &format!(
                "{} stages are deformable registration, which larmorx ports in milestone A5; this version runs the linear stages (Translation, Rigid, Similarity, Affine, CompositeAffine) only",
                method.as_str()
            ),
        ));
    }
    // DoRegistration.
    let floor = IndexFloor::new(ctx.notices.mode());
    if helper.write_interval_volumes != 0 && D != 3 {
        return Err(refuse(
            log,
            &format!("--write-interval-volumes in {D} dimensions is not supported yet"),
        ));
    }
    let done = match (&helper.metrics[0].fixed, &helper.metrics[0].moving) {
        (Some(f), Some(m)) if helper.write_interval_volumes != 0 => {
            let mut sink = IntervalVolumes {
                ctx: &mut ctx,
                fixed: Arc::clone(f),
                moving: Arc::clone(m),
            };
            helper.do_registration(log, &floor, Some(&mut sink))
        }
        _ => helper.do_registration(log, &floor, None),
    };
    ctx.notices.extend(std::mem::take(&mut helper.notices));
    if let Err(RegistrationFailed(_)) = done {
        floor.notice(ctx.notices, "interpolated values");
        return Err(1);
    }
    floor.notice(ctx.notices, "interpolated values");
    // The outputs.
    write_outputs::<R, D>(
        &mut ctx,
        log,
        &helper,
        &prefix,
        &warped_name,
        &inverse_warped_name,
        write_composite,
        collapse,
        moving_initial_count,
        minc,
    )
}

/// The deformable transforms' parameters, read as ANTs reads them (so their conversion errors
/// are ANTs'); the transforms themselves are A5's.
fn parse_deformable(
    t: &OptionValue,
    method: XfrmMethod,
    all: &[OptionValue],
) -> Result<(), ItkException> {
    let n = t.parameters.len();
    match method {
        XfrmMethod::GaussianDisplacementField => {
            Conv::f32(param(t, 1))?;
            Conv::f32(param(t, 2))?;
        }
        XfrmMethod::BSplineDisplacementField => {
            if n > 3 {
                Conv::u32(param(t, 3))?;
            }
            Conv::vec_u32(param(t, 1))?;
            if n > 2 {
                Conv::vec_u32(param(t, 2))?;
            }
        }
        XfrmMethod::BSpline => {
            Conv::vec_u32(param(t, 1))?;
        }
        XfrmMethod::TimeVaryingVelocityField => {
            // ANTs reads the number of time indices from the newest transform.
            Conv::u32(param(&all[0], 1))?;
            for i in 2..6 {
                Conv::f32(param(t, i))?;
            }
        }
        XfrmMethod::TimeVaryingBSplineVelocityField => {
            Conv::vec_u32(param(&all[0], 1))?;
            if n > 2 {
                Conv::u32(param(t, 2))?;
            }
            if n > 3 {
                Conv::u32(param(t, 3))?;
            }
        }
        XfrmMethod::SyN => {
            if n > 1 {
                Conv::f32(param(t, 1))?;
            }
            if n > 2 {
                Conv::f32(param(t, 2))?;
            }
        }
        XfrmMethod::BSplineSyN => {
            if n > 3 {
                Conv::u32(param(t, 3))?;
            }
            let v = Conv::vec_f32(param(t, 1))?;
            if v.len() != 1 {
                Conv::vec_u32(param(t, 1))?;
            }
            if n > 2 {
                let v = Conv::vec_f32(param(t, 2))?;
                if v.len() != 1 {
                    Conv::vec_u32(param(t, 2))?;
                }
            }
        }
        XfrmMethod::Exponential => {
            Conv::f32(param(t, 1))?;
            Conv::f32(param(t, 2))?;
            if n > 3 {
                Conv::u32(param(t, 3))?;
            }
        }
        XfrmMethod::BSplineExponential => {
            if n > 4 {
                Conv::u32(param(t, 4))?;
            }
            Conv::vec_u32(param(t, 1))?;
            if n > 2 {
                Conv::vec_u32(param(t, 2))?;
            }
            if n > 3 {
                Conv::u32(param(t, 3))?;
            }
        }
        _ => {}
    }
    let _ = atof;
    let _ = Conv::u16;
    Ok(())
}

/// `ReadImage<MaskImageType>` (`unsigned char`).
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

/// The interpolator of the warped outputs (`make_interpolator_snip.tmpl`), read with the
/// parser's strict conversions. `fixed_spacing` is `cache_spacing_for_smoothing_sigmas`.
fn output_interpolation<const D: usize>(
    p: &Parsed,
    log: &mut Log<'_>,
    fixed_spacing: [f64; 3],
) -> Result<larmorx_interp::Interpolation, Exit> {
    use larmorx_interp::{Interpolation, Window};
    let Some(v) = function0(p, "interpolation") else {
        return Ok(Interpolation::Linear);
    };
    let name = v.name.to_lowercase();
    let sigma = |log: &mut Log<'_>| -> Result<[f64; 3], Exit> {
        if v.parameters.is_empty() {
            return Ok(fixed_spacing);
        }
        let s = match parser_convert_vector_f64(param(&v, 0)) {
            Some(s) => s,
            None => {
                let e = convert_exception(
                    &first_bad(param(&v, 0), |x| parser_convert_f64(x).is_some()),
                    "d",
                    "double",
                );
                return Err(caught(log, &e));
            }
        };
        Ok(if s.len() == D {
            std::array::from_fn(|d| s[d.min(s.len() - 1)])
        } else {
            [s[0]; 3]
        })
    };
    Ok(match name.as_str() {
        "linear" => Interpolation::Linear,
        "nearestneighbor" => Interpolation::NearestNeighbor,
        "bspline" => {
            let order = if v.parameters.is_empty() {
                3
            } else {
                Conv::u32(param(&v, 0)).map_err(|e| caught(log, &e))?
            };
            Interpolation::BSpline { order }
        }
        "gaussian" => {
            let sigma = sigma(log)?;
            let alpha = if v.parameters.len() > 1 {
                Conv::f64(param(&v, 1)).map_err(|e| caught(log, &e))?
            } else {
                1.0
            };
            Interpolation::Gaussian { sigma, alpha }
        }
        "cosinewindowedsinc" => Interpolation::WindowedSinc(Window::Cosine),
        "hammingwindowedsinc" => Interpolation::WindowedSinc(Window::Hamming),
        "lanczoswindowedsinc" => Interpolation::WindowedSinc(Window::Lanczos),
        "blackmanwindowedsinc" => Interpolation::WindowedSinc(Window::Blackman),
        "welchwindowedsinc" => Interpolation::WindowedSinc(Window::Welch),
        "genericlabel" => Interpolation::GenericLabel,
        "multilabel" => Interpolation::MultiLabel {
            sigma: sigma(log)?,
            alpha: 4.0,
        },
        _ => {
            log.cout(&format!(
                "Error:  Unrecognized interpolation option. {name}\n"
            ));
            return Err(1);
        }
    })
}

/// `itk::ants::WriteTransform`'s parts for one transform of the composite.
fn xfm_parts<R: Compute, const D: usize>(t: &Xfm<R, D>) -> ItkTransformParts {
    let parameters: Vec<R> = t.parameters();
    let parameters = match R::PRECISION {
        Precision::Double => Parameters::F64(parameters.iter().map(|v| v.to_f64()).collect()),
        Precision::Float => Parameters::F32(parameters.iter().map(|v| v.to_f64() as f32).collect()),
    };
    ItkTransformParts {
        name: format!("{}_{}_{D}_{D}", t.class(), R::ITK_NAME),
        parameters,
        fixed_parameters: t.fixed_parameters(),
    }
}

/// The chain `antsApplyTransforms` would build for an ITK queue (items in command-line
/// order: the queue reversed), for the 3D resampler.
fn chain_of<R: Compute, const D: usize>(
    queue: &[Xfm<R, D>],
) -> Result<larmorx_transform::TransformChain, String> {
    let mut items = Vec::new();
    for t in queue.iter().rev() {
        let (name, parameters, fixed) = match t {
            Xfm::Identity => continue,
            Xfm::Translation { offset } => (
                "TranslationTransform_double_3_3".to_owned(),
                offset.iter().map(|v| v.to_f64()).collect::<Vec<_>>(),
                Vec::new(),
            ),
            Xfm::Linear(l) => {
                // The transform maps with its matrix and offset as they are (`m_Offset`, which
                // an inverse sets directly): given with a centre of 0, the reader's offset is
                // the translation, exactly.
                let mut p: Vec<f64> = l.matrix.iter().flatten().map(|v| v.to_f64()).collect();
                p.extend(l.offset.iter().map(|v| v.to_f64()));
                (
                    "MatrixOffsetTransformBase_double_3_3".to_owned(),
                    p,
                    vec![0.0; D],
                )
            }
        };
        let part = ItkTransformParts {
            name,
            parameters: Parameters::F64(parameters),
            fixed_parameters: fixed,
        };
        items.push((
            transform_from_parts(&[part]).map_err(|e| e.to_string())?,
            false,
        ));
    }
    larmorx_transform::TransformChain::new(items).map_err(|e| e.to_string())
}

/// `GetWarpedImage` / `GetInverseWarpedImage`: `input` resampled onto `reference` through
/// the queue with the output interpolator (default value 0), as `R` pixels.
fn resample<R: Compute, const D: usize>(
    input: &Image<R, D>,
    reference: &Image<R, D>,
    queue: &[Xfm<R, D>],
    interpolation: &larmorx_interp::Interpolation,
    ctx: &mut Context<'_, '_>,
) -> Result<Vec<R>, String> {
    if D != 3 {
        return Err(format!(
            "warped images in {D} dimensions are not supported yet"
        ));
    }
    let grid = |s: &Space<D>| {
        larmorx_core::Grid3::new(
            std::array::from_fn(|i| s.size[i]),
            std::array::from_fn(|i| s.spacing[i]),
            std::array::from_fn(|i| s.origin[i]),
            std::array::from_fn(|i| std::array::from_fn(|j| s.direction[i][j])),
        )
        .ok_or_else(|| "singular image grid".to_owned())
    };
    let chain = chain_of(queue)?;
    let options = crate::ApplyTransformsOptions {
        interpolation: interpolation.clone(),
        default_value: 0.0,
        n_threads: ctx.n_threads,
        precision: R::PRECISION,
        upstream_defects: ctx.notices.mode(),
    };
    let (values, notices) = crate::apply_transforms(
        &input.data,
        &grid(&input.space)?,
        &grid(&reference.space)?,
        &chain,
        &options,
    )
    .map_err(|e| e.to_string())?;
    for n in notices {
        ctx.notices.extend([n]);
    }
    Ok(values.into_iter().map(R::r).collect())
}

/// An image to write with the reference's geometry.
/// `--write-interval-volumes`' writer: `WriteIntervalVolumes` resamples the original moving
/// image onto the original fixed image's grid with the moving transforms only (the fixed
/// initial transform is not applied), linearly, default value 0, and writes it; a write
/// error prints ANTs' messages on standard output and the registration goes on.
struct IntervalVolumes<'c, 'x, 'n, R, const D: usize> {
    ctx: &'c mut Context<'x, 'n>,
    fixed: Arc<Image<R, D>>,
    moving: Arc<Image<R, D>>,
}

impl<R: Compute, const D: usize> IntervalSink<R, D> for IntervalVolumes<'_, '_, '_, R, D> {
    fn write_interval_volume(&mut self, log: &mut Log<'_>, name: &str, moving: &[Xfm<R, D>]) {
        let linear = larmorx_interp::Interpolation::Linear;
        let written =
            resample(&self.moving, &self.fixed, moving, &linear, self.ctx).and_then(|data| {
                let image = output_image(&self.fixed, data);
                write_with(
                    self.ctx.store,
                    name,
                    R::output(image),
                    self.ctx.n_threads,
                    self.ctx.notices,
                )
            });
        if let Err(e) = written {
            log.cout(&format!(
                "Can't write warped image {name}\nException Object caught: \n{e}\n"
            ));
        }
    }
}

fn output_image<R: Compute, const D: usize>(reference: &Image<R, D>, data: Vec<R>) -> AntsImage<R> {
    let s = &reference.space;
    AntsImage {
        data,
        geometry: ItkGeometry {
            ndim: D,
            size: s.size.to_vec(),
            spacing: s.spacing.to_vec(),
            origin: s.origin.to_vec(),
            direction: s.direction.iter().map(|r| r.to_vec()).collect(),
            source: GeometrySource::Sform,
            flipped: vec![false; D],
        },
        meta: Default::default(),
    }
}

/// `RegTypeToFileName` for the linear transforms (without MINC).
fn file_suffix(kind: &str) -> &'static str {
    match kind {
        "rigid" => "Rigid.mat",
        "affine" | "compositeaffine" | "compaff" => "Affine.mat",
        "similarity" => "Similarity.mat",
        "translation" => "Translation.mat",
        "genericaffine" => "GenericAffine.mat",
        _ => "BOGUS.XXXX",
    }
}

/// `itk::ants::WriteTransform` of `parts` to `name`: a failure is reported and the run goes
/// on, as ANTs ignores its return value.
fn write_transform_file(
    log: &mut Log<'_>,
    notices: &mut Notices,
    name: &str,
    parts: &[ItkTransformParts],
) {
    let options = larmorx_io::itk_hdf5::WriteOptions::default();
    if let Err(e) = write_itk_transform_parts(Path::new(name), parts, &options, notices) {
        match e.ants_report() {
            Some(r) => log.cerr(&r),
            None => log.cerr(&format!("{e}\n")),
        }
    }
}

/// The outputs of `DoRegistration` (`antsRegistrationTemplateHeader.h`, after the
/// registration): the warped images, the saved state, the transforms.
#[allow(clippy::too_many_arguments)]
fn write_outputs<R: Compute, const D: usize>(
    ctx: &mut Context<'_, '_>,
    log: &mut Log<'_>,
    helper: &RegistrationHelper<R, D>,
    prefix: &str,
    warped_name: &str,
    inverse_warped_name: &str,
    write_composite: bool,
    collapse: bool,
    moving_initial_count: usize,
    minc: bool,
) -> Result<Exit, Exit> {
    let p = ctx.p;
    let first = &helper.metrics[0];
    let (Some(fixed), Some(moving)) = (&first.fixed, &first.moving) else {
        return Err(refuse(log, "the first metric has no images"));
    };
    let spacing3: [f64; 3] =
        std::array::from_fn(|i| if i < D { fixed.space.spacing[i] } else { 1.0 });
    let interpolation = output_interpolation::<D>(p, log, spacing3)?;
    let queue = helper.composite.clone().unwrap_or_default();
    if !warped_name.is_empty() {
        let data =
            resample(moving, fixed, &queue, &interpolation, ctx).map_err(|e| refuse(log, &e))?;
        let image = output_image(fixed, data);
        write_with(
            ctx.store,
            warped_name,
            R::output(image),
            ctx.n_threads,
            ctx.notices,
        )
        .map_err(|e| refuse(log, &e))?;
    }
    if !inverse_warped_name.is_empty() {
        let inverse: Option<Vec<Xfm<R, D>>> = queue.iter().rev().map(invert_xfm).collect();
        if let Some(inverse) = inverse {
            let data = resample(fixed, moving, &inverse, &interpolation, ctx)
                .map_err(|e| refuse(log, &e))?;
            let image = output_image(moving, data);
            write_with(
                ctx.store,
                inverse_warped_name,
                R::output(image),
                ctx.n_threads,
                ctx.notices,
            )
            .map_err(|e| refuse(log, &e))?;
        }
    }
    if let Some(state) = p.last("save-state") {
        // ANTs sets the registration state in SyN stages only: after linear stages it finds
        // none and returns EXIT_FAILURE, writing no transform (ANTS-50). Corrected, the state
        // is the composite of every transform, as the option's description says.
        if ctx.notices.reproduce() {
            ctx.notices.add(
                &larmorx_core::defects::ANTS_50,
                "no state after linear stages: exit 1, no transform written, as in ANTs",
            );
            return Err(1);
        }
        ctx.notices.add(
            &larmorx_core::defects::ANTS_50,
            "the state is written (every transform of the registration); ANTs exits 1 here",
        );
        let mut parts = vec![ItkTransformParts {
            name: format!("CompositeTransform_{}_{D}_{D}", R::ITK_NAME),
            parameters: Parameters::F64(Vec::new()),
            fixed_parameters: Vec::new(),
        }];
        parts.extend(queue.iter().map(xfm_parts));
        write_transform_file(log, ctx.notices, &state.name, &parts);
    }
    if minc {
        return Err(refuse(
            log,
            "MINC transform files (--minc 1) are not supported yet",
        ));
    }
    let (to_write, names): (Vec<Xfm<R, D>>, Vec<String>) = if collapse {
        match collapse_linear(&queue) {
            Some(t) => (vec![Xfm::Linear(t)], vec!["genericaffine".into()]),
            None => {
                return Err(refuse(
                    log,
                    "an IdentityTransform cannot be collapsed (ANTs crashes here)",
                ));
            }
        }
    } else {
        let names = helper
            .transforms
            .iter()
            .map(|t| t.method.as_str().to_lowercase())
            .collect();
        (queue.clone(), names)
    };
    if write_composite {
        let head = ItkTransformParts {
            name: format!("CompositeTransform_{}_{D}_{D}", R::ITK_NAME),
            parameters: Parameters::F64(Vec::new()),
            fixed_parameters: Vec::new(),
        };
        let mut parts = vec![head.clone()];
        parts.extend(to_write.iter().map(xfm_parts));
        write_transform_file(log, ctx.notices, &format!("{prefix}Composite.h5"), &parts);
        let inverse: Option<Vec<Xfm<R, D>>> = to_write.iter().rev().map(invert_xfm).collect();
        if let Some(inverse) = inverse {
            let mut parts = vec![head];
            parts.extend(inverse.iter().map(xfm_parts));
            write_transform_file(
                log,
                ctx.notices,
                &format!("{prefix}InverseComposite.h5"),
                &parts,
            );
        }
    } else if collapse {
        let name = format!("{prefix}0{}", file_suffix("genericaffine"));
        write_transform_file(log, ctx.notices, &name, &[xfm_parts(&to_write[0])]);
    } else {
        // ANTs: skip as many transforms as there are -r options, then name each file after
        // the next stage (ANTS-51). Corrected: every transform a stage computed, named after
        // that stage.
        let mut ants_files = Vec::new();
        let mut names = std::collections::VecDeque::from(names);
        let mut runs_out = false;
        for i in moving_initial_count..to_write.len() {
            match names.pop_front() {
                Some(kind) => ants_files.push((i, kind)),
                None => {
                    runs_out = true;
                    break;
                }
            }
        }
        let fixed_files: Vec<(usize, String)> = helper
            .origins
            .iter()
            .enumerate()
            .filter_map(|(i, o)| {
                o.map(|s| (i, helper.transforms[s].method.as_str().to_lowercase()))
            })
            .collect();
        let differs = runs_out || ants_files != fixed_files;
        let files = if ctx.notices.reproduce() {
            if differs {
                ctx.notices.add(
                    &larmorx_core::defects::ANTS_51,
                    "the transform files are named and chosen as in ANTs",
                );
            }
            ants_files
        } else {
            if differs {
                ctx.notices.add(
                    &larmorx_core::defects::ANTS_51,
                    "each transform a stage computed is written, named after that stage",
                );
            }
            fixed_files
        };
        for (i, kind) in &files {
            let name = format!("{prefix}{i}{}", file_suffix(kind));
            write_transform_file(log, ctx.notices, &name, &[xfm_parts(&to_write[*i])]);
        }
        if runs_out && ctx.notices.reproduce() {
            log.cerr(
                "larmorx: antsRegistration: ANTs runs out of transform names here and reads past \
                 the end of its list (a file name of garbage, then a crash); stopped (ANTS-51)\n",
            );
            return Err(1);
        }
    }
    Ok(0)
}

/// The command's description (`SetCommandDescription`).
const DESCRIPTION: &str = "This program is a user-level registration application meant to utilize classes in ITK v4.0 and later. The user can specify any number of \"stages\" where a stage consists of a transform; an image metric; and iterations, shrink factors, and smoothing sigmas for each level.  Note that explicitly setting the dimensionality, metric, transform, output, convergence, shrink-factors, and smoothing-sigmas parameters is mandatory.";

/// The options, as `antsRegistrationInitializeCommandLineOptions` declares them.
const OPTIONS: &[MenuOption] = &[
    MenuOption {
        long: "version",
        short: None,
        usage: &[],
        description: "Get Version Information.",
        defaults: &[],
    },
    MenuOption {
        long: "dimensionality",
        short: Some('d'),
        usage: &["2/3/4"],
        description: "This option forces the image to be treated as a specified-dimensional image.  If not specified, we try to infer the dimensionality from the input image.",
        defaults: &[],
    },
    MenuOption {
        long: "output",
        short: Some('o'),
        usage: &[
            "outputTransformPrefix",
            "[outputTransformPrefix,<outputWarpedImage>,<outputInverseWarpedImage>]",
        ],
        description: "Specify the output transform prefix (output format is .nii.gz ). Optionally, one can choose to warp the moving image to the fixed space and, if the inverse transform exists, one can also output the warped fixed image.  Note that only the images specified in the first metric call are warped.  Use antsApplyTransforms to warp other images using the resultant transform(s). When a composite transform is not specified, linear transforms are specified with a '.mat' suffix and displacement fields with a 'Warp.nii.gz' suffix (and 'InverseWarp.nii.gz', when applicable.  In addition, for velocity-based transforms, the full velocity field is written to file ('VelocityField.nii.gz') as long as the collapse transforms flag is turned off ('-z 0').",
        defaults: &[],
    },
    MenuOption {
        long: "save-state",
        short: Some('j'),
        usage: &["saveStateAsTransform"],
        description: "Specify the output file for the current state of the registration. The state file is written to an hdf5 composite file. It is specially usefull if we want to save the current state of a SyN registration to the disk, so we can load and restore that later to continue the next registration process directly started from the last saved state. The output file of this flag is the same as the write-composite-transform, unless the last transform is a SyN transform. In that case, the inverse displacement field of the SyN transform is also added to the output composite transform. Again notice that this file cannot be treated as a transform, and restore-state option must be used to load the written file by this flag.",
        defaults: &[],
    },
    MenuOption {
        long: "restore-state",
        short: Some('k'),
        usage: &["restoreStateAsATransform"],
        description: "Specify the initial state of the registration which get immediately used to directly initialize the registration process. The flag is mutually exclusive with other intialization flags.If this flag is used, none of the initial-moving-transform and initial-fixed-transform cannot be used.",
        defaults: &[],
    },
    MenuOption {
        long: "write-composite-transform",
        short: Some('a'),
        usage: &["1/(0)"],
        description: "Boolean specifying whether or not the composite transform (and its inverse, if it exists) should be written to an hdf5 composite file.  This is false by default so that only the transform for each stage is written to file.",
        defaults: &["0"],
    },
    MenuOption {
        long: "print-similarity-measure-interval",
        short: Some('p'),
        usage: &["<unsignedIntegerValue>"],
        description: "Prints out the CC similarity metric measure between the full-size input fixed and the transformed moving images at each iteration a value of 0 (the default) indicates that the full scale computation should not take placeany value greater than 0 represents the interval of full scale metric computation.",
        defaults: &["0"],
    },
    MenuOption {
        long: "write-interval-volumes",
        short: None,
        usage: &["<unsignedIntegerValue>"],
        description: "Writes out the output volume at each iteration. It helps to present the registration process as a short movie a value of 0 (the default) indicates that this option should not take placeany value greater than 0 represents the interval between the iterations which outputs are written to the disk.",
        defaults: &["0"],
    },
    MenuOption {
        long: "collapse-output-transforms",
        short: Some('z'),
        usage: &["(1)/0"],
        description: "Collapse output transforms. Specifically, enabling this option combines all adjacent transforms wherepossible.  All adjacent linear transforms are written to disk in the forman itk affine transform (called xxxGenericAffine.mat).  Similarly, all adjacent displacement field transforms are combined when written to disk (e.g. xxxWarp.nii.gz and xxxInverseWarp.nii.gz (if available)).Also, an output composite transform including the collapsed transforms is written to the disk (called outputCollapsed(Inverse)Composite).",
        defaults: &["1"],
    },
    MenuOption {
        long: "initialize-transforms-per-stage",
        short: Some('i'),
        usage: &["(1)/0"],
        description: "Initialize linear transforms from the previous stage. By enabling this option, the current linear stage transform is directly intialized from the previous stage's linear transform; this allows multiple linear stages to be run where each stage directly updates the estimated linear transform from the previous stage. (e.g. Translation -> Rigid -> Affine).",
        defaults: &["0"],
    },
    MenuOption {
        long: "interpolation",
        short: Some('n'),
        usage: &[
            "Linear",
            "NearestNeighbor",
            "MultiLabel[<sigma=imageSpacing>,<alpha=4.0>]",
            "Gaussian[<sigma=imageSpacing>,<alpha=1.0>]",
            "BSpline[<order=3>]",
            "CosineWindowedSinc",
            "WelchWindowedSinc",
            "HammingWindowedSinc",
            "LanczosWindowedSinc",
            "GenericLabel[<interpolator=Linear>]",
        ],
        description: "Several interpolation options are available in ITK. These have all been made available.  Currently the interpolator choice is only used to warp (and possibly inverse warp) the final output image(s).",
        defaults: &[],
    },
    MenuOption {
        long: "restrict-deformation",
        short: Some('g'),
        usage: &["PxQxR"],
        description: "This option allows the user to restrict the optimization of the displacement field, translation, rigid or affine transform on a per-component basis.  For example, if one wants to limit the deformation or rotation of 3-D volume to the first two dimensions, this is possible by specifying a weight vector of '1x1x0' for a deformation field or  '1x1x0x1x1x0' for a rigid transformation. Low-dimensional restriction only works if there are no preceding transformations.All stages up to and including the desired stage must have this option specified,even if they should not be restricted (in which case specify 1x1x1...)",
        defaults: &[],
    },
    MenuOption {
        long: "initial-fixed-transform",
        short: Some('q'),
        usage: &[
            "initialTransform",
            "[initialTransform,<useInverse>]",
            "[fixedImage,movingImage,initializationFeature]",
        ],
        description: "Specify the initial fixed transform(s) which get immediately incorporated into the composite transform.  The order of the transforms is stack-esque in that the last transform specified on the command line is the first to be applied.  In addition to initialization with ITK transforms, the user can perform an initial translation alignment by specifying the fixed and moving images and selecting an initialization feature.  These features include using the geometric center of the images (=0), the image intensities (=1), or the origin of the images (=2).",
        defaults: &[],
    },
    MenuOption {
        long: "initial-moving-transform",
        short: Some('r'),
        usage: &[
            "initialTransform",
            "[initialTransform,<useInverse>]",
            "[fixedImage,movingImage,initializationFeature]",
        ],
        description: "Specify the initial moving transform(s) which get immediately incorporated into the composite transform.  The order of the transforms is stack-esque in that the last transform specified on the command line is the first to be applied.  In addition to initialization with ITK transforms, the user can perform an initial translation alignment by specifying the fixed and moving images and selecting an initialization feature.  These features include using the geometric center of the images (=0), the image intensities (=1), or the origin of the images (=2).",
        defaults: &[],
    },
    MenuOption {
        long: "metric",
        short: Some('m'),
        usage: &[
            "CC[fixedImage,movingImage,metricWeight,radius,<samplingStrategy={None,Regular,Random}>,<samplingPercentage=[0,1]>,<useGradientFilter=false>]",
            "MI[fixedImage,movingImage,metricWeight,numberOfBins,<samplingStrategy={None,Regular,Random}>,<samplingPercentage=[0,1]>,<useGradientFilter=false>]",
            "Mattes[fixedImage,movingImage,metricWeight,numberOfBins,<samplingStrategy={None,Regular,Random}>,<samplingPercentage=[0,1]>,<useGradientFilter=false>]",
            "MeanSquares[fixedImage,movingImage,metricWeight,radius=NA,<samplingStrategy={None,Regular,Random}>,<samplingPercentage=[0,1]>,<useGradientFilter=false>]",
            "Demons[fixedImage,movingImage,metricWeight,radius=NA,<samplingStrategy={None,Regular,Random}>,<samplingPercentage=[0,1]>,<useGradientFilter=false>]",
            "GC[fixedImage,movingImage,metricWeight,radius=NA,<samplingStrategy={None,Regular,Random}>,<samplingPercentage=[0,1]>,<useGradientFilter=false>]",
            "ICP[fixedPointSet,movingPointSet,metricWeight,<samplingPercentage=[0,1]>,<boundaryPointsOnly=0>]",
            "PSE[fixedPointSet,movingPointSet,metricWeight,<samplingPercentage=[0,1]>,<boundaryPointsOnly=0>,<pointSetSigma=1>,<kNeighborhood=50>]",
            "JHCT[fixedPointSet,movingPointSet,metricWeight,<samplingPercentage=[0,1]>,<boundaryPointsOnly=0>,<pointSetSigma=1>,<kNeighborhood=50>,<alpha=1.1>,<useAnisotropicCovariances=1>]",
            "IGDM[fixedImage,movingImage,metricWeight,fixedMask,movingMask,<neighborhoodRadius=0x0>,<intensitySigma=0>,<distanceSigma=0>,<kNeighborhood=1>,<gradientSigma=1>]",
        ],
        description: "These image metrics are available--- CC:  ANTS neighborhood cross correlation, MI:  Mutual information, Demons: (Thirion), MeanSquares, and GC: Global Correlation. The \"metricWeight\" variable is used to modulate the per stage weighting of the metrics.  The metrics can also employ a sampling strategy defined by a sampling percentage. The sampling strategy defaults to 'None' (aka a dense sampling of one sample per voxel), otherwise it defines a point set over which to optimize the metric. The point set can be on a regular lattice or a random lattice of points slightly perturbed to minimize aliasing artifacts. samplingPercentage defines the fraction of points to select from the domain. useGradientFilter specifies whether a smoothingfilter is applied when estimating the metric gradient.In addition, three point set metrics are available:  Euclidean (ICP), Point-set expectation (PSE), and Jensen-Havrda-Charvet-Tsallis (JHCT).",
        defaults: &[],
    },
    MenuOption {
        long: "transform",
        short: Some('t'),
        usage: &[
            "Rigid[gradientStep]",
            "Affine[gradientStep]",
            "CompositeAffine[gradientStep]",
            "Similarity[gradientStep]",
            "Translation[gradientStep]",
            "BSpline[gradientStep,meshSizeAtBaseLevel]",
            "GaussianDisplacementField[gradientStep,updateFieldVarianceInVoxelSpace,totalFieldVarianceInVoxelSpace]",
            "BSplineDisplacementField[gradientStep,updateFieldMeshSizeAtBaseLevel,<totalFieldMeshSizeAtBaseLevel=0>,<splineOrder=3>]",
            "TimeVaryingVelocityField[gradientStep,numberOfTimeIndices,updateFieldVarianceInVoxelSpace,updateFieldTimeVariance,totalFieldVarianceInVoxelSpace,totalFieldTimeVariance]",
            "TimeVaryingBSplineVelocityField[gradientStep,velocityFieldMeshSize,<numberOfTimePointSamples=4>,<splineOrder=3>]",
            "SyN[gradientStep,<updateFieldVarianceInVoxelSpace=3>,<totalFieldVarianceInVoxelSpace=0>]",
            "BSplineSyN[gradientStep,updateFieldMeshSizeAtBaseLevel,<totalFieldMeshSizeAtBaseLevel=0>,<splineOrder=3>]",
            "Exponential[gradientStep,updateFieldVarianceInVoxelSpace,velocityFieldVarianceInVoxelSpace,<numberOfIntegrationSteps>]",
            "BSplineExponential[gradientStep,updateFieldMeshSizeAtBaseLevel,<velocityFieldMeshSizeAtBaseLevel=0>,<numberOfIntegrationSteps>,<splineOrder=3>]",
        ],
        description: "Several transform options are available.  The gradientStep or learningRate characterizes the gradient descent optimization and is scaled appropriately for each transform using the shift scales estimator.  Subsequent parameters are transform-specific and can be determined from the usage. For the B-spline transforms one can also specify the smoothing in terms of spline distance (i.e. knot spacing). ",
        defaults: &[],
    },
    MenuOption {
        long: "convergence",
        short: Some('c'),
        usage: &[
            "MxNxO",
            "[MxNxO,<convergenceThreshold=1e-6>,<convergenceWindowSize=10>]",
        ],
        description: "Convergence is determined from the number of iterations per level and is determined by fitting a line to the normalized energy profile of the last N iterations (where N is specified by the window size) and determining the slope which is then compared with the convergence threshold.",
        defaults: &[],
    },
    MenuOption {
        long: "smoothing-sigmas",
        short: Some('s'),
        usage: &["MxNxO..."],
        description: "Specify the sigma of gaussian smoothing at each level.  Units are given in terms of voxels ('vox') or physical spacing ('mm'). Example usage is '4x2x1mm' and '4x2x1vox' where no units implies voxel spacing.",
        defaults: &[],
    },
    MenuOption {
        long: "shrink-factors",
        short: Some('f'),
        usage: &["MxNxO..."],
        description: "Specify the shrink factor for the virtual domain (typically the fixed image) at each level.",
        defaults: &[],
    },
    MenuOption {
        long: "use-histogram-matching",
        short: Some('u'),
        usage: &[],
        description: "Histogram match the images before registration.",
        defaults: &[],
    },
    MenuOption {
        long: "winsorize-image-intensities",
        short: Some('w'),
        usage: &["[lowerQuantile,upperQuantile]"],
        description: "Winsorize data based on specified quantiles.",
        defaults: &[],
    },
    MenuOption {
        long: "masks",
        short: Some('x'),
        usage: &["[fixedImageMask,movingImageMask]"],
        description: "Image masks to limit voxels considered by the metric. Two options are allowed for mask specification:  1) Either the user specifies a single mask to be used for all stages or 2) the user specifies a mask for each stage.  With the latter one can select to which stages masks are applied by supplying valid file names.  If the file does not exist, a mask will not be used for that stage.  Note that we handle the fixed and moving masks separately to enforce this constraint.",
        defaults: &[],
    },
    MenuOption {
        long: "float",
        short: None,
        usage: &[],
        description: "Use 'float' instead of 'double' for computations.",
        defaults: &[],
    },
    MenuOption {
        long: "minc",
        short: None,
        usage: &[],
        description: "Use MINC file formats for transformations.",
        defaults: &["0"],
    },
    MenuOption {
        long: "random-seed",
        short: None,
        usage: &["seedValue"],
        description: "Use a fixed seed for random number generation. By default, the system clock is used to initialize the seeding. The fixed seed can be any nonzero int value.",
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
        description: "Print the help menu.  Will also print values used on the current command line call.",
        defaults: &[],
    },
];
