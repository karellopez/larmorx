# 3dUnifize

`lx.afni.unifize` (command line `larmorx afni 3dUnifize`): the white-matter intensity of a
T1-weighted volume made uniform across space and scaled to about 1000 (or, with `-T2`, of a
T2-weighted one; with `-EPI`, of an EPI series), as AFNI 25.2.09's `3dUnifize` computes it.

**Replica only, for now.** 3dUnifize's results depend on AFNI library routines copyrighted by
the Medical College of Wisconsin (GPL-2: the neighbourhood ball, the sorts and medians, the
automask's conversions; `docs/licensing.md`), so larmorx needs two implementations: the
bit-exact **replica** `larmorx-gpl afni 3dUnifize` (package `larmorx-gpl`, GPL-3.0-or-later,
with a port of glibc 2.35's `tanhf` and `cbrtf`) and a clean-room **original**, still to be
written from [`specs/3dUnifize.md`](../../specs/3dUnifize.md). Until then `larmorx afni
3dUnifize` and `lx.afni.unifize` run the replica when it is installed (`pip install
"larmorx[exact]"`), and otherwise stop with a message naming that command (exit status 127;
`lx.ReplicaNotFoundError`); `implementation="original"` is an error.

**Status: `validated`** (the replica) against AFNI 25.2.09 itself: every output of every
compared parity case is AFNI's, bit for bit, with AFNI's exit status and messages; fMRIPrep's
own call on the BOLD reference of every oracle run gives fMRIPrep's own outputs; AFNI's results
do not depend on its OpenMP thread count ([validation record](../validation/afni-unifize.md)).

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
3dUnifize's ranges).

### Option mapping

| AFNI option | Python | Notes |
|---|---|---|
| `-prefix p` | the result's `image` | NIfTI only; AFNI's default `Unifized` is its own format, refused |
| `-input d`, `-inset d`, or the last argument | `image` | |
| `-T2` | `t2=True` (or 1) | invert the contrast, unifize, invert back |
| `-T2 -T2` | `t2=2` | no inversion back; bright edges clipped |
| `-T2up u` | `t2_percentile` | 90 to 100; near 100 see AFNI-45 |
| `-GM` | `gm=True` | grey-matter scaling (not with `-EPI`) |
| `-Urad R`, `-rbt R b t`, `-param R b t` | `radius`, `percentiles=(b, t)` | 5 ≤ R ≤ 99, 30 ≤ b ≤ 80, b < t ≤ 90 |
| `-clfrac c`, `-mfrac c` | `clfrac` | the automask's clip level fraction (default 0.2; AFNI's help says 0.1, AFNI-49) |
| `-noduplo`, `-nodouble` | `duplo=False` | the half-size copy is only used above a million voxels |
| `-EPI` | `epi=True` | implies `-T2` and `-noduplo`, turns off `-GM`, sets the radius from the voxel size |
| `-nosquash`, `-oink` | `squash=False` | |
| `-ssave s`, `-amsave a` | `save_scale`, `save_automask` | |
| `-quiet`, `-verb` | (always quiet) | warnings become Python warnings |
| `-useall` | – | disabled in AFNI 25.2.09 (a warning) |
| global options, `AFNI_*` | `afni_settings` | as for 3dAutomask |
| `OMP_NUM_THREADS` | `n_threads` | the result does not depend on it, in AFNI or larmorx |

## Upstream defects (D13)

Corrected by default, reproduced with `upstream_defects="reproduce"`:

- **AFNI-45**: with `-T2up` at or near 100 the percentile index falls one past the sorted mask
  values, where AFNI reads whatever memory holds (in its runs 0, which zeroes the output); the
  default inverts about the largest value; `reproduce` uses 0.
- **AFNI-47**: a volume of more than a million voxels with an axis of 2 or 3 voxels: the
  half-size scale image comes back smaller than the input and AFNI reads it past its end (it
  crashed in our runs); the default expands it to the input's grid; `reproduce` reads AFNI's
  image (0 past its end).
- **AFNI-01**: `-EPI`'s median volume drops a negative `scl_slope`, so its sign is wrong and the
  output is all zeros; the default applies the factor.
- **AFNI-40**, **AFNI-46**: `-T2up` without its value as the last argument, and `-T2` on a 2D
  image smaller than 16 × 16, crash AFNI; larmorx refuses (exit 1) in both modes.
- The defects of AFNI's dataset reading and writing (AFNI-02, -07, -11, -12, -16, -18, -20, -22,
  -23, -26), as for 3dAutomask.

Refused by default (decided 2026-10-23, rule B: the input makes no sense): **AFNI-48**, `-T2`
given three or more times. The help documents once (a T2-weighted input, inverted back) and
twice (no inversion back, the bright edges clipped); AFNI's third `-T2` does neither and leaves
the output inverted. The default stops with a fatal error naming AFNI-48 (exit 1) before
reading the input; `reproduce` runs as AFNI does. `-EPI` sets a single `-T2` whatever was
given, so it is not refused.

## Deliberate differences

As for 3dAutomask (NIfTI only, no history extension, no banner; an existing output is an
error by default). The elapsed time printed is the replica's own.

## Known last-bit differences

None: the replica is bit-identical to AFNI on every compared case. A clean-room original that
uses a correctly rounded `tanhf` will differ in the last bit of squashed values (values above
1300) wherever glibc's `tanhf` is not correctly rounded (2.8 % of float arguments).

## Rust

The replica's Rust API is in the GPL crate `larmorx-gpl-afni` (`unifize::{local_percmean,
wm_unifize, gm_unifize, invert_contrast, clip_edges, qmed}`, `cli::unifize::main`); the Apache-2.0
crates cannot use it.
