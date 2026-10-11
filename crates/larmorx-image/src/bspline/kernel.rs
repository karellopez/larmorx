// SPDX-License-Identifier: Apache-2.0
//! B-spline kernels as ITK's B-spline filters evaluate them.
//!
//! - Orders 0 to 3: `itk::BSplineKernelFunction<order, double>`
//!   (`Modules/Core/Common/include/itkBSplineKernelFunction.h`), closed forms in double.
//! - Higher orders, and the shape functions the lattice refinement needs:
//!   `itk::CoxDeBoorBSplineKernelFunction` (`Modules/Filtering/ImageGrid/include/
//!   itkCoxDeBoorBSplineKernelFunction.hxx`), polynomials built by the Cox–de Boor recursion
//!   with vnl's `vnl_real_polynomial` arithmetic (`core/vnl/vnl_real_polynomial.cxx`).
//! - [`refined_lattice_coefficients`]: the `2 × (order + 1)` matrix that doubles a control
//!   point lattice's resolution, `vnl_svd<float>(R).solve(S)` from the shape functions
//!   (`SetSplineOrder` of `BSplineControlPointImageFilter` and of
//!   `BSplineScatteredDataPointSetToImageFilter`).

use larmorx_core::vnl_svd_f32::vnl_svd_solve_f32;

use crate::itk_math::almost_equal_f64;

/// A polynomial as `vnl_real_polynomial` stores it: coefficients from the highest degree down
/// (`c[0]·x^d + … + c[d]`).
#[derive(Clone, Debug, PartialEq)]
pub struct Polynomial(pub Vec<f64>);

impl Polynomial {
    fn constant(a: f64) -> Self {
        Polynomial(vec![a])
    }

    fn degree(&self) -> usize {
        self.0.len() - 1
    }

    /// `vnl_real_polynomial_evaluate`: `c[d] + c[d-1]·x + c[d-2]·x² + …`, the powers of `x`
    /// built by repeated multiplication (not Horner's rule).
    pub fn evaluate(&self, x: f64) -> f64 {
        let a = &self.0;
        let mut n = a.len() - 1;
        let mut acc = a[n];
        let mut xn = x;
        while n > 0 {
            n -= 1;
            acc += a[n] * xn;
            xn *= x;
        }
        acc
    }

    /// `operator+`: the coefficients aligned at the constant term, each sum started from 0.
    fn add(&self, other: &Polynomial) -> Polynomial {
        let (d1, d2) = (self.degree(), other.degree());
        let d = d1.max(d2);
        let mut sum = vec![0.0; d + 1];
        for i in 0..=d {
            sum[d - i] = 0.0;
            if i <= d1 {
                sum[d - i] += self.0[d1 - i];
            }
            if i <= d2 {
                sum[d - i] += other.0[d2 - i];
            }
        }
        Polynomial(sum)
    }

    /// `operator*`: the convolution of the coefficients, in vnl's loop order.
    fn mul(&self, other: &Polynomial) -> Polynomial {
        let (d1, d2) = (self.degree(), other.degree());
        let d = d1 + d2;
        let mut prod = vec![0.0; d + 1];
        for i in 0..=d1 {
            for j in 0..=d2 {
                prod[d - (i + j)] += self.0[d1 - i] * other.0[d2 - j];
            }
        }
        Polynomial(prod)
    }
}

/// `CoxDeBoorBSplineKernelFunction::CoxDeBoor(order, knots, whichBasisFunction,
/// whichPiece)`.
fn cox_de_boor(order: usize, knots: &[f64], i: usize, piece: usize) -> Polynomial {
    let p = order - 1;
    if p == 0 && i == piece {
        return Polynomial::constant(1.0);
    }
    // Term 1.
    let den = knots[i + p] - knots[i];
    let poly1 = if almost_equal_f64(den, 0.0) {
        Polynomial::constant(0.0)
    } else {
        let tmp = Polynomial(vec![1.0 / den, -knots[i] / den]);
        tmp.mul(&cox_de_boor(order - 1, knots, i, piece))
    };
    // Term 2.
    let den = knots[i + p + 1] - knots[i + 1];
    let poly2 = if almost_equal_f64(den, 0.0) {
        Polynomial::constant(0.0)
    } else {
        let tmp = Polynomial(vec![-1.0 / den, knots[i + p + 1] / den]);
        tmp.mul(&cox_de_boor(order - 1, knots, i + 1, piece))
    };
    poly1.add(&poly2)
}

/// `GenerateBSplineShapeFunctions(order + 1)`: the polynomial pieces of the centred B-spline
/// of `order` on `|u|` ∈ [0, 0.5), [0.5, 1.5), … (even orders) or [0, 1), [1, 2), … (odd).
pub fn shape_functions(order: usize) -> Vec<Polynomial> {
    let k = order + 1;
    let pieces = (0.5 * (k + 1) as f64) as usize;
    let knots: Vec<f64> = (0..=k).map(|i| -0.5 * k as f64 + i as f64).collect();
    (0..pieces)
        .map(|i| cox_de_boor(k, &knots, 0, (0.5 * k as f64) as usize + i))
        .collect()
}

/// `GetShapeFunctionsInZeroToOneInterval`: the `order + 1` polynomials of the B-spline basis
/// functions on [0, 1), each with `order + 1` coefficients.
pub fn shape_functions_zero_to_one(order: usize) -> Vec<Polynomial> {
    let k = order + 1;
    let knots: Vec<f64> = (0..2 * k).map(|i| -(order as f64) + i as f64).collect();
    (0..k).map(|i| cox_de_boor(k, &knots, i, k - 1)).collect()
}

