# 3dUnifize

`lx.afni.unifize` (command line `larmorx afni 3dUnifize`): the white-matter intensity of a
T1-weighted volume made uniform across space and scaled to about 1000 (or, with `-T2`, of a
T2-weighted one; with `-EPI`, of an EPI series), as AFNI's `3dUnifize` computes it (26.3.05;
first 25.2.09).

**AFNI versions** (2026-10-29): both implementations follow AFNI 26.3.05 and are validated against it (the clean-room original since its alignment the same day). The two releases differ, for these programs, in two corrections larmorx already made by default (AFNI-03, header slice code 2; AFNI-05, 3dTshift's `-no_detrend`): AFNI 26.3.05 behaves as larmorx corrected them, so `reproduce` now gives the same result as the default where they apply, and neither is named ([findings](../findings/afni-relicensing-26.md) for the new licence).

**Two implementations, both in `pip install larmorx`.** The bit-exact **replica**, translated
from AFNI's source (`crates/larmorx-afni-replica`; Apache-2.0, with CC BY 4.0's attribution for the code of the Medical College of Wisconsin, which AFNI 26.3.05 releases under CC BY 4.0 ([NOTICE](../../NOTICE), [licensing](../licensing.md)); with fdlibm's `tanhf` as glibc 2.35 runs
it; until 2026-10-29 the separate GPL package `larmorx-gpl`), and the clean-room **original**
(`crates/larmorx-afni`, Apache-2.0), built for speed and written from
[`specs/3dUnifize.md`](../../specs/3dUnifize.md) and black-box runs of AFNI only
([PROVENANCE](../../crates/larmorx-afni/PROVENANCE.md)). `implementation="auto"` (the default, and
`larmorx afni 3dUnifize`) runs the replica, in the same process; `"replica"` or `"original"`
(`LARMORX_IMPLEMENTATION`) forces one.

