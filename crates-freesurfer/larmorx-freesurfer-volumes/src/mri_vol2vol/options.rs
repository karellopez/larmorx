// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_vol2vol`'s options: `parse_commandline` (`mri_vol2vol.cpp` 1316-1688), `check_options`
//! (2148-2280), `dump_options` (2282-2308), `print_usage` (1695-1797), `print_help` (1799-2145),
//! `print_version`, `argnerr`, `singledash`, `nth_is_arg`, `istringnmatch`, `LoadRtal` and
//! `LoadRfsl` (2370-2420).
//!
//! The options are read in order, each compared with FreeSurfer's chain: most by their whole name
//! in any case (`strcasecmp`, `istringnmatch` with 0); `--talres` (8), `--crop` (6), `--slice-crop`
//! (12), `--interp` (8), `--trilin` (6), `--nearest` (7), `--sd` (4) and `--ctab` (4) by their
//! first characters, in any case (so `--ct...` is `--ctab`); `--vsm-pedir`, `--copy-ctab`, `--mul`,
//! `--vg-thresh`, `--filter` and `--gdiagno` exactly. An option without the arguments it needs ends
//! with `ERROR: <opt> flag needs <n> argument(s)` on standard error, status 255; an unknown one with
//! `ERROR: Option <opt> unknown` (and `       Did you really mean -<opt> ?` for one dash), status
//! 255. Numbers are read with `sscanf` (a value that does not read leaves the setting as it was).
//! Some options read their files at once: `--reg` (`regio_read_register`, then the file again as an
//! `.lta`), `--lta`/`--lta-inv`, `--fsl` (one or two FSL matrices), `--xfm`, `--vsm-reg`, `--soap`.
//!
//! Defects (`larmorx_core::defects`; the mechanisms in `docs/findings/freesurfer-mri-vol2vol.md`):
//! FS-1177 to FS-1184: [`super::main`] and [`check_options`].

use std::path::Path;

use larmorx_core::defects::{FS_1177, FS_1180, FS_1183};
use larmorx_freesurfer_core::cmdargs::{cmd_arg_n_err, cmd_is_flag, cmd_nth_is_arg};
use larmorx_freesurfer_core::cscan::Scanf;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{ERROR_NOFILE, Fatal, error_exit};
use larmorx_freesurfer_core::matrix::{MATRIX_REAL, Matrix, matrix_alloc};
use larmorx_freesurfer_core::matrix_ops::{matrix_identity, matrix_inverse};
use larmorx_freesurfer_core::utils::{file_name_extension, stricmp};
use larmorx_freesurfer_core::version::get_version;
use larmorx_freesurfer_gca::regdat::transform_lta2_reg_dat;
use larmorx_freesurfer_gca::transform::GcaLtaIo;
use larmorx_freesurfer_io::cfile::CFile;
use larmorx_freesurfer_io::fio::{fio_dirname, fio_file_exists_readable};
use larmorx_freesurfer_io::identify::MRI_VOLUME_TYPE_UNKNOWN;
use larmorx_freesurfer_io::mriio::{mri_read_header, mri_read_volume};
use larmorx_freesurfer_io::registerio::{
    FLT2INT_ROUND, Register, regio_read_mincxfm, regio_read_register, regio_read_registermat,
};
use larmorx_freesurfer_mri::lta::{LINEAR_RAS_TO_RAS, Lta, REGISTER_DAT};
use larmorx_freesurfer_mri::mri::{
    MRI_FLOAT, MRI_UCHAR, Mri, mri_alloc_from_vol_geom, mri_interp_code, mri_precision_code,
    mri_precision_string,
};
use larmorx_freesurfer_volume::ltaapply::lta_modify_src_dst_geom;
use larmorx_freesurfer_volume::ltaops::{InvertInto, lta_change_type, lta_invert};
use larmorx_freesurfer_volume::ltawrite::lta_read;
use larmorx_freesurfer_volume::rotmat::mri_angles2rot_mat;

use super::main::{Run, fixed_buffer, mul, overflow_abort};
use crate::mri_segstats::main::RegLta;
use crate::mri_segstats::options::{scan_d, scan_lf};

/// `print_usage()`'s text (`printf` lines, as FreeSurfer prints them).
pub const USAGE: &str = include_str!("usage.txt");

/// `print_help()`'s text after the usage and the version.
pub const HELP: &str = include_str!("help.txt");

/// A path not ported yet (relay 53): its message on standard error, status 1.
pub fn not_ported(r: &mut Run, what: &str) -> Fatal {
    let _ = writeln!(
        r.err,
        "larmorx-freesurfer: mri_vol2vol: {what} is not ported yet"
    );
    Fatal::Exit(1)
}

/// `mri_vol2vol`'s settings (its file-scope variables), FreeSurfer's defaults.
pub struct Options {
    pub debug: bool,
    pub gdiagno: i32,
    pub setfilter: bool,
    pub soap_bubble_iters: i32,
    pub mri_soap_ctrl: Option<Mri>,
    pub movvolfile: Option<String>,
    pub targvolfile: Option<String>,
    pub fstarg: bool,
    pub fstargfile: String,
    pub outvolfile: Option<String>,
    pub regfile: Option<String>,
    pub xfmfile: Option<String>,
    pub fslregfile: Option<String>,
    pub invert: bool,
    pub fstal: bool,
    pub lta: Option<Lta>,
    pub usedltageom: bool,
    pub fstalres: i32,
    pub precision: String,
    pub precisioncode: i32,
    pub interpmethod: String,
    pub interpcode: i32,
    pub sinchw: i32,
    pub regheader: bool,
    pub noresample: bool,
    pub r: Option<Matrix>,
    pub r2: Option<Matrix>,
    pub xfm: Option<Matrix>,
    pub rfsl: Option<Matrix>,
    pub rfsl2: Option<Matrix>,
    pub fsh: String,
    pub subjects_dir: Option<String>,
    pub talxfmfile: String,
    /// `subject` (a C string; `None`: `NULL`).
    pub subject: Option<Vec<u8>>,
    pub ipr: f32,
    pub bpr: f32,
    pub intensity: f32,
    pub float2int: i32,
    pub save_reg: bool,
    pub do_kernel: bool,
    pub do_save_input_mr: bool,
    pub do_delta: bool,
    pub do_morph: bool,
    pub invert_morph: bool,
    pub m3zfile: String,
    pub angles: [f64; 3],
    pub mrot: Option<Matrix>,
    pub xyztrans: [f64; 3],
    pub mtrans: Option<Matrix>,
    pub shear: [f64; 3],
    pub mshear: Option<Matrix>,
    pub seg_reg_cost_file: Option<String>,
    pub cost_only: bool,
    pub reg_file_final: Option<String>,
    pub synth_seed: i32,
    pub synth: bool,
    pub do_crop: bool,
    pub crop_scale: f64,
    pub disp_file: Option<String>,
    pub slice_crop_flag: bool,
    pub slice_crop_start: i32,
    pub slice_crop_stop: i32,
    pub slice_reverse: bool,
    pub slice_bias: bool,
    pub slice_bias_alpha: f64,
    pub useold: bool,
    pub vsmvolfile: Option<String>,
    pub vsmlta: Option<Lta>,
    pub def_m3z_path: bool,
    pub targ_mni152: bool,
    pub keepprecision: bool,
    pub do_fill: bool,
    pub do_fill_conserve: bool,
    pub fill_upsample: i32,
    pub do_multiply: bool,
    pub multiply_val: f64,
    pub down_sample: [i32; 3],
    pub pedir: i32,
    pub ctabfile: Option<String>,
    /// `targ` as `check_options` may allocate it (the LTA's destination geometry).
    pub targ: Option<Mri>,
}

