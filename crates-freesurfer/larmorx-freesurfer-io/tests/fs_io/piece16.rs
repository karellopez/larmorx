// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Piece 16 (track A relay 26): ANALYZE 7.5 volumes. `ard` and `arh` (`mri_read` with voxels and
//! `MRIreadHeader`, as piece 6's `mrd` and `mrh`, after a `Gdiag_no`, `N_Zero_Pad_Input` and
//! `SPM_START_FRAME`) of ANALYZE pairs and SPM series built here field by field (both byte orders,
//! every data type FreeSurfer takes and some it refuses, odd extents, sizes, offsets, scales and
//! orientation codes, `.mat` files of MATLAB's version 4 with a voxel-to-RAS matrix of any shape or
//! piece 5's files, NIfTI-1 pair and single-file headers in the `.hdr`); `awr` (piece 14's `mgw`
//! after a `Gdiag_no`, `N_Zero_Pad_Output` and the read back's `N_Zero_Pad_Input`, writing with
//! `MRIwriteType` as `MRI_ANALYZE_FILE` or `MRI_ANALYZE4D_FILE`, or `MRIwrite` of a `.img` name).

use std::io::Write;

use larmorx_freesurfer_core::globals::Globals;

use super::piece4::file_entry;
use super::piece15::{handmade, hfloat, le16, le32, lef, text, voxels};
use super::{Gen, Out, Work, enc};

pub const KINDS: [&str; 3] = ["ard", "arh", "awr"];

// ---------------------------------------------------------------- generation

/// The bytes a voxel of ANALYZE type `dt` takes in `analyzeRead` (the refused types: 1).
fn dt_bytes(dt: i16) -> usize {
    match dt {
        2 => 1,
        4 | 512 => 2,
        8 | 16 | 768 => 4,
        64 => 8,
        _ => 1,
    }
}

