// SPDX-License-Identifier: Apache-2.0
//! `larmorx tedana t2smap`: tedana's t2smap through the shared implementation choice since its
//! clean-room original (`larmorx_tedana`, written from `specs/t2smap.md`) was wired in
//! (2026-11-12). The standalone `larmorx` binary, run as a separate process, gives the crate's
//! own files with `--implementation clean-room`; `auto` runs the original when `larmorx-gpl`'s
//! program is not found, and `replica` then exits 127 saying how to install it; larmorx's own
//! options before the family give the bytes of the variables they replace and win over them;
//! the thread count reaches the run, which writes its log while it runs.
//!
//! One small series (8 × 7 × 6 × 4, three echoes) and a few seconds in all on CI (rule 10's
//! budget); the original's own behaviour is tested in `crates/larmorx-tedana/tests/run.rs`.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::{Command, Output};

use larmorx_core::affine::Affine;
use larmorx_core::element::DataType;
use larmorx_core::ndarray::{ArrayD, IxDyn, ShapeBuilder};
use larmorx_io::nifti::{WriteOptions, header_for_image, write};

const EXE: &str = env!("CARGO_BIN_EXE_larmorx");

/// The variables larmorx and t2smap read, removed from every run here so that this machine's
/// environment cannot change a result.
const CLEARED: &[&str] = &[
    "LARMORX_IMPLEMENTATION",
    "LARMORX_UPSTREAM_DEFECTS",
    "LARMORX_GPL_BIN",
    "LARMORX_TEDANA_LIBRARIES",
    "OMP_NUM_THREADS",
    "COLUMNS",
];

const SHAPE: [usize; 4] = [8, 7, 6, 4];

/// A float32 image of `values` (Fortran order) on a 3 × 3 × 3.5 mm grid.
fn save(path: &Path, shape: &[usize], values: Vec<f32>) {
    let affine = Affine::from_rows([
        [-3.0, 0.0, 0.0, 10.5],
        [0.0, 3.0, 0.0, -9.0],
        [0.0, 0.0, 3.5, -8.75],
        [0.0, 0.0, 0.0, 1.0],
    ]);
    let data = ArrayD::from_shape_vec(IxDyn(shape).f(), values).unwrap();
    let header = header_for_image(None, None, shape, DataType::F32, &affine).unwrap();
    write(path, &header, data.view(), &WriteOptions::default()).unwrap();
}

/// Three echoes of a small series in `dir` (a block of signal decaying by the ratios 1, 0.6 and
/// 0.36 over a dim background, plain arithmetic only) and a mask of the block; t2smap's
/// arguments for them, ending with `--out-dir out --overwrite`.
fn inputs(dir: &Path) -> Vec<String> {
    let n: usize = SHAPE.iter().product();
    let spatial = SHAPE[0] * SHAPE[1] * SHAPE[2];
    let inside = |i: usize| {
        let (x, y, z) = (
            i % SHAPE[0],
            (i / SHAPE[0]) % SHAPE[1],
            (i / (SHAPE[0] * SHAPE[1])) % SHAPE[2],
        );
        (1..7).contains(&x) && (1..6).contains(&y) && (1..5).contains(&z)
    };
    let mut args: Vec<String> = vec!["-d".into()];
    for (e, ratio) in [1.0f32, 0.6, 0.36].into_iter().enumerate() {
        let values = (0..n)
            .map(|i| {
                let noise = ((i * 7919 + e * 31) % 97) as f32;
                let t = (i / spatial) as f32;
                if inside(i % spatial) {
                    (1000.0 + noise + 3.0 * t) * ratio
                } else {
                    noise * ratio
                }
            })
            .collect();
        let name = format!("e{}.nii", e + 1);
        save(&dir.join(&name), &SHAPE, values);
        args.push(name);
    }
    let mask = (0..spatial)
        .map(|i| f32::from(u8::from(inside(i))))
        .collect();
    save(&dir.join("mask.nii"), &SHAPE[..3], mask);
    for a in ["-e", "0.012", "0.028", "0.044", "--mask", "mask.nii"] {
        args.push(a.into());
    }
    for a in ["--out-dir", "out", "--overwrite"] {
        args.push(a.into());
    }
    args
}

/// The standalone binary in `dir` with `args` and the variables `env` (those of [`CLEARED`]
/// removed first, `PATH` an empty directory: no `larmorx-gpl` there).
fn run(dir: &Path, env: &[(&str, &str)], args: &[&str]) -> Output {
    let empty = dir.join("no-programs");
    std::fs::create_dir_all(&empty).unwrap();
    let mut command = Command::new(EXE);
    command.current_dir(dir).args(args).env("PATH", &empty);
    for name in CLEARED {
        command.env_remove(name);
    }
    for (k, v) in env {
        command.env(k, v);
    }
    command.output().unwrap()
}

