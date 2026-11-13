# ThresholdImage

`lx.ants.threshold_image`, `lx.ants.otsu_threshold` (command line `larmorx ants
ThresholdImage`): an image thresholded to two values between two bounds, or into classes by
Otsu's thresholds, as ANTs 2.6.5's `ThresholdImage` computes them.

The port comes first, then the program's section, the source of its reference page on the
documentation site (`python -m larmorx_validation reference`), with its options table checked
against the code; then the speed, the behaviour worth knowing and ANTs' defects as the port
corrects them.

## Ports

`ThresholdImage` has one port, in `pip install larmorx`: **the replica** (`crates/larmorx-ants`,
`crates/larmorx-image`; Apache-2.0; Rust `larmorx_ants::threshold_image`,
`larmorx_image::threshold`), translated from ANTs 2.6.5 on ITK 5.4.5 (Apache-2.0).

## `ThresholdImage`

<!-- reference
program: ThresholdImage
group: ants
original: ANTs 2.6.5
python: larmorx.ants.threshold_image
command-line: crates/larmorx-ants/src/cli/threshold_image.rs
records: {replica: [ants-threshold-image]}
evidence: ants-programs
-->

`ThresholdImage` sets the voxels between two bounds to one value and the others to another, or
labels an image's classes by Otsu's thresholds. sMRIPrep's brain extraction, which fMRIPrep runs,
binarises a brain probability map with it (`0.5 1 1 0`).

### Python (`ThresholdImage`)

`lx.ants.threshold_image(image, lower, upper, inside=1.0, outside=0.0, *, n_threads=1)`

Returns a float32 `lx.Image`: `inside` where `lower <= v <= upper` (the bounds compared in
float32), else `outside`; `lower > upper` raises `ValueError` (ANTs aborts). Otsu's mode is a
function of its own, `lx.ants.otsu_threshold(image, n_thresholds=1, *, mask=None, n_threads=1,
upstream_defects="fix")`, which returns `OtsuResult(image, thresholds)`, the thresholds being
the upper bounds of the chosen histogram bins.

```python
import larmorx as lx

# fMRIPrep binarises a brain probability map: 1 where 0.5 <= p <= 1, else 0
mask = lx.ants.threshold_image("prob.nii.gz", 0.5, 1.0)

# Otsu: three classes inside a brain mask (labels 1..3, 0 outside)
r = lx.ants.otsu_threshold(t1w, 2, mask=brain_mask)
r.image, r.thresholds
```

### Options of `ThresholdImage`

| ANTs argument (positional) | Python | Notes |
|---|---|---|
| 1 `ImageDimension` | – | 2, 3, 4; Python takes any array shape |
| 2 `ImageIn.ext` | `image` | path, `lx.Image`, nibabel image or `(array, affine)` |
| 3 `outImage.ext` | the result | float32, as ANTs writes it |
| 4 `threshlo` | `lower` | inclusive; read with `atof`, then compared in float |
| 5 `threshhi` | `upper` | inclusive |
| 6 `insideValue` | `inside` | default 1 |
| 7 `outsideValue` | `outside` | default 0 |
| 4 `Otsu`, 5 `NumberofThresholds` | `lx.ants.otsu_threshold(image, n_thresholds)` | ITK's `OtsuMultipleThresholdsImageFilter`, 128 bins, labels `0..n` |
| 6 `maskImage.ext` (with `Otsu`) | `lx.ants.otsu_threshold(..., mask=...)` | ANTs' own code: region = mask value 1 (as `int`), 200 bins, labels `1..n+1` inside, 0 outside; the output takes the mask's geometry |
| 4 `Kmeans`, 5 `NumberofThresholds`, 6 `maskImage.ext` | – | not supported yet |
| `ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS` (or `NSLOTS`, as ITK reads them) | `n_threads` | the result does not depend on it |

### Examples of `ThresholdImage`

<!-- inputs: prob.nii.gz=head mask.nii.gz=head-mask -->

