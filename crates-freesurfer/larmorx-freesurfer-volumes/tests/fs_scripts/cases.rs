// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The cases of `seg2cc`, `mergeseg` and `fscalc` against FreeSurfer 8.2.0's official scripts:
//! a fixture written by the port (32^3 coronal volumes made with integer arithmetic, NaNs as bit
//! patterns; a stand-in `FREESURFER_HOME` with `sources.csh`, `build-stamp.txt` and a small colour
//! table; expert-options files), the cases (the scripts' options, messages, failures and the
//! names tcsh reads again), and the canonical text of a run: its status, both streams and every
//! file the folder then holds (volumes by geometry and voxels, logs as text), with what changes
//! from run to run masked (the folder, dates, `fs_time`'s lines, run times, `uname -a`, the
//! machine's and user's names, process numbers, temporary names, larmorx's notices). Shared by
//! the tests `fs_scripts_official` and `scripts_golden` and the example `fs_scripts_probe`, with
//! which `validation/probes/fs-scripts/check.py` runs the official scripts on the same cases.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

use larmorx_core::defects::UpstreamDefects;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::run::{Environment, Settings};
use larmorx_freesurfer_io::mriio::{mri_read_volume, mri_write};
use larmorx_freesurfer_mri::mri::{MRI_FLOAT, MRI_INT, MRI_UCHAR, Mri, mri_alloc};
use sha2::{Digest, Sha256};

/// `sources.csh` of FreeSurfer 8.2.0's installation (its text).
pub const SOURCES_CSH: &str = "#!/bin/tcsh -ef\n\n# Call configuration script:\n\nif (\"`uname -s`\" == \"Darwin\") then\n  source $FREESURFER_HOME/SetUpFreeSurfer.csh\nendif\n";
/// `build-stamp.txt` of the reference installation.
pub const BUILD_STAMP: &str = "freesurfer-linux-ubuntu22_x86_64-8.2.0-20260314-d932c45\n";
/// The stand-in colour table (`FreeSurferColorLUT.txt` and `vol/ctab.txt`).
pub const CTAB: &str = "0 Unknown 0 0 0 0\n2 Left-Cerebral-White-Matter 245 245 245 0\n3 Left-Cerebral-Cortex 205 62 78 0\n41 Right-Cerebral-White-Matter 0 225 0 0\n42 Right-Cerebral-Cortex 205 62 78 0\n251 CC_Posterior 0 0 64 0\n252 CC_Mid_Posterior 0 0 112 0\n253 CC_Central 0 0 160 0\n254 CC_Mid_Anterior 0 0 208 0\n255 CC_Anterior 0 0 255 0\n";

/// The fixture's files' modification time (seconds since 1970).
const T0: u64 = 1_700_000_000;

