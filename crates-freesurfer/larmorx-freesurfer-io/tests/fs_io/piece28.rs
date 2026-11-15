// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Piece 28 (track A relay 36): `MRIread` and `MRIreadEx`. `mrr` (`MRIread(name, NULL)`, now and
//! then twice in one process, so the second read meets `MRIremoveNaNs`' once-per-process line
//! already printed) and `mrx` (`MRIreadEx(name, nthframe)`, frames -1, 0, 1, 2 and 5) of small
//! MGH volumes (`.mgh`, `.mgz`) of bytes, 32-bit integers, shorts and floats, the floats with no,
//! one or several non-finite voxels (quiet and signalling NaNs with payloads, both infinities, in
//! one column of x or in several) among ordinary, huge and subnormal values; `FS_LEAVE_NANS` set
//! or not; names given whole, with a `*` that matches one file or two (the first in `glob`'s
//! order is read), with a `*` that matches none, or with a `#` frame list (`mri_read`'s frame
//! check then refuses a volume with a non-finite voxel); now and then a missing file.

use std::io::Write;

use flate2::Compression;
use flate2::write::GzEncoder;
use larmorx_freesurfer_core::globals::Globals;

use super::piece4::file_entry;
use super::{Gen, Out, Work, enc};

pub const KINDS: [&str; 2] = ["mrr", "mrx"];

fn be32(d: &mut Vec<u8>, v: i32) {
    d.extend_from_slice(&v.to_be_bytes());
}

fn bef(d: &mut Vec<u8>, v: f32) {
    d.extend_from_slice(&v.to_be_bytes());
}

/// An ordinary float voxel.
fn ordinary(g: &mut Gen) -> f32 {
    match g.below(16) {
        0 => f32::MAX,
        1 => f32::from_bits(1),
        2 => -0.0,
        _ => g.int(-4000, 4000) as f32 / 16.0,
    }
}

/// A non-finite float voxel.
fn non_finite(g: &mut Gen) -> f32 {
    f32::from_bits(g.pick(&[
        0x7fc0_0000,
        0x7fc0_0001,
        0xffc0_0000,
        0x7fa0_0001,
        0xff80_0003,
        0x7f80_0000,
        0xff80_0000,
    ]))
}

/// An MGH file: version 1, the extent, frames, type, dof 1, ras_good 1 with 1 mm voxels and the
/// coronal cosines, the header filled to 284 bytes, the voxels (x fastest) big-endian.
fn mgh(g: &mut Gen) -> Vec<u8> {
    let (w, h, d) = (g.int(1, 4) as i32, g.int(1, 3) as i32, g.int(1, 3) as i32);
    let nf = g.pick(&[1, 1, 2, 3]);
    let ty = g.pick(&[3, 3, 3, 3, 0, 1, 4]);
    let mut b = Vec::new();
    for v in [1, w, h, d, nf, ty, 1] {
        be32(&mut b, v);
    }
    b.extend_from_slice(&1i16.to_be_bytes());
    for v in [
        1.0, 1.0, 1.0, -1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0,
    ] {
        bef(&mut b, v);
    }
    b.resize(284, 0);
    let n = (w * h * d * nf) as usize;
    // the non-finite voxels: none, one, or a few in one column of x or anywhere
    let mut bad = vec![false; n];
    if ty == 3 {
        match g.below(5) {
            0 => {}
            1 => bad[g.below(n as u64) as usize] = true,
            2 => {
                let x = g.int(0, i64::from(w) - 1) as usize;
                for (i, v) in bad.iter_mut().enumerate() {
                    if i % w as usize == x && g.chance(2) {
                        *v = true;
                    }
                }
            }
            _ => {
                for v in bad.iter_mut() {
                    *v = g.chance(3);
                }
            }
        }
    }
    for flag in bad {
        match ty {
            3 => {
                let v = if flag { non_finite(g) } else { ordinary(g) };
                bef(&mut b, v);
            }
            0 => b.push(g.int(0, 255) as u8),
            1 => be32(&mut b, g.int(-70000, 70000) as i32),
            _ => b.extend_from_slice(&(g.int(-3000, 3000) as i16).to_be_bytes()),
        }
    }
    b
}

fn gz(b: &[u8]) -> Vec<u8> {
    let mut e = GzEncoder::new(Vec::new(), Compression::new(6));
    e.write_all(b).unwrap();
    e.finish().unwrap()
}

pub fn generate(g: &mut Gen, kind: &str, _portable: bool) -> Vec<String> {
    let mut t = vec![kind.to_owned(), "0".into(), "1".into(), "0".into()];
    t.push(if g.chance(3) { "1" } else { "0" }.to_owned());
    if kind == "mrx" {
        t.push(g.pick(&[-1, -1, 0, 1, 2, 5]).to_string());
    } else {
        t.push(if g.chance(4) { "1" } else { "0" }.to_owned());
    }
    let ext = g.pick(&[".mgh", ".mgz"]);
    let raw = mgh(g);
    let bytes = if ext == ".mgz" { gz(&raw) } else { raw };
    let mut entries = Vec::new();
    let name = match g.below(10) {
        0 => {
            // two files a * matches: glob's first (ab) is read
            entries.push(file_entry(&format!("w/ab{ext}"), &bytes));
            let other = mgh(g);
            entries.push(file_entry(&format!("w/ac{ext}"), &other));
            format!("w/a*{ext}")
        }
        1 => {
            entries.push(file_entry(&format!("w/ab{ext}"), &bytes));
            format!("w/*b{ext}")
        }
        2 => {
            entries.push(file_entry(&format!("w/ab{ext}"), &bytes));
            format!("w/q*{ext}")
        }
        3 => {
            entries.push(file_entry(&format!("w/a{ext}"), &bytes));
            format!("w/a{ext}{}", g.pick(&["#0", "#1", "#0:1"]))
        }
        4 if g.chance(2) => format!("w/nowhere{ext}"),
        _ => {
            entries.push(file_entry(&format!("w/a{ext}"), &bytes));
            format!("w/a{ext}")
        }
    };
    t.push(enc(Some(name.as_bytes())));
    t.push(if entries.is_empty() {
        "-".to_owned()
    } else {
        entries.join(",")
    });
    t
}

pub fn run(
    tk: &[&str],
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    o: &mut Out,
    w: &Work,
    extra: &mut Vec<&'static str>,
) -> Result<(), u8> {
    super::piece6::run_env(tk, g, out, err, o, w, extra, &[])
}
