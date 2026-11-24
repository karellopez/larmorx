// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_label2vol`'s `parse_commandline` (`mri_label2vol.cpp` 553-789), `check_options`
//! (1118-1201), `dump_options` (1203-1228), `usage_exit`, `print_usage`, `print_help`,
//! `print_version`, `argnerr`, `singledash`, `checkhemi` and `get_proj_type_id` (791-1254).
//!
//! The options are read one by one; `--help` prints the usage and the help (status 1),
//! `--version` the version (status 1); an option missing its arguments `ERROR: <option> flag needs
//! <n> argument(s)` on standard error, status 255; an unknown one `ERROR: Option <option> unknown`
//! (and `       Did you really mean -<option> ?` for one dash) on standard error, status 255. The
//! flags `--help`, `--version`, `--debug`, `--native-vox2ras`, `--invertmtx`, `--new-aseg2vol`,
//! `--no-new-aseg2vol`, `--aparc+aseg`, `--ttype`, `--ttype+head` and `--fill-ribbon` match in any
//! case (`strcasecmp`), the rest exactly. Numbers are read with `sscanf` (a value that does not
//! read leaves the setting as it was). Read at once: `--ttype`'s and `--ttype+head`'s tissue-type
//! tables (`TissueTypeSchema`), `--tkr-template`'s header (status 1 when it does not read),
//! `--lta`'s transform (status 1; its subject becomes the subject), `--hemi`'s check (`ERROR: hemi
//! = <h>, must be lh or rh`, status 1), `--proj`'s type and checks (`ERROR: projtype = <t>, must be
//! abs or frac`, `ERROR: projection start must be <= stop`, `ERROR: cannot spec projection delta =
//! 0.`, `ERROR: projection delta must be > 0`, status 1). `--pvf <vol>` prints `PVF <vol>`.
//! `--regheader` takes the next argument unless it is an option; then `--seg`'s segmentation if
//! one was given before, else `ERROR: --regheader requires one argument unless you` / `are going
//! to use a --seg. If so specify that before --regheader`, status 1. `--defects` does its work at
//! once ([`super::main::defects`]) and ends the program.
//!
//! Defects (`larmorx_core::defects`): FS-1251 (`--defects`' seventh word taken as the label even
//! when it is an option), FS-1259 (more than 100 `--label`s).

use larmorx_core::defects::FS_1259;
use larmorx_freesurfer_core::cmdargs::{cmd_arg_n_err, cmd_nth_is_arg};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::version::get_version;
use larmorx_freesurfer_io::colortab::tissue_type_schema;
use larmorx_freesurfer_io::identify::MRI_VOLUME_TYPE_UNKNOWN;
use larmorx_freesurfer_io::mriio::mri_read_header;
use larmorx_freesurfer_mri::ctab::ColorTable;
use larmorx_freesurfer_mri::lta::Lta;
use larmorx_freesurfer_mri::mri::Mri;

use super::main::Run;
use crate::mri_segstats::options::{scan_d, scan_lf};
use crate::mri_vol2vol::options::read_lta;

/// `PROJ_TYPE_NONE`, `PROJ_TYPE_ABS`, `PROJ_TYPE_FRAC`.
pub const PROJ_TYPE_NONE: i32 = 0;
pub const PROJ_TYPE_ABS: i32 = 1;
pub const PROJ_TYPE_FRAC: i32 = 2;

/// `char *LabelList[100]`: the labels FreeSurfer's table holds.
pub const LABEL_LIST_MAX: usize = 100;

/// `print_usage()`'s text up to the version line.
pub const USAGE: &str = include_str!("usage.txt");
/// `print_help()`'s text after the usage.
pub const HELP: &str = include_str!("help.txt");

