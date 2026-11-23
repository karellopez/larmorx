// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_cc`'s golden cases: subjects of 32^3 volumes made with integer arithmetic only (the same
//! bytes on every platform; NaNs and infinities as bit patterns) on a coronal 1 mm grid centred on
//! the origin, and a Talairach transform as text, written by the port. The white-matter path's
//! `mri/wm` is a single-file NIfTI without a suffix (FreeSurfer finds its format from its
//! content). The program run on them, and each output's digest (a volume's extent, type, voxel
//! sizes, direction cosines, centre and voxels). Shared by the test `mri_cc_golden` and the
//! example `fs_cc_golden`, with which `validation/probes/fs-cc/prog.py --golden` runs the official
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

/// The subjects: `s` (white matter, aseg, norm, transform), `n` (a `float` white matter with NaNs
/// and infinities, transform), `w` (the transform only), `x` (the white matter only), `c` (an
/// aseg that holds the callosum's central label, norm).
pub const SUBJECTS: [&str; 5] = ["s", "n", "w", "x", "c"];

/// The command lines: the white-matter path (`-a 0`; on 32^3 its seed search finds no column:
/// FS-1210), with other divisions and thickness, with NaNs kept ([`case_env`]); without `mri/wm` (FS-1205: a crash, an
/// error by default) and without the transform; more than 50 divisions (FS-1211: a crash); the
/// aseg path (`-aseg`; its rotation search within 1 degree, `-m 1`, for the test's time), on an aseg that holds the callosum already (status 77), with `-lh` (the
/// aseg copied); a name of 1,024 bytes (FS-1212: an abort); no subjects folder;
/// a norm that does not read.
pub fn cases() -> Vec<Vec<String>> {
    let long = "a".repeat(1024);
    let c: [&[&str]; 12] = [
        &["-sdir", ".", "-a", "0", "s"],
        &["-sdir", ".", "-a", "0", "-d", "3", "-t", "1", "s"],
        &["-sdir", ".", "-a", "0", "n"],
        &["-sdir", ".", "-a", "0", "w"],
        &["-sdir", ".", "-a", "0", "x"],
        &["-sdir", ".", "-d", "51", "s"],
        &[
            "-sdir", ".", "-aseg", "aseg.mgz", "-o", "out.mgz", "-m", "1", "s",
        ],
        &["-sdir", ".", "-aseg", "aseg.mgz", "-o", "out.mgz", "c"],
        &[
            "-sdir", ".", "-aseg", "aseg.mgz", "-o", "out.mgz", "-lh", "s",
        ],
        &["-sdir", ".", "-o", "LONG", "c"],
        &["-a", "0", "s"],
        &["-sdir", ".", "-aseg", "aseg.mgz", "-norm", "no.mgz", "s"],
    ];
    c.iter()
        .map(|a| {
            a.iter()
                .map(|w| {
                    if *w == "LONG" {
                        long.clone()
                    } else {
                        (*w).to_owned()
                    }
                })
                .collect()
        })
        .collect()
}

/// The environment a case adds: case 2 keeps the NaNs FreeSurfer's reader would set to 0
/// (`FS_LEAVE_NANS`), so that they reach the resamplings and the binarizations.
pub fn case_env(k: usize) -> &'static [(&'static str, &'static str)] {
    if k == 2 {
        &[("FS_LEAVE_NANS", "1")]
    } else {
        &[]
    }
}

/// The files each case may write (compared by digest; removed before each case).
pub const OUTPUTS: [&[&str]; 12] = [
    &["s/mri/cc_org.mgz"],
    &["s/mri/cc_org.mgz"],
    &["n/mri/cc_org.mgz"],
    &["w/mri/cc_org.mgz"],
    &["x/mri/cc_org.mgz"],
    &["s/mri/cc_org.mgz"],
    &["s/mri/out.mgz"],
    &["c/mri/out.mgz"],
    &["s/mri/out.mgz"],
    &[],
    &["s/mri/cc_org.mgz"],
    &["s/mri/aseg_with_cc.mgz"],
];

/// The Talairach transform (`talairach.xfm`): a small rotation, scaling and shift.
const XFM: &str = "MNI Transform File\n% golden\n\nTransform_Type = Linear;\nLinear_Transform = \n1.0625 0.03125 -0.015625 2.5\n-0.046875 1.015625 0.4375 -6.25\n0.0078125 -0.40625 1.1875 3.75;\n";

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

/// The white matter: 110 in a slab with two hemispheres' blocks and a bridge between them, the
/// fill labels (200 to 240) at some voxels, 0 elsewhere.
fn wm_value(x: i32, y: i32, z: i32) -> u32 {
    let inside = (4..28).contains(&y) && (6..26).contains(&z);
    let hemi = (4..14).contains(&x) || (18..28).contains(&x);
    let bridge = (14..18).contains(&x) && (10..14).contains(&y) && (8..24).contains(&z);
    if !(inside && hemi || bridge) {
        return 0;
    }
    match (x * 7 + y * 11 + z * 13) % 37 {
        0 => 200,
        1 => 210,
        2 => 220,
        3 => 230,
        4 => 240,
        5 => 250,
        _ => 110,
    }
}

