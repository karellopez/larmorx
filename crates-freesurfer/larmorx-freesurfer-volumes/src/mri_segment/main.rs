// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_segment`'s program (`mri_segment/mri_segment.cpp` 117-703 and 1573): the entry point
//! [`run`] (registered as `mri_segment` in the crate's `PROGRAMS`), [`main`] and the options
//! ([`get_option`]).
//!
//! `mri_segment [options] <invol> <outvol>` (recon-all: `mri_segment -wsizemm 13 -mprage
//! antsdn.brain.mgz wm.seg.mgz`). The version options first (only those: status 0); the options
//! while the next argument starts with `-` (names after the dash compared without case except
//! `-polvwsize`, `-min-wm-mask`, `-polvlen`, `-wsizemm`, `-diagno`/`-diag_no`, `-diag-write`,
//! `-diag-verbose`; then by the first letter: `b` the blur sigma, `n` the border classification's
//! passes, `t` the thin strands' thickness, `v` verbose, `p` a threshold, `x` an integer, `w` the
//! window, `?`/`h`/`u` the help (status 0); anything else `unknown option <arg>` on standard
//! error and the help, status 1); then exactly the input and the output (`fs::fatal` otherwise).
//!
//! The input read (none: `ERROR_NOFILE`), the command line recorded, changed to bytes when it is
//! not (`MRIchangeType(..., 0, 1000, 1)`); `-wsizemm` sets the window from the mean voxel size.
//! Without `-seg`: the trinary intensity segmentation ([`intensity_segmentation`]), the local
//! histograms (`MRIhistoSegment`), the class statistics (`MRIcomputeClassStatistics`, which set
//! `wm_low` and `gray_hi` unless given; `segment.dat`), both again; with `-seg`: the aseg's
//! non-parametric classification. Then the ambiguous voxels by the planes of least variance
//! (`MRIcpolvMedianCurveSegment`), the Gaussian border classification (`MRIreclassify`, `-n`
//! times), the labels' mask, the bright white matter recovered, the voxels of the wrong direction
//! removed, the one-dimensional strands removed and the thin strands thickened (by default), the
//! bright non-white-matter voxels masked out (`MRIfindBrightNonWM`), the diagonal connections
//! repaired, the basal ganglia and ventricles filled (`-fillbg`, `-fillv`), the edits kept
//! (`-keep`), the output written; status 0.
//!
//! Defects (`larmorx_core::defects`; the mechanisms in `docs/findings/freesurfer-mri-segment.md`):
//! FS-1216 to FS-1218 (the planes' tables), FS-1219 (an option without its values), FS-1220
//! (`-b`), FS-1221 to FS-1225 (`-seg`, `-min-wm-mask`), FS-1226 (an unwritten output), FS-1227
//! and FS-1228 (`-fillv`, `-fillbg`), FS-1229 and FS-1230 (kept), FS-1231 (`-seg`'s statistics),
//! FS-1232 (`-wm_low_factor`, kept).

use std::io::Write;

use larmorx_core::defects::{
    FS_1216, FS_1217, FS_1218, FS_1219, FS_1220, FS_1221, FS_1222, FS_1223, FS_1224, FS_1226,
    FS_1229, FS_1230, FS_1231, FS_1232,
};
use larmorx_freesurfer_core::cstd::{atof, atoi, fmt_f, format_double};
use larmorx_freesurfer_core::diag::{DIAG_VERBOSE, DIAG_WRITE, diag_init};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::help_xml::output_help_xml;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, ERROR_NOFILE, Fatal};
use larmorx_freesurfer_core::run::{Environment, Settings};
use larmorx_freesurfer_core::sse;
use larmorx_freesurfer_core::system;
use larmorx_freesurfer_core::timer::Timer;
use larmorx_freesurfer_core::utils::{resolve, stricmp};
use larmorx_freesurfer_core::version::handle_version_option;
use larmorx_freesurfer_filter::classify::{mri_compute_class_statistics, mri_reclassify};
use larmorx_freesurfer_filter::nonpar::{
    mri_classify_ambiguous_non_par, mri_compute_class_statistics_seg, mri_reclassify_wm_ctx_non_par,
};
use larmorx_freesurfer_filter::polv::{
    Ctx, MAX_MEDIANS, MAXLEN, cpolv_median_curve_segment, intensity_segmentation, mask_labels,
    mask_labels_in_place, thicken_thin_wm_strands_in_place,
};
use larmorx_freesurfer_io::mriio::{mri_add_command_line, mri_read_volume, mri_write};
use larmorx_freesurfer_io::tags::tag_make_command_line_string;
use larmorx_freesurfer_mri::copy::mri_copy;
use larmorx_freesurfer_mri::mri::{MGZ_INTENT_UNKNOWN, MRI_UCHAR, Mri};
use larmorx_freesurfer_mri::voxel::mri_set_vox_val;
use larmorx_freesurfer_volume::arith::{Pair, mri_binarize, mri_clone};
use larmorx_freesurfer_volume::labels::WM_MIN_VAL;
use larmorx_freesurfer_volume::mri2ops::mri_find_bright_non_wm;
use larmorx_freesurfer_volume::mrihisto::mri_histo_segment;
use larmorx_freesurfer_volume::mriset::mri_copy_label;
use larmorx_freesurfer_volume::resize::mri_change_type;

use super::local::{
    fill_basal_ganglia, fill_ventricles, filter_morphology, recover_bright_white,
    remove_1d_structures, remove_wrong_direction,
};
use crate::mri_mask::main::Cout;
use crate::mri_segstats::options::{scan_d, scan_f, scan_lf};

/// The program's name as `argv[0]` gives it (recon-all runs it by name).
pub const NAME: &str = "mri_segment";

/// `mri_segment.help.xml`, the text `usage_exit` renders (`outputHelpXml`).
pub const HELP_XML: &[u8] = include_bytes!("mri_segment.help.xml");

/// The status of a process ended by `SIGSEGV`, as a shell reports it.
const SEGFAULT: i32 = 139;
/// `CMD_LINE_LEN` (`mri_segment.cpp`'s command-line buffer).
const CMD_LINE_LEN: usize = 4096;
/// `WHITE_MATTER_MEAN` (`WM_MEAN`, `include/mrinorm.h`).
const WHITE_MATTER_MEAN: i32 = 110;
/// `MRI_UNKNOWN`, `MRI_MGH_MPRAGE`, `MRI_WASHU_MPRAGE` (`include/mri.h`).
const MRI_UNKNOWN: i32 = 0;
const MRI_MGH_MPRAGE: i32 = 1;
const MRI_WASHU_MPRAGE: i32 = 3;
/// `WM_EDITED_ON_VAL`, `WM_EDITED_OFF_VAL` (`include/mri.h`).
const WM_EDITED_ON_VAL: i32 = 255;
const WM_EDITED_OFF_VAL: i32 = 1;

