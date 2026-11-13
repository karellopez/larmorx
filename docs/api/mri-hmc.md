# Head-motion correction (`mcflirt`, `lx.mri.hmc`)

`lx.mri.hmc` (command line `larmorx fsl mcflirt`, or `larmorx mri hmc`; Rust crate
`larmorx-mri`): head-motion correction of a 4D series, compatible with FSL's `mcflirt` (FSL
6.0.7.23 since 2026-10-29; first FSL 6.0.7.17): the same algorithm, options, output files and,
within mcflirt's own variability, the same numbers.

The ports come first, then the program's section, the source of its reference page on the
documentation site (`python -m larmorx_validation reference`), with its options table checked
against the code; then the replica, the differences by design, mcflirt's defects as both ports
correct them, the clean room's agreement with mcflirt and its speed. The page's dates are in its
[History](#history).

## Ports

FSL is distributed under the FSL Licence (non-commercial use only), so `mcflirt` has two ports, a
replica and a clean room, in different packages ([licensing](../licensing.md)):

- **The replica** is bit-exact: `larmorx-nc fsl mcflirt`, in the package `larmorx-nc`
  (`crates-nc/`), under the FSL Licence's terms, translated from mcflirt's source (fsl-mcflirt
  2111.0 and the libraries it uses). It gives mcflirt's output files byte for byte
  ([The replica](#the-replica)).
- **The clean room** is in the main package (Apache-2.0, crate `larmorx-mri`), written from the
  behaviour spec [`specs/mcflirt.md`](../../specs/mcflirt.md), the published papers, `mcflirt
  -help` and black-box runs of FSL's program, without FSL's source
  ([PROVENANCE](../../crates/larmorx-mri/PROVENANCE.md)). It is named by what it does, `mri
  hmc`, not after the program it is compatible with (decision D4).

Both follow mcflirt 6.0.7.23 and are compared with it; the clean room follows its `-sinc_final`
and `-nn_final` (FSL-59), its zstd and bzip2 output types, its reading of files by content and
its names (FSL-60), and computes `-cost mutualinfo` and `normmi` exactly as the spec's appendix B
describes them (2026-10-29).

`implementation="auto"` (the default) runs the replica when `larmorx-nc` is installed, else the
clean room; `"replica"` or `"clean-room"` (`"original"`, 0.1.0rc1's spelling, still works) force
one. The command line chooses alike (`--implementation` before the family, or
`LARMORX_IMPLEMENTATION`).

## `mcflirt`

<!-- reference
program: mcflirt
group: fsl
original: FSL 6.0.7.23
command: larmorx fsl mcflirt
python: larmorx.mri.hmc
command-line: crates/larmorx-mri/src/cli/hmc.rs
records: {replica: [fsl-mcflirt-replica, fsl-mcflirt-replica-full], clean-room: [mri-hmc]}
evidence: mri-hmc
-->

`mcflirt` corrects head motion in a 4D series. It registers every volume to the reference with a rigid-body transform (or up to 12 degrees
of freedom with `dof`) by mcflirt's schedule: one sweep of line searches on the reference
sampled every 8 mm, then every 4 mm, then 4 mm with an eight times tighter tolerance; each
volume's first search starts from its predecessor's result. The cost is mcflirt's normalised
correlation. It then resamples the series with the transforms. mcflirt's defects are corrected
by default; `upstream_defects="reproduce"` gives mcflirt's matrices
([Defects of the original](#defects-of-the-original-corrected-by-default)).

### Python (`mcflirt`)

`lx.mri.hmc(bold, *, reference=None, ref_index=None, mean=False, stages=3, final="trilinear", dof=6, cost="normcorr", smooth=1.0, rotation=1.0, bins=256, fudge=False, in_plane="auto", fov=20, init=None, resample=True, n_threads=1, upstream_defects="fix", normcorr_count="mcflirt", implementation="auto")`

`bold` is a path (read with mcflirt's rules, below) or an in-memory image (`lx.Image`, nibabel
image, `(array, affine)`); `reference` likewise. The result, an `HmcResult`, also writes
mcflirt's files: `save_mats(directory)` and `save_par(path)`, and `save_itk(path)` writes `itk`
as fMRIPrep's `mat2itk.txt`; `affine_series` wraps `affines` as `lx.transforms.AffineSeries`. Its
`implementation` is `lx.Implementation("clean-room", "larmorx", ...)` or the replica's
(`"replica"`, `"larmorx-nc"`, its version, `"LicenseRef-FSL"`, its program).
`lx.mri.read_mats(directory)` reads an mcflirt-style `.mat` directory into an `(N, 4, 4)` array.

**Reading.** As mcflirt: values become float32 (`scl_slope`/`scl_inter` applied when they ask
for it), voxel sizes come from `pixdim`, and images whose sform (or qform) has a positive
determinant have their x axis reversed in memory, so the matrices refer to "radiological"
order. Obliquity does not matter: the estimation works in voxel coordinates. The world
transforms (`affines`, `itk`) undo all of this.

**Threads.** `n_threads` (0 = all logical CPUs). The first stage is a chain within each sweep,
so its volumes run one after another with each cost evaluation's slices in parallel; the later
stages run the volumes in parallel. The result does not depend on the thread count.

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

### Options of `mcflirt`

| `mcflirt` option | Python | Notes |
|---|---|---|
| `-in` (or a bare argument) | `bold` | the series |
| `-out`, `-o` | command line only | the outputs' names |
| `-reffile`, `-r` | `reference` | a separate reference volume: the first volume of a 4D file, with mcflirt's warning |
| `-refvol` | `ref_index` | `-1`: the default `N/2` |
| `-meanvol` | `mean=True` | |
| `-stages` | `stages` | 1 to 4; 4 adds the sinc stage |
| `-mats`, `-plots`, `-rmsrel`, `-rmsabs` | the result: `HmcResult.matrices`, `.params`, `.rms_rel`, `.rms_abs` | the same files and text formats; through the replica, Python returns the files' values (matrices to 6 decimals, parameters and RMS to 6 significant digits) |
| `-stats` | command line only | `_meanvol`, `_variance`, `_sigma`; every slice, named after `-out` without its extension; mcflirt's end slices at 0 and names with `reproduce` (FSL-08), in both ports |
| `-sinc_final`, `-spline_final`, `-nn_final` | `final="sinc"`, `"spline"`, `"nearest"` | |
| `-dof` | `dof` | 6 to 12; the command line clamps with mcflirt's message |
| `-cost` | `cost` | the clean room: `normcorr` (its count's running total takes each point's weight as the point is visited, since 2026-11-07), `mutualinfo`, `normmi` (exactly as the spec's appendix B: mcflirt's evaluated matrices, evaluation by evaluation); `corratio`, `woods` (FLIRT's, since 2026-11-02), `leastsquares` (FLIRT's `leastsq`: three single-precision levels, since 2026-11-07); the replica: every cost, bit-identical |
| `-bins` | `bins` | below 8 with a histogram cost: refused by default (FSL-21) |
| `-smooth` | `smooth` | 0: unweighted, Pearson's correlation |
| `-rotation` | `rotation` | |
| `-fudge` | `fudge=True` | also makes the first stage fully parallel, in both ports |
| `-2d`, `-fov` | `in_plane="always"`, `fov` | `in_plane="never"` is LARMORX's own: under `"auto"` the clean room runs it |
| `-init` | `init` | final resampling only, as mcflirt |
| `-scaling`, `-hist`, `-v`, `-verbose`, `-report` | command line only | accepted; `-scaling` and `-hist` have no effect (in mcflirt either; a warning says so, FSL-05); the clean room's `-report` prints progress messages (not mcflirt's exact text); the replica prints mcflirt's messages to the byte (`-verbose 20` prints every evaluated matrix) |
| `-gdt` | command line only | not supported by the clean room (exit 1); the replica refuses it by default, and with `reproduce` registers the gradient images as mcflirt does (FSL-04) |
| `-help` | command line only | the usage, exit 0; without arguments: exit 1 (`-help` alone exits 0; mcflirt's exit 1 with `reproduce`, FSL-07); the same in both ports |
| (none: mcflirt always resamples) | `resample=False` | the matrices only, as fMRIPrep needs; it saves no time with the replica |
| (none) | `normcorr_count="mcflirt"` | `"weights"`: the corrected correlation count (FSL-02), opt-in ([The correlation count](#the-correlation-count-fsl-02)); the clean room's only (the replica counts as mcflirt; under `"auto"` the clean room runs it) |
| `FSLOUTPUTTYPE` | command line only | the output format ([Output types and file names](#output-types-and-file-names)) |
| `LARMORX_UPSTREAM_DEFECTS` | `upstream_defects` | mcflirt's defects corrected (default) or reproduced; the replica's `reproduce` gives mcflirt's bytes |

### Output types and file names

`FSLOUTPUTTYPE` chooses the output format as for mcflirt 6.0.7.23 (`NIFTI`, `NIFTI2`, the pair
types `NIFTI_PAIR` and `NIFTI2_PAIR`, each also `_GZ`, `_ZST` and `_BZ2`: gzip, zstd and bzip2,
pure Rust); an unknown value prints FSL's message (`ERROR:: Unrecognised value (…) of environment
variable FSLOUTPUTTYPE` and the legal values) and exits 1 before any work. Inputs are read by
their content (gzip, zstd, bzip2 or uncompressed), whatever their names, and a name without an
image extension is completed as FSL completes it (`.nii.gz`, `.nii`, `.hdr`, `.hdr.gz`,
`.nii.zst`, `.hdr.zst`, `.nii.bz2`, `.hdr.bz2`; two matches or none are FSL's errors), also by
the Python API. The CLI's threads come from `OMP_NUM_THREADS` (default: all).

### Examples of `mcflirt`

<!-- inputs: bold.nii.gz=head4d ref.nii.gz=head -->

```python example
import larmorx as lx

# fMRIPrep's way: a separate reference, the matrices only.
hmc = lx.mri.hmc("bold.nii.gz", reference="ref.nii.gz", resample=False)
hmc.matrices.shape  # -> (4, 4, 4)
hmc.image is None  # -> True
```

```bash example
larmorx fsl mcflirt -in bold.nii.gz -reffile ref.nii.gz -mats -plots -rmsrel -rmsabs
larmorx mri hmc -in bold.nii.gz -out bold_mcf -meanvol
```

### Ports of `mcflirt`

**The replica** against mcflirt 6.0.7.23 itself ([its record](../validation/fsl-mcflirt-replica.md),
the standard comparison, 2026-11-02): 205 of 205 compared cases bit-identical (every file, the
exit status and both streams, the replica in `reproduce` mode), 32 where both end with the same
error. Against FSL 6.0.7.17 before the version update ([its full record](../validation/fsl-mcflirt-replica-full.md),
2026-10-25): 137 of the 137 recorded mcflirt runs, with the same messages and exit status where
both fail (12 of them), fMRIPrep's own mcflirt calls on the 12 fMRIPrep runs with BOLD series (22
of 22, the data panel's included; fMRIPrep's 4,517 matrices are identical too) and 35 edge cases:
173 of 173 compared cases bit-identical, 21 rejected alike.

**The clean room** against mcflirt 6.0.7.23 ([its record](../validation/mri-hmc.md), the
standard comparison, 2026-11-11): every recorded run of mcflirt replayed and compared, 170
cases: 145 within the thresholds, 21 rejected by both alike, 4 expected divergences (each with
its reason in the record), 0 failures. On fMRIPrep's command the matrices differ from mcflirt's
by less than mcflirt differs from itself when its input is perturbed slightly (half an intensity
unit of noise, the volume order reversed): for nine real runs, the 95th percentile of the matrix
RMS deviation is **0.0151–0.0274 mm** (mcflirt's own band: 0.0164–0.1014 mm), on each run at or
below mcflirt's own. On every run with `-mats` (116) the worst 95th percentile is 0.1186 mm (the
thresholds: 0.1 mm, or mcflirt's band where wider, as on 26 runs). The differences are explained
under [Agreement with mcflirt](#agreement-with-mcflirt).

## The replica

The replica is a translation of mcflirt's source (fsl-mcflirt 2111.0 and the libraries it
uses, FSL 6.0.7.23 since 2026-10-29; first FSL 6.0.7.17) in the `larmorx-nc` package (`crates-nc/`), under the FSL Licence's terms:
**non-commercial use only**, FSL's original source included ([licensing](../licensing.md)).
It is installed by name, so that its licence is accepted knowingly:

```bash
pip install larmorx-nc
```

(on PyPI from 0.1.0rc1; or `pip install "larmorx[all]"`, with everything).

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

Its defaults are the clean room's: mcflirt's deterministic defects are corrected
(below), and `upstream_defects="reproduce"` gives mcflirt's bytes. Where mcflirt aborts (an
uncaught exception, or glibc's allocator after FSL-21's out-of-bounds write) the replica prints
what mcflirt prints and one line of its own, and exits with status 1. Its own threads
(`n_threads`, `OMP_NUM_THREADS`) run the volumes of the unchained stages and the final
resampling in parallel; the output does not depend on them. `descrip` in its images is
`larmorx-nc fsl mcflirt`.

Speed (informal: the evidence's runs on a shared machine, fMRIPrep's command on ds000005, 240
volumes, writing the corrected series too): 20 s on 3 threads and 36 s on one; mcflirt took
70 s in the same set of runs (31 s on a quiet machine, [the benchmark](../benchmarks/mri-hmc.md)).


## Differences by design

- Without `FSLOUTPUTTYPE`, larmorx writes `.nii.gz` and names FSL-11, the clean room and the
  replica alike (mcflirt writes the matrices, then fails; `reproduce` does so too).
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


## Defects of the original, corrected by default

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
| FSL-11 | `FSLOUTPUTTYPE` unset: mcflirt writes the matrices and the `.par` file, then stops at its first image with FSL's message (exit 1; with `-meanvol` its mean image comes first: nothing written) | the images as `NIFTI_GZ` (`.nii.gz`), FSL-11 named on one line, in the same words by both implementations; `reproduce` stops where mcflirt does | the command line without `FSLOUTPUTTYPE` |
| FSL-10 | the output's units are always millimetres and seconds: a TR in milliseconds is labelled seconds | the input's units are kept (millimetres and seconds where it gives none) | inputs in other units (ds003345: milliseconds) |
| FSL-13 | `-rmsrel` and `-rmsabs` create the `.mat` directory without `-mats` (holding only the reference's identity) | no `.mat` directory without `-mats` | command line |
| FSL-04 | `-gdt` registers each volume's gradient image to the reference's plain image (the reference's gradient is computed and discarded) | refused (the clean room does not support `-gdt`); the replica reproduces it with `reproduce` | `-gdt` |
| FSL-20 | when the `.mat` directory cannot be created for another reason than its permissions (a parent directory that does not exist or is a file), mcflirt prints "Error!: Unknown Log directory return value" and exits with status 0, writing nothing more | an error (exit 1) before any output of the step, nothing written; `reproduce`: mcflirt's message and exit status 0, nothing more written (since 2026-10-30) | the command line's `-mats`, `-rmsrel`, `-rmsabs` |
| FSL-21 | with a histogram cost (`corratio`, `mutualinfo`, `normmi`, `woods`) and `-bins` below 8, the first stage's histogram has no bins: the matrices are arbitrary, and `corratio` writes before its arrays and aborts | an error before any work (exit 1; the Python API raises it), with `-stages` 1 or more, whatever the series; `reproduce`: the zero-bin costs as mcflirt computes them (`specs/mcflirt.md` §12.1), `corratio` an error where mcflirt aborts (since 2026-10-30) | `bins` below 8 with a histogram cost |
| FSL-65 | an output text file that cannot be opened (its name a directory, its directory missing or not writable): `-plots`' `.par` file ends the run at volume 0's line with "error: unable to open output file!" and status 0, after `MAT_0000` and the `.rms` files; an `.rms` file is silently not written | an error naming the file (exit 1) before any output of the step, nothing written; `reproduce`: mcflirt's files, message and exit status 0 (since 2026-10-30) | the command line's `-plots`, `-rmsrel`, `-rmsabs` |
| FSL-66 | a negative `-bins`: each stage has `-bins / 8` or `-bins / 4` bins (toward zero), prepared for every cost: −1 bins kill mcflirt with SIGFPE, −2 or fewer abort it (`std::bad_array_new_length`), nothing written | an error before any work (exit 1), with `-stages` 1 or more, every cost; `reproduce`: an error saying how mcflirt ends where it crashes, and mcflirt's run where no stage gets fewer than 0 bins (−3 to −1 with `normcorr` and `leastsquares`) (since 2026-10-30) | negative `bins` |
| FSL-59 | FSL 6.0.7.23 only: `-sinc_final` and `-nn_final` resample with trilinear interpolation, whatever the option asks (the `.par` and `.rms` follow) | the interpolation asked for (FSL 6.0.7.17's result); `reproduce` trilinear; named where the two series differ (with an integer output, FSL-09 is then named in both modes, its counts from mcflirt's trilinear series) | `-sinc_final`, `-nn_final`, `final="sinc"`, `"nearest"` |
| FSL-60 | FSL 6.0.7.23 only: an image name holding `.img` before its extension (a directory, the file's stem) is opened with its last `.img` made `.hdr`: another file is read or written, or mcflirt aborts (a pair's header renamed, its data file not) | the name given; `reproduce` opens FSL's name; named in both modes | the command line's `-in`, `-reffile`, `-out` and the images named after it (the Python API opens the name given) |

### Outputs that cannot be written, and bins no histogram can have (since 2026-10-30)

`specs/mcflirt.md` §10.6 and §12.1. The order of the writes is mcflirt's in both modes: the
`-meanvol` and `-stats` images, the `refnum` lines (`-report`), then the text outputs (the `.par`
file opened, the `.mat` directory made, the `.rms` files, then volume by volume its `MAT_` file
and `.par` line), the corrected series last. By default every text output is opened (the `.mat`
directory made) before anything is written, and one that cannot be is an error naming the file
and the defect (FSL-20, FSL-65; what this step made is removed); a `.mat` directory whose parent
refuses the permission is an error too (exit 1, not FSL-20's case). With `reproduce` the run is
mcflirt's: an uncreatable `.mat` directory ends it with mcflirt's message and exit status 0
(FSL-20), the permission's message is mcflirt's (its `-in` directory before the name) and the run
goes on without matrices, an unopenable `.rms` file is not written, an unopenable `.par` file ends
the run at volume 0 with status 0 (FSL-65). An image that cannot be written is an error in both
modes (mcflirt aborts; with `reproduce` its two `terminate` lines are printed first). **The
replica** (`larmorx-nc fsl mcflirt`, `implementation="replica"`) does the same since 2026-10-30
(by default it opens the `.mat` directory, the `.par` file and the `.rms` files in that order,
an existing file kept until it is written; with `reproduce` mcflirt's order and files), the
§14.1 cases in its record as in the clean room's.

`bins` (`-bins`) below 8 with a histogram cost (FSL-21) and negative values (FSL-66) are refused
by default before the input is read, with `-stages` 1 or more (`-stages 0` registers nothing and
is not refused), in the command line and the Python API alike. With `reproduce`, `mutualinfo`,
`normmi` and `woods` run with no bins in a stage as mcflirt does (appendix B with `B = 0`: their
cost is rounding noise, their matrices arbitrary; FSL-21 named); where mcflirt crashes
(`corratio` with no bins, −1 bins or fewer in a stage that runs) larmorx prints mcflirt's own
lines (`double free or corruption (out)`, the two `std::bad_array_new_length` lines; none for
SIGFPE), names the defect and exits 1, writing nothing (a crash is never reproduced). With one
volume and no reference nothing is registered, so `reproduce` runs (identities). The replica
likewise since 2026-10-30 (where mcflirt crashes its own line says how, after mcflirt's; its
matrices with no bins in a stage are mcflirt's to the byte).

On fMRIPrep's command (`-reffile <boldref> -mats`) FSL-01 changes the last volume's matrix
(the others are mcflirt's), and FSL-09 the rounding of the corrected series when the input is
integer (fMRIPrep uses only the matrices). The parity record's "D13 fix pass" lists, run by
run, what the corrections change.

Where the last volume is close to its predecessor the two starts reach nearly the same matrix:
on the 52 real runs of the record of 2026-11-11 a run's largest change is 0.0204 mm (median)
and at most 15.9 mm, and on fMRIPrep's command for ds000005 the last volume moves by 0.06 mm,
after which it is 0.021 mm from its predecessor instead of 0.055 mm (the other volumes' steps
have a median of 0.038 mm). Where the motion is large, mcflirt's start from the identity can
fail on its own: on the synthetic `background` series (large translations over a non-zero
background), the record's largest change, 106 mm, mcflirt's last volume converges to a 180°
rotation, 106 mm from the true motion, and the corrected one to 0.36 mm
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

### glibc's maths functions (GLIBC-07)

mcflirt's histogram costs (`cost="mutualinfo"`, `"normmi"`) take their logarithms from glibc 2.35's
`logf` (`specs/mcflirt.md` appendix B), which is not correctly rounded. The clean room
computes them correctly rounded by default and takes glibc's with `upstream_defects="reproduce"`
(`larmorx_core::math`'s `*_glibc` functions, proved bit for bit against glibc 2.35; the user's
decision of 2026-11-04: glibc's bits for `reproduce` only). The registration is an iterative fit:
one logarithm's last bit can move the matrices' last digits, and the estimate cannot tell without
registering twice, so neither mode names GLIBC-07: the default's correctly rounded functions can
move a mutual-information registration's last bits. The other costs call none of glibc's functions;
the rotations' sines and cosines are the correctly rounded single-precision values in both modes
(the spec names no glibc function there).


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
| LARMORX against mcflirt | 0.0151–0.0274 mm | 0.8687–0.9989 |
| mcflirt against itself, input perturbed slightly (the band) | 0.0164–0.1014 mm | 0.7941–0.9977 |

With fMRIPrep's command, LARMORX is as close to mcflirt as mcflirt is to itself, or closer, on
every run; FD correlations are low
only for subjects that barely move (FD differences stay below 0.05 mm). On synthetic series
with known motion, larmorx and mcflirt are equally accurate (mean RMS deviation from the
truth 0.4171 and 0.4143 mm on 27 runs). 230 of the 11,475 matrix files compared are identical to
mcflirt's as text (6 decimals).

Larger differences, all within mcflirt's own band for the same runs: `-meanvol` (the mean
itself depends on the first pass), `-dof 12`, very small images (10 × 10 × 8 voxels) and
synthetic copies of one volume. A linear ramp, which only an x translation can align, leaves
the other parameters unconstrained: both programs find the translation (within 0.3 mm of each
other) and wander differently elsewhere. Woods' cost registers no BOLD run for either
program. `corratio` and `leastsquares` agree within the thresholds on ds000005, although the
spec describes them only in outline; `mutualinfo` and `normmi` follow the spec's appendix B
exactly since 2026-10-29.

**Random phantoms** (2026-10-29, [findings](../findings/mri-hmc-clean-room.md)): on the 782
random phantoms where random testing found the estimates beyond the thresholds (small images
moved by 5° to 30°: a cost with several minima), the corrected series is as well aligned as
mcflirt's for every cost (temporal standard deviation, median ratio 1.000). `cost="normmi"`
aligned worse until the exact histogram costs (median ratio 1.051 against mcflirt 6.0.7.23 on
91 phantoms, now 1.000, 55 of them within 1 %; `mutualinfo` 0.983, now 1.000, 54 of 78 within
1 %). With `-fudge -stages 1`, mcflirt's evaluated matrices are larmorx's, evaluation by
evaluation, on 22 of 30 phantoms; the others part after 19 to 224 evaluations in the last
decimals, as with `normcorr`.

**The count per point and `leastsquares`** (2026-11-07; `specs/mcflirt.md` §6.4 and appendix A,
the spec writer's answers of 2026-11-06): the weighted correlation's count takes each point's
weight into its first running total as the point is visited (each row's total before), which
follows mcflirt's evaluated matrices longer on every recorded run traced
([findings](../findings/mri-hmc-clean-room.md)); `cost="leastsquares"` is FLIRT's `leastsq` (three
single-precision levels; double sums before).

**Centres of mass** (the clean-room catch-up, 2026-11-05; `specs/mcflirt.md` §6.1): both of
mcflirt's centres, each test volume's (the rotations' centre) and the reference's (the `.par`
file's), are FLIRT's blocked sums (`v − v_min` in single precision, double running sums in z, y, x
order flushed into the totals every `max(1000, trunc(√N)) + 1` voxels; a total below 1e−5 in
magnitude counts 1); before, one running sum (the same bits wherever the sums are exact, as on
integer images). A centre whose total is below
1e−5 (a constant volume) prints mcflirt's `WARNING::in calculating COG, total = 0.0` on standard
error as often as mcflirt does (black box,
`validation/investigations/cleanroom-align/cog_warning_probe.py`, 8 of 8 calls alike): once per
registration of such a test volume (three with the default stages, four with `-stages 4`, six with
`-meanvol`) and once per `.par` line when the reference is such a volume (`-plots`). The
decomposition's arctangent is `larmorx_core::math::atan2` (correctly rounded, double) since the same
day, the `libm` crate's before; mcflirt's is glibc's single-precision `atan2f`, a later part's
`reproduce` item. On the record (`mri-hmc`, re-made 2026-11-05) nothing moves: the real runs are
integer images, whose centres are the same bit for bit either way, and fMRIPrep's calls give the
same matrices as text; the golden cases' random series move with `atan2`.

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


## History

- **The clean room** (FSL 6.0.7.17 first, FSL 6.0.7.23 since 2026-10-29; the exact histogram costs
  of 2026-10-29; `corratio` and `woods` of 2026-11-02; the catch-up of 2026-11-05; the count per
  point and `leastsquares` of 2026-11-07); its record of 2026-11-11. An earlier record had one
  case missing its threshold by a hair: with mcflirt's default reference on ds003345, the
  framewise displacement correlated with mcflirt's at r = 0.9885, where mcflirt against itself
  reached 0.9888–0.9903; the record of 2026-11-11 has no failure (that case: r = 0.9907).
- **The replica** ([PROVENANCE](../../crates-nc/larmorx-nc-mcflirt/PROVENANCE.md)): FSL 6.0.7.17
  first (its full record of 2026-10-25), FSL 6.0.7.23 since 2026-10-29 (its record of
  2026-11-02); the failure cases since 2026-10-30.
- **Status: `experimental`.**
