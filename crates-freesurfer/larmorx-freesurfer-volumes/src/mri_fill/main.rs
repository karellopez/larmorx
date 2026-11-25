// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_fill`'s program (queue items 3.2-35 and 3.2-37, `mri_fill/mri_fill.cpp` 1996-3362,
//! 3505-3840): the entry point [`run`] (registered as `mri_fill` in the crate's `PROGRAMS`),
//! [`main`] and the options ([`get_option`]).
//!
//! `mri_fill [options] <wm> <filled>` (recon-all: `-a ../scripts/ponscc.cut.log -xform
//! transforms/talairach.lta -segmentation aseg.presurf.mgz -ctab SubCorticalMassLUT.txt wm.mgz
//! filled.mgz`). The white matter is read (made bytes), its fill labels (200 to 240) saved and
//! erased; a Talairach volume made through the transform (`-xform`, else the volume's own or the
//! identity); the corpus callosum's cutting plane found (from a seed, `-C`/`-CV`, or the
//! segmentation's midline, or `find_corpus_callosum`, or Talairach (0, 0, 27)) and with no
//! segmentation the pons' (`-P`/`-PV`, or `find_pons`); the brainstem erased (with a segmentation)
//! or the planes cut (without); the hemispheres' seeds found near Talairach x of the callosum ±18
//! mm; the fill made by the segmentation (`fill_with_aseg`) or by two flood fills combined; then
//! `-topofix`, `-auto-man`'s edits, `-ctab`'s colour table, the output, and the check of the
//! split against the callosum's Talairach x.
//!
//! Defects (`larmorx_core::defects`; the mechanisms in `docs/findings/freesurfer-mri-fill.md`):
//! FS-1291 and FS-1300 (voxels outside a volume), FS-1292 (an option without its values), FS-1293, FS-1294,
//! FS-1295 ([`super::planes`]), FS-1296, FS-1297 ([`super::hemi`]), FS-1298 (a step that returns no
//! volume: FreeSurfer goes on and crashes), FS-1299 (an output that is not written: status 0);
//! FS-1301 (`-lhonly` and `-rhonly` without a segmentation crash), FS-1302 (a seed outside the
//! volume: [`super::hemi`]); FS-1287 to FS-1290 (`-topofix`: [`super::topofix`]). `-atlas` (the ventricles filled from an
//! atlas after the fill) is not ported yet: no recorded step reaches it.

use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

use larmorx_core::defects::{FS_1292, FS_1298, FS_1299, FS_1301};
use larmorx_freesurfer_core::cstd::{atof, atoi, fmt_f, format_double};
use larmorx_freesurfer_core::diag::{DIAG_SHOW, DIAG_VERBOSE, DIAG_WRITE, diag_init};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::help_xml::output_help_xml;
use larmorx_freesurfer_core::log::{
    self, ERROR_BADFILE, ERROR_BADPARM, ERROR_NOFILE, Fatal, errno_of,
};
use larmorx_freesurfer_core::matrix_ops::matrix_print;
use larmorx_freesurfer_core::run::{Environment, Settings};
use larmorx_freesurfer_core::system;
use larmorx_freesurfer_core::utils::{nint, resolve, stricmp};
use larmorx_freesurfer_core::version::{get_all_info, get_version, handle_version_option};
use larmorx_freesurfer_gca::transform::GcaLtaIo;
use larmorx_freesurfer_io::colortab::ctab_read_ascii;
use larmorx_freesurfer_io::ctrpoints::{ControlInfo, mri_fill_points, mri_read_control_points};
use larmorx_freesurfer_io::mriio::{mri_add_command_line, mri_read_volume, mri_write};
use larmorx_freesurfer_mri::copy::mri_copy;
use larmorx_freesurfer_mri::ctab::ColorTable;
use larmorx_freesurfer_mri::lta::{LINEAR_RAS_TO_RAS, Lta, lta_alloc};
use larmorx_freesurfer_mri::mri::{
    MGZ_INTENT_UNKNOWN, MRI_UCHAR, Mri, mri_alloc, mri_alloc_header, mri_copy_header,
};
use larmorx_freesurfer_mri::vol_geom::{fill_cache, voxel_to_world};
use larmorx_freesurfer_volume::arith::{Pair, mri_binarize, mri_clone, mri_replace_values};
use larmorx_freesurfer_volume::cma::{Brain_Stem, CC_Central};
use larmorx_freesurfer_volume::combine::{DiffDst, mri_apply_diff_vect, mri_diff2vect};
use larmorx_freesurfer_volume::coords::MRI_SAGITTAL;
use larmorx_freesurfer_volume::ltaapply::lta_voxel_transform_to_coronal_ras_transform;
use larmorx_freesurfer_volume::ltaops::lta_change_type;
use larmorx_freesurfer_volume::ltaread::lta_read_ex;
use larmorx_freesurfer_volume::mriset::{
    mri_copy_label, mri_dilate, mri_label_in_volume, mri_mask, mri_set_label_values,
};
use larmorx_freesurfer_volume::resize::mri_change_type;
use larmorx_freesurfer_volume::talairachex::{
    modify_talairach_cras, mri_from_talairach_ex, mri_talairach_to_voxel_ex, mri_to_talairach_ex,
    mri_to_talairach_ex_interp, mri_voxel_to_talairach_ex,
};
use larmorx_freesurfer_volume::vgeom::get_vol_geom;

use super::hemi::{
    FillState, fill_with_aseg, mri_combine_hemispheres, mri_fill_degenerate_locations,
    mri_fill_volume,
};
use super::local::{
    Ax, FILL_LABELS, MRI_HORIZONTAL, MRI_LEFT_HEMISPHERE, MRI_RIGHT_HEMISPHERE, SEGFAULT,
    WM_MIN_VAL, WM_THRESH, int64, ix, vox,
};
use super::planes::{
    extend_to_lateral_borders, find_cc_seed_with_segmentation, find_corpus_callosum,
    find_cutting_plane, find_pons, find_rh_seed_point, mri_erase_nonmidline_voxels,
    mri_replace_cc_with_wm, neighbors_on, note_backup_space, write_diag,
};
use super::topofix::{Counters, find_min_size, mri_topofix, verify_lr_split};
use crate::mri_mask::main::Cout;

/// The program's name as `argv[0]` gives it (recon-all runs it by name).
pub const NAME: &str = "mri_fill";

/// `mri_fill.help.xml`, the text `print_help` renders (`outputHelpXml`).
pub const HELP_XML: &[u8] = include_bytes!("mri_fill.help.xml");

/// `SEED_SEARCH_SIZE`, `MIN_NEIGHBORS`.
const SEED_SEARCH_SIZE: i32 = 9;
const MIN_NEIGHBORS: i32 = 5;
/// `CORPUS_CALLOSUM_TAL_*`, `PONS_TAL_*`: the fallback seeds.
const CC_TAL: [f64; 3] = [0.0, 0.0, 27.0];
const PONS_TAL: [f64; 3] = [-2.0, -15.0, -17.0];
/// `FLT_MIN`: `-auto-man`'s difference threshold.
const FLT_MIN: f64 = f32::MIN_POSITIVE as f64;

/// A file the program writes with `fprintf` (`-a`, `-L`): created at the open, its text kept
/// until it is closed or the program ends (C's buffer: a crash loses it).
pub struct LogFile {
    path: PathBuf,
    pub buf: Vec<u8>,
}

impl LogFile {
    fn close(self) {
        if let Ok(mut f) = std::fs::OpenOptions::new().append(true).open(&self.path) {
            let _ = f.write_all(&self.buf);
        }
    }
}

/// The settings the options give (the file-scope variables of `mri_fill.cpp`).
pub struct Options {
    pub find_rh_voxel: bool,
    pub find_lh_voxel: bool,
    pub fillonly: bool,
    pub fillven: i32,
    pub log_fp: Option<LogFile>,
    pub alog_fp: Option<LogFile>,
    pub topofix: bool,
    pub automaneditsfile: Option<String>,
    pub do_auto_man: bool,
    pub automandiffvec: Option<Vec<[f64; 5]>>,
    pub cc_tal: [f64; 3],
    pub pons_tal: [f64; 3],
    pub cc_seed_set: bool,
    pub pons_seed_set: bool,
    pub cc_vol: [f64; 3],
    pub pons_vol: [f64; 3],
    pub cc_seed_vol_set: bool,
    pub pons_seed_vol_set: bool,
    pub lh_seed_set: bool,
    pub rh_seed_set: bool,
    pub lh_tal: [f64; 3],
    pub rh_tal: [f64; 3],
    pub rh_vol: (i32, i32, i32),
    pub lh_vol: (i32, i32, i32),
    pub lhv: bool,
    pub rhv: bool,
    pub logging: bool,
    pub fill_val: i32,
    pub cc_mask: bool,
    pub sname: Option<String>,
    pub atlas_name: Option<String>,
    pub lta: Option<Lta>,
    pub norm_fname: Option<String>,
    pub segmentation_fname: Option<String>,
    pub ctab: Option<ColorTable>,
    pub fill: FillState,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            find_rh_voxel: false,
            find_lh_voxel: false,
            fillonly: false,
            fillven: 0,
            log_fp: None,
            alog_fp: None,
            topofix: false,
            automaneditsfile: None,
            do_auto_man: false,
            automandiffvec: None,
            cc_tal: CC_TAL,
            pons_tal: PONS_TAL,
            cc_seed_set: false,
            pons_seed_set: false,
            cc_vol: [0.0; 3],
            pons_vol: [0.0; 3],
            cc_seed_vol_set: false,
            pons_seed_vol_set: false,
            lh_seed_set: false,
            rh_seed_set: false,
            lh_tal: [0.0; 3],
            rh_tal: [0.0; 3],
            rh_vol: (0, 0, 0),
            lh_vol: (0, 0, 0),
            lhv: false,
            rhv: false,
            logging: false,
            fill_val: 0,
            cc_mask: true,
            sname: None,
            atlas_name: None,
            lta: None,
            norm_fname: None,
            segmentation_fname: None,
            ctab: None,
            fill: FillState {
                neighbor_threshold: 1,
                min_filled: 0,
                fill_holes_flag: true,
                lhonly: false,
                rhonly: false,
                lh_fill_val: MRI_LEFT_HEMISPHERE,
                rh_fill_val: MRI_RIGHT_HEMISPHERE,
                xlim0: 0,
                xlim1: 0,
                ylim0: 0,
                ylim1: 0,
            },
        }
    }
}