/// The settings the options give (`mri_label2vol.cpp`'s globals).
pub struct Options {
    pub label_list: Vec<String>,
    pub temp_vol_id: Option<String>,
    pub reg_mat_file: Option<String>,
    pub reg_header: bool,
    pub reg_identity: bool,
    pub invert_mtx: bool,
    pub fill_thresh: f64,
    pub proj_delta: f64,
    pub proj_start: f64,
    pub proj_stop: f64,
    pub proj_type: Option<String>,
    pub out_vol_id: Option<String>,
    pub hit_vol_id: Option<String>,
    pub pvf_vol_id: Option<String>,
    /// `subject` (`--subject`, or `--lta`'s subject field).
    pub subject: Option<Vec<u8>>,
    pub hemi: Option<String>,
    pub annot_file: Option<String>,
    pub aseg_fspec: Option<String>,
    pub surf_id: String,
    pub label_vol_file: Option<String>,
    pub label_stat_vol_fspec: Option<String>,
    pub do_label_stat_vol: bool,
    pub debug: bool,
    pub proj_type_id: i32,
    pub do_proj: bool,
    pub surf_needed: bool,
    /// `SUBJECTS_DIR` (`--sd`, else the environment's, set by [`check_options`]).
    pub subjects_dir: Option<String>,
    pub label_vox_vol: f64,
    pub use_native_vox2ras: bool,
    pub use_new_aseg2vol: bool,
    pub use_aparc_plus_aseg: bool,
    pub do_stat_thresh: bool,
    pub stat_thresh: f64,
    pub label_code_offset: i32,
    pub ct_tissue_type: Option<ColorTable>,
    pub upsample_factor: i32,
    pub resmm: f64,
    pub fill_ribbon: bool,
    pub lta: Option<Lta>,
    pub to_tkr_template: Option<Mri>,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            label_list: Vec::new(),
            temp_vol_id: None,
            reg_mat_file: None,
            reg_header: false,
            reg_identity: false,
            invert_mtx: false,
            fill_thresh: 0.0,
            proj_delta: 0.1,
            proj_start: 0.0,
            proj_stop: 0.0,
            proj_type: None,
            out_vol_id: None,
            hit_vol_id: None,
            pvf_vol_id: None,
            subject: None,
            hemi: None,
            annot_file: None,
            aseg_fspec: None,
            surf_id: "white".to_owned(),
            label_vol_file: None,
            label_stat_vol_fspec: None,
            do_label_stat_vol: false,
            debug: false,
            proj_type_id: PROJ_TYPE_NONE,
            do_proj: false,
            surf_needed: false,
            subjects_dir: None,
            label_vox_vol: 1.0,
            use_native_vox2ras: false,
            use_new_aseg2vol: false,
            use_aparc_plus_aseg: false,
            do_stat_thresh: false,
            stat_thresh: -1.0,
            label_code_offset: 0,
            ct_tissue_type: None,
            upsample_factor: -1,
            resmm: 0.0,
            fill_ribbon: false,
            lta: None,
            to_tkr_template: None,
        }
    }
}

impl Options {
    /// `nlabels` (the labels given; `main` sets it again for an annotation).
    pub fn nlabels(&self) -> usize {
        self.label_list.len()
    }
}

/// `print_usage()`.
pub fn print_usage(r: &mut Run) {
    let _ = write!(r.out, "{USAGE}{}\n\n", get_version());
}

/// `usage_exit()`: the usage, status 1.
pub fn usage_exit(r: &mut Run) -> Fatal {
    print_usage(r);
    Fatal::Exit(1)
}

/// `print_help()`: the usage and the help, status 1.
fn print_help(r: &mut Run) -> Fatal {
    print_usage(r);
    let _ = r.out.write_all(HELP.as_bytes());
    Fatal::Exit(1)
}

/// `print_version()`: the version, status 1.
fn print_version(r: &mut Run) -> Fatal {
    let _ = writeln!(r.out, "{}", get_version());
    let _ = r.out.flush();
    Fatal::Exit(1)
}

/// `argnerr(option, n)`: `ERROR: <option> flag needs <n> argument(s)` on standard error, status
/// 255.
pub fn argnerr(r: &mut Run, option: &str, n: i32) -> Fatal {
    cmd_arg_n_err(r.err, option, n)
}

/// `singledash(flag)`.
fn singledash(flag: &str) -> bool {
    let b = flag.as_bytes();
    b.len() >= 2 && b[0] == b'-' && b[1] != b'-'
}

