// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_binarize`'s command line (`mri_binarize.cpp` 712-1269 `parse_commandline`, 1275-1355
//! `print_usage`, 1357-1517 `print_help`, 1519-1522 `print_version`, 1524-1567 `check_options`,
//! 1570-1612 `dump_options`).
//!
//! Every option is compared without case (`strcasecmp`); a number that cannot be read leaves the
//! setting as it was (`sscanf`), a match value or replacement 0. Missing arguments end with
//! `ERROR: <option> flag needs <n> argument(s)` (status 255); an unknown option with `ERROR:
//! Option <option> unknown` (and `       Did you really mean -<option> ?` for a single dash) on
//! standard error, status 255. `--help` and `--version` print and end with status 1. Three options
//! are programs of their own, run where they stand and ending the program with their write's
//! status: `--ohe`, `--crop-around-ras` and `--dilate-vertex(-sum)`.
//!
//! The defects: FS-1023 (more than 1,000 match values or replacements), FS-1024 (`--dilate-vertex`'s
//! usage line), FS-1026 (a number of the stand-alone options that cannot be read).

use std::io::Write;
use std::path::Path;

use larmorx_core::cfmt;
use larmorx_core::defects::{FS_1023, FS_1024, FS_1026};
use larmorx_freesurfer_core::cmdargs::{cmd_arg_n_err, cmd_nth_is_arg, cmd_single_dash};
use larmorx_freesurfer_core::cscan::{Bytes, Scanf};
use larmorx_freesurfer_core::cstd::atoi;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_core::utils::find_first_match;
use larmorx_freesurfer_core::version::get_version;
use larmorx_freesurfer_gca::transform::GcaLtaIo;
use larmorx_freesurfer_io::colortab::{
    ctab_get_number_of_total_entries, ctab_is_entry_valid, ctab_read_ascii,
};
use larmorx_freesurfer_io::mriio::{mri_read_volume, mri_write};
use larmorx_freesurfer_mri::ctab::ColorTable;
use larmorx_freesurfer_mri::mri::MGZ_INTENT_UNKNOWN;
use larmorx_freesurfer_surf::aaa::SurfGlobals;
use larmorx_freesurfer_surf::coords::Io;
use larmorx_freesurfer_surf::readwrite;
use larmorx_freesurfer_surf::valrest::dilate_vertex_to_sum;
use larmorx_freesurfer_volume::cma::mri_one_hot_encode;
use larmorx_freesurfer_volume::ltawrite::lta_read;
use larmorx_freesurfer_volume::mri2ops::mri_crop_around_ras;
use larmorx_freesurfer_volume::mriset::{
    NEAREST_NEIGHBOR_CORNER, NEAREST_NEIGHBOR_EDGE, NEAREST_NEIGHBOR_FACE,
};

use super::help::{HELP, USAGE};

/// FreeSurfer's room for match values, replacements and nearest-neighbour replacements (FS-1023).
pub const LIST_ROOM: usize = 1000;

/// The program's streams, environment and folder.
pub struct Run<'a, 'b> {
    pub out: &'a mut dyn Write,
    pub err: &'a mut dyn Write,
    pub env: &'b Environment,
    pub dir: Option<&'b Path>,
}