```python example
import larmorx as lx

# 1 where 300 <= v <= 700 (the head's brain and skull), else 0.
inside = lx.ants.threshold_image("prob.nii.gz", 300.0, 700.0)
sorted(set(inside.data.ravel().tolist()))  # -> [0.0, 1.0]
# Otsu's single threshold inside the mask: labels 1 and 2 there, 0 outside.
r = lx.ants.otsu_threshold("prob.nii.gz", 1, mask="mask.nii.gz")
r.image.data.dtype  # -> dtype('float32')
```

```bash example
larmorx ants ThresholdImage 3 prob.nii.gz binary.nii.gz 300 700 1 0
larmorx ants ThresholdImage 3 prob.nii.gz otsu.nii.gz Otsu 1 mask.nii.gz
```

### Ports of `ThresholdImage`

**The replica: status `validated`** against ThresholdImage itself (its
[record](../validation/ants-threshold-image.md), 2026-10-12): 41 cases, 35 compared, 5 rejected
by both alike, 1 expected divergence (`Kmeans`, not supported yet); every compared case
bit-identical, with ANTs' exact header bytes. Compared with ANTs' defects reproduced
(`LARMORX_UPSTREAM_DEFECTS=reproduce`); the record's "D13 fix pass" lists what larmorx's default
changes.

**On the data panel** ([record](../validation/ants-threshold-image-panel.md), 2026-10-26):
sMRIPrep's `ThresholdImage` in the brain extraction of all fourteen finished oracle runs, each
replayed from its node with its inputs, and four calls fMRIPrep never makes (Otsu with 4, 3 and 2
thresholds on the 7 T MP2RAGE, the 82-year-old's FLAIR and the 1.5 T T1w as float; a range on the
stroke's lesion mask): **18 of 18 bit-identical, header bytes included**.

## Speed

From the [benchmark](../benchmarks/ants-programs.md): `0.5 1` on the 1 mm MNI probability map 100
ms on one thread, 49 ms on 12 (ANTs: 115 ms and 103 ms); `Otsu 3` with a mask on fMRIPrep's T1w
205 ms and 125 ms (ANTs: 558 ms and 315 ms).

## Behaviour worth knowing

From ANTs ([findings](../findings/ants-image-programs.md#thresholdimage)):
- the sixth argument is first read as a mask image, even for a range threshold (so
  `... 0.5 1 255 0` prints " file 255 does not exist . ");
- any mode other than exactly `Otsu` or `Kmeans` is a range threshold's lower bound; bounds
  may be `nan`, `inf` or hexadecimal (`strtod`);
- `lower > upper` aborts ANTs (larmorx: exit 1; Python: `ValueError`); NaN voxels are
  outside; non-finite input values are read as 0 anyway (ITK's NIfTI reader);
- Otsu keeps the first of equally good threshold sets (a new maximum must be larger by
  more than one ulp).

## Upstream defects (D13)

ANTs' deterministic defects are corrected by default and reproduced bit for bit on request:
`upstream_defects="reproduce"` in Python, `LARMORX_UPSTREAM_DEFECTS=reproduce` on the command
line. A warning (on the command line a line on standard error) names the entry wherever it
changed the result ([upstream-defects.md](upstream-defects.md); each entry in the
[register](../findings/upstream-defects.md)).

| ID | ANTs (what `reproduce` gives) | larmorx's default |
|---|---|---|
| ANTS-24 | a mode that is neither `Otsu`, `Kmeans` nor a number is the lower bound `atof(mode)`: `otsu 2` thresholds at [0, 2] | an error (exit 1); a lower bound is text that `strtod` reads entirely |
| ANTS-25 | `otsu_threshold(..., mask=...)`, ANTs' own masked Otsu, starts the maximum at `FLT_MIN` and finds the range with `else if`: the first voxel is never a candidate for the minimum, and an image without positive values keeps the maximum `FLT_MIN` | the true range of the voxels in the mask |
| ANTS-03, ITK-02, ITK-08 | as for every ANTs program: names of fewer than 3 characters, the direction of a file with more dimensions, `x.nii` read for `x.nii.gz` ([ImageMath](ants-image-math.md#upstream-defects-d13)) | as there |

## Deliberate differences

- `Otsu 0` hangs ANTs (an endless loop in ITK's calculator); larmorx reports an error.
- Where ANTs crashes or aborts, larmorx exits 1 with a message.
