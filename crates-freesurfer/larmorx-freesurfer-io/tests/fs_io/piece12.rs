// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The fs.io probe's piece-12 cases (relay 21): `mgi` (`mri_read` of GIFTI files: MRIread's GIFTI
//! branch), `mwr` (`MRIwrite` of one or several volumes: its GIFTI branch, the names it refuses),
//! `lps` (`loadfsPointSet` on JSON files), `hlp` (`outputHelpXml`, FreeSurfer's XML help renderer, on
//! help texts built as the programs' `.help.xml` files are: sections, kinds of arguments, references,
//! CDATA, comments, long words, both encodings, now and then broken). The GIFTI files are built here from the format (data
//! arrays of every intent, type, encoding, byte order and shape, label tables, `TimeStep`), whole
//! or broken; written GIFTI files are printed with their `UserName` value as `U` and each payload
//! decoded and inflated (fs-surfio's `gii_normal`).

use std::io::{Read, Write};
use std::path::Path;

use flate2::Compression;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_io::{mriio, pointset};
use larmorx_freesurfer_mri::mri::{self, Mri};
use larmorx_freesurfer_mri::voxel::mri_set_vox_val;

use super::piece4::{file_entry, setup};
use super::piece10::{Toks, print_table};
use super::piece11::{f32_of, fnv, obytes, opoint, ostr};
use super::{Gen, Out, Work, enc, hex};

/// The piece-12 commands.
pub const KINDS: [&str; 4] = ["mgi", "mwr", "lps", "hlp"];

// ---------------------------------------------------------------- generation

fn b64(b: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::new();
    for c in b.chunks(3) {
        let n = (u32::from(c[0]) << 16)
            | (u32::from(*c.get(1).unwrap_or(&0)) << 8)
            | u32::from(*c.get(2).unwrap_or(&0));
        for k in 0..4 {
            if k <= c.len() {
                s.push(T[((n >> (18 - 6 * k)) & 63) as usize] as char);
            } else {
                s.push('=');
            }
        }
    }
    s
}

fn zlib(b: &[u8]) -> Vec<u8> {
    let mut e = ZlibEncoder::new(Vec::new(), Compression::new(6));
    e.write_all(b).unwrap();
    e.finish().unwrap()
}

/// A value of a data array's type: (its text, its little-endian bytes).
fn value(g: &mut Gen, ty: &str) -> (String, Vec<u8>) {
    match ty {
        "NIFTI_TYPE_UINT8" => {
            let v = g.below(256) as u8;
            (v.to_string(), vec![v])
        }
        "NIFTI_TYPE_INT32" => {
            let v = if g.chance(10) {
                -1
            } else {
                g.int(-5, 40) as i32
            };
            (v.to_string(), v.to_le_bytes().to_vec())
        }
        _ => {
            let v = g.int(-40, 40) as f32 / 4.0;
            (format!("{v}"), v.to_le_bytes().to_vec())
        }
    }
}