/// One case: the script, its arguments and environment changes (`<D>`: the case's folder; `None`
/// unsets), the folder it starts in (relative to the case's), and whether it is a heavy case (a
/// 256^3 conformation: the probe's, never the tests').
#[derive(Clone, Debug)]
pub struct Case {
    pub prog: &'static str,
    pub args: Vec<String>,
    pub env: Vec<(&'static str, Option<String>)>,
    pub cwd: &'static str,
    pub heavy: bool,
    /// A case that ends before the scripts run a program on a volume (or nearly): the recorded
    /// test's set in a debug build.
    pub quick: bool,
}

fn c(prog: &'static str, args: &[&str]) -> Case {
    Case {
        prog,
        args: args.iter().map(|s| s.to_string()).collect(),
        env: Vec::new(),
        cwd: "",
        heavy: false,
        quick: false,
    }
}

impl Case {
    fn env(mut self, k: &'static str, v: Option<&str>) -> Case {
        self.env.push((k, v.map(str::to_owned)));
        self
    }
    fn cwd(mut self, d: &'static str) -> Case {
        self.cwd = d;
        self
    }
    fn heavy(mut self) -> Case {
        self.heavy = true;
        self
    }
    fn quick(mut self) -> Case {
        self.quick = true;
        self
    }
}

/// Every case.
pub fn cases() -> Vec<Case> {
    let mut v = Vec::new();
    // seg2cc: the command line
    for a in [
        &[][..],
        &["--help"],
        &["-help"],
        &["--s", "sub-helper"],
        &["--version"],
        &["-version"],
        &["--s", "x--version"],
        &["--x"],
        &["--s", "s", "--bogus", "1"],
        &["-n", "x"],
        &["--s"],
        &["--cc"],
        &["--nocc"],
        &["--n"],
        &["--ctab"],
        &["--threads"],
        &["--expert"],
        &["--log"],
        &["--tmp"],
        &["--tmpdir"],
        &["--cc", "x"],
        &["--s", "m"],
        &["--s", "none"],
        &["--s", "u"],
        &["--s", "s", "--expert", "xopts.txt"],
        &["--s", "s", "--expert", "xopts2.txt"],
        &["--s", "s", "--expert", "xopts3.txt"],
        &["--s", "s", "--expert", "nofile"],
    ] {
        v.push(c("seg2cc", a).quick());
    }
    // seg2cc: the conformed path, to mri_cc (which stops: the segmentation holds a callosum label)
    for a in [
        &["--s", "s"][..],
        &["--s", "u", "--force-update"],
        &["--s", "o"],
        &["--s", "s", "-lh"],
        &["--s", "s", "-rh"],
        &["--s", "s", "--lh-only", "--rh-only"],
        &["--s", "s", "-lh-only"],
        &["--s", "s", "-rh-only"],
        &["--s", "s", "--cc", "outcc"],
        &["--s", "s", "--nocc", "other"],
        &["--s", "s", "--n", "norm2"],
        &["--s", "s", "--n", "missing"],
        &["--s", "s", "--nolog"],
        &["--s", "s", "--no-log"],
        &["--s", "s", "--log", "<D>/my.log"],
        &["--s", "s", "--log", "rel.log"],
        &["--s", "s", "--log", "<D>/nofolder/x.log"],
        &["--s", "s", "--log", "<D>/subjects"],
        &["--s", "s", "--tmpdir", "<D>/t1"],
        &["--s", "s", "--tmp", "t2"],
        &["--s", "s", "--nocleanup"],
        &["--s", "s", "--nocleanup", "--cleanup"],
        &["--s", "s", "--no-force-update"],
        &["--s", "s", "--threads", "2"],
        &["--s", "s", "--ctab", "<D>/vol/ctab.txt"],
        &["--s", "s", "--debug"],
        &["--s", "b"],
    ] {
        v.push(c("seg2cc", a));
    }
    v.push(c("seg2cc", &["--s", "s"]).env("SUBJECTS_DIR", None).quick());
    v.push(
        c("seg2cc", &["--s", "s"])
            .env("FREESURFER_HOME", None)
            .quick(),
    );
    v.push(
        c("seg2cc", &["--help"])
            .env("FREESURFER_HOME", None)
            .quick(),
    );
    v.push(c("seg2cc", &["--s", "s"]).env("PBS_JOBID", Some("job7")));
    v.push(c("seg2cc", &["--s", "s"]).env("FS_TIME_ALLOW", Some("0")));
    v.push(c("seg2cc", &["--s", "s"]).env("FSTIME_LOAD", Some("0")));
    v.push(c("seg2cc", &["--s", "s"]).env("HOME", Some("/nonexistent-home")));
    v.push(c("seg2cc", &["--s", "s"]).env("SUBJECTS_DIR", Some("subjects")));
    v.push(
        c("seg2cc", &["--s", "s"])
            .env("SUBJECTS_DIR", Some("../subjects"))
            .cwd("vol"),
    );
    v.push(c("seg2cc", &["--s", "s"]).cwd("subjects/s/mri"));
    // names tcsh reads again (FS-1277, FS-1278)
    for a in [
        &["--s", "a b"][..],
        &["--s", "s*"],
        &["--s", "s", "--log", "x\\ty.log"],
        &["--s", ""],
    ] {
        v.push(c("seg2cc", a));
    }
    // seg2cc: the path for a segmentation not conformed (heavy: a 256^3 conformation)
    v.push(c("seg2cc", &["--s", "n"]).heavy());
    v.push(c("seg2cc", &["--s", "n", "--nocleanup"]).heavy());

    // mergeseg
    let m = |a: &[&str]| c("mergeseg", a);
    for a in [
        &[][..],
        &["--help"],
        &["--version"],
        &[
            "--src",
            "vol/src.mgz",
            "--merge",
            "vol/merge.mgz",
            "--o",
            "o/x-help.mgz",
        ],
        &["--x"],
        &["--src"],
        &["--merge"],
        &["--o"],
        &["--segid"],
        &["--i"],
        &["--segid-only"],
        &["--segid-erode"],
        &["--ctab"],
        &["--log"],
        &["--tmpdir"],
        &["--src", "vol/none.mgz"],
        &["--merge", "vol/none.mgz"],
        &["--merge", "vol/merge.mgz", "--o", "o/a.mgz"],
        &["--src", "vol/src.mgz", "--o", "o/a.mgz"],
        &["--src", "vol/src.mgz", "--merge", "vol/merge.mgz"],
        &[
            "--src",
            "vol/src.mgz",
            "--merge",
            "vol/merge.mgz",
            "--o",
            "o/a.mgz",
        ],
        &[
            "--src",
            "vol/src.mgz",
            "--merge",
            "vol/merge.mgz",
            "--o",
            "a.mgz",
        ],
        &[
            "--src",
            "vol/src.mgz",
            "--merge",
            "vol/merge.mgz",
            "--o",
            "o/a.mgz",
            "--segid",
            "7",
        ],
        &[
            "--src",
            "vol/src.mgz",
            "--merge",
            "vol/merge.mgz",
            "--o",
            "o/a.mgz",
            "--i",
            "9",
        ],
        &[
            "--src",
            "vol/src.mgz",
            "--merge",
            "vol/merge.mgz",
            "--o",
            "o/a.mgz",
            "--segid-only",
            "2",
        ],
        &[
            "--src",
            "vol/src.mgz",
            "--merge",
            "vol/merge.mgz",
            "--o",
            "o/a.mgz",
            "--segid-only",
            "2",
            "--segid-erode",
            "1",
        ],
        &[
            "--src",
            "vol/src.mgz",
            "--merge",
            "vol/merge.mgz",
            "--o",
            "o/a.mgz",
            "--segid-only",
            "2",
            "--segid-erode",
            "x",
        ],
        &[
            "--src",
            "vol/src.mgz",
            "--merge",
            "vol/merge.mgz",
            "--o",
            "o/a.mgz",
            "--segid-erode",
            "2",
        ],
        &[
            "--src",
            "vol/src.mgz",
            "--merge",
            "vol/merge.mgz",
            "--o",
            "o/a.mgz",
            "--segid",
            "251",
            "--ctab",
            "vol/ctab.txt",
        ],
        &[
            "--src",
            "vol/src.mgz",
            "--merge",
            "vol/merge.mgz",
            "--o",
            "o/a.mgz",
            "--log",
            "m.log",
        ],
        &[
            "--src",
            "vol/src.mgz",
            "--merge",
            "vol/merge.mgz",
            "--o",
            "o/a.mgz",
            "--nolog",
        ],
        &[
            "--src",
            "vol/src.mgz",
            "--merge",
            "vol/merge.mgz",
            "--o",
            "o/a.mgz",
            "--tmpdir",
            "tm",
        ],
        &[
            "--src",
            "vol/src.mgz",
            "--merge",
            "vol/merge.mgz",
            "--o",
            "o/a.mgz",
            "--nocleanup",
        ],
        &[
            "--src",
            "vol/src.mgz",
            "--merge",
            "vol/mergef.mgh",
            "--o",
            "o/f.mgz",
        ],
        &[
            "--src",
            "vol/mergef.mgh",
            "--merge",
            "vol/merge.mgz",
            "--o",
            "o/g.nii.gz",
        ],
        &[
            "--src",
            "vol/src.mgz",
            "--merge",
            "vol/small.mgz",
            "--o",
            "o/a.mgz",
        ],
        &[
            "--src",
            "vol/src.mgz",
            "--merge",
            "vol/text.mgz",
            "--o",
            "o/a.mgz",
        ],
        &[
            "--src",
            "vol/src.mgz",
            "--merge",
            "vol/merge.mgz",
            "--o",
            "o/a.mgz",
            "--debug",
        ],
        &[
            "--src",
            "vol/src.mgz",
            "--merge",
            "vol/merge.mgz",
            "--o",
            "o/a b.mgz",
        ],
    ] {
        // the cases that stop before the merge's programs
        let full = a.contains(&"--src") && a.contains(&"--merge") && a.contains(&"--o");
        let missing = a.iter().any(|w| w.contains("none.mgz"));
        let mut case = m(a);
        if !full || missing {
            case = case.quick();
        }
        v.push(case);
    }
    let base = [
        "--src",
        "vol/src.mgz",
        "--merge",
        "vol/merge.mgz",
        "--o",
        "o/a.mgz",
    ];
    v.push(m(&base).env("SUBJECTS_DIR", None));
    v.push(m(&base).env("FREESURFER_HOME", None).quick());
    v.push(m(&["--help"]).env("FREESURFER_HOME", None).quick());
    v.push(m(&base).env("FREESURFER_HOME", Some("<D>/vol")).quick());

    // fscalc
    let f = |a: &[&str]| c("fscalc", a);
    for a in [
        &[][..],
        &["--help"],
        &["--version"],
        &["vol/a.mgz"],
        &["vol/a.mgz", "add"],
        &["vol/a.mgz", "add", "vol/b.mgz"],
        &["--o"],
        &["--odt"],
        &["--log"],
        &["--tmp"],
        &["vol/a.mgz", "add", "vol/b.mgz", "-o", "o/s.mgz"],
        &["vol/a.mgz", "sum", "vol/b.mgz", "--o", "o/s.mgz"],
        &["--o", "o/s.mgz", "vol/a.mgz", "-mul", "2"],
        &["vol/a.mgz", "mul", "vol/b.mgz", "sqrt", "--o", "o/s.mgz"],
        &["vol/a.mgz", "sqr", "--o", "o/s.mgz"],
        &["vol/a.mgz", "abs", "--o", "o/s.mgz"],
        &[
            "vol/a.mgz",
            "sub",
            "vol/b.mgz",
            "div",
            "vol/a.mgz",
            "--o",
            "o/s.mgz",
        ],
        &["vol/a.mgz", "gt", "1", "--o", "o/s.mgz"],
        &["vol/a.mgz", "bogus", "--o", "o/s.mgz"],
        &["vol/a.mgz", "add", "--o", "o/s.mgz"],
        &["vol/a.mgz", "-n", "--o", "o/s.mgz"],
        &["100", "add", "vol/a.mgz", "--o", "o/s.mgz"],
        &["100", "add", "200", "--o", "o/s.mgz"],
        &["vol/none.mgz", "add", "vol/a.mgz", "--o", "o/s.mgz"],
        &[
            "vol/a.mgz",
            "add",
            "vol/b.mgz",
            "--o",
            "o/s.mgz",
            "--odt",
            "uchar",
        ],
        &["vol/a.mgz", "add", "vol/b.mgz", "--o", "o/s.mgz", "--float"],
        &[
            "vol/a.mgz",
            "add",
            "vol/b.mgz",
            "--o",
            "o/s.mgz",
            "--no-run",
        ],
        &[
            "vol/a.mgz",
            "add",
            "vol/b.mgz",
            "--o",
            "o/s.mgz",
            "--tmpdir",
            "ft",
        ],
        &[
            "vol/a.mgz",
            "add",
            "vol/b.mgz",
            "--o",
            "o/s.mgz",
            "--nocleanup",
        ],
        &[
            "vol/a.mgz",
            "add",
            "vol/b.mgz",
            "--o",
            "o/s.mgz",
            "--log",
            "f.log",
        ],
        &["vol/a.mgz", "add", "vol/b.mgz", "--o", "o/s.mgz", "--nolog"],
        &["vol/a.mgz", "add", "vol/mergef.mgh", "--o", "o/n.mgz"],
        &["vol/a.mgz", "add", "vol/small.mgz", "--o", "o/s.mgz"],
        &["vol/a.mgz", "add", "vol/b.mgz", "--o", "o/s.mgz", "--debug"],
    ] {
        // the cases without an output or an operation stop before any program
        let mut case = f(a);
        if !a.iter().any(|w| w.starts_with("o/")) || a.len() < 2 {
            case = case.quick();
        }
        v.push(case);
    }
    v.push(
        f(&["vol/a.mgz", "add", "vol/b.mgz", "--o", "o/s.mgz"])
            .env("FREESURFER_HOME", None)
            .quick(),
    );
    v
}

fn io() -> (std::io::Sink, std::io::Sink) {
    (std::io::sink(), std::io::sink())
}

/// A volume of `mtype` and extent `n` whose voxel holds the bits `f(x, y, z)`, coronal 1 mm voxels
/// centred on the origin.
fn volume(g: &mut Globals, mtype: i32, n: [i32; 3], f: impl Fn(i32, i32, i32) -> u32) -> Mri {
    let (mut o, mut e) = io();
    let mut m = mri_alloc(g, &mut o, &mut e, n[0], n[1], n[2], mtype).expect("a volume");
    m.vg.xsize = 1.0;
    m.vg.ysize = 1.0;
    m.vg.zsize = 1.0;
    [m.vg.x_r, m.vg.x_a, m.vg.x_s] = [-1.0, 0.0, 0.0];
    [m.vg.y_r, m.vg.y_a, m.vg.y_s] = [0.0, 0.0, -1.0];
    [m.vg.z_r, m.vg.z_a, m.vg.z_s] = [0.0, 1.0, 0.0];
    let bytes = m.bytes_per_vox;
    let c = m.chunk.as_mut().expect("voxels");
    for z in 0..n[2] {
        for y in 0..n[1] {
            for x in 0..n[0] {
                let i = ((z * n[1] + y) * n[0] + x) as usize * bytes;
                let v = f(x, y, z);
                if bytes == 1 {
                    c[i] = v as u8;
                } else {
                    c[i..i + 4].copy_from_slice(&v.to_le_bytes());
                }
            }
        }
    }
    m
}

/// The segmentation's labels: left white matter and cortex (2, 3) for x below 16, right (41,
/// 42) above, the cortex on the slab's outer shell, and one voxel of the callosum's label 253
/// (mri_cc then stops at once with status 77).
fn aseg_value(x: i32, y: i32, z: i32) -> u32 {
    if (x, y, z) == (16, 12, 16) {
        return 253;
    }
    let inside = (4..28).contains(&y) && (6..26).contains(&z) && (3..29).contains(&x);
    if !inside {
        return 0;
    }
    let shell = y == 4 || y == 27 || z == 6 || z == 25 || x == 3 || x == 28;
    match (x < 16, shell) {
        (true, false) => 2,
        (true, true) => 3,
        (false, false) => 41,
        (false, true) => 42,
    }
}

fn norm_value(x: i32, y: i32, z: i32) -> u32 {
    if aseg_value(x, y, z) == 0 {
        0
    } else {
        (60 + (x * 3 + y * 5 + z * 7) % 61) as u32
    }
}

/// The merge volume: 0 but a block, where 1, 2, 3 or 251.
fn merge_value(x: i32, y: i32, z: i32) -> u32 {
    if !((10..20).contains(&x) && (8..22).contains(&y) && (9..23).contains(&z)) {
        return 0;
    }
    [1, 2, 3, 251][((x + 2 * y + 3 * z) % 4) as usize]
}

/// Writes a volume with the port's writer.
fn put(g: &mut Globals, m: &mut Mri, p: &Path) {
    let (mut o, mut e) = io();
    std::fs::create_dir_all(p.parent().expect("a folder")).expect("a folder");
    mri_write(
        g,
        &mut o,
        &mut e,
        &Environment::default(),
        None,
        m,
        p.to_str().expect("a UTF-8 name"),
        None,
        -1,
    )
    .expect("written");
}

/// Sets a file's modification time to `T0 + dt` seconds.
fn touch(p: &Path, dt: i64) {
    let t = std::time::UNIX_EPOCH + std::time::Duration::from_secs((T0 as i64 + dt) as u64);
    std::fs::File::options()
        .write(true)
        .open(p)
        .expect("the file")
        .set_modified(t)
        .expect("a time");
}

/// Writes the fixture into `dir`.
pub fn write_fixture(dir: &Path) {
    let mut g = Globals::new(UpstreamDefects::Reproduce);
    let n = [32, 32, 32];
    let mut aseg = volume(&mut g, MRI_INT, n, aseg_value);
    let mut norm = volume(&mut g, MRI_UCHAR, n, norm_value);
    let fsh = dir.join("fsh");
    std::fs::create_dir_all(&fsh).expect("a folder");
    std::fs::write(fsh.join("sources.csh"), SOURCES_CSH).expect("written");
    std::fs::write(fsh.join("build-stamp.txt"), BUILD_STAMP).expect("written");
    std::fs::write(fsh.join("FreeSurferColorLUT.txt"), CTAB).expect("written");
    let subj = dir.join("subjects");
    let mut files: Vec<(PathBuf, i64)> = Vec::new();
    for s in ["s", "u", "o"] {
        let mri = subj.join(s).join("mri");
        put(&mut g, &mut aseg, &mri.join("aseg.auto_noCCseg.mgz"));
        put(&mut g, &mut norm, &mri.join("norm.mgz"));
        files.push((mri.join("aseg.auto_noCCseg.mgz"), 0));
        files.push((mri.join("norm.mgz"), 0));
    }
    let s_mri = subj.join("s").join("mri");
    put(&mut g, &mut aseg, &s_mri.join("other.mgz"));
    put(&mut g, &mut norm, &s_mri.join("norm2.mgz"));
    files.push((s_mri.join("other.mgz"), 0));
    files.push((s_mri.join("norm2.mgz"), 0));
    for (s, dt) in [("u", 100), ("o", -100)] {
        let p = subj.join(s).join("mri").join("aseg.auto.mgz");
        put(&mut g, &mut aseg, &p);
        files.push((p, dt));
    }
    // m: no segmentation; b: a segmentation that does not read
    let m_mri = subj.join("m").join("mri");
    put(&mut g, &mut norm, &m_mri.join("norm.mgz"));
    files.push((m_mri.join("norm.mgz"), 0));
    let b_mri = subj.join("b").join("mri");
    put(&mut g, &mut norm, &b_mri.join("norm.mgz"));
    std::fs::write(b_mri.join("aseg.auto_noCCseg.mgz"), "").expect("written");
    files.push((b_mri.join("norm.mgz"), 0));
    files.push((b_mri.join("aseg.auto_noCCseg.mgz"), 0));
    // n: not conformed to its smallest voxel (32 x 32 x 30)
    let nn = [32, 32, 30];
    let mut aseg_n = volume(&mut g, MRI_INT, nn, aseg_value);
    let mut norm_n = volume(&mut g, MRI_UCHAR, nn, norm_value);
    let n_mri = subj.join("n").join("mri");
    put(&mut g, &mut aseg_n, &n_mri.join("aseg.auto_noCCseg.mgz"));
    put(&mut g, &mut norm_n, &n_mri.join("norm.mgz"));
    files.push((n_mri.join("aseg.auto_noCCseg.mgz"), 0));
    files.push((n_mri.join("norm.mgz"), 0));
    // mergeseg's and fscalc's volumes
    let vol = dir.join("vol");
    put(&mut g, &mut aseg, &vol.join("src.mgz"));
    let mut merge = volume(&mut g, MRI_UCHAR, n, merge_value);
    put(&mut g, &mut merge, &vol.join("merge.mgz"));
    let mut mergef = volume(&mut g, MRI_FLOAT, n, |x, y, z| {
        match (x * 5 + y * 3 + z * 11) % 23 {
            0 => 0x7fc0_0000,
            1 => 0xffc0_0001,
            2 => 0x7f80_0000,
            3 => 0x7fa0_0000,
            _ => (merge_value(x, y, z) as f32 * 0.75).to_bits(),
        }
    });
    put(&mut g, &mut mergef, &vol.join("mergef.mgh"));
    let mut small = volume(&mut g, MRI_UCHAR, [16, 16, 16], |x, _, _| (x % 3) as u32);
    put(&mut g, &mut small, &vol.join("small.mgz"));
    let mut a = volume(&mut g, MRI_FLOAT, n, |x, y, z| {
        (((x * 7 + y * 3 + z) % 50) as f32 * 0.5 - 4.0).to_bits()
    });
    put(&mut g, &mut a, &vol.join("a.mgz"));
    let mut b = volume(&mut g, MRI_INT, n, |x, y, z| ((x + y * z) % 9) as u32);
    put(&mut g, &mut b, &vol.join("b.mgz"));
    std::fs::write(vol.join("ctab.txt"), CTAB).expect("written");
    // an empty file: neither mode reads a header (a short text there makes the default fill
    // index tables sized by the text's bytes: relay 58's finding, not in these cases)
    std::fs::write(vol.join("text.mgz"), "").expect("written");
    std::fs::write(dir.join("xopts.txt"), "mri_cc -x 1\nmri_normalize -y\n").expect("written");
    std::fs::write(dir.join("xopts2.txt"), "mri_cc -x 1\n# a note\nmri_cc -y\n").expect("written");
    std::fs::write(dir.join("xopts3.txt"), "mri_cc -x 1\n\n\n").expect("written");
    // every file's time T0, but the two outputs whose age the cases test
    for p in walk(dir) {
        touch(&p, 0);
    }
    for (p, dt) in files {
        if dt != 0 {
            touch(&p, dt);
        }
    }
}

/// Every file under `dir`.
fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut names = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else {
                names.push(p);
            }
        }
    }
    names.sort();
    names
}

