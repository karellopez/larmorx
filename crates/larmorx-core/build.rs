// SPDX-License-Identifier: Apache-2.0
// Sets `larmorx_core::VERSION`: the package's version in PEP 440's form (src/pep440.rs), so that
// `larmorx --version`, `larmorx._core.__version__` and pip's version of the wheel are one string.

include!("src/pep440.rs");

fn main() {
    let cargo = std::env::var("CARGO_PKG_VERSION").expect("Cargo sets CARGO_PKG_VERSION");
    let pep440 = cargo_to_pep440(&cargo).unwrap_or_else(|e| panic!("{e}"));
    println!("cargo:rustc-env=LARMORX_CORE_PEP440_VERSION={pep440}");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/pep440.rs");
}
