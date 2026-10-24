# 3dAutomask

`lx.afni.automask` (command line `larmorx afni 3dAutomask`): a brain mask from an EPI series or
a skull-stripped anatomical, as AFNI 25.2.09's `3dAutomask` makes it.

**Replica only, for now.** 3dAutomask's results depend on AFNI library routines copyrighted by
the Medical College of Wisconsin (GPL-2; the routine-by-routine check is in `docs/licensing.md`),
so larmorx needs two implementations: the bit-exact **replica** `larmorx-gpl afni 3dAutomask`
(package `larmorx-gpl`, `crates-gpl/`, GPL-3.0-or-later; translated from AFNI's source) and a
clean-room **original** in the main package, which is still to be written from
[`specs/3dAutomask.md`](../../specs/3dAutomask.md). Until then `larmorx afni 3dAutomask` and
`lx.afni.automask` run the replica, as a separate program, when it is installed
(`pip install "larmorx[exact]"`), and otherwise stop with a message naming that command (exit
status 127; `lx.ReplicaNotFoundError`). `LARMORX_IMPLEMENTATION=original` (or
`implementation="original"`) is an error until the original exists (`NotImplementedError` in
Python).

**Status: `validated`** (the replica) against AFNI 25.2.09 itself: every output of every
compared parity case is AFNI's, bit for bit (data and NIfTI header), with AFNI's exit status and
messages; fMRIPrep's own call on the BOLD reference of every oracle run gives fMRIPrep's own
masks ([validation record](../validation/afni-automask.md)).

## Quick start

```python
import larmorx as lx

# As fMRIPrep calls 3dAutomask on its unifized BOLD reference.
r = lx.afni.automask("uni_xform.nii.gz", dilate=1, apply=True)
r.mask      # uint8, 1 inside the brain
r.masked    # the input, 0 outside the mask
r.mask.implementation   # Implementation(kind='replica', package='larmorx-gpl', ...)
```

```bash
larmorx afni 3dAutomask -apply_prefix uni_xform_masked.nii.gz -dilate 1 \
    -prefix uni_xform_mask.nii.gz uni_xform.nii.gz
```

## `lx.afni.automask(image, *, clfrac=0.5, gradual=True, peels=1, neighbours=17, nn=None, exterior_clip=False, dilate=0, erode=0, si=None, apply=False, depth=False, n_threads=1, implementation="auto", upstream_defects="fix", afni_settings=None)`

Returns an `AutomaskResult`: `mask` (uint8, 3D), `masked` (with `apply=True`; the input's
storage type, its values as `lx.load` reads them) and `depth` (with `depth=True`; int16). Each
image's `implementation` names the replica that ran. `image` is a path (read with AFNI's NIfTI
rules) or an in-memory image (2D, 3D or 4D; uint8, int16 and float32 kept, other types
float32). The arguments are checked before anything runs (`ValueError`); AFNI's variables are
settings of the call (`afni_settings`, as for [`lx.afni.tshift`](afni-tshift.md#afnis-settings-in-python));
the process environment is never read.

### Option mapping

| AFNI option | Python | Notes |
|---|---|---|
| `-prefix p` | the result's `mask` | the command line writes NIfTI only (`.nii`, `.nii.gz`); AFNI's default `automask` is its own format, refused |
| `-apply_prefix p` | `apply=True` → `masked` | alone (no `-prefix`) it is the only output |
| `-depth p` | `depth=True` → `depth` | from the mask before `-dilate`, `-erode` and `-SI` |
| `-clfrac c`, `-mfrac c` | `clfrac` | 0.1 to 0.9 |
| `-nograd` | `gradual=False` | any option starting with `-nogr` |
| `-peels p` | `peels` | 0 turns the peeling off (AFNI-43; AFNI peels once) |
| `-nbhrs n` | `neighbours` | 6 to 26; another value is an error by default (AFNI uses 17, AFNI-41) |
| `-NN1`, `-NN2`, `-NN3` | `nn=1`, `2`, `3` | also set `neighbours` to 6, 18, 26 (AFNI-42) |
| `-eclip` | `exterior_clip=True` | |
| `-dilate nd`, `-erode ne` | `dilate`, `erode` | dilations need 3 neighbours; each is followed by filling holes |
| `-SI hh` | `si` | millimetres, along the superior axis |
| `-q` | (always) | the Python wrapper runs quietly; its warnings become Python warnings |
| `-overwrite`, `-Dname=value`, `-ok_1D_text`, `-skip_afnirc` | `afni_settings` | AFNI's global options, anywhere on the command line |
| `OMP_NUM_THREADS` | `n_threads` | the result does not depend on it (3dAutomask is single-threaded) |
| `LARMORX_UPSTREAM_DEFECTS` | `upstream_defects` | `fix` (default) or `reproduce` |
| `LARMORX_IMPLEMENTATION` | `implementation` | `auto`/`replica` run the replica; `original` is an error for now |

## Upstream defects (D13)

Corrected by default, reproduced with `upstream_defects="reproduce"`
(`LARMORX_UPSTREAM_DEFECTS=reproduce`); each correction that changes a result is named as an
`lx.UpstreamDefectWarning` (on the command line `*+ WARNING: upstream defect <ID> ...`):

- **AFNI-43**: `-peels 0` peels once in AFNI; its help says 0 turns the peeling off, which the
  default does.
- **AFNI-01**: averaging the sub-bricks of a series, AFNI drops a negative `scl_slope` (and its
  magnitude); the default applies it. The mask rarely changes (the clip level scales with the
  data); the clip levels AFNI prints do.
- **AFNI-41** (decided 2026-10-23, rule B: the value makes no sense): `-nbhrs` outside 6..26,
  the range the help gives, is silently replaced in AFNI (the peeling acts as with 17, or as
  `-NN1/2/3` say); the default stops with a fatal error naming AFNI-41 (exit 1), whether or not
  `-NN1/2/3` follow; `reproduce` runs as AFNI does.
- **AFNI-40**: an option without its value as the last argument (`-peels`, `-nbhrs`, `-clfrac`,
  `-SI`, `-dilate`, `-erode`) crashes AFNI; larmorx refuses with an error (exit 1) in both modes.
- The defects of AFNI's dataset reading and writing, shared with 3dTshift (AFNI-02, -07, -11,
  -12, -16, -18, -20, -22, -23, -26; [3dTshift](afni-tshift.md#upstream-defects-d13)). With
  `AFNI_NIFTI_SCALE=NO` (AFNI-23) the default reads integer data with an intercept as AFNI does
  without the variable (float32, `slope * x + inter`).

Kept as AFNI's and documented (decided 2026-10-23, rule C): AFNI-42 (`-NN1/2/3` change the
peeling threshold), AFNI-44 (the large-hole filling depends on the number of slices only; it
shapes fMRIPrep's BOLD masks).

## Deliberate differences

- NIfTI only, input and outputs; AFNI's history extension is not written; AFNI's version banner
  and the compile date after a fatal error are not printed. "CPU time = 0.000000 sec" is
  printed as AFNI 25.2.09 prints it on Linux (its CPU clock reads 0 there).
- An existing output is an error by default (AFNI-11; `-overwrite` and `AFNI_DECONFLICT` as in
  AFNI); any output name is accepted (AFNI-12).
- Where AFNI crashes (AFNI-40), larmorx refuses with one line.

## Known last-bit differences

None: the replica is bit-identical to AFNI on every compared case.

## Rust

The replica's Rust API is in the GPL crate `larmorx-gpl-afni` (`mask::automask_image`,
`automask::{aveabs, apply_mask, dilate_erode, planes_and_si}`, `cli::automask::main`); the
Apache-2.0 crates cannot use it. The main package's Rust API comes with the clean-room original.
