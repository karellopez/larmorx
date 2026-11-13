// SPDX-License-Identifier: Apache-2.0
//! Float matrices (spec §2, §8.1, §16): the record and its addressing, allocation, freeing, the
//! copy, and the products accumulated in double and in float.

use larmorx_core::defects::{FS_13, FS_14, FS_15, FS_16};

use super::{Output, Returned};
use crate::process::{BUILD_ROOT, Process, Stop, codes};
use crate::sse;

/// The type of a real matrix.
pub const MATRIX_REAL: i32 = 1;
/// The type of a complex matrix.
pub const MATRIX_COMPLEX: i32 = 2;

/// A float matrix (§2.1): its type, its sizes and FreeSurfer's array of floats, which starts with
/// two unused floats; one-based elements, row by row, two floats an element when complex.
#[derive(Clone, Debug, PartialEq)]
pub struct Matrix {
    /// 1 real, 2 complex (another value only for a matrix of no rows).
    pub kind: i32,
    pub rows: i32,
    pub cols: i32,
    /// The whole array, from its float 0.
    pub data: Vec<f32>,
    /// Whether the matrix lies in a caller's buffer (`MatrixAlloc2`, §2.2).
    pub in_buf: bool,
}

impl Matrix {
    /// A real matrix of zeros with the default allocator's array (for internal results).
    pub fn zeros(rows: i32, cols: i32, kind: i32) -> Matrix {
        let rc = (rows.max(0) as usize) * (cols.max(0) as usize);
        let len = if kind == MATRIX_COMPLEX {
            2 * rc + 4
        } else {
            rc + 2
        };
        Matrix {
            kind,
            rows,
            cols,
            data: vec![0.0; len],
            in_buf: false,
        }
    }

    /// Whether the matrix is complex.
    pub fn is_complex(&self) -> bool {
        self.kind == MATRIX_COMPLEX
    }

    /// The count of the floats that hold the numbers (`rows cols`, twice for complex).
    pub fn numbers(&self) -> usize {
        let rc = (self.rows.max(0) as usize) * (self.cols.max(0) as usize);
        if self.is_complex() { 2 * rc } else { rc }
    }

    /// The numbers, from float 2 (what the probe prints after the two unused floats).
    pub fn number(&self, i: usize) -> f32 {
        self.data.get(2 + i).copied().unwrap_or(0.0)
    }

    /// The real-addressed place of (r, c) by the matrix's own type (§2.1): float
    /// `1 + (r - 1) cols + c` of a real matrix, `2 + 2((r - 1) cols - 1) + c` of a complex one;
    /// row 0 or beyond `rows` has none.
    pub fn real_place(&self, r: i32, c: i32) -> Option<i64> {
        if r < 1 || r > self.rows {
            return None;
        }
        let (r, c, n) = (i64::from(r), i64::from(c), i64::from(self.cols));
        Some(if self.is_complex() {
            2 + 2 * ((r - 1) * n - 1) + c
        } else {
            1 + (r - 1) * n + c
        })
    }

    /// The complex-addressed pair of (r, c) (its first float): `2 + 2((r - 1) cols + c - 1)` of a
    /// complex matrix, `2 + ((r - 1) cols - 1) + 2c` of a real one.
    pub fn complex_place(&self, r: i32, c: i32) -> Option<i64> {
        if r < 1 || r > self.rows {
            return None;
        }
        let (r, c, n) = (i64::from(r), i64::from(c), i64::from(self.cols));
        Some(if self.is_complex() {
            2 + 2 * ((r - 1) * n + c - 1)
        } else {
            2 + ((r - 1) * n - 1) + 2 * c
        })
    }

    /// The float at a place, 0 outside the array (§1).
    pub fn read(&self, place: Option<i64>) -> f32 {
        match place {
            Some(i) if i >= 0 && (i as usize) < self.data.len() => self.data[i as usize],
            _ => 0.0,
        }
    }

    /// Writes the float at a place; outside the array the write is dropped (§1). Returns whether
    /// it landed.
    pub fn write(&mut self, place: Option<i64>, v: f32) -> bool {
        match place {
            Some(i) if i >= 0 && (i as usize) < self.data.len() => {
                self.data[i as usize] = v;
                true
            }
            _ => false,
        }
    }

