// SPDX-License-Identifier: Apache-2.0
//! ImageMath's voxel-wise arithmetic: the `ImageMath<DIM>` function of
//! `Examples/ImageMath_Templates.hxx` (ANTs v2.6.5, operations `m`, `+`, `-`, `/`, `^`,
//! `exp`, `max`, `abs`, `addtozero`, `overadd`, `Decision`, `total`, `mean`, `vtotal`) and
//! `NegativeImage` (`Neg`).
//!
//! Everything is computed in `float`, voxel by voxel in image order, as ANTs does, including
//! its quirks (`docs/findings/ants-image-math.md`):
//! - `/` divides only where the divisor is positive; elsewhere the voxel keeps the value of
//!   the previous voxel (the result variable is not reset);
//! - `total` and `mean` write the running sum into every voxel;
//! - `vtotal` is dispatched to this function but has no branch: the image is all zeros;
//! - `exp` and `Decision` call the double-precision `exp` (CORE-MATH's, correctly rounded)
//!   and round to float; `^` calls `powf` (correctly rounded here, glibc's is not always);
//! - `max` is `std::max(a, b)`: `b` if `a < b`, else `a` (so NaN in `a` wins);
//! - `addtozero` and `overadd` test for zero with `itk::Math::FloatAlmostEqual`.

use larmorx_core::math;
use larmorx_core::parallel;
use larmorx_image::FilterError;
use larmorx_image::itk_math::{almost_equal_f32, almost_equal_f64};
use rayon::prelude::*;

const CHUNK: usize = 1 << 16;

/// A voxel-wise operation of `ImageMath<DIM>`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arithmetic {
    /// `m`: `a · b`.
    Multiply,
    /// `+`.
    Add,
    /// `-`.
    Subtract,
    /// `/`: `a / b` where `b > 0`; elsewhere the previous voxel's result.
    Divide,
    /// `^`: `powf(a, b)`.
    Power,
    /// `exp`: `float(exp(double(a · b)))`.
    Exp,
    /// `max`: `std::max(a, b)`.
    Max,
    /// `abs`: `|a|` (the operand is ignored).
    Abs,
    /// `addtozero`: `a + b` where `a` is zero (within `FloatAlmostEqual`), else `a`.
    AddToZero,
    /// `overadd`: `b` where `b` is not zero, else `a`.
    OverAdd,
    /// `Decision`: `1 / (1 + float(exp(double(−(a − 0.25) / b))))`.
    Decision,
    /// `total`: the running sum of `a · b` (printed: the total and total × voxel volume).
    Total,
    /// `mean`: the running sum of `a · b` (printed: the sum / the voxel count).
    Mean,
    /// `vtotal`: zeros (ANTs dispatches it here but implements nothing).
    VTotal,
}

impl Arithmetic {
    /// The operation for ImageMath's name.
    pub fn from_name(name: &str) -> Option<Arithmetic> {
        Some(match name {
            "m" => Arithmetic::Multiply,
            "+" => Arithmetic::Add,
            "-" => Arithmetic::Subtract,
            "/" => Arithmetic::Divide,
            "^" => Arithmetic::Power,
            "exp" => Arithmetic::Exp,
            "max" => Arithmetic::Max,
            "abs" => Arithmetic::Abs,
            "addtozero" => Arithmetic::AddToZero,
            "overadd" => Arithmetic::OverAdd,
            "Decision" => Arithmetic::Decision,
            "total" => Arithmetic::Total,
            "mean" => Arithmetic::Mean,
            "vtotal" => Arithmetic::VTotal,
            _ => return None,
        })
    }

