// SPDX-License-Identifier: LicenseRef-FSL
// Part of larmorx-nc (FSL Licence terms; see LICENSE). Copyright 2026 Karel Lopez Vilaret.
//! FSL-11 for the replicas, `FSLOUTPUTTYPE` unset (the `larmorx-nc` program, run as a separate
//! process): FSL's programs stop with an error at their first image (exit 1), after what they
//! wrote before it. By default a tool that writes an image writes `NIFTI_GZ`, exactly the bytes
//! it writes with `FSLOUTPUTTYPE=NIFTI_GZ`, and its run names FSL-11 on one line of its own, in
//! the clean-room originals' words (one line also when the run writes several images or its
//! steps copy the settings: mcflirt); with `--upstream-defects reproduce`, FSL's message, FSL's
//! exit status and the files FSL writes before it stops. A call that writes no image runs as
//! with the variable set, naming nothing, in both modes.
//!
//! One call per code path here, small and quick for CI's six platforms; every FSL tool's image
//! calls, compared with FSL 6.0.7.23 itself and with the clean-room originals' lines, are the
//! parity test `tests/parity/test_fsl_output_types_replica.py` (this machine).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const EXE: &str = env!("CARGO_BIN_EXE_larmorx-nc");

/// The variables larmorx and FSL's programs read, removed from every run here so that this
/// machine's environment cannot change a result.
const CLEARED: &[&str] = &[
    "LARMORX_UPSTREAM_DEFECTS",
    "LARMORX_IMPLEMENTATION",
    "OMP_NUM_THREADS",
    "FSLOUTPUTTYPE",
    "FSL_LOAD_NIFTI_EXTENSIONS",
    "FSLDIR",
];

/// FSL's message, its first line.
const UNSET: &str = "ERROR:: Environment variable FSLOUTPUTTYPE is not set!";

/// The line a run of `tool` ends with when it wrote its images as `NIFTI_GZ` in place of the
/// unset variable: the clean-room originals' words, after the replica program's name.
fn fsl11_line(tool: &str) -> String {
    format!(
        "larmorx-nc: upstream defect FSL-11 (FSL) corrected: FSLOUTPUTTYPE is not set: the \
         images are written as NIFTI_GZ (.nii.gz); FSL's {tool} stops with an error at its first \
         image (exit 1) [LARMORX_UPSTREAM_DEFECTS=reproduce gives FSL's result]"
    )
}

/// A float32 NIfTI-1 image of `shape` (1 mm voxels, an identity sform) with a bright block in its
/// middle moving one voxel per volume (plain arithmetic only: the same bytes on every platform).
fn image(path: &Path, shape: &[usize]) {
    let mut h = vec![0u8; 352];
    h[0..4].copy_from_slice(&348i32.to_le_bytes());
    let mut dims = [1i16; 8];
    dims[0] = shape.len() as i16;
    for (k, &s) in shape.iter().enumerate() {
        dims[k + 1] = s as i16;
    }
    for (k, d) in dims.iter().enumerate() {
        h[40 + 2 * k..42 + 2 * k].copy_from_slice(&d.to_le_bytes());
    }
    h[70..72].copy_from_slice(&16i16.to_le_bytes());
    h[72..74].copy_from_slice(&32i16.to_le_bytes());
    for k in 0..5 {
        h[76 + 4 * k..80 + 4 * k].copy_from_slice(&1f32.to_le_bytes());
    }
    h[108..112].copy_from_slice(&352f32.to_le_bytes());
    h[112..116].copy_from_slice(&1f32.to_le_bytes());
    h[123] = 10;
    h[254..256].copy_from_slice(&1i16.to_le_bytes());
    for r in 0..3 {
        let o = 280 + 16 * r + 4 * r;
        h[o..o + 4].copy_from_slice(&1f32.to_le_bytes());
    }
    h[344..348].copy_from_slice(b"n+1\0");
    let (nx, ny, nz) = (shape[0], shape[1], shape[2]);
    let nt = shape.get(3).copied().unwrap_or(1);
    let mut i = 0usize;
    for t in 0..nt {
        for z in 0..nz {
            for y in 0..ny {
                for x in 0..nx {
                    let inside = (nx / 4 + t..nx - nx / 4 + t).contains(&x)
                        && (ny / 4..ny - ny / 4).contains(&y)
                        && (nz / 4..nz - nz / 4).contains(&z);
                    let noise = ((i * 7919) % 97) as f32;
                    let v = if inside { 1000.0 + noise } else { noise };
                    h.extend_from_slice(&v.to_le_bytes());
                    i += 1;
                }
            }
        }
    }
    std::fs::write(path, h).unwrap();
}

/// Every file under `dir` (its path relative to `dir`, `/`-separated) and its bytes.
fn files(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut all = BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                stack.push(p);
            } else {
                let name = p.strip_prefix(dir).unwrap().to_string_lossy();
                all.insert(name.replace('\\', "/"), std::fs::read(&p).unwrap());
            }
        }
    }
    all
}

