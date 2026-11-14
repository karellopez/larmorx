// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The fs.io probe's piece-5 cases (queue item 3.1-27): mosaics and MATLAB files. `mvm`
//! (`VolSS2MosSS`), `mmv` (`MosSS2VolSS`), `mck` (`CheckMosaic`), `mlw` (`MatlabWrite`), `mlr`
//! (`MatlabRead`), `mfr` (`MatFileRead`), `mrc` (`ReadMatlabFileContents`, `MLFCprint`,
//! `ReadMatlabFileVariable`). The `.mat` files are built here from MATLAB's formats: version 4
//! (every type, both byte orders, complex, several variables) and version 5 (the header, the
//! matrix element with its flags, dimensions, small and normal names and numbers of every type,
//! complex, compressed with zlib), right or cut or changed.

use std::io::Write;

use flate2::Compression;
use flate2::write::ZlibEncoder;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::matrix::{MATRIX_COMPLEX, MATRIX_REAL, Matrix, matrix_alloc};
use larmorx_freesurfer_io::{matfile, mosaic};

use super::piece4::{file_entry, setup};
use super::{Gen, Out, Work, dec, enc, hex};

/// The piece-5 commands.
pub const KINDS: [&str; 7] = ["mvm", "mmv", "mck", "mlw", "mlr", "mfr", "mrc"];

// ---------------------------------------------------------------- generation

fn size(g: &mut Gen) -> i64 {
    if g.chance(10) {
        g.pick(&[0, -1, -8, i64::from(i32::MIN), i64::from(i32::MAX), 1 << 20])
    } else {
        g.pick(&[1, 2, 3, 4, 8, 16, 64, 128, 256])
    }
}

fn mosaic_case(g: &mut Gen, kind: &str) -> Vec<String> {
    let (nv, nr) = (size(g), size(g));
    let (tc, tr) = (g.int(1, 6), g.int(1, 6));
    let (mut ncmos, mut nrmos) = (nv.wrapping_mul(tc), nr.wrapping_mul(tr));
    if g.chance(4) {
        ncmos += g.int(-3, 3);
    }
    if g.chance(4) {
        nrmos += g.int(-3, 3);
    }
    if g.chance(8) {
        ncmos = size(g);
    }
    let clamp = |v: i64| v.clamp(i64::from(i32::MIN), i64::from(i32::MAX));
    let (ncmos, nrmos) = (clamp(ncmos), clamp(nrmos));
    let coord = |g: &mut Gen, n: i64| -> i64 {
        if g.chance(6) {
            clamp(g.pick(&[-1, n, n + 1, i64::from(i32::MIN), i64::from(i32::MAX), -100]))
        } else {
            g.int(0, (n - 1).clamp(0, 300))
        }
    };
    let ns = clamp(tc * tr + g.int(-2, 1));
    let a: Vec<i64> = if kind == "mvm" {
        vec![
            coord(g, nv),
            coord(g, nr),
            coord(g, ns),
            nv,
            nr,
            ncmos,
            nrmos,
        ]
    } else {
        vec![coord(g, ncmos), coord(g, nrmos), ncmos, nrmos, nv, nr, ns]
    };
    let mut t = vec![kind.to_owned()];
    t.extend(a.iter().map(|v| v.to_string()));
    t.push(i64::from(g.chance(4)).to_string());
    t
}

fn number(g: &mut Gen) -> f64 {
    if g.chance(5) {
        g.pick(&[
            0.0,
            -0.0,
            1.0,
            -1.0,
            127.5,
            128.0,
            255.0,
            300.0,
            -129.0,
            65535.0,
            3e9,
            -3e9,
            1e300,
            f64::NAN,
            f64::INFINITY,
        ])
    } else {
        (g.int(-2000, 2000) as f64) / 8.0
    }
}

