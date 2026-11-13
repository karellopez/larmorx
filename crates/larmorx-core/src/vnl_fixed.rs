// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! vnl's closed-form determinants and inverses of small matrices, as written in VXL as
//! bundled with ITK v5.4.5 and v4.13.2 (BSD licence; notice in [`crate::vnl_svd`]):
//! `core/vnl/vnl_det.hxx` (`vnl_det`), `core/vnl/algo/vnl_determinant.hxx`
//! (`vnl_determinant` of 1 to 4 rows, which uses the same expressions), `core/vnl/vnl_inverse.h`
//! (`vnl_inverse` of `vnl_matrix_fixed<T, 2..4, 2..4>` and of a `vnl_matrix` of up to 4 rows).
//!
//! Each expression is evaluated as C++ evaluates it (left to right, no fused multiply-adds),
//! so the numbers are bit-identical to vnl's on every platform. **NaNs too:** where two NaNs
//! meet, x86-64 passes on the first operand's, and which operand of a `+` or `*` comes first is
//! the compiler's choice. [`sse`] holds the expressions with the operands in the order GCC 8.4.0
//! gives ITK 4.13.2's instances (read from their object code), each operation through
//! [`ItkReal::sse_add`] and its kin, so NaN results carry x86-64's signs and payloads on every
//! platform. The `float` and `double` instances share the determinants' and the 3 x 3 inverse's
//! order; the 2 x 2 and 4 x 4 inverses have one each ([`ItkReal::vnl_inverse2`]).
//!
//! The functions are generic over `float` and `double` ([`ItkReal`]), as `vnl_matrix_fixed<T, n,
//! n>` is; each operation is in that type (ITK 4.13.2's `vnl_det.hxx` and `vnl_inverse.h`, the
//! same expressions as 5.4.5's). [`determinant`] of more than 4 rows is
//! `vnl_determinant<T>`'s QR determinant ([`crate::vnl_qr::Qr`]), and
//! [`determinant_balanced`] its `balance` form (their NaNs follow `vnl_qr`'s port, not checked
//! against x86-64's). `vnl_inverse` of a matrix whose determinant is 0 returns VNL's
//! uninitialised `vnl_matrix_fixed` (its assertion is compiled out): memory contents, given here
//! as the zero matrix (ITK-107).

pub(crate) mod sse;

/// `vnl_det(row0, row1)`.
#[inline]
pub fn det2<T: ItkReal>(m: &[[T; 2]; 2]) -> T {
    sse::det2(&m[0], &m[1])
}

/// `vnl_det(row0, row1, row2)`.
#[inline]
pub fn det3<T: ItkReal>(m: &[[T; 3]; 3]) -> T {
    sse::det3(&m[0], &m[1], &m[2])
}

/// `vnl_det(row0, row1, row2, row3)`.
#[inline]
pub fn det4<T: ItkReal>(m: &[[T; 4]; 4]) -> T {
    sse::det4(&m[0], &m[1], &m[2], &m[3])
}

/// `vnl_determinant<T>(M, balance)`: with `balance`, more than 4 rows are first balanced by
/// five rounds of dividing each row, then each column, by its RMS (where positive), the
/// divisors multiplied into `scalings`, and the result is `T(scalings)·det` of the balanced
/// matrix (an RMS that is NaN or not positive leaves its row or column alone); `balance` changes
/// nothing up to 4 rows.
pub fn determinant_balanced<T: ItkReal>(m: &[T], n: usize, balance: bool) -> T {
    if n <= 4 || !balance {
        return determinant(m, n);
    }
    assert_eq!(m.len(), n * n, "not an n × n matrix");
    let mut tmp = m.to_vec();
    let mut scalings = T::ONE;
    // vnl_c_vector_rms_norm: the squares summed in T from the first, divided by n (in T),
    // the square root in double rounded to T.
    let rms = |v: &mut dyn Iterator<Item = T>| -> T {
        let mut val = T::ZERO;
        for x in v {
            val = val + x * x;
        }
        val = val / T::from_f64(n as f64);
        T::from_f64(val.to_f64().sqrt())
    };
    for _ in 0..5 {
        for i in 0..n {
            let rn = rms(&mut (0..n).map(|j| tmp[i * n + j]));
            if rn > T::ZERO {
                scalings = scalings.sse_mul(rn);
                let s = T::ONE.sse_div(rn);
                for j in 0..n {
                    tmp[i * n + j] = tmp[i * n + j] * s;
                }
            }
        }
        for j in 0..n {
            let rn = rms(&mut (0..n).map(|i| tmp[i * n + j]));
            if rn > T::ZERO {
                scalings = scalings.sse_mul(rn);
                let s = T::ONE.sse_div(rn);
                for i in 0..n {
                    tmp[i * n + j] = tmp[i * n + j] * s;
                }
            }
        }
    }
    // `T(scalings) * balanced_det`, the operands in the other order in both instances
    crate::vnl_qr::qr_determinant(&tmp, n).sse_mul(scalings)
}

