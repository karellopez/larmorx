// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Piece 20 (track A relay 28): GE Signa and GE XIMG series. `srd` and `srh` (`mri_read` with
//! voxels and `MRIreadHeader`, as piece 6's `mrd` and `mrh`, the type -1, 14 or 19): Signa series
//! (a 14,336-byte header of 16-bit integers and Data General floats, every field FreeSurfer reads,
//! image counts and echoes of every kind, slices `I.001` ... beside it, cut or missing) and XIMG
//! series (Genesis files with XIMG's fixed offsets, `I.<nnn>` or `<stem><n>.MR`), after piece 6's
//! environment bits 64 `GE_ODD`, 128 `GE_EVEN`, 256 `KILLIANY_SWAP`.

use std::io::Write;

use larmorx_freesurfer_core::globals::Globals;

use super::piece4::file_entry;
use super::{Gen, Out, Work, enc, hex};

pub const KINDS: [&str; 2] = ["srd", "srh"];

fn dir_entry(name: &str) -> String {
    format!("D{}", hex(name.as_bytes()))
}

fn put(b: &mut Vec<u8>, at: usize, v: &[u8]) {
    if b.len() < at + v.len() {
        b.resize(at + v.len(), 0);
    }
    b[at..at + v.len()].copy_from_slice(v);
}

/// A Data General float (sign, 7-bit exponent of 16 excess 64, 24-bit fraction) for `v`.
fn dg(v: f64) -> [u8; 4] {
    if v == 0.0 || !v.is_finite() {
        return [0; 4];
    }
    let sign = if v < 0.0 { 0x8000_0000u32 } else { 0 };
    let mut a = v.abs();
    let mut e: i32 = 64;
    while a >= 1.0 {
        a /= 16.0;
        e += 1;
    }
    while a < 1.0 / 16.0 {
        a *= 16.0;
        e -= 1;
    }
    let m = ((a * 16_777_216.0) as u32) & 0x00ff_ffff;
    (sign | ((e.clamp(0, 127) as u32) << 24) | m).to_be_bytes()
}

/// A header float as FreeSurfer's reader gives it (`ghf` divides by 1,000): `v` thousandths now
/// and then raw bytes.
fn dgf(g: &mut Gen, v: f64) -> [u8; 4] {
    if g.chance(15) {
        return g
            .pick(&[[0xffu8, 0xff, 0xff, 0xff], [0x7f, 0x80, 0, 1], [0, 0, 0, 1]])
            .to_owned();
    }
    dg(v * 1000.0)
}

const SEHDR: usize = 2 * 8 * 256;
const IHDR: usize = 2 * 10 * 256;
const HLENGTH: usize = 28 * 256 * 2;

