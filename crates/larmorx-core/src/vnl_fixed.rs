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
//! the compiler's choice. The private module `sse` holds the expressions with the operands in
//! the order GCC 8.4.0 gives ITK 4.13.2's instances (read from their object code), each operation
//! through [`ItkReal::sse_add`] and its kin, so NaN results carry x86-64's signs and payloads on
//! every platform. The `float` and `double` instances share the determinants' and the 3 x 3
//! inverse's order; the 2 x 2 and 4 x 4 inverses have one each ([`ItkReal::vnl_inverse2`]).
//!
//! The functions are generic over `float` and `double` ([`ItkReal`]), as `vnl_matrix_fixed<T, n,
//! n>` is; each operation is in that type (ITK 4.13.2's `vnl_det.hxx` and `vnl_inverse.h`, the
//! same expressions as 5.4.5's). [`determinant`] of more than 4 rows is
//! `vnl_determinant<T>`'s QR determinant ([`crate::vnl_qr::Qr`]), and
//! [`determinant_balanced`] its `balance` form (their NaNs follow `vnl_qr`'s port, not checked
//! against x86-64's). `vnl_inverse` of a matrix whose determinant is 0 returns VNL's
//! uninitialised `vnl_matrix_fixed` (its assertion is compiled out): memory contents, given here
//! as the zero matrix (ITK-107).

#[doc(hidden)]
pub mod cases;
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

    /// Where two NaNs meet, the result is the first operand's: `[[A, 1], [1, B]]` has the
    /// determinant `A·B - 1 = A`, and its inverse's first entry is `m(1,1) · (1/det)` in the
    /// `float` instance (`B`) and `(1/det) · m(1,1)` in the `double` one (`A`).
    #[test]
    fn the_float_and_double_inverses_take_their_own_operand_order() {
        let (a, b) = (f32::from_bits(0x7fc0_0001), f32::from_bits(0xffc0_0002));
        let i = inverse2(&[[a, 1.0], [1.0, b]]);
        assert_eq!(det2(&[[a, 1.0], [1.0, b]]).to_bits(), 0x7fc0_0001);
        assert_eq!(
            i.concat().iter().map(|x| x.to_bits()).collect::<Vec<_>>(),
            [0xffc0_0002, 0x7fc0_0001, 0x7fc0_0001, 0x7fc0_0001]
        );
        let (a, b) = (
            f64::from_bits(0x7ff8_0000_0000_0001),
            f64::from_bits(0xfff8_0000_0000_0002),
        );
        let i = inverse2(&[[a, 1.0], [1.0, b]]);
        assert_eq!(i[0][0].to_bits(), 0x7ff8_0000_0000_0001);
        // a NaN made from numbers is x86-64's (sign set), not AArch64's
        let inf = f32::INFINITY;
        assert_eq!(det2(&[[inf, inf], [1.0, 1.0]]).to_bits(), 0xffc0_0000);
    }

    /// The closed forms' NaNs, bit for bit, on every platform: a digest of the determinants and
    /// inverses of `cases`' matrices (NaN signs and payloads hashed exactly), recorded on Linux
    /// x86-64, where the probe `validation/probes/fs-vnl` (family `order`) shows every value
    /// identical to VNL's own instances. `LARMORX_PRINT_DIGESTS=1` prints it.
    #[test]
    fn nan_bits_are_x86_64s() {
        use crate::x87::cases::SplitMix64;
        let mut rng = SplitMix64::new(0x0de7_0bad_5eed);
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        let mut word = |w: u64| {
            for b in w.to_le_bytes() {
                h = (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
            }
        };
        let mut nans = 0;
        for n in 1..=4 {
            for _ in 0..2000 {
                let m = cases::matrix_f32(&mut rng, n);
                let d = determinant(&m, n);
                word(u64::from(d.to_bits()));
                if d != 0.0 {
                    for x in inverse(&m, n) {
                        nans += usize::from(x.is_nan());
                        word(u64::from(x.to_bits()));
                    }
                }
                let m = cases::matrix_f64(&mut rng, n);
                let d = determinant(&m, n);
                word(d.to_bits());
                if d != 0.0 {
                    for x in inverse(&m, n) {
                        nans += usize::from(x.is_nan());
                        word(x.to_bits());
                    }
                }
            }
        }
        if std::env::var_os("LARMORX_PRINT_DIGESTS").is_some() {
            println!("NAN_BITS = {h:#018x} ({nans} NaN entries of inverses)");
        }
        assert_eq!(h, NAN_BITS);
    }

    const NAN_BITS: u64 = 0x39ff_7891_bbbd_e925;
}