/// The settings the options give (the file-scope variables of `mri_segment.cpp`).
#[derive(Debug)]
pub struct Options {
    pub extract: i32,
    pub verbose: bool,
    pub wsize: i32,
    pub wsizemm: f64,
    pub pct: f32,
    pub pslope: f32,
    pub nslope: f32,
    pub wm_low: f32,
    pub wm_low_factor: f32,
    pub wm_hi: f32,
    pub gray_hi: f32,
    pub gray_low: f32,
    pub gray_low_set: bool,
    pub niter: i32,
    pub gray_hi_set: bool,
    pub wm_hi_set: bool,
    pub wm_low_set: bool,
    pub scan_type: i32,
    pub thickness: i32,
    pub thicken: i32,
    pub nsegments: i32,
    pub fill_bg: bool,
    pub fill_ventricles: bool,
    pub keep_edits: bool,
    pub auto_detect_stats: bool,
    pub no1d_remove: bool,
    pub log_stats: bool,
    /// `segmentdatfile` (`None`: `-dat` given last, FreeSurfer's `NULL`).
    pub segmentdatfile: Option<String>,
    pub blur_sigma: f32,
    /// `MinWMMask` (`-min-wm-mask`: read, every voxel of frame 0 set to 1).
    pub min_wm_mask: Option<Mri>,
    /// `-min-wm-mask`'s file name.
    pub min_wm_mask_name: String,
    /// `segfilename` (`-seg`; `Some(None)`: given without its values, FreeSurfer's `NULL`).
    pub segfilename: Option<Option<String>>,
    pub qwm: f64,
    pub qctx: f64,
    pub ndil_wm: i32,
    pub ndil_ctx: i32,
    pub polvwsize: i32,
    pub polvlen: f32,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            extract: 0,
            verbose: false,
            wsize: 11,
            wsizemm: 0.0,
            pct: 0.8,
            pslope: 1.0,
            nslope: 1.0,
            wm_low: 90.0,
            wm_low_factor: 0.0,
            wm_hi: 125.0,
            gray_hi: 100.0,
            gray_low: 30.0,
            gray_low_set: false,
            niter: 1,
            gray_hi_set: false,
            wm_hi_set: false,
            wm_low_set: false,
            scan_type: MRI_UNKNOWN,
            thickness: 4,
            thicken: 1,
            nsegments: 20,
            fill_bg: false,
            fill_ventricles: false,
            keep_edits: false,
            auto_detect_stats: true,
            no1d_remove: false,
            log_stats: true,
            segmentdatfile: Some("segment.dat".to_owned()),
            blur_sigma: 0.25,
            min_wm_mask: None,
            min_wm_mask_name: String::new(),
            segfilename: None,
            qwm: 3.0,
            qctx: 3.0,
            ndil_wm: 2,
            ndil_ctx: 2,
            polvwsize: 5,
            polvlen: 3.0,
        }
    }
}

/// The program's streams, environment and working directory. Standard output is kept until the
/// end (FreeSurfer's buffer: a crash in `reproduce` loses what it would).
pub struct Run<'a, 'b> {
    pub out: &'a mut Cout,
    pub err: &'a mut dyn Write,
    pub env: &'b Environment,
    pub dir: Option<&'b std::path::Path>,
}

impl<'b> Run<'_, 'b> {
    fn ctx(&self) -> Ctx<'b> {
        Ctx {
            env: self.env,
            dir: self.dir,
        }
    }
}

/// `mri_segment`'s entry point: its original arguments (without the program's name), the
/// settings, the two streams; the exit status. The notices of the run follow on standard error.
pub fn run(args: &[String], settings: &Settings, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let mut g = Globals::new(settings.mode);
    g.stderr.read_write = system::stderr_read_write();
    let mut cout = Cout::new(&g);
    let status = {
        let mut terr = log::TrackedErr::new(&g, &mut *err);
        let mut r = Run {
            out: &mut cout,
            err: &mut terr,
            env: &settings.env,
            dir: settings.dir.as_deref(),
        };
        match main(&mut g, &mut r, args) {
            Ok(()) => 0,
            Err(f) => f.status(),
        }
    };
    let keep = if i32::from(status) == SEGFAULT && g.notices.reproduce() {
        cout.kept_by_crash()
    } else {
        cout.buf.len()
    };
    let _ = out.write_all(&cout.buf[..keep]);
    let _ = out.flush();
    g.notices.print(err);
    status
}

/// `usage_exit(code)`: the help (`outputHelpXml`), then `exit(code)`.
fn usage_exit(g: &mut Globals, r: &mut Run, code: i32) -> Fatal {
    output_help_xml(g, r.out, r.err, Some(HELP_XML));
    Fatal::Exit(code)
}

/// FS-1219: `n` values of option `arg` read where `rest` (the option and what follows) has fewer.
fn need(g: &mut Globals, r: &mut Run, rest: &[String], n: usize) -> Result<(), Fatal> {
    if rest.len() > n {
        return Ok(());
    }
    let arg = &rest[0];
    g.notices.add(
        &FS_1219,
        format!(
            "mri_segment: option '{arg}' without its {n} value(s): FreeSurfer reads past its arguments and crashes"
        ),
    );
    if g.notices.reproduce() {
        return Err(Fatal::Exit(SEGFAULT));
    }
    let _ = writeln!(r.err, "{NAME}: option {arg} needs {n} value(s)");
    Err(usage_exit(g, r, 1))
}

fn f1(x: f32) -> String {
    fmt_f(f64::from(x), 2, 1)
}

fn f2(x: f32) -> String {
    fmt_f(f64::from(x), 2, 2)
}

fn pg(x: f64) -> String {
    format_double("%g", x).unwrap_or_default()
}

/// The positions along the normal of `-polvlen len` (FreeSurfer's loop from `-len` to `len` in
/// steps of `0.25f`), counted up to one more than its tables hold.
fn polv_positions(len: f32) -> usize {
    let mut n = 0usize;
    let mut dist = -len;
    while dist <= len && n <= MAX_MEDIANS {
        n += 1;
        dist += 0.25;
    }
    n
}

