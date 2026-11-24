// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_edit_wm_with_aseg`'s program (`mri_edit_wm_with_aseg/mri_edit_wm_with_aseg.cpp` 104-438):
//! the entry point [`run`] (registered as `mri_edit_wm_with_aseg` in the crate's `PROGRAMS`),
//! [`main`] and the options ([`get_option`]).
//!
//! `mri_edit_wm_with_aseg [options] <wm> <T1/brain> <aseg> <out>` (recon-all: `-keep-in wm.seg.mgz
//! brain.mgz aseg.presurf.mgz wm.asegedit.mgz`; v8 adds `-fix-ento-wm entowm.mgz 3 255 255
//! -fix-acj aseg.presurf.mgz 255 255 -fill-seg-wm -fix-scm-ha 1`, and the stand-alone
//! `-sa-fix-ento-wm` and `-sa-fix-acj` on `brain.finalsurfs.mgz` and `wm.mgz`). The version
//! options first; the command line on standard output; the options while the next argument starts
//! with `-` (compared without case but `fillven`; then by the first letter: `?`, `h`, `u` the help
//! with status 0, anything else `unknown option <arg>` on standard error, status 1); fewer than four
//! arguments left: the help, status 1.
//!
//! The white matter read (bytes only: `ERROR_UNSUPPORTED` otherwise), the T1 changed to bytes, the
//! segmentation read; the paths from the hippocampus to cortex closed ([`remove_paths_to_cortex`]),
//! the white matter edited from the labels ([`edit_segmentation`]), the cortex above the medial
//! temporal lobe spackled; `-fix-scm-ha`'s hippocampus and amygdala, the edits kept, one
//! hemisphere's labels cleared (`-lh`, `-rh`), the entorhinal white matter and the
//! amygdala-cortical junction filled, `-keep-hailvcp`'s structures set, `-wmsa`'s lesions filled;
//! the output written; `auto filling took <m> minutes` on standard error; status 0.
//!
//! Defects (`larmorx_core::defects`; the mechanisms in
//! `docs/findings/freesurfer-mri-edit-wm-with-aseg.md`): FS-1234 (the path search's byte
//! pointers, kept), FS-1236 (voxels outside the volume), FS-1237 (a diagonal neighbour's wrong
//! voxel), FS-1238 (`-keep-hailvcp`'s values), FS-1239 (an option without its values), FS-1240 (an
//! unwritten output).

use std::io::Write;

use larmorx_core::defects::{FS_1238, FS_1239, FS_1240};
use larmorx_freesurfer_core::cstd::{atoi, format_double};
use larmorx_freesurfer_core::diag::diag_init;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::help_xml::output_help_xml;
use larmorx_freesurfer_core::log::{self, ERROR_NOFILE, ERROR_UNSUPPORTED, Fatal};
use larmorx_freesurfer_core::run::{Environment, Settings};
use larmorx_freesurfer_core::system;
use larmorx_freesurfer_core::timer::Timer;
use larmorx_freesurfer_core::utils::stricmp;
use larmorx_freesurfer_core::version::{get_all_info, handle_version_option};
use larmorx_freesurfer_io::mriio::{mri_add_command_line, mri_read_volume, mri_write};
use larmorx_freesurfer_mri::mri::{MGZ_INTENT_UNKNOWN, MRI_UCHAR, Mri};
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};
use larmorx_freesurfer_volume::cma::{is_lh_class, is_rh_class};
use larmorx_freesurfer_volume::morph::fix_scm;
use larmorx_freesurfer_volume::mri2ops::{label_amygala_cortal_junction, mri_fix_ento_wm};
use larmorx_freesurfer_volume::mriset::mri_copy_label;
use larmorx_freesurfer_volume::resize::mri_change_type;

use super::edit::edit_segmentation;
use super::local::{Seg, int, keep_hailvcp, spackle_wm_superior_to_mtl};
use super::paths::{Ctx, remove_paths_to_cortex};
use crate::mri_mask::main::Cout;
use crate::mri_segstats::options::{scan_d, scan_lf};

