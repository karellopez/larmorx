# antsAI and antsAffineInitializer (ANTs v2.6.5, ITK v5.4.5)

What `antsAI` and `antsAffineInitializer` really compute, found while porting them (milestone
A4, part 3). File names are at the tags `ANTs-v2.6.5` (`Examples/`, `Utilities/`) and
`ITK-v5.4.5` (`Modules/`). The parity records are [ants-ai](../validation/ants-ai.md) and
[ants-affine-initializer](../validation/ants-affine-initializer.md), the API pages
[ants-ai](../api/ants-ai.md) and [ants-affine-initializer](../api/ants-affine-initializer.md).
The v4 framework both programs run on (metrics, scales, the conjugate-gradient optimizer) is in
[ants-registration.md](ants-registration.md).

## In short

- **Both programs compute ANTs' one-thread result in larmorx, bit for bit**, on every case of
  their parity suites but the clock-seeded ones, including fMRIPrep's own `antsAI` command lines
  (sMRIPrep's brain extraction and the BOLD reference's skull-stripping) with fMRIPrep's seed.
  The only differences to the unmodified binary are glibc's `cbrt` (`Similarity3DTransform`'s
  scale) where it is not correctly rounded.
- **fMRIPrep's transform is ANTs' one-thread result within its thread band.** fMRIPrep ran
  `antsAI` with 8 threads (`NSLOTS=8`) and `ANTS_RANDOM_SEED=50495`: re-run in fMRIPrep's image
  (ANTs 2.6.2) at one thread, it equals the 2.6.5 oracle and larmorx bit for bit; at 8 threads
  it moves by about 1e-13 from run to run, and fMRIPrep's file is within 1.3e-13 of larmorx
  (ds000005, run 1; `oracles/logs/a43-bold-*.log`, 2026-10-17).
- **`Random` sampling is never repeatable**, whatever the seed: its voxels come from the
  iterator's own generator, seeded from the clock (ANTS-58); `--random-seed 0` does the same to
  `Regular` sampling's perturbations.
- Defects found: ANTS-52 (the translation grid ignored by some transforms), ANTS-53 (the
  alignments with iterations write no transform), ANTS-54 (antsAffineInitializer ignores its
  local-search argument), ANTS-55 (a short grid read past its end), ANTS-56 (an unknown
  dimension runs as 2D), ANTS-57 (two or three arguments abort), ANTS-58 (the clock), ANTS-59
  (the convergence window's documented default), ANTS-60 (the blob detector's per-thread heaps),
  ITK-19 (a start point that throws keeps the previous value), ITK-20 (the Mattes metric
  unusable after a refused histogram spacing), ITK-21 (the learning rate carried from one start
  point to the next). All in the [register](upstream-defects.md).

## antsAI

### The steps

`antsAI<D>` (`antsAI.cxx:610-1577`):

