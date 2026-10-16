# Upstream defects: corrected by default, reproduced on request

Some of the programs larmorx reproduces have deterministic defects that change their results:
AFNI's 3dTshift flips the sign of data with a negative `scl_slope`, mcflirt starts the last
volume of a series from the identity, fMRIPrep differentiates its field maps along the wrong
axis in standard space. larmorx corrects such defects by default and says so when a correction
changed a result; a compatibility switch reproduces the original bit for bit (decision D13,
[decision log](../decision-log.md), 2026-10-11). Every defect found is in the
[register](../findings/upstream-defects.md), with an ID (`FSL-01`, `PIPE-03`, ...), its
evidence and what it does to fMRIPrep users.

Crashes, out-of-bounds reads, races and nondeterminism are never reproduced: larmorx gives a
defined result or refuses with a message, whatever the switch says
([porting guide](../porting-guide.md) §8).

## Choosing

| Interface | Default | Reproduce the original |
|---|---|---|
| Python | `upstream_defects="fix"` | `upstream_defects="reproduce"` on the call |
| Command line | `LARMORX_UPSTREAM_DEFECTS` unset, empty or `fix` | `LARMORX_UPSTREAM_DEFECTS=reproduce` |
| Rust | `UpstreamDefects::Fix` (`Default`) | `UpstreamDefects::Reproduce` in the tool's options |

```python
import larmorx as lx

img = lx.afni.tshift("bold.nii.gz", slice_times="alt+z")                    # corrected
afni = lx.afni.tshift("bold.nii.gz", slice_times="alt+z", upstream_defects="reproduce")
```

```bash
LARMORX_UPSTREAM_DEFECTS=reproduce larmorx mri hmc -in bold.nii.gz -reffile ref.nii.gz -mats
```

- **One switch per call.** It restores all of a tool's reproduced defects at once: comparing
  with the original needs all of them together.
- **The command line reads an environment variable**, as it reads `LARMORX_IMPLEMENTATION`,
  so that no flag can clash with the original program's arguments. Any value other than `fix`,
  `reproduce` or empty is an error (exit code 2). The Python API ignores the variable (no
  global configuration, CLAUDE.md rule 6); `larmorx.cli.run`, which runs the command line in
  process, reads it.
- **Replicas** receive the same variable: `lx.afni.tshift(..., implementation="replica")` sets
  `LARMORX_UPSTREAM_DEFECTS` for the `larmorx-gpl` program, and `larmorx afni 3dTshift` passes
  its environment on. Replicas share the corrected default, and with `reproduce` give exactly
  the original's result (decision log, "D13's details"): the 3dTshift replica corrects the same
  defects as the clean-room original by default (2026-10-12), and with `reproduce` writes
  AFNI's output bytes.

## Warnings

A defect is named when it changed this result (`fix`), or when the correction would have
changed it (`reproduce`); nothing is said when the two would agree.

- **Python:** `lx.UpstreamDefectWarning`, a `UserWarning` with the attributes `entry`
  (`"FSL-01"`), `tool`, `mode` (`"fix"` or `"reproduce"`) and `detail`:

  ```python
  import warnings
  warnings.filterwarnings("ignore", category=lx.UpstreamDefectWarning)   # silence them
  warnings.filterwarnings("error", category=lx.UpstreamDefectWarning)    # or make them errors
  ```

- **Command line:** one line on standard error per defect,

  ```
  larmorx: upstream defect FSL-01 (mcflirt) corrected: the last volume (239) starts the first
  stage from volume 238's result, like the others; mcflirt starts it from the identity
  [LARMORX_UPSTREAM_DEFECTS=reproduce gives mcflirt's result]
  ```

  (on one line; `larmorx afni 3dTshift` writes it as an AFNI warning, `*+ WARNING: upstream
  defect ...`). The Python wrappers turn such lines from a replica into
  `UpstreamDefectWarning`s.

## What is corrected

