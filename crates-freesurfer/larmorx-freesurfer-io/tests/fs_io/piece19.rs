// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Piece 19 (track A relay 28): GE Genesis and GE Signa LX series. `grd` and `grh` (`mri_read`
//! with voxels and `MRIreadHeader`, as piece 6's `mrd` and `mrh`, the type -1, 4 or 5): FreeSurfer
//! crashes once the series' names are found (FS-680), so `reproduce` is compared up to the crash;
//! `gfd` and `gfh` (voxels, header): FreeSurfer's two readers with FS-680's line corrected
//! (`genesisReadFixed`, `gelxReadFixed`: the probe's `ge_fixed.cpp`) against the port's with
//! FS-680 bypassed. The series built here: `I.<nnn>` and `<stem><n>.MR` (plain or zero-padded)
//! Genesis files, `e<n>s<n>i<n>` LX files, their headers field by field (extents, offsets, sizes,
//! timing, echoes, corners with NaN and infinite values), voxels cut or missing, a folder in a
//! file's place, gaps in the numbers; piece 6's environment bits 64 `GE_ODD`, 128 `GE_EVEN`, 256
//! `KILLIANY_SWAP`.

use std::io::Write;

use larmorx_freesurfer_core::globals::Globals;

use super::piece4::file_entry;
use super::{Gen, Out, Work, enc, hex};

pub const KINDS: [&str; 4] = ["grd", "grh", "gfd", "gfh"];

fn dir_entry(name: &str) -> String {
    format!("D{}", hex(name.as_bytes()))
}

/// A header float: usually a plain number near `v`, now and then a special one.
fn float(g: &mut Gen, v: f32) -> f32 {
    if !g.chance(10) {
        return v;
    }
    g.pick(&[
        f32::NAN,
        -f32::NAN,
        f32::from_bits(0x7fa0_0001),
        f32::from_bits(0xff80_0005),
        f32::INFINITY,
        f32::NEG_INFINITY,
        0.0,
        -0.0,
        1e-40,
        3.4e38,
    ])
}

fn put(b: &mut Vec<u8>, at: usize, v: &[u8]) {
    if b.len() < at + v.len() {
        b.resize(at + v.len(), 0);
    }
    b[at..at + v.len()].copy_from_slice(v);
}

/// The corners and normal: centre, normal, top-left, top-right, bottom-right (fifteen floats).
fn corners(g: &mut Gen) -> Vec<f32> {
    let (w, h) = (g.int(1, 8) as f32 * 8.0, g.int(1, 8) as f32 * 8.0);
    let o = [
        g.int(-20, 20) as f32,
        g.int(-20, 20) as f32,
        g.int(-20, 20) as f32,
    ];
    let mut v = vec![o[0], o[1], o[2]];
    let n = match g.below(4) {
        0 => [0.0, 0.0, 1.0],
        1 => [0.0, 1.0, 0.0],
        2 => [0.6, 0.0, 0.8],
        _ => [0.0, 0.0, 2.5],
    };
    v.extend(n);
    // top-left, top-right (along x), bottom-right (along y)
    let (tl, tr, br) = match g.below(3) {
        0 => (
            [o[0] - w / 2.0, o[1] + h / 2.0, o[2]],
            [o[0] + w / 2.0, o[1] + h / 2.0, o[2]],
            [o[0] + w / 2.0, o[1] - h / 2.0, o[2]],
        ),
        1 => (
            [o[0] - w / 2.0, o[1], o[2] + h / 2.0],
            [o[0] + w / 2.0, o[1], o[2] + h / 2.0],
            [o[0] + w / 2.0, o[1], o[2] - h / 2.0],
        ),
        _ => (
            [o[0] - 3.0, o[1] + 4.0, o[2] + 1.5],
            [o[0] + 5.0, o[1] - 2.0, o[2] + 1.5],
            [o[0] + 7.0, o[1] + 1.0, o[2] - 4.5],
        ),
    };
    v.extend(tl);
    v.extend(tr);
    v.extend(br);
    v.into_iter().map(|x| float(g, x)).collect()
}

/// `n` big-endian shorts.
fn voxels(g: &mut Gen, n: usize) -> Vec<u8> {
    let mut b = Vec::with_capacity(n * 2);
    for _ in 0..n {
        b.extend_from_slice(&(g.int(-3000, 3000) as i16).to_be_bytes());
    }
    b
}

fn extent(g: &mut Gen) -> i32 {
    if g.chance(25) {
        g.pick(&[0, -1, 70000])
    } else {
        g.int(1, 4) as i32
    }
}

