// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Bruker's format probe as FreeSurfer 8.2.0 makes it (`utils/Bruker.cpp`, queue item 3.1-27):
//! `checkBrukerFiles` and `is_bruker`. Bruker's readers come with the rest of the item.

use std::io::Write;
use std::path::Path;

use larmorx_core::defects::FS_221;
use larmorx_freesurfer_core::globals::Globals;

use crate::identify::c_stat;

/// A study file's part of the name, its message, and where its name goes.
type Part = (&'static str, &'static str, fn(&mut BrukerFiles) -> &mut String);

/// The bytes of each file name `is_bruker` builds (`char methodFile[512]` and the others).
const BRUKER_NAME: usize = 512;

/// The five files of a Bruker study directory, as `checkBrukerFiles` names them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BrukerFiles {
    pub method: String,
    pub acqp: String,
    pub data: String,
    pub d3proc: String,
    pub reco: String,
}

/// `checkBrukerFiles(fname, ...)`: 1 when `fname` is a directory holding `method`, `acqp`,
/// `pdata/1/2dseq`, `pdata/1/d3proc` and `pdata/1/reco` (each found by `stat`, in that order); else
/// 0 at the first missing one (`errno` as `stat` left it). With `flag` set it reports each step:
/// `ERROR: could not stat <f>.` or `ERROR: <f> is not a directory.` (standard error), `INFO:
/// directory <f> ` and the found files' lines (standard output; the reco file's line reads
/// `d3proc`, as FreeSurfer prints it). The names are built as `fname` and the file's part; one of
/// 512 bytes or more overflows FreeSurfer's buffer (FS-221: used whole).
pub fn check_bruker_files(
    g: &mut Globals,
    dir: Option<&Path>,
    out: &mut dyn Write,
    err: &mut dyn Write,
    fname: &str,
    files: &mut BrukerFiles,
    flag: bool,
) -> i32 {
    match c_stat(dir, fname) {
        Err(e) => {
            g.errno = e;
            if flag {
                let _ = writeln!(err, "ERROR: could not stat {fname}.");
            }
            return 0;
        }
        Ok(m) if !m.is_dir() => {
            if flag {
                let _ = writeln!(err, "ERROR: {fname} is not a directory.");
            }
            return 0;
        }
        Ok(_) => {}
    }
    if flag {
        let _ = writeln!(out, "INFO: directory {fname} ");
    }
    let parts: [Part; 5] = [
        ("/method", "INFO:    method {}\n", |f| &mut f.method),
        ("/acqp", "INFO:      acqp {}\n", |f| &mut f.acqp),
        ("/pdata/1/2dseq", "INFO:     2dseq {} \n", |f| &mut f.data),
        ("/pdata/1/d3proc", "INFO:    d3proc {} \n", |f| {
            &mut f.d3proc
        }),
        ("/pdata/1/reco", "INFO:    d3proc {} \n", |f| &mut f.reco),
    ];
    for (part, line, slot) in parts {
        let name = format!("{fname}{part}");
        if name.len() >= BRUKER_NAME {
            g.notices.add(
                &FS_221,
                format!(
                    "checkBrukerFiles: a name of {} bytes used whole (FreeSurfer's buffer holds {BRUKER_NAME})",
                    name.len()
                ),
            );
        }
        *slot(files) = name.clone();
        if let Err(e) = c_stat(dir, &name) {
            g.errno = e;
            if flag {
                let _ = writeln!(err, "ERROR: could not stat {name}.");
            }
            return 0;
        }
        if flag {
            let _ = out.write_all(line.replace("{}", &name).as_bytes());
        }
    }
    1
}

/// `is_bruker(fname)`: 1 when `fname` is a directory that [`check_bruker_files`] accepts (no
/// messages); 0 otherwise (`errno` as `stat` left it).
pub fn is_bruker(g: &mut Globals, dir: Option<&Path>, fname: &str) -> i32 {
    match c_stat(dir, fname) {
        Err(e) => {
            g.errno = e;
            0
        }
        Ok(m) if !m.is_dir() => 0,
        Ok(_) => {
            let mut files = BrukerFiles::default();
            check_bruker_files(
                g,
                dir,
                &mut std::io::sink(),
                &mut std::io::sink(),
                fname,
                &mut files,
                false,
            )
        }
    }
}
