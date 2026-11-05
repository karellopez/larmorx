// SPDX-License-Identifier: Apache-2.0
//! `larmorx fsl applytopup`: FSL 6.0.7.23's `applytopup` command line (`specs/topup.md` §7,
//! §8, §10), on the clean-room corrections [`crate::topup::apply`].
//!
//! Options are `--name=value` words or short options with their value as the next word (`-v`
//! and `-h` without one). The checks run in FSL's order; FSL's uncaught errors (its program
//! aborts with signal 6) print FSL's lines and exit with status 1.

use std::io::Write;

use larmorx_core::defects::{self, Notice, UpstreamDefects};
use larmorx_core::element::DataType;
use larmorx_io::nifti::NiftiHeader;
use larmorx_mri::hmc::files;

use super::Settings;
use super::bet2::{FSLOUTPUTTYPE_UNSET, resolve_image};
use super::common::{abort, image_exception, name_start, print_notices, read_rows};
use super::fmt;
use super::options::atof;
use crate::bet::image::{self, Output, Values};
use crate::topup::Scan;
use crate::topup::apply::{self, Corrected, FieldOnGrid, Image, Interp, LsrError, Sampling};
use crate::topup::vb::{self, VbError, VbMethod, VbOptions, VbResult};

/// The usage text (standard error).
pub const USAGE: &str = "\nPart of FSL (ID: 2412.6-dirty)\napplytopup (Version 1.0)\nCopyright(c) 2009, University of Oxford (Jesper Andersson)\n\nUsage: \napplytopup -in=topdn,botup --topup=mytu --inindex=1,2 --out=hifi\napplytopup -in=topdn --topup=mytu --inindex=1 --method=jac --interp=spline --out=hifi\n\n\nCompulsory arguments (You MUST set one or more of):\n\t-i,--imain\tcomma separated list of names of input image (to be corrected)\n\t-a,--datain\tname of text file with PE directions/times\n\t-x,--inindex\tcomma separated list of indicies into --datain of the input image (to be corrected)\n\t-t,--topup\tname of field/movements (from topup)\n\t-o,--out\tbasename for output (warped) image\n\nOptional arguments (You may optionally specify one or more of):\n\t-m,--method\tUse jacobian modulation (jac) or least-squares resampling (lsr), default=lsr.\n\t-n,--interp\tinterpolation method {trilinear,spline}, default=spline\n\t-d,--datatype\tForce output data type [preserve char short int float double].\n\t-v,--verbose\tswitch on diagnostic messages\n\t-h,--help\tdisplay this message\n\n\n";

/// The `descrip` of the image written.
pub const DESCRIP: &str = "larmorx fsl applytopup";

/// The options: short and long names, whether a value follows, the help line of the
/// compulsory ones.
const OPTIONS: &[(&str, &str, bool, Option<&str>)] = &[
    (
        "-i",
        "imain",
        true,
        Some("comma separated list of names of input image (to be corrected)"),
    ),
    (
        "-a",
        "datain",
        true,
        Some("name of text file with PE directions/times"),
    ),
    (
        "-x",
        "inindex",
        true,
        Some("comma separated list of indicies into --datain of the input image (to be corrected)"),
    ),
    (
        "-t",
        "topup",
        true,
        Some("name of field/movements (from topup)"),
    ),
    (
        "-o",
        "out",
        true,
        Some("basename for output (warped) image"),
    ),
    ("-m", "method", true, None),
    ("-n", "interp", true, None),
    ("-d", "datatype", true, None),
    ("-v", "verbose", false, None),
    ("-h", "help", false, None),
];

const APPLYTOPUP: &str = "TOPUP::ApplyTopupException";
const FILE_IO: &str = "TOPUP::TopupFileIOException";
const RUNTIME: &str = "std::runtime_error";

/// The original's message for what it does not take yet (the next sub-task of its part).
pub const NOT_YET: &str = "is not implemented yet in the clean-room original";

fn usage_error(err: &mut dyn Write, message: &str) -> u8 {
    let _ = err.write_all(USAGE.as_bytes());
    if !message.is_empty() {
        let _ = writeln!(err, "\n{message}");
    }
    1
}

/// A refusal of the original, exit status 1.
fn refuse(err: &mut dyn Write, notices: &[Notice], why: &str) -> u8 {
    print_notices(err, notices);
    let _ = writeln!(err, "larmorx: error: applytopup: {why}");
    1
}

