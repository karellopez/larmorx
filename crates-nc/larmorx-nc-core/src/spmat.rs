// SPDX-License-Identifier: LicenseRef-FSL
// Translated to Rust from FSL 6.0.7.23 (fsl-miscmaths 2412.6: SpMat.h, cg.h, bicg.h). FSL is
// Copyright (C) 2007 University of Oxford (cg.h and bicg.h after IML++, Copyright NIST). Rust
// translation Copyright 2026 Karel Lopez Vilaret; modified from the originals. Distributed under
// the FSL Licence (see LICENSE).
//! fsl-miscmaths' sparse matrices (`SpMat<T>`, one sorted vector of row indices and one of
//! values per column, `T` = `float` or `double`) and the iterative solvers their `SolveForx`
//! uses: IML++'s conjugate gradients (`CG`) for symmetric positive definite matrices and
//! bi-conjugate gradients (`BiCG`) otherwise, with a diagonal preconditioner.
//!
//! The arithmetic follows the C++ types: a `float` matrix adds a `double` into a `float` value
//! as `(float)((double)v + d)` (`T += double`), adds two `float`s in `float`, and multiplies a
//! `double` vector by its values in `double`. NEWMAT's vector algebra is Armadillo's: element by
//! element, two roundings per `a + k·b`; `DotProduct` is [`crate::arma::dot_blas`] (Armadillo's
//! two-accumulator loop up to 32 elements, OpenBLAS's `ddot` beyond, on `blas_threads` threads:
//! FSL's programs set OpenBLAS to one); `NormFrobenius` is `sqrt(accu(square(x)))`.
//!
//! FSL's threaded paths (`_nt > 1`, the matrix's thread count) are computed here one after the
//! other with FSL's own partition: every operation stores the same values as one thread except
//! the product with a vector, whose per-thread partial sums FSL adds in a tree
//! ([`SpMat::mul_vec`]); its result depends on the thread count.

use crate::arma::{Mat, accu, dot_blas};
use crate::cnum::fmt_g;
use crate::volume::FslError;

/// `SpMatException(msg)`: `what()` is `"SpMat::" + msg`.
pub fn spmat_error(msg: &str) -> FslError {
    FslError {
        printed: Vec::new(),
        type_name: "MISCMATHS::SpMatException",
        what: format!("SpMat::{msg}"),
    }
}

/// The element type of a sparse matrix: `float` or `double`, with C++'s mixed arithmetic.
pub trait SpElem: Copy + PartialEq + std::fmt::Debug + Send + Sync + 'static {
    const ZERO: Self;
    /// `static_cast<T>(d)`.
    fn from_f64(d: f64) -> Self;
    /// `static_cast<double>(t)`.
    fn to_f64(self) -> f64;
    /// `t += d` for a `double` `d`: `(T)((double)t + d)`.
    fn add_f64(self, d: f64) -> Self;
    /// `t + u` in `T`.
    fn add_t(self, o: Self) -> Self;
    /// `t *= s` for a `double` `s`: `(T)((double)t * s)`.
    fn mul_f64(self, s: f64) -> Self;
    /// `t != 0` (as a condition: NaN is true, a negative zero false).
    fn nonzero(self) -> bool;
    /// `std::ostream << t` with precision `p` (a `float` is promoted).
    fn stream(self, p: usize) -> String {
        fmt_g(self.to_f64(), p)
    }
}

impl SpElem for f32 {
    const ZERO: f32 = 0.0;
    #[inline(always)]
    fn from_f64(d: f64) -> f32 {
        d as f32
    }
    #[inline(always)]
    fn to_f64(self) -> f64 {
        f64::from(self)
    }
    #[inline(always)]
    fn add_f64(self, d: f64) -> f32 {
        (f64::from(self) + d) as f32
    }
    #[inline(always)]
    fn add_t(self, o: f32) -> f32 {
        self + o
    }
    #[inline(always)]
    fn mul_f64(self, s: f64) -> f32 {
        (f64::from(self) * s) as f32
    }
    #[inline(always)]
    fn nonzero(self) -> bool {
        self != 0.0
    }
}

impl SpElem for f64 {
    const ZERO: f64 = 0.0;
    #[inline(always)]
    fn from_f64(d: f64) -> f64 {
        d
    }
    #[inline(always)]
    fn to_f64(self) -> f64 {
        self
    }
    #[inline(always)]
    fn add_f64(self, d: f64) -> f64 {
        self + d
    }
    #[inline(always)]
    fn add_t(self, o: f64) -> f64 {
        self + o
    }
    #[inline(always)]
    fn mul_f64(self, s: f64) -> f64 {
        self * s
    }
    #[inline(always)]
    fn nonzero(self) -> bool {
        self != 0.0
    }
}

/// `MatrixType`: what `SolveForx` may assume (`SYM_POSDEF` takes CG, the others BiCG).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatrixType {
    Unknown,
    Asym,
    Sym,
    SymPosdef,
}

/// NEWMAT's `NormFrobenius()` of a vector: `sqrt(accu(square(x)))`.
pub fn norm_frobenius(x: &[f64]) -> f64 {
    let sq: Vec<f64> = x.iter().map(|&v| v * v).collect();
    accu(&sq).sqrt()
}

/// The helper `Accumulator<T>`: a dense scratch column that remembers which positions were
/// touched, handed back in increasing order.
struct Accumulator<T: SpElem> {
    no: usize,
    sorted: bool,
    occ: Vec<bool>,
    val: Vec<T>,
    occi: Vec<u32>,
}

