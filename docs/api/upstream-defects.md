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
| AFNI-02 | 3dTshift, 3dAutomask, 3dUnifize | same | float32 data with a slope and an intercept are scaled, and the slope is kept as a scale factor: scaled twice | scaled once, no factor |
| AFNI-03 | 3dTshift | `larmorx afni 3dTshift` (header slice timing); 3dAutomask's `-apply_prefix` and 3dUnifize `-EPI` keep the corrected slice fields (both implementations) | AFNI 25.2.09: header slice code 2 (sequential decreasing) gives every slice time 0: nothing is shifted. **Fixed in AFNI 26.3.05** (the replicas follow it in both modes) | slices `slice_end` down to `slice_start`, as NIfTI defines the code |
| AFNI-05 | 3dTshift | `lx.afni.tshift(detrend=False)`, `-no_detrend` | AFNI 25.2.09: only the first voxel of each pair has its mean removed and restored; the second is shifted raw (its edges fall toward 0). **Fixed in AFNI 26.3.05** (the replica follows it in both modes) | every voxel has its mean removed and restored |
| AFNI-07 | 3dTshift, 3dAutomask, 3dUnifize | both | metre and micron units: the voxel sizes are converted to millimetres, the origin is not, and an oblique sform not at all | the whole matrix is converted |
| AFNI-08 | 3dTshift | `larmorx afni 3dTshift` | a copy of the input whose kept slice times are all within 3 ms of 0 is written with `slice_end` 0 | the defaults: no slice code, `slice_end` = nz − 1 (with the slice fields AFNI derives from the times otherwise) |
| AFNI-11 | 3dTshift, 3dAutomask, 3dUnifize | `larmorx afni 3dTshift`, `3dAutomask`, `3dUnifize` | an existing output is not written, exit 0 | an error, exit 1 (`reproduce` writes nothing and exits 0) |
| AFNI-12 | 3dTshift, 3dAutomask, 3dUnifize | `larmorx afni 3dTshift`, `3dAutomask`, `3dUnifize` | a `-prefix` with blanks, shell metacharacters or non-ASCII characters is refused; one that passes only through AFNI's exception for names beginning with `3dcalc` is refused when written (nothing written, exit 0) | any name is written, with a warning when AFNI would refuse it (`reproduce` refuses it at both checks) |
| AFNI-16 | 3dTshift, 3dAutomask, 3dUnifize | both | a grid within about 0.01° to 0.024° of plumb (a single-precision test) is written exactly plumb, its tilt discarded | the input's matrix is kept, sform and qform (decided 2026-10-12; `reproduce` writes AFNI's plumb matrix, byte for byte) |
| AFNI-18 | 3dTshift, 3dAutomask, 3dUnifize | `larmorx afni 3dTshift`, `3dAutomask`, `3dUnifize` | an input name with a blank is read as a list of datasets to join in time (unless `AFNI_PATH_SPACES_OK` is yes), so an input in a directory whose name has a blank cannot be opened | the name is one file (`reproduce` reads AFNI's list) |
| AFNI-19 | 3dTshift | `larmorx afni 3dTshift` | with `AFNI_ALLOW_MILLISECONDS` yes and `-TR` in milliseconds, `toffset` is written in milliseconds, labelled seconds | `toffset` in seconds, with the TR |
| AFNI-20 | 3dTshift | `larmorx afni 3dTshift` | a dataset of one row (one voxel along y and z) is written as a 1D text file under the NIfTI name, unless `AFNI_WRITE_1D_AS_PREFIX` is yes | NIfTI, in both modes: AFNI's file holds a random identifier, so it is not reproduced; the warning says what AFNI would do |
| AFNI-22 | 3dTshift | `larmorx afni 3dTshift` | `-Dname=value` with blanks in the value or around `=` sets another value (`-DX=a b` sets `X` to `a=b`) | the value as written, after the first `=` |
| AFNI-23 | 3dTshift, 3dAutomask, 3dUnifize | both, with `AFNI_NIFTI_SCALE=NO` (command line, or `lx.afni.tshift(afni_settings=...)`) | uint8 and int16 data with a slope and an intercept stay integers with the slope as their scale factor, and the intercept is dropped: the values are off by it | AFNI's integers and `scl_slope` kept, the intercept written as `scl_inter`; with `-rlt -ignore`, the data read as without the variable (decided 2026-10-12) |
| AFNI-26 | 3dTshift, 3dAutomask, 3dUnifize | `larmorx afni 3dTshift`, `3dAutomask`, `3dUnifize` | an output that cannot be written (its directory is missing or cannot be written, the file is read-only, or a directory has its name) is not written, and the exit status is 0 | an error, exit 1 (`reproduce` prints AFNI's message, writes nothing and exits 0: both implementations, since 2026-10-12) |
| AFNI-43 | 3dAutomask | `lx.afni.automask(peels=0)`, `larmorx afni 3dAutomask -peels 0` (a value read as 0; a negative one peels once, as in AFNI) | peels once | no peeling, as the help says |
| AFNI-41 | 3dAutomask | `larmorx afni 3dAutomask -nbhrs n`, `lx.afni.automask(neighbours=n)` with n outside 6..26 | 17 used silently | an error (exit 1, `ValueError`); `reproduce` uses 17 (decided 2026-10-23) |
| AFNI-48 | 3dUnifize | `larmorx afni 3dUnifize -T2 -T2 -T2` | neither the inversion back nor the edge clipping: the output stays inverted | an error (exit 1); `reproduce` gives AFNI's output (decided 2026-10-23); `lx.afni.unifize` accepts `t2` 0 to 2 only |
| AFNI-45 | 3dUnifize | `lx.afni.unifize(t2=True, t2_percentile=100)`, `-T2up` near 100 | the percentile is read one past the sorted values (memory never written; AFNI read 0 and wrote zeros) | the largest value (the 100th percentile); `reproduce` uses 0 |
| AFNI-47 | 3dUnifize | inputs of more than a million voxels with an axis of 2 or 3 | the half-size scale image is read past its end | brought back to the input's grid; `reproduce` reads AFNI's image, 0 past its end |
| AFNI-01 | 3dAutomask, 3dUnifize `-EPI` | a series with a negative `scl_slope` | the factor dropped when the series is averaged (3dAutomask) or its median taken (3dUnifize `-EPI`: all zeros) | the factor applied |
| AFNI-75 | 3dUnifize `-EPI` | `larmorx afni 3dUnifize -EPI` (the GPL replica) | the ball of neighbours is sized from the voxel size, unbounded: beyond any machine's memory on micrometre voxels | the same computation; by default the replica stops before building the ball when it exceeds the machine's memory (or `LARMORX_MEMORY_LIMIT`), naming AFNI-75; `reproduce` attempts it (the clean-room original: to do, from its spec) |
| FSL-01 | mcflirt | `lx.mri.hmc`, `larmorx mri hmc` | the last volume of the series starts the first stage from the identity instead of its predecessor's result (every fMRIPrep run's last volume) | it continues its chain like every other volume |
| FSL-03 | mcflirt | both | the in-plane mode pads the first stage's reference grid twice | once, like the other stages |
| FSL-14 | mcflirt | both | one NaN or infinite voxel makes the volume's centre of mass, and so its matrix, NaN | non-finite voxels are left out of the centre of mass and the cost |
| FSL-05 | mcflirt | `larmorx mri hmc` | `-scaling` and `-hist` are accepted and ignored | still ignored (mcflirt gives them no meaning), with a warning |
| FSL-06 | mcflirt | `larmorx mri hmc` | an unknown `-cost` silently uses normcorr | an error (exit 255) |
| FSL-07 | mcflirt | `larmorx mri hmc` | numbers are read with C's `atoi`/`atof` (`abc` is 0, `1.5` is 1 for integers); `-help` alone exits 1 | anything but a number is an error (exit 255); `-help` alone exits 0 |
| FSL-08 | mcflirt | `larmorx mri hmc -stats`, `-meanvol` | the `-stats` images leave the first and last slices at 0, and `-stats`/`-meanvol` images are named after `-out` with its extension inside (`x.nii.gz_meanvol.nii.gz`) | every slice computed; `x_meanvol.nii.gz` (the names nipype expects) |
| FSL-09 | mcflirt | `larmorx mri hmc` | int8 input is written as uint8; integer outputs are truncated toward zero, and values beyond the type's range wrap around (x86-64; C++ leaves it undefined) | int8 stays int8; values are rounded to the nearest (half to even) and clamped |
| FSL-10 | mcflirt | `larmorx mri hmc` | the output's units are always millimetres and seconds, so a TR in milliseconds is labelled seconds | the input's units are kept |
| FSL-13 | mcflirt | `larmorx mri hmc` | `-rmsrel` and `-rmsabs` create the `.mat` directory without `-mats` | no `.mat` directory without `-mats` |
| FSL-04 | mcflirt | `larmorx fsl mcflirt` (the replica; the original refuses `-gdt` in both modes) | `-gdt` registers each volume's gradient image to the reference's plain image | refused (exit 1); `reproduce` registers the gradient images as mcflirt does |
| FSL-20 | mcflirt | `larmorx fsl mcflirt` (the replica) | when the `.mat` directory cannot be created for another reason than its permissions, mcflirt prints "Error!: Unknown Log directory return value" and exits with status 0, writing nothing more | an error (exit 1) |
| FSL-21 | mcflirt | `larmorx fsl mcflirt` (the replica) | with a histogram cost and `-bins` below 8, the first stage's histogram has no bins: arbitrary matrices, and `corratio` aborts | an error before any work (exit 1) |
| FSL-40 | bet2 | `larmorx fsl bet2`, `bet`, `lx.fsl.bet` | the first `.hdr` of a name is removed wherever it stands; a name with `.img` and no `.hdr` aborts | a trailing image extension removed |
| FSL-41 | bet2 | `larmorx fsl bet2`, `bet`, `lx.fsl.bet` | FSL 6.0.7.17: a constant or binary image crashed reading the median of an empty list. **Fixed in FSL 6.0.7.23** (the robust maximum; the replica follows it in both modes) | the robust maximum, as FSL 6.0.7.23 |
| FSL-42 | bet2 | `larmorx fsl bet2`, `bet`, `lx.fsl.bet` | NaN `-f`/`-g`, a negative, NaN or infinite `-r`, `-w` ≤ 0, a non-finite `-c` or search depth used as given | an error naming the value (exit 1; decided 2026-10-24, rule B) |
| FSL-43 | bet2, betsurf | both programs | a mask's fill starts at the surface's centroid unchecked: one voxel outside the image (a single slice) FSL writes and reads outside the volume; further out it crashes | the fill from the nearest voxel inside; `reproduce` FSL's fill by the image's linear index (memory beyond the volume read as not part of the mask), and skips a fill FSL crashes |
| FSL-44 | bet | `larmorx fsl bet` | names with blanks split into words; an empty argument ends the options | each argument one word |
| FSL-45 | bet `-Z` | `larmorx fsl bet -Z`, `lx.fsl.bet(variation="small_fov")` | the outputs cropped back by `imglob` from the PATH: left padded when FSL is not on it | exactly its outputs cropped |
| FSL-46 | bet | `larmorx fsl bet` | every `<output>_tmp*` file removed | only the run's own |
| FSL-47 | betsurf `--t1only` | `larmorx fsl betsurf -1 -m`, `lx.fsl.betsurf(masks=True)` | the scalp mask filled from the outer skull surface's centre in the padded grid | from the scalp surface's |
| FSL-48 | betsurf | `larmorx fsl betsurf`, `lx.fsl.betsurf(t2=...)` | a T2 on another grid sampled at the T1's voxel coordinates | an error (decided 2026-10-24, rule B) |
| FSL-50 | betsurf | both | one profile with an empty search range ends the program (255, no message, no output) | the profile dropped |
| FSL-51 | betsurf | both | profile searches read past the samples, into memory never set | the profile dropped; `reproduce` reads "nothing there" and warns |
| FSL-52 | betsurf | both | a mesh file read without checks: FSL crashes on values it never set or faces naming missing vertices | an error naming the problem; `reproduce` takes 0 for the values never set, leaves out those faces and writes the outputs |
| FSL-53 | betsurf | both | a matrix with fewer than 12 readable numbers: entries never set | an error (decided 2026-10-24, rule B); `reproduce` uses 0 |
| FSL-54 | betsurf `--t1only` | `larmorx fsl betsurf -1` | exit status 0 after a failure | the failure's status (255) |
| FSL-55 | betsurf | both | NaN inside the brain surface: the 98th percentile undefined | an error (decided 2026-10-24, rule B) |
| FSL-56 | FSL's NIfTI writer | every FSL replica (`larmorx fsl bet2`, `bet`, `betsurf`, `mcflirt`) with `FSL_LOAD_NIFTI_EXTENSIONS` | several extensions of different sizes: a wrong `vox_offset` | the correct offset |
| FSL-58 | betsurf | both | no voxel of the T1 inside the brain surface: FSL reads the 98th percentile of an empty list and crashes | an error naming FSL-58; `reproduce` takes it as 0 |
| FSL-59 | mcflirt | `larmorx fsl mcflirt -sinc_final`, `-nn_final`, `lx.mri.hmc(implementation="replica", final="sinc"|"nearest")` | FSL 6.0.7.23 resamples trilinearly whatever the option | the interpolation asked for; `reproduce` trilinear |
| FSL-60 | FSL's NIfTI layer | every image name of `larmorx fsl ...` holding `.img` before its extension | FSL 6.0.7.23 opens the name with its last `.img` made `.hdr` | the name given; `reproduce` FSL's renamed name |
| FSL-61 | bet2 | `larmorx fsl bet2 --binaryMask=<mask>` with a mask on another grid | FSL reads and writes outside the mask | an error in both modes |
| FSL-62 | bet2 | `larmorx fsl bet2 --ct --binaryMask=<mask>` | FSL writes the mask, masked, as the brain image | the CT input masked; `reproduce` FSL's |
| FSL-63 | bet | `larmorx fsl bet <in> <out> -R -e ...`, `lx.fsl.bet(..., variation="robust", mesh=True)` | with `-R`, a leading `-e` is dropped: no mesh | the options as given (the mesh written); `reproduce` FSL's |
| SCI-02 | SciPy `ndimage` | `lx.ndimage.*` | NaN results written to a signed integer output, and a `cval` an integer input cannot hold, are converted as x86-64 converts them (C leaves it undefined) | NaN is 0, `cval` clamped |

Rows naming `larmorx fsl mcflirt` are the replica's (`larmorx-nc`, `lx.mri.hmc(implementation="replica")`); it shares the clean-room original's corrections of FSL-01, -03, -05 to -10, -13 and -14. Rows FSL-40 to FSL-58 are BET's replica's (`larmorx-nc`: `larmorx fsl bet`, `bet2`, `betsurf`, `lx.fsl.bet`, `lx.fsl.betsurf`; there "both" means the command line and the Python function); BET has no clean-room original yet. FSL-49 and FSL-57 are kept as FSL's (rule C).

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
| ANTS-17 | ImageMath `ME`, `MO` | `ME r v` on a label map keeps every other label as 1; `MO r 0` is a dilation of the zeros | the eroded mask of `v`; the zeros opened |
| ANTS-19 | ImageMath `FillHoles` | `p ≤ 1` counts neighbours outside the image (memory beyond it on the end slices) | outside neighbours not counted |
| ANTS-20 | ImageMath `GetLargestComponent` | nothing kept: every voxel 1 | every voxel 0 |
| ANTS-22 | ImageMath `PadImage` | a fractional pad grows by ⌊2p⌋ and shifts by ⌊p⌋ (a negative one also misplaces the data by a voxel) | `trunc(p)` voxels on each side |
| ANTS-23 | ImageMath `ExtractContours` | labels are the truncated values (1.2 and 1.7 one label, 0.6 background) | every value its own label |
| ANTS-24 | ThresholdImage | an unknown mode is the lower bound `atof(mode)` | an error |
| ANTS-25 | ThresholdImage Otsu with a mask | range from `FLT_MIN` with `else if` | the true range |
| ANTS-27 | MultiplyImages | an unreadable second file multiplies by 0 | an error |
| ANTS-28 | ResampleImageBySpacing, SmoothImage | uninitialised memory reaches the output | ITK's error (`reproduce`: the buffer as zeros) |
| ANTS-29 | ResampleImage | size 1: an image of zeros, infinite spacing, NaN origin | an error (`reproduce`: ANTs' file byte for byte) |
| ANTS-30 | ResampleImage | the seventh argument is both the pixel type and the interpolator's parameter | parameters after the interpolator, the pixel type after them ([design](ants-resample-image.md#the-seventh-argument)) |
| ANTS-31 | ResampleImageBySpacing | `nn` read from the `addvox` argument; other dimensions do nothing | each from its own argument; other dimensions an error |
| ANTS-32 | ResampleImageBySpacing | the smoothing sigma `out/in − 1` taken in millimetres; points beyond the input get the value at index 1 | the sigma in input voxels; those points 0 |
| ANTS-33 | antsApplyTransforms | unknown `-u` kept as default; `MultiLabel`'s alpha and `GenericLabel`'s interpolator ignored | an error; honoured |
| ANTS-35 | N4BiasFieldCorrection | `-c`, `-b`, `-t` without brackets set nothing | the value is the first parameter |
| ANTS-36 | N4BiasFieldCorrection | an unreadable mask becomes ones | an error |
| ANTS-38 | N4 (ITK) | a first-voxel minimum is left out of the histogram | the true range |
| ANTS-39 | N4 (ITK) | 64, 128 or 256 bins padded to twice the FFT size needed | the exact size |
| ANTS-40 | N4 (ITK) | zero and negative voxels kept in the log image, inside the histogram and the fit | left out (as a mask of the positive voxels would) |
| ANTS-43 | DenoiseImage (ITK) | the Rician correction is NaN, and the bias 0, at a high SNR | a finite factor |
| ANTS-44 | DenoiseImage (ITK) | outside the mask the Rician bias stays the raw noise estimate of a centre | the smoothed and corrected bias wherever a patch of the mask reaches |
| ANTS-47 | DenoiseImage | an unreadable mask is no mask | an error |
| ANTS-49 | antsRegistration | a linear stage that throws is dropped, exit 0 | an error |
| ANTS-50 | antsRegistration | `--save-state` after linear stages: exit 1, nothing written | the state is written |
| ANTS-51 | antsRegistration | `-z 0`: files misnamed, then a read past the end of the names (crash) | every computed transform under its stage's name (`reproduce`: ANTs' files up to the crash, then an error) |
| ANTS-52 | antsAI | the translation grid does not move `Rigid`/`Similarity` start points in 2D, nor `Similarity` ones in 3D | translated as the 3D rigid ones |
| ANTS-53 | antsAI | `AlignGeometricCenters`/`AlignCentersOfMass` with iterations write an empty `.mat` | the alignment, as with 0 iterations |
| ANTS-54 | antsAffineInitializer | the eighth argument (the local search's iterations) is never read: 20 iterations whatever it says | the argument's iterations (0: no local search; text or a negative value: an error) |
| ANTS-55 | antsAI | a translation grid with fewer extents than dimensions read past its end | an error (`reproduce`: the missing extents read as 0) |
| ANTS-56 | antsAffineInitializer | a dimension other than 2, 3 or 4 runs as 2D | an error |
| ANTS-61 | antsRegistration | IGDM with ICP, PSE or JHCT in a stage, or after an image metric: only one kind of point set reaches ITK (an exception, the stage dropped; or a crash) | every metric gets its inputs (`reproduce`: ITK's exception, or a stop where ANTs crashes) |
| ANTS-62 | antsRegistration | a text point set ending with a newline gets one more record (uninitialised; a copy of the last one in the oracle's build) | no extra record (`reproduce`: the copy of the last record) |
| ANTS-63 | antsRegistration | a binary VTK point set's labels read only as far as one point's components | every label read (`reproduce`: ANTs' bytes, the unread labels 0) |
| ANTS-65 | antsRegistration | IGDM's estimated intensity sigma: the minima never updated, the moving maximum subtracted | the range of both point sets' intensities |
| ANTS-66 | antsRegistration | IGDM's value reads before its intensity arrays for neighbourhoods of more than one voxel | the centre intensity, as the derivative reads it (`reproduce`: the two intensities taken as equal) |
| ANTS-67 | antsRegistration | IGDM's `kNeighborhood` and `gradientSigma` are ignored (one neighbour, central differences) | the mean over the k nearest points, Gaussian gradients of that sigma |
| ANTS-68 | antsRegistration | IGDM's derivative lacks the Gaussians' `1/σ²`: not the value's gradient | the value's gradient |
| ANTS-70 | antsRegistration | IGDM's estimated distance sigma can be 0 (NaN parameters, the stage dropped) | at least one voxel |
| ANTS-71 | antsRegistration SyN | `--print-similarity-measure-interval` crashes without some transforms or with several metrics | the missing transforms are the identity, the first metric's images used |
| ANTS-73 | antsRegistration | an initial transform image that is not a field is read as one | refused |
| ANTS-74 | antsRegistration | `--save-state` after linear stages that follow SyN saves the SyN state alone | every transform |
| ANTS-75 | antsRegistration | the full-scale similarity and interval volumes fail with a field or a B-spline transform | the transforms used as they are |
| ANTS-76 | antsRegistration | the collapse loses transforms around B-spline transforms | every transform kept |
| ANTS-78 | antsRegistration | `BSplineDisplacementField`'s and `BSplineExponential`'s base-level meshes doubled at the base level | the mesh as given (twice it gives ANTs' result) |
| ANTS-79 | antsRegistration | the time-varying stages read their time indices and mesh from the last `-t` | each stage's own |
| ANTS-80 | antsRegistration | the exponential stages take their sigmas as millimetres | the units given |
| ANTS-81 | antsRegistration | `Exp` (the short name) writes no `InverseWarp` with `-z 0` | written |
| ANTS-90 | Atropos | `KMeans` with several images: the second k-means seeded from the mask's statistics, its labels never read | the second k-means from each class's means, its labels kept |
| ANTS-91 | Atropos | `Otsu`'s range from `FLT_MIN` with `else if` | the image's range in the mask |
| ANTS-92 | Atropos | `Random` labels voxels outside the mask, which enter the first estimates | only the mask's voxels labelled |
| ANTS-93 | Atropos | several `-a` options: the adaptive smoothing weights in reverse order | each image its own weight |
| ANTS-94 | Atropos | the outlier filters keep their previous output | each call filters its own input |
| ANTS-95 | Atropos | `ManifoldParzenWindows` ignores the posterior weights | weighted |
| ANTS-96 | Atropos | partial-volume classes never re-estimated | re-estimated from their two classes |
| ANTS-97 | Atropos | `-u 1`: the priors' voxel numbers stored in `float` | exact |
| ANTS-98 | Atropos | `KMeans`' range from the mask's label 1 only | the mask's non-zero voxels |
| ANTS-99 | Atropos, N4BiasFieldCorrection, DenoiseImage | without `-d`, every image read as 3D | the image's dimension (N4BiasFieldCorrection and DenoiseImage since 2026-10-25) |
| ANTS-100 | Atropos | vector priors: every class the last component | each class its component |
| ANTS-101 | Atropos | an unknown `-p` runs Socrates silently | an error |
| ANTS-102 | Atropos | `--icm [1,2]` (the documented form) aborts | read as documented (`reproduce`: ANTs' message, exit 1) |
| ANTS-103 | Atropos | the joint shape and orientation model at the raw components and an unset coordinate | at the tensor's eigenvalues and direction (`reproduce`: the unset coordinate 0) |
| ANTS-104 | Atropos | `ManifoldParzenWindows`' local covariances double their running sums | the local covariance |
| ANTS-105 | Atropos | `-u 1` with priors and a mask crashes | runs (`reproduce`: exit 1 where ANTs crashes) |
| ANTS-106 | Atropos | partial-volume classes with an MRF hardly win a voxel (Atropos stops) | a partial-volume label is a member of its own set in the MRF |
| ANTS-107 | Atropos | Grubbs-Rosner's grown sample weighed with uninitialized memory | not met (ANTS-94 corrected; `reproduce`: those weights 0) |
| ANTS-110 | antsAI `-b` | landmarks kept by the lengths of the pairs' displacements | by their preserved distances to three other pairs |
| ANTS-111 | antsAI `-b` | after the correlations run out, the first blobs paired again and again | the pairing stops |
| ANTS-112 | antsAI `-b` | the blob detector does not keep the N strongest maxima | the N strongest |
| ANTS-114 | antsAI `-b` | no landmark: a rigid transform of NaNs, exit 0 | an error |
| ANTS-116 | antsAI `-b` | blob patches compared in the orientation the eigenvectors' signs give, reflected or not | proper rotations, every sign combination, the best correlation |
| ANTS-160 | ResampleImage | fewer Gaussian sigmas than dimensions: read past the list (heap-dependent result) | an error in both modes |
| ANTS-161 | ResampleImage | a Gaussian sigma or alpha ≤ 0: NaN weights, an image of NaN or zeros | an error in both modes, named (`reproduce` not yet ANTs' NaN image) |
| ANTS-162 | N4BiasFieldCorrection, ResampleImage, ResampleImageBySpacing | lattice, padding or grid sized from the physical extent with no check: beyond any machine's memory for a header in metres or micrometres | the same computation; by default stopped before allocating when it exceeds the machine's memory (`LARMORX_MEMORY_LIMIT`), with the GiB needed and the likely cause |
| ANTS-163 | ImageMath `Normalize`, `TruncateImageIntensity` | a mask of another size read outside its buffer | an error in both modes |
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
| ITK-19 | ITK `MultiStartOptimizerv4` (antsAI, antsAffineInitializer) | a start point that throws keeps the previous one's value (0 for the first) and can be the best | it takes no part in the search for the best |
| ITK-21 | ITK `MultiStartOptimizerv4` without a scales estimator (antsAffineInitializer) | each start point's local search begins at the learning rate the previous one's line search left | each at the configured rate (0.1); named where the result changes |
| ITK-22 | ITK labelled point-set metrics (`ICP`, `PSE`, `JHCT`) | a fixed point whose label the moving set lacks throws: the stage is dropped | those points take no part (no label in common: an error) |
| ITK-23 | ITK `GaussianMembershipFunction` (`JHCT`) | an identity covariance keeps the univariate prefactor `1/√(2π)` | the `D`-dimensional prefactor |
| ITK-25 | ITK manifold Parzen windows (`JHCT`) | fewer points than the covariance neighbourhood: a read past the list of neighbours | the neighbours there are (in both modes) |
| ITK-26 | ITK manifold Parzen windows (`JHCT`) | anisotropic covariances count each diagonal product twice | once |
| ITK-28 | ITK `JHCT` | an alpha within 4 ulps of 1 takes the logarithm and divides by `alpha − 1` | both tests treat it as 1 |
| ITK-24 | ITK `JHCT` | the total number of points leaves out the fixed ones | fixed and moving points |
| ITK-29 | ITK manifold Parzen windows (`JHCT`) | the kernel sigma used as a variance | as a sigma (`σ²·I`) |
| ITK-30 | ITK `GC` with a displacement field | every point's derivative in the first voxel | each voxel its own |
| ITK-31 | ITK SyN | a stage with one of its two masks keeps a stale resampled mask | the current one |
| ITK-32 | ITK labelled point-set metrics | SyN evaluations use the first `Initialize`'s per-label metrics | the current ones |
| ITK-33 | ITK SyN | point-set metrics among several metrics get identity transforms | their composites |
| ITK-35 | ITK B-spline transforms | no Jacobian with respect to position: later linear, field and `BSpline` stages fail | the derivative of the B-spline sum |
| ITK-36 | ITK time-varying B-spline method | every metric evaluated with the last metric's transforms | each its own |
| ITK-37 | ITK time-varying B-spline method | stale inverse fields at the first and last time samples | zeroed |
| ITK-38 | ITK time-varying B-spline method | a single point-set metric's points fitted at physical positions in the parametric domain (throws on oblique and RAS grids) | mapped into the parametric domain |
| ITK-39 | ITK time-varying method | `-q` ignored | applied |
| ITK-40 | ITK time-varying methods | masks not moved with the resampled images | moved |
| ITK-41 | ITK velocity-field transforms | a transform that never integrated crashes or has no inverse | integrated |
| ITK-42 | ITK scales estimator | scales all below epsilon (a vanished probe) refused | ones (equal scales cancel) |
| ITK-50 | ITK `KdTreeBasedKmeansEstimator` (Atropos `KMeans`) | the point each divided k-d tree node is split at is left out of the new centres | every point counted |
| ITK-51 | ITK `TDistribution` (Atropos' Grubbs-Rosner) | NaN above about 340 degrees of freedom: no outlier is ever found in an image | the Gamma ratio through its logarithm where it would overflow |
| ITK-60 | ITK `LandmarkBasedTransformInitializer` (antsAI `-b`, 2D) | −90° whenever the dot product is near 0 | `atan2` |
| ITK-95 | ITK grayscale morphology (ImageMath `GO`, `GC`, `GD`, `GE`) | a first axis of one voxel: voxels at ±FLT_MAX at radius 0, values from outside the structuring element above | the true result (`reproduce` not yet ITK's values) |
| ITK-96 | ITK HDF5 transform writer (antsRegistration's inverse composite) | a transform without parameters: HDF5's error, a part-written file (ITK-15, met there) | not written, named |
| ITK-97 | ITK sampled metrics (displacement-field stages) | a sample point outside the shrunk virtual region: its derivative added to another voxel's parameters | dropped; `reproduce` adds it as ITK does |
| ITK-98 | ITK B-spline interpolation | an axis too short for one mirror: coefficients read outside their buffer | the mirror repeated; `reproduce` reads them as 0 |
| ITK-99 | ITK sampled metrics | a derivative written past its array | dropped in both modes |
| ITK-100 | ITK Mattes metric | a later level's bin size of 0 used silently (the metric 0) where the first level refuses | an error by default; `reproduce` as ITK |

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

The 3dAutomask and 3dUnifize replicas (`larmorx-gpl`, 2026-10-23) have the switch for every
AFNI entry that applies to them; they have no clean-room original yet. AFNI-40 and AFNI-46
(crashes) are refused in both modes.
- **ITK-06** (nifti_clib reads NaN and ±inf as 0) is kept in both modes, by measurement
  (2026-10-23, rule C): no fMRIPrep input has such a value, and keeping them would spread one NaN
  through every smoothing step ([investigation](../../validation/investigations/itk-06-nonfinite/README.md)).
Their clean-room originals (2026-10-23) have the same switch and the same defaults; their parity
records' fix passes show each correction where it applies.

## How the switch is validated

- **Parity** compares larmorx with the original in `reproduce` mode, where bit-identity held
  before D13 (`LARMORX_UPSTREAM_DEFECTS=reproduce` for the command lines,
  `upstream_defects="reproduce"` for the Python API; the stage-by-stage comparison with
  fMRIPrep too, since it compares with fMRIPrep as it ran). A second pass runs the default and
  reports, case by case, which defects were named and what the corrections changed. It also
  checks the rule above: a defect is named exactly when the two passes differ (`D13 fix pass`
  in the records: [afni-tshift.md](../validation/afni-tshift.md), [afni-automask.md](../validation/afni-automask.md) and [afni-automask-replica.md](../validation/afni-automask-replica.md), [afni-unifize.md](../validation/afni-unifize.md) and [afni-unifize-replica.md](../validation/afni-unifize-replica.md),
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
