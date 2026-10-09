//! The volume type every filter takes and returns.

/// A 3D image: voxels in Fortran order (`x` fastest), its size and its spacing (mm).
#[derive(Clone, Debug, PartialEq)]
pub struct Volume<T> {
    pub data: Vec<T>,
    pub size: [usize; 3],
    pub spacing: [f64; 3],
}

impl<T> Volume<T> {
    /// A volume; panics if `data` does not hold `size[0]·size[1]·size[2]` voxels.
    pub fn new(data: Vec<T>, size: [usize; 3], spacing: [f64; 3]) -> Self {
        assert_eq!(
            data.len(),
            size.iter().product::<usize>(),
            "volume size mismatch"
        );
        Volume {
            data,
            size,
            spacing,
        }
    }

    /// The linear index of voxel `(i, j, k)`.
    #[inline]
    pub fn index(&self, i: usize, j: usize, k: usize) -> usize {
        i + self.size[0] * (j + self.size[1] * k)
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// A volume with the same size and spacing and new data.
    pub fn with_data<U>(&self, data: Vec<U>) -> Volume<U> {
        Volume::new(data, self.size, self.spacing)
    }
}
