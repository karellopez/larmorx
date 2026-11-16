// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! GCA morphs read as volumes: the paths the fs-io probe's piece 31 cannot compare (damaged
//! compressed data after the nodes, the machine's memory, the crashes, the default corrections)
//! and a small morph whose volumes are known by hand. The probe compares the rest with
//! FreeSurfer's own code (`validation/probes/fs-io`), and the recorded heads' morphs too.

use std::io::Write;
use std::path::Path;

use larmorx_core::defects::UpstreamDefects;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_gca::gcam::{self, Gcam};
use larmorx_freesurfer_mri::mri::Mri;
use larmorx_freesurfer_mri::voxel::mri_get_vox_val;

/// An `.m3z`'s bytes, as `GCAMwrite` lays them out.
struct Morph {
    d: Vec<u8>,
}

impl Morph {
    fn i(&mut self, v: i32) -> &mut Self {
        self.d.extend_from_slice(&v.to_be_bytes());
        self
    }
    fn f(&mut self, v: f32) -> &mut Self {
        self.d.extend_from_slice(&v.to_be_bytes());
        self
    }
    /// Version 1, the grid, the spacing, `exp_k` 20, then each node displaced by `disp(x, y, z)`.
    fn new(whd: [i32; 3], spacing: i32, disp: impl Fn(i32, i32, i32) -> [f32; 3]) -> Morph {
        let mut m = Morph { d: Vec::new() };
        m.f(1.0).i(whd[0]).i(whd[1]).i(whd[2]).i(spacing).f(20.0);
        for x in 0..whd[0] {
            for y in 0..whd[1] {
                for z in 0..whd[2] {
                    let o = [x as f32, y as f32, z as f32];
                    let d = disp(x, y, z);
                    m.f(o[0]).f(o[1]).f(o[2]);
                    m.f(o[0] + d[0]).f(o[1] + d[1]).f(o[2] + d[2]);
                    m.i(x).i(y).i(z);
                }
            }
        }
        m
    }
    /// The geometry tag: an image of `img` voxels and an atlas of `atl`, coronal, 1 mm.
    fn geom(&mut self, img: [i32; 3], atl: [i32; 3]) -> &mut Self {
        self.i(10);
        for whd in [img, atl] {
            self.i(1).i(whd[0]).i(whd[1]).i(whd[2]);
            for v in [
                1.0, 1.0, 1.0, -1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0,
            ] {
                self.f(v);
            }
            self.d.extend_from_slice(&[0u8; 512]);
        }
        self
    }
}

fn write(dir: &Path, name: &str, b: &[u8]) -> String {
    let p = dir.join(name);
    std::fs::File::create(&p).unwrap().write_all(b).unwrap();
    p.to_string_lossy().into_owned()
}

/// A gzip stream of `b` in stored blocks, its CRC damaged (zlib's error at the stream's end).
fn gz_damaged(b: &[u8]) -> Vec<u8> {
    let mut o = vec![0x1f, 0x8b, 8, 0, 0, 0, 0, 0, 0, 3];
    let blocks: Vec<&[u8]> = b.chunks(65535).collect();
    for (i, block) in blocks.iter().enumerate() {
        o.push(u8::from(i + 1 == blocks.len()));
        let n = block.len() as u16;
        o.extend_from_slice(&n.to_le_bytes());
        o.extend_from_slice(&(!n).to_le_bytes());
        o.extend_from_slice(block);
    }
    o.extend_from_slice(&[0xde, 0xad, 0xbe, 0xef]);
    o.extend_from_slice(&(b.len() as u32).to_le_bytes());
    o
}

type Read = (Result<(Option<Mri>, i32, i32), Fatal>, Globals, String);

fn read(mode: UpstreamDefects, env: &Environment, name: &str, frame: i32) -> Read {
    let mut g = Globals::new(mode);
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let r = gcam::read_morph(&mut g, &mut out, &mut err, env, None, name, frame, frame);
    out.extend_from_slice(&err);
    (r, g, String::from_utf8_lossy(&out).into_owned())
}

