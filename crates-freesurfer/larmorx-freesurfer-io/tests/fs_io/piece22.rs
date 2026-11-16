// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Piece 22 (track A relay 30): AFNI datasets, curvature files and annotations read as volumes,
//! and the AFNI and NRRD writers. `afd` and `afh` (piece 6's `mrd` and `mrh`, the type -1 or 7) of
//! AFNI `.HEAD`/`.BRIK` pairs built attribute by attribute (every attribute FreeSurfer reads, in
//! any order, with AFNI's spacing or none, CR LF lines, unknown attributes; then one fault: an
//! attribute missing, of the wrong type, with too few or too many numbers, a bad token, a
//! negative, zero or bare count, a string without its quote or its `~`, long strings, a count
//! before any type line, an unknown type, a last line without a newline, orientations out of range
//! or degenerate, other byte orders and brick types, frames scaled differently) and BRIKs of the
//! right length, cut, longer, missing or a folder; `cud` and `cuh` (the type -1 or 25) of new-format
//! curvature files (whole, cut, old-format, several values per vertex); `and` and `anh` (the type
//! -1 or 33) of annotation files (vertex numbers in and out of range, colour tables of both
//! versions or none, other tags); `kwr` (piece 17's `vwr`, `how` 17 BRIK or 36 NRRD).

use std::io::Write;

use larmorx_freesurfer_core::globals::Globals;

use super::piece4::file_entry;
use super::{Gen, Out, Work, enc, hex};

pub const KINDS: [&str; 7] = ["afd", "afh", "cud", "cuh", "and", "anh", "kwr"];

fn dir_entry(name: &str) -> String {
    format!("D{}", hex(name.as_bytes()))
}

/// A float as AFNI writes it, now and then in another form.
fn fnum(g: &mut Gen, v: f32) -> String {
    if g.chance(30) {
        return g
            .pick(&["nan", "-inf", "1e40", "-0", "0x1p-2", "1e-50", "3.5e+00"])
            .to_owned();
    }
    format!("{v}")
}

/// The numbers of an attribute, five to a line (AFNI's layout), or all on one line.
fn body(g: &mut Gen, vals: &[String]) -> String {
    if vals.is_empty() {
        return String::new();
    }
    let per = if g.chance(4) { vals.len() } else { 5 };
    let mut s = String::new();
    for (i, v) in vals.iter().enumerate() {
        if i > 0 && i % per == 0 {
            s.push('\n');
        }
        s.push(' ');
        s.push_str(v);
    }
    s
}

/// One attribute's text: its type, name and count lines and its body.
struct Attr {
    ty: String,
    name: &'static str,
    count: String,
    body: String,
}

impl Attr {
    fn ints(g: &mut Gen, name: &'static str, v: &[i64]) -> Attr {
        let vals: Vec<String> = v.iter().map(|x| x.to_string()).collect();
        Attr {
            ty: "integer-attribute".into(),
            name,
            count: v.len().to_string(),
            body: body(g, &vals),
        }
    }
    fn floats(g: &mut Gen, name: &'static str, v: &[f32]) -> Attr {
        let vals: Vec<String> = v.iter().map(|&x| fnum(g, x)).collect();
        Attr {
            ty: "float-attribute".into(),
            name,
            count: v.len().to_string(),
            body: body(g, &vals),
        }
    }
    fn string(name: &'static str, s: &str) -> Attr {
        Attr {
            ty: "string-attribute".into(),
            name,
            count: (s.len() + 1).to_string(),
            body: format!("'{s}~"),
        }
    }
    fn text(&self, spaced: bool) -> String {
        if spaced {
            format!(
                "\ntype = {}\nname = {}\ncount = {}\n{}\n",
                self.ty, self.name, self.count, self.body
            )
        } else {
            format!(
                "\ntype={}\nname={}\ncount={}\n{}\n",
                self.ty, self.name, self.count, self.body
            )
        }
    }
}

