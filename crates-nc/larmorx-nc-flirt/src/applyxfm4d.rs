// SPDX-License-Identifier: LicenseRef-FSL
// Translated to Rust from FSL 6.0.7.23: fsl-flirt 2111.4 (applyxfm4D.cc) and fsl-newimage
// 2601.0 (fmribmain.h, the templated affine_transform). Copyright (C) 1999-2023 University of
// Oxford (Mark Jenkinson). Rust translation Copyright 2026 Karel Lopez Vilaret; modified from the
// originals. Distributed under the FSL Licence (see LICENSE).
//! `applyxfm4D`: every volume of a series resampled onto a reference grid through one matrix
//! (`-singlematrix`) or one matrix per volume (a directory of `MAT_0000n` files), or a 3D volume
//! with `-3D`; FSL's argument scan, messages and exit statuses, its output in the input's data
//! type.
//!
//! FSL templates the program on the input's type (`char`, `short`, `int`, `float`; `double` is
//! refused here, not translated): the values are interpolated in single precision from the
//! type's values (each converted to `float` where it is read, as the replica's float volume
//! holds them: exact for `char` and `short`, and for `int` the rounding FSL applies at every
//! read), the background value is the type's, and each result is cast to the type. Defects,
//! each with its switch (D13):
//! - FSL-09: an integer output is the interpolated value truncated toward zero (and wrapped out
//!   of range); rounded and clamped by default, FSL's cast with `reproduce`; an int8 input is
//!   written as uint8 (the type's bits kept), as int8 by default.
//! - FSL-75: the background value's buffer (as in `flirt`): the whole border by default, the
//!   values that fit FSL's buffer with `reproduce`; a 1 × 1 × 1 volume refused.
//! - FSL-80: newimage's sinc-kernel cache holds kernels of different widths the same, so
//!   `-3D`'s rectangular kernel of width 9 is replaced by the default blackman kernel of width
//!   7 defined just before it; the rectangular kernel by default, FSL's with `reproduce`.
//! - FSL-79: spline interpolation of an integer image stores its spline coefficients in the
//!   image's type (truncated toward zero); kept in single precision by default (the float
//!   image's coefficients), truncated with `reproduce`.

use std::io::Write;

use larmorx_core::defects::{self, Notice, UpstreamDefects};
use larmorx_nc_core::arma::Mat;
use larmorx_nc_core::interp::SplineCoefs;
use larmorx_nc_core::kernel;
use larmorx_nc_core::miscmaths::{self, MatrixFileError};
use larmorx_nc_core::nifti::{self, dt};
use larmorx_nc_core::newimagefns;
use larmorx_nc_core::settings::Settings;
use larmorx_nc_core::volume::{
    self, BorderBuffer, Conversion, Elem, Extrapolation, FslError, Interpolation, SaveError,
    SaveOptions, SaveType, Volume,
};

use crate::run::PREFIX;

/// The `descrip` of the images the replica writes.
const DESCRIP: &[u8] = b"larmorx-nc fsl applyxfm4D";

enum End {
    Exit(u8),
    Terminate {
        printed: String,
        type_name: &'static str,
        what: String,
    },
    Refuse(String),
}

impl From<FslError> for End {
    fn from(e: FslError) -> End {
        End::Terminate {
            printed: e.printed_text(),
            type_name: if e.type_name.is_empty() {
                "std::runtime_error"
            } else {
                e.type_name
            },
            what: e.what,
        }
    }
}

/// The interpolation methods (`interpolation`: nearestneighbour 0, trilinear 1, sinc 2,
/// spline 4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Interp {
    Nearest,
    Trilinear,
    Sinc,
    Spline,
}

impl Interp {
    fn code(self) -> i32 {
        match self {
            Interp::Nearest => 0,
            Interp::Trilinear => 1,
            Interp::Sinc => 2,
            Interp::Spline => 4,
        }
    }
}

struct Opts {
    iname: String,
    refname: String,
    oname: String,
    transname: String,
    usrinterp: String,
    matprefix: String,
    interp: Interp,
    singlematrix: bool,
    fourd: bool,
    verbose: bool,
}

fn usage() -> String {
    "Usage: applyxfm4D <input volume> <ref volume> <output volume> <transformation matrix file/dir>\n\n\
     \t--interp, -interp <nearestneighbour (or nn), trilinear, spline, sinc (default)>\n\
     \t--singlematrix, -singlematrix (flag option, do not provide an argument)\n\
     \t--fourdigit, -fourdigit (flag option, do not provide an argument)\n\
     \t--userprefix, -userprefix <prefix>\n\n"
        .to_owned()
}

