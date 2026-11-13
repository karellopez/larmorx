# FreeSurfer 8.2.0's mathematics (`fs.maths`): findings

**Closed to FreeSurfer clean-room implementers** (source-derived: it names FreeSurfer's files and
lines; the behaviour is in `specs/freesurfer-maths.md`). Written by the replica author of queue
items 3.1-23 and 3.1-24 (FreeSurfer's foundations, track A). Defect IDs FS-13 to FS-24 (this
part 1), FS-25 to FS-33 (part 2, items 3.1-24 and 3.1-23's inverse), FS-34 to FS-39 (part 3) and FS-40 to
FS-49 (part 4, the rest of item 3.1-24) and FS-50 to FS-51 (part 5, the histograms' NaN and infinite inputs), decided by rules A to D (CLAUDE.md, D13); the
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

After main's VNL merge (2026-11-13), with `MatrixInverse`, `MatrixDeterminant` and
`MatrixQRdecomposition` (two more commands, `minv` and `mqr`: real matrices of 1 to 7 rows, a sixth
of them singular, so the closed forms and `vnl_qr<float>`'s inverse are both reached; invertible
complex ones of 1 to 3 rows, whose real form of up to 6 rows goes through `vnl_qr<float>`):
`check.py --seed 7 --n 64000`: **63,173 of 63,173 comparable lines identical** with `reproduce`
(827 not comparable: 817 FS-22, 10 FS-16); the default differs on 4,271 lines, all with notices
but 19 rotations, 4 Gaussian draws and 10 entropies (the correctly rounded `sinf`, `cosf`, `log`).
Seed 1, 640 cases: 627 identical, 13 not comparable, SHA-256 `bb0da3da...`. A singular complex
matrix crashes FreeSurfer's probe (FS-33), so the generator keeps them out.

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

## FS-33: 0 x 0 matrices and singular complex inverses (memory errors and a crash: never reproduced)

`numerics.cpp` 1091-1153 (`OpenLUMatrixInverse`): `if (r <= 4 && r == cols)` with `r == 0` falls to
the `else` branch, `vnl_matrix_fixed<float, 4, 4> m(vnlMatrix)`, which copies 16 floats from the
empty matrix's data (VNL's assertion compiled out), and `copy_out(oInverse->data)` writes 16 floats
into an output with none. 1198-1219 (`OpenMatrixDeterminant`): 0 rows is not 1 to 4, so
`vnl_determinant` runs `vnl_qr` on the empty matrix and reads its factor's `(0, 0)`. `matrix.cpp`
240-242 (`MatrixInverse`, complex): `mInv = MatrixInverse(mTmp, NULL)` is NULL for a singular
matrix and is copied from at once (`MatrixCopyRegion(mInv, ...)` reads `mInv->rows`): a crash. The
memory errors depend on the heap: both modes give the empty inverse (what FreeSurfer returns unless
that memory makes a zero determinant) and the determinant 1 (the empty product) by default, 0 with
`reproduce`; the crash is never reproduced (nothing returned). No program is known to invert or take
the determinant of an empty matrix.

## Kept as FreeSurfer has them (rule C), part 2

- `VectorDistance` returns the **squared** distance (2342-2355): its callers use it so.
- `GaussianMatrix`'s `norm == 1` scales by the centre row's sum, not each row's (FreeSurfer's own
  header comment before 3563 and the comment at 3581-3582 say so and add `norm == 2` for the other).
- `MatrixDiag` clears a given output before checking its size (1372-1376): a wrong-sized output
  comes back cleared as well as refused.
- `MatrixPrint` flushes standard output, not its own stream (878).
- The cross product's test refuses a 1 x 3 row vector.


# Part 3 (item 3.1-23's minimizers; the double matrices' NaNs)

## The measurement, part 3

`validation/probes/fs-maths/` gained `powell`, `powell2` and `dfpmin` (`OpenPowell`, `OpenPowell2`,
`OpenDFPMin` on four seeded float costs given as C function pointers: a positive-definite
quadratic, Rosenbrock's, the quadratic rounded down to multiples of 1/q with the quadratic's
gradient, weighted absolute values; the iteration callback `powell_iteration_func`, the step
function and the user callback; every evaluation and callback hashed; standard output and error
captured byte for byte; `OpenDFPMin` in a child process, since FreeSurfer crashes there, FS-37),
and NaNs and infinities among the double matrices' inputs (`dmat`, `dmmul`, `dliststats`).
2026-11-13, seed 7, 67,000 cases of 67 commands: `reproduce` identical on 66,042 of 66,042
comparable lines; 958 not comparable (827 FS-22, 131 `dfpmin` lines with VNL's L-BFGS printout of
the vectors under `DIAG_VERBOSE`, ITK-106, which prints the double before each vector: a heap
header in FreeSurfer, 0 in the port; 906 of the 958 were identical anyway). The default differs only
with a defect's notice, but the known correctly rounded `sinf`/`cosf`/`log` last bits (33 lines).
All comparable `powell`, `powell2` and `dfpmin` lines identical (1,000, 1,000 and 869), including
every evaluated point and the printed text. FreeSurfer **crashed** in 29 of the 1,000 `OpenDFPMin`
cases (FS-37; 42 of 1,000 in an earlier draw of the cases). Seed 1, 670 cases (the recorded test,
`tests/fs_maths_official.rs`): 657 identical, 13 not comparable.

The official code's own variation: run 8 times on the same 67,000 cases, one `robust` line
differed between runs and one run crashed (signal 11) in another `robust` case: both are FS-22
cases (`HISTOrobustGaussianFit` reading outside its arrays, already not compared), whose reads
depend on the heap's layout. Since then `fsmaths` runs `robust`, like `dfpmin`, in a child process.

On x86 with AArch64's NaN emulated (`nan_trap.c`, `NAN_TRAP_ARM=1`, `NAN_TRAP_DIR` set): the
recorded test passes in debug and release builds; the probe's 67,000 lines are identical with and
without the trap in `fix`, and in `reproduce` but for 17 `dfpmin` lines of the verbose L-BFGS
printout (ITK-105/ITK-106, below; they are among those not compared).

## The double matrices' NaN operands (measured; a porting trap, not a defect)