fn read_gcam(mode: UpstreamDefects, name: &str) -> (Option<Gcam>, Globals) {
    let mut g = Globals::new(mode);
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let r = gcam::gcam_read(
        &mut g,
        &mut out,
        &mut err,
        &Environment::default(),
        None,
        name,
    );
    (r.unwrap(), g)
}

fn voxels(g: &mut Globals, m: &Mri, f: i32) -> Vec<f32> {
    let mut v = Vec::new();
    let mut err = Vec::new();
    for z in 0..m.vg.depth {
        for y in 0..m.vg.height {
            for x in 0..m.vg.width {
                v.push(mri_get_vox_val(g, &mut err, m, x, y, z, f, 0).unwrap());
            }
        }
    }
    v
}

fn ids(g: &Globals) -> Vec<&'static str> {
    g.notices.as_slice().iter().map(|n| n.defect.id).collect()
}

#[test]
fn displacements_and_fields_by_hand() {
    let t = tempfile::tempdir().unwrap();
    let mut m = Morph::new([3, 2, 2], 2, |x, _, z| [0.5 * x as f32, 0.0, -(z as f32)]);
    m.geom([3, 2, 2], [6, 4, 4]);
    let name = write(t.path(), "a.m3d", &m.d);
    let env = Environment::default();
    let (r, mut g, text) = read(UpstreamDefects::Fix, &env, &name, -1);
    let (warp, s, e) = r.unwrap();
    let warp = warp.unwrap();
    assert_eq!((s, e), (-1, -1));
    // spacing 2, det 1: orig areas 8
    assert!(text.contains("setting orig areas to linear transform determinant scaled 8.00"));
    assert!(text.contains("width:3 too small for parallelism"), "{text}");
    assert_eq!(
        (warp.vg.width, warp.vg.height, warp.vg.depth, warp.nframes),
        (3, 2, 2, 3)
    );
    // the atlas's voxel sizes times the spacing
    assert_eq!(
        (warp.vg.xsize, warp.vg.ysize, warp.vg.zsize),
        (2.0, 2.0, 2.0)
    );
    let dx = voxels(&mut g, &warp, 0);
    let dz = voxels(&mut g, &warp, 2);
    // x fastest: x = 0, 1, 2 for each (y, z)
    assert_eq!(&dx[..3], &[0.0, 0.5, 1.0]);
    assert_eq!(dz[0], 0.0);
    assert_eq!(dz[6], -1.0);
    // the node coordinates' z (frame 9) through the inverse onto the image's 3 x 2 x 2 grid
    let (r, mut g, text) = read(UpstreamDefects::Fix, &env, &name, gcam::GCAM_NODEZ);
    let (v, s, e) = r.unwrap();
    assert_eq!((s, e), (0, 0));
    assert!(text.contains("reading 'frame' # 9 from gcam"));
    let v = v.unwrap();
    assert_eq!(
        (v.vg.width, v.vg.height, v.vg.depth, v.nframes),
        (3, 2, 2, 1)
    );
    let zs = voxels(&mut g, &v, 0);
    assert!(zs.iter().all(|&z| (0.0..=1.0).contains(&z)), "{zs:?}");
}

#[test]
fn damaged_compressed_morph() {
    let t = tempfile::tempdir().unwrap();
    // zlib inflates 16 KiB at a time and drops the piece in which it finds the damage: the
    // nodes (288 kB) are read whole, the labels after them are not, then the tags meet the damage
    let mut m = Morph::new([20, 20, 20], 1, |_, _, _| [0.25, 0.0, 0.0]);
    m.i(12);
    for _ in 0..8000 {
        m.i(2);
    }
    let name = write(t.path(), "a.m3z", &gz_damaged(&m.d));
    let (r, g, text) = read(UpstreamDefects::Fix, &Environment::default(), &name, -1);
    let (v, _, _) = r.unwrap();
    assert!(v.is_none(), "{text}");
    assert!(ids(&g).contains(&"FS-854"), "{:?} {text}", ids(&g));
    assert!(text.contains("the compressed data is damaged"), "{text}");
    assert!(text.contains("could not read .m3z"), "{text}");
    // reproduce: the tags end at the damage (FreeSurfer's outcome when memory ends them; observed
    // with this file), the displacements as read
    let (r, mut g, text) = read(
        UpstreamDefects::Reproduce,
        &Environment::default(),
        &name,
        -1,
    );
    let v = r.unwrap().0.expect(&text);
    assert!(ids(&g).contains(&"FS-854"));
    assert!(voxels(&mut g, &v, 0).iter().all(|&d| d == 0.25));
}

