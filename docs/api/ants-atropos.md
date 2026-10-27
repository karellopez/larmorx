# Atropos

`lx.ants.atropos` (command line `larmorx ants Atropos`; Rust `larmorx_ants::atropos::Atropos`,
the command line `larmorx_ants::cli::atropos`). A replica of ANTs 2.6.5's `Atropos` on ITK
5.4.5: finite-mixture segmentation with a Markov random field prior (Avants et al., *An open
source multivariate framework for n-tissue segmentation with evaluation on public data*,
Neuroinformatics 9:381-400, 2011). sMRIPrep runs it once per subject, in its brain extraction
(`atropos_wf`): K-means with three classes, Gaussian likelihoods, MRF 0.1, three iterations,
the fixed seed.

**Status: `validated`** against Atropos itself (see the
[validation record](../validation/ants-atropos.md)), on every option and failure case on
phantoms, and on sMRIPrep's call and antsCorticalThickness-style prior-based segmentations of
the five fMRIPrep oracle runs' T1w images (standard tier: 93 cases, 56 compared):
- against the **unmodified ANTs 2.6.5 binary** (one thread): 25 of 56 compared cases
  bit-identical, the classified image identical in all 56. The others' posteriors differ in
  their last bits where glibc's functions are not correctly rounded for a value the run
  meets: on sMRIPrep's call, in 11 to 4,175 of 5.9 to 17.5 million values of a posterior
  image (none on ds001600), by at most 1.2e-7;
- against the **same binary with glibc's `expf`, `logf`, `exp`, `log`, `pow` (and the others)
  replaced by CORE-MATH's correctly rounded ones** (`LD_PRELOAD`,
  `scripts/build_crmath_preload.sh`), the arithmetic larmorx uses: **56 of 56 bit-identical**, sMRIPrep's call on the five oracle
  runs and two prior-based segmentations included;
- **ANTs' Atropos does not depend on its thread count**, and neither does larmorx's; with the
  clock's seed (`-r 1`, Atropos' default) two ANTs runs of sMRIPrep's options on ds000005 disagree on 343 labels
  (smallest Dice 0.99942) and on posteriors by up to 0.28; larmorx, with its fixed seed,
  is as far from them (294 and 327 labels).

