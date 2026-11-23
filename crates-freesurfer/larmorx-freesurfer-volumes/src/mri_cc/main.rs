// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_cc`'s program (`mri_cc/mri_cc.cpp` 162-762 and 1432-1555): the entry point [`run`]
//! (registered as `mri_cc` in the crate's `PROGRAMS`), [`main`] and the options ([`get_option`]).
//!
//! `mri_cc [options] <subject>`. The version options first (only those: status 0); the options
//! while the next argument starts with `-` (their names after the dash compared without case:
//! `-sdir d`, `--help`/`--usage` (the help, status 1), `-DEBUG_VOXEL x y z`, `-aseg f`, `-norm f`,
//! `-lta f`, `-force`, `-lh`, `-rh`; then by the first letter: `m` the rotation search's maximum
//! in degrees, `?`/`h`/`u` the help (status 1), `f` the fornix kept, `d` the divisions (at most
//! 50), `s` the voxels skipped in the rotation search, `a` the aseg used or not, `o` the output
//! aseg, `t` the half thickness; anything else `unknown option <arg>` on standard output, status
//! 1); then exactly one argument, the subject (else the help and an empty error, `ERROR_BADPARM`).
//! The subjects folder from `-sdir` or `SUBJECTS_DIR` (none: an error, `ERROR_BADPARM`); the files
//! `<sdir>/<subject>/mri/<name>`. With the aseg (the default, recon-all's `seg2cc`): the aseg read
//! (none: `ERROR_NOFILE`); with `-lh` or `-rh` it is written unchanged to the output and the
//! program ends (status 0); an aseg that holds the callosum's central label already ends with
//! status 77 unless `-force`; `norm` read (none: `ERROR_NOFILE`); the callosum found
//! ([`super::aseg::find_cc_with_aseg`]); then (both inputs) its centre printed, its principal
//! axes, the axis nearest z (pointing up) dividing the midsagittal slab (the centre's slice
//! `+-` the half thickness) into the divisions; with the aseg the callosum and fornix rotated back
//! and pasted into the aseg as labels 251 to 255 (or `230 +` the division with another count)
//! and the fornix (250, with `f`), the holes between callosum voxels filled, the posterior and
//! anterior parts in the wrong place relabelled, the aseg written; the time, the peak memory
//! (`#VMPC# mri_cc VmPeak`) and `mri_cc done`, status 0.
//!
//! Defects (`larmorx_core::defects`; the mechanisms in `docs/findings/freesurfer-mri-cc.md`):
//! - **FS-1203** (crash): an option given last whose value FreeSurfer reads past the arguments
//!   (status 139). By default `mri_cc: option <option> needs <n> value(s)`, the help, status 1.
//! - **FS-1204** (rule A): an output (the aseg, or `-lta`'s transform) that cannot be written still
//!   ends with status 0. By default status 1 after the writer's messages; `reproduce` 0.
//!
//! Not ported yet: the white-matter path (`-a 0`: `mri/wm` and `talairach.xfm`, recon-all never
//! runs it) ends with `mri_cc: the white-matter path (-a 0) is not ported yet`, status 1.

use std::io::Write;

use larmorx_core::defects::{FS_1203, FS_1204, FS_1211, FS_1212, FS_1213, FS_1214, FS_1215};
use larmorx_freesurfer_core::cstd::{atof, atoi, fmt_f};
use larmorx_freesurfer_core::diag::{DIAG_VERBOSE, DIAG_WRITE, diag_init};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::help_xml::output_help_xml;
use larmorx_freesurfer_core::log::{
    self, ERROR_BADFILE, ERROR_BADPARM, ERROR_NOFILE, ERROR_UNSUPPORTED, Fatal,
};
use larmorx_freesurfer_core::matrix::{MATRIX_REAL, matrix_alloc};
use larmorx_freesurfer_core::run::{Environment, Settings};
use larmorx_freesurfer_core::system;
use larmorx_freesurfer_core::timer::Timer;
use larmorx_freesurfer_core::utils::{get_vm_peak, nint, stricmp};
use larmorx_freesurfer_core::version::handle_version_option;
use larmorx_freesurfer_filter::segment::{mri_segment, mri_segment_max};
use larmorx_freesurfer_io::mriio::mri_read_volume;
use larmorx_freesurfer_mri::copy::mri_copy;
use larmorx_freesurfer_mri::mri::Mri;
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};
use larmorx_freesurfer_volume::arith::mri_clone;
use larmorx_freesurfer_volume::coords::{SAMPLE_NEAREST, principle_components};
use larmorx_freesurfer_volume::mriset::mri_voxels_in_label;
use larmorx_freesurfer_volume::xform::mri_linear_transform_interp;