impl Options {
    /// The log files as the C library leaves them when the process ends normally (`exit`, or
    /// returning from `main`): their buffers written.
    fn flush_logs(&mut self) {
        if let Some(f) = self.log_fp.take() {
            f.close();
        }
        if let Some(f) = self.alog_fp.take() {
            f.close();
        }
    }
}

/// The streams and the environment of a run.
pub struct Run<'a, 'b> {
    pub out: &'a mut Cout,
    pub err: &'a mut dyn Write,
    pub env: &'b Environment,
    pub dir: Option<&'b std::path::Path>,
}

/// `mri_fill`'s entry point: its original arguments (without the program's name), the settings,
/// the two streams; the exit status. The notices of the run follow on standard error.
pub fn run(args: &[String], settings: &Settings, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let mut g = Globals::new(settings.mode);
    g.stderr.read_write = system::stderr_read_write();
    let mut cout = Cout::new(&g);
    let mut o = Options::default();
    let status = {
        let mut terr = log::TrackedErr::new(&g, &mut *err);
        let mut r = Run {
            out: &mut cout,
            err: &mut terr,
            env: &settings.env,
            dir: settings.dir.as_deref(),
        };
        match main(&mut g, &mut r, &mut o, args) {
            Ok(()) => 0,
            Err(f) => f.status(),
        }
    };
    let crashed = i32::from(status) == SEGFAULT && g.notices.reproduce();
    let keep = if crashed {
        cout.kept_by_crash()
    } else {
        cout.buf.len()
    };
    if !crashed {
        o.flush_logs();
    }
    let _ = out.write_all(&cout.buf[..keep]);
    let _ = out.flush();
    g.notices.print(err);
    status
}

/// `print_help()`: the help (`outputHelpXml`) on standard output, then `exit(0)`.
fn print_help(g: &mut Globals, r: &mut Run) -> Fatal {
    output_help_xml(g, r.out, r.err, Some(HELP_XML));
    Fatal::Exit(0)
}

fn read(g: &mut Globals, r: &mut Run, fname: &str) -> Result<Option<Mri>, Fatal> {
    mri_read_volume(g, r.out, r.err, r.env, r.dir, fname)
}

/// How FreeSurfer goes on when an option's value is missing (`argv[k]` is the `NULL` after the
/// last argument).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Missing {
    /// `atoi`, `atof`, `sscanf` or a reader of the `NULL`: a crash.
    Crash,
    /// The `NULL` kept as a name and printed `(null)`; the options then end and the help follows.
    Null,
}

/// FS-1292: `opt` takes `n` values and `rest` (the option and what follows it) holds fewer.
/// `Ok(None)`: they are there. Otherwise the notice and, by default, a clear error; with
/// `reproduce` FreeSurfer's crash (`Err`) or `Ok(Some(have))` to go on with the values it has.
fn missing(
    g: &mut Globals,
    r: &mut Run,
    rest: &[String],
    n: usize,
    how: Missing,
) -> Result<Option<usize>, Fatal> {
    let have = rest.len() - 1;
    if have >= n {
        return Ok(None);
    }
    g.notices.add(
        &FS_1292,
        format!(
            "mri_fill: option '{}' without its {n} value(s): FreeSurfer reads past its arguments{}",
            rest[0],
            if how == Missing::Crash {
                " and crashes"
            } else {
                ""
            }
        ),
    );
    if g.notices.reproduce() {
        if how == Missing::Crash {
            return Err(Fatal::Exit(SEGFAULT));
        }
        return Ok(Some(have));
    }
    let _ = writeln!(r.err, "{NAME}: option {} needs {n} value(s)", rest[0]);
    Err(Fatal::Exit(1))
}

/// `fopen(name, "w")` of `-a` or `-L`: the file created; `None` (errno set) when it does not open.
fn open_log(g: &mut Globals, r: &Run, name: &str) -> Option<LogFile> {
    let path = resolve(r.dir, name);
    match std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&path)
    {
        Ok(_) => Some(LogFile {
            path,
            buf: Vec::new(),
        }),
        Err(e) => {
            g.errno = errno_of(&e).unwrap_or(0);
            None
        }
    }
}

/// Three numbers of an option (`atof` of each).
fn three_f(rest: &[String]) -> [f64; 3] {
    [
        atof(rest[1].as_bytes()),
        atof(rest[2].as_bytes()),
        atof(rest[3].as_bytes()),
    ]
}

/// Three integers of an option (`atoi` of each).
fn three_i(rest: &[String]) -> (i32, i32, i32) {
    (
        atoi(rest[1].as_bytes()),
        atoi(rest[2].as_bytes()),
        atoi(rest[3].as_bytes()),
    )
}

/// `sscanf(s, "%d", &n) < 1`'s value: the leading integer (after white space, with a sign) or
/// `None` when there is none.
fn scan_int(s: &str) -> Option<i32> {
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() && larmorx_freesurfer_core::cstd::is_space(b[i]) {
        i += 1;
    }
    let mut j = i;
    if j < b.len() && (b[j] == b'+' || b[j] == b'-') {
        j += 1;
    }
    if j < b.len() && b[j].is_ascii_digit() {
        Some(atoi(&b[i..]))
    } else {
        None
    }
}

