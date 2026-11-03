// SPDX-License-Identifier: Apache-2.0
//! fslmaths' kernels (`specs/fslmaths.md` §9): the current kernel of the spatial filters, which
//! `-kernel` sets and `-s` replaces; at the start the 3 × 3 × 3 box of ones, not separable.
//!
//! A kernel is a volume of single-precision values, x fastest, with its sizes as FSL computes
//! them (a size of 0 or below, or an unknown type, leaves a kernel without voxels: FSL-161); its
//! centre is voxel `(n - 1) / 2` along each axis (C's integer division).

use super::num::{self, DEFAULT_NAN_32};
use larmorx_core::math;

/// A kernel.
#[derive(Clone, Debug)]
pub struct Kernel {
    /// The sizes as computed (0 or below: no voxels).
    pub dims: [i64; 3],
    /// The values, x fastest (empty without voxels).
    pub data: Vec<f32>,
    /// Filtered separably by `-fmean` (§10.2).
    pub separable: bool,
    /// A Gaussian of zero width (FSL-160): its centre is 0/0.
    pub zero_width: bool,
}

impl Default for Kernel {
    fn default() -> Self {
        Kernel::ones([3, 3, 3], false)
    }
}

/// C's `(int)` of a single-precision value (x86-64: `INT_MIN` outside the range and for NaN).
#[inline]
fn c_int(v: f32) -> i64 {
    i64::from(num::cvtt(f64::from(v)))
}

impl Kernel {
    /// A box of ones of the given sizes.
    pub fn ones(dims: [i64; 3], separable: bool) -> Kernel {
        let n = if dims.iter().all(|&d| d > 0) {
            dims.iter().product::<i64>() as usize
        } else {
            0
        };
        Kernel {
            dims,
            data: vec![1.0; n],
            separable,
            zero_width: false,
        }
    }

    /// Whether the kernel has no voxels (FSL-161).
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// The sizes as `usize` (0 without voxels).
    pub fn size(&self) -> [usize; 3] {
        if self.is_empty() {
            [0; 3]
        } else {
            self.dims.map(|d| d as usize)
        }
    }

    /// The centre voxel, `(n - 1) / 2` per axis (truncating, as C divides).
    pub fn centre(&self) -> [i64; 3] {
        self.dims.map(|d| (d - 1) / 2)
    }

    /// The value at `(i, j, k)`.
    #[inline]
    pub fn at(&self, i: usize, j: usize, k: usize) -> f32 {
        let [nx, ny, _] = self.size();
        self.data[i + nx * (j + ny * k)]
    }

    /// `-kernel box L`: `2 ⌊L / d / 2⌋ + 1` voxels along each axis (single precision), ones.
    pub fn box_mm(l: f64, vox: [f64; 3]) -> Kernel {
        let l = l as f32;
        let dims = vox.map(|d| {
            let h = (l / d as f32 / 2.0).floor();
            2 * c_int(h) + 1
        });
        Kernel::ones(dims, true)
    }

    /// `-kernel gauss σ`: `2 ⌈4σ / d⌉ + 1` voxels along each axis; the values
    /// `expf(−(i² dx² + j² dy² + k² dz²) / (2σ²))` in single precision, not normalised.
    pub fn gauss(sigma: f64, vox: [f64; 3]) -> Kernel {
        let s = sigma as f32;
        let d = vox.map(|v| v as f32);
        let dims = d.map(|v| 2 * c_int((4.0 * s / v).ceil()) + 1);
        let mut k = Kernel::ones(dims, true);
        let den = 2.0 * s * s;
        k.zero_width = den == 0.0 && !k.is_empty();
        if k.is_empty() {
            return k;
        }
        let [nx, ny, nz] = k.size();
        let [cx, cy, cz] = k.centre();
        for z in 0..nz {
            let dk = z as i64 - cz;
            for y in 0..ny {
                let dj = y as i64 - cy;
                for x in 0..nx {
                    let di = x as i64 - cx;
                    let r2 = (di * di) as f32 * d[0] * d[0]
                        + (dj * dj) as f32 * d[1] * d[1]
                        + (dk * dk) as f32 * d[2] * d[2];
                    let q = num::nan32(-r2, den, -r2 / den);
                    let v = if q.is_nan() { q } else { math::expf(q) };
                    k.data[x + nx * (y + ny * z)] = v;
                }
            }
        }
        k
    }

