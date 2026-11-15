// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Piece 26 (track A relay 34): MINC 1 volumes written. `mcw` (piece 17's `vwr`: `how` 11,
//! `MRIwriteType` as `MRI_MINC_FILE`, or 1, `MRIwrite` by a `.mnc` name) of volumes of every type
//! the writer takes (bytes, 16- and 32-bit integers, longs, floats) and others (unsigned 16-bit,
//! RGB, complex, tensor: refused), one frame or more (none now and then), with voxels or made
//! without; voxel sizes ordinary, odd, negative or 0; direction cosines coronal, a signed
//! permutation, oblique, not unit, of length 0 or holding a NaN; centres ordinary or far; RAS good
//! or not; voxel values constant, ordinary or at the types' edges (integers past 2^24, longs past 32
//! bits, floats NaN, infinite, subnormal, -0); onto `.mnc`, `.MNC`, `.mnc.gz` and bare names, over a
//! file, in a sub-folder, in a missing folder or in a folder's place; the file read back now and then
//! (`mri_read` of type 1 or by the name: piece 25's reader).

use std::io::Write;

use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_mri::mri;

use super::piece4::file_entry;
use super::piece12::fhex;
use super::{Gen, Out, Work, enc, hex};

pub const KINDS: [&str; 1] = ["mcw"];

const UCHAR: i64 = 0;
const INT: i64 = 1;
const LONG: i64 = 2;
const FLOAT: i64 = 3;
const SHORT: i64 = 4;

fn dir_entry(name: &str) -> String {
    format!("D{}", hex(name.as_bytes()))
}

/// The bytes of one voxel of type `ty` (little-endian), from the type's interesting values.
fn voxel(g: &mut Gen, ty: i64, wide: bool) -> Vec<u8> {
    match ty {
        UCHAR => {
            let any = g.below(256) as u8;
            vec![g.pick(&[0u8, 1, 127, 128, 255, any])]
        }
        SHORT => {
            let (any, small) = (g.next() as i16, g.int(-9, 9) as i16);
            let v: i16 = g.pick(&[0, 1, -1, 32767, -32768, any, small]);
            v.to_le_bytes().to_vec()
        }
        INT => {
            let any = g.next() as i32;
            let v: i32 = if wide {
                g.pick(&[
                    16_777_217,
                    -16_777_217,
                    16_777_219,
                    2_147_483_647,
                    -2_147_483_648,
                    123_456_789,
                    any,
                ])
            } else {
                g.int(-1000, 1000) as i32
            };
            v.to_le_bytes().to_vec()
        }
        LONG => {
            let any = g.next() as i64;
            let v: i64 = if wide {
                g.pick(&[
                    3_000_000_000,
                    -5_000_000_000,
                    i64::MAX,
                    i64::MIN,
                    2_147_483_648,
                    -2_147_483_649,
                    any,
                ])
            } else {
                g.int(-70_000, 70_000)
            };
            v.to_le_bytes().to_vec()
        }
        FLOAT => {
            let bits: u32 = match g.below(16) {
                0 => 0x7fc0_0000,                     // quiet NaN
                1 => 0x7fa0_0001,                     // signalling NaN
                2 => 0xffc0_0005,                     // negative quiet NaN
                3 => 0x7f80_0000,                     // +inf
                4 => 0xff80_0000,                     // -inf
                5 => 0x0000_0001,                     // the least subnormal
                6 => 0x8000_0000,                     // -0
                7 => 0x7f7f_ffff,                     // FLT_MAX
                8 => (g.next() as u32) & 0xbfff_ffff, // any finite
                _ => (g.int(-400, 400) as f32 / 8.0).to_bits(),
            };
            bits.to_le_bytes().to_vec()
        }
        _ => vec![g.below(256) as u8],
    }
}

fn size_of(ty: i64) -> usize {
    mri::mri_sizeof(ty as i32)
}

