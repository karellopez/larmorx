// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Karel Lopez Vilaret.
//! Bit-identity with AFNI's own 3dTshift, when the oracle is available.
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
            let refs: Vec<&str> = gpl_args.iter().map(String::as_str).collect();
            let g = common::run(EXE, &refs);
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
            n += 1;
        }
    }
    eprintln!("{n} cases bit-identical to {}", afni.display());
}
