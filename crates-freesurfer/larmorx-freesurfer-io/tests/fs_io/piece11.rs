// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The fs.io probe's piece-11 cases (relay 20): fs.labels' labels, control points, annotation
//! tables and point sets. `lrd` (`LabelRead` of a generated label file, with or without a
//! subject, `SUBJECTS_DIR` and a Talairach transform), `lwr` (`LabelWrite` of a given label, then
//! every file under `w`), `lal` (`LabelAlloc`), `lre` (`LabelRealloc`), `lcp` (`LabelCopy`), `ldu`
//! (`LabelRemoveDuplicates`), `lhv` (`LabelHasVertex`), `crd` (`MRIreadControlPoints` of a
//! control-point or label file), `cwr` (`MRIwriteControlPoints`), `cfp` (`MRIfillPoints` on a
//! small volume), `atr` (the annotation table read from a file, `FREESURFER_HOME` and
//! `annotation_table_file` given, then lookups and prints), `ats` (`set_atable_from_ctable`, then
//! the same), `pss` (an `fsPointSet` built, then its writers, queries and changes). The files are
//! built from the formats' rules (label files with every first line, count and point line form,
//! comments, blank lines, CR LF, long lines; control-point files with points, `info`,
//! `numpoints`, `useRealRAS` lines; annotation tables of index, name and colour lines).

use std::io::Write;
use std::path::Path;

use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_io::annotation::{self, AnnotationTables};
use larmorx_freesurfer_io::ctrpoints::{self, ControlInfo, MPoint};
use larmorx_freesurfer_io::label::{self, Label, LabelVertex};
use larmorx_freesurfer_io::pointset::{Point, PointSet};
use larmorx_freesurfer_mri::mri::{MRI_FLOAT, MRI_INT, MRI_SHORT, MRI_UCHAR, Mri};
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};

use super::piece4::{file_entry, setup};
use super::piece10::{Toks, fbits, read_table, table, xfm_text};
use super::{Gen, Out, Work, enc, hex};

/// The piece-11 commands.
pub const KINDS: [&str; 13] = [
    "lrd", "lwr", "lal", "lre", "lcp", "ldu", "lhv", "crd", "cwr", "cfp", "atr", "ats", "pss",
];

// ---------------------------------------------------------------- generation

fn d_entry(name: &str) -> String {
    format!("D{}", hex(name.as_bytes()))
}

fn dbits(v: f64) -> String {
    format!("{:016x}", v.to_bits())
}

/// A coordinate in text as a label or control-point file holds it.
fn coord_text(g: &mut Gen) -> String {
    match g.below(24) {
        0 => g
            .pick(&[
                "nan", "-inf", "1e40", "1e-50", "-0", "0x1p3", "+.5", "1.", "7e",
            ])
            .into(),
        1 => "x".into(),
        2 => format!("{}", g.int(-3000, 3000)),
        _ => format!("{:.3}", (g.int(-200_000, 200_000) as f64) / 997.0),
    }
}

fn eol(g: &mut Gen) -> &'static [u8] {
    if g.chance(10) { b"\r\n" } else { b"\n" }
}

/// A label file: its first line, count line and point lines in every form the reader meets.
fn label_text(g: &mut Gen) -> Vec<u8> {
    let mut s = Vec::new();
    if g.chance(40) {
        return s;
    }
    let first: Vec<u8> = match g.below(14) {
        0 => b"#!ascii label  , from subject  vox2ras=scanner".to_vec(),
        1 => b"#!ascii label lh.x , from subject bert vox2ras=voxel".to_vec(),
        2 => b"# a scanner label without the key".to_vec(),
        3 => b"#!ascii from subject".to_vec(),
        4 => {
            let mut l = b"#!ascii label x , from subject s vox2ras=".to_vec();
            let n = g.pick(&[98usize, 99, 100, 101, 140]);
            l.extend(std::iter::repeat_n(b'v', n));
            l
        }
        5 => {
            let mut l = b"# long ".to_vec();
            l.extend(std::iter::repeat_n(b'a', g.pick(&[1015usize, 1016, 1030])));
            l.extend_from_slice(b" vox2ras=TkReg");
            l
        }
        6 => b"vox2ras=TkReg scanner".to_vec(),
        7 => b"".to_vec(),
        _ => b"#!ascii label  , from subject  vox2ras=TkReg".to_vec(),
    };
    s.extend_from_slice(&first);
    s.extend_from_slice(eol(g));
    if g.chance(30) {
        return s;
    }
    let k = if g.chance(12) {
        g.int(0, 40)
    } else {
        g.int(1, 6)
    };
    if g.chance(5) {
        s.extend_from_slice(g.pick(&[b"# c\n".as_slice(), b"\n", b"   \n", b"\t# x\n"]));
    }
    let count = match g.below(16) {
        0 => "0".to_string(),
        1 => format!("{}", k + g.int(1, 3)),
        2 => format!("{}", (k - 1).max(0)),
        3 => g
            .pick(&["abc", "-1", "-3", "99999999999", " 4 # c", "2x"])
            .into(),
        _ => k.to_string(),
    };
    s.extend_from_slice(count.as_bytes());
    s.extend_from_slice(eol(g));
    for _ in 0..k {
        if g.chance(12) {
            s.extend_from_slice(g.pick(&[b"# c\n".as_slice(), b"\n", b"  \n"]));
        }
        let vno = if g.chance(4) { -1 } else { g.int(0, 300) };
        let mut l = format!(
            "{}  {}  {}  {} {}",
            vno,
            coord_text(g),
            coord_text(g),
            coord_text(g),
            coord_text(g)
        );
        match g.below(20) {
            0 => l = l.replace("  ", "\t"),
            1 => l.push_str(" 7 extra"),
            2 => l = l.rsplit_once(' ').map(|x| x.0.to_string()).unwrap_or(l),
            3 => l.push_str(" # comment"),
            4 => l = format!("  {l}"),
            _ => {}
        }
        s.extend_from_slice(l.as_bytes());
        if !g.chance(25) {
            s.extend_from_slice(eol(g));
        }
    }
    if g.chance(8) {
        s.extend_from_slice(b"\n# trailing\n");
    }
    s
}

