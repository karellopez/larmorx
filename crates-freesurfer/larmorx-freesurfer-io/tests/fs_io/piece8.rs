// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The fs.io probe's piece-8 cases (relay 18): FreeSurfer's 2-D images. `ial` (`ImageAlloc`),
//! `icp` (`ImageCopy`), `ica` (`ImageClearArea`), `ick` (`ImageCheckSize`), `iss`
//! (`ImageSetSize`), `icf` (`ImageCopyFrames`), `irs` (`ImageRescale`), `isd` and `isu`
//! (`ImageScaleDown`, `ImageScaleUp` called directly), `ird` (`ImageReduce`), `ic1` and `ir1`
//! (`ImageConvolve1d`, `ImageReduce1d` with a given kernel). Each case starts with `Gerror`'s
//! value; an image is given as `ImageAlloc`'s arguments, its extents (not the stored ones; -7:
//! as allocated), its pixel sizes and its bytes (or `N` for none). Images are small (at most 12
//! by 12 pixels and 3 frames), with odd extents, frames, formats and buffers now and then, and
//! floats of every kind (NaN with payloads, infinities, -0, subnormal, beyond the byte and int
//! ranges).

use std::io::Write;

use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_io::image::{self, Image};

use super::{Gen, Out, Work, hex};

/// The piece-8 commands.
pub const KINDS: [&str; 12] = [
    "ial", "icp", "ica", "ick", "iss", "icf", "irs", "isd", "isu", "ird", "ic1", "ir1",
];

// ---------------------------------------------------------------- generation

fn fbits(g: &mut Gen) -> u32 {
    match g.below(24) {
        0 => g.pick(&[
            0x7fc0_0000u32,
            0xffc0_0000,
            0x7fc1_2345,
            0xff80_0001,
            0x7f80_0001,
        ]),
        1 => g.pick(&[
            0x7f80_0000u32,
            0xff80_0000,
            0x8000_0000,
            0x0000_0001,
            0x0080_0000,
        ]),
        2 => g.pick(&[
            300.5f32.to_bits(),
            (-1.5f32).to_bits(),
            (-200.0f32).to_bits(),
            3e9f32.to_bits(),
            (-3e9f32).to_bits(),
            255.9f32.to_bits(),
            1e30f32.to_bits(),
        ]),
        3..=6 => (g.int(0, 255) as f32).to_bits(),
        _ => ((g.int(-40000, 40000) as f32) / 64.0).to_bits(),
    }
}

fn fhex(g: &mut Gen) -> String {
    format!("{:08x}", fbits(g))
}

const FORMATS: [i32; 9] = [
    image::PFBYTE,
    image::PFFLOAT,
    image::PFINT,
    image::PFDOUBLE,
    image::PFRGB,
    image::PFSHORT,
    image::PFCOMPLEX,
    image::PFMSBF,
    99,
];

fn ext(g: &mut Gen) -> i64 {
    if g.chance(25) {
        g.pick(&[0, -1, 13])
    } else {
        g.int(1, 12)
    }
}

/// The bytes of an image of `len` bytes and format `fmt`.
fn pixels(g: &mut Gen, fmt: i64, len: u64) -> Vec<u8> {
    let mut b = Vec::with_capacity(len as usize);
    let float = fmt == i64::from(image::PFFLOAT);
    let int = fmt == i64::from(image::PFINT);
    while (b.len() as u64) < len {
        if float && len.is_multiple_of(4) {
            b.extend_from_slice(&fbits(g).to_le_bytes());
        } else if int && len.is_multiple_of(4) {
            let v: u32 = match g.below(6) {
                0 => g.pick(&[u32::MAX, 0x8000_0000, 0x0100_0001, 0xffff_ff01]),
                _ => g.below(300) as u32,
            };
            b.extend_from_slice(&v.to_le_bytes());
        } else {
            b.push(g.below(256) as u8);
        }
    }
    b.truncate(len as usize);
    b
}