**`-EPI`'s radius and the cube root's last bit** (2026-10-29). `-EPI` sets its radius from the
voxel volume with the C library's `cbrtf`; AFNI's Linux builds take glibc's, whose code is
LGPL-2.1 and not part of larmorx, and whose result is one unit in the last place from the
correctly rounded one on about one input in ten. The replica uses a correctly rounded `cbrtf`.
For most voxel sizes nothing changes; where one unit in the last place of the cube root would
change the ball of neighbours or the printed radius (measured on every float32 voxel volume: 2,953
of the volumes in [1, 100) mm³), the replica warns that its result may differ from AFNI's (a
warning ending in `(larmorx)`). On those, glibc's last bit changes the ball for 80 volumes and the
printed radius for 43 in [1, 100) mm³ ([findings](../findings/afni-relicensing-26.md), "The C
library's maths"). Each image's `implementation` names the one that ran.

**Decided by the user on 2026-10-30, and done in the clean-room original the same day:** the
default keeps the correctly rounded cube root (`larmorx_core::math::cbrtf`, CORE-MATH's, the more
accurate), and `reproduce` takes a clean-room cube root with glibc 2.35's bits
(`larmorx_core::math::cbrtf_glibc`, written from [`specs/cbrtf.md`](../../specs/cbrtf.md) only and
proved equal to the system's `cbrtf` on all 2^32 float inputs, NaN bits included:
[the proof](../../validation/investigations/cleanroom-cbrtf/README.md)), so the original's
`reproduce` gives AFNI's radius, ball and message on every voxel size. By default the original
names **GLIBC-05** (glibc 2.35's `cbrtf` is not correctly rounded) exactly where glibc's last bit
would have changed the result: the ball of neighbours within the grid's reach (the only offsets
ever used) or the radius printed with one decimal, with the radius each cube root gives and what
differs; `reproduce` names it too ("reproduced", AFNI's radius). The replica's 3dUnifize takes the
same function in a later part (a replica author's); until then it keeps its warning.

**Status: `validated`**, both, against AFNI 26.3.05 itself (the original against AFNI 25.2.09 until 2026-10-29):
- the original ([record](../validation/afni-unifize.md), 129 cases, none failing; GLIBC-05's 5
  since 2026-10-30): AFNI's exit status and messages in every case; in 44 of the 99 compared
  cases that write outputs every output is AFNI's, bit for bit (data and NIfTI header); in the
  other 55 the headers are AFNI's and some squashed values differ in their last bit (at most
  8.7e-8 of the largest value; the threshold is 1e-5), because AFNI's `tanhf` is glibc's and the
  original's is correctly rounded: AFNI run with CORE-MATH's correctly rounded functions preloaded
  gives the original's bytes in all 55. fMRIPrep's own outputs of 22 oracle BOLD references: 1
  bit for bit, the other 21 AFNI's oracle output with the same last-bit differences. Its result
  does not depend on the thread count (nor does AFNI's: 1 and 6 threads, and fMRIPrep's build,
  give the oracle's bytes);
- the replica ([record](../validation/afni-unifize-replica.md)): every compared case, bit for bit.

## Quick start

```python
import larmorx as lx

# As fMRIPrep's BOLD reference workflow calls 3dUnifize (-clfrac 0.2 -rbt 18.3 65.0 90.0 -T2).
uni = lx.afni.unifize("boldref_brain_masked.nii.gz", **lx.afni.fmriprep_unifize_options())
uni.image   # float32, white matter about 1000

# A T1w, with the scale factors.
r = lx.afni.unifize("sub-01_T1w.nii.gz", save_scale=True, n_threads=6)
```

```bash
larmorx afni 3dUnifize -clfrac 0.2 -rbt 18.3 65.0 90.0 -prefix uni.nii.gz -T2 -input ref.nii.gz
```

## `lx.afni.unifize(image, *, t2=False, t2_percentile=98.5, gm=False, radius=18.3, percentiles=(70.0, 80.0), clfrac=0.2, duplo=True, epi=False, squash=True, save_scale=False, save_automask=False, n_threads=1, implementation="auto", upstream_defects="fix", afni_settings=None)`

Returns an `UnifizeResult`: `image` (float32; with `epi=True` every volume), `scale` (with
`save_scale=True`) and `automasked` (with `save_automask=True`). `lx.afni.fmriprep_unifize_options()`
gives fMRIPrep's arguments. The arguments are checked before anything runs (`ValueError`, with
3dUnifize's ranges); with the original, AFNI's fatal errors (no automask) are `ValueError`s too.

### Option mapping

| AFNI option | Python | Notes |
|---|---|---|
| `-prefix p` | the result's `image` | NIfTI only; AFNI's default `Unifized` is its own format, refused |
| `-input d`, `-inset d`, or the last argument | `image` | |
| `-T2` | `t2=True` (or 1) | invert the contrast, unifize, invert back |
| `-T2 -T2` | `t2=2` | no inversion back; bright edges clipped |
| `-T2` three or more times | – | AFNI neither inverts back nor clips edges: refused by default (AFNI-48) |
| `-T2up u` | `t2_percentile` | 90 to 100; near 100 see AFNI-45 |
| `-GM` | `gm=True` | grey-matter scaling (not with `-EPI`) |
| `-Urad R`, `-rbt R b t`, `-param R b t` | `radius`, `percentiles=(b, t)` | 5 ≤ R ≤ 99, 30 ≤ b ≤ 80, b < t ≤ 90 |
| `-clfrac c`, `-mfrac c` | `clfrac` | the automask's clip level fraction (default 0.2; AFNI's help says 0.1, AFNI-49) |
| `-noduplo`, `-nodouble` | `duplo=False` | the half-size copy is only used above a million voxels |
| `-EPI` | `epi=True` | implies `-T2` (once, whatever `-T2` gave) and `-noduplo`, turns off `-GM`, sets the radius from the voxel size |
| `-nosquash`, `-oink` | `squash=False` | |
| `-ssave s`, `-amsave a` | `save_scale`, `save_automask` | |
| `-quiet`, `-verb` | (always quiet) | warnings become Python warnings |
| `-useall` | – | disabled in AFNI 25.2.09 and 26.3.05 (a warning) |
| global options, `AFNI_*` | `afni_settings` | as for 3dAutomask |
| `OMP_NUM_THREADS` | `n_threads` | the result does not depend on it, in AFNI or larmorx |

## Upstream defects (D13)

Corrected by default, reproduced with `upstream_defects="reproduce"`, by both implementations:

- **AFNI-45**: with `-T2up` at or near 100 the percentile index falls one past the sorted mask
  values, where AFNI reads whatever memory holds (in its runs 0, which zeroes the output); the
  default inverts about the largest value; `reproduce` uses 0.
- **AFNI-47**: a volume of more than a million voxels with an axis of 2 or 3 voxels: the
  half-size scale image comes back smaller than the input and AFNI reads it past its end (it
  crashed in our runs); the default expands it to the input's grid; `reproduce` reads AFNI's
  image (0 past its end).
- **AFNI-48** (decided 2026-10-23, rule B): `-T2` three or more times neither inverts back nor
  clips edges in AFNI (the output stays inverted); the default refuses it after the option scan,
  before a dataset given last is read (`-T2 given 3 times: give it once (a T2-weighted input)
  or twice (also clip the bright edges) (AFNI-48; ...)`, exit 1); `reproduce` gives AFNI's
  result and names AFNI-48 after the output. With `-EPI` (which implies one `-T2`) any number
  of `-T2` runs as in AFNI. Both implementations word it the same.
- **AFNI-75** (random testing, 2026-10-25; the replica, and the clean-room original since
  2026-10-30, `specs/3dUnifize.md` §9.1): `-EPI` sets the radius from the voxel size with no
  upper bound (AFNI prints "-EPI changes -Urad to N voxels"); on micrometre voxels N is
  thousands and the ball of neighbours AFNI builds first needs terabytes, so AFNI fills the
  machine until it is stopped. By default both implementations compute that memory, AFNI's
  method's `(4/3·π·r³ + 1)·(24 + 4·w)` bytes for `w` workers (`OMP_NUM_THREADS`, `n_threads`), right
  after the radius line and before any work (a one-volume input too), and when it exceeds the
  limit (a limit given to the computation, `LARMORX_MEMORY_LIMIT`, above 4 GiB the machine's
  physical memory; 1 TiB where it cannot be read) stop at once with the GiB needed, the radius,
  the voxels and the likely cause (micrometre units), naming AFNI-75 (exit 1, nothing written;
  the Python API raises the same message): the two refuse the same calls with the same words.
  Every case that fits is unchanged; `reproduce` attempts it as AFNI does: the replica builds
  the whole ball, the original only the ball's lines that reach the grid (the same values; 3 µm
  voxels on a 24 × 24 × 16 series: 0.01 s, 5 MiB).
- **AFNI-01**: `-EPI`'s median volume drops a negative `scl_slope`, so its sign is wrong and the
  output is all zeros; the default applies the factor.
- **GLIBC-05** (the clean-room original, 2026-10-30; `specs/cbrtf.md`): `-EPI`'s radius is 18.3
  mm over the cube root of the voxel volume, which AFNI takes from glibc 2.35's `cbrtf`, not
  correctly rounded (one unit in the last place on 10.6 % of float inputs). The default uses the
  correctly rounded cube root and names GLIBC-05 where glibc's would have changed the ball of
  neighbours within the grid or the printed radius, for example (`-EPI` on 2 × 2 × 2.9924254 mm
  voxels, 11.969702 mm³, a 24 × 24 × 16 series):
  `upstream defect GLIBC-05 (glibc 2.35) corrected: -EPI's radius is 7.9999995 voxels: 18.3 mm
  over the correctly rounded cube root (2.2875001) of the voxel volume, 11.969702 mm^3; with glibc
  2.35's cbrtf (2.2875), which AFNI calls, it is 8 voxels, a ball of neighbours of 2109 offsets
  within the grid instead of 2103`; on 1 × 1 × 1.0421294 mm voxels the radius printed changes
  (`... it is 18.050001 voxels, printed 18.1 instead of 18.0`). `reproduce` uses glibc's bits
  (`larmorx_core::math::cbrtf_glibc`) and gives AFNI's bytes. Where the two radii differ in their
  last bit but give the same ball and printed radius (2.4 × 2.4 × 2.78 mm: 7.2604246 and
  7.2604237 voxels) nothing is named; on ordinary EPI voxels the two agree (3 mm, 2.5 mm,
  3.4375 × 3.4375 × 4 mm). Of the voxel sizes with two decimals from 0.5 to 4 mm (x = y, z up to
  3 mm larger), 9,467 give radii one unit apart and none a different ball or printed radius on
  a 24 × 24 × 16 grid.
- **AFNI-03 and AFNI-08**: `-EPI`'s output keeps the input's time axis and slice timing: header
  slice code 2 keeps its slice fields in both modes, as AFNI 26.3.05 does, and AFNI-03 is not
  named (AFNI 25.2.09 read every slice time as 0); AFNI-08 is named where the kept times are all
  within 3 ms of 0, in both modes, as in 3dAutomask's masked input.
- **AFNI-40**, **AFNI-46**: `-T2up` without its value as the last argument, and `-T2` on a 2D
  image smaller than 16 × 16, crash AFNI; larmorx refuses (exit 1) in both modes.
- The defects of AFNI's dataset reading and writing (AFNI-02, -07, -11, -12, -16, -18, -20, -22,
  -23, -26), as for 3dAutomask.

