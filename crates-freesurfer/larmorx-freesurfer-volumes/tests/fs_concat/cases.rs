// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The fs-concat probe's cases (`validation/probes/fs-concat/`): a seeded generator of
//! `mri_concat` command lines with the files they read, and the port's answer to each, printed as
//! `check.py` prints the official program's (`<status> | <stdout> | <stderr> | <files>`,
//! C-escaped).
//!
//! A case line is `cat <env> <nfiles> [<name> <hex>]... <ndirs> [<hex>]... <nargs> [<hex>]...`,
//! as the fs-vol2vol probe's (`<env>`: `-` or the hex of `;`-separated assignments, `NAME=value`
//! or `-NAME`; `@DIR@` in a value is the case's folder). The answer's lines mask the resource
//! usage `--debug` prints (`mri_ca_label <name> <value>` lines) and `--rusage` writes (a file
//! ending in `.rusage`). Pseudo-notices, which `check.py` counts, not compares: `CRASH` (a status
//! FreeSurfer's build reaches by a signal: 134, 136, 139; its unflushed standard output is lost)
//! (and FS-564's covariance that cannot be computed, FS-13's matrix that cannot be allocated: the
//! port ends with an error in both modes where FreeSurfer crashes) and `MEMORY` (a result
//! FreeSurfer takes from memory contents or leaves undefined: FS-53, FS-249, FS-1196, FS-1243,
//! FS-1247).
#![allow(dead_code)]

#[path = "../fs_info/cases.rs"]
mod info;

use std::io::Write;
use std::path::Path;

use larmorx_core::defects::UpstreamDefects;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::TrackedErr;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_mri::mri::{MRI_FLOAT, MRI_INT, MRI_SHORT, MRI_UCHAR, Mri};
use larmorx_freesurfer_mri::voxel::mri_set_vox_val;
use larmorx_freesurfer_volumes::mri_concat::main::{Run, main};

pub use info::{Gen, escape, hex, probe_env, unhex};
use info::{LUT, Spec, build, fnv, mask_bytes};

// ------------------------------------------------------------------ the files

fn spec(ty: i32, dims: [i32; 4]) -> Spec {
    Spec {
        dims,
        ty,
        size: [1.0, 1.5, 2.0],
        mdc: [-1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 1.0, 0.0],
        c: [1.0, -2.0, 3.0],
        good: 1,
        tr: 2300.0,
        flip: 0.157_079_632_679_489_66,
        te: 2.98,
        ti: 900.0,
        fov: 256.0,
        dof: 1,
        xform: None,
        pedir: None,
        cmds: Vec::new(),
        auto_align: None,
        orig: None,
        field: 3.0,
        frames: Vec::new(),
        ctab: false,
        seg: false,
    }
}

/// The inputs a case may name.
pub struct Inputs {
    files: Vec<(String, Vec<u8>)>,
    dirs: Vec<String>,
}

fn write_as(m: &mut Mri, name: &str) -> Vec<(String, Vec<u8>)> {
    let (dir, base) = match name.rfind('/') {
        Some(i) => (&name[..=i], &name[i + 1..]),
        None => ("", name),
    };
    info::write_volume(m, base)
        .into_iter()
        .map(|(n, b)| (format!("{dir}{n}"), b))
        .collect()
}

/// `m` with the voxels `g` picks (about one in `k`) set to `v` in every frame.
fn sprinkle(g: &mut Gen, m: &mut Mri, k: u64, v: f32) {
    let mut fs = Globals::new(UpstreamDefects::Fix);
    let mut e = Vec::new();
    for z in 0..m.vg.depth {
        for y in 0..m.vg.height {
            for x in 0..m.vg.width {
                if g.chance(k) {
                    for f in 0..m.nframes {
                        mri_set_vox_val(&mut fs, &mut e, m, x, y, z, f, v, 0).expect("a voxel");
                    }
                }
            }
        }
    }
}

/// `m`'s voxels zero at random, frame by frame (about one in `k`).
fn zeros(g: &mut Gen, m: &mut Mri, k: u64) {
    let mut fs = Globals::new(UpstreamDefects::Fix);
    let mut e = Vec::new();
    for f in 0..m.nframes {
        for z in 0..m.vg.depth {
            for y in 0..m.vg.height {
                for x in 0..m.vg.width {
                    if g.chance(k) {
                        mri_set_vox_val(&mut fs, &mut e, m, x, y, z, f, 0.0, 0).expect("a voxel");
                    }
                }
            }
        }
    }
}

/// A binary mask of `dims` (about half the voxels 1).
fn mask(g: &mut Gen, dims: [i32; 3]) -> Mri {
    let mut m = build(g, &spec(MRI_FLOAT, [dims[0], dims[1], dims[2], 1]));
    let mut fs = Globals::new(UpstreamDefects::Fix);
    let mut e = Vec::new();
    for z in 0..dims[2] {
        for y in 0..dims[1] {
            for x in 0..dims[0] {
                let v = if g.chance(2) { 1.0 } else { 0.0 };
                mri_set_vox_val(&mut fs, &mut e, &mut m, x, y, z, 0, v, 0).expect("a voxel");
            }
        }
    }
    m
}

/// The common grid of the inputs.
const D: [i32; 3] = [4, 5, 3];

