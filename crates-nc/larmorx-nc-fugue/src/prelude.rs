// SPDX-License-Identifier: LicenseRef-FSL
// Translated to Rust from FSL 6.0.7.23: fsl-fugue 2201.5 (prelude.cc: the options, `wrap` of a
// volume, `do_unwrapping`, `main`). PRELUDE (Phase Region Expanding Labeller for Unwrapping
// Discrete Estimates): Mark Jenkinson, FMRIB Image Analysis Group; Copyright (C) 2000-2001
// University of Oxford. Rust translation Copyright 2026 Karel Lopez Vilaret; modified from the
// originals. Distributed under the FSL Licence (see LICENSE).
//! `prelude`'s `main`: its options (fsl-utils' parser, FSL's usage text and messages), the
//! inputs (a complex image, or a phase and a magnitude image; a mask), the checks of the phase
//! range, the masks, labels and unwrapping of each volume ([`crate::unwarpfns`]) and the files
//! it writes.
//!
//! D13 (`LARMORX_UPSTREAM_DEFECTS`): `reproduce` gives FSL's result bit for bit; `fix` (the
//! default) corrects FSL-261 to FSL-272 (`docs/findings/fsl-prelude.md`), with a notice where a
//! correction changed the result.

use std::io::Write;

use larmorx_core::defects::{self, Defect, Notices, UpstreamDefects};
use larmorx_nc_core::arma::{self, Mat};
use larmorx_nc_core::cnum;
use larmorx_nc_core::complex;
use larmorx_nc_core::exception;
use larmorx_nc_core::nifti::dt;
use larmorx_nc_core::options::{ArgFlag, Opt, OptionParser, Parsed, Value};
use larmorx_nc_core::settings::Settings;
use larmorx_nc_core::volume::{self, Elem, FslError, SaveError, SaveOptions, Saved, Volume};

use crate::unwarpfns::{self as uw, UnwrapLog};

const TITLE: &str = "prelude (Version 2.0 in c# minor)\nPhase Region Expanding Labeller for \
                     Unwrapping Discrete Estimates\nCopyright(c) 2001, University of Oxford \
                     (Mark Jenkinson)";
const EXAMPLES: &str = "prelude -c <rawphase> -o <unwrappedphase> [options]\nprelude -p \
                        <phasevol> -a <absvol> -o <unwrappedphase> [options]";

const PI: f64 = std::f64::consts::PI;

/// How the replica runs: the settings read from the environment and D13's mode.
pub struct Run<'a> {
    pub fsl: &'a Settings,
    pub mode: UpstreamDefects,
}

fn opt(key: &'static str, default: Value, help: &'static str, flag: ArgFlag) -> Opt {
    Opt {
        key,
        default,
        help,
        compulsory: false,
        flag,
        visible: true,
    }
}

/// The options of `prelude`, in the order its `main` adds them (the order of the usage text).
pub fn parser() -> OptionParser {
    use ArgFlag::{NoArgument as N, Requires as R};
    use Value::{Bool as B, Float as F, Int as I, Str as S};
    let s = String::new;
    let mut out = opt(
        "-o,--out,-u,--unwrap",
        S(s()),
        "filename for saving the unwrapped phase output",
        R(1),
    );
    out.compulsory = true;
    OptionParser::new(
        TITLE,
        EXAMPLES,
        vec![
            opt(
                "-c,--complex",
                S(s()),
                "filename of complex phase input volume",
                R(1),
            ),
            opt(
                "-a,--abs",
                S(s()),
                "filename of absolute input volume",
                R(1),
            ),
            opt(
                "-p,--phase",
                S(s()),
                "filename of raw phase input volume",
                R(1),
            ),
            out,
            opt(
                "-n,--numphasesplit",
                I(8),
                "number of phase partitions to use",
                R(1),
            ),
            opt(
                "--labelslices",
                B(false),
                "does label processing in 2D (slice at a time)",
                N,
            ),
            opt(
                "-s,--slices",
                B(false),
                "does all processing in 2D (slice at a time)",
                N,
            ),
            opt(
                "-f,--force3D",
                B(false),
                "forces all processing to be full 3D",
                N,
            ),
            opt(
                "-t,--thresh",
                F(0.0),
                "intensity threshold for masking",
                R(1),
            ),
            opt("-m,--mask", S(s()), "filename of mask input volume", R(1)),
            opt(
                "--start",
                I(0),
                "first image number to process (default 0)",
                R(1),
            ),
            opt(
                "--end",
                I(0),
                "final image number to process (default Inf)",
                R(1),
            ),
            opt(
                "--savemask",
                S(s()),
                "filename for saving the mask volume",
                R(1),
            ),
            opt(
                "-r,--rawphase",
                S(s()),
                "filename for saving the raw phase output",
                R(1),
            ),
            opt(
                "-l,--labels",
                S(s()),
                "filename for saving the area labels output",
                R(1),
            ),
            opt(
                "--removeramps",
                B(false),
                "remove phase ramps during unwrapping",
                N,
            ),
            opt("-v,--verbose", B(false), "switch on diagnostic messages", N),
            opt("-h,--help", B(false), "display this message", N),
        ],
    )
}