/// One image's tokens: `rows cols format frames bufsize setrows setcols xsize ysize bytes`.
fn img(g: &mut Gen, t: &mut Vec<String>, fmts: &[i32], rows: Option<i64>, cols: Option<i64>) {
    let fmt = if g.chance(12) {
        i64::from(g.pick(&FORMATS))
    } else {
        i64::from(g.pick(fmts))
    };
    let rows = rows.unwrap_or_else(|| ext(g));
    let cols = cols.unwrap_or_else(|| ext(g));
    let nf = if g.chance(20) {
        g.pick(&[0, -1])
    } else {
        g.int(1, 3)
    };
    let sizepix: i64 = match fmt as i32 {
        image::PFFLOAT | image::PFINT => 4,
        image::PFDOUBLE => 8,
        image::PFSHORT => 2,
        image::PFRGB => 3,
        _ => 1,
    };
    let numpix = (rows as u64).wrapping_mul(cols as u64);
    let sizeimage = numpix.wrapping_mul(sizepix as u64);
    let mut bs = 0u64;
    let mut len = sizeimage.wrapping_mul(nf as u64);
    if g.chance(20) && sizeimage > 0 {
        bs = g.pick(&[
            1u64,
            4,
            8,
            len.saturating_sub(4).max(1),
            len.wrapping_add(8),
        ]);
        len = bs;
    }
    let len = if len > 1 << 20 || sizeimage == 0 {
        0
    } else {
        len
    };
    let (sr, sc) = if g.chance(10) {
        (rows + g.int(-1, 1), cols + g.int(-1, 1))
    } else {
        (-7, -7)
    };
    let px = pixels(g, fmt, len);
    t.extend([
        rows.to_string(),
        cols.to_string(),
        fmt.to_string(),
        nf.to_string(),
        bs.to_string(),
        sr.to_string(),
        sc.to_string(),
        fhex(g),
        fhex(g),
        hex(&px),
    ]);
}

fn scale(g: &mut Gen, up: Option<bool>) -> String {
    let s: f32 = match g.below(20) {
        0 => f32::from_bits(g.pick(&[0x7fc0_0000u32, 0x7f80_0000, 0xff80_0000, 0x8000_0000, 0])),
        1 => g.pick(&[-0.5f32, -2.0, 1.0, 1e-30, 1e30]),
        _ => match up {
            Some(true) => g.pick(&[1.5f32, 2.0, 2.5, 3.0, 4.0, 1.25]),
            Some(false) => g.pick(&[0.5f32, 0.25, 0.75, 0.3, 0.125, 0.6, 0.4999999]),
            None => g.pick(&[0.5f32, 0.25, 0.75, 1.5, 2.0, 3.0, 1.0, 0.3, 0.125]),
        },
    };
    format!("{:08x}", s.to_bits())
}

