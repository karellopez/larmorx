# antsApplyTransforms

`lx.ants.apply_transforms`, `lx.ants.apply_transforms_to_points`, `lx.ants.combine_transforms`
and `lx.transforms` (command line `larmorx ants antsApplyTransforms`; Rust crates `larmorx-ants`,
`larmorx-interp`, `larmorx-transform`, `larmorx-io`). Ported from ANTs 2.6.5 on ITK 5.4.5.

**Status: `validated`** against antsApplyTransforms itself (see the
[validation record](../validation/ants-apply-transforms.md)). The record covers 258 cases:
- every interpolator;
- every transform type and file format, `.h5` files written by ITK (linear, inverted, float,
  six types in one composite, a dense group of nine affines, a float field) and by larmorx
  included, and B-spline transforms (antsRegistration's `BSpline` stage output, A5 part 3: text,
  MATLAB and HDF5 files, float coefficients, inside composites and chains, the inverse refused);
- awkward geometries, output types and time series;
- real fMRIPrep derivatives;
- all of them again with `--float 1`, ANTs' single-precision pipeline;
- the transform outputs, every form and format, and the errors around them.

All 258 agree (20 of them errors both programs report; 4 expected divergences, where ANTs
crashes and larmorx refuses, ANTS-48). Of the 200 resampling cases compared, 185 are
byte-identical to the unmodified ANTs program, among them all 102 `--float 1` cases; the other
15 (two of them through fMRIPrep's `.h5` composite, one through a B-spline transform with
Lanczos) differ only where ITK calls the platform's `exp`, `log`, `sin` or `cos` (Gaussian and
windowed sinc in double), by at most about 1e-15 relative ([why](../findings/platform-math.md)).
With correctly rounded maths preloaded into ANTs, all 200 are byte-identical. The 40 cases of
B-spline transforms (A5 part 3: text, MATLAB and HDF5 files, float coefficients, composites,
chains, interpolators, time series, `--float 1`, the transform outputs, and the inverses both
programs refuse) all agree. The 46 transform-output cases (`-o Linear[...]`,
`-o CompositeTransform[...]`, `-o DisplacementField[...]`) write ANTs's files: text and MATLAB
byte for byte, NIfTI fields bit for bit, HDF5 dataset for dataset. All 14 of fMRIPrep's
`--float 1` outputs in the ds000005 oracle run are byte-identical to fMRIPrep's own
([stage record](../validation/fmriprep-stages.md)). Compared with ANTs' defects reproduced
(`LARMORX_UPSTREAM_DEFECTS=reproduce`); the record's "D13 fix pass" lists what larmorx's
default changes.

**On the data panel** ([record](../validation/ants-apply-transforms-panel.md), 2026-10-26):
every antsApplyTransforms node of the fourteen finished fMRIPrep oracle runs, each replayed from
its own command line with its inputs (421 calls: masks, references, tissue maps and
segmentations to the templates and the T1w, the confounds' masks, the reports' images; 179
through fMRIPrep's `.h5` composites, 166 with `--float 1`; `Gaussian`, `MultiLabel`,
`LanczosWindowedSinc`, `Linear` and `NearestNeighbor`), and four
calls fMRIPrep never makes (the 600-volume multiband-8 series to the T1w, the raw 7 T MP2RAGE to
MNI, the stroke's lesion mask to MNI, the 82-year-old's FLAIR onto its T1w): **425 of 425
bit-identical with CORE-MATH, 329 to the unmodified ANTs** (the others where ITK calls glibc's
`exp`, `log`, `sin` or `cos`).

## Quick start

