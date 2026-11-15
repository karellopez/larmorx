// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! GCA atlases read as volumes: the paths the fs-io probe's piece 27 cannot compare (damaged
//! compressed data, the crashes, the default refusals) and a small atlas whose volumes are known
//! by hand. The probe compares the rest with FreeSurfer's own code (`validation/probes/fs-io`).

use std::io::Write;
use std::path::Path;

use larmorx_core::defects::UpstreamDefects;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::log::Fatal;
use larmorx_freesurfer_gca::likely::read_gca;
use larmorx_freesurfer_gca::read::gca_read;
use larmorx_freesurfer_mri::mri::Mri;
use larmorx_freesurfer_mri::voxel::mri_get_vox_val;

const FILE_TAG: i32 = 0xab2c;

/// An atlas file of the current layout (version 5), as `GCAwrite` writes one.
struct Atlas {
    d: Vec<u8>,
}

impl Atlas {
    fn i(&mut self, v: i32) -> &mut Self {
        self.d.extend_from_slice(&v.to_be_bytes());
        self
    }
    fn f(&mut self, v: f32) -> &mut Self {
        self.d.extend_from_slice(&v.to_be_bytes());
        self
    }
    /// The header: version 5, the spacings, the stored grids, inputs, flags (no neighbours).
    fn new(ps: f32, ns: f32, pw: [i32; 3], nw: [i32; 3], ninputs: i32) -> Atlas {
        let mut a = Atlas { d: Vec::new() };
        a.f(5.0).f(ps).f(ns);
        for v in pw.into_iter().chain(nw) {
            a.i(v);
        }
        a.i(ninputs).i(1);
        a
    }
    /// A node of one class (label, its means; 1 input: one covariance).
    fn node(&mut self, label: i32, mean: f32) -> &mut Self {
        self.i(1).i(10).i(label).f(mean).f(4.0)
    }
    fn empty_node(&mut self) -> &mut Self {
        self.i(0).i(0)
    }
    fn prior(&mut self, items: &[(i32, f32)]) -> &mut Self {
        self.i(items.len() as i32).i(10);
        for &(l, p) in items {
            self.i(l).f(p);
        }
        self
    }
    /// The tagged section's direction cosines (coronal), centre 0, the sizes, voxel sizes 1.
    fn dircos(&mut self, whd: [i32; 3]) -> &mut Self {
        self.i(3);
        for v in [-1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0] {
            self.f(v);
        }
        for v in whd {
            self.i(v);
        }
        self.f(1.0).f(1.0).f(1.0)
    }
}

/// A 2 x 1 x 1 node grid (spacing 2) over a 4 x 2 x 2 volume, priors every 2 voxels (2 x 1 x 1).
fn small_atlas() -> Atlas {
    let mut a = Atlas::new(2.0, 2.0, [2, 1, 1], [2, 1, 1], 1);
    a.node(3, 50.0).node(7, 90.0);
    a.prior(&[(3, 0.75), (7, 0.25)])
        .prior(&[(3, 0.2), (7, 0.8)]);
    a.i(FILE_TAG).dircos([4, 2, 2]);
    a
}

fn write(dir: &Path, name: &str, b: &[u8]) -> String {
    let p = dir.join(name);
    std::fs::File::create(&p).unwrap().write_all(b).unwrap();
    p.to_string_lossy().into_owned()
}

