// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! `vnl_matrix<double>`'s arithmetic as ITK 4.13.2's VNL computes it (VXL, BSD-3-Clause; the
//! files `core/vnl/vnl_matrix.hxx`, `vnl_c_vector.hxx`, `vnl_diag_matrix.h`, `vnl_complexify.h`,
//! `vnl_matlab_print.hxx`, `vnl_matlab_print_scalar.cxx`), for the FreeSurfer
//! replica's robust registration (`fs.robust`) and any port that multiplies VNL matrices:
//!
//! - [`VnlMatrix::mul`] (`vnl_tag_mul`): each element `sum += a(i, j) * b(j, k)` from `+0`, `j`
//!   in order;
//! - [`VnlMatrix::fro_norm`] (`frobenius_norm`, `vnl_c_vector::two_norm`): the squares summed from
//!   `+0` in storage order, then `sqrt`;
//! - [`VnlMatrix::operator_one_norm`]: the largest column sum of absolute values (a column sum
//!   kept only when greater: a NaN sum is never kept);
//! - [`VnlMatrix::scale_columns`] (`vnl_matrix * vnl_diag_matrix`: `a(i, j) * d(j)`);
//! - [`VnlMatrix::transpose`], subtraction, identity;
//! - [`VnlCMatrix`]: `vnl_matrix<std::complex<double>>`'s product, conjugate transpose, real and
//!   imaginary parts and one-norm, and `vnl_matlab_print`'s long format of both kinds.
//!
//! Every operation that can meet a NaN follows x86-64's SSE rules (`ItkReal::sse_*`: the first
//! operand's NaN, SSE's default NaN from numbers), so the bits are the official x86-64 build's on
//! every platform.

use crate::cfmt;
use crate::cplx::{C64, cabs, cxx_mul};
use crate::itk_real::ItkReal;

/// `sqrt` (`sqrtsd`) with SSE's NaNs: a NaN quieted, SSE's default NaN for a negative number.
#[inline(always)]
pub fn sse_sqrt(x: f64) -> f64 {
    let r = x.sqrt();
    if r.is_nan() {
        if x.is_nan() {
            f64::from_bits(x.to_bits() | 0x0008_0000_0000_0000)
        } else {
            f64::from_bits(0xfff8_0000_0000_0000)
        }
    } else {
        r
    }
}

/// A `vnl_matrix<double>`: `rows x cols`, row-major (`data[i * cols + j]`, vnl's storage).
#[derive(Clone, Debug, PartialEq)]
pub struct VnlMatrix {
    pub rows: usize,
    pub cols: usize,
    pub data: Vec<f64>,
}

impl VnlMatrix {
    /// A `rows x cols` matrix of zeros (`vnl_matrix<double>(r, c, 0.0)`).
    pub fn zeros(rows: usize, cols: usize) -> VnlMatrix {
        VnlMatrix {
            rows,
            cols,
            data: vec![0.0; rows * cols],
        }
    }

    /// The `n x n` identity (`set_identity`).
    pub fn identity(n: usize) -> VnlMatrix {
        let mut m = VnlMatrix::zeros(n, n);
        for i in 0..n {
            m.data[i * n + i] = 1.0;
        }
        m
    }

    /// From rows of four (`vnl_matrix_fixed<double, 4, 4>::as_matrix`).
    pub fn from_4x4(m: &[[f64; 4]; 4]) -> VnlMatrix {
        VnlMatrix {
            rows: 4,
            cols: 4,
            data: m.iter().flatten().copied().collect(),
        }
    }

    /// The first 16 elements as rows of four (`vnl_matrix_fixed<double, 4, 4>`'s copy of a
    /// `vnl_matrix`, which copies 16 elements whatever the size; `None` when it has fewer).
    pub fn to_4x4(&self) -> Option<[[f64; 4]; 4]> {
        (self.data.len() >= 16)
            .then(|| std::array::from_fn(|i| std::array::from_fn(|j| self.data[i * 4 + j])))
    }

    /// Element `(i, j)`.
    #[inline(always)]
    pub fn at(&self, i: usize, j: usize) -> f64 {
        self.data[i * self.cols + j]
    }