/// `mri_binarize`'s settings (its file-scope variables), FreeSurfer's defaults.
pub struct Options {
    pub debug: bool,
    pub checkoptsonly: bool,
    pub noverbose: bool,
    pub in_vol: Option<String>,
    pub out_vol: Option<String>,
    pub merge_vol: Option<String>,
    pub copy_vol: Option<String>,
    pub mask_vol: Option<String>,
    pub min_thresh: f64,
    pub max_thresh: f64,
    pub min_set: bool,
    pub max_set: bool,
    pub rmin: f64,
    pub rmax: f64,
    pub rmin_set: bool,
    pub rmax_set: bool,
    pub count_file: Option<String>,
    pub bin_val: i32,
    pub bin_val_not: i32,
    pub frame: i32,
    pub do_frame_loop: bool,
    pub do_abs: bool,
    pub do_neg: bool,
    pub zero_col_edges: bool,
    pub zero_row_edges: bool,
    pub zero_slice_edges: bool,
    pub do_match: bool,
    pub match_values: Vec<i32>,
    pub mask_thresh: f64,
    pub n_erode2d: i32,
    pub n_erode3d: i32,
    pub n_dilate3d: i32,
    pub do_bin_col: bool,
    pub uchar: bool,
    pub do_frame_sum: bool,
    pub do_frame_and: bool,
    pub do_percent: bool,
    pub top_percent: f64,
    pub fdr: f64,
    pub do_fdr: bool,
    pub fdr_sign: i32,
    pub n_erode_nn: i32,
    pub nn_type: i32,
    pub replace_only: bool,
    pub copy_ctab: bool,
    pub transitive_replace: bool,
    pub src_replace: Vec<i32>,
    pub trg_replace: Vec<i32>,
    pub src_replace_nn: Vec<i32>,
    pub window_nn: Vec<i32>,
    pub surf_file: Option<String>,
    pub nsmoothsurf: i32,
    pub cmdlinectab: Option<ColorTable>,
    pub do_bb: bool,
    pub npad_bb: i32,
    pub do_count: bool,
    pub reverse_face_order: bool,
    pub fill_holes: bool,
    pub remove_islands: bool,
    pub fix_vol_topology: bool,
    pub erode_edges: bool,
    /// The last `--threads` (`--nthreads`) value read: the threads FreeSurfer's loops would use.
    pub threads: Option<i32>,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            debug: false,
            checkoptsonly: false,
            noverbose: false,
            in_vol: None,
            out_vol: None,
            merge_vol: None,
            copy_vol: None,
            mask_vol: None,
            min_thresh: 0.0,
            max_thresh: 0.0,
            min_set: false,
            max_set: false,
            rmin: 0.0,
            rmax: 0.0,
            rmin_set: false,
            rmax_set: false,
            count_file: None,
            bin_val: 1,
            bin_val_not: 0,
            frame: 0,
            do_frame_loop: true,
            do_abs: false,
            do_neg: false,
            zero_col_edges: false,
            zero_row_edges: false,
            zero_slice_edges: false,
            do_match: false,
            match_values: Vec::new(),
            mask_thresh: 0.5,
            n_erode2d: 0,
            n_erode3d: 0,
            n_dilate3d: 0,
            do_bin_col: false,
            uchar: false,
            do_frame_sum: false,
            do_frame_and: false,
            do_percent: false,
            top_percent: -1.0,
            fdr: 0.0,
            do_fdr: false,
            fdr_sign: 0,
            n_erode_nn: 0,
            nn_type: 0,
            replace_only: false,
            copy_ctab: false,
            transitive_replace: true,
            src_replace: Vec::new(),
            trg_replace: Vec::new(),
            src_replace_nn: Vec::new(),
            window_nn: Vec::new(),
            surf_file: None,
            nsmoothsurf: 0,
            cmdlinectab: None,
            do_bb: false,
            npad_bb: 0,
            do_count: true,
            reverse_face_order: false,
            fill_holes: false,
            remove_islands: false,
            fix_vol_topology: false,
            erode_edges: false,
            threads: None,
        }
    }
}

/// `sscanf(s, "%d", &v)`: the value, or `None` when it assigns none (`errno` as glibc leaves it).
pub fn scan_d(g: &mut Globals, s: &str) -> Option<i32> {
    let mut b = Bytes::new(s.as_bytes());
    let mut sc = Scanf::new(&mut b);
    let v = sc.d(&mut g.errno, true);
    if sc.finish(&mut g.errno) == 1 {
        v
    } else {
        None
    }
}

/// `sscanf(s, "%lf", &v)`, as [`scan_d`].
pub fn scan_lf(g: &mut Globals, s: &str) -> Option<f64> {
    let mut b = Bytes::new(s.as_bytes());
    let mut sc = Scanf::new(&mut b);
    let v = sc.lf(&mut g.errno, true);
    if sc.finish(&mut g.errno) == 1 {
        v
    } else {
        None
    }
}

/// A list entry FreeSurfer keeps in an array of [`LIST_ROOM`]: past it, FS-1023 (both modes keep
/// every entry).
fn note_room(g: &mut Globals, len: usize, what: &str) {
    if len > LIST_ROOM {
        g.notices.add(
            &FS_1023,
            format!(
                "mri_binarize: {len} {what} kept (FreeSurfer's list holds 1,000 and writes past it)"
            ),
        );
    }
}

