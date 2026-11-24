// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The fs-label2vol probe's cases (`validation/probes/fs-label2vol/`): a seeded generator of
//! `mri_label2vol` command lines with the files they read, and the port's answer to each, printed
//! as `check.py` prints the official program's (`<status> | <stdout> | <stderr> | <files>`,
//! C-escaped).
//!
//! A case line is `cat <env> <nfiles> [<name> <hex>]... <ndirs> [<hex>]... <nargs> [<hex>]...`,
//! as the fs-concat probe's (`<env>`: `-` or the hex of `;`-separated assignments, `NAME=value` or
//! `-NAME`; `@DIR@` in a value is the case's folder). Every case runs with `SUBJECTS_DIR` the case's
//! folder and `FREESURFER_SEED=1234` (the defects' colour table takes random colours) unless its
//! environment says otherwise. The inputs: volumes written by the port's writer, a closed surface
//! of 66 vertices (an octahedron divided twice) in the binary triangle format with and without its
//! volume geometry, curvature files (thickness, defect numbers with NaN from their bits), labels in
//! tkregister and scanner coordinates, an annotation with its colour table, `register.dat` and
//! `.lta` files, a subject's folder (`subj/surf`, `subj/label`, `subj/mri`). The written volumes are
//! compared through `gzread` with every command line the run recorded (`getAllInfo`'s line: its
//! length, time, user and machine) masked as `<CMD>`. Pseudo-notices, which `check.py` counts, not
//! compares: `CRASH` (a status FreeSurfer's build reaches by a signal: 134, 136, 139; its unflushed
//! standard output is lost), `MEMORY` (a result FreeSurfer takes from memory contents: FS-53,
//! FS-249, FS-1252, FS-1256, FS-1258, FS-1259) and `HANG` (FS-1257: FreeSurfer never ends).
#![allow(dead_code)]

#[path = "../fs_info/cases.rs"]
mod info;

use std::io::Write;
use std::path::Path;

use larmorx_core::defects::UpstreamDefects;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::TrackedErr;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_io::cfile::CFile;
use larmorx_freesurfer_io::colortab::ctab_read_ascii;
use larmorx_freesurfer_io::colortab_bin::ctab_write_into_binary;
use larmorx_freesurfer_mri::mri::{MRI_FLOAT, MRI_INT, MRI_UCHAR, Mri};
use larmorx_freesurfer_mri::voxel::mri_set_vox_val;
use larmorx_freesurfer_volumes::mri_label2vol::main::{Run, main};

pub use info::{Gen, escape, hex, probe_env, unhex};
use info::{LUT, Spec, build, fnv, mask_bytes};

// ------------------------------------------------------------------ the files

