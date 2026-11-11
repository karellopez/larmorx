// SPDX-License-Identifier: Apache-2.0
//! The clean-room `t2smap` end to end through its command line (`specs/t2smap.md` §2, §3, §11,
//! §16): the help, the files and their order, the defaults' errors, thread invariance.

use std::path::Path;

use larmorx_core::affine::Affine;
use larmorx_io::nifti::{self, WriteOptions};
use larmorx_tedana::cli::{self, Settings};
use ndarray::{ArrayD, IxDyn, ShapeBuilder};

const NX: usize = 8;
const NY: usize = 6;
const NZ: usize = 4;
const NT: usize = 5;
const TES: [u32; 3] = [12, 28, 44];

/// A small phantom: S0 · (1 − 1/T2*)^TE with a seeded perturbation (plain arithmetic).
fn write_inputs(dir: &Path) {
    let affine = Affine::from_rows([
        [-3.0, 0.0, 0.0, 10.5],
        [0.0, 3.0, 0.0, -7.5],
        [0.0, 0.0, 3.5, -5.25],
        [0.0, 0.0, 0.0, 1.0],
    ]);
    let mut seed: u64 = 20261111;
    let mut rnd = move || {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((seed >> 33) as f64) / (1u64 << 31) as f64 - 0.5
    };
    for (e, &te) in TES.iter().enumerate() {
        let mut v = vec![0.0f32; NX * NY * NZ * NT];
        for t in 0..NT {
            for z in 0..NZ {
                for y in 0..NY {
                    for x in 0..NX {
                        let s0 = 1000.0 + 25.0 * x as f64 - 10.0 * y as f64;
                        let t2 = 18.0 + 3.0 * y as f64 + 2.0 * z as f64;
                        let mut d = 1.0f64;
                        for _ in 0..te {
                            d *= 1.0 - 1.0 / t2;
                        }
                        let val = s0 * d * (1.0 + 0.02 * rnd()) + 0.5 * rnd().abs();
                        v[x + NX * (y + NY * (z + NZ * t))] = val as f32;
                    }
                }
            }
        }
        let shape = [NX, NY, NZ, NT];
        let hdr = nifti::header_for_image(
            None,
            None,
            &shape,
            larmorx_core::element::DataType::F32,
            &affine,
        )
        .unwrap();
        let arr = ArrayD::from_shape_vec(IxDyn(&shape).f(), v).unwrap();
        nifti::write(
            dir.join(format!("e{}.nii.gz", e + 1)),
            &hdr,
            arr.view(),
            &WriteOptions::default(),
        )
        .unwrap();
    }
    let mut m = vec![0u8; NX * NY * NZ];
    for z in 0..NZ {
        for y in 0..NY {
            for x in 1..NX - 1 {
                m[x + NX * (y + NY * z)] = 1;
            }
        }
    }
    let shape = [NX, NY, NZ];
    let hdr = nifti::header_for_image(
        None,
        None,
        &shape,
        larmorx_core::element::DataType::U8,
        &affine,
    )
    .unwrap();
    let arr = ArrayD::from_shape_vec(IxDyn(&shape).f(), m).unwrap();
    nifti::write(
        dir.join("mask.nii.gz"),
        &hdr,
        arr.view(),
        &WriteOptions::default(),
    )
    .unwrap();
}

fn run(dir: &Path, args: &[&str], env: &[(&str, &str)]) -> (u8, String, String) {
    let mut vars: std::collections::HashMap<String, String> =
        [("COLUMNS".to_owned(), "80".to_owned())]
            .into_iter()
            .collect();
    for (k, v) in env {
        vars.insert((*k).to_owned(), (*v).to_owned());
    }
    let settings = Settings {
        vars,
        dir: Some(dir.to_path_buf()),
    };
    let args: Vec<String> = args.iter().map(|s| (*s).to_owned()).collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = cli::run(&args, &settings, &mut out, &mut err);
    (
        code,
        String::from_utf8(out).unwrap(),
        String::from_utf8(err).unwrap(),
    )
}

const BASE: [&str; 9] = [
    "-d",
    "e1.nii.gz",
    "e2.nii.gz",
    "e3.nii.gz",
    "-e",
    "0.012",
    "0.028",
    "0.044",
    "--mask",
];