/// The options given (the last occurrence wins); `Err`: FSL's message after the usage.
fn parse(args: &[String]) -> Result<std::collections::BTreeMap<&'static str, String>, String> {
    let mut given = std::collections::BTreeMap::new();
    let mut it = args.iter();
    while let Some(word) = it.next() {
        let (opt, value) = if let Some(rest) = word.strip_prefix("--") {
            let (name, v) = match rest.split_once('=') {
                Some((n, v)) => (n, Some(v.to_owned())),
                None => (rest, None),
            };
            let Some(o) = OPTIONS.iter().find(|o| o.1 == name) else {
                return Err(format!("--{name}: Option doesn't exist!"));
            };
            (o, v)
        } else {
            let Some(o) = OPTIONS.iter().find(|o| o.0 == word) else {
                return Err(format!("{word}: Option doesn't exist!"));
            };
            let v = if o.2 { it.next().cloned() } else { None };
            (o, v)
        };
        let (short, long, takes, _) = *opt;
        if takes {
            let Some(v) = value else {
                let shown = if word.starts_with("--") {
                    format!("--{long}")
                } else {
                    short.to_owned()
                };
                return Err(format!("{shown}: Missing non-optional argument!"));
            };
            given.insert(long, v);
        } else {
            given.insert(long, String::new());
        }
    }
    Ok(given)
}

/// Whether the name has an extension (a `.` after its last directory separator,
/// [`name_start`]: `\` one on Windows only).
fn has_extension(name: &str) -> bool {
    name[name_start(name)..].contains('.')
}

/// The "No image files match" text of a missing image, else the reader's.
fn missing_text(name: &str, e: &image::ImageError) -> String {
    let text = e.to_string();
    if text.starts_with("No image files match") || text.contains("No such file") {
        format!(
            "No image files match: {}",
            files::strip_image_extension(name)
        )
    } else {
        text
    }
}

/// An image read: its volumes in FSL's frame and the first volume's header.
struct Read {
    volumes: Vec<Vec<f32>>,
    input: image::Input,
}

fn read_image(
    settings: &Settings,
    name: &str,
    mode: UpstreamDefects,
    notices: &mut Vec<Notice>,
) -> Result<Read, image::ImageError> {
    let f = resolve_image(settings, name, mode, notices)?;
    let (vols, input) = image::read(&f, settings.threads())?;
    Ok(Read {
        volumes: vols.into_iter().map(|v| v.data).collect(),
        input,
    })
}

/// The data of `r` as stored in its file (FSL's mirroring along x undone): topup's
/// coefficients are written in its frame, without the mirroring.
fn as_stored(r: &Read) -> Vec<f64> {
    let [nx, _, _] = r.input.vol.shape;
    let v = &r.volumes[0];
    if !r.input.flipped {
        return v.iter().map(|&x| f64::from(x)).collect();
    }
    v.chunks(nx)
        .flat_map(|line| line.iter().rev().map(|&x| f64::from(x)))
        .collect()
}

/// What `--topup` names (§7.2).
enum Topup {
    Coefficients {
        field: FieldOnGrid,
        voxel: [f64; 3],
        movements: Movements,
    },
    FieldImage {
        field: FieldOnGrid,
        voxel: [f64; 3],
    },
}

/// The movements of a topup result.
enum Movements {
    None,
    /// One row of six parameters per acquisition row.
    Parameters(Vec<[f64; 6]>),
    /// One 4 × 4 matrix per acquisition row.
    Matrices(Vec<[[f64; 4]; 4]>),
}

pub fn main(args: &[String], settings: &Settings, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let mode = match settings.defects() {
        Ok(m) => m,
        Err(e) => {
            let _ = writeln!(err, "larmorx fsl applytopup: {e}");
            return 1;
        }
    };
    let given = match parse(args) {
        Ok(g) => g,
        Err(m) => return usage_error(err, &m),
    };
    if given.contains_key("help") {
        return usage_error(err, "");
    }
    let missing: Vec<_> = OPTIONS
        .iter()
        .filter(|o| o.3.is_some() && !given.contains_key(o.1))
        .collect();
    if !missing.is_empty() {
        let stars = "*".repeat(51);
        let _ = writeln!(
            err,
            "{stars}\nThe following COMPULSORY options have not been set:"
        );
        for (short, long, _, help) in missing {
            let _ = writeln!(err, "\t{short},--{long}\t{}", help.unwrap_or_default());
        }
        let _ = writeln!(err, "{stars}");
        return usage_error(err, "");
    }
    let mut notices: Vec<Notice> = Vec::new();
    match run(&given, settings, mode, &mut notices, out, err) {
        Ok(()) => {
            print_notices(err, &notices);
            0
        }
        Err(code) => code,
    }
}

