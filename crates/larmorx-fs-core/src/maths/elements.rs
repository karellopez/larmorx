// SPDX-License-Identifier: Apache-2.0
//! Float matrices, part 2 (spec §8.2 to §8.7): the element-wise operations, regions, rotations,
//! vectors, printing, the similarity transform and the Gaussian matrices. FS-25 (a complex
//! operand of a real-only operation) and FS-28 (an output that does not fit) are the switches
//! of §7.

use larmorx_core::defects::{FS_25, FS_26, FS_27, FS_28, FS_31};

use super::matrix::{MATRIX_COMPLEX, MATRIX_REAL, Matrix};
use super::{Output, Returned};
use crate::clib::printf::sprintf_f64;
use crate::process::{Process, Stop, codes};
use crate::sse;

impl Process {
    /// FS-25: whether a real-only operation goes on with `m` (a complex matrix: refused with its
    /// error by default, addressed as real with `reproduce`).
    pub(crate) fn real_only(&mut self, name: &str, m: &Matrix) -> bool {
        if !m.is_complex() {
            return true;
        }
        if self.mode().reproduce() {
            self.note(&FS_25, format!("{name}: a complex matrix addressed as a real one"));
            true
        } else {
            self.note(&FS_25, format!("{name}: a complex matrix refused"));
            self.error_printf(
                codes::BAD_PARM,
                format!("{name}: a complex matrix (the function handles real matrices only)\n").as_bytes(),
            );
            false
        }
    }

    /// FS-28: whether an output that does not fit is filled (`reproduce`) or refused with its
    /// error (default). `fits` true: nothing to decide.
    pub(crate) fn fits(&mut self, name: &str, what: &str, fits: bool) -> bool {
        if fits {
            return true;
        }
        if self.mode().reproduce() {
            self.note(&FS_28, format!("{name}: {what}: filled at FreeSurfer's places"));
            true
        } else {
            self.note(&FS_28, format!("{name}: {what}: refused"));
            self.error_printf(codes::BAD_PARM, format!("{name}: {what}\n").as_bytes());
            false
        }
    }

    /// A new matrix of zeros (the allocator's errors as FreeSurfer's).
    fn new_matrix(&mut self, rows: i32, cols: i32, kind: i32) -> Result<Option<Matrix>, Stop> {
        self.matrix_alloc(rows, cols, kind)
    }

    /// Clears a matrix as `MatrixClear` does (FS-25 for a complex one).
    fn clear_matrix(&mut self, m: &mut Matrix) {
        if m.is_complex() && self.mode().fix() {
            for i in 1..m.data.len() {
                m.data[i] = 0.0;
            }
            return;
        }
        for r in 1..=m.rows {
            for c in 0..=m.cols {
                m.set(r, c, 0.0);
            }
        }
    }

    // ---- §8.2

    /// `MatrixIdentity(n, m)`.
    pub fn matrix_identity(&mut self, n: i32, m: Option<&mut Matrix>) -> Result<Returned<Matrix>, Stop> {
        match m {
            None => {
                let Some(mut o) = self.new_matrix(n, n, MATRIX_REAL)? else {
                    return Ok(Returned::Nothing);
                };
                for i in 1..=n {
                    o.set(i, i, 1.0);
                }
                Ok(Returned::New(o))
            }
            Some(o) => {
                if !self.real_only("MatrixIdentity", o) {
                    return Ok(Returned::Nothing);
                }
                if !self.fits("MatrixIdentity", "the given matrix is smaller than the identity", o.rows >= n && o.cols >= n) {
                    return Ok(Returned::Nothing);
                }
                self.clear_matrix(o);
                for i in 1..=n {
                    o.set(i, i, 1.0);
                }
                Ok(Returned::Output)
            }
        }
    }

    /// `MatrixTranspose(in, out)`.
    pub fn matrix_transpose(&mut self, m: &Matrix, out: Option<&mut Matrix>) -> Result<Returned<Matrix>, Stop> {
        if !self.real_only("MatrixTranspose", m) {
            return Ok(Returned::Nothing);
        }
        let src = m.clone();
        let mut fresh = None;
        let o: &mut Matrix = match out {
            None => match self.new_matrix(m.cols, m.rows, m.kind)? {
                Some(x) => fresh.insert(x),
                None => return Ok(Returned::Nothing),
            },
            Some(o) => {
                let ok = o.rows == m.cols && o.cols == m.rows;
                if !self.fits("MatrixTranspose", "the output is not the transpose's size", ok) {
                    return Ok(Returned::Nothing);
                }
                o
            }
        };
        for r in 1..=src.rows {
            for c in 1..=src.cols {
                o.set(c, r, src.at(r, c));
            }
        }
        Ok(match fresh {
            Some(f) => Returned::New(f),
            None => Returned::Output,
        })
    }

    /// An element-wise operation of one or two matrices into a new or given output of the
    /// first's size (§8.2). `check` gives the message of a given output of another size.
    fn elementwise(
        &mut self,
        name: &str,
        a: &Matrix,
        out: Option<&mut Matrix>,
        bad_out: impl Fn(&Matrix) -> String,
        f: impl Fn(i32, i32, f32) -> f32,
    ) -> Result<Returned<Matrix>, Stop> {
        let src = a.clone();
        let mut fresh = None;
        let o: &mut Matrix = match out {
            None => match self.new_matrix(a.rows, a.cols, a.kind)? {
                Some(x) => fresh.insert(x),
                None => return Ok(Returned::Nothing),
            },
            Some(o) => {
                if !o.same_size(a) {
                    let msg = bad_out(o);
                    self.error_printf(codes::BAD_PARM, msg.as_bytes());
                    return Ok(Returned::Nothing);
                }
                o
            }
        };
        let _ = name;
        for r in 1..=src.rows {
            for c in 1..=src.cols {
                o.set(r, c, f(r, c, src.at(r, c)));
            }
        }
        Ok(match fresh {
            Some(f) => Returned::New(f),
            None => Returned::Output,
        })
    }

