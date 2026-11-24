// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_pretess`'s program (`mri_mc/mri_pretess.cpp` 1049-1329): the entry point [`run`]
//! (registered as `mri_pretess` in the crate's `PROGRAMS`), [`main`] and the options
//! ([`get_option`]).
//!
//! `mri_pretess [options] <filled> <label> <norm> <out>` (recon-all: `wm.asegedit.mgz wm norm.mgz
//! wm.mgz`, with `-keep` when it runs again over an existing `wm.mgz`; `filled.mgz 255 norm.mgz
//! filled-pretess255.mgz` and the same with 127). The version options first (`argc` kept: their
//! places in the arguments repeat others, FS-07); the options while the next argument starts with
//! `-` (each uppercased after its dash in the arguments themselves, compared without case:
//! `debug_voxel`, `-help`, `-usage`, `nocorners`, `keep`, `test`; then by the first letter: `W`
//! the diagnostic write flag, `?`, `H`, `U` the help with status 1, anything else `unknown option
//! <ARG>` on standard output, status 1); fewer than four arguments left: the help, status 255.
//!
//! The segmentation read (`-test`: an edge voxel added to it first), the label (`wm`, without
//! case: 128 on the white matter binarized at `WM_MIN_VAL`; else `atoi`), the segmentation
//! changed to bytes, the intensities read (their extent must be the segmentation's); up to 100
//! iterations of the edge, corner (unless `-nocorners`) and background corner removals until one
//! changes nothing; the count of the label's voxels; for `wm`, the voxels the binarized volume
//! gained written as `PRETESS_FILL` into the segmentation where it was below `WM_MIN_VAL`;
//! `-keep`'s edits (the values 255 and 1 of the output file as it is) copied back; the output
//! written (not with `-test`); `mri_pretess done`; status 0.
//!
//! Defects (`larmorx_core::defects`; the mechanisms in `docs/findings/freesurfer-mri-pretess.md`):
//! FS-1281 (`-debug_voxel` without its values), FS-1282 (a label number outside 0 to 255),
//! FS-1283 (no voxel of the label: a percentage that is not a number, kept), FS-1284 (a
//! segmentation not of bytes, kept), FS-1285 (an unwritten output); FS-1286 (the background
//! corners' check, kept: [`super::tess`]).

use std::io::Write;

use larmorx_core::defects::{FS_1281, FS_1282, FS_1283, FS_1284, FS_1285};
use larmorx_freesurfer_core::cstd::{atoi, fmt_f};
use larmorx_freesurfer_core::diag::DIAG_WRITE;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::help_xml::output_help_xml;
use larmorx_freesurfer_core::log::{self, ERROR_BADPARM, ERROR_NOFILE, Fatal};
use larmorx_freesurfer_core::run::{Environment, Settings};
use larmorx_freesurfer_core::system;
use larmorx_freesurfer_core::utils::stricmp;
use larmorx_freesurfer_core::version::{get_all_info, handle_version_option_whole};
use larmorx_freesurfer_io::mriio::{mri_add_command_line, mri_read_volume, mri_write};
use larmorx_freesurfer_mri::mri::{MGZ_INTENT_UNKNOWN, MRI_UCHAR, Mri, mri_alloc};
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};
use larmorx_freesurfer_volume::arith::{Pair, mri_binarize};
use larmorx_freesurfer_volume::mriset::mri_copy_label;
use larmorx_freesurfer_volume::resize::mri_change_type;

use super::tess::{Counters, Tess, WM_MIN_VAL, mri_add_edge_voxel};
use crate::mri_mask::main::Cout;

/// The program's name as `argv[0]` gives it (recon-all runs it by name).
pub const NAME: &str = "mri_pretess";

/// `mri_pretess.help.xml`, the text `print_help` renders (`outputHelpXml`).
pub const HELP_XML: &[u8] = include_bytes!("mri_pretess.help.xml");

/// The status of a process ended by `SIGSEGV`, as a shell reports it.
const SEGFAULT: i32 = 139;
/// `USE_WM`: the label `wm` stands for.
const USE_WM: i32 = -1;
/// `WM_EDITED_ON_VAL`, `WM_EDITED_OFF_VAL` (`include/mri.h`).
const WM_EDITED_ON_VAL: i32 = 255;
const WM_EDITED_OFF_VAL: i32 = 1;
/// `PRETESS_FILL` (`include/mri.h`).
const PRETESS_FILL: f32 = 215.0;
/// x86-64's default NaN (sign bit set), the quotient `0.0 / 0.0` of FreeSurfer's build.
const DEFAULT_NAN: u64 = 0xfff8_0000_0000_0000;