fn afni_case(g: &mut Gen, entries: &mut Vec<String>, portable: bool) -> (String, i64) {
    let nf = if g.chance(25) {
        0
    } else {
        g.pick(&[1i64, 1, 1, 2, 3])
    };
    let dim = |g: &mut Gen| if g.chance(40) { 0 } else { g.int(1, 4) };
    let (w, h, d) = (dim(g), dim(g), dim(g));
    let mut bt = if g.chance(14) {
        g.pick(&[2i64, 4, 5, -1])
    } else {
        g.pick(&[0i64, 1, 3])
    };
    let big = g.chance(3);
    let facs: Vec<f32> = (0..nf.max(1))
        .map(|_| {
            if g.chance(2) {
                0.0
            } else {
                g.pick(&[0.5f32, 2.0, 1.0, 1e-3, -1.0, 0.25])
            }
        })
        .collect();
    let mut attrs: Vec<Attr> = vec![
        Attr::ints(g, "DATASET_RANK", &[3, nf, 0, 0, 0, 0, 0, 0]),
        Attr::ints(g, "DATASET_DIMENSIONS", &[w, h, d, 0, 0]),
        Attr::string("TYPESTRING", "3DIM_HEAD_ANAT"),
        Attr::ints(g, "SCENE_DATA", &[0, 0, 0, -999, -999, -999, -999, -999]),
    ];
    let orient: Vec<i64> = match g.below(16) {
        0 => vec![0, 0, 4],
        1 => vec![1, 2, 6],
        _ => {
            let mut axes = [0i64, 2, 4];
            for i in (1..3).rev() {
                let j = g.below(i + 1) as usize;
                axes.swap(i as usize, j);
            }
            axes.iter().map(|&a| a + g.int(0, 1)).collect()
        }
    };
    attrs.push(Attr::ints(g, "ORIENT_SPECIFIC", &orient));
    let ori: Vec<f32> = (0..3).map(|_| g.int(-120, 120) as f32 * 0.5).collect();
    attrs.push(Attr::floats(g, "ORIGIN", &ori));
    let del: Vec<f32> = (0..3)
        .map(|_| g.pick(&[1.0f32, -1.0, 0.9375, 2.0, -1.5]))
        .collect();
    attrs.push(Attr::floats(g, "DELTA", &del));
    attrs.push(Attr::string("IDCODE_STRING", "XYZ_abcdefghijklmnopqrstuv"));
    let order = if big { "MSB_FIRST" } else { "LSB_FIRST" };
    attrs.push(Attr::string("BYTEORDER_STRING", order));
    let stats: Vec<f32> = (0..2 * nf.max(1)).map(|k| (k * 7 - 3) as f32).collect();
    attrs.push(Attr::floats(g, "BRICK_STATS", &stats));
    let types: Vec<i64> = (0..nf.max(1)).map(|_| bt).collect();
    attrs.push(Attr::ints(g, "BRICK_TYPES", &types));
    attrs.push(Attr::floats(g, "BRICK_FLOAT_FACS", &facs));
    if g.chance(3) {
        attrs.push(Attr::string("HISTORY_NOTE", "made by a test, with spaces"));
    }
    if g.chance(4) {
        attrs.push(Attr::floats(g, "MARKS_XYZ", &[1.0, 2.0, 3.0]));
    }
    // the order: AFNI's mostly, shuffled now and then
    if g.chance(4) {
        for i in (1..attrs.len()).rev() {
            let j = g.below(i as u64 + 1) as usize;
            attrs.swap(i, j);
        }
    }
    // one fault
    let n = attrs.len();
    let k = g.below(n as u64) as usize;
    let mut bare_count = false;
    let mut drop_type = false;
    let mut no_eol = false;
    match g.below(34) {
        0 => {
            attrs.remove(k);
        }
        1 => {
            // another type
            let a = &mut attrs[k];
            a.ty = g
                .pick(&["string-attribute", "float-attribute", "integer-attribute"])
                .into();
            if a.ty == "string-attribute" && !a.body.starts_with('\'') {
                a.body = format!("'{}~", a.body.trim());
                a.count = (a.body.len() - 1).to_string();
            } else if a.ty != "string-attribute" && a.body.starts_with('\'') {
                a.body = " 1 2 3".into();
                a.count = "3".into();
            }
        }
        2 | 3 => {
            // a count larger than the numbers given (the next line is blank)
            let a = &mut attrs[k];
            if !a.body.starts_with('\'') {
                let c: i64 = a.count.parse().unwrap_or(0);
                a.count = (c + g.int(1, 3)).to_string();
            }
        }
        4 | 5 => {
            // a count smaller than the numbers given
            let a = &mut attrs[k];
            if !a.body.starts_with('\'') {
                let c: i64 = a.count.parse().unwrap_or(0);
                a.count = (c - g.int(1, 3).min(c)).to_string();
            }
            if g.chance(3) {
                attrs[k].body.push_str(&" 9".repeat(g.int(1, 12) as usize));
            }
        }
        6 => {
            let a = &mut attrs[k];
            if !a.body.starts_with('\'') {
                a.body.push(' ');
                a.body.push_str(g.pick(&["abc", "1,5", "1.5", ".", "+"]));
            }
        }
        7 => attrs[k].count = g.pick(&["-1", "-2", "-7"]).into(),
        8 => attrs[k].count = "0".into(),
        9 => {
            attrs[k].count = String::new();
            bare_count = true;
        }
        10 => {
            if attrs[k].body.starts_with('\'') {
                attrs[k].body.remove(0);
            }
        }
        11 => {
            if attrs[k].body.ends_with('~') {
                attrs[k].body.pop();
                attrs[k].body.push('x');
            }
        }
        12 | 13 => {
            // long strings over the structure
            let len = g.pick(&[15usize, 16, 17, 20, 30, 50, 66, 67, 68, 90]);
            let s: String = (0..len).map(|i| (b'A' + (i % 26) as u8) as char).collect();
            if let Some(a) = attrs.iter_mut().find(|a| a.name == "TYPESTRING") {
                *a = Attr::string("TYPESTRING", &s);
            }
        }
        14 => {
            let len = g.pick(&[9usize, 10, 11, 12, 20]);
            let s: String = (0..len).map(|_| 'L').collect();
            if let Some(a) = attrs.iter_mut().find(|a| a.name == "BYTEORDER_STRING") {
                *a = Attr::string("BYTEORDER_STRING", &s);
            }
        }
        15 => drop_type = true,
        16 => attrs[k].ty = "double-attribute".into(),
        17 => no_eol = true,
        18 => {
            if let Some(a) = attrs.iter_mut().find(|a| a.name == "BYTEORDER_STRING") {
                *a = Attr::string("BYTEORDER_STRING", "MIDDLE_FIRST");
            }
        }
        19 => {
            // mixed brick types
            if let Some(a) = attrs.iter_mut().find(|a| a.name == "BRICK_TYPES") {
                let c: usize = a.count.parse().unwrap_or(1);
                let v: Vec<String> = (0..c.max(2))
                    .map(|i| if i == 0 { bt.to_string() } else { "1".into() })
                    .collect();
                a.count = v.len().to_string();
                a.body = format!(" {}", v.join(" "));
            }
        }
        20 => {
            // the stats or factors short of the frames
            let nm = g.pick(&["BRICK_STATS", "BRICK_FLOAT_FACS"]);
            if let Some(a) = attrs.iter_mut().find(|a| a.name == nm) {
                a.count = "1".into();
                a.body = " 2".into();
            }
        }
        21 => {
            bt = g.pick(&[2i64, 4, 5, -1, 6]);
            if let Some(a) = attrs.iter_mut().find(|a| a.name == "BRICK_TYPES") {
                let c: usize = a.count.parse().unwrap_or(1);
                a.body = format!(" {}", vec![bt.to_string(); c].join(" "));
            }
        }
        22 => {
            // a numbers line starting with 'type'
            if !attrs[k].body.starts_with('\'') {
                attrs[k].body = format!("type{}", attrs[k].body);
            }
        }
        23 => {
            // CR LF lines
            for a in attrs.iter_mut() {
                a.body = a.body.replace('\n', "\r\n");
            }
        }
        24 => {
            // a long count line: count=12 then a bare count later reads it
            let a = &mut attrs[k];
            a.count = "12x".into();
        }
        _ => {}
    }
    let spaced = !g.chance(5);
    let mut head = String::new();
    for (i, a) in attrs.iter().enumerate() {
        let mut t = a.text(spaced);
        if i == 0 && drop_type {
            t = t.replacen(&format!("type = {}\n", a.ty), "", 1);
            t = t.replacen(&format!("type={}\n", a.ty), "", 1);
        }
        if bare_count && a.count.is_empty() {
            t = t
                .replace("count = \n", "count\n")
                .replace("count=\n", "count\n");
        }
        head.push_str(&t);
    }
    if no_eol {
        while head.ends_with('\n') {
            head.pop();
        }
    }
    entries.push(file_entry("w/a+orig.HEAD", head.as_bytes()));
    // the BRIK
    let bpv = match bt {
        0 => 1,
        1 => 2,
        3 => 4,
        _ => 1,
    };
    let nv = (w * h * d * nf.max(0)) as usize;
    let mut data: Vec<u8> = Vec::new();
    for i in 0..nv {
        match bpv {
            1 => data.push((i * 37 % 251) as u8),
            2 => {
                let v = ((i as i64 * 1009) % 30000 - 15000) as i16;
                data.extend_from_slice(&if big {
                    v.to_be_bytes()
                } else {
                    v.to_le_bytes()
                });
            }
            _ => {
                let v = if g.chance(20) {
                    f32::from_bits(g.pick(&[0x7f80_0001u32, 0xffc0_0000, 0x7f80_0000]))
                } else {
                    (i as f32) * 0.75 - 3.0
                };
                data.extend_from_slice(&if big {
                    v.to_be_bytes()
                } else {
                    v.to_le_bytes()
                });
            }
        }
    }
    match g.below(14) {
        0 => {}
        1 => entries.push(dir_entry("w/a+orig.BRIK")),
        2 => {
            data.truncate(data.len().saturating_sub(bpv));
            entries.push(file_entry("w/a+orig.BRIK", &data));
        }
        3 => {
            data.extend_from_slice(&vec![7u8; bpv * g.int(1, 3) as usize]);
            entries.push(file_entry("w/a+orig.BRIK", &data));
        }
        _ => entries.push(file_entry("w/a+orig.BRIK", &data)),
    }
    let name = match g.below(14) {
        // a name the BRIK's differs from only by case: another file on case-insensitive file
        // systems (Windows, macOS) in the portable cases (relay 40)
        0 if portable => "w/c+orig.brik",
        0 => "w/a+orig.brik",
        1 => "w/a+orig",
        2 => "w/b+orig.BRIK",
        _ => "w/a+orig.BRIK",
    };
    let ty = if g.chance(3) { 7 } else { -1 };
    (name.to_owned(), ty)
}

