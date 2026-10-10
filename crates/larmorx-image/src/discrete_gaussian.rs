// SPDX-License-Identifier: Apache-2.0
//! Discrete Gaussian smoothing, as ITK v5.4.5 computes it for `float` images
//! (`DiscreteGaussianImageFilter`, with the kernels of `GaussianOperator`).
//!
//! The kernel along each axis is the sampled Gaussian of Lindeberg's discrete scale space,
//! `e^{-t} I_k(t)` for a variance `t` in voxels², built from the modified Bessel functions
//! `I_0`, `I_1` and `I_k` (Numerical Recipes' polynomial approximations and downward
//! recurrence, as `GaussianOperator` computes them), until the kernel holds `1 − maximum_error`
//! of the mass or reaches its maximum width, then normalised to sum 1.
//!
//! The image is convolved axis by axis, **the last axis first**, each pass `float → float`
//! with the sum in double, summed from the kernel's first tap to its last; outside the image
//! the nearest edge voxel is used (`ZeroFluxNeumannBoundaryCondition`).

use larmorx_core::math;

use crate::FilterError;
use crate::lines::{LineFilter, filter_lines};
use crate::volume::VolumeRef;

/// `GaussianOperator::ModifiedBesselI0`.
fn bessel_i0(y: f64) -> f64 {
    let d = y.abs();
    if d < 3.75 {
        let mut m = y / 3.75;
        m *= m;
        1.0 + m
            * (3.5156229
                + m * (3.0899424
                    + m * (1.2067492 + m * (0.2659732 + m * (0.360768e-1 + m * 0.45813e-2)))))
    } else {
        let m = 3.75 / d;
        (math::exp(d) / d.sqrt())
            * (0.39894228
                + m * (0.1328592e-1
                    + m * (0.225319e-2
                        + m * (-0.157565e-2
                            + m * (0.916281e-2
                                + m * (-0.2057706e-1
                                    + m * (0.2635537e-1
                                        + m * (-0.1647633e-1 + m * 0.392377e-2))))))))
    }
}

/// `GaussianOperator::ModifiedBesselI1`.
fn bessel_i1(y: f64) -> f64 {
    let d = y.abs();
    let accumulator = if d < 3.75 {
        let mut m = y / 3.75;
        m *= m;
        d * (0.5
            + m * (0.87890594
                + m * (0.51498869
                    + m * (0.15084934 + m * (0.2658733e-1 + m * (0.301532e-2 + m * 0.32411e-3))))))
    } else {
        let m = 3.75 / d;
        let mut a = 0.2282967e-1 + m * (-0.2895312e-1 + m * (0.1787654e-1 - m * 0.420059e-2));
        a = 0.39894228
            + m * (-0.3988024e-1
                + m * (-0.362018e-2 + m * (0.163801e-2 + m * (-0.1031555e-1 + m * a))));
        a * (math::exp(d) / d.sqrt())
    };
    if y < 0.0 { -accumulator } else { accumulator }
}

/// `GaussianOperator::ModifiedBesselI` for `n ≥ 2` (Miller's downward recurrence).
fn bessel_i(n: i32, y: f64) -> f64 {
    const ACCURACY: f64 = 40.0;
    if y == 0.0 {
        return 0.0;
    }
    let toy = 2.0 / y.abs();
    let mut qip = 0.0f64;
    let mut accumulator = 0.0f64;
    let mut qi = 1.0f64;
    let mut j = 2 * (n + (ACCURACY * f64::from(n)).sqrt() as i32);
    while j > 0 {
        let qim = qip + f64::from(j) * toy * qi;
        qip = qi;
        qi = qim;
        if qi.abs() > 1.0e10 {
            accumulator *= 1.0e-10;
            qi *= 1.0e-10;
            qip *= 1.0e-10;
        }
        if j == n {
            accumulator = qip;
        }
        j -= 1;
    }
    accumulator *= bessel_i0(y) / qi;
    if y < 0.0 && (n & 1) == 1 {
        -accumulator
    } else {
        accumulator
    }
}

