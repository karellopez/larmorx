// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! fs.io's TIFF images (`TiffReadImage`, `TiffWriteImage`; relay 40 of track A): volumes written
//! as TIFF files and read back, hand-made files of one and several pages, stripped and tiled, and
//! FS-863 to FS-874 in both modes. FreeSurfer's own outputs for these paths are the probe's
//! (`validation/probes/fs-io`, piece 23); these tests pin the corrected defaults.

use std::path::Path;

use larmorx_core::defects::UpstreamDefects;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_io::mriio;
use larmorx_freesurfer_mri::mri::{self, MRI_FLOAT, MRI_UCHAR, Mri};
use larmorx_freesurfer_mri::voxel::{mri_get_vox_val, mri_set_vox_val};

const MODES: [UpstreamDefects; 2] = [UpstreamDefects::Fix, UpstreamDefects::Reproduce];

fn dir() -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix("fs tiff ~")
        .tempdir()
        .unwrap()
}

/// A little-endian TIFF file of one or more pages: each page's tags (tag, type, values) and its
/// data blocks (strips or tiles), the offsets and byte counts filled in.
struct Page {
    tags: Vec<(u16, u16, Vec<u32>)>,
    blocks: Vec<Vec<u8>>,
    tiled: bool,
}

fn tiff_file(pages: &[Page]) -> Vec<u8> {
    let mut f = b"II*\0\0\0\0\0".to_vec();
    let mut link = 4usize;
    for p in pages {
        let mut offs = Vec::new();
        for b in &p.blocks {
            offs.push(f.len() as u32);
            f.extend_from_slice(b);
            if f.len() % 2 == 1 {
                f.push(0);
            }
        }
        let counts: Vec<u32> = p.blocks.iter().map(|b| b.len() as u32).collect();
        let (ot, ct) = if p.tiled { (324, 325) } else { (273, 279) };
        let mut tags = p.tags.clone();
        tags.push((ot, 4, offs));
        tags.push((ct, 4, counts));
        tags.sort_by_key(|t| t.0);
        // the values that do not fit in an entry, after the directory
        let ifd = f.len();
        f[link..link + 4].copy_from_slice(&(ifd as u32).to_le_bytes());
        let n = tags.len();
        let mut extra = ifd + 2 + 12 * n + 4;
        let mut tail = Vec::new();
        f.extend_from_slice(&(n as u16).to_le_bytes());
        for (tg, ty, v) in &tags {
            f.extend_from_slice(&tg.to_le_bytes());
            f.extend_from_slice(&ty.to_le_bytes());
            f.extend_from_slice(&(v.len() as u32).to_le_bytes());
            let size = if *ty == 3 { 2 } else { 4 } * v.len() * if *ty == 5 { 2 } else { 1 };
            let bytes: Vec<u8> = match ty {
                3 => v.iter().flat_map(|x| (*x as u16).to_le_bytes()).collect(),
                5 => v
                    .iter()
                    .flat_map(|x| [x.to_le_bytes(), 1u32.to_le_bytes()].concat())
                    .collect(),
                _ => v.iter().flat_map(|x| x.to_le_bytes()).collect(),
            };
            if size <= 4 {
                let mut b = bytes.clone();
                b.resize(4, 0);
                f.extend_from_slice(&b);
            } else {
                f.extend_from_slice(&(extra as u32).to_le_bytes());
                extra += bytes.len();
                tail.extend_from_slice(&bytes);
            }
        }
        link = f.len();
        f.extend_from_slice(&[0; 4]);
        f.extend_from_slice(&tail);
    }
    f
}

/// A stripped page of one strip: width, height, samples, bits, orientation, fill order.
fn strip_page(w: u32, h: u32, spp: u32, bps: u32, orient: u32, fill: u32, data: Vec<u8>) -> Page {
    Page {
        tags: vec![
            (256, 4, vec![w]),
            (257, 4, vec![h]),
            (258, 3, vec![bps; spp as usize]),
            (259, 3, vec![1]),
            (262, 3, vec![if spp >= 3 { 2 } else { 1 }]),
            (266, 3, vec![fill]),
            (274, 3, vec![orient]),
            (277, 3, vec![spp]),
            (278, 4, vec![h]),
            (284, 3, vec![1]),
        ],
        blocks: vec![data],
        tiled: false,
    }
}