/// `get_option(argc, argv)` on `rest` (`argv[1]` and what follows): the arguments it takes after
/// its own.
pub fn get_option(
    g: &mut Globals,
    r: &mut Run,
    o: &mut Options,
    rest: &[String],
) -> Result<usize, Fatal> {
    let option = &rest[0][1..];
    let is = |g: &mut Globals, n: &str| stricmp(g, option, n) == 0;
    if is(g, "-help") || is(g, "-usage") {
        return Err(print_help(g, r));
    }
    if is(g, "-version") {
        let _ = writeln!(r.err, "fill version {}", get_version());
        return Err(Fatal::Exit(0));
    }
    if is(g, "rval") || is(g, "lval") {
        missing(g, r, rest, 1, Missing::Crash)?;
        let v = atoi(rest[1].as_bytes());
        if is(g, "rval") {
            o.fill.rh_fill_val = v;
            let _ = writeln!(r.err, "using {v} as fill val for right hemisphere.");
        } else {
            o.fill.lh_fill_val = v;
            let _ = writeln!(r.err, "using {v} as fill val for left hemisphere.");
        }
        return Ok(1);
    }
    if is(g, "findrhv") {
        o.find_rh_voxel = true;
        let _ = writeln!(r.err, "finding any rh seed point that has all 27 nbrs on");
        return Ok(0);
    }
    if is(g, "findlhv") {
        o.find_lh_voxel = true;
        let _ = writeln!(r.err, "finding any lh seed point that has all 27 nbrs on");
        return Ok(0);
    }
    if is(g, "debug_voxel") {
        missing(g, r, rest, 3, Missing::Crash)?;
        let (x, y, z) = three_i(rest);
        (g.gx, g.gy, g.gz) = (x, y, z);
        let _ = writeln!(r.out, "debugging voxel ({x},  {y},  {z})");
        return Ok(3);
    }
    if is(g, "fillven") {
        missing(g, r, rest, 1, Missing::Crash)?;
        o.fillven = atoi(rest[1].as_bytes());
        let _ = writeln!(
            r.out,
            "{}filling ventricles",
            if o.fillven == 0 { "not " } else { "" }
        );
        return Ok(1);
    }
    if is(g, "lh") || is(g, "rh") {
        missing(g, r, rest, 3, Missing::Crash)?;
        let t = three_f(rest);
        let side = if is(g, "lh") { "lh" } else { "rh" };
        if side == "lh" {
            o.lh_seed_set = true;
            o.lh_tal = t;
        } else {
            o.rh_seed_set = true;
            o.rh_tal = t;
        }
        let _ = writeln!(
            r.err,
            "using Talairach position ({}, {}, {}) as {side} seed",
            fmt_f(t[0], 2, 1),
            fmt_f(t[1], 2, 1),
            fmt_f(t[2], 2, 1)
        );
        return Ok(3);
    }
    if is(g, "segmentation") {
        if missing(g, r, rest, 1, Missing::Null)?.is_some() {
            o.segmentation_fname = None;
            let _ = writeln!(r.err, "using segmentation (null)...");
            return Ok(1);
        }
        o.segmentation_fname = Some(rest[1].clone());
        let _ = writeln!(r.err, "using segmentation {}...", rest[1]);
        return Ok(1);
    }
    if is(g, "lhonly") {
        o.fill.lhonly = true;
        let _ = writeln!(r.err, "assuming only lh is present");
        return Ok(0);
    }
    if is(g, "rhonly") {
        o.fill.rhonly = true;
        let _ = writeln!(r.err, "assuming only rh is present");
        return Ok(0);
    }
    if is(g, "fillonly") {
        o.fillonly = true;
        let _ = writeln!(r.out, "only filling volume...");
        return Ok(0);
    }
    if is(g, "topofix") {
        o.topofix = true;
        let _ = writeln!(r.out, "fixing the topology of the volume...");
        if missing(g, r, rest, 1, Missing::Null)?.is_some() {
            o.norm_fname = None;
            return Ok(1);
        }
        o.norm_fname = Some(rest[1].clone());
        return Ok(1);
    }
    if is(g, "rhv") || is(g, "lhv") {
        missing(g, r, rest, 3, Missing::Crash)?;
        let v = three_i(rest);
        let side = if is(g, "lhv") { "lh" } else { "rh" };
        if side == "lh" {
            o.lh_seed_set = true;
            o.lhv = true;
            o.lh_vol = v;
        } else {
            o.rh_seed_set = true;
            o.rhv = true;
            o.rh_vol = v;
        }
        let _ = writeln!(
            r.err,
            "using Volume position ({}, {}, {}) as {side} seed",
            v.0, v.1, v.2
        );
        return Ok(3);
    }
    if is(g, "ccmask") {
        o.cc_mask = !o.cc_mask;
        let _ = writeln!(
            r.err,
            "{}using corpus callosum to mask possible location of pons.",
            if o.cc_mask { "" } else { "not " }
        );
        return Ok(0);
    }
    if is(g, "atlas") {
        if missing(g, r, rest, 2, Missing::Null)?.is_some() {
            o.atlas_name = rest.get(1).cloned();
            o.sname = None;
            let _ = writeln!(
                r.err,
                "using atlas {} for auto filling...",
                o.atlas_name.as_deref().unwrap_or("(null)")
            );
            return Ok(2);
        }
        o.atlas_name = Some(rest[1].clone());
        o.sname = Some(rest[2].clone());
        let _ = writeln!(r.err, "using atlas {} for auto filling...", rest[1]);
        return Ok(2);
    }
    if is(g, "xform") {
        let name = if missing(g, r, rest, 1, Missing::Null)?.is_some() {
            // stat(NULL) fails with EFAULT
            g.errno = 14;
            log::error_exit(
                g,
                r.err,
                ERROR_BADPARM,
                "ERROR: cound not stat xform:(null).  Does it exist?\n",
            )?;
            return Ok(1);
        } else {
            rest[1].clone()
        };
        if let Err(e) = std::fs::metadata(resolve(r.dir, &name)) {
            g.errno = errno_of(&e).unwrap_or(0);
            log::error_exit(
                g,
                r.err,
                ERROR_BADPARM,
                &format!("ERROR: cound not stat xform:{name}.  Does it exist?\n"),
            )?;
        }
        let mut io = GcaLtaIo { env: r.env };
        let lta = lta_read_ex(g, r.out, r.err, r.dir, &name, &mut io)?;
        if lta.is_none() {
            log::error_exit(
                g,
                r.err,
                ERROR_BADPARM,
                &format!("ERROR: cound not load lta from {name}.\n"),
            )?;
        }
        o.lta = lta;
        let _ = writeln!(
            r.err,
            "INFO: Using {name} and its offset for Talairach volume ..."
        );
        return Ok(1);
    }
    if is(g, "PV") || is(g, "CV") {
        missing(g, r, rest, 3, Missing::Crash)?;
        let v = three_f(rest);
        let (a, b, c) = (fmt_f(v[0], 2, 0), fmt_f(v[1], 2, 0), fmt_f(v[2], 2, 0));
        if is(g, "PV") {
            o.pons_vol = v;
            let _ = writeln!(
                r.err,
                "using voxel position ({a}, {b}, {c}) as pons seed point"
            );
            o.pons_seed_vol_set = true;
        } else {
            o.cc_vol = v;
            let _ = writeln!(
                r.err,
                "using voxel position ({a}, {b}, {c}) as corpus callosum seed point"
            );
            o.cc_seed_vol_set = true;
        }
        return Ok(3);
    }
    if is(g, "ctab") {
        if missing(g, r, rest, 1, Missing::Null)?.is_some() {
            // fopen(NULL) fails with EFAULT
            g.errno = 14;
            log::error_printf(
                g,
                r.err,
                ERROR_NOFILE,
                "CTABreadASCII((null)): could not open file",
            );
            return Err(Fatal::Exit(1));
        }
        let Some(ct) = ctab_read_ascii(g, r.out, r.err, r.dir, &rest[1]) else {
            return Err(Fatal::Exit(1));
        };
        o.ctab = Some(ct);
        return Ok(1);
    }
    if is(g, "auto-man") {
        // without its third value FreeSurfer reads both volumes and keeps a NULL file name
        let partial = rest.len() == 3;
        let third = if partial {
            missing(g, r, rest, 3, Missing::Null)?;
            None
        } else {
            missing(g, r, rest, 3, Missing::Crash)?;
            Some(rest[3].clone())
        };
        o.do_auto_man = true;
        let Some(mriauto) = read(g, r, &rest[1])? else {
            return Err(Fatal::Exit(1));
        };
        let Some(mriman) = read(g, r, &rest[2])? else {
            return Err(Fatal::Exit(1));
        };
        let _ = writeln!(
            r.out,
            "Using auto-man {} {} {}",
            rest[1],
            rest[2],
            third.as_deref().unwrap_or("(null)")
        );
        o.automaneditsfile = third;
        let v = mri_diff2vect(g, r.out, r.err, &mriauto, &mriman, FLT_MIN, None)?;
        let Some(v) = v else {
            // FreeSurfer reads the size of the vector it did not get
            g.notices.add(
                &FS_1298,
                "mri_fill -auto-man: the two volumes' differences could not be listed (their extents differ): FreeSurfer goes on and crashes",
            );
            if g.notices.reproduce() {
                return Err(Fatal::Exit(SEGFAULT));
            }
            let _ = writeln!(
                r.err,
                "{NAME}: -auto-man's two volumes must have the same extent"
            );
            return Err(Fatal::Exit(1));
        };
        let _ = writeln!(r.out, "ndiff {}", v.len() as i32);
        o.automandiffvec = Some(v);
        return Ok(3);
    }
    if is(g, "no-auto-man") {
        o.do_auto_man = false;
        return Ok(0);
    }
    if is(g, "pointset") {
        if rest.len() + 1 < 4 {
            let _ = writeln!(r.out, "-pointset invol outvol ps1 <ps2 ...>");
            return Err(Fatal::Exit(1));
        }
        let _ = writeln!(r.out, "Reading input {}", rest[1]);
        let Some(mut invol) = read(g, r, &rest[1])? else {
            return Err(Fatal::Exit(1));
        };
        let outvolfile = rest[2].clone();
        for ps in &rest[3..] {
            let _ = writeln!(r.out, "Reading control point file {ps}");
            let mut info = ControlInfo::default();
            let Some(mp) = mri_read_control_points(g, r.env, r.out, r.err, r.dir, ps, &mut info)?
            else {
                return Err(Fatal::Exit(1));
            };
            mri_fill_points(
                g,
                r.out,
                r.err,
                &mut invol,
                0,
                info.count,
                info.use_real_ras,
                &mp,
            )?;
        }
        let _ = writeln!(r.out, "Writing output {outvolfile}");
        let e = mri_write(
            g,
            r.out,
            r.err,
            r.env,
            r.dir,
            &mut invol,
            &outvolfile,
            None,
            MGZ_INTENT_UNKNOWN,
        )?;
        return Err(Fatal::Exit(e));
    }
    match option.bytes().next().map(|c| c.to_ascii_uppercase()) {
        Some(b'P') | Some(b'C') => {
            missing(g, r, rest, 3, Missing::Crash)?;
            let t = three_f(rest);
            let (a, b, c) = (fmt_f(t[0], 2, 0), fmt_f(t[1], 2, 0), fmt_f(t[2], 2, 0));
            if option.as_bytes()[0].eq_ignore_ascii_case(&b'P') {
                o.pons_tal = t;
                let _ = writeln!(
                    r.err,
                    "using Talairach position ({a}, {b}, {c}) as pons seed point"
                );
                o.pons_seed_set = true;
            } else {
                o.cc_tal = t;
                let _ = writeln!(
                    r.err,
                    "using Talairach position ({a}, {b}, {c}) as corpus callosum seed point"
                );
                o.cc_seed_set = true;
            }
            Ok(3)
        }
        Some(b'T') => {
            missing(g, r, rest, 1, Missing::Crash)?;
            match scan_int(&rest[1]) {
                Some(n) => o.fill.neighbor_threshold = n,
                None => {
                    let _ = write!(r.err, "fill: could not scan threshold from '{}'", rest[1]);
                    return Err(Fatal::Exit(1));
                }
            }
            Ok(1)
        }
        Some(c @ (b'L' | b'A')) => {
            let name = match missing(g, r, rest, 1, Missing::Null)? {
                Some(_) => {
                    // fopen(NULL, "w") fails with EFAULT
                    g.errno = 14;
                    log::error_exit(
                        g,
                        r.err,
                        ERROR_BADPARM,
                        &format!(
                            "{}: could not open cutting plane log file (null)",
                            g.progname
                        ),
                    )?;
                    return Ok(1);
                }
                None => rest[1].clone(),
            };
            let Some(mut f) = open_log(g, r, &name) else {
                log::error_exit(
                    g,
                    r.err,
                    ERROR_BADPARM,
                    &format!(
                        "{}: could not open cutting plane log file {name}",
                        g.progname
                    ),
                )?;
                return Ok(1);
            };
            let _ = writeln!(r.err, "logging cutting plane coordinates to {name}...");
            if c == b'A' {
                f.buf.extend_from_slice(
                    b"# This file contains the column, row, slice (CRS) and talirach XYZ (TAL)\n# of the cutting planes for the corpus callosum (CC) and pons\n# as generated by mri_fill. \n",
                );
                // a second -a leaves the first file as it was written so far (never closed)
                if let Some(old) = o.alog_fp.replace(f) {
                    old.close();
                }
            } else if let Some(old) = o.log_fp.replace(f) {
                old.close();
            }
            Ok(1)
        }
        Some(b'F') => {
            missing(g, r, rest, 1, Missing::Crash)?;
            o.fill_val = atoi(rest[1].as_bytes());
            Ok(1)
        }
        Some(b'D') => {
            o.logging = true;
            Ok(0)
        }
        Some(b'?') | Some(b'H') | Some(b'U') => Err(print_help(g, r)),
        _ => {
            let _ = writeln!(r.err, "unknown option {}", rest[0]);
            Err(Fatal::Exit(1))
        }
    }
}

fn binarize(g: &mut Globals, r: &mut Run, m: &mut Mri, thresh: f32, hi: f32) -> Result<(), Fatal> {
    mri_binarize(g, r.out, r.err, Pair::Same(m), thresh, 0.0, hi)?;
    Ok(())
}

/// `MRIvoxelToTalairachEx(mri, x, y, z, ..., lta)`: the point, or 0s when it cannot be
/// transformed (FS-487's error printed).
fn vox_to_tal(
    g: &mut Globals,
    r: &mut Run,
    m: &Mri,
    p: [f64; 3],
    lta: Option<&Lta>,
) -> Result<[f64; 3], Fatal> {
    let (_, t) = mri_voxel_to_talairach_ex(g, r.out, r.err, m, p[0], p[1], p[2], lta)?;
    Ok(t.unwrap_or([0.0; 3]))
}