impl<T: SpElem> Accumulator<T> {
    fn new(sz: u32) -> Self {
        Accumulator {
            no: 0,
            sorted: true,
            occ: vec![false; sz as usize],
            val: vec![T::ZERO; sz as usize],
            occi: vec![0; sz as usize],
        }
    }
    fn reset(&mut self) {
        for i in 0..self.no {
            let k = self.occi[i] as usize;
            self.occ[k] = false;
            self.val[k] = T::ZERO;
        }
        self.no = 0;
    }
    /// `operator()(i)`: the value at `i`, marked as occupied.
    fn at(&mut self, i: u32) -> &mut T {
        let iu = i as usize;
        if !self.occ[iu] {
            if self.sorted && self.no > 0 && i < self.occi[self.no - 1] {
                self.sorted = false;
            }
            self.occ[iu] = true;
            self.occi[self.no] = i;
            self.no += 1;
        }
        &mut self.val[iu]
    }
    fn sort(&mut self) {
        if !self.sorted {
            self.occi[..self.no].sort_unstable();
            self.sorted = true;
        }
    }
    /// The occupied positions and their values, in increasing order.
    fn take(&mut self) -> (Vec<u32>, Vec<T>) {
        self.sort();
        let ri: Vec<u32> = self.occi[..self.no].to_vec();
        let val: Vec<T> = ri.iter().map(|&r| self.val[r as usize]).collect();
        (ri, val)
    }
}

/// `SpMat<T>`: an `m × n` sparse matrix stored by columns. Indices in the methods are 1-based,
/// as FSL's.
#[derive(Clone, Debug)]
pub struct SpMat<T: SpElem> {
    m: u32,
    n: u32,
    /// `_nz`: FSL's count of stored elements (not always the true count: see
    /// [`SpMat::add_assign`]); it sets the threads' shares in [`SpMat::mul_vec`].
    nz: u64,
    ri: Vec<Vec<u32>>,
    val: Vec<Vec<T>>,
    pw: bool,
    nt: u32,
}

impl<T: SpElem> Default for SpMat<T> {
    /// `SpMat()`: an empty 0 × 0 matrix, one thread.
    fn default() -> Self {
        SpMat {
            m: 0,
            n: 0,
            nz: 0,
            ri: Vec::new(),
            val: Vec::new(),
            pw: false,
            nt: 1,
        }
    }
}

impl<T: SpElem> SpMat<T> {
    /// `SpMat(m, n, nt)`: an `m × n` matrix of zeros.
    pub fn new(m: u32, n: u32, nt: u32) -> Self {
        SpMat {
            m,
            n,
            nz: 0,
            ri: vec![Vec::new(); n as usize],
            val: vec![Vec::new(); n as usize],
            pw: false,
            nt,
        }
    }

    /// `SpMat(m, n, irp, jcp, sp, nt)`: from compressed columns (`irp` row indices, `jcp`
    /// column starts, `sp` values as `double`, each cast to `T`), explicit zeros kept.
    pub fn from_csc(
        m: u32,
        n: u32,
        irp: &[u32],
        jcp: &[u32],
        sp: &[f64],
        nt: u32,
    ) -> Result<Self, FslError> {
        let mut out = SpMat::new(m, n, nt);
        out.nz = u64::from(jcp[n as usize]);
        let mut nz = 0u64;
        for c in 0..n as usize {
            let len = jcp[c + 1].wrapping_sub(jcp[c]) as i32;
            if len < 0 {
                // FSL would ask std::vector for a negative (huge) size; never reached by FSL's
                // own callers, refused here.
                return Err(spmat_error(
                    "SpMat: Compressed column input not self consistent",
                ));
            }
            if len > 0 {
                let s = jcp[c] as usize;
                let e = s + len as usize;
                out.ri[c] = irp[s..e].to_vec();
                out.val[c] = sp[s..e].iter().map(|&v| T::from_f64(v)).collect();
                nz += len as u64;
            }
        }
        if nz != out.nz {
            return Err(spmat_error(
                "SpMat: Compressed column input not self consistent",
            ));
        }
        Ok(out)
    }

    /// [`Self::from_csc`] of a [`crate::basisfield::Csc`].
    pub fn from_csc_struct(c: &crate::basisfield::Csc, nt: u32) -> Result<Self, FslError> {
        Self::from_csc(c.m, c.n, &c.irp, &c.jcp, &c.val, nt)
    }

    /// `SpMat(const NEWMAT::GeneralMatrix& M, nt)`: the non-zero elements of a dense matrix
    /// (a negative zero is not stored, a NaN is).
    pub fn from_dense(mat: &Mat, nt: u32) -> Self {
        let (m, n) = (mat.rows() as u32, mat.cols() as u32);
        let mut out = SpMat::new(m, n, nt);
        for c in 0..n as usize {
            let col = mat.colptr(c);
            let ri: Vec<u32> = (0..m).filter(|&i| col[i as usize] != 0.0).collect();
            if !ri.is_empty() {
                out.val[c] = ri.iter().map(|&i| T::from_f64(col[i as usize])).collect();
                out.nz += ri.len() as u64;
                out.ri[c] = ri;
            }
        }
        out
    }