    /// `MatrixAdd(m1, m2, out)` (`subtract`: `MatrixSubtract`).
    pub fn matrix_add_sub(&mut self, m1: &Matrix, m2: &Matrix, out: Option<&mut Matrix>, subtract: bool) -> Result<Returned<Matrix>, Stop> {
        let (name, op) = if subtract { ("MatrixSubtract", "-") } else { ("MatrixAdd", "+") };
        if !self.real_only(name, m1) || !self.real_only(name, m2) {
            return Ok(Returned::Nothing);
        }
        if !m1.same_size(m2) {
            let msg = format!(
                "{name}: incompatable matrices {} x {} {op} {} x {}\n",
                m1.rows, m1.cols, m2.rows, m2.cols
            );
            self.error_printf(codes::BAD_PARM, msg.as_bytes());
            return Ok(Returned::Nothing);
        }
        let b = m2.clone();
        let (r1, c1) = (m1.rows, m1.cols);
        self.elementwise(
            name,
            m1,
            out,
            |o| format!("{name}: incompatable matrices {r1} x {c1} = {} x {}\n", o.rows, o.cols),
            |r, c, x| {
                if subtract { sse::sub32(x, b.at(r, c)) } else { sse::add32(x, b.at(r, c)) }
            },
        )
    }

    /// `MatrixScalarMul(in, v, out)`.
    pub fn matrix_scalar_mul(&mut self, m: &Matrix, v: f32, out: Option<&mut Matrix>) -> Result<Returned<Matrix>, Stop> {
        if !self.real_only("MatrixScalarMul", m) {
            return Ok(Returned::Nothing);
        }
        let (r1, c1) = (m.rows, m.cols);
        self.elementwise(
            "MatrixScalarMul",
            m,
            out,
            |o| format!("MatrixScalarMul: incompatable matrices {r1} x {c1} != {} x {}\n", o.rows, o.cols),
            |_, _, x| sse::mul32(x, v),
        )
    }

    /// `MatrixSquareElts(in, out)`.
    pub fn matrix_square_elts(&mut self, m: &Matrix, out: Option<&mut Matrix>) -> Result<Returned<Matrix>, Stop> {
        if !self.real_only("MatrixSquareElts", m) {
            return Ok(Returned::Nothing);
        }
        let (r1, c1) = (m.rows, m.cols);
        self.elementwise(
            "MatrixSquareElts",
            m,
            out,
            |o| format!("MatrixSquareElts: incompatable matrices {r1} x {c1} != {} x {}\n", o.rows, o.cols),
            |_, _, x| sse::mul32(x, x),
        )
    }

    /// `MatrixClear(m)`: every element 0 (FS-25 for a complex matrix).
    pub fn matrix_clear(&mut self, m: &mut Matrix) -> Returned<Matrix> {
        if m.is_complex() {
            if self.mode().reproduce() {
                self.note(&FS_25, "MatrixClear: a complex matrix cleared as FreeSurfer clears it (part of it)");
            } else {
                self.note(&FS_25, "MatrixClear: every float of a complex matrix cleared");
            }
        }
        self.clear_matrix(m);
        Returned::Output
    }

    /// `MatrixDiag(d, out)`.
    pub fn matrix_diag(&mut self, d: &Matrix, out: Option<&mut Matrix>) -> Result<Returned<Matrix>, Stop> {
        if !self.real_only("MatrixDiag", d) {
            return Ok(Returned::Nothing);
        }
        let n = d.rows.max(d.cols);
        let src = d.clone();
        let mut fresh = None;
        let o: &mut Matrix = match out {
            None => match self.new_matrix(n, n, d.kind)? {
                Some(x) => fresh.insert(x),
                None => return Ok(Returned::Nothing),
            },
            Some(o) => {
                self.clear_matrix(o);
                if o.rows != n || o.cols != n {
                    let msg = format!("MatrixDiag: incompatable matrices {n} x {n} != {} x {}\n", o.rows, o.cols);
                    self.error_printf(codes::BAD_PARM, msg.as_bytes());
                    return Ok(Returned::Nothing);
                }
                o
            }
        };
        if src.rows > 1 {
            for r in 1..=src.rows {
                o.set(r, r, src.at(r, 1));
            }
        } else {
            for c in 1..=src.cols {
                o.set(c, c, src.at(1, c));
            }
        }
        Ok(match fresh {
            Some(f) => Returned::New(f),
            None => Returned::Output,
        })
    }

    /// `MatrixIsSymmetric(m)`.
    pub fn matrix_is_symmetric(&mut self, m: &Matrix) -> i32 {
        if !self.real_only("MatrixIsSymmetric", m) {
            return 0;
        }
        if m.rows != m.cols {
            return 0;
        }
        for r in 1..=m.rows {
            for c in 1..=m.cols {
                if m.at(r, c) != m.at(c, r) {
                    return 0;
                }
            }
        }
        1
    }