/// The default's refusal of `-polvwsize` and `-polvlen` settings FreeSurfer cannot hold
/// (FS-1216 to FS-1218; with `reproduce` the planes' functions meet them where FreeSurfer does).
fn check_polv(g: &mut Globals, r: &mut Run, o: &Options) -> Result<(), Fatal> {
    if g.notices.reproduce() {
        return Ok(());
    }
    let w = o.polvwsize;
    if w.unsigned_abs() > 16 {
        g.notices.add(
            &FS_1218,
            format!(
                "mri_segment: -polvwsize {w} is refused (more than {MAXLEN} values in a plane)"
            ),
        );
        log::error_exit(
            g,
            r.err,
            ERROR_BADPARM,
            &format!(
                "{NAME}: -polvwsize {w}: a plane's window holds at most {MAXLEN} values (at most 15, odd)"
            ),
        )?;
    }
    if w < 0 || (w > 0 && w % 2 == 0) {
        g.notices.add(
            &FS_1217,
            format!(
                "mri_segment: -polvwsize {w} is refused (FreeSurfer sorts values it never sampled)"
            ),
        );
        log::error_exit(
            g,
            r.err,
            ERROR_BADPARM,
            &format!("{NAME}: -polvwsize {w}: the plane's window must be 0 or odd, from 1 to 15"),
        )?;
    }
    if polv_positions(o.polvlen) > MAX_MEDIANS {
        let len = pg(f64::from(o.polvlen));
        g.notices.add(
            &FS_1216,
            format!(
                "mri_segment: -polvlen {len} is refused (more than {MAX_MEDIANS} positions along the normal)"
            ),
        );
        log::error_exit(
            g,
            r.err,
            ERROR_BADPARM,
            &format!(
                "{NAME}: -polvlen {len}: more than {MAX_MEDIANS} positions along the normal (at most 24.75)"
            ),
        )?;
    }
    Ok(())
}

/// `MRIread(name)`: `None` after the reader's messages.
fn read(g: &mut Globals, r: &mut Run, fname: &str) -> Result<Option<Mri>, Fatal> {
    mri_read_volume(g, r.out, r.err, r.env, r.dir, fname)
}

/// `get_option(argc, argv)` on `rest` (`argv[1]` and what follows): the arguments it takes after
/// its own.
pub fn get_option(
    g: &mut Globals,
    r: &mut Run,
    o: &mut Options,
    rest: &[String],
) -> Result<usize, Fatal> {
    let arg = rest[0].clone();
    let option = &arg[1..];
    let is = |g: &mut Globals, n: &str| stricmp(g, option, n) == 0;
    let value = |i: usize| rest[i].as_bytes();
    if option.eq_ignore_ascii_case("-version") {
        g.notices.add(
            &FS_1229,
            "mri_segment: its own version option prints FreeSurfer's placeholder 'Version: ##version##': kept",
        );
        let _ = r.err.write_all(b"Version: ##version## \n");
        return Err(Fatal::Exit(0));
    }
    if is(g, "-help") || is(g, "-usage") {
        return Err(usage_exit(g, r, 0));
    }
    if is(g, "MGH_MPRAGE") || is(g, "MPRAGE") {
        o.scan_type = MRI_MGH_MPRAGE;
        if !o.gray_hi_set {
            o.gray_hi = 99.0;
        }
        if !o.wm_low_set {
            o.wm_low = 89.0;
        }
        return Ok(0);
    }
    if is(g, "WASHU_MPRAGE") {
        o.scan_type = MRI_WASHU_MPRAGE;
        let _ = writeln!(r.out, "assuming input volume is WashU MP-RAGE (dark GM)");
        o.gray_hi = 85.0;
        o.wm_low = 80.0;
        return Ok(0);
    }
    if is(g, "no1d_remove") {
        let _ = writeln!(r.err, "disabling removal of 1D strands");
        o.no1d_remove = true;
        return Ok(0);
    }
    if is(g, "slope") {
        need(g, r, rest, 1)?;
        o.pslope = atof(value(1)) as f32;
        o.nslope = o.pslope;
        let _ = writeln!(r.err, "using curvature slope = {}", f2(o.pslope));
        return Ok(1);
    }
    if is(g, "pslope") {
        need(g, r, rest, 1)?;
        o.pslope = atof(value(1)) as f32;
        let _ = writeln!(r.err, "using curvature pslope = {}", f2(o.pslope));
        return Ok(1);
    }
    if is(g, "debug_voxel") {
        need(g, r, rest, 3)?;
        g.gx = atoi(value(1));
        g.gy = atoi(value(2));
        g.gz = atoi(value(3));
        let _ = writeln!(r.err, "debugging voxel ({}, {}, {})", g.gx, g.gy, g.gz);
        return Ok(3);
    }
    if is(g, "auto") {
        o.auto_detect_stats = !o.auto_detect_stats;
        let _ = writeln!(
            r.err,
            "{}automatically detecting class statistics...",
            if o.auto_detect_stats { "" } else { "not " }
        );
        return Ok(0);
    }
    if is(g, "noauto") {
        o.auto_detect_stats = false;
        let _ = writeln!(r.err, "not automatically detecting class statistics...");
        return Ok(0);
    }
    if is(g, "log") {
        o.log_stats = true;
        let _ = writeln!(r.err, "logging class statistics and thresholds...");
        return Ok(0);
    }
    if is(g, "keep") {
        o.keep_edits = true;
        let _ = writeln!(r.err, "preserving editing changes in output volume...");
        return Ok(0);
    }
    if is(g, "nslope") {
        need(g, r, rest, 1)?;
        o.nslope = atof(value(1)) as f32;
        let _ = writeln!(r.err, "using curvature nslope = {}", f2(o.nslope));
        return Ok(1);
    }
    if is(g, "ghi") || is(g, "gray_hi") {
        need(g, r, rest, 1)?;
        o.gray_hi = atof(value(1)) as f32;
        o.gray_hi_set = true;
        let _ = writeln!(r.err, "using gray hilim = {}", f1(o.gray_hi));
        return Ok(1);
    }
    if is(g, "glo") || is(g, "gray_low") {
        need(g, r, rest, 1)?;
        o.gray_low = atof(value(1)) as f32;
        o.gray_low_set = true;
        let _ = writeln!(r.err, "using gray low limit = {}", f1(o.gray_low));
        return Ok(1);
    }
    if is(g, "wlo") || is(g, "wm_low") {
        need(g, r, rest, 1)?;
        o.wm_low = atof(value(1)) as f32;
        o.wm_low_set = true;
        let _ = writeln!(r.err, "using white lolim = {}", f1(o.wm_low));
        return Ok(1);
    }
    if is(g, "wm_low_factor") {
        need(g, r, rest, 1)?;
        o.wm_low_factor = atof(value(1)) as f32;
        let _ = writeln!(r.out, "wm_low_factor set to {}", f1(o.wm_low_factor));
        return Ok(1);
    }
    if is(g, "whi") || is(g, "wm_hi") {
        need(g, r, rest, 1)?;
        o.wm_hi = atof(value(1)) as f32;
        o.wm_hi_set = true;
        let _ = writeln!(r.err, "using white hilim = {}", f1(o.wm_hi));
        return Ok(1);
    }
    if is(g, "nseg") {
        need(g, r, rest, 1)?;
        o.nsegments = atoi(value(1));
        let _ = writeln!(r.err, "thickening the {} largest thin strands", o.nsegments);
        return Ok(1);
    }
    if is(g, "thicken") {
        need(g, r, rest, 1)?;
        o.thicken = atoi(value(1));
        let _ = writeln!(
            r.err,
            "{}thickening thin strands",
            if o.thicken != 0 { "" } else { "not " }
        );
        return Ok(1);
    }
    if is(g, "thickenonly") {
        o.thicken = 2;
        let _ = writeln!(r.err, "thickening thin strands");
        return Ok(0);
    }
    if is(g, "fillbg") || is(g, "fill_bg") {
        o.fill_bg = !o.fill_bg;
        let _ = writeln!(
            r.err,
            "{}filling basal ganglia",
            if o.fill_bg { "" } else { "not " }
        );
        return Ok(0);
    }
    if is(g, "fillv") || is(g, "fill_ventricles") {
        o.fill_ventricles = !o.fill_ventricles;
        let _ = writeln!(
            r.err,
            "{}filling ventricles",
            if o.fill_ventricles { "" } else { "not " }
        );
        return Ok(0);
    }
    if is(g, "dat") {
        // `segmentdatfile = argv[2]`: NULL when it is the last argument (no crash: the positional
        // arguments are then missing)
        o.segmentdatfile = rest.get(1).cloned();
        return Ok(1);
    }
    if is(g, "seg") {
        return seg_option(g, r, o, rest);
    }
    if option == "polvwsize" {
        need(g, r, rest, 1)?;
        if let Some(v) = scan_d(g, &rest[1]) {
            o.polvwsize = v;
        }
        return Ok(1);
    }
    if option == "min-wm-mask" {
        need(g, r, rest, 1)?;
        let Some(mut mask) = read(g, r, &rest[1])? else {
            return Err(Fatal::Exit(1));
        };
        for c in 0..mask.vg.width {
            for rr in 0..mask.vg.height {
                for s in 0..mask.vg.depth {
                    // FreeSurfer sets 1 whatever the label (both branches)
                    mri_set_vox_val(g, r.err, &mut mask, c, rr, s, 0, 1.0, 0)?;
                }
            }
        }
        o.min_wm_mask = Some(mask);
        o.min_wm_mask_name = rest[1].clone();
        return Ok(1);
    }
    if option == "polvlen" {
        need(g, r, rest, 1)?;
        if let Some(v) = scan_f(g, &rest[1]) {
            o.polvlen = v;
        }
        return Ok(1);
    }
    if option == "wsizemm" {
        need(g, r, rest, 1)?;
        if let Some(v) = scan_lf(g, &rest[1]) {
            o.wsizemm = v;
        }
        return Ok(1);
    }
    if option == "diagno" || option == "diag_no" {
        need(g, r, rest, 1)?;
        g.gdiag_no = atoi(value(1));
        return Ok(1);
    }
    if option == "diag-write" {
        g.gdiag |= DIAG_WRITE;
        return Ok(0);
    }
    if option == "diag-verbose" {
        g.gdiag |= DIAG_VERBOSE;
        return Ok(0);
    }
    let letter = option.as_bytes().first().map(u8::to_ascii_uppercase);
    match letter {
        Some(b'B') => {
            // FS-1220: FreeSurfer reads the option's own name (atof(argv[1]))
            let own = atof(arg.as_bytes()) as f32;
            match rest.get(1) {
                Some(v) => {
                    let value = atof(v.as_bytes()) as f32;
                    if value.to_bits() != own.to_bits() {
                        g.notices.add(
                            &FS_1220,
                            format!(
                                "mri_segment: {arg} {v}: FreeSurfer takes the blur sigma from the option's own name ({})",
                                pg(f64::from(own))
                            ),
                        );
                    }
                    o.blur_sigma = if g.notices.reproduce() { own } else { value };
                }
                None => o.blur_sigma = own,
            }
            Ok(1)
        }
        Some(b'N') => {
            need(g, r, rest, 1)?;
            o.niter = atoi(value(1));
            let _ = writeln!(r.err, "running border classification {} times", o.niter);
            Ok(1)
        }
        Some(b'T') => {
            need(g, r, rest, 1)?;
            o.thickness = atoi(value(1));
            let _ = writeln!(
                r.err,
                "finding signicant strands thinner than {} mm",
                o.thickness
            );
            Ok(1)
        }
        Some(b'V') => {
            o.verbose = !o.verbose;
            Ok(0)
        }
        Some(b'P') => {
            need(g, r, rest, 1)?;
            o.pct = atof(value(1)) as f32;
            let _ = writeln!(
                r.err,
                "using {}% threshold",
                fmt_f(f64::from(o.pct * 100.0), 2, 0)
            );
            Ok(1)
        }
        Some(b'X') => {
            need(g, r, rest, 1)?;
            match scan_d(g, &rest[1]) {
                Some(v) => o.extract = v,
                None => {
                    log::error_exit(
                        g,
                        r.err,
                        ERROR_BADPARM,
                        &format!("{NAME}: could not scan option from '{}'", rest[1]),
                    )?;
                }
            }
            Ok(1)
        }
        Some(b'W') => {
            need(g, r, rest, 1)?;
            o.wsize = atoi(value(1));
            let _ = writeln!(r.err, "using wsize = {}", o.wsize);
            Ok(1)
        }
        Some(b'?' | b'H' | b'U') => Err(usage_exit(g, r, 0)),
        _ => {
            let _ = writeln!(r.err, "unknown option {arg}");
            Err(usage_exit(g, r, 1))
        }
    }
}