fn fval(g: &mut Gen) -> f32 {
    match g.below(16) {
        0 => f32::NAN,
        1 => -0.0,
        2 => 1.0 + f32::EPSILON,
        3 => 1.0 + f32::EPSILON / 2.0,
        4 => f32::INFINITY,
        _ => (g.int(-8, 8) as f32) * 0.5,
    }
}

/// A label's tokens (`N` sometimes): its fields and places.
fn label_toks(g: &mut Gen, may_null: bool) -> Vec<String> {
    if may_null && g.chance(4) {
        return vec!["N".into()];
    }
    let max_points = if g.chance(10) {
        g.int(30, 90)
    } else {
        g.int(0, 8)
    };
    let n_points = g.int(0, max_points);
    let mut t = vec![
        max_points.to_string(),
        g.int(0, 3).to_string(),
        n_points.to_string(),
        enc(Some(g.pick(&[
            b"".as_slice(),
            b"lh.cortex.label",
            b"w/a.label",
        ]))),
        enc(Some(g.pick(&[b"".as_slice(), b"s1", b"bert"]))),
        enc(Some(g.pick(&[
            b"TkReg".as_slice(),
            b"scanner",
            b"voxel",
            b"",
        ]))),
        dbits(g.int(-4, 4) as f64 * 0.25),
        max_points.to_string(),
    ];
    for _ in 0..max_points {
        let vno = if g.chance(3) { -1 } else { g.int(0, 6) };
        t.push(vno.to_string());
        for _ in 0..3 {
            t.push(format!("{:08x}", fval(g).to_bits()));
        }
        for _ in 0..3 {
            t.push(g.int(-2, 9).to_string());
        }
        t.push(if g.chance(6) { "1" } else { "0" }.into());
        t.push(fbits(g));
    }
    if g.chance(8) {
        t.push(max_points.to_string());
        for _ in 0..max_points {
            t.push(g.int(-1, 50).to_string());
        }
    } else {
        t.push("N".into());
    }
    t
}

fn subjects_dir(g: &mut Gen) -> String {
    match g.below(8) {
        0 => "N".into(),
        1 => enc(Some(b"w/nosuch")),
        _ => enc(Some(b"w/subj")),
    }
}

/// The subject folders a case may need, then its files.
fn subject_dirs() -> Vec<String> {
    [
        "w/subj",
        "w/subj/s1",
        "w/subj/s1/label",
        "w/subj/s1/mri",
        "w/subj/s1/mri/transforms",
    ]
    .iter()
    .map(|d| d_entry(d))
    .collect()
}

fn ctrl_text(g: &mut Gen) -> Vec<u8> {
    let mut s = Vec::new();
    let k = g.int(0, 7);
    for _ in 0..k {
        if g.chance(8) {
            s.extend_from_slice(g.pick(&[b"# c\n".as_slice(), b"\n", b"\t\n"]));
        }
        let mut l = format!("{} {} {}", coord_text(g), coord_text(g), coord_text(g));
        if g.chance(10) {
            l.push_str(" 1");
        }
        if g.chance(30) {
            l = "1 ".repeat(110);
        }
        s.extend_from_slice(l.as_bytes());
        s.extend_from_slice(eol(g));
    }
    if !g.chance(4) {
        s.extend_from_slice(b"info\n");
        s.extend_from_slice(format!("numpoints {k}\n").as_bytes());
        let u = g.pick(&[
            "useRealRAS 1",
            "useRealRAS 0",
            "useRealRAS",
            "useRealRAS x",
            "useRealRAS 7 9",
            "  useRealRAS 1 # c",
            "userealras 1",
        ]);
        s.extend_from_slice(u.as_bytes());
        s.extend_from_slice(eol(g));
        if g.chance(3) {
            s.extend_from_slice(b"written by someone on a day\n");
        }
    }
    s
}

