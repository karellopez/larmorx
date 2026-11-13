// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! FreeSurfer's float matrices (`fs.maths`, queue item 3.1-23), translated from FreeSurfer
//! 8.2.0's `include/matrix.h` (`MATRIX`, `MatrixBuffer`, the element macros) and
//! `utils/matrix.cpp` 75-720 (`MatrixAlloc_old`, `MatrixAlloc_new`, `use_new_MatricAlloc`,
//! `MatrixAlloc_wkr`, `MatrixAlloc2_wkr`, `MatrixFree`, `MatrixCopy`, `MatrixMultiplyD`; the
//! allocation statistics `noteMatrixAlloced` return at once, `if (1) return;`, so nothing of
//! them is seen). `MatrixInverse` and `MatrixQRdecomposition` (lines 183-296) call
//! `utils/numerics.cpp`'s VNL wrappers on `float` matrices (queue item 3.1-24, with the main
//! package's `float` VNL routines, item 3.1-07): they come with those.
//!
//! **The layout.** A `MATRIX` is one-based (Numerical Recipes' convention): `rptr[r][c]` for
//! `r` in `1..=rows`, `c` in `1..=cols`. Its numbers are one array, row by row, preceded by two
//! unused floats so that `rptr[1][0]` is a valid address; a complex matrix holds the real and
//! imaginary parts of each element side by side, and its row pointers point at floats, so
//! `MATRIX_RELT` on a complex matrix addresses its floats one by one (the row pointer of row
//! `r` is the float before element `(r, 1)`'s pair, counted in pairs). [`Matrix`] keeps that
//! array with the two leading floats (and the new allocator's two trailing pairs for complex
//! matrices); [`Matrix::relt_index`] and [`Matrix::celt_index`] compute the addresses the C
//! macros compute, so code that mixes the two (as `MatrixCopy` does on complex matrices, FS-14)
//! reads and writes the floats FreeSurfer's does.
//!
//! **The two allocators.** `MatrixAlloc_new` (the default) puts the header, the row pointers and
//! the numbers in one aligned block, or into the caller's `MatrixBuffer` when they fit
//! (`MatrixAlloc2`; such a matrix is never freed: `inBuf`); `MatrixAlloc_old` (the variable
//! `FREESURFER_MatrixAlloc_old` set) allocates the three separately. They give the same numbers,
//! zeros, and the same messages; a complex matrix of the new allocator has two more floats at its
//! end. FreeSurfer reads the variable at the first allocation and keeps the answer; no FreeSurfer
//! program sets it, so [`Globals::matrix_alloc_old`] is set at the program's start.
//!
//! Defects (`larmorx_core::defects`):
//! - **FS-13** (a clear error by default): a size that is negative or does not fit an `int`
//!   (`rows * cols`, with the two leading floats) is an allocation FreeSurfer's code cannot make:
//!   it prints `MatrixAlloc(r, c): Using mmap'd tmpfile` on standard output and then, depending on
//!   the sizes, crashes (the new allocator reads a file pointer it never set, or clears memory
//!   with a size that wrapped), or prints `MatrixAlloc(r, c): allocation failed` and exits with 1.
//!   Both modes print the two lines and exit with 1 (the crash depends on memory contents and is
//!   never reproduced); FS-13 is noted.
//! - **FS-14** (corrected by default): `MatrixCopy` copies element by element through
//!   `MATRIX_RELT` whatever the type, so of a complex matrix it copies `cols` floats per row from
//!   the float before the row's first pair (the first row's starts at the unused float before the
//!   numbers): half the values, shifted. The default copies both parts of every element;
//!   `reproduce` copies the floats FreeSurfer copies.
//! - **FS-15** (corrected by default): `MatrixMultiplyD`'s three complex branches add each
//!   product term to the output element without clearing it first (`MatrixClear` is commented
//!   out): a new output is zero, but a given output (or one that is also an input) adds its
//!   earlier contents to the product. The default clears the output's elements first;
//!   `reproduce` adds onto them.
//! - **FS-16** (both modes defined): an element addressed outside the matrix's array: a complex
//!   product written into a real output (its pairs, addressed through the real row pointers, run
//!   past the end of the array), and any other mismatch of types and sizes the C code does not
//!   check. FreeSurfer writes and reads memory it does not own there. A write outside the array
//!   is dropped and a read gives 0 in both modes, with FS-16 noted; by default a complex product
//!   into a real output is refused first, as an error (rule B).

use std::io::Write;

use larmorx_core::defects::{FS_13, FS_14, FS_15, FS_16};

use crate::diag::BUILD_ROOT;
use crate::globals::Globals;
use crate::log::{self, ERROR_BADPARM, ERROR_NO_MEMORY, Fatal};
use crate::ret::Ret;
use crate::sse;

/// `MATRIX_REAL`, `MATRIX_COMPLEX` (`include/matrix.h`).
pub const MATRIX_REAL: i16 = 1;
pub const MATRIX_COMPLEX: i16 = 2;
/// `MATRIX_SYM`, `MATRIX_UPPER`, `MATRIX_LOWER`.
pub const MATRIX_SYM: i32 = 0;
pub const MATRIX_UPPER: i32 = 1;
pub const MATRIX_LOWER: i32 = 2;

/// `sizeof(MatrixBuffer)` on x86-64 Linux: the `MATRIX` header (40 bytes), five row pointers
/// (40) and 27 floats (108), padded to 8 bytes.
pub const MATRIX_BUFFER_SIZE: usize = 192;

