//! Shared foundation of larmorx (PLAN.md §5): format-independent in-memory types.
//!
//! - [`Affine`]: 4×4 voxel-to-world transforms (RAS+ mm), with LPS conversion for ITK/ANTs.
//! - [`Image`]: an n-dimensional voxel array plus its affine; [`DynImage`] and [`DynArray`]
//!   when the element type is known only at run time.
//! - [`Element`] / [`DataType`]: the supported voxel types.
//! - [`Grid3`]: sampling grids in ITK's physical space (LPS), for tools ported from ITK.
//! - [`parallel`]: running work on an explicit number of threads.
//! - [`linalg`], [`rotation`]: small fixed-size linear algebra and quaternions.
#![forbid(unsafe_code)]

pub mod affine;
pub mod array;
pub mod element;
pub mod grid;
pub mod image;
pub mod linalg;
pub mod parallel;
pub mod rotation;

pub use affine::{Affine, SingularAffine};
pub use array::DynArray;
pub use element::{DataType, Element, RealElement};
pub use grid::Grid3;
pub use image::{DynImage, Image, Image3, Image4};
pub use ndarray;
pub use num_complex::Complex;

/// Version shared by every larmorx crate, the CLI and the Python package.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::VERSION;

    #[test]
    fn version_is_major_minor_patch() {
        let release = VERSION.split(['-', '+']).next().unwrap();
        let parts: Vec<&str> = release.split('.').collect();
        assert_eq!(parts.len(), 3, "{VERSION}");
        assert!(parts.iter().all(|p| p.parse::<u64>().is_ok()), "{VERSION}");
    }
}