fn run(
    given: &std::collections::BTreeMap<&'static str, String>,
    settings: &Settings,
    mode: UpstreamDefects,
    notices: &mut Vec<Notice>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<(), u8> {
    let get = |k: &str| given.get(k).map(String::as_str);
    let fail = |err: &mut dyn Write, notices: &[Notice], kind: &str, what: &str| -> u8 {
        print_notices(err, notices);
        abort(err, "applytopup", kind, what)
    };
    // --interp
    let interp = match get("interp").unwrap_or("spline") {
        "spline" => Interp::Spline,
        "trilinear" => Interp::Trilinear,
        v => {
            return Err(fail(
                err,
                notices,
                APPLYTOPUP,
                &format!("ApplyTopup::Unknown interpolation type {v}"),
            ));
        }
    };
    let names: Vec<String> = get("imain")
        .unwrap_or_default()
        .split(',')
        .map(str::to_owned)
        .collect();
    let index: Vec<i64> = get("inindex")
        .unwrap_or_default()
        .split(',')
        .map(|v| {
            let t = v.trim_start();
            let end = t
                .char_indices()
                .find(|&(i, c)| !(c.is_ascii_digit() || (i == 0 && (c == '+' || c == '-'))))
                .map_or(t.len(), |(i, _)| i);
            t[..end].parse::<i64>().unwrap_or(0)
        })
        .collect();
    if names.len() != index.len() {
        return Err(fail(
            err,
            notices,
            APPLYTOPUP,
            "ApplyTopup::Mismatched --in and --inindex lists",
        ));
    }
    // --datatype (preserve: the first image's type, read before any image)
    let datatype = get("datatype");
    let mut out_type: Option<DataType> = None;
    match datatype {
        None | Some("float") => {}
        Some("char") => out_type = Some(DataType::U8),
        Some("short") => out_type = Some(DataType::I16),
        Some("int") => out_type = Some(DataType::I32),
        Some("double") => out_type = Some(DataType::F64),
        Some("preserve") => {
            let name = &names[0];
            match resolve_image(settings, name, mode, notices)
                .and_then(|f| image::read(&f, settings.threads()).map(|(_, i)| i.header.clone()))
            {
                Ok(h) => {
                    let (stored, scaled) = crate::flirt::engine::stored_type(&h);
                    out_type = Some(crate::flirt::engine::grouped_type(stored, scaled, mode));
                }
                Err(e) => {
                    let what = missing_text(name, &e);
                    let _ = writeln!(err, "Image Exception : #63 :: {what}");
                    return Err(fail(err, notices, RUNTIME, &what));
                }
            }
        }
        Some(v) => {
            return Err(fail(
                err,
                notices,
                APPLYTOPUP,
                &format!("ApplyTopup::Unknown data type {v}"),
            ));
        }
    }
    // the images
    let mut reads: Vec<Read> = Vec::with_capacity(names.len());
    for name in &names {
        let r = match read_image(settings, name, mode, notices) {
            Ok(r) => r,
            Err(e) => {
                let (lines, what) = image_exception(name, &e);
                let _ = err.write_all(lines.as_bytes());
                return Err(fail(err, notices, RUNTIME, &what));
            }
        };
        if let Some(first) = reads.first()
            && (first.input.vol.shape != r.input.vol.shape
                || first.volumes.len() != r.volumes.len())
        {
            return Err(fail(
                err,
                notices,
                APPLYTOPUP,
                &format!("ApplyTopup::Mismatched input files {name} and {}", names[0]),
            ));
        }
        reads.push(r);
    }
    // the acquisition parameters (as topup reads them)
    let acq_name = get("datain").unwrap_or_default().to_owned();
    let reader = |err: &mut dyn Write, notices: &[Notice], what: &str| {
        fail(
            err,
            notices,
            FILE_IO,
            &format!("TopupFileIO::TopupDatafileReader:: {what}"),
        )
    };
    let rows = read_rows(settings, &acq_name, err).unwrap_or_default();
    let ncols = rows.first().map_or(0, Vec::len);
    let acq: Vec<[f64; 4]> = if !rows.is_empty() && ncols == 4 && rows.iter().all(|r| r.len() == 4)
    {
        rows.iter().map(|r| [r[0], r[1], r[2], r[3]]).collect()
    } else if rows.len() == 4 && ncols > 0 && rows.iter().all(|r| r.len() == ncols) {
        (0..ncols)
            .map(|c| [rows[0][c], rows[1][c], rows[2][c], rows[3][c]])
            .collect()
    } else {
        return Err(reader(
            err,
            notices,
            &format!("error reading file {acq_name}"),
        ));
    };
    if acq
        .iter()
        .any(|r| ((r[0] * r[0] + r[1] * r[1] + r[2] * r[2]).sqrt() - 1.0).abs() > 0.01)
    {
        return Err(reader(
            err,
            notices,
            "phase-encode vectors must be unity length",
        ));
    }
    if index.iter().any(|&i| i < 1 || i as usize > acq.len()) {
        return Err(fail(
            err,
            notices,
            APPLYTOPUP,
            "ApplyTopup::Invalid entry in --inindex list",
        ));
    }
    // --topup
    let tu = get("topup").unwrap_or_default().to_owned();
    let reader_fail = |err: &mut dyn Write, notices: &[Notice], what: &str| {
        fail(
            err,
            notices,
            FILE_IO,
            &format!("TopupFileIO::TopupFileReader::TopupFileReader: {what}"),
        )
    };
    if has_extension(&tu) {
        return Err(reader_fail(
            err,
            notices,
            &format!("Filename must be given without extension, fname={tu}"),
        ));
    }
    let topup = {
        let mut scratch = Vec::new();
        let coef = read_image(settings, &format!("{tu}_fieldcoef"), mode, &mut scratch)
            .ok()
            .filter(|r| matches!(r.input.header.intent_code, 2016 | 2017));
        match coef {
            Some(c) => {
                let h = &c.input.header;
                let n =
                    [h.qoffset_x, h.qoffset_y, h.qoffset_z].map(|v| v.round().max(1.0) as usize);
                let k = c.input.vol.voxel.map(|v| v.round().max(1.0) as usize);
                let voxel = [h.intent_p1, h.intent_p2, h.intent_p3].map(f64::abs);
                let order = if h.intent_code == 2017 { 2 } else { 3 };
                // on the call's thread pool (its lines' prefilters run in parallel)
                let coefficients = as_stored(&c);
                let shape = c.input.vol.shape;
                // FSL-145 reproduced: FSL's own quadratic count only (FSL aborts on a file of
                // the general count on a short axis: observed, catchup_probes.py general-count)
                let fsl_only = mode == UpstreamDefects::Reproduce;
                let evaluated = larmorx_core::parallel::with_threads(settings.threads(), || {
                    apply::coefficient_field(&coefficients, shape, n, k, order, fsl_only)
                });
                let field = match evaluated {
                    Ok(f) => f,
                    Err(e) => return Err(refuse(err, notices, &e.to_string())),
                };
                let Some((field, general_on_short)) = field else {
                    return Err(reader_fail(
                        err,
                        notices,
                        &format!("Unable to read file {tu}"),
                    ));
                };
                if general_on_short {
                    notices.push(Notice::new(
                        &defects::FSL_145,
                        mode,
                        "a coefficient file of the general quadratic count on an axis shorter than two knot spacings read (FSL's count there is one or two coefficients: FSL refuses the file)",
                    ));
                }
                let movpar = format!("{tu}_movpar.txt");
                let rows = read_rows(settings, &movpar, err).unwrap_or_default();
                let movements = if !rows.is_empty() && rows.iter().all(|r| r.len() == 6) {
                    Movements::Parameters(
                        rows.iter()
                            .map(|r| [r[0], r[1], r[2], r[3], r[4], r[5]])
                            .collect(),
                    )
                } else if !rows.is_empty()
                    && rows.len().is_multiple_of(4)
                    && rows.iter().all(|r| r.len() == 4)
                {
                    Movements::Matrices(
                        rows.chunks(4)
                            .map(|m| std::array::from_fn(|i| std::array::from_fn(|j| m[i][j])))
                            .collect(),
                    )
                } else {
                    Movements::None
                };
                Topup::Coefficients {
                    field,
                    voxel,
                    movements,
                }
            }
            None => match read_image(settings, &tu, mode, &mut scratch) {
                Ok(f) => {
                    let n = f.input.vol.shape;
                    let h: Vec<f64> = f.volumes[0].iter().map(|&x| f64::from(x)).collect();
                    // on the call's thread pool (its lines' prefilters run in parallel)
                    let field =
                        match larmorx_core::parallel::with_threads(settings.threads(), || {
                            apply::image_field(h, n)
                        }) {
                            Ok(field) => field,
                            Err(e) => return Err(refuse(err, notices, &e.to_string())),
                        };
                    Topup::FieldImage {
                        field,
                        voxel: f.input.vol.voxel,
                    }
                }
                Err(_) => {
                    return Err(reader_fail(
                        err,
                        notices,
                        &format!("Unable to read file {tu}"),
                    ));
                }
            },
        }
    };
    // the method
    let method = get("method").unwrap_or("lsr").to_owned();
    match method.as_str() {
        "jac" | "lsr" | "vb2D" | "vb3D" | "vb4D" => {}
        _ => {
            let n = Notice::new(
                &defects::FSL_135,
                mode,
                match mode {
                    UpstreamDefects::Fix => format!(
                        "--method={method}: an unknown method is refused (FSL writes nothing and exits with status 0)"
                    ),
                    UpstreamDefects::Reproduce => format!(
                        "--method={method}: an unknown method writes nothing and exits with status 0, as FSL's"
                    ),
                },
            );
            return match mode {
                UpstreamDefects::Fix => Err(refuse(
                    err,
                    &[n],
                    &format!(
                        "--method={method}: not a method (jac, lsr, vb2D, vb3D or vb4D) (FSL-135: FSL writes nothing and exits with status 0)"
                    ),
                )),
                UpstreamDefects::Reproduce => {
                    notices.push(n);
                    Ok(())
                }
            };
        }
    }
    let vb_method = VbMethod::parse(&method);
    // the restorations (lsr and the vb methods) share the movements, the collections and the
    // lines
    let lsr = method == "lsr" || vb_method.is_some();
    let first = &reads[0];
    let dims = first.input.vol.shape;
    let voxel = first.input.vol.voxel;
    let (mut field, field_voxel, movements, field_image) = match topup {
        Topup::Coefficients {
            field,
            voxel,
            movements,
        } => (field, voxel, movements, false),
        Topup::FieldImage { field, voxel } => (field, voxel, Movements::None, true),
    };
    // FSL-134: a field on another grid than the images'
    if field.dims != dims {
        let same_count = field.dims.iter().product::<usize>() == dims.iter().product::<usize>();
        let what = format!(
            "the field's grid {}x{}x{} is not the images' {}x{}x{}",
            field.dims[0], field.dims[1], field.dims[2], dims[0], dims[1], dims[2]
        );
        let n = Notice::new(&defects::FSL_134, mode, what.clone());
        match (mode, lsr, same_count) {
            (UpstreamDefects::Fix, _, _) => {
                return Err(refuse(
                    err,
                    &[n],
                    &format!("--topup={tu}: {what} (FSL-134: FSL does not refuse it)"),
                ));
            }
            (UpstreamDefects::Reproduce, true, _) => {
                return Err(refuse(
                    err,
                    &[n],
                    &format!(
                        "--topup={tu}: {what}; with --method={method} FSL reads the field's lines past their end (FSL-134: refused in both modes)"
                    ),
                ));
            }
            (UpstreamDefects::Reproduce, false, false) => {
                print_notices(err, &[n]);
                let _ = writeln!(
                    err,
                    "Image Exception : #2 :: Attempted to copydata with non-matching sizes"
                );
                return Err(abort(
                    err,
                    "applytopup",
                    RUNTIME,
                    "Attempted to copydata with non-matching sizes",
                ));
            }
            (UpstreamDefects::Reproduce, false, true) => {
                // FSL's layout (specs/topup.md §7.3, observed): the field laid as stored for the
                // displacement, its Jacobian at the same voxel indices
                let laid = if field_image {
                    larmorx_core::parallel::with_threads(settings.threads(), || {
                        apply::image_field_on(field.h.clone(), field.dims, dims)
                    })
                } else {
                    Ok(field.laid_onto(dims))
                };
                field = match laid {
                    Ok(f) => f,
                    Err(e) => return Err(refuse(err, &[n], &e.to_string())),
                };
                notices.push(Notice::new(
                    &defects::FSL_134,
                    mode,
                    format!(
                        "{what}: the field laid on the images' grid as it is stored (its Jacobian at the same voxel indices, 0 beyond its own grid), as FSL's"
                    ),
                ));
            }
        }
    }
    // the images' movements (about the field's centre)
    let centre: [f64; 3] =
        std::array::from_fn(|a| (field.dims[a] as f64 - 1.0) * field_voxel[a] / 2.0);
    let mut samplings: Vec<Option<Sampling>> = Vec::with_capacity(index.len());
    for &i in &index {
        let row = i as usize - 1;
        let s = match &movements {
            Movements::Parameters(p) => p.get(row).map(|&p| Sampling::of_movement(p, centre)),
            Movements::Matrices(m) if lsr => m.get(row).and_then(|&m| Sampling::of_matrix(m)),
            _ => None,
        };
        if lsr && s.is_none() {
            let n = Notice::new(
                &defects::FSL_135,
                mode,
                format!("--method={method} without movement parameters for every image"),
            );
            return Err(match mode {
                UpstreamDefects::Fix => refuse(
                    err,
                    &[n],
                    &format!(
                        "--method={method} needs topup's movement parameters for every image (--topup={tu} gives none for row {i}; FSL-135: FSL aborts with an internal index error)"
                    ),
                ),
                UpstreamDefects::Reproduce => {
                    print_notices(err, &[n]);
                    abort(
                        err,
                        "applytopup",
                        FILE_IO,
                        "TopupFileIO::TopupFileReader::MoveMatrix: Index i out of bound",
                    )
                }
            });
        }
        samplings.push(s);
    }
    let images: Vec<Image> = reads
        .iter()
        .zip(&index)
        .zip(&samplings)
        .map(|((r, &i), s)| {
            let a = acq[i as usize - 1];
            Image {
                volumes: r.volumes.clone(),
                scan: Scan {
                    pe: [a[0], a[1], a[2]],
                    t: a[3],
                },
                sampling: *s,
            }
        })
        .collect();
    let first_input = reads[0].input.clone();
    drop(reads);
    // FSL-144: jac leaves a phase encoding's z component out (topup estimates no field for one)
    if !lsr && let Some((k, img)) = images.iter().enumerate().find(|(_, i)| i.scan.pe[2] != 0.0) {
        let row = index[k];
        let pe = img.scan.pe;
        let n = Notice::new(
            &defects::FSL_144,
            mode,
            match mode {
                UpstreamDefects::Fix => format!(
                    "--method=jac: image {} has a phase encoding with a z component ({} {} {}, row {row} of --datain): refused (FSL leaves the component out silently)",
                    k + 1,
                    fmt::g(pe[0]),
                    fmt::g(pe[1]),
                    fmt::g(pe[2])
                ),
                UpstreamDefects::Reproduce => format!(
                    "--method=jac: the z component of image {}'s phase encoding left out, as FSL's (the image displaced along x and y only)",
                    k + 1
                ),
            },
        );
        match mode {
            UpstreamDefects::Fix => {
                return Err(refuse(
                    err,
                    &[n],
                    &format!(
                        "--method=jac: image {} ({}) has a phase encoding with a z component (row {row} of --datain: {} {} {}); topup estimates no field for such an acquisition (its --datain refuses one), and FSL would leave the component out silently (FSL-144)",
                        k + 1,
                        names.get(k).map_or("", String::as_str),
                        fmt::g(pe[0]),
                        fmt::g(pe[1]),
                        fmt::g(pe[2])
                    ),
                ));
            }
            UpstreamDefects::Reproduce => notices.push(n),
        }
    }
    // FSL-132: the first volume of the first image (trilinear in FSL)
    let first_interpolated = !lsr || images[0].sampling.is_some_and(|s| !s.is_identity());
    if interp == Interp::Spline && first_interpolated {
        notices.push(Notice::new(
            &defects::FSL_132,
            mode,
            match mode {
                UpstreamDefects::Fix => "the first volume of the first image is resampled with --interp=spline as every other (FSL's: trilinear)",
                UpstreamDefects::Reproduce => "the first volume of the first image is resampled trilinearly, as FSL's",
            },
        ));
    }
    // the Jacobian's scales (specs/topup.md §7.3): a coefficient field's derivatives per voxel
    // of the field's grid times the image's voxel size over the field's (1 on its own grid); a
    // field image's per voxel of the images' grid (1), or FSL's (FSL-134 reproduced: both
    // displacements in units of the image's x voxel size, divided by the field image's own
    // voxel sizes)
    let mut jac_scale = if field_image {
        [1.0, 1.0]
    } else {
        [voxel[0] / field_voxel[0], voxel[1] / field_voxel[1]]
    };
    let fsl_scale = [voxel[0] / field_voxel[0], voxel[0] / field_voxel[1]];
    if field_image
        && !lsr
        && ((images.iter().any(|i| i.scan.pe[0] != 0.0) && fsl_scale[0] != 1.0)
            || (images.iter().any(|i| i.scan.pe[1] != 0.0) && fsl_scale[1] != 1.0))
    {
        notices.push(Notice::new(
            &defects::FSL_134,
            mode,
            match mode {
                UpstreamDefects::Fix => "the field image's Jacobian per voxel along y (FSL's in units of the x voxel size)",
                UpstreamDefects::Reproduce => "the field image's Jacobian along y in units of the x voxel size, as FSL's",
            },
        ));
        if mode == UpstreamDefects::Reproduce {
            jac_scale = fsl_scale;
        }
    }
    let s = apply::Settings {
        dims,
        voxel,
        interp,
        first_trilinear: mode == UpstreamDefects::Reproduce,
        jac_scale,
        fsl133: mode == UpstreamDefects::Reproduce,
    };
    let n_threads = settings.threads();
    let vb_options = vb_method.map(|m| VbOptions {
        method: m,
        counts_32: mode == UpstreamDefects::Reproduce,
        count_two: mode == UpstreamDefects::Reproduce,
        half_coupling: mode == UpstreamDefects::Reproduce,
    });
    type Restored = (Corrected, Option<(String, bool, bool)>);
    let computed: Result<Result<Restored, VbError>, _> =
        larmorx_core::parallel::with_threads(n_threads, || match &vb_options {
            Some(o) => vb::vb(&s, o, &images, &field).map(|r| {
                let VbResult {
                    corrected,
                    stdout,
                    counts_wrap,
                    count_differs,
                } = r;
                (corrected, Some((stdout, counts_wrap, count_differs)))
            }),
            None if lsr => apply::lsr(&s, &images, &field)
                .map(|c| (c, None))
                .map_err(VbError::Lsr),
            None => Ok((apply::jac(&s, &images, &field), None)),
        });
    let (corrected, vb_result) = match computed {
        Ok(Ok(c)) => c,
        Ok(Err(VbError::Lsr(LsrError::Vector))) => {
            return Err(fail(
                err,
                notices,
                APPLYTOPUP,
                "ApplyTopup::Invalid phase-encode vector for least-squares restoration",
            ));
        }
        Ok(Err(VbError::Lsr(LsrError::Combination))) => {
            return Err(fail(
                err,
                notices,
                APPLYTOPUP,
                "ApplyTopup::Invalid combination of phase-encode vectors for least-squares restoration",
            ));
        }
        Ok(Err(VbError::Singular)) => {
            return Err(refuse(
                err,
                notices,
                &format!(
                    "--method={method}: the restoration's matrix of a line is singular (the noise or smoothness precision has left its range)"
                ),
            ));
        }
        Err(e) => return Err(refuse(err, notices, &e.to_string())),
    };
    if let (Some((stdout, counts_wrap, count_differs)), Some(o)) = (&vb_result, &vb_options) {
        let _ = out.write_all(stdout.as_bytes());
        let _ = out.flush();
        if *counts_wrap {
            notices.push(Notice::new(
                &defects::FSL_141,
                mode,
                match mode {
                    UpstreamDefects::Fix => format!("--method={method}: the priors' counts in 64-bit integers (FSL's 32-bit counts wrap around for this image)"),
                    UpstreamDefects::Reproduce => format!("--method={method}: the priors' counts wrapped around in 32-bit integers, as FSL's"),
                },
            ));
        }
        if o.method != VbMethod::Vb2D {
            if *count_differs {
                notices.push(Notice::new(
                    &defects::FSL_142,
                    mode,
                    match mode {
                        UpstreamDefects::Fix => format!("--method={method}: the noise prior counts every image's data points (FSL's: two images' whatever the collection holds)"),
                        UpstreamDefects::Reproduce => format!("--method={method}: the noise prior counts two images' data points whatever the collection holds, as FSL's"),
                    },
                ));
            }
            notices.push(Notice::new(
                &defects::FSL_143,
                mode,
                match mode {
                    UpstreamDefects::Fix => format!("--method={method}: the smoothness prior couples neighbouring slices one step along the phase encoding as the squared second differences give it (FSL's: at half, so its precision is indefinite and its iterations can diverge)"),
                    UpstreamDefects::Reproduce => format!("--method={method}: the smoothness prior couples neighbouring slices one step along the phase encoding at half the squared second differences' weight, as FSL's"),
                },
            ));
        }
    }
    // FSL-133: a restored line read the mapping before its first voxel where FSL's read and the
    // circular one take different field values (line lengths that do not divide 2^32)
    if corrected.wrap_read_differs {
        notices.push(Notice::new(
            &defects::FSL_133,
            mode,
            match mode {
                UpstreamDefects::Fix => {
                    "the lines wrap around circularly (FSL reads the displacement before a line's first voxel from the middle of the line)"
                }
                UpstreamDefects::Reproduce => {
                    "the displacement before a line's first voxel read from voxel (k + 2^32) mod n, as FSL's"
                }
            },
        ));
    }
    write_output(
        get("out").unwrap_or_default(),
        settings,
        mode,
        &first_input,
        &corrected,
        out_type,
        notices,
        err,
    )
}

/// `--out`: the first image's header (4D with the images' volume count), display range 0 0,
/// float32 or `--datatype`'s type (integers converted as FSL-09 says).
#[allow(clippy::too_many_arguments)]
fn write_output(
    out: &str,
    settings: &Settings,
    mode: UpstreamDefects,
    first: &image::Input,
    corrected: &Corrected,
    out_type: Option<DataType>,
    notices: &mut Vec<Notice>,
    err: &mut dyn Write,
) -> Result<(), u8> {
    let ot = match settings.get("FSLOUTPUTTYPE") {
        None => {
            print_notices(err, notices);
            let _ = err.write_all(FSLOUTPUTTYPE_UNSET.as_bytes());
            return Err(1);
        }
        Some(t) if !image::OUTPUT_TYPES.contains(&t) => {
            print_notices(err, notices);
            let _ = writeln!(err, "{}", image::unknown_output_type_message(t));
            return Err(1);
        }
        Some(t) => t.to_owned(),
    };
    let stem = settings.name(files::strip_image_extension(out));
    let Some((name, version)) = image::output_file(&stem, &ot) else {
        return Err(refuse(
            err,
            notices,
            &format!("--out={out}: not an image name"),
        ));
    };
    let extensions = settings
        .get("FSL_LOAD_NIFTI_EXTENSIONS")
        .is_some_and(|v| atof(v) != 0.0);
    let mut header: NiftiHeader = first.header.clone();
    header.intent_code = 0;
    let o = Output {
        template: &header,
        flipped: first.flipped,
        shape: first.vol.shape,
        volumes: corrected.volumes.len(),
        cal: Some((0.0, 0.0)),
        extensions,
        defects: mode,
        n_threads: settings.threads(),
        descrip: DESCRIP,
        rewrite_quaternion: true,
    };
    let values: Vec<f32> = corrected.volumes.concat();
    let t = out_type.unwrap_or(DataType::F32);
    let v = match larmorx_mri::hmc::image::integer_range(t) {
        Some((lo, hi)) => {
            let (data, conv) =
                larmorx_mri::hmc::image::convert_values(&values, mode, lo, hi, |i| i as f64, |f| f);
            if conv.rounded + conv.out_of_range > 0 {
                defects::push_once(
                    notices,
                    larmorx_mri::hmc::image::fsl09_notice(
                        std::path::Path::new(&name),
                        conv,
                        t,
                        mode,
                    ),
                );
            }
            match t {
                DataType::U8 => Values::U8(data.iter().map(|&x| x as i64 as u8).collect()),
                DataType::I8 => Values::I8(data.iter().map(|&x| x as i64 as i8).collect()),
                DataType::I16 => Values::I16(data.iter().map(|&x| x as i64 as i16).collect()),
                _ => Values::I32(data.iter().map(|&x| x as i64 as i32).collect()),
            }
        }
        None if t == DataType::F64 => Values::F64(values.iter().map(|&x| f64::from(x)).collect()),
        None => Values::F32(values),
    };
    match image::write(&name, version, &o, v) {
        Ok(ns) => {
            for x in ns {
                defects::push_once(notices, x);
            }
            Ok(())
        }
        Err(e) => Err(refuse(
            err,
            notices,
            &format!("{name} cannot be written ({e})"),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `--topup`'s base name under a folder whose name holds a dot, with the platform's own
    /// separators (docs/api/fsl-topup.md, "File names on Windows"): no extension; `\` a
    /// separator on Windows only.
    #[test]
    fn extensions_under_a_folder_with_a_dot() {
        let dir = std::env::temp_dir().join("lx.applytopup").join("my_topup");
        assert!(!has_extension(dir.to_str().unwrap()));
        assert!(has_extension(dir.with_extension("nii").to_str().unwrap()));
        assert!(!has_extension("a.b/my_topup"));
        assert_eq!(has_extension("a.b\\my_topup"), !cfg!(windows));
    }
}
