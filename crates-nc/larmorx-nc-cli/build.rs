// SPDX-License-Identifier: LicenseRef-FSL
// Part of larmorx-nc (FSL Licence terms; see LICENSE). Copyright 2026 Karel Lopez Vilaret.
// Sets `LARMORX_NC_PEP440_VERSION`: this package's version in PEP 440's form (Cargo's
// `0.1.0-rc.1` is `0.1.0rc1`), the one pip and PyPI show for the larmorx-nc wheel, so that
// `larmorx-nc --version` prints it as `larmorx --version` prints larmorx's. The conversion is
// larmorx-core's (`crates/larmorx-core/src/pep440.rs`, Apache-2.0), included as it is, so the
// three programs convert alike; its golden test reads the variable too.

include!("../../crates/larmorx-core/src/pep440.rs");

fn main() {
    let cargo = std::env::var("CARGO_PKG_VERSION").expect("Cargo sets CARGO_PKG_VERSION");
    let pep440 = cargo_to_pep440(&cargo).unwrap_or_else(|e| panic!("larmorx-nc-cli: {e}"));
    println!("cargo:rustc-env=LARMORX_NC_PEP440_VERSION={pep440}");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../../crates/larmorx-core/src/pep440.rs");
}
