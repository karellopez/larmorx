// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! `mri_make_uchar`'s golden cases: 32^3 volumes made with integer arithmetic only (the same
//! bytes on every platform; NaNs and infinities as bit patterns) on a coronal 4 mm grid centred
//! on the origin, an 8^3 axial one, and an MNI transform as text, written by the port; the
//! program run on them, and each output's digest (a volume's extent, type, voxel sizes,
//! direction cosines, centre and voxels; a text file's bytes). Shared by the test
//! `mri_make_uchar_golden` and the example `fs_make_uchar_golden`, with which
//! `validation/probes/fs-make-uchar/prog.py --golden` runs the official program on the same
//! inputs.
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

/// The command lines: `mri_nu_correct.mni`'s call on a `float` volume; the histogram and ball
/// files with a smaller ball; a `uchar` volume; an empty ball (no voxel inside: the CDF of zero
/// counts, a NaN mapping, NaNs into `uchar` voxels); a volume with NaNs and infinities; an axial
/// volume (an error per voxel, `errno`'s message first, FS-1130); no output name (FS-1127).
pub const CASES: [&[&str]; 7] = [
    &["vf.mgz", "tal.xfm", "out.mgz"],
    &[
        "-h", "h.txt", "-v", "rad.mgz", "-r", "30", "vf.mgz", "tal.xfm", "out.mgz",
    ],
    &["vu.mgz", "tal.xfm", "out.mgz"],
    &["-r", "0", "-h", "h.txt", "vf.mgz", "tal.xfm", "out.mgz"],
    &["vn.mgz", "tal.xfm", "out.mgz"],
    &["vax.mgz", "tal.xfm", "out.mgz"],
    &["vf.mgz", "tal.xfm"],
];

/// The files each case writes (compared by digest).
pub const OUTPUTS: [&[&str]; 7] = [
    &["out.mgz"],
    &["h.txt", "rad.mgz", "out.mgz"],
    &["out.mgz"],
    &["h.txt", "out.mgz"],
    &["out.mgz"],
    &["out.mgz"],
    &[],
];

/// The transform (`tal.xfm`): a talairach.xfm's numbers as text.
const XFM: &str = "MNI Transform File\n%golden\n\nTransform_Type = Linear;\nLinear_Transform =\n   0.98976505    0.00706433    0.02478982    2.90567017\n  -0.04069944    1.02083659    0.45262712   15.16511536\n  -0.00279743   -0.42630810    1.08509552    2.64450073 ;\n";

fn io() -> (std::io::Sink, std::io::Sink) {
    (std::io::sink(), std::io::sink())
}