fn be32(d: &mut Vec<u8>, v: i32) {
    d.extend_from_slice(&v.to_be_bytes());
}

fn curv_case(g: &mut Gen, entries: &mut Vec<String>) -> (String, i64) {
    let mut d = Vec::new();
    let old = g.chance(10);
    let vnum = if g.chance(20) {
        g.pick(&[-1, 0])
    } else {
        g.int(1, 6) as i32
    };
    if old {
        d.extend_from_slice(&(vnum as u32).to_be_bytes()[1..]);
        d.extend_from_slice(&[0, 0, 1]);
    } else {
        d.extend_from_slice(&[0xff, 0xff, 0xff]);
        be32(&mut d, vnum);
        be32(&mut d, g.int(0, 9) as i32);
        be32(&mut d, if g.chance(10) { g.pick(&[0, 2, 3]) } else { 1 });
    }
    for k in 0..vnum.max(0) {
        let v = if g.chance(12) {
            f32::from_bits(g.pick(&[0x7f80_0001u32, 0xffc0_0000, 0x7f80_0000, 0x8000_0000]))
        } else {
            k as f32 * 0.125 - 0.3
        };
        d.extend_from_slice(&v.to_be_bytes());
    }
    if g.chance(6) {
        let cut = g.below(d.len() as u64 + 1) as usize;
        d.truncate(cut);
    }
    let name = g.pick(&["w/lh.thickness", "w/lh.curv", "w/rh.sulc"]);
    if g.chance(20) {
        entries.push(dir_entry(name));
    } else {
        entries.push(file_entry(name, &d));
    }
    let ty = if g.chance(2) { 25 } else { -1 };
    (name.to_owned(), ty)
}

