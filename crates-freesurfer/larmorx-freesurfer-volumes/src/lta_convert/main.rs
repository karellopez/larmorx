// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `lta_convert`'s `main` (`lta_convert.cpp` 923-1083), `parseNextCommand` (1101-1373),
//! `parseCommandLine` (1383-1441) and `printUsage` (1086-1090): the program's entry point [`run`]
//! (registered as `lta_convert` in the crate's `PROGRAMS`) and [`main`].
//!
//! `lta_convert [options]` prints the version (`8.2.0` and an empty line), handles the version
//! options (`lta_convert freesurfer 8.2.0`; nothing else on the line: status 0), then reads the
//! options while the next argument starts with `-`: the name after one or two dashes, upper-cased
//! in place (so messages show it upper-cased), each printing what it set (`--inlta: <x> input LTA
//! transform.` and so on); `--help` the help (`lta_convert.help.xml`), status 1; an unknown one
//! `ERROR: Option: <arg> unknown (see --help) !! ` between empty lines on standard error, status
//! 1. No argument at all: the help, status 1. The input: `--inlta` (`--regheader`:
//! `identity.nofile`), `--infsl`, `--inmni`/`--inxfm`, `--inreg`, `--inniftyreg`/`--inras`,
//! `--inniftyreg2d`, `--initk`/`--inlps`, `--invox` (none: `<progname>: can't read input file `,
//! `ERROR_BADFILE`); then ` LTA read, type : 1` and the matrix (`MatrixPrint`); the source's and
//! target's geometry conformed (`--srcconform`, `-dc`, `-min`, `-size`; `--trgconform...`), up- or
//! downsampled (`--srcupsample N`, ...); `--invert` (the matrix inverted, the geometries swapped;
//! a warning when either is not valid); `--subject` (`setting subject to <s>`); `fscale` 0.1;
//! then the outputs: `--outlta` (`--ltavox2vox`, `--ltatkreg`: its type; `Writing  LTA to file
//! <name>...`, `LTAprint`; one that cannot be created: `<progname>: can't create file <name>`,
//! `ERROR_BADFILE`), `--sqrt` (the singular value decomposition printed), `--outfsl`, `--outmni`,
//! `--outreg`, `--outniftyreg`/`--outras`, `--outitk`/`--outlps`, `--outvox`
//! ([`super::formats`]); `<progname> successful.`, status 0.
//!
//! Defects (`larmorx_core::defects`; the mechanisms in `docs/findings/freesurfer-lta-convert.md`):
//! - **FS-1159**: an option that takes a value given last: FreeSurfer aborts (the options taking a
//!   name: `basic_string::_M_construct null not valid`, status 134) or crashes (the sampling
//!   factors and the conform sizes, status 139). By default `ERROR: Option: <arg> needs a value
//!   (see --help) !! `, status 1 (rule B).
//! - **FS-1160**: `--srcconform-size` and `--trgconform-size` read their value but do not step
//!   over it: the reading of the options stops at the value and every argument after it is
//!   ignored. By default the value is stepped over (rule A).
//! - **FS-1168**: a `--subject` of 1,024 bytes or more is copied past the transform array's room
//!   (memory corruption: long ones crash); the port keeps the whole name in both modes.
//! - **FS-1169**: `--sqrt` (which prints the matrix's singular value decomposition) leaves the
//!   decomposition's left factor in place of the transform: every output written after the LTA
//!   (`--outfsl`, `--outmni`, `--outreg`, `--outniftyreg`, `--outitk`, `--outvox`) holds it. By
//!   default the outputs keep the transform (rule A).
//! - **FS-1170**: an argument that does not start with `-` ends the reading of the options without
//!   a message: it and everything after it are ignored. By default `ERROR: Option: <arg> unknown
//!   (see --help) !! `, status 1 (rule B).
//! - FS-1161 to FS-1165: [`super::formats`]; FS-1166, FS-1167: [`super::geom`].

use std::io::Write;