    /// `MatrixIsZero(m)`.
    pub fn matrix_is_zero(&mut self, m: &Matrix) -> i32 {
        if !self.real_only("MatrixIsZero", m) {
            return 0;
        }
        for r in 1..=m.rows {
            for c in 1..=m.cols {
                if f64::from(m.at(r, c)).abs() > 1e-11 {
                    return 0;
                }
            }
        }
        1
    }

    /// `MatrixCheck(m)`: -4 at the first element that is not finite, else 0.
    pub fn matrix_check(&mut self, m: &Matrix) -> i32 {
        if !self.real_only("MatrixCheck", m) {
            return codes::BAD_PARM;
        }
        for r in 1..=m.rows {
            for c in 1..=m.cols {
                if !m.at(r, c).is_finite() {
                    return codes::BAD_PARM;
                }
            }
        }
        0
    }

    /// `MatrixTrace(m)`.
    pub fn matrix_trace(&mut self, m: &Matrix) -> f32 {
        if !self.real_only("MatrixTrace", m) {
            return 0.0;
        }
        let mut t = 0.0f32;
        for i in 1..=m.rows.min(m.cols) {
            t = sse::add32(t, m.at(i, i));
        }
        t
    }

    /// `MatrixSumElts(m)`.
    pub fn matrix_sum_elts(&mut self, m: &Matrix) -> f64 {
        if !self.real_only("MatrixSumElts", m) {
            return 0.0;
        }
        let mut s = 0.0f64;
        for r in 1..=m.rows {
            for c in 1..=m.cols {
                s = sse::add64(s, sse::widen(m.at(r, c)));
            }
        }
        s
    }

    /// `MatrixConstVal(v, rows, cols, X)`.
    pub fn matrix_const_val(&mut self, v: f32, rows: i32, cols: i32, x: Option<&mut Matrix>) -> Result<Returned<Matrix>, Stop> {
        let mut fresh = None;
        let o: &mut Matrix = match x {
            None => match self.new_matrix(rows, cols, MATRIX_REAL)? {
                Some(m) => fresh.insert(m),
                None => return Ok(Returned::Nothing),
            },
            Some(o) => {
                if !self.real_only("MatrixConstVal", o) {
                    return Ok(Returned::Nothing);
                }
                o
            }
        };
        for r in 1..=o.rows {
            for c in 1..=o.cols {
                o.set(r, c, v);
            }
        }
        Ok(match fresh {
            Some(f) => Returned::New(f),
            None => Returned::Output,
        })
    }

    /// `MatrixZero(rows, cols, X)`.
    pub fn matrix_zero(&mut self, rows: i32, cols: i32, x: Option<&mut Matrix>) -> Result<Returned<Matrix>, Stop> {
        self.matrix_const_val(0.0, rows, cols, x)
    }

    /// `MatrixClone(m)`: a matrix of `m`'s size and type, all zeros (FS-14, FS-25 for a complex
    /// one: FreeSurfer's floats with `reproduce`).
    pub fn matrix_clone(&mut self, m: &Matrix) -> Result<Option<Matrix>, Stop> {
        let Returned::New(mut c) = self.matrix_copy(Some(m), Output::New)? else {
            return Ok(None);
        };
        if c.is_complex() {
            if self.mode().reproduce() {
                self.note(&FS_25, "MatrixClone: a complex matrix zeroed as a real one");
                for r in 1..=c.rows {
                    for k in 1..=c.cols {
                        c.set(r, k, 0.0);
                    }
                }
            } else {
                self.note(&FS_25, "MatrixClone: every float of a complex clone zeroed");
                for i in 2..c.data.len() {
                    c.data[i] = 0.0;
                }
            }
        } else {
            for r in 1..=c.rows {
                for k in 1..=c.cols {
                    c.set(r, k, 0.0);
                }
            }
        }
        Ok(Some(c))
    }