    /// Whether a place lies inside the array.
    pub fn inside(&self, place: Option<i64>) -> bool {
        matches!(place, Some(i) if i >= 0 && (i as usize) < self.data.len())
    }

    /// Element (r, c), real-addressed (1-based).
    pub fn at(&self, r: i32, c: i32) -> f32 {
        self.read(self.real_place(r, c))
    }

    /// Sets element (r, c), real-addressed.
    pub fn set(&mut self, r: i32, c: i32, v: f32) {
        let p = self.real_place(r, c);
        self.write(p, v);
    }

    /// The real and imaginary parts of (r, c), complex-addressed.
    pub fn pair(&self, r: i32, c: i32) -> (f32, f32) {
        let p = self.complex_place(r, c);
        (self.read(p), self.read(p.map(|i| i + 1)))
    }

    /// Sets (r, c)'s parts, complex-addressed.
    pub fn set_pair(&mut self, r: i32, c: i32, re: f32, im: f32) {
        let p = self.complex_place(r, c);
        self.write(p, re);
        self.write(p.map(|i| i + 1), im);
    }

    /// Whether `self` and `other` have the same sizes.
    pub fn same_size(&self, other: &Matrix) -> bool {
        self.rows == other.rows && self.cols == other.cols
    }
}

/// Which product (§2.5 or §8.1).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Accumulate {
    Double,
    Float,
}

impl Process {
    /// Whether the old allocator is in use: `FREESURFER_MatrixAlloc_old` set at the first
    /// allocation, kept for the run (§2.2).
    fn old_allocator(&mut self) -> bool {
        if let Some(v) = self.maths.old_alloc {
            return v;
        }
        let v = self.env.is_set("FREESURFER_MatrixAlloc_old");
        self.maths.old_alloc = Some(v);
        v
    }

    /// `MatrixAlloc(rows, cols, type)` (§2.2): a matrix of zeros; an unknown type with rows is
    /// an error (nothing returned). FS-13 (a negative count, or a product beyond an `int`): by
    /// default the two lines and the end of the program (status 1); with `reproduce`,
    /// FreeSurfer's matrix where it makes one, its two lines and exit where the old allocator
    /// gives them, the default's refusal where FreeSurfer crashes.
    pub fn matrix_alloc(
        &mut self,
        rows: i32,
        cols: i32,
        kind: i32,
    ) -> Result<Option<Matrix>, Stop> {
        self.allocate(rows, cols, kind, false)
    }

    /// `MatrixAlloc2(rows, cols, type, buffer)` (§2.2): as [`Process::matrix_alloc`], the matrix
    /// marked as in the caller's buffer when the default allocator's block fits 192 bytes.
    pub fn matrix_alloc2(
        &mut self,
        rows: i32,
        cols: i32,
        kind: i32,
    ) -> Result<Option<Matrix>, Stop> {
        self.allocate(rows, cols, kind, true)
    }

