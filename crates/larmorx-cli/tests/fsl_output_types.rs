// SPDX-License-Identifier: Apache-2.0
//! The FSL tools' output types: FSL-09's notice names the program that ran (an integer output
//! of flirt, applyxfm4D, applytopup); and FSL-11, `FSLOUTPUTTYPE` unset, for the FSL tools'
//! clean-room originals (the standalone `larmorx` binary, run as a separate process): FSL's
//! programs stop with an error at their first image (exit 1; `standard_space_roi`'s script goes
//! on and exits 0 without an image). By default a tool that writes an image writes `NIFTI_GZ`,
//! exactly the bytes it writes with `FSLOUTPUTTYPE=NIFTI_GZ`, and its run names FSL-11 on one
//! line of its own (one also when its steps run other programs: `standard_space_roi`); with
//! `--upstream-defects reproduce`, FSL's message, no image, and FSL's exit status. A tool that
//! writes no image (or a call of one that writes none: `flirt -omat`) runs as with the variable
//! set, naming nothing, in both modes.
//!
//! On CI (every platform, the debug profile) one call per code path, a few seconds in all
//! (rule 10's budget): one tool writing one image (`fslmaths`), FLIRT's engine (`flirt
//! -applyxfm`), FAST's writer, the image names bet2's code resolves (`prelude`), a script whose
//! steps are FSL programs (`standard_space_roi`, FSL-11's wording for it), mcflirt's own code
//! (`fsl mcflirt`, `mri hmc`), the calls that write no image, mcflirt's files before it stops,
//! FSL-09's three names. Every FSL tool's calls (bet2, bet, betsurf, fslroi, applyxfm4D, topup,
//! applytopup, pairreg, ... besides these), each compared with FSL 6.0.7.23's own run, are in
//! `tests/parity/test_fsl_output_types_original.py` (this machine; the parity smoke tier, at
//! every merge), as `tests/parity/test_fsl_output_types_replica.py` for the replicas.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use larmorx_core::affine::Affine;
use larmorx_core::element::DataType;
use larmorx_core::ndarray::{ArrayD, IxDyn, ShapeBuilder};
use larmorx_io::nifti::{WriteOptions, header_for_image, write};

const EXE: &str = env!("CARGO_BIN_EXE_larmorx");

/// The variables larmorx and FSL's programs read, removed from every run here so that this
/// machine's environment cannot change a result.
const CLEARED: &[&str] = &[
    "LARMORX_IMPLEMENTATION",
    "LARMORX_UPSTREAM_DEFECTS",
    "LARMORX_NC_BIN",
    "LARMORX_GPL_BIN",
    "OMP_NUM_THREADS",
    "FSLOUTPUTTYPE",
    "FSL_LOAD_NIFTI_EXTENSIONS",
];

/// FSL's message (`specs/bet.md` §3.1).
const UNSET: &str = "ERROR:: Environment variable FSLOUTPUTTYPE is not set!";

/// The line a run of `tool` names FSL-11 with when it wrote its images as `NIFTI_GZ` in place
/// of the unset variable: what FSL's program does instead (`standard_space_roi`'s script goes on and
/// exits 0 without an image; FSL's other programs stop at their first image, exit 1).
fn fsl11_line(tool: &str) -> String {
    let fsl = if tool == "standard_space_roi" {
        "FSL's standard_space_roi writes no image (its steps stop with an error at their first \
         image) and exits 0"
            .to_owned()
    } else {
        format!("FSL's {tool} stops with an error at its first image (exit 1)")
    };
    format!(
        "larmorx: upstream defect FSL-11 (FSL) corrected: FSLOUTPUTTYPE is not set: the images \
         are written as NIFTI_GZ (.nii.gz); {fsl} [LARMORX_UPSTREAM_DEFECTS=reproduce gives \
         FSL's result]"
    )
}

/// A float32 image of `shape` with a bright block in its middle (plain arithmetic only: the
/// same bytes on every platform), its values mapped by `f`, written to `path`.
fn image(path: &Path, shape: &[usize], f: impl Fn(f32) -> f32) {
    let n: usize = shape.iter().product();
    let mut v = Vec::with_capacity(n);
    for i in 0..n {
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
        v.push(f(if inside {
            1000.0 + noise + 3.0 * t
        } else {
            noise
        }));
    }
    let data = ArrayD::from_shape_vec(IxDyn(shape).f(), v).unwrap();
    let header = header_for_image(None, None, shape, DataType::F32, &Affine::IDENTITY).unwrap();
    write(path, &header, data.view(), &WriteOptions::default()).unwrap();
}

