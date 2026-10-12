// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Karel Lopez Vilaret.
//! Bit-identity with AFNI's own 3dTshift, when the oracle is available: the replica with
//! `LARMORX_UPSTREAM_DEFECTS=reproduce` writes AFNI's data and its 348-byte header (`vox_offset`
//! aside: AFNI adds its history extension).
//!
//! The oracle is `LARMORX_AFNI_BIN` (the binary or its directory), else the workspace's
//! `oracles/afni-25.2.09/bin/3dTshift` (built by `scripts/build_afni_oracle.sh`). Without it the
//! test prints why and passes, so it runs only where the oracle exists (not in CI). The full
//! comparison is the parity suite (`python -m larmorx_validation parity afni-tshift
//! --implementation replica`).

mod common;

use std::path::{Path, PathBuf};

const EXE: &str = env!("CARGO_BIN_EXE_larmorx-gpl");

fn oracle() -> Option<PathBuf> {
    if let Some(env) = std::env::var_os("LARMORX_AFNI_BIN") {
        let p = PathBuf::from(env);
        return Some(if p.is_dir() { p.join("3dTshift") } else { p });
    }
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .map(|a| a.join("oracles/afni-25.2.09/bin/3dTshift"))
        .find(|p| p.is_file())
}

#[test]
fn bit_identical_to_afni() {
    let Some(afni) = oracle() else {
        eprintln!("skipped: the AFNI 3dTshift oracle was not found (LARMORX_AFNI_BIN)");
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let inputs = [
        ("float32", dir.path().join("f32.nii")),
        ("int16", dir.path().join("i16.nii")),
    ];
    common::float32(&inputs[0].1);
    common::int16_scaled(&inputs[1].1);
    let cases: &[&[&str]] = &[
        &["-tpattern", "alt+z"],
        &["-tpattern", "seq-z", "-ignore", "3"],
        &["-tpattern", "alt+z", "-linear"],
        &["-tpattern", "alt-z", "-cubic", "-rlt+"],
        &["-tpattern", "alt+z2", "-quintic"],
        &["-tpattern", "alt+z", "-heptic", "-tzero", "0"],
        &["-tpattern", "alt+z", "-heptic", "-no_detrend"],
        &["-tpattern", "seq+z", "-wsinc5", "-rlt"],
        &["-tpattern", "alt+z", "-wsinc9", "-slice", "2"],
        &[
            "-tpattern",
            "@1D: 0 1.5 0.5 1",
            "-TR",
            "2000ms",
            "-tzero",
            "300",
        ],
    ];
    let mut n = 0;
    for (kind, input) in &inputs {
        for (i, args) in cases.iter().enumerate() {
            let outs: Vec<PathBuf> = ["afni", "gpl"]
                .iter()
                .map(|w| dir.path().join(format!("{kind}-{i}-{w}.nii")))
                .collect();
            let tail = |out: &Path| {
                let mut a: Vec<String> = args.iter().map(|s| (*s).to_owned()).collect();
                a.extend(["-prefix".into(), out.display().to_string()]);
                a.push(input.display().to_string());
                a
            };
            let a = std::process::Command::new(&afni)
                .args(tail(&outs[0]))
                .env("AFNI_DONT_LOGFILE", "YES")
                .output()
                .unwrap();
            assert!(a.status.success(), "AFNI {kind} {args:?}: {a:?}");
            let mut gpl_args = vec!["afni".to_owned(), "3dTshift".to_owned()];
            gpl_args.extend(tail(&outs[1]));
            let g = std::process::Command::new(EXE)
                .args(&gpl_args)
                .env("LARMORX_UPSTREAM_DEFECTS", "reproduce")
                .output()
                .unwrap();
            assert!(g.status.success(), "larmorx-gpl {kind} {args:?}: {g:?}");
            let (ab, ah) = common::read(&outs[0]);
            let (gb, gh) = common::read(&outs[1]);
            assert_eq!(ah.datatype, gh.datatype, "{kind} {args:?}");
            assert_eq!(
                (ah.scl_slope, ah.pixdim[4], ah.toffset),
                (gh.scl_slope, gh.pixdim[4], gh.toffset),
                "{kind} {args:?}"
            );
            let differ = ab.iter().zip(&gb).filter(|(x, y)| x != y).count();
            assert!(ab == gb, "{kind} {args:?}: {differ} bytes differ");
            // The header, byte for byte, but `vox_offset` (bytes 108-111).
            let (ra, rg) = (
                std::fs::read(&outs[0]).unwrap(),
                std::fs::read(&outs[1]).unwrap(),
            );
            let header = |r: &[u8]| [&r[..108], &r[112..348]].concat();
            assert_eq!(header(&ra), header(&rg), "{kind} {args:?}: header");
            n += 1;
        }
    }
    eprintln!("{n} cases bit-identical to {}", afni.display());
}

/// AFNI-16 and AFNI-12's check when AFNI writes, against AFNI itself: with
/// `LARMORX_UPSTREAM_DEFECTS=reproduce` the replica writes AFNI's plumb header for an sform
/// 0.006° from plumb (byte for byte, `vox_offset` aside), and refuses to write a `-prefix` that
/// passes AFNI's option scan only through its `3dcalc` exception (exit 0, no file), as AFNI
/// does; by default it keeps the matrix and writes the file.
#[test]
fn nearly_plumb_and_write_time_names_as_afni() {
    use larmorx_core::affine::Affine;
    use larmorx_core::element::DataType;
    use larmorx_core::ndarray::{ArrayViewD, IxDyn, ShapeBuilder};
    use larmorx_io::nifti::{self, NiftiHeader, NiftiVersion, WriteOptions};

    let Some(afni) = oracle() else {
        eprintln!("skipped: the AFNI 3dTshift oracle was not found (LARMORX_AFNI_BIN)");
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("nearly-plumb.nii");
    let mut h = NiftiHeader::new(NiftiVersion::V1, &common::SHAPE, DataType::F32).unwrap();
    let affine = Affine::from_linear(
        [[2.0, 0.0, -3.0e-4], [0.0, 2.0, 0.0], [2.0e-4, 0.0, 3.0]],
        [-4.0, -2.0, -6.0],
    );
    h.set_sform(&affine, 1);
    h.set_qform(&affine, 1).unwrap();
    h.pixdim[4] = 2.0;
    h.xyzt_units = 2 | 8;
    let v: Vec<f32> = common::values().iter().map(|&x| x as f32).collect();
    let view = ArrayViewD::from_shape(IxDyn(&common::SHAPE).f(), v.as_slice()).unwrap();
    nifti::write(&input, &h, view, &WriteOptions::default()).unwrap();

    let run = |exe: &Path, gpl: bool, prefix: &str, mode: &str| {
        let mut command = std::process::Command::new(exe);
        if gpl {
            command.args(["afni", "3dTshift"]);
        }
        let out = command
            .args(["-tpattern", "alt+z", "-prefix", prefix])
            .arg(&input)
            .current_dir(dir.path())
            .env("AFNI_DONT_LOGFILE", "YES")
            .env("LARMORX_UPSTREAM_DEFECTS", mode)
            .output()
            .unwrap();
        (
            out.status.code(),
            String::from_utf8_lossy(&out.stderr).into_owned(),
        )
    };
    let exe = Path::new(EXE);
    let (a, _) = run(&afni, false, "afni.nii", "");
    let (r, err) = run(exe, true, "reproduce.nii", "reproduce");
    let (f, _) = run(exe, true, "fix.nii", "fix");
    assert_eq!((a, r, f), (Some(0), Some(0), Some(0)), "{err}");
    assert!(
        err.contains("upstream defect AFNI-16 (3dTshift) reproduced"),
        "{err}"
    );
    let raw = |name: &str| std::fs::read(dir.path().join(name)).unwrap();
    let header = |r: &[u8]| [&r[..108], &r[112..348]].concat();
    assert_eq!(header(&raw("afni.nii")), header(&raw("reproduce.nii")));
    let (_, kept) = common::read(&dir.path().join("fix.nii"));
    assert_eq!(
        kept.srow_z[0],
        f64::from(2.0e-4f32),
        "the default keeps the matrix"
    );

    // "./3dcalc a b.nii" is refused when AFNI writes it.
    for (exe, gpl, mode) in [(afni.as_path(), false, ""), (exe, true, "reproduce")] {
        let (code, err) = run(exe, gpl, "3dcalc a b.nii", mode);
        assert_eq!(code, Some(0), "{err}");
        assert!(
            err.contains("Illegal filename for NIfTI output: ./3dcalc a b.nii"),
            "{err}"
        );
        assert!(!dir.path().join("3dcalc a b.nii").exists());
    }
}