/// `utils/matrix.cpp`'s path in the official build, as `__FILE__` prints it in the messages.
pub fn matrix_cpp() -> String {
    format!("{BUILD_ROOT}/utils/matrix.cpp")
}

/// A FreeSurfer `MATRIX` (`VECTOR` is the same type with one column or one row): see the
/// module's documentation for the layout.
#[derive(Clone, Debug, PartialEq)]
pub struct Matrix {
    /// `type`: [`MATRIX_REAL`] or [`MATRIX_COMPLEX`].
    pub mtype: i16,
    /// `inBuf`: the matrix is in its caller's `MatrixBuffer` (`MatrixAlloc2`).
    pub in_buf: bool,
    pub rows: i32,
    pub cols: i32,
    /// The allocation: two unused floats, then the numbers (`data` points at index 2).
    store: Vec<f32>,
}

/// Why an allocation of `rows` x `cols` could not be made (FS-13).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AllocFailure {
    pub rows: i32,
    pub cols: i32,
}

/// The number of floats the `data` pointer starts after.
const LEAD: isize = 2;

impl Matrix {
    /// A matrix of zeros, as FreeSurfer's allocator makes it (`old`: `MatrixAlloc_old`'s array
    /// size); `None` where the size is negative or does not fit (FS-13; the caller prints).
    fn zeros(rows: i32, cols: i32, mtype: i16, old: bool) -> Result<Matrix, AllocFailure> {
        let fail = AllocFailure { rows, cols };
        if rows < 0 || cols < 0 {
            return Err(fail);
        }
        let n = i64::from(rows) * i64::from(cols);
        let pair = if mtype == MATRIX_COMPLEX { 2 } else { 1 };
        // new: ((rows * cols) + 2) * pair floats, data 2 floats in; old: rows * cols * pair + 2.
        let len = if old { n * pair + 2 } else { (n + 2) * pair };
        if n * pair + 2 > i64::from(i32::MAX) || len > i64::from(i32::MAX) {
            return Err(fail);
        }
        let len = len as usize;
        let mut store = Vec::new();
        if store.try_reserve_exact(len).is_err() {
            return Err(fail);
        }
        store.resize(len, 0.0);
        Ok(Matrix {
            mtype,
            in_buf: false,
            rows,
            cols,
            store,
        })
    }

    /// A real matrix of zeros (the new allocator's layout); panics where FreeSurfer's allocation
    /// would fail (for code that knows its sizes are valid).
    pub fn real(rows: i32, cols: i32) -> Matrix {
        Matrix::zeros(rows, cols, MATRIX_REAL, false).expect("matrix size out of range")
    }

    /// A real matrix from its rows (one-based in the C code: `m[r - 1][c - 1]` is element
    /// `(r, c)`).
    pub fn from_rows(m: &[Vec<f32>]) -> Matrix {
        let rows = m.len() as i32;
        let cols = m.first().map_or(0, |r| r.len()) as i32;
        let mut out = Matrix::real(rows, cols);
        for (r, row) in m.iter().enumerate() {
            assert_eq!(row.len() as i32, cols, "rows of different lengths");
            for (c, &v) in row.iter().enumerate() {
                out.set_relt(r as i32 + 1, c as i32 + 1, v);
            }
        }
        out
    }

    /// The offset of row `r`'s pointer from `data`, in floats (`rptr[r] - data`), or `None` for
    /// a row the matrix has no pointer for (`rptr[0]` is `NULL`; beyond `rows` is outside the
    /// pointer array).
    fn row_base(&self, r: i32) -> Option<isize> {
        if r < 1 || r > self.rows {
            return None;
        }
        let k = (r as isize - 1) * self.cols as isize - 1;
        match self.mtype {
            MATRIX_REAL => Some(k),
            MATRIX_COMPLEX => Some(2 * k),
            _ => None,
        }
    }

    /// `MATRIX_RELT(m, r, c)`'s place in the array (`rptr[r] + c`), whatever the type; `None`
    /// outside the array.
    pub fn relt_index(&self, r: i32, c: i32) -> Option<usize> {
        let i = LEAD + self.row_base(r)? + c as isize;
        (i >= 0 && (i as usize) < self.store.len()).then_some(i as usize)
    }

    /// `MATRIX_CELT(m, r, c)`'s real part's place (`(COMPLEX_FLOAT *) rptr[r] + c`), whatever the
    /// type; the imaginary part follows it. `None` when either is outside the array.
    pub fn celt_index(&self, r: i32, c: i32) -> Option<usize> {
        let i = LEAD + self.row_base(r)? + 2 * c as isize;
        (i >= 0 && (i as usize + 1) < self.store.len()).then_some(i as usize)
    }

    /// `*MATRIX_RELT(m, r, c)`; panics outside the array (an error of the translation's caller).
    pub fn relt(&self, r: i32, c: i32) -> f32 {
        self.store[self
            .relt_index(r, c)
            .expect("MATRIX_RELT outside the matrix")]
    }

    /// `*MATRIX_RELT(m, r, c) = v`.
    pub fn set_relt(&mut self, r: i32, c: i32, v: f32) {
        let i = self
            .relt_index(r, c)
            .expect("MATRIX_RELT outside the matrix");
        self.store[i] = v;
    }

    /// `MATRIX_RELT(m, r, c)` as a place to write.
    pub fn relt_mut(&mut self, r: i32, c: i32) -> &mut f32 {
        let i = self
            .relt_index(r, c)
            .expect("MATRIX_RELT outside the matrix");
        &mut self.store[i]
    }

