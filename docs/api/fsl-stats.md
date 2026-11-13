# fslstats (image statistics)

`lx.fsl.fslstats` (command line `larmorx fsl fslstats`): FSL 6.0.7.23's `fslstats`, which prints
statistics of an image. Beside the generic function that mirrors the command line, each statistic
has a typed wrapper (`lx.fsl.image_mean`, `image_std`, `image_range`, `centre_of_gravity`, ...)
that returns numbers.

The ports come first, then the program's section, the source of its reference page on the
documentation site (`python -m larmorx_validation reference`), with its options table checked
against the code; then the typed wrappers, the records, FSL's defects as both ports correct them
and what is not done yet. The page's dates are in its [History](#history).

## Ports

FSL is distributed under the FSL Licence (non-commercial use only), so `fslstats` has two ports,
a replica and a clean room, in different packages ([licensing](../licensing.md)):

- **The replica** is bit-exact: `larmorx-nc fsl fslstats`, in the package `larmorx-nc`, under the
  FSL Licence's terms, translated from FSL's source. The program is in `crates-nc/larmorx-nc-core`,
  because BET's script runs it, with its command line in `crates-nc/larmorx-nc-avwutils`.
- **The clean room** is in the main package (Apache-2.0, crate `larmorx-fsl`, its
  `cli::fslstats`), written from [`specs/fslstats.md`](../../specs/fslstats.md), FSL's
  documentation and black-box runs of FSL's program
  ([PROVENANCE](../../crates/larmorx-fsl/PROVENANCE.md)).

