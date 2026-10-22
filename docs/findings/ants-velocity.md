# antsRegistration's velocity-field stages (ANTs v2.6.5, ITK v5.4.5)

What the `Exponential`, `BSplineExponential`, `TimeVaryingVelocityField` and
`TimeVaryingBSplineVelocityField` stages of `antsRegistration` compute, found while porting them
(milestone A5, part 3, 2026-10-22), and the corrections decided on 2026-10-21 with their
measured effects. File names are at the tags `ANTs-v2.6.5` (`Examples/`) and `ITK-v5.4.5`
(`Modules/`). SyN and the outputs the deformable stages share are in [ants-syn.md](ants-syn.md),
the B-spline stages in [ants-bspline.md](ants-bspline.md), the linear stages and point-set
metrics in [ants-registration.md](ants-registration.md); the parity record is
[ants-registration](../validation/ants-registration.md), the API page
[ants-registration](../api/ants-registration.md). The port: `larmorx_registration::exponential`
(the exponential transforms), `::time_varying` (time-varying velocity fields and their stage),
`::tv_bspline` (the time-varying B-spline stage), `::field` (the transforms' updates and
adaptors), `::optimizer::GradientDescent`, and `larmorx_ants::registration::{velocity, helper}`.

## In short

- **Two families.** `Exponential` and `BSplineExponential` optimise a stationary velocity field
  `v` whose displacement is `exp(v)` (scaling and squaring), with ITK's generic registration
  method and a plain gradient-descent optimizer whose learning rate is the stage's gradient step
  (no scales estimator). `TimeVaryingVelocityField` and `TimeVaryingBSplineVelocityField` run
  their own registration methods: a velocity field over space and time (a dense image in
  `D + 1` dimensions, or a B-spline lattice over them) whose displacement is its flow from time 0
  to 1, integrated by fourth-order Runge–Kutta.
- **Bit-identical to ANTs at one thread**, every compared case of the four stages, most against
  the unmodified binary (the rest through glibc's `log`/`exp`, identical with CORE-MATH
  preloaded): see the record. larmorx's result does not depend on the thread count (every
  voxel's integration on its own, every sum in one work unit's order).
- **Defects found** (in the [register](upstream-defects.md)): ANTS-79 (the number of time
  indices and the time-varying B-spline mesh are read from the last `-t` option), ANTS-80 (the
  exponential stages take their smoothing sigmas as millimetres), ANTS-81 (`Exp`, the short
  name, writes no `InverseWarp`), ANTS-82 (three of the stages never seed their sampling: ANTs
  samples from the clock), ITK-36 to ITK-38 (the time-varying B-spline method's multi-metric
  transforms, stale inverse fields, physical points in a parametric domain), ITK-39 (`-q`
  ignored by the time-varying method), ITK-40 (masks not moved with the resampled images),
  ITK-41 (a velocity transform that never integrated), ITK-42 (equal scales below epsilon);
  ANTS-75 and ANTS-77 extended to these stages.
- **The corrections decided on 2026-10-21** are in place, each with `reproduce` giving ANTs'
  result: ANTS-78, ITK-35, ANTS-67, ANTS-68, ANTS-70, ITK-24, ITK-29 (measured below).

## Exponential (`Exponential[gradientStep,updateVariance,velocityVariance,<steps>]`)

- **The transform** is ITK's `GaussianExponentialDiffeomorphicTransform`, a
  `ConstantVelocityFieldTransform`: the velocity field `v` (an image of vectors on the level's
  virtual domain) and its displacement `exp(v)`, computed by `ExponentialDisplacementFieldImageFilter`:
  `v / 2^n`, then `n` times `u ← u + u∘(x + u)`, each warp a `WarpVectorImageFilter` with
  `VectorLinearInterpolateNearestNeighborExtrapolateImageFunction` (linear inside, the nearest
  voxel outside), continuous indices in double. Its inverse is `exp(−v)` the same way.
- **The number of squarings** is the stage's `numberOfIntegrationSteps` when given (ANTs sets
  10 when it is not, `SetNumberOfIntegrationSteps` only above 0) or computed by the filter: the
  largest squared norm of `v`'s vectors over the smallest spacing squared gives
  `⌊2 + ½·log₂(max)⌋ + 1`, at most 20. A field with an infinite vector makes the conversion to an
  integer undefined (x86-64 gives 0): larmorx clamps it to the maximum (`reproduce`: 0).