    /// `MatrixReshape(src, dst, rows, cols)`.
    pub fn matrix_reshape(&mut self, src: &Matrix, dst: Option<&mut Matrix>, rows: i32, cols: i32) -> Result<Returned<Matrix>, Stop> {
        if !self.real_only("MatrixReshape", src) {
            return Ok(Returned::Nothing);
        }
        let s = src.clone();
        let mut fresh = None;
        let o: &mut Matrix = match dst {
            None => match self.new_matrix(rows, cols, src.kind)? {
                Some(m) => fresh.insert(m),
                None => return Ok(Returned::Nothing),
            },
            Some(o) => o,
        };
        let (rows, cols) = (o.rows, o.cols);
        let total = i64::from(rows) * i64::from(cols);
        let mut k: i64 = 0;
        'outer: for r in 1..=s.rows {
            for c in 1..=s.cols {
                if k >= total {
                    break 'outer;
                }
                let (dr, dc) = ((k / i64::from(cols)) as i32 + 1, (k % i64::from(cols)) as i32 + 1);
                o.set(dr, dc, s.at(r, c));
                k += 1;
            }
        }
        Ok(match fresh {
            Some(f) => Returned::New(f),
            None => Returned::Output,
        })
    }

    /// The column lengths of `m` (`(float) sqrt` of the double sums of squares).
    fn col_lengths(m: &Matrix) -> Vec<f32> {
        (1..=m.cols)
            .map(|c| {
                let mut s = 0.0f64;
                for r in 1..=m.rows {
                    let x = sse::widen(m.at(r, c));
                    s = sse::add64(s, sse::mul64(x, x));
                }
                sse::narrow(sse::sqrt64(s))
            })
            .collect()
    }

    /// `MatrixNormalizeColScale(m, scale)`.
    pub fn matrix_normalize_col_scale(&mut self, m: &Matrix, scale: Option<&mut Matrix>) -> Result<Returned<Matrix>, Stop> {
        if !self.real_only("MatrixNormalizeColScale", m) {
            return Ok(Returned::Nothing);
        }
        let len = Process::col_lengths(m);
        let mut fresh = None;
        let o: &mut Matrix = match scale {
            None => match self.new_matrix(1, m.cols, MATRIX_REAL)? {
                Some(x) => fresh.insert(x),
                None => return Ok(Returned::Nothing),
            },
            Some(o) => {
                let ok = o.rows >= 1 && o.cols >= m.cols;
                if !self.fits("MatrixNormalizeColScale", "the scale row is smaller than the matrix's columns", ok) {
                    return Ok(Returned::Nothing);
                }
                o
            }
        };
        for (c, &l) in len.iter().enumerate() {
            o.set(1, c as i32 + 1, l);
        }
        Ok(match fresh {
            Some(f) => Returned::New(f),
            None => Returned::Output,
        })
    }

    /// `MatrixNormalizeCol(m, out, scale)`.
    pub fn matrix_normalize_col(
        &mut self,
        m: &Matrix,
        out: Option<&mut Matrix>,
        scale: Option<&mut Matrix>,
    ) -> Result<Returned<Matrix>, Stop> {
        if !self.real_only("MatrixNormalizeCol", m) {
            return Ok(Returned::Nothing);
        }
        let src = m.clone();
        let mut fresh = None;
        let o: &mut Matrix = match out {
            None => match self.new_matrix(m.rows, m.cols, MATRIX_REAL)? {
                Some(x) => fresh.insert(x),
                None => return Ok(Returned::Nothing),
            },
            Some(o) => {
                if !o.same_size(m) {
                    self.put_out(b"ERROR: MatrixNormalizeCol: dimension mismatch\n");
                    return Ok(Returned::Nothing);
                }
                o
            }
        };
        let len = Process::col_lengths(&src);
        if let Some(s) = scale {
            let ok = s.rows >= 1 && s.cols >= m.cols;
            if self.fits("MatrixNormalizeCol", "the scale row is smaller than the matrix's columns", ok) {
                for (c, &l) in len.iter().enumerate() {
                    s.set(1, c as i32 + 1, l);
                }
            }
        }
        for r in 1..=src.rows {
            for c in 1..=src.cols {
                let l = len[c as usize - 1];
                let v = if l == 0.0 {
                    0.0
                } else {
                    sse::narrow(sse::div64(sse::widen(src.at(r, c)), sse::widen(l)))
                };
                o.set(r, c, v);
            }
        }
        Ok(match fresh {
            Some(f) => Returned::New(f),
            None => Returned::Output,
        })
    }
}

/// Which region copy (§8.3).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Region {
    /// `MatrixCopyRegion`: whole elements.
    Whole,
    /// `MatrixCopyRealRegion`: the real parts.
    Real,
    /// `MatrixCopyImagRegion`: the imaginary parts.
    Imag,
}

impl Region {
    fn name(self) -> &'static str {
        match self {
            Region::Whole => "MatrixCopyRegion",
            Region::Real => "MatrixCopyRealRegion",
            Region::Imag => "MatrixCopyImagRegion",
        }
    }
}