/// An ANALYZE header built field by field: its bytes, type, extents (columns, rows, slices,
/// `dim[4]`), voxel offset and whether its fields are big-endian. A big-endian header has
/// `sizeof_hdr` 348 in its order; a little-endian `sizeof_hdr` other than 348 comes with
/// big-endian fields (FreeSurfer swaps the header whenever `sizeof_hdr` is not 348: the fields
/// then read right, the extents stay small).
fn header(g: &mut Gen) -> (Vec<u8>, i16, [i16; 5], usize, bool) {
    let mut h = vec![0u8; 348];
    let odd = g.chance(10);
    let big = odd || g.chance(6);
    if odd {
        le32(&mut h, 0, g.pick(&[0, 349, 540]), false);
    } else {
        le32(&mut h, 0, 348, big);
    }
    text(g, &mut h, 4, 10);
    text(g, &mut h, 14, 18);
    le32(&mut h, 32, g.pick(&[16384, 16384, 0, 70000, -5]), big);
    le16(&mut h, 36, g.int(-3, 3) as i16, big);
    h[38] = g.pick(&[b'r', b'r', 0, b'x']);
    h[39] = g.pick(&[0u8, 0, b'q', 200]);
    let mut dims = [4i16, 1, 1, 1, 1];
    dims[0] = g.pick(&[4, 4, 4, 3, 3, 2, 0, 9]);
    for d in dims.iter_mut().take(4).skip(1) {
        *d = if g.chance(25) {
            g.pick(&[0, -1, 1])
        } else {
            g.int(1, 4) as i16
        };
    }
    dims[4] = if g.chance(20) {
        -1
    } else {
        g.pick(&[0, 0, 1, 1, 2, 3])
    };
    for (i, d) in dims.iter().enumerate() {
        le16(&mut h, 40 + 2 * i, *d, big);
    }
    for i in 5..8 {
        le16(&mut h, 40 + 2 * i, g.pick(&[0, 1, 1, 7, -2]), big);
    }
    if g.chance(4) {
        h[56..60].copy_from_slice(b"mm\0\0");
    } else if g.chance(8) {
        h[56..60].copy_from_slice(b"msec");
    }
    text(g, &mut h, 60, 8);
    le16(&mut h, 68, g.int(-2, 2) as i16, big);
    let dt: i16 = if g.chance(12) {
        g.pick(&[0, 1, 32, 128, 256, 1536, -4])
    } else {
        g.pick(&[2, 2, 4, 4, 8, 16, 16, 16, 64, 64, 512, 768])
    };
    le16(&mut h, 70, dt, big);
    le16(
        &mut h,
        72,
        if g.chance(6) {
            g.pick(&[0, 3, 64])
        } else {
            8 * dt_bytes(dt) as i16
        },
        big,
    );
    le16(&mut h, 74, g.int(-1, 1) as i16, big);
    lef(&mut h, 76, hfloat(g, &[1.0, 0.0, 4.0]), big);
    for i in 1..4 {
        let v = hfloat(g, &[1.0, 1.0, 2.0, 0.5, -1.5, 3.25, 1.0, -1.0, 0.0]);
        lef(&mut h, 76 + 4 * i, v, big);
    }
    lef(&mut h, 92, hfloat(g, &[0.0, 2.0, 1.5, 0.0, 2.5]), big);
    for i in 5..8 {
        lef(&mut h, 76 + 4 * i, hfloat(g, &[0.0, 1.0, 7.5]), big);
    }
    let off: f32 = if g.chance(8) {
        g.pick(&[16.0, 7.9, -4.0, f32::NAN, 3.0e9, 348.0, 1.0e20, -0.0])
    } else {
        0.0
    };
    lef(&mut h, 108, off, big);
    let scale: f32 = if g.chance(3) {
        g.pick(&[
            0.0,
            2.5,
            0.5,
            -1.0,
            f32::from_bits(0x7fc0_0005),
            1.000_000_1,
            0.999_999_94,
            f32::INFINITY,
            1.0e-9,
            3.0,
        ])
    } else {
        1.0
    };
    lef(&mut h, 112, scale, big);
    for o in [116, 120, 124, 128] {
        lef(&mut h, o, hfloat(g, &[0.0, 1.0, 255.0]), big);
    }
    for o in [132, 136] {
        le32(&mut h, o, g.int(-1, 2) as i32, big);
    }
    le32(&mut h, 140, g.pick(&[0, 255, 4095, -7, 70000]), big);
    le32(&mut h, 144, g.pick(&[0, 0, -3, 12]), big);
    text(g, &mut h, 148, 80);
    text(g, &mut h, 228, 24);
    h[252] = if g.chance(10) {
        g.pick(&[6u8, 9, 0xfe, 127, b'x'])
    } else {
        g.pick(&[0u8, 0, 1, 2, 3, 4, 5, 0xff, 0xff])
    };
    for b in h.iter_mut().take(263).skip(253) {
        *b = g.below(256) as u8;
    }
    for o in (316..348).step_by(4) {
        le32(&mut h, o, g.int(-2, 2) as i32, big);
    }
    // the voxel offset the data file is written at (the reader's conversion decides the rest)
    let at = if (0.0..=400.0).contains(&off) {
        off as usize
    } else {
        0
    };
    (h, dt, dims, at, big)
}

/// A NIfTI header to be read as ANALYZE: `sizeof_hdr` set to 348 in the byte order whose extents
/// are small (FreeSurfer's ANALYZE reader swaps the header when `sizeof_hdr` is not 348).
fn sane(h: &mut [u8]) {
    let small = |big: bool| {
        let d = |i: usize| {
            let b = [h[40 + 2 * i], h[41 + 2 * i]];
            i64::from(if big {
                i16::from_be_bytes(b)
            } else {
                i16::from_le_bytes(b)
            })
        };
        (1..=4).all(|i| (-1..=64).contains(&d(i)))
    };
    let big = !small(false) && small(true);
    if small(false) || big {
        le32(h, 0, 348, big);
    } else {
        h[40..48].fill(0);
        le32(h, 0, 348, false);
    }
}

/// A MATLAB version 4 file of one little-endian matrix of doubles named `M`.
fn v4_matrix(rows: i32, cols: i32, vals: &[f64]) -> Vec<u8> {
    let mut b = Vec::new();
    for v in [0i32, rows, cols, 0, 2] {
        b.extend_from_slice(&v.to_le_bytes());
    }
    b.extend_from_slice(b"M\0");
    for v in vals {
        b.extend_from_slice(&v.to_le_bytes());
    }
    b
}

