# 3dAutomask

`lx.afni.automask` (command line `larmorx afni 3dAutomask`): a brain mask from an EPI series or
a skull-stripped anatomical, as AFNI 25.2.09's `3dAutomask` makes it.

**Two implementations.** 3dAutomask's results depend on AFNI library routines copyrighted by the
Medical College of Wisconsin (GPL-2), so larmorx has the bit-exact **replica**
`larmorx-gpl afni 3dAutomask` (package `larmorx-gpl`, `crates-gpl/`, GPL-3.0-or-later; translated
from AFNI's source) and the clean-room **original** in the main package (`crates/larmorx-afni`,
Apache-2.0), written from [`specs/3dAutomask.md`](../../specs/3dAutomask.md) and black-box runs of
AFNI only ([PROVENANCE](../../crates/larmorx-afni/PROVENANCE.md)). `implementation="auto"` (and
`larmorx afni 3dAutomask`) runs the replica, as a separate program, when it is installed
(`pip install "larmorx[exact]"`), else the original; `implementation="replica"` or
`"original"` (`LARMORX_IMPLEMENTATION`) forces one. Each image's `implementation` names the one
that ran.

**Status: `validated`**, both, against AFNI 25.2.09 itself:
- the original ([record](../validation/afni-automask.md)): every output of 80 of the 81 compared
  cases that write one is AFNI's, bit for bit (data and NIfTI header), with AFNI's exit status and
  messages in every case; the other one, fMRIPrep's call on ds005454's BOLD reference, has AFNI's
  data and a header that differs only where AFNI reads its own header extension of the input
  (AFNI-10, below); fMRIPrep's own masks of all 7 oracle BOLD references, bit for bit;
- the replica ([record](../validation/afni-automask-replica.md)): every compared case, bit for bit.

## Quick start

```python
import larmorx as lx

# As fMRIPrep calls 3dAutomask on its unifized BOLD reference.
r = lx.afni.automask("uni_xform.nii.gz", dilate=1, apply=True)
r.mask      # uint8, 1 inside the brain
r.masked    # the input, 0 outside the mask
r.mask.implementation   # Implementation(kind='original', ...) without larmorx-gpl
```

```bash
larmorx afni 3dAutomask -apply_prefix uni_xform_masked.nii.gz -dilate 1 \
    -prefix uni_xform_mask.nii.gz uni_xform.nii.gz
```

## `lx.afni.automask(image, *, clfrac=0.5, gradual=True, peels=1, neighbours=17, nn=None, exterior_clip=False, dilate=0, erode=0, si=None, apply=False, depth=False, n_threads=1, implementation="auto", upstream_defects="fix", afni_settings=None)`

