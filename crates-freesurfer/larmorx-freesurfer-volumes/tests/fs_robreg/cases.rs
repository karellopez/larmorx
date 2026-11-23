// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The fs-robust-register probe's cases (`validation/probes/fs-robust-register/`): a seeded
//! generator of `mri_robust_register` command lines with the files they read, and the port's
//! answer to each, printed as `check.py` prints the official program's (`<status> | <stdout> |
//! <stderr> | <files>`, C-escaped; the line of the time taken masked).
//!
//! A case line is `rreg <env> <nfiles> [<name> <hex>]... <ndirs> [<hex>]... <nargs> [<hex>]...`,
//! as `fs_xform/cases.rs`' (`<env>`: `-` or the hex of one `NAME=value` added). The volumes are
//! head-like patterns made with integer arithmetic (a bright ellipsoid with a darker shell, a box,
//! seeded noise), the target the source moved by a few voxels and rotated by small integer steps,
//! written by the port's own writer (both sides read the same bytes).
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
use larmorx_freesurfer_mri::mri::{
    MGZ_INTENT_UNKNOWN, MRI_FLOAT, MRI_INT, MRI_SHORT, MRI_UCHAR, Mri,
};
use larmorx_freesurfer_mri::voxel::mri_set_vox_val;
use larmorx_freesurfer_volumes::mri_robust_register::main::main;
use larmorx_freesurfer_volumes::mri_robust_register::options::Run;

pub use info::{Gen, escape, hex, unhex};
use info::{Spec, build, collect, fnv, mask_bytes};

/// The probe's environment: `probe_env`'s, with the LTA's creation line fixed (the user, the
/// time FreeSurfer's override names).
pub fn probe_env() -> Vec<(String, String)> {
    let mut e = info::probe_env();
    for (k, v) in [
        ("USER", "larmorx"),
        ("TZ", "UTC0"),
        (
            "FREESURFER_REPLACEMENT_FOR_CREATION_TIME_STRING",
            "Thu Jan  1 00:00:00 1970",
        ),
    ] {
        e.push((k.to_owned(), v.to_owned()));
    }
    e
}

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

/// LIA, as `mri_convert --conform` gives (most heads), and RAS.
const LIA: [f32; 9] = [-1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 1.0, 0.0];
const RAS: [f32; 9] = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];

/// How a case's image is made.
#[derive(Clone, Copy)]
pub struct Img {
    pub dims: [i32; 3],
    pub nframes: i32,
    pub ty: i32,
    pub size: [f32; 3],
    pub mdc: [f32; 9],
    pub c: [f32; 3],
    /// the pattern's shift (voxels, times 4) and in-plane shear (times 16)
    pub shift: [i32; 3],
    pub shear: i32,
    /// the intensity factor (times 16) and the noise's range
    pub gain: i32,
    pub noise: i32,
    /// a white background (the pattern inverted) and a mask (1 inside the ellipsoid)
    pub white: bool,
    pub mask: bool,
}

impl Img {
    pub fn base(n: i32, ty: i32) -> Img {
        Img {
            dims: [n, n, n],
            nframes: 1,
            ty,
            size: [1.0, 1.0, 1.0],
            mdc: LIA,
            c: [0.5, -2.0, 1.0],
            shift: [0, 0, 0],
            shear: 0,
            gain: 16,
            noise: 3,
            white: false,
            mask: false,
        }
    }
}

/// The image's voxels (integer arithmetic only: the same everywhere) and header.
pub fn image(g: &mut Gen, im: &Img) -> Mri {
    let s = spec(
        im.ty,
        [im.dims[0], im.dims[1], im.dims[2], im.nframes],
        im.size,
        im.mdc,
        im.c,
    );
    let mut m = build(g, &s);
    let mut fs = Globals::new(UpstreamDefects::Fix);
    let mut e: Vec<u8> = Vec::new();
    let [w, h, d] = im.dims;
    for f in 0..im.nframes {
        for z in 0..d {
            for y in 0..h {
                for x in 0..w {
                    // the voxel's place in the pattern (quarter voxels), sheared in plane
                    let px = 4 * x - im.shift[0] + (im.shear * y) / 4;
                    let py = 4 * y - im.shift[1] - (im.shear * x) / 4;
                    let pz = 4 * z - im.shift[2];
                    let (cx, cy, cz) = (2 * w, 2 * h, 2 * d);
                    let (dx, dy, dz) = (px - cx, py - cy, pz - cz);
                    let r2 = (dx * dx * 100) / (w * w)
                        + (dy * dy * 81) / (h * h)
                        + (dz * dz * 64) / (d * d);
                    let mut v = if r2 < 300 {
                        110
                    } else if r2 < 600 {
                        75
                    } else if r2 < 900 {
                        30
                    } else {
                        2
                    };
                    if px > cx && px < cx + 2 * w / 3 && py > cy - h / 2 && py < cy + h / 3 {
                        v += 40;
                    }
                    if im.mask {
                        v = i32::from(r2 < 600);
                    } else {
                        if im.white {
                            v = 200 - v;
                        }
                        v = v * im.gain / 16 + g.int(0, i64::from(im.noise)) as i32;
                    }
                    let val = if im.ty == MRI_FLOAT {
                        v as f32 + 0.25 * (g.int(0, 3) as f32)
                    } else {
                        v as f32
                    };
                    mri_set_vox_val(&mut fs, &mut e, &mut m, x, y, z, f, val, 0).expect("a voxel");
                }
            }
        }
    }
    m
}