/// A number of a stand-alone option that FreeSurfer reads into a variable it never set: its
/// value, or (FS-1026) an error (`reproduce`: 0, FreeSurfer's being memory contents).
fn number<T: Default>(
    g: &mut Globals,
    r: &mut Run,
    option: &str,
    arg: &str,
    v: Option<T>,
) -> Result<T, Fatal> {
    if let Some(v) = v {
        return Ok(v);
    }
    g.notices.add(
        &FS_1026,
        format!("mri_binarize {option}: {arg} is not a number (FreeSurfer takes memory contents)"),
    );
    if g.notices.reproduce() {
        return Ok(T::default());
    }
    let _ = writeln!(
        r.err,
        "mri_binarize: {option}: {arg} is not a number (FreeSurfer takes memory contents here)"
    );
    Err(Fatal::Exit(1))
}

/// `print_usage()`.
pub fn print_usage(r: &mut Run, progname: &str) {
    let _ = write!(r.out, "USAGE: {progname} \n{USAGE}{}\n\n", get_version());
}

/// `usage_exit()`: the usage, status 1.
pub fn usage_exit(r: &mut Run, progname: &str) -> Fatal {
    print_usage(r, progname);
    Fatal::Exit(1)
}

/// `print_help()`: the usage and the help, status 1.
fn print_help(r: &mut Run, progname: &str) -> Fatal {
    print_usage(r, progname);
    let _ = r.out.write_all(HELP.as_bytes());
    Fatal::Exit(1)
}

/// `print_version()`: the version, status 1.
fn print_version(r: &mut Run) -> Fatal {
    let _ = writeln!(r.out, "{}", get_version());
    Fatal::Exit(1)
}

/// Appends a preset's match values.
fn preset(o: &mut Options, values: &[i32]) {
    o.match_values.extend_from_slice(values);
    o.do_match = true;
}

/// The cerebral white matter's labels (`--ctx-wm`, `--wm`).
const WM: [i32; 8] = [2, 41, 77, 251, 252, 253, 254, 255];
/// The cerebellar white matter (`--all-wm` adds them).
const CBWM: [i32; 2] = [7, 46];
/// The ventricles and choroid plexus (`--ventricles`).
const VENT: [i32; 8] = [4, 5, 14, 43, 44, 72, 31, 63];

