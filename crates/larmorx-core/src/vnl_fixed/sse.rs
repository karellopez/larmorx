// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! GENERATED: do not edit (`validation/probes/fs-vnl/order/orders.py --rust`).
//!
//! VNL's closed-form determinants and inverses (ITK 4.13.2's `vnl_det.hxx`, `vnl_inverse.h`),
//! every `+`, `-`, `*`, `/` with its operands in the order x86-64 receives them from GCC 8.4.0
//! at `-O3 -msse2 -mfpmath=sse` (the first operand is SSE's first source, whose NaN wins), read
//! from the object code. The values are the source's: only `+` and `*` have their operands
//! swapped, which changes nothing but the NaN passed on where two meet. The `sse_*` operations
//! give x86-64's NaN on every platform ([`ItkReal::sse_add`]).
//! Read from g++-8 (Ubuntu 8.4.0-3ubuntu2) 8.4.0's objects of ITK 4.13.2.

use crate::itk_real::ItkReal;

/// `vnl_det` of 2 rows.
#[rustfmt::skip]
pub(super) fn det2<T: ItkReal>(r0: &[T; 2], r1: &[T; 2]) -> T {
    let (sub, mul) = (T::sse_sub, T::sse_mul);
    sub(mul(r0[0], r1[1]), mul(r0[1], r1[0]))
}

/// `vnl_det` of 3 rows.
#[rustfmt::skip]
pub(super) fn det3<T: ItkReal>(r0: &[T; 3], r1: &[T; 3], r2: &[T; 3]) -> T {
    let (add, sub, mul) = (T::sse_add, T::sse_sub, T::sse_mul);
    let s = sub(mul(mul(r0[0], r1[1]), r2[2]), mul(mul(r0[0], r2[1]), r1[2]));
    let s = sub(s, mul(mul(r1[0], r0[1]), r2[2]));
    let s = add(s, mul(mul(r2[1], r1[0]), r0[2]));
    let s = add(s, mul(mul(r0[1], r2[0]), r1[2]));
    sub(s, mul(mul(r1[1], r2[0]), r0[2]))
}

/// `vnl_det` of 4 rows.
#[rustfmt::skip]
pub(super) fn det4<T: ItkReal>(r0: &[T; 4], r1: &[T; 4], r2: &[T; 4], r3: &[T; 4]) -> T {
    let (add, sub, mul) = (T::sse_add, T::sse_sub, T::sse_mul);
    let s = sub(mul(mul(mul(r0[0], r1[1]), r2[2]), r3[3]), mul(mul(mul(r0[0], r1[1]), r3[2]), r2[3]));
    let s = sub(s, mul(mul(mul(r0[0], r2[1]), r1[2]), r3[3]));
    let s = add(s, mul(mul(mul(r0[0], r2[1]), r3[2]), r1[3]));
    let s = add(s, mul(mul(r1[2], mul(r0[0], r3[1])), r2[3]));
    let s = sub(s, mul(mul(mul(r0[0], r3[1]), r2[2]), r1[3]));
    let s = sub(s, mul(mul(mul(r1[0], r0[1]), r2[2]), r3[3]));
    let s = add(s, mul(mul(mul(r1[0], r0[1]), r3[2]), r2[3]));
    let s = add(s, mul(mul(mul(r2[1], r1[0]), r0[2]), r3[3]));
    let s = sub(s, mul(mul(mul(r2[1], r1[0]), r3[2]), r0[3]));
    let s = sub(s, mul(mul(mul(r1[0], r3[1]), r0[2]), r2[3]));
    let s = add(s, mul(mul(mul(r1[0], r3[1]), r2[2]), r0[3]));
    let s = add(s, mul(mul(mul(r0[1], r2[0]), r1[2]), r3[3]));
    let s = sub(s, mul(mul(mul(r0[1], r2[0]), r3[2]), r1[3]));
    let s = sub(s, mul(r3[3], mul(mul(r1[1], r2[0]), r0[2])));
    let s = add(mul(mul(r3[2], mul(r1[1], r2[0])), r0[3]), s);
    let s = add(s, mul(mul(mul(r3[1], r2[0]), r0[2]), r1[3]));
    let s = sub(s, mul(mul(mul(r3[1], r2[0]), r1[2]), r0[3]));
    let s = sub(s, mul(mul(r1[2], mul(r0[1], r3[0])), r2[3]));
    let s = add(mul(mul(mul(r0[1], r3[0]), r2[2]), r1[3]), s);
    let s = add(mul(r2[3], mul(r0[2], mul(r1[1], r3[0]))), s);
    let s = sub(s, mul(mul(mul(r1[1], r3[0]), r2[2]), r0[3]));
    let s = sub(s, mul(r1[3], mul(r0[2], mul(r2[1], r3[0]))));
    add(mul(mul(mul(r2[1], r3[0]), r1[2]), r0[3]), s)
}