/// A `width^3` volume of `mtype` whose voxel `(x, y, z)` holds the bits `f(x, y, z)`, with voxels
/// of `size` mm and the direction cosines `dirs` (columns x, y, z), centred on the origin.
fn volume(
    g: &mut Globals,
    width: i32,
    mtype: i32,
    size: f32,
    dirs: [[f32; 3]; 3],
    f: impl Fn(i32, i32, i32) -> u32,
) -> Mri {
    let (mut o, mut e) = io();
    let mut m = mri_alloc(g, &mut o, &mut e, width, width, width, mtype).expect("a volume");
    m.vg.xsize = size;
    m.vg.ysize = size;
    m.vg.zsize = size;
    [m.vg.x_r, m.vg.x_a, m.vg.x_s] = dirs[0];
    [m.vg.y_r, m.vg.y_a, m.vg.y_s] = dirs[1];
    [m.vg.z_r, m.vg.z_a, m.vg.z_s] = dirs[2];
    let bytes = m.bytes_per_vox;
    let c = m.chunk.as_mut().expect("voxels");
    for z in 0..width {
        for y in 0..width {
            for x in 0..width {
                let i = ((z * width + y) * width + x) as usize * bytes;
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

/// Coronal (LIA) direction cosines.
const LIA: [[f32; 3]; 3] = [[-1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0]];
/// Axial (RAS) direction cosines.
const RAS: [[f32; 3]; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

/// An intensity as integer arithmetic makes it: 0 at some voxels, else `40 + k / 2` (exact).
fn intensity(x: i32, y: i32, z: i32) -> f32 {
    let k = (x * 7 + y * 13 + z * 29) % 211;
    if k % 9 == 0 {
        0.0
    } else {
        40.0 + k as f32 / 2.0
    }
}

/// Writes `vf.mgz` (`float`), `vu.mgz` (`uchar`, the same values cut to 255), `vn.mgz` (`float`
/// with NaNs, infinities and a negative value at some voxels), `vax.mgz` (8^3, axial, 16 mm) and
/// `tal.xfm` into `dir`.
pub fn write_inputs(dir: &Path) {
    let mut g = Globals::new(UpstreamDefects::Reproduce);
    let env = Environment::default();
    let mut vf = volume(&mut g, N, MRI_FLOAT, 4.0, LIA, |x, y, z| {
        intensity(x, y, z).to_bits()
    });
    let mut vu = volume(&mut g, N, MRI_UCHAR, 4.0, LIA, |x, y, z| {
        intensity(x, y, z).min(255.0) as u32
    });
    let mut vn = volume(&mut g, N, MRI_FLOAT, 4.0, LIA, |x, y, z| {
        match (x * 5 + y * 3 + z * 11) % 61 {
            0 => 0x7fc0_0000,
            1 => 0xffc0_0001,
            2 => 0x7f80_0000,
            3 => 0xff80_0000,
            4 => (-30.0f32).to_bits(),
            _ => intensity(x, y, z).to_bits(),
        }
    });
    let mut vax = volume(&mut g, 8, MRI_FLOAT, 16.0, RAS, |x, y, z| {
        intensity(x, y, z).to_bits()
    });
    let (mut o, mut e) = io();
    for (m, name) in [
        (&mut vf, "vf.mgz"),
        (&mut vu, "vu.mgz"),
        (&mut vn, "vn.mgz"),
        (&mut vax, "vax.mgz"),
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
    std::fs::write(dir.join("tal.xfm"), XFM).expect("written");
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

/// A written file's digest by its kind: a volume (`.mgz`), else its bytes; `absent` for a file
/// the run did not write.
pub fn digest(path: &Path) -> String {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if !path.exists() {
        "absent".to_owned()
    } else if name.ends_with(".mgz") {
        volume_digest(path)
    } else {
        let b = std::fs::read(path).expect("a file");
        Sha256::digest(&b)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }
}

/// The figures of the timing line `type change took <m> minutes and <s> seconds.` masked.
fn mask_took(l: &str) -> String {
    match (l.find(" took "), l.find(" seconds.")) {
        (Some(a), Some(b)) if a < b => format!("{} took <m> minutes and <s>{}", &l[..a], &l[b..]),
        _ => l.to_owned(),
    }
}

/// The two streams as the golden hash takes them: standard output, then standard error without
/// larmorx's notices and with the timing line's figures masked.
pub fn mask(out: &str, err: &str) -> String {
    let mut t = String::from(out);
    t.push_str("--\n");
    for l in err.split_inclusive('\n') {
        if !l.starts_with("larmorx: ") {
            t.push_str(&mask_took(l));
        }
    }
    t
}

/// Runs case `k` in `dir` (the inputs there, the earlier cases' outputs removed) with `mode`: the
/// exit status and the two streams (masked by [`mask`]).
pub fn run_case(dir: &Path, k: usize, mode: UpstreamDefects) -> (u8, String) {
    for name in ["out.mgz", "rad.mgz", "h.txt"] {
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
        larmorx_freesurfer_volumes::mri_make_uchar::main::run(&args, &settings, &mut out, &mut err);
    let text = mask(
        &String::from_utf8_lossy(&out),
        &String::from_utf8_lossy(&err),
    );
    (status, text)
}
