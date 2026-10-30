# BET's clean-room original: black-box observations (2026-10-30)

What the clean-room implementer of BET (`crates/larmorx-fsl`, written from
[`specs/bet.md`](../../specs/bet.md)) learnt from **black-box runs** of FSL 6.0.7.23's `bet` and
`bet2` (`~/fsl-6.0.7.23/bin`, run by full path, FSL off the `PATH`), beyond what the spec says.
No FSL source, replica crate or source-derived finding was opened (the clean-room statement is in
[`crates/larmorx-fsl/PROVENANCE.md`](../../crates/larmorx-fsl/PROVENANCE.md)). The probes are in
[`validation/investigations/cleanroom-bet/`](../../validation/investigations/cleanroom-bet/README.md);
each item below names the one that shows it. They are written here for the spec writer to move
into the spec, marked (observed).

## The surface (§4.4)

- **The mesh's numbering** (`topo.py`, `topo2.py` on `bet2 -e`'s file): the icosahedron's twelve
  vertices in the order `(τ, o, 0)`, `(−τ, o, 0)`, `(−τ, −o, 0)`, `(τ, −o, 0)`, `(o, 0, τ)`,
  `(o, 0, −τ)`, `(−o, 0, −τ)`, `(−o, 0, τ)`, `(0, τ, o)`, `(0, −τ, o)`, `(0, −τ, −o)`,
  `(0, τ, −o)`; its twenty faces `(7,8,4) (9,7,4) (11,6,5) (6,10,5) (3,4,0) (5,3,0) (1,7,2)
  (6,1,2) (11,0,8) (1,11,8) (3,10,9) (10,2,9) (0,4,8) (5,0,11) (3,9,4) (10,3,5) (1,8,7) (11,1,6)
  (9,2,7) (2,10,6)`; each subdivision, for every triangle `(p0, p1, p2)` in order, makes the
  midpoints `m01`, `m12`, `m20` not made yet (in that order, numbered after the existing
  vertices) and the four triangles `(m01, m12, m20)`, `(p2, m20, m12)`, `(p1, m12, m01)`,
  `(p0, m01, m20)`; a triangle `(a, b, c)` is written to the mesh file as `c a b`; its outward
  normal is `(c − a) × (b − a)`. With these, every vertex of `bet2 -e`'s file is reproduced to
  its 6 decimals on the phantoms and on real T1w images (record: [validation/fsl-bet.md](../validation/fsl-bet.md)).
- **The median** `tm` (`py/params.py`, on images of distinct float values): the value of rank
  `floor(N/2)` (1-based) of the sorted list, as the spec says; with one voxel, that voxel.
- **Reruns** (§4.5; open): where the first pass self-intersects (a total above 4000), FSL's rerun
  does not follow the spec's rule as written. On ds002422 sub-02's T1w the first pass's total is
  FSL's (90050), the first rerun's is 19809.7 in FSL and 16099 by the rule; on ds005003's tumour
  T1w with `-e` (one rerun), the rerun's mesh is 0.34 mm from FSL's (median; 95th percentile
  2.5 mm). Tried, none reproducing FSL: the factor `10^P`; the condition `s·n < 0`; all vertices;
  no lowering; lowering over the whole pass, from `i > 500`, with `(i + 1)/1000`, as a power;
  `f2` uncapped; the factor on `1/R` or on `rmin`, `rmax`; `l` every iteration, fixed, or the
  previous pass's; the rerun from the previous pass's surface (`variants.py`,
  `variants2.py`, with a temporary build). A scan of the first rerun's factor on the tumour
  (2, 5, 20, 50, 100, 200, 1000) has its sharp minimum at the spec's 100 (0.93 mm at 50, 0.83 mm
  at 200): the factor is right, the remaining difference lies elsewhere in the rerun (FSL's
  rerun surface is a little smoother than the rule's on its convex parts too: mean `|s·n|`
  1.382 against 1.404 mm). A question for the spec writer; the five standard-tier cases that
  rerun are outside the record's thresholds.
- **The exact test** (`-e`; open): on small random heads FSL finds an intersection where a
  segment-triangle test between triangles that share no vertex finds none (random testing
  2026-10-30, case 27: a 14 x 14 x 12 head of 2.5 x 2.5 x 1.5 mm voxels); coplanar or touching
  triangles may count for FSL. A question for the spec writer.

## The mask and the outline (§4.6, §4.7)

- **The rasterisation** (`raster_probe.py` to `raster_probe4.py`): for a triangle `(a, b, c)`
  (written `c a b`), the points from `a` towards `c` at `0, s, 2s, …` up to the edge's length
  (`s = min(d)/2`, accumulated), and from `b` towards each of them at the same spacing; each
  point to the voxel `floor(p/d + 0.5)` (mask) or `trunc(p/d + 0.5)` (outline). Exactly FSL's
  voxels: the masks, brains and outlines are bit-identical wherever the surface is.
