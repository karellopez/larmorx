// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_segstats`'s golden cases: 32^3 inputs made with integer arithmetic only (the same bytes
//! on every platform), written as MGZ by the port, the program run on them, and each output's
//! digest (a volume's extent, type, voxel sizes, direction cosines, centre and voxels; a text
//! file's bytes without FreeSurfer's provenance lines). Shared by the test `mri_segstats_golden`
//! and the example `fs_segstats_golden`, with which `validation/probes/fs-segstats/prog.py
//! --golden` runs the official program on the same inputs.
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

/// The command lines: the statistics of the input with the signal-to-noise ratio; the
/// partial-volume corrected volumes of given ids with robust statistics; a mask, the cortex
/// excluded, a replacement and an erosion of the segmentation; the waveforms (table, means,
/// volume) with the old table and accumulated means; a segmentation of 1s from the input with
/// its square root; `--rescale-by-seg`; the segmentation's own statistics, whose signal-to-noise
/// ratios divide by a deviation of 0 (a NaN for label 0, an infinity for 2).
pub const CASES: [&[&str]; 7] = [
    &[
        "--seg",
        "seg.mgz",
        "--sum",
        "s1.stats",
        "--in",
        "t1.mgz",
        "--snr",
        "--excludeid",
        "0",
    ],
    &[
        "--seg", "seg.mgz", "--sum", "s2.stats", "--pv", "t1.mgz", "--in", "t1.mgz", "--id", "2",
        "41", "3", "17", "99", "--empty", "--robust", "10",
    ],
    &[
        "--seg",
        "seg.mgz",
        "--sum",
        "s3.stats",
        "--mask",
        "t1.mgz",
        "--maskthresh",
        "50",
        "--in",
        "t1.mgz",
        "--excl-ctxgmwm",
        "--replace",
        "17",
        "53",
        "--seg-erode",
        "1",
    ],
    &[
        "--seg",
        "seg.mgz",
        "--in",
        "t1.mgz",
        "--avgwf",
        "wf.txt",
        "--sfavg",
        "sf.txt",
        "--avgwfvol",
        "wf.mgz",
        "--sum",
        "s4.stats",
        "--no-newprint",
        "--acc",
    ],
    &[
        "--seg-from-input",
        "--in",
        "t1.mgz",
        "--sum",
        "s5.stats",
        "--sqrt",
    ],
    &[
        "--rescale-by-seg",
        "t1.mgz",
        "seg.mgz",
        "100",
        "rescaled.mgz",
        "2",
        "41",
    ],
    &[
        "--seg", "seg.mgz", "--sum", "s6.stats", "--in", "seg.mgz", "--snr", "--id", "0", "2",
    ],
];

/// The files each case writes (compared by digest).
pub const OUTPUTS: [&[&str]; 7] = [
    &["s1.stats"],
    &["s2.stats"],
    &["s3.stats"],
    &["wf.txt", "sf.txt", "wf.mgz", "s4.stats"],
    &["s5.stats"],
    &["rescaled.mgz"],
    &["s6.stats"],
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

/// The segmentation's label at a voxel: white matter 2 and 41 in the halves of a ball, cortex 3
/// around it, hippocampi 17 and 53, a block of cerebellar white matter 7 with a tunnel through it
/// (a handle for the topology fix), background 0.
fn label(x: i32, y: i32, z: i32) -> i32 {
    let d2 = squared_distance(x, y, z);
    if (2..10).contains(&x) && (3..11).contains(&y) && (2..12).contains(&z) {
        if (5..7).contains(&x) && (6..8).contains(&y) {
            return 0;
        }
        return 7;
    }
    if (14..16).contains(&y) && (11..13).contains(&z) && (9..23).contains(&x) {
        return if x < N / 2 { 17 } else { 53 };
    }
    if d2 < 52 {
        if x < N / 2 { 2 } else { 41 }
    } else if d2 < 92 {
        3
    } else {
        0
    }
}

/// Writes `seg.mgz` (int, [`label`]) and `t1.mgz` (uchar: a head-like pattern, white matter near
/// 110, grey matter, CSF, a pattern of -5 to 5) into `dir`.
pub fn write_inputs(dir: &Path) {
    let mut g = Globals::new(UpstreamDefects::Reproduce);
    let env = Environment::default();
    let mut seg = volume(&mut g, MRI_INT, label);
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
        let noise = (x * 7 + y * 13 + z * 29) % 7 + (x * 11 + y * 5 + z * 3) % 5 - 5;
        (base + noise).clamp(0, 255)
    });
    let (mut o, mut e) = io();
    for (m, name) in [(&mut seg, "seg.mgz"), (&mut t1, "t1.mgz")] {
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
            -1,
        )
        .expect("written");
    }
}

/// A volume file's digest: extent, frames, type, voxel sizes, direction cosines, centre (their
/// bits) and the voxels' bytes, as SHA-256 hex.
pub fn volume_digest(path: &Path) -> String {
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

/// A text output's lines as the digest takes them: FreeSurfer's provenance (the command line,
/// the machine, the user and the files' times) left out.
pub fn text_without_provenance(t: &str) -> String {
    t.lines()
        .filter(|l| {
            !l.contains("TimeStamp")
                && ![
                    "# cmdline",
                    "# sysname",
                    "# hostname",
                    "# machine",
                    "# user",
                ]
                .iter()
                .any(|k| l.starts_with(k))
        })
        .map(|l| format!("{l}\n"))
        .collect()
}

/// A written file's digest by its kind: a text file (`.stats`, `.txt`), or a volume.
pub fn digest(path: &Path) -> String {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if name.ends_with(".stats") || name.ends_with(".txt") {
        let t = std::fs::read_to_string(path).expect("a text file");
        Sha256::digest(text_without_provenance(&t).as_bytes())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    } else {
        volume_digest(path)
    }
}

/// A line of standard output as the golden hash takes it: the report's lines that name the
/// machine, the user and the command line masked.
pub fn mask_line(l: &str) -> String {
    for key in [
        "cwd ",
        "cmdline ",
        "sysname  ",
        "hostname ",
        "machine  ",
        "user     ",
    ] {
        if l.starts_with(key) {
            return format!("{key}<masked>");
        }
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
        larmorx_freesurfer_volumes::mri_segstats::main::run(&args, &settings, &mut out, &mut err);
    let text = String::from_utf8_lossy(&out).into_owned();
    let masked = text.lines().map(mask_line).collect::<Vec<_>>().join("\n");
    (status, masked)
}
