// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The fs-robust-template probe's cases (`validation/probes/fs-robust-template/`): a seeded
//! generator of `mri_robust_template` command lines with the files they read, and the port's
//! answer to each, printed as `check.py` prints the official program's (`<status> | <stdout> |
//! <stderr> | <files>`, C-escaped; the line of the time taken masked).
//!
//! A case line is `rtpl <env> <nfiles> [<name> <hex>]... <ndirs> [<hex>]... <nargs> [<hex>]...`,
//! as `fs_robreg/cases.rs`' (whose head-like images it uses: a bright ellipsoid with a darker
//! shell, a box, seeded noise; each further input the first's pattern moved by a few voxels and
//! sheared, written by the port's own writer, so both sides read the same bytes).
#![allow(dead_code)]

#[path = "../fs_robreg/cases.rs"]
mod rreg;

use std::io::Write;
use std::path::Path;

use larmorx_core::defects::UpstreamDefects;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::TrackedErr;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_mri::mri::{MRI_FLOAT, MRI_INT, MRI_SHORT, MRI_UCHAR};
use larmorx_freesurfer_volumes::mri_robust_template::main::main;
use larmorx_freesurfer_volumes::mri_robust_template::options::Run;

pub use rreg::{Gen, Img, escape, hex, image, unhex, write_as};

/// LIA, RAS and a coronal LPS-like orientation (an axis order the template reorders).
const LIA: [f32; 9] = [-1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 1.0, 0.0];
const RAS: [f32; 9] = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];
const LPI: [f32; 9] = [-1.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, -1.0];

/// An initial transform, RAS to RAS, with no geometry (the template needs one: an error).
const LTA_NOGEOM: &[u8] = b"# transform file init.lta\ntype      = 1 # LINEAR_RAS_TO_RAS\nnxforms   = 1\nmean      = 0.0000 0.0000 0.0000\nsigma     = 1.0000\n1 4 4\n9.998e-01 -1.745e-02 0.0 1.0\n1.745e-02 9.998e-01 0.0 -0.5\n0.0 0.0 1.0 0.25\n0.0 0.0 0.0 1.0\nsrc volume info\nvalid = 0  # volume info invalid\ndst volume info\nvalid = 0  # volume info invalid\nsubject unknown\nfscale 0.100000\n";

/// The text files `--iscalein` reads.
const ISCALES: &[(&str, &[u8])] = &[
    ("s09.txt", b"0.9\n"),
    ("s11.txt", b"1.1"),
    ("s12.txt", b"  1.25 junk"),
    ("sbad.txt", b"abc"),
    ("sempty.txt", b""),
];