/// The bytes of `m` written as `name` (`mri_write`, the format the name gives).
pub fn write_as(m: &mut Mri, name: &str) -> Vec<u8> {
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
        .into_iter()
        .find(|(n, _)| n == name)
        .map(|(_, b)| b)
        .expect("the written volume")
}

/// An initial transform, RAS to RAS, without geometries (FreeSurfer takes the images'), a small
/// shift and rotation.
const LTA_INIT: &[u8] = b"# transform file init.lta\ntype      = 1 # LINEAR_RAS_TO_RAS\nnxforms   = 1\nmean      = 0.0000 0.0000 0.0000\nsigma     = 1.0000\n1 4 4\n9.998e-01 -1.745e-02 0.0 1.0\n1.745e-02 9.998e-01 0.0 -0.5\n0.0 0.0 1.0 0.25\n0.0 0.0 0.0 1.0\nsrc volume info\nvalid = 0  # volume info invalid\ndst volume info\nvalid = 0  # volume info invalid\nsubject unknown\nfscale 0.100000\n";
/// The same transform as MINC's text.
const XFM_INIT: &[u8] = b"MNI Transform File\n\nTransform_Type = Linear;\nLinear_Transform =\n 0.9998 -0.01745 0 1\n 0.01745 0.9998 0 -0.5\n 0 0 1 0.25;\n";

/// The text files `--iscalein` reads.
const ISCALES: &[(&str, &[u8])] = &[
    ("s09.txt", b"0.9\n"),
    ("s12.txt", b"  1.25 junk"),
    ("sbad.txt", b"abc"),
    ("sempty.txt", b""),
    ("sbig.txt", b"1e400"),
    ("sneg.txt", b"-2"),
];