    /// Element `(i, j)`, mutable.
    #[inline(always)]
    pub fn at_mut(&mut self, i: usize, j: usize) -> &mut f64 {
        &mut self.data[i * self.cols + j]
    }

    /// Whether it has no elements (`empty()`).
    pub fn is_empty(&self) -> bool {
        self.rows == 0 || self.cols == 0
    }

    /// `self * b` (`vnl_tag_mul`): `sum += a(i, j) * b(j, k)` from `+0` in `j` order, `sum`
    /// the destination of each addition. Requires `self.cols == b.rows`.
    pub fn mul(&self, b: &VnlMatrix) -> VnlMatrix {
        assert_eq!(self.cols, b.rows, "vnl_tag_mul: sizes");
        let (l, m, n) = (self.rows, self.cols, b.cols);
        let mut r = VnlMatrix::zeros(l, n);
        for i in 0..l {
            for k in 0..n {
                let mut sum = 0.0f64;
                for j in 0..m {
                    sum = sum.sse_add(self.data[i * m + j].sse_mul(b.data[j * n + k]));
                }
                r.data[i * n + k] = sum;
            }
        }
        r
    }

    /// `self - b`, element by element (`operator-`).
    pub fn sub(&self, b: &VnlMatrix) -> VnlMatrix {
        assert_eq!((self.rows, self.cols), (b.rows, b.cols), "operator-: sizes");
        VnlMatrix {
            rows: self.rows,
            cols: self.cols,
            data: self
                .data
                .iter()
                .zip(&b.data)
                .map(|(&x, &y)| x.sse_sub(y))
                .collect(),
        }
    }

    /// `transpose()`.
    pub fn transpose(&self) -> VnlMatrix {
        let mut r = VnlMatrix::zeros(self.cols, self.rows);
        for i in 0..self.rows {
            for j in 0..self.cols {
                r.data[j * self.rows + i] = self.data[i * self.cols + j];
            }
        }
        r
    }

    /// `self * D` for a diagonal `D` (`vnl_matrix * vnl_diag_matrix`): column `j` times `d[j]`.
    pub fn scale_columns(&self, d: &[f64]) -> VnlMatrix {
        assert_eq!(self.cols, d.len(), "vnl_matrix * vnl_diag_matrix: sizes");
        let mut r = self.clone();
        for i in 0..self.rows {
            for (j, &dj) in d.iter().enumerate() {
                r.data[i * self.cols + j] = self.data[i * self.cols + j].sse_mul(dj);
            }
        }
        r
    }

    /// `frobenius_norm()`: `sqrt` of the squares summed in storage order from `+0`.
    pub fn fro_norm(&self) -> f64 {
        let mut val = 0.0f64;
        for &x in &self.data {
            val = val.sse_add(x.sse_mul(x));
        }
        sse_sqrt(val)
    }

    /// `operator_one_norm()`: for each column the sum of `|a(i, j)|` from `+0`; the largest kept
    /// (`tmp > max`, from `max = 0`).
    pub fn operator_one_norm(&self) -> f64 {
        let mut max = 0.0f64;
        for j in 0..self.cols {
            let mut tmp = 0.0f64;
            for i in 0..self.rows {
                tmp = tmp.sse_add(self.data[i * self.cols + j].abs());
            }
            if tmp > max {
                max = tmp;
            }
        }
        max
    }
}

/// A `vnl_matrix<std::complex<double>>`: `rows x cols`, row-major. Its arithmetic is C++'s
/// complex arithmetic as GCC compiles it (`crate::cplx`: the plain product with `__muldc3` where
/// both parts are NaN; `abs` is `cabs`, the decided correctly rounded `hypot`).
#[derive(Clone, Debug, PartialEq)]
pub struct VnlCMatrix {
    pub rows: usize,
    pub cols: usize,
    pub data: Vec<C64>,
}

impl VnlCMatrix {
    /// A `rows x cols` matrix of zeros (`vnl_matrix<std::complex<double>>(r, c, 0.0)`).
    pub fn zeros(rows: usize, cols: usize) -> VnlCMatrix {
        VnlCMatrix {
            rows,
            cols,
            data: vec![C64::ZERO; rows * cols],
        }
    }

