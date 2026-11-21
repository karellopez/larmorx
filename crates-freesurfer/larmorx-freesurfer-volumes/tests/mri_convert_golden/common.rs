// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_convert`'s golden cases: a 32^3 input made with integer arithmetic only (the same bytes
//! on every platform), written as MGZ by the port, with a linear transform, an outline series, its
//! colour table and a header-only volume giving the series' size (text and headers written as
//! bytes); the program run on them, and each output's digest (the volume's extent, type, voxel
//! sizes, direction cosines, centre and voxels). Shared by the test `mri_convert_golden` and the
//! example `fs_convert_golden`, with which `validation/probes/fs-convert/golden.py` runs the
//! official program on the same inputs.
#![allow(dead_code)]

use std::path::Path;

use larmorx_core::defects::UpstreamDefects;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::run::{Environment, Settings};
use larmorx_freesurfer_io::mriio::{mri_read_volume, mri_write};
use larmorx_freesurfer_mri::mri::{MRI_FLOAT, mri_alloc};
use sha2::{Digest, Sha256};

/// The size of the input.
pub const N: i32 = 32;

/// The command lines: a conversion to NIfTI; a type change with scaling; the conformed volume
/// (LIA, 1 mm, uchar) cut to 32^3 voxels; a resampling to 2 mm voxels, nearest; a linear
/// transform; an outline series read into a 180 x 180 x 1 volume.
pub const CASES: [&[&str]; 6] = [
    &["in.mgz", "out.nii.gz"],
    &["in.mgz", "out.mgz", "-odt", "uchar"],
    &[
        "in.mgz", "out.mgz", "-c", "-oni", "32", "-onj", "32", "-onk", "32",
    ],
    &["in.mgz", "out.mgz", "-vs", "2", "2", "2", "-rt", "nearest"],
    &["in.mgz", "out.mgz", "-at", "in.lta"],
    &[
        "--in_like",
        "like.mgh",
        "-cf",
        "lut.txt",
        "in_1.otl",
        "out.mgz",
    ],
];

/// The file each case writes (compared by digest).
pub const OUTPUTS: [&str; 6] = [
    "out.nii.gz",
    "out.mgz",
    "out.mgz",
    "out.mgz",
    "out.mgz",
    "out.mgz",
];

fn io() -> (std::io::Sink, std::io::Sink) {
    (std::io::sink(), std::io::sink())
}

/// The input's value at a voxel: a head-like pattern (110 inside a ball, 75 around it, 35, 4
/// outside) with a pattern of -5 to 5 and a quarter-unit step.
fn value(x: i32, y: i32, z: i32) -> f32 {
    let c = N / 2;
    let d2 = (x - c) * (x - c) + (y - c) * (y - c) + (z - c) * (z - c);
    let base = if d2 < 52 {
        110
    } else if d2 < 92 {
        75
    } else if d2 < 144 {
        35
    } else {
        4
    };
    let noise = (x * 7 + y * 13 + z * 29) % 7 + (x * 11 + y * 5 + z * 3) % 5 - 5;
    (base + noise) as f32 + 0.25 * ((x + y + z) % 4) as f32
}

/// The outline file: two rectangles of known labels, one of an unknown one.
fn outline() -> Vec<u8> {
    let mut s = String::from(
        "GDF FILE VERSION 1\nSTART MAIN HEADER\nONUM 3\nCOL_NUM 2\nTYPE ascii short\nEND MAIN HEADER\n",
    );
    for (label, x0, y0, x1, y1) in [
        ("Left Cerebral Cortex", 40, 50, 120, 110),
        ("Left-Cerebral-White-Matter", 60, 70, 100, 90),
        ("Mystery", 200, 200, 230, 240),
    ] {
        let mut pts = Vec::new();
        for x in x0..=x1 {
            pts.push((x, y0));
            pts.push((x, y1));
        }
        for y in y0 + 1..y1 {
            pts.push((x0, y));
            pts.push((x1, y));
        }
        s += &format!(
            "START GDF HEADER\nROW_NUM {}\nSEED {} {}\nLABEL {label}\nEND GDF HEADER\nSTART POINTS\n",
            pts.len(),
            (x0 + x1) / 2,
            (y0 + y1) / 2
        );
        for (x, y) in pts {
            s += &format!("{x} {y}\n");
        }
        s += "END POINTS\n";
    }
    s.into_bytes()
}

