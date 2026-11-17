// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Piece 32 (track A relay 39): GCA morphs written, and `TRANSFORM`s read and sampled.
//!
//! `m3w` is piece 17's `vwr` (piece 14's writer fields) for three-frame volumes (now and then one,
//! two or four frames: `MRIwriteType`'s refusal) of a few voxels a side, of float displacements
//! (NaN and infinite now and then) or bytes, shorts and ints, with FreeSurfer's coronal axes or
//! permuted ones: `MRIwrite` of `.m3z`, `.m3d`, `@m3z` names (upper case, a name holding `.m3z`
//! before `.m3d`, a folder that does not exist, a folder of that name), and `MRIwriteType(...,
//! MGH_MORPH)` of `.mgz`, `.mgh`, `.nii`, `.nii.gz` names (the warp volume; NIfTI's change to RAS
//! displacements), `.m3z`, and names of other formats or none (FS-856); the file's bytes, and half
//! the time the file read back with `mri_read` (the morph reader of relay 38).
//!
//! `gcw` reads a morph (piece 31's files, now and then a warp volume) and writes it again
//! (`GCAMwrite`, `mri_convert`'s path for a morph input) under a morph or warp name in each warp
//! format; the status and the files.
//!
//! `trs` reads a transform (`TransformRead`): `.lta` files (types 0, 1, 14 and 11, FS-858; one,
//! two or no transforms, FS-859; geometries valid or not), MNI `.xfm` files, `register.dat` files,
//! `identity.nofile`, morphs (`.m3d`, `.m3z`: piece 31's files with a small image), missing files;
//! inverts it (`TransformInvert`: not, without a volume, or with a header volume of its source
//! geometry) and asks it: `TransformSample`, `TransformSampleReal`, `TransformSampleReal2`,
//! `TransformSampleInverse`, `TransformSampleInverseFloat`, `TransformSampleInverseVoxel`,
//! `TransformSampleDirection`, `TransformSourceVoxelToAtlas`, the source and destination
//! geometries, `TransformSwapInverse`, `TransformSetMRIVolGeomToSrc`/`...ToDst`,
//! `TransformCopyVolGeomToMRI`; then `TransformFree`.

use std::io::Write;

use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_gca::gcam::gcam_read;
use larmorx_freesurfer_gca::gcam_write::gcam_write;
use larmorx_freesurfer_gca::transform::{self as tr, Transform};
use larmorx_freesurfer_mri::mri::{self, Mri};
use larmorx_freesurfer_mri::vol_geom::VolGeom;

use super::piece4::{file_entry, setup};
use super::piece6::oname;
use super::piece10::Toks;
use super::piece12::{fhex, odir};
use super::{Gen, Out, Work, enc, hex};

pub const KINDS: [&str; 3] = ["m3w", "trs", "gcw"];

/// What the port's side writes for a resampling past fsio's time or message limits (not run; the
/// case is marked `SKIP`).
const NOT_RUN: &[u8] = b"fs_io_probe: a morph resampling past fsio's limits, not run\n";

// ---------------------------------------------------------------- generation

/// A displacement: small steps, now and then NaN, infinite, subnormal or large.
fn disp(g: &mut Gen) -> f32 {
    match g.below(40) {
        0 => f32::from_bits(0x7fc0_0000),
        1 => f32::from_bits(0xffa0_0001),
        2 => f32::INFINITY,
        3 => f32::from_bits(1),
        4 => 1e30,
        5 => -0.0,
        _ => g.int(-16, 16) as f32 / 8.0,
    }
}