    /// `vnl_complexify(m)`: each element `(x, +0)`.
    pub fn complexify(m: &VnlMatrix) -> VnlCMatrix {
        VnlCMatrix {
            rows: m.rows,
            cols: m.cols,
            data: m.data.iter().map(|&x| C64::real(x)).collect(),
        }
    }

    /// Element `(i, j)`.
    #[inline(always)]
    pub fn at(&self, i: usize, j: usize) -> C64 {
        self.data[i * self.cols + j]
    }

    /// Element `(i, j)`, mutable.
    #[inline(always)]
    pub fn at_mut(&mut self, i: usize, j: usize) -> &mut C64 {
        &mut self.data[i * self.cols + j]
    }

    /// `vnl_real(m)`.
    pub fn real(&self) -> VnlMatrix {
        VnlMatrix {
            rows: self.rows,
            cols: self.cols,
            data: self.data.iter().map(|z| z.re).collect(),
        }
    }

    /// `vnl_imag(m)`.
    pub fn imag(&self) -> VnlMatrix {
        VnlMatrix {
            rows: self.rows,
            cols: self.cols,
            data: self.data.iter().map(|z| z.im).collect(),
        }
    }

    /// `transpose()`.
    pub fn transpose(&self) -> VnlCMatrix {
        let mut r = VnlCMatrix::zeros(self.cols, self.rows);
        for i in 0..self.rows {
            for j in 0..self.cols {
                r.data[j * self.rows + i] = self.data[i * self.cols + j];
            }
        }
        r
    }

    /// `conjugate_transpose()`: the transpose, each element conjugated.
    pub fn conjugate_transpose(&self) -> VnlCMatrix {
        let mut r = self.transpose();
        for z in &mut r.data {
            *z = z.conj();
        }
        r
    }

    /// `self * b` (`vnl_tag_mul`): `sum += a(i, j) * b(j, k)` from `(0, 0)` in `j` order.
    pub fn mul(&self, b: &VnlCMatrix) -> VnlCMatrix {
        assert_eq!(self.cols, b.rows, "vnl_tag_mul: sizes");
        let (l, m, n) = (self.rows, self.cols, b.cols);
        let mut r = VnlCMatrix::zeros(l, n);
        for i in 0..l {
            for k in 0..n {
                let mut sum = C64::ZERO;
                for j in 0..m {
                    sum = sum.add(cxx_mul(self.data[i * m + j], b.data[j * n + k]));
                }
                r.data[i * n + k] = sum;
            }
        }
        r
    }

    /// `self * D` for a diagonal `D` (`vnl_matrix * vnl_diag_matrix`): `a(i, j) * d(j)`.
    pub fn scale_columns(&self, d: &[C64]) -> VnlCMatrix {
        assert_eq!(self.cols, d.len(), "vnl_matrix * vnl_diag_matrix: sizes");
        let mut r = self.clone();
        for i in 0..self.rows {
            for (j, &dj) in d.iter().enumerate() {
                r.data[i * self.cols + j] = cxx_mul(self.data[i * self.cols + j], dj);
            }
        }
        r
    }

    /// `operator_one_norm()`: for each column the sum of `abs(a(i, j))` (`cabs`) from `+0`; the
    /// largest kept (`tmp > max`, from `max = 0`).
    pub fn operator_one_norm(&self) -> f64 {
        let mut max = 0.0f64;
        for j in 0..self.cols {
            let mut tmp = 0.0f64;
            for i in 0..self.rows {
                tmp = tmp.sse_add(cabs(self.data[i * self.cols + j]));
            }
            if tmp > max {
                max = tmp;
            }
        }
        max
    }
}

/// `vnl_matlab_print_scalar(std::complex<double>, buf, vnl_matlab_print_format_long)`: the real
/// part as `"%16d "` of 0 when it equals zero, else `"%16.12f "`; the imaginary part as `" %15s  "`
/// of nothing when it equals zero, else its sign (`-` below zero, otherwise `+`, a NaN's sign
/// printed by `%f`) and `"%15.12fi "` of its magnitude.
pub fn matlab_long_complex(z: C64) -> String {
    let mut s = if z.re == 0.0 {
        format!("{:>16} ", 0)
    } else {
        format!("{:>16} ", cfmt::f(z.re, 12))
    };
    if z.im == 0.0 {
        s.push_str(&format!(" {:>15}  ", ""));
    } else {
        let (sign, i) = if z.im < 0.0 {
            ('-', -z.im)
        } else {
            ('+', z.im)
        };
        s.push_str(&format!("{sign}{:>15}i ", cfmt::f(i, 12)));
    }
    s
}

