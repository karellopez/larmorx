# Head-motion correction (`lx.mri.hmc`)

`lx.mri.hmc` (command line `larmorx mri hmc`, or `larmorx fsl mcflirt`; Rust crate
`larmorx-mri`): head-motion correction of a 4D series, compatible with FSL's `mcflirt` (FSL
6.0.7.23 since 2026-10-29; first FSL 6.0.7.17): the same algorithm, options, output files and,
within mcflirt's own variability, the same numbers. **FSL versions:** both implementations
follow mcflirt 6.0.7.23 and are validated against it; the clean-room original follows its
`-sinc_final` and `-nn_final` (FSL-59), its zstd and bzip2 output types, its reading of files by
content and its names (FSL-60), and computes `-cost mutualinfo` and `normmi` exactly as the
spec's appendix B describes them (2026-10-29).

**Two implementations** (`implementation="auto" | "replica" | "original"`): the clean-room
original described on most of this page (Apache-2.0, in `larmorx`), and the bit-exact
**replica** in the separately licensed `larmorx-nc` package (FSL Licence: non-commercial use
only), which gives mcflirt's output files byte for byte: 137 of the 137 recorded mcflirt runs,
with the same messages and exit status where both fail (12 of them), fMRIPrep's own mcflirt
calls on the 12 oracle runs with BOLD runs (22 of 22, the data panel's included; fMRIPrep's
4,517 matrices are identical too) and 35 edge cases: 173 of 173 compared cases bit-identical,
21 rejected alike, against FSL 6.0.7.17 ([full tier](../validation/fsl-mcflirt-replica-full.md));
since the version update of 2026-10-29 it follows FSL 6.0.7.23: 176 of 176 compared cases
bit-identical, 22 rejected alike, 5 expected divergences in the last bits of 6.0.7.23's LAPACK
inverses ([standard tier](../validation/fsl-mcflirt-replica.md))
([below](#the-bit-exact-replica-larmorx-nc)). `"auto"`, the default, runs the replica when
`larmorx-nc` is installed.

**Clean-room re-implementation.** FSL is under a non-commercial licence, so larmorx did not
port it: the code was written from the behaviour spec [`specs/mcflirt.md`](../../specs/mcflirt.md),
the published papers, `mcflirt -help` and black-box runs of the FSL binary, without FSL's
source code ([provenance](../../crates/larmorx-mri/PROVENANCE.md)). It is Apache-2.0 and is
named by what it does, not after the tool it is compatible with (decision D4).

**Status: `experimental`** until the one case below that misses its threshold is resolved
(CLAUDE.md rule 8: no equivalence claim while a parity case fails). It is compared with
mcflirt itself: every recorded oracle run is replayed and compared ([validation record](../validation/mri-hmc.md)). On fMRIPrep's command the matrices
differ from mcflirt's by less than mcflirt differs from itself when its input is perturbed
slightly (half an intensity unit of noise, the volume order reversed, one slice cropped): for
nine real runs, the 95th percentile of the matrix RMS deviation is **0.014–0.030 mm**
(mcflirt's own band: 0.016–0.10 mm), and on each run it is below mcflirt's own. The numbers are in the record; the differences are explained under
[Agreement with mcflirt](#agreement-with-mcflirt).

One of the 122 compared cases misses its threshold, by a hair: with mcflirt's default
reference (the middle volume) on ds003345, the framewise displacement correlates with
mcflirt's at r = 0.9885, where mcflirt against itself reaches 0.9888–0.9903 (three noise
realisations and the cropped slice). The matrices of that run are at the edge of the band:
the 95th percentile of the RMS deviation is 0.028 mm, against 0.025–0.026 mm for the noise
realisations and 0.029 mm for the cropped slice (the threshold is 0.1 mm). The largest FD
difference, 0.085 mm, is at a volume where the noise realisations move FD by 0.01–0.04 mm.

## Quick start

```python
import larmorx as lx

# fMRIPrep's head-motion step: the HMC reference, the matrices only.
hmc = lx.mri.hmc("sub-01_task-rest_bold.nii.gz", reference="hmc_boldref.nii.gz",
                 resample=False, n_threads=8)
hmc.matrices          # (N, 4, 4) FSL matrices, as mcflirt's MAT_* files
hmc.affines           # (N, 4, 4) RAS transforms, reference -> volume (nitransforms)
hmc.save_itk("mat2itk.txt")       # fMRIPrep's ITKLinearTransformArray (LPS)
hmc.fd                # Power's framewise displacement (radius 50 mm)

# Everything mcflirt writes
res = lx.mri.hmc("bold.nii.gz")   # reference: volume N // 2
res.image.save("bold_mcf.nii.gz")
res.save_mats("bold_mcf.mat"); res.save_par("bold_mcf.par")

# One-shot resampling with the motion (fMRIPrep's way)
lx.transforms.resample_series(..., transforms=[res.affine_series, ...])
```

The same from a shell, with mcflirt's arguments:

```bash
larmorx mri hmc -in sub-01_task-rest_bold.nii.gz -out bold_mcf.nii.gz \
    -reffile hmc_boldref.nii.gz -mats -plots -rmsrel -rmsabs
```

## `lx.mri.hmc(bold, *, reference=None, ref_index=None, mean=False, stages=3, final="trilinear", dof=6, cost="normcorr", smooth=1.0, rotation=1.0, bins=256, fudge=False, in_plane="auto", fov=20, init=None, resample=True, n_threads=1, upstream_defects="fix", normcorr_count="mcflirt", implementation="auto")`

Every volume is registered to the reference with a rigid-body transform (or up to 12 degrees
of freedom with `dof`) by mcflirt's schedule: one sweep of line searches on the reference
sampled every 8 mm, then every 4 mm, then 4 mm with an eight times tighter tolerance; each
volume's first search starts from its predecessor's result. The cost is mcflirt's normalised
correlation. The series is then resampled with the transforms. mcflirt's defects are corrected
by default; `upstream_defects="reproduce"` gives mcflirt's matrices
([below](#upstream-defects-d13)).

`bold` is a path (read with mcflirt's rules, below) or an in-memory image (`lx.Image`,
nibabel image, `(array, affine)`); `reference` likewise. Returns an `HmcResult`:

| Field | Meaning |
|---|---|
| `matrices` | `(N, 4, 4)`: mcflirt's `.mat` convention: a volume's FSL-mm coordinates (voxel index × voxel size, x reversed for images whose affine has a positive determinant) to the reference's |
| `affines` | `(N, 4, 4)`: the same motion in RAS world coordinates, reference points → the volume's points (nitransforms' `LinearTransformsMapping`, as fMRIPrep stores head motion); `affine_series` wraps them as `lx.transforms.AffineSeries` |
| `itk` | `(N, 4, 4)`: `affines` in LPS (ITK, ANTs); `save_itk(path)` writes them as fMRIPrep's `mat2itk.txt` |
| `params` | `(N, 6)`: `rx ry rz` (radians) `tx ty tz` (mm) about the reference's intensity-weighted centre, as `mcflirt -plots`; `save_par(path)` |
| `rms_abs`, `rms_rel` | mcflirt's RMS displacements (radius 80 mm) from the reference and from the previous volume |
| `fd` | Power's framewise displacement from `params` (radius 50 mm; 0 for the first volume) |
| `reference_index` | the reference volume of the series, or `None` |
| `in_plane` | whether the in-plane mode for thin slabs was used |
| `image` | the corrected series (float32 `lx.Image`), or `None` with `resample=False` |
| `mean` | the mean volume of `mean=True` |
| `implementation` | which implementation computed it: `lx.Implementation("original", "larmorx", ...)` or the replica's (`"replica"`, `"larmorx-nc"`, its version, `"LicenseRef-FSL"`, its program) |

`lx.mri.read_mats(directory)` reads an mcflirt-style `.mat` directory into an `(N, 4, 4)` array.

**Reading.** As mcflirt: values become float32 (`scl_slope`/`scl_inter` applied when they ask
for it), voxel sizes come from `pixdim`, and images whose sform (or qform) has a positive
determinant have their x axis reversed in memory, so the matrices refer to "radiological"
order. Obliquity does not matter: the estimation works in voxel coordinates. The world
transforms (`affines`, `itk`) undo all of this.

**Threads.** `n_threads` (0 = all logical CPUs). The first stage is a chain within each sweep,
so its volumes run one after another with each cost evaluation's slices in parallel; the later
stages run the volumes in parallel. The result does not depend on the thread count.

## Option mapping

| mcflirt option | Python | Clean-room original | Replica (`larmorx-nc`) |
|---|---|---|---|
| `-in f` (or a bare argument) | `bold` | ✓ | ✓ |
| `-out f`, `-o f` | (CLI only: output names) | ✓ | ✓ |
| `-reffile f`, `-r f` | `reference` | ✓ (first volume of a 4D file, with mcflirt's warning) | ✓ |
| `-refvol n` | `ref_index` | ✓ (`-1`: the default `N/2`) | ✓ |
| `-meanvol` | `mean=True` | ✓ | ✓ |
| `-stages n` | `stages` | ✓ (1–4; 4 adds the sinc stage) | ✓ |
| `-mats`, `-plots`, `-rmsrel`, `-rmsabs` | `matrices`, `params`, `rms_rel`, `rms_abs` (`save_mats`, `save_par`) | ✓ (same files and text formats) | ✓ (Python returns the files' values: matrices to 6 decimals, parameters and RMS to 6 significant digits) |
| `-stats` | (CLI only) | ✓ (`_meanvol`, `_variance`, `_sigma`; every slice, named after `-out` without its extension; mcflirt's end slices at 0 and names with `reproduce`, FSL-08) | ✓ (the same corrections) |
| `-sinc_final`, `-spline_final`, `-nn_final` | `final="sinc"`, `"spline"`, `"nearest"` | ✓ | ✓ |
| `-dof n` | `dof` | ✓ (6–12; CLI clamps with mcflirt's message) | ✓ |
| `-cost c` | `cost` | ✓ `normcorr`, `mutualinfo`, `normmi` (exactly as the spec's appendix B: mcflirt's evaluated matrices, evaluation by evaluation); ~ `leastsquares`, `corratio`, `woods` (described in the spec only in outline; not validated) | ✓ every cost, bit-identical |
| `-bins n` | `bins` | ✓ | ✓ (below 8 with a histogram cost: refused by default, FSL-21) |
| `-smooth x` | `smooth` | ✓ (0: unweighted, Pearson's correlation) | ✓ |
| `-rotation x` | `rotation` | ✓ | ✓ |
| `-fudge` | `fudge=True` | ✓ (also makes the first stage fully parallel) | ✓ (the same) |
| `-2d`, `-fov n` | `in_plane="always"`, `fov` | ✓ (`in_plane="never"` is larmorx's) | ✓ (`in_plane="never"`: the original runs under `"auto"`) |
| `-init f` | `init` | ✓ (final resampling only, as mcflirt) | ✓ |
| `-scaling x`, `-hist`, `-v`, `-verbose n`, `-report` | – | accepted; `-scaling` and `-hist` have no effect (in mcflirt either; a warning says so, FSL-05); `-report` prints progress messages (not mcflirt's exact text) | ✓ mcflirt's messages to the byte (`-verbose 20` prints every evaluated matrix) |
| `-gdt` | – | ✗ not supported (exit 1) | refused by default; `reproduce` registers the gradient images as mcflirt does (FSL-04) |
| `-help` | – | ✓ (usage, exit 0; without arguments: exit 1. `-help` alone exits 0; mcflirt's exit 1 with `reproduce`, FSL-07) | ✓ (the same) |
| (`LARMORX_UPSTREAM_DEFECTS=fix\|reproduce`) | `upstream_defects="fix"` | mcflirt's defects corrected (default) or reproduced ([below](#upstream-defects-d13)) | the same corrections; `reproduce` gives mcflirt's bytes |
| – | `normcorr_count="mcflirt"` | `"weights"`: the corrected correlation count (FSL-02), opt-in ([below](#the-correlation-count-fsl-02)) | ✗ (mcflirt's count; the original runs under `"auto"`) |
| – | `implementation` | `"original"` | `"replica"`; `"auto"` (default) when `larmorx-nc` is installed |

`FSLOUTPUTTYPE` chooses the output format as for mcflirt 6.0.7.23 (`NIFTI`, `NIFTI2`, the pair
types `NIFTI_PAIR` and `NIFTI2_PAIR`, each also `_GZ`, `_ZST` and `_BZ2`: gzip, zstd and bzip2,
pure Rust); an unknown value prints FSL's message (`ERROR:: Unrecognised value (…) of environment
variable FSLOUTPUTTYPE` and the legal values) and exits 1 before any work. Inputs are read by
their content (gzip, zstd, bzip2 or uncompressed), whatever their names, and a name without an
image extension is completed as FSL completes it (`.nii.gz`, `.nii`, `.hdr`, `.hdr.gz`,
`.nii.zst`, `.hdr.zst`, `.nii.bz2`, `.hdr.bz2`; two matches or none are FSL's errors), also by
the Python API. The CLI's threads come from `OMP_NUM_THREADS` (default: all).

## The bit-exact replica (`larmorx-nc`)

The replica is a translation of mcflirt's source (fsl-mcflirt 2111.0 and the libraries it
uses, FSL 6.0.7.23 since 2026-10-29; first FSL 6.0.7.17) in the `larmorx-nc` package (`crates-nc/`), under the FSL Licence's terms:
**non-commercial use only**, FSL's original source included ([licensing](../licensing.md)).
It is installed by name, so that its licence is accepted knowingly:

```bash
pip install larmorx-nc        # not on PyPI yet: build it from crates-nc/ (crates-nc/README.md)
```

larmorx never imports it: `lx.mri.hmc` and `larmorx fsl mcflirt` run its program
(`larmorx-nc fsl mcflirt <mcflirt's arguments>`, found through `LARMORX_NC_BIN`, the Python
environment's scripts directory or `PATH`) on files and read back what it writes. It gives
mcflirt's output files byte for byte, its messages and its exit statuses, on every platform:
it carries the C library's float functions that FSL's Linux build calls (glibc 2.35's `sinf`,
`cosf`, `sincosf`, `logf`, `atanf`, `atan2f`, each bit-identical to the C library on every input
checked; [findings](../findings/fsl-mcflirt.md#the-replica-larmorx-nc-2026-10-24)).
The record: [validation/fsl-mcflirt-replica.md](../validation/fsl-mcflirt-replica.md); the
random tests' differences of 2026-10-25 were all one constant of `atanf` (fixed) and a missing
FSL-08 notice for the `-meanvol` image's name (added).

Its defaults are the clean-room original's: mcflirt's deterministic defects are corrected
(below), and `upstream_defects="reproduce"` gives mcflirt's bytes. Where mcflirt aborts (an
uncaught exception, or glibc's allocator after FSL-21's out-of-bounds write) the replica prints
what mcflirt prints and one line of its own, and exits with status 1. Its own threads
(`n_threads`, `OMP_NUM_THREADS`) run the volumes of the unchained stages and the final
resampling in parallel; the output does not depend on them. `descrip` in its images is
`larmorx-nc fsl mcflirt`.

Speed (informal: the evidence's runs on a shared machine, fMRIPrep's command on ds000005, 240
volumes, writing the corrected series too): 20 s on 3 threads and 36 s on one; mcflirt took
70 s in the same set of runs (31 s on a quiet machine, [the benchmark](../benchmarks/mri-hmc.md)).

## Deliberate differences

- Without `FSLOUTPUTTYPE`, larmorx writes `.nii.gz` (mcflirt writes the matrices, then fails).
  With an unknown value, larmorx prints mcflirt's message and exits 1 before any work (mcflirt
  prints it after the estimation and the `.mat` and `.par` files). Compressed outputs hold the
  same decompressed bytes as mcflirt's would, compressed by larmorx's own encoders (zstd in one
  frame per piece, bzip2 in one stream).
- Impossible inputs are rejected before any work with exit code 1 and a message: a reference
  index out of range, a reference with another number of voxels than the volumes, `-stages 0`
  with a reference file (mcflirt aborts with signal 6, the last one after the whole
  estimation).
- `-gdt` (registering gradient images; it fails with an absolute `-out` path) is not supported.
- `descrip` in output headers is `larmorx mri hmc`, not FSL's build string.
- The Python API returns the corrected series as float32 on the output grid; the CLI writes
  mcflirt's output types (int8 kept as int8, integers rounded to the nearest and clamped;
  mcflirt's uint8 and truncation with `reproduce`, FSL-09).

## Upstream defects (D13)

larmorx corrects mcflirt's deterministic defects by default and reproduces them with
`upstream_defects="reproduce"` (command line: `LARMORX_UPSTREAM_DEFECTS=reproduce`); a warning
(Python) or a line on standard error (command line) names each one that changed the result,
in both modes ([upstream-defects.md](upstream-defects.md); register entries in
[findings/upstream-defects.md](../findings/upstream-defects.md)):

| ID | mcflirt 6.0.7 | larmorx by default | Where |
|---|---|---|---|
| FSL-01 | the last volume of the series starts the first stage from the identity, not from its predecessor's result | it continues its chain like every other volume | every run where the last volume follows another in the first stage: fMRIPrep's command (a separate reference) on every run |
| FSL-03 | the in-plane mode pads the first stage's reference grid twice | once, like the other stages | thin slabs, `-2d` |
| FSL-14 | one NaN or infinite voxel makes the volume's centre of mass, and so its matrix, NaN; the corrected volume is then the background value | non-finite voxels are left out of the centre of mass and of the cost (points whose reference or interpolated value is not finite do not count) | volumes or references with NaN or infinite voxels |
| FSL-05 | `-scaling`, `-hist` accepted and ignored | still ignored, with a warning | command line |
| FSL-06 | an unknown `-cost` silently uses normcorr | an error, exit 255 | command line |
| FSL-07 | numbers read with C's `atoi`/`atof` (`-refvol abc` is 0, `-stages 1.5` is 1); `-help` alone exits 1 | anything but a number is an error (exit 255); `-help` alone exits 0 | command line |
| FSL-08 | the `-stats` images leave the first and last slices at 0, and the `-stats` and `-meanvol` images are named after the `-out` string with its extension inside (`x.nii.gz_meanvol.nii.gz`) | every slice; `x_meanvol.nii.gz`, `x_mean_reg.nii.gz` (the names nipype expects) | `-stats`, `-meanvol` |
| FSL-09 | int8 input is written as uint8; integer outputs are truncated toward zero, and values beyond the type's range wrap around (x86-64; observed) | int8 kept; rounded to the nearest (half to even) and clamped | integer outputs of the command line |
| FSL-10 | the output's units are always millimetres and seconds: a TR in milliseconds is labelled seconds | the input's units are kept (millimetres and seconds where it gives none) | inputs in other units (ds003345: milliseconds) |
| FSL-13 | `-rmsrel` and `-rmsabs` create the `.mat` directory without `-mats` (holding only the reference's identity) | no `.mat` directory without `-mats` | command line |
| FSL-04 | `-gdt` registers each volume's gradient image to the reference's plain image (the reference's gradient is computed and discarded) | refused (the clean-room original does not support `-gdt`); the replica reproduces it with `reproduce` | `-gdt` |
| FSL-20 | when the `.mat` directory cannot be created for another reason than its permissions (a missing parent directory), mcflirt prints "Error!: Unknown Log directory return value" and exits with status 0, writing nothing more | an error (exit 1) | the replica's command line |
| FSL-21 | with a histogram cost (`corratio`, `mutualinfo`, `normmi`, `woods`) and `-bins` below 8, the first stage's histogram has no bins: the matrices are arbitrary, and `corratio` writes before its arrays and aborts | an error before any work (exit 1) | the replica's command line |
| FSL-59 | FSL 6.0.7.23 only: `-sinc_final` and `-nn_final` resample with trilinear interpolation, whatever the option asks (the `.par` and `.rms` follow) | the interpolation asked for (FSL 6.0.7.17's result); `reproduce` trilinear; named where the two series differ | `-sinc_final`, `-nn_final`, `final="sinc"`, `"nearest"` |
| FSL-60 | FSL 6.0.7.23 only: an image name holding `.img` before its extension (a directory, the file's stem) is opened with its last `.img` made `.hdr`: another file is read or written, or mcflirt aborts (a pair's header renamed, its data file not) | the name given; `reproduce` opens FSL's name; named in both modes | the command line's `-in`, `-reffile`, `-out` and the images named after it (the Python API opens the name given) |

On fMRIPrep's command (`-reffile <boldref> -mats`) FSL-01 changes the last volume's matrix
(the others are mcflirt's), and FSL-09 the rounding of the corrected series when the input is
integer (fMRIPrep uses only the matrices). The parity record's "D13 fix pass" lists, run by
run, what the corrections change.

Where the last volume is close to its predecessor the two starts reach nearly the same matrix:
on the 52 real parity runs a run's largest change is 0.024 mm (median), and on fMRIPrep's
command for ds000005 the last volume moves by 0.06 mm, after which it is 0.021 mm from its
predecessor instead of 0.055 mm (the other volumes' steps have a median of 0.038 mm). The
largest real change, 72 mm, is a `-cost woods` run where mcflirt and larmorx both fail on every
volume (rotations near 90°); corrected, the last volume stays 0.44 mm from its predecessor
instead of 72 mm. Where the motion is large, mcflirt's start from the identity can fail on its
own: on the synthetic `background` series (large translations over a non-zero background)
mcflirt's last volume converges to a 180° rotation, 106 mm from the true motion, and the
corrected one to 0.36 mm
([validation/mri-hmc-normcorr-count.md](../validation/mri-hmc-normcorr-count.md), ground
truth).

### The correlation count (FSL-02)

mcflirt's normalised correlation divides by a count `N` that is a running sum of running sums
of the edge weights (`specs/mcflirt.md` §6.4): many orders of magnitude larger than the number
of points, which makes the correlation effectively uncentred. `normcorr_count="weights"` divides
by the sum of the weights instead, the weighted Pearson correlation. It is opt-in until the
user decides (D13 item 4): its measurement against mcflirt's variability band and the synthetic
ground truth is in [validation/mri-hmc-normcorr-count.md](../validation/mri-hmc-normcorr-count.md).
In short, the corrected count moves the matrices by about mcflirt's own variability (0.014 to
0.033 mm at the 95th percentile on 19 real runs, where mcflirt's band is 0.016 to 0.10 mm) and
is not more accurate on the synthetic ground truth (better on 5 of 8 series, worse on 3).
It cannot be combined with `upstream_defects="reproduce"`, and the command line has no option
for it.

## Agreement with mcflirt

The implementation follows mcflirt's algorithm to the float32 operation where the spec
and black-box runs describe it: the same reference grids, sample positions, edge weights,
count `N`, correlation, line searches, initialisation chain and output formats. mcflirt's
`-verbose 20` traces (every evaluated matrix) were used to check the search path evaluation by
evaluation: the evaluated matrices of each search start out identical, and black-box runs
settled what the spec left open (the rotations' sines and cosines are rounded to float32; the
square roots are float32; the sums are in exactly three levels).

The cost values still differ from mcflirt's in the last float32 bit for some evaluations once
the test volume is interpolated (with test volumes of constant intensity, the first
parabolic step of 20 of 20 volumes is identical; with real data, of 140 of 240). The searches
then part ways by up to about one tolerance of the last stage (0.0005 rad, 0.02 mm), which is
the scale of mcflirt's own sensitivity to its input:

| Comparison (fMRIPrep's command, nine real runs) | 95th pct of the RMS deviation | FD correlation |
|---|---|---|
| larmorx against mcflirt | 0.014–0.030 mm | 0.854–0.999 |
| mcflirt against itself, input perturbed slightly (the band) | 0.016–0.10 mm | 0.794–0.998 |

With fMRIPrep's command, larmorx is closer to mcflirt than mcflirt is to itself on every run
(with mcflirt's default reference, see the one marginal case above); FD correlations are low
only for subjects that barely move (FD differences stay below 0.05 mm). On synthetic series
with known motion, larmorx and mcflirt are equally accurate (mean RMS deviation from the
truth 0.409 and 0.414 mm). 49 of the 11 271 matrix files compared are identical to mcflirt's as
text.

Larger differences, all within mcflirt's own band for the same runs: `-meanvol` (the mean
itself depends on the first pass), `-dof 12`, very small images (10 × 10 × 8 voxels) and
synthetic copies of one volume. A linear ramp, which only an x translation can align, leaves
the other parameters unconstrained: both programs find the translation (within 0.3 mm of each
other) and wander differently elsewhere. Woods' cost registers no BOLD run for either
program. `corratio` and `leastsquares` agree within the thresholds on ds000005, although the
spec describes them only in outline; `mutualinfo` and `normmi` follow the spec's appendix B
exactly since 2026-10-29.

**Random phantoms** (2026-10-29, [findings](../findings/mri-hmc-original.md)): on the 782
random phantoms where random testing found the estimates beyond the thresholds (small images
moved by 5° to 30°: a cost with several minima), the corrected series is as well aligned as
mcflirt's for every cost (temporal standard deviation, median ratio 1.000). `cost="normmi"`
aligned worse until the exact histogram costs (median ratio 1.051 against mcflirt 6.0.7.23 on
91 phantoms, now 1.000, 55 of them within 1 %; `mutualinfo` 0.983, now 1.000, 54 of 78 within
1 %). With `-fudge -stages 1`, mcflirt's evaluated matrices are larmorx's, evaluation by
evaluation, on 22 of 30 phantoms; the others part after 19 to 224 evaluations in the last
decimals, as with `normcorr`.

**Better algorithms (decision D11).** None is the default: the uncentred correlation, the
single sweep and the tolerance quantisation decide mcflirt's numbers, and changing them moves
the results beyond mcflirt's band. `fudge=True` (mcflirt's `-fudge`) starts every volume's
first stage from the identity, which makes that stage parallel too; on ds000005 it moves the
matrices by 0.05 mm (median) from the default (spec §14), at the edge of the band, so it stays
an option. `smooth=0` gives Pearson's correlation (mcflirt's `-smooth 0`).

## Performance

On fMRIPrep's command ([benchmark](../benchmarks/mri-hmc.md), 6-core laptop): 1.2–2.3× faster
than mcflirt on one thread, 3.6–7.1× on 4 and 4.9–9.4× on 12, writing everything mcflirt
writes; estimating only (`resample=False`, all fMRIPrep uses), 5.7–16× on 12 threads. A
240-volume run takes 5.4 s instead of 31 s. The first stage is a chain (each volume starts
from its predecessor's result), so its cost evaluations are parallel over grid slices; the
later stages are parallel over volumes. `-stages 4` (sinc in the cost) is the slow option:
110 s on 12 threads for ds000005 (mcflirt: 531 s).
