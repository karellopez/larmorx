// SPDX-License-Identifier: LicenseRef-FreeSurfer
// Part of larmorx-freesurfer (FreeSurfer's licence; see crates-freesurfer/LICENSE). Copyright
// 2026 Karel Lopez Vilaret.
//! FreeSurfer's double matrices (`fs.maths`, queue item 3.1-23), translated from FreeSurfer
//! 8.2.0's `include/dmatrix.h` (`DMATRIX`, `DVECTOR`) and `utils/dmatrix.cpp`:
//! `DMatrixCheckDims`, `DMatrixAlloc`, `DMatrixFree`, `DMatrixPrintFmt`, `DMatrixPrint`,
//! `DMatrixCompareFMatrix`, `DMatrixCopyFMatrix`, `DMatrixCopy`, `DMatrixTranspose`,
//! `DMatrixAddMul`, `DMatrixScalarMul`, `DMatrixSubtract`, `DMatrixAdd`, `DMatrixConstVal`,
//! `DMatrixZero`, `DMatrixMaxAbs`, and `DMatrixMultiply` and `DVectorDot`, which accumulate in
//! x87 `long double` (`larmorx_core::x87::F80`, queue item 3.1-03: the official build's x87
//! arithmetic at its default 64-bit precision). Not translated (no program reaches them):
//! `DVectorLen` (an OpenMP reduction whose order depends on the threads), `DVectorCrossProduct`,
//! `DMatrixDRand48`, `DMatrixTest` (a self-test).
//!
//! A `DMATRIX` has the layout of a real `MATRIX` ([`crate::matrix`]) with `double` numbers: one
//! array, two unused numbers before the first, rows one-based. `DMatrixAddMul`'s OpenMP loop
//! over the rows computes each element alone, so its result does not depend on the threads.
//!
//! Messages: FreeSurfer's, including `DMatrixAlloc`'s `ERROR: DMatrixAlloc(): types 1 not
//! recognized` for any type other than real and complex (it prints `MATRIX_REAL`, not the type
//! it was given: kept, a message only).

use std::io::Write;

use larmorx_core::x87::F80;

use crate::cstd::format_double;
use crate::diag::BUILD_ROOT;
use crate::globals::Globals;
use crate::log::{self, ERROR_BADPARM, ERROR_NO_MEMORY, Fatal, NO_ERROR};
use crate::matrix::{MATRIX_COMPLEX, MATRIX_REAL, Matrix};
use crate::ret::Ret;

/// `utils/dmatrix.cpp`'s path in the official build (`__FILE__`).
pub fn dmatrix_cpp() -> String {
    format!("{BUILD_ROOT}/utils/dmatrix.cpp")
}

/// A FreeSurfer `DMATRIX` (`DVECTOR`): see the module's documentation.
#[derive(Clone, Debug, PartialEq)]
pub struct DMatrix {
    /// `type`: always [`MATRIX_REAL`] (`DMatrixAlloc` makes no other).
    pub mtype: i16,
    pub rows: i32,
    pub cols: i32,
    /// The allocation: two unused numbers, then the numbers row by row.
    store: Vec<f64>,
}

impl DMatrix {
    /// A matrix of zeros of valid sizes (panics otherwise).
    pub fn zeros(rows: i32, cols: i32) -> DMatrix {
        assert!(rows >= 0 && cols >= 0, "DMATRIX of {rows} x {cols}");
        DMatrix {
            mtype: MATRIX_REAL,
            rows,
            cols,
            store: vec![0.0; rows as usize * cols as usize + 2],
        }
    }

    /// From rows (`m[r - 1][c - 1]` is element `(r, c)`).
    pub fn from_rows(m: &[Vec<f64>]) -> DMatrix {
        let rows = m.len() as i32;
        let cols = m.first().map_or(0, |r| r.len()) as i32;
        let mut d = DMatrix::zeros(rows, cols);
        for (r, row) in m.iter().enumerate() {
            assert_eq!(row.len() as i32, cols, "rows of different lengths");
            for (c, &v) in row.iter().enumerate() {
                d.set(r as i32 + 1, c as i32 + 1, v);
            }
        }
        d
    }

