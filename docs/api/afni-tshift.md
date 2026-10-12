# 3dTshift

`lx.afni.tshift` (command line `larmorx afni 3dTshift`; Rust crate `larmorx-afni`):
slice-timing correction, compatible with AFNI 25.2.09's `3dTshift`.

**Clean-room re-implementation.** AFNI is GPL-2, so larmorx did not port it: the code was
written from the behaviour spec [`specs/3dTshift.md`](../../specs/3dTshift.md), AFNI's help
text and black-box runs of the AFNI binary, without AFNI's source code
([provenance](../../crates/larmorx-afni/PROVENANCE.md)).

**Status: `validated`** against AFNI 25.2.09 itself (see the
[validation record](../validation/afni-tshift.md)). The record covers 311 cases:
- every file of `larmorx-testdata` `synthetic/tshift/` (every FFT length class, every data
  type AFNI reads, edge content);
- every method, option and `-tpattern` form, slice timing from NIfTI headers, the "copy of
  input" cases, AFNI's defects (D13) and 18 error cases;
- 20 runs on real BOLD data (ds000210 multi-echo, ds001600, ds003345, ds005454 multiband
  with 96 slices, ds006010, ds006736), called as fMRIPrep calls 3dTshift;
- AFNI's global options and environment variables (60 cases), where exit statuses and
  messages are compared byte for byte too.

All 311 agree: 279 compared, 29 rejected by both, and 3 documented divergences (AFNI's own
output format and `-voxshift`, which larmorx does not support, and a dataset of one row,
which AFNI writes as a text file with a random identifier, AFNI-20). 176 of the 279 compared
are byte-identical in their data; in every compared case the header fields are AFNI's and the
geometry agrees to 1e-6 mm (the whole header byte for byte where the record compares it), and
in the 60 cases of the global options and variables, where exit statuses and both output
streams are compared byte for byte too, all 60 are identical (2026-10-12, completing
3dTshift, part 2b). The Lagrange and weighted-sinc methods and the
copies give **the same bytes** as AFNI (129 of 132). The Fourier method (the default) differs
in the last float32 bits, at most 4.3e-7 of the largest value, because AFNI's FFT is float32
and larmorx's is double; integer outputs then differ by at most 1, in a few values per million
on the real runs (up to 0.2 % on a synthetic series built to clip). The known last-bit
differences are listed at the end of this page.