use larmorx_core::defects::{FS_1159, FS_1160, FS_1168, FS_1169, FS_1170};
use larmorx_freesurfer_core::cstd::{atof, atoi};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::help_xml::output_help_xml;
use larmorx_freesurfer_core::log::{self, ERROR_BADFILE, Fatal, error_exit};
use larmorx_freesurfer_core::matrix::{MATRIX_REAL, Matrix, matrix_alloc};
use larmorx_freesurfer_core::matrix_decomp::matrix_svd;
use larmorx_freesurfer_core::matrix_ops::matrix_inverse;
use larmorx_freesurfer_core::ret::Ret;
use larmorx_freesurfer_core::run::{Environment, Settings};
use larmorx_freesurfer_core::system;
use larmorx_freesurfer_core::version::{get_version, handle_version_option};
use larmorx_freesurfer_io::cfile::CFile;
use larmorx_freesurfer_mri::lta::{LINEAR_RAS_TO_RAS, LINEAR_VOX_TO_VOX, Lta, REGISTER_DAT};
use larmorx_freesurfer_mri::vol_geom::VolGeom;
use larmorx_freesurfer_volume::ltaops::{lta_change_type, lta_print};

use super::cxx::put;
use super::formats::{
    CRASH, no_transform, not_invertible, read_fsl, read_itk, read_lta, read_mni, read_niftyreg,
    read_reg, read_vox, write_fsl, write_itk, write_mni, write_niftyreg, write_reg, write_vox,
};
use super::geom::{conform_geom, downsample_geom, print_matrix, upsample_geom};

/// The program's name as `argv[0]` gives it (recon-all, bbregister and fMRIPrep run it by name).
pub const NAME: &str = "lta_convert";

/// `lta_convert.help.xml`, the text `printUsage` renders (`outputHelpXml`).
pub const HELP_XML: &[u8] = include_bytes!("lta_convert.help.xml");

/// `STRLEN`: the transform array's subject buffer.
const STRLEN: usize = 1024;

/// libstdc++'s lines when a `std::string` is made from a null pointer (FS-1159, `reproduce`).
const NULL_STRING: &str = "terminate called after throwing an instance of 'std::logic_error'\n  what():  basic_string::_M_construct null not valid\n";

/// The program's streams, environment and working directory.
pub struct Run<'a, 'b> {
    pub out: &'a mut dyn Write,
    pub err: &'a mut dyn Write,
    pub env: &'b Environment,
    pub dir: Option<&'b std::path::Path>,
}

/// `lta_convert`'s entry point: its original arguments (without the program's name), the
/// settings, the two streams; the exit status. The notices of the run follow on standard error
/// (`larmorx: ...`).
pub fn run(args: &[String], settings: &Settings, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let mut g = Globals::new(settings.mode);
    // the process's own standard error, as the C library's perror sees it
    g.stderr.read_write = system::stderr_read_write();
    let status = {
        let mut terr = log::TrackedErr::new(&g, &mut *err);
        let mut r = Run {
            out: &mut *out,
            err: &mut terr,
            env: &settings.env,
            dir: settings.dir.as_deref(),
        };
        match main(&mut g, &mut r, args) {
            Ok(()) => 0,
            Err(f) => f.status(),
        }
    };
    let _ = out.flush();
    g.notices.print(err);
    status
}

/// The input's format (`intypes::InputType`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum InType {
    #[default]
    Unknown,
    Lta,
    Reg,
    Fsl,
    Mni,
    NiftyReg,
    Itk,
    Vox,
}

/// `struct Parameters` with its defaults, and `do_sqrt`.
#[derive(Clone, Debug, PartialEq)]
pub struct Params {
    pub transin: String,
    pub ltaout: String,
    pub fslout: String,
    pub mniout: String,
    pub regout: String,
    pub niftyregout: String,
    pub itkout: String,
    pub voxout: String,
    pub src: String,
    pub trg: String,
    pub dim: i32,
    pub invert: bool,
    pub ltaouttype: i32,
    pub trgconform: bool,
    pub trgconform_dc: bool,
    pub trgconform_min: bool,
    pub trgconform_size: f32,
    pub subject: String,
    pub intype: InType,
    pub srcconform: bool,
    pub srcconform_dc: bool,
    pub srcconform_min: bool,
    pub srcconform_size: f32,
    pub srcupsample: bool,
    pub srcdownsample: bool,
    pub srcupsample_factor: i32,
    pub srcdownsample_factor: i32,
    pub trgupsample: bool,
    pub trgdownsample: bool,
    pub trgupsample_factor: i32,
    pub trgdownsample_factor: i32,
    pub do_sqrt: bool,
}

