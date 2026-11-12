# FreeSurfer 8.2.0's mathematics (`fs.maths`): findings

**Closed to FreeSurfer clean-room implementers** (source-derived: it names FreeSurfer's files and
lines; the behaviour is in `specs/freesurfer-maths.md`). Written by the replica author of queue
items 3.1-23 and 3.1-24 (FreeSurfer's foundations, track A). Defect IDs FS-13 to FS-24 (this
part 1) and FS-25 to FS-32 (part 2, item 3.1-24), decided by rules A to D (CLAUDE.md, D13); the
register's entries are in the orchestrator's handoff. Every claim below is from the source of the 8.2.0 tag and, where a number is given, from
the probe `validation/probes/fs-maths/` (FreeSurfer's own functions from the unit oracle's build).

## The measurement

`check.py --seed 7 --n 49000` (2026-11-12): 49,000 cases of 49 commands (every reached function of
`histo.cpp`, `MatrixAlloc`/`MatrixCopy`/`MatrixMultiplyD` with real and complex matrices, the
affine helpers, the double matrices with the x87 `long double` products `DMatrixMultiply` and
`DVectorDot`, and `utils.cpp`'s `DListStats`). The port with `reproduce`: **48,185 of 48,185
comparable lines identical** (the 2,000 long-double cases included: `larmorx_core::x87::F80`);
815 cases read outside FreeSurfer's arrays (FS-22: `HISTOfindStartOfPeak` 288,
`HISTOrobustGaussianFit` 277, `HISTOfindNextPeak` 144, `HISTOfindPreviousValley` 54,
`HISTOfindNextValley` 48, `HISTOfindPreviousPeak` 4), where the heap's contents decide; in 768 of
them they happened to equal the port's 0. The port's default differs from FreeSurfer on 2,348
lines, each with the notice of the defect that changed it (FS-18 1,000, FS-15 993, FS-14 649,
FS-20 287, FS-19 144), except 13 entropies (`HISTOgetEntropy`), where larmorx's correctly rounded
`log` moved the last bit (rule 5; `reproduce` uses glibc's). Seed 1, 490 cases: 481 identical, 9
FS-22; the official outputs' SHA-256 `394127d7...` is checked by `tests/fs_maths_official.rs` on
every platform. A first run (47,000 cases, 47 commands, before the long-double ones) gave the
same: 46,138 of 46,138. The C side takes about 35 s for such a run, the port about 1 s (release
builds). `randomNumber`'s sequences (`fs.core`, now drawing from `larmorx_core::vnl_random`) for
the seeds -1, 1234 and 53 equal the unit oracle's `fs-random` (250 numbers each; the test
`random::tests::the_official_sequences`).

## FS-13: impossible matrix sizes (rule B: a clear error)

