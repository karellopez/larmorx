# antsRegistration's SyN stages (ANTs v2.6.5, ITK v5.4.5)

What a `-t SyN[gradientStep,updateFieldVariance,totalFieldVariance]` stage of `antsRegistration`
computes, found while porting it (milestone A5, part 1, 2026-10-19). File names are at the tags
`ANTs-v2.6.5` (`Examples/`, `ImageRegistration/`, `Temporary/`) and `ITK-v5.4.5` (`Modules/`).
The linear stages, the metrics and the outputs they share are in
[ants-registration.md](ants-registration.md); the parity record is
[ants-registration](../validation/ants-registration.md), the API page
[ants-registration](../api/ants-registration.md). The port: `larmorx_registration::syn`
(`run_syn`), `::field` (the displacement-field filters), the metrics' local-support mode
(`metric/`), `pointset::metric` (tangent space), and `larmorx_ants::registration::syn` (the
helper's `case SyN` and its observer).

## In short

- **The algorithm is ITK's `SyNImageRegistrationMethod`** as ANTs configures it:
  `DownsampleImagesForMetricDerivatives` on, `AverageMidPointGradients` off, a fixed-to-middle
  and a moving-to-middle `DisplacementFieldTransform`, each with its inverse field. Each
  iteration resamples the images through the current half-way transforms onto the level's
  virtual domain, computes the metric's derivative twice (once per direction, the roles of the
  images swapped), smooths and scales the two updates, composes them into the fields, smooths
  the total fields, and inverts each field twice.
- **ANTs' SyN is (almost) independent of its thread count.** The metric derivative of a
  displacement field is stored per voxel, not summed; only the metric values (and Mattes' joint
  histogram) are reduced per work unit. At 2, 4 and 8 threads the fields moved by 0 (CC,
  MeanSquares) to 3.6e-14 mm (Mattes) from ANTs' one-thread fields. larmorx gives the one-thread
  result at any thread count.
