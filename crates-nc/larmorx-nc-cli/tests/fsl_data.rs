// SPDX-License-Identifier: LicenseRef-FSL
// Part of larmorx-nc (FSL Licence terms; see LICENSE). Copyright 2026 Karel Lopez Vilaret.
//! FSL's standard images the larmorx-nc wheel ships (the user's decision of 2026-11-04,
//! `crates-nc/wheel-data/data/share/larmorx-nc/fsl-data/README.md`): the committed copy holds
//! exactly the images of its `MANIFEST.tsv`, unchanged, with FSL's Licence and MNI's notice;
//! they are the images the replicated scripts ask for; maturin installs the directory as the
//! wheel's data; and the program finds them beside itself, without any FSL installation.

use std::collections::BTreeSet;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

use larmorx_nc_core::fsldata::{self, BUNDLED_IMAGES, Source};
use larmorx_nc_core::settings::Settings;
use sha2::{Digest, Sha256};

const EXE: &str = env!("CARGO_BIN_EXE_larmorx-nc");

fn crates_nc() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

/// The committed copy, as the wheel installs it (`<prefix>/share/larmorx-nc/fsl-data`).
fn copy_dir() -> PathBuf {
    crates_nc().join("wheel-data/data/share/larmorx-nc/fsl-data")
}

/// Every file below `dir`, relative, with `/`.
fn files_below(dir: &Path, rel: &str, out: &mut BTreeSet<String>) {
    for e in std::fs::read_dir(dir).unwrap() {
        let e = e.unwrap();
        let name = e.file_name().to_string_lossy().into_owned();
        let r = if rel.is_empty() {
            name
        } else {
            format!("{rel}/{name}")
        };
        if e.file_type().unwrap().is_dir() {
            files_below(&e.path(), &r, out);
        } else {
            out.insert(r);
        }
    }
}

#[test]
fn the_copy_is_the_manifest_unchanged() {
    let dir = copy_dir();
    let manifest = std::fs::read_to_string(dir.join("MANIFEST.tsv")).expect("MANIFEST.tsv");
    let mut lines = manifest.lines();
    assert_eq!(
        lines.next(),
        Some("file\tbytes\tsha256\tpackage\tversion\tread_by")
    );
    let mut listed = BTreeSet::new();
    for line in lines {
        let f: Vec<&str> = line.split('\t').collect();
        assert_eq!(f.len(), 6, "{line}");
        assert_eq!(
            (f[3], f[4]),
            ("fsl-data_standard", "2208.0"),
            "{line}: FSL 6.0.7.23's data package"
        );
        let data = std::fs::read(dir.join(f[0])).unwrap_or_else(|e| panic!("{}: {e}", f[0]));
        assert_eq!(data.len().to_string(), f[1], "{}: size", f[0]);
        let digest: String = Sha256::digest(&data)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        assert_eq!(digest, f[2], "{} changed", f[0]);
        listed.insert(f[0].to_owned());
    }
    // the images the scripts read, and no other
    let images: BTreeSet<String> = BUNDLED_IMAGES
        .iter()
        .map(|r| format!("{r}.nii.gz"))
        .collect();
    assert_eq!(listed, images);
    let mut present = BTreeSet::new();
    files_below(&dir, "", &mut present);
    let mut expected = images;
    for f in ["LICENSE", "NOTICE", "README.md", "MANIFEST.tsv"] {
        expected.insert(f.to_owned());
    }
    assert_eq!(present, expected, "the copy holds exactly these files");
    // FSL's Licence (crates-nc/LICENSE) and MNI's notice
    let licence = std::fs::read(dir.join("LICENSE")).unwrap();
    assert!(
        licence == std::fs::read(crates_nc().join("LICENSE")).unwrap(),
        "fsl-data/LICENSE differs from crates-nc/LICENSE: copy it again"
    );
    let notice = std::fs::read_to_string(dir.join("NOTICE")).unwrap();
    for s in [
        "Copyright (C) 1993-2009 Louis Collins, McConnell Brain Imaging Centre, Montreal",
        "fsl-data_standard 2208.0",
        "2faefb4d366b6427cbf4e5c3e54c63c040ec9bcfe11b1f10848f89a1571b828a",
    ] {
        assert!(notice.contains(s), "NOTICE lacks {s:?}");
    }
    // the wheel installs the directory as its data, and carries the notice as a licence file
    let pyproject = std::fs::read_to_string(crates_nc().join("pyproject.toml")).unwrap();
    assert!(
        pyproject.contains("\ndata = \"wheel-data\"\n"),
        "pyproject.toml"
    );
    assert!(
        pyproject.contains("\"wheel-data/data/share/larmorx-nc/fsl-data/NOTICE\""),
        "pyproject.toml"
    );
    // maturin accepts only the wheel's schemes as the data directory's entries
    let mut top = BTreeSet::new();
    for e in std::fs::read_dir(crates_nc().join("wheel-data")).unwrap() {
        top.insert(e.unwrap().file_name().to_string_lossy().into_owned());
    }
    assert_eq!(top, BTreeSet::from(["data".to_owned()]));
}

