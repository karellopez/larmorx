// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `fs_temp_file`'s `main` (`io/fs_temp_file.cpp` 13-31): the program's entry point [`run`]
//! (registered as `fs_temp_file` in the crate's `PROGRAMS`) and [`main`].
//!
//! `fs_temp_file [-s|--suffix <str>] [-b|--base <dir>] [--scratch]`, read by FreeSurfer's
//! `ArgumentParser` (its messages and statuses: `not enough inputs supplied to '<opt>'`,
//! `unexpected argument '<arg>'`, status 2; `-h`/`--help` the help of `fs_temp_file.help.xml`,
//! status 0; `--version` and `--all-info`, which end the program when they are its only
//! arguments). Then `--scratch` sets `FS_TMPDIR` to `/scratch` unless it is set (even empty),
//! `--base` sets it to its value; the suffix (empty without `--suffix`) is given to
//! `makeTempFile`: the first of `FS_TMPDIR`, `TMPDIR`, `TMP`, `TEMP`, `TEMPDIR` that is set and
//! names a folder a file can be made in, else `/tmp` (`Can not write to temporary directory
//! <dir>`, status 1, when the last variable is set and no folder is writable, or `/tmp` is not), a
//! new empty file `<dir>/tmp.XXXXXX<suffix>` (six random letters and digits, mode 0600:
//! `Could not create temporary file in <dir>`, status 1, when it cannot be made). Its name on
//! standard output, a new line; status 0.
//!
//! Defects (`larmorx_core::defects`; the mechanisms in `docs/findings/freesurfer-fs-temp-file.md`):
//! - **FS-1171**: a `--base` folder that does not exist, cannot be written or is empty is replaced
//!   without a message by the next variable's folder or `/tmp` (by default `Can not write to
//!   temporary directory <base>`, status 1; `reproduce` goes on as FreeSurfer does).

use std::io::Write;
use std::path::Path;

use larmorx_core::defects::FS_1171;
use larmorx_freesurfer_core::argparse::{ArgType, ArgumentParser, Value, output_help_xml};
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::{self, Fatal};
use larmorx_freesurfer_core::run::{Environment, Settings};
use larmorx_freesurfer_core::system;
use larmorx_freesurfer_core::utils::{directory_is_writable, make_temp_file};

/// The program's name as `argv[0]` gives it (FreeSurfer's scripts run it by name).
pub const NAME: &str = "fs_temp_file";

/// `fs_temp_file.help.xml`, the help `ArgumentParser` prints (`outputHelpXml`).
pub const HELP_XML: &[u8] = include_bytes!("fs_temp_file.help.xml");

/// `fs_temp_file`'s entry point: its original arguments (without the program's name), the
/// settings, the two streams; the exit status. The notices of the run follow on standard error
/// (`larmorx: ...`).
pub fn run(args: &[String], settings: &Settings, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let mut g = Globals::new(settings.mode);
    // the process's own standard error, as the C library's perror sees it
    g.stderr.read_write = system::stderr_read_write();
    let status = {
        let mut terr = log::TrackedErr::new(&g, &mut *err);
        match main(
            &mut g,
            &settings.env,
            settings.dir.as_deref(),
            &mut *out,
            &mut terr,
            args,
        ) {
            Ok(()) => 0,
            Err(f) => f.status(),
        }
    };
    let _ = out.flush();
    g.notices.print(err);
    status
}

/// A string argument's value (`retrieve<std::string>`).
fn retrieve_string(
    g: &mut Globals,
    err: &mut dyn Write,
    p: &ArgumentParser,
    name: &str,
) -> Result<String, Fatal> {
    match p.retrieve(g, err, name, "std::string")? {
        Value::Str(s) => Ok(s),
        _ => Ok(String::new()),
    }
}

/// `main(argc, argv)` on the arguments after the program's name, in the environment `env` and
/// the working folder `dir` (`None`: the process's).
pub fn main(
    g: &mut Globals,
    env: &Environment,
    dir: Option<&Path>,
    out: &mut dyn Write,
    err: &mut dyn Write,
    args: &[String],
) -> Result<(), Fatal> {
    let mut av = Vec::with_capacity(args.len() + 1);
    av.push(NAME.to_owned());
    av.extend(args.iter().cloned());
    let mut parser = ArgumentParser::new(g, err)?;
    parser.add_help(g, err, HELP_XML)?;
    parser.add_argument2(g, err, "-s", "--suffix", 1, ArgType::Unknown, false)?;
    parser.add_argument2(g, err, "-b", "--base", 1, ArgType::Unknown, false)?;
    parser.add_argument(g, err, "--scratch", 0, ArgType::Unknown, false)?;
    parser.parse(g, env, out, err, &av, output_help_xml)?;

    // setenv(name, value, overwrite) on the program's own environment
    let mut env = env.clone();
    if parser.exists(g, err, "scratch")? && env.get("FS_TMPDIR").is_none() {
        env.set("FS_TMPDIR", "/scratch");
    }
    if parser.exists(g, err, "base")? {
        let basedir = retrieve_string(g, err, &parser, "base")?;
        if basedir.is_empty() || !directory_is_writable(dir, &basedir) {
            g.notices.add(
                &FS_1171,
                if g.notices.reproduce() {
                    format!(
                        "fs_temp_file: the base '{basedir}' cannot be written: another folder used without a message, as in FreeSurfer"
                    )
                } else {
                    format!(
                        "fs_temp_file: the base '{basedir}' cannot be written: an error (FreeSurfer uses another folder without a message)"
                    )
                },
            );
            if !g.notices.reproduce() {
                log::fatal(
                    g,
                    err,
                    1,
                    &format!("Can not write to temporary directory {basedir}"),
                )?;
            }
        }
        env.set("FS_TMPDIR", basedir);
    }
    let suffix = retrieve_string(g, err, &parser, "suffix")?;
    let tempfile = make_temp_file(g, &env, dir, err, &suffix)?;
    let _ = out.write_all(tempfile.as_bytes());
    let _ = out.write_all(b"\n");
    Ok(())
}
