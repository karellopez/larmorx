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
to 21 throughout), so these are indications, the 6-thread ones especially; a benchmark report on
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
