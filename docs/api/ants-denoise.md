# DenoiseImage

`lx.ants.denoise_image` (command line `larmorx ants DenoiseImage`; Rust
`larmorx_ants::denoise_image`, `larmorx_image::adaptive_denoising`). A replica of ANTs 2.6.5's
`DenoiseImage` on the `AdaptiveNonLocalMeansDenoisingImageFilter` of ITK 5.4.5's remote module
AdaptiveDenoising (commit `24825c8d`, Apache-2.0): adaptive non-local means denoising (Manjón et
al., *Adaptive non-local means denoising of MR images with spatially varying noise levels*,
JMRI 31:192-203, 2010). sMRIPrep runs it on every T1w (`-n Rician`, ANTs' other defaults).

**Status: `validated`** against DenoiseImage itself (see the
[validation record](../validation/ants-denoise.md)), on sMRIPrep's command line on real T1w
images and on every option, in 2, 3 and 4 dimensions (standard tier: 40 compared cases, 52
outputs):
- against the **unmodified ANTs 2.6.5 binary** at one thread: 7 of 40 cases (9 of 52 outputs)
  bit-identical. In the others, glibc's `expf` (or `exp`), which is not always correctly
  rounded, changes a few voxels by a few float units: on sMRIPrep's command, 5,522 of
  5,898,240 voxels of ds000005's T1w by at most 2.0e-4 and 5,389 of 11,534,336 of ds000210's
  by at most 2.1e-3; 1 − NCC at most 2.2e-13 in every case, and every difference at most
  2.2e-4 of ANTs' own difference between thread counts;
- against the **same binary run with glibc's `expf`, `exp` (and `logf`, `log`, `sin`, `cos`,
  `sincos`) replaced by CORE-MATH's correctly rounded ones** (`LD_PRELOAD`,
  `scripts/build_crmath_preload.sh`), which is the arithmetic larmorx uses: **52 of 52 outputs
  bit-identical**. ANTs' own code is not changed; only those C-library functions are.
- **ANTs' own result depends on its thread count**, far more than on the C library: with 2
  threads a fifth of the voxels of a T1w change, by up to 97; with 8 or 12 the result also
  changes from run to run. larmorx always gives ANTs' one-thread result.

**Speed** ([benchmark](../benchmarks/ants-denoise.md)): sMRIPrep's command on a raw T1w
(160×192×192) takes 13.0 s on one thread and 2.9 s on 12, against 217 s and 45 s for ANTs. On
the six command lines measured, larmorx is 10.7–17× faster than ANTs on one thread, and
5.8–16× faster than ANTs on 12 threads when both use 12.

## Quick start

```python
import larmorx as lx

# sMRIPrep's call: Rician noise model, patch radius 1, search radius 2, no shrinking
res = lx.ants.denoise_image("sub-01_T1w.nii.gz", n_threads=8)
res.denoised.to_filename("sub-01_desc-denoised_T1w.nii.gz")
noise = res.noise                       # image − denoised

# the Gaussian model, only inside a mask, larger patches
res = lx.ants.denoise_image(t1w, "Gaussian", mask=brain_mask, patch_radius=2)
```

```bash
larmorx ants DenoiseImage -i sub-01_T1w.nii.gz -n Rician -o denoised.nii.gz -s 1
larmorx ants DenoiseImage -d 3 -i t1w.nii.gz -n Rician -x mask.nii.gz \
    -o [ denoised.nii.gz, noise.nii.gz ]
```

## Option mapping

