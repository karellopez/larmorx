# FAST (`fast`: tissue-type segmentation)

`lx.fsl.fast` (command line `larmorx fsl fast`): FSL 6.0.7.23's FAST4, the tissue-type
segmentation.

The ports come first, then the program's section, the source of its reference page on the
documentation site (`python -m larmorx_validation reference`), with its options table checked
against the code; then FSL's defects as both ports correct them, the clean room's design and what
is not done yet. The page's dates are in its [History](#history).

## Ports

FSL is distributed under the FSL Licence (non-commercial use only), so `fast` has two ports, a
replica and a clean room, in different packages ([licensing](../licensing.md)):

- **The replica** is bit-exact: `larmorx-nc fsl fast`, in the package `larmorx-nc`
  (`crates-nc/larmorx-nc-fast`), under the FSL Licence's terms, translated from FSL's source.
- **The clean room** is in the main package (crate `larmorx-fsl`, module `fast`, Apache-2.0),
  written from [`specs/fast.md`](../../specs/fast.md), the FAST paper, FSL's documentation and
  black-box runs of FSL's program ([PROVENANCE](../../crates/larmorx-fsl/PROVENANCE.md)). It
  segments one channel and several (any number: `-S 9` and more too, which the replica refuses),
  with the tissue priors (`-a` registered by LARMORX's own FLIRT in this process, `-A`, `-P`).

`implementation="auto"` (the default) runs the replica when `larmorx-nc` is installed, else the
clean room; `"replica"` needs `larmorx-nc` (`pip install larmorx-nc`; otherwise
`ReplicaNotFoundError`); `"clean-room"` (`"original"`, 0.1.0rc1's spelling, still works) runs the
clean room. The command line chooses alike (`--implementation` before the family, or
`LARMORX_IMPLEMENTATION`).

## `fast`

<!-- reference
program: fast
group: fsl
original: FSL 6.0.7.23
python: larmorx.fsl.fast
command-line: crates/larmorx-fsl/src/cli/fast.rs
records: {replica: [fsl-fast-replica], clean-room: [fsl-fast]}
evidence: fsl-fast
-->

`fast` segments a skull-stripped brain image into tissue classes (by default CSF, grey matter and
white matter) with a hidden Markov random field fitted by expectation-maximisation. It also
estimates a bias field and the partial volume of each class in each voxel, on one image or 2 to 8
co-registered channels, with FSL's tissue priors if asked.

### Python (`fast`)

`lx.fsl.fast(images, *, n_classes=3, image_type="T1", bias_iterations=4, bias_lowpass=20.0, no_bias=False, fixed_iterations=4, init_iterations=15, initial_smoothness=0.02, smoothness=0.1, mixel_smoothness=0.3, pve=True, segments=False, probabilities=False, restored=False, bias_field=False, manual_means=None, priors_matrix=None, priors=None, priors_throughout=False, fsldir=None, verbose=False, n_threads=1, implementation="auto", upstream_defects="fix")`

FSL's settings are the call's (CLAUDE.md rule 6): the outputs are NIfTI-1 read back, and no FSL
variable of the process reaches the program. Invalid arguments raise `ValueError` before anything
runs (also the values FAST would use without meaning: FSL-99).

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

### Options of `fast`

| `fast` option | Python | Notes |
|---|---|---|
| the last word(s) | `images` | one image (path or `lx.Image`), or a list of channels, channel 1 first (the command line names channel 1 **last**: `larmorx fsl fast -S 2 -B -o out T2w_brain.nii.gz T1w_brain.nii.gz`) |
| `-S`, `--channels` | `images` | the number of channels (default 1): the list's length |
| `-n`, `--class` | `n_classes` | tissue classes |
| `-t`, `--type` | `image_type` | `"T1"`, `"T2"` (reverses the class order: brightest first), `"PD"`; 1, 2, 3 on the command line |
| `-I`, `--iter` | `bias_iterations` | main-loop iterations with the bias field |
| `-l`, `--lowpass` | `bias_lowpass` | the bias field's smoothing (FWHM, mm) |
| `-N`, `--nobias` | `no_bias=True` | no bias field |
| `-O`, `--fixed` | `fixed_iterations` | main-loop iterations after the bias field (also added to the initialisation's iterations) |
| `-W`, `--init` | `init_iterations` | initialisation iterations |
| `-f`, `--fHard` | `initial_smoothness` | the MRF weight while the bias field is estimated |
| `-H`, `--Hyper` | `smoothness` | the MRF weight afterwards |
| `-R`, `--mixel` | `mixel_smoothness` | the MRF weight of the mixel types (partial volumes) |
| `--nopve` | `pve=False` | no partial volumes |
| `-g`, `--segments` | `segments=True` | one binary image per class |
| `-p` | `probabilities=True` | each class's posterior probability |
| `-B` | `restored=True` | the bias-corrected image(s) |
| `-b` | `bias_field=True` | the bias field(s) |
| `-s`, `--manualseg` | `manual_means` | the initial class intensities: a file, or `n_classes` numbers (several channels: `n_channels` rows of `n_classes`) |
| `-a` | `priors_matrix` | the standard-to-input FLIRT matrix (4 x 4 array or file): initialise from tissue priors registered with it |
| `-A` | `priors` | the CSF, grey- and white-matter priors in standard space (three images); without them FSL's standard priors from `fsldir` |
| `-P`, `--Prior` | `priors_throughout=True` | use the priors in every iteration (needs `priors_matrix`) |
| `-o`, `--out` | the result | the outputs' basename |
| `-v`, `--verbose` | `verbose=True` | FAST's diagnostic messages (in the result's `stdout`) |
| `-h`, `--help` | command line only | |
| `FSLDIR` | `fsldir` | the FSL installation whose `data/standard/tissuepriors` the standard priors come from (needed only for `priors_matrix` without `priors`) |
| `OMP_NUM_THREADS` | `n_threads` | threads (0: all); the results do not depend on it (the clean room's: the same bits on any count) |
| `LARMORX_UPSTREAM_DEFECTS` | `upstream_defects` | `"reproduce"`: FSL's defects as FSL has them |

The command line takes every option of FAST's own (`specs/fast.md` §2), its messages and exit
statuses.

### Examples of `fast`

<!-- inputs: T1w_brain.nii.gz=head -->

```python example
import larmorx as lx

# fMRIPrep's call: fast -I 0 -N -p -g -S 1 <T1w brain>
r = lx.fsl.fast(
    "T1w_brain.nii.gz", bias_iterations=0, no_bias=True, probabilities=True, segments=True
)
len(r.partial_volumes)  # -> 3
```

```bash example
larmorx fsl fast -I 0 -N -p -g -S 1 T1w_brain.nii.gz
```

### Ports of `fast`

**The replica** against FSL 6.0.7.23 itself ([its record](../validation/fsl-fast-replica.md), the
standard comparison): fMRIPrep's call bit-identical in three of fMRIPrep's runs (ds000005, an
adult; ds000228, a 3.5-year-old; ds005003, a glioma), every file FAST writes, its standard output
and error; every option, 2 to 8 channels, the priors, the odd inputs and every message, exit
status and failure case of the command line: 59 of 59 compared cases bit-identical, 27 rejected
by both alike, 1 refused where FSL reads past a smaller channel (FSL-104). With `-a`, FSL's FLIRT
is the replica's own, run in the same process (no FSL installation needed when `-A` names the
priors).

**The clean room** against FSL 6.0.7.23 ([its record](../validation/fsl-fast.md), the standard
comparison, `specs/fast.md` §11's thresholds, 2026-11-05): 64 of 65 compared cases within the
thresholds, fMRIPrep's three calls included (with `reproduce`, as FSL is built with glibc's
functions, fMRIPrep's calls and every single-channel case FSL's values bit for bit, two to eight
channels FSL's segmentations); 35 rejected by both alike (every abort and message of FSL's,
the lines of standard output an abort loses among them), 2 expected divergences (FSL-104's
refusal, and `multi/s2-manualseg-cycle`, below); the one outside, `multi/s5`, is explained (five
channels with nearly singular covariances, decided by the matrix arithmetic's last bits). Its
default is held to the replica's default (the same corrections): with glibc's functions it is the
replica's default bit for bit on every single-channel case, and with its correctly rounded
functions it stays within the narrow band of FSL-202's corrected variance on one channel
(fMRIPrep's calls: the same labels as the replica's default); several channels have no FSL-202,
so there the two defaults lie FSL's own last-bit band apart (`multi/s3`, `multi/s2-n4`: to every
digit FSL against FSL with correctly rounded maths), and `multi/s2-manualseg-cycle`, whose
defaults both correct FSL-98's cycle, is the expected divergence measured in
[the findings](../findings/fsl-fast-clean-room.md). It is built for speed: fMRIPrep's call on the
standard comparison's three T1w brains takes 2.4 to 5.2 s on one thread, 0.4 to 0.9 s on 14 (FSL
22 to 60 s, the replica 15 to 31 s).

## Defects of the original, corrected by default

FAST's defects are corrected by default and reproduced with `upstream_defects="reproduce"`
(`LARMORX_UPSTREAM_DEFECTS=reproduce`); each correction that changes a result is named on standard
error and as an `UpstreamDefectWarning` ([all of them](upstream-defects.md)):

- **FSL-11**: with `FSLOUTPUTTYPE` unset FSL's FAST stops with FSL's message at its first image
  (exit 1). Both ports, the clean room and the replica, write the images as
  `NIFTI_GZ` (`.nii.gz`: the bytes `FSLOUTPUTTYPE=NIFTI_GZ` gives) and name FSL-11 on one line per
  command, in the same words (the replica's line starts `larmorx-nc:`); `reproduce` stops as FSL
  does. Command line only: the Python functions set the type themselves.
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
- **FSL-202**: with one channel, each class's variance subtracts the single-precision square of
  the single-precision mean, which cancels most of its digits in the log domain (up to about
  0.3 % off on white matter): the mean's square in double precision by default (the replica since
  2026-11-03, the clean room since 2026-11-05: with glibc's functions the two defaults
  are the same bits). fMRIPrep's FAST call is affected:
  its default labels move towards those of the exactly computed statistics (ds000005: 54 `_seg`
  and 112 `_mixeltype` voxels); `reproduce` gives FSL's.
- **FSL-203**: an input whose first voxel is NaN has NaN extrema: FAST keeps its negative values
  and, with one channel, its mask is empty (exit 255): the extrema of the values that are not NaN
  by default (the replica since 2026-11-03, the clean room since 2026-11-05).

### glibc's maths functions (GLIBC-07)

FSL's FAST calls glibc 2.35's `exp`, `log`, `expf` and `logf` (the class likelihoods, the Markov
random field, the bias field and the partial volumes; `specs/fast.md` §8), which are not correctly
rounded. The clean room computes them correctly rounded by default and takes glibc's with
`upstream_defects="reproduce"` (`larmorx_core::math`'s `*_glibc` functions, proved bit for bit
against glibc 2.35; the user's decision of 2026-11-04: glibc's bits for `reproduce` only). The
segmentation is an iterative fit: one function value's last bit can move a probability's or the bias
field's last bits, rarely a voxel's class, and FAST cannot tell without segmenting twice, so neither
mode names GLIBC-07: the default's correctly rounded functions can move a segmentation's last bits.

## The clean room

`larmorx --implementation clean-room fsl fast` (`implementation="clean-room"`, and
`"auto"` without `larmorx-nc`): FAST's command line, usage, messages and exit statuses, the
single-channel segmentation (§4: the mask, the log image, the initialisation, the MRF iterations
with FSL's random numbers, the bias field, the class order and every output of §3.3 with the
partial volumes), the multi-channel segmentation (§5: any number of channels, larmorx's own LU
for the covariances), the tissue priors (§6: `-a` with FSL's standard priors or `-A`'s,
registered by larmorx's FLIRT in this process, `-P`), `-v`'s messages, FSL-96 to FSL-104,
FSL-202 and FSL-203 in both modes (with `reproduce`: FAST's estimate of β for a NaN `-H`,
FSL-98's pairing, FSL-101's runs on an empty mask and its aborts, FSL-102's shell splitting,
FSL-104's corner reads and abort, FSL-202's single-precision square of the mean, FSL-203's NaN
extrema; FAST's loops without a class for `-n 0`). Its standard output is written as FAST's C
library buffers it: where FAST aborts, the lines it had not flushed (the k-means lines, the
partial-volume heading) are lost there too (`specs/fast.md` §7). It is compared with FSL by its
own parity suite (`fsl-fast`, the replica suite's cases and its own, `specs/fast.md` §11's
thresholds), and its default with the replica's default; the observations of FSL's behaviour
that the spec does not state are in [the findings](../findings/fsl-fast-clean-room.md). Built for
speed (`specs/fast.md` §10; [the benchmark](../benchmarks/fsl-fast.md)): every per-voxel quantity
is kept for the brain mask's voxels only; the steps where voxels are independent run in parallel
(`n_threads`); the MRF sweeps and the mixel sweep keep FAST's in-place order and run as a pipeline
of planes with exactly the serial result; FSL's blocked sums run block by block, their order
kept; the partial volumes' `exp` runs four values at a time (vectorised, the same bits). The
results are the same bits on any thread count. fMRIPrep's call on the standard comparison's three
T1w brains: 2.4 to 5.2 s on one thread, 0.4 to 0.9 s on 14 (FSL 22 to 60 s, the replica 15 to 31 s).

More than 8 channels: the clean room segments them (rules A-D: FSL segments them too, and the
clean room's thresholds do not need OpenBLAS's bits, so there is nothing to refuse); the replica
refuses them. Channels that are nearly a combination of others (an ill-conditioned covariance,
`multi/s5` of the suite) are decided by the last bits of the matrix arithmetic: there the
clean room lands as far from FSL as from itself with one rounding changed (the findings, "Five
channels").

Its record ([fsl-fast](../validation/fsl-fast.md)), golden cases (`cli.fsl.fast`, `.bias`,
`.multi`, `.priors`, `.reproduce`, `py.fsl.fast`), memory test (fMRIPrep's call at 152 MiB
against FSL's 707) and thread test are in place; its random-testing trial (25 minutes, 9,005
realistic cases) is triaged in [the findings](../findings/fsl-fast-clean-room.md).

## Not yet

- **More than 8 channels** (`-S 9` and more), by the replica: refused in both modes (the
  clean room segments them), with a clear message
  (`larmorx-nc: error: fast: -S 9: more than 8 channels are not replicated ...`, exit 1): FSL's
  results there follow OpenBLAS's kernels for larger matrices (its aligned `iamax` path, the 8-way
  dot products, LAPACK's blocked LU), which larmorx does not reproduce.
- The full tier (fMRIPrep's call on every oracle run), the data panel's other images and long
  random testing: in the final validation phase, for both ports. The standard comparison,
  one random-testing trial each (the replica: half an hour, 17,497 realistic cases, its four
  differences, all FSL-101, fixed in `reproduce` and kept as cases of the record; the clean room:
  25 minutes, 9,005 cases, triaged in its findings) and the site's page (fMRIPrep's call on three
  of fMRIPrep's runs, the bias field, the priors, three channels, every case, both ports) are
  done.
- Two behaviours of the multi-channel segmentation are kept as FAST's until they are measured on
  real T1w and T2w pairs (rule D): its initialisation (each channel's classes started in the order
  of that channel's own intensities, so channels of opposite contrast can start from empty classes)
  and its bias field ([spec](../../specs/fast.md) §9.2).

## History

- **The replica** (2026-11-02): [PROVENANCE](../../crates-nc/larmorx-nc-fast/PROVENANCE.md).
- **The clean room** (2026-11-03); FSL-202 and FSL-203 in both modes since 2026-11-05, when its
  record was made again.
- **Status: `experimental`.**