    /// Both allocation forms (§2.2); `buffer` for `MatrixAlloc2`.
    fn allocate(
        &mut self,
        rows: i32,
        cols: i32,
        kind: i32,
        buffer: bool,
    ) -> Result<Option<Matrix>, Stop> {
        let old = self.old_allocator();
        let complex = kind == MATRIX_COMPLEX;
        let known = kind == MATRIX_REAL || complex;
        let k: i64 = if complex { 2 } else { 1 };
        let rc = i64::from(rows) * i64::from(cols);
        let int = |x: i64| i32::try_from(x).is_ok();
        let beyond = !int(rc) || !int(k * (rc + 2));
        if rows < 0 || cols < 0 || beyond {
            // What FreeSurfer does with the size (§2.2's FS-13).
            #[derive(PartialEq)]
            enum Fs {
                Made,
                Exit,
                Crash,
            }
            let fs = if beyond {
                Fs::Crash
            } else if old {
                if k * rc + 2 < 0 {
                    Fs::Exit
                } else if rows <= -2 {
                    Fs::Crash
                } else {
                    Fs::Made
                }
            } else if rows >= -16 && rc >= -2 {
                Fs::Made
            } else {
                Fs::Crash
            };
            if self.mode().fix() || fs != Fs::Made {
                self.note(
                    &FS_13,
                    "a matrix of a negative size or of more numbers than an int holds: refused, the program ends with status 1",
                );
                return Err(self.matrix_alloc_failed(rows, cols));
            }
            self.note(
                &FS_13,
                "a matrix of a negative size: FreeSurfer's matrix of no element",
            );
        }
        if rows >= 1 && !known {
            self.error_printf(
                codes::BAD_PARM,
                format!("MatrixAlloc: unknown type {kind}\n").as_bytes(),
            );
            return Ok(None);
        }
        // §2.1's length; non-negative here (a negative count gives a length of 0 or more).
        let len = if complex {
            2 * rc + if old { 2 } else { 4 }
        } else {
            rc + 2
        };
        let len = usize::try_from(len).unwrap_or(0);
        let mut data = Vec::new();
        if data.try_reserve_exact(len).is_err() {
            return Err(self.matrix_alloc_failed(rows, cols));
        }
        data.resize(len, 0.0);
        // The default allocator's block (each size modulo 2^64 before each rounding up).
        let up = |b: u64| b.wrapping_add(63) & !63;
        let pointers = 64u64.wrapping_add(8u64.wrapping_mul((i64::from(rows) + 1) as u64));
        let numbers = (4 * k * (rc + 2)) as u64;
        let block = up(pointers).wrapping_add(up(numbers));
        let in_buf = buffer && !old && block <= 192;
        if !old && (-16..=-9).contains(&rows) {
            // The array lies over the matrix's own record (FS-13, `reproduce` only).
            let tag = kind + if in_buf { 65_536 } else { 0 };
            let record = [tag, rows, cols, 0];
            for (d, v) in data.iter_mut().zip(record) {
                *d = f32::from_bits(v as u32);
            }
            if len > 4 {
                self.note(
                    &FS_13,
                    "the array over the matrix's record: its four floats of memory contents 0",
                );
            }
        }
        Ok(Some(Matrix {
            kind,
            rows,
            cols,
            data,
            in_buf,
        }))
    }

    /// The two lines of a failed allocation and the end of the program (status 1).
    fn matrix_alloc_failed(&mut self, rows: i32, cols: i32) -> Stop {
        self.put_out(format!("MatrixAlloc({rows}, {cols}): Using mmap'd tmpfile\n").as_bytes());
        self.put_err(format!("MatrixAlloc({rows}, {cols}): allocation failed\n").as_bytes());
        self.flush();
        Stop::Exit(1)
    }

    /// `MatrixFree(&m)` (§2.3): the caller's matrix becomes none; returns 0.
    pub fn matrix_free(&mut self, m: &mut Option<Matrix>) -> i32 {
        *m = None;
        0
    }

    /// `MatrixCopy(in, out)` (§2.4). No `in`: the output is freed (`out` becomes none) and nothing
    /// returned. FS-14 for complex matrices.
    pub fn matrix_copy(
        &mut self,
        input: Option<&Matrix>,
        out: Output<'_, Matrix>,
    ) -> Result<Returned<Matrix>, Stop> {
        let Some(input) = input else {
            return Ok(Returned::Nothing);
        };
        match out {
            Output::New => {
                let Some(mut m) = self.matrix_alloc(input.rows, input.cols, input.kind)? else {
                    self.error_exit(codes::NO_MEMORY, b"MatrixCopy: couldn't allocate mOut")?;
                    return Ok(Returned::Nothing);
                };
                self.copy_into(input, &mut m);
                Ok(Returned::New(m))
            }
            Output::Given(o) => {
                if o.rows < input.rows || o.cols < input.cols {
                    return Ok(Returned::Nothing);
                }
                self.copy_into(input, o);
                Ok(Returned::Output)
            }
            // a matrix copied onto itself: every place keeps its value (FS-14's default copies
            // the same pairs), only the notice remains
            Output::First | Output::Second => {
                let mut m = input.clone();
                self.copy_into(input, &mut m);
                Ok(Returned::Output)
            }
        }
    }

    /// The element copy of `MatrixCopy` into an output at least as large.
    pub(crate) fn copy_into(&mut self, input: &Matrix, out: &mut Matrix) {
        if input.is_complex() && out.is_complex() && input.rows > 0 && input.cols > 0 {
            if self.mode().reproduce() {
                self.note(
                    &FS_14,
                    "a complex matrix copied as FreeSurfer copies it: half its values, shifted",
                );
            } else {
                self.note(
                    &FS_14,
                    "both parts of every element of a complex matrix copied",
                );
                for r in 1..=input.rows {
                    for c in 1..=input.cols {
                        let (re, im) = input.pair(r, c);
                        out.set_pair(r, c, re, im);
                    }
                }
                return;
            }
        }
        for c in 1..=input.cols {
            for r in 1..=input.rows {
                let v = input.at(r, c);
                out.set(r, c, v);
            }
        }
    }

