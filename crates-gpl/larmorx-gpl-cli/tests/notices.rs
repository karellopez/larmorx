// SPDX-License-Identifier: GPL-3.0-or-later
// Part of larmorx-gpl (GNU GPL; see crates-gpl/LICENSE). Copyright 2026 Karel Lopez Vilaret.
//! The larmorx-gpl wheel carries the licence and notices of the Apache-2.0 larmorx crates it
//! includes and tedana's own licence text (`crates-gpl/pyproject.toml`). maturin takes licence
//! files only from below `crates-gpl/`, so the Apache texts are copies of the repository's
//! `LICENSE` and `NOTICE`; these tests keep them identical, and tedana's text as tedana 26.0.3
//! ships it.

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
            "crates-gpl/{name} differs from the repository's file: copy it again"
        );
    }
}

#[test]
fn tedanas_licence_is_its_own_text() {
    // tedana 26.0.3's LICENSE (the GNU LGPL version 2.1), byte for byte (also 25.0.1's)
    let text = include_bytes!("../../LICENSES/LGPL-2.1-tedana.txt");
    let digest: String = Sha256::digest(text)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!(
        digest,
        "d716613abed076e2db04e79e7d402d69a10bd57c0d990321a568db528b5795ad"
    );
    // the program's notice names the conversion
    let notice = include_str!("../../NOTICE");
    assert!(notice.contains("section 3"), "crates-gpl/NOTICE");
    assert!(notice.contains("Copyright 2023, The ME-ICA Developers"));
}