/// `std::string(argv[i])` with `i == argc`: libstdc++'s null check throws.
fn null_string() -> End {
    End::Terminate {
        printed: String::new(),
        type_name: "std::logic_error",
        what: "basic_string: construction from null is not valid".to_owned(),
    }
}

fn parse(args: &[String], err: &mut String) -> Result<Opts, End> {
    let mut o = Opts {
        iname: args[0].clone(),
        refname: args[1].clone(),
        oname: args[2].clone(),
        transname: args[3].clone(),
        usrinterp: "sinc".to_owned(),
        matprefix: "/MAT_0".to_owned(),
        interp: Interp::Sinc,
        singlematrix: false,
        fourd: true,
        verbose: false,
    };
    let mut i = 4;
    while i < args.len() {
        match args[i].as_str() {
            "-singlematrix" | "--singlematrix" => o.singlematrix = true,
            "-fourdigit" | "--fourdigit" => o.matprefix = "/MAT_".to_owned(),
            "-userprefix" | "--userprefix" => {
                i += 1;
                let Some(p) = args.get(i) else {
                    return Err(null_string());
                };
                o.matprefix = format!("/{p}");
            }
            "-interp" | "--interp" => {
                i += 1;
                let Some(p) = args.get(i) else {
                    return Err(null_string());
                };
                o.usrinterp = p.clone();
                o.interp = match p.as_str() {
                    "nearestneighbour" | "nn" => Interp::Nearest,
                    "trilinear" => Interp::Trilinear,
                    "sinc" => Interp::Sinc,
                    "spline" => Interp::Spline,
                    _ => {
                        err.push_str(&format!(
                            "interpolation method: {p} is not a valid choice. Please choose from \
                             nearestneighbour, trilinear, sinc, or spline.\n"
                        ));
                        return Err(End::Exit(1));
                    }
                };
            }
            "-3D" | "--3D" | "-3d" | "--3d" => o.fourd = false,
            "--verbose" | "-verbose" | "-v" => o.verbose = true,
            _ => {}
        }
        i += 1;
    }
    Ok(o)
}

/// The matrix of a file, as `read_ascii_matrix` reads it (FSL prints the message and goes on
/// with an empty matrix when the file cannot be opened).
fn read_matrix(name: &str, err: &mut String) -> Result<Mat, End> {
    match miscmaths::read_ascii_matrix(name) {
        Ok(m) => Ok(m),
        Err(MatrixFileError::CannotOpen) => {
            err.push_str(&format!("Could not open matrix file {name}\n"));
            Ok(Mat::zeros(0, 0))
        }
        Err(MatrixFileError::NotEnoughNumbers) => Err(End::Terminate {
            printed: String::new(),
            type_name: "armawrap::AWException",
            what: "Could not find enough numbers in matrix file".to_owned(),
        }),
    }
}

/// `matname = transname + matprefix + four digits` (each `'0' + digit`, as chars).
fn mat_name(transname: &str, matprefix: &str, m: i64) -> String {
    let mut s = format!("{transname}{matprefix}");
    let mut n = m;
    for d in [1000, 100, 10, 1] {
        let q = n / d;
        s.push(char::from((b'0' as i64 + q).clamp(0, 255) as u8));
        n -= q * d;
    }
    s
}

struct Run<'a> {
    mode: UpstreamDefects,
    notices: Vec<Notice>,
    err: &'a mut String,
}