#[test]
fn nodes_past_the_machine_s_memory() {
    let t = tempfile::tempdir().unwrap();
    let mut m = Morph { d: Vec::new() };
    m.f(1.0).i(2000).i(2000).i(1000).i(1).f(20.0);
    let name = write(t.path(), "a.m3d", &m.d);
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let (r, g, text) = read(mode, &Environment::default(), &name, -1);
        assert!(matches!(r, Err(Fatal::Exit(_))), "{text}");
        assert!(ids(&g).contains(&"FS-851"), "{:?}", ids(&g));
        assert!(text.contains("GCAMalloc: could not allocate"), "{text}");
    }
}

#[test]
fn negative_sizes_end_as_freesurfer_s() {
    let t = tempfile::tempdir().unwrap();
    for (whd, msg) in [
        ([-1, 2, 2], "GCAMalloc: could not allocate nodes"),
        ([2, -1, 2], "GCAMalloc: could not allocate 0th **"),
        ([2, 2, -1], "GCAMalloc: could not allocate 0,0th *"),
    ] {
        let mut m = Morph { d: Vec::new() };
        m.f(1.0).i(whd[0]).i(whd[1]).i(whd[2]).i(1).f(20.0);
        let name = write(t.path(), "a.m3d", &m.d);
        let (r, g, text) = read(UpstreamDefects::Fix, &Environment::default(), &name, -1);
        assert!(matches!(r, Err(Fatal::Exit(_))), "{text}");
        assert!(text.contains("Cannot allocate memory"), "{text}");
        assert!(text.contains(msg), "{text}");
        assert!(!ids(&g).contains(&"FS-851"));
    }
}

#[test]
fn spacing_cube_past_32_bits() {
    let t = tempfile::tempdir().unwrap();
    let m = Morph::new([2, 2, 2], 1291, |_, _, _| [0.0; 3]);
    let name = write(t.path(), "a.m3d", &m.d);
    let (fix, g) = read_gcam(UpstreamDefects::Fix, &name);
    assert!(ids(&g).contains(&"FS-853"));
    let (rep, g) = read_gcam(UpstreamDefects::Reproduce, &name);
    assert!(ids(&g).contains(&"FS-853"));
    let a = fix.unwrap().node(0, 0, 0).orig_area;
    let b = rep.unwrap().node(0, 0, 0).orig_area;
    assert_eq!(a, (1291f64.powi(3)) as f32);
    assert_eq!(b, 1291i32.wrapping_mul(1291).wrapping_mul(1291) as f32);
}

#[test]
fn node_frames_of_an_empty_image() {
    let t = tempfile::tempdir().unwrap();
    let mut m = Morph::new([2, 2, 2], 1, |_, _, _| [0.0; 3]);
    m.geom([0, 2, 2], [2, 2, 2]);
    let name = write(t.path(), "a.m3d", &m.d);
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let (r, g, text) = read(mode, &Environment::default(), &name, gcam::GCAM_NODEX);
        assert!(matches!(r, Err(Fatal::Exit(_))), "{text}");
        assert!(ids(&g).contains(&"FS-855"), "{:?}", ids(&g));
    }
}

#[test]
fn unknown_fields_end_the_program() {
    let t = tempfile::tempdir().unwrap();
    let m = Morph::new([2, 2, 2], 1, |_, _, _| [0.0; 3]);
    let name = write(t.path(), "a.m3d", &m.d);
    for which in [11, 12, 25] {
        let (r, _, text) = read(UpstreamDefects::Fix, &Environment::default(), &name, which);
        assert!(matches!(r, Err(Fatal::Exit(_))), "{text}");
        assert!(
            text.contains(&format!("GCAMwriteMRI({which}): unknown which parameter")),
            "{text}"
        );
    }
}