/// The fixture written once (a template in a temporary folder) and its files' names.
fn template() -> &'static (tempfile::TempDir, Vec<String>) {
    static T: std::sync::OnceLock<(tempfile::TempDir, Vec<String>)> = std::sync::OnceLock::new();
    T.get_or_init(|| {
        let d = tempfile::tempdir().expect("a folder");
        write_fixture(d.path());
        let names = walk(d.path()).iter().map(|p| rel(d.path(), p)).collect();
        (d, names)
    })
}

fn rel(dir: &Path, p: &Path) -> String {
    p.strip_prefix(dir)
        .expect("inside")
        .to_string_lossy()
        .replace('\\', "/")
}

/// The fixture copied into `dir` (the template's files, their times kept).
pub fn copy_fixture(dir: &Path) {
    let (t, names) = template();
    for n in names {
        let from = t.path().join(n);
        let to = dir.join(n);
        std::fs::create_dir_all(to.parent().expect("a folder")).expect("a folder");
        std::fs::copy(&from, &to).expect("copied");
        let m = std::fs::metadata(&from)
            .and_then(|m| m.modified())
            .expect("a time");
        std::fs::File::options()
            .write(true)
            .open(&to)
            .expect("the file")
            .set_modified(m)
            .expect("a time");
    }
}

