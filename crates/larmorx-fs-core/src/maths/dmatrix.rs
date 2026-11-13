// SPDX-License-Identifier: Apache-2.0
//! Double matrices (spec §4, §12): allocation, the size check, weighted sums, copies, the
//! transpose, printing, the product and the dot product in x87 extended precision, the
//! comparison with and copy of a float matrix, constants and the largest magnitude. (The list
//! statistics, `DListStats`, are fs.core's [`Process::dlist_stats`].)

use larmorx_core::x87::F80;

use super::matrix::Matrix;
use super::{Output, Returned};
use crate::clib::printf::sprintf_f64;
use crate::process::{BUILD_ROOT, Process, Stop, codes};
use crate::sse;

/// A double matrix: real, its elements row by row (one-based in FreeSurfer).
#[derive(Clone, Debug, PartialEq)]
pub struct DMatrix {
    pub rows: i32,
    pub cols: i32,
    pub data: Vec<f64>,
}

impl DMatrix {
    /// A matrix of zeros (a negative row count gives no elements).
    pub fn zeros(rows: i32, cols: i32) -> DMatrix {
        let n = (rows.max(0) as usize) * (cols.max(0) as usize);
        DMatrix {
            rows,
            cols,
            data: vec![0.0; n],
        }
    }

    /// Element (r, c), one-based; 0 outside the matrix (§1).
    pub fn at(&self, r: i32, c: i32) -> f64 {
        if r < 1 || c < 1 || r > self.rows || c > self.cols {
            return 0.0;
        }
        self.data[(r as usize - 1) * self.cols as usize + (c as usize - 1)]
    }

    fn idx(&self, r: i32, c: i32) -> usize {
        (r as usize - 1) * self.cols as usize + (c as usize - 1)
    }
}

/// The stream a function writes its messages to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StreamSel {
    Out,
    Err,
}

/// The size checks of `DMatrixCheckDims` (§4).
pub mod check {
    /// The rows must agree.
    pub const ROWS: i32 = 1;
    /// The columns must agree.
    pub const COLS: i32 = 2;
    /// Both.
    pub const SAME: i32 = 3;
    /// m1's columns must equal m2's rows.
    pub const MULT: i32 = 4;
}

/// A double widened as an x87 instruction's memory operand: a signalling NaN stays signalling
/// (§12).
fn mem_operand(x: f64) -> F80 {
    let b = x.to_bits();
    if x.is_nan() && b & 0x0008_0000_0000_0000 == 0 {
        let m = (1u64 << 63) | ((b & 0x000f_ffff_ffff_ffff) << 11);
        let se = if b >> 63 != 0 { 0xffffu128 } else { 0x7fffu128 };
        F80::from_bits((se << 64) | u128::from(m))
    } else {
        F80::from_f64(x)
    }
}

impl Process {
    fn put_stream(&mut self, s: Option<StreamSel>, b: &[u8]) {
        match s {
            Some(StreamSel::Out) => self.put_out(b),
            Some(StreamSel::Err) => self.put_err(b),
            None => {}
        }
    }

    /// `DMatrixAlloc(rows, cols, type)` (§4).
    pub fn dmatrix_alloc(
        &mut self,
        rows: i32,
        cols: i32,
        kind: i32,
    ) -> Result<Option<DMatrix>, Stop> {
        if kind == super::matrix::MATRIX_COMPLEX {
            self.put_out(b"ERROR: DMatrixAlloc(): complex types not supported\n");
            return Ok(None);
        }
        if kind != super::matrix::MATRIX_REAL {
            self.put_out(b"ERROR: DMatrixAlloc(): types 1 not recognized\n");
            return Ok(None);
        }
        let n = i64::from(rows) * i64::from(cols);
        if n < -2 || n > i64::from(i32::MAX) {
            self.put_out(
                format!("ERROR: DMatrixAlloc({rows}, {cols}): allocation failed\n").as_bytes(),
            );
            return Ok(None);
        }
        if rows < -1 {
            self.errno = 12;
            self.error_exit(
                codes::NO_MEMORY,
                format!("DMatrixAlloc({rows}, {cols}): could not allocate rptr").as_bytes(),
            )?;
            return Ok(None);
        }
        Ok(Some(DMatrix::zeros(rows, cols)))
    }