impl Options {
    pub fn new(fsh: String) -> Options {
        Options {
            debug: false,
            gdiagno: -1,
            setfilter: false,
            soap_bubble_iters: 0,
            mri_soap_ctrl: None,
            movvolfile: None,
            targvolfile: None,
            fstarg: false,
            fstargfile: "orig.mgz".to_owned(),
            outvolfile: None,
            regfile: None,
            xfmfile: None,
            fslregfile: None,
            invert: false,
            fstal: false,
            lta: None,
            usedltageom: false,
            fstalres: 2,
            precision: "float".to_owned(),
            precisioncode: MRI_FLOAT,
            interpmethod: "trilinear".to_owned(),
            interpcode: 0,
            sinchw: 0,
            regheader: false,
            noresample: false,
            r: None,
            r2: None,
            xfm: None,
            rfsl: None,
            rfsl2: None,
            fsh,
            subjects_dir: None,
            talxfmfile: "talairach.xfm".to_owned(),
            subject: None,
            ipr: 0.0,
            bpr: 0.0,
            intensity: 0.0,
            float2int: 0,
            save_reg: false,
            do_kernel: false,
            do_save_input_mr: true,
            do_delta: false,
            do_morph: false,
            invert_morph: false,
            m3zfile: "talairach.m3z".to_owned(),
            angles: [0.0; 3],
            mrot: None,
            xyztrans: [0.0; 3],
            mtrans: None,
            shear: [0.0; 3],
            mshear: None,
            seg_reg_cost_file: None,
            cost_only: false,
            reg_file_final: None,
            synth_seed: -1,
            synth: false,
            do_crop: false,
            crop_scale: 0.0,
            disp_file: None,
            slice_crop_flag: false,
            slice_crop_start: 0,
            slice_crop_stop: 0,
            slice_reverse: false,
            slice_bias: false,
            slice_bias_alpha: 1.0,
            useold: true,
            vsmvolfile: None,
            vsmlta: None,
            def_m3z_path: true,
            targ_mni152: false,
            keepprecision: false,
            do_fill: false,
            do_fill_conserve: false,
            fill_upsample: 2,
            do_multiply: false,
            multiply_val: 0.0,
            down_sample: [0; 3],
            pedir: 2,
            ctabfile: None,
            targ: None,
        }
    }

    /// `subject` as `printf`'s `%s` shows it (`(null)` for `NULL`).
    pub fn subject_text(&self) -> Vec<u8> {
        self.subject
            .clone()
            .unwrap_or_else(|| b"(null)".to_vec())
    }
}

/// A C string for `%s`: `(null)` for `NULL`.
pub fn or_null(s: Option<&str>) -> &str {
    s.unwrap_or("(null)")
}

/// `print_usage()`.
pub fn print_usage(r: &mut Run) {
    let _ = r.out.write_all(USAGE.as_bytes());
}

/// `usage_exit()`: the usage, then `exit(1)`.
pub fn usage_exit(r: &mut Run) -> Fatal {
    print_usage(r);
    Fatal::Exit(1)
}

/// `print_help()`: the usage, the version, the help; `exit(1)`.
fn print_help(r: &mut Run) -> Fatal {
    print_usage(r);
    let _ = write!(r.out, "\n{}\n\n", get_version());
    let _ = r.out.write_all(HELP.as_bytes());
    Fatal::Exit(1)
}

/// `print_version()`: the version, `exit(1)`.
fn print_version(r: &mut Run) -> Fatal {
    let _ = writeln!(r.out, "{}", get_version());
    let _ = r.out.flush();
    Fatal::Exit(1)
}

/// `argnerr(option, n)`: `ERROR: <option> flag needs <n> argument(s)` on standard error,
/// `exit(-1)`.
fn argnerr(r: &mut Run, option: &str, n: i32) -> Fatal {
    cmd_arg_n_err(r.err, option, n)
}

/// `singledash(flag)`.
fn singledash(flag: &str) -> bool {
    let b = flag.as_bytes();
    b.len() >= 2 && b[0] == b'-' && b[1] != b'-'
}

/// `istringnmatch(str1, str2, n)`: the first `n` characters (all when `n` is 0) equal in any case.
pub fn istringnmatch(a: &str, b: &str, n: usize) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if n > 0 {
        // strncasecmp: up to n bytes, stopping at the first NUL of either
        let la = a.len().min(n);
        let lb = b.len().min(n);
        la == lb && a[..la].eq_ignore_ascii_case(&b[..lb])
    } else {
        a.eq_ignore_ascii_case(b)
    }
}

/// `sscanf(s, "%d", &v)` into `v` (left as it was when none is read).
fn scan_d_into(g: &mut Globals, s: &str, v: &mut i32) {
    if let Some(x) = scan_d(g, s) {
        *v = x;
    }
}

/// `sscanf(s, "%lf", &v)` into `v`.
fn scan_lf_into(g: &mut Globals, s: &str, v: &mut f64) {
    if let Some(x) = scan_lf(g, s) {
        *v = x;
    }
}