**On the data panel** ([record](../validation/ants-atropos-panel.md), 2026-10-26): sMRIPrep's
call in all fourteen finished oracle runs (the 3.5-year-old, the glioma, the stroke, strong
motion, 1.5 T, 7 T and the 82-year-old among them), its inputs from each run's working directory,
and three calls fMRIPrep never makes (the stroke outside its manual lesion mask, a child's T1w and
T2w as two channels, a FLAIR of 3 mm slices): **17 of 17 compared cases bit-identical with
CORE-MATH, the classified image identical in all 17** (3 bit-identical to the unmodified ANTs).
Against fMRIPrep's own files (ANTs 2.6.2, glibc's maths): the same segmentation in every run,
the posteriors different in 0 to 4,175 voxels (their last bits, as above). The panel found a
missing check, now ported: an input on another grid (the FLAIR's mask written from its affine,
2.9e-6 mm off) makes ITK throw, as ANTs does ([findings](../findings/ants-atropos.md), "Inputs
on another grid").

## Quick start

```python
import larmorx as lx

# sMRIPrep's call (the defaults are Atropos': Gaussian likelihoods, MRF 0.3, 5 iterations)
seg = lx.ants.atropos("t1w_n4.nii.gz", "brain_mask.nii.gz", classes=3, mrf_smoothing=0.1,
                      iterations=3, convergence_threshold=0, use_random_seed=False,
                      n_threads=8)
seg.labels.to_filename("labels.nii.gz")      # 1 = CSF, 2 = GM, 3 = WM (by intensity)
csf, gm, wm = seg.posteriors                  # float32 probability images

# antsCorticalThickness-style: tissue priors, weight 0.25, five iterations
seg = lx.ants.atropos(t1w, mask, initialization="prior-probabilities",
                      priors=["csf.nii.gz", "gm.nii.gz", "wm.nii.gz"], prior_weight=0.25,
                      mrf_smoothing=0.1, iterations=5, convergence_threshold=0)

# two contrasts, histogram Parzen windows, box-plot winsorizing
seg = lx.ants.atropos([t1w, t2w], mask, initialization="otsu",
                      likelihood=lx.ants.HistogramParzenWindows(sigma=1.0, bins=32),
                      outliers=lx.ants.BoxPlot())
```

```bash
larmorx ants Atropos -d 3 -a t1w_n4.nii.gz -x brain_mask.nii.gz -i KMeans[3] -k Gaussian \
    -m [0.1,1x1x1] -c [3,0] -r 0 -o [labels.nii.gz,POSTERIOR_%02d.nii.gz]
```

## Option mapping

| Atropos | Python | Notes |
|---|---|---|
| `-d 2/3/4` | `dimension` (default: the first image's) | without `-d` ANTs reads every image as 3D (ANTS-99 below); the Python wrapper always passes it |
| `-a image` / `-a [image,w]` (repeatable) | `images` (one or a list); `adaptive_smoothing=w` (one value, or one per image) | paths, `lx.Image`, nibabel images or `(array, affine)`, read as `float` on the first image's grid; the weight mixes the intensity with its B-spline fit per class, for prior-based initializations only (ANTS-93 below) |
| `-x mask` | `mask` | required; read as `unsigned int` (0.6 is 0); its non-zero voxels are segmented |
| `-i Random[K]` | `initialization="random"`, `classes=K` | labels drawn with the filter's generator |
| `-i Otsu[K]` | `initialization="otsu"` | Otsu's thresholds of the first image (200 bins) |
| `-i KMeans[K]` / `KMeans[K,c1xc2x...]` | `initialization="kmeans"`, `kmeans_centers=[...]` | k-means on the first image from centres spread over its range in the mask, or given; with several images, a second k-means on all |
| `-i PriorProbabilityImages[K,pattern%02d,w(,t)]` | `initialization="prior-probabilities"`, `priors=[...]`, `prior_weight=w`, `prior_threshold=t` | one image per class (paths or images; the wrapper names them as a `%02d` series), or one vector image on the command line; `t`: below it a voxel counts as without priors |
| `-i PriorLabelImage[K,labels,w]` | `initialization="prior-label"`, `prior_label`, `prior_weight` | labels 1 to K; unlabelled voxels of the mask are reached by propagation |
| `-p Socrates[mix,T0,rate,Tmin]` (`Plato`, `Aristotle`, `Sigmoid`) | `posterior_formulation="socrates"`, `use_mixture_model_proportions`, `annealing=(T0, rate, Tmin)` | see the [findings](../findings/ants-atropos.md#the-algorithm-as-the-code-has-it); annealing applies only when `T0` is not 1 |
| `-k Gaussian` | `likelihood=lx.ants.GaussianLikelihood()` (the default) | weighted mean and covariance per class |
| `-k HistogramParzenWindows[σ,bins]` | `lx.ants.HistogramParzenWindows(sigma, bins)` | |
| `-k ManifoldParzenWindows[σr,kEval,kCov,σk]` | `lx.ants.ManifoldParzenWindows(regularization_sigma, evaluation_k, covariance_k, kernel_sigma)` | ANTS-95, ANTS-104 |
| `-k JointShapeAndOrientationProbability[σs,bs,σo,bo]` | `lx.ants.JointShapeAndOrientationProbability(...)` | tensors: 6 images in 3D (xx xy xz yy yz zz); ANTS-103 |
| `-k LogEuclideanGaussian` | `lx.ants.LogEuclideanGaussian()` | tensors, as above |
| `-c [n,threshold]` | `iterations=5`, `convergence_threshold=0.001` | stops when the mean largest posterior grows by less than the threshold |
| `-m [β,radius]` / `-m [coefficients,radius]` | `mrf_smoothing=0.3` (a number or an image), `mrf_radius=1` (one value or one per axis) | |
| `-g [async,iterations,codes]` | `icm=True`, `icm_iterations=1`, `icm_code_image=True` → `AtroposResult.icm_codes` | the documented bracketed form makes ANTs abort (ANTS-102); the wrapper writes `-g 1[1,n]` |
| `-r 0/1` | `use_random_seed=True` | ANTs seeds from the clock (`1`, its default) or with 19650218 (`0`); larmorx never reads the clock: `1` gives the seed 121212 |
| `-e 0/1` | `euclidean_distance` | propagate prior labels by Euclidean distance instead of fast marching |
| `-l [λ,p]` / `-l label[λ,p]` | `label_propagation=(λ, p)` / `{label: (λ, p)}` | |
| `-u 0/1` | `minimize_memory` | ANTS-97, ANTS-105 |
| `-w BoxPlot[l,u,w]` / `-w GrubbsRosner[s,w]` | `outliers=lx.ants.BoxPlot(...)` / `lx.ants.GrubbsRosner(...)` | ANTS-94, ANTS-107, ITK-51 |
| `-s 1x2` (repeatable), `--use-partial-volume-likelihoods` | `partial_volume=[(1, 2)]`, `partial_volume_likelihoods` | partial-volume classes (labels after the tissues, in the order given); with the MRF, ANTs' partial-volume labels hardly win a voxel and Atropos often stops (ANTS-106, corrected below) |
| `-b [levels,mesh,order]` | `bspline=(6, 1, 3)` | the adaptive smoothing's B-spline fit |
| `-o [seg,post%02d,lik%02d,dist%02d,bsp%02d]` | `AtroposResult.labels`, `.posteriors`; `likelihood_images`, `distance_images`, `smoothed_images` → `.likelihoods`, `.distances`, `.smoothed` | the classified image as `uint32`, the others `float32`, 0 outside the mask |
| `-v 1` | `AtroposResult.iterations`, `.log` | the progress lines and the filter's settings and models |
| `ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS` | `n_threads` | neither ANTs' result nor larmorx's depends on it |

`lx.ants.atropos` raises `lx.ants.AtroposError` with ANTs' message where Atropos stops (a
class that loses all its voxels, a singular covariance, a missing file), and `ValueError` for
arguments the command line cannot express.

## Upstream defects (D13)

ANTs' and ITK's deterministic defects are corrected by default and reproduced bit for bit on
request: `upstream_defects="reproduce"` in Python (`AtroposOptions::upstream_defects` in
Rust), `LARMORX_UPSTREAM_DEFECTS=reproduce` on the command line. A warning (on the command
line a line on standard error) names the entry wherever it changed the result
([upstream-defects.md](upstream-defects.md); each entry in the
[register](../findings/upstream-defects.md)).

| ID | ANTs (what `reproduce` gives) | larmorx's default |
|---|---|---|
| ITK-50 | k-means leaves out the point each divided k-d tree node is split at | every point counted |
| ITK-51 | Student quantiles are NaN above about 340 degrees of freedom: Grubbs-Rosner never finds an outlier in an image (SLATEC's overflow messages on standard error) | the ratio of Gamma functions through their logarithms where it would overflow |
| ANTS-90 | with several images only the first decides the initial labelling (the second k-means is seeded wrongly and its labels thrown away) | the second k-means from each class's means, its labels kept |
| ANTS-91 | Otsu's range from `FLT_MIN` with `else if` | the image's range |
| ANTS-92 | random labels outside the mask enter the first estimates | labels in the mask only |
| ANTS-93 | the adaptive smoothing weights of several `-a` apply in reverse order | each image its own weight |
| ANTS-94 | the outlier filters keep their previous output (box plot: bounds from the previous values too; Grubbs-Rosner: the later images get the first image's values, the sample grows) | each call on its own input |
| ANTS-95 | manifold Parzen windows ignore the posterior weights | weighted |
| ANTS-96 | partial-volume classes keep their initial means | re-estimated from their two classes' posteriors |
| ANTS-97 | `-u 1` stores prior voxel numbers in `float` (rounded above 2^24 voxels) | exact |
| ANTS-98 | K-means' range from the mask's label 1 only (a mask of 255s: infinite centres) | the mask's non-zero voxels |
| ANTS-99 | without `-d` every image is 3D | the first image's dimension |
| ANTS-100 | priors as one vector image: every class gets the last component | each class its component |
| ANTS-101 | an unknown `-p` name keeps Socrates silently | an error |
| ANTS-102 | `-g [1,2]` aborts | read as documented |
| ANTS-103 | the joint shape and orientation model evaluated at the raw components and an unset coordinate (0 here) | at the tensor's eigenvalues and principal direction |
| ANTS-104 | manifold Parzen covariances double their running sums | the documented local covariance |
| ANTS-105 | `-u 1` with priors and a mask crashes | runs |
| ANTS-106 | in the MRF a partial-volume label is +1 against every neighbour, its own tissues included: with partial-volume likelihoods and an MRF the partial-volume classes hardly win a voxel, and Atropos stops when one wins none | a partial-volume label counts as a member of its own set: −1 with its tissues, −2 with itself, +1 with any other label (decided 2026-10-22) |
| ANTS-107 | Grubbs-Rosner's grown sample weighed with uninitialized memory (ANTs' result depends on the heap) | not met (the sample does not grow); reproduced, the extra weights are 0 |
| ANTS-03, ANTS-06, ITK-02, ITK-08 | as for every ANTs program: names of fewer than 3 characters, out-of-range conversions, the direction of a file with more dimensions than `-d`, `x.nii` read for `x.nii.gz` ([ImageMath](ants-image-math.md#upstream-defects-d13)) | as there |

**ANTS-106, measured** on a phantom whose partial-volume fractions are known (nested spheres of
three tissues box-averaged from a five-times finer grid; the record's `partial-volume-phantom-*`
cases; classes 1x2 and 2x3, five iterations): with the MRF at 0.1, ANTs labels 0.295 and 0.405
(Dice) of the two partial-volume classes and 94.0 % of the voxels as the truth; corrected, 0.862,
0.805 and 97.7 %. With the MRF at 0.3 (Atropos' default) ANTs stops ("2 non-zero label sets but
5 is requested"); corrected, 0.835, 0.801 and 97.8 %. Without the MRF ANTs gives 0.728 and 0.578
(93.7 %), larmorx's default 0.711 and 0.594 (93.4 %: ITK-50's and ANTS-96's corrections alone).
On sMRIPrep's call with partial-volume classes on fMRIPrep's T1w the correction is what lets
Atropos finish (the record's `real/*/smriprep-partial-volume`: ANTs stops; corrected, 31 % of
ds000005's brain voxels fall in the two partial-volume classes).

On sMRIPrep's call ITK-50 is met (its K-means): on the five oracle runs the correction
changes 1 to 25 of 0.8 to 2.5 million brain voxels' labels (Dice at least 0.99999 for every
class) and the posteriors by at most 0.14, less than ANTs' own spread when it seeds from
the clock (343 labels, 0.28 on ds000005).

## Deliberate differences

- **Seeds**: larmorx never reads the clock; `-r 1` (and Python's `use_random_seed=True`) use
  the seed 121212, so its runs repeat. ANTs' do not.
- **Maths**: `expf`, `logf`, `exp`, `log`, `pow`, `acos`, `atan` are correctly rounded
  (CORE-MATH), the same on every platform; glibc's are not always, so ANTs as built differs in
  the last bits of a few posteriors ([findings](../findings/ants-atropos.md#glibcs-maths)).
- Where ANTs aborts or crashes (an unknown output extension, a value the parser cannot convert,
  ANTS-105 when reproduced), larmorx prints ANTs' message and exits 1.
- `-v` prints the filter's settings and models but not ITK's base-class lines (addresses,
  times, thread settings).
- Images are NIfTI only.

## Rust

```rust
use larmorx_ants::atropos::{Atropos, AtroposOptions, Initialization};

let mut options = AtroposOptions::new(3);
options.initialization = Initialization::KMeans { centers: None };
options.mrf_smoothing = 0.1;
options.max_iterations = 3;
options.convergence_threshold = 0.0;
let mut filter = Atropos::new(&options, &size, &spacing, &origin, vec![&t1w], Some(&mask), n_threads)?;
filter.run()?;
let labels = filter.labels.clone();          // 0 outside the mask
let posterior_1 = filter.posterior(0)?;      // class 1
```

The models are `larmorx_ants::atropos::models` (Gaussian, partial volume, histogram and
manifold Parzen windows, log-Euclidean, joint shape and orientation), the outlier filters
`larmorx_ants::atropos::outliers`, Student's distribution `larmorx_ants::atropos::tdist`;
ITK's k-d tree and k-means estimator are `larmorx_image::kdtree`.
