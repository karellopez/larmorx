// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! Stacked autoencoders read as volumes (track A's relay 41): small files known by hand, the
//! one-voxel check's lines as FreeSurfer's own code prints them (the fs-io probe's `fsio`, case
//! `aed` of `a.sae` with `FREESURFER_SEED=4711`), and the defaults of FS-877, FS-921 and FS-927
//! where FreeSurfer's reading goes on with memory contents or reads past its voxels. The probe's
//! piece 33 compares the rest with FreeSurfer's code (`validation/probes/fs-io`).

use std::path::Path;

use larmorx_core::defects::UpstreamDefects;
use larmorx_freesurfer_core::globals::Globals;
use larmorx_freesurfer_core::run::Environment;
use larmorx_freesurfer_io::mriio::mri_read;
use larmorx_freesurfer_mri::mri::Mri;
use larmorx_freesurfer_mri::voxel::mri_get_vox_val;

fn ids(g: &Globals) -> Vec<&'static str> {
    g.notices.as_slice().iter().map(|n| n.defect.id).collect()
}

fn mat(rows: i32, cols: i32, vals: &[f64]) -> Vec<u8> {
    let mut b = Vec::new();
    for v in [1, rows, cols] {
        b.extend_from_slice(&v.to_be_bytes());
    }
    for v in vals {
        b.extend_from_slice(&v.to_bits().to_be_bytes());
    }
    b
}

fn e15(x: f64) -> String {
    let s = format!("{x:.15e}");
    let (m, e) = s.split_once('e').unwrap();
    let e: i32 = e.parse().unwrap();
    format!("{m}e{}{:02}", if e < 0 { '-' } else { '+' }, e.abs())
}

fn vol_geom(w: i32, h: i32, d: i32) -> Vec<u8> {
    let (o, m) = (e15(1.0), e15(-1.0));
    let z = e15(0.0);
    format!(
        "valid = 1  # volume info valid\nfilename = norm.mgz\nvolume = {w} {h} {d}\nvoxelsize = {o} {o} {o}\nxras   = {m} {z} {z}\nyras   = {z} {z} {m}\nzras   = {z} {o} {z}\ncras   = {z} {z} {z}\n"
    )
    .into_bytes()
}

/// A layer as `SAEwrite` lays it out: the input-to-hidden weights `wih`, output bias 0.25,
/// hidden bias 0, hidden-to-output weights 1, input, hidden and output vectors 0, the kernel.
fn layer(nin: i32, nh: i32, nout: i32, wih: &[f64], ksize: i32) -> Vec<u8> {
    let mut b = mat(nh, nin, wih);
    b.extend(mat(nout, 1, &vec![0.25; nout as usize]));
    b.extend(mat(nh, 1, &vec![0.0; nh as usize]));
    b.extend(mat(nout, nh, &vec![1.0; (nout * nh) as usize]));
    b.extend(mat(nin, 1, &vec![0.0; nin as usize]));
    b.extend(mat(nh, 1, &vec![0.0; nh as usize]));
    b.extend(mat(nout, 1, &vec![0.0; nout as usize]));
    b.extend_from_slice(&ksize.to_be_bytes());
    b
}

/// The layer-0 weights of file B: hidden unit `h`, input `i` (from 0) is `(9 h + i) / 8`.
fn w0() -> Vec<f64> {
    (0..18).map(|k| k as f64 / 8.0).collect()
}

/// File B: a 2-D window of 3 x 3 (one level), two hidden units; a second layer of kernel 2 with one
/// hidden unit, weights `(v + 1) / 4`; its stored extent `ext`.
fn file_b(ext: (i32, i32, i32)) -> Vec<u8> {
    let mut b = b"1 2 1.000000 9 1\n".to_vec();
    b.extend(vol_geom(ext.0, ext.1, ext.2));
    b.extend(layer(9, 2, 9, &w0(), 1));
    let w1: Vec<f64> = (0..8).map(|v| (v + 1) as f64 / 4.0).collect();
    b.extend(layer(8, 1, 8, &w1, 2));
    b
}