/// A Signa file: its header, then `n` big-endian shorts.
#[allow(clippy::too_many_arguments)]
fn signa_file(
    g: &mut Gen,
    x: i16,
    y: i16,
    imnr1: i16,
    echoes: i16,
    plane: i16,
    n: usize,
) -> Vec<u8> {
    let mut b = vec![0u8; HLENGTH];
    let w = |b: &mut Vec<u8>, base: usize, word: usize, v: &[u8]| put(b, base + 2 * word, v);
    w(&mut b, SEHDR, 138, &plane.to_be_bytes());
    w(&mut b, IHDR, 137, &x.to_be_bytes());
    w(&mut b, IHDR, 138, &y.to_be_bytes());
    let tr = {
        let v = g.int(100, 3000) as f64;
        dgf(g, v)
    };
    w(&mut b, IHDR, 82, &tr);
    let te = {
        let v = g.int(5, 90) as f64;
        dgf(g, v)
    };
    w(&mut b, IHDR, 86, &te);
    let ti = {
        let v = g.int(1, 900) as f64;
        dgf(g, v)
    };
    w(&mut b, IHDR, 88, &ti);
    for (k, word) in [52usize, 54, 56, 58, 60, 62].iter().enumerate() {
        let v = {
            let v = (g.int(-120, 120) as f64) / 1000.0 + if k % 2 == 1 { 0.01 } else { 0.0 };
            dgf(g, v)
        };
        w(&mut b, IHDR, *word, &v);
    }
    let loc = {
        let v = g.int(-50, 50) as f64 / 1000.0;
        dgf(g, v)
    };
    w(&mut b, IHDR, 73, &loc);
    let thick = {
        let v = g.pick(&[1.5, 3.0, 5.0]) / 1000.0;
        dgf(g, v)
    };
    w(&mut b, IHDR, 77, &thick);
    let psiz = {
        let v = g.pick(&[0.9375, 1.0, 2.0]) / 1000.0;
        dgf(g, v)
    };
    w(&mut b, IHDR, 139, &psiz);
    let fov = {
        let v = g.pick(&[0.24, 0.256]);
        dgf(g, v)
    };
    w(&mut b, SEHDR, 151, &fov);
    for word in [153usize, 155, 157] {
        let c = {
            let v = g.int(-20, 20) as f64;
            dgf(g, v)
        };
        w(&mut b, SEHDR, word, &c);
    }
    w(&mut b, SEHDR, 202, &imnr1.to_be_bytes());
    w(&mut b, IHDR, 98, &echoes.to_be_bytes());
    for _ in 0..n {
        b.extend_from_slice(&(g.int(-3000, 3000) as i16).to_be_bytes());
    }
    b
}

/// An XIMG file: Genesis's layout with the image header at 2,308 and the voxels at 8,432.
fn ximg_file(g: &mut Gen, w: i32, h: i32) -> Vec<u8> {
    let ih = 2308usize;
    let mut b = vec![0u8; 8432];
    b[..4].copy_from_slice(b"IMGF");
    put(&mut b, 4, &g.pick(&[8432i32, 8432, 7900]).to_be_bytes());
    put(&mut b, 8, &w.to_be_bytes());
    put(&mut b, 12, &h.to_be_bytes());
    put(&mut b, 148, &g.pick(&[2308i32, 2308, 156]).to_be_bytes());
    put(
        &mut b,
        ih + 28,
        &g.pick(&[1.0f32, 1.5, f32::NAN]).to_be_bytes(),
    );
    put(&mut b, ih + 52, &g.pick(&[0.9375f32, 1.0]).to_be_bytes());
    put(&mut b, ih + 56, &g.pick(&[0.9375f32, 2.0]).to_be_bytes());
    put(&mut b, ih + 198, &(g.int(0, 900_000) as i32).to_be_bytes());
    put(
        &mut b,
        ih + 200,
        &(g.int(0, 3_000_000) as i32).to_be_bytes(),
    );
    put(&mut b, ih + 254, &(g.int(-90, 180) as i16).to_be_bytes());
    let o = [
        g.int(-9, 9) as f32,
        g.int(-9, 9) as f32,
        g.int(-9, 9) as f32,
    ];
    let v = [
        o[0],
        o[1],
        o[2],
        0.0,
        0.0,
        1.0,
        o[0] - 4.0,
        o[1] + 4.0,
        o[2],
        o[0] + 4.0,
        o[1] + 4.0,
        o[2],
        o[0] + 4.0,
        o[1] - 4.0,
        o[2],
    ];
    for (k, x) in v.iter().enumerate() {
        let x = if g.chance(20) { f32::INFINITY } else { *x };
        put(&mut b, ih + 136 + 4 * k, &x.to_be_bytes());
    }
    let n = (w.max(0) * h.max(0)) as usize;
    if n < 1 << 12 {
        for _ in 0..n {
            b.extend_from_slice(&(g.int(-3000, 3000) as i16).to_be_bytes());
        }
    }
    if g.chance(12) {
        let len = b.len();
        let k = g.below(len as u64) as usize;
        b.truncate(k);
    }
    b
}

