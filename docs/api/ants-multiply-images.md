# MultiplyImages

`lx.ants.multiply_images` (command line `larmorx ants MultiplyImages`; Rust
`larmorx_ants::multiply_images`). A replica of ANTs 2.6.5 (Apache-2.0).

**Status: `validated`** against MultiplyImages itself (see the
[validation record](../validation/ants-multiply-images.md)): 15 cases, every compared case
bit-identical, with ANTs' exact header bytes.

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
| `d img1 img2 out` | `multiply_images(img1, img2)` | float32 product at `img1`'s voxel indices; `img2`'s geometry is ignored (it may be larger, not smaller) |
| `d img1 number out` | `multiply_images(img1, number)` | |
| `d` = 2, 3, 4 | any array shape | ANTs also accepts 1 |
| vector and tensor images | – | not supported yet (ANTs multiplies component by component) |

## Behaviour worth knowing

- On the command line, **a second argument that cannot be read as an image is a number**
  (`atof`): a missing or misspelt file multiplies by 0, silently, as in ANTs.
  `lx.ants.multiply_images` raises `FileNotFoundError` for a path that does not exist.
- Without an output name ANTs prints "missing output filename" and aborts; larmorx exits 1.
- Details: [findings](../findings/ants-image-programs.md#multiplyimages).