impl Process {
    /// The region copies (§8.3; FS-26, FS-27, FS-28).
    #[allow(clippy::too_many_arguments)]
    pub fn matrix_copy_region(
        &mut self,
        which: Region,
        src: &Matrix,
        dst: Option<&mut Matrix>,
        sr: i32,
        sc: i32,
        rows: i32,
        cols: i32,
        dr: i32,
        dc: i32,
    ) -> Result<Returned<Matrix>, Stop> {
        let name = which.name();
        if dr < 1 || dc < 1 {
            self.error_printf(codes::BAD_PARM, format!("{name}: bad destination ({dr},{dc})\n").as_bytes());
            return Ok(Returned::Nothing);
        }
        let (er, ec) = (sr + rows - 1, sc + cols - 1);
        if sr < 1 || sr > src.rows || sc < 1 || sc > src.cols || er > src.rows || ec > src.cols {
            let msg = format!("{name}: bad source region ({sr},{sc}) --> ({er},{ec})\n");
            self.error_printf(codes::BAD_PARM, msg.as_bytes());
            return Ok(Returned::Nothing);
        }
        let s = src.clone();
        let mut fresh = None;
        let given = dst.is_some();
        let o: &mut Matrix = match dst {
            None => match self.new_matrix(rows, cols, src.kind)? {
                Some(x) => fresh.insert(x),
                None => return Ok(Returned::Nothing),
            },
            Some(o) => o,
        };
        if o.rows < rows || o.cols < cols {
            let msg = format!("{name}: destination matrix not large enough ({rows}x{cols})\n");
            self.error_printf(codes::BAD_PARM, msg.as_bytes());
            return Ok(Returned::Nothing);
        }
        let fit = dr + rows - 1 <= o.rows && dc + cols - 1 <= o.cols;
        let what = if given {
            "the destination region extends beyond the destination matrix"
        } else {
            "the destination region extends beyond the new matrix"
        };
        if !self.fits(name, what, fit) {
            return Ok(Returned::Nothing);
        }
        let reproduce = self.mode().reproduce();
        if which == Region::Whole && s.is_complex() != o.is_complex() {
            if reproduce {
                self.note(&FS_27, "a region between a real and a complex matrix copied at FreeSurfer's places");
            } else {
                self.note(&FS_27, "a region between a real and a complex matrix refused");
                self.error_printf(
                    codes::BAD_PARM,
                    b"MatrixCopyRegion: a region between a real and a complex matrix\n",
                );
                return Ok(Returned::Nothing);
            }
        }
        if which != Region::Whole && !s.is_complex() && rows > 0 && cols > 0 {
            if reproduce {
                self.note(&FS_27, "a real source's parts read as a complex matrix's (beyond the array: 0)");
            } else {
                self.note(&FS_27, "a real source's real part its element, its imaginary part 0");
            }
        }
        if which == Region::Real && o.is_complex() && rows > 0 && cols > 0 {
            if reproduce {
                self.note(&FS_26, "the real-part copy into a complex destination copied the imaginary parts");
            } else {
                self.note(&FS_26, "the real-part copy into a complex destination copied the real parts");
            }
        }
        // the parts of the source at (r, c), as FreeSurfer reads them (a complex matrix's)
        let parts = |r: i32, c: i32| -> (f32, f32) {
            if !s.is_complex() && !reproduce {
                (s.at(r, c), 0.0)
            } else {
                let p = s.complex_place(r, c);
                (s.read(p), s.read(p.map(|i| i + 1)))
            }
        };
        for i in 0..rows {
            for j in 0..cols {
                let (r, c) = (sr + i, sc + j);
                let (tr, tc) = (dr + i, dc + j);
                if !o.is_complex() {
                    let v = match which {
                        Region::Whole => {
                            if s.is_complex() {
                                // FreeSurfer's real-addressed read of the complex source
                                s.at(r, c)
                            } else {
                                s.at(r, c)
                            }
                        }
                        Region::Real => parts(r, c).0,
                        Region::Imag => parts(r, c).1,
                    };
                    o.set(tr, tc, v);
                } else {
                    match which {
                        Region::Whole => {
                            let (re, im) = if s.is_complex() {
                                s.pair(r, c)
                            } else {
                                // a real source addressed as complex
                                let p = s.complex_place(r, c);
                                (s.read(p), s.read(p.map(|k| k + 1)))
                            };
                            o.set_pair(tr, tc, re, im);
                        }
                        Region::Real => {
                            let (re, im) = parts(r, c);
                            let p = o.complex_place(tr, tc);
                            if reproduce {
                                o.write(p.map(|k| k + 1), im);
                            } else {
                                o.write(p, re);
                            }
                        }
                        Region::Imag => {
                            let (_, im) = parts(r, c);
                            let p = o.complex_place(tr, tc);
                            o.write(p.map(|k| k + 1), im);
                        }
                    }
                }
            }
        }
        Ok(match fresh {
            Some(f) => Returned::New(f),
            None => Returned::Output,
        })
    }

    /// `MatrixRealToComplex(re, im, out)` (FS-25, FS-28).
    pub fn matrix_real_to_complex(&mut self, re: &Matrix, im: &Matrix, out: Option<&mut Matrix>) -> Result<Returned<Matrix>, Stop> {
        if !self.real_only("MatrixRealToComplex", re) || !self.real_only("MatrixRealToComplex", im) {
            return Ok(Returned::Nothing);
        }
        if !self.fits("MatrixRealToComplex", "the imaginary parts' matrix is smaller than the real parts'", im.rows >= re.rows && im.cols >= re.cols) {
            return Ok(Returned::Nothing);
        }
        let (a, b) = (re.clone(), im.clone());
        let mut fresh = None;
        let o: &mut Matrix = match out {
            None => match self.new_matrix(re.rows, re.cols, MATRIX_COMPLEX)? {
                Some(x) => fresh.insert(x),
                None => return Ok(Returned::Nothing),
            },
            Some(o) => {
                let ok = o.is_complex() && o.rows >= re.rows && o.cols >= re.cols;
                if !self.fits("MatrixRealToComplex", "the output is real or smaller than the real parts' matrix", ok) {
                    return Ok(Returned::Nothing);
                }
                o
            }
        };
        for r in 1..=a.rows {
            for c in 1..=a.cols {
                let p = o.complex_place(r, c);
                o.write(p, a.at(r, c));
                o.write(p.map(|k| k + 1), b.at(r, c));
            }
        }
        Ok(match fresh {
            Some(f) => Returned::New(f),
            None => Returned::Output,
        })
    }

    // ---- §8.4, §8.5

    /// `MatrixAllocRotation(n, angle, which)` (`m` absent) and `MatrixReallocRotation` (given).
    pub fn matrix_rotation(&mut self, n: i32, angle: f32, which: i32, m: Option<&mut Matrix>) -> Result<Returned<Matrix>, Stop> {
        let mut fresh = None;
        let o: &mut Matrix = match m {
            None => match self.matrix_identity(n, None)? {
                Returned::New(x) => fresh.insert(x),
                _ => return Ok(Returned::Nothing),
            },
            Some(o) => o,
        };
        let c = self.libmf("cosf", angle, larmorx_core::math::cosf, larmorx_core::math::cosf_glibc);
        let s = self.libmf("sinf", angle, larmorx_core::math::sinf, larmorx_core::math::sinf_glibc);
        let ns = -s;
        match which {
            0 => {
                o.set(2, 2, c);
                o.set(2, 3, s);
                o.set(3, 2, ns);
                o.set(3, 3, c);
            }
            1 => {
                o.set(1, 1, c);
                o.set(1, 3, ns);
                o.set(3, 1, s);
                o.set(3, 3, c);
            }
            2 => {
                o.set(1, 1, c);
                o.set(1, 2, s);
                o.set(2, 1, ns);
                o.set(2, 2, c);
            }
            _ => {}
        }
        Ok(match fresh {
            Some(f) => Returned::New(f),
            None => Returned::Output,
        })
    }