/// `-seg <aseg> <Qwm> <Qctx> <NdilWM> <NdilCtx>` (`sscanf` of each: a value that does not read
/// leaves the setting). Given alone, FreeSurfer reads the four numbers from the strings after
/// its arguments (the environment's, which hold none) and prints the name as `(null)`; with some
/// of them missing it reads past its arguments (FS-1219).
fn seg_option(
    g: &mut Globals,
    r: &mut Run,
    o: &mut Options,
    rest: &[String],
) -> Result<usize, Fatal> {
    if rest.len() > 1 {
        need(g, r, rest, 5)?;
    }
    o.segfilename = Some(rest.get(1).cloned());
    if rest.len() > 5 {
        if let Some(v) = scan_lf(g, &rest[2]) {
            o.qwm = v;
        }
        if let Some(v) = scan_lf(g, &rest[3]) {
            o.qctx = v;
        }
        if let Some(v) = scan_d(g, &rest[4]) {
            o.ndil_wm = v;
        }
        if let Some(v) = scan_d(g, &rest[5]) {
            o.ndil_ctx = v;
        }
    }
    let name = rest.get(1).map_or("(null)", String::as_str);
    let _ = writeln!(r.out, "Using aseg {name}");
    let _ = writeln!(
        r.out,
        "  params {} {} {} {}",
        fmt_f(o.qwm, 0, 6),
        fmt_f(o.qctx, 0, 6),
        o.ndil_wm,
        o.ndil_ctx
    );
    Ok(5)
}

