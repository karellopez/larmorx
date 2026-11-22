// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_add_xform_to_header`'s `main` (`mri_add_xform_to_header.cpp` 45-158), `get_option`
//! (164-205), `usage_exit` and `print_usage` (208-221): the program's entry point [`run`]
//! (registered as `mri_add_xform_to_header` in the crate's `PROGRAMS`) and [`main`].
//!
//! `mri_add_xform_to_header [options] <xfm> <in> [<out>]`. The version options first
//! (`handleVersionOption`: `<name> freesurfer 8.2.0` for each; the program goes on); no argument
//! left: the help (`mri_add_xform_to_header.help.xml`), status 1. Then the options, while the next
//! argument starts with `-`: `--help` and `--usage` (any case) the help, status 1; otherwise the
//! option's first letter after the dash, in any case: `c` (copy the name only: the transform is
//! not read), `v` (verbose: toggled at each), `n` (skips the next argument), `?`, `h`, `u` (the
//! help, status 1); anything else `unknown option <arg>` on standard error, status 1. Then the
//! transform's name (`<progname>: no transform name specified`, `ERROR_BADPARM`), the input
//! (`<progname>: no input name specified`), the output (the input when not given; more arguments
//! are ignored).
//!
//! The input: a folder is read as a COR folder's header (`MRIreadInfo`); a readable file must have
//! an extension (`<progname>: no extension found`), printed as `INFO: extension is <ext>` on
//! standard output, which must be `mgz` or `mgh` (`<progname>: currently only .mgz or .mgh saves
//! transform name`; both `ERROR_BADPARM`), and is read whole (`MRIread`). No volume:
//! `<progname>: could not open source file <in>` (`ERROR_NO_FILE`). Without `-c` the transform
//! file is read into the volume (one that does not read: `<progname>: could not read xform file
//! '<xfm>'` as an error, and the program goes on with what the read left) and its matrices taken
//! (a transform that is not linear ends the program, status 1). The name is copied into the
//! header. A folder as the output gets its `COR-.info` rewritten ([`super::write_info`]), any
//! other name the whole volume (`MRIwrite`, in the format its name gives). Status 0.
//!
//! Defects (`larmorx_core::defects`; the mechanisms in `docs/findings/freesurfer-add-xform.md`):
//! - **FS-1154** (crash): a transform name of 1,024 bytes or more passes the header's room for it:
//!   the official build aborts (`*** buffer overflow detected ***: terminated`, status 134) before
//!   writing (by default the whole name; `reproduce` aborts as it does).
//! - **FS-1155**: an output that cannot be written still ends with status 0 (by default status 1
//!   after the writer's messages).
//! - FS-1156 to FS-1158: [`super::write_info`].

use std::io::Write;

use larmorx_core::defects::{FS_1154, FS_1155};
use larmorx_freesurfer_core::diag::diag_init;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::help_xml::output_help_xml;
use larmorx_freesurfer_core::log::{
    self, ERROR_BADPARM, ERROR_NO_FILE, ERROR_NO_MEMORY, Fatal, error_exit, error_printf,
};
use larmorx_freesurfer_core::run::{Environment, Settings};
use larmorx_freesurfer_core::system;
use larmorx_freesurfer_core::version::handle_version_option;
use larmorx_freesurfer_io::fio::{fio_extension, fio_file_exists_readable, fio_is_directory};
use larmorx_freesurfer_io::minc::input_transform_file_ex;
use larmorx_freesurfer_io::mriio::{mri_read_info, mri_read_volume, mri_write};
use larmorx_freesurfer_mri::mri::{
    MGZ_INTENT_UNKNOWN, get_inverse_linear_transform_ptr, get_linear_transform_ptr,
};

use super::write_info::{BUFFER_OVERFLOW, mri_write_info};

/// The program's name as `argv[0]` gives it (recon-all runs it by name).
pub const NAME: &str = "mri_add_xform_to_header";

