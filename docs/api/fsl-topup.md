# topup and applytopup (susceptibility distortion from reversed phase encodings)

`lx.fsl.topup` and `lx.fsl.applytopup` (command line `larmorx fsl topup`, `larmorx fsl
applytopup`): FSL 6.0.7.23's `topup`, which estimates the off-resonance field of echo-planar
images, and `applytopup`, which corrects images with it.

The ports come first. One section per program follows, the source of its reference page on the
documentation site (`python -m larmorx_validation reference`), with its options table checked
against the code; then what the two programs share: the records, FSL's settings files, FSL's
defects as both ports correct them, the clean room's design and what is not done yet. The page's
dates are in its [History](#history).

## Ports

FSL is distributed under the FSL Licence (non-commercial use only), so each program has two
ports, a replica and a clean room, in different packages ([licensing](../licensing.md)):

- **The replica** is bit-exact: `larmorx-nc fsl topup` and `fsl applytopup`, in the package
  `larmorx-nc` (`crates-nc/larmorx-nc-topup`), under the FSL Licence's terms, translated from
  FSL's source with the B-spline fields, sparse matrices, non-linear optimisers and warps of FSL's
  libraries.
- **The clean room** is in the main package (Apache-2.0, crate `larmorx-fsl`, module `topup`),
  written from [`specs/topup.md`](../../specs/topup.md) without FSL's source.

`implementation="auto"` (the default) runs the replica when `larmorx-nc` is installed (`pip
install larmorx-nc`) and the clean room otherwise; `"replica"` needs `larmorx-nc` (otherwise
`ReplicaNotFoundError`); `"clean-room"` (`"original"`, 0.1.0rc1's spelling, still works) always
runs the clean room. The command line chooses alike (`--implementation` before the family, or
`LARMORX_IMPLEMENTATION`).

## `topup`

<!-- reference
program: topup
group: fsl
original: FSL 6.0.7.23
python: larmorx.fsl.topup
command-line: crates/larmorx-fsl/src/cli/topup.rs
records: {replica: [fsl-topup-replica], clean-room: [fsl-topup]}
evidence: fsl-topup
-->

`topup` estimates the off-resonance field (Hz) that displaces echo-planar images along their phase
encoding, from two or more images acquired with different phase encodings (typically "blip up"
and "blip down"), with each image's rigid-body movement if asked.

### Python (`topup`)

`lx.fsl.topup(images, acquisition, *, config=None, fsldir=None, warpres=None, subsamp=None, fwhm=None, miter=None, lambda_=None, ssqlambda=None, regmod=None, estmov=None, minmet=None, splineorder=None, numprec=None, interp=None, scale=None, regrid=None, field=True, corrected=False, feat=False, displacement_fields=False, rigid_body_matrices=False, jacobians=False, verbose=False, n_threads=1, implementation="auto", upstream_defects="fix")`

`None` (the default of every estimation setting) leaves the option out, so the configuration
file, or topup's own default, sets it; a value given wins over the configuration, as on topup's
command line. FSL's settings are the call's (CLAUDE.md rule 6): the outputs are NIfTI-1 read
back, and no FSL variable of the process reaches the program.

```python
import larmorx as lx

# fMRIPrep's call (sdcflows' PEPOLAR estimation):
# topup --config=b02b0.cnf --datain=<encfile> --imain=<pair> --out=<base> --iout --fout
#       --jacout --logout --rbmout --dfout
acq = [[0, 1, 0, 0.0138], [0, -1, 0, 0.0138]]   # phase-encoding vector and read-out time (s)
r = lx.fsl.topup(
    "b0_pair.nii.gz", acq, config="sdcflows/data/flirtsch/b02b0.cnf",
    corrected=True, jacobians=True, rigid_body_matrices=True, displacement_fields=True,
)
field_hz = r.field                     # lx.Image on the input's grid
up_corrected = r.corrected.data[..., 0]
```

### Options of `topup`

| `topup` option | Python | Notes |
|---|---|---|
| `--imain` | `images` | the 4D image of the volumes (path or `lx.Image`) |
| `--datain`, `--acqp` | `acquisition` | per volume the phase-encoding vector (3 numbers, voxel axes) and the read-out time (s): a file, or an array of shape `(n, 4)` |
| `--config` | `config` | a configuration file, or the name of one of FSL's: the six FSL ships (`"b02b0"`, `"b02b0_1"`, `"b02b0_2"`, `"b02b0_4"`, `"b02b0_7T"`, `"test_acqp"`, with or without `.cnf`) need nothing more ([LARMORX's own settings](#larmorxs-own-settings)); any other needs `fsldir` |
| `FSLDIR` | `fsldir` | the FSL installation whose `etc/flirtsch` holds FSL's configurations (only for a `config` given by name; its file wins over LARMORX's own settings of the same name) |
| `--warpres` | `warpres` | knot spacing (mm), one value or one per level; 10 |
| `--subsamp` | `subsamp` | subsampling factor per level; 1 |
| `--fwhm` | `fwhm` | Gaussian smoothing (mm) per level; 8 |
| `--miter` | `miter` | iterations per level; 5 |
| `--lambda` | `lambda_` | regularisation weight per level; 0 |
| `--ssqlambda` | `ssqlambda` | the weight times the current sum of squared differences; on |
| `--regmod` | `regmod` | `"bending_energy"` (default) or `"membrane_energy"` |
| `--estmov` | `estmov` | estimate the movements, per level; on |
| `--minmet` | `minmet` | `"LM"` (Levenberg-Marquardt, default) or `"SCG"` (scaled conjugate gradients), per level |
| `--splineorder` | `splineorder` | 2 (quadratic) or 3 (cubic, default) |
| `--numprec` | `numprec` | the Hessian's precision: `"double"` (default) or `"float"` |
| `--interp` | `interp` | `"spline"` (default) or `"linear"` |
| `--scale` | `scale` | each volume scaled to a mean of 100 by itself; off |
| `--regrid` | `regrid` | work on a grid of `n + s_max` voxels per axis; on |
| `--out` | the result | the base name of the coefficients (`_fieldcoef`) and the movements (`_movpar.txt`) |
| `--fout` | `field` | the field in Hz |
| `--iout` | `corrected` | the corrected volumes and their mean (`_mean`) |
| `--featout` | `feat` | the field in rad/s and the corrected mean, for FEAT |
| `--dfout` | `displacement_fields` | each volume's displacement field (mm) |
| `--rbmout` | `rigid_body_matrices` | each volume's rigid-body matrix |
| `--jacout` | `jacobians` | each volume's Jacobian determinant |
| `--logout` | the result | the log's text: the result's `log` |
| `-v`, `--verbose` | `verbose=True` | the cost of every evaluation, in the result's `stdout` |
| `--nthr` | `n_threads` | threads; the results are FSL's one-thread results at any count (FSL-116). On the command line without `--nthr` (nor in the configuration), the replica takes LARMORX's own `--n-threads` (else `OMP_NUM_THREADS`, else every logical CPU) |
| `--debug`, `--trace`, `--profile` | command line only | the hidden debugging options |
| `-h`, `--help` | command line only | |
| `LARMORX_UPSTREAM_DEFECTS` | `upstream_defects` | `"reproduce"`: FSL's defects as FSL has them |

### Examples of `topup`

<!-- inputs: b0.nii.gz=head4d acqp.txt=acqp4 -->

```python example
import larmorx as lx

# One level of a coarse field, without movements: a quick estimate.
r = lx.fsl.topup("b0.nii.gz", "acqp.txt", warpres=20, subsamp=2, miter=2, estmov=False)
r.field.shape  # -> (40, 48, 40)
```

```bash example
larmorx fsl topup --imain=b0 --datain=acqp.txt --warpres=20 --subsamp=2 --miter=2 --estmov=0 --out=tu --fout=field
```

### Ports of `topup`

Both ports are complete (`specs/topup.md` §2 to §10): every option of topup (precision,
interpolation, both regularisers, spline orders, scaling, regridding, the minimisers per level,
the movements, FSL's shipped configurations, x and y and oblique phase encodings, four volumes, a
zero read-out time, `--debug`, `--trace`, `--profile`), every message, exit status and failure
case, each compared file by file with FSL's. fMRIPrep's topup call: bit-identical in the
replica in two of fMRIPrep's runs (ds005040 and ds001600: every file topup writes); in the clean
room, ds001600 within the thresholds, ds005040 within FSL's own band of a last-bit change (the
field's p99 0.61 Hz against FSL at `--nthr=4`'s 0.64).