use super::aseg::{FORNIX, find_cc_with_aseg, print_matrix, radians, write_volume};
use crate::mri_mask::main::Cout;

/// The program's name as `argv[0]` gives it (`seg2cc` runs it by name).
pub const NAME: &str = "mri_cc";

/// `mri_cc.help.xml`, the text `print_usage` renders (`outputHelpXml`).
pub const HELP_XML: &[u8] = include_bytes!("mri_cc.help.xml");

/// The status of a process ended by `SIGSEGV`, as a shell reports it.
const SEGFAULT: i32 = 139;
/// `STRLEN` (`const.h`: `STR_LEN`, 1,024): the names' buffers.
const STRLEN: usize = 1024;
/// `MAX_CC_DIVISIONS`, `DEFAULT_CC_DIVISIONS`.
const MAX_CC_DIVISIONS: i32 = 50;
const DEFAULT_CC_DIVISIONS: i32 = 5;
/// `CC_Central` and the callosum's other labels (`cma.h`); `MIN_CC_EXTRA` (`mri_cc.cpp`).
const CC_POSTERIOR: i32 = 251;
const CC_MID_POSTERIOR: i32 = 252;
const CC_CENTRAL: i32 = 253;
const CC_MID_ANTERIOR: i32 = 254;
const CC_ANTERIOR: i32 = 255;
const MIN_CC_EXTRA: i32 = 230;
const LEFT_WM: i32 = 2;
const RIGHT_WM: i32 = 41;

/// The settings the options give (the file-scope variables of `mri_cc.cpp`).
#[derive(Clone, Debug)]
pub struct Options {
    pub norm_thresh: i32,
    pub max_cc_rot: f64,
    pub use_aseg: i32,
    pub write_lta: bool,
    pub force: bool,
    /// `dxi`: the half thickness (`-t`).
    pub dxi: i32,
    pub fornix: bool,
    pub lh_only: bool,
    pub rh_only: bool,
    pub skip: i32,
    pub output_fname: String,
    pub norm_fname: String,
    pub aseg_fname: String,
    pub lta_fname: String,
    pub sdir: String,
    pub cc_divisions: i32,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            norm_thresh: 40,
            max_cc_rot: radians(7.0),
            use_aseg: 1,
            write_lta: false,
            force: false,
            dxi: 2,
            fornix: false,
            lh_only: false,
            rh_only: false,
            skip: 0,
            output_fname: "aseg_with_cc.mgz".to_owned(),
            norm_fname: "norm.mgz".to_owned(),
            aseg_fname: "aseg.mgz".to_owned(),
            lta_fname: String::new(),
            sdir: String::new(),
            cc_divisions: DEFAULT_CC_DIVISIONS,
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

/// `mri_cc`'s entry point: its original arguments (without the program's name), the settings,
/// the two streams; the exit status. The notices of the run follow on standard error.
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
    let keep = if matches!(i32::from(status), SEGFAULT | ABORT) && g.notices.reproduce() {
        cout.kept_by_crash()
    } else {
        cout.buf.len()
    };
    let _ = out.write_all(&cout.buf[..keep]);
    let _ = out.flush();
    g.notices.print(err);
    status
}

/// `print_usage()`: the help (`outputHelpXml`).
fn print_usage(g: &mut Globals, r: &mut Run) {
    output_help_xml(g, r.out, r.err, Some(HELP_XML));
}

/// FS-1203: `n` values of option `arg` read where `rest` (the option and what follows) has fewer.
fn need(g: &mut Globals, r: &mut Run, rest: &[String], n: usize) -> Result<(), Fatal> {
    if rest.len() > n {
        return Ok(());
    }
    let arg = &rest[0];
    g.notices.add(
        &FS_1203,
        format!(
            "mri_cc: option '{arg}' without its {n} value(s): FreeSurfer reads past its arguments and crashes"
        ),
    );
    if g.notices.reproduce() {
        return Err(Fatal::Exit(SEGFAULT));
    }
    let _ = writeln!(r.err, "{NAME}: option {arg} needs {n} value(s)");
    print_usage(g, r);
    Err(Fatal::Exit(1))
}

/// The status of a process ended by `SIGABRT` (glibc's fortified `strcpy`).
const ABORT: i32 = 134;