pub fn generate(g: &mut Gen, kind: &str, _portable: bool) -> Vec<String> {
    let mut t: Vec<String> = vec![kind.into()];
    let gerror = if g.chance(5) {
        g.pick(&[-4i64, 5, -2])
    } else {
        0
    };
    t.push(gerror.to_string());
    let bf = [image::PFBYTE, image::PFFLOAT];
    match kind {
        "ial" => {
            let (rows, cols, fmt, nf) = match g.below(10) {
                0 => (
                    1i64 << 20,
                    1i64 << 20,
                    i64::from(image::PFDOUBLE),
                    1i64 << 10,
                ),
                1 => (-2, 2, i64::from(g.pick(&FORMATS)), 1),
                2 => (-1, -1, i64::from(g.pick(&FORMATS)), g.pick(&[1, -1, 0])),
                _ => (ext(g), ext(g), i64::from(g.pick(&FORMATS)), g.int(-1, 3)),
            };
            let bs = if g.chance(6) { g.int(1, 64) } else { 0 };
            t.extend([rows, cols, fmt, nf, bs].map(|v| v.to_string()));
        }
        "icp" => {
            img(
                g,
                &mut t,
                &[image::PFFLOAT, image::PFBYTE, image::PFINT],
                None,
                None,
            );
            if g.chance(3) {
                t.push("N".into());
            } else {
                img(
                    g,
                    &mut t,
                    &[image::PFFLOAT, image::PFBYTE, image::PFINT],
                    None,
                    None,
                );
            }
        }
        "ica" => {
            img(g, &mut t, &bf, None, None);
            for _ in 0..4 {
                t.push(
                    if g.chance(4) {
                        g.int(-2, 14)
                    } else {
                        g.int(0, 6)
                    }
                    .to_string(),
                );
            }
            t.push(fhex(g));
            t.push(if g.chance(5) { g.int(0, 3) } else { -1 }.to_string());
        }
        "ick" => {
            img(g, &mut t, &bf, None, None);
            if g.chance(6) {
                t.push("N".into());
            } else {
                img(g, &mut t, &bf, None, None);
            }
            for _ in 0..3 {
                t.push(if g.chance(3) { 0 } else { g.int(-2, 14) }.to_string());
            }
        }
        "iss" => {
            img(g, &mut t, &bf, None, None);
            t.push(g.int(-1, 13).to_string());
            t.push(g.int(-1, 13).to_string());
        }
        "icf" => {
            let r = ext(g);
            let c = ext(g);
            let fm = [
                image::PFFLOAT,
                image::PFBYTE,
                image::PFINT,
                image::PFDOUBLE,
                image::PFRGB,
            ];
            img(g, &mut t, &fm, Some(r), Some(c));
            if g.chance(8) {
                t.push("N".into());
            } else {
                let same = g.chance(2);
                let (r2, c2) = if same { (r, c) } else { (ext(g), ext(g)) };
                img(g, &mut t, &fm, Some(r2), Some(c2));
            }
            t.push(g.int(0, 2).to_string());
            t.push(g.int(0, 3).to_string());
            t.push(g.int(0, 2).to_string());
        }
        "irs" => {
            img(g, &mut t, &bf, None, None);
            if g.chance(4) {
                img(g, &mut t, &bf, None, None);
            } else {
                t.push("N".into());
            }
            t.push(scale(g, None));
        }
        "isd" | "isu" => {
            let up = kind == "isu";
            let r = ext(g);
            let c = ext(g);
            let fm = [image::PFFLOAT, image::PFBYTE, image::PFINT];
            img(g, &mut t, &fm, Some(r), Some(c));
            // an output of the scaled extents (or of others)
            let k = if up { 3 } else { 1 };
            if g.chance(5) {
                img(g, &mut t, &fm, None, None);
            } else {
                img(g, &mut t, &fm, Some(r * k + 1), Some(c * k + 1));
            }
            t.push(scale(g, Some(up)));
        }
        "ird" => {
            let fm = [image::PFFLOAT];
            img(g, &mut t, &fm, None, None);
            if g.chance(3) {
                t.push("N".into());
            } else {
                img(g, &mut t, &fm, None, None);
            }
        }
        "ic1" | "ir1" => {
            let fm = [image::PFFLOAT];
            let r = ext(g);
            let c = ext(g);
            img(g, &mut t, &fm, Some(r), Some(c));
            if kind == "ic1" {
                img(g, &mut t, &fm, Some(r), Some(c));
            } else {
                let (r2, c2) = (r / 2 + g.int(0, 1), c / 2 + g.int(0, 1));
                img(g, &mut t, &fm, Some(r2), Some(c2));
            }
            t.push(g.pick(&["0", "1"]).into());
            let len = g.int(1, 7);
            t.push(len.to_string());
            for _ in 0..len {
                let v = if g.chance(8) {
                    g.pick(&[0.25f32, -1.0, 1e30, 0.0])
                } else {
                    (g.int(-64, 64) as f32) / 64.0
                };
                t.push(format!("{:08x}", v.to_bits()));
            }
        }
        _ => unreachable!(),
    }
    t
}

// ---------------------------------------------------------------- the port's answers

