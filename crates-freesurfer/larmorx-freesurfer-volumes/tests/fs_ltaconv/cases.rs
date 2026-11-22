// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The fs-ltaconv probe's cases (`validation/probes/fs-ltaconv/`): a seeded generator of
//! `lta_convert` command lines with the files they read, and the port's answer to each, printed as
//! `check.py` prints the official program's (`<status> | <stdout> | <stderr> | <files>`,
//! C-escaped).
//!
//! A case line is `lta <env> <nfiles> [<name> <hex>]... <ndirs> [<hex>]... <nargs> [<hex>]...`, as
//! the fs-add-xform probe's: `<env>` is `-` or the hex of `NAME=value` assignments (separated by
//! `;`) added to the probe's environment; the files are written into the case's folder, then the
//! (empty) folders made; the arguments follow the program's name. Each case writes only the files
//! its arguments name. The port's answer adds the pseudo-notices `CRASH` (a status FreeSurfer's
//! build reaches by a signal: 134, 136, 139) and `MEMORY` (FS-1163's memory contents), which
//! `check.py` counts, not compares.
#![allow(dead_code)]

#[path = "../fs_info/cases.rs"]
mod info;

use std::io::Write;
use std::path::Path;

use larmorx_core::defects::UpstreamDefects;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::TrackedErr;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_io::mriio::mri_write;
use larmorx_freesurfer_mri::mri::{MGZ_INTENT_UNKNOWN, MRI_FLOAT, MRI_SHORT, MRI_UCHAR, Mri};
use larmorx_freesurfer_volumes::lta_convert::main::{Run, main};

pub use info::{Gen, escape, hex, probe_env, unhex};
use info::{Spec, build, collect, fnv, mask_bytes};

// ------------------------------------------------------------------ the files