/// One `<DataArray>`.
fn data_array(g: &mut Gen, n: usize) -> String {
    let intent = g.pick(&[
        "NIFTI_INTENT_NONE",
        "NIFTI_INTENT_SHAPE",
        "NIFTI_INTENT_TIME_SERIES",
        "NIFTI_INTENT_LABEL",
        "NIFTI_INTENT_NORMAL",
        "NIFTI_INTENT_POINTSET",
        "NIFTI_INTENT_TRIANGLE",
        "NIFTI_INTENT_VECTOR",
        "NIFTI_INTENT_NODE_INDEX",
        "NIFTI_INTENT_SHAPE",
        "NIFTI_INTENT_TIME_SERIES",
    ]);
    let ty = g.pick(&[
        "NIFTI_TYPE_FLOAT32",
        "NIFTI_TYPE_FLOAT32",
        "NIFTI_TYPE_INT32",
        "NIFTI_TYPE_UINT8",
    ]);
    let cols = if g.chance(4) { g.int(1, 3) as usize } else { 0 };
    let total = n * cols.max(1);
    let have = if g.chance(12) {
        g.int(0, total as i64 + 2) as usize
    } else {
        total
    };
    let (mut words, mut bytes) = (Vec::new(), Vec::new());
    for _ in 0..have {
        let (t, b) = value(g, ty);
        words.push(t);
        bytes.extend_from_slice(&b);
    }
    let big = g.chance(5);
    if big {
        let w = match ty {
            "NIFTI_TYPE_UINT8" => 1,
            _ => 4,
        };
        for c in bytes.chunks_mut(w) {
            c.reverse();
        }
    }
    let encoding = g.pick(&["ASCII", "Base64Binary", "GZipBase64Binary"]);
    let data = match encoding {
        "ASCII" => words.join(if g.chance(3) { "\n" } else { " " }),
        "Base64Binary" => b64(&bytes),
        _ => b64(&zlib(&bytes)),
    };
    let dims = if cols > 0 {
        format!("Dimensionality=\"2\" Dim0=\"{n}\" Dim1=\"{cols}\"")
    } else {
        format!("Dimensionality=\"1\" Dim0=\"{n}\"")
    };
    let order = if g.chance(6) {
        "ColumnMajorOrder"
    } else {
        "RowMajorOrder"
    };
    let endian = if big { "BigEndian" } else { "LittleEndian" };
    let meta = match g.below(5) {
        0 => format!(
            "<MetaData><MD><Name><![CDATA[TimeStep]]></Name><Value><![CDATA[{}]]></Value></MD></MetaData>",
            g.pick(&["2.000000", "0.5", "x", "", "1e3"])
        ),
        1 => "<MetaData/>".to_owned(),
        _ => String::new(),
    };
    format!(
        "<DataArray Intent=\"{intent}\" DataType=\"{ty}\" ArrayIndexingOrder=\"{order}\" {dims} Encoding=\"{encoding}\" Endian=\"{endian}\" ExternalFileName=\"\" ExternalFileOffset=\"\">\n{meta}<Data>{data}</Data>\n</DataArray>\n"
    )
}

/// A GIFTI file of data arrays (the vertices' values), whole or broken.
fn gii(g: &mut Gen) -> Vec<u8> {
    if g.chance(25) {
        return (0..g.int(0, 60)).map(|_| g.below(256) as u8).collect();
    }
    let n = g.int(1, 6) as usize;
    let k = g.int(0, 3) as usize;
    let mut s = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    if g.chance(2) {
        s.push_str(
            "<!DOCTYPE GIFTI SYSTEM \"http://www.nitrc.org/frs/download.php/115/gifti.dtd\">\n",
        );
    }
    let count = if g.chance(10) { k + 1 } else { k };
    let version = if g.chance(8) { "1" } else { "1.0" };
    s.push_str(&format!(
        "<GIFTI Version=\"{version}\" NumberOfDataArrays=\"{count}\">\n"
    ));
    if g.chance(3) {
        s.push_str("<LabelTable>\n");
        for key in 0..g.int(1, 4) {
            s.push_str(&format!(
                "<Label Key=\"{}\" Red=\"{}\" Green=\"0.5\" Blue=\"{}\" Alpha=\"1\"><![CDATA[{}]]></Label>\n",
                if g.chance(8) { -1 } else { key },
                g.pick(&["0", "1", "0.25", "0.0039"]),
                g.pick(&["0", "1", "0.75"]),
                g.pick(&["unknown", "cortex", "a b", ""])
            ));
        }
        s.push_str("</LabelTable>\n");
    }
    let same = g.chance(4);
    for _ in 0..k {
        let len = if same || g.chance(2) {
            n
        } else {
            g.int(1, 6) as usize
        };
        s.push_str(&data_array(g, len));
    }
    s.push_str("</GIFTI>\n");
    let mut b = s.into_bytes();
    if g.chance(12) {
        let cut = g.int(0, b.len() as i64) as usize;
        b.truncate(cut);
    }
    b
}

pub(crate) fn fhex(v: f32) -> String {
    format!("{:08x}", v.to_bits())
}