fn base_inputs() -> Inputs {
    let mut g = Gen::new(0x000c_0ca7_0039);
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    let put = |files: &mut Vec<(String, Vec<u8>)>, name: &str, m: &mut Mri| {
        files.extend(write_as(m, name));
    };
    let v = |g: &mut Gen, ty: i32, nf: i32| build(g, &spec(ty, [D[0], D[1], D[2], nf]));
    let mut a = v(&mut g, MRI_FLOAT, 1);
    put(&mut files, "a.mgz", &mut a);
    let mut b = v(&mut g, MRI_FLOAT, 1);
    put(&mut files, "b.mgz", &mut b);
    let mut c = v(&mut g, MRI_FLOAT, 3);
    put(&mut files, "c.mgz", &mut c);
    let mut d = v(&mut g, MRI_SHORT, 2);
    put(&mut files, "d.nii.gz", &mut d);
    let mut e = v(&mut g, MRI_FLOAT, 1);
    put(&mut files, "e.nii", &mut e);
    let mut u = v(&mut g, MRI_UCHAR, 1);
    put(&mut files, "u.mgz", &mut u);
    let mut u2 = v(&mut g, MRI_UCHAR, 2);
    put(&mut files, "u2.mgz", &mut u2);
    let mut f4 = v(&mut g, MRI_FLOAT, 4);
    put(&mut files, "f4.mgz", &mut f4);
    let mut f5 = v(&mut g, MRI_FLOAT, 5);
    put(&mut files, "f5.mgz", &mut f5);
    let mut f6 = v(&mut g, MRI_FLOAT, 6);
    put(&mut files, "f6.mgz", &mut f6);
    // zeros: first-non-zero, combine, prune, the max index's pruning
    let mut z = v(&mut g, MRI_FLOAT, 3);
    zeros(&mut g, &mut z, 2);
    sprinkle(&mut g, &mut z, 4, 0.0);
    put(&mut files, "z.mgz", &mut z);
    let mut z1 = v(&mut g, MRI_FLOAT, 1);
    zeros(&mut g, &mut z1, 2);
    put(&mut files, "z1.mgz", &mut z1);
    // segmentations (with and without a colour table)
    let mut s1 = build(
        &mut g,
        &Spec {
            seg: true,
            ctab: true,
            ..spec(MRI_INT, [D[0], D[1], D[2], 1])
        },
    );
    put(&mut files, "seg1.mgz", &mut s1);
    let mut s2 = build(
        &mut g,
        &Spec {
            seg: true,
            ..spec(MRI_INT, [D[0], D[1], D[2], 1])
        },
    );
    put(&mut files, "seg2.mgz", &mut s2);
    let mut s3 = build(
        &mut g,
        &Spec {
            seg: true,
            ..spec(MRI_UCHAR, [D[0], D[1], D[2], 4])
        },
    );
    put(&mut files, "seg4.mgz", &mut s3);
    // another grid
    let mut big = build(&mut g, &spec(MRI_FLOAT, [5, 5, 3, 1]));
    put(&mut files, "big.mgz", &mut big);
    // masks
    let mut k = mask(&mut g, D);
    put(&mut files, "k.mgz", &mut k);
    let mut k2 = mask(&mut g, [3, 5, 3]);
    put(&mut files, "k2.mgz", &mut k2);
    // NaN and infinite voxels (made 0 when read), from their bits
    let mut nan = v(&mut g, MRI_FLOAT, 2);
    sprinkle(&mut g, &mut nan, 5, f32::from_bits(0x7fc0_0001));
    sprinkle(&mut g, &mut nan, 6, f32::from_bits(0xffc0_0000));
    sprinkle(&mut g, &mut nan, 7, f32::from_bits(0x7f80_0000));
    put(&mut files, "nan.mgz", &mut nan);
    // surface overlays (pctsurfcon's form: one row of vertices)
    let mut o1 = build(&mut g, &spec(MRI_FLOAT, [30, 1, 1, 1]));
    sprinkle(&mut g, &mut o1, 6, 0.0);
    put(&mut files, "lh.wm.mgh", &mut o1);
    let mut o2 = build(&mut g, &spec(MRI_FLOAT, [30, 1, 1, 1]));
    sprinkle(&mut g, &mut o2, 6, 0.0);
    put(&mut files, "lh.gm.mgh", &mut o2);
    // --zconcat's slabs
    let mut zb = v(&mut g, MRI_FLOAT, 2);
    put(&mut files, "zb.mgz", &mut zb);
    let mut zt = build(&mut g, &spec(MRI_FLOAT, [4, 5, 4, 2]));
    put(&mut files, "zt.mgz", &mut zt);
    let mut zw = build(&mut g, &spec(MRI_FLOAT, [3, 5, 2, 2]));
    put(&mut files, "zw.mgz", &mut zw);
    // a header that reads whole volumes do not: the MGH file cut after its header
    let mut tr = v(&mut g, MRI_FLOAT, 2);
    let bytes = write_as(&mut tr, "t.mgh");
    let cut = bytes[0].1[..300].to_vec();
    files.push(("trunc.mgh".into(), cut));
    // text files: colour table, lists, matrices, weights
    files.push(("lut.txt".into(), LUT.to_vec()));
    files.push(("notvol.txt".into(), b"not a volume\n".to_vec()));
    files.push(("list.txt".into(), b"a.mgz\nb.mgz\n".to_vec()));
    files.push(("list2.txt".into(), b"c.mgz  d.nii.gz\n\n u.mgz".to_vec()));
    files.push(("my list.txt".into(), b"a.mgz e.nii\n".to_vec()));
    files.push(("my".into(), b"b.mgz\n".to_vec()));
    files.push(("empty.txt".into(), Vec::new()));
    files.push(("listbad.txt".into(), b"a.mgz nofile.mgz\n".to_vec()));
    let long = format!("{}.mgz", "w".repeat(2096));
    files.push(("longw.txt".into(), format!("{long}\nb.mgz\n").into_bytes()));
    files.push((
        "longlast.txt".into(),
        format!("a.mgz {long}\n").into_bytes(),
    ));
    let exact = format!("{}.mgz", "x".repeat(1996));
    files.push((
        "long2000.txt".into(),
        format!("{exact} a.mgz\n").into_bytes(),
    ));
    files.push((
        "m24.txt".into(),
        b"0.25 0.25 0.25 0.25\n1 -1 1 -1\n".to_vec(),
    ));
    files.push(("m23.txt".into(), b"1 0 0\n0 1 0\n".to_vec()));
    files.push(("m33.txt".into(), b"1 0 0\n0 1 0\n0 0 1\n".to_vec()));
    files.push(("m14.txt".into(), b"1.5 -2 0.125 3\n".to_vec()));
    files.push(("w4.txt".into(), b"1\n0.5\n-2\n0.25\n".to_vec()));
    files.push(("w3.txt".into(), b"1\n2\n3\n".to_vec()));
    files.push(("w2.txt".into(), b"2\n-1\n".to_vec()));
    files.push(("wnan.txt".into(), b"1\nnan\n-inf\n2\n".to_vec()));
    files.push(("w0.txt".into(), b"1\n-1\n2\n-2\n".to_vec()));
    files.push(("wbad.txt".into(), b"1\nx\n".to_vec()));
    let dirs = vec!["listdir".into(), "outdir".into()];
    Inputs { files, dirs }
}

