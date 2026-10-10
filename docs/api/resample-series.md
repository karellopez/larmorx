# One-shot BOLD resampling

`lx.transforms.resample_series` (Rust: `larmorx_transform::resample_series`): fMRIPrep's
one-shot resampler, which applies head motion, susceptibility distortion and the transforms to
a target space to a BOLD series in a single cubic interpolation. With it:
`lx.transforms.load_transforms` and the chain types (nitransforms' conventions), and
`lx.ndimage`, SciPy's `map_coordinates` and spline prefilter underneath.

**Replica.** Ported from fMRIPrep (`fmriprep/interfaces/resampling.py`, `utils/transforms.py`,
commit `21a490fb`, Apache-2.0), nitransforms 25.1.0 (MIT), SciPy 1.15.2's `scipy/ndimage`
(BSD-3-Clause) and nibabel 5.4.2 (MIT), with their notices
([NOTICE](../../NOTICE); provenance:
[larmorx-transform](../../crates/larmorx-transform/PROVENANCE.md),
[larmorx-interp](../../crates/larmorx-interp/PROVENANCE.md)).

**Status: `validated`** against fMRIPrep's `ResampleSeries`, nitransforms and SciPy, run
in-process on the same inputs (see the [validation record](../validation/resample-series.md)):
every compared output is **bit-identical**, including real OpenNeuro runs resampled to native,
T1w and MNI152NLin2009cAsym space through fMRIPrep's own transforms. The behaviour this rests
on is in [fmriprep-resampling.md](../findings/fmriprep-resampling.md) and
[scipy-ndimage.md](../findings/scipy-ndimage.md).

**Speed:** 4.1–5.2× faster than fMRIPrep's implementation on one thread and 4.8–6.9× on 12
threads, bit-identical, with memory bounded by the thread count
([benchmarks](../benchmarks/resample-series.md)).

## Quick start

```python
import json
import larmorx as lx

meta = json.load(open("sub-01_task-rest_bold.json"))
bold_t1w = lx.transforms.resample_series(
    "sub-01_task-rest_desc-stc_bold.nii.gz",           # 4D source (after slice timing)
    "sub-01_space-T1w_boldref.nii.gz",                  # target grid (only its header is read)
    [                                                    # fMRIPrep's order: head motion first
        "sub-01_from-orig_to-boldref_mode-image_desc-hmc_xfm.txt",
        "sub-01_from-boldref_to-T1w_mode-image_desc-coreg_xfm.txt",
    ],
    fieldmap="fmap_hz_in_T1w.nii.gz",                    # Hz, on the target grid
    pe_dir=meta["PhaseEncodingDirection"],
    ro_time=meta["TotalReadoutTime"],
    jacobian=True,
    n_threads=8,
)
bold_t1w.save("sub-01_space-T1w_desc-preproc_bold.nii.gz")
```

Standard space adds fMRIPrep's composite `.h5` (`[hmc, boldref2anat, anat2std.h5]`).
Everything also works on in-memory data (`lx.Image`, nibabel images, `(array, affine)`
pairs; a target can be a `(shape, affine)` pair) and on loaded chains:

```python
chain = lx.transforms.load_transforms([hmc_txt, coreg_txt, anat2std_h5])
points = chain.map(ras_points)                 # (n, 3) RAS+ mm, as nitransforms maps them
values = lx.ndimage.map_coordinates(vol, coords, order=3, mode="grid-constant")
```

## `lx.transforms.resample_series(source, target, transforms=None, *, inverse=False, fieldmap=None, pe_dir=None, ro_time=None, jacobian=True, order=3, mode="grid-constant", cval=0.0, prefilter=True, output_dtype="float32", n_threads=1)`

For each volume `t` and each target voxel, as fMRIPrep's `resample_vol`:

1. the target voxel's RAS+ position (float32) goes through the transform chain and the
   source's inverse affine to source voxel coordinates (once per run);
2. volume `t`'s head-motion affine moves it (voxel to voxel);
3. the voxel-shift map, `fieldmap (Hz) × readout time`, is added along the phase-encoding axis;
4. the volume is interpolated there (SciPy's cubic B-spline, prefiltered in float64);
5. with `jacobian`, the value is multiplied by `1 + d(vsm)/d(axis)`.

Returns an `lx.Image` on the target grid (plus the volumes for a 4D source) with the target's
affine and a float32 header: the target's, with the source's time step.

### Option mapping

| fMRIPrep `ResampleSeries` input | `resample_series` | Notes |
|---|---|---|
| `in_file` | `source` | 3D or 4D; read as float32 (`get_fdata(dtype='f4')`) |
| `ref_file` | `target` | only the grid (shape, affine) and header are used |
| `transforms` | `transforms` | files in fMRIPrep's order, or a loaded chain; head-motion series first in the file list |
| `inverse` (default `[False]`) | `inverse` | one flag or one per file; displacement fields cannot be inverted |
| `fieldmap` | `fieldmap` | Hz on the target grid (path, image or array) |
| `ro_time`, `pe_dir` | `ro_time`, `pe_dir` | both needed for distortion correction; then the source is reoriented to positive direction cosines first |
| `jacobian` (mandatory) | `jacobian` (default `True`, `resample_image`'s default) | |
| `num_threads` | `n_threads` | 0 = all; the result does not depend on it |
| `output_data_type` (`float32`) | `output_dtype` | `float32` or `float64` |
| `order` (3), `mode` (`grid-constant`), `cval` (0), `prefilter` (True) | the same | SciPy's `map_coordinates` settings; every SciPy mode and order 0–5 |
| – (output file `*_resampled.nii.gz`) | – | the image is returned; `lx.save` writes it |

### Transforms (`lx.transforms`)

| nitransforms / fMRIPrep | larmorx | Notes |
|---|---|---|
| `fmriprep.utils.transforms.load_transforms(paths, inverse)` | `load_transforms(paths, inverse)` | `.h5` → composite, anything else → ITK linear; the last file first |
| `nt.linear.load` | `load_itk_linear` | ITK text (one or more transforms; values parsed as float32), MATLAB `.mat` |
| `nt.manip.load(path, fmt="h5")` | `load_itk_composite` | affines and displacement fields from ITK's HDF5 layout |
| `Affine` | `Affine` | `matrix` (4×4 RAS+), `inverse`, `~`, `map` |
| `LinearTransformsMapping` | `AffineSeries` | `matrices` `(t, 4, 4)`; head motion |
| `DenseFieldTransform` | `DenseField` | `deltas` `(x, y, z, 3)` RAS+ mm and the grid's `affine` |
| `TransformChain` | `TransformChain` | `transforms`, `steps()`, `map`, `+` |

`TransformChain.map` (and `Affine.map`, `DenseField.map`) map `(n, 3)` RAS+ points exactly as
nitransforms does: float32 inputs at every affine, numpy's fused products, the displacement
field looked up when every point is on its grid and interpolated otherwise, no displacement
outside it.

### `lx.ndimage`

| SciPy | larmorx | Notes |
|---|---|---|
| `map_coordinates(input, coordinates, output, order, mode, cval, prefilter)` | the same, plus `n_threads` | `output` is a dtype, not an array; 1- to 4-D inputs; float, integer outputs |
| `spline_filter(input, order, output, mode)` | `spline_filter(input, order, mode, n_threads=)` | float64 result |
| `spline_filter1d(input, order, axis, output, mode)` | `spline_filter1d(input, order, axis, mode, n_threads=)` | float64 result |

## Rust API

```rust
use larmorx_transform::nitransforms::{DenseField, Step};
use larmorx_transform::resample_series::{PeInfo, ResampleOptions, SeriesResampler};

// Once per run: map the target grid (target RAS → source RAS → source voxels).
let plan = SeriesResampler::new(
    target_shape, &target_affine, &[Step::Affine(boldref2anat), Step::DenseField(warp)],
    &ras2vox, source_shape, Some(fmap_hz), ResampleOptions::default(), n_threads,
)?;
// All volumes (or one at a time with `resample_volume`, for streaming).
plan.resample_series(&source_f32, n_volumes, Some(&hmc_vox2vox), &[PeInfo { axis: 1, readout: -0.0312 }],
                     &mut out_f32, n_threads)?;
```

`larmorx_interp::ndimage` has `Spline` (an input prefiltered once, then `sample` /
`sample_batch` at any coordinates), `map_coordinates`, `spline_filter`, `spline_filter1d` and
`Mode`. The Rust API takes the 4×4 matrices ready-made; the Python layer computes them with
numpy, exactly as nitransforms and fMRIPrep do.

## Command line

None. fMRIPrep has no command-line program for this step (it is a nipype interface inside the
workflow), so there are no original arguments to accept; the pipeline calls the Python
function. A `larmorx` subcommand can follow if a standalone use appears.

## Differences and limits

- **Undefined behaviour in SciPy.** Coordinates beyond about 1e18 make SciPy's index arithmetic
  overflow and read outside its array; larmorx returns NaN there
  ([scipy-ndimage.md](../findings/scipy-ndimage.md)). fMRIPrep never produces them.
- **The 4×4 matrices come from numpy** (inverses, products, ITK-to-RAS), as in fMRIPrep, so
  they match fMRIPrep on the same machine. Their last bits depend on the CPU's BLAS kernel and
  on the BLAS library, for fMRIPrep too ([golden.md](../validation/golden.md), "Known risk"). The per-voxel arithmetic is larmorx's and gives the
  same bits on every platform: SciPy's x86-64 results (no contraction; numpy's BLAS-style fused
  products, which x86-64 and aarch64 OpenBLAS both use).
- **Not covered yet:** `ReconstructFieldmap` (B-spline field maps reconstructed on the target
  grid; the field map here is given in Hz), multi-echo (each echo resampled separately, as
  fMRIPrep does today; sharing one coordinate mapping between echoes is planned), per-volume
  readout times in the Python API, CIFTI and surfaces, X5 and non-ITK transform formats (FSL,
  AFNI, LTA), and complex data.
