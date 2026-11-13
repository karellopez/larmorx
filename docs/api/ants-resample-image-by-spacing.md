# ResampleImageBySpacing

`lx.ants.resample_image_by_spacing` (command line `larmorx ants ResampleImageBySpacing`): an
optional recursive Gaussian smoothing, then ITK's `ResampleImageFilter` with an identity
transform onto a grid with a new spacing, as ANTs 2.6.5's `ResampleImageBySpacing` computes them.

The port comes first, then the program's section, the source of its reference page on the
documentation site (`python -m larmorx_validation reference`), with its options table checked
against the code; then the speed, the behaviour worth knowing and ANTs' defects as the port
corrects them.

## Ports

`ResampleImageBySpacing` has one port, in `pip install larmorx`: **the replica**
(`crates/larmorx-ants`, `crates/larmorx-image`; Apache-2.0; Rust
`larmorx_ants::resample_image_by_spacing`, `larmorx_ants::resample`), translated from ANTs 2.6.5
on ITK 5.4.5 (Apache-2.0).

## `ResampleImageBySpacing`

<!-- reference
program: ResampleImageBySpacing
group: ants
original: ANTs 2.6.5
python: larmorx.ants.resample_image_by_spacing
command-line: crates/larmorx-ants/src/cli/resample_image_by_spacing.rs
records: {replica: [ants-resample-image-by-spacing]}
evidence: ants-programs
-->

`ResampleImageBySpacing` resamples an image onto a grid with a new spacing, keeping its origin
and direction, after smoothing it with a Gaussian where the spacing grows, by linear or
nearest-neighbour interpolation.

### Python (`ResampleImageBySpacing`)

`lx.ants.resample_image_by_spacing(image, spacing, *, smooth=True, add_voxels=0, nearest=False, n_threads=1, upstream_defects="fix")`

Returns a float32 `lx.Image` with the new spacing; the Python result keeps the input's header,
with the new time step in 4D.

```python
import larmorx as lx

low = lx.ants.resample_image_by_spacing("sub-01_T1w.nii.gz", [2.0, 2.0, 2.0])
up = lx.ants.resample_image_by_spacing(boldref, [1.0, 1.0, 1.0], smooth=False)
labels = lx.ants.resample_image_by_spacing(seg, [1, 1, 1], smooth=False, nearest=True)
```

### Options of `ResampleImageBySpacing`

| ANTs argument (positional) | Python | Notes |
|---|---|---|
| 1 `ImageDimension` (2, 3, 4) | `len(spacing)` | a path is read in that many dimensions; an in-memory image must have them |
| 2 `inputImageFile` | `image` | path, `lx.Image`, nibabel image or `(array, affine)`; read as `float` |
| 3 `outputImageFile` | the result | float32; same origin and direction, the new spacing |
| 4 `outxspc`, 5 `outyspc`, then `outzspacing` (and the time step in 4D) | `spacing` | one value per axis (`atof`); 4D: the last is the time step in seconds |
| `dosmooth?` (default 1), after the spacings | `smooth=True` | per axis, a recursive Gaussian of sigma `out/in − 1` input voxels where positive (ANTs: the same number in mm, ANTS-32) |
| `addvox` (default 0), after it | `add_voxels=0` | added to every axis's size |
| `nn-interp?` (default 0), last | `nearest=False` | nearest neighbour instead of linear |
| `ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS` (or `NSLOTS`, as ITK reads them) | `n_threads` | the result does not depend on it |

The output size on each axis is `int(size · old spacing / new spacing + add_voxels)`. Output
voxels beyond the input are 0 (ANTs: the input's value at index `(1, 1, …)`, before smoothing,
ANTS-32).

### Examples of `ResampleImageBySpacing`

<!-- inputs: t1w.nii.gz=head mask.nii.gz=head-mask -->

```python example
import larmorx as lx

# The 4 mm head to 8 mm, smoothed first; the mask to 2 mm by nearest neighbour.
low = lx.ants.resample_image_by_spacing("t1w.nii.gz", [8.0, 8.0, 8.0])
low.shape  # -> (20, 24, 20)
labels = lx.ants.resample_image_by_spacing("mask.nii.gz", [2, 2, 2], smooth=False, nearest=True)
labels.shape  # -> (80, 96, 80)
```

```bash example
larmorx ants ResampleImageBySpacing 3 t1w.nii.gz t1w_8mm.nii.gz 8 8 8
larmorx ants ResampleImageBySpacing 3 mask.nii.gz mask_2mm.nii.gz 2 2 2 0 0 1
```

### Ports of `ResampleImageBySpacing`