/// `vnl_matlab_print(os, M, name, vnl_matlab_print_format_long)` of a `vnl_matrix<double>`:
/// `<name> = [ ...`, each row's elements (`vnl_svd::matlab_long`), ` ]` after the last row, each row
/// on its line; `<name> = [ ...` and `];` for a matrix without rows.
pub fn matlab_print_long(m: &VnlMatrix, name: &str) -> String {
    let mut s = format!("{name} = [ ...\n");
    if m.rows == 0 {
        s.push_str("];\n");
        return s;
    }
    for i in 0..m.rows {
        for j in 0..m.cols {
            s.push_str(&crate::vnl_svd::matlab_long(m.at(i, j)));
        }
        if i + 1 == m.rows {
            s.push_str(" ]");
        }
        s.push('\n');
    }
    s
}

/// `vnl_matlab_print(os, D, name, vnl_matlab_print_format_long)` of a `vnl_diag_matrix<double>`
/// (`vnl_matlab_print.hxx`): `<name> = diag([ `, each diagonal element (`vnl_svd::matlab_long`),
/// ` ])` and a newline.
pub fn matlab_print_long_diag(d: &[f64], name: &str) -> String {
    let mut s = format!("{name} = diag([ ");
    for &v in d {
        s.push_str(&crate::vnl_svd::matlab_long(v));
    }
    s.push_str(" ])\n");
    s
}

/// `vnl_matlab_print(os, v, name, vnl_matlab_print_format_long)` of a `vnl_vector<double>`
/// (`vnl_matlab_print.hxx`): `<name> = [ `, each element (`vnl_svd::matlab_long`), ` ]` and a
/// newline.
pub fn matlab_print_long_vector(v: &[f64], name: &str) -> String {
    let mut s = format!("{name} = [ ");
    for &x in v {
        s.push_str(&crate::vnl_svd::matlab_long(x));
    }
    s.push_str(" ]\n");
    s
}

/// [`matlab_print_long`] of a `vnl_matrix<std::complex<double>>` ([`matlab_long_complex`]).
pub fn matlab_print_long_complex(m: &VnlCMatrix, name: &str) -> String {
    let mut s = format!("{name} = [ ...\n");
    if m.rows == 0 {
        s.push_str("];\n");
        return s;
    }
    for i in 0..m.rows {
        for j in 0..m.cols {
            s.push_str(&matlab_long_complex(m.at(i, j)));
        }
        if i + 1 == m.rows {
            s.push_str(" ]");
        }
        s.push('\n');
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn products_sum_from_zero_in_order() {
        let a = VnlMatrix {
            rows: 2,
            cols: 3,
            data: vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0],
        };
        let b = a.transpose();
        let c = a.mul(&b);
        assert_eq!(c.data, vec![14.0, 32.0, 32.0, 77.0]);
        assert_eq!(a.fro_norm(), 91f64.sqrt());
        assert_eq!(a.operator_one_norm(), 9.0);
        assert_eq!(
            a.scale_columns(&[1.0, 0.5, 2.0]).data,
            vec![1.0, 1.0, 6.0, 4.0, 2.5, 12.0]
        );
    }

    #[test]
    fn a_nan_sum_takes_the_first_operands_nan() {
        // sum (+0) + NaN * x: the product's NaN, quieted; a NaN in the sum is kept first
        let n1 = f64::from_bits(0x7ff4_0000_0000_0001);
        let n2 = f64::from_bits(0xfff0_0000_0000_0002);
        let a = VnlMatrix {
            rows: 1,
            cols: 2,
            data: vec![n1, n2],
        };
        let b = VnlMatrix {
            rows: 2,
            cols: 1,
            data: vec![1.0, 1.0],
        };
        assert_eq!(a.mul(&b).data[0].to_bits(), 0x7ffc_0000_0000_0001);
    }
}
