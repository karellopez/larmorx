# One-shot BOLD resampling

`lx.transforms.resample_series` (Rust: `larmorx_transform::resample_series`): fMRIPrep's
one-shot resampler, which applies head motion, susceptibility distortion and the transforms to
a target space to a BOLD series in a single cubic interpolation. With it:
`lx.transforms.load_transforms` and the chain types (nitransforms' conventions), and
`lx.ndimage`, SciPy's `map_coordinates` and spline prefilter underneath.

**Replica.** Ported from fMRIPrep (`fmriprep/interfaces/resampling.py`, `utils/transforms.py`,
commit `21a490fb`, Apache-2.0), nitransforms 25.1.0 (MIT), SciPy's `scipy/ndimage` (1.15.2,
following SciPy 1.18.1 since 2026-10-29; BSD-3-Clause) and nibabel 5.4.2 (MIT), and numpy's 4 × 4
arithmetic on OpenBLAS's Haswell kernels (numpy 2.5.4 with OpenBLAS 0.3.34, unchanged from 2.3.5
with 0.3.30), with their notices
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

## `lx.transforms.resample_series(source, target, transforms=None, *, inverse=False, fieldmap=None, pe_dir=None, ro_time=None, jacobian=True, order=3, mode="grid-constant", cval=0.0, prefilter=True, output_dtype="float32", n_threads=1, upstream_defects="fix")`

For each volume `t` and each target voxel, as fMRIPrep's `resample_vol`:

1. the target voxel's RAS+ position (float32) goes through the transform chain and the
   source's inverse affine to source voxel coordinates (once per run);
2. volume `t`'s head-motion affine moves it (voxel to voxel);
3. the voxel-shift map, `fieldmap (Hz) × readout time`, is added along the phase-encoding axis;
4. the volume is interpolated there (SciPy's cubic B-spline, prefiltered in float64);
5. with `jacobian`, the value is multiplied by the Jacobian of the voxel-shift map: by default
   `1 + ∇vsm · ∂t/∂s_pe`, the derivative along the phase-encoding direction mapped into the
   target grid; fMRIPrep's `1 + np.gradient(vsm, axis=pe)` with `upstream_defects="reproduce"`
   (PIPE-03, [below](#upstream-defects-d13)).

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
| `order` (3), `mode` (`grid-constant`), `cval` (0), `prefilter` (True) | the same | SciPy's `map_coordinates` settings; every SciPy mode and order 0–5 (`reflect`, `grid-mirror` and `nearest` with SciPy 1.18.1's exact boundary, SCI-03 fixed upstream) |
| – | `upstream_defects` | `"fix"` (default) corrects PIPE-03 and PIPE-04; `"reproduce"` gives fMRIPrep's result bit for bit (with SciPy 1.18.1) |
| – (output file `*_resampled.nii.gz`) | – | the image is returned; `lx.save` writes it |

### Transforms (`lx.transforms`)

| nitransforms / fMRIPrep | larmorx | Notes |
|---|---|---|
| `fmriprep.utils.transforms.load_transforms(paths, inverse)` | `load_transforms(paths, inverse, *, upstream_defects="fix")` | `.h5` → composite, anything else → ITK linear; the last file first |
| `nt.linear.load` | `load_itk_linear` | ITK text (one or more transforms; values parsed as float32), MATLAB `.mat` |
| `nt.manip.load(path, fmt="h5")` | `load_itk_composite(path, *, upstream_defects="fix")` | affines and displacement fields from ITK's HDF5 layout, in numeric order (nitransforms' name order with `reproduce`, PIPE-04) |
| `Affine` | `Affine` | `matrix` (4×4 RAS+), `inverse`, `~`, `map` |
| `LinearTransformsMapping` | `AffineSeries` | `matrices` `(t, 4, 4)`; head motion |
| `DenseFieldTransform` | `DenseField` | `deltas` `(x, y, z, 3)` RAS+ mm and the grid's `affine` |
| `TransformChain` | `TransformChain` | `transforms`, `steps()`, `map`, `+` |

`TransformChain.map` (and `Affine.map`, `DenseField.map`) map `(n, 3)` RAS+ points exactly as
nitransforms does: float32 inputs at every affine, numpy's fused products (its `dgemv` order
for a single point), the displacement field looked up when every point is on its grid and
interpolated otherwise, no displacement outside it.