    fn index(&self, r: i32, c: i32) -> usize {
        assert!(r >= 1 && r <= self.rows, "DMATRIX row {r} of {}", self.rows);
        let i = 1 + (r as isize - 1) * self.cols as isize + c as isize;
        assert!(
            i >= 0 && (i as usize) < self.store.len(),
            "DMATRIX element outside"
        );
        i as usize
    }

    /// `m->rptr[r][c]`.
    pub fn get(&self, r: i32, c: i32) -> f64 {
        self.store[self.index(r, c)]
    }

    /// `m->rptr[r][c] = v`.
    pub fn set(&mut self, r: i32, c: i32, v: f64) {
        let i = self.index(r, c);
        self.store[i] = v;
    }

    /// `m->data[0 .. rows * cols]`.
    pub fn data(&self) -> &[f64] {
        let n = self.rows.max(0) as usize * self.cols.max(0) as usize;
        &self.store[2..2 + n]
    }

    /// `m->data[0 .. rows * cols]`, to write.
    pub fn data_mut(&mut self) -> &mut [f64] {
        let n = self.rows.max(0) as usize * self.cols.max(0) as usize;
        &mut self.store[2..2 + n]
    }
}

/// `DMatrixCheckDims(m1, m2, checktype, fp, str)`: 0 when the sizes agree as `checktype` asks
/// (1 rows, 2 columns, 3 both, 4 `m1`'s columns against `m2`'s rows), else 1 after
/// `ERROR: <str> ...` and `break <file>:<line>` on `fp`; 1 for a `NULL` matrix.
pub fn dmatrix_check_dims(
    m1: Option<&DMatrix>,
    m2: Option<&DMatrix>,
    checktype: i32,
    fp: &mut dyn Write,
    s: &str,
) -> i32 {
    let file = dmatrix_cpp();
    let Some(m1) = m1 else {
        let _ = write!(fp, "ERROR: {s} matrix 1 is NULL\nbreak {file}:309\n");
        return 1;
    };
    let Some(m2) = m2 else {
        let _ = write!(fp, "ERROR: {s} matrix 2 is NULL\nbreak {file}:314\n");
        return 1;
    };
    if (checktype == 1 || checktype == 3) && m1.rows != m2.rows {
        let _ = write!(
            fp,
            "ERROR: {s} rows mismatch {}, {}\nbreak {file}:319\n",
            m1.rows, m2.rows
        );
        return 1;
    }
    if (checktype == 2 || checktype == 3) && m1.cols != m2.cols {
        let _ = write!(
            fp,
            "ERROR: {s} cols mismatch {}, {}\nbreak {file}:324\n",
            m1.cols, m2.cols
        );
        return 1;
    }
    if checktype == 4 && m1.cols != m2.rows {
        let _ = write!(
            fp,
            "ERROR: {s} cols of m1 mismatch rows of m2 {}, {}\nbreak {file}:329\n",
            m1.cols, m2.rows
        );
        return 1;
    }
    0
}

/// `DMatrixAlloc(rows, cols, type)`: a matrix of zeros; `None` (`NULL`) after `ERROR:
/// DMatrixAlloc(): ...` on standard output for a complex or unknown type and for an element
/// count that cannot be allocated (negative, or beyond `int`); the end of the program when the
/// row pointers cannot be allocated (fewer than -1 rows, with a count that can).
pub fn dmatrix_alloc(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    rows: i32,
    cols: i32,
    mtype: i16,
) -> Result<Option<DMatrix>, Fatal> {
    if mtype == MATRIX_COMPLEX {
        let _ = writeln!(out, "ERROR: DMatrixAlloc(): complex types not supported");
        return Ok(None);
    }
    if mtype != MATRIX_REAL {
        let _ = writeln!(
            out,
            "ERROR: DMatrixAlloc(): types {MATRIX_REAL} not recognized"
        );
        return Ok(None);
    }
    let nelts = i64::from(rows) * i64::from(cols);
    let mut store = Vec::new();
    let ok = nelts >= i64::from(i32::MIN)
        && nelts <= i64::from(i32::MAX)
        && nelts + 2 >= 0
        && store.try_reserve_exact((nelts + 2) as usize).is_ok();
    if !ok {
        let _ = writeln!(
            out,
            "ERROR: DMatrixAlloc({rows}, {cols}): allocation failed"
        );
        return Ok(None);
    }
    if rows < -1 {
        log::error_exit(
            g,
            err,
            ERROR_NO_MEMORY,
            &format!("DMatrixAlloc({rows}, {cols}): could not allocate rptr"),
        )?;
        return Ok(None);
    }
    store.resize((nelts + 2) as usize, 0.0);
    Ok(Some(DMatrix {
        mtype,
        rows,
        cols,
        store,
    }))
}

