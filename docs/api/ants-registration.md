# antsRegistration (linear stages, point-set metrics)

`lx.ants.registration` (command line `larmorx ants antsRegistration`; Rust
`larmorx_ants::registration::helper::RegistrationHelper` and the crate `larmorx-registration`).
A replica of ANTs 2.6.5's `antsRegistration` on ITK 5.4.5's v4 registration framework
(Apache-2.0) for **linear stages**: `Translation`, `Rigid`, `Similarity`, `Affine` and
`CompositeAffine`, in 2, 3 and 4 dimensions, double and float (`--float 1`), with every image
metric (`Mattes`/`MI`, `MI2`, `MeanSquares`, `CC`, `GC`; `Demons` fails in a linear stage as in
ANTs) and every point-set metric (`ICP`, `PSE`, `JHCT`, `IGDM`; see
[Point-set metrics](#point-set-metrics)), several metrics per stage, sampling, masks,
multi-resolution shrinking and smoothing,
the gradient filter, winsorizing and histogram matching, initial transforms (files and
`[fixed,moving,feature]`), `--initialize-transforms-per-stage`, `--restore-state` and
`--save-state`, `--print-similarity-measure-interval` and `--write-interval-volumes`.
Deformable stages (`SyN`, `BSpline`, displacement and velocity fields) come in milestone A5;
larmorx refuses them with a message.

**Status: `validated`** against antsRegistration itself (the
[validation record](../validation/ants-registration.md), standard tier, and the
[full tier](../validation/ants-registration-full.md)): every compared case identical to ANTs
2.6.5 at one thread with the same seed (exit status, standard output with every `DIAGNOSTIC`
line, standard error, every file), most against the unmodified binary, the rest once glibc's
`exp`/`log`/`sin`/`cos` are replaced by CORE-MATH's correctly rounded ones, which larmorx uses
(CLAUDE.md rule 5).

| Tier | Cases | Identical to the unmodified ANTs | Identical with CORE-MATH | Rejected by both | Expected divergences | Failures |
|---|---|---|---|---|---|---|
| standard | 168 | 104 | 24 | 31 | 9 | 0 |
| full | 175 | 102 | 33 | 31 | 9 | 0 |

The standard record was re-run by A4 part 3 (2026-10-17), with two cases of the gradient filter
on oblique grids (its float components, [findings](../findings/ants-ai.md)); the full record
predates them.

The cases cover every transform in 2D, 3D and 4D, double and float, every metric and sampling,
masks, the initial transforms and their files, every output and interpolator, every
command-line check and message, fMRIPrep 25.2.6's own command lines of four oracle nodes, and
(full tier) the linear stages of sMRIPrep's brain extraction and template registration in all
five oracle runs, and (A4 part 4) the point-set metrics: each metric alone and with the
others and image metrics, label images (with boundary points), text and VTK point sets
(ASCII, binary, without scalars), in-memory points, 2D, 3D and 4D, double and float, initial
transforms, several levels and stages, and every message, failure and crash. The expected
divergences: no seed (ANTs from the clock), ANTS-51's crash, SyN (A5), `--version`, three of
ANTS-45's silent failures, MINC transforms (refused), a point set's random fraction (ANTs seeds
it from the clock, ANTS-64), IGDM's value for neighbourhoods of more than one voxel (ANTs reads
before its arrays, ANTS-66) and a text point set ANTs reads forever (ANTS-69). **ANTs' own thread band** (14 cases, 2, 4 and 8 threads against one):
about 1e-13 to 1e-12 in a transform parameter in double, 3.5e-9 on fMRIPrep's BOLD-reference
affine and 5e-3 on a float affine; larmorx gives the one-thread result at any thread count.

**Results do not depend on threads.** ANTs' do (its metrics sum per work unit); larmorx always
gives ANTs' one-thread result. **Without a seed ANTs is not repeatable** (its sampling is seeded
from the clock); larmorx then uses a fixed seed (121212).

## Quick start

```python
import larmorx as lx

stage1 = lx.ants.RegistrationStage(
    transform="Rigid", gradient_step=0.1,
    metrics=lx.ants.RegistrationMetric("Mattes", "fixed.nii.gz", "moving.nii.gz",
                                       bins=32, sampling="Regular", sampling_percentage=0.25),
    iterations=[1000, 500, 250], shrink_factors=[4, 2, 1], smoothing_sigmas=[2, 1, 0],
)
stage2 = lx.ants.RegistrationStage(
    transform="Affine", gradient_step=0.1,
    metrics=lx.ants.RegistrationMetric("Mattes", "fixed.nii.gz", "moving.nii.gz",
                                       sampling="Regular", sampling_percentage=0.25),
    iterations=[1000, 500, 250], shrink_factors=[4, 2, 1], smoothing_sigmas=[2, 1, 0],
)
res = lx.ants.registration(
    [stage1, stage2],
    initial_moving_transforms=[lx.ants.AlignCenters("fixed.nii.gz", "moving.nii.gz", "moments")],
    winsorize=(0.005, 0.995), use_histogram_matching=True,
    warped=True, random_seed=42,
)
res.files["0GenericAffine.mat"]                 # the collapsed affine, as ITK transforms
warped = lx.ants.apply_transforms("moving.nii.gz", "fixed.nii.gz", res.transforms)
[(it.stage, it.level, it.iteration, it.value) for it in res.iterations]
```

