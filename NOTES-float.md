# Notes: antsApplyTransforms `--float 1` (paused 2026-10-11)

Working notes for the `--float 1` task (single precision as ANTs computes it, and the speed of
the template-prior step). Paused at the coordinator's request before any code was written.
Remove this file (archive it) when the task is done.

## State

- Branch `worktree-agent-a689dd2fa349688de`, fast-forwarded to `main` at `8a5bca3` (the GCC 14.3
  oracle merge). No code changes yet.
- The speed target is stage **A06** of `docs/validation/fmriprep-stages.md` (brain extraction,
  template priors to the T1w, Gaussian through the inverse SyN warp, `.h5`, **double**
  precision): larmorx 10.2 + 10.3 s against ANTs' re-run 8.23 + 7.77 s, one thread.
- The 14 `--float 1` nodes: A11 `t1w_std`, `anat2std_t1w`, `anat2std_tpms0-2` (5); F08, F09
  and F12 `resample_ref` / `t1w_boldref` (3 runs each, 9).

## What ANTs 2.6.5 / ITK 5.4.5 do with `--float 1` (read, not yet verified)

`antsApplyTransforms<float, 3, OT>` (`Examples/antsApplyTransforms.cxx:154`): `RealType =
PixelType = float`; images `Image<float,3>` (geometry stays double); transforms
`CompositeTransform<float,3>`; `ResampleImageFilter<Image<float>, Image<float>, float>` (both
the interpolator and the transform precision are float); interpolators from
`make_interpolator_snip.tmpl` with `TCoordRep = float`.

- **Default value**: `Convert<float>` = `istringstream >> float` (strtof, one rounding; the CLI
  now uses `atof` then `as f32`, a double rounding).
- **Transform reading** (`itkantsReadWriteTransform.h`, `TransformFileReaderTemplate<float>`):
  - text (`itkTxtTransformIO.cxx`): `Parameters` **and `FixedParameters`** are parsed into an
    `OptimizerParameters<float>` (`>> float`, strtof), the fixed ones then widened to double;
    so read text numbers with `parse::<f32>()` in float mode, not `strtod` then `as f32`.
  - `.mat`, `.h5`: still to read (`itkMatlabTransformIO.cxx`, `itkHDF5TransformIO.cxx`);
    expectation: parameters cast to float, fixed parameters double.
  - NIfTI warps: `Image<Vector<float,3>,3>` (a float64 field is cast to float).
- **Nonlinear resampler path** (`itkResampleImageFilter.hxx:370`): output point in double
  (`TransformIndexToPhysicalPoint`, summed from the origin), **rounded to `Point<float>`**
  (implicit `FixedArray` converting constructor, `static_cast`) before `TransformPoint`; each
  transform takes and returns `Point<float>`; `TransformPhysicalPointToContinuousIndex<float>`
  computes `(p − origin)` and `M⁻¹·v` in double and casts the index to float
  (`itkImageBase.h:510`); `IsInsideBuffer` compares floats against `float(start − 0.5)` and
  `float(end + 0.5)`.
- **Linear path** (`:432`): the two line ends as above (float indices); `vector = end − start`
  in float; each voxel `inputIndex[i] += alpha·vector[i]`, i.e. `float(start + alpha·vector)`
  computed in double (as `larmorx_ants::resample::Coordinates::Float` already does).
- **Output pixel**: the interpolator returns double (`NumericTraits<float>::RealType`);
  `CastComponentWithBoundsChecking` clamps to `[-FLT_MAX, FLT_MAX]` before `static_cast<float>`
  (so ±inf and overflowing values become ±FLT_MAX; NaN passes). Then `CastImageFilter` to `-u`.
- **MatrixOffsetTransformBase<float>** (`itkMatrixOffsetTransformBase.hxx`):
  `TransformPoint = M·p + offset`, each row summed from 0 in float (`itkMatrix.hxx:48`), then
  `+ offset` in float. `SetFixedParameters` casts the centre to float; `ComputeOffset` (`:627`)
  in float, terms subtracted one at a time. `GetInverse` (`:436`): `m_Matrix.GetInverse()` =
  `vnl_determinant` (3×3 formula of `vnl_determinant.hxx`, in float) `== 0` check, then
  `vnl_svd<float>` pinverse `V·W⁻¹·Uᵀ` with `Winverse = float(1.0)/w` and the products of
  `vnl_matrix` (sum from 0, float). `larmorx_core::vnl_svd_f32` (N4) has the SVD and
  `matmul_f32`; a `vnl_inverse_f32` must be added. Offset `−(M⁻¹·offset)` in float.
  Inverses are plain `MatrixOffsetTransformBase` (`InvertTransform(*this)` with the base's
  static type), so Euler's `ComputeMatrixParameters` (which recomputes the matrix from angles)
  is **not** used; this matches today's double code.
