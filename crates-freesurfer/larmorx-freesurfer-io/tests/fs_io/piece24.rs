// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Piece 24 (track A relay 32): NRRD volumes. `nnd` and `nnh` (piece 6's `mrd` and `mrh`, the
//! type -1 or 26, or a name with `@nrrd`) of NRRD files built field by field: attached (`.nrrd`)
//! and detached (`.nhdr` with its data file), the raw, ASCII and hex encodings in both byte orders,
//! every sample type (doubles and blocks refused), two to five axes, the spaces (RAS, LAS, LPS,
//! scanner, right- and left-handed, the four-dimensional ones, a bare space dimension, none),
//! space directions diagonal, oblique, flipped, of length 0 or `none`, with and without a space
//! origin, kinds, key/value pairs and a content line; samples past the range of the type
//! FreeSurfer stores them in; then now and then one fault: the data cut short, a missing or
//! folder data file, a wrong magic line, a missing file or a folder in the file's place.

use std::io::Write;

use larmorx_freesurfer_core::globals::Globals;

use super::piece4::file_entry;
use super::{Gen, Out, Work, enc, hex};

pub const KINDS: [&str; 2] = ["nnd", "nnh"];

fn dir_entry(name: &str) -> String {
    format!("D{}", hex(name.as_bytes()))
}

/// A NRRD type: its name in the header, its size, and whether it is an integer type.
const TYPES: [(&str, usize); 11] = [
    ("signed char", 1),
    ("uchar", 1),
    ("short", 2),
    ("ushort", 2),
    ("int", 4),
    ("uint", 4),
    ("longlong", 8),
    ("ulonglong", 8),
    ("float", 4),
    ("double", 8),
    ("block", 3),
];

/// A number as text: now and then in another spelling.
fn num(g: &mut Gen, v: f64) -> String {
    if g.chance(25) {
        return format!("{v:e}");
    }
    format!("{v}")
}

/// One sample of type `t` (an index into [`TYPES`]) as its bytes, little-endian, and as text.
fn sample(g: &mut Gen, t: usize) -> (Vec<u8>, String) {
    let wide = g.chance(6); // a value past the range FreeSurfer's type holds
    match t {
        0 => {
            let v = if wide {
                -(g.int(1, 128) as i8 as i64)
            } else {
                g.int(0, 127)
            } as i8;
            (vec![v as u8], v.to_string())
        }
        1 => {
            let v = g.below(256) as u8;
            (vec![v], v.to_string())
        }
        2 => {
            let v = g.int(-32768, 32767) as i16;
            (v.to_le_bytes().to_vec(), v.to_string())
        }
        3 => {
            let v = if wide {
                g.int(32768, 65535)
            } else {
                g.int(0, 32767)
            } as u16;
            (v.to_le_bytes().to_vec(), v.to_string())
        }
        4 => {
            let v = g.next() as i32;
            (v.to_le_bytes().to_vec(), v.to_string())
        }
        5 => {
            let v = if wide {
                0x8000_0000u32 | g.next() as u32
            } else {
                g.next() as u32 & 0x7fff_ffff
            };
            (v.to_le_bytes().to_vec(), v.to_string())
        }
        6 => {
            let v = g.next() as i64;
            (v.to_le_bytes().to_vec(), v.to_string())
        }
        7 => {
            let v = if wide {
                0x8000_0000_0000_0000u64 | g.next()
            } else {
                g.next() & 0x7fff_ffff_ffff_ffff
            };
            (v.to_le_bytes().to_vec(), v.to_string())
        }
        8 => {
            let v: f32 = if g.chance(20) {
                g.pick(&[f32::NAN, f32::INFINITY, -0.0])
            } else {
                (g.int(-2000, 2000) as f32) / 8.0
            };
            (v.to_le_bytes().to_vec(), format!("{v}"))
        }
        9 => {
            let v = (g.int(-2000, 2000) as f64) / 4.0;
            (v.to_le_bytes().to_vec(), format!("{v}"))
        }
        _ => {
            let b = vec![g.below(256) as u8, g.below(256) as u8, g.below(256) as u8];
            (b, String::new())
        }
    }
}