/// `MRItalairachToVoxelEx(mri, x, y, z, ..., lta)` (see [`vox_to_tal`]).
fn tal_to_vox(
    g: &mut Globals,
    r: &mut Run,
    m: &Mri,
    p: [f64; 3],
    lta: Option<&Lta>,
) -> Result<[f64; 3], Fatal> {
    let (_, t) = mri_talairach_to_voxel_ex(g, r.out, r.err, m, p[0], p[1], p[2], lta)?;
    Ok(t.unwrap_or([0.0; 3]))
}

/// A Talairach volume like `talheader` holding `src` resampled (`MRItoTalairachEx`) and
/// binarized at 54 (0 and 110).
fn tal_volume(
    g: &mut Globals,
    r: &mut Run,
    src: &Mri,
    talheader: &Mri,
    lta: Option<&Lta>,
) -> Result<Mri, Fatal> {
    let mut tal = mri_alloc(
        g,
        r.out,
        r.err,
        src.vg.width,
        src.vg.height,
        src.vg.depth,
        src.mtype,
    )?;
    mri_copy_header(g, r.out, r.err, talheader, Some(&mut tal))?;
    mri_to_talairach_ex(g, r.out, r.err, r.env, src, Some(&mut tal), lta)?;
    binarize(g, r, &mut tal, WM_THRESH, 110.0)?;
    Ok(tal)
}

/// The cutting plane (in Talairach space) brought back into `im`'s space (`MRIfromTalairachEx` into
/// a copy of `im`) and binarized to 0 and 1.
fn plane_to_src(
    g: &mut Globals,
    r: &mut Run,
    plane: &Mri,
    im: &Mri,
    lta: Option<&Lta>,
) -> Result<Mri, Fatal> {
    let mut tmp = mri_copy(g, r.out, r.err, im, None)?.unwrap_new();
    mri_from_talairach_ex(g, r.out, r.err, r.env, plane, Some(&mut tmp), lta)?;
    binarize(g, r, &mut tmp, 1.0, 1.0)?;
    Ok(tmp)
}

/// The pons' mask of the callosum's plane: the plane dilated twice, inverted (0 to 255, 1 to 0) and
/// extended to the lateral borders; `im` (a copy) and the saved labels masked by it.
fn cc_mask_volumes(
    g: &mut Globals,
    r: &mut Run,
    im: &Mri,
    cc: &Mri,
    labels: &mut Mri,
) -> Result<Mri, Fatal> {
    let mut tmp = mri_copy(g, r.out, r.err, im, None)?.unwrap_new();
    let mut mask = mri_copy(g, r.out, r.err, cc, None)?.unwrap_new();
    mri_dilate(g, r.out, r.err, Pair::Same(&mut mask))?;
    mri_dilate(g, r.out, r.err, Pair::Same(&mut mask))?;
    mri_replace_values(g, r.out, r.err, Pair::Same(&mut mask), 0.0, 255.0)?;
    mri_replace_values(g, r.out, r.err, Pair::Same(&mut mask), 1.0, 0.0)?;
    extend_to_lateral_borders(g, &mut mask, 0);
    mri_mask(
        g,
        r.out,
        r.err,
        r.env,
        Pair::Same(&mut tmp),
        &mask,
        255,
        0.0,
    )?;
    mri_mask(g, r.out, r.err, r.env, Pair::Same(labels), &mask, 255, 0.0)?;
    Ok(tmp)
}

/// `MRIwrite(mri, fname)`'s status.
fn write(g: &mut Globals, r: &mut Run, m: &mut Mri, fname: &str) -> Result<i32, Fatal> {
    mri_write(
        g,
        r.out,
        r.err,
        r.env,
        r.dir,
        m,
        fname,
        None,
        MGZ_INTENT_UNKNOWN,
    )
}

/// FS-1299: an output `MRIwrite` did not write. FreeSurfer goes on and ends with status 0 (the
/// `-fillonly` and `-F` exits too); by default status 1 at the end.
fn unwritten(g: &mut Globals, fname: &str) -> bool {
    g.notices.add(
        &FS_1299,
        if g.notices.reproduce() {
            format!("mri_fill: '{fname}' was not written; status 0, as in FreeSurfer")
        } else {
            format!("mri_fill: '{fname}' was not written: status 1 (FreeSurfer ends with 0)")
        },
    );
    !g.notices.reproduce()
}

/// The hemispheres' seed search around `(x, y, z)`: of the voxels within `size` (index tables) at
/// or above `WM_MIN_VAL` with 5 or more face neighbours on, the first nearest (z outermost).
fn search_seed(
    g: &mut Globals,
    im: &Mri,
    c: (i32, i32, i32),
    size: i32,
    voxsize: f64,
) -> Option<(i32, i32, i32)> {
    let mut found = None;
    let mut min_dist = f64::from(10000.0f32) / voxsize;
    for z in c.2 - size..=c.2 + size {
        let zi = ix(g, im, Ax::Z, z);
        for y in c.1 - size..=c.1 + size {
            let yi = ix(g, im, Ax::Y, y);
            for x in c.0 - size..=c.0 + size {
                let xi = ix(g, im, Ax::X, x);
                if i32::from(vox(g, im, xi, yi, zi)) >= WM_MIN_VAL
                    && neighbors_on(g, im, xi, yi, zi) >= MIN_NEIGHBORS
                {
                    let (xd, yd, zd) = (xi - c.0, yi - c.1, zi - c.2);
                    let dist = f64::from(xd * xd + yd * yd + zd * zd);
                    if found.is_none() {
                        found = Some((0, 0, 0));
                    }
                    if dist < min_dist {
                        found = Some((xi, yi, zi));
                        min_dist = dist;
                    }
                }
            }
        }
    }
    found
}

/// A hemisphere's white-matter seed (`wm_rh_*` or `wm_lh_*`) from a Talairach point: its voxel
/// (`nint`), which must lie in the volume, moved by [`search_seed`] when it is dark or has fewer
/// than 5 face neighbours on.
#[allow(clippy::too_many_arguments)]
fn hemi_seed(
    g: &mut Globals,
    r: &mut Run,
    im: &Mri,
    p: [f64; 3],
    side: &str,
    size: i32,
    voxsize: f64,
) -> Result<(i32, i32, i32), Fatal> {
    let mut s = (nint(g, p[0]), nint(g, p[1]), nint(g, p[2]));
    let (w, h, d) = (im.vg.width, im.vg.height, im.vg.depth);
    if s.0 < 0 || s.0 >= w || s.1 < 0 || s.1 >= h || s.2 < 0 || s.2 >= d {
        log::error_exit(
            g,
            r.err,
            ERROR_BADPARM,
            &format!(
                "{side} white matter seed point out of bounds ({}, {}, {})\n",
                s.0, s.1, s.2
            ),
        )?;
    }
    if i32::from(vox(g, im, s.0, s.1, s.2)) <= WM_MIN_VAL
        || neighbors_on(g, im, s.0, s.1, s.2) < MIN_NEIGHBORS
    {
        if g.gdiag & DIAG_SHOW != 0 {
            let _ = write!(r.err, "searching for {side} wm seed...");
        }
        let Some(n) = search_seed(g, im, s, size, voxsize) else {
            log::error_exit(
                g,
                r.err,
                ERROR_BADPARM,
                &format!(
                    "could not find {side} seed point around ({}, {}, {})",
                    s.0, s.1, s.2
                ),
            )?;
            unreachable!("ErrorExit ends the program")
        };
        if g.gdiag & DIAG_SHOW != 0 {
            let _ = writeln!(r.err, "found at ({}, {}, {})", n.0, n.1, n.2);
        }
        s = n;
    }
    if g.gdiag & DIAG_SHOW != 0 {
        let n = neighbors_on(g, im, s.0, s.1, s.2);
        let _ = writeln!(
            r.err,
            "{side} seed point at ({}, {}, {}): {n} neighbors on.",
            s.0, s.1, s.2
        );
    }
    Ok(s)
}

fn diag_write(g: &Globals) -> bool {
    g.gdiag & DIAG_WRITE != 0 && g.gdiag & DIAG_VERBOSE != 0
}

/// The voxel seed (`-CV`, `-PV`) must lie in the volume (`ERROR: <what> voxel coordinate
/// out-of-bounds!`, the program ends).
fn check_vol_seed(
    g: &mut Globals,
    r: &mut Run,
    im: &Mri,
    v: [f64; 3],
    what: &str,
) -> Result<(), Fatal> {
    let dims = [im.vg.width, im.vg.height, im.vg.depth];
    for (k, axis) in ["x", "y", "z"].iter().enumerate() {
        if v[k] >= f64::from(dims[k]) || v[k] < 0.0 {
            log::error_exit(
                g,
                r.err,
                ERROR_BADPARM,
                &format!("ERROR: {what}_vol_{axis} voxel coordinate out-of-bounds!\n"),
            )?;
        }
    }
    Ok(())
}

/// A Talairach seed's voxel must lie in the volume: otherwise the two lines and `return -1`
/// (status 255).
fn check_tal_seed(
    g: &mut Globals,
    r: &mut Run,
    im: &Mri,
    t: [f64; 3],
    lta: Option<&Lta>,
    opt: &str,
) -> Result<(), Fatal> {
    let [xv, yv, zv] = tal_to_vox(g, r, im, t, lta)?;
    let (w, h, d) = (im.vg.width, im.vg.height, im.vg.depth);
    if xv > f64::from(w - 1)
        || xv < 0.0
        || yv > f64::from(h - 1)
        || yv < 0.0
        || zv > f64::from(d - 1)
        || zv < 0.0
    {
        let f = |v: f64| fmt_f(v, 0, 2);
        let _ = writeln!(
            r.err,
            "The seed point ({}, {}, {}) is mapped to a voxel ({}, {}. {}).",
            f(t[0]),
            f(t[1]),
            f(t[2]),
            f(xv),
            f(yv),
            f(zv)
        );
        let _ = writeln!(
            r.err,
            "Make sure that the seed point is given in talaraich position or use {opt} option"
        );
        return Err(Fatal::Exit(255));
    }
    Ok(())
}