/// Writes `in.mgz` (float, [`value`]; RAS axes, voxels of 1 x 1 x 1.5 mm, centre (2, -3,
/// 1.5)), `in.lta` (a RAS-to-RAS shift and rotation), the outline
/// `in_1.otl`, `lut.txt` and the header-only `like.mgh` (180 x 180 x 1) into `dir`.
pub fn write_inputs(dir: &Path) {
    let mut g = Globals::new(UpstreamDefects::Reproduce);
    let env = Environment::default();
    let (mut o, mut e) = io();
    let mut m = mri_alloc(&mut g, &mut o, &mut e, N, N, N, MRI_FLOAT).expect("a volume");
    let c = m.chunk.as_mut().expect("voxels");
    for z in 0..N {
        for y in 0..N {
            for x in 0..N {
                let i = ((z * N + y) * N + x) as usize * 4;
                c[i..i + 4].copy_from_slice(&value(x, y, z).to_le_bytes());
            }
        }
    }
    let v = &mut m.vg;
    (v.x_r, v.x_a, v.x_s) = (1.0, 0.0, 0.0);
    (v.y_r, v.y_a, v.y_s) = (0.0, 1.0, 0.0);
    (v.z_r, v.z_a, v.z_s) = (0.0, 0.0, 1.0);
    (v.c_r, v.c_a, v.c_s) = (2.0, -3.0, 1.5);
    (v.xsize, v.ysize, v.zsize) = (1.0, 1.0, 1.5);
    v.ras_good_flag = 1;
    let p = dir.join("in.mgz");
    mri_write(
        &mut g,
        &mut o,
        &mut e,
        &env,
        None,
        &mut m,
        p.to_str().expect("a UTF-8 name"),
        None,
        -1,
    )
    .expect("written");
    let lta = "type      = 1\nnxforms   = 1\nmean      = 0.0000 0.0000 0.0000\nsigma     = 1.0000\n1 4 4\n0 1 0 2\n-1 0 0 -1.5\n0 0 1 0.5\n0 0 0 1\nsrc volume info\nvalid = 0  # volume info invalid\ndst volume info\nvalid = 0  # volume info invalid\n";
    std::fs::write(dir.join("in.lta"), lta).expect("the LTA");
    std::fs::write(dir.join("in_1.otl"), outline()).expect("the outline");
    std::fs::write(
        dir.join("lut.txt"),
        "0 Unknown 0 0 0 0\n2 Left-Cerebral-White-Matter 245 245 245 0\n3 Left-Cerebral-Cortex 205 62 78 0\n",
    )
    .expect("the colour table");
    let mut like = Vec::new();
    for x in [1i32, 180, 180, 1, 1, 0, 0] {
        like.extend_from_slice(&x.to_be_bytes());
    }
    like.extend_from_slice(&0i16.to_be_bytes());
    like.resize(284, 0);
    std::fs::write(dir.join("like.mgh"), like).expect("the header");
}

/// A volume file's digest: extent, frames, type, voxel sizes, direction cosines, centre (their
/// bits) and the voxels' bytes, as SHA-256 hex.
pub fn digest(path: &Path) -> String {
    let mut g = Globals::new(UpstreamDefects::Reproduce);
    let (mut o, mut e) = io();
    let m = mri_read_volume(
        &mut g,
        &mut o,
        &mut e,
        &Environment::default(),
        None,
        path.to_str().expect("a UTF-8 name"),
    )
    .expect("read")
    .expect("a volume");
    let mut h = Sha256::new();
    let v = &m.vg;
    for i in [v.width, v.height, v.depth, m.nframes, m.mtype] {
        h.update(i.to_le_bytes());
    }
    for f in [
        v.xsize, v.ysize, v.zsize, v.x_r, v.x_a, v.x_s, v.y_r, v.y_a, v.y_s, v.z_r, v.z_a, v.z_s,
        v.c_r, v.c_a, v.c_s,
    ] {
        h.update(f.to_bits().to_le_bytes());
    }
    h.update(m.chunk.as_ref().expect("voxels"));
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// A line of standard output as the golden hash takes it: the geometry's validity flag, which
/// FreeSurfer prints from memory never written (FS-1053), masked.
pub fn mask_line(l: &str) -> String {
    if l.starts_with("valid   : ") {
        return "valid   : <masked>".to_owned();
    }
    l.to_owned()
}

/// Runs case `k` in `dir` (the inputs there) with `mode`: the exit status and standard output
/// (masked by [`mask_line`]).
pub fn run_case(dir: &Path, k: usize, mode: UpstreamDefects) -> (u8, String) {
    larmorx_freesurfer_volume::image::register_io();
    larmorx_freesurfer_gca::register_io();
    let args: Vec<String> = CASES[k].iter().map(|s| (*s).to_owned()).collect();
    let settings = Settings {
        threads: 1,
        mode,
        env: Environment::from_pairs([("OMP_NUM_THREADS", "1")]),
        dir: Some(dir.to_path_buf()),
    };
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let status =
        larmorx_freesurfer_volumes::mri_convert::main::run(&args, &settings, &mut out, &mut err);
    let text = String::from_utf8_lossy(&out).into_owned();
    let masked = text.lines().map(mask_line).collect::<Vec<_>>().join("\n");
    (status, masked)
}