fn names(a: &str, f: &str) -> bool {
    let a = a.trim_end_matches('/');
    !a.is_empty() && (f == a || f.strip_prefix(a).is_some_and(|r| r.starts_with('/')))
}

/// A case line: the files and folders the arguments name (and the files the named list files
/// name).
pub fn line(env: &[&str], i: &Inputs, args: &[String]) -> String {
    let mut wanted: Vec<String> = args.to_vec();
    for (f, b) in &i.files {
        if (f.ends_with(".txt") || f == "my")
            && args
                .iter()
                .any(|a| names(a, f) || a.split_whitespace().next() == Some(f))
        {
            for w in String::from_utf8_lossy(b).split_whitespace() {
                wanted.push(w.to_owned());
            }
        }
    }
    let files: Vec<&(String, Vec<u8>)> = i
        .files
        .iter()
        .filter(|(f, _)| {
            wanted
                .iter()
                .any(|a| names(a, f) || a.split_whitespace().next() == Some(f))
        })
        .collect();
    let dirs: Vec<&String> = i
        .dirs
        .iter()
        .filter(|d| {
            args.iter()
                .any(|a| names(a, d) || a.starts_with(&format!("{d}/")))
        })
        .collect();
    let mut t = vec![
        "cat".to_owned(),
        if env.is_empty() {
            "-".to_owned()
        } else {
            hex(env.join(";").as_bytes())
        },
        files.len().to_string(),
    ];
    for (f, c) in files {
        t.push(hex(f.as_bytes()));
        t.push(hex(c));
    }
    t.push(dirs.len().to_string());
    for d in dirs {
        t.push(hex(d.as_bytes()));
    }
    t.push(args.len().to_string());
    for a in args {
        t.push(hex(a.as_bytes()));
    }
    t.join(" ")
}

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| (*x).to_owned()).collect()
}

/// The reductions and operations a seeded case may add (each with its arguments).
const OPS: &[&[&str]] = &[
    &["--mean"],
    &["--median"],
    &["--mean-div-n"],
    &["--sum"],
    &["--std"],
    &["--var"],
    &["--max"],
    &["--max-index"],
    &["--max-index-prune"],
    &["--max-index", "--max-index-add", "2"],
    &["--min"],
    &["--conjunct"],
    &["--vote"],
    &["--vote-ex0"],
    &["--first-non-zero"],
    &["--sort"],
    &["--norm-mean"],
    &["--norm1"],
    &["--prune"],
    &["--max-bonfcor"],
    &["--asl"],
    &["--combine"],
    &["--cumsum"],
    &["--fnorm"],
    &["--tar1", "1"],
    &["--tar1", "0"],
    &["--rep", "2"],
    &["--abs"],
    &["--pos"],
    &["--neg"],
    &["--mul", "2.5"],
    &["--div", "4"],
    &["--add", "-3.25"],
    &["--keep-datatype"],
    &["--paired-sum"],
    &["--paired-avg"],
    &["--paired-diff"],
    &["--paired-diff-norm"],
    &["--paired-diff-norm1"],
    &["--paired-diff-norm2"],
    &["--gmean", "2"],
    &["--in-frame", "0"],
    &["--in-frame", "1"],
    &["--mtx", "m24.txt"],
    &["--mtx", "m14.txt"],
    &["--w", "w4.txt"],
    &["--wn", "w2.txt"],
    &["--ctab", "lut.txt"],
    &["--mask", "k.mgz"],
    &["--no-check"],
    &["--rms"],
    &["--scm"],
    &["--pca"],
];

const INS: &[&str] = &[
    "a.mgz", "b.mgz", "c.mgz", "d.nii.gz", "e.nii", "u.mgz", "u2.mgz", "f4.mgz", "f5.mgz",
    "f6.mgz", "z.mgz", "z1.mgz", "seg1.mgz", "seg2.mgz", "seg4.mgz", "nan.mgz",
];
const OUTS: &[&str] = &["o.mgz", "o.nii.gz", "o.mgh", "o.nii", "outdir/o.mgz"];

fn seeded(g: &mut Gen, i: &Inputs) -> String {
    let mut args = Vec::new();
    let n = g.int(1, 3);
    for _ in 0..n {
        let x = g.pick(INS);
        if g.chance(4) {
            args.push("--i".to_owned());
        }
        args.push(x.to_owned());
    }
    for _ in 0..g.int(0, 3) {
        args.extend(g.pick(OPS).iter().map(|x| (*x).to_owned()));
    }
    args.push("--o".to_owned());
    args.push(g.pick(OUTS).to_owned());
    if g.chance(8) {
        args.push("--debug".to_owned());
    }
    // shuffle a little: the output first sometimes
    if g.chance(4) {
        let k = args.len();
        args.rotate_left(k - 2);
    }
    line(&[], i, &args)
}