/// A volume: extent, type, frames, the 15 geometry floats, tr, the name suffix, voxels.
pub(crate) fn volume(g: &mut Gen, t: &mut Vec<String>, postfix: bool) {
    let (w, h, d, nf) = (
        g.int(1, 6),
        if g.chance(3) { g.int(1, 3) } else { 1 },
        if g.chance(4) { g.int(1, 2) } else { 1 },
        if g.chance(3) { g.int(1, 3) } else { 1 },
    );
    let ty = g.pick(&[
        mri::MRI_FLOAT,
        mri::MRI_FLOAT,
        mri::MRI_UCHAR,
        mri::MRI_INT,
        mri::MRI_SHORT,
    ]);
    for v in [w, h, d, i64::from(ty), nf] {
        t.push(v.to_string());
    }
    let geom = [
        1.0f32, 1.0, 1.0, -1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, -1.0, 0.0,
    ];
    for v in geom {
        t.push(fhex(if g.chance(10) { 2.0 * v + 0.5 } else { v }));
    }
    for _ in 0..3 {
        t.push(fhex(g.int(-8, 8) as f32 / 2.0));
    }
    t.push(fhex(g.pick(&[0.0, 2000.0, 1.5, 750.0])));
    if postfix && g.chance(2) {
        let p = g.pick(&["_e1", "", "-echo2", "_x"]);
        t.push(g.pick(&[p.len() as i64, 0]).to_string());
        t.push(enc(Some(p.as_bytes())));
    } else {
        t.push("0".into());
        t.push("N".into());
    }
    let k = g.int(0, 8);
    t.push(k.to_string());
    for _ in 0..k {
        t.push(g.int(0, w - 1).to_string());
        t.push(g.int(0, h - 1).to_string());
        t.push(g.int(0, d - 1).to_string());
        t.push(g.int(0, nf - 1).to_string());
        t.push(fhex(g.int(-40, 300) as f32 / 4.0));
    }
}

fn json_number(g: &mut Gen) -> String {
    g.pick(&[
        "1",
        "-2",
        "0.5",
        "1e2",
        "-0",
        "12345678901234567890",
        "1.25E-3",
        "3",
    ])
    .to_owned()
}

fn json_any(g: &mut Gen) -> String {
    match g.below(9) {
        0 => "null".into(),
        1 => "true".into(),
        2 => "\"s\"".into(),
        3 => "[]".into(),
        4 => "{}".into(),
        5 => "[1, 2]".into(),
        _ => json_number(g),
    }
}

/// A point-set file: as `fsPointSet::save` writes it, with fields missing or of other types, or not
/// JSON.
fn point_set_json(g: &mut Gen) -> Vec<u8> {
    if g.chance(10) {
        return g
            .pick(&[
                "",
                "   ",
                "{",
                "[1, 2",
                "{\"vox2ras\": }",
                "nul",
                "\u{feff}{}",
                "{} x",
                "\"a\\q\"",
            ])
            .as_bytes()
            .to_vec();
    }
    let mut s = String::from("{\n");
    if g.chance(6) {
        s.push_str("    \"data_type\": \"fs_pointset\",\n");
    }
    let mut fields = Vec::new();
    if !g.chance(8) {
        let v = if g.chance(8) {
            json_any(g)
        } else {
            format!("\"{}\"", g.pick(&["scanner_ras", "tkreg", "", "voxel"]))
        };
        fields.push(format!("    \"vox2ras\": {v}"));
    }
    if !g.chance(10) {
        let mut pts = Vec::new();
        for _ in 0..g.int(0, 4) {
            if g.chance(12) {
                pts.push(json_any(g));
                continue;
            }
            let mut c = Vec::new();
            for axis in ["x", "y", "z"] {
                if !g.chance(12) {
                    let v = if g.chance(10) {
                        json_any(g)
                    } else {
                        json_number(g)
                    };
                    c.push(format!("\"{axis}\": {v}"));
                }
            }
            let coords = if g.chance(12) {
                json_any(g)
            } else {
                format!("{{{}}}", c.join(", "))
            };
            let mut p = vec![format!("\"coordinates\": {coords}")];
            if !g.chance(10) {
                let v = if g.chance(10) {
                    json_any(g)
                } else {
                    json_number(g)
                };
                p.push(format!("\"legacy_stat\": {v}"));
            }
            if g.chance(6) {
                p.push("\"statistics\": {\"a\": 1}".into());
            }
            pts.push(format!("{{{}}}", p.join(", ")));
        }
        let v = match g.below(12) {
            0 => json_any(g),
            1 => format!(
                "{{\"b\": {}, \"a\": {}}}",
                pts.first().map_or("1", |s| s),
                "{}"
            ),
            _ => format!("[{}]", pts.join(",\n")),
        };
        fields.push(format!("    \"points\": {v}"));
    }
    if g.chance(2) {
        fields.reverse();
    }
    s.push_str(&fields.join(",\n"));
    s.push_str("\n}\n");
    if g.chance(10) {
        s = format!("[{s}]");
    }
    s.into_bytes()
}