/// `MRIwrite(mri, fname)` of an output: FS-1226 when it is not written (FreeSurfer's status 0;
/// by default 1, at the end).
fn write_output(g: &mut Globals, r: &mut Run, mri: &mut Mri, fname: &str) -> Result<bool, Fatal> {
    let rc = mri_write(
        g,
        r.out,
        r.err,
        r.env,
        r.dir,
        mri,
        fname,
        None,
        MGZ_INTENT_UNKNOWN,
    )?;
    if rc == 0 {
        return Ok(true);
    }
    g.notices.add(
        &FS_1226,
        if g.notices.reproduce() {
            format!("mri_segment: '{fname}' was not written; status 0, as in FreeSurfer")
        } else {
            format!("mri_segment: '{fname}' was not written: status 1 (FreeSurfer ends with 0)")
        },
    );
    Ok(g.notices.reproduce())
}

/// `MRIwrite` of a diagnostic volume (`DIAG_WRITE` with `DIAG_VERBOSE`; its status not looked at).
fn diag_write(g: &mut Globals, r: &mut Run, mri: &mut Mri, fname: &str) -> Result<(), Fatal> {
    if g.gdiag & DIAG_WRITE != 0 && g.gdiag & DIAG_VERBOSE != 0 {
        mri_write(
            g,
            r.out,
            r.err,
            r.env,
            r.dir,
            mri,
            fname,
            None,
            MGZ_INTENT_UNKNOWN,
        )?;
    }
    Ok(())
}

/// `(int)` of a threshold `MRIhistoSegment` takes (FS-17 beyond the range).
fn to_int(g: &mut Globals, v: f32) -> i32 {
    larmorx_freesurfer_core::affine::float_to_int(g, f64::from(v), "mri_segment")
}

/// The class statistics (`white_mean`, `white_sigma`, `gray_mean`, `gray_sigma`).
#[derive(Clone, Copy, Debug, Default)]
struct Stats {
    white_mean: f32,
    white_sigma: f32,
    gray_mean: f32,
    gray_sigma: f32,
}