    /// FS-31's check of a vector argument of the 3-vector functions: at least 3 rows and a
    /// column (`what` names the vector).
    fn vector3_ok(&mut self, name: &str, v: &Matrix, what: &str) -> bool {
        let ok = v.rows >= 3 && v.cols >= 1;
        if ok {
            return true;
        }
        if self.mode().reproduce() {
            self.note(&FS_31, format!("{name}: {what} read past its elements (beyond the array: 0)"));
            true
        } else {
            self.note(&FS_31, format!("{name}: {what} refused"));
            self.error_printf(codes::BAD_PARM, format!("{name}: {what} is not a 3-vector\n").as_bytes());
            false
        }
    }

    /// `VectorLen(v)`.
    pub fn vector_len(&mut self, v: &Matrix) -> f32 {
        if !self.real_only("VectorLen", v) {
            return 0.0;
        }
        let mut s = 0.0f32;
        for i in 1..=v.rows {
            let x = v.at(i, 1);
            s = sse::add32(s, sse::mul32(x, x));
        }
        sse_sqrt32(s)
    }

    /// `VectorDot(v1, v2)` (`distance`: `VectorDistance`, the squared distance).
    pub fn vector_dot_dist(&mut self, v1: &Matrix, v2: &Matrix, distance: bool) -> f32 {
        let name = if distance { "VectorDistance" } else { "VectorDot" };
        if !self.real_only(name, v1) || !self.real_only(name, v2) {
            return 0.0;
        }
        if v2.rows < v1.rows {
            if self.mode().reproduce() {
                self.note(&FS_31, format!("{name}: the second vector read to the first's length (beyond the array: 0)"));
            } else {
                self.note(&FS_31, format!("{name}: a second vector shorter than the first refused"));
                self.error_printf(codes::BAD_PARM, format!("{name}: the second vector is shorter than the first\n").as_bytes());
                return 0.0;
            }
        }
        let mut s = 0.0f32;
        for i in 1..=v1.rows {
            let (a, b) = (v1.at(i, 1), v2.at(i, 1));
            let t = if distance {
                let d = sse::sub32(a, b);
                sse::mul32(d, d)
            } else {
                sse::mul32(a, b)
            };
            s = sse::add32(s, t);
        }
        s
    }

    /// The first three elements of a vector.
    fn xyz(v: &Matrix) -> (f32, f32, f32) {
        (v.at(1, 1), v.at(2, 1), v.at(3, 1))
    }

    /// `VectorCrossProduct(v1, v2, out)`.
    pub fn vector_cross_product(&mut self, v1: &Matrix, v2: &Matrix, out: Option<&mut Matrix>) -> Result<Returned<Matrix>, Stop> {
        if v1.rows != 3 && v1.cols != 1 {
            self.error_printf(codes::BAD_PARM, b"VectorCrossProduct: must be 3-vectors");
            return Ok(Returned::Nothing);
        }
        if !self.real_only("VectorCrossProduct", v1) || !self.real_only("VectorCrossProduct", v2) {
            return Ok(Returned::Nothing);
        }
        if !self.vector3_ok("VectorCrossProduct", v1, "the first vector")
            || !self.vector3_ok("VectorCrossProduct", v2, "the second vector")
        {
            return Ok(Returned::Nothing);
        }
        let (x1, y1, z1) = Process::xyz(v1);
        let (x2, y2, z2) = Process::xyz(v2);
        let mut fresh = None;
        let o: &mut Matrix = match out {
            None => match self.matrix_clone(v1)? {
                Some(x) => fresh.insert(x),
                None => return Ok(Returned::Nothing),
            },
            Some(o) => {
                if !self.vector3_ok("VectorCrossProduct", o, "the output") {
                    return Ok(Returned::Nothing);
                }
                o
            }
        };
        o.set(1, 1, sse::sub32(sse::mul32(y1, z2), sse::mul32(z1, y2)));
        o.set(2, 1, sse::sub32(sse::mul32(z1, x2), sse::mul32(x1, z2)));
        o.set(3, 1, sse::sub32(sse::mul32(x1, y2), sse::mul32(y1, x2)));
        Ok(match fresh {
            Some(f) => Returned::New(f),
            None => Returned::Output,
        })
    }

    /// `VectorTripleProduct(v1, v2, v3)`.
    pub fn vector_triple_product(&mut self, v1: &Matrix, v2: &Matrix, v3: &Matrix) -> f32 {
        if v1.rows != 3 && v1.cols != 1 {
            self.error_printf(codes::BAD_PARM, b"VectorCrossProduct: must be 3-vectors");
            return 0.0;
        }
        for v in [v1, v2, v3] {
            if !self.real_only("VectorTripleProduct", v) {
                return 0.0;
            }
        }
        for (v, w) in [(v1, "the first vector"), (v2, "the second vector"), (v3, "the third vector")] {
            if !self.vector3_ok("VectorTripleProduct", v, w) {
                return 0.0;
            }
        }
        let (x1, y1, z1) = Process::xyz(v1);
        let (x2, y2, z2) = Process::xyz(v2);
        let (x3, y3, z3) = Process::xyz(v3);
        // the official build's operand order (which NaN a result keeps), fitted on the probe's
        // `mvectors` group (several orders fit every case; this is one of them)
        let a = sse::sub32(sse::mul32(z2, y1), sse::mul32(y2, z1));
        let b = sse::sub32(sse::mul32(x2, z1), sse::mul32(z2, x1));
        let c = sse::sub32(sse::mul32(x1, y2), sse::mul32(x2, y1));
        let t = sse::add32(sse::mul32(b, y3), sse::mul32(a, x3));
        sse::add32(sse::mul32(c, z3), t)
    }