```bash
larmorx ants antsRegistration -d 3 -o [out_,out_warped.nii.gz] \
    -r [fixed.nii.gz,moving.nii.gz,1] \
    -t Rigid[0.1] -m Mattes[fixed.nii.gz,moving.nii.gz,1,32,Regular,0.25] \
    -c [1000x500x250,1e-6,10] -f 4x2x1 -s 2x1x0vox \
    -t Affine[0.1] -m Mattes[fixed.nii.gz,moving.nii.gz,1,32,Regular,0.25] \
    -c [1000x500x250,1e-6,10] -f 4x2x1 -s 2x1x0vox -w [0.005,0.995] -u 1 --random-seed 42
```

## Option mapping

| antsRegistration | Python | Notes |
|---|---|---|
| `-d 2/3/4` | `dimension` | default: the first fixed image's |
| `-o [prefix,warped,inverse]` | `RegistrationResult.files`, `.transforms`, `warped=True`, `inverse_warped=True` | Python writes the transforms to a temporary directory and reads them back; the warped images stay in memory |
| `-t Rigid[step]` | `RegistrationStage(transform="Rigid", gradient_step=step)` | `Translation`, `Rigid`, `Similarity`, `Affine`, `CompositeAffine` (deformable transforms: A5) |
| `-m ICP[f,m,w,p,boundary]` | `PointSetMetric("ICP", f, m, weight=w, sampling_percentage=p, boundary_points_only=boundary)` | `f`, `m`: a label image, a `.txt` or `.vtk` point set, or `(points, labels)` arrays |
| `-m PSE[f,m,w,p,boundary,sigma,k]` | `PointSetMetric("PSE", ..., sigma=sigma, k_neighborhood=k)` | |
| `-m JHCT[f,m,w,p,boundary,sigma,k,alpha,aniso]` | `PointSetMetric("JHCT", ..., sigma=, k_neighborhood=, alpha=, anisotropic_covariances=)` | |
| `-m IGDM[f,m,w,fmask,mmask,radius,isigma,dsigma,k,gsigma]` | `IntensityPointSetMetric(f, m, fmask, mmask, weight=, neighborhood_radius=, intensity_sigma=, distance_sigma=, k_neighborhood=, gradient_sigma=)` | sigmas of 0 are estimated; `k` and `gsigma` are not used by ANTs (ANTS-67) |
| `-m Mattes[f,m,w,bins,sampling,p,gf]` | `RegistrationMetric("Mattes", f, m, weight=w, bins=bins, sampling=..., sampling_percentage=p, use_gradient_filter=gf)` | `MI` is ANTs' other name for Mattes; `MI2` the joint-histogram MI; `CC[f,m,w,radius,...]`: `radius=`; `MeanSquares`, `GC`, `Demons` have no parameter |
| several `-m` in a stage | `metrics=[...]` | the first metric's value; derivatives weighed by `weight` (normalised) |
| `-c [10x5,1e-6,10]` | `iterations=[10, 5]`, `convergence_threshold=1e-6`, `convergence_window=10` | a level stops when the window's convergence value is ≤ the threshold (negative when the metric rises: [findings](../findings/ants-registration.md)) |
| `-f 4x2x1` | `shrink_factors=[4, 2, 1]` | per level; per axis as `CalculateShrinkFactorsPerDimension` chooses |
| `-s 2x1x0vox` / `mm` | `smoothing_sigmas=[2, 1, 0]`, `sigmas_in_mm=` | |
| `-x mask` / `-x [fixed,moving]` | `RegistrationStage.masks=mask` / `(fixed, moving)` | `None` in the pair is ANTs' `NULL` |
| `-g 1x1x0x1x1x1` | `restrict_deformation=[1, 1, 0, 1, 1, 1]` | weights of the transform's parameters |
| `-r file` / `-r [file,1]` / `-r [f,m,0/1/2]` | `initial_moving_transforms=[path or ItkTransforms, (t, True), AlignCenters(f, m, "geometry"/"moments"/"origins")]` | |
| `-q ...` | `initial_fixed_transforms=` | |
| `-k state` | `restore_state=` | |
| `-j state` | – | only on the command line (`--save-state`, needs `-a 1`; ANTS-50) |
| `-w [0.005,0.995]` | `winsorize=(0.005, 0.995)` | |
| `-u 1` | `use_histogram_matching=True` | |
| `-z 0/1` | `collapse_output_transforms=` | default 1 |
| `-a 1` | `write_composite_transform=True` | `Composite.h5`, `InverseComposite.h5` |
| `-i 1` | `initialize_transforms_per_stage=True` | needs `-a 1` and `-z 0` |
| `-n Linear` | `interpolation="Linear"` | the warped images' interpolator, as `antsApplyTransforms -n`, in every dimension |
| `--float 1` | `precision="float32"` | ANTs' single-precision pipeline |
| `--random-seed N` / `ANTS_RANDOM_SEED` | `random_seed=N` | without one: ANTs from the clock, larmorx 121212 |
| `--print-similarity-measure-interval N` | `print_similarity_measure_interval=N` | `RegistrationIteration.full_scale_value` |
| `--write-interval-volumes N` | – | command line only |
| `-v 1` | `RegistrationResult.log`, `.iterations` | Python always runs verbose and parses the `DIAGNOSTIC` lines |
| `--minc 1` | – | MINC transforms: not supported yet |
| `ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS` | `n_threads` | the result does not depend on it |
| `LARMORX_UPSTREAM_DEFECTS` | `upstream_defects=` | ANTS-49, -50, -51, ITK-18; the point-set metrics' ANTS-61 to -63, -65, -66, ITK-22, -23, -26, -28 ([upstream defects](upstream-defects.md)) |

