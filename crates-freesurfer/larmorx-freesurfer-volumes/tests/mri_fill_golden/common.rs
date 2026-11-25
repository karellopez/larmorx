// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_fill`'s golden cases: 32^3 volumes made with integer arithmetic only (the same bytes on
//! every platform; NaNs as bit patterns) on a coronal 2 mm grid centred on the
//! origin, written by the port: a white matter volume (`wm.mgz`, bytes: two hemispheres joined by
//! a thin bridge above a brainstem, with the edit values 255 and 1, the fill labels 200 to 240,
//! holes and specks), its segmentation (`aseg.mgz`, `int`: white matter, cortex, the bridge's
//! corpus callosum labels, brainstem, cerebellum, lesions and hypointensities), an intensity
//! volume (`norm.mgz`), the white matter as `float` with NaNs and values near 255 (`wm_nan.mgz`) and a
//! colour table (`lut.txt`). The program run on them, and each output's digest. Shared by the test
//! `mri_fill_golden` and the example `fs_fill_golden`, with which
//! `validation/probes/fs-fill/prog.py --golden` runs the official program on the same inputs.
#![allow(dead_code)]

use std::path::Path;

use larmorx_core::defects::UpstreamDefects;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::run::{Environment, Settings};
use larmorx_freesurfer_io::mriio::{mri_read_volume, mri_write};
use larmorx_freesurfer_mri::mri::{MRI_FLOAT, MRI_INT, MRI_UCHAR, Mri, mri_alloc};
use sha2::{Digest, Sha256};

/// The size of the coronal volumes.
pub const N: i32 = 32;

/// The command lines: recon-all's call (the cutting-plane log and the colour table; no transform:
/// the identity), the fill without a segmentation (the cutting planes found, two flood fills
/// combined), `-topofix`, `-fillonly` from a voxel seed, the `float` white matter with NaNs (quiet,
/// negative, signalling) and values near 255, `-lhonly` without a segmentation (FS-1301: a crash), `-rval` without its value
/// (FS-1292: a crash), an output into a missing folder (FS-1299).
pub fn cases() -> Vec<Vec<String>> {
    let c: [&[&str]; 8] = [
        &[
            "-a",
            "cut.log",
            "-segmentation",
            "aseg.mgz",
            "-ctab",
            "lut.txt",
            "wm.mgz",
            "out.mgz",
        ],
        &["-L", "cc.log", "wm.mgz", "out.mgz"],
        &[
            "-topofix",
            "norm.mgz",
            "-segmentation",
            "aseg.mgz",
            "wm.mgz",
            "out.mgz",
        ],
        &["-fillonly", "-rhv", "6", "8", "16", "wm.mgz", "out.mgz"],
        &["-segmentation", "aseg.mgz", "wm_nan.mgz", "out.mgz"],
        &["-lhonly", "wm.mgz", "out.mgz"],
        &["-rval"],
        &["-segmentation", "aseg.mgz", "wm.mgz", "nodir/out.mgz"],
    ];
    c.iter()
        .map(|a| a.iter().map(|w| (*w).to_owned()).collect())
        .collect()
}

/// The files each case may write (compared by digest; the inputs written again before each case).
pub const OUTPUTS: [&[&str]; 8] = [
    &["out.mgz", "cut.log"],
    &["out.mgz", "cc.log"],
    &["out.mgz"],
    &["out.mgz"],
    &["out.mgz"],
    &["out.mgz"],
    &[],
    &[],
];

/// The variables each case adds: the `float` white matter's NaNs kept as they are
/// (`FS_LEAVE_NANS`; without it the reader makes them 0).
pub const ENVS: [&[(&str, &str)]; 8] =
    [&[], &[], &[], &[], &[("FS_LEAVE_NANS", "1")], &[], &[], &[]];

/// The inputs' names, removed before they are written again (a new file, never one truncated and
/// rewritten).
pub const INPUTS: [&str; 4] = ["wm.mgz", "aseg.mgz", "norm.mgz", "wm_nan.mgz"];

/// The colour table (`lut.txt`).
pub const LUT: &str = "0   Unknown        0   0   0   0\n127 Right-Hemisphere 255 0 0 0\n255 Left-Hemisphere 0 255 0 0\n";

fn io() -> (std::io::Sink, std::io::Sink) {
    (std::io::sink(), std::io::sink())
}