/// A file's bytes cut now and then (a random length, or inside the header).
fn cut(g: &mut Gen, mut b: Vec<u8>, header: usize) -> Vec<u8> {
    if g.chance(12) && !b.is_empty() {
        let k = if g.chance(2) {
            g.below(header.min(b.len()) as u64 + 1) as usize
        } else {
            g.below(b.len() as u64) as usize
        };
        b.truncate(k);
    }
    b
}

/// A Genesis file: width, height, offsets and the image header's fields.
fn genesis_file(g: &mut Gen, w: i32, h: i32, nframes: i16, first: bool) -> Vec<u8> {
    let ih = g.pick(&[156usize, 200, 300]);
    let pixel = if g.chance(15) {
        g.pick(&[-8i32, 1 << 20, 3])
    } else {
        (ih + 256 + g.pick(&[0, 0, 8])) as i32
    };
    let mut b = vec![0u8; ih + 256];
    if !g.chance(20) {
        b[..4].copy_from_slice(b"IMGF");
    }
    put(&mut b, 4, &pixel.to_be_bytes());
    put(&mut b, 8, &w.to_be_bytes());
    put(&mut b, 12, &h.to_be_bytes());
    let ihv = if g.chance(25) {
        g.pick(&[-40i32, 100_000])
    } else {
        ih as i32
    };
    put(&mut b, 148, &ihv.to_be_bytes());
    put(
        &mut b,
        ih + 26,
        &{
            let v = g.pick(&[1.0f32, 1.5, 3.0]);
            float(g, v)
        }
        .to_be_bytes(),
    );
    put(
        &mut b,
        ih + 50,
        &{
            let v = g.pick(&[0.9375f32, 1.0, 2.0]);
            float(g, v)
        }
        .to_be_bytes(),
    );
    put(
        &mut b,
        ih + 54,
        &{
            let v = g.pick(&[0.9375f32, 1.0, 2.5]);
            float(g, v)
        }
        .to_be_bytes(),
    );
    for (k, x) in corners(g).iter().enumerate() {
        put(&mut b, ih + 130 + 4 * k, &x.to_be_bytes());
    }
    put(
        &mut b,
        ih + 194,
        &(g.int(0, 3_000_000) as i32).to_be_bytes(),
    );
    put(&mut b, ih + 198, &(g.int(0, 900_000) as i32).to_be_bytes());
    put(&mut b, ih + 202, &(g.int(0, 90_000) as i32).to_be_bytes());
    put(&mut b, ih + 210, &nframes.to_be_bytes());
    put(&mut b, ih + 254, &(g.int(-90, 180) as i16).to_be_bytes());
    if pixel > 0 && (pixel as usize) < 1 << 16 {
        b.resize(pixel as usize, 0);
        let n = (w.max(0) * h.max(0)) as usize;
        if n < 1 << 12 {
            b.extend(voxels(g, n));
        }
    }
    if first {
        cut(g, b, ih + 256)
    } else {
        cut(g, b, 8)
    }
}

/// An LX file: its header at 2,184, extents at 3,236, voxels from 8,432.
fn lx_file(g: &mut Gen, w: i32, h: i32) -> Vec<u8> {
    let mut b = vec![0u8; 8432];
    if !g.chance(20) {
        put(&mut b, 3228, b"IMGF");
    }
    put(&mut b, 3236, &w.to_be_bytes());
    put(&mut b, 3240, &h.to_be_bytes());
    put(
        &mut b,
        2184 + 28,
        &{
            let v = g.pick(&[1.0f32, 1.5, 3.0]);
            float(g, v)
        }
        .to_be_bytes(),
    );
    put(
        &mut b,
        2184 + 52,
        &{
            let v = g.pick(&[0.9375f32, 1.0]);
            float(g, v)
        }
        .to_be_bytes(),
    );
    put(
        &mut b,
        2184 + 56,
        &{
            let v = g.pick(&[0.9375f32, 2.0]);
            float(g, v)
        }
        .to_be_bytes(),
    );
    for (k, x) in corners(g).iter().enumerate() {
        put(&mut b, 2184 + 136 + 4 * k, &x.to_be_bytes());
    }
    let n = (w.max(0) * h.max(0)) as usize;
    if n < 1 << 12 {
        b.extend(voxels(g, n));
    }
    let len = b.len();
    if g.chance(12) {
        let k = if g.chance(2) {
            g.int(2300, 3244) as usize
        } else {
            g.below(len as u64) as usize
        };
        b.truncate(k);
    }
    b
}