/// `main(argc, argv)` on the arguments after the program's name. `Ok` is status 0.
pub fn main(g: &mut Globals, r: &mut Run, args: &[String]) -> Result<(), Fatal> {
    let argv: Vec<String> = std::iter::once(NAME.to_owned())
        .chain(args.iter().cloned())
        .collect();
    let argv_bytes: Vec<&[u8]> = argv.iter().map(|a| a.as_bytes()).collect();
    let cmdline = tag_make_command_line_string(g, &argv_bytes, CMD_LINE_LEN);
    let (nargs, argv) = handle_version_option(g, r.env, r.out, &argv, NAME);
    if nargs != 0 && argv.len() == 1 {
        return Err(Fatal::Exit(0));
    }
    g.progname = NAME.to_owned();
    diag_init(g, r.env, r.out);
    let mut o = Options::default();
    let mut argc = argv.len() as isize;
    let mut at = 1usize;
    while argc > 1 && argv.get(at).is_some_and(|a| a.starts_with('-')) {
        let nargs = get_option(g, r, &mut o, &argv[at..])?;
        argc -= nargs as isize + 1;
        at += nargs + 1;
    }
    if argc < 3 {
        log::fatal(
            g,
            r.err,
            1,
            "Must provide input and output files. Use --help for more info.",
        )?;
    }
    if argc > 3 {
        log::fatal(
            g,
            r.err,
            1,
            "Too many arguments provided. Flagged options must be used before positional arguments. Use --help for more info.",
        )?;
    }
    check_polv(g, r, &o)?;
    let then = Timer::new();
    let input_file_name = argv[at].clone();
    let output_file_name = argv[at + 1].clone();

    let Some(mut src) = read(g, r, &input_file_name)? else {
        log::error_exit(
            g,
            r.err,
            ERROR_NOFILE,
            &format!("{NAME}: could not read source volume from {input_file_name}"),
        )?;
        unreachable!("ErrorExit ends the program")
    };
    mri_add_command_line(g, r.err, &mut src, &cmdline);
    if src.mtype != MRI_UCHAR {
        let _ = writeln!(r.out, "changing input type from {} to UCHAR", src.mtype);
        let Some(m) = mri_change_type(g, r.out, r.err, r.env, &src, MRI_UCHAR, 0.0, 1000.0, 1)?
        else {
            // FreeSurfer goes on with no volume and crashes; the flag 1 makes no robust range
            return Err(Fatal::Exit(SEGFAULT));
        };
        src = m;
    }

    if o.wsizemm > 0.0 {
        let voxres =
            (f64::from(src.vg.xsize) + f64::from(src.vg.ysize) + f64::from(src.vg.zsize)) / 3.0;
        let w = sse::div64(o.wsizemm, voxres).round();
        o.wsize = larmorx_freesurfer_core::affine::float_to_int(g, w, "mri_segment -wsizemm");
        let _ = writeln!(
            r.out,
            "wsizemm = {}, voxres = {}, wsize = {}",
            pg(o.wsizemm),
            pg(voxres),
            o.wsize
        );
    }

    if o.thicken > 1 {
        g.notices.add(
            &FS_1230,
            "mri_segment -thickenonly (-thicken above 1): the input written with its one-dimensional strands removed, not thickened: kept",
        );
        let mut dst = mri_copy(g, r.out, r.err, &src, None)?.unwrap_new();
        if !o.no1d_remove {
            let _ = writeln!(r.out, "removing 1-dimensional structures...");
            remove_1d_structures(g, r.out, r.err, &mut dst, 10000, 2, None)?;
        }
        if !write_output(g, r, &mut dst, &output_file_name)? {
            return Err(Fatal::Exit(1));
        }
        return Err(Fatal::Exit(0));
    }

    // FS-1223, FS-1224: -min-wm-mask
    if let Some(mask) = o.min_wm_mask.as_ref() {
        let same = mask.vg.width == src.vg.width
            && mask.vg.height == src.vg.height
            && mask.vg.depth == src.vg.depth;
        if !same {
            g.notices.add(
                &FS_1223,
                format!(
                    "mri_segment -min-wm-mask: the mask {} is {}x{}x{}, the input {}x{}x{}: refused",
                    o.min_wm_mask_name,
                    mask.vg.width,
                    mask.vg.height,
                    mask.vg.depth,
                    src.vg.width,
                    src.vg.height,
                    src.vg.depth
                ),
            );
            log::error_exit(
                g,
                r.err,
                ERROR_BADPARM,
                &format!(
                    "{NAME}: the -min-wm-mask volume {} does not have the input's dimensions",
                    o.min_wm_mask_name
                ),
            )?;
        }
        g.notices.add(
            &FS_1224,
            format!(
                "mri_segment -min-wm-mask {}: the mask does not change the segmentation (FreeSurfer overwrites it): kept",
                o.min_wm_mask_name
            ),
        );
        if !g.notices.reproduce() {
            o.min_wm_mask = None;
        }
    }

    let mut labels: Option<Mri> = None;
    let mut leaked: Option<Mri> = None;
    let mut st = Stats::default();
    let tmp: Mri;
    match o.segfilename.clone() {
        None => {
            labels = Some(mri_clone(g, r.out, r.err, &src, None)?.unwrap_new());
            if o.auto_detect_stats && !o.wm_low_set {
                let decrease = 10.0f32;
                let _ = writeln!(
                    r.out,
                    "Widening wm low from {} to {}",
                    pg(f64::from(o.wm_low)),
                    pg(f64::from(sse::sub(o.wm_low, decrease)))
                );
                let _ = r.out.flush();
                o.wm_low = sse::sub(o.wm_low, decrease);
            }
            if o.scan_type == MRI_MGH_MPRAGE {
                let _ = writeln!(
                    r.out,
                    "assuming input volume is MGH (Van der Kouwe) MP-RAGE"
                );
            }
            for (label, v) in [
                ("wm mean:  ", WHITE_MATTER_MEAN.to_string()),
                ("wsize:    ", o.wsize.to_string()),
                ("wm low:   ", pg(f64::from(o.wm_low))),
                ("wm hi:    ", pg(f64::from(o.wm_hi))),
                ("gray low: ", pg(f64::from(o.gray_low))),
                ("gray hi:  ", pg(f64::from(o.gray_hi))),
            ] {
                let _ = writeln!(r.out, "{label}{v}");
                let _ = r.out.flush();
            }
            let _ = writeln!(r.out, "Doing initial trinary intensity segmentation ");
            let mut t = first_labels(g, r, &o, &src)?;
            diag_write(g, r, &mut t, "wmseg.int.1.mgz")?;
            let _ = writeln!(r.out, "Using local statistics to label ambiguous voxels");
            let _ = r.out.flush();
            histo(g, r, &o, &src, &mut t)?;
            diag_write(g, r, &mut t, "wmseg.histo.1.mgz")?;
            if o.auto_detect_stats {
                let _ = writeln!(r.out, "Autodetecting stats");
                let _ = writeln!(r.out, "Computing class statistics for intensity windows...");
                let _ = r.out.flush();
                let cs = mri_compute_class_statistics(
                    g,
                    r.out,
                    r.err,
                    &src,
                    &t,
                    o.gray_low,
                    WHITE_MATTER_MEAN as f32,
                )?;
                st = Stats {
                    white_mean: cs.mean_wm,
                    white_sigma: cs.sigma_wm,
                    gray_mean: cs.mean_gm,
                    gray_sigma: cs.sigma_gm,
                };
                if !st.white_mean.is_finite()
                    || !st.white_sigma.is_finite()
                    || !st.gray_mean.is_finite()
                    || !st.gray_sigma.is_finite()
                {
                    log::error_exit(
                        g,
                        r.err,
                        ERROR_BADPARM,
                        &format!("{NAME}: class statistics not finite - check input volume!"),
                    )?;
                }
                let _ = writeln!(r.out, " white_mean {}", pg(f64::from(st.white_mean)));
                let _ = writeln!(r.out, " white_sigma {}", pg(f64::from(st.white_sigma)));
                let _ = writeln!(r.out, " gray_mean {}", pg(f64::from(st.gray_mean)));
                let _ = writeln!(r.out, " gray_sigma {}", pg(f64::from(st.gray_sigma)));
                let _ = r.out.flush();
                if !o.wm_low_set {
                    if st.gray_sigma.abs() < f32::EPSILON {
                        o.wm_low = sse::add(st.white_mean, st.gray_mean) / 2.0;
                    } else if o.wm_low_factor == 0.0 {
                        o.wm_low = sse::add(st.gray_mean, st.gray_sigma);
                    } else {
                        g.notices.add(
                            &FS_1232,
                            "mri_segment -wm_low_factor without -seg: wm_low = (1 - f) gray_mean + f white_mean, the help's formula swapped (-seg follows the help): kept",
                        );
                        let _ = writeln!(r.out, "using wm_low_factor = {}", f1(o.wm_low_factor));
                        let f = f64::from(o.wm_low_factor);
                        o.wm_low = ((1.0 - f) * f64::from(st.gray_mean)
                            + f * f64::from(st.white_mean))
                            as f32;
                    }
                }
                set_gray_hi(&mut o, &st);
                print_thresholds(r, &o, true);
                if o.log_stats {
                    write_dat(r, &o, &st);
                }
                let _ = writeln!(r.out, "Redoing initial intensity segmentation...");
                let _ = r.out.flush();
                let mut t2 = first_labels(g, r, &o, &src)?;
                // FreeSurfer keeps the first labels (it never frees them)
                leaked = Some(t);
                diag_write(g, r, &mut t2, "wmseg.int.2.mgz")?;
                let _ = writeln!(
                    r.out,
                    "Recomputing local statistics to label ambiguous voxels..."
                );
                let _ = r.out.flush();
                histo(g, r, &o, &src, &mut t2)?;
                diag_write(g, r, &mut t2, "wmseg.histo.2.mgz")?;
                t = t2;
            } else {
                st = Stats {
                    white_mean: 110.0,
                    white_sigma: 5.0,
                    gray_mean: 65.0,
                    gray_sigma: 12.0,
                };
            }
            let _ = writeln!(r.out, " wm_low {}", pg(f64::from(o.wm_low)));
            let _ = writeln!(r.out, " wm_hi  {}", pg(f64::from(o.wm_hi)));
            let _ = writeln!(r.out, " gray_low {}", pg(f64::from(o.gray_low)));
            let _ = writeln!(r.out, " gray_hi  {}", pg(f64::from(o.gray_hi)));
            tmp = t;
        }
        Some(segname) => {
            tmp = seg_path(g, r, &mut o, &src, segname.as_deref(), &mut st)?;
        }
    }
    // with reproduce the mask became the labels (FreeSurfer's call); either way it is done
    o.min_wm_mask = None;

    let _ = writeln!(
        r.out,
        "using local geometry to label remaining ambiguous voxels..."
    );
    let _ = writeln!(
        r.out,
        "polvwsize = {}, polvlen = {}, gray_hi = {}, wm_low = {}",
        o.polvwsize,
        pg(f64::from(o.polvlen)),
        pg(f64::from(o.gray_hi)),
        pg(f64::from(o.wm_low))
    );
    let _ = r.out.flush();
    let ctx = r.ctx();
    let mut labeled = cpolv_median_curve_segment(
        g,
        r.out,
        r.err,
        ctx,
        &src,
        &tmp,
        o.polvwsize,
        o.polvlen,
        o.gray_hi,
        o.wm_low,
    )?;
    drop(tmp);
    diag_write(g, r, &mut labeled, "wmseg.medcurv.mgz")?;

    let _ = writeln!(
        r.out,
        "\nReclassifying voxels using Gaussian border classifier niter={}",
        o.niter
    );
    let _ = r.out.flush();
    let low5 = sse::sub(o.wm_low, 5.0);
    for _ in 0..o.niter.max(0) {
        mri_reclassify(
            g,
            r.out,
            r.err,
            &src,
            Pair::Same(&mut labeled),
            low5,
            o.gray_hi,
            o.wsize,
        )?;
    }
    diag_write(g, r, &mut labeled, "wmseg.reclassified.mgz")?;

    let mut dst = mask_labels(g, r.out, r.err, &src, &labeled)?;
    drop(labeled);

    let _ = writeln!(r.out, "Recovering bright white");
    let _ = r.out.flush();
    recover_bright_white(
        g,
        r.out,
        r.err,
        &src,
        &mut dst,
        o.wm_low,
        o.wm_hi,
        st.white_sigma,
        0.33,
    )?;
    diag_write(g, r, &mut dst, "wmseg.recover-brightwm.mgz")?;

    let _ = writeln!(r.out, "\nremoving voxels with positive offset direction...");
    let _ = r.out.flush();
    remove_wrong_direction(g, r.out, r.err, &mut dst, 3, low5, o.gray_hi, o.blur_sigma)?;
    diag_write(g, r, &mut dst, "wmseg.wrong-dir.mgz")?;

    let _ = writeln!(r.out, "thicken = {}", o.thicken);
    if o.thicken != 0 {
        if !o.no1d_remove {
            let _ = writeln!(r.out, "removing 1-dimensional structures...");
            let _ = r.out.flush();
            remove_1d_structures(g, r.out, r.err, &mut dst, 10000, 2, labels.as_mut())?;
            diag_write(g, r, &mut dst, "wmseg.rm1d.mgz")?;
        }
        let _ = writeln!(r.out, "thickening thin strands....");
        let _ = writeln!(r.out, "thickness {}", o.thickness);
        let _ = writeln!(r.out, "nsegments {}", o.nsegments);
        let _ = writeln!(r.out, "wm_hi {}", pg(f64::from(o.wm_hi)));
        let _ = r.out.flush();
        let ctx = r.ctx();
        thicken_thin_wm_strands_in_place(
            g,
            r.out,
            r.err,
            ctx,
            &src,
            &mut dst,
            o.thickness,
            o.nsegments,
            o.wm_hi,
        )?;
        diag_write(g, r, &mut dst, "wmseg.thicken.mgz")?;
    }

    // the library's MRIfindBrightNonWM
    let Some(mut bright) = mri_find_bright_non_wm(g, r.out, r.err, &src, &dst)? else {
        return Err(Fatal::Exit(1));
    };
    mri_binarize(
        g,
        r.out,
        r.err,
        Pair::Same(&mut bright),
        WM_MIN_VAL as f32,
        255.0,
        0.0,
    )?;
    mask_labels_in_place(g, r.err, &mut dst, &bright)?;
    drop(bright);
    diag_write(g, r, &mut dst, "wmseg.bright-nonwm.mgz")?;

    filter_morphology(g, r.out, r.err, &mut dst)?;
    diag_write(g, r, &mut dst, "wmseg.filter.mgz")?;

    if o.fill_bg {
        let _ = writeln!(r.err, "filling basal ganglia....");
        fill_basal_ganglia(g, r.out, r.err, &mut src, &mut dst)?;
    }
    if o.fill_ventricles {
        let _ = writeln!(r.err, "filling ventricles....");
        let ctx = r.ctx();
        fill_ventricles(g, r.out, r.err, ctx, &mut dst)?;
    }

    drop(src);
    let msec = then.milliseconds();
    let _ = writeln!(
        r.out,
        "white matter segmentation took {} minutes",
        fmt_f(f64::from(msec as f32 / (1000.0f32 * 60.0f32)), 2, 1)
    );
    let _ = r.out.flush();

    if o.keep_edits {
        match read(g, r, &output_file_name)? {
            None => {
                log::error_printf(
                    g,
                    r.err,
                    ERROR_NOFILE,
                    &format!("{NAME}: could not read file {output_file_name} to preserve edits"),
                );
                return Err(Fatal::Exit(1));
            }
            Some(old) => {
                mri_copy_label(g, r.err, &old, &mut dst, WM_EDITED_ON_VAL)?;
                mri_copy_label(g, r.err, &old, &mut dst, WM_EDITED_OFF_VAL)?;
            }
        }
    }

    let _ = writeln!(r.out, "writing output to {output_file_name}...");
    let _ = r.out.flush();
    let written = write_output(g, r, &mut dst, &output_file_name)?;
    drop(dst);
    // FreeSurfer never frees these
    drop(labels);
    drop(leaked);
    if !written {
        return Err(Fatal::Exit(1));
    }
    Ok(())
}