/// A three-frame volume's tokens as piece 14's `read_mri14` reads them (no scan parameters, no
/// command lines, matrices, morph fields, colour table or b-values).
fn volume32(g: &mut Gen, t: &mut Vec<String>) -> (i64, bool) {
    let w = g.int(1, 4);
    let h = if g.chance(5) { 1 } else { g.int(1, 4) };
    let d = if g.chance(5) { 1 } else { g.int(1, 4) };
    let nf = if g.chance(8) {
        g.pick(&[1i64, 2, 4])
    } else {
        3
    };
    let ty = g.pick(&[
        mri::MRI_FLOAT,
        mri::MRI_FLOAT,
        mri::MRI_FLOAT,
        mri::MRI_FLOAT,
        mri::MRI_UCHAR,
        mri::MRI_SHORT,
        mri::MRI_INT,
    ]);
    let alloc = !g.chance(40);
    for v in [w, h, d, i64::from(ty), nf, i64::from(alloc)] {
        t.push(v.to_string());
    }
    let mut geom = [
        1.0f32, 1.0, 1.0, -1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, -1.0, 0.0,
    ];
    if g.chance(3) {
        let perm = g.pick(&[[0, 1, 2], [0, 2, 1], [1, 0, 2], [2, 1, 0]]);
        geom[3..].fill(0.0);
        for (axis, p) in perm.iter().enumerate() {
            geom[3 + 3 * axis + p] = if g.chance(2) { 1.0 } else { -1.0 };
        }
    }
    for s in geom.iter_mut().take(3) {
        *s = g.pick(&[1.0, 1.0, 2.0, 0.5, 1.5]);
    }
    if g.chance(8) {
        geom[3 + g.below(9) as usize] += 0.25;
    }
    for v in geom {
        t.push(fhex(v));
    }
    for _ in 0..3 {
        t.push(fhex(g.int(-8, 8) as f32 / 2.0));
    }
    t.push("1".into()); // ras_good_flag
    for _ in 0..4 {
        t.push(fhex(0.0));
    }
    t.push(format!("{:016x}", 0f64.to_bits()));
    t.push(fhex(0.0));
    t.push("1".into()); // dof
    let n = (w * h * d * nf) as usize;
    let mut b = Vec::new();
    for _ in 0..n {
        match ty {
            mri::MRI_FLOAT => b.extend_from_slice(&disp(g).to_le_bytes()),
            mri::MRI_UCHAR => b.push(g.pick(&[0u8, 1, 2, 255, 7])),
            mri::MRI_SHORT => b.extend_from_slice(&(g.int(-3, 3) as i16).to_le_bytes()),
            _ => b.extend_from_slice(&(g.int(-3, 3) as i32).to_le_bytes()),
        }
    }
    t.push(if g.chance(12) { "-".into() } else { hex(&b) });
    t.push("N".into()); // transform name
    t.push("N".into()); // phase-encoding direction
    t.push("0".into()); // command lines
    t.push("0".into()); // the first frame's label
    t.push(fhex(0.0)); // its threshold
    for _ in 0..2 {
        t.push("-1".into());
        t.push("-1".into());
    }
    t.push("-1".into()); // warpFieldFormat
    t.push("0".into()); // colour table
    t.push("-1".into()); // b-values
    (nf, alloc)
}

fn write_case(g: &mut Gen) -> Vec<String> {
    let mut t = vec![
        "m3w".to_owned(),
        "N".into(),
        "N".into(),
        "0".into(),
        "N".into(),
    ];
    t.push(g.pick(&["0", "0", "0", "40"]).into());
    // MRIwrite (1) of morph names, MRIwriteType as MGH_MORPH (39) of any
    let how = if g.chance(2) { 1 } else { 39 };
    let name = if how == 1 {
        g.pick(&[
            "w/o.m3z",
            "w/o.m3z",
            "w/o.m3d",
            "w/o.m3d",
            "w/a.b.m3z",
            "w/o.m3z.m3d",
            "w/O.M3Z",
            "w/o.mgz@m3z",
            "w/no/o.m3z",
            "w/d.m3z",
        ])
    } else {
        g.pick(&[
            "w/o.mgz",
            "w/o.mgz",
            "w/o.mgh",
            "w/o.nii",
            "w/o.nii",
            "w/o.nii.gz",
            "w/o.m3z",
            "w/o.m3d",
            "w/o.img",
            "w/o",
            "w/no/o.mgz",
            "w/no/o.m3d",
        ])
    };
    t.push(if name == "w/d.m3z" {
        format!("D{}", hex(b"w/d.m3z"))
    } else {
        "-".into()
    });
    t.push("0".into());
    t.push(how.to_string());
    let (nf, alloc) = volume32(g, &mut t);
    t.push("-1".into());
    t.push("-1".into());
    t.push(enc(Some(name.as_bytes())));
    let back = alloc && nf == 3 && !name.contains("no/") && name != "w/d.m3z" && g.chance(2);
    t.push(i64::from(back).to_string());
    t
}

/// A row of a 4 x 4 matrix (`r` its row): the identity's, scaled, shifted or random; the last row
/// `0 0 0 1`.
fn mat_row(g: &mut Gen, r: usize, kind: u64) -> String {
    let mut v = [0f64; 4];
    if r == 3 {
        v[3] = 1.0;
    } else {
        v[r] = match kind {
            0 => 1.0,
            1 => g.pick(&[2.0, 0.5, -1.0]),
            2 => 0.0,
            _ => g.int(-12, 12) as f64 / 8.0,
        };
        if kind >= 3 {
            for (c, x) in v.iter_mut().enumerate() {
                if c != r {
                    *x = g.int(-8, 8) as f64 / 16.0;
                }
            }
        }
        if kind != 2 && g.chance(2) {
            v[3] = g.int(-8, 8) as f64 / 2.0;
        }
    }
    let s: Vec<String> = v.iter().map(|x| format!("{x:.6}")).collect();
    format!("{}\n", s.join(" "))
}