fn atable_text(g: &mut Gen) -> Vec<u8> {
    let mut s = Vec::new();
    let k = g.int(0, 8);
    for i in 0..k {
        if g.chance(8) {
            s.extend_from_slice(g.pick(&[b"# c\n".as_slice(), b"\n", b"  \n"]));
        }
        let name: Vec<u8> = match g.below(20) {
            0 => vec![b'n'; g.pick(&[99usize, 100, 150])],
            1 => b"with#hash".to_vec(),
            _ => format!(
                "{}-region{}",
                g.pick(&["ctx", "Left", "unknown"]),
                g.int(0, 9)
            )
            .into_bytes(),
        };
        let mut l = format!("{} ", if g.chance(8) { g.int(-5, 5) } else { i }).into_bytes();
        l.extend_from_slice(&name);
        let fields = if g.chance(8) { g.int(0, 3) } else { 4 };
        for _ in 0..fields {
            l.extend_from_slice(format!(" {}", g.int(-20, 300)).as_bytes());
        }
        if g.chance(30) {
            l.extend(std::iter::repeat_n(b' ', 200));
            l.extend_from_slice(b"7");
        }
        s.extend_from_slice(&l);
        if i + 1 < k || !g.chance(4) {
            s.extend_from_slice(eol(g));
        }
    }
    s
}

fn point_toks(g: &mut Gen) -> Vec<String> {
    let mut t = Vec::new();
    for _ in 0..4 {
        let v = match g.below(14) {
            0 => f64::NAN,
            1 => -0.0,
            2 => 1e300,
            3 => 123_456_789.123,
            _ => (g.int(-100_000, 100_000) as f64) / 7.0,
        };
        t.push(dbits(v));
    }
    t.push(g.int(-3, 120).to_string());
    t.push(g.int(-1, 3).to_string());
    t.push(enc(Some(g.pick(&[
        b"".as_slice(),
        b"Left-Hippocampus",
        b"x",
        b"a long structure name",
    ]))));
    t
}