/// `DMatrixFree(&m)`: 0.
pub fn dmatrix_free(m: &mut Option<DMatrix>) -> i32 {
    *m = None;
    0
}

/// `DMatrixMultiply(m1, m2, m3)`: `m1 m2`, each element the x87 `long double` sum (from 0, in
/// order) of the `double` products `m1(r, i) m2(i, c)`, stored as a `double`. `Null` after
/// [`dmatrix_check_dims`]'s message (kind 4, standard output) for `NULL` inputs or a size
/// mismatch, or after `DMatrixMultiply(): m1/m2 dim mismatch` (with the file and line 429) on
/// standard output and `DMatrixMultiply: (r1 x c1) * (r2 x c2) != (r3 x c3)` as an error for a
/// given `m3` of another size. To multiply into an input, pass a copy of it as the input
/// (FreeSurfer does not copy it: its loop would read the elements it has already written; no
/// caller does that).
pub fn dmatrix_multiply(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    m1: Option<&DMatrix>,
    m2: Option<&DMatrix>,
    m3: Option<&mut DMatrix>,
) -> Result<Ret<DMatrix>, Fatal> {
    if dmatrix_check_dims(m1, m2, 4, out, "DMatrixMultiply(): ") != 0 {
        return Ok(Ret::Null);
    }
    let (m1, m2) = (m1.expect("checked"), m2.expect("checked"));
    let mut fresh = None;
    let m3: &mut DMatrix = match m3 {
        None => match dmatrix_alloc(g, out, err, m1.rows, m2.cols, m1.mtype)? {
            Some(m) => fresh.insert(m),
            None => return Ok(Ret::Null),
        },
        Some(m3) => {
            if m3.rows != m1.rows || m3.cols != m2.cols {
                let _ = write!(
                    out,
                    "DMatrixMultiply(): m1/m2 dim mismatch\n break {}:429\n",
                    dmatrix_cpp()
                );
                log::error_printf(
                    g,
                    err,
                    ERROR_BADPARM,
                    &format!(
                        "DMatrixMultiply: ({} x {}) * ({} x {}) != ({} x {})\n",
                        m1.rows, m1.cols, m2.rows, m2.cols, m3.rows, m3.cols
                    ),
                );
                return Ok(Ret::Null);
            }
            m3
        }
    };
    for row in 1..=m3.rows {
        for col in 1..=m3.cols {
            let mut val = F80::ZERO;
            for i in 1..=m1.cols {
                val += F80::from_f64(m1.get(row, i) * m2.get(i, col));
            }
            m3.set(row, col, val.to_f64());
        }
    }
    Ok(match fresh {
        Some(m) => Ret::New(m),
        None => Ret::Given,
    })
}

/// `DVectorDot(v1, v2)`: the x87 `long double` sum of the `long double` products of the first
/// columns' elements, as a `double`; `-10e10` after [`dmatrix_check_dims`]'s message (kind 3,
/// standard output, prefix `DDVectorDot(): `) for `NULL` vectors or a size mismatch.
pub fn dvector_dot(out: &mut dyn Write, v1: Option<&DMatrix>, v2: Option<&DMatrix>) -> f64 {
    if dmatrix_check_dims(v1, v2, 3, out, "DDVectorDot(): ") != 0 {
        return -10e10;
    }
    let (v1, v2) = (v1.expect("checked"), v2.expect("checked"));
    let mut dot = F80::ZERO;
    for i in 1..=v1.rows {
        dot += F80::from_f64(v1.get(i, 1)) * F80::from_f64(v2.get(i, 1));
    }
    dot.to_f64()
}