/// A volume geometry block of an `.lta` (`src` or `dst`): small extents.
fn lta_vg(g: &mut Gen, which: &str) -> String {
    let mut s = format!("{which} volume info\n");
    s += &format!("valid = {}  # volume info valid\n", g.pick(&[1, 1, 1, 0]));
    s += "filename = /a/b.mgz\n";
    s += &format!("volume = {} {} {}\n", g.int(1, 5), g.int(1, 5), g.int(1, 5));
    s += &format!(
        "voxelsize = {:.15e} {:.15e} {:.15e}\n",
        g.pick(&[1.0, 2.0, 0.5]),
        1.0,
        g.pick(&[1.0, 1.5])
    );
    let axes = g.pick(&[
        ["-1 0 0", "0 0 -1", "0 1 0"],
        ["1 0 0", "0 1 0", "0 0 1"],
        ["0 1 0", "-1 0 0", "0 0 1"],
    ]);
    s += &format!("xras   = {}\n", axes[0]);
    s += &format!("yras   = {}\n", axes[1]);
    s += &format!("zras   = {}\n", axes[2]);
    s += &format!(
        "cras   = {} {} {}\n",
        g.int(-4, 4),
        g.int(-4, 4),
        g.int(-4, 4)
    );
    s
}

/// An `.lta` text.
fn lta32(g: &mut Gen) -> Vec<u8> {
    let nx = g.pick(&[1, 1, 1, 1, 2, 0]);
    let ty = g.pick(&[0, 0, 1, 1, 1, 14, 11]);
    let mut s = format!("type      = {ty}\nnxforms   = {nx}\n");
    for _ in 0..nx {
        s += "mean      = 0.0000 0.0000 0.0000\nsigma     = 1.0000\n1 4 4\n";
        let kind = g.below(6);
        for r in 0..4 {
            s += &mat_row(g, r, kind);
        }
    }
    for _ in 0..nx {
        if g.chance(8) {
            continue;
        }
        s += &lta_vg(g, "src");
        s += &lta_vg(g, "dst");
    }
    if g.chance(3) {
        s += "subject bert\n";
    }
    s.into_bytes()
}

/// A transform file and its name.
fn transform_file(g: &mut Gen, entries: &mut Vec<String>) -> (String, bool) {
    match g.below(20) {
        0..=7 => {
            let (b, _) = super::piece31::m3z_bytes(g, true);
            if g.chance(3) {
                let z = super::piece31::gz_stored(&b, None, false);
                entries.push(file_entry("w/t.m3z", &z));
                ("w/t.m3z".into(), true)
            } else {
                entries.push(file_entry("w/t.m3d", &b));
                ("w/t.m3d".into(), true)
            }
        }
        8..=15 => {
            entries.push(file_entry("w/t.lta", &lta32(g)));
            ("w/t.lta".into(), false)
        }
        16 => {
            let mut s = String::from(
                "MNI Transform File\n%Generated by test\n\nTransform_Type = Linear;\nLinear_Transform =\n",
            );
            let kind = g.below(6);
            for r in 0..3 {
                let row = mat_row(g, r, kind);
                s += row.trim_end();
                s += if r == 2 { ";\n" } else { "\n" };
            }
            entries.push(file_entry("w/t.xfm", s.as_bytes()));
            ("w/t.xfm".into(), false)
        }
        17 => {
            let mut s = String::from("bert\n1.000000\n1.500000\n0.150000\n");
            let kind = g.below(6);
            for r in 0..4 {
                s += &mat_row(g, r, kind);
            }
            s += "round\n";
            entries.push(file_entry("w/register.dat", s.as_bytes()));
            ("w/register.dat".into(), false)
        }
        18 => (
            g.pick(&["w/identity.nofile", "identity.nofile"]).into(),
            false,
        ),
        _ => (
            g.pick(&["w/none.lta", "w/none.m3z", "w/none.m3d"]).into(),
            false,
        ),
    }
}

/// A sample point's coordinate.
fn coord(g: &mut Gen) -> f32 {
    match g.below(30) {
        0 => f32::from_bits(0x7fc0_0000),
        1 => f32::INFINITY,
        2 => -0.0,
        3 => 1e10,
        _ => g.int(-4, 24) as f32 / 4.0,
    }
}

