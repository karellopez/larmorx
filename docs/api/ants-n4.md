# N4BiasFieldCorrection

`lx.ants.n4_bias_field_correction` (command line `larmorx ants N4BiasFieldCorrection`; Rust
`larmorx_ants::n4_bias_field_correction`, `larmorx_image::n4`). A replica of ANTs 2.6.5's
`N4BiasFieldCorrection` on ITK 5.4.5's `N4BiasFieldCorrectionImageFilter` (Apache-2.0): N4
bias field correction (Tustison et al., *N4ITK: Improved N3 Bias Correction*, IEEE TMI 29(6),
2010).

**Status: `validated`** against N4BiasFieldCorrection itself (see the
[validation record](../validation/ants-n4.md)), on every command line fMRIPrep, sMRIPrep and
sdcflows give it (standard tier: 45 compared cases, 54 outputs, in 2, 3 and 4 dimensions):
- against the **unmodified ANTs 2.6.5 binary** at one thread: 29 of 45 cases (35 of 54
  outputs) bit-identical. The others differ only because glibc's `expf`, `logf` and `exp` are
  not always correctly rounded and larmorx's are; the differences are smaller than ANTs' own
  difference between thread counts in 14 of the 16 cases, up to 4.2 times that in two, and
  NCC is at least 0.9999999 (corrected images) and 0.9999998 (bias fields);
- against the **same binary run with glibc's `expf`, `logf`, `exp`, `log`, `sin`, `cos` and
  `sincos` replaced by CORE-MATH's correctly rounded ones** (`LD_PRELOAD`,
  `scripts/build_crmath_preload.sh`), which is the arithmetic larmorx uses: **54 of 54 outputs
  bit-identical**. ANTs' own code is not changed; only those C-library functions are;
- the same against **fMRIPrep 25.2.6's own ANTs (2.6.2)**: 45 of 45 cases bit-identical with
  CORE-MATH, 29 of 45 as built.

larmorx computes what ITK's C++ source defines. ANTs built by GCC 11.4 at `-O3` (Ubuntu
22.04's compiler) does not: GCC bug 110182 drops two float roundings of the B-spline fit, so
such a build gives slightly different N4 results
([findings](../findings/ants-n4.md), "GCC 11 miscompiles the fit").

**Speed** ([benchmark](../benchmarks/ants-n4.md)): sMRIPrep's N4 before the anatomical template (ANTs'
defaults) on a raw T1w (160×192×192) takes 8.7 s on one thread and 2.4 s on 12, against
55.2 s and 10.7 s for ANTs; fMRIPrep's BOLD reference 135 ms against 1.00 s. On the six
command lines measured, larmorx is 6.4–7.4× faster than ANTs on one thread, and 4.2–6.3×
faster than ANTs on 12 threads when both use 12.

## Quick start

```python
import larmorx as lx

res = lx.ants.n4_bias_field_correction("sub-01_T1w.nii.gz")                 # ANTs' defaults
res.corrected.to_filename("sub-01_desc-n4_T1w.nii.gz")

# sMRIPrep's final N4: weight = WM probability, spline distance 200 mm, five levels
res = lx.ants.n4_bias_field_correction(
    t1w, weight=wm_probseg, shrink_factor=4,
    convergence=([50] * 5, 1e-7), bspline_fitting=200, n_threads=8,
)
corrected, bias = res.corrected, res.bias_field

# with a brain mask, rescaled to the input's range inside it (the command line's default)
res = lx.ants.n4_bias_field_correction(boldref, mask=brain_mask, bspline_fitting=200,
                                       rescale_intensities=True)
```

```bash
larmorx ants N4BiasFieldCorrection -d 3 -i sub-01_T1w.nii.gz -o n4.nii.gz
larmorx ants N4BiasFieldCorrection -d 3 -b [ 200 ] -c [ 50x50x50x50x50, 1e-07 ] -s 4 -r \
    -w wm.nii.gz -i t1w.nii.gz -o [ corrected.nii.gz, bias.nii.gz ]
```

## Option mapping