The replica's record holds 102 cases of `topup`: 59 identical to FSL, 42 where both refuse
alike, 1 refused in both modes. The clean room's holds 112: 62 within its thresholds, 42 refused
alike, 8 expected divergences ([the records](#the-records) say why). The clean room is built for
speed: fMRIPrep's call on ds005040 takes 2.8 s on one thread and 0.8 s on 14 (FSL 32.6 s, the
replica 29.7 s), on ds001600 7.7 s and 2.0 s (FSL 77.4 s, the replica 71.4 s).

## `applytopup`

<!-- reference
program: applytopup
group: fsl
original: FSL 6.0.7.23
python: larmorx.fsl.applytopup
command-line: crates/larmorx-fsl/src/cli/applytopup.rs
records: {replica: [fsl-topup-replica], clean-room: [fsl-topup]}
evidence: fsl-topup
-->

`applytopup` corrects images with topup's result, by Jacobian modulation of each image or by the
least-squares restoration of pairs with opposite phase encodings.

### Python (`applytopup`)

`lx.fsl.applytopup(images, acquisition, indices, topup_result, *, method="lsr", interp="spline", datatype=None, verbose=False, n_threads=1, implementation="auto", upstream_defects="fix")`

FSL's settings are the call's (CLAUDE.md rule 6): the outputs are NIfTI-1 read back, and no FSL
variable of the process reaches the program.

### Options of `applytopup`

| `applytopup` option | Python | Notes |
|---|---|---|
| `-i`, `--imain` | `images` | one image or a list of them (3D or 4D, one grid) |
| `-a`, `--datain` | `acquisition` | as topup's |
| `-x`, `--inindex` | `indices` | each image's row of `acquisition` (1-based) |
| `-t`, `--topup` | `topup_result` | a `TopupResult`, a field image in Hz, or the base name of topup's files (without extension) |
| `-o`, `--out` | the result | the corrected image |
| `-m`, `--method` | `method` | `"lsr"` (least-squares restoration of opposite phase encodings), `"jac"` (Jacobian modulation), `"vb2D"`, `"vb3D"`, `"vb4D"` (undocumented variational-Bayes restorations) |
| `-n`, `--interp` | `interp` | `"spline"` (default) or `"trilinear"` |
| `-d`, `--datatype` | `datatype` | `"preserve"`, `"char"`, `"short"`, `"int"`, `"float"` (default), `"double"` |
| `-v`, `--verbose` | `verbose=True` | (applytopup prints nothing more) |
| `-h`, `--help` | command line only | |
| `OMP_NUM_THREADS` | `n_threads` | threads of the resampling; the results do not depend on it |
| `LARMORX_UPSTREAM_DEFECTS` | `upstream_defects` | as topup's |

### Examples of `applytopup`

<!-- inputs: b0.nii.gz=head4d acqp.txt=acqp4 -->

```python example
import larmorx as lx

r = lx.fsl.topup("b0.nii.gz", "acqp.txt", warpres=20, subsamp=2, miter=2, estmov=False)
# Each volume corrected by Jacobian modulation, with topup's result.
c = lx.fsl.applytopup("b0.nii.gz", "acqp.txt", [1], r, method="jac")
c.corrected.shape  # -> (40, 48, 40, 4)
```

```bash example
larmorx fsl topup --imain=b0 --datain=acqp.txt --warpres=20 --subsamp=2 --miter=2 --estmov=0 --out=tu
larmorx fsl applytopup --imain=b0 --datain=acqp.txt --inindex=1 --topup=tu --method=jac --out=b0_corrected
```

### Ports of `applytopup`

Both ports are complete (`specs/topup.md` §2 to §10): applytopup's every method (`lsr`, `jac`,
`vb2D`, `vb3D`, `vb4D`; lines of 30, 32 and 96 voxels), data type and failure, each compared file
by file with FSL's. The replica's record holds 75 cases of `applytopup`: 48 identical to FSL, 25
where both refuse alike, 2 refused in both modes. The clean room's holds 75: 44 within its
thresholds, 25 refused alike, 6 expected divergences ([the records](#the-records) say why). The
clean room's `lsr` on ds005040's pair takes 0.13 s on one thread and 0.04 s on 14 (FSL 0.42 s, the
replica 3.47 s).

## The records

**The replica** against FSL 6.0.7.23 itself (the comparison suite `fsl-topup-replica`, record
[fsl-topup-replica](../validation/fsl-topup-replica.md), 2026-11-03): 177 cases, 107 of 107
compared cases bit-identical, 67 more rejected by both alike, 3 refused in both modes; random
testing's trial: 14,460 cases, every one bit-identical.

**The clean room** against FSL 6.0.7.23 (the comparison suite `fsl-topup`, record
[fsl-topup](../validation/fsl-topup.md), 2026-11-05, §12's thresholds, compared in `reproduce`
mode): 187 cases of the standard comparison (the replica suite's cases, fMRIPrep's two calls and
random testing's permanent cases), 106 within the thresholds, 67 rejected by both alike, 14
expected divergences (each with its reason in the record: 6 cases whose movements are estimated
and not converged, where FSL's own path is not determined beyond its inputs' last bits (FSL-201's
forward differences: FSL's runs with the read-out times changed in their last bits spread as far,
finding 37); FSL-143's diverging iterations; vb2D's transients; FSL-126's negative weight, with
which both minimisations diverge; the inputs refused for FSL-124 and FSL-134), 0 failures.
Random testing's trial (14,400 cases, the replica trial's draws) is triaged case by case in
[the findings](../findings/fsl-topup-clean-room.md) (finding 36; its open items closed by the
catch-up of 2026-11-04, findings 37 to 44); the catch-up's trial of 2026-11-05 (17,535 cases,
13,300 agree) and FSL-242 in findings 45 to 47.

## FSL's settings files without FSL

larmorx-nc carries FSL 6.0.7.23's six `topup` settings
files byte for byte (`b02b0.cnf`, `b02b0_1.cnf`, `b02b0_2.cnf`, `b02b0_4.cnf`, `b02b0_7T.cnf`,
`test_acqp.cnf`, from fsl-topup 2203.5; the user's decision of 2026-11-09), installed by its wheel
in `<prefix>/share/larmorx-nc/fsl-data/etc/flirtsch/`: the replica program (`larmorx-nc fsl topup
--config=b02b0`) looks for a name without a directory there after FSL's own places (as named,
with `.cnf`, `$FSLDIR/etc/flirtsch/`), so the name works without an FSL installation; when
neither has it, FSL's messages (and FSL-121's) are unchanged.

