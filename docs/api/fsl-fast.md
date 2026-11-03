# FAST (`fast`: tissue-type segmentation)

`lx.fsl.fast` (command line `larmorx fsl fast`): FSL 6.0.7.23's FAST4, the segmentation of a
skull-stripped brain image into tissue classes (by default CSF, grey matter and white matter)
with a hidden Markov random field fitted by expectation-maximisation, a bias field, and the
partial volume of each class in each voxel; on one image or 2 to 8 co-registered channels, with
FSL's tissue priors if asked.

**Two implementations.** FSL is distributed under the FSL Licence (non-commercial use only), so
larmorx keeps its FSL tools in two implementations ([licensing](../licensing.md)): the bit-exact
**replica**, `larmorx-nc fsl fast` (package `larmorx-nc`, `crates-nc/larmorx-nc-fast`, under the
FSL Licence's terms; translated from FSL's source:
[PROVENANCE](../../crates-nc/larmorx-nc-fast/PROVENANCE.md)), done (2026-11-02), and the
clean-room **original** in the main package (crate `larmorx-fsl`, module `fast`, Apache-2.0),
written from [`specs/fast.md`](../../specs/fast.md), the FAST paper, FSL's documentation and
black-box runs of FSL's program ([PROVENANCE](../../crates/larmorx-fsl/PROVENANCE.md)), in
progress since 2026-11-03: it segments one channel and several (any number: `-S 9` and more
too, which the replica refuses), with the tissue priors (`-a` registered by larmorx's own FLIRT in
this process, `-A`, `-P`), every option, message and failure case and FSL-96 to FSL-104 in both
modes (fMRIPrep's call within the thresholds of `specs/fast.md` §11 on its oracle runs).
`implementation="auto"` (the default) runs the replica when `larmorx-nc` is installed,
else the original; `"replica"` needs `larmorx-nc` (`pip install larmorx-nc`; otherwise
`ReplicaNotFoundError`); `"original"` runs the original. The command line chooses alike
(`LARMORX_IMPLEMENTATION`).

**Status: `experimental`**: the replica against FSL 6.0.7.23 itself
([record](../validation/fsl-fast-replica.md), standard tier): fMRIPrep's call bit-identical on
three oracle runs (ds000005, an adult; ds000228, a 3.5-year-old; ds005003, a glioma), every file
FAST writes, its standard output and error; every option, 2 to 8 channels, the priors, the odd
inputs and every message, exit status and failure case of the command line: 59 of 59 compared
cases bit-identical, 26 rejected by both alike, 1 refused where FSL reads past a smaller channel
(FSL-104). The evidence (fMRIPrep's call on three T1w brains in an image viewer, the bias field,
the priors, three channels, every case with its verdict) is FAST's page on the documentation site.

## Quick start

```python
import larmorx as lx

# fMRIPrep's call: fast -I 0 -N -p -g -S 1 <T1w brain>
r = lx.fsl.fast(
    "T1w_brain.nii.gz", bias_iterations=0, no_bias=True, probabilities=True, segments=True
)
csf, gm, wm = r.partial_volumes          # lx.Image each, on the input's grid
labels = r.segmentation.data             # 1 CSF, 2 grey matter, 3 white matter, 0 outside

# The bias field and the corrected image, then a T2w + T1w pair (channel 1 first).
r = lx.fsl.fast("T1w_brain.nii.gz", bias_field=True, restored=True)
r2 = lx.fsl.fast(["T1w_brain.nii.gz", "T2w_brain.nii.gz"], restored=True)
```

From the command line, FAST's own arguments:

```
larmorx fsl fast -I 0 -N -p -g -S 1 T1w_brain.nii.gz
larmorx fsl fast -S 2 -B -o out T2w_brain.nii.gz T1w_brain.nii.gz   # channel 1 is the last word
```

## Options

| `lx.fsl.fast` | `fast` | Default | Meaning |
|---|---|---|---|
| `images` | the last word(s) | | one image (path or `lx.Image`), or a list of channels, channel 1 first (the command line names channel 1 **last**) |
| `n_classes` | `-n` | 3 | tissue classes |
| `image_type` | `-t` | `"T1"` | `"T1"`, `"T2"` (reverses the class order: brightest first), `"PD"` |
| `bias_iterations` | `-I` | 4 | main-loop iterations with the bias field |
| `bias_lowpass` | `-l` | 20 | the bias field's smoothing (FWHM, mm) |
| `no_bias` | `-N` | `False` | no bias field |
| `fixed_iterations` | `-O` | 4 | main-loop iterations after the bias field (also added to the initialisation's iterations) |
| `init_iterations` | `-W` | 15 | initialisation iterations |
| `initial_smoothness` | `-f` | 0.02 | the MRF weight while the bias field is estimated |
| `smoothness` | `-H` | 0.1 | the MRF weight afterwards |
| `mixel_smoothness` | `-R` | 0.3 | the MRF weight of the mixel types (partial volumes) |
| `pve=False` | `--nopve` | `True` | no partial volumes |
| `segments` | `-g` | `False` | one binary image per class |
| `probabilities` | `-p` | `False` | each class's posterior probability |
| `restored` | `-B` | `False` | the bias-corrected image(s) |
| `bias_field` | `-b` | `False` | the bias field(s) |
| `manual_means` | `-s` | | the initial class intensities: a file, or `n_classes` numbers (several channels: `n_channels` rows of `n_classes`) |
| `priors_matrix` | `-a` | | the standard-to-input FLIRT matrix (4 x 4 array or file): initialise from tissue priors registered with it |
| `priors` | `-A` | | the CSF, grey- and white-matter priors in standard space (three images); without them FSL's standard priors from `fsldir` |
| `priors_throughout` | `-P` | `False` | use the priors in every iteration (needs `priors_matrix`) |
| `fsldir` | `FSLDIR` | | the FSL installation whose `data/standard/tissuepriors` the standard priors come from (needed only for `priors_matrix` without `priors`) |
| `verbose` | `-v` | `False` | FAST's diagnostic messages (in `FastResult.stdout`) |
| `n_threads` | `OMP_NUM_THREADS` | 1 | threads of the bias field's convolutions (0: all); the results do not depend on it |
| `implementation` | `LARMORX_IMPLEMENTATION` | `"auto"` | `"auto"`, `"replica"`, `"original"` (the original: any number of channels) |
| `upstream_defects` | `LARMORX_UPSTREAM_DEFECTS` | `"fix"` | `"reproduce"`: FSL's defects as FSL has them (below) |

`lx.fsl.fast` returns a `FastResult`: `segmentation` (`_seg`), `segments` (`_seg_<i>`),
`probabilities` (`_prob_<i>`), `partial_volumes` (`_pve_<i>`), `pve_segmentation` (`_pveseg`),
`mixeltype` (`_mixeltype`), `restored` (`_restore`, `_restore_<c>`), `bias_fields` (`_bias`,
`_bias_<c>`), each an `lx.Image` (or a tuple of them; `None` when not asked for), and `stdout`.
FSL's settings are the call's (CLAUDE.md rule 6): the outputs are NIfTI-1 read back, and no FSL
variable of the process reaches the program. Invalid arguments raise `ValueError` before anything
runs (also the values FAST would use without meaning: see FSL-99).

The command line takes every option of FAST's own (`specs/fast.md` §2), its messages and exit
statuses; with `-a`, FSL's FLIRT is the replica's own, run in the same process (no FSL
installation needed when `-A` names the priors).

## Upstream defects (D13)

FAST's defects are corrected by default and reproduced with `upstream_defects="reproduce"`
(`LARMORX_UPSTREAM_DEFECTS=reproduce`); each correction that changes a result is named on standard
error and as an `UpstreamDefectWarning` ([all of them](upstream-defects.md)):

- **FSL-96**: `-p`'s probability maps outside the brain mask (and everywhere with `--nopve` and
  priors or no initialisation iteration) are memory FAST never writes: larmorx writes 0 (both
  modes) and names it.
- **FSL-97**: several channels with `-N` and `-b`/`-B`: FAST saves a bias field it never computed;
  larmorx writes no field (1, and each channel's image).
- **FSL-98**: several channels: FAST's class sort gives a sorted class another class's statistics
  when the classes move in a cycle; corrected.
- **FSL-99**: a smoothness (`-H`, `-f`, `-R`) that is not a number, a bias smoothing `-l` of 0 or
  less, fewer than one class: refused (FAST uses them as given).
- **FSL-100**: FSLDIR unset: FAST aborts even without priors; larmorx segments (with `-a` and no
  `-A` it asks for the priors' names).
- **FSL-101**: priors FLIRT could not register: FAST says they are not used and then crashes;
  larmorx segments without them.
- **FSL-102**: `-a` runs FLIRT through the shell with the names unquoted; larmorx passes each name
  whole.
- **FSL-103**: the mask leaves out the voxels of the image's maximum at 2^24 and above; corrected.
- **FSL-104**: channels of different sizes are read past their ends; refused.

## The clean-room original

`larmorx fsl fast` with `LARMORX_IMPLEMENTATION=original` (`implementation="original"`, and
`"auto"` without `larmorx-nc`): FAST's command line, usage, messages and exit statuses, the
single-channel segmentation (§4: the mask, the log image, the initialisation, the MRF iterations
with FSL's random numbers, the bias field, the class order and every output of §3.3 with the
partial volumes), the multi-channel segmentation (§5: any number of channels, larmorx's own LU
for the covariances), the tissue priors (§6: `-a` with FSL's standard priors or `-A`'s,
registered by larmorx's FLIRT in this process, `-P`), `-v`'s messages, FSL-96 to FSL-104 in both
modes (with `reproduce`: FAST's estimate of β for a NaN `-H`, FSL-98's pairing, FSL-101's runs
on an empty mask and its aborts, FSL-102's shell splitting, FSL-104's corner reads and abort). It is compared with FSL by its own parity suite (`fsl-fast`,
the replica suite's cases, `specs/fast.md` §11's thresholds); the observations of FSL's behaviour
that the spec does not state are in [the findings](../findings/fsl-fast-original.md). Built for
speed: every per-voxel quantity is kept for the brain mask's voxels only, the steps where voxels
are independent run in parallel (`n_threads`), the MRF sweeps keep FAST's order; fMRIPrep's call
on a 1 mm T1w brain takes about 5 s (FSL about 25 s).

More than 8 channels: the original segments them (rules A-D: FSL segments them too, and the
original's thresholds do not need OpenBLAS's bits, so there is nothing to refuse); the replica
refuses them. Channels that are nearly a combination of others (an ill-conditioned covariance,
`multi/s5` of the suite) are decided by the last bits of the matrix arithmetic: there the
original lands as far from FSL as from itself with one rounding changed (the findings, "Five
channels").

Not in the original yet (its next steps): the speed work (§10), the golden cases, the record,
the benchmark and its random testing.

## Not yet

- **More than 8 channels** (`-S 9` and more) are refused in both modes, with a clear message
  (`larmorx-nc: error: fast: -S 9: more than 8 channels are not replicated ...`, exit 1): FSL's
  results there follow OpenBLAS's kernels for larger matrices (its aligned `iamax` path, the 8-way
  dot products, LAPACK's blocked LU), which larmorx does not reproduce.
- The full tier (fMRIPrep's call on every oracle run), the data panel's other images and long
  random testing: in the final validation phase. The standard tier, one random-testing trial
  (half an hour, 17,497 realistic cases; its four differences, all FSL-101, fixed in `reproduce`
  and kept as cases of the record) and the site's page (fMRIPrep's call on three oracle runs, the
  bias field, the priors, three channels, every case) are done.
- Two behaviours of the multi-channel segmentation are kept as FAST's until they are measured on
  real T1w and T2w pairs (rule D): its initialisation (each channel's classes started in the order
  of that channel's own intensities, so channels of opposite contrast can start from empty classes)
  and its bias field ([spec](../../specs/fast.md) §9.2).