fn spec(ty: i32, dims: [i32; 3], size: [f32; 3], mdc: [f32; 9], c: [f32; 3]) -> Spec {
    Spec {
        dims: [dims[0], dims[1], dims[2], 1],
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

/// The files of `m` written as `name` (`mri_write`, the format its name gives).
fn write_as(m: &mut Mri, name: &str) -> Vec<(String, Vec<u8>)> {
    let t = tempfile::tempdir().expect("a temporary folder");
    let mut fs = Globals::new(UpstreamDefects::Fix);
    let (mut o, mut e) = (Vec::new(), Vec::new());
    let env = Environment::default();
    let _ = mri_write(
        &mut fs,
        &mut o,
        &mut e,
        &env,
        Some(t.path()),
        m,
        name,
        None,
        MGZ_INTENT_UNKNOWN,
    );
    collect(t.path())
}

const IDENT: [f32; 9] = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];
const CORONAL: [f32; 9] = [-1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 1.0, 0.0];
/// 30 degrees about the superior axis.
const OBLIQUE: [f32; 9] = [0.866_025_4, 0.5, 0.0, -0.5, 0.866_025_4, 0.0, 0.0, 0.0, 1.0];
/// 45 degrees: two axes share their main direction (FS-1167). The number as written (its `float`,
/// which the recorded cases' volumes hold), not `FRAC_1_SQRT_2`'s.
#[allow(clippy::approx_constant)]
const DIAG: [f32; 9] = [
    0.707_106_8,
    0.707_106_8,
    0.0,
    -0.707_106_8,
    0.707_106_8,
    0.0,
    0.0,
    0.0,
    1.0,
];

/// An `.lta` text (`LTAprint`'s form) of `ty` with `rows` and both geometries.
fn lta_text(ty: i32, rows: &str, valid: i32) -> Vec<u8> {
    let vg = |name: &str, dims: &str, size: &str, x: &str, y: &str, z: &str, c: &str| {
        format!(
            "valid = {valid}  # volume info valid\nfilename = {name}\nvolume = {dims}\nvoxelsize = {size}\nxras   = {x}\nyras   = {y}\nzras   = {z}\ncras   = {c}\n"
        )
    };
    format!(
        "# transform file x.lta\n# created by probe on Sun Nov 22 00:00:00 2026\n\ntype      = {ty} # LINEAR_RAS_TO_RAS\nnxforms   = 1\nmean      = 0.0000 0.0000 0.0000\nsigma     = 10000.0000\n1 4 4\n{rows}src volume info\n{}dst volume info\n{}subject probe\nfscale 0.100000\n",
        vg("s.mgz", "4 5 6", "1 1.5 2", "1 0 0", "0 1 0", "0 0 1", "-3 4 5"),
        vg("t.mgz", "6 4 5", "1 1.2 1", "-1 0 0", "0 0 -1", "0 1 0", "1 -2 3"),
    )
    .into_bytes()
}

const ROT: &str =
    "9.8e-01 -1.7e-01 0 2.5e+00\n1.7e-01 9.8e-01 0 -1.25e+00\n0 0 1 3.0e+00\n0 0 0 1\n";

fn itk(transform: &str, params: &str, fixed: Option<&str>, end: &str) -> Vec<u8> {
    let mut s = format!(
        "#Insight Transform File V1.0\n#Transform 0\nTransform: {transform}\nParameters: {params}"
    );
    if let Some(f) = fixed {
        s.push_str(&format!("\nFixedParameters: {f}"));
    }
    s.push_str(end);
    s.into_bytes()
}

/// The inputs a case may name.
pub struct Inputs {
    files: Vec<(String, Vec<u8>)>,
    dirs: Vec<String>,
}

fn base_inputs() -> Inputs {
    let mut g = Gen::new(0x17a_c0de);
    let mut files = Vec::new();
    let vol = |g: &mut Gen, name: &str, s: Spec, files: &mut Vec<(String, Vec<u8>)>| {
        let mut m = build(g, &s);
        files.extend(write_as(&mut m, name));
    };
    vol(
        &mut g,
        "s.mgz",
        spec(
            MRI_FLOAT,
            [4, 5, 6],
            [1.0, 1.5, 2.0],
            IDENT,
            [-3.0, 4.0, 5.0],
        ),
        &mut files,
    );
    vol(
        &mut g,
        "t.mgz",
        spec(
            MRI_UCHAR,
            [6, 4, 5],
            [1.0, 1.2, 1.0],
            CORONAL,
            [1.0, -2.0, 3.0],
        ),
        &mut files,
    );
    vol(
        &mut g,
        "obl.mgz",
        spec(
            MRI_SHORT,
            [5, 4, 3],
            [1.1, 0.9, 1.3],
            OBLIQUE,
            [0.5, 1.5, -2.5],
        ),
        &mut files,
    );
    vol(
        &mut g,
        "d45.mgz",
        spec(MRI_UCHAR, [4, 4, 4], [1.0, 1.0, 1.0], DIAG, [0.0, 0.0, 0.0]),
        &mut files,
    );
    let mut bad = spec(
        MRI_UCHAR,
        [4, 4, 4],
        [1.0, 1.0, 1.0],
        CORONAL,
        [0.0, 0.0, 0.0],
    );
    bad.good = 0;
    vol(&mut g, "bad.mgz", bad, &mut files);
    vol(
        &mut g,
        "flat.mgz",
        spec(
            MRI_FLOAT,
            [1, 5, 6],
            [1.0, 1.0, 1.0],
            IDENT,
            [0.0, 0.0, 0.0],
        ),
        &mut files,
    );
    vol(
        &mut g,
        "flat2.mgz",
        spec(
            MRI_FLOAT,
            [1, 4, 3],
            [2.0, 1.0, 1.0],
            IDENT,
            [1.0, 0.0, 0.0],
        ),
        &mut files,
    );
    vol(
        &mut g,
        "flatz.mgz",
        spec(
            MRI_FLOAT,
            [5, 6, 1],
            [1.0, 1.0, 1.0],
            IDENT,
            [0.0, 0.0, 0.0],
        ),
        &mut files,
    );
    vol(
        &mut g,
        "s.nii",
        spec(
            MRI_FLOAT,
            [4, 5, 6],
            [1.0, 1.5, 2.0],
            IDENT,
            [-3.0, 4.0, 5.0],
        ),
        &mut files,
    );
    vol(
        &mut g,
        "t.nii",
        spec(
            MRI_FLOAT,
            [6, 4, 5],
            [1.0, 1.2, 1.0],
            CORONAL,
            [1.0, -2.0, 3.0],
        ),
        &mut files,
    );
    vol(
        &mut g,
        "z0.mgz",
        spec(
            MRI_FLOAT,
            [4, 4, 4],
            [1.0, 0.0, 1.0],
            IDENT,
            [0.0, 0.0, 0.0],
        ),
        &mut files,
    );
    vol(
        &mut g,
        "big.mgz",
        spec(
            MRI_UCHAR,
            [30, 20, 24],
            [1.2, 1.2, 1.2],
            CORONAL,
            [2.0, 3.0, -4.0],
        ),
        &mut files,
    );
    let s_mgz = files[0].1.clone();
    for name in ["noext", "a.b/c", "trail.", "U.MGZ"] {
        files.push((name.to_owned(), s_mgz.clone()));
    }
    files.push(("c.mgz".to_owned(), b"\x1f\x8b\x08\x00not a volume".to_vec()));
    let texts: Vec<(&str, Vec<u8>)> = vec![
        ("r.lta", lta_text(1, ROT, 1)),
        ("v.lta", lta_text(0, "1 0 0 1\n0 1 0 -1\n0 0 1 0.5\n0 0 0 1\n", 1)),
        ("nogeom.lta", lta_text(1, ROT, 0)),
        ("tk.lta", lta_text(14, "0 -1 0 1\n0 0 1 2\n-1 0 0 3\n0 0 0 1\n", 1)),
        ("bad.lta", b"type = 1\nnxforms = 1\nnot a matrix\n".to_vec()),
        ("fsl.mat", b"1 0 0 2\n0 1 0 3\n0 0 1 4\n0 0 0 1\n".to_vec()),
        ("fslrot.mat", b"0.98 -0.17 0 1.5\n0.17 0.98 0 -2\n0 0 1 0.25\n0 0 0 1\n".to_vec()),
        ("fslbad.mat", b"1 2\n".to_vec()),
        ("t.xfm", info::XFM.to_vec()),
        ("inv.xfm", b"MNI Transform File\nTransform_Type = Linear;\nInvert_Flag = True;\nLinear_Transform =\n 1.05 0.01 -0.02 -1.5\n -0.01 1.1 0.03 2.25\n 0.02 -0.03 0.95 -3.0;\n".to_vec()),
        ("bad.xfm", b"MNI Transform Fil\nTransform_Type = Linear;\n".to_vec()),
        ("r.dat", b"probe\n1.0\n2.0\n0.15\n1 0 0 1\n0 1 0 2\n0 0 1 3\n0 0 0 1\nround\n".to_vec()),
        ("bad.dat", b"probe\n1.0\n".to_vec()),
        ("nr.txt", b"0.98 -0.17 0 2.5\n0.17 0.98 0 -1.25\n0 0 1 3\n0 0 0 1\n".to_vec()),
        ("nr2d.txt", b"0.9 0.1 3\n-0.1 0.9 -2\n".to_vec()),
        ("nrsing.txt", b"1 2 3 0\n2 4 6 0\n0 0 0 0\n0 0 0 1\n".to_vec()),
        ("nr3.txt", b"1 0 0 2\n0 1 0 3\n0 0 1 4\n".to_vec()),
        ("nrx.txt", b"1 0 0 2\n0 1 x 3\n0 0 1 4\n0 0 0 1\n".to_vec()),
        ("empty.txt", b"".to_vec()),
        ("itk_r.txt", itk("AffineTransform_double_3_3", "0.9 0.1 0 -0.1 0.9 0 0 0 1 1 2 3", Some("10 20 30"), "\n")),
        ("itk_rn.txt", itk("AffineTransform_double_3_3", "0.9 0.1 0 -0.1 0.9 0 0 0 1 1 2 3", Some("10 20 30"), "")),
        ("itk_c.txt", itk("AffineTransform_double_3_3", "1 0 0 0 1 0 0 0 1 1 2 3", Some("10 20 30"), "\n")),
        ("itk_mo.txt", itk("MatrixOffsetTransformBase_double_3_3", "1.1 0 0.05 0 0.95 0 -0.05 0 1 -1 0.5 2", Some("0 0 0"), "\n")),
        ("itk_p.txt", itk("AffineTransform_double_3_3", "0.9 0.1 0 -0.1 0.9 0 0 0 1 1 2 3", None, "\n")),
        ("itk_2d.txt", itk("AffineTransform_double_2_2", "0.9 0.1 -0.1 0.9 1.5 -2", Some("0 0"), "\n")),
        ("itk_2dfp.txt", itk("AffineTransform_double_2_2", "0.9 0.1 -0.1 0.9 1.5 -2", Some("1 0"), "\n")),
        ("itk_eu.txt", itk("Euler3DTransform_double_3_3", "0 0 0 1 2 3", Some("0 0 0"), "\n")),
        ("itk_word.txt", b"#Insight Transform File V1.0\nTransform: AffineTransform_double_3_3\nOffset: 1 2 3\n".to_vec()),
        ("itk_tl.txt", b"#Insight Transform File V1.0\nParameters: 1 0 0 0 1 0 0 0 1 1 2 3\nTransform: AffineTransform_double_3_3\n".to_vec()),
        ("itk_cl.txt", itk("AffineTransform_double_3_3", "1 0 0 0 1 0 0 0 1 1 2 3", Some("0 0 0"), "\n# end\n")),
        ("itk_short.txt", b"Transform: AffineTransform_double_3_3\nParameters: 1 0 0 0 1 0\n".to_vec()),
        ("itk_hang.txt", itk("AffineTransform_double_3_3", "1 0 x 0 1 0 0 0 1 1 2 3", Some("0 0 0"), "\n")),
        ("vox.txt", b"1 0 0 1\n0 1 0 -0.5\n0 0 1 2\n0 0 0 1\n".to_vec()),
        ("voxsing.txt", b"0 0 0 0\n0 0 0 0\n0 0 0 0\n0 0 0 1\n".to_vec()),
    ];
    for (n, b) in texts {
        files.push((n.to_owned(), b));
    }
    Inputs {
        files,
        dirs: vec!["outdir".to_owned()],
    }
}

/// Whether the argument `a` names the file or folder `f` (itself, or a file in it).
fn names(a: &str, f: &str) -> bool {
    let a = a.trim_end_matches('/');
    !a.is_empty() && (f == a || f.strip_prefix(a).is_some_and(|r| r.starts_with('/')))
}

/// A case line: the inputs' files and folders the arguments name.
pub fn line(env: &[&str], i: &Inputs, args: &[String]) -> String {
    let files: Vec<&(String, Vec<u8>)> = i
        .files
        .iter()
        .filter(|(f, _)| args.iter().any(|a| names(a, f)))
        .collect();
    let dirs: Vec<&String> = i
        .dirs
        .iter()
        .filter(|d| args.iter().any(|a| names(a, d)))
        .collect();
    let mut t = vec![
        "lta".to_owned(),
        if env.is_empty() {
            "-".to_owned()
        } else {
            hex(env.join(";").as_bytes())
        },
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

/// FSL's output type, which the FSL conversions need.
const FSL: &str = "FSLOUTPUTTYPE=NIFTI_GZ";

/// The input options and the names each reads.
const INPUTS: &[(&str, &[&str])] = &[
    (
        "--inlta",
        &[
            "identity.nofile",
            "r.lta",
            "v.lta",
            "nogeom.lta",
            "tk.lta",
            "bad.lta",
            "gone.lta",
            "sub/identity.nofile",
        ],
    ),
    (
        "--infsl",
        &[
            "fsl.mat",
            "fslrot.mat",
            "fslbad.mat",
            "gone.mat",
            "identity.nofile",
        ],
    ),
    ("--inmni", &["t.xfm", "inv.xfm", "bad.xfm", "gone.xfm"]),
    ("--inxfm", &["t.xfm", "inv.xfm"]),
    ("--inreg", &["r.dat", "bad.dat", "gone.dat"]),
    (
        "--inniftyreg",
        &["nr.txt", "nrsing.txt", "nr3.txt", "gone.txt"],
    ),
    ("--inras", &["nr.txt"]),
    ("--inniftyreg2d", &["nr2d.txt", "nr.txt"]),
    (
        "--initk",
        &[
            "itk_r.txt",
            "itk_rn.txt",
            "itk_c.txt",
            "itk_mo.txt",
            "itk_p.txt",
            "itk_2d.txt",
            "itk_eu.txt",
            "itk_word.txt",
            "itk_tl.txt",
            "itk_cl.txt",
            "itk_short.txt",
            "gone.txt",
        ],
    ),
    ("--inlps", &["itk_r.txt"]),
    ("--invox", &["vox.txt", "voxsing.txt", "gone.txt"]),
];

const VOLUMES: &[&str] = &[
    "s.mgz", "s.mgz", "t.mgz", "t.mgz", "obl.mgz", "s.nii", "t.nii", "big.mgz", "d45.mgz",
    "bad.mgz", "z0.mgz", "gone.mgz", "c.mgz", "noext", "a.b/c", "U.MGZ",
];

const OUTPUTS: &[(&str, &[&str])] = &[
    ("--outlta", &["o.lta", "o.lta", "nodir/o.lta", "outdir"]),
    ("--outfsl", &["o.mat", "nodir/o.mat"]),
    ("--outmni", &["o.xfm"]),
    ("--outreg", &["o.dat", "o.reg"]),
    ("--outniftyreg", &["o.nr.txt"]),
    ("--outras", &["o.ras.txt"]),
    ("--outitk", &["o.itk.txt", "nodir/o.itk.txt"]),
    ("--outlps", &["o.lps.txt"]),
    ("--outvox", &["o.vox.txt"]),
];

/// The fixed cases: recon-all's, bbregister's and fMRIPrep's calls, each option alone, each input,
/// the command line's oddities.
fn prelude() -> Vec<String> {
    let i = base_inputs();
    let mut out = Vec::new();
    let st = ["--src", "s.mgz", "--trg", "t.mgz"];
    let with = |v: &[&str]| {
        let mut a = s(v);
        a.extend(s(&st));
        a
    };
    for args in [
        s(&[]),
        s(&["--version"]),
        s(&["--help"]),
        s(&["--HELP"]),
        s(&["-help"]),
        s(&["--inlta"]),
        s(&["--inlta", "identity.nofile", "--src"]),
        s(&["--inlta", "identity.nofile", "--srcupsample"]),
        s(&["--inlta", "identity.nofile", "--trgdownsample"]),
        s(&["--inlta", "identity.nofile", "--srcconform-size"]),
        s(&["--inlta", "identity.nofile", "--foo-bar"]),
        s(&["--inlta", "identity.nofile", "-"]),
        s(&["--inlta", "identity.nofile", "--"]),
        s(&["--inlta", "identity.nofile", "---x"]),
        s(&["foo", "--inlta", "identity.nofile", "--outlta", "o.lta"]),
        s(&["--inlta", "identity.nofile", "foo", "--outlta", "o.lta"]),
        s(&["--outlta", "o.lta"]),
        s(&[
            "--regheader",
            "--src",
            "s.mgz",
            "--trg",
            "t.mgz",
            "--outlta",
            "o.lta",
        ]),
        s(&["--inlta", "identity.nofile", "--outlta", "o.lta"]),
        s(&[
            "--inlta",
            "identity.nofile",
            "--outlta",
            "o.lta",
            "--ltavox2vox",
        ]),
        s(&[
            "--inlta",
            "identity.nofile",
            "--srcconform-size",
            "2",
            "--outlta",
            "o.lta",
        ]),
        s(&[
            "--inlta",
            "identity.nofile",
            "--srcconform-dc",
            "--outlta",
            "o.lta",
        ]),
        s(&[
            "--inlta",
            "identity.nofile",
            "--src",
            "s.mgz",
            "--trg",
            "t.mgz",
            "--outlta",
            "o.lta",
            "--subject",
            &"q".repeat(1030),
        ]),
        s(&[
            "--inlta",
            "identity.nofile",
            "--src",
            "s.mgz",
            "--trg",
            "t.mgz",
            "--outlta",
            "o.lta",
            "--subject",
            &"q".repeat(5000),
        ]),
        // recon-all's step 4, talairach_avi's, bbregister's, fMRIPrep's
        s(&[
            "--inlta",
            "identity.nofile",
            "--src",
            "s.mgz",
            "--trg",
            "t.mgz",
            "--outlta",
            "o.lta",
            "--subject",
            "sub-01",
        ]),
        s(&[
            "--src",
            "s.mgz",
            "--trg",
            "big.mgz",
            "--inxfm",
            "t.xfm",
            "--outlta",
            "o.lta",
            "--subject",
            "fsaverage",
            "--ltavox2vox",
        ]),
        s(&["--inlta", "r.lta", "--outreg", "o.dat"]),
        s(&["--inlta", "r.lta", "--outfsl", "o.mat"]),
        s(&[
            "--infsl",
            "fslrot.mat",
            "--src",
            "s.nii",
            "--trg",
            "t.nii",
            "--outlta",
            "o.lta",
        ]),
        s(&["--inlta", "r.lta", "--outlta", "o.lta"]),
        s(&[
            "--inlta",
            "identity.nofile",
            "--src",
            "s.mgz",
            "--trg",
            "t.mgz",
            "--outlta",
            "o.lta",
            "--invert",
        ]),
    ] {
        out.push(line(&[FSL], &i, &args));
    }
    // an ITK number that is not one: FreeSurfer never ends (FS-1162)
    out.push(line(
        &[FSL],
        &i,
        &with(&["--initk", "itk_hang.txt", "--outlta", "o.lta"]),
    ));
    // FSL's conversions without FSLOUTPUTTYPE
    out.push(line(
        &[],
        &i,
        &s(&["--inlta", "r.lta", "--outfsl", "o.mat"]),
    ));
    out.push(line(
        &[],
        &i,
        &with(&["--infsl", "fsl.mat", "--outlta", "o.lta"]),
    ));
    // every input, with the geometries and without, to an .lta
    for (opt, names) in INPUTS {
        for n in *names {
            out.push(line(&[FSL], &i, &with(&[opt, n, "--outlta", "o.lta"])));
            out.push(line(&[FSL], &i, &s(&[opt, n, "--outlta", "o.lta"])));
        }
    }
    for n in ["itk_2d.txt", "itk_2dfp.txt"] {
        for (a, b) in [
            ("flat.mgz", "flat.mgz"),
            ("flat.mgz", "flat2.mgz"),
            ("flatz.mgz", "flatz.mgz"),
            ("flat.mgz", "flatz.mgz"),
        ] {
            out.push(line(
                &[FSL],
                &i,
                &s(&["--initk", n, "--src", a, "--trg", b, "--outlta", "o.lta"]),
            ));
        }
    }
    // every output, from three inputs
    for input in [
        ["--initk", "itk_r.txt"],
        ["--inlta", "r.lta"],
        ["--inmni", "t.xfm"],
    ] {
        for (opt, names) in OUTPUTS {
            for n in *names {
                out.push(line(&[FSL], &i, &with(&[input[0], input[1], opt, n])));
            }
        }
        out.push(line(
            &[FSL],
            &i,
            &with(&[
                input[0],
                input[1],
                "--ltatkreg",
                "--outlta",
                "o.lta",
                "--outmni",
                "o.xfm",
            ]),
        ));
        out.push(line(
            &[FSL],
            &i,
            &with(&[
                input[0],
                input[1],
                "--sqrt",
                "--outlta",
                "o.lta",
                "--outfsl",
                "o.mat",
                "--outitk",
                "o.itk.txt",
            ]),
        ));
        out.push(line(
            &[FSL],
            &i,
            &with(&[
                input[0],
                input[1],
                "--invert",
                "--outlta",
                "o.lta",
                "--outvox",
                "o.vox.txt",
            ]),
        ));
    }
    // the geometry options
    for g in [
        &["--srcconform"][..],
        &["--srcconform-dc"],
        &["--srcconform-min"],
        &["--srcconform-size", "0.5"],
        &["--trgconform"],
        &["--trgconform-dc"],
        &["--trgconform-min"],
        &["--trgconform-size", "1.5", "--outitk", "o.itk.txt"],
        &["--srcupsample", "2"],
        &["--srcdownsample", "2"],
        &["--srcdownsample", "3"],
        &["--srcdownsample", "0"],
        &["--srcupsample", "0"],
        &["--srcupsample", "-2"],
        &["--trgupsample", "3"],
        &["--trgdownsample", "5"],
        &["--trgdownsample", "-1"],
        &["--srcupsample", "2", "--srcdownsample", "2"],
    ] {
        for (src, trg) in [
            ("s.mgz", "t.mgz"),
            ("obl.mgz", "big.mgz"),
            ("d45.mgz", "bad.mgz"),
        ] {
            let mut a = s(&[
                "--inlta",
                "identity.nofile",
                "--src",
                src,
                "--trg",
                trg,
                "--outlta",
                "o.lta",
            ]);
            a.extend(s(g));
            out.push(line(&[FSL], &i, &a));
        }
    }
    out.push(line(
        &[FSL],
        &i,
        &s(&[
            "--inlta",
            "identity.nofile",
            "--srcconform-dc",
            "--trgconform-min",
            "--outlta",
            "o.lta",
        ]),
    ));
    out.push(line(
        &[FSL],
        &i,
        &with(&["--INLTA", "r.lta", "-OutLta", "o.lta", "--Invert"]),
    ));
    out
}

/// A seeded case: an input, its geometries, zero to three geometry options, the switches, one
/// to three outputs, sometimes in another order or spelling.
fn seeded(g: &mut Gen, i: &Inputs) -> String {
    let mut groups: Vec<Vec<String>> = Vec::new();
    if !g.chance(25) {
        let (opt, names) = g.pick(INPUTS);
        groups.push(s(&[opt, g.pick(names)]));
    } else {
        groups.push(s(&["--regheader"]));
    }
    for opt in ["--src", "--trg"] {
        if !g.chance(8) {
            groups.push(s(&[opt, g.pick(VOLUMES)]));
        }
    }
    let geoms: &[&[&str]] = &[
        &["--srcconform"],
        &["--srcconform-dc"],
        &["--srcconform-min"],
        &["--trgconform"],
        &["--trgconform-dc"],
        &["--trgconform-min"],
        &["--srcupsample", "2"],
        &["--srcdownsample", "2"],
        &["--trgupsample", "2"],
        &["--trgdownsample", "2"],
    ];
    for _ in 0..g.below(3) {
        if g.chance(3) {
            groups.push(s(g.pick(geoms)));
        }
    }
    if g.chance(10) {
        let opt = g.pick(&["--srcconform-size", "--trgconform-size"]);
        groups.push(s(&[opt, g.pick(&["2", "0.5", "1", "1.25"])]));
    }
    if g.chance(6) {
        groups.push(s(&["--invert"]));
    }
    if g.chance(5) {
        groups.push(s(&["--subject", g.pick(&["sub-01", "fsaverage", "x"])]));
    }
    if g.chance(20) {
        groups.push(s(&["--sqrt"]));
    }
    if g.chance(6) {
        groups.push(s(&[g.pick(&["--ltavox2vox", "--ltatkreg"])]));
    }
    let nout = 1 + g.below(3);
    for _ in 0..nout {
        let (opt, names) = g.pick(OUTPUTS);
        groups.push(s(&[opt, g.pick(names)]));
    }
    if g.chance(3) {
        // another order
        for k in (1..groups.len()).rev() {
            let j = g.below(k as u64 + 1) as usize;
            groups.swap(k, j);
        }
    }
    let mut args: Vec<String> = groups.into_iter().flatten().collect();
    if g.chance(5) {
        // another spelling of an option: one dash, upper or mixed case
        let k = g.below(args.len() as u64) as usize;
        if let Some(name) = args[k].strip_prefix("--") {
            args[k] = match g.below(3) {
                0 => format!("-{name}"),
                1 => format!("--{}", name.to_ascii_uppercase()),
                _ => format!("-{}", name.to_ascii_uppercase()),
            };
        }
    }
    let env: &[&str] = match g.below(8) {
        0 => &[],
        1 => &["FSLOUTPUTTYPE=NIFTI"],
        2 => &["FSLOUTPUTTYPE=ANALYZE"],
        _ => &[FSL],
    };
    line(env, i, &args)
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

/// The cases only Linux's names hold (not in the recorded test: Windows drops a name's last dot):
/// a volume named `trail.`.
pub fn extra_cases() -> Vec<String> {
    let i = base_inputs();
    let mut out = Vec::new();
    for args in [
        s(&[
            "--inlta",
            "identity.nofile",
            "--src",
            "trail.",
            "--trg",
            "t.mgz",
            "--outlta",
            "o.lta",
        ]),
        s(&["--inlta", "r.lta", "--trg", "trail.", "--outlta", "o.lta"]),
        s(&[
            "--initk",
            "itk_r.txt",
            "--src",
            "s.mgz",
            "--trg",
            "trail.",
            "--outlta",
            "o.lta",
        ]),
    ] {
        out.push(line(&[FSL], &i, &args));
    }
    out
}

/// The golden cases (`tests/lta_convert_golden.rs`): recon-all's, talairach's, fMRIPrep's and
/// bbregister's calls, ITK both ways, an inverse, on 32^3 volumes.
pub fn golden_cases() -> Vec<String> {
    let mut g = Gen::new(0x601d);
    let mut files = Vec::new();
    for (name, s) in [
        (
            "rawavg.mgz",
            spec(
                MRI_SHORT,
                [32, 30, 28],
                [1.0, 1.333_333, 1.333_333],
                IDENT,
                [-1.0, -5.0, -1.0],
            ),
        ),
        (
            "orig.mgz",
            spec(
                MRI_UCHAR,
                [32, 32, 32],
                [1.0, 1.0, 1.0],
                CORONAL,
                [-1.0, -5.0, -1.0],
            ),
        ),
        (
            "mni.mgz",
            spec(
                MRI_UCHAR,
                [32, 32, 32],
                [1.0, 1.0, 1.0],
                CORONAL,
                [0.0, 18.0, 18.0],
            ),
        ),
        (
            "bold.nii",
            spec(
                MRI_FLOAT,
                [16, 16, 12],
                [3.0, 3.0, 3.5],
                IDENT,
                [2.0, -10.0, 15.0],
            ),
        ),
    ] {
        let mut m = build(&mut g, &s);
        files.extend(write_as(&mut m, name));
    }
    files.push(("talairach.xfm".to_owned(), info::XFM.to_vec()));
    files.push(("register.lta".to_owned(), lta_text(1, ROT, 1)));
    files.push((
        "reg.txt".to_owned(),
        itk(
            "AffineTransform_double_3_3",
            "0.99 0.05 -0.02 -0.05 0.99 0.01 0.02 -0.01 1 1.5 -2 0.75",
            Some("0 0 0"),
            "\n",
        ),
    ));
    let i = Inputs {
        files,
        dirs: Vec::new(),
    };
    let mut out = Vec::new();
    for args in [
        s(&[
            "--inlta",
            "identity.nofile",
            "--src",
            "rawavg.mgz",
            "--trg",
            "orig.mgz",
            "--outlta",
            "rawavg2orig.lta",
            "--subject",
            "sub-01",
        ]),
        s(&[
            "--src",
            "orig.mgz",
            "--trg",
            "mni.mgz",
            "--inxfm",
            "talairach.xfm",
            "--outlta",
            "talairach.xfm.lta",
            "--subject",
            "fsaverage",
            "--ltavox2vox",
        ]),
        s(&[
            "--initk",
            "reg.txt",
            "--src",
            "bold.nii",
            "--trg",
            "orig.mgz",
            "--outlta",
            "bold2t1.lta",
            "--outfsl",
            "bold2t1.mat",
            "--outreg",
            "bold2t1.dat",
        ]),
        s(&["--inlta", "register.lta", "--outitk", "x.txt"]),
        s(&[
            "--initk",
            "reg.txt",
            "--src",
            "bold.nii",
            "--trg",
            "orig.mgz",
            "--invert",
            "--outitk",
            "inv.txt",
            "--outvox",
            "inv.vox.txt",
            "--outniftyreg",
            "inv.nr.txt",
            "--outmni",
            "inv.xfm",
        ]),
    ] {
        out.push(line(&[FSL], &i, &args));
    }
    out
}

// ------------------------------------------------------------------ the port's answer

/// Text files listed with their bytes: the transforms' formats.
fn is_text(name: &str) -> bool {
    [".lta", ".mat", ".xfm", ".dat", ".reg", ".txt", ".nofile"]
        .iter()
        .any(|e| name.ends_with(e))
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
    assert_eq!(t[0], "lta");
    let mut env = probe_env();
    if t[1] != "-" {
        let e = String::from_utf8(unhex(t[1])).expect("assignments");
        for a in e.split(';') {
            let (k, v) = a.split_once('=').expect("NAME=value");
            env.push((k.to_owned(), v.to_owned()));
        }
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

/// The port's line for a case line and the defects it noted (`dir`: an empty folder), with the
/// pseudo-notices `CRASH` and `MEMORY`.
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
    let mut ids: Vec<&'static str> = g.notices.as_slice().iter().map(|n| n.defect.id).collect();
    if matches!(status, 134 | 136 | 139) {
        ids.push("CRASH");
    }
    if g.notices
        .as_slice()
        .iter()
        .any(|n| n.defect.id == "FS-1163" && n.detail.contains("memory contents"))
    {
        ids.push("MEMORY");
    }
    (text, ids)
}

/// [`run_line_in`] in a fresh temporary folder.
pub fn run_line(line: &str, mode: UpstreamDefects) -> (String, Vec<&'static str>) {
    let dir = tempfile::tempdir().expect("a temporary folder");
    run_line_in(line, mode, dir.path())
}