/// `vnl_inverse` of a 2 x 2 matrix whose determinant `det` is not 0: `1 / det`, then
/// the adjugate's entries times it; the order of the `float` instance.
#[rustfmt::skip]
pub(crate) fn inverse2_f32<T: ItkReal>(a: &[[T; 2]; 2], det: T) -> [[T; 2]; 2] {
    let (mul, div) = (T::sse_mul, T::sse_div);
    let m = |i: usize, j: usize| a[i][j];
    let recip = div(T::ONE, det);
    let d: [T; 4] = [
        mul(m(1, 1), recip), // [0][0]
        mul(-m(0, 1), recip), // [0][1]
        mul(-m(1, 0), recip), // [1][0]
        mul(m(0, 0), recip), // [1][1]
    ];
    std::array::from_fn(|i| std::array::from_fn(|j| d[i * 2 + j]))
}

/// `vnl_inverse` of a 2 x 2 matrix whose determinant `det` is not 0: `1 / det`, then
/// the adjugate's entries times it; the order of the `double` instance.
#[rustfmt::skip]
pub(crate) fn inverse2_f64<T: ItkReal>(a: &[[T; 2]; 2], det: T) -> [[T; 2]; 2] {
    let (mul, div) = (T::sse_mul, T::sse_div);
    let m = |i: usize, j: usize| a[i][j];
    let recip = div(T::ONE, det);
    let d: [T; 4] = [
        mul(recip, m(1, 1)), // [0][0]
        mul(-m(0, 1), recip), // [0][1]
        mul(recip, -m(1, 0)), // [1][0]
        mul(m(0, 0), recip), // [1][1]
    ];
    std::array::from_fn(|i| std::array::from_fn(|j| d[i * 2 + j]))
}

/// `vnl_inverse` of a 3 x 3 matrix whose determinant `det` is not 0: `1 / det`, then
/// the adjugate's entries times it; the order of the `float` and `double` instances (the same).
#[rustfmt::skip]
pub(crate) fn inverse3<T: ItkReal>(a: &[[T; 3]; 3], det: T) -> [[T; 3]; 3] {
    let (sub, mul, div) = (T::sse_sub, T::sse_mul, T::sse_div);
    let m = |i: usize, j: usize| a[i][j];
    let recip = div(T::ONE, det);
    let d: [T; 9] = [
        mul(sub(mul(m(1, 1), m(2, 2)), mul(m(1, 2), m(2, 1))), recip), // [0][0]
        mul(sub(mul(m(2, 1), m(0, 2)), mul(m(2, 2), m(0, 1))), recip), // [0][1]
        mul(sub(mul(m(1, 2), m(0, 1)), mul(m(1, 1), m(0, 2))), recip), // [0][2]
        mul(sub(mul(m(1, 2), m(2, 0)), mul(m(2, 2), m(1, 0))), recip), // [1][0]
        mul(sub(mul(m(2, 2), m(0, 0)), mul(m(0, 2), m(2, 0))), recip), // [1][1]
        mul(sub(mul(m(0, 2), m(1, 0)), mul(m(1, 2), m(0, 0))), recip), // [1][2]
        mul(sub(mul(m(2, 1), m(1, 0)), mul(m(1, 1), m(2, 0))), recip), // [2][0]
        mul(sub(mul(m(2, 0), m(0, 1)), mul(m(2, 1), m(0, 0))), recip), // [2][1]
        mul(recip, sub(mul(m(1, 1), m(0, 0)), mul(m(0, 1), m(1, 0)))), // [2][2]
    ];
    std::array::from_fn(|i| std::array::from_fn(|j| d[i * 3 + j]))
}