/// `strcpy` into one of the names' `STRLEN` buffers. FS-1212: a name of `STRLEN` bytes or more
/// (the official build's checked `strcpy` aborts: `*** buffer overflow detected ***: terminated`,
/// status 134, standard output lost); by default the name is kept whole.
fn name(g: &mut Globals, r: &mut Run, what: &str, s: &str) -> Result<String, Fatal> {
    if s.len() >= STRLEN {
        g.notices.add(
            &FS_1212,
            format!(
                "mri_cc: {what} of {} bytes is kept whole (FreeSurfer's {STRLEN}-byte buffer: it aborts)",
                s.len()
            ),
        );
        if g.notices.reproduce() {
            let _ = r
                .err
                .write_all(b"*** buffer overflow detected ***: terminated\n");
            return Err(Fatal::Exit(ABORT));
        }
    }
    Ok(s.to_owned())
}

/// `get_option(argc, argv)` on `rest` (`argv[1]` and what follows): the arguments it takes after
/// its own.
pub fn get_option(
    g: &mut Globals,
    r: &mut Run,
    o: &mut Options,
    rest: &[String],
) -> Result<usize, Fatal> {
    let arg = &rest[0];
    let option = &arg[1..];
    let is = |g: &mut Globals, n: &str| stricmp(g, option, n) == 0;
    if is(g, "SDIR") {
        need(g, r, rest, 1)?;
        o.sdir = name(g, r, "-sdir's value", &rest[1])?;
        let _ = writeln!(r.out, "using {} as SUBJECTS_DIR...", o.sdir);
        return Ok(1);
    }
    if is(g, "-HELP") || is(g, "-USAGE") {
        print_usage(g, r);
        return Err(Fatal::Exit(1));
    }
    if is(g, "DEBUG_VOXEL") {
        need(g, r, rest, 3)?;
        g.gx = atoi(rest[1].as_bytes());
        g.gy = atoi(rest[2].as_bytes());
        g.gz = atoi(rest[3].as_bytes());
        let _ = writeln!(r.out, "debugging voxel ({}, {}, {})", g.gx, g.gy, g.gz);
        return Ok(3);
    }
    if is(g, "ASEG") {
        need(g, r, rest, 1)?;
        o.aseg_fname = name(g, r, "-aseg's value", &rest[1])?;
        let _ = writeln!(r.out, "will read input aseg from {}", o.aseg_fname);
        o.use_aseg = 1;
        return Ok(1);
    }
    if is(g, "NORM") {
        need(g, r, rest, 1)?;
        o.norm_fname = name(g, r, "-norm's value", &rest[1])?;
        let _ = writeln!(r.out, "will read norm from {}", o.norm_fname);
        return Ok(1);
    }
    if is(g, "LTA") {
        need(g, r, rest, 1)?;
        o.lta_fname = name(g, r, "-lta's value", &rest[1])?;
        let _ = writeln!(r.out, "will write lta as {}", o.lta_fname);
        o.write_lta = true;
        return Ok(1);
    }
    if is(g, "force") {
        o.force = true;
        let _ = writeln!(
            r.out,
            "processing regardless of existence of cc in input volume"
        );
        return Ok(0);
    }
    if is(g, "lh") {
        o.lh_only = true;
        let _ = writeln!(r.out, "assuming only left hemisphere image");
        return Ok(0);
    }
    if is(g, "rh") {
        o.rh_only = true;
        let _ = writeln!(r.out, "assuming only right hemisphere image");
        return Ok(0);
    }
    let letter = option.as_bytes().first().map(u8::to_ascii_uppercase);
    match letter {
        Some(b'M') => {
            need(g, r, rest, 1)?;
            o.max_cc_rot = radians(atof(rest[1].as_bytes()));
            let _ = writeln!(
                r.out,
                "setting maximum rotation search to {} deg",
                fmt_f(super::aseg::degrees(o.max_cc_rot), 2, 1)
            );
            Ok(1)
        }
        Some(b'?' | b'H' | b'U') => {
            print_usage(g, r);
            Err(Fatal::Exit(1))
        }
        Some(b'F') => {
            o.fornix = true;
            let _ = writeln!(r.out, "including fornix in segmentation");
            Ok(0)
        }
        Some(b'D') => {
            need(g, r, rest, 1)?;
            o.cc_divisions = atoi(rest[1].as_bytes());
            if o.cc_divisions > MAX_CC_DIVISIONS {
                // FS-1211: FreeSurfer's message gives the count to a `%s`
                g.notices.add(
                    &FS_1211,
                    format!(
                        "mri_cc: -d {} is above {MAX_CC_DIVISIONS}: FreeSurfer's error message reads the count as text and crashes",
                        o.cc_divisions
                    ),
                );
                if g.notices.reproduce() {
                    return Err(Fatal::Exit(SEGFAULT));
                }
                log::error_exit(
                    g,
                    r.err,
                    ERROR_UNSUPPORTED,
                    &format!(
                        "{NAME}: too many CC divisions specified (max = {MAX_CC_DIVISIONS})\n"
                    ),
                )?;
            }
            if o.cc_divisions < 1 {
                // FS-1213 (rule B): no compartment to count into
                g.notices.add(
                    &FS_1213,
                    if g.notices.reproduce() {
                        format!(
                            "mri_cc: -d {}: the divisions' table is indexed before its start (memory outside it: not written), as FreeSurfer runs",
                            o.cc_divisions
                        )
                    } else {
                        format!(
                            "mri_cc: -d {} is refused (FreeSurfer counts into the table's element before its start)",
                            o.cc_divisions
                        )
                    },
                );
                if !g.notices.reproduce() {
                    log::error_exit(
                        g,
                        r.err,
                        ERROR_BADPARM,
                        &format!(
                            "{NAME}: the number of CC divisions must be at least 1 (-d {})\n",
                            o.cc_divisions
                        ),
                    )?;
                }
            }
            let _ = writeln!(
                r.out,
                "subdividing the cc into {} compartments",
                o.cc_divisions
            );
            Ok(1)
        }
        Some(b'S') => {
            need(g, r, rest, 1)?;
            o.skip = atoi(rest[1].as_bytes());
            let _ = writeln!(r.out, "skipping {} voxels in rotational alignment", o.skip);
            Ok(1)
        }
        Some(b'A') => {
            need(g, r, rest, 1)?;
            o.use_aseg = atoi(rest[1].as_bytes());
            let _ = writeln!(
                r.out,
                "{}using aseg as input instead of wm volume",
                if o.use_aseg != 0 { "" } else { "not " }
            );
            Ok(1)
        }
        Some(b'O') => {
            need(g, r, rest, 1)?;
            o.output_fname = name(g, r, "-o's value", &rest[1])?;
            let _ = writeln!(r.out, "writing aseg with cc labels to {}", o.output_fname);
            Ok(1)
        }
        Some(b'T') => {
            need(g, r, rest, 1)?;
            o.dxi = atoi(rest[1].as_bytes());
            let _ = writeln!(
                r.out,
                "setting callosum thickness to {} mm",
                o.dxi.wrapping_mul(2).wrapping_add(1)
            );
            Ok(1)
        }
        _ => {
            let _ = writeln!(r.out, "unknown option {arg}");
            Err(Fatal::Exit(1))
        }
    }
}