/// Whether a file holds the fixture's time (not written by the run).
fn fixture_time(p: &Path) -> bool {
    let Ok(m) = std::fs::metadata(p).and_then(|m| m.modified()) else {
        return false;
    };
    [-100i64, 0, 100].iter().any(|dt| {
        m == std::time::UNIX_EPOCH + std::time::Duration::from_secs((T0 as i64 + dt) as u64)
    })
}

/// The case's folder name `<D>` replaced.
fn fill(s: &str, dir: &Path) -> String {
    s.replace("<D>", &dir.to_string_lossy())
}

/// The case's arguments.
pub fn args_of(case: &Case, dir: &Path) -> Vec<String> {
    case.args.iter().map(|a| fill(a, dir)).collect()
}

/// The case's environment (without what only the official scripts need: `PATH`, `FS_LICENSE`).
pub fn env_of(case: &Case, dir: &Path) -> Vec<(String, Option<String>)> {
    let mut e: Vec<(String, Option<String>)> = [
        ("FREESURFER_HOME", "<D>/fsh"),
        ("SUBJECTS_DIR", "<D>/subjects"),
        ("HOME", "<D>"),
        ("LC_ALL", "C"),
        ("LANG", "C"),
        ("TZ", "UTC"),
        ("USER", "probe"),
        ("FS_DISABLE_LANG", "1"),
        ("OMP_NUM_THREADS", "1"),
    ]
    .iter()
    .map(|(k, v)| (k.to_string(), Some(fill(v, dir))))
    .collect();
    // the shell's folder as given (tcsh takes PWD when it names the folder it starts in: the same
    // texts where the temporary folder's name holds a link, macOS's /var)
    e.push((
        "PWD".to_owned(),
        Some(cwd_of(case, dir).to_string_lossy().into_owned()),
    ));
    for (k, v) in &case.env {
        e.retain(|(n, _)| n != k);
        e.push((k.to_string(), v.as_ref().map(|v| fill(v, dir))));
    }
    e
}