/// `DMatrixPrintFmt(fp, fmt, mat)`: each row's elements in `fmt` (a `%f` format), two spaces
/// between them, a newline after each row; `ERROR_BADPARM` after `DMatrixPrint: mat = NULL!` as
/// an error for no matrix; `NO_ERROR`. Panics on a format other than `%f`'s (no FreeSurfer
/// caller passes one).
pub fn dmatrix_print_fmt(
    g: &mut Globals,
    err: &mut dyn Write,
    fp: &mut dyn Write,
    fmt: &str,
    mat: Option<&DMatrix>,
) -> i32 {
    let Some(mat) = mat else {
        log::error_printf(g, err, ERROR_BADPARM, "DMatrixPrint: mat = NULL!");
        return ERROR_BADPARM;
    };
    let mut text = String::new();
    for row in 1..=mat.rows {
        for col in 1..=mat.cols {
            text.push_str(
                &format_double(fmt, mat.get(row, col))
                    .unwrap_or_else(|| panic!("DMatrixPrintFmt: format {fmt:?} not translated")),
            );
            if col < mat.cols {
                text.push_str("  ");
            }
        }
        text.push('\n');
    }
    let _ = fp.write_all(text.as_bytes());
    let _ = fp.flush();
    NO_ERROR
}

/// `DMatrixPrint(fp, mat)`: [`dmatrix_print_fmt`] with `%8.5lf`.
pub fn dmatrix_print(
    g: &mut Globals,
    err: &mut dyn Write,
    fp: &mut dyn Write,
    mat: Option<&DMatrix>,
) -> i32 {
    dmatrix_print_fmt(g, err, fp, "%8.5lf", mat)
}

/// `DMatrixCompareFMatrix(mf, md)`: the largest `|mf - md|` (the `float` promoted); -1 after
/// `ERROR: DMatrixCompareFMatrix(): ...` on standard output for a `NULL` or a size mismatch.
pub fn dmatrix_compare_fmatrix(
    out: &mut dyn Write,
    mf: Option<&Matrix>,
    md: Option<&DMatrix>,
) -> f64 {
    let Some(mf) = mf else {
        let _ = writeln!(out, "ERROR: DMatrixCompareFMatrix(): mf is NULL");
        return -1.0;
    };
    let Some(md) = md else {
        let _ = writeln!(out, "ERROR: DMatrixCompareFMatrix(): md is NULL");
        return -1.0;
    };
    if mf.rows != md.rows {
        let _ = writeln!(
            out,
            "ERROR: DMatrixCompareFMatrix(): row mismatch {} {}",
            mf.rows, md.rows
        );
        return -1.0;
    }
    if mf.cols != md.cols {
        let _ = writeln!(
            out,
            "ERROR: DMatrixCompareFMatrix(): col mismatch {} {}",
            mf.cols, md.cols
        );
        return -1.0;
    }
    let mut dmax = 0.0f64;
    for r in 1..=mf.rows {
        for c in 1..=mf.cols {
            let d = (f64::from(mf.relt(r, c)) - md.get(r, c)).abs();
            if dmax < d {
                dmax = d;
            }
        }
    }
    dmax
}

/// `DMatrixCopyFMatrix(mf, md)`: the float matrix's values into a double one (a new one when
/// there is none); `Null` after `ERROR: DMatrixCopyFMatrix(): ...` on standard output for no
/// `mf` or a size mismatch.
pub fn dmatrix_copy_fmatrix(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mf: Option<&Matrix>,
    md: Option<&mut DMatrix>,
) -> Result<Ret<DMatrix>, Fatal> {
    let Some(mf) = mf else {
        let _ = writeln!(out, "ERROR: DMatrixCopyFMatrix(): mf is NULL");
        return Ok(Ret::Null);
    };
    let mut fresh = None;
    let md: &mut DMatrix = match md {
        Some(m) => m,
        None => match dmatrix_alloc(g, out, err, mf.rows, mf.cols, MATRIX_REAL)? {
            Some(m) => fresh.insert(m),
            // FreeSurfer reads the rows of the NULL it got: a crash; the translation stops here.
            None => return Ok(Ret::Null),
        },
    };
    if mf.rows != md.rows {
        let _ = writeln!(
            out,
            "ERROR: DMatrixCopyFMatrix(): row mismatch {} {}",
            mf.rows, md.rows
        );
        return Ok(Ret::Null);
    }
    if mf.cols != md.cols {
        let _ = writeln!(
            out,
            "ERROR: DMatrixCopyFMatrix(): col mismatch {} {}",
            mf.cols, md.cols
        );
        return Ok(Ret::Null);
    }
    for r in 1..=mf.rows {
        for c in 1..=mf.cols {
            md.set(r, c, f64::from(mf.relt(r, c)));
        }
    }
    Ok(match fresh {
        Some(m) => Ret::New(m),
        None => Ret::Given,
    })
}

