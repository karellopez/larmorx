// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_pretess`'s golden cases: 32^3 volumes made with integer arithmetic only (the same bytes on
//! every platform; NaNs and infinities as bit patterns) on a coronal 1 mm grid centred on the
//! origin, written by the port: a white-matter segmentation (`wm.mgz`, bytes: a pseudo-random
//! pattern in a ball, so that voxels meet at edges and corners in every direction, with the edit
//! values 255 and 1 and values below 5), a filled volume (`filled.mgz`: the labels 255 and 127 of
//! the two halves), an intensity volume (`norm.mgz`, bytes, with equal values) and its `float` form
//! with NaNs and infinities (`norm_nan.mgz`), the white matter as `float` with fractions and a
//! value above 255 (`wm_float.mgz`) and an earlier output with edits (`old.mgz`). The program run
//! on them, and each output's digest. Shared by the test `mri_pretess_golden` and the example
//! `fs_pretess_golden`, with which `validation/probes/fs-pretess/prog.py --golden` runs the
//! official program on the same inputs.
#![allow(dead_code)]

use std::path::Path;

use larmorx_core::defects::UpstreamDefects;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::run::{Environment, Settings};
use larmorx_freesurfer_io::mriio::{mri_read_volume, mri_write};
use larmorx_freesurfer_mri::mri::{MRI_FLOAT, MRI_UCHAR, Mri, mri_alloc};
use sha2::{Digest, Sha256};

/// The size of the coronal volumes.
pub const N: i32 = 32;

/// The command lines: recon-all's white-matter call and its filled call for 255, `-nocorners`
/// with 127 on the `float` intensities with NaNs, `-keep` over an earlier output, `-test` on the
/// `float` white matter (FS-1284; no output), a label above 255 (FS-1282, FS-1283), an output into
/// a missing folder (FS-1285), `-debug_voxel` without its last value (FS-1281: a crash).
pub fn cases() -> Vec<Vec<String>> {
    let c: [&[&str]; 8] = [
        &["wm.mgz", "wm", "norm.mgz", "out.mgz"],
        &["filled.mgz", "255", "norm.mgz", "out.mgz"],
        &["-nocorners", "filled.mgz", "127", "norm_nan.mgz", "out.mgz"],
        &["-keep", "wm.mgz", "wm", "norm.mgz", "old.mgz"],
        &["-test", "wm_float.mgz", "wm", "norm.mgz", "out.mgz"],
        &["wm.mgz", "300", "norm.mgz", "out.mgz"],
        &["wm.mgz", "wm", "norm.mgz", "nodir/out.mgz"],
        &["-debug_voxel", "1", "2"],
    ];
    c.iter()
        .map(|a| a.iter().map(|w| (*w).to_owned()).collect())
        .collect()
}

/// The files each case may write (compared by digest; the inputs written again before each case).
pub const OUTPUTS: [&[&str]; 8] = [
    &["out.mgz"],
    &["out.mgz"],
    &["out.mgz"],
    &["old.mgz"],
    &["out.mgz"],
    &["out.mgz"],
    &[],
    &[],
];

/// The variables each case adds: the `float` intensities' NaNs kept as they are (`FS_LEAVE_NANS`;
/// without it the reader makes them 0).
pub const ENVS: [&[(&str, &str)]; 8] =
    [&[], &[], &[("FS_LEAVE_NANS", "1")], &[], &[], &[], &[], &[]];

/// The inputs' names, removed before they are written again (a new file, never one truncated and
/// rewritten).
pub const INPUTS: [&str; 6] = [
    "wm.mgz",
    "filled.mgz",
    "norm.mgz",
    "norm_nan.mgz",
    "wm_float.mgz",
    "old.mgz",
];

fn io() -> (std::io::Sink, std::io::Sink) {
    (std::io::sink(), std::io::sink())
}

/// A 32^3 volume of `mtype` whose voxel `(x, y, z)` holds the bits `f(x, y, z)`, with coronal
/// voxels of 1 mm centred on the origin.
fn volume(g: &mut Globals, mtype: i32, f: impl Fn(i32, i32, i32) -> u32) -> Mri {
    let (mut o, mut e) = io();
    let mut m = mri_alloc(g, &mut o, &mut e, N, N, N, mtype).expect("a volume");
    m.vg.xsize = 1.0;
    m.vg.ysize = 1.0;
    m.vg.zsize = 1.0;
    [m.vg.x_r, m.vg.x_a, m.vg.x_s] = [-1.0, 0.0, 0.0];
    [m.vg.y_r, m.vg.y_a, m.vg.y_s] = [0.0, 0.0, -1.0];
    [m.vg.z_r, m.vg.z_a, m.vg.z_s] = [0.0, 1.0, 0.0];
    let bytes = m.bytes_per_vox;
    let c = m.chunk.as_mut().expect("voxels");
    for z in 0..N {
        for y in 0..N {
            for x in 0..N {
                let i = ((z * N + y) * N + x) as usize * bytes;
                let v = f(x, y, z);
                if bytes == 1 {
                    c[i] = v as u8;
                } else {
                    c[i..i + 4].copy_from_slice(&v.to_le_bytes());
                }
            }
        }
    }
    m
}

/// A small pseudo-random integer of the voxel, 0 to `n - 1`.
fn jitter(x: i32, y: i32, z: i32, n: i32) -> i32 {
    (x * 73 + y * 151 + z * 283 + x * y * 7 + y * z * 13 + x * z * 3) % n
}