/// The full symmetric kernel of `GaussianOperator` for `variance` (in voxels²): taps
/// `−r..=r`, normalised to sum 1 (`GenerateCoefficients`). Taps are added while the kernel
/// holds less than `1 − maximum_error` of the mass, up to `r = maximum_kernel_width`.
pub fn gaussian_kernel(variance: f64, maximum_error: f64, maximum_kernel_width: usize) -> Vec<f64> {
    let et = math::exp(-variance);
    let cap = 1.0 - maximum_error;
    let mut coeff = vec![et * bessel_i0(variance)];
    let mut sum = coeff[0];
    coeff.push(et * bessel_i1(variance));
    sum += coeff[1] * 2.0;
    let mut i = 2;
    while sum < cap {
        let c = et * bessel_i(i, variance);
        coeff.push(c);
        sum += c * 2.0;
        if c <= 0.0 {
            break;
        }
        if coeff.len() > maximum_kernel_width {
            break;
        }
        i += 1;
    }
    for c in &mut coeff {
        *c /= sum;
    }
    // Make symmetric: the reversed tail, then the centre and the tail.
    let mut kernel: Vec<f64> = coeff[1..].iter().rev().copied().collect();
    kernel.extend_from_slice(&coeff);
    kernel
}

/// A 1D convolution with a symmetric kernel, nearest-edge outside the line.
struct Convolution {
    kernel: Vec<f64>,
}

impl LineFilter for Convolution {
    fn filter(&self, input: &[f64], output: &mut [f64], _: &mut [f64], n: usize, w: usize) {
        let r = (self.kernel.len() / 2) as isize;
        let last = n as isize - 1;
        for x in 0..n {
            let row = &mut output[x * w..(x + 1) * w];
            row.fill(0.0);
            for (k, &c) in self.kernel.iter().enumerate() {
                let i = (x as isize + k as isize - r).clamp(0, last) as usize;
                let src = &input[i * w..(i + 1) * w];
                for (o, &v) in row.iter_mut().zip(src) {
                    *o += c * v;
                }
            }
        }
    }
}

/// The parameters of [`discrete_gaussian`], with ITK's defaults in `Default` (ImageMath's `G`
/// sets `maximum_error` to `0.01f`).
#[derive(Clone, Debug, PartialEq)]
pub struct DiscreteGaussianOptions {
    /// The variance along each axis (physical units² with `use_image_spacing`, else voxels²).
    pub variance: Vec<f64>,
    /// The maximum error along each axis (default 0.01).
    pub maximum_error: Vec<f64>,
    /// The maximum kernel width (default 32).
    pub maximum_kernel_width: usize,
    /// Whether the variance is in physical units (default true).
    pub use_image_spacing: bool,
}

impl DiscreteGaussianOptions {
    /// ITK's defaults for a `d`-dimensional image with variance `variance` on every axis.
    pub fn new(d: usize, variance: f64) -> Self {
        DiscreteGaussianOptions {
            variance: vec![variance; d],
            maximum_error: vec![0.01; d],
            maximum_kernel_width: 32,
            use_image_spacing: true,
        }
    }

    /// The kernel along `axis` of an image with `spacing` (`GenerateKernel`).
    pub fn kernel(&self, axis: usize, spacing: &[f64]) -> Vec<f64> {
        let variance = if self.use_image_spacing {
            let s = spacing[axis] * spacing[axis];
            self.variance[axis] / s
        } else {
            self.variance[axis]
        };
        gaussian_kernel(
            variance,
            self.maximum_error[axis],
            self.maximum_kernel_width,
        )
    }
}

