// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! The two element types of the robust registration's templates (`float` and `double`: the
//! registration step runs in `float` unless `--doubleprec`) and VNL's arithmetic on them as ITK
//! 4.13.2's library computes it (queue 3.2-03):
//!
//! - [`Real`]: `float` and `double` with x86-64's SSE NaNs (`ItkReal::sse_*`), C's conversions
//!   between them (`cvtss2sd`, `cvtsd2ss`: a NaN quieted, its sign and the top of its payload kept)
//!   and `fabs`;
//! - [`Mat`]: a `vnl_matrix<T>` (row-major), with `vnl_tag_mul`'s product, `matrix_x_vector`
//!   (VNL's portable loop: ITK 4.13.2's VNL is built without its SSE2 kernels,
//!   `VNL_CONFIG_ENABLE_SSE2 0`), the transpose and `is_finite`.

use larmorx_core::itk_real::ItkReal;

/// `float` or `double` as the robust registration's templates compute with them.
pub trait Real: ItkReal + PartialOrd + Default + 'static {
    /// Whether this is `float`.
    const SINGLE: bool;
    /// `(double) x` (`cvtss2sd` for a `float`: a signalling NaN comes out quiet).
    fn wide(self) -> f64;
    /// `(T) x` of a `double` (`cvtsd2ss` for a `float`: a NaN keeps its sign and the top of its
    /// payload, quieted).
    fn narrow(x: f64) -> Self;
    /// `fabs` in the type (the sign bit cleared; a NaN's payload kept).
    fn fabs(self) -> Self;
    /// Whether the number is finite (`vnl_math::isfinite`).
    fn finite(self) -> bool;
    /// Whether the number is a NaN.
    fn nan(self) -> bool;
    /// `-x` (the sign bit flipped).
    fn negate(self) -> Self;
    /// The number's bits as the probes print them (`%08x` of a `float`, `%016x` of a `double`).
    fn hex(self) -> String;
    /// `(float) x`: nothing for a `float` (its bits kept), `cvtsd2ss` for a `double`.
    fn f32_of(self) -> f32;
    /// `(T) x` of a `float`: nothing for a `float`, `cvtss2sd` for a `double`.
    fn of_f32(x: f32) -> Self;
}

impl Real for f32 {
    const SINGLE: bool = true;
    #[inline(always)]
    fn wide(self) -> f64 {
        let d = f64::from(self);
        if d.is_nan() {
            f64::from_bits(d.to_bits() | 0x0008_0000_0000_0000)
        } else {
            d
        }
    }
    #[inline(always)]
    fn narrow(x: f64) -> f32 {
        if x.is_nan() {
            let b = x.to_bits();
            f32::from_bits(
                ((b >> 32) as u32 & 0x8000_0000) | 0x7fc0_0000 | ((b >> 29) as u32 & 0x003f_ffff),
            )
        } else {
            x as f32
        }
    }
    #[inline(always)]
    fn fabs(self) -> f32 {
        f32::from_bits(self.to_bits() & 0x7fff_ffff)
    }
    #[inline(always)]
    fn finite(self) -> bool {
        self.is_finite()
    }
    #[inline(always)]
    fn nan(self) -> bool {
        self.is_nan()
    }
    #[inline(always)]
    fn negate(self) -> f32 {
        f32::from_bits(self.to_bits() ^ 0x8000_0000)
    }
    fn hex(self) -> String {
        format!("{:08x}", self.to_bits())
    }
    #[inline(always)]
    fn f32_of(self) -> f32 {
        self
    }
    #[inline(always)]
    fn of_f32(x: f32) -> f32 {
        x
    }
}

impl Real for f64 {
    const SINGLE: bool = false;
    #[inline(always)]
    fn wide(self) -> f64 {
        self
    }
    #[inline(always)]
    fn narrow(x: f64) -> f64 {
        x
    }
    #[inline(always)]
    fn fabs(self) -> f64 {
        f64::from_bits(self.to_bits() & 0x7fff_ffff_ffff_ffff)
    }
    #[inline(always)]
    fn finite(self) -> bool {
        self.is_finite()
    }
    #[inline(always)]
    fn nan(self) -> bool {
        self.is_nan()
    }
    #[inline(always)]
    fn negate(self) -> f64 {
        f64::from_bits(self.to_bits() ^ 0x8000_0000_0000_0000)
    }
    fn hex(self) -> String {
        format!("{:016x}", self.to_bits())
    }
    #[inline(always)]
    fn f32_of(self) -> f32 {
        f32::narrow(self)
    }
    #[inline(always)]
    fn of_f32(x: f32) -> f64 {
        x.wide()
    }
}

/// A `vnl_matrix<T>`: `rows x cols`, row-major (`data[i * cols + j]`).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Mat<T> {
    pub rows: usize,
    pub cols: usize,
    pub data: Vec<T>,
}

