# antsRegistration's B-spline and displacement-field stages (ANTs v2.6.5, ITK v5.4.5)

What the `BSplineSyN`, `GaussianDisplacementField`, `BSplineDisplacementField` and `BSpline`
stages of `antsRegistration` compute, found while porting them (milestone A5, part 2,
2026-10-20). File names are at the tags `ANTs-v2.6.5` (`Examples/`) and `ITK-v5.4.5`
(`Modules/`). SyN itself, the metrics with displacement fields and the outputs they share are in
[ants-syn.md](ants-syn.md), the linear stages in [ants-registration.md](ants-registration.md);
the parity record is [ants-registration](../validation/ants-registration.md), the API page
[ants-registration](../api/ants-registration.md). The port: `larmorx_registration::bspline_field`
(the B-spline approximation of fields), `::syn` (B-spline SyN), `::field` (fields with smoothing
on update), `::transform::bspline` (the B-spline transform and its adaptor), `::method` (the
stages), and `larmorx_ants::registration::{bspline_syn, helper}`.

## In short

- **Four stages, two families.** `BSplineSyN` is SyN with every Gaussian smoothing replaced by a
  B-spline approximation (`BSplineSyNImageRegistrationMethod`). `GaussianDisplacementField` and
  `BSplineDisplacementField` optimise one displacement field with ITK's generic
  `ImageRegistrationMethodv4` and conjugate-gradient line search, the field's update and total
  field smoothed after each step (`GaussianSmoothingOnUpdateDisplacementFieldTransform`,
  `BSplineSmoothingOnUpdateDisplacementFieldTransform`). `BSpline` optimises a cubic
  `BSplineTransform` (a control-point grid over the fixed image) the same way.
- **Bit-identical to ANTs at one thread**, every compared case of the four stages, most against
  the unmodified binary (the rest through glibc's `log`/`exp`, identical with CORE-MATH
  preloaded): see the record.
- **Not repeatable across thread counts in ANTs.** ITK's B-spline fit
  (`BSplineScatteredDataPointSetToImageFilter`) accumulates each work unit's points into its own
  lattices and adds them up, so `BSplineSyN` and `BSplineDisplacementField` change with the
  thread count; the linear-stage reductions do the same for `BSpline` (a transform of global
  support: per-work-unit compensated sums). larmorx gives the one-thread result at any thread
  count: the fit's control points are split among tasks along the last axis, each adding its
  points in order.
- **Defects found:** ANTS-75 extended (the observer's transform copies refuse B-spline
  transforms too), ANTS-76 (the collapse loses transforms around B-spline transforms), ANTS-77
  (a mesh size of 0, or fewer mesh sizes than dimensions), ANTS-78 (c?: `BSplineDisplacementField`
  doubles the "base level" mesh at the base level). In the [register](upstream-defects.md).
- **A B-spline transform has no Jacobian with respect to position in ITK** (not implemented,
  `itkBSplineBaseTransform.h:301`): any stage after a `BSpline` stage (or with a B-spline
  initial transform) whose metric needs the composite's Jacobian fails at its first derivative:
  linear and displacement-field stages and `BSpline` stages; SyN and `BSplineSyN` work (their
  metrics see identity fields). A linear stage's failure is dropped and the run goes on
  (ANTS-49); the others end the run with exit 1. larmorx fails the same way, with ITK's message.

## BSplineSyN (`BSplineSyN[gradientStep,updateMesh,totalMesh,splineOrder]`)

- **The method** is SyN's ([ants-syn.md](ants-syn.md)) with `BSplineSmoothDisplacementField`
  instead of `GaussianSmoothDisplacementField` for the update and the total fields, both
  directions. The mesh sizes are at the base level and double at each level
  (`mesh << level`); the number of control points is the mesh plus the order where the mesh is
  positive, else 0 (`itkantsRegistrationHelper.hxx:2236-2310`). A mesh of 0 leaves the field as
  it is (a copy); the default total mesh is 0.
- **The fit** (`DisplacementFieldToBSplineImageFilter`, `BSplineScatteredDataPointSetToImageFilter`,
  one level): every voxel of the field is a point, at its parametric position (the physical point
  in `float`, mapped back to a continuous index in double and onto the B-spline domain without
  the direction, in `float`); the boundary is stationary (the boundary voxels are points of
  displacement 0 and weight 1e10); the control-point lattice is Lee, Wolberg and Shin's
  (`ω += w·B²`, `δ += v·B³·w/ΣB²`, `φ = δ/ω`), the field the B-spline of the lattice on the grid.
  The filter's `RealType` is `float`: parametric coordinates, kernel products, weights and `ω` are
  floats; the kernel values are double, each product rounded to float as it is formed (larmorx
  first rounded the kernel values to float: 1e-7 relative differences, found with an ITK probe).
- **Weighted masks:** `ComputeUpdateField` weighs each fit by the first metric's fixed (or
  moving) mask resampled onto the virtual domain through the half-way transform with linear
  interpolation (`ResampleImageFilter<Image<uchar>, Image<double>, R>`, default 0), so the
  mask's edge gets fractional confidences.