With two NaN operands SSE2 returns the first source operand's, so the order GCC chose matters.
Measured on 3,000 cases with NaNs of both signs, payloads and a signalling NaN: `DMatrixAddMul`'s
`val += v * m[r][c]` is computed as `(m[r][c] * v) + val` (the element's NaN in the product, the
product's in the sum; not the left-operand order §7 of the spec found for the float matrices),
`val += m[r][c]` (factor exactly 1) as `val + m[r][c]`; `DMatrixMultiply`'s products as `m1 * m2`.
`DVectorDot` (dmatrix.cpp 712) is `fldl v2[i]; fmull v1[i]; faddp` (the official `dmatrix.cpp.o`):
`fld` makes a signalling NaN quiet, `fmul`'s memory operand keeps it signalling, and of a quiet and
a signalling NaN the x87 returns the quiet one, so `v1`'s signalling NaN loses to `v2`'s quiet one
(the port's first version, loading both, gave the signalling one's payload: 1 of 1,000 cases).
`DListStats` loads its values (`fldl`) and squares them in SSE; its minimum and maximum are
`minsd`/`maxsd` with the C comparisons' NaN behaviour. All 3,000 cases identical after the change.

## FS-34: the Powell wrappers write the directions through `NULL` (a crash, never reproduced)

`numerics.cpp` 1001-1003 and 1059-1061 accept `ioInitialDirection == NULL` (the identity), but
1018 and 1074 write the final directions through it unconditionally: a segmentation fault. Every
caller in the tree passes a matrix. Both modes write nothing (noted).

## FS-35: `fs_powell`'s return codes (rule C: kept)

VNL's Powell (ITK 4.13.2 `vnl_powell.cxx` 127, 175) returns `TOO_MANY_ITERATIONS` when its
iteration limit ends the loop and `FAILED_USER_REQUEST` only when `report_iter` asks to stop (173).
FreeSurfer's copy returns `FAILED_USER_REQUEST` in both places (`fs_powell.cpp` 192, 224) and
ignores `report_iter`. So `OpenPowell`'s `ErrorExit(..., "powell exceeding maximum iterations.")`
(1007) and `OpenPowell2`'s `powell exceeded maximum iterations` with return 1 (1076-1079) never
run. The change looks deliberate (with VNL's code, a program whose Powell reached 2000 iterations
in `OpenPowell` would exit); every caller (`mri_segreg`, `mrimorph.cpp`, `mri_hires_register`,
...) ignores `OpenPowell2`'s value. Kept in both modes, documented.

## FS-36: `OpenPowell`'s tolerance has no effect (rule C: kept)

`numerics.cpp` 991 `set_x_tolerance(iTolerance)`: VNL's Powell and FreeSurfer's copy read only
`ftol` (default 1e-10) for their stop, never `xtol`; `fs_powell.cpp` 95's comment says otherwise.
Every `OpenPowell` call (`mrimorph.cpp`'s, `mri_hires_register`'s, ...) therefore stops at a
relative change of 1e-10 of the cost, whatever it asks; with float costs that means "no change at
all". Honouring the tolerance would change those programs' results; nothing says which they were
tuned for: kept, documented.

## FS-37: `fs_lbfgs`' best point (rule A; its crash never reproduced)

`fs_lbfgs.cpp` 87-115: `best_f` starts at the first value but `best_x` stays empty until a value
below it; 166-171 on success `x = best_x` and `end_error_ = f` (the last value evaluated); 173-198
on failure the same only `if (best_x.size() != 0)` ("an DJ addition"). So:

- a run that converges without improving on its start (an accepted line-search step whose value
  equals the start's: L-BFGS's sufficient-decrease test `f <= f0 + 1e-4 stp g'd` holds with
  equality when the term is below `f0`'s last bit, common with float costs) returns **empty**
  parameters; `OpenDFPMin` then either runs the next pass on them (the bridge copies nothing, the
  cost function reads stale stack floats, `lbfgs_` dereferences the null data pointer) or converts
  them back (`ConvertFromVNLDoubleToFloat` reads the empty vector): a crash either way. Measured: 29
  of 1,000 probe cases (`CRASH` lines, signal 11). `mrimorph.cpp` 6743 has `if (iter == 0) break;
  // parms can be corrupted if no step taken`.
- a failed run without an improvement leaves `x` at the line search's last trial point (netlib's
  `mcsrch` returns with it) and `end_error_` at the previous pass's value;
- otherwise `end_error_` is the last value evaluated, which can be a worse trial point's than the
  best point returned.

Rule A: the starting point is the best until a smaller value, every return gives the best point and
its value. `reproduce` gives FreeSurfer's, but where it empties the parameters: there the starting
point, and `OpenDFPMin` stops (the crash is never reproduced). This changes `mri_em_register`'s
quasi-Newton passes (`mrimorph.cpp` 5728, 6719) where a pass ends without improvement.

## FS-38: `OpenDFPMin`'s passes (rule A)

`numerics.cpp` 885-896: `previousValue` is set once, from the caller's `*oFinalFunctionReturn`,
and the loop continues while `(previousValue - currentValue) / previousValue > iTolerance`; its
comment says "keep iterating if the error isn't significantly better", i.e. than the previous
pass's. As written the improvement only grows, so the passes repeat until one fails (or FreeSurfer
crashes, FS-37); a NaN ratio (a cost of 0) continues too. With FS-37 corrected, a pass that cannot
improve returns the same point and value forever, so the loop must compare with the previous pass:
rule A (the intent written beside it; without it the corrected driver would not end). Default:
continue only while the pass improved on the previous one by more than the tolerance relative to it
(a NaN stops); `reproduce`: FreeSurfer's. Noted where the decisions differ (70 of 1,000 probe cases).

## FS-39: `OpenDFPMin` at the evaluation limit (rule A)

`numerics.cpp` 905-924: `FAILED_USER_REQUEST` (`fs_lbfgs`' 2,000-evaluation limit, 189) matches
none of the branches: the parameters, the iteration count and the final value are not written,
and the code 9 is returned, which `mrimorph.cpp` ignores ("retval is not to be trusted"); its
`iter` is then an unset local. Rule A: written as for `ERROR_FAILURE`. Not met in the probe.

## For the main package's VNL (track C), found here

- VXL's `write50` (L-BFGS's printout under `IPRINT(2) = 1`, ITK-105) divides 0 by 0 for a vector of
  zeros with `reproduce`: x86 prints `-nan`, an AArch64 run of the port `nan` (17 probe lines under
  the emulation; they are lines ITK-106 already makes non-comparable). Using SSE's default NaN there
  (`-nan` on every platform) would make the port's `reproduce` output the same everywhere.
- On a cost that falls without end along a line, `vnl_bracket_minimum` runs into infinities and
  makes NaNs from them (`inf - inf`); their sign differs on AArch64, so the points handed to the cost
  function differ in their NaN bits (an earlier version of the probe's absolute-value cost, with
  cross terms, showed it in 17 Powell cases; the results were the same). FreeSurfer's costs are
  bounded; noted only.

# Part 4: the decompositions, the matrix files and the random-field specifications (item 3.1-24)

