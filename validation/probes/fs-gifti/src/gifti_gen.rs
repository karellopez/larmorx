// SPDX-License-Identifier: Apache-2.0
//! Generated GIFTI files: every element and attribute gifticlib reads, every data type in every
//! encoding and byte order, external data, label tables, coordinate systems, metadata; odd values,
//! misplaced elements, bad Base64, cut and mutated files; and the library's settings.

use crate::xmlgen::Gen;
use base64_lite::encode;
use std::io::Write;

/// A tiny Base64 encoder (the standard alphabet with '=' padding, as gifticlib's decoder reads).
mod base64_lite {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    pub fn encode(b: &[u8]) -> String {
        let mut s = String::new();
        for c in b.chunks(3) {
            let n = (c[0] as u32) << 16
                | (*c.get(1).unwrap_or(&0) as u32) << 8
                | *c.get(2).unwrap_or(&0) as u32;
            s.push(T[(n >> 18) as usize & 63] as char);
            s.push(T[(n >> 12) as usize & 63] as char);
            s.push(if c.len() > 1 {
                T[(n >> 6) as usize & 63] as char
            } else {
                '='
            });
            s.push(if c.len() > 2 {
                T[n as usize & 63] as char
            } else {
                '='
            });
        }
        s
    }
}

pub struct Case {
    pub file: Vec<u8>,
    pub ext: Vec<(String, Vec<u8>)>,
    /// verb, b64_check, buf_size, update_ok, zlevel, indent
    pub set: [i32; 6],
    pub read_data: bool,
    pub dalist: Vec<i32>,
    pub nfiles: i32,
    pub disp: bool,
}

pub const TYPES: &[(&str, usize)] = &[
    ("NIFTI_TYPE_UINT8", 1),
    ("NIFTI_TYPE_INT16", 2),
    ("NIFTI_TYPE_INT32", 4),
    ("NIFTI_TYPE_FLOAT32", 4),
    ("NIFTI_TYPE_COMPLEX64", 8),
    ("NIFTI_TYPE_FLOAT64", 8),
    ("NIFTI_TYPE_RGB24", 3),
    ("NIFTI_TYPE_INT8", 1),
    ("NIFTI_TYPE_UINT16", 2),
    ("NIFTI_TYPE_UINT32", 4),
    ("NIFTI_TYPE_INT64", 8),
    ("NIFTI_TYPE_UINT64", 8),
    ("NIFTI_TYPE_FLOAT128", 16),
    ("NIFTI_TYPE_COMPLEX128", 16),
    ("NIFTI_TYPE_COMPLEX256", 32),
];
const INTENTS: &[&str] = &[
    "NIFTI_INTENT_POINTSET",
    "NIFTI_INTENT_TRIANGLE",
    "NIFTI_INTENT_NONE",
    "NIFTI_INTENT_TIME_SERIES",
    "NIFTI_INTENT_LABEL",
    "NIFTI_INTENT_SHAPE",
    "NIFTI_INTENT_NODE_INDEX",
    "NIFTI_INTENT_VECTOR",
    "NIFTI_INTENT_GENMATRIX",
    "NIFTI_INTENT_TTEST",
    "NIFTI_INTENT_ESTIMATE",
    "NIFTI_INTENT_RGB_VECTOR",
    "NIFTI_INTENT_QUATERNION",
    "NIFTI_INTENT_DIMLESS",
    "NIFTI_INTENT_FTEST_NONC",
    "BOGUS",
    "",
];

impl Gen {
    fn num_text(&mut self, float: bool) -> String {
        if !float {
            return match self.below(12) {
                0 => "-1".into(),
                1 => "300".into(),
                2 => "65536".into(),
                3 => "4294967296".into(),
                4 => "-9223372036854775809".into(),
                5 => "18446744073709551616".into(),
                6 => "0x1f".into(),
                7 => "+7".into(),
                8 => "1.5".into(),
                _ => format!("{}", self.below(200) as i64 - 50),
            };
        }
        let v = f64::from_bits(self.rng.next_u64());
        match self.below(14) {
            0 => format!("{v:e}"),
            1 => larmorx_core::cfmt::g(v, 9),
            2 => larmorx_core::cfmt::g(v, 17),
            3 => self
                .pick(&[
                    "inf", "-inf", "nan", "-nan", "NaN", "Infinity", "1e400", "-1e-400",
                    "4.9e-324", "0x1.8p3", "-0", "1e", ".",
                ])
                .into(),
            4 => larmorx_core::cfmt::f((self.below(2_000_000) as f64 - 1e6) / 1000.0, 6),
            5 => format!("{}", (self.below(100_000) as f64) / 7.0),
            _ => larmorx_core::cfmt::f((self.below(20_000) as f64 - 1e4) / 100.0, 3),
        }
    }