fn read(
    mode: UpstreamDefects,
    dir: &Path,
    name: &str,
    env: &Environment,
) -> (Globals, Option<Mri>, String) {
    larmorx_freesurfer_gca::register_io();
    // names with a folder: mri_read puts the working folder before a bare name
    let name = format!("w/{name}");
    let mut g = Globals::new(mode);
    g.comm = "fsio".into();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let m = mri_read(
        &mut g,
        &mut out,
        &mut err,
        env,
        Some(dir),
        &name,
        -1,
        true,
        -1,
        -1,
    )
    .unwrap();
    out.extend(err);
    (g, m, String::from_utf8_lossy(&out).into_owned())
}

fn write(dir: &Path, name: &str, b: Vec<u8>) -> std::io::Result<()> {
    std::fs::create_dir_all(dir.join("w"))?;
    std::fs::write(dir.join("w").join(name), b)
}

fn vox(g: &mut Globals, m: &Mri, x: i32, y: i32, f: i32) -> f32 {
    mri_get_vox_val(g, &mut Vec::new(), m, x, y, 0, f, 0).unwrap()
}

#[test]
fn layer_zero_is_the_input_weights() {
    let t = tempfile::tempdir().unwrap();
    write(t.path(), "b.sae", file_b((3, 3, 1))).unwrap();
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let (mut g, m, text) = read(mode, t.path(), "b.sae", &Environment::default());
        let m = m.unwrap();
        assert_eq!((m.width, m.height, m.depth, m.nframes), (3, 3, 1, 2));
        assert!(
            text.contains("allocating SAE with inputs/hidden 9/9\n"),
            "{text}"
        );
        assert!(
            text.contains("layer 2 read with 1 hidden units\n"),
            "{text}"
        );
        // voxel (x, y) of hidden unit h: input x 3 + y of that unit
        for h in 0..2 {
            for x in 0..3 {
                for y in 0..3 {
                    let want = (9 * h + 3 * x + y) as f32 / 8.0;
                    assert_eq!(vox(&mut g, &m, x, y, h), want);
                }
            }
        }
    }
}

#[test]
fn a_later_layer_through_the_one_before() {
    let t = tempfile::tempdir().unwrap();
    write(t.path(), "b.sae", file_b((3, 3, 1))).unwrap();
    // the kernel: 2 + 2 x 1 voxels a side; each voxel the mean of the products of the layer-0
    // voxels that land on it with the weight of their place (v = 4 p + 2 x + y)
    let mut want = [[0f32; 4]; 4];
    let mut count = [[0i32; 4]; 4];
    for p in 0..2 {
        for x in 0..2 {
            for y in 0..2 {
                let w = (4 * p + 2 * x + y + 1) as f32 / 4.0;
                for xp in 0..3 {
                    for yp in 0..3 {
                        let prev = (9 * p + 3 * xp + yp) as f32 / 8.0;
                        want[(x + xp) as usize][(y + yp) as usize] += prev * w;
                        count[(x + xp) as usize][(y + yp) as usize] += 1;
                    }
                }
            }
        }
    }
    // by default the kernel's extent; FreeSurfer's header takes the stored 3 x 3 x 1 (FS-921)
    let (mut g, m, _) = read(
        UpstreamDefects::Fix,
        t.path(),
        "b.sae#1",
        &Environment::default(),
    );
    let m = m.unwrap();
    assert_eq!((m.width, m.height, m.depth, m.nframes), (4, 4, 1, 1));
    assert!(ids(&g).contains(&"FS-921"));
    for x in 0..4 {
        for y in 0..4 {
            let w = want[x][y] / count[x][y] as f32;
            assert_eq!(vox(&mut g, &m, x as i32, y as i32, 0), w, "({x}, {y})");
        }
    }
    let (g, m, _) = read(
        UpstreamDefects::Reproduce,
        t.path(),
        "b.sae#1",
        &Environment::default(),
    );
    let m = m.unwrap();
    assert_eq!((m.width, m.height, m.depth), (3, 3, 1));
    assert!(ids(&g).contains(&"FS-921"), "{:?}", g.notices.as_slice());
    // a stored extent larger than the kernel: FreeSurfer reads past the voxels (FS-927)
    write(t.path(), "c.sae", file_b((256, 256, 256))).unwrap();
    let (g, m, text) = read(
        UpstreamDefects::Reproduce,
        t.path(),
        "c.sae#1",
        &Environment::default(),
    );
    assert!(m.is_none());
    assert!(ids(&g).contains(&"FS-927"));
    assert!(text.contains("MRIread(") || text.contains("larger than the layer's kernel"));
    let (_, m, _) = read(
        UpstreamDefects::Fix,
        t.path(),
        "c.sae#1",
        &Environment::default(),
    );
    assert_eq!(m.map(|m| (m.width, m.height, m.depth)), Some((4, 4, 1)));
}

