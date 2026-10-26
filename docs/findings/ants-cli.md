# ANTs command lines (ANTs v2.6.5)

## The argument parser (`Utilities/antsCommandLineParser.cxx`, `antsCommandLineOption.cxx`)

**Repeated options come back last first.** `AddFunction` pushes each value to the *front*
of the option's list (`antsCommandLineOption.cxx:42,64`). `GetFunction(0)` is therefore the
last occurrence on the command line, and that is what every single-valued option reads.
*Read.*
*larmorx:* `cli::parser` keeps values in command-line order. `Parsed::last()` is
`GetFunction(0)`.

**Brackets are rewritten only at the ends of a word.** `RegroupCommandLineArguments`
(`antsCommandLineParser.cxx:201`) replaces `{ ( <` only as a word's first character and
`} ) >` only as its last. So `Gaussian(1x2x3,2)` becomes `Gaussian(1x2x3,2]`, which ANTs
rejects ("Incorrect command line specification"). `[a.mat, 1]` split across two words is
joined back. A word with text after its first `]` is an error. *Read.*
*larmorx:* reproduced, including the errors (`parser.rs` tests).

**Short options take two characters.** `-x...` gives the name `argument.substr(1, 2)`, so
`-tf` is looked up as the long option `tf`. *Read.* *larmorx:* reproduced.

**Negative numbers are values, not options.** A following word ends an option's values when
it starts with `-` and `atof(word) == 0`. So `-f -0.5` passes `-0.5` to `-f`, while `-0`
would end it. An option with no value gets `"1"` (`-v` alone means verbose on).
*Read.* *larmorx:* reproduced (`parser::atof` mirrors C `atof`).

**`atof` is glibc's `strtod`:** besides decimals it reads `inf`, `infinity`, `nan` (any
case, with a sign) and hexadecimal floats (`0x40` is 64, `0x1.8p1` is 3). ThresholdImage
reads its bounds this way. *Read; validated* (`range/nan-lower`, `range/hex`).
*larmorx:* `parser::atof` reads them too (since 2026-10-10).

**Values are `name[p1,p2,...]`.** The parameters are split at every comma up to the first
`]`, without nesting. *Read.*

**`ConvertVector` splits on `x`** (`antsCommandLineParser.h:142`): `1x2x3` gives
`[1, 2, 3]`. `Convert<bool>` reads an integer, so `true` is not true. *Read.*

## `antsApplyTransforms` (`Examples/antsApplyTransforms.cxx`)

**The `-t` order.** For points of the output grid, the *first* `-t` acts first:
`-t A -t B` maps an output point `x` to `B(A(x))`. Seen as moving an image, the order
reverses: the image moves by `B` first, then `A`. That is the "last transform is applied
first" of the ANTs documentation, and why `-t warp.nii.gz -t affine.mat` is the usual order.
The mechanism: the parser returns the options last first, `GetCompositeTransformFromParserOption`
(`itkantsRegistrationHelper.h:1281-1440`) adds them to an ITK `CompositeTransform` in that
order, and the composite applies its queue back to front.
*Verified:* with `-t scale2.mat -t translate+1.mat` on a ramp image, output voxel 3 reads
input position 7 = 2·3 + 1.
*larmorx:* `TransformChain` stores the same queue. An earlier draft had the order reversed;
the oracle run caught it.

**Composite files keep their own order.** A `CompositeTransform` read from a file (`.h5`) is
expanded in place, component by component, in the file's queue order
(`itkantsRegistrationHelper.h:1438-1450`). *Read.* *larmorx:* `TransformChain::new`
flattens the same way, and a unit test pins the resulting mapping.

**Inverting** (`-t [file,1]`) calls `GetInverseTransform()` (`itkantsRegistrationHelper.h:1427`). For a displacement field
without an inverse field that returns null, and the run fails with "Inverse does not exist".
*Read.* *larmorx:* `TransformError::NotInvertible`.

**`-t identity`** (or `Identity`) adds an identity `MatrixOffsetTransformBase`. With no
`-t` at all, ANTs adds one too. *Read.* *larmorx:* an empty chain, which maps points
exactly the same way.

**Initializer transforms** `-t [fixed,moving,feature]` (three or more parameters) build a
rigid transform from the images. *Read.* *larmorx:* not supported yet (error).