/// The aseg's labels: left white matter and cortex (2, 3) for x below 16, right (41, 42) above,
/// the cortex on the slab's outer shell.
fn aseg_value(x: i32, y: i32, z: i32) -> u32 {
    let inside = (4..28).contains(&y) && (6..26).contains(&z) && (3..29).contains(&x);
    if !inside {
        return 0;
    }
    let shell = y == 4 || y == 27 || z == 6 || z == 25 || x == 3 || x == 28;
    match (x < 16, shell) {
        (true, false) => 2,
        (true, true) => 3,
        (false, false) => 41,
        (false, true) => 42,
    }
}

/// Writes the subjects into `dir`: `<subject>/mri/...` and `<subject>/mri/transforms/
/// talairach.xfm`.
pub fn write_inputs(dir: &Path) {
    let mut g = Globals::new(UpstreamDefects::Reproduce);
    let env = Environment::default();
    let mut wm = volume(&mut g, MRI_UCHAR, wm_value);
    let mut wmf = volume(&mut g, MRI_FLOAT, |x, y, z| {
        match (x * 5 + y * 3 + z * 11) % 41 {
            0 => 0x7fc0_0000,
            1 => 0xffc0_0001,
            2 => 0x7f80_0000,
            3 => 0xff80_0000,
            _ => (wm_value(x, y, z) as f32 + 0.25).to_bits(),
        }
    });
    let mut aseg = volume(&mut g, MRI_INT, aseg_value);
    let mut aseg_cc = volume(&mut g, MRI_INT, |x, y, z| {
        if (x, y, z) == (16, 12, 16) {
            253
        } else {
            aseg_value(x, y, z)
        }
    });
    let mut norm = volume(&mut g, MRI_UCHAR, |x, y, z| {
        if aseg_value(x, y, z) == 0 {
            0
        } else {
            (60 + (x * 3 + y * 5 + z * 7) % 61) as u32
        }
    });
    let (mut o, mut e) = io();
    let mut put = |g: &mut Globals, m: &mut Mri, subject: &str, name: &str| {
        let mri = dir.join(subject).join("mri");
        std::fs::create_dir_all(mri.join("transforms")).expect("a folder");
        let p = mri.join(name);
        mri_write(
            g,
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
    };
    for s in SUBJECTS {
        std::fs::create_dir_all(dir.join(s).join("mri").join("transforms")).expect("a folder");
    }
    put(&mut g, &mut wm, "s", "wm.nii");
    put(&mut g, &mut aseg, "s", "aseg.mgz");
    put(&mut g, &mut norm, "s", "norm.mgz");
    put(&mut g, &mut wmf, "n", "wm.nii");
    put(&mut g, &mut wm, "x", "wm.nii");
    put(&mut g, &mut aseg_cc, "c", "aseg.mgz");
    put(&mut g, &mut norm, "c", "norm.mgz");
    for s in ["s", "n", "x"] {
        let mri = dir.join(s).join("mri");
        std::fs::rename(mri.join("wm.nii"), mri.join("wm")).expect("renamed");
    }
    for s in ["s", "n", "w"] {
        let t = dir
            .join(s)
            .join("mri")
            .join("transforms")
            .join("talairach.xfm");
        std::fs::write(t, XFM).expect("written");
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

/// A written file's digest: a volume's, or `absent` for a file the run did not write.
pub fn digest(path: &Path) -> String {
    if path.exists() {
        volume_digest(path)
    } else {
        "absent".to_owned()
    }
}

/// The two streams as the golden hash takes them: standard output without the run's time and peak
/// memory, then standard error without larmorx's notices; each carriage return a new line (as
/// Python's text streams read the official program's, `\r` from the rotation search's lines).
pub fn mask(out: &str, err: &str) -> String {
    let lines = |s: &str| s.replace("\r\n", "\n").replace('\r', "\n");
    let (out, err) = (lines(out), lines(err));
    let mut t = String::new();
    for l in out.split_inclusive('\n') {
        if l.starts_with("corpus callosum segmentation took ") {
            t.push_str("corpus callosum segmentation took <time> minutes\n");
        } else if l.starts_with("#VMPC# mri_cc VmPeak") {
            t.push_str("#VMPC# mri_cc VmPeak <peak>\n");
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

/// Runs case `k` in `dir` (the inputs there, every case's outputs removed first) with `mode`: the
/// exit status and the two streams (masked by [`mask`]).
pub fn run_case(dir: &Path, k: usize, mode: UpstreamDefects) -> (u8, String) {
    for outs in OUTPUTS {
        for name in outs {
            let _ = std::fs::remove_file(dir.join(name));
        }
    }
    let args = cases().swap_remove(k);
    let settings = Settings {
        threads: 1,
        mode,
        env: Environment::from_pairs(
            [("OMP_NUM_THREADS", "1")]
                .into_iter()
                .chain(case_env(k).iter().copied()),
        ),
        dir: Some(dir.to_path_buf()),
    };
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let status =
        larmorx_freesurfer_volumes::mri_cc::main::run(&args, &settings, &mut out, &mut err);
    let text = mask(
        &String::from_utf8_lossy(&out),
        &String::from_utf8_lossy(&err),
    );
    (status, text)
}
