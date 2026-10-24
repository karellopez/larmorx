// SPDX-License-Identifier: LicenseRef-FSL
// Part of larmorx-nc (FSL Licence terms; see LICENSE). Copyright 2026 Karel Lopez Vilaret.
//! The `larmorx-nc fsl mcflirt` command line end to end, on a tiny generated series: mcflirt's
//! messages and exit statuses for its usage errors and failure cases, each corrected by default
//! and reproduced with `LARMORX_UPSTREAM_DEFECTS=reproduce` (D13). The texts are mcflirt's
//! (FSL 6.0.7.17, checked against the oracle by the parity suite `fsl-mcflirt-replica`).

use std::path::{Path, PathBuf};
use std::process::Command;

const EXE: &str = env!("CARGO_BIN_EXE_larmorx-nc");

/// A 10 × 8 × 6 float32 series of 3 volumes (a block shifted by one voxel), as `.nii`.
fn series(dir: &Path) -> PathBuf {
    let (nx, ny, nz, nt) = (10usize, 8usize, 6usize, 3usize);
    let mut h = vec![0u8; 352];
    h[0..4].copy_from_slice(&348i32.to_le_bytes());
    let dims: [i16; 8] = [4, nx as i16, ny as i16, nz as i16, nt as i16, 1, 1, 1];
    for (k, d) in dims.iter().enumerate() {
        h[40 + 2 * k..42 + 2 * k].copy_from_slice(&d.to_le_bytes());
    }
    h[70..72].copy_from_slice(&16i16.to_le_bytes());
    h[72..74].copy_from_slice(&32i16.to_le_bytes());
    let pix: [f32; 5] = [1.0, 4.0, 4.0, 5.0, 2.0];
    for (k, p) in pix.iter().enumerate() {
        h[76 + 4 * k..80 + 4 * k].copy_from_slice(&p.to_le_bytes());
    }
    h[108..112].copy_from_slice(&352f32.to_le_bytes());
    h[112..116].copy_from_slice(&1f32.to_le_bytes());
    h[123] = 10;
    h[254..256].copy_from_slice(&1i16.to_le_bytes());
    let srow = [
        [-4.0f32, 0.0, 0.0, 20.0],
        [0.0, 4.0, 0.0, -16.0],
        [0.0, 0.0, 5.0, -15.0],
    ];
    for (r, row) in srow.iter().enumerate() {
        for (c, v) in row.iter().enumerate() {
            let o = 280 + 16 * r + 4 * c;
            h[o..o + 4].copy_from_slice(&v.to_le_bytes());
        }
    }
    h[344..348].copy_from_slice(b"n+1\0");
    for t in 0..nt {
        for z in 0..nz {
            for y in 0..ny {
                for x in 0..nx {
                    let inside =
                        (2 + t..6 + t).contains(&x) && (2..6).contains(&y) && (1..5).contains(&z);
                    let v: f32 = if inside {
                        500.0 + (x * 7 + y * 3 + z) as f32
                    } else {
                        20.0 + ((x + y + z) % 3) as f32
                    };
                    h.extend_from_slice(&v.to_le_bytes());
                }
            }
        }
    }
    let path = dir.join("bold.nii");
    std::fs::write(&path, h).unwrap();
    path
}

struct Out {
    code: i32,
    stdout: String,
    stderr: String,
}

fn mcflirt(dir: &Path, args: &[&str], mode: &str, env: &[(&str, Option<&str>)]) -> Out {
    let mut c = Command::new(EXE);
    c.arg("fsl")
        .arg("mcflirt")
        .args(args)
        .current_dir(dir)
        .env("LARMORX_UPSTREAM_DEFECTS", mode)
        .env("FSLOUTPUTTYPE", "NIFTI")
        .env("OMP_NUM_THREADS", "1");
    for (k, v) in env {
        match v {
            Some(v) => c.env(k, v),
            None => c.env_remove(k),
        };
    }
    let o = c.output().unwrap();
    Out {
        code: o.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&o.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&o.stderr).into_owned(),
    }
}

fn named(stderr: &str) -> Vec<String> {
    stderr
        .lines()
        .filter_map(|l| l.strip_prefix("larmorx-nc: upstream defect "))
        .filter_map(|r| r.split(' ').next())
        .map(str::to_owned)
        .collect()
}