    /// `Vector3Angle(v1, v2)` (acos correctly rounded in both modes, §7).
    pub fn vector3_angle(&mut self, v1: &Matrix, v2: &Matrix) -> f64 {
        for v in [v1, v2] {
            if !self.real_only("Vector3Angle", v) {
                return 0.0;
            }
        }
        if !self.vector3_ok("Vector3Angle", v1, "the first vector") || !self.vector3_ok("Vector3Angle", v2, "the second vector") {
            return 0.0;
        }
        let len = |v: &Matrix| {
            let (x, y, z) = Process::xyz(v);
            let (x, y, z) = (sse::widen(x), sse::widen(y), sse::widen(z));
            sse::sqrt64(sse::add64(sse::add64(sse::mul64(x, x), sse::mul64(y, y)), sse::mul64(z, z)))
        };
        let mut norm = sse::mul64(len(v1), len(v2));
        if norm.abs() < f64::from(f32::EPSILON) {
            return 0.0;
        }
        let (x1, y1, z1) = Process::xyz(v1);
        let (x2, y2, z2) = Process::xyz(v2);
        let dot = sse::add32(sse::add32(sse::mul32(x1, x2), sse::mul32(y1, y2)), sse::mul32(z1, z2));
        let dot = sse::widen(dot);
        if dot.abs() > norm {
            norm = dot.abs();
        }
        if dot > norm {
            larmorx_core::math::acos(1.0)
        } else {
            larmorx_core::math::acos(sse::div64(dot, norm))
        }
    }
}

/// `sqrtf` (`sqrtss`: a negative number gives the default NaN, a NaN passes quieted).
fn sse_sqrt32(x: f32) -> f32 {
    if x.is_nan() {
        f32::from_bits(x.to_bits() | 0x0040_0000)
    } else if x < 0.0 {
        f32::from_bits(0xffc0_0000)
    } else {
        x.sqrt()
    }
}

/// `XYZ_NORMALIZED_LOAD(v, len, x, y, z)` (§8.5): the unit vector and the length.
pub fn xyz_normalized_load(x: f32, y: f32, z: f32) -> ([f32; 3], f32) {
    let (a, b, c) = (sse::widen(x), sse::widen(y), sse::widen(z));
    let len = sse::narrow(sse::sqrt64(sse::add64(sse::add64(sse::mul64(a, a), sse::mul64(b, b)), sse::mul64(c, c))));
    if len == 0.0 {
        return ([1.0, 0.0, 0.0], len);
    }
    let inv = sse::div32(1.0, len);
    ([sse::mul32(x, inv), sse::mul32(y, inv), sse::mul32(z, inv)], len)
}

/// `XYZApproxAngle_knownLength(n, x2, y2, z2, norm)` (§8.5).
pub fn xyz_approx_angle_known_length(n: [f32; 3], x2: f32, y2: f32, z2: f32, norm: f32) -> f32 {
    let (a, b, c) = (sse::widen(n[0]), sse::widen(n[1]), sse::widen(n[2]));
    let dot = sse::narrow(sse::add64(
        sse::add64(sse::mul64(a, sse::widen(x2)), sse::mul64(b, sse::widen(y2))),
        sse::mul64(c, sse::widen(z2)),
    ));
    let mut norm = norm;
    if dot.abs() > norm {
        norm = dot.abs();
    }
    let q = sse::widen(sse::div32(dot, norm));
    if q < f64::from(0.99f32) {
        sse::narrow(larmorx_core::math::acos(q))
    } else {
        sse::narrow(sse::sqrt64(sse::mul64(2.0, sse::sub64(1.0, q))))
    }
}

impl Process {
    // ---- §8.6, §8.7