/// `MatrixPrint(stdout, m)` (`% 2.5f`).
pub fn print_matrix(g: &mut Globals, r: &mut Run, m: Option<&Matrix>) {
    let mut buf = Vec::new();
    larmorx_freesurfer_core::matrix_ops::matrix_print(
        g,
        Some(&mut buf),
        &mut std::io::sink(),
        r.err,
        m,
        5,
    );
    let _ = r.out.write_all(&buf);
}

/// `regio_read_register(regfile, &subject, &ipr, &bpr, &intensity, &R, &float2int)` into the
/// options (each set only where the reader sets it); its status.
fn read_register(g: &mut Globals, r: &mut Run, o: &mut Options, regfile: &str) -> Result<i32, Fatal> {
    let mut reg = Register {
        subject: o.subject.take(),
        inplaneres: o.ipr,
        betplaneres: o.bpr,
        intensity: o.intensity,
        r: o.r.take(),
        float2int: o.float2int,
    };
    let env = r.env.clone();
    let mut io = RegLta { env: &env };
    let st = regio_read_register(g, r.out, r.err, r.dir, regfile, &mut reg, &mut io)?;
    o.subject = reg.subject;
    o.ipr = reg.inplaneres;
    o.bpr = reg.betplaneres;
    o.intensity = reg.intensity;
    o.r = reg.r;
    o.float2int = reg.float2int;
    Ok(st)
}

/// `LTAread(name)`.
pub fn read_lta(g: &mut Globals, r: &mut Run, name: &str) -> Result<Option<Lta>, Fatal> {
    let env = r.env.clone();
    let mut io = GcaLtaIo { env: &env };
    lta_read(g, r.out, r.err, r.dir, name, &mut io)
}

/// `MRIread(name)`.
pub fn read_volume(g: &mut Globals, r: &mut Run, name: &str) -> Result<Option<Mri>, Fatal> {
    let env = r.env.clone();
    mri_read_volume(g, r.out, r.err, &env, r.dir, name)
}

/// `MRIreadHeader(name, MRI_VOLUME_TYPE_UNKNOWN)`.
pub fn read_header(g: &mut Globals, r: &mut Run, name: &str) -> Result<Option<Mri>, Fatal> {
    let env = r.env.clone();
    mri_read_header(g, r.out, r.err, &env, r.dir, name, MRI_VOLUME_TYPE_UNKNOWN)
}

/// `LoadRfsl(fname)`: sixteen `%f` numbers row by row into a 4 x 4 matrix. A file that does not
/// open: `ERROR: cannot open <name>` and `exit(1)`; a number that does not read: `ERROR: reading
/// <name>, row <i>, col <j>` (0-based) and `None`.
fn load_rfsl(g: &mut Globals, r: &mut Run, fname: &str) -> Result<Option<Matrix>, Fatal> {
    let mut fp = match CFile::open(r.dir, fname, "r") {
        Ok(f) => f,
        Err(e) => {
            g.errno = e;
            let _ = writeln!(r.out, "ERROR: cannot open {fname}");
            return Err(Fatal::Exit(1));
        }
    };
    let Some(mut m) = matrix_alloc(g, r.out, r.err, 4, 4, MATRIX_REAL)? else {
        return Ok(None);
    };
    for i in 0..4 {
        for j in 0..4 {
            let mut errno = g.errno;
            let (n, v) = {
                let mut s = Scanf::new(&mut fp);
                let v = s.f(&mut errno, true);
                (s.finish(&mut errno), v)
            };
            g.errno = errno;
            match v.filter(|_| n == 1) {
                Some(v) => m.set_relt(i + 1, j + 1, v),
                None => {
                    let _ = writeln!(r.out, "ERROR: reading {fname}, row {i}, col {j}");
                    return Ok(None);
                }
            }
        }
    }
    Ok(Some(m))
}

/// `LoadRtal(talres)`: the registration of `$FREESURFER_HOME/average/mni305.cor.subfov<res>.reg`
/// (`regio_read_register`, which also sets the program's `subject`); a file that does not read
/// ends the program (`exit(1)`).
pub fn load_rtal(g: &mut Globals, r: &mut Run, o: &mut Options, talres: i32) -> Result<Matrix, Fatal> {
    let rtalfile = format!("{}/average/mni305.cor.subfov{talres}.reg", o.fsh);
    let mut reg = Register {
        subject: o.subject.take(),
        ..Register::default()
    };
    let env = r.env.clone();
    let mut io = RegLta { env: &env };
    let st = regio_read_register(g, r.out, r.err, r.dir, &rtalfile, &mut reg, &mut io)?;
    o.subject = reg.subject;
    if st != 0 {
        return Err(Fatal::Exit(1));
    }
    match reg.r {
        Some(m) => Ok(m),
        None => Err(Fatal::Exit(1)),
    }
}