/// A binary colour table (version 1 or 2) of `n` entries, colours from `cols`.
pub(crate) fn ctab(g: &mut Gen, d: &mut Vec<u8>, cols: &[(i32, i32, i32)]) {
    let name = |d: &mut Vec<u8>, s: &[u8]| {
        be32(d, s.len() as i32 + 1);
        d.extend_from_slice(s);
        d.push(0);
    };
    if g.chance(3) {
        be32(d, cols.len().max(1) as i32);
        name(d, b"ct.txt");
        for (i, c) in cols.iter().enumerate() {
            name(d, format!("r{i}").as_bytes());
            for x in [c.0, c.1, c.2, 0] {
                be32(d, x);
            }
        }
    } else {
        be32(d, -2);
        be32(d, cols.len() as i32);
        name(d, b"lut.txt");
        be32(d, cols.len() as i32);
        for (i, c) in cols.iter().enumerate() {
            be32(d, i as i32);
            name(d, format!("label{i}").as_bytes());
            for x in [c.0, c.1, c.2, 0] {
                be32(d, x);
            }
        }
    }
}

fn annot_case(g: &mut Gen, entries: &mut Vec<String>) -> (String, i64) {
    let mut d = Vec::new();
    let num = if g.chance(20) {
        g.pick(&[0, -1])
    } else {
        g.int(1, 6) as i32
    };
    be32(&mut d, num);
    let cols: Vec<(i32, i32, i32)> = (0..g.int(1, 4))
        .map(|i| {
            (
                20 * i as i32 + 5,
                220 - 30 * i as i32,
                (i as i32 * 77) % 256,
            )
        })
        .collect();
    let annot = |c: (i32, i32, i32)| c.0 | (c.1 << 8) | (c.2 << 16);
    for v in 0..num.max(0) {
        let vno = match g.below(30) {
            0 => num,
            1 => -1,
            _ => v,
        };
        be32(&mut d, vno);
        let a = if g.chance(5) {
            0
        } else if g.chance(6) {
            0x0012_3456
        } else {
            annot(cols[g.below(cols.len() as u64) as usize])
        };
        be32(&mut d, a);
    }
    match g.below(10) {
        0 => {}
        1 => be32(&mut d, 7),
        _ => {
            be32(&mut d, 1);
            ctab(g, &mut d, &cols);
        }
    }
    if g.chance(8) {
        let cut = g.below(d.len() as u64 + 1) as usize;
        d.truncate(cut);
    }
    let name = g.pick(&["w/lh.aparc.annot", "w/rh.x.annot"]);
    if g.chance(25) {
        entries.push(dir_entry(name));
    } else if !g.chance(25) {
        entries.push(file_entry(name, &d));
    }
    let ty = if g.chance(2) { 33 } else { -1 };
    (name.to_owned(), ty)
}