    fn ascii_data(&mut self, tname: &str, nvals: usize) -> String {
        let float = tname.contains("FLOAT") || tname.contains("COMPLEX");
        let per = if tname.contains("COMPLEX") {
            2
        } else if tname.contains("RGB24") {
            3
        } else {
            1
        };
        let mut n = nvals * per;
        match self.below(8) {
            0 => n = n.saturating_sub(1 + self.below(3) as usize),
            1 => n += 1 + self.below(3) as usize,
            _ => {}
        }
        let mut s = String::new();
        for k in 0..n {
            if k > 0 {
                s.push_str(self.pick(&[" ", "\n", "  ", "\t", "\r\n", " \n "]));
            }
            s += &self.num_text(float);
            if self.chance(200) {
                s.push_str(self.pick(&["x", ",", "abc"]));
            }
        }
        if self.chance(3) {
            s.push('\n');
        }
        s
    }

    fn cdata_or_text(&mut self, s: &str) -> String {
        // text after a CDATA section crashes gifticlib (FS-86): rare
        match self.below(80) {
            0 => format!(" <![CDATA[{s}]]> "),
            1 => format!("<![CDATA[{s}]]><![CDATA[2]]>"),
            2..=29 => s.replace('&', "&amp;").replace('<', "&lt;"),
            30..=34 => format!(" <![CDATA[{s}]]>"),
            _ => format!("<![CDATA[{s}]]>"),
        }
    }

    fn meta(&mut self, out: &mut String, ind: &str) {
        out.push_str(&format!("{ind}<MetaData>\n"));
        for k in 0..self.below(5) {
            out.push_str(&format!("{ind}   <MD>\n"));
            if !self.chance(40) {
                let n = self
                    .pick(&[
                        "Name",
                        "AnatomicalStructurePrimary",
                        "UniqueID",
                        "gifticlib-version",
                        "Date",
                        "",
                        "a b",
                        "TimeStep",
                    ])
                    .to_string();
                let n = self.cdata_or_text(&n);
                out.push_str(&format!("{ind}      <Name>{n}</Name>\n"));
            }
            if !self.chance(30) {
                let v = format!("v{k}{}", self.pick(&["", " x", "\u{e9}", "&", "1.0"]));
                let v = self.cdata_or_text(&v);
                out.push_str(&format!("{ind}      <Value>{v}</Value>\n"));
            }
            if self.chance(30) {
                out.push_str(&format!("{ind}      <Value>dup</Value>\n"));
            }
            out.push_str(&format!("{ind}   </MD>\n"));
        }
        out.push_str(&format!("{ind}</MetaData>\n"));
    }

    fn label_table(&mut self, out: &mut String) {
        out.push_str("   <LabelTable>\n");
        let all_rgba = self.chance(2);
        for k in 0..self.below(6) {
            let key = if self.chance(4) { "Index" } else { "Key" };
            let mut a = format!(
                " {key}=\"{}\"",
                if self.chance(20) {
                    "x".into()
                } else {
                    format!("{}", k as i64 * 3 - 1)
                }
            );
            if all_rgba || self.chance(3) {
                for c in ["Red", "Green", "Blue", "Alpha"] {
                    if all_rgba || !self.chance(25) {
                        let v = if self.chance(40) {
                            "x"
                        } else {
                            self.pick(&[
                                "0", "1", "0.5", "0.333333", "1e-3", "2", "-1", "nan", "0.25 ",
                                "1e-50",
                            ])
                        };
                        a += &format!(" {c}=\"{v}\"");
                    }
                }
            }
            if self.chance(50) {
                a += " Other=\"1\"";
            }
            let name = format!("label{k}");
            let text = if self.chance(10) {
                String::new()
            } else {
                self.cdata_or_text(&name)
            };
            out.push_str(&format!("      <Label{a}>{text}</Label>\n"));
        }
        out.push_str("   </LabelTable>\n");
    }

