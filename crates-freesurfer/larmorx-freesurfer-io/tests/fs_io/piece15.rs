// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The fs.io probe's piece-15 cases (track A relay 25): `nrd` and `nrh`, `mri_read` with voxels
//! and `MRIreadHeader` of NIfTI-1 files (`niiRead`: `.nii`, `.nii.gz`; `nifti1Read`: the
//! `.hdr`/`.img` pair), after a `Gdiag_no` (`niiPrintHdr` above 0), then as piece 6's `mrd` and
//! `mrh` (the volume printed with piece 6's printing). The files: headers built field by field
//! (both byte orders; every data type FreeSurfer meets, scaled or not; extents past `dim[0]`,
//! `dim[1]` of -1 with `glmin`, the ico7 shape; units, slice codes, the forms' codes, quaternions
//! with NaNs, odd voxel offsets; texts without NUL bytes), extensions (none, another program's,
//! odd codes, FreeSurfer's with random tags, the file ending at the extender), voxels cut short
//! or doubled; and files written by the port's `niiWrite` (FreeSurfer's extension with every tag,
//! checked by both readers against the sform or qform), then changed.

use std::io::Write;

use flate2::Compression;
use flate2::write::GzEncoder;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_io::mriio_write;

use super::piece4::file_entry;
use super::piece10::Toks;
use super::{Gen, Out, Work, enc};

/// The piece-15 commands.
pub const KINDS: [&str; 2] = ["nrd", "nrh"];

// ---------------------------------------------------------------- generation

fn le16(b: &mut [u8], o: usize, v: i16, big: bool) {
    let x = if big {
        v.to_be_bytes()
    } else {
        v.to_le_bytes()
    };
    b[o..o + 2].copy_from_slice(&x);
}
fn le32(b: &mut [u8], o: usize, v: i32, big: bool) {
    let x = if big {
        v.to_be_bytes()
    } else {
        v.to_le_bytes()
    };
    b[o..o + 4].copy_from_slice(&x);
}
fn lef(b: &mut [u8], o: usize, v: f32, big: bool) {
    le32(b, o, v.to_bits() as i32, big);
}

/// A float of a header: usual values, now and then an odd one (NaNs with payloads, infinities,
/// zeros, a subnormal).
fn hfloat(g: &mut Gen, usual: &[f32]) -> f32 {
    match g.below(14) {
        0 => f32::from_bits(g.pick(&[0x7fc0_0001u32, 0xff80_0003, 0x7f80_0010])),
        1 => g.pick(&[f32::INFINITY, -0.0, 1.0e-40, -3.5]),
        _ => g.pick(usual),
    }
}

/// The bytes a voxel of NIfTI type `dt` takes in FreeSurfer's readers (0: not read).
fn dt_size(dt: i16) -> usize {
    match dt {
        2 | 256 => 1,
        4 | 512 => 2,
        8 | 16 | 768 => 4,
        32 | 64 => 8,
        _ => 0,
    }
}

/// Random voxel bytes of type `dt`: floats and doubles with odd values among the usual ones.
fn voxels(g: &mut Gen, dt: i16, n: usize, big: bool) -> Vec<u8> {
    let mut b = Vec::with_capacity(n * 8);
    for _ in 0..n {
        match dt {
            16 | 32 => {
                let k = if dt == 32 { 2 } else { 1 };
                for _ in 0..k {
                    let v = if g.chance(10) {
                        f32::from_bits(g.pick(&[
                            0x7f80_0001u32,
                            0xffc0_0002,
                            0x7f80_0000,
                            0x8000_0000,
                            0x0000_0001,
                        ]))
                    } else {
                        g.int(-400, 400) as f32 / 8.0
                    };
                    let x = if big {
                        v.to_be_bytes()
                    } else {
                        v.to_le_bytes()
                    };
                    b.extend_from_slice(&x);
                }
            }
            64 => {
                let v = if g.chance(10) {
                    f64::from_bits(g.pick(&[
                        0x7ff0_0000_0000_0001u64,
                        0xfff8_0000_2000_0003,
                        0x47ef_ffff_e000_0000,
                        0x3690_0000_0000_0000,
                        0x7ff0_0000_0000_0000,
                    ]))
                } else {
                    g.int(-4000, 4000) as f64 / 3.0
                };
                let x = if big {
                    v.to_be_bytes()
                } else {
                    v.to_le_bytes()
                };
                b.extend_from_slice(&x);
            }
            _ => {
                for _ in 0..dt_size(dt).max(1) {
                    b.push(g.below(256) as u8);
                }
            }
        }
    }
    b
}