/// The parsed options.
struct Args {
    complexvol: Option<String>,
    absvol: Option<String>,
    phasevol: Option<String>,
    uphasevol: String,
    num_phase_split: i32,
    labelslices: bool,
    allslices: bool,
    force3d: bool,
    thresh: Option<f32>,
    inmask: Option<String>,
    start: Option<i32>,
    end: Option<i32>,
    maskvol: Option<String>,
    rawphasevol: Option<String>,
    labelvol: Option<String>,
    removeramps: bool,
    verbose: bool,
}

fn args_of(parser: &OptionParser, p: &Parsed) -> Args {
    let get = |k: &str| p.get(parser, k);
    let string = |k: &str| {
        let o = get(k);
        o.set.then(|| o.str().to_owned())
    };
    let int = |k: &str| {
        let o = get(k);
        o.set.then(|| o.int())
    };
    let t = get("-t,--thresh");
    Args {
        complexvol: string("-c,--complex"),
        absvol: string("-a,--abs"),
        phasevol: string("-p,--phase"),
        uphasevol: get("-o,--out,-u,--unwrap").str().to_owned(),
        num_phase_split: get("-n,--numphasesplit").int(),
        labelslices: get("--labelslices").bool(),
        allslices: get("-s,--slices").bool(),
        force3d: get("-f,--force3D").bool(),
        thresh: t.set.then(|| t.float()),
        inmask: string("-m,--mask"),
        start: int("--start"),
        end: int("--end"),
        maskvol: string("--savemask"),
        rawphasevol: string("-r,--rawphase"),
        labelvol: string("-l,--labels"),
        removeramps: get("--removeramps").bool(),
        verbose: get("-v,--verbose").bool(),
    }
}

/// The streams and the notices of one run.
struct Io<'a> {
    out: &'a mut dyn Write,
    err: &'a mut dyn Write,
    notices: &'a mut Notices,
}

impl Io<'_> {
    fn cout(&mut self, line: &str) {
        let _ = writeln!(self.out, "{line}");
    }

    fn reproduce(&self) -> bool {
        self.notices.reproduce()
    }

    fn notice(&mut self, d: &'static Defect, detail: impl Into<String>) {
        self.notices.add(d, detail);
    }

    /// An input the default refuses (rule B): the message, status 1.
    fn reject(&mut self, d: &'static Defect, what: &str) -> End {
        self.notice(d, what);
        let _ = writeln!(self.err, "larmorx-nc: error: prelude: {what} ({})", d.id);
        End::Exit(1)
    }

    /// A case FSL's result depends on memory it does not own: refused in both modes.
    fn refuse(&mut self, d: &'static Defect, what: &str) -> End {
        self.notice(d, what);
        let _ = writeln!(
            self.err,
            "larmorx-nc: error: prelude: {what} ({}: FSL reads memory it does not own here)",
            d.id
        );
        End::Exit(1)
    }
}

/// How `do_unwrapping` ends early.
enum End {
    Exit(u8),
    Throw(FslError),
}

impl From<FslError> for End {
    fn from(e: FslError) -> End {
        End::Throw(e)
    }
}

/// `prelude`'s `main`: `args[0]` is the program name. Returns the exit status. The notices of
/// FSL's defects are printed last, as every `larmorx-nc` tool prints them.
pub fn main(args: &[String], run: &Run, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let mut notices = Notices::new(run.mode);
    let code = exception::catch(|| {
        let mut io = Io {
            out: &mut *out,
            err: &mut *err,
            notices: &mut notices,
        };
        match main_inner(args, run, &mut io) {
            Ok(code) | Err(End::Exit(code)) => code,
            Err(End::Throw(e)) => exception::throw(e),
        }
    });
    let _ = out.flush();
    for n in notices.as_slice() {
        let _ = writeln!(err, "larmorx-nc: {}", n.line());
    }
    // an exception nothing in prelude catches goes on to the program's entry (std::terminate)
    code.unwrap_or_else(|e| exception::throw(e))
}

