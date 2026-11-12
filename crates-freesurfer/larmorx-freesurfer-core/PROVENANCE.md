# Provenance: larmorx-freesurfer-core

**Replica: translated from FreeSurfer's source.** This crate holds `fs.core` (queue item 3.1-22) and `fs.maths` (3.1-23, 3.1-24) of the FreeSurfer
replica (`docs/freesurfer/waves.md` section 4): FreeSurfer's messages, diagnostics, program name, environment, version stamps, the ROMP switch, timers, paths, signals and the licence check's messages (`fs.core`); its matrices (float and long double), the numerics wrappers over VNL and Cephes, `fs_powell`, `fs_lbfgs`, histograms, statistics, PDFs and random fields (`fs.maths`). It is distributed under FreeSurfer's
licence (`LicenseRef-FreeSurfer`, `../LICENSE`: the FreeSurfer Software License with its required
preface), as part of the `larmorx-freesurfer` package (`docs/licensing.md`).

The upstream is FreeSurfer 8.2.0 (GitHub's `v8.2.0` archive, SHA-256
`bb1699f7655ebf0f3790db6a78af9532ea7c832c97fe5f71bfd9f0e9526d7e78`; `docs/freesurfer-plan.md`),
built for the official binaries with GCC 8.4.0 (`docs/freesurfer/variation.md`).

## FreeSurfer files and functions translated