    /// `SpMat(const std::string& fname, nt)`: a matrix from a text file of `row col value`
    /// lines (1-based, columns increasing, rows increasing within a column), whose last line is
    /// `m n 0`.
    pub fn from_file(fname: &str, nt: u32) -> Result<Self, FslError> {
        let rcv = crate::miscmaths::read_ascii_matrix(fname).map_err(|_| {
            spmat_error("SpMat::SpMat(string& fname): cannot read file given by fname")
        })?;
        let nr = rcv.rows();
        if nr == 0 || rcv.cols() < 3 {
            return Err(spmat_error(
                "SpMat::SpMat(string& fname): cannot read file given by fname",
            ));
        }
        let at = |r: usize, c: usize| rcv.at(r - 1, c - 1);
        let ui = |v: f64| crate::cnum::f64_to_i64(v + 0.5) as u32;
        if at(nr, 3) != 0.0 {
            return Err(spmat_error(
                "SpMat::SpMat(string& fname): Last row must have zero value and indicate matrix size",
            ));
        }
        let m = ui(at(nr, 1));
        let n = ui(at(nr, 2));
        let mut out = SpMat::new(m, n, nt);
        let mut col_count = vec![0u32; n as usize];
        let mut col = ui(at(1, 2));
        for indx in 1..nr {
            let ci = ui(at(indx, 2));
            if ci != col {
                if ci < col {
                    return Err(spmat_error(
                        "SpMat::SpMat(string& fname): Column index must be monotonously increasing",
                    ));
                }
                col = ci;
                if col > n {
                    return Err(spmat_error(
                        "SpMat::SpMat(string& fname): File internally inconsistent",
                    ));
                }
            }
            if col == 0 {
                // FSL writes before the start of its array (column 0); refused here.
                return Err(spmat_error(
                    "SpMat::SpMat(string& fname): File internally inconsistent",
                ));
            }
            col_count[col as usize - 1] += 1;
        }
        let mut indx = 1usize;
        for c in 0..n as usize {
            let cnt = col_count[c] as usize;
            let mut ri = vec![0u32; cnt];
            let mut val = vec![T::ZERO; cnt];
            for i in 0..cnt {
                let r = ui(at(indx, 1));
                // FSL compares the slot about to be written (`ri[i]`, still 0) with the new
                // row, so the check only refuses a row 0 after the first of a column (the
                // first such row meets the range check below).
                if i > 0 && ri[i] >= r {
                    return Err(spmat_error(
                        "SpMat::SpMat(string& fname): Row index must be monotonously increasing",
                    ));
                }
                if r < 1 || r > m {
                    return Err(spmat_error(
                        "SpMat::SpMat(string& fname): Row index outside 1 -- -m range",
                    ));
                }
                ri[i] = r - 1;
                val[i] = T::from_f64(at(indx, 3));
                out.nz += 1;
                indx += 1;
            }
            out.ri[c] = ri;
            out.val[c] = val;
        }
        Ok(out)
    }

    pub fn nrows(&self) -> u32 {
        self.m
    }
    pub fn ncols(&self) -> u32 {
        self.n
    }
    /// `NZ()`: FSL's count (an `unsigned long` returned as `unsigned int`).
    pub fn nz(&self) -> u32 {
        self.nz as u32
    }
    pub fn nthreads(&self) -> u32 {
        self.nt
    }
    pub fn set_nthreads(&mut self, nt: u32) {
        self.nt = nt;
    }
    pub fn warnings_on(&mut self) {
        self.pw = true;
    }
    pub fn warnings_off(&mut self) {
        self.pw = false;
    }
    /// Column `c`'s (0-based) row indices (0-based) and values.
    pub fn column(&self, c: usize) -> (&[u32], &[T]) {
        (&self.ri[c], &self.val[c])
    }

    /// `IsSorted()`: every column's row indices non-decreasing.
    pub fn is_sorted(&self) -> bool {
        self.ri.iter().all(|r| r.windows(2).all(|w| w[1] >= w[0]))
    }

    /// `AsNEWMAT()`: the dense matrix.
    pub fn as_dense(&self) -> Mat {
        let mut out = Mat::zeros(self.m as usize, self.n as usize);
        for c in 0..self.n as usize {
            for (k, &r) in self.ri[c].iter().enumerate() {
                out.set(r as usize, c, self.val[c][k].to_f64());
            }
        }
        out
    }

    /// `Print(fname, precision)`'s text: `row  col  value` for every stored non-zero value
    /// (1-based), then `m  n  0`.
    pub fn print_text(&self, precision: usize) -> String {
        let mut s = String::new();
        for c in 0..self.n as usize {
            for (k, &r) in self.ri[c].iter().enumerate() {
                let v = self.val[c][k];
                if v.nonzero() {
                    s.push_str(&format!("{}  {}  {}\n", r + 1, c + 1, v.stream(precision)));
                }
            }
        }
        s.push_str(&format!("{}  {}  0\n", self.m, self.n));
        s
    }

    /// `Save(fname, precision)`.
    pub fn save(&self, fname: &str, precision: usize) -> Result<(), FslError> {
        if fname.is_empty() {
            return Err(spmat_error("SpMat::Save: Must specify filename"));
        }
        // std::ofstream does not throw: a file that cannot be opened is silently not written.
        let _ = std::fs::write(fname, self.print_text(precision));
        Ok(())
    }

    /// `found(ri, key, pos)`: binary search; `pos` is where `key` is or would be inserted.
    fn found(ri: &[u32], key: u32) -> (bool, usize) {
        if ri.is_empty() || key < ri[0] {
            return (false, 0);
        }
        if key > *ri.last().unwrap() {
            return (false, ri.len());
        }
        let mut ll: i64 = -1;
        let mut pos: i64 = ri.len() as i64;
        while pos - ll > 1 {
            let mp = (pos + ll) >> 1;
            if key > ri[mp as usize] {
                ll = mp;
            } else {
                pos = mp;
            }
        }
        (ri[pos as usize] == key, pos as usize)
    }