Returns an `AutomaskResult`: `mask` (uint8, 3D; 2D for a plane), `masked` (with `apply=True`;
the input's storage type, its values as `lx.load` reads them) and `depth` (with `depth=True`;
int16). `image` is a path (read with AFNI's NIfTI rules) or an in-memory image (2D, 3D or 4D;
uint8, int16 and float32 kept, other types float32). The arguments are checked before anything
runs (`ValueError`); with the original, AFNI's fatal errors (no mask of a plane smaller than
16 × 16, an empty mask) are `ValueError`s too. AFNI's variables are settings of the call
(`afni_settings`, as for [`lx.afni.tshift`](afni-tshift.md#afnis-settings-in-python)); the
process environment is never read.

### Option mapping

| AFNI option | Python | Notes |
|---|---|---|
| `-prefix p` | the result's `mask` | the command line writes NIfTI only (`.nii`, `.nii.gz`); AFNI's default `automask` is its own format, refused |
| `-apply_prefix p` | `apply=True` → `masked` | alone (no `-prefix`) it is the only output |
| `-depth p` | `depth=True` → `depth` | from the mask before `-dilate`, `-erode` and `-SI` |
| `-clfrac c`, `-mfrac c` | `clfrac` | 0.1 to 0.9 |
| `-nograd` | `gradual=False` | any option starting with `-nogr` |
| `-peels p` | `peels` | 0 turns the peeling off (AFNI-43; AFNI peels once) |
| `-nbhrs n` | `neighbours` | 6 to 26 (19 to 26 act as 18); outside it an error by default (AFNI-41) |
| `-NN1`, `-NN2`, `-NN3` | `nn=1`, `2`, `3` | also set `neighbours` to 6, 18, 26 (AFNI-42, kept) |
| `-eclip` | `exterior_clip=True` | |
| `-dilate nd`, `-erode ne` | `dilate`, `erode` | dilations need 3 neighbours; each is followed by filling holes |
| `-SI hh` | `si` | millimetres, along the superior axis |
| `-q` | (always) | the Python wrapper runs quietly; its warnings become Python warnings |
| `-overwrite`, `-Dname=value`, `-ok_1D_text`, `-skip_afnirc` | `afni_settings` | AFNI's global options, anywhere on the command line |
| `OMP_NUM_THREADS` | `n_threads` | the result does not depend on it (AFNI's 3dAutomask is single-threaded; the original runs its passes in parallel) |
| `LARMORX_UPSTREAM_DEFECTS` | `upstream_defects` | `fix` (default) or `reproduce` |
| `LARMORX_IMPLEMENTATION` | `implementation` | `auto`, `replica` or `original` |

## Upstream defects (D13)

Corrected by default, reproduced with `upstream_defects="reproduce"`
(`LARMORX_UPSTREAM_DEFECTS=reproduce`), by both implementations; each correction that changes a
result is named as an `lx.UpstreamDefectWarning` (on the command line `*+ WARNING: upstream
defect <ID> ...`):

- **AFNI-43**: `-peels 0` peels once in AFNI; its help says 0 turns the peeling off, which the
  default does (named when the mask changes).
- **AFNI-41** (decided 2026-10-23, rule B): `-nbhrs` outside 6..26 is silently 17 in AFNI; the
  default refuses it (exit 1, `ValueError`); `reproduce` uses 17.
- **AFNI-01**: averaging the sub-bricks of a series, AFNI drops a negative `scl_slope` (and its
  magnitude); the default applies it. The mask rarely changes (the clip level scales with the
  data); the clip levels AFNI prints do.
- **AFNI-40**: an option without its value as the last argument (`-peels`, `-nbhrs`, `-clfrac`,
  `-SI`, `-dilate`, `-erode`) crashes AFNI; larmorx refuses with an error (exit 1) in both modes,
  after AFNI's messages.
- The defects of AFNI's dataset reading and writing, shared with 3dTshift (AFNI-02, -07, -11,
  -12, -16, -18, -20, -22, -23, -26; [3dTshift](afni-tshift.md#upstream-defects-d13)). With
  `AFNI_NIFTI_SCALE=NO` (AFNI-23) the default reads integer data with an intercept as AFNI does
  without the variable (float32, `slope * x + inter`).

Kept as AFNI's and documented (decided 2026-10-23, rule C): **AFNI-42** (`-NN1/2/3` set the peel
threshold to 6, 18 and 26, overriding `-nbhrs`: an explicit `-NN2` peels with 18, the default
with 17) and **AFNI-44** (the large-hole filling runs or not depending on the number of slices:
it shapes fMRIPrep's BOLD masks).

## Deliberate differences

- NIfTI only, input and outputs; AFNI's history extension is not written; AFNI's version banner
  and the compile date after a fatal error are not printed. "CPU time = 0.000000 sec" is
  printed as AFNI 25.2.09 prints it on Linux (its CPU clock reads 0 there).
- An existing output is an error by default (AFNI-11; `-overwrite` and `AFNI_DECONFLICT` as in
  AFNI); any output name is accepted (AFNI-12).
- Where AFNI crashes (AFNI-40), larmorx refuses with one line.
- The original ignores AFNI's own header extension in an AFNI-written input (AFNI-10, as
  3dTshift's original does); AFNI takes the geometry from its attributes (7 significant digits),
  so on such an input (fMRIPrep's 3dAutomask reads 3dUnifize's output) the output header's
  sform can differ from AFNI's in the 7th digit (ds005454: voxel sizes 1.40000034 against the
  header's 1.39999998 mm, the origin by up to 3.8e-5 mm). The data are AFNI's; AFNI run on the
  input without its extension writes the original's header byte for byte
  ([findings](../findings/afni-masks-original.md)). The replica reproduces AFNI's reading.

## Known last-bit differences

None in the data: both implementations are bit-identical to AFNI on every compared case (the
method is integer and comparison arithmetic but the average and the clip levels, whose float
steps the spec states exactly).

## Speed

The original decides every pass of the mask operations voxel by voxel in parallel over z planes
(the result does not depend on the thread count), finds clusters by run-based union-find, and
replaces two of AFNI's repeated sweeps (the exterior clip, the depth) by breadth-first searches
with the same results ([findings](../findings/afni-masks-original.md), "Speed"). AFNI's
3dAutomask is single-threaded; there is no benchmark report yet.

## Rust

The original's Rust API is `larmorx_afni::automask`: `run(image, options, mode, settings,
n_threads)` on an `AfniImage` (`larmorx_afni::dataset::read_as`), with `AutomaskOptions`; the mask
of a float image is `form_mask(v, dims, settings, log)` (3dUnifize uses it), its steps in
`automask::{clip, cluster, morph, program}`; the command line is `cli::automask::main`. The
replica's Rust API is in the GPL crate `larmorx-gpl-afni`; the Apache-2.0 crates cannot use it.