    fn coordsys(&mut self, out: &mut String) {
        out.push_str("      <CoordinateSystemTransformMatrix>\n");
        let spaces = [
            "NIFTI_XFORM_UNKNOWN",
            "NIFTI_XFORM_TALAIRACH",
            "NIFTI_XFORM_SCANNER_ANAT",
            "NIFTI_XFORM_MNI_152",
            "",
        ];
        if !self.chance(10) {
            let s = self.pick(&spaces).to_string();
            let s = self.cdata_or_text(&s);
            out.push_str(&format!("         <DataSpace>{s}</DataSpace>\n"));
        }
        if !self.chance(10) {
            let s = self.pick(&spaces).to_string();
            let s = self.cdata_or_text(&s);
            out.push_str(&format!(
                "         <TransformedSpace>{s}</TransformedSpace>\n"
            ));
        }
        if !self.chance(10) {
            let n = match self.below(6) {
                0 => 15,
                1 => 17,
                _ => 16,
            };
            let mut s = String::new();
            for k in 0..n {
                s.push_str(if k % 4 == 0 { "\n" } else { " " });
                s += &self.num_text(true);
            }
            out.push_str(&format!(
                "         <MatrixData>{s}\n         </MatrixData>\n"
            ));
        }
        out.push_str("      </CoordinateSystemTransformMatrix>\n");
    }