fn main_inner(args: &[String], run: &Run, io: &mut Io) -> Result<u8, End> {
    let parser = parser();
    let parsed = match parser.parse_command_line(args, 0, false) {
        Ok(p) => p,
        Err((_, what)) => {
            let _ = io.err.write_all(parser.usage().as_bytes());
            let _ = writeln!(io.err, "\n{what}");
            return Ok(1);
        }
    };
    if parsed.get(&parser, "-h,--help").bool() {
        let _ = io.err.write_all(parser.usage().as_bytes());
        return Ok(1);
    }
    if let Some(t) = parser.check_compulsory(&parsed) {
        let _ = io.err.write_all(t.as_bytes());
        let _ = io.err.write_all(parser.usage().as_bytes());
        return Ok(1);
    }
    let a = args_of(&parser, &parsed);
    let usage_and = |io: &mut Io, msg: &str| {
        let _ = io.err.write_all(parser.usage().as_bytes());
        let _ = writeln!(io.err, "\n{msg}");
    };
    if a.complexvol.is_some() && (a.absvol.is_some() || a.phasevol.is_some()) {
        usage_and(io, "Cannot specify both --complex AND --phase or --abs.");
        return Ok(1);
    }
    if a.phasevol.is_some() != a.absvol.is_some() {
        usage_and(io, "Both --phase AND --abs must be used together.");
        return Ok(1);
    }
    if a.num_phase_split < 2 {
        usage_and(
            io,
            &format!(
                "Always set --numphasesplit greater than 1.\nNOT {}",
                a.num_phase_split
            ),
        );
        return Ok(1);
    }
    do_unwrapping(&a, run, io)
}

/// FSL's `volume4D` of `n` volumes of the size of `like`, with `like`'s properties.
fn series_like(like: &Volume, n: i64) -> Volume {
    let mut v = Volume::new(like.dims[0], like.dims[1], like.dims[2], n);
    v.copy_properties(like);
    v
}

/// The outputs that FSL writes once per volume (FSL-263): FSL's file is the last volume's; the
/// correction collects every volume's into one series.
struct PerVolume<T: Elem> {
    name: Option<String>,
    volumes: Vec<Volume<T>>,
}

impl<T: Elem> PerVolume<T> {
    fn new(name: &Option<String>) -> PerVolume<T> {
        PerVolume {
            name: name.clone(),
            volumes: Vec::new(),
        }
    }
}