/// `MRIintensitySegmentation(mri_src, NULL, MinWMMask, wm_low, wm_hi, gray_hi)`: into the mask
/// when there is one (`reproduce` only: FS-1224), else into a clone of the input.
fn first_labels(g: &mut Globals, r: &mut Run, o: &Options, src: &Mri) -> Result<Mri, Fatal> {
    // FreeSurfer's call passes the mask as the labels' volume (its minimal mask is NULL)
    match o.min_wm_mask.as_ref() {
        Some(mask) => {
            let mut labels = mri_copy(g, r.out, r.err, mask, None)?.unwrap_new();
            intensity_segmentation(
                g,
                r.out,
                r.err,
                src,
                None,
                Some(&mut labels),
                o.wm_low,
                o.wm_hi,
                o.gray_hi,
            )?;
            Ok(labels)
        }
        None => Ok(intensity_segmentation(
            g, r.out, r.err, src, None, None, o.wm_low, o.wm_hi, o.gray_hi,
        )?
        .unwrap_new()),
    }
}

/// `MRIhistoSegment(mri_src, mri_tmp, wm_low, wm_hi, gray_hi, wsize, 3.0f)` (the thresholds as
/// `int`s).
fn histo(g: &mut Globals, r: &mut Run, o: &Options, src: &Mri, t: &mut Mri) -> Result<(), Fatal> {
    let (lo, hi, gh) = (
        to_int(g, o.wm_low),
        to_int(g, o.wm_hi),
        to_int(g, o.gray_hi),
    );
    mri_histo_segment(
        g,
        r.out,
        r.err,
        Pair::Given(src, t),
        lo,
        hi,
        gh,
        o.wsize,
        3.0,
    )?;
    Ok(())
}

