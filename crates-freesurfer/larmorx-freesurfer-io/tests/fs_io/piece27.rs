// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Piece 27 (track A relay 35): GCA atlases read as volumes. `gcd` and `gch` (piece 6's `mrd` and
//! `mrh`, the type -1 or 22, a name ending in `.gca`, a `.gcz` name of type 22, or `@gca`) of
//! atlas files built here as `GCAwrite` lays them out: every version (the old layouts 1, 2 and 3,
//! the byte labels of 4, the current 5, others refused, NaN), node and prior spacings ordinary,
//! fractional, equal, inverted, 0, negative or NaN over training volumes of a few voxels (so the
//! grids' sizes the file stores and those its spacings give agree or not: FS-749), now and then a
//! stored grid one larger; 0 to 3 inputs, and now and then 101 to 112 or past 46,340; flags with
//! and without neighbour statistics; nodes of 0 to 3 classes (now and then a negative or huge
//! count), labels in and past 16 bits, means and covariances ordinary, NaN, infinite or
//! subnormal; neighbour lists of 0 to 2 labels (now and then a count past 16 bits or negative);
//! priors of 0 to 3 labels with ties, zeros, NaN; then the tagged section (junk before it now and
//! then): the type, the MR parameters, a binary colour table, the direction cosines (coronal,
//! RAS, permuted, oblique, of length 0, NaN), sizes and voxel sizes, unknown tags. Frames by the
//! name (`#0`, `#1`, `#2`, `#3`, none), `Gdiag` with `DIAG_SHOW | DIAG_VERBOSE` now and then; and
//! now and then one fault: the file cut short, a missing file, a folder in its place, a file that
//! is not an atlas, a compressed `.gcz`.

use std::io::Write;

use larmorx_freesurfer_core::cstd::double_to_int_x86;
use larmorx_freesurfer_core::globals::Globals;

use super::piece4::file_entry;
use super::piece22::ctab;
use super::{Gen, Out, Work, enc, hex};

pub const KINDS: [&str; 2] = ["gcd", "gch"];

const FILE_TAG: i32 = 0xab2c;

fn dir_entry(name: &str) -> String {
    format!("D{}", hex(name.as_bytes()))
}

fn be32(d: &mut Vec<u8>, v: i32) {
    d.extend_from_slice(&v.to_be_bytes());
}

fn bef(d: &mut Vec<u8>, v: f32) {
    d.extend_from_slice(&v.to_be_bytes());
}

/// `(int)(((float)width / spacing) + .99)`, as x86-64 converts.
fn grid(w: i32, s: f32) -> i32 {
    double_to_int_x86(f64::from(w as f32 / s) + 0.99)
}

fn cells(d: [i32; 3]) -> i64 {
    if d.iter().any(|&v| v <= 0) {
        0
    } else {
        d.iter().map(|&v| i64::from(v)).product()
    }
}

/// A mean, covariance or parameter value.
fn value(g: &mut Gen) -> f32 {
    match g.below(24) {
        0 => f32::from_bits(0x7fc0_0000),
        1 => f32::from_bits(0x7fa0_0001),
        2 => f32::from_bits(0xffc0_0005),
        3 => f32::INFINITY,
        4 => f32::NEG_INFINITY,
        5 => f32::from_bits(1),
        6 => -0.0,
        _ => g.int(-800, 800) as f32 / 8.0,
    }
}

/// A prior.
fn prior(g: &mut Gen) -> f32 {
    match g.below(24) {
        0 => f32::from_bits(0x7fc0_0000),
        1 => f32::from_bits(0x7fa0_0003),
        2 | 3 => 0.0,
        4 => 1e-16,
        5 => -0.25,
        6..=9 => 0.25,
        _ => g.int(1, 100) as f32 / 100.0,
    }
}

/// The one anomaly an atlas may carry (at most one per file, at a place chosen at random).
#[derive(Clone, Copy, PartialEq)]
enum Anomaly {
    None,
    /// a label past 16 bits or negative (written as an `int`)
    Label(i32),
    /// a neighbour label count past 16 bits or negative
    NbrCount(i32),
    /// a prior label count past 16 bits or negative
    PriorCount(i32),
    /// a node class count negative or huge
    NodeCount(i32),
}

/// The anomaly at an eligible place (one chance in four), used once.
fn take(g: &mut Gen, an: &mut Anomaly, want: fn(Anomaly) -> Option<i32>) -> Option<i32> {
    let v = want(*an)?;
    if g.chance(4) {
        *an = Anomaly::None;
        Some(v)
    } else {
        None
    }
}

