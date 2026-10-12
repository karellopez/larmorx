// SPDX-License-Identifier: Apache-2.0
//! AFNI's global options in the clean-room 3dTshift (`larmorx afni 3dTshift`, the original), on
//! every platform: `-overwrite` replacing an existing output through a temporary file and a
//! rename (Windows included, where another program may hold the output open), `-D` setting
//! AFNI's variables for the run and winning over the environment (`specs/3dTshift.md` §7.1).
//! Paths have blanks and non-ASCII characters.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use larmorx_afni::dataset::{self, BrickData, Bricks, OutputTiming};
use larmorx_afni::geometry::{InputGeometry, Source};
use larmorx_afni::settings::AfniSettings;
use larmorx_core::defects::UpstreamDefects;

/// A small 4D float32 series (seeded integer arithmetic only), written as NIfTI-1.
fn input(path: &Path) {
    let shape = [3, 2, 4, 8];
    let n: usize = shape.iter().product();
    let values: Vec<f32> = (0..n)
        .map(|i| 100.0 + ((i * 37 + 11) % 23) as f32 + (i % 8) as f32 * 0.5)
        .collect();
    let bricks = Bricks::new(shape, BrickData::Float(values), vec![0.0; 8]).unwrap();
    let geometry = InputGeometry {
        m: [
            [3.0, 0.0, 0.0, -4.0],
            [0.0, 3.0, 0.0, 2.0],
            [0.0, 0.0, 3.5, 1.0],
        ],
        source: Source::Sform,
        code: 1,
        pixdim: [3.0, 3.0, 3.5],
        mm_per_unit: 1.0,
        out_code: 1,
    };
    let timing = OutputTiming {
        tr: 2.0,
        toffset: 0.0,
        slices: None,
    };
    let (header, _, _) = dataset::output_header(
        &geometry,
        &bricks,
        &timing,
        None,
        UpstreamDefects::Fix,
        &AfniSettings::new(),
    )
    .unwrap();
    dataset::write(path, &header, &bricks, 1).unwrap();
}

/// `larmorx afni 3dTshift` (the original), with AFNI's variables of this process removed and
/// `env` set.
fn tshift(env: &[(&str, &str)], args: &[&OsStr]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_larmorx"));
    command
        .args(["afni", "3dTshift"])
        .args(args)
        .env("LARMORX_IMPLEMENTATION", "original")
        .env_remove("LARMORX_UPSTREAM_DEFECTS");
    for (key, _) in std::env::vars_os() {
        if key
            .to_string_lossy()
            .to_ascii_uppercase()
            .starts_with("AFNI_")
        {
            command.env_remove(key);
        }
    }
    for (key, value) in env {
        command.env(key, value);
    }
    command.output().unwrap()
}

/// The first four bytes: 348 for NIfTI-1, 540 for NIfTI-2.
fn sizeof_hdr(path: &Path) -> i32 {
    let bytes = std::fs::read(path).unwrap();
    i32::from_le_bytes(bytes[..4].try_into().unwrap())
}

