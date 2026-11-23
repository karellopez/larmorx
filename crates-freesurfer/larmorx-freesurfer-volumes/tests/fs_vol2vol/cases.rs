// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The fs-vol2vol probe's cases (`validation/probes/fs-vol2vol/`): a seeded generator of
//! `mri_vol2vol` command lines with the files they read, and the port's answer to each, printed as
//! `check.py` prints the official program's (`<status> | <stdout> | <stderr> | <files>`,
//! C-escaped).
//!
//! A case line is `v2v <env> <nfiles> [<name> <hex>]... <ndirs> [<hex>]... <nargs> [<hex>]...`, as
//! the fs-ltaconv probe's: `<env>` is `-` or the hex of assignments (separated by `;`) changing the
//! probe's environment: `NAME=value` sets (`@DIR@` in the value is the case's folder), `-NAME`
//! removes. Every case sets `FREESURFER_HOME` and `SUBJECTS_DIR` to folders of its own
//! (`fsh/`, `subjects/`) unless it removes them. The answer's lines mask the noise seed (`SynthSeed
//! <seed>`, a time of day unless `--seed` is given), `VmPeak <kB>` and the `# created by` line of
//! the transforms written (user and date). Pseudo-notices, which `check.py` counts, not compares:
//! `CRASH` (a status FreeSurfer's build reaches by a signal: 134, 136, 139; its unflushed standard
//! output is lost) and `NOTPORTED` (a path relay 53 leaves for later).
#![allow(dead_code)]

#[path = "../fs_info/cases.rs"]
mod info;

use std::io::Write;
use std::path::Path;

use larmorx_core::defects::UpstreamDefects;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::TrackedErr;
use larmorx_freesurfer_core::matrix::Matrix;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_mri::lta::{LINEAR_RAS_TO_RAS, LINEAR_VOX_TO_VOX, lta_alloc};
use larmorx_freesurfer_mri::mri::{MRI_FLOAT, MRI_INT, MRI_SHORT, MRI_UCHAR, Mri};
use larmorx_freesurfer_mri::voxel::mri_set_vox_val;
use larmorx_freesurfer_volume::ltawrite::lta_write;
use larmorx_freesurfer_volume::vgeom::get_vol_geom;
use larmorx_freesurfer_volumes::mri_vol2vol::main::{Run, main};

pub use info::{Gen, escape, hex, probe_env, unhex};
use info::{LUT, Spec, XFM, build, fnv, mask_bytes};

// ------------------------------------------------------------------ the files