- **The update** (`UpdateTransformParameters`): the metric's derivative over the virtual domain
  (one vector per voxel), smoothed by a Gaussian of variance `updateVariance` (in place: the
  optimizer keeps the smoothed array), added to `v` times the learning rate, `v` smoothed by a
  Gaussian of variance `velocityVariance`, then integrated again. The smoothing takes the
  variances in voxels² (`GaussianOperator` per axis, the field's boundary kept).
- **The optimizer** (`optimizer2` in `itkantsRegistrationHelper.hxx:1403`) is
  `GradientDescentOptimizerv4` without a scales estimator: the learning rate is the gradient step
  for every iteration; convergence when the window's value is ≤ the threshold; the observer is
  `antsRegistrationOptimizerCommandIterationUpdate` (the `2DIAGNOSTIC` lines).
- **Levels** (`GaussianExponentialDiffeomorphicTransformParametersAdaptor`): the velocity field is
  resampled onto each level's domain (linear, the vectors as they are) and integrated again.
- **The smoothing sigmas are millimetres** whatever their units (ANTS-80): the stage never
  passes `-s ...vox` to its method, whose default is physical units. Corrected: voxels as given.
- **Outputs.** `Warp.nii.gz` and `InverseWarp.nii.gz`; collapsed with the other stages' fields;
  with `-z 0` the stage's files carry its given name's suffixes, and `VelocityField.nii.gz` is
  written for `Exponential` and `TimeVaryingVelocityField` (and their short names: the list of
  names with a velocity field has `exp`, `exponential`, `tvf`, `timevaryingvelocityfield` and
  `bsplineexponential`, but a B-spline exponential transform is not a Gaussian one, the cast
  fails and nothing is written). `-z 0` with the short name `Exp` writes no `InverseWarp`: the
  list of transforms with an inverse has only the long names (ANTS-81; corrected, written).
- **Failures.** `--print-similarity-measure-interval` and `--write-interval-volumes` copy the
  transform for the observer and fail (ANTS-75: ITK's `SetParameters` on a copy whose field has
  no size). `--save-state` saves nothing (ANTS-50). A point-set metric first crashes (the field
  is allocated on the first metric's image, null). A stage of no iteration on one level never
  integrates: the transform keeps `v` as its field and has no inverse field, so the inverse
  warped image throws "No displacement field is specified." (ITK-41).

## BSplineExponential (`BSplineExponential[gradientStep,updateMesh,velocityMesh,<steps>,<order>]`)

- `BSplineExponentialDiffeomorphicTransform`: as `Exponential`, with the update and the velocity
  field smoothed by B-spline approximations (`BSplineSmoothDisplacementField`, the boundary
  pinned, [ants-bspline.md](ants-bspline.md)) of the given meshes (a single number is a knot
  spacing in mm), a mesh of 0 leaving the field as it is. The adaptor gives each level the
  meshes `mesh << (level + 1)`: the "base level" mesh is doubled already at the base level, as
  in `BSplineDisplacementField` (ANTS-78; corrected, `mesh << level`).

## TimeVaryingVelocityField (`TimeVaryingVelocityField[gradientStep,timeIndices,updateVariance,updateTimeVariance,totalVariance,totalTimeVariance]`)

- **The transform** (`GaussianSmoothingOnUpdateTimeVaryingVelocityFieldTransform`): a velocity
  field of `D` components over `D + 1` dimensions (space, then `timeIndices` time points), its
  displacement the flow from time 0 to 1 and its inverse the flow back
  (`TimeVaryingVelocityFieldIntegrationImageFilter`: per voxel, fourth-order Runge–Kutta, the
  velocity linearly interpolated in space and time, the point rounded to `R` before its
  continuous index; `TimeBoundsAsRates`).
- **The method** (`TimeVaryingVelocityFieldImageRegistrationMethodv4`), per iteration and per
  time point `t`: the fixed images resampled through the flow from `t` back to 0, the moving
  images through the moving initial transforms and the flow from `t` to 1; the metric's
  derivative on the virtual domain with identity transforms, scaled to a largest norm of 1; the
  update field averaged with the previous iteration's, smoothed in space and time (Gaussians of
  the update variances), added to the velocity times the learning rate, the total smoothed;
  the metric value the mean over the time points. The level ends with the flows 0→1 and 1→0.
  Levels resample the velocity field (`TimeVaryingVelocityFieldTransformParametersAdaptor`).
- **Its own observer** (`antsRegistrationCommandIterationUpdate`): `XDIAGNOSTIC`/`WDIAGNOSTIC`
  lines, the stream left in scientific notation of precision 4; no full-scale similarity and
  no interval volumes.
- **Outputs:** `Warp`, `InverseWarp`, and with `-z 0` the velocity field itself
  (`VelocityField.nii.gz`, a `D + 1`-dimensional vector image; in 4D registrations the
  5-dimensional field cannot be written as NIfTI: ITK's writer refuses and ANTs goes on).
- **Defects:** the number of time indices is read from the last `-t` option of the command line,
  not the stage's own (ANTS-79); `-q` is ignored (ITK-39); the masks are given to the metric as
  they are while the images are resampled through the flows (ITK-40); the sampling is seeded
  from the clock (ANTS-82); a stage that never iterates never integrates, and the next use of the
  transform crashes (ITK-41); 0 time indices divide by 0 (a crash); a point-set metric first
  crashes. Point-set metrics after an image metric run in tangent space as in SyN (ITK-32's
  frozen per-label metrics included).

## TimeVaryingBSplineVelocityField (`TimeVaryingBSplineVelocityField[gradientStep,mesh,samples,order]`)

- **The velocity field is a B-spline over space and time**: a control-point lattice of the
  given mesh (`D + 1` sizes; ANTs prints "The transform domain mesh size does not have the
  correct number of elements." and fails otherwise), sampled at `samples` times on each level's
  domain (`BSplineControlPointImageFilter`) and integrated like the time-varying field.
- **Each iteration** evaluates the metric's derivative at every time sample (as above), scales
  each sample's derivatives to a largest norm of 1, averages them with the previous
  iteration's, fits one B-spline in space and time to all of them
  (`BSplineScatteredDataPointSetToImageFilter`, one level, time spacing 1/(samples − 1), the
  domain's boundary pinned with weight 10¹⁰, a fixed mask as the weights), and adds that
  lattice, times the learning rate, to the velocity's.
- **Levels** (`TimeVaryingBSplineVelocityFieldTransformParametersAdaptor`): the lattice is
  refined by B-spline resampling (cubic whatever the order) and decomposed with the
  transform's order; the per-level adaptors have no direction (the identity) and order 3.
- **Point-set metrics:** a single point-set metric gives its derivative at the fixed points,
  fitted with the boundary points at their physical positions in the B-spline's parametric
  domain, which leaves out the domain's direction: with any direction but the identity the
  points fall outside the domain and the fit throws (ITK-38; the parity cases run this path on
  phantoms stored so ITK reads them with the identity direction). Among several metrics, the
  method sets each metric's transforms on the whole multi-metric (ITK-36: point-set metrics
  before an image metric see identity fields, image metrics after one are warped twice) and
  keeps the previous integration's inverse fields at the first and last samples (ITK-37); a
  point-set metric first among several throws "Invalid conversion from the multi metric queue."
- **Other defects:** the mesh is read from the last `-t` option (ANTS-79); a mesh size of 0
  divides by zero and fails on a domain of no span (ANTS-77: refused).

## Antsapplytransforms with B-spline transforms

`antsApplyTransforms` applies `BSplineTransform` files (`antsRegistration`'s `BSpline` stage
output; `.txt`, `.mat`, `.h5`, inside composites, double and float) as ITK's
`BSplineTransform::TransformPoint` does (`larmorx_transform::BSplineTransform`): the point's
continuous index on the coefficient grid, moved inside the valid region as `InsideValidRegion`
does (an index within 4 ulps of the upper limit moved 6 ulps below it), the 4 × 4 × 4 support's
cubic weights, the weighted coefficients summed in `R`; outside the valid region the point is
unchanged. A B-spline transform has no inverse: `-t [bspline,1]` fails ("Inverse does not
exist"), `-o Linear[...]` with one in the chain fails ("not linear"), `-o CompositeTransform[...]`
writes it as it is, `-o DisplacementField[...]` samples it. Every case is bit-identical to ANTs
([record](../validation/ants-apply-transforms.md)).

## The corrections decided on 2026-10-21, and what they change

Measured by larmorx in both modes (`reproduce` is ANTs' result, bit for bit) on the suite's
phantoms; the parity suite's fix pass reports the same corrections case by case.

- **ANTS-78** (`BSplineDisplacementField`, `BSplineExponential`: the base-level mesh used at the
  base level). The corrected stage with twice the mesh is ANTs' stage byte for byte (verified:
  `BSplineDisplacementField[0.3,6x6x6,0,3]` corrected = `[0.3,3x3x3,0,3]` reproduced over two
  levels; `BSplineExponential` 8x8x8 = 4x4x4). On the registration phantoms (two levels) the
  fields move by up to 3.0 to 5.5 mm (mean 1.1 to 2.1 mm) for meshes of 3 to 5 and knot spacings
  of 12 mm, 0.12 mm for `BSplineExponential` 4x4x4, the fits (mean squared difference in the
  mask) within 1 % to 14 % of ANTs' (the coarser corrected mesh is smoother). A coarse mesh at a
  shrunken level used to fail (every control point touches the pinned boundary, the scales
  estimator's probe vanishes: ITK-42); with ITK-42 corrected it runs.
- **ITK-42** (equal scales below epsilon). The scales estimator probes a transform of local
  support with a unit change of the central voxel's parameters; a B-spline-smoothed update pinned
  to its boundary everywhere returns about 4e-24 for every scale, and ITK refuses scales ≤ ε.
  Equal scales cancel in the learning rate the optimizer estimates (the scaled gradient is
  `g/s`, the estimated rate proportional to `s`): larmorx uses ones. `BSplineDisplacementField
  [0.3,3x3x3,0,3]` over two levels then runs (mean squared difference 30082 against ANTs'
  30136 with its doubled mesh).
- **ITK-35** (a B-spline transform's Jacobian with respect to position: the identity plus
  `Σ c_k ⊗ ∇w_k`, the cubic kernel's derivative along one axis times its values along the
  others, through the grid's point-to-index matrix). Validated numerically: against central
  finite differences of `TransformPoint` (2D and 3D, oblique grids, within 1e-6), of the
  composite's Jacobian with respect to an affine transform's parameters through it (1e-5), and of
  MeanSquares' value through the composite (the derivative is minus the value's gradient, scale
  −0.99995, 3.6 % residual, as accurate as without the B-spline transform, 2.0 %: the residual is
  the central-difference image gradient). Against the closest runs ANTs can do, with the
  interior mask (unit tests and runs in `<scratch>`, numbers in the record's fix pass):
  - a `BSpline` stage then `Affine` (ANTs drops the affine stage, ANTS-49): within 0.37 mm (mean
    0.19 mm) of ANTs' affine registration to the moving image resampled through the B-spline
    transform, with a mean squared difference (computed exactly through both transforms) 3 %
    above it; then `Rigid`: within 0.29 mm (mean 0.14 mm), equal fit;
  - two `BSpline` stages (ANTs fails): within 0.42 mm (mean 0.21 mm) of ANTs' second stage with
    the first B-spline transform sampled as a displacement field on the moving grid, 0.24 mm
    (mean 0.13 mm) on a grid of half the spacing: the difference is the field's sampling;
  - `-r bspline.txt` then `Rigid`: a lower mean squared difference than either ANTs construction.
  ANTs' runs with the B-spline transform as a displacement field stop early on linear stages
  (ITK's Jacobian of a field is its nearest voxel's, so the derivative is approximate); larmorx
  given the same field stops with them, identically. Without a mask a linear stage after a
  B-spline transform can stall: the transform is the identity outside its valid region, so
  where its coefficients are not 0 at that border the composite jumps there, and points crossing
  it make the metric discontinuous. Use a mask (fMRIPrep's registrations have one).
- **ANTS-67, ANTS-68** (IGDM's `kNeighborhood`, `gradientSigma` and true gradient). On the
  point-set phantoms (the moving blocks shifted by (−2, 2, 2.5) mm), Rigid, 40 iterations,
  given sigmas (50, 5): ANTs' IGDM does not move (its derivative, the two terms weighed by
  σ² instead of 1/σ², is no descent direction the line search can use; translation error
  3.77 mm, the whole shift); corrected with one neighbour 3.30 mm, with 5 neighbours 0.92 mm,
  with 5 neighbours and Gaussian gradients of σ 1.5 mm 0.62 mm.
- **ANTS-70** (IGDM's estimated distance sigma at least one voxel). An image registered to itself:
  ANTs' sigma is 0, the parameters NaN after the first step, the stage throws and is dropped;
  corrected, the stage runs and stays at the identity (value −1). On the phantoms the estimate
  (the standard deviation of the nearest distances) is below the 2 mm voxel and the floor applies.
- **ITK-24** (JHCT's total number of points with the fixed points). The total scales the value and
  the derivative alike (and shifts the logarithm by a constant), which the line search and the
  estimated learning rate cancel: parameters within 1.1e-5 of ANTs' (isotropic, sigma 2).
- **ITK-29** (the Parzen kernel sigma used as a sigma, covariance 100·I instead of 10·I). It
  weighs the neighbours of the local covariances (anisotropic only). With ITK-24 and ITK-26:
  translation error 0.027 → 0.010 mm (sigma 1, α 1.1) and 0.104 → 0.058 mm (sigma 2, α 1.5).

## NaN signs

ANTs prints IGDM's exception for a point of NaN coordinates ("The corresponding data for point
[-nan, -nan, nan] ... does not exist."). IEEE 754 leaves the sign of a NaN result unspecified
(only copy, negate and abs define it); x86 propagates the first operand's NaN, so the sign
depends on the order in which the compiler places the operands. larmorx computes the same NaN
coordinates and prints their signs as they come; the parity suite masks NaN signs
(`-nan` → `nan`).

## Speed

At 4 threads larmorx runs the time-varying stages 4 to 6 times faster than ANTs at one thread
on the 40 × 44 × 36 phantoms (TimeVaryingVelocityField about 5 to 11 s in ANTs, TimeVaryingBSpline
about 16 to 22 s), the exponential ones about as fast as their metric. No benchmark was run
(the machine was shared with another agent).
