// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! FreeSurfer's environment record (`fs.core`, queue item 3.1-22), translated from FreeSurfer
//! 8.2.0's `utils/fsenv.cpp` (`FSENVgetenv`, `FSENVfree`, `FSENVgetSUBJECTS_DIR`,
//! `FSENVsetSUBJECTS_DIR`) and `include/fsenv.h`. The colour table `FSENVgetenv` loads
//! (`CTABreadASCII`, `fs.labels`, queue item 3.1-33) is read by the function the caller passes.
//!
//! **FS-08**: `FSENVgetenv` reads memory it never wrote in two cases: `FS_DESIRED_BVEC_SPACE`
//! without a number leaves its `int` unset before the check (`sscanf(pc, "%d", &b)`), and a
//! working directory `getcwd` cannot give leaves its buffer unset after `ERROR: getcwd returned no
//! path` (no newline). Both modes fill them deterministically: the space as 0 (so the error
//! `ERROR: FS_DESIRED_BVEC_SPACE = <value>, must be 1 or 2` and `None` follow, as for any other
//! wrong number) and the directory as empty, with a notice.

use std::io::Write;
use std::path::Path;

use larmorx_core::defects::FS_08;

use crate::globals::Globals;
use crate::run::Environment;
use crate::system;
use crate::utils::scan_d;
use crate::version;

/// `BVEC_SPACE_SCANNER`, `BVEC_SPACE_VOXEL` (`include/mri.h`).
pub const BVEC_SPACE_SCANNER: i32 = 1;
pub const BVEC_SPACE_VOXEL: i32 = 2;

/// `FSENV`, with the colour table's type `C`.
#[derive(Clone, Debug, PartialEq)]
pub struct Fsenv<C> {
    pub freesurfer_home: String,
    pub subjects_dir: Option<String>,
    /// Set only when `SUBJECTS_DIR` is (FreeSurfer's order).
    pub user: Option<String>,
    pub date: String,
    pub cwd: String,
    pub hostname: String,
    pub sysname: String,
    pub machine: String,
    pub ctab: C,
    pub desired_bvec_space: i32,
}

fn var(env: &Environment, name: &str) -> Option<String> {
    env.get(name).map(|v| v.to_string_lossy().into_owned())
}

/// `FSENVgetenv()`: `FREESURFER_HOME` (else `FREESURFER_HOME not defined` and `None`),
/// `SUBJECTS_DIR` (else `SUBJECTS_DIR not defined`, kept unset), the user, the directory, the
/// machine's names, the colour table `$FREESURFER_HOME/FreeSurferColorLUT.txt` (else `ERROR:
/// reading <file>` and `None`), the time stamp and `FS_DESIRED_BVEC_SPACE` (1 or 2, default 2).
/// Messages on standard output.
pub fn fsenv_getenv<C>(
    g: &mut Globals,
    env: &Environment,
    dir: Option<&Path>,
    out: &mut dyn Write,
    read_ctab: impl FnOnce(&mut Globals, &mut dyn Write, &str) -> Option<C>,
) -> Option<Fsenv<C>> {
    let Some(home) = var(env, "FREESURFER_HOME") else {
        let _ = writeln!(out, "FREESURFER_HOME not defined");
        return None;
    };
    let (subjects_dir, user) = match var(env, "SUBJECTS_DIR") {
        Some(s) => (Some(s), Some(version::ver_user(env))),
        None => {
            let _ = writeln!(out, "SUBJECTS_DIR not defined");
            (None, None)
        }
    };
    let cwd = match system::cwd(dir) {
        Some(c) => c,
        None => {
            let _ = write!(out, "ERROR: getcwd returned no path");
            g.notices.add(
                &FS_08,
                "the working directory could not be read: empty, not memory never written",
            );
            String::new()
        }
    };
    let (sysname, machine, hostname) = match system::uname(env) {
        Some(u) => (u.sysname, u.machine, u.nodename),
        None => (String::new(), String::new(), String::new()),
    };
    let ctab_file = format!("{home}/FreeSurferColorLUT.txt");
    let Some(ctab) = read_ctab(g, out, &ctab_file) else {
        let _ = writeln!(out, "ERROR: reading {ctab_file}");
        return None;
    };
    let date = version::ver_cur_time_stamp();
    let desired_bvec_space = match var(env, "FS_DESIRED_BVEC_SPACE") {
        Some(pc) => {
            let b = scan_d(pc.as_bytes()).unwrap_or_else(|| {
                g.notices.add(
                    &FS_08,
                    "FS_DESIRED_BVEC_SPACE holds no number: taken as 0, not memory never written",
                );
                0
            });
            if b != BVEC_SPACE_SCANNER && b != BVEC_SPACE_VOXEL {
                let _ = writeln!(
                    out,
                    "ERROR: FS_DESIRED_BVEC_SPACE = {pc}, must be {BVEC_SPACE_SCANNER} or {BVEC_SPACE_VOXEL}"
                );
                return None;
            }
            b
        }
        None => BVEC_SPACE_VOXEL,
    };
    Some(Fsenv {
        freesurfer_home: home,
        subjects_dir,
        user,
        date,
        cwd,
        hostname,
        sysname,
        machine,
        ctab,
        desired_bvec_space,
    })
}