/// A write case: piece 17's `vwr` fields, `how` 17 (BRIK) or 36 (NRRD).
fn write_case(g: &mut Gen, mut t: Vec<String>) -> Vec<String> {
    let ty = g.pick(&[7i64, 26]);
    let name = match ty {
        7 => g.pick(&["w/o+orig.BRIK", "w/o.BRIK", "w/no/o.BRIK", "w/d"]),
        _ => g.pick(&["w/o.nrrd", "w/o.nhdr", "w/no/o.nrrd", "w/d"]),
    };
    let mut entries = Vec::new();
    if name == "w/d" {
        entries.push(dir_entry("w/d"));
    }
    t.push("N".into());
    t.push("N".into());
    t.push("0".into());
    t.push("N".into());
    t.push(g.pick(&["0", "0", "10000040"]).into());
    t.push(if entries.is_empty() {
        "-".into()
    } else {
        entries.join(",")
    });
    t.push("0".into());
    t.push((10 + ty).to_string());
    let (_, _, _, _, nf, alloc) = super::piece14::volume14(g, &mut t, true, false);
    t.push("-1".into());
    t.push(g.pick(&[-1i64, 0, 3]).to_string());
    t.push(enc(Some(name.as_bytes())));
    let back = alloc && nf > 0 && g.chance(4);
    t.push(i64::from(back).to_string());
    t
}