/// Every file under `dir` (its path relative to `dir`) and its bytes.
fn files(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut all = BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                stack.push(p);
            } else {
                all.insert(
                    p.strip_prefix(dir).unwrap().to_path_buf(),
                    std::fs::read(&p).unwrap(),
                );
            }
        }
    }
    all
}

/// `larmorx --implementation clean-room [--upstream-defects reproduce] fsl|mri <args>` in `dir`
/// with [`CLEARED`] removed and `env` set: its output and the files it wrote (removed again, with
/// the directories it made, so that the next run starts from the same inputs).
fn run(
    dir: &Path,
    env: &[(&str, &str)],
    reproduce: bool,
    args: &[&str],
) -> (Output, BTreeMap<PathBuf, Vec<u8>>) {
    let before = files(dir);
    let dirs_before: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    let mut command = Command::new(EXE);
    command
        .current_dir(dir)
        .args(["--implementation", "clean-room"]);
    if reproduce {
        command.args(["--upstream-defects", "reproduce"]);
    }
    command.args(args);
    for name in CLEARED {
        command.env_remove(name);
    }
    for (k, v) in env {
        command.env(k, v);
    }
    let output = command.output().unwrap();
    let mut written = files(dir);
    written.retain(|p, _| !before.contains_key(p));
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if dirs_before.contains(&p) {
            continue;
        }
        if p.is_dir() {
            std::fs::remove_dir_all(&p).unwrap();
        } else {
            std::fs::remove_file(&p).unwrap();
        }
    }
    (output, written)
}

/// The inputs of every call: a 3D image, a 3-volume series, a 2-volume pair for topup with its
/// acquisition parameters, a phase image, an identity matrix; with `topup`, topup's results (for
/// applytopup), made with `FSLOUTPUTTYPE=NIFTI_GZ`.
fn inputs(d: &Path, topup: bool) {
    image(&d.join("in.nii"), &[12, 12, 10], |v| v);
    image(&d.join("series.nii"), &[12, 12, 10, 3], |v| v);
    image(&d.join("pair.nii"), &[12, 12, 10, 2], |v| v);
    image(&d.join("phase.nii"), &[12, 12, 10], |v| {
        v / 1100.0 * 3.0 - 1.5
    });
    std::fs::write(d.join("eye.mat"), "1 0 0 0\n0 1 0 0\n0 0 1 0\n0 0 0 1\n").unwrap();
    std::fs::write(d.join("acq.txt"), "0 1 0 0.05\n0 -1 0 0.05\n").unwrap();
    let keep = |args: &[&str]| {
        let mut c = Command::new(EXE);
        c.current_dir(d)
            .args(["--implementation", "clean-room"])
            .args(args);
        for name in CLEARED {
            c.env_remove(name);
        }
        let o = c.env("FSLOUTPUTTYPE", "NIFTI_GZ").output().unwrap();
        assert_eq!(o.status.code(), Some(0), "{args:?}: {o:?}");
    };
    if topup {
        keep(&TOPUP_FOR_APPLYTOPUP);
    }
}

const TOPUP_FOR_APPLYTOPUP: [&str; 9] = [
    "fsl",
    "topup",
    "--imain=pair.nii",
    "--datain=acq.txt",
    "--out=tu0",
    "--subsamp=1",
    "--miter=1",
    "--warpres=20",
    "--fwhm=0",
];

/// The calls that write an image, one per code path (every FSL tool's:
/// `tests/parity/test_fsl_output_types_original.py`), with the program FSL-11's line names and
/// FSL's exit status when the variable is unset.
fn image_calls() -> Vec<(&'static str, Vec<&'static str>, i32)> {
    vec![
        (
            "fslmaths",
            vec!["fsl", "fslmaths", "in.nii", "-add", "1", "m"],
            1,
        ),
        // FLIRT's engine (applyxfm4D's too)
        (
            "flirt",
            vec![
                "fsl",
                "flirt",
                "-in",
                "in.nii",
                "-ref",
                "in.nii",
                "-applyxfm",
                "-init",
                "eye.mat",
                "-out",
                "f",
            ],
            1,
        ),
        ("fast", vec!["fsl", "fast", "-o", "seg", "in.nii"], 1),
        // the image names bet2's code resolves (topup's, applytopup's and fast's inputs too)
        (
            "prelude",
            vec![
                "fsl",
                "prelude",
                "-a",
                "in.nii",
                "-p",
                "phase.nii",
                "-o",
                "pu",
            ],
            1,
        ),
        // a script whose steps are FSL programs: one line; FSL's goes on after its steps stop
        // (exit 0, no image), which FSL-11's line says
        (
            "standard_space_roi",
            vec![
                "fsl",
                "standard_space_roi",
                "in.nii",
                "ss",
                "-ssref",
                "in.nii",
                "-maskNONE",
            ],
            0,
        ),
        (
            "mcflirt",
            vec![
                "fsl",
                "mcflirt",
                "-in",
                "series.nii",
                "-out",
                "mc",
                "-plots",
                "-mats",
            ],
            1,
        ),
        // `mri hmc` is mcflirt's clean-room original: the same rule
        (
            "mcflirt",
            vec![
                "mri",
                "hmc",
                "-in",
                "series.nii",
                "-out",
                "mm",
                "-meanvol",
                "-mats",
            ],
            1,
        ),
    ]
}