#[test]
fn the_one_voxel_check_prints_freesurfers_lines() {
    let t = tempfile::tempdir().unwrap();
    let mut b = b"0 1 1.000000 1 1\n".to_vec();
    b.extend(vol_geom(1, 1, 1));
    b.extend(layer(1, 1, 1, &[0.5], 1));
    write(t.path(), "a.sae", b).unwrap();
    let env = Environment::from_pairs(vec![("FREESURFER_SEED", "4711")]);
    for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
        let (mut g, m, text) = read(mode, t.path(), "a.sae", &env);
        // fsio's output for this file (FreeSurfer 8.2.0's code, the unit oracle's build)
        assert_eq!(
            text,
            "allocating SAE with inputs/hidden 1/1\n\
             Setting seed for random number genererator to 4711\n\
             f(I=0.00 * wij=-0.04 + bj=1.00 = 1.00) = 0.73 * wjk=0.06 + bk=1.00 = 1.04, E=1.04\n\
             f(I=1.00 * wij=-0.04 + bj=1.00 = 0.96) = 0.72 * wjk=0.06 + bk=1.00 = 1.04, E=0.04\n"
        );
        let m = m.unwrap();
        assert_eq!((m.width, m.height, m.depth, m.nframes), (1, 1, 1, 1));
        assert_eq!(vox(&mut g, &m, 0, 0, 0), 0.5);
    }
}

#[test]
fn a_header_short_of_its_numbers() {
    let t = tempfile::tempdir().unwrap();
    let mut b = b"1 1\n".to_vec();
    b.extend(vol_geom(3, 3, 1));
    b.extend(layer(9, 1, 9, &[0.5; 9], 1));
    write(t.path(), "d.sae", b).unwrap();
    // by default a clear error (FS-877: FreeSurfer goes on with memory contents)
    let (g, m, text) = read(
        UpstreamDefects::Fix,
        t.path(),
        "d.sae",
        &Environment::default(),
    );
    assert!(m.is_none());
    assert!(ids(&g).contains(&"FS-877"));
    assert!(text.starts_with(
        "File(w/d.sae) Warning: expected number of values to read does not match those read\n"
    ));
    assert!(text.contains("SAEread(w/d.sae): the header line does not hold its 5 numbers"));
    assert!(text.contains("MRIread(w/d.sae): could not read autoencoder"));
    // reproduce: the missing numbers 0 (scale, type, levels), the reading goes on
    let (g, _, text) = read(
        UpstreamDefects::Reproduce,
        t.path(),
        "d.sae",
        &Environment::default(),
    );
    assert!(ids(&g).contains(&"FS-877"));
    assert!(
        text.contains("allocating SAE with inputs/hidden 0/0\n"),
        "{text}"
    );
}

#[test]
fn a_missing_file_and_a_layer_past_the_count() {
    let t = tempfile::tempdir().unwrap();
    let (_, m, text) = read(
        UpstreamDefects::Fix,
        t.path(),
        "x.sae",
        &Environment::default(),
    );
    assert!(m.is_none());
    assert!(
        text.contains("SAEread(w/x.sae): could not open file"),
        "{text}"
    );
    assert!(text.contains("MRIread(w/x.sae): could not read autoencoder"));
    write(t.path(), "b.sae", file_b((3, 3, 1))).unwrap();
    larmorx_freesurfer_gca::register_io();
    let mut g = Globals::new(UpstreamDefects::Fix);
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let r = mri_read(
        &mut g,
        &mut out,
        &mut err,
        &Environment::default(),
        Some(t.path()),
        "w/b.sae#2",
        -1,
        true,
        -1,
        -1,
    );
    // the program's end with ERROR_BADPARM's status
    assert!(r.is_err());
    let err = String::from_utf8_lossy(&err).into_owned();
    assert!(
        err.contains("CSAlayerWeightsToMRI: invalid layer 2 (2 encoders)"),
        "{err}"
    );
}
