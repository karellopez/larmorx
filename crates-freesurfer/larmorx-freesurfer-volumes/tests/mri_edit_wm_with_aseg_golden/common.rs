// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_edit_wm_with_aseg`'s golden cases: 32^3 volumes made with integer arithmetic only (the same
//! bytes on every platform; NaNs and infinities as bit patterns) on a coronal 1 mm grid centred on
//! the origin, written by the port: a two-hemisphere segmentation (`aseg.presurf.mgz`, `int`;
//! cerebral white matter and cortex, lateral and inferior lateral ventricles, hippocampus,
//! amygdala, choroid plexus, thalamus, caudate, putamen, pallidum, accumbens, ventral DC, vessels,
//! cerebellum, brain stem, optic chiasm, lesions; and as bytes, `aseg_u8.mgz`), its white matter
//! (`wm.seg.mgz`, with holes, stray voxels and edits), a T1 (`brain.mgz`; as `float` with NaNs and
//! infinities, `brain_nan.mgz`; `brain.finalsurfs.mgz`), the entorhinal labels (`entowm.mgz`),
//! lesion labels (`wmsa.mgz`) and an earlier output with edits (`keep.mgz`). The program run on
//! them, and each output's digest. Shared by the test `mri_edit_wm_with_aseg_golden` and the
//! example `fs_editwm_golden`, with which `validation/probes/fs-editwm/prog.py --golden` runs the
//! official program on the same inputs.
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

const IO: [&str; 4] = ["wm.seg.mgz", "brain.mgz", "aseg.presurf.mgz", "out.mgz"];

/// The command lines: recon-all's classic call, its v8 call without `-fix-acj` (whose colour
/// table of random colours takes seconds in a debug build; the stand-alone fills cover its fill),
/// the segmentation as bytes (the search for paths to cortex runs: FS-1234), the stand-alone
/// entorhinal fill, one hemisphere with the lesions kept and the edits of an earlier output,
/// `-keep-hailvcp` with negative values (FreeSurfer's unknown options) into a missing folder
/// (FS-1238, FS-1240), `-wmsa` with the other hemisphere and
/// a `float` T1 with NaNs, an option without its value (FS-1239: a crash), `-fix-scm-ha-only`.
pub fn cases() -> Vec<Vec<String>> {
    let v8 = [
        "-keep-in",
        "-fix-ento-wm",
        "entowm.mgz",
        "3",
        "255",
        "255",
        "-fill-seg-wm",
        "-fix-scm-ha",
        "1",
    ];
    let mut c: Vec<Vec<&str>> = vec![
        [&["-keep-in"][..], &IO].concat(),
        [&v8[..], &IO].concat(),
        vec![
            "-keep-in",
            "wm.seg.mgz",
            "brain.mgz",
            "aseg_u8.mgz",
            "out.mgz",
        ],
        vec![
            "-sa-fix-ento-wm",
            "entowm.mgz",
            "2",
            "255",
            "255",
            "brain.finalsurfs.mgz",
            "o.mgz",
        ],
        vec![
            "-lh",
            "-fcd",
            "-fillven",
            "0",
            "-keep",
            "wm.seg.mgz",
            "brain.mgz",
            "aseg.presurf.mgz",
            "keep.mgz",
        ],
        vec![
            "-keep-hailvcp",
            "-5",
            "-6",
            "wm.seg.mgz",
            "brain.mgz",
            "aseg.presurf.mgz",
            "nodir/out.mgz",
        ],
        vec![
            "-wmsa",
            "wmsa.mgz",
            "-rh",
            "wm.seg.mgz",
            "brain_nan.mgz",
            "aseg.presurf.mgz",
            "out.mgz",
        ],
        vec!["-fix-scm-ha"],
        vec![
            "-fix-scm-ha-only",
            "aseg.presurf.mgz",
            "wm.seg.mgz",
            "1",
            "o.mgz",
        ],
    ];
    c.iter_mut()
        .map(|a| a.iter().map(|w| (*w).to_owned()).collect())
        .collect()
}

/// The files each case may write (compared by digest; the inputs written again before each case).
pub const OUTPUTS: [&[&str]; 9] = [
    &["out.mgz"],
    &["out.mgz"],
    &["out.mgz"],
    &["o.mgz"],
    &["keep.mgz"],
    &[],
    &["out.mgz"],
    &[],
    &["o.mgz"],
];

