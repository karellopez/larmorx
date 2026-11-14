// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Piece 18 (track A relay 27): GDF and Varian SDT volumes. `xrd` and `xrh` (`mri_read` with
//! voxels and `MRIreadHeader`, as piece 6's `mrd` and `mrh`, after the GDF crop flag) of GDF
//! headers line by line (every field, missing, without its value, units, orientations, axes,
//! offsets, crop boxes) with their slice files (plain and zero-padded numbers, cut, missing) and of
//! SDT volumes (`.spr` headers of every line form, a last line without a newline, a folder in its
//! place; voxels of the four types, cut). GDF writes are piece 17's `vwr` with its image stem.

use std::io::Write;

use larmorx_freesurfer_core::globals::Globals;

use super::piece4::file_entry;
use super::{Gen, Out, Work, enc, hex};

pub const KINDS: [&str; 2] = ["xrd", "xrh"];

fn dir_entry(name: &str) -> String {
    format!("D{}", hex(name.as_bytes()))
}

/// A number as a header writes it, now and then in another form.
fn num(g: &mut Gen, v: f64) -> String {
    if !g.chance(25) {
        return format!("{v}");
    }
    g.pick(&["nan", "-inf", "abc", "", "1e40", "-0"]).to_owned()
}

/// `n` big-endian voxels of a type (1 bytes, 2 shorts, 4 ints or floats).
fn voxels(g: &mut Gen, n: usize, bytes: usize, float: bool) -> Vec<u8> {
    let mut b = Vec::new();
    for _ in 0..n {
        match bytes {
            1 => b.push(g.below(256) as u8),
            2 => b.extend_from_slice(&(g.int(-600, 600) as i16).to_be_bytes()),
            _ if float => {
                let v: f32 = if g.chance(30) {
                    g.pick(&[f32::NAN, f32::INFINITY, -0.0])
                } else {
                    g.int(-400, 400) as f32 / 8.0
                };
                b.extend_from_slice(&v.to_be_bytes());
            }
            _ => b.extend_from_slice(&(g.int(-70000, 70000) as i32).to_be_bytes()),
        }
    }
    b
}

