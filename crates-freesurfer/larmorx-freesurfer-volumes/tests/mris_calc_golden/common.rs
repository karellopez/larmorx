// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mris_calc`'s golden cases: 32^3 volumes and curvature files made with integer arithmetic only
//! (the same bytes on every platform; NaNs, infinities, zeros and denormals as bit patterns),
//! written by the port, the program run on them, and each output's digest (a volume's extent,
//! type, voxel sizes, direction cosines, centre and voxels; a curvature or text file's bytes).
//! Shared by the test `mris_calc_golden` and the example `fs_calc_golden`, with which
//! `validation/probes/fs-calc/prog.py --golden` runs the official program on the same inputs.
#![allow(dead_code)]

use std::path::Path;

use larmorx_core::defects::UpstreamDefects;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::run::{Environment, Settings};
use larmorx_freesurfer_io::mriio::{mri_read_volume, mri_write};
use larmorx_freesurfer_mri::mri::{MRI_FLOAT, MRI_UCHAR, Mri, mri_alloc};
use sha2::{Digest, Sha256};

/// The size of the volumes.
pub const N: i32 = 32;
/// The curvature files' values.
pub const NV: usize = 1000;

/// The command lines: `mri_nu_correct.mni`'s rescale of a volume; a square root of negative
/// values (SSE's default NaN in the output); a volume over another with zeros; two curvature
/// files with NaN operands added; `atan2`; `bcor` (`pow` and `log10`); the statistics; `norm` in a
/// label; the values as text; a bitwise `and` (NaN and infinities as integers).
pub const CASES: [&[&str]; 10] = [
    &["-o", "out.mgz", "vf.mgz", "mul", "1.50205015575164325309"],
    &["-o", "out.mgz", "vf.mgz", "sqrt"],
    &["-o", "out.mgz", "vf.mgz", "div", "vu.mgz"],
    &["-o", "out.crv", "lh.a", "add", "lh.b"],
    &["-o", "out.crv", "lh.a", "atan2", "lh.b"],
    &["-o", "out.crv", "lh.a", "bcor", "lh.b"],
    &["lh.a", "stats"],
    &["-l", "lab.label", "-o", "out.crv", "lh.a", "norm"],
    &["-o", "out.txt", "lh.a", "ascii"],
    &["-o", "out.crv", "lh.a", "andbw", "3"],
];

/// The files each case writes (compared by digest).
pub const OUTPUTS: [&[&str]; 10] = [
    &["out.mgz"],
    &["out.mgz"],
    &["out.mgz"],
    &["out.crv"],
    &["out.crv"],
    &["out.crv"],
    &[],
    &["out.crv"],
    &["out.txt"],
    &["out.crv"],
];

fn io() -> (std::io::Sink, std::io::Sink) {
    (std::io::sink(), std::io::sink())
}

fn volume(g: &mut Globals, mtype: i32, f: impl Fn(i32, i32, i32) -> u32) -> Mri {
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

/// A value as integer arithmetic makes it: `k / 4` (exact) for most, special bits at some.
fn value_bits(i: usize, salt: usize) -> u32 {
    match (i * 7 + salt) % 53 {
        0 => 0x7fa0_0000 | (i as u32 & 0xff),
        1 => 0xffc0_0000 | (i as u32 & 0xff),
        2 => 0x7f80_0000,
        3 => 0xff80_0000,
        4 => 1,
        5 => 0x8000_0000,
        6 => 0,
        _ => (((i * 31 + salt * 17) % 97) as f32 / 4.0 - 12.0).to_bits(),
    }
}

/// A curvature file of the new format with `NV` values.
fn curvature(path: &Path, salt: usize, nfaces: i32) {
    let mut b = vec![0xff, 0xff, 0xff];
    for v in [NV as i32, nfaces, 1] {
        b.extend_from_slice(&v.to_be_bytes());
    }
    for i in 0..NV {
        b.extend_from_slice(&value_bits(i, salt).to_be_bytes());
    }
    std::fs::write(path, b).expect("written");
}

/// Writes `vf.mgz` (float: `k / 8` from -20 / 8 to 20 / 8, zeros among them), `vu.mgz` (uchar,
/// 0 to 6), `lh.a`, `lh.b` (curvature files with NaNs, infinities, a denormal and zeros) and
/// `lab.label` (100 vertices) into `dir`.
pub fn write_inputs(dir: &Path) {
    let mut g = Globals::new(UpstreamDefects::Reproduce);
    let env = Environment::default();
    let mut vf = volume(&mut g, MRI_FLOAT, |x, y, z| {
        (((x * 7 + y * 13 + z * 29) % 41 - 20) as f32 / 8.0).to_bits()
    });
    let mut vu = volume(&mut g, MRI_UCHAR, |x, y, z| ((x * 3 + y * 5 + z * 11) % 7) as u32);
    let (mut o, mut e) = io();
    for (m, name) in [(&mut vf, "vf.mgz"), (&mut vu, "vu.mgz")] {
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
    curvature(&dir.join("lh.a"), 0, 1996);
    curvature(&dir.join("lh.b"), 5, 2004);
    let mut t = String::from("#!ascii label  , from subject golden vox2ras=TkReg\n100\n");
    for k in 0..100 {
        t.push_str(&format!("{}  {}.000  0.000  0.000 0.0000000000\n", k * 7 + 3, k));
    }
    std::fs::write(dir.join("lab.label"), t).expect("written");
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

/// A written file's digest by its kind: a volume (`.mgz`), else its bytes.
pub fn digest(path: &Path) -> String {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if name.ends_with(".mgz") {
        volume_digest(path)
    } else {
        let b = std::fs::read(path).expect("a file");
        Sha256::digest(&b)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }
}

/// The two streams as the golden hash takes them: standard output without the peak memory's
/// figure, then standard error without larmorx's notices.
pub fn mask(out: &str, err: &str) -> String {
    let mut t = String::new();
    for l in out.lines() {
        if l.starts_with("#VMPC# mris_calc VmPeak") {
            t.push_str("#VMPC# mris_calc VmPeak <kB>\n");
        } else {
            t.push_str(l);
            t.push('\n');
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

/// Runs case `k` in `dir` (the inputs there, the earlier cases' outputs removed) with `mode`: the
/// exit status and the two streams (masked by [`mask`]).
pub fn run_case(dir: &Path, k: usize, mode: UpstreamDefects) -> (u8, String) {
    for name in ["out.mgz", "out.crv", "out.txt"] {
        let _ = std::fs::remove_file(dir.join(name));
    }
    let args: Vec<String> = CASES[k].iter().map(|s| (*s).to_owned()).collect();
    let settings = Settings {
        threads: 1,
        mode,
        env: Environment::from_pairs([("OMP_NUM_THREADS", "1")]),
        dir: Some(dir.to_path_buf()),
    };
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let status =
        larmorx_freesurfer_volumes::mris_calc::main::run(&args, &settings, &mut out, &mut err);
    let text = mask(&String::from_utf8_lossy(&out), &String::from_utf8_lossy(&err));
    (status, text)
}