/// `checkhemi(hemi)`: `lh` or `rh`, else `ERROR: hemi = <h>, must be lh or rh` and status 1.
fn checkhemi(r: &mut Run, hemi: &str) -> Result<(), Fatal> {
    if hemi == "lh" || hemi == "rh" {
        return Ok(());
    }
    let _ = writeln!(r.out, "ERROR: hemi = {hemi}, must be lh or rh");
    Err(Fatal::Exit(1))
}

/// `get_proj_type_id(projtype)`: `none` 0, `abs` 1, `frac` 2; else `ERROR: projtype = <t>, must be
/// abs or frac` and status 1.
fn get_proj_type_id(r: &mut Run, projtype: &str) -> Result<i32, Fatal> {
    match projtype {
        "none" => Ok(PROJ_TYPE_NONE),
        "abs" => Ok(PROJ_TYPE_ABS),
        "frac" => Ok(PROJ_TYPE_FRAC),
        _ => {
            let _ = writeln!(r.out, "ERROR: projtype = {projtype}, must be abs or frac");
            Err(Fatal::Exit(1))
        }
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

/// `parse_commandline(argc, argv)` on `args` (after the program's name).
pub fn parse_commandline(g: &mut Globals, r: &mut Run, args: &[String]) -> Result<Options, Fatal> {
    let mut o = Options::default();
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
        } else if ci("--native-vox2ras") {
            o.use_native_vox2ras = true;
        } else if ci("--invertmtx") {
            o.invert_mtx = true;
        } else if ci("--new-aseg2vol") {
            o.use_new_aseg2vol = true;
        } else if ci("--no-new-aseg2vol") {
            o.use_new_aseg2vol = false;
        } else if ci("--aparc+aseg") {
            o.use_aparc_plus_aseg = true;
        } else if ci("--ttype") || ci("--ttype+head") {
            let schema = if ci("--ttype") {
                "default-jan-2014"
            } else {
                "default-jan-2014+head"
            };
            let env = r.env.clone();
            o.ct_tissue_type = tissue_type_schema(g, &env, r.out, r.err, r.dir, None, schema)?;
        } else if ci("--fill-ribbon") {
            o.fill_ribbon = true;
        } else if option == "--surf" {
            if nargc < 1 {
                return Err(argnerr(r, option, 1));
            }
            o.surf_id = rest[0].clone();
            nargsused = 1;
        } else if option == "--label" {
            if nargc < 1 {
                return Err(argnerr(r, option, 1));
            }
            if o.label_list.len() == LABEL_LIST_MAX {
                // the table's 101st place is past its end (the next globals)
                g.notices.add(
                    &FS_1259,
                    format!(
                        "mri_label2vol: more than {LABEL_LIST_MAX} --label options: FreeSurfer's table holds {LABEL_LIST_MAX} (the next ones overwrite other settings)"
                    ),
                );
                if g.notices.reproduce() {
                    return Err(Fatal::Exit(139));
                }
            }
            o.label_list.push(rest[0].clone());
            nargsused = 1;
        } else if option == "--label-stat" {
            if nargc < 1 {
                return Err(argnerr(r, option, 1));
            }
            o.do_label_stat_vol = true;
            o.label_stat_vol_fspec = Some(rest[0].clone());
            nargsused = 1;
        } else if option == "--annot" {
            if nargc < 1 {
                return Err(argnerr(r, option, 1));
            }
            o.annot_file = Some(rest[0].clone());
            o.surf_needed = true;
            nargsused = 1;
        } else if option == "--seg" {
            if nargc < 1 {
                return Err(argnerr(r, option, 1));
            }
            o.aseg_fspec = Some(rest[0].clone());
            nargsused = 1;
        } else if option == "--upsample" {
            if nargc < 1 {
                return Err(argnerr(r, option, 1));
            }
            scan_d_into(g, &rest[0], &mut o.upsample_factor);
            nargsused = 1;
        } else if option == "--resmm" {
            if nargc < 1 {
                return Err(argnerr(r, option, 1));
            }
            scan_lf_into(g, &rest[0], &mut o.resmm);
            nargsused = 1;
        } else if option == "--temp" {
            if nargc < 1 {
                return Err(argnerr(r, option, 1));
            }
            o.temp_vol_id = Some(rest[0].clone());
            nargsused = 1;
        } else if option == "--tkr-template" {
            if nargc < 1 {
                return Err(argnerr(r, option, 1));
            }
            let env = r.env.clone();
            let h = mri_read_header(
                g,
                r.out,
                r.err,
                &env,
                r.dir,
                &rest[0],
                MRI_VOLUME_TYPE_UNKNOWN,
            )?;
            let Some(h) = h else {
                return Err(Fatal::Exit(1));
            };
            o.to_tkr_template = Some(h);
            nargsused = 1;
        } else if option == "--reg" {
            if nargc < 1 {
                return Err(argnerr(r, option, 1));
            }
            o.reg_mat_file = Some(rest[0].clone());
            nargsused = 1;
        } else if option == "--lta" {
            if nargc < 1 {
                return Err(argnerr(r, option, 1));
            }
            let Some(lta) = read_lta_run(g, r, &rest[0])? else {
                return Err(Fatal::Exit(1));
            };
            o.subject = Some(lta.subject.c_bytes().to_vec());
            o.lta = Some(lta);
            nargsused = 1;
        } else if option == "--fillthresh" {
            if nargc < 1 {
                return Err(argnerr(r, option, 1));
            }
            scan_lf_into(g, &rest[0], &mut o.fill_thresh);
            nargsused = 1;
        } else if option == "--labvoxvol" {
            if nargc < 1 {
                return Err(argnerr(r, option, 1));
            }
            scan_lf_into(g, &rest[0], &mut o.label_vox_vol);
            nargsused = 1;
        } else if option == "--proj" {
            if nargc < 4 {
                return Err(argnerr(r, option, 4));
            }
            o.proj_type = Some(rest[0].clone());
            scan_lf_into(g, &rest[1], &mut o.proj_start);
            scan_lf_into(g, &rest[2], &mut o.proj_stop);
            scan_lf_into(g, &rest[3], &mut o.proj_delta);
            o.proj_type_id = get_proj_type_id(r, &rest[0])?;
            if o.proj_start > o.proj_stop {
                let _ = writeln!(r.out, "ERROR: projection start must be <= stop");
                return Err(Fatal::Exit(1));
            }
            if o.proj_delta == 0.0 && o.proj_start != o.proj_stop {
                let _ = writeln!(r.out, "ERROR: cannot spec projection delta = 0.");
                return Err(Fatal::Exit(1));
            }
            if o.proj_delta < 0.0 {
                let _ = writeln!(r.out, "ERROR: projection delta must be > 0");
                return Err(Fatal::Exit(1));
            }
            o.do_proj = true;
            nargsused = 4;
        } else if option == "--subject" {
            if nargc < 1 {
                return Err(argnerr(r, option, 1));
            }
            o.subject = Some(rest[0].clone().into_bytes());
            nargsused = 1;
        } else if option == "--stat-thresh" {
            if nargc < 1 {
                return Err(argnerr(r, option, 1));
            }
            scan_lf_into(g, &rest[0], &mut o.stat_thresh);
            o.do_stat_thresh = true;
            nargsused = 1;
        } else if option == "--identity" {
            o.reg_identity = true;
        } else if option == "--regheader" {
            if cmd_nth_is_arg(rest, 0) {
                o.label_vol_file = Some(rest[0].clone());
                nargsused = 1;
            } else if let Some(a) = o.aseg_fspec.clone() {
                o.label_vol_file = Some(a);
            } else {
                let _ = writeln!(r.out, "ERROR: --regheader requires one argument unless you");
                let _ = writeln!(
                    r.out,
                    "are going to use a --seg. If so specify that before --regheader"
                );
                return Err(Fatal::Exit(1));
            }
            o.reg_header = true;
        } else if option == "--hemi" {
            if nargc < 1 {
                return Err(argnerr(r, option, 1));
            }
            checkhemi(r, &rest[0])?;
            o.hemi = Some(rest[0].clone());
            nargsused = 1;
        } else if option == "--hits" {
            if nargc < 1 {
                return Err(argnerr(r, option, 1));
            }
            o.hit_vol_id = Some(rest[0].clone());
            nargsused = 1;
        } else if option == "--pvf" {
            if nargc < 1 {
                return Err(argnerr(r, option, 1));
            }
            o.pvf_vol_id = Some(rest[0].clone());
            let _ = writeln!(r.out, "PVF {}", rest[0]);
            nargsused = 1;
        } else if option == "--sd" {
            if nargc < 1 {
                return Err(argnerr(r, option, 1));
            }
            o.subjects_dir = Some(rest[0].clone());
            nargsused = 1;
        } else if option == "--o" {
            if nargc < 1 {
                return Err(argnerr(r, option, 1));
            }
            o.out_vol_id = Some(rest[0].clone());
            nargsused = 1;
        } else if option == "--offset" {
            if nargc < 1 {
                return Err(argnerr(r, option, 1));
            }
            scan_d_into(g, &rest[0], &mut o.label_code_offset);
            nargsused = 1;
        } else if option == "--defects" {
            if nargc < 6 {
                let _ = writeln!(
                    r.out,
                    "usage: --defects surf defectno.mgz voltemplate.mgz offset mergeflag <label>"
                );
                return Err(argnerr(r, option, 6));
            }
            return Err(super::main::defects(g, r, rest)?);
        } else {
            let _ = writeln!(r.err, "ERROR: Option {option} unknown");
            if singledash(option) {
                let _ = writeln!(r.err, "       Did you really mean -{option} ?");
            }
            return Err(Fatal::Exit(-1));
        }
        at += nargsused;
    }
    Ok(o)
}