- **The fill's start** (random testing 2026-10-30, cases 4 and 35): the start voxel is filled
  whatever it holds, and the fill goes on from it: a surface collapsed to a sheet through the
  start voxel lets the fill reach the whole image (FSL's mask is then every voxel). The mask is
  made whatever outputs are asked for: with `-n` alone FSL still crashes at a centroid that is
  not a number (FSL-43).
- **Small images** (random testing 2026-10-30, case 36): with fewer than 50 voxels (`n/50 = 0`)
  no bin is added: `t2` lies one bin below the minimum and `t98` one above the maximum
  (a 2-voxel image of 50 and 51: `thresh2 49.999`, `thresh98 51.001`); with no voxel above the
  threshold the centre of gravity is `0/0` (`c-of-g -nan -nan -nan mm`) and the
  self-intersection total 0.
- **The skull** (`skull_probe.py`; open): from each surface voxel of a vertex (§6.4's choice), the
  profile along the vertex's normal from the voxel's own position, sampled every `min(d)/2` from
  −3 mm to 30 mm; `d_max` the largest sample at or beyond 0; the point maximising
  `s/30 − I(s)/(t98 − t2)` before it; the skull voxel at the sample the steepest rise between them
  reaches, `trunc(p/d)`. Dice 0.91 with FSL's `_skull` on the phantom (16249 voxels against FSL's
  16836, 1771 and 1184 differing): the details the spec leaves open (§4.7) decide the rest.

## Images (§3)

- **The qform's quaternion** (`quatern_probe.py`): FSL writes the quaternion turned into its
  matrix and back by the NIfTI standard's conversions; for a half turn (`a = 0`, frequent in real
  qforms) it picks the other sign of `(b, c, d)` (the oblique phantom). With the matrix rounded to
  float, 3 of 5 real inputs give FSL's bits; the others differ in the last float bit of one
  component (x87 extended precision, §8, would decide it): the record allows one unit in the last
  place there.
- **ANALYZE 7.5 inputs**: FSL works on them with x reversed (as on a neurologically stored image)
  and writes the outputs with a qform and sform of code 2, the voxel sizes on the diagonal and the
  offset `−(origin − 1) d` (the header's `originator`; 0 gives `+d`): the outputs line up with the
  input voxel for voxel.
- **A single slice** is written with `dim[0] = 2` (a trailing axis of one voxel not counted); so
  are `bet -Z`'s one-slice temporary images.
- **FSL-56's layout**: FSL writes the extensions in full and the data right after them; only the
  recorded `vox_offset` is its wrong one (`py/ext.py` in the scratch notes; the record's
  `defects/extensions`).
- **FSL-62**: with `--ct --binaryMask`, FSL's brain image is the mask as read (its 0 and 1),
  masked.

## Messages (§4.1, §5.1, §7)

- **The option parser** (`py/batch2.py` to `batch5.py`): an option of `k` values given fewer than
  `k` words takes the words left, each as `atof` (0 when it does not read), the others 0, and
  never fails (`-c 4` is `-c 4 0 0`; `-c 1 -v` swallows `-v`); with `k` words or more, the first
  `k − 1` are read as `atof` and the last must be a whole number (`strtod`; leading blanks,
  hexadecimal, `inf` and `nan` read); a failure's `valstr` is the option's `k` words and the
  next one, if any. A long flag given a value (`--mask=1`) is the flag.
- **A missing input**: `Image Exception : #63 :: No image files match: <name without its
  extension>`, `Image Exception : #22 :: Failed to read volume <name as given>`,
  `Error : No image files match: <name without its extension>`.
- **The `bet` script**: `-v`'s `variation=` numbers are 0 (default), 1 `-R`, 2 `-S`, 3 `-Z`, 4
  `-F`, 5 `-A`, 6 `-A2`, 7 `-B`; `bet2opts=` lists the options in the order given, each after a
  blank (`-d` not among them); `-h` prints the usage and exits 1; a failing step prints `<script
  path> failed during command:<the arguments joined by blanks>` (no blank after the colon).
- **FSL-44**: the script splits names with blanks only where it runs bet2 (its `$IN` and `$OUT`
  unquoted): `bet in "my out" -m` runs `bet2 in my out -m`, which reports `out:  is an
  unrecognised token!`, and the script fails; its own option loop sees the arguments whole and
  stops at an empty one.
- **`-F`'s files**: the first volume as `<out>_tmp`, bet2's mask as `<out>_tmp_brain_mask`; the
  dilated mask `<out>_mask` float32 and the 4D brain float32, both with the display range 0 and 0
  (`fslmaths`). **`-Z`'s**: `<out>_tmp` (padded), `<out>_tmp_bottom`, `<out>_tmp_top` (one slice,
  its origin moved to the top slice).