## Deliberate differences

As for 3dAutomask (NIfTI only, no history extension, no banner; an existing output is an
error by default). The elapsed time printed is larmorx's own.

## Known last-bit differences

The replica is bit-identical to AFNI on every compared case. The clean-room original's squash
(values above 1300) uses a correctly rounded `tanhf` (`larmorx_core::math::tanhf`, a port of
CORE-MATH's) where AFNI calls glibc 2.35's, which is not correctly rounded on some arguments
(2.8 % of float arguments): squashed values can differ in their last bit. AFNI run with
CORE-MATH's correctly rounded functions preloaded gives the original's bytes
([findings](../findings/afni-masks-original.md)). `-EPI`'s radius uses a correctly rounded
`cbrtf` by default and glibc 2.35's bits with `reproduce` (GLIBC-05, above).

## Speed

The local white-matter intensity (the ball's percentile mean at every voxel) dominates
3dUnifize's time: AFNI gathers and sorts every ball (about 25,000 values at the default radius).
The original keeps the ball's values as counts per rank (each distinct value's position in the
image's sorted values) and slides the ball from voxel to voxel, adding and removing only its two
surfaces; the two percentiles come from block counts and the values between them are read in
ascending order, which gives the spec's float sum exactly. Planes run in parallel, with the same
result at any thread count; the inversion's percentile and `-GM`'s medians are selections, not
sorts. Informal timings (2026-10-24, a shared machine; [findings](../findings/afni-masks-original.md#informal-timings-2026-10-24-not-a-benchmark-report)):
fMRIPrep's call on a 64 × 64 × 34 BOLD reference takes AFNI 93 s at one thread and 22 s at six,
the replica 39 s and 8.7 s, the original 3.8 s and 0.94 s; a T1w (half-size path) 67 s and 24 s,
35 s and 15 s, 9.3 s and 2.7 s. There is no benchmark report yet.

## Rust

The original's Rust API is `larmorx_afni::unifize`: `run(image, options, mode, settings,
n_threads)` on an `AfniImage` with `UnifizeOptions`; its steps `unifize::{wmi, duplo}`,
`invert`, `grey_matter`, `clip_edges`, `median_volume`, `epi_radius(steps, mode)` and
`epi_cbrtf_notice(steps, dims, mode)` (GLIBC-05); the command line is `cli::unifize::main`. The
two cube roots are `larmorx_core::math::cbrtf` (correctly rounded) and
`larmorx_core::math::cbrtf_glibc` (glibc 2.35's bits, clean-room). The replica's Rust API is `larmorx_afni_replica` (`unifize`, with
`epi_radius`; the command line `cli::unifize::main`, and `cli::run_in` for a run inside a
caller's process).