    /// `Peek(r, c)`.
    pub fn peek(&self, r: u32, c: u32) -> Result<T, FslError> {
        if r < 1 || r > self.m || c < 1 || c > self.n {
            return Err(spmat_error("Peek: index out of range"));
        }
        let ci = (c - 1) as usize;
        match Self::found(&self.ri[ci], r - 1) {
            (true, i) => Ok(self.val[ci][i]),
            _ => Ok(T::ZERO),
        }
    }

    /// `here(r, c)`: the stored value at `(r, c)`, inserted as a zero when absent.
    fn here(&mut self, r: u32, c: u32) -> Result<&mut T, FslError> {
        if r < 1 || r > self.m || c < 1 || c > self.n {
            return Err(spmat_error("here: index out of range"));
        }
        let ci = (c - 1) as usize;
        let (f, i) = Self::found(&self.ri[ci], r - 1);
        if !f {
            self.ri[ci].insert(i, r - 1);
            self.val[ci].insert(i, T::ZERO);
            self.nz += 1;
        }
        Ok(&mut self.val[ci][i])
    }

    /// `Set(r, c, v)`.
    pub fn set(&mut self, r: u32, c: u32, v: T) -> Result<(), FslError> {
        *self.here(r, c)? = v;
        Ok(())
    }

    /// `AddTo(r, c, v)`: `here(r, c) += v` in `T`.
    pub fn add_to(&mut self, r: u32, c: u32, v: T) -> Result<(), FslError> {
        let h = self.here(r, c)?;
        *h = h.add_t(v);
        Ok(())
    }

    /// `SetColumn(c, col, eps)`: column `c` replaced by the elements of `col` greater than
    /// `eps` (so negative values are dropped too, as FSL does).
    pub fn set_column(&mut self, c: u32, col: &[f64], eps: f64) -> Result<(), FslError> {
        if c < 1 || c > self.n {
            return Err(spmat_error("SetColumn: column index out of range"));
        }
        if col.len() != self.m as usize {
            return Err(spmat_error("SetColumn: column size mismatch"));
        }
        let mut acc = Accumulator::<T>::new(self.m);
        for (i, &v) in col.iter().enumerate() {
            if v > eps {
                *acc.at(i as u32) = T::from_f64(v);
            }
        }
        let ci = (c - 1) as usize;
        let old_sz = self.ri[ci].len() as u64;
        let (ri, val) = acc.take();
        let no = ri.len() as u64;
        self.ri[ci] = ri;
        self.val[ci] = val;
        self.nz = self.nz.wrapping_add(no).wrapping_sub(old_sz);
        Ok(())
    }

    /// `MultiplyColumns(vals)`: each column times its value (`vals` a row or column vector).
    pub fn multiply_columns(&mut self, vals: &Mat) -> Result<(), FslError> {
        if vals.rows() != 1 && vals.cols() != 1 {
            return Err(spmat_error(
                "MultiplyColumns: vals must be a row- or column-vector",
            ));
        }
        let n = self.n as usize;
        if vals.rows() != n && vals.cols() != n {
            return Err(spmat_error(
                "MultiplyColumns: vals must have as many elements as there are columns",
            ));
        }
        for ci in 0..n {
            // A 1 × 1 `vals` is read as a row vector; a vector of the other orientation of
            // the wrong length reads out of range in FSL (NEWMAT's bounds check throws).
            let v = if vals.rows() == 1 {
                if ci >= vals.cols() {
                    return Err(index_error());
                }
                vals.at(0, ci)
            } else {
                if ci >= vals.rows() {
                    return Err(index_error());
                }
                vals.at(ci, 0)
            };
            for x in &mut self.val[ci] {
                *x = x.mul_f64(v);
            }
        }
        Ok(())
    }

    /// `operator*=(double s)`.
    pub fn scale(&mut self, s: f64) {
        for col in &mut self.val {
            for x in col.iter_mut() {
                *x = x.mul_f64(s);
            }
        }
    }

    /// `s * M` (and `M * s`): a copy scaled.
    pub fn scaled(&self, s: f64) -> Self {
        let mut out = self.clone();
        out.scale(s);
        out
    }

    /// The member `operator-(const SpMat& M)`, which FSL declares as a unary minus but which
    /// takes an argument: `A - B` resolves to it (a non-template member beats the global
    /// template) and returns `-B`, whatever `A` is. Kept as FSL computes it.
    pub fn minus_member(&self, other: &Self) -> Self {
        other.scaled(-1.0)
    }

    /// `operator+=` (`s` = 1) and `operator-=` (`s` = −1).
    pub fn add_assign(&mut self, other: &Self) -> Result<(), FslError> {
        self.add_scaled(other, 1.0)
    }
    pub fn sub_assign(&mut self, other: &Self) -> Result<(), FslError> {
        self.add_scaled(other, -1.0)
    }

    fn add_scaled(&mut self, other: &Self, s: f64) -> Result<(), FslError> {
        if self.same_sparsity(other) {
            self.add_same_sparsity(other, s);
            Ok(())
        } else {
            self.add_diff_sparsity(other, s)
        }
    }

    fn same_sparsity(&self, other: &Self) -> bool {
        if self.m != other.m || self.n != other.n {
            return false;
        }
        if (0..self.n as usize).any(|c| self.ri[c].len() != other.ri[c].len()) {
            return false;
        }
        (0..self.n as usize).all(|c| self.ri[c] == other.ri[c])
    }

    /// `add_same_sparsity_mat_to_me` (and its threaded helper, column by column alike):
    /// `val += s·Mval` (`T += double`).
    fn add_same_sparsity(&mut self, other: &Self, s: f64) {
        for c in 0..self.n as usize {
            for (v, mv) in self.val[c].iter_mut().zip(other.val[c].iter()) {
                *v = v.add_f64(s * mv.to_f64());
            }
        }
    }

