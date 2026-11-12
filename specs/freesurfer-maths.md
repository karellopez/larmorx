# FreeSurfer 8.2.0's mathematics: behaviour spec (`fs.maths`)

Written by the replica author of queue items 3.1-23 and 3.1-24 as their spec writer
(`docs/porting-guide.md` §1): behaviour, maths, conventions, messages and the validation plan, from
FreeSurfer 8.2.0's source and the official build's outputs, never code or routines. A clean-room
implementer writes the clean room from this file, black-box runs of the official programs (and of
the probe `validation/probes/fs-maths/`, a black-box run of FreeSurfer's code) and the cited
standards only.

Status: **part 1** (§1-6): the float matrices' storage, allocation, copy and double-accumulated
product; the 4 x 4 affine helpers; the double matrices with their long-double products; the
histograms. To come: the matrices' remaining operations (inverse, determinant, decompositions,
statistics), the numerics wrappers, the probability functions, the
random fields.

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

Freeing sets the caller's matrix to none and returns 0.

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
- **Floor of a vector:** `(int) floor(v)` of each of the first three (FS-17).
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
histogram` (reallocation: `HISTOrealloc(<n>): ...`). "The histogram's bins" below are `0..nbins-1`.
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
  bin **Find bin** (§5.5) gives for `|value|` (FS-22 outside).
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
  has it: the test `nbins ≥ b` always holds).
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
   kept yet (the first).
9. With the diagnostic bits of step 3, the fitted curve (the scale recomputed with `z - m`) is
   written as the plot `hp.plt`. Returns status 0, the offset m and the kept σ.

### 5.8 Files

- **Plot** (h, name): the file (relative to the working directory) truncated and written with, from
  the first bin of count above 0 to the last (the last at least the first), lines `printf("%f  %10.10f\n",
  value, count)`; returns 0, or -1 when it cannot be opened.
- **Text** (h, name): every bin `printf("%4d %10.5f %10.5f\n", b, value, count)`; returns 0, or 1 after
  `ERROR: HISTOwrite(): opening <name>` + `\n` on standard output.

## 6. Validation plan, part 1

- **Against the official code, function by function** (`validation/probes/fs-maths/`): `fsmaths`
  runs FreeSurfer's own functions from the unit oracle's build (queue item 3.1-02: the official
  build's library, byte-identical programs) on seeded cases of 47 commands (every function of §2-5:
  the in-place forms, the outputs given and absent, the defects' cases), each number as its bits.
  Measured 2026-11-12: 49,000 cases of 49 commands, `reproduce` identical on all 48,185 comparable
  ones (815 read outside FreeSurfer's arrays: FS-22, not comparable; in 768 of them FreeSurfer's
  memory happened to hold 0); `fix` differs only where FS-14, FS-15, FS-18, FS-19 or FS-20 changed
  the result, and in 13 entropies, where the correctly rounded `log` moved the last bit. The
  SHA-256 of the official outputs of seed 1 (490 cases) is checked by the port's test on every
  platform.
- **The defects' two modes**: the port's unit tests (`crates-freesurfer/larmorx-freesurfer-core`).
- **The replays** of the first programs that use these functions (`mri_normalize`, `mri_segment`,
  `mris_make_surfaces`, `mri_ca_register`'s histograms, `mri_em_register`), with `reproduce`, on the
  recorded heads (queue: the programs' items).