impl Default for Params {
    fn default() -> Self {
        Params {
            transin: String::new(),
            ltaout: String::new(),
            fslout: String::new(),
            mniout: String::new(),
            regout: String::new(),
            niftyregout: String::new(),
            itkout: String::new(),
            voxout: String::new(),
            src: String::new(),
            trg: String::new(),
            dim: 3,
            invert: false,
            ltaouttype: LINEAR_RAS_TO_RAS,
            trgconform: false,
            trgconform_dc: false,
            trgconform_min: false,
            trgconform_size: 1.0,
            subject: String::new(),
            intype: InType::Unknown,
            srcconform: false,
            srcconform_dc: false,
            srcconform_min: false,
            srcconform_size: 1.0,
            srcupsample: false,
            srcdownsample: false,
            srcupsample_factor: 1,
            srcdownsample_factor: 1,
            trgupsample: false,
            trgdownsample: false,
            trgupsample_factor: 1,
            trgdownsample_factor: 1,
            do_sqrt: false,
        }
    }
}

/// `printUsage()`: the help of `lta_convert.help.xml`.
fn print_usage(g: &mut Globals, r: &mut Run) {
    output_help_xml(g, r.out, r.err, Some(HELP_XML));
}

/// How an option reads `argv[1]`: as a name (`std::string`), with `atoi` or with `atof`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Value {
    Name,
    Int,
    Float,
}

/// FS-1159: an option that takes a value is the last argument.
fn missing_value(g: &mut Globals, r: &mut Run, shown: &str, how: Value) -> Fatal {
    g.notices.add(
        &FS_1159,
        format!(
            "lta_convert: {shown} given last, without its value (FreeSurfer aborts or crashes)"
        ),
    );
    if g.notices.reproduce() {
        if how == Value::Name {
            let _ = r.err.write_all(NULL_STRING.as_bytes());
            return Fatal::Exit(134);
        }
        return Fatal::Exit(CRASH);
    }
    let _ = write!(
        r.err,
        "\n\nERROR: Option: {shown} needs a value (see --help) !! \n\n"
    );
    Fatal::Exit(1)
}

