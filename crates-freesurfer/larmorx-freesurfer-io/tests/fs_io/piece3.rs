// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The fs.io probe's piece-3 cases (queue item 3.1-25's third piece): `.npy` volumes
//! (`fscnpy::npy2mri`), gifti.cpp's two helpers over gifticlib's structures, dti.cpp's b-value
//! writers, pulse-sequence names and gradient spaces. The `.npy` files are built here from NumPy's
//! format (its headers as NumPy writes them, then changed: other versions, alignments, keys,
//! values, cut and mutated files); the port answers each case as `fsio` prints FreeSurfer's.

use std::io::Write;

use larmorx_core::cfmt::Streams;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::matrix::{MATRIX_REAL, Matrix, matrix_alloc};
use larmorx_freesurfer_io::{dti, fscnpy, gifti};
use larmorx_freesurfer_mri::mri::{self, MRI_UCHAR};
use larmorx_io::gifti::{DataArray, Gifti, GiftiImage};

use super::{Gen, Out, Work, dec, enc, hex, unhex};

/// The piece-3 commands.
pub const KINDS: [&str; 6] = ["npy", "gda", "gdv", "bvw", "psn", "bcs"];

// ---------------------------------------------------------------- generation

fn fbits(g: &mut Gen, finite: bool) -> u32 {
    match g.below(10) {
        0 if !finite => g.pick(&[
            0x7f80_0000u32,
            0xff80_0000,
            0x7fc0_0000,
            0xffc0_0000,
            0x7f80_0001,
            0x7fa0_beef,
        ]),
        0 | 1 => g.pick(&[
            0u32,
            0x8000_0000,
            0x3f80_0000,
            0xbf80_0000,
            0x7f7f_ffff,
            0x0000_0001,
            0x3dcc_cccd,
            0x3f35_04f3,
        ]),
        2 if !finite => g.next() as u32,
        _ => (g.int(-16, 16) as f32 / 8.0).to_bits(),
    }
}

/// A NumPy type string and its item size.
fn npy_dtype(g: &mut Gen) -> (String, usize) {
    let (d, n) = g.pick(&[
        ("<f4", 4),
        ("<f4", 4),
        ("<f4", 4),
        ("<f4", 4),
        ("<f4", 4),
        ("|u1", 1),
        ("<i2", 2),
        ("<i4", 4),
        (">f4", 4),
        ("<u2", 2),
        ("<i2", 2),
        ("<i4", 4),
        ("<i8", 8),
        ("|u1", 1),
        ("|u1", 1),
        ("<u2", 2),
        (">f4", 4),
        (">i2", 2),
        (">i4", 4),
        (">i8", 8),
        (">u2", 2),
        ("<f8", 8),
        ("|b1", 1),
        ("<c8", 8),
        ("|i1", 1),
        ("<u4", 4),
        ("<f2", 2),
        ("<U3", 12),
        ("=f4", 4),
        ("|u2", 2),
        ("<u1", 1),
        ("<i4294967300", 4),
    ]);
    (d.to_owned(), n)
}

fn shape_text(g: &mut Gen, shape: &[i64]) -> String {
    let parts: Vec<String> = shape
        .iter()
        .map(|v| {
            if g.chance(30) {
                format!("{v}L")
            } else {
                v.to_string()
            }
        })
        .collect();
    match shape.len() {
        0 => "()".into(),
        1 => format!("({},)", parts[0]),
        _ => format!("({})", parts.join(", ")),
    }
}