/// `vnl_inverse` of a 4 x 4 matrix whose determinant `det` is not 0: `1 / det`, then
/// the adjugate's entries times it; the order of the `float` instance.
#[rustfmt::skip]
pub(crate) fn inverse4_f32<T: ItkReal>(a: &[[T; 4]; 4], det: T) -> [[T; 4]; 4] {
    let (add, sub, mul, div) = (T::sse_add, T::sse_sub, T::sse_mul, T::sse_div);
    let m = |i: usize, j: usize| a[i][j];
    let recip = div(T::ONE, det);
    let d: [T; 16] = [
        { // [0][0]
            let s = sub(mul(mul(m(1, 1), m(2, 2)), m(3, 3)), mul(mul(m(1, 1), m(2, 3)), m(3, 2)));
            let s = sub(s, mul(mul(m(2, 1), m(1, 2)), m(3, 3)));
            let s = add(s, mul(mul(m(2, 1), m(1, 3)), m(3, 2)));
            let s = add(s, mul(mul(m(1, 2), m(3, 1)), m(2, 3)));
            let s = sub(s, mul(mul(m(1, 3), m(3, 1)), m(2, 2)));
            mul(s, recip)
        },
        { // [0][1]
            let s = add(mul(mul(-m(0, 1), m(2, 2)), m(3, 3)), mul(mul(m(2, 3), m(0, 1)), m(3, 2)));
            let s = add(mul(mul(m(2, 1), m(0, 2)), m(3, 3)), s);
            let s = sub(s, mul(mul(m(2, 1), m(0, 3)), m(3, 2)));
            let s = sub(s, mul(m(2, 3), mul(m(3, 1), m(0, 2))));
            let s = add(s, mul(mul(m(3, 1), m(0, 3)), m(2, 2)));
            mul(s, recip)
        },
        { // [0][2]
            let s = sub(mul(mul(m(0, 1), m(1, 2)), m(3, 3)), mul(mul(m(1, 3), m(0, 1)), m(3, 2)));
            let s = sub(s, mul(mul(m(1, 1), m(0, 2)), m(3, 3)));
            let s = add(s, mul(mul(m(1, 1), m(0, 3)), m(3, 2)));
            let s = add(s, mul(mul(m(3, 1), m(0, 2)), m(1, 3)));
            let s = sub(s, mul(mul(m(3, 1), m(0, 3)), m(1, 2)));
            mul(s, recip)
        },
        { // [0][3]
            let s = add(mul(mul(-m(0, 1), m(1, 2)), m(2, 3)), mul(mul(m(1, 3), m(0, 1)), m(2, 2)));
            let s = add(s, mul(mul(m(1, 1), m(0, 2)), m(2, 3)));
            let s = sub(s, mul(mul(m(1, 1), m(0, 3)), m(2, 2)));
            let s = sub(s, mul(mul(m(2, 1), m(0, 2)), m(1, 3)));
            let s = add(s, mul(mul(m(2, 1), m(0, 3)), m(1, 2)));
            mul(s, recip)
        },
        { // [1][0]
            let s = add(mul(mul(-m(1, 0), m(2, 2)), m(3, 3)), mul(mul(m(1, 0), m(2, 3)), m(3, 2)));
            let s = add(mul(mul(m(1, 2), m(2, 0)), m(3, 3)), s);
            let s = sub(s, mul(mul(m(1, 3), m(2, 0)), m(3, 2)));
            let s = sub(s, mul(mul(m(1, 2), m(3, 0)), m(2, 3)));
            let s = add(s, mul(mul(m(1, 3), m(3, 0)), m(2, 2)));
            mul(s, recip)
        },
        { // [1][1]
            let s = sub(mul(mul(m(0, 0), m(2, 2)), m(3, 3)), mul(mul(m(0, 0), m(2, 3)), m(3, 2)));
            let s = sub(s, mul(mul(m(0, 2), m(2, 0)), m(3, 3)));
            let s = add(mul(mul(m(0, 3), m(2, 0)), m(3, 2)), s);
            let s = add(mul(mul(m(0, 2), m(3, 0)), m(2, 3)), s);
            let s = sub(s, mul(mul(m(0, 3), m(3, 0)), m(2, 2)));
            mul(s, recip)
        },
        { // [1][2]
            let s = add(mul(mul(-m(0, 0), m(1, 2)), m(3, 3)), mul(m(3, 2), mul(m(0, 0), m(1, 3))));
            let s = add(mul(mul(m(0, 2), m(1, 0)), m(3, 3)), s);
            let s = sub(s, mul(m(3, 2), mul(m(0, 3), m(1, 0))));
            let s = sub(s, mul(mul(m(0, 2), m(3, 0)), m(1, 3)));
            let s = add(s, mul(mul(m(0, 3), m(3, 0)), m(1, 2)));
            mul(s, recip)
        },
        { // [1][3]
            let s = sub(mul(mul(m(0, 0), m(1, 2)), m(2, 3)), mul(m(2, 2), mul(m(0, 0), m(1, 3))));
            let s = sub(s, mul(mul(m(0, 2), m(1, 0)), m(2, 3)));
            let s = add(mul(m(2, 2), mul(m(0, 3), m(1, 0))), s);
            let s = add(mul(mul(m(0, 2), m(2, 0)), m(1, 3)), s);
            let s = sub(s, mul(mul(m(0, 3), m(2, 0)), m(1, 2)));
            mul(s, recip)
        },
        { // [2][0]
            let s = sub(mul(mul(m(1, 0), m(2, 1)), m(3, 3)), mul(mul(m(1, 0), m(2, 3)), m(3, 1)));
            let s = sub(s, mul(mul(m(1, 1), m(2, 0)), m(3, 3)));
            let s = add(s, mul(mul(m(1, 3), m(2, 0)), m(3, 1)));
            let s = add(s, mul(mul(m(1, 1), m(3, 0)), m(2, 3)));
            let s = sub(s, mul(mul(m(1, 3), m(3, 0)), m(2, 1)));
            mul(s, recip)
        },
        { // [2][1]
            let s = add(mul(mul(m(0, 0), m(2, 3)), m(3, 1)), mul(mul(-m(0, 0), m(2, 1)), m(3, 3)));
            let s = add(mul(mul(m(2, 0), m(0, 1)), m(3, 3)), s);
            let s = sub(s, mul(mul(m(0, 3), m(2, 0)), m(3, 1)));
            let s = sub(s, mul(m(2, 3), mul(m(3, 0), m(0, 1))));
            let s = add(s, mul(mul(m(0, 3), m(3, 0)), m(2, 1)));
            mul(s, recip)
        },
        { // [2][2]
            let s = sub(mul(mul(m(0, 0), m(1, 1)), m(3, 3)), mul(m(3, 1), mul(m(0, 0), m(1, 3))));
            let s = sub(s, mul(m(3, 3), mul(m(1, 0), m(0, 1))));
            let s = add(s, mul(m(3, 1), mul(m(0, 3), m(1, 0))));
            let s = add(s, mul(m(1, 3), mul(m(3, 0), m(0, 1))));
            let s = sub(s, mul(mul(m(0, 3), m(3, 0)), m(1, 1)));
            mul(s, recip)
        },
        { // [2][3]
            let s = add(mul(mul(m(0, 0), m(1, 3)), m(2, 1)), mul(mul(-m(0, 0), m(1, 1)), m(2, 3)));
            let s = add(s, mul(m(2, 3), mul(m(1, 0), m(0, 1))));
            let s = sub(s, mul(mul(m(0, 3), m(1, 0)), m(2, 1)));
            let s = sub(s, mul(m(1, 3), mul(m(2, 0), m(0, 1))));
            let s = add(s, mul(mul(m(0, 3), m(2, 0)), m(1, 1)));
            mul(s, recip)
        },
        { // [3][0]
            let s = add(mul(mul(m(1, 0), m(2, 2)), m(3, 1)), mul(mul(-m(1, 0), m(2, 1)), m(3, 2)));
            let s = add(s, mul(mul(m(1, 1), m(2, 0)), m(3, 2)));
            let s = sub(s, mul(mul(m(1, 2), m(2, 0)), m(3, 1)));
            let s = sub(s, mul(mul(m(1, 1), m(3, 0)), m(2, 2)));
            let s = add(s, mul(mul(m(1, 2), m(3, 0)), m(2, 1)));
            mul(recip, s)
        },
        { // [3][1]
            let s = sub(mul(mul(m(0, 0), m(2, 1)), m(3, 2)), mul(mul(m(0, 0), m(2, 2)), m(3, 1)));
            let s = sub(s, mul(mul(m(2, 0), m(0, 1)), m(3, 2)));
            let s = add(mul(mul(m(0, 2), m(2, 0)), m(3, 1)), s);
            let s = add(mul(m(2, 2), mul(m(3, 0), m(0, 1))), s);
            let s = sub(s, mul(mul(m(0, 2), m(3, 0)), m(2, 1)));
            mul(recip, s)
        },
        { // [3][2]
            let s = add(mul(mul(m(0, 0), m(1, 2)), m(3, 1)), mul(mul(-m(0, 0), m(1, 1)), m(3, 2)));
            let s = add(s, mul(m(3, 2), mul(m(1, 0), m(0, 1))));
            let s = sub(s, mul(m(3, 1), mul(m(0, 2), m(1, 0))));
            let s = sub(s, mul(mul(m(3, 0), m(0, 1)), m(1, 2)));
            let s = add(s, mul(mul(m(0, 2), m(3, 0)), m(1, 1)));
            mul(recip, s)
        },
        { // [3][3]
            let s = sub(mul(mul(m(0, 0), m(1, 1)), m(2, 2)), mul(mul(m(0, 0), m(1, 2)), m(2, 1)));
            let s = sub(s, mul(m(2, 2), mul(m(1, 0), m(0, 1))));
            let s = add(mul(m(2, 1), mul(m(0, 2), m(1, 0))), s);
            let s = add(mul(m(1, 2), mul(m(2, 0), m(0, 1))), s);
            let s = sub(s, mul(m(1, 1), mul(m(0, 2), m(2, 0))));
            mul(recip, s)
        },
    ];
    std::array::from_fn(|i| std::array::from_fn(|j| d[i * 4 + j]))
}