/// A tiled page: tiles of `tw` x `tl` RGB pixels whose bytes are `f(tile, offset)`.
fn tiled_page(w: u32, h: u32, tw: u32, tl: u32, spp: u32, orient: u32) -> Page {
    let tiles = w.div_ceil(tw) * h.div_ceil(tl);
    let blocks = (0..tiles)
        .map(|t| {
            (0..tw * tl * spp)
                .map(|k| (t * 37 + k) as u8)
                .collect::<Vec<u8>>()
        })
        .collect();
    Page {
        tags: vec![
            (256, 4, vec![w]),
            (257, 4, vec![h]),
            (258, 3, vec![8; spp as usize]),
            (259, 3, vec![1]),
            (262, 3, vec![if spp >= 3 { 2 } else { 1 }]),
            (274, 3, vec![orient]),
            (277, 3, vec![spp]),
            (284, 3, vec![1]),
            (322, 4, vec![tw]),
            (323, 4, vec![tl]),
        ],
        blocks,
        tiled: true,
    }
}

struct Run {
    vol: Option<Mri>,
    out: String,
    err: String,
    ids: Vec<&'static str>,
    exit: Option<i32>,
}

fn read(mode: UpstreamDefects, d: &Path, name: &str, env: &[(&str, &str)]) -> Run {
    larmorx_freesurfer_volume::image::register_io();
    let mut g = Globals::new(mode);
    let env = Environment::from_pairs(env.iter().map(|(a, b)| (*a, *b)));
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let r = mriio::mri_read(
        &mut g,
        &mut out,
        &mut err,
        &env,
        Some(d),
        name,
        -1,
        true,
        -1,
        -1,
    );
    let (vol, exit) = match r {
        Ok(v) => (v, None),
        Err(Fatal::Exit(c)) => (None, Some(c)),
        Err(e) => panic!("{e:?}"),
    };
    Run {
        vol,
        out: String::from_utf8_lossy(&out).into_owned(),
        err: String::from_utf8_lossy(&err).into_owned(),
        ids: g.notices.as_slice().iter().map(|n| n.defect.id).collect(),
        exit,
    }
}

fn vox(m: &Mri, x: i32, y: i32, f: i32) -> f32 {
    let mut g = Globals::new(UpstreamDefects::Fix);
    mri_get_vox_val(&mut g, &mut Vec::new(), m, x, y, 0, f, 0).unwrap()
}

/// The first page's SampleFormat (tag 339) of a little-endian TIFF file.
fn sample_format(b: &[u8]) -> Option<u16> {
    let u16_at = |o: usize| u16::from_le_bytes([b[o], b[o + 1]]);
    let ifd = u32::from_le_bytes([b[4], b[5], b[6], b[7]]) as usize;
    let n = u16_at(ifd) as usize;
    (0..n)
        .map(|k| ifd + 2 + 12 * k)
        .find(|&e| u16_at(e) == 339)
        .map(|e| u16_at(e + 8))
}

/// A volume written as a TIFF file (bytes and floats, two frames: the image holds the first) and
/// read back: the voxels as written; FS-874's sample formats in the file (bytes 1 and floats 3 by default, 2
/// with `reproduce`).
#[test]
fn volumes_written_and_read_back() {
    let d = dir();
    for mode in MODES {
        for ty in [MRI_UCHAR, MRI_FLOAT] {
            let mut g = Globals::new(mode);
            larmorx_freesurfer_volume::image::register_io();
            let env = Environment::default();
            let (mut out, mut err) = (Vec::new(), Vec::new());
            let mut m =
                mri::mri_alloc_sequence(&mut g, &mut out, &mut err, 5, 3, 1, ty, 2).unwrap();
            for f in 0..2 {
                for y in 0..3 {
                    for x in 0..5 {
                        let v = (x + 5 * y + 15 * f) as f32 * 1.5;
                        mri_set_vox_val(&mut g, &mut err, &mut m, x, y, 0, f, v, 0).unwrap();
                    }
                }
            }
            let name = format!("o{}{}.tif", ty, mode.name());
            let r = mriio::mri_write(
                &mut g,
                &mut out,
                &mut err,
                &env,
                Some(d.path()),
                &mut m,
                &name,
                None,
                0,
            )
            .unwrap();
            assert_eq!(r, 0, "{}", String::from_utf8_lossy(&err));
            let ids: Vec<_> = g.notices.as_slice().iter().map(|n| n.defect.id).collect();
            assert!(ids.contains(&"FS-874"), "{ids:?}");
            let b = std::fs::read(d.path().join(&name)).unwrap();
            let want: u16 = match (mode.fix(), ty) {
                (false, _) => 2,
                (true, MRI_UCHAR) => 1,
                _ => 3,
            };
            assert_eq!(sample_format(&b), Some(want), "{mode:?} {ty}");
            let back = read(mode, d.path(), &name, &[]);
            let v = back.vol.expect(&back.err);
            // MRItoImage takes the first frame: one page
            assert_eq!(v.nframes, 1);
            assert_eq!((v.width, v.height), (5, 3));
            for f in 0..1 {
                for y in 0..3 {
                    for x in 0..5 {
                        let want = (x + 5 * y + 15 * f) as f32 * 1.5;
                        let want = if ty == MRI_UCHAR { want.round() } else { want };
                        assert_eq!(vox(&v, x, y, f), want, "{mode:?} {ty} {x} {y} {f}");
                    }
                }
            }
        }
    }
}

