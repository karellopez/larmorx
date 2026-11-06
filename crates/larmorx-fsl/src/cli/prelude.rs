// SPDX-License-Identifier: Apache-2.0
//! `larmorx fsl prelude`: FSL's phase unwrapping (PRELUDE), the clean-room original written from
//! `specs/prelude.md` (§2 the command line, §3 the images, §4.1 to §4.3 and §4.10 the series,
//! §4.11 and §5 the messages, §9 the defects), Jenkinson 2003 and black-box runs of FSL
//! 6.0.7.23's program. The work on each volume is [`crate::prelude`].

use std::io::Write;

use larmorx_core::defects::{self, Notice, UpstreamDefects};
use larmorx_core::math::Libm;
use larmorx_io::nifti::NiftiHeader;
use larmorx_mri::hmc::files::{self, Compression, ImageFiles};
use larmorx_mri::hmc::image::{ImageError, is_neurological, read_stored_files, scaling};
use rayon::prelude::*;

use super::Settings;
use super::bet2::{FSLOUTPUTTYPE_UNSET, resolve_image};
use super::common::{abort, image_exception, print_notices};
use super::fmt::g;
use super::options::{self, Given, Spec};
use crate::bet::image::{self as bimage, Output, Values};
use crate::prelude::num::{extrema_finite, extrema_fsl, wrap};
use crate::prelude::volume::{self, Mode, Params, Rules, Threshold, Undefined, VolumeResult};

/// FSL 6.0.7.23's usage text (§2.3).
pub const USAGE: &str = "\nPart of FSL (ID: 2412.6-dirty)\nprelude (Version 2.0 in c# minor)\nPhase Region Expanding Labeller for Unwrapping Discrete Estimates\nCopyright(c) 2001, University of Oxford (Mark Jenkinson)\n\nUsage: \nprelude -c <rawphase> -o <unwrappedphase> [options]\nprelude -p <phasevol> -a <absvol> -o <unwrappedphase> [options]\n\nCompulsory arguments (You MUST set one or more of):\n\t-o,--out,-u,--unwrap\tfilename for saving the unwrapped phase output\n\nOptional arguments (You may optionally specify one or more of):\n\t-c,--complex\tfilename of complex phase input volume\n\t-a,--abs\tfilename of absolute input volume\n\t-p,--phase\tfilename of raw phase input volume\n\t-n,--numphasesplit\tnumber of phase partitions to use\n\t--labelslices\tdoes label processing in 2D (slice at a time)\n\t-s,--slices\tdoes all processing in 2D (slice at a time)\n\t-f,--force3D\tforces all processing to be full 3D\n\t-t,--thresh\tintensity threshold for masking\n\t-m,--mask\tfilename of mask input volume\n\t--start\tfirst image number to process (default 0)\n\t--end\tfinal image number to process (default Inf)\n\t--savemask\tfilename for saving the mask volume\n\t-r,--rawphase\tfilename for saving the raw phase output\n\t-l,--labels\tfilename for saving the area labels output\n\t--removeramps\tremove phase ramps during unwrapping\n\t-v,--verbose\tswitch on diagnostic messages\n\t-h,--help\tdisplay this message\n\n\n";

/// The compulsory-options block (§2.4, check 3), printed before the usage.
const COMPULSORY: &str = "***************************************************\nThe following COMPULSORY options have not been set:\n\t-o,--out,-u,--unwrap\tfilename for saving the unwrapped phase output\n***************************************************\n";

/// The `descrip` field of the images larmorx's prelude writes.
pub const DESCRIP: &str = "larmorx fsl prelude";

const fn spec(short: Option<char>, long: Option<&'static str>, kind: u8) -> Spec {
    // kind: 0 flag, 1 text, 2 integer, 3 float
    Spec {
        short,
        long,
        values: if kind == 0 { 0 } else { 1 },
        numeric: kind >= 2,
        integer: kind == 2,
    }
}

const COMPLEX: usize = 0;
const ABS: usize = 1;
const PHASE: usize = 2;
const OUT: usize = 3;
const UNWRAP: usize = 4;
const SPLIT: usize = 5;
const LABEL_SLICES: usize = 6;
const SLICES: usize = 7;
const FORCE3D: usize = 8;
const THRESH: usize = 9;
const MASK: usize = 10;
const START: usize = 11;
const END: usize = 12;
const SAVEMASK: usize = 13;
const RAW: usize = 14;
const LABELS: usize = 15;
const RAMPS: usize = 16;
const VERBOSE: usize = 17;
const HELP: usize = 18;

const SPECS: [Spec; 19] = [
    spec(Some('c'), Some("complex"), 1),
    spec(Some('a'), Some("abs"), 1),
    spec(Some('p'), Some("phase"), 1),
    spec(Some('o'), Some("out"), 1),
    spec(Some('u'), Some("unwrap"), 1),
    spec(Some('n'), Some("numphasesplit"), 2),
    spec(None, Some("labelslices"), 0),
    spec(Some('s'), Some("slices"), 0),
    spec(Some('f'), Some("force3D"), 0),
    spec(Some('t'), Some("thresh"), 3),
    spec(Some('m'), Some("mask"), 1),
    spec(None, Some("start"), 2),
    spec(None, Some("end"), 2),
    spec(None, Some("savemask"), 1),
    spec(Some('r'), Some("rawphase"), 1),
    spec(Some('l'), Some("labels"), 1),
    spec(None, Some("removeramps"), 0),
    spec(Some('v'), Some("verbose"), 0),
    spec(Some('h'), Some("help"), 0),
];

