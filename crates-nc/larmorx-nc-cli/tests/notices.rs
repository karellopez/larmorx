// SPDX-License-Identifier: LicenseRef-FSL
// Part of larmorx-nc (FSL Licence terms; see LICENSE). Copyright 2026 Karel Lopez Vilaret.
//! The larmorx-nc wheel carries the licence and notices of the Apache-2.0 larmorx crate it
//! includes, and FSL's original source of the replicated parts (`crates-nc/pyproject.toml`).
//! maturin takes licence files only from below `crates-nc/`, so the Apache texts are copies of
//! the repository's `LICENSE` and `NOTICE`; these tests keep them identical, and check that
//! every FSL file of `fsl-source/MANIFEST.tsv` is present and unchanged.

use std::path::Path;

use sha2::{Digest, Sha256};

#[test]
fn the_apache_notices_are_current_copies() {
    let pairs = [
        (
            "LICENSES/Apache-2.0.txt",
            include_str!("../../LICENSES/Apache-2.0.txt"),
            include_str!("../../../LICENSE"),
        ),
        (
            "LICENSES/NOTICE-larmorx.txt",
            include_str!("../../LICENSES/NOTICE-larmorx.txt"),
            include_str!("../../../NOTICE"),
        ),
    ];
    for (name, copy, original) in pairs {
        assert!(
            copy == original,
            "crates-nc/{name} differs from the repository's file: copy it again"
        );
    }
}

#[test]
fn the_fsl_source_is_complete() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../fsl-source");
    let manifest = std::fs::read_to_string(dir.join("MANIFEST.tsv")).expect("fsl-source/MANIFEST.tsv");
    let mut n = 0;
    for line in manifest.lines().skip(1) {
        let f: Vec<&str> = line.split('\t').collect();
        assert_eq!(f.len(), 4, "{line}");
        let path = dir.join(f[0]).join(f[2]);
        let data = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let digest: String = Sha256::digest(&data).iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(digest, f[3], "{} changed (scripts/fsl_source.py)", path.display());
        n += 1;
    }
    assert!(n > 0, "fsl-source/MANIFEST.tsv lists no files");
}