/// `DMatrixCopy(msrc, mcopy)`: `msrc` into `mcopy` (a new one when there is none); `Null` after
/// `ERROR: DMatrixCopy(): msrc is NULL` or [`dmatrix_check_dims`]'s message on standard output.
pub fn dmatrix_copy(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    msrc: Option<&DMatrix>,
    mcopy: Option<&mut DMatrix>,
) -> Result<Ret<DMatrix>, Fatal> {
    let Some(msrc) = msrc else {
        let _ = writeln!(out, "ERROR: DMatrixCopy(): msrc is NULL");
        return Ok(Ret::Null);
    };
    let mut fresh: Option<DMatrix> = None;
    let given = mcopy.is_some();
    let mut slot = match mcopy {
        Some(m) => Some(m),
        None => {
            fresh = dmatrix_alloc(g, out, err, msrc.rows, msrc.cols, MATRIX_REAL)?;
            fresh.as_mut()
        }
    };
    let dims = dmatrix_check_dims(Some(msrc), slot.as_deref(), 3, out, "DMatrixCopy(): ");
    if dims != 0 {
        return Ok(Ret::Null);
    }
    let dst = slot.as_mut().expect("checked");
    for r in 1..=msrc.rows {
        for c in 1..=msrc.cols {
            dst.set(r, c, msrc.get(r, c));
        }
    }
    Ok(if given {
        Ret::Given
    } else {
        Ret::New(fresh.expect("allocated"))
    })
}

/// `DMatrixTranspose(mIn, mOut)` into another matrix or a new one; `Null` after
/// `DMatrixTranspose(): ERROR: row/col mismatch ...` (or `col/row`) on standard output for an
/// output of the wrong shape.
pub fn dmatrix_transpose(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    m_in: &DMatrix,
    m_out: Option<&mut DMatrix>,
) -> Result<Ret<DMatrix>, Fatal> {
    let mut fresh = None;
    let m_out: &mut DMatrix = match m_out {
        Some(m) => m,
        None => match dmatrix_alloc(g, out, err, m_in.cols, m_in.rows, m_in.mtype)? {
            Some(m) => fresh.insert(m),
            None => return Ok(Ret::Null),
        },
    };
    if m_in.rows != m_out.cols {
        let _ = writeln!(
            out,
            "DMatrixTranspose(): ERROR: row/col mismatch {} {}",
            m_in.rows, m_out.cols
        );
        return Ok(Ret::Null);
    }
    if m_in.cols != m_out.rows {
        let _ = writeln!(
            out,
            "DMatrixTranspose(): ERROR: col/row mismatch {} {}",
            m_in.cols, m_out.rows
        );
        return Ok(Ret::Null);
    }
    for row in 1..=m_in.rows {
        for col in 1..=m_in.cols {
            m_out.set(col, row, m_in.get(row, col));
        }
    }
    Ok(match fresh {
        Some(m) => Ret::New(m),
        None => Ret::Given,
    })
}

/// `DMatrixTranspose(m, m)`: `DMatrixTranspose(): ERROR: cannot be done in-place` on standard
/// output; `NULL`.
pub fn dmatrix_transpose_in_place(out: &mut dyn Write) -> Ret<DMatrix> {
    let _ = writeln!(out, "DMatrixTranspose(): ERROR: cannot be done in-place");
    Ret::Null
}