    /// `add_diff_sparsity_mat_to_me` (and its threaded helper alike): for each column of
    /// `other` holding values, `s·Mval` then this column's values added into an accumulator,
    /// handed back sorted. A column `other` leaves empty keeps its values, but FSL does not
    /// count them in `_nz` (it restarts the count from zero), so `_nz` falls short of the true
    /// count after such an addition.
    fn add_diff_sparsity(&mut self, other: &Self, s: f64) -> Result<(), FslError> {
        if self.m != other.m || self.n != other.n {
            return Err(spmat_error(
                "add_diff_sparsity_mat_to_me: Size mismatch between matrices",
            ));
        }
        let mut acc = Accumulator::<T>::new(self.m);
        self.nz = 0;
        for c in 0..self.n as usize {
            acc.reset();
            if !other.ri[c].is_empty() {
                for (k, &r) in other.ri[c].iter().enumerate() {
                    let a = acc.at(r);
                    *a = a.add_f64(s * other.val[c][k].to_f64());
                }
                for (k, &r) in self.ri[c].iter().enumerate() {
                    let v = self.val[c][k];
                    let a = acc.at(r);
                    *a = a.add_t(v);
                }
                let (ri, val) = acc.take();
                self.nz += ri.len() as u64;
                self.ri[c] = ri;
                self.val[c] = val;
            }
        }
        Ok(())
    }

    /// `columns_per_thread(balance)`: the first column of each thread's share (and `n` last);
    /// balanced by FSL's count of stored elements, or equal numbers of columns.
    fn columns_per_thread(&self, balance: bool) -> Vec<u32> {
        let nt = self.nt as usize;
        let mut cpt = vec![0u32; nt + 1];
        if balance {
            let per = (self.nz as f64 / f64::from(self.nt)).round() as u32;
            let mut cum: u32 = 0;
            let mut ti = 1usize;
            for c in 0..self.n as usize {
                let len = self.ri[c].len() as u32;
                cum = cum.wrapping_add(len);
                if cum >= per {
                    let this_c = (cum.wrapping_sub(per) as i32).wrapping_abs();
                    let prev_c = (cum.wrapping_sub(len).wrapping_sub(per) as i32).wrapping_abs();
                    if this_c < prev_c {
                        cpt[ti] = c as u32 + 1;
                        cum = 0;
                    } else {
                        cpt[ti] = c as u32;
                        cum = len;
                    }
                    if ti < nt - 1 {
                        ti += 1;
                    } else {
                        break;
                    }
                }
            }
            cpt[nt] = self.n;
        } else {
            let dcpt = f64::from(self.n) / f64::from(self.nt);
            for (i, x) in cpt.iter_mut().enumerate().take(nt).skip(1) {
                *x = (i as f64 * dcpt).floor() as u32;
            }
            cpt[nt] = self.n;
        }
        cpt
    }

    /// The products of the columns `first..last` with `x` added into `b` (`bp[ri] += wgt·val`
    /// in `double`).
    fn mul_cols_into(&self, first: u32, last: u32, x: &[f64], b: &mut [f64]) {
        for c in first..last {
            let c = c as usize;
            let wgt = x[c];
            for (k, &r) in self.ri[c].iter().enumerate() {
                b[r as usize] += wgt * self.val[c][k].to_f64();
            }
        }
    }

    /// `operator*(const NEWMAT::ColumnVector& x)`. With more than one thread FSL gives each
    /// thread a share of the columns (balanced by `_nz`) and its own vector, and adds the
    /// vectors in a tree (thread `t` adds `t − 1`, then `t − 2`, ... while
    /// `(nt − 1 − t)/step` is even), into the last thread's: computed here in that order.
    pub fn mul_vec(&self, x: &[f64]) -> Result<Vec<f64>, FslError> {
        if self.n as usize != x.len() {
            return Err(spmat_error(
                "operator*: # of rows in vector must match # of columns in matrix",
            ));
        }
        let m = self.m as usize;
        if self.nt == 1 {
            let mut b = vec![0.0; m];
            self.mul_cols_into(0, self.n, x, &mut b);
            return Ok(b);
        }
        let nt = self.nt as usize;
        let cpt = self.columns_per_thread(true);
        let mut bufs: Vec<Vec<f64>> = (0..nt)
            .map(|t| {
                let mut b = vec![0.0; m];
                self.mul_cols_into(cpt[t], cpt[t + 1], x, &mut b);
                b
            })
            .collect();
        // Thread t (in increasing order: its neighbours are always lower and finished).
        for tid in 0..nt {
            let mut step = 1usize;
            while step < nt {
                if ((nt - 1 - tid) / step) % 2 == 1 {
                    break;
                }
                if tid < step {
                    break;
                }
                let nb = tid - step;
                let (lo, hi) = bufs.split_at_mut(tid);
                for (my, ne) in hi[0].iter_mut().zip(lo[nb].iter()) {
                    *my += *ne;
                }
                step *= 2;
            }
        }
        Ok(bufs.pop().unwrap())
    }

    /// `trans_mult(x)` (`TransMult`): `Mᵀx`, each element a sum over its column from 0 in
    /// `double` (the threaded version computes the same columns alike).
    pub fn trans_mult(&self, x: &[f64]) -> Result<Vec<f64>, FslError> {
        if self.m as usize != x.len() {
            return Err(spmat_error(
                "trans_mult: # of rows in vector must match # of columns in transpose of matrix",
            ));
        }
        Ok((0..self.n as usize)
            .map(|c| {
                let mut res = 0.0;
                for (k, &r) in self.ri[c].iter().enumerate() {
                    res += self.val[c][k].to_f64() * x[r as usize];
                }
                res
            })
            .collect())
    }

