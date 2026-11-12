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
  bet2's defaults, the standard tier's 13 real images): on 9 the two surfaces agree within a
  micrometre and every output is the same (most last-bit differences vanish in `1 + tanh`; on
  ds000005's T1w the vertices still differ in their last bits, and no output with them); on 4 not:
  the three rerun cases (ds002422's 75-year-old: Dice 0.995919, 13,442 voxels, the second rerun's
  total 2198.2 against glibc's 2817.8; ds005454's 7 T: Dice 0.984779, 50,100 voxels; ds005003's
  tumour: Dice 0.991957, 24,874 voxels; vertices up to 5.7 to 8.0 mm apart) and the first volume
  of ds000005's BOLD run 1 (no rerun: the first pass's total 87.2 against 76.8, Dice 0.996618, 226
  voxels). The correctly rounded result is not more accurate there (the reruns amplify any last
  bit); it moves the default away from FSL's. GLIBC-06's exact test (in the branch, not the
  default yet): the weight `f2` computed with both `tanh` at every step; where no `f2` differs,
  the surfaces are identical bit for bit (the rest of the arithmetic is the same); otherwise a
  second search with glibc's `tanh` and every output asked for compared. Some weight's last bit
  differs in practically every run, so the second search always runs: the default 1.9 times
  slower (ds000005's T1w 0.43 → 0.83 s, ds002422's 1.20 → 2.33 s on one thread). bet2 keeps
  glibc's `tanh` in both modes until part 2b's decision.
  **The user's decision (2026-10-31, GLIBC-06 option c):** glibc's bits stay the default; the
  correctly rounded `tanh` is larmorx's own option `--tanh=correctly-rounded` (bet2 and bet;
  `lx.fsl.bet(tanh="correctly-rounded")`). Asked for, it needs no notice: the exact test and
  its second search were removed (part 2b), so the option costs nothing beyond its own run.

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

## betsurf (§6), part 2b (2026-10-31)