/// The settings the options give (the file-scope variables of `mri_pretess.cpp`).
#[derive(Debug)]
pub struct Options {
    pub corners: bool,
    pub keep_edits: bool,
    pub test_edge: bool,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            corners: true,
            keep_edits: false,
            test_edge: false,
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

/// `mri_pretess`'s entry point: its original arguments (without the program's name), the
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

/// `print_help()`: the help (`outputHelpXml`) on standard output.
fn print_help(g: &mut Globals, r: &mut Run) {
    output_help_xml(g, r.out, r.err, Some(HELP_XML));
}

fn read(g: &mut Globals, r: &mut Run, fname: &str) -> Result<Option<Mri>, Fatal> {
    mri_read_volume(g, r.out, r.err, r.env, r.dir, fname)
}

/// `get_option(argc, argv)` on `rest` (`argv[1]` and what follows): the arguments it takes after
/// its own. FreeSurfer uppercases the option after its dash in the arguments themselves
/// (`StrUpper`), so `unknown option` names it uppercased.
pub fn get_option(
    g: &mut Globals,
    r: &mut Run,
    o: &mut Options,
    rest: &[String],
) -> Result<usize, Fatal> {
    let arg = format!("{}{}", &rest[0][..1], rest[0][1..].to_ascii_uppercase());
    let option = &arg[1..];
    let is = |g: &mut Globals, n: &str| stricmp(g, option, n) == 0;
    if is(g, "debug_voxel") {
        if rest.len() < 4 {
            // FS-1281: atoi of the NULL after the last argument
            g.notices.add(
                &FS_1281,
                format!(
                    "mri_pretess: option '{}' without its 3 values: FreeSurfer reads past its arguments and crashes",
                    rest[0]
                ),
            );
            if g.notices.reproduce() {
                return Err(Fatal::Exit(SEGFAULT));
            }
            let _ = writeln!(r.err, "{NAME}: option {} needs 3 values", rest[0]);
            print_help(g, r);
            return Err(Fatal::Exit(1));
        }
        g.gx = atoi(rest[1].as_bytes());
        g.gy = atoi(rest[2].as_bytes());
        g.gz = atoi(rest[3].as_bytes());
        let _ = writeln!(r.out, "debugging voxel ({}, {}, {})", g.gx, g.gy, g.gz);
        return Ok(3);
    }
    if is(g, "-help") || is(g, "-usage") {
        print_help(g, r);
        return Err(Fatal::Exit(1));
    }
    if is(g, "nocorners") {
        o.corners = false;
        let _ = writeln!(
            r.out,
            "no removal of corner configurations in addition to edge ones"
        );
        return Ok(0);
    }
    if is(g, "keep") {
        o.keep_edits = true;
        let _ = writeln!(r.out, "keeping edits");
        return Ok(0);
    }
    if is(g, "test") {
        o.test_edge = true;
        let _ = writeln!(r.out, "testing, output will not be saved");
        return Ok(0);
    }
    match option.bytes().next() {
        Some(b'W') => {
            g.gdiag |= DIAG_WRITE;
            Ok(0)
        }
        Some(b'?') | Some(b'H') | Some(b'U') => {
            print_help(g, r);
            Err(Fatal::Exit(1))
        }
        _ => {
            let _ = writeln!(r.out, "unknown option {arg}");
            Err(Fatal::Exit(1))
        }
    }
}

/// `100.0 * ntotal / nvoxels` as FreeSurfer's build computes it: with no voxel of the label (and
/// so none modified) x86-64's default NaN, printed `-nan` (FS-1283).
fn percent(g: &mut Globals, ntotal: i32, nvoxels: i32) -> f64 {
    if nvoxels == 0 {
        g.notices.add(
            &FS_1283,
            "mri_pretess: no voxel of the label: the percentage of modified voxels is 0 / 0, printed -nan as in FreeSurfer (kept)",
        );
        if ntotal == 0 {
            return f64::from_bits(DEFAULT_NAN);
        }
        return if ntotal > 0 {
            f64::INFINITY
        } else {
            f64::NEG_INFINITY
        };
    }
    100.0 * f64::from(ntotal) / f64::from(nvoxels)
}

/// For `wm`, after the removals: each voxel above `WM_MIN_VAL` in the binarized volume whose
/// segmentation value is below it set to `PRETESS_FILL` in the segmentation (frame 0; columns
/// outermost; a byte volume's voxels directly).
fn restore(g: &mut Globals, err: &mut dyn Write, b: &Mri, seg: &mut Mri) -> Result<(), Fatal> {
    let n = b.vox_per_vol as usize;
    if seg.mtype == MRI_UCHAR
        && seg.vox_per_row == b.vox_per_row
        && seg.vox_per_slice == b.vox_per_slice
        && let (Some(bc), Some(sc)) = (b.chunk.as_ref(), seg.chunk.as_mut())
        && bc.len() >= n
        && sc.len() >= n
    {
        for (s, &v) in sc[..n].iter_mut().zip(&bc[..n]) {
            if i32::from(v) > WM_MIN_VAL && i32::from(*s) < WM_MIN_VAL {
                *s = PRETESS_FILL as u8;
            }
        }
        return Ok(());
    }
    for x in 0..b.vg.width {
        for y in 0..b.vg.height {
            for z in 0..b.vg.depth {
                let v = mri_get_vox_val(g, err, b, x, y, z, 0, 0)?;
                if v > WM_MIN_VAL as f32
                    && mri_get_vox_val(g, err, seg, x, y, z, 0, 0)? < WM_MIN_VAL as f32
                {
                    mri_set_vox_val(g, err, seg, x, y, z, 0, PRETESS_FILL, 0)?;
                }
            }
        }
    }
    Ok(())
}

/// `main(argc, argv)` on the arguments after the program's name.
pub fn main(g: &mut Globals, r: &mut Run, args: &[String]) -> Result<(), Fatal> {
    let argv0: Vec<String> = std::iter::once(NAME.to_owned())
        .chain(args.iter().cloned())
        .collect();
    let env = r.env.clone();
    let (nargs, argv) = handle_version_option_whole(g, &env, r.out, &argv0, NAME);
    if nargs != 0 && argv0.len() as i32 - nargs == 1 {
        return Err(Fatal::Exit(0));
    }
    let cmdline = get_all_info(g, &env, &argv, NAME);
    g.progname = NAME.to_owned();
    let mut o = Options::default();
    let mut argc = argv.len() as isize;
    let mut at = 1usize;
    while argc > 1 && argv.get(at).is_some_and(|a| a.starts_with('-')) {
        let nargs = get_option(g, r, &mut o, &argv[at..])?;
        argc -= nargs as isize + 1;
        at += nargs + 1;
    }
    if argc < 5 {
        print_help(g, r);
        return Err(Fatal::Exit(255));
    }
    let seg_name = argv[at].clone();
    let label_arg = argv[at + 1].clone();
    let orig_name = argv[at + 2].clone();
    let out_name = argv[at + 3].clone();

    let Some(mut seg) = read(g, r, &seg_name)? else {
        log::error_exit(
            g,
            r.err,
            ERROR_NOFILE,
            &format!("{NAME}: could not open {seg_name}"),
        )?;
        unreachable!("ErrorExit ends the program")
    };
    if o.test_edge {
        let _ = writeln!(r.out, "Adding WM edge to input segmentation.");
        if mri_add_edge_voxel(g, r.out, r.err, &mut seg, WM_EDITED_ON_VAL)? == 0 {
            let _ = writeln!(r.out, "ERROR: could not add test edge");
            return Err(Fatal::Exit(1));
        }
    }
    let is_wm = stricmp(g, &label_arg, "wm") == 0;
    let mut label = if is_wm {
        USE_WM
    } else {
        atoi(label_arg.as_bytes())
    };
    if !is_wm && !(0..=255).contains(&label) {
        // FS-1282: a byte segmentation holds no such label; -1 is FreeSurfer's code for wm
        g.notices.add(
            &FS_1282,
            if label == USE_WM {
                format!("mri_pretess: the label '{label_arg}' is -1, which FreeSurfer takes as wm")
            } else {
                format!(
                    "mri_pretess: the label '{label_arg}' ({label}) is outside 0 to 255: FreeSurfer finds no voxel of it and writes the segmentation unchanged"
                )
            },
        );
        if !g.notices.reproduce() {
            let _ = writeln!(
                r.err,
                "{NAME}: ERROR: the label {label_arg} is neither wm nor a number from 0 to 255 (the segmentation is read as bytes)"
            );
            return Err(Fatal::Exit(1));
        }
    }
    if seg.mtype != MRI_UCHAR {
        g.notices.add(
            &FS_1284,
            "mri_pretess: the segmentation is changed to bytes before the edges are removed (its values rounded and clamped to 0 to 255) and the output is of bytes: kept",
        );
        let Some(m) = mri_change_type(g, r.out, r.err, &env, &seg, MRI_UCHAR, 0.0, 255.0, 1)?
        else {
            // FreeSurfer goes on with no volume and crashes
            return Err(Fatal::Exit(SEGFAULT));
        };
        seg = m;
    }
    let Some(orig) = read(g, r, &orig_name)? else {
        log::error_exit(
            g,
            r.err,
            ERROR_NOFILE,
            &format!("{NAME}: could not open {orig_name}"),
        )?;
        unreachable!("ErrorExit ends the program")
    };
    if orig.vg.width != seg.vg.width
        || orig.vg.height != seg.vg.height
        || orig.vg.depth != seg.vg.depth
    {
        log::error_exit(
            g,
            r.err,
            ERROR_BADPARM,
            &format!(
                "intensity ({}, {}, {}) and segmentation volume ({} x {} x {}) don't match",
                orig.vg.width,
                orig.vg.height,
                orig.vg.depth,
                seg.vg.width,
                seg.vg.height,
                seg.vg.depth
            ),
        )?;
    }
    // for wm, the binarized white matter is worked on and `seg` keeps the segmentation
    let mut binary = None;
    if label == USE_WM {
        let _ = writeln!(r.out, "binarizing input wm segmentation...");
        label = 128;
        let mut b = mri_alloc(
            g,
            r.out,
            r.err,
            seg.vg.width,
            seg.vg.height,
            seg.vg.depth,
            MRI_UCHAR,
        )?;
        mri_binarize(
            g,
            r.out,
            r.err,
            Pair::Given(&seg, &mut b),
            WM_MIN_VAL as f32,
            0.0,
            label as f32,
        )?;
        binary = Some(b);
    }

    let _ = writeln!(r.out, "Ambiguous edge configurations... ");
    let mut counters = Counters::default();
    let mut ntotal = 0i32;
    let nvoxels = {
        let work = binary.as_mut().unwrap_or(&mut seg);
        let mut t = Tess::new(work, &orig, label);
        for _ in 0..100 {
            let mut nmodified = t.remove_edges(g, r.err, &mut counters)?;
            if o.corners {
                nmodified += t.remove_corners(g, r.err, &mut counters)?;
            }
            nmodified += t.remove_background_corners(g, r.err, &mut counters)?;
            if nmodified == 0 {
                break;
            }
            ntotal += nmodified;
        }
        t.count()
    };
    let pct = percent(g, ntotal, nvoxels);
    let _ = write!(
        r.err,
        "\n\nTotal Number of Modified Voxels = {ntotal} (out of {nvoxels}: {})\n",
        fmt_f(pct, 0, 6)
    );
    let _ = writeln!(r.out);

    if let Some(b) = binary.take() {
        restore(g, r.err, &b, &mut seg)?;
    }

    if o.keep_edits {
        let _ = writeln!(r.out, "Searching for edits to keep ...");
        let Some(old) = read(g, r, &out_name)? else {
            log::error_printf(
                g,
                r.err,
                ERROR_NOFILE,
                &format!("{NAME}: could not read file {out_name} to preserve edits"),
            );
            return Err(Fatal::Exit(1));
        };
        let n = mri_copy_label(g, r.err, &old, &mut seg, WM_EDITED_ON_VAL)?;
        let _ = writeln!(r.out, "  kept {n} WM ON voxels");
        let n = mri_copy_label(g, r.err, &old, &mut seg, WM_EDITED_OFF_VAL)?;
        let _ = writeln!(r.out, "  kept {n} WM OFF voxels");
        let _ = writeln!(r.out);
    }

    mri_add_command_line(g, r.err, &mut seg, cmdline.as_bytes());
    if !o.test_edge {
        let rc = mri_write(
            g,
            r.out,
            r.err,
            r.env,
            r.dir,
            &mut seg,
            &out_name,
            None,
            MGZ_INTENT_UNKNOWN,
        )?;
        if rc != 0 {
            g.notices.add(
                &FS_1285,
                if g.notices.reproduce() {
                    format!("mri_pretess: '{out_name}' was not written; status 0, as in FreeSurfer")
                } else {
                    format!(
                        "mri_pretess: '{out_name}' was not written: status 1 (FreeSurfer ends with 0)"
                    )
                },
            );
            if !g.notices.reproduce() {
                return Err(Fatal::Exit(1));
            }
        }
    } else {
        let _ = writeln!(r.out);
        let _ = writeln!(r.out, "NOT saving output because called with -test.");
        let _ = writeln!(r.out);
    }
    let _ = writeln!(r.out, "mri_pretess done");
    let _ = writeln!(r.out);
    Ok(())
}