pub fn generate(g: &mut Gen, kind: &str, _portable: bool) -> Vec<String> {
    let mut t = vec![kind.to_owned()];
    let mut entries = Vec::new();
    let mut bits = 0;
    if g.chance(5) {
        bits |= g.pick(&[64, 128, 192]);
    }
    let (name, ty) = if g.chance(3) {
        // XIMG
        if g.chance(6) {
            bits |= 256;
        }
        let (w, h) = (g.int(1, 4) as i32, g.int(1, 3) as i32);
        let lo = g.int(0, 3);
        let hi = lo + g.int(0, 3);
        let mr = g.chance(2);
        let stem = g.pick(&["w/s", "w/7", "w/"]);
        let nm = |n: i64| {
            if mr {
                format!("{stem}{n}.MR")
            } else {
                format!("w/I.{n:03}")
            }
        };
        for n in lo..=hi {
            let b = ximg_file(g, w, h);
            entries.push(file_entry(&nm(n), &b));
        }
        let pick = if g.chance(12) {
            g.pick(&["w/I.", "w/x.mr", "w/nope.MR"]).to_owned()
        } else {
            nm(g.int(lo, hi))
        };
        (pick, if g.chance(5) { -1 } else { 19 })
    } else {
        // Signa: the slices I.001 ... beside the header file
        let x = if g.chance(15) {
            g.pick(&[0i16, -2])
        } else {
            g.int(1, 4) as i16
        };
        let y = if g.chance(15) {
            g.pick(&[0i16, -3])
        } else {
            g.int(1, 3) as i16
        };
        let echoes: i16 = if g.chance(4) {
            g.pick(&[2, 2, 3, 4, -1])
        } else {
            g.pick(&[0, 1])
        };
        let files = g.int(1, if echoes > 1 { 8 } else { 4 }) as i16;
        let imnr1 = if g.chance(10) {
            g.pick(&[-1i16, 0, files + 2])
        } else {
            files
        };
        let plane = g.pick(&[0i16, 1, 2, 3, 300]);
        let n = (x.max(0) as usize) * (y.max(0) as usize);
        let header = signa_file(g, x, y, imnr1, echoes, plane, n);
        for i in 1..=files {
            let name = format!("w/I.{i:03}");
            if i > 1 && g.chance(25) {
                entries.push(dir_entry(&name));
                continue;
            }
            let mut b = if i == 1 {
                header.clone()
            } else {
                let mut b = header[..HLENGTH].to_vec();
                for _ in 0..n {
                    b.extend_from_slice(&(g.int(-3000, 3000) as i16).to_be_bytes());
                }
                b
            };
            if g.chance(12) {
                let len = b.len();
                let k = if g.chance(2) {
                    g.int((HLENGTH - 4) as i64, len as i64) as usize
                } else {
                    g.below(len as u64) as usize
                };
                b.truncate(k);
            }
            entries.push(file_entry(&name, &b));
        }
        let pick = if g.chance(6) {
            // a header file by another name
            entries.push(file_entry("w/hdr", &header));
            "w/hdr".to_owned()
        } else if g.chance(15) {
            "w/nope".to_owned()
        } else {
            format!("w/I.{:03}", g.int(1, files as i64))
        };
        (pick, if g.chance(4) { -1 } else { 14 })
    };
    t.push("0".into());
    t.push(if g.chance(12) { "0" } else { "1" }.into());
    t.push(bits.to_string());
    t.push(ty.to_string());
    t.push(enc(Some(name.as_bytes())));
    t.push(entries.join(","));
    t
}

/// The port's answer to a piece-20 case: piece 6's `mrd`/`mrh`.
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
    let mut t: Vec<&str> = vec![if tk[0] == "srd" { "mrd" } else { "mrh" }];
    t.extend_from_slice(&tk[1..]);
    let r = super::piece6::run_env(&t, g, out, err, o, w, extra, &[]);
    for n in g.notices.as_slice() {
        // FreeSurfer writes past its slice table (FS-686), or a % in the names (FS-682)
        if (n.defect.id == "FS-686" && n.detail.contains("past the table"))
            || n.defect.id == "FS-682"
        {
            extra.push("SKIP");
        }
    }
    r
}