/// A number's bytes as an element of `ty` (0 double, 1 single, 2 int32, 3 int16, 4 uint16,
/// 5 uint8, 6 int8, 7 uint32), in the file's byte order.
fn put_num(b: &mut Vec<u8>, g: &mut Gen, ty: u8, be: bool) {
    let x = number(g);
    let mut v: Vec<u8> = match ty {
        0 => x.to_le_bytes().to_vec(),
        1 => (x as f32).to_le_bytes().to_vec(),
        2 => (x as i32).to_le_bytes().to_vec(),
        3 => (x as i16).to_le_bytes().to_vec(),
        4 => (x as u16).to_le_bytes().to_vec(),
        5 => vec![x as u8],
        6 => vec![x as i8 as u8],
        _ => (x as u32).to_le_bytes().to_vec(),
    };
    if g.chance(12) {
        for c in v.iter_mut() {
            *c = g.below(256) as u8;
        }
    }
    if be {
        v.reverse();
    }
    b.extend_from_slice(&v);
}

fn put32(b: &mut Vec<u8>, v: i32, be: bool) {
    if be {
        b.extend_from_slice(&v.to_be_bytes());
    } else {
        b.extend_from_slice(&v.to_le_bytes());
    }
}

fn name_bytes(g: &mut Gen, max: i64) -> Vec<u8> {
    let n = if g.chance(4) {
        g.pick(&[4, 5, 8, 16, 24, 40])
    } else {
        g.int(0, max)
    };
    (0..n)
        .map(|_| {
            if g.chance(30) {
                0
            } else {
                g.pick(b"XYMabcdz_19")
            }
        })
        .collect()
}

/// One version 4 variable.
fn v4_var(g: &mut Gen, portable: bool) -> Vec<u8> {
    // FreeSurfer reads little-endian doubles and big-endian files as they are; little-endian
    // files of other types it swaps (FS-230: their sizes mostly beyond the machine), so fewer
    let be = g.chance(2);
    let p = if g.chance(2) || (!be && !g.chance(4)) {
        0
    } else {
        g.int(1, 5) as u8
    };
    let mut ty = if be { 1000 } else { 0 } + i32::from(p) * 10;
    // the odd types (some read as sizes of hundreds of millions of numbers, printed for minutes:
    // not in the portable cases, which the recorded test runs in a debug build)
    if g.chance(12) && !portable {
        ty = g.pick(&[-1, 2, 7, 60, 100, 4052, 1001, 255, 1050, 30]);
    }
    let dim = |g: &mut Gen| -> i32 {
        if g.chance(14) {
            if portable {
                g.pick(&[-1, -3, 0])
            } else {
                g.pick(&[-1, -3, 0, 70000, 1 << 20])
            }
        } else {
            g.int(0, 4) as i32
        }
    };
    let (rows, cols) = (dim(g), dim(g));
    let imagf = i32::from(g.chance(4));
    let name = name_bytes(g, 10);
    let mut namlen = name.len() as i32 + 1;
    let mut nm = name.clone();
    nm.push(0);
    if g.chance(10) {
        nm.pop();
        namlen -= 1;
    }
    if g.chance(20) {
        namlen = g.pick(&[-1, -2, 0, 3]);
    }
    let mut b = Vec::new();
    for v in [ty, rows, cols, imagf, namlen] {
        put32(&mut b, v, be);
    }
    b.extend_from_slice(&nm);
    // the numbers in the type's element (P: 0 double, 1 single, 2 int32, 3 int16, 4 uint16,
    // 5 uint8)
    let el = [0u8, 1, 2, 3, 4, 5][p as usize];
    let n =
        i64::from(rows.clamp(0, 4)) * i64::from(cols.clamp(0, 4)) * if imagf != 0 { 2 } else { 1 };
    for _ in 0..n {
        put_num(&mut b, g, el, be);
    }
    b
}

fn pad8(b: &mut Vec<u8>, from: usize) {
    while !(b.len() - from).is_multiple_of(8) {
        b.push(0);
    }
}