| N4BiasFieldCorrection | Python | Notes |
|---|---|---|
| `-d 2/3/4` | `dimension` (default: the image's) | a 4D file read with `-d 3` gives its first volume; in 4D the time axis is not shrunk but gets a spline too |
| `-i image` | `image` | path, `lx.Image`, nibabel image or `(array, affine)`; read as `float` |
| `-x mask` | `mask` | N4 uses the voxels where the mask is not 0; outside it the output keeps the input's values |
| `-w weight` | `weight` | the fit weights each voxel by it; voxels with weight ≤ 0 are not used |
| `-s 4` | `shrink_factor=4` | `ShrinkImageFilter` along the spatial axes; 0 acts as 1 |
| `-c [50x50x50x50,0.0]` | `convergence=((50, 50, 50, 50), 0.0)` | iterations per level (one level per entry) and the threshold that ends a level |
| `-b [200]` | `bspline_fitting=200.0` | a spline distance in mm: the image is padded so that whole spans of 200 mm cover it |
| `-b [2x2x3]` | `bspline_fitting=(2, 2, 3)` | mesh elements per axis (control points = elements + order) |
| `-b [...,3]` | `spline_order=3` | |
| no `-b` | `bspline_fitting=None` | one element per axis (4 control points for cubic splines) |
| `-t [0.15,0.01,200]` | `histogram_sharpening=(0.15, 0.01, 200)` | the Gaussian's FWHM, the Wiener noise, the number of bins |
| `-r 0/(1)` | `rescale_intensities=False` | **different default**: the command line rescales whenever a mask is given unless `-r 0`; Python rescales only when asked (as ANTsPy). Without a mask it never applies |
| `-o out` / `-o [out,bias]` | `N4Result.corrected`, `N4Result.bias_field` | float32 images on the input's grid; the bias field is always returned |
| `-v 1` | `N4Result.iterations` | ANTs' progress lines; Python returns every iteration as `N4Iteration(level, iteration, convergence)` |
| `ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS` (or `NSLOTS`, as ITK reads them) | `n_threads` | **ANTs' result depends on it; larmorx's does not**: it always gives ANTs' one-thread result |

## Behaviour worth knowing

From ANTs, reproduced ([findings](../findings/ants-n4.md)):
- options that need brackets do nothing without them: `-c 50x50` or `-b 200` leave ITK's
  defaults (one level of 50 iterations, threshold 0.001, 4 control points); without `-c`
  ANTs uses four levels of 50 and threshold 0;
- the log image keeps zero and negative values as they are, so without a mask a raw image's
  background is part of the histogram;
- with a mask, voxels outside it (mask almost 0) get the input's values back, and rescaling
  maps the corrected range in the mask (mask not 0) to the input's, changing only voxels where
  the mask is almost exactly 1;
- a spline distance pads the image with zeros (and its mask and weight); the outputs are cropped
  back;
- a mask file that cannot be read is replaced by ones but still counts as given;
- the mask and weight must share the input's grid (ITK's tolerance: 1e-6 of the first spacing).

## Deliberate differences

- **Threads**: the result does not depend on `n_threads` and equals ANTs' single-thread
  result. ANTs' changes with its thread count (by up to about 1e-3 relative on a T1w).
- **Maths**: `expf`, `logf` and `exp` are correctly rounded (CORE-MATH), the same on every
  platform; glibc's are not always, so ANTs as built can differ slightly
  ([findings](../findings/ants-n4.md#glibcs-math-in-n4)).
- Where ANTs aborts or crashes (an unreadable input, a value that is not a number, a mask on
  another grid), larmorx exits 1 with a message.
- `-v` prints ANTs' progress lines but not ITK's dump of the filter object.
- Images are NIfTI only.

## Rust

```rust
use larmorx_ants::n4_bias_field_correction::{n4_bias_field_correction, BSplineMesh, N4Options};

let options = N4Options {
    iterations: vec![50; 5],
    convergence_threshold: 1e-7,
    mesh: BSplineMesh::Distance(200.0),
    ..N4Options::default()
};
let out = n4_bias_field_correction(&input, None, Some(&weight), &options, n_threads, None)?;
// out.corrected, out.bias_field, out.iterations
```

The filter itself is `larmorx_image::n4::n4_bias_field`, on any grid with an optional mask and
confidence image; it returns the control-point lattice, which `larmorx_image::bspline::evaluate`
turns into a field of any size. `larmorx_image::shrink::{shrink_plan, shrink}` is ITK's
`ShrinkImageFilter` for any dimension and pixel type.