fn of(o: &mut Out, f: f32) {
    o.x(&f.to_bits().to_be_bytes(), 4);
}

fn fnv(o: &mut Out, b: &[u8]) {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &c in b {
        h ^= u64::from(c);
        h = h.wrapping_mul(1_099_511_628_211);
    }
    o.0.push_str(&format!("fnv{h:016x} "));
}

fn print_img(o: &mut Out, i: Option<&Image>) {
    let Some(i) = i else {
        o.0.push_str("N ");
        return;
    };
    for v in [
        i.num_frame,
        i.orows,
        i.ocols,
        i.rows,
        i.cols,
        i.frow,
        i.fcol,
        i.pixel_format,
        i.numcolor,
    ] {
        o.i(i64::from(v));
    }
    o.i(i.numpix as i64);
    o.i(i64::from(i.sizepix));
    o.i(i.sizeimage as i64);
    o.i(i64::from(i.imdealloc));
    match i.firstpix {
        None => o.0.push_str("N "),
        Some(f) => o.i(f),
    }
    of(o, i.xsize);
    of(o, i.ysize);
    let Some(b) = &i.image else {
        o.0.push_str("B- ");
        return;
    };
    let n = i
        .sizeimage
        .wrapping_mul(i.num_frame as i64 as u64)
        .min(b.len() as u64) as usize;
    if n <= 128 {
        o.bytes(&b[..n]);
    } else {
        fnv(o, &b[..n]);
    }
}

fn mkimg(
    tk: &[&str],
    i: &mut usize,
    g: &mut Globals,
    err: &mut dyn Write,
) -> Result<Option<Image>, u8> {
    if tk[*i] == "N" {
        *i += 1;
        return Ok(None);
    }
    let v: Vec<&str> = tk[*i..*i + 10].to_vec();
    *i += 10;
    let n = |k: usize| v[k].parse::<i64>().unwrap() as i32;
    let bs: u64 = v[4].parse().unwrap();
    let mut im = image::image_alloc(g, err, n(0), n(1), n(2), n(3), bs).map_err(|f| f.status())?;
    if n(5) != -7 {
        im.rows = n(5);
    }
    if n(6) != -7 {
        im.cols = n(6);
    }
    im.xsize = f32::from_bits(u32::from_str_radix(v[7], 16).unwrap());
    im.ysize = f32::from_bits(u32::from_str_radix(v[8], 16).unwrap());
    let px = super::unhex(v[9]);
    if let Some(b) = im.image.as_mut() {
        let k = px.len().min(b.len());
        b[..k].copy_from_slice(&px[..k]);
    }
    Ok(Some(im))
}

fn getf(s: &str) -> f32 {
    f32::from_bits(u32::from_str_radix(s, 16).unwrap())
}

pub fn run(
    tk: &[&str],
    g: &mut Globals,
    _out: &mut dyn Write,
    err: &mut dyn Write,
    o: &mut Out,
    _w: &Work,
    extra: &mut Vec<&'static str>,
) -> Result<(), u8> {
    let r = answer(tk, g, err, o);
    for n in g.notices.as_slice() {
        // FreeSurfer reads or writes outside the pixels, or uses an image it freed
        if matches!(n.defect.id, "FS-405" | "FS-409") {
            extra.push("SKIP");
        }
    }
    r
}

