// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Piece 25 (track A relay 33): MINC 1 volumes. `mcd` and `mch` (piece 6's `mrd` and `mrh`, the
//! type -1 or 1, a name ending in `.mnc`, or `@minc`, or a file starting `CDF`) of netCDF classic
//! files built here: an `image` variable of bytes, 16- or 32-bit integers, floats or doubles,
//! signed or unsigned, over the spatial dimensions in the standard or another order, with or
//! without a `time` dimension (first, as the record dimension or not, or elsewhere), now and then
//! one dimension fewer or an extra one (`vector_dimension`, or an unknown name first: five
//! dimensions); the dimensions' variables with steps (positive, negative, zero), starts and
//! direction cosines; valid ranges, `image-max` and `image-min` per volume, per slice or per slice
//! and frame; values past what FreeSurfer's types hold (signed bytes below 0, unsigned 16-bit
//! values above 32,767, 32-bit values); then now and then one fault: the file cut short, a file
//! that is not netCDF, a netCDF file without an image, a compressed name, a missing file or a
//! folder in its place. `Gdiag` with `DIAG_SHOW | DIAG_VERBOSE` now and then (mincRead's prints).

use std::io::Write;

use larmorx_freesurfer_core::globals::Globals;

use super::piece4::file_entry;
use super::{Gen, Out, Work, enc, hex};

pub const KINDS: [&str; 2] = ["mcd", "mch"];

const BYTE: i32 = 1;
const CHAR: i32 = 2;
const SHORT: i32 = 3;
const INT: i32 = 4;
const FLOAT: i32 = 5;
const DOUBLE: i32 = 6;

fn dir_entry(name: &str) -> String {
    format!("D{}", hex(name.as_bytes()))
}

/// An attribute's values.
enum Val {
    Num(i32, Vec<f64>),
    Text(Vec<u8>),
}

struct Var {
    name: Vec<u8>,
    dimids: Vec<u32>,
    atts: Vec<(Vec<u8>, Val)>,
    ty: i32,
    data: Vec<f64>,
}

/// A netCDF classic file (version 1 or 2).
struct Cdf {
    version: u8,
    numrecs: u32,
    dims: Vec<(Vec<u8>, u32)>,
    gatts: Vec<(Vec<u8>, Val)>,
    vars: Vec<Var>,
}

fn tsize(t: i32) -> usize {
    match t {
        BYTE | CHAR => 1,
        SHORT => 2,
        INT | FLOAT => 4,
        _ => 8,
    }
}

fn pad4(v: &mut Vec<u8>) {
    while v.len() % 4 != 0 {
        v.push(0);
    }
}

fn put_name(o: &mut Vec<u8>, n: &[u8]) {
    o.extend_from_slice(&(n.len() as u32).to_be_bytes());
    o.extend_from_slice(n);
    pad4(o);
}

/// Values in the type's external (big-endian) form; integers stored as their low bits.
fn encode(t: i32, vals: &[f64]) -> Vec<u8> {
    let mut o = Vec::new();
    for &v in vals {
        match t {
            BYTE | CHAR => o.push(v as i64 as u8),
            SHORT => o.extend_from_slice(&(v as i64 as i16).to_be_bytes()),
            INT => o.extend_from_slice(&(v as i64 as i32).to_be_bytes()),
            FLOAT => o.extend_from_slice(&(v as f32).to_bits().to_be_bytes()),
            _ => o.extend_from_slice(&v.to_bits().to_be_bytes()),
        }
    }
    o
}

fn put_atts(o: &mut Vec<u8>, atts: &[(Vec<u8>, Val)]) {
    if atts.is_empty() {
        o.extend_from_slice(&[0; 8]);
        return;
    }
    o.extend_from_slice(&0x0Cu32.to_be_bytes());
    o.extend_from_slice(&(atts.len() as u32).to_be_bytes());
    for (name, val) in atts {
        put_name(o, name);
        let (t, n, mut b) = match val {
            Val::Num(t, x) => (*t, x.len() as u32, encode(*t, x)),
            Val::Text(b) => (CHAR, b.len() as u32, b.clone()),
        };
        o.extend_from_slice(&(t as u32).to_be_bytes());
        o.extend_from_slice(&n.to_be_bytes());
        pad4(&mut b);
        o.extend_from_slice(&b);
    }
}