/// The fixed cases: the command line, every check, every option and path.
fn prelude() -> Vec<String> {
    let i = base_inputs();
    let mut out = Vec::new();
    let mut add = |a: &[&str]| out.push(line(&[], &i, &s(a)));
    // the command line
    add(&[]);
    add(&["--help"]);
    add(&["--HELP", "a.mgz"]);
    add(&["--version"]);
    add(&["--VERSION"]);
    add(&["a.mgz", "--version", "--o", "o.mgz"]);
    for o in [
        "--o",
        "--i",
        "--mtx",
        "--w",
        "--wn",
        "--gmean",
        "--in-frame",
        "--rep",
        "--rusage",
        "--mask",
        "--mul",
        "--div",
        "--add",
        "--ctab",
        "--tar1",
        "--f",
        "--pca-mask",
        "--max-index-add",
    ] {
        add(&["a.mgz", o]);
    }
    add(&["a.mgz", "--zconcat", "zb.mgz", "zt.mgz", "1"]);
    add(&["a.mgz"]);
    add(&["--o", "o.mgz"]);
    add(&["--debug", "a.mgz", "b.mgz", "--o", "o.mgz"]);
    add(&["--DEBUG", "a.mgz", "--o", "o.mgz", "--Mean"]);
    add(&["a.mgz", "-debug", "--o", "o.mgz"]);
    add(&["a.mgz", "--foo", "--o", "o.mgz"]);
    add(&["a.mgz", "--chunk", "b.mgz", "--NO-CHUNK", "--o", "o.mgz"]);
    add(&["a.mgz", "--check", "b.mgz", "--o", "o.mgz"]);
    add(&["a.mgz", "--o", "o.mgz", "--o", "o2.mgz"]);
    // the combinations refused
    for a in [
        &["--paired-diff", "--paired-avg"][..],
        &["--paired-diff-norm1", "--paired-diff-norm2"],
        &["--paired-diff-norm", "--paired-diff-norm1"],
        &["--paired-diff-norm", "--paired-diff-norm2"],
        &["--paired-avg", "--paired-diff-norm"],
        &["--mean", "--std"],
        &["--std", "--var"],
        &["--abs", "--pos"],
        &["--pos", "--neg"],
        &["--abs", "--neg"],
        &["--paired-sum", "--paired-diff"],
        &["--mask", "k.mgz", "--mean"],
    ] {
        let mut v = vec!["f4.mgz", "--o", "o.mgz"];
        v.extend_from_slice(a);
        add(&v);
    }
    // plain concatenation, formats and checks
    add(&["a.mgz", "b.mgz", "--o", "o.mgz"]);
    add(&[
        "--i", "a.mgz", "--i", "c.mgz", "d.nii.gz", "--o", "o.nii.gz",
    ]);
    add(&["a.mgz", "e.nii", "--o", "o.nii"]);
    add(&["a.mgz", "u.mgz", "--o", "o.mgh"]);
    add(&["c.mgz", "--o", "outdir/o.mgz"]);
    add(&["a.mgz", "big.mgz", "--o", "o.mgz"]);
    add(&["big.mgz", "a.mgz", "--o", "o.mgz"]);
    add(&["a.mgz", "nofile.mgz", "--o", "o.mgz"]);
    add(&["nofile.mgz", "--o", "o.mgz"]);
    add(&["a.mgz", "notvol.txt", "--o", "o.mgz"]);
    add(&["a.mgz", "trunc.mgh", "--o", "o.mgz"]);
    add(&["trunc.mgh", "--o", "o.mgz"]);
    add(&["nan.mgz", "a.mgz", "--o", "o.mgz"]);
    add(&["a.mgz", "--o", "nodir/o.mgz"]);
    add(&["a.mgz", "--o", "o.xyz"]);
    add(&["a.mgz", "--o", "onoext"]);
    add(&["seg1.mgz", "seg2.mgz", "--o", "o.mgz"]);
    add(&["seg2.mgz", "seg1.mgz", "--o", "o.mgz"]);
    add(&["a.mgz", "--o", "o.mgz", "--rusage", "o.rusage"]);
    add(&[
        "a.mgz",
        "--o",
        "o.mgz",
        "--debug",
        "--rusage",
        "nodir/o.rusage",
    ]);
    // --no-check
    add(&["--no-check", "a.mgz", "b.mgz", "--o", "o.mgz"]);
    add(&["--no-check", "a.mgz", "c.mgz", "--o", "o.mgz"]);
    add(&["--no-check", "c.mgz", "--o", "o.mgz"]);
    add(&["--no-check", "a.mgz", "big.mgz", "--o", "o.mgz"]);
    add(&["--no-check", "big.mgz", "a.mgz", "--o", "o.mgz"]);
    add(&["--no-check", "nofile.mgz", "--o", "o.mgz"]);
    add(&["--no-check", "a.mgz", "nofile.mgz", "--o", "o.mgz"]);
    add(&[
        "--no-check",
        "a.mgz",
        "b.mgz",
        "--in-frame",
        "0",
        "--o",
        "o.mgz",
    ]);
    add(&[
        "--no-check",
        "a.mgz",
        "c.mgz",
        "--w",
        "w2.txt",
        "--o",
        "o.mgz",
    ]);
    add(&[
        "--no-check",
        "u.mgz",
        "a.mgz",
        "--keep-datatype",
        "--o",
        "o.mgz",
    ]);
    // the reductions, each on four frames and more
    for op in OPS {
        for input in [&["f4.mgz"][..], &["z.mgz"], &["a.mgz", "b.mgz", "c.mgz"]] {
            let mut v: Vec<&str> = input.to_vec();
            v.extend_from_slice(op);
            v.extend_from_slice(&["--o", "o.mgz"]);
            add(&v);
        }
    }
    add(&["a.mgz", "--std", "--o", "o.mgz"]);
    add(&["a.mgz", "--var", "--o", "o.mgz"]);
    add(&["a.mgz", "--fnorm", "--o", "o.mgz"]);
    add(&["a.mgz", "--tar1", "1", "--o", "o.mgz"]);
    add(&["f5.mgz", "--asl", "--o", "o.mgz"]);
    add(&["f6.mgz", "--asl", "--o", "o.mgz"]);
    add(&["f5.mgz", "--paired-diff", "--o", "o.mgz"]);
    add(&["f4.mgz", "--tar1", "x", "--o", "o.mgz"]);
    add(&["f4.mgz", "--tar1", "-2", "--o", "o.mgz"]);
    add(&["f4.mgz", "--TAR1", "3", "--o", "o.mgz"]);
    add(&["f4.mgz", "--rep", "0", "--o", "o.mgz"]);
    add(&["f4.mgz", "--rep", "-1", "--o", "o.mgz"]);
    add(&["f4.mgz", "--rep", "3", "--mean", "--o", "o.mgz"]);
    add(&["f4.mgz", "--rep", "x", "--o", "o.mgz"]);
    add(&[
        "z.mgz",
        "--max-index-prune",
        "--max-index-add",
        "5",
        "--o",
        "o.mgz",
    ]);
    add(&["z.mgz", "--max-index-add", "5", "--o", "o.mgz"]);
    add(&[
        "f4.mgz",
        "--max-index",
        "--max-index-add",
        "-3",
        "--o",
        "o.mgz",
    ]);
    add(&[
        "f4.mgz",
        "--max-index",
        "--max-index-add",
        "+3",
        "--o",
        "o.mgz",
    ]);
    add(&[
        "f4.mgz",
        "--max-index",
        "--max-index-add",
        "-",
        "--o",
        "o.mgz",
    ]);
    add(&[
        "f4.mgz",
        "--max-index",
        "--max-index-add",
        "x",
        "--o",
        "o.mgz",
    ]);
    add(&[
        "f4.mgz",
        "--max-index",
        "--max-index-add",
        "",
        "--o",
        "o.mgz",
    ]);
    add(&[
        "f4.mgz",
        "--max-index",
        "--max-index-add",
        "2147483647",
        "--o",
        "o.mgz",
    ]);
    add(&[
        "f4.mgz",
        "--max-index",
        "--max-index-add",
        "9e9",
        "--o",
        "o.mgz",
    ]);
    add(&[
        "f4.mgz",
        "--MAX-INDEX-ADD",
        "1",
        "--max-index",
        "--o",
        "o.mgz",
    ]);
    add(&["z.mgz", "--prune", "--no-prune", "--o", "o.mgz"]);
    add(&["z.mgz", "--prune", "--mean", "--o", "o.mgz"]);
    add(&["z.mgz", "--prune", "--rep", "2", "--o", "o.mgz"]);
    add(&["z.mgz", "--combine", "--keep-datatype", "--o", "o.mgz"]);
    add(&["seg4.mgz", "--vote", "--o", "o.mgz"]);
    add(&["seg4.mgz", "--vote-ex0", "--o", "o.mgz"]);
    add(&["seg4.mgz", "--vote", "--keep-datatype", "--o", "o.mgz"]);
    add(&["seg4.mgz", "--sort", "--keep-datatype", "--o", "o.mgz"]);
    add(&[
        "seg4.mgz",
        "--first-non-zero",
        "--keep-datatype",
        "--o",
        "o.mgz",
    ]);
    add(&["f4.mgz", "--scm", "--o", "o.mgz"]);
    add(&["f4.mgz", "--pca", "--o", "o.mgz"]);
    add(&["f4.mgz", "--pca", "--o", "o.nii.gz"]);
    add(&["f4.mgz", "--pca", "--o", "outdir/o.mgh"]);
    add(&["f4.mgz", "--pca", "--pca-mask", "k.mgz", "--o", "o.mgz"]);
    add(&[
        "f4.mgz",
        "--pca",
        "--pca-mask",
        "nofile.mgz",
        "--o",
        "o.mgz",
    ]);
    add(&["f4.mgz", "--pca-mask", "k.mgz", "--o", "o.mgz"]);
    add(&["f4.mgz", "--pca", "--o", "onoext"]);
    add(&["a.mgz", "--pca", "--o", "o.mgz"]);
    add(&["f4.mgz", "--rms", "--o", "o.mgz"]);
    add(&["c.mgz", "--rms", "--mean", "--o", "o.mgz"]);
    add(&["a.mgz", "--rms", "--o", "o.mgz"]);
    add(&["a.mgz", "b.mgz", "--rms", "--o", "o.mgz"]);
    add(&["trunc.mgh", "--rms", "--o", "o.mgz"]);
    add(&["c.mgz", "--rms", "--in-frame", "1", "--o", "o.mgz"]);
    add(&["u2.mgz", "--rms", "--keep-datatype", "--o", "o.mgz"]);
    // pairs (pctsurfcon's call first)
    add(&[
        "lh.wm.mgh",
        "lh.gm.mgh",
        "--paired-diff-norm",
        "--mul",
        "100",
        "--o",
        "o.mgh",
    ]);
    for p in [
        "--paired-avg",
        "--paired-sum",
        "--paired-diff",
        "--paired-diff-norm",
        "--paired-diff-norm1",
        "--paired-diff-norm2",
    ] {
        add(&["lh.wm.mgh", "lh.gm.mgh", p, "--o", "o.mgh"]);
        add(&["z1.mgz", "a.mgz", p, "--o", "o.mgz"]);
    }
    add(&["c.mgz", "--paired-sum", "--o", "o.mgz"]);
    add(&[
        "seg1.mgz", "seg2.mgz", "--sum", "--ctab", "lut.txt", "--o", "o.mgz",
    ]);
    add(&["seg1.mgz", "seg2.mgz", "--sum", "--o", "o.mgz"]);
    add(&[
        "seg2.mgz", "a.mgz", "--sum", "--CTAB", "lut.txt", "--o", "o.mgz",
    ]);
    add(&["a.mgz", "--ctab", "nofile.txt", "--o", "o.mgz"]);
    add(&["a.mgz", "--ctab", "notvol.txt", "--o", "o.mgz"]);
    // matrices and weights
    add(&["f4.mgz", "--mtx", "m24.txt", "--o", "o.mgz"]);
    add(&["f4.mgz", "--mtx", "m23.txt", "--o", "o.mgz"]);
    add(&["f4.mgz", "--mtx", "m33.txt", "--o", "o.mgz"]);
    add(&["c.mgz", "--mtx", "m33.txt", "--o", "o.mgz"]);
    add(&["c.mgz", "--mtx", "m23.txt", "--o", "o.mgz"]);
    add(&["f4.mgz", "--mtx", "nofile.txt", "--o", "o.mgz"]);
    add(&["f4.mgz", "--mtx", "empty.txt", "--o", "o.mgz"]);
    add(&["f4.mgz", "--mtx", "wbad.txt", "--o", "o.mgz"]);
    add(&["f4.mgz", "--mtx", "m24.txt", "--gmean", "2", "--o", "o.mgz"]);
    for ng in ["1", "2", "3", "4", "5", "0", "-1", "-2", "-4", "x"] {
        add(&["f4.mgz", "--gmean", ng, "--o", "o.mgz"]);
    }
    add(&["f4.mgz", "--gmean", "2", "--debug", "--o", "o.mgz"]);
    add(&["f4.mgz", "--asl", "--mtx", "m24.txt", "--o", "o.mgz"]);
    add(&["f4.mgz", "--w", "w4.txt", "--o", "o.mgz"]);
    add(&["f4.mgz", "--wn", "w4.txt", "--o", "o.mgz"]);
    add(&["f4.mgz", "--w", "w3.txt", "--o", "o.mgz"]);
    add(&["f4.mgz", "--w", "wnan.txt", "--o", "o.mgz"]);
    add(&["f4.mgz", "--wn", "wnan.txt", "--o", "o.mgz"]);
    add(&["f4.mgz", "--wn", "w0.txt", "--o", "o.mgz"]);
    add(&["f4.mgz", "--w", "nofile.txt", "--o", "o.mgz"]);
    add(&["f4.mgz", "--w", "wbad.txt", "--o", "o.mgz"]);
    add(&["a.mgz", "b.mgz", "--w", "w2.txt", "--sum", "--o", "o.mgz"]);
    add(&["f4.mgz", "--w", "m24.txt", "--o", "o.mgz"]);
    // frames
    add(&["c.mgz", "d.nii.gz", "--in-frame", "1", "--o", "o.mgz"]);
    add(&["c.mgz", "d.nii.gz", "--in-frame", "2", "--o", "o.mgz"]);
    add(&["c.mgz", "--in-frame", "3", "--o", "o.mgz"]);
    add(&["c.mgz", "--in-frame", "-3", "--o", "o.mgz"]);
    add(&["c.mgz", "--in-frame", "x", "--o", "o.mgz"]);
    add(&["c.mgz", "--in-frame", "1", "--debug", "--o", "o.mgz"]);
    // signs
    for sgn in ["--abs", "--pos", "--neg"] {
        add(&["a.mgz", "b.mgz", sgn, "--debug", "--o", "o.mgz"]);
        add(&["d.nii.gz", sgn, "--keep-datatype", "--o", "o.mgz"]);
    }
    // constants
    for (o, v) in [
        ("--mul", "2"),
        ("--mul", ".5"),
        ("--mul", "-1e3"),
        ("--mul", "+nan"),
        ("--mul", "-inf"),
        ("--mul", "x"),
        ("--mul", ""),
        ("--mul", "1e400"),
        ("--div", "4"),
        ("--div", "0"),
        ("--div", "-0"),
        ("--div", "x"),
        ("--div", ".x"),
        ("--add", "-3.5"),
        ("--add", "+inf"),
        ("--add", "-nan"),
        ("--add", "x"),
        ("--add", "."),
        ("--add", ""),
        ("--add", "1e-320"),
    ] {
        add(&["a.mgz", o, v, "--o", "o.mgz"]);
    }
    add(&["u.mgz", "--mul", "300", "--keep-datatype", "--o", "o.mgz"]);
    add(&["u.mgz", "--add", "-300", "--keep-datatype", "--o", "o.mgz"]);
    add(&[
        "d.nii.gz",
        "--mul",
        "+nan",
        "--keep-datatype",
        "--o",
        "o.mgz",
    ]);
    add(&[
        "a.mgz", "--mul", "2", "--div", "4", "--add", "1", "--o", "o.mgz",
    ]);
    add(&["f4.mgz", "--max-bonfcor", "--add", "7", "--o", "o.mgz"]);
    add(&[
        "u2.mgz",
        "seg1.mgz",
        "--keep-datatype",
        "--sum",
        "--o",
        "o.mgz",
    ]);
    add(&["seg1.mgz", "u2.mgz", "--keep-datatype", "--o", "o.mgz"]);
    // masks
    for op in [
        "--sort",
        "--vote",
        "--vote-ex0",
        "--first-non-zero",
        "--cumsum",
        "--mean",
    ] {
        add(&["f4.mgz", op, "--mask", "k.mgz", "--o", "o.mgz"]);
        add(&["f4.mgz", op, "--mask", "k2.mgz", "--o", "o.mgz"]);
    }
    add(&["f4.mgz", "--sort", "--mask", "nofile.mgz", "--o", "o.mgz"]);
    add(&["f4.mgz", "--sort", "--mask", "notvol.txt", "--o", "o.mgz"]);
    // --zconcat
    add(&["--zconcat", "zb.mgz", "zt.mgz", "1", "oz.mgz"]);
    add(&[
        "--zconcat",
        "zb.mgz",
        "zt.mgz",
        "0",
        "oz.nii.gz",
        "--o",
        "o.mgz",
    ]);
    add(&["a.mgz", "--zconcat", "zb.mgz", "zt.mgz", "3", "oz.mgz"]);
    add(&["--zconcat", "zb.mgz", "zt.mgz", "4", "oz.mgz"]);
    add(&["--zconcat", "zb.mgz", "zt.mgz", "-1", "oz.mgz"]);
    add(&["--zconcat", "zb.mgz", "zw.mgz", "0", "oz.mgz"]);
    add(&["--zconcat", "zb.mgz", "nofile.mgz", "0", "oz.mgz"]);
    add(&["--zconcat", "nofile.mgz", "zt.mgz", "0", "oz.mgz"]);
    add(&["--zconcat", "zb.mgz", "zt.mgz", "1", "nodir/oz.mgz"]);
    add(&["--zconcat", "zb.mgz", "zt.mgz", "x", "oz.mgz"]);
    add(&["--zconcat", "zb.mgz", "zt.mgz", "2x", "oz.mgz"]);
    // --f
    add(&["--f", "list.txt", "--o", "o.mgz"]);
    add(&[
        "--F", "list.txt", "c.mgz", "--f", "list.txt", "--o", "o.mgz",
    ]);
    add(&["--f", "list2.txt", "--o", "o.mgz"]);
    add(&["--f", "my list.txt", "--o", "o.mgz"]);
    add(&["--f", " list.txt", "--o", "o.mgz"]);
    add(&["--f", "", "--o", "o.mgz"]);
    add(&["--f", " ", "--o", "o.mgz"]);
    add(&["--f", "nofile.txt", "--o", "o.mgz"]);
    add(&["--f", "empty.txt", "--o", "o.mgz"]);
    add(&["--f", "empty.txt", "a.mgz", "--o", "o.mgz"]);
    add(&["--f", "listdir", "a.mgz", "--o", "o.mgz"]);
    add(&["--f", "listbad.txt", "--o", "o.mgz"]);
    add(&["--f", "longw.txt", "--o", "o.mgz"]);
    add(&["--f", "longlast.txt", "--o", "o.mgz"]);
    add(&["--f", "long2000.txt", "--o", "o.mgz"]);
    add(&["--f", "list.txt", "--debug", "--mean", "--o", "o.mgz"]);
    out
}

