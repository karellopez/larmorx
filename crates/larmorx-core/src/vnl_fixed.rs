// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! vnl's closed-form determinants and inverses of small matrices, as written in VXL as
//! bundled with ITK v5.4.5 (BSD licence; notice in [`crate::vnl_svd`]):
//! `core/vnl/vnl_det.hxx` (`vnl_det`), `core/vnl/algo/vnl_determinant.hxx`
//! (`vnl_determinant` of 1 to 4 rows, which uses the same expressions), `core/vnl/vnl_inverse.h`
//! (`vnl_inverse` of `vnl_matrix_fixed<T, 2..4, 2..4>` and of a `vnl_matrix` of up to 4 rows).
//!
//! Each expression is evaluated left to right as C++ evaluates it (no fused multiply-adds),
//! so the results are bit-identical to vnl's on every platform.

/// `vnl_det(row0, row1)`.
#[inline]
pub fn det2(m: &[[f64; 2]; 2]) -> f64 {
    m[0][0] * m[1][1] - m[0][1] * m[1][0]
}

/// `vnl_det(row0, row1, row2)`.
#[inline]
pub fn det3(m: &[[f64; 3]; 3]) -> f64 {
    let (r0, r1, r2) = (&m[0], &m[1], &m[2]);
    r0[0] * r1[1] * r2[2] - r0[0] * r2[1] * r1[2] - r1[0] * r0[1] * r2[2]
        + r1[0] * r2[1] * r0[2]
        + r2[0] * r0[1] * r1[2]
        - r2[0] * r1[1] * r0[2]
}

/// `vnl_det(row0, row1, row2, row3)`.
#[rustfmt::skip]
pub fn det4(m: &[[f64; 4]; 4]) -> f64 {
    let (r0, r1, r2, r3) = (&m[0], &m[1], &m[2], &m[3]);
    r0[0]*r1[1]*r2[2]*r3[3]
    - r0[0]*r1[1]*r3[2]*r2[3]
    - r0[0]*r2[1]*r1[2]*r3[3]
    + r0[0]*r2[1]*r3[2]*r1[3]
    + r0[0]*r3[1]*r1[2]*r2[3]
    - r0[0]*r3[1]*r2[2]*r1[3]
    - r1[0]*r0[1]*r2[2]*r3[3]
    + r1[0]*r0[1]*r3[2]*r2[3]
    + r1[0]*r2[1]*r0[2]*r3[3]
    - r1[0]*r2[1]*r3[2]*r0[3]
    - r1[0]*r3[1]*r0[2]*r2[3]
    + r1[0]*r3[1]*r2[2]*r0[3]
    + r2[0]*r0[1]*r1[2]*r3[3]
    - r2[0]*r0[1]*r3[2]*r1[3]
    - r2[0]*r1[1]*r0[2]*r3[3]
    + r2[0]*r1[1]*r3[2]*r0[3]
    + r2[0]*r3[1]*r0[2]*r1[3]
    - r2[0]*r3[1]*r1[2]*r0[3]
    - r3[0]*r0[1]*r1[2]*r2[3]
    + r3[0]*r0[1]*r2[2]*r1[3]
    + r3[0]*r1[1]*r0[2]*r2[3]
    - r3[0]*r1[1]*r2[2]*r0[3]
    - r3[0]*r2[1]*r0[2]*r1[3]
    + r3[0]*r2[1]*r1[2]*r0[3]
}

/// `vnl_determinant` of an `n × n` matrix (row-major), `n` from 1 to 4.
pub fn determinant(m: &[f64], n: usize) -> f64 {
    assert_eq!(m.len(), n * n, "not an n × n matrix");
    match n {
        1 => m[0],
        2 => det2(&[[m[0], m[1]], [m[2], m[3]]]),
        3 => det3(&[[m[0], m[1], m[2]], [m[3], m[4], m[5]], [m[6], m[7], m[8]]]),
        4 => det4(&std::array::from_fn(|i| {
            std::array::from_fn(|j| m[i * 4 + j])
        })),
        _ => panic!("vnl_determinant by QR is not ported (n = {n})"),
    }
}

