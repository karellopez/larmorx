// SPDX-License-Identifier: LicenseRef-FSL
// Copyright 2026 Karel Lopez Vilaret.
//! The FAST replica's command line on small synthetic images: its messages and exit statuses,
//! and a three-tissue segmentation with fMRIPrep's options (the parity suite
//! `fsl-fast-replica` compares it with FSL itself).

use larmorx_core::defects::UpstreamDefects;
use larmorx_nc_core::settings::Settings;
use larmorx_nc_core::volume::{self, SaveOptions, Volume};

fn settings() -> Settings {
    Settings::new()
        .with("FSLOUTPUTTYPE", "NIFTI")
        .with("FSLDIR", "/nonexistent-fsl")
}

fn run(args: &[&str], dir: &std::path::Path, s: &Settings) -> (u8, String, String) {
    let args: Vec<String> = args
        .iter()
        .map(|a| match a.strip_prefix('@') {
            Some(name) => dir.join(name).to_string_lossy().into_owned(),
            None => (*a).to_owned(),
        })
        .collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = larmorx_nc_core::exception::catch(|| {
        larmorx_nc_fast::fast(&args, s, UpstreamDefects::Reproduce, 2, &mut out, &mut err)
    })
    .unwrap_or_else(|e| panic!("{}: {}", e.type_name, e.what));
    (
        code,
        String::from_utf8(out).unwrap(),
        String::from_utf8(err).unwrap(),
    )
}

/// A 16 x 16 x 12 brain-like image: a dark ring (CSF), a middle shell (grey matter) and a
/// bright core (white matter), with seeded noise, zero outside.
fn phantom(dir: &std::path::Path) {
    phantom_as(dir, "t1", [300.0, 200.0, 80.0], 12345);
}

/// The phantom with the core's, shell's and ring's intensities `levels`, saved as `name`.
fn phantom_as(dir: &std::path::Path, name: &str, levels: [f32; 3], seed: u64) {
    let (nx, ny, nz) = (16i64, 16i64, 12i64);
    let mut v: Volume = Volume::new(nx, ny, nz, 1);
    v.setdims(1.0, 1.0, 1.5);
    let mut state = seed;
    for z in 0..nz {
        for y in 0..ny {
            for x in 0..nx {
                state = state
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                let noise = ((state >> 40) % 2001) as f32 / 100.0 - 10.0;
                let (dx, dy, dz) = (x as f32 - 7.5, y as f32 - 7.5, (z as f32 - 5.5) * 1.5);
                let r2 = dx * dx + dy * dy + dz * dz;
                let base = if r2 < 9.0 {
                    levels[0]
                } else if r2 < 25.0 {
                    levels[1]
                } else if r2 < 49.0 {
                    levels[2]
                } else {
                    0.0
                };
                let val = if base > 0.0 { base + noise } else { 0.0 };
                v.set(x, y, z, val);
            }
        }
    }
    let opts = SaveOptions::new(volume::BUILDSTRING.as_bytes());
    v.save_volume(&dir.join(name).to_string_lossy(), &settings(), &opts)
        .unwrap();
}

#[test]
fn usage_and_errors() {
    let dir = tempfile::tempdir().unwrap();
    let s = settings();
    let (code, out, err) = run(&[], dir.path(), &s);
    assert_eq!((code, out.as_str()), (1, ""));
    assert!(
        err.starts_with(
            "\nPart of FSL (ID: 2412.6-dirty)\nFAST \nCopyright(c) 2004-2012, University of \
             Oxford\n\nUsage: \nfast [options] file(s)\n"
        ),
        "{err}"
    );
    assert!(err.contains("\t-n,--class\tnumber of tissue-type classes; default=3\n"));
    assert!(err.contains("\t-A <prior1> <prior2> <prior3>    alternative prior images\n"));
    assert!(err.contains("\t-p\t\toutputs individual probability maps\n"));
    let (code, _, err) = run(&["-q", "x"], dir.path(), &s);
    assert_eq!(code, 1);
    assert!(err.ends_with("\n-q: Option doesn't exist!\n"), "{err}");
    let (code, _, err) = run(&["-H", "-1", "x"], dir.path(), &s);
    assert_eq!(code, 1);
    assert_eq!(
        err,
        "ERROR: Segmentation smoothness must be positive. Exiting\n"
    );
    let (code, _, err) = run(&["-S", "0", "x"], dir.path(), &s);
    assert_eq!(code, 1);
    assert_eq!(
        err,
        "At least 2 channels required for Multi Channel Segmentation"
    );
}

