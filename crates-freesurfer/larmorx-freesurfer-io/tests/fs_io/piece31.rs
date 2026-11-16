// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Piece 31 (track A relay 38): GCA morphs read as volumes. `m3d` and `m3h` (piece 6's `mrd` and
//! `mrh`: the type -1 or 29, `.m3z` and `.m3d` names, `@m3z`) of morph files built here as
//! `GCAMwrite` lays them out: the version (1.0, others refused, NaN), a grid of 0 to 5 nodes a
//! side (now and then a negative size or one past the machine's memory: FS-851), spacings
//! ordinary, 0, negative or with a cube past 32 bits (FS-853), `exp_k`; per node the original and
//! current positions (near the node, all zero: an invalid position, NaN, infinite, far away) and
//! node coordinates; then tags in any order: the labels, the two geometries (small valid images
//! for the node-coordinate frames), the type (voxels, RAS, others), the affine matrix (a positive,
//! negative, zero or NaN determinant), unknown tags, a 0 or the file's end; `FS_SKIP_TAGS` and
//! `SHOW_NEG` now and then. Frames by the name: none, `#0` to `#24`, `#11`, `#12`, `#25` (fields
//! `GCAMwriteMRI` does not write), `#7`, `#8`, `#9` (through the inverse). Faults: the file cut
//! short, a missing file, a folder, a file that is not a morph, an empty one, a compressed file
//! damaged after its nodes (FS-854). And MGH warp volumes named with `@m3z` (`Warpfield::read`):
//! the warp's version or not, its formats, spacing, geometries, affine and labels.

use std::io::Write;

use flate2::Compression;
use flate2::write::GzEncoder;
use larmorx_freesurfer_core::globals::Globals;

use super::piece4::file_entry;
use super::{Gen, Out, Work, enc, hex};

pub const KINDS: [&str; 2] = ["m3d", "m3h"];

fn be32(d: &mut Vec<u8>, v: i32) {
    d.extend_from_slice(&v.to_be_bytes());
}

fn bef(d: &mut Vec<u8>, v: f32) {
    d.extend_from_slice(&v.to_bits().to_be_bytes());
}

fn tag_long(d: &mut Vec<u8>, tag: i32, data: &[u8]) {
    be32(d, tag);
    d.extend_from_slice(&(data.len() as i64).to_be_bytes());
    d.extend_from_slice(data);
}

fn dir_entry(name: &str) -> String {
    format!("D{}", hex(name.as_bytes()))
}

/// A special float now and then.
fn special(g: &mut Gen) -> Option<f32> {
    Some(match g.below(40) {
        0 => f32::from_bits(0x7fc0_0000),
        1 => f32::from_bits(0x7fa0_0001),
        2 => f32::INFINITY,
        3 => f32::NEG_INFINITY,
        4 => f32::from_bits(1),
        5 => -0.0,
        6 => 1e30,
        _ => return None,
    })
}

/// A displacement or a geometry's number.
fn small(g: &mut Gen) -> f32 {
    special(g).unwrap_or_else(|| g.int(-24, 24) as f32 / 8.0)
}

/// A geometry block as `VOL_GEOM::write` lays it out without shears: valid, sizes, voxel sizes,
/// direction cosines, centre, 512 name bytes.
fn vol_geom(g: &mut Gen, small_image: bool) -> Vec<u8> {
    let mut b = Vec::new();
    let dim = |g: &mut Gen| {
        if small_image || g.chance(3) {
            g.int(1, 4) as i32
        } else {
            g.pick(&[0, 2, 3, 5])
        }
    };
    be32(&mut b, g.pick(&[1, 1, 0]));
    for _ in 0..3 {
        let v = dim(g);
        be32(&mut b, v);
    }
    for k in 0..15 {
        let v = match k {
            0..=2 => {
                if !small_image && g.chance(12) {
                    small(g)
                } else {
                    g.pick(&[1.0, 2.0, 0.5, 1.5])
                }
            }
            // a direction cosine matrix: a permutation with signs, or oblique now and then
            3..=11 => {
                let (r, c) = ((k - 3) / 3, (k - 3) % 3);
                if g.chance(10) {
                    small(g)
                } else if r == c {
                    g.pick(&[1.0, -1.0])
                } else {
                    0.0
                }
            }
            _ => g.int(-40, 40) as f32 / 4.0,
        };
        bef(&mut b, v);
    }
    let mut name = vec![0u8; 512];
    let s = g.pick(&["", "orig.mgz", "/x/y/T1.mgz"]).as_bytes();
    name[..s.len()].copy_from_slice(s);
    b.extend_from_slice(&name);
    b
}