/// FS-866: a 1-bit top-left image 5 pixels wide, its bits low bit first (fill order 2). The
/// default gives each pixel its bit; FreeSurfer's unpacking (`reproduce`) spills 3 pixels of each
/// row into the row above it and mirrors the bits of each byte.
#[test]
fn one_bit_rows() {
    let d = dir();
    let data = vec![0b1010_0110u8, 0b0001_1101, 0b1111_0000];
    let f = tiff_file(&[strip_page(5, 3, 1, 1, 1, 2, data.clone())]);
    std::fs::write(d.path().join("b.tif"), f).unwrap();
    for mode in MODES {
        let r = read(mode, d.path(), "b.tif", &[]);
        assert!(r.ids.contains(&"FS-866"), "{:?}", r.ids);
        let v = r.vol.expect(&r.err);
        let got: Vec<u8> = (0..3)
            .flat_map(|y| (0..5).map(move |x| (x, y)))
            .map(|(x, y)| vox(&v, x, y, 0) as u8)
            .collect();
        let want: Vec<u8> = (0..3)
            .flat_map(|y| (0..5).map(move |x| (y, x)))
            .map(|(y, x)| (data[y] >> x) & 1)
            .collect();
        if mode.fix() {
            assert_eq!(got, want, "{}", r.err);
        } else {
            assert_ne!(got, want);
        }
    }
}

/// FS-870: a tiled RGB image 32 pixels wide in tiles of 16 (the width a multiple of the tile
/// width), top-left and bottom-left: the default places every tile; FreeSurfer (`reproduce`)
/// misplaces those after the first row of tiles and reads the bottom-left image as top-left.
#[test]
fn tiled_images() {
    let d = dir();
    for orient in [1u32, 4] {
        let name = format!("t{orient}.tif");
        let f = tiff_file(&[tiled_page(32, 32, 16, 16, 3, orient)]);
        std::fs::write(d.path().join(&name), f).unwrap();
        for mode in MODES {
            let r = read(mode, d.path(), &name, &[]);
            assert!(r.ids.contains(&"FS-870"), "{:?}", r.ids);
            let v = r.vol.expect(&r.err);
            let mut ok = true;
            for row in 0..32u32 {
                for x in 0..32u32 {
                    let t = (row / 16) * 2 + x / 16;
                    let k = ((row % 16) * 16 + x % 16) * 3;
                    let px = |c: u32| (t * 37 + k + c) & 255;
                    let want = px(0) | px(1) << 8 | px(2) << 16;
                    let y = if orient == 1 { row } else { 31 - row };
                    ok &= vox(&v, x as i32, y as i32, 0) as u32 == want;
                }
            }
            assert_eq!(ok, mode.fix(), "{mode:?} orientation {orient}: {}", r.err);
        }
    }
}

/// FS-869: two 1-bit pages: the default puts the second page in the second frame; FreeSurfer
/// (`reproduce`) reads both onto the first (the pages' sizes are 0 bytes).
#[test]
fn one_bit_pages() {
    let d = dir();
    let p0 = strip_page(8, 2, 1, 1, 1, 1, vec![0xff, 0x00]);
    let p1 = strip_page(8, 2, 1, 1, 1, 1, vec![0x0f, 0xf0]);
    std::fs::write(d.path().join("p.tif"), tiff_file(&[p0, p1])).unwrap();
    for mode in MODES {
        let r = read(mode, d.path(), "p.tif", &[]);
        assert!(r.ids.contains(&"FS-869"), "{:?}", r.ids);
        let v = r.vol.expect(&r.err);
        assert_eq!(v.nframes, 2);
        let frame = |f: i32| -> Vec<u8> {
            (0..2)
                .flat_map(|y| (0..8).map(move |x| (x, y)))
                .map(|(x, y)| vox(&v, x, y, f) as u8)
                .collect()
        };
        let page = |b: [u8; 2]| -> Vec<u8> {
            (0..2)
                .flat_map(|y| (0..8).map(move |x| (b[y] >> (7 - x)) & 1))
                .collect()
        };
        if mode.fix() {
            assert_eq!(frame(0), page([0xff, 0x00]));
            assert_eq!(frame(1), page([0x0f, 0xf0]));
        } else {
            assert_eq!(frame(0), page([0x0f, 0xf0]));
            assert_eq!(frame(1), vec![0; 16]);
        }
    }
}

