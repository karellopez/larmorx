// SPDX-License-Identifier: Apache-2.0
//! The dumps `shim.c` writes, written the same way from the ports' values, and read back as text
//! lines for the differences shown. A byte of the port's dump may be marked as "any" (memory the
//! original never wrote, which the port fills with zeros and a notice).

use larmorx_io::gifti::{DataArray, GiftiImage, NvPairs};
use larmorx_io::xml::{Content, Handler};

#[derive(Default)]
pub struct D {
    pub b: Vec<u8>,
    pub any: Vec<bool>,
}

impl D {
    pub fn raw(&mut self, v: &[u8]) {
        self.b.extend_from_slice(v);
        self.any.extend(std::iter::repeat_n(false, v.len()));
    }
    pub fn wild(&mut self, n: usize) {
        self.b.extend(std::iter::repeat_n(0u8, n));
        self.any.extend(std::iter::repeat_n(true, n));
    }
    pub fn c(&mut self, c: u8) {
        self.raw(&[c]);
    }
    pub fn i(&mut self, v: i32) {
        self.raw(&v.to_le_bytes());
    }
    pub fn l(&mut self, v: i64) {
        self.raw(&v.to_le_bytes());
    }
    pub fn s(&mut self, s: Option<&[u8]>) {
        match s {
            None => self.i(-1),
            Some(s) => {
                // put_s takes strlen: up to the first 0
                let s: Vec<u8> = s.iter().copied().take_while(|&c| c != 0).collect();
                self.i(s.len() as i32);
                self.raw(&s);
            }
        }
    }
    pub fn n(&mut self, s: &[u8]) {
        self.i(s.len() as i32);
        self.raw(s);
    }
    /// Whether `theirs` matches (every byte not marked "any" equal; the same length).
    pub fn matches(&self, theirs: &[u8]) -> bool {
        self.b.len() == theirs.len()
            && self
                .b
                .iter()
                .zip(theirs)
                .zip(&self.any)
                .all(|((a, b), w)| *w || a == b)
    }
}

// ------------------------------------------------------------------ parser events

/// A handler recording [`larmorx_io::xml`]'s events as `shim.c`'s `x_*` callbacks do.
pub struct Events<'a>(pub &'a mut D);

fn model(d: &mut D, m: &Content) {
    d.i(m.kind);
    d.i(m.quant);
    d.s(m.name.as_deref());
    d.i(m.children.len() as i32);
    for c in &m.children {
        model(d, c);
    }
}

impl Handler for Events<'_> {
    fn start_element(&mut self, name: &[u8], attrs: &[(Vec<u8>, Vec<u8>)]) {
        self.0.c(b'S');
        self.0.s(Some(name));
        self.0.i(attrs.len() as i32);
        for (k, v) in attrs {
            self.0.s(Some(k));
            self.0.s(Some(v));
        }
    }
    fn end_element(&mut self, name: &[u8]) {
        self.0.c(b'E');
        self.0.s(Some(name));
    }
    fn characters(&mut self, data: &[u8]) {
        self.0.c(b'C');
        self.0.n(data);
    }
    fn processing_instruction(&mut self, target: &[u8], data: &[u8]) {
        self.0.c(b'P');
        self.0.s(Some(target));
        self.0.s(Some(data));
    }
    fn comment(&mut self, data: &[u8]) {
        self.0.c(b'M');
        self.0.s(Some(data));
    }
    fn start_cdata(&mut self) {
        self.0.c(b'A');
    }
    fn end_cdata(&mut self) {
        self.0.c(b'B');
    }
    fn default(&mut self, data: &[u8]) {
        self.0.c(b'D');
        self.0.n(data);
    }
    fn xml_decl(&mut self, version: Option<&[u8]>, encoding: Option<&[u8]>, standalone: i32) {
        self.0.c(b'X');
        self.0.s(version);
        self.0.s(encoding);
        self.0.i(standalone);
    }
    fn start_doctype(
        &mut self,
        name: &[u8],
        sysid: Option<&[u8]>,
        pubid: Option<&[u8]>,
        sub: bool,
    ) {
        self.0.c(b'T');
        self.0.s(Some(name));
        self.0.s(sysid);
        self.0.s(pubid);
        self.0.i(sub as i32);
    }
    fn end_doctype(&mut self) {
        self.0.c(b'U');
    }
    fn element_decl(&mut self, name: &[u8], m: &Content) {
        self.0.c(b'L');
        self.0.s(Some(name));
        model(self.0, m);
    }
}

/// A reader of the dumps, for the text shown with a difference.
pub struct R<'a> {
    pub b: &'a [u8],
    pub p: usize,
}

