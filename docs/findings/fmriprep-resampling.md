# fMRIPrep's one-shot resampler and nitransforms

How fMRIPrep's `ResampleSeries` (`fmriprep/interfaces/resampling.py`, fMRIPrep at
`21a490fb`) and the nitransforms 25.1.0 code under it compute, and what larmorx
(`lx.transforms.resample_series`, `larmorx_transform::{nitransforms, resample_series}`) does
about it. The interpolation itself is SciPy's: [scipy-ndimage.md](scipy-ndimage.md).

## The arithmetic that decides bit-identity

**numpy's 4×4 products are BLAS calls with fused multiply-adds.** nitransforms maps points
with `affine.dot(points)` (`base.py:378-384`, `linear.py:278-282`), and nibabel's
`apply_affine` with `pts @ rzs.T + trans`. numpy hands both to OpenBLAS `dgemm`, whose Haswell
kernel accumulates `fma(a3, p3, fma(a2, p2, fma(a1, p1, a0·p0)))`. Evaluated with separate
roundings, about 1,900 of 6,000 values differed; with the fused chain, 0, for both the
4×N point products and 4×4 matrix products. *Verified* (OpenBLAS 0.3.30, numpy 2.3.5, x86-64
with FMA). larmorx uses `f64::mul_add` in the same order. A numpy whose BLAS does not fuse
(an old CPU, another BLAS) gives fMRIPrep results that differ from these in the last bits.

**Points go through float32 before every affine.** `_as_homogeneous` converts its input to
float32 (`base.py:351-375`), so `Affine.map` and the displacement field's index lookup round
every coordinate to float32 at each step of a chain; outputs are float64. fMRIPrep also
rounds the target grid's coordinates to float32 (`ndcoords.astype('f4')`,
`resampling.py:572`). *Read*; *validated*.

**ITK text transforms are read as float32.** nitransforms parses `Parameters` and
`FixedParameters` with `np.genfromtxt(..., dtype='f4')` (`io/itk.py:200-203`), and stores the
centre of rotation as float32 even when read from `.h5` (`template_dtype`, `itk.py:23-30`).
fMRIPrep's head-motion and coregistration matrices are therefore float32-exact before
`to_ras` (`LPS · C⁺ · M · C⁻ · LPS`, in float64 with numpy). ANTs reads the same files in
double precision, so ANTs and fMRIPrep apply slightly different transforms (up to about 1e-7
relative). *Read*; larmorx reproduces nitransforms (it parses with the same numpy call).

**The 4×4 matrices are computed with numpy in both.** `np.linalg.inv` (LAPACK `dgesv` in
OpenBLAS, which multiplies by reciprocals of the pivots) and the matrix products are not
re-implemented: larmorx's Python layer evaluates the same numpy expressions as nitransforms
and fMRIPrep (`ras2vox @ M @ vox2ras` for head motion, `np.linalg.inv(source.affine)`, the
ITK-to-RAS conversion) and passes the matrices to Rust. So they carry the same bits as
fMRIPrep's on the same machine.

**Those bits depend on the CPU's BLAS kernel, so fMRIPrep's do too (2026-10-11).** OpenBLAS
picks its kernels by CPU at run time. Forcing other kernels on the development machine
(`OPENBLAS_CORETYPE`, numpy 2.3.5, OpenBLAS 0.3.30): the products `inv(A) @ M @ A` of 10,000
random affines differ in 99.9 % of cases between the kernels with fused multiply-adds
(Haswell, Zen) and those without (Nehalem, Sandy Bridge, Prescott); `np.linalg.inv` differs in
23 % of cases between the Haswell and Sandy Bridge kernels (up to 6e-13 relative) but not
between Haswell and Nehalem. In the resampler's golden case the float64 output then differs by
up to 81 ulp at the sampled voxels, and the float32 output, which fMRIPrep writes, did not
change. numpy's macOS wheels use Accelerate instead of OpenBLAS. *Verified.* So fMRIPrep's
results differ in the last bits between machines, and larmorx's `lx.transforms` (which
evaluates the same numpy expressions) would differ between platforms; the golden tests keep it
visible, and [golden.md](../validation/golden.md) lists the options (a fixed-order Rust
implementation that reproduces the Haswell kernel is the recommended one).