impl<T: Real> Mat<T> {
    /// A `rows x cols` matrix of zeros.
    pub fn zeros(rows: usize, cols: usize) -> Mat<T> {
        Mat {
            rows,
            cols,
            data: vec![T::ZERO; rows * cols],
        }
    }

    /// Element `(i, j)`.
    #[inline(always)]
    pub fn at(&self, i: usize, j: usize) -> T {
        self.data[i * self.cols + j]
    }

    /// Element `(i, j)`, to write.
    #[inline(always)]
    pub fn at_mut(&mut self, i: usize, j: usize) -> &mut T {
        &mut self.data[i * self.cols + j]
    }

    /// `self * b` (`vnl_tag_mul`): `sum += a(i, j) * b(j, k)` from `+0` in `j` order.
    pub fn mul(&self, b: &Mat<T>) -> Mat<T> {
        assert_eq!(self.cols, b.rows, "vnl_tag_mul: sizes");
        let (l, m, n) = (self.rows, self.cols, b.cols);
        let mut r = Mat::zeros(l, n);
        for i in 0..l {
            for k in 0..n {
                let mut sum = T::ZERO;
                for j in 0..m {
                    sum = sum.sse_add(self.data[i * m + j].sse_mul(b.data[j * n + k]));
                }
                r.data[i * n + k] = sum;
            }
        }
        r
    }

    /// `self * v` (`vnl_sse<T>::matrix_x_vector`, the portable loop): each row's `som += m(i, j)
    /// * v(j)` from `+0` in `j` order, with the official library's operand orders (GCC 8's
    /// vectorised products, the sums kept in order; they decide which of two NaNs comes out). In
    /// `float`, for rows of four or more numbers, blocks of four products `v * m` summed as
    /// `p0 + s`, then `p1 + s`, `s + p2` and `s + p3`. In `double`, for rows of two or more, pairs
    /// `v * m` summed `s + p`. The rest (and every number of shorter rows): `s + m * v`.
    pub fn mul_vec(&self, v: &[T]) -> Vec<T> {
        assert_eq!(self.cols, v.len(), "matrix_x_vector: sizes");
        let c = self.cols;
        let (lanes, min) = if T::SINGLE { (4, 4) } else { (2, 2) };
        let blocked = if c >= min { c / lanes * lanes } else { 0 };
        (0..self.rows)
            .map(|i| {
                let row = &self.data[i * c..(i + 1) * c];
                let mut s = T::ZERO;
                let mut j = 0;
                while j < blocked {
                    if T::SINGLE {
                        let p: [T; 4] = std::array::from_fn(|k| v[j + k].sse_mul(row[j + k]));
                        s = p[0].sse_add(s);
                        s = p[1].sse_add(s);
                        s = s.sse_add(p[2]);
                        s = s.sse_add(p[3]);
                    } else {
                        s = s.sse_add(v[j].sse_mul(row[j]));
                        s = s.sse_add(v[j + 1].sse_mul(row[j + 1]));
                    }
                    j += lanes;
                }
                for k in j..c {
                    s = s.sse_add(row[k].sse_mul(v[k]));
                }
                s
            })
            .collect()
    }

    /// `transpose()`.
    pub fn transpose(&self) -> Mat<T> {
        let mut r = Mat::zeros(self.cols, self.rows);
        for i in 0..self.rows {
            for j in 0..self.cols {
                r.data[j * self.rows + i] = self.data[i * self.cols + j];
            }
        }
        r
    }

    /// `is_finite()`: no infinite or NaN element.
    pub fn is_finite(&self) -> bool {
        self.data.iter().all(|x| x.finite())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `matrix_x_vector` takes the operand orders of the official library: of two NaNs in a
    /// float row of four, the product's first operand is the vector's; in a row of three, the
    /// matrix's.
    #[test]
    fn matrix_x_vector_nan_orders() {
        let (a, b) = (f32::from_bits(0x7fc0_0001), f32::from_bits(0x7fc0_0002));
        let four = Mat {
            rows: 1,
            cols: 4,
            data: vec![a, 0.0, 0.0, 0.0],
        };
        assert_eq!(four.mul_vec(&[b, 1.0, 1.0, 1.0])[0].to_bits(), 0x7fc0_0002);
        let three = Mat {
            rows: 1,
            cols: 3,
            data: vec![a, 0.0, 0.0],
        };
        assert_eq!(three.mul_vec(&[b, 1.0, 1.0])[0].to_bits(), 0x7fc0_0001);
        let d = Mat::<f64> {
            rows: 2,
            cols: 2,
            data: vec![1.0, 2.0, 3.0, 4.0],
        };
        assert_eq!(d.mul_vec(&[1.0, 1.0]), vec![3.0, 7.0]);
        assert_eq!(
            f32::narrow(f64::from_bits(0xfff4_0000_0000_0001)).to_bits(),
            0xffe0_0000
        );
    }
}
