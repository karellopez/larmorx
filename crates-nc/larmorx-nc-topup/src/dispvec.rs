// SPDX-License-Identifier: LicenseRef-FSL
// Translated to Rust from FSL 6.0.7.23 (fsl-topup 2203.5: displacement_vector.{h,cpp}).
// Copyright (C) 2012 University of Oxford (Jesper Andersson). Rust translation Copyright 2026
// Karel Lopez Vilaret; modified from the originals. Distributed under the FSL Licence (see
// LICENSE).
//! applytopup's displacement vectors (`CircularArray`, `DispVec`): the off-resonance field along
//! one phase-encoding line, a circular array of displacements, and the matrix `K` that maps an
//! undistorted line to the distorted one (each distorted voxel the integral of the undistorted
//! line over the voxel's displaced extent), with the Laplacian `S` least-squares restoration
//! regularises with.

use larmorx_nc_core::arma::Mat;
use larmorx_nc_core::volume::{FslError, Volume};

/// `DispVecException(msg)`: `what()` is `"DispVec::" + msg`.
pub fn dispvec_error(msg: &str) -> FslError {
    FslError {
        printed: Vec::new(),
        type_name: "TOPUP::DispVecException",
        what: format!("DispVec::{msg}"),
    }
}

/// `CircularArray`: `n` values `v`, read as the positions `i + sf * v[i mod n]` of the voxels
/// `i` of a line repeated without end.
#[derive(Clone, Debug)]
pub struct CircularArray {
    v: Vec<f64>,
    sf: f64,
    /// FSL-133: `operator[]` wraps a negative index with an unsigned remainder (FSL's), not the
    /// circular one.
    unsigned_wrap: bool,
    /// Whether a negative index was read where the two wraps differ.
    pub wrapped_differently: bool,
}

impl CircularArray {
    /// `CircularArray(n)`: `n` zeros, scale factor 1.
    pub fn new(n: usize, unsigned_wrap: bool) -> CircularArray {
        CircularArray {
            v: vec![0.0; n],
            sf: 1.0,
            unsigned_wrap,
            wrapped_differently: false,
        }
    }

    /// `N()`.
    pub fn n(&self) -> usize {
        self.v.len()
    }

    /// The element `i` reads: `i % _n` with `_n` unsigned converts `i` to unsigned first, so a
    /// negative `i` wraps by `2^32 mod n` (FSL's, FSL-133) unless `n` divides `2^32`; the
    /// circular index otherwise.
    fn slot(&mut self, i: i32) -> usize {
        let n = self.v.len() as u32;
        let fsl = ((i as u32) % n) as usize;
        if i >= 0 {
            return fsl;
        }
        let circular = i.rem_euclid(n as i32) as usize;
        if circular != fsl {
            self.wrapped_differently = true;
        }
        if self.unsigned_wrap { fsl } else { circular }
    }

    /// `operator[](i)`: `i + sf * v[i mod n]`.
    pub fn at(&mut self, i: i32) -> f64 {
        let j = self.slot(i);
        f64::from(i) + self.sf * self.v[j]
    }

    /// `IndexInRange(i)`: the circular index of `i` (a signed remainder).
    pub fn index_in_range(&self, i: i32) -> usize {
        let n = self.v.len() as i32;
        let j = i % n;
        if j < 0 { (n + j) as usize } else { j as usize }
    }

    /// `Find(x)`: the last position at or below `x` (its index).
    pub fn find(&mut self, x: f64) -> i32 {
        let mut i = x.ceil() as i32;
        if x > self.at(i) {
            i += 1;
            while x > self.at(i) {
                i += 1;
            }
            i - 1
        } else if x == self.at(i) {
            i
        } else {
            while x < self.at(i - 1) {
                i -= 1;
            }
            i - 1
        }
    }

    /// `set_from_row_or_col(ima, k, ij, row)`: the values of row `ij` (along x) or column `ij`
    /// (along y) of slice `k`, as doubles; the array takes the line's length.
    pub fn set_from_line(&mut self, ima: &Volume, k: i64, ij: i64, row: bool) {
        let (xs, ys) = (ima.xsize(), ima.ysize());
        let offs = if row {
            k * xs * ys + ij * xs
        } else {
            k * xs * ys + ij
        };
        let ss = if row { 1 } else { xs };
        let n = if row { xs } else { ys } as usize;
        if n != self.v.len() {
            self.v = vec![0.0; n];
        }
        for (i, x) in self.v.iter_mut().enumerate() {
            *x = f64::from(ima.data[(offs + i as i64 * ss) as usize]);
        }
    }

    /// `SetScaleFactor(sf)`.
    pub fn set_scale_factor(&mut self, sf: f64) {
        self.sf = sf;
    }

    /// `GetScaleFactor()`.
    pub fn scale_factor(&self) -> f64 {
        self.sf
    }
}

/// `DispVec`: a [`CircularArray`] of one line's field.
#[derive(Clone, Debug)]
pub struct DispVec {
    pub ca: CircularArray,
}

impl DispVec {
    /// `DispVec(n)`.
    pub fn new(n: usize, unsigned_wrap: bool) -> DispVec {
        DispVec {
            ca: CircularArray::new(n, unsigned_wrap),
        }
    }

    /// `SetFromRow(ima, k, j)`: row `j` of slice `k` (along x).
    pub fn set_from_row(&mut self, ima: &Volume, k: i64, j: i64) {
        self.ca.set_from_line(ima, k, j, true);
    }