#[test]
fn fmriprep_options_on_a_phantom() {
    let dir = tempfile::tempdir().unwrap();
    phantom(dir.path());
    let s = settings();
    let (code, out, err) = run(
        &["-I", "0", "-N", "-p", "-g", "-S", "1", "@t1.nii"],
        dir.path(),
        &s,
    );
    assert_eq!((code, out.as_str(), err.as_str()), (0, "", ""));
    for f in [
        "seg",
        "seg_0",
        "seg_1",
        "seg_2",
        "prob_0",
        "prob_1",
        "prob_2",
        "pve_0",
        "pve_1",
        "pve_2",
        "pveseg",
        "mixeltype",
    ] {
        assert!(dir.path().join(format!("t1_{f}.nii")).is_file(), "{f}");
    }
    // the classes are ordered by intensity: the bright core is white matter (3), the ring CSF
    let seg: Volume<i32> =
        volume::read_volume::<i32>(&dir.path().join("t1_pveseg").to_string_lossy(), &s)
            .unwrap()
            .0;
    assert_eq!(seg.value(8, 8, 6), 3);
    assert_eq!(seg.value(8, 13, 6), 1);
    assert_eq!(seg.value(0, 0, 0), 0);
    // the partial volumes of a voxel add up to 1
    let pv: Vec<Volume> = (0..3)
        .map(|i| {
            volume::read_volume::<f32>(
                &dir.path().join(format!("t1_pve_{i}")).to_string_lossy(),
                &s,
            )
            .unwrap()
            .0
        })
        .collect();
    for k in 0..pv[0].data.len() {
        let t: f32 = pv.iter().map(|v| v.data[k]).sum();
        assert!(t == 0.0 || (t - 1.0).abs() < 1e-5, "{k}: {t}");
    }
}

/// A 20 x 12 x 10 image of three slabs along x (dark, middle, bright by `levels`: the first
/// six columns, the next six, the last six), with seeded noise and a zero border of one voxel.
fn slabs_as(dir: &std::path::Path, name: &str, levels: [f32; 3], seed: u64) {
    let (nx, ny, nz) = (20i64, 12i64, 10i64);
    let mut v: Volume = Volume::new(nx, ny, nz, 1);
    v.setdims(1.0, 1.0, 1.2);
    let mut state = seed;
    for z in 0..nz {
        for y in 0..ny {
            for x in 0..nx {
                state = state
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                let noise = ((state >> 40) % 2001) as f32 / 100.0 - 10.0;
                let border =
                    x == 0 || y == 0 || z == 0 || x == nx - 1 || y == ny - 1 || z == nz - 1;
                let val = if border {
                    0.0
                } else {
                    levels[((x - 1) / 6).min(2) as usize] + noise
                };
                v.set(x, y, z, val);
            }
        }
    }
    let opts = SaveOptions::new(volume::BUILDSTRING.as_bytes());
    v.save_volume(&dir.join(name).to_string_lossy(), &settings(), &opts)
        .unwrap();
}

#[test]
fn two_channels_on_slabs() {
    let dir = tempfile::tempdir().unwrap();
    // two contrasts with the same order of the slabs (FAST starts each channel's class means
    // from its own percentiles: with contrasts in opposite orders, such as T1 and T2 here, a
    // class can start empty and FSL's statistics turn NaN)
    slabs_as(dir.path(), "t1", [80.0, 200.0, 300.0], 12345);
    slabs_as(dir.path(), "t2", [110.0, 190.0, 330.0], 777);
    let s = settings();
    let (code, out, err) = run(
        &["-S", "2", "-B", "-b", "-o", "@seg", "@t2.nii", "@t1.nii"],
        dir.path(),
        &s,
    );
    assert_eq!((code, out.as_str(), err.as_str()), (0, "", ""));
    for f in [
        "seg",
        "pve_0",
        "pve_1",
        "pve_2",
        "pveseg",
        "mixeltype",
        "restore_1",
        "restore_2",
        "bias_1",
        "bias_2",
    ] {
        assert!(dir.path().join(format!("seg_{f}.nii")).is_file(), "{f}");
    }
    // channel 1 is the T1-like image (given last), which orders the classes: one per slab
    let seg: Volume<i32> =
        volume::read_volume::<i32>(&dir.path().join("seg_pveseg").to_string_lossy(), &s)
            .unwrap()
            .0;
    assert_eq!(seg.value(3, 6, 5), 1);
    assert_eq!(seg.value(9, 6, 5), 2);
    assert_eq!(seg.value(15, 6, 5), 3);
    assert_eq!(seg.value(0, 0, 0), 0);
    // the bias field is finite and positive inside, 1 outside
    let b: Volume =
        volume::read_volume::<f32>(&dir.path().join("seg_bias_2").to_string_lossy(), &s)
            .unwrap()
            .0;
    assert!(b.data.iter().all(|&v| v.is_finite() && v > 0.0));
    assert_eq!(b.value(0, 0, 0), 1.0);
    // more channels than words after the program (FSL would read its own path, then memory)
    let (code, _, err) = run(&["-S", "9", "@t1.nii", "@t2.nii"], dir.path(), &s);
    assert_eq!(code, 1);
    assert!(err.contains("fewer images than channels"), "{err}");
}