/// Words of a help text: plain, with entity and character references, tabs, line ends, the word
/// separators `_` and `/`, long words around a line's width, non-ASCII letters.
fn help_word(g: &mut Gen, latin1: bool) -> Vec<u8> {
    match g.below(30) {
        0 => b"&lt;in&gt;".to_vec(),
        1 => b"&amp;".to_vec(),
        2 => g
            .pick(&[
                &b"&#38;"[..],
                b"&#62;",
                b"&#09;",
                b"&quot;",
                b"&apos;",
                b"&#x41;",
            ])
            .to_vec(),
        3 => b"\n".to_vec(),
        4 => b"\t".to_vec(),
        5 => g
            .pick(&[
                &b"a_b"[..],
                b"/usr/local/freesurfer",
                b"--opt-name",
                b"x/y_z",
                b"_",
                b"/",
            ])
            .to_vec(),
        6 => {
            let n = g.int(55, 80) as usize;
            (0..n).map(|k| b"abcdefghij"[k % 10]).collect()
        }
        7 => {
            if latin1 {
                vec![b'r', 0xE9, b's']
            } else {
                "r\u{e9}sum\u{e9}".as_bytes().to_vec()
            }
        }
        8 => b"<![CDATA[x < y & z]]>".to_vec(),
        9 => b"<!-- note -->".to_vec(),
        10 => b"<b>bold</b>".to_vec(),
        11 => b" ".to_vec(),
        _ => g
            .pick(&[
                &b"the"[..],
                b"input",
                b"volume",
                b"file",
                b"is",
                b"written",
                b"to",
                b"output",
                b"with",
                b"default",
                b"surface",
                b"mri_convert",
                b"--i",
                b"1",
                b"(see",
                b"below)",
            ])
            .to_vec(),
    }
}

fn help_text(g: &mut Gen, latin1: bool, max: i64) -> Vec<u8> {
    let mut t = Vec::new();
    if g.chance(6) {
        t.push(b' ');
    }
    let n = if g.chance(30) {
        0
    } else {
        g.int(1, max.max(1))
    };
    for k in 0..n {
        if k > 0 {
            t.push(if g.chance(12) { b'\n' } else { b' ' });
        }
        t.extend(help_word(g, latin1));
    }
    t
}

fn help_tag(g: &mut Gen, name: &str) -> String {
    match g.below(12) {
        0 => name.to_ascii_uppercase(),
        1 => {
            let mut c = name.as_bytes().to_vec();
            c[0] = c[0].to_ascii_uppercase();
            String::from_utf8(c).unwrap()
        }
        _ => name.to_owned(),
    }
}

fn help_elem(g: &mut Gen, name: &str, latin1: bool, max: i64, out: &mut Vec<u8>) {
    let tag = help_tag(g, name);
    if g.chance(150) {
        out.extend_from_slice(format!("<{tag}/>").as_bytes());
        return;
    }
    let attr = if g.chance(15) {
        " id=\"a &amp; b\""
    } else {
        ""
    };
    out.extend_from_slice(format!("<{tag}{attr}>").as_bytes());
    out.extend(help_text(g, latin1, max));
    out.extend_from_slice(format!("</{tag}>").as_bytes());
}