fn read_case(g: &mut Gen) -> Vec<String> {
    let mut t = vec!["trs".to_owned()];
    t.push(g.pick(&["0", "0", "0", "40"]).into());
    let mut entries = Vec::new();
    let (name, morph) = transform_file(g, &mut entries);
    let inv = if morph {
        g.pick(&[2, 2, 2, 1, 0])
    } else {
        g.pick(&[0, 1, 2])
    };
    let mut bits = 0;
    if g.chance(6) {
        bits |= 512;
    }
    // FS_SKIP_TAGS leaves a morph's image at 256 x 256 x 256 voxels, whose inverse takes
    // FreeSurfer minutes: only without the inverse
    if g.chance(10) && !(morph && inv == 2) {
        bits |= 1;
    }
    t.push(bits.to_string());
    t.push(enc(Some(name.as_bytes())));
    t.push(if entries.is_empty() {
        "-".into()
    } else {
        entries.join(",")
    });
    t.push(inv.to_string());
    let nq = g.int(1, 6);
    let nq_at = t.len();
    t.push(nq.to_string());
    for _ in 0..nq {
        let k = g.pick(&[
            "s", "s", "r", "R", "i", "i", "f", "v", "d", "a", "g", "w", "m", "W", "C", "A", "A",
            "D",
        ]);
        t.push(k.into());
        query_args(g, k, &mut t);
    }
    // relay 41's queries (I, V, X, O): drawn from a generator seeded by the case's text so far,
    // so the earlier cases of every seed keep their stream (the recorded test's set with them)
    let seed = t.join(" ").bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    });
    let mut h = Gen::new(seed);
    let extra = h.below(3) as i64;
    for _ in 0..extra {
        let k = h.pick(&["I", "I", "V", "X", "O", "O"]);
        t.push(k.into());
        query_args(&mut h, k, &mut t);
    }
    t[nq_at] = (nq + extra).to_string();
    t
}

/// A query's arguments (after its letter).
fn query_args(g: &mut Gen, k: &str, t: &mut Vec<String>) {
    match k {
        "s" | "r" | "R" | "f" => {
            for _ in 0..3 {
                t.push(fhex(coord(g)));
            }
        }
        "i" | "a" => {
            for _ in 0..3 {
                t.push(g.pick(&[-1i64, 0, 1, 2, 3, 4, 7, 9]).to_string());
            }
        }
        "v" => {
            for _ in 0..3 {
                t.push(g.int(1, 5).to_string());
            }
            for _ in 0..3 {
                t.push(g.pick(&[-1i64, 0, 1, 2, 3, 4, 7]).to_string());
            }
        }
        "d" => {
            for _ in 0..6 {
                t.push(fhex(coord(g)));
            }
        }
        // relay 40: TransformWrite's name; TransformApplyType's and
        // TransformCreateDensityMap's volume (width, height, depth) and interpolation
        "W" => {
            let n = g.pick(&[
                "w/tw.lta",
                "w/tw.lta",
                "w/tw.xfm",
                "w/tw.dat",
                "w/tw.m3z",
                "w/tw.mat",
                "w/no/tw.lta",
                "w/tw",
            ]);
            t.push(enc(Some(n.as_bytes())));
        }
        "A" | "D" => {
            for _ in 0..3 {
                t.push(g.int(1, 4).to_string());
            }
            t.push(g.pick(&[0i64, 1, 1, 2]).to_string());
        }
        // relay 41: TransformApplyInverseType's volume and interpolation (-1:
        // TransformApplyInverse); TransformVox2Ras's and TransformRas2Vox's volumes;
        // TransformCompose's matrices and destination
        "I" => {
            for _ in 0..3 {
                t.push(g.int(1, 4).to_string());
            }
            t.push(g.pick(&[-1i64, 0, 1, 1, 2]).to_string());
        }
        "V" | "X" => t.push(g.pick(&[0i64, 1]).to_string()),
        "O" => {
            for _ in 0..3 {
                t.push(g.pick(&[0i64, 1]).to_string());
            }
        }
        _ => {}
    }
}

/// `gcw`: a morph file of piece 31 (`.m3d`, `.m3z`, now and then an MGH warp volume) read and
/// written again under a morph or warp name with one of the four warp formats.
fn rewrite_case(g: &mut Gen) -> Vec<String> {
    let mut t = vec!["gcw".to_owned()];
    t.push(g.pick(&["0", "0", "0", "40"]).into());
    let mut entries = Vec::new();
    let name = if g.chance(8) {
        let b = super::piece31::warp_mgz(g);
        entries.push(file_entry("w/a.mgz", &b));
        "w/a.mgz"
    } else if g.chance(3) {
        let (b, _) = super::piece31::m3z_bytes(g, false);
        entries.push(file_entry(
            "w/a.m3z",
            &super::piece31::gz_stored(&b, None, false),
        ));
        "w/a.m3z"
    } else {
        let (b, _) = super::piece31::m3z_bytes(g, false);
        entries.push(file_entry("w/a.m3d", &b));
        "w/a.m3d"
    };
    let bits = if g.chance(10) { 1 } else { 0 } | if g.chance(8) { 512 } else { 0 };
    t.push(bits.to_string());
    t.push(enc(Some(name.as_bytes())));
    t.push(entries.join(","));
    let out = g.pick(&[
        "w/o.m3z",
        "w/o.m3z",
        "w/o.m3d",
        "w/o.mgz",
        "w/o.mgz",
        "w/o.nii",
        "w/o.nii.gz",
        "w/o.img",
        "w/no/o.m3d",
    ]);
    t.push(enc(Some(out.as_bytes())));
    t.push(g.pick(&[0i64, 0, 0, 1, 2, 3]).to_string());
    t
}

