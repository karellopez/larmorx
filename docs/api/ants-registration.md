# antsRegistration (linear stages)

`lx.ants.registration` (command line `larmorx ants antsRegistration`; Rust
`larmorx_ants::registration::helper::RegistrationHelper` and the crate `larmorx-registration`).
A replica of ANTs 2.6.5's `antsRegistration` on ITK 5.4.5's v4 registration framework
(Apache-2.0) for **linear stages**: `Translation`, `Rigid`, `Similarity`, `Affine` and
`CompositeAffine`, in 2, 3 and 4 dimensions, double and float (`--float 1`), with every image
metric (`Mattes`/`MI`, `MI2`, `MeanSquares`, `CC`, `GC`; `Demons` fails in a linear stage as in
ANTs), several metrics per stage, sampling, masks, multi-resolution shrinking and smoothing,
the gradient filter, winsorizing and histogram matching, initial transforms (files and
`[fixed,moving,feature]`), `--initialize-transforms-per-stage`, `--restore-state` and
`--save-state`, `--print-similarity-measure-interval` and `--write-interval-volumes`.
Deformable stages (`SyN`, `BSpline`, displacement and velocity fields) come in milestone A5;
larmorx refuses them with a message.

**Status: `validated`** against antsRegistration itself (see the
[validation record](../validation/ants-registration.md)): every compared case identical to
ANTs 2.6.5 at one thread with the same seed (exit status, standard output with every
`DIAGNOSTIC` line, standard error, every file) — most against the unmodified binary, the rest
once glibc's `exp`/`log`/`sin`/`cos` are replaced by CORE-MATH's correctly rounded ones, which
larmorx uses (CLAUDE.md rule 5).

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
| `-n Linear` | `interpolation="Linear"` | the warped images' interpolator, as `antsApplyTransforms -n`; in 2D and 4D `Linear` and `NearestNeighbor` only |
| `--float 1` | `precision="float32"` | ANTs' single-precision pipeline |
| `--random-seed N` / `ANTS_RANDOM_SEED` | `random_seed=N` | without one: ANTs from the clock, larmorx 121212 |
| `--print-similarity-measure-interval N` | `print_similarity_measure_interval=N` | `RegistrationIteration.full_scale_value` |
| `--write-interval-volumes N` | – | command line only |
| `-v 1` | `RegistrationResult.log`, `.iterations` | Python always runs verbose and parses the `DIAGNOSTIC` lines |
| `--minc 1` | – | MINC transforms: not supported yet |
| `ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS` | `n_threads` | the result does not depend on it |
| `LARMORX_UPSTREAM_DEFECTS` | `upstream_defects=` | ANTS-49, -50, -51, ITK-18 ([upstream defects](upstream-defects.md)) |

## Not supported yet

- Deformable stages (A5), point-set metrics (`ICP`, `PSE`, `JHCT`, `IGDM`), MINC transform
  files: refused with a message.
- In 2D and 4D, the warped images' `BSpline`, `Gaussian`, windowed-sinc, `GenericLabel` and
  `MultiLabel` interpolators (3D has them all): refused before the registration runs.
- Displacement fields among the initial transforms (`-r field.nii.gz`, or a composite holding
  one), in any dimension.
- Transform files of classes larmorx does not build in 2D and 4D (ITK's `CenteredRigid2D`,
  `CenteredSimilarity2D`, `CenteredAffine`, `Scale`, B-spline transforms): refused. Files of the
  classes antsRegistration writes (`AffineTransform`, `MatrixOffsetTransformBase`,
  `Euler2DTransform`, `Rigid2DTransform`, `Similarity2DTransform`, `TranslationTransform`,
  `IdentityTransform`) are read in every dimension.