| ID | Original | larmorx | The original's defect | larmorx's default |
|---|---|---|---|---|
| PIPE-03 | fMRIPrep `ResampleSeries` | `lx.transforms.resample_series` | the field map's Jacobian is the gradient along the *target* axis with the source's phase-encoding index, in target voxels: right only when the target's axes are the source's (the native boldref) | `1 + ∇vsm · ∂t/∂s_pe`: the derivative along the phase-encoding direction mapped into the target grid; on the source's own grid, fMRIPrep's value bit for bit ([resample-series.md](resample-series.md)) |
| PIPE-04 | nitransforms | `lx.transforms.load_itk_composite`, `load_transforms`, `resample_series` | a composite `.h5` file's transforms are read in name order (`"10"` before `"2"`) | numeric order |
| PIPE-17 | nitransforms | `lx.transforms.load_itk_composite`, `load_transforms` | the first group of an `.h5` file is skipped as if it were the composite: a file of one transform fails, one of several without a composite loses its first, an empty composite fails | the first group read when it is not a composite; an empty composite is the identity |
| HDF-01 | HDF5 1.12 | `lx.transforms.read`, `-t x.h5` (non-native floating-point parameters) | conversion state carried from one element to the next (denormals) | each element converted on its own ([transforms-hdf5.md](transforms-hdf5.md)) |
| SCI-03 | SciPy `ndimage` | `lx.ndimage.*`, `resample_series(mode=...)` | the spline prefilter's reflect boundary (modes `reflect`, `grid-mirror`, `nearest`) reads a coefficient it has already updated: the splines miss the samples on short axes (by up to 1e-2 on a 6-voxel axis, order 5) | the exact half-sample-symmetric initialisation: splines through the samples |
| AFNI-01 | 3dTshift | `lx.afni.tshift`, `larmorx afni 3dTshift` (both implementations) | a negative `scl_slope` is applied on output only: shifted slices come out sign-flipped and scaled | the factor is applied on input too |
| AFNI-02 | 3dTshift | same | float32 data with a slope and an intercept are scaled, and the slope is kept as a scale factor: scaled twice | scaled once, no factor |
| AFNI-03 | 3dTshift | `larmorx afni 3dTshift` (header slice timing) | header slice code 2 (sequential decreasing) gives every slice time 0: nothing is shifted | slices `slice_end` down to `slice_start`, as NIfTI defines the code |
| AFNI-05 | 3dTshift | `lx.afni.tshift(detrend=False)`, `-no_detrend` | only the first voxel of each pair has its mean removed and restored; the second is shifted raw (its edges fall toward 0) | every voxel has its mean removed and restored |
| AFNI-07 | 3dTshift | both | metre and micron units: the voxel sizes are converted to millimetres, the origin is not, and an oblique sform not at all | the whole matrix is converted |
| AFNI-08 | 3dTshift | `larmorx afni 3dTshift` | a copy of the input whose kept slice times are all within 3 ms of 0 is written with `slice_end` 0 | the defaults: no slice code, `slice_end` = nz − 1 (with the slice fields AFNI derives from the times otherwise) |
| AFNI-11 | 3dTshift | `larmorx afni 3dTshift` | an existing output is not written, exit 0 | an error, exit 1 (`reproduce` writes nothing and exits 0) |
| AFNI-12 | 3dTshift | `larmorx afni 3dTshift` | a `-prefix` with blanks, shell metacharacters or non-ASCII characters is refused; one that passes only through AFNI's exception for names beginning with `3dcalc` is refused when written (nothing written, exit 0) | any name is written, with a warning when AFNI would refuse it (`reproduce` refuses it at both checks) |
| AFNI-16 | 3dTshift | both | a grid within about 0.01° to 0.024° of plumb (a single-precision test) is written exactly plumb, its tilt discarded | the input's matrix is kept, sform and qform (decided 2026-10-12; `reproduce` writes AFNI's plumb matrix, byte for byte) |
| AFNI-18 | 3dTshift | `larmorx afni 3dTshift` | an input name with a blank is read as a list of datasets to join in time (unless `AFNI_PATH_SPACES_OK` is yes), so an input in a directory whose name has a blank cannot be opened | the name is one file (`reproduce` reads AFNI's list) |
| AFNI-19 | 3dTshift | `larmorx afni 3dTshift` | with `AFNI_ALLOW_MILLISECONDS` yes and `-TR` in milliseconds, `toffset` is written in milliseconds, labelled seconds | `toffset` in seconds, with the TR |
| AFNI-20 | 3dTshift | `larmorx afni 3dTshift` | a dataset of one row (one voxel along y and z) is written as a 1D text file under the NIfTI name, unless `AFNI_WRITE_1D_AS_PREFIX` is yes | NIfTI, in both modes: AFNI's file holds a random identifier, so it is not reproduced; the warning says what AFNI would do |
| AFNI-22 | 3dTshift | `larmorx afni 3dTshift` | `-Dname=value` with blanks in the value or around `=` sets another value (`-DX=a b` sets `X` to `a=b`) | the value as written, after the first `=` |
| AFNI-23 | 3dTshift | both, with `AFNI_NIFTI_SCALE=NO` (command line, or `lx.afni.tshift(afni_settings=...)`) | uint8 and int16 data with a slope and an intercept stay integers with the slope as their scale factor, and the intercept is dropped: the values are off by it | AFNI's integers and `scl_slope` kept, the intercept written as `scl_inter`; with `-rlt -ignore`, the data read as without the variable (decided 2026-10-12) |
| AFNI-26 | 3dTshift | `larmorx afni 3dTshift` | an output that cannot be written (its directory is missing or cannot be written, the file is read-only, or a directory has its name) is not written, and the exit status is 0 | an error, exit 1 (`reproduce` prints AFNI's message, writes nothing and exits 0: both implementations, since 2026-10-12) |
| FSL-01 | mcflirt | `lx.mri.hmc`, `larmorx mri hmc` | the last volume of the series starts the first stage from the identity instead of its predecessor's result (every fMRIPrep run's last volume) | it continues its chain like every other volume |
| FSL-03 | mcflirt | both | the in-plane mode pads the first stage's reference grid twice | once, like the other stages |
| FSL-14 | mcflirt | both | one NaN or infinite voxel makes the volume's centre of mass, and so its matrix, NaN | non-finite voxels are left out of the centre of mass and the cost |
| FSL-05 | mcflirt | `larmorx mri hmc` | `-scaling` and `-hist` are accepted and ignored | still ignored (mcflirt gives them no meaning), with a warning |
| FSL-06 | mcflirt | `larmorx mri hmc` | an unknown `-cost` silently uses normcorr | an error (exit 255) |
| FSL-07 | mcflirt | `larmorx mri hmc` | numbers are read with C's `atoi`/`atof` (`abc` is 0, `1.5` is 1 for integers); `-help` alone exits 1 | anything but a number is an error (exit 255); `-help` alone exits 0 |
| FSL-08 | mcflirt | `larmorx mri hmc -stats` | the `-stats` images leave the first and last slices at 0, and `-stats`/`-meanvol` images are named after `-out` with its extension inside (`x.nii.gz_meanvol.nii.gz`) | every slice computed; `x_meanvol.nii.gz` (the names nipype expects) |
| FSL-09 | mcflirt | `larmorx mri hmc` | int8 input is written as uint8; integer outputs are truncated toward zero, and values beyond the type's range wrap around (x86-64; C++ leaves it undefined) | int8 stays int8; values are rounded to the nearest (half to even) and clamped |
| FSL-10 | mcflirt | `larmorx mri hmc` | the output's units are always millimetres and seconds, so a TR in milliseconds is labelled seconds | the input's units are kept |
| FSL-13 | mcflirt | `larmorx mri hmc` | `-rmsrel` and `-rmsabs` create the `.mat` directory without `-mats` | no `.mat` directory without `-mats` |
| SCI-02 | SciPy `ndimage` | `lx.ndimage.*` | NaN results written to a signed integer output, and a `cval` an integer input cannot hold, are converted as x86-64 converts them (C leaves it undefined) | NaN is 0, `cval` clamped |

**ANTs and ITK** (2026-10-12; the tables of each tool's page say which apply to it:
[ImageMath](ants-image-math.md#upstream-defects-d13),
[ThresholdImage](ants-threshold-image.md#upstream-defects-d13),
[MultiplyImages](ants-multiply-images.md#upstream-defects-d13),
[SmoothImage](ants-smooth-image.md#upstream-defects-d13),
[ResampleImageBySpacing](ants-resample-image-by-spacing.md#upstream-defects-d13),
[ResampleImage](ants-resample-image.md#upstream-defects-d13),
[N4BiasFieldCorrection](ants-n4.md#upstream-defects-d13),
[DenoiseImage](ants-denoise.md#upstream-defects-d13),
[antsApplyTransforms](ants-apply-transforms.md#upstream-defects-d13)):

| ID | Original | The original's defect | larmorx's default |
|---|---|---|---|
| ANTS-03 | ANTs I/O | file names of fewer than 3 characters are neither read nor written, silently | read and written like any other |
| ANTS-04 | ImageMath | exit 0 after an operation that failed and wrote nothing | exit 1 |
| ANTS-05 | ImageMath | `1e` and `-` are the number 0, `1e40` is `FLT_MAX`, no operand is 1 | an error (no operand allowed where 1 means the image alone) |
| ANTS-06 | ANTs programs | values out of an integer type's range, and NaN, wrap around (x86-64's answer; C++ leaves it undefined) | clamped, NaN to 0, with a warning |
| ANTS-07 | ImageMath, MultiplyImages | a smaller second image is read past its voxels | an error (`reproduce`: read as ANTs where it stays inside the buffer, 0 beyond) |
| ANTS-08 | ImageMath `/` | divides only where the divisor is positive; other voxels keep the previous result | divides wherever the divisor is not 0, 0 where it is |
| ANTS-09 | ImageMath `total`, `mean` | the image holds running sums | every voxel holds the printed value |
| ANTS-10 | ImageMath `vtotal` | an image of zeros | an error |
| ANTS-11, ANTS-12, ANTS-13 | ImageMath `Neg`, `TruncateImageIntensity`, `Normalize` | ranges found with `else if` or from fixed starts | the true ranges |
| ANTS-14 | ImageMath `G` | NaN everywhere above a variance of about 709 voxels² | a finite kernel (scaled Bessel functions) |
| ANTS-15 | ImageMath `G`, SmoothImage | a sigma list of the wrong length is not an error | an error |
| ANTS-16 | ImageMath `Laplacian` | the normalize flag read from the sigma's argument | from its own argument |
| ANTS-19 | ImageMath `FillHoles` | `p ≤ 1` counts neighbours outside the image (memory beyond it on the end slices) | outside neighbours not counted |
| ANTS-20 | ImageMath `GetLargestComponent` | nothing kept: every voxel 1 | every voxel 0 |
| ANTS-24 | ThresholdImage | an unknown mode is the lower bound `atof(mode)` | an error |
| ANTS-25 | ThresholdImage Otsu with a mask | range from `FLT_MIN` with `else if` | the true range |
| ANTS-27 | MultiplyImages | an unreadable second file multiplies by 0 | an error |
| ANTS-28 | ResampleImageBySpacing, SmoothImage | uninitialised memory reaches the output | ITK's error (`reproduce`: the buffer as zeros) |
| ANTS-29 | ResampleImage | size 1: an image of zeros, infinite spacing, NaN origin | an error (`reproduce`: ANTs' file byte for byte) |
| ANTS-30 | ResampleImage | the seventh argument is both the pixel type and the interpolator's parameter | parameters after the interpolator, the pixel type after them ([design](ants-resample-image.md#the-seventh-argument)) |
| ANTS-31 | ResampleImageBySpacing | `nn` read from the `addvox` argument; other dimensions do nothing | each from its own argument; other dimensions an error |
| ANTS-33 | antsApplyTransforms | unknown `-u` kept as default; `MultiLabel`'s alpha and `GenericLabel`'s interpolator ignored | an error; honoured |
| ANTS-35 | N4BiasFieldCorrection | `-c`, `-b`, `-t` without brackets set nothing | the value is the first parameter |
| ANTS-36 | N4BiasFieldCorrection | an unreadable mask becomes ones | an error |
| ANTS-38 | N4 (ITK) | a first-voxel minimum is left out of the histogram | the true range |
| ANTS-39 | N4 (ITK) | 64, 128 or 256 bins padded to twice the FFT size needed | the exact size |
| ANTS-43 | DenoiseImage (ITK) | the Rician correction is NaN, and the bias 0, at a high SNR | a finite factor |
| ANTS-47 | DenoiseImage | an unreadable mask is no mask | an error |
| ANTS-49 | antsRegistration | a linear stage that throws is dropped, exit 0 | an error |
| ANTS-50 | antsRegistration | `--save-state` after linear stages: exit 1, nothing written | the state is written |
| ANTS-51 | antsRegistration | `-z 0`: files misnamed, then a read past the end of the names (crash) | every computed transform under its stage's name (`reproduce`: ANTs' files up to the crash, then an error) |
| ITK-01 | ITK `Math::Floor` | 0 instead of −1 just below 0 (x86-64) | the exact floor |
| ITK-02 | ITK reader | extra dimensions read with an identity direction | the file's direction, where it applies |
| ITK-03 | ITK relabelling | float labels stop at 2^24 | integer labels |
| ITK-08 | nifti_clib | asked for `x.nii.gz`, the voxels of `x.nii` | the named file (`reproduce`: nifti_clib's) |
| ITK-10 | ITK `Versor<float>` | `--float 1` refuses some valid versors | they load |
| ITK-12 | ITK HDF5 reader | a fixed-length `TransformType` string is refused | read |
| ITK-13 | ITK HDF5 reader | messages that name `TransformType` for any dataset and miss a blank | the dataset named |
| ITK-14 | ITK HDF5 reader | parameters wider than 8 bytes read through float | read in the reader's precision |
| ITK-15 | ITK HDF5 writer | a transform without parameters cannot be written with compression; a file ITK cannot read is left | its empty parameters stored uncompressed (`reproduce`: ITK's error and file) |
| ITK-18 | ITK joint-histogram MI (`MI2`) | the marginal densities swapped | each image's own marginal |

**Not the default yet:**

- **FSL-02** (mcflirt's correlation count). mcflirt's normalised correlation divides by a count
  many orders of magnitude larger than the number of points, which makes it effectively
  uncentred. The corrected count, the sum of the weights, is available as
  `lx.mri.hmc(..., normcorr_count="weights")`; whether it becomes the default is decided with
  the user once its effect is measured ([mri-hmc.md](mri-hmc.md), "The correlation count").
Both 3dTshift implementations have the switch for every AFNI entry above since 2026-10-12:
the clean-room original followed the replica that day (completing 3dTshift, part 2b, from
`specs/3dTshift.md` §6.1 to §6.5 and §7.1), and with `reproduce` gives AFNI's output, its
values within the original's thresholds (the last bits of its own numerics) and everything
else byte for byte.

## How the switch is validated

- **Parity** compares larmorx with the original in `reproduce` mode, where bit-identity held
  before D13 (`LARMORX_UPSTREAM_DEFECTS=reproduce` for the command lines,
  `upstream_defects="reproduce"` for the Python API; the stage-by-stage comparison with
  fMRIPrep too, since it compares with fMRIPrep as it ran). A second pass runs the default and
  reports, case by case, which defects were named and what the corrections changed. It also
  checks the rule above: a defect is named exactly when the two passes differ (`D13 fix pass`
  in the records: [afni-tshift.md](../validation/afni-tshift.md),
  [mri-hmc.md](../validation/mri-hmc.md), [resample-series.md](../validation/resample-series.md),
  and every ANTs record, such as [ants-image-math.md](../validation/ants-image-math.md)).
  Where the original's output depends on memory it never wrote, the comparison covers
  everything else (exit code, header, printed text) and the record says so.
- **Golden cases** record the default outputs, and a small `*.reproduce` case per affected
  tool records the reproduced ones ([validation/golden.md](../validation/golden.md)). The
  reproduced outputs equal the checksums recorded before D13.

## For tool authors

- Take `larmorx_core::defects::UpstreamDefects` in the tool's options (a field named
  `upstream_defects`) and return `Vec<larmorx_core::defects::Notice>` with the result.
- Add the defect to `larmorx_core::defects` (`defects!` table: ID, tool, the original's
  behaviour in a few words, whether `fix` corrects it). For AFNI and FSL entries describe
  observable behaviour only: the clean-room originals use the table.
- Detect the triggering condition and emit a notice only when the result differs, in both
  modes; print `Notice::line()` on standard error from the command line; in Python, raise the
  notices with `larmorx._defects.warn` and take `upstream_defects: UpstreamDefectsName = "fix"`.
- The command line reads the mode with `UpstreamDefects::from_env()`; an invalid value exits
  with `larmorx_core::defects::EXIT_BAD_MODE` (2).
- Parity: reproduce for the comparison, a fix pass with
  `larmorx_validation.parity.defects.add_fix_check`; golden: a `*.reproduce` case.
- Document it in the tool's API page and update the entry's "larmorx" line in the register.
