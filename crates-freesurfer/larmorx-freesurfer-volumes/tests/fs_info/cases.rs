// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The fs-info probe's cases (`validation/probes/fs-info/`): a seeded generator of `mri_info`
//! command lines with the files they read, and the port's answer to each, printed as `check.py`
//! prints the official program's (`<status> | <stdout> | <stderr> | <files>`, C-escaped).
//!
//! A case line is `info <env> <nfiles> [<name> <hex>]... <nargs> [<hex>]...`: `<env>` is `-` or the
//! hex of one `NAME=value` added to the probe's environment; the files are written into the case's
//! folder (names may hold one `/`: a folder made first); the arguments follow the program's name.
//! The input volumes are written by the port's own writers (`mri_write`, `write_morph`,
//! `gcam_write`), so both sides read the same bytes.
#![allow(dead_code)]

use std::io::Write;
use std::path::Path;

use larmorx_core::defects::UpstreamDefects;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::TrackedErr;
use larmorx_freesurfer_core::matrix::{MATRIX_REAL, Matrix};
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_gca::gcam::gcam_read;
use larmorx_freesurfer_gca::gcam_write::{gcam_write, write_morph};
use larmorx_freesurfer_io::colortab::ctab_read_ascii;
use larmorx_freesurfer_io::identify::MRI_CORONAL_SLICE_DIRECTORY;
use larmorx_freesurfer_io::mriio::{mri_write, mri_write_type};
use larmorx_freesurfer_mri::mri::{
    MGZ_INTENT_UNKNOWN, MRI_FLOAT, MRI_INT, MRI_SHORT, MRI_UCHAR, Mri, mri_alloc_sequence,
};
use larmorx_freesurfer_mri::voxel::mri_set_vox_val;
use larmorx_freesurfer_volumes::mri_info::main::main;
use larmorx_freesurfer_volumes::mri_info::options::Run;

/// The cases' random numbers (splitmix64).
pub struct Gen(u64);

impl Gen {
    pub fn new(seed: u64) -> Gen {
        Gen(seed)
    }
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    fn int(&mut self, lo: i64, hi: i64) -> i64 {
        lo + self.below((hi - lo + 1) as u64) as i64
    }
    fn pick<T: Copy>(&mut self, xs: &[T]) -> T {
        xs[self.below(xs.len() as u64) as usize]
    }
    fn chance(&mut self, k: u64) -> bool {
        self.below(k) == 0
    }
}

pub fn hex(b: &[u8]) -> String {
    if b.is_empty() {
        return "@".to_owned();
    }
    b.iter().map(|c| format!("{c:02x}")).collect()
}

pub fn unhex(h: &str) -> Vec<u8> {
    if h == "@" {
        return Vec::new();
    }
    (0..h.len() / 2)
        .map(|i| u8::from_str_radix(&h[2 * i..2 * i + 2], 16).expect("hex"))
        .collect()
}

/// C-escaped bytes: printable ASCII as itself (`\` escaped), the rest `\xNN`.
pub fn escape(b: &[u8]) -> String {
    let mut s = String::new();
    for &c in b {
        if !(0x20..=0x7e).contains(&c) || c == b'\\' {
            s += &format!("\\x{c:02x}");
        } else {
            s.push(c as char);
        }
    }
    s
}