/// `gray_hi = gray_mean + 2 gray_sigma` (at most `white_mean - 1`) unless given.
fn set_gray_hi(o: &mut Options, st: &Stats) {
    if !o.gray_hi_set {
        o.gray_hi = sse::add(st.gray_mean, sse::mul(2.0, st.gray_sigma));
        if o.gray_hi >= st.white_mean {
            o.gray_hi = sse::sub(st.white_mean, 1.0);
        }
    }
}

/// The thresholds' lines after the statistics.
fn print_thresholds(r: &mut Run, o: &Options, flush: bool) {
    let _ = writeln!(
        r.out,
        "setting bottom of white matter range wm_low to {}",
        f1(o.wm_low)
    );
    let _ = writeln!(
        r.out,
        "setting top of gray matter range gray_hi to {}",
        f1(o.gray_hi)
    );
    let _ = writeln!(r.out, " wm_low {}", pg(f64::from(o.wm_low)));
    let _ = writeln!(r.out, " wm_hi  {}", pg(f64::from(o.wm_hi)));
    let _ = writeln!(r.out, " gray_low {}", pg(f64::from(o.gray_low)));
    let _ = writeln!(r.out, " gray_hi  {}", pg(f64::from(o.gray_hi)));
    if flush {
        let _ = r.out.flush();
    }
}

/// `segment.dat` (or `-dat`'s name): the statistics and thresholds, when it can be written.
fn write_dat(r: &mut Run, o: &Options, st: &Stats) {
    let Some(name) = o.segmentdatfile.as_deref() else {
        return;
    };
    let text = format!(
        "WM: {} +- {}\nGM: {} +- {}\nsetting bottom of white matter range to {}\nsetting top of gray matter range to {}\n",
        f1(st.white_mean),
        f1(st.white_sigma),
        f1(st.gray_mean),
        f1(st.gray_sigma),
        f1(o.wm_low),
        f1(o.gray_hi)
    );
    let _ = std::fs::write(resolve(r.dir, name), text);
}

/// The `-seg` path: the aseg read (FS-1221: FreeSurfer reads `aseg.presurf.mgz` whatever the
/// name), relabelled (`MRIreclassifyWMCtxNonPar`, 30 passes), its ambiguous voxels
/// (`MRIclassifyAmbiguousNonPar`), the class statistics (`MRIcomputeClassStatisticsSeg`) and the
/// thresholds from them. The labels returned.
fn seg_path(
    g: &mut Globals,
    r: &mut Run,
    o: &mut Options,
    src: &Mri,
    segname: Option<&str>,
    st: &mut Stats,
) -> Result<Mri, Fatal> {
    let shown = segname.unwrap_or("(null)");
    let _ = writeln!(r.out, "Reading {shown}");
    const FIXED: &str = "aseg.presurf.mgz";
    let name = match segname {
        Some(n) if n != FIXED => {
            g.notices.add(
                &FS_1221,
                format!(
                    "mri_segment -seg {n}: FreeSurfer reads {FIXED} of the working folder instead"
                ),
            );
            if g.notices.reproduce() { FIXED } else { n }
        }
        Some(n) => n,
        None => FIXED,
    };
    let Some(seg) = read(g, r, name)? else {
        return Err(Fatal::Exit(1));
    };
    if seg.vg.width != src.vg.width
        || seg.vg.height != src.vg.height
        || seg.vg.depth != src.vg.depth
    {
        g.notices.add(
            &FS_1222,
            format!(
                "mri_segment -seg: the aseg is {}x{}x{}, the input {}x{}x{}: refused",
                seg.vg.width,
                seg.vg.height,
                seg.vg.depth,
                src.vg.width,
                src.vg.height,
                src.vg.depth
            ),
        );
        log::error_exit(
            g,
            r.err,
            ERROR_BADPARM,
            &format!("{NAME}: the -seg volume {name} does not have the input's dimensions"),
        )?;
    }
    let Some(newseg) =
        mri_reclassify_wm_ctx_non_par(g, r.out, r.err, src, Pair::New(&seg), 30)?.into_new()
    else {
        return Err(Fatal::Exit(1));
    };
    diag_write_ref(g, r, &newseg, "wmseg.newseg.mgz")?;
    let Some(mut tmp) = mri_classify_ambiguous_non_par(
        g, r.out, r.err, src, &newseg, &seg, o.qwm, o.qctx, o.ndil_wm, o.ndil_ctx, r.dir,
    )?
    else {
        return Err(Fatal::Exit(SEGFAULT));
    };
    diag_write(g, r, &mut tmp, "wmseg.ambi.mgz")?;
    let cs = mri_compute_class_statistics_seg(g, r.out, r.err, src, &newseg)?;
    drop(seg);
    drop(newseg);
    let (wm, ctx) = match (cs.wm, cs.ctx) {
        (Some(w), Some(c)) => (w, c),
        (Some(w), None) => {
            g.notices.add(
                &FS_1231,
                "mri_segment -seg: no white matter left in the relabelled aseg: FreeSurfer's cortex statistics are memory it never set; 0",
            );
            if !g.notices.reproduce() {
                log::error_exit(
                    g,
                    r.err,
                    ERROR_BADPARM,
                    &format!("{NAME}: -seg: the relabelled aseg holds no white matter"),
                )?;
            }
            (w, (0.0, 0.0))
        }
        _ => unreachable!("the white matter's statistics always come first"),
    };
    *st = Stats {
        white_mean: wm.0,
        white_sigma: wm.1,
        gray_mean: ctx.0,
        gray_sigma: ctx.1,
    };
    if !o.wm_low_set {
        if o.wm_low_factor == 0.0 {
            o.wm_low = sse::add(st.gray_mean, st.gray_sigma);
        } else {
            let _ = writeln!(r.out, "using wm_low_factor = {}", f1(o.wm_low_factor));
            let f = f64::from(o.wm_low_factor);
            o.wm_low = (f * f64::from(st.gray_mean) + (1.0 - f) * f64::from(st.white_mean)) as f32;
        }
    }
    set_gray_hi(o, st);
    print_thresholds(r, o, true);
    Ok(tmp)
}

/// [`diag_write`] of a volume the caller keeps.
fn diag_write_ref(g: &mut Globals, r: &mut Run, mri: &Mri, fname: &str) -> Result<(), Fatal> {
    if g.gdiag & DIAG_WRITE != 0 && g.gdiag & DIAG_VERBOSE != 0 {
        let mut m = mri_copy(g, r.out, r.err, mri, None)?.unwrap_new();
        diag_write(g, r, &mut m, fname)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn polv_positions_count_the_float_loop() {
        assert_eq!(super::polv_positions(3.0), 25);
        assert_eq!(super::polv_positions(24.75), 199);
        assert_eq!(super::polv_positions(24.875), 200);
        assert_eq!(super::polv_positions(25.0), 201);
        assert_eq!(super::polv_positions(-1.0), 0);
        assert_eq!(super::polv_positions(f32::NAN), 0);
        assert_eq!(super::polv_positions(1e30), 201);
    }
}