The 4×4 matrices (`Affine.inverse`, `~`, the ITK-to-RAS conversion of loaded files, head motion
in voxel space, `ensure_positive_cosines`) are computed with nitransforms' expressions, in Rust,
in the operation order of the OpenBLAS kernels numpy uses on x86-64 with FMA: the same bits as
nitransforms on such a machine, and on every platform
([numpy-blas.md](../findings/numpy-blas.md)).

### `lx.ndimage`

| SciPy | larmorx | Notes |
|---|---|---|
| `map_coordinates(input, coordinates, output, order, mode, cval, prefilter)` | the same, plus `n_threads`, `upstream_defects` | `output` is a dtype, not an array; 1- to 4-D inputs; float, integer outputs |
| `spline_filter(input, order, output, mode)` | `spline_filter(input, order, mode, n_threads=, upstream_defects=)` | float64 result |
| `spline_filter1d(input, order, axis, output, mode)` | `spline_filter1d(input, order, axis, mode, n_threads=, upstream_defects=)` | float64 result |

## Upstream defects (D13)

Four defects of fMRIPrep's resampler and its libraries are corrected by default and reproduced
with `upstream_defects="reproduce"`; an `lx.UpstreamDefectWarning` names each one that changed
the result, in both modes ([upstream-defects.md](upstream-defects.md); register entries in
[findings/upstream-defects.md](../findings/upstream-defects.md)):

| ID | fMRIPrep, nitransforms, SciPy | larmorx by default |
|---|---|---|
| PIPE-03 | `ResampleSeries` multiplies by `1 + np.gradient(vsm, axis=pe)`: the gradient along the *target* array axis whose index is the source's phase-encoding axis, in target voxels. It is the Jacobian only when the target's axes are the source's (the native boldref); in T1w or standard space it is taken along another direction (a target whose axes are permuted or flipped relative to the BOLD grid) or at another scale (a target voxel smaller than the source's: the derivative per target voxel is a fraction of the derivative per source voxel) | `1 + ∇_t vsm · ∂t/∂s_pe`, the Jacobian determinant of the corrected mapping relative to the uncorrected one (the matrix determinant lemma), with `∂t/∂s_pe` column `pe` of the inverse Jacobian of the map from target voxels to source voxels: exact for a chain of affines, by central differences (±0.5 voxel) through a displacement field. Where the target's axes are the source's, it is fMRIPrep's float32 value bit for bit. Head motion is left out, as in fMRIPrep |
| PIPE-04 | nitransforms reads the transforms of a composite `.h5` file in h5py's name order, so a composite of 10 or more transforms is applied out of order (`"10"` before `"2"`) | numeric order |
| SCI-03 | SciPy 1.15's spline prefilter, for the reflect boundary (modes `reflect`, `grid-mirror`, `nearest`), read a coefficient it had already updated, so its splines missed the samples on short axes (by up to 1e-2 on a 6-voxel axis, order 5) | fixed in SciPy 1.18, whose exact half-sample-symmetric initialisation larmorx follows in both modes since 2026-10-29 (no notice) |
| SCI-02 | `lx.ndimage`: a NaN result written to a signed integer output, and a `cval` an integer input type cannot hold, are converted as x86-64 converts them (C leaves it undefined): `INT32_MIN` for int32, 0 once narrowed to int8 or int16 | NaN is 0, `cval` clamped to the type (NaN to 0), with a warning |