**The replica: status `validated`** against ResampleImageBySpacing itself (its
[record](../validation/ants-resample-image-by-spacing.md), 2026-10-24): 37 cases, 30 outputs
compared, all bit-identical with ANTs' exact header bytes and printed text, in 2, 3 and 4
dimensions; 5 rejected by both alike. Where ANTs resamples uninitialised memory, larmorx
reproduces the rest (exit code, header, printed text) and fills that memory with zeros
(ANTS-28). Compared with ANTs' defects reproduced (`LARMORX_UPSTREAM_DEFECTS=reproduce`); the
record's "D13 fix pass" lists what larmorx's default changes.

**On the data panel** ([record](../validation/ants-resample-image-by-spacing-panel.md),
2026-10-26): fMRIPrep 25.2.6 never calls `ResampleImageBySpacing`; on the panel's images (the 7 T
MP2RAGE to 2 mm, the 82-year-old's FLAIR to 1 mm, the 3.5-year-old's T1w to 1.5 mm): **3 of 3
bit-identical, header bytes included**.

## Speed

From the [benchmark](../benchmarks/ants-gaussian.md): a raw T1w to 2 mm takes 149 ms on one
thread and 80 ms on 12, against 496 ms and 151 ms for ANTs; the 1 mm MNI template to 3 mm 252 ms
against 629 ms on one thread.

## Behaviour worth knowing

From ANTs ([findings](../findings/ants-gaussian-filters.md#resampleimagebyspacing-examplesresampleimagebyspacingcxx)):
- **the smoothing sigma is in millimetres but computed as a ratio**: 1 → 2 mm smooths with
  one voxel, 2 → 4 mm with half a voxel (corrected by default: one input voxel per unit of
  `out/in − 1`, ANTS-32);
- it prints the input spacing, the new spacing, each axis's sigma (larmorx's default prints
  the sigma the filter receives, in mm) and the output size;
- with smoothing, every spacing must be given (else ANTs crashes);
- 4D linear interpolation runs in four dimensions (time included), as ITK's
  `LinearInterpolateImageFunction` does.

## Upstream defects (D13)

ANTs' deterministic defects are corrected by default and reproduced bit for bit on request:
`upstream_defects="reproduce"` in Python, `LARMORX_UPSTREAM_DEFECTS=reproduce` on the command
line. A warning (on the command line a line on standard error) names the entry wherever it
changed the result ([upstream-defects.md](upstream-defects.md); each entry in the
[register](../findings/upstream-defects.md)).

| ID | ANTs (what `reproduce` gives) | larmorx's default |
|---|---|---|
| ANTS-162 | the output grid (`extent / spacing + addvox` voxels per axis) is sized with no check: an image in metres resampled to millimetre voxels can need more memory than the machine has, and ANTs fills it until it is stopped | larmorx first computes the memory (7.5 bytes per output voxel, the most it was measured to need) and, above `LARMORX_MEMORY_LIMIT` or the machine's physical memory (1 TiB where it cannot be read), stops at once with the GiB needed and the likely cause (exit 1); `reproduce` attempts it as ANTs does |
| ANTS-31 | the `nn` flag is read from the `addvox` argument in 2D (whenever an `nn` argument follows), and in 4D as soon as there are more than 9 arguments, so an `addvox` without `nn` aborts; a dimension other than 2, 3 or 4 does nothing and exits 0 | each value from its own argument; other dimensions are an error (exit 1) |
| ITK-01 | ITK's floor of a continuous index just below 0 is 0, not −1 (x86-64), in 4D linear interpolation | the exact floor; the warning counts the output voxels this changed |
| ANTS-28 | an axis of fewer than 4 voxels that needs smoothing: ITK throws, ANTs prints the exception after "Exception catched !" and resamples the smoothing filter's never-written buffer (uninitialised memory, different on every run), exit 0. `reproduce` prints the same text (ITK's two addresses and its build directory written as `0x0` and `<ITK source>`), fills the buffer with zeros, writes the same file and warns | ITK's message, exit 1 (Python `ValueError`) |
| ANTS-32 | the smoothing sigma `out/in − 1`, a number of input voxels (the downsampling factor less one), is taken in millimetres, so the smoothing depends on the units of the spacing (1 → 2 mm: one voxel; 2 → 4 mm: half a voxel); output voxels beyond the input get the input's value at index `(1, 1, …)` (an axis of one voxel: a read outside the image), though the source says "zero regions without source" | the sigma is `out/in − 1` input voxels (`out − in` mm; printed in mm), the voxels beyond the input 0, an axis of one voxel accepted (decided 2026-10-23, rule A); each part named where it changes the result |
| ANTS-03, ITK-02, ITK-08 | as for every ANTs program: names of fewer than 3 characters, the direction of a file with more dimensions than `d`, `x.nii` read for `x.nii.gz` ([ImageMath](ants-image-math.md#upstream-defects-d13)) | as there |

## Deliberate differences

- `nearest=True` works in 2D in Python.
- Where ANTs crashes, larmorx exits 1 with a message; where it reads outside the image (an
  axis of one voxel for the `(1, 1, …)` default value), `reproduce` exits 1 and the default,
  which needs no such value, resamples.
- Images are NIfTI only.