/// A text field of `n` bytes: empty, a short text, or text to the field's end without a NUL.
fn text(g: &mut Gen, b: &mut [u8], o: usize, n: usize) {
    match g.below(8) {
        0 => {
            let t = b"FreeSurfer probe";
            let k = t.len().min(n);
            b[o..o + k].copy_from_slice(&t[..k]);
        }
        1 => b[o..o + n].fill(b'x'),
        _ => {}
    }
}

/// A header built field by field, with its extension block and voxels: the file's bytes (a pair's
/// header alone in the first, its voxels in the second).
fn handmade(g: &mut Gen, pair: bool) -> (Vec<u8>, Vec<u8>) {
    let big = g.chance(6);
    let mut h = vec![0u8; 348];
    le32(&mut h, 0, g.pick(&[348, 348, 348, 348, 540, 0]), big);
    text(g, &mut h, 4, 10);
    text(g, &mut h, 14, 18);
    if g.chance(10) {
        le32(&mut h, 32, 16384, big);
        h[38] = b'r';
    }
    let dim0: i16 = g.pick(&[3, 3, 3, 3, 4, 4, 4, 4, 2, 2, 1, 5, 5, 0, 6, -3]);
    let mut dims = [1i16; 8];
    dims[0] = dim0;
    for d in dims.iter_mut().take(4).skip(1) {
        *d = g.int(1, 4) as i16;
    }
    let mut glmin = g.int(-2, 6) as i32;
    match g.below(14) {
        0 => {
            dims[1] = g.pick(&[-1, 0]);
            // a big-endian header's glmin is read unswapped (FS-643): 0, whose bytes read the same
            glmin = if big { 0 } else { g.int(0, 5) as i32 };
        }
        1 => {
            // the ico7 surface as FreeSurfer writes it
            dims[1] = 27307;
            dims[2] = 1;
            dims[3] = 6;
        }
        2 if !big => {
            dims[1] = -1;
            glmin = 163842;
            dims[2] = 1;
            dims[3] = 1;
        }
        _ => {}
    }
    // the extents past dim[0] (FS-458): 1 as writers leave them, now and then 0 or 2
    for (i, d) in dims.iter_mut().enumerate().skip(2).take(2) {
        if i as i16 > dim0 {
            *d = if g.chance(3) { g.pick(&[0, 2]) } else { 1 };
        }
    }
    dims[4] = if dim0 >= 4 {
        g.pick(&[1, 1, 2, 3, 0, -1])
    } else {
        g.pick(&[1, 1, 1, 0, 2])
    };
    dims[5] = if dim0 >= 5 { g.pick(&[1, 2, 0, -1]) } else { 1 };
    for (i, d) in dims.iter().enumerate() {
        le16(&mut h, 40 + 2 * i, *d, big);
    }
    if g.chance(10) {
        for o in [56, 60, 64] {
            lef(&mut h, o, hfloat(g, &[0.0, 1.0, 2.5]), big);
        }
    }
    let intent: i16 = if g.chance(10) { 1006 } else { 0 };
    le16(&mut h, 68, intent, big);
    let dt: i16 = g.pick(&[
        2, 2, 2, 4, 4, 4, 8, 8, 16, 16, 16, 16, 64, 64, 256, 512, 512, 768, 768, 32, 32, 0, 128,
        1024,
    ]);
    le16(&mut h, 70, dt, big);
    le16(&mut h, 72, (dt_size(dt) * 8) as i16, big);
    lef(&mut h, 76, g.pick(&[1.0, 1.0, -1.0, 0.0]), big);
    for i in 1..4 {
        lef(
            &mut h,
            76 + 4 * i,
            hfloat(g, &[1.0, 1.0, 2.0, 0.5, 1.25, 3.0]),
            big,
        );
    }
    lef(&mut h, 92, hfloat(g, &[0.0, 2.0, 2000.0, 0.8]), big);
    let (slope, inter) = match g.below(10) {
        0..=3 => (0.0, 0.0),
        4 => (1.0, 0.0),
        5 => (1.0, g.pick(&[0.0, 5.0])),
        _ => (
            hfloat(g, &[2.0, 0.5, -1.5, 1.0e-3, 3.0]),
            hfloat(g, &[0.0, 1.0, -10.0, 0.25]),
        ),
    };
    lef(&mut h, 112, slope, big);
    lef(&mut h, 116, inter, big);
    if g.chance(8) {
        h[39] = g.pick(&[0x10, 0x20, 0x30, 0x01, 0x39]);
        h[122] = g.pick(&[1, 2, 3, 4, 5, 6, 7, 9]);
        lef(&mut h, 132, g.pick(&[0.0, 0.05, -1.0]), big);
    }
    h[123] = g.pick(&[
        0,
        2,
        2,
        2,
        10,
        10,
        10,
        18,
        26,
        1,
        3,
        9,
        11,
        34,
        42,
        4,
        0x0a | 0x40,
    ]);
    if g.chance(6) {
        lef(&mut h, 124, 255.0, big);
        lef(&mut h, 136, 1.5, big);
        le32(&mut h, 140, 7, big);
    }
    le32(&mut h, 144, glmin, big);
    text(g, &mut h, 148, 80);
    text(g, &mut h, 228, 24);
    let (qc, sc): (i16, i16) = g.pick(&[(0, 0), (1, 0), (1, 0), (0, 1), (2, 2), (1, 4), (-1, 0)]);
    le16(&mut h, 252, qc, big);
    le16(&mut h, 254, sc, big);
    // the quaternion: a rotation's (one of eight axis turns), random, or with NaNs
    let q: [f32; 3] = match g.below(6) {
        0 => [0.0, 0.0, 0.0],
        1 => [0.0, 0.0, 1.0],
        2 => [0.5, -0.5, 0.5],
        3 => [
            g.int(-8, 8) as f32 / 8.0,
            g.int(-8, 8) as f32 / 8.0,
            g.int(-8, 8) as f32 / 8.0,
        ],
        4 => [
            hfloat(g, &[0.25, -0.1]),
            hfloat(g, &[0.0, 0.3]),
            hfloat(g, &[0.7, -0.9]),
        ],
        _ => [0.0, 0.707_106_77, 0.0],
    };
    for (i, v) in q.iter().enumerate() {
        lef(&mut h, 256 + 4 * i, *v, big);
    }
    for i in 0..3 {
        lef(
            &mut h,
            268 + 4 * i,
            hfloat(g, &[0.0, -90.0, 126.0, 72.5]),
            big,
        );
    }
    // the sform's rows: axes with sizes and shears, or odd numbers
    for r in 0..3 {
        for c in 0..4 {
            let v = if c == 3 {
                hfloat(g, &[0.0, -100.0, 64.0, 12.75])
            } else if g.chance(3) {
                hfloat(g, &[0.0, 0.5, -1.0, 2.0, 0.1])
            } else if r == c {
                g.pick(&[1.0, -1.0, 2.0])
            } else {
                0.0
            };
            lef(&mut h, 280 + 16 * r + 4 * c, v, big);
        }
    }
    if g.chance(6) {
        h[328..340].copy_from_slice(b"displacement");
    }
    let magic: &[u8; 4] = if g.chance(20) {
        g.pick(&[b"n+2\0", b"ni1 ", b"NI1\0"])
    } else if pair == !g.chance(30) {
        b"ni1\0"
    } else {
        b"n+1\0"
    };
    h[344..348].copy_from_slice(magic);

    // the voxels FreeSurfer reads (its own extents), at most a few thousand
    let ncols = if dims[1] > 0 {
        i64::from(dims[1])
    } else {
        i64::from(glmin)
    };
    let nsl = if dim0 < 4 {
        1
    } else {
        i64::from(dims[4].max(1))
    };
    let nf = nsl
        * if dim0 > 4 && dims[5] > 0 {
            i64::from(dims[5])
        } else {
            1
        };
    let count =
        (ncols.max(0) * i64::from(dims[2].max(1)) * i64::from(dims[3].max(1)) * nf.max(0)) as usize;
    let mut data = if count > 4096 {
        vec![0u8; count * dt_size(dt).max(1)]
    } else {
        voxels(g, dt, count, big)
    };
    match g.below(12) {
        0 => {
            let k = g.int(0, data.len() as i64) as usize;
            data.truncate(k);
        }
        1 => {
            let more = data.clone();
            data.extend_from_slice(&more);
        }
        2 => data.extend_from_slice(&[7u8; 5]),
        _ => {}
    }
    if pair {
        if g.chance(12) {
            h.truncate(g.int(0, 347) as usize);
        }
        return (h, data);
    }
    // the extension block: the extender, then extensions
    let mut ext: Vec<u8> = Vec::new();
    let mut ends_early = false;
    match g.below(20) {
        0..=8 => ext.extend_from_slice(&[0, 0, 0, 0]),
        9 => ends_early = true,
        10..=12 => {
            // another program's extension (AFNI's text), now and then FreeSurfer's after it
            ext.extend_from_slice(&[1, 0, 0, 0]);
            let body = b"<?xml version='1.0' ?>\n<AFNI_attributes/>\n";
            let esize = (8 + body.len()).div_ceil(16) * 16;
            let mut e = vec![0u8; esize];
            le32(&mut e, 0, esize as i32, big);
            le32(&mut e, 4, g.pick(&[4, 6, 2, 0]), big);
            e[8..8 + body.len()].copy_from_slice(body);
            ext.extend_from_slice(&e);
            if g.chance(2) {
                ext.extend_from_slice(&fs_extension(g, big));
            }
        }
        13..=14 => {
            // odd codes and sizes
            ext.extend_from_slice(&[1, 0, 0, 0]);
            let mut e = vec![0u8; 16];
            le32(&mut e, 0, g.pick(&[16, 0, -16, 24, 1 << 30]), big);
            le32(&mut e, 4, g.pick(&[3, 15, 100, -2, 14]), big);
            ext.extend_from_slice(&e);
        }
        15..=17 => {
            ext.extend_from_slice(&[1, 0, 0, 0]);
            ext.extend_from_slice(&fs_extension(g, big));
        }
        _ => ext.extend_from_slice(&[1, 0, 0, 0]),
    }
    let vox = 348 + ext.len();
    let off: f32 = match g.below(16) {
        0 => 348.0,
        1 => 0.0,
        2 => f32::NAN,
        3 => -16.0,
        4 => 1.0e30,
        5 => vox as f32 + 0.75,
        6 => (vox + 64) as f32,
        _ => vox as f32,
    };
    lef(&mut h, 108, off, big);
    let mut b = h;
    if ends_early {
        b.extend_from_slice(&[1, 0][..g.int(0, 2) as usize]);
        return (b, Vec::new());
    }
    b.extend_from_slice(&ext);
    let start = if off.is_nan() || off < 0.0 || off > 1.0e9 {
        vox
    } else {
        off as usize
    };
    b.resize(start.max(b.len()), 0);
    b.truncate(start);
    b.extend_from_slice(&data);
    (b, Vec::new())
}