fMRIPrep's own calls (`grid-constant`, composites of two or three transforms) meet only PIPE-03,
with field maps in T1w and standard spaces (PEPOLAR field maps apply the Jacobian by default).
The parity record's "D13 fix pass" lists what the corrections change, case by case. On the
grids and transforms of a real fMRIPrep run, with a 3 T-like field map, fMRIPrep's factor is
off the exact one by more than 1 % of the signal in 40 % of brain voxels on the 1 mm T1w grid
and 33 % on the MNI res-2 one (99th-percentile error 0.23 and 0.16), larmorx's by at most 0.002
and 0.008. On fMRIPrep's own PEPOLAR field map (ds005040, where fMRIPrep applies the Jacobian),
larmorx's default changes fMRIPrep's MNI res-2 BOLD by more than 1 % in 60 % of brain voxels
(99th percentile 11 %, up to 56 % in the inferior temporal lobes), its `space-T1w` BOLD (the
BOLD's voxel size) by more than 1 % in 2.8 %, and its native BOLD not at all
([validation/resample-series-jacobian.md](../validation/resample-series-jacobian.md)).

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
// The notices name the upstream defects that changed the result (ResampleOptions::
// upstream_defects: UpstreamDefects::Fix by default, Reproduce for fMRIPrep's result).
let notices = plan.resample_series(&source_f32, n_volumes, Some(&hmc_vox2vox),
                                   &[PeInfo { axis: 1, readout: -0.0312 }], &mut out_f32, n_threads)?;
```

`larmorx_interp::ndimage` has `Spline` (an input prefiltered once, then `sample` /
`sample_batch` at any coordinates), `map_coordinates`, `spline_filter`, `spline_filter1d` and
`Mode`. The Rust API takes the 4×4 matrices ready-made; compute them as nitransforms and
fMRIPrep do with `larmorx_transform::openblas::{matmul4, inv4}` (numpy's `@` and
`np.linalg.inv` in the operation order of OpenBLAS's Haswell kernels), as the Python layer
does.

## Command line

None. fMRIPrep has no command-line program for this step (it is a nipype interface inside the
workflow), so there are no original arguments to accept; the pipeline calls the Python
function. A `larmorx` subcommand can follow if a standalone use appears.

## Differences and limits

- **Undefined behaviour in SciPy.** Coordinates beyond about 1e18 make SciPy's index arithmetic
  overflow and read outside its array; larmorx returns NaN there
  ([scipy-ndimage.md](../findings/scipy-ndimage.md)). fMRIPrep never produces them.
- **The same bits on every platform.** The 4×4 matrices (inverses, products, ITK-to-RAS,
  head motion, `ensure_positive_cosines`) and the per-voxel arithmetic are larmorx's, in a
  fixed order: numpy's results on x86-64 with FMA (OpenBLAS's Haswell kernels; 18,143,058 of
  18,143,058 compared results bit-identical, [numpy-linalg.md](../validation/numpy-linalg.md))
  and SciPy's x86-64 interpolation (no contraction). fMRIPrep's own matrices depend on numpy's
  BLAS kernel, so on a CPU with AVX-512 or without FMA, or on macOS or ARM, fMRIPrep's last
  bits differ from larmorx's (and from its own on this machine)
  ([numpy-blas.md](../findings/numpy-blas.md)).
- **Exactly 45° oblique grids.** With distortion correction, the axis flips of
  `ensure_positive_cosines` follow nibabel's `io_orientation`, computed with a platform-
  independent SVD. When two direction cosines tie exactly (a grid rotated by exactly 45°),
  nibabel's choice follows its SVD's rounding: 4,300 of 4,800 such grids get the same flips.
  A tie needs a rotation of exactly 45° to within rounding.
- **Not covered yet:** `ReconstructFieldmap` (B-spline field maps reconstructed on the target
  grid; the field map here is given in Hz), multi-echo (each echo resampled separately, as
  fMRIPrep does today; sharing one coordinate mapping between echoes is planned), per-volume
  readout times in the Python API, CIFTI and surfaces, X5 and non-ITK transform formats (FSL,
  AFNI, LTA), and complex data.
