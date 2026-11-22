// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_segstats`'s command line (`mri_segstats.cpp` 102-200 the file-scope settings, 1500-2132
//! `parse_commandline`, 2134-2171 the usage, help, version and `argnerr`, 2173-2241
//! `check_options`, 2244-2257 `dump_options`, 2259-2273 `singledash`).
//!
//! The options are compared with case (`strcmp`) but for `--help`, `--usage`, `-h`, `-u`,
//! `--version`, `--debug`, `--newprint`, `--no-newprint`, `--dontrun`, `--nonempty`,
//! `--non-empty`, `--empty`, `--no-cached`, `--robust`, `--maskinvert`,
//! `--gtm-default-seg-merge(-choroid)`, `--replace` and `--seed` (`strcasecmp`). A number that
//! cannot be read leaves the setting as it was (`sscanf`). Missing arguments end with `ERROR:
//! <option> flag needs <n> argument(s)` on standard error (status 255); an unknown option with
//! `ERROR: Option <option> unknown` (and `       Did you really mean -<option> ?` for a single
//! dash), status 255. `--help` and `--version` print and end with status 1; no argument: the
//! usage, status 1. Some options act where they stand: `--reg` reads its registration, `--sd`
//! sets `SUBJECTS_DIR`, `--stiv` reads its file, `--seed` seeds the random numbers, `--masksign`
//! is checked, and four are programs of their own that end the run: `--xfm2etiv`,
//! `--rescale-by-seg`, `--sum-in` (status 1 after its table) and `--qa-stats`.
//!
//! The defects: FS-1103 (`--mul`, `--div`, `--rescale-by-seg`: a number that cannot be read),
//! FS-1104 (`--vox` with fewer than three numbers), FS-1105 (`--annot` and `--xfm2etiv`'s
//! messages), FS-1107 (`--gtm-default-seg-merge-choroid`), FS-1109 (more than 1,000 ids,
//! exclusions or replacements), FS-1110 (`--slabel`'s exclusion of 0); FS-1108 in
//! [`super::gtm`].

use std::io::Write;
use std::path::Path;

use larmorx_core::cfmt;
use larmorx_core::defects::{FS_1103, FS_1104, FS_1105, FS_1107, FS_1109, FS_1110};
use larmorx_freesurfer_core::cmdargs::{cmd_arg_n_err, cmd_nth_is_arg, cmd_single_dash};
use larmorx_freesurfer_core::cscan::{Bytes, Scanf};
use larmorx_freesurfer_core::cstd::{atoi, strtol10};
use larmorx_freesurfer_core::fsenv::fsenv_set_subjects_dir;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::help_xml::output_help_xml;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::matrix::Matrix;
use larmorx_freesurfer_core::random::set_random_seed;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_core::version::get_version;
use larmorx_freesurfer_gca::transform::GcaLtaIo;
use larmorx_freesurfer_io::fio::fio_file_exists_readable;
use larmorx_freesurfer_io::registerio::regio_read_registermat;
use larmorx_freesurfer_volume::combine::mri_estimate_tiv;

use super::gtm::{Replacements, gtm_default_seg_replacment_list, gtm_load_replacment_list};
use super::qa::{count_edits, rescale_by_seg_option};
use super::table::{dump_stat_sum_table, load_stat_sum_file};

/// `mri_segstats.help.xml`, the text `print_usage` renders (`outputHelpXml`).
pub const HELP_XML: &[u8] = include_bytes!("mri_segstats.help.xml");

/// FreeSurfer's room for ids (`UserSegIdList`), exclusions (`ExclSegIdList`) and replacements
/// (`SrcReplace`, `TrgReplace`): 1,000 each (FS-1109 past it).
pub const LIST_ROOM: usize = 1000;

/// eTIV's scale factor for `talairach.xfm` (`MRIestimateTIV`).
pub const ETIV_SCALE: f64 = 1948.106;

/// The program's streams, environment and folder. The environment is the program's own copy:
/// `--sd` sets `SUBJECTS_DIR` in it (`setenv`).
pub struct Run<'a, 'b> {
    pub out: &'a mut dyn Write,
    pub err: &'a mut dyn Write,
    pub env: Environment,
    pub dir: Option<&'b Path>,
    /// The threads of the ids' statistics (`Settings::threads`; 0: every logical CPU).
    pub threads: usize,
}

