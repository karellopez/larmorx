// SPDX-License-Identifier: GPL-3.0-or-later
// Translated to Rust from AFNI 25.2.09: src/thd_niftiread.c (THD_nifti_process_afni_ext),
// src/thd_nimlatr.c (THD_dblkatr_from_niml), src/thd_initdblk.c (the time-axis, matrix and
// template-space parts of THD_datablock_apply_atr), src/thd_zblock.c (THD_unzblock) and
// the text decoding of the NIML library (src/niml/niml_elemio.c: NI_read_element,
// NI_decode_one_double, NI_decode_one_string; header_stuff_is_group).
// Major portions Copyright (C) 1994-2000 Medical College of Wisconsin (AFNI, Robert W. Cox et al.),
// released under the GNU GPL version 2 or any later version (thd_initdblk.c, thd_zblock.c).
// thd_niftiread.c, thd_nimlatr.c and the NIML library were written at the NIH and are in the
// public domain.
// Rust translation Copyright 2026 Karel Lopez Vilaret; modified from the original.
//! AFNI's NIfTI header extension (code 4): the dataset attributes AFNI stores in the files it
//! writes, which override the NIfTI header when AFNI reads the file back.
//!
//! The extension is NIML text: an `<AFNI_attributes>` group of `<AFNI_atr>` elements, each
//! with a name, a type (`float`, `int`, `String`) and its values, numbers written with 7
//! significant digits (`%14.7g`). AFNI applies some of them to the dataset it opens
//! (`THD_datablock_apply_atr`); those that matter to 3dTshift are read here:
//!
//! - `TAXIS_NUMS`, `TAXIS_FLOATS`, `TAXIS_OFFSETS`: the time axis (number of time points,
//!   slices with offsets, units; time origin, TR) and the slice time offsets, replacing the
//!   NIfTI header's. The offsets are the 7-digit decimals, so they can differ in the last bits
//!   from the ones the header's `slice_duration` gives.
//! - `IJK_TO_DICOM_REAL`: the voxel-to-world matrix AFNI writes back.
//! - `TEMPLATE_SPACE`: the space, which decides the xform code AFNI writes.

use crate::cnum::scanf_f64;

/// The value of one `AFNI_atr` element.
#[derive(Clone, Debug, PartialEq)]
pub enum Attr {
    Float(Vec<f32>),
    Int(Vec<i32>),
    /// The concatenated strings, with AFNI's `~` turned back into NUL (`THD_unzblock`).
    Str(Vec<u8>),
}

/// The attributes of an AFNI extension, in the order they appear (a later one with the same
/// name replaces an earlier one, as `THD_set_*_atr` does).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AfniAttributes {
    pub attrs: Vec<(String, Attr)>,
    /// The group's `NIfTI_nums` attribute: the dimensions and datatype when AFNI wrote it.
    pub nifti_nums: Option<String>,
}

impl AfniAttributes {
    /// The attribute `name` (the last one of that name).
    pub fn get(&self, name: &str) -> Option<&Attr> {
        self.attrs
            .iter()
            .rev()
            .find(|(n, _)| n == name)
            .map(|(_, a)| a)
    }

    pub fn floats(&self, name: &str) -> Option<&[f32]> {
        match self.get(name)? {
            Attr::Float(v) => Some(v),
            _ => None,
        }
    }

    pub fn ints(&self, name: &str) -> Option<&[i32]> {
        match self.get(name)? {
            Attr::Int(v) => Some(v),
            _ => None,
        }
    }

    /// A string attribute up to its first NUL.
    pub fn string(&self, name: &str) -> Option<String> {
        match self.get(name)? {
            Attr::Str(v) => {
                let end = v.iter().position(|&c| c == 0).unwrap_or(v.len());
                Some(String::from_utf8_lossy(&v[..end]).into_owned())
            }
            _ => None,
        }
    }
}

/// `THD_nifti_process_afni_ext`: the attributes of the first AFNI extension (code 4, more
/// than 32 bytes) whose text starts with `<?xml`, or `None` when there is none or it cannot
/// be read (AFNI then keeps the NIfTI header's values).
pub fn parse(extensions: &[(i32, &[u8])]) -> Option<AfniAttributes> {
    // `esize > 32`: the content (esize - 8 bytes) is longer than 24 bytes.
    let (_, buf) = extensions
        .iter()
        .find(|(code, data)| *code == 4 && data.len() + 8 > 32)?;
    let nbuf = buf.len();
    if nbuf <= 32 || !buf.starts_with(b"<?xml") {
        return None;
    }
    // Terminated at its last byte, as AFNI terminates it.
    let text = String::from_utf8_lossy(&buf[..nbuf - 1]);
    let text = text.split('\0').next().unwrap_or("");
    let start = text.find("?>")? + 2;
    let mut p = Parser {
        s: text.as_bytes(),
        pos: start,
    };
    let root = p.element()?;
    let group = if root.name == "AFNI_attributes" {
        &root
    } else {
        root.find_group("AFNI_attributes")?
    };
    if !group.is_group {
        return None;
    }
    let mut out = AfniAttributes {
        attrs: Vec::new(),
        nifti_nums: group.attribute("NIfTI_nums").map(str::to_owned),
    };
    collect(group, &mut out.attrs);
    Some(out)
}