    /// `MATRIX_CELT(m, r, c)`'s parts.
    pub fn celt(&self, r: i32, c: i32) -> (f32, f32) {
        let i = self
            .celt_index(r, c)
            .expect("MATRIX_CELT outside the matrix");
        (self.store[i], self.store[i + 1])
    }

    /// `MATRIX_CELT(m, r, c)`'s parts, set.
    pub fn set_celt(&mut self, r: i32, c: i32, re: f32, im: f32) {
        let i = self
            .celt_index(r, c)
            .expect("MATRIX_CELT outside the matrix");
        self.store[i] = re;
        self.store[i + 1] = im;
    }

    /// The array's float at `i` (`None` outside it).
    pub fn raw(&self, i: usize) -> Option<f32> {
        self.store.get(i).copied()
    }

    /// The array's float at `i`, set (`false`, nothing written, outside it).
    pub fn set_raw(&mut self, i: usize, v: f32) -> bool {
        match self.store.get_mut(i) {
            Some(p) => {
                *p = v;
                true
            }
            None => false,
        }
    }

    /// The number of floats of the numbers: `rows * cols`, twice that for a complex matrix.
    pub fn nfloats(&self) -> usize {
        let n = self.rows.max(0) as usize * self.cols.max(0) as usize;
        if self.mtype == MATRIX_COMPLEX {
            2 * n
        } else {
            n
        }
    }

    /// `m->data[0 ..]`: the numbers, row by row (pairs for a complex matrix).
    pub fn data(&self) -> &[f32] {
        let n = self.nfloats();
        &self.store[2..2 + n]
    }

    /// `m->data[0 ..]`, to write.
    pub fn data_mut(&mut self) -> &mut [f32] {
        let n = self.nfloats();
        &mut self.store[2..2 + n]
    }

    /// The whole array: the two unused floats, the numbers and any floats after them.
    pub fn store(&self) -> &[f32] {
        &self.store
    }
}

/// `use_new_MatricAlloc()`: the new allocator unless `FREESURFER_MatrixAlloc_old` is set.
pub fn use_new_matrix_alloc(g: &Globals) -> bool {
    !g.matrix_alloc_old
}

/// `MatrixAlloc_new`'s block size for a `rows` x `cols` matrix of `mtype` (the header, the row
/// pointers and the numbers, each rounded up to 64 bytes), for valid sizes.
pub fn matrix_alloc_new_size(rows: i32, cols: i32, mtype: i16) -> usize {
    let round = |s: usize| (s + 63) & !63;
    let mut size = round(40);
    size += (rows as usize + 1) * 8;
    size = round(size);
    let pair = if mtype == MATRIX_COMPLEX { 2 } else { 1 };
    let nelts = (rows as usize * cols as usize + 2) * pair;
    size += nelts * 4;
    round(size)
}

/// FS-13's two lines and exit status 1 (see the module's documentation).
fn alloc_failed(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    f: AllocFailure,
) -> Fatal {
    let _ = writeln!(
        out,
        "MatrixAlloc({}, {}): Using mmap'd tmpfile",
        f.rows, f.cols
    );
    let _ = writeln!(
        err,
        "MatrixAlloc({}, {}): allocation failed",
        f.rows, f.cols
    );
    g.notices.add(
        &FS_13,
        format!(
            "MatrixAlloc({}, {}): a size that cannot be allocated, a clear error (FreeSurfer crashes or exits)",
            f.rows, f.cols
        ),
    );
    Fatal::Exit(1)
}

fn matrix_alloc_wkr(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    rows: i32,
    cols: i32,
    mtype: i16,
    buf: bool,
) -> Result<Option<Matrix>, Fatal> {
    let old = !use_new_matrix_alloc(g);
    // `MatrixAlloc_old` doubles the numbers for a complex matrix only; an unknown type is sized
    // as a real one by both.
    let mut m = match Matrix::zeros(rows, cols, mtype, old) {
        Ok(m) => m,
        Err(f) => return Err(alloc_failed(g, out, err, f)),
    };
    if !old && buf {
        m.in_buf = matrix_alloc_new_size(rows, cols, mtype) <= MATRIX_BUFFER_SIZE;
    }
    if rows >= 1 && mtype != MATRIX_REAL && mtype != MATRIX_COMPLEX {
        log::error_printf(
            g,
            err,
            ERROR_BADPARM,
            &format!("MatrixAlloc: unknown type {mtype}\n"),
        );
        return Ok(None);
    }
    Ok(Some(m))
}

/// `MatrixAlloc(rows, cols, type)`: a matrix of zeros; `None` (`NULL`) for an unknown type
/// with at least one row (after `MatrixAlloc: unknown type <type>` as an error); an exit for a
/// size that cannot be allocated (FS-13).
pub fn matrix_alloc(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    rows: i32,
    cols: i32,
    mtype: i16,
) -> Result<Option<Matrix>, Fatal> {
    matrix_alloc_wkr(g, out, err, rows, cols, mtype, false)
}

/// `MatrixAlloc2(rows, cols, type, buf)`: as [`matrix_alloc`], in the caller's `MatrixBuffer`
/// when the new allocator's block fits it (`in_buf`; never with the old allocator). The numbers
/// are the same.
pub fn matrix_alloc2(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    rows: i32,
    cols: i32,
    mtype: i16,
) -> Result<Option<Matrix>, Fatal> {
    matrix_alloc_wkr(g, out, err, rows, cols, mtype, true)
}

/// `MatrixFree(&m)`: `m` becomes `NULL`; 0. (`MatrixFree(NULL)`, a null pointer to the pointer,
/// cannot be written in Rust.)
pub fn matrix_free(m: &mut Option<Matrix>) -> i32 {
    *m = None;
    0
}