    /// `MatrixMultiplyD(m1, m2, m3)` (§2.5): the product accumulated in double. FS-15, FS-16.
    /// An output that is also an input: pass a copy of that input as the input (FreeSurfer copies
    /// it first, step 4) and the matrix itself as `m3`.
    pub fn matrix_multiply_d(
        &mut self,
        m1: Option<&Matrix>,
        m2: Option<&Matrix>,
        m3: Option<&mut Matrix>,
    ) -> Result<Returned<Matrix>, Stop> {
        self.matrix_product(m1, m2, m3, Accumulate::Double)
    }

    /// `MatrixMultiply(m1, m2, m3)` (§8.1, §16): the product accumulated in float (an output
    /// that is also an input: as [`Process::matrix_multiply_d`]).
    pub fn matrix_multiply(
        &mut self,
        m1: Option<&Matrix>,
        m2: Option<&Matrix>,
        m3: Option<&mut Matrix>,
    ) -> Result<Returned<Matrix>, Stop> {
        self.matrix_product(m1, m2, m3, Accumulate::Float)
    }

    /// The products' checks (§2.5 steps 1 to 3, §8.1 steps 1 to 3).
    fn matrix_product(
        &mut self,
        m1: Option<&Matrix>,
        m2: Option<&Matrix>,
        m3: Option<&mut Matrix>,
        acc: Accumulate,
    ) -> Result<Returned<Matrix>, Stop> {
        let (name, short) = match acc {
            Accumulate::Double => ("MatrixMultiplyD()", "MatrixMultiplyD"),
            Accumulate::Float => ("MatrixMultiply()", "MatrixMultiply"),
        };
        for (which, m, line_d, line_f) in [
            ("m1", m1.is_none(), 597, 725),
            ("m2", m2.is_none(), 601, 729),
        ] {
            if m {
                match acc {
                    Accumulate::Double => {
                        let msg = format!(
                            "{name}: {which} is null\n break {BUILD_ROOT}/utils/matrix.cpp:{line_d}\n"
                        );
                        self.error_exit(codes::BAD_PARM, msg.as_bytes())?;
                    }
                    Accumulate::Float => {
                        self.put_out(
                            format!(
                                "{name}: {which} is null \n break {BUILD_ROOT}/utils/matrix.cpp:{line_f}\n"
                            )
                            .as_bytes(),
                        );
                        self.error_exit(
                            codes::BAD_PARM,
                            format!("{short}: {which} is null\n").as_bytes(),
                        )?;
                    }
                }
                return Ok(Returned::Nothing);
            }
        }
        let (a, b) = (m1.expect("m1"), m2.expect("m2"));
        if a.cols != b.rows {
            match acc {
                Accumulate::Double => {
                    let msg = format!(
                        "{name}: m1 cols {} does not match m2 rows {}\n break {BUILD_ROOT}/utils/matrix.cpp:610\n",
                        a.cols, b.rows
                    );
                    self.error_printf(codes::BAD_PARM, msg.as_bytes());
                }
                Accumulate::Float => {
                    self.put_out(
                        format!(
                            "{name}: m1/m2 dim mismatch\n break {BUILD_ROOT}/utils/matrix.cpp:733\n"
                        )
                        .as_bytes(),
                    );
                    let msg = format!(
                        "{short}: m1 cols {} does not match m2 rows {}\n",
                        a.cols, b.rows
                    );
                    self.error_printf(codes::BAD_PARM, msg.as_bytes());
                }
            }
            return Ok(Returned::Nothing);
        }
        let complex = a.is_complex() || b.is_complex();
        let mismatch = |p: &mut Process, o: &Matrix| {
            let (text, line) = match acc {
                Accumulate::Double => ("m1/m2 dim mismatch", 623),
                Accumulate::Float => ("m3 dim mismatch", 748),
            };
            p.put_out(
                format!("{name}: {text}\n break {BUILD_ROOT}/utils/matrix.cpp:{line}\n").as_bytes(),
            );
            let msg = format!(
                "{short}: ({} x {}) * ({} x {}) != ({} x {})\n",
                a.rows, a.cols, b.rows, b.cols, o.rows, o.cols
            );
            p.error_printf(codes::BAD_PARM, msg.as_bytes());
        };
        match m3 {
            None => {
                let kind = if complex { MATRIX_COMPLEX } else { a.kind };
                let Some(mut o) = self.matrix_alloc(a.rows, b.cols, kind)? else {
                    return Ok(Returned::Nothing);
                };
                if self.product_into(a, b, &mut o, acc, false, short) {
                    Ok(Returned::New(o))
                } else {
                    Ok(Returned::Nothing)
                }
            }
            Some(o) => {
                if o.rows != a.rows || o.cols != b.cols {
                    mismatch(self, o);
                    return Ok(Returned::Nothing);
                }
                if self.product_into(a, b, o, acc, true, short) {
                    Ok(Returned::Output)
                } else {
                    Ok(Returned::Nothing)
                }
            }
        }
    }