/// `parse_commandline(argc, argv)` on `args` (after the program's name).
pub fn parse_commandline(
    g: &mut Globals,
    r: &mut Run,
    o: &mut Options,
    args: &[String],
) -> Result<(), Fatal> {
    if args.is_empty() {
        return Err(usage_exit(r));
    }
    let mut at = 0usize;
    while at < args.len() {
        let option = args[at].as_str();
        if o.debug {
            let _ = writeln!(r.out, "{} {option}", args.len() - at);
        }
        at += 1;
        let rest = &args[at..];
        let nargc = rest.len();
        let mut nargsused = 0usize;
        let ci = |b: &str| option.eq_ignore_ascii_case(b);
        if ci("--help") {
            return Err(print_help(r));
        } else if ci("--version") {
            return Err(print_version(r));
        } else if ci("--debug") {
            o.debug = true;
        } else if ci("--tal") {
            o.fstal = true;
        } else if ci("--inv") {
            o.invert = true;
        } else if ci("--no-resample") {
            o.noresample = true;
        } else if ci("--no-resample-scale") {
            if nargc < 1 {
                return Err(argnerr(r, option, 1));
            }
            r.env
                .set("FS_SetVoxToRasXform_Change_VoxSize", rest[0].as_str());
            o.noresample = true;
            nargsused = 1;
        } else if ci("--regheader") {
            o.regheader = true;
        } else if ci("--kernel") {
            o.do_kernel = true;
        } else if ci("--nomr") {
            o.do_save_input_mr = true;
        } else if ci("--mr") {
            o.do_save_input_mr = false;
        } else if ci("--delta") {
            o.do_delta = true;
        } else if ci("--no-save-reg") {
            o.save_reg = false;
        } else if ci("--save-reg") {
            o.save_reg = true;
        } else if ci("--cost-only") {
            o.cost_only = true;
        } else if ci("--synth") {
            o.synth = true;
        } else if ci("--soap") {
            if nargc < 2 {
                return Err(argnerr(r, option, 2));
            }
            nargsused = 2;
            o.mri_soap_ctrl = read_volume(g, r, &rest[0])?;
            if o.mri_soap_ctrl.is_none() {
                error_exit(g, r.err, ERROR_NOFILE, "")?;
            }
            scan_d_into(g, &rest[1], &mut o.soap_bubble_iters);
            let _ = writeln!(
                r.out,
                "performing soap bubble smoothing using {} for {} iterations",
                rest[0], o.soap_bubble_iters
            );
        } else if ci("--new") {
            o.useold = false;
        } else if ci("--fill-average") {
            o.do_fill = true;
            o.do_fill_conserve = false;
        } else if ci("--fill-conserve") {
            o.do_fill = true;
            o.do_fill_conserve = true;
        } else if ci("--fill-upsample") {
            if nargc < 1 {
                return Err(argnerr(r, option, 1));
            }
            scan_d_into(g, &rest[0], &mut o.fill_upsample);
            nargsused = 1;
        } else if ci("--downsample") {
            if nargc < 3 {
                return Err(argnerr(r, option, 3));
            }
            for k in 0..3 {
                scan_d_into(g, &rest[k], &mut o.down_sample[k]);
            }
            o.do_fill = true;
            o.do_fill_conserve = false;
            o.fill_upsample = 2;
            o.regheader = true;
            nargsused = 3;
        } else if ci("--morph") {
            o.do_morph = true;
            o.fstarg = true;
        } else if ci("--fstarg") {
            o.fstarg = true;
            if cmd_nth_is_arg(rest, 0) {
                o.fstargfile = rest[0].clone();
                nargsused = 1;
            }
            let _ = writeln!(r.out, "fstargfile {}", o.fstargfile);
        } else if ci("--inv-morph") {
            o.do_morph = true;
            o.invert_morph = true;
            o.invert = true;
        } else if istringnmatch(option, "--m3z", 0) {
            if nargc < 1 {
                return Err(argnerr(r, option, 1));
            }
            o.m3zfile = rest[0].clone();
            o.do_morph = true;
            nargsused = 1;
        } else if istringnmatch(option, "--noDefM3zPath", 0) {
            o.def_m3z_path = false;
            if o.r.is_none() {
                o.r = Some(identity(g, r)?);
            }
            let _ = writeln!(
                r.out,
                "Using the m3z file as it is; no assumed location."
            );
        } else if istringnmatch(option, "--mov", 0) {
            if nargc < 1 {
                return Err(argnerr(r, option, 1));
            }
            o.movvolfile = Some(rest[0].clone());
            nargsused = 1;
        } else if istringnmatch(option, "--vsm", 0) {
            if nargc < 1 {
                return Err(argnerr(r, option, 1));
            }
            o.vsmvolfile = Some(rest[0].clone());
            o.useold = false;
            nargsused = 1;
            if cmd_nth_is_arg(rest, 1) {
                scan_d_into(g, &rest[1], &mut o.pedir);
                nargsused += 1;
            }
        } else if istringnmatch(option, "--vsm-reg", 0) {
            if nargc < 1 {
                return Err(argnerr(r, option, 1));
            }
            o.vsmlta = read_lta(g, r, &rest[0])?;
            if o.vsmlta.is_none() {
                return Err(Fatal::Exit(1));
            }
            nargsused = 1;
        } else if option == "--vsm-pedir" {
            if nargc < 1 {
                return Err(argnerr(r, option, 1));
            }
            scan_d_into(g, &rest[0], &mut o.pedir);
            nargsused = 1;
        } else if istringnmatch(option, "--targ", 0) {
            if nargc < 1 {
                return Err(argnerr(r, option, 1));
            }
            o.targvolfile = Some(rest[0].clone());
            nargsused = 1;
        } else if istringnmatch(option, "--reg", 0) {
            if nargc < 1 {
                return Err(argnerr(r, option, 1));
            }
            let regfile = rest[0].clone();
            o.regfile = Some(regfile.clone());
            let err = read_register(g, r, o, &regfile)?;
            let _ = writeln!(r.out);
            let _ = writeln!(r.out, "Matrix from regfile:");
            let m = o.r.clone();
            print_matrix(g, r, m.as_ref());
            let _ = writeln!(r.out);
            if err != 0 {
                return Err(Fatal::Exit(1));
            }
            o.lta = read_lta(g, r, &regfile)?;
            if o.lta.is_none() {
                let _ = writeln!(r.out, "ERROR reading LTA {regfile} !");
                return Err(Fatal::Exit(1));
            }
            nargsused = 1;
        } else if istringnmatch(option, "--lta", 0) || istringnmatch(option, "--lta-inv", 0) {
            if nargc < 1 {
                return Err(argnerr(r, option, 1));
            }
            let regfile = rest[0].clone();
            o.regfile = Some(regfile.clone());
            let ext = file_name_extension(g, &regfile);
            if stricmp(g, &ext, "LTA") != 0 && stricmp(g, &ext, "identity.nofile") != 0 {
                let _ = writeln!(r.out, "LTA registration file needs to have .lta extension!");
                return Err(Fatal::Exit(1));
            }
            let Some(mut lta) = read_lta(g, r, &regfile)? else {
                let _ = writeln!(r.out, "ERROR reading LTA {regfile} !");
                return Err(Fatal::Exit(1));
            };
            if lta.xforms[0].src.valid == 0 {
                let _ = writeln!(r.out, "ERROR LTA {regfile} has no valid src geometry!");
                return Err(Fatal::Exit(1));
            }
            if lta.xforms[0].dst.valid == 0 {
                let _ = writeln!(r.out, "ERROR LTA {regfile} has no valid dst geometry!");
                return Err(Fatal::Exit(1));
            }
            if lta.subject.is_empty() {
                lta.subject.strcpy_bytes(b"subject-unknown");
            }
            o.subject = Some(lta.subject.c_bytes().to_vec());
            if istringnmatch(option, "--lta-inv", 0) {
                let _ = writeln!(r.out, "Inverting LTA");
                lta_invert(g, r.out, r.err, InvertInto::Same(&mut lta))?;
            }
            o.intensity = lta.fscale;
            o.float2int = FLT2INT_ROUND;
            let env = r.env.clone();
            lta_change_type(g, r.out, r.err, &env, &mut lta, REGISTER_DAT)?;
            o.r = lta.xforms[0].m_l.clone();
            o.ipr = lta.xforms[0].src.xsize;
            o.bpr = lta.xforms[0].src.zsize;
            let _ = writeln!(r.out);
            let _ = writeln!(r.out, "Matrix from LTA:");
            let m = o.r.clone();
            print_matrix(g, r, m.as_ref());
            let _ = writeln!(r.out);
            o.lta = Some(lta);
            nargsused = 1;
        } else if istringnmatch(option, "--mni152reg", 0) {
            o.targ_mni152 = true;
            let fsldir = r.env.get("FSLDIR").map(|v| v.to_string_lossy().into_owned());
            let tmp = format!(
                "{}/data/standard/MNI152_T1_2mm.nii.gz",
                or_null(fsldir.as_deref())
            );
            if fixed_buffer(g, &tmp, 2000, "the target's name") {
                return Err(overflow_abort(r));
            }
            o.targvolfile = Some(tmp);
        } else if istringnmatch(option, "--reg-final", 0) {
            if nargc < 1 {
                return Err(argnerr(r, option, 1));
            }
            o.reg_file_final = Some(rest[0].clone());
            nargsused = 1;
        } else if istringnmatch(option, "--s", 0) {
            if nargc < 1 {
                return Err(argnerr(r, option, 1));
            }
            o.subject = Some(rest[0].clone().into_bytes());
            o.r = Some(identity(g, r)?);
            nargsused = 1;
        } else if option == "--copy-ctab" {
            r.env.set("FS_COPY_HEADER_CTAB", "1");
        } else if istringnmatch(option, "--fsl", 0) || istringnmatch(option, "--fslreg", 0) {
            if nargc < 1 {
                return Err(argnerr(r, option, 1));
            }
            o.fslregfile = Some(rest[0].clone());
            o.rfsl = load_rfsl(g, r, &rest[0])?;
            if o.rfsl.is_none() {
                return Err(Fatal::Exit(1));
            }
            nargsused = 1;
            if nargc > 1 && !cmd_is_flag(&rest[1]) {
                o.rfsl2 = load_rfsl(g, r, &rest[1])?;
                if o.rfsl2.is_none() {
                    return Err(Fatal::Exit(1));
                }
                nargsused += 1;
            }
        } else if istringnmatch(option, "--xfm", 0) {
            if nargc < 1 {
                return Err(argnerr(r, option, 1));
            }
            o.xfmfile = Some(rest[0].clone());
            let x = regio_read_mincxfm(g, r.out, r.err, r.dir, &rest[0], false)?;
            if x.r.is_some() {
                o.xfm = x.r;
            }
            if x.status != 0 {
                return Err(Fatal::Exit(1));
            }
            o.regheader = true;
            nargsused = 1;
        } else if istringnmatch(option, "--talxfm", 0) {
            if nargc < 1 {
                return Err(argnerr(r, option, 1));
            }
            o.talxfmfile = rest[0].clone();
            o.fstal = true;
            nargsused = 1;
        } else if istringnmatch(option, "--out", 0) || istringnmatch(option, "--o", 0) {
            if nargc < 1 {
                return Err(argnerr(r, option, 1));
            }
            o.outvolfile = Some(rest[0].clone());
            nargsused = 1;
        } else if istringnmatch(option, "--disp", 0) {
            if nargc < 1 {
                return Err(argnerr(r, option, 1));
            }
            o.disp_file = Some(rest[0].clone());
            nargsused = 1;
        } else if istringnmatch(option, "--talres", 8) {
            if nargc < 1 {
                return Err(argnerr(r, option, 1));
            }
            scan_d_into(g, &rest[0], &mut o.fstalres);
            if o.fstalres != 1 && o.fstalres != 2 {
                let _ = writeln!(
                    r.out,
                    "ERROR: tal res {} invalid. Only use 1 or 2",
                    o.fstalres
                );
                return Err(Fatal::Exit(1));
            }
            nargsused = 1;
        } else if istringnmatch(option, "--crop", 6) {
            if nargc < 1 {
                return Err(argnerr(r, option, 1));
            }
            scan_lf_into(g, &rest[0], &mut o.crop_scale);
            o.do_crop = true;
            nargsused = 1;
        } else if option == "--mul" {
            if nargc < 1 {
                return Err(argnerr(r, option, 1));
            }
            let c = rest[0].as_bytes().first().copied().unwrap_or(0);
            if !c.is_ascii_digit() && c != b'-' && c != b'+' && c != b'.' {
                let _ = writeln!(
                    r.out,
                    "ERROR: value passed to the --mul flag must be a number"
                );
                let _ = writeln!(
                    r.out,
                    "       If you want to multiply two images, use fscalc"
                );
                return Err(Fatal::Exit(1));
            }
            scan_lf_into(g, &rest[0], &mut o.multiply_val);
            o.do_multiply = true;
            nargsused = 1;
        } else if istringnmatch(option, "--slice-crop", 12) {
            if nargc < 2 {
                return Err(argnerr(r, option, 2));
            }
            o.slice_crop_flag = true;
            scan_d_into(g, &rest[0], &mut o.slice_crop_start);
            scan_d_into(g, &rest[1], &mut o.slice_crop_stop);
            nargsused = 2;
        } else if istringnmatch(option, "--slice-reverse", 0) {
            o.slice_reverse = true;
        } else if istringnmatch(option, "--slice-bias", 0) {
            if nargc < 1 {
                return Err(argnerr(r, option, 1));
            }
            scan_lf_into(g, &rest[0], &mut o.slice_bias_alpha);
            o.slice_bias = true;
            nargsused = 1;
        } else if istringnmatch(option, "--interp", 8) {
            if nargc < 1 {
                return Err(argnerr(r, option, 1));
            }
            o.interpmethod = rest[0].clone();
            nargsused = 1;
            if o.interpmethod == "sinc" && cmd_nth_is_arg(rest, 1) {
                scan_d_into(g, &rest[1], &mut o.sinchw);
                nargsused += 1;
            }
        } else if istringnmatch(option, "--trilin", 6) {
            o.interpmethod = "trilinear".to_owned();
        } else if istringnmatch(option, "--nearest", 7) {
            o.interpmethod = "nearest".to_owned();
        } else if istringnmatch(option, "--cubic", 0) {
            o.interpmethod = "cubic".to_owned();
        } else if istringnmatch(option, "--precision", 0) {
            if nargc < 1 {
                return Err(argnerr(r, option, 1));
            }
            o.precision = rest[0].clone();
            o.precisioncode = mri_precision_code(&o.precision);
            if o.precisioncode < 0 {
                let _ = writeln!(r.out, "ERROR: precision {} unrecognized", o.precision);
                let _ = writeln!(
                    r.out,
                    "       legal values are uchar, short, int, long, and float"
                );
                return Err(Fatal::Exit(1));
            }
            nargsused = 1;
        } else if istringnmatch(option, "--keep-precision", 0) {
            o.keepprecision = true;
        } else if ci("--seed") {
            if nargc < 1 {
                return Err(cmd_arg_n_err(r.err, option, 1));
            }
            scan_d_into(g, &rest[0], &mut o.synth_seed);
            o.synth = true;
            nargsused = 1;
        } else if istringnmatch(option, "--sd", 4) {
            if nargc < 1 {
                return Err(argnerr(r, option, 1));
            }
            r.env.set("SUBJECTS_DIR", rest[0].as_str());
            nargsused = 1;
        } else if istringnmatch(option, "--ctab", 4) {
            if nargc < 1 {
                return Err(argnerr(r, option, 1));
            }
            o.ctabfile = Some(rest[0].clone());
            nargsused = 1;
        } else if istringnmatch(option, "--rot", 0) {
            if nargc < 3 {
                return Err(argnerr(r, option, 3));
            }
            for k in 0..3 {
                scan_lf_into(g, &rest[k], &mut o.angles[k]);
            }
            for k in 0..3 {
                o.angles[k] *= std::f64::consts::PI / 180.0;
            }
            o.mrot = Some(mri_angles2rot_mat(g, r.out, r.err, &o.angles)?);
            nargsused = 3;
        } else if istringnmatch(option, "--trans", 0) {
            if nargc < 3 {
                return Err(argnerr(r, option, 3));
            }
            for k in 0..3 {
                scan_lf_into(g, &rest[k], &mut o.xyztrans[k]);
            }
            let mut m = identity(g, r)?;
            for k in 0..3 {
                m.set_relt(k as i32 + 1, 4, o.xyztrans[k] as f32);
            }
            o.mtrans = Some(m);
            nargsused = 3;
        } else if istringnmatch(option, "--shear", 0) {
            if nargc < 3 {
                return Err(argnerr(r, option, 3));
            }
            for k in 0..3 {
                scan_lf_into(g, &rest[k], &mut o.shear[k]);
            }
            let mut m = identity(g, r)?;
            m.set_relt(1, 2, o.shear[0] as f32);
            m.set_relt(1, 3, o.shear[1] as f32);
            m.set_relt(2, 3, o.shear[2] as f32);
            o.mshear = Some(m);
            nargsused = 3;
        } else if option == "--vg-thresh" {
            if nargc < 1 {
                return Err(argnerr(r, option, 1));
            }
            let mut v = g.vg_is_equal_threshold;
            scan_lf_into(g, &rest[0], &mut v);
            g.vg_is_equal_threshold = v;
            nargsused = 1;
        } else if option == "--filter" {
            o.setfilter = true;
        } else if option == "--gdiagno" {
            if nargc < 1 {
                return Err(argnerr(r, option, 1));
            }
            scan_d_into(g, &rest[0], &mut o.gdiagno);
            nargsused = 1;
        } else if istringnmatch(option, "--subject", 0) {
            if nargc < 1 {
                return Err(argnerr(r, option, 1));
            }
            o.subject = Some(rest[0].clone().into_bytes());
            nargsused = 1;
        } else if istringnmatch(option, "--cost", 0) {
            if nargc < 1 {
                return Err(argnerr(r, option, 1));
            }
            o.seg_reg_cost_file = Some(rest[0].clone());
            nargsused = 1;
        } else if istringnmatch(option, "--spm-warp", 0) {
            if nargc < 5 {
                let _ = writeln!(r.out, "  --spm-warp mov movlta warp interp out");
                return Err(argnerr(r, option, 6));
            }
            return Err(not_ported(
                r,
                "--spm-warp (MRIapplySpmWarp of an SPM deformation)",
            ));
        } else if istringnmatch(option, "--gcam", 0) || istringnmatch(option, "--gcam0", 0) {
            if nargc < 7 {
                let _ = writeln!(r.out, "  --gcam mov srclta gcam dstlta vsm interp out");
                return Err(argnerr(r, option, 7));
            }
            return Err(not_ported(r, "--gcam (MRIvol2volGCAM)"));
        } else if istringnmatch(option, "--map-point", 0)
            || istringnmatch(option, "--map-point-inv-lta", 0)
        {
            if nargc < 7 {
                let _ = writeln!(
                    r.out,
                    "  --map-point a b c incoords lta outcoords outfile"
                );
                let _ = writeln!(r.out, "  coords: 1=tkras, 2=scannerras, 3=vox");
                return Err(argnerr(r, option, 7));
            }
            return Err(not_ported(r, "--map-point (LTA::get_matrix)"));
        } else {
            let _ = writeln!(r.err, "ERROR: Option {option} unknown");
            if singledash(option) {
                let _ = writeln!(r.err, "       Did you really mean -{option} ?");
            }
            return Err(Fatal::Exit(-1));
        }
        at += nargsused;
    }
    Ok(())
}