/// An SPM `.mat` file: a voxel-to-RAS matrix of doubles (column-major), 4 x 4 most of the time,
/// other shapes now and then, or one of piece 5's files (any version, cut or changed).
fn mat_bytes(g: &mut Gen) -> Vec<u8> {
    if g.chance(5) {
        return super::piece5::mat_file(g, true, false);
    }
    let (rows, cols) = if g.chance(5) {
        g.pick(&[(3, 4), (4, 3), (2, 4), (1, 1), (5, 4), (0, 4)])
    } else {
        (4, 4)
    };
    let mut vals = Vec::new();
    for c in 0..cols {
        for r in 0..rows {
            let v = if r == 3 {
                if c == 3 { 1.0 } else { 0.0 }
            } else if c == 3 {
                g.int(-130, 130) as f64 + 0.5
            } else if r == c {
                g.pick(&[-2.0, 2.0, -1.0, 1.0, 0.937_5])
            } else if g.chance(4) {
                g.int(-3, 3) as f64 / 7.0
            } else {
                0.0
            };
            vals.push(v);
        }
    }
    v4_matrix(rows, cols, &vals)
}

/// The data of an ANALYZE header: `n` voxels of its type after `at` zero bytes, now and then cut
/// or longer.
fn data(g: &mut Gen, dt: i16, n: usize, at: usize, big: bool) -> Vec<u8> {
    let mut d = vec![0u8; at];
    d.extend(voxels(g, dt, n, big));
    match g.below(10) {
        0 if !d.is_empty() => {
            let k = g.below(d.len() as u64) as usize;
            d.truncate(k);
        }
        1 => {
            let k = g.int(1, 9) as usize;
            for _ in 0..k {
                d.push(g.below(256) as u8);
            }
        }
        _ => {}
    }
    d
}

/// The voxels an ANALYZE header asks for (its frames as the reader counts them; negative extents
/// none).
fn nvox(dims: [i16; 5], frames: i64) -> usize {
    let p = i64::from(dims[1]) * i64::from(dims[2]) * i64::from(dims[3]) * frames;
    p.clamp(0, 4096) as usize
}