/// prelude's options as given.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Options {
    pub complex: Option<String>,
    pub abs: Option<String>,
    pub phase: Option<String>,
    pub out: String,
    pub n: i32,
    pub label_slices: bool,
    pub slices: bool,
    pub force3d: bool,
    pub thresh: Option<f32>,
    pub mask: Option<String>,
    pub start: Option<i64>,
    pub end: Option<i64>,
    pub savemask: Option<String>,
    pub raw: Option<String>,
    pub labels: Option<String>,
    pub ramps: bool,
    pub verbose: bool,
}

/// The result of reading the command line.
pub enum Parsed {
    Run(Box<Options>),
    Exit(u8),
}

/// Reads the command line and its checks (§2.1, §2.4).
pub fn parse(args: &[String], err: &mut dyn Write) -> Parsed {
    let usage_then = |err: &mut dyn Write, msg: &str| {
        let _ = err.write_all(USAGE.as_bytes());
        let _ = writeln!(err, "\n{msg}");
        Parsed::Exit(1)
    };
    let all = match options::parse_all(args, &SPECS, false) {
        Ok((g, _)) => g,
        Err(e) => return usage_then(err, &e.0),
    };
    let mut given: Vec<Option<Given>> = vec![None; SPECS.len()];
    let mut out: Option<String> = None;
    for (k, g) in all {
        if k == OUT || k == UNWRAP {
            if let Given::Text(t) = &g {
                out = Some(t.clone());
            }
        }
        given[k] = Some(g);
    }
    if given[HELP].is_some() {
        let _ = err.write_all(USAGE.as_bytes());
        return Parsed::Exit(1);
    }
    let Some(out) = out else {
        let _ = err.write_all(COMPULSORY.as_bytes());
        let _ = err.write_all(USAGE.as_bytes());
        return Parsed::Exit(1);
    };
    let text = |k: usize| match &given[k] {
        Some(Given::Text(t)) => Some(t.clone()),
        _ => None,
    };
    let int = |k: usize| match &given[k] {
        Some(Given::Numbers(v)) => Some(v[0] as i64),
        _ => None,
    };
    let flag = |k: usize| given[k].is_some();
    let o = Options {
        complex: text(COMPLEX),
        abs: text(ABS),
        phase: text(PHASE),
        out,
        n: int(SPLIT).map_or(8, |v| v as i32),
        label_slices: flag(LABEL_SLICES),
        slices: flag(SLICES),
        force3d: flag(FORCE3D),
        thresh: match &given[THRESH] {
            Some(Given::Numbers(v)) => Some(v[0] as f32),
            _ => None,
        },
        mask: text(MASK),
        start: int(START),
        end: int(END),
        savemask: text(SAVEMASK),
        raw: text(RAW),
        labels: text(LABELS),
        ramps: flag(RAMPS),
        verbose: flag(VERBOSE),
    };
    if o.complex.is_some() && (o.phase.is_some() || o.abs.is_some()) {
        return usage_then(err, "Cannot specify both --complex AND --phase or --abs.");
    }
    if o.phase.is_some() != o.abs.is_some() {
        return usage_then(err, "Both --phase AND --abs must be used together.");
    }
    if o.n < 2 {
        return usage_then(
            err,
            &format!("Always set --numphasesplit greater than 1.\nNOT {}", o.n),
        );
    }
    Parsed::Run(Box::new(o))
}

/// Runs prelude.
pub fn main(args: &[String], settings: &Settings, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let o = match parse(args, err) {
        Parsed::Run(o) => o,
        Parsed::Exit(code) => return code,
    };
    let mode = match settings.defects() {
        Ok(m) => m,
        Err(e) => {
            let _ = writeln!(err, "larmorx fsl prelude: {e} (larmorx)");
            return 1;
        }
    };
    let mut notices = Vec::new();
    let code = run(&o, settings, mode, &mut notices, out, err);
    print_notices(err, &notices);
    code
}

/// An image series as prelude holds it: every volume in single precision, in FSL's radiological
/// order (x fastest, then y, z, volumes).
struct Series {
    header: NiftiHeader,
    flipped: bool,
    shape: [usize; 3],
    volumes: usize,
    data: Vec<f32>,
}

impl Series {
    fn per(&self) -> usize {
        self.shape[0] * self.shape[1] * self.shape[2]
    }

    fn volume(&self, v: usize) -> &[f32] {
        let n = self.per();
        &self.data[v * n..(v + 1) * n]
    }
}

/// Why an image could not be read.
enum ReadFail {
    /// FSL's image layer's exception: the lines it prints and the exception's message.
    Exception(String, String),
    /// A complex image given where a real one is read (`Fslread: DT <code> not supported`).
    Complex(i16),
    /// larmorx's refusal (`-c` with another datatype, FSL-263).
    Refused(String),
}

/// The raw datatype code of the image `files` (its header), if it can be read.
fn datatype_code(settings: &Settings, name: &str, mode: UpstreamDefects) -> Option<i16> {
    let mut ns = Vec::new();
    super::common::read_nifti_header(settings, name, mode, &mut ns)
        .ok()
        .map(|h| h.datatype)
}