/// `parseNextCommand(argc, argv, P)` on `args` (`argv[0]` the option): the arguments it takes
/// after its own.
fn parse_next_command(
    g: &mut Globals,
    r: &mut Run,
    args: &[String],
    p: &mut Params,
) -> Result<usize, Fatal> {
    let mut raw = args[0].clone().into_bytes();
    let start = if raw.get(1) == Some(&b'-') { 2 } else { 1 };
    raw[start..].make_ascii_uppercase();
    let shown = String::from_utf8_lossy(&raw).into_owned();
    let option = &raw[start..];
    let value = |g: &mut Globals, r: &mut Run, how: Value| -> Result<String, Fatal> {
        match args.get(1) {
            Some(v) => Ok(v.clone()),
            None => Err(missing_value(g, r, &shown, how)),
        }
    };
    let mut nargs = 0;
    macro_rules! name_option {
        ($field:ident, $line:literal) => {{
            let v = value(g, r, Value::Name)?;
            let _ = writeln!(r.out, $line, v);
            p.$field = v;
            nargs = 1;
        }};
    }
    match option {
        b"INLTA" => {
            let v = value(g, r, Value::Name)?;
            p.transin = v;
            p.intype = InType::Lta;
            nargs = 1;
            let _ = writeln!(r.out, "--inlta: {} input LTA transform.", p.transin);
        }
        b"REGHEADER" => {
            p.transin = "identity.nofile".to_owned();
            p.intype = InType::Lta;
            let _ = writeln!(r.out, "--regheader");
        }
        b"INFSL" => {
            p.transin = value(g, r, Value::Name)?;
            p.intype = InType::Fsl;
            nargs = 1;
            let _ = writeln!(r.out, "--infsl: {} input FSL transform.", p.transin);
        }
        b"INMNI" | b"INXFM" => {
            p.transin = value(g, r, Value::Name)?;
            p.intype = InType::Mni;
            nargs = 1;
            let _ = writeln!(r.out, "--inmni: {} input MNI/XFM transform.", p.transin);
        }
        b"INREG" => {
            p.transin = value(g, r, Value::Name)?;
            p.intype = InType::Reg;
            nargs = 1;
            let _ = writeln!(r.out, "--inreg: {} input TK REG transform.", p.transin);
        }
        b"SUBJECT" => name_option!(subject, "--s: {} subject name"),
        b"SQRT" => {
            let _ = r.out.write_all(b" !!!!!!! WARNING !!!!!!!!!!\n");
            let _ = r
                .out
                .write_all(b" !!!! MATRIX SQRT not supported yet !!!!!!!!!!\n");
            p.do_sqrt = true;
            let _ = writeln!(r.out, "--sqrt: True ");
        }
        b"INNIFTYREG" | b"INRAS" => {
            p.transin = value(g, r, Value::Name)?;
            p.intype = InType::NiftyReg;
            nargs = 1;
            let _ = writeln!(
                r.out,
                "--inniftyreg: {} input NiftyReg transform.",
                p.transin
            );
        }
        b"INNIFTYREG2D" => {
            p.transin = value(g, r, Value::Name)?;
            p.intype = InType::NiftyReg;
            p.dim = 2;
            nargs = 1;
            let _ = writeln!(
                r.out,
                "--inniftyreg2d: {} input NiftyReg transform.",
                p.transin
            );
        }
        b"INITK" | b"INLPS" => {
            p.transin = value(g, r, Value::Name)?;
            p.intype = InType::Itk;
            nargs = 1;
            let _ = writeln!(r.out, "--initk: {} input ITK txt transform.", p.transin);
        }
        b"INVOX" => {
            p.transin = value(g, r, Value::Name)?;
            p.intype = InType::Vox;
            nargs = 1;
            let _ = writeln!(
                r.out,
                "--invox: {} input source voxel space transform.",
                p.transin
            );
        }
        b"OUTLTA" => name_option!(ltaout, "--outlta: {} output LTA."),
        b"OUTFSL" => name_option!(fslout, "--outfsl: {} output FSL matrix."),
        b"OUTMNI" => name_option!(mniout, "--outmni: {} output MNI/XFM matrix."),
        b"OUTREG" => name_option!(regout, "--outreg: {} output reg.dat matrix."),
        b"OUTNIFTYREG" | b"OUTRAS" => {
            name_option!(niftyregout, "--outniftyreg: {} output NiftyReg matrix.")
        }
        b"OUTITK" | b"OUTLPS" => name_option!(itkout, "--outitk: {} output ITK txt matrix."),
        b"OUTVOX" => name_option!(voxout, "--outvox: {} output source voxel space matrix."),
        b"SRC" => name_option!(src, "--src: {} src image (geometry)."),
        b"SRCCONFORM" => {
            p.srcconform = true;
            let _ = writeln!(r.out, "--srcconform:: will conform source geometry.");
        }
        b"SRCCONFORM-DC" => {
            p.srcconform = true;
            p.srcconform_dc = true;
            let _ = writeln!(r.out, "--srcconform-dc:: will keep source geometry dc.");
        }
        b"SRCCONFORM-MIN" => {
            p.srcconform = true;
            p.srcconform_min = true;
            let _ = writeln!(
                r.out,
                "--srcconform-min:: will conform source geometry to min direction size."
            );
        }
        b"SRCCONFORM-SIZE" => {
            p.srcconform = true;
            let v = value(g, r, Value::Float)?;
            p.srcconform_size = atof(v.as_bytes()) as f32;
            let _ = writeln!(
                r.out,
                "--srcconform-size: {} input src conform size.",
                put(f64::from(p.srcconform_size), 6)
            );
            nargs = conform_size_nargs(g, &shown);
        }
        b"SRCUPSAMPLE" => {
            let v = value(g, r, Value::Int)?;
            p.srcupsample = true;
            p.srcupsample_factor = atoi(v.as_bytes());
            nargs = 1;
            let _ = writeln!(
                r.out,
                "--srcupsample: {} input src upsample factor.",
                p.srcupsample_factor
            );
        }
        b"SRCDOWNSAMPLE" => {
            let v = value(g, r, Value::Int)?;
            p.srcdownsample = true;
            p.srcdownsample_factor = atoi(v.as_bytes());
            nargs = 1;
            let _ = writeln!(
                r.out,
                "--srcdownsample: {} input src downsample factor.",
                p.srcdownsample_factor
            );
        }
        b"TRG" => name_option!(trg, "--trg: {} trg image (geometry)."),
        b"TRGCONFORM" => {
            p.trgconform = true;
            let _ = writeln!(r.out, "--trgconform: will conform target geometry.");
        }
        b"TRGCONFORM-DC" => {
            p.trgconform = true;
            p.trgconform_dc = true;
            let _ = writeln!(r.out, "--trgconform-dc:: will keep target geometry dc.");
        }
        b"TRGCONFORM-MIN" => {
            p.trgconform = true;
            p.trgconform_min = true;
            let _ = writeln!(
                r.out,
                "--trgconform-min:: will conform target geometry to min direction size."
            );
        }
        b"TRGCONFORM-SIZE" => {
            p.trgconform = true;
            let v = value(g, r, Value::Float)?;
            p.trgconform_size = atof(v.as_bytes()) as f32;
            let _ = writeln!(
                r.out,
                "--trgconform-size: {} input trg conform size.",
                put(f64::from(p.trgconform_size), 6)
            );
            nargs = conform_size_nargs(g, &shown);
        }
        b"TRGUPSAMPLE" => {
            let v = value(g, r, Value::Int)?;
            p.trgupsample = true;
            p.trgupsample_factor = atoi(v.as_bytes());
            nargs = 1;
            let _ = writeln!(
                r.out,
                "--trgupsample: {} input trg upsample factor.",
                p.trgupsample_factor
            );
        }
        b"TRGDOWNSAMPLE" => {
            let v = value(g, r, Value::Int)?;
            p.trgdownsample = true;
            p.trgdownsample_factor = atoi(v.as_bytes());
            nargs = 1;
            let _ = writeln!(
                r.out,
                "--trgdownsample: {} input trg downsample factor.",
                p.trgdownsample_factor
            );
        }
        b"LTAVOX2VOX" => {
            p.ltaouttype = LINEAR_VOX_TO_VOX;
            let _ = writeln!(r.out, "--ltavox2vox: output LTA as VOX_TO_VOX transform.");
        }
        b"LTATKREG" => {
            p.ltaouttype = REGISTER_DAT;
            let _ = writeln!(r.out, "--ltatkreg: output LTA as REGISTER_DAT transform.");
        }
        b"INVERT" => {
            p.invert = true;
            let _ = writeln!(r.out, "--invert: will invert transform.");
        }
        b"HELP" => {
            print_usage(g, r);
            return Err(Fatal::Exit(1));
        }
        _ => {
            let _ = write!(
                r.err,
                "\n\nERROR: Option: {shown} unknown (see --help) !! \n\n"
            );
            return Err(Fatal::Exit(1));
        }
    }
    Ok(nargs)
}