/// `mri_add_xform_to_header.help.xml`, the text `print_usage` renders (`outputHelpXml`).
pub const HELP_XML: &[u8] = include_bytes!("mri_add_xform_to_header.help.xml");

/// `STRLEN`: the header's transform-name buffer.
const STRLEN: usize = 1024;

/// The program's streams, environment and working directory.
pub struct Run<'a, 'b> {
    pub out: &'a mut dyn Write,
    pub err: &'a mut dyn Write,
    pub env: &'b Environment,
    pub dir: Option<&'b std::path::Path>,
}

/// `mri_add_xform_to_header`'s entry point: its original arguments (without the program's name),
/// the settings, the two streams; the exit status. The notices of the run follow on standard error
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

/// `print_usage()`: the help of `mri_add_xform_to_header.help.xml`.
fn print_usage(g: &mut Globals, r: &mut Run) {
    output_help_xml(g, r.out, r.err, Some(HELP_XML));
}

/// `usage_exit()`: the help, then `exit(1)`.
fn usage_exit(g: &mut Globals, r: &mut Run) -> Fatal {
    print_usage(g, r);
    Fatal::Exit(1)
}

/// The settings the options give.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Options {
    /// `CopyNameOnly` (`-c`).
    pub copy_name_only: bool,
    /// `verbose` (`-v`, toggled).
    pub verbose: bool,
}

/// `get_option(argc, argv)` on `argv[1]` (`arg`): the arguments it takes after its own.
fn get_option(g: &mut Globals, r: &mut Run, o: &mut Options, arg: &str) -> Result<usize, Fatal> {
    let option = &arg.as_bytes()[1..];
    if option.eq_ignore_ascii_case(b"-help") || option.eq_ignore_ascii_case(b"-usage") {
        return Err(usage_exit(g, r));
    }
    match option.first().map(u8::to_ascii_uppercase) {
        Some(b'C') => o.copy_name_only = true,
        Some(b'V') => o.verbose = !o.verbose,
        Some(b'N') => return Ok(1),
        Some(b'?' | b'H' | b'U') => {
            print_usage(g, r);
            return Err(Fatal::Exit(1));
        }
        _ => {
            let _ = writeln!(r.err, "unknown option {arg}");
            return Err(Fatal::Exit(1));
        }
    }
    Ok(0)
}