1. **The images** are `itk::Image<float, D>`, read from `--metric`'s first two parameters
   (whatever the metric's name; an unknown name is an error only when the search runs).
2. **The initial transform** is an `AffineTransform<double, D>`:
   - `AlignGeometricCenters` / `AlignCentersOfMass`: `CenteredTransformInitializer` (`GeometryOn`
     / `MomentsOn`): the rotation centre the fixed image's centre, the translation the moving
     centre minus it.
   - otherwise the centres of mass from `ImageMomentsCalculator`, **stored in
     `itk::Vector<float, D>`** (`antsAI`'s `VectorType`), so the centre and the translation are
     rounded to float; with `-p 1` the rotation of Wahba's problem between the principal axes
     (the last row of `GetPrincipalAxes` in 2D, the last two in 3D): `vnl_svd` of
     `B = Σ outer(moving axis, fixed axis)`, `A = vnl_inverse(V·Uᵀ)`; a negative determinant is
     turned positive by negating the columns whose diagonal entry is negative (`-v` prints
     `Bad determinant`, `det( V )`, `det( U )`, `New determinant`). The 3D search rotates about
     the tertiary axis (`vnl_cross_3d(primary, secondary)`) and the secondary one.
   - `-b n` (blob landmarks): `MultiScaleLaplacianBlobDetectorImageFilter`, patch correlations
     and `LandmarkBasedTransformInitializer` (see "Blobs" below).
3. **With 0 iterations** (the default) the initial transform is written as the output class:
   `Euler2DTransform`/`Euler3DTransform` for `Rigid` and the two alignments, `AffineTransform`,
   `Similarity2DTransform`/`Similarity3DTransform`, each given the initial centre, then
   `SetMatrix` (ITK's orthogonality check at 1e-10; `Similarity3D` takes the cube root of the
   determinant, `Versor::Set(matrix)`), then `SetOffset`; with compression requested (it only
   matters for HDF5).
4. **The search** (`antsAI.cxx:1125-1577`): the masks are read; the metric is built (`Mattes`:
   `MattesMutualInformationImageToImageMetricv4`; `GC`: `CorrelationImageToImageMetricv4`;
   `MI`: `JointHistogramMutualInformationImageToImageMetricv4` with a joint-PDF smoothing
   variance of 1), with the fixed image as virtual domain and the default moving-image
   gradient filter; the fixed image is sampled (below); `Initialize()`;
   `RegistrationParameterScalesFromPhysicalShift::EstimateScales` once (only its warnings
   matter); the start points (below); `MultiStartOptimizerv4` with
   `ConjugateGradientLineSearchOptimizerv4` (line search over `[0, 2]`, ε 0.1, at most 10
   steps, learning rate and maximum step the transform's gradient step, the scales estimator,
   the learning rate estimated once per start point); the best start point is written,
   centred at the initial centre.

### Sampling

- **Regular**: every `ceil(1/p)`-th voxel of the fixed image's region in memory order,
  starting with the first; **Random**: `static_cast<unsigned long>(float(N) · float(p))` voxels
  from `ImageRandomConstIteratorWithIndex`. `ceil(1/p)` of `p = 0` is infinite and its
  conversion to `unsigned long` undefined (ANTS-06: x86-64 gives 2⁶³, one sample point).
- Each point is a `Point<float, D>` (the metric's `FixedSampledPointSetType`):
  `TransformIndexToPhysicalPoint` into float coordinates (the origin, then each index term
  added in double and stored in float).
- Each point is moved by a normal variate times a third of the spacing: Regular computes
  `float(normal) · float(spacing / 3)` (a float product), Random `float(normal · spacing / 3)`
  (a double product, then rounded). The variates come from a `MersenneTwisterRandomVariateGenerator`
  seeded with `--random-seed`, else `ANTS_RANDOM_SEED` (`std::stoi`: `12abc` is 12; `abc`
  aborts), else 1234; a seed of 0 leaves it seeded **from the clock**. Random's voxels come
  from the iterator's own generator, which `antsAI` never seeds: also the clock (ANTS-58).
  larmorx seeds the iterator with the seed plus one and uses 121212 for a seed of 0.
- Points outside the fixed mask are dropped; then `MapFixedSampledPointSetToVirtual` drops those
  whose nearest index lies outside the virtual domain. No point generated: "Fixed Sample point
  set is empty." (`itkImageToImageMetricv4.hxx:595`); none left: "The virtual sampled point set
  has zero points..." (`:628`). Both abort `antsAI`.
- **The metric stores interpolated values in the image's pixel type**: `float(linear
  interpolation)` for both images, before the Parzen windows or the sums.

### The moving image's gradient

`GradientRecursiveGaussianImageFilter` (σ the largest spacing, normalised across scale) of the
float image computes each derivative image in float (`NumericTraits<double>::FloatType` is
`float`) and stores each component through `NthElementImageAdaptor<TOutputImage, float>`:
**the quotient by the spacing is rounded to float** before it lands in the `double` component,
then the direction cosines are applied in double. With power-of-two spacings this is invisible;
on a 1.5 × 1.2 × 2 mm grid every component differs from the unrounded quotient
(`GradientRecursiveGaussianImageFilter.hxx:263-273`, `.h:98`; verified against ITK on the
oblique phantom, 2026-10-17).

### The start points

`for (angle = −π·f; angle <= π·f + 1e-6; angle += step)` about each axis (2D: one angle; 3D:
three nested, about the tertiary axis, the secondary axis, then the tertiary axis again), and
translations `for (t = −g_k; t <= g_k + 1e-6; t += s)` along each axis of `-g [s,g₀xg₁xg₂]`.
Each start point is built from the initial transform:

- **Affine**: `SetIdentity` (which keeps the centre), centre, matrix and offset of the initial
  transform, `Translate(t)`, `Rotate2D(angle, pre)` or three `Rotate3D(axis, angle, pre)`,
  `Scale(1)`.
- **Rigid** and **Similarity**: the affine start point's matrix through the rigid or similarity
  class's `SetMatrix`; in 3D the rigid start point is translated (`Translate(t)` on the
  rigid transform's offset), **in 2D neither class is, and in 3D the similarity is not: every
  translation gives the same start point** (ANTS-52).
- `trialCounter` counts the loops' iterations, so for the two alignments, which list no start
  point, `-v` still prints "Starting optimizer with 6859 starting points" (ANTS-53).

A step that does not advance the angle or the translation (0, negative, NaN for the
translation) never ends the loop; ANTs runs forever. A negative arc fraction or grid extent
lists no start point, and ANTs crashes reading the best of an empty list.

### The multi-start optimizer

`MultiStartOptimizerv4::ResumeOptimization` (`itkMultiStartOptimizerv4.hxx:160-225`): per start
point, the metric's parameters are set, the local optimizer runs, its result replaces the start
point, and the metric is evaluated once more (`GetValue`). An ITK exception in a start point is
caught and printed as a warning (`line 187`); **the metric value then stays the previous start
point's (0 before the first) and still competes for the minimum** (ITK-19), so a first start
point that throws stays the best unless another value is below 0. `-v`'s observer prints
`k/N,value` every `N/10`-th start point and, at the end, `metricValues[bestIndex]`: when start
points threw, that index can point at another start point's value or past the end of the list
(every one threw: ANTs crashes).

**ITK-20.** The Mattes metric's threader allocates its joint histogram as an image whose spacing
is the two bin sizes, `(max − min)/(bins − 4)`. When the analysis region has one intensity (a
single sample point, a constant image) or none (an empty mask: the range starts at
`[FLT_MAX, −FLT_MAX]`), `ImageBase::SetSpacing` refuses the zero or negative spacing (ANTs
builds ITK with `ITK_LEGACY_REMOVE`, so a negative spacing throws too); the image stays without a
buffer, and the next evaluation of the same metric crashes in `FillBuffer`: in `antsAI`, the
start point after the first.

### Messages and failures

- Most errors are explained only with `-v` (ANTS-45); larmorx always explains them.
- An exception `antsAI` does not catch reaches `std::terminate`; libstdc++ prints
  `terminate called after throwing an instance of 'itk::ExceptionObject'` and the exception's
  `what()` (its file and line, then `ITK ERROR: Class(address): description`), and the program
  aborts. These include a parser conversion (`-c 1o`), a missing image (ANTs goes on with a null
  image; the first use throws, at `itkImageMomentsCalculator.hxx:225` or
  `itkCenteredTransformInitializer.hxx:31/35`), an image whose total mass is 0 (`:123`), the
  sample-set exceptions above and the transform writer's (an unknown extension,
  `itkTransformFileWriterSpecializations.cxx:153`; a file that cannot be opened,
  `itkTransformIOBase.cxx:88`, "Failed opening file" with no blank before the name).
- ITK's warnings (the multi-start's, the scales estimator's) are printed when they happen, so
  they come before a later abort's message on standard error.
- `--convergence`'s window defaults to 5 in the code; its help says 10 (ANTS-59).
- The description in the menu is "Program to calculate the optimallinear transform parameters
  for aligning two images." (two string literals joined without a blank).

### Blobs

`-b n` (at least 3 blobs) detects up to `n` blobs per image with
`MultiScaleLaplacianBlobDetectorImageFilter` (`Utilities/`, scales from 1 to 1024 in 10 steps
per octave), keeps the pairs whose patch correlation is highest, filters them by preserved
distances, and fits an affine (or a rigid, `LandmarkBasedTransformInitializer`) transform to the
pairs. The detector keeps one heap per thread of `GetGlobalDefaultNumberOfThreads()` entries but
is run on more work units than that with more than one thread (an index past the end,
ANTS-60). larmorx does not implement `-b` yet: it refuses it with a message.

What a replica needs, read on 2026-10-17 (for the task that ports it):

- the detector: `LaplacianRecursiveGaussianImageFilter<float, float>` (normalised across scale)
  at `ceil(log(32 k)/log k) + 1` scales, `k = 2^(1/10)`, from the largest σ down; the local
  maxima of each scale against its 26 neighbours in it and the two adjacent scales
  (`ImageBoundaryFacesCalculator` faces, zero-flux Neumann outside), kept in a min-heap of
  `n` blobs with libstdc++'s `push_heap`/`pop_heap`/`sort_heap` and merged with `std::merge`
  (the order of equal values depends on them);
- the patch correlations: `GradientRecursiveGaussianImageFilter<float, CovariantVector<float>>`
  (σ 1 mm, not normalised), spherical patches of radius 20 voxels weighted by
  `expf(−d/400)`, sums in float over about 33,500 points, `vnl_symmetric_eigensystem<float>`
  (computed in double), `vnl_svd<float>` (single-precision LINPACK), the linear interpolator
  with float coordinates;
- the landmark fit (`LandmarkBasedTransformInitializer`): for `Affine` a least-squares system
  solved with `vnl_qr` (LINPACK `dqrdc`/`dqrsl`); for `Rigid`/`Similarity` in 3D
  `VersorRigid3DTransform` from Horn's quaternion, the eigenvectors of a 4 × 4 matrix from
  **Eigen's `SelfAdjointEigenSolver`** (`SymmetricEigenAnalysisFixedDimension`, vectorised by
  Eigen); in 2D `Rigid2DTransform`. The initial transform then has its centre at 0 and the
  search rotates about the image axes.

## antsAffineInitializer

`antsAffineInitializerImp<D>` (`antsAffineInitializer.cxx:200-565`), positional arguments only:
`ImageDimension Image1 Image2 Output [SearchFactor [RadianFraction [UsePrincipalAxes
[LocalSearch [Image1Mask]]]]]`.

- Fewer than two arguments print the usage on standard error and **exit 0**. With two or three,
  `std::string(argv[k])` of the null pointer after the last argument throws (`basic_string:
  construction from null is not valid`) and the program aborts (ANTS-57).
- The dimension is `std::stoi`; 2, 3 and 4 run as such, **any other number as 2D** (ANTS-56).
  The search factor and the radian fraction are `atof` (text reads as 0), the fraction clamped
  to `[0.01, 1]`; the principal-axes flag is `std::stoi` (any non-zero value is true). **The
  local search's iterations are never read** ("NOT USED"): every start point runs 20 (ANTS-54),
  though the usage says the argument sets them.
- `degtorad` is `0.0174532925`, not `π/180`.
- The moments: `ImageMomentsCalculator` of both images, the centres **in double** (unlike
  antsAI). A missing image, or one whose values sum to 0, prints ` zero image1 error ` (or `2`,
  without a newline) and then aborts on `calculator2->GetTotalMass()`
  (`itkImageMomentsCalculator.hxx:186`).
- Wahba's problem as in antsAI but with `vnl_svd_fixed` and fixed-size products; the primary
  axis is row 2 in 3D, row 1 otherwise, the secondary row 1 (so in 2D and 4D both are row 1),
  and `B` sums the secondary pair first. A negative determinant prints ` bad det d v dV u dU`
  and then the same with ` new d'` on standard error, whatever the flag.
- The initial transform: `SetIdentity`, `SetOffset(moving centre − fixed centre)`, the rotation
  if asked, `SetCenter(fixed centre)`; **written first** (with compression requested). In 4D the
  program stops there.
- The metric: Mattes, 32 bins, on **every 10th voxel** of the fixed image (no perturbation, no
  mask applied to the points; the fixed mask applies to the metric), the moving-image gradient
  filter; the scales from `RegistrationParameterScalesFromPhysicalShift`, **estimated once, for
  the search transform as it is then: centred at 0** with the initial transform's parameters.
- The start points: `for (a = −π·f; a <= π·f + step; a += step)` (the upper bound is one whole
  step beyond, not 1e-6), 2D one angle (`Rotate2D`), 3D two (`Rotate3D` about the tertiary axis,
  then the secondary one), each from `SetIdentity`, the centre, the offset and the rotation if
  asked, then `Scale(1)`.
- The optimizers get the scales with `SetScales` and **no scales estimator**: neither the scales
  nor the learning rate is estimated, so the learning rate the line search leaves in one start
  point is where the next one starts (ITK-21).
- The best start point is written over the initial transform, centred at the fixed centre of
  mass. Nothing is printed on success but the determinant lines.

## Not found in fMRIPrep's runs

`antsAffineInitializer` runs inside niworkflows' `RobustMNINormalization` only when it gets no
initial transform (`niworkflows/interfaces/norm.py:174-190`, nipype's defaults: `15 0.1 0 10`);
fMRIPrep 25.2.6 gives it one, so the oracle runs have no `antsAffineInitializer` node. The
parity suite runs niworkflows' call on the MNI template and fMRIPrep's preprocessed T1w instead.