/// A case line: the files (name, bytes), the folders and the arguments.
pub fn line(
    env: Option<&str>,
    files: &[(String, Vec<u8>)],
    dirs: &[&str],
    args: &[String],
) -> String {
    let mut t = vec![
        "rtpl".to_owned(),
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

/// `k` inputs: the first the base pattern, each further one moved, sheared and scaled.
fn set(g: &mut Gen, k: usize, n: i32, ty: i32) -> Vec<Img> {
    let a = Img::base(n, ty);
    let mut v = vec![a];
    for _ in 1..k {
        let mut b = a;
        b.shift = [
            g.int(-6, 6) as i32,
            g.int(-6, 6) as i32,
            g.int(-4, 4) as i32,
        ];
        b.shear = g.int(-2, 2) as i32;
        b.gain = g.pick(&[16, 16, 15, 17, 18]);
        b.noise = g.pick(&[0, 2, 3, 6]);
        v.push(b);
    }
    v[0].noise = g.pick(&[0, 2, 3, 6]);
    v
}

/// The files of a set of inputs, named `t1.mgz`, `t2.mgz`, ...
fn files_of(g: &mut Gen, imgs: &[Img], ext: &str) -> Vec<(String, Vec<u8>)> {
    imgs.iter()
        .enumerate()
        .map(|(i, im)| {
            let name = format!("t{}{ext}", i + 1);
            let b = write_as(&mut image(g, im), &name);
            (name, b)
        })
        .collect()
}

fn names(k: usize, stem: &str, ext: &str) -> Vec<String> {
    (1..=k).map(|i| format!("{stem}{i}{ext}")).collect()
}

/// The cases every seed starts with: the command line's own paths (the help, the version, the
/// checks, unknown options, the options' missing arguments, the files that do not read) and one
/// template per option and output.
pub fn prelude() -> Vec<String> {
    let mut g = Gen::new(0x0074_706c);
    let three = set(&mut g, 3, 16, MRI_UCHAR);
    let mut files = files_of(&mut g, &three, ".mgz");
    // a 4-D input of three frames and its 4-D mask; a 2-frame mask
    let mut four = three[0];
    four.nframes = 3;
    files.push((
        "f4.mgz".to_owned(),
        write_as(&mut image(&mut g, &four), "f4.mgz"),
    ));
    let mut m4 = four;
    m4.mask = true;
    files.push((
        "m4.mgz".to_owned(),
        write_as(&mut image(&mut g, &m4), "m4.mgz"),
    ));
    let mut m2 = four;
    m2.mask = true;
    m2.nframes = 2;
    files.push((
        "m2f.mgz".to_owned(),
        write_as(&mut image(&mut g, &m2), "m2f.mgz"),
    ));
    let mut one = three[0];
    one.mask = true;
    for k in 1..=3 {
        let n = format!("mask{k}.mgz");
        let b = write_as(&mut image(&mut g, &one), &n);
        files.push((n, b));
    }
    // other voxel sizes, an other orientation, another type, a 2-D image, an empty image
    let mut aniso = three[1];
    aniso.size = [1.0, 1.0, 1.5];
    aniso.dims[2] = 12;
    files.push((
        "an.mgz".to_owned(),
        write_as(&mut image(&mut g, &aniso), "an.mgz"),
    ));
    let mut ras = three[1];
    ras.mdc = RAS;
    files.push((
        "ras.mgz".to_owned(),
        write_as(&mut image(&mut g, &ras), "ras.mgz"),
    ));
    let mut lpi = three[2];
    lpi.mdc = LPI;
    files.push((
        "lpi.mgz".to_owned(),
        write_as(&mut image(&mut g, &lpi), "lpi.mgz"),
    ));
    let mut fl = three[1];
    fl.ty = MRI_FLOAT;
    files.push((
        "fl.mgz".to_owned(),
        write_as(&mut image(&mut g, &fl), "fl.mgz"),
    ));
    let mut flat = three[0];
    flat.dims[2] = 1;
    files.push((
        "d2a.mgz".to_owned(),
        write_as(&mut image(&mut g, &flat), "d2a.mgz"),
    ));
    let mut flat2 = three[1];
    flat2.dims[2] = 1;
    files.push((
        "d2b.mgz".to_owned(),
        write_as(&mut image(&mut g, &flat2), "d2b.mgz"),
    ));
    let mut empty = three[0];
    empty.gain = 0;
    empty.noise = 0;
    files.push((
        "e1.mgz".to_owned(),
        write_as(&mut image(&mut g, &empty), "e1.mgz"),
    ));
    files.push((
        "e2.mgz".to_owned(),
        write_as(&mut image(&mut g, &empty), "e2.mgz"),
    ));
    // two 12^3 inputs: the joint Tukey biweight's matrix (--average 3) of 3,456 x 1,728
    let small = set(&mut g, 2, 12, MRI_UCHAR);
    for (i, im) in small.iter().enumerate() {
        let n = format!("sm{}.mgz", i + 1);
        let b = write_as(&mut image(&mut g, im), &n);
        files.push((n, b));
    }
    files.push(("nogeom.lta".to_owned(), LTA_NOGEOM.to_vec()));
    files.push(("bad.lta".to_owned(), b"not a transform\n".to_vec()));
    for (n, b) in ISCALES {
        files.push(((*n).to_owned(), b.to_vec()));
    }
    let two = |extra: &[&str]| {
        let mut v = s(&["--mov", "t1.mgz", "t2.mgz", "--template", "tp.mgz"]);
        v.extend(s(extra));
        v
    };
    let tri = |extra: &[&str]| {
        let mut v = s(&[
            "--mov",
            "t1.mgz",
            "t2.mgz",
            "t3.mgz",
            "--template",
            "tp.mgz",
        ]);
        v.extend(s(extra));
        v
    };
    // fMRIPrep's StructuralReference and recon-all's motion correction
    let fmriprep = |k: usize| {
        let mut v = s(&["--satit", "--fixtp", "--mov"]);
        v.extend(names(k, "t", ".mgz"));
        v.extend(s(&[
            "--inittp",
            "1",
            "--iscale",
            "--noit",
            "--template",
            "tp.nii.gz",
            "--subsample",
            "200",
            "--lta",
        ]));
        v.extend(names(k, "tp", ".lta"));
        v
    };
    let reconall = |k: usize| {
        let mut v = s(&["--mov"]);
        v.extend(names(k, "t", ".mgz"));
        v.extend(s(&[
            "--average",
            "1",
            "--template",
            "rawavg.mgz",
            "--satit",
            "--inittp",
            "1",
            "--fixtp",
            "--noit",
            "--iscale",
            "--iscaleout",
        ]));
        v.extend(names(k, "t", "-iscale.txt"));
        v.extend(s(&["--subsample", "200", "--lta"]));
        v.extend(names(k, "t", ".lta"));
        v
    };
    let mut cases: Vec<Vec<String>> = vec![
        s(&[]),
        s(&["--help"]),
        s(&["-h"]),
        s(&["--USAGE"]),
        s(&["-u"]),
        s(&["--version"]),
        s(&["-all-info"]),
        s(&["--foo"]),
        s(&["-"]),
        s(&["---x"]),
        s(&["t1.mgz"]),
        s(&["--mov", "t1.mgz", "t2.mgz"]),
        s(&["--template", "tp.mgz"]),
        s(&["--mov", "--template", "tp.mgz"]),
        two(&[]),
        two(&["--sat", "0"]),
        two(&["--noit", "--ixforms", "t1.lta", "t2.lta"]),
        two(&["--satit", "--lta", "a.lta"]),
        two(&["--satit", "--mapmov", "a.mgz"]),
        two(&["--satit", "--mapmovhdr", "a.mgz"]),
        two(&["--satit", "--weights", "a.mgz"]),
        two(&["--satit", "--iscaleout", "a.txt"]),
        two(&["--satit", "--ixforms", "a.lta"]),
        two(&["--satit", "--iscalein", "s09.txt"]),
        // missing arguments (FS-1080)
        s(&["--mov"]),
        s(&["--template"]),
        s(&["--lta"]),
        two(&["--satit", "--average"]),
        two(&["--satit", "--conform"]),
        two(&["--satit", "--inittp"]),
        two(&["--satit", "--masks"]),
        two(&["--satit", "--threads"]),
        two(&["--satit", "--test"]),
        // the files that do not read
        s(&[
            "--mov",
            "gone.mgz",
            "t2.mgz",
            "--template",
            "tp.mgz",
            "--satit",
        ]),
        s(&[
            "--mov",
            "t1.mgz",
            "gone.mgz",
            "--template",
            "tp.mgz",
            "--satit",
        ]),
        s(&["--mov", "t1.mgz", "--template", "tp.mgz", "--satit"]),
        s(&["--mov", "gone.mgz", "--template", "tp.mgz", "--satit"]),
        s(&[
            "--mov",
            "t1.mgz",
            "f4.mgz",
            "--template",
            "tp.mgz",
            "--satit",
        ]),
        two(&["--satit", "--masks", "gone.mgz", "mask2.mgz"]),
        two(&["--satit", "--masks", "mask1.mgz", "f4.mgz"]),
        two(&["--satit", "--masks", "mask1.mgz"]),
        two(&["--satit", "--ixforms", "gone.lta", "t2.lta"]),
        two(&["--satit", "--ixforms", "bad.lta", "bad.lta"]),
        two(&["--satit", "--ixforms", "nogeom.lta", "nogeom.lta"]),
        two(&[
            "--satit",
            "--fixtp",
            "--inittp",
            "1",
            "--iscalein",
            "gone.txt",
            "x.txt",
        ]),
        // the paths of the template
        fmriprep(2),
        fmriprep(3),
        reconall(2),
        reconall(3),
        two(&["--satit"]),
        tri(&["--satit", "--noit"]),
        two(&["--satit", "--seed", "7"]),
        two(&["--satit", "--seed", "-5", "--noit"]),
        two(&["--satit", "--inittp", "2"]),
        two(&["--satit", "--inittp", "3"]),
        two(&["--satit", "--inittp", "-4"]),
        two(&["--satit", "--inittp", "0"]),
        two(&["--satit", "--inittp", "0", "--iscaleonly"]),
        tri(&["--satit", "--inittp", "0", "--noit"]),
        two(&["--satit", "--fixtp", "--inittp", "1", "--average", "0"]),
        tri(&[
            "--satit",
            "--fixtp",
            "--inittp",
            "1",
            "--noit",
            "--average",
            "0",
        ]),
        tri(&[
            "--satit",
            "--fixtp",
            "--inittp",
            "2",
            "--noit",
            "--average",
            "1",
        ]),
        two(&["--satit", "--fixtp", "--inittp", "1", "--average", "2"]),
        s(&[
            "--mov",
            "sm1.mgz",
            "sm2.mgz",
            "--template",
            "tp.mgz",
            "--sat",
            "4",
            "--noit",
            "--ixforms",
            "identity.nofile",
            "identity.nofile",
            "--average",
            "3",
        ]),
        s(&[
            "--mov",
            "sm1.mgz",
            "sm2.mgz",
            "--template",
            "tp.mgz",
            "--sat",
            "4",
            "--noit",
            "--ixforms",
            "identity.nofile",
            "identity.nofile",
            "--average",
            "2",
        ]),
        two(&["--satit", "--fixtp", "--inittp", "1", "--average", "7"]),
        two(&["--satit", "--inittp", "1", "--iscale"]),
        two(&["--satit", "--inittp", "1", "--cras"]),
        two(&[
            "--satit",
            "--inittp",
            "1",
            "--frobnorm-thresh",
            "0.00000001",
        ]),
        two(&[
            "--satit", "--fixtp", "--inittp", "1", "--affine", "--lta", "a1.lta", "a2.lta",
        ]),
        two(&["--satit", "--inittp", "1", "--affine"]),
        two(&[
            "--satit",
            "--inittp",
            "1",
            "--iscaleonly",
            "--iscaleout",
            "i1.txt",
            "i2.txt",
        ]),
        two(&["--satit", "--fixtp", "--inittp", "1", "--transonly"]),
        two(&[
            "--satit",
            "--fixtp",
            "--inittp",
            "1",
            "--nomulti",
            "--maxit",
            "3",
        ]),
        two(&[
            "--sat",
            "6",
            "--fixtp",
            "--inittp",
            "1",
            "--leastsquares",
            "--weights",
            "w1.mgz",
            "w2.mgz",
        ]),
        two(&["--sat", "4.685", "--fixtp", "--inittp", "1", "--doubleprec"]),
        two(&["--satit", "--fixtp", "--inittp", "1", "--subsample", "8"]),
        two(&["--satit", "--fixtp", "--inittp", "1", "--subsample", "-1"]),
        two(&["--satit", "--fixtp", "--inittp", "1", "--highit", "1"]),
        two(&["--satit", "--fixtp", "--inittp", "1", "--floattype"]),
        two(&["--satit", "--fixtp", "--inittp", "1", "--finalnearest"]),
        two(&[
            "--satit",
            "--fixtp",
            "--inittp",
            "1",
            "--masks",
            "mask1.mgz",
            "mask2.mgz",
        ]),
        two(&[
            "--satit",
            "--fixtp",
            "--inittp",
            "1",
            "--vox2vox",
            "--lta",
            "v1.lta",
            "v2.lta",
        ]),
        two(&[
            "--satit", "--fixtp", "--inittp", "1", "--mapmov", "w1.mgz", "w2.mgz",
        ]),
        tri(&[
            "--satit", "--fixtp", "--inittp", "1", "--noit", "--mapmov", "w4.mgz",
        ]),
        two(&[
            "--satit",
            "--fixtp",
            "--inittp",
            "1",
            "--warp",
            "w1.mgz",
            "w2.mgz",
            "--mapmovhdr",
            "h1.mgz",
            "h2.mgz",
        ]),
        two(&[
            "--satit",
            "--fixtp",
            "--inittp",
            "1",
            "--mapmovhdr",
            "h1.mgz",
            "nodir/h2.mgz",
        ]),
        two(&[
            "--satit",
            "--fixtp",
            "--inittp",
            "1",
            "--weights",
            "w1.mgz",
            "w2.mgz",
        ]),
        two(&["--satit", "--fixtp", "--inittp", "1", "--conform", "c.mgz"]),
        two(&[
            "--satit",
            "--fixtp",
            "--inittp",
            "1",
            "--iscalein",
            "s09.txt",
            "s11.txt",
        ]),
        two(&[
            "--satit",
            "--noit",
            "--ixforms",
            "identity.nofile",
            "identity.nofile",
        ]),
        two(&[
            "--satit",
            "--noit",
            "--ixforms",
            "identity.nofile",
            "identity.nofile",
            "--iscalein",
            "s12.txt",
            "sbad.txt",
            "--iscale",
        ]),
        two(&[
            "--satit",
            "--noit",
            "--ixforms",
            "identity.nofile",
            "identity.nofile",
            "--iscalein",
            "sempty.txt",
            "xd",
            "--iscale",
        ]),
        two(&["--satit", "--fixtp", "--inittp", "1", "--res-thresh", "0.5"]),
        two(&["--satit", "--fixtp", "--inittp", "1", "--threads", "1"]),
        two(&[
            "--satit",
            "--fixtp",
            "--inittp",
            "1",
            "--allow-diff-vox-size",
        ]),
        two(&["--satit", "--fixtp", "--inittp", "1", "--debug"]),
        two(&["--satit", "--inittp", "1", "--debug"]),
        two(&[
            "--satit",
            "--fixtp",
            "--inittp",
            "1",
            "--pairmaxit",
            "2",
            "--pairepsit",
            "0.1",
            "--maxit",
            "-1",
            "--epsit",
            "0",
        ]),
        // three inputs: the global iterations
        tri(&["--satit", "--inittp", "1"]),
        tri(&["--satit", "--inittp", "1", "--maxit", "2", "--epsit", "0.5"]),
        tri(&["--satit", "--inittp", "0", "--maxit", "1"]),
        tri(&[
            "--satit",
            "--fixtp",
            "--inittp",
            "2",
            "--maxit",
            "1",
            "--weights",
            "w1.mgz",
            "w2.mgz",
            "w3.mgz",
        ]),
        tri(&["--satit", "--inittp", "1", "--maxit", "1", "--finalnearest"]),
        tri(&[
            "--sat",
            "4",
            "--inittp",
            "0",
            "--maxit",
            "2",
            "--iscaleonly",
        ]),
        tri(&["--satit", "--inittp", "1", "--maxit", "1", "--debug"]),
        tri(&[
            "--sat",
            "6",
            "--inittp",
            "1",
            "--maxit",
            "1",
            "--leastsquares",
            "--weights",
            "w1.mgz",
            "w2.mgz",
            "w3.mgz",
        ]),
        // inputs of another geometry
        s(&[
            "--mov",
            "t1.mgz",
            "an.mgz",
            "--template",
            "tp.mgz",
            "--satit",
            "--inittp",
            "1",
        ]),
        s(&[
            "--mov",
            "t1.mgz",
            "an.mgz",
            "--template",
            "tp.mgz",
            "--satit",
            "--inittp",
            "1",
            "--allow-diff-vox-size",
        ]),
        s(&[
            "--mov",
            "t1.mgz",
            "ras.mgz",
            "--template",
            "tp.mgz",
            "--satit",
            "--inittp",
            "1",
        ]),
        s(&[
            "--mov",
            "t1.mgz",
            "ras.mgz",
            "lpi.mgz",
            "--template",
            "tp.mgz",
            "--satit",
            "--inittp",
            "1",
            "--noit",
        ]),
        s(&[
            "--mov",
            "t1.mgz",
            "fl.mgz",
            "--template",
            "tp.mgz",
            "--satit",
            "--inittp",
            "2",
            "--fixtp",
        ]),
        s(&[
            "--mov",
            "d2a.mgz",
            "d2b.mgz",
            "--template",
            "tp.mgz",
            "--satit",
            "--inittp",
            "1",
        ]),
        s(&[
            "--mov",
            "t1.mgz",
            "d2b.mgz",
            "--template",
            "tp.mgz",
            "--satit",
            "--inittp",
            "1",
            "--allow-diff-vox-size",
        ]),
        s(&[
            "--mov",
            "e1.mgz",
            "e2.mgz",
            "--template",
            "tp.mgz",
            "--sat",
            "4",
            "--noit",
            "--ixforms",
            "identity.nofile",
            "identity.nofile",
        ]),
        s(&[
            "--mov",
            "e1.mgz",
            "e2.mgz",
            "--template",
            "tp.mgz",
            "--sat",
            "4",
            "--noit",
            "--inittp",
            "1",
            "--fixtp",
        ]),
        // the 4-D input
        s(&[
            "--mov",
            "f4.mgz",
            "--template",
            "tp.mgz",
            "--satit",
            "--fixtp",
            "--inittp",
            "1",
            "--lta",
            "a.lta",
            "--mapmov",
            "w.mgz",
        ]),
        s(&[
            "--mov",
            "f4.mgz",
            "--template",
            "tp.mgz",
            "--satit",
            "--noit",
        ]),
        s(&[
            "--mov",
            "f4.mgz",
            "--template",
            "tp.mgz",
            "--satit",
            "--fixtp",
            "--inittp",
            "2",
            "--masks",
            "m4.mgz",
        ]),
        s(&[
            "--mov",
            "f4.mgz",
            "--template",
            "tp.mgz",
            "--satit",
            "--masks",
            "m2f.mgz",
        ]),
        s(&[
            "--mov",
            "f4.mgz",
            "--template",
            "tp.mgz",
            "--satit",
            "--masks",
            "gone.mgz",
        ]),
        s(&[
            "--mov",
            "f4.mgz",
            "--template",
            "tp.mgz",
            "--satit",
            "--masks",
            "m4.mgz",
            "m4.mgz",
        ]),
        s(&[
            "--mov",
            "f4.mgz",
            "--template",
            "tp.mgz",
            "--satit",
            "--noit",
            "--ixforms",
            "identity.nofile",
        ]),
        // the outputs' names
        s(&[
            "--mov",
            "t1.mgz",
            "t2.mgz",
            "--template",
            "out/tp.mgz",
            "--satit",
            "--fixtp",
            "--inittp",
            "1",
            "--debug",
        ]),
        s(&[
            "--mov",
            "t1.mgz",
            "t2.mgz",
            "--template",
            "nodir/tp.mgz",
            "--satit",
            "--fixtp",
            "--inittp",
            "1",
            "--lta",
            "nodir/a.lta",
            "x.lta",
            "--iscaleout",
            "nodir/i.txt",
            "i2.txt",
        ]),
        s(&[
            "-MOV",
            "t1.mgz",
            "t2.mgz",
            "--TEMPLATE",
            "tp.mgz",
            "--SATIT",
            "-I",
            "-A",
            "--FixTP",
            "--inittp",
            "1",
        ]),
        // the test mode
        s(&["--test", "1", "t1.mgz"]),
    ];
    for c in &mut cases {
        // no case may make FreeSurfer's licence check print (no diagnostic level is ever set)
        assert!(!c.iter().any(|a| a.contains("DIAG")));
    }
    cases
        .iter()
        .map(|a| line(None, &files, &["xd", "out"], a))
        .collect()
}

/// One seeded case: 2 to 4 inputs of a random type, size and geometry, and random options.
fn seeded(g: &mut Gen) -> String {
    let k = g.pick(&[2usize, 2, 2, 3, 3, 4]);
    let n = g.pick(&[16, 16, 17, 18]);
    let ty = g.pick(&[MRI_UCHAR, MRI_UCHAR, MRI_SHORT, MRI_FLOAT, MRI_INT]);
    let mut imgs = set(g, k, n, ty);
    if g.chance(6) {
        for im in imgs.iter_mut().skip(1) {
            im.mdc = g.pick(&[LIA, RAS, LPI]);
        }
    }
    if g.chance(8) {
        imgs[k - 1].ty = g.pick(&[MRI_UCHAR, MRI_FLOAT, MRI_SHORT]);
    }
    let ext = g.pick(&[".mgz", ".mgz", ".mgz", ".nii.gz", ".nii"]);
    let mut files = files_of(g, &imgs, ext);
    let mut args = s(&["--mov"]);
    args.extend(names(k, "t", ext));
    args.push("--template".to_owned());
    args.push(
        g.pick(&["tp.mgz", "tp.mgz", "tp.nii.gz", "tp.nii"])
            .to_owned(),
    );
    if g.chance(4) {
        args.extend(s(&["--sat", g.pick(&["2", "4.685", "8"])]));
    } else {
        args.push("--satit".to_owned());
    }
    let fixed = g.chance(2);
    if fixed {
        args.extend(s(&["--fixtp", "--inittp"]));
        args.push(g.int(1, k as i64).to_string());
    } else if g.chance(3) {
        args.extend(s(&["--inittp"]));
        args.push(g.int(0, k as i64).to_string());
    } else if g.chance(2) {
        args.extend(s(&["--seed"]));
        args.push(g.int(1, 99).to_string());
    }
    let noit = k == 2 || g.chance(2);
    if noit {
        args.push("--noit".to_owned());
    } else {
        args.extend(s(&["--maxit", g.pick(&["1", "2"]), "--epsit", "0.1"]));
    }
    let mut opt = |g: &mut Gen, kk: u64, a: &[&str]| {
        if g.chance(kk) {
            args.extend(s(a));
        }
    };
    opt(g, 2, &["--iscale"]);
    if fixed {
        opt(g, 6, &["--affine"]);
    }
    opt(g, 12, &["--transonly"]);
    opt(g, 8, &["--nomulti"]);
    opt(g, 8, &["--doubleprec"]);
    opt(g, 10, &["--floattype"]);
    opt(g, 6, &["--subsample", "12"]);
    opt(g, 10, &["--highit", "2"]);
    opt(g, 8, &["--pairmaxit", "3"]);
    opt(g, 8, &["--average", "0"]);
    opt(g, 8, &["--finalnearest"]);
    opt(g, 10, &["--cras"]);
    opt(g, 10, &["--vox2vox"]);
    if g.chance(2) {
        args.push("--lta".to_owned());
        args.extend(names(k, "o", ".lta"));
    }
    if g.chance(4) {
        args.push("--mapmov".to_owned());
        if g.chance(3) {
            args.push("w4.mgz".to_owned());
        } else {
            args.extend(names(k, "w", ".mgz"));
        }
    }
    if g.chance(8) {
        args.push("--mapmovhdr".to_owned());
        args.extend(names(k, "h", ".mgz"));
    }
    if g.chance(4) {
        args.push("--iscaleout".to_owned());
        args.extend(names(k, "i", ".txt"));
    }
    if g.chance(6) {
        args.push("--weights".to_owned());
        args.extend(names(k, "wt", ".mgz"));
        if g.chance(2) {
            args.push("--oneminusw".to_owned());
        }
    }
    if g.chance(8) {
        let mut m = imgs[0];
        m.mask = true;
        args.push("--masks".to_owned());
        for i in 1..=k {
            let name = format!("mk{i}.mgz");
            let b = write_as(&mut image(g, &m), &name);
            files.push((name.clone(), b));
            args.push(name);
        }
    }
    line(None, &files, &[], &args)
}

/// The prelude, then seeded cases up to `n`.
pub fn generate(seed: u64, n: usize) -> Vec<String> {
    let mut out = prelude();
    let mut g = Gen::new(seed);
    while out.len() < n {
        out.push(seeded(&mut g));
    }
    out.truncate(n);
    out
}

/// `LIA`'s axes rotated by the unit quaternion `(1, w v)` normalised (`v` from `g`; about `2 w`
/// radians; square roots only, so every platform makes the same headers).
fn rotated_lia(g: &mut Gen, w: f64) -> [f32; 9] {
    let v: [f64; 3] = std::array::from_fn(|_| g.int(-1000, 1000) as f64 / 1000.0);
    let nv = v.iter().map(|x| x * x).sum::<f64>().sqrt().max(1e-3);
    let q = [1.0, w * v[0] / nv, w * v[1] / nv, w * v[2] / nv];
    let n = q.iter().map(|x| x * x).sum::<f64>().sqrt();
    let [a, b, c, d] = q.map(|x| x / n);
    let r = [
        [
            a * a + b * b - c * c - d * d,
            2.0 * (b * c - a * d),
            2.0 * (b * d + a * c),
        ],
        [
            2.0 * (b * c + a * d),
            a * a - b * b + c * c - d * d,
            2.0 * (c * d - a * b),
        ],
        [
            2.0 * (b * d - a * c),
            2.0 * (c * d + a * b),
            a * a - b * b - c * c + d * d,
        ],
    ];
    let mut out = [0.0f32; 9];
    for axis in 0..3 {
        for i in 0..3 {
            out[axis * 3 + i] = (0..3)
                .map(|k| r[i][k] * f64::from(LIA[axis * 3 + k]))
                .sum::<f64>() as f32;
        }
    }
    out
}

/// One case of the affine mean space (`--affine` without `--fixtp`, queue 3.2-15): two to four
/// inputs whose headers are rotated (small angles, and some past the logarithm's 0.1 blocking:
/// its separate eigenvalues), with the program's other options.
fn affine_case(g: &mut Gen) -> String {
    let k = g.pick(&[2usize, 2, 3, 3, 4]);
    let n = g.pick(&[16, 16, 17]);
    let ty = g.pick(&[MRI_UCHAR, MRI_UCHAR, MRI_SHORT, MRI_FLOAT]);
    let mut imgs = set(g, k, n, ty);
    // the inputs' voxel sizes stay equal (the program refuses others: `images have different
    // voxel sizes`)
    for im in imgs.iter_mut().skip(1) {
        if !g.chance(4) {
            let w = g.pick(&[0.005, 0.02, 0.04, 0.08, 0.15]);
            im.mdc = rotated_lia(g, w);
        }
    }
    let ext = g.pick(&[".mgz", ".mgz", ".nii.gz"]);
    let files = files_of(g, &imgs, ext);
    let mut args = s(&["--mov"]);
    args.extend(names(k, "t", ext));
    args.extend(s(&["--template", "tp.mgz", "--affine"]));
    if g.chance(3) {
        args.extend(s(&["--sat", g.pick(&["4.685", "8"])]));
    } else {
        args.push("--satit".to_owned());
    }
    if g.chance(2) {
        args.push("--inittp".to_owned());
        args.push(g.int(1, k as i64).to_string());
    } else {
        args.push("--seed".to_owned());
        args.push(g.int(1, 99).to_string());
    }
    if g.chance(2) {
        args.push("--noit".to_owned());
    } else {
        args.extend(s(&["--maxit", "1", "--epsit", "0.1"]));
    }
    let mut opt = |g: &mut Gen, kk: u64, a: &[&str]| {
        if g.chance(kk) {
            args.extend(s(a));
        }
    };
    opt(g, 2, &["--iscale"]);
    opt(g, 4, &["--average", "0"]);
    opt(g, 6, &["--nomulti"]);
    opt(g, 8, &["--doubleprec"]);
    opt(g, 8, &["--cras"]);
    opt(g, 8, &["--vox2vox"]);
    opt(g, 8, &["--finalnearest"]);
    if g.chance(2) {
        args.push("--lta".to_owned());
        args.extend(names(k, "o", ".lta"));
    }
    if g.chance(4) {
        args.push("--mapmov".to_owned());
        args.extend(names(k, "w", ".mgz"));
    }
    if g.chance(4) {
        args.push("--iscaleout".to_owned());
        args.extend(names(k, "i", ".txt"));
    }
    line(None, &files, &[], &args)
}

/// The affine mean space's cases of `seed` (`fs_robtmpl_probe cases-affine`).
pub fn generate_affine(seed: u64, n: usize) -> Vec<String> {
    let mut g = Gen::new(seed ^ 0x6166_6669);
    (0..n).map(|_| affine_case(&mut g)).collect()
}

/// The golden cases (`tests/mri_robust_template_golden.rs`): fMRIPrep's and recon-all's calls on
/// 32^3 inputs, the mean space of two, the global iterations of three, a NaN input (its NaNs made 0
/// on reading) and a 4-D input of two frames with one `--mapmov` name (FS-1090), each written as MGZ.
pub fn golden_cases() -> Vec<String> {
    // 16^3 (the registrations' smallest size; one pyramid level, as at 32^3) keeps the debug
    // build's test fast; the seed gives the same converged and not-converged pairs as 32^3 did
    let mut g = Gen::new(1);
    let three = set(&mut g, 3, 16, MRI_SHORT);
    let mut files = files_of(&mut g, &three, ".mgz");
    // the NaN input: the second's voxels with some made NaN from their bits
    let mut nan_img = image(&mut g, &{
        let mut b = three[1];
        b.ty = MRI_FLOAT;
        b
    });
    {
        let mut fs = Globals::new(UpstreamDefects::Fix);
        let mut e: Vec<u8> = Vec::new();
        for (k, (x, y, z)) in [(3, 4, 5), (16, 16, 16), (20, 9, 30), (0, 0, 0)]
            .iter()
            .enumerate()
        {
            let bits = [0x7fc0_0000u32, 0x7f80_0001, 0xffc0_0001, 0x7f80_0000][k];
            larmorx_freesurfer_mri::voxel::mri_set_vox_val(
                &mut fs,
                &mut e,
                &mut nan_img,
                *x,
                *y,
                *z,
                0,
                f32::from_bits(bits),
                0,
            )
            .expect("a voxel");
        }
    }
    files.push(("tnan.mgz".to_owned(), write_as(&mut nan_img, "tnan.mgz")));
    // a 4-D input of two frames (FS-1090: its one --mapmov name's header)
    let mut four = three[0];
    four.nframes = 2;
    files.push((
        "f4.mgz".to_owned(),
        write_as(&mut image(&mut g, &four), "f4.mgz"),
    ));
    let fmriprep = s(&[
        "--satit",
        "--fixtp",
        "--mov",
        "t1.mgz",
        "t2.mgz",
        "t3.mgz",
        "--inittp",
        "1",
        "--iscale",
        "--noit",
        "--template",
        "tp.mgz",
        "--subsample",
        "200",
        "--lta",
        "a1.lta",
        "a2.lta",
        "a3.lta",
    ]);
    let reconall = s(&[
        "--mov",
        "t1.mgz",
        "t2.mgz",
        "--average",
        "1",
        "--template",
        "rawavg.mgz",
        "--satit",
        "--inittp",
        "1",
        "--fixtp",
        "--noit",
        "--iscale",
        "--iscaleout",
        "i1.txt",
        "i2.txt",
        "--subsample",
        "200",
        "--lta",
        "a1.lta",
        "a2.lta",
    ]);
    let mean_space = s(&[
        "--mov",
        "t1.mgz",
        "t3.mgz",
        "--template",
        "tp.mgz",
        "--satit",
        "--inittp",
        "2",
        "--lta",
        "a1.lta",
        "a2.lta",
        "--mapmov",
        "w1.mgz",
        "w2.mgz",
    ]);
    let four_d = s(&[
        "--mov",
        "f4.mgz",
        "--template",
        "tp.mgz",
        "--satit",
        "--fixtp",
        "--inittp",
        "1",
        "--noit",
        "--mapmov",
        "w4.mgz",
    ]);
    let iterations = s(&[
        "--mov",
        "t1.mgz",
        "t2.mgz",
        "t3.mgz",
        "--template",
        "tp.mgz",
        "--satit",
        "--inittp",
        "1",
        "--maxit",
        "1",
        "--iscale",
        "--weights",
        "w1.mgz",
        "w2.mgz",
        "w3.mgz",
    ]);
    let nan = s(&[
        "--mov",
        "t1.mgz",
        "tnan.mgz",
        "--template",
        "tp.mgz",
        "--satit",
        "--fixtp",
        "--inittp",
        "1",
        "--average",
        "0",
        "--lta",
        "a1.lta",
        "a2.lta",
    ]);
    // the affine mean space (3.2-15): three inputs, two with rotated headers (one past the
    // logarithm's blocking, one within it)
    let mut rot = three.clone();
    rot[1].mdc = rotated_lia(&mut g, 0.08);
    rot[2].mdc = rotated_lia(&mut g, 0.02);
    for (i, im) in rot.iter().enumerate() {
        let name = format!("r{}.mgz", i + 1);
        files.push((name.clone(), write_as(&mut image(&mut g, im), &name)));
    }
    let affine = s(&[
        "--mov",
        "r1.mgz",
        "r2.mgz",
        "r3.mgz",
        "--template",
        "tp.mgz",
        "--affine",
        "--satit",
        "--inittp",
        "1",
        "--noit",
        "--iscale",
        "--lta",
        "a1.lta",
        "a2.lta",
        "a3.lta",
    ]);
    [
        fmriprep, reconall, mean_space, iterations, nan, four_d, affine,
    ]
    .iter()
    .map(|a| line(None, &files, &[], a))
    .collect()
}

fn is_text(name: &str) -> bool {
    [".lta", ".xfm", ".dat", ".txt"]
        .iter()
        .any(|e| name.ends_with(e))
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

/// `b` with every `d` replaced by `<DIR>`.
fn mask_bytes(b: &[u8], d: &[u8]) -> Vec<u8> {
    if d.is_empty() {
        return b.to_vec();
    }
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i..].starts_with(d) {
            out.extend_from_slice(b"<DIR>");
            i += d.len();
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    out
}

fn fnv(b: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &c in b {
        h = (h ^ u64::from(c)).wrapping_mul(0x0100_0000_01b3);
    }
    h
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

/// `-all-info`'s run-time fields (`TimeStamp`, `User`, `Machine`, `PlatformVersion`: each value
/// up to the next blank as `*`, fs-temp-file's rule).
fn mask_info(s: &str) -> String {
    let mut out = s.to_owned();
    for key in ["TimeStamp: ", "User: ", "Machine: ", "PlatformVersion: "] {
        let mut from = 0;
        while let Some(at) = out[from..].find(key) {
            let start = from + at + key.len();
            let end = out[start..].find(' ').map_or(out.len(), |e| start + e);
            out.replace_range(start..end, "*");
            from = start + 1;
        }
    }
    out
}

/// The line of the time taken, masked: `registration took <T> minutes and <T> seconds.`; and
/// `-all-info`'s run-time fields.
pub fn mask_time(s: &str) -> String {
    let s = &mask_info(s);
    let head = "registration took ";
    let Some(at) = s.find(head) else {
        return s.to_owned();
    };
    let rest = &s[at + head.len()..];
    let Some(end) = rest.find(" seconds.") else {
        return s.to_owned();
    };
    format!(
        "{}registration took <T> minutes and <T> seconds.{}",
        &s[..at],
        &rest[end + " seconds.".len()..]
    )
}

/// Builds a case's folder (`dir`: empty) and returns its arguments, environment and input names.
fn setup(line: &str, dir: &Path) -> (Vec<String>, Vec<(String, String)>, Vec<String>) {
    let t: Vec<&str> = line.split_whitespace().collect();
    assert_eq!(t[0], "rtpl");
    let mut env = rreg::probe_env();
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
            input: None,
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
    // an abort (SIGABRT) and a segmentation fault (SIGSEGV) are the official child's `CRASH 6` and
    // `CRASH 11`; the port's statuses 134 and 139 stand for them
    let end = match status {
        134 => "CRASH 6".to_owned(),
        139 => "CRASH 11".to_owned(),
        s => format!("EXIT {s}"),
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