/// The cases of `seed`: the fixed ones, then seeded ones up to `n`.
pub fn generate(seed: u64, n: usize) -> Vec<String> {
    let mut out = prelude();
    let i = base_inputs();
    let mut g = Gen::new(seed);
    while out.len() < n {
        out.push(seeded(&mut g, &i));
    }
    out.truncate(n);
    out
}

/// The golden cases (`tests/mri_concat_golden.rs`): pctsurfcon's and `mri_mcadura_seg`'s calls,
/// a plain concatenation into NIfTI, the reductions, weights and a matrix, a vote, the pairs, a
/// NaN made by `--mul +nan`, NaN weights and an input with NaN and infinite voxels (made 0 when
/// read).
pub fn golden_cases() -> Vec<String> {
    let i = base_inputs();
    let mut out = Vec::new();
    for a in [
        &[
            "lh.wm.mgh",
            "lh.gm.mgh",
            "--paired-diff-norm",
            "--mul",
            "100",
            "--o",
            "o.mgh",
        ][..],
        &[
            "seg1.mgz", "seg2.mgz", "--sum", "--ctab", "lut.txt", "--o", "o.mgz",
        ],
        &["a.mgz", "c.mgz", "d.nii.gz", "--o", "o.nii.gz"],
        &["f4.mgz", "--mean", "--o", "o.mgz"],
        &["f4.mgz", "--std", "--o", "o.mgz"],
        &["f4.mgz", "--w", "w4.txt", "--max-index", "--o", "o.mgz"],
        &["f4.mgz", "--mtx", "m24.txt", "--o", "o.mgz"],
        &["seg4.mgz", "--vote", "--mask", "k.mgz", "--o", "o.mgz"],
        &["z.mgz", "--first-non-zero", "--o", "o.mgz"],
        &["a.mgz", "--mul", "+nan", "--o", "o.mgz"],
        &["f4.mgz", "--w", "wnan.txt", "--o", "o.mgz"],
        &["nan.mgz", "--sum", "--o", "o.mgz"],
        &["f4.mgz", "--pca", "--o", "o.mgz"],
        &["--zconcat", "zb.mgz", "zt.mgz", "1", "oz.mgz"],
        &["--f", "list.txt", "--median", "--o", "o.mgz"],
    ] {
        out.push(line(&[], &i, &s(a)));
    }
    out
}