/// `snprintf(buf, STRLEN, "%s/%s/mri/%s", ...)` with `main`'s truncation message on the C++ error
/// stream (`std::cerr << __FUNCTION__ << ": Truncation on line " << line`) when the name is
/// `STRLEN` bytes or more (cut to `STRLEN - 1` bytes).
pub(crate) fn path_in(g: &mut Globals, r: &mut Run, name: String, line: u32) -> String {
    if name.len() < STRLEN {
        return name;
    }
    g.stderr.mark_cxx_message();
    let _ = writeln!(r.err, "main: Truncation on line {line}");
    let mut end = STRLEN - 1;
    while !name.is_char_boundary(end) {
        end -= 1;
    }
    name[..end].to_owned()
}

/// FS-1215 (rule B): the callosum's slab, `center - thick` to `center + thick`, reaches past the
/// volume's last column (`width`): FreeSurfer reads other voxels of the volume there (the next
/// rows'), and further out memory past it (a crash). By default an error; `reproduce` goes on (the
/// reads as fs.mri's accessors give them: FS-52, FS-53).
pub(crate) fn slab_inside(
    g: &mut Globals,
    r: &mut Run,
    center: i32,
    thick: i32,
    width: i32,
) -> Result<(), Fatal> {
    if i64::from(center) + i64::from(thick) < i64::from(width) {
        return Ok(());
    }
    g.notices.add(
        &FS_1215,
        format!(
            "mri_cc: the callosum's slab, column {center} +- {thick}, reaches past the volume's last column {}: FreeSurfer reads other voxels there",
            width - 1
        ),
    );
    if g.notices.reproduce() {
        return Ok(());
    }
    log::error_exit(
        g,
        r.err,
        ERROR_BADPARM,
        &format!(
            "{NAME}: the callosum's slab (column {center}, half thickness {thick}: -t) reaches past the volume's last column {}\n",
            width - 1
        ),
    )
}