```python
import larmorx as lx

# fMRIPrep's T1w → MNI (an .h5 composite: affine + warp), onto the 2 mm template grid
mni = lx.ants.apply_transforms(
    "sub-01_desc-preproc_T1w.nii.gz",
    "tpl-MNI152NLin2009cAsym_res-02_T1w.nii.gz",
    ["sub-01_from-T1w_to-MNI152NLin2009cAsym_mode-image_xfm.h5"],
    interpolation="lanczos",
    n_threads=8,
)
mni.save("sub-01_space-MNI_T1w.nii.gz")

# A 4D BOLD series into T1w space, labels with GenericLabel
bold_t1 = lx.ants.apply_transforms(bold_path, t1w_path, [xfm_txt], interpolation="lanczos")
aseg = lx.ants.apply_transforms(aseg_path, boldref_path, [xfm_txt], interpolation="genericlabel", dtype="int16")
```

The same from a shell, with ANTs' own arguments:

```bash
larmorx ants antsApplyTransforms -d 3 -i sub-01_desc-preproc_T1w.nii.gz \
    -r tpl-MNI152NLin2009cAsym_res-02_T1w.nii.gz \
    -t sub-01_from-T1w_to-MNI152NLin2009cAsym_mode-image_xfm.h5 \
    -n LanczosWindowedSinc -o sub-01_space-MNI_T1w.nii.gz
```

## `lx.ants.apply_transforms(image, reference, transforms=(), *, ...)`

Resamples `image` onto the grid of `reference`. Returns an `lx.Image`: float64 data (float32
with `precision="float32"`) on the reference grid, plus a time axis for a series.

**Precision.** `precision="float64"` (the default) computes in double, as ANTs does by
default; `precision="float32"` computes in single precision exactly as
`antsApplyTransforms --float 1` does: the image is read as float32, transform files are read as
ITK's float reader reads them (a text file's numbers rounded once to float), points, transform
arithmetic and continuous indices are float, and the result is float32. What is float and what
stays double, step by step: [ants-float.md](../findings/ants-float.md).

**Transform order.** As with `-t`, for each point of the output grid the *first* transform
acts first. So `[warp, affine]` maps an output point `x` to `affine(warp(x))`. Equivalently,
the image moves by the affine first, then the warp; that is ANTs' "last transform is applied
first". `apply_transforms_to_points` maps points the same way.

**Files and in-memory images.** Paths are read as ITK reads them: ITK's choice between qform
and sform, and ITK's rescaling of scaled integer data through float32. This is what makes the
output match ANTs bit for bit. `lx.Image` objects, nibabel images and `(array, affine)` pairs
are placed by their affine.

### Option mapping