fn base(extra: &[&'static str]) -> Vec<&'static str> {
    let mut v = BASE.to_vec();
    v.push("mask.nii.gz");
    v.extend_from_slice(extra);
    v
}

#[test]
fn help_at_80_columns_is_the_specs() {
    let dir = tempfile::tempdir().unwrap();
    let (code, out, err) = run(dir.path(), &["-h"], &[]);
    assert_eq!(code, 0);
    assert!(err.is_empty());
    let spec = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../specs/t2smap.md"
    ))
    .unwrap();
    let start = spec.find("usage: t2smap [-h]").unwrap();
    let end = start + spec[start..].find("\n```").unwrap();
    assert_eq!(out, format!("{}\n", &spec[start..end]));
    let (code, out, _) = run(dir.path(), &["-v"], &[]);
    assert_eq!((code, out.as_str()), (0, "t2smap v26.0.3\n"));
}

#[test]
fn parser_errors_exit_two() {
    let dir = tempfile::tempdir().unwrap();
    let (code, _, err) = run(dir.path(), &[], &[]);
    assert_eq!(code, 2);
    assert!(err.ends_with("t2smap: error: the following arguments are required: -d, -e\n"));
    let (code, _, err) = run(dir.path(), &["-d", "none.nii.gz", "-e", "0.012"], &[]);
    assert_eq!(code, 2);
    assert!(err.ends_with("t2smap: error: The file none.nii.gz does not exist!\n"));
}

#[test]
fn loglin_files_order_and_threads() {
    let dir = tempfile::tempdir().unwrap();
    write_inputs(dir.path());
    let mut outs = Vec::new();
    for threads in ["1", "4"] {
        let o = format!("out{threads}");
        let mut args = base(&["--n-threads", threads, "--out-dir"]);
        args.push(Box::leak(o.clone().into_boxed_str()));
        let (code, _, err) = run(dir.path(), &args, &[]);
        assert_eq!(code, 0, "{err}");
        assert!(err.contains("INFO     t2smap:t2smap_workflow:566 Workflow completed"));
        outs.push(dir.path().join(o));
    }
    let reg = std::fs::read_to_string(outs[0].join("desc-tedana_registry.json")).unwrap();
    assert!(
        reg.starts_with("{\n    \"adaptive mask img\": \"desc-adaptiveGoodSignal_mask.nii.gz\",")
    );
    assert!(!reg.ends_with('\n'));
    let call = std::fs::read_to_string(outs[0].join("t2smap_call.sh")).unwrap();
    assert!(call.starts_with("t2smap -d e1.nii.gz e2.nii.gz e3.nii.gz -e 0.012"));
    assert!(outs[0].join("figures").is_dir());
    for name in [
        "desc-adaptiveGoodSignal_mask.nii.gz",
        "S0map.nii.gz",
        "T2starmap.nii.gz",
        "desc-limited_S0map.nii.gz",
        "desc-limited_T2starmap.nii.gz",
        "desc-rmse_statmap.nii.gz",
        "desc-optcom_bold.nii.gz",
        "desc-confounds_timeseries.tsv",
    ] {
        let read = |d: &Path| {
            if name.ends_with(".tsv") {
                std::fs::read(d.join(name)).unwrap()
            } else {
                let img = nifti::read(d.join(name), &Default::default()).unwrap();
                format!("{:?}", img.data).into_bytes()
            }
        };
        assert_eq!(
            read(&outs[0]),
            read(&outs[1]),
            "{name} depends on the threads"
        );
    }
    // a second run stops at the adaptive mask (§11.2)
    let mut args = base(&["--out-dir"]);
    args.push("out1");
    let (code, _, err) = run(dir.path(), &args, &[]);
    assert_eq!(code, 1);
    assert!(err.contains("RuntimeError: File "));
    assert!(err.contains("desc-adaptiveGoodSignal_mask.nii.gz already exists."));
}