/// A case line: the files (name, bytes), the folders and the arguments.
pub fn line(
    env: Option<&str>,
    files: &[(String, Vec<u8>)],
    dirs: &[&str],
    args: &[String],
) -> String {
    let mut t = vec![
        "rreg".to_owned(),
        env.map_or("-".to_owned(), |e| hex(e.as_bytes())),
    ];
    t.push(files.len().to_string());
    for (n, b) in files {
        t.push(n.clone());
        t.push(hex(b));
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

fn s(a: &[&str]) -> Vec<String> {
    a.iter().map(|x| (*x).to_owned()).collect()
}

/// A random pair of images: the target is the source's pattern moved, sheared and scaled.
fn pair(g: &mut Gen, n: i32, ty: i32) -> (Img, Img) {
    let mut a = Img::base(n, ty);
    let mut b = a;
    b.shift = [
        g.int(-6, 6) as i32,
        g.int(-6, 6) as i32,
        g.int(-4, 4) as i32,
    ];
    b.shear = g.int(-2, 2) as i32;
    b.gain = g.pick(&[16, 16, 15, 17, 18]);
    a.noise = g.pick(&[0, 2, 3, 6]);
    b.noise = g.pick(&[0, 2, 3, 6]);
    if g.chance(6) {
        b.ty = g.pick(&[MRI_UCHAR, MRI_SHORT, MRI_FLOAT, MRI_INT]);
    }
    (a, b)
}

/// The cases every seed starts with: the command line's own paths (the help, the version, the
/// checks, unknown options and costs, the options' missing arguments, the files that do not
/// read) and one registration per output option.
pub fn prelude() -> Vec<String> {
    let mut g = Gen::new(0x0070_656b);
    let (a, b) = pair(&mut g, 16, MRI_UCHAR);
    let mov = write_as(&mut image(&mut g, &a), "mov.mgz");
    let dst = write_as(&mut image(&mut g, &b), "dst.mgz");
    let mut two = a;
    two.nframes = 2;
    let multi = write_as(&mut image(&mut g, &two), "m2.mgz");
    let small = write_as(&mut image(&mut g, &Img::base(10, MRI_UCHAR)), "small.mgz");
    let files = vec![
        ("mov.mgz".to_owned(), mov),
        ("dst.mgz".to_owned(), dst),
        ("m2.mgz".to_owned(), multi),
        ("small.mgz".to_owned(), small),
        ("init.lta".to_owned(), LTA_INIT.to_vec()),
        ("bad.lta".to_owned(), b"not a transform\n".to_vec()),
    ];
    let base = ["--mov", "mov.mgz", "--dst", "dst.mgz", "--lta", "out.lta"];
    let with = |extra: &[&str]| {
        let mut v = s(&base);
        v.extend(s(extra));
        v
    };
    let mut cases: Vec<Vec<String>> = vec![
        s(&[]),
        s(&["--help"]),
        s(&["-h"]),
        s(&["--version"]),
        s(&["--foo"]),
        s(&["-"]),
        s(&["---x"]),
        s(&["--Mov", "mov.mgz"]),
        s(&["--mov", "mov.mgz", "--dst", "dst.mgz"]),
        s(&["--mov", "mov.mgz", "--dst", "dst.mgz", "--lta", "o.lta"]),
        with(&["--sat", "4", "--maxit", "0"]),
        with(&["--sat", "4", "--maxit", "-3"]),
        with(&["--sat", "4", "--iscaleout", "i.txt"]),
        with(&["--satit", "--weights", "w.mgz", "--mapmov", "w.mgz"]),
        with(&["--cost", "xyz"]),
        with(&["--cost", "rob", "--sat", "4"]),
        with(&["--subsample"]),
        s(&["--mov"]),
        s(&["--dst"]),
        s(&["--cost"]),
        s(&["--sat", "3", "--maxit"]),
        s(&["--epsit"]),
        s(&["--mapmov"]),
        s(&["--iscalein"]),
        s(&[
            "--mov", "gone.mgz", "--dst", "dst.mgz", "--lta", "o.lta", "--satit",
        ]),
        s(&[
            "--mov", "mov.mgz", "--dst", "gone.mgz", "--lta", "o.lta", "--satit",
        ]),
        s(&[
            "--mov", "m2.mgz", "--dst", "dst.mgz", "--lta", "o.lta", "--satit",
        ]),
        s(&[
            "--mov", "mov.mgz", "--dst", "m2.mgz", "--lta", "o.lta", "--satit",
        ]),
        s(&[
            "--mov",
            "small.mgz",
            "--dst",
            "dst.mgz",
            "--lta",
            "o.lta",
            "--sat",
            "4",
        ]),
        with(&["--satit", "--maskmov", "gone.mgz"]),
        with(&["--satit", "--ixform", "gone.lta"]),
        with(&["--satit", "--ixform", "bad.lta"]),
        with(&["--satit", "--iscalein", "gone.txt"]),
        with(&["--satit", "--iscalein", "."]),
        with(&["--sat", "4", "--ixform", "init.lta", "--verbose", "2"]),
        with(&["--satit", "--iscaleonly"]),
        s(&[
            "--mov",
            "mov.mgz",
            "--dst",
            "dst.mgz",
            "--iscaleonly",
            "--sat",
            "4",
        ]),
        with(&["--satit", "--transonly", "--noinit"]),
        with(&["--satit", "--affine", "--iscale", "--vox2vox"]),
        with(&["--satit", "--isoscale", "--initorient"]),
        with(&["--sat", "6", "--nosym", "--halfmov", "h.mgz"]),
        with(&[
            "--sat",
            "6",
            "--leastsquares",
            "--doubleprec",
            "--nomulti",
            "--maxit",
            "3",
        ]),
        with(&["--leastsquares", "--epsit", "0.5"]),
        with(&[
            "--satest",
            "--conform",
            "--sat",
            "4",
            "--entball",
            "--entcorrection",
            "--entradius",
            "2",
        ]),
        with(&[
            "--sat",
            "4",
            "--entmov",
            "e1.mgz",
            "--entdst",
            "e2.mgz",
            "--powelltol",
            "0.001",
        ]),
        with(&[
            "--sat",
            "4",
            "--wlimit",
            "0.3",
            "--highit",
            "2",
            "--minsize",
            "8",
            "--maxsize",
            "32",
        ]),
        with(&["--satit", "--subsample", "8"]),
        with(&["--satit", "--subsample", "-1"]),
        with(&["--satit", "--initscaling", "--floattype"]),
        with(&[
            "--satit",
            "--iscale",
            "--mapmov",
            "mm.mgz",
            "--weights",
            "w.mgz",
        ]),
        with(&[
            "--satit",
            "--iscale",
            "--mapmovhdr",
            "mh.mgz",
            "--weights",
            "w.mgz",
            "--oneminusw",
        ]),
        with(&["--satit", "--mapmovhdr", "nodir/mh.mgz"]),
        with(&["--satit", "--weights", "w.mgz"]),
        with(&[
            "--satit",
            "--halfmov",
            "hm.mgz",
            "--halfdst",
            "hd.mgz",
            "--halfweights",
            "hw.mgz",
            "--halfmovlta",
            "hm.lta",
            "--halfdstlta",
            "hd.lta",
        ]),
        with(&[
            "--satit",
            "--vox2vox",
            "--halfmovlta",
            "hm.lta",
            "--halfdstlta",
            "hd.lta",
        ]),
        with(&["--satit", "--iscale", "--iscaleout", "i.txt"]),
        with(&["--satit", "--iscalein", "s09.txt", "--iscale"]),
        with(&["--satit", "--iscalein", "s12.txt"]),
        with(&["--satit", "--iscalein", "sbad.txt", "--iscale"]),
        with(&["--satit", "--iscalein", "sempty.txt", "--iscale"]),
        with(&["--satit", "--iscalein", "sbig.txt"]),
        with(&["--satit", "--iscalein", "sneg.txt", "--iscale"]),
        with(&["--satit", "--uchar"]),
        with(&["--satit", "--whitebgmov", "--whitebgdst"]),
        with(&["--satit", "--sobel"]),
        with(&["--satit", "--verbose", "0"]),
        with(&["--satit", "--verbose", "7"]),
        s(&[
            "-m", "mov.mgz", "-d", "dst.mgz", "--LTA", "out.xfm", "--SATIT", "-a", "-i",
        ]),
        s(&[
            "--mov", "mov.mgz", "--dst", "dst.mgz", "--lta", "noext", "--satit",
        ]),
        s(&[
            "--mov", "mov.mgz", "--dst", "dst.mgz", "--lta", ".lta", "--satit",
        ]),
        s(&[
            "--mov",
            "mov.mgz",
            "--dst",
            "dst.mgz",
            "--lta",
            "nodir/o.lta",
            "--satit",
        ]),
    ];
    // the costs only RegPowell computes, the entropy images, the test mode and the debug output
    // (not ported yet: check.py counts them apart)
    for c in [
        "ls", "mi", "nmi", "ecc", "ncc", "scr", "tb", "lncc", "sad", "sb", "robent",
    ] {
        cases.push(with(&["--cost", c]));
    }
    cases.push(with(&["--satit", "--debug"]));
    cases.push(s(&["--test", "1", "mov.mgz"]));
    let mut all = files.clone();
    for (n, b) in ISCALES {
        all.push(((*n).to_owned(), b.to_vec()));
    }
    cases.iter().map(|a| line(None, &all, &["xd"], a)).collect()
}

/// One seeded case: a pair of images of a random type, size and geometry, and random options.
fn seeded(g: &mut Gen) -> String {
    let n = g.pick(&[16, 16, 17, 18, 20]);
    let ty = g.pick(&[MRI_UCHAR, MRI_UCHAR, MRI_SHORT, MRI_FLOAT, MRI_INT]);
    let (mut a, mut b) = pair(g, n, ty);
    // geometry: another orientation (the source reordered), anisotropic voxels (resliced),
    // another extent
    if g.chance(5) {
        b.mdc = RAS;
    }
    if g.chance(6) {
        a.size = [1.0, 1.0, g.pick(&[1.25, 1.5, 2.0])];
        a.dims[2] = (a.dims[2] * 3) / 4 + 2;
    }
    if g.chance(6) {
        b.dims = [n + 2, n, n + 1];
    }
    if g.chance(10) {
        b.white = true;
    }
    let mut files = vec![
        ("mov.mgz".to_owned(), write_as(&mut image(g, &a), "mov.mgz")),
        ("dst.mgz".to_owned(), write_as(&mut image(g, &b), "dst.mgz")),
    ];
    let mut args = s(&["--mov", "mov.mgz", "--dst", "dst.mgz", "--lta"]);
    args.push(
        g.pick(&["out.lta", "out.lta", "out.lta", "out.xfm", "out.dat"])
            .to_owned(),
    );
    if g.chance(4) {
        args.extend(s(&["--sat", g.pick(&["2", "4.685", "8", "12"])]));
    } else {
        args.push("--satit".to_owned());
    }
    let mut opt = |g: &mut Gen, k: u64, a: &[&str]| {
        if g.chance(k) {
            args.extend(s(a));
        }
    };
    opt(g, 2, &["--iscale"]);
    opt(g, 5, &["--affine"]);
    opt(g, 12, &["--isoscale"]);
    opt(g, 12, &["--transonly"]);
    opt(g, 8, &["--initorient"]);
    opt(g, 12, &["--noinit"]);
    opt(g, 8, &["--nomulti"]);
    opt(g, 8, &["--doubleprec"]);
    opt(g, 10, &["--floattype"]);
    opt(g, 10, &["--nosym"]);
    opt(g, 10, &["--vox2vox"]);
    opt(g, 12, &["--uchar"]);
    opt(g, 12, &["--whitebgmov"]);
    opt(g, 12, &["--sobel"]);
    opt(g, 6, &["--maxit", "3"]);
    opt(g, 10, &["--highit", "2"]);
    opt(g, 10, &["--epsit", "0.05"]);
    opt(g, 12, &["--subsample", "12"]);
    opt(g, 12, &["--wlimit", "0.25"]);
    opt(g, 3, &["--mapmov", "mm.mgz"]);
    opt(g, 4, &["--weights", "w.mgz"]);
    opt(g, 10, &["--mapmovhdr", "mh.mgz"]);
    opt(
        g,
        8,
        &[
            "--halfmov",
            "hm.mgz",
            "--halfweights",
            "hw.mgz",
            "--halfdstlta",
            "hd.lta",
        ],
    );
    opt(g, 10, &["--oneminusw"]);
    opt(g, 10, &["--verbose", "2"]);
    if g.chance(8) {
        let mut m = a;
        m.mask = true;
        files.push((
            "mask.mgz".to_owned(),
            write_as(&mut image(g, &m), "mask.mgz"),
        ));
        args.extend(s(&["--maskmov", "mask.mgz"]));
    }
    if g.chance(8) {
        let (n, t) = g.pick(&[("init.lta", LTA_INIT), ("init.xfm", XFM_INIT)]);
        files.push((n.to_owned(), t.to_vec()));
        args.extend(s(&["--ixform", n]));
    }
    if g.chance(10) {
        args.extend(s(&["--iscaleout", "i.txt", "--iscale"]));
    }
    line(None, &files, &[], &args)
}

/// The Powell registration's cases (relay 46; `check.py --family powell`, the recorded test
/// `fs_robreg_powell_official`): pairs of images of a random type (a few 2-D), each
/// with one of the costs ported (`LS`, `NCC`, `TB`, `LNCC`, `SAD`) and random options (the
/// models, the intensity scale, no symmetry, the full images alone, the tolerance, the start, the
/// subsampling, the outputs but the half-way ones).
pub fn powell_cases() -> Vec<String> {
    let mut g = Gen::new(0x0070_6f77);
    (0..40).map(|_| seeded_powell(&mut g)).collect()
}

/// The Powell registration's costs of a joint histogram and the SB cost (relay 47): 32 cases of
/// `MI`, `NMI`, `ECC`, `SCR` and `SB` with the Powell family's random options, a seed of their own
/// (the Powell family's 40 cases stay as they were).
pub fn histo_cases() -> Vec<String> {
    let mut g = Gen::new(0x0068_6973);
    (0..32)
        .map(|_| seeded_powell_with(&mut g, &["mi", "nmi", "ecc", "scr", "sb", "MI"]))
        .collect()
}

fn seeded_powell(g: &mut Gen) -> String {
    seeded_powell_with(g, &["ls", "ncc", "tb", "lncc", "sad", "LS"])
}

fn seeded_powell_with(g: &mut Gen, costs: &[&str]) -> String {
    let flat = g.chance(8);
    let n = g.pick(&[16, 16, 17, 18]);
    let ty = g.pick(&[MRI_UCHAR, MRI_UCHAR, MRI_SHORT, MRI_FLOAT, MRI_INT]);
    let (mut a, mut b) = pair(g, n, ty);
    if flat {
        a.dims = [n + 2, n, 1];
        b.dims = [n + 2, n, 1];
    } else {
        if g.chance(6) {
            b.mdc = RAS;
        }
        if g.chance(6) {
            b.dims = [n + 2, n, n + 1];
        }
    }
    let files = vec![
        ("mov.mgz".to_owned(), write_as(&mut image(g, &a), "mov.mgz")),
        ("dst.mgz".to_owned(), write_as(&mut image(g, &b), "dst.mgz")),
    ];
    let mut args = s(&["--mov", "mov.mgz", "--dst", "dst.mgz", "--lta", "out.lta"]);
    args.extend(s(&["--cost", g.pick(costs)]));
    let mut opt = |g: &mut Gen, k: u64, a: &[&str]| {
        if g.chance(k) {
            args.extend(s(a));
        }
    };
    opt(g, 3, &["--iscale"]);
    opt(g, 5, &["--affine"]);
    opt(g, 10, &["--isoscale"]);
    opt(g, 8, &["--transonly"]);
    opt(g, 6, &["--nosym"]);
    opt(g, 6, &["--nomulti"]);
    opt(g, 6, &["--powelltol", "0.001"]);
    opt(g, 10, &["--initorient"]);
    opt(g, 10, &["--noinit"]);
    opt(g, 8, &["--subsample", "12"]);
    opt(g, 10, &["--uchar"]);
    opt(g, 10, &["--satit"]);
    opt(g, 8, &["--vox2vox"]);
    opt(g, 4, &["--mapmov", "mm.mgz"]);
    opt(g, 8, &["--weights", "w.mgz"]);
    opt(g, 6, &["--verbose", "2"]);
    opt(g, 12, &["--iscaleout", "i.txt", "--iscale"]);
    line(None, &files, &[], &args)
}

/// The entropy images' cases (relay 45, `--cost ROBENT`): byte, short and float sources, a mask
/// on each side, 2-D images, every entropy option (the radius 0, 1, 2, 9 and below 0, the ball,
/// the correction and both, the written images), the Sobel image and the intensity scale first.
pub fn entropy_cases() -> Vec<String> {
    let mut g = Gen::new(0x0065_6e74);
    let (a, b) = pair(&mut g, 16, MRI_UCHAR);
    let mov = write_as(&mut image(&mut g, &a), "mov.mgz");
    let dst = write_as(&mut image(&mut g, &b), "dst.mgz");
    let (mut fa, mut sb) = (a, b);
    fa.ty = MRI_FLOAT;
    sb.ty = MRI_SHORT;
    let fmov = write_as(&mut image(&mut g, &fa), "fmov.mgz");
    let sdst = write_as(&mut image(&mut g, &sb), "sdst.mgz");
    let mut m = a;
    m.mask = true;
    let mask = write_as(&mut image(&mut g, &m), "mask.mgz");
    let (mut a2, mut b2) = (a, b);
    a2.dims = [18, 16, 1];
    b2.dims = [18, 16, 1];
    let flat_mov = write_as(&mut image(&mut g, &a2), "flatmov.mgz");
    let flat_dst = write_as(&mut image(&mut g, &b2), "flatdst.mgz");
    let files = vec![
        ("mov.mgz".to_owned(), mov),
        ("dst.mgz".to_owned(), dst),
        ("fmov.mgz".to_owned(), fmov),
        ("sdst.mgz".to_owned(), sdst),
        ("mask.mgz".to_owned(), mask),
        ("flatmov.mgz".to_owned(), flat_mov),
        ("flatdst.mgz".to_owned(), flat_dst),
    ];
    let base = [
        "--mov", "mov.mgz", "--dst", "dst.mgz", "--lta", "out.lta", "--cost", "robent",
    ];
    let with = |extra: &[&str]| {
        let mut v = s(&base);
        v.extend(s(extra));
        v
    };
    let flat = |extra: &[&str]| {
        let mut v = s(&[
            "--mov",
            "flatmov.mgz",
            "--dst",
            "flatdst.mgz",
            "--lta",
            "out.lta",
            "--cost",
            "ROBENT",
        ]);
        v.extend(s(extra));
        v
    };
    let cases: Vec<Vec<String>> = vec![
        with(&["--satit"]),
        with(&["--sat", "4", "--entradius", "1"]),
        with(&["--sat", "4", "--entradius", "2", "--entball"]),
        with(&["--sat", "4", "--entradius", "1", "--entcorrection"]),
        with(&["--sat", "4", "--entradius", "2", "--entcorrection"]),
        with(&[
            "--sat",
            "4",
            "--entradius",
            "1",
            "--entball",
            "--entcorrection",
        ]),
        with(&[
            "--sat",
            "4",
            "--entradius",
            "2",
            "--entmov",
            "e1.mgz",
            "--entdst",
            "e2.mgz",
        ]),
        s(&[
            "--mov",
            "fmov.mgz",
            "--dst",
            "sdst.mgz",
            "--lta",
            "out.lta",
            "--cost",
            "robent",
            "--sat",
            "4",
            "--entradius",
            "2",
            "--entmov",
            "e1.mgz",
        ]),
        with(&[
            "--sat",
            "4",
            "--entradius",
            "2",
            "--maskmov",
            "mask.mgz",
            "--maskdst",
            "mask.mgz",
            "--entdst",
            "e2.mgz",
        ]),
        with(&["--sat", "4", "--entradius", "0"]),
        with(&["--sat", "4", "--entradius", "-1"]),
        with(&["--sat", "4", "--entradius", "9"]),
        with(&["--sat", "4", "--entradius", "1", "--sobel"]),
        with(&["--satit", "--iscale", "--entradius", "2", "--affine"]),
        flat(&["--sat", "4", "--entradius", "2", "--entmov", "e1.mgz"]),
        flat(&[
            "--sat",
            "4",
            "--entradius",
            "1",
            "--entcorrection",
            "--entdst",
            "e2.mgz",
        ]),
        flat(&["--sat", "4", "--entradius", "-2", "--entmov", "e1.mgz"]),
    ];
    cases.iter().map(|a| line(None, &files, &[], a)).collect()
}

/// `n` cases of `seed`: the prelude, the entropy cases, then seeded ones.
pub fn generate(seed: u64, n: usize) -> Vec<String> {
    let mut out = prelude();
    out.extend(entropy_cases());
    let mut g = Gen::new(seed);
    while out.len() < n {
        out.push(seeded(&mut g));
    }
    out.truncate(n);
    out
}

/// The golden cases (`tests/mri_robust_register_golden.rs`): 32^3 images (integer arithmetic),
/// fMRIPrep's call with every output, an affine registration with its half-way outputs, one of a
/// short source onto a float target with anisotropic voxels, the entropy images of a short
/// source and a byte target (`--cost ROBENT`, both written), and the Powell registration of the
/// short source onto the byte target (`--cost NCC`).
pub fn golden_cases() -> Vec<String> {
    let mut g = Gen::new(32);
    let mut a = Img::base(32, MRI_UCHAR);
    let mut b = a;
    b.shift = [5, -3, 2];
    b.shear = 1;
    b.gain = 17;
    let mov = write_as(&mut image(&mut g, &a), "mov.mgz");
    let dst = write_as(&mut image(&mut g, &b), "dst.mgz");
    a.ty = MRI_SHORT;
    b.ty = MRI_FLOAT;
    b.size = [1.0, 1.0, 1.5];
    b.dims = [32, 32, 22];
    let smov = write_as(&mut image(&mut g, &a), "smov.mgz");
    let fdst = write_as(&mut image(&mut g, &b), "fdst.mgz");
    let files = vec![
        ("mov.mgz".to_owned(), mov),
        ("dst.mgz".to_owned(), dst),
        ("smov.mgz".to_owned(), smov),
        ("fdst.mgz".to_owned(), fdst),
    ];
    let lines = [
        s(&[
            "--mov",
            "mov.mgz",
            "--dst",
            "dst.mgz",
            "--lta",
            "reg.lta",
            "--satit",
            "--iscale",
            "--mapmov",
            "mm.mgz",
            "--weights",
            "w.mgz",
            "--iscaleout",
            "i.txt",
        ]),
        s(&[
            "--mov",
            "mov.mgz",
            "--dst",
            "dst.mgz",
            "--lta",
            "reg.lta",
            "--sat",
            "4.685",
            "--affine",
            "--halfmov",
            "hm.mgz",
            "--halfdst",
            "hd.mgz",
            "--halfweights",
            "hw.mgz",
            "--halfmovlta",
            "hm.lta",
            "--halfdstlta",
            "hd.lta",
        ]),
        s(&[
            "--mov", "smov.mgz", "--dst", "fdst.mgz", "--lta", "reg.lta", "--satit", "--iscale",
            "--mapmov", "mm.mgz",
        ]),
        // relay 45: the entropy images (the default radius 5 on the short source)
        s(&[
            "--mov", "smov.mgz", "--dst", "dst.mgz", "--lta", "reg.lta", "--cost", "ROBENT",
            "--sat", "4", "--entmov", "em.mgz", "--entdst", "ed.mgz",
        ]),
        // relay 46: the Powell registration (the normalised correlation, the short source)
        s(&[
            "--mov", "smov.mgz", "--dst", "dst.mgz", "--lta", "reg.lta", "--cost", "NCC",
            "--mapmov", "mm.mgz",
        ]),
    ]
    .iter()
    .map(|args| line(None, &files, &[], args))
    .collect::<Vec<_>>();
    // relay 47: a joint histogram's cost (mutual information: log2, exp) and the SB cost
    // (libstdc++'s sort), on 16^3 byte images of their own (the cost of a debug build)
    let c = Img::base(16, MRI_UCHAR);
    let mut d = c;
    d.shift = [2, -1, 1];
    d.gain = 9;
    let small = vec![
        (
            "mov16.mgz".to_owned(),
            write_as(&mut image(&mut g, &c), "mov16.mgz"),
        ),
        (
            "dst16.mgz".to_owned(),
            write_as(&mut image(&mut g, &d), "dst16.mgz"),
        ),
    ];
    let more = [
        s(&[
            "--mov",
            "mov16.mgz",
            "--dst",
            "dst16.mgz",
            "--lta",
            "reg.lta",
            "--cost",
            "MI",
            "--nomulti",
            "--transonly",
            "--powelltol",
            "0.01",
        ]),
        s(&[
            "--mov",
            "mov16.mgz",
            "--dst",
            "dst16.mgz",
            "--lta",
            "reg.lta",
            "--cost",
            "SB",
            "--nomulti",
            "--transonly",
        ]),
    ];
    let mut lines = lines;
    lines.extend(more.iter().map(|args| line(None, &small, &[], args)));
    lines
}

// ------------------------------------------------------------------ the port's answer

/// Text files listed with their bytes: transforms and the intensity scale.
fn is_text(name: &str) -> bool {
    [".lta", ".xfm", ".dat", ".txt"]
        .iter()
        .any(|e| name.ends_with(e))
        || name == "noext"
}

/// Every file of a case's folder, sorted, each as `F <name> <fnv> <size>` (text files with their
/// bytes escaped), the folder's name in them as `<DIR>`; each folder as `D <name>`. The inputs
/// (named in `inputs`) are left out.
pub fn written(dir: &Path, inputs: &[String]) -> String {
    let d = dir.to_string_lossy().into_owned();
    let mut out = Vec::new();
    for (n, b) in collect_all(dir, "") {
        if inputs.contains(&n) {
            continue;
        }
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

/// The lines of the time taken, masked: `Registration took <T> minutes and <T> seconds.` and the
/// entropy image's ` Entropy computation took <T> seconds.` (the first of each).
pub fn mask_time(s: &str) -> String {
    let s = mask_one(
        s,
        " Entropy computation took ",
        " Entropy computation took <T> seconds.",
    );
    mask_one(
        &s,
        "Registration took ",
        "Registration took <T> minutes and <T> seconds.",
    )
}

/// The first `<head>...<n> seconds.` of `s` replaced by `masked`.
fn mask_one(s: &str, head: &str, masked: &str) -> String {
    let Some(at) = s.find(head) else {
        return s.to_owned();
    };
    let rest = &s[at + head.len()..];
    let Some(end) = rest.find(" seconds.") else {
        return s.to_owned();
    };
    format!("{}{masked}{}", &s[..at], &rest[end + " seconds.".len()..])
}

/// Builds a case's folder (`dir`: empty) and returns its arguments, environment and input names.
fn setup(line: &str, dir: &Path) -> (Vec<String>, Vec<(String, String)>, Vec<String>) {
    let t: Vec<&str> = line.split_whitespace().collect();
    assert_eq!(t[0], "rreg");
    let mut env = probe_env();
    if t[1] != "-" {
        let e = String::from_utf8(unhex(t[1])).expect("an assignment");
        let (k, v) = e.split_once('=').expect("NAME=value");
        env.push((k.to_owned(), v.to_owned()));
    }
    let nfiles: usize = t[2].parse().expect("a count");
    let mut i = 3;
    let mut inputs = Vec::new();
    for _ in 0..nfiles {
        let p = dir.join(t[i]);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).expect("a folder");
        }
        std::fs::write(&p, unhex(t[i + 1])).expect("a case file");
        inputs.push(t[i].to_owned());
        i += 2;
    }
    let ndirs: usize = t[i].parse().expect("a count");
    for k in 0..ndirs {
        let name = String::from_utf8(unhex(t[i + 1 + k])).expect("a folder name");
        std::fs::create_dir_all(dir.join(&name)).expect("a folder");
        inputs.push(name);
    }
    i += 1 + ndirs;
    let nargs: usize = t[i].parse().expect("a count");
    let args = (0..nargs)
        .map(|k| String::from_utf8(unhex(t[i + 1 + k])).expect("an argument"))
        .collect();
    (args, env, inputs)
}

/// The port's line for a case line and the defects it noted (`dir`: an empty folder).
pub fn run_line_in(line: &str, mode: UpstreamDefects, dir: &Path) -> (String, Vec<&'static str>) {
    let (args, env, inputs) = setup(line, dir);
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
            // the official side's OMP_NUM_THREADS (the probe's environment: 1)
            threads: environment
                .get("OMP_NUM_THREADS")
                .and_then(|v| v.to_str())
                .and_then(|v| v.parse().ok())
                .unwrap_or(0),
        };
        match main(&mut g, &mut r, &args) {
            Ok(()) => 0,
            Err(f) => f.status(),
        }
    };
    let _ = out.flush();
    let d = dir.to_string_lossy().into_owned();
    let mask = |b: &[u8]| mask_time(&escape(b).replace(&escape(d.as_bytes()), "<DIR>"));
    let files = written(dir, &inputs);
    // libstdc++'s terminate ends with abort (SIGABRT, the official child's `CRASH 6`); the port's
    // status 134 stands for it
    let end = if status == 134 {
        "CRASH 6".to_owned()
    } else {
        format!("EXIT {status}")
    };
    let text = format!("{end} | {} | {} | {files}", mask(&out), mask(&err));
    let ids = g.notices.as_slice().iter().map(|n| n.defect.id).collect();
    (text, ids)
}

/// [`run_line_in`] in a fresh temporary folder.
pub fn run_line(line: &str, mode: UpstreamDefects) -> (String, Vec<&'static str>) {
    let dir = tempfile::tempdir().expect("a temporary folder");
    run_line_in(line, mode, dir.path())
}