    /// ImageMath's name.
    pub fn name(self) -> &'static str {
        match self {
            Arithmetic::Multiply => "m",
            Arithmetic::Add => "+",
            Arithmetic::Subtract => "-",
            Arithmetic::Divide => "/",
            Arithmetic::Power => "^",
            Arithmetic::Exp => "exp",
            Arithmetic::Max => "max",
            Arithmetic::Abs => "abs",
            Arithmetic::AddToZero => "addtozero",
            Arithmetic::OverAdd => "overadd",
            Arithmetic::Decision => "Decision",
            Arithmetic::Total => "total",
            Arithmetic::Mean => "mean",
            Arithmetic::VTotal => "vtotal",
        }
    }

    /// The value for one voxel, or `None` where ANTs leaves the result unchanged.
    #[inline]
    fn apply(self, a: f32, b: f32) -> Option<f32> {
        Some(match self {
            Arithmetic::Multiply => a * b,
            Arithmetic::Add => a + b,
            Arithmetic::Subtract => a - b,
            Arithmetic::Divide => {
                if b > 0.0 {
                    a / b
                } else {
                    return None;
                }
            }
            Arithmetic::Power => math::powf(a, b),
            Arithmetic::Exp => math::exp(f64::from(a * b)) as f32,
            Arithmetic::Max => {
                if a < b {
                    b
                } else {
                    a
                }
            }
            Arithmetic::Abs => a.abs(),
            Arithmetic::AddToZero => {
                if almost_equal_f32(a, 0.0) {
                    a + b
                } else {
                    a
                }
            }
            Arithmetic::OverAdd => {
                if !almost_equal_f32(b, 0.0) {
                    b
                } else {
                    a
                }
            }
            Arithmetic::Decision => {
                // `-1.0f * (pix1 - 0.25f)` is an exact negation.
                let e = math::exp(f64::from(-(a - 0.25) / b)) as f32;
                1.0 / (1.0 + e)
            }
            Arithmetic::Total | Arithmetic::Mean | Arithmetic::VTotal => return None,
        })
    }
}

/// The second operand of an operation: an image, or a number.
#[derive(Clone, Copy, Debug)]
pub enum Operand<'a> {
    /// An image with this size along each axis (Fortran order). ANTs reads it at the first
    /// image's voxel indices, so it may be larger than the first image but not smaller.
    Image { data: &'a [f32], size: &'a [usize] },
    /// A number.
    Scalar(f32),
}

/// The second image's values at the first image's voxel indices (`image2->GetPixel(index)`):
/// the image itself when the sizes match, the voxels at the same indices when it is larger.
/// A smaller image is an error (ANTs reads outside it).
pub fn operand_values(
    data: &[f32],
    size: &[usize],
    first: &[usize],
) -> Result<Option<Vec<f32>>, FilterError> {
    if size == first {
        if data.len() != size.iter().product::<usize>() {
            return Err(FilterError::Invalid("second image size mismatch".into()));
        }
        return Ok(None);
    }
    if size.len() != first.len() || size.iter().zip(first).any(|(s, f)| s < f) {
        return Err(FilterError::Invalid(format!(
            "the second image ({size:?}) is smaller than the first ({first:?}); ANTs reads outside it"
        )));
    }
    let n: usize = first.iter().product();
    let mut out = Vec::with_capacity(n);
    for linear in 0..n {
        let mut rest = linear;
        let mut offset = 0;
        let mut stride = 1;
        for (axis, &extent) in first.iter().enumerate() {
            offset += (rest % extent) * stride;
            rest /= extent;
            stride *= size[axis];
        }
        out.push(data[offset]);
    }
    Ok(Some(out))
}

/// What [`arithmetic`] returns.
#[derive(Clone, Debug, PartialEq)]
pub struct ArithmeticOutput {
    /// The output voxels.
    pub data: Vec<f32>,
    /// The final value of ANTs' running `result` (the total for `total` and `mean`).
    pub result: f32,
    /// The voxel count (`ct`) for `mean`.
    pub count: u64,
}

