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
- **Reruns** (§4.5; **resolved 2026-10-30, part 2a**): the spec's rule was right; the reruns
  amplify last-bit differences of the deformation's arithmetic, which the spec writer then gave
  operation by operation (§8.1). With §8.1 followed in full (the mesh held in the file's triangle
  order, the neighbour lists by §4.4's move-to-the-end rule, the normal and the neighbour mean by
  three divisions, the initial placement `c + (r/2)((p + c) − c)`, `1/R` from `s_n·n`, the update
  `p + ((u1 + u2) + u3)`, the inward depth accumulated, the field of view in float, `rmax`'s
  product in float, the centre of gravity one running sum, the radius's float products, the
  robust range's bin width first) and glibc's `tanh` (below), the standard tier's three rerun
  cases (`real/older-75y`, `real/7t`, `real/tumour`) are FSL's **bit for bit**: their brains,
  masks, outlines, meshes and verbose streams (the rerun totals printed included).
- **The exact test** (`-e`; **resolved 2026-10-30, part 2a**): §4.5's sweep and predicate as the
  spec writer gave them; the tumour's `-e` run (one rerun) is FSL's bit for bit, mesh included.
- **glibc's `tanh`** (§8.1, part 2a; `validation/investigations/cleanroom-bet/tanh-verify/`):
  fdlibm 5.3's `tanh` over its `expm1`, as netlib distributes them, differs from the system's
  (glibc 2.35) in the last bit or two on about 0.03 % of arguments (1,363 of 4.2 million random
  ones), all where `expm1`'s rational approximation is used; with that polynomial evaluated in
  Estrin's order, `(1 + Q1 z) + z²(Q2 + Q3 z)` then `+ z⁴(Q4 + Q5 z)` left to right (the pipelined
  revision the spec names), **every one of 12,918,456,348 inputs** agrees bit for bit (3 x 2^32
  random ones of three families and 33.5 million around each branch threshold).
  `larmorx_core::math::tanh_glibc`. Horner's order and two other Estrin groupings were tried and
  differ (6,640 to 9,698 of 36.7 million). glibc's `tanh` is not correctly rounded on 21.3 % of
  that random sample (CORE-MATH's `cr_tanh` as the reference) and on 22,506 of CORE-MATH's
  62,880 worst cases: GLIBC-06.
- **What the correctly rounded `tanh` changes** (GLIBC-06, rule D; `crates/larmorx-fsl/examples/tanh_effect.rs`,
  bet2's defaults, the standard tier's 13 real images): on 9 the surface is the same **to the
  last bit** with either `tanh` (the last-bit differences vanish in `1 + tanh`); on 4 it is not:
  the three rerun cases (ds002422's 75-year-old: Dice 0.995919, 13,442 voxels, the second rerun's
  total 2198.2 against glibc's 2817.8; ds005454's 7 T: Dice 0.984779, 50,100 voxels; ds005003's
  tumour: Dice 0.991957, 24,874 voxels; vertices up to 5.7 to 8.0 mm apart) and the first volume
  of ds000005's BOLD run 1 (no rerun: the first pass's total 87.2 against 76.8, Dice 0.996618, 226
  voxels). The correctly rounded result is not more accurate there (the reruns amplify any last
  bit); it moves the default away from FSL's. A cheap exact test of GLIBC-06 follows: the weight
  `f2` computed with both `tanh` at every step; where no `f2` differs, the surfaces are identical
  (the rest of the arithmetic is the same), and only otherwise does a second search with glibc's
  `tanh` say what changes.

- **The robust range's rounding** (`range_probe.py`; fMRIPrep's calls): `t2 = lo + b (hi − lo) /
  1000` with `hi − lo` rounded to float, the product and the division in double, the result
  rounded to float and added to `lo` in float. With `hi − lo` in double, ds005454's BOLD
  reference gives `thresh2 -0.142002` where FSL prints `-0.142004` (and another surface); with
  the product in float, ds000005's run 3 a `thresh98` one float bit off FSL's (and another
  mask). The spec writer's exact arithmetic (the bin width `w/1000` first, the narrowed range's
  own formula, the end bins' counts left out of the tenth pass) settles the rest: random-testing
  case 74's `thresh` digit agrees since part 2a.
- **A name with an extension** (§3.1; part 2a): `bet2 in.nii.gz` with only `in.nii` present reads
  `in.nii`, as the spec writer observed; larmorx resolves names so (`bet::image::resolve_input`).
- **`bet -F` on a float64 series** (§5.2; part 2a, case 425): the brain keeps the series' double
  values (`bet::image::read_f64_series`).
- **Small and degenerate random heads** (random testing 2026-10-30, 75 cases of 573 after part
  1's corrections): with §8.1, §4.7 and §8.2 followed (part 2a), **73 of the 75 agree** with FSL
  (and 237 of the trial's 239 differing cases). The two left:
  - case 677 (`bet2 -c 10 6 15 -w 5 -s -v` on a neurologically stored 2 x 4 x 2 mm head): the
    first pass's self-intersection total differs (258194 in FSL, 259595 here), and the surface
    and skull with it: `-c` on a neurologically stored image, which FSL converts through a 4 x 4
    inverse (§8.1 leaves its last bits open; the spec writer named this case for it);
  - case 622 (`bet -R -f 0.2 -m -t`): every file and standard output FSL's, but FSL prints
    `dc: stack empty` twice: FSL's `fslstats -C` (run by hand on FSL's brain, a black box) gives
    `-0.000000 8.507173 16.273980` for a centre whose x is 0 in the file's voxels (the image is
    stored neurologically: the conversion's last bits again), and `dc` reads the leading `-` as
    its subtraction command, with too few numbers on its stack. larmorx's centre is `0.000000`.
    Both wait for the conversion's exact arithmetic (a question for the spec writer).

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
- **The skull** (**resolved 2026-10-30, part 2a**): §4.7's search as the spec writer gave it
  (outside voxels counted as surface voxels, unclipped candidates, three scans on grids of their
  own, §8.1's trilinear formula): `_skull` is FSL's bit for bit on the real T1w images compared by
  hand (ds005454's 7 T, ds000005) and in every case of the record; the suite now holds it to the
  masks' thresholds instead of reporting its Dice.

## Images (§3)

- **The qform's quaternion** (`quatern_probe.py`; **resolved 2026-10-30, part 2a**): FSL writes
  the quaternion turned into its matrix and back by the NIfTI standard's conversions; for a half
  turn (`a = 0`, frequent in real qforms) it picks the other sign of `(b, c, d)`. With §8.2's
  precision path (x87 extended precision in the steps it names, `crates/larmorx-fsl/src/bet/x87.rs`;
  NIfTI-2's doubles unrounded) every header of the record and of the random trial is FSL's byte
  for byte; the suite's quaternion tolerance is now 0.
- **`bet -Z`'s quaternion** (part 2a; random testing 2026-10-30, cases 132, 135, 223, 378, 583):
  the round trip of §8.2 is not idempotent in the last bits, and FSL applies it once per program
  of the script's chain: `fslroi` writes the bottom slice, `fslmerge -z` writes the padded image
  with the bottom slice's header (one more round trip), bet2 another, the crop another. larmorx's
  `-Z` takes the padded image's quaternion from the bottom slice it wrote and the crop's from the
  padded output: those five cases are FSL's byte for byte.
- **The script's steps and `FSLOUTPUTTYPE`** (part 2a; case 617): with an unknown output type the
  script's first step (`fslroi` for `-Z`, `-F`) prints the image layer's two `ERROR::` lines and
  the script fails; larmorx now prints them too (it printed its own line).
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