fn blank(g: &mut Gen) -> &'static [u8] {
    g.pick(&[&b"\n"[..], b"\n  ", b"", b"\r\n", b"\n\t"])
}

/// A program's help text as FreeSurfer's `.help.xml` files write it, now and then broken.
fn help_doc(g: &mut Gen) -> Vec<u8> {
    let latin1 = g.chance(2);
    let mut d = Vec::new();
    if g.chance(4) {
        d.extend_from_slice(if latin1 {
            b"<?xml version=\"1.0\" encoding=\"ISO-8859-1\"?>\n"
        } else {
            b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n"
        });
        d.extend_from_slice(b"<!DOCTYPE help [\n<!ENTITY lt \"#38;#60;\">\n<!ENTITY gt \"&#62;\">\n<!ENTITY amp \"&#38;#38;\">\n]>\n");
    } else if latin1 {
        d.extend_from_slice(b"<?xml version=\"1.0\" encoding=\"ISO-8859-1\"?>\n");
    }
    if g.chance(10) {
        d.extend_from_slice(b"<!-- a comment -->\n");
    }
    let root = match g.below(20) {
        0 => "other",
        1 => "HELP",
        _ => "help",
    };
    d.extend_from_slice(format!("<{root}>").as_bytes());
    for _ in 0..g.int(0, 7) {
        d.extend_from_slice(blank(g));
        let s = g.pick(&[
            "name",
            "synopsis",
            "description",
            "arguments",
            "arguments",
            "outputs",
            "example",
            "example",
            "bugs",
            "reporting",
            "see-also",
            "references",
            "text",
            "cost_function",
            "mutually-exclusive",
            "usage",
        ]);
        if s == "arguments" {
            let at = help_tag(g, s);
            d.extend_from_slice(format!("<{at}>").as_bytes());
            for _ in 0..g.int(0, 3) {
                d.extend_from_slice(blank(g));
                if g.chance(25) {
                    d.extend_from_slice(g.pick(&[&b"<!-- c -->"[..], b"<?pi x?>"]));
                    continue;
                }
                let kind = g.pick(&[
                    "required-flagged",
                    "optional-flagged",
                    "positional",
                    "required",
                    "optional",
                    "text",
                ]);
                let kt = help_tag(g, kind);
                d.extend_from_slice(format!("<{kt}>").as_bytes());
                if g.chance(4) {
                    help_elem(g, "intro", latin1, 12, &mut d);
                }
                for _ in 0..g.int(0, 3) {
                    d.extend_from_slice(blank(g));
                    help_elem(g, "argument", latin1, 4, &mut d);
                    d.extend_from_slice(blank(g));
                    help_elem(g, "explanation", latin1, 30, &mut d);
                }
                d.extend_from_slice(format!("</{kt}>").as_bytes());
            }
            d.extend_from_slice(format!("</{at}>").as_bytes());
        } else if s == "outputs" {
            d.extend_from_slice(b"<outputs>");
            if g.chance(3) {
                help_elem(g, "intro", latin1, 10, &mut d);
            }
            for _ in 0..g.int(0, 3) {
                d.extend_from_slice(blank(g));
                help_elem(g, "output", latin1, 4, &mut d);
                help_elem(g, "explanation", latin1, 20, &mut d);
            }
            d.extend_from_slice(b"</outputs>");
        } else {
            help_elem(g, s, latin1, 40, &mut d);
        }
    }
    d.extend_from_slice(blank(g));
    d.extend_from_slice(format!("</{root}>\n").as_bytes());
    if g.chance(15) {
        let cut = g.int(0, d.len() as i64) as usize;
        d.truncate(cut);
    }
    d
}