/// A B-spline kernel of one order, evaluated as ITK's B-spline filters do.
#[derive(Clone, Debug)]
pub struct Kernel {
    order: usize,
    /// The Cox–de Boor pieces, for orders above 3.
    pieces: Vec<Polynomial>,
}

impl Kernel {
    pub fn new(order: usize) -> Self {
        Kernel {
            order,
            pieces: if order > 3 {
                shape_functions(order)
            } else {
                Vec::new()
            },
        }
    }

    pub fn order(&self) -> usize {
        self.order
    }

    /// The kernel at `u` (double precision).
    pub fn evaluate(&self, u: f64) -> f64 {
        let a = u.abs();
        match self.order {
            0 => {
                if a < 0.5 {
                    1.0
                } else if a == 0.5 {
                    0.5
                } else {
                    0.0
                }
            }
            1 => {
                if a < 1.0 {
                    1.0 - a
                } else {
                    0.0
                }
            }
            2 => {
                if a < 0.5 {
                    0.75 - a * a
                } else if a < 1.5 {
                    let sqr = a * a;
                    (9.0 - 12.0 * a + 4.0 * sqr) * 0.125
                } else {
                    0.0
                }
            }
            3 => {
                if a < 1.0 {
                    let sqr = a * a;
                    (4.0 - 6.0 * sqr + 3.0 * sqr * a) / 6.0
                } else if a < 2.0 {
                    let sqr = a * a;
                    (8.0 - 12.0 * a + 6.0 * sqr - sqr * a) / 6.0
                } else {
                    0.0
                }
            }
            _ => {
                let which = if self.order.is_multiple_of(2) {
                    (a + 0.5) as usize
                } else {
                    a as usize
                };
                self.pieces.get(which).map_or(0.0, |p| p.evaluate(a))
            }
        }
    }
}

/// `m_RefinedLatticeCoefficients` for a spline order: a `2 × (order + 1)` float matrix
/// (row-major) giving each control point of a lattice refined to twice the resolution from
/// `order + 1` coarse ones. Computed as ITK does: `R = S = float(C)` from the shape functions
/// on [0, 1), column `j` of `R` scaled by `2^(order − j)`, both transposed and flipped upside
/// down, then `vnl_svd<float>(R).solve(S)`, of which the first two rows are kept.
pub fn refined_lattice_coefficients(order: usize) -> Option<Vec<f32>> {
    let c = shape_functions_zero_to_one(order);
    let n = order + 1;
    // C(j, k): row j = piece j's coefficients.
    let mut r = vec![0.0f32; n * n];
    let mut s = vec![0.0f32; n * n];
    for j in 0..n {
        for k in 0..n {
            let v = c[j].0[k] as f32;
            r[j * n + k] = v;
            s[j * n + k] = v;
        }
    }
    for j in 0..n {
        let scale = (2.0f64).powi((n - j - 1) as i32) as f32; // powf(2, cols - j - 1), exact
        for k in 0..n {
            r[k * n + j] *= scale;
        }
    }
    // transpose, then flipud: new(i, j) = old(j, n - 1 - i).
    let flip = |m: &[f32]| -> Vec<f32> {
        let mut out = vec![0.0f32; n * n];
        for i in 0..n {
            for j in 0..n {
                out[i * n + j] = m[j * n + (n - 1 - i)];
            }
        }
        out
    };
    let (r, s) = (flip(&r), flip(&s));
    let x = vnl_svd_solve_f32(&r, n, &s, n)?;
    Some(x[..2 * n].to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cubic_kernel_values() {
        let k = Kernel::new(3);
        assert_eq!(k.evaluate(0.0), 4.0 / 6.0);
        assert_eq!(k.evaluate(1.0), 1.0 / 6.0);
        assert_eq!(k.evaluate(-1.5), 0.020833333333333332);
        assert_eq!(k.evaluate(2.0), 0.0);
    }

    #[test]
    fn cox_de_boor_matches_the_closed_forms() {
        for order in 1..=3 {
            let closed = Kernel::new(order);
            let pieces = shape_functions(order);
            let general = Kernel {
                order: 99,
                pieces: pieces.clone(),
            };
            for i in -30..=30 {
                let u = f64::from(i) * 0.07;
                let a = u.abs();
                let which = if order % 2 == 0 {
                    (a + 0.5) as usize
                } else {
                    a as usize
                };
                let g = pieces.get(which).map_or(0.0, |p| p.evaluate(a));
                assert!((closed.evaluate(u) - g).abs() < 1e-14, "{order} {u}");
                let _ = &general;
            }
        }
    }

    #[test]
    fn refinement_of_cubic_splines() {
        // Row 0 (even refined points) 1/2, 1/2, 0, 0; row 1 (odd) 1/8, 6/8, 1/8, 0. vnl's float
        // SVD gets within a few float ulps.
        let c = refined_lattice_coefficients(3).unwrap();
        let exact = [0.5f32, 0.5, 0.0, 0.0, 0.125, 0.75, 0.125, 0.0];
        for (a, b) in c.iter().zip(exact) {
            assert!((a - b).abs() < 1e-5, "{c:?}");
        }
    }
}
