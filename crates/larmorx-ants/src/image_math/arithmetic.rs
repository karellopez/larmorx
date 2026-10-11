// SPDX-License-Identifier: Apache-2.0
//! ImageMath's voxel-wise arithmetic: the `ImageMath<DIM>` function of
//! `Examples/ImageMath_Templates.hxx` (ANTs v2.6.5, operations `m`, `+`, `-`, `/`, `^`,
//! `exp`, `max`, `abs`, `addtozero`, `overadd`, `Decision`, `total`, `mean`, `vtotal`) and
//! `NegativeImage` (`Neg`).
//!
//! Everything is computed in `float`, voxel by voxel in image order, as ANTs does
//! (`docs/findings/ants-image-math.md`):
//! - `exp` and `Decision` call the double-precision `exp` (CORE-MATH's, correctly rounded)
//!   and round to float; `^` calls `powf` (correctly rounded here, glibc's is not always);
//! - `max` is `std::max(a, b)`: `b` if `a < b`, else `a` (so NaN in `a` wins);
//! - `addtozero` and `overadd` test for zero with `itk::Math::FloatAlmostEqual`.
//!
//! ANTs' defects are corrected by default and reproduced with [`UpstreamDefects::Reproduce`]
//! (D13, `docs/api/upstream-defects.md`):
//! - ANTS-08: `/` divides only where the divisor is positive; elsewhere the voxel keeps the
//!   previous voxel's result (the result variable is not reset). Corrected: `a / b` wherever
//!   `b` is not 0, and 0 where it is;
//! - ANTS-09: `total` and `mean` write the running sum into every voxel. Corrected: every
//!   voxel holds the value printed (the total, or the mean);
//! - ANTS-10: `vtotal` is dispatched to this function but has no branch: the image is all
//!   zeros. Corrected: an error;
//! - ANTS-11: `Neg` finds the range with `else if` ([`negative`]). Corrected: the true range;
//! - ANTS-07: a second image smaller than the first is read outside its voxels. Corrected:
//!   an error; reproduced: the reads that land inside its buffer as ANTs makes them, 0 for
//!   those beyond it (their values depend on memory; a notice says so).

use larmorx_core::defects::{self, Notice, Notices, UpstreamDefects};
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
    /// image's voxel indices, so it may be larger than the first image (its other voxels are
    /// ignored); a smaller one is read outside its voxels (ANTS-07).
    Image { data: &'a [f32], size: &'a [usize] },
    /// A number.
    Scalar(f32),
}

/// Whether a second image of `size` is smaller than the first (`first`) along some axis, so
/// that ANTs reads it outside its voxels (ANTS-07).
pub fn operand_is_smaller(size: &[usize], first: &[usize]) -> bool {
    size.len() != first.len() || size.iter().zip(first).any(|(s, f)| s < f)
}

/// The second image's values at the first image's voxel indices (`image2->GetPixel(index)`,
/// the index's offset with the second image's own strides): `None` when the sizes match (the
/// image itself), else the gathered values and how many of them ANTs reads beyond the second
/// image's buffer.
///
/// A smaller second image (ANTS-07) is an error with [`UpstreamDefects::Fix`]. With
/// [`UpstreamDefects::Reproduce`] the reads that land inside its buffer are ANTs' (they wrap
/// onto other rows), and those beyond the buffer, whose values depend on memory, are 0.
pub fn operand_values(
    data: &[f32],
    size: &[usize],
    first: &[usize],
    mode: UpstreamDefects,
) -> Result<Option<(Vec<f32>, usize)>, FilterError> {
    if data.len() != size.iter().product::<usize>() {
        return Err(FilterError::Invalid("second image size mismatch".into()));
    }
    if size == first {
        return Ok(None);
    }
    if size.len() != first.len() {
        return Err(FilterError::Invalid(format!(
            "the second image has {} dimensions, the first {}",
            size.len(),
            first.len()
        )));
    }
    if mode.fix() && operand_is_smaller(size, first) {
        return Err(FilterError::Invalid(format!(
            "the second image ({size:?}) is smaller than the first ({first:?}); ANTs reads \
             outside it (ANTS-07; with the upstream defects reproduced, larmorx reads it as \
             ANTs does)"
        )));
    }
    let n: usize = first.iter().product();
    let mut out = Vec::with_capacity(n);
    let mut outside = 0;
    for linear in 0..n {
        let mut rest = linear;
        let mut offset = 0;
        let mut stride = 1;
        for (axis, &extent) in first.iter().enumerate() {
            offset += (rest % extent) * stride;
            rest /= extent;
            stride *= size[axis];
        }
        out.push(match data.get(offset) {
            Some(&v) => v,
            None => {
                outside += 1;
                0.0
            }
        });
    }
    Ok(Some((out, outside)))
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
    /// The upstream defects that changed this result (or, reproduced, would have).
    pub notices: Vec<Notice>,
}