fn read(
    mode: UpstreamDefects,
    name: &str,
    frame: i32,
) -> (Result<Option<Mri>, Fatal>, Globals, String) {
    let mut g = Globals::new(mode);
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let r = read_gca(&mut g, &mut out, &mut err, None, name, frame, frame);
    out.extend_from_slice(&err);
    (r, g, String::from_utf8_lossy(&out).into_owned())
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
fn small_atlas_as_volumes() {
    let t = tempfile::tempdir().unwrap();
    let name = write(t.path(), "a.gca", &small_atlas().d);
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        // frame 1: the prior grid's most likely labels
        let (r, mut g, text) = read(mode, &name, 1);
        let m = r.unwrap().unwrap();
        assert!(text.starts_with("reading frame 1 of gca\n"), "{text}");
        assert_eq!(
            (m.vg.width, m.vg.height, m.vg.depth, m.nframes),
            (2, 1, 1, 1)
        );
        assert_eq!(voxels(&mut g, &m, 0), [3.0, 7.0]);
        // frame 2: their priors
        let (r, mut g, _) = read(mode, &name, 2);
        let m = r.unwrap().unwrap();
        assert_eq!(voxels(&mut g, &m, 0), [0.75, 0.8]);
        // frame 0: the node grid's means of the most likely labels
        let (r, mut g, _) = read(mode, &name, 0);
        let m = r.unwrap().unwrap();
        assert_eq!(voxels(&mut g, &m, 0), [50.0, 90.0]);
        // every other frame: the atlas's own 4 x 2 x 2 volume, three frames
        let (r, mut g, _) = read(mode, &name, -1);
        let m = r.unwrap().unwrap();
        assert_eq!(
            (m.vg.width, m.vg.height, m.vg.depth, m.nframes),
            (4, 2, 2, 3)
        );
        let means = voxels(&mut g, &m, 0);
        let labels = voxels(&mut g, &m, 1);
        let pvals = voxels(&mut g, &m, 2);
        assert!(means.iter().all(|&v| v == 50.0 || v == 90.0 || v == 0.0));
        // the means and labels from the same prior point; the priors resampled (nearest
        // neighbour, its own rounding) from the prior grid
        for ((&mu, &l), &p) in means.iter().zip(&labels).zip(&pvals) {
            match l as i32 {
                3 => assert!(mu == 50.0),
                7 => assert!(mu == 90.0),
                _ => assert!(mu == 0.0, "{l} {mu} {p}"),
            }
            assert!([0.0, 0.75, 0.8].contains(&p), "{p}");
        }
        assert!(g.notices.as_slice().is_empty(), "{:?}", ids(&g));
    }
}

#[test]
fn cut_file_refused_by_default() {
    let t = tempfile::tempdir().unwrap();
    let mut b = small_atlas().d;
    b.truncate(60);
    let name = write(t.path(), "a.gca", &b);
    let (r, g, text) = read(UpstreamDefects::Fix, &name, 1);
    assert!(r.unwrap().is_none());
    assert!(
        text.contains("the file ends inside the atlas's nodes"),
        "{text}"
    );
    assert!(ids(&g).contains(&"FS-750"));
    let (r, g, _) = read(UpstreamDefects::Reproduce, &name, 1);
    assert!(r.unwrap().is_some());
    assert!(ids(&g).contains(&"FS-750"));
}

#[test]
fn sixteen_bit_counts() {
    let t = tempfile::tempdir().unwrap();
    let mut a = Atlas::new(2.0, 2.0, [1, 1, 1], [1, 1, 1], 1);
    a.empty_node();
    // a prior label count of 65,537 is 1 in 16 bits
    a.i(65_537).i(10).i(5).f(1.0);
    let name = write(t.path(), "a.gca", &a.d);
    let (r, g, text) = read(UpstreamDefects::Fix, &name, 1);
    assert!(r.unwrap().is_none());
    assert!(
        text.contains("a prior label count of 65537 does not fit 16 bits"),
        "{text}"
    );
    assert!(ids(&g).contains(&"FS-751"));
    let (r, mut g, _) = read(UpstreamDefects::Reproduce, &name, 1);
    let m = r.unwrap().unwrap();
    assert_eq!(voxels(&mut g, &m, 0), [5.0]);
}