/// Reads the image `name` whole (every volume), scaled to single precision (§3.1).
fn read_real(
    settings: &Settings,
    name: &str,
    mode: UpstreamDefects,
    notices: &mut Vec<Notice>,
    n_threads: usize,
) -> Result<Series, ReadFail> {
    let exception = |e: ImageError| {
        let (lines, what) = image_exception(name, &e);
        ReadFail::Exception(lines, what)
    };
    if name.is_empty() {
        return Err(exception(ImageError::Invalid(
            "No image files match: ".into(),
        )));
    }
    let files = resolve_image(settings, name, mode, notices).map_err(exception)?;
    if let Some(code @ (32 | 1792 | 2048)) = datatype_code(settings, name, mode) {
        return Err(ReadFail::Complex(code));
    }
    if bimage::analyze_header(&files) {
        let (vols, input) = bimage::read(&files, n_threads).map_err(exception)?;
        let shape = input.vol.shape;
        let volumes = vols.len();
        let data: Vec<f32> = vols.into_iter().flat_map(|v| v.data).collect();
        return Ok(Series {
            header: input.header,
            flipped: input.flipped,
            shape,
            volumes,
            data,
        });
    }
    let (header, array) = read_stored_files(&files, n_threads).map_err(exception)?;
    let dims = array.shape().to_vec();
    let shape = [0, 1, 2].map(|k| dims.get(k).copied().unwrap_or(1));
    let per = shape.iter().product::<usize>();
    if per == 0 {
        return Err(exception(ImageError::Invalid(format!(
            "{}: the image has no voxels",
            files.header.display()
        ))));
    }
    let scale = scaling(&header);
    let flipped = is_neurological(&header);
    let mut data = to_f32_series(array, scale);
    let volumes = data.len() / per;
    if flipped && shape[0] > 1 {
        data.par_chunks_mut(shape[0]).for_each(|row| row.reverse());
    }
    Ok(Series {
        header,
        flipped,
        shape,
        volumes,
        data,
    })
}

/// The stored values in single precision, scaled as FSL's image layer scales them (`v × slope +
/// inter` in single precision); a float32 array without scaling is taken as it is (no copy).
fn to_f32_series(array: larmorx_core::DynArray, scale: Option<(f32, f32)>) -> Vec<f32> {
    use larmorx_core::element::RealElement;
    fn convert<T: RealElement + Sync>(
        a: &larmorx_core::ndarray::ArrayD<T>,
        scale: Option<(f32, f32)>,
    ) -> Vec<f32> {
        let t = a.t();
        let owned: Vec<T>;
        let src: &[T] = match t.as_slice() {
            Some(s) => s,
            None => {
                owned = t.iter().copied().collect();
                &owned
            }
        };
        src.par_iter()
            .with_min_len(1 << 16)
            .map(|v| match scale {
                None => v.to_f32(),
                Some((s, i)) => v.to_f32() * s + i,
            })
            .collect()
    }
    if scale.is_none()
        && let larmorx_core::DynArray::F32(a) = array
    {
        if a.t().is_standard_layout() {
            let n = a.len();
            let (v, off) = a.into_raw_vec_and_offset();
            let off = off.unwrap_or(0);
            return if off == 0 && v.len() == n {
                v
            } else {
                v[off..off + n].to_vec()
            };
        }
        return convert(&a, None);
    }
    larmorx_core::dispatch_real_dyn_array!(
        &array,
        a => convert(a, scale),
        complex => Vec::new()
    )
}

/// FSL's datatype names in the refusal of FSL-263.
fn type_name(code: i16) -> String {
    match code {
        2 => "uint8".into(),
        4 => "int16".into(),
        8 => "int32".into(),
        16 => "float32".into(),
        64 => "float64".into(),
        256 => "int8".into(),
        512 => "uint16".into(),
        768 => "uint32".into(),
        1024 => "int64".into(),
        1280 => "uint64".into(),
        2048 => "complex256".into(),
        c => format!("datatype {c}"),
    }
}