/// `MatrixCopy(mIn, mOut)`: `mIn` into `mOut` (a new matrix of `mIn`'s size and type when there
/// is none); `Null` when `mIn` is `NULL` (FreeSurfer then frees `mOut`: the caller should drop
/// it) or `mOut` has fewer rows or columns than `mIn` (FS-14 for complex matrices).
pub fn matrix_copy(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    m_in: Option<&Matrix>,
    m_out: Option<&mut Matrix>,
) -> Result<Ret<Matrix>, Fatal> {
    let Some(m_in) = m_in else {
        return Ok(Ret::Null);
    };
    let mut fresh = None;
    let dst: &mut Matrix = match m_out {
        Some(m) => {
            if m.rows < m_in.rows || m.cols < m_in.cols {
                return Ok(Ret::Null);
            }
            m
        }
        None => match matrix_alloc(g, out, err, m_in.rows, m_in.cols, m_in.mtype)? {
            Some(m) => fresh.insert(m),
            None => {
                log::error_exit(
                    g,
                    err,
                    ERROR_NO_MEMORY,
                    "MatrixCopy: couldn't allocate mOut",
                )?;
                return Ok(Ret::Null);
            }
        },
    };
    copy_elements(g, m_in, dst);
    Ok(match fresh {
        Some(m) => Ret::New(m),
        None => Ret::Given,
    })
}

/// `MatrixCopy`'s loop, `*MATRIX_RELT(mOut, row, col) = *MATRIX_RELT(mIn, row, col)` column
/// by column (FS-14 for complex matrices; FS-16 for an element outside an array).
fn copy_elements(g: &mut Globals, m_in: &Matrix, dst: &mut Matrix) {
    let (rows, cols) = (m_in.rows, m_in.cols);
    let complex = m_in.mtype == MATRIX_COMPLEX || dst.mtype == MATRIX_COMPLEX;
    if complex && !g.notices.reproduce() && m_in.mtype == dst.mtype {
        for col in 1..=cols {
            for row in 1..=rows {
                let (re, im) = m_in.celt(row, col);
                dst.set_celt(row, col, re, im);
            }
        }
        g.notices.add(
            &FS_14,
            "MatrixCopy of a complex matrix: every element's two parts copied",
        );
        return;
    }
    if complex {
        g.notices.add(
            &FS_14,
            "MatrixCopy of a complex matrix: the floats MATRIX_RELT addresses, as FreeSurfer copies them",
        );
    }
    for col in 1..=cols {
        for row in 1..=rows {
            let v = read_relt(g, m_in, row, col);
            write_relt(g, dst, row, col, v);
        }
    }
}

/// `*MATRIX_RELT(m, r, c)` read where it may fall outside the array (FS-16: 0 there).
pub(crate) fn read_relt(g: &mut Globals, m: &Matrix, r: i32, c: i32) -> f32 {
    match m.relt_index(r, c) {
        Some(i) => m.store[i],
        None => {
            g.notices
                .add(&FS_16, "a matrix element read outside its array: 0 used");
            0.0
        }
    }
}

/// `*MATRIX_RELT(m, r, c) = v` where it may fall outside the array (FS-16: dropped there).
pub(crate) fn write_relt(g: &mut Globals, m: &mut Matrix, r: i32, c: i32, v: f32) {
    match m.relt_index(r, c) {
        Some(i) => m.store[i] = v,
        None => g.notices.add(
            &FS_16,
            "a matrix element written outside its array: dropped",
        ),
    }
}

/// A float of `m`'s array read where the C pointer arithmetic may leave it (FS-16: 0 there).
pub(crate) fn read_at(g: &mut Globals, m: &Matrix, i: isize) -> f32 {
    if i >= 0
        && let Some(&v) = m.store.get(i as usize)
    {
        return v;
    }
    g.notices
        .add(&FS_16, "a matrix element read outside its array: 0 used");
    0.0
}

/// A float of `m`'s array written where the C pointer arithmetic may leave it (FS-16).
pub(crate) fn write_at(g: &mut Globals, m: &mut Matrix, i: isize, v: f32) {
    if i >= 0
        && let Some(p) = m.store.get_mut(i as usize)
    {
        *p = v;
        return;
    }
    g.notices.add(
        &FS_16,
        "a matrix element written outside its array: dropped",
    );
}

/// `rptr[r]`'s place in the array as the C code would compute it, for rows the matrix has
/// (the caller checks them).
pub(crate) fn rptr(m: &Matrix, r: i32) -> isize {
    LEAD + m.row_base(r).unwrap_or(isize::MIN / 4)
}