fn fnv(b: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &c in b {
        h = (h ^ u64::from(c)).wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ------------------------------------------------------------------ the volumes

/// A volume to write: its geometry, scan parameters and the tags of its header.
#[derive(Clone)]
struct Spec {
    dims: [i32; 4],
    ty: i32,
    size: [f32; 3],
    mdc: [f32; 9],
    c: [f32; 3],
    good: i32,
    tr: f32,
    flip: f64,
    te: f32,
    ti: f32,
    fov: f32,
    dof: i32,
    xform: Option<String>,
    pedir: Option<String>,
    cmds: Vec<String>,
    auto_align: Option<[f32; 16]>,
    orig: Option<[f32; 16]>,
    field: f32,
    /// frames' label, threshold and name
    frames: Vec<(i32, f32, String)>,
    ctab: bool,
    seg: bool,
}

const LUT: &[u8] = b"0 Unknown 0 0 0 0\n2 Left-Cerebral-White-Matter 245 245 245 0\n3 Left-Cerebral-Cortex 205 62 78 0\n17 Left-Hippocampus 220 216 20 0\n41 Right-Cerebral-White-Matter 0 225 0 0\n";

const XFM: &[u8] = b"MNI Transform File\n% avi2talxfm\n\nTransform_Type = Linear;\nLinear_Transform =\n 1.05 0.01 -0.02 -1.5\n -0.01 1.1 0.03 2.25\n 0.02 -0.03 0.95 -3.0;\n";

/// Direction cosines (`x_r x_a x_s y_r y_a y_s z_r z_a z_s`): a signed permutation (most), an
/// oblique rotation, a degenerate set or none at all.
fn rand_mdc(g: &mut Gen) -> [f32; 9] {
    let mut m = [0f32; 9];
    match g.below(12) {
        0 => {
            let (a, b) = (0.6f32, 0.8f32);
            let cols = match g.below(3) {
                0 => [[a, b, 0.0], [-b, a, 0.0], [0.0, 0.0, 1.0]],
                1 => [[1.0, 0.0, 0.0], [0.0, a, b], [0.0, -b, a]],
                _ => [[a, 0.0, -b], [0.0, 1.0, 0.0], [b, 0.0, a]],
            };
            for k in 0..3 {
                m[3 * k..3 * k + 3].copy_from_slice(&cols[k]);
            }
        }
        1 => {
            let cols = if g.chance(2) {
                [[-1.0, 0.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]
            } else {
                [[0.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]
            };
            for k in 0..3 {
                m[3 * k..3 * k + 3].copy_from_slice(&cols[k]);
            }
        }
        2 => {
            // equal components: the orientation's ties
            let h = std::f32::consts::FRAC_1_SQRT_2;
            m = [h, h, 0.0, -h, h, 0.0, 0.0, 0.0, 1.0];
        }
        _ => {
            let perms = [
                [0, 1, 2],
                [0, 2, 1],
                [1, 0, 2],
                [1, 2, 0],
                [2, 0, 1],
                [2, 1, 0],
            ];
            let p = g.pick(&perms);
            for k in 0..3 {
                let s = if g.chance(2) { -1.0 } else { 1.0 };
                m[3 * k + p[k]] = s;
            }
        }
    }
    m
}

fn rand_matrix(g: &mut Gen) -> [f32; 16] {
    let mut m = [0f32; 16];
    for (k, v) in m.iter_mut().enumerate() {
        *v = if k % 5 == 0 {
            1.0 + g.int(-4, 4) as f32 * 0.125
        } else {
            g.int(-20, 20) as f32 * 0.0625
        };
    }
    m[12] = 0.0;
    m[13] = 0.0;
    m[14] = 0.0;
    m[15] = 1.0;
    m
}

fn rand_spec(g: &mut Gen) -> Spec {
    let sizes = [1.0f32, 1.0, 1.0, 0.5, 2.0, 1.333_333_3, 0.999_5, 1.000_6, 3.0];
    let dims = [
        g.int(1, 5) as i32,
        g.int(1, 5) as i32,
        g.int(1, 5) as i32,
        if g.chance(3) { g.int(2, 3) as i32 } else { 1 },
    ];
    let ty = g.pick(&[MRI_UCHAR, MRI_SHORT, MRI_INT, MRI_FLOAT]);
    let size = if g.chance(4) {
        let s = g.pick(&sizes);
        [s, s, s]
    } else {
        [g.pick(&sizes), g.pick(&sizes), g.pick(&sizes)]
    };
    let names = ["talairach.xfm", "transforms/talairach.xfm", "/no/such/t.xfm", "t.xfm"];
    let pedirs = ["ROW", "COL", "LR", "AP", "UNKNOWN"];
    let mut frames = Vec::new();
    if dims[3] > 1 || g.chance(5) {
        for k in 0..dims[3] {
            let label = g.pick(&[0, 0, 2, 3, 17, 41, 77, 9999]);
            let thresh = g.pick(&[0.0f32, 0.0, 0.5, 2.25, -1.0, 1e-8]);
            frames.push((label, thresh, format!("frame{k}")));
        }
    }
    Spec {
        dims,
        ty,
        size,
        mdc: rand_mdc(g),
        c: [
            g.int(-8, 8) as f32 * 0.5,
            g.int(-8, 8) as f32 * 0.25,
            g.int(-8, 8) as f32,
        ],
        good: if g.chance(6) { 0 } else { 1 },
        tr: g.pick(&[0.0f32, 2300.0, 1.5, 800.0]),
        flip: g.pick(&[0.0f64, 0.15707963267948966, 0.5235987755982988, 1.0]),
        te: g.pick(&[0.0f32, 2.98, 30.0]),
        ti: g.pick(&[0.0f32, 900.0, 1100.5]),
        fov: g.pick(&[0.0f32, 256.0, 3.5]),
        dof: g.int(0, 3) as i32,
        xform: g.chance(3).then(|| g.pick(&names).to_owned()),
        pedir: g.chance(3).then(|| g.pick(&pedirs).to_owned()),
        cmds: (0..g.int(0, 2))
            .map(|k| format!("mri_convert in{k}.mgz out.mgz --conform"))
            .collect(),
        auto_align: g.chance(5).then(|| rand_matrix(g)),
        orig: g.chance(6).then(|| rand_matrix(g)),
        field: g.pick(&[0.0f32, 1.5, 3.0, 7.0]),
        frames,
        ctab: g.chance(6),
        seg: g.chance(4),
    }
}

fn matrix_of(v: &[f32; 16]) -> Matrix {
    let mut m = Matrix::real(4, 4);
    for r in 0..4 {
        for c in 0..4 {
            m.set_relt(r + 1, c + 1, v[(r * 4 + c) as usize]);
        }
    }
    debug_assert_eq!(m.mtype, MATRIX_REAL);
    m
}

/// The volume of a spec, with seeded voxels.
fn build(g: &mut Gen, s: &Spec) -> Mri {
    let mut fs = Globals::new(UpstreamDefects::Fix);
    let (mut o, mut e) = (Vec::new(), Vec::new());
    let mut m = mri_alloc_sequence(
        &mut fs, &mut o, &mut e, s.dims[0], s.dims[1], s.dims[2], s.ty, s.dims[3],
    )
    .expect("a volume");
    let labels = [0.0f32, 2.0, 3.0, 17.0, 41.0];
    // a large volume stays zero (a small compressed file)
    let large = s.dims.iter().map(|&d| i64::from(d)).product::<i64>() > 100_000;
    for f in 0..if large { 0 } else { s.dims[3] } {
        for z in 0..s.dims[2] {
            for y in 0..s.dims[1] {
                for x in 0..s.dims[0] {
                    let v = if s.seg {
                        g.pick(&labels)
                    } else {
                        match s.ty {
                            MRI_UCHAR => g.int(0, 250) as f32,
                            MRI_FLOAT => g.int(-400, 2000) as f32 * 0.125,
                            _ => g.int(-50, 600) as f32,
                        }
                    };
                    mri_set_vox_val(&mut fs, &mut e, &mut m, x, y, z, f, v, 0).expect("a voxel");
                }
            }
        }
    }
    let vg = &mut m.vg;
    vg.xsize = s.size[0];
    vg.ysize = s.size[1];
    vg.zsize = s.size[2];
    [vg.x_r, vg.x_a, vg.x_s] = [s.mdc[0], s.mdc[1], s.mdc[2]];
    [vg.y_r, vg.y_a, vg.y_s] = [s.mdc[3], s.mdc[4], s.mdc[5]];
    [vg.z_r, vg.z_a, vg.z_s] = [s.mdc[6], s.mdc[7], s.mdc[8]];
    [vg.c_r, vg.c_a, vg.c_s] = s.c;
    vg.ras_good_flag = s.good;
    m.tr = s.tr;
    m.flip_angle = s.flip;
    m.te = s.te;
    m.ti = s.ti;
    m.fov = s.fov;
    m.dof = s.dof;
    if let Some(x) = &s.xform {
        m.transform_fname = x.as_bytes().to_vec();
    }
    m.pedir = s.pedir.as_ref().map(|p| p.as_bytes().to_vec());
    m.cmdlines = s.cmds.iter().map(|c| c.as_bytes().to_vec()).collect();
    m.auto_align = s.auto_align.as_ref().map(matrix_of);
    m.orig_ras2vox = s.orig.as_ref().map(matrix_of);
    m.field_strength = s.field;
    for (k, (label, thresh, name)) in s.frames.iter().enumerate() {
        if let Some(fr) = m.frames.get_mut(k) {
            fr.label = *label;
            fr.thresh = *thresh;
            fr.name = larmorx_freesurfer_core::cname::CName::new(name.as_bytes());
        }
    }
    if s.ctab {
        let t = tempfile::tempdir().expect("a temporary folder");
        std::fs::write(t.path().join("lut.txt"), LUT).expect("the table");
        m.ct = ctab_read_ascii(&mut fs, &mut o, &mut e, Some(t.path()), "lut.txt");
    }
    m
}

/// The files `write` leaves in an empty folder, by name (one level of folders).
fn collect(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let mut files = Vec::new();
    let mut names: Vec<_> = std::fs::read_dir(dir)
        .expect("the folder")
        .filter_map(|e| e.ok())
        .collect();
    names.sort_by_key(|e| e.file_name());
    for e in names {
        let n = e.file_name().to_string_lossy().into_owned();
        if e.path().is_dir() {
            for (m, b) in collect(&e.path()) {
                files.push((format!("{n}/{m}"), b));
            }
        } else {
            files.push((n, std::fs::read(e.path()).expect("a file")));
        }
    }
    files
}

/// The files of `m` written as `name` (`mri_write`; a COR folder for names ending in `.cor`).
fn write_volume(m: &mut Mri, name: &str) -> Vec<(String, Vec<u8>)> {
    let t = tempfile::tempdir().expect("a temporary folder");
    let mut fs = Globals::new(UpstreamDefects::Fix);
    let (mut o, mut e) = (Vec::new(), Vec::new());
    let env = Environment::default();
    if name.ends_with(".cor") {
        let _ = mri_write_type(
            &mut fs,
            &mut o,
            &mut e,
            &env,
            Some(t.path()),
            m,
            name,
            MRI_CORONAL_SLICE_DIRECTORY,
            MGZ_INTENT_UNKNOWN,
        );
    } else {
        let _ = mri_write(&mut fs, &mut o, &mut e, &env, Some(t.path()), m, name, None, MGZ_INTENT_UNKNOWN);
    }
    collect(t.path())
}

/// A three-frame warp volume (displacements) written as a morph (`name` `.m3z`), and the same
/// morph written as a warp field (`.mgz`, `MGZ_INTENT_WARPMAP`) when `warp` names one.
fn morph_files(g: &mut Gen, m3z: &str, warp: Option<&str>) -> Vec<(String, Vec<u8>)> {
    let t = tempfile::tempdir().expect("a temporary folder");
    let mut fs = Globals::new(UpstreamDefects::Fix);
    let (mut o, mut e) = (Vec::new(), Vec::new());
    let env = Environment::default();
    let whd = [g.int(2, 3) as i32, g.int(2, 3) as i32, g.int(1, 3) as i32];
    let mut w = mri_alloc_sequence(&mut fs, &mut o, &mut e, whd[0], whd[1], whd[2], MRI_FLOAT, 3)
        .expect("a warp");
    for x in 0..whd[0] {
        for y in 0..whd[1] {
            for z in 0..whd[2] {
                for f in 0..3 {
                    let v = g.int(-8, 8) as f32 * 0.25;
                    mri_set_vox_val(&mut fs, &mut e, &mut w, x, y, z, f, v, 0).expect("a voxel");
                }
            }
        }
    }
    w.vg.xsize = g.pick(&[1.0f32, 2.0]);
    w.vg.ysize = w.vg.xsize;
    w.vg.zsize = w.vg.xsize;
    let p = t.path().join(m3z).to_string_lossy().into_owned();
    let _ = write_morph(&mut fs, &mut o, &mut e, &env, None, &w, &p);
    if let Some(warp) = warp
        && let Ok(Some(mut gc)) = gcam_read(&mut fs, &mut o, &mut e, &env, None, &p)
    {
        let _ = gcam_write(&mut fs, &mut o, &mut e, &env, Some(t.path()), &mut gc, warp, 0);
    }
    collect(t.path())
}

// ------------------------------------------------------------------ the options

/// Every option without an argument.
const FLAGS: &[&str] = &[
    "--dump", "--zero-cras", "--tr", "--te", "--ti", "--fa", "--fad", "--flip_angle", "--pedir",
    "--cres", "--xsize", "--rres", "--ysize", "--sres", "--zsize", "--voxvol", "--type",
    "--is-1mm-iso", "--conformed", "--conformed-to-min", "--ncols", "--width", "--nrows",
    "--height", "--nslices", "--depth", "--dim", "--res", "--min-res", "--dof", "--cdc", "--rdc",
    "--sdc", "--vox2ras", "--ras2vox", "--vox2ras-tkr", "--ras2vox-tkr", "--vox2ras-fsl",
    "--tkr2scanner", "--ctab", "--scanner2tkr", "--ras_good", "--cras", "--center", "--p0", "--det",
    "--nframes", "--mid-frame", "--format", "--orientation", "--ori", "--slicedirection",
    "--autoalign", "--orig_ras2vox", "--entropy", "--voxvolsum", "--stats", "--cmds",
    "--diag-debug", "--debug",
];

/// The in-type names `-it` is given.
const TYPES: &[&str] = &["mgz", "nii", "analyze4d", "cor", "m3z", "minc", "bogus", "MGH", "nifti1"];

fn info_line(env: Option<&str>, files: &[(String, Vec<u8>)], args: &[String]) -> String {
    let mut t = vec![
        "info".to_owned(),
        env.map_or("-".to_owned(), |e| hex(e.as_bytes())),
        files.len().to_string(),
    ];
    for (f, c) in files {
        t.push(f.clone());
        t.push(hex(c));
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

/// The fixed cases: each option once on a plain volume, each kind of input, each failure.
fn prelude() -> Vec<String> {
    let mut g = Gen::new(0x1f0_1f0);
    let mut out = Vec::new();
    let base = Spec {
        dims: [3, 4, 2, 1],
        ty: MRI_SHORT,
        size: [1.0, 1.333_333_3, 1.333_333_3],
        mdc: [-1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 1.0, 0.0],
        c: [-1.0, -5.0, -1.0],
        good: 1,
        tr: 2300.0,
        flip: 0.157_079_632_679_489_66,
        te: 2.98,
        ti: 900.0,
        fov: 256.0,
        dof: 1,
        xform: Some("transforms/talairach.xfm".to_owned()),
        pedir: Some("ROW".to_owned()),
        cmds: s(&["mri_convert a.mgz b.mgz --conform", "mri_add_xform_to_header -c t.xfm b.mgz"]),
        auto_align: Some(rand_matrix(&mut g)),
        orig: Some(rand_matrix(&mut g)),
        field: 3.0,
        frames: vec![(17, 0.5, "f0".to_owned())],
        ctab: true,
        seg: false,
    };
    let mut m = build(&mut g, &base);
    let vol = write_volume(&mut m, "v.mgz");
    let mut bare = base.clone();
    bare.xform = None;
    bare.pedir = None;
    bare.cmds.clear();
    bare.auto_align = None;
    bare.orig = None;
    bare.frames.clear();
    bare.ctab = false;
    bare.ty = MRI_FLOAT;
    let mut mb = build(&mut g, &bare);
    let plain = write_volume(&mut mb, "p.mgh");
    // each option once on both
    for f in FLAGS {
        for files in [&vol, &plain] {
            let name = files[0].0.clone();
            out.push(info_line(None, files, &[f.to_string(), name]));
        }
    }
    for files in [&vol, &plain] {
        let name = files[0].0.clone();
        out.push(info_line(None, files, std::slice::from_ref(&name)));
        for extra in [
            s(&["--o", "out.txt", "--nrows"]),
            s(&["--o", "out.txt"]),
            s(&["--o", "out.txt", "--autoalign"]),
            s(&["--o", "out.txt", "--ctab"]),
            s(&["--o", "out.txt", "--stats"]),
            s(&["--o", "out.txt", "--voxel", "1", "2", "0"]),
            s(&["--otags", "tags.bin"]),
            s(&["--otags", "tags.binz"]),
            s(&["--otags", "tags.mgz", "--o", "out.txt"]),
            s(&["--otags", "nodir/tags.bin"]),
            s(&["--otags", ""]),
            s(&["--voxel", "0", "0", "0"]),
            s(&["--voxel", "2", "3", "1"]),
            s(&["--voxel", "a", "1", "x"]),
            s(&["--voxel", "-1", "0", "0"]),
            s(&["--voxel", "1", "2"]),
            s(&["-it", "nii"]),
            s(&["--in_type", "mgh", "--voxel", "1", "1", "1"]),
            s(&["--debug", "--tr", "--te"]),
            s(&["--tr", "--type"]),
            s(&["--ti", "--is-1mm-iso"]),
            s(&["--DIM", "--Tr"]),
            s(&["--zero-cras", "--cras"]),
            s(&["--zero-cras", "--vox2ras"]),
            s(&["--zero-cras"]),
            s(&["--diag-debug", "--ctab"]),
            s(&["--diag-debug", "--otags", "t2.bin"]),
        ] {
            let mut a = extra.clone();
            a.push(name.clone());
            out.push(info_line(None, files, &a));
        }
    }
    // the environment's diagnostics
    // (a name without an extension: the program ends before any volume is read, whose licence
    // check prints the licence file under DIAG_NO)
    out.push(info_line(Some("DIAG_NO=2"), &vol, &s(&["--voxel", "1", "1", "0", "noext"])));
    // the command line alone
    for a in [
        s(&[]),
        s(&["--help"]),
        s(&["--HELP"]),
        s(&["--version"]),
        s(&["--Version"]),
        s(&["--version", "v.mgz"]),
        s(&["--nrows"]),
        s(&["v.mgz", "--o"]),
        s(&["v.mgz", "-it"]),
        s(&["v.mgz", "--otags"]),
        s(&["--o", "nodir/out.txt", "v.mgz"]),
        s(&["--o", "", "v.mgz"]),
        s(&["noext"]),
        s(&["nothere.mgz"]),
        s(&["nothere.nii"]),
        s(&["v.mgz", "nothere.mgz", "v.mgz"]),
        s(&["--nrows", "v.mgz", "v.mgz", "v.mgz"]),
        s(&["v."]),
        s(&["dir.d"]),
        s(&["x.unknownext"]),
    ] {
        let mut files = vol.clone();
        if a.iter().any(|x| x == "dir.d") {
            files.push(("dir.d/README".to_owned(), b"a folder\n".to_vec()));
        }
        if a.iter().any(|x| x == "x.unknownext") {
            files.push(("x.unknownext".to_owned(), b"no volume\n".to_vec()));
        }
        out.push(info_line(None, &files, &a));
    }
    // other formats, each with the full report and a few options
    let mut o2 = base.clone();
    o2.dims = [4, 3, 2, 3];
    o2.frames = vec![(2, 0.0, "a".into()), (0, 1.5, "b".into()), (41, -2.0, "c".into())];
    for name in ["f.nii", "f.nii.gz", "f.img", "f.mgh", "f.cor"] {
        let mut s2 = o2.clone();
        if name.ends_with(".cor") {
            s2.ty = MRI_UCHAR;
            s2.dims[3] = 1;
        }
        let mut m = build(&mut g, &s2);
        let files = write_volume(&mut m, name);
        for args in [
            s(&[name]),
            s(&["--dump", name]),
            s(&["--format", name]),
            s(&["--stats", name]),
            s(&["--voxvolsum", name]),
            s(&["--entropy", name]),
            s(&["--voxel", "1", "1", "1", name]),
            s(&["--otags", "o.bin", name]),
            s(&["--cmds", name]),
        ] {
            // (a COR volume's transform name holds the case's folder, whose length the tag file
            // keeps: not comparable between two folders)
            if name.ends_with(".cor") && args[0] == "--otags" {
                continue;
            }
            out.push(info_line(None, &files, &args));
        }
    }
    // a talairach transform the reader finds
    let mut withx = base.clone();
    withx.xform = Some("t.xfm".to_owned());
    let mut mx = build(&mut g, &withx);
    let mut xf = write_volume(&mut mx, "x.mgz");
    xf.push(("t.xfm".to_owned(), XFM.to_vec()));
    out.push(info_line(None, &xf, &s(&["x.mgz"])));
    out.push(info_line(None, &xf, &s(&["--stats", "x.mgz"])));
    // a conformed volume (256 voxels a side, uchar, coronal)
    let mut conf = bare.clone();
    conf.dims = [256, 256, 256, 1];
    conf.ty = MRI_UCHAR;
    conf.size = [1.0, 1.0, 1.0];
    conf.seg = true;
    let mut mc = build(&mut Gen::new(5), &conf);
    let cf = write_volume(&mut mc, "c.mgz");
    for a in ["--conformed", "--conformed-to-min", "--is-1mm-iso", "--res", "--min-res"] {
        out.push(info_line(None, &cf, &s(&[a, "c.mgz"])));
    }
    // morphs and warp fields
    let mf = morph_files(&mut g, "m.m3z", Some("w.mgz"));
    for args in [
        s(&["m.m3z"]),
        s(&["--format", "m.m3z"]),
        s(&["--o", "out.txt", "m.m3z"]),
        s(&["w.mgz"]),
        s(&["--format", "w.mgz"]),
        s(&["--otags", "wt.bin", "w.mgz"]),
        s(&["--otags", "wt.bin", "--o", "out.txt", "w.mgz"]),
        s(&["--dump", "w.mgz"]),
        s(&["nothere.m3z"]),
    ] {
        out.push(info_line(None, &mf, &args));
    }
    let mut mixed = morph_files(&mut g, "m.m3z", None);
    mixed[0].0 = "m.M3z".to_owned();
    for args in [
        s(&["m.M3z"]),
        s(&["--format", "m.M3z"]),
        s(&["--nrows", "m.M3z"]),
        s(&["--otags", "t.bin", "m.M3z"]),
    ] {
        out.push(info_line(None, &mixed, &args));
    }
    let mut upper = morph_files(&mut g, "m.m3z", None);
    upper[0].0 = "m.M3Z".to_owned();
    out.push(info_line(None, &upper, &s(&["m.M3Z"])));
    // a voxel size of 0: matrices that cannot be inverted
    let mut zero = bare.clone();
    zero.size = [0.0, 1.0, 1.0];
    let mut mz = build(&mut g, &zero);
    let zf = write_volume(&mut mz, "z.mgz");
    for a in [
        "--ras2vox", "--ras2vox-tkr", "--tkr2scanner", "--scanner2tkr", "--det", "--center", "--vox2ras",
    ] {
        out.push(info_line(None, &zf, &s(&[a, "z.mgz"])));
    }
    out.push(info_line(None, &zf, &s(&["z.mgz"])));
    out
}

/// A seeded case: one to three inputs (volumes of every kind, missing names), zero to three
/// options.
fn seeded(g: &mut Gen) -> String {
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    let mut inputs: Vec<String> = Vec::new();
    for k in 0..g.int(1, 3) {
        match g.below(14) {
            0 => {
                let f = morph_files(g, &format!("m{k}.m3z"), Some(&format!("w{k}.mgz")));
                let pick = if g.chance(2) { &f[0].0 } else { &f[f.len() - 1].0 };
                inputs.push(pick.clone());
                files.extend(f);
            }
            1 => inputs.push(g.pick(&["nothere.mgz", "noext", "nothere.nii.gz"]).to_owned()),
            _ => {
                let spec = rand_spec(g);
                let ext = g.pick(&[".mgz", ".mgz", ".mgh", ".mgz", ".nii", ".nii.gz", ".img", ".mgh"]);
                let name = format!("v{k}{ext}");
                let mut m = build(g, &spec);
                let f = write_volume(&mut m, &name);
                if spec.xform.as_deref() == Some("t.xfm") && g.chance(2) {
                    files.push(("t.xfm".to_owned(), XFM.to_vec()));
                }
                files.extend(f);
                inputs.push(name);
            }
        }
    }
    // option groups (an option with its arguments), the inputs as groups of their own
    let mut groups: Vec<Vec<String>> = Vec::new();
    for _ in 0..g.int(0, 3) {
        groups.push(match g.below(16) {
            0 => vec!["--o".into(), "out.txt".into()],
            1 => vec!["--otags".into(), g.pick(&["t.bin", "t.mgz", "t.binz"]).into()],
            2 => vec![g.pick(&["-it", "--in_type"]).into(), g.pick(TYPES).into()],
            3 => {
                let mut v = vec!["--voxel".to_owned()];
                for _ in 0..3 {
                    v.push(g.int(0, 4).to_string());
                }
                v
            }
            _ => vec![g.pick(FLAGS).into()],
        });
    }
    for i in inputs {
        let at = g.below(groups.len() as u64 + 1) as usize;
        groups.insert(at, vec![i]);
    }
    let args: Vec<String> = groups.into_iter().flatten().collect();
    info_line(None, &files, &args)
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

// ------------------------------------------------------------------ the port's answer

/// The probe's environment (the official side adds `FREESURFER_HOME`, `FS_LICENSE` and
/// `FS_DISABLE_LANG`).
pub fn probe_env() -> Vec<(String, String)> {
    [
        ("PATH", "/usr/bin:/bin"),
        ("LC_ALL", "C"),
        ("LANG", "C"),
        ("OMP_NUM_THREADS", "1"),
    ]
    .iter()
    .map(|(a, b)| ((*a).to_owned(), (*b).to_owned()))
    .collect()
}

/// `b` with every occurrence of `pat` replaced by `<DIR>`.
fn mask_bytes(b: &[u8], pat: &[u8]) -> Vec<u8> {
    if pat.is_empty() {
        return b.to_vec();
    }
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i..].starts_with(pat) {
            out.extend_from_slice(b"<DIR>");
            i += pat.len();
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    out
}

/// The files of a case's folder that are not its inputs, sorted, each as `F <name> <fnv> <size>`
/// (text files ending in `.txt` with their bytes escaped), the folder's name in them as `<DIR>`.
pub fn written(dir: &Path, inputs: &[String]) -> String {
    let d = dir.to_string_lossy().into_owned();
    let mut out = Vec::new();
    for (n, b) in collect(dir) {
        if inputs.contains(&n) {
            continue;
        }
        let b = mask_bytes(&b, d.as_bytes());
        if n.ends_with(".txt") {
            out.push(format!("F {n} {}", escape(&b)));
        } else {
            out.push(format!("F {n} {:016x} {}", fnv(&b), b.len()));
        }
    }
    out.join(" ")
}

/// The port's line for a case line and the defects it noted (`dir`: an empty folder).
pub fn run_line_in(line: &str, mode: UpstreamDefects, dir: &Path) -> (String, Vec<&'static str>) {
    let t: Vec<&str> = line.split_whitespace().collect();
    assert_eq!(t[0], "info");
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
        let name = t[i];
        let p = dir.join(name);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).expect("a folder");
        }
        std::fs::write(&p, unhex(t[i + 1])).expect("a case file");
        inputs.push(name.to_owned());
        i += 2;
    }
    let nargs: usize = t[i].parse().expect("a count");
    let args: Vec<String> = (0..nargs)
        .map(|k| String::from_utf8(unhex(t[i + 1 + k])).expect("an argument"))
        .collect();
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
    let files = written(dir, &inputs);
    let text = format!(
        "EXIT {status} | {} | {} | {files}",
        mask(&out),
        mask(&err)
    );
    let ids = g.notices.as_slice().iter().map(|n| n.defect.id).collect();
    (text, ids)
}

/// [`run_line_in`] in a fresh temporary folder.
pub fn run_line(line: &str, mode: UpstreamDefects) -> (String, Vec<&'static str>) {
    let dir = tempfile::tempdir().expect("a temporary folder");
    run_line_in(line, mode, dir.path())
}