/// A space direction vector, or `none`.
fn direction(g: &mut Gen, i: usize, oblique: bool) -> String {
    if g.chance(30) {
        return "none".into();
    }
    if g.chance(40) {
        return "(0,0,0)".into();
    }
    let mut v = [0.0f64; 3];
    v[i] = g.pick(&[1.0, 2.0, 0.5, -1.0, 1.5, 3.25, -0.75]);
    if oblique {
        for (k, c) in v.iter_mut().enumerate() {
            if k != i {
                *c = (g.int(-40, 40) as f64) / 64.0;
            }
        }
    }
    let parts: Vec<String> = v.iter().map(|&c| num(g, c)).collect();
    format!("({})", parts.join(","))
}

/// A NRRD file: its header lines (with the data when attached) and, when detached, the data file.
fn nrrd_file(g: &mut Gen, detached: Option<&str>) -> (Vec<u8>, Option<Vec<u8>>) {
    let t = if g.chance(10) {
        g.pick(&[9usize, 10])
    } else {
        g.below(9) as usize
    };
    let dim: usize = if g.chance(10) {
        g.pick(&[2usize, 5])
    } else {
        g.pick(&[3usize, 3, 4])
    };
    let sizes: Vec<usize> = (0..dim)
        .map(|i| if i < 3 { g.int(1, 5) } else { g.int(1, 3) } as usize)
        .collect();
    let n: usize = sizes.iter().product();
    let encoding = if t == 10 {
        "raw"
    } else {
        g.pick(&["raw", "raw", "raw", "ascii", "hex", "text"])
    };
    let big = encoding == "raw" && g.chance(4);
    let mut h: Vec<String> = Vec::new();
    h.push(if g.chance(30) {
        "NRRD0005".into()
    } else {
        "NRRD0004".into()
    });
    if g.chance(4) {
        h.push("# a comment".into());
    }
    if g.chance(5) {
        h.push(format!(
            "content: {}",
            g.pick(&["probe volume", "x", "a:b c"])
        ));
    }
    h.push(format!("type: {}", TYPES[t].0));
    if t == 10 {
        h.push("block size: 3".into());
    }
    h.push(format!("dimension: {dim}"));
    // the space: a name, a bare dimension, or none
    let space = match g.below(14) {
        0 => Some("right-anterior-superior"),
        1 => Some("RAS"),
        2 => Some("left-anterior-superior"),
        3 | 4 => Some("left-posterior-superior"),
        5 => Some("LPS"),
        6 => Some("scanner-xyz"),
        7 => Some("3D-right-handed"),
        8 => Some("3D-left-handed"),
        9 => Some("right-anterior-superior-time"),
        10 => None,
        11 => Some(""), // a bare space dimension
        _ => Some("left-posterior-superior"),
    };
    let space_dim = match space {
        Some(s) if s.ends_with("-time") => 4,
        Some(_) if g.chance(25) => 2,
        Some(_) => 3,
        None => 0,
    };
    match space {
        Some("") => h.push(format!("space dimension: {space_dim}")),
        Some(s) => h.push(format!("space: {s}")),
        None => {}
    }
    h.push(format!(
        "sizes: {}",
        sizes
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
            .join(" ")
    ));
    if space_dim > 0 {
        if !g.chance(15) {
            let oblique = g.chance(3);
            let dirs: Vec<String> = (0..dim)
                .map(|i| {
                    if i >= 3 || i >= space_dim {
                        "none".into()
                    } else if space_dim == 3 {
                        direction(g, i, oblique)
                    } else {
                        let mut v = vec!["0".to_string(); space_dim];
                        v[i] = "1".into();
                        format!("({})", v.join(","))
                    }
                })
                .collect();
            h.push(format!("space directions: {}", dirs.join(" ")));
        }
        if !g.chance(6) {
            let o: Vec<String> = (0..space_dim)
                .map(|_| {
                    let v = (g.int(-4000, 4000) as f64) / 16.0;
                    num(g, v)
                })
                .collect();
            h.push(format!("space origin: ({})", o.join(",")));
        }
    } else if g.chance(2) {
        h.push(format!(
            "spacings: {}",
            (0..dim).map(|_| "1.5").collect::<Vec<_>>().join(" ")
        ));
    }
    if g.chance(3) {
        let kinds: Vec<&str> = (0..dim)
            .map(|i| {
                if i < 3 {
                    g.pick(&["domain", "space", "domain"])
                } else {
                    g.pick(&["list", "time", "domain", "vector"])
                }
            })
            .collect();
        h.push(format!("kinds: {}", kinds.join(" ")));
    }
    if TYPES[t].1 > 1 && encoding == "raw" {
        h.push(format!("endian: {}", if big { "big" } else { "little" }));
    }
    h.push(format!("encoding: {encoding}"));
    for k in 0..g.below(4) {
        let key = g.pick(&["modality", "DWMRI_b-value", "x", "a b"]);
        let val = g.pick(&["DWMRI", "1000", "", "v:=w"]);
        h.push(format!("{key}{k}:={val}"));
    }
    // the samples
    let mut data = Vec::new();
    let mut words = Vec::new();
    for _ in 0..n {
        let (mut b, s) = sample(g, t);
        if big {
            b.reverse();
        }
        data.extend_from_slice(&b);
        words.push(s);
    }
    let body: Vec<u8> = match encoding {
        "raw" => data,
        "hex" => {
            let mut s = String::new();
            for (i, c) in data.iter().enumerate() {
                if i > 0 && i % 16 == 0 {
                    s.push('\n');
                }
                s.push_str(&format!("{c:02x}"));
            }
            s.push('\n');
            s.into_bytes()
        }
        _ => {
            let mut s = words.join(" ");
            s.push('\n');
            s.into_bytes()
        }
    };
    let mut body = body;
    if g.chance(12) {
        let cut = g.below(body.len() as u64 + 1) as usize;
        body.truncate(cut);
    }
    if let Some(df) = detached {
        h.push(format!("data file: {df}"));
    }
    let mut text = h.join("\n").into_bytes();
    text.extend_from_slice(b"\n");
    if detached.is_some() {
        (text, Some(body))
    } else {
        text.extend_from_slice(b"\n");
        text.extend_from_slice(&body);
        (text, None)
    }
}