/// `LTAread(name)` with the run's streams.
fn read_lta_run(g: &mut Globals, r: &mut Run, name: &str) -> Result<Option<Lta>, Fatal> {
    let mut vr = crate::mri_vol2vol::main::Run {
        out: &mut *r.out,
        err: &mut *r.err,
        env: r.env.clone(),
        dir: r.dir,
    };
    read_lta(g, &mut vr, name)
}

/// `check_options()`.
pub fn check_options(g: &mut Globals, r: &mut Run, o: &mut Options) -> Result<(), Fatal> {
    if o.do_proj || o.fill_ribbon {
        o.surf_needed = true;
    }
    let nlabels = o.nlabels();
    if o.use_aparc_plus_aseg && o.subject.is_none() && o.reg_mat_file.is_none() {
        let _ = writeln!(
            r.out,
            "ERROR: you must spec --subject or --reg with --aparc+aseg"
        );
        return Err(Fatal::Exit(1));
    }
    if nlabels == 0 && o.annot_file.is_none() && o.aseg_fspec.is_none() && !o.use_aparc_plus_aseg {
        let _ = writeln!(
            r.out,
            "ERROR: you must spec at least one label or annot file"
        );
        return Err(Fatal::Exit(1));
    }
    if nlabels != 0 && o.annot_file.is_some() {
        let _ = writeln!(r.out, "ERROR: you cannot specify a label AND an annot file");
        return Err(Fatal::Exit(1));
    }
    if nlabels != 0 && o.aseg_fspec.is_some() {
        let _ = writeln!(r.out, "ERROR: you cannot specify a label AND an aseg file");
        return Err(Fatal::Exit(1));
    }
    if o.annot_file.is_some() && o.aseg_fspec.is_some() {
        let _ = writeln!(
            r.out,
            "ERROR: you cannot specify an annot file AND an aseg file"
        );
        return Err(Fatal::Exit(1));
    }
    if o.out_vol_id.is_none() {
        let _ = writeln!(r.out, "ERROR: no output specified");
        return Err(Fatal::Exit(1));
    }
    if o.surf_needed {
        if o.subject.is_none() {
            let _ = writeln!(r.out, "ERROR: subject needed in order to load surface.");
            return Err(Fatal::Exit(1));
        }
        if o.hemi.is_none() {
            let _ = writeln!(r.out, "ERROR: hemi needed in order to load surface.");
            return Err(Fatal::Exit(1));
        }
    }
    if o.subject.is_some() && !o.do_proj && o.annot_file.is_none() && !o.use_aparc_plus_aseg {
        let _ = writeln!(r.out, "INFO: subject not needed, igorning.");
    }
    if o.hemi.is_some() && !o.do_proj && o.annot_file.is_none() && !o.fill_ribbon {
        let _ = writeln!(r.out, "INFO: hemi not needed, igorning.");
    }
    if o.do_label_stat_vol && nlabels == 0 {
        let _ = writeln!(
            r.out,
            "ERROR: {}: must specify a --label with --label-stat",
            g.progname
        );
        return Err(Fatal::Exit(1));
    }
    if !o.reg_identity && !o.reg_header && o.reg_mat_file.is_none() {
        let _ = writeln!(r.out, "ERROR: you must specify a registration method:");
        let _ = writeln!(r.out, "  Either: --reg, --regheader, or --identity");
        return Err(Fatal::Exit(1));
    }
    if o.reg_header && o.reg_mat_file.is_some() {
        let _ = writeln!(r.out, "ERROR: cannot spec --regheader and --reg");
        return Err(Fatal::Exit(1));
    }
    if o.reg_identity && o.reg_mat_file.is_some() {
        let _ = writeln!(r.out, "ERROR: cannot spec --identity and --reg");
        return Err(Fatal::Exit(1));
    }
    if o.reg_identity && o.reg_header {
        let _ = writeln!(r.out, "ERROR: cannot spec --identity and --regheader");
        return Err(Fatal::Exit(1));
    }
    if o.subjects_dir.is_none() {
        match r.env.get("SUBJECTS_DIR") {
            Some(v) => o.subjects_dir = Some(v.to_string_lossy().into_owned()),
            None => {
                let _ = writeln!(r.out, "ERROR: SUBJECTS_DIR not defined in environment");
                return Err(Fatal::Exit(1));
            }
        }
    }
    if o.temp_vol_id.is_none() && !o.fill_ribbon {
        let _ = writeln!(r.out, "ERROR: must specify template volume with --temp");
        return Err(Fatal::Exit(1));
    }
    Ok(())
}