| Rust module | FreeSurfer file | Functions and data |
|---|---|---|
| `globals` | `utils/Progname.cpp`, `utils/diag.cpp` 47-68, `utils/log.cpp` 55-58, `utils/hips.cpp` 4-5 | `Progname`; `Gprofile`, `Gvx` ... `Gvz`, `Gsx` ... `Gsz`, `Gdiag`, `Gdiag_no`, `Gdiag_no2`, `Gx` ... `Gz2`, `IMAGE_SIZE`; `errorlog`, `exceptions`, `Gerror`; `hips_cplxtor`, `hips_rtocplx`; the `static` memories of `DiagHeartbeat` and `DiagShowPctDone`; the C library's `errno` |
| `log` | `include/log.h`, `utils/log.cpp` | the error codes, `fs::fatal`, `fs::error`, `fs::warning`, `fs::debug`, `setErrorLog` and `throwExceptions` (as `Globals` fields), `writeToErrorLog`, `errorExit`, `ErrorPrintf`, `ErrorExit` (`ErrorReturn` is `ErrorPrintf` and a return); glibc's `strerror` texts (from glibc's documented messages); the `term` colours are always empty (`termAllowsColor` returns false) |
| `diag` | `include/diag.h`, `utils/diag.cpp`, `utils/fsinit.cpp` | the `DIAG_*` bits, `DIAG_VERBOSE_ON`, `assertFailed`, `DiagInit`, `DiagPrintf`, `DiagBreak`, `DiagHeartbeat`, `DiagShowPctDone`, `check_finite`; `FSinit` and `setLocale` as far as they are seen (below) |
| `machine` | `utils/machine.cpp` | `swapDouble`, `swapLong32`, `swapFloat`, `swapShort`, `swapInt` |
| `cmdargs` | `utils/cmdargs.cpp` | `CMDargNErr`, `CMDisFlag`, `CMDsingleDash`, `CMDnthIsArg` |
| `base` | `include/base.h` | `squaref`, `squared` |
| `random` | `utils/utils.cpp` 73-162, 1557-1573; `utils/numerics.cpp` 1256-1279 | `seedHasBeenSet`, `idum`, `nrgcalls`, `setRandomSeed`, `getRandomSeed`, `getRandomCalls`, `randomNumber` (with its `static laterTime`), `compute_permutation`; `OpenRan1` (its `static` seed and generator; item 3.1-24's file, brought forward: `randomNumber`'s only source) |
| `utils` | `utils/utils.cpp` 284-306, 347-488, 711-720, 788-808, 838-849, 930-1009, 1011-1149, 1169-1490, 1521-1580, 1582-1697, 1699-1744, 1749-1830, 1881-1968, 1990-1998 | `fgetl` (with `fgets`), `StrUpper`, `StrLower`, `FileNameOnly`, `FileExists`, `FileName`, `stricmp`, `FileNamePath`, `deltaAngle`, `devIsinf`, `devIsnan`, `devFinite`, `FileNameRemoveExtension`, `FileNameExtension`, `FileNameFromWildcard`, `getMemoryUsed`, `strcpyalloc` (a `String`'s copy), `ItemsInString`, `deblank`, `str_toupper` (as `StrUpper`), `sum2stddev`, `compare_ints`, `compare_floats`, `nunqiue_int_list`, `unqiue_int_list`, `most_frequent_int_list`, `CountItemsInString`, `GetNthItemFromString`, `kth_smallest`, `quick_select`, `median`, `nint`, `nint2`, `global_progress_range` and `exec_progress_callback` (no callback is ever installed on the programs' paths: not translated beyond that), `GetVmPeak`, `GetMemUsage`, `PrintRUsage`, `WriteRUsage`, `Pi`, `halfPi`, `fastApproxAtan2fWkr`, `fastApproxAtan2f`, `ReportEntry`, `reportEntry`, `spendTimeCheckingForLogicProblem`, `copeWithLogicProblem2`, `getEnvironVar`, `directoryExists`, `directoryIsWritable`, `getBaseTempDir`, `makeTempFile`, `makeTempDir`, `FindFirstMatch`; `include/const.h`'s `STR_LEN` |
| `version` | `utils/version.cpp` | `PLATFORM`, `COMPILER_NAME`, `COMPILER_VERSION` (the official build's), `ver_string`'s result, `argv2cmdline`, `VERuser`, `VERfileTimeStamp`, `VERcurTimeStamp`, `getVersion`, `getBuildStamp`, `handleVersionOption`, `getAllInfo`; `FS_VERSION`, `FS_BUILD_STAMP` and `__DATE__ __TIME__` as the official programs print them |
| `fsenv` | `utils/fsenv.cpp`, `include/fsenv.h`; `include/mri.h` 80-81 | `FSENV`, `FSENVgetenv` (the colour table read by the caller's function), `FSENVfree` (dropping the value), `FSENVgetSUBJECTS_DIR`, `FSENVsetSUBJECTS_DIR`; `BVEC_SPACE_SCANNER`, `BVEC_SPACE_VOXEL` |
| `timer` | `include/timer.h`, `utils/timer.cpp` | `Timer` and its methods, `TIMER_INTERVAL_END`'s line, `currentDateTime` |
| `romp` | `utils/romp_support.cpp` (with `ROMP_SUPPORT_ENABLED` off, as built), `include/base.h` 70 | `before_main`'s thread limit, `romp_omp_get_thread_num`, `ROMP_main_started`, `initMainTimer` and `rompExitHandler` (nothing seen); `_MAX_FS_THREADS` |
| `chklc` | `utils/chklc.cpp` | no translation: the replica checks no licence key (decision log 2026-10-19); `chklc`'s valid-key result only |
| `xdebug` | `utils/xDebug.cpp`, `include/xDebug.h` | `xDbg_PushStack`, `xDbg_PopStack`, `xDbg_SetStackDesc`, `xDbg_SetCurrentNote`, `xDbg_Printf` (output off), `DebugEnterFunction`, `DebugExitFunction`; `xDbg_knMaxStackDepth`, `xDbg_knMaxDescLength`, `mCurrentStackDepth`, the stack's arrays |
| `fio` | `include/fio.h` 95-112 | `TSV`'s fields, `ncols`, `nrows` |
| `matrix` | `include/matrix.h`; `utils/matrix.cpp` 61-147, 150-181, 298-587, 587-710 | `MATRIX`, `VECTOR`, `MatrixBuffer`, `MATRIX_RELT`, `MATRIX_CELT`, `MATRIX_REAL`, `MATRIX_COMPLEX`, `MATRIX_SYM`/`UPPER`/`LOWER`; `MatrixStats`, `stats_compare`, `matrixStatsExitHandler`, `noteMatrixAlloced` (nothing seen: it returns at once), `MatrixCopy`, `MatrixAlloc_old`, `MatrixAlloc_new`, `use_new_MatricAlloc` (as `Globals::matrix_alloc_old`), `MatrixAlloc_wkr`, `MatrixAlloc2_wkr`, `MatrixFree`, `MatrixMultiplyD`; FS-13 to FS-16 |
| `affine` | `include/affine.h` | `AffineVector`, `AffineMatrix`, `SetAffineVector`, `GetAffineVector`, `GetFloorAffineVector`, `SetAffineMatrix`, `GetAffineMatrix`, `AffineMatrixFree`, `AffineMatrixAlloc`, `AffineMatrixCopy`, `AffineMV`, `AffineMM` (the SSE branch, as built); FS-17's conversion |
| `dmatrix` | `include/dmatrix.h`; `utils/dmatrix.cpp` 290-850 | `DMATRIX`, `DVECTOR`, `DMatrixCheckDims`, `DMatrixAlloc`, `DMatrixFree`, `DMatrixMultiply`, `DVectorDot` (x87 `long double` through `larmorx_core::x87::F80`), `DMatrixPrintFmt`, `DMatrixPrint`, `DMatrixCompareFMatrix`, `DMatrixCopyFMatrix`, `DMatrixCopy`, `DMatrixTranspose`, `DMatrixAddMul`, `DMatrixScalarMul`, `DMatrixSubtract`, `DMatrixAdd`, `DMatrixConstVal`, `DMatrixZero`, `DMatrixMaxAbs` |
| `histo` | `include/histo.h`; `utils/histo.cpp` | `HISTOGRAM`, `HISTOGRAM2D`, `MAX_BINS`; the 55 reached functions: `HISTOfree`, `HISTOrealloc`, `HISTOalloc`, `HISTOcopy`, `HISTOclear`, `HISTOclearCounts`, `HISTOadd`, `HISTOsubtract`, `HISTOclearBins`, `HISTOsmooth`, `HISTOfindLastPeak`, `HISTOfindFirstPeak`, `HISTOfindValley`, `HISTOfindLastPeakInRegion`, `HISTOfindHighestPeakInRegion`, `HISTOplot`, `HISTOaddSample`, `HISTOfindPreviousValley`, `HISTOfindNextValley`, `HISTOfindNextPeak`, `HISTOfindPreviousPeak`, `HISTOfindStartOfPeak`, `HISTOfindEndOfPeak`, `HISTOfindCurrentPeak`, `HISTOfillHoles`, `HISTOtotalInRegion`, `HISTOtotal`, `HISTOclearZeroBin`, `HISTOfindBinWithCount`, `HISTOfindBin`, `HISTOgetCount`, `HISTOcorrelate`, `histoComputeLinearFitCorrelation`, `HISTOfindLinearFit`, `HISTOlinearScale`, `HISTOthreshSum`, `HISTOmakePDF`, `HISTObins`, `HISTOcount`, `HISTOvalToBin`, `HISTOinit`, `HISTOfindMedian`, `HISTOmakeReverseCDF`, `HISTOmakeCDF`, `HISTOrobustGaussianFit`, `HISTOabs`, `HISTOgetEntropy`, `HISTOsoapBubbleZeros`, `HISTO2DfindBin1`, `HISTO2DfindBin2`, `HISTO2DgetCount`, `HISTOcomputeFWHM`, `HISTOwriteTxt`, `HISTOsumNorm`, `HISTOcumsum`; `include/macros.h`'s `MIN`, `MAX`, `EVEN`, `FZERO`, `DZERO` as they evaluate; FS-17 to FS-24 |
| `matrix` (3.1-24) | `utils/matrix.cpp` 717-842 | `MatrixMultiply_wkr` (`MatrixMultiply`; the product's body shared with `MatrixMultiplyD`) |
| `matrix_ops` | `include/matrix.h` 193-297; `utils/matrix.cpp` 844-880, 892-926, 1072-1195, 1232-1272, 1359-1543, 1586-1602, 1673-1690, 1931-1963, 2289-2313, 2342-2356, 2444-2462, 2530-2551, 2582-2602, 2638-2774, 2848-2911, 3039-3065, 3162-3172, 3428-3501, 3543-3612 | `MatrixPrint`, `MatrixPrintFmt`, `MatrixIdentity`, `MatrixTranspose`, `MatrixAdd`, `MatrixSubtract`, `MatrixScalarMul`, `MatrixClear`, `MatrixSquareElts`, `MatrixDiag`, `MatrixCopyRegion`, `MatrixCopyRealRegion`, `MatrixCopyImagRegion`, `MatrixRealToComplex`, `MatrixIsSymmetric`, `MatrixReallocRotation`, `MatrixAllocRotation`, `VectorLen`, `VectorDot`, `VectorDistance`, `MatrixIsZero`, `VectorCrossProduct`, `VectorTripleProduct`, `Vector3Angle`, `XYZ_NORMALIZED_LOAD`, `XYZApproxAngle_knownLength`, `MatrixCheck`, `MatrixReshape`, `MatrixTrace`, `MatrixConstVal`, `MatrixZero`, `MatrixSumElts`, `MatrixNormalizeColScale`, `MatrixNormalizeCol`, `MatrixSimilarityTransform`, `GaussianMatrix2`, `GaussianMatrix`; the macros `VECTOR_ELT`, `RVECTOR_ELT`, `V3_X`/`Y`/`Z`, `V3_DOT`, `MatrixClone`, `VectorClone`, `X_ROTATION`/`Y`/`Z`, `XYZ`; FS-25 to FS-28, FS-31 |
| `matrix_ops` (3.1-23) | `utils/matrix.cpp` 183-296, 1604 | `MatrixQRdecomposition`, `MatrixInverse` (real and complex), `MatrixDeterminant`; FS-33 |
| `numerics` | `utils/numerics.cpp` 1091-1219 | `OpenLUMatrixInverse`, `OpenQRdecomposition`, `OpenMatrixDeterminant` (on the main package's `float` VNL: `larmorx_core::vnl_fixed`, `larmorx_core::vnl_qr::Qr<f32>`, item 3.1-07); FS-33 |
| `prime` | `utils/prime.cpp` 20-235 | `GetPrimes`, `GetPrimeFactors`, `CompareFactors` (as a sort of integers), `GetClosestPrimeFactor`, `GetClosestPrimeFactorLess`; FS-32 |
| `pdf` | `utils/pdf.cpp` 26-61 | `PDFtodSeed`, `PDFgaussian`; FS-29, FS-30 |
| `sse` | none (larmorx's own) | SSE2's NaN rules for the official build's arithmetic (the porting guide's "NaN bits are part of the output") |
| `ret` | none (larmorx's own) | the result of a function with an optional output (`T *F(..., T *dst)`) |
| `cstd` | none (larmorx's own) | the C library's `atoi`, `atof`, `sscanf`'s `%lx`, `printf`'s `%W.Pf`, x86-64's double-to-int conversion, written from the C standard and glibc's documented behaviour |
| `cglob`, `ctime`, `system` | none (larmorx's own) | the C library's `glob` and `fnmatch` (from POSIX), `gmtime_r`, `localtime` and `strftime("%c")` in the C locale (from POSIX's `TZ`, RFC 8536's TZif files and two behaviours of glibc 2.35 measured against it, `validation/investigations/freesurfer-core/ctime_probe.py`: 180,240 cases, 0 differ), `getpwuid(geteuid())`, `uname`, `getcwd` (from `/proc`, `/etc/passwd` and the environment); no C library source |
| `run` | none (larmorx's own) | what `larmorx-freesurfer` gives a program (queue item 3.1-01) |

Third-party code in these files (CLAUDE.md rule 3): none; each carries MGH's notice
(`Copyright © 2021 The General Hospital Corporation (Boston, MA) "MGH"`) or none (`Progname.cpp`,
`hips.cpp`, `fsinit.cpp`: FreeSurfer's own, Part B as far as MGH holds the rights,
`docs/freesurfer/third-party.md` §2).

Third-party code in the part 2 files: `kth_smallest` and `quick_select` are N. Devillard's 1998
public-domain code (Wirth's algorithm; Numerical Recipes' description; TP-07, the placement table's
P-06), translated here from FreeSurfer's copy as part of `utils.cpp`; the placement table puts
Devillard's own code in the main package (the permissive track, from its own upstream), which this
translation may then be checked against or replaced by. `OpenRan1`
calls VXL's `vnl_random` (TP-01, BSD-3-Clause), which this crate does not contain: it reaches the
main package's port (`larmorx_core::vnl_random`, queue item 3.1-04) through `random::Ran1Source`.

`utils.cpp`'s `DListStats` (lines 1832-1865) is in `utils` (`dlist_stats`, its sums in x87 `long
double` through `larmorx_core::x87::F80`); `OpenRan1` draws from `larmorx_core::vnl_random` by
default (`random::vnl_source`). Still to translate in `fs.core` (each comes with the module or
item that needs it): the unreached functions of these files (static reach, `code.md` §6:
`IntSqrt`, `StrReplace`, `FileType`, `FileNumber`, `mad`, `FindMatches`, `GetVmSize`,
`PrintMemUsage`, `SetProgressCallback`, `FSENVprintenv`, `xDbg_Init` and the other `xDbg_*`, ROMP's
statistics, ...).

## Changes from the originals

- Process-wide globals are fields of `Globals`, one value per program run.
- Where FreeSurfer exits, the functions return `log::Fatal`, which the program turns into its exit
  status (the code's low 8 bits).
- Variadic messages are formatted by the callers; the functions take the formatted text.
- (`fs.maths`) A function with an optional output takes it as `Option<&mut T>` and returns
  `ret::Ret` (the new object, the caller's, or `NULL`); where FreeSurfer's callers pass one object
  as input and output and the aliasing changes the arithmetic (`HISTOsmooth`), an `_in_place` form
  does what the C code does. Matrices keep FreeSurfer's array layout (two unused floats, one-based
  rows, complex pairs), so a mixed real/complex access reads the floats FreeSurfer's does.
- (`fs.maths`) `exp`, `log` and `pow` go through `larmorx_core::math::Libm` (correctly rounded by
  default, glibc 2.35's with `reproduce`); in the C++ files a function of a `float` argument is
  C++'s `float` overload (`expf`, `sinf`, `cosf`, `sqrtf`), as `<cmath>` makes it (`GaussianMatrix`,
  `MatrixReallocRotation`). `acos` is larmorx's correctly rounded one in both modes until glibc's is
  ported (queue item 3.1-08).
- (`fs.maths`, 3.1-24) The float matrices' arithmetic follows SSE2's NaN rules on every platform
  (`sse`); `PDFgaussian` draws from the C library's 48-bit generator through
  `larmorx_core::drand48` (POSIX's definition, `Globals::drand48`). Item 3.1-24 is in progress: still
  to come are `matrix.cpp`'s decompositions and the functions over them (the
  eigensystems, `MatrixSVD*`, `MatrixConditionNumber`, `MatrixNSConditionNumber`,
  `MatrixPseudoInverse`, `MatrixSVDPseudoInverse`), its text and binary files (`MatrixReadTxt`,
  `MatrixAsciiWriteInto`, `MatrixAsciiReadFrom`, `MatrixReadFrom`, `MatrixWriteTxt`),
  `numerics.cpp`'s VNL and Numerical Recipes wrappers (without its GSL functions, item 3.1-53 in
  `larmorx-gpl`, and Burkardt's spline, item 3.1-54) and `randomfields.cpp`.
- (`fs.maths`) Not translated: the unreached functions of these files (static reach: `HISTOdump`,
  `HISTOcrunch`, `HISTOinvert`, `HISTOnormalize`, `HISTOcompose*`, `HISTOfillZeros`, `HISTOmul`,
  `HISTO*PeakRelative`, `HISTOfindFirstPeakInRegion`, `HISTOcountPeaksInRegion`,
  `HISTOaddFractionalSample`, `HISTOclearBG`, `HISTOvalToBinDirect`, `HISTOvalToCount`,
  `HISTOwriteInto`, `HISTOreadFrom`, `HISTOgaussian*`, `HISTOscalarMul`, `HISTOscalarAdd`,
  `HISTOfindMaxDerivative`, the other `HISTO2D*`, `HISTOrmsDifference`, `HISTOearthMoversDistance`,
  `HISTOksDistance`, `HISTOerase*`, `HISTOisPeak`, `HISTOwrite`; `DMatrixTest`). Still to come in item 3.1-23: `fs_powell`, `fs_lbfgs`,
  `fs_cost_function` (VNL's Powell, Brent and L-BFGS, item 3.1-07).
- The defects FS-01 (a NUL byte after the C-style error messages), FS-02 (`PROFILE`'s undefined
  conversion) and FS-03 (`assertFailed`'s deliberate segmentation fault) are corrected by default
  and reproduced with `reproduce` (`docs/findings/freesurfer-core.md`).
- `FSinit`'s locale is not set (numbers are written as in the C locale, which FreeSurfer's chosen
  locales keep) and its warning about a missing locale is not printed; its one-thread default is
  reported (`diag::fs_init`) for the programs whose arithmetic depends on their thread count.
- `assertFailed`'s endless loop under `FREESURFER_HANG_ON_ASSERT` is not reproduced.
- Part 2: the defects FS-04 (unseeded numbers from the clock), FS-05 (`FREESURFER_SEED`'s
  uncaught exception), FS-07 (the version options' argument loop), FS-10 (empty integer lists) and
  FS-12 (`nint`'s undefined conversion) are corrected by default and reproduced with `reproduce`;
  FS-06 (the arctangent's lookup outside its table), FS-08 (`FSENVgetenv`'s unset memory), FS-09
  (the median of an empty list) and FS-11 (fixed stack buffers) are memory errors, filled
  deterministically in both modes with a notice (`docs/findings/freesurfer-core.md`).
- `OpenRan1`'s generator is installed by the program (`random::Random::set_source`) rather than
  being a `static` object; its thread-0 check is not needed (the replica draws on one thread).
- `Timer` reads the monotonic clock (FreeSurfer's `high_resolution_clock` is libstdc++'s wall
  clock); `getMemoryUsed` reads `/proc/self/status` itself instead of running `grep` and `cut`
  through a shell; `PrintRUsage` reads `getrusage`'s counters from `/proc`; the temporary names'
  six characters come from larmorx's own unpredictable choice instead of glibc's.
- The licence check is not translated (decision log 2026-10-19).