pub fn generate(g: &mut Gen, kind: &str, _portable: bool) -> Vec<String> {
    match kind {
        "m3w" => write_case(g),
        "gcw" => rewrite_case(g),
        _ => read_case(g),
    }
}

// ---------------------------------------------------------------- the port's answers

fn fh(o: &mut Out, v: f32) {
    o.x(&v.to_bits().to_be_bytes(), 4);
}

fn dh(o: &mut Out, v: f64) {
    o.x(&v.to_bits().to_be_bytes(), 8);
}

fn pvg(o: &mut Out, w: &Work, v: &VolGeom) {
    for x in [v.valid, v.width, v.height, v.depth] {
        o.i(i64::from(x));
    }
    for x in [
        v.xsize, v.ysize, v.zsize, v.x_r, v.x_a, v.x_s, v.y_r, v.y_a, v.y_s, v.z_r, v.z_a, v.z_s,
        v.c_r, v.c_a, v.c_s, v.s_r, v.s_a, v.s_s,
    ] {
        fh(o, x);
    }
    oname(o, w, Some(v.fname.c_bytes()));
}

fn pmri(o: &mut Out, m: &Mri) {
    let v = &m.vg;
    for x in [v.width, v.height, v.depth] {
        o.i(i64::from(x));
    }
    for x in [
        v.xsize, v.ysize, v.zsize, v.x_r, v.x_a, v.x_s, v.y_r, v.y_a, v.y_s, v.z_r, v.z_a, v.z_s,
        v.c_r, v.c_a, v.c_s,
    ] {
        fh(o, x);
    }
    o.i(i64::from(v.ras_good_flag));
}

fn f32_tok(t: &mut Toks) -> f32 {
    f32::from_bits(u32::from_str_radix(t.next(), 16).unwrap())
}

#[allow(clippy::too_many_arguments)]
fn trs(
    tk: &[&str],
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    o: &mut Out,
    w: &Work,
) -> Result<(), Fatal> {
    let dir = Some(w.fix.as_path());
    g.progname = "fsio".into();
    let mut t = Toks { t: tk, i: 1 };
    g.gdiag = u64::from_str_radix(t.next(), 16).unwrap();
    let bits = t.int();
    let mut pairs: Vec<(&str, &str)> = Vec::new();
    if bits & 1 != 0 {
        pairs.push(("FS_SKIP_TAGS", "1"));
    }
    if bits & 512 != 0 {
        pairs.push(("SHOW_NEG", "1"));
    }
    let env = Environment::from_pairs(pairs);
    let name = t.string();
    setup(w, t.next());
    let inv = t.int();
    let nq = t.int();
    g.errno = 0;
    let Some(mut tr) = tr::transform_read(g, out, err, &env, dir, &name)? else {
        o.0.push_str("N ");
        o.i(i64::from(g.errno));
        o.i(i64::from(g.gerror));
        return Ok(());
    };
    o.i(i64::from(tr.ttype));
    if inv == 1 {
        o.i(i64::from(tr::transform_invert(
            g, out, err, &env, dir, &mut tr, None,
        )?));
    }
    if inv == 2 {
        let vg = tr::transform_get_src_vol_geom(g, err, &tr)?;
        let mut m = mri::mri_alloc_header(
            g,
            out,
            err,
            vg.width,
            vg.height,
            vg.depth,
            mri::MRI_UCHAR,
            1,
        )?;
        mri::mri_copy_vol_geom_to_mri(&mut m, &vg);
        o.i(i64::from(tr::transform_invert(
            g,
            out,
            err,
            &env,
            dir,
            &mut tr,
            Some(&m),
        )?));
    }
    for _ in 0..nq {
        let k = t.next();
        o.0.push_str(k);
        o.0.push(' ');
        query(g, out, err, &env, o, w, &mut tr, k, &mut t)?;
    }
    let mut tr = Some(tr);
    o.i(i64::from(tr::transform_free(g, &mut tr)));
    o.i(i64::from(g.errno));
    Ok(())
}