    /// `t()`: the transpose (made with FSL's default thread count, `nt`; it keeps this
    /// matrix's `_nz`).
    pub fn t(&self, nt: u32) -> Self {
        let mut out = SpMat::new(self.n, self.m, nt);
        let mut no_per_row = vec![0usize; self.m as usize];
        for c in 0..self.n as usize {
            for &r in &self.ri[c] {
                no_per_row[r as usize] += 1;
            }
        }
        for i in 0..self.m as usize {
            out.ri[i] = vec![0; no_per_row[i]];
            out.val[i] = vec![T::ZERO; no_per_row[i]];
        }
        no_per_row.iter_mut().for_each(|x| *x = 0);
        for c in 0..self.n as usize {
            for (k, &r) in self.ri[c].iter().enumerate() {
                let ru = r as usize;
                out.ri[ru][no_per_row[ru]] = c as u32;
                out.val[ru][no_per_row[ru]] = self.val[c][k];
                no_per_row[ru] += 1;
            }
        }
        out.nz = self.nz;
        out
    }

    /// `operator|=(rh)`: `rh`'s columns appended on the right.
    pub fn hor_concat(&mut self, rh: &Self) -> Result<(), FslError> {
        if self.m != rh.m {
            return Err(spmat_error("operator|=: Matrices must have same # of rows"));
        }
        self.ri.extend(rh.ri.iter().cloned());
        self.val.extend(rh.val.iter().cloned());
        self.n += rh.n;
        self.nz += rh.nz;
        Ok(())
    }

    /// `operator&=(bh)`: `bh`'s rows appended below.
    pub fn vert_concat(&mut self, bh: &Self) -> Result<(), FslError> {
        if self.n != bh.n {
            return Err(spmat_error(
                "operator&=: Matrices must have same # of columns",
            ));
        }
        for c in 0..self.n as usize {
            let m = self.m;
            self.ri[c].extend(bh.ri[c].iter().map(|&r| m + r));
            self.val[c].extend(bh.val[c].iter().copied());
        }
        self.m += bh.m;
        self.nz += bh.nz;
        Ok(())
    }

    /// `operator*(lh, rh)`: the product of two sparse matrices, column by column through an
    /// accumulator (`acc += (double)rval·lval`, `T += double`); the result has FSL's default
    /// thread count `nt`.
    pub fn mul_spmat(lh: &Self, rh: &Self, nt: u32) -> Result<Self, FslError> {
        if lh.n != rh.m {
            return Err(spmat_error(
                "operator*: Left hand matrix must have same # of columns as right hand has rows",
            ));
        }
        let mut out = SpMat::new(lh.m, rh.n, nt);
        let mut acc = Accumulator::<T>::new(lh.m);
        for cr in 0..rh.n as usize {
            acc.reset();
            for (i, &rr) in rh.ri[cr].iter().enumerate() {
                let wgt = rh.val[cr][i].to_f64();
                let lc = rr as usize;
                for (j, &lr) in lh.ri[lc].iter().enumerate() {
                    let a = acc.at(lr);
                    *a = a.add_f64(wgt * lh.val[lc][j].to_f64());
                }
            }
            let (ri, val) = acc.take();
            out.nz += ri.len() as u64;
            out.ri[cr] = ri;
            out.val[cr] = val;
        }
        Ok(out)
    }

    /// `TransMultSelf()`: `t() * (*this)`.
    pub fn trans_mult_self(&self, nt: u32) -> Result<Self, FslError> {
        Self::mul_spmat(&self.t(nt), self, nt)
    }

    /// `TransMult(B)`: `t() * B`.
    pub fn trans_mult_spmat(&self, b: &Self, nt: u32) -> Result<Self, FslError> {
        Self::mul_spmat(&self.t(nt), b, nt)
    }

    /// `operator==`: same sizes, `_nz`, row indices and values.
    pub fn equals(&self, rh: &Self) -> bool {
        if self.m != rh.m || self.n != rh.n || self.nz != rh.nz {
            return false;
        }
        (0..self.n as usize).all(|c| self.ri[c] == rh.ri[c] && self.val[c] == rh.val[c])
    }

    /// `SolveForx(b, type, tol, miter, C, x_init)`: `x` in `b = M x`, from `x_init` (or zeros),
    /// by preconditioned CG (`SymPosdef`) or BiCG (the others), with `C` or else the diagonal
    /// preconditioner. The second value is the warning FSL prints when warnings are on and
    /// the tolerance was not reached.
    pub fn solve_for_x(
        &self,
        b: &[f64],
        mtype: MatrixType,
        tol: f64,
        miter: u32,
        x_init: Option<&[f64]>,
        blas_threads: usize,
    ) -> Result<(Vec<f64>, Option<String>), FslError> {
        if self.m != self.n {
            return Err(spmat_error("SolveForx: Matrix must be square"));
        }
        if self.m as usize != b.len() {
            return Err(spmat_error("SolveForx: Mismatch between matrix and vector"));
        }
        let mut x = match x_init {
            Some(xi) if xi.len() == self.n as usize => xi.to_vec(),
            Some(xi) if !xi.is_empty() => {
                return Err(spmat_error(
                    "SolveForx: initialisation vector has incorrect size",
                ));
            }
            _ => vec![0.0; self.n as usize],
        };
        let mut liter = miter as i32;
        let ltol = tol;
        let mut tol = tol;
        let pre = DiagPrecond::new(self)?;
        let status = match mtype {
            MatrixType::SymPosdef => cg(self, &mut x, b, &pre, &mut liter, &mut tol, blas_threads)?,
            _ => bicg(self, &mut x, b, &pre, &mut liter, &mut tol, blas_threads)?,
        };
        let warning = (status != 0 && self.pw).then(|| {
            format!(
                "SpMat::SolveForx: Warning requested tolerence not obtained.\nRequested tolerance was {}, and achieved tolerance was {}\nThis may or may not be a problem in your application, but you should look into it\n",
                fmt_g(ltol, 6),
                fmt_g(tol, 6)
            )
        });
        Ok((x, warning))
    }
}