/// FS-864: frames past the pages, and FS-868: a page of separate planes. The default refuses
/// them (`mri_read` fails); FreeSurfer (`reproduce`) reads the stack or ends on its allocation.
#[test]
fn frames_and_planes_refused() {
    let d = dir();
    let p = strip_page(4, 3, 1, 8, 1, 1, (0..12).collect());
    std::fs::write(d.path().join("f.tif"), tiff_file(&[p])).unwrap();
    let mut sep = strip_page(4, 3, 3, 8, 1, 1, (0..36).collect());
    sep.tags.retain(|t| t.0 != 284);
    sep.tags.push((284, 3, vec![2]));
    sep.blocks = (0..3).map(|k| (12 * k..12 * (k + 1)).collect()).collect();
    sep.tags.retain(|t| t.0 != 278);
    sep.tags.push((278, 4, vec![3]));
    std::fs::write(d.path().join("s.tif"), tiff_file(&[sep])).unwrap();
    let fix = UpstreamDefects::Fix;
    let r = read(fix, d.path(), "f.tif#2", &[]);
    assert!(r.vol.is_none() && r.ids.contains(&"FS-864"), "{:?}", r.ids);
    assert!(
        r.err.contains("frames 2 to 2 asked of a file of 1 pages"),
        "{}",
        r.err
    );
    let r = read(fix, d.path(), "s.tif", &[]);
    assert!(r.vol.is_none() && r.ids.contains(&"FS-868"), "{:?}", r.ids);
    assert!(r.err.contains("PLANARCONFIG_SEPARATE unsupported"));
    // reproduce: the separate planes end the program on the allocation of 2^64 - 3 bytes, with
    // the error code -3 (status 253)
    let r = read(UpstreamDefects::Reproduce, d.path(), "s.tif#0", &[]);
    assert_eq!(r.exit, Some(-3), "{}", r.err);
    assert!(r.err.contains("ImageAlloc: could not allocate 3x4 buffer"));
    // FS_TIFF_DEBUG's lines
    let r = read(fix, d.path(), "f.tif", &[("FS_TIFF_DEBUG", "1")]);
    assert!(
        r.out
            .contains("[DEBUG] TiffReadImage(): pages found=1, pages to read=1 [0, 1)")
    );
    assert!(r.vol.is_some());
}

/// FS-870 and FS-865: tiled grey and 4-sample pages (40 x 20 pixels in tiles of 16): the default
/// places each tile's pixels by the image's pixel size (the fourth sample dropped); FreeSurfer
/// (`reproduce`) places them as 3-byte pixels (grey: past the image's memory, noted).
#[test]
fn tiled_grey_and_four_samples() {
    let d = dir();
    for spp in [1u32, 4] {
        let name = format!("g{spp}.tif");
        std::fs::write(
            d.path().join(&name),
            tiff_file(&[tiled_page(40, 20, 16, 16, spp, 1)]),
        )
        .unwrap();
        for mode in MODES {
            let r = read(mode, d.path(), &name, &[]);
            assert!(r.ids.contains(&"FS-870"), "{:?}", r.ids);
            if spp == 1 && !mode.fix() {
                assert!(r.ids.contains(&"FS-865"), "{:?}", r.ids);
            }
            let v = r.vol.expect(&r.err);
            let mut ok = true;
            for row in 0..20u32 {
                for x in 0..40u32 {
                    let t = (row / 16) * 3 + x / 16;
                    let k = ((row % 16) * 16 + x % 16) * spp;
                    let px = |c: u32| (t * 37 + k + c) & 255;
                    let want = if spp == 1 {
                        px(0)
                    } else {
                        px(0) | px(1) << 8 | px(2) << 16
                    };
                    ok &= vox(&v, x as i32, row as i32, 0) as u32 == want;
                }
            }
            assert_eq!(ok, mode.fix(), "{mode:?} {spp} samples: {}", r.err);
        }
    }
}
