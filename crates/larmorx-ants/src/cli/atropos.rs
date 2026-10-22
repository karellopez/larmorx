// SPDX-License-Identifier: Apache-2.0
//! `Atropos` with its original arguments (ANTs v2.6.5, `Examples/Atropos.cxx`), through ANTs'
//! command-line parser: the options read, converted and checked in ANTs' order, with its
//! messages on its streams and its exit statuses; the images read where ANTs reads them; the
//! segmentation ([`crate::atropos`]); the outputs written in ANTs' order.
//!
//! As in ANTs:
//! - most errors are explained only with `--verbose 1`; larmorx always explains them
//!   (ANTS-45), on ANTs' stream;
//! - a value the parser cannot convert (`-c 1o`), and an ITK exception outside the
//!   segmentation's `try` (writing the outputs), abort ANTs; larmorx prints what ANTs' exception
//!   says and exits 1;
//! - option values are read newest first (`GetFunction(0)` is the last one given);
//! - `--use-random-seed 1` (or no `-r`) seeds ANTs' generator from the clock; larmorx uses
//!   121212 (D13 (b): no clock), `--use-random-seed 0` 19650218 in both;
//!   `ANTS_RANDOM_SEED` is not read by Atropos.
//!
//! `-v` prints ANTs' progress lines and the filter's own settings and models (`Print`), without
//! the ITK base classes' lines (addresses, times, thread settings); addresses print as `0x0`.

use std::io::Write;
use std::time::Instant;

use larmorx_core::defects::{self, EXIT_BAD_MODE, Notices, UpstreamDefects};
use larmorx_io::nifti::itk::ItkMeta;
use larmorx_registration::ItkException;

use super::cstd::format_g;
use super::menu::{MenuOption, print_menu};
use super::parser::{self, OptionValue, ParseFailure, Parsed};
use super::registration::Conv;
use crate::atropos::{
    Atropos, AtroposError, AtroposOptions, CLOCK_STAND_IN_SEED, FIXED_SEED, Formulation,
    Initialization, Likelihood, LikelihoodModel, OutlierHandling,
};
use crate::image::{AntsImage, FileStore, ImageStore, OutputImage, ReadError, read_with, write_with};

/// What ends a run early: the exit status (the messages are already printed).
type Exit = u8;

/// ANTs' streams, in the order they are written.
struct Log<'a> {
    out: &'a mut dyn Write,
    err: &'a mut dyn Write,
    verbose: bool,
}

impl Log<'_> {
    fn cout(&mut self, s: &str) {
        let _ = self.out.write_all(s.as_bytes());
    }
    fn cerr(&mut self, s: &str) {
        let _ = self.err.write_all(s.as_bytes());
    }
    /// A message ANTs prints only with `--verbose 1` (on `stream`); larmorx always prints it
    /// (ANTS-45), adding a note when ANTs would have been silent.
    fn silent(&mut self, to_err: bool, s: &str) {
        if to_err {
            self.cerr(s);
        } else {
            self.cout(s);
        }
        if !self.verbose {
            self.cerr("larmorx: Atropos: ANTs prints the line above only with --verbose 1 (ANTS-45)\n");
        }
    }
}

/// An ITK exception ANTs does not catch: ANTs aborts; larmorx prints what ANTs prints and
/// exits 1.
fn uncaught(log: &mut Log<'_>, e: &ItkException) -> Exit {
    log.cerr(&super::ai::terminate_text(e));
    log.cerr("larmorx: Atropos: ANTs aborts here (an uncaught exception); larmorx stops with exit 1\n");
    1
}

/// A case ANTs runs with undefined behaviour: larmorx refuses.
fn refuse(log: &mut Log<'_>, message: &str) -> Exit {
    log.cerr(&format!("larmorx: Atropos: {message}\n"));
    1
}

fn itk_exception(e: &AtroposError) -> ItkException {
    match e {
        AtroposError::Itk {
            file,
            line,
            class,
            description,
        } => ItkException::new(file, *line, class, description.clone()),
        AtroposError::Refused(m) => ItkException::new("", 0, "", m.clone()),
    }
}

/// `GetFunction(i)`: the `i`-th value counted from the last one given.
fn function<'p>(values: &'p [OptionValue], i: usize) -> Option<&'p OptionValue> {
    values.len().checked_sub(i + 1).map(|k| &values[k])
}

/// `GetParameter(i)`: the parameter, or "" past the end.
fn param(v: &OptionValue, i: usize) -> &str {
    v.parameters.get(i).map_or("", String::as_str)
}

/// Runs `Atropos` on files, in the D13 mode of `LARMORX_UPSTREAM_DEFECTS`; returns the exit
/// status.
pub fn main(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    match UpstreamDefects::from_env() {
        Ok(mode) => run(args, &mut FileStore, super::itk_threads(), mode, out, err),
        Err(e) => {
            let _ = writeln!(err, "larmorx: {e}");
            EXIT_BAD_MODE
        }
    }
}

