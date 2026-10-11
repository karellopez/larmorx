# ThresholdImage

`lx.ants.threshold_image`, `lx.ants.otsu_threshold` (command line
`larmorx ants ThresholdImage`; Rust `larmorx_ants::threshold_image`,
`larmorx_image::threshold`). A replica of ANTs 2.6.5 on ITK 5.4.5 (Apache-2.0).

**Status: `validated`** against ThresholdImage itself (see the
[validation record](../validation/ants-threshold-image.md)): 41 cases, every compared case
bit-identical, with ANTs' exact header bytes. `Kmeans` is not supported yet.

**Speed** ([benchmark](../benchmarks/ants-programs.md)): `0.5 1` on the 1 mm MNI probability
map 100 ms on one thread, 49 ms on 12 (ANTs: 115 ms and 103 ms); `Otsu 3` with a mask on
fMRIPrep's T1w 205 ms and 125 ms (ANTs: 558 ms and 315 ms).

## Quick start

```python
import larmorx as lx

# fMRIPrep binarises a brain probability map: 1 where 0.5 <= p <= 1, else 0
mask = lx.ants.threshold_image("prob.nii.gz", 0.5, 1.0)

# Otsu: three classes inside a brain mask (labels 1..3, 0 outside)
r = lx.ants.otsu_threshold(t1w, 2, mask=brain_mask)
r.image, r.thresholds
```

```bash
larmorx ants ThresholdImage 3 prob.nii.gz mask.nii.gz 0.5 1 1 0
larmorx ants ThresholdImage 3 t1w.nii.gz otsu.nii.gz Otsu 2 brain_mask.nii.gz
```

## Option mapping

| ThresholdImage | Python | Notes |
|---|---|---|
| `d in out lo hi` | `threshold_image(in, lo, hi)` | inclusive; bounds compared in float32 (`atof`, then float) |
| `... inside outside` | `inside=1.0, outside=0.0` | |
| `d in out Otsu n` | `otsu_threshold(in, n)` | ITK's `OtsuMultipleThresholdsImageFilter`, 128 bins, labels `0..n` |
| `d in out Otsu n mask` | `otsu_threshold(in, n, mask=mask)` | ANTs' own code: region = mask value 1 (as `int`), 200 bins, labels `1..n+1` inside, 0 outside; the output takes the mask's geometry |
| `d in out Kmeans n [mask]` | – | not supported yet |
| `d` = 2, 3, 4 | any array shape | |

Outputs are float32, as ANTs writes them. `otsu_threshold` returns `OtsuResult(image,
thresholds)`, the thresholds being the upper bounds of the chosen histogram bins.

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