/// FreeSurfer's extension (code 14): its 4-byte header (an endian mark, an intent, a version) and
/// random tags in the extension's form (an `int` tag, a `long long` length, the data), padded.
fn fs_extension(g: &mut Gen, big: bool) -> Vec<u8> {
    let mut body = vec![b'>', 0, g.pick(&[0, 1, 3, 9]), 1];
    for _ in 0..g.int(0, 3) {
        let tag: i32 = g.pick(&[3, 10, 30, 32, 42, 43, 44, 45, 0, 99]);
        let len = g.int(0, 24) as usize;
        body.extend_from_slice(&tag.to_be_bytes());
        body.extend_from_slice(&(len as i64).to_be_bytes());
        for _ in 0..len {
            // a command line's text in ASCII (the port keeps command lines as text: NOTES, relay 25)
            body.push(if tag == 3 {
                b'a' + g.below(26) as u8
            } else {
                g.below(256) as u8
            });
        }
    }
    let esize = (8 + body.len()).div_ceil(16) * 16;
    let mut e = vec![0u8; esize];
    le32(&mut e, 0, esize as i32, big);
    le32(&mut e, 4, 14, big);
    e[8..8 + body.len()].copy_from_slice(&body);
    e
}

/// A file written by the port's `niiWrite` from a piece-14 volume (FreeSurfer's extension with
/// its tags), then changed in its header, cut or not. `None` when the writer refuses the volume.
fn written(g: &mut Gen) -> Option<Vec<u8>> {
    let mut t = vec![];
    let (_, _, _, _, _, alloc) = super::piece14::volume14(g, &mut t, false, true);
    if !alloc {
        return None;
    }
    let toks: Vec<&str> = t.iter().map(String::as_str).collect();
    let mut gl = Globals::new(larmorx_core::defects::UpstreamDefects::Fix);
    let (mut o, mut e) = (Vec::new(), Vec::new());
    let mut m = super::piece14::read_mri14(&mut gl, &mut o, &mut e, &mut Toks { t: &toks, i: 0 });
    use std::sync::atomic::{AtomicUsize, Ordering};
    static N: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "fsio15-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).ok()?;
    let env = Environment::default();
    let intent = g.pick(&[-1, -1, 0, 1, 3]);
    let r = mriio_write::nii_write(
        &mut gl,
        &mut o,
        &mut e,
        &env,
        Some(&dir),
        &mut m,
        "o.nii",
        intent,
    );
    let bytes = std::fs::read(dir.join("o.nii")).ok();
    let _ = std::fs::remove_dir_all(&dir);
    if !matches!(r, Ok(0)) {
        return None;
    }
    let mut b = bytes?;
    if b.len() < 352 {
        return None;
    }
    // changes, now and then two
    for _ in 0..g.int(0, 2) {
        match g.below(12) {
            0 => le16(&mut b, 254, 0, false),
            1 => {
                le16(&mut b, 252, 0, false);
                le16(&mut b, 254, 0, false);
            }
            2 => {
                lef(&mut b, 112, 2.0, false);
                lef(&mut b, 116, 1.0, false);
            }
            3 => b[123] = g.pick(&[1, 3, 9, 26, 0]),
            4 => le16(&mut b, 48, 0, false),
            5 => le16(&mut b, 70, g.pick(&[2, 4, 64, 768]), false),
            6 => {
                // a cut file: in the voxels, the extension, or the extender
                let k = g.int(345, b.len() as i64) as usize;
                b.truncate(k);
            }
            7 => b[348] = 0,
            8 => le16(&mut b, 68, 1006, false),
            9 => lef(&mut b, 260, g.pick(&[0.25f32, f32::NAN]), false),
            10 => {
                // the RAS transform's sizes against the header's: a small change
                let v = f32::from_le_bytes([b[280], b[281], b[282], b[283]]);
                lef(&mut b, 280, v * 1.000_01, false);
            }
            _ => {}
        }
    }
    Some(b)
}