    /// `DMatrixCheckDims(m1, m2, kind, stream, prefix)` (§4): 0 when they agree, else 1 after the
    /// two lines on the stream.
    pub fn dmatrix_check_dims(
        &mut self,
        m1: Option<&DMatrix>,
        m2: Option<&DMatrix>,
        kind: i32,
        stream: Option<StreamSel>,
        prefix: &[u8],
    ) -> i32 {
        let what = match (m1, m2) {
            (None, _) => Some(("matrix 1 is NULL".to_owned(), 309)),
            (_, None) => Some(("matrix 2 is NULL".to_owned(), 314)),
            (Some(a), Some(b)) => {
                if (kind == check::ROWS || kind == check::SAME) && a.rows != b.rows {
                    Some((format!("rows mismatch {}, {}", a.rows, b.rows), 319))
                } else if (kind == check::COLS || kind == check::SAME) && a.cols != b.cols {
                    Some((format!("cols mismatch {}, {}", a.cols, b.cols), 324))
                } else if kind == check::MULT && a.cols != b.rows {
                    Some((
                        format!("cols of m1 mismatch rows of m2 {}, {}", a.cols, b.rows),
                        329,
                    ))
                } else {
                    None
                }
            }
        };
        let Some((what, line)) = what else {
            return 0;
        };
        let mut b = b"ERROR: ".to_vec();
        b.extend_from_slice(prefix);
        b.extend_from_slice(
            format!(" {what}\nbreak {BUILD_ROOT}/utils/dmatrix.cpp:{line}\n").as_bytes(),
        );
        self.put_stream(stream, &b);
        1
    }

    /// `DMatrixAddMul(m1, m2, v1, v2, out)` (§4, §12): `v1 m1 + v2 m2` element by element. An
    /// output that is also an input: pass a copy of that input (the elements are independent).
    pub fn dmatrix_add_mul(
        &mut self,
        m1: &DMatrix,
        m2: Option<&DMatrix>,
        v1: f64,
        v2: f64,
        out: Option<&mut DMatrix>,
    ) -> Returned<DMatrix> {
        if m2.is_some()
            && self.dmatrix_check_dims(
                Some(m1),
                m2,
                check::SAME,
                Some(StreamSel::Out),
                b"DMatrixAddMul(): ",
            ) != 0
        {
            return Returned::Nothing;
        }
        let mut fresh;
        let (o, new) = match out {
            Some(o) => (o, false),
            None => {
                fresh = DMatrix::zeros(m1.rows, m1.cols);
                (&mut fresh, true)
            }
        };
        if self.dmatrix_check_dims(
            Some(m1),
            Some(o),
            check::SAME,
            Some(StreamSel::Out),
            b"DMatrixAddMul(): ",
        ) != 0
        {
            return Returned::Nothing;
        }
        let term = |val: f64, v: f64, x: f64| {
            if v == 1.0 {
                sse::add64(val, x)
            } else {
                sse::add64(sse::mul64(x, v), val)
            }
        };
        for i in 0..o.data.len() {
            let mut val = 0.0;
            if v1 != 0.0 {
                val = term(val, v1, m1.data[i]);
            }
            if let Some(m2) = m2
                && v2 != 0.0
            {
                val = term(val, v2, m2.data[i]);
            }
            o.data[i] = val;
        }
        if new {
            Returned::New(std::mem::replace(o, DMatrix::zeros(0, 0)))
        } else {
            Returned::Output
        }
    }