/// `MatrixMultiplyD(m1, m2, m3)`: `m1 * m2`, each element accumulated in `double` (FreeSurfer's
/// recommended product). `m1` or `m2` `NULL` ends the program (`MatrixMultiplyD(): m1 is null`
/// with the file and line, as an error); `m1`'s columns not `m2`'s rows, or a given `m3` of
/// another size, return `Null` after their messages. A new `m3` is complex when either input is.
/// To multiply into an input, pass a copy of it as the input (FreeSurfer copies it first).
/// FS-15, FS-16.
pub fn matrix_multiply_d(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    m1: Option<&Matrix>,
    m2: Option<&Matrix>,
    m3: Option<&mut Matrix>,
) -> Result<Ret<Matrix>, Fatal> {
    let file = matrix_cpp();
    let Some(m1) = m1 else {
        let msg = format!("MatrixMultiplyD(): m1 is null\n break {file}:597\n");
        log::error_exit(g, err, ERROR_BADPARM, &msg)?;
        return Ok(Ret::Null);
    };
    let Some(m2) = m2 else {
        let msg = format!("MatrixMultiplyD(): m2 is null\n break {file}:601\n");
        log::error_exit(g, err, ERROR_BADPARM, &msg)?;
        return Ok(Ret::Null);
    };
    if m1.cols != m2.rows {
        let msg = format!(
            "MatrixMultiplyD(): m1 cols {} does not match m2 rows {}\n break {file}:610\n",
            m1.cols, m2.rows
        );
        log::error_printf(g, err, ERROR_BADPARM, &msg);
        return Ok(Ret::Null);
    }
    let either_complex = m1.mtype == MATRIX_COMPLEX || m2.mtype == MATRIX_COMPLEX;
    let given = m3.is_some();
    let mut fresh = None;
    let m3: &mut Matrix = match m3 {
        None => {
            let t = if either_complex {
                MATRIX_COMPLEX
            } else {
                m1.mtype
            };
            match matrix_alloc(g, out, err, m1.rows, m2.cols, t)? {
                Some(m) => fresh.insert(m),
                None => return Ok(Ret::Null),
            }
        }
        Some(m3) => {
            if m3.rows != m1.rows || m3.cols != m2.cols {
                let _ = write!(
                    out,
                    "MatrixMultiplyD(): m1/m2 dim mismatch\n break {file}:623\n"
                );
                let msg = format!(
                    "MatrixMultiplyD: ({} x {}) * ({} x {}) != ({} x {})\n",
                    m1.rows, m1.cols, m2.rows, m2.cols, m3.rows, m3.cols
                );
                log::error_printf(g, err, ERROR_BADPARM, &msg);
                return Ok(Ret::Null);
            }
            m3
        }
    };
    if either_complex && m3.mtype != MATRIX_COMPLEX && !g.notices.reproduce() {
        // FS-16 (rule B): FreeSurfer writes the pairs through the real row pointers, past the
        // end of the array.
        log::error_printf(
            g,
            err,
            ERROR_BADPARM,
            "MatrixMultiplyD: a complex product into a real matrix\n",
        );
        g.notices.add(
            &FS_16,
            "MatrixMultiplyD: a complex product into a real matrix refused (FreeSurfer writes past its end)",
        );
        return Ok(Ret::Null);
    }
    multiply(g, m1, m2, m3, given, Acc::Double);
    Ok(match fresh {
        Some(m) => Ret::New(m),
        None => Ret::Given,
    })
}

/// `MatrixMultiply(m1, m2, m3)` (`MatrixMultiply_wkr`): `m1 * m2`, each element accumulated in
/// `float` (every product and sum rounded to `float`). `m1` or `m2` `NULL` ends the program
/// (`MatrixMultiply(): m1 is null ` with the file and line on standard output, then
/// `MatrixMultiply: m1 is null` as an error); `m1`'s columns not `m2`'s rows, or a given `m3` of
/// another size, return `Null` after a line on standard output and an error. A new `m3` is
/// complex when either input is. To multiply into an input, pass a copy of it as the input
/// (FreeSurfer copies it first). FS-15, FS-16.
pub fn matrix_multiply(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    m1: Option<&Matrix>,
    m2: Option<&Matrix>,
    m3: Option<&mut Matrix>,
) -> Result<Ret<Matrix>, Fatal> {
    let file = matrix_cpp();
    let Some(m1) = m1 else {
        let _ = write!(out, "MatrixMultiply(): m1 is null \n break {file}:725\n");
        log::error_exit(g, err, ERROR_BADPARM, "MatrixMultiply: m1 is null\n")?;
        return Ok(Ret::Null);
    };
    let Some(m2) = m2 else {
        let _ = write!(out, "MatrixMultiply(): m2 is null \n break {file}:729\n");
        log::error_exit(g, err, ERROR_BADPARM, "MatrixMultiply: m2 is null\n")?;
        return Ok(Ret::Null);
    };
    if m1.cols != m2.rows {
        let _ = write!(
            out,
            "MatrixMultiply(): m1/m2 dim mismatch\n break {file}:733\n"
        );
        let msg = format!(
            "MatrixMultiply: m1 cols {} does not match m2 rows {}\n",
            m1.cols, m2.rows
        );
        log::error_printf(g, err, ERROR_BADPARM, &msg);
        return Ok(Ret::Null);
    }
    let either_complex = m1.mtype == MATRIX_COMPLEX || m2.mtype == MATRIX_COMPLEX;
    let given = m3.is_some();
    let mut fresh = None;
    let m3: &mut Matrix = match m3 {
        None => {
            let t = if either_complex {
                MATRIX_COMPLEX
            } else {
                m1.mtype
            };
            match matrix_alloc(g, out, err, m1.rows, m2.cols, t)? {
                Some(m) => fresh.insert(m),
                None => return Ok(Ret::Null),
            }
        }
        Some(m3) => {
            if m3.rows != m1.rows || m3.cols != m2.cols {
                let _ = write!(
                    out,
                    "MatrixMultiply(): m3 dim mismatch\n break {file}:748\n"
                );
                let msg = format!(
                    "MatrixMultiply: ({} x {}) * ({} x {}) != ({} x {})\n",
                    m1.rows, m1.cols, m2.rows, m2.cols, m3.rows, m3.cols
                );
                log::error_printf(g, err, ERROR_BADPARM, &msg);
                return Ok(Ret::Null);
            }
            m3
        }
    };
    if either_complex && m3.mtype != MATRIX_COMPLEX && !g.notices.reproduce() {
        // FS-16 (rule B), as MatrixMultiplyD.
        log::error_printf(
            g,
            err,
            ERROR_BADPARM,
            "MatrixMultiply: a complex product into a real matrix\n",
        );
        g.notices.add(
            &FS_16,
            "MatrixMultiply: a complex product into a real matrix refused (FreeSurfer writes past its end)",
        );
        return Ok(Ret::Null);
    }
    multiply(g, m1, m2, m3, given, Acc::Float);
    Ok(match fresh {
        Some(m) => Ret::New(m),
        None => Ret::Given,
    })
}

