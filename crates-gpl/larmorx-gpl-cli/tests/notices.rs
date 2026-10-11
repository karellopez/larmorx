// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Karel Lopez Vilaret.
//! The larmorx-gpl wheel carries the licence and notices of the Apache-2.0 larmorx crates it
//! includes (`crates-gpl/pyproject.toml`). maturin takes licence files only from below
//! `crates-gpl/`, so they are copies of the repository's `LICENSE` and `NOTICE`; this test keeps
//! them identical.

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
            "crates-gpl/{name} differs from the repository's file: copy it again"
        );
    }
}