| DenoiseImage | Python | Notes |
|---|---|---|
| `-d 2/3/4` | `dimension` (default: the image's) | a 4D file read with `-d 3` gives its first volume; in 4D, patches and search boxes extend along time too |
| `-i image` | `image` | path, `lx.Image`, nibabel image or `(array, affine)`; read as `float` |
| `-n Rician/(Gaussian)` | `noise_model="Rician"` | **different default**: the command line's is Gaussian, Python's is Rician (sMRIPrep's choice). Case does not matter |
| `-x mask` | `mask` | read as `unsigned char` (0.4 → 0; values beyond 0 to 255 clamped, ANTS-06 below); only voxels where it is not 0 are patch centres. Must be on the grid the filter runs on (the shrunk grid with `-s` above 1) |
| `-s 1` | `shrink_factor=1` | `ShrinkImageFilter` along every axis (time too); 0 acts as 1. The noise is resampled linearly onto the full grid |
| `-p 1` / `-p 1x1x1` | `patch_radius=1` / `(1, 1, 1)` | one value for every axis or one per axis |
| `-r 2` / `-r 2x2x2` | `search_radius=2` / `(2, 2, 2)` | one value for every axis or one per axis |
| `-o out` / `-o [out,noise]` | `DenoiseResult.denoised`, `DenoiseResult.noise` | float32 images on the input's grid; the noise is always returned |
| `-v 1` | – | ANTs' progress characters and the elapsed time |
| `ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS` (or `NSLOTS`, as ITK reads them) | `n_threads` | **ANTs' result depends on it; larmorx's does not**: it always gives ANTs' one-thread result |

Not exposed by ANTs (fixed as in `DenoiseImage`): local mean and variance radius 1, ε = 1e-5,
mean threshold 0.95, variance threshold 0.5, smoothing variance 2. The Rust filter
(`AdaptiveNlmParameters`) takes them all.

## Behaviour worth knowing

From ANTs ([findings](../findings/ants-denoise.md)):
- the output is `input − noise`, with the noise `shrunk − denoised` resampled onto the input's
  grid with float coordinates; so even without shrinking it is not exactly the filter's
  output, and with shrinking, voxels beyond the shrunk grid's extent (more than half a shrunk
  voxel past its edge) get no noise and keep their input values;
- with the Rician model the patches' squared values are averaged and a bias, estimated from
  the smoothed local noise and corrected for the local signal-to-noise ratio, is subtracted
  before the square root; values below it become 0;
- voxels that do not take part (value ≤ 0, local variance ≤ 1e-5, outside the mask) are still
  averaged with themselves through their neighbours' patches, so they change by rounding;
- `-r [2]` (in brackets) reads no radius and exits 1;
- `-o [denoised,noise]` writes the noise first; without `-o` nothing is written.

## Upstream defects (D13)

ANTs' and ITK's deterministic defects are corrected by default and reproduced bit for bit on
request: `upstream_defects="reproduce"` in Python (`N4Options::upstream_defects` /
`DenoiseOptions::upstream_defects` in Rust), `LARMORX_UPSTREAM_DEFECTS=reproduce` on the
command line. A warning (on the command line a line on standard error) names the entry
wherever it changed the result ([upstream-defects.md](upstream-defects.md); each entry in the
[register](../findings/upstream-defects.md)).

| ID | ANTs (what `reproduce` gives) | larmorx's default |
|---|---|---|
| ANTS-43 | at a high local signal-to-noise ratio the Rician correction factor overflows (`float(I₀) · expf(−x)` is `∞ · 0`): the factor is NaN, passes both range checks, and the bias becomes 0 | the factor computed in double with exponentially scaled Bessel functions (finite at any ratio), then ITK's range checks; the warning counts the voxels whose factor ITK makes NaN |
| ANTS-47 | a mask file that cannot be read is no mask: the whole image is denoised, exit 0 | an error (exit 1) |
| ANTS-06 | mask values beyond `unsigned char`'s range are converted as x86-64 converts them (256 → 0, −1 → 255; C++ leaves it undefined) | clamped (256 → 255, −1 → 0), NaN to 0, with a warning |
| ITK-01 | with `-s` above 1, the noise is resampled with ITK's floor, 0 instead of −1 for an index just below 0 (x86-64), in 4D linear interpolation | the exact floor; the warning counts the output voxels this changed |
| ANTS-03, ITK-02, ITK-08 | as for every ANTs program: names of fewer than 3 characters, the direction of a file with more dimensions than `-d`, `x.nii` read for `x.nii.gz` ([ImageMath](ants-image-math.md#upstream-defects-d13)) | as there |

On sMRIPrep's command (`-n Rician`) ANTS-43 is common: on two real T1w images (ds000005,
ds000210) ITK's factor is NaN in 7.4 % and 8.4 % of the voxels, and the corrected bias lowers
them by a median of 0.58 and 2.9, at most 2.8 and 17.7 (0.7 % of each image's 99th
percentile); ANTs' own result moves far more between thread counts (ANTS-42).

## Deliberate differences

- **Threads**: the result does not depend on `n_threads` and equals ANTs' single-thread
  result. ANTs' changes with its thread count, by up to 271 in a T1w whose values reach
  1,850, and with 8 or more threads from run to run.
- **Maths**: `expf` and `exp` are correctly rounded (CORE-MATH), the same on every platform;
  glibc's are not always, so ANTs as built differs in a few float units in some voxels
  ([findings](../findings/ants-denoise.md#glibcs-math-in-denoiseimage)).
- Where ANTs aborts or crashes (an unreadable input, a value that is not a number), larmorx
  exits 1 with a message.
- `-v` prints ANTs' progress characters but not ITK's dump of the filter object.
- Images are NIfTI only.

## Rust

```rust
use larmorx_ants::denoise_image::{denoise_image, DenoiseOptions, NoiseModel};

let options = DenoiseOptions { noise_model: NoiseModel::Rician, ..DenoiseOptions::default() };
let out = denoise_image(&input, None, &options, n_threads)?;
// out.denoised, out.noise
```

The filter itself is `larmorx_image::adaptive_denoising::adaptive_non_local_means`, on any
1-4D volume with an optional mask; `larmorx_image::neighborhood::{mean_filter,
variance_filter}` are ITK's box mean and AdaptiveDenoising's box variance.