**Pixel type.** Everything is computed in `double`, or in `float` with `--float 1`. The
input is read straight into that type, so integer data are converted on reading. The output
type is the computation type unless `-u` says otherwise. *Read.*
*larmorx:* the same two pipelines (since 2026-10-11): `--float 1` computes in single
precision exactly as ITK's float instantiation does, transforms and interpolators included
([ants-float.md](ants-float.md)). *Correction:* before, larmorx rounded the input to float32,
computed in double and wrote float32, which differed from ANTs in the last float bits.

**`-u` casts with `CastImageFilter`, a plain `static_cast`.** Integers truncate toward zero.
An unknown `-u` name silently falls back to the default type
(`antsApplyTransforms.cxx:1453-1600`). *Read.*

**Out-of-range values wrap on x86-64.** C++ leaves the cast of an out-of-range double to a
small integer undefined. x86-64 builds convert through a 32-bit integer (`cvttsd2si`: NaN and
values beyond ±2³¹ give `INT_MIN`) and keep the low bits. So `-u uchar` turns 400.7 into 144
and −3.7 into 253, and `-u char` wraps too. ARM builds (`fcvtzs`) saturate instead, so ANTs
gives different results on different hardware. *Verified* on x86-64: an `-u uchar` and an
`-u char` output both match the int32 wrap exactly, and neither matches saturation.
*larmorx:* the CLI reproduces x86-64 on every platform. `lx.ants.apply_transforms(dtype=...)`
saturates instead, which is safer, and says so.

**`-e 0` with a 4D file fails.** v2.6.5 (`antsApplyTransforms.cxx:355-375`) checks the file's number of dimensions against
`-d` and exits ("Input image dimension does not match"). It also rejects non-scalar pixel
types. *Read.* *larmorx:* same errors.

**`-e 3` (time series).** The 4D image is cut into 3D volumes with
`ExtractImageFilter::SetDirectionCollapseToSubmatrix` (`antsApplyTransforms.cxx:526`), so each volume takes the top-left 3×3
of the 4D direction. Each volume is resampled with the same transforms. The output takes
its first three axes from the reference and the fourth axis from the input (spacing, origin
and the rest via `CopyInformation`). `--time-index t` extracts a single volume instead and
writes a 3D image. *Read.*
*larmorx:* the same. The continuous indices are computed once and reused for every volume,
which gives the same numbers ITK computes per volume.

**Gaussian and MultiLabel sigmas default to the input image's spacing**
(`make_interpolator_snip.tmpl`). A sigma parameter of `s` applies to every axis; `sxsxs`
gives one per axis. **MultiLabel ignores its alpha parameter.** Alpha is always 4.0: the
snippet has no branch for a second parameter (`make_interpolator_snip.tmpl:127`). **GenericLabel ignores its parameter.** It
always uses linear interpolation. Gaussian alpha defaults to 1.0. `BlackmanWindowedSinc` is
accepted although the help does not list it. *Read.* *larmorx:* reproduced.

**`-o` forms.**
- `file` or `[file,0]` writes the warped image.
- `[file,1]` or `DisplacementField[file]` writes the composed displacement field.
- `Linear[file,<invert>]` writes the collapsed affine.
- `CompositeTransform[file]` writes the composite.
- `[file]` with a single parameter gives an empty file name (`antsApplyTransforms.cxx:762-771`).

*Read.* *larmorx:* every form (the transform outputs since 2026-10-16, A4 part 2).