fn read_case(g: &mut Gen, entries: &mut Vec<String>) -> (String, i64) {
    let k = g.below(20);
    let (name, ty): (String, i64) = match k {
        0..=11 => {
            let name = g.pick(&["w/v.nrrd", "w/v.nrrd", "w/V.NRRD", "w/v.dat@nrrd"]);
            let file = name.split('@').next().unwrap_or(name);
            match g.below(30) {
                0 => entries.push(dir_entry(file)),
                1 => {}
                2 => {
                    let mut b = b"NRRD0009\ntype: uchar\n".to_vec();
                    b.extend_from_slice(b"dimension: 1\nsizes: 1\nencoding: raw\n\n\x01");
                    entries.push(file_entry(file, &b));
                }
                _ => {
                    let (b, _) = nrrd_file(g, None);
                    entries.push(file_entry(file, &b));
                }
            }
            (name.into(), g.pick(&[-1i64, -1, 26]))
        }
        _ => {
            let name = g.pick(&["w/v.nhdr", "w/v.nhdr", "w/v.NHDR"]);
            let data = g.pick(&["v.raw", "./v.raw", "v.raw", "sub/v.raw"]);
            let (b, body) = nrrd_file(g, Some(data));
            entries.push(file_entry(name, &b));
            let path = format!("w/{data}");
            if data.starts_with("sub/") {
                entries.push(dir_entry("w/sub"));
            }
            match g.below(20) {
                0 => entries.push(dir_entry(&path)),
                1 => {}
                _ => entries.push(file_entry(&path, body.as_deref().unwrap_or(&[]))),
            }
            (name.into(), g.pick(&[-1i64, 26]))
        }
    };
    let name = if g.chance(10) {
        format!("{name}#{}", g.pick(&["0", "1", "0:1", "1:2"]))
    } else {
        name
    };
    (name, ty)
}

pub fn generate(g: &mut Gen, kind: &str, _portable: bool) -> Vec<String> {
    let mut t = vec![kind.to_owned()];
    let mut entries = Vec::new();
    let (name, ty) = read_case(g, &mut entries);
    t.push(if g.chance(10) { "40" } else { "0" }.into()); // Gdiag
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

/// The port's answer to a piece-24 case: piece 6's `mrd`/`mrh`.
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
    let mut t: Vec<&str> = vec![if tk[0] == "nnh" { "mrh" } else { "mrd" }];
    t.extend_from_slice(&tk[1..]);
    let r = super::piece6::run_env(&t, g, out, err, o, w, extra, &[]);
    for n in g.notices.as_slice() {
        // NrrdIO's undefined paths (track C's probe does not compare them either), and the
        // reads past FreeSurfer's array
        if matches!(
            n.defect.id,
            "FS-761" | "FS-762" | "FS-765" | "FS-766" | "FS-767" | "FS-768" | "FS-771" | "FS-739"
        ) {
            extra.push("SKIP");
        }
    }
    r
}