fn answer(tk: &[&str], g: &mut Globals, err: &mut dyn Write, o: &mut Out) -> Result<(), u8> {
    let st = |f: larmorx_freesurfer_core::log::Fatal| f.status();
    let mut i = 1;
    g.gerror = tk[i].parse().unwrap();
    i += 1;
    let int = |i: &mut usize| {
        let v = tk[*i].parse::<i64>().unwrap() as i32;
        *i += 1;
        v
    };
    match tk[0] {
        "ial" => {
            let (rows, cols, fmt, nf) = (int(&mut i), int(&mut i), int(&mut i), int(&mut i));
            let bs: u64 = tk[i].parse().unwrap();
            let im = image::image_alloc(g, err, rows, cols, fmt, nf, bs).map_err(st)?;
            print_img(o, Some(&im));
        }
        "icp" => {
            let mut s = mkimg(tk, &mut i, g, err)?.unwrap();
            let d = mkimg(tk, &mut i, g, err)?;
            let r = image::image_copy(g, err, &mut s, d).map_err(st)?;
            print_img(o, Some(&s));
            print_img(o, Some(&r));
        }
        "ica" => {
            let mut im = mkimg(tk, &mut i, g, err)?.unwrap();
            let (r0, c0, rows, cols) = (int(&mut i), int(&mut i), int(&mut i), int(&mut i));
            let val = getf(tk[i]);
            i += 1;
            let frame = int(&mut i);
            let r = image::image_clear_area(g, err, &mut im, r0, c0, rows, cols, val, frame);
            o.i(i64::from(r));
            print_img(o, Some(&im));
        }
        "ick" => {
            let a = mkimg(tk, &mut i, g, err)?.unwrap();
            let b = mkimg(tk, &mut i, g, err)?;
            let (rows, cols, nf) = (int(&mut i), int(&mut i), int(&mut i));
            o.i(i64::from(image::image_check_size(
                &a,
                b.as_ref(),
                rows,
                cols,
                nf,
            )));
        }
        "iss" => {
            let mut im = mkimg(tk, &mut i, g, err)?.unwrap();
            let (rows, cols) = (int(&mut i), int(&mut i));
            o.i(i64::from(image::image_set_size(&mut im, rows, cols)));
            print_img(o, Some(&im));
        }
        "icf" => {
            let a = mkimg(tk, &mut i, g, err)?.unwrap();
            let mut b = mkimg(tk, &mut i, g, err)?;
            let (start, nf, dst) = (int(&mut i), int(&mut i), int(&mut i));
            let r = image::image_copy_frames(g, err, &a, b.as_mut(), start, nf, dst).map_err(st)?;
            o.i(i64::from(r));
            print_img(o, b.as_ref());
        }
        "irs" => {
            let mut a = mkimg(tk, &mut i, g, err)?.unwrap();
            let b = mkimg(tk, &mut i, g, err)?;
            let s = getf(tk[i]);
            let r = image::image_rescale(g, err, &mut a, b, s).map_err(st)?;
            print_img(o, Some(&a));
            print_img(o, Some(&r));
        }
        "isd" | "isu" => {
            let mut a = mkimg(tk, &mut i, g, err)?.unwrap();
            let mut b = mkimg(tk, &mut i, g, err)?.unwrap();
            let s = getf(tk[i]);
            let r = if tk[0] == "isd" {
                image::image_scale_down(g, err, &mut a, &mut b, s)
            } else {
                image::image_scale_up(g, err, &mut a, &mut b, s)
            }
            .map_err(st)?;
            o.i(i64::from(r));
            print_img(o, Some(&a));
            print_img(o, Some(&b));
        }
        "ird" => {
            let a = mkimg(tk, &mut i, g, err)?.unwrap();
            let b = mkimg(tk, &mut i, g, err)?;
            let r = image::image_reduce(g, err, &a, b).map_err(st)?;
            print_img(o, Some(&r));
        }
        "ic1" | "ir1" => {
            let a = mkimg(tk, &mut i, g, err)?.unwrap();
            let mut b = mkimg(tk, &mut i, g, err)?.unwrap();
            let axis = int(&mut i);
            let len = int(&mut i);
            let k: Vec<f32> = (0..len as usize).map(|n| getf(tk[i + n])).collect();
            if tk[0] == "ic1" {
                image::image_convolve1d(g, err, &a, &mut b, &k, axis).map_err(st)?;
            } else {
                image::image_reduce1d(g, &a, &mut b, &k, axis);
            }
            print_img(o, Some(&b));
        }
        _ => unreachable!(),
    }
    o.i(i64::from(g.gerror));
    Ok(())
}