## LARMORX's own settings

Since 2026-11-11 (the user's decision of 2026-11-09), the clean room
carries its own settings of the same six names, written from `specs/topup.md` §2.5 by an
implementer who never opened FSL's files (`crates/larmorx-fsl/src/topup/settings/`, Apache-2.0,
compiled into `larmorx fsl topup`). It finds a name the same way, after FSL's own places (as
named, with `.cnf`, `$FSLDIR/etc/flirtsch/`), FSLDIR unset included: so `config="b02b0"` (or
`b02b0_1`, `_2`, `_4`, `b02b0_7T`, `test_acqp`, with or without `.cnf`) needs no FSL and no
larmorx-nc, and `fsldir` is needed only for other names. Where an FSL installation holds the
file, FSL's file is read, as FSL does; larmorx-nc's copy is read only by the replica. The values
are FSL's: every output of topup is bit for bit the same with larmorx's settings as with FSL's
files, for the replica program (FSL's own results) and for the clean room, on fMRIPrep's inputs of
ds005040 and ds006736 and a synthetic pair with movements (54 of 54 comparisons; 234 runs with one
value changed all differ:
[`validation/investigations/topup-settings-original/`](../../validation/investigations/topup-settings-original/README.md)).

## File names on Windows

FSL finds a name's directory and extension at the last `/`. On Windows,
where FSL does not run, `\` separates directories as well. So a configuration given without
`.cnf`, and applytopup's `--topup` base name, behave as on Linux when a directory's name holds a dot
(`C:\Users\...\.tmp1\b02b0`). On Linux and macOS, `\` stays an ordinary character, as in FSL.

## The clean room

Every option, message, exit status and failure case of both programs (the spec's §2 to §10):
the configuration files, both minimisers per level, both regularisers, both spline orders, the
precisions and interpolations, the movements, every output, `--debug`, `--trace` and
`--profile`; applytopup's `jac`, `lsr`, `vb2D`, `vb3D` and `vb4D`. Its results never depend on
the thread count (`n_threads`, `--nthr`: deterministic sums), and correctly rounded `sin`,
`cos` and `exp` give the same bits on every platform. Built for speed (§11; the benchmark
[fsl-topup](../benchmarks/fsl-topup.md), informal, one run each):

| Call | FSL (1 thread) | replica | clean room, 1 thread | clean room, 14 threads |
|---|---|---|---|---|
| fMRIPrep's topup, ds005040 | 32.6 s | 29.7 s | 2.8 s | 0.8 s |
| fMRIPrep's topup, ds001600 | 77.4 s | 71.4 s | 7.7 s | 2.0 s |
| applytopup `lsr`, ds005040's pair | 0.42 s | 3.47 s | 0.13 s | 0.04 s (0.03 s with the matrices row by row, 2026-11-04: `lsr_speed.py`) |

Where the spec leaves a convention open, the clean room follows FSL's as measured: an oblique
phase encoding wraps along no axis; the scaled conjugate gradients' second gradient keeps the
current point's regulariser weight; `--debug`'s images are written in FSL's order; the mask's
frame follows each volume's phase encoding (a volume with a read-out time of 0 included) and
holds for moved volumes too. Both modes follow FSL's spline prefilter (its truncated start, which
decides the interpolation on axes shorter than 15 voxels), its regulariser's sampling of quadratic
B-splines and applytopup's resampling matrices built row by row. By default it corrects FSL where
FSL's derivatives of the movements are not its cost's (FSL-201: the clean room's are exact, and its
result moves in proportion to a change of its inputs' last bits, where FSL's jumps by up to 1e-2)
and where FSL-133 makes FSL's restoration wrong; `reproduce` gives FSL's derivatives and FSL's
read.

## Defects of the original, corrected by default

topup's and applytopup's defects are corrected by default and reproduced with
`upstream_defects="reproduce"` (`LARMORX_UPSTREAM_DEFECTS=reproduce`); each correction that
changes a result is named on standard error and as an `UpstreamDefectWarning`
([all of them](upstream-defects.md)):

- **FSL-11**: with `FSLOUTPUTTYPE` unset FSL's topup and applytopup stop with FSL's message at
  their first image (exit 1). Both ports, the clean room and the replica, write
  the images as `NIFTI_GZ` (`.nii.gz`: the bytes `FSLOUTPUTTYPE=NIFTI_GZ` gives) and name FSL-11 on
  one line per command, in the same words (the replica's line starts `larmorx-nc:`); `reproduce`
  stops as FSL does. Command line only: the Python functions set the type themselves. applytopup's
  integer `--datatype` outputs meet FSL-09 (rounded and clamped), its notice naming applytopup.
- **FSL-116**: FSL's results depend on its thread count (`--nthr`, `FSL_NUM_THREADS`); larmorx
  gives FSL's one-thread result at any count.
- **FSL-117**: with the membrane energy, Levenberg-Marquardt's Hessian of the regulariser is in
  other units than its cost: corrected (the final cost 0 to 1.7 % lower).
- **FSL-118**: subsamplings whose largest does not divide the others (3 and 2) stop topup; larmorx
  grows the grid by their least common multiple.
- **FSL-119**: the field maps get the input's intent parameters shifted by one place: corrected.
- **FSL-120**: a row-count mismatch with `--acqp` names an empty file: corrected.
- **FSL-121**: FSLDIR unset with a configuration name not found: a clear message.
- **FSL-122**: a configuration line longer than 1023 characters silently ends the file: read whole.
- **FSL-123** to **FSL-126**: a negative `--miter`, `--warpres`, `--fwhm` (which FSL takes as no
  smoothing) or `--lambda`: refused.
- **FSL-132**: applytopup resamples the first volume trilinearly whatever `--interp` asks: corrected.
- **FSL-133**: applytopup's restoration wraps a line wrongly for lengths that do not divide 2³²:
  corrected (`reproduce`: FSL's read, the field of voxel `(k + 2³²) mod n` before a line's first
  voxel, in both ports since 2026-11-04).
- **FSL-134**: applytopup with a field image scales the y displacement by the x voxel size, and
  takes fields on other grids: corrected, refused (`reproduce`: FSL's scales, and a field of as
  many voxels as the images in another shape laid as stored, its Jacobian at the same indices;
  in the clean room since 2026-11-04).
- **FSL-273**: applytopup `--method=jac` divides the field's derivatives by the field's own voxel
  sizes while it applies the displacement at the images' voxel indices, so a field (coefficients
  or a field image) of the images' shape whose voxel sizes differ from the images' gets a
  Jacobian scaled by their ratio: corrected (the Jacobian of the displacement applied; the same
  result wherever the voxel sizes agree), named when the voxel sizes differ along an encoded
  axis (`reproduce`: FSL's scales; the replica since 2026-11-06, the clean room since
  2026-11-07).
- **FSL-135**: an unknown `--method` writes nothing and exits 0; a restoration without movements
  aborts: refused clearly.
- **FSL-141** to **FSL-143**: the vb methods' counts in 32-bit integers, `vb3D`/`vb4D`'s count of
  data points and their prior across slices (indefinite: FSL's iterations diverge): corrected.
- **FSL-144**: `--method=jac` with a phase encoding along z (which topup does not estimate)
  leaves the image uncorrected silently: refused (both ports; the replica since
  2026-11-03).
- **FSL-145**: `--splineorder=2` lays two coefficients on an axis shorter than two knot
  spacings (one on an axis shorter than one spacing, whose refit between levels or for the
  outputs stops topup), so the field is forced towards 0 at its far end: the full count by
  default (both ports; applytopup reads a coefficient file of either count by default
  and FSL's only with `reproduce`, where FSL aborts on the other).
- **FSL-146**: with volumes encoded along different axes (AP, PA, LR, RL) the gradient is not
  the cost's: corrected, the cost's own (both ports; the volumes left out are those
  whose encoding along the axis is 0, whatever their read-out time).
- **FSL-201**: with the movements estimated, the derivatives with respect to the rotations are
  forward differences of rotation matrices built in single precision, so the minimisers end
  elsewhere than the cost's minimum and FSL's path depends on the last bits of its inputs: the
  exact derivatives by default (both ports; `reproduce` FSL's matrices and forward
  differences, in the clean room since 2026-11-04; fMRIPrep's calls, which estimate no movements,
  unchanged).
- **FSL-242**: with `--minmet=1` (scaled conjugate gradients), a level whose last iteration's
  step is rejected (its cost above the current point's) ends at that rejected, worse point, from
  which the next level or the outputs start: the lowest cost the level found by default
  (the clean room since 2026-11-05; `reproduce` FSL's; named on every run where it
  happens). `--miter` counts every iteration, the rejected ones too, in both modes (FSL's, observed
  with `-v`).

### glibc's maths functions (GLIBC-07)

FSL's topup takes its smoothing's Gaussian taps from glibc 2.35's `exp` (`specs/topup.md` §4), which
is not correctly rounded. The clean room computes it correctly rounded by default and takes
glibc's with `upstream_defects="reproduce"` (`larmorx_core::math`'s `*_glibc` functions, proved bit
for bit against glibc 2.35; the user's decision of 2026-11-04: glibc's bits for `reproduce` only).
The estimate is an iterative fit: a tap's last bit can move the field's and the movements' last
bits, and topup cannot tell without estimating twice, so neither mode names GLIBC-07 (the taps are
rounded to single precision, where the two choices practically never differ). applytopup calls none
of glibc's functions.

## Not yet

- applytopup's restorations of lines of fewer than 8 or more than 256 voxels along the phase
  encoding, and band systems that need LAPACK's blocked band factorisation, are refused in both
  modes with a clear message (`larmorx-nc: ... not replicated`): FSL's arithmetic there follows
  other OpenBLAS and Armadillo paths, which the replica does not reproduce yet.
- The full comparison, the data panel's PEPOLAR runs and long random testing: in the final validation
  phase (the record's standard comparison holds fMRIPrep's calls in two of its runs; the site's page,
  `/tools/fsl-topup/`, shows them with every case of the record).

## History

- **The replica** (2026-11-03): [PROVENANCE](../../crates-nc/larmorx-nc-topup/PROVENANCE.md), with
  FSL's libraries in `larmorx-nc-core`.
- **The clean room** (2026-11-03); its catch-up of 2026-11-04 and 2026-11-05; LARMORX's own
  settings since 2026-11-11.
- **Status: `experimental`.**
