// SPDX-License-Identifier: Apache-2.0
//! The TIFF port's unit tests: messages and values of small files, a written file's bytes read
//! back, and a digest of many generated files read and written (recorded on Linux x86-64 after
//! the differential probe `validation/probes/fs-tiff` found the port identical to libtiff 3.6.1
//! as FreeSurfer 8.2.0 builds it; the same on every platform).

use super::cases::{self, fnv};
use super::*;
use larmorx_core::cfmt::{Captured, Stream};

fn opts(mode: UpstreamDefects) -> Options {
    Options {
        upstream_defects: mode,
        max_alloc: None,
        messages: TiffMessages::default(),
    }
}

/// A read transcript with the folder's path taken out of the messages.
fn read_text(dir: &std::path::Path, name: &str, mode: UpstreamDefects) -> String {
    let p = dir.join(name);
    cases::reset_thread_state();
    let (out, err, ids, unp) = cases::port_read(p.to_str().expect("path"), opts(mode));
    let prefix = format!("{}{}", dir.display(), std::path::MAIN_SEPARATOR);
    format!("{out}--\n{}--\n{ids:?} {unp:?}\n", err.replace(&prefix, ""))
}

#[test]
fn a_missing_file_and_a_bad_header() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut cap = Captured::default();
    let p = dir.path().join("none.tif");
    let name = p.to_str().expect("path");
    assert!(
        Tiff::open(name, "r", opts(UpstreamDefects::Fix), &mut cap)
            .expect("ported")
            .is_none()
    );
    assert_eq!(
        cap.text(Stream::Err),
        format!("TIFFOpen: {name}: Cannot open.\n")
    );
    std::fs::write(&p, b"XY*\0\0\0\0\0").expect("write");
    let mut cap = Captured::default();
    assert!(
        Tiff::open(name, "r", opts(UpstreamDefects::Fix), &mut cap)
            .expect("ported")
            .is_none()
    );
    assert_eq!(
        cap.text(Stream::Err),
        format!("{name}: Not a TIFF file, bad magic number 22872 (0x5958).\n")
    );
    let mut cap = Captured::default();
    assert!(
        Tiff::open(name, "q", opts(UpstreamDefects::Fix), &mut cap)
            .expect("ported")
            .is_none()
    );
    assert_eq!(cap.text(Stream::Err), "TIFFOpen: \"q\": Bad mode.\n");
}

#[test]
fn a_written_file_reads_back() {
    let dir = tempfile::tempdir().expect("tempdir");
    let p = dir.path().join("w.tif");
    let name = p.to_str().expect("path");
    let c = cases::WriteCase {
        cols: 37,
        rows: 11,
        spp: 1,
        bps: 16,
        sample_format: 2,
        xres: 25.0,
        yres: 0.0,
        compression: compression::DEFLATE,
        data: (0..37 * 11 * 2).map(|i| (i * 7 % 251) as u8).collect(),
    };
    let (out, err, ids) = cases::port_write(name, &c, opts(UpstreamDefects::Fix));
    assert!(err.is_empty(), "{err}");
    assert!(ids.is_empty());
    assert!(out.contains("rows 11 bad 0\nwritedir 1\n"), "{out}");
    let mut cap = Captured::default();
    let mut t = Tiff::open(name, "r", opts(UpstreamDefects::Fix), &mut cap)
        .expect("ported")
        .expect("open");
    assert_eq!(t.get_field(tag::IMAGEWIDTH, &mut cap), Some(Value::U32(37)));
    assert_eq!(
        t.get_field(tag::COMPRESSION, &mut cap),
        Some(Value::U16(compression::DEFLATE))
    );
    assert_eq!(
        t.get_field(tag::XRESOLUTION, &mut cap),
        Some(Value::F32(25.0))
    );
    assert_eq!(
        t.get_field(tag::YRESOLUTION, &mut cap),
        Some(Value::F32(0.0))
    );
    assert_eq!(t.get_field(tag::ROWSPERSTRIP, &mut cap), None);
    let mut row = vec![0u8; t.scanline_size() as usize];
    for r in 0..11u32 {
        assert_eq!(t.read_scanline(&mut row, r, 0, &mut cap), Ok(1));
        assert_eq!(&row[..], &c.data[r as usize * 74..(r as usize + 1) * 74]);
    }
    t.close(&mut cap);
    assert!(cap.text(Stream::Err).is_empty());
}