**The transform outputs** (`antsApplyTransforms.cxx:409-456, 551-748`; added to larmorx
2026-10-16; *read; verified* against the GCC 14.3 oracle, the `transform-output` cases of
[validation/ants-apply-transforms.md](../validation/ants-apply-transforms.md), every file
byte for byte or dataset for dataset):
- **Order of checks.** An input image, if any, is read first; then `Linear` and
  `CompositeTransform` need a parameter ("An output parameter is required with -o Linear or
  -o CompositeTransform, see usage") and need no reference image; every other output needs
  `-r` ("A reference image is required."); `-o [file,0]` without `-i` is "An input image is
  required."; then the chain is built, an identity added when it has no parameters ("WARNING:
  No transforms found, using identify transform", `antsApplyTransforms.cxx:559-568`), the
  interpolator set up, and **every input image resampled onto the reference** before the
  output section looks at `-o`: the warped image is computed and thrown away. Each message is
  printed only with `-v` (ANTS-45's pattern); larmorx prints it always.
- **Inverses keep or lose their class.** `-t [file,1]` calls `GetInverseTransform()`.
  `AffineTransform`, `TranslationTransform` and the 2-D Euler, rigid and similarity classes
  override it (`InvertTransform(*this)` with their own type), but the 3-D Euler, versor,
  similarity and rigid classes inherit `MatrixOffsetTransformBase::GetInverseTransform`, whose
  `InvertTransform(*this)` instantiates the base class (`itkTransform.h:597-603`,
  `itkMatrixOffsetTransformBase.hxx:463-467`). So the inverse of an `Euler3DTransform` is a
  `MatrixOffsetTransformBase` with `M⁻¹` (vnl's SVD inverse), offset `−(M⁻¹·offset)`, the
  translation `ComputeTranslation` derives, and the original's fixed parameters: an
  `Euler3DTransform` always has four (`GetFixedParameters` appends its ZYX flag), so the
  inverse's are four values too (`0 1 -2 1`). *larmorx:* `larmorx_transform::itk_object`.
- **`CollapseLinearTransforms`** (`itkantsRegistrationHelper.hxx:3746-3799`): a new
  `AffineTransform` centred on the last transform of the queue that is a
  `MatrixOffsetTransformBase` (a `TranslationTransform` is not), composed (`Compose(nth,
  true)`: `offset = M·nth.offset + offset`, `M = M·nth.M`, then `ComputeTranslation`) with an
  `AffineTransform` made of each transform in queue order: a translation by `SetOffset`, any
  other by `SetCenter`, `SetMatrix`, `SetTranslation`. An `IdentityTransform` (read from a
  file) fails the `dynamic_cast` and is dereferenced as a null pointer: ANTs crashes (ANTS-48).
  `Linear[file,1]` writes a new `AffineTransform` with `SetMatrix(M⁻¹)` and
  `SetOffset(−(M⁻¹·offset))`: centre 0, unlike `GetInverse`.
- **Writers.** The text writer (`TxtTransformIO`) prints numbers with `itk::NumberToString`:
  double-conversion's ECMAScript shortest form (`1`, `0.1`, `1.5e-7`, `1e+21`), a float
  transform's parameters as floats (`ToShortestSingle`, `1.0510259`), fixed parameters as
  doubles; an empty list is `FixedParameters: ` with its blank. The MATLAB writer
  (`MatlabTransformIO`) does not expand composites: `CompositeTransform[x.mat]` is one
  variable named `CompositeTransform_double_3_3` holding `GetParameters()` (the components'
  parameters from the **last** to the first, `itkCompositeTransform.hxx`) and one `fixed`
  likewise; and it converts the fixed parameters to the transform's parameter type
  (`TempArray = GetFixedParameters()` into an `OptimizerParameters<float>`), so a float
  transform's `fixed` is written as single precision (type 10). HDF5 as A4 part 1 found
  ([itk-hdf5.md](itk-hdf5.md)).
- **`TransformToDisplacementFieldFilter`** (`itkTransformToDisplacementFieldFilter.hxx:150-271`):
  its `PointType` is the image's, `Point<double>`, whatever the transform's precision; each
  point is converted to `Point<float>` for a float transform and the result widened back, so
  `T(p) − p` is a double difference, stored as the field's component type. With every
  transform linear, each scan line is mapped at index 0 and at index `nx` (one past the end)
  and interpolated, `(1 − α)·start + α·end` with `α = x / nx` in double. The field is written
  by the transform writer when the last extension is exactly `.h5` or `.hdf5` (`.xfm` is
  MINC), else by `ImageFileWriter`: a NIfTI vector image (`dim[0] = 5`, `dim[4] = 1`,
  `dim[5] = 3`, intent `NIFTI_INTENT_VECTOR`, components as the slowest axis,
  `NiftiImageIO::WriteImageInformation`).
- **Crashes** (ANTS-48): an unknown extension (`TransformFileWriter` throws, uncaught: abort,
  exit 134), `-i` without `-r` (the resampler's reference is null), a Gaussian or MultiLabel
  interpolator without `-i` (`inputImages[0]` of an empty vector), an `IdentityTransform`
  collapsed. *larmorx:* refuses each, exit 1, in both D13 modes.

**Threads.** ITK uses all cores unless `ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS` is set.
*larmorx's CLI:* the same environment variable. The result does not depend on the thread
count, in ANTs or in larmorx.

*Correction (2026-10-11):* ITK reads more than that one variable
(`MultiThreaderBase::GetGlobalDefaultNumberOfThreads`, `itkMultiThreaderBase.cxx:266-330`,
ITK v5.4.5): the variables named in `ITK_NUMBER_OF_THREADS_ENV_LIST` (`:`-separated), or
`NSLOTS` when that is not set, then `ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS`; every one that is
set replaces the count with its `atoi` value, so the last one set wins; 0 or less means all
cores. nipype sets `NSLOTS` (fMRIPrep 25.2.6: `NSLOTS=1` for antsApplyTransforms and the other
image programs, 8 for DenoiseImage and the anatomical N4s; [fmriprep-oracle.md](fmriprep-oracle.md)).
*Read; verified* in fMRIPrep's working directory. *larmorx:* every `larmorx ants` program reads
them the same way (`larmorx_ants::cli::itk_threads_from`, tested); before this, a larmorx
program run by nipype would have used every core.

## larmorx's own threads: some loops ran on rayon's global pool (2026-10-26, fixed)

*Found during the data panel's re-validation (part a), verified, fixed in the same part.* larmorx
runs a call's parallel work in a pool of exactly the call's thread count
(`larmorx_core::parallel::with_threads`, threads named `larmorx-<n>-<i>`; CLAUDE.md rule 5,
explicit `n_threads`). Some loops were reached outside such a pool and so ran on rayon's global
pool, which has as many threads as the CPUs the process may use (or `RAYON_NUM_THREADS`), whatever
the call asked for. The results never depended on it (every reduction is fixed-order); only the
number of threads did.

- **How it showed:** the panel tier's antsAI runs (`ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS=3`, three
  parallel cases on ten CPUs) each had its idle `larmorx-3` pool and ten busy unnamed threads;
  the machine's load average reached 31 on 12 CPUs.
- **The probe:** `larmorx_validation.parity.thread_pools` runs every registered program over its
  distinct code paths at one thread (`ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS=1` for the ANTs
  programs, `OMP_NUM_THREADS=1` for the others) on small seeded inputs and lists every thread of
  its processes from `/proc/<pid>/task` every 10 ms; `validation/investigations/panel-ants/
  global_pool_probe.py` prints the table. 74 paths: 63 of the ANTs programs (every
  antsRegistration stage and metric, the point-set metrics, masks with an image-based initial
  transform and a composite output; antsApplyTransforms with a field, on a series, a field and
  label output; antsAI with masks, principal axes and blobs; antsAffineInitializer; Atropos'
  k-means, two channels, priors, Otsu with Parzen windows and a prior label image; N4 with
  weights; DenoiseImage's two models; twenty ImageMath operations; ThresholdImage,
  MultiplyImages, SmoothImage, ResampleImage(BySpacing)), the three AFNI clean-room originals and
  their GPL replicas, the clean-room head-motion correction and the FSL replicas (mcflirt, bet2,
  bet, betsurf).
- **Before the fix:** threads outside the pool in antsAffineInitializer, antsAI, the linear
  antsRegistration stages (every image metric), the BSpline and TimeVaryingBSplineVelocityField
  stages and Atropos (its k-means initialisation and its Parzen-window likelihoods, which reach
  larmorx-interp's B-spline coefficients); none in the AFNI, FSL and head-motion programs (the
  clean-room crates need no change), nor in N4, DenoiseImage, antsApplyTransforms, ImageMath and
  the other image programs.
- **The fix:** the image metric carries the call's thread count (`ImageMetricv4::n_threads`; its
  sampled values, `for_each_sample`, and the neighbourhood correlation run in that pool), and so
  do the level's problem (the update, the scales and the step's scale), the field and B-spline
  adaptors at each level, the TimeVaryingBSplineVelocityField stage's time samples, antsAI's and
  antsAffineInitializer's searches and the helper's full-scale CC; Atropos' public entry points
  (`run`, `posterior`, `likelihood`, `distance_prior`, `smooth_intensity`) and its k-means
  classification install the call's pool. After it, the probe finds no thread outside a
  `larmorx-<n>` pool on any of the 74 paths.
- **Tests:** `tests/parity/test_thread_pools.py` (every path, Linux) and
  `larmorx_registration::method::tests::metric_samples_run_in_the_calls_pool`; the results are
  unchanged (the golden suite's checksums and the thread-count invariance tests pass as before).
- **Records:** the panel records were made before the fix, with `RAYON_NUM_THREADS=3` (their
  larmorx thread count), so their larmorx runs used the call's count either way; their results are
  the same after the fix. Speeds measured before the fix at "one thread" may have used more
  threads in those programs (the BSpline stage's "20 times faster than ANTs at one thread" in the
  porting guide among them; re-measured below where it matters).