#[test]
fn fsl_tools_write_nifti_gz_when_fsloutputtype_is_unset() {
    let tmp = tempfile::tempdir().unwrap();
    let d = tmp.path();
    inputs(d, false);
    for (tool, args, fsl_status) in image_calls() {
        let line = fsl11_line(tool);
        let (given, given_files) = run(d, &[("FSLOUTPUTTYPE", "NIFTI_GZ")], false, &args);
        let (unset, unset_files) = run(d, &[], false, &args);
        assert_eq!(given.status.code(), Some(0), "{args:?}: {given:?}");
        assert_eq!(unset.status.code(), Some(0), "{args:?}: {unset:?}");
        assert_eq!(given.stdout, unset.stdout, "{args:?}");
        let given_err = String::from_utf8(given.stderr).unwrap();
        let unset_err = String::from_utf8(unset.stderr).unwrap();
        assert!(!given_err.contains("FSL-11"), "{args:?}: {given_err}");
        // one line names FSL-11; without it, the messages of the run with the variable set
        let named: Vec<&str> = unset_err.lines().filter(|l| l.contains("FSL-11")).collect();
        assert_eq!(named, [line.as_str()], "{args:?}: {unset_err}");
        let others: Vec<&str> = unset_err
            .lines()
            .filter(|l| !l.contains("FSL-11"))
            .collect();
        assert_eq!(others, given_err.lines().collect::<Vec<_>>(), "{args:?}");
        // the same files, byte for byte, an image among them
        assert!(
            unset_files
                .keys()
                .any(|p| p.to_string_lossy().ends_with(".nii.gz")),
            "{args:?}: {:?}",
            unset_files.keys()
        );
        assert_eq!(
            given_files.keys().collect::<Vec<_>>(),
            unset_files.keys().collect::<Vec<_>>(),
            "{args:?}"
        );
        assert!(given_files == unset_files, "{args:?}: the bytes differ");

        // reproduced: FSL's message, no image written, FSL's exit status
        let (r, r_files) = run(d, &[], true, &args);
        let r_err = String::from_utf8(r.stderr).unwrap();
        assert_eq!(r.status.code(), Some(fsl_status), "{args:?}: {r_err}");
        assert!(r_err.contains(UNSET), "{args:?}: {r_err}");
        assert!(!r_err.contains("FSL-11"), "{args:?}: {r_err}");
        assert!(
            !r_files.keys().any(|p| {
                let p = p.to_string_lossy();
                p.ends_with(".nii.gz") || p.ends_with(".nii")
            }),
            "{args:?}: {:?}",
            r_files.keys()
        );
    }
}

#[test]
fn mcflirt_reproduced_stops_where_mcflirt_does() {
    // mcflirt writes the matrices and the .par file before the series (FSL stops at the
    // series), and -meanvol's mean image before the matrices (FSL stops there, nothing written;
    // observed with FSL 6.0.7.23, validation/investigations/easy-cli/fsl11_probe.py)
    let tmp = tempfile::tempdir().unwrap();
    let d = tmp.path();
    inputs(d, false);
    let (o, written) = run(
        d,
        &[],
        true,
        &[
            "fsl",
            "mcflirt",
            "-in",
            "series.nii",
            "-out",
            "mc",
            "-plots",
            "-mats",
        ],
    );
    assert_eq!(o.status.code(), Some(1), "{o:?}");
    let names: Vec<String> = written
        .keys()
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .collect();
    assert_eq!(
        names,
        [
            "mc.mat/MAT_0000",
            "mc.mat/MAT_0001",
            "mc.mat/MAT_0002",
            "mc.par"
        ]
    );
    let (o, written) = run(
        d,
        &[],
        true,
        &[
            "fsl",
            "mcflirt",
            "-in",
            "series.nii",
            "-out",
            "mc",
            "-meanvol",
            "-mats",
        ],
    );
    assert_eq!(o.status.code(), Some(1), "{o:?}");
    assert!(written.is_empty(), "{:?}", written.keys());
}