fn do_unwrapping(a: &Args, run: &Run, io: &mut Io) -> Result<u8, End> {
    let fsl = io.reproduce();
    let settings = run.fsl;

    // -------------------------------------------------------------------------------------
    // The inputs
    let (mut phasemaps, absmaps) = if let Some(name) = &a.complexvol {
        read_complex(name, settings, io)?
    } else {
        if a.verbose {
            io.cout("Loading volumes");
        }
        let (p, _, _) =
            volume::read_volume4d::<f32>(a.phasevol.as_deref().unwrap_or(""), settings)?;
        if a.verbose {
            io.cout("Phase loaded");
        }
        let (m, _, _) = volume::read_volume4d::<f32>(a.absvol.as_deref().unwrap_or(""), settings)?;
        if a.verbose {
            io.cout("Magnitude loaded");
        }
        (p, m)
    };

    let mut threshset = a.thresh.is_some();
    let mut threshval = a.thresh.unwrap_or(0.0);
    let mut imgstart = 0i64;
    let mut imgend = phasemaps.dims[3] - 1;
    if let Some(s) = a.start {
        imgstart = i64::from(s);
    }
    if let Some(e) = a.end {
        imgend = i64::from(e);
    }
    let mut masks = None;
    let mut maskend = 0i64;
    if let Some(name) = &a.inmask {
        let (m, _, _) = volume::read_volume4d::<f32>(name, settings)?;
        if a.verbose {
            io.cout("Mask loaded");
        }
        maskend = m.dims[3] - 1;
        masks = Some(m);
        if !threshset {
            threshset = true;
            threshval = 0.5;
        }
    }

    // FSL-271: no volume to process (FSL writes no output and exits 0)
    if imgend < imgstart && !fsl {
        // FSL's own exception for a first volume outside the series comes first
        phasemaps.sub_volume(imgstart)?;
        return Err(io.reject(
            &defects::FSL_271,
            &format!("no volume to process: --end ({imgend}) is before --start ({imgstart})"),
        ));
    }

    // sanity check on phase values
    for n in imgstart..=imgend {
        let ph = phasemaps.sub_volume(n)?;
        let (mx, mn) = ph.extrema();
        let range = (larmorx_nc_core::sse::S32(mx) - mn).0;
        if f64::from(range) > 3.0 * PI {
            let _ = writeln!(
                io.err,
                "ERROR: input phase image exceeds allowable phase range.\nAllowable range is \
                 6.283 radians.  Image range is: {} radians.\nAborting.",
                cnum::stream(f64::from(range))
            );
            return Ok(255);
        }
        if f64::from(mx) > 1.001 * PI || f64::from(mn) < -1.001 * PI {
            if a.verbose {
                io.cout("Rewrapping phase range to [-pi,pi]");
            }
            let mut w = ph;
            for v in &mut w.data {
                *v = uw::wrap(*v);
            }
            phasemaps.set_sub_volume(n, &w)?;
        }
    }

    let mut label_2d = a.labelslices;
    let mut unwrap_2d = a.allslices;
    // The automatic behaviour is to use 2D labels for large images
    let first = phasemaps.sub_volume(imgstart)?;
    if first.dims[0] > 64 || first.dims[1] > 64 {
        label_2d = true;
    }
    if a.force3d {
        label_2d = false;
        unwrap_2d = false;
    }

    // FSL-272: a magnitude (or, with -m, a mask) of another size than the phase: FSL reads it by
    // the phase's indices, with its extrapolation outside it
    let size3 = |v: &Volume| [v.dims[0], v.dims[1], v.dims[2]];
    let source_size = masks.as_ref().map_or_else(|| size3(&absmaps), size3);
    if source_size != size3(&phasemaps) && imgend >= imgstart {
        let what = if masks.is_some() {
            "the mask image has another size than the phase image"
        } else {
            "the magnitude image has another size than the phase image"
        };
        if !fsl {
            return Err(io.reject(&defects::FSL_272, what));
        }
        return Err(io.refuse(&defects::FSL_272, what));
    }

    let mut uphase: Option<Volume> = None;
    let mut count = 0i64;
    let mut saved_masks = PerVolume::<f32>::new(&a.maskvol);
    let mut saved_labels = PerVolume::<i32>::new(&a.labelvol);
    for n in imgstart..=imgend {
        // Make mask
        let source = match &masks {
            Some(m) => m.sub_volume(n.min(maskend))?,
            None => absmaps.sub_volume(n)?,
        };
        // FSL-269: NaN values change the threshold (from percentiles), and a NaN first value
        // makes the maximum NaN (an empty mask)
        let nan_first = source.data.first().is_some_and(|v| v.is_nan());
        if uw::has_nan(&source) && (!threshset || nan_first) {
            io.notice(
                &defects::FSL_269,
                if fsl {
                    "NaN values of the magnitude (or mask) image entered its threshold or maximum"
                } else {
                    "NaN values of the magnitude (or mask) image left out of its threshold and maximum"
                },
            );
        }
        if !threshset {
            threshval = match uw::basic_mask_threshold(&source, !fsl) {
                Ok(t) => t,
                Err(_) => {
                    return Err(io.refuse(
                        &defects::FSL_269,
                        "NaN values of the magnitude image in the sort of its percentiles",
                    ));
                }
            };
        }
        let mask = if unwrap_2d {
            uw::make_head_mask2d(&source, threshval, !fsl)
        } else {
            uw::make_head_mask(&source, threshval, !fsl)
        };
        if let Some(name) = &saved_masks.name {
            if fsl {
                if let Some(code) = save(&mask, name, run, io)? {
                    return Ok(code);
                }
            } else {
                saved_masks.volumes.push(mask.clone());
            }
        }

        let mut ph = phasemaps.sub_volume(n)?;
        // Remove phase ramps
        let mut ramps = [0.0f64; 3];
        if a.removeramps {
            if uw::ramps_skip_voxels(&ph, &mask) {
                io.notice(
                    &defects::FSL_261,
                    if fsl {
                        "the phase ramps left the last column, row and slice out"
                    } else {
                        "the phase ramps estimated, removed and restored on every column, row and slice"
                    },
                );
            }
            if a.verbose {
                io.cout("Removing linear ramps");
            }
            ramps = uw::estimate_linear_ramps(&ph, &mask, fsl);
            if a.verbose {
                io.cout("Estimated linear ramps");
            }
            uw::remove_linear_ramps(&ramps, &mut ph, &mask, fsl);
            phasemaps.set_sub_volume(n, &ph)?;
            if a.verbose {
                io.cout("Removed linear ramps");
                let m = Mat::col(&ramps);
                let _ = writeln!(io.out, "Ramp values: {}", arma::print_expr(&m));
                if fsl {
                    io.notice(
                        &defects::FSL_267,
                        "the ramp-free phase written to grot in the working directory",
                    );
                    if let Some(code) = save(&ph, "grot", run, io)? {
                        return Ok(code);
                    }
                } else {
                    io.notice(
                        &defects::FSL_267,
                        "no file grot written in the working directory",
                    );
                }
            }
        }

        // Make labels
        if a.verbose {
            io.cout(&format!("Number of phase splits = {}", a.num_phase_split));
        }
        let label = if unwrap_2d {
            uw::find_phase_labels2d(&ph, &mask, a.num_phase_split, false, fsl).0
        } else if label_2d {
            let (l, met) = uw::find_phase_labels2d(&ph, &mask, a.num_phase_split, true, fsl);
            if met.reused {
                io.notice(
                    &defects::FSL_262,
                    if fsl {
                        "label numbers reused after a slice without labels"
                    } else {
                        "label numbers kept unique across a slice without labels"
                    },
                );
            }
            l
        } else {
            uw::find_phase_labels(&ph, &mask, a.num_phase_split)
        };
        if let Some(name) = &saved_labels.name {
            if fsl {
                if let Some(code) = save(&label, name, run, io)? {
                    return Ok(code);
                }
            } else {
                saved_labels.volumes.push(label.clone());
            }
        }

        // Unwrap phase
        let (mut uph, log): (Volume, UnwrapLog) = if unwrap_2d {
            match uw::unwrap2d(&ph, &label, a.verbose) {
                Ok(r) => r,
                Err(_) => {
                    return Err(io.refuse(
                        &defects::FSL_270,
                        "NaN differences between slices in the sort of their median",
                    ));
                }
            }
        } else {
            uw::unwrap(&ph, &label, a.verbose)
        };
        for l in &log.lines {
            io.cout(l);
        }

        // Restore phase ramps
        if a.removeramps {
            uw::restore_linear_ramps(&ramps, &mut uph, &mask, fsl);
            uw::restore_linear_ramps(&ramps, &mut ph, &mask, fsl);
            phasemaps.set_sub_volume(n, &ph)?;
            if a.verbose {
                io.cout("Removed linear ramps");
            }
        }

        // uphase.addvolume(uph)
        let total = imgend - imgstart + 1;
        let u = uphase.get_or_insert_with(|| series_like(&uph, total));
        u.set_sub_volume(count, &uph)?;
        count += 1;
    }

    // Save outputs
    let fsl_saves_nothing = uphase.is_none();
    let mut uphase = uphase.unwrap_or_else(|| Volume::new(0, 0, 0, 0));
    uphase.pixdims[3] = phasemaps.pixdims[3];
    if !fsl {
        if saved_masks.volumes.len() > 1 || saved_labels.volumes.len() > 1 {
            io.notice(
                &defects::FSL_263,
                "every volume's mask or labels saved, not only the last",
            );
        }
        if let Some(name) = &saved_masks.name
            && let Some(code) = save_series(&saved_masks.volumes, name, run, io)?
        {
            return Ok(code);
        }
        if let Some(name) = &saved_labels.name
            && let Some(code) = save_series(&saved_labels.volumes, name, run, io)?
        {
            return Ok(code);
        }
    } else if count > 1 && (saved_masks.name.is_some() || saved_labels.name.is_some()) {
        io.notice(
            &defects::FSL_263,
            "only the last volume's mask or labels saved",
        );
    }
    if fsl_saves_nothing {
        // newimage writes nothing for an image without volumes (save_volume4D's -1, ignored)
        io.notice(
            &defects::FSL_271,
            "no volume processed and no output written",
        );
    } else if let Some(code) = save(&uphase, &a.uphasevol, run, io)? {
        return Ok(code);
    }
    if let Some(name) = &a.rawphasevol
        && let Some(code) = save(&phasemaps, name, run, io)?
    {
        return Ok(code);
    }
    Ok(0)
}