fn gz(b: &[u8]) -> Vec<u8> {
    let mut e = GzEncoder::new(Vec::new(), Compression::new(6));
    e.write_all(b).unwrap();
    e.finish().unwrap()
}

/// The case's name and setup.
fn nifti_case(g: &mut Gen) -> (String, String) {
    let container = g.below(20);
    let mut entries = Vec::new();
    let name: String = if container < 15 {
        let compressed = container >= 11;
        let mut b = if g.chance(3) {
            written(g).unwrap_or_else(|| handmade(g, false).0)
        } else {
            handmade(g, false).0
        };
        let file = if compressed {
            b = gz(&b);
            if g.chance(15) {
                let k = g.int(10, b.len() as i64) as usize;
                b.truncate(k);
            }
            "w/a.nii.gz"
        } else {
            g.pick(&["w/a.nii", "w/a.nii", "w/a.nii", "w/A.NII", "w/a.nii.Z"])
        };
        let name = format!("{file}{}", g.pick(&["", "", "", "", "#0", "@nii"]));
        entries.push(file_entry(file, &b));
        name
    } else {
        let (h, d) = handmade(g, true);
        entries.push(file_entry("w/a.hdr", &h));
        if !g.chance(10) {
            entries.push(file_entry("w/a.img", &d));
        }
        // `.img` names are ANALYZE names to mri_read (FreeSurfer's analyzeRead, not ported yet)
        g.pick(&[
            "w/a.hdr",
            "w/a.hdr",
            "w/a.hdr",
            "w/a",
            "w/a",
            "w/a.img",
            "w/a.hdr#0",
        ])
        .to_owned()
    };
    (name, entries.join(","))
}

