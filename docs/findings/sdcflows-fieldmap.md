# sdcflows' field-map estimation: findings

What porting sdcflows' field-map model (field maps, part A: `lx.sdcflows`, `crates/larmorx-sdcflows`)
showed about sdcflows 2.17.1 and the libraries under it: the arithmetic a bit-exact port has to
follow, the defects found (SDC-01 to SDC-10) and how each was decided (rules A to D). sdcflows
is Apache-2.0, so this file is not restricted. The scripts behind every number are in
[validation/investigations/sdcflows-fieldmap](../../validation/investigations/sdcflows-fieldmap/README.md);
the API in [docs/api/sdcflows-fieldmap.md](../api/sdcflows-fieldmap.md).

## Versions: no step on these paths

- **sdcflows 2.15.1 (fMRIPrep 25.2.6) to 2.17.1 (the newest, ported):** `transform.py`,
  `interfaces/bspline.py`, `interfaces/fmap.py`, `utils/phasemanip.py`, `utils/tools.py` and
  `workflows/fit/fieldmap.py` are byte-identical. The release's changes are in part B's files
  (`fieldmaps.py`, the wrangler, `interfaces/epi.py`, `interfaces/utils.py`, PEPOLAR's
  workflow) and the reports.
- **scikit-learn 1.6.1 to 1.9.1:** `_solve_sparse_cg` and the solver resolution (sparse input,
  no intercept: `sparse_cg`) are the same. **SciPy 1.15 to 1.18:** `cg` does the same
  arithmetic (only `postprocess` removed), `make_system`, `_get_atol_rtol`, `csr_matvec`,
  `csc_matvec`, `_find_interval` are identical; `_deBoor_D` differs only for repeated knots
  and derivatives, never reached (sdcflows' knots are distinct, `nu = 0`). **OpenBLAS 0.3.30 to
  0.3.34:** `sdot.c` and the Haswell micro-kernel are identical.
- Checked rather than assumed: the coefficients larmorx fits are bit-identical to fMRIPrep
  25.2.6's own `*_coeff000` files of the seven oracle estimators (scikit-learn 1.6.1, SciPy
  1.15.2, numpy 2.2.6, run on the old laptop), and every check passes in both kept
  environments (SciPy 1.18.1/numpy 2.5.4 and SciPy 1.15.3/numpy 2.3.5).

## The arithmetic a bit-exact port follows

### numpy's float32 dot product is OpenBLAS's `sdot`, and the kernel matters