/// `read_complexvolume` and each volume's phase (`atan2f(imag, real)`) and magnitude.
fn read_complex(name: &str, settings: &Settings, io: &mut Io) -> Result<(Volume, Volume), End> {
    let fsl = io.reproduce();
    let Some(c) = complex::read_complexvolume(name, settings, !fsl)? else {
        // FSL returns -1: no volumes
        return Ok((Volume::new(0, 0, 0, 0), Volume::new(0, 0, 0, 0)));
    };
    if fsl {
        if c.imag_unwritten {
            let detail = if c.beyond_buffer > 0 {
                format!(
                    "a {} image read as complex: its bytes taken as floats ({} values beyond \
                     them and every imaginary value from memory FSL never wrote, zeros here)",
                    datatype_name(c.datatype),
                    c.beyond_buffer
                )
            } else {
                format!(
                    "a {} image read as complex: its bytes taken as floats, every imaginary \
                     value from memory FSL never wrote (zeros here)",
                    datatype_name(c.datatype)
                )
            };
            let d = if c.datatype == dt::COMPLEX128 {
                &defects::FSL_266
            } else {
                &defects::FSL_264
            };
            io.notice(d, detail);
        }
        if c.swapped && c.datatype == dt::COMPLEX {
            io.notice(
                &defects::FSL_265,
                "a complex image of the other byte order read with its real and imaginary parts exchanged",
            );
        }
    } else {
        if c.not_complex {
            return Err(io.reject(
                &defects::FSL_264,
                &format!(
                    "the complex input {name} is a {} image, not a complex one",
                    datatype_name(c.datatype)
                ),
            ));
        }
        if c.swapped && c.datatype == dt::COMPLEX {
            io.notice(
                &defects::FSL_265,
                "a complex image of the other byte order read with its real and imaginary parts in place",
            );
        }
        if c.datatype == dt::COMPLEX128 {
            io.notice(&defects::FSL_266, "a complex128 image read (as float)");
        }
    }
    Ok((complex::phase(&c.re, &c.im), complex::abs(&c.re, &c.im)))
}