/// A read case's name, type and files: one pair, an SPM series, NIfTI headers in the `.hdr`,
/// missing files.
fn read_case(
    g: &mut Gen,
    nzpad: &mut i32,
    spm: &mut Option<&'static str>,
) -> (String, i32, String) {
    let mut entries = Vec::new();
    let kind = g.below(20);
    let (name, ty): (String, i32) = if kind < 11 {
        // one pair
        let (h, dt, dims, at, big) = header(g);
        let frames = if dims[4] == 0 { 1 } else { i64::from(dims[4]) };
        entries.push(file_entry("w/a.hdr", &h));
        if !g.chance(25) {
            let d = data(g, dt, nvox(dims, frames.max(0)), at, big);
            entries.push(file_entry("w/a.img", &d));
        }
        if g.chance(3) {
            let m = mat_bytes(g);
            entries.push(file_entry("w/a.mat", &m));
        }
        let name = match g.below(14) {
            0 => "w/a",
            1 => "w/a.hdr",
            2 => "w/A.IMG",
            3 => "w/a.img#0",
            4 => "w/a.img@analyze",
            5 => "w/a.img@img",
            6 if g.chance(2) => {
                *nzpad = 3;
                "w/a.img"
            }
            _ => "w/a.img",
        };
        let ty = match name {
            "w/a" | "w/a.hdr" => g.pick(&[2, 16, -1]),
            _ => g.pick(&[-1, -1, 2, 16]),
        };
        (name.into(), ty)
    } else if kind < 15 {
        // an SPM series: <stem><n>.img from the start frame on, one header (the first frame's)
        let pad = g.pick(&[3, 3, 3, 1, 2, 4, 0]);
        *nzpad = pad;
        let start = match g.below(6) {
            0 => {
                *spm = Some("0");
                0
            }
            1 => {
                *spm = Some("2");
                2
            }
            2 => {
                *spm = Some(g.pick(&["1", " 1", "abc", "-1", "2x", "99999999999"]));
                1
            }
            _ => 1,
        };
        let nfr = g.int(1, 3);
        let (h, dt, dims, at, big) = header(g);
        let num = |n: i64| format!("{n:0width$}", width = pad as usize);
        entries.push(file_entry(&format!("w/s{}.hdr", num(start)), &h));
        for f in 0..nfr {
            if f > 0 && g.chance(12) {
                continue;
            }
            let d = data(g, dt, nvox(dims, 1), at, big);
            entries.push(file_entry(&format!("w/s{}.img", num(start + f)), &d));
        }
        if g.chance(3) {
            let m = mat_bytes(g);
            entries.push(file_entry(&format!("w/s{}.mat", num(start)), &m));
        }
        ("w/s".into(), g.pick(&[2, 2, 16, -1]))
    } else if kind < 18 {
        // NIfTI headers in the .hdr: a NIfTI-1 pair (nifti1Read), a single file's header (read
        // as ANALYZE), a series of pairs, or only a .nii beside the .img
        let (mut h, d) = handmade(g, true);
        match g.below(4) {
            0 => {
                let (mut n1, _) = handmade(g, false);
                n1.resize(348.max(n1.len()), 0);
                sane(&mut n1);
                entries.push(file_entry("w/a.hdr", &n1[..348]));
                entries.push(file_entry("w/a.img", &d));
                ("w/a.img".into(), g.pick(&[2, -1]))
            }
            1 => {
                *nzpad = 3;
                h.resize(348.max(h.len()), 0);
                sane(&mut h);
                entries.push(file_entry("w/s001.hdr", &h));
                entries.push(file_entry("w/s001.img", &d));
                entries.push(file_entry("w/s002.img", &d));
                ("w/s".into(), 2)
            }
            2 => {
                let (n1, _) = handmade(g, false);
                entries.push(file_entry("w/a.nii", &n1));
                entries.push(file_entry("w/a.img", &d));
                ("w/a.img".into(), 2)
            }
            _ => {
                entries.push(file_entry("w/a.hdr", &h));
                entries.push(file_entry("w/a.img", &d));
                ("w/a.img".into(), g.pick(&[2, 16, -1]))
            }
        }
    } else {
        // missing files
        if g.chance(2) {
            let (h, _, _, _, _) = header(g);
            entries.push(file_entry("w/a.hdr", &h));
        } else {
            entries.push(file_entry("w/a.img", b"\x01\x02"));
        }
        (g.pick(&["w/a.img", "w/b.img", "w/a"]).into(), 2)
    };
    let spec = if entries.is_empty() {
        "-".into()
    } else {
        entries.join(",")
    };
    (name, ty, spec)
}

pub fn generate(g: &mut Gen, kind: &str, _portable: bool) -> Vec<String> {
    let mut t = vec![kind.to_owned()];
    if kind == "awr" {
        return write_case(g, t);
    }
    let gdiag_no = if g.chance(6) { "1" } else { "-1" };
    let mut nzpad = if g.chance(20) { g.int(0, 2) as i32 } else { -1 };
    let mut spm: Option<&'static str> = if g.chance(20) {
        Some(g.pick(&["1", "0", "abc"]))
    } else {
        None
    };
    let gdiag = if g.chance(8) {
        g.pick(&["10000", "40", "10000040"])
    } else {
        "0"
    };
    let strip = if g.chance(12) { "0" } else { "1" };
    let mut bits = 0;
    if g.chance(8) {
        bits |= 16;
    }
    if g.chance(8) {
        bits |= 32;
    }
    let (name, ty, spec) = read_case(g, &mut nzpad, &mut spm);
    t.push(gdiag_no.into());
    t.push(nzpad.to_string());
    t.push(enc(spm.map(str::as_bytes)));
    t.push(gdiag.into());
    t.push(strip.into());
    t.push(bits.to_string());
    t.push(ty.to_string());
    t.push(enc(Some(name.as_bytes())));
    t.push(spec);
    t
}