/// A matrix tag's 1,600 bytes of text (`Matrix` and sixteen numbers, zeros after).
fn matrix_text(g: &mut Gen) -> Vec<u8> {
    let mut t = b"Matrix".to_vec();
    let kind = g.below(8);
    for k in 0..16 {
        let (r, c) = (k / 4, k % 4);
        let v = match kind {
            // singular
            0 => "0".to_owned(),
            // a negative determinant
            1 if r == c && r == 0 => "-1.5".to_owned(),
            2 if k == 5 => g.pick(&["nan", "inf"]).to_owned(),
            _ => {
                if r == 3 {
                    if c == 3 { "1" } else { "0" }.to_owned()
                } else if r == c {
                    format!("{:10.6}", g.int(4, 12) as f64 / 8.0)
                } else {
                    format!("{:10.6}", g.int(-20, 20) as f64 / 16.0)
                }
            }
        };
        t.push(if c == 0 { b'\n' } else { b' ' });
        t.extend_from_slice(v.as_bytes());
    }
    t.resize(1600, 0);
    t
}

/// What the case's grid is.
struct Grid {
    w: i32,
    h: i32,
    d: i32,
}

impl Grid {
    fn count(&self) -> usize {
        if self.w <= 0 || self.h <= 0 || self.d <= 0 {
            0
        } else {
            (i64::from(self.w) * i64::from(self.h) * i64::from(self.d)) as usize
        }
    }
}