- **Bit-identical to ANTs at one thread**: every compared SyN case of the standard tier, most
  against the unmodified binary, the rest (Mattes, MI2, float runs: glibc's `logf`/`log`/`exp`)
  once CORE-MATH's correctly rounded functions are preloaded.
- **Seven defects found:** ANTS-71 (the full-scale similarity crashes), ANTS-72 (a linear stage
  after a field is not repeatable), ANTS-73 (any image accepted as an initial field), ANTS-74
  (a stale saved state), ITK-30 (GC's derivative in one voxel), ITK-31 (stale masks), ITK-32
  (frozen point-set metrics), ITK-33 (point-set metrics among several metrics never see the
  deformation). All in the [register](upstream-defects.md); every deterministic one has a
  switch (`LARMORX_UPSTREAM_DEFECTS=reproduce`).
- **Fast:** on a 96³ phantom pair (CC radius 4, 10 and 5 iterations at shrink 2 and 1) ANTs
  takes 122 s at one thread; larmorx about 24 s at one thread and 5 s at 12, bit-identical
  (one measurement on a busy machine; the benchmark report is pending).

## One iteration (`SyNImageRegistrationMethod::StartOptimization`)

1. The fixed composite is the fixed initial transform (`-q`) then the inverse of the
   fixed-to-middle transform; the moving composite the registration's composite (`-r` and the
   earlier stages) then the inverse of the moving-to-middle transform.
2. `ComputeUpdateField` for the fixed-to-middle field: each image metric's fixed and moving
   images (smoothed for the level) are resampled onto the level's virtual domain through the
   composites (`ResampleImageFilter`, linear interpolation, default 0), its masks with the
   nearest neighbour; the metric gets identity displacement field transforms on the virtual
   domain (`HasLocalSupport()`), so its derivative is one vector per virtual voxel, not averaged
   ([the metrics](#the-metrics-with-local-support)). The same with the roles swapped gives the
   moving-to-middle update (the "moving" metric value comes from the first call, the "fixed"
   one from the second).
3. Each derivative field is multiplied by `-g`'s weights (only when they have `D` values and are
   not all within 1e-4 of 1), smoothed (`GaussianSmoothDisplacementField`, the update variance)
   and scaled so its largest norm in voxels is the gradient step (`ScaleUpdateField`).
4. Each update is composed with its field (`ComposeDisplacementFieldsImageFilter`, the update as
   the displacement field and the current field as the warping field) and the sum smoothed with
   the total variance.
5. Each total field is inverted from the previous inverse (`InvertDisplacementFieldImageFilter`:
   20 iterations at most, mean tolerance 0.001, max tolerance 0.1 in voxels, step ε = 0.75 then
   0.5, the boundary pinned to 0), and that inverse inverted again from the total field: the
   new field.
6. The metric value is `0.5·(moving + fixed)` (stored in `R`); `WindowConvergenceMonitoringFunction`
   gives the convergence value, and the level stops when it is **strictly below** the threshold.

At each new level the two transforms are resampled onto the level's domain
(`DisplacementFieldTransformParametersAdaptor`, a linear `ResampleImageFilter` with an identity
transform) unless their geometry already equals it. The stage's output transform is the
moving-to-middle inverse composed with the fixed-to-middle field (`ComposeDisplacementFieldsImageFilter`,
`GetModifiableTransform`), its inverse the fixed-to-middle inverse composed with the
moving-to-middle field.

`GaussianSmoothDisplacementField(field, variance)`: a copy for a variance ≤ 0; else
`GaussianOperator` (maximum error 0.001, maximum width the field's size along the axis; the
kernel ITK-11 notes) along each axis in turn through `VectorNeighborhoodOperatorImageFilter`
(zero-flux Neumann boundary), then every boundary voxel set to 0 and, for variances below 0.5,
the result blended with the input (`w₁ = 1 − variance/0.5`).

## The metrics with local support

With a displacement-field moving transform an image metric stores each virtual voxel's
derivative in that voxel's `D` parameters (`HasLocalSupport()`), and does not divide by the
number of points. The value is still the mean over the valid points.

- **Mattes** has its own local path: each point's derivative from the Parzen-window
  derivatives of its own bins (`m_LocalDerivativeByParzenBin`, `m_JointPdfIndex1DArray`,
  the log-ratio array `m_PRatioArray` times the normalisation factor), computed after the joint
  histogram is complete.
- **MI2** (`JointHistogramMutualInformationImageToImageMetricv4`), **CC**, **MeanSquares**: the
  point's derivative where the global path adds it to the sum.
- **Demons** (`DemonsImageToImageMetricv4`, only allowed with a displacement field): the fixed
  image's gradient (gradient source fixed), the normaliser the mean squared spacing, the
  thresholds 0.001 (intensity difference) and 1e-9 (denominator). Its `Initialize` throws "The
  moving transform must be a displacement field transform" for any other transform, which is
  why it fails in linear stages.
- **GC** (`CorrelationImageToImageMetricv4`): its constructor is meant to refuse displacement
  fields but the check never fires; its derivative is then accumulated into the first voxel's
  entries only (ITK-30).
- **Several metrics** (`ObjectToObjectMultiMetricv4`): `Initialize` normalises the weights to
  sum 1, and SyN calls it twice per iteration; the derivative is the magnitude-weighted
  combination, the value the first metric's.
- **Sampling** (`Regular`, `Random`) draws one point set per metric per level from the first
  metric's fixed mask, as in a linear stage.
- **Masks** are resampled with the nearest neighbour through the composites at every iteration,
  only for the roles that have one (ITK-31).

## Point-set metrics

ICP, PSE and JHCT (`LabeledPointSetToPointSetMetricv4`) and IGDM run in **tangent space**
(`SetCalculateValueAndDerivativeInTangentSpace(true)`): both point sets are mapped into the
virtual domain by the inverses of their transforms (the fixed points by the fixed composite's
inverse, the moving points by the moving composite's), each point's local derivative is added
to the virtual voxel nearest it (`SetStoreDerivativeAsSparseFieldForLocalSupportTransforms`),
in point order, and the rest of the derivative field is 0 before the Gaussian smoothing. IGDM
also maps the moving points' gradients (`TransformCovariantVector` at each point: the
transposed inverse matrix of a linear transform, `I − ∇u` at the nearest voxel for a field).
What ANTs adds:

- **ITK-32:** the labelled metric appends one metric per label at every `Initialize` and keeps
  using the first ones, so every evaluation compares with the moving points as the stage's first
  evaluation mapped them, and the moving-to-middle update compares the moving points with
  themselves.
- **ITK-33:** with several metrics SyN sets the point-set metrics' transforms and then gives
  every metric of the multi-metric identity displacement field transforms (meant for the
  pre-resampled image metrics), so a point-set metric among several never sees the initial
  transforms or the deformation.
- **ANTS-61:** ANTs' SyN method holds labelled point sets only: IGDM first is refused ("Intensity
  point set metric is not implemented yet for the specified transform."); IGDM after another
  metric gets no input ("Mismatch between number of image pairs and the number of metrics.", or
  a crash when a metric with input follows it).
- A transform that cannot be inverted (a displacement field read without its inverse, `-r
  field.nii.gz`) makes ITK map the points with a null transform: a crash.
- Labels: ITK-22 now in both directions (the moving-to-middle update looks the moving points up
  in the fixed set's labels).

## The observer (`antsRegistrationCommandIterationUpdate` for SyN)

ANTs prints, at the first iteration of each level, `XXDIAGNOSTIC,Iteration,metricValue,convergenceValue,ITERATION_TIME_INDEX,SINCE_LAST`
(followed by `,FullScaleCCInterval=<N>` when `--print-similarity-measure-interval` is set), then
one line per iteration: `" 1DIAGNOSTIC, "`, the iteration in a field of 5, the metric and
convergence values in scientific notation with 12 digits, the two times with 4, and the
full-scale CC (12 digits) every interval. Afterwards it resets the stream to the default
notation with the precision it had before. SyN never fires the `InitializeEvent`: there are no
"Current level" lines. An exception prints "Exception caught: " and the exception, and the
program exits 1 (no stage is dropped, unlike ANTS-49's linear stages).

`--write-interval-volumes N` writes the moving image warped through the current composite at
every N-th iteration, `<prefix>Stage<n>_level<l>_Iter<iiii>.nii.gz`, and prints `*<name>`.

## Initial transforms and outputs

- **Displacement fields among the initial transforms.** `itk::ants::ReadTransform` reads any file
  whose last extension is not a transform file's (`.h5`, `.hdf5`, `.hdf4`, `.mat`, `.txt`,
  `.xfm`) as a displacement field image (`ImageFileReader<Image<Vector<R, D>, D>>`), without an
  inverse: `-r [field,1]` fails ("cannot be inverted"). A composite `.h5` may hold
  `DisplacementFieldTransform` entries (their fixed parameters: size, origin, spacing,
  direction). ITK's reader converts whatever the image holds into vectors (ANTS-73).
- **Collapsing** (`CollapseCompositeTransform`): runs of linear transforms become one
  `GenericAffine`; runs of displacement fields are composed into one field
  (`CollapseDisplacementFieldTransforms`, the inverse fields composed too when every field has
  one), named `Warp`/`InverseWarp` (SyN's with an inverse; a field without one is written as a
  Gaussian displacement field's `Warp` only). The last transform, when it starts a new run, is
  added uncollapsed.
- **Files:** a field is written as `<prefix><n>Warp.nii.gz` and its inverse
  `<prefix><n>InverseWarp.nii.gz` (NIfTI vector images, intent 1007, double or float); where
  ANTs writes the inverse of a field that has none (a field read from a file, or composed with
  one), ITK's writer prints "No input to writer!" and the run goes on. In composites
  (`-a 1`) fields are stored forward only (`DisplacementFieldTransform_double_3_3`, parameters
  the vectors); the inverse composite holds the inverse fields.
- **`--save-state`**: the registration state of the last SyN stage, its fixed-to-middle and
  moving-to-middle transforms each followed by its inverse; `--restore-state` reads such a file
  back (the last four transforms) and restarts SyN from it ("Initial FixedToMiddle and
  MovingToMiddle transforms are restored from the registration state file."). After linear
  stages that follow, the saved state is stale (ANTS-74).
- **`--initialize-transforms-per-stage`**: a SyN stage after another starts from its fields
  ("Current SyN transform is directly initialized from the previous stage."), the earlier
  transforms as its moving initial transform.

## Not repeatable in ANTs

- **ANTS-72:** a linear stage whose moving composite holds a displacement field estimates its
  parameter scales and learning rate from 1000 random virtual voxels
  (`RegistrationParameterScalesFromPhysicalShift`, `SizeOfSmallDomain`), drawn with ITK's
  `ImageRandomConstIteratorWithIndex`, whose generator is seeded from the clock: two runs of
  ANTs differ, whatever `--random-seed` says. larmorx draws them with the stage's seed.

## Validation notes

- Real data: sMRIPrep's brain extraction (`antsBrainExtraction`'s OASIS template, Rigid, Affine,
  then SyN with two CC metrics and the gradient filter) and template registration
  (MNI152NLin2009cAsym, Rigid, Affine, SyN with CC radius 4), as fMRIPrep 25.2.6 ran them, on
  every third voxel of their images (standard tier, ds000005) and at full size (full tier).
  The intermediates nipype did not keep are stood in for as in the linear stages' real cases
  (A4), plus the template's Laplacian (`lap_tmpl`), made by the oracle's `ImageMath`.
