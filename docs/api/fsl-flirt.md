# FLIRT (`flirt`, `convert_xfm`, `avscale`, the coordinate programs, `rmsdiff`, `makerot`, `midtrans`, `pointflirt`, `applyxfm4D` and the scripts)

`lx.fsl.flirt` and `lx.fsl.convert_xfm` (command line `larmorx fsl flirt`, `larmorx fsl
convert_xfm`): FSL 6.0.7.23's linear (affine) registration, with every cost function and
boundary-based registration (BBR), and its matrix tool. Since part 2 (2026-11-01) also FLIRT's
other programs `avscale`, `img2imgcoord`, `std2imgcoord`, `img2stdcoord`, `rmsdiff` and
`makerot` (`lx.fsl.avscale`, ... below), and since part 3 `midtrans`, `pointflirt`, `applyxfm4D`
and the scripts `pairreg`, `extracttxt`, `aff2rigid` (`lx.fsl.midtrans`, ... below), with every
schedule command (`alignpaxes` and any parameter-subset mask) and `-debug`.

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
| `-v`, `-verbose`, `-printinit`, `-debugsave`, `-debug`, `-i`, `-help`, `-version`, `-anglerep` | command line only | `-anglerep quaternion` since part 2 |
| `OMP_NUM_THREADS` | `n_threads` | the results do not depend on it |
| `LARMORX_UPSTREAM_DEFECTS` | `upstream_defects` | `fix` (default) or `reproduce` |

`convert_xfm`: `-omat` (the result), `-concat` (`concat`), `-fixscaleskew` (`fix_scale_skew`),
`-inverse` (`inverse`), `-v`, `-verbose`, `-help` (command line).

## FLIRT's other programs (part 2)

Each is the replica of FSL 6.0.7.23's program (every option, message, exit status and failure,
`specs/flirt.md` §15), run as a separate program; its Python function returns what the program
prints, parsed (FSL prints 6 decimals or 6 significant digits, and so do these).

```python
m = np.loadtxt("bold2t1.mat")
d = lx.fsl.avscale(m, "boldref.nii.gz", all_params=True)    # avscale --allparams
d.scales, d.rotations, d.determinant, d.forward_half
xyz = lx.fsl.img2imgcoord([[10, 20, 30]], "boldref.nii.gz", "T1w.nii.gz", xfm=m)   # voxels
mm = lx.fsl.img2stdcoord(xyz, "T1w.nii.gz", "MNI152_T1_2mm.nii.gz", xfm=t1_to_mni, mm=False)
lx.fsl.rmsdiff(m, np.eye(4), "boldref.nii.gz")               # mm, 80 mm about its centre
r = lx.fsl.makerot(30, axis=(1, 2, 3), centre=(4, 5, 6))      # 4 x 4, 30 degrees
```

