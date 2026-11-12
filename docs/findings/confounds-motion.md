# fMRIPrep's confounds: the motion parameters and the RMS deviation

What porting fMRIPrep's `FSLMotionParams` (both fMRIPrep releases' forms) and `FSLRMSDeviation` showed,
with nitransforms' ITK reading and `FSLLinearTransform.from_ras`, SciPy's `Rotation.from_matrix` and
`as_euler`, SciPy's `center_of_mass`, and the defects found (PIPE-71 to PIPE-73) and the C library's
`atan2`/`hypot` (decision D4). fMRIPrep and nitransforms are Apache-2.0, SciPy BSD-3-Clause: not
restricted. The port: `crates/larmorx-confounds/src/motion.rs`, `python/larmorx/confounds/motion.py`,
niworkflows' copies in `python/larmorx/niworkflows/confounds.py`
([docs/api/confounds.md](../api/confounds.md)). The scripts: `check_motion.py`, `probe_be_sum.py`,
`probe_euler.py`, `probe_libm.py` ([validation/investigations/confounds](../../validation/investigations/confounds/README.md)).

## fMRIPrep 25.2.6 → 25.2.7 changed `FSLMotionParams` (a step per case)

(read; verified: both forms bit for bit)

- **25.2.6** (and niworkflows 1.15.2's and 1.15.3's own copy): the centre of gravity is `diag(zooms) @
  center_of_mass(np.asanyarray(dataobj))` (the stored type: float32 sums for a float32 reference; a
  byte-swapped array summed in chunks of 8,192 in every numpy release: [confounds-numpy.md](confounds-numpy.md)),
  `rot = as_euler(from_matrix(M.T))`, `trans = b - M.T @ cog + cog` (the strided `M.T`: OpenBLAS's
  `dgemv_n` with `lda` 4, the products added one at a time from 0, no FMA).