/// `parse_commandline(argc, argv)` on the arguments after the program's name. The stand-alone
/// options end the program here (`Err`, their status).
pub fn parse_commandline(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    r: &mut Run,
    o: &mut Options,
    args: &[String],
    progname: &str,
) -> Result<(), Fatal> {
    if args.is_empty() {
        return Err(usage_exit(r, progname));
    }
    let mut k = 0;
    while k < args.len() {
        let option = args[k].as_str();
        if o.debug {
            let _ = writeln!(r.out, "{} {option}", args.len() - k);
        }
        k += 1;
        let rest = &args[k..];
        let nargc = rest.len();
        let need = |r: &mut Run, n: usize| -> Result<(), Fatal> {
            if nargc < n {
                Err(cmd_arg_n_err(r.err, option, n as i32))
            } else {
                Ok(())
            }
        };
        let mut used = 0usize;
        let is = |name: &str| option.eq_ignore_ascii_case(name);
        if is("--help") {
            return Err(print_help(r, progname));
        } else if is("--version") {
            return Err(print_version(r));
        } else if is("--debug") {
            o.debug = true;
        } else if is("--noverbose") {
            o.noverbose = true;
        } else if is("--checkopts") {
            o.checkoptsonly = true;
        } else if is("--nocheckopts") {
            o.checkoptsonly = false;
        } else if is("--abs") {
            o.do_abs = true;
        } else if is("--neg") {
            o.do_neg = true;
        } else if is("--bincol") {
            o.do_bin_col = true;
        } else if is("--uchar") {
            o.uchar = true;
        } else if is("--no-count") {
            o.do_count = false;
        } else if is("--reverse") {
            o.reverse_face_order = true;
        } else if is("--zero-edges") {
            o.zero_col_edges = true;
            o.zero_row_edges = true;
            o.zero_slice_edges = true;
        } else if is("--zero-slice-edges") {
            o.zero_slice_edges = true;
        } else if is("--fill-holes") {
            o.fill_holes = true;
        } else if is("--no-fill-holes") {
            o.fill_holes = false;
        } else if is("--remove-islands") {
            o.remove_islands = true;
        } else if is("--no-remove-islands") {
            o.remove_islands = false;
        } else if is("--fix-vol-topo") {
            o.fix_vol_topology = true;
        } else if is("--no-fix-vol-topo") {
            o.fix_vol_topology = false;
        } else if is("--erode-edges") {
            o.erode_edges = true;
        } else if is("--no-erode-edges") {
            o.erode_edges = false;
        } else if is("--ctx-wm") || is("--wm") {
            preset(o, &WM);
        } else if is("--all-wm") {
            preset(o, &WM);
            preset(o, &CBWM);
        } else if is("--ventricles") {
            preset(o, &VENT);
        } else if is("--wm+vcsf") {
            preset(o, &WM);
            preset(o, &CBWM);
            preset(o, &VENT);
        } else if is("--gm") {
            preset(o, &WM);
            preset(o, &CBWM);
            preset(o, &[4, 5, 14, 43, 44, 15, 72, 31, 63, 0, 24]);
            o.bin_val = 0;
            o.bin_val_not = 1;
        } else if is("--scm-lh") {
            preset(o, &[26, 11, 2, 4, 13, 12, 10, 28, 30, 34, 78]);
            o.remove_islands = true;
            o.fill_holes = true;
        } else if is("--scm-rh") {
            preset(o, &[58, 50, 41, 43, 52, 51, 49, 60, 62, 66, 79]);
            o.remove_islands = true;
            o.fill_holes = true;
        } else if is("--i") {
            need(r, 1)?;
            o.in_vol = Some(rest[0].clone());
            used = 1;
        } else if is("--o") {
            need(r, 1)?;
            o.out_vol = Some(rest[0].clone());
            used = 1;
        } else if is("--surf") {
            need(r, 1)?;
            o.surf_file = Some(rest[0].clone());
            used = 1;
        } else if is("--surf-smooth") {
            need(r, 1)?;
            if let Some(v) = scan_d(g, &rest[0]) {
                o.nsmoothsurf = v;
            }
            used = 1;
        } else if is("--merge") {
            need(r, 1)?;
            o.merge_vol = Some(rest[0].clone());
            used = 1;
        } else if is("--copy") {
            need(r, 1)?;
            o.copy_vol = Some(rest[0].clone());
            used = 1;
        } else if is("--mask") {
            need(r, 1)?;
            o.mask_vol = Some(rest[0].clone());
            used = 1;
        } else if is("--mask-thresh") {
            need(r, 1)?;
            if let Some(v) = scan_lf(g, &rest[0]) {
                o.mask_thresh = v;
            }
            used = 1;
        } else if is("--pct") {
            need(r, 1)?;
            if let Some(v) = scan_lf(g, &rest[0]) {
                o.top_percent = v;
            }
            o.min_set = true;
            o.do_percent = true;
            used = 1;
        } else if is("--min") {
            need(r, 1)?;
            if let Some(v) = scan_lf(g, &rest[0]) {
                o.min_thresh = v;
            }
            o.min_set = true;
            used = 1;
        } else if is("--max") {
            need(r, 1)?;
            if let Some(v) = scan_lf(g, &rest[0]) {
                o.max_thresh = v;
            }
            o.max_set = true;
            used = 1;
        } else if is("--rmin") {
            need(r, 1)?;
            if let Some(v) = scan_lf(g, &rest[0]) {
                o.rmin = v;
            }
            o.rmin_set = true;
            used = 1;
        } else if is("--rmax") {
            need(r, 1)?;
            if let Some(v) = scan_lf(g, &rest[0]) {
                o.rmax = v;
            }
            o.rmax_set = true;
            used = 1;
        } else if is("--fdr") {
            need(r, 1)?;
            if let Some(v) = scan_lf(g, &rest[0]) {
                o.fdr = v;
            }
            o.do_fdr = true;
            used = 1;
        } else if is("--ohe") {
            need(r, 3)?;
            return Err(one_hot(g, r, rest));
        } else if is("--threads") || is("--nthreads") {
            need(r, 1)?;
            o.threads = Some(scan_d(g, &rest[0]).unwrap_or(1));
            used = 1;
        } else if is("--fdr-pos") {
            o.fdr_sign = 1;
        } else if is("--fdr-neg") {
            o.fdr_sign = -1;
        } else if is("--fdr-abs") {
            o.fdr_sign = 0;
        } else if is("--bb") || is("--crop") {
            need(r, 1)?;
            if let Some(v) = scan_d(g, &rest[0]) {
                o.npad_bb = v;
            }
            o.do_bb = true;
            used = 1;
        } else if is("--crop-around-ras") {
            need(r, 9)?;
            return Err(crop_around_ras(g, r, rest)?);
        } else if is("--no-transitive-replace") {
            o.transitive_replace = false;
        } else if is("--replace") || is("--replaceonly") {
            need(r, 2)?;
            o.src_replace.push(scan_d(g, &rest[0]).unwrap_or(0));
            o.trg_replace.push(scan_d(g, &rest[1]).unwrap_or(0));
            note_room(g, o.src_replace.len(), "replacements");
            if is("--replaceonly") {
                o.replace_only = true;
            }
            used = 2;
        } else if is("--replace-nn") || is("--replaceonly-nn") {
            need(r, 2)?;
            o.src_replace_nn.push(scan_d(g, &rest[0]).unwrap_or(0));
            o.window_nn.push(scan_d(g, &rest[1]).unwrap_or(0));
            note_room(g, o.src_replace_nn.len(), "nearest-neighbour replacements");
            if is("--replaceonly-nn") {
                o.replace_only = true;
            }
            used = 2;
        } else if is("--binval") {
            need(r, 1)?;
            if let Some(v) = scan_d(g, &rest[0]) {
                o.bin_val = v;
            }
            used = 1;
        } else if is("--binvalnot") {
            need(r, 1)?;
            if let Some(v) = scan_d(g, &rest[0]) {
                o.bin_val_not = v;
            }
            used = 1;
        } else if is("--inv") {
            o.bin_val = 0;
            o.bin_val_not = 1;
        } else if is("--match") {
            need(r, 1)?;
            let mut nth = 0;
            while cmd_nth_is_arg(rest, nth) {
                let v = scan_d(g, &rest[nth]).unwrap_or(0);
                o.match_values.push(v);
                nth += 1;
            }
            used = nth;
            o.do_match = true;
        } else if is("--copy-ctab") {
            o.copy_ctab = true;
        } else if is("--ctab") {
            need(r, 1)?;
            o.cmdlinectab = ctab_read_ascii(g, r.out, r.err, r.dir, &rest[0]);
            used = 1;
        } else if is("--match-ctab") {
            need(r, 1)?;
            let Some(ctab) = ctab_read_ascii(g, r.out, r.err, r.dir, &rest[0]) else {
                return Err(Fatal::Exit(1));
            };
            let mut xids = Vec::new();
            let mut nth = 1;
            while cmd_nth_is_arg(rest, nth) {
                let v = scan_d(g, &rest[nth]);
                xids.push(number(g, r, option, &rest[nth], v)?);
                nth += 1;
            }
            used = nth;
            let ntotal = ctab_get_number_of_total_entries(&ctab);
            for n in 0..ntotal {
                let valid = ctab_is_entry_valid(g, r.err, &ctab, n).unwrap_or(false);
                if !valid || find_first_match(&xids, n) != -1 {
                    continue;
                }
                o.match_values.push(n);
            }
            o.do_match = true;
        } else if is("--subcort-gm") {
            preset(
                o,
                &[
                    10, 11, 12, 13, 17, 18, 26, 28, 27, 8, 49, 50, 51, 52, 53, 54, 58, 60, 59, 47,
                ],
            );
        } else if is("--frame") {
            need(r, 1)?;
            if let Some(v) = scan_d(g, &rest[0]) {
                o.frame = v;
            }
            used = 1;
            o.do_frame_loop = false;
        } else if is("--frame-sum") {
            o.do_frame_sum = true;
            o.do_frame_loop = false;
        } else if is("--frame-and") {
            o.do_frame_and = true;
            o.min_set = true;
            o.do_frame_loop = false;
        } else if is("--dilate") {
            need(r, 1)?;
            if let Some(v) = scan_d(g, &rest[0]) {
                o.n_dilate3d = v;
            }
            used = 1;
        } else if is("--erode-face") || is("--erode-edge") || is("--erode-corner") || is("--erode")
        {
            need(r, 1)?;
            if let Some(v) = scan_d(g, &rest[0]) {
                o.n_erode_nn = v;
            }
            o.nn_type = if is("--erode-face") {
                NEAREST_NEIGHBOR_FACE
            } else if is("--erode-edge") {
                NEAREST_NEIGHBOR_EDGE
            } else {
                NEAREST_NEIGHBOR_CORNER
            };
            used = 1;
        } else if is("--erode2d") {
            need(r, 1)?;
            if let Some(v) = scan_d(g, &rest[0]) {
                o.n_erode2d = v;
            }
            used = 1;
        } else if is("--count") {
            need(r, 1)?;
            o.count_file = Some(rest[0].clone());
            o.do_count = true;
            used = 1;
        } else if is("--dilate-vertex-sum") || is("--dilate-vertex") {
            return Err(dilate_vertex(g, sg, r, rest, is("--dilate-vertex"))?);
        } else {
            let _ = writeln!(r.err, "ERROR: Option {option} unknown");
            if cmd_single_dash(option) {
                let _ = writeln!(r.err, "       Did you really mean -{option} ?");
            }
            return Err(Fatal::Exit(-1));
        }
        note_room(g, o.match_values.len(), "match values");
        k += used;
    }
    Ok(())
}