impl Run<'_> {
    fn notice(&mut self, d: &'static defects::Defect, detail: String) {
        defects::push_once(&mut self.notices, Notice::new(d, self.mode, detail));
    }

    /// The background value of `vin` (`backgroundval()`) with FSL-75's switch.
    fn padvalue(&mut self, vin: &Volume) -> Result<f32, End> {
        match vin.background_buffer() {
            BorderBuffer::Empty => Err(End::Refuse(
                "the background value of a 1 x 1 x 1 volume: FSL reads an empty buffer (FSL-75)"
                    .to_owned(),
            )),
            BorderBuffer::Overflow { numbins, written } => {
                let (fsl, _) = vin.background_value_with(false);
                let (whole, _) = vin.background_value_with(true);
                if fsl.to_bits() != whole.to_bits() {
                    self.notice(
                        &defects::FSL_75,
                        format!(
                            "the background value's buffer holds {numbins} of the border's \
                             {written} values: {}",
                            if self.mode.fix() {
                                format!("the whole border's value {whole} is used (FSL: {fsl})")
                            } else {
                                format!("the value of those that fit, {fsl}, as FSL")
                            }
                        ),
                    );
                }
                Ok(if self.mode.fix() { whole } else { fsl })
            }
            BorderBuffer::Exact => Ok(vin.backgroundval()),
        }
    }

    /// `affine_transform(vin, vout, aff, 0, true)` for a volume of the input's type held in
    /// single precision: the background value as padding, extraslice extrapolation.
    fn transform(
        &mut self,
        vin: &Volume,
        vout: &mut Volume,
        aff: &Mat,
        integer: Option<SaveType>,
    ) -> Result<(), End> {
        if aff.rows() != 4 || aff.cols() != 4 {
            return Err(End::Terminate {
                printed: String::new(),
                type_name: "std::logic_error",
                what: format!(
                    "matrix multiplication: incompatible matrix dimensions: {}x{} and 4x4",
                    aff.rows(),
                    aff.cols()
                ),
            });
        }
        let mut src = vin.clone();
        src.padvalue = self.padvalue(vin)?;
        src.extrap = Extrapolation::Extraslice;
        let coefs = if src.interp == Interpolation::Spline {
            Some(self.spline_coefs(&src, integer))
        } else {
            None
        };
        newimagefns::raw_affine_transform_coefs(&src, vout, aff, coefs.as_ref())?;
        newimagefns::affine_transform_mask(&src, vout, aff, 0.0, src.padvalue)?;
        Ok(())
    }

    /// The spline coefficients: FSL-79 for an integer image.
    fn spline_coefs(&mut self, src: &Volume, integer: Option<SaveType>) -> SplineCoefs {
        let float = SplineCoefs::new(src);
        let Some(ty) = integer else {
            return float;
        };
        let stored = SplineCoefs::new_stored(src, &|c: f64| match ty {
            SaveType::Char => f64::from(i8::from_f64(c)),
            SaveType::Short => f64::from(i16::from_f64(c)),
            _ => f64::from(i32::from_f64(c)),
        });
        if !float.same_as(&stored) {
            self.notice(
                &defects::FSL_79,
                if self.mode.fix() {
                    "the spline coefficients of the integer image are kept in single precision"
                        .to_owned()
                } else {
                    "the spline coefficients are truncated to the image's integer type, as FSL"
                        .to_owned()
                },
            );
        }
        if self.mode.fix() { float } else { stored }
    }
}

/// The output values in the input's type: `(T) value` (FSL's cast) or FSL-09's rounding.
fn to_type<T: Elem>(v: &Volume, conv: Conversion) -> Vec<T> {
    v.data
        .iter()
        .map(|&x| match conv {
            Conversion::X86Cast => T::from_f32(x),
            Conversion::RoundClamp => T::from_f64(round_clamp::<T>(x)),
        })
        .collect()
}

/// FSL-09's correction: rounded to the nearest (ties to even) and clamped to `T`'s range; NaN
/// is 0.
fn round_clamp<T: Elem>(x: f32) -> f64 {
    let (lo, hi) = match T::DTYPE {
        dt::UNSIGNED_CHAR => (-128.0, 127.0),
        dt::SIGNED_SHORT => (-32768.0, 32767.0),
        _ => (-2147483648.0, 2147483647.0),
    };
    if x.is_nan() {
        0.0
    } else {
        f64::from(x).round_ties_even().clamp(lo, hi)
    }
}

