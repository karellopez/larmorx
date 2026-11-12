// SPDX-License-Identifier: Apache-2.0
//! Shared foundation of larmorx (PLAN.md §5): format-independent in-memory types.
//!
//! - [`Affine`]: 4×4 voxel-to-world transforms (RAS+ mm), with LPS conversion for ITK/ANTs.
//! - [`Image`]: an n-dimensional voxel array plus its affine; [`DynImage`] and [`DynArray`]
//!   when the element type is known only at run time.
//! - [`Element`] / [`DataType`]: the supported voxel types.
//! - [`Grid3`]: sampling grids in ITK's physical space (LPS), for tools ported from ITK.
//! - [`parallel`]: running work on an explicit number of threads.
//! - [`defects`]: D13's switch between correcting an upstream defect and reproducing it, and the
//!   notices that say when one changed a result.
//! - [`itk_real`]: ITK's two computation types (`double`, and `float` as ANTs' `--float 1`) as
//!   a type parameter, [`ItkReal`], and as a run-time choice, [`Precision`].
//! - [`linalg`], [`rotation`]: small fixed-size linear algebra and quaternions; [`vnl_svd`],
//!   [`vnl_svd_f32`]: vnl's SVD in double and single precision, bit-exact with ITK;
//!   [`vnl_eigen`]: vnl's symmetric and real eigensystems (EISPACK); [`vnl_fixed`]: vnl's
//!   closed-form determinants and inverses of small matrices; [`vnl_random`]: vnl's
//!   subtract-with-borrow random numbers (FreeSurfer's `-seed` generator).
//! - [`cscan`]: C's `sscanf` conversions `%lf` and `%d` and `strtod`, as the C library reads them.
//! - [`drand48`]: POSIX's 48-bit random numbers, as the C library gives them.
//! - [`numpy`]: numpy's sums, means, variances, percentiles and weighted averages over one axis
//!   in numpy's own order (and SciPy's `scoreatpercentile`), for replicas of Python programs.
//! - [`x87`]: the x87's 80-bit extended arithmetic (`long double` on x86-64 Linux) in software,
//!   bit-exact with the hardware, for replicas of programs that compute in `long double`.
//! - [`python`]: CPython's `argparse` (parsing, usage, help, errors), `textwrap` and number
//!   parsing and printing, for replicas of Python programs' command lines and messages.
// No unsafe code, except the audited CPU-feature dispatch in `math` and `numpy` (`dot`, `gemm`)
// (CLAUDE.md rule 5).
#![deny(unsafe_code)]

pub mod affine;
pub mod alloc;
pub mod array;
pub mod cfmt;
pub mod cli;
pub mod cscan;
pub mod defects;
pub mod drand48;
pub mod eigen_sym;
pub mod element;
pub mod grid;
pub mod image;
pub mod itk_real;
pub mod linalg;
pub mod math;
pub mod numpy;
pub mod parallel;
pub mod python;
pub mod rotation;
pub mod vnl_cholesky;
#[cfg(test)]
mod vnl_digests;
pub mod vnl_eigen;
pub mod vnl_fixed;
pub mod vnl_lbfgs;
pub mod vnl_minimize;
pub mod vnl_qr;
pub mod vnl_random;
pub mod vnl_svd;
pub mod vnl_svd_f32;
pub mod x87;

pub use affine::{Affine, SingularAffine};
pub use array::DynArray;
pub use defects::{Notice, UpstreamDefects};
pub use element::{DataType, Element, RealElement};
pub use grid::Grid3;
pub use image::{DynImage, Image, Image3, Image4};
pub use itk_real::{IndexFloor, ItkReal, Precision, same_value};
pub use ndarray;
pub use num_complex::Complex;

/// Version shared by every larmorx crate, the CLI and the Python package, in PEP 440's form, as
/// pip and PyPI show it: Cargo's `0.1.0-rc.1` is `0.1.0rc1` (build.rs, `src/pep440.rs`).
pub const VERSION: &str = env!("LARMORX_CORE_PEP440_VERSION");

#[cfg(test)]
mod pep440;

#[cfg(test)]
mod tests {
    use super::VERSION;
    use super::pep440::cargo_to_pep440;

    #[test]
    fn version_is_the_packages_in_pep440_form() {
        assert_eq!(
            cargo_to_pep440(env!("CARGO_PKG_VERSION")).as_deref(),
            Ok(VERSION)
        );
        let release = VERSION.split(['a', 'b', 'r']).next().unwrap();
        let parts: Vec<&str> = release.split('.').collect();
        assert_eq!(parts.len(), 3, "{VERSION}");
        assert!(parts.iter().all(|p| p.parse::<u64>().is_ok()), "{VERSION}");
    }

    #[test]
    fn pep440_forms() {
        for (cargo, pep440) in [
            ("0.0.1", "0.0.1"),
            ("0.1.0", "0.1.0"),
            ("0.1.0-rc.1", "0.1.0rc1"),
            ("0.2.0-alpha.3", "0.2.0a3"),
            ("1.10.0-beta.12", "1.10.0b12"),
        ] {
            assert_eq!(cargo_to_pep440(cargo).as_deref(), Ok(pep440), "{cargo}");
        }
        for refused in [
            "0.1",
            "0.1.0.1",
            "0.1.0-rc1",
            "0.1.0-rc",
            "0.1.0-dev.1",
            "0.1.0+build",
            "01.0.0",
            "0.1.0-rc.01",
            "0.1.0-",
            "x.1.0",
        ] {
            assert!(cargo_to_pep440(refused).is_err(), "{refused}");
        }
    }
}