    /// The product of the inputs into `out` (steps 4 to 6); `given`: `out` is the caller's
    /// matrix (FS-15's notice). Returns false when FS-16's default refuses it.
    fn product_into(
        &mut self,
        m1: &Matrix,
        m2: &Matrix,
        out: &mut Matrix,
        acc: Accumulate,
        given: bool,
        short: &str,
    ) -> bool {
        let (a, b) = (m1, m2);
        if !a.is_complex() && !b.is_complex() {
            match acc {
                Accumulate::Double => real_product_d(a, b, out),
                Accumulate::Float => real_product_f(a, b, out),
            }
            return true;
        }
        if !out.is_complex() {
            if self.mode().fix() {
                self.note(&FS_16, "a complex product into a real matrix refused");
                self.error_printf(
                    codes::BAD_PARM,
                    format!("{short}: a complex product into a real matrix\n").as_bytes(),
                );
                return false;
            }
            self.note(
                &FS_16,
                "a complex product into a real matrix written at FreeSurfer's places (those outside dropped)",
            );
        }
        let reproduce = self.mode().reproduce();
        let mut held = false;
        for r in 1..=a.rows {
            for c in 1..=b.cols {
                let p = out.complex_place(r, c);
                let (re0, im0) = (out.read(p), out.read(p.map(|i| i + 1)));
                if given && (re0.to_bits() != 0 || im0.to_bits() != 0) {
                    held = true;
                }
                let (re, im) = if reproduce { (re0, im0) } else { (0.0, 0.0) };
                let (re, im) = complex_element(a, b, r, c, re, im, acc);
                out.write(p, re);
                out.write(p.map(|i| i + 1), im);
            }
        }
        if held {
            if reproduce {
                self.note(
                    &FS_15,
                    "the complex product added onto the output's earlier contents",
                );
            } else {
                self.note(
                    &FS_15,
                    "the complex product started from 0, not the output's earlier contents",
                );
            }
        }
        true
    }
}

/// One element of a complex product (§2.5 step 6, §8.1 step 5): the sums from `re`, `im`.
fn complex_element(
    a: &Matrix,
    b: &Matrix,
    r: i32,
    c: i32,
    re: f32,
    im: f32,
    acc: Accumulate,
) -> (f32, f32) {
    let (mut re, mut im) = (re, im);
    for i in 1..=a.cols {
        let (ar, ai) = if a.is_complex() {
            a.pair(r, i)
        } else {
            (a.at(r, i), 0.0)
        };
        let (br, bi) = if b.is_complex() {
            b.pair(i, c)
        } else {
            (b.at(i, c), 0.0)
        };
        match acc {
            Accumulate::Double => {
                let (x, y, u, v) = (
                    sse::widen(ar),
                    sse::widen(ai),
                    sse::widen(br),
                    sse::widen(bi),
                );
                let (t_re, t_im) = if !a.is_complex() {
                    (sse::mul64(x, u), sse::mul64(x, v))
                } else if !b.is_complex() {
                    (sse::mul64(x, u), sse::mul64(y, u))
                } else {
                    (
                        sse::sub64(sse::mul64(x, u), sse::mul64(y, v)),
                        sse::add64(sse::mul64(x, v), sse::mul64(y, u)),
                    )
                };
                // the term first (the official build's operand order, measured: `mproduct`)
                re = sse::narrow(sse::add64(t_re, sse::widen(re)));
                im = sse::narrow(sse::add64(t_im, sse::widen(im)));
            }
            Accumulate::Float => {
                let (t_re, t_im) = if !a.is_complex() {
                    (sse::mul32(ar, br), sse::mul32(ar, bi))
                } else if !b.is_complex() {
                    (sse::mul32(ar, br), sse::mul32(ai, br))
                } else {
                    (
                        sse::sub32(sse::mul32(ar, br), sse::mul32(ai, bi)),
                        sse::add32(sse::mul32(ar, bi), sse::mul32(ai, br)),
                    )
                };
                re = sse::add32(re, t_re);
                im = sse::add32(im, t_im);
            }
        }
    }
    (re, im)
}