scikit-learn's ridge solver runs SciPy's `cg`, whose inner products are `np.dot` of float32
vectors: numpy's `FLOAT_dot` hands them to OpenBLAS's `sdot` and adds the float result to a
double, then rounds to float32. OpenBLAS's Haswell/Zen kernel keeps **32 partial sums** (four
256-bit registers of eight lanes, fused multiply-adds over `n & -32` elements), reduces them in
a fixed tree (each register's halves, then 4+5, 6+7, their sum, two horizontal adds), adds the
tail elements one by one in double (`dot += (float)(y[i] * x[i])`), and returns a float.
`sdot` never splits the work between threads (unlike `ddot`). A one-element vector is not sent
to BLAS at all (numpy's `cblasfuncs.c`: a plain float product). The fit's bits depend on that
kernel: fMRIPrep's image (conda-forge's OpenBLAS 0.3.30, `DYNAMIC_ARCH`) picks the **Cooperlake**
kernel (AVX-512) on the main machine and Haswell on the old laptop, where the oracle runs ran;
so the image is run with `OPENBLAS_CORETYPE=Haswell` for comparisons, and larmorx follows the
Haswell kernel on every platform, as for numpy's other BLAS paths ([numpy-blas.md](numpy-blas.md)).

### SciPy's `cg` and scikit-learn's `sparse_cg`

- scikit-learn keeps float32 (`validate_data` accepts float64 and float32), copies `X`
  (`_preprocess_data`, offsets 0, scale 1), and solves with `sparse_cg`: when there are more
  features than samples it solves the kernel system `(X Xᵀ + αI) s = y` on the samples and
  returns `Xᵀ s` (its `max_iter` is then ignored), else `(XᵀX + αI) c = Xᵀy`; `α` is a float32
  array; `cg(C, b, rtol=1e-4)` with `maxiter=None`.
- `cg`: `x0` zeros, `atol = max(0, 1e-4 · float(‖b‖))` in double (the norm itself in float32,
  `sqrt(sdot(b, b))`), at most `10 n` iterations, the identity preconditioner returning `r`
  itself (so `z` is `r`), each update a float32 temporary then the in-place operation. A zero
  right-hand side (an all-zero field) returns `b` itself, which is zero: harmless. The
  operator's products follow scipy's: `X @ v` row by row in ascending column order from +0
  (`csr_matvec`), `Xᵀ y` column by column in ascending sample order (`csc_matvec`).
- sdcflows fits up to three times on the same inputs, stopping when `max|coef| < 1e4`; the fit is
  deterministic, so the retries change nothing: larmorx fits once, with the same outcome,
  records and error.
- No "least-squares option" exists: `BSplineApprox` always uses `Ridge(alpha,
  fit_intercept=False)`; `ridge_alpha=0` is plain least squares through the same `cg`.

### The B-spline weights and scipy's sparse storage

- Per axis, sdcflows evaluates `BSpline(arange(-3, K + 3), eye(K + 2), 3)` (SciPy's
  `_find_interval` and de Boor's recursion in float64), keeps the knots within distance 2,
  rounds to float32 in a `lil_array` (which drops exact zeros) and multiplies the axes with
  `kron(kron(w0, w1), w2)`.
- **scipy's `kron` stores explicit zeros.** When its right operand has `2 · nnz ≥ rows · cols`,
  `kron` goes through BSR and stores that operand **densely**; the final CSR matrix
  (`sparse_hstack(...).tocsr()`) then holds explicit zeros (a 20 × 18 × 7 image on a 5 × 5 × 4
  grid: 71,136 zeros among 191,520 stored entries, `probe_colmat.py`). They matter only through
  `0 × inf = NaN`, but larmorx keeps exactly scipy's stored pattern so that even a field with
  infinities gives sdcflows' bits. The products are `fl32(fl32(w0 · w1) · w2)`.
- **Memory.** sdcflows' fit holds `X` (8 bytes per stored entry: 11 million for a 64 × 64 × 44
  field map, 126 million for ds002603's 128³) at least three times (scikit-learn's copies, the
  adjoint's conjugate copy). larmorx applies `X` from the per-axis weights without storing it.

### scipy's float64 stacks and numpy's NaN of nothing

- **An empty `kron` is float64.** `scipy.sparse.kron(A, B)` returns the zero matrix as a new
  float64 `coo` when `A` or `B` has no stored value, whatever their type. A sampling grid with
  an axis no knot reaches (a target beyond a control grid's support) gives that level a float64
  block, and `sparse_hstack` then a float64 matrix: `B0FieldTransform`'s field is float64 (zeros,
  or, beside a level that does reach the target, its sums in float64: the float32 weights and
  coefficients converted exactly, each row summed from +0). larmorx follows
  (`Collocation::scipy_upcasts`, `matvec_f64_upcast`); the field's file is float32 either way
  (the header's type).
- **The NaN of an empty mask.** numpy's `median` and `mean` of no values are `0.0 / 0`, whose
  NaN on x86 has its sign bit set (0xffc00000 in float32); a NaN among the values is returned as
  it is (`_median_nancheck`); SciPy's `mode` of nothing is `np.nan` (positive). After a resize
  sdcflows' error map is that NaN everywhere; larmorx gives the same bits.
- **scikit-learn's dtype in its message.** "Input y contains infinity or a value too large for
  dtype('<f4')." when nibabel returned the file's raw array (an unscaled little-endian float32
  file: the header's dtype keeps its explicit byte order), "dtype('float32')" otherwise.

### numpy's small rules

- **NEP 50.** A Python float times a float32 array stays float32; a numpy float64 scalar makes it
  float64. `B0FieldTransform.apply` with a Python-float readout time computes the shift, the
  coordinates and the Jacobian in float32; with a numpy float64 readout time, in float64,
  rounded into the float32 coordinates and output. `phdiff2fmap`'s `1 / (2π ΔTE)` is a double
  rounded to float32.
- **Reductions.** `np.median`: the mean of the middle one or two values (NaN if any NaN);
  `np.mean` of float32: numpy's pairwise sum (eight lanes, blocks of up to 128) divided by the
  count **in float64** (the count is an `intp`), then rounded to float32; `scipy.stats.mode`:
  the smallest of the most frequent values, NaN counted after the numbers (SciPy 1.15's
  `np.unique` and 1.18's `unique_counts` agree). Left open, as it changes only signs of zeros in
  the error map: the sign of a zero mode follows numpy's unstable sort.
- **Messages.** sdcflows' f-strings of numpy float32 values go through Python's float repr
  ("max(|coeffs|) = 1319.5245361328125"): CPython's shortest repr with its tie rule, which
  Rust's `{:?}` does not follow on ties (791.0652465820313 against Python's …312).

### nibabel

- **The mask is scaled.** `np.asanyarray(masknii.dataobj) > 1e-4` applies `scl_slope` and
  `scl_inter` (nibabel's `ArrayProxy.__array__`): an int16 mask with slope 1e-5 is below the
  threshold everywhere (`probe_mask_scaling.py`). The comparison is in the scaled array's type
  (float32 data: against `fl32(1e-4)`).
- **The scaled writer.** An output that keeps an integer input header (`BSplineApprox`'s error
  map, `B0FieldTransform.apply`'s corrected series, `CheckB0Units`' field map) is written by
  nibabel in that integer type, scaled. nibabel computes the slope and intercept in
  `np.longdouble`: **x87's 80-bit extended precision on x86-64 Linux, float64 elsewhere**, so
  nibabel itself writes other scale factors on macOS, Windows and Linux aarch64. larmorx's port
  (`lx.io.save_nibabel`) computes those few operations in exact rational arithmetic rounded to
  a 64-bit significand: every platform writes Linux's bits (`check_nibabel_write.py`: 60,000
  extended operations against `np.longdouble`, 782 writes against nibabel 5.4.2).
- An in-memory header has `scl_slope`/`scl_inter` NaN and `vox_offset` 0 until it is written;
  numpy's 16-byte `intent_name` truncates "B-Spline coefficients" to "B-Spline coeffic" (kept).
- **The qform nibabel writes.** `set_qform(affine)` takes the voxel sizes
  `√((a0² + a1²) + a2²)`, `qfac` from `numpy.linalg.det(R) > 0` (numpy's
  `sign · exp(Σ log|u_ii|)` on OpenBLAS's `dgetrf`), the closest rotation `P @ Qs` from
  `numpy.linalg.svd(R)` (LAPACK `dgesdd`, path 5a for 3 × 3) and the quaternion from
  `numpy.linalg.eigh` of Bar-Itzhack's `K / 3` (`dsyevd`), negated when `w < 0`. The last bits,
  and the signs of zeros (a signed permutation: `0 · x` products and `q *= -1`), are LAPACK's
  and its BLAS kernels'. The relay-6 trial found larmorx's own closed-form quaternion
  (`larmorx_core::rotation`, Shepperd) different in more than 60 % of its sampled differences.
  larmorx now replicates numpy's linear algebra for these sizes
  (`larmorx_transform::numpy_linalg`): every routine on the path is unblocked; gfortran
  compiled LAPACK without fused multiply-adds; on the Haswell kernels `drot` (its micro-kernel
  file is compiled with FMA: `x' = fma(c, x, s·y)`, `y' = fma(c, y, -(s·x))`, strided too) and
  `dgemv_t`'s tails are fused, `dnrm2` is x87 (`nrm2.S`: squares summed in extended precision,
  the root rounded to 64 bits, then to a double), the others plain (checked in the
  disassembly of numpy's `libscipy_openblas64_`). `check_numpy_linalg.py`: 4,048 matrices
  (signed permutations, rational rotations, rotations with last-bit noise, scaled and sheared
  affines, random matrices) bit-identical to numpy 2.5.4 and 2.3.5 (svd, eigh, det, nibabel's
  quaternions, signs of zeros included).
- **LAPACK 3.12.0 to 3.12.1 (OpenBLAS 0.3.30 to 0.3.34) changes nibabel's quaternions.** LAPACK
  3.12.1 applies a Householder reflector with `DLARF1F` (its leading 1 implicit: the product
  of the other rows, then the first row added), 3.12.0 with `DLARF` (the 1 stored, inside
  `dgemv`'s sum): `dorm2r`, `dorml2` and `dgebd2` change (`dbdsqr`, `dlascl`, `dlacpy`,
  `ilaenv` change only on paths not reached here). The float64 quaternions of 70 to 90 % of
  rotations differ in the last bits and the float32 (NIfTI-1) fields of 2 to 3 % of oblique
  ones (`check_numpy_linalg.py`'s matrices; none of the signed permutations): nibabel 5.4.2
  itself writes other quaternions with numpy 2.3.5 (and fMRIPrep 25.2.6's conda-forge OpenBLAS
  0.3.30) than with numpy 2.5.4. larmorx follows 3.12.1 (the newest), and selects 3.12.0 for the
  checks in the older environment (`lx.io.nibabel_header.use_lapack`); the originals' step of
  the records shows it as a change of the headers, with this reason.
- **The qform nibabel reads** (`get_qform`) is computed in `np.longdouble`
  (`quaternions.MAX_FLOAT`): x87's extended format on x86-64 Linux. `fillpositive`'s
  `w = √(1 - (b² + c² + d²))` (numpy's longdouble `matmul` loop: from 0, each product and sum
  rounded), `quat2mat`, then `R @ diag(pixdim)` (`LONGDOUBLE_dot`: from 0) rounded once to
  float64. 216 of the 717 images of relay 6's findings had a qform-only affine whose last bits
  or zero signs differ from `lx.load`'s float64 qform; `lx.io.load_nibabel` reads them as
  nibabel does (exact rational arithmetic), and so do every orientation decision
  (`io_orientation` with numpy's SVD: a rotation with two equal axis candidates picks by its
  last bits, which decides whether `ensure_positive_cosines` reorients and so which error
  `apply` raises).
- **The base affine** of a header with neither code (`shape_zoom_affine`) has `-0.0`
  translations along singleton axes (`-0.0 · zoom`).

## The defects (SDC-01 to SDC-10)

Decided by the rules (CLAUDE.md, D13): **A** the input makes sense and the result is wrong:
correct it; **B** the input makes no sense: a clear error; **C** keep and document; **D**
measure first. Every one keeps `upstream_defects="reproduce"`, which gives sdcflows' result
bit for bit. fMRIPrep 25.2.6 reaches none of them on its own paths (below).

### SDC-01: weights beyond the control grid (rule D, measured: A)

`grid_bspline_weights` evaluates `BSpline(..., extrapolate=True)`, whose base interval is
`[0, K − 1]`: a sampling position beyond the first or the last control point (within distance 2
of a kept knot) gets **the end interval's polynomial continued**, not the B-spline's value. One
dimension, knot 0: at −0.5 the weight is 0.354 instead of 0.479, at −1.0 −0.833 instead of
0.167, at −1.5 −3.27 instead of 0.0208, at −1.75 −5.08 instead of 0.0026 (`measure_sdc01_02.py`,
part 0). The correct value is the cubic B-spline's: the same recursion on the knots continued
beyond the grid (inside `[0, K − 1]` larmorx keeps the original's intervals, so the same bits).

Measured (`measure_sdc01_02.py`, parts 1 and 1b; log `oracles/logs/fmap-a-5-sdc01b.log`):
- **fMRIPrep never reaches it.** Its `ReconstructFieldmap` evaluates the weights directly on the
  target only when the target, projected through the transforms, is aligned with the
  coefficients; in all 28 reconstructions of the oracle runs (ds002422, ds002603, ds005040,
  ds005454: BOLD references, T1w and MNI targets) it is not, and the field is evaluated on the
  field map's own grid, which lies inside the knots (0 positions beyond), then resampled.
- **sdcflows' own unwarping reaches it** (`ApplyCoeffsField`, `B0FieldTransform.fit`, used by
  sdcflows' unwarping workflow): on ds005040's BOLD reference (aligned with TOPUP's coefficients,
  evaluated directly) 9,049 of 163,840 voxels (5.5 %) lie beyond the grid; they are all outside
  the BOLD brain mask, where the field differs by up to 70.4 Hz; on ds002603's oblique BOLD
  references 19 % of the intermediate grid's positions lie beyond (the corners of the grid
  around the rotated target) and the field there differs by up to 380 Hz, but no BOLD voxel
  falls there (the resampled field changes by at most 0.0005 Hz).
- So the brain was not affected in these runs, but the input makes sense (a target larger than
  the field map's extent) and the result is not the model's field: **rule A**. Corrected by
  default; a larger field of view or a smaller field map brings the wrong values into the brain.

### SDC-02: a rotated sampling grid (rule D, measured: A)

On a sampling grid rotated relative to the control grid, `grid_bspline_weights` warns "Image's
and B-Spline's grids are not aligned." and still multiplies per-axis weights, each computed from
one index of the voxel (the others set to 0), which is not the field at the voxel. The correct
value is the B-spline field at the voxel's own position: per voxel the product of its three
axes' weights at its three coordinates (up to 64 control points).

Measured on ds005454's field (max 427 Hz) evaluated on its own grid rotated about its centre
(`measure_sdc01_02.py`, part 2), inside the control grid, against the exact evaluation:

| rotation | separable weights (sdcflows): RMS, max (Hz) | sdcflows' `approx=True`: RMS, max (Hz) |
|---|---|---|
| 0.05° about z | 0.234, 4.02 | 0.001, 0.06 |
| 0.5° about z | 2.34, 40.3 | 0.001, 0.07 |
| 2° about z | 9.13, 158 | 0.001, 0.06 |
| 5° about z | 20.1, 345 | 0.002, 0.13 |
| 15° about z | 33.7, 438 | 0.002, 0.09 |
| 5° about x | 24.5, 461 | 0.002, 0.13 |
| 45° about z | 38.7, 1,132 | 0.003, 0.12 |
| 90° about z | 25.4, 428 | 0.002, 0.11 |

The tolerance of sdcflows' alignment test (`atol=1e-3` on the cross products of the affines'
columns, which carry the voxel sizes) is about 0.0014° for 16 mm control spacing and 2.5 mm
voxels, so any visible rotation warns. sdcflows' own approximation (`approx=True`: the field on
a grid aligned with the coefficients, 4 times finer, then cubic resampling) stays within 0.13 Hz
of the exact evaluation, which also confirms the exact evaluation; the separable product is
wrong by up to the field's size.

**sdcflows' own unwarping meets it with real registrations** (`measure_sdc01_02.py`, part 2b;
log `oracles/logs/fmap-a-5-sdc02b.log`): `B0FieldTransform.fit(target, xfm_data2fmap)` takes the
direct path when the *target* is aligned with the coefficients, and evaluates the weights on the
*projected* grid, which any rotation of the registration turns. With fMRIPrep's own
registrations of the BOLD references to their field maps (rotations of 0.89 to 1.50°), the
separable field differs from the field at every voxel's position, inside the BOLD brain mask,
by 2.3 Hz RMS (30 Hz at most) and 1.5 and 1.7 Hz RMS (25 and 27 Hz) on ds002422's three runs,
4.6 Hz RMS (49 Hz) on ds005040 and 3.5 Hz RMS (31 Hz) on ds005454 (ds002603's oblique references
take the approximation's path: SDC-10). The input makes sense (a target rotated relative to the
field map, `B0FieldTransform.fit` without a transform, with `approx=False`, or a rotating
registration of an aligned target): **rule A**, the exact
weights by default (`ExactLevel`: the location by nibabel's `apply_affine`, every axis' weights
rounded to the matrix type, the products `(w0 · w1) · w2`, ascending columns). fMRIPrep never
reaches it (its reconstruction refuses non-aligned grids: "Reference passed is not aligned with
spline grids"); sdcflows' `ApplyCoeffsField` and unwarping workflow reach it whenever the
registration rotates.

### SDC-03: `CheckRegister`'s signed translation test (rule A, not switched yet)

Two magnitude images whose affines differ are registered with FreeSurfer's
`mri_robust_register`, and the registration is refused when a rotation exceeds `rot_thresh` or
a translation component exceeds `trans_thresh`: the test is `vec > trans_thresh` on the signed
components, so a large negative shift is never "too big". Its critical message also prints
"{mag_files}" literally (the f-prefix is missing). Rule A for both; the branch needs FreeSurfer
and is not ported yet (`check_register` raises `NotImplementedError` there; no oracle run
reaches it): the switch comes with the FreeSurfer part.

### SDC-04: a constant phase map (rule B)

`au2rads` rescales `(d − min) × 2π / (max − min)`: a constant map gives 0 / 0, NaN everywhere,
with only numpy's `RuntimeWarning`. The input makes no sense as a phase map: **rule B**, a
`ValueError` by default.

### SDC-05: an integer or float32 readout time (rule A)

`B0FieldTransform.apply` wraps `ro_time` in a list only when it is a Python `float`; an `int`
(or a numpy float32) reaches `zip` and fails with a `TypeError`. A readout time of 1 s or a
float32 is a sensible input: **rule A**, accepted as the float (with the notice).

### SDC-06: a 4D header of one volume (rule A)

`bspline_grid` divides the affine's columns by `img.header.get_zooms()`, every zoom: a field
map stored as `(X, Y, Z, 1)`, as fMRIPrep writes its `desc-preproc_fieldmap` (9 of 9 in the
oracle runs, `probe_header_zooms.py`), fails with numpy's broadcasting `ValueError`, and
`BSplineApprox` with it. **Rule A**: the first three zooms (several volumes: a clear error,
since a field map is one volume). The other half of the question, header zooms against the
affine's voxel sizes, was measured equal on all 34 field-map images of the oracle runs (largest
relative difference 7e-8): kept (rule C).

### SDC-07: `TOPUPCoeffReorient`'s negation (rule D, measured: A)

`_fix_topup_fieldcoeff` negates TOPUP's coefficients when the phase-encoding direction,
reoriented to RAS, is written with BIDS' letter `i`; never for FSL's `x`, which fMRIPrep passes.
TOPUP's coefficients are a field in Hz (a scalar that a left-right flip does not negate), so the
negation inverts the field: on synthetic left-right and anterior-posterior blips of a phantom
with a known field, TOPUP's field (through `lx.fsl.topup`, whose coefficients equal FSL's,
r = 1.0000) correlates r = −0.999 with the truth after the negation (RMS 55 Hz; the unwarping
worsens the blips' agreement by 0.18) and r = +0.999 without it (0.9 Hz), in LAS and RAS
(`measure_sdc_g.py`, `probe_sdc07_topup.py`). **Rule A**: never negated.

### SDC-08: a positive-axes series from a file (rule A)

`B0FieldTransform.apply` (and `ApplyCoeffsField`) on a series whose axes already run to R, A and
S, read from a file, fails with an `AttributeError`: no reorientation happens, so numpy's
`rollaxis` receives nibabel's `ArrayProxy`, which has no `transpose` (also with fMRIPrep's
sdcflows 2.15.1 and numpy 2.2.6). **Rule A**: unwarped.

### SDC-09: `SubtractPhases` without `EchoTime` (rule B)

`subtract_phases` pops `EchoTime` from each map's metadata and compares them: a missing one is
`None`, and the comparison fails with Python's `TypeError`. The input is incomplete: **rule B**,
a `ValueError` that names the missing metadata.

### SDC-10: the approximation ignores the transform (rule A)

`B0FieldTransform.fit(target, xfm_data2fmap, approx=True)` (the default of `ApplyCoeffsField`
and of sdcflows' unwarping workflow) on a target not aligned with the coefficients builds its
intermediate grid with `deoblique_and_zooms(coeffs[-1], target_reference)`, the target **as
given**, not the projected reference, and resamples that grid at the target's own coordinates
(`Affine(reference=target).apply(mapped)`): the field is taken where the target lies before the
registration, so `xfm_data2fmap` changes nothing but the choice of the path. The direct path
(an aligned target) uses the projected reference, as intended. Measured
(`measure_sdc01_02.py`, part 3) against the exact field where the transform puts the target:
on ds002603's two oblique BOLD references (registrations to the field map of 2.5 and 3.7 mm),
7.7 and 8.5 Hz RMS, up to 87 Hz, inside the BOLD brain mask; on ds005454's BOLD reference
rotated 5° with a 4 mm transform, 11.4 Hz RMS, up to 232 Hz. **Rule A**: the grid built around,
and sampled at, the target where the transform puts it (the notice whenever the transform is
not the identity). fMRIPrep does not use this path.

### Not defects

- The three identical fit attempts (above); the "and" of "Approximating B-Splines grids (and
  …)" with a single grid (cosmetic, the text kept).
- `CheckB0Units` with `rad/s` divides values that are already scaled (`np.asanyarray(dataobj)`
  applies the scaling) and keeps the input's stored type, which nibabel re-quantises (an int16
  map of ±500 Hz in steps of about 0.015 Hz): nibabel's documented writer, a negligible loss,
  kept (rule C).
- `_b0_resampler`'s head-motion branch fails in sdcflows (`unwarp.xfm` on a slots class): a
  private function no workflow calls; not ported.