/// `ImageMath<DIM>` for `op` on `a` (with `size`) and `b`, as ANTs computes it.
///
/// The voxel values are computed in parallel. The two operations whose result depends on the
/// previous voxels (`/` keeping the last value where it divides by a non-positive number,
/// and the running sums of `total` and `mean`) finish with a sequential pass in image order,
/// as ANTs does, so the result does not depend on the thread count.
pub fn arithmetic(
    op: Arithmetic,
    a: &[f32],
    size: &[usize],
    b: Operand<'_>,
    n_threads: usize,
) -> Result<ArithmeticOutput, FilterError> {
    let expected: usize = size.iter().product();
    if a.len() != expected {
        return Err(FilterError::SizeMismatch {
            what: "first image",
            actual: a.len(),
            expected,
        });
    }
    let gathered;
    let b_values: Option<&[f32]> = match b {
        Operand::Image { data, size: bsize } => {
            gathered = operand_values(data, bsize, size)?;
            Some(gathered.as_deref().unwrap_or(data))
        }
        Operand::Scalar(_) => None,
    };
    let scalar = match b {
        Operand::Scalar(v) => v,
        Operand::Image { .. } => 0.0,
    };
    let operand = |i: usize| b_values.map_or(scalar, |v| v[i]);
    let mut result = 0.0f32;
    let mut count = 0u64;
    let data = match op {
        Arithmetic::Total | Arithmetic::Mean => {
            let mut out = Vec::with_capacity(a.len());
            for (i, &x) in a.iter().enumerate() {
                result += x * operand(i);
                if op == Arithmetic::Mean {
                    count += 1;
                }
                out.push(result);
            }
            out
        }
        Arithmetic::VTotal => vec![0.0; a.len()],
        Arithmetic::Divide => {
            // Where the divisor is not positive, the voxel keeps the previous result: compute
            // in parallel, marking those voxels, then carry the values in image order.
            let values: Vec<f32> = parallel::with_threads(n_threads, || {
                a.par_iter()
                    .enumerate()
                    .with_min_len(CHUNK)
                    .map(|(i, &x)| op.apply(x, operand(i)).unwrap_or(f32::NAN))
                    .collect()
            })?;
            let mut out = values;
            for (i, v) in out.iter_mut().enumerate() {
                if operand(i) > 0.0 {
                    result = *v;
                } else {
                    *v = result;
                }
            }
            out
        }
        _ => {
            let out: Vec<f32> = parallel::with_threads(n_threads, || {
                a.par_iter()
                    .enumerate()
                    .with_min_len(CHUNK)
                    .map(|(i, &x)| op.apply(x, operand(i)).expect("assigned for every voxel"))
                    .collect()
            })?;
            if let Some(&last) = out.last() {
                result = last;
            }
            out
        }
    };
    Ok(ArithmeticOutput {
        data,
        result,
        count,
    })
}