/// A NIfTI datatype's name, as the messages give it.
fn datatype_name(d: i16) -> String {
    match d {
        dt::UNSIGNED_CHAR => "uint8".to_owned(),
        dt::SIGNED_SHORT => "int16".to_owned(),
        dt::SIGNED_INT => "int32".to_owned(),
        dt::FLOAT => "float32".to_owned(),
        dt::DOUBLE => "float64".to_owned(),
        dt::COMPLEX => "complex64".to_owned(),
        dt::COMPLEX128 => "complex128".to_owned(),
        dt::COMPLEX256 => "complex256".to_owned(),
        dt::INT8 => "int8".to_owned(),
        dt::UINT16 => "uint16".to_owned(),
        dt::UINT32 => "uint32".to_owned(),
        dt::INT64 => "int64".to_owned(),
        dt::UINT64 => "uint64".to_owned(),
        other => format!("datatype {other}"),
    }
}

/// Saves one output as `save_volume` does (newimage's `descrip`); `Some(status)` when prelude
/// stops (an invalid `FSLOUTPUTTYPE`).
fn save<T: Elem>(v: &Volume<T>, name: &str, run: &Run, io: &mut Io) -> Result<Option<u8>, End> {
    let opts =
        SaveOptions::new(volume::BUILDSTRING.as_bytes()).reproducing_offset(run.mode.reproduce());
    match v.save_volume(name, run.fsl, &opts) {
        // save_volume's -1 (no volume) is ignored by prelude
        Ok(Saved::Written | Saved::Nothing) => Ok(None),
        Err(SaveError::OutputType(o)) => {
            for l in &o.printed {
                let _ = writeln!(io.err, "{l}");
            }
            Ok(Some(1))
        }
        Err(SaveError::Exception(e)) => Err(End::Throw(e)),
    }
}

/// FSL-263's correction: every processed volume's mask (or labels) as one series.
fn save_series<T: Elem>(
    vols: &[Volume<T>],
    name: &str,
    run: &Run,
    io: &mut Io,
) -> Result<Option<u8>, End> {
    let Some(first) = vols.first() else {
        return Ok(None);
    };
    if vols.len() == 1 {
        return save(first, name, run, io);
    }
    let mut s = Volume::<T>::new(
        first.dims[0],
        first.dims[1],
        first.dims[2],
        vols.len() as i64,
    );
    s.copy_properties(first);
    let n = first.data.len();
    for (t, v) in vols.iter().enumerate() {
        s.data[t * n..(t + 1) * n].copy_from_slice(&v.data);
    }
    save(&s, name, run, io)
}