/// FS-1204: a file the program wrote that could not be written.
fn unwritten(g: &mut Globals, fname: &str) -> Result<(), Fatal> {
    g.notices.add(
        &FS_1204,
        if g.notices.reproduce() {
            format!("mri_cc: '{fname}' was not written; status 0, as in FreeSurfer")
        } else {
            format!("mri_cc: '{fname}' was not written: status 1 (FreeSurfer ends with 0)")
        },
    );
    if g.notices.reproduce() {
        Ok(())
    } else {
        Err(Fatal::Exit(1))
    }
}

/// `MRIread(name)` of an input: `ERROR_NOFILE` with FreeSurfer's message when it does not read.
fn read_input(g: &mut Globals, r: &mut Run, fname: &str, what: &str) -> Result<Mri, Fatal> {
    match mri_read_volume(g, r.out, r.err, r.env, r.dir, fname)? {
        Some(m) => Ok(m),
        None => {
            log::error_exit(
                g,
                r.err,
                ERROR_NOFILE,
                &format!("{NAME}: could not read {what} volume from {fname}"),
            )?;
            unreachable!("ErrorExit ends the program")
        }
    }
}

/// `nint(MRIgetVoxVal(mri, x, y, z, 0))`.
fn nint_at(g: &mut Globals, r: &mut Run, mri: &Mri, x: i32, y: i32, z: i32) -> Result<i32, Fatal> {
    let v = mri_get_vox_val(g, r.err, mri, x, y, z, 0, 0)?;
    Ok(nint(g, f64::from(v)))
}

/// `IS_CC(l)` (`cma.h`): 251 to 255.
fn is_cc(l: i32) -> bool {
    (CC_POSTERIOR..=CC_ANTERIOR).contains(&l)
}