/// Runs `Atropos` with images from `store`, `n_threads` threads (larmorx's result does not
/// depend on it) and the D13 mode `mode`.
pub fn run(
    args: &[String],
    store: &mut dyn ImageStore,
    n_threads: usize,
    mode: UpstreamDefects,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let mut notices = Notices::new(mode);
    let code = {
        let mut log = Log {
            out,
            err,
            verbose: false,
        };
        match atropos(args, store, n_threads, &mut log, &mut notices) {
            Ok(code) | Err(code) => code,
        }
    };
    notices.print(err);
    code
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

/// `Atropos(args)` (`Atropos.cxx:1592-1731`).
fn atropos(
    args: &[String],
    store: &mut dyn ImageStore,
    n_threads: usize,
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
    if args.is_empty() {
        print_menu(log.out, "Atropos", DESCRIPTION, OPTIONS, Some(&p), 5, false);
        return Err(1);
    }
    if let Some(h) = p.last("help")
        && Conv::bool(&h.name).map_err(|e| uncaught(log, &e))?
    {
        print_menu(log.out, "Atropos", DESCRIPTION, OPTIONS, Some(&p), 5, false);
        return Ok(0);
    }
    if let Some(h) = p.last("")
        && Conv::bool(&h.name).map_err(|e| uncaught(log, &e))?
    {
        print_menu(log.out, "Atropos", DESCRIPTION, OPTIONS, Some(&p), 5, true);
        return Ok(0);
    }
    let dimension = if let Some(d) = p.last("image-dimensionality") {
        Conv::u32(&d.name).map_err(|e| uncaught(log, &e))? as usize
    } else {
        let Some(first) = p.all("intensity-image").last() else {
            log.cerr("No input images were specified.  Specify an input image with the -a option\n");
            return Err(1);
        };
        let name = if first.parameters.is_empty() {
            first.name.clone()
        } else {
            first.parameters[0].clone()
        };
        // ANTs asks a new ImageIO for the dimension before it reads the header: NIfTI's answers
        // 3 for every file (ANTS-99). A file no reader recognizes is a null ImageIO: ANTs
        // crashes.
        let dims = match store.load(&name, n_threads, &mut Notices::new(notices.mode())) {
            Ok(image) => image.geometry.ndim,
            Err(_) => {
                return Err(refuse(
                    log,
                    &format!(
                        "{name}: no image reader recognizes the file, and ANTs dereferences the \
                         null reader (a crash)"
                    ),
                ));
            }
        };
        if dims != 3 {
            notices.add(
                &defects::ANTS_99,
                format!("{name} is {dims}-dimensional and segmented as such; ANTs segments it in 3D"),
            );
        }
        if notices.fix() { dims } else { 3 }
    };
    match dimension {
        2..=4 => segmentation(dimension, &p, store, n_threads, log, notices),
        _ => {
            log.cerr("Unsupported dimension\n");
            Err(1)
        }
    }
}

/// The verbose observer's line.
fn iteration_line(elapsed: u32, max: u32, posterior: f32, temperature: f32) -> String {
    format!(
        "  Iteration {elapsed} (of {max}): posterior probability = {} (annealing temperature = {})\n",
        format_g(f64::from(posterior)),
        format_g(f64::from(temperature))
    )
}

/// `NumericSeriesFileNames` with start 1 and end `count`: `snprintf(format, i)` for each `i`.
/// One integer conversion is supported (C's `%d`, `%i`, `%u`, `%x`, `%X`, `%o` with flags,
/// width, precision and the `l`, `ll`, `h`, `hh`, `z`, `j`, `t` lengths, and `%%`); any other
/// conversion, or a second one, reads arguments ANTs never passes (undefined behaviour).
pub fn series_names(format: &str, count: u32) -> Result<Vec<String>, String> {
    (1..=u64::from(count)).map(|i| c_format_one(format, i)).collect()
}

fn c_format_one(format: &str, value: u64) -> Result<String, String> {
    let bytes = format.as_bytes();
    let mut out = String::new();
    let mut i = 0;
    let mut used = false;
    let mut literal_start = 0;
    while i < bytes.len() {
        if bytes[i] != b'%' {
            i += 1;
            continue;
        }
        out.push_str(&format[literal_start..i]);
        i += 1;
        if i < bytes.len() && bytes[i] == b'%' {
            out.push('%');
            i += 1;
            literal_start = i;
            continue;
        }
        let mut left = false;
        let mut plus = false;
        let mut space = false;
        let mut alt = false;
        let mut zero = false;
        while i < bytes.len() {
            match bytes[i] {
                b'-' => left = true,
                b'+' => plus = true,
                b' ' => space = true,
                b'#' => alt = true,
                b'0' => zero = true,
                _ => break,
            }
            i += 1;
        }
        let mut width = 0usize;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            width = width * 10 + usize::from(bytes[i] - b'0');
            i += 1;
        }
        if i < bytes.len() && bytes[i] == b'*' {
            return Err(format!("'{format}': a '*' width reads an argument ANTs does not pass"));
        }
        let mut precision: Option<usize> = None;
        if i < bytes.len() && bytes[i] == b'.' {
            i += 1;
            let mut p = 0usize;
            if i < bytes.len() && bytes[i] == b'*' {
                return Err(format!("'{format}': a '*' precision reads an argument ANTs does not pass"));
            }
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                p = p * 10 + usize::from(bytes[i] - b'0');
                i += 1;
            }
            precision = Some(p);
        }
        // Length modifiers: the argument is an unsigned long; `int`-sized conversions read its
        // low 32 bits (x86-64 passes it in a register).
        let mut bits = 32u32;
        while i < bytes.len() {
            match bytes[i] {
                b'h' => bits = if bits == 16 { 8 } else { 16 },
                b'l' | b'z' | b'j' | b't' | b'q' => bits = 64,
                b'L' => bits = 64,
                _ => break,
            }
            i += 1;
        }
        if i >= bytes.len() {
            return Err(format!("'{format}': an incomplete conversion"));
        }
        let conv = bytes[i];
        i += 1;
        literal_start = i;
        if used {
            return Err(format!(
                "'{format}': a second conversion reads an argument ANTs does not pass"
            ));
        }
        used = true;
        let masked = match bits {
            8 => value & 0xff,
            16 => value & 0xffff,
            32 => value & 0xffff_ffff,
            _ => value,
        };
        let signed = match bits {
            8 => i64::from(masked as u8 as i8),
            16 => i64::from(masked as u16 as i16),
            32 => i64::from(masked as u32 as i32),
            _ => masked as i64,
        };
        let (mut digits, negative, prefix) = match conv {
            b'd' | b'i' => (signed.unsigned_abs().to_string(), signed < 0, ""),
            b'u' => (masked.to_string(), false, ""),
            b'x' => (format!("{masked:x}"), false, if alt && masked != 0 { "0x" } else { "" }),
            b'X' => (format!("{masked:X}"), false, if alt && masked != 0 { "0X" } else { "" }),
            b'o' => {
                let s = format!("{masked:o}");
                (if alt && !s.starts_with('0') { format!("0{s}") } else { s }, false, "")
            }
            other => {
                return Err(format!(
                    "'{format}': the conversion '%{}' does not take the integer ANTs passes",
                    other as char
                ));
            }
        };
        if let Some(p) = precision {
            if p == 0 && masked == 0 && matches!(conv, b'd' | b'i' | b'u' | b'x' | b'X') {
                digits.clear();
            }
            while digits.len() < p {
                digits.insert(0, '0');
            }
        }
        let sign = if negative {
            "-"
        } else if plus && matches!(conv, b'd' | b'i') {
            "+"
        } else if space && matches!(conv, b'd' | b'i') {
            " "
        } else {
            ""
        };
        let body_len = sign.len() + prefix.len() + digits.len();
        let mut field = String::new();
        if body_len < width {
            let pad = width - body_len;
            if left {
                field.push_str(sign);
                field.push_str(prefix);
                field.push_str(&digits);
                field.push_str(&" ".repeat(pad));
            } else if zero && precision.is_none() {
                field.push_str(sign);
                field.push_str(prefix);
                field.push_str(&"0".repeat(pad));
                field.push_str(&digits);
            } else {
                field.push_str(&" ".repeat(pad));
                field.push_str(sign);
                field.push_str(prefix);
                field.push_str(&digits);
            }
        } else {
            field.push_str(sign);
            field.push_str(prefix);
            field.push_str(&digits);
        }
        out.push_str(&field);
    }
    out.push_str(&format[literal_start..]);
    Ok(out)
}

/// The context the dimension-specific part reads from.
struct Ctx<'a, 'n> {
    store: &'a mut dyn ImageStore,
    n_threads: usize,
    notices: &'n mut Notices,
    dim: usize,
}

fn read_float(
    ctx: &mut Ctx<'_, '_>,
    name: &str,
) -> Result<AntsImage<f32>, ReadError> {
    read_with::<f32, _>(ctx.store, name, ctx.dim, ctx.n_threads, ctx.notices)
}

fn read_label(
    ctx: &mut Ctx<'_, '_>,
    name: &str,
) -> Result<AntsImage<u32>, ReadError> {
    read_with::<u32, _>(ctx.store, name, ctx.dim, ctx.n_threads, ctx.notices)
}

/// Whether two images are on one grid (Atropos assumes they are: its iterators walk them
/// together).
fn same_grid<T, U>(a: &AntsImage<T>, b: &AntsImage<U>) -> bool {
    a.geometry.size == b.geometry.size
}