impl Cdf {
    fn is_rec(&self, v: &Var) -> bool {
        v.dimids
            .first()
            .is_some_and(|&d| self.dims.get(d as usize).is_some_and(|x| x.1 == 0))
    }
    /// Values per record (all values of a variable without the record dimension).
    fn per(&self, v: &Var) -> usize {
        let rec = self.is_rec(v);
        v.dimids
            .iter()
            .enumerate()
            .filter(|&(i, _)| !(i == 0 && rec))
            .map(|(_, &d)| self.dims[d as usize].1 as usize)
            .product()
    }
    fn vsize(&self, v: &Var) -> usize {
        (self.per(v) * tsize(v.ty)).div_ceil(4) * 4
    }
    fn write(&self) -> Vec<u8> {
        let header = |begins: &[u64]| {
            let mut o = b"CDF".to_vec();
            o.push(self.version);
            o.extend_from_slice(&self.numrecs.to_be_bytes());
            if self.dims.is_empty() {
                o.extend_from_slice(&[0; 8]);
            } else {
                o.extend_from_slice(&0x0Au32.to_be_bytes());
                o.extend_from_slice(&(self.dims.len() as u32).to_be_bytes());
                for (n, s) in &self.dims {
                    put_name(&mut o, n);
                    o.extend_from_slice(&s.to_be_bytes());
                }
            }
            put_atts(&mut o, &self.gatts);
            if self.vars.is_empty() {
                o.extend_from_slice(&[0; 8]);
            } else {
                o.extend_from_slice(&0x0Bu32.to_be_bytes());
                o.extend_from_slice(&(self.vars.len() as u32).to_be_bytes());
                for (v, &b) in self.vars.iter().zip(begins) {
                    put_name(&mut o, &v.name);
                    o.extend_from_slice(&(v.dimids.len() as u32).to_be_bytes());
                    for d in &v.dimids {
                        o.extend_from_slice(&d.to_be_bytes());
                    }
                    put_atts(&mut o, &v.atts);
                    o.extend_from_slice(&(v.ty as u32).to_be_bytes());
                    o.extend_from_slice(&(self.vsize(v) as u32).to_be_bytes());
                    if self.version == 1 {
                        o.extend_from_slice(&(b as u32).to_be_bytes());
                    } else {
                        o.extend_from_slice(&b.to_be_bytes());
                    }
                }
            }
            o
        };
        let hlen = header(&vec![0u64; self.vars.len()]).len() as u64;
        let mut begins = vec![0u64; self.vars.len()];
        let mut off = hlen;
        for (i, v) in self.vars.iter().enumerate() {
            if !self.is_rec(v) {
                begins[i] = off;
                off += self.vsize(v) as u64;
            }
        }
        let recs: Vec<usize> = (0..self.vars.len())
            .filter(|&i| self.is_rec(&self.vars[i]))
            .collect();
        let single = recs.len() == 1;
        for &i in &recs {
            begins[i] = off;
            if !single {
                off += self.vsize(&self.vars[i]) as u64;
            }
        }
        let mut out = header(&begins);
        for v in self.vars.iter().filter(|v| !self.is_rec(v)) {
            let mut b = encode(v.ty, &v.data);
            b.resize(self.per(v) * tsize(v.ty), 0);
            pad4(&mut b);
            out.extend_from_slice(&b);
        }
        for r in 0..self.numrecs as usize {
            for &i in &recs {
                let v = &self.vars[i];
                let per = self.per(v);
                let mut piece = vec![0u8; per * tsize(v.ty)];
                let b = encode(v.ty, &v.data);
                let a = (r * piece.len()).min(b.len());
                let e = ((r + 1) * piece.len()).min(b.len());
                piece[..e - a].copy_from_slice(&b[a..e]);
                if !single {
                    pad4(&mut piece);
                }
                out.extend_from_slice(&piece);
            }
        }
        out
    }
}

/// The values a type holds (signed or not).
fn range(t: i32, signed: bool) -> (f64, f64) {
    match (t, signed) {
        (BYTE, true) => (-128.0, 127.0),
        (BYTE, false) => (0.0, 255.0),
        (SHORT, true) => (-32768.0, 32767.0),
        (SHORT, false) => (0.0, 65535.0),
        (INT, true) => (-2147483648.0, 2147483647.0),
        (INT, false) => (0.0, 4294967295.0),
        _ => (-1000.0, 1000.0),
    }
}