/// One piece-11 case.
pub fn generate(g: &mut Gen, kind: &str, portable: bool) -> Vec<String> {
    let mut t = vec![kind.to_string()];
    match kind {
        "lrd" => {
            t.push(subjects_dir(g));
            let mut spec = subject_dirs();
            let place = g.pick(&[
                "w/a.label",
                "w/a",
                "w/subj/s1/label/lh.a.label",
                "w/a.LABEL",
                "w/a.label.label",
            ]);
            let place = if portable && place == "w/a.LABEL" {
                "w/b.LABEL"
            } else {
                place
            };
            if !g.chance(15) {
                spec.push(file_entry(place, &label_text(g)));
            }
            if g.chance(3) {
                let x = if g.chance(6) {
                    b"garbage\n".to_vec()
                } else {
                    xfm_text(g)
                };
                spec.push(file_entry("w/subj/s1/mri/transforms/talairach.xfm", &x));
            }
            t.push(spec.join(","));
            t.push(match g.below(3) {
                0 => "N".into(),
                1 => enc(Some(b"s2")),
                _ => enc(Some(b"s1")),
            });
            let asked = g.pick(&[
                place,
                place,
                "w/a.label",
                "w/a",
                "lh.a",
                "lh.a.label",
                "w/missing",
                "w/subj",
                "w/a.LABEL",
            ]);
            // names that differ from the file's only in case open on Windows' and macOS' file
            // systems: the portable cases use another name for the upper-case one
            let asked = if portable && asked == "w/a.LABEL" {
                "w/b.LABEL"
            } else {
                asked
            };
            t.push(enc(Some(asked.as_bytes())));
        }
        "lwr" => {
            t.push(subjects_dir(g));
            let mut spec = subject_dirs();
            if g.chance(6) {
                spec.truncate(2);
            }
            t.push(spec.join(","));
            let l = label_toks(g, false);
            let has_subject = l[4] != "x";
            t.extend(l);
            let mut names = vec![
                "w/out",
                "w/out.label",
                "w/out.LABEL",
                "w/x.y",
                "w/nodir/o",
                "w/o.label.label",
            ];
            if has_subject {
                names.extend(["lh.out", "lh.out.label"]);
            }
            t.push(enc(Some(g.pick(&names).as_bytes())));
        }
        "lal" => {
            t.push(subjects_dir(g));
            t.push(
                if g.chance(15) {
                    g.pick(&[-1i64, -5, 0])
                } else {
                    g.int(0, 12)
                }
                .to_string(),
            );
            t.push(if g.chance(2) {
                "N".into()
            } else {
                enc(Some(b"s1"))
            });
            t.push(if g.chance(3) {
                "N".into()
            } else {
                enc(Some(g.pick(&[b"".as_slice(), b"lh.x", b"w/a.label"])))
            });
        }
        "lre" => {
            t.extend(label_toks(g, true));
            t.push(g.int(-2, 14).to_string());
        }
        "lcp" => {
            t.push(subjects_dir(g));
            t.extend(label_toks(g, false));
            t.extend(label_toks(g, true));
        }
        "ldu" => {
            t.push(g.pick(&["0", "64"]).into());
            t.extend(label_toks(g, false));
        }
        "lhv" => {
            t.extend(label_toks(g, false));
            let k = g.int(0, 6);
            t.push(k.to_string());
            for _ in 0..k {
                t.push(g.int(-2, 7).to_string());
            }
        }
        "crd" => {
            t.push(g.pick(&["0", "64"]).into());
            let place = g.pick(&["w/c.dat", "w/c.label", "w/c.LABEL", "w/c"]);
            let body = if place.to_ascii_lowercase().ends_with(".label") && !g.chance(5) {
                label_text(g)
            } else {
                ctrl_text(g)
            };
            let spec = if g.chance(15) {
                "-".to_string()
            } else {
                file_entry(place, &body)
            };
            t.push(spec);
            let asked = if g.chance(12) {
                g.pick(&["w/missing.dat", "w", "w/missing.label"])
            } else {
                place
            };
            t.push(enc(Some(asked.as_bytes())));
        }
        "cwr" => {
            t.push(g.pick(&["0", "64"]).into());
            t.push("-".into());
            let k = g.int(0, 6);
            t.push(k.to_string());
            for _ in 0..3 * k {
                let v = match g.below(12) {
                    0 => f64::NAN,
                    1 => -0.0,
                    2 => 1e20,
                    _ => (g.int(-100_000, 100_000) as f64) / 13.0,
                };
                t.push(dbits(v));
            }
            t.push(g.int(0, k).to_string());
            t.push(
                if g.chance(10) {
                    g.pick(&[2i64, -1])
                } else {
                    g.int(0, 1)
                }
                .to_string(),
            );
            t.push(enc(Some(
                g.pick(&["w/o.dat", "w/o.dat", "w/nodir/o.dat"]).as_bytes(),
            )));
        }
        "cfp" => {
            let (w, h, d) = (g.int(1, 6), g.int(1, 6), g.int(1, 6));
            t.push(w.to_string());
            t.push(h.to_string());
            t.push(d.to_string());
            t.push(
                g.pick(&[MRI_UCHAR, MRI_INT, MRI_FLOAT, MRI_SHORT])
                    .to_string(),
            );
            for _ in 0..3 {
                let s: f32 = if g.chance(20) {
                    0.0
                } else {
                    g.pick(&[0.5f32, 1.0, 1.5, 2.0, 3.0])
                };
                t.push(format!("{:08x}", s.to_bits()));
            }
            let perm = g.pick(&[[0usize, 1, 2], [1, 0, 2], [0, 2, 1], [2, 0, 1]]);
            for (axis, &p) in perm.iter().enumerate() {
                let _ = axis;
                for c in 0..3 {
                    let v: f32 = if c == p {
                        if g.chance(3) { -1.0 } else { 1.0 }
                    } else {
                        0.0
                    };
                    t.push(format!("{:08x}", v.to_bits()));
                }
            }
            for _ in 0..3 {
                t.push(format!("{:08x}", (g.int(-4, 4) as f32 * 0.5).to_bits()));
            }
            let k = g.int(0, 4);
            t.push(k.to_string());
            for _ in 0..k {
                t.push(g.int(0, w - 1).to_string());
                t.push(g.int(0, h - 1).to_string());
                t.push(g.int(0, d - 1).to_string());
                t.push(format!(
                    "{:08x}",
                    (g.pick(&[127.0f32, 255.0, 3.0])).to_bits()
                ));
            }
            t.push(g.pick(&[0i64, 0, 1, 127, 255, 7]).to_string());
            t.push(g.int(0, 1).to_string());
            let k = g.int(0, 5);
            t.push(k.to_string());
            for _ in 0..3 * k {
                let v = if g.chance(25) {
                    f64::NAN
                } else {
                    (g.int(-80, 80) as f64) / 8.0
                };
                t.push(dbits(v));
            }
        }
        "atr" | "ats" => {
            if kind == "atr" {
                t.push(match g.below(6) {
                    0 => "N".into(),
                    1 => enc(Some(b"w/nosuch")),
                    _ => enc(Some(b"w/fsh")),
                });
                let mut spec = vec![d_entry("w/fsh")];
                if !g.chance(6) {
                    spec.push(file_entry(
                        "w/fsh/Simple_surface_labels2009.txt",
                        &atable_text(g),
                    ));
                }
                if g.chance(2) {
                    spec.push(file_entry("w/fsh/tab.txt", &atable_text(g)));
                }
                if g.chance(3) {
                    spec.push(file_entry("w/t2.txt", &atable_text(g)));
                }
                t.push(spec.join(","));
                t.push(match g.below(5) {
                    0 => enc(Some(b"w/t2.txt")),
                    1 => enc(Some(b"w/missing")),
                    _ => "N".into(),
                });
            } else {
                let holes = g.chance(4);
                t.extend(table(g, holes));
                t.push(if g.chance(10) { "0" } else { "1" }.into());
            }
            let k = g.int(1, 8);
            t.push(k.to_string());
            for _ in 0..k {
                let op = g.pick(&["R", "N", "i", "n", "a", "m", "p", "c", "i", "n", "m"]);
                t.push(op.into());
                match op {
                    "N" => t.push(enc(Some(
                        g.pick(&["tab.txt", "w/t2.txt", "nosuch.txt", "w/fsh/tab.txt"])
                            .as_bytes(),
                    ))),
                    "i" | "m" => t.push(
                        match g.below(3) {
                            0 => g.int(-1, 70000),
                            _ => {
                                let (r, gg, b) = (g.int(0, 300), g.int(-1, 300), g.int(0, 300));
                                r + (gg << 8) + (b << 16)
                            }
                        }
                        .to_string(),
                    ),
                    "n" | "a" => t.push(g.int(-6, 10).to_string()),
                    _ => {}
                }
            }
        }
        _ => {
            // pss
            t.push(d_entry("w/sub"));
            t.push(g.pick(&["1", "1", "0"]).into());
            t.push(g.pick(&["1", "1", "0"]).into());
            t.push(enc(Some(g.pick(&[
                b"scanner_ras".as_slice(),
                b"tkreg",
                b"",
            ]))));
            let k = g.int(0, 5);
            t.push(k.to_string());
            for _ in 0..k {
                t.extend(point_toks(g));
            }
            let nops = g.int(1, 6);
            t.push(nops.to_string());
            for _ in 0..nops {
                let op = g.pick(&[
                    "S", "C", "P", "K", "W", "T", "G", "E", "F", "A", "R", "Z", "T",
                ]);
                t.push(op.into());
                let file = |g: &mut Gen| {
                    enc(Some(
                        g.pick(&["w/p.json", "w/sub/p.dat", "w/nodir/p.txt"])
                            .as_bytes(),
                    ))
                };
                match op {
                    "S" | "C" | "W" => t.push(file(g)),
                    "T" => {
                        t.push(file(g));
                        t.push(enc(Some(g.pick(&[b"".as_slice(), b"reg.lta"]))));
                    }
                    "G" => t.push(g.int(-1, 6).to_string()),
                    "E" => {
                        t.push(g.int(-1, 6).to_string());
                        t.extend(point_toks(g).into_iter().take(4));
                    }
                    "F" => {
                        t.push(g.int(-1, 6).to_string());
                        t.extend(point_toks(g));
                    }
                    "A" => t.extend(point_toks(g).into_iter().take(4)),
                    "R" => t.push(
                        if g.chance(10) {
                            g.int(-1, 9)
                        } else {
                            g.int(0, k.max(1) - 1)
                        }
                        .to_string(),
                    ),
                    _ => {}
                }
            }
        }
    }
    t
}