/// The accumulation of a product: `MatrixMultiply`'s `float` (every product and sum rounded to
/// `float`) or `MatrixMultiplyD`'s `double`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Acc {
    Float,
    Double,
}

impl Acc {
    fn name(self) -> &'static str {
        match self {
            Acc::Float => "MatrixMultiply",
            Acc::Double => "MatrixMultiplyD",
        }
    }
}

/// The four branches of `MatrixMultiply` (`acc` `Float`) and `MatrixMultiplyD` (`Double`) on
/// `m3` (`given`: the caller's, which FS-15 concerns).
/// `MatrixMultiply_wkr`'s real sum `val += *r1++ * *r2` (`a` the row of `m1`, `b` the column of
/// `m2`) as the official build computes it, which decides which NaN a sum of two NaNs keeps (the
/// values are the same in any order): each product is `b * a` (m2's element the first operand);
/// for 4 or more terms GCC 8.4.0 adds them in blocks of four, `val + p0`, `p1 + val`, `p2 + val`,
/// `val + p3`, and the rest (or all of fewer than 4) as `val + p` (the disassembly of the unit
/// oracle's `matrix.cpp.o`: `mulps`, `addss` after `shufps` and `unpckhps`).
fn dot_float(a: &[f32], b: &[f32]) -> f32 {
    let n = a.len();
    let blocks = if n >= 4 { n / 4 } else { 0 };
    let mut val = 0.0f32;
    for k in 0..blocks {
        let i = 4 * k;
        let p: [f32; 4] = std::array::from_fn(|l| sse::mul(b[i + l], a[i + l]));
        val = sse::add(val, p[0]);
        val = sse::add(p[1], val);
        val = sse::add(p[2], val);
        val = sse::add(val, p[3]);
    }
    for i in 4 * blocks..n {
        val = sse::add(val, sse::mul(b[i], a[i]));
    }
    val
}

pub(crate) fn multiply(
    g: &mut Globals,
    m1: &Matrix,
    m2: &Matrix,
    m3: &mut Matrix,
    given: bool,
    acc: Acc,
) {
    let (rows, cols, m1_cols) = (m3.rows, m3.cols, m1.cols);
    let (t1, t2) = (m1.mtype, m2.mtype);
    if t1 == MATRIX_REAL && t2 == MATRIX_REAL {
        // r3 = &m3->rptr[row][1]; r1 = &m1->rptr[row][1]; r2 = &m2->rptr[1][col], r2 += cols.
        if m3.mtype == MATRIX_REAL && m2.cols == cols {
            for row in 1..=rows {
                for col in 1..=cols {
                    let v = match acc {
                        Acc::Double => {
                            let mut val = 0.0f64;
                            for i in 1..=m1_cols {
                                let p = sse::mul64(
                                    f64::from(m1.relt(row, i)),
                                    f64::from(m2.relt(i, col)),
                                );
                                val = sse::add64(val, p);
                            }
                            val as f32
                        }
                        Acc::Float => {
                            let a: Vec<f32> = (1..=m1_cols).map(|i| m1.relt(row, i)).collect();
                            let b: Vec<f32> = (1..=m1_cols).map(|i| m2.relt(i, col)).collect();
                            dot_float(&a, &b)
                        }
                    };
                    m3.set_relt(row, col, v);
                }
            }
            return;
        }
        for row in 1..=rows {
            let r3 = rptr(m3, row) + 1;
            for col in 1..=cols {
                let r1 = rptr(m1, row) + 1;
                let r2 = rptr(m2, 1) + col as isize;
                let mut vd = 0.0f64;
                let (mut av, mut bv) = (Vec::new(), Vec::new());
                for i in 0..m1_cols as isize {
                    let a = read_at(g, m1, r1 + i);
                    let b = read_at(g, m2, r2 + i * cols as isize);
                    match acc {
                        Acc::Double => vd = sse::add64(vd, sse::mul64(f64::from(a), f64::from(b))),
                        Acc::Float => {
                            av.push(a);
                            bv.push(b);
                        }
                    }
                }
                let v = match acc {
                    Acc::Double => vd as f32,
                    Acc::Float => dot_float(&av, &bv),
                };
                write_at(g, m3, r3 + col as isize - 1, v);
            }
        }
        return;
    }
    if complex_kind(t1, t2).is_none() {
        return;
    }
    let clear = !g.notices.reproduce();
    if given {
        let nonzero = (1..=rows).any(|r| {
            (1..=cols).any(|c| {
                let i = rptr(m3, r) + 2 * c as isize;
                read_pair(m3, i) != (0.0, 0.0)
            })
        });
        if nonzero {
            let name = acc.name();
            g.notices.add(
                &FS_15,
                if clear {
                    format!(
                        "{name}: a complex product into a given matrix, its earlier contents cleared"
                    )
                } else {
                    format!(
                        "{name}: a complex product added onto the given matrix's earlier contents"
                    )
                },
            );
        }
    }
    for row in 1..=rows {
        for col in 1..=cols {
            let i3 = rptr(m3, row) + 2 * col as isize;
            let (mut re, mut im) = if clear {
                (0.0f32, 0.0f32)
            } else {
                (read_at(g, m3, i3), read_at(g, m3, i3 + 1))
            };
            for i in 1..=m1.cols {
                // a + ib times c + id; b = 0 for a real m1, d = 0 for a real m2.
                let (a, b, c, d) = match (t1, t2) {
                    (MATRIX_COMPLEX, MATRIX_COMPLEX) => {
                        let (a, b) = celt_read(g, m1, row, i);
                        let (c, d) = celt_read(g, m2, i, col);
                        (a, Some(b), c, Some(d))
                    }
                    (MATRIX_REAL, MATRIX_COMPLEX) => {
                        let a = f64::from(read_at(g, m1, rptr(m1, row) + i as isize));
                        let (c, d) = celt_read(g, m2, i, col);
                        (a, None, c, Some(d))
                    }
                    _ => {
                        let (a, b) = celt_read(g, m1, row, i);
                        let c = f64::from(read_at(g, m2, rptr(m2, i) + col as isize));
                        (a, Some(b), c, None)
                    }
                };
                match acc {
                    Acc::Double => {
                        use sse::{add64 as add, mul64 as mul, sub64 as sub};
                        let (dr, di) = match (b, d) {
                            (Some(b), Some(d)) => {
                                (sub(mul(a, c), mul(b, d)), add(mul(a, d), mul(b, c)))
                            }
                            (None, Some(d)) => (mul(a, c), mul(a, d)),
                            (Some(b), None) => (mul(a, c), mul(b, c)),
                            (None, None) => unreachable!(),
                        };
                        re = add(f64::from(re), dr) as f32;
                        im = add(f64::from(im), di) as f32;
                    }
                    Acc::Float => {
                        // the parts were floats: widening them was exact
                        let (a, c) = (a as f32, c as f32);
                        use sse::{add, mul, sub};
                        let (dr, di) = match (b.map(|x| x as f32), d.map(|x| x as f32)) {
                            (Some(b), Some(d)) => {
                                (sub(mul(a, c), mul(b, d)), add(mul(a, d), mul(b, c)))
                            }
                            (None, Some(d)) => (mul(a, c), mul(a, d)),
                            (Some(b), None) => (mul(a, c), mul(b, c)),
                            (None, None) => unreachable!(),
                        };
                        re = add(re, dr);
                        im = add(im, di);
                    }
                }
            }
            write_at(g, m3, i3, re);
            write_at(g, m3, i3 + 1, im);
        }
    }
}