// ------------------------------------------------------------------ the port's answer

/// Text files listed with their bytes.
fn is_text(name: &str) -> bool {
    [".txt", ".mtx", ".dat"].iter().any(|e| name.ends_with(e))
}

/// Every file of a case's folder but its inputs left as they were, sorted, each as `F <name>
/// <fnv> <size>` (text files with their bytes escaped; a resource usage file as `F <name>
/// <rusage>`), the folder's name in them as `<DIR>`; each folder the run made as `D <name>`.
pub fn written(dir: &Path, inputs: &[(String, Vec<u8>)], dirs: &[String]) -> String {
    let d = dir.to_string_lossy().into_owned();
    let mut out = Vec::new();
    for (n, b) in collect_all(dir, "") {
        match b {
            None => {
                let given = dirs.contains(&n)
                    || inputs.iter().any(|(f, _)| f.starts_with(&format!("{n}/")));
                if !given {
                    out.push(format!("D {n}"));
                }
            }
            Some(b) => {
                if inputs.iter().any(|(f, c)| *f == n && *c == b) {
                    continue;
                }
                let b = mask_bytes(&b, d.as_bytes());
                if n.ends_with(".rusage") {
                    out.push(format!("F {n} <rusage>"));
                } else if is_text(&n) {
                    out.push(format!("F {n} {}", escape(&b)));
                } else {
                    out.push(format!("F {n} {:016x} {}", fnv(&b), b.len()));
                }
            }
        }
    }
    out.join(" ")
}

