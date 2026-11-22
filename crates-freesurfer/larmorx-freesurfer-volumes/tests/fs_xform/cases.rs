// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The fs-add-xform probe's cases (`validation/probes/fs-add-xform/`): a seeded generator of
//! `mri_add_xform_to_header` command lines with the files and folders they read, and the port's
//! answer to each, printed as `check.py` prints the official program's (`<status> | <stdout> |
//! <stderr> | <files>`, C-escaped).
//!
//! A case line is `xform <env> <nfiles> [<name> <hex>]... <ndirs> [<hex>]... <nargs> [<hex>]...`:
//! `<env>` is `-` or the hex of one `NAME=value` added to the probe's environment; the files are
//! written into the case's folder (their folders made first), then the (empty) folders made; the
//! arguments follow the program's name. The volumes are written by the port's own writers
//! (`fs_info/cases.rs`' builders), so both sides read the same bytes. Every file of the folder
//! after the run is listed (the program rewrites its input in place).
#![allow(dead_code)]

#[path = "../fs_info/cases.rs"]
mod info;

use std::io::Write;
use std::path::Path;

use larmorx_core::defects::UpstreamDefects;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::TrackedErr;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_io::identify::MRI_CORONAL_SLICE_DIRECTORY;
use larmorx_freesurfer_io::mriio::{mri_write, mri_write_type};
use larmorx_freesurfer_mri::mri::{MGZ_INTENT_UNKNOWN, MRI_FLOAT, MRI_SHORT, MRI_UCHAR, Mri};
use larmorx_freesurfer_volumes::mri_add_xform_to_header::main::{Run, main};

pub use info::{Gen, escape, hex, probe_env, unhex};
use info::{Spec, build, collect, fnv, mask_bytes, rand_spec};

// ------------------------------------------------------------------ the files

/// A good linear transform (`fs_info`'s).
const XFM: &[u8] = info::XFM;
/// The same, inverted (`Invert_Flag = True`).
const XFM_INV: &[u8] = b"MNI Transform File\n\nTransform_Type = Linear;\nInvert_Flag = True;\nLinear_Transform =\n 1.05 0.01 -0.02 -1.5\n -0.01 1.1 0.03 2.25\n 0.02 -0.03 0.95 -3.0;\n";
/// Two linear transforms (concatenated into one).
const XFM_TWO: &[u8] = b"MNI Transform File\nTransform_Type = Linear;\nLinear_Transform =\n 1 0 0 2\n 0 1 0 -3\n 0 0 1 4;\nTransform_Type = Linear;\nLinear_Transform =\n 0 1 0 0\n -1 0 0 1\n 0 0 1 0;\n";
/// A good transform, then one of a type FreeSurfer does not read: the first stays read.
const XFM_PART: &[u8] = b"MNI Transform File\nTransform_Type = Linear;\nLinear_Transform =\n 2 0 0 1\n 0 2 0 1\n 0 0 2 1;\nTransform_Type = Thin_Plate_Spline_Transform;\n";
/// A wrong header.
const XFM_BAD: &[u8] = b"MNI Transform Fil\nTransform_Type = Linear;\n";
/// An element that is not a number.
const XFM_NAN: &[u8] =
    b"MNI Transform File\nTransform_Type = Linear;\nLinear_Transform =\n 1 0 0 0\n 0 1 x 0\n 0 0 1 0;\n";
/// A singular matrix, inverted (FreeSurfer's inverse fails: the identity both ways).
const XFM_SING: &[u8] = b"MNI Transform File\nTransform_Type = Linear;\nInvert_Flag = True;\nLinear_Transform =\n 1 2 3 0\n 2 4 6 0\n 0 0 0 0;\n";
/// No transform after the header.
const XFM_NONE: &[u8] = b"MNI Transform File\n% nothing\n";

/// The transform files of the cases, by name.
const XFMS: &[(&str, &[u8])] = &[
    ("t.xfm", XFM),
    ("inv.xfm", XFM_INV),
    ("two.xfm", XFM_TWO),
    ("part.xfm", XFM_PART),
    ("bad.xfm", XFM_BAD),
    ("nan.xfm", XFM_NAN),
    ("sing.xfm", XFM_SING),
    ("none.xfm", XFM_NONE),
    ("empty.xfm", b""),
    ("tt.xfm", XFM),
];

