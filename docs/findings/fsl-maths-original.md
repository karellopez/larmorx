# Findings: fslmaths' clean-room original (black-box runs of FSL 6.0.7.23)

What black-box runs of FSL 6.0.7.23's `fslmaths` (`~/fsl-6.0.7.23/bin/fslmaths`, through the
oracle-output cache) showed that `specs/fslmaths.md` does not say, or says otherwise, written by
the clean-room implementer of `crates/larmorx-fsl`'s `maths` (no FSL source, no replica code, no
restricted finding was opened: `crates/larmorx-fsl/PROVENANCE.md`). The probes are in
[`validation/investigations/cleanroom-maths/`](../../validation/investigations/cleanroom-maths/README.md).
Each item is a spec question in the part's handoff.

## 1. The thresholds' far side (FSL-152), relay 1

`probe_thresholds.py` (a float image `[1, 2, 2^25, 5, 2^25]`; `[1, +inf, -inf, 3, 0]`; int16,
int8 and int32 images holding their type's extremes):

- `-thr t` keeps `t ≤ v ≤ max + 1` and `-uthr t` keeps `min − 1 ≤ v ≤ t`: the far side is
  **inclusive**. So in `float` from 2^24, where `max + 1` rounds back to `max`, `-thr` keeps the
  maximum (`-thr 0` of the first image keeps both 2^25 voxels), and an infinite maximum is kept.
- `-bin` (and so `-binv` and `-mas`'s mask) is 1 where `0 < v < max + 1`: the far side is
  **exclusive**, so the maximum from 2^24 (and `+inf`) becomes 0. This is where FSL-152's
  "rounds back to the maximum from 2^24 ... the largest voxels are zeroed" applies.
- `max + 1` (`min − 1`) wraps under `short` as it does under `char` and `int`: `-dt short` of an
  int16 image holding 32767 makes `-thr 0`, `-uthr 10` and `-bin` zero every voxel. The spec and
  the register name `char` and `int` only.

The original reproduces all three with `reproduce`; by default the far side bounds nothing
(FSL-152's correction) and the notice names the defect only where the output changes.

## 2. `-save`'s type and display range, relay 1

- `-save` under `-dt char` writes uint8 bytes (the computing value's low 8 bits: `-128` is
  written 128); under `-dt short` int16.
- `-save` writes the **input's** display range (`cal_min`, `cal_max` as read: the replica suite's
  `cli/save-restart` keeps `-3 7` after `-mul 3`), not `(0, 0)` and not `-range`'s: the final
  output alone gets `(0, 0)` once an operation ran, and `-range`'s at the end.

## 3. An image of other sizes (FSL-159), relay 1

On a 3 × 4 × 1 image (values 1 to 12) with a 2 × 4 × 1 and a 4 × 4 × 1 operand:
- `-div` and `-rem` take the operand's value at the same voxel indices, 0 outside it (`-div`'s
  result 0 there; `-rem` leaves the voxel), as the spec says;
- `-max` and `-min` with the larger operand also take the value at the same indices (its
  storage read with its own strides at the current image's indices); the original refuses, in
  both modes, where those indices fall past a smaller operand's storage (FSL reads memory it
  never wrote there).

## 4. The messages of an image that cannot be read, relay 1

- A missing **input**: `Image Exception : #63 :: No image files match: <name>` only, then the
  C++ runtime's lines with `what():  No image files match: <name>` (no `#22` line and no
  `Error :` line, unlike BET's and an operand's).
- A missing **operand** (`-add out`): the image layer's three lines (`#63`, `#22 :: Failed to
  read volume <name>`, `Error : No image files match: <name>`), then the runtime's lines with
  `what():  Failed to read volume <name>` and the `Error :` line.
- A file cut short (gzip or raw) or a header claiming more data than the file holds: `#22 ::
  Failed to read volume <name>` and `Error : Error: short read, file may be truncated`, then the
  runtime's lines.