/// A `.npy` file: NumPy's header (sometimes changed) and the data.
fn npy_file(g: &mut Gen, small: bool) -> Vec<u8> {
    let ndim = match g.below(20) {
        0 => 0usize,
        1 => 1,
        2..=4 => 2,
        5 => 5,
        6..=12 => 3,
        _ => 4,
    };
    let limit = if small { 24 } else { 512 };
    let mut shape: Vec<i64> = Vec::new();
    let mut total: i64 = 1;
    for _ in 0..ndim {
        let v = if g.chance(6) {
            1
        } else {
            g.int(1, 5).min((limit / total).max(1))
        };
        total *= v;
        shape.push(v);
    }
    if g.chance(40) && !shape.is_empty() {
        let i = g.below(shape.len() as u64) as usize;
        shape[i] = g.pick(&[0, -1, 4_294_967_297]);
    }
    let total = shape.iter().product::<i64>().clamp(0, 4096) as usize;
    let (dtype, size) = npy_dtype(g);
    let fortran = if g.chance(3) { "True" } else { "False" };
    let fortran = if g.chance(40) {
        g.pick(&["0", "false", "1", "Tru", "False ", "None"])
    } else {
        fortran
    };
    let mut items: Vec<String> = vec![
        format!("'descr': '{dtype}'"),
        format!("'fortran_order': {fortran}"),
        format!("'shape': {}", shape_text(g, &shape)),
    ];
    match g.below(30) {
        0 => {
            items.remove(g.below(3) as usize);
        }
        1 => items.swap(0, 2),
        2 => items.insert(g.below(3) as usize, "'foo': 7".into()),
        3 => items.push(format!("'shape': {}", shape_text(g, &shape))),
        _ => {}
    }
    let sep = if g.chance(20) { "," } else { ", " };
    let mut dict = format!("{{{}{sep}}}", items.join(sep));
    if g.chance(30) {
        dict = format!("{{{}}}", items.join(", "));
    }
    let (major, minor) = match g.below(40) {
        0 => (3u8, 0u8),
        1 => (3, 1),
        2 => (0, 0),
        3 => (4, 0),
        4 => (7, 0),
        5..=9 => (2, 0),
        _ => (1, g.pick(&[0u8, 0, 0, 1])),
    };
    let lb = if major == 1 { 2 } else { 4 };
    let align = g.pick(&[64usize, 64, 64, 64, 16, 16, 16, 1, 7]);
    let mut body = dict.into_bytes();
    let base = 6 + 2 + lb + body.len() + 1;
    body.extend(std::iter::repeat_n(b' ', (align - base % align) % align));
    if !g.chance(25) {
        body.push(b'\n');
    } else if g.chance(2) {
        body.push(b' ');
    }
    let mut hl = body.len() as u64;
    if g.chance(25) {
        hl = match g.below(4) {
            0 => hl + g.int(1, 20) as u64,
            1 => hl.saturating_sub(g.int(1, 20) as u64),
            2 => 0,
            _ => g.pick(&[0x7fff_ffffu64, 0xffff, 1 << 23]),
        };
    }
    let mut f = Vec::new();
    f.extend_from_slice(b"\x93NUMPY");
    f.push(major);
    f.push(minor);
    f.extend_from_slice(&hl.to_le_bytes()[..lb]);
    f.extend_from_slice(&body);
    let n = total * size;
    for k in 0..n {
        f.push(if g.chance(3) {
            g.below(256) as u8
        } else {
            (k * 7 + 1) as u8
        });
    }
    match g.below(16) {
        0 => {
            let cut = g.below(f.len() as u64 + 1) as usize;
            f.truncate(cut);
        }
        1 => {
            let i = g.below(f.len().max(1) as u64) as usize;
            if i < f.len() {
                f[i] = g.below(256) as u8;
            }
        }
        2 => f.truncate(f.len().saturating_sub(g.int(1, 9) as usize)),
        3 => f.extend_from_slice(&[0xab; 5]),
        _ => {}
    }
    f
}

fn psn_name(g: &mut Gen) -> Vec<u8> {
    let num = |g: &mut Gen| -> String {
        match g.below(8) {
            0 => g.pick(&["", "x", "1e", "inf", "-nan", "0x1p3", "1e400", " 5", "+.5"]).into(),
            1 => format!("{}.{}", g.int(0, 3000), g.int(0, 99)),
            _ => g.int(0, 3000).to_string(),
        }
    };
    let mut s: Vec<u8> = Vec::new();
    s.extend_from_slice(g.pick(&[b"ep_b".as_slice(), b"ep_b", b"ep_b", b"xx_y", b"ep_", b""]));
    s.extend_from_slice(num(g).as_bytes());
    if !g.chance(8) {
        s.push(b'#');
    }
    let n = match g.below(8) {
        0 => g.pick(&["", "x", "-3", "99999999999", "2147483648", " 7", "12abc"]).into(),
        _ => g.int(0, 60).to_string(),
    };
    s.extend_from_slice(n.as_bytes());
    if g.chance(30) {
        let k = g.pick(&[98usize, 99, 100, 101, 150]);
        let at = if g.chance(2) { 4.min(s.len()) } else { s.len() };
        let pad: Vec<u8> = std::iter::repeat_n(b'5', k).collect();
        s.splice(at..at, pad);
    }
    if g.chance(15) {
        s = (0..g.int(0, 12)).map(|_| g.int(1, 255) as u8).collect();
    }
    s
}