#[test]
fn usage_and_parsing_errors() {
    let d = tempfile::tempdir().unwrap();
    let dir = d.path();
    let o = mcflirt(dir, &[], "fix", &[]);
    assert_eq!(o.code, 1);
    assert!(
        o.stdout
            .starts_with("Usage: mcflirt -in <infile> [options]"),
        "{}",
        o.stdout
    );
    // `-help` alone: exit 1 in mcflirt (fewer than 3 arguments; FSL-07), 0 corrected.
    let o = mcflirt(dir, &["-help"], "reproduce", &[]);
    assert_eq!(o.code, 1);
    let o = mcflirt(dir, &["-help"], "fix", &[]);
    assert_eq!(o.code, 0);
    assert_eq!(named(&o.stderr), ["FSL-07"]);
    let bold = series(dir);
    let b = bold.to_str().unwrap();
    let o = mcflirt(dir, &["-in", b, "-out", "o", "-bogus"], "fix", &[]);
    assert_eq!(
        (o.code, o.stderr.as_str()),
        (255, "Lacking argument to option -bogus\n")
    );
    let o = mcflirt(dir, &["-in", b, "-bogus", "x", "-out", "o"], "fix", &[]);
    assert_eq!(
        (o.code, o.stderr.as_str()),
        (255, "Unrecognised option -bogus\n")
    );
    // No -in: the message, the usage on standard output, exit 2.
    let o = mcflirt(dir, &["-out", "o", "-mats"], "fix", &[]);
    assert_eq!(o.code, 2);
    assert_eq!(o.stderr, "Input filename not found\n\n");
    assert!(o.stdout.starts_with("Usage: mcflirt"), "{}", o.stdout);
    // An -in that names no file: newimage's exceptions, then mcflirt's abort.
    let o = mcflirt(dir, &["-in", "nosuch", "-out", "o"], "fix", &[]);
    assert_eq!(o.code, 1);
    assert!(
        o.stderr
            .starts_with("Image Exception : #63 :: No image files match: nosuch\n"),
        "{}",
        o.stderr
    );
    // Numbers read with atoi (FSL-07): refused corrected.
    let o = mcflirt(dir, &["-in", b, "-out", "o", "-stages", "abc"], "fix", &[]);
    assert_eq!(o.code, 255);
    assert!(o.stderr.contains("-stages"), "{}", o.stderr);
    // An unknown cost (FSL-06): refused corrected, normcorr reproduced.
    let o = mcflirt(dir, &["-in", b, "-out", "o", "-cost", "bogus"], "fix", &[]);
    assert_eq!(o.code, 255, "{}", o.stderr);
    let o = mcflirt(
        dir,
        &["-in", b, "-out", "o", "-cost", "bogus", "-stages", "1"],
        "reproduce",
        &[],
    );
    assert_eq!(o.code, 0, "{}", o.stderr);
    assert_eq!(named(&o.stderr), ["FSL-06"]);
    // D13's own switch.
    let o = mcflirt(dir, &["-in", b, "-out", "o"], "faithful", &[]);
    assert_eq!(o.code, 2);
    assert!(
        o.stderr.contains("LARMORX_UPSTREAM_DEFECTS"),
        "{}",
        o.stderr
    );
}