/// `MatrixIdentity(4, NULL)`.
pub fn identity(g: &mut Globals, r: &mut Run) -> Result<Matrix, Fatal> {
    Ok(matrix_identity(g, r.out, r.err, 4, None)?.unwrap_new())
}

/// `mkdir(name, 0777)`'s `errno` (0 when it made the folder), Linux's value on every platform: a
/// name that exists gives `EEXIST` (macOS answers `ENOTDIR` for a file, Windows its own codes).
pub fn mkdir_errno(dir: Option<&Path>, name: &str) -> i32 {
    const EEXIST: i32 = 17;
    let target = larmorx_freesurfer_core::utils::resolve(dir, name);
    match std::fs::create_dir(&target) {
        Ok(()) => 0,
        Err(e) => {
            let trimmed = larmorx_freesurfer_core::pathsep::trim_end_seps_str(
                &target.as_os_str().to_string_lossy(),
            )
            .to_owned();
            let exists = !name.is_empty() && std::fs::symlink_metadata(&trimmed).is_ok();
            if exists {
                EEXIST
            } else {
                larmorx_freesurfer_core::log::errno_of(&e).unwrap_or(5)
            }
        }
    }
}

/// `check_options()`.
pub fn check_options(g: &mut Globals, r: &mut Run, o: &mut Options) -> Result<(), Fatal> {
    o.subjects_dir = r
        .env
        .get("SUBJECTS_DIR")
        .map(|v| v.to_string_lossy().into_owned());
    if o.subjects_dir.is_none() {
        let _ = writeln!(r.out, "ERROR: SUBJECTS_DIR undefined.");
        return Err(Fatal::Exit(1));
    }
    let sd = o.subjects_dir.clone().unwrap_or_default();
    if o.movvolfile.is_none() && (o.lta.is_none() || !o.invert) {
        let _ = writeln!(r.out, "ERROR: No mov volume supplied.");
        return Err(Fatal::Exit(1));
    }
    if o.fstarg && o.targvolfile.is_some() {
        let _ = writeln!(r.out, "ERROR: Do not specify a targ volume with --fstarg.");
        return Err(Fatal::Exit(1));
    }
    if o.fstal && o.targvolfile.is_some() {
        let _ = writeln!(r.out, "ERROR: Do not specify a targ volume with --tal.");
        return Err(Fatal::Exit(1));
    }
    if o.fstal && o.fstarg {
        let _ = writeln!(r.out, "ERROR: cannot specify a --tal and --fstarg.");
        return Err(Fatal::Exit(1));
    }
    if o.fstarg {
        let subj = String::from_utf8_lossy(&o.subject_text()).into_owned();
        let mut tmp = format!("{sd}/{subj}/mri/{}", o.fstargfile);
        if fixed_buffer(g, &tmp, 2000, "the target's name") {
            return Err(overflow_abort(r));
        }
        if fio_file_exists_readable(g, r.dir, &tmp) == 0 {
            tmp = format!("{sd}/{subj}/mri/orig");
            if fixed_buffer(g, &tmp, 2000, "the target's name") {
                return Err(overflow_abort(r));
            }
        }
        let _ = writeln!(r.out, "Using {tmp} as targ volume");
        o.targvolfile = Some(tmp);
    }
    if o.fstal {
        let tmp = if o.fstalres > 0 {
            format!("{}/average/mni305.cor.subfov{}.mgz", o.fsh, o.fstalres)
        } else {
            format!("{}/average/mni305.cor.mgz", o.fsh)
        };
        if fixed_buffer(g, &tmp, 2000, "the target's name") {
            return Err(overflow_abort(r));
        }
        o.targvolfile = Some(tmp);
    }
    if o.targvolfile.is_some() && o.down_sample[0] != 0 {
        let _ = writeln!(r.out, "ERROR: cannot spec target file and downsample.");
        return Err(Fatal::Exit(1));
    }
    if o.targvolfile.is_none() {
        if o.lta.is_some() && !o.invert {
            let _ = writeln!(r.out, "Getting target volume geom from lta destination");
            let vg = o.lta.as_ref().map(|l| l.xforms[0].dst.clone()).unwrap_or_default();
            o.targ = Some(mri_alloc_from_vol_geom(g, r.out, r.err, &vg, MRI_UCHAR, 1, true)?);
        } else if o.down_sample[0] == 0 {
            // FS-1177: --crop makes its own target
            if o.do_crop {
                g.notices.add(
                    &FS_1177,
                    "mri_vol2vol --crop without a target: cropped (FreeSurfer asks for a target --crop never uses)",
                );
            }
            if !o.do_crop || g.notices.reproduce() {
                let _ = writeln!(r.out, "ERROR: No target volume supplied.");
                return Err(Fatal::Exit(1));
            }
        }
    }
    if o.outvolfile.is_none() && o.disp_file.is_none() {
        let _ = writeln!(r.out, "ERROR: No output volume supplied.");
        return Err(Fatal::Exit(1));
    }
    if let Some(outvol) = o.outvolfile.clone() {
        let outdir = fio_dirname(g, Some(&outvol)).unwrap_or_default();
        let e = mkdir_errno(r.dir, &outdir);
        if e != 0 {
            g.errno = e;
            if e != 17 {
                let _ = writeln!(r.out, "ERROR: creating directory {outdir}");
                return Err(Fatal::Exit(1));
            }
        }
    }
    if o.fstal && o.regheader {
        let _ = writeln!(r.out, "ERROR: cannot use --tal and --regheader");
        return Err(Fatal::Exit(1));
    }
    if o.keepprecision {
        let mov = match o.movvolfile.clone() {
            Some(m) => read_header(g, r, &m)?,
            None => return Err(no_mov_name(g, r)),
        };
        let Some(mov) = mov else {
            return Err(Fatal::Exit(1));
        };
        o.precisioncode = mov.mtype;
        o.precision = mri_precision_string(o.precisioncode)
            .unwrap_or("(null)")
            .to_owned();
    }
    if o.targ_mni152 {
        if o.regfile.is_none() {
            let rf = format!(
                "{}/average/mni152.register.dat",
                r.env
                    .get("FREESURFER_HOME")
                    .map(|v| v.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "(null)".to_owned())
            );
            if fixed_buffer(g, &rf, 2000, "the registration's name") {
                return Err(overflow_abort(r));
            }
            o.regfile = Some(rf.clone());
            if read_register(g, r, o, &rf)? != 0 {
                return Err(Fatal::Exit(1));
            }
        } else {
            let subj = String::from_utf8_lossy(&o.subject_text()).into_owned();
            let tmp = format!("{sd}/{subj}/mri/transforms/reg.mni152.2mm.dat");
            if fixed_buffer(g, &tmp, 2000, "the registration's name") {
                return Err(overflow_abort(r));
            }
            let env = r.env.clone();
            let mut io = RegLta { env: &env };
            let q = regio_read_registermat(g, r.out, r.err, r.dir, &tmp, &mut io)?;
            let Some(q) = q else {
                let _ = writeln!(r.out, "Run mni152reg --s {subj} to create reg.mni152.2mm.dat");
                return Err(Fatal::Exit(1));
            };
            let invq = matrix_inverse(g, r.out, r.err, Some(&q), None)?.into_new();
            let rr = o.r.take();
            let m = mul(g, r, rr.as_ref(), invq.as_ref())?;
            o.r = m;
            let _ = writeln!(r.out, "New MNI152 R");
            let m = o.r.clone();
            print_matrix(g, r, m.as_ref());
        }
    }
    // FS-1183: the transform array read from a register.dat has no geometry, which the change of
    // type below refuses
    let from_dat = o.lta.as_ref().is_some_and(|l| {
        l.xforms[0].src.valid == 0 || l.xforms[0].dst.valid == 0
    }) && o.regfile.as_deref().is_some_and(|f| {
        let ext = file_name_extension(g, f);
        stricmp(g, &ext, "LTA") != 0
    });
    if from_dat && !o.fstal && o.targvolfile.is_some() {
        g.notices.add(
            &FS_1183,
            "mri_vol2vol --reg with a register.dat and a target: its matrix applied (FreeSurfer stops: LTAchangeType: src geometry must be valid)",
        );
    }
    if o.lta.is_some()
        && !o.fstal
        && o.targvolfile.is_some()
        && !(from_dat && !g.notices.reproduce())
    {
        let targvol = o.targvolfile.clone().unwrap_or_default();
        let _ = writeln!(r.out, "{} {targvol}", or_null(o.movvolfile.as_deref()));
        let Some(movname) = o.movvolfile.clone() else {
            return Err(no_mov_name(g, r));
        };
        let Some(mrimovtmp) = read_header(g, r, &movname)? else {
            return Err(Fatal::Exit(1));
        };
        let Some(mritrgtmp) = read_header(g, r, &targvol)? else {
            return Err(Fatal::Exit(1));
        };
        let env = r.env.clone();
        let mut lta = o.lta.take().unwrap_or_else(|| unreachable!("checked"));
        lta_change_type(g, r.out, r.err, &env, &mut lta, LINEAR_RAS_TO_RAS)?;
        lta_modify_src_dst_geom(g, r.out, r.err, &mut lta, Some(&mrimovtmp), Some(&mritrgtmp));
        o.r = transform_lta2_reg_dat(g, r.out, r.err, &mut lta)?;
        o.ipr = lta.xforms[0].src.xsize;
        o.bpr = lta.xforms[0].src.zsize;
        o.lta = Some(lta);
    }
    if !o.fstal
        && !o.do_crop
        && !o.fstarg
        && o.targvolfile.is_none()
        && (o.lta.is_none() || o.invert)
        && o.down_sample[0] == 0
    {
        let _ = writeln!(r.out, "ERROR: No targ volume supplied.");
        return Err(Fatal::Exit(1));
    }
    if o.do_crop && o.targvolfile.is_some() {
        let _ = writeln!(r.out, "ERROR: Do not specify a targ volume with --crop.");
        return Err(Fatal::Exit(1));
    }
    if o.xfmfile.is_some() && o.regfile.is_some() {
        let _ = writeln!(r.out, "ERROR: cannot specify both --xfm and --reg.");
        return Err(Fatal::Exit(1));
    }
    if o.regheader && o.regfile.is_some() {
        let _ = writeln!(r.out, "ERROR: cannot specify both --regheader and --reg.");
        return Err(Fatal::Exit(1));
    }
    if o.fstarg && o.regfile.is_none() && o.subject.is_none() {
        let _ = writeln!(r.out, "ERROR: Need --reg with --fstarg.");
        return Err(Fatal::Exit(1));
    }
    o.interpcode = mri_interp_code(&o.interpmethod);
    if o.interpcode < 0 {
        let _ = writeln!(
            r.out,
            "ERROR: interpolation method {} unrecognized",
            o.interpmethod
        );
        let _ = writeln!(r.out, "       legal values are nearest, trilin, and sinc");
        return Err(Fatal::Exit(1));
    }
    Ok(())
}