/// The Talairach header: `mri_im`'s header with the Talairach volume's centre (`-xform`'s
/// transform made RAS to RAS; else a transform from the volume's own, or the identity).
fn talairach_header(g: &mut Globals, r: &mut Run, o: &mut Options, im: &Mri) -> Result<Mri, Fatal> {
    let mut talheader = mri_alloc_header(
        g,
        r.out,
        r.err,
        im.vg.width,
        im.vg.height,
        im.vg.depth,
        im.mtype,
        im.nframes,
    )?;
    mri_copy_header(g, r.out, r.err, im, Some(&mut talheader))?;
    if let Some(lta) = o.lta.as_mut() {
        if lta.ltype != LINEAR_RAS_TO_RAS {
            let valid = lta
                .xforms
                .first()
                .is_some_and(|x| x.src.valid != 0 && x.dst.valid != 0);
            if valid {
                lta_change_type(g, r.out, r.err, r.env, lta, LINEAR_RAS_TO_RAS)?;
            } else {
                lta_voxel_transform_to_coronal_ras_transform(g, r.out, r.err, lta)?;
            }
        }
        modify_talairach_cras(g, r.out, r.err, r.env, &mut talheader, Some(lta))?;
        return Ok(talheader);
    }
    let mut lta = lta_alloc(g, r.out, r.err, 1, None)?;
    {
        let lt = &mut lta.xforms[0];
        lt.sigma = 1.0;
        lt.x0 = 0.0;
        lt.y0 = 0.0;
        lt.z0 = 0.0;
        lt.m_l = larmorx_freesurfer_core::matrix_ops::matrix_identity(g, r.out, r.err, 4, None)?
            .into_new();
    }
    lta.ltype = LINEAR_RAS_TO_RAS;
    let lt = &mut lta.xforms[0];
    if let Some(t) = &im.linear_transform {
        let m_l = lt.m_l.as_mut().expect("the identity above");
        for row in 1..=3 {
            for col in 1..=4 {
                // `m[col - 1][row - 1]` of FreeSurfer's column-major `Transform`: element (row,
                // col), `t[row - 1][col - 1]` here (larmorx_io's row-major layout)
                *m_l.relt_mut(row, col) = t[(row - 1) as usize][(col - 1) as usize] as f32;
            }
        }
        let _ = r.err.write_all(b"talairach transform\n");
        let mut buf: Vec<u8> = Vec::new();
        matrix_print(
            g,
            Some(&mut buf as &mut dyn Write),
            r.out,
            r.err,
            Some(m_l),
            5,
        );
        let _ = r.err.write_all(&buf);
        get_vol_geom(&im.vg, &mut lt.src);
        get_vol_geom(&im.vg, &mut lt.dst);
        if r.env.get("NO_AVERAGE305").is_some() {
            let _ = r.err.write_all(b"INFO: tal dst c_(r,a,s) not modified\n");
        } else {
            let _ = r
                .err
                .write_all(b"INFO: Modifying talairach volume c_(r,a,s) based on average_305\n");
            lt.dst.valid = 1;
            lt.dst.c_r = -0.095f64 as f32;
            lt.dst.c_a = -16.51f64 as f32;
            lt.dst.c_s = 9.75f64 as f32;
            talheader.vg.c_r = lt.dst.c_r;
            talheader.vg.c_a = lt.dst.c_a;
            talheader.vg.c_s = lt.dst.c_s;
        }
    } else {
        let _ = writeln!(
            r.out,
            "INFO: volume does not have linear_transform set nor lta is given by option.-xform."
        );
        let _ = writeln!(
            r.out,
            "INFO: use identity matrix as the talairach transform."
        );
        let _ = writeln!(
            r.out,
            "INFO: use src volume parameters for the talairach volume."
        );
        get_vol_geom(&im.vg, &mut lt.src);
        get_vol_geom(&im.vg, &mut lt.dst);
        lt.dst.valid = 1;
        talheader.vg.c_r = lt.dst.c_r;
        talheader.vg.c_a = lt.dst.c_a;
        talheader.vg.c_s = lt.dst.c_s;
    }
    talheader.vg.i_to_r = None;
    talheader.vg.r_to_i = None;
    fill_cache(g, r.out, r.err, &mut talheader.vg)?;
    o.lta = Some(lta);
    Ok(talheader)
}

