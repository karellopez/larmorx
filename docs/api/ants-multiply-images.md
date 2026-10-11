# MultiplyImages

`lx.ants.multiply_images` (command line `larmorx ants MultiplyImages`; Rust
`larmorx_ants::multiply_images`). A replica of ANTs 2.6.5 (Apache-2.0).

**Status: `validated`** against MultiplyImages itself (see the
[validation record](../validation/ants-multiply-images.md)): 15 cases, every compared case
bit-identical, with ANTs' exact header bytes.

**Speed** ([benchmark](../benchmarks/ants-programs.md)): T1w × brain mask 128 ms on one thread,
62 ms on 12 (ANTs: 157 ms either way).

## Quick start

```python
import larmorx as lx

brain = lx.ants.multiply_images("t1w.nii.gz", "brain_mask.nii.gz")
half = lx.ants.multiply_images(gm_probseg, 0.5)
```

```bash
larmorx ants MultiplyImages 3 t1w.nii.gz brain_mask.nii.gz brain.nii.gz
larmorx ants MultiplyImages 3 gm.nii.gz 0.5 gm_half.nii.gz
```

## Option mapping

| MultiplyImages | Python | Notes |
|---|---|---|
| `d img1 img2 out` | `multiply_images(img1, img2)` | float32 product at `img1`'s voxel indices; `img2`'s geometry is ignored (it may be larger; smaller is ANTS-07 below) |
| `d img1 number out` | `multiply_images(img1, number)` | |
| `d` = 2, 3, 4 | any array shape | ANTs also accepts 1 |
| vector and tensor images | – | not supported yet (ANTs multiplies component by component) |

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