/// `main(argc, argv)` on the arguments after the program's name. `Ok` is status 0.
pub fn main(g: &mut Globals, r: &mut Run, args: &[String]) -> Result<(), Fatal> {
    let argv: Vec<String> = std::iter::once(NAME.to_owned())
        .chain(args.iter().cloned())
        .collect();
    let (nargs, argv) = handle_version_option(g, r.env, r.out, &argv, NAME);
    if nargs != 0 && argv.len() == 1 {
        return Err(Fatal::Exit(0));
    }
    g.progname = NAME.to_owned();
    diag_init(g, r.env, r.out);
    let mut o = Options::default();
    let mut argc = argv.len() as isize;
    let mut at = 1usize;
    while argc > 1 && argv[at].starts_with('-') {
        let nargs = get_option(g, r, &mut o, &argv[at..])?;
        argc -= nargs as isize + 1;
        at += nargs + 1;
    }
    let _ = r.out.flush();
    if argc != 2 {
        print_usage(g, r);
        log::error_exit(g, r.err, ERROR_BADPARM, "")?;
    }
    let subject = argv[at].clone();
    let then = Timer::new();
    if o.sdir.is_empty() {
        match r.env.get("SUBJECTS_DIR") {
            Some(v) => o.sdir = name(g, r, "SUBJECTS_DIR", &v.to_string_lossy())?,
            None => {
                log::error_exit(
                    g,
                    r.err,
                    ERROR_BADPARM,
                    &format!("{NAME}: SUBJECTS_DIR not defined in environment.\n"),
                )?;
            }
        }
    }
    let data_dir = o.sdir.clone();
    let mut vol_fp: Option<std::fs::File> = None;
    if g.gdiag & DIAG_WRITE != 0 && g.gdiag & DIAG_VERBOSE != 0 {
        let ifname = path_in(
            g,
            r,
            format!("{data_dir}/{subject}/mri/cc_volume_{}.txt", o.dxi),
            221,
        );
        let p = larmorx_freesurfer_core::utils::resolve(r.dir, &ifname);
        match std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(p)
        {
            Ok(f) => vol_fp = Some(f),
            Err(_) => {
                log::error_printf(
                    g,
                    r.err,
                    ERROR_BADFILE,
                    &format!("could not open cc volume measurement file {ifname}"),
                );
                return Err(Fatal::Exit(ERROR_BADFILE));
            }
        }
        let _ = writeln!(r.out, "writing results to {ifname}");
    }

    // FS-1204: a failed write ends the run with status 1 by default only after the rest is done
    let mut failed: Vec<String> = Vec::new();
    // with the aseg: the aseg, the fornix and the transform that rotated them upright
    let mut with_aseg: Option<(Mri, Mri, larmorx_freesurfer_mri::lta::Lta)> = None;
    let (mut cc, xc, yc, zc);
    if o.use_aseg == 0 {
        let found = super::wm::find_cc_with_wm(g, r, &o, &data_dir, &subject)?;
        failed.extend(found.unwritten);
        (cc, xc, yc, zc) = (found.cc, found.xc, found.yc, found.zc);
    } else {
        let ifname = path_in(
            g,
            r,
            format!("{data_dir}/{subject}/mri/{}", o.aseg_fname),
            240,
        );
        let _ = writeln!(r.out, "reading aseg from {ifname}");
        let mut aseg = read_input(g, r, &ifname, "aseg")?;
        if o.lh_only || o.rh_only {
            let ofname = path_in(
                g,
                r,
                format!("{data_dir}/{subject}/mri/{}", o.output_fname),
                252,
            );
            let _ = writeln!(r.out, "copying aseg WITHOUT callosum to {ofname}...");
            if write_volume(g, r, &mut aseg, &ofname)? != 0 {
                unwritten(g, &ofname)?;
            }
            return Err(Fatal::Exit(0));
        }
        if mri_voxels_in_label(g, r.err, &aseg, CC_CENTRAL)? > 0 && !o.force {
            let _ = r.out.flush();
            log::error_exit(
                g,
                r.err,
                77,
                &format!(
                    "{NAME}: volume {ifname} already has CC in it.\nRun with -force to reprocess (not recommended)\n"
                ),
            )?;
        }
        let ifname = path_in(
            g,
            r,
            format!("{data_dir}/{subject}/mri/{}", o.norm_fname),
            275,
        );
        let _ = writeln!(r.out, "reading norm from {ifname}");
        let norm = read_input(g, r, &ifname, "aseg")?;
        let mut fornix = mri_clone(g, r.out, r.err, &aseg, None)?.unwrap_new();
        let found = find_cc_with_aseg(g, r, &o, &aseg, &norm, &mut fornix)?;
        if o.write_lta && found.lta_unwritten {
            failed.push(o.lta_fname.clone());
        }
        drop(norm);
        (cc, xc, yc, zc) = (found.cc, found.xc, found.yc, found.zc);
        with_aseg = Some((aseg, fornix, found.lta));
    }

    // the midsagittal slab divided along the axis nearest z
    let xi = nint(g, xc);
    let (yi, zi) = (nint(g, yc), nint(g, zc));
    let _ = writeln!(r.out, "cc center is found at {xi} {yi} {zi}");
    let mut m_evectors = matrix_alloc(g, r.out, r.err, 3, 3, MATRIX_REAL)?.expect("a real matrix");
    let mut evalues = [0f32; 3];
    let (_, means, _) =
        principle_components(g, r.out, r.err, &cc, Some(&mut m_evectors), &mut evalues, 1)?;
    let _ = writeln!(r.out, "eigenvectors:");
    print_matrix(g, r, &m_evectors);
    let mut ez_x = m_evectors.relt(1, 1);
    let mut ez_y = m_evectors.relt(2, 1);
    let mut ez_z = m_evectors.relt(3, 1);
    for i in 2..=3 {
        if m_evectors.relt(3, i).abs() > ez_z.abs() {
            ez_x = m_evectors.relt(1, i);
            ez_y = m_evectors.relt(2, i);
            ez_z = m_evectors.relt(3, i);
        }
    }
    if ez_z < 0.0 {
        ez_x *= -1.0;
        ez_y *= -1.0;
        ez_z *= -1.0;
    }
    let along = |x: i32, y: i32, z: i32| -> f32 {
        ((f64::from(x) - means[0]) * f64::from(ez_x)
            + (f64::from(y) - means[1]) * f64::from(ez_y)
            + (f64::from(z) - means[2]) * f64::from(ez_z)) as f32
    };
    let (mut zf_low, mut zf_high) = (256f32, 0f32);
    for y in 0..cc.vg.height {
        for z in 0..cc.vg.depth {
            let v = mri_get_vox_val(g, r.err, &cc, xi, y, z, 0, 0)?;
            if nint(g, f64::from(v)) != 0 {
                let zf = along(xi, y, z);
                if zf < zf_low {
                    zf_low = zf;
                }
                if zf > zf_high {
                    zf_high = zf;
                }
            }
        }
    }
    let ndiv = o.cc_divisions;
    let mut volume = [0f32; MAX_CC_DIVISIONS as usize];
    slab_inside(g, r, xi, o.dxi, cc.vg.width)?;
    for i in xi.wrapping_sub(o.dxi)..=xi.wrapping_add(o.dxi) {
        for j in 0..cc.vg.height {
            for k in 0..cc.vg.depth {
                let v = mri_get_vox_val(g, r.err, &cc, i, j, k, 0, 0)?;
                if nint(g, f64::from(v)) <= 0 {
                    continue;
                }
                let zf = along(i, j, k);
                if zf >= zf_low - 10.0 {
                    let q = (zf - zf_low) / ((zf_high - zf_low + 1.0) / ndiv as f32);
                    let mut temp = larmorx_freesurfer_core::affine::float_to_int(
                        g,
                        f64::from(q.floor()),
                        "mri_cc",
                    );
                    if temp < 0 {
                        temp = 0;
                    }
                    if temp >= ndiv {
                        temp = ndiv - 1;
                    }
                    if let Some(slot) = usize::try_from(temp).ok().and_then(|t| volume.get_mut(t)) {
                        *slot += 1.0;
                    }
                    let val = (temp.wrapping_add(1)).wrapping_mul(20).wrapping_add(10);
                    mri_set_vox_val(g, r.err, &mut cc, i, j, k, 0, val as f32, 0)?;
                }
            }
        }
    }

    // with the aseg: the callosum pasted into it, the aseg written
    let mut fornix_out = None;
    if let Some((aseg, fornix, lta)) = with_aseg {
        let (ofname, ok, fornix) =
            paste_into_aseg(g, r, &o, &data_dir, &subject, cc, aseg, fornix, &lta)?;
        if !ok {
            failed.push(ofname);
        }
        fornix_out = Some(fornix);
    }

    if let Some(fp) = vol_fp.as_mut() {
        if !(5..=MAX_CC_DIVISIONS).contains(&o.cc_divisions) {
            // FS-1214: the five divisions printed, of which only the first `-d` were counted
            g.notices.add(
                &FS_1214,
                format!(
                    "mri_cc: the volume measurement file's line prints five divisions; with -d {} the others were never counted (memory contents): 0",
                    o.cc_divisions
                ),
            );
        }
        let line = format!(
            "{subject} {} {} {} {} {} \n",
            nint(g, f64::from(volume[4])),
            nint(g, f64::from(volume[3])),
            nint(g, f64::from(volume[2])),
            nint(g, f64::from(volume[1])),
            nint(g, f64::from(volume[0]))
        );
        let _ = fp.write_all(line.as_bytes());
        let _ = r.out.write_all(line.as_bytes());
    }
    // the fornix exists with the aseg only (FreeSurfer's `mri_fornix` is NULL with `-a 0`)
    if let (Some(fornix), true) = (fornix_out.as_mut(), o.fornix) {
        let ofname = path_in(g, r, format!("{data_dir}/{subject}/mri/fornix.mgz"), 711);
        let _ = writeln!(r.out, "writing fornix to to {ofname}...");
        write_volume(g, r, fornix, &ofname)?;
    }
    let msec = then.milliseconds();
    let _ = writeln!(
        r.out,
        "corpus callosum segmentation took {} minutes",
        fmt_f(f64::from(msec as f32 / 60000.0f32), 2, 1)
    );
    drop(vol_fp);
    let peak = get_vm_peak(g);
    let _ = writeln!(r.out, "#VMPC# mri_cc VmPeak  {peak}");
    let _ = writeln!(r.out, "mri_cc done");
    let mut status = Ok(());
    for f in &failed {
        if let Err(e) = unwritten(g, f) {
            status = Err(e);
        }
    }
    status
}

