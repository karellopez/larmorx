// SPDX-License-Identifier: Apache-2.0
//! The image types the spatial filters take and return.
//!
//! ITK's filters are `D`-dimensional: an `itk::Image<float, 4>` is smoothed along time too,
//! and a 2D image is not a 3D image with one slice (a neighbourhood never reaches outside its
//! dimensions). So a volume here has any number of dimensions, as many as the ITK image it
//! stands for.

/// A `D`-dimensional image: voxels in Fortran order (the first axis fastest), its size and
/// its spacing (mm, and seconds for a fourth axis read from NIfTI).
#[derive(Clone, Debug, PartialEq)]
pub struct Volume<T> {
    pub data: Vec<T>,
    pub size: Vec<usize>,
    pub spacing: Vec<f64>,
}

impl<T> Volume<T> {
    /// A volume; panics if `data` does not hold `size[0]·size[1]·…` voxels or if `spacing`
    /// has another length than `size`.
    pub fn new(data: Vec<T>, size: &[usize], spacing: &[f64]) -> Self {
        check(data.len(), size, spacing);
        Volume {
            data,
            size: size.to_vec(),
            spacing: spacing.to_vec(),
        }
    }

    /// The number of dimensions.
    pub fn dim(&self) -> usize {
        self.size.len()
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// A borrowed view of this volume.
    pub fn view(&self) -> VolumeRef<'_, T> {
        VolumeRef {
            data: &self.data,
            size: &self.size,
            spacing: &self.spacing,
        }
    }

    /// A volume with the same size and spacing and new data.
    pub fn with_data<U>(&self, data: Vec<U>) -> Volume<U> {
        Volume::new(data, &self.size, &self.spacing)
    }
}

/// A borrowed `D`-dimensional image: what the spatial filters read.
#[derive(Debug, PartialEq)]
pub struct VolumeRef<'a, T> {
    pub data: &'a [T],
    pub size: &'a [usize],
    pub spacing: &'a [f64],
}

impl<T> Clone for VolumeRef<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for VolumeRef<'_, T> {}

impl<'a, T> VolumeRef<'a, T> {
    /// A view; panics on the same mismatches as [`Volume::new`].
    pub fn new(data: &'a [T], size: &'a [usize], spacing: &'a [f64]) -> Self {
        check(data.len(), size, spacing);
        VolumeRef {
            data,
            size,
            spacing,
        }
    }

    /// The number of dimensions.
    pub fn dim(&self) -> usize {
        self.size.len()
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// The distance in memory between neighbours along each axis.
    pub fn strides(&self) -> Vec<usize> {
        strides(self.size)
    }
}

/// The distance in memory between neighbours along each axis of an image of `size`
/// (Fortran order).
pub fn strides(size: &[usize]) -> Vec<usize> {
    let mut out = Vec::with_capacity(size.len());
    let mut s = 1;
    for &n in size {
        out.push(s);
        s *= n;
    }
    out
}

fn check(len: usize, size: &[usize], spacing: &[f64]) {
    assert_eq!(len, size.iter().product::<usize>(), "volume size mismatch");
    assert_eq!(size.len(), spacing.len(), "spacing has the wrong length");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn views_and_strides() {
        let v = Volume::new(vec![0u8; 24], &[4, 3, 2], &[1.0, 2.0, 3.0]);
        assert_eq!(v.dim(), 3);
        assert_eq!(v.view().strides(), [1, 4, 12]);
        assert_eq!(strides(&[5, 2]), [1, 5]);
        let w = v.with_data(vec![1.0f32; 24]);
        assert_eq!(w.view().spacing, [1.0, 2.0, 3.0]);
    }

    #[test]
    #[should_panic(expected = "volume size mismatch")]
    fn size_mismatch_panics() {
        let _ = Volume::new(vec![0u8; 5], &[2, 2], &[1.0, 1.0]);
    }
}
