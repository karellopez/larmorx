# 3dUnifize

`lx.afni.unifize` (command line `larmorx afni 3dUnifize`): the white-matter intensity of a
T1-weighted volume made uniform across space and scaled to about 1000 (or, with `-T2`, of a
T2-weighted one; with `-EPI`, of an EPI series), as AFNI's `3dUnifize` computes it (26.3.05;
25.2.09 for the clean-room original until it is aligned, below).

**AFNI versions** (2026-10-29): the replica follows AFNI 26.3.05 and is validated against it; the clean-room original is validated against AFNI 25.2.09 until a clean-room agent aligns it with 26.3.05. The two releases differ, for these programs, in two corrections larmorx already made by default (AFNI-03, header slice code 2; AFNI-05, 3dTshift's `-no_detrend`): the default output is the same with either; only `reproduce` differs where they apply ([findings](../findings/afni-relicensing-26.md) for the new licence).

**Two implementations.** 3dUnifize's results depend on AFNI library routines copyrighted by the
Medical College of Wisconsin (GPL-2), so larmorx has the bit-exact **replica**
`larmorx-gpl afni 3dUnifize` (package `larmorx-gpl`, GPL-3.0-or-later, with a port of glibc
2.35's `tanhf` and `cbrtf`) and the clean-room **original** in the main package
(`crates/larmorx-afni`, Apache-2.0), written from [`specs/3dUnifize.md`](../../specs/3dUnifize.md)
and black-box runs of AFNI only ([PROVENANCE](../../crates/larmorx-afni/PROVENANCE.md)).
`implementation="auto"` (and `larmorx afni 3dUnifize`) runs the replica, as a separate program,
when it is installed (`pip install "larmorx[exact]"`), else the original; `"replica"` or
`"original"` (`LARMORX_IMPLEMENTATION`) forces one. Each image's `implementation` names the one
that ran.

**Status: `validated`**, both: the original against AFNI 25.2.09 itself, the replica against AFNI 26.3.05:
- the original ([record](../validation/afni-unifize.md), 99 cases, none failing): AFNI's exit
  status and messages in every case; in 32 of the 72 compared cases that write outputs every
  output is AFNI's, bit for bit (data and NIfTI header); in the other 40 the headers are AFNI's
  and some squashed values differ in their last bit (at most 1,633 values of a T1w, at most
  8.7e-8 of the largest value; the threshold is 1e-5), because AFNI's `tanhf` is glibc's and the
  original's is correctly rounded: AFNI run with CORE-MATH's correctly rounded functions preloaded
  gives the original's bytes in all 40. fMRIPrep's own outputs of the 7 oracle BOLD references:
  1 bit for bit, the other 6 AFNI's oracle output with the same last-bit differences. Its result
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
- **AFNI-75** (random testing, 2026-10-25; the GPL replica, the clean-room original to follow):
  `-EPI` sets the radius from the voxel size with no upper bound (AFNI prints "-EPI changes
  -Urad to N voxels"); on micrometre voxels N is thousands and the ball of neighbours AFNI
  builds first needs terabytes, so AFNI fills the machine until it is stopped. By default the
  replica computes that memory before building the ball and, when it exceeds the machine's
  physical memory (or `LARMORX_MEMORY_LIMIT`; 1 TiB where the machine's memory cannot be read),
  stops at once with the GiB needed and the likely
  cause, naming AFNI-75 (exit 1); every case that fits is unchanged; `reproduce` attempts it
  as AFNI does.
- **AFNI-01**: `-EPI`'s median volume drops a negative `scl_slope`, so its sign is wrong and the
  output is all zeros; the default applies the factor.
- **AFNI-03 and AFNI-08**: `-EPI`'s output keeps the input's time axis and slice timing: header
  slice code 2 keeps its corrected slice fields by default and is named in both modes; AFNI-08
  is named where the kept times are all within 3 ms of 0 (in `reproduce`, every code-2 input),
  as in 3dAutomask's masked input (since 2026-10-29 in the original, in the replica's words).
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
([findings](../findings/afni-masks-original.md)). Likewise `-EPI`'s radius uses a correctly
rounded `cbrtf`.

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
`invert`, `grey_matter`, `clip_edges`, `median_volume`; the command line is
`cli::unifize::main`. The replica's Rust API is in the GPL crate `larmorx-gpl-afni`; the
Apache-2.0 crates cannot use it.
