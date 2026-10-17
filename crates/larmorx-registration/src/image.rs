// SPDX-License-Identifier: Apache-2.0
//! An image: voxels (x fastest) on a [`Space`].

use crate::Space;

/// `itk::Image<T, D>`: voxels in Fortran order on a geometry.
#[derive(Clone, Debug, PartialEq)]
pub struct Image<T, const D: usize> {
    pub space: Space<D>,
    pub data: Vec<T>,
}

impl<T: Copy, const D: usize> Image<T, D> {
    /// An image; panics if `data` does not match the size.
    pub fn new(space: Space<D>, data: Vec<T>) -> Self {
        assert_eq!(space.len(), data.len(), "voxel count mismatch");
        Image { space, data }
    }

    /// `GetPixel(index)` (inside the region).
    #[inline]
    pub fn at(&self, idx: &[i64; D]) -> T {
        self.data[self.space.offset(idx)]
    }

    /// The same geometry with other voxels.
    pub fn with_data<U: Copy>(&self, data: Vec<U>) -> Image<U, D> {
        Image::new(self.space.clone(), data)
    }
}