/// `--ohe seg ohe segno1 ...`: the segmentation one-hot encoded (`MRIoneHotEncode`, its every
/// argument after the two names taken as a number by `atoi`) and written; the program's end.
fn one_hot(g: &mut Globals, r: &mut Run, rest: &[String]) -> Fatal {
    let go = |g: &mut Globals, r: &mut Run| -> Result<Fatal, Fatal> {
        let Some(seg) = mri_read_volume(g, r.out, r.err, r.env, r.dir, &rest[0])? else {
            return Ok(Fatal::Exit(1));
        };
        let ids: Vec<i32> = rest[2..].iter().map(|a| atoi(a.as_bytes())).collect();
        let Some(mut ohe) = mri_one_hot_encode(g, r.out, r.err, &seg, &ids, 1)? else {
            return Ok(Fatal::Exit(1));
        };
        let e = mri_write(
            g,
            r.out,
            r.err,
            r.env,
            r.dir,
            &mut ohe,
            &rest[1],
            None,
            MGZ_INTENT_UNKNOWN,
        )?;
        Ok(Fatal::Exit(e))
    };
    match go(g, r) {
        Ok(f) | Err(f) => f,
    }
}

/// `--crop-around-ras cropout invol lta|nolta rCent aCent sCent voxFoVR voxFoVA voxFoVS
/// <ikeep...>` (or `cras <volume> <ignored>` for the centre): `MRIcropAroundRAS` written; the
/// program's end.
fn crop_around_ras(g: &mut Globals, r: &mut Run, rest: &[String]) -> Result<Fatal, Fatal> {
    const OPT: &str = "--crop-around-ras";
    let Some(mut invol) = mri_read_volume(g, r.out, r.err, r.env, r.dir, &rest[1])? else {
        return Ok(Fatal::Exit(1));
    };
    let lta = if rest[2] != "nolta" {
        let mut io = GcaLtaIo { env: r.env };
        let Some(lta) = lta_read(g, r.out, r.err, r.dir, &rest[2], &mut io)? else {
            return Ok(Fatal::Exit(1));
        };
        Some(lta)
    } else {
        None
    };
    let mut ras = [0.0f64; 3];
    if rest[3] == "cras" {
        let _ = writeln!(r.out, "Getting cras from {}", rest[4]);
        let Some(m) = mri_read_volume(g, r.out, r.err, r.env, r.dir, &rest[4])? else {
            return Ok(Fatal::Exit(1));
        };
        ras = [
            f64::from(m.vg.c_r),
            f64::from(m.vg.c_a),
            f64::from(m.vg.c_s),
        ];
    } else {
        for (k, x) in ras.iter_mut().enumerate() {
            let v = scan_lf(g, &rest[3 + k]);
            *x = number(g, r, OPT, &rest[3 + k], v)?;
        }
    }
    let mut fov = [0i32; 3];
    for (k, x) in fov.iter_mut().enumerate() {
        let v = scan_d(g, &rest[6 + k]);
        *x = number(g, r, OPT, &rest[6 + k], v)?;
    }
    let mut keep = Vec::new();
    let mut nth = 9;
    while cmd_nth_is_arg(rest, nth) {
        let v = scan_d(g, &rest[nth]);
        keep.push(number(g, r, OPT, &rest[nth], v)?);
        nth += 1;
    }
    let crop = mri_crop_around_ras(
        g,
        r.out,
        r.err,
        r.env,
        &mut invol,
        ras,
        fov,
        lta.as_ref(),
        &keep,
    )?;
    let Some(mut crop) = crop else {
        return Ok(Fatal::Exit(1));
    };
    let e = mri_write(
        g,
        r.out,
        r.err,
        r.env,
        r.dir,
        &mut crop,
        &rest[0],
        None,
        MGZ_INTENT_UNKNOWN,
    )?;
    Ok(Fatal::Exit(e))
}