fn typed_main<T: Elem>(
    o: &Opts,
    settings: &Settings,
    integer: Option<SaveType>,
    int8_input: bool,
    r: &mut Run,
    out: &mut String,
) -> Result<(), End> {
    let set_interp = |v: &mut Volume, msgs: &mut Vec<String>, window: &str, w: i32| {
        v.extrap = Extrapolation::Extraslice;
        v.interp = match o.interp {
            Interp::Nearest => Interpolation::NearestNeighbour,
            Interp::Trilinear => Interpolation::Trilinear,
            Interp::Sinc => Interpolation::Sinc,
            Interp::Spline => Interpolation::Spline,
        };
        if o.interp == Interp::Sinc {
            v.kernel = Some(std::sync::Arc::new(kernel::sinc_kernel(window, w, 1201, msgs)));
        }
    };
    let conv = if integer.is_some() && r.mode.fix() {
        Conversion::RoundClamp
    } else {
        Conversion::X86Cast
    };
    let mut fsl09 = false;
    let mut check09 = |v: &Volume| {
        if integer.is_some() && !fsl09 {
            let a: Vec<T> = to_type::<T>(v, Conversion::X86Cast);
            let b: Vec<T> = to_type::<T>(v, Conversion::RoundClamp);
            if a.iter().zip(&b).any(|(x, y)| x.to_f64().to_bits() != y.to_f64().to_bits()) {
                fsl09 = true;
            }
        }
    };
    let result: Volume<T>;
    if o.fourd {
        let (inv_t, _, _) = volume::read_volume4d::<T>(&o.iname, settings)?;
        let mut invol = Volume::<f32>::copyconvert(&inv_t);
        drop(inv_t);
        if o.verbose {
            out.push_str(&format!(
                "using interpolation method (enum, string): {}, {}\n",
                o.interp.code(),
                o.usrinterp
            ));
        }
        let mut msgs = Vec::new();
        set_interp(&mut invol, &mut msgs, "b", 7);
        for m in &msgs {
            r.err.push_str(m);
            r.err.push('\n');
        }
        let (ref_t, warning) = volume::read_volume::<T>(&o.refname, settings)?;
        if let Some(w) = warning {
            r.err.push_str(&w);
            r.err.push('\n');
        }
        let refvol = Volume::<f32>::copyconvert(&ref_t);
        let mut affmat = Mat::eye(4);
        if o.singlematrix {
            affmat = read_matrix(&o.transname, r.err)?;
        }
        let (mint, maxt) = (0i64, invol.tsize() - 1);
        if maxt - mint > 10000 {
            r.err.push_str("WARNING:: More than 10000 volumes - only doing first 10000\n");
        }
        let last = maxt.min(mint + 10000);
        let count = (last - mint + 1).max(0);
        let mut outvol = Volume::<T>::new(
            refvol.xsize(),
            refvol.ysize(),
            refvol.zsize(),
            count.max(1),
        );
        let mut first = true;
        for m in mint..=last {
            if !o.singlematrix {
                let matname = mat_name(&o.transname, &o.matprefix, m);
                out.push_str(&matname);
                out.push('\n');
                affmat = read_matrix(&matname, r.err)?;
            }
            let mut dummy = refvol.clone();
            let vin = invol.sub_volume(m)?;
            r.transform(&vin, &mut dummy, &affmat, integer)?;
            check09(&dummy);
            let mut typed = Volume::<T>::copyconvert(&dummy);
            typed.data = to_type::<T>(&dummy, conv);
            if first {
                outvol.copy_properties(&typed);
                first = false;
            }
            outvol.set_sub_volume(m - mint, &typed)?;
        }
        outvol.pixdims[3] = invol.tdim().abs();
        result = outvol;
    } else {
        let (in_t, warning) = volume::read_volume::<T>(&o.iname, settings)?;
        if let Some(w) = warning {
            r.err.push_str(&w);
            r.err.push('\n');
        }
        let mut invol = Volume::<f32>::copyconvert(&in_t);
        let (out_t, warning) = volume::read_volume::<T>(&o.refname, settings)?;
        if let Some(w) = warning {
            r.err.push_str(&w);
            r.err.push('\n');
        }
        let mut outvol = Volume::<f32>::copyconvert(&out_t);
        let mut msgs = Vec::new();
        // FSL-80: setinterpolationmethod(sinc) first defines newimage's default kernel
        // (blackman, width 7), and the kernel cache's comparison holds kernels of other widths
        // the same, so definesincinterpolation("r", 9) gets the cached blackman kernel back.
        // The rectangular kernel of width 9 by default; FSL's blackman 7 with reproduce.
        if o.interp == Interp::Sinc {
            let fix = r.mode.fix();
            r.notice(
                &defects::FSL_80,
                if fix {
                    "-3D's sinc kernel is the rectangular one of width 9 the program defines \
                     (applyxfm4D uses the default blackman kernel of width 7)"
                        .to_owned()
                } else {
                    "-3D's sinc kernel is the cached blackman kernel of width 7, as in \
                     applyxfm4D (not the rectangular one of width 9 it defines)"
                        .to_owned()
                },
            );
            if fix {
                set_interp(&mut invol, &mut msgs, "r", 9);
            } else {
                set_interp(&mut invol, &mut msgs, "b", 7);
            }
        } else {
            set_interp(&mut invol, &mut msgs, "r", 9);
        }
        for m in &msgs {
            r.err.push_str(m);
            r.err.push('\n');
        }
        let affmat = read_matrix(&o.transname, r.err)?;
        r.transform(&invol, &mut outvol, &affmat, integer)?;
        check09(&outvol);
        outvol.pixdims[3] = in_t.tdim().abs();
        let mut typed = Volume::<T>::copyconvert(&outvol);
        typed.data = to_type::<T>(&outvol, conv);
        result = typed;
    }
    if fsl09 {
        r.notice(
            &defects::FSL_09,
            if r.mode.fix() {
                "the integer output is rounded and clamped (applyxfm4D truncates toward zero)"
                    .to_owned()
            } else {
                "integer outputs are truncated toward zero, as in applyxfm4D".to_owned()
            },
        );
    }
    let mut result = result;
    result.display_max = 0.0;
    result.display_min = 0.0;
    let opts = SaveOptions {
        description: DESCRIP,
        units: nifti::UNITS_MM_SEC,
        reproduce_offset: r.mode.reproduce(),
    };
    if volume::offset_defect_applies(&result, settings) {
        r.notice(
            &defects::FSL_56,
            format!(
                "{} NIfTI extensions of different sizes: the data offset counts each extension's \
                 own size (NewNifti adds the first one's once per extension)",
                result.extensions.len()
            ),
        );
    }
    let saved = if int8_input && r.mode.fix() {
        // FSL-09: an int8 input stays int8 (FSL writes its char volume as uint8).
        r.notice(
            &defects::FSL_09,
            "int8 data are written as int8 (applyxfm4D writes them as uint8)".to_owned(),
        );
        let f = Volume::<f32>::copyconvert(&result);
        f.save_typed(&o.oname, SaveType::Int8, Conversion::X86Cast, settings, &opts, None)
    } else {
        if int8_input {
            r.notice(
                &defects::FSL_09,
                "int8 data are written as uint8, as in applyxfm4D".to_owned(),
            );
        }
        result.save_volume(&o.oname, settings, &opts).map(|_| ())
    };
    match saved {
        Ok(()) => Ok(()),
        Err(SaveError::OutputType(e)) => {
            for l in &e.printed {
                r.err.push_str(l);
                r.err.push('\n');
            }
            Err(End::Exit(1))
        }
        Err(SaveError::Exception(e)) => Err(e.into()),
    }
}