#[test]
fn tools_that_write_no_image_run_as_with_the_variable() {
    let tmp = tempfile::tempdir().unwrap();
    let d = tmp.path();
    inputs(d, false);
    for args in [
        // flirt without an output image: nothing where FSL would stop
        vec![
            "fsl",
            "flirt",
            "-in",
            "in.nii",
            "-ref",
            "in.nii",
            "-omat",
            "o.mat",
            "-dof",
            "6",
            "-nosearch",
        ],
        // a tool that writes no image (convert_xfm, avscale, bet2 -e -n, pairreg: the parity
        // test)
        vec!["fsl", "fslstats", "in.nii", "-M"],
    ] {
        for reproduce in [false, true] {
            let (given, given_files) = run(d, &[("FSLOUTPUTTYPE", "NIFTI_GZ")], reproduce, &args);
            let (unset, unset_files) = run(d, &[], reproduce, &args);
            assert_eq!(given.status.code(), Some(0), "{args:?}: {given:?}");
            assert_eq!(
                (given.status.code(), &given.stdout, &given.stderr),
                (unset.status.code(), &unset.stdout, &unset.stderr),
                "{args:?} reproduce={reproduce}"
            );
            assert!(given_files == unset_files, "{args:?}");
        }
    }
}

/// An int16 copy of [`image`] (plain arithmetic: the block's values are whole numbers).
fn image_i16(path: &Path, shape: &[usize]) {
    let n: usize = shape.iter().product();
    let v: Vec<i16> = (0..n)
        .map(|i| {
            let (x, y) = (i % shape[0], (i / shape[0]) % shape[1]);
            let inside = x >= shape[0] / 4 && x < shape[0] - shape[0] / 4 && y >= shape[1] / 4;
            if inside {
                1000
            } else {
                ((i * 7919) % 97) as i16
            }
        })
        .collect();
    let data = ArrayD::from_shape_vec(IxDyn(shape).f(), v).unwrap();
    let header = header_for_image(None, None, shape, DataType::I16, &Affine::IDENTITY).unwrap();
    write(path, &header, data.view(), &WriteOptions::default()).unwrap();
}

#[test]
fn fsl09_names_the_program_that_ran() {
    // integer outputs whose values a half-voxel shift or a fractional input makes fractional:
    // rounded by default, and the notice names the program that wrote them (not mcflirt)
    let tmp = tempfile::tempdir().unwrap();
    let d = tmp.path();
    inputs(d, true);
    image_i16(&d.join("s16.nii"), &[12, 12, 10, 2]);
    std::fs::write(
        d.join("shift.mat"),
        "1 0 0 0.5\n0 1 0 0\n0 0 1 0\n0 0 0 1\n",
    )
    .unwrap();
    let env = [("FSLOUTPUTTYPE", "NIFTI_GZ")];
    for (tool, args) in [
        (
            "flirt",
            vec![
                "fsl",
                "flirt",
                "-in",
                "phase.nii",
                "-ref",
                "in.nii",
                "-applyxfm",
                "-init",
                "eye.mat",
                "-out",
                "x",
                "-datatype",
                "short",
            ],
        ),
        (
            "applyxfm4D",
            vec![
                "fsl",
                "applyxfm4D",
                "s16.nii",
                "in.nii",
                "a",
                "shift.mat",
                "-singlematrix",
            ],
        ),
        (
            "applytopup",
            vec![
                "fsl",
                "applytopup",
                "--imain=phase.nii",
                "--inindex=1",
                "--datain=acq.txt",
                "--topup=tu0",
                "--out=at",
                "--method=jac",
                "--datatype=short",
            ],
        ),
    ] {
        for reproduce in [false, true] {
            let (o, _) = run(d, &env, reproduce, &args);
            assert_eq!(o.status.code(), Some(0), "{args:?}: {o:?}");
            let err = String::from_utf8(o.stderr).unwrap();
            let lines: Vec<&str> = err.lines().filter(|l| l.contains("FSL-09")).collect();
            assert_eq!(lines.len(), 1, "{args:?}: {err}");
            let (verb, says, hint) = if reproduce {
                (
                    "reproduced",
                    format!(
                        "integer output is truncated and wraps around beyond its range, as {tool} does on x86-64"
                    ),
                    "[LARMORX_UPSTREAM_DEFECTS=fix corrects it]".to_owned(),
                )
            } else {
                (
                    "corrected",
                    format!(
                        "integer output is rounded to the nearest and clamped; {tool} truncates and wraps around"
                    ),
                    format!("[LARMORX_UPSTREAM_DEFECTS=reproduce gives {tool}'s result]"),
                )
            };
            let line = lines[0];
            assert!(
                line.starts_with(&format!(
                    "larmorx: upstream defect FSL-09 ({tool}) {verb}: "
                )) && line.contains(&says)
                    && line.ends_with(&hint)
                    && !line.contains("mcflirt"),
                "{line}"
            );
        }
    }
}