/// `mri_segstats`'s settings (its file-scope variables), FreeSurfer's defaults.
pub struct Options {
    pub seg_vol_file: Option<String>,
    pub in_vol_file: Option<String>,
    pub in_vol_reg_file: Option<String>,
    pub in_vol_reg: Option<Matrix>,
    pub in_vol_reg_header: bool,
    pub in_intensity_name: String,
    pub in_intensity_units: String,
    pub mask_vol_file: Option<String>,
    pub pv_vol_file: Option<String>,
    pub brain_mask_file: Option<String>,
    pub stat_table_file: Option<String>,
    pub frame_avg_file: Option<String>,
    pub frame_avg_vol_file: Option<String>,
    pub spat_frame_avg_file: Option<String>,
    pub do_frame_avg: bool,
    pub do_frame_sum: bool,
    pub rm_frame_avg_mn: bool,
    pub norm_frame_avg_mn: f64,
    pub do_accumulate: bool,
    pub frame: i32,
    pub debug: bool,
    pub dontrun: bool,
    pub non_empty_only: bool,
    /// `UserSegIdList` with `nUserSegIdList`: the array as FreeSurfer keeps it (FS-1109 past
    /// [`LIST_ROOM`]).
    pub user_seg_ids: Vec<i32>,
    pub n_user_seg_ids: usize,
    pub n_erode_seg: i32,
    pub do_excl_seg_id: bool,
    pub excl_seg_ids: Vec<i32>,
    pub do_excl_ctx_gm_wm: bool,
    pub do_surf_ctx_vol: bool,
    pub do_surf_wm_vol: bool,
    pub do_supra_tent: bool,
    pub gca_file: Option<String>,
    pub mask_thresh: f32,
    pub mask_invert: bool,
    pub mask_frame: i32,
    pub mask_sign: Option<String>,
    pub mask_erode: i32,
    pub do_sub_cort_gray_vol: bool,
    pub do_total_gray_vol: bool,
    pub brain_vol_from_seg: bool,
    pub do_etiv: bool,
    pub do_etiv_only: bool,
    pub do_old_etiv_only: bool,
    pub do_stiv: bool,
    pub stiv: f64,
    pub talxfm_file: Option<String>,
    pub seg_from_input: bool,
    pub ctab_file: Option<String>,
    pub ctab_file_out: Option<String>,
    pub subject: Option<String>,
    pub hemi: Option<String>,
    pub annot: Option<String>,
    pub white_surf_name: String,
    pub vox: [i32; 3],
    pub do_vox: bool,
    pub segbase: i32,
    pub do_square: bool,
    pub do_square_root: bool,
    pub label_file: Option<String>,
    pub label_thresh: f64,
    pub use_label_thresh: bool,
    pub do_multiply: bool,
    pub mult_val: f64,
    pub do_snr: bool,
    pub use_robust: bool,
    pub robust_pct: f32,
    pub do_euler: bool,
    pub do_abs: bool,
    pub use_print_seg_stat: bool,
    pub replace: Replacements,
    pub get_cached_brain_vol_stats: bool,
    pub use_seg_border: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            seg_vol_file: None,
            in_vol_file: None,
            in_vol_reg_file: None,
            in_vol_reg: None,
            in_vol_reg_header: false,
            in_intensity_name: String::new(),
            in_intensity_units: "unknown".to_owned(),
            mask_vol_file: None,
            pv_vol_file: None,
            brain_mask_file: None,
            stat_table_file: None,
            frame_avg_file: None,
            frame_avg_vol_file: None,
            spat_frame_avg_file: None,
            do_frame_avg: false,
            do_frame_sum: false,
            rm_frame_avg_mn: false,
            norm_frame_avg_mn: 0.0,
            do_accumulate: false,
            frame: 0,
            debug: false,
            dontrun: false,
            non_empty_only: true,
            user_seg_ids: vec![0; LIST_ROOM],
            n_user_seg_ids: 0,
            n_erode_seg: 0,
            do_excl_seg_id: false,
            excl_seg_ids: Vec::new(),
            do_excl_ctx_gm_wm: false,
            do_surf_ctx_vol: false,
            do_surf_wm_vol: false,
            do_supra_tent: false,
            gca_file: None,
            mask_thresh: 0.5,
            mask_invert: false,
            mask_frame: 0,
            mask_sign: None,
            mask_erode: 0,
            do_sub_cort_gray_vol: false,
            do_total_gray_vol: false,
            brain_vol_from_seg: false,
            do_etiv: false,
            do_etiv_only: false,
            do_old_etiv_only: false,
            do_stiv: false,
            stiv: 0.0,
            talxfm_file: None,
            seg_from_input: false,
            ctab_file: None,
            ctab_file_out: None,
            subject: None,
            hemi: None,
            annot: None,
            white_surf_name: "white".to_owned(),
            vox: [0; 3],
            do_vox: false,
            segbase: -1000,
            do_square: false,
            do_square_root: false,
            label_file: None,
            label_thresh: 0.0,
            use_label_thresh: false,
            do_multiply: false,
            mult_val: 1.0,
            do_snr: false,
            use_robust: false,
            robust_pct: 5.0,
            do_euler: false,
            do_abs: false,
            use_print_seg_stat: true,
            replace: Replacements::default(),
            get_cached_brain_vol_stats: true,
            use_seg_border: true,
        }
    }
}