/// `vnl_inverse(vnl_matrix_fixed<T, 2, 2>)`: the zero matrix (vnl's assertion is compiled
/// out) for a zero determinant.
pub fn inverse2(m: &[[f64; 2]; 2]) -> [[f64; 2]; 2] {
    let mut det = det2(m);
    if det == 0.0 {
        return [[0.0; 2]; 2];
    }
    det = 1.0 / det;
    [
        [m[1][1] * det, -m[0][1] * det],
        [-m[1][0] * det, m[0][0] * det],
    ]
}

/// `vnl_inverse(vnl_matrix_fixed<T, 3, 3>)`.
pub fn inverse3(m: &[[f64; 3]; 3]) -> [[f64; 3]; 3] {
    let mut det = det3(m);
    if det == 0.0 {
        return [[0.0; 3]; 3];
    }
    det = 1.0 / det;
    [
        [
            (m[1][1] * m[2][2] - m[1][2] * m[2][1]) * det,
            (m[2][1] * m[0][2] - m[2][2] * m[0][1]) * det,
            (m[0][1] * m[1][2] - m[0][2] * m[1][1]) * det,
        ],
        [
            (m[1][2] * m[2][0] - m[1][0] * m[2][2]) * det,
            (m[0][0] * m[2][2] - m[0][2] * m[2][0]) * det,
            (m[1][0] * m[0][2] - m[1][2] * m[0][0]) * det,
        ],
        [
            (m[1][0] * m[2][1] - m[1][1] * m[2][0]) * det,
            (m[0][1] * m[2][0] - m[0][0] * m[2][1]) * det,
            (m[0][0] * m[1][1] - m[0][1] * m[1][0]) * det,
        ],
    ]
}