/// `NegativeImage` (`Neg`): `(1 − (v − min) / (max − min)) · (max − min)` in double,
/// stored as float.
///
/// ANTs finds the range with `if (v > max) max = v; else if (v < min) min = v;` starting from
/// `max = −1e12`, `min = 1e12`. So a voxel that raises the maximum is never compared with
/// the minimum: the first voxel only sets the maximum, and in an image whose values only
/// increase the minimum stays 1e12. When the two are equal (within `FloatAlmostEqual`, as
/// in a constant image) they become 1 and 0, so a constant `c` maps to `1 − c`.
pub fn negative(data: &[f32], n_threads: usize) -> Result<Vec<f32>, FilterError> {
    let (mut mx, mut mn) = (-1.0e12f64, 1.0e12f64);
    for &v in data {
        let px = f64::from(v);
        if px > mx {
            mx = px;
        } else if px < mn {
            mn = px;
        }
    }
    if almost_equal_f64(mx, mn) {
        mx = 1.0;
        mn = 0.0;
    }
    Ok(parallel::with_threads(n_threads, || {
        data.par_iter()
            .with_min_len(CHUNK)
            .map(|&v| {
                let mut pix = f64::from(v);
                pix = (pix - mn) / (mx - mn);
                pix = (1.0 - pix) * (mx - mn);
                pix as f32
            })
            .collect()
    })?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(op: Arithmetic, a: &[f32], b: Operand<'_>) -> ArithmeticOutput {
        arithmetic(op, a, &[a.len()], b, 2).unwrap()
    }

    #[test]
    fn divide_keeps_the_previous_value() {
        let a = [6.0, 8.0, 10.0, 12.0];
        let b = [2.0, 0.0, -1.0, 4.0];
        let out = run(
            Arithmetic::Divide,
            &a,
            Operand::Image {
                data: &b,
                size: &[4],
            },
        );
        assert_eq!(out.data, [3.0, 3.0, 3.0, 3.0]);
        let out = run(
            Arithmetic::Divide,
            &[1.0, 2.0],
            Operand::Image {
                data: &[0.0, 2.0],
                size: &[2],
            },
        );
        assert_eq!(out.data, [0.0, 1.0]); // nothing before the first voxel: 0
    }

    #[test]
    fn running_sums_and_zeros() {
        let a = [1.0, 2.0, 3.0];
        let out = run(Arithmetic::Total, &a, Operand::Scalar(2.0));
        assert_eq!(out.data, [2.0, 6.0, 12.0]);
        assert_eq!(out.result, 12.0);
        let out = run(Arithmetic::Mean, &a, Operand::Scalar(1.0));
        assert_eq!((out.result, out.count), (6.0, 3));
        assert_eq!(
            run(Arithmetic::VTotal, &a, Operand::Scalar(1.0)).data,
            [0.0; 3]
        );
    }

    #[test]
    fn zero_tests_and_max() {
        let a = [0.0, 1e-9, 5.0, f32::NAN];
        let b = [7.0, 7.0, 7.0, 7.0];
        let img = Operand::Image {
            data: &b,
            size: &[4],
        };
        let out = run(Arithmetic::AddToZero, &a, img).data;
        assert_eq!(&out[..3], [7.0, 7.0 + 1e-9, 5.0]);
        assert!(out[3].is_nan());
        let out = run(Arithmetic::Max, &a, img).data;
        assert_eq!(&out[..3], [7.0, 7.0, 7.0]);
        assert!(out[3].is_nan()); // std::max(NaN, 7) is NaN
    }

    #[test]
    fn larger_second_images_are_read_by_index() {
        // a: 2 x 2; b: 3 x 2 (Fortran order), read at the same (i, j).
        let a = [1.0, 1.0, 1.0, 1.0];
        let b = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let out = arithmetic(
            Arithmetic::Multiply,
            &a,
            &[2, 2],
            Operand::Image {
                data: &b,
                size: &[3, 2],
            },
            1,
        )
        .unwrap();
        assert_eq!(out.data, [1.0, 2.0, 4.0, 5.0]);
        assert!(
            arithmetic(
                Arithmetic::Multiply,
                &b,
                &[3, 2],
                Operand::Image {
                    data: &a,
                    size: &[2, 2]
                },
                1
            )
            .is_err()
        );
    }

    #[test]
    fn negative_reproduces_the_range_quirk() {
        // Increasing values: each raises the max, so the min stays 1e12 and the result,
        // max - v algebraically, carries rounding errors of about 1e-4.
        let out = negative(&[1.0, 2.0, 3.0], 1).unwrap();
        for (o, e) in out.iter().zip([2.0, 1.0, 0.0]) {
            assert!((o - e).abs() < 1e-3, "{out:?}");
        }
        // Constant image: max = min = 5, replaced by 1 and 0: 1 - v.
        assert_eq!(negative(&[5.0, 5.0], 1).unwrap(), [-4.0, -4.0]);
        // Decreasing values: the first is the max, the others lower the min.
        assert_eq!(negative(&[3.0, 2.0, 1.0], 1).unwrap(), [0.0, 1.0, 2.0]);
    }
}