/// The program's name as `argv[0]` gives it (recon-all runs it by name).
pub const NAME: &str = "mri_edit_wm_with_aseg";

/// `mri_edit_wm_with_aseg.help.xml`, the text `usage_exit` renders (`outputHelpXml`).
pub const HELP_XML: &[u8] = include_bytes!("mri_edit_wm_with_aseg.help.xml");

/// The status of a process ended by `SIGSEGV`, as a shell reports it.
const SEGFAULT: i32 = 139;
/// `WM_EDITED_ON_VAL`, `WM_EDITED_OFF_VAL` (`include/mri.h`).
const WM_EDITED_ON_VAL: i32 = 255;
const WM_EDITED_OFF_VAL: i32 = 1;

/// The settings the options give (the file-scope variables of `mri_edit_wm_with_aseg.cpp`).
#[derive(Debug, Default)]
pub struct Options {
    pub lh_only: bool,
    pub rh_only: bool,
    pub fillven: i32,
    pub keep_edits: bool,
    pub keep_edits_input: bool,
    pub fcd: bool,
    pub fix_scm_ha: bool,
    pub fix_scm_ha_ndil: i32,
    pub wmsafile: Option<String>,
    pub ento_lh: f64,
    pub ento_rh: f64,
    pub ento_level: i32,
    pub entowm: Option<Mri>,
    pub acj_lh: f64,
    pub acj_rh: f64,
    pub acj: Option<Mri>,
    pub keep: (bool, bool, bool, bool),
    pub hilvcp_lh: f64,
    pub hilvcp_rh: f64,
    pub fill_seg_wm: bool,
}