/// `DiscreteGaussianImageFilter<float, float>`: separable convolution with
/// [`gaussian_kernel`]s, the last axis first, each pass `float → float`.
pub fn discrete_gaussian(
    input: VolumeRef<'_, f32>,
    options: &DiscreteGaussianOptions,
    n_threads: usize,
) -> Result<Vec<f32>, FilterError> {
    let d = input.dim();
    if options.variance.len() != d || options.maximum_error.len() != d {
        return Err(FilterError::invalid(format!(
            "variances and maximum errors need one value per axis ({d})"
        )));
    }
    let mut out = input.data.to_vec();
    for axis in (0..d).rev() {
        let filter = Convolution {
            kernel: options.kernel(axis, input.spacing),
        };
        filter_lines(&mut out, input.size, axis, &filter, n_threads)?;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kernels_sum_to_one_and_widen_with_the_variance() {
        let k0 = gaussian_kernel(0.0, 0.01, 32);
        assert_eq!(k0, [0.0, 1.0, 0.0]);
        let mut previous = 0;
        for v in [0.25, 1.0, 2.25, 9.0, 30.0] {
            let k = gaussian_kernel(v, 0.01, 32);
            assert_eq!(k.len() % 2, 1);
            let sum: f64 = k.iter().sum();
            assert!((sum - 1.0).abs() < 1e-12, "{v}: {sum}");
            assert!(k.len() >= previous);
            previous = k.len();
            let r = k.len() / 2;
            for i in 0..r {
                assert_eq!(k[i], k[k.len() - 1 - i]);
            }
            // The variance of the kernel is close to v.
            let var: f64 = k
                .iter()
                .enumerate()
                .map(|(i, &c)| c * (i as f64 - r as f64).powi(2))
                .sum();
            assert!((var - v).abs() < 0.1 * v + 0.05, "{v}: {var}");
        }
        // The width is capped: 33 one-sided coefficients, the centre and 32 on each side.
        assert_eq!(gaussian_kernel(300.0, 0.01, 32).len(), 2 * 32 + 1);
        // Above a variance of about 709, e^t overflows in I_0 and the kernel is NaN, as in
        // ITK.
        assert!(gaussian_kernel(1000.0, 0.01, 32).iter().all(|c| c.is_nan()));
    }

    #[test]
    fn bessel_functions() {
        // I0(1) = 1.2660658777520082, I1(1) = 0.5651591039924851, I2(1) = 0.13574766976703828.
        assert!((bessel_i0(1.0) - 1.266_065_877_752_008).abs() < 1e-6);
        assert!((bessel_i1(1.0) - 0.565_159_103_992_485).abs() < 1e-6);
        assert!((bessel_i(2, 1.0) - 0.135_747_669_767_038).abs() < 1e-6);
        assert!((bessel_i0(5.0) - 27.239_871_823_604_44).abs() < 1e-4);
        assert!((bessel_i1(-5.0) + 24.335_642_142_450_52).abs() < 1e-4);
    }

    #[test]
    fn smoothing_is_separable_and_thread_independent() {
        let size = [12usize, 7, 5];
        let spacing = [1.0, 2.0, 0.5];
        let data: Vec<f32> = (0..12 * 7 * 5).map(|i| ((i * 31) % 17) as f32).collect();
        let v = VolumeRef::new(&data, &size, &spacing);
        let options = DiscreteGaussianOptions::new(3, 1.5);
        let a = discrete_gaussian(v, &options, 1).unwrap();
        assert_eq!(a, discrete_gaussian(v, &options, 4).unwrap());
        let mean = |x: &[f32]| x.iter().map(|&v| f64::from(v)).sum::<f64>() / x.len() as f64;
        assert!((mean(&a) - mean(&data)).abs() < 0.5);
        // A constant image stays constant.
        let c = vec![3.5f32; data.len()];
        let out = discrete_gaussian(VolumeRef::new(&c, &size, &spacing), &options, 2).unwrap();
        assert!(out.iter().all(|&v| (v - 3.5).abs() < 1e-5));
    }
}