- **Point-set metrics:** with a single point-set metric (no image metric), the derivative is not
  stored per voxel: each fixed point's tangent-space derivative, times the optimizer weights, is
  fitted at the point's virtual position (`SetBSplineDomainFromImage` of the virtual domain);
  a point is inside when its continuous index is within half a voxel of the grid.
- **Knot spacing:** a single mesh value is a knot spacing in millimetres:
  `CalculateMeshSizeForSpecifiedKnotSpacing` takes per axis `ceil((size − 1)·spacing / knot)` of
  the stage's first fixed image, in the computation type, 0 where the domain is almost 0
  (`FloatAlmostEqual<float>`, which `FloatAlmostEqual(float, double)` selects). An unreadable
  image crashes ANTs (null pointer); an out-of-range quotient is ANTS-06.
- **Messages:** `*** Running B-spline SyN registration (updateMeshSizeAtBaseLevel = [..],
  totalMeshSizeAtBaseLevel = [..]) ***`; a mesh of another dimension: `ERROR:  The mesh size(s)
  don't match the ImageDimension.`; order 0: the B-spline filter's exception (`The spline order
  in each dimension must be greater than 0`).
- **Outputs:** as SyN's (`Warp`, `InverseWarp`); the stage's transform in the composite is a
  `BSplineSmoothingOnUpdateDisplacementFieldTransform` (later stages' `-i` listings show it).
  B-spline SyN does not set the registration state: `--save-state` after it is ANTS-50's case.

## GaussianDisplacementField and BSplineDisplacementField

- **The optimizer** is the linear stages' (`ConjugateGradientLineSearchOptimizerv4`, scales and
  learning rate from `RegistrationParameterScalesFromPhysicalShift`), the transform a
  displacement field on the stage's first fixed image, the metrics in their local-support mode
  through the composite (each point's derivative turned by the composite's Jacobian, `D` local
  parameters, at its virtual voxel). The observer prints `2DIAGNOSTIC` lines as for a linear
  stage.
- **Scales of a field** (`IsDisplacementFieldTransform`): `CentralRegionSampling` (the voxels
  within 5 of the central index), each of the `D` local parameters at the central voxel changed
  by 0.01 through `UpdateTransformParameters` (so through the smoothing on update), the largest
  shift squared over 0.01²; the step scale from every voxel (`FullDomainSampling`) without
  scaling the step down.
- **Smoothing on update** (`UpdateTransformParameters`): the update smoothed (Gaussian of the
  update variance, or B-spline of the update mesh's control points, when it has more than the
  order in every axis), added to the field, the total smoothed alike. The Gaussian transform
  smooths the **caller's** array in place (`const_cast`): the optimizer's gradient is smoothed
  by its own update. `EstimateStepScale(m_Gradient)` works on a converted temporary, so that
  call does not smooth the gradient (larmorx first smoothed it there: the second iteration
  differed).
- **Level adaptors:** the field resampled onto each level's virtual domain
  (`DisplacementFieldTransformParametersAdaptor`); `BSplineDisplacementField`'s meshes are
  `mesh << (level + 1)` (ANTS-78: doubled already at the base level, against the option's name
  and B-spline SyN's `mesh << level`).
- **A coarse mesh fails in ANTs:** the stationary boundary's weight (1e10) dominates a fit with
  few control points, so the scales estimator's 0.01 perturbation of one voxel, smoothed by the
  update (and total) fit, shifts no voxel by more than about 1e-13 mm; the scales fall below
  epsilon and the optimizer throws (`m_Scales values must be > epsilon`). A total mesh of
  1x1x1 in 3D (`displacement-field/bspline-coarse-total`), or a 4D update mesh of 2x2x2x1 on
  four volumes, fails so; larmorx fails alike.
- **Messages and failures:** IGDM first is refused (`Intensity point set metric is not
  implemented yet for the specified transform.`); a point-set metric first makes ANTs allocate
  the field on a null image (a crash; larmorx stops); `--print-similarity-measure-interval` and
  `--write-interval-volumes` fail at the first iteration (ANTS-75).
- **Outputs:** one field without an inverse: `Warp.nii.gz`, named `gdf` by the collapse;
  composites hold a `DisplacementFieldTransform`; no inverse composite and no inverse-warped
  image.

## BSpline (`BSpline[gradientStep,meshSizeAtBaseLevel]`)

- **The transform** is `BSplineTransform<R, D, 3>`: `D` coefficient images on a grid of
  `mesh + 3` control points per axis (the fixed parameters: size, origin, spacing, direction).
  A point inside the grid's valid region (`InsideValidRegion`: its continuous index within
  `[1, size − 2)`, the upper limit itself moved down by 6 ulps) moves by the coefficients of the
  `4^D` control points around it weighted by the cubic kernel (the weights' products in double,
  each term stored in `R`); outside it does not move. It has no inverse (no `InverseWarp`, no
  inverse composite, no inverse-warped image).
- **The domain** (`itkantsRegistrationHelper.hxx:3438-3530`): the first metric's fixed image (as
  preprocessed): origin, direction and physical dimensions `spacing·(size − 1)` (in `R`);
  `SetTransformDomainOrigin`, `…PhysicalDimensions`, `…MeshSize`, `…Direction` each recompute
  the fixed parameters from the others as the getters return them (`R` roundings in float), then
  `SetIdentity`. A point-set metric first makes ANTs read a null image (a crash).
- **Level adaptors** (`BSplineTransformParametersAdaptor`): at level `l` the mesh `mesh << l` on
  the level's shrunk virtual domain (its origin and direction, the stage's physical dimensions,
  all in double). When the fixed parameters change, each coefficient image is resampled onto the
  new grid (`ResampleImageFilter` with `BSplineResampleImageFunction`: the cubic B-spline of the
  coefficients as they are, coordinates in `R`, mirror boundaries) and turned back into
  coefficients (`BSplineDecompositionImageFilter<Image<R>>`: each axis in turn, each line in
  double, stored in `R` between axes). The base level is adapted too (the shrunk origin differs
  from the fixed image's).
- **Scales** (`IsBSplineTransform`): `CentralRegionSampling`; every parameter changed by 0.01 in
  turn, its largest sample shift; then **every scale is the smallest shift above epsilon**
  (`Fill(minNonZeroShift)`: neither squared nor divided by 0.01², unlike every other transform).
  ANTs maps every central sample for every parameter (slow: seconds for a 2 x 2 x 2 mesh); larmorx
  maps again only the samples a parameter's control point supports, the same values. The step
  scale comes from every voxel without scaling the step down. The weights (`-g`) are compared
  with the dimension (`PrepareRegistrationMethod`'s `parametersDimensionSize`), so any `-g` with
  `D` values makes the optimizer throw (`Size of weights (3) must equal number of local
  parameters (375)`).
- **The metrics** see a transform of global support with all its parameters local: each point's
  Jacobian is `D × parameters`, zero but at the point's `4^D` control points. ITK computes and
  adds the whole row for every point (each parameter's compensated sum receives a zero from
  every point away from its control point); larmorx computes only the non-zero columns and adds
  each parameter's zeros when it is next touched (or at the end), with the same compensated sums
  bit for bit (`CompensatedSum::add_zeros`: adding zero is a function of the sum's state, which
  reaches a fixed point after at most a step, so the deferred zeros give the state of the
  in-order ones). Demons refuses (it needs a displacement field).
- **What this costs ANTs on real data** (A5 part 3, 2026-10-22; one informal measurement on a
  loaded machine, not a benchmark). The mesh doubles at each level, and both the scales (each
  parameter perturbed in turn, the parameter vector copied each time, the central samples mapped)
  and every metric derivative (a `D × parameters` Jacobian row filled and summed at every voxel)
  grow with the number of parameters. sMRIPrep's template registration of ds000005 sub-01, on
  every third voxel, with `BSpline[0.1,26]` in place of its SyN stage (antsRegistrationSyN.sh's
  B-spline spacing; a mesh of 8 × 9 × 8 at the base level and 64 × 72 × 64 at the last, about a
  million parameters): ANTs 2.6.5 at 4 threads had not finished after 3 h 50 min, when it was
  stopped; for its last 2 h 30 min only its main thread was busy (as in the single-threaded
  scales). larmorx (`reproduce`, one thread), which computes each point's `4^D` non-zero columns
  only and maps again only the samples a parameter's control point supports, ran the whole
  registration in 38 s. The antsApplyTransforms evidence (its B-spline example) therefore takes
  its transform from a 104 mm spacing (a mesh of 2 × 3 × 2 at the base level).
- **Messages:** `*** Running bspline registration (meshSizeAtBaseLevel = [2, 2, 2]) ***`; a mesh
  size of 0, or fewer mesh sizes than dimensions (ANTs reads past its vector), is ANTS-77.
- **Outputs:** `BSpline.txt` (ITK's text format, the `BSplineTransform_double_3_3` parameters
  and fixed parameters as ITK writes numbers), named `bspline` by the collapse; composites hold
  the `BSplineTransform`. Reading B-spline transforms (`-r`, `-q`, composites) builds them as ITK's
  readers do: `SetFixedParameters`, then `SetParametersByValue`.
- **The collapse** (`CollapseCompositeTransform`, `-z 1`) knows linear and displacement-field
  runs only; around B-spline transforms it loses transforms (ANTS-76).
- **The observer's copies** (`--print-similarity-measure-interval`, `--write-interval-volumes`):
  `CreateAnother()` gives a B-spline transform of mesh 1 (`D·4^D` parameters), `SetParameters`
  before `SetFixedParameters` throws for any other mesh (ANTS-75).

## antsApplyTransforms

`larmorx ants antsApplyTransforms` does not read `BSplineTransform` files yet (refused with a
message): apply a `BSpline` stage's result with `antsRegistration`'s own warped image, or with
ANTs. To do with antsApplyTransforms' next part.