/// The real product accumulated in double (§2.5 step 5) into `out`'s real-addressed places.
fn real_product_d(a: &Matrix, b: &Matrix, out: &mut Matrix) {
    let (n, m) = (a.cols.max(0) as usize, b.cols.max(0) as usize);
    let simple = !out.is_complex() && out.cols == b.cols;
    // One row at a time, every element's sum in its own order (i ascending), the columns side by
    // side (vectorised); a NaN is absorbing under + and x, so only a NaN result can depend on
    // x86-64's NaN rules: those elements are summed again with them.
    let mut acc = vec![0.0f64; m];
    for r in 1..=a.rows {
        let ra = 2 + (r as usize - 1) * n;
        acc.iter_mut().for_each(|v| *v = 0.0);
        for i in 0..n {
            let x = f64::from(a.data[ra + i]);
            let row = &b.data[2 + i * m..2 + i * m + m];
            for (v, &y) in acc.iter_mut().zip(row) {
                *v += x * f64::from(y);
            }
        }
        for c in 1..=b.cols {
            let mut s = acc[c as usize - 1];
            if s.is_nan() {
                s = 0.0;
                for i in 0..n {
                    let x = sse::widen(a.data[ra + i]);
                    let y = sse::widen(b.data[2 + i * m + (c as usize - 1)]);
                    s = sse::add64(s, sse::mul64(x, y));
                }
            }
            if simple {
                out.data[2 + (r as usize - 1) * m + (c as usize - 1)] = sse::narrow(s);
            } else {
                out.set(r, c, sse::narrow(s));
            }
        }
    }
}

/// The real product accumulated in float (§8.1 step 4) with the official build's operand order
/// (§16): each product `m2(i, c) × m1(r, i)`; blocks of four `val + p0`, `p1 + val`, `p2 + val`,
/// `val + p3`, then `val + p`.
fn real_product_f(a: &Matrix, b: &Matrix, out: &mut Matrix) {
    let (n, m) = (a.cols.max(0) as usize, b.cols.max(0) as usize);
    let simple = !out.is_complex() && out.cols == b.cols;
    // As `real_product_d`: plain arithmetic in each element's order (an operand order changes
    // only a NaN's bits), the NaN results summed again with the official build's order.
    let mut acc = vec![0.0f32; m];
    for r in 1..=a.rows {
        let ra = 2 + (r as usize - 1) * n;
        acc.iter_mut().for_each(|v| *v = 0.0);
        for i in 0..n {
            let x = a.data[ra + i];
            let row = &b.data[2 + i * m..2 + i * m + m];
            for (v, &y) in acc.iter_mut().zip(row) {
                *v += y * x;
            }
        }
        for c in 1..=b.cols {
            let mut v = acc[c as usize - 1];
            if v.is_nan() {
                let p = |i: usize| sse::mul32(b.data[2 + i * m + (c as usize - 1)], a.data[ra + i]);
                v = 0.0f32;
                let mut i = 0;
                while i + 4 <= n {
                    v = sse::add32(v, p(i));
                    v = sse::add32(p(i + 1), v);
                    v = sse::add32(p(i + 2), v);
                    v = sse::add32(v, p(i + 3));
                    i += 4;
                }
                while i < n {
                    v = sse::add32(v, p(i));
                    i += 1;
                }
            }
            if simple {
                out.data[2 + (r as usize - 1) * m + (c as usize - 1)] = v;
            } else {
                out.set(r, c, v);
            }
        }
    }
}