/// A write case: `awr` <Gdiag_no> <N_Zero_Pad_Output> <N_Zero_Pad_Input> then piece 14's `mgw`
/// tokens with `how` 3 (`MRI_ANALYZE_FILE`), 4 (`MRI_ANALYZE4D_FILE`) or 1 (`MRIwrite`).
fn write_case(g: &mut Gen, mut t: Vec<String>) -> Vec<String> {
    let how = g.pick(&[3i64, 3, 4, 4, 1]);
    let name = match how {
        3 => g.pick(&[
            "w/o.img",
            "w/o.img",
            "w/o",
            "w/o",
            "w/O.IMG",
            "w/o.hdr",
            "w/no/o.img",
        ]),
        4 => g.pick(&[
            "w/o.img",
            "w/o.img",
            "w/o.img",
            "w/o",
            "w/o.IMG",
            "w/a.b.img",
        ]),
        _ => "w/o.img",
    };
    let nzout = if g.chance(4) { g.int(0, 4) } else { -1 };
    // the read back of a series uses its padding (3 when it was below 0)
    let series = how == 3 && !name.ends_with(".img");
    let nzin = if series {
        if nzout < 0 { 3 } else { nzout }
    } else {
        -1
    };
    t.push(if g.chance(6) { "1" } else { "-1" }.into());
    t.push(nzout.to_string());
    t.push(nzin.to_string());
    t.push(g.pick(&["0", "0", "0", "10000"]).into());
    t.push("-".into());
    t.push(if g.chance(6) { "1" } else { "0" }.into());
    t.push(how.to_string());
    let (_, _, _, _, nf, alloc) = super::piece14::volume14(g, &mut t, true, false);
    t.push("-1".into());
    t.push(g.pick(&[-1i64, -1, 0, 3]).to_string());
    t.push(enc(Some(name.as_bytes())));
    let back = alloc && nf > 0 && g.chance(2);
    t.push(i64::from(back).to_string());
    t
}

// ---------------------------------------------------------------- the port's answers

/// The port's answer to a piece-16 case: `ard`/`arh` set `Gdiag_no`, `N_Zero_Pad_Input` and
/// `SPM_START_FRAME`, then piece 6's `mrd`/`mrh`; `awr` is piece 14's.
pub fn run(
    tk: &[&str],
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    o: &mut Out,
    w: &Work,
    extra: &mut Vec<&'static str>,
) -> Result<(), u8> {
    let r = if tk[0] == "awr" {
        super::piece14::run(tk, g, out, err, o, w, extra)
    } else {
        g.gdiag_no = tk[1].parse().unwrap();
        g.n_zero_pad_input = tk[2].parse().unwrap();
        let spm = super::dec(tk[3]);
        let mut t: Vec<&str> = vec![if tk[0] == "ard" { "mrd" } else { "mrh" }];
        t.extend_from_slice(&tk[4..]);
        let more: Vec<(&str, Vec<u8>)> = spm.map(|v| ("SPM_START_FRAME", v)).into_iter().collect();
        super::piece6::run_env(&t, g, out, err, o, w, extra, &more)
    };
    // MatReadHeader's message prints the stream's and the structure's addresses (DIAG_SHOW and
    // DIAG_VERBOSE), when there is a .mat file
    if tk[0] != "awr" && tk[4] == "10000040" && tk.last().is_some_and(|s| s.contains("2e6d6174")) {
        extra.push("VARY");
    }
    for n in g.notices.as_slice() {
        // the .mat reader's own classes (piece 5): FreeSurfer crashes, reads memory, exits
        if let Some(c) = larmorx_freesurfer_io::matfile::probe_class(n.defect.id, &n.detail) {
            extra.push(c);
        }
        if matches!(n.defect.id, "FS-229" | "FS-13") {
            extra.push("SKIP");
        }
        // FreeSurfer crashes there: an orientation code outside -1 to 5, a .mat of another shape,
        // an SPM series of NIfTI pairs whose first pair fails, a long name
        if matches!(n.defect.id, "FS-648" | "FS-650" | "FS-651" | "FS-646") {
            extra.push("SKIP");
        }
        // FreeSurfer's start frame from memory contents
        if n.defect.id == "FS-645" {
            extra.push("VARY");
        }
    }
    r
}