/// NEWMAT's out-of-range element access (`MultiplyColumns` reading past a vector): Armadillo
/// 14.2.1's `arma_stop_bounds_error`, a `std::out_of_range`.
fn index_error() -> FslError {
    FslError {
        printed: Vec::new(),
        type_name: "std::out_of_range",
        what: "Mat::operator(): index out of bounds".to_owned(),
    }
}

/// A matrix for the iterative solvers: `A * x` and `A.trans_mult(x)`.
pub trait LinOp {
    fn apply(&self, x: &[f64]) -> Result<Vec<f64>, FslError>;
    fn apply_trans(&self, x: &[f64]) -> Result<Vec<f64>, FslError>;
}

impl<T: SpElem> LinOp for SpMat<T> {
    fn apply(&self, x: &[f64]) -> Result<Vec<f64>, FslError> {
        self.mul_vec(x)
    }
    fn apply_trans(&self, x: &[f64]) -> Result<Vec<f64>, FslError> {
        self.trans_mult(x)
    }
}

/// A preconditioner for the iterative solvers: `M.solve(x)` and `M.trans_solve(x)`.
pub trait Precond {
    fn solve(&self, x: &[f64]) -> Result<Vec<f64>, FslError>;
    fn trans_solve(&self, x: &[f64]) -> Result<Vec<f64>, FslError> {
        self.solve(x)
    }
}

/// `DiagPrecond<T>`: the matrix's diagonal (in `T`); `solve` divides by it in `double`.
pub struct DiagPrecond<T: SpElem> {
    diag: Vec<T>,
}

impl<T: SpElem> DiagPrecond<T> {
    pub fn new(m: &SpMat<T>) -> Result<Self, FslError> {
        if m.nrows() != m.ncols() {
            return Err(spmat_error(
                "Preconditioner: Matrix to condition must be square",
            ));
        }
        let mut diag = Vec::with_capacity(m.nrows() as usize);
        for i in 0..m.nrows() {
            let d = m.peek(i + 1, i + 1)?;
            if d.to_f64() == 0.0 {
                return Err(spmat_error("DiagPrecond: Cannot condition singular matrix"));
            }
            diag.push(d);
        }
        Ok(DiagPrecond { diag })
    }
}

impl<T: SpElem> Precond for DiagPrecond<T> {
    fn solve(&self, x: &[f64]) -> Result<Vec<f64>, FslError> {
        if x.len() != self.diag.len() {
            return Err(spmat_error(
                "DiagPrecond::solve: Vector x has incompatible size",
            ));
        }
        Ok(x.iter()
            .zip(&self.diag)
            .map(|(&xi, d)| xi / d.to_f64())
            .collect())
    }
}

/// `x + k·y` element by element (Armadillo's `x + k*y`).
fn axpy(x: &[f64], k: f64, y: &[f64]) -> Vec<f64> {
    x.iter().zip(y).map(|(&a, &b)| a + b * k).collect()
}

/// IML++'s `CG(A, x, b, M, max_iter, tol)` (`cg.h`): preconditioned conjugate gradients.
/// Returns 0 when the relative residual reached `tol` (then `tol` is the residual and
/// `max_iter` the iterations), 1 otherwise (`tol` the last residual).
pub fn cg<A: LinOp, P: Precond>(
    a: &A,
    x: &mut [f64],
    b: &[f64],
    m: &P,
    max_iter: &mut i32,
    tol: &mut f64,
    blas_threads: usize,
) -> Result<i32, FslError> {
    let mut normb = norm_frobenius(b);
    let ax = a.apply(x)?;
    let mut r: Vec<f64> = b.iter().zip(&ax).map(|(&bi, &ai)| bi - ai).collect();
    if normb == 0.0 {
        normb = 1.0;
    }
    let mut resid = norm_frobenius(&r) / normb;
    if resid <= *tol {
        *tol = resid;
        *max_iter = 0;
        return Ok(0);
    }
    let mut p: Vec<f64> = Vec::new();
    let mut rho_1 = 0.0;
    let mut i = 1;
    while i <= *max_iter {
        let z = m.solve(&r)?;
        let rho = dot_blas(&r, &z, blas_threads);
        if i == 1 {
            p = z;
        } else {
            let beta = rho / rho_1;
            p = axpy(&z, beta, &p);
        }
        let q = a.apply(&p)?;
        let alpha = rho / dot_blas(&p, &q, blas_threads);
        for (xi, pi) in x.iter_mut().zip(&p) {
            *xi += *pi * alpha;
        }
        for (ri, qi) in r.iter_mut().zip(&q) {
            *ri -= *qi * alpha;
        }
        resid = norm_frobenius(&r) / normb;
        if resid <= *tol {
            *tol = resid;
            *max_iter = i;
            return Ok(0);
        }
        rho_1 = rho;
        i += 1;
    }
    *tol = resid;
    Ok(1)
}