/// The aseg path's end (`mri_cc.cpp` 524-680): the callosum and the fornix rotated back, pasted
/// into the aseg as the divisions' labels (and the fornix's with `f`), the holes between callosum
/// voxels filled, the curved callosums' ends relabelled, the aseg written: its name, whether it
/// was written, and the fornix (rotated back).
#[allow(clippy::too_many_arguments)]
fn paste_into_aseg(
    g: &mut Globals,
    r: &mut Run,
    o: &Options,
    data_dir: &str,
    subject: &str,
    mut cc: Mri,
    mut aseg: Mri,
    mut fornix: Mri,
    lta: &larmorx_freesurfer_mri::lta::Lta,
) -> Result<(String, bool, Mri), Fatal> {
    let ndiv = o.cc_divisions;
    // the callosum and the fornix rotated back and pasted into the aseg
    let inv = lta.inv_xforms[0].m_l.clone();
    let tmp =
        mri_linear_transform_interp(g, r.out, r.err, &cc, None, inv.as_ref(), SAMPLE_NEAREST)?
            .unwrap_new();
    mri_copy(g, r.out, r.err, &tmp, Some(&mut cc))?;
    let tmp =
        mri_linear_transform_interp(g, r.out, r.err, &fornix, None, inv.as_ref(), SAMPLE_NEAREST)?
            .unwrap_new();
    mri_copy(g, r.out, r.err, &tmp, Some(&mut fornix))?;
    drop(tmp);
    for i in 0..cc.vg.width {
        for j in 0..cc.vg.height {
            for k in 0..cc.vg.depth {
                let c = nint_at(g, r, &cc, i, j, k)?;
                if c > 0 {
                    let mut temp = ((c - 10) / 20) - 1;
                    if temp < 0 {
                        temp = 0;
                    } else if temp >= ndiv {
                        temp = ndiv - 1;
                    }
                    let label = if ndiv != DEFAULT_CC_DIVISIONS {
                        (MIN_CC_EXTRA.wrapping_add(temp)).min(255)
                    } else {
                        match temp {
                            1 => CC_MID_POSTERIOR,
                            2 => CC_CENTRAL,
                            3 => CC_MID_ANTERIOR,
                            4 => CC_ANTERIOR,
                            _ => CC_POSTERIOR,
                        }
                    };
                    mri_set_vox_val(g, r.err, &mut aseg, i, j, k, 0, label as f32, 0)?;
                } else if o.fornix && nint_at(g, r, &fornix, i, j, k)? > 0 {
                    mri_set_vox_val(g, r.err, &mut aseg, i, j, k, 0, FORNIX as f32, 0)?;
                }
            }
        }
    }

    // small holes in the callosum (from the interpolation) filled
    for i in 0..cc.vg.width {
        for j in 0..cc.vg.height {
            for k in 0..cc.vg.depth {
                let label = nint_at(g, r, &aseg, i, j, k)?;
                if label != LEFT_WM && label != RIGHT_WM {
                    continue;
                }
                let left_label = nint_at(g, r, &aseg, i.wrapping_add(1), j, k)?;
                if !is_cc(left_label) {
                    continue;
                }
                let right_label = nint_at(g, r, &aseg, i.wrapping_sub(1), j, k)?;
                if !is_cc(right_label) {
                    continue;
                }
                mri_set_vox_val(g, r.err, &mut aseg, i, j, k, 0, left_label as f32, 0)?;
            }
        }
    }

    // curved callosums: the parts of the mid-anterior and mid-posterior labels beyond the
    // largest one's rows relabelled anterior and posterior
    for (lab, other, skip_self) in [
        (CC_MID_ANTERIOR, CC_ANTERIOR, true),
        (CC_MID_POSTERIOR, CC_POSTERIOR, false),
    ] {
        let mseg = mri_segment(g, r.out, r.err, &aseg, lab as f32, lab as f32)?;
        if mseg.nsegments > 1 {
            let j = mri_segment_max(&mseg);
            let ju = usize::try_from(j).ok();
            for i in 0..mseg.nsegments as usize {
                if skip_self && Some(i) == ju {
                    continue;
                }
                let (Some(si), Some(sj)) =
                    (mseg.segments.get(i), ju.and_then(|j| mseg.segments.get(j)))
                else {
                    continue;
                };
                if si.y0 > sj.y1 {
                    let _ = writeln!(r.out, "error in mid anterior detected - correcting...");
                    for v in &si.voxels {
                        mri_set_vox_val(
                            g,
                            r.err,
                            &mut aseg,
                            i32::from(v.x),
                            i32::from(v.y),
                            i32::from(v.z),
                            0,
                            other as f32,
                            0,
                        )?;
                    }
                }
            }
        }
    }
    let ofname = path_in(
        g,
        r,
        format!("{data_dir}/{subject}/mri/{}", o.output_fname),
        673,
    );
    let _ = writeln!(r.out, "writing aseg with callosum to {ofname}...");
    let written = write_volume(g, r, &mut aseg, &ofname)? == 0;
    Ok((ofname, written, fornix))
}