    /// `DMatrixAdd(m1, m2, out)`.
    pub fn dmatrix_add(
        &mut self,
        m1: &DMatrix,
        m2: &DMatrix,
        out: Option<&mut DMatrix>,
    ) -> Returned<DMatrix> {
        self.dmatrix_add_mul(m1, Some(m2), 1.0, 1.0, out)
    }

    /// `DMatrixSubtract(m1, m2, out)`.
    pub fn dmatrix_subtract(
        &mut self,
        m1: &DMatrix,
        m2: &DMatrix,
        out: Option<&mut DMatrix>,
    ) -> Returned<DMatrix> {
        self.dmatrix_add_mul(m1, Some(m2), 1.0, -1.0, out)
    }

    /// `DMatrixScalarMul(m, v, out)`.
    pub fn dmatrix_scalar_mul(
        &mut self,
        m: &DMatrix,
        v: f64,
        out: Option<&mut DMatrix>,
    ) -> Returned<DMatrix> {
        self.dmatrix_add_mul(m, None, v, 0.0, out)
    }

    /// `DMatrixCopy(src, out)` (§4).
    pub fn dmatrix_copy(
        &mut self,
        src: Option<&DMatrix>,
        out: Option<&mut DMatrix>,
    ) -> Returned<DMatrix> {
        let Some(src) = src else {
            self.put_out(b"ERROR: DMatrixCopy(): msrc is NULL\n");
            return Returned::Nothing;
        };
        match out {
            None => Returned::New(src.clone()),
            Some(o) => {
                if self.dmatrix_check_dims(
                    Some(src),
                    Some(o),
                    check::SAME,
                    Some(StreamSel::Out),
                    b"DMatrixCopy(): ",
                ) != 0
                {
                    return Returned::Nothing;
                }
                o.data.copy_from_slice(&src.data);
                Returned::Output
            }
        }
    }

    /// `DMatrixTranspose(in, out)` (§4); `Output::First` is the same matrix as both (refused).
    pub fn dmatrix_transpose(
        &mut self,
        m: &DMatrix,
        out: Output<'_, DMatrix>,
    ) -> Returned<DMatrix> {
        let mut fresh;
        let (o, new) = match out {
            Output::First | Output::Second => {
                self.put_out(b"DMatrixTranspose(): ERROR: cannot be done in-place\n");
                return Returned::Nothing;
            }
            Output::Given(o) => (o, false),
            Output::New => {
                fresh = DMatrix::zeros(m.cols, m.rows);
                (&mut fresh, true)
            }
        };
        if m.rows != o.cols {
            self.put_out(
                format!(
                    "DMatrixTranspose(): ERROR: row/col mismatch {} {}\n",
                    m.rows, o.cols
                )
                .as_bytes(),
            );
            return Returned::Nothing;
        }
        if m.cols != o.rows {
            self.put_out(
                format!(
                    "DMatrixTranspose(): ERROR: col/row mismatch {} {}\n",
                    m.cols, o.rows
                )
                .as_bytes(),
            );
            return Returned::Nothing;
        }
        for r in 1..=m.rows {
            for c in 1..=m.cols {
                let i = o.idx(c, r);
                o.data[i] = m.at(r, c);
            }
        }
        if new {
            Returned::New(std::mem::replace(o, DMatrix::zeros(0, 0)))
        } else {
            Returned::Output
        }
    }

    /// `DMatrixPrintFmt(stdout, fmt, m)` (§4); `DMatrixPrint` is the format `%8.5lf`. Returns 0,
    /// or -4 after an error for no matrix.
    pub fn dmatrix_print_fmt(&mut self, fmt: &[u8], m: Option<&DMatrix>) -> i32 {
        let Some(m) = m else {
            self.error_printf(codes::BAD_PARM, b"DMatrixPrint: mat = NULL!");
            return codes::BAD_PARM;
        };
        let mut b = Vec::new();
        for r in 1..=m.rows {
            for c in 1..=m.cols {
                b.extend_from_slice(&sprintf_f64(fmt, m.at(r, c)));
                if c < m.cols {
                    b.extend_from_slice(b"  ");
                }
            }
            b.push(b'\n');
        }
        self.put_out(&b);
        0
    }