/// FS-1160: the arguments `--srcconform-size` and `--trgconform-size` step over (FreeSurfer: none).
fn conform_size_nargs(g: &mut Globals, shown: &str) -> usize {
    g.notices.add(
        &FS_1160,
        if g.notices.reproduce() {
            format!("lta_convert: {shown} does not step over its value: the options after it are not read, as in FreeSurfer")
        } else {
            format!("lta_convert: {shown}'s value stepped over (FreeSurfer stops reading the options at it)")
        },
    );
    usize::from(g.notices.fix())
}

/// `parseCommandLine(argc, argv, P)` on the arguments after the program's name.
fn parse_command_line(g: &mut Globals, r: &mut Run, args: &[String]) -> Result<Params, Fatal> {
    let mut p = Params::default();
    let mut i = 0;
    while i < args.len() && args[i].starts_with('-') {
        let nargs = parse_next_command(g, r, &args[i..], &mut p)?;
        i += 1 + nargs;
    }
    if args.is_empty() {
        print_usage(g, r);
        return Err(Fatal::Exit(1));
    }
    if i < args.len() {
        let shown = &args[i];
        g.notices.add(
            &FS_1170,
            format!("lta_convert: the argument {shown:?} is not an option: it and those after it ignored (FreeSurfer says nothing)"),
        );
        if g.notices.fix() {
            let _ = write!(
                r.err,
                "\n\nERROR: Option: {shown} unknown (see --help) !! \n\n"
            );
            return Err(Fatal::Exit(1));
        }
    }
    Ok(p)
}

/// A transform array's first matrix, which the steps after the reading use.
fn first(lta: &Lta) -> Option<&Matrix> {
    lta.xforms.first().and_then(|lt| lt.m_l.as_ref())
}