    /// The kernel the convolutions use: a zero-width Gaussian (FSL-160) replaced by the unit
    /// impulse with `fix`, else as it is.
    pub fn for_convolution(&self, fix: bool) -> std::borrow::Cow<'_, Kernel> {
        if fix && self.zero_width {
            let mut k = self.clone();
            for v in &mut k.data {
                *v = if v.is_nan() { 1.0 } else { 0.0 };
            }
            k.zero_width = false;
            std::borrow::Cow::Owned(k)
        } else {
            std::borrow::Cow::Borrowed(self)
        }
    }

    /// `-kernel sphere r`: `2 round(r / d) + 1` voxels along each axis; 1 where
    /// `i² dx² + j² dy² + k² dz² ≤ r²` (single precision), else 0.
    pub fn sphere(r: f64, vox: [f64; 3]) -> Kernel {
        let r = r as f32;
        let d = vox.map(|v| v as f32);
        let dims = d.map(|v| 2 * c_int((r / v).round()) + 1);
        let mut k = Kernel::ones(dims, false);
        if k.is_empty() {
            return k;
        }
        let [nx, ny, nz] = k.size();
        let [cx, cy, cz] = k.centre();
        let r2 = r * r;
        for z in 0..nz {
            let dk = z as i64 - cz;
            for y in 0..ny {
                let dj = y as i64 - cy;
                for x in 0..nx {
                    let di = x as i64 - cx;
                    let q = (di * di) as f32 * d[0] * d[0]
                        + (dj * dj) as f32 * d[1] * d[1]
                        + (dk * dk) as f32 * d[2] * d[2];
                    k.data[x + nx * (y + ny * z)] = if q <= r2 { 1.0 } else { 0.0 };
                }
            }
        }
        k
    }

    /// `-kernel file img`: the image's first volume as single precision values.
    pub fn from_values(dims: [usize; 3], data: Vec<f32>) -> Kernel {
        Kernel {
            dims: dims.map(|d| d as i64),
            data,
            separable: false,
            zero_width: false,
        }
    }

    /// An unknown type: a kernel without voxels.
    pub fn none(separable: bool) -> Kernel {
        Kernel::ones([0, 0, 0], separable)
    }

    /// The sum of the values in double precision (storage order).
    pub fn sum64(&self) -> f64 {
        self.data.iter().fold(0.0f64, |s, &v| {
            let x = num::to_f64(v);
            num::nan64(s, x, s + x)
        })
    }

    /// Whether any value is NaN or infinite.
    pub fn non_finite(&self) -> bool {
        self.data.iter().any(|v| !v.is_finite())
    }
}

/// The default NaN in single precision (x86-64's 0/0).
pub fn default_nan32() -> f32 {
    f32::from_bits(DEFAULT_NAN_32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_follow_the_spec() {
        let v = [2.0, 2.5, 3.0];
        assert_eq!(Kernel::box_mm(24.0, v).dims, [13, 9, 9]);
        assert_eq!(Kernel::box_mm(-3.0, v).dims, [-1, -1, -1]);
        assert_eq!(Kernel::sphere(12.0, v).dims, [13, 11, 9]);
        assert_eq!(Kernel::sphere(5.0, v).dims, [7, 5, 5]);
        let g = Kernel::gauss(1.0, v);
        assert_eq!(g.dims, [5, 5, 5]);
        assert_eq!(g.at(2, 2, 2), 1.0);
        assert!(g.separable && !g.zero_width);
        let z = Kernel::gauss(0.0, v);
        assert_eq!(z.dims, [1, 1, 1]);
        assert!(z.zero_width);
        assert_eq!(z.data[0].to_bits(), DEFAULT_NAN_32);
        assert_eq!(z.for_convolution(true).data, vec![1.0]);
        assert!(Kernel::gauss(-1.0, v).is_empty());
    }
}
