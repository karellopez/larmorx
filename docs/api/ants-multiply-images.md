# MultiplyImages

`lx.ants.multiply_images` (command line `larmorx ants MultiplyImages`): an image multiplied by
another image or by a number, as ANTs 2.6.5's `MultiplyImages` computes it.

The port comes first, then the program's section, the source of its reference page on the
documentation site (`python -m larmorx_validation reference`), with its options table checked
against the code; then the speed, the behaviour worth knowing and ANTs' defects as the port
corrects them.

## Ports

`MultiplyImages` has one port, in `pip install larmorx`: **the replica** (`crates/larmorx-ants`;
Apache-2.0; Rust `larmorx_ants::multiply_images`), translated from ANTs 2.6.5 (Apache-2.0).

## `MultiplyImages`

<!-- reference
program: MultiplyImages
group: ants
original: ANTs 2.6.5
python: larmorx.ants.multiply_images
command-line: crates/larmorx-ants/src/cli/multiply_images.rs
records: {replica: [ants-multiply-images]}
evidence: ants-programs
-->

`MultiplyImages` multiplies an image by another image or by a number. sMRIPrep's brain
extraction runs it three times after Atropos (`08_mult_gm`, `09_relabel_wm` and
`12_relabel_gm`).

### Python (`MultiplyImages`)

`lx.ants.multiply_images(image, other, *, n_threads=1, upstream_defects="fix")`

Returns the float32 product at `image`'s voxel indices, as an `lx.Image`; `other` is an image or
a number. A path that does not exist raises `FileNotFoundError`.

```python
import larmorx as lx

brain = lx.ants.multiply_images("t1w.nii.gz", "brain_mask.nii.gz")
half = lx.ants.multiply_images(gm_probseg, 0.5)
```

### Options of `MultiplyImages`

| ANTs argument (positional) | Python | Notes |
|---|---|---|
| 1 `ImageDimension` | – | 2, 3, 4 (ANTs also accepts 1); Python takes any array shape |
| 2 `img1` | `image` | the product is at its voxel indices |
| 3 `img2` or a number | `other` | an image (its geometry is ignored: it may be larger; smaller is ANTS-07 in [Upstream defects](#upstream-defects-d13)) or a number; vector and tensor images are not supported yet (ANTs multiplies component by component) |
| 4 `product` | the result | without an output name ANTs prints "missing output filename" and aborts; larmorx exits 1 |
| `ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS` | `n_threads` | the result does not depend on it |
| `LARMORX_UPSTREAM_DEFECTS` | `upstream_defects` | `fix` (default) or `reproduce` ([Upstream defects](#upstream-defects-d13)) |

### Examples of `MultiplyImages`

<!-- inputs: t1w.nii.gz=head mask.nii.gz=head-mask -->

```python example
import larmorx as lx

# The brain of the head: the image times its mask.
brain = lx.ants.multiply_images("t1w.nii.gz", "mask.nii.gz")
brain.data.dtype  # -> dtype('float32')
```

```bash example
larmorx ants MultiplyImages 3 t1w.nii.gz mask.nii.gz brain.nii.gz
larmorx ants MultiplyImages 3 t1w.nii.gz 0.5 half.nii.gz
```

### Ports of `MultiplyImages`

**The replica: status `validated`** against MultiplyImages itself (its
[record](../validation/ants-multiply-images.md), 2026-10-12): 15 cases, 12 compared, 3 rejected
by both alike; every compared case bit-identical, with ANTs' exact header bytes. Compared with
ANTs' defects reproduced (`LARMORX_UPSTREAM_DEFECTS=reproduce`); the record's "D13 fix pass" lists
what larmorx's default changes.

**On the data panel** ([record](../validation/ants-multiply-images-panel.md), 2026-10-26): all
42 `MultiplyImages` nodes of the fourteen finished oracle runs (the three of sMRIPrep's brain
extraction after Atropos, `08_mult_gm`, `09_relabel_wm` and `12_relabel_gm`, in every run), each
replayed from its node with its inputs, and two calls fMRIPrep never makes (the stroke's T2w times
its lesion mask, a 588-volume multiband series times a scalar): **44 of 44 bit-identical, header
bytes included**.

## Speed

From the [benchmark](../benchmarks/ants-programs.md): T1w × brain mask 128 ms on one thread, 62
ms on 12 (ANTs: 157 ms either way).

## Behaviour worth knowing

- `lx.ants.multiply_images` raises `FileNotFoundError` for a path that does not exist.
- Without an output name ANTs prints "missing output filename" and aborts; larmorx exits 1.
- Details: [findings](../findings/ants-image-programs.md#multiplyimages).

## Upstream defects (D13)

ANTs' deterministic defects are corrected by default and reproduced bit for bit on request:
`upstream_defects="reproduce"` in Python, `LARMORX_UPSTREAM_DEFECTS=reproduce` on the command
line. A warning (on the command line a line on standard error) names the entry wherever it
changed the result ([upstream-defects.md](upstream-defects.md); each entry in the
[register](../findings/upstream-defects.md)).

| ID | ANTs (what `reproduce` gives) | larmorx's default |
|---|---|---|
| ANTS-27 | a second argument that cannot be read as an image is a number (`atof`): a missing or misspelt file multiplies by 0, silently | an error (exit 1) unless the argument is a number |
| ANTS-07 | a second image smaller than the first is read past its voxels (the reads beyond its memory are undefined: `reproduce` takes them as 0 and warns) | an error |
| ANTS-03, ITK-02, ITK-08 | as for every ANTs program ([ImageMath](ants-image-math.md#upstream-defects-d13)) | as there |