/// The series' files: numbers `lo..=hi` (a gap now and then), each by `name(n)`; the name to read.
fn series(
    g: &mut Gen,
    entries: &mut Vec<String>,
    name: &dyn Fn(i64) -> String,
    lo: i64,
    hi: i64,
    file: &mut dyn FnMut(&mut Gen, bool) -> Vec<u8>,
) -> String {
    let gap = if hi > lo && g.chance(10) {
        Some(g.int(lo, hi))
    } else {
        None
    };
    for n in lo..=hi {
        if Some(n) == gap {
            continue;
        }
        if n > lo && g.chance(30) {
            entries.push(dir_entry(&name(n)));
            continue;
        }
        let b = file(g, n == lo);
        entries.push(file_entry(&name(n), &b));
    }
    let pick = if g.chance(15) {
        hi + 1 + g.int(0, 2)
    } else {
        g.int(lo, hi)
    };
    name(pick)
}

pub fn generate(g: &mut Gen, kind: &str, _portable: bool) -> Vec<String> {
    let mut t = vec![kind.to_owned()];
    let mut entries = Vec::new();
    let lx = g.chance(3);
    let fixed = kind.starts_with("gf");
    let (w, h) = (extent(g), extent(g));
    let mut bits = 0;
    let name = if lx {
        let pre = g.pick(&["w/e1s2i", "w/e12s3i", "w/s4e5i", "w/i"]);
        let lo = g.int(0, 3);
        let hi = lo + g.int(0, 3);
        let nm = move |n: i64| format!("{pre}{n}");
        let pick = series(g, &mut entries, &nm, lo, hi, &mut |g, _| lx_file(g, w, h));
        if g.chance(10) {
            // names of no LX form (made as files, so the form is what refuses them; none differs
            // from a series file by case alone: Windows' names ignore case)
            let bad = g.pick(&["w/e1s2x3", "w/e1e2s3i4", "w/e1s2i", "w/q1s2i3"]);
            entries.push(file_entry(bad, b"x"));
            bad.to_owned()
        } else {
            pick
        }
    } else {
        let nframes: i16 = if g.chance(5) {
            g.pick(&[0, 2, 3, -1, -30000])
        } else {
            1
        };
        if g.chance(5) {
            bits |= g.pick(&[64, 128, 192]);
        }
        if g.chance(6) {
            bits |= 256;
        }
        let lo = g.int(0, 3);
        let hi = lo + g.int(0, if nframes > 1 { 5 } else { 3 });
        let style = g.below(4);
        let stem = g.pick(&["w/s", "w/ab", "w/7", "w/"]);
        let nm = move |n: i64| match style {
            0 | 1 => format!("w/I.{n:03}"),
            2 => format!("{stem}{n}.MR"),
            _ => format!("{stem}{n:03}.MR"),
        };
        let pick = series(g, &mut entries, &nm, lo, hi, &mut |g, first| {
            genesis_file(g, w, h, nframes, first)
        });
        if g.chance(12) {
            // both series of a .MR stem: the plain numbers and the zero-padded ones
            for n in 0..g.int(1, 3) {
                let other = if style == 3 {
                    format!("{stem}{}.MR", hi + 10 + n)
                } else {
                    format!("{stem}{:03}.MR", lo + n)
                };
                let b = genesis_file(g, w, h, nframes, true);
                entries.push(file_entry(&other, &b));
            }
        }
        if g.chance(12) {
            g.pick(&["w/I.", "w/x.MR", "w/MR", "w/I.abc", "w/.MR", "w/ab.mr"])
                .to_owned()
        } else {
            pick
        }
    };
    let ty = if fixed {
        if lx { 5 } else { 4 }
    } else if g.chance(4) {
        -1
    } else if lx {
        5
    } else {
        4
    };
    t.push("0".into());
    t.push(if g.chance(12) { "0" } else { "1" }.into());
    t.push(bits.to_string());
    t.push(ty.to_string());
    t.push(enc(Some(name.as_bytes())));
    t.push(entries.join(","));
    t
}

/// The port's answer to a piece-19 case: piece 6's `mrd`/`mrh`, or the readers with FS-680
/// bypassed (`gfd`, `gfh`).
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
    let mut t: Vec<&str> = vec![match tk[0] {
        "grd" => "mrd",
        "grh" => "mrh",
        k => k,
    }];
    t.extend_from_slice(&tk[1..]);
    let r = super::piece6::run_env(&t, g, out, err, o, w, extra, &[]);
    for n in g.notices.as_slice() {
        // FreeSurfer's names undefined (a % in them)
        if n.defect.id == "FS-682" {
            extra.push("SKIP");
        }
        // FreeSurfer's outcome from memory contents (past a frame table)
        if n.defect.id == "FS-685" {
            extra.push("VARY");
        }
    }
    r
}