impl Options {
    /// The ids given (`UserSegIdList[0..nUserSegIdList]`).
    pub fn user_ids(&self) -> &[i32] {
        &self.user_seg_ids[..self.n_user_seg_ids]
    }
}

/// What `dump_options` prints besides the settings.
pub struct RunInfo {
    pub cmdline: String,
    pub sysname: String,
    pub hostname: String,
    pub machine: String,
    pub user: String,
}

/// `sscanf(s, "%d", &v)`: the value, or `None` when it assigns none.
pub fn scan_d(g: &mut Globals, s: &str) -> Option<i32> {
    let mut b = Bytes::new(s.as_bytes());
    let mut sc = Scanf::new(&mut b);
    let v = sc.d(&mut g.errno, true);
    if sc.finish(&mut g.errno) == 1 { v } else { None }
}

/// `sscanf(s, "%lf", &v)`, as [`scan_d`].
pub fn scan_lf(g: &mut Globals, s: &str) -> Option<f64> {
    let mut b = Bytes::new(s.as_bytes());
    let mut sc = Scanf::new(&mut b);
    let v = sc.lf(&mut g.errno, true);
    if sc.finish(&mut g.errno) == 1 { v } else { None }
}

/// `sscanf(s, "%f", &v)`, as [`scan_d`].
pub fn scan_f(g: &mut Globals, s: &str) -> Option<f32> {
    let mut b = Bytes::new(s.as_bytes());
    let mut sc = Scanf::new(&mut b);
    let v = sc.f(&mut g.errno, true);
    if sc.finish(&mut g.errno) == 1 { v } else { None }
}

/// A number FreeSurfer reads into a variable it never set (FS-1103): its value; or, when it
/// cannot be read, an error (`reproduce`: 0, FreeSurfer's being memory contents).
pub fn number(
    g: &mut Globals,
    err: &mut dyn Write,
    option: &str,
    arg: &str,
    v: Option<f64>,
) -> Result<f64, Fatal> {
    if let Some(v) = v {
        return Ok(v);
    }
    g.notices.add(
        &FS_1103,
        format!("mri_segstats {option}: {arg} is not a number (FreeSurfer takes memory contents)"),
    );
    if g.notices.reproduce() {
        return Ok(0.0);
    }
    let _ = writeln!(
        err,
        "mri_segstats: {option}: {arg} is not a number (FreeSurfer takes memory contents here)"
    );
    Err(Fatal::Exit(1))
}

/// An entry of one of FreeSurfer's 1,000-place lists: past it, FS-1109 (both modes keep every
/// entry).
pub fn note_room(g: &mut Globals, len: usize, what: &str) {
    if len > LIST_ROOM {
        g.notices.add(
            &FS_1109,
            format!(
                "mri_segstats: {len} {what} kept (FreeSurfer's list holds 1,000 and writes past it)"
            ),
        );
    }
}

/// `argnerr(option, n)`: `ERROR: <option> flag needs <n> argument(s)`, `exit(-1)`.
pub fn argnerr(err: &mut dyn Write, option: &str, n: i32) -> Fatal {
    cmd_arg_n_err(err, option, n)
}

/// `print_usage()`: the help of `mri_segstats.help.xml`.
pub fn print_usage(g: &mut Globals, r: &mut Run) {
    output_help_xml(g, r.out, r.err, Some(HELP_XML));
}

/// `usage_exit()`: the usage, status 1.
pub fn usage_exit(g: &mut Globals, r: &mut Run) -> Fatal {
    print_usage(g, r);
    Fatal::Exit(1)
}

/// `print_version()`: the version, status 1.
fn print_version(r: &mut Run) -> Fatal {
    let _ = writeln!(r.out, "{}", get_version());
    Fatal::Exit(1)
}

/// A slot of one of FreeSurfer's lists, growing the vector past [`LIST_ROOM`] (FS-1109).
fn put(list: &mut Vec<i32>, at: usize, v: i32) {
    if list.len() <= at {
        list.resize(at + 1, 0);
    }
    list[at] = v;
}