/// A version 5 sub-element (small when it holds 4 bytes or fewer and `small` allows).
fn sub(b: &mut Vec<u8>, ty: i32, data: &[u8], be: bool, small: bool) {
    if small && data.len() <= 4 && !data.is_empty() {
        let w = ((data.len() as i32) << 16) | ty;
        put32(b, w, be);
        b.extend_from_slice(data);
        b.resize(b.len() + 4 - data.len(), 0);
        return;
    }
    put32(b, ty, be);
    put32(b, data.len() as i32, be);
    let at = b.len();
    b.extend_from_slice(data);
    pad8(b, at);
}

/// A version 5 matrix element (tag and contents).
fn v5_element(g: &mut Gen, be: bool, portable: bool) -> Vec<u8> {
    let complex = g.chance(4);
    let class = g.pick(&[6, 6, 6, 7, 8, 9, 10, 11, 12, 13]);
    let flags =
        class | if complex { 0x0800 } else { 0 } | if g.chance(10) { 0x1000_0000 } else { 0 };
    let ndims = if g.chance(8) { g.pick(&[1, 3]) } else { 2 };
    let dims: Vec<i32> = (0..ndims)
        .map(|_| {
            if g.chance(14) && !portable {
                g.pick(&[70000, -1])
            } else {
                g.int(0, 4) as i32
            }
        })
        .collect();
    let name = name_bytes(g, 20);
    let small = g.chance(2);
    // miINT8 1, miUINT8 2, miINT16 3, miUINT16 4, miINT32 5, miUINT32 6, miSINGLE 7, miDOUBLE 9
    let mt = g.pick(&[9, 9, 7, 1, 2, 3, 4, 5, 6, 0, 8, 12]);
    let el = match mt {
        9 => 0u8,
        7 => 1,
        5 => 2,
        3 => 3,
        4 => 4,
        2 => 5,
        1 => 6,
        6 => 7,
        _ => 0,
    };
    let n: i64 = dims.iter().map(|&d| i64::from(d.clamp(0, 4))).product();
    let mut body = Vec::new();
    let mut fl = Vec::new();
    put32(&mut fl, flags, be);
    put32(&mut fl, 0, be);
    sub(&mut body, 6, &fl, be, false);
    let mut dd = Vec::new();
    for &d in &dims {
        put32(&mut dd, d, be);
    }
    sub(&mut body, 5, &dd, be, false);
    sub(&mut body, 1, &name, be, small);
    for _ in 0..if complex { 2 } else { 1 } {
        let mut data = Vec::new();
        for _ in 0..n {
            put_num(&mut data, g, el, be);
        }
        sub(&mut body, mt, &data, be, true);
    }
    let mut e = Vec::new();
    put32(&mut e, 14, be);
    put32(&mut e, body.len() as i32, be);
    e.extend_from_slice(&body);
    e
}

fn v5_file(g: &mut Gen, portable: bool) -> Vec<u8> {
    let be = g.chance(4);
    let mut b =
        b"MATLAB 5.0 MAT-file, Platform: GLNXA64, Created on: Fri Nov 13 12:00:00 2026".to_vec();
    b.resize(116, b' ');
    b.extend_from_slice(&[0u8; 8]);
    if be {
        b.extend_from_slice(&[1, 0, b'M', b'I']);
    } else {
        b.extend_from_slice(&[0, 1, b'I', b'M']);
    }
    let mut e = if g.chance(10) {
        // zeros that compress more than 100 to 1
        let mut z = Vec::new();
        put32(&mut z, 14, be);
        let mut body = Vec::new();
        let mut fl = Vec::new();
        put32(&mut fl, 6, be);
        put32(&mut fl, 0, be);
        sub(&mut body, 6, &fl, be, false);
        let mut dd = Vec::new();
        put32(&mut dd, 32, be);
        put32(&mut dd, 32, be);
        sub(&mut body, 5, &dd, be, false);
        sub(&mut body, 1, b"Z", be, true);
        sub(&mut body, 9, &[0u8; 8192], be, false);
        put32(&mut z, body.len() as i32, be);
        z.extend_from_slice(&body);
        z
    } else {
        v5_element(g, be, portable)
    };
    if g.chance(2) || e.len() > 4000 {
        let mut zl = ZlibEncoder::new(Vec::new(), Compression::new(g.int(1, 9) as u32));
        zl.write_all(&e).unwrap();
        let c = zl.finish().unwrap();
        e.clear();
        put32(&mut e, 15, be);
        put32(&mut e, c.len() as i32, be);
        e.extend_from_slice(&c);
    }
    b.extend_from_slice(&e);
    b
}