Defect IDs FS-40 to FS-49. The probe's eight new commands (`meig`, `msvd`, `msvdinv`, `mpinv`,
`mrdtxt`, `mrdasc`, `mrdbin`, `mwr`; `validation/probes/fs-maths/`) and the C library's conversions
(`validation/probes/fs-maths/scanf/`).

## The measurement, part 4

`check.py --seed 7 --n 75000` (2026-11-13): 75,000 cases of 75 commands (the eight new ones 1,000
each). The port with `reproduce`: **73,122 of 73,122 comparable lines identical**; 1,878 not compared
(`msvd` 464, `startpeak` 308, `robust` 278, `meig` 194, `nextpeak` 153, `dfpmin` 125, `mrdbin` 119,
`msvdinv` 74, `mpinv` 67, ...): FreeSurfer's reads of memory it does not own or never wrote (FS-22,
FS-16, FS-42, FS-49, ITK-106, ITK-110, ITK-111) and 313 matrices with infinite or NaN entries the
port refuses (FS-47, FS-48): on 97 of them FreeSurfer never returned (the probe's `TIMEOUT`), on 174
its line equals the port's all the same (the SVD's failure lines), on 42 FreeSurfer finished with
results the port does not compute. The default differs from FreeSurfer only with a defect's notice
(FS-43 381, FS-46 151, FS-44 95, FS-45 54 among the new ones), but for 8 rotations, 1 Gaussian draw
and 10 entropies (the correctly rounded `sinf`, `cosf`, `log`). FreeSurfer's crashes are the lines
`CRASH`, matched by the port's `reproduce` (FS-44: every tall `MatrixSVDInverse` whose decomposition
succeeds; FS-45, FS-46). Seed 1, 750 cases: 723 identical, 27 not compared, hash `6cebc7a8...`
(`tests/fs_maths_official.rs`), passing with AArch64's NaN emulated in debug and release builds.

## The float product's NaN operands (measured; a porting trap, not a defect)