**Bit-exact replica.** `larmorx-gpl afni 3dTshift` (package `larmorx-gpl`, `crates-gpl/`,
GPL-3.0-or-later) is a translation of AFNI's own source, with AFNI's float32 FFT and a port of
glibc's `sinf`/`cosf`. It takes the same arguments as `larmorx afni 3dTshift`, follows the
same conventions (NIfTI output only, an existing output is an error), corrects the same AFNI
defects by default, and with `LARMORX_UPSTREAM_DEFECTS=reproduce` gives AFNI's output bytes
on every compared parity case ([replica record](../validation/afni-tshift-replica.md); see
[Upstream defects](#upstream-defects-d13)).
It is a separate program because of its licence ([licensing](../licensing.md)). When it is
installed, `lx.afni.tshift` and `larmorx afni 3dTshift` run it by default (see
[Choosing the implementation](#choosing-the-implementation)).

## Quick start

```python
import json
import larmorx as lx

# As fMRIPrep calls 3dTshift: SliceTiming from the BIDS sidecar, tzero mid-acquisition.
meta = json.load(open("sub-01_task-rest_bold.json"))
times, t0 = lx.afni.fmriprep_slice_timing(meta["SliceTiming"], meta.get("SliceEncodingDirection"))
stc = lx.afni.tshift(
    "sub-01_task-rest_bold.nii.gz",
    slice_times=times,
    tzero=t0,
    tr=meta["RepetitionTime"],
    n_threads=8,
)
stc.save("sub-01_task-rest_desc-stc_bold.nii.gz")

# A named pattern and another method
img = lx.afni.tshift("bold.nii.gz", slice_times="alt+z", method="heptic")
```

The same from a shell, with AFNI's own arguments:

```bash
larmorx afni 3dTshift -ignore 0 -tzero 0.969 -TR 2.0s -tpattern @slice_timing.1D \
    -prefix sub-01_desc-stc_bold.nii.gz sub-01_task-rest_bold.nii.gz
```

## `lx.afni.tshift(image, *, slice_times, tr=None, tzero=None, slice=None, ignore=0, method="fourier", restore="trend", detrend=True, n_threads=1, implementation="auto", upstream_defects="fix", afni_settings=None)`

Each voxel's time series is detrended (least-squares line), shifted in time so that its slice
refers to `tzero`, clipped to its own range, and given its trend back. The output at time
point `n` estimates the signal at `n·TR + tzero`.

Returns an `lx.Image` with the header `3dTshift` writes: `pixdim[4]` = TR (seconds),
`toffset` = `tzero`, no slice timing, millimetres and seconds. Data keep AFNI's storage type
(`uint8`, `int16` or `float32`); when the output has a brick factor (`scl_slope`), the data
are the scaled values, as `lx.load` returns them. `img.implementation` says which
implementation computed it (below).

**Files and in-memory images.** Paths are read with AFNI's rules (below), which is what makes
the result match AFNI. In-memory images (`lx.Image`, nibabel images, `(array, affine)`) keep
`uint8`, `int16` and `float32` data and convert other types to `float32`; `tr` is required
when they have no NIfTI header.

`lx.afni.fmriprep_slice_timing(slice_timing, slice_encoding_direction)` gives the slice times
and `tzero` fMRIPrep passes: `SliceTiming` reversed when the direction ends in `-`, and
`round(min + 0.5·(max − min), 3)`.

### Option mapping

| 3dTshift | `lx.afni.tshift` | Notes |
|---|---|---|
| `dataset` | `image` | a 4D NIfTI path, `lx.Image`, nibabel image or `(array, affine)` |
| `-prefix out` | the returned `Image` (`.save(path)`) | the CLI needs `.nii` or `.nii.gz` and refuses an existing file |
| `-tpattern alt+z` (and `altplus`, `alt+z2`, `alt-z`, `altminus`, `alt-z2`, `seq+z`, `seqplus`, `seq-z`, `seqminus`, `zero`, `simult`) | `slice_times="alt+z"` | case-sensitive, except `zero` and `simult` |
| `-tpattern @file`, `-tpattern '@1D: ...'` | `slice_times=[...]` (seconds) | values must lie in `[0, TR]`; the file is AFNI 1D text (see below) |
| (no `-tpattern`) | – | the CLI uses the header's slice timing, or copies the input when there is none |
| `-TR 2s`, `-TR 2000ms` | `tr=2.0` | default: the header's TR (ms and µs converted), 1 s if not positive |
| `-tzero z` | `tzero=z` | default: the mean slice time; must be ≥ 0 |
| `-slice k` | `slice=k` | wins over `tzero` |
| `-ignore n` | `ignore=n` | at most nt − 5 |
| `-Fourier` `-linear` `-cubic` `-quintic` `-heptic` `-wsinc5` `-wsinc9` | `method="fourier"` (default), `"linear"`, … | the CLI compares 4 characters (`-cub`), 7 for `-wsinc5/9` (any case) |
| `-rlt` / `-rlt+` | `restore="none"` / `"intercept"` | `-rlt+` adds back the fitted intercept, not the mean |
| `-no_detrend` | `detrend=False` | every voxel's mean removed and restored; AFNI's voxel pairs with `upstream_defects="reproduce"` (AFNI-05, below) |
| `-verbose` | – | progress on stderr |
| `-voxshift dset` | – | not supported |
| (threads: `OMP_NUM_THREADS`) | `n_threads=1` (0 = all CPUs) | AFNI is single-threaded; the result does not depend on the thread count |
| (`LARMORX_IMPLEMENTATION=auto\|replica\|original`) | `implementation="auto"` | larmorx's choice between the replica and the original (below) |
| (`LARMORX_UPSTREAM_DEFECTS=fix\|reproduce`) | `upstream_defects="fix"` | AFNI's defects corrected (default) or reproduced ([upstream defects](#upstream-defects-d13), [upstream-defects.md](upstream-defects.md)) |
| AFNI's environment variables, `-Dname=value` | `afni_settings={"AFNI_NIFTI_PRIORITY": "Q"}` | the command line reads the process environment, as AFNI does; Python never does: [AFNI's settings in Python](#afnis-settings-in-python) |
| `-overwrite`, `-ok_1D_text`, `-skip_afnirc` | – | the Python API writes no output file and its own slice-time file |

## Choosing the implementation

```python
img = lx.afni.tshift("bold.nii.gz", slice_times="alt+z")   # implementation="auto"
img.implementation
# Implementation(kind='replica', package='larmorx-gpl', version='0.0.1',
#                licence='GPL-3.0-or-later', program='/.../venv/bin/larmorx-gpl')
lx.afni.tshift("bold.nii.gz", slice_times="alt+z", implementation="original")
```

- **`"auto"`** (the default) runs the bit-exact replica when the `larmorx-gpl` program is
  installed, and the clean-room original otherwise. **`"replica"`** and **`"original"`** force
  one; `"replica"` without the program raises `lx.ReplicaNotFoundError`, which names the
  install command.
- **Installing the replica:** `pip install "larmorx[exact]"`. Neither package is on PyPI yet;
  build them from the repository (`docs/licensing.md`, "Packages").
- **How it runs** (`docs/licensing.md`, "Choosing at run time"): as a separate program, on
  files in a temporary directory. A path is passed as it is; an in-memory image is written
  first, with its data as they are (no scaling). The keyword arguments become `3dTshift`
  options with the same meaning; `slice_times` as numbers become a `-tpattern` file, and
  `detrend=False` with `method="fourier"` becomes `-linear -no_detrend -Fourier`, the order
  AFNI accepts. `n_threads` reaches it as `OMP_NUM_THREADS`, AFNI's variables only from
  `afni_settings` ([AFNI's settings in Python](#afnis-settings-in-python)); its warnings come
  back as Python warnings. The extra cost per call is one program start, the output written and read back,
  and for in-memory images the input written as well. Measured on this project's development
  machine (2026-10-11, median of 5) on two real runs called as fMRIPrep calls 3dTshift
  (ds001600, 64×64×44×5, and ds003345, 64×64×34×216, both int16 `.nii.gz`), on 1 thread and
  all 12: `lx.afni.tshift(path, implementation="replica")` took 0-15 ms longer than running
  the program directly and reading its output with `lx.load`; from the image in memory it took
  2-133 ms less, because the input is then written uncompressed instead of read from gzip.
- **The two differ only in the last bits** of some values (Fourier, quintic, heptic,
  weighted sinc and `-no_detrend` cases; see the [validation record](../validation/afni-tshift.md)).
  So larmorx never falls back from one to the other: a replica that is found but fails
  raises `lx.ReplicaError`, with its exit code and messages.
- **Errors.** Both implementations check the arguments that do not need the image the same
  way (`ValueError`). The checks that need the image (the number of slice times, their range,
  `slice`, `ignore`) come from the implementation that runs: a `ValueError` from the original,
  an `lx.ReplicaError` with AFNI's message from the replica.
- **Where the program is looked for:** `LARMORX_GPL_BIN` (the path of the program), then the
  Python environment's scripts directory, then `PATH`, once per call.
- **Command line:** `larmorx afni 3dTshift` follows the same rule; `LARMORX_IMPLEMENTATION`
  overrides it. The replica's output, output files and exit code are passed on unchanged;
  127 means `replica` was asked for and the program is missing. With `-verbose`, the first
  line names the implementation: `++ implementation: original (larmorx 0.0.1, Apache-2.0),
  clean-room 3dTshift` or `++ implementation: replica (larmorx-gpl 0.0.1, GPL-3.0-or-later),
  bit-exact translation of AFNI 25.2.09's 3dTshift`.

## How AFNI reads and writes NIfTI (reproduced)

| Stored type | Header scaling | Kept as | Values |
|---|---|---|---|
| uint8, int16, float32 | `scl_slope` ≠ 0 and `scl_inter` ≠ 0 | float32 | `slope·x + inter`, in double from float32 values |
| uint8, int16, float32 | otherwise | as stored | a slope other than 0 or 1 becomes the brick factor |
| int8, uint16, int32, uint32, float64 | any | float32 | converted, then scaled if the slope is not 0 |

Non-finite values read as 0. Outputs keep the type and brick factor (`scl_slope`); integers are
rounded half to even and clamped (shorts to ±32767). Three of these rules are AFNI defects that
larmorx corrects by default: a **negative** `scl_slope` (AFNI-01), float32 data with a slope and
an intercept (AFNI-02) and metre or micron units (AFNI-07); see below.
The output header is NIfTI-1 with qform and sform both from AFNI's matrix (the sform if
`sform_code > 0`, else the qform), codes mapped 1, 2 → 1, 3, 5 → 3, 4 → 4, the slice axis
third (`dim_info` 48) and slice range `0..nz−1`; AFNI's history extension is not written.
AFNI writes a grid within about 0.01° to 0.024° of plumb as exactly plumb, discarding its
tilt (AFNI-16; AFNI's test is in single precision, so the angle depends on the voxel size:
0.024° for a 2 mm voxel, 0.013° for 3 mm). larmorx keeps the input's matrix by default (user
decision, 2026-10-12): the sform and the qform describe the input's grid, as AFNI writes them
for an oblique one. With `upstream_defects="reproduce"` both implementations write AFNI's
plumb matrix, the header byte for byte (the clean-room original since 2026-10-12: AFNI's
plumb test in single precision, the cardinal matrix with its signed zeros, the quaternion by
the NIfTI reference library's method in double, with the steps AFNI's x86-64 build carries out
in x87 extended precision emulated with integers, so NIfTI-2 headers are AFNI's on every
platform). The exact rules, worked examples included:
[`specs/3dTshift.md`](../../specs/3dTshift.md) §6.1.

## Upstream defects (D13)

larmorx corrects 3dTshift's defects by default, in both implementations, and reproduces them
with `upstream_defects="reproduce"` (command line: `LARMORX_UPSTREAM_DEFECTS=reproduce`); a
warning names each one that changed the output, in both modes
([upstream-defects.md](upstream-defects.md), register entries in
[findings/upstream-defects.md](../findings/upstream-defects.md)):

| ID | AFNI 25.2.09 | larmorx by default | Where |
|---|---|---|---|
| AFNI-01 | a negative `scl_slope` is applied when values are stored but not when they are read: shifted slices come out sign-flipped and scaled (int16 with slope −0.5: a mean of 942 comes out −1884) | the factor is applied when reading too, so the output has the input's values | files with a negative `scl_slope`; slices left unshifted and copies are the same in both modes |
| AFNI-02 | float32 data with a slope and an intercept are scaled when read, and the slope is also kept as the output's `scl_slope`: the output reads back scaled twice | scaled once, written without a factor | float32 files with both fields set |
| AFNI-03 | header slice code 2 (`NIFTI_SLICE_SEQ_DEC`) gives every slice time 0: nothing is shifted | slices `slice_end` down to `slice_start`, as NIfTI defines the code | the command line without `-tpattern` |
| AFNI-05 | `-no_detrend` removes and restores the mean of the first voxel of each pair (index order within a slice) only; the second is shifted raw, its edges falling toward 0 | every voxel's mean is removed and restored | `detrend=False`, `-no_detrend` |
| AFNI-07 | metre and micron units: the voxel sizes are converted to millimetres, the origin is not (a plumb grid), or the whole sform is not (an oblique grid) | the whole matrix is converted | files whose `xyzt_units` give metres or micrometres |
| AFNI-08 | a copy of the input whose kept slice times are all within 3 ms of 0 gets `slice_end` 0 | no slice code, `slice_end` = nz − 1 | the command line: header slice times beyond the TR, e.g. 0.5 ms apart with a 1 ms TR |
| AFNI-11 | an existing output is not written, and the exit status is 0 | an error (exit 1) | the command line |
| AFNI-12 | a `-prefix` with blanks, shell metacharacters or non-ASCII characters anywhere in its path is refused (exit 1); one that passes only through AFNI's exception for names beginning with `3dcalc`, or whose directory begins with `-`, is refused when written (nothing written, exit 0) | written | the command line |
| AFNI-16 | a grid within about 0.01° to 0.024° of plumb is written exactly plumb, its tilt discarded | the input's matrix is kept | slightly oblique inputs (headers with tiny rotations) |
| AFNI-18 | an input name with a blank is read as a list of datasets to join in time, unless `AFNI_PATH_SPACES_OK` is yes: an input in a directory whose name has a blank cannot be opened | the name is one file | the command line (the Python API always passes one file) |
| AFNI-19 | with `AFNI_ALLOW_MILLISECONDS` yes and `-TR` in milliseconds, `toffset` is written in milliseconds, labelled seconds | `toffset` in seconds | the command line, with that variable |
| AFNI-20 | a dataset of one row (one voxel along y and z) is written as a 1D text file under the NIfTI name, unless `AFNI_WRITE_1D_AS_PREFIX` is yes | NIfTI, in both modes (AFNI's file holds a random identifier, so `reproduce` does not reproduce it; the warning says so) | the command line |
| AFNI-22 | `-Dname=value` with blanks in the value or around `=` sets another value (`-DX=a b` sets `X` to `a=b`) | the value as written | the command line |
| AFNI-23 | with `AFNI_NIFTI_SCALE=NO`, uint8 and int16 data with both a slope and an intercept stay integers with the slope as their scale factor, and the intercept is dropped: the output's values are off by it (12.5 lower for an intercept of 12.5) | the integers and `scl_slope` kept, the intercept written as `scl_inter` (`scl_slope` 1 where AFNI writes 0), so the output reads back as without the variable, to within half a stored unit; with `-rlt` and `-ignore`, the data read as without the variable (below) | with that variable (the command line, or `afni_settings`) |
| AFNI-26 | an output that cannot be written (its directory is missing or cannot be written) is not written, and the exit status is 0 | an error (exit 1); `reproduce` prints AFNI's message, writes nothing and exits 0 (the replica since 2026-10-12; the clean-room original to follow) | the command line |

On fMRIPrep's calls (int16 or float32 BOLD in millimetres, `-tpattern` from the sidecar, no
`-no_detrend`, fresh output names) none of them applies but AFNI-16, so the default output is
AFNI's except on a BOLD header whose rotation lies within about 0.01° to 0.024° of plumb,
where it keeps the input's grid and AFNI writes a plumb one. The
parity records' "D13 fix pass" lists what the corrections change, case by case.

**The two implementations.** Both correct the same defects by default and, with
`reproduce`, give AFNI's output: the replica (`larmorx-gpl`, which receives the choice as
`LARMORX_UPSTREAM_DEFECTS`) byte for byte, the clean-room original within its thresholds for
the values (the last bits of its Fourier, quintic, heptic, weighted-sinc and `-no_detrend`
numerics) and as AFNI otherwise: identical header fields and file names, the geometry to
1e-6 mm (byte for byte where the header is compared whole), and exit statuses and messages
byte for byte in the cases of AFNI's global options and variables. So `implementation="auto"` gives the corrected output
whichever runs. The replica's record compares its default with the original's on every case
where a correction applies: they agree in every header field, the geometry, the scaling and
the slice fields of copies (AFNI-08's included), and in the values up to those last bits. The
original follows AFNI's geometry, the slice fields of copies, the output-name rules and
AFNI-03's notice for a copy since 2026-10-12 (completing 3dTshift, part 2b; before, these were
expected divergences in its record). Warning and error labels: AFNI writes them in terminal
colour codes, also into files and pipes (AFNI-21, messages only); larmorx does so in
`reproduce` mode only.

## AFNI's global options and environment variables

AFNI programs take *global options* out of the command line before reading their own, wherever
they stand, and read many environment variables. Both implementations do the same since
2026-10-12 (`larmorx afni 3dTshift`, whichever runs, and `larmorx-gpl afni 3dTshift`), on
Linux, macOS and Windows alike (decided 2026-10-12; behaviour in `specs/3dTshift.md` §7.1).

```
larmorx afni 3dTshift -tpattern alt+z -prefix out.nii in.nii -overwrite
larmorx afni 3dTshift -DAFNI_NIFTI_WRITE_TYPE=2 -tpattern alt+z -prefix out.nii in.nii
AFNI_DECONFLICT=YES larmorx afni 3dTshift -tpattern alt+z -prefix out.nii in.nii   # out_AA1.nii
```

- **`-overwrite`** replaces an existing output (as `AFNI_DECONFLICT=OVERWRITE`, quietly).
  **`-Dname=value`** sets a variable for the run; it wins over the environment, and a later
  setting over an earlier one. **`-ok_1D_text`** reads text in a `-tpattern @file` as 0;
  **`-skip_afnirc`** is accepted (larmorx never reads `~/.afnirc`). A `-D` of a variable
  3dTshift does not read is applied with a warning that it has no effect; one that sets
  nothing (`-DX=`) is dropped. By default the value is the text after the first `=`, as
  written; with `LARMORX_UPSTREAM_DEFECTS=reproduce` it is read as AFNI does (AFNI-22).
- **The process environment is never changed.** It is read once into a table of settings,
  the global options are applied to it, and every part of the program that reads a variable
  reads the table. (On Windows, environment variable names match whatever their case.)
- **The variables that change something:** `AFNI_DECONFLICT` (an existing output: replaced,
  renamed to `out_AA1.nii` ..., or an error), `AFNI_ALLOW_ARBITRARY_FILENAMES`,
  `AFNI_PATH_SPACES_OK`, `AFNI_NOREALPATH`, `HOME`, `AFNI_NIFTI_PRIORITY`,
  `AFNI_NIFTI_SCALE`, `AFNI_NIFTI_TYPE_WARN`, `AFNI_NIFTI_VIEW`, `AFNI_DEFAULT_STD_SPACE`,
  `AFNI_NO_OBLIQUE_WARNING`, `AFNI_1D_ZERO_TEXT`, `AFNI_USE_FGETS`, `AFNI_NIFTI_WRITE_TYPE`
  (2 writes NIfTI-2), `AFNI_ALLOW_MILLISECONDS`, `AFNI_WRITE_1D_AS_PREFIX`,
  `AFNI_MESSAGE_PREFIX`, `AFNI_MESSAGE_COLORIZE`, `AFNI_ECHO_COMMANDLINE` and
  `AFNI_DONT_LOGFILE`. Each does what it does in AFNI, with AFNI's messages; the defaults
  differ only where a defect is named (AFNI-11, -12, -18, -19, -20, -21, -22, -23). The other
  variables AFNI 25.2.09's 3dTshift reads change nothing larmorx writes or prints (AFNI's
  header extension, its debugging output, its own dataset format, its version banner): they
  are accepted and ignored. The full list, with each variable's values and effect, is in the
  spec, §7.1.
- **Writing the output** goes through a temporary file in the output's directory, renamed
  over the output when complete, so an existing output is replaced whole or not at all. On
  Windows, while another program holds the output open (an antivirus scan, a viewer), the
  rename is retried for about 2 s; then the run fails with a message that names the file and
  says that nothing was written. Paths with blanks and non-ASCII characters work on every
  system.
- **`AFNI_NIFTI_SCALE=NO`** (AFNI-23, decided 2026-10-12). AFNI keeps uint8 and int16 data
  with both a slope and an intercept as integers, with the slope as their scale factor, and
  drops the intercept, so its output's values are off by the intercept. The variable is
  undocumented; it meant "do not scale" until AFNI began keeping scaled integers as integers
  (2021), and since then its only effect is to keep such data as integers. larmorx keeps that,
  and the values right: the output has AFNI's integers and `scl_slope` (1 where AFNI writes
  0) and the input's intercept as `scl_inter`, so it reads back as the run without the
  variable, to within half a stored unit. With `-rlt`, which removes the trend and the
  intercept with it, AFNI's output is already right; with `-rlt -ignore`, where the ignored
  volumes keep the intercept and the others do not, the data are read as without the variable
  (float32). `LARMORX_UPSTREAM_DEFECTS=reproduce` gives AFNI's output.
- **The Python API does not read the environment**: [below](#afnis-settings-in-python).

## AFNI's settings in Python

`lx.afni.tshift` **never reads the process environment** (decided 2026-10-12; CLAUDE.md rule
6: no global configuration). The same call gives the same result whatever the shell has set.
AFNI's variables are a keyword argument:

```python
img = lx.afni.tshift(
    "bold.nii.gz",
    slice_times="alt+z",
    afni_settings={"AFNI_NIFTI_PRIORITY": "Q", "AFNI_NIFTI_WRITE_TYPE": "2"},
)
```

- **Both implementations** read exactly these variables (the clean-room original since
  2026-10-12). The replica runs with them, every AFNI variable of the process environment
  removed (whatever its case), plus the ones `lx.afni.tshift` sets itself for its
  own use of the replica: a path is always one file (`AFNI_PATH_SPACES_OK=YES`; temporary
  directories can have blanks), messages are plain text, read as warnings
  (`AFNI_MESSAGE_COLORIZE=NO`, no `AFNI_MESSAGE_PREFIX`, no `AFNI_ECHO_COMMANDLINE`), and an
  oblique input is not reported (`AFNI_NO_OBLIQUE_WARNING=YES`: AFNI's advice to run
  `3dWarp -deoblique` is not a warning about the result). Those five cannot be given. The
  values reach the replica as written, as from the environment (never read as `-D` is,
  AFNI-22).
- **The variables that change what a call returns:** `AFNI_NIFTI_PRIORITY`,
  `AFNI_NIFTI_SCALE` (AFNI-23), `AFNI_NIFTI_VIEW`, `AFNI_DEFAULT_STD_SPACE`,
  `AFNI_NIFTI_WRITE_TYPE` (the header of the result: NIfTI-2 fields) and `AFNI_NOREALPATH`
  (the input's name in warnings).
- **The other variables 3dTshift reads** are accepted and change nothing a call returns, with
  either implementation: those about the output's name (`AFNI_DECONFLICT`,
  `AFNI_ALLOW_ARBITRARY_FILENAMES`: the call writes a fresh temporary file), the slice-time
  file (`AFNI_1D_ZERO_TEXT`, `AFNI_USE_FGETS`: the call writes it), `-TR` in milliseconds
  (`AFNI_ALLOW_MILLISECONDS`: the call passes seconds), `AFNI_WRITE_1D_AS_PREFIX`, `HOME`,
  `AFNI_DONT_LOGFILE`, `AFNI_NIFTI_TYPE_WARN` (a message the call does not turn into a
  warning), and those that change nothing larmorx writes (spec §7.1's list).
- **Checked:** names are case-sensitive; a name 3dTshift does not read (`AFNI_NIFTI_SCAL`,
  `PATH`), one of the five above, or an empty value is a `ValueError`; anything but a mapping
  of strings to strings a `TypeError`.
- **The command line** (`larmorx afni 3dTshift`, `larmorx-gpl afni 3dTshift`) reads the
  process environment, and `-Dname=value`, as AFNI does (above).
- **Checked against the command line:** both parity suites run every case of AFNI's variables
  and global options that the Python API can express with the case's variables as
  `afni_settings`, while the process environment holds AFNI variables that would change the
  output (NIfTI-2, `AFNI_NIFTI_SCALE`, the tlrc view, the qform); each must give the command
  line's output ([record](../validation/afni-tshift.md),
  [replica record](../validation/afni-tshift-replica.md)). Tests on all six platforms check
  that the replica program receives `afni_settings` and none of the environment's AFNI
  variables, and that the original applies them and ignores the environment
  (`tests/python/test_replica.py`).

## Behaviour found by black-box runs

The spec left these open; each was settled by running AFNI 25.2.09 (details in
`specs/3dTshift.md`, marked *(observed)*):

- **Header slice timing** (no `-tpattern`): used when the slice axis is the third, `slice_code`
  (a signed byte) is positive, `slice_duration` is positive and `0 ≤ slice_start <
  slice_end < nz`. Codes 1, 3, 4, 5, 6 follow NIfTI's orders; code 2 (`SEQ_DEC`) and codes
  7–127 give every slice 0. Times are built by adding the duration in float32.
  Out-of-range times make the output a copy of the input, with the timing kept in the header.
- **Heptic interpolation** sums the terms −2…+3 first, then −3, then +4.
- **Weighted sinc** takes the fraction once per slice from its offset (not per output sample),
  sums in double near the edges, and in float32 only where the whole window ±R lies inside
  the series.
- **Scaling** to float32 multiplies and adds in double after converting the stored value to
  float32.
- **`-no_detrend`**: within each slice, voxels are taken in pairs in index order (x fastest);
  the first of a pair has its mean removed and restored, the second is shifted as raw values
  (zeros beyond the ends, clipped to its range); reproduced with
  `upstream_defects="reproduce"` (AFNI-05). The last bits of these cases are not bit-identical
  yet.
- **`-TR … ms`** switches every time to milliseconds (header offsets, `-tzero`, `@file`
  values are taken as milliseconds); the output is written in seconds.
- **`@file`**: AFNI 1D text read column by column (one line reads left to right); `#`
  comments, commas and `N@value` repeats work; the first row sets the number of columns.
- **Copies of the input** (no slice timing, one volume): the input as AFNI read it, with its
  TR and `toffset`, even when `-TR` is given.

Found while completing 3dTshift, part 2b (2026-10-12; spec §3.1, §6.1, §7 and §7.1):

- **The messages of a run**, in AFNI's order: which lines go to standard output (`++ updating
  time offset to ...` always, `-verbose`'s progress), the checks of `-slice` and `-ignore`
  before the TR, `-verbose`'s `++ Wrote output:` labelled after a copy and plain after a
  shift, naming the renamed file even when nothing is written (AFNI-11), numbers as C's `%g`.
- **1D files** read leniently: a token is an error only when it starts with a character other
  than a digit, `+`, `-`, `.`, `e`, `@` or `*`; otherwise it is the number it starts with, or 0
  (`2x` is 2, `*` is 0); colons separate values; the error message shows the row with its
  comment, cut at 64 characters. Inline `1D:` strings are strict.
- **AFNI-18's list** is split at blanks and at `~` (only a blank makes a name a list).
- **Template spaces**: the oracle answers with AFNI's table (`TT_N27` is TLRC, `MNI_152` MNI);
  names are case-sensitive, `AFNI_NIFTI_VIEW` is a whole word in any case.
- **AFNI crashes on `-ignore` without a value** (AFNI-25; larmorx: an error, exit 1, as AFNI's
  crash handler), and **exits 0 after a failed write** (AFNI-26: an output in a directory that
  does not exist or cannot be written; larmorx: exit 1 by default, AFNI's result with the
  switch).

## Deliberate differences

- Output is NIfTI only (`-prefix` must end in `.nii` or `.nii.gz`); an existing output is an
  error (AFNI warns and exits 0 without writing; AFNI-11, which both implementations reproduce
  with `LARMORX_UPSTREAM_DEFECTS=reproduce`).
- Any `-prefix` path is accepted; AFNI refuses names with blanks, shell metacharacters or
  non-ASCII characters ("illegal value ... after -prefix"; AFNI-12). Both implementations
  name the defect when they accept such a name, and refuse it with
  `LARMORX_UPSTREAM_DEFECTS=reproduce`.
- `-voxshift`, AFNI's own formats, sub-brick selectors and 1D selectors (`file[2]`,
  `file'`) are not supported; hexadecimal numbers in 1D files are rejected.
- AFNI's global options other than `-overwrite`, `-Dname=value`, `-ok_1D_text` and
  `-skip_afnirc` (`-Vname=`, `-echo_edu`, `-np`, `-all_opts`, `-h_view`, `<<` inclusions,
  ...) are unknown options (exit 1). Both implementations take those four and AFNI's
  variables (above).
- The Fourier method uses larmorx's own FFT in double precision (AFNI uses float32): float32
  outputs differ in the last bits, integer outputs occasionally round the other way.
- No history extension; AFNI's version banner and compile date are not printed. The other
  messages are AFNI's, with larmorx's own lines added (the defect notices, `-verbose`'s first
  two lines, warnings about global options ending in `(larmorx)`).
- `3dTshift -h` prints the help (AFNI prints nothing and exits 1); `-ignore` without a value
  is an error (AFNI crashes); an output that cannot be written is an error, exit 1 (AFNI
  exits 0); in 1D files, hexadecimal numbers and signed infinities are errors (AFNI reads
  them), and an inline `1D:` value such as `0:1` is an error.

## Known last-bit differences

Within the thresholds, but not bit-identical yet (each affects the last float32 bit of some
values; integer outputs only where a value sits on a rounding boundary):

- **Weighted sinc**: AFNI's weights use glibc's float `sinf`/`cosf`, which are not correctly
  rounded; larmorx uses correctly rounded functions ([why](../findings/platform-math.md)).
  Calling glibc's functions in a model of the computation reproduces AFNI exactly, so this
  is the only cause.
- **Quintic and heptic**: for some fractions (real slice times), a few weights differ from
  AFNI's by an ulp or more; linear and cubic are exact. Unresolved.
- **`-no_detrend`**: the pair structure is reproduced, its rounding not yet.

## Rust

```rust
use larmorx_afni::dataset::{self, OutputTiming};
use larmorx_afni::settings::AfniSettings;
use larmorx_afni::timing::{Pattern, mean_time};
use larmorx_afni::tshift::{Method, Restore, TshiftParams, tshift};
use larmorx_core::UpstreamDefects;

let defects = UpstreamDefects::Fix; // or Reproduce: AFNI's output
let settings = AfniSettings::new(); // AFNI's variables (AfniSettings::from_env() reads them)
let mut image = dataset::read("bold.nii.gz", "bold.nii.gz", 8, defects, &settings)?;
let nz = image.bricks.shape[2];
let slice_times = Pattern::AltPlus.times(nz, image.tr);
let params = TshiftParams {
    tr: image.tr,
    tzero: mean_time(&slice_times),
    slice_times,
    ignore: 0,
    method: Method::Fourier,
    restore: Restore::Trend,
    detrend: true,
    n_threads: 8,
    upstream_defects: defects,
};
let notices = tshift(&mut image.bricks, &params)?; // the defects that changed the output
let timing = OutputTiming { tr: params.tr, toffset: params.tzero, slices: None };
let (header, geometry_notices, messages) =
    dataset::output_header(&image.geometry, &image.bricks, &timing, None, defects, &settings)?;
dataset::write("bold_stc.nii.gz", &header, &image.bricks, 8)?;
```

`Bricks` holds a dataset as AFNI does (bytes, shorts or floats, a brick factor per volume),
so `tshift` reproduces AFNI's rounding when values are stored back. `dataset::read` takes the
name AFNI shows in its messages (`larmorx_afni::input::resolve` finds it, with AFNI's rules
for `~/`, blanks and `.gz`); `output_header` returns AFNI's header with the notices of AFNI-07
and AFNI-16 and AFNI's messages about it. The command line is `larmorx_afni::cli::tshift`.