/// `ImageMath<DIM>` for `op` on `a` (with `size`) and `b`, as ANTs computes it, with its
/// defects corrected or reproduced as `mode` says (see the module).
///
/// The voxel values are computed in parallel. The operations whose result depends on the
/// previous voxels (`/` reproduced, keeping the last value where it divides by a non-positive
/// number, and the running sums of `total` and `mean`) finish with a sequential pass in image
/// order, as ANTs does, so the result does not depend on the thread count.
pub fn arithmetic(
    op: Arithmetic,
    a: &[f32],
    size: &[usize],
    b: Operand<'_>,
    n_threads: usize,
    mode: UpstreamDefects,
) -> Result<ArithmeticOutput, FilterError> {
    let mut notices = Notices::new(mode);
    let expected: usize = size.iter().product();
    if a.len() != expected {
        return Err(FilterError::SizeMismatch {
            what: "first image",
            actual: a.len(),
            expected,
        });
    }
    if op == Arithmetic::VTotal && mode.fix() {
        return Err(FilterError::Invalid(
            "vtotal is not implemented: ANTs writes an image of zeros (ANTS-10; with the \
             upstream defects reproduced, so does larmorx)"
                .into(),
        ));
    }
    let gathered;
    let b_values: Option<&[f32]> = match b {
        Operand::Image { data, size: bsize } => {
            gathered = operand_values(data, bsize, size, mode)?;
            if let Some((_, outside)) = &gathered
                && operand_is_smaller(bsize, size)
            {
                notices.add(
                    &defects::ANTS_07,
                    format!(
                        "the second image ({bsize:?}) is smaller than the first ({size:?}) and is \
                         read at the first image's indices as ANTs reads it; {outside} of {} \
                         reads land beyond its voxels, where ANTs reads whatever memory follows: \
                         here 0",
                        a.len()
                    ),
                );
            }
            Some(gathered.as_ref().map_or(data, |(v, _)| v.as_slice()))
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
            // ANTS-09: the value printed, in every voxel.
            let printed = if op == Arithmetic::Mean {
                result / count as f32
            } else {
                result
            };
            let differ = out.iter().any(|v| v.to_bits() != printed.to_bits());
            if differ {
                let what = if op == Arithmetic::Mean {
                    "mean"
                } else {
                    "total"
                };
                if mode.fix() {
                    notices.add(
                        &defects::ANTS_09,
                        format!(
                            "every voxel holds the {what} printed; ANTs writes the running sum"
                        ),
                    );
                    out = vec![printed; a.len()];
                } else {
                    notices.add(
                        &defects::ANTS_09,
                        format!(
                            "the image holds the running sum, as ANTs writes it, not the {what}"
                        ),
                    );
                }
            }
            out
        }
        Arithmetic::VTotal => {
            notices.add(
                &defects::ANTS_10,
                "vtotal writes an image of zeros, as ANTs does",
            );
            vec![0.0; a.len()]
        }
        Arithmetic::Divide => {
            // ANTs divides where the divisor is positive; elsewhere the voxel keeps the previous
            // result. Compute in parallel, then carry the values in image order, counting the
            // voxels where the corrected quotient differs.
            let values: Vec<f32> = parallel::with_threads(n_threads, || {
                a.par_iter()
                    .enumerate()
                    .with_min_len(CHUNK)
                    .map(|(i, &x)| op.apply(x, operand(i)).unwrap_or(f32::NAN))
                    .collect()
            })?;
            let mut out = values;
            let mut changed = 0usize;
            for (i, v) in out.iter_mut().enumerate() {
                let d = operand(i);
                if d > 0.0 {
                    result = *v;
                    continue;
                }
                let corrected = if d == 0.0 { 0.0 } else { a[i] / d };
                if corrected.to_bits() != result.to_bits() {
                    changed += 1;
                }
                *v = if mode.fix() { corrected } else { result };
            }
            if changed > 0 {
                let detail = if mode.fix() {
                    format!(
                        "{changed} voxel(s) with a divisor that is not positive are divided (0 \
                         where it is 0); ANTs gives them the previous voxel's result"
                    )
                } else {
                    format!(
                        "{changed} voxel(s) with a divisor that is not positive get the previous \
                         voxel's result, as in ANTs"
                    )
                };
                notices.add(&defects::ANTS_08, detail);
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
        notices: notices.into_vec(),
    })
}

/// `NegativeImage` (`Neg`): `(1 − (v − min) / (max − min)) · (max − min)` in double,
/// stored as float; with the notices of the defect it meets.
///
/// ANTs finds the range with `if (v > max) max = v; else if (v < min) min = v;` starting from
/// `max = −1e12`, `min = 1e12` (ANTS-11). So a voxel that raises the maximum is never compared
/// with the minimum: the first voxel only sets the maximum, and in an image whose values only
/// increase the minimum stays 1e12. With [`UpstreamDefects::Fix`] the range is the image's
/// true minimum and maximum. When the two are equal (within `FloatAlmostEqual`, as in a
/// constant image) they become 1 and 0, so a constant `c` maps to `1 − c`.
pub fn negative(
    data: &[f32],
    n_threads: usize,
    mode: UpstreamDefects,
) -> Result<(Vec<f32>, Vec<Notice>), FilterError> {
    let (mut mx, mut mn) = (-1.0e12f64, 1.0e12f64);
    for &v in data {
        let px = f64::from(v);
        if px > mx {
            mx = px;
        } else if px < mn {
            mn = px;
        }
    }
    let mut notices = Notices::new(mode);
    // The true range (from ANTs' starting values, which only matter for an image without a
    // number in it).
    let (tmx, tmn) = data
        .iter()
        .map(|&v| f64::from(v))
        .filter(|v| !v.is_nan())
        .fold(None, |acc: Option<(f64, f64)>, v| match acc {
            None => Some((v, v)),
            Some((hi, lo)) => Some((hi.max(v), lo.min(v))),
        })
        .unwrap_or((mx, mn));
    if (tmx, tmn) != (mx, mn) {
        let detail = format!(
            "the range is [{tmn}, {tmx}]; ANTs finds [{}, {mx}]",
            if mn == 1.0e12 {
                "1e12 (never set)".to_owned()
            } else {
                mn.to_string()
            }
        );
        notices.add(&defects::ANTS_11, detail);
        if mode.fix() {
            (mx, mn) = (tmx, tmn);
        }
    }
    if almost_equal_f64(mx, mn) {
        mx = 1.0;
        mn = 0.0;
    }
    let out = parallel::with_threads(n_threads, || {
        data.par_iter()
            .with_min_len(CHUNK)
            .map(|&v| {
                let mut pix = f64::from(v);
                pix = (pix - mn) / (mx - mn);
                pix = (1.0 - pix) * (mx - mn);
                pix as f32
            })
            .collect()
    })?;
    Ok((out, notices.into_vec()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const R: UpstreamDefects = UpstreamDefects::Reproduce;
    const F: UpstreamDefects = UpstreamDefects::Fix;

    fn run(op: Arithmetic, a: &[f32], b: Operand<'_>, mode: UpstreamDefects) -> ArithmeticOutput {
        arithmetic(op, a, &[a.len()], b, 2, mode).unwrap()
    }

    fn ids(notices: &[Notice]) -> Vec<&'static str> {
        notices.iter().map(|n| n.defect.id).collect()
    }

    #[test]
    fn divide_keeps_the_previous_value() {
        let a = [6.0, 8.0, 10.0, 12.0];
        let b = [2.0, 0.0, -1.0, 4.0];
        let img = Operand::Image {
            data: &b,
            size: &[4],
        };
        let out = run(Arithmetic::Divide, &a, img, R);
        assert_eq!(out.data, [3.0, 3.0, 3.0, 3.0]);
        assert_eq!(ids(&out.notices), ["ANTS-08"]);
        let out = run(
            Arithmetic::Divide,
            &[1.0, 2.0],
            Operand::Image {
                data: &[0.0, 2.0],
                size: &[2],
            },
            R,
        );
        assert_eq!(out.data, [0.0, 1.0]); // nothing before the first voxel: 0
        // ...which is also the corrected value: nothing to say.
        assert!(out.notices.is_empty());
        // ANTS-08 corrected: divided wherever the divisor is not 0.
        let out = run(Arithmetic::Divide, &a, img, F);
        assert_eq!(out.data, [3.0, 0.0, -10.0, 3.0]);
        assert_eq!(ids(&out.notices), ["ANTS-08"]);
        let positive = Operand::Image {
            data: &[2.0, 4.0, 5.0, 4.0],
            size: &[4],
        };
        let (f, r) = (
            run(Arithmetic::Divide, &a, positive, F),
            run(Arithmetic::Divide, &a, positive, R),
        );
        assert_eq!(f.data, r.data);
        assert!(f.notices.is_empty() && r.notices.is_empty());
    }

    #[test]
    fn running_sums_and_zeros() {
        let a = [1.0, 2.0, 3.0];
        let out = run(Arithmetic::Total, &a, Operand::Scalar(2.0), R);
        assert_eq!(out.data, [2.0, 6.0, 12.0]);
        assert_eq!(out.result, 12.0);
        assert_eq!(ids(&out.notices), ["ANTS-09"]);
        let out = run(Arithmetic::Mean, &a, Operand::Scalar(1.0), R);
        assert_eq!((out.result, out.count), (6.0, 3));
        let out = run(Arithmetic::VTotal, &a, Operand::Scalar(1.0), R);
        assert_eq!(out.data, [0.0; 3]);
        assert_eq!(ids(&out.notices), ["ANTS-10"]);
        // Corrected: the printed value everywhere; vtotal refused.
        let out = run(Arithmetic::Total, &a, Operand::Scalar(2.0), F);
        assert_eq!((out.data, out.result), (vec![12.0; 3], 12.0));
        let out = run(Arithmetic::Mean, &a, Operand::Scalar(1.0), F);
        assert_eq!(out.data, [2.0; 3]);
        assert_eq!(ids(&out.notices), ["ANTS-09"]);
        let e = arithmetic(Arithmetic::VTotal, &a, &[3], Operand::Scalar(1.0), 1, F);
        assert!(e.unwrap_err().to_string().contains("ANTS-10"));
        // A sum that never changes after the first voxel: the two agree.
        let out = run(Arithmetic::Total, &[5.0, 0.0], Operand::Scalar(1.0), F);
        assert_eq!(out.data, [5.0, 5.0]);
        assert!(out.notices.is_empty());
    }

    #[test]
    fn zero_tests_and_max() {
        let a = [0.0, 1e-9, 5.0, f32::NAN];
        let b = [7.0, 7.0, 7.0, 7.0];
        let img = Operand::Image {
            data: &b,
            size: &[4],
        };
        let out = run(Arithmetic::AddToZero, &a, img, F).data;
        assert_eq!(&out[..3], [7.0, 7.0 + 1e-9, 5.0]);
        assert!(out[3].is_nan());
        let out = run(Arithmetic::Max, &a, img, F).data;
        assert_eq!(&out[..3], [7.0, 7.0, 7.0]);
        assert!(out[3].is_nan()); // std::max(NaN, 7) is NaN
    }

    #[test]
    fn second_images_are_read_by_index() {
        // a: 2 x 2; b: 3 x 2 (Fortran order), read at the same (i, j).
        let a = [1.0, 1.0, 1.0, 1.0];
        let b = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        for mode in [R, F] {
            let out = arithmetic(
                Arithmetic::Multiply,
                &a,
                &[2, 2],
                Operand::Image {
                    data: &b,
                    size: &[3, 2],
                },
                1,
                mode,
            )
            .unwrap();
            assert_eq!(out.data, [1.0, 2.0, 4.0, 5.0]);
            assert!(out.notices.is_empty());
        }
        // A smaller second image (ANTS-07): an error, or ANTs' reads with 0 beyond the buffer.
        let smaller = Operand::Image {
            data: &a,
            size: &[2, 2],
        };
        let e = arithmetic(Arithmetic::Multiply, &b, &[3, 2], smaller, 1, F).unwrap_err();
        assert!(e.to_string().contains("ANTS-07"), "{e}");
        let out = arithmetic(
            Arithmetic::Multiply,
            &b,
            &[3, 2],
            Operand::Image {
                data: &[10.0, 20.0, 30.0, 40.0],
                size: &[2, 2],
            },
            1,
            R,
        )
        .unwrap();
        // Offsets i + 2j: 0, 1, 2, 2, 3, 4 (the last beyond the 4 values).
        assert_eq!(out.data, [10.0, 40.0, 90.0, 120.0, 200.0, 0.0]);
        assert_eq!(ids(&out.notices), ["ANTS-07"]);
        assert!(out.notices[0].detail.contains("1 of 6"));
    }

    #[test]
    fn negative_range() {
        // Increasing values: each raises the max, so the min stays 1e12 and the result,
        // max - v algebraically, carries rounding errors of about 1e-4.
        let (out, notices) = negative(&[1.0, 2.0, 3.0], 1, R).unwrap();
        for (o, e) in out.iter().zip([2.0, 1.0, 0.0]) {
            assert!((o - e).abs() < 1e-3, "{out:?}");
        }
        assert_ne!(out, [2.0, 1.0, 0.0]);
        assert_eq!(ids(&notices), ["ANTS-11"]);
        // ANTS-11 corrected: the true range, exactly max - v.
        let (out, notices) = negative(&[1.0, 2.0, 3.0], 1, F).unwrap();
        assert_eq!(out, [2.0, 1.0, 0.0]);
        assert_eq!(ids(&notices), ["ANTS-11"]);
        // Constant image: max = min = 5, replaced by 1 and 0: 1 - v.
        for mode in [R, F] {
            let (out, notices) = negative(&[5.0, 5.0], 1, mode).unwrap();
            assert_eq!(out, [-4.0, -4.0]);
            assert!(notices.is_empty());
        }
        // Decreasing values: the first is the max, the others lower the min; the range is
        // right.
        let (out, notices) = negative(&[3.0, 2.0, 1.0], 1, R).unwrap();
        assert_eq!(out, [0.0, 1.0, 2.0]);
        assert!(notices.is_empty());
    }
}