/// A 32^3 volume of `mtype` whose voxel `(x, y, z)` holds the bits `f(x, y, z)`, with coronal
/// voxels of 2 mm centred on the origin.
fn volume(g: &mut Globals, mtype: i32, f: impl Fn(i32, i32, i32) -> u32) -> Mri {
    let (mut o, mut e) = io();
    let mut m = mri_alloc(g, &mut o, &mut e, N, N, N, mtype).expect("a volume");
    m.vg.xsize = 2.0;
    m.vg.ysize = 2.0;
    m.vg.zsize = 2.0;
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

fn within(v: i32, lo: i32, hi: i32) -> bool {
    (lo..=hi).contains(&v)
}

/// The hemispheres (x 2-13 right, 18-29 left; y 2-14; z 4-27), the bridge (x 13-18, y 6-10, z
/// 6-25), the brainstem (x 11-20, y 17-30, z 12-21): 1 right, 2 left, 3 bridge, 4 brainstem.
fn part(x: i32, y: i32, z: i32) -> i32 {
    if within(y, 2, 14) && within(z, 4, 27) {
        if within(x, 2, 13) {
            return 1;
        }
        if within(x, 18, 29) {
            return 2;
        }
    }
    if within(x, 13, 18) && within(y, 6, 10) && within(z, 6, 25) {
        return 3;
    }
    if within(x, 11, 20) && within(y, 17, 30) && within(z, 12, 21) {
        return 4;
    }
    0
}

/// The white matter: 110 in the parts, with one voxel in 97 of each of the edit values 255 and
/// 1, of 0 (holes), of 250 and of each fill label 200 to 240; specks of 110 outside.
pub fn wm_value(x: i32, y: i32, z: i32) -> u32 {
    let j = jitter(x, y, z, 97);
    if part(x, y, z) == 0 {
        return if j == 5 { 110 } else { 0 };
    }
    match j {
        11 => 255,
        23 => 1,
        37 => 0,
        41 => 250,
        53 => 200,
        59 => 210,
        61 => 220,
        67 => 230,
        71 => 240,
        79 => 3,
        _ => 110,
    }
}

/// The segmentation: right (41) and left (2) white matter, their cortex (42, 3) around them, the
/// bridge's halves with corpus callosum labels (251 to 255) inside, the brainstem (16), the
/// cerebellum (7, 8, 46, 47), a lesion (25) or a hypointensity (77) at some dark voxels.
pub fn seg_value(x: i32, y: i32, z: i32) -> u32 {
    let p = part(x, y, z);
    let dark = wm_value(x, y, z) < 5 && p != 0;
    let v: i32 = match p {
        1 => 41,
        2 => 2,
        3 => {
            if within(x, 14, 17) && within(y, 7, 9) && within(z, 8, 23) {
                251 + (x + z) % 5
            } else if x <= 15 {
                41
            } else {
                2
            }
        }
        4 => 16,
        _ => {
            if within(y, 1, 15) && within(z, 3, 28) && within(x, 1, 14) {
                42
            } else if within(y, 1, 15) && within(z, 3, 28) && within(x, 17, 30) {
                3
            } else {
                0
            }
        }
    };
    let v = if within(x, 3, 7) && within(y, 10, 13) && within(z, 4, 7) {
        if within(y, 12, 13) && within(z, 4, 5) {
            7
        } else {
            8
        }
    } else if within(x, 24, 28) && within(y, 10, 13) && within(z, 4, 7) {
        if within(y, 12, 13) && within(z, 4, 5) {
            46
        } else {
            47
        }
    } else if dark && jitter(x, y, z, 3) == 0 {
        25
    } else if dark && jitter(x, y, z, 3) == 1 {
        77
    } else {
        v
    };
    v as u32
}

/// The intensities: 50 more in the parts, multiples of 10 (many equal ones).
pub fn norm_value(x: i32, y: i32, z: i32) -> u32 {
    (jitter(x, y + 1, z + 2, 12) * 10 + if part(x, y, z) != 0 { 50 } else { 0 }) as u32
}

/// Writes the inputs into `dir` (the same bytes every time).
pub fn write_inputs(dir: &Path) {
    let mut g = Globals::new(UpstreamDefects::Reproduce);
    let env = Environment::default();
    let mut wm = volume(&mut g, MRI_UCHAR, wm_value);
    let mut seg = volume(&mut g, MRI_INT, seg_value);
    let mut norm = volume(&mut g, MRI_UCHAR, norm_value);
    // the NaNs and the values near 255 only in the parts (an edited-on voxel far from the
    // hemispheres widens its search window to 20 voxels)
    let mut nan = volume(&mut g, MRI_FLOAT, |x, y, z| {
        let k = if part(x, y, z) != 0 {
            (x * 5 + y * 3 + z * 11) % 61
        } else {
            60
        };
        match k {
            0 => 0x7fc0_0000,
            1 => 0xffc0_0001,
            2 => 0x7fa0_0000,
            3 => 0x437f_0000 + (x as u32 & 7),
            _ => (wm_value(x, y, z) as f32 + 0.25).to_bits(),
        }
    });
    let (mut o, mut e) = io();
    std::fs::create_dir_all(dir).expect("a folder");
    let vols = [&mut wm, &mut seg, &mut norm, &mut nan];
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
    let p = dir.join("lut.txt");
    let _ = std::fs::remove_file(&p);
    std::fs::write(&p, LUT).expect("written");
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

/// A written file's digest: a volume's (`.mgz`), a text's bytes (SHA-256 hex), or `absent` for a
/// file the run did not write.
pub fn digest(path: &Path) -> String {
    if !path.exists() {
        return "absent".to_owned();
    }
    if path.extension().is_some_and(|e| e == "mgz") {
        return volume_digest(path);
    }
    let b = std::fs::read(path).expect("a file");
    Sha256::digest(&b)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// A text as Python's universal newlines read it (`prog.py --golden` takes the official's that
/// way): `\r\n` and a lone `\r` as `\n` (the fill's passes start with `\r`).
fn newlines(s: &str) -> String {
    s.replace("\r\n", "\n").replace('\r', "\n")
}

/// The two streams as the golden hash takes them: standard output, then standard error without
/// larmorx's notices and the fill's time.
pub fn mask(out: &str, err: &str) -> String {
    let mut t = String::new();
    t.push_str(&newlines(out));
    t.push_str("--\n");
    let mut rest = newlines(err);
    // the notices follow at the end, also after a line cut short by a crash
    if let Some(i) = rest.find("larmorx: upstream defect") {
        rest.truncate(i);
    }
    for l in rest.split_inclusive('\n') {
        if l.starts_with("filling took ") {
            t.push_str("filling took <t> minutes\n");
        } else {
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
        larmorx_freesurfer_volumes::mri_fill::main::run(args, &settings, &mut out, &mut err);
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