/// `parse_commandline(argc, argv)` on the arguments after the program's name. The stand-alone
/// options end the program here (`Err`, their status).
pub fn parse_commandline(
    g: &mut Globals,
    r: &mut Run,
    o: &mut Options,
    args: &[String],
) -> Result<(), Fatal> {
    if args.is_empty() {
        return Err(usage_exit(g, r));
    }
    // "It was previously using a different random sequence every time" (FreeSurfer's comment)
    set_random_seed(g, 4321);
    let mut k = 0;
    while k < args.len() {
        let option = args[k].as_str();
        if o.debug {
            let _ = writeln!(r.out, "{} {option}", args.len() - k);
        }
        k += 1;
        let rest = &args[k..];
        let nargc = rest.len();
        let mut used = 0usize;
        let ci = |name: &str| option.eq_ignore_ascii_case(name);
        let is = |name: &str| option == name;
        if ci("--help") || ci("--usage") || ci("-h") || ci("-u") {
            print_usage(g, r);
            return Err(Fatal::Exit(1));
        } else if ci("--version") {
            return Err(print_version(r));
        } else if ci("--debug") {
            o.debug = true;
        } else if ci("--newprint") {
            o.use_print_seg_stat = true;
        } else if ci("--no-newprint") {
            o.use_print_seg_stat = false;
        } else if ci("--dontrun") {
            o.dontrun = true;
        } else if ci("--nonempty") || ci("--non-empty") {
            o.non_empty_only = true;
        } else if ci("--empty") {
            o.non_empty_only = false;
        } else if ci("--no-cached") {
            o.get_cached_brain_vol_stats = false;
        } else if is("--brain-vol-from-seg") {
            o.brain_vol_from_seg = true;
        } else if is("--subcortgray") {
            o.do_sub_cort_gray_vol = true;
        } else if is("--supratent") {
            o.do_supra_tent = true;
        } else if is("--totalgray") {
            o.do_total_gray_vol = true;
        } else if is("--etiv") {
            o.do_etiv = true;
        } else if is("--etiv-only") {
            o.do_etiv_only = true;
        } else if is("--old-etiv-only") {
            o.do_old_etiv_only = true;
        } else if is("--stiv") {
            if nargc < 1 {
                return Err(argnerr(r.err, option, 1));
            }
            if fio_file_exists_readable(g, r.dir, &rest[0]) != 0 {
                // fscanf(fp, "%lf", &stiv) of the file's first number (unchanged when none)
                let path = larmorx_freesurfer_core::utils::resolve(r.dir, &rest[0]);
                let bytes = std::fs::read(path).unwrap_or_default();
                let mut b = Bytes::new(&bytes);
                let mut sc = Scanf::new(&mut b);
                if let Some(v) = sc.lf(&mut g.errno, true) {
                    o.stiv = v;
                }
            } else if let Some(v) = scan_lf(g, &rest[0]) {
                o.stiv = v;
            }
            o.do_stiv = true;
            used = 1;
        } else if is("--surf-ctx-vol") {
            o.do_surf_ctx_vol = true;
        } else if is("--surf-wm-vol") {
            o.do_surf_wm_vol = true;
        } else if is("--no-global-stats") {
            o.do_surf_wm_vol = false;
            o.do_surf_ctx_vol = false;
            o.do_supra_tent = false;
            o.brain_vol_from_seg = false;
            o.do_sub_cort_gray_vol = false;
            o.do_total_gray_vol = false;
            o.do_euler = false;
        } else if is("--euler") {
            o.do_euler = true;
        } else if is("--abs") {
            o.do_abs = true;
        } else if is("--sqr") {
            o.do_square = true;
        } else if is("--sqrt") {
            o.do_square_root = true;
        } else if is("--snr") {
            o.do_snr = true;
        } else if is("--segborder") {
            o.use_seg_border = true;
        } else if is("--no-segborder") {
            o.use_seg_border = false;
        } else if is("--acc") || is("--accumulate") {
            o.do_accumulate = true;
        } else if is("--mul") || is("--div") {
            if nargc < 1 {
                return Err(argnerr(r.err, option, 1));
            }
            o.do_multiply = true;
            let v = scan_lf(g, &rest[0]);
            let v = number(g, r.err, option, &rest[0], v)?;
            if is("--mul") {
                o.mult_val *= v;
            } else {
                o.mult_val /= v;
            }
            used = 1;
        } else if ci("--robust") {
            if nargc < 1 {
                return Err(argnerr(r.err, option, 1));
            }
            if let Some(v) = scan_f(g, &rest[0]) {
                o.robust_pct = v;
            }
            o.use_robust = true;
            used = 1;
        } else if is("--talxfm") {
            if nargc < 1 {
                return Err(argnerr(r.err, option, 1));
            }
            o.talxfm_file = Some(rest[0].clone());
            used = 1;
        } else if is("--sd") {
            if nargc < 1 {
                return Err(argnerr(r.err, option, 1));
            }
            fsenv_set_subjects_dir(&mut r.env, &rest[0]);
            used = 1;
        } else if is("--ctab-default") {
            let home = r
                .env
                .get("FREESURFER_HOME")
                .map(|v| v.to_string_lossy().into_owned())
                .unwrap_or_else(|| "(null)".to_owned());
            let name = format!("{home}/FreeSurferColorLUT.txt");
            // a calloc of 1,000 bytes: FS-1102 past it
            super::main::note_name(g, &name, 1000, "--ctab-default's colour table");
            let _ = writeln!(r.out, "Using defalt ctab {name}");
            o.ctab_file = Some(name);
        } else if is("--ctab-gca") {
            if nargc < 1 {
                return Err(argnerr(r.err, option, 1));
            }
            o.gca_file = Some(rest[0].clone());
            used = 1;
        } else if is("--seg") {
            if nargc < 1 {
                return Err(argnerr(r.err, option, 1));
            }
            o.seg_vol_file = Some(rest[0].clone());
            used = 1;
        } else if is("--seg-from-input") {
            o.seg_from_input = true;
            o.user_seg_ids[0] = 1;
            o.n_user_seg_ids = 1;
        } else if is("--seg-erode") {
            if nargc < 1 {
                return Err(argnerr(r.err, option, 1));
            }
            if let Some(v) = scan_d(g, &rest[0]) {
                o.n_erode_seg = v;
            }
            used = 1;
        } else if is("--vox") {
            if nargc < 1 {
                return Err(argnerr(r.err, option, 3));
            }
            if nargc < 3 {
                // sscanf of the command line's end (NULL): FreeSurfer crashes
                g.notices.add(
                    &FS_1104,
                    "mri_segstats --vox: fewer than three numbers (FreeSurfer reads past the command line and crashes)",
                );
                if g.notices.reproduce() {
                    return Err(Fatal::Exit(139));
                }
                return Err(argnerr(r.err, option, 3));
            }
            for (i, a) in rest[..3].iter().enumerate() {
                if let Some(v) = scan_d(g, a) {
                    o.vox[i] = v;
                }
            }
            o.do_vox = true;
            used = 3;
        } else if is("--in") || is("--i") {
            if nargc < 1 {
                return Err(argnerr(r.err, option, 1));
            }
            o.in_vol_file = Some(rest[0].clone());
            used = 1;
        } else if is("--reg") {
            if nargc < 1 {
                return Err(argnerr(r.err, option, 1));
            }
            o.in_vol_reg_file = Some(rest[0].clone());
            let mut io = super::main::RegLta { env: &r.env };
            let m = regio_read_registermat(g, r.out, r.err, r.dir, &rest[0], &mut io)?;
            let Some(m) = m else {
                return Err(Fatal::Exit(1));
            };
            o.in_vol_reg = Some(m);
            used = 1;
        } else if is("--regheader") {
            o.in_vol_reg_header = true;
        } else if is("--xfm2etiv") {
            if nargc < 2 {
                return Err(short_of(g, r, option, 1, 2));
            }
            return Err(xfm2etiv(g, r, rest)?);
        } else if is("--rescale-by-seg") {
            if nargc < 5 {
                return Err(argnerr(r.err, option, 5));
            }
            return Err(rescale_by_seg_option(g, r, rest)?);
        } else if is("--in-intensity-name") {
            if nargc < 1 {
                return Err(argnerr(r.err, option, 1));
            }
            o.in_intensity_name = rest[0].clone();
            used = 1;
        } else if is("--in-intensity-units") {
            if nargc < 1 {
                return Err(argnerr(r.err, option, 1));
            }
            o.in_intensity_units = rest[0].clone();
            used = 1;
        } else if is("--brainmask") {
            if nargc < 1 {
                return Err(argnerr(r.err, option, 1));
            }
            o.brain_mask_file = Some(rest[0].clone());
            used = 1;
        } else if is("--id") {
            if nargc < 1 {
                return Err(argnerr(r.err, option, 1));
            }
            let mut nth = 0;
            while cmd_nth_is_arg(rest, nth) {
                let at = o.n_user_seg_ids;
                if let Some(v) = scan_d(g, &rest[nth]) {
                    put(&mut o.user_seg_ids, at, v);
                } else if o.user_seg_ids.len() <= at {
                    o.user_seg_ids.resize(at + 1, 0);
                }
                o.n_user_seg_ids += 1;
                nth += 1;
            }
            note_room(g, o.n_user_seg_ids, "segmentation ids (--id)");
            used = nth;
        } else if is("--excl-ctxgmwm") {
            o.do_excl_seg_id = true;
            o.do_excl_ctx_gm_wm = true;
            o.excl_seg_ids.extend_from_slice(&[2, 3, 41, 42]);
            note_room(g, o.excl_seg_ids.len(), "excluded ids");
        } else if is("--excludeid") || is("--exclude") {
            if nargc < 1 {
                return Err(argnerr(r.err, option, 1));
            }
            let mut nth = 0;
            while cmd_nth_is_arg(rest, nth) {
                // an unreadable id leaves the slot as it was: 0 (a zeroed array)
                o.excl_seg_ids.push(scan_d(g, &rest[nth]).unwrap_or(0));
                nth += 1;
            }
            note_room(g, o.excl_seg_ids.len(), "excluded ids");
            o.do_excl_seg_id = true;
            used = nth;
        } else if is("--mask") {
            if nargc < 1 {
                return Err(argnerr(r.err, option, 1));
            }
            o.mask_vol_file = Some(rest[0].clone());
            used = 1;
        } else if is("--masksign") {
            if nargc < 1 {
                return Err(argnerr(r.err, option, 1));
            }
            let s = rest[0].as_bytes();
            let ok = |w: &[u8]| s.len() >= 3 && s[..3].eq_ignore_ascii_case(w);
            o.mask_sign = Some(rest[0].clone());
            used = 1;
            if !ok(b"abs") && !ok(b"pos") && !ok(b"neg") {
                let _ = writeln!(
                    r.err,
                    "ERROR: mask sign = {}, must be abs, pos, or neg",
                    rest[0]
                );
                return Err(Fatal::Exit(1));
            }
        } else if is("--maskthresh") {
            if nargc < 1 {
                return Err(argnerr(r.err, option, 1));
            }
            if let Some(v) = scan_f(g, &rest[0]) {
                o.mask_thresh = v;
            }
            used = 1;
        } else if is("--maskframe") {
            if nargc < 1 {
                return Err(argnerr(r.err, option, 1));
            }
            if let Some(v) = scan_d(g, &rest[0]) {
                o.mask_frame = v;
            }
            used = 1;
        } else if is("--maskerode") {
            if nargc < 1 {
                return Err(argnerr(r.err, option, 1));
            }
            if let Some(v) = scan_d(g, &rest[0]) {
                o.mask_erode = v;
            }
            used = 1;
        } else if ci("--maskinvert") {
            o.mask_invert = true;
        } else if is("--sum") || is("--o") {
            if nargc < 1 {
                return Err(argnerr(r.err, option, 1));
            }
            o.stat_table_file = Some(rest[0].clone());
            used = 1;
        } else if is("--sum-in") {
            if nargc < 1 {
                return Err(argnerr(r.err, option, 1));
            }
            let table = load_stat_sum_file(g, r, &rest[0])?;
            let _ = writeln!(r.out, "Found {}", table.len());
            dump_stat_sum_table(r.out, &table);
            return Err(Fatal::Exit(1));
        } else if is("--avgwf") {
            if nargc < 1 {
                return Err(argnerr(r.err, option, 1));
            }
            o.frame_avg_file = Some(rest[0].clone());
            o.do_frame_avg = true;
            used = 1;
        } else if is("--avgwf-remove-mean") {
            o.rm_frame_avg_mn = true;
        } else if is("--avgwf-norm-mean") {
            if nargc < 1 {
                return Err(argnerr(r.err, option, 1));
            }
            if let Some(v) = scan_lf(g, &rest[0]) {
                o.norm_frame_avg_mn = v;
            }
            used = 1;
        } else if is("--sumwf") {
            if nargc < 1 {
                return Err(argnerr(r.err, option, 1));
            }
            o.frame_avg_file = Some(rest[0].clone());
            o.do_frame_avg = true;
            o.do_frame_sum = true;
            used = 1;
        } else if is("--sfavg") {
            if nargc < 1 {
                return Err(argnerr(r.err, option, 1));
            }
            o.spat_frame_avg_file = Some(rest[0].clone());
            o.do_frame_avg = true;
            used = 1;
        } else if is("--avgwfvol") {
            if nargc < 1 {
                return Err(argnerr(r.err, option, 1));
            }
            o.frame_avg_vol_file = Some(rest[0].clone());
            o.do_frame_avg = true;
            used = 1;
        } else if is("--ctab") {
            if nargc < 1 {
                return Err(argnerr(r.err, option, 1));
            }
            o.ctab_file = Some(rest[0].clone());
            used = 1;
        } else if is("--ctab-out") {
            if nargc < 1 {
                return Err(argnerr(r.err, option, 1));
            }
            o.ctab_file_out = Some(rest[0].clone());
            used = 1;
        } else if is("--frame") {
            if nargc < 1 {
                return Err(argnerr(r.err, option, 1));
            }
            if let Some(v) = scan_d(g, &rest[0]) {
                o.frame = v;
            }
            used = 1;
        } else if is("--subject") {
            if nargc < 1 {
                return Err(argnerr(r.err, option, 1));
            }
            o.subject = Some(rest[0].clone());
            used = 1;
        } else if is("--qa-stats") {
            if nargc < 2 {
                return Err(argnerr(r.err, option, 2));
            }
            let st = count_edits(g, r, &rest[0], Some(&rest[1]))?;
            return Err(Fatal::Exit(st));
        } else if is("--annot") {
            if nargc < 3 {
                return Err(short_of(g, r, option, 1, 3));
            }
            o.subject = Some(rest[0].clone());
            o.hemi = Some(rest[1].clone());
            o.annot = Some(rest[2].clone());
            used = 3;
        } else if is("--surf") {
            if nargc < 1 {
                return Err(argnerr(r.err, option, 1));
            }
            o.white_surf_name = rest[0].clone();
            used = 1;
        } else if is("--slabel") {
            if nargc < 3 {
                return Err(argnerr(r.err, option, 3));
            }
            o.subject = Some(rest[0].clone());
            o.hemi = Some(rest[1].clone());
            o.label_file = Some(rest[2].clone());
            // `ExclSegId = 0; DoExclSegId = 1;`: the exclusion of 0 set on a variable nothing
            // reads (FS-1110: larmorx excludes 0)
            g.notices.add(
                &FS_1110,
                "mri_segstats --slabel: segmentation 0 (the vertices outside the label) excluded as meant (FreeSurfer reports it)",
            );
            if !g.notices.reproduce() {
                o.excl_seg_ids.push(0);
                note_room(g, o.excl_seg_ids.len(), "excluded ids");
            }
            o.do_excl_seg_id = true;
            used = 3;
        } else if is("--label-thresh") {
            if nargc < 1 {
                return Err(argnerr(r.err, option, 1));
            }
            if let Some(v) = scan_lf(g, &rest[0]) {
                o.label_thresh = v;
            }
            o.use_label_thresh = true;
            used = 1;
        } else if is("--segbase") {
            if nargc < 1 {
                return Err(argnerr(r.err, option, 1));
            }
            if let Some(v) = scan_d(g, &rest[0]) {
                o.segbase = v;
            }
            used = 1;
        } else if is("--synth") {
            if nargc < 1 {
                return Err(argnerr(r.err, option, 1));
            }
            // sscanf(.., "%ld", &seed); synth = 1: neither is used again
            used = 1;
        } else if is("--pv") {
            if nargc < 1 {
                return Err(argnerr(r.err, option, 1));
            }
            o.pv_vol_file = Some(rest[0].clone());
            used = 1;
        } else if ci("--gtm-default-seg-merge") {
            gtm_default_seg_replacment_list(&mut o.replace);
            note_room(g, o.replace.n as usize, "replacements");
        } else if ci("--gtm-default-seg-merge-choroid") {
            gtm_default_seg_replacment_list(&mut o.replace);
            note_room(g, o.replace.n as usize, "replacements");
            // "Last two items are choroid": FreeSurfer's list no longer holds them (FS-1107)
            g.notices.add(
                &FS_1107,
                "mri_segstats --gtm-default-seg-merge-choroid: the default list kept whole (FreeSurfer drops its last two replacements, 62 and 80 to 0, as if they were the choroid plexus)",
            );
            if g.notices.reproduce() {
                o.replace.n -= 2;
            }
        } else if is("--replace-file") {
            if nargc < 1 {
                return Err(cmd_arg_n_err(r.err, option, 1));
            }
            if gtm_load_replacment_list(g, r, &rest[0], &mut o.replace)? != 0 {
                return Err(Fatal::Exit(1));
            }
            note_room(g, o.replace.n.max(0) as usize, "replacements");
            used = 1;
        } else if ci("--replace") {
            if nargc < 2 {
                return Err(cmd_arg_n_err(r.err, option, 2));
            }
            let at = o.replace.n.max(0) as usize;
            o.replace.ensure(at + 1);
            if let Some(v) = scan_d(g, &rest[0]) {
                o.replace.src[at] = v;
            }
            if let Some(v) = scan_d(g, &rest[1]) {
                o.replace.trg[at] = v;
            }
            o.replace.n += 1;
            note_room(g, o.replace.n as usize, "replacements");
            used = 2;
        } else if ci("--seed") {
            if nargc < 1 {
                return Err(cmd_arg_n_err(r.err, option, 1));
            }
            set_random_seed(g, strtol10(rest[0].as_bytes()));
            let _ = writeln!(
                r.out,
                "setting seed for random number genererator to {}",
                atoi(rest[0].as_bytes())
            );
            used = 1;
        } else {
            let _ = writeln!(r.err, "ERROR: Option {option} unknown");
            if cmd_single_dash(option) {
                let _ = writeln!(r.err, "       Did you really mean -{option} ?");
            }
            return Err(Fatal::Exit(-1));
        }
        k += used;
    }
    Ok(())
}

