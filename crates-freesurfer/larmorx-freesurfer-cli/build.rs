// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
// Sets `LARMORX_FREESURFER_PEP440_VERSION`: this package's version in PEP 440's form (Cargo's
// `0.1.0-rc.1` is `0.1.0rc1`), the one pip and PyPI show for the larmorx-freesurfer wheel, so that
// `larmorx-freesurfer --version` prints it as `larmorx --version` prints larmorx's. The conversion
// is larmorx-core's (`crates/larmorx-core/src/pep440.rs`, Apache-2.0), included as it is, so the
// programs convert alike.

include!("../../crates/larmorx-core/src/pep440.rs");

fn main() {
    let cargo = std::env::var("CARGO_PKG_VERSION").expect("Cargo sets CARGO_PKG_VERSION");
    let pep440 = cargo_to_pep440(&cargo).unwrap_or_else(|e| panic!("larmorx-freesurfer-cli: {e}"));
    println!("cargo:rustc-env=LARMORX_FREESURFER_PEP440_VERSION={pep440}");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../../crates/larmorx-core/src/pep440.rs");
}