pub(crate) fn mat_file(g: &mut Gen, portable: bool, several: bool) -> Vec<u8> {
    let mut b = if g.chance(2) || several {
        let k = if several { g.int(1, 3) } else { 1 };
        (0..k).flat_map(|_| v4_var(g, portable)).collect()
    } else {
        v5_file(g, portable)
    };
    if g.chance(6) && !b.is_empty() {
        let at = g.below(b.len() as u64) as usize;
        b.truncate(at);
    }
    if g.chance(8) && !b.is_empty() {
        for _ in 0..g.int(1, 3) {
            let at = g.below(b.len() as u64) as usize;
            b[at] = g.below(256) as u8;
        }
    }
    if g.chance(30) && !b.is_empty() {
        b[0] = 0xff;
    }
    b
}

fn mat_name(g: &mut Gen) -> &'static str {
    if g.chance(12) {
        g.pick(&["w/none.mat", "w", ""])
    } else {
        "w/m.mat"
    }
}

fn f32hex(g: &mut Gen) -> String {
    let x = number(g) as f32;
    let bits = if g.chance(10) {
        g.next() as u32
    } else {
        x.to_bits()
    };
    format!("{bits:08x}")
}

pub fn generate(g: &mut Gen, kind: &str, portable: bool) -> Vec<String> {
    match kind {
        "mvm" | "mmv" => mosaic_case(g, kind),
        "mck" => vec!["mck".into()],
        "mlw" => {
            let name = if g.chance(6) {
                None
            } else {
                Some(name_bytes(g, 12))
            };
            let (rows, cols) = (g.int(0, 4), g.int(0, 4));
            let cplx = g.chance(4);
            let mut t = vec![
                "mlw".into(),
                enc(name.as_deref()),
                rows.to_string(),
                cols.to_string(),
            ];
            t.push(i64::from(cplx).to_string());
            for _ in 0..rows * cols * if cplx { 2 } else { 1 } {
                t.push(f32hex(g));
            }
            let fname = if g.chance(10) {
                g.pick(&["w", "w/x/m.mat"])
            } else {
                "w/m.mat"
            };
            t.push(enc(Some(fname.as_bytes())));
            t.push("-".into());
            t
        }
        _ => {
            let gd = if g.chance(12) {
                "10000040"
            } else {
                g.pick(&["0", "0", "10000000", "40"])
            };
            let mut t = vec![
                kind.to_owned(),
                gd.to_owned(),
                g.pick(&[-1, 0, 1]).to_string(),
            ];
            if kind == "mfr" {
                t.push(g.pick(&[0, 4, 4, 0, 1, 3]).to_string());
            }
            let b = mat_file(g, portable, kind != "mlr");
            if kind == "mrc" {
                let v: Option<Vec<u8>> = if g.chance(3) {
                    None
                } else if g.chance(2) {
                    // a name the file may hold
                    Some(
                        b.iter()
                            .copied()
                            .filter(u8::is_ascii_alphabetic)
                            .take(g.int(0, 3) as usize)
                            .collect(),
                    )
                } else {
                    Some(name_bytes(g, 4))
                };
                t.push(enc(v.as_deref()));
            }
            let fname = mat_name(g);
            t.push(enc(Some(fname.as_bytes())));
            let entry = if fname == "w/m.mat" && !(g.chance(20)) {
                file_entry("w/m.mat", &b)
            } else {
                String::new()
            };
            t.push(if entry.is_empty() { "-".into() } else { entry });
            t
        }
    }
}