/// `argnerr(option, said)` where the option needs `needs` arguments (FS-1105: FreeSurfer's
/// message says `said`; larmorx's the number it needs).
fn short_of(g: &mut Globals, r: &mut Run, option: &str, said: i32, needs: i32) -> Fatal {
    g.notices.add(
        &FS_1105,
        format!("mri_segstats {option}: the message says {needs} arguments (FreeSurfer's says {said})"),
    );
    let n = if g.notices.reproduce() { said } else { needs };
    argnerr(r.err, option, n)
}

/// `--xfm2etiv xfm outfile|nofile`: eTIV of the transform (`%12.4lf`), printed and written; the
/// program's end (status 0, or 1 when the file does not open).
fn xfm2etiv(g: &mut Globals, r: &mut Run, rest: &[String]) -> Result<Fatal, Fatal> {
    let mut io = GcaLtaIo { env: &r.env };
    let (icv, _det) = mri_estimate_tiv(g, r.out, r.err, r.dir, &rest[0], ETIV_SCALE, &mut io)?;
    let line = format!("{}\n", cfmt::pad(&cfmt::f(icv, 4), 12));
    let _ = r.out.write_all(line.as_bytes());
    if rest[1] != "nofile" {
        let path = larmorx_freesurfer_core::utils::resolve(r.dir, &rest[1]);
        match std::fs::File::create(path) {
            Ok(mut f) => {
                let _ = f.write_all(line.as_bytes());
            }
            Err(_) => return Ok(Fatal::Exit(1)),
        }
    }
    Ok(Fatal::Exit(0))
}