/// Every file under `dir/out` by its path there, removed afterwards.
fn take_outputs(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut files = BTreeMap::new();
    let mut todo = vec![dir.join("out")];
    while let Some(d) = todo.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else {
            continue;
        };
        for entry in entries {
            let p = entry.unwrap().path();
            if p.is_dir() {
                todo.push(p);
            } else {
                let name = p
                    .strip_prefix(dir)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                files.insert(name, std::fs::read(&p).unwrap());
            }
        }
    }
    let _ = std::fs::remove_dir_all(dir.join("out"));
    files
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// Whether this machine has a `larmorx-gpl` the binary would find without the variable and
/// `PATH`: beside the binary (a build of `crates-gpl` into the same target folder).
fn replica_beside_the_binary() -> bool {
    let name = if cfg!(windows) {
        "larmorx-gpl.exe"
    } else {
        "larmorx-gpl"
    };
    Path::new(EXE).with_file_name(name).exists()
}

/// `larmorx --implementation clean-room tedana t2smap ...` writes the files and the log of
/// `larmorx_tedana::cli::run` with the same arguments in the same kind of directory.
#[test]
fn the_command_line_runs_the_clean_room_original() {
    let tmp = tempfile::tempdir().unwrap();
    let (a, b) = (tmp.path().join("a"), tmp.path().join("b"));
    std::fs::create_dir_all(&a).unwrap();
    std::fs::create_dir_all(&b).unwrap();
    let args = inputs(&a);
    inputs(&b);
    let argv: Vec<&str> = ["--implementation", "clean-room", "tedana", "t2smap"]
        .into_iter()
        .chain(args.iter().map(String::as_str))
        .collect();
    let o = run(&a, &[("OMP_NUM_THREADS", "2")], &argv);
    assert_eq!(o.status.code(), Some(0), "{}", text(&o.stderr));
    assert!(o.stdout.is_empty(), "{}", text(&o.stdout));
    let from_cli = take_outputs(&a);
    for name in [
        "out/T2starmap.nii.gz",
        "out/desc-optcom_bold.nii.gz",
        "out/desc-adaptiveGoodSignal_mask.nii.gz",
        "out/desc-confounds_timeseries.tsv",
        "out/desc-tedana_registry.json",
        "out/t2smap_call.sh",
    ] {
        assert!(from_cli.contains_key(name), "{name}: {:?}", from_cli.keys());
    }
    let settings = larmorx_tedana::cli::Settings {
        vars: [("COLUMNS", "80"), ("OMP_NUM_THREADS", "1")]
            .into_iter()
            .map(|(k, v)| (k.to_owned(), v.to_owned()))
            .collect(),
        dir: Some(b.clone()),
        terminal_columns: None,
    };
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = larmorx_tedana::cli::run(&args, &settings, &mut out, &mut err);
    assert_eq!(code, 0, "{}", text(&err));
    let direct = take_outputs(&b);
    assert_eq!(
        from_cli.keys().collect::<Vec<_>>(),
        direct.keys().collect::<Vec<_>>()
    );
    for (name, bytes) in &from_cli {
        assert!(*bytes == direct[name], "{name} differs");
    }
    let log = |bytes: &[u8], dir: &Path| text(bytes).replace(&dir.display().to_string(), "<dir>");
    assert_eq!(log(&o.stderr, &a), log(&err, &b));
    assert!(text(&o.stderr).contains("Workflow completed"));
}

/// `--n-threads`, `--upstream-defects` and `--implementation` before the family give the bytes
/// of `OMP_NUM_THREADS`, `LARMORX_UPSTREAM_DEFECTS` and `LARMORX_IMPLEMENTATION` (the files and
/// both streams), and an option given wins over its variable.
#[test]
fn the_options_give_the_variables_bytes() {
    let tmp = tempfile::tempdir().unwrap();
    let d = tmp.path();
    let args = inputs(d);
    let with = |options: &[&'static str]| -> Vec<String> {
        options
            .iter()
            .map(|s| (*s).to_owned())
            .chain(["tedana".into(), "t2smap".into()])
            .chain(args.iter().cloned())
            .collect()
    };
    let same = |base: &[(&str, &str)], option: &[&'static str], variable: &[(&str, &str)]| {
        let a_args = with(option);
        let a = run(
            d,
            base,
            &a_args.iter().map(String::as_str).collect::<Vec<_>>(),
        );
        let fa = take_outputs(d);
        let env: Vec<(&str, &str)> = base.iter().chain(variable).copied().collect();
        let b_args = with(&[]);
        let b = run(
            d,
            &env,
            &b_args.iter().map(String::as_str).collect::<Vec<_>>(),
        );
        let fb = take_outputs(d);
        assert_eq!(a.status.code(), Some(0), "{option:?}: {}", text(&a.stderr));
        assert_eq!(a.status.code(), b.status.code(), "{option:?}");
        assert_eq!(a.stdout, b.stdout, "{option:?}");
        assert_eq!(text(&a.stderr), text(&b.stderr), "{option:?}");
        assert!(fa == fb, "{option:?}: the output files differ");
        (text(&a.stderr), fa)
    };
    let clean = [("LARMORX_IMPLEMENTATION", "clean-room")];
    let (log_1, files_1) = same(&clean, &["--n-threads", "3"], &[("OMP_NUM_THREADS", "3")]);
    let (log_r, files_r) = same(
        &clean,
        &["--upstream-defects", "reproduce"],
        &[("LARMORX_UPSTREAM_DEFECTS", "reproduce")],
    );
    let (log_c, files_c) = same(
        &[],
        &["--implementation", "clean-room"],
        &[("LARMORX_IMPLEMENTATION", "clean-room")],
    );
    let (log_o, _) = same(
        &[],
        &["--implementation", "original"],
        &[("LARMORX_IMPLEMENTATION", "original")],
    );
    // the thread count never changes the files; the default and reproduce agree here except
    // where a defect's correction or a maths notice applies, which each log names
    assert!(files_1 == files_c && log_1 == log_c && log_o == log_c);
    let named = |log: &str, mode: &str| log.lines().any(|l| l.contains(&format!(") {mode}")));
    assert!(
        files_r != files_c || named(&log_r, "reproduced") || !named(&log_c, "corrected"),
        "{log_r}\n{log_c}"
    );
    // the option wins over its variable
    let (log_w, files_w) = same(
        &[
            ("LARMORX_IMPLEMENTATION", "replica"),
            ("LARMORX_UPSTREAM_DEFECTS", "reproduce"),
            ("OMP_NUM_THREADS", "1"),
        ],
        &[
            "--implementation",
            "clean-room",
            "--upstream-defects",
            "fix",
            "--n-threads",
            "3",
        ],
        &[
            ("LARMORX_IMPLEMENTATION", "clean-room"),
            ("LARMORX_UPSTREAM_DEFECTS", "fix"),
            ("OMP_NUM_THREADS", "3"),
        ],
    );
    assert!(files_w == files_c && log_w == log_c);
}

