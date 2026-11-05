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
`ORIGINAL_PENDING_CASES`), and `bet -S`, `-B`, `-A`, `-A2`, which run these programs.