/// `THD_dblkatr_from_niml`: the `AFNI_atr` elements of a group and its sub-groups.
fn collect(group: &Element, out: &mut Vec<(String, Attr)>) {
    for part in &group.children {
        if part.is_group {
            collect(part, out);
            continue;
        }
        let name = part
            .attribute("atr_name")
            .or_else(|| part.attribute("AFNI_name"));
        let Some(name) = name.filter(|n| !n.is_empty()) else {
            continue;
        };
        if !part.name.eq_ignore_ascii_case("AFNI_atr") {
            continue;
        }
        let dimen: usize = part
            .attribute("ni_dimen")
            .and_then(|d| d.trim().parse().ok())
            .unwrap_or(1);
        let attr = match part.attribute("ni_type").unwrap_or("") {
            "float" => Attr::Float(
                numbers(&part.body, dimen)
                    .into_iter()
                    .map(|v| v as f32)
                    .collect(),
            ),
            "int" => Attr::Int(
                numbers(&part.body, dimen)
                    .into_iter()
                    .map(|v| v as i32)
                    .collect(),
            ),
            "String" => {
                let mut s: Vec<u8> = strings(&part.body, dimen).concat().into_bytes();
                // THD_unzblock: '~' (ZBLOCK) becomes NUL, and so does the last byte of the
                // NUL-terminated copy.
                for c in &mut s {
                    if *c == b'~' {
                        *c = 0;
                    }
                }
                Attr::Str(s)
            }
            _ => continue,
        };
        let empty = match &attr {
            Attr::Float(v) => v.is_empty(),
            Attr::Int(v) => v.is_empty(),
            Attr::Str(_) => false,
        };
        if !empty {
            out.push((name.to_owned(), attr));
        }
    }
}

/// `NI_decode_one_double` repeated: each value is the run of characters up to the next white
/// space, control character or `<`, read with `sscanf("%lf")` (0 if that fails).
fn numbers(body: &str, max: usize) -> Vec<f64> {
    body.split(|c: char| c.is_whitespace() || c.is_control())
        .filter(|t| !t.is_empty())
        .take(max)
        .map(|t| scanf_f64(t).unwrap_or(0.0))
        .collect()
}

/// `NI_decode_one_string` repeated: quoted strings (`"..."` or `'...'`) or bare words, with
/// the XML character entities replaced.
fn strings(body: &str, max: usize) -> Vec<String> {
    let b = body.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() && out.len() < max {
        while i < b.len() && (b[i].is_ascii_whitespace() || b[i].is_ascii_control()) {
            i += 1;
        }
        if i >= b.len() {
            break;
        }
        let (start, end, next) = if b[i] == b'"' || b[i] == b'\'' {
            let q = b[i];
            let close = b[i + 1..]
                .iter()
                .position(|&c| c == q)
                .map_or(b.len(), |k| i + 1 + k);
            (i + 1, close, (close + 1).min(b.len()))
        } else {
            let mut j = i;
            while j < b.len() && !b[j].is_ascii_whitespace() && b[j] != b'<' {
                j += 1;
            }
            (i, j, j)
        };
        out.push(unescape(&body[start..end]));
        i = next;
    }
    out
}

/// NIML's `unescape_inplace`: `&lt;` `&gt;` `&quot;` `&apos;` `&amp;`.
fn unescape(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

/// A NIML element read from text: a group (with children) or a data element (with a body).
#[derive(Debug, Default)]
struct Element {
    name: String,
    attrs: Vec<(String, String)>,
    is_group: bool,
    children: Vec<Element>,
    body: String,
}

impl Element {
    fn attribute(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }

    /// `NI_search_group_deep`: the first group named `name` inside this one.
    fn find_group(&self, name: &str) -> Option<&Element> {
        for c in &self.children {
            if c.name == name {
                return Some(c);
            }
            if c.is_group
                && let Some(g) = c.find_group(name)
            {
                return Some(g);
            }
        }
        None
    }
}

struct Parser<'a> {
    s: &'a [u8],
    pos: usize,
}