`MatrixMultiply_wkr`'s real loop (`matrix.cpp` 760-770, `val += *r1++ * *r2`) is vectorised by
GCC 8.4.0 for 4 or more terms (the unit oracle's `matrix.cpp.o`): `mulps` with `m2`'s gathered
column as the destination (each product `b × a`), then the four lanes summed as `val + p0`,
`p1 + val`, `p2 + val`, `val + p3`; the terms after the last block, and all of 3 or fewer, as
`val + (b × a)`. The values are the same in any order, but of two NaNs SSE keeps the first
operand's, so a NaN sum's sign followed `m2` and the lane order: the probe's `MatrixNSConditionNumber`
of matrices with NaNs and infinities printed `nan` where the port's earlier `val + (a × b)` printed
`-nan` (the products of `inf × 0` make x86's default NaN, negative). Ported as measured
(`matrix::dot_float`). The double product (`MatrixMultiplyD`) is not vectorised: `(a × b)` then
`val + p`, as the port had it.

## FS-40: the singular values read to the row count (rule A)

`matrix.cpp` 1810-1824 (`MatrixSVDEigenValues`) and 2481-2486 (`MatrixConditionNumber`) loop
`for (row = 2; row <= rows; ...)` and `i < nevalues` (`nevalues = m->rows`) over `v_w`, a 1 x cols
vector: for more rows than columns they read past its numbers (the allocation's rounding: memory
`bzero` never cleared, `matrix.cpp` 1188-1189's `MatrixAlloc_new`). Where that memory holds 0, the
condition number is 1e8 ("something big") for every tall matrix, and the extra eigenvalues are
whatever it holds. Rule A: the cols values; the extra eigenvalues 0 (the `memset` before). Callers:
`mrisurf_metricProperties.cpp` 10607 and 11009, `mrisurf_defect.cpp` 1018 (`m_Q`: square 3 x 3
there), `mris_compute_layer_intensities` through `MatrixNSConditionNumber` (square products): no
tall matrix on a default path known. `MatrixConditionNumber` of fewer rows than columns fails the
decomposition and gives 1e8 (kept, rule C: the "something big" the callers test against a limit;
`MatrixNSConditionNumber` is the function for such matrices).

## FS-41: `OpenNonSymmetricEigenSystem`'s copy on the stack (a crash, never reproduced)

`numerics.cpp` 951: `double data[icData * icData]`, a variable-length array: above the default
8 MiB stack (about 1,024 rows) the program crashes. The callers' non-symmetric matrices are small
(3 x 3 covariances that are not exactly symmetric); both modes compute, noted.

## FS-42: `OpenEigenSystem` ignores VNL's failure (memory contents)

`numerics.cpp` 930-943 constructs `vnl_symmetric_eigensystem<float>` and copies `V` and `D`;
ITK 4.13.2's `vnl_symmetric_eigensystem_compute` (`vnl_symmetric_eigensystem.hxx` 114-118) writes
`ierr = <i>` and returns before filling `V` when EISPACK's `tql2` does not converge, and the
constructor ignores it: `V` is a fresh, uninitialised `vnl_matrix`. Rule B: an error by default
(`MatrixEigenSystem` returns `NULL`, its documented failure); `reproduce` fills the vectors with 0.
Not reached by finite matrices in the probe (FS-47 refuses the non-finite ones first).

## FS-47: VNL's symmetric eigensystem never returns on non-finite matrices (a hang, never reproduced)

EISPACK's `pythag` (`v3p/netlib/eispack/pythag.c`, VNL) iterates `t = r + 4` until `t == 4`: with
`p = max(|a|, |b|)` NaN (f2c's `max` keeps the second argument when the comparison fails: `pythag(x,
NaN)` and `pythag(NaN, NaN)`, or two infinities, `r = inf / inf`) it never ends. `tql2` calls it on
the tridiagonal matrix of any matrix with infinite or NaN entries in the triangle `tred2` reads, so
FreeSurfer's `MatrixEigenSystem` of a symmetric matrix with infinities (a NaN fails the exact
symmetry test and goes to the non-symmetric path, which hangs in `balanc`, ITK-112) never returns.
Measured: of the probe's symmetric-path cases with infinities or NaNs, most never returned (the
probe's 2-second alarm, `TIMEOUT`); some did (a NaN only on the diagonal of a 2 x 2: `pythag(NaN, 1)`
returns, f2c's `max(NaN, 1)` being 1). Both modes refuse every such matrix with an error (rule B;
the hang never reproduced), so the matrices FreeSurfer does finish are not reproduced: they wait for
the main package's eigensystem to report a loop that never ends (below).

## FS-48: VNL's SVD never returns on some non-finite matrices (a hang, never reproduced)

LINPACK's `ssvdc` (VNL's `vnl_svd<float>`) loops for ever on some matrices with infinite and NaN
entries (probe case `mpinv 4 3 3 80000000 ff800000 3f800000 80000000 7fc00000 7f800000 80000000
7fc00000 7fc00000`: `MatrixSVDPseudoInverse` and `msvd` of the same matrix never return, FreeSurfer
and the main package's port alike); on most it fails to converge (VNL's `suspicious return value`
lines, `info` = min(rows + 1, cols) on every measured case) and on a few it succeeds (a 1 x 1
infinity). `OpenSvdcmp` hands any matrix to it. Both modes give the failure lines and
`ERROR_BADPARM` for every matrix with an infinite or NaN entry without decomposing it (rule B). The
callers' matrices are finite (Jacobians, covariances, design matrices).

## FS-43: `MatrixSVD`'s V and its ignored failure (rule A, rule B)

`matrix.cpp` 1779-1788: `mV = MatrixAlloc(mA->rows, mA->rows, ...)` where `vnl_svd`'s V is cols x
cols (`OpenSvdcmp` copies its cols² numbers into the first floats of the rows x rows matrix, so for
a tall matrix V's rows are misplaced), and `OpenSvdcmp`'s `ERROR_BADPARM` (fewer rows than columns,
or LINPACK's failure) is not checked: V comes back unchanged, mA and v_z too, with no message. The
function's own comment says V is cols x cols. Rule A (cols x cols) and rule B (an error). Callers on
default paths: `mri2.cpp` 1673 (`MRIpca`: the square temporal covariance matrix), `lta_convert` 1043 (a
given V): square or given; no change known on fMRIPrep's paths.

## FS-44: `MatrixSVDInverse` of a tall matrix (rule A; a crash, never reproduced)

`matrix.cpp` 1880-1912: `m_Ut = MatrixAlloc2(cols, cols, ...)` (its comment says cols x rows), the
loops `for (row = 1; row <= rows; ...)` over `v_w` (cols numbers) and `m_w->rptr[row][row]` (a cols x
cols matrix): for more rows than columns the row pointers past the last row are 0 (`bzero` of the
pointer block's padding) and the write crashes; measured: every tall case whose decomposition
succeeded gave `CRASH`. Rule A: the decomposition's sizes, which give the pseudo-inverse. Callers'
matrices are square (`MatrixPseudoInverse`'s `mᵀm`, covariances, `rbf.cpp`'s `GᵀG`).

## FS-45: `MatrixPseudoInverse` of a wide matrix with an output (rule A; crashes, never reproduced)

`matrix.cpp` 2796-2804: for fewer rows than columns the caller's `m_pseudo_inv` (cols x rows, the
result's shape) is passed to the pseudo-inverse of the transpose (rows x cols): `MatrixMultiply`
refuses it (`m3 dim mismatch`, `NULL`) and `MatrixTranspose(NULL, NULL)` crashes; an output of the
inner shape is filled, then freed by `MatrixFree(&mT)` (the caller's pointer dangles) and a new
matrix is returned. The same crash for a wide zero matrix with no output (its inner pseudo-inverse is
`NULL`). Callers passing an output (`mri_cnr`, `mri_train`, `mri_ms_fitparms`,
`mris_intensity_profile`) pass tall design matrices; no default path known.

## FS-46: `MatrixSVDPseudoInverse`'s output, failure and empty inner result (rule A, rule B; a crash never reproduced)

`matrix.cpp` 3613-3658: `m_pseudo_inv = MatrixMultiply(m_tmp, m_Ur, NULL)` ignores the caller's
matrix; `OpenSvdcmp`'s return is not checked (the singular values stay 0: `0 / 0` keeps every one,
`1 / 0` inverts them to infinities); for a wide zero matrix the inner `NULL` is transposed (a crash).
Every caller passes `NULL` (`mri_vol2surf` 2431, `class_array.cpp` 777, `gcamorph.cpp` 12350 and
12784). Rule A (the output written) and rule B (an error for the failure).

## FS-49: `freadInt` and `freadDouble` at the end of the file (memory contents)

`fio.cpp` 169-193: `fread` of a missing item leaves the local `d` or `i` as the stack held it (a
partial item keeps the bytes it read); `freadDouble` prints `freadDouble: fread failed`, `freadInt`
tests `fread(...) < 0`, which a `size_t` never is, and prints nothing. `MatrixReadFrom` of a short
file therefore takes its type, sizes or elements from the stack (the probe's cut files: memory
contents, counted and not compared; a garbage size can end the program, FS-13, so `mrdbin` runs in a
child process). Rule B: an error by default; `reproduce` reads the missing bytes as 0. The
readers that use them come with `fs.io` (item 3.1-26), where the same applies.

## The C library's `%f` (measured; larmorx's own reader)

glibc 2.35's `scanf` departs from C11's "longest prefix of a matching sequence" (7.21.6.2 ¶9)
in measured ways the readers depend on: `1e` and `1e+` read 1 (the marker and sign consumed, no
failure), so `100ergs` reads 100; `0x` without digits fails unless a point follows (`0x.` reads 0
and leaves a following `p`); `inf` stops before a non-`i` letter, an unfinished `infinity` fails and
consumes the letter that broke it (as an unfinished `inf` or `nan` does; numbers leave it unread);
`nan(...)` is never read beyond `nan`; underflow sets `ERANGE` after rounding (`0x1.fffffep-127` sets
it, `0x1.ffffffp-127` does not), and so does any inexact subnormal result, exactly representable
ones not; a conversion that meets the end of the input before its item sets `errno` to 0, or, when
an earlier item of the same call ended at the end of the input, to the value `errno` had when that
item met it (before that item's own `ERANGE`: `sscanf("-1877   1e40", "%f %f %f %f")` with `errno` 7
before leaves 7, not 34; `"1e-50 3"` leaves 34; measured by track C's
`validation/probes/fs-volume/scanf_word_errno.c`). Every later attempt of the same call to read sets
`errno` back to that value, white space at the format's end included (`sscanf("1e40", "%f ")` with
`errno` 7 leaves 7, without the blank 34; white space just before a conversion reads nothing itself);
a call whose only reading meets the end at once (`fscanf(fp, "\n")` at the end of a file) leaves
`errno` alone (track E's measurement, specs/freesurfer-maths.md §18.1: 9,008 `sscanf` calls, all
given by `cscan::Scanf`, `validation/probes/fs-maths/scanf_errno_cases.py`). This is why
`MatrixReadTxt`'s `could not scan value` after an `ERANGE` read prints no `Numerical result out of
range` when the file simply ended. `cscan.rs` reproduces each (its `EndRule` keeps the `errno` of the
moment the call first met the end, which gives this rule); `scanf_probe.c` and its companions are the
measurements.

## For the main package's VNL (track C), found here, part 4

- **EISPACK's `pythag` with NaN** (`larmorx_core::vnl_eigen::pythag`): it uses Rust's `f64::max` and
  `min`, which ignore a NaN, where f2c's `max(a, b)` is `a >= b ? a : b` (the second argument when
  the comparison fails) and `abs` keeps NaN: `pythag(x, NaN)` returns about `x √2` in the port and
  never returns in VNL; and the port's `pythag` too loops for ever on `pythag(NaN, NaN)` and two
  infinities, so `symmetric_eigensystem` hangs on such input (ImageMomentsCalculator, antsAI would as
  well). A version that follows f2c's macros and reports the loop (as `BalancNeverReturns` does for
  `balanc`) would let FS-47's matrices that FreeSurfer finishes be reproduced; a VXL defect ID is
  track C's.
- **LINPACK's `ssvdc` never returning** (`larmorx_core::vnl_svd_f32`): the port loops on the matrix
  above as VNL does. A report of the loop, and the `info` of a failure (`vnl_svd_f32` returns `None`
  without it; the double version has `vnl_svd_info`), would let FS-48 follow VNL exactly.

# Part 5: the histograms with NaN and infinite inputs (item 3.1-23, track A relay 7)

Defect IDs FS-50 and FS-51. The probe's histogram cases now carry NaNs (quiet and signalling, both
signs, with payloads) and infinities in about a third of the histograms (counts, bin values between
the ends, the bin size, minimum or maximum) and in the scalar inputs (samples, values to find, scale
and offset, thresholds, sigma), from a second generator that never draws from the first, so every
other command's cases are as before (`specs/freesurfer-maths.md` §22).

## The measurement, part 5

`check.py --seed 7 --n 75000` (2026-11-13): 75,000 cases of 75 commands. The port with `reproduce`:
**73,035 of 73,035 comparable lines identical**; 1,965 not compared (`msvd` 464, `startpeak` 314,
`robust` 306, `meig` 194, `nextpeak` 152, `dfpmin` 125, `mrdbin` 119, `msvdinv` 74, `mpinv` 67,
`nextvalley` 41, `prevvalley` 40, `cdf` 29, `abs` 24, `mregion` 9, `prevpeak` 7): memory FreeSurfer
does not own or never wrote (FS-22, FS-16, FS-42, FS-49, ITK-106, ITK-110, ITK-111; with NaN inputs
FreeSurfer's bin number -2147483648 reads or writes 8 GiB below the array: the probe's `CRASH` lines,
`HISTOabs` and the cumulative distributions writing, the robust fit and the searches reading) and the
313 non-finite matrices of FS-47 and FS-48 (as in part 4). The default differs from FreeSurfer only
with a defect's notice (among the histograms FS-12 456, FS-19 481, FS-20 311, FS-18 1,000, FS-50 117,
FS-17 77), but for 8 rotations, 9 entropies and 1 Gaussian draw (the correctly rounded `sinf`,
`cosf`, `log`). The non-finite subset alone (16,968 histogram cases with a NaN or an infinity):
16,575 of 16,575 comparable identical. Seed 1, 750 cases: 723 identical, 27 not compared, hash
`b6452da0...` (`tests/fs_maths_official.rs`), passing with AArch64's NaN emulated (nan_trap.c,
`NAN_TRAP_ARM=1` with `NAN_TRAP_DIR`) in debug and release builds; the port's lines for the 16,968
non-finite cases are the same with and without the emulation in both modes.

## The histograms' NaN operands (measured; a porting trap, not a defect)

GCC 8.4.0 compiled `histo.cpp`'s arithmetic with commutative operands swapped and some loops
vectorised (the unit oracle's `histo.cpp.o`); the values are the same, but of two NaNs SSE keeps the
first operand's, and Rust's `+` leaves the order to LLVM (it differs between builds and targets), so
every operation where two NaNs, an infinity and its negative, or 0 and an infinity can meet goes
through `sse` in the official order (`specs/freesurfer-maths.md` §21):

- `HISTOadd` (`histo.cpp` 402-424, `*pcdst++ = *pc1++ + *pc2++`): with a separate output and 4 or
  more bins, `addps` with `h1`'s counts as the destination, then the 1 to 3 bins left `h2 + h1`,
  `h2 + h1`, `h1 + h2` (the tail's loads alternate); with the output one of the inputs (the alias
  check of the vectorised loop fails) or 3 bins or fewer, one bin at a time `h1 + h2`. FreeSurfer's
  callers add in place (`HISTOadd(h1, h2, h1)`: `mrisurf_mri.cpp` 8163, 8175, 8994, 9006,
  `mrihisto.cpp` 169; `HISTOadd(h1, h2, h2)`: `mrihisto.cpp` 1328, `gca.cpp` 15590, 15600, 16513,
  16522): `histo_add_into_first`, `histo_add_into_second`.
- `HISTOtotalInRegion` (1250-1260) and `HISTOmakePDF`'s total: vectorised sums in blocks of four,
  `T + c0`, `T + c1`, `c2 + T`, `c3 + T` and `c0 + T`, `T + c1`, `c2 + T`, `T + c3` (`sse::sum_blocks`).
- `HISTOlinearScale` (1425-1437): `max` as `offset + (max × scale)` and the bin size `scale ×
  bin_size`; `HISTOsoapBubbleZeros` (1886-1917): the middle bins' `(c[b+1] + c[b] + c[b-1]) / 3` as
  `((c[b] + c[b+1]) + c[b-1]) / 3`; `HISTOcorrelate` (1336-1362) and
  `histoComputeLinearFitCorrelation` (1364-1394): the products in the destination (`(c2 × c1) + Σ`);
  `HISTOfillHoles`' and the soap bubble's `/ 2` as `× 0.5`.
- `HISTOabs`' bin values `b × bin_size` with an infinite bin size: `0 × ∞` is the default NaN (sign set
  on x86, clear on AArch64): through `sse` on every platform, as `HISTObins` and `HISTOinit` for one bin.

One translation error the NaNs found: `HISTOfindCurrentPeak`'s backward scan (1179-1196) skips a bin
with `if (center_val < min_count) continue;`, which the port had written as `center >= min_count`:
the same for numbers, but a NaN count is a candidate in FreeSurfer (its comparisons with the window
all false, so it is a peak). Fixed (37 of 16,968 non-finite cases had differed).

## FS-50: `HISTOabs` of an infinite range (rule B: a clear error)

`histo.cpp` 1818-1842: `HISTOrealloc(habs, nint(ceil((mx - mn) / h->bin_size) + 1))` with an
infinite largest absolute value (an infinite bin value), or a bin size of 0 or NaN, converts an
infinity or a NaN to `int` (undefined; `cvttsd2si` gives -2147483648, FS-12): `calloc` of
-2147483648 floats fails and `HISTOalloc` ends the program, `Cannot allocate memory` and
`HISTOalloc(-2147483648): could not allocate histogram`, status 254 (measured: the probe's `EXIT 254`
lines, `abs` and the cumulative distributions, which start with `HISTOabs`). FS-12's correction
(saturation) would ask for 2³¹ bins, 16 GiB (measured: an unboxed run reached about 70 GB before the
correction existed). The default ends the program with an error that names the cause, `HISTOabs:
absolute values from <mn> to <mx> in bins of <bin_size> need <n> bins: outside the int range (an
infinite range, or a bin size of 0 or NaN)`, status 252 (`ERROR_BADPARM`); `reproduce` gives
FreeSurfer's. A NaN bin value is never the largest or smallest (`fabs(NaN) > mx` is false), so it
never causes this.

## FS-51: the robust Gaussian fit's search over sigma that cannot advance (a hang, never reproduced)

`histo.cpp` 1717-1748: `for (sigma = delta_sigma; sigma <= max_sigma; sigma += delta_sigma)` with
`delta_sigma = h->bin_size / 10` and `max_sigma = habs->nbins * habs->bin_size / 2`: with an
infinite bin size both are infinite (the bin size `<= 0` check lets it through), `inf <= inf` holds
and `inf + inf` stays infinite, so FreeSurfer never returns (measured: the probe's `TIMEOUT` lines,
4 of seed 7's robust fits); a bin size so small that a tenth of it rounds to 0 stops the search the
same way. Both modes (the hang is never reproduced): the error `HISTOrobustGaussianFit: the bin size
<s> gives no search over sigma (steps of <d>)`, status `ERROR_BADPARM`, the offset set and sigma
not. FreeSurfer's callers (`mri_normalize.cpp` 1846, `gca.cpp` 11677, `mrisurf_mri.cpp` 8169, 8177,
9000, 9008, `gcamorph.cpp` 16965, `mrifilter.cpp` 5508: histograms of intensities) pass finite bin
sizes, so no default path is known to reach it.

## The probe's fork and the shared file offset (a probe finding)

Every histogram case now runs in a child process (`fsmaths.cpp`'s `run_forked`). Where FreeSurfer's
code ends the child with `exit()` (FS-50's `HISTOalloc`), glibc's exit flushes the stdio streams the
child inherited, and the flush of an input stream sets the file offset to the stream's logical
position; the offset is shared with the parent, whose stream had read ahead, so the parent read the
following cases again (each `EXIT` line followed by a repeated one, every later line misaligned).
`fsmaths` now reads every case before the first child (the earlier forked commands never ended
with `exit()`, so their measurements stand).

## `XYZApproxAngle_knownLength`'s third product (a porting mistake, fixed; track A relay 12)

Track D's surfaces (`metric.rs`, relay 7) found that the port multiplied the dot product's third
term as `n.z * z2` where the official build does `z2 * n.z`: in the unit oracle's `matrix.cpp.o`
(`objdump -d`) the first two products are `mulsd` with the normal's component as destination
(`n.x * x2`, `n.y * y2`) but the third has `z2` widened into the destination (`mulsd %xmm1,%xmm2`
with `%xmm2 = (double)z2`). SSE returns the destination's NaN when both operands are NaNs, so the
two orders differ only where `n.z` and `z2` are NaNs of different payloads or signs and the first
two products are not NaN. The rest of the function as the object code does it: the sums left to
right, `fabs(dot) > norm` as `maxss` (the norm when either is NaN), the float division `dot / norm`,
the comparison with 0.99 in float (`comiss`: a NaN takes the square-root branch), `2 (1 - x)` as
`(1 - x) + (1 - x)` (the same bits, NaNs included), glibc's `acos` and `sqrt` called. The port now
takes `z2` first. The probe's `mvec` command could not see it (its normal comes from
`XYZ_NORMALIZED_LOAD`, which makes all three components NaN together); the new command `xyzang`
(an arbitrary normal and vector, NaNs of six payloads) has 18 of its 1,000 seed-7 cases where the
order decides the bits. Measured: seed 7, `xyzang`, `mvec` and `v3angle`, 3,000 of 3,000 lines
identical with `reproduce`, the port's lines identical with AArch64's NaN rules emulated (both
modes); seed 1, 760 cases of 76 commands: 734 identical, 26 not comparable, hash
`2b67d14767130507cd9f5804c61af52fd736629169f68c74b6beaeb9821d338d` (`tests/fs_maths_official.rs`),
passing under `NAN_TRAP_ARM=1` with `NAN_TRAP_DIR` in debug (8,295 flips) and release (8,299).

## FNV: FreeSurfer's side of the FNV hashes (track A relay 14)

- `include/fnv_hash.h` (Bevin Brett) and `include/fnvhash.h` keep the hash in `unsigned long`
  (`hash *= 16777619L`, `value *= prime`): FNV-1a 32's basis and prime with a 64-bit accumulator on
  LP64 (`larmorx_core::fnv::fnv1a_32_wide`). On Windows' LLP64 `unsigned long` has 32 bits, so a
  Windows build of FreeSurfer would hash differently; the replica follows the Linux build everywhere.
- `fnv_hash` hashes `unsigned int combined[3] = {i, k, (*random_counter)++}` by its memory
  (`(unsigned char*)&combined[0]`, 12 bytes, little-endian on x86-64); the `int` counter's increase
  past 2^31 - 1 is undefined in C++ (GCC's `addl` wraps): the replica wraps.
- `(double)(hash & 0xffffffff) / (double)0xffffffff` then `(HI-LO)*f + LO` in `double` (x86-64's baseline,
  FreeSurfer's flags, has no FMA: a product then a sum, each rounded), then the conversion to `float`;
  the probe's `fnvh` lines, built with the same flags, agree bit for bit.
- `FnvHash::write` opens an `std::ofstream` and writes `value << std::endl` (or `<< " | " << suffix`);
  a failed open is silent.
- Measured (2026-11-13, the probe's `fnvh`): seed 7, 1,000 cases identical with `reproduce` and
  `fix`; the probe's official hash re-recorded (seed 1, 770 cases of 77 commands, 747 identical, 23
  not comparable, ae473ef0...; it was 2b67d147... with 760 cases of 76 commands before `fnvh`).
- Users: track D's surface metric properties (`mrisurf_metricProperties_*`: `fnv_hash` for the random
  moves, `fnv_add` for `mrisVertexHash`; D keeps local copies until it switches), `mri.cpp`'s
  `MRI::hash`, `realm.cpp`.

## `AffineMV`'s and `AffineMM`'s NaNs (track A relay 15, from track C's relay 11)

`include/affine.h`'s SSE branch is `static inline`: its compiled order is that of each place it is
inlined. Track C measured `MRIcalcCRASforExtractedVolume`'s copy (`objdump -dr` of the unit oracle's
`mri.cpp.o`, docs/findings/freesurfer-volume.md §3): each product `mulps` with the vector's broadcast
number in the destination register, each sum with the running sum there, the fourth product (by 1)
folded away (`x + c3` has the bits of `x + c3 * 1`: a NaN `c3` is quieted either way). The translation
used plain Rust arithmetic, whose NaNs made from numbers (`inf - inf`, `0 * inf`) take AArch64's sign
under the ARM emulation. Now `crate::sse` in that order (`affine.rs`'s `combine`); `AffineMM` takes the
same order (not measured: no inlined copy measured yet). Test: `tests/affine_nan.rs`, 20,000 seeded
products of matrices and vectors with zeros, infinities and NaNs of both signs and with payloads,
hash `0x5297abf083b3d8b8`, passing in debug and release and under `NAN_TRAP_ARM=1` with
`NAN_TRAP_DIR` (287,019 NaNs corrected by the emulation).

## The register's texts before the open-files rewrite (2026-11-13)

The orchestrator's rule of 2026-11-13 ("open files carry behaviour only", the foundations' brief):
`crates/larmorx-core/src/defects.rs` is open to FreeSurfer's clean-room implementers, so its texts
describe only what a caller sees (inputs, outputs, messages, exit status). The texts it held
before, with the mechanisms they named (pointers, which function passes what, the libraries
underneath, the casts), are kept here verbatim; the sections above give their evidence.

- **FS-13**: MatrixAlloc of a negative size, or of more numbers than an int holds, crashes or corrupts memory after printing that it uses an mmap'd file (the new allocator reads a file pointer it never set or clears memory with a wrapped size), or exits
- **FS-14**: MatrixCopy copies a complex matrix float by float through its real-element pointers: of each row it copies as many floats as the matrix has columns, starting at the float before the row's first pair, so half the values are lost and the rest are shifted
- **FS-15**: MatrixMultiply and MatrixMultiplyD of complex matrices add the product onto the output's earlier contents (they never clear the output): a given output, or one that is also an input, gives a wrong product
- **FS-16**: FreeSurfer's float matrices are addressed outside their arrays where types and sizes do not match (a complex product written into a real output writes past its end): memory the program does not own
- **FS-17**: FreeSurfer's matrices and histograms convert floats and doubles beyond the int range, and NaN, to int with no check (bins, counts compared as integers, floor of a vector), an undefined conversion that gives -2147483648 on x86-64
- **FS-18**: HISTOclearCounts clears only the first count of the histogram (and, making a new histogram, copies only the first bin's value): the counts it is meant to reset keep accumulating
- **FS-19**: HISTOaddSample decides where a sample outside the range goes by comparing its bin number with the range's lower intensity: below a negative lower intensity it is dropped, above the range with a lower intensity beyond the bin count it is counted in the first bin; a NaN sample is counted in the first bin
- **FS-20**: HISTOaddSample gives a sample exactly at a bin's value the weight 0 in both neighbouring bins, so it is not counted at all (with a bin size of 1 and integer intensities, no sample is counted)
- **FS-21**: HISTOcopy, HISTOadd, HISTOsubtract and HISTOcumsum write past the arrays of an output histogram that has fewer bins allocated than the result: memory the program does not own
- **FS-22**: FreeSurfer's histogram searches and helpers read and write bins outside the histogram's arrays when given a start bin outside it (a search's -1 passed on), when a peak window reaches past an end, and for histograms of 0 or 1 bins: memory the program does not own
- **FS-23**: HISTOmakePDF given an output histogram other than its source normalises the output's own counts instead of the source's
- **FS-24**: HISTOrobustGaussianFit of a histogram of one bin never returns: the loop that widens its window cannot widen it
- **FS-25**: FreeSurfer's real-only matrix operations (identity, transpose, sums, differences, scalar products, squares, diagonals, constants, reshapes, traces, element sums, symmetry and zero tests, column norms, vector lengths and products) address a complex matrix's floats as if it were real, and MatrixClear clears only part of a complex matrix: wrong values, no message
- **FS-26**: MatrixCopyRealRegion into a complex destination copies the source's imaginary parts instead of its real parts
- **FS-27**: MatrixCopyRealRegion and MatrixCopyImagRegion read a real source as if it were complex (DCTcopy passes real vectors: each value comes from the next element, the last from beyond the array), and MatrixCopyRegion between a real and a complex matrix addresses one through the other's layout
- **FS-28**: matrix functions that fill a caller's output do not check that it fits (the region copies' destination region, MatrixTranspose's and MatrixRealToComplex's output, MatrixIdentity's given matrix, MatrixDiag's and MatrixNormalizeColScale's): elements are written into other rows or outside the array
- **FS-29**: PDFtodSeed seeds from the clock (seconds plus microseconds): the programs and random fields that take it when no seed is given never repeat a run
- **FS-30**: PDFgaussian divides zero by zero when both of its uniform draws give exactly 0 (drand48 returned 0.5 twice): it returns NaN
- **FS-31**: FreeSurfer's vector functions do not check their vectors' lengths (VectorCrossProduct's and VectorTripleProduct's test lets column vectors of one or two elements through and never tests the other vectors; VectorDot and VectorDistance read the second vector to the first's length): elements read outside the vectors
- **FS-32**: GetClosestPrimeFactor and GetClosestPrimeFactorLess of 1, which has no prime factor, read the first element of the empty factor list (0 from glibc's calloc): mri_surf2surf and mri_vol2surf reshape a one-vertex surface by a factor of 0
- **FS-33**: MatrixInverse and MatrixDeterminant of a 0 x 0 matrix read memory VNL never allocated (the inverse goes through the 4 x 4 closed form and copies 16 floats past its output; the determinant reads the empty QR factor's first element), and MatrixInverse of a singular complex matrix crashes (it copies from the NULL inverse of its real form)
- **FS-34**: OpenPowell and OpenPowell2 accept no initial directions (a NULL matrix) but then write the final directions through it: a crash
- **FS-35**: fs_powell returns FAILED_USER_REQUEST where VNL's Powell, which it copies, returns FAILED_TOO_MANY_ITERATIONS (its iteration limit), so OpenPowell's exit and OpenPowell2's message and return value 1 for too many iterations never happen (kept: FreeSurfer's change keeps programs from exiting at the limit)
- **FS-36**: OpenPowell's tolerance argument sets VNL's x tolerance, which Powell's method never reads: every call stops at VNL's default relative tolerance on the cost, 1e-10 (kept: the programs' results depend on it)
- **FS-37**: fs_lbfgs records its best point only from the second evaluation on and reports the last value evaluated: without an improvement on the starting point a successful run empties the parameters (OpenDFPMin then crashes) and a failed one leaves them at the line search's last trial point with the previous run's end value; otherwise the end value can be a worse point's
- **FS-38**: OpenDFPMin repeats the minimization while the cost improved by more than the tolerance relative to the caller's first value, never to the previous pass's (its comment: keep iterating if the error is not significantly better), so it repeats until a pass fails or FreeSurfer crashes (FS-37)
- **FS-39**: OpenDFPMin returns FAILED_USER_REQUEST without writing the parameters, the iteration count or the final value when an L-BFGS pass reaches its evaluation limit: the progress made is dropped and the caller reads its own unset count
- **FS-40**: MatrixSVDEigenValues and MatrixConditionNumber read the singular values up to the matrix's row count where the decomposition has as many as its columns: for a matrix with more rows than columns they read past the vector (memory the program does not own), and the eigenvalues and the condition number (1e8 where that memory holds 0) depend on it
- **FS-41**: OpenNonSymmetricEigenSystem copies the matrix into a variable-length array on the stack (8 bytes an element): a non-symmetric matrix of about a thousand rows or more overflows the default 8 MiB stack, a crash
- **FS-42**: OpenEigenSystem ignores VNL's failure (EISPACK's tql2 not converging, as on a matrix with NaN entries): it returns success with eigenvectors VNL never wrote (memory contents)
- **FS-43**: MatrixSVD allocates V with as many rows and columns as the matrix has rows where the decomposition's V has as many as its columns (with more rows than columns V's numbers land in the wrong places), and ignores a failed decomposition (fewer rows than columns, or LINPACK not converging): V comes back unchanged, zeros for a new one, with no message
- **FS-44**: MatrixSVDInverse of a matrix with more rows than columns allocates the transpose of U as a square matrix and reads the singular values and writes their inverses up to the row count, past their vectors: FreeSurfer crashes or ends the program
- **FS-45**: MatrixPseudoInverse of a matrix with fewer rows than columns crashes when the pseudo-inverse of its transpose is NULL (a zero matrix) or cannot be written into the caller's output, which it passes on although that has the other shape; an output of the inner shape is filled, freed, and a new matrix returned
- **FS-46**: MatrixSVDPseudoInverse never writes a given output (it returns a new matrix), ignores a failed decomposition (a pseudo-inverse of infinities from the unset singular values), and of a zero matrix with fewer rows than columns crashes transposing the NULL inner result
- **FS-47**: OpenEigenSystem passes a matrix with infinite or NaN entries to VNL's symmetric eigensystem, whose EISPACK pythag never returns on a NaN or on two infinite arguments: MatrixEigenSystem of a symmetric matrix with infinite entries (and the helper's symmetric path with NaN ones) hangs
- **FS-48**: OpenSvdcmp passes a matrix with infinite or NaN entries to VNL's singular value decomposition, whose LINPACK ssvdc can loop for ever on such a matrix: the SVD functions (MatrixSVD, the SVD inverses and pseudo-inverses, the condition numbers) hang on some of them
- **FS-49**: freadInt and freadDouble at the end of the file return a value they never read (the stack's contents; freadInt prints nothing): MatrixReadFrom of a short file fills the matrix's type, sizes or elements with memory contents
- **FS-50**: HISTOabs of a histogram whose absolute bin values span an infinite range, or whose bin size is 0 or NaN, asks for a number of bins outside the int range: the undefined conversion gives -2147483648 on x86-64, the allocation fails and the program ends with 'could not allocate histogram' (a saturated conversion would ask for 2^31 bins)
- **FS-51**: HISTOrobustGaussianFit never returns when its search over sigma cannot advance: an infinite bin size (the search starts at infinity, below an infinite limit), or one so small that a tenth of it rounds to 0

## Relay 16 (track A, 2026-11-13): FS-13, FS-47, FS-48 and FS-49 under `reproduce`

- **FS-13 (decided by the orchestrator, rule B + D13):** the default exits for every negative size;
  `reproduce` gives FreeSurfer's matrix where `MatrixAlloc_new` makes one. Its block size is
  computed in `size_t`: 64 bytes of header, `(rows + 1) * sizeof(float *)` (a negative `int`
  converted to `size_t`: the sum wraps), rounded up to 64, then `((rows * cols) + 2) * (2 for
  complex)` floats (`int`), rounded up. It returns a matrix when the float count is not negative
  (so `bzero(data, nelts * 4)` is small) and neither the header's part nor the whole wraps to a size
  no allocation gives (rows of -16 or more; at -9 to -16 the header's part wraps to 0 and the
  numbers overlap the header, which a matrix with no rows to visit never reads). Measured with
  `validation/probes/fs-maths/matrixalloc_negative.cpp` (the unit oracle's `libutils.a`, each size in
  a forked child, every size from -20 to 3 with a negative one, both types): 724 of 1,120 sizes
  return, exactly those the rule names (the replica's `Matrix::negative`; its unit test checks all
  1,120); the others crash after `MatrixAlloc(<r>, <c>): Using mmap'd tmpfile` (`mat` NULL in the
  mmap branch) or in `bzero`. The old allocator (`FREESURFER_MatrixAlloc_old`) is not reproduced.
- **FS-47 / FS-48:** per track E's amended §17.1/§17.4: both modes refuse where `larmorx_core`'s
  `symmetric_eigensystem_f32_full` / `vnl_svd_f32_full` say VNL never returns; `reproduce` gives
  VNL's result elsewhere (FS-42 and LINPACK's `info` as for finite matrices). The fs-maths probe's
  port marks the hanging lines `HANG` (not compared) and compares the rest.
- **FS-49:** per track E's §18.2 (the stack slot shared by `freadDouble`'s `double` and
  `freadInt`'s `int` 4 bytes higher; `MatrixAlloc` between header and elements clobbers it): the
  replica's `matrix_io::Slot`. Lines whose numbers took a byte of memory FreeSurfer never wrote are
  marked `MEM49` (not compared); the others are compared.
- The fs-maths probe's recorded hash: seed 1, 770 cases, 748 identical (one more than before: an
  FS-49 line now compared), 22 `SKIP`.