`implementation="auto"` (the default) runs the replica when `larmorx-nc` is installed, the clean
room otherwise; `"replica"` needs `larmorx-nc` (otherwise `ReplicaNotFoundError`); `"clean-room"`
(`"original"`, 0.1.0rc1's spelling, still works) runs the clean room in this process on
`n_threads` threads (its results the same at any count; a failure raises `RuntimeError` with its
messages). The command line chooses alike (`--implementation` before the family, or
`LARMORX_IMPLEMENTATION`).

## `fslstats`

<!-- reference
program: fslstats
group: fsl
original: FSL 6.0.7.23
python: larmorx.fsl.fslstats
command-line: crates/larmorx-fsl/src/cli/fslstats.rs
records: {replica: [fsl-stats-replica], clean-room: [fsl-stats]}
evidence: fsl-stats
-->

`fslstats` prints statistics of an image, per volume, per label or as JSON. Its options apply in
order: thresholds, masks, absolute values and a difference image change what the later options
see; the others print the mean, standard deviation, minimum and maximum, robust range,
percentiles, entropy, voxel count and volume, centre of gravity, the extremes' coordinates, the
bounding box of the nonzero voxels or a histogram. With `-t` it prints one line per volume, with
`-K` one per label of an index image, with `-json` everything as JSON.

**Order matters**: `-k` replaces the masks the thresholds made, as a later threshold replaces
`-k`'s, so `fslstats img -M -l 10 -M` reports the nonzero mean, applies a threshold, then reports
the new nonzero mean.

### Python (`fslstats`)

`lx.fsl.fslstats(image, *options, timeseries=False, index_mask=None, json=False, n_threads=1, implementation="auto", upstream_defects="fix")`

- `image`, `index_mask` and the image operands (`-k`, `-d`) are `lx.Image`s, nibabel images,
  `(array, affine)` pairs or paths (in-memory images written for the program in their own data
  type); options are fslstats' own words, numbers passed as their shortest round-trip text.
- **The program runs in the caller's working directory**: relative names are read there.
- **Numbers**: the text output has six decimals (`%.6f`), as nipype reads it for fMRIPrep;
  `json=True` gives each number at full precision (a decimal that reads back to the same double,
  as nlohmann/json writes it; NaN and the infinities as `null`).
- FSL's environment variables are settings of the call, never read from the process. FSL's
  fslstats runs on one thread; the clean room runs on `n_threads`, with the same results at any
  count.
- Errors: `ValueError` for invalid arguments, `larmorx.ReplicaError` when the replica fails (its
  message in the error, without the usage text), `RuntimeError` when the clean room does,
  `larmorx.ReplicaNotFoundError` without `larmorx-nc`.
- The typed wrappers (`lx.fsl.image_mean` and the others in the options table) choose the voxels
  in a fixed order ([The typed wrappers](#the-typed-wrappers)); for another order call
  `fslstats()`.

```python
import larmorx as lx

# fMRIPrep's goodvoxels mask: fslstats <cov_ribbon> -M, then -S (the nonzero voxels)
m = lx.fsl.image_mean("cov_ribbon.nii.gz", nonzero=True)
s = lx.fsl.image_std("cov_ribbon.nii.gz", nonzero=True)
upper = m + 0.5 * s

# BET's script: the centre of gravity, in voxels, of the voxels above a threshold
cog = lx.fsl.centre_of_gravity("t1w.nii.gz", lower=0.001, voxels=True)

# per volume, per label, in a mask
means = lx.fsl.image_mean("bold.nii.gz", per_volume=True)            # shape (n_volumes,)
by_label = lx.fsl.image_mean("t1w.nii.gz", labels="labels.nii.gz")   # shape (n_labels,)

# any option list, in fslstats' own words; JSON for full precision
text = lx.fsl.fslstats("t1w.nii.gz", "-M", "-l", 10, "-M")
doc = lx.fsl.fslstats("t1w.nii.gz", "-r", "-p", 50, json=True)
```

### Options of `fslstats`

| `fslstats` option | Python | Notes |
|---|---|---|
| the first word after the preoptions | `image` | the image |
| `-t` | `timeseries=True` | one line per volume |
| `-K` | `index_mask` | one line per label 1..n of the 3D label image |
| `-json` | `json=True` | the numbers in JSON, at full precision; NaN and the infinities as `null` |
| `-l`, `-u` | `options` | the voxels strictly between (the typed wrappers' `lower=`, `upper=`) |
| `-k` | `options` | the voxels where the mask is above 0.5 (it overrides thresholds, as a later threshold overrides it; the typed wrappers' `mask=`) |
| `-d` | `options` | the image minus this one first (the typed wrappers' `difference=`) |
| `-a` | `options` | absolute values first (the typed wrappers' `absolute=True`) |
| `-n` | `options` | NaN and infinities as 0 first (the typed wrappers' `nan_as_zero=True`) |
| `-m`, `-M` | `options`; `lx.fsl.image_mean(image, nonzero=False)` | the mean (of the nonzero voxels) |
| `-s`, `-S` | `options`; `lx.fsl.image_std(image, nonzero=False)` | the standard deviation (of the nonzero voxels) |
| `-p`, `-P` | `options`; `lx.fsl.image_percentile(image, percent, nonzero=False)` | the percentile, `percent` in [0, 100] |
| `-R`, `-r` | `options`; `lx.fsl.image_range(image, robust=False)` | `[min, max]`, or the robust range (FSL's 2 %-98 % histogram limits) |
| `-e`, `-E` | `options`; `lx.fsl.image_entropy(image, nonzero=False)` | the mean entropy of the intensities' histogram |
| `-v`, `-V` | `options`; `lx.fsl.image_volume(image, nonzero=False)` | `[voxels, mm³]` |
| `-c`, `-C` | `options`; `lx.fsl.centre_of_gravity(image, voxels=False)` | `[x, y, z]` in mm, or in voxel coordinates |
| `-x`, `-X` | `options`; `lx.fsl.extremum_position(image, minimum=False)` | the voxel coordinates of the maximum (minimum) |
| `-w` | `options`; `lx.fsl.bounding_roi(image)` | `[xmin, xsize, ymin, ysize, zmin, zsize, tmin, tsize]` of the nonzero voxels, as `lx.fsl.roi` takes a region |
| `-h`, `-H` | `options`; `lx.fsl.image_histogram(image, bins, limits=None)` | the counts in `bins` bins over the image's range (`-h`) or `limits` (`-H`) |
| `-D` | `options` | (hidden in FSL's usage) the sum of every value, masks ignored (`specs/fslstats.md` §5.2) |
| `LARMORX_UPSTREAM_DEFECTS` | `upstream_defects` | `"reproduce"`: FSL's defects as FSL has them |


### Examples of `fslstats`

<!-- inputs: t1w.nii.gz=head bold.nii.gz=head4d -->

```python example
import larmorx as lx

# BET's script: the centre of gravity, in voxels, of the voxels above a threshold
cog = lx.fsl.centre_of_gravity("t1w.nii.gz", lower=0.001, voxels=True)
len(cog)  # -> 3
# One mean per volume of a series.
lx.fsl.image_mean("bold.nii.gz", per_volume=True).shape  # -> (4,)
# Any option list, in fslstats' own words: the nonzero mean, then above a threshold.
lx.fsl.fslstats("t1w.nii.gz", "-M", "-l", 100, "-M")  # -> '124.499801 498.625669 \n'
```

```bash example
larmorx fsl fslstats t1w.nii.gz -M -S
larmorx fsl fslstats -t bold.nii.gz -m
larmorx fsl fslstats -json t1w.nii.gz -r -p 50
```

### Ports of `fslstats`

Both ports take every option, preoption (`-t`, `-K`, `-json`), message, exit status and failure
case of FSL's, and correct the defects FSL-181 to FSL-189.

**The replica** against FSL 6.0.7.23 itself ([its record](../validation/fsl-stats-replica.md),
the standard comparison, 2026-11-03): 172 of 172 compared cases identical, their standard output,
standard error and exit status; 36 more rejected by both alike. Among them fMRIPrep's four
goodvoxels calls (`-M` and `-S` of the ribbon's coefficient of variation, before and after its
modulation) on ds005040's chain (the ribbon a stand-in: the segmentation's grey matter, as the
no-reconall runs have no FreeSurfer ribbon), BET's `-l 0.001 -C` on ds005040's T1w, the option
lists of FSL's own test script against its expected outputs, and random testing's permanent cases.

**The clean room** against FSL 6.0.7.23 ([its record](../validation/fsl-stats.md), the standard
comparison, 2026-11-04, `specs/fslstats.md` §10's thresholds): 162 cases, 126 within the
thresholds, 36 rejected by both alike, 0 failures. 125 of the 126 are identical to FSL's (standard
output, standard error, exit status), fMRIPrep's four goodvoxels calls on ds005040's chain among
them; BET's `-l 0.001 -C` on ds005040's T1w is within §10's bound (its x 6.1e-8 relative from
FSL's).

## The typed wrappers

Each runs `fslstats [-t] [-K labels] <image> [-d difference] [-a] [-n] [-l lower] [-u upper] [-k
mask] <statistic>` and returns the statistic's numbers as fslstats prints them (six decimals): a
float or an array; with `per_volume` one result per volume (first axis), with `labels` one per
label 1..n (NaN for a label the image lacks), with both an array of volumes by labels.

| fslstats | `lx.fsl` | Returns |
|---|---|---|
| `-m`, `-M` | `image_mean(image, nonzero=False)` | the mean (of the nonzero voxels) |
| `-s`, `-S` | `image_std(image, nonzero=False)` | the standard deviation (of the nonzero voxels) |
| `-p <n>`, `-P <n>` | `image_percentile(image, percent, nonzero=False)` | the percentile, `percent` in [0, 100] |
| `-R`, `-r` | `image_range(image, robust=False)` | `[min, max]`, or the robust range (FSL's 2 %-98 % histogram limits) |
| `-e`, `-E` | `image_entropy(image, nonzero=False)` | the mean entropy of the intensities' histogram |
| `-v`, `-V` | `image_volume(image, nonzero=False)` | `[voxels, mm³]` |
| `-c`, `-C` | `centre_of_gravity(image, voxels=False)` | `[x, y, z]` in mm, or in voxel coordinates |
| `-x`, `-X` | `extremum_position(image, minimum=False)` | the voxel coordinates of the maximum (minimum) |
| `-w` | `bounding_roi(image)` | `[xmin, xsize, ymin, ysize, zmin, zsize, tmin, tsize]` of the nonzero voxels, as `lx.fsl.roi` takes a region |
| `-h <n>`, `-H <n> <min> <max>` | `image_histogram(image, bins, limits=None)` | the counts in `bins` bins over the image's range or `limits` |
| `-k <mask>` | `mask=` | the voxels where the mask is above 0.5 (it overrides thresholds: give one or the other, else `ValueError`) |
| `-l`, `-u` | `lower=`, `upper=` | the voxels strictly between |
| `-d <image>` | `difference=` | the image minus this one first |
| `-a` | `absolute=True` | absolute values first |
| `-n` | `nan_as_zero=True` | NaN and infinities as 0 first |
| `-t` | `per_volume=True` | one result per volume |
| `-K <labels>` | `labels=` | one result per label |
| `-json`, `-D`, other orders | `fslstats(image, *options, json=True)` | the text or the JSON |
| | `n_threads`, `implementation`, `upstream_defects` | as `fslstats()`'s (`"reproduce"`: FSL's defects, below) |

The typed wrappers apply the selection in the fixed order above; for another order call
`fslstats()`.

## The records

Random testing of the replica (2026-11-03, 24 minutes: 69,077 realistic option lists, the
preoptions and fMRIPrep's and BET's forms among them): every one identical after two fixes (the
centre of gravity's NaN signs, and `-json`'s notices before an error).

The clean room's record holds the replica suite's cases but its `fsltest/` ones, BET's call, the
clean room's own `orig/` cases and 4 permanent cases of random testing. Random testing's trial
(2026-11-03, beside fslmaths': 150,000 realistic option lists): 149,214 identical, 7 differing
only by a named correction; of the 779 others, 545 fixed by the part's correction, 74 by
corrections of the suite's checks, 160 explained (`-c` and `-C` of images holding NaN and
infinities: the NaN's sign; the last printed digit at §10's bound), none open. The correction:
with an empty mask `-h` and `-H` print `findHistogram: mask is empty` and then the histogram of
zeros (the JSON holds the zeros), and without a histogram at all (a constant image's `-h`,
FSL-186's `-H`) the text holds nothing and the JSON an empty list
([findings](../findings/fsl-maths-original.md) 28).

## File names on Windows

As fslmaths' ([fsl-maths](fsl-maths.md)).

## Defects of the original, corrected by default

fslstats' defects are corrected by default and reproduced with `upstream_defects="reproduce"`
(`LARMORX_UPSTREAM_DEFECTS=reproduce`); each correction that changes the printed output is named
on standard error (`larmorx-nc: upstream defect FSL-<n> (fslstats) corrected: ...`) and as an
`UpstreamDefectWarning` ([all of them](upstream-defects.md)):

- **FSL-181**: NaN values in the order statistics (a NaN first value makes `-R` NaN and `-x`, `-X`
  the first voxel; `-p`, `-P` sort values holding NaN arbitrarily; the histograms and the robust
  range count NaN in their first bin): NaN values left out.
- **FSL-182**: `-s` and `-S` without a mask: the variance not clamped (NaN for one value, or for
  a nearly constant image), `-S`'s squares in single precision: the masked forms.
- **FSL-183**: `-t` with a 4D `-k` or `-d` image fails: the time point's volume is used.
- **FSL-184**: `-x`, `-X` of an empty mask crash: refused (both modes).
- **FSL-185**: `-e`, `-E` of a constant image crash: entropy 0.
- **FSL-186**: `-H` with a maximum not above its minimum prints nothing: an error.
- **FSL-187** (shared with fslmaths, BET and FLIRT): a value far above a histogram's range counts
  in its first bin (`-H`, `-e`, `-E`, `-r`): the last.
- **FSL-188**: `-w` of a 4D image without nonzero voxels gives a negative time range: the whole
  range.
- **FSL-189**: `-json`'s voxel counts are written in single precision: exact.

### glibc's maths functions (GLIBC-07)

FSL's fslstats takes the entropies' logarithms (`-e`, `-E`) from glibc 2.35's `log`, which is not
correctly rounded. The clean room computes them correctly rounded by default and takes glibc's
with `upstream_defects="reproduce"` (`larmorx_core::math`'s `*_glibc` functions, proved bit for
bit against glibc 2.35; the user's decision of 2026-11-04: glibc's bits for `reproduce` only).
Both modes name GLIBC-07 exactly where the other choice prints another value: with six
significant digits, practically never.

## Not yet

- The clean room's spec questions (its findings). Its speed: the
  [benchmarks](../benchmarks/fsl-maths.md) (BET's call and a 63-volume `-t -M -S -r`).
- zstd and bzip2 inputs are decompressed whole before they are read (about twice the memory of
  gzip ones; fMRIPrep writes neither).
- The goodvoxels calls with a real cortical ribbon (it needs a run of FreeSurfer), the full
  tier, the data panel and long random testing: in the final validation phase (the site's
  evidence page, `/evidence/fsl-stats/`, shows fMRIPrep's calls with every case of the record).

## History

- **The replica** (2026-11-03): [PROVENANCE](../../crates-nc/larmorx-nc-core/PROVENANCE.md);
  random testing the same day.
- **The clean room** (from 2026-11-03; its record of 2026-11-04); random testing's trial
  (2026-11-03).
- **Status: `experimental`.**