/// The transform names the cases give (files of [`XFMS`], a name `.xfm` completes, a missing
/// file, a folder, an empty name).
const XFM_NAMES: &[&str] = &[
    "t.xfm",
    "t.xfm",
    "inv.xfm",
    "two.xfm",
    "part.xfm",
    "bad.xfm",
    "nan.xfm",
    "sing.xfm",
    "none.xfm",
    "empty.xfm",
    "tt",
    "gone.xfm",
    "gone",
    "xd.xfm",
    "sub/t.xfm",
    "",
];

fn spec(ty: i32, dims: [i32; 4], xform: Option<&str>) -> Spec {
    Spec {
        dims,
        ty,
        size: [1.0, 1.0, 1.0],
        mdc: [-1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 1.0, 0.0],
        c: [0.5, -2.0, 1.0],
        good: 1,
        tr: 2300.0,
        flip: 0.157_079_632_679_489_66,
        te: 2.98,
        ti: 900.0,
        fov: 256.0,
        dof: 1,
        xform: xform.map(str::to_owned),
        pedir: Some("ROW".to_owned()),
        cmds: vec!["mri_convert a.mgz orig.mgz --conform".to_owned()],
        auto_align: None,
        orig: None,
        field: 3.0,
        frames: Vec::new(),
        ctab: false,
        seg: false,
    }
}

/// The files of `m` written as `name`: `cor` gives a COR folder, any other name the format it
/// names (`mri_write`).
fn write_as(m: &mut Mri, name: &str, cor: bool) -> Vec<(String, Vec<u8>)> {
    let t = tempfile::tempdir().expect("a temporary folder");
    let mut fs = Globals::new(UpstreamDefects::Fix);
    let (mut o, mut e) = (Vec::new(), Vec::new());
    let env = Environment::default();
    let _ = if cor {
        mri_write_type(
            &mut fs,
            &mut o,
            &mut e,
            &env,
            Some(t.path()),
            m,
            name,
            MRI_CORONAL_SLICE_DIRECTORY,
            MGZ_INTENT_UNKNOWN,
        )
    } else {
        mri_write(
            &mut fs,
            &mut o,
            &mut e,
            &env,
            Some(t.path()),
            m,
            name,
            None,
            MGZ_INTENT_UNKNOWN,
        )
    };
    collect(t.path())
}

/// A COR folder `name` of a 4 x 4 x 3 byte volume (slice thickness `zsize`), naming `xform`.
fn cor(g: &mut Gen, name: &str, zsize: f32, xform: Option<&str>) -> Vec<(String, Vec<u8>)> {
    let s = spec(MRI_UCHAR, [4, 4, 3, 1], xform);
    let mut m = build(g, &s);
    m.vg.zsize = zsize;
    m.imnr0 = 1;
    m.imnr1 = 3;
    m.fov = 4.0;
    m.thick = zsize;
    m.ps = 1.0;
    write_as(&mut m, name, true)
}

/// The fixed inputs every case of the prelude has: volumes, COR folders, transform files.
struct Inputs {
    files: Vec<(String, Vec<u8>)>,
    dirs: Vec<String>,
}