| Program | Command line | `lx.fsl` | Notes |
|---|---|---|---|
| `avscale [--allparams/--inverteddies] matrix [volume]` | `larmorx fsl avscale` | `avscale(matrix, volume=None, *, all_params, inverted_eddies)` → `AvscaleResult` | the rotation-and-translation matrix, angles and translations (about the volume's centre of gravity), scales, skews, their average, the determinant and left-right order, the half transforms (the matrix's square root), `--inverteddies`' matrices |
| `img2imgcoord -src -dest [-xfm/-premat] [-vox/-mm] [-v] coords` | `larmorx fsl img2imgcoord` | `img2imgcoord(coords, src, dest, *, xfm, mm)` → N × 3 | voxels by default; `-` or no file: standard input (refused at a terminal, as FSL) |
| `std2imgcoord -img [-std] [-xfm] [-mm/-vox] [-v/-verbose] coords` | `larmorx fsl std2imgcoord` | `std2imgcoord(coords, img, std=None, *, xfm, vox)` → N × 3 | millimetres by default; the matrix inverted |
| `img2stdcoord -img [-std] [-xfm] [-vox/-mm] [-v/-verbose] coords` | `larmorx fsl img2stdcoord` | `img2stdcoord(coords, img, std=None, *, xfm, mm)` → N × 3 | input voxels by default; output in the standard image's mm |
| `rmsdiff m1 m2 refvol [mask]` | `larmorx fsl rmsdiff` | `rmsdiff(m1, m2, ref, mask=None)` → float or (largest, rms) | the RMS deviation over 80 mm about the reference's centre of gravity; with a mask, the largest and RMS displacement over it |
| `makerot -t angle [-a ax,ay,az] [-c cx,cy,cz] [--cov=image] [-o file]` | `larmorx fsl makerot` | `makerot(theta, axis=None, centre=None, *, cov=None)` → 4 × 4 | a rotation by `theta` degrees about the axis through the centre (or the image's centre); the axis divided by its length (FSL by its square root: **FSL-76**, corrected by default); an axis or centre of other than three values refused (**FSL-77**) |

The coordinate programs' `-warp` (a FNIRT warp field) is refused until FNIRT is ported (an
unreadable file gives FSL's messages and exit status 1, as FSL); matrices other than 4 × 4 where
FSL would invert them beyond 4 × 4 are refused (FSL prints part of its output, then aborts).

## FLIRT's other programs and scripts (part 3)

```python
mid = lx.fsl.midtrans(["A2temp.mat", "B2temp.mat", "C2temp.mat"])    # 4 x 4 to the middle
aff = lx.fsl.pointflirt(in_points, ref_points)                         # N x 3 each, mm
lx.fsl.applyxfm4d("bold.nii.gz", "boldref.nii.gz", "out.nii.gz", "mats_dir")  # MAT_0000n
pair = lx.fsl.pairreg("b1_brain.nii.gz", "b2_brain.nii.gz", "b1.nii.gz", "b2.nii.gz")
text = lx.fsl.extracttxt("value", "log.txt", 3, -1)
rigid = lx.fsl.aff2rigid("T1w2MNI.mat")                                 # 6-parameter 4 x 4
```

| Program | Command line | `lx.fsl` | Notes |
|---|---|---|---|
| `midtrans [-o file] [--template img] [--separate prefix] [--debug] [-v] t1 t2 ...` | `larmorx fsl midtrans` | `midtrans(transforms, *, template, separate)` → 4 × 4 | the transform from the template space to the "middle" of the transforms (FSL's Procrustes average about the MNI origin or the template's centre of gravity); `--separate` writes each transform composed with it; an error after the arguments exits 1 (FSL 0: **FSL-78**) |
| `pointflirt -i file -r file [-o file] [--vox --invol=img --refvol=img] [-v]` | `larmorx fsl pointflirt` | `pointflirt(in_points, ref_points, *, in_image, ref_image)` → 4 × 4 | the least-squares affine matrix mapping the input points onto the reference points (mm; voxels scaled by the images' voxel sizes with `--vox`); a warning when the points do not determine it; a file of exactly three points is read with one point per column (as FSL) |
| `applyxfm4D in ref out mats [-singlematrix] [-fourdigit] [-userprefix p] [-interp m] [-3D] [-v]` | `larmorx fsl applyxfm4D` | `applyxfm4d(in, ref, out, transforms, *, single_matrix, four_digit, user_prefix, interp, three_d)` | each volume resampled onto the reference's grid through one matrix or one per volume (`MAT_0000n`, `MAT_nnnn`, `<prefix>nnnn`), sinc (default), trilinear, nearest neighbour or spline, in the input's data type (char, short, int, float, double); integer outputs rounded and clamped (**FSL-09**); spline on integer images computed as on float ones (**FSL-79**); `-3D` sinc with the rectangular kernel the program asks for (**FSL-80**) |
| `pairreg brain1 brain2 skull1 skull2 outmat [flirt args]` | `larmorx fsl pairreg` | `pairreg(brain1, brain2, skull1, skull2, *, flirt_args)` → 4 × 4 | brain-to-brain registration refined with the skulls (three FLIRT runs with FSL's `pairreg` schedules, in-process) |
| `extracttxt word file [count [start]]` | `larmorx fsl extracttxt` | `extracttxt(word, file, trailing_lines, relative_start)` → str | the lines from the first match (a basic regular expression), or `count` of them from `start` lines after it; FSL's messages for ranges before the first line |
| `aff2rigid in.mat out.mat` | `larmorx fsl aff2rigid` | `aff2rigid(matrix)` → 4 × 4 | the 6-parameter matrix keeping the anterior commissure, the AC-PC line and the mid-sagittal plane of an input-to-standard matrix; FSL 6.0.7.23's script stops with a Python error before writing (**FSL-81**): computed by default, FSL's error with `reproduce` |

`flirt`'s schedules may now use `alignpaxes` (the rotation between the two images' principal axes,
with the centres of gravity aligned) and `setoption paramsubset` with any finite mask (its
pseudo-inverse by the SVD, as FSL's); `-debug` writes FLIRT's debug volumes (the pyramid, the
weights and, with `-cost bbr`, BBR's `debug` volume).

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

Part 3's programs: **FSL-78** (`midtrans` exits 0 after an error of its computation: exit 1 by
default), **FSL-79** (`applyxfm4D`'s spline interpolation of an integer image keeps the spline
coefficients and values in the image's type: computed as for a float image by default, named
where the output changes), **FSL-80** (`applyxfm4D -3D` with sinc uses a blackman kernel of width
7 instead of the rectangular one of width 9 it asks for: the rectangular kernel by default, named
where the output changes), **FSL-81** (`aff2rigid` stops with a Python `NameError` under NumPy 2:
the matrix computed by default). **FSL-75** also in `applyxfm4D` (a volume 3 voxels wide) and in
mcflirt's replica (named in both modes, the whole border by default).

Refused in both modes (FLIRT's behaviour is undefined): FSL-21/FSL-66 (`-bins` giving no or negative
bins at a scale), FSL-70 (a schedule matrix name FLIRT cannot interpret), FSL-71 (`bbrstep` below 1
with BBR: FLIRT never finishes), FSL-72 (a schedule `read` of a file that is not numbers: FLIRT reads
for ever), FSL-73 (`-pedir` without a field map read), the background value of a single voxel
(**FSL-75**: FSL reads an empty buffer; `larmorx-nc: error: FLIRT's background value of a 1 x 1 x 1
volume reads the first value of an empty buffer (FSL-75)`). **FSL-75** otherwise (part 2: the
background value of an image 3 voxels wide along x, or along y with x neither 2 nor 4, which the
pyramid meets on small images; FSL writes past its buffer and takes the percentile of the values
that fit; it may abort, crash or finish): corrected by default (the percentile of the whole
border), FSL's values with `reproduce`, named in both modes (`upstream defect FSL-75 (flirt)
corrected: the background value of a 3 x 3 x 3 pyramid level: FLIRT writes 36 border values into a
buffer of 28 ...`). **FSL-22** (NaN voxels in a border whose background value FLIRT computes: no
defined order in its sort): sorted last by default, libstdc++'s order (FSL's build's) with
`reproduce`, named, as mcflirt's replica does. Kept and documented:
FSL-67 (BBR's normals scaled by the reference level's voxel sizes; measured in part 2: computing
them in millimetres moves fMRIPrep's BBR results by 0.02 mm on average and brings no gain on a
strongly anisotropic case with a known motion: rule C), FSL-69 (a coarse `setscale` the data
cannot take leaves the previous level in place with the unblurred input). FSL-14 is named whenever
an input holds NaN or infinite voxels; FSL-09 whenever an integer output is not an integer or out
of its type's range.

## Not yet

The coordinate programs' `-warp` (FNIRT's warp fields, refused until FSL's warp-field libraries are
ported); `standard_space_roi`, `flirt_average` and `epi_reg` (scripts of FSL's other programs: not
ported); the clean-room original. Known difference (part 3's random testing): with an infinite
input voxel, `reproduce` (FSL-14) and a schedule that optimises relative to a row, an all-NaN
matrix is written as `nan` where FSL writes `-nan`.