## Point-set metrics

`ICP` (the distance to the nearest moving point of the same label), `PSE` (a Gaussian
expectation over the `k` nearest), `JHCT` (the Jensen-Havrda-Charvát-Tsallis divergence of a
manifold Parzen density) and ANTs' `IGDM` (the intensity and the distance of the nearest
masked voxel), with ITK's k-d tree for the neighbours. What they compute, and the defects
found: [findings](../findings/ants-registration.md#point-set-metrics-a4-part-4-2026-10-18).

```python
import numpy as np
import larmorx as lx

points = np.array([[10.0, -4.0, 2.5], [12.0, -4.0, 2.5], [11.0, 6.0, -3.0]])  # LPS, mm
labels = np.array([1, 1, 2])
stage = lx.ants.RegistrationStage(
    "Rigid", 0.1,
    lx.ants.PointSetMetric("ICP", (points, labels), "moving_labels.nii.gz"),
    iterations=[50], shrink_factors=[1], smoothing_sigmas=[0],
    masks="fixed_mask.nii.gz",   # the virtual domain: required for point-set metrics
)
res = lx.ants.registration([stage], dimension=3, random_seed=42)

igdm = lx.ants.IntensityPointSetMetric("fixed.nii.gz", "moving.nii.gz",
                                       "fixed_mask.nii.gz", "moving_mask.nii.gz",
                                       neighborhood_radius=0)
```

- **A fixed mask is required** (`masks=` / `-x`): its grid is the stage's virtual domain (ANTs
  crashes without one, ANTS-69). Warped images, `print_similarity_measure_interval` and
  `--write-interval-volumes` need an image metric first; a Regular or Random sampling strategy
  cannot share a stage with a point-set metric (ANTs crashes; larmorx stops with a message).
- **Point sets:** a label image (every voxel whose label is not 0, at its physical point; with
  `boundary_points_only`, the voxels on each label's contour), a `.txt` file (`x y z label`
  per line, `x y 0 label` in 2D, ITK's LPS coordinates), a `.vtk` polydata file (ASCII or
  binary; the scalars are the labels), or `(points, labels)` arrays. Only labels both sets
  have are matched (ITK-22: by default the others take no part).
- **`sampling_percentage` below 1** keeps a random fraction of each point set, seeded from
  `random_seed` (ANTs: the clock, ANTS-64).
- **Defaults that differ from ANTs** (D13; `upstream_defects="reproduce"` gives ANTs' result):
  ANTS-61 (IGDM with other kinds of metrics), ANTS-62 (a text file's final newline), ANTS-63
  (binary VTK labels), ANTS-65 (IGDM's intensity sigma), ANTS-66 (IGDM's value with a
  neighbourhood), ITK-22 (labels the moving set lacks), ITK-23 (an identity covariance's
  prefactor), ITK-26 (JHCT's covariance diagonal), ITK-28 (JHCT's `alpha` near 1). Each is
  named in a notice where it changes the result.
- **Correctly rounded powers:** JHCT calls `pow`; larmorx's is CORE-MATH's (glibc's differs by
  an ulp in about 0.1 % of calls).

## Not supported yet

- Deformable stages (A5), MINC transform files: refused with a message.
- Displacement fields among the initial transforms (`-r field.nii.gz`, or a composite holding
  one), in any dimension.
- Transform files of classes larmorx does not build in 2D and 4D (ITK's `CenteredRigid2D`,
  `CenteredSimilarity2D`, `CenteredAffine`, `Scale`, B-spline transforms): refused. Files of the
  classes antsRegistration writes (`AffineTransform`, `MatrixOffsetTransformBase`,
  `Euler2DTransform`, `Rigid2DTransform`, `Similarity2DTransform`, `TranslationTransform`,
  `IdentityTransform`) are read in every dimension.
