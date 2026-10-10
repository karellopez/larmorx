# 3dTshift

`lx.afni.tshift` (command line `larmorx afni 3dTshift`; Rust crate `larmorx-afni`):
slice-timing correction, compatible with AFNI 25.2.09's `3dTshift`.

**Clean-room re-implementation.** AFNI is GPL-2, so larmorx did not port it: the code was
written from the behaviour spec [`specs/3dTshift.md`](../../specs/3dTshift.md), AFNI's help
text and black-box runs of the AFNI binary, without AFNI's source code
([provenance](../../crates/larmorx-afni/PROVENANCE.md)).

**Status: `validated`** against AFNI 25.2.09 itself (see the
[validation record](../validation/afni-tshift.md)). The record covers 236 cases:
- every file of `larmorx-testdata` `synthetic/tshift/` (every FFT length class, every data
  type AFNI reads, edge content);
- every method, option and `-tpattern` form, slice timing from NIfTI headers, the "copy of
  input" cases and 17 errors both must raise;
- 20 runs on real BOLD data (ds000210 multi-echo, ds001600, ds003345, ds005454 multiband
  with 96 slices, ds006010, ds006736), called as fMRIPrep calls 3dTshift.

All 236 agree, and 158 of the 217 compared are byte-identical. The Lagrange and
weighted-sinc methods and the copies give **the same bytes** as AFNI (117 of 120). The
Fourier method (the default) differs in the last float32 bits, at most 4.3e-7 of the largest
value, because AFNI's FFT is float32 and larmorx's is double; integer outputs then differ by
at most 1, in a few values per million on the real runs (up to 0.2 % on a synthetic series
built to clip). The known last-bit differences are listed at the end of this page.

**Bit-exact replica.** `larmorx-gpl afni 3dTshift` (package `larmorx-gpl`, `crates-gpl/`,
GPL-3.0-or-later) is a translation of AFNI's own source, with AFNI's float32 FFT and a port of
glibc's `sinf`/`cosf`. It takes the same arguments as `larmorx afni 3dTshift`, follows the
same conventions (NIfTI output only, an existing output is an error), and gives AFNI's output
bytes on every compared parity case ([replica record](../validation/afni-tshift-replica.md)).
It is a separate program because of its licence ([licensing](../licensing.md)); choosing it
from Python (`implementation="replica"`) is planned.

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

## `lx.afni.tshift(image, *, slice_times, tr=None, tzero=None, slice=None, ignore=0, method="fourier", restore="trend", detrend=True, n_threads=1)`

Each voxel's time series is detrended (least-squares line), shifted in time so that its slice
refers to `tzero`, clipped to its own range, and given its trend back. The output at time
point `n` estimates the signal at `n·TR + tzero`.

Returns an `lx.Image` with the header `3dTshift` writes: `pixdim[4]` = TR (seconds),
`toffset` = `tzero`, no slice timing, millimetres and seconds. Data keep AFNI's storage type
(`uint8`, `int16` or `float32`); when the output has a brick factor (`scl_slope`), the data
are the scaled values, as `lx.load` returns them.

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
| `-no_detrend` | `detrend=False` | AFNI's voxel-pair behaviour is reproduced, with a warning (below) |
| `-verbose` | – | progress on stderr |
| `-voxshift dset` | – | not supported |
| (threads: `OMP_NUM_THREADS`) | `n_threads=1` (0 = all CPUs) | AFNI is single-threaded; the result does not depend on the thread count |

## How AFNI reads and writes NIfTI (reproduced)

| Stored type | Header scaling | Kept as | Values |
|---|---|---|---|
| uint8, int16, float32 | `scl_slope` ≠ 0 and `scl_inter` ≠ 0 | float32 | `slope·x + inter`, in double from float32 values |
| uint8, int16, float32 | otherwise | as stored | a slope other than 0 or 1 becomes the brick factor |
| int8, uint16, int32, uint32, float64 | any | float32 | converted, then scaled if the slope is not 0 |

Non-finite values read as 0. Outputs keep the type and brick factor (`scl_slope`); integers are
rounded half to even and clamped (shorts to ±32767). A **negative** `scl_slope` is applied on
output but not on input, so AFNI's output is sign-flipped; larmorx reproduces this and warns.
The output header is NIfTI-1 with qform and sform both from AFNI's matrix (the sform if
`sform_code > 0`, else the qform), codes mapped 1, 2 → 1, 3, 5 → 3, 4 → 4, the slice axis
third (`dim_info` 48) and slice range `0..nz−1`; AFNI's history extension is not written.

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
  (zeros beyond the ends, clipped to its range). The last bits of these cases are not
  bit-identical yet.
- **`-TR … ms`** switches every time to milliseconds (header offsets, `-tzero`, `@file`
  values are taken as milliseconds); the output is written in seconds.
- **`@file`**: AFNI 1D text read column by column (one line reads left to right); `#`
  comments, commas and `N@value` repeats work; the first row sets the number of columns.
- **Copies of the input** (no slice timing, one volume): the input as AFNI read it, with its
  TR and `toffset`, even when `-TR` is given.

## Deliberate differences

- Output is NIfTI only (`-prefix` must end in `.nii` or `.nii.gz`); an existing output is an
  error (AFNI warns and exits 0 without writing).
- `-voxshift`, AFNI's own formats, sub-brick selectors and 1D selectors (`file[2]`,
  `file'`) are not supported; hexadecimal numbers in 1D files are rejected.
- The Fourier method uses larmorx's own FFT in double precision (AFNI uses float32): float32
  outputs differ in the last bits, integer outputs occasionally round the other way.
- No history extension; messages differ from AFNI's, exit codes do not.

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
use larmorx_afni::timing::{Pattern, mean_time};
use larmorx_afni::tshift::{Method, Restore, TshiftParams, tshift};

let mut image = dataset::read("bold.nii.gz", 8)?;
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
};
tshift(&mut image.bricks, &params)?;
let timing = OutputTiming { tr: params.tr, toffset: params.tzero, slices: None };
let header = dataset::output_header(&image.header, &image.bricks, &timing)?;
dataset::write("bold_stc.nii.gz", &header, &image.bricks, 8)?;
```

`Bricks` holds a dataset as AFNI does (bytes, shorts or floats, a brick factor per volume),
so `tshift` reproduces AFNI's rounding when values are stored back.