/// `--dilate-vertex-sum vno surf measure|nofile|area target outmask` and `--dilate-vertex vno surf
/// radius outmask` (target `pi * radius * radius`): `MRISdilateVertexToSum`'s mask written; the
/// program's end. Too few arguments: the usage line, status 1 (FS-1024: FreeSurfer's for
/// `--dilate-vertex` names `--dilate-vertex-sum`).
fn dilate_vertex(
    g: &mut Globals,
    sg: &mut SurfGlobals,
    r: &mut Run,
    rest: &[String],
    radius: bool,
) -> Result<Fatal, Fatal> {
    let opt = if radius {
        "--dilate-vertex"
    } else {
        "--dilate-vertex-sum"
    };
    if !radius && rest.len() < 5 {
        let _ = writeln!(
            r.out,
            "USAGE: --dilate-vertex-sum vno surf measure target outmask"
        );
        return Ok(Fatal::Exit(1));
    }
    if radius && rest.len() < 4 {
        g.notices.add(
            &FS_1024,
            "mri_binarize --dilate-vertex: its own usage line (FreeSurfer prints --dilate-vertex-sum's name)",
        );
        let name = if g.notices.reproduce() {
            "--dilate-vertex-sum"
        } else {
            "--dilate-vertex"
        };
        let _ = writeln!(r.out, "USAGE: {name} vno surf radius outmask");
        return Ok(Fatal::Exit(1));
    }
    let v = scan_d(g, &rest[0]);
    let vno = number(g, r, opt, &rest[0], v)?;
    let surf = {
        let mut io = Io {
            out: &mut *r.out,
            err: &mut *r.err,
            env: r.env,
        };
        readwrite::read(g, sg, &mut io, r.dir, &rest[1], true)?
    };
    let Some(mut surf) = surf else {
        return Ok(Fatal::Exit(1));
    };
    let mut measure = None;
    if !radius && rest[2] != "nofile" && rest[2] != "area" {
        let Some(m) = mri_read_volume(g, r.out, r.err, r.env, r.dir, &rest[2])? else {
            return Ok(Fatal::Exit(1));
        };
        measure = Some(m);
    }
    let target = if radius {
        let v = scan_lf(g, &rest[2]);
        let rad = number(g, r, opt, &rest[2], v)?;
        larmorx_freesurfer_core::sse::mul64(
            larmorx_freesurfer_core::sse::mul64(std::f64::consts::PI, rad),
            rad,
        )
    } else {
        let v = scan_lf(g, &rest[3]);
        number(g, r, opt, &rest[3], v)?
    };
    let tmp = {
        let mut io = Io {
            out: &mut *r.out,
            err: &mut *r.err,
            env: r.env,
        };
        dilate_vertex_to_sum(
            g,
            sg,
            &mut io,
            vno,
            Some(&mut surf),
            measure.as_ref(),
            target,
        )?
    };
    let Some(mut tmp) = tmp else {
        return Ok(Fatal::Exit(1));
    };
    let name = if radius { &rest[3] } else { &rest[4] };
    let e = mri_write(
        g,
        r.out,
        r.err,
        r.env,
        r.dir,
        &mut tmp,
        name,
        None,
        MGZ_INTENT_UNKNOWN,
    )?;
    Ok(Fatal::Exit(e))
}