/// IML++'s `BiCG(A, x, b, M, max_iter, tol)` (`bicg.h`): preconditioned bi-conjugate gradients.
/// Returns 0 on convergence (the residual strictly below `tol`), 1 when the iterations ran
/// out, 2 on a breakdown (`rho` exactly zero).
pub fn bicg<A: LinOp, P: Precond>(
    a: &A,
    x: &mut [f64],
    b: &[f64],
    m: &P,
    max_iter: &mut i32,
    tol: &mut f64,
    blas_threads: usize,
) -> Result<i32, FslError> {
    let mut normb = norm_frobenius(b);
    let ax = a.apply(x)?;
    let mut r: Vec<f64> = b.iter().zip(&ax).map(|(&bi, &ai)| bi - ai).collect();
    let mut rtilde = r.clone();
    if normb == 0.0 {
        normb = 1.0;
    }
    let mut resid = norm_frobenius(&r) / normb;
    if resid <= *tol {
        *tol = resid;
        *max_iter = 0;
        return Ok(0);
    }
    let mut p: Vec<f64> = Vec::new();
    let mut ptilde: Vec<f64> = Vec::new();
    let mut rho_2 = 0.0;
    let mut i = 1;
    while i <= *max_iter {
        let z = m.solve(&r)?;
        let ztilde = m.trans_solve(&rtilde)?;
        let rho_1 = dot_blas(&z, &rtilde, blas_threads);
        if rho_1 == 0.0 {
            *tol = norm_frobenius(&r) / normb;
            *max_iter = i;
            return Ok(2);
        }
        if i == 1 {
            p = z;
            ptilde = ztilde;
        } else {
            let beta = rho_1 / rho_2;
            p = axpy(&z, beta, &p);
            ptilde = axpy(&ztilde, beta, &ptilde);
        }
        let q = a.apply(&p)?;
        let qtilde = a.apply_trans(&ptilde)?;
        let alpha = rho_1 / dot_blas(&ptilde, &q, blas_threads);
        for (xi, pi) in x.iter_mut().zip(&p) {
            *xi += *pi * alpha;
        }
        for (ri, qi) in r.iter_mut().zip(&q) {
            *ri -= *qi * alpha;
        }
        for (ri, qi) in rtilde.iter_mut().zip(&qtilde) {
            *ri -= *qi * alpha;
        }
        rho_2 = rho_1;
        resid = norm_frobenius(&r) / normb;
        if resid < *tol {
            *tol = resid;
            *max_iter = i;
            return Ok(0);
        }
        i += 1;
    }
    *tol = resid;
    Ok(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_index_out_of_range_is_armadillos_out_of_range() {
        assert_eq!(index_error().type_name, "std::out_of_range");
    }

    fn small() -> SpMat<f64> {
        // [[4, 1, 0], [1, 3, 0], [0, 0, 2]] as compressed columns.
        SpMat::from_csc(
            3,
            3,
            &[0, 1, 0, 1, 2],
            &[0, 2, 4, 5],
            &[4.0, 1.0, 1.0, 3.0, 2.0],
            1,
        )
        .unwrap()
    }

    #[test]
    fn basic_ops() {
        let mut a = small();
        assert_eq!(a.peek(2, 1).unwrap(), 1.0);
        assert_eq!(a.peek(3, 1).unwrap(), 0.0);
        a.set(3, 1, 5.0).unwrap();
        assert_eq!(a.nz(), 6);
        assert_eq!(a.column(0).0, &[0, 1, 2]);
        let y = a.mul_vec(&[1.0, 2.0, 3.0]).unwrap();
        assert_eq!(y, vec![6.0, 7.0, 11.0]);
        let yt = a.trans_mult(&[1.0, 2.0, 3.0]).unwrap();
        assert_eq!(yt, vec![21.0, 7.0, 6.0]);
        let at = a.t(1);
        assert_eq!(at.peek(1, 3).unwrap(), 5.0);
        assert!(a.peek(4, 1).is_err());
    }

    #[test]
    fn threaded_product_tree() {
        let mut a = SpMat::<f64>::new(4, 7, 1);
        for c in 1..=7 {
            for r in 1..=4 {
                a.set(r, c, 0.1 * f64::from(r * c) + 1e-17 * f64::from(c))
                    .unwrap();
            }
        }
        let x: Vec<f64> = (0..7).map(|i| 1.0 / f64::from(i + 3)).collect();
        let one = a.mul_vec(&x).unwrap();
        for nt in [2u32, 3, 4, 5] {
            let mut b = a.clone();
            b.set_nthreads(nt);
            let v = b.mul_vec(&x).unwrap();
            for (p, q) in one.iter().zip(&v) {
                assert!((p - q).abs() < 1e-12);
            }
        }
    }

    #[test]
    fn cg_solves() {
        let a = small();
        let (x, w) = a
            .solve_for_x(&[1.0, 2.0, 3.0], MatrixType::SymPosdef, 1e-12, 50, None, 1)
            .unwrap();
        assert!(w.is_none());
        let r = a.mul_vec(&x).unwrap();
        assert!((r[0] - 1.0).abs() < 1e-10 && (r[2] - 3.0).abs() < 1e-10);
        let (x2, _) = a
            .solve_for_x(&[1.0, 2.0, 3.0], MatrixType::Unknown, 1e-12, 50, None, 1)
            .unwrap();
        assert!((x2[1] - x[1]).abs() < 1e-9);
    }

    #[test]
    fn minus_is_fsls() {
        let a = small();
        let b = small().scaled(2.0);
        let d = a.minus_member(&b);
        assert_eq!(d.peek(1, 1).unwrap(), -8.0);
    }
}