/// Reads the complex image `name` (§3.2): its phase (`atan2f`) and magnitude series, the data
/// unscaled; FSL-263 to FSL-265 by `mode`; GLIBC-07 named where the arctangents differ.
fn read_complex(
    settings: &Settings,
    name: &str,
    mode: UpstreamDefects,
    notices: &mut Vec<Notice>,
) -> Result<(Series, Series), ReadFail> {
    let exception = |e: ImageError| {
        let (lines, what) = image_exception(name, &e);
        ReadFail::Exception(lines, what)
    };
    if name.is_empty() {
        return Err(exception(ImageError::Invalid(
            "No image files match: ".into(),
        )));
    }
    let files = resolve_image(settings, name, mode, notices).map_err(exception)?;
    let header =
        super::common::read_nifti_header(settings, name, mode, notices).map_err(exception)?;
    let code = header.datatype;
    let dim = |k: usize| -> usize {
        if (header.dim[0] as usize) >= k {
            usize::try_from(header.dim[k]).unwrap_or(0).max(1)
        } else {
            1
        }
    };
    let shape = [dim(1), dim(2), dim(3)];
    let volumes = dim(4);
    let per = shape.iter().product::<usize>();
    let n = per * volumes;
    let complex = code == 32 || code == 1792;
    if !complex && mode.fix() {
        return Err(ReadFail::Refused(format!(
            "the complex input {name} is a {} image, not a complex one (FSL-263)",
            type_name(code)
        )));
    }
    let size = match larmorx_io::nifti::datatype::from_code(code) {
        Some(t) => t.size(),
        None => {
            return Err(ReadFail::Refused(format!(
                "the complex input {name} is a {} image, whose values cannot be decoded (FSL-263)",
                type_name(code)
            )));
        }
    };
    let bytes = data_bytes(&files, &header, n * size).map_err(exception)?;
    let swap = !header.byte_order.is_native();
    let (mut re, mut im): (Vec<f32>, Vec<f32>) = (vec![0.0; n], vec![0.0; n]);
    match code {
        32 => {
            let word = |c: &[u8]| {
                let mut b = [c[0], c[1], c[2], c[3]];
                if swap {
                    b.reverse();
                }
                f32::from_ne_bytes(b)
            };
            for (i, c) in bytes.chunks_exact(8).take(n).enumerate() {
                re[i] = word(&c[..4]);
                im[i] = word(&c[4..]);
            }
            if swap {
                let detail = match mode {
                    UpstreamDefects::Fix => format!(
                        "{name} is a complex image in the other byte order: each part swapped on its own (FSL swaps each value's 8 bytes as one, exchanging the real and imaginary parts)"
                    ),
                    UpstreamDefects::Reproduce => format!(
                        "{name} is a complex image in the other byte order: its real and imaginary parts exchanged, as prelude reads them"
                    ),
                };
                if mode.reproduce() {
                    std::mem::swap(&mut re, &mut im);
                }
                defects::push_once(notices, Notice::new(&defects::FSL_264, mode, detail));
            }
        }
        1792 => {
            if mode.fix() {
                let word = |c: &[u8]| {
                    let mut b = [c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7]];
                    if swap {
                        b.reverse();
                    }
                    crate::maths::num::to_f32(f64::from_ne_bytes(b))
                };
                for (i, c) in bytes.chunks_exact(16).take(n).enumerate() {
                    re[i] = word(&c[..8]);
                    im[i] = word(&c[8..]);
                }
            }
            let detail = match mode {
                UpstreamDefects::Fix => format!(
                    "{name} is a complex128 image: its values read, each part rounded to single precision (prelude reads none of them)"
                ),
                UpstreamDefects::Reproduce => format!(
                    "{name} is a complex128 image: every value 0, prelude reading none of them"
                ),
            };
            defects::push_once(notices, Notice::new(&defects::FSL_265, mode, detail));
        }
        _ => {
            // FSL-263 reproduced: the decoded values' bytes as consecutive floats
            let mut decoded = bytes;
            if swap && size > 1 {
                decoded.par_chunks_mut(size).for_each(|c| c.reverse());
            }
            let floats = decoded.len() / 4;
            for i in 0..n.min(floats) {
                let c = &decoded[4 * i..4 * i + 4];
                re[i] = f32::from_le_bytes([c[0], c[1], c[2], c[3]]);
            }
            let detail = format!(
                "the complex input {name} is a {} image: its bytes taken as single-precision real parts ({} of {n} values beyond its data set to 0) and every imaginary part 0, in place of the memory prelude reads",
                type_name(code),
                n.saturating_sub(floats)
            );
            defects::push_once(notices, Notice::new(&defects::FSL_263, mode, detail));
        }
    }
    let libm = Libm::new(mode);
    let (mut phase, mut mag): (Vec<f32>, Vec<f32>) = re
        .par_iter()
        .zip(im.par_iter())
        .with_min_len(1 << 14)
        .map(|(&r, &i)| {
            let rr = crate::maths::num::nan32(r, r, r * r);
            let ii = crate::maths::num::nan32(i, i, i * i);
            let s = crate::maths::num::nan32(rr, ii, rr + ii);
            (libm.atan2f(i, r), s.sqrt())
        })
        .unzip();
    drop((re, im));
    if libm.differed() {
        defects::push_once(
            notices,
            Notice::new(
                &defects::GLIBC_07,
                mode,
                format!(
                    "prelude's phase of {name} (atan2f): the correctly rounded arctangent and glibc 2.35's differ on some voxels"
                ),
            ),
        );
    }
    let flipped = is_neurological(&header);
    if flipped && shape[0] > 1 {
        phase.par_chunks_mut(shape[0]).for_each(|r| r.reverse());
        mag.par_chunks_mut(shape[0]).for_each(|r| r.reverse());
    }
    let mut h = header;
    // the voxel sizes as read: their magnitudes, a zero kept
    for k in 1..4 {
        h.pixdim[k] = h.pixdim[k].abs();
    }
    let series = |data: Vec<f32>| Series {
        header: h.clone(),
        flipped,
        shape,
        volumes,
        data,
    };
    Ok((series(phase), series(mag)))
}

/// The data bytes of an image (`want` of them, fewer when the file ends before), decompressed.
fn data_bytes(files: &ImageFiles, h: &NiftiHeader, want: usize) -> Result<Vec<u8>, ImageError> {
    let load = |p: &std::path::Path| -> Result<Vec<u8>, ImageError> {
        let c = Compression::of_file(p)
            .map_err(|e| ImageError::Invalid(format!("{}: {e}", p.display())))?;
        std::fs::read(p)
            .and_then(|raw| c.decompress(raw))
            .map_err(|e| ImageError::Invalid(format!("{}: {e}", p.display())))
    };
    let (mut bytes, offset) = match &files.data {
        Some(d) => (load(d)?, 0usize),
        None => {
            let off = if h.vox_offset.is_finite() && h.vox_offset >= 0.0 {
                h.vox_offset as usize
            } else {
                0
            };
            (load(&files.header)?, off)
        }
    };
    let end = (offset + want).min(bytes.len());
    let start = offset.min(end);
    bytes.truncate(end);
    bytes.drain(..start);
    Ok(bytes)
}