/// `vnl_inverse(vnl_matrix_fixed<T, 4, 4>)`: the adjugate's entries, then the matrix times
/// `1 / det`.
#[rustfmt::skip]
pub fn inverse4(m: &[[f64; 4]; 4]) -> [[f64; 4]; 4] {
    let mut det = det4(m);
    if det == 0.0 {
        return [[0.0; 4]; 4];
    }
    det = 1.0 / det;
    let m = |i: usize, j: usize| m[i][j];
    let mut d = [0.0f64; 16];
    d[0] =  m(1,1)*m(2,2)*m(3,3) - m(1,1)*m(2,3)*m(3,2) - m(2,1)*m(1,2)*m(3,3)
          + m(2,1)*m(1,3)*m(3,2) + m(3,1)*m(1,2)*m(2,3) - m(3,1)*m(1,3)*m(2,2);
    d[1] = -m(0,1)*m(2,2)*m(3,3) + m(0,1)*m(2,3)*m(3,2) + m(2,1)*m(0,2)*m(3,3)
          - m(2,1)*m(0,3)*m(3,2) - m(3,1)*m(0,2)*m(2,3) + m(3,1)*m(0,3)*m(2,2);
    d[2] =  m(0,1)*m(1,2)*m(3,3) - m(0,1)*m(1,3)*m(3,2) - m(1,1)*m(0,2)*m(3,3)
          + m(1,1)*m(0,3)*m(3,2) + m(3,1)*m(0,2)*m(1,3) - m(3,1)*m(0,3)*m(1,2);
    d[3] = -m(0,1)*m(1,2)*m(2,3) + m(0,1)*m(1,3)*m(2,2) + m(1,1)*m(0,2)*m(2,3)
          - m(1,1)*m(0,3)*m(2,2) - m(2,1)*m(0,2)*m(1,3) + m(2,1)*m(0,3)*m(1,2);
    d[4] = -m(1,0)*m(2,2)*m(3,3) + m(1,0)*m(2,3)*m(3,2) + m(2,0)*m(1,2)*m(3,3)
          - m(2,0)*m(1,3)*m(3,2) - m(3,0)*m(1,2)*m(2,3) + m(3,0)*m(1,3)*m(2,2);
    d[5] =  m(0,0)*m(2,2)*m(3,3) - m(0,0)*m(2,3)*m(3,2) - m(2,0)*m(0,2)*m(3,3)
          + m(2,0)*m(0,3)*m(3,2) + m(3,0)*m(0,2)*m(2,3) - m(3,0)*m(0,3)*m(2,2);
    d[6] = -m(0,0)*m(1,2)*m(3,3) + m(0,0)*m(1,3)*m(3,2) + m(1,0)*m(0,2)*m(3,3)
          - m(1,0)*m(0,3)*m(3,2) - m(3,0)*m(0,2)*m(1,3) + m(3,0)*m(0,3)*m(1,2);
    d[7] =  m(0,0)*m(1,2)*m(2,3) - m(0,0)*m(1,3)*m(2,2) - m(1,0)*m(0,2)*m(2,3)
          + m(1,0)*m(0,3)*m(2,2) + m(2,0)*m(0,2)*m(1,3) - m(2,0)*m(0,3)*m(1,2);
    d[8] =  m(1,0)*m(2,1)*m(3,3) - m(1,0)*m(2,3)*m(3,1) - m(2,0)*m(1,1)*m(3,3)
          + m(2,0)*m(1,3)*m(3,1) + m(3,0)*m(1,1)*m(2,3) - m(3,0)*m(1,3)*m(2,1);
    d[9] = -m(0,0)*m(2,1)*m(3,3) + m(0,0)*m(2,3)*m(3,1) + m(2,0)*m(0,1)*m(3,3)
          - m(2,0)*m(0,3)*m(3,1) - m(3,0)*m(0,1)*m(2,3) + m(3,0)*m(0,3)*m(2,1);
    d[10] = m(0,0)*m(1,1)*m(3,3) - m(0,0)*m(1,3)*m(3,1) - m(1,0)*m(0,1)*m(3,3)
          + m(1,0)*m(0,3)*m(3,1) + m(3,0)*m(0,1)*m(1,3) - m(3,0)*m(0,3)*m(1,1);
    d[11] = -m(0,0)*m(1,1)*m(2,3) + m(0,0)*m(1,3)*m(2,1) + m(1,0)*m(0,1)*m(2,3)
          - m(1,0)*m(0,3)*m(2,1) - m(2,0)*m(0,1)*m(1,3) + m(2,0)*m(0,3)*m(1,1);
    d[12] = -m(1,0)*m(2,1)*m(3,2) + m(1,0)*m(2,2)*m(3,1) + m(2,0)*m(1,1)*m(3,2)
          - m(2,0)*m(1,2)*m(3,1) - m(3,0)*m(1,1)*m(2,2) + m(3,0)*m(1,2)*m(2,1);
    d[13] = m(0,0)*m(2,1)*m(3,2) - m(0,0)*m(2,2)*m(3,1) - m(2,0)*m(0,1)*m(3,2)
          + m(2,0)*m(0,2)*m(3,1) + m(3,0)*m(0,1)*m(2,2) - m(3,0)*m(0,2)*m(2,1);
    d[14] = -m(0,0)*m(1,1)*m(3,2) + m(0,0)*m(1,2)*m(3,1) + m(1,0)*m(0,1)*m(3,2)
          - m(1,0)*m(0,2)*m(3,1) - m(3,0)*m(0,1)*m(1,2) + m(3,0)*m(0,2)*m(1,1);
    d[15] = m(0,0)*m(1,1)*m(2,2) - m(0,0)*m(1,2)*m(2,1) - m(1,0)*m(0,1)*m(2,2)
          + m(1,0)*m(0,2)*m(2,1) + m(2,0)*m(0,1)*m(1,2) - m(2,0)*m(0,2)*m(1,1);
    std::array::from_fn(|i| std::array::from_fn(|j| d[i * 4 + j] * det))
}

/// `vnl_inverse` of an `n × n` `vnl_matrix` (row-major), `n` from 1 to 4.
pub fn inverse(m: &[f64], n: usize) -> Vec<f64> {
    assert_eq!(m.len(), n * n, "not an n × n matrix");
    match n {
        1 => vec![1.0 / m[0]],
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