/// The folder the case starts in.
pub fn cwd_of(case: &Case, dir: &Path) -> PathBuf {
    if case.cwd.is_empty() {
        dir.to_path_buf()
    } else {
        dir.join(case.cwd)
    }
}

/// Runs case `k` with the port in `dir` (the fixture there): its status and both streams.
pub fn run_port(dir: &Path, k: usize, mode: UpstreamDefects) -> (u8, Vec<u8>, Vec<u8>) {
    let case = cases().swap_remove(k);
    let pairs: Vec<(String, String)> = env_of(&case, dir)
        .into_iter()
        .filter_map(|(k, v)| v.map(|v| (k, v)))
        .collect();
    let settings = Settings {
        threads: 1,
        mode,
        env: Environment::from_pairs(pairs),
        dir: Some(cwd_of(&case, dir)),
    };
    let args = args_of(&case, dir);
    let run = larmorx_freesurfer_volumes::PROGRAMS
        .iter()
        .find(|p| p.name == case.prog)
        .expect("a registered program")
        .run;
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let status = run(&args, &settings, &mut out, &mut err);
    (status, out, err)
}

/// Replaces `from` with `to` in `text`.
fn replace_all(text: &str, from: &str, to: &str) -> String {
    if from.is_empty() {
        text.to_owned()
    } else {
        text.replace(from, to)
    }
}