/// `vnl_determinant<T>(M)` of an `n × n` matrix (row-major): the closed forms up to 4 rows,
/// then the QR determinant (`balance` false, VNL's default).
pub fn determinant<T: ItkReal>(m: &[T], n: usize) -> T {
    assert_eq!(m.len(), n * n, "not an n × n matrix");
    match n {
        1 => m[0],
        2 => det2(&[[m[0], m[1]], [m[2], m[3]]]),
        3 => det3(&[[m[0], m[1], m[2]], [m[3], m[4], m[5]], [m[6], m[7], m[8]]]),
        4 => det4(&std::array::from_fn(|i| {
            std::array::from_fn(|j| m[i * 4 + j])
        })),
        _ => crate::vnl_qr::qr_determinant(m, n),
    }
}

/// `vnl_inverse(vnl_matrix_fixed<T, 2, 2>)`: the zero matrix for a zero determinant (VNL returns
/// an uninitialised matrix, its assertion compiled out: ITK-107).
pub fn inverse2<T: ItkReal>(m: &[[T; 2]; 2]) -> [[T; 2]; 2] {
    let det = det2(m);
    if det == T::ZERO {
        return [[T::ZERO; 2]; 2];
    }
    T::vnl_inverse2(m, det)
}

/// `vnl_inverse(vnl_matrix_fixed<T, 3, 3>)`.
pub fn inverse3<T: ItkReal>(m: &[[T; 3]; 3]) -> [[T; 3]; 3] {
    let det = det3(m);
    if det == T::ZERO {
        return [[T::ZERO; 3]; 3];
    }
    sse::inverse3(m, det)
}

/// `vnl_inverse(vnl_matrix_fixed<T, 4, 4>)`: the adjugate's entries, then the matrix times
/// `1 / det` (`vnl_matrix_fixed::operator*`, whose last row takes the operands the other way).
pub fn inverse4<T: ItkReal>(m: &[[T; 4]; 4]) -> [[T; 4]; 4] {
    let det = det4(m);
    if det == T::ZERO {
        return [[T::ZERO; 4]; 4];
    }
    T::vnl_inverse4(m, det)
}

/// `vnl_inverse` of an `n × n` `vnl_matrix` (row-major), `n` from 1 to 4.
pub fn inverse<T: ItkReal>(m: &[T], n: usize) -> Vec<T> {
    assert_eq!(m.len(), n * n, "not an n × n matrix");
    match n {
        1 => vec![T::ONE.sse_div(m[0])],
        2 => inverse2(&[[m[0], m[1]], [m[2], m[3]]])
            .iter()
            .flatten()
            .copied()
            .collect(),
        3 => inverse3(&[[m[0], m[1], m[2]], [m[3], m[4], m[5]], [m[6], m[7], m[8]]])
            .iter()
            .flatten()
            .copied()
            .collect(),
        4 => inverse4(&std::array::from_fn(|i| {
            std::array::from_fn(|j| m[i * 4 + j])
        }))
        .iter()
        .flatten()
        .copied()
        .collect(),
        _ => panic!("vnl_inverse takes at most 4 rows (n = {n})"),
    }
}

use crate::itk_real::ItkReal;

#[cfg(test)]
#[allow(clippy::needless_range_loop)]
mod tests {
    use super::*;

    #[test]
    fn inverses_invert() {
        let m3 = [[2.0, 0.3, -0.1], [0.2, 1.5, 0.4], [-0.3, 0.1, 3.2]];
        let i3 = inverse3(&m3);
        for r in 0..3 {
            for c in 0..3 {
                let x: f64 = (0..3).map(|k| m3[r][k] * i3[k][c]).sum();
                assert!((x - if r == c { 1.0 } else { 0.0 }).abs() < 1e-15);
            }
        }
        let m4 = [
            [1.0, 0.1, 0.0, 0.2],
            [-0.2, 1.3, 0.05, 0.0],
            [0.0, 0.3, 0.8, 0.1],
            [0.1, 0.0, 0.0, 2.0],
        ];
        let i4 = inverse4(&m4);
        for r in 0..4 {
            for c in 0..4 {
                let x: f64 = (0..4).map(|k| m4[r][k] * i4[k][c]).sum();
                assert!((x - if r == c { 1.0 } else { 0.0 }).abs() < 1e-14);
            }
        }
        assert_eq!(
            inverse2(&[[2.0, 0.0], [0.0, 4.0]]),
            [[0.5, 0.0], [-0.0, 0.25]]
        );
        assert_eq!(det3(&m3), determinant(&m3.concat(), 3));
        assert_eq!(det4(&m4), determinant(&m4.concat(), 4));
    }
}