#[test]
fn stored_grid_against_the_spacings() {
    // trained on 10 voxels with nodes every 4 and priors every 2: 3 nodes, 5 priors; read back,
    // the spacings give a width of 12 and so 6 priors (FS-749)
    let t = tempfile::tempdir().unwrap();
    let mut a = Atlas::new(2.0, 4.0, [5, 1, 1], [3, 1, 1], 1);
    a.node(1, 10.0).node(2, 20.0).node(3, 30.0);
    for l in [1, 1, 2, 2, 3] {
        a.prior(&[(l, 1.0)]);
    }
    a.i(FILE_TAG).dircos([10, 4, 4]);
    let name = write(t.path(), "a.gca", &a.d);
    let (r, mut g, _) = read(UpstreamDefects::Fix, &name, 1);
    let m = r.unwrap().unwrap();
    assert_eq!(m.vg.width, 5);
    assert_eq!(voxels(&mut g, &m, 0), [1.0, 1.0, 2.0, 2.0, 3.0]);
    assert!(ids(&g).contains(&"FS-749"));
    // reproduce: the spacings give a 6 x 2 x 2 prior grid (12 x 4 x 4 voxels); its sixth prior
    // (1,0,1) has for its label count the tag that begins the tagged section
    // (43,820, negative in 16 bits): FreeSurfer's allocation error
    let (r, g, text) = read(UpstreamDefects::Reproduce, &name, 1);
    assert!(matches!(r, Err(Fatal::Exit(-2))), "{text}");
    assert!(
        text.contains("could not allocate -21716 labels @ (1,0,1)"),
        "{text}"
    );
    assert!(ids(&g).contains(&"FS-749"));
}

fn many_inputs(n: i32, then_dircos: bool) -> Atlas {
    let mut a = Atlas::new(2.0, 2.0, [1, 1, 1], [1, 1, 1], n);
    a.empty_node().prior(&[(4, 1.0)]);
    a.i(FILE_TAG).i(1).i(3);
    for k in 0..n {
        a.f(20.0).f(0.5).f(k as f32);
    }
    if then_dircos {
        a.dircos([2, 2, 2]);
    }
    a
}

#[test]
fn parameters_of_more_than_100_inputs() {
    let t = tempfile::tempdir().unwrap();
    let name = write(t.path(), "a.gca", &many_inputs(105, false).d);
    let mut g = Globals::new(UpstreamDefects::Fix);
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let gca = gca_read(&mut g, &mut out, &mut err, None, &name)
        .unwrap()
        .unwrap();
    assert_eq!(gca.tes.len(), 105);
    assert_eq!(gca.tes[104], 104.0);
    assert_eq!(gca.x_r, -1.0);
    assert!(ids(&g).contains(&"FS-752"));
    assert!(
        String::from_utf8_lossy(&out)
            .contains("input 104: TR=20.0 msec, FA=28.6 deg, TE=104.0 msec")
    );
    // reproduce: the TEs of inputs 100 to 104 over the direction cosines and the centre; here
    // the direction cosines become dependent and FreeSurfer crashes (FS-754)
    let mut g = Globals::new(UpstreamDefects::Reproduce);
    let r = gca_read(&mut g, &mut out, &mut err, None, &name);
    assert!(matches!(r, Err(Fatal::Exit(139))));
    assert!(ids(&g).contains(&"FS-754"));
    // the direction cosines given after the parameters undo that; the TRs of inputs 100 and up
    // stay over the FAs of 0 and up, their FAs over the TEs
    let name = write(t.path(), "b.gca", &many_inputs(105, true).d);
    let mut g = Globals::new(UpstreamDefects::Reproduce);
    let gca = gca_read(&mut g, &mut out, &mut err, None, &name)
        .unwrap()
        .unwrap();
    assert_eq!((gca.x_r, gca.width), (-1.0, 2));
    assert_eq!(gca.fas[0], 20.0);
    assert_eq!(gca.tes[4], f64::from(0.5f32));
    // beyond 109 inputs FreeSurfer crashes (FS-759): reproduce ends as it does
    let name = write(t.path(), "c.gca", &many_inputs(110, false).d);
    let mut g = Globals::new(UpstreamDefects::Reproduce);
    let r = gca_read(&mut g, &mut out, &mut err, None, &name);
    assert!(matches!(r, Err(Fatal::Exit(139))));
    assert!(ids(&g).contains(&"FS-759"));
    let mut g = Globals::new(UpstreamDefects::Fix);
    assert!(
        gca_read(&mut g, &mut out, &mut err, None, &name)
            .unwrap()
            .is_some()
    );
}