impl R<'_> {
    pub fn more(&self) -> bool {
        self.p < self.b.len()
    }
    pub fn c(&mut self) -> u8 {
        let v = *self.b.get(self.p).unwrap_or(&0);
        self.p += 1;
        v
    }
    pub fn i(&mut self) -> i32 {
        let v = self
            .b
            .get(self.p..self.p + 4)
            .map_or(0, |s| i32::from_le_bytes(s.try_into().unwrap()));
        self.p += 4;
        v
    }
    pub fn l(&mut self) -> i64 {
        let v = self
            .b
            .get(self.p..self.p + 8)
            .map_or(0, |s| i64::from_le_bytes(s.try_into().unwrap()));
        self.p += 8;
        v
    }
    pub fn bytes(&mut self, n: usize) -> &[u8] {
        let a = self.p.min(self.b.len());
        let e = (self.p + n).min(self.b.len());
        self.p += n;
        &self.b[a..e]
    }
    pub fn s(&mut self) -> String {
        let n = self.i();
        if n < 0 {
            return "NULL".into();
        }
        format!("{:?}", String::from_utf8_lossy(self.bytes(n as usize)))
    }
}

fn model_text(r: &mut R, out: &mut String) {
    let (t, q, n, k) = (r.i(), r.i(), r.s(), r.i());
    out.push_str(&format!("({t},{q},{n}"));
    for _ in 0..k.clamp(0, 1000) {
        out.push(' ');
        model_text(r, out);
    }
    out.push(')');
}

/// An events dump as one line per event.
pub fn events_text(b: &[u8]) -> Vec<String> {
    let mut r = R { b, p: 0 };
    let mut v = Vec::new();
    while r.more() {
        let c = r.c();
        let line = match c {
            b'S' => {
                let name = r.s();
                let n = r.i();
                let mut s = format!("start {name}");
                for _ in 0..n.clamp(0, 10000) {
                    let (k, x) = (r.s(), r.s());
                    s += &format!(" {k}={x}");
                }
                s
            }
            b'E' => format!("end {}", r.s()),
            b'C' => format!("chars {}", r.s()),
            b'P' => format!("pi {} {}", r.s(), r.s()),
            b'M' => format!("comment {}", r.s()),
            b'A' => "cdata-start".into(),
            b'B' => "cdata-end".into(),
            b'D' => format!("default {}", r.s()),
            b'X' => format!("xmldecl {} {} {}", r.s(), r.s(), r.i()),
            b'T' => format!("doctype {} {} {} {}", r.s(), r.s(), r.s(), r.i()),
            b'U' => "end-doctype".into(),
            b'L' => {
                let mut s = format!("elementdecl {} ", r.s());
                model_text(&mut r, &mut s);
                s
            }
            b'R' => format!("ERROR {} line {}", r.i(), r.l()),
            b'K' => "piece".into(),
            x => format!("?{x}"),
        };
        v.push(line);
    }
    v
}

// ------------------------------------------------------------------ gifticlib's images

fn nvp(d: &mut D, p: &NvPairs) {
    d.i(p.len() as i32);
    for k in 0..p.len() {
        d.s(p.name[k].as_deref());
        d.s(p.value[k].as_deref());
    }
}

fn darray(d: &mut D, da: &DataArray) {
    d.i(da.intent);
    d.i(da.datatype);
    d.i(da.ind_ord);
    d.i(da.num_dim);
    for k in 0..6 {
        d.i(da.dims[k]);
    }
    d.i(da.encoding);
    d.i(da.endian);
    d.s(da.ext_fname.as_deref());
    d.l(da.ext_offset);
    nvp(d, &da.meta);
    d.i(da.num_cs());
    for cs in &da.coordsys {
        d.i(cs.is_some() as i32);
        if let Some(cs) = cs {
            d.s(cs.dataspace.as_deref());
            d.s(cs.xformspace.as_deref());
            for r in &cs.xform {
                for v in r {
                    d.raw(&v.to_le_bytes());
                }
            }
        }
    }
    d.l(da.nvals);
    d.i(da.nbyper);
    d.i(da.data.is_some() as i32);
    if let Some(data) = &da.data {
        d.l(data.len() as i64);
        d.raw(data);
    }
    nvp(d, &da.ex_atrs);
}