/// FSL's abort lines and larmorx's exit status for a reading failure.
fn read_failed(err: &mut dyn Write, f: ReadFail) -> u8 {
    match f {
        ReadFail::Exception(lines, what) => {
            let _ = err.write_all(lines.as_bytes());
            abort(err, "prelude", "std::runtime_error", &what)
        }
        ReadFail::Complex(code) => {
            let what = format!("Fslread: DT {code} not supported");
            let _ = writeln!(err, "Image Exception : #8 :: {what}");
            abort(err, "prelude", "std::runtime_error", &what)
        }
        ReadFail::Refused(why) => refuse(err, &why),
    }
}

/// larmorx's refusal (§9): `larmorx: error: prelude: <what>`, exit status 1.
fn refuse(err: &mut dyn Write, why: &str) -> u8 {
    let _ = writeln!(err, "larmorx: error: prelude: {why}");
    1
}

/// FSL's exception for a volume index outside the series (§4.1).
fn bad_index(err: &mut dyn Write) -> u8 {
    let what = "Invalid t index in [] operator";
    let _ = writeln!(err, "Image Exception : #61 :: {what}");
    abort(err, "prelude", "std::runtime_error", what)
}

/// What §4.2 decides for a volume from its extrema.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Range {
    /// The range error, with the range.
    Error(f32),
    Rewrap,
    Neither,
}

fn range_of(ext: Option<(f32, f32)>) -> Range {
    let pi = std::f64::consts::PI;
    match ext {
        Some((mx, mn)) => {
            let r = mx - mn;
            if f64::from(r) > 3.0 * pi {
                Range::Error(r)
            } else if f64::from(mx) > 1.001 * pi || f64::from(mn) < -1.001 * pi {
                Range::Rewrap
            } else {
                Range::Neither
            }
        }
        None => Range::Neither,
    }
}

/// Writes prelude's images (§3.3), `FSLOUTPUTTYPE` checked at the first write.
struct Writer<'a> {
    settings: &'a Settings,
    mode: UpstreamDefects,
    n_threads: usize,
    extensions: bool,
}

/// Why a write stopped the run.
enum WriteFail {
    /// `FSLOUTPUTTYPE` unset or unknown (FSL's message printed).
    OutputType,
    /// The file could not be created (its name as FSL names it).
    Open(String),
}

impl Writer<'_> {
    #[allow(clippy::too_many_arguments)]
    fn put(
        &self,
        stem: &str,
        header: &NiftiHeader,
        flipped: bool,
        shape: [usize; 3],
        volumes: usize,
        values: Values,
        notices: &mut Vec<Notice>,
        err: &mut dyn Write,
    ) -> Result<(), WriteFail> {
        let ot = match self.settings.get("FSLOUTPUTTYPE") {
            None => {
                let _ = err.write_all(FSLOUTPUTTYPE_UNSET.as_bytes());
                return Err(WriteFail::OutputType);
            }
            Some(t) if !bimage::OUTPUT_TYPES.contains(&t) => {
                let _ = writeln!(err, "{}", bimage::unknown_output_type_message(t));
                return Err(WriteFail::OutputType);
            }
            Some(t) => t.to_owned(),
        };
        let stem_as_given = files::strip_image_extension(stem);
        let full = self.settings.name(stem_as_given);
        let (Some((name, version)), Some((shown, _))) = (
            bimage::output_file(&full, &ot),
            bimage::output_file(stem_as_given, &ot),
        ) else {
            return Err(WriteFail::OutputType);
        };
        let o = Output {
            template: header,
            flipped,
            shape,
            volumes,
            cal: None,
            extensions: self.extensions,
            defects: self.mode,
            n_threads: self.n_threads,
            descrip: DESCRIP,
            rewrite_quaternion: true,
        };
        match bimage::write(&name, version, &o, values) {
            Ok(ns) => {
                for x in ns {
                    defects::push_once(notices, x);
                }
                Ok(())
            }
            Err(_) => Err(WriteFail::Open(shown)),
        }
    }
}

/// The exit status of a failed write.
fn write_failed(err: &mut dyn Write, f: WriteFail) -> u8 {
    match f {
        WriteFail::OutputType => 1,
        WriteFail::Open(name) => abort(
            err,
            "prelude",
            "NiftiIO::NiftiException",
            &format!("Error: cant open file {name}"),
        ),
    }
}

/// The mask's values as written (0 or 1, float32).
fn mask_values(m: &[u8]) -> Vec<f32> {
    m.par_iter().map(|&v| f32::from(v)).collect()
}

/// Whether two results differ in anything the run writes or prints.
fn differs(a: &VolumeResult, b: &VolumeResult, o: &Options) -> bool {
    let bits = |x: &[f32], y: &[f32]| {
        x.len() != y.len()
            || x.par_iter()
                .zip(y.par_iter())
                .any(|(p, q)| p.to_bits() != q.to_bits())
    };
    bits(&a.unwrapped, &b.unwrapped)
        || (o.raw.is_some() && bits(&a.phase, &b.phase))
        || (o.savemask.is_some() && a.mask != b.mask)
        || (o.labels.is_some() && a.labels != b.labels)
        || (o.verbose && (a.ramp_lines != b.ramp_lines || a.merge_lines != b.merge_lines))
}