/// The inputs' names, removed before they are written again (a new file, never one truncated and
/// rewritten).
pub const INPUTS: [&str; 9] = [
    "aseg.presurf.mgz",
    "aseg_u8.mgz",
    "wm.seg.mgz",
    "brain.mgz",
    "brain_nan.mgz",
    "brain.finalsurfs.mgz",
    "entowm.mgz",
    "wmsa.mgz",
    "keep.mgz",
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

/// The segmentation: a head (cortex to a squared radius of 196, white matter inside 132) and boxes
/// of each structure on the right (small columns), mirrored on the left with the left labels.
pub fn aseg_value(x: i32, y: i32, z: i32) -> u32 {
    // (right label, left label, x range on the right, y range, z range)
    type Bx = (i32, i32, (i32, i32), (i32, i32), (i32, i32));
    const BOXES: [Bx; 21] = [
        (43, 4, (10, 13), (8, 12), (10, 21)),
        (49, 10, (12, 15), (12, 16), (12, 17)),
        (50, 11, (9, 12), (6, 9), (16, 21)),
        (51, 12, (5, 9), (10, 14), (14, 20)),
        (52, 13, (8, 10), (12, 15), (14, 19)),
        (58, 26, (8, 12), (15, 17), (19, 22)),
        (60, 28, (12, 15), (16, 19), (12, 16)),
        (62, 30, (6, 8), (16, 18), (18, 20)),
        (53, 17, (6, 11), (18, 22), (8, 15)),
        (54, 18, (6, 11), (17, 22), (15, 19)),
        (44, 5, (4, 6), (17, 19), (9, 14)),
        (63, 31, (10, 13), (13, 15), (12, 14)),
        (42, 3, (6, 11), (22, 24), (8, 19)),
        (0, 0, (7, 9), (24, 25), (10, 14)),
        (47, 8, (5, 14), (24, 29), (3, 10)),
        (46, 7, (7, 12), (25, 28), (4, 9)),
        (57, 25, (3, 5), (8, 10), (20, 22)),
        (79, 78, (4, 5), (10, 12), (22, 24)),
        (82, 81, (6, 7), (10, 12), (22, 24)),
        (77, 77, (5, 6), (10, 12), (22, 24)),
        (80, 80, (7, 8), (10, 12), (22, 24)),
    ];
    let d2 = (2 * x - 31) * (2 * x - 31)
        + (2 * y - 31) * (2 * y - 31)
        + (2 * z - 31) * (2 * z - 31) * 4 / 5;
    let mut v = if z < 2 {
        0
    } else if d2 < 4 * 132 {
        if x < 16 { 41 } else { 2 }
    } else if d2 < 4 * 196 {
        if x < 16 { 42 } else { 3 }
    } else {
        0
    };
    let (xr, left) = if x < 16 {
        (x, false)
    } else {
        (N - 1 - x, true)
    };
    for &(r, l, (x0, x1), (y0, y1), (z0, z1)) in &BOXES {
        if (x0..x1).contains(&xr) && (y0..y1).contains(&y) && (z0..z1).contains(&z) {
            v = if left { l } else { r };
        }
    }
    if (14..18).contains(&x) && (20..29).contains(&y) && (8..13).contains(&z) {
        v = 16;
    }
    if (15..17).contains(&x) && (19..21).contains(&y) && (20..23).contains(&z) {
        v = 85;
    }
    v as u32
}

/// The white matter: 110 in the white matter, thalamus, ventral DC and brain stem but for one
/// voxel in 8, in a voxel of the cortex in 14 and of the hippocampus and amygdala in 7, in a
/// background voxel in 64; a few edits (255 in the white matter, 1 elsewhere).
pub fn wm_value(x: i32, y: i32, z: i32) -> u32 {
    let l = aseg_value(x, y, z) as i32;
    let j = jitter(x, y, z, 448);
    let white = matches!(l, 2 | 41 | 10 | 49 | 28 | 60 | 16);
    if white && j == 5 {
        return 255;
    }
    if l > 0 && j == 7 {
        return 1;
    }
    let on = match l {
        _ if white => j % 8 != 0,
        3 | 42 => j % 14 == 3,
        17 | 53 | 18 | 54 => j % 7 == 2,
        0 => j % 64 == 9,
        _ => false,
    };
    if on { 110 } else { 0 }
}

/// The T1 by label, with a jitter of -3 to 3.
pub fn t1_value(x: i32, y: i32, z: i32) -> u32 {
    let v = match aseg_value(x, y, z) {
        2 | 41 => 110,
        3 | 42 => 70,
        4 | 43 | 5 | 44 => 30,
        17 | 53 => 76,
        18 | 54 => 72,
        10 | 49 | 11 | 50 | 12 | 51 | 13 | 52 | 26 | 58 | 28 | 60 => 88,
        7 | 46 => 100,
        8 | 47 => 68,
        16 => 95,
        31 | 63 | 30 | 62 => 60,
        0 => return jitter(x, y, z, 5) as u32,
        _ => 62,
    };
    (v + jitter(x, y, z, 7) - 3) as u32
}

/// Writes the inputs into `dir` (the same bytes every time).
pub fn write_inputs(dir: &Path) {
    let mut g = Globals::new(UpstreamDefects::Reproduce);
    let env = Environment::default();
    let mut aseg = volume(&mut g, MRI_INT, aseg_value);
    let mut aseg_u8 = volume(&mut g, MRI_UCHAR, aseg_value);
    let mut wm = volume(&mut g, MRI_UCHAR, wm_value);
    let mut t1 = volume(&mut g, MRI_UCHAR, t1_value);
    let mut nan = volume(&mut g, MRI_FLOAT, |x, y, z| {
        match (x * 5 + y * 3 + z * 11) % 97 {
            0 => 0x7fc0_0000,
            1 => 0xffc0_0001,
            2 => 0x7f80_0001,
            3 => 0x7f80_0000,
            4 => 0xff80_0000,
            _ => (t1_value(x, y, z) as f32 * 1.5 + 0.25).to_bits(),
        }
    });
    let mut bfs = volume(&mut g, MRI_UCHAR, t1_value);
    let mut ento = volume(&mut g, MRI_INT, |x, y, z| {
        if !(21..23).contains(&y) || !(9..13).contains(&z) {
            0
        } else if (21..25).contains(&x) {
            3006
        } else if (19..21).contains(&x) {
            3201
        } else if (7..11).contains(&x) {
            4006
        } else if (11..13).contains(&x) {
            4201
        } else {
            0
        }
    });
    let mut wmsa = volume(&mut g, MRI_INT, |x, y, z| {
        if (8..10).contains(&y) && (18..20).contains(&z) {
            match x {
                20 => 77,
                21 => 78,
                22 => 79,
                23 => 99,
                24 => 80,
                _ => 0,
            }
        } else {
            0
        }
    });
    let mut keep = volume(&mut g, MRI_UCHAR, |x, y, z| {
        if (20..22).contains(&x) && (9..11).contains(&y) && (15..17).contains(&z) {
            255
        } else if (10..12).contains(&x) && (9..11).contains(&y) && (15..17).contains(&z) {
            1
        } else {
            wm_value(x, y, z)
        }
    });
    let (mut o, mut e) = io();
    std::fs::create_dir_all(dir).expect("a folder");
    let vols = [
        &mut aseg,
        &mut aseg_u8,
        &mut wm,
        &mut t1,
        &mut nan,
        &mut bfs,
        &mut ento,
        &mut wmsa,
        &mut keep,
    ];
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
/// the run's time and larmorx's notices.
pub fn mask(out: &str, err: &str) -> String {
    let mut t = String::new();
    t.push_str(out);
    t.push_str("--\n");
    for l in err.split_inclusive('\n') {
        if l.starts_with("auto filling took ") {
            t.push_str("auto filling took <time> minutes\n");
        } else if !l.starts_with("larmorx: ") {
            t.push_str(l);
        }
    }
    t
}

/// Runs case `k` in `dir` (the inputs written again there, every case's outputs removed first)
/// with `mode`: the exit status and the two streams (masked by [`mask`]).
pub fn run_case(dir: &Path, k: usize, mode: UpstreamDefects) -> (u8, String) {
    for outs in OUTPUTS {
        for name in outs {
            let _ = std::fs::remove_file(dir.join(name));
        }
    }
    write_inputs(dir);
    let args = cases().swap_remove(k);
    let settings = Settings {
        threads: 1,
        mode,
        env: Environment::from_pairs([("OMP_NUM_THREADS", "1")]),
        dir: Some(dir.to_path_buf()),
    };
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let status = larmorx_freesurfer_volumes::mri_edit_wm_with_aseg::main::run(
        &args, &settings, &mut out, &mut err,
    );
    let text = mask(
        &String::from_utf8_lossy(&out),
        &String::from_utf8_lossy(&err),
    );
    (status, text)
}