/// `AtroposSegmentation<D>` (`Atropos.cxx:78-1191`).
fn segmentation(
    dim: usize,
    p: &Parsed,
    store: &mut dyn ImageStore,
    n_threads: usize,
    log: &mut Log<'_>,
    notices: &mut Notices,
) -> Result<Exit, Exit> {
    let fix = notices.fix();
    let conv_err = |log: &mut Log<'_>, e: ItkException| uncaught(log, &e);
    let mut verbose = false;
    if let Some(v) = p.last("verbose") {
        verbose = Conv::bool(&v.name).map_err(|e| conv_err(log, e))?;
    }
    log.verbose = verbose;
    if verbose {
        log.cout(&format!("\nRunning Atropos for {dim}-dimensional images.\n"));
    }
    let mut ctx = Ctx {
        store,
        n_threads,
        notices,
        dim,
    };
    let mut o = AtroposOptions::new(dim);
    // The seed when -r is not 0: the clock in ANTs (D13 (b)).
    o.seed = CLOCK_STAND_IN_SEED;
    o.upstream_defects = ctx.notices.mode();

    // --minimize-memory-usage (default "0").
    let memory = p.last("minimize-memory-usage").map_or("0", |v| v.name.as_str());
    o.minimize_memory = Conv::bool(memory).map_err(|e| conv_err(log, e))?;

    // --initialization
    let init_values = p.all("initialization");
    let Some(init) = function(init_values, 0) else {
        log.cerr("No initialization option specified.\n");
        return Err(1);
    };
    let init_description = OPTIONS[3].description;
    if init.parameters.is_empty() {
        log.silent(
            true,
            &format!("Incorrect initialization option specification.\n   {init_description}\n"),
        );
        return Err(1);
    }
    let classes = Conv::u32(param(init, 0)).map_err(|e| conv_err(log, e))?;
    o.classes = classes.max(2);
    let k = o.classes;
    let strategy = init.name.to_lowercase();
    let mut prior_images: Option<Vec<Vec<f32>>> = None;
    let mut prior_label: Option<AntsImage<u32>> = None;
    let mut first_image: Option<AntsImage<f32>> = None;
    match strategy.as_str() {
        "random" => o.initialization = Initialization::Random,
        "otsu" => o.initialization = Initialization::Otsu,
        "kmeans" => {
            let mut centers = None;
            if init.parameters.len() > 1 {
                let c = Conv::vec_f32(param(init, 1)).map_err(|e| conv_err(log, e))?;
                if c.len() != k as usize {
                    log.silent(
                        true,
                        "The cluster center vector size does not equal the specified number of classes.\n",
                    );
                    return Err(1);
                }
                centers = Some(c);
            }
            o.initialization = Initialization::KMeans { centers };
        }
        "priorprobabilityimages" => {
            if init.parameters.len() < 3 {
                log.silent(
                    true,
                    &format!("Incorrect initialization option specification.\n   {init_description}\n"),
                );
                return Err(1);
            }
            o.prior_weight = Conv::f32(param(init, 2))
                .map_err(|e| conv_err(log, e))?
                .clamp(0.0, 1.0e9);
            if init.parameters.len() > 3 {
                o.probability_threshold = clamp01(
                    Conv::f32(param(init, 3)).map_err(|e| conv_err(log, e))?,
                );
            }
            let filename = param(init, 1).to_string();
            let mut images = Vec::with_capacity(k as usize);
            if filename.contains('%') {
                let names = match series_names(&filename, k) {
                    Ok(n) => n,
                    Err(m) => return Err(refuse(log, &m)),
                };
                for name in names {
                    match read_float(&mut ctx, &name) {
                        Ok(img) => images.push(img),
                        Err(e) => {
                            if let Some(m) = e.ants_message() {
                                log.cerr(&format!("{m}\n"));
                            }
                            log.cout(&format!(
                                "Input prior probability image could not be read: {name}\n"
                            ));
                            return Err(1);
                        }
                    }
                }
            } else {
                match read_vector(&mut ctx, &filename) {
                    Ok(components) => {
                        if components.len() != k as usize {
                            log.silent(
                                true,
                                "The number of components does not match the number of classes.\n",
                            );
                            return Err(1);
                        }
                        // ANTs gives every class the caster's output, which holds the last
                        // component after the loop (ANTS-100), unless the sparse copies of
                        // --minimize-memory-usage are made as it goes.
                        let n = components.len();
                        let aliased = !o.minimize_memory && n > 1;
                        if aliased {
                            ctx.notices.add(
                                &defects::ANTS_100,
                                "the prior probability images of a vector image",
                            );
                            o.aliased_priors = !fix;
                        }
                        for comp in components.iter() {
                            let src = if aliased && !fix { &components[n - 1] } else { comp };
                            images.push(AntsImage {
                                data: src.data.clone(),
                                geometry: comp.geometry.clone(),
                                meta: ItkMeta::default(),
                            });
                        }
                    }
                    Err(e) => {
                        if let Some(m) = e.ants_message() {
                            log.cerr(&format!("{m}\n"));
                        }
                        log.cout("Input prior probability image could not be read.\n");
                        return Err(1);
                    }
                }
            }
            if init.parameters.len() > 3 {
                o.probability_threshold = clamp01(
                    Conv::f32(param(init, 3)).map_err(|e| conv_err(log, e))?,
                );
            }
            prior_images = Some(images.into_iter().map(|i| i.data).collect());
        }
        "priorlabelimage" => {
            if init.parameters.len() < 3 {
                log.silent(
                    true,
                    &format!("Incorrect initialization option specification.\n   {init_description}\n"),
                );
                return Err(1);
            }
            o.prior_weight = Conv::f32(param(init, 2))
                .map_err(|e| conv_err(log, e))?
                .clamp(0.0, 1.0e9);
            let filename = param(init, 1).to_string();
            match read_label(&mut ctx, &filename) {
                Ok(img) => prior_label = Some(img),
                Err(e) => {
                    if let Some(m) = e.ants_message() {
                        log.cerr(&format!("{m}\n"));
                    }
                    log.cout("Input prior label image could not be read.\n");
                    return Err(1);
                }
            }
        }
        _ => {
            log.silent(true, "Unrecognized initialization strategy request.\n");
            return Err(1);
        }
    }

    // --posterior-formulation
    if let Some(post) = function(p.all("posterior-formulation"), 0) {
        if !post.parameters.is_empty() {
            o.use_mixture_model_proportions =
                Conv::bool(param(post, 0)).map_err(|e| conv_err(log, e))?;
            let mut t = 1.0f32;
            if post.parameters.len() > 1 {
                t = Conv::f32(param(post, 1)).map_err(|e| conv_err(log, e))?;
                if t <= 0.0 {
                    log.silent(true, "Annealing temperature must be positive.\n");
                    return Err(1);
                }
            }
            o.initial_annealing_temperature = t.max(0.0);
            let mut rate = 1.0f32;
            if post.parameters.len() > 2 {
                rate = Conv::f32(param(post, 2)).map_err(|e| conv_err(log, e))?;
                if !(0.0..=1.0).contains(&rate) {
                    log.silent(true, "Annealing rate must be in the range [0, 1].\n");
                    return Err(1);
                }
            }
            o.annealing_rate = rate;
            if post.parameters.len() > 3 {
                let m = Conv::f32(param(post, 3)).map_err(|e| conv_err(log, e))?;
                o.minimum_annealing_temperature = m.max(0.0);
            }
        }
        match post.name.to_lowercase().as_str() {
            "socrates" => o.formulation = Formulation::Socrates,
            "plato" => o.formulation = Formulation::Plato,
            "aristotle" => o.formulation = Formulation::Aristotle,
            "sigmoid" => o.formulation = Formulation::Sigmoid,
            other => {
                // ANTs keeps Socrates for any other name (ANTS-101).
                ctx.notices.add(
                    &defects::ANTS_101,
                    format!("the posterior formulation '{}' is not one of Atropos' four", post.name),
                );
                if fix {
                    log.cerr(&format!(
                        "larmorx: Atropos: unknown posterior formulation '{other}': use Socrates, \
                         Plato, Aristotle or Sigmoid (ANTs runs Socrates)\n"
                    ));
                    return Err(1);
                }
            }
        }
    }

    // --convergence
    if let Some(c) = function(p.all("convergence"), 0) {
        if c.parameters.is_empty() {
            o.max_iterations = Conv::u32(&c.name).map_err(|e| conv_err(log, e))?;
        }
        if !c.parameters.is_empty() {
            o.max_iterations = Conv::u32(param(c, 0)).map_err(|e| conv_err(log, e))?;
        }
        if c.parameters.len() > 1 {
            o.convergence_threshold = Conv::f32(param(c, 1)).map_err(|e| conv_err(log, e))?;
        }
    }

    // --mask-image (required)
    let mask: AntsImage<u32> = if let Some(m) = function(p.all("mask-image"), 0) {
        let name = m.name.clone();
        match read_label(&mut ctx, &name) {
            Ok(img) => img,
            Err(e) => {
                if let Some(msg) = e.ants_message() {
                    log.cerr(&format!("{msg}\n"));
                }
                log.cout("Input mask image could not be read.\n");
                return Err(1);
            }
        }
    } else {
        log.silent(
            true,
            "An image mask is required.  Specify a mask image with the -x option.\n",
        );
        return Err(1);
    };
    // ANTS-105: with --minimize-memory-usage, the check below rebuilds each prior on the first
    // intensity image, which is not read yet: ANTs crashes.
    if o.minimize_memory && prior_images.is_some() {
        ctx.notices.add(
            &defects::ANTS_105,
            "the prior probability images are checked against the mask; ANTs crashes here",
        );
        if !fix {
            return Err(refuse(
                log,
                "--minimize-memory-usage with prior probability images and a mask: ANTs reads a                  null intensity image here and crashes (ANTS-105)",
            ));
        }
    }
    // The checks of the priors against the mask (warnings only with -v).
    if let Some(label) = &prior_label {
        if same_grid(&mask, label) {
            let uncovered = mask.data.iter().zip(&label.data).any(|(&m, &l)| m != 0 && l == 0);
            if uncovered && verbose {
                log.cout("\nWarning: the labels in the the prior label image do not encompass the entire mask region.  As a result each unlabeled voxel will be initially assigned a random label.  The user might want to consider various alternative strategies like assigning an additional \"background\" label to the unlabeled voxels or propagating the labels within the mask region.\n\n");
            }
        }
    }
    if let Some(priors) = &prior_images
        && priors.iter().all(|p| p.len() == mask.data.len())
    {
        let threshold = o.probability_threshold;
        let mut below = false;
        for v in 0..mask.data.len() {
            if mask.data[v] != 0 {
                let mut s = 0.0f32;
                for p in priors {
                    let x = p[v];
                    s += if o.minimize_memory && x <= threshold { 0.0 } else { x };
                }
                if s < threshold {
                    below = true;
                    break;
                }
            }
        }
        if below && verbose {
            log.cout("\nWarning: the sum of the priors from the the prior probability images are less than the probability threshold within the mask region.  As a result each zero probability voxel will be initially assigned a random label.  The user might want to consider various alternative strategies like assigning an additional \"background\" label to the zero probability voxels or propagating the probabilities within the mask region.\n\n");
        }
    }

    // --bspline
    if let Some(b) = function(p.all("bspline"), 0) {
        if !b.parameters.is_empty() {
            let levels = Conv::vec_u32(param(b, 0)).map_err(|e| conv_err(log, e))?;
            if levels.len() == 1 {
                o.levels = vec![levels[0]; dim];
            } else if levels.len() == dim {
                o.levels = levels;
            } else {
                log.silent(true, "Incorrect number of levels\n");
                return Err(1);
            }
        }
        if b.parameters.len() > 2 {
            o.spline_order = Conv::u32(param(b, 2)).map_err(|e| conv_err(log, e))?;
        }
        if b.parameters.len() > 1 {
            let mesh = Conv::vec_u32(param(b, 1)).map_err(|e| conv_err(log, e))?;
            if mesh.len() == 1 {
                o.control_points = vec![mesh[0].wrapping_add(o.spline_order); dim];
            } else if mesh.len() == dim {
                o.control_points = mesh.iter().map(|&m| m.wrapping_add(o.spline_order)).collect();
            } else {
                log.silent(true, "Incorrect mesh resolution\n");
                return Err(1);
            }
        }
    }

    // --label-propagation
    let labels = p.all("label-propagation");
    if !labels.is_empty() {
        let read_pair = |v: &OptionValue, log: &mut Log<'_>| -> Result<(f32, f32), Exit> {
            let lambda = Conv::f32(param(v, 0)).map_err(|e| uncaught(log, &e))?;
            let mut boundary = 1.0f32;
            if v.parameters.len() > 1 {
                boundary = Conv::f32(param(v, 1)).map_err(|e| uncaught(log, &e))?;
                if boundary < 0.0 {
                    boundary = 0.0;
                }
                if boundary > 1.0 {
                    boundary = 1.0;
                }
            }
            Ok((lambda, boundary))
        };
        if labels.len() == 1 && labels[0].name.is_empty() {
            let pair = read_pair(&labels[0], log)?;
            for n in 1..=k {
                o.label_parameters.insert(n, pair);
            }
        } else {
            for i in 0..labels.len() {
                let v = function(labels, i).expect("value");
                let pair = read_pair(v, log)?;
                let which = Conv::u32(&v.name).map_err(|e| conv_err(log, e))?;
                o.label_parameters.insert(which, pair);
            }
        }
    }

    // --intensity-image
    let image_values = p.all("intensity-image");
    let mut intensities: Vec<AntsImage<f32>> = Vec::new();
    if image_values.is_empty() {
        log.silent(
            true,
            "No input images were specified.  Specify an input image with the -a option.\n",
        );
        return Err(1);
    }
    let count_images = image_values.len();
    let mut weights_given = Vec::new();
    for n in (0..count_images).rev() {
        let v = function(image_values, n).expect("value");
        let name = if v.parameters.is_empty() {
            v.name.clone()
        } else {
            v.parameters[0].clone()
        };
        match read_float(&mut ctx, &name) {
            Ok(img) => intensities.push(img),
            Err(e) => {
                if let Some(m) = e.ants_message() {
                    log.cerr(&format!("{m}\n"));
                }
                log.cout("Input intensity image could not be read.\n");
                return Err(1);
            }
        }
        let count = intensities.len() - 1;
        // ANTs takes the weight from GetFunction(count), the count-th value from the end
        // (ANTS-93); the image's own value is GetFunction(n).
        let ants = function(image_values, count).expect("value");
        let own = v;
        let weight_of = |w: &OptionValue, log: &mut Log<'_>| -> Result<f32, Exit> {
            if w.parameters.len() > 1 {
                Conv::f32(param(w, 1)).map_err(|e| uncaught(log, &e))
            } else {
                Ok(0.0)
            }
        };
        let w_ants = weight_of(ants, log)?;
        let w_own = weight_of(own, log)?;
        weights_given.push((w_ants, w_own));
    }
    for (w_ants, w_own) in &weights_given {
        if w_ants != w_own {
            ctx.notices.add(
                &defects::ANTS_93,
                "the adaptive smoothing weights of the intensity images",
            );
        }
    }
    o.adaptive_smoothing = weights_given
        .iter()
        .map(|&(a, own)| (if fix { own } else { a }).clamp(0.0, 1.0))
        .collect();
    let first_grid = intensities[0].geometry.clone();
    first_image.get_or_insert_with(|| intensities[0].clone());

    // --mrf
    if let Some(m) = function(p.all("mrf"), 0) {
        if !m.parameters.is_empty() {
            let name = param(m, 0).to_string();
            // ANTs first tries to read the value as an image (itk::ImageFileReader); a value
            // that is not an image file is the smoothing factor.
            let mut probe = Notices::new(ctx.notices.mode());
            let coefficient = if name.contains('.') && !is_number(&name) {
                ctx.store.load(&name, ctx.n_threads, &mut probe).ok()
            } else {
                None
            };
            match coefficient {
                Some(_) => match read_float(&mut ctx, &name) {
                    Ok(img) => o.mrf_coefficients = Some(img.data),
                    Err(_) => {
                        o.mrf_smoothing = Conv::f32(&name).map_err(|e| conv_err(log, e))?.max(0.0);
                    }
                },
                None => {
                    o.mrf_smoothing = Conv::f32(&name).map_err(|e| conv_err(log, e))?.max(0.0);
                }
            }
        }
        if m.parameters.len() > 1 {
            let r = Conv::vec_u32(param(m, 1)).map_err(|e| conv_err(log, e))?;
            if r.len() == 1 {
                o.mrf_radius = vec![r[0]; dim];
            } else if r.len() == dim {
                o.mrf_radius = r;
            } else {
                log.silent(
                    true,
                    "MRF radius size needs to be equal to the image dimension.\n",
                );
                return Err(1);
            }
        }
    }

    // --icm
    let icm_values = p.all("icm");
    let mut icm_code_name: Option<String> = None;
    if let Some(icm) = function(icm_values, 0) {
        if icm_values.len() == 1 {
            // ANTs converts the value's name, which is empty in the documented bracketed form
            // `--icm [1,1]`: the conversion throws and ANTs aborts (ANTS-102).
            if icm.name.is_empty() && !icm.parameters.is_empty() {
                ctx.notices.add(
                    &defects::ANTS_102,
                    "--icm with brackets is read as documented; ANTs aborts on it",
                );
                if !fix {
                    let e = Conv::bool(&icm.name).expect_err("an empty name does not convert");
                    return Err(uncaught(log, &e));
                }
            } else {
                o.asynchronous = Conv::bool(&icm.name).map_err(|e| conv_err(log, e))?;
            }
        }
        if !icm.parameters.is_empty() {
            o.asynchronous = Conv::bool(param(icm, 0)).map_err(|e| conv_err(log, e))?;
        }
        if icm.parameters.len() > 1 {
            o.max_icm_iterations = Conv::u32(param(icm, 1)).map_err(|e| conv_err(log, e))?;
        }
        if icm.parameters.len() > 2 {
            icm_code_name = Some(param(icm, 2).to_string());
        }
    }

    // --use-random-seed
    if let Some(r) = function(p.all("use-random-seed"), 0) {
        let random = Conv::bool(&r.name).map_err(|e| conv_err(log, e))?;
        if !random {
            o.seed = FIXED_SEED;
        }
    }

    // --use-euclidean-distance (default "0")
    let euclid = p.last("use-euclidean-distance").map_or("0", |v| v.name.as_str());
    o.euclidean_distance = Conv::bool(euclid).map_err(|e| conv_err(log, e))?;

    // --likelihood-model
    if let Some(l) = function(p.all("likelihood-model"), 0) {
        let tensor_images = dim * (dim + 1) / 2;
        let check_tensor = |log: &mut Log<'_>| -> Result<(), Exit> {
            if intensities.len() != tensor_images {
                log.silent(
                    true,
                    " Expect images in upper triangular order \n xx xy xz yy yz zz \nIncorrect number of intensity images specified.\n",
                );
                return Err(1);
            }
            Ok(())
        };
        o.likelihood = match l.name.to_lowercase().as_str() {
            "gaussian" => LikelihoodModel::Gaussian,
            "manifoldparzenwindows" => {
                let mut rs = 1.0f32;
                let mut ek = 50u32;
                let mut ck = 0u32;
                let mut ks = 1.0f32;
                if !l.parameters.is_empty() {
                    rs = Conv::f32(param(l, 0)).map_err(|e| conv_err(log, e))?;
                }
                if l.parameters.len() > 1 {
                    ek = Conv::u32(param(l, 1)).map_err(|e| conv_err(log, e))?;
                }
                if l.parameters.len() > 2 {
                    ck = Conv::u32(param(l, 2)).map_err(|e| conv_err(log, e))?;
                }
                if l.parameters.len() > 3 {
                    ks = Conv::f32(param(l, 3)).map_err(|e| conv_err(log, e))?;
                }
                LikelihoodModel::ManifoldParzenWindows {
                    regularization_sigma: rs,
                    evaluation_k: ek,
                    covariance_k: ck,
                    kernel_sigma: ks,
                }
            }
            "histogramparzenwindows" => {
                let mut sigma = 1.0f32;
                let mut bins = 32u32;
                if !l.parameters.is_empty() {
                    sigma = Conv::f32(param(l, 0)).map_err(|e| conv_err(log, e))?;
                }
                if l.parameters.len() > 1 {
                    bins = Conv::u32(param(l, 1)).map_err(|e| conv_err(log, e))?;
                }
                LikelihoodModel::HistogramParzenWindows { sigma, bins }
            }
            "jointshapeandorientationprobability" => {
                check_tensor(log)?;
                let mut ss = 2.0f32;
                let mut sb = 64u32;
                let mut os = 1.0f32;
                let mut ob = 32u32;
                if !l.parameters.is_empty() {
                    ss = Conv::f32(param(l, 0)).map_err(|e| conv_err(log, e))?;
                }
                if l.parameters.len() > 1 {
                    sb = Conv::u32(param(l, 1)).map_err(|e| conv_err(log, e))?;
                }
                if l.parameters.len() > 2 {
                    os = Conv::f32(param(l, 2)).map_err(|e| conv_err(log, e))?;
                }
                if l.parameters.len() > 3 {
                    ob = Conv::u32(param(l, 3)).map_err(|e| conv_err(log, e))?;
                }
                LikelihoodModel::JointShapeAndOrientationProbability {
                    shape_sigma: ss,
                    shape_bins: sb,
                    orientation_sigma: os,
                    orientation_bins: ob,
                }
            }
            "logeuclideangaussian" => {
                check_tensor(log)?;
                LikelihoodModel::LogEuclideanGaussian
            }
            _ => {
                log.silent(true, "Unrecognized likelihood model request.\n");
                return Err(1);
            }
        };
    }

    // --partial-volume-label-set and --use-partial-volume-likelihoods
    let pv_values = p.all("partial-volume-label-set");
    if !pv_values.is_empty() {
        for n in (0..pv_values.len()).rev() {
            let v = function(pv_values, n).expect("value");
            let set = Conv::vec_u32(&v.name).map_err(|e| conv_err(log, e))?;
            if set.len() != 2 {
                log.silent(
                    true,
                    "Error:  Currently Atropos only supports partial volume label sets of size equal to 2.\n",
                );
                return Err(1);
            }
            // AddPartialVolumeLabelSet: warnings for labels out of range and duplicates.
            if let Some(bad) = set.iter().find(|&&l| l < 1 || l > k) {
                log.cerr(&format!(
                    "WARNING: In {}, line 371\nAtroposSegmentationImageFilter (0x0): The label {bad} is outside the specified range of the specified tissue class labels.\n\n",
                    "ImageSegmentation/antsAtroposSegmentationImageFilter.hxx"
                ));
            } else if o
                .partial_volume_sets
                .iter()
                .any(|s| s.len() == set.len() && s.iter().zip(&set).all(|(a, b)| a == b))
            {
                log.cerr("WARNING: In ImageSegmentation/antsAtroposSegmentationImageFilter.hxx, line 396\nAtroposSegmentationImageFilter (0x0): Duplicate label set.\n\n");
            } else if set[0] == set[1] {
                log.cerr(&format!(
                    "WARNING: In ImageSegmentation/antsAtroposSegmentationImageFilter.hxx, line 405\nAtroposSegmentationImageFilter (0x0): Duplicate label {}\n\n",
                    set[0]
                ));
            } else {
                o.partial_volume_sets.push(set);
            }
            o.partial_volume_functions += 1;
        }
        let use_pv = p
            .last("use-partial-volume-likelihoods")
            .map(|v| v.name.to_lowercase());
        o.use_partial_volume_likelihoods =
            matches!(use_pv.as_deref(), Some("true") | Some("1"));
    }

    // --winsorize-outliers
    if let Some(w) = function(p.all("winsorize-outliers"), 0) {
        match w.name.to_lowercase().as_str() {
            "boxplot" => {
                let mut lower = 0.25f64;
                let mut upper = 0.75f64;
                let mut whisker = 1.5f64;
                if !w.parameters.is_empty() {
                    lower = f64::from(Conv::f32(param(w, 0)).map_err(|e| conv_err(log, e))?);
                }
                if w.parameters.len() > 1 {
                    upper = f64::from(Conv::f32(param(w, 1)).map_err(|e| conv_err(log, e))?);
                }
                if w.parameters.len() > 2 {
                    whisker = f64::from(Conv::f32(param(w, 2)).map_err(|e| conv_err(log, e))?);
                }
                o.outliers = Some(OutlierHandling::BoxPlot {
                    lower,
                    upper,
                    whisker,
                });
            }
            "grubbsrosner" => {
                let mut significance = 0.05f64;
                let mut winsorizing = 0.10f64;
                if !w.parameters.is_empty() {
                    significance =
                        f64::from(Conv::f32(param(w, 0)).map_err(|e| conv_err(log, e))?);
                }
                if w.parameters.len() > 1 {
                    winsorizing =
                        f64::from(Conv::f32(param(w, 1)).map_err(|e| conv_err(log, e))?);
                }
                o.outliers = Some(OutlierHandling::GrubbsRosner {
                    significance,
                    winsorizing,
                });
            }
            _ => {
                log.silent(true, "Unrecognized outlier handling strategy request.\n");
                return Err(1);
            }
        }
    }

    // The images must share the first image's grid (Atropos walks them together; a smaller
    // one is read past its end).
    let n = intensities[0].data.len();
    let grid_ok = intensities.iter().all(|i| i.data.len() == n)
        && mask.data.len() == n
        && prior_images.as_ref().is_none_or(|v| v.iter().all(|p| p.len() == n))
        && prior_label.as_ref().is_none_or(|l| l.data.len() == n)
        && o.mrf_coefficients.as_ref().is_none_or(|c| c.len() == n);
    if !grid_ok {
        return Err(refuse(
            log,
            "the images do not all have the first intensity image's size (ANTs reads past the \
             smaller ones)",
        ));
    }
    if let Some(priors) = prior_images.take() {
        o.initialization = Initialization::PriorProbabilityImages { images: priors };
    }
    if let Some(label) = &prior_label {
        o.initialization = Initialization::PriorLabelImage {
            image: label.data.clone(),
        };
    }
    if o.partial_volume_classes() > 0
        && o.initialization.uses_priors()
        && o.adaptive_smoothing.iter().any(|&w| w > 0.0)
    {
        return Err(refuse(
            log,
            "adaptive smoothing with partial-volume classes and priors reads past ANTs' list of \
             control-point lattices",
        ));
    }

    // Update
    let start = Instant::now();
    if verbose {
        log.cout("\nProgress: \n");
    }
    let sizes = first_grid.size.clone();
    let spacing = first_grid.spacing.clone();
    let image_refs: Vec<&[f32]> = intensities.iter().map(|i| i.data.as_slice()).collect();
    let origin = first_grid.origin.clone();
    let mut filter = match Atropos::new(
        &o,
        &sizes,
        &spacing,
        &origin,
        image_refs,
        Some(&mask.data),
        n_threads,
    )
    {
        Ok(f) => f,
        Err(e) => return Err(refuse(log, &e.to_string())),
    };
    let result = filter.run();
    for w in &filter.warnings {
        log.cerr(&format!(
            "WARNING: In ImageSegmentation/antsGaussianListSampleFunction.hxx, line 105\nGaussianListSampleFunction (0x0): {w}\n\n"
        ));
    }
    filter.warnings.clear();
    if verbose {
        for it in &filter.iterations {
            log.cout(&iteration_line(
                it.elapsed,
                o.max_iterations,
                it.posterior_probability,
                it.annealing_temperature,
            ));
        }
    }
    if let Err(e) = result {
        ctx.notices.extend(filter.notices.as_slice().iter().cloned());
        return Err(match e {
            AtroposError::Refused(m) => refuse(log, &m),
            e => {
                if verbose {
                    log.cerr(&format!("{}\n", itk_exception(&e).print()));
                } else {
                    log.cerr(&format!(
                        "larmorx: Atropos: {e} (ANTs exits 1 and prints this only with --verbose 1, ANTS-45)\n"
                    ));
                }
                1
            }
        });
    }

    // Outputs
    let geometry = first_grid.clone();
    let float_image = |data: Vec<f32>| AntsImage {
        data,
        geometry: geometry.clone(),
        meta: ItkMeta::default(),
    };
    let label_image = |data: Vec<u32>| AntsImage {
        data,
        geometry: geometry.clone(),
        meta: ItkMeta::default(),
    };
    let write_plain = |ctx: &mut Ctx<'_, '_>, log: &mut Log<'_>, name: &str, image: OutputImage| -> Result<(), Exit> {
        ctx.store.save(name, &image, ctx.n_threads).map_err(|m| {
            let e = ItkException::new(
                "Modules/IO/ImageBase/include/itkImageFileWriter.hxx",
                0,
                "ImageFileWriter",
                m,
            );
            uncaught(log, &e)
        })
    };
    if let Some(name) = &icm_code_name
        && o.asynchronous
        && let Some(codes) = filter.icm_codes()
    {
        let img = label_image(codes.to_vec());
        write_plain(&mut ctx, log, name, OutputImage::U32(img))?;
    }
    if verbose {
        log.cout("\nWriting output:\n");
    }
    if let Some(out) = function(p.all("output"), 0) {
        let classified = label_image(filter.labels.clone());
        let name = if out.parameters.is_empty() {
            out.name.clone()
        } else {
            param(out, 0).to_string()
        };
        match write_with(ctx.store, &name, OutputImage::U32(classified), ctx.n_threads, ctx.notices) {
            Ok(_) => {}
            Err(m) => {
                let e = ItkException::new(
                    "Modules/IO/ImageBase/include/itkImageFileWriter.hxx",
                    0,
                    "ImageFileWriter",
                    m,
                );
                return Err(uncaught(log, &e));
            }
        }
        let classes = filter.options.classes;
        if out.parameters.len() > 1 {
            let names = series_names(param(out, 1), classes).map_err(|m| refuse(log, &m))?;
            for (i, name) in names.iter().enumerate() {
                if verbose {
                    log.cout(&format!("  Writing posterior image (class {})\n", i + 1));
                }
                let mut post = filter.posterior(i).map_err(|e| uncaught(log, &itk_exception(&e)))?;
                for (pv, &m) in post.iter_mut().zip(&mask.data) {
                    if m == 0 {
                        *pv = 0.0;
                    }
                }
                write_plain(&mut ctx, log, name, OutputImage::F32(float_image(post)))?;
            }
        }
        if out.parameters.len() > 2 {
            let names = series_names(param(out, 2), classes).map_err(|m| refuse(log, &m))?;
            for i in 0..classes as usize {
                if verbose {
                    log.cout(&format!("  Writing likelihood image (class {})\n", i + 1));
                }
                let l = filter.likelihood(i).map_err(|e| uncaught(log, &itk_exception(&e)))?;
                write_plain(&mut ctx, log, &names[i], OutputImage::F32(float_image(l)))?;
            }
        }
        if out.parameters.len() > 3 {
            let names = series_names(param(out, 3), classes).map_err(|m| refuse(log, &m))?;
            for i in 0..classes as usize {
                if filter.has_prior(i) || filter.has_prior_label() {
                    if verbose {
                        log.cout(&format!("  Writing distance image (class {})\n", i + 1));
                    }
                    let d = filter
                        .distance_prior(i)
                        .map_err(|e| uncaught(log, &itk_exception(&e)))?
                        .unwrap_or_else(|| vec![0.0; n]);
                    write_plain(&mut ctx, log, &names[i], OutputImage::F32(float_image(d)))?;
                }
            }
        }
        if out.parameters.len() > 4 {
            let names = series_names(param(out, 4), classes).map_err(|m| refuse(log, &m))?;
            if o.adaptive_smoothing.first().is_some_and(|&w| w > 0.0) {
                for i in 0..classes as usize {
                    if filter.has_prior(i) || filter.has_prior_label() {
                        if verbose {
                            log.cout(&format!("  Writing B-spline image (class {})\n", i + 1));
                        }
                        let s = filter
                            .smooth_intensity(0, i)
                            .map_err(|e| uncaught(log, &itk_exception(&e)))?;
                        write_plain(&mut ctx, log, &names[i], OutputImage::F32(float_image(s)))?;
                    }
                }
            }
        }
    }
    if verbose {
        log.cout("\n");
        log.cout(&print_filter(&filter));
        log.cout(&format!(
            "Elapsed time: {}\n",
            format_g(start.elapsed().as_secs_f64())
        ));
    }
    for w in &filter.warnings {
        log.cerr(&format!(
            "WARNING: In ImageSegmentation/antsGaussianListSampleFunction.hxx, line 105\nGaussianListSampleFunction (0x0): {w}\n\n"
        ));
    }
    ctx.notices.extend(filter.notices.as_slice().iter().cloned());
    drop(first_image);
    Ok(0)
}