/// Runs prelude after its command line's checks.
fn run(
    o: &Options,
    settings: &Settings,
    mode: UpstreamDefects,
    notices: &mut Vec<Notice>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let n_threads = settings.threads();
    let say = |out: &mut dyn Write, line: &str| {
        if o.verbose {
            let _ = writeln!(out, "{line}");
        }
    };
    // §3.1, §3.2: the images (`Loading volumes` before the phase; nothing before a complex
    // image, observed)
    let pool = |f: Box<dyn FnOnce() -> Result<(Series, Option<Series>), ReadFail> + Send + '_>| {
        larmorx_core::parallel::with_threads(n_threads, f)
            .unwrap_or_else(|e| Err(ReadFail::Refused(format!("no thread pool: {e}"))))
    };
    let (mut phase, mag) = if let Some(c) = &o.complex {
        let mut ns = Vec::new();
        let r = pool(Box::new(|| {
            read_complex(settings, c, mode, &mut ns).map(|(p, m)| (p, Some(m)))
        }));
        notices.extend(ns);
        match r {
            Ok((p, m)) => (p, m.expect("the complex image's magnitude")),
            Err(f) => return read_failed(err, f),
        }
    } else {
        say(out, "Loading volumes");
        let mut ns = Vec::new();
        let name = o.phase.clone().unwrap_or_default();
        let r = pool(Box::new(|| {
            read_real(settings, &name, mode, &mut ns, 0).map(|p| (p, None))
        }));
        notices.extend(ns);
        let p = match r {
            Ok((p, _)) => p,
            Err(f) => return read_failed(err, f),
        };
        say(out, "Phase loaded");
        let mut ns = Vec::new();
        let name = o.abs.clone().unwrap_or_default();
        let r = pool(Box::new(|| {
            read_real(settings, &name, mode, &mut ns, 0).map(|m| (m, None))
        }));
        notices.extend(ns);
        let m = match r {
            Ok((m, _)) => m,
            Err(f) => return read_failed(err, f),
        };
        say(out, "Magnitude loaded");
        (p, m)
    };
    let mask_image = match &o.mask {
        Some(name) => {
            let mut ns = Vec::new();
            let r = pool(Box::new(|| {
                read_real(settings, name, mode, &mut ns, 0).map(|m| (m, None))
            }));
            notices.extend(ns);
            match r {
                Ok((m, _)) => {
                    say(out, "Mask loaded");
                    Some(m)
                }
                Err(f) => return read_failed(err, f),
            }
        }
        None => None,
    };
    let writer = Writer {
        settings,
        mode,
        n_threads,
        extensions: settings
            .get("FSL_LOAD_NIFTI_EXTENSIONS")
            .is_some_and(|v| super::options::atof(v) != 0.0),
    };
    // §4.1: the volumes processed
    let t = phase.volumes as i64;
    let s = o.start.unwrap_or(0);
    let e = o.end.unwrap_or(t - 1);
    let inside = |v: i64| (0..t).contains(&v);
    if e < s {
        if !inside(s) {
            return bad_index(err);
        }
        if mode.fix() {
            return refuse(
                err,
                &format!("no volume to process: --end ({e}) is before --start ({s}) (FSL-270)"),
            );
        }
        defects::push_once(
            notices,
            Notice::new(
                &defects::FSL_270,
                mode,
                format!(
                    "--end ({e}) is before --start ({s}): no volume processed, no unwrapped output written, as in prelude"
                ),
            ),
        );
        if let Some(r) = &o.raw {
            let data = std::mem::take(&mut phase.data);
            if let Err(f) = writer.put(
                r,
                &phase.header,
                phase.flipped,
                phase.shape,
                phase.volumes,
                Values::F32(data),
                notices,
                err,
            ) {
                return write_failed(err, f);
            }
        }
        return 0;
    }
    // §4.2: each volume's range, the rewrap
    let per = phase.per();
    for v in s..=e {
        if !inside(v) {
            return bad_index(err);
        }
        let vol = &mut phase.data[v as usize * per..(v as usize + 1) * per];
        let fsl = range_of(extrema_fsl(vol));
        let fixed = if vol.first().is_some_and(|x| x.is_nan()) {
            range_of(extrema_finite(vol))
        } else {
            fsl
        };
        if fsl != fixed {
            let what = match fixed {
                Range::Error(_) => "the range check",
                _ => "the rewrap to [-pi,pi]",
            };
            let detail = match mode {
                UpstreamDefects::Fix => format!(
                    "volume {v}: its first phase value is NaN: {what} taken over its other values (prelude skips it)"
                ),
                UpstreamDefects::Reproduce => format!(
                    "volume {v}: its first phase value is NaN: {what} skipped, as in prelude"
                ),
            };
            defects::push_once(notices, Notice::new(&defects::FSL_272, mode, detail));
        }
        match if mode.fix() { fixed } else { fsl } {
            Range::Error(r) => {
                let _ = writeln!(
                    err,
                    "ERROR: input phase image exceeds allowable phase range.\nAllowable range is 6.283 radians.  Image range is: {} radians.\nAborting.",
                    g(f64::from(r))
                );
                return 255;
            }
            Range::Rewrap => {
                say(out, "Rewrapping phase range to [-pi,pi]");
                let _ = larmorx_core::parallel::with_threads(n_threads, || {
                    vol.par_iter_mut()
                        .with_min_len(1 << 14)
                        .for_each(|x| *x = wrap(*x));
                });
            }
            Range::Neither => {}
        }
    }
    // §4.3: 2D or 3D, from the first processed volume's grid
    let [nx, ny, _] = phase.shape;
    let vmode = if o.force3d {
        Mode::ThreeD
    } else if o.slices {
        Mode::Slices
    } else if nx > 64 || ny > 64 || o.label_slices {
        Mode::Labels2D
    } else {
        Mode::ThreeD
    };
    // FSL-271: the mask's source on another grid
    let source: &Series = mask_image.as_ref().unwrap_or(&mag);
    if source.shape != phase.shape {
        let which = if mask_image.is_some() {
            "mask"
        } else {
            "magnitude"
        };
        if mode.fix() {
            return refuse(
                err,
                &format!("the {which} image has another size than the phase image (FSL-271)"),
            );
        }
        defects::push_once(
            notices,
            Notice::new(
                &defects::FSL_271,
                mode,
                format!(
                    "the {which} image ({}) has another size than the phase image ({}): its mask read at the phase image's voxel indices, as prelude reads it",
                    dims_text(source.shape),
                    dims_text(phase.shape)
                ),
            ),
        );
    }
    let threshold = match (o.thresh, &mask_image) {
        (Some(t), _) => Threshold::Given(t),
        (None, Some(_)) => Threshold::Given(0.5),
        (None, None) => Threshold::Default,
    };
    let rules = Rules::all(mode.fix());
    let params = Params {
        n: o.n,
        mode: vmode,
        threshold,
        ramps: o.ramps,
        verbose: o.verbose,
        rules,
        keep_ramp_free: mode.reproduce() && o.verbose && o.ramps,
    };
    if o.verbose && o.ramps {
        let detail = match mode {
            UpstreamDefects::Fix => {
                "-v with --removeramps: the ramp-free phase not written to grot in the working directory (prelude writes it there)"
            }
            UpstreamDefects::Reproduce => {
                "-v with --removeramps: the ramp-free phase written to grot in the working directory, as prelude does"
            }
        };
        defects::push_once(notices, Notice::new(&defects::FSL_266, mode, detail));
    }
    let processed = (e - s + 1) as usize;
    let mut unwrapped: Vec<f32> = Vec::with_capacity(processed * per);
    let mut masks: Vec<u8> = Vec::new();
    let mut labels: Vec<i32> = Vec::new();
    let mut mask_shape = source.shape;
    let say_lines = |out: &mut dyn Write, lines: &[String]| {
        for l in lines {
            let _ = writeln!(out, "{l}");
        }
    };
    for v in s as usize..=e as usize {
        let src: &[f32] = match &mask_image {
            Some(m) => m.volume(v.min(m.volumes - 1)),
            None => {
                if v >= mag.volumes {
                    return bad_index(err);
                }
                mag.volume(v)
            }
        };
        let vol = phase.volume(v).to_vec();
        let computed = larmorx_core::parallel::with_threads(n_threads, || {
            compute(vol, phase.shape, src, source.shape, &params, o, mode, v)
        });
        let (result, extra) = match computed {
            Ok(Ok(r)) => r,
            Ok(Err(u)) => {
                return refuse(
                    err,
                    &format!(
                        "volume {v}: {} ({}: FSL reads memory it does not own here)",
                        u.what, u.defect
                    ),
                );
            }
            Err(e) => return refuse(err, &format!("no thread pool: {e}")),
        };
        for n in extra {
            defects::push_once(notices, n);
        }
        mask_shape = result.mask_shape;
        // FSL's order of writes and messages (§3.3, §4.11)
        if mode.reproduce()
            && let Some(m) = &o.savemask
            && let Err(f) = writer.put(
                m,
                &source.header,
                source.flipped,
                result.mask_shape,
                1,
                Values::F32(mask_values(&result.mask)),
                notices,
                err,
            )
        {
            return write_failed(err, f);
        }
        say_lines(out, &result.ramp_lines);
        if let Some(free) = &result.ramp_free
            && let Err(f) = writer.put(
                "grot",
                &phase.header,
                phase.flipped,
                phase.shape,
                1,
                Values::F32(free.clone()),
                notices,
                err,
            )
        {
            return write_failed(err, f);
        }
        say(out, &format!("Number of phase splits = {}", o.n));
        if mode.reproduce()
            && let Some(l) = &o.labels
            && let Err(f) = writer.put(
                l,
                &phase.header,
                phase.flipped,
                phase.shape,
                1,
                Values::I32(result.labels.clone()),
                notices,
                err,
            )
        {
            return write_failed(err, f);
        }
        say_lines(out, &result.merge_lines);
        if o.ramps {
            say(out, "Removed linear ramps");
        }
        unwrapped.extend_from_slice(&result.unwrapped);
        phase.data[v * per..(v + 1) * per].copy_from_slice(&result.phase);
        if mode.fix() {
            if o.savemask.is_some() {
                masks.extend_from_slice(&result.mask);
            }
            if o.labels.is_some() {
                labels.extend_from_slice(&result.labels);
            }
        }
    }
    if processed > 1 && (o.savemask.is_some() || o.labels.is_some()) {
        let detail = match mode {
            UpstreamDefects::Fix => format!(
                "the masks and labels of the {processed} volumes written as series (prelude keeps only the last volume's)"
            ),
            UpstreamDefects::Reproduce => format!(
                "only the last of the {processed} volumes' masks and labels written, as prelude does"
            ),
        };
        defects::push_once(notices, Notice::new(&defects::FSL_262, mode, detail));
    }
    // the series at the end (FSL-262's correction), then the unwrapped and the raw phase
    if mode.fix() {
        if let Some(m) = &o.savemask
            && let Err(f) = writer.put(
                m,
                &source.header,
                source.flipped,
                mask_shape,
                processed,
                Values::F32(mask_values(&masks)),
                notices,
                err,
            )
        {
            return write_failed(err, f);
        }
        drop(masks);
        if let Some(l) = &o.labels
            && let Err(f) = writer.put(
                l,
                &phase.header,
                phase.flipped,
                phase.shape,
                processed,
                Values::I32(std::mem::take(&mut labels)),
                notices,
                err,
            )
        {
            return write_failed(err, f);
        }
    }
    if let Err(f) = writer.put(
        &o.out,
        &phase.header,
        phase.flipped,
        phase.shape,
        processed,
        Values::F32(unwrapped),
        notices,
        err,
    ) {
        return write_failed(err, f);
    }
    if let Some(r) = &o.raw {
        let data = std::mem::take(&mut phase.data);
        if let Err(f) = writer.put(
            r,
            &phase.header,
            phase.flipped,
            phase.shape,
            phase.volumes,
            Values::F32(data),
            notices,
            err,
        ) {
            return write_failed(err, f);
        }
    }
    0
}

