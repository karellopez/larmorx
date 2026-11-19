// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_normalize`'s golden cases: 32^3 inputs made with integer arithmetic only (the same bytes
//! on every platform), written as MGZ by the port, the program run on them, and each output's
//! digest (extent, type, voxel sizes, direction cosines, centre and voxels: FreeSurfer's
//! provenance, the command line and its time, left out). Shared by the test
//! `mri_normalize_golden` and the example `fs_normalize_golden`, with which
//! `validation/probes/fs-normalize/prog.py --golden` runs the official program on the same inputs.
#![allow(dead_code)]

use std::path::Path;

use larmorx_core::defects::UpstreamDefects;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::run::{Environment, Settings};
use larmorx_freesurfer_io::mriio::{mri_read_volume, mri_write};
use larmorx_freesurfer_mri::mri::{MRI_INT, MRI_UCHAR, Mri, mri_alloc};
use sha2::{Digest, Sha256};

/// The size of the inputs.
pub const N: i32 = 32;

/// The command lines: recon-all's two calls (`T1.mgz` from `nu.mgz`, `brain.mgz` from
/// `norm.mgz` with the segmentation and the mask), then `-w`'s control-point and bias volumes.
pub const CASES: [&[&str]; 3] = [
    &["-g", "1", "-seed", "1234", "-mprage", "in.mgz", "out.mgz"],
    &[
        "-seed", "1234", "-mprage", "-aseg", "aseg.mgz", "-mask", "mask.mgz", "in.mgz", "out.mgz",
    ],
    &[
        "-seed", "1234", "-mprage", "-n", "1", "-w", "ctrl.mgz", "bias.mgz", "in.mgz", "out.mgz",
    ],
];

/// The files each case writes (compared by digest).
pub const OUTPUTS: [&[&str]; 3] = [
    &["out.mgz"],
    &["out.mgz"],
    &["ctrl.mgz", "bias.mgz", "out.mgz"],
];

fn io() -> (std::io::Sink, std::io::Sink) {
    (std::io::sink(), std::io::sink())
}

fn squared_distance(x: i32, y: i32, z: i32) -> i32 {
    let c = N / 2;
    (x - c) * (x - c) + (y - c) * (y - c) + (z - c) * (z - c)
}

fn volume(g: &mut Globals, mtype: i32, f: impl Fn(i32, i32, i32) -> i32) -> Mri {
    let (mut o, mut e) = io();
    let mut m = mri_alloc(g, &mut o, &mut e, N, N, N, mtype).expect("a volume");
    let size = m.bytes_per_vox;
    let c = m.chunk.as_mut().expect("voxels");
    for z in 0..N {
        for y in 0..N {
            for x in 0..N {
                let i = ((z * N + y) * N + x) as usize * size;
                let v = f(x, y, z);
                if size == 1 {
                    c[i] = v as u8;
                } else {
                    c[i..i + 4].copy_from_slice(&v.to_le_bytes());
                }
            }
        }
    }
    m
}

/// Writes `in.mgz` (a head of uchar: white matter near 110, grey matter, CSF, background, a
/// pattern of -5 to 5, -2 to 2 in the white matter, and a bias of 5 % along x), `aseg.mgz` (int: labels 2 and 41 in the white
/// matter's halves, 3 around them) and `mask.mgz` (uchar 1 inside the head) into `dir`.
pub fn write_inputs(dir: &Path) {
    let mut g = Globals::new(UpstreamDefects::Reproduce);
    let env = Environment::default();
    let mut t1 = volume(&mut g, MRI_UCHAR, |x, y, z| {
        let d2 = squared_distance(x, y, z);
        let base = if d2 < 52 {
            110
        } else if d2 < 92 {
            75
        } else if d2 < 144 {
            35
        } else {
            4
        };
        // a pattern of -5 to 5 (two sums of residues; -2 to 2 in the white matter) and a bias
        // of 5 % along x
        let noise = (x * 7 + y * 13 + z * 29) % 7 + (x * 11 + y * 5 + z * 3) % 5 - 5;
        let noise = if d2 < 52 { noise / 2 } else { noise };
        (base + base * (x - N / 2) * 5 / 3200 + noise).clamp(0, 255)
    });
    let mut aseg = volume(&mut g, MRI_INT, |x, y, z| {
        let d2 = squared_distance(x, y, z);
        if d2 < 52 {
            if x < N / 2 { 2 } else { 41 }
        } else if d2 < 92 {
            3
        } else {
            0
        }
    });
    let mut mask = volume(&mut g, MRI_UCHAR, |x, y, z| {
        i32::from(squared_distance(x, y, z) < 144)
    });
    let (mut o, mut e) = io();
    for (m, name) in [
        (&mut t1, "in.mgz"),
        (&mut aseg, "aseg.mgz"),
        (&mut mask, "mask.mgz"),
    ] {
        let p = dir.join(name);
        mri_write(
            &mut g,
            &mut o,
            &mut e,
            &env,
            None,
            m,
            p.to_str().expect("a UTF-8 name"),
            None,
            0,
        )
        .expect("written");
    }
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

/// Runs case `k` in `dir` (the inputs there) with `mode`: the exit status and standard output
/// (the timing line's numbers masked).
pub fn run_case(dir: &Path, k: usize, mode: UpstreamDefects) -> (u8, String) {
    larmorx_freesurfer_volume::image::register_io();
    larmorx_freesurfer_gca::register_io();
    let args: Vec<String> = CASES[k].iter().map(|s| (*s).to_owned()).collect();
    let settings = Settings {
        threads: 1,
        mode,
        env: Environment::default(),
        dir: Some(dir.to_path_buf()),
    };
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let status =
        larmorx_freesurfer_volumes::mri_normalize::main::run(&args, &settings, &mut out, &mut err);
    let text = String::from_utf8_lossy(&out).into_owned();
    let masked = text.lines().map(mask_line).collect::<Vec<_>>().join("\n");
    (status, masked)
}

/// A line of standard output as the golden hash takes it: the timing line's numbers masked, and
/// a tissue level read from a histogram bin below 0 (`peak at <v> (<bin>)`, `valley at <v>
/// (<bin>)` with a negative bin: FS-597, memory contents in FreeSurfer, 0 in the port) as `?`.
pub fn mask_line(l: &str) -> String {
    if l.starts_with("3D bias adjustment took ") {
        return "3D bias adjustment took M minutes and S seconds.".to_owned();
    }
    // split at single blanks (the padding of `%2.0f` stays as empty words); a masked value
    // loses its padding
    let w: Vec<&str> = l.split(' ').collect();
    let mut keep: Vec<Option<&str>> = w.iter().map(|x| Some(*x)).collect();
    for i in 1..w.len() {
        if w[i] != "at" || !(w[i - 1] == "peak" || w[i - 1] == "valley") {
            continue;
        }
        let Some(j) = (i + 1..w.len()).find(|&j| !w[j].is_empty()) else {
            continue;
        };
        if w.get(j + 1).is_some_and(|n| n.starts_with("(-")) {
            for k in keep.iter_mut().take(j).skip(i + 1) {
                *k = None;
            }
            keep[j] = Some("?");
        }
    }
    keep.into_iter().flatten().collect::<Vec<_>>().join(" ")
}