    /// `DMatrixPrint(stdout, m)`.
    pub fn dmatrix_print(&mut self, m: Option<&DMatrix>) -> i32 {
        self.dmatrix_print_fmt(b"%8.5lf", m)
    }

    /// `DMatrixMultiply(m1, m2, m3)` (§4, §12): each element the x87 extended sum from 0 of the
    /// double products, rounded to double.
    pub fn dmatrix_multiply(
        &mut self,
        m1: &DMatrix,
        m2: &DMatrix,
        m3: Option<&mut DMatrix>,
    ) -> Returned<DMatrix> {
        if self.dmatrix_check_dims(
            Some(m1),
            Some(m2),
            check::MULT,
            Some(StreamSel::Out),
            b"DMatrixMultiply(): ",
        ) != 0
        {
            return Returned::Nothing;
        }
        let mut fresh;
        let (o, new) = match m3 {
            Some(o) => (o, false),
            None => {
                fresh = DMatrix::zeros(m1.rows, m2.cols);
                (&mut fresh, true)
            }
        };
        if o.rows != m1.rows || o.cols != m2.cols {
            self.put_out(
                format!("DMatrixMultiply(): m1/m2 dim mismatch\n break {BUILD_ROOT}/utils/dmatrix.cpp:429\n").as_bytes(),
            );
            let msg = format!(
                "DMatrixMultiply: ({} x {}) * ({} x {}) != ({} x {})\n",
                m1.rows, m1.cols, m2.rows, m2.cols, o.rows, o.cols
            );
            self.error_printf(codes::BAD_PARM, msg.as_bytes());
            return Returned::Nothing;
        }
        let (n, m) = (m1.cols.max(0) as usize, m2.cols.max(0) as usize);
        for r in 0..m1.rows.max(0) as usize {
            for c in 0..m {
                let mut s = F80::ZERO;
                for i in 0..n {
                    let p = sse::mul64(m1.data[r * n + i], m2.data[i * m + c]);
                    s += mem_operand(p);
                }
                o.data[r * m + c] = s.to_f64();
            }
        }
        if new {
            Returned::New(std::mem::replace(o, DMatrix::zeros(0, 0)))
        } else {
            Returned::Output
        }
    }

    /// `DVectorDot(v1, v2)` (§4, §12): the extended sum of the extended products of the first
    /// columns, rounded to double; `-10e10` when the sizes differ.
    pub fn dvector_dot(&mut self, v1: &DMatrix, v2: &DMatrix) -> f64 {
        if self.dmatrix_check_dims(
            Some(v1),
            Some(v2),
            check::SAME,
            Some(StreamSel::Out),
            b"DDVectorDot(): ",
        ) != 0
        {
            return -10e10;
        }
        let mut s = F80::ZERO;
        for r in 1..=v1.rows {
            // v2's element loaded, v1's the memory operand of the product
            let p = F80::from_f64(v2.at(r, 1)) * mem_operand(v1.at(r, 1));
            s += p;
        }
        s.to_f64()
    }

    /// `DMatrixCompareFMatrix(mf, md)` (§4): the largest `|(double) f - d|`, -1 after an error.
    pub fn dmatrix_compare_fmatrix(&mut self, mf: Option<&Matrix>, md: Option<&DMatrix>) -> f64 {
        let (f, d) = match (mf, md) {
            (None, _) => {
                self.put_out(b"ERROR: DMatrixCompareFMatrix(): mf is NULL\n");
                return -1.0;
            }
            (_, None) => {
                self.put_out(b"ERROR: DMatrixCompareFMatrix(): md is NULL\n");
                return -1.0;
            }
            (Some(f), Some(d)) => (f, d),
        };
        if f.rows != d.rows {
            self.put_out(
                format!(
                    "ERROR: DMatrixCompareFMatrix(): row mismatch {} {}\n",
                    f.rows, d.rows
                )
                .as_bytes(),
            );
            return -1.0;
        }
        if f.cols != d.cols {
            self.put_out(
                format!(
                    "ERROR: DMatrixCompareFMatrix(): col mismatch {} {}\n",
                    f.cols, d.cols
                )
                .as_bytes(),
            );
            return -1.0;
        }
        let mut mx = 0.0f64;
        for r in 1..=f.rows {
            for c in 1..=f.cols {
                let v = sse::sub64(sse::widen(f.at(r, c)), d.at(r, c)).abs();
                if v > mx {
                    mx = v;
                }
            }
        }
        mx
    }