fn base_inputs(g: &mut Gen) -> Inputs {
    let mut files = Vec::new();
    // MGZ volumes: naming a transform that loads, one that is missing, one that does not read,
    // one that fails after its first transform; an MGH volume without tags
    for (name, ty, xform) in [
        ("v.mgz", MRI_SHORT, Some("t.xfm")),
        ("x.mgz", MRI_UCHAR, Some("gone.xfm")),
        ("y.mgz", MRI_FLOAT, Some("bad.xfm")),
        ("z.mgz", MRI_SHORT, Some("part.xfm")),
        ("w.mgz", MRI_UCHAR, None),
    ] {
        let mut m = build(g, &spec(ty, [3, 4, 2, 1], xform));
        files.extend(write_as(&mut m, name, false));
    }
    let mut p = build(g, &spec(MRI_FLOAT, [2, 3, 2, 2], None));
    p.pedir = None;
    p.cmdlines.clear();
    files.extend(write_as(&mut p, "p.mgh", false));
    // the same bytes under names FreeSurfer refuses or reads otherwise (none differing from
    // another only by case: Windows' names do not)
    let v = files[0].1.clone();
    for name in ["U.MGZ", "noext", "a.b/c", "v.nii.gz"] {
        files.push((name.to_owned(), v.clone()));
    }
    let mut n = build(g, &spec(MRI_FLOAT, [2, 2, 2, 1], None));
    files.extend(write_as(&mut n, "n.nii", false));
    files.push(("c.mgz".to_owned(), b"\x1f\x8b\x08\x00not a volume".to_vec()));
    // an MGH volume cut short in its voxels
    let mut t = build(g, &spec(MRI_FLOAT, [3, 3, 3, 1], None));
    let mut tf = write_as(&mut t, "trunc.mgh", false);
    tf[0].1.truncate(300);
    files.extend(tf);
    // COR folders: naming a transform it holds, none (slices 2 mm thick), a missing one
    let mut c = cor(g, "cor", 1.0, Some("t.xfm"));
    c.push(("cor/t.xfm".to_owned(), XFM.to_vec()));
    files.extend(c);
    files.extend(cor(g, "cor2", 2.0, None));
    files.extend(cor(g, "cor3", 1.0, Some("missing.xfm")));
    for (name, b) in XFMS {
        files.push(((*name).to_owned(), b.to_vec()));
    }
    files.push(("sub/t.xfm".to_owned(), XFM_INV.to_vec()));
    // a folder whose COR-.info is itself a folder (it cannot be opened)
    let dirs = vec![
        "outdir".to_owned(),
        "o2/COR-.info".to_owned(),
        "xd.xfm".to_owned(),
        "emptydir".to_owned(),
    ];
    Inputs { files, dirs }
}

/// The transform files a volume's header names (each one of the volumes above, and its copies
/// under other names).
fn tag_files(name: &str) -> &'static [&'static str] {
    match name {
        "v.mgz" | "U.MGZ" | "noext" | "trail." | "a.b/c" | "v.nii.gz" | "r.mgz" | "r.mgh" => {
            &["t.xfm"]
        }
        "y.mgz" => &["bad.xfm"],
        "z.mgz" => &["part.xfm"],
        _ => &[],
    }
}

/// Whether the argument `a` names the file or folder `f`: itself, a file in it, the name `.xfm`
/// completes.
fn names(a: &str, f: &str) -> bool {
    let a = a.trim_end_matches('/');
    !a.is_empty()
        && (f == a
            || f.strip_prefix(a).is_some_and(|r| r.starts_with('/'))
            || f.strip_suffix(".xfm") == Some(a))
}

