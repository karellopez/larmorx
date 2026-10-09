// SPDX-License-Identifier: Apache-2.0
//! Images: voxel arrays placed in world space by an affine.

use ndarray::{Array, Dimension, Ix3, Ix4, IxDyn};

use crate::affine::Affine;
use crate::array::DynArray;
use crate::element::{DataType, Element};

/// A voxel array and the affine that maps its first three indices to world coordinates
/// (RAS+ mm).
///
/// Data are indexed `[i, j, k, ...]` as in nibabel: `i` varies fastest on disk, so arrays
/// read from NIfTI files use Fortran (column-major) memory order. Kernels that walk memory
/// sequentially should iterate with `i` innermost, or use [`Image::as_fortran_slice`].
#[derive(Clone, Debug, PartialEq)]
pub struct Image<T, D: Dimension = IxDyn> {
    data: Array<T, D>,
    affine: Affine,
}

/// A 3D image.
pub type Image3<T> = Image<T, Ix3>;
/// A 4D image (e.g. a BOLD series: `[i, j, k, t]`).
pub type Image4<T> = Image<T, Ix4>;

impl<T, D: Dimension> Image<T, D> {
    pub fn new(data: Array<T, D>, affine: Affine) -> Self {
        Image { data, affine }
    }

    pub fn data(&self) -> &Array<T, D> {
        &self.data
    }

    pub fn data_mut(&mut self) -> &mut Array<T, D> {
        &mut self.data
    }

    pub fn affine(&self) -> &Affine {
        &self.affine
    }

    pub fn set_affine(&mut self, affine: Affine) {
        self.affine = affine;
    }

    pub fn into_parts(self) -> (Array<T, D>, Affine) {
        (self.data, self.affine)
    }

    pub fn shape(&self) -> &[usize] {
        self.data.shape()
    }

    pub fn ndim(&self) -> usize {
        self.data.ndim()
    }

    /// The first three dimensions, padded with 1 for images with fewer.
    pub fn spatial_shape(&self) -> [usize; 3] {
        let s = self.data.shape();
        [0, 1, 2].map(|i| s.get(i).copied().unwrap_or(1))
    }

    /// Voxel sizes in mm, from the affine.
    pub fn voxel_sizes(&self) -> [f64; 3] {
        self.affine.voxel_sizes()
    }

    /// The data as one slice in Fortran order, if the array is stored that way.
    pub fn as_fortran_slice(&self) -> Option<&[T]> {
        if self.data.ndim() <= 1 || self.data.t().is_standard_layout() {
            self.data.as_slice_memory_order()
        } else {
            None
        }
    }

    /// The same image with a different dimensionality type (e.g. dynamic to 3D).
    pub fn into_dimensionality<D2: Dimension>(self) -> Result<Image<T, D2>, ndarray::ShapeError> {
        Ok(Image {
            data: self.data.into_dimensionality()?,
            affine: self.affine,
        })
    }

    /// The same image with dynamic dimensionality.
    pub fn into_dyn(self) -> Image<T, IxDyn> {
        Image {
            data: self.data.into_dyn(),
            affine: self.affine,
        }
    }
}

/// An image whose element type is known only at run time.
#[derive(Clone, Debug, PartialEq)]
pub struct DynImage {
    pub data: DynArray,
    pub affine: Affine,
}

impl DynImage {
    pub fn data_type(&self) -> DataType {
        self.data.data_type()
    }

    pub fn shape(&self) -> &[usize] {
        self.data.shape()
    }

    /// The typed image, or `self` back (boxed) if it holds another element type.
    pub fn into_typed<T: Element>(self) -> Result<Image<T>, Box<DynImage>> {
        let affine = self.affine;
        match self.data.into_typed::<T>() {
            Ok(data) => Ok(Image::new(data, affine)),
            Err(data) => Err(Box::new(DynImage { data, affine })),
        }
    }
}

impl<T: Element> From<Image<T>> for DynImage {
    fn from(image: Image<T>) -> Self {
        let (data, affine) = image.into_parts();
        DynImage {
            data: data.into(),
            affine,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::{ArrayD, ShapeBuilder};

    #[test]
    fn fortran_slices_and_dimensionality() {
        let data =
            ArrayD::from_shape_vec(IxDyn(&[2, 3, 4]).f(), (0..24).map(|v| v as f32).collect())
                .unwrap();
        let img = Image::new(data, Affine::from_zooms([2.0, 2.0, 3.0], [0.0; 3]));
        assert_eq!(img.as_fortran_slice().unwrap()[1], 1.0);
        assert_eq!(img.data()[[1, 0, 0]], 1.0);
        assert_eq!(img.spatial_shape(), [2, 3, 4]);
        assert_eq!(img.voxel_sizes(), [2.0, 2.0, 3.0]);
        let img3: Image3<f32> = img.clone().into_dimensionality().unwrap();
        assert_eq!(img3.data()[[1, 2, 3]], 23.0);
        assert!(img.clone().into_dimensionality::<Ix4>().is_err());

        let c_order = ArrayD::<f32>::zeros(IxDyn(&[2, 3, 4]));
        assert!(
            Image::new(c_order, Affine::IDENTITY)
                .as_fortran_slice()
                .is_none()
        );
    }

    #[test]
    fn dyn_image_typing() {
        let img = Image::new(ArrayD::<u8>::zeros(IxDyn(&[2, 2])), Affine::IDENTITY);
        let dynamic = DynImage::from(img.clone());
        assert_eq!(dynamic.data_type(), DataType::U8);
        assert_eq!(dynamic.shape(), &[2, 2]);
        let dynamic = dynamic.into_typed::<i16>().unwrap_err();
        assert_eq!(dynamic.into_typed::<u8>().unwrap(), img);
    }
}