/// `%s` of a C string that may be `NULL` (`(null)`, as glibc prints it).
pub fn or_null(s: Option<&str>) -> String {
    s.map_or_else(|| "(null)".to_owned(), str::to_owned)
}

/// `%g` of a `double`.
pub fn g_fmt(x: f64) -> String {
    larmorx_core::cfmt::g(x, 6)
}

/// `dump_options(stdout)`.
pub fn dump_options(r: &mut Run, o: &Options) {
    let out = &mut *r.out;
    let _ = writeln!(out, "Number of labels: {}", o.nlabels());
    for l in &o.label_list {
        let _ = writeln!(out, "{l}");
    }
    let _ = writeln!(out, "Annot File:      {}", or_null(o.annot_file.as_deref()));
    let _ = writeln!(
        out,
        "Template Volume: {}",
        or_null(o.temp_vol_id.as_deref())
    );
    let _ = writeln!(out, "Outut Volume: {}", or_null(o.out_vol_id.as_deref()));
    let _ = writeln!(
        out,
        "Registration File: {}",
        or_null(o.reg_mat_file.as_deref())
    );
    let _ = writeln!(out, "Fill Threshold: {}", g_fmt(o.fill_thresh));
    let _ = writeln!(out, "Label Vox Vol:  {}", g_fmt(o.label_vox_vol));
    let _ = writeln!(out, "ProjType:       {}", or_null(o.proj_type.as_deref()));
    let _ = writeln!(out, "ProjTypeId:     {}", o.proj_type_id);
    let _ = writeln!(out, "ProjStart:      {}", g_fmt(o.proj_start));
    let _ = writeln!(out, "ProjStop:       {}", g_fmt(o.proj_stop));
    let _ = writeln!(out, "ProjDelta:      {}", g_fmt(o.proj_delta));
    let subject = o
        .subject
        .as_ref()
        .map(|s| String::from_utf8_lossy(s).into_owned());
    let _ = writeln!(out, "Subject:  {}", or_null(subject.as_deref()));
    let _ = writeln!(out, "Hemi:     {}", or_null(o.hemi.as_deref()));
    let _ = writeln!(out, "UseNewASeg2Vol:  {}", i32::from(o.use_new_aseg2vol));
    let _ = writeln!(out, "DoLabelStatVol  {}", i32::from(o.do_label_stat_vol));
    let _ = writeln!(out, "LabelCodeOffset  {}", o.label_code_offset);
    let _ = writeln!(
        out,
        "setenv SUBJECTS_DIR {}",
        or_null(o.subjects_dir.as_deref())
    );
}
