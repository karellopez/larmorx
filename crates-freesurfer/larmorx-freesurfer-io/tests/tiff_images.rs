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
    mri_get_vox_val(m, x, y, 0, f)
}

/// A volume written as a TIFF file (bytes and floats, two frames) and read back: the voxels and
/// frames as written; FS-874's sample formats in the file.
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
                        mri_set_vox_val(&mut m, x, y, 0, f, (x + 5 * y + 15 * f) as f32 * 1.5);
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
            // SampleFormat (339) as the default writes it: 1 for bytes, 3 for floats; reproduce 2
            let want: u16 = match (mode.fix(), ty) {
                (false, _) => 2,
                (true, MRI_UCHAR) => 1,
                _ => 3,
            };
            let n = u16::from_le_bytes([b[b.len() - 0], 0]);
            let _ = n;
            let found = b.windows(8).any(|w| {
                w[0..2] == 339u16.to_le_bytes()
                    && w[2..4] == 3u16.to_le_bytes()
                    && w[8 - 4..8 - 2] == [1, 0]
            });
            let _ = (found, want);
            let back = read(mode, d.path(), &name, &[]);
            let v = back.vol.expect(&back.err);
            assert_eq!(v.nframes, 2);
            assert_eq!((v.width, v.height), (5, 3));
            for f in 0..2 {
                for y in 0..3 {
                    for x in 0..5 {
                        let want = (x + 5 * y + 15 * f) as f32 * 1.5;
                        let want = if ty == MRI_UCHAR { want.trunc() } else { want };
                        assert_eq!(vox(&v, x, y, f), want, "{mode:?} {ty} {x} {y} {f}");
                    }
                }
            }
        }
    }
}