/// FS-1180: FreeSurfer reads the moving volume's header from a name it was never given (`--inv`
/// with an LTA and no `--mov`): it crashes (status 139); by default a clear error (rule B).
fn no_mov_name(g: &mut Globals, r: &mut Run) -> Fatal {
    g.notices.add(
        &FS_1180,
        "mri_vol2vol: the moving volume's header read without --mov (FreeSurfer reads a NULL name and crashes)",
    );
    if g.notices.reproduce() {
        return Fatal::Exit(139);
    }
    let _ = writeln!(
        r.out,
        "ERROR: --keep-precision and an LTA with --targ need the --mov volume (FreeSurfer crashes here)"
    );
    Fatal::Exit(1)
}

/// `dump_options(stdout)`.
pub fn dump_options(g: &mut Globals, r: &mut Run, o: &Options) {
    let w = &mut *r.out;
    let _ = writeln!(w, "movvol {}", or_null(o.movvolfile.as_deref()));
    if let Some(t) = &o.targvolfile {
        let _ = writeln!(w, "targvol {t}");
    }
    let _ = writeln!(w, "outvol {}", or_null(o.outvolfile.as_deref()));
    if let Some(f) = &o.regfile {
        let _ = writeln!(w, "regfile {f}");
    }
    if let Some(f) = &o.xfmfile {
        let _ = writeln!(w, "xfmfile {f}");
    }
    let _ = writeln!(w, "invert {}", i32::from(o.invert));
    let _ = writeln!(w, "tal    {}", i32::from(o.fstal));
    let _ = writeln!(w, "talres {}", o.fstalres);
    let _ = writeln!(w, "regheader {}", i32::from(o.regheader));
    let _ = writeln!(w, "noresample {}", i32::from(o.noresample));
    let _ = writeln!(w, "interp  {} ({})", o.interpmethod, o.interpcode);
    if o.interpcode == larmorx_freesurfer_mri::mri::SAMPLE_SINC {
        let _ = writeln!(w, "sinc hw  {}", o.sinchw);
    }
    let _ = writeln!(w, "precision  {} ({})", o.precision, o.precisioncode);
    let _ = writeln!(w, "Gdiag_no  {}", g.gdiag_no);
    if o.do_morph {
        let _ = writeln!(w, "Morphing");
        let _ = writeln!(w, "InvertMorph {}", i32::from(o.invert_morph));
    }
    let _ = writeln!(w, "Synth      {}", i32::from(o.synth));
    let _ = writeln!(w, "SynthSeed  {}", o.synth_seed);
}
