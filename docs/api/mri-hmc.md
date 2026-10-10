# Head-motion correction (`lx.mri.hmc`)

`lx.mri.hmc` (command line `larmorx mri hmc`; Rust crate `larmorx-mri`): head-motion
correction of a 4D series, compatible with FSL's `mcflirt` (FSL 6.0.7): the same algorithm,
options, output files and, within mcflirt's own variability, the same numbers.

**Clean-room re-implementation.** FSL is under a non-commercial licence, so larmorx did not
port it: the code was written from the behaviour spec [`specs/mcflirt.md`](../../specs/mcflirt.md),
the published papers, `mcflirt -help` and black-box runs of the FSL binary, without FSL's
source code ([provenance](../../crates/larmorx-mri/PROVENANCE.md)). It is Apache-2.0 and is
named by what it does, not after the tool it is compatible with (decision D4).

**Status: `validated`** against mcflirt itself: every recorded oracle run is replayed and
compared ([validation record](../validation/mri-hmc.md)). The matrices differ from mcflirt's
by less than mcflirt differs from itself when its input is perturbed slightly (half an
intensity unit of noise, the volume order reversed, one slice cropped): on fMRIPrep's command
for nine real runs, the 95th percentile of the matrix RMS deviation is
**0.014–0.030 mm** (mcflirt's own band: 0.016–0.10 mm), and on each run it is below
mcflirt's own. The numbers are in the record; the differences are explained under
[Agreement with mcflirt](#agreement-with-mcflirt).

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

## `lx.mri.hmc(bold, *, reference=None, ref_index=None, mean=False, stages=3, final="trilinear", dof=6, cost="normcorr", smooth=1.0, rotation=1.0, bins=256, fudge=False, in_plane="auto", fov=20, init=None, resample=True, n_threads=1)`

Every volume is registered to the reference with a rigid-body transform (or up to 12 degrees
of freedom with `dof`) by mcflirt's schedule: one sweep of line searches on the reference
sampled every 8 mm, then every 4 mm, then 4 mm with an eight times tighter tolerance; each
volume's first search starts from its predecessor's result. The cost is mcflirt's normalised
correlation. The series is then resampled with the transforms.

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

| mcflirt option | Python | Status |
|---|---|---|
| `-in f` (or a bare argument) | `bold` | ✓ |
| `-out f`, `-o f` | (CLI only: output names) | ✓ |
| `-reffile f`, `-r f` | `reference` | ✓ (first volume of a 4D file, with mcflirt's warning) |
| `-refvol n` | `ref_index` | ✓ (`-1`: the default `N/2`) |
| `-meanvol` | `mean=True` | ✓ |
| `-stages n` | `stages` | ✓ (1–4; 4 adds the sinc stage) |
| `-mats`, `-plots`, `-rmsrel`, `-rmsabs` | `matrices`, `params`, `rms_rel`, `rms_abs` (`save_mats`, `save_par`) | ✓ (same files and text formats) |
| `-stats` | (CLI only) | ✓ (`_meanvol`, `_variance`, `_sigma`; end slices 0) |
| `-sinc_final`, `-spline_final`, `-nn_final` | `final="sinc"`, `"spline"`, `"nearest"` | ✓ |
| `-dof n` | `dof` | ✓ (6–12; CLI clamps with mcflirt's message) |
| `-cost c` | `cost` | ✓ `normcorr`; ~ `leastsquares`, `corratio`, `woods`, `mutualinfo`, `normmi` (described in the spec only in outline; not validated) |
| `-bins n` | `bins` | ✓ |
| `-smooth x` | `smooth` | ✓ (0: unweighted, Pearson's correlation) |
| `-rotation x` | `rotation` | ✓ |
| `-fudge` | `fudge=True` | ✓ (also makes the first stage fully parallel) |
| `-2d`, `-fov n` | `in_plane="always"`, `fov` | ✓ (`in_plane="never"` is larmorx's) |
| `-init f` | `init` | ✓ (final resampling only, as mcflirt) |
| `-scaling x`, `-hist`, `-v`, `-verbose n`, `-report` | – | accepted; `-report` prints progress messages (not mcflirt's exact text) |
| `-gdt` | – | ✗ not supported (exit 1) |
| `-help` | – | ✓ (usage, exit 0; alone or without arguments: exit 1, as mcflirt) |

`FSLOUTPUTTYPE` chooses the output format as for mcflirt (`NIFTI_GZ`, `NIFTI`, `NIFTI2_GZ`,
`NIFTI2`, the pair types); the CLI's threads come from `OMP_NUM_THREADS` (default: all).

## Deliberate differences

- Without `FSLOUTPUTTYPE`, larmorx writes `.nii.gz` (mcflirt writes the matrices, then fails).
- Impossible inputs are rejected before any work with exit code 1 and a message: a reference
  index out of range, a reference with another number of voxels than the volumes, `-stages 0`
  with a reference file (mcflirt aborts with signal 6, the last one after the whole
  estimation).
- `-gdt` (registering gradient images; it fails with an absolute `-out` path) is not supported.
- `descrip` in output headers is `larmorx mri hmc`, not FSL's build string.
- The Python API returns the corrected series as float32 on the output grid; the CLI writes
  mcflirt's output types (truncating to integers).

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

On every run larmorx is closer to mcflirt than mcflirt is to itself; FD correlations are low
only for subjects that barely move (FD differences stay below 0.05 mm). On synthetic series
with known motion, larmorx and mcflirt are equally accurate (mean RMS deviation from the
truth 0.409 and 0.414 mm). 49 of the 11 271 matrix files compared are identical to mcflirt's as
text.

Larger differences, all within mcflirt's own band for the same runs: `-meanvol` (the mean
itself depends on the first pass), `-dof 12`, very small images (10 × 10 × 8 voxels) and
synthetic copies of one volume. A linear ramp, which only an x translation can align, leaves
the other parameters unconstrained: both programs find the translation (within 0.3 mm of each
other) and wander differently elsewhere. Woods' cost registers no BOLD run for either
program. The other histogram costs (`corratio`, `mutualinfo`, `normmi`) and `leastsquares`
agree within the thresholds on ds000005, although the spec describes them only in outline.

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
