// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_mask`'s golden cases: 32^3 volumes made with integer arithmetic only (the same bytes on
//! every platform; NaNs and infinities as bit patterns) on a coronal 2 mm grid centred on the
//! origin, a 16 x 16 x 32 pair, and a linear transform array as text, written by the port; the
//! program run on them, and each output's digest (a volume's extent, type, voxel sizes, direction
//! cosines, centre and voxels). Shared by the test `mri_mask_golden` and the example
//! `fs_mask_golden`, with which `validation/probes/fs-mask/prog.py --golden` runs the official
//! program on the same inputs.
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

/// The command lines: recon-all's threshold (`-T 5`); the inverted mask; the transform with a
/// threshold and a dilation; a transfer value with the mask's deletion edits kept; the bounding
/// box and its square form; a crop to a field of view; a mask with NaNs and infinities (their
/// voxels, `-oval nan`); a threshold of NaN; the left hemisphere of a segmentation without the
/// cerebellum and samseg's labels; erosion with background noise; `-BBEQ` that finds no box
/// (FS-1133: a crash, an error by default).
pub const CASES: [&[&str]; 13] = [
    &["-T", "5", "vf.mgz", "mk.mgz", "out.mgz"],
    &["-invert", "vf.mgz", "mk.mgz", "out.mgz"],
    &[
        "-xform", "v2v.lta", "-T", "3", "-dilate", "1", "vf.mgz", "mk.mgz", "out.mgz",
    ],
    &[
        "-transfer",
        "2",
        "-keep_mask_deletion_edits",
        "vf.mgz",
        "m012.mgz",
        "out.mgz",
    ],
    &["-BB", "2", "vf.mgz", "mk.mgz", "out.mgz"],
    &["-BBEQ", "1", "vf.mgz", "mk.mgz", "out.mgz"],
    &[
        "-crop-to-fov",
        "16",
        "18",
        "10",
        "-T",
        "2",
        "vf.mgz",
        "mk.mgz",
        "out.mgz",
    ],
    &["-oval", "nan", "vf.mgz", "mn.mgz", "out.mgz"],
    &["-T", "nan", "vf.mgz", "mn.mgz", "out.mgz"],
    &["-lh", "-no_cerebellum", "vf.mgz", "seg.mgz", "out.mgz"],
    &["-samseg", "vf.mgz", "seg.mgz", "out.mgz"],
    &[
        "-erode", "1", "-bgnoise", "5", "1", "vf.mgz", "mk.mgz", "out.mgz",
    ],
    &["-BBEQ", "1", "nz.mgz", "nzm.mgz", "out.mgz"],
];

/// The files each case writes (compared by digest).
pub const OUTPUTS: [&[&str]; 13] = [
    &["out.mgz"],
    &["out.mgz"],
    &["out.mgz"],
    &["out.mgz"],
    &["out.mgz"],
    &["out.mgz"],
    &["out.mgz"],
    &["out.mgz"],
    &["out.mgz"],
    &["out.mgz"],
    &["out.mgz"],
    &["out.mgz"],
    &["out.mgz"],
];

/// The transform (`v2v.lta`): one voxel along x, voxel to voxel, between two volumes of the
/// coronal grid.
const LTA: &str = "type      = 0 # LINEAR_VOX_TO_VOX\nnxforms   = 1\nmean      = 0.0000 0.0000 0.0000\nsigma     = 1.0000\n1 4 4\n1 0 0 1\n0 1 0 0\n0 0 1 0\n0 0 0 1\nsrc volume info\nvalid = 1  # volume info valid\nfilename = mk.mgz\nvolume = 32 32 32\nvoxelsize = 2.0 2.0 2.0\nxras   = -1.0 0.0 0.0\nyras   = 0.0 0.0 -1.0\nzras   = 0.0 1.0 0.0\ncras   = 0.0 0.0 0.0\ndst volume info\nvalid = 1  # volume info valid\nfilename = vf.mgz\nvolume = 32 32 32\nvoxelsize = 2.0 2.0 2.0\nxras   = -1.0 0.0 0.0\nyras   = 0.0 0.0 -1.0\nzras   = 0.0 1.0 0.0\ncras   = 0.0 0.0 0.0\n";

fn io() -> (std::io::Sink, std::io::Sink) {
    (std::io::sink(), std::io::sink())
}

