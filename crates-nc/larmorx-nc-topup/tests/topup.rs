// SPDX-License-Identifier: LicenseRef-FSL
// Copyright 2026 Karel Lopez Vilaret.
//! The topup replica's command line on a small synthetic pair of reversed phase-encoding
//! images: its messages and exit statuses, a configuration file, and short estimations with
//! and without movements (the parity suite `fsl-topup-replica` compares it with FSL itself).

use larmorx_core::defects::UpstreamDefects;
use larmorx_nc_core::settings::Settings;
use larmorx_nc_core::volume::{self, SaveOptions, Volume};

fn settings() -> Settings {
    Settings::new()
        .with("FSLOUTPUTTYPE", "NIFTI")
        .with("FSLDIR", "/nonexistent-fsl")
}

fn run(args: &[&str], dir: &std::path::Path) -> (u8, String, String) {
    let args: Vec<String> = args
        .iter()
        .map(|a| match a.split_once('@') {
            Some((k, name)) => format!("{k}{}", dir.join(name).to_string_lossy()),
            None => (*a).to_owned(),
        })
        .collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = larmorx_nc_topup::topup(
        &args,
        &settings(),
        UpstreamDefects::Reproduce,
        2,
        &mut out,
        &mut err,
    );
    (
        code,
        String::from_utf8(out).unwrap(),
        String::from_utf8(err).unwrap(),
    )
}

/// A blob shifted by one voxel up and down along y (16 × 16 × 8, 3 mm voxels), and the
/// acquisition parameters of the pair.
fn inputs(dir: &std::path::Path) {
    let mut v = Volume::new(16, 16, 8, 2);
    v.setdims(3.0, 3.0, 3.0);
    for t in 0..2i64 {
        let shift = if t == 0 { 1.0 } else { -1.0 };
        for k in 0..8i64 {
            for j in 0..16i64 {
                for i in 0..16i64 {
                    let (x, y, z) = (i as f32 - 7.5, j as f32 - 7.5 + shift, k as f32 - 3.5);
                    let r2 = x * x + y * y + 2.0 * z * z;
                    let val = (400.0 - 4.0 * r2).max(0.0) + 10.0;
                    let n = v.nvoxels();
                    let idx = (t * n) as usize + v.idx(i, j, k);
                    v.data[idx] = val;
                }
            }
        }
    }
    v.save_volume(
        dir.join("b0").to_str().unwrap(),
        &settings(),
        &SaveOptions::new(volume::BUILDSTRING.as_bytes()),
    )
    .unwrap();
    std::fs::write(dir.join("acq.txt"), "0 1 0 0.05\n0 -1 0 0.05\n").unwrap();
}

#[test]
fn messages_and_exit_statuses() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    inputs(d);
    let (code, _, err) = run(&[], d);
    assert_eq!(code, 1);
    assert!(
        err.contains("Part of FSL (ID: 2412.6-dirty)\ntopup\n\nUsage: \n"),
        "{err}"
    );
    assert!(
        err.contains("\t--imain\t\tname of 4D file with images\n"),
        "{err}"
    );
    assert_eq!(err.matches("\t-h,--help\tdisplay help info\n").count(), 2);
    assert!(!err.contains("--dfout"), "hidden options are not listed");
    let (code, _, err) = run(&["--imain=@b0"], d);
    assert_eq!(code, 1);
    assert!(
        err.ends_with(
            "\nTopup::topup_clp::topup_clp: Exactly one of --datain and --acqp must be set.\n"
        ),
        "{err}"
    );
    let (code, _, err) = run(&["--imain=@b0", "--datain=@acq.txt", "--numprec=half"], d);
    assert_eq!(code, 1);
    assert!(
        err.ends_with("Unrecognized value half as argument to --numprec.\n"),
        "{err}"
    );
    let (code, _, err) = run(
        &["--imain=@b0", "--datain=@acq.txt", "--config=@nothere"],
        d,
    );
    assert_eq!(code, 1);
    assert!(err.starts_with("Cannot find config-file "), "{err}");
    let (code, _, err) = run(&["--imain=@b0", "--datain=@acq.txt", "--bogus=1"], d);
    assert_eq!(code, 1);
    assert!(err.ends_with("\n--bogus: Option doesn't exist!\n"), "{err}");
    std::fs::write(d.join("short.txt"), "0 1 0 0.05\n").unwrap();
    let (code, _, err) = run(&["--imain=@b0", "--datain=@short.txt"], d);
    assert_eq!(code, 1);
    assert!(
        err.contains("\nTopup::topup_clp::topup_clp: Mismatch between "),
        "{err}"
    );
}

