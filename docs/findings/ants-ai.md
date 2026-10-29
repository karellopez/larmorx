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
- **Memory and speed (2026-10-28):** the images kept in float as in ANTs (the peak at or below
  ANTs'), the Mattes metric's sampled points computed in batches with each fixed side once:
  antsAI 1.8 to 2 times as fast as ANTs at one thread, antsAffineInitializer at ANTs' time in
  `reproduce` (its ITK-21 notice runs a second search) and half of it by default; bit for bit
  as before ("Memory and speed (2026-10-28)").
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
- **Still above ANTs, a follow-up** (done 2026-10-28, the next section): the metric holds the two images in double (`Image<f64>`,
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
  both ways, to say where ITK's carried-over learning rate changes the result. A follow-up
  (done 2026-10-28, the next section).

## Memory and speed (2026-10-28): ANTs' memory, faster than ANTs

The follow-ups of the section above, closed without changing a bit of either program's results.
Measured on the main machine (AMD, 32 threads, shared with other jobs: informal timings), each
run its own process under `/usr/bin/time -v`: ANTs 2.6.5 with CORE-MATH preloaded at one thread,
larmorx's command (`python -m larmorx ants ...`, the interpreter included); the
[benchmark](../benchmarks/ants-ai.md) has every figure (medians of three) and
`validation/investigations/ants-ai-speed/` the tools.

**ANTs stays on one thread.** Neither program sets a thread count; ITK's global default comes
from `ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS=1` (`itkMultiThreaderBase.cxx:266-330`). A whole run
uses 99 % of one CPU with 3 voluntary context switches (the head, 35 s) or 59 (the 7 T, 252 s).

**Memory** (peak resident, one thread; the rule: at most 1.10 × ANTs + 8 MiB):

| Call | ANTs | larmorx before | larmorx now |
|---|---|---|---|
| antsAI, the memory test's head (128 × 128 × 96; `Mattes`, `Regular` 25 %, `Rigid`) | 92 to 96 MiB | 126 MiB | 95 MiB |
| antsAffineInitializer, the head (niworkflows' `15 0.1 0 10`) | 89 MiB | 109 MiB | 91 MiB |
| antsAffineInitializer, the 7 T MP2RAGE and the 1 mm MNI template | 688 MiB | 844 MiB (2026-10-26) | 676 MiB |
| antsAI, the same pair, the memory test's options | 754 MiB | | 717 MiB |

- **The images in float.** The metric's images are now kept as read, `itk::Image<float, D>`
  (`metric::MetricImage::Float`: 4 bytes a voxel where `Image<f64>` took 8), and nothing makes a
  double copy any more: the moments, the centred initializer and the search read the float
  images. The bits stay: each value is read as a double, exactly (`f64::from(f32)`), which is
  what the double copies held, and every operation after it is unchanged. ANTs holds the same
  two float images and the moving image's gradient (24 bytes a voxel). The two
  `xfail(strict=True)` marks of `tests/parity/test_memory_peak.py` are gone: both cases pass in
  both modes (antsAI 96 MiB against 91, antsAffineInitializer 90 against 90). At eight threads
  each thread adds a few MiB of buffers.

**Where the time went.** `perf` is refused to users on this machine
(`kernel.perf_event_paranoid` 4) and py-spy sees only Python threads, so `gdb_profile.py`
samples every thread's stack with gdb (the command runs in a child that allows any tracer). On
antsAffineInitializer and the head nearly all the time was the Mattes metric's loop over the
sampled points: about half in the linear interpolation (the moving value and the gradient's
three components each finding their cell again, ITK's index rounding computed in five
dependent operations), the rest in each point's `Vec`s (the Jacobian, the derivative terms), the
buffers of each chunk and the pool hop of every chunk. A search makes about 2,200 value-only
and 300 value-and-derivative evaluations of the 157,000 points (16 start points, 20 iterations,
golden-section line searches); ANTs takes 35 s for its one search.

| Step (antsAffineInitializer, the head, `reproduce`, one thread) | larmorx |
|---|---|
| before (ANTs: 35 s) | 133 s |
| the images in float (less to read) | 116 s |
| the sampled path: each point's fixed bin once, the points one at a time on the calling thread | 66 s |
| batches of 64 points stage by stage; ITK's rounding in two operations; the gradient's components in one pass; the derivative's row of cells added as a slice | 48 s |
| the interpolation cell inlined; the corners written in place (no `memcpy`); the Jacobian's array not cleared twice | 42 s |
| ITK-21's second search takes the first one's shared evaluations | 41 s |
| value-only evaluations added as they are computed (no buffer) | 40 s |

- **The sampled path** (`metric::mattes::Mattes::evaluate_sampled`, taken when the images are
  float-stored, the points sampled, no fixed transform, a transform of global support, 1 to 3
  dimensions: antsAI's and antsAffineInitializer's metric, nothing else). The fixed side of a
  sampled point never changes (the point, the fixed mask, the fixed value interpolated and
  stored in float, its Parzen bin): ITK computes it at every evaluation, larmorx once
  (`Mattes::fixed_bins`, 4 bytes a point). The moving side is computed as before, operation for
  operation, and each point is added to the joint histogram in the points' order, so every sum
  is the same sum. The points are taken in batches of 64, stage by stage (the cells; the eight
  corners read in a loop of reads only; the values and Parzen windows; the gradient's corners;
  the Jacobian and the products), so that the processor overlaps the points of a batch and has
  many reads in flight; at more threads, chunks of batches are computed in parallel and then
  added in order. `interp::Cell::eval_corners` performs `EvaluateOptimized`'s operations in its
  order, axis skips included; the gradient shares the value's cell when its image has the moving
  image's grid (region, origin and index matrix checked bit for bit).
- **ITK's index rounding** (`itk::Math::Floor`, `round_half_to_even(2x − 0.5) >> 1`;
  `larmorx_core::itk_real`): below 2^51 in magnitude, `x + 1.5·2^52` lies where the doubles are
  the integers and consecutive integers have consecutive bit patterns, so one addition rounds (to
  nearest, ties to even) and the bits give the integer: the general method's integer (tested on
  600,000 values, ties, signed zeros and the edges of the range), in two operations instead of
  five on the path to every voxel read. Every tool's floor goes through it; the golden checksums
  and the smoke tier are unchanged.
- **ITK-21's notice, cheaper** (`affine_initializer::search`). The two searches ask the metric
  the same questions up to where their learning rates part: the whole first start point (both
  begin at 0.1) and each later start point's first evaluation (at the start point, before any
  step). At one thread the second search takes those from the first one's results, kept by the
  parameters' bits (the metric is a function of the parameters); at more threads the two run at
  once in the call's pool. The notice is the same, case for case. The rest of the second search
  cannot be shared: from the second start point on ITK begins each local search at the learning
  rate the previous one left, the golden-section brackets scale with it, and no later
  evaluation repeats (stopping the second search where the two agree to the bit was considered:
  they never do once the rates part).

