// SPDX-License-Identifier: Apache-2.0
//! larmorx's own options before the family (`--n-threads`, `--implementation`,
//! `--upstream-defects`; the user's decision of 2026-11-11): the standalone `larmorx` binary,
//! run as a separate process, gives the same output bytes with an option as with the
//! environment variable it replaces, and the option wins when both are set; in this process,
//! the thread count reaches every family's tools.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use larmorx_core::affine::Affine;
use larmorx_core::element::DataType;
use larmorx_core::ndarray::{ArrayD, IxDyn, ShapeBuilder};
use larmorx_io::nifti::{WriteOptions, header_for_image, write};

const EXE: &str = env!("CARGO_BIN_EXE_larmorx");

/// The variables larmorx and the original programs read, removed from every run here so that
/// this machine's environment cannot change a result.
const CLEARED: &[&str] = &[
    "LARMORX_IMPLEMENTATION",
    "LARMORX_UPSTREAM_DEFECTS",
    "LARMORX_NC_BIN",
    "LARMORX_GPL_BIN",
    "OMP_NUM_THREADS",
    "NSLOTS",
    "ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS",
    "ITK_NUMBER_OF_THREADS_ENV_LIST",
    "FSLOUTPUTTYPE",
];

/// A float32 image of `shape` with a bright block in its middle (plain arithmetic only: the
/// same bytes on every platform), written to `path`.
fn image(path: &Path, shape: &[usize]) {
    let n: usize = shape.iter().product();
    let mut v = Vec::with_capacity(n);
    for i in 0..n {
        // Fortran order: i fastest
        let (x, y, z) = (
            i % shape[0],
            (i / shape[0]) % shape[1],
            (i / (shape[0] * shape[1])) % shape[2],
        );
        let inside = [x, y, z]
            .iter()
            .zip(shape)
            .all(|(&c, &s)| c >= s / 4 && c < s - s / 4);
        let noise = ((i * 7919) % 97) as f32;
        let t = if shape.len() > 3 {
            (i / (shape[0] * shape[1] * shape[2])) as f32
        } else {
            0.0
        };
        v.push(if inside {
            1000.0 + noise + 3.0 * t
        } else {
            noise
        });
    }
    let data = ArrayD::from_shape_vec(IxDyn(shape).f(), v).unwrap();
    let header = header_for_image(None, None, shape, DataType::F32, &Affine::IDENTITY).unwrap();
    write(path, &header, data.view(), &WriteOptions::default()).unwrap();
}

/// A float32 image with negative values in every third voxel: ImageMath's `/` divides only
/// by positive ones (ANTS-08: corrected by default, reproduced on request).
fn image_with_zeros(path: &Path, shape: &[usize]) {
    let n: usize = shape.iter().product();
    let v: Vec<f32> = (0..n)
        .map(|i| {
            if i % 3 == 0 {
                -2.0
            } else {
                1.0 + ((i * 31) % 7) as f32
            }
        })
        .collect();
    let data = ArrayD::from_shape_vec(IxDyn(shape).f(), v).unwrap();
    let header = header_for_image(None, None, shape, DataType::F32, &Affine::IDENTITY).unwrap();
    write(path, &header, data.view(), &WriteOptions::default()).unwrap();
}

/// `larmorx <args>` in `dir`, with [`CLEARED`] removed and `env` set: its exit status, output
/// streams and the bytes of the files it wrote (`outputs`, relative to `dir`).
fn run(
    dir: &Path,
    env: &[(&str, &str)],
    args: &[&str],
    outputs: &[&str],
) -> (Output, Vec<Vec<u8>>) {
    let mut command = Command::new(EXE);
    command.current_dir(dir).args(args);
    for name in CLEARED {
        command.env_remove(name);
    }
    for (k, v) in env {
        command.env(k, v);
    }
    let output = command.output().unwrap();
    let files = outputs
        .iter()
        .map(|f| {
            let p = dir.join(f);
            let bytes =
                std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}: {output:?}", p.display()));
            std::fs::remove_file(&p).unwrap();
            bytes
        })
        .collect();
    (output, files)
}

/// The same run, once with an option and once with the variable it replaces: the same exit
/// status, standard output, standard error and output bytes. Returns the run's output.
fn same(
    dir: &Path,
    option: &[&str],
    variable: &[(&str, &str)],
    args: &[&str],
    outputs: &[&str],
) -> (Output, Vec<Vec<u8>>) {
    same_in(dir, &[], option, variable, args, outputs)
}