    /// One `DataArray` (its external data, when it has some, appended to `ext`).
    fn darray(&mut self, out: &mut String, ext: &mut Vec<(String, Vec<u8>)>, idx: usize) {
        let ti = self.below(TYPES.len() as u64 + 1) as usize;
        let (tname, nb) = if ti == TYPES.len() {
            ("NIFTI_TYPE_BOGUS", 4)
        } else {
            TYPES[ti]
        };
        let ndim = [1usize, 2, 2, 2, 3, 0, 6, 7][self.below(8) as usize];
        let mut dims = [0usize; 6];
        for d in dims.iter_mut().take(ndim.min(6)) {
            *d = [1usize, 2, 3, 4, 5, 7, 0, 33][self.below(8) as usize];
        }
        // at most 65,536 values declared (both sides allocate what the dimensions say)
        while dims.iter().take(ndim.min(6)).product::<usize>() > 65536 {
            let k = (0..6).max_by_key(|&k| dims[k]).unwrap_or(0);
            dims[k] /= 2;
        }
        let nvals: usize = dims.iter().take(ndim.min(6)).product::<usize>().min(4096);
        let enc = [
            "ASCII",
            "Base64Binary",
            "GZipBase64Binary",
            "ExternalFileBinary",
            "Bogus",
        ][match self.below(20) {
            0..=4 => 0,
            5..=9 => 1,
            10..=15 => 2,
            16..=18 => 3,
            _ => 4,
        }];
        let big = self.chance(3);
        let mut a = String::new();
        let intent = self.pick(INTENTS);
        if !self.chance(30) {
            a += &format!(" Intent=\"{intent}\"");
        }
        if !self.chance(30) {
            a += &format!(" DataType=\"{tname}\"");
        }
        if !self.chance(20) {
            a += &format!(
                " ArrayIndexingOrder=\"{}\"",
                self.pick(&["RowMajorOrder", "ColumnMajorOrder", "Bogus"])
            );
        }
        a += &format!(" Dimensionality=\"{ndim}\"");
        for k in 0..ndim.min(7) {
            let d = if k < 6 { dims[k] } else { 1 };
            a += &format!(" Dim{k}=\"{d}\"");
        }
        if self.chance(10) {
            a += " Dim5=\"2\"";
        }
        a += &format!(" Encoding=\"{enc}\"");
        if !self.chance(20) {
            a += &format!(
                " Endian=\"{}\"",
                if big { "BigEndian" } else { "LittleEndian" }
            );
        }
        // the values' bytes (little-endian unless big)
        let mut raw = vec![0u8; nvals * nb];
        for b in raw.iter_mut() {
            *b = self.rng.next_u64() as u8;
        }
        if tname.contains("FLOAT128") || tname.contains("COMPLEX256") {
            // x87 extended values in 16 bytes, the top 6 zero (gifticlib's long double)
            for c in raw.chunks_mut(16) {
                for b in &mut c[10..] {
                    *b = 0;
                }
            }
        }
        let swapsize = match tname {
            "NIFTI_TYPE_COMPLEX64" => 4,
            "NIFTI_TYPE_COMPLEX128" => 8,
            "NIFTI_TYPE_COMPLEX256" | "NIFTI_TYPE_FLOAT128" => 16,
            "NIFTI_TYPE_RGB24" | "NIFTI_TYPE_UINT8" | "NIFTI_TYPE_INT8" => 0,
            _ => nb,
        };
        let disk: Vec<u8> = if big && swapsize > 1 {
            raw.chunks(swapsize)
                .flat_map(|c| c.iter().rev().copied())
                .collect()
        } else {
            raw.clone()
        };
        let data = match enc {
            "ASCII" => self.ascii_data(tname, nvals),
            "Base64Binary" | "Bogus" => {
                let mut s = encode(&disk);
                if self.chance(8) {
                    // a character outside the alphabet, or a cut
                    let at = self.below(s.len() as u64 + 1) as usize;
                    s.insert_str(at, self.pick(&["\n", " ", "*", "!", "==", "\r\n", "\t"]));
                }
                if self.chance(30) && !s.is_empty() {
                    s.truncate(s.len() - 1 - self.below(s.len() as u64).min(5) as usize);
                }
                s
            }
            "GZipBase64Binary" => {
                let level = flate2::Compression::new(self.below(10) as u32);
                let mut e = flate2::write::ZlibEncoder::new(Vec::new(), level);
                e.write_all(&disk).unwrap();
                let mut z = e.finish().unwrap();
                if self.chance(15) && !z.is_empty() {
                    let at = self.below(z.len() as u64) as usize;
                    z[at] ^= 0x55;
                }
                if self.chance(20) {
                    z.truncate(z.len() / 2);
                }
                let mut s = encode(&z);
                if self.chance(6) {
                    let at = self.below(s.len() as u64 + 1) as usize;
                    s.insert_str(at, self.pick(&["\n", " ", "*", "\r\n"]));
                }
                s
            }
            _ => {
                let name = format!("ext-{idx}-{}.bin", self.below(3));
                let off = [0usize, 0, 7, 1000][self.below(4) as usize];
                let mut file = vec![0xA5u8; off];
                file.extend_from_slice(&disk);
                if self.chance(10) {
                    file.truncate(file.len().saturating_sub(5));
                }
                let shown_name = if self.chance(15) {
                    "missing.bin".to_string()
                } else {
                    name.clone()
                };
                a += &format!(" ExternalFileName=\"{shown_name}\" ExternalFileOffset=\"{off}\"");
                ext.push((name, file));
                String::new()
            }
        };
        if enc != "ExternalFileBinary" && self.chance(4) {
            a += " ExternalFileName=\"\" ExternalFileOffset=\"\"";
        }
        if self.chance(15) {
            a += &format!(" {}=\"x\"", self.pick(&["Bogus", "Category", "Location"]));
        }
        out.push_str(&format!("   <DataArray{a}>\n"));
        if !self.chance(5) {
            let mut m = String::new();
            self.meta(&mut m, "      ");
            out.push_str(&m);
        }
        for _ in 0..[0usize, 1, 1, 1, 2][self.below(5) as usize] {
            self.coordsys(out);
        }
        let data = if self.chance(15) {
            format!("<![CDATA[{data}]]>")
        } else {
            data
        };
        if !self.chance(25) {
            out.push_str(&format!("      <Data>{data}</Data>\n"));
        }
        if self.chance(40) {
            out.push_str("      <Data>1</Data>\n");
        }
        out.push_str("   </DataArray>\n");
    }

