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
(2026-11-03), and the clean-room **original** in the main package (Apache-2.0), to be written
from [`specs/topup.md`](../../specs/topup.md). Until then both functions run the replica only:
`implementation="auto"` (the default) and `"replica"` need `larmorx-nc` installed (`pip install
larmorx-nc`; otherwise `ReplicaNotFoundError`), `"original"` raises `NotImplementedError`. The
command line chooses alike (`LARMORX_IMPLEMENTATION`).

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

# FSL's own configuration by name, then the correction of a DWI pair by least squares
r = lx.fsl.topup("b0_pair.nii.gz", acq, config="b02b0.cnf", fsldir="/opt/fsl")
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
| `config` | `--config` | | a configuration file, or the name of one of FSL's (`"b02b0.cnf"`, `"b02b0_2.cnf"`, ...) with `fsldir` |
| `fsldir` | `FSLDIR` | | the FSL installation whose `etc/flirtsch` holds FSL's configurations (only for a `config` given by name) |
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
| `implementation` | `LARMORX_IMPLEMENTATION` | `"auto"` | `"auto"`, `"replica"` (`"original"`: not yet) |
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
- **FSL-123** to **FSL-126**: a negative `--miter`, `--warpres`, `--fwhm` or `--lambda`: refused.
- **FSL-132**: applytopup resamples the first volume trilinearly whatever `--interp` asks: corrected.
- **FSL-133**: applytopup's restoration wraps a line wrongly for lengths that do not divide 2³²:
  corrected.
- **FSL-134**: applytopup with a field image scales the y displacement by the x voxel size, and
  takes fields on other grids: corrected, refused.
- **FSL-135**: an unknown `--method` writes nothing and exits 0; a restoration without movements
  aborts: refused clearly.
- **FSL-141** to **FSL-143**: the vb methods' counts in 32-bit integers, `vb3D`/`vb4D`'s count of
  data points and their prior across slices (indefinite: FSL's iterations diverge): corrected.

## Not yet

- The clean-room originals (`implementation="original"`), from `specs/topup.md`.
- applytopup's restorations of lines of fewer than 8 or more than 256 voxels along the phase
  encoding, and band systems that need LAPACK's blocked band factorisation, are refused in both
  modes with a clear message (`larmorx-nc: ... not replicated`): FSL's arithmetic there follows
  other OpenBLAS and Armadillo paths, which the replica does not reproduce yet.
- The full tier, the data panel's PEPOLAR runs and long random testing: in the final validation
  phase (the record's standard tier holds fMRIPrep's calls on its two oracle runs; the site's page,
  `/tools/fsl-topup/`, shows them with every case of the record).