/// A case line: the inputs' files and folders the arguments name (and the transform files the
/// volumes named name), so that each case writes only what it reads.
fn line(env: Option<&str>, i: &Inputs, args: &[String]) -> String {
    let mut wanted: Vec<&str> = Vec::new();
    for (f, _) in &i.files {
        if args.iter().any(|a| names(a, f)) {
            wanted.push(f);
            wanted.extend_from_slice(tag_files(f));
        }
    }
    let files: Vec<&(String, Vec<u8>)> = i
        .files
        .iter()
        .filter(|(f, _)| wanted.contains(&f.as_str()))
        .collect();
    let dirs: Vec<&String> = i
        .dirs
        .iter()
        .filter(|d| args.iter().any(|a| names(a, d)))
        .collect();
    let mut t = vec![
        "xform".to_owned(),
        env.map_or("-".to_owned(), |e| hex(e.as_bytes())),
        files.len().to_string(),
    ];
    for (f, c) in files {
        t.push(f.clone());
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

/// The inputs the cases name: every kind of volume, folder and failure.
const INPUTS: &[&str] = &[
    "v.mgz",
    "x.mgz",
    "y.mgz",
    "z.mgz",
    "w.mgz",
    "p.mgh",
    "U.MGZ",
    "noext",
    "a.b/c",
    "v.nii.gz",
    "n.nii",
    "c.mgz",
    "trunc.mgh",
    "cor",
    "cor/",
    "cor2",
    "cor3",
    "outdir",
    "emptydir",
    "gone.mgz",
    "",
];

/// The outputs the cases name (besides the input itself).
const OUTPUTS: &[&str] = &[
    "o.mgz",
    "o.mgh",
    "o.nii",
    "o.nii.gz",
    "o.foo",
    "nodir/o.mgz",
    "outdir",
    "outdir/",
    "o2",
    "cor2",
    "v.mgz",
    "o.img",
];

/// The fixed cases: the command line's paths, each input, each transform, each output.
fn prelude() -> Vec<String> {
    let mut g = Gen::new(0xadd_f0e);
    let base = base_inputs(&mut g);
    let mut out = Vec::new();
    for args in [
        s(&[]),
        s(&["--version"]),
        s(&["--version", "-c", "t.xfm", "v.mgz", "o.mgz"]),
        s(&["--help"]),
        s(&["--usage"]),
        s(&["--HELP"]),
        s(&["--Usage", "t.xfm", "v.mgz"]),
        s(&["-h"]),
        s(&["-H"]),
        s(&["-?"]),
        s(&["-u"]),
        s(&["-U"]),
        s(&["-help"]),
        s(&["-x"]),
        s(&["--x"]),
        s(&["-"]),
        s(&["-c"]),
        s(&["-v"]),
        s(&["-n"]),
        s(&["-n", "a"]),
        s(&["-n", "-c"]),
        s(&["-n", "a", "t.xfm", "v.mgz", "o.mgz"]),
        s(&["-n", "t.xfm", "v.mgz", "o.mgz"]),
        s(&["-c", "t.xfm"]),
        s(&["t.xfm"]),
        s(&["t.xfm", "v.mgz", "o.mgz", "extra", "more"]),
        s(&["-c", "-c", "-V", "t.xfm", "v.mgz", "o.mgz"]),
        s(&["-copy", "-verbose", "t.xfm", "v.mgz", "o.mgz"]),
        s(&["-v", "-v", "t.xfm", "v.mgz", "o.mgz"]),
        s(&["-C", "gone.xfm", "v.mgz", "o.mgz"]),
        s(&["-c", "t.xfm", "v.mgz", "-v"]),
        s(&["", "v.mgz", "o.mgz"]),
        s(&["-c", "", "v.mgz", "o.mgz"]),
    ] {
        out.push(line(None, &base, &args));
    }
    // each input, with and without loading, in place and to another file
    for input in INPUTS {
        out.push(line(None, &base, &s(&["-c", "t.xfm", input])));
        out.push(line(None, &base, &s(&["t.xfm", input, "o.mgz"])));
        out.push(line(None, &base, &s(&["-v", "gone.xfm", input, "outdir"])));
    }
    // each transform, read into an MGZ volume and a COR folder
    for x in XFM_NAMES {
        out.push(line(None, &base, &s(&[x, "w.mgz", "o.mgz"])));
        out.push(line(None, &base, &s(&["-v", x, "v.mgz"])));
        out.push(line(None, &base, &s(&[x, "cor2", "outdir"])));
        out.push(line(None, &base, &s(&[x, "x.mgz", "outdir"])));
        out.push(line(None, &base, &s(&["-c", x, "z.mgz", "outdir"])));
    }
    // each output
    for o in OUTPUTS {
        for input in ["v.mgz", "cor", "p.mgh"] {
            out.push(line(None, &base, &s(&["-c", "t.xfm", input, o])));
            out.push(line(None, &base, &s(&["inv.xfm", input, o])));
        }
    }
    // the environment: no tags read, verbose diagnostics
    for env in ["FS_SKIP_TAGS=1", "DIAG_VERBOSE=1"] {
        for args in [
            s(&["t.xfm", "v.mgz", "o.mgz"]),
            s(&["-c", "two.xfm", "z.mgz", "outdir"]),
            s(&["t.xfm", "cor", "outdir"]),
        ] {
            out.push(line(Some(env), &base, &args));
        }
    }
    // a transform name of 1,024 bytes or more (FS-1154), copied into an MGZ and an MGH volume
    let long = format!("{}.xfm", "L".repeat(1100));
    for o in ["o.mgz", "o.mgh"] {
        out.push(line(None, &base, &s(&["-c", &long, "v.mgz", o])));
    }
    out
}

/// A seeded case: a random volume (or one of the fixed inputs), zero to three options, a
/// transform, an output.
fn seeded(g: &mut Gen) -> String {
    let mut base = base_inputs(&mut Gen::new(0xadd_f0e));
    let input = if g.chance(2) {
        let spec = rand_spec(g);
        let ext = g.pick(&[".mgz", ".mgz", ".mgh"]);
        let name = format!("r{ext}");
        let mut m = build(g, &spec);
        base.files.extend(write_as(&mut m, &name, false));
        name
    } else {
        g.pick(INPUTS).to_owned()
    };
    let mut args: Vec<String> = Vec::new();
    for _ in 0..g.int(0, 3) {
        match g.below(6) {
            0 => args.extend(s(&["-n", "skipped"])),
            1 => args.push("-v".to_owned()),
            2 => args.push(g.pick(&["-c", "-C", "-copy"]).to_owned()),
            _ => args.push("-c".to_owned()),
        }
    }
    args.push(g.pick(XFM_NAMES).to_owned());
    args.push(input);
    if g.below(4) != 0 {
        args.push(g.pick(OUTPUTS).to_owned());
    }
    let env = match g.below(10) {
        0 => Some("FS_SKIP_TAGS=1"),
        1 => Some("DIAG_VERBOSE=1"),
        _ => None,
    };
    line(env, &base, &args)
}

/// The cases of a seed: the fixed ones, then seeded ones, `n` in all.
pub fn generate(seed: u64, n: usize) -> Vec<String> {
    let mut out = prelude();
    let mut g = Gen::new(seed);
    while out.len() < n {
        out.push(seeded(&mut g));
    }
    out.truncate(n);
    out
}

/// The cases only Linux's file names hold (not in the recorded test): a COR folder 1,014 bytes or
/// more deep (FS-1156; macOS's paths stop at 1,024 bytes), written with and without a transform
/// name; a volume whose name ends in a dot (Windows drops a name's last dots).
pub fn extra_cases() -> Vec<String> {
    let mut g = Gen::new(0xadd_f0e);
    let mut base = base_inputs(&mut g);
    let deep = (0..5)
        .map(|k| format!("{k}{}", "d".repeat(210)))
        .collect::<Vec<_>>()
        .join("/");
    base.dirs.push(deep.clone());
    let v = base.files[0].1.clone();
    base.files.push(("trail.".to_owned(), v));
    let mut out = Vec::new();
    for args in [
        s(&["-c", "t.xfm", "v.mgz", &deep]),
        s(&["t.xfm", "cor", &deep]),
        s(&["-c", "t.xfm", "trail."]),
        s(&["t.xfm", "trail.", "o.mgz"]),
        s(&["-v", "gone.xfm", "trail.", "outdir"]),
    ] {
        out.push(line(None, &base, &args));
    }
    out
}

// ------------------------------------------------------------------ the golden cases

/// The golden cases (`tests/mri_add_xform_to_header_golden.rs`): a 32^3 byte volume (a head-like
/// pattern made with integer arithmetic) as MGZ and as a COR folder, the transform added as
/// recon-all adds it and read whole.
pub fn golden_cases() -> Vec<String> {
    const N: i32 = 32;
    let mut g = Gen::new(32);
    let mut fs = Globals::new(UpstreamDefects::Fix);
    let mut e: Vec<u8> = Vec::new();
    let mut m = build(&mut g, &spec(MRI_UCHAR, [N, N, N, 1], None));
    let c = N / 2;
    for z in 0..N {
        for y in 0..N {
            for x in 0..N {
                let d2 = (x - c) * (x - c) + (y - c) * (y - c) + (z - c) * (z - c);
                let v = if d2 < 52 {
                    110
                } else if d2 < 144 {
                    75
                } else {
                    (x * 7 + y * 13 + z * 29) % 9
                };
                larmorx_freesurfer_mri::voxel::mri_set_vox_val(
                    &mut fs, &mut e, &mut m, x, y, z, 0, v as f32, 0,
                )
                .expect("a voxel");
            }
        }
    }
    m.imnr0 = 1;
    m.imnr1 = N;
    m.ps = 1.0;
    m.thick = 1.0;
    let mut files = write_as(&mut m, "orig.mgz", false);
    files.extend(write_as(&mut m, "cor", true));
    files.push(("transforms/talairach.xfm".to_owned(), XFM.to_vec()));
    let base = Inputs {
        files,
        dirs: vec!["outdir".to_owned()],
    };
    let mut out = Vec::new();
    for args in [
        s(&["-c", "transforms/talairach.xfm", "orig.mgz", "orig.mgz"]),
        s(&["transforms/talairach.xfm", "orig.mgz", "nu.mgz"]),
        s(&["-v", "transforms/talairach.xfm", "orig.mgz", "out.mgh"]),
        s(&["transforms/talairach.xfm", "cor", "outdir"]),
        s(&["-c", "transforms/talairach.xfm", "cor", "cor"]),
    ] {
        out.push(line(None, &base, &args));
    }
    out
}

// ------------------------------------------------------------------ the port's answer

/// Text files listed with their bytes: transform files, COR folders' `COR-.info`, `.txt`.
fn is_text(name: &str) -> bool {
    name.ends_with(".xfm") || name.ends_with(".info") || name.ends_with(".txt")
}

/// Every file of a case's folder, sorted, each as `F <name> <fnv> <size>` (text files with their
/// bytes escaped), the folder's name in them as `<DIR>`; each folder as `D <name>`.
pub fn written(dir: &Path) -> String {
    let d = dir.to_string_lossy().into_owned();
    let mut out = Vec::new();
    for (n, b) in collect_all(dir, "") {
        match b {
            None => out.push(format!("D {n}")),
            Some(b) => {
                let b = mask_bytes(&b, d.as_bytes());
                if is_text(&n) {
                    out.push(format!("F {n} {}", escape(&b)));
                } else {
                    out.push(format!("F {n} {:016x} {}", fnv(&b), b.len()));
                }
            }
        }
    }
    out.join(" ")
}

/// Every file and folder under `dir`, sorted by name's bytes (a folder: `None`).
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

/// Builds a case's folder (`dir`: empty) and returns its arguments and environment.
fn setup(line: &str, dir: &Path) -> (Vec<String>, Vec<(String, String)>) {
    let t: Vec<&str> = line.split_whitespace().collect();
    assert_eq!(t[0], "xform");
    let mut env = probe_env();
    if t[1] != "-" {
        let e = String::from_utf8(unhex(t[1])).expect("an assignment");
        let (k, v) = e.split_once('=').expect("NAME=value");
        env.push((k.to_owned(), v.to_owned()));
    }
    let nfiles: usize = t[2].parse().expect("a count");
    let mut i = 3;
    for _ in 0..nfiles {
        let p = dir.join(t[i]);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).expect("a folder");
        }
        std::fs::write(&p, unhex(t[i + 1])).expect("a case file");
        i += 2;
    }
    let ndirs: usize = t[i].parse().expect("a count");
    for k in 0..ndirs {
        let name = String::from_utf8(unhex(t[i + 1 + k])).expect("a folder name");
        std::fs::create_dir_all(dir.join(name)).expect("a folder");
    }
    i += 1 + ndirs;
    let nargs: usize = t[i].parse().expect("a count");
    let args = (0..nargs)
        .map(|k| String::from_utf8(unhex(t[i + 1 + k])).expect("an argument"))
        .collect();
    (args, env)
}