#[test]
fn lzw_and_packbits_round_trips() {
    let data: Vec<u8> = (0..5000u32)
        .map(|i| ((i / 7) % 13) as u8 ^ (i % 3) as u8)
        .collect();
    for old in [false, true] {
        let z = cases::lzw_encode(&data, old);
        let dir = tempfile::tempdir().expect("tempdir");
        // A one-strip 5000x1 grey image around the strip.
        let mut f = b"II*\0".to_vec();
        let off = 8u32 + z.len() as u32 + (z.len() as u32 & 1);
        f.extend_from_slice(&off.to_le_bytes());
        f.extend_from_slice(&z);
        if f.len() % 2 == 1 {
            f.push(0);
        }
        let e = |t: u16, ty: u16, c: u32, v: u32| {
            let mut b = t.to_le_bytes().to_vec();
            b.extend_from_slice(&ty.to_le_bytes());
            b.extend_from_slice(&c.to_le_bytes());
            b.extend_from_slice(&v.to_le_bytes());
            b
        };
        f.extend_from_slice(&6u16.to_le_bytes());
        f.extend(e(256, 4, 1, 5000));
        f.extend(e(257, 4, 1, 1));
        f.extend(e(258, 3, 1, 8));
        f.extend(e(259, 3, 1, 5));
        f.extend(e(273, 4, 1, 8));
        f.extend(e(279, 4, 1, z.len() as u32));
        f.extend_from_slice(&0u32.to_le_bytes());
        let p = dir.path().join("l.tif");
        std::fs::write(&p, &f).expect("write");
        let mut cap = Captured::default();
        let mut t = Tiff::open(
            p.to_str().expect("path"),
            "r",
            opts(UpstreamDefects::Fix),
            &mut cap,
        )
        .expect("ported")
        .expect("open");
        let mut row = vec![0u8; 5000];
        assert_eq!(t.read_scanline(&mut row, 0, 0, &mut cap), Ok(1));
        assert_eq!(row, data);
        t.close(&mut cap);
        let want = if old {
            format!(
                "{}: Warning, Old-style LZW codes, convert file.\n",
                p.display()
            )
        } else {
            String::new()
        };
        assert_eq!(cap.text(Stream::Err), want);
    }
    let runs: Vec<u8> = (0..3000u32).map(|i| (i / 40) as u8).collect();
    assert!(cases::packbits_encode(&runs).len() < 300);
}

#[test]
fn a_directory_loop_is_counted_once() {
    // FS-814: the second directory links back to the first.
    let dir = tempfile::tempdir().expect("tempdir");
    let mut found = false;
    for i in 0..400 {
        let f = cases::make(77, i);
        let p = dir.path().join(format!("c{i}.tif"));
        std::fs::write(&p, &f).expect("write");
        let (_, _, ids, _) =
            cases::port_read(p.to_str().expect("path"), opts(UpstreamDefects::Fix));
        if ids.contains(&"FS-814") {
            found = true;
            break;
        }
    }
    assert!(found, "no generated file with a looping chain in 400");
}

/// The digest of the generated files read in both modes and the writing cases: recorded after
/// the probe's runs (relay 32: seeds 1 to 6, 15,300 files read, 2,300 written, identical where
/// the original's behaviour does not depend on memory contents).
#[test]
fn generated_files_digest() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut all = Vec::new();
    for i in 0..300 {
        let name = format!("g{i}.tif");
        std::fs::write(dir.path().join(&name), cases::make(4242, i)).expect("write");
        for mode in [UpstreamDefects::Reproduce, UpstreamDefects::Fix] {
            all.extend_from_slice(read_text(dir.path(), &name, mode).as_bytes());
        }
    }
    for i in 0..40 {
        let c = cases::write_case(4242, i);
        let p = dir.path().join(format!("w{i}.tif"));
        cases::reset_thread_state();
        let (out, err, ids) = cases::port_write(
            p.to_str().expect("path"),
            &c,
            opts(UpstreamDefects::Reproduce),
        );
        let prefix = format!("{}{}", dir.path().display(), std::path::MAIN_SEPARATOR);
        all.extend_from_slice(out.as_bytes());
        all.extend_from_slice(err.replace(&prefix, "").as_bytes());
        all.extend_from_slice(format!("{ids:?}").as_bytes());
        all.extend_from_slice(&std::fs::read(&p).expect("written"));
    }
    assert_eq!(fnv(&all), DIGEST, "digest {:#018x}", fnv(&all));
}

const DIGEST: u64 = 0xe67f_3fcd_5a45_2473;

