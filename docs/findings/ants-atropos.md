# Atropos: ANTs' finite-mixture segmentation (ANTs v2.6.5, ITK v5.4.5)

What `Atropos` and the ITK classes behind it really compute, found while porting them
(milestone A6). Line numbers are at the pinned tags: `Atropos.cxx` is ANTs'
`Examples/Atropos.cxx`, `Atropos.hxx` is `ImageSegmentation/antsAtroposSegmentationImageFilter.hxx`,
and the list-sample functions and filters are their files in `ImageSegmentation/`
(`antsGaussianListSampleFunction.hxx`, `antsManifoldParzenWindowsListSampleFunction.hxx`,
`antsJointHistogramParzenListSampleFunction.hxx`, `antsJointHistogramParzenShapeAndOrientationListSampleFunction.hxx`,
`antsLogEuclideanGaussianListSampleFunction.hxx`, `antsPartialVolumeGaussianListSampleFunction.hxx`,
`antsBoxPlotQuantileListSampleFilter.hxx`, `antsGrubbsRosnerListSampleFilter.hxx`); ITK's are
under `Modules/` of ITK v5.4.5. The parity record is [ants-atropos](../validation/ants-atropos.md);
the API page, with the option mapping, [api/ants-atropos.md](../api/ants-atropos.md); how ANTs
reads and writes images is in [ants-image-programs.md](ants-image-programs.md), its argument
parser in [ants-cli.md](ants-cli.md). The defects have their entries in the
[register](upstream-defects.md) (ITK-50, ITK-51, ANTS-90 to ANTS-107).

## In short

- **sMRIPrep's Atropos call is reproduced bit for bit.** On the five fMRIPrep oracle runs,
  larmorx's classified image and its three posteriors are identical to ANTs' when ANTs runs
  with correctly rounded maths (CORE-MATH preloaded); against the unmodified binary the
  classified images are identical and the posteriors differ in the last bits of a few voxels
  where glibc's `expf`/`logf` are not correctly rounded (the numbers are in the
  [record](../validation/ants-atropos.md)).
- **ANTs' Atropos does not depend on its thread count** (the posteriors are computed voxel by
  voxel, the sums sequentially): one, two, four and eight threads give the same bits. larmorx's
  does not either; it runs the per-voxel work in parallel and keeps every sum in ANTs' order.
- **Atropos never reads `ANTS_RANDOM_SEED`.** Its generator is ITK's Mersenne twister, seeded
  from the clock unless `--use-random-seed 0` gives it the fixed seed 19650218
  (`Atropos.cxx:729-737`, `Atropos.hxx:107-131`). sMRIPrep passes `--use-random-seed 0`, so
  its runs are repeatable whatever fMRIPrep's seed. Without it, ANTs' runs differ (the ICM
  visiting order is a shuffle, `Atropos.hxx:1311-1325`); larmorx never reads the clock and uses
  the seed 121212 instead.