/// `main(argc, argv)` on the arguments after the program's name. `Ok` is status 0.
pub fn main(g: &mut Globals, r: &mut Run, args: &[String]) -> Result<(), Fatal> {
    let _ = write!(r.out, "{}\n\n", get_version());
    let argv: Vec<String> = std::iter::once(NAME.to_owned())
        .chain(args.iter().cloned())
        .collect();
    let (nargs, left) = handle_version_option(g, r.env, r.out, &argv, NAME);
    if nargs != 0 && left.len() == 1 {
        return Err(Fatal::Exit(0));
    }
    g.progname = NAME.to_owned();
    // ErrorInit: FreeSurfer's initialisation, nothing seen outside (no DiagInit: the diagnostic
    // variables are not read)
    let p = parse_command_line(g, r, &left[1..])?;
    let progname = g.progname.clone();
    let mut lta = match p.intype {
        InType::Lta => read_lta(g, r, &p.transin, &p.src, &p.trg)?,
        InType::Fsl => read_fsl(g, r, &p.transin, &p.src, &p.trg)?,
        InType::Mni => read_mni(g, r, &p.transin, &p.src, &p.trg)?,
        InType::Reg => read_reg(g, r, &p.transin, &p.src, &p.trg)?,
        InType::NiftyReg => read_niftyreg(g, r, &p.transin, &p.src, &p.trg, p.dim)?,
        InType::Itk => read_itk(g, r, &p.transin, &p.src, &p.trg)?,
        InType::Vox => read_vox(g, r, &p.transin, &p.src, &p.trg)?,
        InType::Unknown => {
            error_exit(
                g,
                r.err,
                ERROR_BADFILE,
                &format!("{progname}: can't read input file {}", p.transin),
            )?;
            unreachable!("ErrorExit returns only for code 0");
        }
    };
    if lta.ltype != LINEAR_RAS_TO_RAS {
        let _ = writeln!(r.err, "ERROR: lta should be RAS_TO_RAS by now!!!");
        return Err(Fatal::Exit(1));
    }
    if lta.xforms.is_empty() {
        // an .lta without a transform, read without --src and --trg: FreeSurfer reads past it
        let _ = writeln!(r.out, " LTA read, type : {}", lta.ltype);
        return Err(no_transform(g, r, "readLTA", &p.transin));
    }
    let _ = writeln!(r.out, " LTA read, type : {}", lta.ltype);
    print_matrix(g, r, first(&lta));
    let lt = &mut lta.xforms[0];
    if p.srcconform {
        conform_geom(
            g,
            r,
            &mut lt.src,
            p.srcconform_min,
            p.srcconform_size,
            p.srcconform_dc,
        )?;
    }
    if p.srcupsample {
        upsample_geom(g, r, &mut lt.src, p.srcupsample_factor)?;
    } else if p.srcdownsample {
        downsample_geom(g, r, &mut lt.src, p.srcdownsample_factor)?;
    }
    if p.trgconform {
        conform_geom(
            g,
            r,
            &mut lt.dst,
            p.trgconform_min,
            p.trgconform_size,
            p.trgconform_dc,
        )?;
    }
    if p.trgupsample {
        upsample_geom(g, r, &mut lt.dst, p.trgupsample_factor)?;
    } else if p.trgdownsample {
        downsample_geom(g, r, &mut lt.dst, p.trgdownsample_factor)?;
    }
    if p.invert {
        let inv = match matrix_inverse(g, r.out, r.err, lt.m_l.as_ref(), None)? {
            Ret::New(m) => Some(m),
            _ => None,
        };
        lt.m_l = inv;
        if lt.m_l.is_none() {
            not_invertible(g, r, "--invert")?;
        }
        if lt.dst.valid == 0 || lt.src.valid == 0 {
            let _ = r.err.write_all(
                b"WARNING:********************************************************\n\
                  WARNING: dst or src volume is invalid.  Inverse likely wrong.\n\
                  WARNING:********************************************************\n",
            );
        }
        let mut vgtmp = VolGeom::default();
        vgtmp.assign(g, &lt.dst);
        let src = lt.src.clone();
        lt.dst.assign(g, &src);
        lt.src.assign(g, &vgtmp);
    }
    if !p.subject.is_empty() {
        let _ = writeln!(r.out, "setting subject to {}", p.subject);
        if p.subject.len() >= STRLEN {
            g.notices.add(
                &FS_1168,
                format!("lta_convert: a subject of {} bytes is copied past the transform array's room (1,024 bytes; memory corruption in FreeSurfer, long ones crash): kept whole", p.subject.len()),
            );
        }
        lta.subject.strcpy_bytes(p.subject.as_bytes());
    }
    lta.fscale = 0.1;
    if !p.ltaout.is_empty() {
        let env = r.env;
        if lta.ltype != p.ltaouttype {
            if lta.xforms[0].m_l.is_none() && g.notices.reproduce() {
                return Err(Fatal::Exit(CRASH));
            }
            lta_change_type(g, r.out, r.err, env, &mut lta, p.ltaouttype)?;
        }
        let _ = write!(r.out, "Writing  LTA to file {}...\n", p.ltaout);
        let mut fo = match CFile::open(r.dir, &p.ltaout, "w") {
            Ok(f) => f,
            Err(e) => {
                g.errno = e;
                error_exit(
                    g,
                    r.err,
                    ERROR_BADFILE,
                    &format!("{progname}: can't create file {}", p.ltaout),
                )?;
                unreachable!("ErrorExit returns only for code 0");
            }
        };
        if lta.xforms[0].m_l.is_none() {
            // FS-1164 with reproduce: LTAprint reads the missing matrix
            fo.close();
            return Err(Fatal::Exit(CRASH));
        }
        lta_print(g, r.err, &mut fo, &lta)?;
        fo.close();
    }
    if p.do_sqrt {
        sqrt(g, r, &mut lta)?;
    }
    if !p.fslout.is_empty() {
        write_fsl(g, r, &p.fslout, &lta)?;
    }
    if !p.mniout.is_empty() {
        write_mni(g, r, &p.mniout, &lta)?;
    }
    if !p.regout.is_empty() {
        write_reg(g, r, &p.regout, &lta)?;
    }
    if !p.niftyregout.is_empty() {
        write_niftyreg(g, r, &p.niftyregout, &lta)?;
    }
    if !p.itkout.is_empty() {
        write_itk(g, r, &p.itkout, &lta)?;
    }
    if !p.voxout.is_empty() {
        write_vox(g, r, &p.voxout, &lta)?;
    }
    let _ = writeln!(r.out, "{progname} successful.");
    Ok(())
}