/// A volume in piece 14's fields (`read_mri14`): extent, type, frames, voxels or not, the geometry,
/// RAS good or not, the voxels' bytes; the rest plain.
fn volume(g: &mut Gen, t: &mut Vec<String>) -> (i64, bool) {
    let ty = match g.below(24) {
        0..=4 => FLOAT,
        5..=8 => UCHAR,
        9..=12 => SHORT,
        13..=16 => INT,
        17..=19 => LONG,
        20 => 10, // unsigned 16-bit
        21 => 9,  // RGB
        22 => 7,  // complex
        _ => 6,   // tensor
    };
    let pick_extent = |g: &mut Gen, hi: i64| if g.chance(60) { 0 } else { g.int(1, hi) };
    let (w, h, d) = (pick_extent(g, 5), pick_extent(g, 4), pick_extent(g, 3));
    let nf = match g.below(28) {
        0 => 0,
        1..=6 => g.int(2, 3),
        _ => 1,
    };
    let alloc = !g.chance(14);
    for v in [w, h, d, ty, nf, i64::from(alloc)] {
        t.push(v.to_string());
    }
    // voxel sizes
    for _ in 0..3 {
        let s = match g.below(12) {
            0 => g.pick(&[0.5f32, 2.0, 1.25, 3.0]),
            1 => g.pick(&[-1.0f32, 0.0, 1.0e-3, 7.5e6]),
            _ => 1.0,
        };
        t.push(fhex(s));
    }
    // the columns' direction cosines
    let mut cos = [-1.0f32, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 1.0, 0.0];
    match g.below(10) {
        0..=2 => {
            let perm = g.pick(&[
                [0, 1, 2],
                [0, 2, 1],
                [1, 0, 2],
                [1, 2, 0],
                [2, 0, 1],
                [2, 1, 0],
            ]);
            cos = [0.0; 9];
            for (axis, p) in perm.iter().enumerate() {
                cos[3 * axis + p] = if g.chance(2) { 1.0 } else { -1.0 };
            }
        }
        3 => {
            // oblique: a rotation about one axis
            let (c, s) = g.pick(&[
                (0.999_847_7f32, 0.017_452_406f32),
                (0.866_025_4, 0.5),
                (0.707_106_77, 0.707_106_77),
            ]);
            cos = [c, s, 0.0, -s, c, 0.0, 0.0, 0.0, 1.0];
        }
        4 => {
            // not unit, not orthogonal
            for x in cos.iter_mut() {
                *x = g.int(-8, 8) as f32 / 4.0;
            }
        }
        5 if g.chance(2) => {
            let axis = g.below(3) as usize;
            cos[3 * axis..3 * axis + 3].fill(0.0);
        }
        6 if g.chance(2) => {
            let at = g.below(9) as usize;
            cos[at] = f32::from_bits(g.pick(&[0x7fc0_0000u32, 0x7f80_0000]));
        }
        _ => {}
    }
    for x in cos {
        t.push(fhex(x));
    }
    for _ in 0..3 {
        let c = match g.below(10) {
            0 => g.pick(&[1.0e6f32, -3.5e-7, 123.456]),
            1..=4 => g.int(-200, 200) as f32 / 2.0,
            _ => 0.0,
        };
        t.push(fhex(c));
    }
    t.push(g.pick(&[1i64, 1, 1, 1, 1, 0, 0, -1]).to_string());
    for _ in 0..4 {
        t.push(fhex(0.0));
    }
    t.push(format!("{:016x}", 0f64.to_bits()));
    t.push(fhex(0.0));
    t.push("1".into());
    let n = (w * h * d * nf).max(0) as usize;
    let wide = g.chance(3);
    let constant = g.chance(8);
    let mut b = Vec::with_capacity(n * size_of(ty));
    let first = voxel(g, ty, wide);
    for i in 0..n {
        if i == 0 || constant {
            b.extend_from_slice(&first);
            for _ in first.len()..size_of(ty) {
                b.push(g.below(256) as u8);
            }
            continue;
        }
        let v = voxel(g, ty, wide);
        b.extend_from_slice(&v);
        for _ in v.len()..size_of(ty) {
            b.push(g.below(256) as u8);
        }
    }
    b.truncate(n * size_of(ty));
    t.push(if b.is_empty() { "-".into() } else { hex(&b) });
    t.push(enc(None));
    t.push(enc(None));
    t.push("0".into()); // no command lines
    t.push("0".into());
    t.push(fhex(0.0));
    for _ in 0..2 {
        t.push("-1".into());
        t.push("0".into());
    }
    t.push("-1".into()); // no warp
    t.push("0".into()); // no colour table
    t.push("-1".into()); // no b-values
    (nf, alloc)
}