/// CCITT streams made by the generator (its own encoder) decode to the image they encode, in
/// every scheme and Group 3 option, both fill orders.
#[test]
fn ccitt_streams_decode_to_their_images() {
    use super::cases::Rng;
    use super::codecgen::{bilevel, fax_encode, one_strip_file, pack};
    let dir = tempfile::tempdir().expect("tempdir");
    let mut r = Rng::new(77);
    let mut n = 0;
    for (comp, g3) in [
        (2u16, 0u32),
        (32771, 0),
        (3, 0),
        (3, 4),
        (3, 1),
        (3, 5),
        (4, 0),
    ] {
        for width in [1u32, 7, 8, 63, 64, 65, 200, 1800, 2700] {
            for fillorder in [1u32, 2] {
                let rows = 1 + r.below(12) as u32;
                let img = bilevel(&mut r, width, rows);
                let mut data = fax_encode(comp, g3, 1 + n % 3, &img, n % 2 == 0, None);
                if fillorder == 2 {
                    for b in &mut data {
                        *b = b.reverse_bits();
                    }
                }
                let mut extra = vec![(266u16, 3u16, fillorder)];
                if comp == 3 {
                    extra.push((292, 4, g3));
                }
                let file = one_strip_file(width, rows, 1, comp, 0, &extra, &data);
                let p = dir.path().join(format!("c{n}.tif"));
                n += 1;
                std::fs::write(&p, &file).expect("write");
                let mut cap = Captured::default();
                let mut t = Tiff::open(
                    p.to_str().expect("path"),
                    "r",
                    opts(UpstreamDefects::Fix),
                    &mut cap,
                )
                .expect("ported")
                .expect("opened");
                let rb = t.scanline_size() as usize;
                for (y, row) in img.iter().enumerate() {
                    let mut buf = vec![0u8; rb];
                    let rr = t
                        .read_scanline(&mut buf, y as u32, 0, &mut cap)
                        .expect("ported");
                    assert_eq!(
                        rr,
                        1,
                        "comp {comp} g3 {g3} width {width} row {y}: {}",
                        cap.text(Stream::Err)
                    );
                    assert_eq!(
                        buf,
                        pack(row, rb),
                        "comp {comp} g3 {g3} width {width} row {y}"
                    );
                }
                t.close(&mut cap);
                assert_eq!(
                    cap.text(Stream::Err),
                    "",
                    "comp {comp} g3 {g3} width {width}"
                );
            }
        }
    }
}

/// PixarLog strips (deflated 11-bit codes, differenced as libtiff's encoder differences them:
/// from the first pixel for three or four samples per pixel, from the second otherwise) decode to
/// the tables' values of the codes, for 8-bit, 16-bit, float and 11-bit samples.
#[test]
fn pixarlog_strips_decode_to_their_codes() {
    use super::cases::Rng;
    use super::codecgen::one_strip_file;
    let dir = tempfile::tempdir().expect("tempdir");
    let t = super::pixarlog::tables(false);
    let mut r = Rng::new(5);
    let mut n = 0;
    for (bps, sf, size) in [(8u16, 1u32, 1usize), (16, 1, 2), (32, 3, 4), (11, 1, 2)] {
        for spp in [1u32, 2, 3, 4, 5] {
            let (width, rows) = (1 + r.below(40) as u32, 1 + r.below(6) as u32);
            let s = spp as usize;
            let mut codes: Vec<u16> = Vec::new();
            let mut want: Vec<Vec<u16>> = Vec::new();
            for _ in 0..rows {
                let v: Vec<u16> = (0..width as usize * s)
                    .map(|_| r.below(2048) as u16)
                    .collect();
                let mut c = v.clone();
                // the encoder's differences: from the first pixel (3, 4 samples) or the second
                let from = if s == 3 || s == 4 { s } else { 2 * s };
                for k in (from..c.len()).rev() {
                    c[k] = v[k].wrapping_sub(v[k - s]) & 0x7ff;
                }
                codes.extend_from_slice(&c);
                want.push(v);
            }
            let raw: Vec<u8> = codes.iter().flat_map(|c| c.to_le_bytes()).collect();
            let data = larmorx_core::zlib::compress_vec(&raw, 6).expect("deflate");
            let extra = [(277u16, 3u16, spp), (339, 3, sf), (278, 4, rows)];
            let file = one_strip_file(width, rows, bps, 32909, 1, &extra, &data);
            let p = dir.path().join(format!("p{n}.tif"));
            n += 1;
            std::fs::write(&p, &file).expect("write");
            let mut cap = Captured::default();
            let mut tf = Tiff::open(
                p.to_str().expect("path"),
                "r",
                opts(UpstreamDefects::Fix),
                &mut cap,
            )
            .expect("ported")
            .unwrap_or_else(|| panic!("opened: {}", cap.text(Stream::Err)));
            let rb = width as usize * s * size;
            for (y, v) in want.iter().enumerate() {
                let mut buf = vec![0u8; rb.max(tf.scanline_size() as usize)];
                let rr = tf
                    .read_scanline(&mut buf, y as u32, 0, &mut cap)
                    .expect("ported");
                if bps == 11 {
                    // a scanline of 11-bit samples holds fewer codes than a row: cut to none
                    assert_eq!(rr, 1, "{}", cap.text(Stream::Err));
                    continue;
                }
                assert_eq!(rr, 1, "bps {bps} spp {spp}: {}", cap.text(Stream::Err));
                for (k, &c) in v.iter().enumerate() {
                    let c = c as usize;
                    let got = &buf[k * size..(k + 1) * size];
                    let exp: Vec<u8> = match bps {
                        8 => vec![t.to_8[c]],
                        16 => t.to_16[c].to_le_bytes().to_vec(),
                        _ => t.to_f[c].to_le_bytes().to_vec(),
                    };
                    assert_eq!(got, &exp[..], "bps {bps} spp {spp} row {y} sample {k}");
                }
            }
            tf.close(&mut cap);
        }
    }
}