`utils/matrix.cpp` 298-392 (`MatrixAlloc_old`) and 394-517 (`MatrixAlloc_new`). A negative size or
an `int` overflow of `rows * cols` makes `calloc`/`posix_memalign` fail (or, with the new
allocator's `size_t` arithmetic, wrap to a small block and then `bzero` with a wrapped size); both
then print `MatrixAlloc(%d, %d): Using mmap'd tmpfile` and try a temporary file: the new allocator
calls `fileno(mat->mmapfile)` at line 467 with `mat` NULL or with `mmapfile` never set (it is
cleared only later, by the `bzero` of line 481), a crash; the old one maps a file of `(nelts + 2)
* sizeof(float)` bytes that fails and prints `allocation failed`, `exit(1)`. Both modes: the two
lines and exit 1 (the crash depends on memory and is never reproduced).

## FS-14: `MatrixCopy` of a complex matrix (rule A)

`utils/matrix.cpp` 150-181: the loop at 173-174 copies `*MATRIX_RELT(mOut, row, col) =
*MATRIX_RELT(mIn, row, col)` whatever the type; `MATRIX_RELT` (`include/matrix.h`) is `rptr[r] +
c` on a `float **`, and a complex matrix's row pointers (`MatrixAlloc_*`: `(float *)(((CPTR)
mat->data) + (row - 1) * cols - 1)`) point at pairs, so the copy takes `cols` single floats per row
from one float before the row's first pair. Complex matrices come from MATLAB `.mat` files with an
imaginary part (`utils/matfile.cpp` 113, 593, 828); `MatrixInverse`'s complex branch (`matrix.cpp`
212-262) builds its real matrices with the region copies. Default: both
parts copied; `reproduce`: FreeSurfer's floats (measured identical, `mmdc` cases).

## FS-15: `MatrixMultiplyD`'s complex branches accumulate onto the output (rule A)

`utils/matrix.cpp` 587-710: `MatrixClear(m3)` is commented out (643); the real branch (648-658)
stores each element, but the three complex branches (659-706) do `MATRIX_CELT_REAL(m3, row, col)
+= ...`. A new `m3` is zeros (`MatrixAlloc`), a given one is not. `MatrixMultiply_wkr` has the same
comment at 768 (item 3.1-24). Default: started from 0; `reproduce`: added on (measured identical).

## FS-16: matrix addresses outside the array (both modes defined)

`MatrixMultiplyD` does not check `m3`'s type: a complex product into a real `m3` writes through
`MATRIX_CELT` on real row pointers, which reach `rows * cols + cols + 1` floats into an array of
`rows * cols + 2`. Rule B by default (a refusal with an error); `reproduce` writes the floats inside
the array. Not met in the cases (the probe's products allocate their output).

## FS-17: conversions to `int` (the 2026-10-11 rule: saturate and warn)

`HISTOfindValley` (`histo.cpp` 741-778) declares `center_val`, `max_count`, `other_val` as `int`
and assigns `h->counts[b]` to them; `HISTOfindBin` (1296-1305) returns `(int)val - h->bins[0]`, a
`float` converted to `int`; `HISTO2DfindBin1/2`; `HISTOaddSample`'s `(int)floor(dbin)`,
`(int)ceil(dbin)`; `HISTOrobustGaussianFit`'s `peak + 10 / hs->bin_size` passed as an `int`
(1691); `GetFloorAffineVector` (`include/affine.h` 67-75). GCC's `cvttss2si`/`cvttsd2si` give
`INT_MIN` beyond the range.

## FS-18: `HISTOclearCounts` (rule A)

`utils/histo.cpp` 278-289: `memmove(histo_dst->bins, histo_src->bins, sizeof(*histo_src->bins))`
and `memset(histo_dst->counts, 0, sizeof(histo_dst->counts[0]))`: one `float` each. The callers
that reset per-vertex histograms with it (`mris_make_surfaces.cpp` 4560-4562 in
`compute_white_target_locations`, the T2/FLAIR path; `mris_ms_refine.cpp` 2079, 3140) keep
accumulating. `HISTOsmooth` (516) calls it on a histogram whose counts it then overwrites: no
effect there.

## FS-19 and FS-20: `HISTOaddSample` (rule A)

`utils/histo.cpp` 963-1002. FS-19: the out-of-range branch tests `if (bin_no < bmin)` (995), a bin
number against the range's lower intensity, where `bin_no < 0` is meant. FS-20: the in-range branch
(978-994) gives `dp = dbin - floor(dbin)` to `ceil(dbin)` and `dn = ceil(dbin) - dbin` to
`floor(dbin)`, both 0 at an integer `dbin`: FreeSurfer's own `utils/mideface.cpp` 309 says "should
use HISTOaddSample() but it appears to be broken when binsize=1". Callers: `mrihisto.cpp` 1318,
1350 (`MRIhistogram*`), `gcamorph.cpp` 13566, 16955-16956, `mrisurf_sseTerms.cpp` 2647-2655,
`mris_make_surfaces.cpp` 4400-4412, 4645, `rbm.cpp`, `mris_smooth.cpp` 556, `mris_deform.cpp` 3848.
Measured in the cases: FS-20 changed 298 of 1,000 `addsample` lines, FS-19 150.

## FS-21: writes past a smaller output histogram (both modes defined)

`HISTOcopy` (168-180) `memmove`s `src->nbins` values into a `dst` allocated with any size;
`HISTOadd`, `HISTOsubtract` (402-473) and `HISTOcumsum` (3024-3035) write `h1->nbins` counts into
a given output. Every caller in FreeSurfer passes outputs of the same size; both modes grow.

## FS-22: reads and writes outside the arrays (both modes defined)

- `HISTOfindNextValley` (1043-1057) reads `h->counts[b0]` before checking b0 (b0 = -1 from a failed
  search, as `HISTOfindEndOfPeak` passes on); `HISTOfindPreviousValley` (1027-1041) likewise for
  b0 ≥ `nbins`.
- `HISTOfindNextPeak` (1059-1076): `b <= h->nbins - whalf` lets the window `b + whalf` reach bin
  `nbins`; `b - whalf` goes below 0 for small b0. `HISTOfindPreviousPeak` (1078-1095): the window
  reaches `b0 - 1 + whalf`, beyond the last bin for whalf ≥ 3, and its first test is the copy of
  `HISTOfindNextPeak`'s `b0 > nbins - 2`.
- `HISTOfindStartOfPeak` (1097-1112): with no previous valley (-1) the scan reads bin -1.
- `HISTOrobustGaussianFit` (1667-1784): `hs->bins[peak]` with peak -1 (no count above 0), and the
  next valley searched from `peak + 10 / bin_size`, beyond the last bin whenever the mode lies within
  10 intensity units of the top: `HISTOfindNextValley` then returns its start and line 1698 reads
  `h->counts[bin]` there (304 of 1,000 `robust` cases).
- `HISTOclearZeroBin` (1264-1276) writes `counts[nbins]` when every bin is negative;
  `HISTOfillHoles` (1209-1248) and `HISTOsoapBubbleZeros` (1886-1917) read past a histogram of 0 or
  1 bins; `HISTOcount` writes bin 0 of an empty histogram.
- `HISTOfindLastPeakInRegion` (792-845) reads bin -1 when no bin reaches I0, but returns -1 either
  way: the port does not read it and notes nothing.

## FS-23: `HISTOmakePDF` with another output (rule A)

`utils/histo.cpp` 1451-1467: with `h_dst` given it never copies `h_src`. Every caller passes the
same histogram twice (32 calls), or none.

## FS-24: `HISTOrobustGaussianFit` of one bin (hangs are never reproduced)

`utils/histo.cpp` 1706-1710: with `nbins == 1`, `min_bin = MAX(0, min_bin - 1)` and `max_bin =
MIN(nbins - 1, max_bin + 1)` stay 0 and 0, and the loop `while (min_bin >= max_bin)` never ends.

## Kept as FreeSurfer has them (rule C)

- `HISTOfindEndOfPeak` (1114-1131): `if (h->nbins >= b) b = h->nbins - 1;` (1128) always holds (b is
  at most `nbins`), so a peak whose next valley is not crossed below the threshold ends at the last
  bin. The evident intent `if (b >= h->nbins)` would end it after the valley, or right after b0 when
  there is none, which is not better; its callers' results depend on it: kept.
- `HISTOfindLinearFit` (1397-1423): `astep = MIN(amin, (amax - amin) / NSTEPS)` (1405), then at least
  0.01: `amin` is smaller only when `amin < (amax - amin) / 1000`, i.e. for `amin` below 0.01 (where
  the 0.01 floor wins anyway) or ranges over 1000 times `amin`: no caller's.
- `HISTOinit` (1543-1564) clears a given histogram but does not set its `nbins`: its callers
  (`mrihisto.cpp` 349-360, 1280-1292, ...) set it first.
- `HISTOfindBin` with `bin_size == 1` (1300) skips the bins: FreeSurfer's integer-bin shortcut.
- `DMatrixAlloc`'s message `types %d not recognized` prints `MATRIX_REAL` (`dmatrix.cpp` 349), not
  the type.
- The affine helpers' SSE branch (`include/affine.h` 23-25: `__GNUC__ > 3`, no `HAVE_MCHECK`, not
  `ARM64`) is the one built: the sum starts from the first product (the C branch starts from 0 and
  turns four `-0` products into `+0`).
- `noteMatrixAlloced` (`matrix.cpp` 117-146) returns at line 119 (`if (1) return;`): no
  statistics, no exit handler.

# Part 2 (item 3.1-24, first piece)

## The measurement, part 2

`check.py --seed 7 --n 62000` (2026-11-12): 62,000 cases of 62 commands, the 13 new ones
(`MatrixMultiply` real and complex; the element-wise operations on real matrices with zeros, values
around `1e-11`, infinities and NaNs; the same operations on complex matrices; the three region copies
with every combination of types and given or new destinations; `MatrixRealToComplex`; rotations;
the vector functions with `XYZ_NORMALIZED_LOAD` and `XYZApproxAngle_knownLength`; `Vector3Angle`;
`GaussianMatrix` and `GaussianMatrix2`; `MatrixSimilarityTransform`; `MatrixPrint` and
`MatrixPrintFmt`; `GetPrimes`, `GetPrimeFactors` and the closest factors; `PDFgaussian` after
`srand48`). The port with `reproduce`: **61,138 of 61,138 comparable lines identical**; 862 read
outside FreeSurfer's arrays (855 FS-22 as before, 7 region copies of a real source read as complex
past its end, FS-27/FS-16). No angle differed in its last bit (2,000 cases through `acos`, which the
port takes correctly rounded). The default differs on 4,035 lines, each with its defect's notice
(FS-25 1,000, FS-15, FS-14, FS-18, FS-19, FS-20, FS-26 6, FS-27 4, FS-28 3, FS-32 16), except 14
rotations (`sinf`/`cosf`), 3 Gaussian draws and 6 entropies (`log`), where the correctly rounded
function moved a last bit. Seed 1, 620 cases: 607 identical, 13 not comparable; SHA-256
`ed7e3de2...` in `tests/fs_maths_official.rs`.

## C++'s `float` overloads (a porting trap, not a defect)

`utils/matrix.cpp` is C++: `<cmath>` overloads `exp`, `sin`, `cos`, `sqrt` for `float`, so
`GaussianMatrix`'s `exp(-(d * d) / (2 * var))` (3586, 3596; `d`, `var` floats) is `expf` and its
`/ f` a float division, and `MatrixReallocRotation`'s `cos(angle)`, `sin(angle)` (1937-1938, `angle`
a float) are `cosf` and `sinf`. Evaluated in double (as the C reading suggests) 6 of the first 10
Gaussian mixtures differed in a last bit; with the `float` functions all agree. Every libm call of a
`float` argument in FreeSurfer's C++ files is the `float` function (`sqrt` of a float too, which
gives the same bits as the double one rounded).

## NaN bits (the porting guide's rule)

One `MatrixTrace` of a diagonal holding `inf`, `-inf` and a NaN gave `0xffc00000` in FreeSurfer
and `0x7fc00000` in the first port: `inf + -inf` is SSE's default NaN (sign set), and adding the
input's NaN then passes on the left operand's (the accumulator's) NaN, while LLVM had swapped the
operands. The float matrices' arithmetic now follows SSE2's rules explicitly
(`larmorx-freesurfer-core::sse`), with which all 1,000 `melt` cases agree. `histo` and `dmatrix`
(part 1) do not use them yet: their probe had no NaN inputs (left for the item's next piece).

## FS-25: real-only operations given a complex matrix (rule B; `MatrixClear` rule A)

`MatrixIdentity`, `MatrixTranspose`, `MatrixAdd`, `MatrixSubtract`, `MatrixScalarMul`,
`MatrixSquareElts`, `MatrixDiag`, `MatrixConstVal`, `MatrixReshape`, `MatrixTrace`, `MatrixSumElts`,
`MatrixIsSymmetric`, `MatrixIsZero`, `MatrixCheck`, `MatrixNormalizeCol*`, `VectorLen`,
`VectorDot`, `VectorDistance`, the cross and triple products and `Vector3Angle` index `rptr[r][c]`
whatever the type (e.g. 1102, 1131, 1165, 1193), the real-element access of a complex matrix
(part 1, FS-14): they read and write half the floats, shifted, with no message. They have no
complex code path, so the input makes no sense to them: rule B, a clear error by default
(`<function>: a complex matrix (the function handles real matrices only)`). `MatrixClear` (1232-1241)
is meant for any matrix (`MatrixIdentity` and `MatrixDiag` clear with it) and clears `cols + 1`
floats from `rptr[row]`, a complex matrix's first half-row and a little more: rule A, every float.
FreeSurfer's complex matrices come from `MatrixRealToComplex` and `MatrixAlloc(..., MATRIX_COMPLEX)`
(`MatrixInverse`'s complex path, `image.cpp`'s HIPS images): no program of recon-all or fMRIPrep
passes one to these functions. Measured: the 1,000 `mcplx` cases agree with `reproduce`.

## FS-26: `MatrixCopyRealRegion` copies the imaginary parts into a complex matrix (rule A)

1483-1485: `case MATRIX_COMPLEX: MATRIX_CELT_IMAG(mDst, drow, dcol) = MATRIX_CELT_IMAG(mSrc, ...)`,
the imaginary-part copy's text (1534-1536) pasted. Called that way by `MatrixInverse`'s complex path
(225-226, the two real blocks of its 2n x 2n real form: there `mTmp` is real, so the real case is
taken and FS-26 is not reached) and by nothing else known; default the real parts.

## FS-27: the part copies read a real source as complex (rule A); the region copy between types (rule B)

`MatrixCopyRealRegion` and `MatrixCopyImagRegion` read `MATRIX_CELT_REAL`/`_IMAG(mSrc, ...)`
whatever `mSrc`'s type (1481, 1532). `dct.cpp` 139-141 (`DCTcopy`, `mri_dct_align`) passes real
column vectors: each value is read from the next element's place, the last one past the array (heap
memory beyond the numbers, which `posix_memalign` does not clear). The input makes sense (a real
matrix's real part is itself): rule A. `MatrixCopyRegion` (1391-1439) copies with the destination's
access only (`RELT` for a real one, `CELT` for a complex one): between types it reads one layout
through the other, a request with no meaning: rule B.

## FS-28: outputs filled without checking that they fit (rule B)

The region copies check `rows > drows || cols > dcols` (1423, 1473, 1524) but not the destination offset: a
region from `(dest_row, dest_col)` beyond the destination's last row or column is written into the
next rows (inside the array) or past its end, and with no destination given only `(1, 1)` fits.
`MatrixTranspose` (1086-1106) writes `mOut->rptr[col][row]` into a given output of any size;
`MatrixIdentity` (1072-1084) writes `(i, i)` up to `n` into a smaller given matrix;
`MatrixRealToComplex` (1586-1602) checks neither `mImag` nor `mOut`; `MatrixNormalizeColScale`
(3428-3446) neither `scale`. A caller's mistake that FreeSurfer turns into silent corruption or a
crash: rule B. Measured: 3 of the 1,000 random region cases.

## FS-29: `PDFtodSeed`'s clock (nondeterminism)

`pdf.cpp` 32-41: `tv_sec + tv_usec` of `gettimeofday`. `mri_vol2vol`, `mri_glmfit`, `mri_mvglmfit`,
`mris_apply_reg`, `mri_segreg` (bbregister), `mri_gtmpvc` take it when no seed is given (their
`SynthSeed < 0` or `seed == 0`), and `RFspecSetSeed` (randomfields.cpp 103) for a negative seed:
no two runs agree. Nondeterminism is never reproduced by default: the fixed seed 1.

## FS-30: `PDFgaussian`'s zero radius (rule A)

`pdf.cpp` 49-60: the loop rejects only `r2 > 1.0`, so `r2 = 0` (both `2 drand48() - 1` exactly 0,
i.e. two draws of exactly 0.5) gives `0 * sqrt(-2 log(0) / 0) = 0 * inf`, NaN. Marsaglia's method
(and Numerical Recipes' `gasdev`) rejects `r2 = 0`. Possible only where the 48-bit state reaches
`2^47` twice in a row; not met in the measured draws.

## FS-31: unchecked vector lengths (rule B)

`VectorCrossProduct` (2534) and `VectorTripleProduct` (2586) test `v1->rows != 3 && v1->cols != 1`
(meant `||`): a 1- or 2-row column vector passes and `VECTOR_ELT(v, 3)` reads a row pointer beyond
the array; `v2`, `v3` and `vdst` are never tested. `VectorDot` (2303-2312) and `VectorDistance`
(2342-2355) loop over `v1->rows` and read `v2` as far. Rule B. (A 1 x 3 row vector is refused by the
test, as FreeSurfer refuses it: kept.)

## FS-32: the closest prime factor of 1 (rule A)

`prime.cpp` 188-209 and 214-235: `GetPrimeFactors(1)` returns an empty list (`calloc(0, ...)`), and
`dmin = abs(pfactors[0] - P)` and `fclosest = pfactors[0]` read past it; glibc's `calloc` clears the
whole minimal chunk, so FreeSurfer gets 0 (measured in the 16 probe cases with N = 1). Reached by
`mri_surf2surf` (1179) and `mri_vol2surf` (815-818) for a surface of one vertex, which then reshape
by a factor of 0. 1, no reshaping, is the sensible answer: rule A.

## Kept as FreeSurfer has them (rule C), part 2

- `VectorDistance` returns the **squared** distance (2342-2355): its callers use it so.
- `GaussianMatrix`'s `norm == 1` scales by the centre row's sum, not each row's (FreeSurfer's own
  header comment before 3563 and the comment at 3581-3582 say so and add `norm == 2` for the other).
- `MatrixDiag` clears a given output before checking its size (1372-1376): a wrong-sized output
  comes back cleared as well as refused.
- `MatrixPrint` flushes standard output, not its own stream (878).
- The cross product's test refuses a 1 x 3 row vector.

