// SPDX-License-Identifier: Apache-2.0
//! `ImageMaskSpatialObject<D, unsigned char>` (ITK v5.4.5,
//! `Modules/Core/SpatialObjects/include/itkImageMaskSpatialObject.hxx`), as ANTs builds its
//! registration masks: a point is inside where the nearest voxel (`TransformPhysicalPointToIndex`,
//! rounding half up) lies in the image and is not 0. The object-to-world transform is the
//! identity, which maps every point to itself.

use std::sync::Arc;

use crate::Image;

/// A mask image of `unsigned char` voxels.
#[derive(Clone, Debug, PartialEq)]
pub struct Mask<const D: usize> {
    pub image: Arc<Image<u8, D>>,
}

impl<const D: usize> Mask<D> {
    pub fn new(image: Image<u8, D>) -> Self {
        Mask {
            image: Arc::new(image),
        }
    }

    /// `IsInsideInWorldSpace(point)`.
    #[inline]
    pub fn is_inside(&self, p: &[f64; D]) -> bool {
        let space = &self.image.space;
        let idx = space.point_to_index(p);
        space.contains(&idx) && self.image.at(&idx) != 0
    }
}