- **Twenty defects**, from crashes to results that depend on the heap's contents: ITK-50
  (k-means skips the split points), ITK-51 (Student quantiles NaN beyond 340 degrees of
  freedom, so Grubbs-Rosner never finds an outlier in an image), ANTS-90 to ANTS-107. Each is
  corrected by default and reproduced on request (D13), ANTS-106 included since 2026-10-23 (the
  user's decision of 2026-10-22: a partial-volume label is a member of its own set in the MRF).
  On sMRIPrep's call only ITK-50 is met: on the five runs the correction changes 1 to 25 of 0.8 to 2.5 million brain voxels' labels and the posteriors by
  at most 0.14, less than ANTs' own spread with the clock's seed.

## How sMRIPrep calls it

`smriprep/workflows/... brain_extraction_wf/atropos_wf/01_atropos` (niworkflows'
`init_atropos_wf`, through nipype's `Atropos` interface), read in fMRIPrep 25.2.6's working
directories (`command.txt`):

```text
Atropos --image-dimensionality 3 --initialization KMeans[3]
  --intensity-image <brain_extraction_wf>/inu_n4/mapflow/_inu_n40/<T1w>_noise_corrected_maths_corrected.nii.gz
  --likelihood-model Gaussian
  --mask-image <atropos_wf>/get_brainmask/tpl-OASIS30ANTs_res-01_label-brain_probseg_trans_resampled_maths_maths.nii.gz
  --mrf [0.1,1x1x1] --convergence [3,0]
  --output [<T1w>_..._labeled.nii.gz,POSTERIOR_%02d.nii.gz] --use-random-seed 0
```

fMRIPrep runs it with 8 threads (`num_threads : 8` in the node's report) and sets
`ANTS_RANDOM_SEED`, which Atropos ignores. The three classes (CSF, grey and white matter, in
increasing intensity) feed the brain mask's refinement (`atropos_wf` nodes 02 to 27). The
same options with three iterations, the posterior images written as `POSTERIOR_%02d`, are
what larmorx's real cases repeat on each oracle run.

## The algorithm, as the code has it

- **Initial labelling** (`Atropos.hxx:609-660`): `Random` draws a label for **every voxel,
  outside the mask too** (ANTS-92); `KMeans` runs ITK's `KdTreeBasedKmeansEstimator` on the
  first image (a weighted-centroid k-d tree with buckets of 16, at most 200 iterations,
  threshold 0, `:1037-1045`) from centres spread over the first image's range in the mask
  (or given), then, with several images, a second k-means on all of them whose result is
  thrown away (ANTS-90); `Otsu` takes `OtsuMultipleThresholdsCalculator` on a 200-bin histogram
  of the first image (`:941-947`); `PriorProbabilityImages` labels each voxel with its largest
  prior and `PriorLabelImage` copies the labels.
- **Parameters** (`:676-790`): each class's model is estimated from the voxels with its label,
  weighted by the priors when there are prior probability images. The proportions are the
  classes' shares of the voxels.
- **Iterations** (`:461-482`): each iteration updates the labelling once (`UpdateClassLabeling`)
  and stops when the mean of the voxels' largest posterior grows by less than the convergence
  threshold, or after the maximum number of iterations. The value printed with `-v` is the one
  before the update.
- **The update** (`:1281-1450`): with ICM (the default) the voxels are coloured so that no two
  neighbours share a code (the ICM code image), the codes visited in a random order (a
  Fisher-Yates shuffle with the filter's generator), and each voxel of a code takes the label of
  its largest local posterior, its neighbours' labels as they are at that moment; ties between
  labels are broken at random. Then every class's posterior image is computed, each voxel's
  largest posterior gives its new label (ties to the class with fewer voxels), and the models
  are re-estimated from the posteriors as weights (soft EM).
- **Posteriors** (`:1992-2470`, `CalculateLocalPosteriorProbability` at `:1809-1935`): with
  `w` the prior weight, `P` the spatial prior (the prior probability image times the mixture
  proportion over the voxel's sum of priors; without priors the proportion, or `1/K` without
  proportions), `L` the likelihood and `M = exp(−β E_c) / Σ_k exp(−β E_k)` the MRF prior, with
  `E_c = Σ δ(c, neighbour's label) / distance` over the neighbourhood (δ = −1 for the same
  label, 0 otherwise; with partial-volume classes −2, −1 for two labels of one set, +1
  otherwise, `:546-606`), each at least 1e-10: Socrates `P^w (L M)^(1−w)`; Plato the same, but 1 where the
  voxel's prior label is the class or its prior is 1; Aristotle `(P D)^w (L M)^(1−w)` with `D`
  the distance prior; Sigmoid `σ(w (P − 1/K)) L M`. Then, **only when the initial annealing
  temperature is not 1**, `p^(1/T)` with `T = max(T₀ rate^iteration, T_min)`: `-p
  Socrates[1,1,0.5]` anneals nothing. Class 1's posterior is computed in a first pass that
  also accumulates the sum the others are normalized by; non-finite posteriors become 0. The
  mixture proportion reaches the posterior through `P` only (the function's own proportion
  argument is unused).
- **Prior labels** spread through the mask by fast marching (or Euclidean distance, `-e 1`):
  `-l` turns a label's distance into a prior `exp(−λ d)` beyond the boundary probability.
- **Adaptive smoothing** (`-a [image,w]`, prior-based initializations only): each class's
  intensities are fitted with `BSplineScatteredDataPointSetToImageFilter` (levels, mesh and
  order from `-b`), weighted by the posteriors, and each voxel's intensity becomes
  `(1 − w) x + w · fit`.
- **Outlier handling** (`-w`): the samples of every image pass through a box-plot or
  Grubbs-Rosner winsorizing filter before each estimate.

## Seeds and randomness

`--use-random-seed 1` (the default) leaves the filter's generator seeded from the clock
(`MersenneTwisterRandomVariateGenerator::SetSeed()`, a hash of `time` and `clock`); every
draw (the random initial labels, the ICM shuffle, the random tie-breaks of the local update)
then differs from run to run. The [record](../validation/ants-atropos.md) measures it: two ANTs
runs of sMRIPrep's options with the clock's seed on ds000005's T1w disagree on 343 voxels'
labels (smallest Dice 0.99942) and on posteriors by up to 0.28; at 2, 4 and 8 threads, with
the fixed seed, ANTs gives its one-thread result bit for bit.
larmorx replaces the clock by a fixed seed (121212, as for antsRegistration and antsAI), so its
runs are repeatable; `--use-random-seed 0` gives ANTs' fixed seed 19650218 in both.

## glibc's maths

Atropos' only transcendental functions are `expf`, `logf`, `exp`, `log`, `pow` (the Gaussian
likelihoods, the MRF prior, the annealing, the label propagation's `exp`, Grubbs-Rosner's
Student quantiles), and `acos`/`atan` in the joint shape and orientation model. glibc's are
not correctly rounded for every argument; larmorx's are (CLAUDE.md rule 5). That is the only
cause of differences from the unmodified binary: with CORE-MATH's functions preloaded into it
(`oracles/core-math-preload`, now also with `pow` and `powf`), every compared output is
bit-identical.

## Verbose output

`-v 1` prints the iterations (`Iteration n (of N): posterior probability = p (annealing
temperature = T)`), then, after `Writing output:`, the filter's `Print`: its settings, the MRF
and ICM parameters, the outlier filter, and each class's model (`GaussianListSampleFunction`
with `mean = [...]` at 17 significant digits and `covariance = [...]` at 6). ITK's base-class
lines (RTTI, reference count, modification time, the multithreader's settings, the pipeline's
inputs and outputs, all with addresses) are left out: larmorx has no such objects. Addresses
print as `0x0`. Grubbs-Rosner's `Print` lists the outliers of its last call ("Outlier
Identifiers" or "There are no outliers."). SLATEC's messages ("SLATEC/DGAMMA: X SO BIG GAMMA
OVERFLOWS", from ITK-51) are printed on standard error as ITK prints them; a NaN passes
SLATEC's range test, as in C.

## Defects

Each is an entry of the [register](upstream-defects.md), with its evidence; the switch is
`upstream_defects="reproduce"` / `LARMORX_UPSTREAM_DEFECTS=reproduce`.

- **ITK-50** `KdTreeBasedKmeansEstimator` never counts the point a non-terminal k-d tree node is
  split at when the node is divided between candidate centres (`itkKdTreeBasedKmeansEstimator.hxx:209-227`
  recurses into both halves; the node's own point is in neither). The new centres miss those
  points; on sMRIPrep's call, corrected, 1 to 25 voxels change label on each of the five
  oracle runs (Dice at least 0.99999 for every class).
- **ITK-51** `TDistribution::PDF` divides `dgamma((n+1)/2)` by `dgamma(n/2)`
  (`itkTDistribution.cxx:84-85`); both overflow above about 340 degrees of freedom (SLATEC
  prints "X SO BIG GAMMA OVERFLOWS"), the density is NaN, and so is the `InverseCDF` after its
  Newton steps (`:215-219`). Grubbs-Rosner's critical value is NaN, the comparison false: it
  never finds an outlier in a sample of more than about 342 values, that is in any image.
  larmorx takes the ratio through `dlngam` when it would overflow.
- **ANTS-90** With several images, `KMeans`' second k-means starts from `stats->GetMean(j + 1)`
  (the mask's statistics of the first image: its mean for class 1, 0 for the others,
  `Atropos.hxx:1163`) and its classifier's labels are never read (`classifier`, not
  `classifier2`, at `:1260`): only the first image decides the initial labelling.
- **ANTS-91** `Otsu`'s range starts from `NumericTraits<float>::min()` (the smallest positive
  float) and uses `else if` (`:907-922`): an image whose voxels decrease never sets the maximum,
  a negative image's range ends at 0, and a first voxel that is the maximum is missed.
- **ANTS-92** `Random` labels voxels outside the mask too (`:622-629`), and those enter the
  first estimates.
- **ANTS-93** With several `-a` options, image `count` takes the weight of `GetFunction(count)`,
  counted from the last option (`Atropos.cxx:614-637`): the adaptive smoothing weights are
  applied in reverse order.
- **ANTS-94** The outlier filters are created once and their output lists never cleared: a
  `BoxPlotQuantileListSampleFilter` re-runs itself on its own output inside `GenerateData`
  (verified with ITK's code instrumented, `scratch/probe`), so its whiskers come from a
  histogram of the input winsorized at bounds computed from the previous call's values plus the
  kept ones; `GrubbsRosnerListSampleFilter` appends to its output, so the second image's samples
  are the first image's, and the sample grows at every call.
- **ANTS-95** `ManifoldParzenWindowsListSampleFunction::Evaluate` ignores the posterior weights
  (every class is the same Parzen density up to a constant).
- **ANTS-96** A partial-volume class is re-estimated only when `n == labelSet[d] − 1`
  (`Atropos.hxx:1413-1421`), which never holds for a partial-volume class: its Gaussian keeps
  the initial means.
- **ANTS-97** With `--minimize-memory-usage` the prior probability images are stored sparsely,
  their voxel numbers as `float` (exact only up to 2^24 voxels).
- **ANTS-98** `KMeans` takes the first image's range from the mask's label 1 only
  (`Atropos.hxx:1011-1012`): a mask without 1s (a mask of 255s) gives the range `FLT_MAX` to
  `−FLT_MAX`, infinite centres, one class for every voxel, and ITK's LabelMap error.
- **ANTS-99** Without `-d`, Atropos asks the image IO for its dimension before it has read the
  header (`Atropos.cxx:1710-1712`): NIfTI's answers 3, so 2D and 4D images are read as 3D.
- **ANTS-100** Prior probability images given as one vector image are extracted with one
  `VectorIndexSelectionCastImageFilter` whose output every class keeps (`Atropos.cxx:242-249`):
  all hold the last component, normalized in place by each class in turn.
- **ANTS-101** `-p` with another name than the four keeps Socrates, silently
  (`Atropos.cxx:344-360`).
- **ANTS-102** `--icm [1,2]` (the documented form) converts the option's empty name to a bool
  (`Atropos.cxx:711`): the parser throws and ANTs aborts. `-g 1[1,2]` works.
- **ANTS-103** `JointShapeAndOrientationProbability` evaluates its shape and orientation
  histograms at the raw tensor components, at a point whose second coordinate is never set
  (`antsJointHistogramParzenShapeAndOrientationListSampleFunction.hxx:581-608`). larmorx
  evaluates them at the tensor's normalized eigenvalues and principal direction, where they
  were filled; reproduced, the unset coordinate is 0.
- **ANTS-104** `ManifoldParzenWindows` with a covariance neighbourhood (`-k
  ManifoldParzenWindows[s,k,c>0,σ]`) doubles the running diagonal sums at each neighbour and
  adds the running upper values to the lower triangle.
- **ANTS-105** `--minimize-memory-usage` with prior probability images and a mask crashes:
  `Atropos.cxx:440` reads a prior through `GetPriorProbabilityImage`, which with minimized
  memory rebuilds it on the first intensity image before any is set.
- **ANTS-106** Partial-volume classes with their likelihoods (`--use-partial-volume-likelihoods
  1`) and an MRF hardly win a voxel: the MRF energy of a partial-volume label with any tissue
  neighbour, its own tissues included, is +1 (`ComputeMRFDeltaMatrix`, `Atropos.hxx:546-600`,
  compares the tissue labels of each set, never the partial-volume label itself), against −2
  for a tissue with its own kind, and no voxel starts with a partial-volume label. Atropos stops
  when a partial-volume class wins no voxel ("N non-zero label sets but N + P is requested"; the
  test is on the largest label). Without the MRF (`-m [0,...]`) they work. **Corrected by
  default** (decided by the user, 2026-10-22): a partial-volume label counts as a member of its
  own set, −1 with its two tissues, −2 with itself, +1 with any other label
  (`compute_mrf_neighborhood`); `reproduce` keeps ANTs' energies. Measured on a phantom with known
  fractions (nested spheres box-averaged from a five-times finer grid, noise sd 6; truth: a
  tissue where one share is at least 0.8, else the partial-volume class of the two largest):
  MRF 0.1, partial-volume Dice 0.295 and 0.405 in ANTs, 0.862 and 0.805 corrected (voxels as the
  truth: 94.0 % and 97.7 %); MRF 0.3, ANTs stops, corrected 0.835 and 0.801 (97.8 %); no MRF,
  ANTs 0.728 and 0.578 (93.7 %), larmorx's default 0.711 and 0.594 (93.4 %, ITK-50 and ANTS-96
  corrected). The MRF thus helps the partial-volume classes once they are members of their
  sets, as it helps the tissues.
- **ANTS-107** With Grubbs-Rosner the sample grows (ANTS-94); `UpdateClassLabeling` allocates
  the weights for the grown sample (`WeightArrayType weights(totalSampleSize)`, uninitialized)
  and sets those of the masked voxels only (`Atropos.hxx:1361-1372`): the rest is whatever the
  heap held. From the second iteration on, ANTs' class parameters depend on the memory's
  history; the same command line gives different posteriors when only the length of the output
  paths changes (verified: 18,700 of 35,551 posteriors change between output directories of
  139 and 145 characters). larmorx weighs those samples 0 (what a fresh allocation holds) and
  names ANTS-107; corrected, the sample does not grow.

Crashes and aborts besides: an unknown output extension (ITK's writer throws outside the
`try`), a value the parser cannot convert (`-i KMeans[3x]`, `-l 2`): ANTs aborts with the
exception's text; larmorx prints the same text, adds one line and exits 1.

## Other behaviour worth knowing

- The classified image is written as `unsigned int`, the posteriors (and likelihoods,
  distances, B-spline images) as `float`, all with the mask's geometry; the posteriors are 0
  outside the mask.
- `-o [seg,post]` with a name without `%d` writes every class to the same file (the last one
  wins).
- A mask is required; it is read as `unsigned int`, so 0.6 is 0.
- The convergence value is the mean of the voxels' largest posteriors, `float`; with
  `-c [n,0]` Atropos stops early only if the value decreases.
- With `-v 0` (the default) most errors are not explained (exit 1 only, ANTS-45); larmorx
  always explains them, on ANTs' stream.
- nipype's `Atropos` interface writes `--icm [%d,%g]` from `icm_use_synchronous_update` and
  `maximum_number_of_icm_terations` (`nipype/interfaces/ants/segmentation.py:219-223`): the
  bracketed form ANTs 2.6.5 aborts on (ANTS-102), and `True` writes 1, which Atropos reads as
  *asynchronous* updating (ICM), the opposite of the trait's name. sMRIPrep sets neither.

## Inputs on another grid: ITK's check, missing from the port until 2026-10-26

*Found on the data panel (re-validation, part a), fixed.* `AtroposSegmentationImageFilter` is an
`ImageToImageFilter`: at `Update`, before anything is read, `VerifyInputInformation`
(`itkImageToImageFilter.hxx:147-216`, ITK v5.4.5) compares its inputs (the first intensity image,
`Primary`; the mask, `_1`; the prior label image, `_2`; the other intensity images, `_3`, ...;
the prior probability images are not inputs), in the order of their names, with the first: an
origin or spacing further than `1e-6 × spacing[0]` from it, or a direction further than 1e-6,
throws "Inputs do not occupy the same physical space!" with the values that differ. ANTs exits 1
and prints the exception only with `--verbose 1` (ANTS-45); the panel case was the FLAIR's head
mask written by nibabel from the FLAIR's affine, whose recomputed pixdim (0.42187503 × 0.421875
× 2.9999971 mm against 0.421875 × 0.421875 × 3) is 2.9e-6 mm off, beyond the 4.2e-7 tolerance.
larmorx ran such calls (it compared sizes only). It now makes the same check and prints the same
exception (`cli::atropos::input_information_error`; a larmorx line without `--verbose`, as for
ANTs' other silent exceptions). Permanent cases: the standard suite's `geometry/*` (the mask's
spacing, origin and direction, without `--verbose`, the prior label image and a second intensity
image) and the panel's `flair/mask-from-affine`, besides the Rust test
`a_mask_on_another_grid_is_itks_exception`. The panel's `flair/kmeans-3` now writes the mask with
the FLAIR's own header, and segments it bit for bit.