    /// `MatrixPrint(fp, m, precision)` to standard output or error (`None`: the C library's
    /// NULL stream, which FreeSurfer reports and replaces with standard output).
    pub fn matrix_print(&mut self, to_err: Option<bool>, m: Option<&Matrix>, precision: i32) -> i32 {
        let to_err = match to_err {
            Some(e) => e,
            None => {
                self.error_printf(codes::BAD_PARM, b"MatrixPrint: fp = NULL!");
                false
            }
        };
        let Some(m) = m else {
            self.error_printf(codes::BAD_PARM, b"MatrixPrint: mat = NULL!");
            return codes::BAD_PARM;
        };
        let p = if precision < 0 { 6 } else { precision };
        let fmt = format!("% 2.{p}f");
        let mut b = Vec::new();
        let mut status = 0;
        'rows: for r in 1..=m.rows {
            for c in 1..=m.cols {
                match m.kind {
                    MATRIX_REAL => b.extend_from_slice(&sprintf_f64(fmt.as_bytes(), sse::widen(m.at(r, c)))),
                    MATRIX_COMPLEX => {
                        let (re, im) = m.pair(r, c);
                        b.extend_from_slice(&sprintf_f64(b"% 2.3f", sse::widen(re)));
                        b.extend_from_slice(b" + ");
                        b.extend_from_slice(&sprintf_f64(b"% 2.3f", sse::widen(im)));
                        b.extend_from_slice(b" i");
                    }
                    t => {
                        self.write_to(to_err, &b);
                        b.clear();
                        self.error_printf(codes::BAD_PARM, format!("MatrixPrint: unknown type {t}\n").as_bytes());
                        status = codes::BAD_PARM;
                        break 'rows;
                    }
                }
                if c < m.cols {
                    b.extend_from_slice(b"  ");
                }
            }
            b.extend_from_slice(b";\n");
        }
        self.write_to(to_err, &b);
        status
    }

    fn write_to(&mut self, to_err: bool, b: &[u8]) {
        if to_err { self.put_err(b) } else { self.put_out(b) }
    }

    /// `MatrixPrintFmt(stdout, fmt, m)`.
    pub fn matrix_print_fmt(&mut self, fmt: &[u8], m: Option<&Matrix>) -> i32 {
        let Some(m) = m else {
            self.error_printf(codes::BAD_PARM, b"MatrixPrint: mat = NULL!");
            return codes::BAD_PARM;
        };
        let mut b = Vec::new();
        for r in 1..=m.rows {
            for c in 1..=m.cols {
                if m.is_complex() {
                    let (re, im) = m.pair(r, c);
                    b.extend_from_slice(&sprintf_f64(fmt, sse::widen(re)));
                    b.extend_from_slice(b" + ");
                    b.extend_from_slice(&sprintf_f64(fmt, sse::widen(im)));
                    b.extend_from_slice(b" i");
                } else {
                    b.extend_from_slice(&sprintf_f64(fmt, sse::widen(m.at(r, c))));
                }
                if c < m.cols {
                    b.extend_from_slice(b"  ");
                }
            }
            b.push(b'\n');
        }
        self.put_out(&b);
        0
    }

    /// `MatrixSimilarityTransform(src, mul, dst)`: `mul (src mulᵀ)` with the float products.
    pub fn matrix_similarity_transform(&mut self, src: &Matrix, mul: &Matrix, dst: Option<&mut Matrix>) -> Result<Returned<Matrix>, Stop> {
        let t = match self.matrix_transpose(mul, None)? {
            Returned::New(t) => t,
            _ => return Ok(Returned::Nothing),
        };
        let tmp = match self.matrix_multiply(Some(src), Some(&t), None)? {
            Returned::New(m) => Some(m),
            _ => None,
        };
        self.matrix_multiply(Some(mul), tmp.as_ref(), dst)
    }

    /// `GaussianMatrix(n, std, norm, G)` (§8.7).
    pub fn gaussian_matrix(&mut self, n: i32, std: f32, norm: i32, g: Option<&mut Matrix>) -> Result<Returned<Matrix>, Stop> {
        let mut fresh = None;
        let o: &mut Matrix = match g {
            None => match self.new_matrix(n, n, MATRIX_REAL)? {
                Some(x) => fresh.insert(x),
                None => return Ok(Returned::Nothing),
            },
            Some(o) => {
                if o.rows != n || o.cols != n {
                    self.put_out(b"ERROR: GaussianMatrix: dimension mismatch\n");
                    return Ok(Returned::Nothing);
                }
                o
            }
        };
        let var = sse::mul32(std, std);
        let mut f = sse::narrow(sse::mul64(sse::sqrt64(2.0 * std::f64::consts::PI), sse::widen(std)));
        // expf(-(d d) / (2 var)) for every distance d of the matrix, -(n - 1)..n - 1
        let mut e = Vec::new();
        for d in -(n - 1).max(0)..n.max(1) {
            let d = d as f32;
            let x = sse::div32(-sse::mul32(d, d), sse::mul32(2.0, var));
            e.push(self.libmf("expf", x, larmorx_core::math::expf, larmorx_core::math::expf_glibc));
        }
        let off = (n - 1).max(0);
        let g = |d: i32, f: f32| sse::div32(e[(d + off) as usize], f);
        if norm == 1 {
            let mut s = 0.0f32;
            for c in 0..n {
                s = sse::add32(s, g(c - n / 2, f));
            }
            f = sse::div32(f, s);
        }
        for r in 0..n {
            let mut s = 0.0f32;
            for c in 0..n {
                let v = g(c - r, f);
                o.set(r + 1, c + 1, v);
                s = sse::add32(s, v);
            }
            if norm == 2 {
                for c in 0..n {
                    let v = sse::div32(o.at(r + 1, c + 1), s);
                    o.set(r + 1, c + 1, v);
                }
            }
        }
        Ok(match fresh {
            Some(f) => Returned::New(f),
            None => Returned::Output,
        })
    }

    /// `GaussianMatrix2(n, s1, s2, w1, norm, G)`: `w1 G1 + (1 - w1) G2`.
    pub fn gaussian_matrix2(&mut self, n: i32, s1: f32, s2: f32, w1: f32, norm: i32, g: Option<&mut Matrix>) -> Result<Returned<Matrix>, Stop> {
        let g1 = match self.gaussian_matrix(n, s1, norm, None)? {
            Returned::New(m) => m,
            _ => return Ok(Returned::Nothing),
        };
        let g2 = match self.gaussian_matrix(n, s2, norm, None)? {
            Returned::New(m) => m,
            _ => return Ok(Returned::Nothing),
        };
        let a = match self.matrix_scalar_mul(&g1, w1, None)? {
            Returned::New(m) => m,
            _ => return Ok(Returned::Nothing),
        };
        let b = match self.matrix_scalar_mul(&g2, sse::sub32(1.0, w1), None)? {
            Returned::New(m) => m,
            _ => return Ok(Returned::Nothing),
        };
        self.matrix_add_sub(&a, &b, g, false)
    }
}
