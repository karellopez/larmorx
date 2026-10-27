# antsRegistration (every stage, image and point-set metrics)

`lx.ants.registration` (command line `larmorx ants antsRegistration`; Rust
`larmorx_ants::registration::helper::RegistrationHelper` and the crate `larmorx-registration`).
A replica of ANTs 2.6.5's `antsRegistration` on ITK 5.4.5's v4 registration framework
(Apache-2.0) for **linear stages** (`Translation`, `Rigid`, `Similarity`, `Affine`,
`CompositeAffine`), **SyN stages** (`SyN[gradientStep,updateFieldVariance,totalFieldVariance]`,
milestone A5 part 1; see [SyN stages](#syn-stages)) and the **B-spline and displacement-field
stages** (`BSplineSyN`, `GaussianDisplacementField`, `BSplineDisplacementField`, `BSpline`,
milestone A5 part 2; see [B-spline and displacement-field stages](#b-spline-and-displacement-field-stages))
and the **velocity-field stages** (`Exponential`, `BSplineExponential`,
`TimeVaryingVelocityField`, `TimeVaryingBSplineVelocityField`, milestone A5 part 3; see
[Velocity-field stages](#velocity-field-stages)): every transform `antsRegistration` has,
in 2, 3 and 4 dimensions, double and float
(`--float 1`), with every image metric (`Mattes`/`MI`, `MI2`, `MeanSquares`, `CC`, `GC`, and
`Demons`, which needs a displacement field: it fails in a linear stage as in ANTs) and every
point-set metric (`ICP`, `PSE`, `JHCT`, `IGDM`; see [Point-set metrics](#point-set-metrics)),
several metrics per stage, sampling, masks, multi-resolution shrinking and smoothing, the
gradient filter, winsorizing and histogram matching, initial transforms (files, displacement
fields and composites holding them, `[fixed,moving,feature]`), `--initialize-transforms-per-stage`,
`--restore-state` and `--save-state`, `--print-similarity-measure-interval` and
`--write-interval-volumes`. larmorx refuses nothing ANTs accepts, except MINC transform files
and the crashes and hangs listed in the [register](upstream-defects.md).

**Status: `validated`** against antsRegistration itself (the
[validation record](../validation/ants-registration.md), standard tier, and the
[full tier](../validation/ants-registration-full.md)): every compared case identical to ANTs
2.6.5 at one thread with the same seed (exit status, standard output with every `DIAGNOSTIC`
line, standard error, every file), most against the unmodified binary, the rest once glibc's
`exp`/`log`/`sin`/`cos` are replaced by CORE-MATH's correctly rounded ones, which larmorx uses
(CLAUDE.md rule 5).

| Tier | Cases | Identical to the unmodified ANTs | Identical with CORE-MATH | Rejected by both | Expected divergences | Failures |
|---|---|---|---|---|---|---|
| standard | 505 | 347 | 44 | 93 | 21 | 0 |
| full (A5 part 2's run) | 441 | 278 | 68 | 77 | 18 | 0 |
| full, A5 part 3's cases ([record](../validation/ants-registration-velocity-full.md)) | 124 | 77 | 26 | 16 | 5 | 0 |

The standard record was re-run by A5 part 3 (2026-10-23, from `a9401a6`) with the
velocity-field cases (the `exponential`, `bspline-exponential`, `time-varying` and
`time-varying-bspline` categories: every metric, masks, sampling, 2D and float, the outputs and
the collapse, initial transforms, every message and failure; `corrections`: the paths of the
corrections decided on 2026-10-21, compared in `reproduce` mode; `real-exponential-small`,
`real-bspline-exponential-small`, `real-time-varying-small` and
`real-time-varying-bspline-small`: sMRIPrep's template registration with each velocity-field
stage in place of its SyN stage, on every third voxel): every one of the 102 new cases agrees
(81 compared: 73 identical to the unmodified ANTs, 8 with CORE-MATH, the four real ones among
them, through glibc's `log` in their Mattes stages; 16 rejected by both; 5 expected divergences:
three samplings ANTs seeds from the clock, ANTS-82; an affine stage after a velocity field,
ANTS-72; a mesh size of 0, ANTS-77); and `syn/exponential`, an expected divergence while
larmorx refused the stage, is now compared (identical to the unmodified ANTs). Its fix pass shows
each correction of this part. The full tier's new cases (4D, the other four oracle runs on every
third voxel, the exponential stages at full size) ran separately, beside it
([record](../validation/ants-registration-velocity-full.md)): all 124 agree, and every real case
(each velocity-field stage on the five oracle runs on every third voxel, `Exponential` and
`BSplineExponential` on ds000005 at full size) is identical to ANTs with CORE-MATH; the
other full record is A5 part 2's run. What the corrections change is measured in
[ants-registration-corrections](../validation/ants-registration-corrections.md) (19 cases).
The standard record was re-run by A5 part 2 (2026-10-20, from `9a4d65a`) with the B-spline and
displacement-field cases (the `bspline-syn`, `displacement-field` and `bspline` categories: every
metric, masks, sampling, 2D and float, the outputs and the collapse, B-spline transforms among the
initial transforms, every message and failure; `real-bsplinesyn-small`: sMRIPrep's template
registration with antsRegistrationSyN.sh's `BSplineSyN[0.1,26,0,3]` for its SyN stage, on every
third voxel, identical to ANTs with CORE-MATH); every one of the 106 new cases agrees (84
compared: 80 identical to the unmodified ANTs, 4 with CORE-MATH (glibc's `log` and `exp`, which
the Mattes and joint-histogram metrics call); 20 rejected by both; 2 expected divergences, ANTS-77). The full record was re-run from `fc87e0e` (2026-10-20 and 21): it adds 4D cases of
the four stages, B-spline SyN on the template registration of the other four oracle runs on
every third voxel and of the first run at full size, all identical to ANTs with CORE-MATH
(their Mattes stages call glibc's `log`; the full-size case ran 7.3 hours in the harness). Both records were re-run by A5 part 1 (2026-10-19 and 20, from `a3c6cd6`), with the SyN cases (the
`syn` and `syn-point-sets` categories: every metric, masks, outputs, initial fields, states,
messages and failures; `real-syn-small` and, in the full tier, `real-syn`: sMRIPrep's brain
extraction and template registration whole, Rigid, Affine and SyN, as fMRIPrep 25.2.6 ran
them: all 20, small and full size, identical to ANTs with CORE-MATH, as their Mattes and MI
stages call glibc's `log`). A4 part 4 (2026-10-18, from `8bac0bf`) had added the point-set
metrics' cases and A4 part 3 two cases of the gradient filter on oblique grids
([findings](../findings/ants-ai.md)).

The cases cover every transform in 2D, 3D and 4D, double and float, every metric and sampling,
masks, the initial transforms and their files, every output and interpolator, every
command-line check and message, fMRIPrep 25.2.6's own command lines of four oracle nodes, and
(full tier) the linear stages of sMRIPrep's brain extraction and template registration in all
five oracle runs, and (A4 part 4) the point-set metrics: each metric alone and with the
others and image metrics, label images (with boundary points), text and VTK point sets
(ASCII, binary, without scalars), in-memory points, 2D, 3D and 4D, double and float, initial
transforms, several levels and stages, and every message, failure and crash; and (A5 part 1)
SyN stages with each metric, several metrics, masks, sampling, 2D, 3D and 4D, double and float,
the collapse and every output, displacement fields among the initial transforms, saved and
restored states, the point-set metrics in tangent space, and sMRIPrep's two SyN registrations.
The expected divergences: no seed (ANTs from the clock), ANTS-51's crash, `--version`, three of
ANTS-45's silent failures, MINC transforms (refused), a point set's random fraction (ANTs seeds
it from the clock, ANTS-64), IGDM's value for neighbourhoods of more than one voxel (ANTs reads
before its arrays, ANTS-66), a text point set ANTs reads forever (ANTS-69), four linear stages
after a displacement field (ANTs seeds their voxel sample from the clock, ANTS-72); A5 part 2
added ANTS-77's two B-spline meshes (a size of 0, fewer sizes than dimensions: ANTs fails on
garbage, larmorx refuses); A5 part 3 added three samplings of the velocity-field stages (ANTs
seeds them from the clock, ANTS-82), an affine stage after a velocity field (ANTS-72) and a
time-varying B-spline mesh of 0 (ANTS-77). The velocity-field stages, an expected divergence
while larmorx refused them (`syn/exponential`), are compared since A5 part 3.

**ANTs' own thread band** (the standard record's 36 cases, 2, 4 and 8 threads against one; A5
part 3 added the four velocity-field stages: 0 to 1.9e-6 on the phantoms; on sMRIPrep's
registration 0.28 mm with the exponential stage and 0.96 mm with the time-varying field (0.18 and
0.40 mm in the run before: the band changes from run to run), where larmorx lies within 0.11 and
0.27 mm of ANTs at one thread as built, and is identical with CORE-MATH; earlier parts' 24 cases:
about 1e-13 to 1e-12 in a transform parameter in double, 3.9e-10 on fMRIPrep's BOLD-reference
affine (3.5e-9 in A4 part 2's run: the band itself changes from run to run), 4.9e-3 on a float
affine and up to 1.1e-16 with the point-set metrics; the B-spline stages (A5 part 2): 8.9e-16 for
a `BSpline` transform, 6.7e-7 to 2.3e-6 mm for B-spline SyN and the B-spline displacement field
(ITK's B-spline fit adds each work unit's lattice), 1.18 mm on sMRIPrep's template registration
with B-spline SyN on every third voxel; SyN (A5 part 1): 0 to 3.6e-14 mm in a
field, 0.023 to 0.025 mm with ICP (ANTs adds the points' derivatives into the field from several threads
without synchronisation: two runs at 4 threads differ, ITK-34), and 1.4 to 1.7 mm on sMRIPrep's
registrations on every third voxel (the linear stages' thread dependence carried through SyN;
glibc's maths moves the fields as far). larmorx gives the one-thread result at any thread count.

**Results do not depend on threads.** ANTs' do (its metrics sum per work unit); larmorx always
gives ANTs' one-thread result. **Without a seed ANTs is not repeatable** (its sampling is seeded
from the clock); larmorx then uses a fixed seed (121212).

**On the data panel** ([record](../validation/ants-registration-panel.md), 2026-10-27): every
antsRegistration node of the fourteen finished fMRIPrep oracle runs, each replayed from its node
with its inputs and fMRIPrep's seed (the BOLD references' registrations to the template, the
field maps' coregistrations, sMRIPrep's brain extraction at full size: 3.5-year-old, glioma,
stroke, strong motion, 1.5 T, 7 T, multiband-8 and multi-echo among them), and the template
registrations on every third voxel: **71 of 71 cases identical to ANTs 2.6.5 at one thread, 16 as
built and 55 more with CORE-MATH** (warped images, transforms, standard output). The template
registration's inputs, which niworkflows writes for itself and nipype does not keep, are made
again exactly from each node's recorded inputs; its fourteen whole-size runs (about 2.4 hours of
ANTs at one thread each, as built and with CORE-MATH) are left for a larger machine. sMRIPrep's
brain extraction of the 7 T head at full size: ANTs 2884 MiB and 2431 s at one thread, larmorx
2394 MiB and 1165 s, its four files identical.

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
| `-t Rigid[step]` | `RegistrationStage(transform="Rigid", gradient_step=step)` | `Translation`, `Rigid`, `Similarity`, `Affine`, `CompositeAffine` |
| `-t SyN[step,update,total]` | `RegistrationStage(transform="SyN", gradient_step=step, update_field_variance=update, total_field_variance=total)` | variances in voxels², default 3 and 0 |
| `-t BSplineSyN[step,updateMesh,totalMesh,order]` | `RegistrationStage(transform="BSplineSyN", gradient_step=step, mesh_size=updateMesh, total_mesh_size=totalMesh, spline_order=order)` | meshes per axis at the base level (`(2, 2, 2)`, `2x2x2`), or one knot spacing in mm (`26`); total mesh default 0 (no smoothing), order 3 |
| `-t GaussianDisplacementField[step,update,total]` | `RegistrationStage(transform="GaussianDisplacementField", gradient_step=step, update_field_variance=update, total_field_variance=total)` | both variances required on the command line |
| `-t BSplineDisplacementField[step,updateMesh,totalMesh,order]` | `RegistrationStage(transform="BSplineDisplacementField", gradient_step=step, mesh_size=, total_mesh_size=, spline_order=)` | the meshes at the base level, doubled at each level (ANTs doubles them already at the base level, ANTS-78: give twice the mesh, or `reproduce`, for its result) |
| `-t BSpline[step,mesh]` | `RegistrationStage(transform="BSpline", gradient_step=step, mesh_size=mesh)` | a cubic B-spline transform; the mesh (or knot spacing) at the base level, doubled at each level |
| `-t Exponential[step,update,velocity,steps]` | `RegistrationStage(transform="Exponential", gradient_step=step, update_field_variance=update, velocity_field_variance=velocity, integration_steps=steps)` | `Exp` is the short name; `steps` 0 or left out (`None`): automatic; the sigmas `-s` in the units given (ANTs: mm, ANTS-80) |
| `-t BSplineExponential[step,updateMesh,velocityMesh,steps,order]` | `RegistrationStage(transform="BSplineExponential", gradient_step=step, mesh_size=updateMesh, total_mesh_size=velocityMesh, integration_steps=steps, spline_order=order)` | meshes or knot spacings, at the base level (ANTS-78) |
| `-t TimeVaryingVelocityField[step,timeIndices,update,updateTime,total,totalTime]` | `RegistrationStage(transform="TimeVaryingVelocityField", gradient_step=step, number_of_time_indices=, update_field_variance=, update_field_time_variance=, total_field_variance=, total_field_time_variance=)` | `tvf` is the short name; all six parameters required |
| `-t TimeVaryingBSplineVelocityField[step,mesh,samples,order]` | `RegistrationStage(transform="TimeVaryingBSplineVelocityField", gradient_step=step, mesh_size=mesh, number_of_time_point_samples=samples, spline_order=order)` | the mesh has one size per axis and one for time (`4x4x4x2`) |
| `-m ICP[f,m,w,p,boundary]` | `PointSetMetric("ICP", f, m, weight=w, sampling_percentage=p, boundary_points_only=boundary)` | `f`, `m`: a label image, a `.txt` or `.vtk` point set, or `(points, labels)` arrays |
| `-m PSE[f,m,w,p,boundary,sigma,k]` | `PointSetMetric("PSE", ..., sigma=sigma, k_neighborhood=k)` | |
| `-m JHCT[f,m,w,p,boundary,sigma,k,alpha,aniso]` | `PointSetMetric("JHCT", ..., sigma=, k_neighborhood=, alpha=, anisotropic_covariances=)` | |
| `-m IGDM[f,m,w,fmask,mmask,radius,isigma,dsigma,k,gsigma]` | `IntensityPointSetMetric(f, m, fmask, mmask, weight=, neighborhood_radius=, intensity_sigma=, distance_sigma=, k_neighborhood=, gradient_sigma=)` | sigmas of 0 are estimated (the distance sigma at least one voxel, ANTS-70); the `k` nearest points and Gaussian gradients of `gsigma` mm (ANTs uses neither, ANTS-67) |
| `-m Mattes[f,m,w,bins,sampling,p,gf]` | `RegistrationMetric("Mattes", f, m, weight=w, bins=bins, sampling=..., sampling_percentage=p, use_gradient_filter=gf)` | `MI` is ANTs' other name for Mattes; `MI2` the joint-histogram MI; `CC[f,m,w,radius,...]`: `radius=`; `MeanSquares`, `GC`, `Demons` have no parameter |
| several `-m` in a stage | `metrics=[...]` | the first metric's value; derivatives weighed by `weight` (normalised) |
| `-c [10x5,1e-6,10]` | `iterations=[10, 5]`, `convergence_threshold=1e-6`, `convergence_window=10` | a level stops when the window's convergence value is ≤ the threshold (negative when the metric rises: [findings](../findings/ants-registration.md)) |
| `-f 4x2x1` | `shrink_factors=[4, 2, 1]` | per level; per axis as `CalculateShrinkFactorsPerDimension` chooses |
| `-s 2x1x0vox` / `mm` | `smoothing_sigmas=[2, 1, 0]`, `sigmas_in_mm=` | |
| `-x mask` / `-x [fixed,moving]` | `RegistrationStage.masks=mask` / `(fixed, moving)` | `None` in the pair is ANTs' `NULL` |
| `-g 1x1x0x1x1x1` | `restrict_deformation=[1, 1, 0, 1, 1, 1]` | weights of the transform's parameters |
| `-r file` / `-r [file,1]` / `-r [f,m,0/1/2]` | `initial_moving_transforms=[path or ItkTransforms, (t, True), AlignCenters(f, m, "geometry"/"moments"/"origins")]` | a file whose last extension is not a transform file's (`.nii.gz`, ...) is a displacement field image; a composite may hold fields; a field has no inverse (`[field,1]` fails) |
| `-q ...` | `initial_fixed_transforms=` | |
| `-k state` | `restore_state=` | |
| `-j state` | – | only on the command line (`--save-state`, needs `-a 1`): after a SyN stage, the composite before it and SyN's fixed-to-middle and moving-to-middle fields, each followed by its inverse; after linear stages every transform (ANTS-50, ANTS-74) |
| `-w [0.005,0.995]` | `winsorize=(0.005, 0.995)` | |
| `-u 1` | `use_histogram_matching=True` | |
| `-z 0/1` | `collapse_output_transforms=` | default 1 |
| `-a 1` | `write_composite_transform=True` | `Composite.h5`, `InverseComposite.h5` (fields stored forward; the inverse composite holds the inverse fields) |
| `-i 1` | `initialize_transforms_per_stage=True` | needs `-a 1` and `-z 0` |
| `-n Linear` | `interpolation="Linear"` | the warped images' interpolator, as `antsApplyTransforms -n`, in every dimension |
| `--float 1` | `precision="float32"` | ANTs' single-precision pipeline |
| `--random-seed N` / `ANTS_RANDOM_SEED` | `random_seed=N` | without one: ANTs from the clock, larmorx 121212 |
| `--print-similarity-measure-interval N` | `print_similarity_measure_interval=N` | `RegistrationIteration.full_scale_value` |
| `--write-interval-volumes N` | – | command line only |
| `-v 1` | `RegistrationResult.log`, `.iterations` | Python always runs verbose and parses the `DIAGNOSTIC` lines |
| `--minc 1` | – | MINC transforms: not supported yet |
| `ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS` | `n_threads` | the result does not depend on it |
| `LARMORX_UPSTREAM_DEFECTS` | `upstream_defects=` | ANTS-49, -50, -51, ITK-18; the point-set metrics' ANTS-61 to -63, -65 to -68, -70, ITK-22 to -24, -26, -28, -29; SyN's ANTS-71, -73, -74, ITK-30 to -33; the B-spline and field stages' ANTS-75, -76, -78, ITK-35, -42; the velocity-field stages' ANTS-79 to -81, ITK-36 to -41 ([upstream defects](upstream-defects.md)) |

## SyN stages

Symmetric normalisation as ITK's `SyNImageRegistrationMethod` runs it under ANTs: two
displacement fields (fixed-to-middle and moving-to-middle) and their inverses, updated at each
iteration from the metric's derivative over the level's domain, smoothed with the update and
total variances; the output transform is the two fields composed (`Warp`), its inverse the
inverses composed (`InverseWarp`). What it computes, step by step, and the defects found:
[findings](../findings/ants-syn.md).

```python
import larmorx as lx

syn = lx.ants.RegistrationStage(
    "SyN", 0.1,
    lx.ants.RegistrationMetric("CC", "fixed.nii.gz", "moving.nii.gz", radius=4),
    iterations=[100, 70, 50, 20], shrink_factors=[8, 4, 2, 1], smoothing_sigmas=[3, 2, 1, 0],
    convergence_threshold=1e-6, convergence_window=10,
    update_field_variance=3.0, total_field_variance=0.0,
)
res = lx.ants.registration([stage1, stage2, syn], warped=True, random_seed=42)
res.files["1Warp.nii.gz"]            # [ItkTransform("DisplacementFieldTransform_double_3_3", ...)]
warped = lx.ants.apply_transforms("moving.nii.gz", "fixed.nii.gz", res.transforms)
```

```bash
larmorx ants antsRegistration -d 3 -o [out_,out_warped.nii.gz] ... \
    -t SyN[0.1,3,0] -m CC[fixed.nii.gz,moving.nii.gz,1,4] \
    -c [100x70x50x20,1e-6,10] -f 8x4x2x1 -s 3x2x1x0vox
# out_0GenericAffine.mat, out_1Warp.nii.gz, out_1InverseWarp.nii.gz
```

- **Metrics:** every image metric, `Demons` included, each with its derivative per voxel;
  several metrics combined as in a linear stage; masks resampled through the half-way
  transforms at each iteration; sampling (`Regular`, `Random`) of the first metric's fixed mask.
  The point-set metrics run in tangent space (both sets mapped into the middle), each point's
  derivative at its nearest voxel; IGDM cannot come first (ANTs: "Intensity point set metric is
  not implemented yet for the specified transform.").
- **Outputs:** collapsed (`-z 1`, the default), linear runs become `GenericAffine.mat` and runs
  of fields one `Warp.nii.gz`/`InverseWarp.nii.gz` pair (NIfTI vector images); not collapsed,
  each stage's field its own pair. `-a 1` writes the composites; `--save-state` SyN's state,
  which `--restore-state` (`restore_state=`) resumes from; `--initialize-transforms-per-stage`
  starts a SyN stage from the previous one's fields.
- **Results do not depend on threads**, in ANTs (within 4e-14 mm) or larmorx (exactly ANTs'
  one-thread fields).
- **Defaults that differ from ANTs** (D13; `upstream_defects="reproduce"` gives ANTs' result):
  ITK-30 (GC's derivative in one voxel), ITK-31 (a stale mask where a stage has only one of
  its two masks: sMRIPrep's brain extraction), ITK-32 and ITK-33 (point-set metrics frozen at
  the first evaluation, or never deformed among several metrics), ANTS-71 (the full-scale
  similarity's crashes), ANTS-73 (an image that is not a displacement field given as one:
  refused), ANTS-74 (a stale saved state), ANTS-61 (IGDM among other metrics). ANTS-72 (a
  linear stage after a field is not repeatable in ANTs) is not reproduced: larmorx draws its
  voxels with the seed. Found by random testing (2026-10-25): ITK-97 (a sampled metric with a
  shrink factor: a sample point outside the shrunk region has its derivative added to another
  voxel's parameters in ITK; dropped by default), ITK-99 (written past the derivative array:
  dropped in both modes), ITK-100 (Mattes at a later level whose analysis region has one
  intensity: ITK computes a metric of 0 where its first level refuses; an error by default).

## B-spline and displacement-field stages

Four more deformable stages (milestone A5 part 2), what they compute and the defects found:
[findings](../findings/ants-bspline.md).

- **`BSplineSyN`** is SyN with B-spline approximations instead of Gaussian smoothing: the update
  and total fields are fitted by B-splines of the given meshes (doubled at each level), weighted
  by the masks; with a point-set metric alone, each point's derivative is fitted at the point.
  antsRegistrationSyN.sh's `-t b` uses `BSplineSyN[0.1,26,0,3]` (a knot spacing of 26 mm).
  Outputs as SyN's: `Warp.nii.gz`, `InverseWarp.nii.gz`.
- **`GaussianDisplacementField`** and **`BSplineDisplacementField`** optimise one displacement
  field with the linear stages' optimizer (line search, estimated scales and learning rate),
  each update and the total field smoothed by a Gaussian or a B-spline. Outputs: one
  `Warp.nii.gz` without an inverse.
- **`BSpline`** optimises a cubic B-spline transform over the stage's first fixed image (a
  control-point mesh, doubled at each level, its coefficients resampled onto the finer grid).
  Output: `BSpline.txt` (ITK's text format; `BSplineTransform` in composites), no inverse.

```python
import larmorx as lx

bsyn = lx.ants.RegistrationStage(
    "BSplineSyN", 0.1,
    lx.ants.RegistrationMetric("CC", "fixed.nii.gz", "moving.nii.gz", radius=4),
    iterations=[100, 70, 50, 20], shrink_factors=[8, 4, 2, 1], smoothing_sigmas=[3, 2, 1, 0],
    mesh_size=26.0,              # a knot spacing in mm; or (4, 4, 4)
)
ffd = lx.ants.RegistrationStage(
    "BSpline", 0.2, lx.ants.RegistrationMetric("MeanSquares", "fixed.nii.gz", "moving.nii.gz"),
    iterations=[20, 10], shrink_factors=[2, 1], smoothing_sigmas=[1, 0], mesh_size=(4, 4, 4),
)
res = lx.ants.registration([stage1, stage2, bsyn], random_seed=42)
res.files["2Warp.nii.gz"]
```

- **Every image metric** and the point-set metrics (`Demons` needs a displacement field: a
  `BSpline` stage with it fails), masks, sampling, several metrics, 2D, 3D and 4D, double and
  float, `-g` (the dimension's weights for the fields; with `BSpline` they do not match its
  parameters and ITK throws, as in ANTs).
- **Stages after a B-spline transform** (a `BSpline` stage, or a B-spline initial transform)
  run: larmorx computes the B-spline transform's Jacobian with respect to position (ITK-35,
  corrected by default: ITK has none, so in ANTs a linear, displacement-field or `BSpline` stage
  after it fails at its first derivative, a linear one dropped, ANTS-49; `reproduce` does the
  same). Validated against finite differences and against the closest runs ANTs can do
  ([findings](../findings/ants-velocity.md)). Give such a stage a mask: the B-spline transform is
  the identity outside its valid region, so the composite jumps at that border where its
  coefficients are not 0.
- **Results do not depend on threads** in larmorx (ANTs' B-spline fits and the B-spline
  transform's metric sums depend on its thread count).
- **Fast:** larmorx computes a B-spline transform's derivative only at each point's control
  points (ITK computes and sums every parameter at every point), with the same sums bit for bit.
- **Defaults that differ from ANTs** (D13; `upstream_defects="reproduce"` gives ANTs' result):
  ANTS-75 (the full-scale similarity and interval volumes with a field or a B-spline transform),
  ANTS-76 (the collapse loses transforms around B-spline transforms), ANTS-78 (the
  `BSplineDisplacementField` mesh at the base level: ANTs doubles it there; twice the mesh gives
  ANTs' result byte for byte), ITK-35 (above), ITK-42 (a coarse B-spline mesh at a shrunken
  level: ITK refuses the equal, vanishing scales). A mesh size of 0, or fewer mesh sizes than
  dimensions, is refused (ANTS-77: ANTs divides by zero).
- **`antsApplyTransforms` applies `BSpline.txt`** (and B-spline transforms in `.mat`, `.h5` and
  composites), bit for bit as ANTs ([antsApplyTransforms](ants-apply-transforms.md)).

## Velocity-field stages

Four more deformable stages (milestone A5 part 3), what they compute and the defects found:
[findings](../findings/ants-velocity.md).

- **`Exponential`** optimises a stationary velocity field `v` whose displacement is `exp(v)`
  (scaling and squaring), and its inverse `exp(−v)`: each update and the velocity field smoothed
  by Gaussians, plain gradient descent at the gradient step (no scales estimator).
  **`BSplineExponential`** smooths them by B-spline approximations instead. Outputs: `Warp.nii.gz`,
  `InverseWarp.nii.gz`; with `-z 0`, `Exponential` also writes `VelocityField.nii.gz`.
- **`TimeVaryingVelocityField`** optimises a velocity field over space and time (`timeIndices`
  time points), its displacement the flow from time 0 to 1 integrated by fourth-order
  Runge–Kutta, its inverse the flow back; each iteration evaluates the metric at every time
  point. **`TimeVaryingBSplineVelocityField`** holds the velocity as a B-spline over space and
  time (the mesh has a size for time too) and fits each iteration's derivatives by a B-spline.
  Outputs: `Warp.nii.gz`, `InverseWarp.nii.gz`; with `-z 0` the time-varying field writes
  `VelocityField.nii.gz` (a `D + 1`-dimensional vector image; not in 4D, which NIfTI cannot hold).

```python
import larmorx as lx

cc = lx.ants.RegistrationMetric("CC", "fixed.nii.gz", "moving.nii.gz", radius=4)
levels = dict(iterations=[100, 70, 50, 20], shrink_factors=[8, 4, 2, 1], smoothing_sigmas=[3, 2, 1, 0])
exp = lx.ants.RegistrationStage("Exponential", 0.25, cc, **levels,
                                update_field_variance=3.0, velocity_field_variance=0.5)
tvf = lx.ants.RegistrationStage("TimeVaryingVelocityField", 0.5, cc, **levels,
                                number_of_time_indices=4, update_field_variance=3.0)
tvb = lx.ants.RegistrationStage("TimeVaryingBSplineVelocityField", 0.5, cc, **levels,
                                mesh_size=(4, 4, 4, 2), number_of_time_point_samples=4)
res = lx.ants.registration([stage1, stage2, exp], random_seed=42)
res.files["2Warp.nii.gz"], res.files["2InverseWarp.nii.gz"]
```

```bash
larmorx ants antsRegistration -d 3 -o out_ ... \
    -t Exponential[0.25,3,0.5] -m CC[fixed.nii.gz,moving.nii.gz,1,4] \
    -c [100x70x50x20,1e-6,10] -f 8x4x2x1 -s 3x2x1x0vox
larmorx ants antsRegistration -d 3 -o out_ -z 0 \
    -t TimeVaryingVelocityField[0.5,4,3,0,0,0] -m CC[fixed.nii.gz,moving.nii.gz,1,4] \
    -c [100x70x50x20,1e-6,10] -f 8x4x2x1 -s 3x2x1x0vox
# out_0Warp.nii.gz, out_0InverseWarp.nii.gz, out_0VelocityField.nii.gz
```

- **Every image metric and the point-set metrics** (a point-set metric cannot come first: ANTs
  allocates the field on its null image and crashes), masks, sampling, several metrics, 2D, 3D
  and 4D, double and float, `-g`, initial transforms, collapse and composites,
  `--initialize-transforms-per-stage`.
- **Results do not depend on threads**, and larmorx's time-varying stages run several times faster
  than ANTs' (each voxel's integration on its own thread).
- **Not repeatable in ANTs with sampling:** the `Exponential`, `BSplineExponential` and
  `TimeVaryingVelocityField` stages never seed their sampling (ANTS-82): ANTs samples from the
  clock; larmorx with the registration's seed.
- **Defaults that differ from ANTs** (D13; `upstream_defects="reproduce"` gives ANTs' result):
  ANTS-79 (the number of time indices and the time-varying B-spline mesh: each stage's own;
  ANTs reads the last `-t`'s), ANTS-80 (the exponential stages' sigmas in the units given; ANTs:
  mm), ANTS-81 (`Exp` writes its `InverseWarp`), ITK-36 to ITK-38 (the time-varying B-spline
  method's metric transforms, stale inverse fields, its point-set fit on oblique grids), ITK-39
  (`-q` applied by the time-varying method), ITK-40 (the masks moved with the images), ITK-41
  (a stage that never iterates is still integrated). ANTS-75 (`--print-similarity-measure-interval`
  and `--write-interval-volumes` in the exponential stages) and ANTS-77 (a time-varying B-spline
  mesh size of 0, refused) as for the B-spline stages.

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
  neighbourhood), ANTS-67 (IGDM's `kNeighborhood` and `gradientSigma` used), ANTS-68 (IGDM's
  derivative the value's gradient), ANTS-70 (IGDM's estimated distance sigma at least one voxel),
  ITK-22 (labels the moving set lacks), ITK-23 (an identity covariance's prefactor), ITK-24
  (JHCT's total number of points with the fixed points), ITK-26 (JHCT's covariance diagonal),
  ITK-28 (JHCT's `alpha` near 1), ITK-29 (the Parzen kernel sigma as a sigma). Each is named in a
  notice where it changes the result; their measured effects are in the
  [findings](../findings/ants-velocity.md#the-corrections-decided-on-2026-10-21-and-what-they-change).
- **Correctly rounded powers:** JHCT calls `pow`; larmorx's is CORE-MATH's (glibc's differs by
  an ulp in about 0.1 % of calls).

## Not supported yet

- MINC transform files: refused with a message.
- Transform files of classes larmorx does not build in 2D and 4D (ITK's `CenteredRigid2D`,
  `CenteredSimilarity2D`, `CenteredAffine`, `Scale`): refused. Files of the classes
  antsRegistration writes (`AffineTransform`, `MatrixOffsetTransformBase`, `Euler2DTransform`,
  `Rigid2DTransform`, `Similarity2DTransform`, `TranslationTransform`, `IdentityTransform`,
  `DisplacementFieldTransform`, `BSplineTransform`) are read in every dimension.