/// A label: small (a byte, or an `int` that fits), or the atlas's label anomaly.
fn label(g: &mut Gen, as_byte: bool, an: &mut Anomaly) -> i32 {
    if as_byte {
        return g.pick(&[0, 2, 3, 4, 17, 41, 255]);
    }
    if let Some(v) = take(g, an, |a| match a {
        Anomaly::Label(v) => Some(v),
        _ => None,
    }) {
        return v;
    }
    g.pick(&[0, 2, 3, 4, 17, 41, 255, 1000, 65_535])
}

/// The direction cosines, centre, sizes and voxel sizes of `TAG_GCA_DIRCOS`.
fn dircos(g: &mut Gen, d: &mut Vec<u8>) {
    let c: [f32; 9] = match g.below(12) {
        0..=3 => [-1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 1.0, 0.0],
        4 | 5 => [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
        6 => [0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0],
        7 => [0.6, 0.8, 0.0, -0.8, 0.6, 0.0, 0.0, 0.0, 1.0],
        8 => [0.0; 9],
        9 => [f32::NAN, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
        _ => [0.5, 0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 1.0],
    };
    for v in c {
        bef(d, v);
    }
    for _ in 0..3 {
        bef(d, g.int(-80, 80) as f32 / 4.0);
    }
    for _ in 0..3 {
        let v = match g.below(30) {
            0 => 0,
            1 => -2,
            _ => g.int(1, 5) as i32,
        };
        be32(d, v);
    }
    for _ in 0..3 {
        let v = match g.below(30) {
            0 => 0.0,
            1 => f32::NAN,
            _ => g.pick(&[1.0, 1.0, 2.0, 0.5, 1.5]),
        };
        bef(d, v);
    }
}

/// An atlas file, as `GCAwrite` lays one out (or not).
fn gca_file(g: &mut Gen) -> Vec<u8> {
    let version: f32 = g.pick(&[
        5.0,
        5.0,
        5.0,
        5.0,
        5.0,
        4.0,
        4.0,
        3.0,
        2.0,
        1.0,
        0.5,
        4.5,
        6.0,
        f32::NAN,
        -1.0,
    ]);
    let v = f64::from(version);
    let old = v < 4.0;
    let accepted = old || version == 4.0 || version == 5.0;
    let bytes = old || version == 4.0;
    let mut d = Vec::new();
    bef(&mut d, version);
    if !accepted {
        d.extend_from_slice(&[0u8; 24]);
        return d;
    }
    // the spacings and a small training volume, until the grids are small
    let (mut ps, mut ns, mut whd);
    let mut tries = 0;
    loop {
        (ps, ns) = g.pick(&[
            (2.0f32, 4.0f32),
            (2.0, 4.0),
            (1.0, 2.0),
            (1.0, 1.0),
            (2.0, 2.0),
            (1.5, 3.0),
            (0.5, 1.0),
            (0.4, 0.4),
            (0.25, 0.5),
            (2.0, 1.0),
            (0.0, 4.0),
            (4.0, 0.0),
            (f32::NAN, 2.0),
            (-1.0, 2.0),
            (1.0, -2.0),
        ]);
        if old {
            ps = ns;
        }
        whd = [0i32; 3];
        for w in &mut whd {
            *w = if g.chance(25) { 0 } else { g.int(1, 6) as i32 };
        }
        let nw = whd.map(|w| grid(w, ns));
        let pw = whd.map(|w| grid(w, ps));
        tries += 1;
        if (cells(nw) <= 64 && cells(pw) <= 216 && nw.iter().chain(&pw).all(|&x| x > -1000))
            || tries > 30
        {
            break;
        }
    }
    if tries > 30 {
        (ps, ns, whd) = (2.0, 4.0, [4, 4, 4]);
    }
    let mut nw = whd.map(|w| grid(w, ns));
    let pw = whd.map(|w| grid(w, ps));
    if g.chance(30) {
        // a hand-made file: one node axis one larger than its spacing gives
        let a = g.below(3) as usize;
        nw[a] += 1;
    }
    let ninputs: i32 = match g.below(48) {
        0 => -1,
        1 => 0,
        2 => 101,
        3 => 109,
        4 => 110,
        5 => 50_000,
        6 => 70_000,
        7..=10 => 2,
        11 | 12 => 3,
        _ => 1,
    };
    let flags: i32 = if old && version != 3.0 {
        0
    } else {
        g.pick(&[0, 0, 1, 1, 0x10])
    };
    if old {
        bef(&mut d, ns);
        for v in nw {
            be32(&mut d, v);
        }
        be32(&mut d, ninputs);
        if version == 3.0 {
            be32(&mut d, flags);
        }
    } else {
        bef(&mut d, ps);
        bef(&mut d, ns);
        for v in pw {
            be32(&mut d, v);
        }
        for v in nw {
            be32(&mut d, v);
        }
        be32(&mut d, ninputs);
        be32(&mut d, flags);
    }
    let mut an = match g.below(36) {
        0 => Anomaly::Label(70_000),
        1 => Anomaly::Label(-3),
        2 => Anomaly::NbrCount(-1),
        3 => Anomaly::NbrCount(40_000),
        4 => Anomaly::NbrCount(70_001),
        5 => Anomaly::PriorCount(40_000),
        6 => Anomaly::PriorCount(-2),
        7 => Anomaly::PriorCount(70_002),
        8 => Anomaly::NodeCount(-1),
        9 => Anomaly::NodeCount(70_000),
        _ => Anomaly::None,
    };
    // many inputs: classes rarely (their means and covariances are long)
    let wide = !(0..=3).contains(&ninputs);
    let ncov = if wide {
        0
    } else {
        i64::from(ninputs) * i64::from(ninputs + 1) / 2
    };
    for _ in 0..cells(nw) {
        let odd = take(g, &mut an, |a| match a {
            Anomaly::NodeCount(v) => Some(v),
            _ => None,
        });
        let nlabels: i32 = match odd {
            Some(v) => v,
            None if wide && !g.chance(12) => 0,
            None => g.pick(&[0, 0, 1, 1, 1, 2, 3]),
        };
        be32(&mut d, nlabels);
        be32(&mut d, g.pick(&[0, 1, 5, 17, 100, -3, i32::MAX]));
        if nlabels <= 0 {
            continue;
        }
        for _ in 0..nlabels.min(3) {
            let l = label(g, bytes, &mut an);
            if bytes {
                d.push(l as u8);
            } else {
                be32(&mut d, l);
            }
            if wide {
                // a few of the long means: the file ends inside them
                for _ in 0..8 {
                    bef(&mut d, value(g));
                }
                return d;
            }
            for _ in 0..ninputs.max(0) {
                bef(&mut d, value(g));
            }
            for _ in 0..ncov {
                bef(&mut d, value(g));
            }
            if flags & 1 == 0 {
                for _ in 0..6 {
                    let c = take(g, &mut an, |a| match a {
                        Anomaly::NbrCount(v) => Some(v),
                        _ => None,
                    })
                    .unwrap_or_else(|| g.int(0, 2) as i32);
                    be32(&mut d, c);
                    for _ in 0..c.clamp(0, 2) {
                        be32(&mut d, label(g, false, &mut an));
                        bef(&mut d, prior(g));
                    }
                }
            }
        }
        if nlabels > 3 {
            return d;
        }
    }
    if !old {
        for _ in 0..cells(pw) {
            let nl = take(g, &mut an, |a| match a {
                Anomaly::PriorCount(v) => Some(v),
                _ => None,
            })
            .unwrap_or_else(|| g.int(0, 3) as i32);
            be32(&mut d, nl);
            be32(&mut d, g.pick(&[0, 1, 4, 9, 100, -2]));
            for _ in 0..nl.clamp(0, 3) {
                let l = label(g, bytes, &mut an);
                if bytes {
                    d.push(l as u8);
                } else {
                    be32(&mut d, l);
                }
                bef(&mut d, prior(g));
            }
            if nl > 3 {
                return d;
            }
        }
    }
    if g.chance(7) {
        return d;
    }
    if g.chance(10) {
        be32(&mut d, 7);
        be32(&mut d, 123);
    }
    be32(&mut d, FILE_TAG);
    let mut tags: Vec<u32> = Vec::new();
    if g.chance(2) {
        tags.push(2);
    }
    if g.chance(3) && ninputs <= 120 {
        tags.push(1);
    }
    if g.chance(5) {
        tags.push(4);
    }
    if g.chance(10) {
        tags.push(9);
    }
    if !g.chance(4) {
        tags.push(3);
    }
    if g.chance(4) {
        let a = g.below(tags.len().max(1) as u64) as usize;
        let b = g.below(tags.len().max(1) as u64) as usize;
        if a < tags.len() && b < tags.len() {
            tags.swap(a, b);
        }
    }
    for t in tags {
        match t {
            2 => {
                be32(&mut d, 2);
                be32(&mut d, 1);
                be32(&mut d, g.pick(&[0, 1, 2, 99, 5]));
            }
            1 => {
                be32(&mut d, 1);
                be32(&mut d, g.pick(&[3, 3, 0, ninputs]));
                for _ in 0..ninputs.max(0) {
                    let tr = if g.chance(10) { value(g) } else { 20.0 };
                    bef(&mut d, tr);
                    bef(
                        &mut d,
                        g.pick(&[0.523_598_8f32, 0.087_266_46, 0.0, f32::NAN]),
                    );
                    bef(&mut d, g.pick(&[5.0f32, 2.5, 30.0]));
                }
            }
            4 => {
                be32(&mut d, 4);
                let cols: Vec<(i32, i32, i32)> = (0..g.int(1, 3))
                    .map(|i| (40 * i as i32, 200 - 50 * i as i32, (i as i32 * 91) % 256))
                    .collect();
                ctab(g, &mut d, &cols);
            }
            3 => {
                be32(&mut d, 3);
                dircos(g, &mut d);
            }
            _ => be32(&mut d, g.pick(&[7, 0, -1, FILE_TAG])),
        }
    }
    d
}

/// A gzip stream of `b` stored (no compression), as `gzip -0` writes one.
fn gz_stored(b: &[u8]) -> Vec<u8> {
    let mut o = vec![0x1f, 0x8b, 8, 0, 0, 0, 0, 0, 0, 3];
    for (i, block) in b.chunks(65535).enumerate() {
        let last = (i + 1) * 65535 >= b.len();
        o.push(u8::from(last));
        let n = block.len() as u16;
        o.extend_from_slice(&n.to_le_bytes());
        o.extend_from_slice(&(!n).to_le_bytes());
        o.extend_from_slice(block);
    }
    if b.is_empty() {
        o.extend_from_slice(&[1, 0, 0, 0xff, 0xff]);
    }
    let mut c = 0xffff_ffffu32;
    for &x in b {
        c ^= u32::from(x);
        for _ in 0..8 {
            c = if c & 1 != 0 {
                (c >> 1) ^ 0xedb8_8320
            } else {
                c >> 1
            };
        }
    }
    o.extend_from_slice(&(!c).to_le_bytes());
    o.extend_from_slice(&(b.len() as u32).to_le_bytes());
    o
}

fn read_case(g: &mut Gen, entries: &mut Vec<String>) -> (String, i64) {
    let (name, ty) = g.pick(&[
        ("w/a.gca", -1i64),
        ("w/a.gca", -1),
        ("w/a.gca", -1),
        ("w/a.gca", 22),
        ("w/a.gcz", 22),
        ("w/a.dat@gca", -1),
    ]);
    let file = name.split('@').next().unwrap_or(name);
    match g.below(36) {
        0 => entries.push(dir_entry(file)),
        1 => {}
        2 => entries.push(file_entry(file, b"not an atlas at all")),
        3 => entries.push(file_entry(file, b"")),
        4..=6 => {
            let mut b = gca_file(g);
            let cut = g.below(b.len() as u64 + 1) as usize;
            b.truncate(cut);
            entries.push(file_entry(file, &b));
        }
        _ => {
            let b = gca_file(g);
            let b = if file.ends_with(".gcz") {
                gz_stored(&b)
            } else {
                b
            };
            entries.push(file_entry(file, &b));
        }
    }
    let name = match g.below(10) {
        0..=4 => name.to_owned(),
        n => format!("{name}#{}", ["0", "1", "2", "3", "0:2"][n as usize - 5]),
    };
    (name, ty)
}

pub fn generate(g: &mut Gen, kind: &str, _portable: bool) -> Vec<String> {
    let mut t = vec![kind.to_owned()];
    let mut entries = Vec::new();
    let (name, ty) = read_case(g, &mut entries);
    t.push(g.pick(&["0", "0", "0", "40", "10000040"]).into()); // Gdiag
    t.push(if g.chance(12) { "0" } else { "1" }.into()); // MRIIO_Strip_Pound
    t.push("0".into()); // piece 6's environment bits
    t.push(ty.to_string());
    t.push(enc(Some(name.as_bytes())));
    t.push(if entries.is_empty() {
        "-".into()
    } else {
        entries.join(",")
    });
    t
}

/// The port's answer to a piece-27 case: piece 6's `mrd`/`mrh`.
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
    let mut t: Vec<&str> = vec![if tk[0] == "gch" { "mrh" } else { "mrd" }];
    t.extend_from_slice(&tk[1..]);
    let r = super::piece6::run_env(&t, g, out, err, o, w, extra, &[]);
    for n in g.notices.as_slice() {
        // memory contents, a memory address, the machine's memory, a crash (FreeSurfer's
        // atlas reader)
        if matches!(
            n.defect.id,
            "FS-750" | "FS-755" | "FS-756" | "FS-757" | "FS-758" | "FS-759" | "FS-167"
        ) && g.notices.reproduce()
        {
            extra.push("SKIP");
        }
    }
    r
}