/// `check_options()`.
pub fn check_options(r: &mut Run, o: &mut Options) -> Result<(), Fatal> {
    let fail = |r: &mut Run, msg: &str| -> Result<(), Fatal> {
        let _ = writeln!(r.out, "ERROR: {msg}");
        Err(Fatal::Exit(1))
    };
    if o.seg_vol_file.is_none()
        && o.annot.is_none()
        && o.label_file.is_none()
        && !o.do_etiv_only
        && !o.do_old_etiv_only
        && !o.seg_from_input
    {
        return fail(r, "must specify a segmentation volume");
    }
    if o.seg_from_input && o.in_vol_file.is_none() {
        return fail(r, "must specify an input volume with --seg-from-input");
    }
    if o.stat_table_file.is_none()
        && o.frame_avg_file.is_none()
        && !o.do_etiv_only
        && !o.do_old_etiv_only
        && o.frame_avg_vol_file.is_none()
    {
        return fail(r, "must specify an output table file");
    }
    if o.do_frame_avg && o.in_vol_file.is_none() {
        return fail(r, "cannot do frame average without input volume");
    }
    if o.do_etiv && o.subject.is_none() {
        return fail(r, "need subject with --etiv");
    }
    if o.do_supra_tent && !o.do_surf_ctx_vol {
        return fail(r, "need --surf-ctx-vol  with --supratent");
    }
    if o.ctab_file.is_some() && o.gca_file.is_some() {
        return fail(r, "cannot specify ctab and gca");
    }
    if o.do_surf_ctx_vol && o.subject.is_none() {
        return fail(r, "need --subject with --surf-ctx-vol");
    }
    if o.do_total_gray_vol && !o.do_surf_ctx_vol {
        return fail(r, "need --surf-ctx-vol with --totalgray");
    }
    if o.ctab_file_out.is_some() && o.ctab_file.is_none() {
        return fail(r, "need an input ctab to create output ctab");
    }
    if let (Some(a), Some(b)) = (&o.ctab_file_out, &o.ctab_file)
        && a == b
    {
        return fail(r, "output ctab is the same as input");
    }
    if o.mask_sign.is_none() {
        o.mask_sign = Some("abs".to_owned());
    }
    if o.do_euler && o.subject.is_none() {
        return fail(r, "need subject with --euler");
    }
    Ok(())
}

/// `dump_options(fp)`. Its `cwd` line is always empty: the program never fills it (rule C,
/// findings).
pub fn dump_options(fp: &mut dyn Write, o: &Options, info: &RunInfo) {
    let _ = writeln!(fp);
    let _ = writeln!(fp, "{}", get_version());
    let _ = writeln!(fp, "cwd ");
    let _ = writeln!(fp, "cmdline {}", info.cmdline);
    let _ = writeln!(fp, "sysname  {}", info.sysname);
    let _ = writeln!(fp, "hostname {}", info.hostname);
    let _ = writeln!(fp, "machine  {}", info.machine);
    let _ = writeln!(fp, "user     {}", info.user);
    let _ = writeln!(fp, "whitesurfname  {}", o.white_surf_name);
    let _ = writeln!(fp, "UseRobust  {}", i32::from(o.use_robust));
}