/// The port's line for a case line and the defects it noted (`dir`: an empty folder).
pub fn run_line_in(line: &str, mode: UpstreamDefects, dir: &Path) -> (String, Vec<&'static str>) {
    let (args, env) = setup(line, dir);
    let environment = Environment::from_pairs(env);
    let mut g = Globals::new(mode);
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let status = {
        let mut terr = TrackedErr::new(&g, &mut err);
        let mut r = Run {
            out: &mut out,
            err: &mut terr,
            env: &environment,
            dir: Some(dir),
        };
        match main(&mut g, &mut r, &args) {
            Ok(()) => 0,
            Err(f) => f.status(),
        }
    };
    let _ = out.flush();
    let d = dir.to_string_lossy().into_owned();
    let mask = |b: &[u8]| escape(b).replace(&escape(d.as_bytes()), "<DIR>");
    let files = written(dir);
    let text = format!("EXIT {status} | {} | {} | {files}", mask(&out), mask(&err));
    let ids = g.notices.as_slice().iter().map(|n| n.defect.id).collect();
    (text, ids)
}

/// [`run_line_in`] in a fresh temporary folder.
pub fn run_line(line: &str, mode: UpstreamDefects) -> (String, Vec<&'static str>) {
    let dir = tempfile::tempdir().expect("a temporary folder");
    run_line_in(line, mode, dir.path())
}