/// `prefix` followed by digits: the digits masked as `<n>`.
fn mask_number_after(text: &str, prefix: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(i) = rest.find(prefix) {
        out.push_str(&rest[..i + prefix.len()]);
        rest = &rest[i + prefix.len()..];
        let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
        if digits > 0 {
            out.push_str("<n>");
            rest = &rest[digits..];
        }
    }
    out.push_str(rest);
    out
}

/// `tmp.` and six letters or digits not followed by another (`fs_temp_dir`'s names): `tmp.<x>`.
fn mask_temp_names(text: &str) -> String {
    let b = text.as_bytes();
    let mut out = String::new();
    let mut i = 0;
    while i < b.len() {
        if b[i..].starts_with(b"tmp.")
            && (i == 0 || !(b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'.'))
        {
            let run = b[i + 4..]
                .iter()
                .take_while(|c| c.is_ascii_alphanumeric())
                .count();
            let next = b.get(i + 4 + run).copied();
            if run == 6 && next != Some(b'.') {
                out.push_str("tmp.<x>");
                i += 10;
                continue;
            }
        }
        let ch = text[i..].chars().next().expect("a character");
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

/// A C-locale `date` (`Tue Nov 24 17:46:31 UTC 2026`) at byte `i`: its length.
fn date_at(b: &[u8], i: usize) -> Option<usize> {
    let s = b.get(i..i + 28)?;
    let alpha = |k: usize| s[k].is_ascii_alphabetic();
    let digit = |k: usize| s[k].is_ascii_digit();
    let ok = alpha(0)
        && alpha(1)
        && alpha(2)
        && s[3] == b' '
        && alpha(4)
        && alpha(5)
        && alpha(6)
        && s[7] == b' '
        && (s[8] == b' ' || digit(8))
        && digit(9)
        && s[10] == b' '
        && digit(11)
        && digit(12)
        && s[13] == b':'
        && digit(14)
        && digit(15)
        && s[16] == b':'
        && digit(17)
        && digit(18)
        && s[19] == b' ';
    if !ok {
        return None;
    }
    // the zone's name, a blank, four digits
    let rest = &b[i + 20..];
    let z = rest.iter().take_while(|c| !c.is_ascii_whitespace()).count();
    let tail = rest.get(z..z + 5)?;
    (z > 0 && tail[0] == b' ' && tail[1..].iter().all(u8::is_ascii_digit)).then_some(20 + z + 5)
}

fn mask_dates(text: &str) -> String {
    let b = text.as_bytes();
    let mut out = String::new();
    let mut i = 0;
    while i < b.len() {
        if let Some(n) = date_at(b, i) {
            out.push_str("<date>");
            i += n;
            continue;
        }
        let ch = text[i..].chars().next().expect("a character");
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

/// What changes from run to run in a script's text (module documentation); `uname` is this
/// machine's `uname -a` line, `folders` the case folder's names.
pub fn mask_text(text: &str, folders: &[String], uname: &str) -> String {
    let mut t = text.replace("\r\n", "\n").replace('\r', "\n");
    if !uname.is_empty() {
        t = replace_all(&t, uname, "<uname>");
    }
    // the official scripts' folder (the probe's: the reference installation's bin)
    if let Ok(b) = std::env::var("FS_SCRIPTS_PROBE_BIN") {
        t = replace_all(&t, &b, "<FSBIN>");
    }
    for f in folders {
        t = replace_all(&t, f, "<D>");
    }
    t = t.replace("<D>/fsh/bin", "<FSBIN>");
    if cfg!(windows) {
        t = t.replace('\\', "/");
    }
    let mut lines = String::new();
    for l in t.split_inclusive('\n') {
        let body = l.trim_end_matches('\n');
        // fs_time's lines (only where GNU time runs) and its exit line
        if body.starts_with("@#@FSTIME ")
            || body.starts_with("@#@FSLOADPOST ")
            || body.starts_with("Command exited with non-zero status ")
            || body.starts_with("larmorx: ")
        {
            continue;
        }
        let masked = if body.starts_with("valid   : ") {
            // a printed volume geometry's validity flag FreeSurfer never sets (FS-1053)
            "valid   : <memory>".to_owned()
        } else if let Some(rest) = body.strip_prefix("#VMPC# ") {
            let prog = rest.split(' ').next().unwrap_or("");
            format!("#VMPC# {prog} VmPeak <n>")
        } else if body.starts_with("corpus callosum segmentation took ") {
            "corpus callosum segmentation took <n> minutes".to_owned()
        } else if let Some(k) = ["hostname ", "user     ", "sysname  ", "machine  "]
            .iter()
            .find(|k| body.starts_with(**k))
        {
            format!("{k}<x>")
        } else if body.contains("-Run-Time-Sec ") || body.contains("-Run-Time-Hours ") {
            let k = body.find("-Run-Time-").expect("found");
            let space = body[k..].find(' ').map_or(body.len(), |s| k + s);
            format!("{} <n>", &body[..space])
        } else {
            body.to_owned()
        };
        lines.push_str(&masked);
        if l.ends_with('\n') {
            lines.push('\n');
        }
    }
    let t = mask_dates(&lines);
    let t = mask_number_after(&t, "tmpdir.seg2cc.");
    mask_temp_names(&t)
}

/// A file's digest: a volume's extent, frames, type, voxel sizes, directions, centre, voxels and
/// embedded colour table's entries; a log's or a transform's masked text; any other file's bytes.
pub fn file_digest(path: &Path, folders: &[String], uname: &str) -> String {
    let name = path.to_string_lossy().to_string();
    let volume = [".mgz", ".mgh", ".nii", ".nii.gz"]
        .iter()
        .any(|s| name.ends_with(s));
    let bytes = std::fs::read(path).unwrap_or_default();
    if volume && !bytes.starts_with(b"not a volume") {
        let mut g = Globals::new(UpstreamDefects::Reproduce);
        let (mut o, mut e) = io();
        if let Ok(Some(m)) = mri_read_volume(
            &mut g,
            &mut o,
            &mut e,
            &Environment::from_pairs([("FS_LEAVE_NANS", "1")]),
            None,
            &name,
        ) {
            let mut h = Sha256::new();
            let v = &m.vg;
            for i in [v.width, v.height, v.depth, m.nframes, m.mtype] {
                h.update(i.to_le_bytes());
            }
            for f in [
                v.xsize, v.ysize, v.zsize, v.x_r, v.x_a, v.x_s, v.y_r, v.y_a, v.y_s, v.z_r, v.z_a,
                v.z_s, v.c_r, v.c_a, v.c_s,
            ] {
                h.update(f.to_bits().to_le_bytes());
            }
            h.update(m.chunk.as_ref().expect("voxels"));
            if let Some(ct) = &m.ct {
                for (k, e) in ct.entries.iter().enumerate() {
                    if let Some(e) = e {
                        h.update(format!(
                            "{k} {:?} {} {} {} {}\n",
                            e.name, e.ri, e.gi, e.bi, e.ai
                        ));
                    }
                }
            }
            return format!(
                "volume {}",
                h.finalize()
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>()
            );
        }
        return "unreadable volume".to_owned();
    }
    let text_like = name.ends_with(".log") || name.ends_with(".lta");
    let data = if text_like {
        let t = mask_text(&String::from_utf8_lossy(&bytes), folders, uname);
        // the transform's creation line and its file names
        t.lines()
            .map(|l| {
                if l.starts_with("# created by ") {
                    "# created by <x>"
                } else {
                    l
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
            .into_bytes()
    } else {
        bytes
    };
    let h = Sha256::digest(&data);
    format!(
        "{} {}",
        if text_like { "text" } else { "bytes" },
        h.iter().map(|b| format!("{b:02x}")).collect::<String>()
    )
}

/// The folder's files after a run: each file the run wrote (its name masked, its digest), and
/// each file of the fixture that is gone, in order.
pub fn tree(dir: &Path, folders: &[String], uname: &str) -> String {
    let mut rows: Vec<String> = Vec::new();
    for p in walk(dir) {
        if fixture_time(&p) {
            continue;
        }
        let r = rel(dir, &p);
        let r = mask_temp_names(&mask_number_after(&r, "tmpdir.seg2cc."));
        rows.push(format!("{r} {}", file_digest(&p, folders, uname)));
    }
    for n in &template().1 {
        if !dir.join(n).exists() {
            rows.push(format!("{n} gone"));
        }
    }
    rows.sort();
    rows.join("\n")
}

/// The case folder's names in texts: as given and physical, longest first.
pub fn folders(dir: &Path) -> Vec<String> {
    let mut f = vec![dir.to_string_lossy().into_owned()];
    if let Ok(c) = std::fs::canonicalize(dir) {
        let s = c.to_string_lossy().into_owned();
        let s = s.strip_prefix(r"\\?\").map(str::to_owned).unwrap_or(s);
        f.push(s);
    }
    if cfg!(windows) {
        let more: Vec<String> = f.iter().map(|s| s.replace('\\', "/")).collect();
        f.extend(more);
    }
    f.sort_by_key(|s| std::cmp::Reverse(s.len()));
    f.dedup();
    f
}

/// A run's canonical text: status, standard output, standard error and the files, masked.
pub fn outcome(dir: &Path, status: u8, out: &[u8], err: &[u8], uname: &str) -> String {
    let fs = folders(dir);
    format!(
        "status {status}\n--out\n{}\n--err\n{}\n--files\n{}\n",
        mask_text(&String::from_utf8_lossy(out), &fs, uname),
        mask_text(&String::from_utf8_lossy(err), &fs, uname),
        tree(dir, &fs, uname)
    )
}

/// The defects the port's notices name, and `UNMODELLED` when the run holds a form it does not
/// reproduce (tcsh's reading of a name, `--debug`'s trace, `mri_volsynth`).
pub fn notices(err: &[u8]) -> Vec<String> {
    let t = String::from_utf8_lossy(err);
    let mut ids: Vec<String> = t
        .lines()
        .filter_map(|l| l.strip_prefix("larmorx: upstream defect "))
        .filter_map(|l| l.split(' ').next())
        .map(str::to_owned)
        .collect();
    if t.contains("is not reproduced")
        || t.contains("(not reproduced)")
        || t.contains("is not ported yet")
    {
        ids.push("UNMODELLED".to_owned());
    }
    ids.sort();
    ids.dedup();
    ids
}

/// Case `k` with the port in a fresh fixture: its canonical text and notices.
pub fn run_case(k: usize, mode: UpstreamDefects) -> (String, Vec<String>) {
    let d = tempfile::tempdir().expect("a folder");
    let dir = d.path().join("case");
    std::fs::create_dir(&dir).expect("a folder");
    copy_fixture(&dir);
    let (status, out, err) = run_port(&dir, k, mode);
    let env = Environment::default();
    let uname = larmorx_freesurfer_volumes::script::uname_a(&env);
    (outcome(&dir, status, &out, &err, &uname), notices(&err))
}

/// The golden cases (cases where FreeSurfer has no defect: the default gives its outputs on every
/// platform): seg2cc's conformed path to mri_cc, a missing segmentation, its usage; mergeseg's
/// merge with a segid, with a float merge volume holding NaNs and infinities; fscalc's sum, and
/// its sum with that volume.
pub fn golden_indices() -> Vec<usize> {
    let want: [(&str, &[&str]); 7] = [
        ("seg2cc", &["--s", "s"]),
        ("seg2cc", &["--s", "m"]),
        ("seg2cc", &[]),
        (
            "mergeseg",
            &[
                "--src",
                "vol/src.mgz",
                "--merge",
                "vol/merge.mgz",
                "--o",
                "o/a.mgz",
                "--segid",
                "7",
            ],
        ),
        (
            "mergeseg",
            &[
                "--src",
                "vol/src.mgz",
                "--merge",
                "vol/mergef.mgh",
                "--o",
                "o/f.mgz",
            ],
        ),
        (
            "fscalc",
            &["vol/a.mgz", "add", "vol/b.mgz", "-o", "o/s.mgz"],
        ),
        (
            "fscalc",
            &["vol/a.mgz", "add", "vol/mergef.mgh", "--o", "o/n.mgz"],
        ),
    ];
    let all = cases();
    want.iter()
        .map(|(p, a)| {
            all.iter()
                .position(|c| c.prog == *p && c.args == *a && c.env.is_empty() && c.cwd.is_empty())
                .expect("a golden case")
        })
        .collect()
}
