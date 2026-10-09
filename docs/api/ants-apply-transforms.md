# antsApplyTransforms

`lx.ants.apply_transforms`, `lx.ants.apply_transforms_to_points` and `lx.transforms`
(command line `larmorx ants antsApplyTransforms`; Rust crates `larmorx-ants`,
`larmorx-interp`, `larmorx-transform`, `larmorx-io`). Ported from ANTs 2.6.5 on ITK 5.4.5.

**Status: `validated`** against antsApplyTransforms itself (see the
[validation record](../validation/ants-apply-transforms.md)). The record covers 83 cases:
- every interpolator;
- every transform type and file format;
- awkward geometries, output types and time series;
- real fMRIPrep derivatives.

All 83 agree, and 63 of the 79 compared are byte-identical. Every interpolator gives **the same bytes** as ANTs except where ITK calls the
platform's `exp`, `log`, `sin` or `cos` (Gaussian and windowed sinc). There the last bit can
differ, at most about 1e-15 relative
([why](../findings/platform-math.md)).

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

Resamples `image` onto the grid of `reference`. Returns an `lx.Image`: float64 data on the
reference grid, plus a time axis for a series.

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
| `-t file` / `-t [file,1]` | `transforms=[file]`, `invert=[True]` | files or `lx.transforms.read(...)` lists; linear transforms only can be inverted. **Nothing is inverted by default**, unlike ANTsPy, which inverts the first of `[.mat, warp]` |
| `-t identity` | `transforms=()` | |
| `-n Linear` | `interpolation="linear"` | default |
| `-n NearestNeighbor` | `"nearest"` | |
| `-n BSpline[order]` | `"bspline"`, `order=3` | orders 0–5 |
| `-n Gaussian[sigma,alpha]` | `"gaussian"`, `sigma=`, `alpha=` | sigma in mm (one value or three); defaults: input spacing, 1 |
| `-n MultiLabel[sigma]` | `"multilabel"`, `sigma=`, `alpha=` | ANTs ignores an alpha on the command line (always 4); the Python API accepts one, default 4 |
| `-n GenericLabel` | `"genericlabel"` | |
| `-n LanczosWindowedSinc` (and Cosine, Hamming, Welch, Blackman) | `"lanczos"`, `"cosine"`, `"hamming"`, `"welch"`, `"blackman"` | ANTs' names are accepted too |
| `-e 0` / `-e 3` | `time_series=None/False/True` | 4D images are resampled volume by volume by default; `False` with 4D is an error, as `-e 0` is |
| `--time-index t` | `image.data[..., t]` | |
| `-f value` | `default_value=` | value outside the input |
| `-u type` | `dtype=` | default float64. Integers truncate toward zero and saturate in Python; the CLI wraps out-of-range values as ANTs does on x86-64 ([finding](../findings/ants-cli.md)) |
| `--float 1` | – | the CLI rounds input and output to float32 and computes in double |
| `-v 1` | – | |
| (threads: `ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS`) | `n_threads=1` (0 = all CPUs) | the result does not depend on it |

Not supported yet (the CLI says so): vector, tensor, multichannel and 5D images (`-e 1, 2, 4,
5, 6`); `-d 2` and `-d 4`; transform outputs (`-o Linear[...]`, `-o CompositeTransform[...]`,
`-o [field,1]`); initializers (`-t [fixed,moving,feature]`).

## `lx.ants.apply_transforms_to_points(points, transforms=(), *, invert=None, coordinates="lps")`

Maps an `(n, 3+)` array of points (extra columns kept) through the transforms, like
`antsApplyTransformsToPoints`. ANTs works in LPS millimetres; `coordinates="ras"` takes and
returns RAS+. To move points from the moving image to the fixed one, give the inverse
transforms, as with ANTs.

## `lx.transforms`

`lx.transforms.read(path)` returns the transforms stored in an ITK file as `ItkTransform`
objects (`name`, `parameters`, `fixed_parameters`). A leading `CompositeTransform` entry
nests the ones after it. `lx.transforms.write(path, transforms)` writes them.

| Format | Read | Write |
|---|---|---|
| ITK text `.txt`, `.tfm` | ✓ | linear transforms |
| MATLAB `.mat` (ANTs' `*GenericAffine.mat`) | ✓ | one linear transform |
| HDF5 `.h5` (fMRIPrep's `*_xfm.h5`) | ✓ (h5py) | ✓, composites and fields included; ANTs reads what larmorx writes |
| Displacement field `.nii`, `.nii.gz` | ✓ (LPS vectors; RAS→LPS for intent DISPVECT, as ITK) | – |

Linear classes: `AffineTransform`, `MatrixOffsetTransformBase`, `Rigid3DTransform`,
`Euler3DTransform`, `VersorRigid3DTransform`, `Similarity3DTransform`, `TranslationTransform`,
`IdentityTransform`, in float or double. Float32 parameters, as float registrations write
them, stay float32, so a 1 mm warp takes 100 MB instead of 200 MB.

## Rust

```rust
use larmorx_ants::{ApplyTransformsOptions, apply_transforms};
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
};
// f64 values on the reference grid, x fastest (one block per volume for a series)
let values = match &image.voxels {
    ItkVoxels::F32(v) => apply_transforms(v, &input_grid, &reference_grid, &chain, &options)?,
    ItkVoxels::F64(v) => apply_transforms(v, &input_grid, &reference_grid, &chain, &options)?,
};
```

## Performance

See [docs/benchmarks/ants-apply-transforms.md](../benchmarks/ants-apply-transforms.md).