fn push_mat(g: &mut Gen, t: &mut Vec<String>, rows: i64, cols: i64, finite: bool) {
    t.push(rows.to_string());
    t.push(cols.to_string());
    for _ in 0..rows.max(0) * cols {
        t.push(format!("{:08x}", fbits(g, finite)));
    }
}

/// One case line's tokens.
pub fn generate(g: &mut Gen, kind: &str, portable: bool) -> Vec<String> {
    let mut t: Vec<String> = vec![kind.into()];
    match kind {
        "npy" => {
            let verbose = g.chance(6);
            t.push(i64::from(verbose).to_string());
            if g.chance(25) {
                t.push(g.pick(&["nowhere", "d", "f"]).into());
                t.push("-".into());
            } else {
                t.push("-".into());
                t.push(hex(&npy_file(g, verbose)));
            }
        }
        "gda" => {
            t.push(g.pick(&[0i64, 1, 1, 1, 2, 4, 5, 6, 7]).to_string());
            t.push(i64::from(g.chance(10)).to_string());
            t.push(g.int(1, 4).to_string());
        }
        "gdv" => {
            let isnull = g.chance(20);
            let hasdata = !g.chance(10);
            let ind_ord = g.pick(&[1i64, 1, 2, 2, 0, 3, -1]);
            let num_dim = g.pick(&[1i64, 2, 2, 2, 0, 3]);
            let (d0, d1) = (g.int(0, 5), g.int(0, 5));
            let dt = g.pick(&[
                2i64, 4, 8, 16, 16, 32, 64, 256, 512, 768, 128, 1024, 1536, 1792, 0, 7,
            ]);
            let row = g.int(-1, d0);
            let col = g.int(-1, d1);
            let size = match dt {
                2 | 256 => 1,
                4 | 512 => 2,
                32 | 64 | 1024 => 8,
                128 => 3,
                _ => 4,
            };
            let n = (d0 * if num_dim == 2 { d1 } else { 1 }) as usize * size;
            let data: Vec<u8> = (0..n).map(|_| g.below(256) as u8).collect();
            for v in [
                i64::from(isnull),
                i64::from(hasdata),
                ind_ord,
                num_dim,
                d0,
                d1,
                dt,
                row,
                col,
            ] {
                t.push(v.to_string());
            }
            t.push(hex(&data));
        }
        "bvw" => {
            let which = g.pick(&["v", "g"]);
            t.push(which.into());
            let rows = if g.chance(20) { -1 } else { g.int(0, 4) };
            let cols = if which == "v" {
                g.pick(&[1i64, 1, 1, 1, 0, 2, 3])
            } else {
                g.pick(&[3i64, 3, 3, 3, 1, 2, 4])
            };
            push_mat(g, &mut t, rows, cols, false);
            let mut wheres = vec!["-", "-", "-", "-", "-", "-", "nowhere/x", "d"];
            if !portable {
                wheres.push("ro/x");
            }
            t.push(g.pick(&wheres).into());
        }
        "psn" => {
            t.push(enc(Some(&psn_name(g))));
        }
        _ => {
            t.push(g.pick(&[0i64, 0, 1, -1]).to_string());
            t.push(g.pick(&[0i64, 1, 2, 2, 1, 3]).to_string());
            t.push(g.pick(&[1i64, 2, 1, 2, 0, 3, -1]).to_string());
            let singular = g.chance(12);
            for _ in 0..9 {
                let v = if singular {
                    0.0f32
                } else {
                    g.int(-16, 16) as f32 / 8.0
                };
                t.push(format!("{:08x}", v.to_bits()));
            }
            let rows = if g.chance(10) { -1 } else { g.int(0, 4) };
            let cols = g.pick(&[3i64, 3, 3, 3, 1, 2, 4]);
            let finite = portable || g.chance(2);
            push_mat(g, &mut t, rows, cols, finite);
        }
    }
    t
}

