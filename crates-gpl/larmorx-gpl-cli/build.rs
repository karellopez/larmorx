// SPDX-License-Identifier: GPL-3.0-or-later
// Part of larmorx-gpl (GNU GPL; see crates-gpl/LICENSE). Copyright 2026 Karel Lopez Vilaret.
// Sets `LARMORX_GPL_PEP440_VERSION`: this package's version in PEP 440's form (Cargo's
// `0.1.0-rc.1` is `0.1.0rc1`), the one pip and PyPI show for the larmorx-gpl wheel, so that
// `larmorx-gpl --version` prints it as `larmorx --version` prints larmorx's. The conversion is
// larmorx-core's (`crates/larmorx-core/src/pep440.rs`, Apache-2.0), included as it is, so the
// three programs convert alike; its golden test reads the variable too.

include!("../../crates/larmorx-core/src/pep440.rs");

fn main() {
    let cargo = std::env::var("CARGO_PKG_VERSION").expect("Cargo sets CARGO_PKG_VERSION");
    let pep440 = cargo_to_pep440(&cargo).unwrap_or_else(|e| panic!("larmorx-gpl-cli: {e}"));
    println!("cargo:rustc-env=LARMORX_GPL_PEP440_VERSION={pep440}");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../../crates/larmorx-core/src/pep440.rs");
}