#[test]
fn a_short_estimation_from_a_configuration_file() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    inputs(d);
    std::fs::write(
        d.join("short.cnf"),
        "# two levels\n--warpres=12,9\n--subsamp=2,1\n--fwhm=4,0\n--miter=2,2\n\
         --lambda=0.001,0.0001\n--estmov=0\n--minmet=0,1\n--scale=1",
    )
    .unwrap();
    let (code, out, err) = run(
        &[
            "--imain=@b0",
            "--datain=@acq.txt",
            "--config=@short",
            "--out=@res",
            "--fout=@field",
            "--iout=@corr",
            "--jacout=@jac",
            "--rbmout=@xfm",
            "--dfout=@warp",
            "-v",
        ],
        d,
    );
    assert_eq!((code, err.as_str()), (0, ""), "{out}");
    assert!(
        out.contains("***Going to next resolution level***\n"),
        "{out}"
    );
    assert!(out.starts_with("SSD = "), "{out}");
    for f in [
        "res_fieldcoef.nii",
        "res_movpar.txt",
        "field.nii",
        "corr.nii",
        "corr_mean.nii",
        "jac_01.nii",
        "jac_02.nii",
        "xfm_01.mat",
        "warp_02.nii",
        "b0.topup_log",
    ] {
        assert!(d.join(f).is_file(), "{f} not written");
    }
    let log = std::fs::read_to_string(d.join("b0.topup_log")).unwrap();
    assert!(log.contains("\n--warpres=12,9\n"), "{log}");
    assert!(log.contains("\n--minmet=0,1\n"), "{log}");
    let mp = std::fs::read_to_string(d.join("res_movpar.txt")).unwrap();
    assert_eq!(mp, "0  0  0  0  0  0  \n0  0  0  0  0  0  \n");
}

#[test]
fn movements_estimated() {
    let dir = tempfile::tempdir().unwrap();
    let d = dir.path();
    inputs(d);
    let (code, out, err) = run(
        &[
            "--imain=@b0",
            "--datain=@acq.txt",
            "--out=@mv",
            "--warpres=12",
            "--subsamp=1",
            "--fwhm=4",
            "--miter=2",
            "--lambda=0.001",
            "--minmet=1",
        ],
        d,
    );
    assert_eq!((code, err.as_str()), (0, ""), "{out}");
    assert!(
        out.contains("WARNING, it is generally NOT a good idea to include movements"),
        "{out}"
    );
    let mp = std::fs::read_to_string(d.join("mv_movpar.txt")).unwrap();
    assert_eq!(mp.lines().count(), 2);
}

/// applytopup without arguments: the compulsory options, the usage, exit 1 (no file is read).
#[test]
fn applytopup_messages() {
    let settings = settings();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code =
        larmorx_nc_topup::applytopup(&[], &settings, UpstreamDefects::Fix, 1, &mut out, &mut err);
    let e = String::from_utf8_lossy(&err);
    assert_eq!(code, 1);
    assert!(e.contains("The following COMPULSORY options have not been set:"));
    assert!(e.contains("applytopup (Version 1.0)"));
    assert!(out.is_empty());
}