/// `check_options()`: the input, an output, a threshold or match values (replacements alone make
/// it replace only), not both, `--rmin` not with `--min` (nor `--rmax` with `--max`), and the
/// maximum not below the minimum (`ERROR: ...` on standard output, status 1).
pub fn check_options(r: &mut Run, o: &mut Options) -> Result<(), Fatal> {
    let fail = |r: &mut Run, m: &str| -> Result<(), Fatal> {
        let _ = writeln!(r.out, "{m}");
        Err(Fatal::Exit(1))
    };
    if o.in_vol.is_none() {
        return fail(r, "ERROR: must specify input volume");
    }
    if o.out_vol.is_none() && o.count_file.is_none() && o.surf_file.is_none() {
        return fail(r, "ERROR: must specify output volume or output count file");
    }
    if !o.min_set && !o.max_set && !o.rmin_set && !o.rmax_set && !o.do_match && !o.do_fdr {
        if !o.src_replace.is_empty() || !o.src_replace_nn.is_empty() {
            o.replace_only = true;
        } else {
            return fail(
                r,
                "ERROR: must specify minimum and/or maximum threshold or match values",
            );
        }
    }
    if (o.min_set || o.max_set || o.rmin_set || o.rmax_set) && o.do_match {
        return fail(r, "ERROR: cannot specify threshold and match values");
    }
    if o.min_set && o.rmin_set {
        return fail(r, "ERROR: cannot --rmin and --min ");
    }
    if o.max_set && o.rmax_set {
        return fail(r, "ERROR: cannot --rmax and --max ");
    }
    if !o.do_match && o.max_set && o.min_set && o.max_thresh < o.min_thresh {
        let m = format!(
            "ERROR: max thresh = {} < min thresh = {}",
            cfmt::g(o.max_thresh, 6),
            cfmt::g(o.min_thresh, 6)
        );
        return fail(r, &m);
    }
    Ok(())
}