/// `nx x ny x nz`.
fn dims_text(s: [usize; 3]) -> String {
    format!("{} x {} x {}", s[0], s[1], s[2])
}

/// One volume's result by `mode`'s rules, and the notices of FSL-261, FSL-268 and FSL-269: each
/// defect named where its correction changes anything the run writes or prints (the other
/// rule's result computed too).
#[allow(clippy::too_many_arguments)]
fn compute(
    phase: Vec<f32>,
    shape: [usize; 3],
    src: &[f32],
    src_shape: [usize; 3],
    p: &Params,
    o: &Options,
    mode: UpstreamDefects,
    v: usize,
) -> Result<(VolumeResult, Vec<Notice>), Undefined> {
    let mut notices = Vec::new();
    let alternative = |rules: Rules| {
        let q = Params {
            rules,
            keep_ramp_free: false,
            ..*p
        };
        volume::process(phase.clone(), shape, src, src_shape, &q)
    };
    // FSL-268: NaN values in the mask's source
    if src.par_iter().any(|x| x.is_nan()) {
        let slices = p.mode == Mode::Slices;
        let ours = volume::make_mask(src, src_shape, p.threshold, slices, p.rules.nan_source);
        let other = volume::make_mask(src, src_shape, p.threshold, slices, !p.rules.nan_source);
        if ours.is_ok() && ours != other {
            let detail = match mode {
                UpstreamDefects::Fix => format!(
                    "volume {v}: the NaN values of the mask's source left out of its threshold and maximum (prelude's mask differs{})",
                    if other.is_err() {
                        ": its sort reads memory it does not own"
                    } else {
                        ""
                    }
                ),
                UpstreamDefects::Reproduce => format!(
                    "volume {v}: the NaN values of the mask's source taken into its threshold and maximum, as prelude does"
                ),
            };
            notices.push(Notice::new(&defects::FSL_268, mode, detail));
        }
    }
    let result = volume::process(phase.clone(), shape, src, src_shape, p)?;
    // FSL-269: NaN phases inside the mask
    if result.nan_in_mask {
        let other = alternative(Rules {
            nan_phase: !p.rules.nan_phase,
            ..p.rules
        });
        if other.as_ref().map_or(true, |b| differs(&result, b, o)) {
            let detail = match mode {
                UpstreamDefects::Fix => format!(
                    "volume {v}: the NaN phases inside the mask left out of it, the unwrapped phase NaN there (prelude labels them and moves their regions by about 2^31 cycles)"
                ),
                UpstreamDefects::Reproduce => format!(
                    "volume {v}: the NaN phases inside the mask labelled and merged, as prelude does"
                ),
            };
            notices.push(Notice::new(&defects::FSL_269, mode, detail));
        }
    }
    // FSL-261: the ramps' domain
    if p.ramps && result.mask_on_edge {
        let other = alternative(Rules {
            ramps_everywhere: !p.rules.ramps_everywhere,
            ..p.rules
        });
        if other.as_ref().map_or(true, |b| differs(&result, b, o)) {
            let detail = match mode {
                UpstreamDefects::Fix => format!(
                    "volume {v}: the linear ramps estimated, removed and restored over every voxel (prelude leaves out the last column, row and slice)"
                ),
                UpstreamDefects::Reproduce => format!(
                    "volume {v}: the linear ramps without the last column, row and slice, as prelude does"
                ),
            };
            notices.push(Notice::new(&defects::FSL_261, mode, detail));
        }
    }
    Ok((result, notices))
}
