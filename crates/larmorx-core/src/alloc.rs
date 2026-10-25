// SPDX-License-Identifier: Apache-2.0
//! Allocations whose size comes from the input: an output grid, a padding, a resampled image.
//!
//! Rust aborts the process when an allocation fails, which in the Python extension takes the
//! interpreter down with it. ITK throws `itk::MemoryAllocationError` instead ("Failed to
//! allocate memory for image."), which the ANTs programs do not catch: they abort. Where an
//! input can ask for more memory than the machine has (a grid in metres resampled to
//! millimetres, a spline distance of 200 mm on micrometre voxels), larmorx allocates through
//! [`filled`] and refuses with ITK's message (D13 class (a): a crash defines nothing; larmorx
//! exits 1 in both modes). Found by differential random testing (2026-10-25).

/// An allocation the machine cannot make.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("Failed to allocate memory for image ({elements} values of {element_bytes} bytes)")]
pub struct AllocError {
    pub elements: usize,
    pub element_bytes: usize,
}

/// `vec![value; n]`, or an [`AllocError`] where the memory cannot be had.
pub fn filled<T: Clone>(n: usize, value: T) -> Result<Vec<T>, AllocError> {
    let error = || AllocError {
        elements: n,
        element_bytes: std::mem::size_of::<T>(),
    };
    n.checked_mul(std::mem::size_of::<T>())
        .filter(|&bytes| bytes <= isize::MAX as usize)
        .ok_or_else(error)?;
    let mut v = Vec::new();
    v.try_reserve_exact(n).map_err(|_| error())?;
    v.resize(n, value);
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_allocations_are_filled() {
        assert_eq!(filled(4, 1.5f32).unwrap(), vec![1.5; 4]);
        assert!(filled(0, 0u8).unwrap().is_empty());
    }

    #[test]
    fn impossible_allocations_are_errors() {
        let e = filled(usize::MAX / 2, 0.0f32).unwrap_err();
        assert_eq!(e.element_bytes, 4);
        assert!(
            e.to_string()
                .starts_with("Failed to allocate memory for image")
        );
        assert!(filled(1usize << 62, 0u8).is_err());
    }
}