/// An `.m3z`'s uncompressed bytes (`GCAMwrite`'s layout) and where its nodes end; `node_frames`
/// asks for a small valid image geometry (the node-coordinate frames go through the inverse).
pub(crate) fn m3z_bytes(g: &mut Gen, node_frames: bool) -> (Vec<u8>, usize) {
    let mut b = Vec::new();
    let version = match g.below(30) {
        0 => 2.0,
        1 => f32::NAN,
        2 => 0.999,
        _ => 1.0,
    };
    bef(&mut b, version);
    let grid = match g.below(40) {
        0 => Grid { w: -1, h: 2, d: 2 },
        1 => Grid { w: 2, h: -3, d: 2 },
        2 => Grid { w: 2, h: 2, d: -1 },
        3 => Grid { w: 0, h: 3, d: 3 },
        4 => Grid { w: 3, h: 0, d: -5 },
        // past the machine's memory (FS-851)
        5 => Grid {
            w: 2000,
            h: 2000,
            d: g.pick(&[2000, 1000]),
        },
        6 => Grid { w: 1, h: 1, d: 1 },
        _ => Grid {
            w: g.int(1, 5) as i32,
            h: g.int(1, 5) as i32,
            d: g.int(1, 5) as i32,
        },
    };
    for v in [grid.w, grid.h, grid.d] {
        be32(&mut b, v);
    }
    let spacing = match g.below(20) {
        0 => 0,
        1 => -2,
        2 => 1291,
        3 => -2000,
        4 => 3,
        5..=9 => 1,
        _ => 2,
    };
    be32(&mut b, spacing);
    bef(&mut b, if g.chance(10) { small(g) } else { 20.0 });
    // per node: a displacement field around the node, its parts chosen per file
    let scale = g.pick(&[0.0f32, 0.25, 0.5, 1.0, 3.0]);
    let flip = g.chance(6);
    // a grid past the machine's memory ends at its allocation: no nodes written (the case line
    // stays short)
    let n = if grid.count() > 200 { 0 } else { grid.count() };
    let (w, h, d) = (grid.w.max(0), grid.h.max(0), grid.d.max(0));
    let mut i = 0;
    'nodes: for x in 0..w {
        for y in 0..h {
            for z in 0..d {
                if i >= n {
                    break 'nodes;
                }
                i += 1;
                if g.chance(15) {
                    // an invalid position: all six zero (now and then nearly zero)
                    let v = if g.chance(3) { 1e-8 } else { 0.0 };
                    for _ in 0..6 {
                        bef(&mut b, v);
                    }
                } else {
                    let (fx, fy, fz) = (x as f32, y as f32, z as f32);
                    for v in [fx, fy, fz] {
                        bef(&mut b, special(g).unwrap_or(v));
                    }
                    let disp = |g: &mut Gen| scale * g.int(-8, 8) as f32 / 8.0;
                    let px = if flip { -fx } else { fx } + disp(g);
                    let py = fy + disp(g);
                    let pz = fz + disp(g);
                    for v in [px, py, pz] {
                        bef(&mut b, special(g).unwrap_or(v));
                    }
                }
                for v in [x, y, z] {
                    let v = if g.chance(20) {
                        g.pick(&[-1, 7, i32::MAX])
                    } else {
                        v
                    };
                    be32(&mut b, v);
                }
            }
        }
    }
    let nodes_end = b.len();
    let mut tags: Vec<u8> = Vec::new();
    if node_frames {
        tags.push(10);
    }
    for t in [12u8, 10, 11, 31] {
        if g.chance(2) && !tags.contains(&t) {
            tags.push(t);
        }
    }
    if g.chance(8) {
        tags.push(g.pick(&[7u8, 99, 3]));
    }
    if g.chance(5) && tags.len() > 1 && !node_frames {
        let a = g.below(tags.len() as u64) as usize;
        let c = g.below(tags.len() as u64) as usize;
        tags.swap(a, c);
    }
    for t in tags {
        match t {
            12 => {
                be32(&mut b, 12);
                for _ in 0..n {
                    let l = g.pick(&[0, 0, 2, 3, 41, 17, 77, -4, 99_999]);
                    be32(&mut b, l);
                }
            }
            10 => {
                be32(&mut b, 10);
                let im = vol_geom(g, node_frames);
                b.extend_from_slice(&im);
                let at = vol_geom(g, false);
                b.extend_from_slice(&at);
            }
            11 => {
                be32(&mut b, 11);
                be32(&mut b, g.pick(&[2, 2, 1, 1, 0, 7]));
            }
            31 => {
                be32(&mut b, 31);
                // znzReadMatrix: a tag (0, with a long length) and the text
                be32(&mut b, 0);
                b.extend_from_slice(&1600i64.to_be_bytes());
                let m = matrix_text(g);
                b.extend_from_slice(&m);
            }
            k => {
                let data: Vec<u8> = (0..g.int(0, 12)).map(|_| g.below(256) as u8).collect();
                tag_long(&mut b, i32::from(k), &data);
            }
        }
    }
    if g.chance(2) {
        be32(&mut b, 0);
    }
    (b, nodes_end)
}