/// The copy answers every image the scripts ask for, with no FSL installation.
#[test]
fn the_scripts_images_are_found_in_the_copy() {
    let s = Settings::new();
    for rel in BUNDLED_IMAGES {
        let got = fsldata::find_in(rel, &s, Some(copy_dir())).unwrap();
        assert_eq!(got.source, Source::Bundled, "{rel}");
    }
}

/// A 8 × 8 × 8 float32 image of 8 mm voxels (a block), as `.nii`.
fn small(dir: &Path) {
    let n = 8usize;
    let mut h = vec![0u8; 352];
    h[0..4].copy_from_slice(&348i32.to_le_bytes());
    let dims: [i16; 8] = [3, n as i16, n as i16, n as i16, 1, 1, 1, 1];
    for (k, d) in dims.iter().enumerate() {
        h[40 + 2 * k..42 + 2 * k].copy_from_slice(&d.to_le_bytes());
    }
    h[70..72].copy_from_slice(&16i16.to_le_bytes());
    h[72..74].copy_from_slice(&32i16.to_le_bytes());
    for (k, p) in [1.0f32, 8.0, 8.0, 8.0].iter().enumerate() {
        h[76 + 4 * k..80 + 4 * k].copy_from_slice(&p.to_le_bytes());
    }
    h[108..112].copy_from_slice(&352f32.to_le_bytes());
    h[112..116].copy_from_slice(&1f32.to_le_bytes());
    h[344..348].copy_from_slice(b"n+1\0");
    for z in 0..n {
        for y in 0..n {
            for x in 0..n {
                let inside = [x, y, z].iter().all(|&c| (2..6).contains(&c));
                let v: f32 = if inside { 100.0 } else { (x + y + z) as f32 };
                h.extend_from_slice(&v.to_le_bytes());
            }
        }
    }
    std::fs::write(dir.join("in.nii"), h).unwrap();
    std::fs::write(dir.join("eye.mat"), "1 0 0 0\n0 1 0 0\n0 0 1 0\n0 0 0 1\n").unwrap();
}

/// `dim1..dim3` of a `.nii.gz`.
fn dims(path: &Path) -> [i16; 3] {
    let mut h = [0u8; 348];
    flate2::read::GzDecoder::new(std::fs::File::open(path).unwrap())
        .read_exact(&mut h)
        .unwrap();
    let d = |k: usize| i16::from_le_bytes([h[40 + 2 * k], h[41 + 2 * k]]);
    [d(1), d(2), d(3)]
}

/// The program in an installation's layout (`<prefix>/bin`, the copy in
/// `<prefix>/share/larmorx-nc/fsl-data`) finds the images without `FSLDIR`: standard_space_roi
/// `-b` on a small image, FLIRT given the identity (`-applyxfm -init`, the script's FLIRT
/// options) so that it only resamples onto MNI152_T1_2mm's grid; `-d` keeps the dilated brain
/// mask it read. Without the copy it refuses, naming the image.
#[test]
fn the_program_finds_the_copy_beside_itself() {
    let root = tempfile::tempdir().unwrap();
    let prefix = root.path().join("prefix ~1");
    let bin = prefix.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let exe = bin.join(Path::new(EXE).file_name().unwrap());
    if std::fs::hard_link(EXE, &exe).is_err() {
        std::fs::copy(EXE, &exe).unwrap();
    }
    let work = root.path().join("work");
    std::fs::create_dir_all(&work).unwrap();
    small(&work);
    let run = || {
        let o = Command::new(&exe)
            .args([
                "fsl",
                "standard_space_roi",
                "in",
                "out",
                "-b",
                "-d",
                "-applyxfm",
                "-init",
                "eye.mat",
            ])
            .current_dir(&work)
            .env_remove("FSLDIR")
            .env_remove(fsldata::ENV)
            .env_remove("LARMORX_UPSTREAM_DEFECTS")
            .env("FSLOUTPUTTYPE", "NIFTI_GZ")
            .env("OMP_NUM_THREADS", "1")
            .output()
            .unwrap();
        (
            o.status.code(),
            String::from_utf8_lossy(&o.stderr).into_owned(),
        )
    };
    let (code, stderr) = run();
    assert_eq!(code, Some(1), "{stderr}");
    assert!(
        stderr.starts_with(
            "larmorx-nc: error: standard_space_roi: FSL's data image standard/MNI152_T1_2mm \
             was not found: looked in FSLDIR (not set), larmorx-nc's copy (not installed"
        ),
        "{stderr}"
    );
    assert!(!work.join("out.nii.gz").exists());
    // the copy, as the wheel installs it
    let to = prefix.join("share/larmorx-nc/fsl-data");
    let mut files = BTreeSet::new();
    files_below(&copy_dir(), "", &mut files);
    for f in &files {
        let dst = to.join(f);
        std::fs::create_dir_all(dst.parent().unwrap()).unwrap();
        std::fs::copy(copy_dir().join(f), dst).unwrap();
    }
    let (code, stderr) = run();
    assert_eq!((code, stderr.as_str()), (Some(0), ""));
    assert_eq!(dims(&work.join("out.nii.gz")), [8, 8, 8]);
    assert_eq!(dims(&work.join("out_tmp_to_std.nii.gz")), [91, 109, 91]);
    assert_eq!(
        dims(&work.join("out_tmp_standard_mask.nii.gz")),
        [91, 109, 91]
    );
}