/// `larmorx-nc [--upstream-defects reproduce] fsl <args>` in `dir`, one thread, with
/// [`CLEARED`] removed and `env` set: its output and the files it wrote (removed again, with the
/// directories it made, so that the next run starts from the same inputs).
fn run(
    dir: &Path,
    env: &[(&str, &str)],
    reproduce: bool,
    args: &[&str],
) -> (Output, BTreeMap<String, Vec<u8>>) {
    let before = files(dir);
    let entries_before: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    let mut command = Command::new(EXE);
    command.current_dir(dir).args(["--n-threads", "1"]);
    if reproduce {
        command.args(["--upstream-defects", "reproduce"]);
    }
    command.arg("fsl").args(args);
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
        if entries_before.contains(&p) {
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

fn inputs(d: &Path) {
    image(&d.join("in.nii"), &[12, 12, 10]);
    image(&d.join("series.nii"), &[12, 12, 10, 3]);
    std::fs::write(d.join("eye.mat"), "1 0 0 0\n0 1 0 0\n0 0 1 0\n0 0 0 1\n").unwrap();
}

#[test]
fn images_are_written_as_nifti_gz_when_fsloutputtype_is_unset() {
    let tmp = tempfile::tempdir().unwrap();
    let d = tmp.path();
    inputs(d);
    // (the tool FSL-11's line names, the call, the files FSL writes before it stops)
    let calls: [(&str, Vec<&str>, &[&str]); 3] = [
        (
            "fslmaths",
            vec!["fslmaths", "in.nii", "-add", "1", "m"],
            &[],
        ),
        // the series after the matrices and the .par file (FSL stops at the series); the
        // settings copied for the outputs
        (
            "mcflirt",
            vec![
                "mcflirt",
                "-in",
                "series.nii",
                "-out",
                "mc",
                "-mats",
                "-plots",
            ],
            &[
                "mc.mat/MAT_0000",
                "mc.mat/MAT_0001",
                "mc.mat/MAT_0002",
                "mc.par",
            ],
        ),
        // two images, one line; the mean image first (FSL stops there, nothing written)
        (
            "mcflirt",
            vec!["mcflirt", "-in", "series.nii", "-out", "mm", "-meanvol"],
            &[],
        ),
    ];
    for (tool, args, before_the_stop) in calls {
        let (given, given_files) = run(d, &[("FSLOUTPUTTYPE", "NIFTI_GZ")], false, &args);
        let (unset, unset_files) = run(d, &[], false, &args);
        assert_eq!(given.status.code(), Some(0), "{args:?}: {given:?}");
        assert_eq!(unset.status.code(), Some(0), "{args:?}: {unset:?}");
        assert_eq!(given.stdout, unset.stdout, "{args:?}");
        let given_err = String::from_utf8(given.stderr).unwrap();
        let unset_err = String::from_utf8(unset.stderr).unwrap();
        assert!(!given_err.contains("FSL-11"), "{args:?}: {given_err}");
        // one line names FSL-11, the last; the others are the run's with the variable set
        let lines: Vec<&str> = unset_err.lines().collect();
        assert_eq!(
            lines.last(),
            Some(&fsl11_line(tool).as_str()),
            "{unset_err}"
        );
        assert_eq!(
            lines[..lines.len() - 1],
            given_err.lines().collect::<Vec<_>>()[..],
            "{args:?}"
        );
        // the same files, byte for byte, images among them
        assert!(
            unset_files.keys().any(|p| p.ends_with(".nii.gz")),
            "{args:?}: {:?}",
            unset_files.keys()
        );
        assert!(given_files == unset_files, "{args:?}: the files differ");

        // reproduced: FSL's message, FSL's exit status, only what FSL writes before it stops
        let (r, r_files) = run(d, &[], true, &args);
        let r_err = String::from_utf8(r.stderr).unwrap();
        assert_eq!(r.status.code(), Some(1), "{args:?}: {r_err}");
        assert!(r_err.contains(UNSET), "{args:?}: {r_err}");
        assert!(!r_err.contains("FSL-11"), "{args:?}: {r_err}");
        assert_eq!(
            r_files.keys().map(String::as_str).collect::<Vec<_>>(),
            before_the_stop,
            "{args:?}"
        );
        // ... the same bytes as the default's
        for (name, bytes) in &r_files {
            assert!(unset_files.get(name) == Some(bytes), "{args:?}: {name}");
        }
    }
}

#[test]
fn calls_that_write_no_image_run_as_with_the_variable() {
    let tmp = tempfile::tempdir().unwrap();
    let d = tmp.path();
    inputs(d);
    for args in [
        // flirt without an output image: nothing where FSL would stop
        vec![
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
        vec!["fslstats", "in.nii", "-M"],
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

/// A variable that is set, even empty or unknown, is FSL's to judge: no default, no FSL-11.
#[test]
fn a_variable_that_is_set_is_kept() {
    let tmp = tempfile::tempdir().unwrap();
    let d = tmp.path();
    inputs(d);
    for value in ["", "NIFTI_UNKNOWN"] {
        let (o, written) = run(
            d,
            &[("FSLOUTPUTTYPE", value)],
            false,
            &["fslmaths", "in.nii", "-add", "1", "m"],
        );
        let err = String::from_utf8(o.stderr).unwrap();
        assert_eq!(o.status.code(), Some(1), "{value:?}: {err}");
        assert!(
            err.starts_with(&format!(
                "ERROR:: Unrecognised value ({value}) of environment variable FSLOUTPUTTYPE"
            )),
            "{err}"
        );
        assert!(!err.contains("FSL-11"), "{err}");
        assert!(written.is_empty(), "{:?}", written.keys());
    }
}