impl Options {
    fn new() -> Options {
        Options {
            fillven: 1,
            ..Options::default()
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

/// `mri_edit_wm_with_aseg`'s entry point: its original arguments (without the program's name),
/// the settings, the two streams; the exit status. The notices of the run follow on standard
/// error.
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

fn pg(x: f64) -> String {
    format_double("%g", x).unwrap_or_default()
}

fn read(g: &mut Globals, r: &mut Run, fname: &str) -> Result<Option<Mri>, Fatal> {
    mri_read_volume(g, r.out, r.err, r.env, r.dir, fname)
}

/// `MRIwrite(mri, fname)`: FreeSurfer's error code (0 when written).
fn write(g: &mut Globals, r: &mut Run, mri: &mut Mri, fname: &str) -> Result<i32, Fatal> {
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
    )
}

/// `exit(MRIwrite(...))` of the stand-alone options: the code as a status (its low byte).
fn exit_with(rc: i32) -> Fatal {
    Fatal::Exit(rc & 0xff)
}

/// FS-1239: an option whose `n`-th value (from 1) is missing: FreeSurfer passes the missing
/// argument (a null pointer) on and crashes (`reproduce`, after the values it has); by default a
/// clear error and the help, status 1, before anything is done.
fn missing(g: &mut Globals, r: &mut Run, arg: &str, n: usize) -> Fatal {
    g.notices.add(
        &FS_1239,
        format!("mri_edit_wm_with_aseg: option '{arg}' without its {n} value(s): FreeSurfer reads past its arguments and crashes"),
    );
    if g.notices.reproduce() {
        return Fatal::Exit(SEGFAULT);
    }
    let _ = writeln!(r.err, "{NAME}: option {arg} needs {n} value(s)");
    usage_exit(g, r, 1)
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
    // FS-1239: the values an option reads, and how many come before the first use of a missing
    // one (FreeSurfer's reads of the present ones happen first)
    let need = |n: usize| rest.len() > n;
    if is(g, "dilate") {
        return Ok(0);
    }
    if is(g, "-help") || is(g, "-usage") {
        return Err(usage_exit(g, r, 0));
    }
    if option == "fillven" {
        if !need(1) {
            return Err(missing(g, r, &arg, 1));
        }
        o.fillven = atoi(rest[1].as_bytes());
        let _ = writeln!(
            r.out,
            "{}filling ventricles",
            if o.fillven == 0 { "not " } else { "" }
        );
        return Ok(1);
    }
    if is(g, "keep") {
        o.keep_edits = true;
        let _ = writeln!(r.err, "preserving editing changes in output volume...");
        return Ok(0);
    }
    if is(g, "fcd") {
        o.fcd = true;
        let _ = writeln!(
            r.err,
            "preserving focal cortical dysplasias - not filling non-wm lesions"
        );
        return Ok(0);
    }
    if is(g, "lh") {
        o.lh_only = true;
        let _ = writeln!(r.err, "assuming input is only lh");
        return Ok(0);
    }
    if is(g, "rh") {
        o.rh_only = true;
        let _ = writeln!(r.err, "assuming input is only rh");
        return Ok(0);
    }
    if is(g, "keep-in") {
        o.keep_edits = true;
        o.keep_edits_input = true;
        let _ = writeln!(r.err, "preserving editing changes in input volume...");
        return Ok(0);
    }
    if is(g, "fix-scm-ha") {
        if !need(1) {
            return Err(missing(g, r, &arg, 1));
        }
        o.fix_scm_ha = true;
        o.fix_scm_ha_ndil = atoi(rest[1].as_bytes());
        let _ = writeln!(r.out, "FixSCM HA {}", o.fix_scm_ha_ndil);
        return Ok(1);
    }
    if is(g, "fix-scm-ha-only") {
        // mri_edit_wm_with_aseg -fix-scm-ha-only aseg.presurf.mgz SCM.mgz ndil out.mgz
        if !g.notices.reproduce() && !need(4) {
            return Err(missing(g, r, &arg, 4));
        }
        if !need(1) {
            return Err(missing(g, r, &arg, 4));
        }
        let Some(aseg) = read(g, r, &rest[1])? else {
            return Err(Fatal::Exit(1));
        };
        if !need(2) {
            return Err(missing(g, r, &arg, 4));
        }
        let Some(mut scm) = read(g, r, &rest[2])? else {
            return Err(Fatal::Exit(1));
        };
        if !need(3) {
            return Err(missing(g, r, &arg, 4));
        }
        let ndil = atoi(rest[3].as_bytes());
        let _ = fix_scm(g, r.out, r.err, &mut scm, &aseg, ndil)?;
        if !need(4) {
            return Err(missing(g, r, &arg, 4));
        }
        let rc = write(g, r, &mut scm, &rest[4])?;
        return Err(exit_with(rc));
    }
    let sa_ento = is(g, "sa-fix-ento-wm");
    if is(g, "fix-ento-wm") || sa_ento {
        let n = if sa_ento { 6 } else { 4 };
        if !g.notices.reproduce() && !need(n) {
            return Err(missing(g, r, &arg, n));
        }
        if !need(1) {
            return Err(missing(g, r, &arg, n));
        }
        let Some(e) = read(g, r, &rest[1])? else {
            return Err(Fatal::Exit(1));
        };
        o.entowm = Some(e);
        if !need(4) {
            return Err(missing(g, r, &arg, n));
        }
        if let Some(v) = scan_d(g, &rest[2]) {
            o.ento_level = v;
        }
        if let Some(v) = scan_lf(g, &rest[3]) {
            o.ento_lh = v;
        }
        if let Some(v) = scan_lf(g, &rest[4]) {
            o.ento_rh = v;
        }
        let _ = writeln!(
            r.out,
            "Fixing entowm {} {} {}",
            o.ento_level,
            pg(o.ento_lh),
            pg(o.ento_rh)
        );
        if sa_ento {
            if !need(5) {
                return Err(missing(g, r, &arg, n));
            }
            let Some(mut invol) = read(g, r, &rest[5])? else {
                return Err(Fatal::Exit(1));
            };
            let e = o.entowm.as_ref().expect("read above");
            mri_fix_ento_wm(
                g,
                r.out,
                r.err,
                &mut invol,
                e,
                o.ento_level,
                o.ento_lh,
                o.ento_rh,
                false,
            )?;
            if !need(6) {
                return Err(missing(g, r, &arg, n));
            }
            let rc = write(g, r, &mut invol, &rest[6])?;
            return Err(exit_with(rc));
        }
        return Ok(4);
    }
    if is(g, "wmsa") {
        // a missing name is FreeSurfer's null pointer, which it then skips
        o.wmsafile = rest.get(1).cloned();
        return Ok(1);
    }
    if is(g, "label-acj") {
        if !g.notices.reproduce() && !need(2) {
            return Err(missing(g, r, &arg, 2));
        }
        if !need(1) {
            return Err(missing(g, r, &arg, 2));
        }
        let Some(seg) = read(g, r, &rest[1])? else {
            return Err(Fatal::Exit(1));
        };
        let env = r.env.clone();
        let Some(mut acj) =
            label_amygala_cortal_junction(g, r.out, r.err, &env, &seg, 3, None)?.into_new()
        else {
            return Err(Fatal::Exit(1));
        };
        if !need(2) {
            return Err(missing(g, r, &arg, 2));
        }
        let rc = write(g, r, &mut acj, &rest[2])?;
        return Err(exit_with(rc));
    }
    let sa_acj = is(g, "sa-fix-acj");
    if is(g, "fix-acj") || sa_acj {
        let n = if sa_acj { 5 } else { 3 };
        if !g.notices.reproduce() && !need(n) {
            return Err(missing(g, r, &arg, n));
        }
        if !need(1) {
            return Err(missing(g, r, &arg, n));
        }
        let Some(seg) = read(g, r, &rest[1])? else {
            return Err(Fatal::Exit(1));
        };
        let env = r.env.clone();
        let Some(acj) =
            label_amygala_cortal_junction(g, r.out, r.err, &env, &seg, 3, None)?.into_new()
        else {
            return Err(Fatal::Exit(1));
        };
        drop(seg);
        o.acj = Some(acj);
        if !need(3) {
            return Err(missing(g, r, &arg, n));
        }
        if let Some(v) = scan_lf(g, &rest[2]) {
            o.acj_lh = v;
        }
        if let Some(v) = scan_lf(g, &rest[3]) {
            o.acj_rh = v;
        }
        let _ = writeln!(r.out, "Fixing ACJ {} {}", pg(o.acj_lh), pg(o.acj_rh));
        if sa_acj {
            if !need(4) {
                return Err(missing(g, r, &arg, n));
            }
            let Some(mut invol) = read(g, r, &rest[4])? else {
                return Err(Fatal::Exit(1));
            };
            let a = o.acj.as_ref().expect("made above");
            mri_fix_ento_wm(g, r.out, r.err, &mut invol, a, 3, o.acj_lh, o.acj_rh, true)?;
            if !need(5) {
                return Err(missing(g, r, &arg, n));
            }
            let rc = write(g, r, &mut invol, &rest[5])?;
            return Err(exit_with(rc));
        }
        return Ok(3);
    }
    if is(g, "fill-seg-wm") {
        o.fill_seg_wm = true;
        return Ok(0);
    }
    if is(g, "no-fill-seg-wm") {
        o.fill_seg_wm = false;
        return Ok(0);
    }
    if is(g, "keep-hailvcp") {
        if !need(2) {
            return Err(missing(g, r, &arg, 2));
        }
        if let Some(v) = scan_lf(g, &rest[1]) {
            o.hilvcp_lh = v;
        }
        if let Some(v) = scan_lf(g, &rest[2]) {
            o.hilvcp_rh = v;
        }
        o.keep = (true, true, true, true);
        // FS-1238: FreeSurfer reads the two values but does not take them (0 arguments)
        g.notices.add(
            &FS_1238,
            if g.notices.reproduce() {
                "mri_edit_wm_with_aseg -keep-hailvcp: its two values left among the arguments, as in FreeSurfer"
            } else {
                "mri_edit_wm_with_aseg -keep-hailvcp: its two values taken (FreeSurfer leaves them among the arguments)"
            },
        );
        return Ok(if g.notices.reproduce() { 0 } else { 2 });
    }
    if is(g, "debug_voxel") {
        if !need(3) {
            return Err(missing(g, r, &arg, 3));
        }
        g.gx = atoi(rest[1].as_bytes());
        g.gy = atoi(rest[2].as_bytes());
        g.gz = atoi(rest[3].as_bytes());
        let _ = writeln!(r.out, "debugging voxel ({}, {}, {})", g.gx, g.gy, g.gz);
        return Ok(3);
    }
    match option.bytes().next().map(|c| c.to_ascii_uppercase()) {
        Some(b'?') | Some(b'H') | Some(b'U') => Err(usage_exit(g, r, 0)),
        _ => {
            let _ = writeln!(r.err, "unknown option {arg}");
            Err(Fatal::Exit(1))
        }
    }
}

/// `main(argc, argv)` on the arguments after the program's name.
pub fn main(g: &mut Globals, r: &mut Run, args: &[String]) -> Result<(), Fatal> {
    let argv: Vec<String> = std::iter::once(NAME.to_owned())
        .chain(args.iter().cloned())
        .collect();
    let cmdline = get_all_info(g, r.env, &argv, NAME);
    let (nargs, argv) = handle_version_option(g, r.env, r.out, &argv, NAME);
    if nargs != 0 && argv.len() == 1 {
        return Err(Fatal::Exit(0));
    }
    for a in &argv {
        let _ = write!(r.out, "{a} ");
    }
    let _ = writeln!(r.out);
    let _ = r.out.flush();
    let then = Timer::new();
    diag_init(g, r.env, r.out);
    g.progname = NAME.to_owned();
    let mut o = Options::new();
    let mut argc = argv.len() as isize;
    let mut at = 1usize;
    while argc > 1 && argv.get(at).is_some_and(|a| a.starts_with('-')) {
        let nargs = get_option(g, r, &mut o, &argv[at..])?;
        argc -= nargs as isize + 1;
        at += nargs + 1;
    }
    if argc < 5 {
        return Err(usage_exit(g, r, 1));
    }
    let input_file_name = argv[at].clone();
    let output_file_name = argv[at + 3].clone();
    let _ = writeln!(r.out, "reading wm segmentation from {input_file_name}...");
    let Some(mut wm) = read(g, r, &input_file_name)? else {
        log::error_exit(
            g,
            r.err,
            ERROR_NOFILE,
            &format!("{NAME}: could not read wm volume from {input_file_name}"),
        )?;
        unreachable!("ErrorExit ends the program")
    };
    if wm.mtype != MRI_UCHAR {
        log::error_exit(
            g,
            r.err,
            ERROR_UNSUPPORTED,
            &format!("{NAME}: volume {input_file_name} must be MRI_UCHAR"),
        )?;
    }
    mri_add_command_line(g, r.err, &mut wm, cmdline.as_bytes());
    let t1_name = argv[at + 1].clone();
    let Some(mut t1) = read(g, r, &t1_name)? else {
        log::error_exit(
            g,
            r.err,
            ERROR_NOFILE,
            &format!("{NAME}: could not read T1/brain volume from {t1_name}"),
        )?;
        unreachable!("ErrorExit ends the program")
    };
    if t1.mtype != MRI_UCHAR {
        let Some(m) = mri_change_type(g, r.out, r.err, r.env, &t1, MRI_UCHAR, 0.0, 1.0, 1)? else {
            // FreeSurfer goes on with no volume and crashes
            return Err(Fatal::Exit(SEGFAULT));
        };
        t1 = m;
    }
    let aseg_name = argv[at + 2].clone();
    let Some(aseg) = read(g, r, &aseg_name)? else {
        log::error_exit(
            g,
            r.err,
            ERROR_NOFILE,
            &format!("{NAME}: could not read aseg volume from {aseg_name}"),
        )?;
        unreachable!("ErrorExit ends the program")
    };
    let seg = Seg::new(g, r.err, &aseg)?;

    let ctx = Ctx {
        env: r.env,
        dir: r.dir,
    };
    remove_paths_to_cortex(g, r.out, r.err, &ctx, &mut wm, &t1, &seg)?;
    edit_segmentation(
        g,
        r.out,
        r.err,
        &mut wm,
        &t1,
        &seg,
        o.fcd,
        o.fillven,
        o.fill_seg_wm,
    )?;
    spackle_wm_superior_to_mtl(g, r.err, &mut wm, &seg)?;

    if o.fix_scm_ha {
        let _ = fix_scm(g, r.out, r.err, &mut wm, &aseg, o.fix_scm_ha_ndil)?;
    }
    if o.keep_edits {
        let edits_file_name = if o.keep_edits_input {
            input_file_name.clone()
        } else {
            output_file_name.clone()
        };
        let _ = writeln!(
            r.out,
            "propagating editing to output volume from {edits_file_name}"
        );
        match read(g, r, &edits_file_name)? {
            None => {
                log::error_printf(
                    g,
                    r.err,
                    ERROR_NOFILE,
                    &format!("{NAME}: could not read file {edits_file_name} to preserve edits"),
                );
                return Err(Fatal::Exit(1));
            }
            Some(old) => {
                mri_copy_label(g, r.err, &old, &mut wm, WM_EDITED_ON_VAL)?;
                mri_copy_label(g, r.err, &old, &mut wm, WM_EDITED_OFF_VAL)?;
            }
        }
    }
    if o.lh_only || o.rh_only {
        for x in 0..wm.vg.width {
            for y in 0..wm.vg.height {
                for z in 0..wm.vg.depth {
                    let label = seg.label(g, r.err, x, y, z)?;
                    if (o.lh_only && is_rh_class(label)) || (o.rh_only && is_lh_class(label)) {
                        mri_set_vox_val(g, r.err, &mut wm, x, y, z, 0, 0.0, 0)?;
                    }
                }
            }
        }
    }
    if let Some(e) = o.entowm.as_ref() {
        mri_fix_ento_wm(
            g,
            r.out,
            r.err,
            &mut wm,
            e,
            o.ento_level,
            o.ento_lh,
            o.ento_rh,
            false,
        )?;
    }
    if let Some(a) = o.acj.as_ref() {
        mri_fix_ento_wm(g, r.out, r.err, &mut wm, a, 3, o.acj_lh, o.acj_rh, true)?;
    }
    if o.keep != (false, false, false, false) {
        keep_hailvcp(g, r.err, &mut wm, &seg, o.keep, o.hilvcp_lh, o.hilvcp_rh)?;
    }
    if let Some(name) = o.wmsafile.clone() {
        let _ = writeln!(r.out, "Applying WMSA edits from {name}");
        let Some(wmsa) = read(g, r, &name)? else {
            return Err(Fatal::Exit(1));
        };
        for c in 0..wm.vg.width {
            for rr in 0..wm.vg.height {
                for s in 0..wm.vg.depth {
                    let v = mri_get_vox_val(g, r.err, &wmsa, c, rr, s, 0, 0)?;
                    let m = int(g, v);
                    if m != 77 && m != 78 && m != 79 && m != 99 {
                        continue;
                    }
                    mri_set_vox_val(g, r.err, &mut wm, c, rr, s, 0, 250.0, 0)?;
                }
            }
        }
    }

    let _ = writeln!(r.out, "writing edited volume to {output_file_name}....");
    let rc = write(g, r, &mut wm, &output_file_name)?;
    let msec = then.milliseconds();
    let _ = writeln!(
        r.err,
        "auto filling took {} minutes",
        larmorx_freesurfer_core::cstd::fmt_f(f64::from(msec as f32 / (1000.0f32 * 60.0f32)), 2, 2)
    );
    if rc != 0 {
        g.notices.add(
            &FS_1240,
            if g.notices.reproduce() {
                format!("mri_edit_wm_with_aseg: '{output_file_name}' was not written; status 0, as in FreeSurfer")
            } else {
                format!("mri_edit_wm_with_aseg: '{output_file_name}' was not written: status 1 (FreeSurfer ends with 0)")
            },
        );
        if !g.notices.reproduce() {
            return Err(Fatal::Exit(1));
        }
    }
    Ok(())
}