    /// `SetFromColumn(ima, k, i)`: column `i` of slice `k` (along y).
    pub fn set_from_column(&mut self, ima: &Volume, k: i64, i: i64) {
        self.ca.set_from_line(ima, k, i, false);
    }

    /// `GetK_Matrix(sf)`: the `n × n` matrix whose row `i` integrates the undistorted line over
    /// the extent `[i - 0.5, i + 0.5]` of distorted voxel `i`, the positions scaled by `sf`.
    pub fn k_matrix(&mut self, sf: f64) -> Result<Mat, FslError> {
        let n = self.ca.n();
        let mut k = Mat::zeros(n, n);
        let old = self.ca.scale_factor();
        self.ca.set_scale_factor(sf);
        let mut indx = Vec::with_capacity(n);
        let mut val = Vec::with_capacity(n);
        for i in 0..n {
            self.nonzero_entries_of_row(i as u32, &mut indx, &mut val)?;
            if indx.len() > n {
                // FSL's arrays hold n entries: it writes past them
                return Err(dispvec_error(
                    "get_non_zero_entries_of_row: a voxel spans more voxels than the line has \
                     (FSL writes past its arrays)",
                ));
            }
            for (&c, &v) in indx.iter().zip(&val) {
                k.set(i, c, v);
            }
        }
        self.ca.set_scale_factor(old);
        Ok(k)
    }

    /// `GetS_Matrix(wrap)`: the second-difference (Laplacian) matrix, circular or with
    /// first differences at both ends.
    pub fn s_matrix(&self, wrap: bool) -> Mat {
        let n = self.ca.n();
        let mut s = Mat::zeros(n, n);
        let mut put = |r: usize, c: usize, v: f64| s.set(r - 1, c - 1, v);
        if wrap {
            put(1, 1, 2.0);
            put(1, 2, -1.0);
            put(1, n, -1.0);
        } else {
            put(1, 1, 1.0);
            put(1, 2, -1.0);
        }
        for i in 2..n {
            put(i, i - 1, -1.0);
            put(i, i, 2.0);
            put(i, i + 1, -1.0);
        }
        if wrap {
            put(n, 1, -1.0);
            put(n, n - 1, -1.0);
            put(n, n, 2.0);
        } else {
            put(n, n - 1, -1.0);
            put(n, n, 1.0);
        }
        s
    }

    /// `get_non_zero_entries_of_row(i, indx, val)`.
    fn nonzero_entries_of_row(
        &mut self,
        i: u32,
        indx: &mut Vec<usize>,
        val: &mut Vec<f64>,
    ) -> Result<(), FslError> {
        indx.clear();
        val.clear();
        let fi = f64::from(i);
        let ca = &mut self.ca;
        let i1 = ca.find(fi - 0.5);
        let i2 = ca.find(fi + 0.5);
        let (a1, b1) = (ca.at(i1), ca.at(i1 + 1));
        let x1 = (fi - 0.5 - a1) / (b1 - a1);
        let (a2, b2) = (ca.at(i2), ca.at(i2 + 1));
        let x2 = (fi + 0.5 - a2) / (b2 - a2);
        let mut push = |ca: &CircularArray, k: i32, v: f64| {
            indx.push(ca.index_in_range(k));
            val.push(v);
        };
        if i1 == i2 {
            if x1 < 0.5 {
                if x2 < 0.5 {
                    push(ca, i1, x2 - x1);
                } else {
                    push(ca, i1, 0.5 - x1);
                    push(ca, i1 + 1, x2 - 0.5);
                }
            } else {
                push(ca, i1 + 1, x2 - x1);
            }
        } else if i2 - i1 == 1 {
            if x1 < 0.5 {
                push(ca, i1, 0.5 - x1);
                if x2 < 0.5 {
                    push(ca, i1 + 1, 0.5 + x2);
                } else {
                    push(ca, i1 + 1, 1.0);
                    push(ca, i1 + 2, x2 - 0.5);
                }
            } else if x2 < 0.5 {
                push(ca, i1 + 1, 1.0 + x2 - x1);
            } else {
                push(ca, i1 + 1, 1.5 - x1);
                push(ca, i1 + 2, x2 - 0.5);
            }
        } else if i2 - i1 > 1 {
            if x1 < 0.5 {
                push(ca, i1, 0.5 - x1);
                push(ca, i1 + 1, 1.0);
            } else {
                push(ca, i1 + 1, 1.5 - x1);
            }
            for k in i1 + 2..i2 {
                push(ca, k, 1.0);
            }
            if x2 < 0.5 {
                push(ca, i2, 0.5 + x2);
            } else {
                push(ca, i2, 1.0);
                push(ca, i2 + 1, x2 - 0.5);
            }
        } else {
            return Err(dispvec_error(
                "get_non_zero_entries_of_row: This doesn't make sense.",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_field_is_the_identity_and_wraps() {
        let mut dv = DispVec::new(5, true);
        let k = dv.k_matrix(1.0).unwrap();
        for r in 0..5 {
            for c in 0..5 {
                assert_eq!(k.at(r, c), if r == c { 1.0 } else { 0.0 });
            }
        }
        // FSL-133: -1 % 5 as unsigned is 0 (2^32 - 1 = 5 * 858993459), 4 circularly
        let mut ca = CircularArray::new(5, true);
        let _ = ca.at(-1);
        assert!(ca.wrapped_differently);
        let mut ca = CircularArray::new(4, true);
        let _ = ca.at(-1);
        assert!(!ca.wrapped_differently);
    }
}