/// `DMatrixAddMul(m1, m2, v1, v2, mOut)`: `v1 m1 + v2 m2` element by element in `double` (a
/// factor of 0 skips its matrix, a factor of 1 adds it unscaled; each element starts from 0, so
/// a lone -0 becomes +0); `m2` may be `None`. `Null` after [`dmatrix_check_dims`]'s message on
/// standard output for a size mismatch. To write into an input, pass a copy of it as the input.
#[allow(clippy::too_many_arguments)]
pub fn dmatrix_add_mul(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    m1: &DMatrix,
    m2: Option<&DMatrix>,
    v1: f64,
    v2: f64,
    m_out: Option<&mut DMatrix>,
) -> Result<Ret<DMatrix>, Fatal> {
    if let Some(m2) = m2
        && dmatrix_check_dims(Some(m1), Some(m2), 3, out, "DMatrixAddMul(): ") != 0
    {
        return Ok(Ret::Null);
    }
    let mut fresh = None;
    let m_out: &mut DMatrix = match m_out {
        Some(m) => m,
        None => match dmatrix_alloc(g, out, err, m1.rows, m1.cols, m1.mtype)? {
            Some(m) => fresh.insert(m),
            None => return Ok(Ret::Null),
        },
    };
    if dmatrix_check_dims(Some(m1), Some(m_out), 3, out, "DMatrixAddMul(): ") != 0 {
        return Ok(Ret::Null);
    }
    for row in 1..=m1.rows {
        for col in 1..=m1.cols {
            let mut val = 0.0f64;
            if v1 != 0.0 {
                if v1 != 1.0 {
                    val += v1 * m1.get(row, col);
                } else {
                    val += m1.get(row, col);
                }
            }
            if let Some(m2) = m2
                && v2 != 0.0
            {
                if v2 != 1.0 {
                    val += v2 * m2.get(row, col);
                } else {
                    val += m2.get(row, col);
                }
            }
            m_out.set(row, col, val);
        }
    }
    Ok(match fresh {
        Some(m) => Ret::New(m),
        None => Ret::Given,
    })
}

/// `DMatrixScalarMul(m, v, mout)`: [`dmatrix_add_mul`]`(m, None, v, 0, mout)`.
pub fn dmatrix_scalar_mul(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    m: &DMatrix,
    v: f64,
    m_out: Option<&mut DMatrix>,
) -> Result<Ret<DMatrix>, Fatal> {
    dmatrix_add_mul(g, out, err, m, None, v, 0.0, m_out)
}

/// `DMatrixSubtract(m1, m2, mout)`: [`dmatrix_add_mul`]`(m1, m2, 1, -1, mout)`.
pub fn dmatrix_subtract(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    m1: &DMatrix,
    m2: &DMatrix,
    m_out: Option<&mut DMatrix>,
) -> Result<Ret<DMatrix>, Fatal> {
    dmatrix_add_mul(g, out, err, m1, Some(m2), 1.0, -1.0, m_out)
}

/// `DMatrixAdd(m1, m2, mout)`: [`dmatrix_add_mul`]`(m1, m2, 1, 1, mout)`.
pub fn dmatrix_add(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    m1: &DMatrix,
    m2: &DMatrix,
    m_out: Option<&mut DMatrix>,
) -> Result<Ret<DMatrix>, Fatal> {
    dmatrix_add_mul(g, out, err, m1, Some(m2), 1.0, 1.0, m_out)
}

/// `DMatrixConstVal(val, rows, cols, X)`: every element `val` (a new `rows` x `cols` matrix
/// when there is none; a given one keeps its size); `val` 0 is [`dmatrix_zero`].
pub fn dmatrix_const_val(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    val: f64,
    rows: i32,
    cols: i32,
    x: Option<&mut DMatrix>,
) -> Result<Ret<DMatrix>, Fatal> {
    if val == 0.0 {
        return dmatrix_zero(g, out, err, rows, cols, x);
    }
    let mut fresh = None;
    let x: &mut DMatrix = match x {
        Some(m) => m,
        // FreeSurfer reads the size of the NULL it may get: a crash; the translation stops.
        None => match dmatrix_alloc(g, out, err, rows, cols, MATRIX_REAL)? {
            Some(m) => fresh.insert(m),
            None => return Ok(Ret::Null),
        },
    };
    x.data_mut().fill(val);
    Ok(match fresh {
        Some(m) => Ret::New(m),
        None => Ret::Given,
    })
}