/// `main(argc, argv)` on the arguments after the program's name.
pub fn main(g: &mut Globals, r: &mut Run, o: &mut Options, args: &[String]) -> Result<(), Fatal> {
    let argv0: Vec<String> = std::iter::once(NAME.to_owned())
        .chain(args.iter().cloned())
        .collect();
    let env = r.env.clone();
    let cmdline = get_all_info(g, &env, &argv0, NAME);
    let (nargs, argv) = handle_version_option(g, &env, r.out, &argv0, NAME);
    if nargs != 0 && argv.len() == 1 {
        return Err(Fatal::Exit(0));
    }
    let then = Instant::now();
    diag_init(g, r.env, r.out);
    g.progname = NAME.to_owned();
    let mut argc = argv.len() as isize;
    let mut at = 1usize;
    while argc > 1 && argv.get(at).is_some_and(|a| a.starts_with('-')) {
        let n = get_option(g, r, o, &argv[at..])?;
        argc -= n as isize + 1;
        at += n + 1;
    }
    if argc < 3 {
        return Err(print_help(g, r));
    }
    let input_fname = argv[at].clone();
    let out_fname = argv[at + 1].clone();

    let _ = write!(r.out, "reading input volume... {input_fname}");
    let Some(mut mri_im) = read(g, r, &input_fname)? else {
        log::error_exit(
            g,
            r.err,
            ERROR_NOFILE,
            &format!("{NAME}: could not read {input_fname}"),
        )?;
        unreachable!("ErrorExit ends the program")
    };
    mri_add_command_line(g, r.err, &mut mri_im, cmdline.as_bytes());
    if mri_im.mtype != MRI_UCHAR {
        let Some(m) = mri_change_type(g, r.out, r.err, &env, &mri_im, MRI_UCHAR, 0.0, 0.999, 1)?
        else {
            g.notices.add(
                &FS_1298,
                "mri_fill: the white matter could not be made bytes: FreeSurfer goes on without it and crashes",
            );
            if g.notices.reproduce() {
                return Err(Fatal::Exit(SEGFAULT));
            }
            log::error_exit(
                g,
                r.err,
                ERROR_BADPARM,
                &format!("{NAME}: could not change {input_fname} to bytes"),
            )?;
            unreachable!("ErrorExit ends the program")
        };
        mri_im = m;
    }

    if o.find_rh_voxel {
        o.rh_vol = find_rh_seed_point(g, r, &mri_im)?;
    } else if o.find_lh_voxel {
        o.lh_vol = find_rh_seed_point(g, r, &mri_im)?;
    }

    if o.fillonly {
        let mut rh_fill = mri_clone(g, r.out, r.err, &mri_im, None)?.unwrap_new();
        let mut rh_im = mri_copy(g, r.out, r.err, &mri_im, None)?.unwrap_new();
        let mut s = o.fill.clone();
        if o.find_rh_voxel || o.rhv {
            let (x, y, z) = o.rh_vol;
            let v = s.rh_fill_val;
            mri_fill_volume(g, r, &mut s, &mut rh_fill, &mut rh_im, x, y, z, v)?;
        } else {
            let (x, y, z) = o.lh_vol;
            let v = s.lh_fill_val;
            mri_fill_volume(g, r, &mut s, &mut rh_fill, &mut rh_im, x, y, z, v)?;
        }
        if write(g, r, &mut rh_fill, &out_fname)? != 0 && unwritten(g, &out_fname) {
            return Err(Fatal::Exit(1));
        }
        return Ok(());
    }

    // the labels other than white matter saved and erased (restored before the fill)
    let mut mri_labels = mri_clone(g, r.out, r.err, &mri_im, None)?.unwrap_new();
    for &label in &FILL_LABELS {
        mri_copy_label(g, r.err, &mri_im, &mut mri_labels, label)?;
        mri_replace_values(g, r.out, r.err, Pair::Same(&mut mri_im), label as f32, 0.0)?;
    }
    let mut mri_saved_labels = Some(mri_copy(g, r.out, r.err, &mri_labels, None)?.unwrap_new());

    if g.gdiag == 0 {
        let _ = write!(r.err, "done.\nsearching for cutting planes...");
    }
    let talheader = talairach_header(g, r, o, &mri_im)?;
    let lta = o.lta.take();
    let lta_r = lta.as_ref();
    let mut mri_tal = tal_volume(g, r, &mri_im, &talheader, lta_r)?;

    let mut mri_seg = None;
    if let Some(sf) = o.segmentation_fname.clone() {
        let _ = writeln!(r.out, "reading segmented volume {sf}");
        let Some(seg) = read(g, r, &sf)? else {
            log::error_exit(
                g,
                r.err,
                ERROR_NOFILE,
                &format!("{NAME}: could not read segmentation from {sf}"),
            )?;
            unreachable!("ErrorExit ends the program")
        };
        let mut seg = seg;
        if mri_label_in_volume(g, r.err, &seg, CC_Central)? != 0 {
            let _ = writeln!(r.out, "removing CC from segmentation");
            seg = mri_replace_cc_with_wm(g, r, &env, &seg)?;
        }
        mri_seg = Some(seg);
    }

    // the corpus callosum's cutting plane, in the Talairach volume
    let mut pcc = (0i32, 0i32, 0i32);
    let mut mri_cc: Option<Mri> = None;
    if o.cc_seed_set {
        let _ = writeln!(r.out, "Using the seed to calculate the cutting plane");
        let _ = writeln!(
            r.out,
            "Verify whether the seed point is inside the volume first"
        );
        check_tal_seed(g, r, &mri_im, o.cc_tal, lta_r, "-CV")?;
        let [x, y, z] = o.cc_tal;
        mri_cc = find_cutting_plane(
            g,
            r,
            &mut mri_tal,
            x,
            y,
            z,
            MRI_SAGITTAL,
            &mut pcc,
            true,
            lta_r,
            o.logging,
        )?;
    } else if o.cc_seed_vol_set {
        check_vol_seed(g, r, &mri_im, o.cc_vol, "cc")?;
        o.cc_tal = vox_to_tal(g, r, &mri_im, o.cc_vol, lta_r)?;
        let f = |v: f64| fmt_f(v, 0, 2);
        let _ = writeln!(
            r.out,
            "Calculated talairach cc position ({}, {}, {})",
            f(o.cc_tal[0]),
            f(o.cc_tal[1]),
            f(o.cc_tal[2])
        );
        o.cc_seed_set = true;
        let _ = writeln!(r.out, "Using the seed to calculate the cutting plane");
        let [x, y, z] = o.cc_tal;
        mri_cc = find_cutting_plane(
            g,
            r,
            &mut mri_tal,
            x,
            y,
            z,
            MRI_SAGITTAL,
            &mut pcc,
            true,
            lta_r,
            o.logging,
        )?;
    } else {
        if let Some(seg) = mri_seg.as_ref() {
            let mut seg_tal = mri_clone(g, r.out, r.err, seg, None)?.unwrap_new();
            mri_copy_header(g, r.out, r.err, &talheader, Some(&mut seg_tal))?;
            mri_to_talairach_ex_interp(
                g,
                r.out,
                r.err,
                r.env,
                seg,
                Some(&mut seg_tal),
                lta_r,
                larmorx_freesurfer_mri::mri::SAMPLE_NEAREST,
            )?;
            let mut cc = o.cc_tal;
            if find_cc_seed_with_segmentation(g, r, &mut mri_tal, &seg_tal, &mut cc)? == 0 {
                o.cc_tal = cc;
                o.cc_seed_set = true;
                let [x, y, z] = cc;
                mri_cc = find_cutting_plane(
                    g,
                    r,
                    &mut mri_tal,
                    x,
                    y,
                    z,
                    MRI_SAGITTAL,
                    &mut pcc,
                    true,
                    lta_r,
                    o.logging,
                )?;
                if let Some(c) = mri_cc.as_mut() {
                    mri_erase_nonmidline_voxels(g, r, c, &seg_tal)?;
                    if diag_write(g) {
                        write_diag(g, r, c, "cc.mgz")?;
                    }
                }
            }
        }
        if mri_cc.is_none() {
            let mut cc = o.cc_tal;
            if find_corpus_callosum(g, r, &mut mri_tal, &mut cc, lta_r)? == 0 {
                o.cc_tal = cc;
                let [x, y, z] = cc;
                mri_cc = find_cutting_plane(
                    g,
                    r,
                    &mut mri_tal,
                    x,
                    y,
                    z,
                    MRI_SAGITTAL,
                    &mut pcc,
                    o.cc_seed_set,
                    lta_r,
                    o.logging,
                )?;
            }
        }
    }
    if mri_cc.is_none() {
        o.cc_tal = CC_TAL;
        let [x, y, z] = o.cc_tal;
        mri_cc = find_cutting_plane(
            g,
            r,
            &mut mri_tal,
            x,
            y,
            z,
            MRI_SAGITTAL,
            &mut pcc,
            o.cc_seed_set,
            lta_r,
            o.logging,
        )?;
        if mri_cc.is_none()
            && let Some(seg) = mri_seg.as_ref()
        {
            let mut seg_tal = mri_clone(g, r.out, r.err, seg, None)?.unwrap_new();
            mri_to_talairach_ex_interp(
                g,
                r.out,
                r.err,
                r.env,
                seg,
                Some(&mut seg_tal),
                lta_r,
                larmorx_freesurfer_mri::mri::SAMPLE_NEAREST,
            )?;
            let mut cc = o.cc_tal;
            if find_cc_seed_with_segmentation(g, r, &mut mri_tal, &seg_tal, &mut cc)? == 0 {
                o.cc_tal = cc;
                o.cc_seed_set = true;
            }
            let [x, y, z] = o.cc_tal;
            mri_cc = find_cutting_plane(
                g,
                r,
                &mut mri_tal,
                x,
                y,
                z,
                MRI_SAGITTAL,
                &mut pcc,
                o.cc_seed_set,
                lta_r,
                o.logging,
            )?;
        }
        if mri_cc.is_none() {
            log::error_exit(
                g,
                r.err,
                ERROR_BADPARM,
                &format!("{NAME}: could not find corpus callosum"),
            )?;
        }
    }
    let cc_plane = mri_cc.take().expect("found above");
    let mut mri_cc = plane_to_src(g, r, &cc_plane, &mri_im, lta_r)?;
    drop(cc_plane);
    if diag_write(g) {
        write_diag(g, r, &mut mri_cc, "cc.mgz")?;
    }
    let (x_cc, y_cc, z_cc) = pcc;
    let (a, b, c) = voxel_to_world(
        g,
        r.out,
        r.err,
        &mut mri_tal.vg,
        f64::from(x_cc),
        f64::from(y_cc),
        f64::from(z_cc),
    )?;
    o.cc_tal = [a, b, c];
    let _ = writeln!(
        r.out,
        "talairach cc position changed to ({}, {}, {})",
        fmt_f(a, 0, 2),
        fmt_f(b, 0, 2),
        fmt_f(c, 0, 2)
    );
    if let Some(f) = o.log_fp.as_mut() {
        let _ = writeln!(
            f.buf,
            "CC:   {x_cc}, {y_cc}, {z_cc} (TAL: {}, {}, {})",
            fmt_f(a, 2, 1),
            fmt_f(b, 2, 1),
            fmt_f(c, 2, 1)
        );
    }
    if let Some(f) = o.alog_fp.as_mut() {
        let _ = writeln!(f.buf, "CC-CRS {x_cc} {y_cc} {z_cc}");
        let _ = writeln!(
            f.buf,
            "CC-TAL {} {} {}",
            fmt_f(a, 5, 1),
            fmt_f(b, 5, 1),
            fmt_f(c, 5, 1)
        );
    }

    let mut mri_pons: Option<Mri> = None;
    if mri_seg.is_none() {
        let mut ppons = (0i32, 0i32, 0i32);
        if o.pons_seed_vol_set {
            check_vol_seed(g, r, &mri_im, o.pons_vol, "pons")?;
            o.pons_tal = vox_to_tal(g, r, &mri_im, o.pons_vol, lta_r)?;
            let f = |v: f64| fmt_f(v, 0, 2);
            let _ = writeln!(
                r.out,
                "Using voxel pons seed point, calculated talairach pons position ({}, {}, {})",
                f(o.pons_tal[0]),
                f(o.pons_tal[1]),
                f(o.pons_tal[2])
            );
            o.pons_seed_set = true;
        } else if o.pons_seed_set {
            let _ = writeln!(
                r.out,
                "Verify whether the seed point is inside the volume first"
            );
            check_tal_seed(g, r, &mri_im, o.pons_tal, lta_r, "-PV")?;
        } else {
            let tmp = if o.cc_mask {
                let _ = writeln!(
                    r.err,
                    "masking possible pons locations using cc cutting plane"
                );
                let t = cc_mask_volumes(g, r, &mri_im, &mri_cc, &mut mri_labels)?;
                if diag_write(g) {
                    let mut t2 = mri_copy(g, r.out, r.err, &t, None)?.unwrap_new();
                    write_diag(g, r, &mut t2, "./erased.mgz")?;
                }
                Some(t)
            } else {
                None
            };
            let src = tmp.as_ref().unwrap_or(&mri_im);
            let mut t = mri_alloc(
                g,
                r.out,
                r.err,
                src.vg.width,
                src.vg.height,
                src.vg.depth,
                src.mtype,
            )?;
            mri_copy_header(g, r.out, r.err, &talheader, Some(&mut t))?;
            mri_to_talairach_ex(g, r.out, r.err, r.env, src, Some(&mut t), lta_r)?;
            if diag_write(g) {
                write_diag(g, r, &mut t, "./talponsearch.mgz")?;
            }
            binarize(g, r, &mut t, WM_THRESH, 110.0)?;
            mri_tal = t;
            let mut p = o.pons_tal;
            find_pons(g, r, &mut mri_tal, &mut p, x_cc, y_cc, z_cc, 0)?;
            o.pons_tal = p;
        }
        let [x, y, z] = o.pons_tal;
        mri_pons = find_cutting_plane(
            g,
            r,
            &mut mri_tal,
            x,
            y,
            z,
            MRI_HORIZONTAL,
            &mut ppons,
            o.pons_seed_set,
            lta_r,
            o.logging,
        )?;
        if !o.pons_seed_set && mri_pons.is_none() {
            let _ = writeln!(
                r.err,
                "initial attempt at finding brainstem failed - initiating backup plan A...."
            );
            let tmp = if o.cc_mask {
                let t = cc_mask_volumes(g, r, &mri_im, &mri_cc, &mut mri_labels)?;
                Some(t)
            } else {
                None
            };
            let mut src = match tmp {
                Some(t) => t,
                None => mri_copy(g, r.out, r.err, &mri_im, None)?.unwrap_new(),
            };
            if o.cc_mask && diag_write(g) {
                write_diag(g, r, &mut src, "./planA-erased.mgz")?;
            }
            let mut t = mri_alloc(
                g,
                r.out,
                r.err,
                src.vg.width,
                src.vg.height,
                src.vg.depth,
                src.mtype,
            )?;
            mri_copy_header(g, r.out, r.err, &talheader, Some(&mut t))?;
            mri_to_talairach_ex(g, r.out, r.err, r.env, &src, Some(&mut t), lta_r)?;
            binarize(g, r, &mut t, WM_THRESH, 110.0)?;
            mri_tal = t;
            note_backup_space(g, "the pons' backup search (plan A)");
            let mut p = o.pons_tal;
            find_pons(g, r, &mut src, &mut p, x_cc, y_cc, z_cc, 1)?;
            o.pons_tal = p;
            let [x, y, z] = o.pons_tal;
            mri_pons = find_cutting_plane(
                g,
                r,
                &mut mri_tal,
                x,
                y,
                z,
                MRI_HORIZONTAL,
                &mut ppons,
                o.pons_seed_set,
                lta_r,
                o.logging,
            )?;
        }
        if mri_pons.is_none() {
            o.pons_tal = PONS_TAL;
            note_backup_space(g, "the pons' search at Talairach (-2, -15, -17)");
            let [x, y, z] = o.pons_tal;
            mri_pons = find_cutting_plane(
                g,
                r,
                &mut mri_im,
                x,
                y,
                z,
                MRI_HORIZONTAL,
                &mut ppons,
                o.pons_seed_set,
                lta_r,
                o.logging,
            )?;
            if mri_pons.is_none() {
                log::error_exit(
                    g,
                    r.err,
                    ERROR_BADPARM,
                    &format!("{NAME}: could not find pons"),
                )?;
            }
        }
        if let Some(pons) = mri_pons.take() {
            let (xp, yp, zp) = ppons;
            let [pa, pb, pc] = o.pons_tal;
            if let Some(f) = o.log_fp.take() {
                let mut f = f;
                let _ = writeln!(
                    f.buf,
                    "PONS: {xp}, {yp}, {zp} (TAL: {}, {}, {})",
                    fmt_f(pa, 2, 1),
                    fmt_f(pb, 2, 1),
                    fmt_f(pc, 2, 1)
                );
                f.close();
            }
            if let Some(f) = o.alog_fp.take() {
                let mut f = f;
                let _ = writeln!(f.buf, "PONS-CRS {xp} {yp} {zp}");
                let _ = writeln!(
                    f.buf,
                    "PONS-TAL {} {} {}",
                    fmt_f(pa, 5, 1),
                    fmt_f(pb, 5, 1),
                    fmt_f(pc, 5, 1)
                );
                f.close();
            }
            let mut pons = pons;
            if diag_write(g) {
                write_diag(g, r, &mut pons, "./ponstal.mgz")?;
            }
            let mut p = plane_to_src(g, r, &pons, &mri_im, lta_r)?;
            mri_dilate(g, r.out, r.err, Pair::Same(&mut p))?;
            mri_pons = Some(p);
        }
    } else {
        let _ = write!(r.out, "Erasing brainstem...");
        let seg = mri_seg.as_ref().expect("read above");
        mri_set_label_values(
            g,
            r.out,
            r.err,
            Pair::Same(&mut mri_im),
            seg,
            Brain_Stem,
            0.0,
        )?;
        let _ = writeln!(r.out, "done.");
    }
    drop(mri_tal);

    // the cuts made in the image and the labels
    if mri_seg.is_none() {
        let fv = o.fill_val as f32;
        mri_mask(
            g,
            r.out,
            r.err,
            r.env,
            Pair::Same(&mut mri_im),
            &mri_cc,
            1,
            fv,
        )?;
        if let Some(p) = mri_pons.as_ref() {
            mri_mask(g, r.out, r.err, r.env, Pair::Same(&mut mri_im), p, 1, fv)?;
        }
    }
    if let Some(saved) = mri_saved_labels.as_mut() {
        mri_mask(g, r.out, r.err, r.env, Pair::Same(saved), &mri_cc, 1, 0.0)?;
        if let Some(p) = mri_pons.as_ref() {
            mri_mask(g, r.out, r.err, r.env, Pair::Same(saved), p, 1, 0.0)?;
        }
    }
    if o.fill_val != 0 {
        let _ = writeln!(
            r.err,
            "writing out image with cutting planes to 'planes.mgz'."
        );
        if write(g, r, &mut mri_im, "planes.mgz")? != 0 && unwritten(g, "planes.mgz") {
            return Err(Fatal::Exit(1));
        }
        let _ = writeln!(r.err, "done.");
        return Ok(());
    }
    if let Some(saved) = mri_saved_labels.take() {
        for &label in &FILL_LABELS {
            mri_copy_label(g, r.err, &saved, &mut mri_im, label)?;
        }
    }
    drop(mri_labels);
    drop(mri_pons);
    if g.gdiag == 0 {
        let _ = writeln!(r.err, "done.");
    }

    let voxsize = find_min_size(&mri_im);
    let seed_search_size = int64(g, (f64::from(SEED_SEARCH_SIZE) / voxsize).ceil());
    let _ = writeln!(
        r.out,
        "seed_search_size = {seed_search_size}, min_neighbors = {MIN_NEIGHBORS}"
    );
    let cc_tal = o.cc_tal;
    let lhonly = o.fill.lhonly;
    let rhonly = o.fill.rhonly;

    // the right hemisphere's seed
    let wm_rh = if o.rh_seed_set {
        if o.rhv {
            let (x, y, z) = o.rh_vol;
            o.rh_tal = vox_to_tal(
                g,
                r,
                &mri_im,
                [f64::from(x), f64::from(y), f64::from(z)],
                lta_r,
            )?;
            o.rh_vol
        } else {
            let p = tal_to_vox(g, r, &mri_im, o.rh_tal, lta_r)?;
            (nint(g, p[0]), nint(g, p[1]), nint(g, p[2]))
        }
    } else {
        let t = [
            cc_tal[0] + f64::from(2 * SEED_SEARCH_SIZE),
            cc_tal[1],
            cc_tal[2],
        ];
        let p = tal_to_vox(g, r, &mri_im, t, lta_r)?;
        if lhonly {
            (-1, -1, -1)
        } else {
            let f = |v: f64| fmt_f(v, 0, 2);
            let _ = writeln!(
                r.out,
                "search rh wm seed point around talairach space:({}, {}, {}) SRC: ({}, {}, {})",
                f(t[0]),
                f(t[1]),
                f(t[2]),
                f(p[0]),
                f(p[1]),
                f(p[2])
            );
            hemi_seed(g, r, &mri_im, p, "rh", seed_search_size, voxsize)?
        }
    };
    // the left hemisphere's seed
    let wm_lh = if o.lh_seed_set {
        if o.lhv {
            let (x, y, z) = o.lh_vol;
            o.lh_tal = vox_to_tal(
                g,
                r,
                &mri_im,
                [f64::from(x), f64::from(y), f64::from(z)],
                lta_r,
            )?;
            o.lh_vol
        } else {
            let p = tal_to_vox(g, r, &mri_im, o.lh_tal, lta_r)?;
            (nint(g, p[0]), nint(g, p[1]), nint(g, p[2]))
        }
    } else {
        let t = [
            cc_tal[0] - f64::from(2 * SEED_SEARCH_SIZE),
            cc_tal[1],
            cc_tal[2],
        ];
        let p = tal_to_vox(g, r, &mri_im, t, lta_r)?;
        let f = |v: f64| fmt_f(v, 0, 2);
        let _ = writeln!(
            r.out,
            "search lh wm seed point around talairach space ({}, {}, {}), SRC: ({}, {}, {})",
            f(t[0]),
            f(t[1]),
            f(t[2]),
            f(p[0]),
            f(p[1]),
            f(p[2])
        );
        if rhonly {
            (-1, -1, -1)
        } else {
            hemi_seed(g, r, &mri_im, p, "lh", seed_search_size, voxsize)?
        }
    };
    let (w, h, d) = (mri_im.vg.width, mri_im.vg.height, mri_im.vg.depth);
    let out_of =
        |s: (i32, i32, i32)| s.0 < 0 || s.0 >= w || s.1 < 0 || s.1 >= h || s.2 < 0 || s.2 >= d;
    if !lhonly && out_of(wm_rh) {
        log::error_exit(
            g,
            r.err,
            ERROR_BADPARM,
            &format!(
                "rh white matter seed point out of bounds ({}, {}, {})\n",
                wm_rh.0, wm_rh.1, wm_rh.2
            ),
        )?;
    }
    if !rhonly && out_of(wm_lh) {
        log::error_exit(
            g,
            r.err,
            ERROR_BADPARM,
            &format!(
                "lh white matter seed point out of bounds ({}, {}, {})\n",
                wm_lh.0, wm_lh.1, wm_lh.2
            ),
        )?;
    }
    drop(mri_cc);

    let mut mri_fill =
        if let (Some(_), Some(seg)) = (o.segmentation_fname.as_ref(), mri_seg.as_ref()) {
            let _ = writeln!(r.out, "compute mri_fill using aseg");
            let s = o.fill.clone();
            fill_with_aseg(g, r, &s, &mut mri_im, seg)?
        } else {
            let mut lh_fill = mri_clone(g, r.out, r.err, &mri_im, None)?.unwrap_new();
            let mut rh_fill = mri_clone(g, r.out, r.err, &mri_im, None)?.unwrap_new();
            let mut lh_im = mri_copy(g, r.out, r.err, &mri_im, None)?.unwrap_new();
            let mut rh_im = mri_copy(g, r.out, r.err, &mri_im, None)?.unwrap_new();
            if diag_write(g) {
                write_diag(g, r, &mut mri_im, "fill0.mgz")?;
            }
            let mut s = o.fill.clone();
            let (lhv, rhv) = (s.lh_fill_val, s.rh_fill_val);
            if !rhonly {
                let _ = writeln!(r.err, "filling left hemisphere...");
                mri_fill_volume(
                    g,
                    r,
                    &mut s,
                    &mut lh_fill,
                    &mut lh_im,
                    wm_lh.0,
                    wm_lh.1,
                    wm_lh.2,
                    lhv,
                )?;
            }
            if !lhonly {
                let _ = writeln!(r.err, "filling right hemisphere...");
                mri_fill_volume(
                    g,
                    r,
                    &mut s,
                    &mut rh_fill,
                    &mut rh_im,
                    wm_rh.0,
                    wm_rh.1,
                    wm_rh.2,
                    rhv,
                )?;
            }
            drop(lh_im);
            drop(rh_im);
            let _ = writeln!(
                r.err,
                "filling degenerate left hemisphere surface locations..."
            );
            mri_fill_degenerate_locations(g, r, &mut lh_fill, lhv)?;
            let _ = writeln!(
                r.err,
                "filling degenerate right hemisphere surface locations..."
            );
            mri_fill_degenerate_locations(g, r, &mut rh_fill, rhv)?;
            let mut lh_im = mri_copy(g, r.out, r.err, &lh_fill, None)?.unwrap_new();
            let mut rh_im = mri_copy(g, r.out, r.err, &rh_fill, None)?.unwrap_new();
            let _ = writeln!(r.err, "refilling left hemisphere...");
            if refill(g, rhonly, "left") {
                mri_fill_volume(
                    g,
                    r,
                    &mut s,
                    &mut lh_fill,
                    &mut lh_im,
                    wm_lh.0,
                    wm_lh.1,
                    wm_lh.2,
                    lhv,
                )?;
            }
            let _ = writeln!(r.err, "refilling right hemisphere...");
            if refill(g, lhonly, "right") {
                mri_fill_volume(
                    g,
                    r,
                    &mut s,
                    &mut rh_fill,
                    &mut rh_im,
                    wm_rh.0,
                    wm_rh.1,
                    wm_rh.2,
                    rhv,
                )?;
            }
            drop(lh_im);
            drop(rh_im);
            let _ = writeln!(r.err, "combining hemispheres...");
            let pl = [f64::from(wm_lh.0), f64::from(wm_lh.1), f64::from(wm_lh.2)];
            let pr = [f64::from(wm_rh.0), f64::from(wm_rh.1), f64::from(wm_rh.2)];
            vox_to_tal(g, r, &lh_fill, pl, lta_r)?;
            vox_to_tal(g, r, &rh_fill, pr, lta_r)?;
            let m = mri_combine_hemispheres(g, r, &mut lh_fill, &mut rh_fill, wm_lh, wm_rh)?;
            if diag_write(g) {
                write_diag(g, r, &mut lh_fill, "./lh_fill.mgz")?;
                write_diag(g, r, &mut rh_fill, "./rh_fill.mgz")?;
            }
            m
        };
    drop(mri_im);

    if let Some(atlas) = o.atlas_name.clone() {
        atlas_not_ported(g, r, &atlas, o.sname.as_deref())?;
    }

    if o.topofix {
        let mut mri_lh = mri_clone(g, r.out, r.err, &mri_fill, None)?.unwrap_new();
        let mut mri_rh = mri_clone(g, r.out, r.err, &mri_fill, None)?.unwrap_new();
        let (lhv, rhv) = (o.fill.lh_fill_val, o.fill.rh_fill_val);
        mri_copy_label(g, r.err, &mri_fill, &mut mri_lh, lhv)?;
        mri_copy_label(g, r.err, &mri_fill, &mut mri_rh, rhv)?;
        let norm = match o.norm_fname.clone() {
            Some(n) => read(g, r, &n)?,
            None => {
                // MRIread(NULL): FreeSurfer crashes
                g.notices.add(
                    &FS_1292,
                    "mri_fill: option '-topofix' without its 1 value(s): FreeSurfer reads past its arguments and crashes",
                );
                return Err(Fatal::Exit(if g.notices.reproduce() {
                    SEGFAULT
                } else {
                    1
                }));
            }
        };
        match norm {
            None => {
                let _ = writeln!(
                    r.err,
                    "could not read volume {}",
                    o.norm_fname.as_deref().unwrap_or("(null)")
                );
            }
            Some(norm) => {
                let mut counters = Counters::default();
                mri_topofix(g, r.out, r.err, &mut mri_rh, &norm, rhv, lhv, &mut counters)?;
                mri_topofix(g, r.out, r.err, &mut mri_lh, &norm, rhv, lhv, &mut counters)?;
                drop(norm);
                mri_copy_label(g, r.err, &mri_rh, &mut mri_lh, rhv)?;
                mri_fill = mri_lh;
            }
        }
    }

    if let Some(vec) = o.automandiffvec.take() {
        let _ = writeln!(r.out, "DoAutoMan {}", i32::from(o.do_auto_man));
        if !vec.is_empty() && o.do_auto_man {
            let _ = writeln!(r.out, "Applying edits to the output");
            let mut napp = 0;
            let res = mri_apply_diff_vect(
                g,
                r.out,
                r.err,
                &mut mri_fill,
                None,
                &vec,
                &mut napp,
                DiffDst::InPlace,
            )?;
            if res.is_null() {
                return Err(Fatal::Exit(1));
            }
            let _ = writeln!(r.out, "Applied {napp} edits");
        } else {
            let _ = writeln!(r.out, "No edits to apply");
        }
        if o.do_auto_man {
            let ef = o.automaneditsfile.clone().unwrap_or_default();
            if ef != "nofile" {
                let _ = writeln!(r.out, "Writing edits to {ef}");
                let path = resolve(r.dir, &ef);
                let mut text = String::new();
                for v in &vec {
                    let c = int64(g, v[0]);
                    let rr = int64(g, v[1]);
                    let s = int64(g, v[2]);
                    let a = format_double("%g", v[3]).unwrap_or_default();
                    let b = format_double("%g", v[4]).unwrap_or_default();
                    text.push_str(&format!("{c:3} {rr:3} {s:3} {a} {b}\n"));
                }
                if std::fs::write(&path, text.as_bytes()).is_err() {
                    let _ = writeln!(r.out, "ERROR: opening {ef}");
                    return Err(Fatal::Exit(1));
                }
            }
        }
    }
    if let Some(ct) = o.ctab.take() {
        let _ = writeln!(r.out, "Embedding colortable");
        mri_fill.ct = Some(ct);
    }

    let _ = writeln!(r.out, "mri_fill done, writing output to {out_fname}...");
    let failed = write(g, r, &mut mri_fill, &out_fname)? != 0 && unwritten(g, &out_fname);
    let msec = then.elapsed().as_millis() as i32;
    let minutes = msec as f32 / (60.0 * 1000.0f32);
    let _ = writeln!(
        r.err,
        "filling took {} minutes",
        fmt_f(f64::from(minutes), 2, 1)
    );

    if lta_r.is_some() && !lhonly && !rhonly {
        let voxvol = f64::from(mri_fill.vg.xsize * mri_fill.vg.ysize * mri_fill.vg.zsize);
        let (bad_rh, bad_lh, tot_rh, tot_lh) = verify_lr_split(
            g,
            r.out,
            r.err,
            &mri_fill,
            lta_r,
            cc_tal[0],
            o.fill.rh_fill_val,
            o.fill.lh_fill_val,
        )?;
        if f64::from(bad_rh) * voxvol > 10000.0 || f64::from(bad_lh) * voxvol > 10000.0 {
            let _ = writeln!(r.err, "badRH = {bad_rh}/{tot_rh}, badLH={bad_lh}/{tot_lh}");
            g.errno = 0;
            log::error_printf(
                g,
                r.err,
                ERROR_BADPARM,
                "Please check filled volume.  Cerebellum may be included.\n",
            );
        }
    }
    if failed {
        return Err(Fatal::Exit(1));
    }
    Ok(())
}