pub fn generate(g: &mut Gen, kind: &str, _portable: bool) -> Vec<String> {
    let mut t = vec![kind.to_owned()];
    match kind {
        "mgi" => {
            t.push(if g.chance(10) { "40" } else { "0" }.into());
            t.push(if g.chance(10) { "0" } else { "1" }.into());
            t.push(g.pick(&[-1, -1, -1, 27, 3]).to_string());
            t.push(if g.chance(8) { "0" } else { "1" }.into());
            let name = match g.below(14) {
                0 => "w/b.gii".to_owned(),
                1 => "w/a.gii#0".to_owned(),
                2 => "w/a.gii#1".to_owned(),
                3 => "w/a.gii#0:1".to_owned(),
                4 => "w/a.gii@gii".to_owned(),
                5 => "w/a.GII".to_owned(),
                _ => "w/a.gii".to_owned(),
            };
            t.push(enc(Some(name.as_bytes())));
            let file = if name == "w/a.GII" {
                "w/a.GII"
            } else {
                "w/a.gii"
            };
            t.push(file_entry(file, &gii(g)));
        }
        "mwr" => {
            t.push(if g.chance(10) { "40" } else { "0" }.into());
            let name = match g.below(16) {
                0 => "w/d.gii",
                1 => "w/no/o.gii",
                2 => "w/o.mgz",
                3 => "w/o.xyz",
                4 => "w/o",
                5 => "w/o.gii.gz",
                6 => "w/O.GII",
                7 => "w/x.y/o.gii",
                _ => "w/o.gii",
            };
            let spec = match name {
                "w/d.gii" => format!("D{}", hex(b"w/d.gii")),
                "w/x.y/o.gii" => format!("D{}", hex(b"w/x.y")),
                _ => "-".to_owned(),
            };
            t.push(spec);
            let nv = if g.chance(5) { g.int(2, 3) } else { 1 };
            t.push(nv.to_string());
            t.push(if nv > 1 || g.chance(8) { "1" } else { "0" }.into());
            for _ in 0..nv {
                volume(g, &mut t, nv > 1);
            }
            t.push(enc(Some(name.as_bytes())));
        }
        "hlp" => {
            if g.chance(30) {
                t.push("N".into());
            } else {
                t.push(enc(Some(&help_doc(g))));
            }
        }
        _ => {
            let name = g.pick(&["w/p.json", "w/p.json", "w/p.json", "w/none.json", "w"]);
            let spec = if name == "w/p.json" {
                file_entry(name, &point_set_json(g))
            } else {
                "-".to_owned()
            };
            t.push(spec);
            t.push(enc(Some(name.as_bytes())));
        }
    }
    t
}

// ---------------------------------------------------------------- the port's answers

fn b64val(c: u8) -> Option<u32> {
    Some(match c {
        b'A'..=b'Z' => c - b'A',
        b'a'..=b'z' => c - b'a' + 26,
        b'0'..=b'9' => c - b'0' + 52,
        b'+' => 62,
        b'/' => 63,
        _ => return None,
    } as u32)
}

fn find(s: &[u8], pat: &[u8], from: usize) -> Option<usize> {
    if from > s.len() {
        return None;
    }
    s[from..]
        .windows(pat.len())
        .position(|w| w == pat)
        .map(|p| p + from)
}

/// fs-surfio's `gii_normal` (track D's relay 22): the `UserName` value `U`, each `<Data>` payload
/// Base64-decoded and inflated, `Z` and its bytes in hex (`B` and the payload when it does not
/// inflate).
fn gii_normal(f: &[u8]) -> Vec<u8> {
    let mut s = f.to_vec();
    if let Some(u) = find(&s, b"<![CDATA[UserName]]></Name>", 0)
        && let Some(v) = find(&s, b"<Value><![CDATA[", u)
    {
        let v = v + 16;
        if let Some(e) = find(&s, b"]]>", v) {
            s.splice(v..e, b"U".iter().copied());
        }
    }
    let mut r = Vec::new();
    let mut p = 0;
    loop {
        let a = find(&s, b"<Data>", p);
        let b = a.and_then(|a| find(&s, b"</Data>", a + 6));
        let (Some(a), Some(b)) = (a, b) else {
            r.extend_from_slice(&s[p..]);
            break;
        };
        let a = a + 6;
        r.extend_from_slice(&s[p..a]);
        let pay = &s[a..b];
        let mut bin = Vec::new();
        let (mut acc, mut nb) = (0u32, 0);
        for &c in pay {
            let Some(v) = b64val(c) else { continue };
            acc = (acc << 6) | v;
            nb += 6;
            if nb >= 8 {
                nb -= 8;
                bin.push(((acc >> nb) & 0xff) as u8);
            }
        }
        let mut inf = Vec::new();
        if ZlibDecoder::new(&bin[..]).read_to_end(&mut inf).is_ok() {
            r.push(b'Z');
            for c in inf {
                r.extend_from_slice(format!("{c:02x}").as_bytes());
            }
        } else {
            r.push(b'B');
            r.extend_from_slice(pay);
        }
        p = b;
    }
    r
}