/// `main(argc, argv)` on the arguments after the program's name. `Ok` is status 0.
pub fn main(g: &mut Globals, r: &mut Run, args: &[String]) -> Result<(), Fatal> {
    let argv: Vec<String> = std::iter::once(NAME.to_owned())
        .chain(args.iter().cloned())
        .collect();
    let (_, argv) = handle_version_option(g, r.env, r.out, &argv, NAME);
    g.progname = NAME.to_owned();
    diag_init(g, r.env, r.out);
    if argv.len() == 1 {
        return Err(usage_exit(g, r));
    }
    let mut o = Options::default();
    // `argc` and `argv[1]`'s place, as FreeSurfer steps them (an option taking an argument at the
    // end leaves `argc` below 2: no more is read)
    let mut argc = argv.len() as isize;
    let mut at = 1usize;
    while argc > 1 && argv[at].starts_with('-') {
        let nargs = get_option(g, r, &mut o, &argv[at])?;
        argc -= nargs as isize + 1;
        at += nargs + 1;
    }
    let progname = g.progname.clone();
    if argc < 2 {
        error_exit(
            g,
            r.err,
            ERROR_BADPARM,
            &format!("{progname}: no transform name specified"),
        )?;
    }
    let xform_fname = argv[at].clone();
    if argc < 3 {
        error_exit(
            g,
            r.err,
            ERROR_BADPARM,
            &format!("{progname}: no input name specified"),
        )?;
    }
    let in_fname = argv[at + 1].clone();
    let out_fname = if argc < 4 {
        in_fname.clone()
    } else {
        argv[at + 2].clone()
    };
    if o.verbose {
        let _ = write!(r.err, "reading from {in_fname}...");
    }
    let mut mri = None;
    if fio_is_directory(g, r.dir, &in_fname) != 0 {
        mri = mri_read_info(g, r.out, r.err, r.env, r.dir, &in_fname)?;
    } else if fio_file_exists_readable(g, r.dir, &in_fname) != 0 {
        let Some(ext) = fio_extension(Some(&in_fname)) else {
            error_exit(
                g,
                r.err,
                ERROR_BADPARM,
                &format!("{progname}: no extension found"),
            )?;
            unreachable!("ErrorExit returns only for code 0");
        };
        let _ = writeln!(r.out, "INFO: extension is {ext}");
        if ext == "mgz" || ext == "mgh" {
            mri = mri_read_volume(g, r.out, r.err, r.env, r.dir, &in_fname)?;
        } else {
            error_exit(
                g,
                r.err,
                ERROR_BADPARM,
                &format!("{progname}: currently only .mgz or .mgh saves transform name"),
            )?;
        }
    }
    let Some(mut m) = mri else {
        error_exit(
            g,
            r.err,
            ERROR_NO_FILE,
            &format!("{progname}: could not open source file {in_fname}"),
        )?;
        unreachable!("ErrorExit returns only for code 0");
    };
    if !o.copy_name_only {
        let (status, read) =
            input_transform_file_ex(g, r.out, r.err, r.dir, xform_fname.as_bytes());
        // volume_io assigns its argument as it reads: what it read stays, even after a failure
        if read.is_some() {
            m.transform = read;
        }
        if status.is_err() {
            error_printf(
                g,
                r.err,
                ERROR_NO_MEMORY,
                &format!("{progname}: could not read xform file '{xform_fname}'\n"),
            );
        }
        // a transform never read is the volume's zeroed one: a linear transform whose matrices
        // are null pointers
        let (lin, inv) = match &m.transform {
            Some(t) => (
                Some(get_linear_transform_ptr(r.err, t)?),
                Some(get_inverse_linear_transform_ptr(r.err, t)?),
            ),
            None => (None, None),
        };
        m.linear_transform = lin;
        m.inverse_linear_transform = inv;
        m.free_transform = 1;
    }
    if xform_fname.len() >= STRLEN {
        g.notices.add(
            &FS_1154,
            if g.notices.reproduce() {
                format!(
                    "mri_add_xform_to_header: a transform name of {} bytes passes the header's room (1,024 bytes): aborted, as FreeSurfer's build aborts",
                    xform_fname.len()
                )
            } else {
                format!(
                    "mri_add_xform_to_header: a transform name of {} bytes used whole (FreeSurfer's build aborts: the header holds 1,023)",
                    xform_fname.len()
                )
            },
        );
        if g.notices.reproduce() {
            let _ = r.err.write_all(BUFFER_OVERFLOW.as_bytes());
            return Err(Fatal::Exit(134));
        }
    }
    m.transform_fname = xform_fname.into_bytes();
    if o.verbose {
        let _ = write!(r.err, "done.\nwriting to {out_fname}...");
    }
    let status = if fio_is_directory(g, r.dir, &out_fname) != 0 {
        mri_write_info(g, r.err, r.dir, &m, &out_fname)?
    } else {
        mri_write(
            g,
            r.out,
            r.err,
            r.env,
            r.dir,
            &mut m,
            &out_fname,
            None,
            MGZ_INTENT_UNKNOWN,
        )?
    };
    if status != 0 {
        g.notices.add(
            &FS_1155,
            if g.notices.reproduce() {
                "mri_add_xform_to_header: the output was not written; status 0, as in FreeSurfer"
            } else {
                "mri_add_xform_to_header: the output was not written: status 1 (FreeSurfer ends with 0)"
            },
        );
        if g.notices.fix() {
            return Err(Fatal::Exit(1));
        }
    }
    if o.verbose {
        let _ = r.err.write_all(b"done.\n");
    }
    Ok(())
}