/// The curve fit's files (§7.3, §11.1: the failures and, with `--verbose`, the variances, in
/// that order after the adaptive mask) are the same at one and at four threads, in both fit
/// modes.
#[test]
fn curvefit_files_order_and_threads() {
    let dir = tempfile::tempdir().unwrap();
    write_inputs(dir.path());
    for mode in ["all", "ts"] {
        let mut outs = Vec::new();
        for threads in ["1", "4"] {
            let o = format!("cf-{mode}-{threads}");
            let mut args = base(&[
                "--fittype",
                "curvefit",
                "--verbose",
                "--fitmode",
                if mode == "all" { "all" } else { "ts" },
                "--n-threads",
                threads,
                "--out-dir",
            ]);
            args.push(Box::leak(o.clone().into_boxed_str()));
            let (code, _, err) = run(dir.path(), &args, &[]);
            assert_eq!(code, 0, "{err}");
            outs.push(dir.path().join(o));
        }
        let reg = std::fs::read_to_string(outs[0].join("desc-tedana_registry.json")).unwrap();
        let keys: Vec<&str> = reg
            .lines()
            .filter_map(|l| l.trim().strip_prefix('"'))
            .filter_map(|l| l.split('"').next())
            .collect();
        assert_eq!(
            keys,
            [
                "adaptive mask img",
                "combined img",
                "confounds tsv",
                "data description json",
                "fit failures img",
                "limited s0 img",
                "limited t2star img",
                "rmse img",
                "s0 img",
                "s0 variance img",
                "t2star img",
                "t2star variance img",
                "t2star-s0 covariance img",
            ]
        );
        for name in [
            "desc-decayFitFailures_mask.nii.gz",
            "stat-variance_desc-t2star_statmap.nii.gz",
            "stat-variance_desc-s0_statmap.nii.gz",
            "stat-covariance_desc-t2star+s0_statmap.nii.gz",
            "S0map.nii.gz",
            "T2starmap.nii.gz",
            "desc-limited_T2starmap.nii.gz",
            "desc-rmse_statmap.nii.gz",
            "desc-optcom_bold.nii.gz",
            "desc-confounds_timeseries.tsv",
        ] {
            let read = |d: &Path| {
                if name.ends_with(".tsv") {
                    std::fs::read(d.join(name)).unwrap()
                } else {
                    let img = nifti::read(d.join(name), &Default::default()).unwrap();
                    format!("{:?}", img.data).into_bytes()
                }
            };
            assert_eq!(
                read(&outs[0]),
                read(&outs[1]),
                "{mode}: {name} depends on the threads"
            );
        }
        let fail = nifti::read(
            outs[0].join("desc-decayFitFailures_mask.nii.gz"),
            &Default::default(),
        )
        .unwrap();
        assert_eq!(
            fail.header.data_type().unwrap(),
            larmorx_core::element::DataType::U8
        );
    }
}

#[test]
fn defaults_correct_and_name_the_defects() {
    let dir = tempfile::tempdir().unwrap();
    write_inputs(dir.path());
    let (code, _, err) = run(
        dir.path(),
        &base(&["--exclude", "5", "--out-dir", "a"]),
        &[],
    );
    assert_eq!(code, 1);
    assert!(err.contains(
        "ValueError: The maximum exclude index (5) is not below the number of timepoints in the \
         data (5).\nlarmorx: upstream defect TED-02 (tedana) corrected: --exclude index 5 of 5 \
         volumes"
    ));
    let (code, _, err) = run(
        dir.path(),
        &base(&["--exclude", "5", "--out-dir", "b"]),
        &[("LARMORX_UPSTREAM_DEFECTS", "reproduce")],
    );
    assert_eq!(code, 1);
    assert!(err.contains("IndexError: index 5 is out of bounds for axis 0 with size 5"));
    let (code, _, err) = run(
        dir.path(),
        &base(&["--dummy-scans", "5", "--out-dir", "c"]),
        &[],
    );
    assert_eq!(code, 1);
    assert!(
        err.contains(
            "--dummy-scans must be at least 0 and below the number of volumes (5); 5 given"
        )
    );
    let (code, _, err) = run(
        dir.path(),
        &[
            "-d",
            "e1.nii.gz",
            "e2.nii.gz",
            "e3.nii.gz",
            "-e",
            "12",
            "28",
            "inf",
            "--out-dir",
            "d",
        ],
        &[],
    );
    assert_eq!(code, 1);
    assert!(err.contains("TE values must be finite"));
    let (code, _, err) = run(
        dir.path(),
        &base(&["--out-dir", "e"]),
        &[("LARMORX_TEDANA_LIBRARIES", "x")],
    );
    assert_eq!(code, 2);
    assert!(err.contains("LARMORX_TEDANA_LIBRARIES"));
}
