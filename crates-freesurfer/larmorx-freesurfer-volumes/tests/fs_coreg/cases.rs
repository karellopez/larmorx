// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The fs-coreg probe's cases (`validation/probes/fs-coreg/`): a seeded generator of `mri_coreg`
//! command lines with the files they read, and the port's answer to each, printed as `check.py`
//! prints the official program's (`<status> | <stdout> | <stderr> | <files>`, C-escaped; the
//! times and the machine's names masked).
//!
//! A case line is `coreg <env> <nfiles> [<name> <hex>]... <ndirs> [<hex>]... <nargs> [<hex>]...`
//! (`<env>`: `-` or the hex of `NAME=value` assignments joined by `;`, `@DIR@` the case's
//! folder). Every case runs with `FREESURFER_HOME` a folder of the case holding a small colour
//! table (`FSENVgetenv` reads it) unless its environment says otherwise. The volumes are
//! `fs_robreg/cases.rs`' head-like patterns (a bright ellipsoid with a darker shell, a box,
//! seeded noise): a "BOLD" moving volume of 2.4 mm voxels moved and sheared from the reference,
//! a 1.5 mm "brain" reference, masks and segmentations of the same patterns.
#![allow(dead_code)]

#[path = "../fs_robreg/cases.rs"]
mod rreg;

use std::path::Path;

use larmorx_core::defects::UpstreamDefects;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_mri::mri::{MRI_FLOAT, MRI_INT, MRI_SHORT, MRI_UCHAR};
use larmorx_freesurfer_volumes::mri_coreg::main::run_with;

pub use rreg::{Gen, Img, escape, hex, image, unhex, write_as};

/// LIA, RAS and LPI orientations.
const LIA: [f32; 9] = [-1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 1.0, 0.0];
const RAS: [f32; 9] = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];
const LPI: [f32; 9] = [-1.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, -1.0];

/// The colour table `FSENVgetenv` reads (`$FREESURFER_HOME/FreeSurferColorLUT.txt`).
const LUT: &[u8] = b"0 Unknown 0 0 0 0\n2 Left-Cerebral-White-Matter 245 245 245 0\n3 Left-Cerebral-Cortex 205 62 78 0\n41 Right-Cerebral-White-Matter 0 225 0 0\n42 Right-Cerebral-Cortex 205 62 78 0\n";

/// The base environment of every case (both sides): the probe's (one thread), the LTA's creation
/// line fixed (the user, the time FreeSurfer's override names).
pub fn probe_env() -> Vec<(String, String)> {
    rreg::probe_env()
}

