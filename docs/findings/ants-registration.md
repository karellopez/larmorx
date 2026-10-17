# antsRegistration's linear stages (ANTs v2.6.5, ITK v5.4.5)

What `antsRegistration` and the ITK v4 registration framework behind it really compute in a
linear stage, found while porting them (milestone A4, part 2). File names are at the tags
`ANTs-v2.6.5` (`Examples/`) and `ITK-v5.4.5` (`Modules/`). The parity record is
[ants-registration](../validation/ants-registration.md), the API page
[ants-registration](../api/ants-registration.md). How ANTs parses its command line is in
[ants-cli.md](ants-cli.md); ITK's transform classes in [itk-transforms.md](itk-transforms.md).

## In short

- **Without a seed ANTs is not repeatable.** With no `--random-seed` and no
  `ANTS_RANDOM_SEED`, the metric's sampling (Regular sampling too: each point is moved by a
  normal variate) is seeded from the clock, so two runs differ. larmorx then uses a fixed seed
  (121212, ITK's Mersenne twister default). With a seed, ANTs at one thread repeats itself bit
  for bit.
- **ANTs' result depends on its thread count.** The metrics sum per work unit (the Mattes joint
  PDF and its derivative, the mean-squares measure, the correlation sums), so 2 to 8 threads
  move the final parameters by about 1e-13 to 1e-12 from one thread's (`reg/threads.py`,
  2026-10-16), and run to run as well. larmorx always computes ANTs' **single-thread** result.
- **glibc's `exp`, `log`, `sin`, `cos`, `logf`, `cosf` are not always correctly rounded**;
  larmorx's are (CLAUDE.md rule 5). That is the only difference left in the parity cases:
  wherever larmorx is not bit-identical to the unmodified ANTs, it is bit-identical to the same
  binary with CORE-MATH's functions preloaded.
- Four defects found: ITK-18 (MI2's swapped marginals), ANTS-49 (a failed stage dropped,
  exit 0), ANTS-50 (`--save-state` after linear stages), ANTS-51 (file names without
  collapsing, then a crash); and ANTS-46 (an "early stop") explained as ITK's design. All in the
  [register](upstream-defects.md).