fn collect_all(dir: &Path, prefix: &str) -> Vec<(String, Option<Vec<u8>>)> {
    let mut names: Vec<std::fs::DirEntry> = std::fs::read_dir(dir)
        .expect("the folder")
        .filter_map(|e| e.ok())
        .collect();
    names.sort_by_key(|e| e.file_name().to_string_lossy().into_owned().into_bytes());
    let mut out = Vec::new();
    for e in names {
        let n = format!("{prefix}{}", e.file_name().to_string_lossy());
        if e.path().is_dir() {
            out.push((n.clone(), None));
            out.extend(collect_all(&e.path(), &format!("{n}/")));
        } else {
            out.push((n, Some(std::fs::read(e.path()).expect("a file"))));
        }
    }
    out
}

/// A case's parsed line: its environment changes, its files, folders and arguments.
pub struct Case {
    pub env: Vec<String>,
    pub files: Vec<(String, Vec<u8>)>,
    pub dirs: Vec<String>,
    pub args: Vec<String>,
}

pub fn parse(line: &str) -> Case {
    let t: Vec<&str> = line.split_whitespace().collect();
    assert_eq!(t[0], "cat");
    let env = if t[1] == "-" {
        Vec::new()
    } else {
        String::from_utf8(unhex(t[1]))
            .expect("assignments")
            .split(';')
            .map(str::to_owned)
            .collect()
    };
    let nfiles: usize = t[2].parse().expect("a count");
    let mut i = 3;
    let mut files = Vec::new();
    for _ in 0..nfiles {
        files.push((
            String::from_utf8(unhex(t[i])).expect("a file name"),
            unhex(t[i + 1]),
        ));
        i += 2;
    }
    let ndirs: usize = t[i].parse().expect("a count");
    let dirs = (0..ndirs)
        .map(|k| String::from_utf8(unhex(t[i + 1 + k])).expect("a folder name"))
        .collect();
    i += 1 + ndirs;
    let nargs: usize = t[i].parse().expect("a count");
    let args = (0..nargs)
        .map(|k| String::from_utf8(unhex(t[i + 1 + k])).expect("an argument"))
        .collect();
    Case {
        env,
        files,
        dirs,
        args,
    }
}