/// `--sqrt`: `MatrixSVD` of the first matrix (which becomes its left factor `U` in FreeSurfer,
/// FS-1169) into a vector of its column count and a square matrix of its row count; then the
/// matrix, the singular values, `V` and the returned `V` again printed (`MatrixPrint`).
fn sqrt(g: &mut Globals, r: &mut Run, lta: &mut Lta) -> Result<(), Fatal> {
    let Some(m_l) = lta.xforms[0].m_l.as_mut() else {
        // FreeSurfer reads the missing matrix's size
        return Err(Fatal::Exit(CRASH));
    };
    let (rows, cols) = (m_l.rows, m_l.cols);
    let mut v_z = matrix_alloc(g, r.out, r.err, cols, 1, MATRIX_REAL)?.expect("a vector");
    let mut m_v = matrix_alloc(g, r.out, r.err, rows, rows, MATRIX_REAL)?.expect("a matrix");
    let keep = m_l.clone();
    let ret = matrix_svd(g, r.out, r.err, m_l, &mut v_z, Some(&mut m_v))?;
    let m_u = match ret {
        Ret::New(m) => Some(m),
        Ret::Given => Some(m_v.clone()),
        Ret::Null => None,
    };
    let u = lta.xforms[0].m_l.clone();
    print_matrix(g, r, u.as_ref());
    print_matrix(g, r, Some(&v_z));
    print_matrix(g, r, Some(&m_v));
    print_matrix(g, r, m_u.as_ref());
    if u.as_ref() != Some(&keep) {
        g.notices.add(
            &FS_1169,
            if g.notices.reproduce() {
                "lta_convert: --sqrt leaves the decomposition's left factor in place of the transform for the outputs after the LTA, as in FreeSurfer"
            } else {
                "lta_convert: --sqrt: the outputs after the LTA keep the transform (FreeSurfer writes the decomposition's left factor in its place)"
            },
        );
        if g.notices.fix() {
            lta.xforms[0].m_l = Some(keep);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_help_is_freesurfers() {
        assert!(HELP_XML.starts_with(b"<?xml"));
    }
}