impl Parser<'_> {
    fn skip_space(&mut self) {
        while self.pos < self.s.len() && self.s[self.pos].is_ascii_whitespace() {
            self.pos += 1;
        }
    }

    /// The next element, skipping text, `<?...?>` and `<!...>` parts before it.
    fn element(&mut self) -> Option<Element> {
        loop {
            while self.pos < self.s.len() && self.s[self.pos] != b'<' {
                self.pos += 1;
            }
            if self.pos >= self.s.len() {
                return None;
            }
            let rest = &self.s[self.pos..];
            if rest.starts_with(b"<?") || rest.starts_with(b"<!") {
                let close = rest.iter().position(|&c| c == b'>')?;
                self.pos += close + 1;
                continue;
            }
            if rest.starts_with(b"</") {
                return None;
            }
            break;
        }
        self.pos += 1;
        let name_start = self.pos;
        while self.pos < self.s.len()
            && !self.s[self.pos].is_ascii_whitespace()
            && self.s[self.pos] != b'>'
            && self.s[self.pos] != b'/'
        {
            self.pos += 1;
        }
        let mut el = Element {
            name: String::from_utf8_lossy(&self.s[name_start..self.pos]).into_owned(),
            ..Element::default()
        };
        // Attributes, up to '>' or '/>'.
        let empty = loop {
            self.skip_space();
            if self.pos >= self.s.len() {
                return None;
            }
            match self.s[self.pos] {
                b'>' => {
                    self.pos += 1;
                    break false;
                }
                b'/' if self.s.get(self.pos + 1) == Some(&b'>') => {
                    self.pos += 2;
                    break true;
                }
                _ => {}
            }
            let a = self.pos;
            while self.pos < self.s.len()
                && !self.s[self.pos].is_ascii_whitespace()
                && !matches!(self.s[self.pos], b'=' | b'>' | b'/')
            {
                self.pos += 1;
            }
            let key = String::from_utf8_lossy(&self.s[a..self.pos]).into_owned();
            self.skip_space();
            let mut value = String::new();
            if self.s.get(self.pos) == Some(&b'=') {
                self.pos += 1;
                self.skip_space();
                match self.s.get(self.pos) {
                    Some(&q) if q == b'"' || q == b'\'' => {
                        let v0 = self.pos + 1;
                        let close = self.s[v0..].iter().position(|&c| c == q)?;
                        value = String::from_utf8_lossy(&self.s[v0..v0 + close]).into_owned();
                        self.pos = v0 + close + 1;
                    }
                    _ => {
                        let v0 = self.pos;
                        while self.pos < self.s.len()
                            && !self.s[self.pos].is_ascii_whitespace()
                            && self.s[self.pos] != b'>'
                        {
                            self.pos += 1;
                        }
                        value = String::from_utf8_lossy(&self.s[v0..self.pos]).into_owned();
                    }
                }
            } else if key.is_empty() {
                self.pos += 1;
                continue;
            }
            el.attrs.push((key, unescape(&value)));
        };
        // header_stuff_is_group
        el.is_group = el.name == "ni_group" || el.attribute("ni_form") == Some("ni_group");
        if empty {
            return Some(el);
        }
        let close_tag = format!("</{}", el.name);
        if el.is_group {
            loop {
                self.skip_space();
                let rest = &self.s[self.pos.min(self.s.len())..];
                if rest.is_empty() {
                    return Some(el);
                }
                if rest.starts_with(close_tag.as_bytes()) {
                    let close = rest.iter().position(|&c| c == b'>')?;
                    self.pos += close + 1;
                    return Some(el);
                }
                match self.element() {
                    Some(child) => el.children.push(child),
                    None => {
                        // A stray closing tag or the end of the text.
                        return Some(el);
                    }
                }
            }
        }
        let rest = &self.s[self.pos..];
        let end = find(rest, close_tag.as_bytes()).unwrap_or(rest.len());
        el.body = String::from_utf8_lossy(&rest[..end]).into_owned();
        self.pos += end;
        if let Some(close) = self.s[self.pos..].iter().position(|&c| c == b'>') {
            self.pos += close + 1;
        }
        Some(el)
    }
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXT: &str = "<?xml version='1.0' ?>\n<AFNI_attributes\n  self_idcode=\"XYZ_1\"\n  \
        NIfTI_nums=\"64,64,3,10,1,4\"\n  ni_form=\"ni_group\" >\n<AFNI_atr\n  ni_type=\"String\"\n  \
        ni_dimen=\"1\"\n  atr_name=\"TEMPLATE_SPACE\" >\n \"ORIG~\"\n</AFNI_atr>\n\n<AFNI_atr\n  \
        ni_type=\"int\"\n  ni_dimen=\"8\"\n  atr_name=\"TAXIS_NUMS\" >\n 10\n 3\n 77002\n -999\n \
        -999\n -999\n -999\n -999\n</AFNI_atr>\n<AFNI_atr ni_type=\"float\" ni_dimen=\"3\" \
        atr_name=\"TAXIS_OFFSETS\" >\n 0\n 1.030303\n 0.06060606\n</AFNI_atr>\n</AFNI_attributes>\n";

    #[test]
    fn reads_attributes() {
        let data = EXT.as_bytes().to_vec();
        let a = parse(&[
            (6, b"comment that is long enough to count".as_slice()),
            (4, &data),
        ])
        .unwrap();
        assert_eq!(a.nifti_nums.as_deref(), Some("64,64,3,10,1,4"));
        assert_eq!(a.string("TEMPLATE_SPACE").as_deref(), Some("ORIG"));
        assert_eq!(a.ints("TAXIS_NUMS").unwrap()[..3], [10, 3, 77002]);
        assert_eq!(
            a.floats("TAXIS_OFFSETS").unwrap(),
            [0.0, 1.030303f64 as f32, 0.06060606f64 as f32]
        );
        assert!(parse(&[(4, b"<?xml short".as_slice())]).is_none());
        assert!(parse(&[]).is_none());
    }
}
