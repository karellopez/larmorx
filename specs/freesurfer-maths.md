# FreeSurfer 8.2.0's mathematics: behaviour spec (`fs.maths`)

Written by the replica author of queue items 3.1-23 and 3.1-24 as their spec writer
(`docs/porting-guide.md` §1): behaviour, maths, conventions, messages and the validation plan, from
FreeSurfer 8.2.0's source and the official build's outputs, never code or routines. Made ready for
the clean room by the spec finisher (FreeSurfer's foundations, track E, relay 2, 2026-11-13): the
black-box validation plan (§23) with its open probe, a read-through as a clean-room reader, the gaps
filled from the source as behaviour (§0, §3, §5.1, §18.2, §19), the defects' texts in
`larmorx_core::defects` checked. Amended by its relay 4 (2026-11-13), from the clean room's
black-box runs: §17.1, §17.2 and §17.4 (which matrices with infinite or NaN entries make FreeSurfer
hang and which it finishes, with their results under `reproduce`), §18.2 (the bytes of a file cut
short under `reproduce`), §8.8 (the ports' NaNs). What black-box runs confirmed is marked
**(observed)**, with the probe command or the comparison group behind it.

**Whoever implements it must not open:** FreeSurfer's source (`<workspace>/tags/freesurfer-8.2.0`,
any other copy, the installed package's scripts, Python and libraries), the replica crates
(`crates-freesurfer/`, among them `larmorx-freesurfer-core`), the documents written from FreeSurfer's
source (`docs/freesurfer/`, `docs/findings/freesurfer-*.md`), the replica's probes and checks
(`validation/probes/fs-*`, among them `validation/probes/fs-maths/` and its `scanf/`,
`validation/probes/x87-extended/`, `validation/investigations/freesurfer-core/`), the unit oracle's
build tree and tools (`<workspace>/oracles/freesurfer-8.2.0-unit/` except running
`probes/fs_maths_probe` and loading `probes/guard_malloc.so` beside it;
`validation/investigations/freesurfer-unit-oracle/`), the replay tool's and the unit oracle's Python
(`validation/src/larmorx_validation/freesurfer/`, `freesurfer_unit.py`: run, never read),
`docs/parts/` and the scratch folders of FreeSurfer's parts. **It may read** this spec,
`specs/freesurfer-core.md` (§1's process state, the error functions, `nint`), `specs/ranlux.md`, the
standards and papers cited (IEEE 754-2008, C11, POSIX's `drand48`, LINPACK's and EISPACK's guides,
Powell 1964 and Brent 1973 for the line minimizations, Nocedal 1980 and Liu and Nocedal 1989 for
L-BFGS; not Numerical Recipes' code, whose licence restricts it: §13 and §14 say what is needed),
the main package's open code and documentation (for example
`larmorx_core::vnl_minimize`, `larmorx_core::vnl_lbfgs`, the VNL ports, `larmorx_core::x87`,
`larmorx_core::math`, `larmorx_core::cscan`, `larmorx_core::cfmt`, `larmorx_core::defects`), the open
probe folder `validation/investigations/freesurfer-maths-probe/` (calls only: its README says how to
run it), and run as black boxes the prebuilt probe `<workspace>/oracles/freesurfer-8.2.0-unit/probes/fs_maths_probe`
(with the guard allocator beside it), the official FreeSurfer 8.2.0 programs and the C library.

Status: complete for the functions of §0 (queue items 3.1-23 and 3.1-24): the float matrices'
storage, allocation, copy and products; the 4 x 4 affine helpers; the double matrices; the
histograms; the element-wise operations, regions, vectors, printing, rotations, Gaussian matrices;
the inverse, determinant and QR factors; primes; the Gaussian draws and the time-of-day seed; the
minimizers; Numerical Recipes' 1-based arrays; the eigensystems, the SVD and what uses them; the
matrices' files; the random-field specifications (their draws come with the volumes).

## 0. The functions covered

FreeSurfer's names, as the probe calls them (§23; README.md of the probe folder lists each command),
and where they are specified:

| Section | FreeSurfer's functions |
|---|---|
| §2, §8.1, §16 | `MatrixAlloc` (and its old allocator, `FREESURFER_MatrixAlloc_old`), `MatrixAlloc2` (a caller's buffer), `MatrixFree`, `MatrixCopy`, `MatrixMultiplyD`, `MatrixMultiply` |
| §3 | `SetAffineVector`, `GetAffineVector`, `GetFloorAffineVector`, `SetAffineMatrix`, `GetAffineMatrix`, `AffineMV`, `AffineMM` |
| §4, §12 | `DMatrixAlloc`, `DMatrixCheckDims`, `DMatrixAddMul`, `DMatrixAdd`, `DMatrixSubtract`, `DMatrixScalarMul`, `DMatrixCopy`, `DMatrixTranspose`, `DMatrixPrint`, `DMatrixPrintFmt`, `DMatrixMultiply`, `DVectorDot`, `DMatrixCompareFMatrix`, `DMatrixCopyFMatrix`, `DMatrixConstVal`, `DMatrixZero`, `DMatrixMaxAbs`, `DListStats` |
| §5, §21 | `HISTOalloc`, `HISTOrealloc`, `HISTOfree`, `HISTOcopy`, `HISTOclear`, `HISTOclearCounts`, `HISTOinit`, `HISTObins`, `HISTOadd`, `HISTOsubtract`, `HISTOlinearScale`, `HISTOmakePDF`, `HISTOsumNorm`, `HISTOcumsum`, `HISTOclearBins`, `HISTOabs`, `HISTOmakeCDF`, `HISTOmakeReverseCDF`, `HISTOsoapBubbleZeros`, `HISTOfillHoles`, `HISTOsmooth`, `HISTOfindLastPeak`, `HISTOfindFirstPeak`, `HISTOfindValley`, `HISTOfindLastPeakInRegion`, `HISTOfindHighestPeakInRegion`, `HISTOfindPreviousValley`, `HISTOfindNextValley`, `HISTOfindNextPeak`, `HISTOfindPreviousPeak`, `HISTOfindStartOfPeak`, `HISTOfindEndOfPeak`, `HISTOfindCurrentPeak`, `HISTOfindBin`, `HISTOgetCount`, `HISTOfindBinWithCount`, `HISTOvalToBin`, `HISTOcount`, `HISTO2DfindBin1`, `HISTO2DfindBin2`, `HISTO2DgetCount`, `HISTOaddSample`, `HISTOtotalInRegion`, `HISTOtotal`, `HISTOclearZeroBin`, `HISTOthreshSum`, `HISTOfindMedian`, `HISTOgetEntropy`, `HISTOcorrelate`, `HISTOfindLinearFit`, `HISTOcomputeFWHM`, `HISTOrobustGaussianFit`, `HISTOplot`, `HISTOwriteTxt` |
| §8.2 | `MatrixIdentity`, `MatrixTranspose`, `MatrixAdd`, `MatrixSubtract`, `MatrixScalarMul`, `MatrixSquareElts`, `MatrixClear`, `MatrixDiag`, `MatrixIsSymmetric`, `MatrixIsZero`, `MatrixCheck`, `MatrixTrace`, `MatrixSumElts`, `MatrixConstVal`, `MatrixZero`, `MatrixClone` (`VectorClone`), `MatrixReshape`, `MatrixNormalizeColScale`, `MatrixNormalizeCol` |
| §8.3 | `MatrixCopyRegion`, `MatrixCopyRealRegion`, `MatrixCopyImagRegion`, `MatrixRealToComplex` |
| §8.4, §8.5 | `MatrixAllocRotation`, `MatrixReallocRotation`, `VectorLen`, `VectorDot`, `VectorDistance`, `VectorCrossProduct`, `VectorTripleProduct`, `Vector3Angle`, `XYZ_NORMALIZED_LOAD`, `XYZApproxAngle_knownLength` |
| §8.6, §8.7 | `MatrixPrint`, `MatrixPrintFmt`, `MatrixSimilarityTransform`, `GaussianMatrix`, `GaussianMatrix2` |
| §8.8 | `MatrixInverse`, `MatrixDeterminant`, `MatrixQRdecomposition` |
| §9, §10 | `GetPrimes`, `GetPrimeFactors`, `GetClosestPrimeFactor`, `GetClosestPrimeFactorLess`, `PDFgaussian`, `PDFtodSeed` |
| §13 | `OpenPowell`, `OpenPowell2` (with the iteration callback `powell_iteration_func`), `OpenDFPMin` |
| §14 | `vector`, `matrix`, `free_vector`, `free_matrix` |
| §17 | `OpenEigenSystem`, `OpenNonSymmetricEigenSystem`, `MatrixEigenSystem`, the eigensystem helper `MatrixCalculateEigenSystemHelper`, `MatrixSVD`, `MatrixSVDEigenValues`, `MatrixConditionNumber`, `MatrixNSConditionNumber`, `MatrixSVDInverse`, `MatrixPseudoInverse`, `MatrixSVDPseudoInverse` |
| §18 | `MatrixReadTxt`, `MatrixWriteTxt`, `MatrixAsciiWriteInto`, `MatrixAsciiReadFrom`, `MatrixWriteInto`, `MatrixReadFrom` |
| §19 | `RFname2Code`, `RFspecInit`, `RFspecSetSeed` |

**Not covered** (no program on FreeSurfer's default paths reaches them; a later item specifies one
if a program does): the other functions these files hold (among them `HISTOcrunch`, `HISTOinvert`,
`HISTOnormalize`, `HISTOcompose`, the other two-dimensional histogram functions, `MatrixSqrt`,
`MatrixToeplitz`, `MatrixCovariance`, `MatrixMahalanobisDistance`, `DMatrixDRand48`, `PDFerlang`,
`PDFloadCDF`), the random fields' draws (`RFsynth`, `RFdrawVal`, with the volumes).

## 1. Conventions

- **Arithmetic.** The official build is x86-64 Linux, GCC 8.4.0 at `-O3`, SSE2 for `float` and
  `double`, no fused multiply-add (the baseline x86-64 instructions have none), no reassociation:
  every expression below is evaluated operation by operation, each result rounded to its type
  (round to nearest, ties to even). "In float" means every operation of that expression is a
  `float` operation; "in double" likewise. A float combined with a double is widened first; an
  integer combined with a float is converted to float (round to nearest) first; an integer divided
  by an integer is C's truncating division.
- **Conversions to integers** (`(int) x` of a float or double) truncate toward zero. Beyond the `int`
  range, or for NaN, C leaves them undefined and x86-64 gives `-2147483648` (**FS-17**). Default:
  saturated to the nearest `int` limit, NaN to 0, FS-17 noted; `reproduce`: `-2147483648`.
  FreeSurfer's rounding helper `nint` (round half away from zero) has its own defect, FS-12
  (`specs/freesurfer-core.md` §11).
- **The C library.** `exp`, `log`, `pow` are larmorx's correctly rounded functions by default and
  glibc 2.35's with `reproduce` (rule 5; glibc's bits: `specs/glibc-dbl.md`). `sqrt`, `floor`,
  `ceil`, `fabs` are exact.
- **Out-of-range addresses.** Where FreeSurfer reads or writes outside an array it allocated, the
  result depends on memory it does not own. The behaviours below say where; there a read gives 0
  and a write is dropped, **in both modes**, with the defect noted.
- **Outputs.** Most functions take an optional output object: given, it is filled; absent, a new one
  is made; the function returns the object filled, or nothing (C's `NULL`) on an error.

## 2. Float matrices

### 2.1 The record

A matrix has a type (1 real, 2 complex), a row and a column count (`int`), and its numbers in one
array, row by row, **one-based** (element (r, c), 1 ≤ r ≤ rows, 1 ≤ c ≤ cols). The array starts
with **two unused floats** (zero), then the numbers; a complex matrix holds each element as two
consecutive floats (real, imaginary). With the array's floats numbered from 0:

- a real matrix's element (r, c) is float `1 + (r - 1) cols + c`;
- a complex matrix's element (r, c) is the pair at floats `2 + 2((r - 1) cols + c - 1)` and the next;
- **addressing a complex matrix as if real** (FreeSurfer's real-element access used on it, as its
  copy does, §2.4) gives float `2 + 2((r - 1) cols - 1) + c`; addressing a real matrix as complex
  gives the pair at float `2 + ((r - 1) cols - 1) + 2c`. Row 0, or a row beyond `rows`, has no
  address.

The array's length: real `rows cols + 2`; complex `2 rows cols + 4` with the default allocator,
`2 rows cols + 2` with the old one (§2.2).

### 2.2 Allocation

A new matrix is all zeros. Two allocators give the same numbers: the default one, and the **old**
one when the environment variable `FREESURFER_MatrixAlloc_old` is set (any value) at the program's
first allocation (the answer is kept for the run). The second allocation form takes a caller's
small buffer: the matrix is "in the buffer" when the default allocator's block fits 192 bytes,
the block being, each part rounded up to 64 bytes, a 40-byte header, `(rows + 1)` 8-byte row
pointers, and `(rows cols + 2)` floats (twice that for complex): 3 x 3 and 3 x 4 fit, 4 x 4 does
not. Nothing is seen of it beyond that flag (a matrix in a buffer is never freed); the old
allocator never uses the buffer.

- **Unknown type** (neither 1 nor 2) with at least one row: an error (§2 of the core spec), code
  -4, `MatrixAlloc: unknown type <type>` + `\n`; nothing returned. With 0 rows the matrix is made.
- **FS-13.** A negative row or column count, or more numbers (with the two unused floats) than an
  `int` holds: FreeSurfer prints `MatrixAlloc(<rows>, <cols>): Using mmap'd tmpfile` + `\n` on
  standard output and then, depending on the sizes and the allocator, crashes, corrupts memory, or
  writes `MatrixAlloc(<rows>, <cols>): allocation failed` + `\n` on standard error and exits with
  status 1. **Both modes:** the two lines, exit status 1, FS-13 noted.
- An allocation that the machine cannot give: the same two lines and exit status 1.

### 2.3 Freeing

Freeing sets the caller's matrix to none and returns 0 (none given: 0 too, **(observed)** `mfree null`). A
matrix in a caller's buffer (§2.2) is set to none the same way (`malloc2`: `0 none`).

### 2.4 Copy

Copy (`in`, `out`): no `in`: `out` is freed, nothing returned. `out` absent: a new matrix of `in`'s
size and type (an allocation that returns nothing ends the program: code -2, `MatrixCopy: couldn't
allocate mOut`). `out` with fewer rows or fewer columns than `in`: nothing returned, `out`
unchanged. Otherwise, for each column c = 1..cols of `in`, for each row r = 1..rows, `out`'s
real-addressed element (r, c) = `in`'s real-addressed element (r, c) (§2.1: each matrix's own
addressing; a larger `out` keeps its other elements).

**FS-14.** For complex matrices that copies, per row, `cols` floats starting one float before the
row's first pair (the first row's from the unused float before the numbers): half the values,
shifted. **Default:** both parts of every element copied (same types); **reproduce:** FreeSurfer's
floats. A complex `in` with a real `out` (or the reverse) is copied FreeSurfer's way in both modes.

### 2.5 Product accumulated in double

Product (`m1`, `m2`, `m3`):

1. No `m1`: the program ends: an error with code -4 whose message is `MatrixMultiplyD(): m1 is null`
   + `\n` + ` break <build root>/utils/matrix.cpp:597` + `\n` (the build root of
   `specs/freesurfer-core.md` §3). No `m2`: the same with `m2` and line 601.
2. `m1`'s columns ≠ `m2`'s rows: an error, code -4, `MatrixMultiplyD(): m1 cols <c1> does not match
   m2 rows <r2>` + `\n` + ` break <build root>/utils/matrix.cpp:610` + `\n`; nothing returned.
3. `m3` absent: a new `m1.rows` x `m2.cols` matrix, complex when either input is, else of `m1`'s
   type. `m3` given of another size: `MatrixMultiplyD(): m1/m2 dim mismatch` + `\n` + ` break
   <build root>/utils/matrix.cpp:623` + `\n` on standard output, then an error, code -4,
   `MatrixMultiplyD: (<r1> x <c1>) * (<r2> x <c2>) != (<r3> x <c3>)` + `\n`; nothing returned.
4. An input that is also the output is copied first (the product uses the inputs' values before
   the call).
5. Both real: each element (r, c) of `m3` = `(float) S`, S the double sum, for i = 1..m1.cols in
   order starting from 0, of `(double) m1(r, i) * (double) m2(i, c)` (each product a double
   product). The places follow §2.1 for each matrix's own type (a complex `m3` receives the floats
   at its real-addressed places).
6. Complex × complex, real × complex, complex × real: for each element (r, c) in row order, for
   i = 1..m1.cols: with `a + ib` = m1's element (b = 0 for a real m1), `c' + id` = m2's (d = 0 for a
   real m2), all widened to double: `re = (float)((double) re + (a c' - b d))`, `im = (float)((double)
   im + (a d + b c'))` (the real m1's form: `a c'` and `a d`; the real m2's: `a c'` and `b c'`), the
   real-element and complex-element addresses of §2.1.

**FS-15.** FreeSurfer starts each element's sums of step 6 from `m3`'s **current** value, not 0: a
new `m3` is zero, but a given one adds its earlier contents (and an `m3` that is also an input
adds that input's values). **Default:** started from 0; **reproduce:** from the current value;
FS-15 noted when a given `m3` held anything but zeros.

**FS-16.** A complex product into a **real** `m3` writes its pairs through the real addressing, past
the end of `m3`'s array. **Default:** refused before step 5: an error, code -4, `MatrixMultiplyD: a
complex product into a real matrix` + `\n`, nothing returned; **reproduce:** the writes inside the
array as FreeSurfer makes them, those outside dropped, the reads outside 0. FS-16 noted either way.

## 3. The 4 x 4 affine helpers

An affine vector is 4 floats (x, y, z, 1 when set from x, y, z); an affine matrix 16 floats **column
by column** (float `i + 4j` is row i, column j, from 0).

- **From a matrix:** float `i + 4j` = the matrix's number at row-major position `j + 4i` of its data
  (a transposition of the row-major numbers); **to a matrix** the reverse. A matrix that is not 4 x 4:
  `SetAffineMatrix: Bad matrix size` (or `GetAffineMatrix: ...`) + `\n` on standard error and an
  abort (exit status 134), in both modes (FreeSurfer's own check).
- **Matrix × vector:** each output element i = `((A[i] x0 + A[4 + i] x1) + A[8 + i] x2) + A[12 + i]
  x3` in float, the sum **starting from the first product** (not from 0: four products of -0 give
  -0).
- **Matrix × matrix:** output column j = the matrix × vector of the second matrix's column j.
- **Get a vector:** its first three floats. **Floor of a vector:** `(int) floor(v)` of each of the first
  three (FS-17).
- A new affine matrix is all zeros (FreeSurfer's is not cleared; its callers set it).

## 4. Double matrices

The layout of §2.1 with doubles, real only.

- **Allocation** (rows, cols, type): type 2: `ERROR: DMatrixAlloc(): complex types not supported` +
  `\n` on standard output, nothing returned; any other type than 1: `ERROR: DMatrixAlloc(): types 1
  not recognized` + `\n` (the text says 1 whatever the type), nothing returned. An element count
  `rows cols` negative beyond -2, or beyond `int`: `ERROR: DMatrixAlloc(<rows>, <cols>): allocation
  failed` + `\n`, nothing returned. Otherwise, with fewer than -1 rows: the program ends (code -2,
  `DMatrixAlloc(<rows>, <cols>): could not allocate rptr`). Otherwise a matrix of zeros (-1 rows
  with 1 or 2 columns gives a matrix of -1 rows, which has no elements).
- **Size check** (m1, m2, kind, stream, prefix): 0 when they agree, else 1 after two lines on the
  stream: `ERROR: <prefix> <what>` + `\n` and `break <build root>/utils/dmatrix.cpp:<line>` + `\n`:
  no m1: `matrix 1 is NULL` (line 309); no m2: `matrix 2 is NULL` (314); kinds 1 and 3, rows differ:
  `rows mismatch <r1>, <r2>` (319); kinds 2 and 3, columns differ: `cols mismatch <c1>, <c2>` (324);
  kind 4, m1's columns ≠ m2's rows: `cols of m1 mismatch rows of m2 <c1>, <r2>` (329). The prefixes
  callers pass end with a space, so two spaces follow them.
- **Weighted sum** (m1, m2 optional, v1, v2, out): with m2, the size check (kind 3, standard output,
  prefix `DMatrixAddMul(): `) first; `out` absent: a new matrix of m1's size; the check of m1 with
  `out`; each element: `val = 0.0`; if v1 ≠ 0: `val += v1 m1(r, c)` (or `val += m1(r, c)` when v1 is
  exactly 1); then the same with v2 and m2; `out(r, c) = val` (so a lone -0 becomes +0). Elements are
  independent (FreeSurfer's loop is parallel over rows; the result does not depend on threads).
  Scalar multiple: (m, none, v, 0); difference: (m1, m2, 1, -1); sum: (m1, m2, 1, 1).
- **Copy** (src, out): no src: `ERROR: DMatrixCopy(): msrc is NULL` + `\n`, nothing; out absent: a
  new matrix; the size check (kind 3, `DMatrixCopy(): `); the elements copied.
- **Transpose** (in, out): the same matrix as both: `DMatrixTranspose(): ERROR: cannot be done
  in-place` + `\n`, nothing; out absent: a new cols x rows matrix; `in.rows ≠ out.cols`:
  `DMatrixTranspose(): ERROR: row/col mismatch <in.rows> <out.cols>` + `\n`, nothing; `in.cols ≠
  out.rows`: `... col/row mismatch <in.cols> <out.rows>` + `\n`, nothing.
- **Print with a format** (stream, format, m): each row's elements in the `printf` format (a `%f`
  one: `%8.5lf` for the plain print), two spaces between elements, a newline after each row; no m:
  an error, code -4, `DMatrixPrint: mat = NULL!`, and -4 returned; else 0.
- **Product** (m1, m2, m3): the size check (kind 4, standard output, prefix `DMatrixMultiply(): `)
  first; m3 absent: a new m1.rows x m2.cols matrix; given of another size: `DMatrixMultiply(): m1/m2
  dim mismatch` + `\n` + ` break <build root>/utils/dmatrix.cpp:429` + `\n` on standard output, then
  an error, code -4, `DMatrixMultiply: (<r1> x <c1>) * (<r2> x <c2>) != (<r3> x <c3>)` + `\n`,
  nothing returned. Each element: the **x87 extended** (`long double`: 64-bit significand, round to
  nearest even) sum from 0, in order, of the double products `m1(r, i) m2(i, c)`, each widened
  exactly before the addition; the sum rounded to double.
- **Dot product** (v1, v2): the size check (kind 3, prefix `DDVectorDot(): `), failing:
  `-10e10`; else the extended sum of the **extended** products of the first columns' elements
  (both widened first), rounded to double.
- **Comparison with a float matrix**: the largest `|(double) f(r, c) - d(r, c)|`, -1 after
  `ERROR: DMatrixCompareFMatrix(): mf is NULL` (or `md is NULL`, `row mismatch <a> <b>`, `col
  mismatch <a> <b>`) + `\n`. **Copy of a float matrix**: the values widened; the same messages with
  `DMatrixCopyFMatrix`. **Constant**: every element the value (0 is the zeroing: a new matrix, or the
  numbers cleared). **Largest absolute value**: from 0; -1 after `ERROR: DMatrixMaxAbs(): matrix is
  NULL` + `\n`.

## 5. Histograms

### 5.1 The record

`max_bins` (how many bins were allocated), `nbins` (how many are in use, at most `max_bins` in
practice), `bins[b]` (a float value per bin, its position), `counts[b]` (a float count per bin),
`bin_size`, `min`, `max` (floats). A new histogram of n bins: `max_bins = nbins = n`, zeros
everywhere. n negative: the program ends, code -2, `HISTOalloc(<n>): could not allocate
histogram` (reallocation: `HISTOrealloc(<n>): ...`; **(observed)** `halloc -1`: exit status 254,
standard error `error: Cannot allocate memory` + `\n` + `error: HISTOalloc(-1): could not allocate
histogram` + NUL + `\n`, the C-style error of `specs/freesurfer-core.md` §2 after the system's message). Freeing sets the caller's histogram to none and
returns 0 (none given: 0 too, **(observed)** `hfree null`). "The histogram's bins" below are `0..nbins-1`.
Reading bin b with b < 0 or b ≥ the allocation is **FS-22** (§1's out-of-range rule).

### 5.2 Making and copying

- **Reallocate** (h, n): absent: a new one; else new zeroed arrays of n bins, `max_bins = nbins = n`,
  the other fields kept.
- **Copy** (src, dst): `nbins`, `bin_size`, the first `nbins` bins and counts, `min`, `max` copied
  (`max_bins` stays dst's). **FS-21:** a dst with fewer bins allocated than src's `nbins`: FreeSurfer
  writes past its arrays; both modes grow dst's arrays to hold the copy (FS-21 noted). The same for
  the sum, the difference and the running sum below.
- **Clear** (src, dst): dst absent: a new histogram of src's `nbins`; else dst's first `nbins` bins
  and counts set to 0.
- **Clear counts** (src, dst): dst absent: a new histogram of src's `nbins` with src's bins; dst
  given: dst's counts set to 0 (src not read). **FS-18:** FreeSurfer copies only the **first** bin's
  value into a new histogram and clears only the **first** count of a given one. **Default:** every
  bin copied, every count cleared; **reproduce:** the first only; FS-18 noted when other bins or
  counts were not 0.
- **Initialise** (h, n, lo, hi): h absent: a new histogram of n bins; given: cleared (its `nbins`
  is **not** changed: its callers set it); n above `max_bins`: the program ends, code -4, `HISTOinit:
  nbins <n> bigger than max bins <max_bins>` + `\n`. When hi ≤ lo: lo = 0, hi = `max_bins - 1`.
  `min = (float) lo`, `max = (float) hi`, `bin_size = (float)((hi - lo) / (n - 1))` (double), then 1
  when not above 0; bin b's value `(float)(lo + (double)(bin_size * (float) b))` (the product in
  float) for b < n.
- **Uniform bins** (n, lo, hi): a new histogram of n bins, `bin_size = (float)((hi - lo) / (n - 1))`,
  bin b's value `(float)(lo + (double)(bin_size * (float) b))`, `min`, `max` the floats of lo, hi.

### 5.3 Arithmetic

- **Sum** (h1, h2, dst): dst absent: a new histogram of h1's `nbins` with h1's bins, `min`, `max`,
  `bin_size`, and h2 may be absent (then the new zero histogram is h2); dst given: its `nbins`
  becomes h1's. For each of h1's bins: `dst.counts[b] = h1.counts[b] + h2.counts[b]` (float). An
  input that is also dst gives the same (each count read before written). With dst given and no h2,
  FreeSurfer crashes; the clean room adds zeros (FS-22 noted).
- **Difference** (h1, h2, dst): as the sum without copying bins or range to a new dst, `h1 - h2`.
- **Linear scale** (src, dst, s, o): dst = a copy of src (§5.2; no copy when dst is src); each bin
  value `v s + o` (float), `min = src.min s + o`, `max = src.max s + o`, `bin_size = src.bin_size s`.
- **Normalise to a density** (src, dst): dst absent: a copy of src; the float sum T of dst's counts;
  when T > 0 each count `c / T` (float), and a result with `|c| < 1e-15` (compared as double) becomes
  `(float)(1.0 / (double)(10 T))` (10 T in float). **FS-23:** with a dst other than src, FreeSurfer
  normalises dst's own counts. **Default:** src copied into dst first; **reproduce:** dst's own.
- **Sum to 1**: the double sum S of the counts; when S ≠ 0 each count `(float)((double) c / S)`.
- **Running sum** (h, out): out absent: a copy of h; the double running sum of h's counts stored
  (as float) into out's counts; in place the same.
- **Clear bins in a range** (src, dst, lo, hi: integers): dst = a copy of src (none when dst is src);
  each bin whose value v satisfies `v ≥ (float) lo` and `v ≤ (float) hi` gets count 0.
- **Absolute values** (h, out): `mx` = the largest `|bin value|` (from 0), `mn` = the smallest (from
  1e10), in double; when `mn ≥ mx`, `mn = mx 0.9`. out is reallocated (§5.2) to `nint(ceil((mx - mn)
  / bin_size) + 1)` bins (FS-12), `bin_size` = h's, `min = (float) mn`, `max = (float) mx`, bin b's
  value `(float)((double)((float) b bin_size) + mn)`; then each of h's counts is added (float) at the
  bin **Find bin** (§5.5) gives for `|value|` (FS-22 outside). **FS-50:** when the bin count is
  outside the `int` range (an infinite range of absolute values, a bin size of 0 or NaN),
  FreeSurfer's conversion gives -2147483648 and the allocation fails: the error `HISTOalloc(-2147483648):
  could not allocate histogram` (after `Cannot allocate memory`), and the program ends with status
  254. **Default:** the program ends with status 252 (code -4) and the error `HISTOabs: absolute values
  from <mn> to <mx> in bins of <bin_size> need <n> bins: outside the int range (an infinite range, or
  a bin size of 0 or NaN)` (the numbers as `%f`, n as the shortest decimal), never a 2³¹-bin
  allocation; **reproduce:** FreeSurfer's.
- **Cumulative distributions** (src, dst): first the absolute-value histogram A of src; dst absent: a
  copy of src. **Forward:** for each dst bin b in order whose value v ≥ 0: T = the double sum of A's
  counts whose `|value| ≤ v` (A's bins in order), `dst.counts[b] = (float) T`; a bin with v < 0 gets
  0. Then every count of dst becomes `(float)((double) c / T)` with T the **last** sum computed (0
  when none: IEEE division). **Reverse:** dst's bins from the last down, A's from the last down,
  `|value| ≥ v`.
- **Soap bubble** (src, dst, n): dst = a copy of src; the bins whose src count is above 0 are fixed;
  n times: every other bin b gets, from dst's current counts, `(c[0] + c[1]) / 2` for the first bin,
  `(c[b] + c[b-1]) / 2` for the last, `((c[b+1] + c[b]) + c[b-1]) / 3` otherwise (float), the fixed
  ones src's count, all assigned at the end of the pass. In place the same. One bin: the first bin's
  neighbour is outside (FS-22).
- **Fill holes** (h): bins 1..nbins-2 with a count above 0 are filled; between the first and the last
  filled one, each unfilled bin gets `(float)(counts[b-1] + counts[b+1]) / 2` (float, assigned at
  once, so a bin sees its left neighbour's new value); a pass's change is 1 for a bin whose count was
  0, else `|new - old| / |old|` (double); passes repeat while the largest change exceeds 0.01, at most
  101 passes. Then a first bin of count 0 takes the second's, a last bin of count 0 the one before
  it (FS-22 for fewer than 2 bins). Returns 0.

### 5.4 Smoothing

Smooth (src, dst, σ): dst absent: a copy of src; given with fewer bins in use than src: reallocated
to src's `nbins` (§5.2); dst's `nbins` and `bin_size` become src's (its `min`, `max` stay). The
kernel: `len = nint((double)(8 σ)) + 1` (8 σ in float; FS-12), plus 1 when even, at most 2000;
`half = len / 2`; for x = 0..len-1, `f = (float)(x - half)`, two σ = `2 σ` (float):
`k = (float) exp((double)((-f f) / (2σ σ)))` when `|f| ≤ 2σ`; `k = (1 / (16 (float) e²)) (float)
pow(4 - (double)|f| / (double) σ, 4)` (float product; `e²` the double square of e, then float) when
`2σ < |f| ≤ 4σ`; else 0; then every k divided by their float sum. Each output bin b: over the kernel's
x whose bin `b + x - half` is inside `[0, nbins)`, float sums `N += k` and `T += k counts[b + x -
half]` in order; `dst.counts[b] = T / N`, `dst.bins[b] = src.bins[b]`. **In place** (src and dst the
same histogram): the same, each count computed from the counts already smoothed below it.

### 5.5 Searches

`whalf = (wsize - 1) / 2` (integer division). "The largest count" is the float maximum of the counts
from 0; when it is exactly 0 a peak search returns -1.

- **Last peak / first peak** (h, wsize, pct): from the last (first) bin, the first bin whose count is
  above `pct × largest` (float) and that no bin of `[b - whalf, b + whalf]` inside the histogram
  exceeds; -1 when none.
- **Region bins** of intensities I0, I1 (integers): b0 = the bin **before** the first whose value is
  ≥ I0 (itself when it is bin 0), b1 = the bin **after** the first whose value is ≥ I1 (itself when
  it is the last); each -1 when no bin reaches it.
- **Valley** (h, wsize, I0, I1): counts compared as **integers** (each `(int) count`, FS-17); the
  largest integer count from 0, -1 when 0; b0 → 0 when negative, b1 → `nbins - 1` when -1 or beyond;
  from b0 to b1, the first bin whose integer count no bin of its window undercuts; -1 when none.
- **Last peak in a region** (h, wsize, pct, I0, I1): b1 → `nbins - 1` when -1; from b1 down to b0 (or
  0), the first bin b such that no bin of its window exceeds it, scanning the window in order and
  stopping at the first exceeding one, with the window's float sum W (over the bins inside up to
  that point): `W / wsize ≥ pct × largest` and `(double)(count - W / wsize) ≥ 1.9`; -1 when none.
  (FreeSurfer's scan goes on to bin -1 when b0 is -1, outside the array; it returns -1 either way.)
- **Highest peak in a region** (h, b0, b1): b0 → 0 when negative, b1 → `nbins - 1` when beyond; the
  first bin of the largest count **above 0** in `[b0, b1]`; -1 when none.
- **Previous valley** (h, b0): b0 itself when ≤ 0; else from `b0 - 1` down to 0, the first bin whose
  count exceeds the one above it (starting from b0's count); -1 when none. **Next valley** (h, b0):
  b0 itself when ≥ `nbins`; else from `b0 + 1` up, the first bin whose count exceeds the one below it
  (starting from b0's: b0 < 0 is outside, FS-22); -1 when none.
- **Next peak** (h, b0, whalf): b0 itself when `b0 > nbins - 2`; else for b from `b0 + 1` to
  `nbins - whalf`: the first b whose count no bin of `[b - whalf, b + whalf]` exceeds, the window
  **not clipped** to the histogram (outside: FS-22), scanned in order, stopping at the first
  exceeding bin; -1 when none. **Previous peak** (h, b0, whalf): b0 itself when `b0 > nbins - 2`;
  else for b from `b0 - 1` down to whalf, the same test.
- **Start of peak** (h, b0, p): the previous valley v of b0; b0 itself when ≤ 0; t = b0's count × p
  (float); from `b0 - 1` down to v, the first bin whose count is below t; else `v - 1` (v = -1 makes
  the scan read bin -1, FS-22). **End of peak** (h, b0, p): the next valley v of b0; b0 itself when
  ≥ `nbins`; from `b0 + 1` up to v, the first bin below t; else **the last bin** (kept as FreeSurfer
  has it: never `v + 1`, the mirror of the start's `v - 1`).
- **Current peak** (h, b0, wsize, pct): the highest peak of the whole histogram (-1 when none); the
  minimum `pct ×` its count; the float sums of the counts in `[b0 - whalf, b0)` and `(b0, b0 + whalf]`
  inside the histogram; when the later sum is larger, forward from `b0 - whalf` (from 0): the first
  bin at or above the minimum that no bin of `[b, b + whalf]` inside the histogram exceeds;
  otherwise backward from b0: the first such bin with the full window `[b - whalf, b + whalf]`; -1
  when none.
- **Find bin** (h, v): when `bin_size` is exactly 1: `(int)((float)(int) v - bins[0])` (FS-17 twice:
  the value truncated, the difference in float, truncated); otherwise from the last bin down to 1,
  the first b whose predecessor's value is below v; else 0. **Count at a value**: the count of that
  bin (double), 0 when it is outside `[0, nbins)`. **Bin with count** (h, v): the first bin whose
  `|count - v|` (float) is smallest.
- **Value to bin** (h, v: double): the first bin whose `|bin value - v|` (double) is smallest
  (0 for a histogram of no bins). **Count samples** (h, samples): each sample adds 1 (float) at its
  value's bin.
- **Two-dimensional find bin and count** (h2, v1, v2): as find bin on each axis, with the axis's
  `min` in place of its first bin's value when its bin size is 1; the count `counts[b1][b2]`
  (double), 0 when either bin is outside.

### 5.6 Samples, totals and statistics

- **Add a sample** (h, v, lo, hi): when `lo ≥ hi`: lo = h.min, size = h.bin_size; else size = `(hi -
  lo) / ((float) nbins - 1)` (float). `x = (v - lo) / size` (float); the returned bin number is
  `nint(x)` (FS-12). When it lies in `[0, nbins)`, the sample is shared between the bins below and
  above x, in double: `dp = x - floor(x)`, `dn = ceil(x) - x`; `bp = (int) floor(x)` (0 when negative), `bn =
  (int) ceil(x)` (`nbins - 1` when beyond); `counts[bn] = (float)((double) counts[bn] + dp)`, then
  `counts[bp] = (float)((double) counts[bp] + dn)`. Otherwise FreeSurfer adds 1 (float) to bin 0
  when `(float) bin_number < lo`, else to the last bin when the bin number is ≥ `nbins`, else nowhere.
  **FS-19.** That compares a bin number with an intensity: below the range with a negative `lo` the
  sample is dropped, above it with `lo` beyond the bin count it goes to bin 0, and a NaN position
  goes to bin 0 (through FS-12's `-2147483648`). **Default:** below the range → bin 0, above → the
  last bin, a NaN position not counted; **reproduce:** FreeSurfer's. **FS-20.** At an integer x
  (a sample exactly at a bin's position) dp and dn are both 0, and FreeSurfer adds nothing.
  **Default:** 1 added to that bin; **reproduce:** nothing.
- **Total in a region** (h, b0, b1): the float sum of counts b0..b1 (FS-22 outside); the **total** is
  the region `[0, nbins - 1]`.
- **Clear the zero bin**: nothing when bin 0's value exceeds `bin_size`; else the count of the first
  bin whose value is ≥ 0 set to 0 (none: FreeSurfer writes past the end, FS-22, dropped). Returns 0.
- **Threshold sum** (mask, src, t): the float sum of src's counts at the mask's bins whose count is
  above t.
- **Median**: T = the double sum of the counts; going up with the running double sum R (before the
  bin), the first bin where `R < T / 2` and `R + count > T / 2`: its value when `T/2 - R > R + count -
  T/2` or it is bin 0, else the previous bin's value; 0 when none.
- **Entropy**: T = the float total, widened; `-Σ p log p` (double, in bin order) over the bins of
  positive count, `p = (double) count / T`.
- **Correlation** (h1, h2): more than 256 bins in h2 ends the program, code -3,
  `histoComputeLinearFitCorrelation: only 256 bins allowed`. For each bin of h1: c2 = h2's count at
  **Find bin**(h2, h1's value) (0 outside); double sums of `c1 c2`, `c1²`, `c2²`; a norm below
  `FLT_EPSILON` (|norm| < 2⁻²³) becomes 1; the result `Σ c1 c2 / (sqrt(N1) sqrt(N2))`.
- **Linear-fit correlation** (h1, h2, a, b): double sum over h1's bins b1 of `c1 × h2.counts[nint(b1
  a + b)]` (0 outside), then over h2's bins b2 of `h1.counts[nint((b2 - b) / a)] × c2`. **Linear fit**
  (h1, h2, a0, a1, b0, b1): start from the correlation at (1, 0); the step in a is `MAX(MIN(a0, (a1 -
  a0) / 1000), 0.01)` (kept as FreeSurfer has it: `a0` itself when smaller), in b `MAX((b1 - b0) /
  1000, 0.01)`; for a from a0 while ≤ a1 (a += step), for b from b0 while ≤ b1: a correlation
  strictly above the best replaces it; returns the best and its a, b as floats.
- **Full width at half maximum** (h, peak): t = peak's count / 2 (float); `w_max = min(peak, nbins -
  peak - 1)`; the first width w from 1 while `w < w_max` at which both `counts[peak - w]` and
  `counts[peak + w]` are below t, else the width the scan ended at (1 when `w_max ≤ 1`); returns
  `(float)(2w) bin_size`.

### 5.7 The robust Gaussian fit

(h, q) → (status, offset, σ), with q a double fraction:

1. A `bin_size` not above 0 becomes 1 **in h**.
2. S = h smoothed (§5.4) with σ_s = `(float)(2.0 / bin_size)`; the peak P = S's highest peak in the
   whole histogram; the offset `m = S.bins[P]` (P = -1: outside, FS-22).
3. Z = h linearly scaled by (1, `(float) -m`); A = Z's absolute values; C = Z's forward cumulative
   distribution. With the diagnostic bits write (0x8) and verbose (0x10000000) both set, A, C, h, S
   and Z are written as plots (§5.8) to `ha.plt`, `hc.plt`, `h.plt`, `hs.plt`, `hz.plt`.
4. v = S's **next valley** from `(int)((float) P + 10 / S.bin_size)` (float; FS-17). When v < 0:
   v = C's bin with count `(float) q`, the threshold `θ = C.bins[v]`. Otherwise, while `h.counts[v] <
   h.counts[P] q` (double) and `v > P + 1`: v decreases; `θ = h.bins[v] - h.bins[P]` (float). (v may
   start beyond the histogram, which FreeSurfer reads outside its array: FS-22.)
5. lo = Z's bin of `(float) -θ`, hi = Z's bin of `(float) θ` (find bin). Z of no bins: an error, code
   -4, `HISTOrobustGaussianFit: 0 length cdf`, status -4, σ not set.
6. While lo ≥ hi: lo = max(0, lo - 1), hi = min(nbins - 1, hi + 1). **FS-24:** with one bin this
   never ends in FreeSurfer; both modes stop when the window cannot grow (FS-24 noted).
7. When `hi - lo < 12`: hi += (12 - (hi - lo)) / 2, then lo -= (12 - (hi - lo)) / 2 with the new hi;
   lo ≥ 0, hi ≤ nbins - 1.
8. δ = `(double)(bin_size / 10)` (float division), σ_max = `(double)((float) A.nbins A.bin_size / 2)`.
   For σ = δ while σ ≤ σ_max (σ += δ), with `r = σ √(2π)` (√(2π) = `sqrt(2π)` in double):
   the scale `s = (Σ counts[b] / g_b) / n` over the bins b = lo..hi with `g_b = (1 / r) exp(-z z / ((2
   σ) σ)) > 1e-3`, z = Z's bin value (n of them; n = 0 gives a division by 0, IEEE); the error `E =
   Σ (s / r exp(-z z / ((2σ) σ)) - counts[b])²` over lo..hi; σ is kept when `E < E_best` or no σ is
   kept yet (the first). **FS-51:** when the search cannot advance (σ ≤ σ_max and σ + δ not above σ:
   an infinite bin size, where δ and σ_max are both infinite, or one so small that a tenth of it
   rounds to 0) FreeSurfer never returns. **Both modes:** the error `HISTOrobustGaussianFit: the bin
   size <bin_size> gives no search over sigma (steps of <δ>)` (both `%f`; FreeSurfer's error report,
   code -4), status -4, the offset m set, σ not set (FS-51 noted).
9. With the diagnostic bits of step 3, the fitted curve (the scale recomputed with `z - m`) is
   written as the plot `hp.plt`. Returns status 0, the offset m and the kept σ.

### 5.8 Files

- **Plot** (h, name): the file (relative to the working directory) truncated and written with, from
  the first bin of count above 0 to the last (the last at least the first), lines `printf("%f  %10.10f\n",
  value, count)`; returns 0, or -1 when it cannot be opened.
- **Text** (h, name): every bin `printf("%4d %10.5f %10.5f\n", b, value, count)`; returns 0, or 1 after
  `ERROR: HISTOwrite(): opening <name>` + `\n` on standard output.

## 6. Validation plan, part 1

In §23 (the groups `hmake`, `harith`, `hsmooth`, `hsearch`, `hstats`, `hrobust`, `hfiles`, `mmake`,
`mproduct`, `affine`, `dmatrix`), with the replays of the first programs that use these functions
(`mri_normalize`, `mri_segment`, `mris_make_surfaces`, `mri_ca_register`'s histograms,
`mri_em_register`) when they are ported.


## 7. Conventions, part 2

- **NaN bits.** The official build's arithmetic is SSE2's: a NaN made from numbers (`inf - inf`,
  `0 * inf`, `0 / 0`, the square root of a negative number) has the sign bit set (`0xffc00000` in
  float, `0xfff8000000000000` in double); a NaN operand passes on quieted (quiet bit set, sign and
  payload kept); of two NaN operands, the **left** one of the expression as written (`x += y` gives
  `x`'s; `a c - b d` the product `a c`'s). Every expression of §8-11 follows these rules on every
  platform.
- **The C library, C++'s overloads.** FreeSurfer's matrix code is C++: a function of a `float`
  argument is the `float` function (`expf`, `sinf`, `cosf`, `sqrtf`), of a `double` the `double` one.
  `expf`, `sinf`, `cosf`, `log` are correctly rounded by default and glibc 2.35's (x86-64, its FMA
  build) with `reproduce`; `acos` is correctly rounded in both modes (glibc's `acos` differs from it on
  about 0.1 % of arguments; a later part of larmorx brings glibc's).
- **Complex operands.** The operations of §8.2-8.6 not marked otherwise were written for real
  matrices: they address a complex matrix as if real (§2.1). **FS-25:** **default:** such an operand
  is refused: an error, code -4, `<function>: a complex matrix (the function handles real matrices
  only)` + `\n`, then nothing returned (a number-valued function returns its error value: 0, or -4
  for the finiteness check); **reproduce:** FreeSurfer's addresses (§2.1). Noted either way.
- **Outputs that do not fit.** **FS-28:** where a function fills a given output without checking
  that the result fits (said below), **default:** an error, code -4, `<function>: <what>` + `\n`,
  nothing returned; **reproduce:** filled at FreeSurfer's addresses (§2.1; inside the array they
  land in other rows, outside it they are dropped, FS-16). Noted either way.

## 8. Float matrices, part 2

### 8.1 Product accumulated in float

As §2.5, with these differences:

1. No `m1`: `MatrixMultiply(): m1 is null ` + `\n` + ` break <build root>/utils/matrix.cpp:725` +
   `\n` on standard output, then the program ends: an error, code -4, `MatrixMultiply: m1 is null` +
   `\n`. No `m2`: the same with `m2` and line 729.
2. `m1`'s columns ≠ `m2`'s rows: `MatrixMultiply(): m1/m2 dim mismatch` + `\n` + ` break <build
   root>/utils/matrix.cpp:733` + `\n` on standard output, then an error, code -4, `MatrixMultiply: m1
   cols <c1> does not match m2 rows <r2>` + `\n`; nothing returned.
3. A given `m3` of another size: `MatrixMultiply(): m3 dim mismatch` + `\n` + ` break <build
   root>/utils/matrix.cpp:748` + `\n` on standard output, then an error, code -4, `MatrixMultiply:
   (<r1> x <c1>) * (<r2> x <c2>) != (<r3> x <c3>)` + `\n`; nothing returned.
4. Both real: each element = the **float** sum, from 0, for i = 1..m1.cols in order, of the float
   products `m1(r, i) m2(i, c)` (each product and each sum rounded to float).
5. The complex branches in float: `re = re + (a c' - b d)`, `im = im + (a d + b c')`, every operation
   in float. FS-15 and FS-16 as §2.5 (`MatrixMultiply` in their messages).

### 8.2 Element-wise operations

Unless said, a new output has the input's size and type; a given output must have the size stated,
else an error, code -4, with the message given, and nothing returned; each element is read before it
is written (an output that is also an input gives the same result).

- **Identity** (n, m): absent `m`: a new real n x n matrix with ones on the diagonal. Given: cleared
  (below), then (i, i) = 1 for i = 1..n. FS-28: a given `m` with fewer than n rows or columns.
- **Transpose** (in, out): out(c, r) = in(r, c); a new `out` is cols x rows. FS-28: a given `out`
  that is not cols x rows.
- **Add, subtract** (m1, m2, out): `m2` of another size: `MatrixAdd: incompatable matrices <r1> x
  <c1> + <r2> x <c2>` (subtract: `MatrixSubtract: ...` with `-`) + `\n`; a given `out` of another size:
  `MatrixAdd: incompatable matrices <r1> x <c1> = <r> x <c>` + `\n`. Each element in float.
- **Scalar product** (in, v, out), **square** (in, out): each element `x v`, `x x` in float; a given
  `out` of another size: `MatrixScalarMul: incompatable matrices <r> x <c> != <ro> x <co>` + `\n`
  (`MatrixSquareElts: ...`).
- **Clear** (m): every element 0 (and the unused float before the numbers). **FS-25** for a complex
  matrix: FreeSurfer clears, for each row, the `cols + 1` floats from one float before the row's first
  element in real addressing, which leaves most of a complex matrix; **default:** every float cleared;
  **reproduce:** FreeSurfer's floats.
- **Diagonal** (d, out): n = max(rows, cols) of `d`; absent `out`: a new n x n matrix of `d`'s type;
  given: cleared first, then `MatrixDiag: incompatable matrices <n> x <n> != <r> x <c>` + `\n` if it is
  not n x n (cleared all the same). With more than one row, (r, r) = d(r, 1) for r = 1..rows (the
  first column); with one row, (c, c) = d(1, c).
- **Symmetry test** (m): 0 when not square; else 1 unless some (r, c) ≠ (c, r) (float comparison: a
  NaN element makes it 0).
- **Zero test** (m): 0 at the first element (row by row) whose magnitude, widened to double, exceeds
  `1e-11`; else 1 (NaNs count as zero).
- **Finiteness check** (m): -4 at the first element that is infinite or NaN, else 0.
- **Trace** (m): the float sum from 0 of (n, n), n = 1..min(rows, cols).
- **Element sum** (m): the double sum from 0 of the elements widened, row by row.
- **Constant** (v, rows, cols, X): absent `X`: a new real rows x cols matrix; every element of `X`
  (its own sizes) = v. **Zero** is the constant 0. **Clone** (m): a copy of `m` (§2.4), then the
  constant 0 over its own sizes (FS-14 and FS-25 for a complex `m`; **default:** all zeros).
- **Reshape** (src, dst, rows, cols): a given `dst`'s own sizes replace rows and cols; absent, a new
  rows x cols matrix of `src`'s type. The elements of `src` in row order go to `dst` in row order (its
  columns as the row length) until rows x cols are placed or `src` ends; the rest of `dst` unchanged.
- **Column lengths** (m, scale): a new 1 x cols real row (or the given `scale`, FS-28 when it has no
  row or fewer columns) of `(float) sqrt(S)`, S the double sum from 0 of each column's elements
  squared in double. **Column normalisation** (m, out, scale): absent `out`: a new real matrix of
  `m`'s size; a given one of another size: `ERROR: MatrixNormalizeCol: dimension mismatch` + `\n` on
  standard output, nothing returned. The lengths as above (into `scale` when given); each element
  `(float)((double) m(r, c) / (double) length(c))`, or 0 for a column of length 0.

### 8.3 Regions

Three copies (src, dst, start_row, start_col, rows, cols, dest_row, dest_col): the region (whole
elements), the real parts, the imaginary parts; `<name>` below is `MatrixCopyRegion`,
`MatrixCopyRealRegion`, `MatrixCopyImagRegion`. In order:

1. dest_row < 1 or dest_col < 1: an error, code -4, `<name>: bad destination (<dr>,<dc>)` + `\n`;
   nothing returned.
2. With end_row = start_row + rows - 1 and end_col = start_col + cols - 1: a start below 1 or beyond
   `src`, or an end beyond it: an error, code -4, `<name>: bad source region (<sr>,<sc>) --> (<er>,<ec>)`
   + `\n`; nothing returned.
3. Absent `dst`: a new rows x cols matrix of `src`'s type.
4. `dst` with fewer rows than `rows` or fewer columns than `cols`: an error, code -4, `<name>:
   destination matrix not large enough (<rows>x<cols>)` + `\n`; nothing returned.
5. For each source row from start_row (destination rows from dest_row) and each column from
   start_col (destination columns from dest_col), by `dst`'s type: real `dst`: the source element
   (region), its real part, its imaginary part; complex `dst`: the whole pair (region), the imaginary
   part only (the other two copies). The parts are read as a complex matrix's (§2.1) whatever
   `src`'s type; a `dst` of another type receives nothing.

- **FS-26:** the real-part copy into a complex `dst` copies the imaginary parts. **Default:** the
  real parts; **reproduce:** the imaginary parts.
- **FS-27:** the part copies read a **real** `src` as complex (its "real part" at (r, c) is the float
  of element (r, c + 1)'s place, see §2.1; the last ones beyond the array). **Default:** a real
  `src`'s real part is its element, its imaginary part 0; **reproduce:** FreeSurfer's addresses
  (beyond the array: 0, FS-16). The region copy between a real and a complex matrix addresses one
  through the other's layout: **default:** refused, an error, code -4, `MatrixCopyRegion: a region
  between a real and a complex matrix` + `\n`, nothing returned; **reproduce:** FreeSurfer's
  addresses.
- **FS-28:** step 4 does not check the destination region: dest_row + rows - 1 beyond `dst`'s rows or
  dest_col + cols - 1 beyond its columns (and any dest beyond (1, 1) with an absent `dst`).

**Real to complex** (re, im, out): absent `out`: a new complex matrix of `re`'s size; each element's
real part = re(r, c), imaginary = im(r, c). FS-25 for a complex `re` or `im`; FS-28 for an `im` or a
given `out` smaller than `re`, or a real `out`.

### 8.4 Rotations

**Rotation** (n, angle, which, m): absent `m`: a new n x n identity. c = cosf(angle), s =
sinf(angle). which 0 (x): (2,2) = c, (2,3) = s, (3,2) = -s, (3,3) = c; 1 (y): (1,1) = c, (1,3) = -s,
(3,1) = s, (3,3) = c; 2 (z): (1,1) = c, (1,2) = s, (2,1) = -s, (2,2) = c; any other value: `m`
unchanged. Only those four elements are written.

### 8.5 Vectors

Vectors are matrices whose first column is read (element i is (i, 1)).

- **Length** (v): `sqrtf` of the float sum from 0 of the squares, i = 1..rows.
- **Dot** (v1, v2): the float sum from 0 of the products over v1's rows; **distance** (v1, v2): the
  float sum from 0 of the squared differences over v1's rows: the **squared** distance.
- **Cross product** (v1, v2, out) and **triple product** (v1, v2, v3): when v1 has neither 3 rows nor
  1 column: an error, code -4, `VectorCrossProduct: must be 3-vectors` (no new line; the triple
  product's message too), nothing returned (the triple product: 0). Absent `out`: a clone (§8.2) of
  v1. With (x1, y1, z1), (x2, y2, z2), (x3, y3, z3) the first three elements: out = (y1 z2 - z1 y2,
  z1 x2 - x1 z2, x1 y2 - y1 x2) in float; triple = `x3 (y1 z2 - z1 y2)`, then `+ y3 (z1 x2 - x1 z2)`,
  then `+ z3 (x1 y2 - y1 x2)`, in float.
- **Angle of 3-vectors** (v1, v2): the lengths l1, l2 in double (`sqrt` of `x² + y² + z²` of the
  widened elements); norm = l1 l2; 0 when |norm| < `FLT_EPSILON`; the dot product of the first three
  elements in **float**, widened; norm becomes |dot| when that is larger; `acos(1.0)` (0) when dot >
  norm, else `acos(dot / norm)`, double.
- **FS-31:** the size test above lets column vectors of 1 or 2 rows through and never tests v2, v3
  or `out`; dot and distance read v2 to v1's length. **Default:** a vector with fewer than 3 rows (or
  no column) for the 3-vector functions, or a v2 shorter than v1 for dot and distance, is refused: an
  error, code -4, `<function>: <what>` + `\n`, nothing (0) returned; **reproduce:** FreeSurfer's reads
  (beyond the array 0, FS-16).
- **Normalised load** (x, y, z): len = `(float) sqrt(x² + y² + z²)` in double; the unit vector
  `(x inv, y inv, z inv)` with inv = `1 / len` in float, or (1, 0, 0) when len = 0.
- **Approximate angle** (n, x2, y2, z2, norm): dot = `(float)(n.x x2 + n.y y2 + n.z z2)` in double;
  norm becomes |dot| when that is larger; q = `(double)(dot / norm)` (float division); below
  `0.99f` the result is `(float) acos(q)`, else `(float) sqrt(2 (1 - q))` in double.

### 8.6 Printing

- **Print** (fp, m, p; p is 5 by default): no `fp`: an error, code -4, `MatrixPrint: fp = NULL!`
  (no new line), then standard output. No `m`: an error, `MatrixPrint: mat = NULL!`, returns -4.
  Each real element as `printf("% 2.<p>f")` (a negative p as 6), a complex one `% 2.3f + % 2.3f i`;
  two spaces between elements, `;` + `\n` after each row; a type neither real nor complex: at its
  first element an error, `MatrixPrint: unknown type <t>` + `\n`, returns -4. Returns 0.
- **Print with a format** (fp, fmt, m): the same, each element with `fmt` (for example
  `%10f`), complex `<fmt> + <fmt> i`, `\n` after each row (no `;`).

### 8.7 Similarity transform and Gaussian matrices

- **Similarity** (src, mul, dst): `mul (src mulᵀ)` with §8.1's products (a size mismatch gives
  §8.1's messages; the second product of a failed first one ends the program as `m2` absent).
- **Gaussian matrix** (n, std, norm, G): absent `G`: a new real n x n; a given one of another size:
  `ERROR: GaussianMatrix: dimension mismatch` + `\n` on standard output, nothing returned. var = std
  std (float); f = `(float)(sqrt(2π) std)` in double. With g(d) = `expf(-(d d) / (2 var)) / f` in
  float: norm 1: f = f / Σ g(c - n/2) for c = 0..n-1 (float sum from 0; n/2 integer division) — so
  the centre row sums to 1, not every row; then row r, column c
  (from 0) = g(c - r), the row's float sum S kept; norm 2: each element of the row / S in float.
- **Mixture** (n, s1, s2, w1, norm, G): `w1 G1 + (1 - w1) G2` (1 - w1 in float), the two matrices
  scaled (§8.2) then added (§8.2, into `G`).

### 8.8 Inverse, determinant, QR factors

The decompositions are ITK 4.13.2's VNL in **float** (the main package's ports, `larmorx_core::vnl_fixed`
and `larmorx_core::vnl_qr`: `vnl_det` and `vnl_inverse` of 2 to 4 rows, `vnl_determinant`,
`vnl_qr<float>`), on the matrix's numbers row by row. Their NaNs are the official build's (x86-64's
rules in the operand order GCC 8.4.0 gave VNL: the ports carry it): **(observed)** every line of §23's
`minverse` group with an infinite or NaN input whose numbers are printed (20 inverses of 2 to 7 rows
into a given matrix, 20 determinants, 22 QR factorizations) holds the ports' bits.

- **Determinant** (m): 0 when not square; one element: itself; 2 to 4 rows: VNL's closed-form
  determinant in float; more: VNL's QR determinant in float. **FS-33:** a 0 x 0 matrix: FreeSurfer
  reads the empty factor's first element; **default:** 1 (the empty product); **reproduce:** 0.
- **QR factors** (m, Q, R): each missing matrix or mismatch prints its line on standard output and
  returns 1, in this order: `QRdecomposition(): input matrix is NULL`, `... output Q matrix is
  NULL`, `... output R matrix is NULL`, `... Q rows != input Matrix rows`, `... Q cols != Q rows`,
  `... R rows != input Matrix rows`, `... R cols != input Matrix cols` (each + `\n`). Else Q (rows x
  rows) and R (rows x cols) of `vnl_qr<float>` (LINPACK's Householder QR without pivoting; Q by
  backward accumulation of the reflections) into the given matrices; returns 0. No columns: Q the
  identity.
- **Inverse** (m, out): absent `m`: the program ends, code -4, `MatrixInverse: NULL input matrix!`
  + `\n`. Not square: an error, code -4, `MatrixInverse: matrix (<r> x <c>) is not square` + `\n`,
  nothing returned. Absent `out`: a new matrix of `m`'s size and type (FS-28: a given one of another
  size or type).
  - Real: of one element `(float)(1.0 / a)` (double division), nothing returned when a = 0; of 2 to 4
    rows VNL's closed-form inverse, nothing returned when the closed-form determinant is 0; of more,
    `vnl_qr<float>`'s inverse (each column solved with the identity's column; a column that meets a
    zero diagonal prints `Modules/ThirdParty/VNL/src/vxl/core/vnl/algo/vnl_qr.hxx: vnl_qr<T>::solve()
    : matrix is rank-deficient by <k>` + `\n` on standard error), nothing returned when the QR
    determinant is 0.
  - Complex `A + iC` (n x n): the real 2n x 2n matrix `[[A, -C], [C, A]]`, its inverse as above, and
    the result's real part its upper-left n x n block, its imaginary part the lower-left block.
    **FS-33:** a singular complex matrix: FreeSurfer crashes; both modes: nothing returned.
  - Then nothing is returned when an element of the result (both parts of a complex one) is not
    finite; a given `out` keeps what was written.
  - **FS-33:** a 0 x 0 matrix: FreeSurfer takes the 4 x 4 path on memory it never allocated and writes
    16 floats past `out`; both modes: the empty inverse.

## 9. Prime numbers

- **Primes** (nmax): nmax < 1: `ERROR: GetPrimes: cannot compute primes for <nmax>` + `\n` on
  standard output, nothing returned. Else the primes up to nmax in increasing order (1 is not one).
- **Prime factors** (N): N < 1: `ERROR: cannot compute prime factors of <N>` + `\n` on standard
  output, nothing returned. Else N's prime factors with multiplicity, increasing; 1 has none.
- **Closest prime factor** (N, P) and **closest below** (N, P): among the factors in increasing order,
  the first whose |f - P| is smallest; below: only factors f < P are taken, and when none is, the
  smallest factor. **FS-32:** N = 1 has no factor and FreeSurfer reads the empty list's first element
  (0 under glibc's `calloc`). **Default:** 1; **reproduce:** 0. Noted either way. N < 1: FreeSurfer
  crashes after the message (nothing returned).

## 10. Random draws

- **Gaussian draw** (Marsaglia's polar method on the C library's `drand48`, POSIX's definition: the
  process's one 48-bit state, `srand48` seeds it): v1 = 2 u1 - 1, v2 = 2 u2 - 1 from two draws in
  order, r2 = v1² + v2² (double), drawn again while r2 > 1; the result `v1 sqrt(-2 log(r2) / r2)`.
  **FS-30:** r2 = 0 (both draws exactly 0.5) is accepted and gives NaN. **Default:** drawn again;
  **reproduce:** the NaN. Noted either way.
- **Time-of-day seed:** FreeSurfer returns `tv_sec + tv_usec` of the time of day as an unsigned
  long. **FS-29:** **default:** the fixed seed 1; **reproduce:** the clock's. Noted either way.

## 11. Validation plan, part 2

In §23 (the groups `melements`, `mregions`, `mvectors`, `mprint`, `minverse`, `primes`, `draws`,
`mproduct`'s float products). The angles go through `acos` (§7): there a difference of one unit in
the last place is allowed in both modes until glibc's `acos` is in the main package.


## 12. Conventions, part 3: the double matrices' NaN operands

§7's NaN rules hold, with the official build's operand order measured where two NaNs meet (on
infinities, quiet NaNs of both signs with and without payloads, a signalling NaN among the inputs;
§23's `dmatrix` group):

- **Weighted sum** (§4): for a factor other than 0 and 1 the term is `(element × factor) + val` (of
  two NaNs, the element's in the product and the product's in the sum); for a factor of exactly 1
  `val + element` (`val`'s NaN). The product (§4) multiplies `m1(r, i) × m2(i, c)` (`m1`'s NaN) in
  double before widening.
- **x87 sums** (the product's and the dot product's accumulations, the list statistics): the x87's
  rules: an invalid operation gives the *real indefinite* (sign set, significand `0xC000…0`); of a
  quiet and a signalling NaN the quiet one; of two of a kind the larger significand, on equal ones
  the positive. A double is widened either by loading (a signalling NaN made quiet first) or as an
  instruction's memory operand (kept signalling). **Dot product:** `v2`'s element is loaded and
  multiplied by `v1`'s as the memory operand, so a signalling NaN in `v1` against a quiet NaN in
  `v2` gives `v2`'s. **Product:** each double product (never signalling) is added as a memory
  operand. **List statistics** (the count, mean, standard deviation, minimum, maximum of a list of
  doubles: the extended sums of the values, loaded, and of their double squares; the mean the sum
  over the count rounded to double; the deviation the extended square root of `sum2 / n - mean²`,
  `mean²` in double, rounded to double): the minimum keeps the current one unless the value is
  smaller, the maximum unless it is larger (a NaN never replaces either; a NaN first value stays).

## 13. The minimizers

FreeSurfer's programs hand the minimizers C functions over **1-based** `float` arrays (Numerical
Recipes' indexing): parameters at `p[1..n]`. Their element 0 is never set by FreeSurfer (unset stack
memory; 0 for the iteration callback below); a function receives a fresh copy each call, so what
it writes into it is lost. The minimizers themselves run in double precision and are VNL's (ITK
4.13.2), available in the main package (`larmorx_core::vnl_minimize`: `vnl_nonlinear_minimizer`'s
counters, `vnl_brent`, `vnl_bracket_minimum`, `vnl_brent_minimizer`; `larmorx_core::vnl_lbfgs`:
netlib's `lbfgs` as VXL translated it, with its printed text).

### 13.1 The cost-function bridge

Value at `x` (n doubles): each `x[i]` rounded to float into `p[i + 1]`, the function called, its
float result widened. Gradient at `x`: the same copy, the gradient function writes `g[1..n]` (n + 1
floats, element 0 never read), widened. "Compute" is the value, then the gradient. The bridge's
number of unknowns is the `n` it is built with (the L-BFGS driver reads it; Powell's does not).

### 13.2 Powell's method with given directions

Settings (VNL's defaults): `ftol` 1e-10, `maxfev` 2000 (a `long`; the loop bound is it converted to
an unsigned 32-bit number), line tolerance 1e-4, initial step 1.0, verbose off. Inputs: `p` (n
doubles, updated) and the directions, an n x n matrix whose **columns** are the directions (updated
and given back). An evaluation *report* keeps VNL's counters: the first value starts the start and
end values, a smaller later value lowers the end value; the evaluation count goes up.

1. `fret = f(p)`, reported. `pt = p`. Standard output: `fs_powell::minimize`, `  nparams <n>`,
   `  maxfev <maxfev>`, `  ftol   <%f of ftol>`, `  linmin_xtol_   <%f of the line tolerance>`, each
   with a newline.
2. While the iteration count (from 0) is below the bound:
   - `fp = fret`, `ibig = 0`, `del = 0`; `  powell nthiter <count>: fret = <%f>` + newline.
   - The iteration callback, when a program has set one: a float copy of `p` at `[1..n]` (`[0]` = 0)
     and `n`.
   - For each direction `i` (column `i`): verbose `    param <%3d of i>  nthiter <%2d of the count>`;
     `fptt = fret`; a **line minimization** along the column from `p` with the initial step (below),
     giving `xx` and `fret`; verbose `  min xx=<%g>, fret=<%g>`; `p[k] = p[k] + xx × column[k]`; if
     `|fptt - fret| > del`: `del = |fptt - fret|`, `ibig = i`; verbose `  i = <i>  fp=<%f> fret=<%f>`.
   - Verbose: `  niters <count>  fp=<%f> fret=<%f>` and `  check: <%f of 2|fp - fret|> <= <%f of
     ftol (|fp| + |fret|)>`.
   - If `2 |fp - fret| <= ftol (|fp| + |fret|)`: verbose `  converged`; return **CONVERGED_FTOL**.
   - `ptt = 2 p - pt`, `xit = p - pt`, `pt = p` (element by element); `fptt = f(ptt)`; **`fret`** is
     reported (not `fptt`, as VNL).
   - If `fptt < fp` and `t = 2 (fp - 2 fret + fptt) (fp - fret - del)² - del (fp - fptt)² < 0`: a line
     minimization along `xit` with step 1 (no verbose lines) gives `fret`; `p += xx xit`; column
     `ibig` becomes column `n - 1`'s and column `n - 1` becomes `xit`.
   - The iteration count goes up; verbose: `Iter <count, width 4>, Eval <evaluations, width 4>: Best
     F = <end value, %g in width 10>` + newline on **standard error**.
3. Verbose `  niters <count>`; return **FAILED_USER_REQUEST** (**FS-35**, kept in both modes: VNL's
   Powell returns FAILED_TOO_MANY_ITERATIONS here; FreeSurfer's copy changed it, so the wrappers'
   handling of that code never runs).

**Line minimization** along `d` from `x0` with step `s`: the one-dimensional function `λ ↦ f(x0 + λ
d)` (element by element, each evaluation reported); VNL's bracketing from 0 and `s` (as `vnl_brent`'s
`bracket_minimum` is called with `ax = 0`, `xx = s`, `bx = 0`: it brackets from `bx` and `xx` and
returns the bracket with `ax` its upper end, `xx` its interior point, `bx` its lower end; verbose
`  bracketing` before and `  bracket ax=<%g> xx=<%g> bx=<%g>` after, in the iterations' first
loop only), then VNL's Brent minimizer on (`bx`, `xx`, `ax`) to the line tolerance; its minimum is
`xx`, its value `fret`.

### 13.3 The L-BFGS driver and its observer

Settings: memory `m` 5, line-search accuracy 0.9, default step 1.0, `gtol` 1e-5, `maxfev` 2000,
verbose and trace off. `n` is the bridge's number of unknowns. Workspace `n (2m + 1) + 2m` doubles,
a diagonal and a gradient of `n`. The evaluation and iteration counts are set to 0 (the end value
is not). With verbose: `vnl_lbfgs: n = <n>, memory = <m>, Workspace = <length>[ <length / 128 / 1024,
%g> MB], ErrorScale = 1, xnorm = <%g of the square root of the sum of x²>` + newline on standard
error; with verbose and not trace, `vnl_lbfgs: ` there too.

Loop: before each call, the line search's `gtol` = the accuracy and its initial step = the default
step; value and gradient at `x`; on the first evaluation the start value and the best value are
this value; later, a value **strictly below** the best: the best point = `x`, the best value = it,
and the observer is told (below). Then one call of `lbfgs` (no caller diagonal; `eps = gtol`, `xtol
= 1e-16`; IPRINT (1, 1) when the diagnostic bits hold `DIAG_VERBOSE`, else (1, 0) with trace and
(-1, 0) without; its text on standard output, as `larmorx_core::vnl_lbfgs` prints it: with IPRINT (1, 1)
the vectors' printout holds memory the program does not own, ITK-106, a **memory** line of §23); the iteration count goes up; with verbose and not
trace `<iflag>:<value %g> ` on standard error. `iflag` 0: success; negative: failure; else the
evaluation count goes up, and above `maxfev` (as an `int`) the failure code becomes
FAILED_USER_REQUEST: failure. (With verbose and not trace, `done` + newline at the end.)

The returns, **FS-37**: FreeSurfer records no best point before the first improvement. Success:
the end value is the **last** value evaluated and `x` the best point, or **empty** where no
improvement happened (`OpenDFPMin` then crashes). Failure: with an improvement the end value is the
last value and `x` the best point; without one `x` stays where `lbfgs` left it (the line search's
last trial point) and the end value is unchanged (the previous run's). **Default:** the starting
point is the best until a smaller value; every return gives the best point and the best value.
**Reproduce:** FreeSurfer's, but the empty parameters: there the starting point (and `OpenDFPMin`
stops: §13.6). Noted when the two differ.

The **observer** counts best points (from 0): each one is handed to the program's step function
(the count so far, the best value rounded to float, a float copy of the point at `[1..n]`) and then
its user callback (the copy), and the count goes up.

### 13.4 `OpenPowell`

(parameters `p[1..n]`, directions `xi[1..n][1..n]` or none, `n`, tolerance, iterations out, final
value out, the cost function). Powell (§13.2) with VNL's defaults; **FS-36** (kept): the tolerance
sets VNL's x tolerance, which Powell never reads (the stop is always `ftol` 1e-10). Starts from the
widened parameters and the widened directions (`xi[r + 1][c + 1]` is element `(r, c)`), or the
identity without them. A result of FAILED_TOO_MANY_ITERATIONS ends the program (code -4, `powell
exceeding maximum iterations.`), of ERROR_FAILURE, ERROR_DODGY_INPUT or a tolerance code likewise
(`powell error.`): never, FS-35. Otherwise the iteration count (as `int`), the end value (rounded to
float), the parameters and the directions are rounded back. **FS-34:** without directions
FreeSurfer writes them through the null pointer and crashes; both modes write nothing (noted).

### 13.5 `OpenPowell2`

(parameters, directions, `n`, `ftol`, line tolerance, maximum iterations, iterations out, final
value out, the cost function). Powell with `ftol`, the line tolerance and `maxfev` = the maximum
iterations; verbose (and trace) when `Gdiag_no > 0`. The count, end value, parameters and directions
are always written back (FS-34 without directions); then FAILED_TOO_MANY_ITERATIONS prints `powell
exceeded maximum iterations` + newline and returns 1, the error codes print `powell error <code>`
and return 1 (never: FS-35); else 0.

### 13.6 `OpenDFPMin`

(parameters, `n`, tolerance, iterations out, final value in and out, value function, gradient
function, step function and its data, user callback). The L-BFGS driver (§13.3) with memory 7,
accuracy 0.1, `gtol` = the tolerance (its x and f tolerances, 0.001 and 0.1, are not read); one
observer for the whole call. Passes: the driver from the current parameters while the last pass
succeeded and the stop test says continue. **FS-38:** FreeSurfer stops when `(first - current) /
first <= tolerance` in float, `first` the caller's final value on entry, `current` the pass's end
value rounded to float (never the previous pass's; a NaN continues), so it repeats until a pass
fails. **Default:** stops unless `(previous - current) / previous > tolerance` (`previous` the
caller's value for the first pass, then each pass's end value; a NaN stops). Noted where the two
decide differently. Success: the observer's count, the end value and the parameters written, 0
returned. Failure: the driver's failure code; ERROR_FAILURE (-1) writes the same and returns -1;
ERROR_DODGY_INPUT ends the program (code -4, `quasi-Newton minimization (lbfsg) dodgy input`), the
tolerance codes likewise (`... tolerances too small`): never. **FS-39:** FAILED_USER_REQUEST (9, the
evaluation limit): FreeSurfer writes nothing (the caller's count stays unset); **default:** written
as for -1. Noted either way. Returns the code.

## 14. Numerical Recipes' vectors and matrices

`vector(nl, nh)`: floats indexed `nl..nh`, in one block with one float more before `nl`;
`matrix(nrl, nrh, ncl, nch)`: rows `nrl..nrh` of columns `ncl..nch`, all rows one block (row after
row) with one float more before the first. The values are unset (every caller sets them); freeing
releases them. An allocation failure ends the program (code -1, `could not allocate vector(<nl>,
<nh>)`, `could not allocate matrix(<rows>, <cols>)`, `... array`).

## 15. Validation plan, part 3

In §23 (the groups `minimizers`, `nr`, `dmatrix`'s NaN operands), with the replays of
`mri_em_register` (`OpenDFPMin`, `OpenPowell`) and `mri_segreg`/`bbregister` (`OpenPowell2`) when
they are ported.


## 16. Conventions, part 4: the float product's NaN operands

The float product (§8.1) gives the same values in any order, but when two NaNs meet, which one a
sum or product keeps follows the official build's operand order (SSE2's rule, §7: of two NaNs the
first operand's, quieted): each product is `m2(i, c) × m1(r, i)` (the second matrix's element
first); with 4 or more terms the sums go in blocks of four, `val + p0`, `p1 + val`, `p2 + val`,
`val + p3`, and the terms after the last block (or all of 3 or fewer) as `val + p`.

## 17. Decompositions

All on ITK 4.13.2's VNL (the main package's ports: `larmorx_core::vnl_eigen`, `larmorx_core::vnl_svd_f32`),
on the matrix's numbers row by row.

### 17.1 Symmetric eigensystem (n x n numbers, values, vectors)

`vnl_symmetric_eigensystem<float>`: the matrix converted to double, EISPACK's `rs` (Householder
tridiagonalisation of the lower triangle of the transposed array, that is the upper triangle as
stored, then the QL method), the eigenvalues in ascending order and the eigenvectors as the columns
of the vector matrix (row by row), rounded to float. No rows: nothing written, success.

- **FS-42:** when the QL method does not converge within 30 iterations for an eigenvalue, VNL
  writes `vnl_symmetric_eigensystem: ierr = <i>` + `\n` on standard error, keeps the eigenvalues as
  they stand and never writes the vectors; FreeSurfer copies them (memory) and reports success.
  **Default:** the eigenvalues written, the vectors not, an error `OpenEigenSystem: the eigensystem
  did not converge (ierr <i>)` + `\n`, code -4, failure. **Reproduce:** the vectors 0, success.
- **FS-47:** a matrix of 2 rows or more with an infinite or NaN entry among those the
  tridiagonalisation reads (the upper triangle as stored, the diagonal included; an entry below the
  diagonal is never read, so it changes nothing). VNL computes on it as on any other (IEEE arithmetic,
  x86-64's NaNs), but EISPACK's hypotenuse `pythag(a, b)` (√(a² + b²) by an iteration that stops when
  `4 + r == 4`) takes `p = max(|a|, |b|)` and `r = (min(|a|, |b|) / p)²` with C's `x >= y ? x : y`,
  `x <= y ? x : y` and `x >= 0 ? x : -x`: it **never returns** when `b` is NaN, when both are
  infinite, or both NaN (`r` is NaN and the stopping test never passes); a NaN `a` with a number `b`
  gives `pythag(|b|, |b|)`, `pythag(NaN, 0)` is 0, one infinity gives `+inf`. The QL method calls it
  on the tridiagonal form, the subdiagonal's entry as `b` (`docs/findings/vnl-netlib.md` F-VNLN-7,
  open). **Which matrices make FreeSurfer hang:** exactly those on which the main package's
  `larmorx_core::vnl_eigen::symmetric_eigensystem_f32_full` says VNL never returns (`never_returns()`,
  its defect ITK-115): it follows VNL's operations, infinities and NaNs included, and stops at the
  first `pythag` that would not return. **(observed)** §23's `eigen-nonfinite` batch (111 such
  matrices: FreeSurfer never returned on 42, finished on 69) and an exploration of 400 more (154 and
  246): FreeSurfer never returned on exactly the matrices the port names, and finished on every other
  one with the port's values and vectors, bit for bit, NaNs included.
  - Where VNL never returns, **both modes:** an error `OpenEigenSystem: VNL's eigensystem never
    returns on this matrix (an infinite or NaN entry)` + `\n`, code -4, nothing written, failure
    (FreeSurfer hangs: never reproduced).
  - Where it returns (for example `[[inf, -4], [-4, 8]]`: the eigenvalues 8 and inf, the vectors the
    columns of `[[0, 1], [1, 0]]`; `[[NaN, 1], [1, 2]]`; an infinity alone in its row and column),
    **reproduce:** FreeSurfer's result: the port's values and vectors rounded to float as for a
    finite matrix (FS-42 where the QL method stops), success. **Default:** the error above, code -4,
    nothing written, failure, FS-47 noted (rule B: a matrix with an infinite or NaN entry has no
    eigensystem, and whether FreeSurfer returns on it depends only on where the entries fall).
  - A matrix of one row is computed in both modes (its entry the eigenvalue, the vector 1).

### 17.2 Non-symmetric eigensystem (n x n numbers, values, vectors)

`vnl_real_eigensystem` of the matrix converted to double (EISPACK's `rg`: balancing, reduction to
Hessenberg form, the QR method with vectors, back-transformation): the eigenvalues' real parts and
the real eigenvector matrix (row by row), rounded to float; success. The columns of a complex pair
are 0 (VNL never writes them: memory contents in FreeSurfer, ITK-110); when the QR method fails,
VNL's ` *** vnl_real_eigensystem: Failed on <i>th eigenvalue` + `\n` and the matrix on standard
error, and the eigenvalues it did not find are 0 (ITK-111). **ITK-112:** an infinite or NaN entry
(or sums that overflow) in the part the balancing reads: the balancing never returns, exactly where
the main package's `larmorx_core::vnl_eigen::real_eigensystem` returns `BalancNeverReturns`
(**(observed)** an exploration of 400 matrices with infinite or NaN entries: FreeSurfer never
returned on the 148 the port names and returned on every other one); FreeSurfer hangs; **both
modes:** an error `OpenNonSymmetricEigenSystem: VNL's eigensystem never returns on this matrix (an
infinite or NaN entry)` + `\n`, code -4, failure. The other matrices with such entries are computed
in both modes as VNL computes them; the signs of NaNs in their vectors follow the official build's
operand order, which the port does not follow yet (**(observed)** of the exploration's 252 that
FreeSurfer finished, 62 differ from the port in NaNs' bits only, none otherwise; on §23's lines of
such matrices the port's bits are FreeSurfer's). **FS-41:** FreeSurfer copies the matrix to the stack (8 bytes an element): above
8 MiB (about 1,024 rows) it crashes; both modes compute it, noted.

### 17.3 Eigensystem of a matrix (m, values, vectors)

Not square: nothing returned (values untouched). Real only (FS-25). Symmetric when every element
equals its mirror exactly (a NaN never does): §17.1, else §17.2; a failure: nothing returned. The
eigenvalues are then sorted by decreasing magnitude, a stable merge sort where an element goes before
another unless its magnitude is smaller (equal magnitudes keep their order; NaN orders as such a sort
gives them), and the vectors put in the same order as the columns of the output (a new n x n matrix
when none is given; FS-28: a given one smaller than n x n). The values array holds n floats.

### 17.4 Singular value decomposition (A, w, V)

`vnl_svd<float>` (LINPACK's `ssvdc`, economy size, the singular values' absolute values, none zeroed):
rows ≥ cols required (else failure, nothing written); no columns: success, nothing written. U (rows
x cols) replaces A's numbers, the cols singular values go into w's numbers from the first, V (cols x
cols) into V's numbers from the first, row by row whatever their sizes (beyond an array: FS-16).
LINPACK not converging: on standard error `<ITK root>/Modules/ThirdParty/VNL/src/vxl/core/vnl/algo/vnl_svd.hxx:
suspicious return value (<info>) from SVDC` + `\n`, the same path + `: M is <rows>x<cols>` + `\n`,
then `M = [ ...` + `\n` and each row's elements as `%8d ` for an exact zero or `%8.5f ` otherwise
(C's `nan`, `-nan`, `inf`), the last row followed by ` ]`, each row + `\n`; failure, nothing written.
`<ITK root>` is the official build's `/home/rd521/4.13.2_ubuntu20/itk/4.13.2/src/ITK`; `<info>` is
LINPACK's `info` (the index of the singular value it stopped at, as the main package's
`larmorx_core::vnl_svd_f32::vnl_svd_f32_full` gives it), min(rows + 1, cols) on every failing case
observed.

**FS-48:** a matrix with an infinite or NaN entry (the matrix handed to the decomposition: the
condition numbers' products of finite matrices that overflow are such matrices too). LINPACK
computes on it as on any other, but its deflation and splitting steps do not count towards its
limit of iterations and can come back to the same singular values and superdiagonal for ever
(ITK-116; `docs/findings/vnl-netlib.md` F-VNLN-7, open). **Which matrices make FreeSurfer hang:**
exactly those on which `vnl_svd_f32_full` says VNL never returns (`never_returns()`). **(observed)**
§23's `svd-nonfinite` batch (86 such matrices: FreeSurfer never returned on 12, LINPACK failed on 51
and decomposed 23) and an exploration of 400 more (36, 276 and 88): FreeSurfer never returned on
exactly the matrices the port names; every failure printed the port's `info`; every decomposition
has the port's W and V bit for bit, and U's numbers but for the bits of some NaNs (below).

- Where VNL never returns, **both modes:** the failure lines above with `<info>` min(rows + 1, cols),
  failure, nothing written (FreeSurfer hangs: never reproduced).
- Where it returns, **reproduce:** FreeSurfer's result: LINPACK's failure (the lines above with its
  `info`, failure, nothing written) or its decomposition, written as for a finite matrix (a 1 x 1
  `+inf`: U 1, w `inf`, V 1; `-inf`: U 1, w `inf`, V -1). **Default:** every matrix with an infinite
  or NaN entry gives the failure lines with `<info>` min(rows + 1, cols) without being decomposed,
  failure, nothing written, FS-48 noted (rule B: such a matrix has no singular value decomposition,
  and whether FreeSurfer returns on it depends only on where the entries fall).
- **The NaNs' bits** in a decomposition of such a matrix: the signs and payloads of U's NaNs follow
  the operand order of LINPACK's BLAS kernels as the official build compiled them (the order
  `larmorx_core::vnl_qr`'s kernels `nrm2_t`, `dot_t`, `axpy_t`, `scal_t` record for the QR), which
  `vnl_svd_f32` does not follow yet: **(observed)** 82 of those 111 decompositions differ from the
  port in U's NaNs' bits only (W and V identical), 29 not at all. §23's comparison allows that
  difference on the lines of matrices with an infinite or NaN entry until the port follows them.

### 17.5 The SVD functions

- **SVD** (A, w, V): real only (FS-25). **FS-43:** FreeSurfer allocates a new V rows x rows and
  ignores a failure (V returned unchanged, zeros for a new one, no message). **Default:** a new V is
  cols x cols, a given one of another size is refused (FS-28), a failure gives an error `MatrixSVD:
  the decomposition failed (a <r> x <c> matrix: fewer rows than columns, or one LINPACK cannot
  decompose)` + `\n`, code -4, nothing returned. **Reproduce:** as FreeSurfer. Returns V.
- **SVD eigenvalues** (m, values): the first rows values cleared; a failure returns FreeSurfer's last
  error code as a float. Else the singular values sorted as §17.3 into the values, and the condition
  number: the first value as the minimum and maximum, then the others' magnitudes; 1e8 when the
  minimum is below `FLT_EPSILON`, else maximum / minimum (float). **FS-40:** FreeSurfer reads rows
  singular values where there are cols; **default:** cols (the other eigenvalues 0); **reproduce:**
  rows (memory read as 0, FS-16).
- **Condition number** (m): 1e10 when every element is within 1e-11 of 0 (NaN counts as 0); else the
  decomposition's failure ignored (the singular values 0: 1e8, kept, rule C) and the condition number
  as above, FS-40 likewise. Complex: 1e8 by default after the error (FS-25).
- **Condition number of any shape** (m): square: as above; more rows: of mᵀm; fewer: of mmᵀ (float
  products §8.1).
- **SVD inverse** (m, out): nothing returned when every element is within 1e-11 of 0, or the
  decomposition fails. Else w_max the largest singular magnitude, the threshold `(float)(1e-4 ×
  w_max)`, each singular value below it inverted to 0, the others to `1.0f / w`; the result `V ×
  (W⁻¹ × Uᵀ)` (float products) into `out` (a new matrix when none; a given one of another size:
  §8.1's messages, nothing returned). **FS-44:** more rows than columns: FreeSurfer crashes (after a
  successful decomposition); **default:** the same with the decomposition's sizes (the pseudo-inverse,
  cols x rows); **reproduce:** nothing returned (the crash never reproduced).
- **Pseudo-inverse** (m, out): fewer rows than columns: the transpose of the pseudo-inverse of mᵀ.
  Else `(mᵀm)⁻¹ mᵀ` (float products), the inverse §8.8 or, when it gives nothing, the SVD inverse;
  nothing returned when both give nothing; into `out` (§8.1's messages for another size). **FS-45:**
  for fewer rows than columns FreeSurfer passes `out` to the inner pseudo-inverse (the other shape:
  §8.1's messages, then a crash transposing nothing; an `out` of the inner shape is filled, freed and a
  new matrix returned); it also crashes when the inner pseudo-inverse is nothing (a zero matrix).
  **Default:** the inner one new, its transpose into `out` (FS-28), nothing returned for an empty
  inner result. **Reproduce:** an `out` of the inner shape filled and a new transpose returned;
  nothing returned where FreeSurfer crashes.
- **SVD pseudo-inverse** (m, out): fewer rows than columns: the transpose of the SVD pseudo-inverse of
  mᵀ. Else nothing returned when every element is within 1e-11 of 0; r the index of the first
  singular value whose ratio to the first (float division, compared in double) is below 1e-4, minus
  one (all: cols); the result `V_r × diag(1.0f / w_1..r) × U_rᵀ` (the first r columns of V and U;
  float products). **FS-46:** FreeSurfer returns a new matrix whatever `out`, ignores a failed
  decomposition (the singular values 0: a result of infinities and NaN) and crashes for fewer rows than
  columns when the inner result is nothing. **Default:** into `out`; a failure an error
  `MatrixSVDPseudoInverse: the decomposition failed (a <r> x <c> matrix LINPACK cannot decompose)` +
  `\n`, code -4, nothing returned; an empty inner result: nothing returned. **Reproduce:** as
  FreeSurfer, nothing returned where it crashes.

## 18. Matrix files

### 18.1 The C library's `%f` and `%d` (as glibc 2.35 reads them, measured)

`%f`: white space skipped (the C locale's six characters); the end of the input there: an input
failure. Then an optional sign, and:

- `inf` in any case (a letter that does not match is read, then a matching failure); after it, when the
  next letter is `i`, the rest of `infinity` must follow (a mismatch read, then a matching failure);
  the value ±infinity.
- `nan` in any case (a mismatch read, then a matching failure); never a parenthesised part; the value
  the quiet NaN `0x7fc00000`, `0xffc00000` with `-`.
- `0x` or `0X`: hexadecimal digits, an optional point and more digits; no digit at all: a matching
  failure (the next character unread) unless a point was read, then the value 0 with no exponent read;
  then an optional `p`/`P`, sign and decimal digits (none: exponent 0).
- Otherwise decimal digits, an optional point and digits (no digit at all: a matching failure, the
  next character unread); an optional `e`/`E`, sign and digits (none: exponent 0, the marker and sign
  read all the same: `100ergs` reads 100 and leaves `rgs`).

The value correctly rounded to float (nearest, ties to even, subnormals included). `errno` becomes
`ERANGE` (34) on overflow (±infinity) and on underflow: the result inexact and its magnitude, rounded
to 24 bits with an unbounded exponent, below 2⁻¹²⁶. A number's mismatching character stays unread.

`%d`: white space, an optional sign, decimal digits (none: a matching failure); the value as a 64-bit
`long` (beyond its range: its limit and `ERANGE`), kept to its low 32 bits.

`errno` at the end of the input: a conversion that meets it before its item sets `errno` to 0, unless
an earlier item of the same call ended at the end of the input; directives that meet it change
nothing.

### 18.2 Reading and writing

- **Text read** (file name): not openable: an error `MatrixReadTxt(<name>) - file open failed` + `\n`
  (with `errno`'s text first), code -1, nothing returned. The first piece of line (`fgets` of 999
  bytes: up to 998 bytes or the new line) absent: an error `MatrixReadTxt: could not read 1st line from
  <name>` + `\n`, code -4. Columns: the number of white-space-separated items of that piece; fewer than
  1: nothing returned, no message. Rows: 1 + the number of further pieces of line (a longer line counts
  as several). A new real matrix; from the file's start, each element row by row by one `%f`; one that
  fails: an error `MatrixReadTxT: could not scan value [<row>][<col>]` + `\n`, code -4, nothing
  returned.
- **ASCII write** (stream, m): `<type> <rows> <cols>` + `\n`; each row: a complex element `%+f %+f`
  and three spaces, any other `%+f` and two spaces; `\n` after each row. Success.
- **ASCII read** (stream, m): the first line that is not blank or a comment (199 bytes at most, `#`
  starts a comment) read as `%d %d %d` (fewer: an error `MatrixAsciiReadFrom: could not scanf parms`,
  code -5, nothing returned; text after them ignored). No `m`: a new matrix of that type and size
  (FS-13; an unknown type: the allocator's error, then `MatrixAsciiReadFrom: could not allocate
  matrix`, code -5, nothing returned); a given `m` of another type or size: an error
  `MatrixAsciiReadFrom: specified matrix does not match file`, code -5, `m` returned unchanged. Then
  row by row: each element `%f` and white space (complex: `%f`, white space, `%f`, white space), a
  failure: an error `MatrixAsciiReadFrom: could not scan element (<row>, <col>)`, code -5, nothing
  returned (a given `m` keeps what was read); white space after each row. The messages without a
  trailing new line.
- **Binary read** (stream, m): the type, rows and columns as big-endian 32-bit integers, then each
  element as a big-endian double (complex: two) rounded to float; a matrix and a given `m` as the ASCII
  read (`MatrixReadFrom: could not allocate matrix`, `MatrixReadFrom: specified matrix does not match
  file`). A double cut short prints `freadDouble: fread failed` (code -5); an integer cut short prints
  nothing. **FS-49:** a file that ends early. **Default:** an error `MatrixReadFrom: the file ends
  before the matrix does`, code -5, nothing returned. **Reproduce:** FreeSurfer reads each number's
  bytes over the bytes that held the number read before it, so a number the file cuts short (k of
  its 4 or 8 bytes read, 0 ≤ k < its size) is the k bytes read followed by the rest of those earlier
  bytes, taken big-endian:
  - **the type, rows and columns** (32-bit): the missing bytes of each are the previous integer's as
    the file gave them (rows cut after k bytes: the file's k bytes of rows, then the type's bytes k
    to 3 in file order; the columns likewise from the rows); the type's own missing bytes are memory
    FreeSurfer does not own (0 here, §1);
  - **an element** (a double; a complex element's real part, then its imaginary part, each one
    such double): the missing bytes are those of the double read just before it **as the machine
    holds its value**, its least significant byte first: byte j (counting from 0 in the file's order)
    of the new number is byte j of that value stored little-endian. Once nothing more is read, each
    further element is the previous one with its 8 bytes in reverse order, so the values alternate
    between two numbers (**(observed)**: a 3 x 1 complex file holding two doubles reads
    `c01c767fb336de2a`, `c0022d0fb62dea84`, then `84ea2db60f2d02c0`, `c0022d0fb62dea84`,
    `84ea2db60f2d02c0`, `c0022d0fb62dea84`);
  - **the first element:** into a given matrix, its missing bytes 4 to 7 are the file's bytes 8 to
    11 (the column count as written), its missing bytes 0 to 3 memory (0 here); into a matrix
    FreeSurfer made, every missing byte is memory (0 here).

  Each element is then rounded to float as usual. **(observed)** every `mreadbin` case of §23's
  `mfiles` and `mfiles-cut` batches whose bytes come from the file or earlier numbers: FreeSurfer's
  numbers; those that take a byte from memory are the probe's memory lines.
- **Text write** (file name, m): not openable: an error `MatrixWriteTxt(<name>) - file open failed` +
  `\n`, code -1, returned. Each row: each element `%+4.5f` and a space, then `\n`. Real only (FS-25).
  Success.
- **Binary write** (stream, m): the type, rows and columns as big-endian 32-bit integers, then each
  element row by row as a big-endian double widened from its float (complex: the real part, then the
  imaginary part); returns 0 (`mwritebin`).

## 19. Random-field specifications

A specification holds a name, a code, up to 20 parameters, a mean, a standard deviation, a generator
and a seed. **Name to code:** `uniform` 1, `gaussian` 2, `z` 3, `t` 4, `F` 5, `chi2` 6, any other -1
(stored and returned). **Init** (seed): a cleared specification (no name, code 0, no parameters, mean
and deviation 0) with a new generator (RANLUX389, `specs/ranlux.md`), then **set seed** (seed): 0
takes the time-of-day seed (§10, FS-29), any other is kept; the generator is set to it; it returns 0
(**(observed)** `rfinit`). The draws (`RFsynth`, `RFdrawVal`) and the global statistics of a
volume come with the generator's distributions and the volumes.

## 20. Validation plan, part 4

In §23 (the groups `eigen`, `svd`, `mfiles`, `rfields`). §18.1's conversions are glibc 2.35's, as
the main package's `larmorx_core::cscan` gives them. The **(observed)** marks of §8.8, §17 and
§18.2 about matrices with infinite or NaN entries and files cut short (the batches
`eigen-nonfinite`, `svd-nonfinite`, `mfiles-cut` and the explorations of 400 matrices) are measured
by `validation/investigations/freesurfer-maths-nonfinite/` (open: the probe's runs against the main
package's VNL ports and §18.2's rule).


## 21. Conventions, part 5: the histograms' NaN operands

§7's NaN rules hold for the histograms (a NaN made from numbers is SSE's default NaN, sign set; of
two NaN operands the first, quieted), with the official build's operand order where two NaNs (or an
infinity and its negative, or 0 and an infinity) meet, measured on quiet and signalling NaNs of both
signs with and without payloads and on infinities among the counts, the bin values, the header
(`bin_size`, `min`, `max`) and the scalar inputs:

- **Sum** (§5.3): with dst absent or separate (not an input) and 4 or more bins, the bins in blocks
  of four `h1 + h2`; the 1 to 3 bins after the last block `h2 + h1`, `h2 + h1`, `h1 + h2`. With 3
  bins or fewer, or dst an input (h1 or h2), or h2 the new output: `h1 + h2` throughout.
  **Difference:** `h1 - h2`.
- **Linear scale:** each bin `(v s) + o`, `min` `(min s) + o`, but `max` `o + (max s)` and the bin
  size `s bin_size`.
- **Totals** (total in a region, total): with 4 or more terms, blocks of four `T + c0`, `T + c1`,
  `c2 + T`, `c3 + T`, then the rest `T + c`. **Density** (§5.3): its float sum in blocks of four
  `c0 + T`, `T + c1`, `c2 + T`, `T + c3`, then `T + c`; each count `c / T`. **Threshold sum:** `T + c`.
- **Double sums** (cumulative distributions, sum to 1, running sum): `S + c`; the divisions `c / S`.
- **Smoothing** (§5.4): `norm + k`, the product `k × count`, `total + product`; `total / norm`.
- **Fill holes, soap bubble:** the mean of two `(c[b-1] + c[b+1]) × 0.5` (fill holes), `(c[b] +
  c[b±1]) × 0.5` (soap bubble's ends), `((c[b] + c[b+1]) + c[b-1]) / 3` (its middle bins: the bin's
  own count first).
- **Absolute values:** each count `A.count + h.count`; a bin value `(double)(b × bin_size) + mn` (`0
  × ∞` the default NaN).
- **Histogram of bins** and **init**: a bin value `(double)(bin_size × n) + min`.
- **Correlation:** `(c2 c1) + Σ`, `(c1 c1) + N1`, `(c2 c2) + N2` (the products first), the result
  `Σ / (sqrt(N2) sqrt(N1))`. **Linear-fit correlation:** `(c1 c2) + Σ` over h1's bins, `(c2 c1) + Σ`
  over h2's.
- **Entropy:** `p = c / T`, `H - (p log p)`.
- **Comparisons** are IEEE's (a NaN compares false): a NaN count is a candidate of the current-peak
  search's backward scan (`count < min_count` is false for it), a NaN never replaces a largest or
  smallest value; a NaN or infinite bin position or sample becomes a bin number through FS-12 or FS-17
  (-2147483648 with `reproduce`).

## 22. Validation plan, part 5

In §23 (the histogram groups carry non-finite values among the counts, the bin values, the header and
the scalar inputs).

## 23. Validation plan (black box)

### 23.1 The probe

The open probe folder `validation/investigations/freesurfer-maths-probe/` holds the probe's source
(calls of the functions of §0 by name), a guard allocator and the comparison loop; its README lists
every command, its arguments and what it prints, the encodings (every float and double as its bits)
and the four test costs of the minimizers. The implementer runs the prebuilt probe
`<workspace>/oracles/freesurfer-8.2.0-unit/probes/fs_maths_probe` (FreeSurfer 8.2.0's library as the
official build compiles it: the unit oracle, GCC 8.4.0 and FreeSurfer's flags) with
`guard_malloc.so` beside it, and never builds them. Each call runs in a child process of its own with
an alarm: its line is the printed result, or `EXIT <status>` where FreeSurfer's error functions end
the program, `CRASH <signal>`, `TIMEOUT` where FreeSurfer never returns; then its standard output
and standard error.

### 23.2 The groups and their reference hashes

`compare.py refs` (seed 7) runs each group's batch on the probe four times: plainly, plainly again in
another process (the stack below each call zeroed), and in the two guard runs (the guard allocator
with an inaccessible page right after each allocation, fill byte 90, then right before it, fill byte
165). A line whose four results are not the same depends on memory FreeSurfer does not own (a
**memory** line: a read past an array's end or before its start crashes in one guard run and reads
the fill byte in the other; memory never written or freed, contents that follow the process's
addresses, the stack's fill where nothing wrote). The hash is the
SHA-256 of the group's plain lines, each memory line and each line where FreeSurfer crashed as
`<memory>` (such a line may be either from one run to the next), the clock's digits masked. **(observed)** on
the reference machine (glibc 2.35, Linux 6.8, x86-64), through the oracle-output cache and without
it, and again from the cache:

| Group | § | Calls | Memory or crash | EXIT | TIMEOUT | SHA-256 |
|---|---|---|---|---|---|---|
| `hmake` | 5.1, 5.2 | 632 | 38 | 3 | 0 | `f0f701cd2f25ee6a1d531c5a50ae3b0b06ca75b6b1fe39e136a7c4d2404146a7` |
| `harith` | 5.3, 21 | 4,364 | 34 | 28 | 0 | `9dd8b0231e076d6d842ab058965e65c3dc7bfde90d4b47401f20f0bcb839eb8e` |
| `hsmooth` | 5.4 | 450 | 0 | 0 | 0 | `6849da4e3bca2b357f0279e90cdb6b2a39879aa6fe321eced7411f5253a47ed9` |
| `hsearch` | 5.5 | 3,360 | 249 | 0 | 0 | `d56865eae1fc05166fffc44424405f680c10f6502bbf975232cb9dcb63d28e8e` |
| `hstats` | 5.6 | 1,651 | 48 | 1 | 0 | `b967819d9feb9a0d3e22e29d8a9fc596ad1cac8278a8a25f9fb8e2598b375689` |
| `hrobust` | 5.7 | 74 | 29 | 2 | 2 | `0d5172c5fb31984cf4de204efdfe496a890bba4ae89b2fc85d35573b55d38830` |
| `hfiles` | 5.8 | 62 | 0 | 0 | 0 | `8ae7f99741fe6f2fb95a5de15cf1e6ef3a680fd4e12cf0bec60d4ed2a1145b3f` |
| `mmake` | 2.2 to 2.4 | 356 | 2 | 2 | 0 | `06cedef9465553f35894e0a276e548f627648f42b7737682b15189924b474d15` |
| `mproduct` | 2.5, 8.1, 16 | 1,202 | 20 | 2 | 0 | `4f285237bdaa014c350f7011104306f2b6f55ade4b4867700d4d17db05d65c5d` |
| `affine` | 3 | 243 | 2 | 0 | 0 | `514bd8d32cd528c6ed8a062d5ba38f1797a8c218b3940d1dfe902cb239198cfb` |
| `dmatrix` | 4, 12 | 2,904 | 8 | 1 | 1 | `719f4ea4e5bf36174a84e21120f0c7dd87fb4a1086981864ab7daf8acfc040b4` |
| `melements` | 8.2 | 3,500 | 31 | 0 | 0 | `e60f3eb349c7fed12395e0f494d8d38a4f6789ad19f1ed782f5cecb7717c68c0` |
| `mregions` | 8.3 | 880 | 83 | 0 | 0 | `c50b1b9f577cc67267a4d837f5e912d4f777409d8237b05c0a1ed9bbfb500742` |
| `mvectors` | 8.4, 8.5 | 1,602 | 172 | 0 | 0 | `22123ebdb26ee3c5817014a14a633a0eca6b2b81ec32d38d2dd48d0a159435eb` |
| `mprint` | 8.6, 8.7 | 363 | 0 | 1 | 0 | `99f11898d09954688429163f63a6dae14d07aaeb034ee8620dbe0c59efa72127` |
| `minverse` | 8.8 | 862 | 10 | 1 | 0 | `f17057e0444970110a0051cfe1638031987eb2b9fbf05a6b31fdc8ea6f0b5935` |
| `primes` | 9 | 215 | 2 | 0 | 0 | `7161c5f85f0251b95dcb00423c7ea90119bd46deff0750af6a77ff640cf5859c` |
| `draws` | 10 | 6 | 0 | 0 | 0 | `01ada774a50680e6d429d614299f85e24c0306153fa7e0a361735f51ffe275a4` |
| `minimizers` | 13 | 181 | 11 | 0 | 0 | `cbea1be24fc233c0f353a05041189aee308c5501359f6fad0e016c9135ffd2e4` |
| `nr` | 14 | 5 | 0 | 0 | 0 | `ba8b8aea531d281110a5a52a4a2455fe313fc4e8b3a5a4ce6b1e1cc19eaad69d` |
| `eigen` | 17.1 to 17.3 | 1,460 | 252 | 0 | 153 | `c3ddceff612985227ad6d69acf56662aaa0584974c847c827005539ce38b17f3` |
| `svd` | 17.4, 17.5 | 2,280 | 354 | 0 | 142 | `af777aba4138e1ab5d48ba2ee4e4cc8477a2f3d0abc1d9334c05d1a10c40aad7` |
| `mfiles` | 18 | 699 | 28 | 0 | 1 | `ac865eb3e87ec634b6e1580d41005a8da591aff62d674f3463bf984a0b38f157` |
| `rfields` | 19 | 16 | 0 | 0 | 0 | `6f26117e696800de35160344e0a49825b4ff992d68dc6a500ca215703240ce15` |
| all | | 27,367 | 1,373 | 41 | 299 | |

The groups `eigen`, `svd` and `mfiles` gained the batches `eigen-nonfinite`, `svd-nonfinite` and
`mfiles-cut` (§17, §18.2; generators of their own, the groups' other cases unchanged) on 2026-11-13;
their three hashes are from then, the others from the first runs.

### 23.3 The comparison

`compare.py compare --candidate <program> --mode reproduce` (then `--mode fix`) runs every case on
the clean room's program, which takes the probe's commands and prints the same lines, with
`LARMORX_UPSTREAM_DEFECTS` set to the mode; its defect notices (`upstream defect <ID> ...` on
standard error) are taken out and their IDs kept. Every line must be identical, byte for byte, but:

- a line where FreeSurfer did not finish (`TIMEOUT`, `CRASH`): the candidate's line must not be
  one (the defects of those cases say what the clean room gives: FS-13, FS-22, FS-24, FS-31, FS-33,
  FS-34, FS-37, FS-41, FS-44 to FS-48, FS-51, ITK-112); in the groups `eigen` and `svd`, where
  FreeSurfer never returned (`TIMEOUT`), the candidate's standard error must also hold the refusal of
  §17 (`never returns on this matrix` for the eigensystems, VNL's `suspicious return value (<info>)
  from SVDC` for the SVD), in both modes;
- in the group `svd`, a line whose input holds an infinite or NaN number and that differs only in
  the bits of NaNs (their signs and payloads, in the printed numbers and in VNL's printed matrix):
  the official build's LINPACK kernels (§17.4);
- a **memory** line (FreeSurfer reads what it does not own, among others FS-16, FS-21, FS-22, FS-27,
  FS-31 to FS-33, FS-40, FS-42, FS-49, ITK-106, ITK-110, ITK-111; the clean room reads 0 there, §1);
- in `--mode fix`, a line where the candidate noted a defect (the corrected result).

So with `--mode reproduce` the matrices with infinite or NaN entries that FreeSurfer finishes (FS-47,
FS-48: the batches `eigen-nonfinite` and `svd-nonfinite` hold both kinds) are compared as any other
line, and the files cut short (FS-49: the batch `mfiles-cut`, every byte of the header, of the first
element and of later ones) likewise, but where a byte comes from memory.

The loop prints, per group, the identical lines and those of each allowed kind, and the IDs noted on
the allowed lines; exit status 1 for any other difference.

**(observed)** with the probe as its own candidate (`--candidate` the prebuilt probe, `--mode
reproduce`): no unexplained difference; 26,610 of the 26,707 lines identical, 71 memory lines whose
contents changed from process to process and 26 where FreeSurfer crashed in one process and not in
the other (all in `svd`: a pseudo-inverse's freed output, FS-45). Two runs of `refs` in separate
processes classify every line alike.

### 23.4 Beyond the probe

- **The replays** of the programs that use these functions (`mri_normalize`, `mri_segment`,
  `mris_make_surfaces`, `mri_ca_register`, `mri_em_register`, `mri_segreg`/`bbregister`) with
  `reproduce`, on the recorded heads, when they are ported.
- **Every platform:** the clean room's own golden checks (its results on the other five platforms
  are its Linux results; a NaN's bits as x86-64 gives them, §7).

## 24. The defects (index)

Where each defect of this module is specified (its two modes there):

| Defect | § | Defect | § | Defect | § |
|---|---|---|---|---|---|
| FS-13 | 2.2 | FS-26 | 8.3 | FS-39 | 13.6 |
| FS-14 | 2.4 | FS-27 | 8.3 | FS-40 | 17.5 |
| FS-15 | 2.5 | FS-28 | 7 | FS-41 | 17.2 |
| FS-16 | 2.5 | FS-29 | 10 | FS-42 | 17.1 |
| FS-17 | 1 | FS-30 | 10 | FS-43 | 17.5 |
| FS-18 | 5.2 | FS-31 | 8.5 | FS-44 | 17.5 |
| FS-19 | 5.6 | FS-32 | 9 | FS-45 | 17.5 |
| FS-20 | 5.6 | FS-33 | 8.8 | FS-46 | 17.5 |
| FS-21 | 5.2 | FS-34 | 13.4 | FS-47 | 17.1 |
| FS-22 | 5.1 | FS-35 | 13.2 | FS-48 | 17.4 |
| FS-23 | 5.3 | FS-36 | 13.4 | FS-49 | 18.2 |
| FS-24 | 5.7 | FS-37 | 13.3 | FS-50 | 5.3 |
| FS-25 | 7 | FS-38 | 13.6 | FS-51 | 5.7 |

VNL's own (the main package's ports carry them): ITK-106 (§13.3), ITK-110, ITK-111, ITK-112 (§17.2).
FS-12 (`nint`) is `specs/freesurfer-core.md`'s.