#[test]
fn failures_corrected_and_reproduced() {
    let d = tempfile::tempdir().unwrap();
    let dir = d.path();
    let bold = series(dir);
    let b = bold.to_str().unwrap();
    // An out-of-range -refvol (FSL-12): refused before any work, or mcflirt's abort.
    let o = mcflirt(dir, &["-in", b, "-out", "o", "-refvol", "7"], "fix", &[]);
    assert_eq!(o.code, 1);
    assert!(o.stderr.starts_with("Error: -refvol 7"), "{}", o.stderr);
    let o = mcflirt(
        dir,
        &["-in", b, "-out", "o", "-refvol", "7"],
        "reproduce",
        &[],
    );
    assert_eq!(o.code, 1);
    assert!(
        o.stderr
            .contains("Image Exception : #61 :: Invalid t index in [] operator"),
        "{}",
        o.stderr
    );
    assert!(
        o.stderr
            .contains("terminate called after throwing an instance of 'std::runtime_error'")
    );
    // -gdt (FSL-04): refused corrected.
    let o = mcflirt(dir, &["-in", b, "-out", "o", "-gdt"], "fix", &[]);
    assert_eq!(o.code, 1);
    assert!(o.stderr.contains("-gdt"), "{}", o.stderr);
    // The .mat directory in a missing directory (FSL-20): refused, or mcflirt's exit 0.
    let o = mcflirt(
        dir,
        &["-in", b, "-out", "missing/o", "-mats", "-stages", "1"],
        "fix",
        &[],
    );
    assert_eq!(o.code, 1, "{}", o.stderr);
    assert!(o.stderr.contains("FSL-20"), "{}", o.stderr);
    let o = mcflirt(
        dir,
        &["-in", b, "-out", "missing/o", "-mats", "-stages", "1"],
        "reproduce",
        &[],
    );
    assert_eq!(o.code, 0, "{}", o.stderr);
    assert!(
        o.stderr
            .contains("Error!: Unknown Log directory return value"),
        "{}",
        o.stderr
    );
    assert_eq!(named(&o.stderr), ["FSL-20"]);
    // FSLOUTPUTTYPE unset: .nii.gz corrected (FSL-11, larmorx's behaviour); mcflirt fails at
    // its first image after writing the matrices.
    let o = mcflirt(
        dir,
        &["-in", b, "-out", "unset", "-mats", "-stages", "1"],
        "fix",
        &[("FSLOUTPUTTYPE", None)],
    );
    assert_eq!(o.code, 0, "{}", o.stderr);
    assert!(dir.join("unset.nii.gz").is_file());
    let o = mcflirt(
        dir,
        &["-in", b, "-out", "unset2", "-mats", "-stages", "1"],
        "reproduce",
        &[("FSLOUTPUTTYPE", None)],
    );
    assert_ne!(o.code, 0, "{}", o.stderr);
    assert!(dir.join("unset2.mat/MAT_0000").is_file());
    assert!(!dir.join("unset2.nii.gz").exists());
}

#[test]
fn outputs_and_threads() {
    let d = tempfile::tempdir().unwrap();
    let dir = d.path();
    let bold = series(dir);
    let b = bold.to_str().unwrap();
    let args = [
        "-in", b, "-out", "t", "-mats", "-plots", "-rmsrel", "-rmsabs",
    ];
    let o = mcflirt(dir, &args, "fix", &[]);
    assert_eq!(o.code, 0, "{}", o.stderr);
    for f in [
        "t.nii",
        "t.mat/MAT_0000",
        "t.mat/MAT_0002",
        "t.par",
        "t_abs.rms",
        "t_abs_mean.rms",
        "t_rel.rms",
        "t_rel_mean.rms",
    ] {
        assert!(dir.join(f).is_file(), "{f}");
    }
    let one: Vec<Vec<u8>> = ["t.nii", "t.par", "t.mat/MAT_0001"]
        .iter()
        .map(|f| std::fs::read(dir.join(f)).unwrap())
        .collect();
    // The same bytes with 3 threads.
    let d3 = tempfile::tempdir().unwrap();
    let bold3 = series(d3.path());
    let args3 = [
        "-in",
        bold3.to_str().unwrap(),
        "-out",
        "t",
        "-mats",
        "-plots",
        "-rmsrel",
        "-rmsabs",
    ];
    let o = mcflirt(d3.path(), &args3, "fix", &[("OMP_NUM_THREADS", Some("3"))]);
    assert_eq!(o.code, 0, "{}", o.stderr);
    for (f, a) in ["t.nii", "t.par", "t.mat/MAT_0001"].iter().zip(&one) {
        assert_eq!(&std::fs::read(d3.path().join(f)).unwrap(), a, "{f}");
    }
    // -report: mcflirt's progress messages on standard error.
    let o = mcflirt(
        dir,
        &["-in", b, "-out", "r", "-report", "-stages", "1"],
        "fix",
        &[],
    );
    assert_eq!(o.code, 0, "{}", o.stderr);
    assert!(
        o.stderr.contains("McFLIRT v 2.0 - FMRI motion correction"),
        "{}",
        o.stderr
    );
    assert!(
        o.stderr.contains("Saving motion corrected time series... "),
        "{}",
        o.stderr
    );
}