pub fn generate(g: &mut Gen, kind: &str, portable: bool) -> Vec<String> {
    let mut t = vec![kind.to_owned()];
    let mut names = vec![
        "w/o.mnc",
        "w/o.mnc",
        "w/o.mnc",
        "w/o.mnc",
        "w/o.MNC",
        "w/s/o.mnc",
        "w/no/o.mnc",
        "w/o.mnc.gz",
        "w/o",
        "w/d",
        "w/a b.mnc",
    ];
    if !portable {
        names.push("w/f/o.mnc");
    }
    let name = g.pick(&names);
    let mut entries = Vec::new();
    match name {
        "w/d" => entries.push(dir_entry("w/d")),
        "w/s/o.mnc" => entries.push(dir_entry("w/s")),
        "w/f/o.mnc" => entries.push(file_entry("w/f", b"x")),
        "w/o.mnc" if g.chance(6) => entries.push(file_entry("w/o.mnc", b"older file")),
        _ => {}
    }
    for v in ["N", "N", "0", "N"] {
        t.push(v.into());
    }
    t.push(g.pick(&["0", "0", "0", "10000040"]).into());
    t.push(if entries.is_empty() {
        "-".into()
    } else {
        entries.join(",")
    });
    t.push("0".into());
    let by_name = name.ends_with(".mnc") && g.chance(4);
    t.push(if by_name { "1" } else { "11" }.into());
    let (nf, alloc) = volume(g, &mut t);
    t.push("-1".into());
    t.push(g.pick(&[-1i64, 0, 3]).to_string());
    t.push(enc(Some(name.as_bytes())));
    let back = alloc && nf > 0 && !g.chance(3);
    t.push(i64::from(back).to_string());
    t
}

/// The port's answer to a piece-26 case: piece 17's `vwr`, then the SKIP rules of what FreeSurfer
/// cannot do defined (the library's undefined writes, piece 25's for the read back).
pub fn run(
    tk: &[&str],
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    o: &mut Out,
    w: &Work,
    extra: &mut Vec<&'static str>,
) -> Result<(), u8> {
    g.progname = "fsio".into();
    let mut t: Vec<&str> = vec!["vwr"];
    t.extend_from_slice(&tk[1..]);
    let r = super::piece17::run(&t, g, out, err, o, w, extra);
    for n in g.notices.as_slice() {
        // volume_io's undefined writes (a dimension of size 0, a volume never allocated, no spatial
        // axes, no type: track C's probe does not compare them either)
        if matches!(
            n.defect.id,
            "FS-781" | "FS-789" | "FS-798" | "FS-799" | "FS-800" | "FS-801"
        ) {
            extra.push("SKIP");
        }
        // the read back's memory contents (piece 25's rules)
        if matches!(
            n.defect.id,
            "FS-745"
                | "FS-774"
                | "FS-776"
                | "FS-777"
                | "FS-778"
                | "FS-779"
                | "FS-782"
                | "FS-783"
                | "FS-784"
                | "FS-785"
                | "FS-790"
                | "FS-792"
                | "FS-795"
                | "FS-796"
        ) && g.notices.reproduce()
        {
            extra.push("SKIP");
        }
    }
    r
}