// ---------------------------------------------------------------- the port's answers

/// [`Streams`] onto the program's shared output (gifticlib's messages go to standard error).
struct Sw<'a>(&'a mut dyn Write);

impl Streams for Sw<'_> {
    fn out(&mut self, text: &str) {
        let _ = self.0.write_all(text.as_bytes());
    }
    fn err(&mut self, text: &str) {
        let _ = self.0.write_all(text.as_bytes());
    }
}

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
}

fn read_rmat(
    t: &mut Toks,
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<Option<Matrix>, Fatal> {
    let (rows, cols) = (t.i() as i32, t.i() as i32);
    if rows < 0 {
        return Ok(None);
    }
    let mut m = matrix_alloc(g, out, err, rows, cols, MATRIX_REAL)?.expect("a matrix");
    for r in 1..=rows {
        for c in 1..=cols {
            let v = t.f();
            m.set_relt(r, c, v);
        }
    }
    Ok(Some(m))
}

fn print_rmat(o: &mut Out, m: Option<&Matrix>) {
    let Some(m) = m else {
        o.0.push_str("N ");
        return;
    };
    o.i(i64::from(m.rows));
    o.i(i64::from(m.cols));
    for r in 1..=m.rows {
        for c in 1..=m.cols {
            o.x(&m.relt(r, c).to_bits().to_be_bytes(), 4);
        }
    }
}

fn od(o: &mut Out, v: f64) {
    o.x(&v.to_bits().to_be_bytes(), 8);
}

/// The port's answer to a piece-3 case; `extra` takes `SKIP` / `VARY` where FreeSurfer's line is
/// not comparable (it crashes or reads memory past what it was given) or depends on memory.
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
        let d = n.detail.as_str();
        let skip = matches!(
            n.defect.id,
            "FS-199" | "FS-200" | "FS-201" | "FS-203" | "FS-204" | "FS-205" | "FS-211" | "FS-217"
                | "FS-219"
        ) || (n.defect.id == "FS-207" && d.contains("shape"));
        let vary = matches!(n.defect.id, "FS-202" | "FS-209" | "FS-214")
            || (n.defect.id == "FS-206" && d.contains("memory never written"))
            || (n.defect.id == "FS-207" && d.contains("memory contents"))
            || (matches!(n.defect.id, "FS-218" | "FS-220") && d.contains("memory contents"));
        if skip {
            extra.push("SKIP");
        }
        if vary {
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
    let mut t = Toks { t: &tk[1..], i: 0 };
    let fatal = |f: Fatal| f.status();
    let dat = w.dat.to_str().unwrap().to_owned();
    match tk[0] {
        "npy" => {
            let verbose = t.i() != 0;
            let wh = t.s();
            let data = t.s();
            let path = if wh == "-" {
                std::fs::write(&w.dat, unhex(data)).unwrap();
                dat
            } else {
                wh.to_owned()
            };
            let m = fscnpy::npy2mri(g, out, err, Some(&w.fix), &path, verbose).map_err(fatal)?;
            let Some(m) = m else {
                o.0.push_str("N ");
                return Ok(());
            };
            for v in [m.vg.width, m.vg.height, m.vg.depth, m.nframes, m.mtype] {
                o.i(i64::from(v));
            }
            o.i(m.bytes_per_vox as i64);
            o.i(m.bytes_total as i64);
            o.bytes(m.chunk.as_deref().unwrap_or(&[]));
        }
        "gda" => {
            let mut gx = Gifti::new();
            gx.set_verb(t.i() as i32);
            let isnull = t.i() != 0;
            let n = t.i();
            let mut image = (!isnull).then(GiftiImage::default);
            let mut sw = Sw(err);
            let mut any = false;
            for _ in 0..n {
                let got = gifti::gifti_alloc_and_add_darray(&gx, image.as_mut(), &mut sw).is_some();
                any = got;
                o.i(i64::from(got));
                if let Some(im) = &image {
                    o.i(i64::from(im.num_da));
                    o.i(i64::from(got));
                }
            }
            if any && let Some(da) = image.as_ref().and_then(|im| im.darray.last()?.as_ref()) {
                o.i(i64::from(da.intent));
                o.i(i64::from(da.datatype));
                o.i(i64::from(da.ind_ord));
                o.i(i64::from(da.num_dim));
                for x in da.dims {
                    o.i(i64::from(x));
                }
                o.i(i64::from(da.encoding));
                o.i(i64::from(da.endian));
                o.i(i64::from(da.ext_fname.is_some()));
                o.i(da.ext_offset);
                o.i(da.meta.len() as i64);
                o.i(i64::from(da.num_cs()));
                o.i(i64::from(da.data.is_some()));
                o.i(da.nvals);
                o.i(i64::from(da.nbyper));
                o.i(da.ex_atrs.len() as i64);
            }
        }
        "gdv" => {
            let isnull = t.i() != 0;
            let hasdata = t.i() != 0;
            let mut da = DataArray {
                ind_ord: t.i() as i32,
                num_dim: t.i() as i32,
                ..DataArray::default()
            };
            da.dims[0] = t.i() as i32;
            da.dims[1] = t.i() as i32;
            da.datatype = t.i() as i32;
            let (row, col) = (t.i() as i32, t.i() as i32);
            let mut b = unhex(t.s());
            // the probe's room after the values (zeros)
            b.extend_from_slice(&[0u8; 64]);
            if hasdata {
                da.data = Some(b);
            }
            let da = (!isnull).then_some(da);
            let mut sw = Sw(err);
            let v = gifti::gifti_get_da_value_2d(g, &mut sw, da.as_ref(), row, col).map_err(fatal)?;
            od(o, v);
        }
        "bvw" => {
            let which = t.s();
            let m = read_rmat(&mut t, g, out, err).map_err(fatal)?;
            let wh = t.s();
            let path = if wh == "-" {
                let _ = std::fs::remove_file(&w.dat);
                dat.clone()
            } else {
                wh.to_owned()
            };
            let r = if which == "v" {
                dti::dti_write_bvalues(g, out, Some(&w.fix), m.as_ref(), &path)
            } else {
                dti::dti_write_bvectors(g, out, Some(&w.fix), m.as_ref(), &path)
            };
            o.i(i64::from(r));
            let file = if wh == "-" {
                w.dat.clone()
            } else {
                w.fix.join(wh)
            };
            if file.is_file() {
                o.bytes(&std::fs::read(&file).unwrap());
            } else {
                o.0.push_str("N ");
            }
        }
        "psn" => {
            let s = dec(t.s()).unwrap_or_default();
            let mut b = f64::from_bits(0x7ff8_dead_beef_0001);
            let mut nth = -7777;
            o.i(i64::from(dti::dti_parse_pulse_seq_name(g, &s, &mut b, &mut nth)));
            od(o, b);
            o.i(i64::from(nth));
        }
        _ => {
            g.gdiag_no = t.i() as i32;
            let space = t.i() as i32;
            let desired = t.i() as i32;
            let mut m = mri::mri_alloc_header(g, out, err, 1, 1, 1, MRI_UCHAR, 1).map_err(fatal)?;
            let v: Vec<f32> = (0..9).map(|_| t.f()).collect();
            let vg = &mut m.vg;
            (vg.x_r, vg.x_a, vg.x_s) = (v[0], v[1], v[2]);
            (vg.y_r, vg.y_a, vg.y_s) = (v[3], v[4], v[5]);
            (vg.z_r, vg.z_a, vg.z_s) = (v[6], v[7], v[8]);
            m.bvec_space = space;
            m.bvecs = read_rmat(&mut t, g, out, err).map_err(fatal)?;
            let r = dti::dti_bvec_change_space(g, out, err, &mut m, desired).map_err(fatal)?;
            o.i(i64::from(r));
            o.i(i64::from(m.bvec_space));
            print_rmat(o, m.bvecs.as_ref());
        }
    }
    Ok(())
}