- **The gradient filter's components are floats** (found by A4 part 3, 2026-10-17):
  `GradientRecursiveGaussianImageFilter` stores each derivative through
  `NthElementImageAdaptor<_, float>`, so the quotient by the spacing is rounded to float before
  the direction cosines are applied in double ([ants-ai.md](ants-ai.md), "The moving image's
  gradient"). It shows only where a spacing is not a power of two; the oblique cases
  `metrics/*-gradient-filter-oblique` check it.

## The pipeline of one stage

`DoRegistration` (`itkantsRegistrationHelper.hxx`) builds an `ImageRegistrationMethodv4` per
stage (`AddLinearTransformToCompositeTransform`, `itkantsRegistrationHelper.h:1028-1090`). Per
level (`ImageRegistrationMethodv4::InitializeRegistrationAtEachLevel`, then
`m_Metric->Initialize()`, then `StartOptimization`):

1. **Shrinking.** The virtual domain is the first metric's fixed image shrunk by
   `ShrinkImageFilter`'s output geometry; the factors per axis come from
   `CalculateShrinkFactorsPerDimension` (the factor for the smallest spacing, the others chosen
   to match it).
2. **Smoothing.** `SmoothingRecursiveGaussianImageFilter<Image<R>, Image<R>>` with the sigma
   times each image's spacing (or in mm). *For double images the intermediate images are
   float*: `NumericTraits<double>::FloatType` is `float`, so the first pass (along the last
   axis) reads double and stores float, the next passes are float, and the result is cast back
   to double (`larmorx_image::gaussian::smoothing_recursive_gaussian_f64`).
3. **Sampling** (`SetMetricSamplePoints`, `itkImageRegistrationMethodv4.hxx:895-1050`): one set
   per metric, all from the *first* metric's virtual domain and fixed mask. The points are
   `Point<float>` (`PointSet`'s default coordinate type): the index's physical point is summed
   in float (each `+=` computed in double, then stored), the normal variate times a third of
   the spacing added the same way. Regular takes every `ceil(1/p)`-th voxel from the first;
   Random asks `ImageRandomConstIteratorWithIndex` for `ulong(float(N) · p)` voxels. Each set
   seeds its generator with the next value of `m_CurrentRandomSeed` (Random: the iterator with
   the one after), which `MetricSamplingReinitializeSeed(seed)` resets per stage.
4. **Metric initialisation**, then the optimizer.

The moving transform is a composite: the initial transforms (flattened), then the stage's
transform, whose centre starts at the last matrix-offset initial transform's
(`InitializeCenterOfLinearOutputTransform`, `itkImageRegistrationMethodv4.hxx:1057-1098`).

## The optimizer (`ConjugateGradientLineSearchOptimizerv4`)

As ANTs sets it: lower limit 0, upper 2, epsilon 0.2, the golden-section search's φ the literal
`1.618034`, at most 20 line-search steps, the learning rate estimated once per level from the
scales (`RegistrationParameterScalesFromPhysicalShift`, corner samples, a shift of each
parameter through `UpdateTransformParameters`) and the maximum step (the smallest virtual
spacing). Polak-Ribière's γ, reset to 0 below 0 or above 5. The best parameters are recorded
after each step (`ReturnBestParametersAndValue`) and restored at the end. The convergence test
comes before the step, the iteration event after it.

**The window convergence value is signed** (`WindowConvergenceMonitoringFunction`): the
negative slope of a degree-1 B-spline fitted to the last `window` metric values (float
coordinates), normalised by the sum of their absolute values since the start. It is negative
when the metric rose over the window, and the test `value ≤ threshold` then stops the level:
a registration stops when it stops improving. That is fMRIPrep's "early stop" of ANTS-46.

**ANTs' observer** (`antsRegistrationOptimizerCommandIterationUpdate.h`) sets the level's
iteration count only at the level's first iteration: a level of 0 iterations after the first
runs one, and a first level of 0 runs none (nor do the next). After its first `DIAGNOSTIC`
line `std::cout` stays in `std::scientific` with precision 4, which changes how later numbers
print (`smoothing sigmas` are an `itk::Array`, printed by `ConvertNumberToString` and so not
affected).

## The metrics

| ANTs name | ITK class | Notes |
|---|---|---|
| `Mattes`, `MI` | `MattesMutualInformationImageToImageMetricv4` | the PDF sums in `R`, the joint PDF's total by `CompensatedSummation`; a global transform's derivative through per-bin PDF derivatives |
| `MI2` | `JointHistogramMutualInformationImageToImageMetricv4` | the joint histogram smoothed by `DiscreteGaussianImageFilter` (variance 1, ANTs' setting); **the marginals are swapped (ITK-18)**; `GetValue` with no valid point returns the previous value |
| `MeanSquares` | `MeanSquaresImageToImageMetricv4` | the base threader's compensated derivative sums |
| `GC` | `CorrelationImageToImageMetricv4` | two passes (the means, then the sums); the value `−fm²/(f2·m2)` in double |
| `CC` | `ANTSNeighborhoodCorrelationImageToImageMetricv4` | per-column window sums kept in a queue, recomputed at each line start (and for each sampled point, around its rounded index); the derivative factor in double |
| `Demons` | `DemonsImageToImageMetricv4` | needs a displacement field: `Initialize` throws in a linear stage (ANTS-49) |

Several metrics in one stage (`ObjectToObjectMultiMetricv4`): the value is the **first**
metric's; the derivative is `Σ w_j · d_j / |d_j|` times the mean of the `|d_j|`. The weights
are normalised to sum 1 **at every level's `Initialize`**, so from the second level they are
normalised again (they may move by an ulp).

`useGradientFilter` (the metric's 7th parameter): the moving image's gradient comes from
`GradientRecursiveGaussianImageFilter` (sigma the largest spacing, normalised across scale,
float intermediate images, the vectors turned by the image's direction) interpolated linearly,
instead of central differences.

## Initial transforms

`-r [fixed,moving,feature]` builds an `Euler3DTransform` (2D: `Euler2DTransform`, 4D:
`AffineTransform`) with `CenteredTransformInitializer`: 0 the geometric centres (the centre
continuous index converted in `R`), 1 the centres of mass (sums in double), 2 the origins.
Without collapsing, ANTs writes it as `<prefix><n>DerivedInitialMovingTranslation.mat`.

`--initialize-transforms-per-stage` (it needs `-a 1` and no collapsing,
`antsRegistration.cxx:634`): each linear stage starts from the composite's last transform
where ANTs knows how (`InitializeWithPreviousLinearTransform`, `itkantsRegistrationHelper.hxx:4075`:
translation to translation, rigid or affine; rigid to rigid or affine; affine to affine), and
that transform leaves the composite; otherwise it prints "WARNING: Initialization Failed".

`--restore-state`: the state's transforms become the initial transforms (unless its last four
are SyN's displacement fields). `--save-state`: see ANTS-50.

**The initial transforms are clones** (`SetMovingInitialTransform`, `SetFixedInitialTransform`,
`SetRestoreStateTransform`, `itkantsRegistrationHelper.hxx:3559-3700`): `CompositeTransform::Clone`
clones each transform through `Transform::InternalClone` (`itkTransform.hxx:49-63`), which
calls `SetFixedParameters` then `SetParameters`. That recomputes what the parameters define:
an inverted transform (`-r [file,1]`), whose offset `GetInverse` set directly as `−M⁻¹·offset`,
gets the offset `ComputeOffset` derives from its translation and centre, which differs in the
last bits when the matrix rotates and the centre is not 0. larmorx clones the same way
(`Xfm::itk_clone`).

The warped images (`GetWarpedImage`, `GetInverseWarpedImage`, `itkantsRegistrationHelper.hxx:720-763`)
use the moving composite alone (and its inverse), not the fixed initial transform; each linear
transform maps with its matrix and offset as they are. `ResampleImageFilter` takes its
scan-line path (`LinearThreadedGenerateData`, `itkResampleImageFilter.hxx:432-510`): per output
line, the continuous input indices of its first voxel and of the voxel one past its end (the
output point in double, the transform in `R`, the index in `R`), and each voxel at
`start + α·(end − start)`, `α = (i − first)/size` in double, stored in `R`
(`larmorx_registration::resample`, used in 2D and 4D; 3D goes through `antsApplyTransforms`'
resampler, which computes the same). Its interpolators are ITK's, templated on the dimension:
the B-spline, Gaussian, label-Gaussian and windowed-sinc ones in `larmorx_interp::nd` (the 3D
code with every per-axis step done for `D` axes, the neighbourhood visited with x fastest; in
3D bit-identical to the 3D versions), the linear and generic-label ones with ITK's linear
interpolator for the dimension (its optimised path in 2D, the general one in 4D).

**The 2D rotations invert to their own class.** `Rigid2DTransform`, `Euler2DTransform` and
`Similarity2DTransform` override `GetInverse` (`itkRigid2DTransform.hxx:137-150`,
`itkEuler2DTransform.hxx:48-61`, `itkSimilarity2DTransform.hxx:237-256`): the same centre, the
angle negated (the scale `1.0/scale`), the translation `−(M⁻¹·t)`, and the matrix recomputed
from the angle with `cos`/`sin`. So a 2D `-r [rigid.mat,1]`, or the inverse-warped image of a 2D
rigid registration, uses that transform, not the inverted matrix that every 3D class and
`AffineTransform` give (which differs in the last bits).

**Transform files are read in the registration's dimension.** ITK's reader creates each stored
transform through its factory, by the class name with its precision corrected to the reader's
(`CorrectTransformPrecisionType`); a class the factory does not know (ANTs' own
`ANTSCenteredAffine2DTransform`, which `CompositeAffine` writes with `-z 0` in 2D, and
`ANTSAffine3DTransform` in 3D) is ITK's "Could not create an instance" exception, whose list of
known classes includes the `MatrixOffsetTransformBase` antsRegistration registers for its own
dimension (`itkantsRegistrationHelper.h:1388-1396`). A transform of another dimension is read,
then `dynamic_cast` to the registration's transform type gives a null pointer
(`itkantsReadWriteTransform.h`): only "Can't read initial transform" (with `-v`), exit 1.

## Outputs

Collapsed (`-z 1`, the default): `CollapseLinearTransforms` composes every linear transform into
one `AffineTransform`, centred where the last matrix-offset transform of the queue is, each
composed with `Compose(·, pre = true)` (a `TranslationTransform` through its offset; an
`IdentityTransform` dereferences a null pointer, ANTS-48). `-a 1` writes `Composite.h5` and
`InverseComposite.h5`; otherwise one file per transform (ANTS-51 for their names).