/// Which complex branch applies (`None`: neither type is complex, or a type is unknown).
fn complex_kind(t1: i16, t2: i16) -> Option<()> {
    match (t1, t2) {
        (MATRIX_COMPLEX, MATRIX_COMPLEX)
        | (MATRIX_REAL, MATRIX_COMPLEX)
        | (MATRIX_COMPLEX, MATRIX_REAL) => Some(()),
        _ => None,
    }
}

fn read_pair(m: &Matrix, i: isize) -> (f32, f32) {
    if i < 0 {
        return (0.0, 0.0);
    }
    let i = i as usize;
    (
        m.store.get(i).copied().unwrap_or(0.0),
        m.store.get(i + 1).copied().unwrap_or(0.0),
    )
}

/// `MATRIX_CELT_REAL(m, r, c)` and `MATRIX_CELT_IMAG(m, r, c)` as doubles (FS-16 outside).
pub(crate) fn celt_read(g: &mut Globals, m: &Matrix, r: i32, c: i32) -> (f64, f64) {
    let i = rptr(m, r) + 2 * c as isize;
    (f64::from(read_at(g, m, i)), f64::from(read_at(g, m, i + 1)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use larmorx_core::defects::UpstreamDefects;

    fn g(mode: UpstreamDefects) -> Globals {
        Globals::new(mode)
    }

    #[test]
    fn layout_and_buffers() {
        let m = Matrix::real(3, 4);
        assert_eq!(m.store().len(), 14);
        assert_eq!(m.relt_index(1, 1), Some(2));
        assert_eq!(m.relt_index(1, 0), Some(1));
        assert_eq!(m.relt_index(2, 1), Some(6));
        assert_eq!(m.relt_index(0, 1), None);
        assert_eq!(matrix_alloc_new_size(3, 3, MATRIX_REAL), 192);
        assert_eq!(matrix_alloc_new_size(4, 4, MATRIX_REAL), 256);
        let mut gl = g(UpstreamDefects::Fix);
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let b = matrix_alloc2(&mut gl, &mut o, &mut e, 3, 3, MATRIX_REAL)
            .unwrap()
            .unwrap();
        assert!(b.in_buf);
        let c = matrix_alloc(&mut gl, &mut o, &mut e, 2, 3, MATRIX_COMPLEX)
            .unwrap()
            .unwrap();
        assert_eq!(c.store().len(), 16);
        assert_eq!(c.celt_index(1, 1), Some(2));
        assert_eq!(c.celt_index(2, 1), Some(8));
        gl.matrix_alloc_old = true;
        let c = matrix_alloc2(&mut gl, &mut o, &mut e, 2, 3, MATRIX_COMPLEX)
            .unwrap()
            .unwrap();
        assert_eq!(c.store().len(), 14);
        assert!(!c.in_buf);
    }

    #[test]
    fn alloc_failures_and_unknown_types() {
        let mut gl = g(UpstreamDefects::Fix);
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let r = matrix_alloc(&mut gl, &mut o, &mut e, -2, 3, MATRIX_REAL);
        assert_eq!(r, Err(Fatal::Exit(1)));
        assert_eq!(o, b"MatrixAlloc(-2, 3): Using mmap'd tmpfile\n");
        assert_eq!(e, b"MatrixAlloc(-2, 3): allocation failed\n");
        assert!(gl.notices.contains(&FS_13));
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let r = matrix_alloc(&mut gl, &mut o, &mut e, 2, 2, 7).unwrap();
        assert!(r.is_none());
        assert_eq!(e, b"error: MatrixAlloc: unknown type 7\n\n");
        let r = matrix_alloc(&mut gl, &mut o, &mut e, 0, 2, 7).unwrap();
        assert_eq!(r.unwrap().rows, 0);
    }

    #[test]
    fn multiply_d_accumulates_in_double() {
        let mut gl = g(UpstreamDefects::Fix);
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let a = Matrix::from_rows(&[vec![1.0, 1e-8], vec![3.0, 4.0]]);
        let b = Matrix::from_rows(&[vec![1.0, 2.0], vec![1.0, 0.5]]);
        let c = matrix_multiply_d(&mut gl, &mut o, &mut e, Some(&a), Some(&b), None)
            .unwrap()
            .unwrap_new();
        let want = (1.0f64 + f64::from(1e-8f32)) as f32;
        assert_eq!(c.relt(1, 1).to_bits(), want.to_bits());
        assert_eq!(c.relt(2, 2), 8.0);
        let mut bad = Matrix::real(3, 3);
        let r =
            matrix_multiply_d(&mut gl, &mut o, &mut e, Some(&a), Some(&b), Some(&mut bad)).unwrap();
        assert!(r.is_null());
        assert!(String::from_utf8_lossy(&o).contains(":623\n"));
        let r = matrix_multiply_d(&mut gl, &mut o, &mut e, None, Some(&b), None);
        assert_eq!(r, Err(Fatal::Exit(-4)));
    }

    #[test]
    fn multiply_float_messages_and_sums() {
        let mut gl = g(UpstreamDefects::Fix);
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let a = Matrix::from_rows(&[vec![1.0, 1e-8], vec![3.0, 4.0]]);
        let b = Matrix::from_rows(&[vec![1.0, 2.0], vec![1.0, 0.5]]);
        let c = matrix_multiply(&mut gl, &mut o, &mut e, Some(&a), Some(&b), None)
            .unwrap()
            .unwrap_new();
        // float: 1 + 1e-8 rounds to 1
        assert_eq!(c.relt(1, 1), 1.0);
        let wide = Matrix::real(3, 1);
        let r = matrix_multiply(&mut gl, &mut o, &mut e, Some(&a), Some(&wide), None).unwrap();
        assert!(r.is_null());
        let file = matrix_cpp();
        assert_eq!(
            String::from_utf8_lossy(&o),
            format!("MatrixMultiply(): m1/m2 dim mismatch\n break {file}:733\n")
        );
        assert_eq!(
            String::from_utf8_lossy(&e),
            "error: MatrixMultiply: m1 cols 2 does not match m2 rows 3\n\n"
        );
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let mut bad = Matrix::real(3, 3);
        let r =
            matrix_multiply(&mut gl, &mut o, &mut e, Some(&a), Some(&b), Some(&mut bad)).unwrap();
        assert!(r.is_null());
        assert!(String::from_utf8_lossy(&o).ends_with(":748\n"));
        assert!(
            String::from_utf8_lossy(&e).contains("MatrixMultiply: (2 x 2) * (2 x 2) != (3 x 3)")
        );
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let r = matrix_multiply(&mut gl, &mut o, &mut e, Some(&a), None, None);
        assert_eq!(r, Err(Fatal::Exit(-4)));
        assert_eq!(
            String::from_utf8_lossy(&o),
            format!("MatrixMultiply(): m2 is null \n break {file}:729\n")
        );
    }

    #[test]
    fn complex_copy_and_product_fs14_fs15() {
        for mode in [UpstreamDefects::Fix, UpstreamDefects::Reproduce] {
            let mut gl = g(mode);
            let (mut o, mut e) = (Vec::new(), Vec::new());
            let mut a = matrix_alloc(&mut gl, &mut o, &mut e, 2, 2, MATRIX_COMPLEX)
                .unwrap()
                .unwrap();
            for r in 1..=2 {
                for c in 1..=2 {
                    a.set_celt(r, c, (r * 10 + c) as f32, -(c as f32));
                }
            }
            let copy = matrix_copy(&mut gl, &mut o, &mut e, Some(&a), None)
                .unwrap()
                .unwrap_new();
            assert!(gl.notices.contains(&FS_14));
            if mode.reproduce() {
                assert_ne!(copy, a);
            } else {
                assert_eq!(copy.data(), a.data());
            }
            let mut m3 = a.clone();
            let r = matrix_multiply_d(&mut gl, &mut o, &mut e, Some(&a), Some(&a), Some(&mut m3))
                .unwrap();
            assert_eq!(r, Ret::Given);
            assert!(gl.notices.contains(&FS_15));
            let fresh = matrix_multiply_d(&mut gl, &mut o, &mut e, Some(&a), Some(&a), None)
                .unwrap()
                .unwrap_new();
            if mode.reproduce() {
                assert_ne!(m3.data(), fresh.data());
            } else {
                assert_eq!(m3.data(), fresh.data());
            }
        }
    }
}