pub fn generate(g: &mut Gen, kind: &str, portable: bool) -> Vec<String> {
    let t = vec![kind.to_owned()];
    if kind == "kwr" {
        return write_case(g, t);
    }
    let mut t = t;
    let mut entries = Vec::new();
    let (name, ty) = match kind {
        "afd" | "afh" => afni_case(g, &mut entries, portable),
        "cud" | "cuh" => curv_case(g, &mut entries),
        _ => annot_case(g, &mut entries),
    };
    t.push(if g.chance(10) { "40" } else { "0" }.into()); // Gdiag
    t.push(if g.chance(12) { "0" } else { "1" }.into()); // MRIIO_Strip_Pound
    t.push("0".into()); // piece 6's environment bits
    t.push(ty.to_string());
    let name = if g.chance(10) {
        format!("{name}#{}", g.pick(&["0", "1", "0:1", "1:2"]))
    } else {
        name
    };
    t.push(enc(Some(name.as_bytes())));
    t.push(if entries.is_empty() {
        "-".into()
    } else {
        entries.join(",")
    });
    t
}

/// The port's answer to a piece-22 case: piece 6's `mrd`/`mrh`, or piece 17's `vwr`.
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
    let r = if tk[0] == "kwr" {
        let mut t: Vec<&str> = vec!["vwr"];
        t.extend_from_slice(&tk[1..]);
        super::piece17::run(&t, g, out, err, o, w, extra)
    } else {
        let head = matches!(tk[0], "afh" | "cuh" | "anh");
        let mut t: Vec<&str> = vec![if head { "mrh" } else { "mrd" }];
        t.extend_from_slice(&tk[1..]);
        super::piece6::run_env(&t, g, out, err, o, w, extra, &[])
    };
    for n in g.notices.as_slice() {
        let id = n.defect.id;
        // FreeSurfer reads for ever, writes past its buffers, or depends on the machine or the
        // file system (FS-53: an annotation's vertex number below 0 written before the volume)
        if matches!(
            id,
            "FS-693" | "FS-697" | "FS-709" | "FS-710" | "FS-712" | "FS-715" | "FS-716" | "FS-53"
        ) {
            extra.push("SKIP");
        }
        // FreeSurfer's values from memory contents
        if matches!(id, "FS-695" | "FS-701" | "FS-711" | "FS-713" | "FS-714")
            || (id == "FS-696" && n.detail.contains("memory contents"))
        {
            extra.push("VARY");
        }
    }
    r
}
