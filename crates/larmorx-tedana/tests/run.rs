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
    run_at(dir, args, env, None)
}

fn run_at(
    dir: &Path,
    args: &[&str],
    env: &[(&str, &str)],
    terminal_columns: Option<usize>,
) -> (u8, String, String) {
    // one thread unless a test says otherwise (without --n-threads the command line takes
    // OMP_NUM_THREADS, else every logical CPU)
    let mut vars: std::collections::HashMap<String, String> = [
        ("COLUMNS".to_owned(), "80".to_owned()),
        ("OMP_NUM_THREADS".to_owned(), "1".to_owned()),
    ]
    .into_iter()
    .collect();
    for (k, v) in env {
        vars.insert((*k).to_owned(), (*v).to_owned());
    }
    vars.retain(|_, v| !v.is_empty());
    let settings = Settings {
        vars,
        dir: Some(dir.to_path_buf()),
        terminal_columns,
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

/// argparse's width (§2.3): `COLUMNS`, else the terminal's, else 80.
#[test]
fn help_width_from_the_terminal_when_columns_is_unset() {
    let dir = tempfile::tempdir().unwrap();
    let at = |env: &[(&str, &str)], term| run_at(dir.path(), &["-h"], env, term).1;
    let forty = at(&[("COLUMNS", "40")], None);
    assert_ne!(forty, at(&[], None));
    assert_eq!(at(&[("COLUMNS", "")], Some(40)), forty);
    assert_eq!(at(&[("COLUMNS", "")], None), at(&[("COLUMNS", "80")], None));
    assert_eq!(at(&[("COLUMNS", "80")], Some(40)), at(&[], None));
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
/// Without its own `--n-threads` the run takes `OMP_NUM_THREADS` (larmorx's `--n-threads`
/// before the family writes it there), as the replica's program does; given, its own wins.
/// The files are the same (the sizes of the pools started tell which count ran).
#[test]
fn the_thread_count_without_its_own_option() {
    let dir = tempfile::tempdir().unwrap();
    write_inputs(dir.path());
    let read = |out: &str| {
        let d = dir.path().join(out);
        ["T2starmap.nii.gz", "desc-optcom_bold.nii.gz"].map(|f| read_f32(&d.join(f)))
    };
    let mut args = base(&["--out-dir", "env"]);
    let (code, _, err) = run(dir.path(), &args, &[("OMP_NUM_THREADS", "11")]);
    assert_eq!(code, 0, "{err}");
    assert!(larmorx_core::parallel::pool_sizes().contains(&11));
    let call = std::fs::read_to_string(dir.path().join("env/t2smap_call.sh")).unwrap();
    assert!(!call.contains("--n-threads"), "{call}");
    args = base(&["--n-threads", "2", "--out-dir", "own"]);
    let (code, _, err) = run(dir.path(), &args, &[("OMP_NUM_THREADS", "13")]);
    assert_eq!(code, 0, "{err}");
    assert!(!larmorx_core::parallel::pool_sizes().contains(&13));
    assert_eq!(read("env"), read("own"));
}

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

/// §3 step 2: `--exclude`'s four errors (§5.2) stop the run before any message, the output
/// directory made (step 1) and left empty, in both modes. Checked on the directory itself: an
/// oracle-output cache's replay records files, not empty directories.
#[test]
fn exclude_errors_leave_the_output_directory_empty() {
    let dir = tempfile::tempdir().unwrap();
    write_inputs(dir.path());
    let cases = [
        ("0:4:2", "Step sizes are not supported."),
        ("2:", "Open-ended ranges are not supported."),
        ("-1", "Negative indices are not supported."),
        ("a", "invalid literal for int() with base 10: 'a'"),
    ];
    for mode in ["fix", "reproduce"] {
        for (i, (text, message)) in cases.iter().enumerate() {
            let out = format!("x-{mode}-{i}");
            let mut args = base(&["--exclude"]);
            args.push(text);
            args.push("--out-dir");
            args.push(Box::leak(out.clone().into_boxed_str()));
            let (code, stdout, err) = run(dir.path(), &args, &[("LARMORX_UPSTREAM_DEFECTS", mode)]);
            assert_eq!(code, 1, "{text}: {err}");
            assert!(stdout.is_empty());
            assert!(err.contains(message), "{text}: {err}");
            assert!(!err.contains("Using output directory"), "{text}: {err}");
            let made = dir.path().join(&out);
            assert!(made.is_dir(), "{text}: no output directory");
            assert_eq!(
                std::fs::read_dir(&made).unwrap().count(),
                0,
                "{text}: the output directory is not empty"
            );
        }
    }
}

/// TED-16 in `reproduce` without the dropout method (§15.1, §7.3): every volume excluded,
/// `loglin` writes T2* = 500 ms and S0 = 1 with exit 0; `curvefit` stops after the log-linear
/// fit's warning on the voxels with no sample; the default refuses both.
#[test]
fn every_volume_excluded_without_dropout() {
    let dir = tempfile::tempdir().unwrap();
    write_inputs(dir.path());
    let reproduce = [("LARMORX_UPSTREAM_DEFECTS", "reproduce")];
    let common = ["--masktype", "none", "--exclude", "0:5", "--out-dir"];
    let mut args = base(&common);
    args.push("loglin");
    let (code, _, err) = run(dir.path(), &args, &reproduce);
    assert_eq!(code, 0, "{err}");
    assert!(err.contains("RuntimeWarning: divide by zero encountered in divide"));
    assert!(err.contains("TED-16 (tedana) reproduced"));
    let read = |name: &str| {
        let img = nifti::read(dir.path().join("loglin").join(name), &Default::default()).unwrap();
        let larmorx_core::array::DynArray::F32(a) = img.data else {
            panic!("float32")
        };
        let mut v: Vec<f32> = a.iter().copied().filter(|&x| x != 0.0).collect();
        v.dedup();
        v
    };
    assert_eq!(read("T2starmap.nii.gz"), [0.5]);
    assert_eq!(read("S0map.nii.gz"), [1.0]);
    let mut args = base(&[
        "--masktype",
        "none",
        "--exclude",
        "0:5",
        "--fittype",
        "curvefit",
        "--out-dir",
    ]);
    args.push("curvefit");
    let (code, _, err) = run(dir.path(), &args, &reproduce);
    assert_eq!(code, 1, "{err}");
    let warning = err
        .find("RuntimeWarning: divide by zero encountered in divide")
        .unwrap();
    let error = err
        .find("ValueError: zero-size array to reduction operation minimum which has no identity")
        .unwrap();
    assert!(warning < error);
    assert!(
        !dir.path()
            .join("curvefit/desc-decayFitFailures_mask.nii.gz")
            .exists()
    );
    let mut args = base(&common);
    args.push("default");
    let (code, _, err) = run(dir.path(), &args, &[]);
    assert_eq!(code, 1);
    assert!(err.contains(
        "ValueError: --exclude leaves no volume for the adaptive mask and the T2*/S0 fit (all 5 \
         volumes excluded)"
    ));
}

/// An empty user mask (random testing, 2026-11-11, black-box: tedana 26.0.3 and the replica):
/// with the dropout method tedana's `IndexError` in both modes (not TED-01's error, which is for
/// NaN means); without a method, nilearn's refusal of an empty mask at the adaptive mask's image,
/// the first written, in both modes.
#[test]
fn an_empty_user_mask_fails_as_tedana_in_both_modes() {
    let dir = tempfile::tempdir().unwrap();
    write_inputs(dir.path());
    let affine = Affine::from_rows([
        [-3.0, 0.0, 0.0, 10.5],
        [0.0, 3.0, 0.0, -7.5],
        [0.0, 0.0, 3.5, -5.25],
        [0.0, 0.0, 0.0, 1.0],
    ]);
    let shape = [NX, NY, NZ];
    let hdr = nifti::header_for_image(
        None,
        None,
        &shape,
        larmorx_core::element::DataType::U8,
        &affine,
    )
    .unwrap();
    let arr = ArrayD::from_shape_vec(IxDyn(&shape).f(), vec![0u8; NX * NY * NZ]).unwrap();
    nifti::write(
        dir.path().join("empty.nii.gz"),
        &hdr,
        arr.view(),
        &WriteOptions::default(),
    )
    .unwrap();
    for mode in ["fix", "reproduce"] {
        let env = [("LARMORX_UPSTREAM_DEFECTS", mode)];
        let mut args = BASE.to_vec();
        args.extend(["empty.nii.gz", "--out-dir", "dropout"]);
        let (code, _, err) = run(dir.path(), &args, &env);
        assert_eq!(code, 1, "{err}");
        assert!(
            err.ends_with("IndexError: index -1 is out of bounds for axis 0 with size 0\n"),
            "{mode}: {err}"
        );
        assert!(!err.contains("TED-01"), "{err}");
        let mut args = BASE.to_vec();
        args.extend(["empty.nii.gz", "--masktype", "none", "--out-dir", "none"]);
        let (code, _, err) = run(dir.path(), &args, &env);
        assert_eq!(code, 1, "{err}");
        assert!(
            err.ends_with("ValueError: The mask is invalid as it is empty: it masks all data.\n"),
            "{mode}: {err}"
        );
        assert!(
            !dir.path()
                .join("none/desc-adaptiveGoodSignal_mask.nii.gz")
                .exists()
        );
        std::fs::remove_dir_all(dir.path().join("dropout")).unwrap();
        std::fs::remove_dir_all(dir.path().join("none")).unwrap();
    }
}

fn affine() -> Affine {
    Affine::from_rows([
        [-3.0, 0.0, 0.0, 10.5],
        [0.0, 3.0, 0.0, -7.5],
        [0.0, 0.0, 3.5, -5.25],
        [0.0, 0.0, 0.0, 1.0],
    ])
}

/// Writes `values` (Fortran order) as a float32 NIfTI-1 image of `shape` with the inputs' affine.
fn write_f32(dir: &Path, name: &str, shape: &[usize], values: Vec<f32>) {
    let hdr = nifti::header_for_image(
        None,
        None,
        shape,
        larmorx_core::element::DataType::F32,
        &affine(),
    )
    .unwrap();
    let arr = ArrayD::from_shape_vec(IxDyn(shape).f(), values).unwrap();
    nifti::write(dir.join(name), &hdr, arr.view(), &WriteOptions::default()).unwrap();
}

fn read_f32(path: &Path) -> Vec<f32> {
    let img = nifti::read(path, &Default::default()).unwrap();
    let larmorx_core::array::DynArray::F32(a) = img.data else {
        panic!("float32")
    };
    a.as_slice_memory_order().unwrap().to_vec()
}

/// tedana's second reading through nilearn (§4.4, black-box, 2026-11-11: mask_probe.py): a 4D
/// mask of one volume or a 3D echo sends every echo through nilearn's masking, whose checks come
/// in its order, the mask's values first: an empty mask's text after tedana's first error, in
/// both modes.
#[test]
fn nilearns_second_reading_checks_the_mask_first() {
    let dir = tempfile::tempdir().unwrap();
    write_inputs(dir.path());
    write_f32(
        dir.path(),
        "empty4d.nii.gz",
        &[NX, NY, NZ, 1],
        vec![0.0; NX * NY * NZ],
    );
    write_f32(
        dir.path(),
        "empty.nii.gz",
        &[NX, NY, NZ],
        vec![0.0; NX * NY * NZ],
    );
    write_f32(
        dir.path(),
        "e1-3d.nii.gz",
        &[NX, NY, NZ],
        vec![700.0; NX * NY * NZ],
    );
    let chained = |first: &str, last: &str, err: &str| {
        let a = err.find(&format!("ValueError: {first}\n")).expect(err);
        let b = err
            .find("During handling of the above exception, another exception occurred:")
            .expect(err);
        let c = err.rfind(&format!("ValueError: {last}\n")).expect(err);
        assert!(a < b && b < c, "{err}");
    };
    let empty = "The mask is invalid as it is empty: it masks all data.";
    for mode in ["fix", "reproduce"] {
        let env = [("LARMORX_UPSTREAM_DEFECTS", mode)];
        let mut args = BASE.to_vec();
        args.extend(["empty4d.nii.gz", "--out-dir", "a"]);
        let (code, _, err) = run(dir.path(), &args, &env);
        assert_eq!(code, 1);
        chained("Image/mask shape mismatch", empty, &err);
        let args = [
            "-d",
            "e1-3d.nii.gz",
            "e2.nii.gz",
            "e3.nii.gz",
            "-e",
            "0.012",
            "0.028",
            "0.044",
            "--mask",
            "empty.nii.gz",
            "--out-dir",
            "b",
        ];
        let (code, _, err) = run(dir.path(), &args, &env);
        assert_eq!(code, 1);
        chained("Expected 4D image", empty, &err);
        let args = [
            "-d",
            "e1-3d.nii.gz",
            "e2.nii.gz",
            "e3.nii.gz",
            "-e",
            "0.012",
            "0.028",
            "0.044",
            "--mask",
            "mask.nii.gz",
            "--out-dir",
            "c",
        ];
        let (code, _, err) = run(dir.path(), &args, &env);
        assert_eq!(code, 1);
        chained("Expected 4D image", "Expected 4D image", &err);
        for d in ["a", "b", "c"] {
            std::fs::remove_dir_all(dir.path().join(d)).unwrap();
        }
    }
}

/// Every voxel removed by the adaptive mask (black-box, 2026-11-11): tedana's IndexError after
/// `Computing T2* map` with both fit types and modes, TED-31's clear error by default; with
/// `--n-independent-echos` above 3 numpy's scalar-divide warning at each of §6.5's two
/// percentages.
#[test]
fn no_voxel_left_for_the_fit_is_ted31() {
    let dir = tempfile::tempdir().unwrap();
    write_inputs(dir.path());
    // a negative first echo: §6.1's base count 0 everywhere
    let p = dir.path().join("e1.nii.gz");
    let neg: Vec<f32> = read_f32(&p).iter().map(|x| -x).collect();
    write_f32(dir.path(), "e1.nii.gz", &[NX, NY, NZ, NT], neg);
    for (mode, last) in [
        (
            "reproduce",
            "IndexError: index 0 is out of bounds for axis 1 with size 0",
        ),
        (
            "fix",
            "ValueError: no voxel of the mask has good signal (the adaptive mask removed all \
             144), so there is nothing to fit (tedana fails with IndexError)",
        ),
    ] {
        let env = [("LARMORX_UPSTREAM_DEFECTS", mode)];
        for (k, extra) in [
            &[][..],
            &["--fittype", "curvefit", "--fitmode", "ts"][..],
            &["--n-independent-echos", "4"][..],
        ]
        .iter()
        .enumerate()
        {
            let out = format!("{mode}{k}");
            let mut args = base(extra);
            args.extend(["--out-dir", &out]);
            let (code, _, err) = run(dir.path(), &args, &env);
            assert_eq!(code, 1, "{err}");
            let fit = err.find("Computing T2* map").expect(&err);
            assert!(err[fit..].contains(&format!("{last}\n")), "{err}");
            assert!(err.contains("upstream defect TED-31 (tedana)"), "{err}");
            assert!(
                dir.path()
                    .join(&out)
                    .join("desc-adaptiveGoodSignal_mask.nii.gz")
                    .exists()
            );
            let divides = err
                .matches("RuntimeWarning: invalid value encountered in scalar divide")
                .count();
            assert_eq!(divides, if k == 2 { 2 } else { 0 }, "{err}");
        }
    }
}

/// TED-46 (§7.2's observations without a slope): a voxel whose samples are all equal takes the
/// slope 0 (T2* 500 ms) with the notice; the RMSE map's voxels without a value are numpy's `nan`
/// (0x7FC00000, §9); a voxel with observations all 0 is not TED-46's (tedana's slope is 0 too).
#[test]
fn equal_observations_take_the_slope_zero() {
    let dir = tempfile::tempdir().unwrap();
    write_inputs(dir.path());
    let at = |x: usize, y: usize, z: usize, t: usize| x + NX * (y + NY * (z + NZ * t));
    for e in 1..=3 {
        let p = dir.path().join(format!("e{e}.nii.gz"));
        let mut v = read_f32(&p);
        for t in 0..NT {
            v[at(2, 3, 1, t)] = 255.0; // equal observations
            if e == 2 {
                v[at(4, 2, 2, t)] = -1.0; // one good echo: no RMSE
            }
        }
        write_f32(dir.path(), &format!("e{e}.nii.gz"), &[NX, NY, NZ, NT], v);
    }
    for mode in ["fix", "reproduce"] {
        let env = [("LARMORX_UPSTREAM_DEFECTS", mode)];
        let mut args = base(&["--masktype", "none"]);
        args.extend(["--out-dir", mode]);
        let (code, _, err) = run(dir.path(), &args, &env);
        assert_eq!(code, 0, "{err}");
        let verb = if mode == "fix" {
            "corrected"
        } else {
            "reproduced"
        };
        assert!(
            err.contains(&format!(
                "upstream defect TED-46 (tedana) {verb}: 1 voxel(s) whose log-linear \
                 observations have no slope"
            )),
            "{err}"
        );
        assert!(err.contains("RuntimeWarning: divide by zero encountered in divide"));
        let out = dir.path().join(mode);
        let t2s = read_f32(&out.join("T2starmap.nii.gz"));
        if mode == "reproduce" {
            assert_eq!(t2s[at(2, 3, 1, 0)], 0.5);
        } else {
            // 500 ms lies above ten caps: TED-05 caps the full map too
            let limited = read_f32(&out.join("desc-limited_T2starmap.nii.gz"));
            assert_eq!(t2s[at(2, 3, 1, 0)], limited[at(2, 3, 1, 0)]);
            assert!(err.contains("TED-05"), "{err}");
        }
        let s0 = read_f32(&out.join("S0map.nii.gz"));
        assert!(
            (s0[at(2, 3, 1, 0)] - 256.0).abs() < 1e-3,
            "{}",
            s0[at(2, 3, 1, 0)]
        );
        let rmse = read_f32(&out.join("desc-rmse_statmap.nii.gz"));
        assert_eq!(rmse[at(4, 2, 2, 0)].to_bits(), 0x7FC0_0000);
    }
}

/// Random testing's second trial (2026-11-11, findings 455, 588, 761): an infinite first-echo
/// sample is no TED-01 (tedana sorts and matches an infinite mean): the default's dropout
/// thresholds are reproduce's, with no notice.
#[test]
fn an_infinite_first_echo_mean_is_no_ted01() {
    let dir = tempfile::tempdir().unwrap();
    write_inputs(dir.path());
    let p = dir.path().join("e1.nii.gz");
    let mut v = read_f32(&p);
    v[3 + NX * (2 + NY)] = f32::INFINITY; // voxel (3, 2, 1), volume 0
    write_f32(dir.path(), "e1.nii.gz", &[NX, NY, NZ, NT], v);
    let thresholds = |err: &str| {
        err.lines()
            .find(|l| l.contains("Echo-wise intensity thresholds"))
            .expect(err)
            .to_owned()
    };
    let mut out = Vec::new();
    for mode in ["fix", "reproduce"] {
        let mut args = base(&[]);
        args.extend(["--out-dir", mode]);
        let (code, _, err) = run(dir.path(), &args, &[("LARMORX_UPSTREAM_DEFECTS", mode)]);
        assert_eq!(code, 0, "{err}");
        assert!(!err.contains("TED-01"), "{err}");
        out.push(thresholds(&err));
    }
    assert_eq!(out[0], out[1]);
}

/// TED-15's notice counts the first of the fit's echo-count groups (count 1 with count 2), as
/// the replica's default does (random testing's second trial, findings 341, 812, 860, 919).
#[test]
fn ted15_counts_the_first_fit_group() {
    let dir = tempfile::tempdir().unwrap();
    write_inputs(dir.path());
    let at = |x: usize, y: usize, z: usize, t: usize| x + NX * (y + NY * (z + NZ * t));
    for e in 1..=3 {
        let p = dir.path().join(format!("e{e}.nii.gz"));
        let mut v = read_f32(&p);
        for t in 0..NT {
            // constant series: a voxel of count 1 (its second echo bad), one of count 2 (its
            // third echo bad) and one of count 3
            v[at(1, 1, 1, t)] = if e == 2 { -1.0 } else { 900.0 };
            v[at(2, 1, 1, t)] = if e == 3 {
                -1.0
            } else {
                900.0 - 100.0 * e as f32
            };
            v[at(3, 1, 1, t)] = 900.0 - 100.0 * e as f32;
        }
        write_f32(dir.path(), &format!("e{e}.nii.gz"), &[NX, NY, NZ, NT], v);
    }
    let mut args = base(&["--masktype", "none", "--combmode", "paid"]);
    args.extend(["--out-dir", "paid"]);
    let (code, _, err) = run(dir.path(), &args, &[]);
    assert_eq!(code, 0, "{err}");
    assert!(
        err.contains(
            "upstream defect TED-15 (tedana) corrected: 2 voxel(s) whose good echoes do not vary \
             over time"
        ),
        "{err}"
    );
}