/// `SetProbabilityThreshold`'s clamp to `[0, 1]`.
fn clamp01(v: f32) -> f32 {
    if v < 0.0 {
        0.0
    } else if v > 1.0 {
        1.0
    } else {
        v
    }
}

/// Whether `s` reads entirely as a number (then ANTs' attempt to read it as an image fails).
fn is_number(s: &str) -> bool {
    super::cstd::parser_convert_f64(s).is_some()
}

/// `ReadImage<VectorImage<float, D>>`: the components of a vector image (a NIfTI file with
/// `dim[5]` components), each as a scalar image.
fn read_vector(ctx: &mut Ctx<'_, '_>, name: &str) -> Result<Vec<AntsImage<f32>>, ReadError> {
    crate::image::read_vector_components(ctx.store, name, ctx.dim, ctx.n_threads, ctx.notices)
}

/// `segmenter->Print(std::cout, 2)`: the filter's own settings and models (`PrintSelf`), the
/// ITK base classes' lines left out.
fn print_filter(f: &Atropos<'_>) -> String {
    let o = f.options;
    let i = "    ";
    let mut s = String::from("  AtroposSegmentationImageFilter (0x0)\n");
    s += &format!("{i}Maximum number of iterations: {}\n", o.max_iterations);
    s += &format!(
        "{i}Convergence threshold: {}\n",
        format_g(f64::from(o.convergence_threshold))
    );
    s += &format!("{i}Number of tissue classes: {}\n", o.classes);
    s += &format!(
        "{i}Number of partial volume classes: {}\n",
        o.partial_volume_classes()
    );
    s += &format!("{i}Minimize memory usage:");
    let prior_probability = matches!(o.initialization, Initialization::PriorProbabilityImages { .. });
    if o.minimize_memory {
        s += " true";
        if prior_probability {
            s += &format!(
                " (prior probability threshold = {})\n",
                format_g(f64::from(o.probability_threshold))
            );
        }
        s += "\n";
    } else {
        s += " false\n";
    }
    s += &format!("{i}Initialization strategy: ");
    let label_params = |s: &mut String| {
        for (label, (lambda, bp)) in &o.label_parameters {
            *s += &format!(
                "{i}    Class {}: lambda = {}, boundary probability = {}\n",
                format_g(f64::from(*label as f32)),
                format_g(f64::from(*lambda)),
                format_g(f64::from(*bp))
            );
        }
    };
    let euclid = if o.euclidean_distance { " true" } else { " false" };
    match &o.initialization {
        Initialization::Random => s += "Random\n",
        Initialization::KMeans { .. } => s += "K means clustering\n",
        Initialization::Otsu => s += "Otsu thresholding\n",
        Initialization::PriorProbabilityImages { .. } => {
            s += "Prior probability images\n";
            s += &format!("{i}  Use Euclidean distance for prior labels:{euclid}\n");
            if !o.label_parameters.is_empty() {
                s += &format!("{i}  Specified prior label parameters:\n");
                label_params(&mut s);
            }
        }
        Initialization::PriorLabelImage { .. } => {
            s += "Prior label image\n";
            s += &format!("{i}  Use Euclidean distance for prior labels:{euclid}\n");
            s += &format!("{i}  Specified prior label parameters:\n");
            label_params(&mut s);
        }
    }
    s += &format!(
        "{i}Posterior probability formulation: {}\n",
        match o.formulation {
            Formulation::Socrates => "Socrates",
            Formulation::Plato => "Plato",
            Formulation::Aristotle => "Aristotle",
            Formulation::Sigmoid => "Sigmoid",
        }
    );
    s += &format!(
        "{i}  initial annealing temperature = {}\n",
        format_g(f64::from(o.initial_annealing_temperature))
    );
    s += &format!(
        "{i}  annealing rate = {}\n",
        format_g(f64::from(o.annealing_rate))
    );
    s += &format!(
        "{i}  minimum annealing temperature = {}\n",
        format_g(f64::from(o.minimum_annealing_temperature))
    );
    s += &format!("{i}MRF parameters\n");
    if o.mrf_coefficients.is_some() {
        s += &format!("{i}  Using MRF coefficient image\n");
    } else {
        s += &format!(
            "{i}  MRF smoothing factor = {}\n",
            format_g(f64::from(o.mrf_smoothing))
        );
    }
    s += &format!("{i}  MRF radius = {}\n", fixed_array(&o.mrf_radius));
    if o.asynchronous {
        s += &format!("{i}Use asynchronous updating of the labels.\n");
        s += &format!("{i}  ICM parameters\n");
        s += &format!("{i}    maximum ICM code = {}\n", f.maximum_icm_code);
        s += &format!(
            "{i}    maximum number of ICM iterations = {}\n",
            o.max_icm_iterations
        );
    } else {
        s += &format!("{i}Use synchronous updating of the labels.\n");
    }
    match o.outliers {
        Some(h) => {
            s += &format!("{i}Outlier handling \n");
            s += &crate::atropos::outliers::print(&h, "      ");
        }
        None => s += &format!("{i}No outlier handling.\n"),
    }
    if o.initialization.uses_priors() && !o.adaptive_smoothing.is_empty() {
        let w: Vec<String> = o
            .adaptive_smoothing
            .iter()
            .map(|&x| format_g(f64::from(x)))
            .collect();
        s += &format!("{i}Adaptive smoothing weights: [{}]\n", w.join(", "));
        s += &format!("{i}B-spline smoothing\n");
        s += &format!("{i}  spline order = {}\n", o.spline_order);
        s += &format!("{i}  number of levels = {}\n", fixed_array(&o.levels));
        s += &format!(
            "{i}  number of initial control points = {}\n",
            fixed_array(&o.control_points)
        );
    }
    let classes = o.classes as usize;
    for n in 0..classes {
        s += &format!(
            "{i}Tissue class {}: proportion = {}\n",
            n + 1,
            format_g(f64::from(f.proportions[n]))
        );
        s += &f.components()[n].print("      ");
    }
    if o.use_partial_volume_likelihoods {
        let mut n = classes;
        for set in &o.partial_volume_sets {
            let labels: Vec<String> = set.iter().map(|l| l.to_string()).collect();
            s += &format!(
                "{i}Partial volume class {} [labels: {}]: proportion = {}\n",
                n + 1,
                labels.join("x"),
                format_g(f64::from(f.proportions.get(n).copied().unwrap_or(0.0)))
            );
            if let Some(c) = f.components().get(n) {
                s += &c.print("      ");
            }
            n += 1;
        }
    }
    s
}

