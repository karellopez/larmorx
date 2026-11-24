// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_segment`'s golden cases: 32^3 volumes made with integer arithmetic only (the same bytes on
//! every platform; NaNs and infinities as bit patterns) on a coronal 1 mm grid centred on the
//! origin, written by the port: a brain-like intensity volume (a white core, a graded border, a
//! gray shell, fluid, a white plate one voxel thick along the rows, a thicker one, bright voxels
//! outside, voxels touching along an edge), the same as `float` with NaNs and infinities, its
//! segmentation (`aseg.presurf.mgz`) and an earlier output with edits (`keep.mgz`). The program
//! run on them, and each output's digest (a volume's extent, type, voxel sizes, direction cosines,
//! centre and voxels; a text file's bytes). Shared by the test `mri_segment_golden` and the
//! example `fs_segment_golden`, with which `validation/probes/fs-segment/prog.py --golden` runs
//! the official program on the same inputs.
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

/// The command lines: recon-all's call, the defaults, the NaN volume, without the statistics and
/// the thickening, the segmentation's path (`-seg`), the basal ganglia and ventricles, the
/// strands' removal only (FS-1230), the edits kept, `-b` (FS-1220), an output into a folder that
/// does not exist (FS-1226), an option given last (FS-1219: a crash).
pub fn cases() -> Vec<Vec<String>> {
    let c: [&[&str]; 11] = [
        &["-wsizemm", "13", "-mprage", "in.mgz", "out.mgz"],
        &["in.mgz", "out.mgz"],
        &["-wsizemm", "13", "-mprage", "nan.mgz", "out.mgz"],
        &["-noauto", "-thicken", "0", "in.mgz", "out.mgz"],
        &[
            "-seg",
            "aseg.presurf.mgz",
            "3",
            "3",
            "2",
            "2",
            "in.mgz",
            "out.mgz",
        ],
        &["-fillbg", "-fillv", "in.mgz", "out.mgz"],
        &["-thickenonly", "in.mgz", "out.mgz"],
        &["-keep", "in.mgz", "keep.mgz"],
        &["-b", "0.5", "-polvlen", "2", "in.mgz", "out.mgz"],
        &["in.mgz", "nodir/out.mgz"],
        &["-slope"],
    ];
    c.iter()
        .map(|a| a.iter().map(|w| (*w).to_owned()).collect())
        .collect()
}

/// The files each case may write (compared by digest; the inputs written again before each case).
pub const OUTPUTS: [&[&str]; 11] = [
    &["out.mgz", "segment.dat"],
    &["out.mgz", "segment.dat"],
    &["out.mgz", "segment.dat"],
    &["out.mgz"],
    &["out.mgz", "wm.pdf.dat", "ctx.pdf.dat"],
    &["out.mgz", "segment.dat"],
    &["out.mgz"],
    &["keep.mgz", "segment.dat"],
    &["out.mgz", "segment.dat"],
    &["segment.dat"],
    &[],
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
    (x * 73 + y * 151 + z * 283 + x * y * 7 + y * z * 13) % n
}

/// The intensity: by the squared distance from the centre (16, 16, 16) a white core (110, up to
/// 49), a border falling to the gray (to 81), the gray shell (70, to 100), fluid (35, to 144),
/// background 0 to 1; a white plate one voxel thick along the rows (y = 16) from x = 2 to 9, a
/// plate two voxels thick (y 20 and 21), bright voxels outside (150), voxels touching only along
/// edges.
fn intensity(x: i32, y: i32, z: i32) -> u32 {
    let d2 = (x - 16) * (x - 16) + (y - 16) * (y - 16) + (z - 16) * (z - 16);
    let j = jitter(x, y, z, 7) - 3;
    let mut v = if d2 < 49 {
        110 + j
    } else if d2 < 81 {
        110 - (d2 - 49) + j
    } else if d2 < 100 {
        70 + 2 * j
    } else if d2 < 144 {
        35 + j
    } else {
        jitter(x, y, z, 2)
    };
    if y == 16 && (2..10).contains(&x) && (6..26).contains(&z) {
        v = 110 + j / 2;
    }
    if (20..22).contains(&y) && (22..30).contains(&x) && (8..24).contains(&z) {
        v = 112;
    }
    if (27..30).contains(&x) && (2..5).contains(&y) && (14..17).contains(&z) {
        v = 150;
    }
    if (x, y, z) == (3, 3, 3) || (x, y, z) == (4, 4, 3) || (x, y, z) == (5, 5, 4) {
        v = 110;
    }
    v.clamp(0, 255) as u32
}

/// The segmentation: white matter (2 on the left, x below 16; 41 on the right) from an intensity
/// of 92, cortex (3, 42) from 50.
fn aseg_value(x: i32, y: i32, z: i32) -> u32 {
    let v = intensity(x, y, z);
    let left = x < 16;
    match v {
        92..=139 => {
            if left {
                2
            } else {
                41
            }
        }
        50..=91 => {
            if left {
                3
            } else {
                42
            }
        }
        _ => 0,
    }
}

/// Writes the inputs into `dir` (the same bytes every time).
pub fn write_inputs(dir: &Path) {
    let mut g = Globals::new(UpstreamDefects::Reproduce);
    let env = Environment::default();
    let mut vin = volume(&mut g, MRI_UCHAR, intensity);
    let mut vnan = volume(&mut g, MRI_FLOAT, |x, y, z| {
        match (x * 5 + y * 3 + z * 11) % 97 {
            0 => 0x7fc0_0000,
            1 => 0xffc0_0001,
            2 => 0x7f80_0001,
            3 => 0x7f80_0000,
            4 => 0xff80_0000,
            _ => (intensity(x, y, z) as f32 + 0.25).to_bits(),
        }
    });
    let mut aseg = volume(&mut g, MRI_INT, aseg_value);
    let mut keep = volume(&mut g, MRI_UCHAR, |x, y, z| {
        if (10..14).contains(&x) && (10..14).contains(&y) && (10..14).contains(&z) {
            255
        } else if (17..20).contains(&x) && (17..20).contains(&y) && (17..20).contains(&z) {
            1
        } else {
            0
        }
    });
    let (mut o, mut e) = io();
    std::fs::create_dir_all(dir).expect("a folder");
    for (m, name) in [
        (&mut vin, "in.mgz"),
        (&mut vnan, "nan.mgz"),
        (&mut aseg, "aseg.presurf.mgz"),
        (&mut keep, "keep.mgz"),
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

/// A written file's digest: a volume's (`.mgz`), a text file's bytes, or `absent` for a file the
/// run did not write.
pub fn digest(path: &Path) -> String {
    if !path.exists() {
        return "absent".to_owned();
    }
    if path.extension().is_some_and(|e| e == "mgz") {
        return volume_digest(path);
    }
    let mut h = Sha256::new();
    h.update(std::fs::read(path).expect("a file"));
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// The two streams as the golden hash takes them: standard output without the run's time, then
/// standard error without larmorx's notices.
pub fn mask(out: &str, err: &str) -> String {
    let mut t = String::new();
    for l in out.split_inclusive('\n') {
        if l.starts_with("white matter segmentation took ") {
            t.push_str("white matter segmentation took <time> minutes\n");
        } else {
            t.push_str(l);
        }
    }
    t.push_str("--\n");
    for l in err.split_inclusive('\n') {
        if !l.starts_with("larmorx: ") {
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
    let status =
        larmorx_freesurfer_volumes::mri_segment::main::run(&args, &settings, &mut out, &mut err);
    let text = mask(
        &String::from_utf8_lossy(&out),
        &String::from_utf8_lossy(&err),
    );
    (status, text)
}