/// A gzip stream (an `.mgz`: zlib-rs's compressed bytes here, zlib 1.2.11's in `fsio`; the stream
/// they inflate to is the same): `gz:` and the inflated bytes when it inflates whole to its end
/// (track A relay 23), else the bytes as they are.
pub(crate) fn gz_normal(b: &[u8]) -> Vec<u8> {
    if b.len() < 2 || b[0] != 0x1f || b[1] != 0x8b {
        return b.to_vec();
    }
    let mut d = flate2::bufread::GzDecoder::new(b);
    let mut inf = Vec::new();
    if d.read_to_end(&mut inf).is_err() || !d.into_inner().is_empty() {
        return b.to_vec();
    }
    let mut r = b"gz:".to_vec();
    r.extend_from_slice(&inf);
    r
}

/// Every file under `w`: its name and bytes (a `.gii` file, in any case, normalised; a gzip stream
/// inflated), FNV-1a 64 beyond 256 bytes.
pub(crate) fn odir(o: &mut Out, w: &Work) {
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
    for f in files {
        ostr(o, Some(f.as_bytes()));
        let mut b = std::fs::read(w.fix.join(&f)).unwrap_or_default();
        if f.to_ascii_lowercase().ends_with(".gii") {
            b = gii_normal(&b);
        }
        b = gz_normal(&b);
        if b.len() <= 256 {
            obytes(o, &b);
        } else {
            fnv(o, &b);
        }
    }
    o.0.push_str("| ");
}

pub(crate) fn read_volume(g: &mut Globals, tk: &mut Toks) -> Mri {
    let v: Vec<i32> = (0..5).map(|_| tk.int() as i32).collect();
    let (mut s1, mut s2) = (Vec::new(), Vec::new());
    let mut m = mri::mri_alloc_sequence(g, &mut s1, &mut s2, v[0], v[1], v[2], v[3], v[4]).unwrap();
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
    m.tr = f32_of(tk.next());
    m.len_fname_post_fixes = tk.int() as i32;
    if let Some(p) = super::dec(tk.next()) {
        m.fname_post_fixes = String::from_utf8(p).unwrap();
    }
    for _ in 0..tk.int() {
        let (c, r, s, fr) = (
            tk.int() as i32,
            tk.int() as i32,
            tk.int() as i32,
            tk.int() as i32,
        );
        let val = f32_of(tk.next());
        mri_set_vox_val(g, &mut s2, &mut m, c, r, s, fr, val, 0).unwrap();
    }
    m
}