    /// `DMatrixCopyFMatrix(mf, out)` (§4): the values widened.
    pub fn dmatrix_copy_fmatrix(
        &mut self,
        mf: Option<&Matrix>,
        out: Option<&mut DMatrix>,
    ) -> Returned<DMatrix> {
        let Some(f) = mf else {
            self.put_out(b"ERROR: DMatrixCopyFMatrix(): mf is NULL\n");
            return Returned::Nothing;
        };
        let mut fresh;
        let (o, new) = match out {
            Some(o) => (o, false),
            None => {
                fresh = DMatrix::zeros(f.rows, f.cols);
                (&mut fresh, true)
            }
        };
        if f.rows != o.rows {
            self.put_out(
                format!(
                    "ERROR: DMatrixCopyFMatrix(): row mismatch {} {}\n",
                    f.rows, o.rows
                )
                .as_bytes(),
            );
            return Returned::Nothing;
        }
        if f.cols != o.cols {
            self.put_out(
                format!(
                    "ERROR: DMatrixCopyFMatrix(): col mismatch {} {}\n",
                    f.cols, o.cols
                )
                .as_bytes(),
            );
            return Returned::Nothing;
        }
        for r in 1..=f.rows {
            for c in 1..=f.cols {
                let i = o.idx(r, c);
                o.data[i] = sse::widen(f.at(r, c));
            }
        }
        if new {
            Returned::New(std::mem::replace(o, DMatrix::zeros(0, 0)))
        } else {
            Returned::Output
        }
    }

    /// `DMatrixConstVal(v, rows, cols, out)` (§4): every element `v` (a new matrix when none).
    pub fn dmatrix_const_val(
        &mut self,
        v: f64,
        rows: i32,
        cols: i32,
        out: Option<&mut DMatrix>,
    ) -> Result<Returned<DMatrix>, Stop> {
        // 0 (either sign) is the zeroing
        let v = if v == 0.0 { 0.0 } else { v };
        match out {
            Some(o) => {
                o.data.iter_mut().for_each(|x| *x = v);
                Ok(Returned::Output)
            }
            None => {
                let Some(mut o) = self.dmatrix_alloc(rows, cols, super::matrix::MATRIX_REAL)?
                else {
                    return Ok(Returned::Nothing);
                };
                o.data.iter_mut().for_each(|x| *x = v);
                Ok(Returned::New(o))
            }
        }
    }

    /// `DMatrixZero(rows, cols, out)`: the constant 0.
    pub fn dmatrix_zero(
        &mut self,
        rows: i32,
        cols: i32,
        out: Option<&mut DMatrix>,
    ) -> Result<Returned<DMatrix>, Stop> {
        self.dmatrix_const_val(0.0, rows, cols, out)
    }

    /// `DMatrixMaxAbs(m)` (§4): the largest magnitude from 0; -1 after an error for none.
    pub fn dmatrix_max_abs(&mut self, m: Option<&DMatrix>) -> f64 {
        let Some(m) = m else {
            self.put_out(b"ERROR: DMatrixMaxAbs(): matrix is NULL\n");
            return -1.0;
        };
        let mut mx = 0.0f64;
        for &x in &m.data {
            if x.abs() > mx {
                mx = x.abs();
            }
        }
        mx
    }
}