/// Without `larmorx-gpl`'s program, `auto` (the default) runs the clean-room original and
/// `replica` exits 127 with the command that installs it.
#[test]
fn auto_runs_the_original_without_the_replica() {
    if replica_beside_the_binary() {
        eprintln!("a larmorx-gpl is beside the binary: the search would find it; not run");
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    let d = tmp.path();
    let o = run(d, &[], &["tedana", "t2smap", "-v"]);
    assert_eq!(
        (o.status.code(), text(&o.stdout), text(&o.stderr)),
        (Some(0), "t2smap v26.0.3\n".to_owned(), String::new())
    );
    let o = run(d, &[], &["tedana", "t2smap", "-h"]);
    assert_eq!(o.status.code(), Some(0));
    assert!(text(&o.stdout).starts_with("usage: t2smap [-h] -d FILE [FILE ...]"));
    let args = inputs(d);
    let argv: Vec<&str> = ["tedana", "t2smap"]
        .into_iter()
        .chain(args.iter().map(String::as_str))
        .collect();
    let auto = run(d, &[], &argv);
    assert_eq!(auto.status.code(), Some(0), "{}", text(&auto.stderr));
    let files_auto = take_outputs(d);
    let clean = run(d, &[("LARMORX_IMPLEMENTATION", "clean-room")], &argv);
    assert!(take_outputs(d) == files_auto && clean.stderr == auto.stderr);
    for replica in [
        vec!["--implementation", "replica", "tedana", "t2smap", "-v"],
        vec!["tedana", "t2smap", "-v"],
    ] {
        let env: &[(&str, &str)] = if replica[0] == "--implementation" {
            &[]
        } else {
            &[("LARMORX_IMPLEMENTATION", "replica")]
        };
        let o = run(d, env, &replica);
        assert_eq!(o.status.code(), Some(127), "{o:?}");
        let err = text(&o.stderr);
        assert!(
            err.starts_with("error: the replica of tedana t2smap was asked for")
                && err.contains("pip install larmorx-gpl"),
            "{err}"
        );
    }
}

/// In this process: `larmorx --n-threads 37 ... tedana t2smap` runs the original in a pool of 37
/// threads (a size no other test uses) when t2smap's own `--n-threads` is not given, and its log
/// reaches the writer.
#[test]
fn the_thread_count_reaches_the_original() {
    let tmp = tempfile::tempdir().unwrap();
    let d = tmp.path();
    let mut args = inputs(d);
    for a in &mut args {
        if a.ends_with(".nii") || a == "out" {
            *a = d.join(&*a).to_string_lossy().into_owned();
        }
    }
    let argv: Vec<String> = [
        "larmorx",
        "--n-threads",
        "37",
        "--implementation",
        "clean-room",
    ]
    .into_iter()
    .map(str::to_owned)
    .chain(["tedana".into(), "t2smap".into()])
    .chain(args)
    .collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = larmorx_cli::run_with(&argv, &larmorx_cli::Options::default(), &mut out, &mut err);
    assert_eq!(code, 0, "{}", text(&err));
    assert!(text(&err).contains("Workflow completed"), "{}", text(&err));
    assert!(
        larmorx_core::parallel::pool_sizes().contains(&37),
        "pools {:?}",
        larmorx_core::parallel::pool_sizes()
    );
    let call = std::fs::read_to_string(d.join("out").join("t2smap_call.sh")).unwrap();
    assert!(!call.contains("--n-threads"), "{call}");
}