/// `FSENVgetSUBJECTS_DIR()`: the variable, or `FSENVgetSUBJECTS_DIR: SUBJECTS_DIR not defined`
/// on standard output and `None`.
pub fn fsenv_get_subjects_dir(env: &Environment, out: &mut dyn Write) -> Option<String> {
    let s = var(env, "SUBJECTS_DIR");
    if s.is_none() {
        let _ = writeln!(out, "FSENVgetSUBJECTS_DIR: SUBJECTS_DIR not defined");
    }
    s
}

/// `FSENVsetSUBJECTS_DIR(dir)`: sets the program's `SUBJECTS_DIR`; 0.
pub fn fsenv_set_subjects_dir(env: &mut Environment, subjects_dir: &str) -> i32 {
    env.set("SUBJECTS_DIR", subjects_dir);
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use larmorx_core::defects::UpstreamDefects;

    fn run(pairs: &[(&str, &str)], ctab: bool) -> (Option<Fsenv<u8>>, String, Globals) {
        let env = Environment::from_pairs(pairs.iter().copied());
        let mut g = Globals::new(UpstreamDefects::Fix);
        let mut out = Vec::new();
        let r = fsenv_getenv(&mut g, &env, Some(Path::new("/w")), &mut out, |_, _, f| {
            assert_eq!(f, "/fs/FreeSurferColorLUT.txt");
            ctab.then_some(7u8)
        });
        (r, String::from_utf8(out).unwrap(), g)
    }

    #[test]
    fn the_record_and_its_messages() {
        let (r, out, _) = run(&[], true);
        assert!(r.is_none());
        assert_eq!(out, "FREESURFER_HOME not defined\n");
        let (r, out, _) = run(&[("FREESURFER_HOME", "/fs")], true);
        let r = r.unwrap();
        assert_eq!(out, "SUBJECTS_DIR not defined\n");
        assert_eq!((r.subjects_dir, r.user, r.cwd.as_str()), (None, None, "/w"));
        assert_eq!((r.ctab, r.desired_bvec_space), (7, 2));
        let (r, _, _) = run(&[("FREESURFER_HOME", "/fs"), ("SUBJECTS_DIR", "/s")], true);
        assert!(r.unwrap().user.is_some());
        let (r, out, _) = run(&[("FREESURFER_HOME", "/fs"), ("SUBJECTS_DIR", "/s")], false);
        assert!(r.is_none());
        assert_eq!(out, "ERROR: reading /fs/FreeSurferColorLUT.txt\n");
    }

    #[test]
    fn bvec_space_and_fs08() {
        let base = [("FREESURFER_HOME", "/fs"), ("SUBJECTS_DIR", "/s")];
        let with = |v: &'static str| {
            let mut p = base.to_vec();
            p.push(("FS_DESIRED_BVEC_SPACE", v));
            run(&p, true)
        };
        assert_eq!(with(" 1x").0.unwrap().desired_bvec_space, 1);
        let (r, out, g) = with("3");
        assert!(r.is_none() && g.notices.is_empty());
        assert_eq!(out, "ERROR: FS_DESIRED_BVEC_SPACE = 3, must be 1 or 2\n");
        let (r, out, g) = with("abc");
        assert!(r.is_none() && g.notices.contains(&FS_08));
        assert_eq!(out, "ERROR: FS_DESIRED_BVEC_SPACE = abc, must be 1 or 2\n");
        // glibc's %d keeps the low 32 bits of strtol's value.
        assert_eq!(with("4294967298").0.unwrap().desired_bvec_space, 2);
    }

    #[test]
    fn subjects_dir() {
        let mut env = Environment::from_pairs::<&str, &str>([]);
        let mut out = Vec::new();
        assert_eq!(fsenv_get_subjects_dir(&env, &mut out), None);
        assert_eq!(fsenv_set_subjects_dir(&mut env, "/s"), 0);
        assert_eq!(
            fsenv_get_subjects_dir(&env, &mut out).as_deref(),
            Some("/s")
        );
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "FSENVgetSUBJECTS_DIR: SUBJECTS_DIR not defined\n"
        );
    }
}