- **25.2.7** (and niworkflows 1.15.4's copy, which no workflow uses): `get_fdata()` reversed on axis 0
  when numpy's `det(affine) > 0`, `center_of_mass(data - data.min())` (`data[::-1] - data.min()` of
  nibabel's Fortran-ordered array is Fortran-ordered with positive strides), `trans = M @ cog + b - cog`
  (`M` C-ordered with `lda` 4: `dgemv_t`), `rot = -as_euler(from_matrix(M))`.
- So the step changes every `trans_*`/`rot_*` cell and what follows from them (FD, the expansion, the
  spikes) on every run: [confounds-workflow.md](confounds-workflow.md) counts it on the 36 files.
- The common part: `nb.load(boldref)`; `nt.linear.load(xfm)` (ITK text: the banner, one `#Transform`
  block each, the `Transform:` line never read, `genfromtxt(dtype="f4")` for the 12 parameters and 3 fixed
  ones; `to_ras() = LPS · (c_pos · (P · (c_neg · LPS)))`, four `dgemm`s; `LinearTransformsMapping` makes
  an `Affine` per matrix: `allclose(last row, (0, 0, 0, 1))` with its `ValueError`, `inv` with
  `LinAlgError: Singular matrix`); `np.diag(zooms) @ com` (the float32 zooms cast; `dgemv_t`);
  `FSLLinearTransformArray.from_ras(ras, reference=boldref, moving=boldref)`: `_fsl_aff_adapt`
  (nibabel's `voxel_sizes`, the first axis mirrored when `det > 0`, `swp[0, 3] = (shape0 - 1) · zooms[0]`),
  `pre = A @ inv(swp @ spc)`, `post = swp @ spc @ inv(A)`, the parameters `inv((post @ ras @ pre).T).T`.
- **`FSLRMSDeviation`** (the same in every release): `center = 0.5 · (shape[:3] - 1) · zooms[:3]`,
  numpy's stacked `inv(F[:-1])`, `F[1:] @ inv - eye(4)`, `t = D[:3, 3] + M @ center`, `diag(t @ t.T)`
  (numpy's `dsyrk`: the diagonal is the Haswell kernel's fused chain `fma(t2, t2, fma(t1, t1, t0 · t0)) +
  0`; one row: `0 + ddot`, plain), `trace(M.T @ M)` (a `dsyrk` per matrix, the diagonal added in order)
  `· 6400.0 / 5`, `sqrt`, `n/a` first (Rmax 80 mm).

## SciPy's `Rotation` (read, verified; 1.18.1's `_rotation_cy.pyx` = 1.15.3's `_rotation.pyx` arithmetic)

- `np.array(matrix, dtype=float)` (K order: 25.2.6's `M.T` gives F-ordered slices); every determinant
  first (the first `<= 0`: `ValueError ... in rotation matrix {ind}: {matrix[ind]}.`; a NaN determinant
  passes); the Gramian `m @ m.T` (numpy's `dsyrk` case, both layouts) against `isclose(eye, atol=1e-12)`:
  rotations rounded to float32 (every real head-motion matrix but the identity) go through numpy's 3 ×
  3 `svd` (JOBZ='A') and `U @ Vt`; Shepperd's quaternion (`_argmax4` with `>`, `(1 - trace) + 2 · m_ii`,
  `_normalize4`: `((x² + y²) + z²) + w²`, a zero norm left alone).
- `as_euler`: Bernardes and Viollet's `_get_angles` with the C library's `atan2` and `hypot`. **The
  `sign` passed to `_get_angles` as `bint` keeps its value -1** (Cython passes the C int unchanged:
  measured; a model with `sign = 1` misses every odd-permutation sequence, `XYZ` and `xzy` included).
- numpy 2.3.5's and 2.2.6's SVD of a matrix holding an infinity hangs (NP-10,
  [confounds-blas.md](confounds-blas.md)): such a rotation hangs SciPy's `from_matrix` there; larmorx
  raises numpy 2.5.4's `LinAlgError: SVD did not converge` for every release.

## The C library's `atan2` and `hypot` (decision D4)

- glibc's double `atan2` and `hypot` (SciPy calls them through libc) are not correctly rounded: 96
  (`atan2`) and 564 (`hypot`) of 100,000 uniform pairs, 12 and 10 of 100,000 quaternion-like pairs
  (`probe_libm.py`); `hypot`'s rates by family in [confounds-blas.md](confounds-blas.md). Both are glibc's
  own LGPL code, never translated.
- larmorx's `atan2` and `hypot` are correctly rounded in both modes (D4: CORE-MATH's), an expected
  divergence of `reproduce`, counted. **On the 36 real motion tables: 15 of the 22,293 `rot_*` cells (12
  files) change by glibc's `atan2`, none by `hypot`**; the synthetic checks: 266 to 278 of 119,064 angles
  by `atan2`, 718 to 744 by `hypot`. So no glibc-behaviour `hypot` is needed (the real files show no
  change); a glibc-behaviour `atan2` (written as a clean room from a behaviour spec, as `sincos_glibc`)
  would make `reproduce` exact on those 15 cells: a follow-up; meanwhile a D13 gap (GLIBC-07's row).

## Defects (rules A to D; `larmorx_core::defects`)

- **PIPE-71** (rule A; nitransforms `load`): a file of one transform is loaded as one `Affine`
  (`len(xfm) == 1: xfm = xfm[0]`), whose 4 × 4 matrix `FSLLinearTransformArray.from_ras` iterates as four
  transforms (its rows): `post @ row @ pre` is a vector and `np.swapaxes(vector, 0, 1)` raises
  `AxisError: axis2: axis 1 is out of bounds for array of dimension 1` (after `_fsl_aff_adapt`'s
  `Singular matrix`, which comes first). A one-volume series stops both interfaces. Default: a series of
  one (the motion parameters one row, the first row of the original on the transform given twice;
  `rmsd` one `n/a`); `reproduce`: numpy's `AxisError`; the notice in both modes.
- **PIPE-72** (rule B; fMRIPrep `FSLMotionParams`): a reference whose centre of gravity is undefined
  (25.2.7: a constant image, every weight `value - min` zero; 25.2.6: a sum of zero, e.g. +1/-1 voxels; a
  NaN or infinite voxel in either) gives NaN or infinite translations in every row with numpy's
  `RuntimeWarning` only, which `FramewiseDisplacement` turns into `n/a` everywhere (no FD spike).
  Default: `ValueError` naming PIPE-72; `reproduce`: fMRIPrep's values, the notice. (25.2.6 with a
  constant non-zero image has a defined centre: no PIPE-72.)
- **PIPE-73** (rule A; nitransforms `ITKLinearTransform`): every ITK text transform is read with
  `genfromtxt(dtype="f4")`, the `*_double_*` ones too, so their parameters are rounded to float32.
  Default: a type name with `_double_` read in float64 (parameters and fixed parameters); `reproduce`:
  float32; the notice in both modes counts the values the rounding changes. fMRIPrep's head-motion files
  are `AffineTransform_float_3_3`: no real node changes.
- **Kept** (rule C, documented): 25.2.6's centre of mass of the stored values (negative weights allowed;
  25.2.7 is the step); the `Transform:` line is not read (any type with 12 parameters is taken as a
  matrix and a translation; other counts fail with numpy's reshape `ValueError`).
- **Refused** (`TransformFileError`; a follow-up): nitransforms' `.mat` (one transform: an `Affine`, which
  then fails in `from_ras` anyway) and `.h5` readers, and the LTA, AFNI and FSL text formats `linear.load`
  tries after ITK's (AFNI's and FSL's fail there without a reference: `TransformIOError: Cannot build FSL
  linear transform without a reference`; a text file without the banner: `ValueError: could not
  broadcast input array from shape (0,4,4) into shape (4,4)`). Port them when a caller needs them (LTA
  and ITK `.h5` composites are the ones nitransforms would read).

## Proof (validated)

`check_motion.py`'s full tier, CHECK PASS in numpy 2.5.4 / SciPy 1.18.1, numpy 2.3.5 / SciPy 1.15.3 and
numpy 2.2.6 / SciPy 1.15.2 inside fMRIPrep's image: `rotation` 5,138 `from_matrix` quaternions bit for
bit (rotations, float32-rounded rotations, scaled, sheared, nearly singular, Shepperd's four branches,
gimbal lock, signed zeros; C and F stacks of 1 and many) + 22 to 24 error cases (reflection, zero, rank
2, NaN, inf: not run in numpy 2.3.5/2.2.6 for an infinite matrix, which hangs there), 119,064 `as_euler`
angles (24 sequences: the model with glibc's `atan2`/`hypot` = SciPy, larmorx = the model with CORE-MATH's
`atan2` and an exact `hypot`); `itk` 3 sizes × 19 files against `nitransforms.linear.load`; `fsl` 204
affines × 30 transforms of `from_ras` + a singular affine; `iface` 21 cases × both releases byte for byte
but D4's named cells (niworkflows' copies = fMRIPrep's = larmorx's), 21 `FSLRMSDeviation` byte for byte,
PIPE-71's, -72's and -73's models; **`real`, the 36 confounds workflows**: larmorx with
`fmriprep_version="25.2.6"`, `numpy_version="2.2.6"` and `reproduce` = the node files: `rmsd` 36 of 36
byte for byte; `motion_params` 36 of 36 with every `trans_*` cell identical and 15 of 22,293 `rot_*`
cells (12 files) changed by glibc's `atan2`; in fMRIPrep's image fMRIPrep 25.2.6 run again = its node
files (36 of 36); both releases run again in every environment = larmorx with that numpy (D4's cells
apart); the default = `reproduce` on all 36 (no defect reached). The suite `confounds-fmriprep` runs it.
Speed: [docs/benchmarks/confounds.md](../benchmarks/confounds.md).