**The voxel-shift map is float32.** `vsm = fmap_hz * pe_info[1]` multiplies a float32 array by
a Python float, which NumPy 2 (NEP 50) casts to float32 first. It is added to the float64
coordinates. The Jacobian `1 + np.gradient(vsm, axis=pe)` is float32 too
(`(f[i+1] - f[i-1]) / 2`, first-order differences at the edges), and multiplies the float32
output in float32. *Read*; *validated*.

## Behaviour worth knowing

**The default mode is `grid-constant`,** the first value of `ResampleSeries`' `mode` enum
(`resampling.py:56-68`); `resample_image`'s own default is `constant`. *Read*. larmorx's
`resample_series` follows the interface.

**The Jacobian is taken along a target axis.** `np.gradient(vsm, axis=pe_info[0])` uses the
phase-encoding axis index of the *source* voxel grid on the *target* array
(`resampling.py:316`). It is the right derivative only when the target grid's axes match the
source's (the native boldref); in T1w or standard space it differentiates along whatever
target axis has that index. *Read*. larmorx reproduces it.

**Distortion correction reorients the source first.** With `pe_dir` and `ro_time`,
`ensure_positive_cosines` (sdcflows) flips every axis that runs against its world axis and the
readout's sign is negated when the PE axis was flipped XOR the direction ends in `-`
(`resampling.py:101-109`). Without them the source is not reoriented. *Read*; *validated* on
LAS and axis-permuted (ALS-like) series in all six directions.

**A head-motion file with one transform is not head motion.** `nt.linear.load` turns a
one-transform list into an `Affine`, which `resample_image` then applies in world space as part
of the chain (float32-rounded input), not as a voxel-to-voxel matrix (`linear.py:451-452`).
*Read*; larmorx's loader does the same.

**Displacement fields: lookup or cubic interpolation, and no displacement outside.**
`DenseFieldTransform.map` (`nonlinear.py:185-209`) looks displacements up only when *every*
point lies within 1e-3 voxel of a field node; otherwise it calls `map_coordinates` three times
(order 3, mode `constant`, cval NaN, the field prefiltered each time) and replaces NaN results
with the input point, so points beyond the outermost nodes are not displaced at all: a
discontinuity at the edge of the field. For fMRIPrep's MNI152NLin2009cAsym res-2 target and a
res-1 warp, every point lies half a voxel off the nodes (origins -96.5 vs -96), so the field is
interpolated, and 31,411 of 1,082,035 target points (2.9 %) fall outside it and stay
undisplaced. *Verified* (ds000005 sub-01, fMRIPrep 21.0.1's warp).

**Composite `.h5` files are read in reverse.** nitransforms inserts each transform of
`TransformGroup` at the front of the chain (`manip.py:236-240`), so the last one ITK applies
first comes first; it iterates the group in h5py's name order. *Read*.

**Threads.** `resample_series` runs one `resample_vol` per volume through asyncio with a
semaphore of `nthreads` (`resampling.py:396-425`); SciPy releases the GIL, so volumes do run
in parallel. Each task allocates full-size coordinate and spline-coefficient arrays. *Read*;
measured in [the benchmark](../benchmarks/resample-series.md).

## What larmorx changes

Nothing in the values: every output is bit-identical to fMRIPrep's on the validation set
(`docs/validation/resample-series.md`). The work is reorganised: the target grid is mapped
once per run, the voxel-shift map and Jacobian once per readout setting, each volume is
prefiltered once and interpolated in one pass with the head motion and voxel shift applied
per voxel in registers, and only `n_threads` volumes are in flight at a time.