/// What `dump_options` prints of the run (`uname`, `getcwd`, the command line, the user).
pub struct RunInfo {
    pub cwd: String,
    pub cmdline: String,
    pub sysname: String,
    pub hostname: String,
    pub machine: String,
    pub user: String,
}

/// `printf("%s", s)` of a name that may be `NULL` (glibc prints `(null)`).
fn or_null(s: &Option<String>) -> &str {
    s.as_deref().unwrap_or("(null)")
}

/// `dump_options(stdout)`.
pub fn dump_options(fp: &mut dyn Write, o: &Options, info: &RunInfo) {
    let g6 = |v: f64| cfmt::g(v, 6);
    let _ = writeln!(fp);
    let _ = writeln!(fp, "{}", get_version());
    let _ = writeln!(fp, "cwd {}", info.cwd);
    let _ = writeln!(fp, "cmdline {}", info.cmdline);
    let _ = writeln!(fp, "sysname  {}", info.sysname);
    let _ = writeln!(fp, "hostname {}", info.hostname);
    let _ = writeln!(fp, "machine  {}", info.machine);
    let _ = writeln!(fp, "user     {}", info.user);
    let _ = writeln!(fp);
    let _ = writeln!(fp, "input      {}", or_null(&o.in_vol));
    let _ = writeln!(fp, "frame      {}", o.frame);
    let _ = writeln!(fp, "nErode3d   {}", o.n_erode3d);
    let _ = writeln!(fp, "nErode2d   {}", o.n_erode2d);
    let _ = writeln!(fp, "output     {}", or_null(&o.out_vol));
    if !o.do_match {
        let _ = writeln!(fp, "Binarizing based on threshold");
        if o.min_set {
            let _ = writeln!(fp, "min        {}", g6(o.min_thresh));
        } else if o.rmin_set {
            let _ = writeln!(fp, "rmin       {}", g6(o.rmin));
        } else {
            let _ = writeln!(fp, "min        -infinity");
        }
        if o.max_set {
            let _ = writeln!(fp, "max        {}", g6(o.max_thresh));
        } else if o.rmax_set {
            let _ = writeln!(fp, "rmax       {}", g6(o.rmax));
        } else {
            let _ = writeln!(fp, "max        +infinity");
        }
    } else {
        let _ = writeln!(fp, "Binarizing based on matching values");
        let _ = writeln!(fp, "nMatch {}", o.match_values.len() as i32);
        for (n, v) in o.match_values.iter().enumerate() {
            let _ = writeln!(fp, "{n:2}  {v:4}");
        }
    }
    let _ = writeln!(fp, "binval        {}", o.bin_val);
    let _ = writeln!(fp, "binvalnot     {}", o.bin_val_not);
    if let Some(m) = &o.merge_vol {
        let _ = writeln!(fp, "merge      {m}");
    }
    if let Some(m) = &o.mask_vol {
        let _ = writeln!(fp, "mask       {m}");
        let _ = writeln!(fp, "maskthresh {}", cfmt::f(o.mask_thresh, 6));
    }
}