/// [`same`] with `base` set in both runs.
fn same_in(
    dir: &Path,
    base: &[(&str, &str)],
    option: &[&str],
    variable: &[(&str, &str)],
    args: &[&str],
    outputs: &[&str],
) -> (Output, Vec<Vec<u8>>) {
    let with_option: Vec<&str> = option.iter().chain(args).copied().collect();
    let with_variable: Vec<(&str, &str)> = base.iter().chain(variable).copied().collect();
    let (a, fa) = run(dir, base, &with_option, outputs);
    let (b, fb) = run(dir, &with_variable, args, outputs);
    assert_eq!(
        a.status.code(),
        b.status.code(),
        "{option:?} {args:?}\n{a:?}\n{b:?}"
    );
    assert_eq!(a.stdout, b.stdout, "{option:?} {args:?}");
    assert_eq!(a.stderr, b.stderr, "{option:?} {args:?}");
    assert!(fa == fb, "{option:?} {args:?}: the output files differ");
    (a, fa)
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[test]
fn ants_options_give_the_variables_bytes() {
    let tmp = tempfile::tempdir().unwrap();
    let d = tmp.path();
    image(&d.join("in.nii"), &[12, 10, 8]);
    image_with_zeros(&d.join("den.nii"), &[12, 10, 8]);
    let smooth = ["ants", "SmoothImage", "3", "in.nii", "1.5", "out.nii"];
    for (opt, var) in [
        (
            vec!["--n-threads", "3"],
            vec![("ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS", "3")],
        ),
        (vec!["--n-threads=2"], vec![("NSLOTS", "2")]),
        (vec!["--n-threads", "0"], vec![]),
    ] {
        let (o, _) = same(d, &opt, &var, &smooth, &["out.nii"]);
        assert_eq!(o.status.code(), Some(0), "{o:?}");
    }
    // ImageMath's division by zero: corrected by default, ANTs' result on request (ANTS-08)
    let divide = ["ants", "ImageMath", "3", "q.nii", "/", "in.nii", "den.nii"];
    let (_, fixed) = same(
        d,
        &["--upstream-defects", "fix"],
        &[("LARMORX_UPSTREAM_DEFECTS", "fix")],
        &divide,
        &["q.nii"],
    );
    let (_, reproduced) = same(
        d,
        &["--upstream-defects=reproduce"],
        &[("LARMORX_UPSTREAM_DEFECTS", "reproduce")],
        &divide,
        &["q.nii"],
    );
    assert!(fixed != reproduced, "ANTS-08 changes the quotient");
    // the option wins over its variable, even a wrong one
    for bad in ["reproduce", "bogus"] {
        let args: Vec<&str> = ["--upstream-defects", "fix"]
            .iter()
            .chain(&divide)
            .copied()
            .collect();
        let (o, f) = run(d, &[("LARMORX_UPSTREAM_DEFECTS", bad)], &args, &["q.nii"]);
        assert_eq!(o.status.code(), Some(0), "{o:?}");
        assert!(
            f == fixed,
            "--upstream-defects fix over LARMORX_UPSTREAM_DEFECTS={bad}"
        );
    }
    let (o, f) = run(
        d,
        &[("LARMORX_UPSTREAM_DEFECTS", "fix")],
        &[
            "--upstream-defects",
            "reproduce",
            "ants",
            "ImageMath",
            "3",
            "q.nii",
            "/",
            "in.nii",
            "den.nii",
        ],
        &["q.nii"],
    );
    assert_eq!(o.status.code(), Some(0), "{o:?}");
    assert!(f == reproduced);
    // a wrong variable alone is still refused, as before
    let (o, _) = run(d, &[("LARMORX_UPSTREAM_DEFECTS", "bogus")], &divide, &[]);
    assert_eq!(o.status.code(), Some(2), "{o:?}");
}

#[test]
fn afni_options_give_the_variables_bytes() {
    let tmp = tempfile::tempdir().unwrap();
    let d = tmp.path();
    image(&d.join("in.nii"), &[16, 16, 12]);
    image(&d.join("series.nii"), &[8, 8, 6, 8]);
    // the built-in replica (the default) and the clean-room original
    let automask = ["afni", "3dAutomask", "-prefix", "mask.nii", "in.nii"];
    for implementation in ["replica", "clean-room", "original"] {
        same(
            d,
            &["--implementation", implementation],
            &[("LARMORX_IMPLEMENTATION", implementation)],
            &automask,
            &["mask.nii"],
        );
        for n in ["1", "4", "0"] {
            let var: Vec<(&str, &str)> = if n == "0" {
                vec![]
            } else {
                vec![("OMP_NUM_THREADS", n)]
            };
            let opt = ["--implementation", implementation, "--n-threads", n];
            let args: Vec<&str> = automask.to_vec();
            let with_var: Vec<(&str, &str)> = var
                .iter()
                .copied()
                .chain([("LARMORX_IMPLEMENTATION", implementation)])
                .collect();
            let (o, _) = same(d, &opt, &with_var, &args, &["mask.nii"]);
            assert_eq!(o.status.code(), Some(0), "{o:?}");
        }
        for mode in ["fix", "reproduce"] {
            same(
                d,
                &[
                    "--implementation",
                    implementation,
                    "--upstream-defects",
                    mode,
                ],
                &[
                    ("LARMORX_IMPLEMENTATION", implementation),
                    ("LARMORX_UPSTREAM_DEFECTS", mode),
                ],
                &automask,
                &["mask.nii"],
            );
        }
    }
    // the implementation that ran is the one asked for: 3dTshift's -verbose names it
    let verbose = |opt: &[&str], env: &[(&str, &str)]| {
        let args: Vec<&str> = opt
            .iter()
            .copied()
            .chain([
                "afni",
                "3dTshift",
                "-verbose",
                "-tpattern",
                "alt+z",
                "-prefix",
                "ts.nii",
                "series.nii",
            ])
            .collect();
        let (o, _) = run(d, env, &args, &["ts.nii"]);
        assert_eq!(o.status.code(), Some(0), "{o:?}");
        text(&o.stderr) + &text(&o.stdout)
    };
    let replica = verbose(&["--implementation", "replica"], &[]);
    let clean_room = verbose(&["--implementation", "clean-room"], &[]);
    assert_ne!(replica, clean_room);
    assert_eq!(verbose(&["--implementation", "original"], &[]), clean_room);
    assert_eq!(
        verbose(&[], &[("LARMORX_IMPLEMENTATION", "clean-room")]),
        clean_room
    );
    assert_eq!(
        verbose(&[], &[("LARMORX_IMPLEMENTATION", "original")]),
        clean_room
    );
    // the option wins over its variable
    assert_eq!(
        verbose(
            &["--implementation", "clean-room"],
            &[("LARMORX_IMPLEMENTATION", "replica")]
        ),
        clean_room
    );
    assert_eq!(
        verbose(
            &["--implementation", "replica"],
            &[("LARMORX_IMPLEMENTATION", "bogus")]
        ),
        replica
    );
    assert_eq!(verbose(&[], &[]), replica, "auto runs the built-in replica");
}

#[test]
fn fsl_and_mri_options_give_the_variables_bytes() {
    let tmp = tempfile::tempdir().unwrap();
    let d = tmp.path();
    image(&d.join("in.nii"), &[12, 12, 10]);
    image(&d.join("series.nii"), &[12, 12, 10, 3]);
    let env_type = ("FSLOUTPUTTYPE", "NIFTI");
    // the clean-room originals (no larmorx-nc here: LARMORX_IMPLEMENTATION=clean-room)
    let maths = ["fsl", "fslmaths", "in.nii", "-s", "1", "out"];
    for (opt, var) in [
        (vec!["--n-threads", "3"], vec![("OMP_NUM_THREADS", "3")]),
        (vec!["--n-threads", "0"], vec![]),
    ] {
        let opt: Vec<&str> = ["--implementation", "clean-room"]
            .iter()
            .copied()
            .chain(opt)
            .collect();
        let var: Vec<(&str, &str)> = var
            .into_iter()
            .chain([env_type, ("LARMORX_IMPLEMENTATION", "clean-room")])
            .collect();
        // FSLOUTPUTTYPE in both runs
        let with_option: Vec<&str> = opt.iter().chain(&maths).copied().collect();
        let (a, fa) = run(d, &[env_type], &with_option, &["out.nii"]);
        let (b, fb) = run(d, &var, &maths, &["out.nii"]);
        assert_eq!(
            (a.status.code(), &a.stdout, &a.stderr),
            (b.status.code(), &b.stdout, &b.stderr)
        );
        assert_eq!(a.status.code(), Some(0), "{a:?}");
        assert!(fa == fb);
    }
    // mcflirt's clean-room original, `mri hmc`
    let hmc = ["mri", "hmc", "-in", "series.nii", "-out", "mc", "-plots"];
    for n in ["2", "0"] {
        let var: Vec<(&str, &str)> = if n == "0" {
            vec![]
        } else {
            vec![("OMP_NUM_THREADS", n)]
        };
        let (o, _) = same(d, &["--n-threads", n], &var, &hmc, &["mc.nii.gz", "mc.par"]);
        assert_eq!(o.status.code(), Some(0), "{o:?}");
    }
    let mcflirt = [
        "fsl",
        "mcflirt",
        "-in",
        "series.nii",
        "-out",
        "mc",
        "-plots",
    ];
    same(
        d,
        &[
            "--implementation",
            "clean-room",
            "--upstream-defects",
            "reproduce",
        ],
        &[
            ("LARMORX_IMPLEMENTATION", "clean-room"),
            ("LARMORX_UPSTREAM_DEFECTS", "reproduce"),
        ],
        &mcflirt,
        &["mc.nii.gz", "mc.par"],
    );
}

/// A stand-in for a replica's program in `dir` (`name`): it prints the variables larmorx's
/// options reach it through, and its arguments.
fn stub(dir: &Path, name: &str) -> PathBuf {
    if cfg!(windows) {
        let path = dir.join(format!("{name}.cmd"));
        let script = "@echo off\r\necho threads=%OMP_NUM_THREADS% defects=%LARMORX_UPSTREAM_DEFECTS% implementation=%LARMORX_IMPLEMENTATION% args=%*\r\n";
        std::fs::write(&path, script).unwrap();
        path
    } else {
        let path = dir.join(name);
        let script = "#!/bin/sh\necho \"threads=${OMP_NUM_THREADS-unset} defects=${LARMORX_UPSTREAM_DEFECTS-unset} implementation=${LARMORX_IMPLEMENTATION-unset} args=$*\"\n";
        std::fs::write(&path, script).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        path
    }
}

#[test]
fn replica_programs_get_the_options_in_their_environment() {
    let tmp = tempfile::tempdir().unwrap();
    let d = tmp.path();
    let nc = stub(d, "larmorx-nc");
    let gpl = stub(d, "larmorx-gpl");
    let nc_env = ("LARMORX_NC_BIN", nc.to_str().unwrap());
    let gpl_env = ("LARMORX_GPL_BIN", gpl.to_str().unwrap());
    for (env, args) in [
        (nc_env, vec!["fsl", "bet", "in", "out", "-f", "0.4"]),
        (
            gpl_env,
            vec!["tedana", "t2smap", "-d", "e1.nii", "--n-threads", "2"],
        ),
    ] {
        // the options and the variables reach the program alike
        same_in(
            d,
            &[env],
            &[
                "--n-threads",
                "5",
                "--upstream-defects",
                "reproduce",
                "--implementation",
                "replica",
            ],
            &[
                ("OMP_NUM_THREADS", "5"),
                ("LARMORX_UPSTREAM_DEFECTS", "reproduce"),
                ("LARMORX_IMPLEMENTATION", "replica"),
            ],
            &args,
            &[],
        );
        // the option wins; --n-threads 0 removes OMP_NUM_THREADS (every logical CPU)
        let opt: Vec<&str> = ["--n-threads", "0", "--upstream-defects", "fix"]
            .iter()
            .copied()
            .chain(args.iter().copied())
            .collect();
        let (o, _) = run(
            d,
            &[
                env,
                ("OMP_NUM_THREADS", "3"),
                ("LARMORX_UPSTREAM_DEFECTS", "reproduce"),
            ],
            &opt,
            &[],
        );
        let out = text(&o.stdout);
        let want = if cfg!(windows) {
            "threads=%OMP_NUM_THREADS% defects=fix"
        } else {
            "threads=unset defects=fix"
        };
        assert!(out.starts_with(want), "{out}");
        assert!(
            out.trim_end()
                .ends_with(&format!("args={}", args.join(" "))),
            "{out}"
        );
    }
}

#[test]
fn python_programs_and_fetch_take_the_options() {
    let tmp = tempfile::tempdir().unwrap();
    let d = tmp.path();
    // the standalone binary says that sdcflows' BIDS app needs the Python package, whatever the
    // options
    let (o, _) = same(
        d,
        &["--n-threads", "2", "--upstream-defects", "reproduce"],
        &[
            ("OMP_NUM_THREADS", "2"),
            ("LARMORX_UPSTREAM_DEFECTS", "reproduce"),
        ],
        &["sdcflows", "sdcflows", "--version"],
        &[],
    );
    assert_eq!(o.status.code(), Some(127));
    // fetch's usage, the same with the options
    let (o, _) = same(
        d,
        &["--n-threads", "2"],
        &[("OMP_NUM_THREADS", "2")],
        &["fetch"],
        &[],
    );
    assert_eq!(o.status.code(), Some(2));
    assert!(text(&o.stderr).contains("fetch fsl-standard"));
}

#[test]
fn wrong_options_are_usage_errors() {
    let tmp = tempfile::tempdir().unwrap();
    let d = tmp.path();
    for (args, message) in [
        (vec!["--n-threads"], "--n-threads needs a value"),
        (
            vec!["--n-threads", "-1", "ants", "SmoothImage"],
            "--n-threads must be a whole number of threads, 0 for every logical CPU, not '-1'",
        ),
        (vec!["--n-threads=", "ants"], "not ''"),
        (vec!["--n-threads", "two", "ants"], "not 'two'"),
        (
            vec!["--n-threads", "2", "--n-threads=3", "ants"],
            "--n-threads is given twice",
        ),
        (
            vec!["--implementation", "fast", "ants"],
            "--implementation must be auto, replica or clean-room, not 'fast'",
        ),
        (
            vec!["--upstream-defects=keep", "ants"],
            "--upstream-defects must be fix or reproduce, not 'keep'",
        ),
        (
            vec!["ants", "--n-threads", "2", "SmoothImage"],
            "--n-threads after the family",
        ),
    ] {
        let (o, _) = run(d, &[], &args, &[]);
        assert_eq!(o.status.code(), Some(2), "{args:?}: {o:?}");
        assert!(o.stdout.is_empty());
        let err = text(&o.stderr);
        assert!(err.starts_with("error: "), "{err}");
        assert!(err.contains(message), "{args:?}: {err}");
        assert!(err.contains("Run 'larmorx --help' for usage."), "{err}");
        if !args.first().is_some_and(|a| *a == "ants") {
            assert!(err.contains("Usage: larmorx [--n-threads N] [--implementation auto|replica|clean-room] [--upstream-defects fix|reproduce] <family> <tool>"), "{err}");
        }
    }
    // after the tool, an option is the tool's (here ImageMath's, which does not know it)
    let (o, _) = run(
        d,
        &[],
        &["ants", "ImageMath", "3", "o.nii", "--n-threads"],
        &[],
    );
    assert_ne!(o.status.code(), Some(0));
    assert!(!text(&o.stderr).starts_with("error: --n-threads"), "{o:?}");
}

#[test]
fn help_names_the_options_and_their_variables() {
    let out = Command::new(EXE)
        .args(["--n-threads", "2", "--help"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let help = text(&out.stdout);
    for line in [
        "  --n-threads N            threads, 0 = every logical CPU (default); replaces OMP_NUM_THREADS, NSLOTS\n",
        "  --implementation NAME    auto (default), replica or clean-room; replaces LARMORX_IMPLEMENTATION\n",
        "  --upstream-defects MODE  fix (default) or reproduce; replaces LARMORX_UPSTREAM_DEFECTS\n",
        "tedana\n  t2smap --n-threads",
    ] {
        assert!(help.contains(line), "{line:?} in\n{help}");
    }
}

/// In this process: the thread count `--n-threads` gives reaches each family's tools (the
/// sizes of the thread pools they started; each family gets a size no other test uses).
#[test]
fn the_thread_count_reaches_the_tools() {
    let tmp = tempfile::tempdir().unwrap();
    let d = tmp.path();
    image(&d.join("in.nii"), &[12, 12, 10]);
    image(&d.join("series.nii"), &[12, 12, 10, 3]);
    let p = |f: &str| d.join(f).to_str().unwrap().to_owned();
    let cases: [(usize, Vec<String>); 6] = [
        (
            13,
            vec![
                "ants".into(),
                "SmoothImage".into(),
                "3".into(),
                p("in.nii"),
                "1".into(),
                p("s.nii"),
            ],
        ),
        (
            17,
            vec![
                "--implementation".into(),
                "clean-room".into(),
                "afni".into(),
                "3dAutomask".into(),
                "-prefix".into(),
                p("m1.nii"),
                p("in.nii"),
            ],
        ),
        (
            19,
            vec![
                "--implementation".into(),
                "replica".into(),
                "afni".into(),
                "3dAutomask".into(),
                "-prefix".into(),
                p("m2.nii"),
                p("in.nii"),
            ],
        ),
        (
            23,
            vec![
                "--implementation".into(),
                "clean-room".into(),
                "fsl".into(),
                "fslmaths".into(),
                p("in.nii"),
                "-s".into(),
                "1".into(),
                p("f.nii.gz"),
            ],
        ),
        (
            29,
            vec![
                "mri".into(),
                "hmc".into(),
                "-in".into(),
                p("series.nii"),
                "-out".into(),
                p("mc"),
            ],
        ),
        (
            31,
            vec![
                "--implementation".into(),
                "clean-room".into(),
                "fsl".into(),
                "mcflirt".into(),
                "-in".into(),
                p("series.nii"),
                "-out".into(),
                p("mc2"),
            ],
        ),
    ];
    for (n, args) in cases {
        let mut argv = vec![
            "larmorx".to_owned(),
            "--n-threads".to_owned(),
            n.to_string(),
        ];
        argv.extend(args);
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code =
            larmorx_cli::run_with(&argv, &larmorx_cli::Options::default(), &mut out, &mut err);
        assert_eq!(code, 0, "{argv:?}: {}{}", text(&out), text(&err));
        assert!(
            larmorx_core::parallel::pool_sizes().contains(&n),
            "{argv:?}: pools {:?}",
            larmorx_core::parallel::pool_sizes()
        );
    }
}

#[test]
fn parse_globals_reads_the_options_before_the_family() {
    use larmorx_cli::replica::Implementation;
    use larmorx_cli::{Globals, parse_globals};
    use larmorx_core::defects::UpstreamDefects;
    let a = |v: &[&str]| v.iter().map(|s| (*s).to_owned()).collect::<Vec<String>>();
    assert_eq!(
        parse_globals(&a(&["ants", "--n-threads", "2"])),
        Ok((Globals::default(), 0))
    );
    let (g, k) = parse_globals(&a(&[
        "--n-threads=4",
        "--implementation",
        "clean-room",
        "--upstream-defects=reproduce",
        "fsl",
        "bet",
    ]))
    .unwrap();
    assert_eq!(k, 4);
    assert_eq!(
        g,
        Globals {
            n_threads: Some(4),
            implementation: Some(Implementation::Original),
            upstream_defects: Some(UpstreamDefects::Reproduce)
        }
    );
    assert_eq!(
        g.environment(),
        [
            ("OMP_NUM_THREADS", Some("4".to_owned())),
            ("LARMORX_UPSTREAM_DEFECTS", Some("reproduce".to_owned())),
            ("LARMORX_IMPLEMENTATION", Some("clean-room".to_owned()))
        ]
    );
    let (g, _) = parse_globals(&a(&["--implementation=original", "x"])).unwrap();
    assert_eq!(g.implementation, Some(Implementation::Original));
    // a value that looks like an option is the value
    assert!(parse_globals(&a(&["--n-threads", "--implementation", "auto"])).is_err());
    // an option that only starts like one of larmorx's is not one of them
    assert_eq!(
        parse_globals(&a(&["--n-threads4"])),
        Ok((Globals::default(), 0))
    );
    // the Python programs, after the options
    let p = larmorx_cli::python_program(&a(&[
        "larmorx",
        "--n-threads",
        "0",
        "sdcflows",
        "sdcflows",
        "bids",
        "out",
        "participant",
    ]))
    .unwrap();
    assert_eq!(p.args, a(&["bids", "out", "participant"]));
    assert_eq!(p.environment, [("OMP_NUM_THREADS", None)]);
    assert!(larmorx_cli::python_program(&a(&["larmorx", "sdcflows", "other"])).is_none());
    assert!(
        larmorx_cli::python_program(&a(&["larmorx", "--n-threads", "x", "sdcflows", "sdcflows"]))
            .is_none()
    );
    assert!(
        larmorx_cli::python_program(&a(&["larmorx", "ants", "sdcflows", "sdcflows"])).is_none()
    );
}

#[test]
fn an_unknown_option_is_still_unknown() {
    let out = Command::new(EXE)
        .args(["--n-threads", "1", "--frobnicate"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(text(&out.stderr).starts_with("error: unknown option '--frobnicate'"));
    let _ = OsStr::new("");
}