| antsApplyTransforms | `lx.ants.apply_transforms` | Notes |
|---|---|---|
| `-d 3` | – | 3D only for now |
| `-i file` | `image` | path, `lx.Image`, nibabel image or `(array, affine)` |
| `-r file` | `reference` | only its grid is used |
| `-o file` | the returned `Image` (`.save(path)`) | |
| `-o Linear[file,<invert=0>]` | `combine_transforms(..., output="linear", invert_linear=)` | [below](#transform-outputs) |
| `-o CompositeTransform[file]` | `combine_transforms(..., output="composite")` | [below](#transform-outputs) |
| `-o DisplacementField[file]`, `-o [file,1]` | `combine_transforms(..., output="displacement_field", reference=)` | [below](#transform-outputs) |
| `-t file` / `-t [file,1]` | `transforms=[file]`, `invert=[True]` | files or `lx.transforms.read(...)` lists: linear transforms, displacement fields, B-spline transforms (`BSplineTransform_double_3_3`, `_float_3_3`, as antsRegistration's `BSpline` stage writes them) and composites of them; linear transforms only can be inverted (a field or a B-spline transform has no inverse: `[file,1]` fails, as in ANTs). **Nothing is inverted by default**, unlike ANTsPy, which inverts the first of `[.mat, warp]` |
| `-t identity` | `transforms=()` | |
| `-n Linear` | `interpolation="linear"` | default |
| `-n NearestNeighbor` | `"nearest"` | |
| `-n BSpline[order]` | `"bspline"`, `order=3` | orders 0–5 |
| `-n Gaussian[sigma,alpha]` | `"gaussian"`, `sigma=`, `alpha=` | sigma in mm (one value or three); defaults: input spacing, 1 |
| `-n MultiLabel[sigma,alpha]` | `"multilabel"`, `sigma=`, `alpha=` | alpha default 4 (ANTs ignores the command line's, ANTS-33 below) |
| `-n GenericLabel[Linear]` | `"genericlabel"` | `GenericLabel[NearestNeighbor]` interpolates each label's mask by nearest neighbour (ANTs ignores the parameter, ANTS-33) |
| `-n LanczosWindowedSinc` (and Cosine, Hamming, Welch, Blackman) | `"lanczos"`, `"cosine"`, `"hamming"`, `"welch"`, `"blackman"` | ANTs' names are accepted too |
| `-e 0` / `-e 3` | `time_series=None/False/True` | 4D images are resampled volume by volume by default; `False` with 4D is an error, as `-e 0` is |
| `--time-index t` | `image.data[..., t]` | |
| `-f value` | `default_value=` | value outside the input |
| `-u type` | `dtype=` | `char`, `uchar`, `short`, `int`, `float`, `double`; default: the computation type (float64, or float32 with `precision="float32"`). Integers truncate toward zero; out-of-range values clamp (ANTs wraps them, ANTS-06 below; [finding](../findings/ants-cli.md)) |
| `--float 1` | `precision="float32"` | single precision exactly as ANTs computes it ([finding](../findings/ants-float.md)); `--float 0` is `"float64"`, the default |
| `-v 1` | – | |
| (threads: `ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS` or `NSLOTS`, as ITK reads them) | `n_threads=1` (0 = all CPUs) | the result does not depend on it |

Not supported yet (the CLI says so): vector, tensor, multichannel and 5D images (`-e 1, 2, 4,
5, 6`); `-d 2` and `-d 4`; initializers (`-t [fixed,moving,feature]`); MINC transform files
(`.xfm`).

## Transform outputs

`-o Linear[file,<invert>]`, `-o CompositeTransform[file]` and `-o DisplacementField[file]`
(or the older `-o [file,1]`) write the chain of `-t` transforms instead of a warped image, as
ANTs builds it (added 2026-10-16, A4 part 2). In Python, `lx.ants.combine_transforms` returns
the same transforms as `ItkTransform` objects, which `lx.transforms.write` writes.

- **The chain** holds each transform with the class ITK reads it as, in the computation type
  (`_double_3_3`, or `_float_3_3` with `--float 1`). An inverted transform (`-t [file,1]`) has
  the class ITK's inverse makes: an `AffineTransform` stays one, a `TranslationTransform` is the
  opposite translation, and every other matrix-offset class (Euler, versor, similarity, rigid)
  becomes a `MatrixOffsetTransformBase` with the original's fixed parameters (four for an
  Euler transform: its centre and its ZYX flag). A chain without parameters (no `-t`) gets the
  identity `MatrixOffsetTransformBase` ANTs adds.
- **`CompositeTransform[file]`** writes a `CompositeTransform` and the chain's transforms in
  queue order: in HDF5 (`.h5`), ITK text (`.txt`) or MATLAB (`.mat`, where ITK writes the
  composite as one variable: the components' parameters from the last to the first, then their
  fixed parameters).
- **`Linear[file,<invert>]`** collapses a chain of linear transforms into one `AffineTransform`
  (ANTs's `CollapseLinearTransforms`: centred on the last matrix-offset transform of the
  queue); with `invert` 1, its inverse, centred at 0. A chain with a displacement field or a
  B-spline transform is refused ("The transform or set of transforms is not linear.").
- **`DisplacementField[file]`** samples the chain on the reference grid (`-r`, required):
  `T(p) − p` for every voxel (ITK's `TransformToDisplacementFieldFilter`, with points in
  double; a linear chain maps each scan line's two ends and interpolates). `.h5` and `.hdf5`
  give a `DisplacementFieldTransform` file, any other name a NIfTI vector image
  (`x × y × z × 1 × 3`, intent `NIFTI_INTENT_VECTOR`, LPS vectors), as ITK writes them.
- Numbers in text files are written as ITK writes them (the shortest decimal that reads back
  to the same value: `1`, `0.1`, `1.5e-7`), a float transform's as floats; a float
  transform's MATLAB file holds its fixed parameters as floats too.
- An input image (`-i`) is read and checked as ANTs reads it, but not resampled: ANTs resamples
  it onto the reference and discards the result, so the outputs are the same.

Validated against antsApplyTransforms: the record's `transform-output` cases, every file
byte for byte (text, MATLAB), header and vectors bit for bit (NIfTI) or dataset for dataset
(HDF5, whose bytes differ only as [transforms-hdf5.md](transforms-hdf5.md) describes).

## `lx.ants.apply_transforms_to_points(points, transforms=(), *, invert=None, coordinates="lps")`

Maps an `(n, 3+)` array of points (extra columns kept) through the transforms, like
`antsApplyTransformsToPoints`. ANTs works in LPS millimetres; `coordinates="ras"` takes and
returns RAS+. To move points from the moving image to the fixed one, give the inverse
transforms, as with ANTs.

## `lx.ants.combine_transforms(transforms=(), *, invert=None, output="composite", ...)`

`output="composite"`, `"linear"` (with `invert_linear=`) or `"displacement_field"` (with
`reference=`, an image or a path); `precision="float32"` is `--float 1`. Returns a list of
`ItkTransform`: the `CompositeTransform` entry and the chain, the collapsed affine, or the
`DisplacementFieldTransform` (its parameters the interleaved vectors, its fixed parameters the
reference grid). `n_threads` and `upstream_defects` as for `apply_transforms`.

## `lx.transforms`

`lx.transforms.read(path)` returns the transforms stored in an ITK file as `ItkTransform`
objects (`name`, `parameters`, `fixed_parameters`). A leading `CompositeTransform` entry
nests the ones after it. `lx.transforms.write(path, transforms)` writes them.

| Format | Read | Write |
|---|---|---|
| ITK text `.txt`, `.tfm` | ✓ | ✓, numbers as ITK writes them |
| MATLAB `.mat` (ANTs' `*GenericAffine.mat`) | ✓ | ✓ (a composite as ITK writes it: one variable) |
| HDF5 `.h5` (fMRIPrep's `*_xfm.h5`) | ✓, in Rust (Python and command line) | ✓, composites and fields included, laid out as ITK 5.4.5 lays them out ([transforms-hdf5.md](transforms-hdf5.md)) |
| Displacement field `.nii`, `.nii.gz` | ✓ (LPS vectors; RAS→LPS for intent DISPVECT, as ITK) | – |

Linear classes: `AffineTransform`, `MatrixOffsetTransformBase`, `Rigid3DTransform`,
`Euler3DTransform`, `VersorRigid3DTransform`, `Similarity3DTransform`, `TranslationTransform`,
`IdentityTransform`, in float or double. Float32 parameters, as float registrations write
them, stay float32, so a 1 mm warp takes 100 MB instead of 200 MB.

## Rust

```rust
use larmorx_ants::{ApplyTransformsOptions, apply_transforms};
use larmorx_core::Precision;
use larmorx_core::defects::UpstreamDefects;
use larmorx_interp::{Interpolation, Window};
use larmorx_io::itk_transform::read_itk_transform;
use larmorx_io::nifti::{self, itk::{ItkVoxels, itk_geometry, read_itk_image}};
use larmorx_transform::TransformChain;

let image = read_itk_image("in.nii.gz", 0)?;                       // as ITK reads it
let input_grid = image.geometry.grid3().expect("non-singular grid");
let reference = itk_geometry(&nifti::read_header("ref.nii.gz")?)?;
let reference_grid = reference.grid3().expect("non-singular grid");
let chain = TransformChain::new([(read_itk_transform("xfm.mat".as_ref())?, false)])?;
let options = ApplyTransformsOptions {
    interpolation: Interpolation::WindowedSinc(Window::Lanczos),
    default_value: 0.0,
    n_threads: 0,
    precision: Precision::Double, // Precision::Float is --float 1
    upstream_defects: UpstreamDefects::Fix, // Reproduce gives ANTs' defects (D13, below)
};
// f64 values on the reference grid, x fastest (one block per volume for a series); with
// Precision::Float they are the f32 values of ANTs' float output; and the notices of the
// upstream defects met. Text transform files are read for the precision with
// larmorx_io::itk_transform::read_itk_transform_with.
let (values, notices) = match &image.voxels {
    ItkVoxels::F32(v) => apply_transforms(v, &input_grid, &reference_grid, &chain, &options)?,
    ItkVoxels::F64(v) => apply_transforms(v, &input_grid, &reference_grid, &chain, &options)?,
};
```

## Upstream defects (D13)

ANTs' and ITK's deterministic defects are corrected by default and reproduced bit for bit on
request: `upstream_defects="reproduce"` in Python, `LARMORX_UPSTREAM_DEFECTS=reproduce` on the
command line, `ApplyTransformsOptions::upstream_defects` in Rust. A warning (on the command line
a line on standard error) names the entry wherever it changed the result
([upstream-defects.md](upstream-defects.md); each entry in the
[register](../findings/upstream-defects.md)).

| ID | ANTs (what `reproduce` gives) | larmorx's default |
|---|---|---|
| ANTS-33 | an unknown `-u` name keeps the default output type; `MultiLabel[sigma,alpha]` ignores alpha (always 4); `GenericLabel[...]` ignores its parameter (always linear) | an unknown `-u` name is an error; alpha is used; `GenericLabel[Linear]` and `GenericLabel[NearestNeighbor]` interpolate each label's mask that way, any other parameter is an error |
| ANTS-06 | `-u` integer types: values out of the type's range, and NaN, are converted as x86-64 converts them (they wrap around; C++ leaves it undefined): `-u uchar` turns 400.7 into 144 and −3.7 into 253 | clamped to the range, NaN to 0, with a warning (Python's `dtype=` likewise) |
| ITK-01 | ITK's floor of a continuous index just below 0 is 0, not −1 (x86-64), in the displacement-field and windowed-sinc lookups | the exact floor; each output voxel whose lookups met a differing floor is computed again with ITK's, and the warning counts those whose value changed |
| ITK-10 | with `--float 1`, a versor transform whose axis has a norm of at least 1 can fail to load: rounded to float, the scaled axis exceeds 1 and ITK refuses it (exit 1) | the versor is computed in double and stored as float, so the transform loads |
| ANTS-03, ITK-02, ITK-08 | as for every ANTs program ([ImageMath](ants-image-math.md#upstream-defects-d13)) | as there |

## Deliberate and known differences

- **Not reproduced:** `Rigid3DTransform`'s orthogonality check (ITK refuses a non-orthogonal
  matrix; larmorx reads it as an affine), and `GenericLabel`'s merging of `−0.0` and `+0.0` into
  one label ([ants-float.md](../findings/ants-float.md), last section).
- **GenericLabel** evaluates only the labels among the 8 neighbours (ANTS-34): ITK evaluates
  every distinct value in the image and practically never finishes on an intensity image. The
  result is the same.
- **Transform outputs where ANTs crashes or aborts** (ANTS-48): larmorx refuses with a
  message, exit 1, in both D13 modes: an unknown transform file extension (ANTs aborts on an
  uncaught ITK exception), an input image without a reference, a Gaussian or MultiLabel
  interpolator without an input image, an `IdentityTransform` file collapsed by `Linear[...]`
  (ANTs crashes on each), a singular collapsed affine inverted.
- **HDF5 transform outputs** name larmorx as the system (`/OSName`, `/OSVersion`) and record
  object time 0; ITK names the machine and stamps the time ([transforms-hdf5.md](transforms-hdf5.md)).

## Performance

See [docs/benchmarks/ants-apply-transforms.md](../benchmarks/ants-apply-transforms.md).