/// A GDF header and its slices: the name and type to read.
fn gdf_case(g: &mut Gen, entries: &mut Vec<String>, crop: &mut bool) -> (String, i32) {
    let w = if g.chance(20) { 0 } else { g.int(1, 4) };
    let h = if g.chance(20) { 0 } else { g.int(1, 3) };
    let n = g.int(1, 3);
    let (dt, bytes, float) = match g.below(9) {
        0..=2 => ("short", 2, false),
        3 | 4 => ("float", 4, true),
        5 | 6 => ("unsigned char", 1, false),
        7 => ("Short", 2, false),
        _ => ("int", 4, false),
    };
    let pad = g.chance(4);
    let stem = g.pick(&["w/g_", "w/g_", "w/g"]);
    let offset = if g.chance(5) { g.pick(&[0i64, 4, 16, -2]) } else { 0 };
    let mut lines: Vec<String> = vec!["GDF FILE VERSION3".into(), "START MAIN HEADER".into()];
    lines.push(match g.below(12) {
        0 => "IMAGE_FILE_PATH w/g_.img".into(),
        1 => "IMAGE_FILE_PATH".into(),
        2 => "IMAGE_FILE_PATH w/*/g.img".into(),
        _ => format!("IMAGE_FILE_PATH {stem}*.img"),
    });
    lines.push(format!("SIZE {} {}", num(g, w as f64), num(g, h as f64)));
    let (sx, sy, st) = (
        g.pick(&[1.0, 0.9375, 2.0]),
        g.pick(&[1.0, 0.9375, 2.0]),
        g.pick(&[1.0, 3.0, 1.5]),
    );
    lines.push(format!("IP_RES {} {}", num(g, sx), num(g, sy)));
    lines.push(format!("SL_THICK {}", num(g, st)));
    if !g.chance(4) {
        let u = if g.chance(12) {
            g.pick(&["in", ""])
        } else {
            g.pick(&["mm", "mm", "cm", "m", "MM"])
        };
        lines.push(format!("UNITS {u}"));
    }
    if g.chance(3) {
        let ax: [[f64; 3]; 3] = if g.chance(2) {
            [[-1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0]]
        } else {
            [[0.6, 0.8, 0.0], [0.0, 0.0, -1.0], [0.8, -0.6, 0.0]]
        };
        for (k, a) in ["X", "Y", "Z"].iter().zip(ax) {
            if g.chance(12) {
                continue;
            }
            lines.push(format!(
                "FS_{k}_RAS {} {} {}",
                num(g, a[0]),
                num(g, a[1]),
                num(g, a[2])
            ));
        }
    }
    if g.chance(3) {
        lines.push(format!(
            "FS_C_RAS {} {} {}",
            num(g, 1.5),
            num(g, -2.0),
            num(g, 3.25)
        ));
    }
    if !g.chance(5) {
        lines.push(format!("DATA_TYPE {dt}"));
    } else if g.chance(4) {
        lines.push("DATA_TYPE".into());
    }
    if !g.chance(5) {
        lines.push(format!(
            "ORIENTATION {}",
            if g.chance(12) {
                g.pick(&["oblique", ""])
            } else {
                g.pick(&["coronal", "coronal", "sagittal", "axial", "horizontal", "Coronal", "x cor"])
            }
        ));
    }
    lines.push(format!("FILE_OFFSET {offset}"));
    if g.chance(4) {
        *crop = true;
        lines.push(format!(
            "MIN_CROP {} {} {}",
            num(g, 1.0),
            num(g, 1.0),
            num(g, 0.0)
        ));
        lines.push(format!(
            "MAX_CROP {} {} {}",
            num(g, 2.0),
            num(g, 2.0),
            num(g, 1.0)
        ));
    }
    lines.push("END MAIN HEADER".into());
    if g.chance(8) {
        let k = g.below(lines.len() as u64) as usize;
        lines.remove(k);
    }
    if g.chance(10) {
        // a long line first, then a short DATA_TYPE line reads its bytes
        lines.insert(0, format!("COMMENT {}", "x".repeat(g.int(4, 30) as usize)));
        lines.push("DATA_TYPE".into());
    }
    let mut t = String::new();
    for (i, l) in lines.iter().enumerate() {
        t.push_str(l);
        if i + 1 < lines.len() || !g.chance(8) {
            t.push('\n');
        }
    }
    entries.push(file_entry("w/g.gdf", t.as_bytes()));
    let off = offset.max(0) as usize;
    for i in 1..=n {
        if i > 1 && g.chance(12) {
            continue;
        }
        let mut d = vec![0u8; off];
        d.extend(voxels(g, (w * h) as usize, bytes, float));
        if g.chance(10) && !d.is_empty() {
            let k = g.below(d.len() as u64) as usize;
            d.truncate(k);
        }
        let name = if pad {
            format!("{stem}{i:03}.img")
        } else {
            format!("{stem}{i}.img")
        };
        entries.push(file_entry(&name, &d));
    }
    let ty = if g.chance(4) { 12 } else { -1 };
    (
        if g.chance(15) {
            "w/nope.gdf".into()
        } else {
            "w/g.gdf".into()
        },
        ty,
    )
}