/// A value of the type: now and then at its ends.
fn value(g: &mut Gen, t: i32, signed: bool) -> f64 {
    let (lo, hi) = range(t, signed);
    match t {
        FLOAT | DOUBLE => match g.below(30) {
            0 => f64::NAN,
            1 => f64::INFINITY,
            _ => (g.int(-8000, 8000) as f64) / 8.0,
        },
        _ => match g.below(10) {
            0 => lo,
            1 => hi,
            2 => 0.0,
            _ => {
                // mostly small values, now and then anywhere in the range
                if g.chance(3) {
                    lo + (g.below(((hi - lo) as u64).max(1)) as f64)
                } else {
                    g.int(lo.max(-100.0) as i64, hi.min(300.0) as i64) as f64
                }
            }
        },
    }
}

fn dim_var(g: &mut Gen, name: &[u8], spatial: bool, axis: usize) -> Var {
    let mut atts: Vec<(Vec<u8>, Val)> = Vec::new();
    if !g.chance(8) {
        let step = match g.below(12) {
            0 | 1 => -g.pick(&[1.0, 0.5, 2.0, 1.25, 3.0]),
            2 if g.chance(3) => 0.0,
            _ => g.pick(&[1.0, 0.5, 2.0, 1.25, 3.0, 0.9375]),
        };
        atts.push((b"step".to_vec(), Val::Num(DOUBLE, vec![step])));
    }
    if !g.chance(6) {
        let start = (g.int(-1600, 1600) as f64) / 16.0;
        atts.push((b"start".to_vec(), Val::Num(DOUBLE, vec![start])));
    }
    if spatial && !g.chance(3) {
        let mut d = [0.0f64; 3];
        d[axis] = 1.0;
        if g.chance(2) {
            for (k, c) in d.iter_mut().enumerate() {
                if k != axis {
                    *c = (g.int(-30, 30) as f64) / 64.0;
                }
            }
        }
        if g.chance(10) {
            d.swap(axis, (axis + 1) % 3);
        }
        if g.chance(6) {
            for c in &mut d {
                *c = -*c;
            }
        }
        atts.push((b"direction_cosines".to_vec(), Val::Num(DOUBLE, d.to_vec())));
    }
    if spatial && g.chance(3) {
        let st: &[u8] = g.pick(&[&b"talairach_"[..], b"native____", b"world"]);
        atts.push((b"spacetype".to_vec(), Val::Text(st.to_vec())));
    }
    if !spatial && g.chance(3) {
        atts.push((b"spacing".to_vec(), Val::Text(b"regular__".to_vec())));
    }
    Var {
        name: name.to_vec(),
        dimids: Vec::new(),
        atts,
        ty: INT,
        data: vec![0.0],
    }
}