/// FS-1301: whether the refill of a hemisphere runs. FreeSurfer refills both, also the one
/// `-lhonly` or `-rhonly` left out, whose seed (-1, -1, -1) is outside the volume (it crashes);
/// by default that one is not refilled.
fn refill(g: &mut Globals, other_only: bool, side: &str) -> bool {
    if !other_only {
        return true;
    }
    g.notices.add(
        &FS_1301,
        if g.notices.reproduce() {
            format!(
                "mri_fill -lhonly/-rhonly without a segmentation refills the {side} hemisphere it left out, from a seed outside the volume, as FreeSurfer (which crashes)"
            )
        } else {
            format!(
                "mri_fill -lhonly/-rhonly without a segmentation: the {side} hemisphere it left out is not refilled (FreeSurfer refills it from a seed outside the volume and crashes)"
            )
        },
    );
    g.notices.reproduce()
}

/// `-atlas`'s ventricle filling after the fill (`MRI3DreadSmall`, `mriReadBinaryProbabilities`,
/// `MRIfillVentricle`, ...): not ported yet: FreeSurfer's checks of the environment and of the
/// morph file (`<SUBJECTS_DIR>/<subject>/mri/transforms/<atlas>.m3d`: it must open and start with
/// the old 3D morph's magic number), then a clear message and status 1. No recorded step reaches
/// it.
fn atlas_not_ported(
    g: &mut Globals,
    r: &mut Run,
    atlas: &str,
    sname: Option<&str>,
) -> Result<(), Fatal> {
    if r.env.get("FREESURFER_HOME").is_none() {
        log::error_exit(
            g,
            r.err,
            ERROR_BADPARM,
            &format!("{NAME}: could not read FREESURFER_HOME from environment"),
        )?;
    }
    let Some(sd) = r.env.get("SUBJECTS_DIR") else {
        log::error_exit(
            g,
            r.err,
            ERROR_BADPARM,
            &format!("{NAME}: could not read SUBJECTS_DIR from environment"),
        )?;
        return Ok(());
    };
    let sd = sd.to_string_lossy().into_owned();
    let fname = format!(
        "{sd}/{}/mri/transforms/{atlas}.m3d",
        sname.unwrap_or("(null)")
    );
    // MRI3DreadSmall's checks: the file opens and starts with the old 3D morph's magic number
    let bytes = match std::fs::read(resolve(r.dir, &fname)) {
        Ok(b) => b,
        Err(e) => {
            g.errno = errno_of(&e).unwrap_or(0);
            log::error_printf(
                g,
                r.err,
                ERROR_NOFILE,
                &format!("MRI3DreadSmall: could not open file {fname}"),
            );
            log::error_exit(
                g,
                r.err,
                ERROR_NOFILE,
                &format!("{NAME}: could not open transform file {fname}\n"),
            )?;
            return Ok(());
        }
    };
    let magic = bytes
        .get(..4)
        .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]));
    if magic != Some(M3D_MAGIC) {
        log::error_printf(
            g,
            r.err,
            ERROR_BADFILE,
            &format!("file {fname} not an old 3d morph file.\nTry a new 3d morph read routine.\n"),
        );
        log::error_exit(
            g,
            r.err,
            ERROR_NOFILE,
            &format!("{NAME}: could not open transform file {fname}\n"),
        )?;
        return Ok(());
    }
    let _ = writeln!(
        r.err,
        "{NAME}: -atlas (filling ventricles using atlas {atlas} with {fname}) is not ported yet"
    );
    Err(Fatal::Exit(1))
}

/// `M3D_MAGIC` (`include/mrimorph.h`): an old 3D morph file's first four bytes (big-endian).
const M3D_MAGIC: u32 = 0xabcd_ef42;