fn gcw(
    tk: &[&str],
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    o: &mut Out,
    w: &Work,
) -> Result<(), Fatal> {
    let dir = Some(w.fix.as_path());
    g.progname = "fsio".into();
    let mut t = Toks { t: tk, i: 1 };
    g.gdiag = u64::from_str_radix(t.next(), 16).unwrap();
    let bits = t.int();
    let mut pairs: Vec<(&str, &str)> = Vec::new();
    if bits & 1 != 0 {
        pairs.push(("FS_SKIP_TAGS", "1"));
    }
    if bits & 512 != 0 {
        pairs.push(("SHOW_NEG", "1"));
    }
    let env = Environment::from_pairs(pairs);
    let name = t.string();
    setup(w, t.next());
    let oname_ = t.string();
    let datainterp = t.int() as i32;
    g.errno = 0;
    let Some(mut gcam) = gcam_read(g, out, err, &env, dir, &name)? else {
        o.0.push_str("N ");
        o.i(i64::from(g.errno));
        return Ok(());
    };
    let r = gcam_write(g, out, err, &env, dir, &mut gcam, &oname_, datainterp)?;
    o.i(i64::from(r));
    odir(o, w);
    o.i(i64::from(g.errno));
    Ok(())
}

/// `fsio`'s `ptrans`: the transform's type; a morph's type and the FNV-1a 64 of its nodes' current
/// and original positions; an array's type, count, first matrix and its two geometries.
fn ptrans(o: &mut Out, tr: &Transform) {
    o.i(i64::from(tr.ttype));
    if tr.ttype == larmorx_freesurfer_mri::lta::MORPH_3D_TYPE {
        let gm = tr.gcam();
        o.i(i64::from(gm.mtype));
        let mut b = Vec::with_capacity(gm.nodes.len() * 48);
        for n in &gm.nodes {
            for v in [n.x, n.y, n.z, n.origx, n.origy, n.origz] {
                b.extend_from_slice(&v.to_le_bytes());
            }
        }
        super::piece11::fnv(o, &b);
    } else {
        let l = tr.lta();
        o.i(i64::from(l.ltype));
        o.i(i64::from(l.num_xforms));
        if l.num_xforms > 0 {
            let x = &l.xforms[0];
            match &x.m_l {
                None => o.0.push_str("N "),
                Some(m) => {
                    o.i(i64::from(m.rows));
                    o.i(i64::from(m.cols));
                    for r in 1..=m.rows {
                        for c in 1..=m.cols {
                            fh(o, m.relt(r, c));
                        }
                    }
                }
            }
            for v in [&x.src, &x.dst] {
                o.i(i64::from(v.valid));
                o.i(i64::from(v.width));
                o.i(i64::from(v.height));
                o.i(i64::from(v.depth));
                for f in [
                    v.xsize, v.ysize, v.zsize, v.x_r, v.x_a, v.x_s, v.y_r, v.y_a, v.y_s, v.z_r,
                    v.z_a, v.z_s, v.c_r, v.c_a, v.c_s,
                ] {
                    fh(o, f);
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn query(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    env: &Environment,
    o: &mut Out,
    w: &Work,
    tr: &mut Transform,
    k: &str,
    t: &mut Toks,
) -> Result<(), Fatal> {
    match k {
        "s" | "r" | "R" | "f" => {
            let (x, y, z) = (f32_tok(t), f32_tok(t), f32_tok(t));
            let (r, p) = match k {
                "s" => tr::transform_sample(g, out, err, env, tr, x, y, z)?,
                "r" => tr::transform_sample_real(g, out, err, env, tr, x, y, z)?,
                "R" => tr::transform_sample_real2(g, out, err, env, tr, x, y, z)?,
                _ => tr::transform_sample_inverse_float(g, out, err, env, tr, x, y, z)?,
            };
            o.i(i64::from(r));
            // a morph's inverse sample outside the grid leaves FreeSurfer's -7s
            let p = if k == "f" && r != 0 && tr.ttype == larmorx_freesurfer_mri::lta::MORPH_3D_TYPE
            {
                [-7.0; 3]
            } else {
                p
            };
            for v in p {
                fh(o, v);
            }
        }
        "i" => {
            let (x, y, z) = (t.int() as i32, t.int() as i32, t.int() as i32);
            let (r, p) = tr::transform_sample_inverse(g, out, err, env, tr, x, y, z)?;
            o.i(i64::from(r));
            for v in p {
                fh(o, v);
            }
        }
        "v" => {
            let (wd, ht, dp) = (t.int() as i32, t.int() as i32, t.int() as i32);
            let (x, y, z) = (t.int() as i32, t.int() as i32, t.int() as i32);
            let (r, p) =
                tr::transform_sample_inverse_voxel(g, out, err, env, tr, wd, ht, dp, x, y, z)?;
            o.i(i64::from(r));
            for v in p {
                o.i(i64::from(v));
            }
        }
        "d" => {
            let p0 = [f32_tok(t), f32_tok(t), f32_tok(t)];
            let n = [f32_tok(t), f32_tok(t), f32_tok(t)];
            let p = tr::transform_sample_direction(g, out, err, env, tr, p0, n)?;
            o.i(0);
            for v in p {
                fh(o, v);
            }
        }
        "a" => {
            let (x, y, z) = (t.int() as i32, t.int() as i32, t.int() as i32);
            let (r, p) = tr::transform_source_voxel_to_atlas(g, out, err, env, tr, x, y, z)?;
            o.i(i64::from(r));
            for v in p {
                dh(o, v);
            }
        }
        "g" => {
            let a = tr::transform_get_src_vol_geom(g, err, tr)?;
            o.i(0);
            let b = tr::transform_get_dst_vol_geom(g, err, tr)?;
            o.i(0);
            pvg(o, w, &a);
            pvg(o, w, &b);
        }
        "w" => {
            o.i(i64::from(tr::transform_swap_inverse(g, err, tr)));
        }
        "W" => {
            let name = t.string();
            let mut env = env.clone();
            env.set("USER", "u");
            env.set(
                "FREESURFER_REPLACEMENT_FOR_CREATION_TIME_STRING",
                "Sun Nov 16 12:00:00 2026",
            );
            g.errno = 0;
            let r = tr::transform_write(g, out, err, &env, Some(w.fix.as_path()), tr, &name)?;
            o.i(i64::from(r));
            o.i(i64::from(g.errno));
            odir(o, w);
        }
        "C" => {
            let Some(c) = tr::transform_copy(g, out, err, tr, None)? else {
                return Err(Fatal::Exit(139));
            };
            o.i(i64::from(c.ttype));
            o.i(i64::from(c.mri_xn.is_some()));
            if c.ttype == larmorx_freesurfer_mri::lta::MORPH_3D_TYPE {
                let gm = c.gcam();
                o.i(i64::from(gm.mri_xind.is_some()));
                o.i(i64::from(gm.m_affine.is_some()));
            }
            *tr = c;
        }
        "A" | "D" | "I" => {
            let (wd, ht, dp, interp) = (
                t.int() as i32,
                t.int() as i32,
                t.int() as i32,
                t.int() as i32,
            );
            // relay 43: a morph's resampling past fsio's limits is not run (marked SKIP): the
            // inverse of a large image (FS_SKIP_TAGS leaves 256 x 256 x 256 voxels: GCAMinvert takes
            // FreeSurfer minutes, past fsio's 5 seconds) or a sinc sample of a large grid (an
            // error line per voxel, past fsio's 64 MiB of messages)
            if tr.ttype == larmorx_freesurfer_mri::lta::MORPH_3D_TYPE {
                let gm = tr.gcam();
                let voxels = |v: &VolGeom| {
                    i64::from(v.width.max(0))
                        * i64::from(v.height.max(0))
                        * i64::from(v.depth.max(0))
                };
                let grid = if k == "I" { &gm.image } else { &gm.atlas };
                let big = voxels(grid) > 1 << 20;
                let sinc = interp == larmorx_freesurfer_mri::mri::SAMPLE_SINC && k != "D";
                if big && (sinc || (k == "I" && gm.mri_xind.is_none())) {
                    let _ = err.write_all(NOT_RUN);
                    o.0.push_str("N ");
                    return Ok(());
                }
            }
            let mut src = mri::mri_alloc(g, out, err, wd, ht, dp, mri::MRI_FLOAT)?;
            for z in 0..dp {
                for y in 0..ht {
                    for x in 0..wd {
                        let v = (x + 3 * y + 7 * z) as f32 * 0.5;
                        larmorx_freesurfer_mri::voxel::mri_set_vox_val(
                            g, err, &mut src, x, y, z, 0, v, 0,
                        )?;
                    }
                }
            }
            let dir = Some(w.fix.as_path());
            let r = match k {
                "A" => tr::transform_apply_type(g, out, err, env, tr, &src, None, interp)?,
                "D" => tr::transform_create_density_map(g, out, err, env, dir, tr, &src, None)?,
                _ if interp < 0 => {
                    tr::transform_apply_inverse(g, out, err, env, dir, tr, &src, None)?
                }
                _ => {
                    tr::transform_apply_inverse_type(g, out, err, env, dir, tr, &src, None, interp)?
                }
            };
            match r {
                larmorx_freesurfer_core::ret::Ret::New(m) => {
                    pmri(o, &m);
                    o.i(i64::from(m.mtype));
                    o.i(i64::from(m.nframes));
                    match &m.chunk {
                        Some(c) if m.ischunked => super::piece11::fnv(o, c),
                        _ => o.0.push_str("nochunk "),
                    }
                }
                _ => o.0.push_str("N "),
            }
        }
        "V" | "X" => {
            let mode = t.int();
            let a = tr::transform_get_src_vol_geom(g, err, tr)?;
            let b = tr::transform_get_dst_vol_geom(g, err, tr)?;
            let mut ms =
                mri::mri_alloc_header(g, out, err, a.width, a.height, a.depth, mri::MRI_UCHAR, 1)?;
            mri::mri_copy_vol_geom_to_mri(&mut ms, &a);
            let mut md =
                mri::mri_alloc_header(g, out, err, b.width, b.height, b.depth, mri::MRI_UCHAR, 1)?;
            mri::mri_copy_vol_geom_to_mri(&mut md, &b);
            let r = if k == "V" {
                tr::transform_vox2ras(g, out, err, tr, (mode != 0).then_some(&ms), None)?
            } else {
                let d = (mode != 0).then_some(&md);
                tr::transform_ras2vox(g, out, err, env, tr, Some(&ms), d)?
            };
            o.i(i64::from(r));
            ptrans(o, tr);
        }
        "O" => {
            let (l, r, into) = (t.int() != 0, t.int() != 0, t.int() != 0);
            let mut ml = larmorx_freesurfer_core::matrix::Matrix::real(4, 4);
            let mut mr = larmorx_freesurfer_core::matrix::Matrix::real(4, 4);
            for i in 1..=4 {
                ml.set_relt(i, i, 1.0);
                mr.set_relt(i, i, 1.0);
            }
            ml.set_relt(1, 4, 2.0);
            ml.set_relt(2, 4, -1.0);
            ml.set_relt(3, 4, 0.5);
            mr.set_relt(1, 1, 0.0);
            mr.set_relt(1, 2, -1.0);
            mr.set_relt(2, 1, 1.0);
            mr.set_relt(2, 2, 0.0);
            mr.set_relt(3, 3, 2.0);
            let (lm, rm) = (l.then_some(&ml), r.then_some(&mr));
            if into {
                if tr::transform_compose_in_place(g, out, err, tr, lm, rm)? {
                    ptrans(o, tr);
                } else {
                    o.0.push_str("N ");
                }
            } else {
                match tr::transform_compose(g, out, err, tr, lm, rm, None)? {
                    larmorx_freesurfer_core::ret::Ret::New(c) => ptrans(o, &c),
                    _ => o.0.push_str("N "),
                }
            }
        }
        _ => {
            let mut m = mri::mri_alloc_header(g, out, err, 2, 2, 2, mri::MRI_UCHAR, 1)?;
            o.i(i64::from(tr::transform_set_mri_vol_geom_to_src(
                g, err, tr, &mut m,
            )?));
            pmri(o, &m);
            o.i(i64::from(tr::transform_set_mri_vol_geom_to_dst(
                g, err, tr, &mut m,
            )?));
            pmri(o, &m);
            o.i(i64::from(tr::transform_copy_vol_geom_to_mri(
                g, err, tr, &mut m,
            )?));
            pmri(o, &m);
        }
    }
    Ok(())
}

/// The port's answer to a piece-32 case.
pub fn run(
    tk: &[&str],
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    o: &mut Out,
    w: &Work,
    extra: &mut Vec<&'static str>,
) -> Result<(), u8> {
    let r = if tk[0] == "m3w" {
        let mut t: Vec<&str> = vec!["vwr"];
        t.extend_from_slice(&tk[1..]);
        super::piece14::run(&t, g, out, err, o, w, extra)
    } else {
        let s = super::Shared::default();
        let (mut o2, mut e2) = (s.clone(), s.clone());
        let r = if tk[0] == "gcw" {
            gcw(tk, g, &mut o2, &mut e2, o, w)
        } else {
            trs(tk, g, &mut o2, &mut e2, o, w)
        }
        .map_err(Fatal::status);
        let text = s.0.borrow().clone();
        let _ = out.write_all(&text);
        // a resampling past fsio's limits, not run (relay 43)
        if text.windows(NOT_RUN.len()).any(|x| x == NOT_RUN) {
            extra.push("SKIP");
        }
        r
    };
    for n in g.notices.as_slice() {
        // FreeSurfer crashes, reads memory contents or depends on the machine's memory
        if matches!(
            n.defect.id,
            "FS-851"
                | "FS-855"
                | "FS-857"
                | "FS-858"
                | "FS-859"
                | "FS-860"
                | "FS-861"
                | "FS-58"
                // relay 41: track C's LTA conversions where FreeSurfer reads memory or crashes,
                // and the conversions this port refuses (FS-928)
                | "FS-504"
                | "FS-957"
                | "FS-928"
        ) {
            extra.push("SKIP");
        }
        if matches!(n.defect.id, "FS-167" | "FS-49") && !n.detail.ends_with("as in FreeSurfer") {
            extra.push("VARY");
        }
        // memory past a volume's buffer (FS-862's read under reproduce)
        if n.defect.id == "FS-53" {
            extra.push("VARY");
        }
    }
    r
}