/// A `w x h x d` volume of `mtype` whose voxel `(x, y, z)` holds the bits `f(x, y, z)`, with
/// coronal voxels of 2 mm centred on the origin.
fn volume(g: &mut Globals, dims: [i32; 3], mtype: i32, f: impl Fn(i32, i32, i32) -> u32) -> Mri {
    let (mut o, mut e) = io();
    let [w, h, d] = dims;
    let mut m = mri_alloc(g, &mut o, &mut e, w, h, d, mtype).expect("a volume");
    m.vg.xsize = 2.0;
    m.vg.ysize = 2.0;
    m.vg.zsize = 2.0;
    [m.vg.x_r, m.vg.x_a, m.vg.x_s] = [-1.0, 0.0, 0.0];
    [m.vg.y_r, m.vg.y_a, m.vg.y_s] = [0.0, 0.0, -1.0];
    [m.vg.z_r, m.vg.z_a, m.vg.z_s] = [0.0, 1.0, 0.0];
    let bytes = m.bytes_per_vox;
    let c = m.chunk.as_mut().expect("voxels");
    for z in 0..d {
        for y in 0..h {
            for x in 0..w {
                let i = ((z * h + y) * w + x) as usize * bytes;
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

/// An intensity as integer arithmetic makes it: 0 at some voxels, else `40 + k / 2` (exact).
fn intensity(x: i32, y: i32, z: i32) -> f32 {
    let k = (x * 7 + y * 13 + z * 29) % 211;
    if k % 9 == 0 {
        0.0
    } else {
        40.0 + k as f32 / 2.0
    }
}

/// The mask's value: 0 outside a box, `k / 4` (0 to 10, exact) inside.
fn mask_value(x: i32, y: i32, z: i32) -> f32 {
    if (8..24).contains(&x) && (6..26).contains(&y) && (10..22).contains(&z) {
        ((x * 3 + y * 5 + z * 11) % 41) as f32 / 4.0
    } else {
        0.0
    }
}

/// A segmentation's labels: lh and rh cortex and white matter, the cerebellum, samseg's
/// non-brain labels, unknown.
const LABELS: [u32; 12] = [0, 2, 3, 41, 42, 8, 47, 24, 165, 130, 258, 17];

/// Writes `vf.mgz` (`float`), `mk.mgz` (`float` mask), `mn.mgz` (the mask with NaNs, infinities
/// and negative values at some voxels), `m012.mgz` (`uchar` 0, 1, 2), `seg.mgz` (`int` labels),
/// `nz.mgz` and `nzm.mgz` (16 x 16 x 32, a mask of ones: no `-BBEQ` box) and `v2v.lta` into
/// `dir`.
pub fn write_inputs(dir: &Path) {
    let mut g = Globals::new(UpstreamDefects::Reproduce);
    let env = Environment::default();
    let cube = [N, N, N];
    let mut vf = volume(&mut g, cube, MRI_FLOAT, |x, y, z| {
        intensity(x, y, z).to_bits()
    });
    let mut mk = volume(&mut g, cube, MRI_FLOAT, |x, y, z| {
        mask_value(x, y, z).to_bits()
    });
    let mut mn = volume(&mut g, cube, MRI_FLOAT, |x, y, z| {
        match (x * 5 + y * 3 + z * 11) % 53 {
            0 => 0x7fc0_0000,
            1 => 0xffc0_0001,
            2 => 0x7f80_0000,
            3 => 0xff80_0000,
            4 => (-3.0f32).to_bits(),
            _ => mask_value(x, y, z).to_bits(),
        }
    });
    let mut m012 = volume(&mut g, cube, MRI_UCHAR, |x, y, z| {
        if (6..26).contains(&x) && (6..26).contains(&y) && (6..26).contains(&z) {
            ((x + 2 * y + 3 * z) % 3) as u32
        } else {
            0
        }
    });
    let mut seg = volume(&mut g, cube, MRI_INT, |x, y, z| {
        LABELS[((x * 3 + y * 7 + z * 13) % 12) as usize]
    });
    let tall = [16, 16, 32];
    let mut nz = volume(&mut g, tall, MRI_FLOAT, |x, y, z| {
        intensity(x, y, z).to_bits()
    });
    let mut nzm = volume(&mut g, tall, MRI_UCHAR, |_, _, _| 1);
    let (mut o, mut e) = io();
    for (m, name) in [
        (&mut vf, "vf.mgz"),
        (&mut mk, "mk.mgz"),
        (&mut mn, "mn.mgz"),
        (&mut m012, "m012.mgz"),
        (&mut seg, "seg.mgz"),
        (&mut nz, "nz.mgz"),
        (&mut nzm, "nzm.mgz"),
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
    std::fs::write(dir.join("v2v.lta"), LTA).expect("written");
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

/// A written file's digest: a volume's, or `absent` for a file the run did not write.
pub fn digest(path: &Path) -> String {
    if path.exists() {
        volume_digest(path)
    } else {
        "absent".to_owned()
    }
}

/// The two streams as the golden hash takes them: standard output, then standard error without
/// larmorx's notices.
pub fn mask(out: &str, err: &str) -> String {
    let mut t = String::from(out);
    t.push_str("--\n");
    for l in err.split_inclusive('\n') {
        if !l.starts_with("larmorx: ") {
            t.push_str(l);
        }
    }
    t
}

/// Runs case `k` in `dir` (the inputs there, the earlier case's output removed) with `mode`: the
/// exit status and the two streams (masked by [`mask`]).
pub fn run_case(dir: &Path, k: usize, mode: UpstreamDefects) -> (u8, String) {
    let _ = std::fs::remove_file(dir.join("out.mgz"));
    let args: Vec<String> = CASES[k].iter().map(|s| (*s).to_owned()).collect();
    let settings = Settings {
        threads: 1,
        mode,
        env: Environment::from_pairs([("OMP_NUM_THREADS", "1")]),
        dir: Some(dir.to_path_buf()),
    };
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let status =
        larmorx_freesurfer_volumes::mri_mask::main::run(&args, &settings, &mut out, &mut err);
    let text = mask(
        &String::from_utf8_lossy(&out),
        &String::from_utf8_lossy(&err),
    );
    (status, text)
}