    /// A GIFTI file, its external files and the settings to read it with.
    pub fn gifti_case(&mut self) -> Case {
        let mut s = String::new();
        let mut ext = Vec::new();
        if !self.chance(20) {
            s.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        }
        if self.chance(2) {
            s.push_str(
                "<!DOCTYPE GIFTI SYSTEM \"http://www.nitrc.org/frs/download.php/115/gifti.dtd\">\n",
            );
        }
        let nda = [0usize, 1, 1, 2, 2, 3, 4][self.below(7) as usize];
        let mut a = String::new();
        if !self.chance(10) {
            a += &format!(
                " Version=\"{}\"",
                self.pick(&[
                    "1.0", "1.0", "1", "1.00", "2.0", "abc", "", " 1.0", "1.0x", "nan"
                ])
            );
        }
        if !self.chance(10) {
            let shown = match self.below(8) {
                0 => nda + 1,
                1 => nda.saturating_sub(1),
                _ => nda,
            };
            a += &format!(" NumberOfDataArrays=\"{shown}\"");
        }
        if self.chance(3) {
            a += " xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xsi:noNamespaceSchemaLocation=\"http://brainvis.wustl.edu/caret6/xml_schemas/GIFTI_Caret.xsd\"";
        }
        if self.chance(10) {
            a += " Extra=\"e\"";
        }
        s.push_str(&format!("<GIFTI{a}>\n"));
        if !self.chance(5) {
            self.meta(&mut s, "   ");
        }
        if self.chance(2) {
            self.label_table(&mut s);
        }
        for k in 0..nda {
            self.darray(&mut s, &mut ext, k);
        }
        match self.below(30) {
            0 => s.push_str("   text here\n"),
            1 => s.push_str("   <Bogus a=\"1\"><MD/></Bogus>\n"),
            2 => s.push_str("   <!-- a comment -->\n   <?pi data?>\n"),
            3 => s.push_str("   <MD><Name>x</Name></MD>\n"),
            4 => s.push_str("   <Label Key=\"1\">x</Label>\n"),
            5 => s.push_str("   <Data>1 2</Data>\n"),
            6 => s.push_str("   <MetaData><MD><Value>novalue-name</Value></MD></MetaData>\n"),
            7 => s.push_str("   <Name>n</Name>\n"),
            8 => s.push_str("   <MatrixData>1</MatrixData>\n"),
            _ => {}
        }
        s.push_str("</GIFTI>\n");
        // now and then the whole file in UTF-16 (either byte order, a byte order mark) or in
        // ISO-8859-1, as its declaration says: expat converts, in pieces of 1024 bytes
        let mut file = match self.below(12) {
            0 | 1 => {
                let le = self.chance(2);
                let t = s.replace("encoding=\"UTF-8\"", "encoding=\"UTF-16\"");
                let mut v: Vec<u8> = if le {
                    vec![0xFF, 0xFE]
                } else {
                    vec![0xFE, 0xFF]
                };
                for u in t.encode_utf16() {
                    v.extend_from_slice(&if le { u.to_le_bytes() } else { u.to_be_bytes() });
                }
                v
            }
            2 => s
                .replace("encoding=\"UTF-8\"", "encoding=\"ISO-8859-1\"")
                .chars()
                .map(|c| {
                    if (c as u32) < 256 {
                        c as u32 as u8
                    } else {
                        b'?'
                    }
                })
                .collect(),
            _ => s.into_bytes(),
        };
        match self.below(14) {
            0 => {
                let n = self.below(file.len() as u64) as usize;
                file.truncate(n);
            }
            1 => {
                for _ in 0..1 + self.below(3) {
                    let at = self.below(file.len() as u64) as usize;
                    file[at] = [b'<', b'>', b'&', b'"', b' ', b'x', b'/', 0xff, b'0', b'\n']
                        [self.below(10) as usize];
                }
            }
            _ => {}
        }
        let verb = [0, 1, 1, 1, 1, 1, 2, 3, 4, 5, 6, 7][self.below(12) as usize];
        let set = [
            verb,
            [5, 5, 5, 1, 2, 3, 4, -1, 0, 6][self.below(10) as usize],
            [-1, -1, 32768, 2048, 4096, 100, 0, 3000][self.below(8) as usize],
            [1, 1, 0, -1][self.below(4) as usize],
            [-1, -1, 0, 1, 6, 9, 10][self.below(7) as usize],
            [-1, 3, 3, 0, 1, 7, -2][self.below(7) as usize],
        ];
        let dalist = if self.chance(10) {
            (0..1 + self.below(3))
                .map(|_| self.below(nda as u64 + 2) as i32 - (self.chance(10) as i32) * 2)
                .collect()
        } else {
            Vec::new()
        };
        Case {
            file,
            ext,
            set,
            read_data: !self.chance(8),
            dalist,
            nfiles: if self.chance(8) { 2 } else { 1 },
            disp: self.chance(3),
        }
    }
}
