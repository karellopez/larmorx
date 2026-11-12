# Findings: the clean-room originals of 3dAutomask and 3dUnifize

What the clean-room implementer of `crates/larmorx-afni`'s `automask` and `unifize` learned
from **black-box runs** of AFNI 25.2.09's `3dAutomask` and `3dUnifize`
(`oracles/afni-25.2.09/bin/`, `scripts/build_afni_oracle.sh`) and of the replica program, and
how the original is built. Nothing here comes from AFNI's source or the replica's: this file is
**unrestricted** (the source-derived findings are [afni-automask.md](afni-automask.md) and
[afni-unifize.md](afni-unifize.md), which the implementer did not open). The behaviour rules
found are also in the specs, marked *observed by the clean-room implementer*
([specs/3dAutomask.md](../../specs/3dAutomask.md), [specs/3dUnifize.md](../../specs/3dUnifize.md)).

## AFNI reads its own header extension on AFNI-written inputs (AFNI-10)

- **What:** fMRIPrep's 3dAutomask call reads 3dUnifize's output (`fixhdr_unifize/uni_xform.nii.gz`),
  which carries AFNI's own NIfTI header extension (code 4, an XML list of AFNI attributes).
  AFNI takes the geometry from those attributes, written with 7 significant digits, instead of
  the NIfTI header's sform and qform. On ds005454's BOLD reference (an oblique 1.4 mm grid
  whose sform is not exactly its qform) AFNI's masks have an sform whose voxel sizes are
  1.40000034 mm (the input's 1.39999998) and whose origin is moved by up to 3.8e-5 mm. The
  data are bit-identical; only the header differs.
- **Where:** `parity afni-automask`, case `fmriprep/ds005454-task_rest`.
- **How we know:** *verified*: AFNI run on the same file with its extension removed (the header
  bytes unchanged but `vox_offset`) writes the original's header byte for byte
  (`afni_replica.afni_10_explainer`, run in every case of the original's suites whose input has
  the extension).
- **larmorx:** the clean-room original ignores AFNI's extension, as 3dTshift's original does
  (AFNI-10 in the register: class (d), the replica reproduces it). The parity record names the
  case and the evidence; the other oracle runs' BOLD references are plumb or exact in 7 digits,
  and their headers are AFNI's byte for byte.

## glibc's `tanhf` decides the last bit of 3dUnifize's squashed values

- **What:** 3dUnifize squashes scaled values above 1300 with `tanhf`; glibc 2.35's is not
  correctly rounded on some arguments (the spec gives 2.8 % of the floats). The original uses a
  correctly rounded `tanhf` (`larmorx_core::math::tanhf`, a port of CORE-MATH's, bit-identical
  to the C on all 2^32 floats), so a squashed value can differ from AFNI's in its last bit.
- **How we know:** *verified*: on a 4D phantom (8 values differ, at most 1.2e-4) and with
  `-rbt 10 30 60` (25 values), AFNI with CORE-MATH's `tanhf` and `cbrtf` preloaded
  (`scripts/build_crmath_preload.sh`, which builds them since 2026-10-23) writes the original's
  output bit for bit. The parity suite does the same for every case whose float values differ
  (`afni_replica._correctly_rounded`).
- **larmorx:** correctly rounded maths (CLAUDE.md rule 5): the original's float32 outputs are
  held to PLAN.md §11.3's threshold (max |diff| ≤ 1e-5 × max |AFNI output|), bit-identity
  reported; the replica carries glibc's `tanhf` and is bit-identical.

## Messages and outputs the specs did not state

All in the specs as *observed*; in short:

- 3dAutomask: a unit `scl_slope` is kept as a brick factor of 1 also for float32 (nibabel writes
  1 by default), so `-apply_prefix` writes `scl_slope` 1; with `-eclip`, `Mask now has` counts
  the mask before the exterior clip's last peel, whose clustering lines follow; `++ Dilating
  automask` only for at least one dilation, ` + Eroded` only for erosions that cleared
  something; the dilate/erode warning after a successful option scan, whenever both options
  were given; a plane's mask prints no ` + ` lines and is written 2D (`dim[0]` 2); `++ Output
  dataset` after an attempted write iff a file of that name then exists; `AFNI_DECONFLICT=N...`
  acts as unset.
- 3dUnifize: `-verb` once shows the automask's lines; `I` and `A` come before their automask's
  lines, `m` after; `U` also without the half-size path; `-ssave` and `-amsave` print no
  `Output dataset` line; `-EPI`'s two lines are printed with `-quiet`, its dots too (without the
  newline); `-EPI` sets the `-T2` count to 1; the automask works on signed values.

## Speed: what the original does differently

| Step | Direct method | The original | Same result because |
|---|---|---|---|
| 3dUnifize's local intensity (§4.3) | gather and sort the ball's values at every voxel (about 25,000 at the default radius) | a window of counts per rank (each distinct value's position in the sorted list of the image's values) slid along rows and planes; only the two surfaces of the ball change from voxel to voxel; the two order statistics found through block counts; the values between them read in ascending order | the spec's mean depends only on the sorted values between the two ranks and on summing them in ascending order, which the window gives exactly (unit test against the direct method, ties and negatives included) |
| The exterior clip (§5.6) | sweeps in storage order, in place, until one adds nothing | a breadth-first search from the outside through the candidate voxels | the sweeps stop only at the closure, which does not depend on the order (unit test against the sweeps) |
| The depth (§6.3) | erode until empty, counting | a breadth-first search, one layer per erosion | an erosion removes exactly the voxels next to the previous layer (unit test against repeated erosions) |
| Clusters (§5.2, §7) | flood fills | run-based union-find, roots kept at the smallest run | the root is the run of the cluster's first voxel, the spec's tie rule (unit test against a flood fill) |
| The clip level's histogram (§4) | one pass in storage order with a double sum | parallel integer histograms over z slabs | integer counts and an integer sum of squares, equal to the spec's double sum below 2^53 (a sequential double pass above) |
| Everything voxel-wise | one thread | parallel over z planes (and `-EPI`'s volumes, the gradual clip level's octants) | each voxel decided from the mask before the pass |

AFNI 25.2.09's 3dUnifize took 96 s at one OpenMP thread on the 64 × 64 × 40 head phantom of
the parity suite (default radius 18.3 voxels). Informal timings of all three programs are in the
next section.

## Informal timings (2026-10-24; not a benchmark report)

Wall time in seconds of the whole program (reading and writing `.nii.gz` included), each run
once (larmorx after one warm-up run), at 1 and 6 threads (`OMP_NUM_THREADS`; AFNI's 3dAutomask
is single-threaded). The machine (12 logical CPUs) was shared with another agent's runs (load 17
to 22 throughout), so these are indications, the 6-thread ones especially; a benchmark report on
a quiet machine is still to come. Tool: [`validation/investigations/afni-masks-original/timing.py`](../../validation/investigations/afni-masks-original/timing.py)
(numbers in `timings-2026-10-24.json` beside it).

| Input | AFNI 1 | replica 1 | original 1 | AFNI 6 | replica 6 | original 6 |
|---|---|---|---|---|---|---|
| 3dUnifize, fMRIPrep's call, ds000005 BOLD reference (64 × 64 × 34) | 93.3 | 39.4 | **3.8** | 21.6 | 8.7 | **0.94** |
| 3dUnifize, fMRIPrep's call, ds005454 BOLD reference (160 × 160 × 96, half-size path) | 18.7 | 13.3 | **4.6** | 7.5 | 4.9 | **1.2** |
| 3dUnifize, a T1w, ds000005 (half-size path) | 67.1 | 35.4 | **9.3** | 23.6 | 14.9 | **2.7** |
| 3dAutomask, fMRIPrep's call, ds005454 BOLD reference | 1.6 | 1.9 | 1.7 | – | 1.9 | **0.72** |
| 3dAutomask, a BOLD series, ds005040 | 0.39 | 0.25 | 0.22 | – | 0.24 | 0.19 |

3dUnifize's original is 4 to 25 times faster than AFNI at the same thread count, and 3 to 10
times faster than the replica (its ball percentiles by a sliding window of ranks instead of a
sort at every voxel); on the half-size path the ball works on an eighth of the voxels, and
reading and writing the full-size compressed images takes a larger share of the time. 3dAutomask's time is
mostly reading and writing compressed NIfTI: the three programs are within a few tenths of a
second, the original faster at 6 threads.

## The fixes of 2026-10-29 (random testing's and the data panel's differences)

Found by random testing (2026-10-25) and the data panel (2026-10-28), settled by black-box runs
of AFNI 25.2.09 and of the replica program, by an implementer who had never read either's
source; the tools are in
[`validation/investigations/cleanroom-fixes/`](../../validation/investigations/cleanroom-fixes/)
(its README), the rules in the specs (observed by the clean-room implementer, 2026-10-29).

| Difference | Cause found | Fix (all three tools unless named) |
|---|---|---|
| Header bytes of tiny oblique images (3dAutomask `mask.nii`, `.nii.gz`; 3dUnifize `uni.nii`, `-amsave`) | the quaternion's rounding residue: AFNI forms the polar decomposition's `1/det` in extended precision (1,200 of 1,200 quaternions read as doubles from NIfTI-2 outputs) | `geometry::inverse` |
| 3dAutomask's `[from ?]`; a one-slice oblique image's mask with `-SI` | the orientation letters are the NIfTI library's closest plumb orientation, in double (1,604 of 1,604 matrices), not each column's largest entry | `automask::program::orientation` |
| 3dTshift's `toffset` 0.35999995 against 0.36000001 | header slice times in ms: a float32 sum of the step kept in double (every time of 16 headers read back with `-slice k`) | `timing::header_timing` |
| 3dTshift's default on metre and micrometre grids, 5.5e-5 to 2.4e-4 from the replica's | AFNI-07's default (shared with the replica): the steps from the file's matrix then converted; a qform's matrix computed with millimetre voxel sizes and offsets, rounded once (100 of 100) | `geometry::output_geometry`, `qform_matrix_scaled` |
| 3dAutomask's `-apply_prefix` writes +0 where AFNI keeps −0 | float64 (and int8, uint16, int32, uint32) data without a usable slope are only converted | `dataset::read_as` |
| AFNI-03 not named, AFNI-08 in `reproduce` only (3dAutomask `-apply_prefix`, 3dUnifize `-EPI`, header slice code 2) | the replica names AFNI-03 in both modes and AFNI-08 where the kept times are all within 3 ms of 0 | `automask::copy_timing`, the replica's words |
| AFNI-43 not named where only the clustering messages differ (`-peels 0`) | the replica names it there; its default peels once for a negative value | `automask`, the replica's words |
| AFNI-07's and AFNI-16's words and order | the replica's | `geometry::output_geometry` |
| The data panel's obliquity angles (five 3dAutomask calls, 20.646870 against 20.646889) | AFNI reports the obliquity of its own header extension's 7-digit matrix (AFNI-10); on the input without the extension it prints the original's angle | none in the original (it ignores the extension, AFNI-10); the parity suite explains such messages by AFNI-10 |

After the fixes every one of the night's findings of the three tools' originals, and the 83 of
the replicas' default against the original's, replays as *agree* (the masks harness accepts
AFNI-08 named in `reproduce` only for header slice code 2, `one_mode`; one 3dTshift replica
finding replays as *notice*, the replica's own AFNI-01 notice, and five 3dUnifize replica
findings stay *memory*),
and the standard records are re-made: 3dTshift 315 cases, 3dAutomask 130 (with the data
panel's 15 fMRIPrep calls, all passing; 14 inputs carry AFNI's extension), 3dUnifize 117, with
no change of status in any earlier case.

## AFNI 26.3.05 (2026-10-29)

The clean-room originals of 3dTshift, 3dAutomask and 3dUnifize follow AFNI 26.3.05 since
2026-10-29 (next-steps item 3b), from the specs' lines marked "(AFNI 26.3.05)" only. AFNI 26.3.05
changed these programs in two places, both corrections larmorx already made by default: header
slice code 2 is read as NIfTI defines it (`slice_end` down to `slice_start`; AFNI 25.2.09 gave
every slice time 0: AFNI-03), and 3dTshift's `-no_detrend` de-means every voxel (AFNI 25.2.09
left the second voxel of each pair raw: AFNI-05). So `reproduce` now gives the default's result
there, and neither defect is named: in 3dTshift's shift, in the slice fields 3dAutomask's
`-apply_prefix` and 3dUnifize's `-EPI` keep, and in their notices (AFNI-08 is still named where
the kept times are all within 3 ms of 0, in both modes).

- **The records against AFNI 26.3.05** (standard tier, `--afni-version 26.3.05`): 3dTshift 315
  cases, 280 pass, 32 rejected by both, 3 expected divergences, none failing (178 of the 280
  bit-identical in their data); 3dAutomask 130 cases, 110 pass, 19 rejected by both, 1 expected
  divergence (92 of the 106 compared cases that write outputs bit-identical, the other 14 AFNI-10's
  header extension); 3dUnifize 117 cases, 96 pass, 19 rejected by both, 2 expected divergences
  (37 of 92 bit-identical, the other 55 bit-identical to AFNI with correctly rounded maths). Before
  the alignment, against 26.3.05: 3dTshift 6 cases failed (`-no_detrend` and header slice code 2),
  3dAutomask 1 and 3dUnifize 2 (the permanent cases of header slice code 2); every other case
  agreed as against 25.2.09. fMRIPrep's calls meet neither change: AFNI 26.3.05 gives 25.2.09's
  bytes on them, and the original's outputs on them are unchanged.
- **The three permanent cases of header slice code 2** (`afni-automask/slice-code-2-masked-notices`,
  `afni-unifize/epi-slice-code-2-fix-notice`, `epi-slice-code-2-reproduce-notices`) are
  re-described for 26.3.05: the code-2 times kept in both modes, AFNI-03 never named.
- **Speed** (`validation/investigations/cleanroom-align/speed.py`, ds000005's BOLD run): the
  default mode takes the time it took; `reproduce` on a header with slice code 2 now shifts the
  slices, as AFNI 26.3.05 does, where it copied them before (0.45 s against 0.07 s at one thread),
  and `-no_detrend` de-means every voxel (0.30 s against 0.29 s).

## AFNI-75's guard in the clean-room 3dUnifize (2026-10-30)

Written from `specs/3dUnifize.md` §7, §9.1 and §10 (the spec writer's lines of 2026-10-30) by a
clean-room implementer, with black-box runs of AFNI 26.3.05's `3dUnifize` and of the replica
program (`LARMORX_IMPLEMENTATION=replica larmorx afni 3dUnifize`); probe
[`validation/investigations/cleanroom-additions/unifize_guard.py`](../../validation/investigations/cleanroom-additions/unifize_guard.py).

- **The same refusals, word for word:** on the spec's cases (4D and one-volume series of 3 µm
  and 1 µm voxels, 0.05 mm voxels, `LARMORX_MEMORY_LIMIT=4G`, two workers) the original's
  warning and fatal lines are the replica's, byte for byte (`about 2.834e4 GiB`, radius 6100.0;
  `7.651e5 GiB`, radius 18300.0; `6.12 GiB`, radius 366.0, no hint); exit 1, nothing written.
  0.3 mm (61 voxels) and 3 mm (6.1) are not refused, and AFNI, the replica and the original give
  the same output, bit for bit, in both modes.
- **The guard is decided on the caller's thread:** a limit given to the computation
  (`larmorx_core::alloc::with_memory_limit`) is the thread's, and the local mean runs in a pool
  of its own, so the prediction is made before the pool starts.
- **`reproduce` without the guard needs little:** the local mean keeps only the ball's lines
  that reach the grid (at most `n − 1` voxels along an axis of `n`, §9.1); a ball line at least
  as long as its axis covers it from every voxel, so every ball holds the same values. 3 µm and
  1 µm voxels on a 24 × 24 × 16 series (radii 6100 and 18300) then take 0.01 s and 5 MiB; the
  unit tests compare the method with the spec's ball, gathered and sorted, for radii beyond the
  grid on every axis.
- **Random testing after the guard** (a trial of 0.5 h, 2026-10-30, larmorx `6f64291`, AFNI
  26.3.05): 1,768 `afni-unifize` cases, 1,757 agree in both implementations, 9 over the case's
  memory limit (3 GiB: `-EPI` on tiny voxels) where both implementations' defaults stop with
  AFNI-75's error (verdict *guard*, since this part for the original too: `difftest/runner.py`'s
  `GUARDED`), one *notice* (AFNI-45 in both implementations) and one *undefined* case (AFNI's
  values from memory it never wrote), none differing.

## `-EPI`'s radius and glibc's cube root in the clean-room 3dUnifize (GLIBC-05, 2026-10-30)

Written from [`specs/cbrtf.md`](../../specs/cbrtf.md) and `specs/3dUnifize.md` §4.3 and §7 by a
clean-room implementer who has never read glibc's source, with black-box runs of the system's
`cbrtf` (glibc 2.35, through `dlopen`) and of AFNI 26.3.05's `3dUnifize`; the user's decision of
2026-10-30 (the default correctly rounded, `reproduce` glibc's bits).

- **The cube root with glibc's bits:** `larmorx_core::math::cbrtf_glibc`, the spec's method (a
  quadratic start in double rounded to float, its cube in float, one Halley step in double, the
  factor `2^(ρ/3)`, then float; no fused multiply-add), equals the system's `cbrtf` on **all 2^32
  float inputs**, bit for bit, NaN results included (16,777,214 NaN inputs: the quiet bit set, the
  sign and payload kept), and its 4096 block hashes are the spec writer's
  ([the proof and its verifier](../../validation/investigations/cleanroom-cbrtf/README.md)). It
  is faster than glibc's: 3.1 ns per call against 7.1 (through a function pointer) and 3.7 for
  CORE-MATH's correctly rounded one. glibc's result is not the correctly rounded one on
  453,492,162 of the 4,278,190,082 non-NaN inputs (10.6 %).
- **The spec's whole-sequence hash** (§7.2, `c679cb92d4402e33`) is not what the system's own
  results give under §7.1's definition run through all inputs (`335d1fb5fb714c95`; three other
  conventions tried, none matching): the function agrees with the system input by input, so
  the number in the spec was formed another way (a correction for the spec writer).
- **When the last bit matters:** the radius is `(float)(18.3 / u)`; the ball is every offset
  whose float sum of squares is at most `r * r`. A radius one unit in the last place away changes
  the result only when an offset's sum of squares lies between the two `r * r` (within the grid's
  reach: an offset beyond `n − 1` voxels along an axis of `n` is never used) or when the radius
  printed with one decimal changes. On a 24 × 24 × 16 series: 11.969702 mm³ (2 × 2 × 2.9924254 mm)
  gives radius 8 with glibc's cube root and 7.9999995 correctly rounded (the ball's 6 points 8
  voxels away: 2109 offsets within the grid against 2103; the default's output differs from
  `reproduce`'s in 6,321 of the 27,648 values, by at most 1.95); 1 × 1 × 1.0421294 mm gives 18.050001
  and 18.05 (printed 18.1 and 18.0, the same ball). Ordinary voxels give the same radius (3 mm,
  2.5 mm, 2 mm, 3.4375 × 3.4375 × 4 mm), and of the voxel sizes with two decimals from 0.5 to
  4 mm (x = y, z up to 3 mm larger) 9,467 give radii one unit apart in the last place and none a
  different ball or printed radius on that grid; of the float voxel volumes in [1, 100) mm³ on a
  64 × 64 × 40 grid, 123 change the ball or the printed radius (43 the printed radius).
- **What the original does:** `epi_radius(steps, mode)` takes `cbrtf` by default and
  `cbrtf_glibc` with `reproduce`; `epi_cbrtf_notice` compares the two radii's balls within the
  grid's reach (`wmi::Ball::x_lines`, the lines the local mean itself uses) and their printed
  values, and names GLIBC-05 in both modes where either differs (not where AFNI-75's guard
  refuses: the radius is then never used).
- **How we know:** *validated*: the parity suite's cases `glibc-05/epi-*` (the ball, its
  one-volume path, the printed radius, radii one unit apart with the same result, ordinary
  voxels) against AFNI 26.3.05: `reproduce` writes AFNI's bytes and messages in every case; the
  default names GLIBC-05 exactly in the first three, and its output and messages (colour codes
  aside) are AFNI's with CORE-MATH's correctly rounded functions preloaded, `cbrtf` among them.