/// A MINC 1 file.
fn minc_file(g: &mut Gen) -> Vec<u8> {
    let mut spatial: Vec<&[u8]> = vec![b"zspace", b"yspace", b"xspace"];
    if g.chance(4) {
        for i in (1..3).rev() {
            let j = g.below(i as u64 + 1) as usize;
            spatial.swap(i, j);
        }
    }
    if g.chance(14) {
        spatial.truncate(g.int(1, 2) as usize);
    }
    let mut names: Vec<Vec<u8>> = spatial.iter().map(|s| s.to_vec()).collect();
    let with_time = g.chance(3);
    if with_time {
        let at = if g.chance(6) {
            g.below(names.len() as u64 + 1) as usize
        } else {
            0
        };
        names.insert(at, b"time".to_vec());
    }
    if g.chance(20) {
        names.push(b"vector_dimension".to_vec());
    }
    if g.chance(16) {
        names.insert(0, b"foo".to_vec());
    }
    let big = g.chance(8);
    let mut dims: Vec<(Vec<u8>, u32)> = names
        .iter()
        .map(|n| {
            let s = if big { g.int(5, 12) } else { g.int(1, 4) } as u32;
            (n.clone(), s)
        })
        .collect();
    let mut numrecs = 0;
    if with_time && dims[0].0 == b"time" && g.chance(3) {
        numrecs = dims[0].1;
        dims[0].1 = 0;
    }
    let ty = g.pick(&[BYTE, BYTE, SHORT, SHORT, INT, FLOAT, FLOAT, DOUBLE]);
    let signed = match ty {
        BYTE => g.chance(3),
        SHORT | INT => !g.chance(3),
        _ => true,
    };
    let mut iatts: Vec<(Vec<u8>, Val)> = Vec::new();
    let default_sign = ty != BYTE;
    if signed != default_sign || g.chance(2) {
        let st: &[u8] = if signed { b"signed__" } else { b"unsigned" };
        iatts.push((b"signtype".to_vec(), Val::Text(st.to_vec())));
    }
    let (lo, hi) = range(ty, signed);
    match g.below(6) {
        0 | 1 => {
            let (a, b) = if matches!(ty, FLOAT | DOUBLE) {
                (-(g.int(0, 500) as f64), g.int(1, 2000) as f64)
            } else if g.chance(2) {
                (lo, hi)
            } else {
                let a = g.int(lo as i64, (lo + (hi - lo) / 4.0) as i64) as f64;
                let b = g.int((hi - (hi - lo) / 4.0) as i64, hi as i64) as f64;
                (a, b)
            };
            let t = g.pick(&[DOUBLE, DOUBLE, FLOAT]);
            iatts.push((b"valid_range".to_vec(), Val::Num(t, vec![a, b])));
        }
        2 if g.chance(4) => {
            // reversed
            iatts.push((b"valid_range".to_vec(), Val::Num(DOUBLE, vec![hi, lo])));
        }
        _ => {}
    }
    let mut vars = Vec::new();
    for (i, (n, _)) in dims.iter().enumerate() {
        if n == b"vector_dimension" || n == b"foo" || g.chance(10) {
            continue;
        }
        let sp = n != b"time";
        let axis = match n.as_slice() {
            b"xspace" => 0,
            b"yspace" => 1,
            _ => 2,
        };
        let mut v = dim_var(g, n, sp, axis);
        let _ = i;
        v.dimids = Vec::new();
        vars.push(v);
    }
    let ndim = dims.len();
    let size_of = |d: usize| -> usize {
        if dims[d].1 == 0 {
            numrecs as usize
        } else {
            dims[d].1 as usize
        }
    };
    // image-max / image-min: none, per volume, per first dimension, per first two
    let mm = g.below(6);
    let mm_dims: Vec<u32> = match mm {
        2 | 3 => vec![0],
        4 if ndim >= 3 => vec![0, 1],
        _ => Vec::new(),
    };
    if mm >= 1 {
        let count: usize = mm_dims.iter().map(|&d| size_of(d as usize)).product();
        let mut maxs = Vec::new();
        let mut mins = Vec::new();
        for _ in 0..count.max(1) {
            let a = (g.int(-500, 2000) as f64) / 4.0;
            let b = if g.chance(12) {
                a
            } else {
                a - (g.int(1, 4000) as f64) / 4.0
            };
            maxs.push(a);
            mins.push(b);
        }
        let t = g.pick(&[DOUBLE, DOUBLE, FLOAT]);
        vars.push(Var {
            name: b"image-max".to_vec(),
            dimids: mm_dims.clone(),
            atts: Vec::new(),
            ty: t,
            data: maxs,
        });
        vars.push(Var {
            name: b"image-min".to_vec(),
            dimids: mm_dims,
            atts: Vec::new(),
            ty: t,
            data: mins,
        });
    }
    let total: usize = (0..ndim).map(size_of).product();
    let data: Vec<f64> = (0..total).map(|_| value(g, ty, signed)).collect();
    let image = Var {
        name: b"image".to_vec(),
        dimids: (0..ndim as u32).collect(),
        atts: iatts,
        ty,
        data,
    };
    if g.chance(5) {
        vars.insert(0, image);
    } else {
        vars.push(image);
    }
    let mut gatts = Vec::new();
    if g.chance(2) {
        gatts.push((b"history".to_vec(), Val::Text(b"made by the probe\n".to_vec())));
    }
    Cdf {
        version: if g.chance(6) { 2 } else { 1 },
        numrecs,
        dims,
        gatts,
        vars,
    }
    .write()
}

