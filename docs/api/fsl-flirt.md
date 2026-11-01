# FLIRT (`flirt`, `convert_xfm`)

`lx.fsl.flirt` and `lx.fsl.convert_xfm` (command line `larmorx fsl flirt`, `larmorx fsl
convert_xfm`): FSL 6.0.7.23's linear (affine) registration, with every cost function and
boundary-based registration (BBR), and its matrix tool.

**Two implementations, one so far.** FSL is distributed under the FSL Licence (non-commercial use
only), so larmorx keeps its FSL tools in two implementations ([licensing](../licensing.md)): the
bit-exact **replica**, `larmorx-nc fsl flirt|convert_xfm` (package `larmorx-nc`, `crates-nc/`, under
the FSL Licence's terms; translated from FSL's source:
[PROVENANCE](../../crates-nc/larmorx-nc-flirt/PROVENANCE.md)), done (part 1, 2026-11-01), and the
clean-room **original** in the main package (Apache-2.0), to be written from
[`specs/flirt.md`](../../specs/flirt.md). Until the original exists, `implementation="auto"` runs the
replica and needs `larmorx-nc` installed (`pip install larmorx-nc`; without it a clear error names
the package); `"original"` is an error that says so. The command line chooses alike
(`LARMORX_IMPLEMENTATION`).

**Status: `experimental`** (part 1): the replica against FSL 6.0.7.23 itself
([record](../validation/fsl-flirt-replica.md), standard tier): fMRIPrep's boundary-based calls on
three oracle runs (ds005040, ds003592, ds000228) bit-identical, their matrices also identical as
text to fMRIPrep's own (its container's FSL); every cost function, search option, schedule FSL
ships, `-applyxfm` 3D and 4D with every interpolation and data type, the BBR variants and the
command line's errors. What waits for part 2 is listed below.

## Quick start

```python
import larmorx as lx

# fMRIPrep's boundary-based registration of a BOLD reference to the T1w (no FreeSurfer).
r = lx.fsl.flirt(
    "boldref.nii.gz", "T1w_brain.nii.gz",
    init="init.mat", cost="bbr", search_cost="bbr", dof=6, base_scale=1,
    schedule="/path/to/fsl/etc/flirtsch/bbr.sch", wm_seg="wm_mask.nii.gz",
)
r.matrix     # 4 x 4, input FSL mm -> reference FSL mm (as -omat writes it)
r.image      # the BOLD reference on the T1w's grid

# A default 12-dof registration, then the inverse matrix.
r = lx.fsl.flirt("epi.nii.gz", "T1w.nii.gz")
inv = lx.fsl.convert_xfm(r.matrix, inverse=True)
```

## Options (`flirt`)

| FLIRT option | `lx.fsl.flirt` | Notes |
|---|---|---|
| `-in`, `-ref` | `in_image`, `ref_image` | paths or `lx.Image` |
| `-out`, `-omat` | `out=True` (the image), the matrix always | returned in `FlirtResult` |
| `-init` | `init` | a 4 x 4 array or a `.mat` file |
| `-applyxfm`, `-applyisoxfm s` | `apply_xfm=True`, `apply_isoxfm=s` | need `init` (or `use_sqform`) |
| `-usesqform` | `use_sqform` | |
| `-dof 6/7/9/12` | `dof` | |
| `-cost`, `-searchcost` | `cost`, `search_cost` | `corratio` (default), `mutualinfo`, `normmi`, `normcorr`, `leastsq`, `labeldiff`, `bbr`, `woods` |
| `-bins` | `bins` | 256 |
| `-interp` | `interp` | `trilinear`, `nearestneighbour`, `sinc`, `spline` |
| `-sincwidth`, `-sincwindow` | `sinc_width`, `sinc_window` | 7, `hanning` |
| `-searchrx/ry/rz a b`, `-nosearch` | `search_rx`, `search_ry`, `search_rz`, `no_search` | degrees |
| `-coarsesearch`, `-finesearch` | `coarse_search`, `fine_search` | 60, 18 degrees |
| `-schedule` | `schedule` | a schedule file |
| `-datatype` | `datatype` | `char`, `short`, `int`, `float`, `double` |
| `-refweight`, `-inweight` | `ref_weight`, `in_weight` | |
| `-wmseg`, `-wmcoords`, `-wmnorms` | `wm_seg`, `wm_coords`, `wm_norms` | BBR's boundary |
| `-fieldmap`, `-fieldmapmask`, `-pedir`, `-echospacing` | `fieldmap`, `fieldmap_mask`, `pe_dir`, `echo_spacing` | BBR with a field map |
| `-bbrtype`, `-bbrslope` | `bbr_type`, `bbr_slope` | `signed`, `global_abs`, `local_abs`; −0.5 |
| `-paddingsize`, `-setbackground` | `padding_size`, `background` | |
| `-noresample`, `-noclamp`, `-noresampblur`, `-forcescaling` | `no_resample`, `no_clamp`, `no_resample_blur`, `force_scaling` | |
| `-minsampling`, `-basescale` | `min_sampling`, `base_scale` | |
| `-2D` | `two_d` | |
| `-v`, `-verbose`, `-printinit`, `-debugsave`, `-debug`, `-i`, `-help`, `-version`, `-anglerep` | command line only | `-anglerep quaternion`: part 2 |
| `OMP_NUM_THREADS` | `n_threads` | the results do not depend on it |
| `LARMORX_UPSTREAM_DEFECTS` | `upstream_defects` | `fix` (default) or `reproduce` |

`convert_xfm`: `-omat` (the result), `-concat` (`concat`), `-fixscaleskew` (`fix_scale_skew`),
`-inverse` (`inverse`), `-v`, `-verbose`, `-help` (command line).

## Upstream defects (D13)

Corrected by default, FSL's behaviour with `upstream_defects="reproduce"`
([register](../findings/upstream-defects.md)):
- **FSL-09**: integer outputs rounded to the nearest and clamped (FLIRT truncates; int8 kept as
  int8, FLIRT writes uint8).
- **FSL-11**: `FSLOUTPUTTYPE` unset writes `.nii.gz`.
- **FSL-14**: non-finite voxels left out of the costs (one NaN makes FLIRT's cost NaN).
- **FSL-56**: NIfTI extensions' offset.
- **FSL-68**: an error after the command line (an image or matrix that cannot be read, an unknown
  `-bbrtype`) exits with status 1 (FLIRT prints the message and exits 0).

Refused in both modes (FLIRT's behaviour is undefined): FSL-21/FSL-66 (`-bins` giving no or negative
bins at a scale), FSL-70 (a schedule matrix name FLIRT cannot interpret), FSL-71 (`bbrstep` below 1
with BBR), FSL-72 (a schedule `read` of a file that is not numbers), FSL-73 (`-pedir` without a field
map read). Kept and documented: FSL-67 (BBR's normals scaled by the reference level's voxel sizes;
to be measured), FSL-69 (a coarse `setscale` the data cannot take leaves the previous level in place
with the unblurred input).

## Not yet (part 2)

`-anglerep quaternion`; the schedule command `alignpaxes`; the exact last bits of the
pseudo-inverse for parameter-subset masks other than FLIRT's own and FSL's shipped schedules'; the
BBR debug volume of `-debug`; the clean-room original; the other programs of fsl-flirt
(`avscale`, `makerot`, `midtrans`, `rmsdiff`, `img2imgcoord`, `img2stdcoord`, `std2imgcoord`,
`applyxfm4D`, `pointflirt`); random testing's descriptor; the site page.