pub fn generate(g: &mut Gen, kind: &str, _portable: bool) -> Vec<String> {
    let mut t = vec![kind.to_owned()];
    t.push(
        if g.chance(8) {
            g.pick(&["1", "5"])
        } else {
            "-1"
        }
        .into(),
    );
    t.push(
        if g.chance(8) {
            g.pick(&["10000", "40", "10000040"])
        } else {
            "0"
        }
        .into(),
    );
    t.push(if g.chance(12) { "0" } else { "1" }.into());
    let mut bits = 0;
    if g.chance(12) {
        bits |= 1;
    }
    if g.chance(6) {
        bits |= 4;
    }
    if g.chance(8) {
        bits |= 8;
    }
    t.push(bits.to_string());
    t.push(g.pick(&[-1, -1, -1, -1, -1, 24, 20, 3]).to_string());
    let (name, spec) = nifti_case(g);
    t.push(enc(Some(name.as_bytes())));
    t.push(spec);
    t
}

// ---------------------------------------------------------------- the port's answers

/// The port's answer to a piece-15 case: `Gdiag_no`, then piece 6's `mrd` or `mrh`.
pub fn run(
    tk: &[&str],
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    o: &mut Out,
    w: &Work,
    extra: &mut Vec<&'static str>,
) -> Result<(), u8> {
    g.gdiag_no = tk[1].parse().unwrap();
    let mut t: Vec<&str> = vec![if tk[0] == "nrd" { "mrd" } else { "mrh" }];
    t.extend_from_slice(&tk[2..]);
    let r = super::piece6::run(&t, g, out, err, o, w, extra);
    for n in g.notices.as_slice() {
        // FreeSurfer reads memory there: the extender past the file's end, a text past the header
        if matches!(n.defect.id, "FS-641" | "FS-642") {
            extra.push("VARY");
        }
        // FreeSurfer crashes there
        if n.defect.id == "FS-644" {
            extra.push("SKIP");
        }
    }
    r
}