/// A netCDF file without an image variable.
fn no_image(g: &mut Gen) -> Vec<u8> {
    Cdf {
        version: 1,
        numrecs: 0,
        dims: vec![(b"xspace".to_vec(), g.int(1, 3) as u32)],
        gatts: Vec::new(),
        vars: vec![Var {
            name: b"other".to_vec(),
            dimids: vec![0],
            atts: Vec::new(),
            ty: SHORT,
            data: vec![1.0, 2.0, 3.0],
        }],
    }
    .write()
}

/// A gzip stream of `b` stored (no compression), as `gzip -0` writes one.
fn gz_stored(b: &[u8]) -> Vec<u8> {
    let mut o = vec![0x1f, 0x8b, 8, 0, 0, 0, 0, 0, 0, 3];
    for (i, block) in b.chunks(65535).enumerate() {
        let last = (i + 1) * 65535 >= b.len();
        o.push(u8::from(last));
        let n = block.len() as u16;
        o.extend_from_slice(&n.to_le_bytes());
        o.extend_from_slice(&(!n).to_le_bytes());
        o.extend_from_slice(block);
    }
    if b.is_empty() {
        o.extend_from_slice(&[1, 0, 0, 0xff, 0xff]);
    }
    o.extend_from_slice(&crc32(b).to_le_bytes());
    o.extend_from_slice(&(b.len() as u32).to_le_bytes());
    o
}

fn crc32(b: &[u8]) -> u32 {
    let mut c = 0xffff_ffffu32;
    for &x in b {
        c ^= u32::from(x);
        for _ in 0..8 {
            c = if c & 1 != 0 { (c >> 1) ^ 0xedb8_8320 } else { c >> 1 };
        }
    }
    !c
}

fn read_case(g: &mut Gen, entries: &mut Vec<String>) -> (String, i64) {
    let name = g.pick(&[
        "w/v.mnc",
        "w/v.mnc",
        "w/v.mnc",
        "w/V.MNC",
        "w/v.dat@minc",
        "w/v.bin",
        "w/v.mnc.gz",
    ]);
    let file = name.split('@').next().unwrap_or(name);
    match g.below(24) {
        0 => entries.push(dir_entry(file)),
        1 => {}
        2 => entries.push(file_entry(file, b"CDF\x01garbage")),
        3 => entries.push(file_entry(file, b"not a netCDF file at all")),
        4 => {
            let b = no_image(g);
            entries.push(file_entry(file, &b));
        }
        5 => {
            let mut b = minc_file(g);
            let cut = g.below(b.len() as u64 + 1) as usize;
            b.truncate(cut);
            entries.push(file_entry(file, &b));
        }
        _ => {
            let b = minc_file(g);
            let b = if file.ends_with(".gz") { gz_stored(&b) } else { b };
            entries.push(file_entry(file, &b));
        }
    }
    let ty = g.pick(&[-1i64, -1, 1]);
    let name = if g.chance(10) {
        format!("{name}#{}", g.pick(&["0", "1", "0:1", "1:2"]))
    } else {
        name.to_owned()
    };
    (name, ty)
}

pub fn generate(g: &mut Gen, kind: &str, _portable: bool) -> Vec<String> {
    let mut t = vec![kind.to_owned()];
    let mut entries = Vec::new();
    let (name, ty) = read_case(g, &mut entries);
    t.push(g.pick(&["0", "0", "0", "40", "10000040"]).into()); // Gdiag
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

/// The port's answer to a piece-25 case: piece 6's `mrd`/`mrh`.
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
    let mut t: Vec<&str> = vec![if tk[0] == "mch" { "mrh" } else { "mrd" }];
    t.extend_from_slice(&tk[1..]);
    let r = super::piece6::run_env(&t, g, out, err, o, w, extra, &[]);
    for n in g.notices.as_slice() {
        // memory contents (FreeSurfer's dimension names past its array, MINC's own undefined
        // reads, track C's probe does not compare them either)
        if matches!(
            n.defect.id,
            "FS-745"
                | "FS-774"
                | "FS-776"
                | "FS-777"
                | "FS-778"
                | "FS-779"
                | "FS-782"
                | "FS-783"
                | "FS-784"
                | "FS-785"
                | "FS-789"
                | "FS-790"
                | "FS-792"
                | "FS-795"
                | "FS-796"
        ) && g.notices.reproduce()
        {
            extra.push("SKIP");
        }
    }
    r
}