/// An SDT volume: `.spr` header and voxels.
fn sdt_case(g: &mut Gen, entries: &mut Vec<String>) -> (String, i32) {
    let ndim = if g.chance(12) {
        g.pick(&[2i64, 5])
    } else {
        g.pick(&[3i64, 3, 4])
    };
    let (x, y, z) = (g.int(0, 4), g.int(1, 3), g.int(1, 3));
    let (dts, bytes, float) = match g.below(10) {
        0..=2 => ("BYTE", 1, false),
        3 | 4 => ("WORD", 2, false),
        5 | 6 => ("REAL", 4, true),
        7 => ("LWORD", 4, false),
        8 => ("COMPLEX", 8, true),
        _ => ("FOO", 1, false),
    };
    let mut lines: Vec<String> = Vec::new();
    lines.push(format!("numDim: {ndim}"));
    lines.push(match ndim {
        4 => format!("dim: {x} {y} {z} {}", g.pick(&[1, 1, 2])),
        _ => format!("dim: {x} {y} {z}"),
    });
    lines.push(format!("dataType: {dts}"));
    lines.push(format!(
        "interval: {} {} {}{}",
        num(g, 0.1),
        num(g, 0.09375),
        num(g, 0.3),
        if ndim == 4 { " 1" } else { "" }
    ));
    lines.push(format!(
        "sdtOrient: {}",
        if g.chance(10) {
            "obl"
        } else {
            g.pick(&["sag", "ax", "cor", "cor"])
        }
    ));
    lines.push("fov: 25.6 25.6".into());
    if g.chance(8) {
        let k = g.below(lines.len() as u64) as usize;
        lines.remove(k);
    }
    if g.chance(20) && lines.len() > 2 {
        lines.swap(0, 1);
    }
    let mut t = String::new();
    for (i, l) in lines.iter().enumerate() {
        t.push_str(l);
        if i + 1 < lines.len() || !g.chance(8) {
            t.push('\n');
        }
    }
    let folder = g.chance(15);
    let base = if folder { "w/x.d/s" } else { "w/s" };
    if folder {
        entries.push(dir_entry("w/x.d"));
    }
    if g.chance(20) {
        entries.push(dir_entry(&format!("{base}.spr")));
    } else if folder && g.chance(2) {
        entries.push(file_entry("w/x.spr", t.as_bytes()));
    } else {
        entries.push(file_entry(&format!("{base}.spr"), t.as_bytes()));
    }
    let mut d = voxels(g, (x * y * z) as usize, bytes.min(4), float);
    if g.chance(10) && !d.is_empty() {
        let k = g.below(d.len() as u64) as usize;
        d.truncate(k);
    }
    let name = if folder {
        entries.push(file_entry(base, &d));
        base.to_owned()
    } else {
        entries.push(file_entry("w/s.sdt", &d));
        "w/s.sdt".to_owned()
    };
    let ty = if folder || g.chance(4) { 10 } else { -1 };
    (name, ty)
}

pub fn generate(g: &mut Gen, kind: &str, _portable: bool) -> Vec<String> {
    let mut t = vec![kind.to_owned()];
    let mut entries = Vec::new();
    let mut crop = false;
    let (name, ty) = if g.chance(2) {
        gdf_case(g, &mut entries, &mut crop)
    } else {
        sdt_case(g, &mut entries)
    };
    let flag = if crop { g.chance(2) } else { g.chance(12) };
    t.push(if flag { "1" } else { "0" }.into());
    let gd = if g.chance(8) {
        g.pick(&["10000", "10000040", "40"])
    } else {
        "0"
    };
    t.push(gd.into());
    t.push(if g.chance(12) { "0" } else { "1" }.into());
    t.push("0".into());
    t.push(ty.to_string());
    t.push(enc(Some(name.as_bytes())));
    t.push(entries.join(","));
    t
}

/// The port's answer to a piece-18 case: the GDF crop flag, then piece 6's `mrd`/`mrh`.
pub fn run(
    tk: &[&str],
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    o: &mut Out,
    w: &Work,
    extra: &mut Vec<&'static str>,
) -> Result<(), u8> {
    g.gdf_crop_flag = tk[1] != "0";
    g.progname = "fsio".into();
    let mut t: Vec<&str> = vec![if tk[0] == "xrd" { "mrd" } else { "mrh" }];
    t.extend_from_slice(&tk[2..]);
    let r = super::piece6::run_env(&t, g, out, err, o, w, extra, &[]);
    for n in g.notices.as_slice() {
        // FreeSurfer reads for ever
        if n.defect.id == "FS-674" {
            extra.push("SKIP");
        }
        // FreeSurfer's values from memory contents
        if n.defect.id == "FS-676" || (n.defect.id == "FS-677" && n.detail.contains("memory")) {
            extra.push("VARY");
        }
    }
    r
}