fn spec(ty: i32, dims: [i32; 4], size: [f32; 3], mdc: [f32; 9], c: [f32; 3]) -> Spec {
    Spec {
        dims,
        ty,
        size,
        mdc,
        c,
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

const IDENT: [f32; 9] = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];
const CORONAL: [f32; 9] = [-1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 1.0, 0.0];
/// 30 degrees about the superior axis.
const OBLIQUE: [f32; 9] = [0.866_025_4, 0.5, 0.0, -0.5, 0.866_025_4, 0.0, 0.0, 0.0, 1.0];

/// A registration: a rotation of about 10 degrees and a shift (rows, `float`).
const ROT: [f32; 16] = [
    0.984_807_7,
    -0.173_648_2,
    0.0,
    1.5,
    0.173_648_2,
    0.984_807_7,
    0.0,
    -0.75,
    0.0,
    0.0,
    1.0,
    0.5,
    0.0,
    0.0,
    0.0,
    1.0,
];

fn matrix_of(v: &[f32; 16]) -> Matrix {
    let mut m = Matrix::real(4, 4);
    for r in 0..4 {
        for c in 0..4 {
            m.set_relt(r + 1, c + 1, v[(r * 4 + c) as usize]);
        }
    }
    m
}

fn rows(v: &[f32; 16]) -> String {
    let mut s = String::new();
    for r in 0..4 {
        let row: Vec<String> = (0..4).map(|c| format!("{:.6}", v[r * 4 + c])).collect();
        s.push_str(&row.join(" "));
        s.push('\n');
    }
    s
}

/// The file `lta_write` makes of an array from `src` to `dst` of `ty` with `m` (its `# created
/// by` line fixed, so that the case's bytes never change).
fn lta_file(src: &Mri, dst: &Mri, ty: i32, m: &[f32; 16], valid: [i32; 2]) -> Vec<u8> {
    let t = tempfile::tempdir().expect("a temporary folder");
    let mut fs = Globals::new(UpstreamDefects::Fix);
    let (mut o, mut e) = (Vec::new(), Vec::new());
    let mut lta = lta_alloc(&mut fs, &mut o, &mut e, 1, None).expect("an array");
    lta.ltype = ty;
    lta.xforms[0].ltype = ty;
    lta.xforms[0].m_l = Some(matrix_of(m));
    get_vol_geom(&src.vg, &mut lta.xforms[0].src);
    get_vol_geom(&dst.vg, &mut lta.xforms[0].dst);
    lta.xforms[0].src.valid = valid[0];
    lta.xforms[0].dst.valid = valid[1];
    lta.subject.strcpy_bytes(b"s1");
    lta.fscale = 0.1;
    let env = Environment::default();
    lta_write(&mut fs, &mut o, &mut e, &env, Some(t.path()), &lta, "x.lta").expect("written");
    let b = std::fs::read(t.path().join("x.lta")).expect("the file");
    let text = String::from_utf8_lossy(&b);
    text.lines()
        .map(|l| {
            if l.starts_with("# created by") {
                "# created by probe on Sun Nov 22 00:00:00 2026".to_owned()
            } else {
                l.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        .into_bytes()
        .into_iter()
        .chain(*b"\n")
        .collect()
}

/// A tkregister `register.dat` of `m` for `subject`.
fn reg_dat(subject: &str, m: &[f32; 16], method: &str) -> Vec<u8> {
    format!("{subject}\n1.000000\n1.500000\n0.150000\n{}{method}", rows(m)).into_bytes()
}

/// The inputs a case may name.
pub struct Inputs {
    files: Vec<(String, Vec<u8>)>,
    dirs: Vec<String>,
}

/// The volume of `s` with every voxel in the border of `pad` voxels set to 0 (`zero`: all of
/// them).
fn padded(g: &mut Gen, s: &Spec, pad: i32, zero: bool) -> Mri {
    let mut m = build(g, s);
    let mut fs = Globals::new(UpstreamDefects::Fix);
    let mut e = Vec::new();
    let [w, h, d, nf] = s.dims;
    for f in 0..nf {
        for z in 0..d {
            for y in 0..h {
                for x in 0..w {
                    let border = x < pad
                        || y < pad
                        || z < pad
                        || x >= w - pad
                        || y >= h - pad
                        || z >= d - pad;
                    if zero || border {
                        mri_set_vox_val(&mut fs, &mut e, &mut m, x, y, z, f, 0.0, 0).expect("a voxel");
                    }
                }
            }
        }
    }
    m
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

fn base_inputs() -> Inputs {
    let mut g = Gen::new(0x5e2_7017);
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    let put = |files: &mut Vec<(String, Vec<u8>)>, name: &str, m: &mut Mri| {
        files.extend(write_as(m, name));
    };
    // the moving volumes
    let sm = spec(MRI_FLOAT, [8, 9, 10, 1], [1.0, 1.5, 2.0], CORONAL, [1.0, -2.0, 3.0]);
    let mut m = build(&mut g, &sm);
    put(&mut files, "m.mgz", &mut m);
    put(&mut files, "m.nii.gz", &mut m);
    let mut m4 = build(
        &mut g,
        &spec(MRI_FLOAT, [6, 5, 4, 3], [1.0, 1.0, 1.0], IDENT, [0.0, 0.0, 0.0]),
    );
    put(&mut files, "m4.mgz", &mut m4);
    let mut mu = build(
        &mut g,
        &spec(MRI_UCHAR, [7, 6, 5, 1], [1.1, 0.9, 1.3], OBLIQUE, [0.5, 1.5, -2.5]),
    );
    put(&mut files, "mu.mgz", &mut mu);
    let mut ms = build(
        &mut g,
        &spec(MRI_SHORT, [6, 7, 5, 1], [1.0, 1.0, 1.0], IDENT, [2.0, 0.0, -1.0]),
    );
    put(&mut files, "ms.nii.gz", &mut ms);
    let mut sseg = spec(MRI_INT, [6, 6, 6, 1], [1.0, 1.0, 1.0], CORONAL, [0.0, 0.0, 0.0]);
    sseg.seg = true;
    sseg.ctab = true;
    let mut seg = build(&mut g, &sseg);
    put(&mut files, "seg.mgz", &mut seg);
    let mut thin = build(
        &mut g,
        &spec(MRI_FLOAT, [6, 7, 1, 1], [1.0, 1.0, 3.0], IDENT, [0.0, 0.0, 0.0]),
    );
    put(&mut files, "thin.mgz", &mut thin);
    let sbox = spec(MRI_FLOAT, [7, 6, 8, 1], [1.0, 1.0, 1.0], CORONAL, [0.0, 1.0, 0.0]);
    let mut bx = padded(&mut g, &sbox, 1, false);
    put(&mut files, "box.mgz", &mut bx);
    let mut zero = padded(&mut g, &sbox, 0, true);
    put(&mut files, "zero.mgz", &mut zero);
    // the targets
    let st = spec(MRI_UCHAR, [5, 6, 4, 1], [1.2, 1.0, 1.3], IDENT, [0.5, -1.0, 2.0]);
    let mut t = build(&mut g, &st);
    put(&mut files, "t.mgz", &mut t);
    let mut t2 = build(
        &mut g,
        &spec(MRI_FLOAT, [6, 5, 6, 1], [0.8, 1.0, 1.1], OBLIQUE, [-1.0, 0.5, 1.0]),
    );
    put(&mut files, "t2.nii.gz", &mut t2);
    // a voxel shift map of the moving volume's size, small shifts, a zero row
    let mut vsm = build(&mut g, &sm);
    {
        let mut fs = Globals::new(UpstreamDefects::Fix);
        let mut e = Vec::new();
        for z in 0..10 {
            for y in 0..9 {
                for x in 0..8 {
                    let v = if y == 4 {
                        0.0
                    } else {
                        ((x + 2 * y + 3 * z) % 7 - 3) as f32 * 0.4
                    };
                    mri_set_vox_val(&mut fs, &mut e, &mut vsm, x, y, z, 0, v, 0).expect("a voxel");
                }
            }
        }
    }
    put(&mut files, "vsm.mgz", &mut vsm);
    // the transforms
    files.push(("r.lta".into(), lta_file(&m, &t, LINEAR_RAS_TO_RAS, &ROT, [1, 1])));
    files.push(("v.lta".into(), lta_file(&m, &t, LINEAR_VOX_TO_VOX, &ROT, [1, 1])));
    files.push(("rt.lta".into(), lta_file(&t, &m, LINEAR_RAS_TO_RAS, &ROT, [1, 1])));
    files.push(("nos.lta".into(), lta_file(&m, &t, LINEAR_RAS_TO_RAS, &ROT, [0, 1])));
    files.push(("nod.lta".into(), lta_file(&m, &t, LINEAR_RAS_TO_RAS, &ROT, [1, 0])));
    files.push(("seg.lta".into(), lta_file(&seg, &t2, LINEAR_RAS_TO_RAS, &ROT, [1, 1])));
    files.push(("lta.txt".into(), lta_file(&m, &t, LINEAR_RAS_TO_RAS, &ROT, [1, 1])));
    files.push(("r.dat".into(), reg_dat("s1", &ROT, "round\n")));
    files.push(("rt.dat".into(), reg_dat("s1", &ROT, "tkregister\n")));
    files.push(("bad.dat".into(), b"s1\n1.0\n1.5\n".to_vec()));
    files.push((
        "f.mat".into(),
        rows(&[
            1.0, 0.0, 0.0, 2.0, 0.0, 0.98, -0.2, 1.0, 0.0, 0.2, 0.98, -1.0, 0.0, 0.0, 0.0, 1.0,
        ])
        .into_bytes(),
    ));
    files.push((
        "f2.mat".into(),
        rows(&[
            1.0, 0.0, 0.0, 0.5, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.25, 0.0, 0.0, 0.0, 1.0,
        ])
        .into_bytes(),
    ));
    files.push(("fbad.mat".into(), b"1 0 0\n".to_vec()));
    files.push(("x.xfm".into(), XFM.to_vec()));
    files.push(("xbad.xfm".into(), b"not a transform\n1 2 3\n".to_vec()));
    files.push(("lut.txt".into(), LUT.to_vec()));
    files.push(("badlut.txt".into(), b"not a table\n".to_vec()));
    // a subject and FreeSurfer's and FSL's own folders
    put(&mut files, "subjects/s1/mri/orig.mgz", &mut t);
    put(&mut files, "subjects/s1/mri/brain.mgz", &mut t2);
    files.push(("subjects/s1/mri/transforms/talairach.xfm".into(), XFM.to_vec()));
    files.push((
        "subjects/s1/mri/transforms/reg.mni152.2mm.dat".into(),
        reg_dat("s1", &ROT, "round\n"),
    ));
    let ident: [f32; 16] = [
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ];
    let shift: [f32; 16] = [
        1.0, 0.0, 0.0, 1.0, 0.0, 1.0, 0.0, -2.0, 0.0, 0.0, 1.0, 0.5, 0.0, 0.0, 0.0, 1.0,
    ];
    let mut tal = build(
        &mut g,
        &spec(MRI_UCHAR, [6, 7, 6, 1], [2.0, 2.0, 2.0], CORONAL, [0.0, -18.0, 18.0]),
    );
    put(&mut files, "fsh/average/mni305.cor.subfov2.mgz", &mut tal);
    let mut tal1 = build(
        &mut g,
        &spec(MRI_UCHAR, [8, 9, 8, 1], [1.0, 1.0, 1.0], CORONAL, [0.0, -18.0, 18.0]),
    );
    put(&mut files, "fsh/average/mni305.cor.subfov1.mgz", &mut tal1);
    files.push((
        "fsh/average/mni305.cor.subfov2.reg".into(),
        reg_dat("fsaverage", &shift, "round\n"),
    ));
    files.push((
        "fsh/average/mni305.cor.subfov1.reg".into(),
        reg_dat("fsaverage", &ident, "round\n"),
    ));
    files.push((
        "fsh/average/mni152.register.dat".into(),
        reg_dat("fsaverage", &shift, "round\n"),
    ));
    let mut mni = build(
        &mut g,
        &spec(MRI_FLOAT, [7, 8, 7, 1], [2.0, 2.0, 2.0], IDENT, [0.0, -17.0, 18.0]),
    );
    put(&mut files, "fsl/data/standard/MNI152_T1_2mm.nii.gz", &mut mni);
    Inputs {
        files,
        dirs: vec!["outdir".to_owned()],
    }
}

fn names(a: &str, f: &str) -> bool {
    let a = a.trim_end_matches('/');
    !a.is_empty() && (f == a || f.strip_prefix(a).is_some_and(|r| r.starts_with('/')))
}

/// The folders every case gets (its own FreeSurfer, subjects and FSL folders).
const ALWAYS: [&str; 3] = ["fsh/", "subjects/", "fsl/"];

/// The environment every case starts from.
const BASE_ENV: [&str; 2] = ["FREESURFER_HOME=@DIR@/fsh", "SUBJECTS_DIR=@DIR@/subjects"];

/// A case line: the inputs' files and folders the arguments name, and the case's own folders.
pub fn line(env: &[&str], i: &Inputs, args: &[String]) -> String {
    let files: Vec<&(String, Vec<u8>)> = i
        .files
        .iter()
        .filter(|(f, _)| args.iter().any(|a| names(a, f)) || ALWAYS.iter().any(|p| f.starts_with(p)))
        .collect();
    let dirs: Vec<&String> = i
        .dirs
        .iter()
        .filter(|d| args.iter().any(|a| names(a, d) || a.starts_with(&format!("{d}/"))))
        .collect();
    let mut all: Vec<&str> = BASE_ENV.to_vec();
    all.extend_from_slice(env);
    let mut t = vec!["v2v".to_owned(), hex(all.join(";").as_bytes()), files.len().to_string()];
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

/// The fixed cases: the command line, every check, every registration, every path.
fn prelude() -> Vec<String> {
    let i = base_inputs();
    let mut out = Vec::new();
    let mut add = |env: &[&str], a: &[&str]| out.push(line(env, &i, &s(a)));
    let base = ["--mov", "m.mgz", "--targ", "t.mgz", "--o", "o.mgz"];
    let with = |extra: &[&'static str]| -> Vec<&'static str> {
        let mut v = base.to_vec();
        v.extend_from_slice(extra);
        v
    };
    // the command line
    add(&[], &[]);
    add(&[], &["--help"]);
    add(&[], &["--version"]);
    add(&[], &["--version", "--mov", "m.mgz"]);
    add(&[], &["--mov", "m.mgz", "--version"]);
    add(&[], &with(&["--regheader", "--version"]));
    add(&[], &with(&["--regheader", "-version", "--nearest"]));
    add(&[], &["--foo"]);
    add(&[], &["-foo"]);
    add(&[], &["x"]);
    add(&[], &["--debug", "--mov", "m.mgz", "--targ", "t.mgz", "--regheader", "--o", "o.mgz"]);
    add(&["-FREESURFER_HOME"], &with(&["--regheader"]));
    add(&["-SUBJECTS_DIR"], &with(&["--regheader"]));
    for opt in [
        "--mov", "--targ", "--o", "--out", "--reg", "--lta", "--lta-inv", "--fsl", "--xfm",
        "--talxfm", "--disp", "--talres", "--crop", "--mul", "--slice-bias", "--interp",
        "--precision", "--seed", "--sd", "--ctab", "--vsm", "--vsm-reg", "--vsm-pedir",
        "--reg-final", "--s", "--subject", "--cost", "--m3z", "--fill-upsample",
        "--no-resample-scale", "--vg-thresh", "--gdiagno",
    ] {
        add(&[], &["--regheader", opt]);
    }
    add(&[], &["--soap", "m.mgz"]);
    add(&[], &["--downsample", "2", "2"]);
    add(&[], &["--slice-crop", "1"]);
    add(&[], &["--rot", "1", "2"]);
    add(&[], &["--trans", "1"]);
    add(&[], &["--shear"]);
    add(&[], &["--spm-warp", "a", "b", "c", "d"]);
    add(&[], &["--gcam", "a", "b", "c", "d", "e", "f"]);
    add(&[], &["--map-point", "1", "2", "3", "1", "r.lta", "2"]);
    // the checks
    add(&[], &["--targ", "t.mgz", "--regheader", "--o", "o.mgz"]);
    add(&[], &["--mov", "m.mgz", "--targ", "t.mgz", "--regheader"]);
    add(&[], &["--mov", "m.mgz", "--regheader", "--o", "o.mgz"]);
    add(&[], &with(&["--fstarg", "--reg", "r.dat"]));
    add(&[], &with(&["--tal", "--reg", "r.dat"]));
    add(&[], &["--mov", "m.mgz", "--tal", "--fstarg", "--reg", "r.dat", "--o", "o.mgz"]);
    add(&[], &with(&["--downsample", "2", "2", "2"]));
    add(&[], &["--mov", "m.mgz", "--targ", "t.mgz", "--regheader", "--o", "nodir/sub/o.mgz"]);
    add(&[], &["--mov", "m.mgz", "--targ", "t.mgz", "--regheader", "--o", "outdir/o.mgz"]);
    add(&[], &["--mov", "m.mgz", "--targ", "t.mgz", "--regheader", "--o", "newdir/o.mgz"]);
    add(&[], &["--mov", "m.mgz", "--tal", "--regheader", "--o", "o.mgz"]);
    add(&[], &with(&["--xfm", "x.xfm", "--reg", "r.dat"]));
    add(&[], &with(&["--regheader", "--reg", "r.dat"]));
    add(&[], &["--mov", "m.mgz", "--fstarg", "--o", "o.mgz"]);
    add(&[], &with(&["--regheader", "--interp", "spline"]));
    add(&[], &with(&["--regheader", "--precision", "double"]));
    add(&[], &with(&["--regheader", "--talres", "3"]));
    add(&[], &with(&["--regheader", "--mul", "x2"]));
    add(&[], &with(&["--crop", "2", "--regheader"]));
    add(&[], &["--mov", "m.mgz", "--o", "o.mgz"]);
    add(&[], &with(&[]));
    // the registrations
    add(&[], &with(&["--regheader"]));
    add(&[], &with(&["--xfm", "x.xfm"]));
    add(&[], &with(&["--xfm", "xbad.xfm"]));
    add(&[], &with(&["--xfm", "none.xfm"]));
    add(&[], &with(&["--reg", "r.dat"]));
    add(&[], &with(&["--reg", "rt.dat"]));
    add(&[], &with(&["--reg", "bad.dat"]));
    add(&[], &with(&["--reg", "none.dat"]));
    add(&[], &with(&["--reg", "r.lta"]));
    add(&[], &["--mov", "m.mgz", "--reg", "r.lta", "--o", "o.mgz"]);
    add(&[], &with(&["--lta", "r.lta"]));
    add(&[], &with(&["--lta", "v.lta"]));
    add(&[], &["--mov", "m.mgz", "--lta", "r.lta", "--o", "o.mgz"]);
    add(&[], &["--mov", "m.mgz", "--lta", "v.lta", "--o", "o.mgz", "--save-reg"]);
    add(&[], &with(&["--lta-inv", "rt.lta"]));
    add(&[], &["--mov", "m.mgz", "--lta-inv", "rt.lta", "--o", "o.mgz"]);
    add(&[], &with(&["--lta", "nos.lta"]));
    add(&[], &with(&["--lta", "nod.lta"]));
    add(&[], &with(&["--lta", "lta.txt"]));
    add(&[], &with(&["--lta", "identity.nofile"]));
    add(&[], &with(&["--lta", "none.lta"]));
    add(&[], &["--inv", "--lta", "r.lta", "--targ", "t.mgz", "--o", "o.mgz"]);
    add(&[], &["--inv", "--lta", "r.lta", "--keep-precision", "--o", "o.mgz"]);
    add(&[], &["--inv", "--lta", "r.lta", "--o", "o.mgz"]);
    add(&[], &["--inv", "--lta", "r.lta", "--mov", "m.mgz", "--targ", "t.mgz", "--o", "o.mgz"]);
    add(&[], &with(&["--fsl", "f.mat"]));
    add(&[], &with(&["--fsl", "f.mat", "f2.mat"]));
    add(&[], &with(&["--fslreg", "f.mat", "--nearest"]));
    add(&[], &with(&["--fsl", "fbad.mat"]));
    add(&[], &with(&["--fsl", "none.mat"]));
    add(&[], &with(&["--fsl", "f.mat", "fbad.mat"]));
    add(&[], &with(&["--s", "s1"]));
    add(&[], &with(&["--regheader", "--s", "s1"]));
    add(&[], &with(&["--noDefM3zPath"]));
    add(&[], &with(&["--reg", "r.dat", "--s", "s2"]));
    add(&[], &with(&["--s", "s2", "--reg", "r.dat"]));
    // the interpolations and the ways of resampling
    for interp in [
        &["--nearest"][..],
        &["--trilin"],
        &["--cubic"],
        &["--interp", "nearest"],
        &["--interp", "trilinear"],
        &["--interp", "tri"],
        &["--interp", "cubic"],
        &["--interp", "sinc"],
        &["--interp", "sinc", "3"],
        &["--interp", "sinc", "--nearest"],
        &["--neare"],
        &["--INTERP", "nearest"],
    ] {
        add(&[], &with(&[&["--regheader"][..], interp].concat()));
        add(&[], &with(&[&["--reg", "r.dat"][..], interp].concat()));
    }
    add(&[], &with(&["--regheader", "--inv"]));
    add(&[], &with(&["--reg", "r.dat", "--inv", "--cubic"]));
    add(&[], &["--mov", "m4.mgz", "--targ", "t2.nii.gz", "--regheader", "--o", "o.nii.gz"]);
    add(&[], &["--mov", "m4.mgz", "--targ", "t.mgz", "--regheader", "--o", "o.mgz", "--cubic"]);
    add(&[], &["--mov", "mu.mgz", "--targ", "t.mgz", "--regheader", "--o", "o.mgz", "--keep-precision"]);
    add(&[], &["--mov", "ms.nii.gz", "--targ", "t2.nii.gz", "--regheader", "--o", "o.nii", "--keep-precision"]);
    add(&[], &["--mov", "thin.mgz", "--targ", "t.mgz", "--regheader", "--o", "o.mgz"]);
    add(&[], &["--mov", "seg.mgz", "--targ", "t2.nii.gz", "--lta", "seg.lta", "--interp", "nearest", "--o", "seg.nii.gz"]);
    add(&[], &["--mov", "seg.mgz", "--targ", "t.mgz", "--regheader", "--nearest", "--o", "seg.mgz"]);
    add(&[], &["--mov", "seg.mgz", "--targ", "t.mgz", "--regheader", "--nearest", "--o", "o.mgz", "--ctab", "lut.txt"]);
    add(&[], &with(&["--regheader", "--ctab", "badlut.txt"]));
    add(&[], &with(&["--regheader", "--ctab", "lut.txt", "--copy-ctab"]));
    add(&[], &["--mov", "seg.mgz", "--targ", "t.mgz", "--regheader", "--nearest", "--o", "o.mgz", "--inv"]);
    for p in ["uchar", "short", "int", "long", "float", "UCHAR"] {
        add(&[], &with(&["--regheader", "--precision", p]));
    }
    add(&[], &with(&["--regheader", "--mr"]));
    add(&[], &with(&["--regheader", "--nomr"]));
    add(&[], &with(&["--regheader", "--kernel"]));
    add(&[], &with(&["--reg", "r.dat", "--kernel"]));
    add(&[], &with(&["--regheader", "--delta"]));
    add(&[], &with(&["--reg", "r.dat", "--delta", "--inv"]));
    add(&[], &with(&["--regheader", "--fill-average"]));
    add(&[], &with(&["--regheader", "--fill-conserve"]));
    add(&[], &with(&["--reg", "r.dat", "--fill-conserve", "--fill-upsample", "3"]));
    add(&[], &with(&["--regheader", "--fill-average", "--inv"]));
    add(&[], &["--mov", "m.mgz", "--downsample", "2", "2", "2", "--o", "o.mgz"]);
    add(&[], &["--mov", "m.mgz", "--downsample", "3", "1", "2", "--o", "o.mgz"]);
    add(&[], &["--mov", "m.mgz", "--downsample", "2", "0", "2", "--o", "o.mgz"]);
    add(&[], &["--mov", "m.mgz", "--downsample", "2", "2", "0", "--o", "o.mgz"]);
    add(&[], &["--mov", "m.mgz", "--downsample", "2", "-1", "2", "--o", "o.mgz"]);
    add(&[], &["--mov", "m.mgz", "--downsample", "0", "2", "2", "--o", "o.mgz"]);
    add(&[], &["--mov", "m.mgz", "--downsample", "-2", "2", "2", "--o", "o.mgz"]);
    add(&[], &["--mov", "m4.mgz", "--downsample", "2", "2", "2", "--o", "o.mgz", "--save-reg"]);
    add(&[], &with(&["--regheader", "--vsm", "vsm.mgz"]));
    add(&[], &with(&["--regheader", "--vsm", "vsm.mgz", "1"]));
    add(&[], &with(&["--regheader", "--vsm", "vsm.mgz", "-2"]));
    add(&[], &with(&["--regheader", "--vsm", "vsm.mgz", "3", "--cubic"]));
    add(&[], &with(&["--regheader", "--vsm", "vsm.mgz", "--vsm-pedir", "-1"]));
    add(&[], &with(&["--regheader", "--vsm", "vsm.mgz", "--vsm-pedir", "5"]));
    add(&[], &with(&["--regheader", "--vsm", "vsm.mgz", "--vsm-reg", "v.lta"]));
    add(&[], &with(&["--regheader", "--vsm", "vsm.mgz", "--vsm-reg", "r.lta"]));
    add(&[], &with(&["--regheader", "--vsm", "none.mgz"]));
    add(&[], &with(&["--regheader", "--vsm", "t.mgz"]));
    add(&[], &with(&["--regheader", "--new"]));
    add(&[], &with(&["--regheader", "--new", "--nearest"]));
    add(&[], &with(&["--regheader", "--vsm-reg", "none.lta"]));
    // the voxel-to-RAS rewrite
    add(&[], &with(&["--regheader", "--no-resample"]));
    add(&[], &with(&["--reg", "r.dat", "--no-resample"]));
    add(&[], &with(&["--reg", "r.dat", "--no-resample", "--inv"]));
    add(&[], &with(&["--reg", "r.dat", "--no-resample-scale", "1"]));
    add(&[], &with(&["--reg", "r.dat", "--no-resample", "--slice-crop", "2", "6"]));
    add(&[], &with(&["--reg", "r.dat", "--no-resample", "--slice-crop", "2", "12"]));
    add(&[], &with(&["--reg", "r.dat", "--no-resample", "--slice-reverse"]));
    add(&[], &with(&["--reg", "r.dat", "--no-resample", "--slice-bias", "0.5"]));
    add(&[], &with(&["--reg", "r.dat", "--no-resample", "--slice-bias", "2", "--slice-reverse", "--slice-crop", "1", "8"]));
    add(&[], &["--mov", "m4.mgz", "--targ", "t.mgz", "--regheader", "--o", "o.mgz", "--no-resample", "--slice-reverse"]);
    // after the resampling
    add(&[], &with(&["--regheader", "--slice-crop", "1", "2"]));
    add(&[], &with(&["--regheader", "--slice-crop", "1", "9"]));
    add(&[], &with(&["--regheader", "--slice-crop", "3", "1"]));
    add(&[], &with(&["--regheader", "--mul", "2.5"]));
    add(&[], &with(&["--regheader", "--mul", "-0.5", "--precision", "short"]));
    add(&[], &with(&["--regheader", "--mul", ".25e1"]));
    add(&[], &with(&["--regheader", "--save-reg"]));
    add(&[], &with(&["--reg", "r.dat", "--save-reg"]));
    add(&[], &with(&["--reg", "r.dat", "--save-reg", "--no-save-reg"]));
    add(&[], &["--mov", "m.mgz", "--targ", "t.mgz", "--regheader", "--o", "outdir/o.nii.gz", "--save-reg"]);
    add(&[], &with(&["--reg", "r.dat", "--reg-final", "final.dat"]));
    add(&[], &with(&["--regheader", "--reg-final", "final.dat", "--inv"]));
    add(&[], &with(&["--regheader", "--reg-final", "nodir/final.dat"]));
    add(&[], &with(&["--regheader", "--rot", "10", "-5", "3"]));
    add(&[], &with(&["--regheader", "--trans", "1.5", "-2", "0.25"]));
    add(&[], &with(&["--regheader", "--shear", "0.1", "0", "-0.2"]));
    add(&[], &with(&["--reg", "r.dat", "--rot", "1", "2", "3", "--trans", "4", "5", "6", "--shear", "0.01", "0.02", "0.03", "--reg-final", "final.dat"]));
    add(&[], &with(&["--regheader", "--rot", "a", "b", "c"]));
    add(&[], &with(&["--regheader", "--seed", "5"]));
    add(&[], &with(&["--regheader", "--seed", "7", "--synth", "--nearest"]));
    add(&[], &["--mov", "m4.mgz", "--targ", "t.mgz", "--regheader", "--o", "o.mgz", "--seed", "3"]);
    add(&[], &["--mov", "mu.mgz", "--targ", "t.mgz", "--regheader", "--o", "o.mgz", "--seed", "3"]);
    add(&[], &with(&["--regheader", "--vg-thresh", "0.5"]));
    add(&[], &with(&["--regheader", "--gdiagno", "0"]));
    add(&[], &with(&["--regheader", "--gdiagno", "-1"]));
    add(&[], &with(&["--regheader", "--filter"]));
    add(&[], &with(&["--regheader", "--subject", "s9", "--save-reg"]));
    add(&[], &with(&["--regheader", "--sd", "nowhere"]));
    add(&[], &with(&["--regheader", "--SD", "nowhere"]));
    add(&[], &with(&["--regheader", "--ctabx", "lut.txt"]));
    add(&[], &with(&["--regheader", "--cropx", "2"]));
    // the subject's target, Talairach, MNI152
    add(&[], &["--mov", "m.mgz", "--reg", "r.dat", "--fstarg", "--o", "o.mgz"]);
    add(&[], &["--mov", "m.mgz", "--reg", "r.dat", "--fstarg", "brain.mgz", "--o", "o.mgz"]);
    add(&[], &["--mov", "m.mgz", "--reg", "r.dat", "--fstarg", "nothere.mgz", "--o", "o.mgz"]);
    add(&[], &["--mov", "m.mgz", "--s", "s1", "--fstarg", "--o", "o.mgz", "--save-reg"]);
    add(&[], &["--mov", "m.mgz", "--reg", "r.dat", "--fstarg", "--o", "o.mgz", "--inv", "--save-reg"]);
    add(&[], &["--mov", "m.mgz", "--reg", "r.dat", "--fstarg", "--o", "o.mgz", "--no-resample"]);
    add(&[], &["--mov", "m.mgz", "--reg", "r.dat", "--tal", "--o", "o.mgz"]);
    add(&[], &["--mov", "m.mgz", "--reg", "r.dat", "--tal", "--talres", "1", "--o", "o.mgz", "--save-reg"]);
    add(&[], &["--mov", "m.mgz", "--reg", "r.dat", "--tal", "--o", "o.mgz", "--save-reg"]);
    add(&[], &["--mov", "m.mgz", "--reg", "r.dat", "--talxfm", "talairach.xfm", "--o", "o.mgz"]);
    add(&[], &["--mov", "m.mgz", "--reg", "r.dat", "--talxfm", "none.xfm", "--o", "o.mgz"]);
    add(&[], &["--mov", "m.mgz", "--s", "s1", "--tal", "--o", "o.mgz"]);
    add(&[], &["--mov", "m.mgz", "--tal", "--o", "o.mgz"]);
    add(&[], &["--mov", "m.mgz", "--reg", "r.dat", "--tal", "--inv", "--o", "o.mgz"]);
    add(&[], &["--mov", "m.mgz", "--reg", "r.lta", "--tal", "--o", "o.mgz"]);
    add(&["FSLDIR=@DIR@/fsl"], &["--mov", "m.mgz", "--mni152reg", "--o", "o.mgz"]);
    add(&["FSLDIR=@DIR@/fsl"], &["--mov", "m.mgz", "--mni152reg", "--reg", "r.dat", "--o", "o.mgz"]);
    add(&["FSLDIR=@DIR@/fsl"], &["--mov", "m.mgz", "--mni152reg", "--reg", "r.lta", "--o", "o.mgz"]);
    add(&[], &["--mov", "m.mgz", "--mni152reg", "--o", "o.mgz"]);
    add(&["FSLDIR=@DIR@/fsl"], &["--mov", "m.mgz", "--mni152reg", "--s", "s2", "--reg", "r.dat", "--o", "o.mgz"]);
    // --crop
    add(&[], &["--mov", "box.mgz", "--crop", "2", "--lta", "r.lta", "--o", "c.mgz"]);
    add(&[], &["--mov", "box.mgz", "--crop", "1", "--downsample", "2", "2", "2", "--o", "c.mgz"]);
    add(&[], &["--mov", "box.mgz", "--crop", "0.5", "--o", "c.mgz"]);
    add(&[], &["--mov", "box.mgz", "--crop", "2", "--o", "c.nii.gz"]);
    add(&[], &["--mov", "m.mgz", "--crop", "2", "--lta", "r.lta", "--o", "c.mgz"]);
    add(&[], &["--mov", "zero.mgz", "--crop", "2", "--lta", "r.lta", "--o", "c.mgz"]);
    add(&[], &["--mov", "box.mgz", "--crop", "0", "--lta", "r.lta", "--o", "c.mgz"]);
    add(&[], &["--mov", "box.mgz", "--crop", "0.05", "--lta", "r.lta", "--o", "c.mgz"]);
    add(&[], &["--mov", "box.mgz", "--crop", "-1", "--lta", "r.lta", "--o", "c.mgz"]);
    add(&[], &["--mov", "box.mgz", "--cropscale", "3", "--lta", "r.lta", "--o", "c.mgz"]);
    add(&[], &["--mov", "seg.mgz", "--crop", "1.5", "--lta", "r.lta", "--o", "c.mgz"]);
    // --soap
    add(&[], &with(&["--regheader", "--soap", "box.mgz", "2"]));
    add(&[], &["--mov", "box.mgz", "--targ", "t.mgz", "--regheader", "--o", "o.mgz", "--soap", "box.mgz", "0"]);
    add(&[], &with(&["--regheader", "--soap", "none.mgz", "2"]));
    // paths not ported yet (relay 53)
    add(&[], &with(&["--regheader", "--disp", "d.mgz"]));
    add(&[], &with(&["--regheader", "--cost", "cost.txt"]));
    add(&[], &["--mov", "m.mgz", "--s", "s1", "--morph", "--o", "o.mgz"]);
    add(&[], &["--mov", "m.mgz", "--s", "s1", "--inv-morph", "--o", "o.mgz", "--targ", "t.mgz"]);
    add(&[], &with(&["--regheader", "--m3z", "x.m3z"]));
    add(&[], &["--gcam", "m.mgz", "0", "0", "r.lta", "0", "1", "o.mgz"]);
    add(&[], &["--spm-warp", "m.mgz", "0", "w.nii", "1", "o.mgz"]);
    add(&[], &["--map-point", "1", "2", "3", "1", "r.lta", "2", "nofile"]);
    out
}

// ------------------------------------------------------------------ the seeded cases

const REGS: &[&[&str]] = &[
    &["--regheader"],
    &["--reg", "r.dat"],
    &["--reg", "r.lta"],
    &["--lta", "r.lta"],
    &["--lta", "v.lta"],
    &["--lta-inv", "rt.lta"],
    &["--fsl", "f.mat"],
    &["--xfm", "x.xfm"],
    &["--s", "s1"],
];

const MOVS: &[&str] = &["m.mgz", "m.nii.gz", "m4.mgz", "mu.mgz", "ms.nii.gz", "seg.mgz", "thin.mgz", "box.mgz"];
const TARGS: &[&str] = &["t.mgz", "t2.nii.gz", "m.mgz", "mu.mgz"];
const OUTS: &[&str] = &["o.mgz", "o.nii.gz", "o.nii", "o.mgh", "outdir/o.mgz"];

fn seeded(g: &mut Gen, i: &Inputs) -> String {
    let mut groups: Vec<Vec<String>> = Vec::new();
    groups.push(s(&["--mov", g.pick(MOVS)]));
    let reg = g.pick(REGS);
    let lta_dst = reg[0].starts_with("--lta") || (reg[0] == "--reg" && reg[1].ends_with(".lta"));
    if !lta_dst || g.chance(2) {
        groups.push(s(&["--targ", g.pick(TARGS)]));
    }
    groups.push(s(reg));
    groups.push(s(&["--o", g.pick(OUTS)]));
    if g.chance(6) {
        groups.push(s(&["--inv"]));
    }
    let interps: &[&[&str]] = &[
        &["--nearest"],
        &["--trilin"],
        &["--cubic"],
        &["--interp", "sinc", "2"],
        &["--interp", "nearest"],
    ];
    if g.chance(2) {
        groups.push(s(g.pick(interps)));
    }
    let paths: &[&[&str]] = &[
        &["--no-resample"],
        &["--kernel"],
        &["--delta"],
        &["--fill-average"],
        &["--fill-conserve", "--fill-upsample", "3"],
        &["--new"],
    ];
    if g.chance(4) {
        groups.push(s(g.pick(paths)));
    }
    let mods: &[&[&str]] = &[
        &["--precision", "uchar"],
        &["--precision", "short"],
        &["--precision", "int"],
        &["--keep-precision"],
        &["--mul", "1.5"],
        &["--slice-crop", "0", "2"],
        &["--slice-reverse"],
        &["--slice-bias", "0.75"],
        &["--save-reg"],
        &["--mr"],
        &["--rot", "3", "-2", "1"],
        &["--trans", "0.5", "1", "-1"],
        &["--shear", "0.05", "0", "0.02"],
        &["--reg-final", "final.dat"],
        &["--ctab", "lut.txt"],
        &["--seed", "11"],
        &["--subject", "sx"],
    ];
    for _ in 0..g.below(4) {
        groups.push(s(g.pick(mods)));
    }
    if g.chance(3) {
        for k in (1..groups.len()).rev() {
            let j = g.below(k as u64 + 1) as usize;
            groups.swap(k, j);
        }
    }
    let mut args: Vec<String> = groups.into_iter().flatten().collect();
    if g.chance(8) {
        // another spelling: upper or mixed case (most options are compared in any case)
        let k = g.below(args.len() as u64) as usize;
        if args[k].starts_with("--") {
            args[k] = args[k].to_ascii_uppercase();
        }
    }
    line(&[], i, &args)
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

/// The golden cases (`tests/mri_vol2vol_golden.rs`): the v8 scripts' four forms, fMRIPrep's, the
/// voxel-to-RAS rewrite and a NaN in the moving volume.
pub fn golden_cases() -> Vec<String> {
    let mut i = base_inputs();
    // a moving volume with NaN voxels (FreeSurfer's resampling of them)
    {
        let mut g = Gen::new(0x9a9);
        let sm = spec(MRI_FLOAT, [8, 9, 10, 1], [1.0, 1.5, 2.0], CORONAL, [1.0, -2.0, 3.0]);
        let mut m = build(&mut g, &sm);
        let mut fs = Globals::new(UpstreamDefects::Fix);
        let mut e = Vec::new();
        for (x, y, z) in [(3, 4, 5), (4, 4, 5), (0, 0, 0), (7, 8, 9)] {
            mri_set_vox_val(&mut fs, &mut e, &mut m, x, y, z, 0, f32::NAN, 0).expect("a voxel");
        }
        i.files.extend(write_as(&mut m, "nan.mgz"));
    }
    let mut out = Vec::new();
    for a in [
        &["--regheader", "--mov", "m.mgz", "--targ", "t2.nii.gz", "--o", "o.nii.gz"][..],
        &["--reg", "r.lta", "--mov", "m.mgz", "--o", "o.nii.gz"],
        &["--regheader", "--interp", "nearest", "--mov", "seg.mgz", "--targ", "t.mgz", "--o", "o.mgz"],
        &["--mov", "m.mgz", "--lta", "r.lta", "--targ", "t.mgz", "--o", "o.mgz"],
        &["--mov", "seg.mgz", "--targ", "t2.nii.gz", "--lta", "seg.lta", "--interp", "nearest", "--o", "seg.nii.gz"],
        &["--mov", "m.mgz", "--targ", "t.mgz", "--reg", "r.dat", "--o", "o.mgz", "--no-resample", "--save-reg"],
        &["--mov", "nan.mgz", "--targ", "t.mgz", "--regheader", "--o", "o.mgz"],
        &["--mov", "nan.mgz", "--targ", "t2.nii.gz", "--reg", "r.dat", "--o", "o.mgz", "--cubic"],
    ] {
        out.push(line(&[], &i, &s(a)));
    }
    out
}

// ------------------------------------------------------------------ the port's answer

/// Text files listed with their bytes: the transforms' formats.
fn is_text(name: &str) -> bool {
    [".lta", ".mat", ".xfm", ".dat", ".reg", ".txt"]
        .iter()
        .any(|e| name.ends_with(e))
}

/// `b` with every `# created by` line (user and date) masked.
pub fn mask_created(b: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(b.len());
    for (k, l) in b.split(|&c| c == b'\n').enumerate() {
        if k > 0 {
            out.push(b'\n');
        }
        if l.starts_with(b"# created by") {
            out.extend_from_slice(b"# created by <who>");
        } else {
            out.extend_from_slice(l);
        }
    }
    out
}

/// Every file of a case's folder but its inputs left as they were, sorted, each as `F <name>
/// <fnv> <size>` (text files with their bytes escaped, the `# created by` line masked), the
/// folder's name in them as `<DIR>`; each folder the run made as `D <name>`.
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
                if is_text(&n) {
                    out.push(format!("F {n} {}", escape(&mask_created(&b))));
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
    assert_eq!(t[0], "v2v");
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
        files.push((t[i].to_owned(), unhex(t[i + 1])));
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

/// Standard output with the noise seed (unless `--seed` is given) and `VmPeak` masked.
pub fn mask_stdout(text: &str, args: &[String]) -> String {
    let seeded = args.iter().any(|a| a.eq_ignore_ascii_case("--seed"));
    let mut out = Vec::new();
    for l in text.split('\n') {
        if !seeded && l.starts_with("SynthSeed  ") {
            out.push("SynthSeed  <seed>".to_owned());
        } else if let Some(i) = l.find("VmPeak ") {
            out.push(format!("{}VmPeak <kB>", &l[..i]));
        } else {
            out.push(l.to_owned());
        }
    }
    out.join("\n")
}

/// The port's line for a case line and the defects it noted (`dir`: an empty folder), with the
/// pseudo-notices `CRASH` and `NOTPORTED`.
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
    let stdout = mask_stdout(&String::from_utf8_lossy(&out), &c.args);
    let text = format!(
        "EXIT {status} | {} | {} | {files}",
        mask(stdout.as_bytes()),
        mask(&err)
    );
    let mut ids: Vec<&'static str> = g.notices.as_slice().iter().map(|n| n.defect.id).collect();
    if matches!(status, 134 | 136 | 139) {
        ids.push("CRASH");
    }
    if String::from_utf8_lossy(&err).contains("is not ported yet") {
        ids.push("NOTPORTED");
    }
    (text, ids)
}

/// [`run_line_in`] in a fresh temporary folder.
pub fn run_line(line: &str, mode: UpstreamDefects) -> (String, Vec<&'static str>) {
    let dir = tempfile::tempdir().expect("a temporary folder");
    run_line_in(line, mode, dir.path())
}