/// `operator<<` of an `itk::FixedArray<unsigned int>`: `[a, b, c]`.
fn fixed_array(v: &[u32]) -> String {
    let parts: Vec<String> = v.iter().map(|x| x.to_string()).collect();
    format!("[{}]", parts.join(", "))
}

/// `Likelihood` printing lives with the models.
impl Likelihood {
    fn print(&self, indent: &str) -> String {
        crate::atropos::models::print(self, indent)
    }
}

pub const DESCRIPTION: &str = "A finite mixture modeling (FMM) segmentation approach with possibilities for specifying prior constraints. These prior constraints include the specification of a prior label image, prior probability images (one for each class), and/or an MRF prior to enforce spatial smoothing of the labels.  All segmentation images including priors and masks must be in the same voxel and physical space.  Similar algorithms include FAST and SPM.  Reference: Avants BB, Tustison NJ, Wu J, Cook PA, Gee JC. An open source multivariate framework for n-tissue segmentation with evaluation on public data. Neuroinformatics. 2011 Dec;9(4):381-400.";

/// The options, as `AtroposInitializeCommandLineOptions` declares them (`Atropos.cxx:1193-1587`).
pub const OPTIONS: &[MenuOption] = &[
    MenuOption {
        long: "image-dimensionality",
        short: Some('d'),
        usage: &["2/3/4"],
        description: "This option forces the image to be treated as a specified-dimensional image.  If not specified, Atropos tries to infer the dimensionality from the first input image.",
        defaults: &[],
    },
    MenuOption {
        long: "intensity-image",
        short: Some('a'),
        usage: &["[intensityImage,<adaptiveSmoothingWeight>]"],
        description: "One or more scalar images is specified for segmentation using the -a/--intensity-image option.  For segmentation scenarios with no prior information, the first scalar image encountered on the command line is used to order labelings such that the class with the smallest intensity signature is class '1' through class 'N' which represents the voxels with the largest intensity values.  The optional adaptive smoothing weight parameter is applicable only when using prior label or probability images.  This scalar parameter is to be specified between [0,1] which smooths each labeled region separately and modulates the intensity measurement at each voxel in each intensity image between the original intensity and its smoothed counterpart.  The smoothness parameters are governed by the -b/--bspline option.",
        defaults: &[],
    },
    MenuOption {
        long: "bspline",
        short: Some('b'),
        usage: &["[<numberOfLevels=6>,<initialMeshResolution=1x1x...>,<splineOrder=3>]"],
        description: "If the adaptive smoothing weights are > 0, the intensity images are smoothed in calculating the likelihood values. This is to account for subtle intensity differences across the same tissue regions.",
        defaults: &[],
    },
    MenuOption {
        long: "initialization",
        short: Some('i'),
        usage: &[
            "Random[numberOfClasses]",
            "Otsu[numberOfTissueClasses]",
            "KMeans[numberOfTissueClasses,<clusterCenters(in ascending order and for first intensity image only)>]",
            "PriorProbabilityImages[numberOfTissueClasses,fileSeriesFormat(index=1 to numberOfClasses) or vectorImage,priorWeighting,<priorProbabilityThreshold>]",
            "PriorLabelImage[numberOfTissueClasses,labelImage,priorWeighting]",
        ],
        description: "To initialize the FMM parameters, one of the following options must be specified.  If one does not have prior label or probability images we recommend using kmeans as it is typically faster than otsu and can be used with multivariate initialization. However, since a Euclidean distance on the inter cluster distances is used, one might have to appropriately scale the additional input images. Random initialization is meant purely for intellectual curiosity. The prior weighting (specified in the range [0,1]) is used to modulate the calculation of the posterior probabilities between the likelihood*mrfprior and the likelihood*mrfprior*prior.  For specifying many prior probability images for a multi-label segmentation, we offer a minimize usage option (see -m).  With that option one can specify a prior probability threshold in which only those pixels exceeding that threshold are stored in memory. ",
        defaults: &[],
    },
    MenuOption {
        long: "partial-volume-label-set",
        short: Some('s'),
        usage: &["label1xlabel2xlabel3"],
        description: "The partial volume estimation option allows one to modelmixtures of classes within single voxels.  Atropos currently allows the user to model two class mixtures per partial volume class. The user specifies a set of class labels per partial volume class requested.  For example, suppose the user was performing a classic 3-tissue segmentation (csf, gm, wm) using kmeans initialization.  Suppose the user also wanted to model the partial voluming effects between csf/gm and gm/wm. The user would specify it using -i kmeans[3] and -s 1x2 -s 2x3.  So, for this example, there would be 3 tissue classes and 2 partial volume classes.  Optionally,the user can limit partial volume handling to mrf considerations only whereby the output would only be the three tissues.",
        defaults: &[],
    },
    MenuOption {
        long: "use-partial-volume-likelihoods",
        short: None,
        usage: &["1/(0)", "true/(false)"],
        description: "The user can specify whether or not to use the partial volume likelihoods, in which case the partial volume class is considered separate from the tissue classes.  Alternatively, one can use the MRF only to handle partial volume in which case, partial volume voxels are not considered as separate classes.",
        defaults: &[],
    },
    MenuOption {
        long: "posterior-formulation",
        short: Some('p'),
        usage: &[
            "Socrates[<useMixtureModelProportions=1>,<initialAnnealingTemperature=1>,<annealingRate=1>,<minimumTemperature=0.1>]",
            "Plato[<useMixtureModelProportions=1>,<initialAnnealingTemperature=1>,<annealingRate=1>,<minimumTemperature=0.1>]",
            "Aristotle[<useMixtureModelProportions=1>,<initialAnnealingTemperature=1>,<annealingRate=1>,<minimumTemperature=0.1>]",
            "Sigmoid[<useMixtureModelProportions=1>,<initialAnnealingTemperature=1>,<annealingRate=1>,<minimumTemperature=0.1>]]",
        ],
        description: "Different posterior probability formulations are possible as are different update options.  To guarantee theoretical convergence properties, a proper formulation of the well-known iterated conditional modes (ICM) uses an asynchronous update step modulated by a specified annealing temperature.  If one sets the AnnealingTemperature > 1 in the posterior formulation a traditional code set for a proper ICM update will be created. Otherwise, a synchronous update step will take place at each iteration. The annealing temperature, T, converts the posteriorProbability to posteriorProbability^(1/T) over the course of optimization. Options include the following:   Socrates: posteriorProbability = (spatialPrior)^priorWeight*(likelihood*mrfPrior)^(1-priorWeight),  Plato: posteriorProbability = 1.0,  Aristotle: posteriorProbability = 1.0,  Sigmoid: posteriorProbability = 1.0, ",
        defaults: &[],
    },
    MenuOption {
        long: "mask-image",
        short: Some('x'),
        usage: &["maskImageFilename"],
        description: "The image mask (which is required) defines the region which is to be labeled by the Atropos algorithm.",
        defaults: &[],
    },
    MenuOption {
        long: "convergence",
        short: Some('c'),
        usage: &[
            "numberOfIterations",
            "[<numberOfIterations=5>,<convergenceThreshold=0.001>]",
        ],
        description: "Convergence is determined by calculating the mean maximum posterior probability over the region of interest at each iteration. When this value decreases or increases less than the specified threshold from the previous iteration or the maximum number of iterations is exceeded the program terminates.",
        defaults: &[],
    },
    MenuOption {
        long: "likelihood-model",
        short: Some('k'),
        usage: &[
            "Gaussian",
            "HistogramParzenWindows[<sigma=1.0>,<numberOfBins=32>]",
            "ManifoldParzenWindows[<pointSetSigma=1.0>,<evaluationKNeighborhood=50>,<CovarianceKNeighborhood=0>,<kernelSigma=0>]",
            "JointShapeAndOrientationProbability[<shapeSigma=1.0>,<numberOfShapeBins=64>, <orientationSigma=1.0>, <numberOfOrientationBins=32>]",
            "LogEuclideanGaussian",
        ],
        description: "Both parametric and non-parametric options exist in Atropos. The Gaussian parametric option is commonly used (e.g. SPM & FAST) where the mean and standard deviation for the Gaussian of each class is calculated at each iteration.  Other groups use non-parametric approaches exemplified by option 2.  We recommend using options 1 or 2 as they are fairly standard and the default parameters work adequately.",
        defaults: &[],
    },
    MenuOption {
        long: "mrf",
        short: Some('m'),
        usage: &[
            "[<smoothingFactor=0.3>,<radius=1x1x...>]",
            "[<mrfCoefficientImage>,<radius=1x1x...>]",
        ],
        description: "Markov random field (MRF) theory provides a general framework for enforcing spatially contextual constraints on the segmentation solution.  The default smoothing factor of 0.3 provides a moderate amount of smoothing. Increasing this number causes more smoothing whereas decreasing the number lessens the smoothing. The radius parameter specifies the mrf neighborhood.  Different update schemes are possible but only the asynchronous updating has theoretical convergence properties. ",
        defaults: &[],
    },
    MenuOption {
        long: "icm",
        short: Some('g'),
        usage: &["[<useAsynchronousUpdate=1>,<maximumNumberOfICMIterations=1>,<icmCodeImage=''>]"],
        description: "Asynchronous updating requires the construction of an ICM code image which is a label image (with labels in the range {1,..,MaximumICMCode}) constructed such that no MRF neighborhood has duplicate ICM code labels.  Thus, to update the voxel class labels we iterate through the code labels and, for each code label, we iterate through the image and update the voxel class label that has the corresponding ICM code label.  One can print out the ICM code image by specifying an ITK-compatible image filename.",
        defaults: &[],
    },
    MenuOption {
        long: "use-random-seed",
        short: Some('r'),
        usage: &["0/(1)"],
        description: "Initialize internal random number generator with a random seed. Otherwise, initialize with a constant seed number.",
        defaults: &[],
    },
    MenuOption {
        long: "output",
        short: Some('o'),
        usage: &["[classifiedImage,<posteriorProbabilityImageFileNameFormat>]"],
        description: "The output consists of a labeled image where each voxel in the masked region is assigned a label from 1, 2, ..., N.  Optionally, one can also output the posterior probability images specified in the same format as the prior probability images, e.g. posterior%02d.nii.gz (C-style file name formatting).",
        defaults: &[],
    },
    MenuOption {
        long: "minimize-memory-usage",
        short: Some('u'),
        usage: &["(0)/1"],
        description: "By default, memory usage is not minimized, however, if this is needed, the various probability and distance images are calculated on the fly instead of being stored in memory at each iteration. Also, if prior probability images are used, only the non-negligible pixel values are stored in memory. ",
        defaults: &["0"],
    },
    MenuOption {
        long: "winsorize-outliers",
        short: Some('w'),
        usage: &[
            "BoxPlot[<lowerPercentile=0.25>,<upperPercentile=0.75>,<whiskerLength=1.5>]",
            "GrubbsRosner[<significanceLevel=0.05>,<winsorizingLevel=0.10>]",
        ],
        description: "To remove the effects of outliers in calculating the weighted mean and weighted covariance, the user can opt to remove the outliers through the options specified below.",
        defaults: &[],
    },
    MenuOption {
        long: "use-euclidean-distance",
        short: Some('e'),
        usage: &["(0)/1"],
        description: "Given prior label or probability images, the labels are propagated throughout the masked region so that every voxel in the mask is labeled.  Propagation is done by using a signed distance transform of the label. Alternatively, propagation of the labels with the fast marching filter respects the distance along the shape of the mask (e.g. the sinuous sulci and gyri of the cortex).",
        defaults: &["0"],
    },
    MenuOption {
        long: "label-propagation",
        short: Some('l'),
        usage: &["whichLabel[lambda=0.0,<boundaryProbability=1.0>]"],
        description: "The propagation of each prior label can be controlled by the lambda and boundary probability parameters.  The latter parameter is the probability (in the range [0,1]) of the label on the boundary which increases linearly to a maximum value of 1.0 in the interior of the labeled region.  The former parameter dictates the exponential decay of probability propagation outside the labeled region from the boundary probability, i.e. boundaryProbability*exp( -lambda * distance ).",
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn series_formats_like_snprintf() {
        assert_eq!(
            series_names("POSTERIOR_%02d.nii.gz", 3).unwrap(),
            ["POSTERIOR_01.nii.gz", "POSTERIOR_02.nii.gz", "POSTERIOR_03.nii.gz"]
        );
        assert_eq!(series_names("p%d.nii", 2).unwrap(), ["p1.nii", "p2.nii"]);
        assert_eq!(series_names("same.nii", 2).unwrap(), ["same.nii", "same.nii"]);
        assert_eq!(series_names("a%%b%3d.nii", 1).unwrap(), ["a%b  1.nii"]);
        assert_eq!(series_names("x%-3dy", 1).unwrap(), ["x1  y"]);
        assert_eq!(series_names("x%#xy", 1).unwrap(), ["x0x1y"]);
        assert_eq!(series_names("x%.3luy", 1).unwrap(), ["x001y"]);
        assert!(series_names("x%sy", 1).is_err());
        assert!(series_names("x%d%dy", 1).is_err());
        assert!(series_names("x%fy", 1).is_err());
    }

    #[test]
    fn functions_count_from_the_last_value() {
        let v = vec![
            OptionValue {
                name: "a".into(),
                parameters: vec![],
            },
            OptionValue {
                name: "b".into(),
                parameters: vec!["1".into()],
            },
        ];
        assert_eq!(function(&v, 0).unwrap().name, "b");
        assert_eq!(function(&v, 1).unwrap().name, "a");
        assert!(function(&v, 2).is_none());
        assert_eq!(param(&v[1], 0), "1");
        assert_eq!(param(&v[1], 1), "");
    }
}