- **Euler3DTransform<float>**: angles float, `std::cos/sin(float)` = **cosf/sinf** (need
  correctly rounded ports from CORE-MATH, `tags/core-math`, as `expf`/`logf` were), the
  rotation matrices multiplied by `vnl_matrix_fixed_mat_mat_mult` (accumulator starts at the
  first product), in float; ZYX flag from the fourth fixed parameter.
- **VersorRigid3DTransform**: `SetParameters` computes `norm` in **double**
  (`std::sqrt` of float products promoted? check: `parameters[i]` is float, products float,
  then `std::sqrt(float)` → float sqrt, assigned to `const double`); `axis / (norm + eps·norm)`
  when `norm ≥ 1 − 1e-10`; `Versor<float>::Set(axis)` and `GetMatrix()` still to read
  (`itkVersor.hxx`), plus `Similarity3DTransform`, `TranslationTransform`, `Rigid3DTransform`.
- **DisplacementFieldTransform<float>** (`itkDisplacementFieldTransform.hxx:68`): point inside
  test through the interpolator's `IsInsideBuffer(Point)` (index in double, cast to float),
  then `VectorLinearInterpolateImageFunction<…, float>`: `Math::Floor` (vnl SSE floor),
  distances float, `overlap` float (`overlap *= 1.0 − d` is a double product stored as float),
  `output[k] += float(overlap·value)` summed in double, `totalOverlap` double, stop at 1.0;
  then `outputPoint[i] += displacement[i]` in double, stored as float.
  (Also noted: ITK's double `Math::Floor` is the SSE trick too, which gives 0 for tiny
  negative indices; `field.rs` uses `f64::floor` — a latent last-bit difference to check.)
- **Interpolators**, all with `TCoordRep = float`: still to read in detail — linear 3D
  `EvaluateOptimized` (float distances; expected equal to double given float indices),
  nearest (`RoundHalfIntegerUp` of a float), B-spline (`BSplineInterpolateImageFunction<I,
  float>`: coefficient type stays **double** by default; check the brief's "float prefilter"),
  Gaussian and MultiLabel (`GaussianInterpolateImageFunction<I, float>`), GenericLabel
  (`LabelImageGenericInterpolateImageFunction<I, Linear, float>`), windowed sincs: the window
  functions are `XWindowFunction<3, float, float>` (**float** `cosf`/`sinf` inside the window),
  check the sinc itself.

## Plan (decided, not started)

1. A precision type parameter: a small `Real` trait (f32/f64: arithmetic, `from_f64`,
   correctly rounded `sin`/`cos`, `sqrt`) in `larmorx-core`, additive.
2. `larmorx-transform`: keep `Transform`/`LinearTransform`/`TransformChain` unchanged for
   existing users (do not touch `resample_series.rs`, `nitransforms.rs`); add a private record
   of the `(transform, invert)` items to `TransformChain` and a
   `TransformChain::with_precision::<R>()` that builds a point map in `R` from the stored
   parameters (a generic `MatrixOffset<R>` built per ITK class, generic inverse via
   `vnl_inverse`/`vnl_inverse_f32`, generic field lookup). Refactor the f64 path onto the
   generic code only with a bit-identity test against the old code.
3. `larmorx-io`: a float mode for text parsing (`parse::<f32>()` widened to f64), `.mat`
   and `.h5` as ITK reads them in float. `TransformLoader` gets a precision-aware method with
   a default.
4. `larmorx-interp`: the coordinate type as a parameter where ITK's arithmetic changes
   (window functions, nearest rounding, B-spline support index already rounds via float, the
   Gaussian erf arguments, GenericLabel's linear part, the vector field interpolator).
5. `larmorx-ants`: `ApplyTransformsOptions` gains `precision` (default `F64`, today's
   behaviour); the float path rounds points and indices as above and clamps the output; CLI
   `--float 1`; Python `precision="float32"|"float64"`, `.pyi`, API page; leave room for an
   `upstream_defects` option (D13 work is another task).
6. Validation: `--float 1` cases in `validation/.../parity/ants_apply_transforms.py`
   (every interpolator, transform kind, `.h5`, inverses, 4D, oblique), record leading with the
   unmodified oracle and then `run_ants(..., correctly_rounded=True)`; re-run the 14 stage
   nodes; golden cases with `scripts/record_golden.py`.
7. Speed of A06: profile Gaussian through the `.h5` warp (double); candidates: per-row erf
   weight reuse, avoiding `Vec` allocations in `erf_weights`/`gaussian_region`, the field
   lookup.