// ---------------------------------------------------------------- the port's answers

struct Toks<'a> {
    t: &'a [&'a str],
    i: usize,
}

impl<'a> Toks<'a> {
    fn s(&mut self) -> &'a str {
        let x = self.t[self.i];
        self.i += 1;
        x
    }
    fn i(&mut self) -> i64 {
        self.s().parse().unwrap()
    }
    fn f(&mut self) -> f32 {
        f32::from_bits(u32::from_str_radix(self.s(), 16).unwrap())
    }
    fn name(&mut self) -> Option<Vec<u8>> {
        dec(self.s())
    }
}

fn print_anymat(o: &mut Out, m: Option<&Matrix>) {
    let Some(m) = m else {
        o.0.push_str("N ");
        return;
    };
    o.i(i64::from(m.rows));
    o.i(i64::from(m.cols));
    o.i(i64::from(m.mtype));
    // more than 65,536 numbers: FNV-1a (64 bits) of the floats' bytes in memory order
    let big = i64::from(m.rows) * i64::from(m.cols) > 65536;
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut put = |o: &mut Out, f: f32| {
        if !big {
            o.x(&f.to_be_bytes(), 4);
            return;
        }
        for x in f.to_le_bytes() {
            h ^= u64::from(x);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    let rows = if m.cols > 0 { m.rows } else { 0 };
    for r in 1..=rows {
        for c in 1..=m.cols {
            if m.mtype == MATRIX_COMPLEX {
                let (a, b) = m.celt(r, c);
                put(o, a);
                put(o, b);
            } else {
                put(o, m.relt(r, c));
            }
        }
    }
    if big {
        o.0.push_str(&format!("fnv{h:016x} "));
    }
}

fn ofile(o: &mut Out, w: &Work, name: &str) {
    let p = w.fix.join(name);
    match std::fs::metadata(&p) {
        Ok(m) if m.is_file() => o.bytes(&std::fs::read(&p).unwrap()),
        _ => o.0.push_str("N "),
    }
}

/// The port's answer to a piece-5 case (`Err`: the program's exit status).
pub fn run(
    tk: &[&str],
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    o: &mut Out,
    w: &Work,
    extra: &mut Vec<&'static str>,
) -> Result<(), u8> {
    let r = answer(tk, g, out, err, o, w, extra);
    if matches!(tk[0], "mlr" | "mfr" | "mrc") && tk[1] == "10000040" {
        // MatReadHeader's message prints the stream's and the structure's addresses
        extra.push("VARY");
    }
    for n in g.notices.as_slice() {
        if let Some(c) = matfile::probe_class(n.defect.id, &n.detail) {
            extra.push(c);
        }
        // FS-229: FreeSurfer's SIGFPE; FS-13: MatrixAlloc of a negative size (FreeSurfer crashes,
        // exits or, for some, makes a matrix: the port exits in both modes)
        if matches!(n.defect.id, "FS-229" | "FS-13") {
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
    extra: &mut Vec<&'static str>,
) -> Result<(), u8> {
    let mut t = Toks { t: &tk[1..], i: 0 };
    let fatal = |f: Fatal| f.status();
    let dir = Some(w.fix.as_path());
    match tk[0] {
        "mvm" | "mmv" => {
            let a: Vec<i32> = (0..7).map(|_| t.i() as i32).collect();
            let null_oob = t.i() != 0;
            let (mut x, mut y, mut z, mut oob) = (-7, -7, -7, 7);
            let r = if tk[0] == "mvm" {
                mosaic::vol_ss2_mos_ss(
                    g,
                    err,
                    a[0],
                    a[1],
                    a[2],
                    a[3],
                    a[4],
                    a[5],
                    a[6],
                    &mut x,
                    &mut y,
                    (!null_oob).then_some(&mut oob),
                )
            } else {
                mosaic::mos_ss2_vol_ss(
                    g,
                    err,
                    a[0],
                    a[1],
                    a[2],
                    a[3],
                    a[4],
                    a[5],
                    a[6],
                    &mut x,
                    &mut y,
                    &mut z,
                    (!null_oob).then_some(&mut oob),
                )
            };
            for v in [r, x, y, z, oob] {
                o.i(i64::from(v));
            }
        }
        "mck" => o.i(i64::from(mosaic::check_mosaic(g, out, err))),
        "mlw" => {
            let name = t.name();
            let (rows, cols, cplx) = (t.i() as i32, t.i() as i32, t.i() != 0);
            let ty = if cplx { MATRIX_COMPLEX } else { MATRIX_REAL };
            let mut m = matrix_alloc(g, out, err, rows, cols, ty)
                .map_err(fatal)?
                .unwrap();
            for r in 1..=rows {
                for c in 1..=cols {
                    if cplx {
                        let (a, b) = (t.f(), t.f());
                        m.set_celt(r, c, a, b);
                    } else {
                        let a = t.f();
                        m.set_relt(r, c, a);
                    }
                }
            }
            let fname = String::from_utf8(t.name().unwrap()).unwrap();
            setup(w, t.s());
            g.errno = 0;
            o.i(i64::from(matfile::matlab_write(
                g,
                dir,
                &m,
                &fname,
                name.as_deref(),
            )));
            ofile(o, w, &fname);
        }
        kind => {
            g.gdiag = u64::from_str_radix(t.s(), 16).unwrap();
            g.gdiag_no = t.i() as i32;
            let ty = if kind == "mfr" { t.i() as i32 } else { 0 };
            let var = if kind == "mrc" { t.name() } else { None };
            let fname = String::from_utf8(t.name().unwrap()).unwrap();
            setup(w, t.s());
            g.errno = 0;
            match kind {
                "mlr" => {
                    let m = matfile::matlab_read(g, out, err, dir, &fname).map_err(fatal)?;
                    print_anymat(o, m.as_ref());
                }
                "mfr" => match matfile::mat_file_read(g, out, err, dir, &fname, ty)
                    .map_err(fatal)?
                {
                    None => o.0.push_str("N "),
                    Some(mf) => {
                        for v in [
                            mf.mtype, mf.mrows, mf.ncols, mf.imagf, mf.namlen, mf.version,
                        ] {
                            o.i(i64::from(v));
                        }
                        let n =
                            i64::from(mf.mrows) * i64::from(mf.ncols) * if ty == 4 { 4 } else { 1 };
                        match &mf.data {
                            Some(d) if n >= 0 => o.bytes(&d[..(n as usize).min(d.len())]),
                            _ => o.0.push_str("N "),
                        }
                    }
                },
                _ => {
                    match matfile::read_matlab_file_contents(g, out, err, dir, &fname)
                        .map_err(fatal)?
                    {
                        None => o.0.push_str("N "),
                        Some(c) => {
                            o.i(c.vars.len() as i64);
                            // FreeSurfer prints some 3 MB a second: beyond 4 MiB (at least 8 bytes a
                            // number) it may pass the probe's 5 seconds, so not compared (nor printed)
                            let numbers: i64 = c
                                .vars
                                .iter()
                                .map(|(_, m)| {
                                    i64::from(m.rows.max(0))
                                        * i64::from(m.cols.max(0))
                                        * i64::from(m.mtype)
                                })
                                .sum();
                            if numbers * 8 > 4 << 20 {
                                extra.push("SKIP");
                            } else {
                                let mut sink = Vec::new();
                                matfile::mlfc_print(g, &mut sink, out, err, &c);
                                if sink.len() > 4 << 20 {
                                    extra.push("SKIP");
                                }
                                let _ = out.write_all(&sink);
                            }
                        }
                    }
                    if let Some(v) = var {
                        let m = matfile::read_matlab_file_variable(g, out, err, dir, &fname, &v)
                            .map_err(fatal)?;
                        print_anymat(o, m.as_ref());
                    }
                }
            }
            o.i(i64::from(g.errno));
        }
    }
    let _ = hex;
    Ok(())
}