/// `put_gim` of the port's image; with `keys_unset` (an FS-80 notice), keys of 0 match anything
/// (memory gifticlib never wrote).
pub fn gim(d: &mut D, g: &GiftiImage, keys_unset: bool) {
    d.c(b'G');
    d.i(g.num_da);
    d.s(g.version.as_deref());
    nvp(d, &g.meta);
    let lt = &g.labeltable;
    d.i(lt.len() as i32);
    for k in 0..lt.len() {
        if keys_unset && lt.key[k] == 0 {
            d.wild(4);
        } else {
            d.i(lt.key[k]);
        }
        d.s(lt.label[k].as_deref());
    }
    d.i(lt.rgba.is_some() as i32);
    if let Some(rgba) = &lt.rgba {
        for k in 0..4 * lt.len() {
            match rgba.get(k).copied().flatten() {
                Some(v) => d.raw(&v.to_le_bytes()),
                None => d.wild(4),
            }
        }
    }
    for k in 0..g.num_da.max(0) as usize {
        let da = g.darray.get(k).and_then(|x| x.as_ref());
        d.i(da.is_some() as i32);
        if let Some(da) = da {
            darray(d, da);
        }
    }
    d.i(g.swapped);
    d.i(g.compressed);
    nvp(d, &g.ex_atrs);
}

fn nvp_text(r: &mut R, what: &str, out: &mut Vec<String>) {
    let n = r.i();
    for k in 0..n.clamp(0, 100000) {
        out.push(format!("{what}[{k}] {} = {}", r.s(), r.s()));
    }
}

/// A `gp_read` dump as text lines (data as a length and a checksum).
pub fn read_text(b: &[u8]) -> Vec<String> {
    let mut r = R { b, p: 0 };
    let mut v = Vec::new();
    while r.more() {
        match r.c() {
            b'N' => v.push("no image".into()),
            b'W' => v.push(format!("write status {}", r.i())),
            b'G' => {
                let nda = r.i();
                v.push(format!("numDA {nda} version {}", r.s()));
                nvp_text(&mut r, "meta", &mut v);
                let lt = r.i();
                for k in 0..lt.clamp(0, 100000) {
                    v.push(format!("label[{k}] key {} {}", r.i(), r.s()));
                }
                if r.i() != 0 {
                    let x = r.bytes(16 * lt.max(0) as usize).to_vec();
                    v.push(format!("rgba {x:02x?}"));
                }
                for k in 0..nda.clamp(0, 100000) {
                    if r.i() == 0 {
                        v.push(format!("DA[{k}] NULL"));
                        continue;
                    }
                    let a: Vec<i32> = (0..12).map(|_| r.i()).collect();
                    v.push(format!(
                        "DA[{k}] intent {} type {} ord {} ndim {} dims {:?} enc {} endian {}",
                        a[0],
                        a[1],
                        a[2],
                        a[3],
                        &a[4..10],
                        a[10],
                        a[11]
                    ));
                    v.push(format!("DA[{k}] ext {} off {}", r.s(), r.l()));
                    nvp_text(&mut r, &format!("DA[{k}].meta"), &mut v);
                    let ncs = r.i();
                    for c in 0..ncs.clamp(0, 100000) {
                        if r.i() == 0 {
                            v.push(format!("DA[{k}].cs[{c}] NULL"));
                            continue;
                        }
                        let (ds, xs) = (r.s(), r.s());
                        let x: Vec<u64> = (0..16).map(|_| r.l() as u64).collect();
                        v.push(format!("DA[{k}].cs[{c}] {ds} {xs} {x:016x?}"));
                    }
                    let (nv, nb) = (r.l(), r.i());
                    v.push(format!("DA[{k}] nvals {nv} nbyper {nb}"));
                    if r.i() != 0 {
                        let n = r.l();
                        let data = r.bytes(n.max(0) as usize);
                        let mut h: u64 = 0xcbf29ce484222325;
                        for &x in data {
                            h = (h ^ x as u64).wrapping_mul(0x100000001b3);
                        }
                        let head: Vec<u8> = data.iter().take(24).copied().collect();
                        v.push(format!(
                            "DA[{k}] data {n} bytes fnv {h:016x} head {head:02x?}"
                        ));
                    } else {
                        v.push(format!("DA[{k}] data NULL"));
                    }
                    nvp_text(&mut r, &format!("DA[{k}].ex_atrs"), &mut v);
                }
                v.push(format!("swapped {} compressed {}", r.i(), r.i()));
                nvp_text(&mut r, "ex_atrs", &mut v);
                v.push(format!("valid {}", r.i()));
            }
            x => v.push(format!("?{x}")),
        }
    }
    v
}

/// The first differing line of two texts, with a little context.
pub fn first_difference(a: &[String], b: &[String]) -> String {
    let n = a.len().max(b.len());
    for k in 0..n {
        if a.get(k) != b.get(k) {
            let lo = k.saturating_sub(2);
            let show = |x: &[String]| -> String {
                x.iter()
                    .enumerate()
                    .skip(lo)
                    .take(5)
                    .map(|(i, l)| format!("\n      {i:4} {}", &l[..l.len().min(300)]))
                    .collect()
            };
            return format!("line {k}:\n    port{}\n    original{}", show(a), show(b));
        }
    }
    "(same text; masked bytes or lengths differ)".into()
}