**Speed now** (medians of three; wall time, one thread unless said):

| Call | ANTs | larmorx `reproduce` | larmorx default | `reproduce`, 8 threads | default, 8 threads |
|---|---|---|---|---|---|
| antsAffineInitializer, the head | 33.5 s | 40.0 s | 20.9 s | 8.7 s | 4.8 s |
| antsAI, the head | 31.2 s | 17.5 s | 17.8 s | 5.0 s | 4.8 s |
| antsAffineInitializer, the 7 T | 252.2 s | 254.5 s | 124.7 s | 51.6 s | 26.1 s |
| antsAI, the 7 T | 155.8 s | 79.0 s | 79.9 s | 21.8 s | 20.9 s |

**The gap left: antsAffineInitializer in `reproduce` at one thread.** larmorx's search takes
about 0.6 of ANTs' time (a value-only evaluation about 43 ns a point, one with the derivative
about 116 ns; antsAI, which searches once, shows it: 17.5 s against 31.2 s), but the ITK-21
notice needs the second search, which can share only the first start point and each start
point's first evaluation (1/16 of it here): on the head about 20 s of search plus 19 s for the
check against ANTs' 33.5 s; on the 7 T, where both programs wait on memory and larmorx's
batched reads gain more, the two searches together take ANTs' time. In the default mode
antsAffineInitializer runs the iterations the call asks for (niworkflows: 10; ANTs always 20,
ANTS-54): half ANTs' time or less; at eight threads it is 4 to 10 times as fast as ANTs at one.
A further gain at one thread would have to come from SIMD over the points of a batch (each
point's operations unchanged); not attempted.

**Bit-identity, proven.** `compare_builds.py` runs every standard case of both suites with the
build before this part and the new one, in both D13 modes, the new one at 1 and 4 threads: 172
cases, exit status, both streams (every notice) and every file identical. The standard records
were re-made from the clean commit `9b9fe9aa5232` (the original's runs served by the oracle
cache; larmorx at 4 threads, as the suites run it): every status as before
(`compare_records.py`, `record_fields.py`: only times, ANTs' own clock-seeded spread and the
fields the 2026-10-17 record's format lacked differ). The golden checksums are unchanged
(`pytest tests/golden`) and the smoke tier passes (2,549 tests).

## On the data panel: fMRIPrep's transforms and ANTs' own spread (2026-10-26)

The panel record ([ants-ai-panel](../validation/ants-ai-panel.md)) replays all 36 antsAI nodes of
the fourteen finished oracle runs (sMRIPrep's brain extraction `init_aff` in every run, the BOLD
reference's in 22 runs of 12 subjects) with fMRIPrep's seed, and four panel images fMRIPrep never
gives it: **40 of 40 identical to ANTs 2.6.5 at one thread** (38 as built, 2 with CORE-MATH).
Against fMRIPrep's own transforms (ANTs 2.6.2 at the node's 6 or 8 threads), 29 calls are within
1.4e-12 of a parameter and two within 1e-8; five are further: up to 6.8 mm over a 100 mm cube.
`validation/investigations/panel-ants/ai_variability.py` runs each of these calls again with the
ANTs 2.6.5 oracle (once at one thread, three times at 8) and with fMRIPrep's own image (once at
one thread, twice at the node's count), every transform measured as the largest displacement of
a 100 mm cube's corners (`results/ai_variability.json`):

| Call | fMRIPrep's file vs one thread | fMRIPrep's image, one thread | ANTs at 8 threads (3 runs) | fMRIPrep's image at N threads (2 runs) |
|---|---|---|---|---|
| ds002422 `arithm` run 1, BOLD (N = 6) | 5.44 mm | 0 | 2 runs give fMRIPrep's file (5e-12), 1 the one-thread result | both give fMRIPrep's file |
| ds004884 `rest` run 9, BOLD (6) | 6.76 mm | 0 | 1 run 0.21 mm away, 2 the one-thread result | 1 run 0.21 mm, 1 the one-thread result |
| ds004958 brain extraction (6) | 0.61 mm | 0 | 1 run gives fMRIPrep's file, 2 the one-thread result | both the one-thread result |
| ds004958 `rest`, BOLD (6) | 1.2 mm | 0 | all 3 give fMRIPrep's file (3e-10) | 1 fMRIPrep's file, 1 the one-thread result |
| ds005752 `rest` reverse, BOLD (8) | 0.68 mm | 0 | all 3 give fMRIPrep's file (5e-14) | both give fMRIPrep's file |
| ds002422 `arithm` run 2, BOLD (6) | 1.6e-8 mm | 0 | 2e-11 to 8e-7 | both give fMRIPrep's file |
| ds005454 `rest`, BOLD (8) | 2.4e-9 mm | 0 | 4e-7 to 8e-7 | 3e-8 |

- **Cause: ANTs' own thread variability**, not a difference of builds or of larmorx: fMRIPrep's
  own ANTs at one thread gives exactly the oracle's (and larmorx's) one-thread transform in all
  seven, and both builds at fMRIPrep's thread count land, run to run, either on fMRIPrep's file,
  on the one-thread result or elsewhere. Above one thread the Mattes metric's per-thread sums move
  its values by about 1e-13 (`ants-ai` record's thread band); the multi-start search compares
  start points by those values, so a near tie can be broken the other way and the search ends in
  another start point's optimum, millimetres away. larmorx gives ANTs' one-thread result at any
  thread count (its sums are fixed-order), which is one of the transforms ANTs itself produces.
- **Verdict:** within the original's variability (no larmorx change, no defect entry: the
  nondeterminism is ANTs' by thread count; larmorx never reproduces nondeterminism, D13). The
  record gives every call's distance to fMRIPrep's file.