/// Inside the ball of squared radius 196 (in half voxels: 4 * 196) around the centre.
fn inside(x: i32, y: i32, z: i32) -> bool {
    (2 * x - 31) * (2 * x - 31) + (2 * y - 31) * (2 * y - 31) + (2 * z - 31) * (2 * z - 31)
        < 4 * 196
}

/// The white matter: in the ball, 110 where the voxel's pseudo-random number of 7 is below 4 (so
/// voxels meet at edges and corners), 255 or 1 for one voxel in 97 of each, a value from 2 to 5 in
/// the background now and then.
pub fn wm_value(x: i32, y: i32, z: i32) -> u32 {
    if !inside(x, y, z) {
        return 0;
    }
    let j = jitter(x, y, z, 97);
    if j == 11 {
        return 255;
    }
    if j == 23 {
        return 1;
    }
    if jitter(x + 3, y, z + 5, 7) < 4 {
        110
    } else if j % 13 == 4 {
        (2 + j % 4) as u32
    } else {
        0
    }
}

/// The filled volume: the white matter's voxels at or above 5 as 255 in the columns below 16 and
/// 127 in the others.
pub fn filled_value(x: i32, y: i32, z: i32) -> u32 {
    if wm_value(x, y, z) < 5 {
        0
    } else if x < 16 {
        255
    } else {
        127
    }
}

/// The intensities: multiples of 20 from 0 to 220 (many equal ones).
pub fn norm_value(x: i32, y: i32, z: i32) -> u32 {
    (jitter(x, y + 1, z + 2, 12) * 20) as u32
}

/// Writes the inputs into `dir` (the same bytes every time).
pub fn write_inputs(dir: &Path) {
    let mut g = Globals::new(UpstreamDefects::Reproduce);
    let env = Environment::default();
    let mut wm = volume(&mut g, MRI_UCHAR, wm_value);
    let mut filled = volume(&mut g, MRI_UCHAR, filled_value);
    let mut norm = volume(&mut g, MRI_UCHAR, norm_value);
    let mut nan = volume(&mut g, MRI_FLOAT, |x, y, z| {
        match (x * 5 + y * 3 + z * 11) % 61 {
            0 => 0x7fc0_0000,
            1 => 0xffc0_0001,
            2 => 0x7f80_0001,
            3 => 0x7f80_0000,
            4 => 0xff80_0000,
            5 => 0x8000_0000,
            _ => (norm_value(x, y, z) as f32 * 0.5 + 0.25).to_bits(),
        }
    });
    let mut wf = volume(&mut g, MRI_FLOAT, |x, y, z| {
        let v = wm_value(x, y, z) as f32;
        let f = match jitter(x, y, z, 31) {
            0 => 0.5,
            1 => -0.25,
            _ => 0.0,
        };
        if (x, y, z) == (1, 1, 1) {
            300f32.to_bits()
        } else {
            (v + f).to_bits()
        }
    });
    let mut old = volume(&mut g, MRI_UCHAR, |x, y, z| {
        if (10..13).contains(&x) && (14..17).contains(&y) && (12..15).contains(&z) {
            255
        } else if (20..22).contains(&x) && (14..16).contains(&y) && (16..19).contains(&z) {
            1
        } else {
            wm_value(x, y, z)
        }
    });
    let (mut o, mut e) = io();
    std::fs::create_dir_all(dir).expect("a folder");
    let vols = [&mut wm, &mut filled, &mut norm, &mut nan, &mut wf, &mut old];
    for (m, name) in vols.into_iter().zip(INPUTS) {
        let p = dir.join(name);
        let _ = std::fs::remove_file(&p);
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
        &Environment::from_pairs([("FS_LEAVE_NANS", "1")]),
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

/// A written file's digest: a volume's (`.mgz`), or `absent` for a file the run did not write.
pub fn digest(path: &Path) -> String {
    if !path.exists() {
        return "absent".to_owned();
    }
    volume_digest(path)
}

/// The two streams as the golden hash takes them: standard output, then standard error without
/// larmorx's notices.
pub fn mask(out: &str, err: &str) -> String {
    let mut t = String::new();
    t.push_str(out);
    t.push_str("--\n");
    for l in err.split_inclusive('\n') {
        if !l.starts_with("larmorx: ") {
            t.push_str(l);
        }
    }
    t
}

/// Runs `args` with the variables `vars` in `dir` (the inputs written again there, every case's
/// outputs removed first) with `mode`: the exit status and the two streams (masked by [`mask`]).
pub fn run_args(
    dir: &Path,
    args: &[String],
    vars: &[(&str, &str)],
    mode: UpstreamDefects,
) -> (u8, String) {
    for outs in OUTPUTS {
        for name in outs {
            let _ = std::fs::remove_file(dir.join(name));
        }
    }
    write_inputs(dir);
    let settings = Settings {
        threads: 1,
        mode,
        env: Environment::from_pairs(
            [("OMP_NUM_THREADS", "1")]
                .into_iter()
                .chain(vars.iter().copied()),
        ),
        dir: Some(dir.to_path_buf()),
    };
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let status =
        larmorx_freesurfer_volumes::mri_pretess::main::run(args, &settings, &mut out, &mut err);
    let text = mask(
        &String::from_utf8_lossy(&out),
        &String::from_utf8_lossy(&err),
    );
    (status, text)
}

/// Runs case `k` (its arguments and variables) in `dir` with `mode` ([`run_args`]).
pub fn run_case(dir: &Path, k: usize, mode: UpstreamDefects) -> (u8, String) {
    run_args(dir, &cases().swap_remove(k), ENVS[k], mode)
}