fn body(
    args: &[String],
    settings: &Settings,
    out: &mut String,
    r: &mut Run,
) -> Result<(), End> {
    if args.len() < 4 {
        r.err.push_str(&usage());
        return Err(End::Exit(255));
    }
    let o = parse(args, r.err)?;
    let file_type = volume::file_dtype(&o.iname, settings)?;
    let t = match volume::closest_templated_type(file_type) {
        Ok(t) => t,
        Err(_) => {
            // call_fmrib_main returns -1 for a type it has no template for.
            return Err(End::Exit(255));
        }
    };
    let int8 = file_type == dt::INT8;
    match t {
        dt::UNSIGNED_CHAR => typed_main::<i8>(&o, settings, Some(SaveType::Char), int8, r, out),
        dt::SIGNED_SHORT => typed_main::<i16>(&o, settings, Some(SaveType::Short), false, r, out),
        dt::SIGNED_INT => typed_main::<i32>(&o, settings, Some(SaveType::Int), false, r, out),
        dt::FLOAT => typed_main::<f32>(&o, settings, None, false, r, out),
        _ => Err(End::Refuse(
            "a double-precision input: applyxfm4D's double template (its interpolation in double \
             precision) is not translated"
                .to_owned(),
        )),
    }
}

/// `applyxfm4D <input volume> <ref volume> <output volume> <transformation matrix file/dir>
/// [options]`.
pub fn run(
    args: &[String],
    settings: &Settings,
    mode: UpstreamDefects,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let (mut o, mut e) = (String::new(), String::new());
    let mut r = Run {
        mode,
        notices: Vec::new(),
        err: &mut e,
    };
    let res = body(args, settings, &mut o, &mut r);
    let notices = std::mem::take(&mut r.notices);
    drop(r);
    let code = match res {
        Ok(()) => 0,
        Err(End::Exit(c)) => c,
        Err(End::Terminate {
            printed,
            type_name,
            what,
        }) => {
            e.push_str(&printed);
            e.push_str(&format!(
                "terminate called after throwing an instance of '{type_name}'\n  what():  {what}\n"
            ));
            e.push_str(&format!(
                "{PREFIX}: applyxfm4D aborts here (signal 6); the replica exits with status 1\n"
            ));
            1
        }
        Err(End::Refuse(what)) => {
            e.push_str(&format!("{PREFIX}: error: {what}\n"));
            1
        }
    };
    for n in &notices {
        e.push_str(&format!("{PREFIX}: {}\n", n.line()));
    }
    let _ = out.write_all(o.as_bytes());
    let _ = err.write_all(e.as_bytes());
    let _ = out.flush();
    let _ = err.flush();
    code
}