/// `DMatrixZero(rows, cols, X)`: a new matrix of zeros, or `X`'s numbers set to 0.
pub fn dmatrix_zero(
    g: &mut Globals,
    out: &mut dyn Write,
    err: &mut dyn Write,
    rows: i32,
    cols: i32,
    x: Option<&mut DMatrix>,
) -> Result<Ret<DMatrix>, Fatal> {
    match x {
        None => Ok(match dmatrix_alloc(g, out, err, rows, cols, MATRIX_REAL)? {
            Some(m) => Ret::New(m),
            None => Ret::Null,
        }),
        Some(m) => {
            m.data_mut().fill(0.0);
            Ok(Ret::Given)
        }
    }
}

/// `DMatrixMaxAbs(M)`: the largest absolute value (0 for an empty matrix); -1 after `ERROR:
/// DMatrixMaxAbs(): matrix is NULL` on standard output for none.
pub fn dmatrix_max_abs(out: &mut dyn Write, m: Option<&DMatrix>) -> f64 {
    let Some(m) = m else {
        let _ = writeln!(out, "ERROR: DMatrixMaxAbs(): matrix is NULL");
        return -1.0;
    };
    let mut vmax = 0.0f64;
    for r in 1..=m.rows {
        for c in 1..=m.cols {
            if vmax < m.get(r, c).abs() {
                vmax = m.get(r, c).abs();
            }
        }
    }
    vmax
}

#[cfg(test)]
mod tests {
    use super::*;
    use larmorx_core::defects::UpstreamDefects;

    #[test]
    fn alloc_messages() {
        let mut g = Globals::new(UpstreamDefects::Fix);
        let (mut o, mut e) = (Vec::new(), Vec::new());
        assert!(
            dmatrix_alloc(&mut g, &mut o, &mut e, 2, 2, MATRIX_COMPLEX)
                .unwrap()
                .is_none()
        );
        assert!(
            dmatrix_alloc(&mut g, &mut o, &mut e, 2, 2, 5)
                .unwrap()
                .is_none()
        );
        assert!(
            dmatrix_alloc(&mut g, &mut o, &mut e, -3, 2, MATRIX_REAL)
                .unwrap()
                .is_none()
        );
        assert_eq!(
            String::from_utf8(o).unwrap(),
            "ERROR: DMatrixAlloc(): complex types not supported\n\
             ERROR: DMatrixAlloc(): types 1 not recognized\n\
             ERROR: DMatrixAlloc(-3, 2): allocation failed\n"
        );
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let m = dmatrix_alloc(&mut g, &mut o, &mut e, -1, 1, MATRIX_REAL)
            .unwrap()
            .unwrap();
        assert_eq!(m.rows, -1);
        let r = dmatrix_alloc(&mut g, &mut o, &mut e, -2, -1, MATRIX_REAL);
        assert_eq!(r, Err(Fatal::Exit(-2)));
    }

    #[test]
    fn add_mul_print_transpose() {
        let mut g = Globals::new(UpstreamDefects::Fix);
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let a = DMatrix::from_rows(&[vec![-0.0, 2.0], vec![3.0, 4.5]]);
        let b = DMatrix::from_rows(&[vec![1.0, 1.0], vec![1.0, 1.0]]);
        let s = dmatrix_scalar_mul(&mut g, &mut o, &mut e, &a, 1.0, None)
            .unwrap()
            .unwrap_new();
        assert!(s.get(1, 1).is_sign_positive());
        let d = dmatrix_subtract(&mut g, &mut o, &mut e, &a, &b, None)
            .unwrap()
            .unwrap_new();
        assert_eq!(d.data(), &[-1.0, 1.0, 2.0, 3.5]);
        let mut p = Vec::new();
        dmatrix_print(&mut g, &mut e, &mut p, Some(&a));
        assert_eq!(p, b"-0.00000   2.00000\n 3.00000   4.50000\n");
        let t = dmatrix_transpose(&mut g, &mut o, &mut e, &a, None)
            .unwrap()
            .unwrap_new();
        assert_eq!(t.get(1, 2), 3.0);
        let mut wrong = DMatrix::zeros(3, 2);
        let r = dmatrix_add(&mut g, &mut o, &mut e, &a, &b, Some(&mut wrong)).unwrap();
        assert!(r.is_null());
        assert!(
            String::from_utf8_lossy(&o)
                .contains("ERROR: DMatrixAddMul():  rows mismatch 2, 3\nbreak ")
        );
    }
}
