# topup and applytopup (susceptibility distortion from reversed phase encodings)

`lx.fsl.topup` (command line `larmorx fsl topup`): FSL 6.0.7.23's `topup`, the estimation of the
off-resonance field (Hz) that displaces echo-planar images along their phase encoding, from two
or more images acquired with different phase encodings (typically "blip up" and "blip down"),
with each image's rigid-body movement if asked. `lx.fsl.applytopup` (`larmorx fsl applytopup`):
the correction of images with topup's result, by Jacobian modulation of each image or by the
least-squares restoration of pairs with opposite phase encodings.

**Two implementations.** FSL is distributed under the FSL Licence (non-commercial use only), so
larmorx keeps its FSL tools in two implementations ([licensing](../licensing.md)): the bit-exact
**replica**, `larmorx-nc fsl topup` and `fsl applytopup` (package `larmorx-nc`,
`crates-nc/larmorx-nc-topup`, under the FSL Licence's terms; translated from FSL's source with
the B-spline fields, sparse matrices, non-linear optimisers and warps of FSL's libraries in
`larmorx-nc-core`: [PROVENANCE](../../crates-nc/larmorx-nc-topup/PROVENANCE.md)), done
(2026-11-03), and the clean-room **original** in the main package (Apache-2.0, crate
`larmorx-fsl`, module `topup`), written from [`specs/topup.md`](../../specs/topup.md) without
FSL's source (2026-11-03). `implementation="auto"` (the default) runs the replica when
`larmorx-nc` is installed (`pip install larmorx-nc`) and the original otherwise; `"replica"`
needs `larmorx-nc` (otherwise `ReplicaNotFoundError`); `"original"` always runs the original.
The command line chooses alike (`LARMORX_IMPLEMENTATION`).

**Status: `experimental`**: the replica against FSL 6.0.7.23 itself (the parity suite
`fsl-topup-replica`, record [fsl-topup-replica](../validation/fsl-topup-replica.md), 2026-11-03:
104 of 104 compared cases bit-identical, 66 more rejected by both alike, 3 refused in both modes;
random testing's trial: 14,460 cases, every one bit-identical): fMRIPrep's topup call
bit-identical on its two oracle runs (ds005040 and ds001600: every file topup writes), every
option of topup (precision, interpolation, both regularisers, spline orders, scaling,
regridding, the minimisers per level, the movements, FSL's shipped configurations, x and y and
oblique phase encodings, four volumes, a zero read-out time, `--debug`, `--trace`,
`--profile`), every message, exit status and failure case, and applytopup's every method
(`lsr`, `jac`, `vb2D`, `vb3D`, `vb4D`; lines of 30, 32 and 96 voxels), data type and failure,
each compared file by file with FSL's.

The original against FSL 6.0.7.23 (the parity suite `fsl-topup`, record
[fsl-topup](../validation/fsl-topup.md), 2026-11-05, §12's thresholds, compared in `reproduce`
mode): 185 cases of the standard tier (the replica suite's cases, fMRIPrep's two calls and 9
permanent cases of random testing), 104 within the thresholds, 67 rejected by both alike, 14
expected divergences (each with its reason in the record: 6 cases whose movements are estimated
and not converged, where FSL's own path is not determined beyond its inputs' last bits (FSL-201's
forward differences: FSL's runs with the read-out times changed in their last bits spread as far,
finding 37); FSL-143's diverging iterations; vb2D's transients; FSL-126's negative weight, with
which both minimisations diverge; the inputs refused for FSL-124 and FSL-134), 0 failures.
fMRIPrep's calls: ds001600 within the thresholds, ds005040 within FSL's own band of a last-bit
change (the field's p99 0.61 Hz against FSL at `--nthr=4`'s 0.64). Random testing's trial (14,400
cases, the replica trial's draws) is triaged case by case in
[the findings](../findings/fsl-topup-original.md) (finding 36; its open items closed by the
catch-up of 2026-11-04, findings 37 to 44); the catch-up's trial of 2026-11-05 (17,535 cases,
13,300 agree) and FSL-242 in findings 45 to 47.

## Quick start

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

