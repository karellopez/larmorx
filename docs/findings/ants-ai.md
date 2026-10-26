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
- **`-b` (blob alignment) is ported** (2026-10-23): the detector, the patch correlations, the
  pairing and the landmark fits reproduce ANTs at every stage, bit for bit (blobs, the
  correspondence matrix, the pairs, the landmarks, the transform; checked with an instrumented
  copy of `antsAI`'s code on phantoms and real T1w images, and end to end in the record). ANTs'
  `-b` crashes at any thread count above one (ANTS-60) and its distance test keeps the wrong pairs
  whenever the images are rotated (ANTS-110); six defects are corrected by default (ANTS-110,
  ANTS-111, ANTS-112, ANTS-114, ANTS-116 since 2026-10-23, ITK-60); see "Blobs".

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
oblique phantom, 2026-10-17). `antsRegistration`'s metrics run the same filter when their
gradient-filter flag is set; A4 part 2's port divided in double without the rounding, which its
2 mm phantoms could not show: corrected on 2026-10-17, with two oblique cases
(`metrics/*-gradient-filter-oblique` in the ants-registration record) and a changed golden
checksum (`cli.ants.antsRegistration.metrics`, its composite-affine stage).

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

`-b n` or `-b [n,m]` (at least 3 blobs) replaces the centres of mass and principal axes by a
transform fitted to matched blobs (`GetTransformFromFeatureMatching`, `antsAI.cxx:156-604`). It
is parsed only for the transforms that are not the two alignments, and `-p` is then ignored. Ported
in full (2026-10-23): [`larmorx_image::blobs`](../../crates/larmorx-image/src/blobs.rs) (the
detector), [`larmorx_ants::ai_blobs`](../../crates/larmorx-ants/src/ai_blobs.rs) (the patches, the
pairing, the landmarks), [`larmorx_registration::landmark`](../../crates/larmorx-registration/src/landmark.rs)
(ITK's `LandmarkBasedTransformInitializer`), [`larmorx_core::vnl_qr`](../../crates/larmorx-core/src/vnl_qr.rs)
and [`larmorx_core::eigen_sym`](../../crates/larmorx-core/src/eigen_sym.rs). What each step computes:

1. **The detector** (`Utilities/itkMultiScaleLaplacianBlobDetectorImageFilter.hxx`, a public-domain
   work of the National Library of Medicine), `t` from `pow(1, 1)` to `pow(2, 10)` (stored in
   `float`), 10 steps per octave: `k = 2^0.1`, `σ₀ = 1/k`, `⌈ln(32 k)/ln k⌉ + 1 = 53` scales
   `σ_i = σ₀ k^(52 − i)` from the largest (34.3 mm) down (`ln(32k)/ln k` is 51.00000000000002 with
   glibc's and CORE-MATH's `log`, so the 53 does not depend on the C library). Each scale is
   `LaplacianRecursiveGaussianImageFilter<float, float>` normalised across scale (an axis of fewer
   than 4 voxels throws: "The number of pixels along direction 2 is less than 4", and `antsAI`
   aborts). The maxima of the middle of three consecutive scales against the 81 values of their
   `3^D` neighbourhoods in the three (a test `center < neighbour`: ties are maxima, NaN centres
   always pass), the faces of `ImageBoundaryFacesCalculator` visited interior first, each in
   raster order, the boundary faces reading the nearest voxel inside. A blob keeps the middle
   scale's σ; its radius is `√(D/2) σ · √2`.
2. **The heaps**: per work unit a min-heap of at most `n` blobs with libstdc++'s `push_heap`,
   `pop_heap` and `sort_heap` (with `a.value > b.value`, ties as the heap leaves them), merged with
   `std::merge` into the list kept (ties: the earlier scale first), cut to `n`.
3. **The patches** (`GetBlobCorrespondenceMatrix`, `PatchCorrelation`): every fixed and moving blob
   whose centre voxel is above `1e-4`; a sphere of radius 20 voxels (33,401 offsets in 3D, 1,257 in
   2D), weights `expf(−d/400)` normalised by their float sum (from 1 to 0.95: almost uniform; a
   Gaussian `exp(−d²/r²)` may have been meant, recorded here only). A patch that leaves either
   image (a centre within 20 voxels of an edge) has correlation 0, so a blob needs 41 voxels of
   image along every axis around it. Each image's gradient (`GradientRecursiveGaussianImageFilter`,
   σ 1, not normalised, `CovariantVector<float>`, the direction applied in double and stored in
   float) times the weights gives `GᵀG` in float; `vnl_symmetric_eigensystem<float>` (EISPACK in
   double, stored in float) gives each patch's axes (the largest two in 3D, the largest in 2D);
   Wahba's `A = V·Uᵀ` of `vnl_svd<float>` of `Σ outer(moving axis, fixed axis)` rotates the moving
   patch's points about their centroid (the points in double, the rotation in float); the rotated
   points, rounded to `Point<float>` (the interpolator's point type), are interpolated linearly,
   and the correlation of the centred samples is computed in float (NaN and infinity are 0,
   negative values become 0).
4. **The pairs**: repeatedly `arg_max` of the matrix (the first largest in row order), accepted
   when the moving blob's radius is above 1, the radii are within 0.25 (`float(|r_m − r_f|) <
   0.25f`) and both centres above `1e-4`; the row and column are then zeroed; until `m` pairs or as
   many steps as fixed blobs. Once no positive entry is left, `arg_max` returns entry 0 and the loop
   pairs fixed blob 0 with moving blob 0 again at every step (ANTS-111).
5. **The landmarks**: the pairs with at least 3 others whose ratio, stored in float, is within 0.1
   of 1. ANTs' ratio is `|f_j − m_j|² / |f_i − m_i|²`, the squared lengths of two pairs'
   displacements, not the ratio of distances between the blobs that its comment and ImageMath's
   `MatchBlobs` describe (ANTS-110); its symmetric assignment `r(i, j) = r(j, i) = …` inside the
   double loop leaves `r(i, j) = D_min(i,j)/D_max(i,j)` by index. A pair with no displacement (an
   image against itself) gives 0/0.
6. **The fit** (`LandmarkBasedTransformInitializer`): `AffineTransform` for `Affine` (Kim's normal
   equations, the identity weights divided by their Frobenius norm √n through vnl's matrix
   products, `vnl_qr<double>(Q).solve(C)`: LINPACK's `dqrdc` without pivoting and `dqrsl` with
   job 100; fewer than `D + 1` landmarks throw "insufficient number of landmarks, expected 4 got
   0", uncaught: `antsAI` aborts); otherwise `VersorRigid3DTransform` (Horn's 4 × 4 matrix and
   Eigen's `SelfAdjointEigenSolver`; with fewer than 3 landmarks no rotation) or `Rigid2DTransform`
   (`atan2` of the summed cross and dot products; `−π/2` when the dot product is within 5e-5 of 0,
   ITK-60; fewer than 2 landmarks: ITK's warning "Less than 2 landmarks available. Rotation is not
   computed"). `Similarity` takes the rigid fit too. No landmark at all: the centroids are 0/0 and
   the rigid transform is NaN, which `antsAI` writes (ANTS-114). The fitted matrix and offset become
   the initial transform, centred at 0, and the search (with iterations) rotates about the image
   axes.
7. If either blob list is empty, `antsAI` prints "The moving image or fixed image blobs list is
   empty." and dereferences a null transform (it crashes; ANTS-115). A missing image reaches the
   detector as a null input: "Input Primary is required but not set." (`itkProcessObject.cxx:1339`),
   and `antsAI` aborts.

**Eigen's solver works on a column-major copy.** `SelfAdjointEigenSolver<Matrix<double, 4, 4,
RowMajor>>` keeps its eigenvectors as `Matrix<double, 4, 4, ColMajor>` and tridiagonalises that
copy, so the self-adjoint product takes the column-major single-column loop (each column from its
diagonal down); the row-major loop gives other last bits (found when 8,418 of 20,003 random
matrices differed in an early port). ANTs' build vectorises Eigen for SSE4.2, but for a 4 × 4
matrix every reduction is over at most 3 entries and starts with one packet of the first two
terms, which is the sequential order, and the rotations are element-wise: the port computes
Eigen's bits with scalar code, checked on 650,003 matrices (random of every scale, Horn matrices
of random landmark sets, repeated eigenvalues, tiny subdiagonals, singular) against the solver
compiled with ANTs' flags. The landmark fits match ITK's build on 50,000 random landmark sets
(the 2D rigid fit once glibc's `atan2`, `sin` and `cos` are replaced by CORE-MATH's).

**Threads (ANTS-60, class (a)).** The detector sizes its vector of heaps by
`GetGlobalDefaultNumberOfThreads()` but runs on the multi-threader's work units, four times as
many when there is more than one thread. Verified 2026-10-23: `antsAI -b` **crashes (signal 11)
at 2, 4 and 6 threads, in every one of three runs**, on fMRIPrep's T1w of ds000005 and on a
phantom; at one thread its three runs are identical. In practice ANTs' blob alignment runs only
with `ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS=1`. larmorx computes the one-work-unit result on any
thread count (the Laplacians and the maxima test in parallel, the heaps fed in ANTs' order).

**The defects and what larmorx does** (each in the [register](upstream-defects.md); corrected by
default, `upstream_defects="reproduce"` gives ANTs' result bit for bit):

- **ANTS-112** (the detector's two thresholds): after each scale the smallest kept value becomes
  the threshold of the next scales even while fewer than `n` blobs are kept, and when a work
  unit's heap has just filled the next candidate is compared with that threshold instead of the
  heap's smallest value. Corrected, the `n` strongest maxima are kept. Both lists are computed
  from the same candidates, and the notice fires where they differ (it did not on the phantoms
  and real images of the record, whose largest scales already fill the list).
- **ANTS-111** (pairs after the correlations run out): corrected, the pairing stops when no
  positive correlation is left.
- **ANTS-110** (the distance test): corrected, the ratio is `|m_i − m_j|² / |f_i − f_j|²`, the
  distances between the two pairs' blobs in the moving and the fixed image (as `MatchBlobs`
  computes it, `ImageMath_Templates.hxx:14340-14366`). For a pure translation ANTs' test happens to
  be stricter (every true pair has the same displacement), and the corrected one admitted a pair
  matched to a neighbouring blob a few voxels away, whose distances to far blobs are nearly
  preserved, as long as the patches were compared as ANTs orients them; with ANTS-116 corrected
  too (2026-10-23) the identical patches correlate at 1 and no such pair was matched on the
  translations of the record. For any rotation ANTs' test keeps no pair or the wrong ones.
- **ANTS-114** (no landmark): corrected, a rigid fit without a landmark is an error.
- **ITK-60** (the 2D rigid fit near a quarter turn): corrected, `atan2` there too.
- **ANTS-116** (questionable; decided 2026-10-23, rule A): the patch rotation `A = V·Uᵀ` is not
  checked for a determinant of −1 (the main Wahba code of `antsAI` is), and the eigenvectors'
  signs are arbitrary, so identical patches can be compared reflected or turned (with the two
  largest axes, `B` has rank 2 and the singular vector of its zero singular value an arbitrary
  sign too). Corrected by default: every sign combination of the moving axes (4 in 3D, 2 in 2D),
  each with Kabsch's proper rotation, the largest correlation kept; ANTs' rotation is one of the
  candidates when it is proper (its correlation reused bit for bit), a reflection never is. Named
  where the pairs, the landmarks or the transform change; `reproduce` keeps ANTs' method.
  Measured ([investigation](../../validation/investigations/ants-116-blob-orientation/README.md)):
  of the identical blob pairs whose patches fit in both images, ANTs correlates 4 of 13 at 1 on
  the T1w and its shifted copy (5 of 19 and 14 of 28 on the 3D and 2D phantoms), larmorx all of
  them; with it the blob alignment finds every translation exactly (0.33 to 1.19 mm off before),
  the 3D rotated phantom 0.50 mm off (ANTs 34 mm; before, no landmark), the 2D rotated phantom
  0.85 mm off (ANTs 6.2 mm; before 1.62 mm), the rotated T1w 1.08 mm off (ANTs no landmark).
  The cost: up to four patch comparisons per blob pair instead of one.

**What the blob alignment achieves** (the [record](../validation/ants-ai.md), its table "Blob
alignment against the known transforms", 2026-10-24, with ANTS-116 corrected; mean and largest
distance from the true transform; in brackets larmorx's default before ANTS-116's correction,
the record of 2026-10-23):

| Moving image | ANTs (one thread) | larmorx's default |
|---|---|---|
| 3D phantom, an exact translation (rigid fit) | 0 | 0 (0.50, 1.4 mm) |
| 3D phantom rotated by 7° and 4° (rigid; affine) | 34, 67 mm; 654, 1520 mm | 0.50, 0.65 mm; 1.4, 2.9 mm (no landmark: an error) |
| 3D phantom rotated and scaled (rigid) | NaN (ANTS-114) | 2.4, 5.5 mm (no landmark: an error) |
| 2D phantom, a translation (rigid) | 0 | 0 to rounding (0.33, 0.42 mm) |
| 2D phantom rotated by 9° (rigid; affine) | 6.2, 12 mm; 45, 125 mm | 0.85, 1.2 mm; 1.4, 4.4 mm (1.6, 3.5 mm; 2.7, 11 mm) |
| fMRIPrep's T1w of ds000005, origin moved (rigid) | 0 (to rounding) | 0 to rounding (1.2, 2.1 mm) |
| the same T1w rotated by 8° in millimetres and shifted (rigid) | NaN (no landmark) | 1.1, 1.9 mm (1.0, 1.9 mm) |

For a translation ANTs' displacement-length test keeps exactly the true pairs; before ANTS-116's
correction the corrected distance test also kept a pair matched to a neighbouring blob (on the
T1w a blob 3 mm from its true partner), which cost about a millimetre; with the patches compared
in the orientation that matches best, every identical pair correlates at 1 and the translations
are found exactly. For any rotation ANTs keeps no landmark or the wrong ones. Between a T1w and the MNI template (2 mm) neither keeps a landmark with 30 blobs, on
any of the five oracle runs: the patch matching rarely pairs the same anatomy across subjects.
`-b` is a rough initialiser for images of one subject; fMRIPrep does not use it.

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
  though the usage says the argument sets them. larmorx honours the argument by default (the
  user's decision of 2026-10-17; `if (localSearchIterations > 0)` is the code's own path for
  0), with a notice wherever it is not 20; `reproduce` runs 20.
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
  point is where the next one starts (ITK-21). larmorx starts each one at 0.1 by default (the
  user's decision of 2026-10-17) and runs the search ITK's way too, to name ITK-21 where that
  changes the result; `reproduce` carries the rate over. On niworkflows' arguments with the MNI
  template and fMRIPrep's T1w (10 iterations), the same start point (14 of 16) is chosen either
  way, and the carry-over alone moves its parameters by up to 1.6 (with 20 iterations, 1.9).
- The best start point is written over the initial transform, centred at the fixed centre of
  mass. Nothing is printed on success but the determinant lines.

## Inside fMRIPrep's template registration, not a node of its own

`antsAffineInitializer` runs inside niworkflows' `RobustMNINormalization` when it gets no
initial transform (`niworkflows/interfaces/norm.py:174-190`, nipype's defaults: `15 0.1 0 10`,
the node's thread count). fMRIPrep 25.2.6 gives it none (`initial_moving_transform :
<undefined>` in every run's `register_template_wf/.../registration/_report/report.rst`), so it
runs once in each run's template registration, as part of that node: the oracle runs have no
`antsAffineInitializer` node and keep neither its inputs (niworkflows' masked images) nor its
transform. *Corrected 2026-10-26 (data panel, part a): an earlier version of this paragraph
said fMRIPrep gives the registration an initial transform.* The panel tier remakes the call
exactly in each of the fourteen runs (`larmorx_validation.parity.niworkflows_inputs`: the masked
images from the node's recorded inputs, then the initializer), and the standard tier runs
niworkflows' call on the MNI template and fMRIPrep's preprocessed T1w.

## Memory and speed against ANTs (2026-10-26, the data panel's re-validation)

On `tests/parity/test_memory_peak.py`'s head of 128 × 128 × 96 voxels at one thread
(`ants-registration.md`, "Memory against ANTs", has the table):

- **antsAI** (`Mattes`, `Regular` 25 %, `Rigid`, 30° steps): ANTs 92.1 MiB, larmorx 157.2 MiB
  before this part, 127.3 MiB now; **antsAffineInitializer** (niworkflows' `15 0.1 0 10`): ANTs
  90.4 MiB, larmorx 139.7 then 110 MiB. Fixed here: the moving image's gradient is written into
  its vectors axis by axis (it was held twice, as components and as vectors), the float copies
  are dropped once read (the fixed one after the initial transform, the moving one after its
  gradient).
- **Still above ANTs, a follow-up:** the metric holds the two images in double (`Image<f64>`,
  16 bytes a voxel for both) where ANTs holds them in float (8), and larmorx's interpreter and
  its allocator's retained pages add about as much again on this size; the gap grows with the
  image (about 8 bytes a voxel: some 170 MiB on the 7 T MP2RAGE). Closing it means storing the
  metric's images in their pixel type (`pixel_f32` already rounds every value to float). The
  two cases are marked `xfail(strict=True)` in the test until then.
- **Speed at one thread:** antsAffineInitializer on that head takes ANTs 73 s and larmorx 277 s
  (312 s with the build before the thread-pool fix and rayon's global pool held to one thread:
  the port's own time, not the fix's). Before the fix, larmorx's metric loop ran on rayon's
  global pool, every CPU, at any thread count (`ants-cli.md`, "larmorx's own threads"), which hid
  this. Part of it is the D13 notice of ITK-21: larmorx runs the start points' local searches
  both ways, to say where ITK's carried-over learning rate changes the result. A follow-up.