fn names(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

struct Case {
    _tmp: tempfile::TempDir,
    dir: PathBuf,
    input: PathBuf,
    out: PathBuf,
}

fn case() -> Case {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("dir with blank é");
    std::fs::create_dir(&dir).unwrap();
    let input_path = dir.join("in.nii");
    input(&input_path);
    let out = dir.join("out put é.nii");
    std::fs::write(&out, b"taken").unwrap();
    Case {
        _tmp: tmp,
        dir,
        input: input_path,
        out,
    }
}

#[test]
fn overwrite_and_defines_on_every_platform() {
    let c = case();
    let (out, input) = (c.out.as_os_str(), c.input.as_os_str());
    fn tz<'a>(rest: &[&'a OsStr]) -> Vec<&'a OsStr> {
        let mut v: Vec<&OsStr> = vec!["-tpattern".as_ref(), "alt+z".as_ref()];
        v.extend_from_slice(rest);
        v
    }
    let text = |b: &[u8]| String::from_utf8_lossy(b).into_owned();

    // By default an existing output is an error, before the work (AFNI-11 corrected).
    let o = tshift(&[], &tz(&["-prefix".as_ref(), out, input]));
    assert_eq!(o.status.code(), Some(1), "{o:?}");
    assert!(
        text(&o.stderr).contains("conflicts with existing file"),
        "{o:?}"
    );
    assert!(text(&o.stderr).contains("upstream defect AFNI-11"), "{o:?}");
    assert_eq!(std::fs::read(out).unwrap(), b"taken");
    // AFNI's behaviour: nothing written, exit 0 (AFNI also needs to be told that names with
    // blanks are allowed: AFNI-12, AFNI-18).
    let o = tshift(
        &[
            ("LARMORX_UPSTREAM_DEFECTS", "reproduce"),
            ("AFNI_PATH_SPACES_OK", "YES"),
        ],
        &tz(&[
            "-DAFNI_ALLOW_ARBITRARY_FILENAMES=YES".as_ref(),
            "-prefix".as_ref(),
            out,
            input,
        ]),
    );
    assert_eq!(o.status.code(), Some(0), "{o:?}");
    assert!(
        text(&o.stderr).contains("dataset NOT written to disk!"),
        "{o:?}"
    );
    assert_eq!(std::fs::read(out).unwrap(), b"taken");

    // -overwrite, after the dataset: replaced whole, no temporary file left.
    let o = tshift(
        &[],
        &tz(&["-prefix".as_ref(), out, input, "-overwrite".as_ref()]),
    );
    assert!(o.status.success(), "{o:?}");
    assert_eq!(sizeof_hdr(&c.out), 348);
    assert_eq!(names(&c.dir), ["in.nii", "out put é.nii"]);

    // -D sets AFNI's variables and wins over the environment: renamed to _AA1.
    std::fs::write(out, b"taken").unwrap();
    let o = tshift(
        &[("AFNI_DECONFLICT", "NO")],
        &tz(&[
            "-DAFNI_DECONFLICT=YES".as_ref(),
            "-prefix".as_ref(),
            out,
            input,
        ]),
    );
    assert!(o.status.success(), "{o:?}");
    assert!(
        text(&o.stderr)
            .contains("changed output dataset name from 'out put é.nii' to 'out put é_AA1.nii'"),
        "{o:?}"
    );
    assert_eq!(std::fs::read(out).unwrap(), b"taken");
    assert_eq!(sizeof_hdr(&c.dir.join("out put é_AA1.nii")), 348);

    // -DAFNI_NIFTI_WRITE_TYPE=2 with -overwrite in the place of an option's value: NIfTI-2.
    let o = tshift(
        &[],
        &[
            "-tpattern".as_ref(),
            "-overwrite".as_ref(),
            "alt+z".as_ref(),
            "-DAFNI_NIFTI_WRITE_TYPE=2".as_ref(),
            "-prefix".as_ref(),
            out,
            input,
        ],
    );
    assert!(o.status.success(), "{o:?}");
    assert_eq!(sizeof_hdr(&c.out), 540);

    // A -D of a variable 3dTshift does not read: applied, with larmorx's warning.
    let o = tshift(
        &[],
        &tz(&[
            "-DAFNI_NOT_READ=1".as_ref(),
            "-overwrite".as_ref(),
            "-prefix".as_ref(),
            out,
            input,
        ]),
    );
    assert!(o.status.success(), "{o:?}");
    assert!(
        text(&o.stderr).contains("3dTshift does not read AFNI_NOT_READ (larmorx)"),
        "{o:?}"
    );
}

/// On Windows, renaming over a file that another program holds open without allowing its
/// deletion fails; the rename is tried again for about 2 s. Here the output is held for
/// 0.5 s: the run waits, then replaces it.
#[cfg(windows)]
#[test]
fn overwrite_waits_for_a_held_output() {
    use std::os::windows::fs::OpenOptionsExt;
    let c = case();
    const FILE_SHARE_READ: u32 = 1;
    let held = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ)
        .open(&c.out)
        .unwrap();
    let release = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(500));
        drop(held);
    });
    let o = tshift(
        &[],
        &[
            "-overwrite".as_ref(),
            "-tpattern".as_ref(),
            "alt+z".as_ref(),
            "-prefix".as_ref(),
            c.out.as_os_str(),
            c.input.as_os_str(),
        ],
    );
    release.join().unwrap();
    assert!(o.status.success(), "{o:?}");
    assert_eq!(sizeof_hdr(&c.out), 348);
    assert_eq!(names(&c.dir), ["in.nii", "out put é.nii"]);
}