# FSL's configuration by name (larmorx carries the six FSL ships: no FSL needed), then the
# correction of a DWI pair by least squares
r = lx.fsl.topup("b0_pair.nii.gz", acq, config="b02b0")
hifi = lx.fsl.applytopup(["dwi_up.nii.gz", "dwi_dn.nii.gz"], acq, [1, 2], r).corrected
```

From the command line, FSL's own arguments:

```
larmorx fsl topup --imain=b0_pair --datain=acqparams.txt --config=b02b0.cnf --out=tu --fout=field
larmorx fsl applytopup --imain=dwi_up,dwi_dn --inindex=1,2 --datain=acqparams.txt --topup=tu --out=hifi
```

## Options

### `lx.fsl.topup`

| `lx.fsl.topup` | `topup` | Default | Meaning |
|---|---|---|---|
| `images` | `--imain` | | the 4D image of the volumes (path or `lx.Image`) |
| `acquisition` | `--datain` | | per volume the phase-encoding vector (3 numbers, voxel axes) and the read-out time (s): a file, or an array of shape `(n, 4)` |
| `config` | `--config` | | a configuration file, or the name of one of FSL's: the six FSL ships (`"b02b0"`, `"b02b0_1"`, `"b02b0_2"`, `"b02b0_4"`, `"b02b0_7T"`, `"test_acqp"`, with or without `.cnf`) need nothing more; any other needs `fsldir` |
| `fsldir` | `FSLDIR` | | the FSL installation whose `etc/flirtsch` holds FSL's configurations (only for a `config` given by name; its file wins over larmorx's own settings of the same name) |
| `warpres` | `--warpres` | 10 | knot spacing (mm), one value or one per level |
| `subsamp` | `--subsamp` | 1 | subsampling factor per level |
| `fwhm` | `--fwhm` | 8 | Gaussian smoothing (mm) per level |
| `miter` | `--miter` | 5 | iterations per level |
| `lambda_` | `--lambda` | 0 | regularisation weight per level |
| `ssqlambda` | `--ssqlambda` | `True` | the weight times the current sum of squared differences |
| `regmod` | `--regmod` | `"bending_energy"` | or `"membrane_energy"` |
| `estmov` | `--estmov` | `True` | estimate the movements, per level |
| `minmet` | `--minmet` | `"LM"` | `"LM"` (Levenberg-Marquardt) or `"SCG"` (scaled conjugate gradients), per level |
| `splineorder` | `--splineorder` | 3 | 2 (quadratic) or 3 (cubic) |
| `numprec` | `--numprec` | `"double"` | the Hessian's precision, or `"float"` |
| `interp` | `--interp` | `"spline"` | or `"linear"` |
| `scale` | `--scale` | `False` | each volume scaled to a mean of 100 by itself |
| `regrid` | `--regrid` | `True` | work on a grid of `n + s_max` voxels per axis |
| `field` | `--fout` | `True` | the field in Hz |
| `corrected` | `--iout` | `False` | the corrected volumes and their mean (`_mean`) |
| `feat` | `--featout` | `False` | the field in rad/s and the corrected mean, for FEAT |
| `displacement_fields` | `--dfout` | `False` | each volume's displacement field (mm) |
| `rigid_body_matrices` | `--rbmout` | `False` | each volume's rigid-body matrix |
| `jacobians` | `--jacout` | `False` | each volume's Jacobian determinant |
| `verbose` | `-v` | `False` | the cost of every evaluation (in `TopupResult.stdout`) |
| `n_threads` | `--nthr` | 1 | threads; the results are FSL's one-thread results at any count (FSL-116) |
| `implementation` | `LARMORX_IMPLEMENTATION` | `"auto"` | `"auto"`, `"replica"`, `"original"` |
| `upstream_defects` | `LARMORX_UPSTREAM_DEFECTS` | `"fix"` | `"reproduce"`: FSL's defects as FSL has them (below) |

`None` (the default of every estimation setting) leaves the option out, so the configuration
file, or topup's own default, sets it; a value given wins over the configuration, as on topup's
command line. `lx.fsl.topup` returns a `TopupResult`: `coefficients` (`_fieldcoef`),
`movement_parameters` (`_movpar.txt`: one row per volume, three translations in mm and three
rotations in radians), `field`, `corrected`, `corrected_mean`, `feat_fieldmap_rads`,
`feat_fieldmap_mag`, `displacement_fields`, `rigid_body_matrices` (4 × 4 arrays), `jacobians`
(`None` when not asked for), `log` (the `--logout` text) and `stdout`. The hidden debugging
options (`--debug`, `--trace`, `--profile`) are available on the command line.

### `lx.fsl.applytopup`

| `lx.fsl.applytopup` | `applytopup` | Default | Meaning |
|---|---|---|---|
| `images` | `--imain` | | one image or a list of them (3D or 4D, one grid) |
| `acquisition` | `--datain` | | as topup's |
| `indices` | `--inindex` | | each image's row of `acquisition` (1-based) |
| `topup_result` | `--topup` | | a `TopupResult`, a field image in Hz, or the base name of topup's files (without extension) |
| `method` | `--method` | `"lsr"` | `"lsr"` (least-squares restoration of opposite phase encodings), `"jac"` (Jacobian modulation), `"vb2D"`, `"vb3D"`, `"vb4D"` (undocumented variational-Bayes restorations) |
| `interp` | `--interp` | `"spline"` | or `"trilinear"` |
| `datatype` | `--datatype` | float | `"preserve"`, `"char"`, `"short"`, `"int"`, `"float"`, `"double"` |
| `verbose` | `-v` | `False` | (applytopup prints nothing more) |
| `n_threads` | `OMP_NUM_THREADS` | 1 | threads of the resampling; the results do not depend on it |
| `implementation`, `upstream_defects` | | | as topup's |

It returns an `ApplytopupResult`: `corrected` (an `lx.Image`) and `stdout` (the vb methods'
iterations). FSL's settings are the call's (CLAUDE.md rule 6): the outputs are NIfTI-1 read
back, and no FSL variable of the process reaches the program.

**FSL's settings files without FSL.** larmorx-nc carries FSL 6.0.7.23's six `topup` settings
files byte for byte (`b02b0.cnf`, `b02b0_1.cnf`, `b02b0_2.cnf`, `b02b0_4.cnf`, `b02b0_7T.cnf`,
`test_acqp.cnf`, from fsl-topup 2203.5; the user's decision of 2026-11-09), installed by its wheel
in `<prefix>/share/larmorx-nc/fsl-data/etc/flirtsch/`: the replica program (`larmorx-nc fsl topup
--config=b02b0`) looks for a name without a directory there after FSL's own places (as named,
with `.cnf`, `$FSLDIR/etc/flirtsch/`), so the name works without an FSL installation; when
neither has it, FSL's messages (and FSL-121's) are unchanged.

**larmorx's own settings** (2026-11-11, the user's decision of 2026-11-09). The clean-room
original carries its own settings of the same six names, written from `specs/topup.md` §2.5 by an
implementer who never opened FSL's files (`crates/larmorx-fsl/src/topup/settings/`, Apache-2.0,
compiled into `larmorx fsl topup`). It finds a name the same way, after FSL's own places (as
named, with `.cnf`, `$FSLDIR/etc/flirtsch/`), FSLDIR unset included: so `config="b02b0"` (or
`b02b0_1`, `_2`, `_4`, `b02b0_7T`, `test_acqp`, with or without `.cnf`) needs no FSL and no
larmorx-nc, and `fsldir` is needed only for other names. Where an FSL installation holds the
file, FSL's file is read, as FSL does; larmorx-nc's copy is read only by the replica. The values
are FSL's: every output of topup is bit for bit the same with larmorx's settings as with FSL's
files, for the replica program (FSL's own results) and for the original, on fMRIPrep's inputs of
ds005040 and ds006736 and a synthetic pair with movements (54 of 54 comparisons; 234 runs with one
value changed all differ:
[`validation/investigations/topup-settings-original/`](../../validation/investigations/topup-settings-original/README.md)).

**File names on Windows.** FSL finds a name's directory and extension at the last `/`. On Windows,
where FSL does not run, `\` separates directories as well. So a configuration given without
`.cnf`, and applytopup's `--topup` base name, behave as on Linux when a directory's name holds a dot
(`C:\Users\...\.tmp1\b02b0`). On Linux and macOS, `\` stays an ordinary character, as in FSL.

## The clean-room original

Every option, message, exit status and failure case of both programs (the spec's §2 to §10):
the configuration files, both minimisers per level, both regularisers, both spline orders, the
precisions and interpolations, the movements, every output, `--debug`, `--trace` and
`--profile`; applytopup's `jac`, `lsr`, `vb2D`, `vb3D` and `vb4D`. Its results never depend on
the thread count (`n_threads`, `--nthr`: deterministic sums), and correctly rounded `sin`,
`cos` and `exp` give the same bits on every platform. Built for speed (§11; the benchmark
[fsl-topup](../benchmarks/fsl-topup.md), informal, one run each):

| Call | FSL (1 thread) | replica | original, 1 thread | original, 14 threads |
|---|---|---|---|---|
| fMRIPrep's topup, ds005040 | 32.6 s | 29.7 s | 2.8 s | 0.8 s |
| fMRIPrep's topup, ds001600 | 77.4 s | 71.4 s | 7.7 s | 2.0 s |
| applytopup `lsr`, ds005040's pair | 0.42 s | 3.47 s | 0.13 s | 0.04 s (0.03 s with the matrices row by row, 2026-11-04: `lsr_speed.py`) |

Where the spec leaves a convention open, the original follows FSL's as measured: an oblique
phase encoding wraps along no axis; the scaled conjugate gradients' second gradient keeps the
current point's regulariser weight; `--debug`'s images are written in FSL's order; the mask's
frame follows each volume's phase encoding (a volume with a read-out time of 0 included) and
holds for moved volumes too. Both modes follow FSL's spline prefilter (its truncated start, which
decides the interpolation on axes shorter than 15 voxels), its regulariser's sampling of quadratic
B-splines and applytopup's resampling matrices built row by row. By default it corrects FSL where
FSL's derivatives of the movements are not its cost's (FSL-201: the original's are exact, and its
result moves in proportion to a change of its inputs' last bits, where FSL's jumps by up to 1e-2)
and where FSL-133 makes FSL's restoration wrong; `reproduce` gives FSL's derivatives and FSL's
read.

## Upstream defects (D13)

topup's and applytopup's defects are corrected by default and reproduced with
`upstream_defects="reproduce"` (`LARMORX_UPSTREAM_DEFECTS=reproduce`); each correction that
changes a result is named on standard error and as an `UpstreamDefectWarning`
([all of them](upstream-defects.md)):

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
  voxel, in both implementations since 2026-11-04).
- **FSL-134**: applytopup with a field image scales the y displacement by the x voxel size, and
  takes fields on other grids: corrected, refused (`reproduce`: FSL's scales, and a field of as
  many voxels as the images in another shape laid as stored, its Jacobian at the same indices;
  in the original since 2026-11-04).
- **FSL-273**: applytopup `--method=jac` divides the field's derivatives by the field's own voxel
  sizes while it applies the displacement at the images' voxel indices, so a field (coefficients
  or a field image) of the images' shape whose voxel sizes differ from the images' gets a
  Jacobian scaled by their ratio: corrected (the Jacobian of the displacement applied; the same
  result wherever the voxel sizes agree), named when the voxel sizes differ along an encoded
  axis (`reproduce`: FSL's scales; the replica since 2026-11-06, the original since
  2026-11-07).
- **FSL-135**: an unknown `--method` writes nothing and exits 0; a restoration without movements
  aborts: refused clearly.
- **FSL-141** to **FSL-143**: the vb methods' counts in 32-bit integers, `vb3D`/`vb4D`'s count of
  data points and their prior across slices (indefinite: FSL's iterations diverge): corrected.
- **FSL-144**: `--method=jac` with a phase encoding along z (which topup does not estimate)
  leaves the image uncorrected silently: refused (both implementations; the replica since
  2026-11-03).
- **FSL-145**: `--splineorder=2` lays two coefficients on an axis shorter than two knot
  spacings (one on an axis shorter than one spacing, whose refit between levels or for the
  outputs stops topup), so the field is forced towards 0 at its far end: the full count by
  default (both implementations; applytopup reads a coefficient file of either count by default
  and FSL's only with `reproduce`, where FSL aborts on the other).
- **FSL-146**: with volumes encoded along different axes (AP, PA, LR, RL) the gradient is not
  the cost's: corrected, the cost's own (both implementations; the volumes left out are those
  whose encoding along the axis is 0, whatever their read-out time).
- **FSL-201**: with the movements estimated, the derivatives with respect to the rotations are
  forward differences of rotation matrices built in single precision, so the minimisers end
  elsewhere than the cost's minimum and FSL's path depends on the last bits of its inputs: the
  exact derivatives by default (both implementations; `reproduce` FSL's matrices and forward
  differences, in the original since 2026-11-04; fMRIPrep's calls, which estimate no movements,
  unchanged).
- **FSL-242**: with `--minmet=1` (scaled conjugate gradients), a level whose last iteration's
  step is rejected (its cost above the current point's) ends at that rejected, worse point, from
  which the next level or the outputs start: the lowest cost the level found by default
  (the clean-room original since 2026-11-05; `reproduce` FSL's; named on every run where it
  happens). `--miter` counts every iteration, the rejected ones too, in both modes (FSL's, observed
  with `-v`).

### glibc's maths functions (GLIBC-07)

FSL's topup takes its smoothing's Gaussian taps from glibc 2.35's `exp` (`specs/topup.md` §4), which
is not correctly rounded. The clean-room original computes it correctly rounded by default and takes
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
- The full tier, the data panel's PEPOLAR runs and long random testing: in the final validation
  phase (the record's standard tier holds fMRIPrep's calls on its two oracle runs; the site's page,
  `/tools/fsl-topup/`, shows them with every case of the record).