Probes: `betsurf_cases.py` (FSL against the original on the suite's phantoms and real images),
`betsurf_inputs.py` (an input stored the other way along x, or with a larger x voxel),
`betsurf_triangles.py` (one triangle of bet2's mesh at a time: a few profiles) and
`betsurf_profile_oracle.py` (**one profile at a time**: a triangle 0.2 mm across drawn into the
profile's voxel, nearer one vertex, so that betsurf casts exactly one profile; FSL's `-v` says
whether it set it aside). The part's probe build (commit `f1f9a35`) also wrote each profile's
samples (`LX_BETSURF_DUMP`), from which the readings below were tried; the product has no such
switch.

- **The removed profiles** (`-v`): the percentage counts the dropped profiles **and those of
  which only the scalp counts** (exact on every image where the profile logic agrees).
- **The region's end**: a region that "ends at S" holds the sample at S (`[L, S]`): the
  thresholds take it into their minimum and maximum, the forward searches reach it.
- **Step 5 (T1 and T2)**: "more than one gap" counts the **bright** samples found (a bright
  sample with no dark one after it counts); the outer skull search then starts at the first
  dark one.
- **Step 6**: when the second search finds nothing, OS2 is **the region's start** (the outer
  skull search's start), not the first search's result.
- **Step 7**: the scan **includes `a`** (the spec's "not the first" is not what FSL does).
- **T1 only**: a profile whose OS2 lies inside its IS (`OS2 < IS`) is not kept (only its scalp
  counts); `OS < IS` gives the same decisions on every profile probed (a question for the spec
  writer).
- **FSL-51**: on profiles whose searches start past their last sample, FSL's decisions are those
  of reading "nothing there" (14 of 14 overrunning profiles of the tumour, `reproduce`).
- With these, **330 of 330 single profiles agree** (214 T1-and-T2 ordinary ones of ds002603, 31
  near the eyes and 14 overrunning ones of ds005003, 89 T1-only ones of ds000256), and 64 of 64
  triangles of ds000256 (T1 only) and 127 of 128 of ds002603 and ds005003 (T1 and T2). On whole
  images the removed profiles differ by at most 0.06 % (+3 of 66,074 on ds000005, +37 of 110,707
  on ds005003, −1 of 110,938 on ds002603 with the T1 only; −16 of 152,419, +68, −40 with the T2,
  `reproduce`).
- **`-p k`**: k times a copy retessellated and 100 more steps (inner and outer skull: speed 0.5,
  no mask; the scalp: its speed 1.5, no mask, the closest of four readings); the written meshes
  are the copies (10242 vertices for `-p 1`); the inner and outer skull are FSL's to the file's
  6 decimals on the phantom with `-p 1`.
- **The command line**: `-p` reads an int as `strtol` of base 0 on the whole word (`0x10`, `-3`
  read; `08`, `1.5`, `1e3` do not); a lone `-` is an empty run of options; the mesh name `c`
  prints `cancelled` and leaves the mesh empty (FSL then crashes: refused, FSL-52);
  `manual_input` prompts without a newline and reads a whole line, and waits forever at the end
  of its input (larmorx refuses there).
- **Outputs**: each surface's VTK file is written as soon as its phase ends, then its outline and
  its mask (`  outline`, `  mask`), the skull mask after the outer skull; an image write without
  `FSLOUTPUTTYPE` stops the run there (the VTK files before it written); the images keep the
  T1's display range, intent and `aux_file`.
- **The memory**: on the memory test's call (160 × 192 × 160) FSL needs 175 MiB, the original
  144 MiB (the T1 clamped in place, the images freed after the profiles, the smoothing in two
  buffers, the working grid's masks held as floats).

**What still differs** (the record's `real/*-surf*` cases): the decisions agree, and where the
counting images are dense the surfaces are FSL's (the phantom's inner and outer skull to 6
decimals; a T1w whose every profile passes near the eyes: the inner skull exactly), but

- where few profiles vote (a skull base where nearly every profile is set aside, ds002603 with its
  T2) the inner skull moves differently: up to 11 mm on a few vertices, though the decisions
  there agree profile by profile; the deformation's arithmetic in that regime is the first thing
  to ask the spec writer about (the mark of a vertex as outside, the step where no gradient
  shows);
- **the scalp** differs everywhere a little (the phantom: 95 % of vertices within 0.0002 mm, at
  most 0.13 mm; real heads: 95 % within 0.2 to 3 mm, at most about 12 mm at the face); not the
  sphere's placement, its radius, the mask's barriers, the step counts or the marks (each tried);
- so the masks are within betsurf's thresholds (Dice >= 0.99, boundary p95 <= 1 voxel) on 4 of the
  5 real images, and ds002603's scalp mask with its T2 is not (boundary p95 2 voxels, Dice
  0.9938).

**Random testing, part 2b** (a 30-minute trial on 2026-10-31, seed 20261031, 16 cases at a
time, `oracles/difftest/2026-10-31-cleanroom-betsurf`, from commit `21762e8`; 1,846 cases, both
implementations): the replica agreed in every case it finished (1,843; 3 timeouts). The
original:

- **bet2**: 957 of 961 agree (2 time out, 2 differ); **bet**: 513 of 532 (19 differ). 14 of the
  19 are `bet -R`, and nearly every differing case (20 of 21) is an image stored with a positive
  determinant (neurologically). Case 1457 (`bet -f 0.7 -c 2 13 16` on a 29 x 14 x 24 head of
  0.8 x 1 x 2.5 mm, neurologically stored): the meshes differ by 0.11 mm (95th percentile), the
  mask in 5 voxels; the same image stored the other way along x (its world positions
  unchanged, `-c 26 13 16`) gives FSL's mask and mesh exactly. So these are the class of cases
  622 and 677 (the conversion of a centre between the voxel frames of a neurologically stored
  image, the open spec question), now seen with `-c` and with `bet -R` on many more images.
- **betsurf**: 221 of 353 agree; 6 more are FSL-51's (profile searches past their samples:
  FSL's result is undefined there). Of the 126 that differ, 102 differ in masks only (mostly the
  scalp; the trial's images are small synthetic heads and crops of 11 to 32 voxels per side,
  some with voxels of 0.5 by 3 mm, cast from synthetic sphere meshes), 12 also in the share of
  profiles set aside (`removed profiles`, `-v`), 10 in the number of mesh vertices with `-p`, and
  2 in the exit status (below). The share set aside shows that on these images the profiles'
  decisions differ, unlike on the real images: on case 36 (`--t1only`, 24 x 29 x 27 voxels of
  3 x 1.5 x 0.5 mm), FSL sets aside 96.8 % of 378 profiles, the original 87.8 %. Reading the
  samples beyond the image differently (0 only inside `[0, n − 1]`, the nearest edge value, or
  the profile ended at the image's edge) gives 32.8 %, 51.3 % and 25.9 %: none of them FSL's.
  What FSL's profiles read on such images is a question for the spec writer.
- **FSL-50**: on cases 216 and 635 the original's `reproduce` stops (status 255, FSL-50: a
  profile whose outer-skull search range is empty) where FSL runs to the end; the default, which
  sets such a profile aside, prints FSL's share of profiles set aside (79.0323 % on case 635),
  so FSL sets that profile aside too. When FSL stops (the register's FSL-50), and when it only
  sets the profile aside, is a question for the spec writer.

- **Two corrections from the trial** (2026-11-01), each checked by running FSL as a black box:
  `bet2 -c 0 0 0` is FSL's "no centre given": it prints and uses the centre of gravity, as without
  `-c` (the same mask; `-c 0 0 1` is used as given), so the original takes a centre of three zeros
  as not given (case 1608: `bet -R` on an image of negative values, whose brain has no voxel above
  0.001, so the script passes `-c 0 0 0`; and case 1210, where FSL's second bet2 then crashes and
  the script reports it, as the original now does); and `bet -F` with `-d` keeps the first volume
  `<out>_tmp` of a float64 series as float64 with its double values, as `fslroi` writes it (case
  1053; the original wrote float32). `retrial.py` with the corrected build
  (`oracles/logs/retrial-cleanroom-bet-2b-2026-11-01.log`): 1053, 1210, 1608 and 1740 now agree;
  the other bet and bet2 cases are the neurological-storage class above (case 965's second pass
  starts from FSL's centre `-0.000001 0 0`, printed by its centre-of-gravity computation, where the
  original's sum gives exactly 0: the open question of case 622).

The probes: `night_case.py`, `compare.py` and `retrial.py`
([validation/investigations/cleanroom-bet](../../validation/investigations/cleanroom-bet/README.md)), and for the
reading beyond the image a temporary build (not kept: three variants of the sample at a point
outside the image, chosen by an environment variable).

## standard_space_roi and fslroi (the clean-room catch-up, relay 5, 2026-11-05)

The clean-room originals of FLIRT's `standard_space_roi` script and of `fslroi`
(`larmorx_fsl::cli::ssroi`, `::fslroi`), written from
[`specs/standard_space_roi.md`](../../specs/standard_space_roi.md) and `specs/bet.md` §5.4 (FSL's
standard images, `larmorx_fsl::data`), and these black-box runs of FSL 6.0.7.23 through the
oracle-output cache (`ssroi_fsl_runs.py`, `ssroi_stage_probe.py`, `fit_translate.py` in
[`validation/investigations/cleanroom-bet/`](../../validation/investigations/cleanroom-bet/README.md)).
The BET suite's 57 cases of the two programs (`ssroi/`, `fslroi/` and the six `defects/` cases of
FSL-211 to FSL-214) all pass or are refused by both: every `standard_space_roi` case is FSL's
**bit for bit**, the six real images included (an adult, a 3.5-year-old with `-b -d`, an
82-year-old with `-maskFOV -roiNONE -d`, a tumour with `-roiMASK`, a Philips T1w with `-b
-maskMASK`, a T2w): the original's FLIRT gives FSL's matrices on every one, and every later file
follows.

- **fslroi's matrices** (§3.2; `fit_translate.py`): FSL computes the box's new sform and qform in
  the frame it holds the image in. For an image held mirrored (stored neurologically) the matrix
  of that frame, `M·F(nx)`, has its translation `M₀·(nx − 1) + M₃` in single precision (the
  product and the sum each rounded); the output's translation is that matrix applied, in double,
  to the held voxel of the box's first file voxel, `(nx − 1 − x₀, y₀, z₀)`, then stored in
  single precision. The direct `t + M·corner` in double is one unit in the last place away on
  ds005003's T1w (x: −70.20059204 against −70.20058441); a single-precision chain through the
  three products is one unit away on ds000256's oblique T2w (y). The model fits all twelve rows
  (both matrices of both images) and every phantom of the suite. A matrix whose code is 0 is kept
  (`fslroi/type-head-noxform`).
- **fslroi's messages**: an exception is two lines on standard error, `Image Exception : #<n> ::
  <what>` then `<what>` (#21 `Invalid ROI dimensions`, #5 `Out of Bounds (time index)`, #63 `No
  image files match: <name without extension>`).
- **FLIRT's `-out`** (found here, corrected in the FLIRT original; the case
  `out/display-range` of FLIRT's suite): FSL writes the display range `cal_min = cal_max = 0`
  whatever the reference's (MNI152_T1_2mm has 3000 and 8000): in a registration's `-out`,
  `-applyxfm`, `-applyisoxfm`, every interpolation, `-datatype float` or not. The original kept the
  reference's; `standard_space_roi -d`'s `<out>_tmp_to_std` showed it.
- **The script's tests** (FSL-211, `reproduce`): an output name with blanks makes the script's
  test of its second argument print `standard_space_roi: line 112: [: too many arguments`, as
  `[ X$2 = X ]` does; an option with blanks ends the options with `line 133: [: syntax error:
  `-maskNONE' unexpected`, as `[ _$1 != _ ]` does (the options' loop). These forms are inferred from
  the messages (the script was not read); the original evaluates them with GNU bash 5.2's `test`
  rules (its argument-count cases, `-a`, `-o`, `!`, parentheses, the unary and binary operators;
  a leftover argument gives `syntax error: `<it>' unexpected` when it starts with `-`, else `too
  many arguments`). An input, reference or mask name with blanks is "not valid" (FSL's `imtest`
  of several words). For a spec writer: whether the script tests other names unquoted (the
  replica's `[` messages for other options with blanks were not observed).
- **The cascade** (FSL-214, `reproduce`): after FLIRT refuses an option, FSL's script goes on and
  every later program prints its own failure: FLIRT's `Could not open matrix file
  <out>_tmp_to_std.mat` and `Cannot read input-matrix`, `convert_xfm`'s `matrix multiplication:
  incompatible matrix dimensions: 4x4 and 0x0`, fslmaths' abort with bash's report (`<script>:
  line 229: <pid> Aborted` and the command as the script writes it), `fslstats -w` silent on
  standard output so that `fslroi` prints its usage, then the cleanup. The original's programs
  print FSL's text in each of these places: the cases `defects/flirt-refuses`,
  `defects/option-blank` and `defects/output-blank` are FSL's byte for byte.
- **FSL given its own matrix** (the suite's stage check): `standard_space_roi ... -applyxfm -init
  <its own -d matrix>` gives FSL's `-d` files exactly on a phantom, but not in general: on the
  3.5-year-old's T1w the input resampled on MNI152's grid (`<out>_tmp_to_std`) differs in 4,122
  of 902,629 voxels near the field of view's edges (the matrix's text against FLIRT's own
  double-precision matrix). The stage check therefore compares the original with FSL's own run
  from the same matrix's text.
- **imcp's conversion** (§2.5): FSL's (fslpy's, through nibabel) `.nii` copy of a `.nii.gz`
  input is the input's decompressed bytes (`ssroi/phantom-none-convert`); the original converts
  by the files' layout (the same header and data; a pair's magic and offset where the layout
  changes) and passes. The replica refuses that case (pending).
- **FSL's standard images** (§5.4): the original's download of FSL's `fsl-data_standard`
  2208.0-0 (196,312,969 bytes from FSL's site, once, with `curl`) took 17 s on the main machine,
  13.6 MB of memory (the package checked by its size and SHA-256, its bzip2 tar read only until
  the three images, each checked): the three files are FSL 6.0.7.23's byte for byte. CI never
  downloads: the unit tests serve a package of their own as a local file
  (`LARMORX_FSL_DATA_PACKAGE`), with a changed byte, another size and another member refused.
- **Speed** (§7; the adult's T1w of ds000005 to MNI152 2 mm with the defaults, the main
  machine): FSL 21.3 s (one thread, the oracle cache's run), the replica 15.5 s at 1 thread and
  14.6 s at 16, the original 11.5 s at 1 thread and 2.6 s at 16 (its FLIRT's threads), the same
  output. The registration is almost all of it; the original also writes its temporary images
  uncompressed without `-d` and resamples the field of view once when it is both the mask and
  the region. Memory (`tests/parity/test_memory_peak.py`, the suite's 96 × 112 × 84 head with
  `-b`): the original 58 MiB, FSL 67 MiB (its largest program); `fslroi` of the 30-volume int16
  series 69 MiB against FSL's 61 (the input and the box; the allowance 81).

Left for relay 6 (BET's options): `bet -F` and `-Z` of an int8 image (FSL-217 inside bet: the
suite's `bet/fmri-int8`, `bet/small-fov-int8`, `bet/small-fov-int8-debug`, still in
`ORIGINAL_PENDING_CASES`), and `bet -S`, `-B`, `-A`, `-A2`, which run these programs: done, below.

## bet -S, -B, -A, -A2 (the clean-room catch-up, relay 6, 2026-11-05)

`larmorx_fsl::cli::bet_script`, written from `specs/bet.md` §5.3, §5.4, §7, §9 to §11.1, runs each
variation as the script's sequence of the original's own programs, in this process (bet2,
`standard_space_roi`, FLIRT, `fslmaths`, `fslstats`, FAST, betsurf, and the bet script itself for
`-B`'s robust run), with the arguments the spec names; FSL's standard images are found before
anything runs (`larmorx_fsl::data`, relay 5). These black-box runs of FSL 6.0.7.23 (through the
oracle cache: `ssroi_fsl_runs.py` with `-d` and `-v` added, `fsl_program.py`) and of GNU dc
1.4.1 (`dc_probe.py`) showed what the spec leaves open:

- **dc's threshold** (§5.3 step 9, FSL-216): FSL 6.0.7.23's `fslstats -M` and `-S` print six
  decimals (`1951833.117037 ` for `defects/eyes-threshold-exponent`'s brain, whose mean is about
  2 × 10⁶): **no exponent ever reaches dc**, so the spec's first kind of misreading (`2.1e+06`) does
  not occur with this release (that case passes without any dc message). A brain with a negative
  mean (the new case `defects/eyes-negative-mean`, the phantom less 1000: mean `-349.436360`, SD
  `158.050544`) makes FSL print exactly one `dc: stack empty` on standard error and use the
  magnitude, `349.436360 + 3.5 × 158.050544`: the minus sign is dc's subtraction with too few
  numbers on its stack, whatever the order of the expression's words. `reproduce` gives that
  (the case FSL's bit for bit, its message included); the default uses the number, `-349.436360 +
  553.1769040 = 203.740544`, and names FSL-216. dc prints its messages in two pieces, `dc: ` on
  standard error and the rest on standard output (`'e' (0145) unimplemented`), so in FSL's script
  an unimplemented command's text would land in the threshold. `nan` and `inf` (NaN or infinite
  voxels in the brain) are refused in both modes: FSL's script never reaches the threshold with
  them (its FLIRT's NaN matrix ends `standard_space_roi`'s steps; `defects/eyes-nan`), and dc's
  reading of the words `nan` and `inf` as commands (`n`, `a`, `i`, `f`) depends on the script's
  exact expression, which the spec does not give (a question for the spec writer).
- **The threshold's arithmetic**: dc's product `3.5 × SD` at scale 10 keeps `min(1 + s,
  max(10, s))` decimals of an SD with `s` decimals, truncated; the sum is exact; the text has no
  leading zero (`.5`), keeps its scale's zeros (`100.0`), and zero is `0` (`dc_probe.py`, the unit
  tests of `bet_script`).
- **A failed run leaves its temporary files** (`bet/eyes-no-brain`: FSL's 12 `out_tmp_*` files;
  `defects/eyes-nan`: 5): bash's `ERR` trap exits before the script's cleanup. The original does
  the same in both modes (rule C: the files show what failed); a run whose temporary images were
  written uncompressed (no `-d`) rewrites them in `FSLOUTPUTTYPE`'s format before it ends, so
  that the files left are FSL's.
- **bash's reports** of an abort inside `bet` all name line 399 (fslmaths in `-S`'s step 7, bet2
  in its step 3, betsurf in `-A2`), each with the command as the script writes it: `${FSLDIR}/bin/
  bet2 ${OUT}_tmp_premask ${OUT}_tmp_premask_bet -n -m -f 0.4 $bet2opts`, `${FSLDIR}/bin/betsurf
  -o -m -s $IN ${OUT}_tmp_T2_to_T1 ${OUT}_mesh.vtk ${OUT}_tmp_T1_to_std.mat $OUT` (observed); the
  other commands' texts follow the same form (not observed: they do not abort on the suite's
  inputs).
- **`-A2` with a T2 that does not exist** (`bet/surfaces-t2-missing`): FSL's FLIRT prints its
  image errors and exits 0 (FLIRT's FSL-68), the script goes on (the registration to MNI152, bet2
  `-e`), and betsurf aborts on the missing `<out>_tmp_T2_to_T1`: its messages, bash's report and the
  failure line. `reproduce` gives exactly that (refused by both alike); the default's FLIRT exits 1
  there, so the script fails at once.
- **`bet -S` with NaN voxels** (`defects/eyes-nan`, `reproduce`): FSL's FLIRT writes a NaN matrix,
  `convert_xfm -inverse` aborts (`inv(): matrix is singular`), the later steps fail on missing
  files, fslpy's `imcp` prints `Could not find a supported file with prefix
  "out_tmp_premask_tmp_masked"` (standard output) and `standard_space_roi -d` ends with status 0;
  bet2 then aborts on the missing premask. Found here and corrected in the original's
  `standard_space_roi`: fslpy's message for a missing source (it printed the image layer's) and
  the status 0 with `-d` in `reproduce` (it returned imcp's 1). In the default the original's FLIRT
  leaves the NaN voxels out of its costs (`specs/flirt.md`'s FSL-14), the script goes on, the brain's
  mean is printed `nan` and the threshold refuses it (FSL-216's rule B): the default refuses, as the
  replica's (whose FLIRT refuses the input). The spec's §5.3 says "FSL-14 of FLIRT: the default
  refuses there", which `specs/flirt.md` does not say (a question for the spec writer); and the
  original's FLIRT does not name FSL-14 when it leaves NaN voxels out of a registration's costs (it
  names it for the centres of gravity only): a gap of FLIRT's notices, for its next part.
- **The sign of bet2's zero minimum** (`bet/eyes-verbose`, `bet/bias-skull`, `-v`): `standard_space_roi
  -b`'s premasked image holds +0 and −0 (fslmaths' `-mas` makes a negative value outside the mask
  −0, `specs/fslmaths.md` §6.3); its first voxel is +0 and FSL's bet2 prints `min 0`. The original
  combined its parallel chunks' minima with `f32::min`, which can return either zero, and printed
  `min -0`; its extremes now keep the first of equal values in every chunk and between chunks (a
  scan from the first voxel), at any thread count.
- **`-R`'s centre** is now `fslstats <brain> -l 0.001 -C` run in this process (`specs/fslstats.md`
  §5.7: FSL's block sums, every value less the minimum; the script's own call, `specs/fslstats.md`
  §1), its six decimals passed on as text; the earlier sum of its own gave the same text on every
  record case. fslstats' centre now computes each weight as it sums it (no copy of the volume in
  double precision: `bet -R`'s memory call, 130 MiB against an allowance of 127, came back under
  it).
- **The 3.5-year-old with `-B`** (`real/child-3y-B`, the record's failure): every temporary file of
  the `-d` runs is FSL's bit for bit (both `standard_space_roi` runs, FAST's bias field and its six
  other files, the unbiased images); the difference is the robust run's second bet2 pass, `bet2
  <unbiased premask> -c 85.061718 75.366898 129.742609` (the first pass and its centre FSL's
  exactly): FSL's self-intersection total 123.412, the original's 124.668, the mask's Dice 0.9983,
  the boundary distance's 95th percentile 1.0 and its maximum 2.24 voxels (bet2's threshold 2; the
  brain's values FSL's inside both). The image is stored neurologically (its qform a rotation of
  positive determinant): `-c` on such an image is the open question of random testing's cases 622
  and 677 (the conversion of a centre given in voxels between the storage and FSL's radiological
  frame, through FSL's 4 × 4 inverse), for the spec writer.

**What agrees.** The suite's 31 new cases of relay 6 (the record of 2026-11-05): `-S` and `-B` FSL's
**bit for bit** in every case but the 3.5-year-old (every output and every `-d` file, the real
adult, tumour and 82-year-old included; `-S` with `-n`, a NaN head, an int16 pair output), `-A` and
`-A2` with bet2's brain and mesh and FLIRT's matrices FSL's and betsurf's masks within betsurf's
thresholds (the 75-year-old's and the 7.5-year-old's Dice 0.993 to 0.9995, as betsurf's own record);
`-F` and `-Z` of int8 heads (FSL-217) FSL's in `reproduce`. Messages and statuses FSL's byte for
byte in every case (bash's process IDs masked).

**Built for speed.** The programs are the original's (parallel inside); the temporary images are
written uncompressed when they are not kept; the eye threshold in exact decimal arithmetic. Wall
times on the main machine (`bench_options.py`, one run at a time; FSL's from the oracle cache's
record of its single-threaded run, made beside other suite cases): `-S -m` on the adult T1w of
ds000005 FSL 27.6 s, the replica 20.8 s (one thread), the original 15.6 s on one thread, 5.7 s on
4, 3.4 s on 28; `-B -m` 85.8, 57.5, 29.2, 10.3 and 6.2 s; `-A` 19.0, 18.6, 13.5, 4.9 and 3.0 s;
`-A2 -m` on the 7.5-year-old's T1w and T2w 108.3, 91.6, 79.1, 23.4 and 11.0 s (the original 1.4 to
2.9 times FSL's speed on one thread, 6 to 14 times on 28). The steps run one after the
other, as the script runs them: two at once (§10 allows `-S`'s raw bet2 beside its
`standard_space_roi`, `-A`'s FLIRT beside bet2) would hold both programs' images, and larmorx's
peak memory is held to FSL's, whose peak is its largest program's (CLAUDE.md). Memory
(`tests/parity/test_memory_peak.py`, the suite's head with FSL's standard images): `-S -m` 62 MiB
against FSL's 67, `-B` 61 against 115, `-A2` 58 against 72.

## The BET part's random-testing differences and ds002603's scalp (the clean-room catch-up, relay 7, 2026-11-05)

The BET options part's trial on the original (`oracles/difftest/2026-11-04-bet-options-6b`, seed
`bet-options-6`, 2,699 cases) kept 176 differing cases of the original: 164 betsurf, 11 `bet`, 1
`bet2`. Judged again as black-box cases with relay 6's build (`replay_trial.py`: each finding's
recipe and its shrunk reproducer, FSL from the oracle cache, each case in its own 3 GiB leaf of
the memory box), 172 still differed. `betsurf_shares.py` ran every betsurf finding again with
`-v`: the share of profiles set aside (`removed profiles`) was FSL's on only 74 of the 164 images,
and 82 of the 90 others were `--t1only`. So most differences began at the profiles' decisions.
`profile_oracle_cached.py` (`betsurf_profile_oracle.py`'s single profiles, FSL through the cache;
the profiles' voxels, normals and samples from a temporary probe build of the original, never
kept) then gave FSL's decision on every profile of eight T1-only and three T1-and-T2 images
(5,290 single profiles), and `t1model.py`-style readings of the rules were tried against them.
Four readings of `specs/bet.md` §6.4 were the original's own mistakes (rule A: the original now
follows FSL; none is a defect of FSL):

- **A profile's first sample is never read backwards** (T1 only): the backward searches (the
  scalp, `S'`, the outer skull's two searches) stop before the profile's first sample, and the
  T1-only scan for `b` starts at the second sample when `a` is the first (`b` itself starts at
  `a`). A profile whose inner and outer skull both fell on its first sample (−3 mm) is set aside
  by FSL (case 1156: FSL sets aside 63 of 64 profiles, the original kept 7 more; case 678's
  profile 120: the scan from the first sample stopped at 3.5 mm, from the second it finds the
  outer skull at 6.0 mm as FSL). The forward searches do read the first sample (11 mismatches of
  604 profiles otherwise). 3,284 of 3,288 T1-only profiles now decided as FSL decides them.
- **Step 5's "within 10 mm" is strict**: a first bright T2 sample exactly 10 mm past the inner skull
  does not count (case 2530: three such profiles; case 1746: three).
- **Step 5's alternation**: each search starts after the sample the previous one found (case
  617's profile 74: a flat region, 159 on T1 and T2 alike, whose one sample is bright and dark at
  once; FSL sets it aside).
- **An empty outer-skull search range** (step 7: `a` finds nothing) **counts the scalp only, and
  FSL runs on.** The spec's FSL-50 (FSL ends with status 255 there) was never seen: on the 43
  images of the two trials where such a profile occurs (41 of this trial's with these rules, 278,
  913, 1746, 1924 and 2557 among them, and cases 216 and 635 of 2026-10-31) FSL runs to the end
  (status 0; case 364 fails later at its output type, as the original does). Counting only the scalp is what FSL does: on case 617 (719
  such profiles, a ramp whose inner skull lies at its scalp) the scalp mesh is 0.41 mm from FSL's
  (95th percentile) when they count the scalp and 5.76 mm when they are dropped, the shares equal
  either way. The original therefore no longer stops in `reproduce` (`larmorx_fsl::cli::betsurf`:
  the `fsl50` switch of `betsurf::votes` is off); when FSL ends (FSL-50) is a question for the spec
  writer.

With these, **162 of the 164 images' shares are FSL's** (74 before); the two left: case 192 (four
profiles of a constant int16 image, every value 32767, whose two outer searches find nothing in
a two-sample region at the profile's end: FSL keeps them, the rule `OS2 ≥ IS` sets them aside)
and case 1389 (two profiles more set aside in the whole run, though each of its 319 profiles alone
is decided as FSL decides it). And **`bet -Z -d` on a float64 image** (case 330) now keeps the
input's double values in `<out>_tmp`, `<out>_tmp_bottom` and `<out>_tmp_top` (FSL's are float64;
the original wrote float32), as `-F`'s first volume does. Permanent cases (`validation/difftest/
cases/fsl-bet/`): `original-betsurf-t1only-first-sample` (1156),
`original-betsurf-gap-strictly-within-10mm` (2530), `original-betsurf-flat-region-gaps` (617),
`original-betsurf-empty-range-scalp-only` (1746), `original-bet-Z-d-float64-tmp` (330); their
shrunk reproducers agree.

**What still differs** (all 176 judged again with the final build: 25 agree, 151 differ):

- `bet` (6) and `bet2` (1): every one a neurologically stored image (a qform or sform of positive
  determinant) with `-R` or `-c` (the skull image in 5, the self-intersection total in 2: 6.54282e+06
  against 6.3927e+06, 20.1978 against 20.1488): the class of cases 622 and 677, the conversion of a
  centre in voxels into FSL's radiological frame (open with the spec writer).
- betsurf (144): the decisions are FSL's, the surfaces are not exactly. Per image, the 95th
  percentile of the vertex distance to FSL's mesh: the inner skull exactly FSL's on 6 images and
  within 0.1 mm on 122 of 156 (median 0.047 mm), the outer skull median 0.82 mm, the scalp median
  3.6 mm (13 of 163 within 0.1 mm). The failing checks: the scalp's mask or outline alone in 88
  cases, the skull mask alone in 19 and with the scalp in 15, the inner skull in 8, the outer
  skull with the scalp in 7. On these images of 10 to 32 voxels per side the masks are a few
  hundred voxels (the skull masks of case 47: 2 and 8 voxels; Dice 0.4 for 6 voxels), so
  betsurf's thresholds (Dice ≥ 0.99, boundary p95 ≤ 1 voxel) fail on a voxel or two.
  - **The scalp's deformation**: on the four images where every profile is the scalp's only and the
    inner and outer skull are FSL's exactly (cases 1504, 651, 180, 2101: the skulls shrink to 1 to
    2 mm, the scalp's sphere grows from there to 50 to 70 mm, far outside the working grid), the
    scalp is 0.24, 1.58, 1.95 and 3.43 mm from FSL's (95th percentile). Reading the counting image
    outside the grid as its nearest edge value instead of 0, or as 0 beyond `[0, n − 1]`, makes it
    worse on all four (6.2 to 49.6 mm and 1.2 to 7.9 mm). These four are the cleanest inputs for
    the spec writer's question about the scalp phase (§6.5).
  - **`-p` with every profile set aside** (8 cases): the inner skull shrinks to one point (every
    vertex `(20.25, 23.25, 7.5)` on case 1974), and FSL's retessellation of that collapsed mesh
    writes 782 points where the original's (each edge's midpoint once) writes 1,026, with the same
    first faces: how FSL shares midpoints between edges of coincident points is open (the
    coordinates are the same point either way).
- **ds002603 with its T2** (`real/philips-surf-t2`, the record's failure): with the corrected
  profile rules its inner skull is FSL's to 0.026 mm (95th percentile; maximum 1.22 mm) where it
  was 0.95 mm (maximum 11.0 mm: the "few profiles vote" region of part 2b, now explained by step
  5's rules), its outer skull 0.20 mm (0.56); the scalp stays 2.86 mm (3.18; maximum 16.9 mm), its
  mask's Dice 0.9935 and boundary p95 2.0 voxels (the threshold 1): the scalp's deformation above.
  The record's other surface cases keep their status (the 82-year-old's scalp 0.55 → 0.30 mm, the
  adult's 0.66 → 1.36 mm: the scalp phase is sensitive to the vote image's last details).

**A regression of the branch, found by the permanent cases and corrected** (2026-11-05): the
part 2b cases `original-bet-R-centre-zero` and `original-bet-R-negative-image-bet2-crash` (both on
neurologically stored images) agreed on main and differed with relay 6's build. Since relay 6, `bet
-R`'s centre is the original's `fslstats -l 0.001 -C`, FSL's text: for an empty first-pass brain
it prints `7.000000 0.000000 0.000000` on an 8-voxel-wide neurologically stored image (both
programs print it, with the COG warning), and FSL's `bet2 -c 7.000000 0.000000 0.000000` there uses
the centre of gravity (`c-of-g 3.50629 5.57614 6.83629 mm`, black box), where the original used the
voxel `(0, 0, 0)`. So **a centre of three zeros is "not given" in FSL's radiological frame, after the
conversion of §3.2** (`nx − 1 − x` on a mirrored image), not as typed: `-c 0 0 0` on a
radiologically stored image (case 1608 of part 2b) and `-c nx−1 0 0` on a neurologically stored one
both mean the centre of gravity (`larmorx_fsl::cli::bet2::centre_mm`; the three `-R` permanent
cases agree again).

The probes: `replay_trial.py`, `finding_dirs.py`, `betsurf_shares.py`, `profile_oracle_cached.py`
([validation/investigations/cleanroom-bet](../../validation/investigations/cleanroom-bet/README.md)).

## The catch-up's random-testing trial (relay 8, 2026-11-05)

`oracles/difftest/2026-11-05-catchup-bet` (seed `bet-options-6`, the BET part's trial drawn again;
30 minutes, 14 jobs at one thread, 3 GiB a case, both implementations, the oracle cache in `use`
mode): 909 cases; the original 792 agree, 114 differ, 2 notices (FSL-51), 1 undefined; the replica
896 agree, 13 differ, every one where FSL's FLIRT crashes inside standard_space_roi (FSL-75). The
original's: 53 betsurf surfaces (`-A`, `-A2`, betsurf: the scalp's deformation on small heads,
relay 7's open class); about 27 standard_space_roi runs whose FLIRT crashes in FSL
(`Segmentation fault`, `free(): invalid pointer`: FSL-75, never reproduced; FSL-214: the script
goes on) where the original registers; 19 standard_space_roi headers that differ from FSL's even
given FSL's matrix (of four materialised: an input converted to another format by `-maskNONE
-roiNONE`'s copy, FSL's `pixdim[4..7]` 1 where the input's are 0, §2.5's pending conversion,
`bet-options-6-00657`; the cut's translation one float unit in the last place, `00407`; a box a voxel
or more apart on tiny images, `00114`, `00165`) and 14 FLIRT matrices that part (`00009`): open; 6
fslroi boxes of a neurologically stored oblique image reaching its last x voxel, whose corner
translation is one float unit in the last place from FSL's (`00796`: FSL's equals the double
product `M·(20, 4, 4)` rounded once, where §3.2's held frame rounds its translation first: a
question for the spec writer); 5 `-B` and `-A2` runs where FSL fails midway and the messages part
(`00225`, `00409`: open).


## The trial's last differences triaged (relay 10, 2026-11-05)

Every one of the trial's 114 differences of the original replayed with the final build
(`replay_trial.py`, the drawn recipes: `trial_catchup_replay.jsonl`) and put in a category
(`trial_catchup_categories.py`, its output `trial_catchup_triage.json`); the standard_space_roi
findings measured with `trial_ssroi_matrices.py` (each call with `-d`, FSL's own `flirt` measuring
both final matrices' costs at 8, 4, 2 and 1 mm, and the output header given FSL's matrix:
`trial_ssroi_matrices.jsonl`).

- **Fixed: FLIRT's search, starting points of equal cost** (15 findings, and 5 output boxes that
  followed the matrix): standard_space_roi's matrices 37 to 303 mm (RMS) from FSL's and `bet -B`'s
  inner ones, on small heads whose every cost is 1 at the coarse scales (FSL's own measure of both
  matrices and of the identity): the search's starting points tied, and FSL sorts them with
  `std::sort` (`docs/findings/fsl-flirt-clean-room.md`, "The search's starting points of equal
  cost"). All 20 agree on replay; `bet -B` 531, where FSL finished and the original's script had
  failed midway, among them. Permanent cases `original-ssroi-search-equal-costs`,
  `original-bet-B-search-equal-costs`.
- **Open, a registration that still ends elsewhere** (3): 88 at a lower cost than FSL's by FSL's
  measure (0.964919 against 0.967734 at 1 mm, 183 mm apart), 114 at a higher one (0.989526
  against 0.985458; 0.674786 against 0.803613 at 4 mm), 803 (110 mm, its output box).
- **Open, standard_space_roi's output header given FSL's matrix** (17): 9 the cut's translation
  one float unit in the last place (407: `qoffset_z` 8.25 against 8.250000954); 4 a form's
  translation whole voxels from FSL's where the input's qform and sform differ (189:
  `qoffset_x` −14.25 against −38.25; 313, 315, 456; the box, its values and the other form are
  FSL's: which form FSL's fslroi moves by which corner is a question for the spec writer,
  §3.2); 2 NIfTI-2 outputs of a NIfTI-1 input (543: FSL's matrices in single precision,
  −44.40308380 against −44.40308296; 567: FSL moves a sform of code 0 too, its translation
  −1.2412 where the original keeps the input's 51.25); 2 fslpy's conversion (657: `pixdim[4..7]`
  1 against 0; 222: `scl_slope` 1 against 0; 88 also NIfTI-2 for a NIFTI2 type against the
  original's NIfTI-1: §2.5's pending conversion).
- **Unchanged**: 53 betsurf surfaces (the scalp's deformation, relay 7's open class), 8
  standard_space_roi runs whose FLIRT crashes in FSL (FSL-75), 6 fslroi corners one float unit
  apart (relay 8's question), 5 `-B`/`-A2` runs where FSL fails midway (not yet looked at), 2
  `-R`/`-c` centres on neurologically stored images (the question of cases 622 and 677).

## The time step as FSL's image layer reads it (PRELUDE's clean-room part, relay 3, 2026-11-07)

`validation/investigations/fsl-pair-header/time_step.py` (FSL 6.0.7.23 through the oracle cache):
bet2 (a 3D image and a series, `-m`), bet `-F` (its 4D brain and 3D mask) and fslroi (a series,
one volume, a 3D region) wrote the stored time step where FSL writes its magnitude, and 1 for an
image with a time axis whose time step is 0 (FSL's image layer; a 3D image's 0 kept). Corrected
in the shared readers (`larmorx_mri::hmc::image::time_step_as_read`, `bet::image`'s ANALYZE and
lean readers); standard_space_roi's `-maskNONE -roiNONE` copy (fslpy's `imcp`) keeps the stored
value, as FSL's does. The records `fsl-bet` and `fsl-fast` re-run from `f005a08`: unchanged
(their inputs' time steps are positive, or 0 in 3D images).