// ---------------------------------------------------------------- the port's answers

fn oi(o: &mut Out, v: i64) {
    o.i(v);
}

fn of(o: &mut Out, f: f32) {
    o.0.push_str(&format!("{:08x} ", f.to_bits()));
}

fn od(o: &mut Out, v: f64) {
    o.0.push_str(&format!("{:016x} ", v.to_bits()));
}

pub(super) fn ostr(o: &mut Out, s: Option<&[u8]>) {
    o.0.push_str(&enc(s));
    o.0.push(' ');
}

fn ocs(o: &mut Out, s: &[u8]) {
    let n = s
        .iter()
        .position(|&c| c == 0)
        .unwrap_or(s.len())
        .min(label::STRLEN);
    ostr(o, Some(&s[..n]));
}

pub(super) fn fnv(o: &mut Out, b: &[u8]) {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &c in b {
        h ^= u64::from(c);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    o.0.push_str(&format!("fnv{h:016x} "));
}

pub(super) fn obytes(o: &mut Out, b: &[u8]) {
    o.0.push_str(&hex(b));
    o.0.push(' ');
}

pub(super) fn f32_of(t: &str) -> f32 {
    f32::from_bits(u32::from_str_radix(t, 16).unwrap())
}

fn f64_of(t: &str) -> f64 {
    f64::from_bits(u64::from_str_radix(t, 16).unwrap())
}

fn read_label(tk: &mut Toks) -> Option<Label> {
    if tk.t[tk.i] == "N" {
        tk.i += 1;
        return None;
    }
    let max_points = tk.int() as i32;
    let coords = tk.int() as i32;
    let n_points = tk.int() as i32;
    let cut = |b: Vec<u8>, n: usize| -> Vec<u8> {
        let e = b.iter().position(|&c| c == 0).unwrap_or(b.len()).min(n);
        b[..e].to_vec()
    };
    let name = cut(tk.bytes(), label::STRLEN - 1);
    let subject_name = cut(tk.bytes(), label::STRLEN - 1);
    let space = cut(tk.bytes(), label::SPACE_LEN - 1);
    let avg_stat = f64_of(tk.next());
    let mut lv = vec![LabelVertex::default(); max_points.max(0) as usize];
    let k = tk.int() as usize;
    for v in lv.iter_mut().take(k) {
        v.vno = tk.int() as i32;
        v.x = f32_of(tk.next());
        v.y = f32_of(tk.next());
        v.z = f32_of(tk.next());
        v.xv = tk.int() as i32;
        v.yv = tk.int() as i32;
        v.zv = tk.int() as i32;
        v.deleted = tk.int() as u8;
        v.stat = f32_of(tk.next());
    }
    let vertex_label_ind = if tk.t[tk.i] == "N" {
        tk.i += 1;
        None
    } else {
        let m = tk.int() as usize;
        Some((0..m).map(|_| tk.int() as i32).collect())
    };
    Some(Label {
        max_points,
        coords,
        n_points,
        name,
        subject_name,
        lv,
        space,
        avg_stat,
        vertex_label_ind,
        ..Label::default()
    })
}

fn print_t(o: &mut Out, t: &Option<larmorx_freesurfer_mri::mri::Transform4>) {
    match t {
        None => o.0.push_str("N "),
        Some(t) => {
            for row in t.iter() {
                for &x in row {
                    od(o, x);
                }
            }
        }
    }
}

fn print_label(o: &mut Out, a: Option<&Label>) {
    let Some(a) = a else {
        o.0.push_str("N ");
        return;
    };
    oi(o, i64::from(a.max_points));
    oi(o, i64::from(a.coords));
    oi(o, i64::from(a.n_points));
    ocs(o, &a.name);
    ocs(o, &a.subject_name);
    ocs(o, &a.space);
    od(o, a.avg_stat);
    let mut p = Out::default();
    for i in 0..a.max_points.max(0) as usize {
        let v = a.lv.get(i).copied().unwrap_or_default();
        oi(&mut p, i64::from(v.vno));
        of(&mut p, v.x);
        of(&mut p, v.y);
        of(&mut p, v.z);
        oi(&mut p, i64::from(v.xv));
        oi(&mut p, i64::from(v.yv));
        oi(&mut p, i64::from(v.zv));
        oi(&mut p, i64::from(v.deleted));
        of(&mut p, v.stat);
    }
    if p.0.len() > 2000 {
        fnv(o, p.0.as_bytes());
    } else {
        o.0.push_str(&format!("[ {}] ", p.0));
    }
    match &a.vertex_label_ind {
        None => o.0.push_str("N "),
        Some(v) => {
            for &x in v.iter().take(a.max_points.max(0) as usize) {
                oi(o, i64::from(x));
            }
        }
    }
    print_t(o, &a.linear_transform);
    print_t(o, &a.inverse_linear_transform);
}

/// Every file under the fixture's `w`: its name (from `w`) and bytes, sorted by name.
fn odir(o: &mut Out, w: &Work) {
    fn walk(base: &Path, rel: &str, files: &mut Vec<String>) {
        let Ok(rd) = std::fs::read_dir(base.join(rel)) else {
            return;
        };
        for e in rd.flatten() {
            let name = format!("{rel}/{}", e.file_name().to_string_lossy());
            let Ok(ft) = e.file_type() else { continue };
            if ft.is_dir() {
                walk(base, &name, files);
            } else if ft.is_file() {
                files.push(name);
            }
        }
    }
    let mut files = Vec::new();
    walk(&w.fix, "w", &mut files);
    files.sort();
    let user = format!(
        "written by {} on ",
        larmorx_freesurfer_core::system::user_name(&Environment::of_process())
    );
    for f in files {
        ostr(o, Some(f.as_bytes()));
        let b = std::fs::read(w.fix.join(&f)).unwrap_or_default();
        let b = super::replace_all(&b, user.as_bytes(), b"written by U on ");
        if b.len() <= 256 {
            obytes(o, &b);
        } else {
            fnv(o, &b);
        }
    }
    o.0.push_str("| ");
}

fn env_of(tk: &mut Toks, env: &mut Environment, name: &str) {
    let t = tk.next();
    match super::dec(t) {
        None => env.remove(name),
        Some(v) => env.set(name, String::from_utf8(v).unwrap()),
    }
}

fn read_mri(g: &mut Globals, tk: &mut Toks) -> Mri {
    let (w, h, d, ty) = (
        tk.int() as i32,
        tk.int() as i32,
        tk.int() as i32,
        tk.int() as i32,
    );
    let mut sink = Vec::new();
    let mut sink2 = Vec::new();
    let mut m =
        larmorx_freesurfer_mri::mri::mri_alloc(g, &mut sink, &mut sink2, w, h, d, ty).unwrap();
    let mut f = || f32_of(tk.next());
    m.vg.xsize = f();
    m.vg.ysize = f();
    m.vg.zsize = f();
    m.vg.x_r = f();
    m.vg.x_a = f();
    m.vg.x_s = f();
    m.vg.y_r = f();
    m.vg.y_a = f();
    m.vg.y_s = f();
    m.vg.z_r = f();
    m.vg.z_a = f();
    m.vg.z_s = f();
    m.vg.c_r = f();
    m.vg.c_a = f();
    m.vg.c_s = f();
    m.vg.ras_good_flag = 1;
    let k = tk.int();
    for _ in 0..k {
        let (c, r, s) = (tk.int() as i32, tk.int() as i32, tk.int() as i32);
        let v = f32_of(tk.next());
        mri_set_vox_val(g, &mut sink2, &mut m, c, r, s, 0, v, 0).unwrap();
    }
    m
}

fn print_mri(g: &mut Globals, o: &mut Out, m: &Mri) {
    let mut b = Vec::new();
    let mut sink = Vec::new();
    for s in 0..m.vg.depth {
        for r in 0..m.vg.height {
            for c in 0..m.vg.width {
                let v = mri_get_vox_val(g, &mut sink, m, c, r, s, 0, 0).unwrap();
                b.extend_from_slice(&v.to_le_bytes());
            }
        }
    }
    fnv(o, &b);
}

fn read_point(tk: &mut Toks) -> Point {
    let mut p = Point::new(
        f64_of(tk.next()),
        f64_of(tk.next()),
        f64_of(tk.next()),
        f64_of(tk.next()),
    );
    p.index = tk.int() as i32;
    p.count = tk.int() as i32;
    p.labelname = tk.bytes();
    p
}

pub(super) fn opoint(o: &mut Out, p: &Point) {
    od(o, p.x);
    od(o, p.y);
    od(o, p.z);
    od(o, p.value);
    oi(o, i64::from(p.index));
    oi(o, i64::from(p.count));
    ostr(o, Some(&p.labelname));
}

/// The port's answer to a piece-11 case (`Err`: the exit status).
pub fn run(
    tk: &[&str],
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    o: &mut Out,
    w: &Work,
    extra: &mut Vec<&'static str>,
) -> Result<(), u8> {
    let r = answer(tk, g, out, err, o, w);
    for n in g.notices.as_slice() {
        // FreeSurfer writes past its buffers, crashes, or depends on the machine's memory
        if matches!(
            n.defect.id,
            "FS-423" | "FS-424" | "FS-426" | "FS-427" | "FS-429" | "FS-430" | "FS-431" | "FS-11"
        ) {
            extra.push("SKIP");
        }
    }
    r
}

fn answer(
    tk: &[&str],
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    o: &mut Out,
    w: &Work,
) -> Result<(), u8> {
    let fatal = |f: larmorx_freesurfer_core::log::Fatal| f.status();
    let dir = Some(w.fix.as_path());
    g.progname = "fsio".into();
    let mut tk = Toks { t: tk, i: 1 };
    let mut env = Environment::default();
    match tk.t[0] {
        "lrd" => {
            env_of(&mut tk, &mut env, "SUBJECTS_DIR");
            setup(w, tk.next());
            g.errno = 0;
            let subj = super::dec(tk.next());
            let name = tk.string();
            let a =
                label::label_read(g, &env, out, err, dir, subj.as_deref(), &name).map_err(fatal)?;
            print_label(o, a.as_ref());
        }
        "lwr" => {
            env_of(&mut tk, &mut env, "SUBJECTS_DIR");
            setup(w, tk.next());
            g.errno = 0;
            let a = read_label(&mut tk).expect("a label");
            let name = tk.string();
            let r = label::label_write(g, &env, out, err, dir, &a, &name).map_err(fatal)?;
            oi(o, i64::from(r));
            odir(o, w);
        }
        "lal" => {
            env_of(&mut tk, &mut env, "SUBJECTS_DIR");
            let n = tk.int() as i32;
            let subj = super::dec(tk.next());
            let nm = super::dec(tk.next());
            let a = label::label_alloc(g, &env, err, n, subj.as_deref(), nm.as_deref())
                .map_err(fatal)?;
            print_label(o, Some(&a));
        }
        "lre" => {
            let a = read_label(&mut tk);
            let n = tk.int() as i32;
            let a = label::label_realloc(g, &env, err, a, n).map_err(fatal)?;
            print_label(o, Some(&a));
        }
        "lcp" => {
            env_of(&mut tk, &mut env, "SUBJECTS_DIR");
            let s = read_label(&mut tk).expect("a label");
            let d = read_label(&mut tk);
            let a = label::label_copy(g, &env, err, &s, d).map_err(fatal)?;
            print_label(o, Some(&a));
        }
        "ldu" => {
            g.gdiag = tk.int() as u64;
            let mut a = read_label(&mut tk).expect("a label");
            oi(o, i64::from(label::label_remove_duplicates(g, err, &mut a)));
            print_label(o, Some(&a));
        }
        "lhv" => {
            let a = read_label(&mut tk).expect("a label");
            let k = tk.int();
            for _ in 0..k {
                oi(o, i64::from(label::label_has_vertex(tk.int() as i32, &a)));
            }
        }
        "crd" => {
            g.gdiag = tk.int() as u64;
            setup(w, tk.next());
            g.errno = 0;
            let name = tk.string();
            let mut info = ControlInfo {
                count: -7,
                use_real_ras: -7,
            };
            let p = ctrpoints::mri_read_control_points(g, &env, out, err, dir, &name, &mut info)
                .map_err(fatal)?;
            oi(o, i64::from(info.count));
            oi(o, i64::from(info.use_real_ras));
            match p {
                None => o.0.push_str("N "),
                Some(p) => {
                    for q in p.iter().take(info.count.max(0) as usize) {
                        od(o, q.x);
                        od(o, q.y);
                        od(o, q.z);
                    }
                }
            }
        }
        "cwr" => {
            env.set(
                "FREESURFER_REPLACEMENT_FOR_CREATION_TIME_STRING",
                "Thu Jan  1 00:00:00 2026",
            );
            g.gdiag = tk.int() as u64;
            setup(w, tk.next());
            g.errno = 0;
            let k = tk.int();
            let pts: Vec<MPoint> = (0..k)
                .map(|_| MPoint {
                    x: f64_of(tk.next()),
                    y: f64_of(tk.next()),
                    z: f64_of(tk.next()),
                })
                .collect();
            let count = tk.int() as i32;
            let real = tk.int() as i32;
            let name = tk.string();
            let r =
                ctrpoints::mri_write_control_points(g, &env, err, dir, &pts, count, real, &name);
            oi(o, i64::from(r));
            odir(o, w);
        }
        "cfp" => {
            let mut m = read_mri(g, &mut tk);
            let fillval = tk.int() as i32;
            let real = tk.int() as i32;
            let k = tk.int();
            let pts: Vec<MPoint> = (0..k)
                .map(|_| MPoint {
                    x: f64_of(tk.next()),
                    y: f64_of(tk.next()),
                    z: f64_of(tk.next()),
                })
                .collect();
            let r = ctrpoints::mri_fill_points(g, out, err, &mut m, fillval, k as i32, real, &pts)
                .map_err(fatal)?;
            oi(o, i64::from(r));
            print_mri(g, o, &m);
        }
        "atr" | "ats" => {
            let mut t = AnnotationTables::default();
            if tk.t[0] == "atr" {
                env_of(&mut tk, &mut env, "FREESURFER_HOME");
                setup(w, tk.next());
                g.errno = 0;
                t.annotation_table_file = super::dec(tk.next());
            } else {
                let ct = read_table(&mut tk);
                let given = tk.int() != 0;
                let r =
                    annotation::set_atable_from_ctable(g, out, err, &mut t, given.then_some(&ct));
                oi(o, i64::from(r));
            }
            let k = tk.int();
            for _ in 0..k {
                match tk.next() {
                    "R" => oi(
                        o,
                        i64::from(annotation::read_annotation_table(
                            g, &env, out, err, dir, &mut t,
                        )),
                    ),
                    "N" => {
                        let nm = tk.bytes();
                        oi(
                            o,
                            i64::from(annotation::read_named_annotation_table(
                                g, &env, out, err, dir, &mut t, &nm,
                            )),
                        );
                    }
                    "i" => {
                        let a = tk.int() as i32;
                        oi(
                            o,
                            i64::from(annotation::annotation_to_index(
                                g, &env, out, err, dir, &mut t, a,
                            )),
                        );
                    }
                    "n" => {
                        let i = tk.int() as i32;
                        let n = annotation::index_to_name(g, &env, out, err, dir, &mut t, i);
                        ostr(o, Some(&n));
                    }
                    "a" => {
                        let i = tk.int() as i32;
                        oi(
                            o,
                            i64::from(annotation::index_to_annotation(
                                g, &env, out, err, dir, &mut t, i,
                            )),
                        );
                    }
                    "m" => {
                        let a = tk.int() as i32;
                        let (n, ix) =
                            annotation::annotation_to_name(g, &env, out, err, dir, &mut t, a);
                        ostr(o, Some(&n));
                        oi(o, i64::from(ix));
                    }
                    "p" => {
                        let mut b = Vec::new();
                        let r = annotation::print_annotation_table(
                            g, &env, out, err, dir, &mut t, &mut b,
                        );
                        let _ = out.write_all(&b);
                        oi(o, i64::from(r));
                    }
                    _ => {
                        let mut b = Vec::new();
                        let r = annotation::print_annotation_colortable(
                            g, &env, out, err, dir, &mut t, &mut b,
                        );
                        let _ = out.write_all(&b);
                        oi(o, i64::from(r));
                    }
                }
            }
        }
        _ => {
            setup(w, tk.next());
            g.errno = 0;
            let mut ps = PointSet {
                include_zero_count: tk.int() as i32,
                include_zero_index: tk.int() as i32,
                vox2ras: tk.bytes(),
                ..PointSet::default()
            };
            let k = tk.int();
            for _ in 0..k {
                let p = read_point(&mut tk);
                ps.add(p);
            }
            let nops = tk.int();
            for _ in 0..nops {
                match tk.next() {
                    "S" => {
                        let f = tk.string();
                        oi(o, i64::from(ps.save(dir, &f)));
                    }
                    "C" => {
                        let f = tk.string();
                        oi(o, i64::from(ps.save_as_ctrlpoint(dir, &f)));
                    }
                    "P" => oi(o, i64::from(ps.print(out))),
                    "K" => oi(o, i64::from(ps.print_coords(out))),
                    "W" => {
                        let f = tk.string();
                        oi(o, i64::from(ps.write_coords(dir, &f)));
                    }
                    "T" => {
                        let f = tk.string();
                        let l = tk.bytes();
                        oi(o, i64::from(ps.write_centroid_table(out, err, dir, &f, &l)));
                    }
                    "G" => {
                        let i = tk.int() as i32;
                        opoint(o, &ps.get(i));
                    }
                    "E" => {
                        let i = tk.int() as i32;
                        let (x, y, z, v) = (
                            f64_of(tk.next()),
                            f64_of(tk.next()),
                            f64_of(tk.next()),
                            f64_of(tk.next()),
                        );
                        oi(o, i64::from(ps.set_xyz(i, x, y, z, v)));
                    }
                    "F" => {
                        let i = tk.int() as i32;
                        let p = read_point(&mut tk);
                        oi(o, i64::from(ps.set(i, &p)));
                    }
                    "A" => {
                        let (x, y, z, v) = (
                            f64_of(tk.next()),
                            f64_of(tk.next()),
                            f64_of(tk.next()),
                            f64_of(tk.next()),
                        );
                        oi(o, i64::from(ps.add_xyz(x, y, z, v)));
                    }
                    "R" => {
                        let i = tk.int() as i32;
                        ps.remove(g, i);
                    }
                    _ => ps.clear(),
                }
            }
            oi(o, i64::from(ps.npoints()));
            odir(o, w);
        }
    }
    oi(o, i64::from(g.gerror));
    Ok(())
}