/// A case line: the environment's assignments, the files (name, bytes), the folders and the
/// arguments. `FREESURFER_HOME=@DIR@/fsh` (with its colour table) unless `env` names one.
pub fn line(env: &[&str], files: &[(String, Vec<u8>)], dirs: &[&str], args: &[String]) -> String {
    let mut all: Vec<String> = Vec::new();
    let mut files: Vec<(String, Vec<u8>)> = files.to_vec();
    if !env.iter().any(|e| e.starts_with("FREESURFER_HOME=")) {
        all.push("FREESURFER_HOME=@DIR@/fsh".to_owned());
        files.push(("fsh/FreeSurferColorLUT.txt".to_owned(), LUT.to_vec()));
    }
    all.extend(env.iter().map(|e| (*e).to_owned()));
    let mut t = vec!["coreg".to_owned(), hex(all.join(";").as_bytes())];
    t.push(files.len().to_string());
    for (n, b) in &files {
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

pub fn s(a: &[&str]) -> Vec<String> {
    a.iter().map(|x| (*x).to_owned()).collect()
}

/// The text of an LTA with the matrix `m` (row by row), its type and the two volumes'
/// geometries (`(dims, voxel size, cosines (x, y, z columns), centre)`).
#[allow(clippy::type_complexity)]
pub fn lta_text(
    m: &[f64; 16],
    ltype: i32,
    src: ([i32; 3], [f32; 3], [f32; 9], [f32; 3], &str),
    dst: ([i32; 3], [f32; 3], [f32; 9], [f32; 3], &str),
) -> Vec<u8> {
    let tname = if ltype == 0 {
        "LINEAR_VOX_TO_VOX"
    } else {
        "LINEAR_RAS_TO_RAS"
    };
    let mut t = format!(
        "# transform file x.lta\ntype      = {ltype} # {tname}\nnxforms   = 1\nmean      = 0.0000 0.0000 0.0000\nsigma     = 1.0000\n1 4 4\n"
    );
    for r in 0..4 {
        for c in 0..4 {
            t.push_str(&format!("{:.15e} ", m[4 * r + c]));
        }
        t.push('\n');
    }
    for (what, v) in [("src", src), ("dst", dst)] {
        let (d, sz, mdc, c, name) = v;
        t.push_str(&format!(
            "{what} volume info\nvalid = 1  # volume info valid\nfilename = {name}\nvolume = {} {} {}\nvoxelsize = {:.15e} {:.15e} {:.15e}\nxras   = {:.15e} {:.15e} {:.15e}\nyras   = {:.15e} {:.15e} {:.15e}\nzras   = {:.15e} {:.15e} {:.15e}\ncras   = {:.15e} {:.15e} {:.15e}\n",
            d[0], d[1], d[2], sz[0], sz[1], sz[2], mdc[0], mdc[1], mdc[2], mdc[3], mdc[4], mdc[5],
            mdc[6], mdc[7], mdc[8], c[0], c[1], c[2]
        ));
    }
    t.push_str("subject unknown\nfscale 0.100000\n");
    t.into_bytes()
}

/// A registration pair: the "BOLD" moving image and the reference, and their geometry for LTAs.
#[derive(Clone, Copy)]
pub struct Pair {
    pub mov: Img,
    pub refv: Img,
}

impl Pair {
    /// The default pair: a 20 x 20 x 14 moving volume of 2.4 mm voxels (5,600: past FS-1320's
    /// 5,000), its pattern moved and sheared, onto a 32^3 reference of 1.5 mm voxels.
    pub fn base() -> Pair {
        let mut mov = Img::base(20, MRI_SHORT);
        mov.dims = [20, 20, 14];
        mov.size = [2.4, 2.4, 2.4];
        mov.shift = [3, -2, 1];
        mov.shear = 1;
        mov.gain = 22;
        mov.noise = 5;
        let mut refv = Img::base(32, MRI_UCHAR);
        refv.size = [1.5, 1.5, 1.5];
        Pair { mov, refv }
    }

    /// `(dims, size, mdc, c, name)` of an image.
    #[allow(clippy::type_complexity)]
    pub fn geom(i: &Img, name: &'static str) -> ([i32; 3], [f32; 3], [f32; 9], [f32; 3], &'static str) {
        (i.dims, i.size, i.mdc, i.c, name)
    }

    /// The files: `mov.mgz`, `ref.mgz`.
    pub fn files(&self, g: &mut Gen) -> Vec<(String, Vec<u8>)> {
        vec![
            ("mov.mgz".to_owned(), write_as(&mut image(g, &self.mov), "mov.mgz")),
            ("ref.mgz".to_owned(), write_as(&mut image(g, &self.refv), "ref.mgz")),
        ]
    }
}

/// A rigid transform's LTA (RAS to RAS) between the pair's geometries: a small rotation about z
/// and a shift.
pub fn init_lta(p: &Pair, ltype: i32) -> Vec<u8> {
    let (c, sn) = (0.999_390_827_019_095_7, 0.034_899_496_702_500_97);
    let m = [
        c, -sn, 0.0, 1.5, sn, c, 0.0, -0.75, 0.0, 0.0, 1.0, 0.5, 0.0, 0.0, 0.0, 1.0,
    ];
    lta_text(&m, ltype, Pair::geom(&p.mov, "mov.mgz"), Pair::geom(&p.refv, "ref.mgz"))
}

/// A segmentation of the reference's pattern (aparc+aseg-like: 0 outside, 2, 3, 41, 42 inside).
fn seg_bytes(g: &mut Gen, refv: &Img, name: &str) -> Vec<u8> {
    let mut m = *refv;
    m.mask = true;
    m.ty = MRI_INT;
    let mut v = image(g, &m);
    // the mask's 1s split by side into the labels
    let mut gg = Globals::new(UpstreamDefects::Fix);
    let mut e = Vec::new();
    for z in 0..m.dims[2] {
        for y in 0..m.dims[1] {
            for x in 0..m.dims[0] {
                let val = larmorx_freesurfer_mri::voxel::mri_get_vox_val(&mut gg, &mut e, &v, x, y, z, 0, 0)
                    .expect("a voxel");
                if val > 0.0 {
                    let lab = match (x < m.dims[0] / 2, z % 3 == 0) {
                        (true, true) => 2.0,
                        (true, false) => 3.0,
                        (false, true) => 41.0,
                        (false, false) => 42.0,
                    };
                    larmorx_freesurfer_mri::voxel::mri_set_vox_val(&mut gg, &mut e, &mut v, x, y, z, 0, lab, 0)
                        .expect("a voxel");
                }
            }
        }
    }
    write_as(&mut v, name)
}

/// A mask of an image's pattern (1 inside, 0 outside) as `ty`; `half`: 0.5 inside (masked out
/// by FreeSurfer's integer test).
fn mask_bytes_of(g: &mut Gen, im: &Img, ty: i32, name: &str) -> Vec<u8> {
    let mut m = *im;
    m.mask = true;
    m.ty = ty;
    write_as(&mut image(g, &m), name)
}

/// The prelude: the command line's own paths and failures, the stand-alone options, and one
/// case of each registration option on the base pair.
pub fn prelude() -> Vec<String> {
    let mut g = Gen::new(0x636f_7265);
    let p = Pair::base();
    let files = p.files(&mut g);
    let lta_ras = init_lta(&p, 1);
    let lta_vox = init_lta(&p, 0);
    let mut cases: Vec<String> = Vec::new();
    let req = s(&["--mov", "mov.mgz", "--ref", "ref.mgz", "--reg", "reg.lta"]);
    let with = |extra: &[&str]| -> Vec<String> {
        let mut a = req.clone();
        a.extend(extra.iter().map(|x| (*x).to_owned()));
        a
    };
    // the command line
    for a in [
        s(&[]),
        s(&["--help"]),
        s(&["--version"]),
        s(&["--dof", "9", "--nitersmax", "7", "--help"]),
        s(&["--foo"]),
        s(&["-foo"]),
        s(&["--mov"]),
        s(&["--trans", "1", "2"]),
        s(&["--rot", "1"]),
        s(&["--par2mat", "1", "2", "3"]),
        s(&["--mat2rot", "a.lta"]),
        s(&["--lrrev", "a.lta"]),
        s(&["--rms", "50"]),
        s(&["--landmarks", "a", "b", "c"]),
        s(&["--ref", "ref.mgz", "--reg", "reg.lta"]),
        s(&["--mov", "mov.mgz", "--reg", "reg.lta"]),
        s(&["--mov", "mov.mgz", "--ref", "ref.mgz"]),
        with(&["--checkopts"]),
        with(&["--checkopts", "--nocheckopts", "--no-bf", "--nitersmax", "1", "--sep", "4"]),
        with(&["--debug", "--checkopts"]),
        s(&["--mov", "nofile.mgz", "--ref", "ref.mgz", "--reg", "reg.lta"]),
        s(&["--mov", "mov.mgz", "--ref", "nofile.mgz", "--reg", "reg.lta"]),
        with(&["--NO-BF", "--Sep", "4", "--NITERSMAX", "1"]),
        with(&["--No-Dither", "--checkopts"]),
    ] {
        cases.push(line(&[], &files, &[], &a));
    }
    // the stand-alone options
    let mut lfiles = files.clone();
    lfiles.push(("a.lta".to_owned(), lta_ras.clone()));
    lfiles.push(("v.lta".to_owned(), lta_vox.clone()));
    for a in [
        s(&["--mat2par", "a.lta"]),
        s(&["--mat2par-vox", "a.lta"]),
        s(&["--mat2par", "v.lta"]),
        s(&["--mat2par", "nofile.lta"]),
        s(&["--mat2rot", "a.lta", "rot.lta"]),
        s(&["--mat2rot", "v.lta", "rot.lta"]),
        s(&["--par2mat", "1", "-2", "3", "4", "5", "6", "1", "1.1", "0.9", "0", "0.1", "0", "mov.mgz", "ref.mgz", "p.lta"]),
        s(&["--s", "bert", "--par2mat", "0", "0", "0", "10", "0", "0", "1", "1", "1", "0", "0", "0", "mov.mgz", "ref.mgz", "p.lta"]),
        s(&["--par2mat", "1", "x", "3", "4", "5", "6", "1", "1", "1", "0", "0", "0", "mov.mgz", "ref.mgz", "p.lta"]),
        s(&["--par2mat", "1", "2", "3", "4", "5", "6", "1", "1", "1", "0", "0", "0", "nofile.mgz", "ref.mgz", "p.lta"]),
        s(&["--lrrev", "a.lta", "lr.lta"]),
        s(&["--lrrev", "v.lta", "lr.lta"]),
        s(&["--rms", "50", "rms.txt", "a.lta", "v.lta"]),
        s(&["--rms", "50", "nofile", "a.lta", "a.lta"]),
        s(&["--rms", "x50", "rms.txt", "a.lta", "v.lta"]),
        s(&["--rms", "-3", "rms.txt", "a.lta", "v.lta"]),
        s(&["--rms", "0", "rms.txt", "a.lta", "v.lta"]),
        s(&["--rms", "50", "rms.txt", "a.lta", "nofile.lta"]),
        s(&["--landmarks", "a", "b", "c", "d", "e", "f"]),
        s(&["--init-reg", "nofile.lta"]),
        s(&["--mov", "mov.mgz", "--ref", "ref.mgz", "--reg", "reg.lta", "--init-reg-save-only", "i.lta"]),
        s(&["--mov", "mov.mgz", "--ref", "ref.mgz", "--reg", "reg.lta", "--init-reg", "a.lta", "--init-reg-save-only", "i.lta"]),
        s(&["--mov", "mov.mgz", "--ref", "ref.mgz", "--reg", "reg.lta", "--trans", "1", "2", "3", "--rot", "4", "5", "6", "--scale", "1.1", "1", "0.9", "--shear", "0.1", "0", "0", "--init-reg-save-only", "i.lta"]),
        s(&["--mov", "mov.mgz", "--ref", "ref.mgz", "--reg", "reg.lta", "--centroid", "--init-reg-save-only", "i.lta"]),
        s(&["--mov", "mov.mgz", "--ref", "ref.mgz", "--reg", "reg.lta", "--no-cras0", "--init-reg-save-only", "i.lta"]),
    ] {
        cases.push(line(&[], &lfiles, &[], &a));
    }
    // the registrations (each with one Powell iteration at most and no brute force unless asked:
    // the brute force's 186 evaluations and the default's are in the seeded family)
    let quick = ["--no-bf", "--nitersmax", "1", "--sep", "4"];
    let q = |extra: &[&str]| -> Vec<String> {
        let mut a = req.clone();
        a.extend(quick.iter().map(|x| (*x).to_owned()));
        a.extend(extra.iter().map(|x| (*x).to_owned()));
        a
    };
    let mut rfiles = lfiles.clone();
    rfiles.push(("seg.mgz".to_owned(), seg_bytes(&mut g, &p.refv, "seg.mgz")));
    rfiles.push(("mmask.mgz".to_owned(), mask_bytes_of(&mut g, &p.mov, MRI_UCHAR, "mmask.mgz")));
    rfiles.push(("dith.mgz".to_owned(), mask_bytes_of(&mut g, &p.mov, MRI_FLOAT, "dith.mgz")));
    for a in [
        with(&[]),
        q(&[]),
        q(&["--dof", "9"]),
        q(&["--dof", "12"]),
        q(&["--12"]),
        q(&["--9", "--6"]),
        q(&["--zscale"]),
        q(&["--dof", "3"]),
        q(&["--no-dither"]),
        q(&["--no-coord-dither"]),
        q(&["--no-intensity-dither"]),
        q(&["--no-smooth"]),
        q(&["--ref-fwhm", "3"]),
        q(&["--sep", "2"]),
        q(&["--ftol", "1e-4", "--linmintol", "1e-2"]),
        q(&["--ras2ras"]),
        q(&["--regdat", "reg.dat"]),
        q(&["--params", "par.txt", "--final-cost", "cost.txt"]),
        q(&["--log-cost", "log.txt"]),
        q(&["--init-cost-only", "init.txt"]),
        q(&["--movout", "movout.mgz"]),
        q(&["--ref-mask", "seg.mgz"]),
        q(&["--mov-mask", "mmask.mgz"]),
        q(&["--mov-idither", "dith.mgz"]),
        q(&["--mov-oob"]),
        q(&["--no-mov-oob"]),
        q(&["--sat", "99"]),
        q(&["--sat", "100"]),
        q(&["--seed", "7"]),
        q(&["--seed", "-3"]),
        q(&["--centroid"]),
        q(&["--no-cras0"]),
        q(&["--regheader", "--cras0"]),
        q(&["--init-reg", "a.lta"]),
        q(&["--init-reg", "v.lta", "--init-reg-save", "i.lta"]),
        q(&["--trans", "2", "-1", "1", "--rot", "3", "0", "-2"]),
        q(&["--conf-ref"]),
        q(&["--threads", "2"]),
        q(&["--nthreads", "abc"]),
        q(&["--debug"]),
        q(&["--bf", "--bf-lim", "4", "--bf-nsamp", "4"]),
        q(&["--bf-lim", "0"]),
        q(&["--bf-nsamp", "-2", "--bf-lim", "2"]),
        q(&["--sep", "0"]),
        q(&["--sep", "-2"]),
        q(&["--dof", "13"]),
        q(&["--sep", "4", "--sep", "4", "--sep", "4", "--sep", "4", "--sep", "4", "--sep", "4", "--sep", "4", "--sep", "4", "--sep", "4", "--sep", "4"]),
        q(&["--params", "nodir/par.txt"]),
        q(&["--log-cost", "nodir/log.txt"]),
        q(&["--rusage", "ru.txt"]),
        q(&["--trans", "500", "0", "0"]),
    ] {
        cases.push(line(&[], &rfiles, &[], &a));
    }
    // the subject's files (--s, --sd, SUBJECTS_DIR) and FreeSurfer's environment
    let subj = vec![
        ("mov.mgz".to_owned(), files[0].1.clone()),
        ("bert/mri/brainmask.mgz".to_owned(), files[1].1.clone()),
        ("bert/mri/aparc+aseg.mgz".to_owned(), seg_bytes(&mut g, &p.refv, "aparc+aseg.mgz")),
    ];
    let sq = |extra: &[&str]| -> Vec<String> {
        let mut a = s(&["--mov", "mov.mgz", "--reg", "reg.lta"]);
        a.extend(quick.iter().map(|x| (*x).to_owned()));
        a.extend(extra.iter().map(|x| (*x).to_owned()));
        a
    };
    cases.push(line(&["SUBJECTS_DIR=@DIR@"], &subj, &[], &sq(&["--s", "bert"])));
    cases.push(line(&[], &subj, &[], &sq(&["--sd", "@DIR@", "--s", "bert"])));
    cases.push(line(&["SUBJECTS_DIR=@DIR@"], &subj, &[], &sq(&["--s", "bert", "--no-ref-mask"])));
    cases.push(line(&["SUBJECTS_DIR=@DIR@"], &subj, &[], &sq(&["--s", "bert", "--ref", "bert/mri/brainmask.mgz"])));
    cases.push(line(&["SUBJECTS_DIR=@DIR@"], &subj, &[], &sq(&["--s", "nobody"])));
    cases.push(line(&[], &subj, &[], &sq(&["--s", "bert"])));
    cases.push(line(&["FREESURFER_HOME=@DIR@/nowhere", "SUBJECTS_DIR=@DIR@"], &subj, &[], &sq(&["--s", "bert"])));
    cases.push(line(&["FREESURFER_HOME=@DIR@/nowhere"], &files, &[], &q(&[])));
    cases
}

/// A seeded case: a pair of random orientation, type and motion with random options.
fn seeded(g: &mut Gen) -> String {
    let mut p = Pair::base();
    p.mov.ty = g.pick(&[MRI_UCHAR, MRI_SHORT, MRI_FLOAT]);
    p.refv.ty = g.pick(&[MRI_UCHAR, MRI_SHORT, MRI_FLOAT]);
    p.mov.mdc = g.pick(&[LIA, RAS, LPI]);
    p.refv.mdc = g.pick(&[LIA, RAS]);
    p.mov.shift = [g.int(-6, 6) as i32, g.int(-6, 6) as i32, g.int(-4, 4) as i32];
    p.mov.shear = g.int(-2, 2) as i32;
    p.mov.gain = g.int(10, 30) as i32;
    p.mov.white = g.chance(8);
    if g.chance(4) {
        p.mov.dims = [g.int(16, 24) as i32, g.int(16, 24) as i32, g.int(10, 16) as i32];
    }
    if g.chance(5) {
        p.refv.dims = [g.int(24, 32) as i32, g.int(24, 32) as i32, g.int(24, 32) as i32];
    }
    let files = p.files(g);
    let mut a = s(&["--mov", "mov.mgz", "--ref", "ref.mgz", "--reg", "reg.lta"]);
    let opt = |a: &mut Vec<String>, x: &[&str]| a.extend(x.iter().map(|y| (*y).to_owned()));
    if g.chance(2) {
        opt(&mut a, &["--no-bf"]);
    } else {
        let lim = g.pick(&["2", "4", "10"]);
        let ns = g.pick(&["3", "5", "8"]);
        opt(&mut a, &["--bf-lim", lim, "--bf-nsamp", ns]);
    }
    opt(&mut a, &["--nitersmax", g.pick(&["1", "2", "4"])]);
    match g.below(4) {
        0 => opt(&mut a, &["--sep", "4"]),
        1 => opt(&mut a, &["--sep", "4", "--sep", "2"]),
        2 => opt(&mut a, &["--sep", "6"]),
        _ => {}
    }
    match g.below(6) {
        0 => opt(&mut a, &["--dof", "9"]),
        1 => opt(&mut a, &["--dof", "12"]),
        2 => opt(&mut a, &["--zscale"]),
        _ => {}
    }
    if g.chance(4) {
        opt(&mut a, &["--ftol", "1e-4", "--linmintol", "1e-2"]);
    }
    if g.chance(5) {
        opt(&mut a, &[g.pick(&["--no-dither", "--no-coord-dither", "--no-intensity-dither"])]);
    }
    if g.chance(6) {
        opt(&mut a, &["--no-smooth"]);
    }
    if g.chance(6) {
        opt(&mut a, &["--centroid"]);
    }
    if g.chance(6) {
        opt(&mut a, &["--mov-oob"]);
    }
    if g.chance(6) {
        opt(&mut a, &["--ras2ras"]);
    }
    if g.chance(6) {
        opt(&mut a, &["--seed", g.pick(&["1", "99", "12345"])]);
    }
    if g.chance(6) {
        opt(&mut a, &["--sat", g.pick(&["95", "99.9"])]);
    }
    if g.chance(5) {
        opt(&mut a, &["--regdat", "reg.dat", "--params", "p.txt", "--final-cost", "c.txt"]);
    }
    line(&[], &files, &[], &a)
}

/// The prelude, then `n` seeded cases.
pub fn generate(seed: u64, n: usize) -> Vec<String> {
    let mut out = prelude();
    let mut g = Gen::new(seed);
    for _ in 0..n {
        out.push(seeded(&mut g));
    }
    out
}

/// The golden cases (`tests/mri_coreg_golden.rs`): fMRIPrep's call (one separation, its
/// tolerances, the brute force narrowed) on the base pair, the 12-parameter schema with every
/// output, the 2-D schema, a subject's mask, and two stand-alone options.
pub fn golden_cases() -> Vec<String> {
    let mut g = Gen::new(0x676f_6c64);
    let p = Pair::base();
    let mut files = p.files(&mut g);
    files.push(("a.lta".to_owned(), init_lta(&p, 1)));
    files.push(("seg.mgz".to_owned(), seg_bytes(&mut g, &p.refv, "seg.mgz")));
    let req = s(&["--mov", "mov.mgz", "--ref", "ref.mgz", "--reg", "reg.lta"]);
    let with = |extra: &[&str]| -> Vec<String> {
        let mut a = req.clone();
        a.extend(extra.iter().map(|x| (*x).to_owned()));
        a
    };
    vec![
        line(&[], &files, &[], &with(&["--dof", "6", "--sep", "4", "--ftol", "1e-4", "--linmintol", "1e-2", "--bf-lim", "4", "--bf-nsamp", "4", "--nitersmax", "2", "--ref-mask", "seg.mgz"])),
        line(&[], &files, &[], &with(&["--dof", "12", "--no-bf", "--nitersmax", "1", "--sep", "4", "--regdat", "reg.dat", "--params", "p.txt", "--final-cost", "c.txt", "--log-cost", "log.txt", "--movout", "mo.mgz", "--init-reg", "a.lta"])),
        line(&[], &files, &[], &with(&["--xytrans+zrot", "--no-bf", "--nitersmax", "1", "--sep", "4", "--centroid", "--ras2ras"])),
        line(&[], &files, &[], &s(&["--mat2par", "a.lta"])),
        line(&[], &files, &[], &s(&["--lrrev", "a.lta", "lr.lta"])),
    ]
}

/// Text files listed with their bytes.
fn is_text(name: &str) -> bool {
    [".lta", ".xfm", ".dat", ".txt"].iter().any(|e| name.ends_with(e))
}

/// The lines that name the machine and the user, masked: `<key> <M>`.
pub fn mask_machine(s: &str) -> String {
    let keys = ["sysname  ", "hostname ", "machine  ", "user     "];
    let mut out = String::with_capacity(s.len());
    for (i, l) in s.split("\\x0a").enumerate() {
        if i > 0 {
            out.push_str("\\x0a");
        }
        match keys.iter().find(|k| l.starts_with(*k)) {
            Some(k) => {
                out.push_str(k);
                out.push_str("<M>");
            }
            None => out.push_str(l),
        }
    }
    out
}

/// The times masked: `OptTimeSec <T> sec`, `OptTimeMin <T> min`, `mri_coreg RunTimeSec <T> sec`.
pub fn mask_time(s: &str) -> String {
    let mut out = s.to_owned();
    for (head, tail) in [
        ("OptTimeSec ", " sec"),
        ("OptTimeMin ", " min"),
        ("mri_coreg RunTimeSec ", " sec"),
    ] {
        let mut from = 0;
        while let Some(at) = out[from..].find(head) {
            let start = from + at + head.len();
            let Some(end) = out[start..].find(tail) else {
                break;
            };
            out.replace_range(start..start + end, "<T>");
            from = start + 3;
        }
    }
    out
}

/// Every file of a case's folder, sorted, each as `F <name> <fnv> <size>` (text files with their
/// bytes escaped), the folder's name in them as `<DIR>`; each folder as `D <name>`. The inputs
/// (named in `inputs`) are left out.
pub fn written(dir: &Path, inputs: &[String]) -> String {
    let d = dir.to_string_lossy().into_owned();
    let mut out = Vec::new();
    for (n, b) in collect_all(dir, "") {
        if inputs.iter().any(|i| n == *i || i.starts_with(&format!("{n}/"))) {
            continue;
        }
        match b {
            None => out.push(format!("D {n}")),
            Some(b) => {
                let b = mask_dir(&b, d.as_bytes());
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

fn mask_dir(b: &[u8], d: &[u8]) -> Vec<u8> {
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

/// Builds a case's folder (`dir`: empty) and returns its arguments (`@DIR@` replaced),
/// environment and input names.
fn setup(line: &str, dir: &Path) -> (Vec<String>, Vec<(String, String)>, Vec<String>) {
    let t: Vec<&str> = line.split_whitespace().collect();
    assert_eq!(t[0], "coreg");
    let d = dir.to_string_lossy().into_owned();
    let mut env = probe_env();
    if t[1] != "-" {
        let e = String::from_utf8(unhex(t[1])).expect("assignments");
        for a in e.split(';').filter(|a| !a.is_empty()) {
            let (k, v) = a.split_once('=').expect("NAME=value");
            env.push((k.to_owned(), v.replace("@DIR@", &d)));
        }
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
        .map(|k| {
            String::from_utf8(unhex(t[i + 1 + k]))
                .expect("an argument")
                .replace("@DIR@", &d)
        })
        .collect();
    (args, env, inputs)
}

/// The port's line for a case line and the defects it noted (`dir`: an empty folder).
pub fn run_line_in(line: &str, mode: UpstreamDefects, dir: &Path) -> (String, Vec<&'static str>) {
    let (args, env, inputs) = setup(line, dir);
    let environment = Environment::from_pairs(env);
    let mut g = Globals::new(mode);
    let (mut out, mut err) = (Vec::new(), Vec::new());
    // the official side's OMP_NUM_THREADS (the probe's environment: 1)
    let threads = environment
        .get("OMP_NUM_THREADS")
        .and_then(|v| v.to_str())
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let status = run_with(&mut g, &args, environment, Some(dir), threads, &mut out, &mut err);
    let d = dir.to_string_lossy().into_owned();
    let mask = |b: &[u8]| {
        mask_machine(&mask_time(
            &escape(b).replace(&escape(d.as_bytes()), "<DIR>"),
        ))
    };
    let files = written(dir, &inputs);
    let end = match status {
        134 => "CRASH 6".to_owned(),
        136 => "CRASH 8".to_owned(),
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