fn spec(ty: i32, dims: [i32; 4], size: [f32; 3]) -> Spec {
    Spec {
        dims,
        ty,
        size,
        mdc: [-1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 1.0, 0.0],
        c: [1.5, -2.0, 0.5],
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

fn be4(b: &mut Vec<u8>, v: i32) {
    b.extend_from_slice(&v.to_be_bytes());
}

fn bef(b: &mut Vec<u8>, v: f32) {
    b.extend_from_slice(&v.to_be_bytes());
}

/// The surface's vertices and faces: an octahedron of radius `r` about `c`, each face divided
/// into four twice (66 vertices, 128 faces), its corners counter-clockwise seen from outside;
/// exact binary fractions only.
pub fn sphere(r: f32, c: [f32; 3]) -> (Vec<[f32; 3]>, Vec<[i32; 3]>) {
    let mut v: Vec<[f32; 3]> = vec![
        [1.0, 0.0, 0.0],
        [-1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, -1.0, 0.0],
        [0.0, 0.0, 1.0],
        [0.0, 0.0, -1.0],
    ];
    let mut f: Vec<[i32; 3]> = vec![
        [0, 2, 4],
        [2, 1, 4],
        [1, 3, 4],
        [3, 0, 4],
        [2, 0, 5],
        [1, 2, 5],
        [3, 1, 5],
        [0, 3, 5],
    ];
    for _ in 0..2 {
        let mut mids: Vec<((i32, i32), i32)> = Vec::new();
        let mut mid = |v: &mut Vec<[f32; 3]>, a: i32, b: i32| -> i32 {
            let key = (a.min(b), a.max(b));
            if let Some((_, k)) = mids.iter().find(|(e, _)| *e == key) {
                return *k;
            }
            let (p, q) = (v[a as usize], v[b as usize]);
            v.push([
                (p[0] + q[0]) * 0.5,
                (p[1] + q[1]) * 0.5,
                (p[2] + q[2]) * 0.5,
            ]);
            let k = v.len() as i32 - 1;
            mids.push((key, k));
            k
        };
        let mut nf = Vec::new();
        for t in &f {
            let ab = mid(&mut v, t[0], t[1]);
            let bc = mid(&mut v, t[1], t[2]);
            let ca = mid(&mut v, t[2], t[0]);
            nf.push([t[0], ab, ca]);
            nf.push([ab, t[1], bc]);
            nf.push([ca, bc, t[2]]);
            nf.push([ab, bc, ca]);
        }
        f = nf;
    }
    let pts = v
        .iter()
        .map(|p| [p[0] * r + c[0], p[1] * r + c[1], p[2] * r + c[2]])
        .collect();
    (pts, f)
}

/// The volume-geometry block of a 12^3 template of 1 mm (`orig.mgz`'s).
fn vg_block() -> Vec<u8> {
    b"valid = 1  # volume info valid\nfilename = orig.mgz\nvolume = 12 12 12\nvoxelsize = 1.000000000000000e+00 1.000000000000000e+00 1.000000000000000e+00\nxras   = -1.000000000000000e+00 0.000000000000000e+00 0.000000000000000e+00\nyras   = 0.000000000000000e+00 0.000000000000000e+00 -1.000000000000000e+00\nzras   = 0.000000000000000e+00 1.000000000000000e+00 0.000000000000000e+00\ncras   = 1.500000000000000e+00 -2.000000000000000e+00 5.000000000000000e-01\n".to_vec()
}

/// A binary triangle file of `v` and `f`, with the volume geometry's tag when `geom`.
pub fn triangle_file(v: &[[f32; 3]], f: &[[i32; 3]], geom: bool) -> Vec<u8> {
    let mut b = vec![0xff, 0xff, 0xfe];
    b.extend_from_slice(b"created by probe on Mon Nov 23 00:00:00 2026\n\n");
    be4(&mut b, v.len() as i32);
    be4(&mut b, f.len() as i32);
    for p in v {
        for &x in p {
            bef(&mut b, x);
        }
    }
    for t in f {
        for &x in t {
            be4(&mut b, x);
        }
    }
    if geom {
        be4(&mut b, 20);
        b.extend_from_slice(&vg_block());
    }
    b
}

/// A curvature file of the new binary format: one value per vertex (`nf` faces in its header).
pub fn curv_file(vals: &[f32], nf: i32) -> Vec<u8> {
    let mut b = vec![0xff, 0xff, 0xff];
    be4(&mut b, vals.len() as i32);
    be4(&mut b, nf);
    be4(&mut b, 1);
    for &x in vals {
        bef(&mut b, x);
    }
    b
}

/// An ASCII label: `(vno, x, y, z, stat)` per point, in `coords` (`TkReg` or `scanner`).
pub fn label_file(points: &[(i32, String, String, String, String)], coords: &str) -> Vec<u8> {
    let mut s = format!(
        "#!ascii label  , from subject subj vox2ras={coords}\n{}\n",
        points.len()
    );
    for (vno, x, y, z, st) in points {
        s += &format!("{vno} {x} {y} {z} {st}\n");
    }
    s.into_bytes()
}

/// `0.25 k` as the label's text (exact).
fn q(k: i64) -> String {
    format!("{:.3}", k as f64 * 0.25)
}

/// The colour table of [`LUT`] in FreeSurfer's binary form (the annotation's).
fn binary_lut() -> Vec<u8> {
    let t = tempfile::tempdir().expect("a temporary folder");
    std::fs::write(t.path().join("lut.txt"), LUT).expect("the table");
    let mut fs = Globals::new(UpstreamDefects::Fix);
    let (mut o, mut e) = (Vec::new(), Vec::new());
    let ct = ctab_read_ascii(&mut fs, &mut o, &mut e, Some(t.path()), "lut.txt");
    {
        let mut fp = CFile::open(Some(t.path()), "ct.bin", "w").expect("a file");
        ctab_write_into_binary(&mut fs, &mut o, &mut e, ct.as_ref(), Some(&mut fp));
    }
    std::fs::read(t.path().join("ct.bin")).expect("the table's bytes")
}

/// An annotation file of `annots` (one per vertex), with the colour table when `ct`.
fn annot_file(annots: &[i32], ct: bool) -> Vec<u8> {
    let mut b = Vec::new();
    be4(&mut b, annots.len() as i32);
    for (k, &a) in annots.iter().enumerate() {
        be4(&mut b, k as i32);
        be4(&mut b, a);
    }
    if ct {
        be4(&mut b, 1);
        b.extend_from_slice(&binary_lut());
    }
    b
}

/// The annotation value of a colour.
fn annot_of(r: i32, g: i32, b: i32) -> i32 {
    r + (g << 8) + (b << 16)
}

/// A `register.dat` of `m` (row by row) for `subject`.
fn reg_dat(subject: &str, m: &[&str; 16]) -> Vec<u8> {
    let mut s = format!("{subject}\n1.000000\n1.500000\n0.150000\n");
    for r in 0..4 {
        s += &format!(
            "{} {} {} {}\n",
            m[4 * r],
            m[4 * r + 1],
            m[4 * r + 2],
            m[4 * r + 3]
        );
    }
    s += "round\n";
    s.into_bytes()
}

/// An `.lta` of the identity between two 12^3 geometries, subject `subj`.
fn lta_text() -> Vec<u8> {
    let g = "valid = 1  # volume info valid\nfilename = orig.mgz\nvolume = 12 12 12\nvoxelsize = 1.000000000000000e+00 1.000000000000000e+00 1.000000000000000e+00\nxras   = -1.000000000000000e+00 0.000000000000000e+00 0.000000000000000e+00\nyras   = 0.000000000000000e+00 0.000000000000000e+00 -1.000000000000000e+00\nzras   = 0.000000000000000e+00 1.000000000000000e+00 0.000000000000000e+00\ncras   = 1.500000000000000e+00 -2.000000000000000e+00 5.000000000000000e-01\n";
    format!(
        "# transform file x.lta\n# created by probe on Sun Nov 22 00:00:00 2026\n\ntype      = 0 # LINEAR_VOX_TO_VOX\nnxforms   = 1\nmean      = 0.0000 0.0000 0.0000\nsigma     = 1.0000\n1 4 4\n1.000000000000000e+00 0.000000000000000e+00 0.000000000000000e+00 0.000000000000000e+00\n0.000000000000000e+00 1.000000000000000e+00 0.000000000000000e+00 0.000000000000000e+00\n0.000000000000000e+00 0.000000000000000e+00 1.000000000000000e+00 0.000000000000000e+00\n0.000000000000000e+00 0.000000000000000e+00 0.000000000000000e+00 1.000000000000000e+00\nsrc volume info\n{g}dst volume info\n{g}subject subj\nfscale 0.100000\n"
    )
    .into_bytes()
}

/// A segmentation of `dims` whose labels are 0, 2, 3, 17, 41 in blocks (a seeded pick per block of
/// two voxels).
fn seg_volume(g: &mut Gen, ty: i32, dims: [i32; 3], size: [f32; 3], ctab: bool) -> Mri {
    seg_volume_of(g, ty, dims, size, ctab, &[0.0f32, 2.0, 3.0, 17.0, 41.0])
}

/// A segmentation of `dims` whose labels are `labels` in blocks of two voxels (a seeded pick each).
fn seg_volume_of(
    g: &mut Gen,
    ty: i32,
    dims: [i32; 3],
    size: [f32; 3],
    ctab: bool,
    labels: &[f32],
) -> Mri {
    let mut m = build(
        g,
        &Spec {
            ctab,
            ..spec(ty, [dims[0], dims[1], dims[2], 1], size)
        },
    );
    let mut fs = Globals::new(UpstreamDefects::Fix);
    let mut e = Vec::new();
    let nb = ((dims[0] + 1) / 2 * ((dims[1] + 1) / 2) * ((dims[2] + 1) / 2)) as usize;
    let picks: Vec<f32> = (0..nb).map(|_| g.pick(labels)).collect();
    for z in 0..dims[2] {
        for y in 0..dims[1] {
            for x in 0..dims[0] {
                let b = ((z / 2) * ((dims[1] + 1) / 2) + y / 2) * ((dims[0] + 1) / 2) + x / 2;
                mri_set_vox_val(&mut fs, &mut e, &mut m, x, y, z, 0, picks[b as usize], 0)
                    .expect("a voxel");
            }
        }
    }
    m
}

/// The fixed inputs (the same for every seed).
pub fn base_inputs() -> Inputs {
    let mut g = Gen::new(0x1abe_1270_0040);
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    let put = |files: &mut Vec<(String, Vec<u8>)>, name: &str, m: &mut Mri| {
        files.extend(write_as(m, name));
    };
    // templates
    let mut t = build(&mut g, &spec(MRI_FLOAT, [6, 5, 4, 1], [1.0, 1.5, 2.0]));
    put(&mut files, "t.mgz", &mut t);
    let mut t2 = build(&mut g, &spec(MRI_UCHAR, [4, 4, 3, 1], [2.0, 2.0, 2.0]));
    put(&mut files, "t2.mgz", &mut t2);
    let mut t3 = build(&mut g, &spec(MRI_INT, [3, 3, 2, 2], [3.0, 2.5, 4.0]));
    put(&mut files, "t3.nii.gz", &mut t3);
    let mut t1 = build(&mut g, &spec(MRI_FLOAT, [1, 1, 1, 1], [16.0, 16.0, 16.0]));
    put(&mut files, "t1.mgz", &mut t1);
    // a template whose voxel size is NaN (from its bits): no matrix
    let mut tn = build(&mut g, &spec(MRI_FLOAT, [3, 3, 3, 1], [1.0, 1.0, 1.0]));
    tn.vg.xsize = f32::from_bits(0x7fc0_0000);
    put(&mut files, "tnan.mgz", &mut tn);
    // the defects' templates: orig (uchar), int with a colour table, float of two frames
    let mut orig = build(&mut g, &spec(MRI_UCHAR, [12, 12, 12, 1], [1.0, 1.0, 1.0]));
    put(&mut files, "orig.mgz", &mut orig);
    let mut origi = build(
        &mut g,
        &Spec {
            ctab: true,
            ..spec(MRI_INT, [12, 12, 12, 1], [1.0, 1.0, 1.0])
        },
    );
    put(&mut files, "origi.mgz", &mut origi);
    let mut origf = build(&mut g, &spec(MRI_FLOAT, [12, 12, 12, 2], [1.0, 1.0, 1.0]));
    put(&mut files, "origf.mgz", &mut origf);
    let mut origb = build(&mut g, &spec(MRI_FLOAT, [12, 12, 12, 1], [2.0, 1.0, 1.0]));
    put(&mut files, "origb.mgz", &mut origb);
    // segmentations
    let mut s1 = seg_volume(&mut g, MRI_INT, [12, 12, 12], [1.0, 1.0, 1.0], true);
    put(&mut files, "seg.mgz", &mut s1);
    put(&mut files, "subj/mri/aparc+aseg.mgz", &mut s1);
    let mut s2 = seg_volume(&mut g, MRI_UCHAR, [6, 5, 4], [1.0, 1.5, 2.0], false);
    put(&mut files, "segn.mgz", &mut s2);
    let mut s3 = seg_volume(&mut g, MRI_INT, [8, 8, 8], [0.5, 0.5, 0.5], true);
    put(&mut files, "segh.mgz", &mut s3);
    // the surface (66 vertices) and its files
    let (v, f) = sphere(4.0, [0.25, -0.5, 0.125]);
    let nv = v.len();
    files.push(("lh.orig.nofix".into(), triangle_file(&v, &f, true)));
    files.push(("lh.nogeom".into(), triangle_file(&v, &f, false)));
    files.push(("subj/surf/lh.white".into(), triangle_file(&v, &f, true)));
    let (v2, _) = sphere(3.0, [-0.5, 0.75, -0.25]);
    files.push(("subj/surf/lh.orig".into(), triangle_file(&v2, &f, true)));
    files.push(("subj/surf/rh.white".into(), triangle_file(&v2, &f, true)));
    let thick: Vec<f32> = (0..nv).map(|_| g.int(4, 14) as f32 * 0.25).collect();
    files.push((
        "subj/surf/lh.thickness".into(),
        curv_file(&thick, f.len() as i32),
    ));
    files.push((
        "subj/surf/rh.thickness".into(),
        curv_file(&thick, f.len() as i32),
    ));
    // defect numbers: 0 to 3; large; NaN and infinities from their bits; a wrong count
    let dn: Vec<f32> = (0..nv)
        .map(|_| if g.chance(2) { 0.0 } else { g.int(1, 3) as f32 })
        .collect();
    files.push(("lh.defect_labels".into(), curv_file(&dn, f.len() as i32)));
    let mut big = dn.clone();
    big[3] = 2_147_483_000.0;
    files.push(("lh.defect_big".into(), curv_file(&big, f.len() as i32)));
    let mut nan = dn.clone();
    nan[1] = f32::from_bits(0x7fc0_0001);
    nan[5] = f32::from_bits(0xff80_0000);
    nan[7] = f32::from_bits(0xffc0_0000);
    files.push(("lh.defect_nan".into(), curv_file(&nan, f.len() as i32)));
    files.push((
        "lh.defect_short".into(),
        curv_file(&dn[..nv - 2], f.len() as i32),
    ));
    let mut neg = dn.clone();
    neg[2] = -4.0;
    files.push(("lh.defect_neg".into(), curv_file(&neg, f.len() as i32)));
    // labels: volume points in t.mgz's tkregister space, surface labels, odd ones
    let vol_points =
        |g: &mut Gen, n: usize, span: i64| -> Vec<(i32, String, String, String, String)> {
            (0..n)
                .map(|_| {
                    (
                        -1,
                        q(g.int(-span, span)),
                        q(g.int(-span, span)),
                        q(g.int(-span, span)),
                        q(g.int(-8, 8)),
                    )
                })
                .collect()
        };
    let a = vol_points(&mut g, 25, 18);
    files.push(("a.label".into(), label_file(&a, "TkReg")));
    let b = vol_points(&mut g, 30, 14);
    files.push(("b.label".into(), label_file(&b, "TkReg")));
    let mut c = vol_points(&mut g, 12, 30);
    c[2].1 = "nan".into();
    c[4].2 = "-inf".into();
    c[6].3 = "1e30".into();
    files.push(("c.label".into(), label_file(&c, "TkReg")));
    let surf_points = |g: &mut Gen, vs: &[[f32; 3]], k: u64| {
        let mut pts = Vec::new();
        for (i, p) in vs.iter().enumerate() {
            if g.chance(k) {
                pts.push((
                    i as i32,
                    format!("{}", p[0]),
                    format!("{}", p[1]),
                    format!("{}", p[2]),
                    q(g.int(-8, 8)),
                ));
            }
        }
        pts
    };
    let s = surf_points(&mut g, &v, 2);
    files.push(("s.label".into(), label_file(&s, "TkReg")));
    files.push(("subj/label/lh.s.label".into(), label_file(&s, "TkReg")));
    let cortex = surf_points(&mut g, &v, 3);
    files.push(("lh.cortex.label".into(), label_file(&cortex, "TkReg")));
    let mut sbad = s.clone();
    sbad.push((70, "1".into(), "1".into(), "1".into(), "0".into()));
    files.push(("sbad.label".into(), label_file(&sbad, "TkReg")));
    let mut sneg = s.clone();
    sneg.push((-3, "1".into(), "1".into(), "1".into(), "0".into()));
    files.push(("sneg.label".into(), label_file(&sneg, "TkReg")));
    let scan = vol_points(&mut g, 15, 16);
    files.push(("scan.label".into(), label_file(&scan, "scanner")));
    files.push(("empty.label".into(), label_file(&[], "TkReg")));
    files.push(("bad.label".into(), b"not a label\n".to_vec()));
    // an annotation of the surface's vertices (the table's colours; one not in it)
    let cols = [
        annot_of(0, 0, 0),
        annot_of(245, 245, 245),
        annot_of(205, 62, 78),
        annot_of(220, 216, 20),
        annot_of(0, 225, 0),
    ];
    let ann: Vec<i32> = (0..nv)
        .map(|_| {
            if g.chance(12) {
                annot_of(1, 2, 3)
            } else {
                g.pick(&cols)
            }
        })
        .collect();
    files.push(("subj/label/lh.a.annot".into(), annot_file(&ann, true)));
    files.push((
        "subj/label/lh.zero.annot".into(),
        annot_file(&vec![annot_of(1, 2, 3); nv], true),
    ));
    files.push(("lh.short.annot".into(), annot_file(&ann[..10], true)));
    // registrations
    let ident = [
        "1", "0", "0", "0", "0", "1", "0", "0", "0", "0", "1", "0", "0", "0", "0", "1",
    ];
    let shift = [
        "1", "0", "0", "1.5", "0", "1", "0", "-0.75", "0", "0", "1", "0.5", "0", "0", "0", "1",
    ];
    let rot = [
        "0", "1", "0", "0.25", "-1", "0", "0", "0", "0", "0", "1", "-1", "0", "0", "0", "1",
    ];
    let sing = [
        "1", "0", "0", "0", "1", "0", "0", "0", "0", "0", "0", "0", "0", "0", "0", "1",
    ];
    files.push(("reg.dat".into(), reg_dat("subj", &shift)));
    files.push(("regi.dat".into(), reg_dat("subj", &ident)));
    files.push(("regr.dat".into(), reg_dat("subj", &rot)));
    files.push(("regs.dat".into(), reg_dat("subj", &sing)));
    files.push(("badreg.dat".into(), b"subj\n1\n".to_vec()));
    files.push(("x.lta".into(), lta_text()));
    files.push(("notvol.txt".into(), b"not a volume\n".to_vec()));
    files.push(("fsh/FreeSurferColorLUT.txt".into(), LUT.to_vec()));
    part2_inputs(&mut files, &v, &f, &thick);
    let dirs = vec!["outdir".into()];
    Inputs { files, dirs }
}

/// The colour table of the tissue-type cases with labels the schema gives no type (1) and
/// another of white matter (77).
const LUT2: &[u8] = b"0 Unknown 0 0 0 0\n1 Left-Cerebral-Exterior 70 130 180 0\n2 Left-Cerebral-White-Matter 245 245 245 0\n3 Left-Cerebral-Cortex 205 62 78 0\n17 Left-Hippocampus 220 216 20 0\n41 Right-Cerebral-White-Matter 0 225 0 0\n77 WM-hypointensities 200 70 255 0\n";

/// A dense colour table (indices 0, 1, 2) for an annotation whose every index has vertices.
const LUT3: &[u8] = b"0 Unknown 0 0 0 0\n1 lh-one 10 20 30 0\n2 lh-two 40 50 60 0\n";

/// `lut` in FreeSurfer's binary form (an annotation's table).
fn binary_of(lut: &[u8]) -> Vec<u8> {
    let t = tempfile::tempdir().expect("a temporary folder");
    std::fs::write(t.path().join("lut.txt"), lut).expect("the table");
    let mut fs = Globals::new(UpstreamDefects::Fix);
    let (mut o, mut e) = (Vec::new(), Vec::new());
    let ct = ctab_read_ascii(&mut fs, &mut o, &mut e, Some(t.path()), "lut.txt");
    {
        let mut fp = CFile::open(Some(t.path()), "ct.bin", "w").expect("a file");
        ctab_write_into_binary(&mut fs, &mut o, &mut e, ct.as_ref(), Some(&mut fp));
    }
    std::fs::read(t.path().join("ct.bin")).expect("the table's bytes")
}

/// A transform file of type `ty` from geometry `src` to `dst` (each `(dims, sizes)` with the
/// probe's direction cosines and centre) with `rows` as its matrix.
fn lta_of(ty: i32, rows: [[f64; 4]; 4], src: ([i32; 3], [f64; 3]), dst: ([i32; 3], [f64; 3])) -> Vec<u8> {
    let geom = |(d, z): ([i32; 3], [f64; 3])| {
        format!(
            "valid = 1  # volume info valid\nfilename = vol.mgz\nvolume = {} {} {}\nvoxelsize = {:.15e} {:.15e} {:.15e}\nxras   = -1.000000000000000e+00 0.000000000000000e+00 0.000000000000000e+00\nyras   = 0.000000000000000e+00 0.000000000000000e+00 -1.000000000000000e+00\nzras   = 0.000000000000000e+00 1.000000000000000e+00 0.000000000000000e+00\ncras   = 1.500000000000000e+00 -2.000000000000000e+00 5.000000000000000e-01\n",
            d[0], d[1], d[2], z[0], z[1], z[2]
        )
    };
    let mut m = String::new();
    for r in rows {
        m += &format!("{:.15e} {:.15e} {:.15e} {:.15e}\n", r[0], r[1], r[2], r[3]);
    }
    format!(
        "# transform file probe.lta\n# created by probe on Sun Nov 22 00:00:00 2026\n\ntype      = {ty} # LINEAR_VOX_TO_VOX\nnxforms   = 1\nmean      = 0.0000 0.0000 0.0000\nsigma     = 1.0000\n1 4 4\n{m}src volume info\n{}dst volume info\n{}subject subj\nfscale 0.100000\n",
        geom(src),
        geom(dst)
    )
    .into_bytes()
}

/// Part 2's inputs (relay 57, a generator of their own so that part 1's stay as they were): the
/// ribbons (`<subject>/mri/ribbon.mgz`: 3 and 42 with other values; one of 8 mm voxels far from
/// the surface), a second subjects folder, transforms with the ribbon at the source, at the
/// destination and at neither end, segmentations with labels below 0 and NaN (from their bits), a
/// large label, labels without a tissue type or not in the colour table, one of 0.4 mm voxels,
/// a dense annotation and one with gaps.
fn part2_inputs(
    files: &mut Vec<(String, Vec<u8>)>,
    v: &[[f32; 3]],
    f: &[[i32; 3]],
    thick: &[f32],
) {
    let mut g = Gen::new(0x1abe_1270_0057);
    let put = |files: &mut Vec<(String, Vec<u8>)>, name: &str, m: &mut Mri| {
        files.extend(write_as(m, name));
    };
    let ribvals = [0.0f32, 3.0, 3.0, 42.0, 42.0, 2.0, 41.0];
    let mut rib = seg_volume_of(&mut g, MRI_UCHAR, [12, 12, 12], [1.0, 1.0, 1.0], true, &ribvals);
    put(files, "subj/mri/ribbon.mgz", &mut rib);
    let mut rib2 = seg_volume_of(&mut g, MRI_INT, [12, 12, 12], [1.0, 1.0, 1.0], false, &ribvals);
    put(files, "sd2/subj/mri/ribbon.mgz", &mut rib2);
    let (v2, _) = sphere(3.0, [-0.5, 0.75, -0.25]);
    files.push(("sd2/subj/surf/lh.white".into(), triangle_file(&v2, f, true)));
    files.push((
        "sd2/subj/surf/lh.thickness".into(),
        curv_file(thick, f.len() as i32),
    ));
    let mut far = seg_volume_of(&mut g, MRI_UCHAR, [8, 8, 8], [8.0, 8.0, 8.0], false, &[3.0, 3.0, 42.0]);
    put(files, "subjfar/mri/ribbon.mgz", &mut far);
    files.push(("subjfar/surf/lh.white".into(), triangle_file(v, f, true)));
    files.push((
        "subjfar/surf/lh.thickness".into(),
        curv_file(thick, f.len() as i32),
    ));
    // transforms: the ribbon at the destination (from t.mgz's grid), at neither end
    let shift = [
        [1.0, 0.0, 0.0, 2.0],
        [0.0, 1.0, 0.0, 1.0],
        [0.0, 0.0, 1.0, 3.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    files.push((
        "y.lta".into(),
        lta_of(0, shift, ([6, 5, 4], [1.0, 1.5, 2.0]), ([12, 12, 12], [1.0, 1.0, 1.0])),
    ));
    files.push((
        "z.lta".into(),
        lta_of(0, shift, ([6, 5, 4], [1.0, 1.5, 2.0]), ([4, 4, 3], [2.0, 2.0, 2.0])),
    ));
    // segmentations of odd labels
    let nan = f32::from_bits(0x7fc0_0000);
    let mut sneg = seg_volume_of(&mut g, MRI_FLOAT, [12, 12, 12], [1.0, 1.0, 1.0], true, &[0.0, 2.0, -1.0, 17.0, -3.0, 41.0]);
    put(files, "segneg.mgz", &mut sneg);
    let mut snan = seg_volume_of(&mut g, MRI_FLOAT, [6, 5, 4], [1.0, 1.5, 2.0], false, &[0.0, 2.0, nan, 3.0]);
    put(files, "segnan.mgz", &mut snan);
    let mut sbig = seg_volume_of(&mut g, MRI_INT, [6, 5, 4], [1.0, 1.5, 2.0], false, &[0.0, 3.0, 1_000_000.0, 41.0]);
    put(files, "segbig.mgz", &mut sbig);
    let mut su = seg_volume_of(&mut g, MRI_INT, [12, 12, 12], [1.0, 1.0, 1.0], true, &[0.0, 1.0, 3.0, 17.0, 41.0, 77.0]);
    put(files, "segu.mgz", &mut su);
    let mut sx = seg_volume_of(&mut g, MRI_INT, [12, 12, 12], [1.0, 1.0, 1.0], false, &[0.0, 3.0, 99.0, 41.0]);
    put(files, "segx.mgz", &mut sx);
    let mut sq = seg_volume_of(&mut g, MRI_INT, [12, 12, 12], [0.4, 0.4, 0.4], true, &[0.0, 2.0, 3.0, 17.0]);
    put(files, "segq.mgz", &mut sq);
    let mut s0 = seg_volume_of(&mut g, MRI_INT, [6, 5, 4], [1.0, 1.5, 2.0], false, &[0.0]);
    put(files, "seg0.mgz", &mut s0);
    files.push(("fsh2/FreeSurferColorLUT.txt".into(), LUT2.to_vec()));
    // annotations: every index of a dense table, and one with gaps
    let dense = [annot_of(0, 0, 0), annot_of(10, 20, 30), annot_of(40, 50, 60)];
    let ann: Vec<i32> = (0..v.len()).map(|_| g.pick(&dense)).collect();
    let mut b = Vec::new();
    be4(&mut b, ann.len() as i32);
    for (k, &a) in ann.iter().enumerate() {
        be4(&mut b, k as i32);
        be4(&mut b, a);
    }
    be4(&mut b, 1);
    b.extend_from_slice(&binary_of(LUT3));
    files.push(("subj/label/lh.dense.annot".into(), b));
    files.push(("subj/label/rh.dense.annot".into(), files.last().expect("the annotation").1.clone()));
}

/// Whether argument `a` names file `f` (or a folder holding it).
fn names(a: &str, f: &str) -> bool {
    let a = a.trim_end_matches('/');
    !a.is_empty() && (f == a || f.strip_prefix(a).is_some_and(|r| r.starts_with('/')))
}

/// A case line: the environment, the files the arguments name (and the subject's folder when an
/// option reads it), the folders they name.
pub fn line(env: &[&str], i: &Inputs, args: &[String]) -> String {
    let subj = args.iter().any(|a| {
        matches!(
            a.as_str(),
            "--subject" | "--annot" | "--proj" | "--aparc+aseg" | "--lta" | "--fill-ribbon"
        )
    });
    let far = args.iter().any(|a| a == "subjfar");
    let files: Vec<&(String, Vec<u8>)> = i
        .files
        .iter()
        .filter(|(f, _)| {
            (subj && f.starts_with("subj/"))
                || (far && f.starts_with("subjfar/"))
                || args.iter().any(|a| names(a, f))
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
    // the default colour table the tissue types and --fill-ribbon's environment read
    let ttype = args
        .iter()
        .any(|a| a == "--ttype" || a == "--ttype+head" || a == "--fill-ribbon");
    let fsh2 = env.iter().any(|e| e.contains("@DIR@/fsh2"));
    let files: Vec<&(String, Vec<u8>)> = files
        .into_iter()
        .chain(i.files.iter().filter(|(f, _)| {
            (ttype && f.starts_with("fsh/")) || (fsh2 && f.starts_with("fsh2/"))
        }))
        .collect();
    let mut all_env: Vec<&str> = vec!["SUBJECTS_DIR=@DIR@", "FREESURFER_SEED=1234"];
    if ttype {
        // the default colour table the tissue types read: FREESURFER_HOME's
        all_env.push("FREESURFER_HOME=@DIR@/fsh");
    }
    all_env.extend_from_slice(env);
    let mut t = vec![
        "cat".to_owned(),
        hex(all_env.join(";").as_bytes()),
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

/// The fixed cases: the command line, every check, every option and path.
fn prelude() -> Vec<String> {
    let i = base_inputs();
    let mut out = Vec::new();
    let mut add_env = |env: &[&str], a: &[&str]| out.push(line(env, &i, &s(a)));
    // the command line
    for a in [
        &[][..],
        &["--help"],
        &["--HELP", "--o", "o.mgz"],
        &["--version"],
        &["--VERSION"],
        &["--label", "a.label", "--version", "--o", "o.mgz"],
        &["-o", "o.mgz"],
        &["--bogus"],
        &["--o"],
        &["--label"],
        &["--temp"],
        &["--reg"],
        &["--proj", "abs", "0"],
        &[
            "--defects",
            "lh.orig.nofix",
            "lh.defect_labels",
            "orig.mgz",
            "1000",
            "0",
        ],
        &["--surf"],
        &["--label-stat"],
        &["--annot"],
        &["--seg"],
        &["--upsample"],
        &["--resmm"],
        &["--tkr-template"],
        &["--lta"],
        &["--fillthresh"],
        &["--labvoxvol"],
        &["--subject"],
        &["--stat-thresh"],
        &["--hemi"],
        &["--hits"],
        &["--pvf"],
        &["--sd"],
        &["--offset"],
        &["--hemi", "xh"],
        &["--proj", "bad", "0", "1", "0.5"],
        &["--proj", "abs", "1", "0", "0.5"],
        &["--proj", "abs", "0", "1", "0"],
        &["--proj", "abs", "0", "1", "-1"],
        &["--proj", "none", "0", "0", "0"],
        &["--tkr-template", "nofile.mgz"],
        &["--tkr-template", "notvol.txt"],
        &["--lta", "nofile.lta"],
        &["--regheader"],
        &["--regheader", "--temp", "t.mgz"],
        &["--pvf", "p.mgz", "--label", "a.label"],
        // check_options
        &["--o", "o.mgz"],
        &["--label", "a.label"],
        &["--aparc+aseg", "--o", "o.mgz"],
        &[
            "--label",
            "a.label",
            "--annot",
            "lh.a.annot",
            "--o",
            "o.mgz",
        ],
        &["--label", "a.label", "--seg", "seg.mgz", "--o", "o.mgz"],
        &[
            "--annot",
            "subj/label/lh.a.annot",
            "--seg",
            "seg.mgz",
            "--o",
            "o.mgz",
        ],
        &[
            "--label", "a.label", "--proj", "abs", "0", "1", "0.5", "--o", "o.mgz",
        ],
        &[
            "--label",
            "a.label",
            "--proj",
            "abs",
            "0",
            "1",
            "0.5",
            "--subject",
            "subj",
            "--o",
            "o.mgz",
        ],
        &[
            "--label",
            "a.label",
            "--o",
            "o.mgz",
            "--subject",
            "subj",
            "--hemi",
            "lh",
            "--identity",
            "--temp",
            "t.mgz",
        ],
        &["--seg", "seg.mgz", "--label-stat", "st.mgz", "--o", "o.mgz"],
        &["--label", "a.label", "--o", "o.mgz", "--temp", "t.mgz"],
        &[
            "--label",
            "a.label",
            "--o",
            "o.mgz",
            "--temp",
            "t.mgz",
            "--reg",
            "reg.dat",
            "--regheader",
            "t.mgz",
        ],
        &[
            "--label",
            "a.label",
            "--o",
            "o.mgz",
            "--temp",
            "t.mgz",
            "--reg",
            "reg.dat",
            "--identity",
        ],
        &[
            "--label",
            "a.label",
            "--o",
            "o.mgz",
            "--temp",
            "t.mgz",
            "--identity",
            "--regheader",
            "t.mgz",
        ],
        &["--label", "a.label", "--o", "o.mgz", "--identity"],
        &[
            "--label",
            "a.label",
            "--o",
            "o.mgz",
            "--identity",
            "--fill-ribbon",
        ],
        // the template and the registration
        &[
            "--label",
            "a.label",
            "--o",
            "o.mgz",
            "--identity",
            "--temp",
            "nofile.mgz",
        ],
        &[
            "--label",
            "a.label",
            "--o",
            "o.mgz",
            "--identity",
            "--temp",
            "notvol.txt",
        ],
        &[
            "--label",
            "a.label",
            "--o",
            "o.mgz",
            "--identity",
            "--temp",
            "tnan.mgz",
        ],
        &[
            "--label",
            "a.label",
            "--o",
            "o.mgz",
            "--temp",
            "t.mgz",
            "--reg",
            "nofile.dat",
        ],
        &[
            "--label",
            "a.label",
            "--o",
            "o.mgz",
            "--temp",
            "t.mgz",
            "--reg",
            "badreg.dat",
        ],
        &[
            "--label", "a.label", "--o", "o.mgz", "--temp", "t.mgz", "--reg", "reg.dat",
        ],
        &[
            "--label",
            "a.label",
            "--o",
            "o.mgz",
            "--temp",
            "t.mgz",
            "--reg",
            "regr.dat",
            "--invertmtx",
        ],
        &[
            "--label",
            "a.label",
            "--o",
            "o.mgz",
            "--temp",
            "t.mgz",
            "--reg",
            "regs.dat",
            "--invertmtx",
        ],
        &[
            "--label", "a.label", "--o", "o.mgz", "--temp", "t.mgz", "--reg", "x.lta",
        ],
        &[
            "--label",
            "a.label",
            "--o",
            "o.mgz",
            "--temp",
            "t.mgz",
            "--regheader",
            "t2.mgz",
        ],
        &[
            "--label",
            "a.label",
            "--o",
            "o.mgz",
            "--temp",
            "t.mgz",
            "--regheader",
            "nofile.mgz",
        ],
        &[
            "--label",
            "a.label",
            "--o",
            "o.mgz",
            "--temp",
            "t.mgz",
            "--identity",
            "--native-vox2ras",
        ],
        // labels
        &[
            "--label",
            "a.label",
            "--o",
            "o.mgz",
            "--temp",
            "t.mgz",
            "--identity",
        ],
        &[
            "--label",
            "a.label",
            "--label",
            "b.label",
            "--o",
            "o.mgz",
            "--temp",
            "t.mgz",
            "--identity",
            "--hits",
            "h.mgz",
        ],
        &[
            "--label",
            "a.label",
            "--label",
            "b.label",
            "--o",
            "o.mgz",
            "--temp",
            "t2.mgz",
            "--identity",
            "--fillthresh",
            "0.05",
        ],
        &[
            "--label",
            "a.label",
            "--label",
            "b.label",
            "--o",
            "o.mgz",
            "--temp",
            "t2.mgz",
            "--identity",
            "--fillthresh",
            "0.5",
            "--labvoxvol",
            "0.25",
        ],
        &[
            "--label",
            "a.label",
            "--o",
            "o.mgz",
            "--temp",
            "t3.nii.gz",
            "--identity",
            "--offset",
            "100",
        ],
        &[
            "--label",
            "a.label",
            "--o",
            "o.nii.gz",
            "--temp",
            "t.mgz",
            "--identity",
            "--offset",
            "-1",
        ],
        &[
            "--label",
            "a.label",
            "--o",
            "o.mgz",
            "--temp",
            "t.mgz",
            "--identity",
            "--offset",
            "2147483647",
        ],
        &[
            "--label",
            "a.label",
            "--o",
            "o.mgz",
            "--temp",
            "t.mgz",
            "--identity",
            "--offset",
            "2147483646",
        ],
        &[
            "--label",
            "a.label",
            "--o",
            "o.mgz",
            "--temp",
            "t.mgz",
            "--identity",
            "--label-stat",
            "st.mgz",
        ],
        &[
            "--label",
            "a.label",
            "--o",
            "o.mgz",
            "--temp",
            "t.mgz",
            "--identity",
            "--label-stat",
            "st.mgz",
            "--stat-thresh",
            "0.5",
        ],
        &[
            "--label",
            "a.label",
            "--o",
            "o.mgz",
            "--temp",
            "t.mgz",
            "--identity",
            "--stat-thresh",
            "x",
        ],
        &[
            "--label",
            "a.label",
            "--o",
            "o.mgz",
            "--temp",
            "t.mgz",
            "--identity",
            "--pvf",
            "p.mgz",
        ],
        &[
            "--label",
            "a.label",
            "--label",
            "b.label",
            "--o",
            "o.mgz",
            "--temp",
            "t2.mgz",
            "--identity",
            "--pvf",
            "p.mgz",
            "--hits",
            "h.mgz",
        ],
        &[
            "--label",
            "c.label",
            "--o",
            "o.mgz",
            "--temp",
            "t.mgz",
            "--identity",
            "--hits",
            "h.mgz",
        ],
        &[
            "--label",
            "a.label",
            "--o",
            "o.mgz",
            "--temp",
            "t1.mgz",
            "--identity",
            "--hits",
            "h.mgz",
        ],
        &[
            "--label",
            "empty.label",
            "--o",
            "o.mgz",
            "--temp",
            "t.mgz",
            "--identity",
        ],
        &[
            "--label",
            "nofile.label",
            "--o",
            "o.mgz",
            "--temp",
            "t.mgz",
            "--identity",
        ],
        &[
            "--label",
            "bad.label",
            "--o",
            "o.mgz",
            "--temp",
            "t.mgz",
            "--identity",
        ],
        &[
            "--label",
            "scan.label",
            "--o",
            "o.mgz",
            "--temp",
            "t.mgz",
            "--identity",
        ],
        &[
            "--label",
            "scan.label",
            "--o",
            "o.mgz",
            "--temp",
            "t.mgz",
            "--identity",
            "--tkr-template",
            "t.mgz",
        ],
        &[
            "--label",
            "a.label",
            "--o",
            "o.mgz",
            "--temp",
            "t.mgz",
            "--identity",
            "--debug",
        ],
        &[
            "--label",
            "a.label",
            "--o",
            "outdir/o.mgz",
            "--temp",
            "t.mgz",
            "--identity",
        ],
        &[
            "--label",
            "a.label",
            "--o",
            "nodir/o.mgz",
            "--temp",
            "t.mgz",
            "--identity",
        ],
        &[
            "--label",
            "a.label",
            "--o",
            "o.mgz",
            "--temp",
            "t.mgz",
            "--identity",
            "--label-stat",
            "nodir/st.mgz",
        ],
        &[
            "--label",
            "a.label",
            "--o",
            "o.mgz",
            "--temp",
            "t.mgz",
            "--identity",
            "--sd",
            "outdir",
        ],
        &[
            "--label",
            "a.label",
            "--o",
            "o.mgz",
            "--temp",
            "t.mgz",
            "--identity",
            "--lta",
            "x.lta",
        ],
        &[
            "--label",
            "s.label",
            "--o",
            "o.mgz",
            "--temp",
            "orig.mgz",
            "--reg",
            "x.lta",
            "--proj",
            "abs",
            "0",
            "1",
            "0.5",
            "--subject",
            "subj",
            "--hemi",
            "lh",
        ],
        &[
            "--seg",
            "seg.mgz",
            "--temp",
            "t.mgz",
            "--regheader",
            "--o",
            "o.mgz",
            "--resmm",
            "0.5",
        ],
        // projections along the normal
        &[
            "--label",
            "s.label",
            "--o",
            "o.mgz",
            "--temp",
            "orig.mgz",
            "--identity",
            "--proj",
            "abs",
            "-1",
            "1",
            "0.5",
            "--subject",
            "subj",
            "--hemi",
            "lh",
        ],
        &[
            "--label",
            "s.label",
            "--o",
            "o.mgz",
            "--temp",
            "orig.mgz",
            "--identity",
            "--proj",
            "frac",
            "0",
            "1",
            "0.25",
            "--subject",
            "subj",
            "--hemi",
            "lh",
            "--hits",
            "h.mgz",
        ],
        &[
            "--label",
            "s.label",
            "--o",
            "o.mgz",
            "--temp",
            "orig.mgz",
            "--identity",
            "--proj",
            "frac",
            "0.5",
            "0.5",
            "0",
            "--subject",
            "subj",
            "--hemi",
            "lh",
        ],
        &[
            "--label",
            "s.label",
            "--o",
            "o.mgz",
            "--temp",
            "orig.mgz",
            "--identity",
            "--proj",
            "abs",
            "0",
            "2",
            "0.5",
            "--subject",
            "subj",
            "--hemi",
            "lh",
            "--surf",
            "orig",
            "--label-stat",
            "st.mgz",
        ],
        &[
            "--label",
            "s.label",
            "--o",
            "o.mgz",
            "--temp",
            "orig.mgz",
            "--identity",
            "--proj",
            "abs",
            "0",
            "1",
            "0.5",
            "--subject",
            "subj",
            "--hemi",
            "lh",
            "--surf",
            "pial",
        ],
        &[
            "--label",
            "s.label",
            "--o",
            "o.mgz",
            "--temp",
            "orig.mgz",
            "--identity",
            "--proj",
            "abs",
            "0",
            "1",
            "0.5",
            "--subject",
            "subj",
            "--hemi",
            "rh",
            "--debug",
        ],
        &[
            "--label",
            "s.label",
            "--o",
            "o.mgz",
            "--temp",
            "orig.mgz",
            "--identity",
            "--proj",
            "abs",
            "0",
            "1",
            "0.5",
            "--subject",
            "nosubj",
            "--hemi",
            "lh",
        ],
        &[
            "--label",
            "a.label",
            "--o",
            "o.mgz",
            "--temp",
            "orig.mgz",
            "--identity",
            "--proj",
            "abs",
            "0",
            "1",
            "0.5",
            "--subject",
            "subj",
            "--hemi",
            "lh",
        ],
        &[
            "--label",
            "sbad.label",
            "--o",
            "o.mgz",
            "--temp",
            "orig.mgz",
            "--identity",
            "--proj",
            "abs",
            "0",
            "1",
            "0.5",
            "--subject",
            "subj",
            "--hemi",
            "lh",
        ],
        &[
            "--label",
            "sneg.label",
            "--o",
            "o.mgz",
            "--temp",
            "orig.mgz",
            "--identity",
            "--proj",
            "abs",
            "0",
            "1",
            "0.5",
            "--subject",
            "subj",
            "--hemi",
            "lh",
        ],
        &[
            "--label",
            "s.label",
            "--o",
            "o.mgz",
            "--temp",
            "orig.mgz",
            "--identity",
            "--proj",
            "none",
            "0",
            "1",
            "0.5",
            "--subject",
            "subj",
            "--hemi",
            "lh",
        ],
        &[
            "--label",
            "s.label",
            "--o",
            "o.mgz",
            "--temp",
            "orig.mgz",
            "--identity",
            "--proj",
            "abs",
            "0",
            "inf",
            "0.5",
            "--subject",
            "subj",
            "--hemi",
            "lh",
        ],
        &[
            "--label",
            "s.label",
            "--o",
            "o.mgz",
            "--temp",
            "orig.mgz",
            "--identity",
            "--proj",
            "abs",
            "nan",
            "1",
            "0.5",
            "--subject",
            "subj",
            "--hemi",
            "lh",
        ],
        &[
            "--label",
            "s.label",
            "--o",
            "o.mgz",
            "--temp",
            "orig.mgz",
            "--identity",
            "--proj",
            "abs",
            "0",
            "1",
            "nan",
            "--subject",
            "subj",
            "--hemi",
            "lh",
        ],
        &[
            "--label",
            "s.label",
            "--o",
            "o.mgz",
            "--temp",
            "t1.mgz",
            "--identity",
            "--proj",
            "abs",
            "0",
            "1",
            "0.00002",
            "--subject",
            "subj",
            "--hemi",
            "lh",
            "--hits",
            "h.mgz",
        ],
        // annotations
        &[
            "--annot",
            "subj/label/lh.a.annot",
            "--o",
            "o.mgz",
            "--temp",
            "orig.mgz",
            "--identity",
            "--subject",
            "subj",
            "--hemi",
            "lh",
        ],
        &[
            "--annot",
            "subj/label/lh.a.annot",
            "--o",
            "o.mgz",
            "--temp",
            "orig.mgz",
            "--identity",
            "--subject",
            "subj",
            "--hemi",
            "lh",
            "--hits",
            "h.mgz",
            "--offset",
            "1000",
        ],
        &[
            "--annot",
            "subj/label/lh.a.annot",
            "--o",
            "o.mgz",
            "--temp",
            "t2.mgz",
            "--reg",
            "reg.dat",
            "--subject",
            "subj",
            "--hemi",
            "lh",
            "--proj",
            "frac",
            "0",
            "1",
            "0.5",
        ],
        &[
            "--annot",
            "subj/label/lh.zero.annot",
            "--o",
            "o.mgz",
            "--temp",
            "orig.mgz",
            "--identity",
            "--subject",
            "subj",
            "--hemi",
            "lh",
        ],
        &[
            "--annot",
            "lh.short.annot",
            "--o",
            "o.mgz",
            "--temp",
            "orig.mgz",
            "--identity",
            "--subject",
            "subj",
            "--hemi",
            "lh",
        ],
        &[
            "--annot",
            "noannot.annot",
            "--o",
            "o.mgz",
            "--temp",
            "orig.mgz",
            "--identity",
            "--subject",
            "subj",
            "--hemi",
            "lh",
        ],
        &[
            "--annot",
            "subj/label/lh.a.annot",
            "--o",
            "o.mgz",
            "--temp",
            "orig.mgz",
            "--identity",
            "--subject",
            "subj",
            "--hemi",
            "lh",
            "--pvf",
            "p.mgz",
        ],
        // segmentations
        &[
            "--seg",
            "seg.mgz",
            "--temp",
            "t.mgz",
            "--regheader",
            "--o",
            "o.mgz",
        ],
        &[
            "--seg",
            "seg.mgz",
            "--temp",
            "t.mgz",
            "--regheader",
            "seg.mgz",
            "--o",
            "o.mgz",
            "--hits",
            "h.mgz",
            "--pvf",
            "p.mgz",
        ],
        &[
            "--seg",
            "segn.mgz",
            "--temp",
            "t.mgz",
            "--identity",
            "--o",
            "o.mgz",
            "--fillthresh",
            "0.3",
        ],
        &[
            "--seg",
            "segh.mgz",
            "--temp",
            "t2.mgz",
            "--regheader",
            "--o",
            "o.mgz",
            "--upsample",
            "2",
        ],
        &[
            "--seg", "seg.mgz", "--temp", "orig.mgz", "--reg", "reg.dat", "--o", "o.mgz", "--ttype",
        ],
        &[
            "--seg",
            "seg.mgz",
            "--temp",
            "orig.mgz",
            "--regheader",
            "--o",
            "o.mgz",
            "--ttype+head",
        ],
        &[
            "--seg",
            "nofile.mgz",
            "--temp",
            "t.mgz",
            "--regheader",
            "--o",
            "o.mgz",
        ],
        &[
            "--seg",
            "seg.mgz",
            "--temp",
            "t.mgz",
            "--regheader",
            "--o",
            "nodir/o.mgz",
        ],
        &[
            "--seg",
            "seg.mgz",
            "--temp",
            "t.mgz",
            "--regheader",
            "--o",
            "o.mgz",
            "--new-aseg2vol",
            "--no-new-aseg2vol",
        ],
        &[
            "--seg",
            "seg.mgz",
            "--fill-ribbon",
            "--regheader",
            "--o",
            "o.mgz",
            "--subject",
            "subj",
            "--hemi",
            "lh",
        ],
        &[
            "--aparc+aseg",
            "--subject",
            "subj",
            "--temp",
            "t.mgz",
            "--regheader",
            "subj/mri/aparc+aseg.mgz",
            "--o",
            "o.mgz",
        ],
        &[
            "--aparc+aseg",
            "--temp",
            "t.mgz",
            "--reg",
            "reg.dat",
            "--o",
            "o.mgz",
        ],
        // the defects
        &[
            "--defects",
            "lh.orig.nofix",
            "lh.defect_labels",
            "orig.mgz",
            "1000",
            "0",
            "o.mgz",
        ],
        &[
            "--defects",
            "lh.orig.nofix",
            "lh.defect_labels",
            "orig.mgz",
            "1000",
            "0",
            "o.mgz",
            "lh.cortex.label",
        ],
        &[
            "--defects",
            "lh.orig.nofix",
            "lh.defect_labels",
            "origi.mgz",
            "2000",
            "1",
            "o.mgz",
            "lh.cortex.label",
        ],
        &[
            "--defects",
            "lh.orig.nofix",
            "lh.defect_labels",
            "origi.mgz",
            "0",
            "0",
            "o.mgz",
        ],
        &[
            "--defects",
            "lh.orig.nofix",
            "lh.defect_labels",
            "origf.mgz",
            "5",
            "0",
            "o.mgz",
        ],
        &[
            "--defects",
            "lh.orig.nofix",
            "lh.defect_labels",
            "origf.mgz",
            "5",
            "7",
            "o.nii.gz",
        ],
        &[
            "--defects",
            "lh.orig.nofix",
            "lh.defect_labels",
            "orig.mgz",
            "300",
            "1",
            "o.mgz",
        ],
        &[
            "--defects",
            "lh.orig.nofix",
            "lh.defect_labels",
            "origb.mgz",
            "10",
            "0",
            "o.mgz",
        ],
        &[
            "--defects",
            "lh.nogeom",
            "lh.defect_labels",
            "orig.mgz",
            "1000",
            "0",
            "o.mgz",
        ],
        &[
            "--defects",
            "lh.orig.nofix",
            "lh.defect_nan",
            "orig.mgz",
            "1000",
            "0",
            "o.mgz",
        ],
        &[
            "--defects",
            "lh.orig.nofix",
            "lh.defect_neg",
            "orig.mgz",
            "1000",
            "0",
            "o.mgz",
        ],
        &[
            "--defects",
            "lh.orig.nofix",
            "lh.defect_big",
            "orig.mgz",
            "1000",
            "0",
            "o.mgz",
        ],
        &[
            "--defects",
            "lh.orig.nofix",
            "lh.defect_short",
            "orig.mgz",
            "1000",
            "0",
            "o.mgz",
        ],
        &[
            "--defects",
            "lh.orig.nofix",
            "lh.defect_labels",
            "orig.mgz",
            "-5",
            "0",
            "o.mgz",
        ],
        &[
            "--defects",
            "lh.orig.nofix",
            "lh.defect_labels",
            "orig.mgz",
            "x",
            "0",
            "o.mgz",
        ],
        &[
            "--defects",
            "lh.orig.nofix",
            "lh.defect_labels",
            "orig.mgz",
            "1000",
            "y",
            "o.mgz",
        ],
        &[
            "--defects",
            "lh.orig.nofix",
            "lh.defect_labels",
            "orig.mgz",
            "1000",
            "0",
            "o.mgz",
            "--debug",
        ],
        &[
            "--defects",
            "lh.orig.nofix",
            "lh.defect_labels",
            "orig.mgz",
            "1000",
            "0",
            "o.mgz",
            "sbad.label",
        ],
        &[
            "--defects",
            "lh.orig.nofix",
            "lh.defect_labels",
            "orig.mgz",
            "1000",
            "0",
            "o.mgz",
            "nofile.label",
        ],
        &[
            "--defects",
            "nosurf",
            "lh.defect_labels",
            "orig.mgz",
            "1000",
            "0",
            "o.mgz",
        ],
        &[
            "--defects",
            "lh.orig.nofix",
            "nodefects",
            "orig.mgz",
            "1000",
            "0",
            "o.mgz",
        ],
        &[
            "--defects",
            "lh.orig.nofix",
            "lh.defect_labels",
            "nofile.mgz",
            "1000",
            "0",
            "o.mgz",
        ],
        &[
            "--defects",
            "lh.orig.nofix",
            "lh.defect_labels",
            "orig.mgz",
            "1000",
            "0",
            "nodir/o.mgz",
        ],
        &[
            "--o",
            "o.mgz",
            "--defects",
            "lh.orig.nofix",
            "lh.defect_labels",
            "orig.mgz",
            "1000",
            "0",
            "o2.mgz",
            "--label",
            "a.label",
        ],
    ] {
        add_env(&[], a);
    }
    // the environment
    add_env(
        &["-SUBJECTS_DIR"],
        &[
            "--label",
            "a.label",
            "--o",
            "o.mgz",
            "--temp",
            "t.mgz",
            "--identity",
        ],
    );
    add_env(
        &["-SUBJECTS_DIR"],
        &[
            "--label",
            "a.label",
            "--o",
            "o.mgz",
            "--temp",
            "t.mgz",
            "--identity",
            "--sd",
            "outdir",
        ],
    );
    add_env(
        &["FREESURFER_SEED=99"],
        &[
            "--defects",
            "lh.orig.nofix",
            "lh.defect_labels",
            "orig.mgz",
            "1000",
            "0",
            "o.mgz",
        ],
    );
    add_env(
        &["-SUBJECTS_DIR"],
        &[
            "--annot",
            "subj/label/lh.a.annot",
            "--o",
            "o.mgz",
            "--temp",
            "orig.mgz",
            "--identity",
            "--subject",
            "subj",
            "--hemi",
            "lh",
            "--sd",
            ".",
        ],
    );
    part2_prelude(&mut add_env);
    // more than 100 labels
    let mut many = Vec::new();
    for _ in 0..101 {
        many.push("a.label");
        many.push("--label");
    }
    many.pop();
    let mut a = vec!["--label"];
    a.extend(many);
    a.extend(["--o", "o.mgz", "--temp", "t.mgz", "--identity"]);
    add_env(&[], &a);
    out
}

/// Part 2's fixed cases (relay 57): `--new-aseg2vol` and `--fill-ribbon`, every path.
fn part2_prelude(add_env: &mut dyn FnMut(&[&str], &[&str])) {
    let nseg = |seg: &str, temp: &str, extra: &[&str]| -> Vec<String> {
        let mut a = s(&["--seg", seg, "--temp", temp, "--regheader", "--new-aseg2vol"]);
        a.extend(s(extra));
        a.extend(s(&["--o", "o.mgz"]));
        a
    };
    let cases: Vec<(Vec<&str>, Vec<String>)> = vec![
        (vec![], nseg("seg.mgz", "t2.mgz", &[])),
        (vec![], nseg("seg.mgz", "t.mgz", &[])),
        (vec![], nseg("seg.mgz", "t1.mgz", &["--resmm", "0.5"])),
        (vec![], nseg("seg.mgz", "t2.mgz", &["--ttype"])),
        (vec![], nseg("seg.mgz", "t1.mgz", &["--ttype", "--resmm", "1"])),
        (vec![], nseg("seg.mgz", "t1.mgz", &["--ttype+head", "--resmm", "2"])),
        (vec![], nseg("seg.mgz", "t3.nii.gz", &["--resmm", "-2.7"])),
        (vec![], nseg("seg.mgz", "t2.mgz", &["--resmm", "-2.4"])),
        (vec![], nseg("seg.mgz", "t2.mgz", &["--resmm", "-0.3"])),
        (vec![], nseg("seg.mgz", "t2.mgz", &["--resmm", "-0.3", "--ttype"])),
        (vec![], nseg("seg.mgz", "t2.mgz", &["--resmm", "nan"])),
        (vec![], nseg("seg.mgz", "t2.mgz", &["--resmm", "inf"])),
        (vec![], nseg("seg.mgz", "t2.mgz", &["--resmm", "-inf"])),
        (vec![], nseg("seg.mgz", "t2.mgz", &["--upsample", "0"])),
        (vec![], nseg("seg.mgz", "t2.mgz", &["--upsample", "3"])),
        (vec![], nseg("seg.mgz", "t2.mgz", &["--upsample", "-2"])),
        (vec![], nseg("segh.mgz", "orig.mgz", &[])),
        (vec![], nseg("segq.mgz", "t2.mgz", &[])),
        (vec![], nseg("segq.mgz", "t2.mgz", &["--ttype"])),
        (vec![], nseg("segn.mgz", "orig.mgz", &["--hits", "h.mgz", "--pvf", "p.mgz"])),
        (vec![], nseg("segneg.mgz", "t2.mgz", &[])),
        (vec![], nseg("segnan.mgz", "t.mgz", &[])),
        (vec![], nseg("segbig.mgz", "t2.mgz", &["--resmm", "0.5"])),
        (vec![], nseg("segu.mgz", "t1.mgz", &["--ttype", "--resmm", "1"])),
        (vec!["FREESURFER_HOME=@DIR@/fsh2"], nseg("segu.mgz", "t1.mgz", &["--ttype", "--resmm", "1"])),
        (vec!["FREESURFER_HOME=@DIR@/fsh2"], nseg("segu.mgz", "t2.mgz", &["--ttype+head", "--resmm", "0.5"])),
        (vec![], nseg("segx.mgz", "t1.mgz", &["--ttype", "--resmm", "1"])),
        (vec![], nseg("seg.mgz", "t3.nii.gz", &["--ttype", "--resmm", "0.5"])),
        (vec![], nseg("seg.mgz", "t.mgz", &["--ttype+head", "--resmm", "0.25"])),
        (vec![], nseg("segh.mgz", "t3.nii.gz", &["--ttype", "--resmm", "1"])),
        (vec![], nseg("segx.mgz", "t3.nii.gz", &["--ttype", "--resmm", "0.5"])),
        (vec![], nseg("segu.mgz", "t3.nii.gz", &["--ttype", "--resmm", "0.5"])),
        (vec!["FREESURFER_HOME=@DIR@/fsh2"], nseg("segu.mgz", "t3.nii.gz", &["--ttype", "--resmm", "0.5"])),
        (vec![], nseg("segbig.mgz", "t3.nii.gz", &["--ttype", "--resmm", "0.5"])),
        (vec![], nseg("seg.mgz", "t3.nii.gz", &["--resmm", "0.5"])),
        (vec![], nseg("seg0.mgz", "t2.mgz", &[])),
        (vec![], nseg("seg0.mgz", "t2.mgz", &["--ttype"])),
        (vec![], nseg("seg.mgz", "tnan.mgz", &[])),
        (vec![], s(&["--seg", "seg.mgz", "--temp", "t.mgz", "--identity", "--new-aseg2vol", "--o", "o.nii.gz"])),
        (vec![], s(&["--seg", "seg.mgz", "--temp", "t.mgz", "--reg", "reg.dat", "--new-aseg2vol", "--o", "o.mgz"])),
        (vec![], s(&["--seg", "seg.mgz", "--temp", "t.mgz", "--regheader", "--new-aseg2vol", "--o", "nodir/o.mgz"])),
        (vec![], s(&["--seg", "seg.mgz", "--fill-ribbon", "--new-aseg2vol", "--regheader", "--o", "o.mgz", "--subject", "subj", "--hemi", "lh"])),
        (vec![], s(&["--aparc+aseg", "--subject", "subj", "--temp", "t.mgz", "--regheader", "subj/mri/aparc+aseg.mgz", "--new-aseg2vol", "--o", "o.mgz"])),
    ];
    for (env, a) in &cases {
        let a: Vec<&str> = a.iter().map(String::as_str).collect();
        add_env(env, &a);
    }
    let rib = |extra: &[&str]| -> Vec<String> {
        let mut a = s(&["--fill-ribbon", "--subject", "subj", "--hemi", "lh", "--identity"]);
        a.extend(s(extra));
        a.extend(s(&["--o", "o.mgz"]));
        a
    };
    let cases: Vec<(Vec<&str>, Vec<String>)> = vec![
        (vec![], rib(&["--label", "s.label"])),
        (vec![], rib(&["--label", "s.label", "--label", "lh.cortex.label"])),
        (vec![], rib(&["--label", "s.label", "--label-stat", "st.mgz"])),
        (vec![], rib(&["--label", "s.label", "--label-stat", "st.mgz", "--stat-thresh", "0"])),
        (vec![], rib(&["--label", "s.label", "--temp", "t2.mgz"])),
        (vec![], rib(&["--label", "s.label", "--surf", "orig"])),
        (vec![], rib(&["--label", "s.label", "--hits", "h.mgz", "--pvf", "p.mgz"])),
        (vec![], rib(&["--annot", "subj/label/lh.dense.annot"])),
        (vec![], rib(&["--annot", "subj/label/lh.a.annot"])),
        (vec![], rib(&["--annot", "subj/label/lh.zero.annot"])),
        (vec![], rib(&["--label", "scan.label"])),
        (vec![], rib(&["--label", "scan.label", "--tkr-template", "orig.mgz"])),
        (vec![], rib(&["--label", "sbad.label"])),
        (vec![], rib(&["--label", "sneg.label"])),
        (vec![], rib(&["--label", "a.label"])),
        (vec![], rib(&["--label", "nofile.label"])),
        (vec![], rib(&["--label", "s.label", "--lta", "x.lta"])),
        (vec![], rib(&["--label", "s.label", "--lta", "y.lta"])),
        (vec![], rib(&["--label", "s.label", "--lta", "z.lta"])),
        (vec![], rib(&["--label", "s.label", "--lta", "y.lta", "--label-stat", "st.mgz"])),
        (vec![], rib(&["--label", "s.label", "--sd", "."])),
        (vec![], rib(&["--label", "s.label", "--sd", "sd2"])),
        (vec!["-SUBJECTS_DIR"], rib(&["--label", "s.label", "--sd", "."])),
        (vec!["SUBJECTS_DIR=@DIR@/nowhere"], rib(&["--label", "s.label", "--sd", "."])),
        (vec!["FREESURFER_HOME=@DIR@/nowhere"], rib(&["--label", "s.label"])),
        (vec![], rib(&["--label", "s.label", "--o", "nodir/o.mgz"])),
        (vec![], rib(&["--label", "s.label", "--label-stat", "nodir/st.mgz"])),
        (vec![], rib(&["--label", "s.label", "--o", "o.nii.gz"])),
        (vec![], s(&["--fill-ribbon", "--subject", "subj", "--hemi", "rh", "--identity", "--annot", "subj/label/rh.dense.annot", "--o", "o.mgz"])),
        (vec![], s(&["--fill-ribbon", "--subject", "subjfar", "--hemi", "lh", "--identity", "--label", "s.label", "--o", "o.mgz"])),
        (vec![], s(&["--fill-ribbon", "--subject", "nosubj", "--hemi", "lh", "--identity", "--label", "s.label", "--o", "o.mgz"])),
        (vec![], s(&["--fill-ribbon", "--hemi", "lh", "--identity", "--label", "s.label", "--o", "o.mgz"])),
        (vec![], s(&["--fill-ribbon", "--subject", "subj", "--identity", "--label", "s.label", "--o", "o.mgz"])),
        (vec![], s(&["--fill-ribbon", "--subject", "subj", "--hemi", "lh", "--label", "s.label", "--o", "o.mgz"])),
    ];
    for (env, a) in &cases {
        let a: Vec<&str> = a.iter().map(String::as_str).collect();
        add_env(env, &a);
    }
}

/// The options a seeded case draws from.
const LABELS: &[&str] = &[
    "a.label",
    "b.label",
    "c.label",
    "s.label",
    "empty.label",
    "scan.label",
];
const TEMPS: &[&str] = &["t.mgz", "t2.mgz", "t3.nii.gz", "orig.mgz", "t1.mgz"];
const REGS: &[&[&str]] = &[
    &["--identity"],
    &["--reg", "reg.dat"],
    &["--reg", "regr.dat"],
    &["--reg", "regi.dat", "--invertmtx"],
    &["--regheader", "t.mgz"],
    &["--regheader", "orig.mgz"],
];
const EXTRAS: &[&[&str]] = &[
    &["--hits", "h.mgz"],
    &["--fillthresh", "0.1"],
    &["--fillthresh", "0.6"],
    &["--labvoxvol", "0.5"],
    &["--offset", "7"],
    &["--label-stat", "st.mgz"],
    &["--stat-thresh", "0"],
    &["--pvf", "p.mgz"],
    &["--native-vox2ras"],
    &["--tkr-template", "orig.mgz"],
    &["--debug"],
];
const DEFECT_FILES: &[&str] = &["lh.defect_labels", "lh.defect_nan", "lh.defect_neg"];
const DEFECT_TEMPS: &[&str] = &["orig.mgz", "origi.mgz", "origf.mgz", "origb.mgz"];
const SEGS: &[&str] = &["seg.mgz", "segn.mgz", "segh.mgz"];
const SEGS2: &[&str] = &[
    "seg.mgz",
    "seg.mgz",
    "segn.mgz",
    "segh.mgz",
    "segq.mgz",
    "segneg.mgz",
    "segnan.mgz",
    "segbig.mgz",
    "segu.mgz",
];
const RIBBON_LABELS: &[&str] = &[
    "s.label",
    "s.label",
    "lh.cortex.label",
    "scan.label",
    "sbad.label",
];

fn seeded(g: &mut Gen, i: &Inputs) -> String {
    let mut args: Vec<String> = Vec::new();
    let mut env: Vec<&str> = Vec::new();
    match g.below(14) {
        10 | 11 => {
            // part 2: --new-aseg2vol
            args.extend(s(&["--seg", g.pick(SEGS2), "--temp", g.pick(TEMPS)]));
            args.extend(s(g.pick(&[
                &["--regheader"][..],
                &["--identity"],
                &["--reg", "reg.dat"],
            ])));
            args.push("--new-aseg2vol".into());
            if g.chance(2) {
                args.extend(s(&[
                    "--resmm",
                    g.pick(&["0", "0.5", "0.25", "1", "2", "-1", "-2", "-2.7", "-0.3", "3"]),
                ]));
            }
            if g.chance(3) {
                args.extend(s(&["--upsample", g.pick(&["2", "3", "0", "-2"])]));
            }
            if g.chance(3) {
                args.push(g.pick(&["--ttype", "--ttype+head"]).to_owned());
                if g.chance(3) {
                    env.push("FREESURFER_HOME=@DIR@/fsh2");
                }
            }
            if g.chance(4) {
                args.extend(s(&["--hits", "h.mgz"]));
            }
            args.extend(s(&["--o", g.pick(&["o.mgz", "o.nii.gz"])]));
        }
        12 | 13 => {
            // part 2: --fill-ribbon
            if g.chance(3) {
                args.extend(s(&["--annot", g.pick(&["subj/label/lh.dense.annot", "subj/label/lh.a.annot"])]));
            } else {
                for _ in 0..g.int(1, 2) {
                    args.extend(s(&["--label", g.pick(RIBBON_LABELS)]));
                }
                if g.chance(4) {
                    args.extend(s(&["--label-stat", "st.mgz"]));
                }
            }
            args.extend(s(&["--fill-ribbon", "--subject", g.pick(&["subj", "subj", "subj", "subjfar"])]));
            args.extend(s(&["--hemi", "lh"]));
            args.extend(s(g.pick(&[
                &["--identity"][..],
                &["--regheader", "t.mgz"],
                &["--reg", "reg.dat"],
            ])));
            if g.chance(3) {
                args.extend(s(&["--temp", g.pick(TEMPS)]));
            }
            if g.chance(3) {
                args.extend(s(&["--lta", g.pick(&["x.lta", "y.lta", "z.lta"])]));
            }
            if g.chance(4) {
                args.extend(s(&["--stat-thresh", g.pick(&["0", "-2", "4"])]));
            }
            if g.chance(5) {
                args.extend(s(&["--surf", "orig"]));
            }
            if g.chance(6) {
                args.extend(s(&["--sd", g.pick(&[".", "sd2"])]));
            }
            if g.chance(5) {
                args.extend(s(&["--tkr-template", "orig.mgz"]));
            }
            args.extend(s(&["--o", g.pick(&["o.mgz", "o.nii.gz"])]));
        }
        0 | 1 => {
            // the defects
            args.extend(s(&["--defects", "lh.orig.nofix", g.pick(DEFECT_FILES)]));
            args.push(g.pick(DEFECT_TEMPS).to_owned());
            args.push(g.pick(&["0", "1000", "2000", "3", "77"]).to_owned());
            args.push(g.pick(&["0", "1", "0", "2"]).to_owned());
            args.push(g.pick(&["o.mgz", "o.mgh", "o.nii.gz"]).to_owned());
            if g.chance(2) {
                args.push(g.pick(&["lh.cortex.label", "s.label"]).to_owned());
            }
        }
        2 | 3 => {
            // a segmentation
            args.extend(s(&["--seg", g.pick(SEGS), "--temp", g.pick(TEMPS)]));
            args.extend(s(g.pick(&[
                &["--regheader"][..],
                &["--identity"],
                &["--reg", "reg.dat"],
            ])));
            if g.chance(3) {
                args.extend(s(&["--fillthresh", g.pick(&["0.2", "0.5", "0.9"])]));
            }
            if g.chance(3) {
                args.extend(s(&["--upsample", g.pick(&["2", "3"])]));
            }
            if g.chance(4) {
                args.push(g.pick(&["--ttype", "--ttype+head"]).to_owned());
            }
            if g.chance(3) {
                args.extend(s(&["--hits", "h.mgz"]));
            }
            if g.chance(4) {
                args.extend(s(&["--pvf", "p.mgz"]));
            }
            args.extend(s(&["--o", g.pick(&["o.mgz", "o.nii.gz"])]));
        }
        4 | 5 => {
            // surface labels projected, or an annotation
            if g.chance(2) {
                args.extend(s(&["--label", "s.label"]));
            } else {
                args.extend(s(&["--annot", "subj/label/lh.a.annot"]));
            }
            args.extend(s(&["--temp", g.pick(&["orig.mgz", "t2.mgz", "t.mgz"])]));
            args.extend(s(g.pick(REGS)));
            if g.chance(2) {
                let ty = g.pick(&["abs", "frac"]);
                let a = g.int(-4, 0) as f64 * 0.5;
                let b = a + g.int(0, 6) as f64 * 0.5;
                let d = g.pick(&["0.25", "0.5", "1", "0.1"]);
                args.extend(s(&["--proj", ty, &format!("{a}"), &format!("{b}"), d]));
            }
            args.extend(s(&["--subject", "subj", "--hemi", "lh"]));
            if g.chance(4) {
                args.extend(s(&["--surf", "orig"]));
            }
            for _ in 0..g.int(0, 2) {
                args.extend(s(g.pick(EXTRAS)));
            }
            args.extend(s(&["--o", "o.mgz"]));
        }
        _ => {
            // volume labels
            for _ in 0..g.int(1, 3) {
                args.extend(s(&["--label", g.pick(LABELS)]));
            }
            args.extend(s(&["--temp", g.pick(TEMPS)]));
            args.extend(s(g.pick(REGS)));
            for _ in 0..g.int(0, 3) {
                args.extend(s(g.pick(EXTRAS)));
            }
            args.extend(s(&["--o", g.pick(&["o.mgz", "o.mgh", "o.nii.gz"])]));
        }
    }
    if g.chance(6) && args[0] != "--defects" {
        let k = args.len();
        args.rotate_left(k - 2);
    }
    line(&env, i, &args)
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

/// The golden cases (`tests/mri_label2vol_golden.rs`): `defect2seg`'s two calls (the left
/// hemisphere's defects into the template, then merged), defect numbers with NaN and infinities,
/// `seg2cc`'s segmentation onto another grid, labels with hits, statistics and NaN coordinates, a
/// projected surface label and an annotation, on volumes up to 12^3.
pub fn golden_cases() -> Vec<String> {
    let i = base_inputs();
    let mut out = Vec::new();
    for a in [
        &[
            "--defects",
            "lh.orig.nofix",
            "lh.defect_labels",
            "orig.mgz",
            "1000",
            "0",
            "o.mgz",
            "lh.cortex.label",
        ][..],
        &[
            "--defects",
            "lh.orig.nofix",
            "lh.defect_labels",
            "origi.mgz",
            "2000",
            "1",
            "o.mgz",
            "lh.cortex.label",
        ],
        &[
            "--defects",
            "lh.orig.nofix",
            "lh.defect_nan",
            "orig.mgz",
            "1000",
            "0",
            "o.mgz",
        ],
        &[
            "--seg",
            "seg.mgz",
            "--temp",
            "t.mgz",
            "--regheader",
            "--o",
            "o.mgz",
        ],
        &[
            "--seg",
            "segh.mgz",
            "--temp",
            "t2.mgz",
            "--regheader",
            "--o",
            "o.mgz",
            "--hits",
            "h.mgz",
            "--upsample",
            "2",
        ],
        &[
            "--label",
            "a.label",
            "--label",
            "b.label",
            "--temp",
            "t2.mgz",
            "--identity",
            "--hits",
            "h.mgz",
            "--o",
            "o.mgz",
        ],
        &[
            "--label",
            "c.label",
            "--temp",
            "t.mgz",
            "--reg",
            "reg.dat",
            "--label-stat",
            "st.mgz",
            "--o",
            "o.mgz",
        ],
        &[
            "--label",
            "s.label",
            "--temp",
            "orig.mgz",
            "--identity",
            "--proj",
            "frac",
            "0",
            "1",
            "0.25",
            "--subject",
            "subj",
            "--hemi",
            "lh",
            "--o",
            "o.mgz",
        ],
        &[
            "--annot",
            "subj/label/lh.a.annot",
            "--temp",
            "orig.mgz",
            "--identity",
            "--subject",
            "subj",
            "--hemi",
            "lh",
            "--o",
            "o.mgz",
        ],
        &[
            "--seg",
            "segneg.mgz",
            "--temp",
            "t2.mgz",
            "--regheader",
            "--new-aseg2vol",
            "--resmm",
            "0.5",
            "--o",
            "o.mgz",
        ],
        &[
            "--seg",
            "segnan.mgz",
            "--temp",
            "t1.mgz",
            "--regheader",
            "--new-aseg2vol",
            "--ttype",
            "--resmm",
            "2",
            "--o",
            "o.mgz",
        ],
        &[
            "--label",
            "s.label",
            "--fill-ribbon",
            "--subject",
            "subj",
            "--hemi",
            "lh",
            "--identity",
            "--lta",
            "y.lta",
            "--label-stat",
            "st.mgz",
            "--o",
            "o.mgz",
        ],
        &[
            "--annot",
            "subj/label/lh.dense.annot",
            "--fill-ribbon",
            "--subject",
            "subj",
            "--hemi",
            "lh",
            "--identity",
            "--o",
            "o.mgz",
        ],
    ] {
        out.push(line(&[], &i, &s(a)));
    }
    out
}

// ------------------------------------------------------------------ the port's answer

/// Text files listed with their bytes.
fn is_text(name: &str) -> bool {
    [".txt", ".dat", ".label"].iter().any(|e| name.ends_with(e))
}

/// A written volume's bytes as the probe compares them: a gzip file decompressed (`gzread`), then
/// every command line it records (from the 8-byte length before `ProgramName: ` to the NUL after
/// it) as `<CMD>`; a NIfTI-1 file's voxel offset and extension size masked too (they hold the
/// lines' lengths: the user's and the machine's names).
pub fn canon(path: &Path, b: &[u8]) -> Vec<u8> {
    let raw = if b.starts_with(&[0x1f, 0x8b]) {
        let name = path.to_string_lossy().into_owned();
        let mut r = Vec::new();
        if let Some(mut f) = larmorx_io::znz::GzFile::open(name.as_bytes(), b"rb") {
            let mut buf = vec![0u8; 65536];
            loop {
                let n = f.gzread(&mut buf);
                if n <= 0 {
                    break;
                }
                r.extend_from_slice(&buf[..n as usize]);
            }
        }
        r
    } else {
        b.to_vec()
    };
    // a NIfTI-1 file: the voxels' offset and the extension's size (which the masked lines' length
    // decides) masked, the extension's padding zeros left out
    if raw.len() >= 352 && raw[..4] == 348i32.to_le_bytes() && &raw[344..348] == b"n+1\0" {
        let vox = f32::from_le_bytes([raw[108], raw[109], raw[110], raw[111]]) as usize;
        let vox = vox.clamp(352, raw.len());
        let mut out = raw[..108].to_vec();
        out.extend_from_slice(b"<VOX>");
        out.extend_from_slice(&raw[112..352]);
        if vox >= 360 {
            out.extend_from_slice(b"<ESIZE>");
            let mut ext = mask_command_lines(&raw[356..vox]);
            while ext.last() == Some(&0) {
                ext.pop();
            }
            out.extend_from_slice(&ext);
        }
        out.extend_from_slice(&raw[vox..]);
        return out;
    }
    mask_command_lines(&raw)
}

/// Each command line in `raw` (from the 8-byte length before `ProgramName: ` to the NUL after it)
/// as `<CMD>`.
fn mask_command_lines(raw: &[u8]) -> Vec<u8> {
    let key = b"ProgramName: ";
    let mut out = Vec::with_capacity(raw.len());
    let mut i = 0;
    while i < raw.len() {
        if raw[i..].starts_with(key) && out.len() >= 8 {
            out.truncate(out.len() - 8);
            out.extend_from_slice(b"<CMD>");
            while i < raw.len() && raw[i] != 0 {
                i += 1;
            }
            continue;
        }
        out.push(raw[i]);
        i += 1;
    }
    out
}

/// Every file of a case's folder but its inputs left as they were, sorted, each as `F <name>
/// <fnv> <size>` of [`canon`] (text files with their bytes escaped), the folder's name in them as
/// `<DIR>`; each folder the run made as `D <name>`.
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
                if is_text(&n) {
                    let b = mask_bytes(&b, d.as_bytes());
                    out.push(format!("F {n} {}", escape(&b)));
                } else {
                    let b = mask_bytes(&canon(&dir.join(&n), &b), d.as_bytes());
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

/// The defects whose official result depends on memory contents (`reproduce` fills it
/// deterministically): the pseudo-notice `MEMORY`.
const MEMORY: [&str; 9] = [
    "FS-53", "FS-249", "FS-441", "FS-1252", "FS-1256", "FS-1258", "FS-1259", "FS-1263", "FS-1265",
];

/// `MRIseg2SegPVF(): done t = <seconds>,`: the time masked.
pub fn mask_time(text: &str) -> String {
    let key = "MRIseg2SegPVF(): done t = ";
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(k) = rest.find(key) {
        out.push_str(&rest[..k + key.len()]);
        rest = &rest[k + key.len()..];
        let end = rest.find(',').unwrap_or(rest.len());
        out.push_str("<T>");
        rest = &rest[end..];
    }
    out.push_str(rest);
    out
}

/// The port's line for a case line and the defects it noted (`dir`: an empty folder), with the
/// pseudo-notices `CRASH`, `MEMORY` and `HANG`.
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
            threads: 1,
        };
        match main(&mut g, &mut r, &c.args) {
            Ok(()) => 0,
            Err(f) => f.status(),
        }
    };
    let _ = out.flush();
    let mask = |b: &[u8]| mask_time(&escape(b).replace(&escape(d.as_bytes()), "<DIR>"));
    let files = written(dir, &c.files, &c.dirs);
    let text = format!("EXIT {status} | {} | {} | {files}", mask(&out), mask(&err));
    let mut ids: Vec<&'static str> = g.notices.as_slice().iter().map(|n| n.defect.id).collect();
    if matches!(status, 134 | 136 | 139) {
        ids.push("CRASH");
    }
    if ids.iter().any(|x| MEMORY.contains(x)) {
        ids.push("MEMORY");
    }
    if ids.contains(&"FS-1257") {
        ids.push("HANG");
    }
    (text, ids)
}

/// [`run_line_in`] in a fresh temporary folder.
pub fn run_line(line: &str, mode: UpstreamDefects) -> (String, Vec<&'static str>) {
    let dir = tempfile::tempdir().expect("a temporary folder");
    run_line_in(line, mode, dir.path())
}