/// A gzip stream of `b` in stored blocks (`gzip -0`), the first ending at `split` (or at 65,535
/// bytes); with `damage`, the block after `split` has a wrong length check (zlib's `invalid stored
/// block lengths`).
pub(crate) fn gz_stored(b: &[u8], split: Option<usize>, damage: bool) -> Vec<u8> {
    let mut o = vec![0x1f, 0x8b, 8, 0, 0, 0, 0, 0, 0, 3];
    let mut blocks: Vec<&[u8]> = Vec::new();
    let mut rest = b;
    if let Some(s) = split {
        let s = s.min(rest.len());
        blocks.push(&rest[..s]);
        rest = &rest[s..];
    }
    while !rest.is_empty() {
        let k = rest.len().min(65535);
        blocks.push(&rest[..k]);
        rest = &rest[k..];
    }
    if blocks.is_empty() {
        blocks.push(&[]);
    }
    let nb = blocks.len();
    for (i, block) in blocks.iter().enumerate() {
        o.push(u8::from(i + 1 == nb));
        let n = block.len() as u16;
        o.extend_from_slice(&n.to_le_bytes());
        let check = if damage && i == 1 { n } else { !n };
        o.extend_from_slice(&check.to_le_bytes());
        o.extend_from_slice(block);
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

/// An MGH warp volume (`Warpfield::write`'s layout, compressed): three float frames, the warp's
/// version or another, the morph's metadata, geometries, affine and labels as tags.
pub(crate) fn warp_mgz(g: &mut Gen) -> Vec<u8> {
    let mut b = Vec::new();
    let version = if g.chance(6) { 1 } else { (3 << 8) | 1 };
    be32(&mut b, version);
    let (w, h, d) = (g.int(1, 4) as i32, g.int(1, 4) as i32, g.int(1, 3) as i32);
    for v in [w, h, d, 3, 3, 0] {
        be32(&mut b, v);
    }
    b.extend_from_slice(&1i16.to_be_bytes());
    for k in 0..15 {
        let v = match k {
            0..=2 => g.pick(&[1.0, 2.0]),
            3 | 7 | 11 => g.pick(&[1.0, -1.0]),
            4..=10 => 0.0,
            _ => g.int(-8, 8) as f32,
        };
        bef(&mut b, v);
    }
    b.extend(std::iter::repeat_n(0u8, 256 - 62));
    let fmt = g.pick(&[0, 1, 2, 3, -1, 7]);
    for f in 0..3 {
        for _z in 0..d {
            for _y in 0..h {
                for _x in 0..w {
                    let v = special(g).unwrap_or_else(|| g.int(-16, 16) as f32 / 4.0 + f as f32);
                    // MGH voxels are big-endian
                    bef(&mut b, v);
                }
            }
        }
    }
    // TR, flip angle, TE, TI, field of view
    for _ in 0..5 {
        bef(&mut b, 0.0);
    }
    if !g.chance(8) {
        let mut m = Vec::new();
        be32(&mut m, fmt);
        be32(&mut m, g.pick(&[1, 2, 0, -3]));
        bef(&mut m, g.pick(&[20.0, 0.0, 5.5]));
        tag_long(&mut b, 13, &m);
    }
    if !g.chance(5) {
        be32(&mut b, 10);
        let im = vol_geom(g, true);
        b.extend_from_slice(&im);
        let at = vol_geom(g, true);
        b.extend_from_slice(&at);
    }
    if g.chance(3) {
        let m = matrix_text(g);
        tag_long(&mut b, 14, &m);
    }
    if g.chance(3) {
        be32(&mut b, 12);
        for _ in 0..(w * h * d) {
            be32(&mut b, g.pick(&[0, 2, 41]));
        }
    }
    let mut e = GzEncoder::new(Vec::new(), Compression::new(6));
    e.write_all(&b).unwrap();
    e.finish().unwrap()
}

fn read_case(g: &mut Gen, entries: &mut Vec<String>, env: &mut i32) -> (String, i64) {
    // the frame first: the node coordinates need a small valid image
    let frame: Option<i32> = match g.below(16) {
        0..=6 => None,
        7 => Some(g.pick(&[7, 8, 9])),
        8 => Some(g.pick(&[11, 12, 25])),
        _ => Some(g.int(0, 24) as i32),
    };
    let node_frames = matches!(frame, Some(7..=9));
    if g.chance(14) {
        let b = warp_mgz(g);
        entries.push(file_entry("w/a.mgz", &b));
        let name = match frame {
            None => "w/a.mgz@m3z".to_owned(),
            Some(f) => format!("w/a.mgz@m3z#{f}"),
        };
        return (name, -1);
    }
    let (name, ty) = g.pick(&[
        ("w/a.m3z", -1i64),
        ("w/a.m3z", -1),
        ("w/a.m3z", 29),
        ("w/a.m3d", -1),
        ("w/a.m3d", -1),
        ("w/a.dat@m3z", -1),
    ]);
    let file = name.split('@').next().unwrap_or(name);
    let gz = file.ends_with(".m3z");
    // the node-coordinate frames of an image of 256 x 256 x 256 voxels (no geometry read) take
    // FreeSurfer minutes: their morphs are whole, their geometry tag first, their tags read
    let fault = if node_frames { 40 } else { g.below(40) };
    match fault {
        0 => entries.push(dir_entry(file)),
        1 => {}
        2 => entries.push(file_entry(file, b"not a morph at all")),
        3 => entries.push(file_entry(file, b"")),
        4..=6 => {
            let (mut b, _) = m3z_bytes(g, node_frames);
            let cut = g.below(b.len() as u64 + 1) as usize;
            b.truncate(cut);
            let b = if gz { gz_stored(&b, None, false) } else { b };
            entries.push(file_entry(file, &b));
        }
        7 if gz => {
            let (b, nodes_end) = m3z_bytes(g, node_frames);
            entries.push(file_entry(file, &gz_stored(&b, Some(nodes_end), true)));
        }
        _ => {
            let (b, nodes_end) = m3z_bytes(g, node_frames);
            let b = if gz {
                gz_stored(&b, g.chance(2).then_some(nodes_end), false)
            } else {
                b
            };
            entries.push(file_entry(file, &b));
        }
    }
    if g.chance(10) && !node_frames {
        *env |= 1; // FS_SKIP_TAGS
    }
    if g.chance(5) {
        *env |= 512; // SHOW_NEG
    }
    let name = match frame {
        None => name.to_owned(),
        Some(f) => format!("{name}#{f}"),
    };
    (name, ty)
}

pub fn generate(g: &mut Gen, kind: &str, _portable: bool) -> Vec<String> {
    let mut t = vec![kind.to_owned()];
    let mut entries = Vec::new();
    let mut env = 0;
    let (name, ty) = read_case(g, &mut entries, &mut env);
    t.push(
        g.pick(&["0", "0", "0", "40", "10000040", "10000048"])
            .into(),
    ); // Gdiag
    t.push(if g.chance(12) { "0" } else { "1" }.into()); // MRIIO_Strip_Pound
    t.push(env.to_string()); // piece 6's environment bits (512: SHOW_NEG)
    t.push(ty.to_string());
    t.push(enc(Some(name.as_bytes())));
    t.push(if entries.is_empty() {
        "-".into()
    } else {
        entries.join(",")
    });
    t
}

/// The port's answer to a piece-31 case: piece 6's `mrd`/`mrh`.
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
    let mut t: Vec<&str> = vec![if tk[0] == "m3h" { "mrh" } else { "mrd" }];
    t.extend_from_slice(&tk[1..]);
    let r = super::piece6::run_env(&t, g, out, err, o, w, extra, &[]);
    for n in g.notices.as_slice() {
        // the machine's memory, a reading that never ends, memory contents or a crash
        if matches!(n.defect.id, "FS-851" | "FS-855")
            || (n.defect.id == "FS-854" && !g.notices.reproduce())
        {
            extra.push("SKIP");
        }
        // FreeSurfer's tags after damaged compressed data end as memory contents decide
        if n.defect.id == "FS-854" && g.notices.reproduce() {
            extra.push("VARY");
        }
    }
    r
}