/// Standard output with `--debug`'s resource usage masked.
pub fn mask_stdout(text: &str) -> String {
    text.split('\n')
        .map(|l| {
            if l.starts_with("mri_ca_label ") {
                "mri_ca_label <rusage>"
            } else {
                l
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The defects whose official result depends on memory contents (`reproduce` fills it
/// deterministically): the pseudo-notice `MEMORY`.
const MEMORY: [&str; 5] = ["FS-53", "FS-249", "FS-1196", "FS-1243", "FS-1247"];

/// The defects after which FreeSurfer's build crashes where the port ends with an error in both
/// modes (the library's choice): the time-course covariance that cannot be computed (FS-564) and
/// a matrix size that cannot be allocated (FS-13's `allocation failed`).
const CRASHES: [&str; 1] = ["FS-564"];

/// The port's line for a case line and the defects it noted (`dir`: an empty folder), with the
/// pseudo-notices `CRASH` and `MEMORY`.
pub fn run_line_in(line: &str, mode: UpstreamDefects, dir: &Path) -> (String, Vec<&'static str>) {
    let c = parse(line);
    for (name, b) in &c.files {
        let p = dir.join(name);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).expect("a folder");
        }
        std::fs::write(&p, b).expect("a case file");
    }
    for d in &c.dirs {
        std::fs::create_dir_all(dir.join(d)).expect("a folder");
    }
    let d = dir.to_string_lossy().into_owned();
    let mut env = probe_env();
    for a in &c.env {
        if let Some(name) = a.strip_prefix('-') {
            env.retain(|(k, _)| k != name);
        } else {
            let (k, v) = a.split_once('=').expect("NAME=value");
            env.retain(|(kk, _)| kk != k);
            env.push((k.to_owned(), v.replace("@DIR@", &d)));
        }
    }
    let environment = Environment::from_pairs(env);
    let mut g = Globals::new(mode);
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let status = {
        let mut terr = TrackedErr::new(&g, &mut err);
        let mut r = Run {
            out: &mut out,
            err: &mut terr,
            env: environment,
            dir: Some(dir),
        };
        match main(&mut g, &mut r, &c.args) {
            Ok(()) => 0,
            Err(f) => f.status(),
        }
    };
    let _ = out.flush();
    let mask = |b: &[u8]| escape(b).replace(&escape(d.as_bytes()), "<DIR>");
    let files = written(dir, &c.files, &c.dirs);
    let stdout = mask_stdout(&String::from_utf8_lossy(&out));
    let text = format!(
        "EXIT {status} | {} | {} | {files}",
        mask(stdout.as_bytes()),
        mask(&err)
    );
    let mut ids: Vec<&'static str> = g.notices.as_slice().iter().map(|n| n.defect.id).collect();
    if matches!(status, 134 | 136 | 139)
        || ids.iter().any(|x| CRASHES.contains(x))
        || (ids.contains(&"FS-13") && String::from_utf8_lossy(&err).contains("allocation failed"))
    {
        ids.push("CRASH");
    }
    if ids.iter().any(|x| MEMORY.contains(x)) {
        ids.push("MEMORY");
    }
    (text, ids)
}

/// [`run_line_in`] in a fresh temporary folder.
pub fn run_line(line: &str, mode: UpstreamDefects) -> (String, Vec<&'static str>) {
    let dir = tempfile::tempdir().expect("a temporary folder");
    run_line_in(line, mode, dir.path())
}