/// The port's answer to a piece-12 case (`Err`: the exit status).
pub fn run(
    tk: &[&str],
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    o: &mut Out,
    w: &Work,
    extra: &mut Vec<&'static str>,
) -> Result<(), u8> {
    let s = super::Shared::default();
    let (mut o2, mut e2) = (s.clone(), s.clone());
    let r = answer(tk, g, &mut o2, &mut e2, o, w);
    let text = s.0.borrow().clone();
    let _ = out.write_all(&text);
    let _ = err;
    // writers not ported yet; help texts the port does not read as libxml2 does
    if find(&text, b"is not ported yet", 0).is_some()
        || find(&text, b"libxml2's messages are not reproduced", 0).is_some()
        || find(&text, b"XML the port does not read", 0).is_some()
    {
        extra.push("SKIP");
    }
    for n in g.notices.as_slice() {
        // FreeSurfer crashes, writes outside its buffers or reads memory it never wrote there
        // (piece 6's and fs-surfio's GIFTI reader's): counted, never compared
        if matches!(
            n.defect.id,
            "FS-249"
                | "FS-253"
                | "FS-254"
                | "FS-255"
                | "FS-256"
                | "FS-257"
                | "FS-258"
                | "FS-13"
                | "FS-76"
                | "FS-382"
                | "FS-383"
                | "FS-384"
                | "FS-386"
                | "FS-388"
                | "FS-389"
                | "FS-391"
                | "FS-392"
                | "FS-53"
                | "FS-521"
                | "FS-434"
        ) {
            extra.push("SKIP");
        }
        // memory contents, or an address in a message (FS-214)
        if matches!(n.defect.id, "FS-49" | "FS-167" | "FS-214") {
            extra.push("VARY");
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
    match tk.t[0] {
        "mgi" => {
            g.gdiag = u64::from_str_radix(tk.next(), 16).unwrap();
            g.mriio_strip_pound = tk.int() as i32;
            let ty = tk.int() as i32;
            let vf = tk.int() != 0;
            let name = tk.string();
            setup(w, tk.next());
            g.errno = 0;
            let env = Environment::default();
            let m =
                mriio::mri_read(g, out, err, &env, dir, &name, ty, vf, -1, -1).map_err(fatal)?;
            super::piece6::print_mri(o, w, m.as_ref());
            print_table(o, m.as_ref().and_then(|m| m.ct.as_ref()), false, false);
            o.i(i64::from(g.errno));
        }
        "hlp" => {
            let text = super::dec(tk.next());
            let r =
                larmorx_freesurfer_core::help_xml::output_help_xml(g, out, err, text.as_deref());
            o.i(i64::from(r));
        }
        "mwr" => {
            g.gdiag = u64::from_str_radix(tk.next(), 16).unwrap();
            // a user name on every platform (Linux's comes from the password database), so the
            // GIFTI file has its UserName everywhere
            let env = Environment::from_pairs([
                ("FREESURFER_REPLACEMENT_FOR_CREATION_TIME_STRING", "T"),
                ("USER", "U"),
            ]);
            setup(w, tk.next());
            let nv = tk.int();
            let usevec = tk.int() != 0;
            let mut vs: Vec<Mri> = (0..nv).map(|_| read_volume(g, &mut tk)).collect();
            let name = tk.string();
            g.errno = 0;
            // FreeSurfer's first volume is the vector's first: with several, only the vector's are
            // written (a copy stands for the first); with one, the first itself
            let r = if usevec && vs.len() > 1 {
                let (mut s1, mut s2) = (Vec::new(), Vec::new());
                let mut first =
                    mri::mri_alloc_header(g, &mut s1, &mut s2, 1, 1, 1, mri::MRI_UCHAR, 1).unwrap();
                mriio::mri_write(
                    g,
                    out,
                    err,
                    &env,
                    dir,
                    &mut first,
                    &name,
                    Some(&mut vs[..]),
                    -1,
                )
            } else {
                mriio::mri_write(g, out, err, &env, dir, &mut vs[0], &name, None, -1)
            }
            .map_err(fatal)?;
            o.i(i64::from(r));
            o.i(i64::from(g.errno));
            odir(o, w);
        }
        _ => {
            setup(w, tk.next());
            let name = tk.string();
            g.errno = 0;
            let ps = pointset::load_fs_point_set(g, err, dir, &name).map_err(fatal)?;
            let n = ps
                .vox2ras
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(ps.vox2ras.len());
            ostr(o, Some(&ps.vox2ras[..n]));
            o.i(i64::from(ps.npoints()));
            for p in &ps.points {
                opoint(o, p);
            }
        }
    }
    o.i(i64::from(g.gerror));
    Ok(())
}