/// A gzip stream of `b` stored, its CRC damaged (zlib's error at the end of the stream).
fn gz_damaged(b: &[u8]) -> Vec<u8> {
    let mut o = vec![0x1f, 0x8b, 8, 0, 0, 0, 0, 0, 0, 3, 1];
    let n = b.len() as u16;
    o.extend_from_slice(&n.to_le_bytes());
    o.extend_from_slice(&(!n).to_le_bytes());
    o.extend_from_slice(b);
    o.extend_from_slice(&[0xde, 0xad, 0xbe, 0xef]);
    o.extend_from_slice(&(b.len() as u32).to_le_bytes());
    o
}

#[test]
fn damaged_compressed_atlas() {
    let t = tempfile::tempdir().unwrap();
    let name = write(t.path(), "a.gcz", &gz_damaged(&small_atlas().d));
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let (r, g, text) = read(mode, &name, 1);
        assert!(r.unwrap().is_none(), "{text}");
        assert!(ids(&g).contains(&"FS-758"), "{:?} {text}", ids(&g));
    }
}

#[test]
fn singular_prior_grid() {
    let t = tempfile::tempdir().unwrap();
    let mut a = Atlas::new(2.0, 2.0, [1, 1, 1], [1, 1, 1], 1);
    a.empty_node().prior(&[(4, 1.0)]);
    a.i(FILE_TAG).i(3);
    for _ in 0..12 {
        a.f(0.0);
    }
    a.i(2).i(2).i(2).f(1.0).f(1.0).f(1.0);
    let name = write(t.path(), "a.gca", &a.d);
    let (r, g, text) = read(UpstreamDefects::Fix, &name, 1);
    assert!(matches!(r, Err(Fatal::Exit(-4))), "{text}");
    assert!(
        text.contains("voxel-to-RAS matrix with no inverse"),
        "{text}"
    );
    assert!(ids(&g).contains(&"FS-754"));
    let (r, _, _) = read(UpstreamDefects::Reproduce, &name, 1);
    assert!(matches!(r, Err(Fatal::Exit(139))));
}

#[test]
fn negative_neighbour_count() {
    let t = tempfile::tempdir().unwrap();
    let mut a = Atlas::new(2.0, 2.0, [1, 1, 1], [1, 1, 1], 1);
    // flags 0: neighbour lists
    let n = a.d.len();
    a.d[n - 4..].copy_from_slice(&0i32.to_be_bytes());
    a.i(1).i(10).i(3).f(1.0).f(1.0).i(-2);
    let name = write(t.path(), "a.gca", &a.d);
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let (r, g, text) = read(mode, &name, 1);
        assert!(matches!(r, Err(Fatal::Exit(-2))));
        assert!(text.contains("couldn't expand gcs to -2"), "{text}");
        assert!(text.contains("Cannot allocate memory"), "{text}");
        assert!(ids(&g).contains(&"FS-755"));
    }
}

#[test]
fn missing_folder_and_version() {
    let t = tempfile::tempdir().unwrap();
    let missing = t.path().join("none.gca").to_string_lossy().into_owned();
    let (r, _, text) = read(UpstreamDefects::Fix, &missing, 1);
    assert!(r.unwrap().is_none());
    assert!(text.contains("No such file or directory"), "{text}");
    assert!(text.contains("could not open file"), "{text}");
    let folder = t.path().join("d.gca");
    std::fs::create_dir(&folder).unwrap();
    let (r, _, text) = read(UpstreamDefects::Fix, &folder.to_string_lossy(), 1);
    assert!(r.unwrap().is_none());
    assert!(
        text.contains("Is a directory") && text.contains("could not read file"),
        "{text}"
    );
    let mut a = Atlas { d: Vec::new() };
    a.f(f32::NAN).i(0);
    let name = write(t.path(), "v.gca", &a.d);
    let (r, _, text) = read(UpstreamDefects::Fix, &name, 1);
    assert!(r.unwrap().is_none());
    assert!(text.contains("version #nan found, 5.0 expected"), "{text}");
}