/// `vnl_inverse` of a 4 x 4 matrix whose determinant `det` is not 0: `1 / det`, then
/// the adjugate's entries times it; the order of the `double` instance.
#[rustfmt::skip]
pub(crate) fn inverse4_f64<T: ItkReal>(a: &[[T; 4]; 4], det: T) -> [[T; 4]; 4] {
    let (add, sub, mul, div) = (T::sse_add, T::sse_sub, T::sse_mul, T::sse_div);
    let m = |i: usize, j: usize| a[i][j];
    let recip = div(T::ONE, det);
    let d: [T; 16] = [
        { // [0][0]
            let s = sub(mul(mul(m(1, 1), m(2, 2)), m(3, 3)), mul(mul(m(1, 1), m(2, 3)), m(3, 2)));
            let s = sub(s, mul(mul(m(2, 1), m(1, 2)), m(3, 3)));
            let s = add(s, mul(mul(m(2, 1), m(1, 3)), m(3, 2)));
            let s = add(s, mul(mul(m(1, 2), m(3, 1)), m(2, 3)));
            let s = sub(s, mul(mul(m(1, 3), m(3, 1)), m(2, 2)));
            mul(s, recip)
        },
        { // [0][1]
            let s = add(mul(mul(-m(0, 1), m(2, 2)), m(3, 3)), mul(mul(m(2, 3), m(0, 1)), m(3, 2)));
            let s = add(mul(mul(m(2, 1), m(0, 2)), m(3, 3)), s);
            let s = sub(s, mul(mul(m(2, 1), m(0, 3)), m(3, 2)));
            let s = sub(s, mul(m(2, 3), mul(m(3, 1), m(0, 2))));
            let s = add(s, mul(mul(m(3, 1), m(0, 3)), m(2, 2)));
            mul(s, recip)
        },
        { // [0][2]
            let s = sub(mul(mul(m(0, 1), m(1, 2)), m(3, 3)), mul(mul(m(1, 3), m(0, 1)), m(3, 2)));
            let s = sub(s, mul(mul(m(1, 1), m(0, 2)), m(3, 3)));
            let s = add(s, mul(mul(m(1, 1), m(0, 3)), m(3, 2)));
            let s = add(s, mul(mul(m(3, 1), m(0, 2)), m(1, 3)));
            let s = sub(s, mul(mul(m(3, 1), m(0, 3)), m(1, 2)));
            mul(s, recip)
        },
        { // [0][3]
            let s = add(mul(mul(-m(0, 1), m(1, 2)), m(2, 3)), mul(mul(m(1, 3), m(0, 1)), m(2, 2)));
            let s = add(s, mul(mul(m(1, 1), m(0, 2)), m(2, 3)));
            let s = sub(s, mul(mul(m(1, 1), m(0, 3)), m(2, 2)));
            let s = sub(s, mul(mul(m(2, 1), m(0, 2)), m(1, 3)));
            let s = add(s, mul(mul(m(2, 1), m(0, 3)), m(1, 2)));
            mul(s, recip)
        },
        { // [1][0]
            let s = add(mul(mul(-m(1, 0), m(2, 2)), m(3, 3)), mul(mul(m(1, 0), m(2, 3)), m(3, 2)));
            let s = add(mul(mul(m(1, 2), m(2, 0)), m(3, 3)), s);
            let s = sub(s, mul(mul(m(1, 3), m(2, 0)), m(3, 2)));
            let s = sub(s, mul(mul(m(1, 2), m(3, 0)), m(2, 3)));
            let s = add(s, mul(mul(m(1, 3), m(3, 0)), m(2, 2)));
            mul(s, recip)
        },
        { // [1][1]
            let s = sub(mul(mul(m(0, 0), m(2, 2)), m(3, 3)), mul(mul(m(0, 0), m(2, 3)), m(3, 2)));
            let s = sub(s, mul(mul(m(0, 2), m(2, 0)), m(3, 3)));
            let s = add(mul(mul(m(0, 3), m(2, 0)), m(3, 2)), s);
            let s = add(mul(mul(m(0, 2), m(3, 0)), m(2, 3)), s);
            let s = sub(s, mul(mul(m(0, 3), m(3, 0)), m(2, 2)));
            mul(s, recip)
        },
        { // [1][2]
            let s = add(mul(mul(-m(0, 0), m(1, 2)), m(3, 3)), mul(m(3, 2), mul(m(0, 0), m(1, 3))));
            let s = add(mul(mul(m(0, 2), m(1, 0)), m(3, 3)), s);
            let s = sub(s, mul(m(3, 2), mul(m(0, 3), m(1, 0))));
            let s = sub(s, mul(mul(m(0, 2), m(3, 0)), m(1, 3)));
            let s = add(s, mul(mul(m(0, 3), m(3, 0)), m(1, 2)));
            mul(s, recip)
        },
        { // [1][3]
            let s = sub(mul(mul(m(0, 0), m(1, 2)), m(2, 3)), mul(m(2, 2), mul(m(0, 0), m(1, 3))));
            let s = sub(s, mul(mul(m(0, 2), m(1, 0)), m(2, 3)));
            let s = add(mul(m(2, 2), mul(m(0, 3), m(1, 0))), s);
            let s = add(mul(mul(m(0, 2), m(2, 0)), m(1, 3)), s);
            let s = sub(s, mul(mul(m(0, 3), m(2, 0)), m(1, 2)));
            mul(s, recip)
        },
        { // [2][0]
            let s = sub(mul(mul(m(1, 0), m(2, 1)), m(3, 3)), mul(mul(m(1, 0), m(2, 3)), m(3, 1)));
            let s = sub(s, mul(mul(m(1, 1), m(2, 0)), m(3, 3)));
            let s = add(s, mul(mul(m(1, 3), m(2, 0)), m(3, 1)));
            let s = add(s, mul(mul(m(1, 1), m(3, 0)), m(2, 3)));
            let s = sub(s, mul(mul(m(1, 3), m(3, 0)), m(2, 1)));
            mul(s, recip)
        },
        { // [2][1]
            let s = add(mul(mul(m(0, 0), m(2, 3)), m(3, 1)), mul(mul(-m(0, 0), m(2, 1)), m(3, 3)));
            let s = add(mul(mul(m(2, 0), m(0, 1)), m(3, 3)), s);
            let s = sub(s, mul(mul(m(0, 3), m(2, 0)), m(3, 1)));
            let s = sub(s, mul(m(2, 3), mul(m(3, 0), m(0, 1))));
            let s = add(s, mul(mul(m(0, 3), m(3, 0)), m(2, 1)));
            mul(s, recip)
        },
        { // [2][2]
            let s = sub(mul(mul(m(0, 0), m(1, 1)), m(3, 3)), mul(m(3, 1), mul(m(0, 0), m(1, 3))));
            let s = sub(s, mul(m(3, 3), mul(m(1, 0), m(0, 1))));
            let s = add(s, mul(m(3, 1), mul(m(0, 3), m(1, 0))));
            let s = add(s, mul(m(1, 3), mul(m(3, 0), m(0, 1))));
            let s = sub(s, mul(mul(m(0, 3), m(3, 0)), m(1, 1)));
            mul(s, recip)
        },
        { // [2][3]
            let s = add(mul(mul(m(0, 0), m(1, 3)), m(2, 1)), mul(mul(-m(0, 0), m(1, 1)), m(2, 3)));
            let s = add(s, mul(m(2, 3), mul(m(1, 0), m(0, 1))));
            let s = sub(s, mul(mul(m(0, 3), m(1, 0)), m(2, 1)));
            let s = sub(s, mul(m(1, 3), mul(m(2, 0), m(0, 1))));
            let s = add(s, mul(mul(m(0, 3), m(2, 0)), m(1, 1)));
            mul(s, recip)
        },
        { // [3][0]
            let s = add(mul(mul(m(1, 0), m(2, 2)), m(3, 1)), mul(mul(-m(1, 0), m(2, 1)), m(3, 2)));
            let s = add(s, mul(mul(m(1, 1), m(2, 0)), m(3, 2)));
            let s = sub(s, mul(mul(m(1, 2), m(2, 0)), m(3, 1)));
            let s = sub(s, mul(mul(m(1, 1), m(3, 0)), m(2, 2)));
            let s = add(s, mul(mul(m(1, 2), m(3, 0)), m(2, 1)));
            mul(s, recip)
        },
        { // [3][1]
            let s = sub(mul(mul(m(0, 0), m(2, 1)), m(3, 2)), mul(mul(m(0, 0), m(2, 2)), m(3, 1)));
            let s = sub(s, mul(mul(m(2, 0), m(0, 1)), m(3, 2)));
            let s = add(mul(mul(m(0, 2), m(2, 0)), m(3, 1)), s);
            let s = add(mul(m(2, 2), mul(m(3, 0), m(0, 1))), s);
            let s = sub(s, mul(mul(m(0, 2), m(3, 0)), m(2, 1)));
            mul(s, recip)
        },
        { // [3][2]
            let s = add(mul(mul(m(0, 0), m(1, 2)), m(3, 1)), mul(mul(-m(0, 0), m(1, 1)), m(3, 2)));
            let s = add(s, mul(m(3, 2), mul(m(1, 0), m(0, 1))));
            let s = sub(s, mul(m(3, 1), mul(m(0, 2), m(1, 0))));
            let s = sub(s, mul(mul(m(3, 0), m(0, 1)), m(1, 2)));
            let s = add(s, mul(mul(m(0, 2), m(3, 0)), m(1, 1)));
            mul(recip, s)
        },
        { // [3][3]
            let s = sub(mul(mul(m(0, 0), m(1, 1)), m(2, 2)), mul(mul(m(0, 0), m(1, 2)), m(2, 1)));
            let s = sub(s, mul(m(2, 2), mul(m(1, 0), m(0, 1))));
            let s = add(mul(m(2, 1), mul(m(0, 2), m(1, 0))), s);
            let s = add(mul(m(1, 2), mul(m(2, 0), m(0, 1))), s);
            let s = sub(s, mul(m(1, 1), mul(m(0, 2), m(2, 0))));
            mul(recip, s)
        },
    ];
    std::array::from_fn(|i| std::array::from_fn(|j| d[i * 4 + j]))
}
