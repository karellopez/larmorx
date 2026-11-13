# FLIRT (`flirt`, `convert_xfm`, `avscale`, the coordinate programs, `rmsdiff`, `makerot`, `midtrans`, `pointflirt`, `applyxfm4D` and the scripts)

`lx.fsl.flirt` and `lx.fsl.convert_xfm` (command line `larmorx fsl flirt`, `larmorx fsl
convert_xfm`): FSL 6.0.7.23's linear (affine) registration, with every cost function and
boundary-based registration (BBR), and its matrix tool. Also FLIRT's other programs `avscale`,
`img2imgcoord`, `std2imgcoord`, `img2stdcoord`, `rmsdiff`, `makerot`, `midtrans`, `pointflirt`,
`applyxfm4D` and the scripts `pairreg`, `extracttxt`, `aff2rigid` (`lx.fsl.avscale`, ...
below), with every schedule command (`alignpaxes` and any parameter-subset mask) and `-debug`.

The ports come first. One section per program follows: `flirt`, `convert_xfm`, `avscale`, the
three coordinate programs, `rmsdiff`, `makerot`, `midtrans`, `pointflirt`, `applyxfm4D` and the
scripts `pairreg`, `extracttxt` and `aff2rigid`. Each program's section is the source of its
reference page on the documentation site (`python -m larmorx_validation reference`), with its
options table checked against the code. What the programs share comes after them, and the page's dates are in
its [History](#history).

## Ports

FSL's licence allows non-commercial use only, so each of FLIRT's programs has two ports, a
replica and a clean room, in different packages ([licensing](../licensing.md) says what each
package holds):

- **The replica** is translated from FSL's source. The package `larmorx-nc` holds it (`crates-nc/`),
  under the FSL Licence's terms, and `larmorx` runs it as a separate program, `larmorx-nc fsl
  <program>`, when `larmorx-nc` is installed (`pip install larmorx-nc`).
- **The clean room** is written from [`specs/flirt.md`](../../specs/flirt.md) by an implementer who
  never read FSL's source or the replica. It is part of `larmorx`, under Apache-2.0
  (`crates/larmorx-fsl`, with its [PROVENANCE](../../crates/larmorx-fsl/PROVENANCE.md), section
  "FLIRT"), and runs on LARMORX's clean-room registration engine, the head-motion correction's
  (`lx.mri.hmc`).

`implementation="auto"` runs the replica when `larmorx-nc` is installed, and the clean room
otherwise. `"replica"` and `"clean-room"` choose one (`"original"`, 0.1.0rc1's spelling of
`"clean-room"`, still works); `"clean-room"` runs the clean room in this process. On the command
line, `--implementation auto|replica|clean-room` before the family chooses alike, from the next
release; in 0.1.0rc1 the variable `LARMORX_IMPLEMENTATION` chooses (`original` for the clean
room).

## `flirt`

<!-- reference
program: flirt
group: fsl
original: FSL 6.0.7.23
python: larmorx.fsl.flirt
command-line: crates/larmorx-fsl/src/cli/flirt.rs
records: {replica: [fsl-flirt-replica], clean-room: [fsl-flirt]}
evidence: fsl-flirt
-->

`flirt` registers one image to another with an affine transform, with every cost function and
boundary-based registration (BBR). It finds a transform of 6, 7, 9 or 12 degrees of freedom, or
applies a given matrix (`-applyxfm`, `-applyisoxfm`), and writes the 4 × 4 matrix from the
input's FSL millimetres to the reference's (`-omat`) and, on request, the input resampled onto the
reference's grid (`-out`).

Its schedules may use `alignpaxes` (the rotation between the two images' principal axes, with the
centres of gravity aligned) and `setoption paramsubset` with any finite mask (its pseudo-inverse
by the SVD, as FSL's); `-debug` writes FLIRT's debug volumes (the pyramid, the weights and, with
`-cost bbr`, BBR's `debug` volume).

### Python (`flirt`)

`lx.fsl.flirt(in_image, ref_image, *, init=None, dof=12, cost="corratio", search_cost="corratio", bins=256, interp="trilinear", sinc_width=7, sinc_window="hanning", search_rx=(-90.0, 90.0), search_ry=(-90.0, 90.0), search_rz=(-90.0, 90.0), no_search=False, coarse_search=60.0, fine_search=18.0, schedule=None, apply_xfm=False, apply_isoxfm=None, use_sqform=False, out=True, datatype=None, ref_weight=None, in_weight=None, wm_seg=None, wm_coords=None, wm_norms=None, fieldmap=None, fieldmap_mask=None, pe_dir=None, echo_spacing=0.0005, bbr_type="signed", bbr_slope=-0.5, padding_size=0.0, background=None, no_resample=False, no_clamp=False, no_resample_blur=False, force_scaling=False, min_sampling=None, base_scale=None, two_d=False, n_threads=1, implementation="auto", upstream_defects="fix")`

Images are paths or `lx.Image` objects. `init` is a 4 × 4 matrix (an array or a `.mat` file),
`schedule` a schedule file. The result's `matrix` maps the input's FSL millimetres to the
reference's, as `-omat` writes it; its `image` is the input on the reference's grid (`None`
unless `out`); its `stdout` holds FLIRT's verbose lines and a schedule's `print`.

fMRIPrep's boundary-based registration of a BOLD reference to the T1w (no FreeSurfer), with
FSL's `bbr.sch` (in `$FSLDIR/etc/flirtsch`):

```python
r = lx.fsl.flirt(
    "boldref.nii.gz", "T1w_brain.nii.gz",
    init="init.mat", cost="bbr", search_cost="bbr", dof=6, base_scale=1,
    schedule="/path/to/fsl/etc/flirtsch/bbr.sch", wm_seg="wm_mask.nii.gz",
)
r.matrix     # 4 x 4, input FSL mm -> reference FSL mm (as -omat writes it)
r.image      # the BOLD reference on the T1w's grid
```

### Options of `flirt`

| FLIRT option | Python | Notes |
|---|---|---|
| `-in`, `-ref` | `in_image`, `ref_image` | paths or `lx.Image` |
| `-out`, `-omat` | `out=True` (the image), the matrix always | returned in `FlirtResult`; `-o` is `-out` |
| `-init` | `init` | a 4 x 4 array or a `.mat` file |
| `-applyxfm`, `-applyisoxfm s` | `apply_xfm=True`, `apply_isoxfm=s` | need `init` (or `use_sqform`) |
| `-usesqform` | `use_sqform` | |
| `-dof 6/7/9/12` | `dof` | |
| `-cost`, `-searchcost` | `cost`, `search_cost` | `corratio` (default), `mutualinfo`, `normmi`, `normcorr`, `leastsq`, `labeldiff`, `bbr`, `woods` |
| `-bins` | `bins` | 256 |
| `-interp` | `interp` | `trilinear`, `nearestneighbour`, `sinc`, `spline` |
| `-sincwidth`, `-sincwindow` | `sinc_width`, `sinc_window` | 7, `hanning` |
| `-searchrx a b`, `-searchry a b`, `-searchrz a b`, `-nosearch` | `search_rx`, `search_ry`, `search_rz`, `no_search` | degrees |
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
| `-v`, `-verbose`, `-printinit`, `-displayinit`, `-debugsave`, `-debug`, `-i`, `-help`, `-version`, `-anglerep`, `-applynonisoxfm`, `-nosave` | command line only | `-anglerep quaternion` since 2026-11-02 |
| `OMP_NUM_THREADS` | `n_threads` | the results do not depend on it |
| `LARMORX_UPSTREAM_DEFECTS` | `upstream_defects` | `fix` (default) or `reproduce` |

### Examples of `flirt`

<!-- inputs: epi.nii.gz=head T1w.nii.gz=head -->

```python example
import larmorx as lx

# A default 12-dof registration.
r = lx.fsl.flirt("epi.nii.gz", "T1w.nii.gz")
r.matrix.shape  # -> (4, 4)

# The same call, the clean room chosen, without the resampled image.
c = lx.fsl.flirt("epi.nii.gz", "T1w.nii.gz", out=False, implementation="clean-room")
c.image is None  # -> True
```

```bash example
larmorx fsl flirt -in epi.nii.gz -ref T1w.nii.gz -omat epi2t1.mat -out epi2t1.nii.gz
```

### Ports of `flirt`

The replica's record, against FSL 6.0.7.23 itself in its standard comparison, holds fMRIPrep's
boundary-based calls in three of fMRIPrep's runs (ds005040, ds003592, ds000228), bit-identical, their
matrices also identical as text to fMRIPrep's own (its container's FSL); every cost function,
search option, schedule FSL ships, `-applyxfm` 3D and 4D with every interpolation and data type,
the BBR variants and the command line's errors. The clean room is within the thresholds in 239 of
its 240 compared cases, out of 339 (its record of 2026-11-07), which holds FLIRT's fourteen
programs. The one it misses is a weighted `woods` registration (`real/weights-woods`): its matrix
passes, but its standard error holds FSL's warning `Woods is not implemented with cost function
weighting` where FSL's has ended. fMRIPrep's three boundary-based calls are within 0.0113 mm of
FSL's matrices at the 95th percentile of the brain.

## `convert_xfm`

<!-- reference
program: convert_xfm
group: fsl
original: FSL 6.0.7.23
python: larmorx.fsl.convert_xfm
command-line: crates/larmorx-fsl/src/cli/convert_xfm.rs
records: {replica: [fsl-flirt-replica], clean-room: [fsl-flirt]}
evidence: fsl-flirt
-->

`convert_xfm` is FLIRT's matrix tool: it inverts a 4 × 4 matrix (`-inverse`), composes two
(`-concat`) and gives a matrix another's scales and skews (`-fixscaleskew`). It applies
`-fixscaleskew` first, then `-concat` (that matrix times the input), then `-inverse`, and writes
the result (`-omat`).

### Python (`convert_xfm`)

`lx.fsl.convert_xfm(matrix, *, concat=None, fix_scale_skew=None, inverse=False, n_threads=1, implementation="auto")`

### Options of `convert_xfm`

| `convert_xfm` option | Python | Notes |
|---|---|---|
| `<input-matrix-filename>` | `matrix` | a 4 × 4 array or a matrix file |
| `-omat` | the result | |
| `-concat` | `concat` | |
| `-fixscaleskew` | `fix_scale_skew` | |
| `-inverse` | `inverse=True` | |
| `-v`, `-verbose`, `-help` | command line only | |

### Examples of `convert_xfm`

<!-- inputs: epi2t1.mat=matrix -->

```python example
import larmorx as lx

# The inverse of a FLIRT matrix.
inv = lx.fsl.convert_xfm("epi2t1.mat", inverse=True)
inv.shape  # -> (4, 4)
```

```bash example
larmorx fsl convert_xfm -omat t12epi.mat -inverse epi2t1.mat
```

### Ports of `convert_xfm`

Both ports are complete. The replica is translated from FSL 6.0.7.23's source, giving FSL's results
byte for byte. In the
clean room, `convert_xfm`'s messages are FSL's, and `-fixscaleskew` aborts where its decomposition
does not solve (a zero scale or a non-finite entry), as `avscale` does.

## `avscale`

<!-- reference
program: avscale
group: fsl
original: FSL 6.0.7.23
python: larmorx.fsl.avscale
command-line: crates/larmorx-fsl/src/cli/avscale.rs
records: {replica: [fsl-flirt-replica], clean-room: [fsl-flirt]}
evidence: fsl-flirt
-->

`avscale` takes a FLIRT matrix apart: its rotation and translation, angles, scales, skews,
determinant and half transforms. The angles and translations are about the volume's centre of
gravity, the half transforms are the matrix's square root, and the scales' average and the
left-right order come with them; with `--inverteddies` it also prints the eddy-current
inversion's matrices.

### Python (`avscale`)

`lx.fsl.avscale(matrix, volume=None, *, all_params=False, inverted_eddies=False, n_threads=1, implementation="auto")`

### Options of `avscale`

| `avscale` option | Python | Notes |
|---|---|---|
| `matrixfile` `[non-reference-volume]` | `matrix`, `volume` | |
| `--allparams` | `all_params=True` | also the rotation angles and translations |
| `--inverteddies` | `inverted_eddies=True` | the eddy-current inversion; exclusive with `--allparams` |

### Examples of `avscale`

<!-- inputs: bold2t1.mat=matrix boldref.nii.gz=head -->

```python example
import larmorx as lx

d = lx.fsl.avscale("bold2t1.mat", "boldref.nii.gz", all_params=True)
d.scales, d.rotations, d.determinant, d.forward_half
```

```bash example
larmorx fsl avscale --allparams bold2t1.mat boldref.nii.gz
```

### Ports of `avscale`

Both ports are complete (`specs/flirt.md` §15.1); where FSL aborts, they print the same lines and
exit with status 1. A matrix of 4 rows that is not 4 × 4, where FSL aborts at a determinant, an
inverse or a product, is refused (FSL prints part of its output first). One more matrix, from
random testing (a reflection's `--inverteddies` check), differs in the sign of a 1e-10 residual.

The replica's record holds 9 cases of `avscale`: 5 identical to FSL, 4 where both refuse or abort
alike. The clean room's holds 9: 5 identical to FSL in every byte of text and file, 4 where both
refuse or abort alike.

## `img2imgcoord`

<!-- reference
program: img2imgcoord
group: fsl
original: FSL 6.0.7.23
python: larmorx.fsl.img2imgcoord
command-line: crates/larmorx-fsl/src/cli/coords.rs
records: {replica: [fsl-flirt-replica], clean-room: [fsl-flirt]}
evidence: fsl-flirt
-->

`img2imgcoord` maps coordinates from one image to another through a FLIRT matrix: voxels by
default, millimetres with `-mm`.

### Python (`img2imgcoord`)

`lx.fsl.img2imgcoord(coords, src, dest, *, xfm=None, mm=False, n_threads=1, implementation="auto")`

### Options of `img2imgcoord`

| `img2imgcoord` option | Python | Notes |
|---|---|---|
| `<filename containing coordinates>` | `coords` | `-` or no file: standard input (refused at a terminal, as FSL) |
| `-src`, `-dest` | `src`, `dest` | |
| `-xfm`, `-premat` | `xfm` | |
| `-vox`, `-mm` | `mm` | voxels by default |
| `-warp` | command line only | FNIRT's warp fields: refused until FNIRT is ported |
| `-v`, `-help`, `-flirt` | command line only | `-flirt` accepted and ignored |

### Examples of `img2imgcoord`

<!-- inputs: boldref.nii.gz=head T1w.nii.gz=head bold2t1.mat=matrix coords.txt=coords -->

```python example
import larmorx as lx

xyz = lx.fsl.img2imgcoord([[10, 20, 30]], "boldref.nii.gz", "T1w.nii.gz", xfm="bold2t1.mat")
xyz.shape  # -> (1, 3)
```

```bash example
larmorx fsl img2imgcoord -src boldref.nii.gz -dest T1w.nii.gz -xfm bold2t1.mat coords.txt
```

### Ports of `img2imgcoord`

Both ports are complete (`specs/flirt.md` §15.2), standard input included (the prompts and the
refusal at a terminal); where FSL aborts, they print the same lines and exit with status 1. The
coordinate programs' `-warp` (a FNIRT warp field) is refused until FNIRT is ported: an unreadable
file, or an image of other than three volumes, gives FSL's messages and exit status 1, as FSL.
Matrices other than 4 × 4 where FSL would invert them beyond 4 × 4 are refused (FSL prints part
of its output, then aborts).

The replica's record holds 11 cases of `img2imgcoord`: 7 identical to FSL, 4 where both refuse or
abort alike. The clean room's holds 12: 7 identical to FSL in every byte of text and file, 5
where both refuse or abort alike.

## `std2imgcoord`

<!-- reference
program: std2imgcoord
group: fsl
original: FSL 6.0.7.23
python: larmorx.fsl.std2imgcoord
command-line: crates/larmorx-fsl/src/cli/coords.rs
records: {replica: [fsl-flirt-replica], clean-room: [fsl-flirt]}
evidence: fsl-flirt
-->

`std2imgcoord` maps coordinates from a standard image to an image through the inverse of a FLIRT
matrix: millimetres by default, the image's voxels with `-vox`.

### Python (`std2imgcoord`)

`lx.fsl.std2imgcoord(coords, img, std=None, *, xfm=None, vox=False, n_threads=1, implementation="auto")`

### Options of `std2imgcoord`

| `std2imgcoord` option | Python | Notes |
|---|---|---|
| `<filename containing coordinates>` | `coords` | `-` or no file: standard input (refused at a terminal, as FSL) |
| `-img`, `-std` | `img`, `std` | |
| `-xfm`, `-premat` | `xfm` | the matrix inverted |
| `-mm`, `-vox` | `vox` | millimetres by default |
| `-warp` | command line only | FNIRT's warp fields: refused until FNIRT is ported |
| `-v`, `-verbose`, `-help`, `-flirt` | command line only | `-flirt` gives FSL's warning about outdated options |

### Examples of `std2imgcoord`

<!-- inputs: boldref.nii.gz=head std.nii.gz=head bold2std.mat=matrix coords.txt=coords -->

```python example
import larmorx as lx

ijk = lx.fsl.std2imgcoord([[0, 0, 0]], "boldref.nii.gz", "std.nii.gz", xfm="bold2std.mat", vox=True)
ijk.shape  # -> (1, 3)
```

```bash example
larmorx fsl std2imgcoord -img boldref.nii.gz -std std.nii.gz -xfm bold2std.mat coords.txt
```

### Ports of `std2imgcoord`

Both ports are complete (`specs/flirt.md` §15.2), standard input included (the prompts and the
refusal at a terminal): there the list also ends at a coordinate equal to the one before it,
which is not printed. Where FSL aborts, they print the same lines and exit with status 1; `-warp`
is refused until FNIRT is ported.

The replica's record holds 13 cases of `std2imgcoord`: 8 identical to FSL, 5 where both refuse or
abort alike. The clean room's holds 14: 8 identical to FSL in every byte of text and file, 6
where both refuse or abort alike.

## `img2stdcoord`

<!-- reference
program: img2stdcoord
group: fsl
original: FSL 6.0.7.23
python: larmorx.fsl.img2stdcoord
command-line: crates/larmorx-fsl/src/cli/coords.rs
records: {replica: [fsl-flirt-replica], clean-room: [fsl-flirt]}
evidence: fsl-flirt
-->

`img2stdcoord` maps coordinates from an image to a standard image through a FLIRT matrix: input
voxels by default, output in the standard image's millimetres.

### Python (`img2stdcoord`)

`lx.fsl.img2stdcoord(coords, img, std=None, *, xfm=None, mm=False, n_threads=1, implementation="auto")`

### Options of `img2stdcoord`

| `img2stdcoord` option | Python | Notes |
|---|---|---|
| `<filename containing coordinates>` | `coords` | `-` or no file: standard input (refused at a terminal, as FSL) |
| `-img`, `-std` | `img`, `std` | |
| `-xfm`, `-premat` | `xfm` | |
| `-vox`, `-mm` | `mm` | input voxels by default |
| `-warp` | command line only | FNIRT's warp fields: refused until FNIRT is ported |
| `-v`, `-verbose`, `-help`, `-flirt` | command line only | `-flirt` gives FSL's warning about outdated options |

### Examples of `img2stdcoord`

<!-- inputs: T1w.nii.gz=head std.nii.gz=head t1_to_std.mat=matrix coords.txt=coords -->

```python example
import larmorx as lx

mm = lx.fsl.img2stdcoord([[10, 20, 30]], "T1w.nii.gz", "std.nii.gz", xfm="t1_to_std.mat")
mm.shape  # -> (1, 3)
```

```bash example
larmorx fsl img2stdcoord -img T1w.nii.gz -std std.nii.gz -xfm t1_to_std.mat coords.txt
```

### Ports of `img2stdcoord`

Both ports are complete (`specs/flirt.md` §15.2), standard input included (the prompts and the
refusal at a terminal). Where FSL aborts, they print the same lines and exit with status 1;
`-warp` is refused until FNIRT is ported.

The replica's record holds 12 cases of `img2stdcoord`: 7 identical to FSL, 5 where both refuse or
abort alike. The clean room's holds 12: 7 identical to FSL in every byte of text and file, 5
where both refuse or abort alike.

## `rmsdiff`

<!-- reference
program: rmsdiff
group: fsl
original: FSL 6.0.7.23
python: larmorx.fsl.rmsdiff
command-line: crates/larmorx-fsl/src/cli/rmsdiff.rs
records: {replica: [fsl-flirt-replica], clean-room: [fsl-flirt]}
evidence: fsl-flirt
-->

`rmsdiff` measures how far apart two FLIRT matrices are: the RMS deviation in millimetres over 80
mm about the reference's centre of gravity or, with a mask, the largest and the RMS displacement
over the mask's voxels.

### Python (`rmsdiff`)

`lx.fsl.rmsdiff(matrix1, matrix2, ref, mask=None, *, n_threads=1, implementation="auto")`

### Options of `rmsdiff`

| `rmsdiff` arguments | Python | Notes |
|---|---|---|
| `matrixfile1` `matrixfile2` `refvol` `[mask]` | `matrix1`, `matrix2`, `ref`, `mask` | |

### Examples of `rmsdiff`

<!-- inputs: a.mat=matrix b.mat=matrix boldref.nii.gz=head -->

```python example
import larmorx as lx

d = lx.fsl.rmsdiff("a.mat", "b.mat", "boldref.nii.gz")
```

```bash example
larmorx fsl rmsdiff a.mat b.mat boldref.nii.gz
```

### Ports of `rmsdiff`

Both ports are complete (`specs/flirt.md` §15.3). Where FSL aborts, they print the same lines and
exit with status 1.

The replica's record holds 7 cases of `rmsdiff`: 4 identical to FSL, 3 where both refuse or abort
alike. The clean room's holds 7: 4 identical to FSL in every byte of text and file, 3 where both
refuse or abort alike.

## `makerot`

<!-- reference
program: makerot
group: fsl
original: FSL 6.0.7.23
python: larmorx.fsl.makerot
command-line: crates/larmorx-fsl/src/cli/makerot.rs
records: {replica: [fsl-flirt-replica], clean-room: [fsl-flirt]}
evidence: fsl-flirt
-->

`makerot` writes the 4 × 4 matrix of a rotation by `theta` degrees about an axis through a centre
(or through the centre of an image).

### Python (`makerot`)

`lx.fsl.makerot(theta, axis=None, centre=None, *, cov=None, n_threads=1, upstream_defects="fix", implementation="auto")`

The result is a 4 × 4 array.

### Options of `makerot`

| `makerot` option | Python | Notes |
|---|---|---|
| `-t`, `--theta` | `theta` | degrees |
| `-a`, `--axis` | `axis` | three values; divided by its length (FSL by its square root: **FSL-76**, corrected by default) |
| `-c`, `--centre` | `centre` | three values |
| `--cov` | `cov` | an image: the rotation's centre is its centre |
| `-o`, `--out` | the result | |
| `-v`, `--verbose`, `-h`, `--help` | command line only | |

An axis or centre of other than three values is refused (**FSL-77**).

### Examples of `makerot`

```python example
import larmorx as lx

r = lx.fsl.makerot(30, axis=(1, 2, 3), centre=(4, 5, 6))
r.shape  # -> (4, 4)
```

```bash example
larmorx fsl makerot -t 30 -a 1,2,3 -c 4,5,6 -o rot.mat
```

The axis (1, 2, 3) is not of length 1, so LARMORX corrects FSL-76 and says so; `reproduce` gives
FSL's matrix.

### Ports of `makerot`

Both ports are complete (`specs/flirt.md` §15.4): FSL-76 corrected by default (FSL's matrix with
`reproduce`, named), FSL-77 refused by default (FSL's reading with `reproduce`, named in both
modes). Where FSL aborts, they print the same lines and exit with status 1.

The replica's record holds 14 cases of `makerot`: 8 identical to FSL, 6 where both refuse or abort
alike. The clean room's holds 14: 8 identical to FSL in every byte of text and file, 6 where both
refuse or abort alike.

## `midtrans`

<!-- reference
program: midtrans
group: fsl
original: FSL 6.0.7.23
python: larmorx.fsl.midtrans
command-line: crates/larmorx-fsl/src/cli/midtrans.rs
records: {replica: [fsl-flirt-replica], clean-room: [fsl-flirt]}
evidence: fsl-flirt
-->

`midtrans` finds the transform from a template space to the "middle" of several transforms into
that space. The middle is FSL's Procrustes average of the transforms, about the MNI origin or about
the centre of gravity of a template image (`--template`). With `--separate`, each transform
composed with it is written too.

### Python (`midtrans`)

`lx.fsl.midtrans(transforms, *, template=None, separate=None, n_threads=1, implementation="auto", upstream_defects="fix")`

`template` is a path or an `lx.Image`.

### Options of `midtrans`

| `midtrans` option | Python | Notes |
|---|---|---|
| `transform1` `transform2` `...` | `transforms` | at least two |
| `-o`, `--out` | the result | |
| `--template` | `template` | an image: the average about its centre of gravity (about the MNI origin without one) |
| `--separate` | `separate` | a basename: each transform composed with the middle, written to `<basename>0001.mat` and on |
| `--debug`, `-v`, `--verbose`, `-h`, `--help` | command line only | `-v` and `--debug` print every stage |

### Examples of `midtrans`

<!-- inputs: A2temp.mat=matrix B2temp.mat=matrix C2temp.mat=matrix -->

```python example
import larmorx as lx

mid = lx.fsl.midtrans(["A2temp.mat", "B2temp.mat", "C2temp.mat"])
mid.shape  # -> (4, 4)
```

```bash example
larmorx fsl midtrans -o temp2mid.mat A2temp.mat B2temp.mat C2temp.mat
```

### Ports of `midtrans`

Both ports are complete (`specs/flirt.md` §15.5), with every stage that `-v` and `--debug` print. An error after the
arguments exits with status 1 (FSL exits 0: **FSL-78**, corrected by default and named). The clean
room uses a transform whose determinant is not finite (FSL's test is `|det| < 1e-6`), and refuses
transforms whose averages reach beyond double precision, such as a scale of 1e200 or a translation
of 1e300, with exit status 1 in both modes (**FSL-241**: FSL writes rows of NaN, or reports its
failed decomposition as a matrix-size error, and exits 0).

The replica's record holds 12 cases of `midtrans`: 8 identical to FSL, 4 where both refuse alike.
The clean room's holds 14: 8 within its thresholds, 4 refused alike, and the 2 of FSL-241.

## `pointflirt`

<!-- reference
program: pointflirt
group: fsl
original: FSL 6.0.7.23
python: larmorx.fsl.pointflirt
command-line: crates/larmorx-fsl/src/cli/pointflirt.rs
records: {replica: [fsl-flirt-replica], clean-room: [fsl-flirt]}
evidence: fsl-flirt
-->

`pointflirt` finds the affine matrix that maps one set of points onto another, in the
least-squares sense. The points are in millimetres, or in voxels with `--vox`, scaled by the two
images' voxel sizes. It warns when the points do not determine the matrix, and a file of exactly
three points is read with one point per column, as FSL reads it.

### Python (`pointflirt`)

`lx.fsl.pointflirt(in_points, ref_points, *, in_image=None, ref_image=None, n_threads=1, implementation="auto")`

The two sets hold as many points; `in_image` and `ref_image` are paths or `lx.Image` objects,
given both or neither.

### Options of `pointflirt`

| `pointflirt` option | Python | Notes |
|---|---|---|
| `-i`, `-r` | `in_points`, `ref_points` | the input's and the reference's points |
| `-o`, `--out` | the result | |
| `--vox` | `in_image`, `ref_image` given | voxel coordinates; needs `--invol` and `--refvol` |
| `--invol`, `--refvol` | `in_image`, `ref_image` | the images whose voxel sizes scale the points |
| `-v`, `--verbose`, `-h`, `--help` | command line only | |

### Examples of `pointflirt`

<!-- inputs: in.txt=points ref.txt=points-moved -->

```python example
import larmorx as lx

# The second file holds the first's points moved by 2, -3 and 4 mm.
aff = lx.fsl.pointflirt("in.txt", "ref.txt")
aff[:3, 3].round(6).tolist()  # -> [2.0, -3.0, 4.0]
```

```bash example
larmorx fsl pointflirt -i in.txt -r ref.txt -o in2ref.mat
```

### Ports of `pointflirt`

Both ports are complete (`specs/flirt.md` §15.6), with its transposition rule, `--vox` and the
nullspace warning. Two
coordinate files both empty or missing are refused in both modes (**FSL-204**: FSL crashes after
its `Could not open matrix file` lines): `larmorx-nc: error: pointflirt: no points in the
coordinate files (FSL-204)`, exit status 1, the clean room after `-v`'s headings, as FSL prints
them. The clean room reads a `nan` word as 0 (the fit of the points as read) and stops on an
overflow as FSL's SVD does.

The replica's record holds 14 cases of `pointflirt`: 7 identical to FSL, 6 where both refuse
alike, and 1 of FSL-204. The clean room's holds 17: 8 within its thresholds, 7 refused alike, and
the 2 of FSL-204.

## `applyxfm4D`

<!-- reference
program: applyxfm4D
group: fsl
original: FSL 6.0.7.23
python: larmorx.fsl.applyxfm4d
command-line: crates/larmorx-fsl/src/cli/applyxfm4d.rs
records: {replica: [fsl-flirt-replica], clean-room: [fsl-flirt]}
evidence: fsl-flirt
-->

`applyxfm4D` resamples each volume of a series onto a reference's grid, through one matrix or one
per volume. The matrices are one file (`-singlematrix`) or a directory of files named `MAT_0000n`,
`MAT_nnnn` (`-fourdigit`) or `<prefix>nnnn` (`-userprefix`). The interpolation is sinc (the
default), trilinear, nearest neighbour or spline, and the output keeps the input's data type
(char, short, int, float, double).

### Python (`applyxfm4D`)

`lx.fsl.applyxfm4d(in_image, ref_image, out_image, transforms, *, single_matrix=False, four_digit=False, user_prefix=None, interp="sinc", three_d=False, n_threads=1, implementation="auto", upstream_defects="fix")`

The images and `transforms` are paths.

### Options of `applyxfm4D`

| `applyxfm4D` option | Python | Notes |
|---|---|---|
| `<input volume>` `<ref volume>` `<output volume>` | `in_image`, `ref_image`, `out_image` | |
| `<transformation matrix file/dir>` | `transforms` | a directory of `MAT_0000n` files, or one file with `-singlematrix` |
| `-singlematrix`, `--singlematrix` | `single_matrix` | one matrix for every volume |
| `-fourdigit`, `--fourdigit` | `four_digit` | the files named `MAT_nnnn` |
| `-userprefix`, `--userprefix` | `user_prefix` | the files named `<prefix>nnnn` |
| `-interp`, `--interp` | `interp` | `sinc` (default), `trilinear`, `nearestneighbour` (or `nn`), `spline` |
| `-3D`, `--3D`, `-3d`, `--3d` | `three_d` | one volume; with sinc, the rectangular kernel the program asks for (**FSL-80**) |
| `-v`, `--v`, `-verbose`, `--verbose` | command line only | |

### Examples of `applyxfm4D`

<!-- inputs: bold.nii.gz=head4d boldref.nii.gz=head bold2ref.mat=matrix -->

```python example
import larmorx as lx

path = lx.fsl.applyxfm4d(
    "bold.nii.gz", "boldref.nii.gz", "bold_on_ref.nii.gz", "bold2ref.mat", single_matrix=True
)
```

```bash example
larmorx fsl applyxfm4D bold.nii.gz boldref.nii.gz bold_on_ref.nii.gz bold2ref.mat -singlematrix
```

### Ports of `applyxfm4D`

Both ports are complete (`specs/flirt.md` §15.7), on FLIRT's shared resampling (no blur, as FSL). FSL's defects are corrected
by default, and FSL's behaviour comes with `upstream_defects="reproduce"`:

- **FSL-09**: integer outputs are rounded to the nearest and clamped (FSL truncates).
- **FSL-79**: spline interpolation of an integer image is computed as for a float image (FSL keeps
  the spline's coefficients and values in the image's type); named where the output changes.
- **FSL-80**: `-3D` with sinc uses the rectangular kernel of width 9 that the program asks for
  (FSL uses a blackman kernel of width 7); named where the output changes.
- **FSL-111**: a uint8 image's values are read as stored (FSL reads them as signed char, its
  values 128 to 255 as `v − 256`); named where a written value changes.
- **FSL-75**: the background value of a volume 3 voxels wide is taken from the whole border.
- **FSL-11**: with `FSLOUTPUTTYPE` unset, the images are written as `.nii.gz` and FSL-11 is named
  (FSL stops at its first image); command line only, as the Python function sets the type itself.

With a reference that has neither form, the output carries the input's form through the first
matrix, as a registration's `-out` does.

The replica's record holds 25 cases of `applyxfm4D`: 19 identical to FSL, 6 where both refuse
alike. The clean room's holds 25: 20 within its thresholds, 5 refused alike.

## `pairreg`

<!-- reference
program: pairreg
group: fsl
original: FSL 6.0.7.23
python: larmorx.fsl.pairreg
command-line: crates/larmorx-fsl/src/cli/pairreg.rs
records: {replica: [fsl-flirt-replica], clean-room: [fsl-flirt]}
evidence: fsl-flirt
-->

`pairreg` is a script that registers one brain to another and refines the result with the two
heads' skulls. It runs FLIRT three times, with FSL's schedules `pairreg1.sch` to `pairreg3.sch`:
the brains first, then the skulls, then the brains again; `brain1` and `skull1` are the references.

### Python (`pairreg`)

`lx.fsl.pairreg(brain1, brain2, skull1, skull2, *, flirt_args=None, n_threads=1, implementation="auto", upstream_defects="fix")`

The images are paths; `flirt_args` is a list of FLIRT's options. The result is the 4 × 4 matrix
from `brain2` to `brain1`.

### Options of `pairreg`

| `pairreg` arguments | Python | Notes |
|---|---|---|
| `brain1` `brain2` `skull1` `skull2` | `brain1`, `brain2`, `skull1`, `skull2` | `brain1` and `skull1` the references |
| `outputmatrix` | the result | |
| `[extra flirt args]` | `flirt_args` | passed to every FLIRT run |

### Examples of `pairreg`

<!-- inputs: b1_brain.nii.gz=head b2_brain.nii.gz=head b1.nii.gz=head b2.nii.gz=head -->

```python example
import larmorx as lx

pair = lx.fsl.pairreg("b1_brain.nii.gz", "b2_brain.nii.gz", "b1.nii.gz", "b2.nii.gz")
pair.shape  # -> (4, 4)
```

```bash example
larmorx fsl pairreg b1_brain.nii.gz b2_brain.nii.gz b1.nii.gz b2.nii.gz b2_to_b1.mat
```

### Ports of `pairreg`

Both ports are complete (`specs/flirt.md` §15.8). The clean room runs its own `flirt` three times and one `convert_xfm -fixscaleskew`, in
this process, and carries FSL's three schedules itself: it needs no `$FSLDIR`. `-v` echoes FSL's
lines, and a step that aborts is followed by the shell's `Aborted` line. Arguments that the
script's shell would split or expand are refused.

The replica's record holds 3 cases of `pairreg`: 2 identical to FSL, 1 where both refuse alike.
The clean room's holds 5: 4 within its thresholds, 1 refused alike.

## `extracttxt`

<!-- reference
program: extracttxt
group: fsl
original: FSL 6.0.7.23
python: larmorx.fsl.extracttxt
command-line: crates/larmorx-fsl/src/cli/extracttxt.rs
records: {replica: [fsl-flirt-replica], clean-room: [fsl-flirt]}
evidence: fsl-flirt
-->

`extracttxt` is a script that prints the lines of a text file from the first line that matches a
word. The word is a basic regular expression, as grep reads it. With a count, it prints that many
lines, from a given number of lines after the match (before it, when the number is negative), and
FSL's messages for ranges before the first line.

### Python (`extracttxt`)

`lx.fsl.extracttxt(search_word, file, trailing_lines=None, relative_start=None, *, n_threads=1, implementation="auto")`

The result is the text printed, as a string. `relative_start` needs `trailing_lines`.

### Options of `extracttxt`

| `extracttxt` arguments | Python | Notes |
|---|---|---|
| `search_word` | `search_word` | a basic regular expression |
| `file` | `file` | |
| `num_trailing_lines` | `trailing_lines` | without it, every line to the end |
| `relative_start` | `relative_start` | lines after the match, or before it when negative |

### Examples of `extracttxt`

<!-- inputs: log.txt=log -->

```python example
import larmorx as lx

# Three lines, from the line before the first that holds "value".
text = lx.fsl.extracttxt("value", "log.txt", 3, -1)
text  # -> 'start\nfirst value 1\nmiddle\n'
```

```bash example
larmorx fsl extracttxt value log.txt 3 -1
```

### Ports of `extracttxt`

Both ports are complete (`specs/flirt.md` §15.8). The clean room has its own POSIX basic regular expressions (with GNU grep's extensions) and
the script's dc and sed arithmetic, messages included. GNU sed's usage text, which a range read as
sed's options prints, is not reproduced: one LARMORX line takes its place. Refused, as the spec
says: arguments that the script's shell would split or expand, counts and starts that are not
integers, and files with NUL bytes.

The replica's record holds 13 cases of `extracttxt`: 8 identical to FSL, 5 where both refuse
alike. The clean room's holds 12: 8 within its thresholds, 4 refused alike.

## `aff2rigid`

<!-- reference
program: aff2rigid
group: fsl
original: FSL 6.0.7.23
python: larmorx.fsl.aff2rigid
command-line: crates/larmorx-fsl/src/cli/aff2rigid.rs
records: {replica: [fsl-flirt-replica], clean-room: [fsl-flirt]}
evidence: fsl-flirt
-->

`aff2rigid` is a script that turns FLIRT's 12-parameter matrix from an image to standard space
into a 6-parameter (rigid) one. The rigid matrix keeps, in order of decreasing accuracy, the
anterior commissure, the AC-PC line and the mid-sagittal plane.

### Python (`aff2rigid`)

`lx.fsl.aff2rigid(matrix, *, n_threads=1, implementation="auto", upstream_defects="fix")`

### Options of `aff2rigid`

| `aff2rigid` arguments | Python | Notes |
|---|---|---|
| `<input2standard mat>` | `matrix` | FLIRT's matrix from the image to standard space |
| `<output mat>` | the result | |

### Examples of `aff2rigid`

<!-- inputs: T1w2MNI.mat=matrix -->

```python example
import larmorx as lx

rigid = lx.fsl.aff2rigid("T1w2MNI.mat")
rigid.shape  # -> (4, 4)
```

```bash example
larmorx fsl aff2rigid T1w2MNI.mat T1w2MNI_rigid.mat
```

### Ports of `aff2rigid`

FSL 6.0.7.23's script stops with a Python `NameError` under NumPy 2, before it writes the matrix
(**FSL-81**). Both ports compute the matrix as the script defines it by default, and stop with
FSL's traceback with `upstream_defects="reproduce"`; the clean room's matrix is identical to the
replica's. Each record holds 3 cases of `aff2rigid`: its usage, and two matrices (fMRIPrep's
initial one among them) on which the ports, with `reproduce`, stop as FSL does.

## The clean room (part 1, 2026-11-02)

`lx.fsl.flirt(..., implementation="clean-room")`, `lx.fsl.convert_xfm(..., implementation="clean-room")`,
`larmorx --implementation clean-room fsl flirt|convert_xfm` (crate `larmorx-fsl`, module
`flirt`; Apache-2.0, written from [`specs/flirt.md`](../../specs/flirt.md) by an implementer who
never read FSL's source or the replica). It runs on the main package's clean-room registration
engine, shared with `lx.mri.hmc` (`larmorx_mri::hmc`: the parametrisation, the cost sampling and
sums, the line search, the interpolators), not a copy of it.

**What it has.** FLIRT's whole command line with its messages and exit statuses; the base scale,
the clamping, single slices; the reference pyramid; every cost (`corratio`, `mutualinfo`,
`normmi`, `normcorr`, `leastsq`, `labeldiff`, `woods`, the boundary-based `bbr` from `-wmseg` or
`-wmcoords`/`-wmnorms`, with `-bbrtype`, `-bbrslope` and a field map) and their weighted forms;
the schedule language with every command (`alignpaxes` included), the 3D and 2D default schedules (`-v`
prints FSL's lines) and every schedule FSL ships, `bbr.sch` included; the search; the
optimisation (the bound guesses, the convergence test, Powell's correction, parameter subsets of
any mask); `-anglerep quaternion`; the sform/qform initialisation; `-omat`, `-out`, `-applyxfm`, `-applyisoxfm` with
every interpolation, sinc width and window, `-paddingsize`, `-setbackground` and data type; the
background value with FSL-75 and FSL-22; FSL-68's exit status; the refusals of FSL-21/66, FSL-71,
FSL-72, an undefined matrix name, `-pedir` without a field map, a single-voxel background.
`convert_xfm` complete. Deterministic at any thread count (`n_threads`, `OMP_NUM_THREADS`): the
search's grid points, the large evaluations' slices, the pyramid's filters, the boundary points
and the output's slices run in parallel, each result folded in a fixed order.

**FLIRT's other programs (part 2, relay 1, 2026-11-02).** `avscale`, `img2imgcoord`,
`std2imgcoord`, `img2stdcoord`, `rmsdiff` and `makerot` (`larmorx --implementation clean-room
fsl <program>`, `lx.fsl.<program>(..., implementation="clean-room")`; crate `larmorx-fsl`, modules
`flirt::programs` and `cli::{avscale, coords, rmsdiff, makerot, common}`), complete: every option,
usage, message, exit status and failure case of §15.1 to §15.4 (where FSL aborts, the same lines
and exit status 1), standard input for the coordinate programs (their prompts, `std2imgcoord`'s
repeated-triple end, the terminal refusal), FSL-76 (corrected by default, FSL's matrix with
`reproduce`, named), FSL-77 (refused by default, FSL's reading with `reproduce`, named in both
modes) and FSL-14 for a NaN volume's centre of gravity (left out by default). Refused, as the spec
says: matrices of 4 rows that are not 4 × 4 where FSL aborts at a determinant, an inverse or a
product (FSL prints part of its output first), and a `-warp` of three volumes (FNIRT's warp fields
are not ported; other images get FSL's read error, since the catch-up of 2026-11-05). The fsl-flirt suite's 66 cases of these programs: 39 identical to FSL in every byte of
text and file, 27 more where both refuse or abort alike; one more matrix of the findings
(a reflection's `--inverteddies` check) differs in the sign of a 1e-10 residual
([findings](../findings/fsl-flirt-clean-room.md), "FLIRT's other programs").

**FLIRT's other programs and scripts (part 2, relay 2, 2026-11-02).** `midtrans`, `pointflirt`,
`applyxfm4D` and the scripts `pairreg`, `extracttxt` and `aff2rigid` (modules `flirt::fits` and
`cli::{midtrans, pointflirt, applyxfm4d, pairreg, extracttxt, bre, aff2rigid}`), complete: every
option, usage, message, exit status and failure case of §15.5 to §15.8. `midtrans` with every
stage `-v` and `--debug` print, FSL-78 (exit status 1 by default, named); `pointflirt` with its
transposition rule, `--vox` and the nullspace warning; `applyxfm4D` on FLIRT's shared resampling
(no blur, as FSL: observed), in the input's grouped type, a double input's padding in double
precision, FSL-09, FSL-75, FSL-79 (the spline's coefficients per pass in the image's type with
`reproduce`, converted as x86-64 converts them), FSL-80 (the rectangular width-9 kernel with `-3D`
by default) and FSL-111 (a uint8 image's values read as stored by default, as signed char with
`reproduce`); with a reference that has neither form, the output carries the input's form through
the first matrix (as a registration's `-out` does, observed); `pairreg` as three
runs of the clean room's `flirt` and one `convert_xfm -fixscaleskew` in this process, FSL's three
schedules carried by the clean room (since 2026-11-05: no `$FSLDIR` needed; `-v` echoes FSL's lines);
`extracttxt` with its own POSIX basic regular expressions (GNU grep's extensions) and the script's
dc and sed arithmetic, messages included (GNU sed's usage text, which a range read as sed's options
prints, is not reproduced: one larmorx line instead); `aff2rigid`'s matrix computed by default
(FSL-81), FSL 6.0.7.23's traceback with `reproduce`. Refused, as the spec says (rule B): averages
beyond double precision (`midtrans`, **FSL-241** since 2026-11-05), arguments the scripts'
shell would split or expand, non-integer counts and starts, files with NUL bytes. The fsl-flirt
suite's 63 cases of these programs: 40 identical to FSL (the fits within 1e-9 of FSL's matrices
in every element, `applyxfm4D`'s nearest-neighbour and trilinear images bit for bit, its sinc and
spline images at a correlation of 1.0000000 with FSL's, the last bits of single precision apart),
23 more where both refuse or abort alike; `aff2rigid`'s corrected matrix identical to the
replica's. Also in this relay: FLIRT's matrix files read as FSL reads them (`inf`, `nan` and words
that are not numbers read 0 and stop their line, a value beyond the double range the largest
double; [findings](../findings/fsl-flirt-clean-room.md)).

**The verbose levels and `-debug` (part 2, relay 3).** `-verbose 2` to `7` print FSL 6.0.7.23's
lines in its order: the images as read and prepared, each `setscale`'s lines and evaluation
counts, `setoption`'s echo, the rotation samplings, every optimisation's start, tolerances,
evaluations and Powell's corrections, and the search's grids, thresholds and minima at the run's
level lowered by 2; `-debugsave` writes the search's grids from level 6 (FSL writes none at 4);
`-debug` writes the reference pyramid, its weights and the input at 8 mm (FSL's bit for bit on the
suite's cases), prints each `setscale`'s and the boundary-based cost's debug lines and makes the
final debug call with BBR's volume `debug`
([findings](../findings/fsl-flirt-clean-room.md), "The verbose levels and -debug"). At level 5 and
more the evaluation lines follow the optimisation's path, so their count differs from FSL's where
the matrices differ in their last digits. The suite's 22 cases of these options (13 of the
clean room's own): every line, exit status and file FSL's.

**Status: `experimental`.** Its record: [`docs/validation/fsl-flirt.md`](../validation/fsl-flirt.md)
(the replica suite's cases, the clean room's own and its permanent cases, against FSL 6.0.7.23;
re-made 2026-11-05 from `a835ddd`): 238 of 240 compared cases within thresholds, 89 more rejected
by both alike, 10 where FLIRT crashes or never finishes or writes what the specification cannot
define; the 2 it misses are two weighted registrations whose paths part at their costs' last bits
(since 2026-11-05 the search orders its starting points of equal cost as FSL does, with
libstdc++'s `std::sort`: an empty weight or a one-voxel input, where every cost ties, now gives
FSL's matrix). fMRIPrep's three
boundary-based calls within 0.1 mm of FSL's matrices at the 95th percentile over the brain (0.053,
0.011 and 0.030 mm), their images' correlation with FSL's 0.9993 to 0.99996; `-applyxfm` with
trilinear and nearest neighbour FSL's images bit for bit, sinc and spline within 1e-5 of the
image's range; the command line's errors, `convert_xfm` and the schedules' messages FSL's; the
costs FSL's printed values at every scale. The cases it misses and why are in the record (the
optimisation's paths part from FSL's in the costs' last bits, BBR's normals from `-wmseg`,
ill-posed registrations); what the spec does not say is in
[`docs/findings/fsl-flirt-clean-room.md`](../findings/fsl-flirt-clean-room.md).

**Speed** ([benchmark report](../benchmarks/fsl-flirt.md): one run each on a shared machine,
the clean room on at most 14 threads): fMRIPrep's boundary-based call on the standard tier's three
oracle runs 1.2 to 1.3 times FSL's speed on one thread and 12 to 14 times on 14 (ds005040: FSL
47 s, the replica 57 s, the clean room 39 s and 3.8 s); the default 12-dof registration of a BOLD
reference to its T1w at FSL's speed on one thread (FSL 0.97 s, the clean room 1.10 s) and 3.2 times
on 14 (0.30 s); BBR with a field map and its mask 3.2 and 36 times (FSL 256 s, the clean room 81 s
and 7.0 s); `-applyxfm` 1.9 and 5.6 times. The same bits at any thread count: the correlation
ratio's bins are summed in parallel with each bin's additions in the sampling order, the search's
grid points, the boundary pairs and the field map's searches run in parallel and are collected in
order.

**The spec writer's answers (part 2, relay 4).** The costs, parameters, search, optimiser,
boundary normals, spline, sinc and forms follow `specs/flirt.md`'s exact arithmetic of 2026-11-02:
a development run of the standard tier's 305 cases had 206 within thresholds, 85 rejected by both
alike, 6 where FLIRT crashes or never finishes, and 8 failures (part 1: 22 of 157); every
boundary-based case and fMRIPrep's three calls, `-anglerep quaternion`, `-usesqform`, `convert_xfm
-fixscaleskew` and every spline output FSL's (the spline's and `-usesqform`'s bit for bit); the
sinc outputs within 1e-5 of the image's range (1.6 % of the voxels in their last bit). The
failures and why: [findings](../findings/fsl-flirt-clean-room.md), "The spec writer's answers". Its
defaults for the four defects the spec writer found: **FSL-90** (`-anglerep quaternion`: the
search's starting points composed from their Euler angles; FLIRT's quaternion reading, with its
messages, with `reproduce`), **FSL-91** (a `-refweight` or `-inweight` with no voxel of at least
0.01: the registration refused; FLIRT's run with `reproduce`), **FSL-92** (an input or reference of
one voxel: refused; FLIRT's run with `reproduce`), **FSL-93** (a header voxel size of 0: counted
as 1 for the base scale; refused with `reproduce`, where FLIRT never finishes; `-basescale 0`
refused in both modes), each named in both modes. `lx.mri.hmc`'s `-cost corratio` and `-cost
woods` now use the same sums (`specs/mcflirt.md` appendix A).

**The clean-room catch-up, programs and messages (2026-11-05).** `specs/flirt.md` §14's answers
of 2026-11-03 (items 1 to 8 of `docs/next-steps.md`'s list for the clean room): the search's free
optimisations print at its lowered level (confirmed: `verbose/5`, `verbose/7`); with `-anglerep
quaternion` and `reproduce` a starting point outside the unit ball is composed once and keeps only
its scale (the identity at 6 dof: no translations), FSL-90's message prints at every such
composition, the evaluations' included, and each start's composition, its dof message and its
optimisation's messages follow one another start by start (each free optimisation writes its own
log, passed on in order whatever runs in parallel): FSL's 8 messages at 6 dof, 4 at 12, the 102
dof messages at 3 interleaved as FSL's (`real/anglerep-quaternion-dof3`); avscale's check signs
(the 82 avscale findings of the 2026-11-02 trial now FSL's text); `midtrans` uses a transform whose
determinant is not finite (FSL's test is `|det| < 1e-6`) and refuses its averages (FSL-241);
`pointflirt` refuses no points (FSL-204) after FSL's lines, reads a `nan` word as 0 (the fit of the
points as read) and stops on an overflow as FSL's SVD does; `-warp` takes exactly three volumes;
FSL's 4 × 4 inverse fails only on an exact zero (NaN and infinite matrices are inverted, and with
`reproduce` FLIRT goes on with its NaN matrices and writes them: 37 of the trials' 57 findings
that stopped at `inv(): matrix is singular` now FSL's; the sign of a NaN can differ, below);
`convert_xfm -fixscaleskew` aborts where its decomposition does not solve, as avscale (a zero
scale or a non-finite entry); `pairreg`'s schedules carried, and a step that aborts followed by
the shell's `Aborted` line. Found on the way (FSL's verbose lines): the centres' COG warning
where `-verbose 3` measures them, the levels' fourth size (the image's TR as read, FSL's 0 for
images without one), the sampling and initial matrices in scaled millimetres with a base scale
([findings](../findings/fsl-flirt-clean-room.md), "The clean-room catch-up").

**The clean-room catch-up, numerics and `-debug` (2026-11-05).** Items 9 to 13 of the list:
`leastsq` sums in three single-precision levels (`real/cost-leastsq` passes); applyxfm4D's spline
is FLIRT's own (§9.3: a double image filtered in double; the 5,177 voxels of five trial findings
that differed are FSL's); `-debug`'s levels take their headers from the resampling's steps (only
set forms composed, a neurological reference's forms swapped back with each level's grid,
`-noresample`'s levels the reference's own forms), `global_refweight1`'s qfac and `pixdim[4..7]`
are 1, and with the boundary-based cost and no boundary point the set-up's lines and error come at
every evaluation and FLIRT goes on to write `debug`, `-omat` and `-out` (123 of the trial's 161
`-debug` findings agree, 52 before); a perturbed row is composed with all twelve parameters (its
scales and skews kept whatever the command's dof); the search's `P` keeps each start composed
once; with `reproduce` and NaN centres (FSL-14) an optimisation starts from the identity's
parameters where its own are NaN, and a NaN pre-image is outside (the background value): 52 of
the 57 trial findings with non-finite voxels agree (37 before). Open: FSL-91's and FSL-92's
`reproduce` traces part at the sign of a zero skew under gimbal lock, and the weighted `woods`
registration at its cost's last bits ([findings](../findings/fsl-flirt-clean-room.md), "numerics,
`-debug` and NaN centres"; the weighted `normcorr` FSL's matrix as text since 2026-11-07, its
count's running total taking each point's weight as the point is visited).

## Defects of the original, corrected by default

Corrected by default, FSL's behaviour with `upstream_defects="reproduce"`
([register](../findings/upstream-defects.md)):
- **FSL-09**: integer outputs rounded to the nearest and clamped (FLIRT truncates; int8 kept as
  int8, FLIRT writes uint8); the notice names the program that wrote the output (flirt,
  applyxfm4D).
- **FSL-11**: with `FSLOUTPUTTYPE` unset FSL's flirt and applyxfm4D stop with FSL's message at
  their first image (exit 1; `-omat` alone writes no image, and runs; with `-out` the matrix is
  written first). Both ports, the clean room and the replica, write the images
  as `NIFTI_GZ` (`.nii.gz`: the bytes `FSLOUTPUTTYPE=NIFTI_GZ` gives) and name FSL-11 on one line
  per command, in the same words (the replica's line starts `larmorx-nc:`); `reproduce` stops as
  FSL does. Command line only: the Python functions set the type themselves.
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
the matrix computed by default), **FSL-111** (`applyxfm4D` reads a uint8 image into a signed
char image: its values 128 to 255 as `v − 256`; read as stored by default, named where a written
value changes; in the replica too since 2026-11-03). **FSL-75** also in `applyxfm4D` (a volume 3 voxels wide) and in
mcflirt's replica (named in both modes, the whole border by default).
The spec answers (2026-11-02, the replica): **FSL-90** (`-anglerep quaternion`: the search's
starting points, Euler angles, composed as Euler angles by default; FLIRT reads them as a
quaternion's vector part), **FSL-91** (a `-refweight` or `-inweight` with no voxel of at least 0.01:
refused by default, exit status 1; FLIRT registers on constant costs), **FSL-92** (an input or
reference of one voxel: refused by default), **FSL-93** (a header voxel size of 0: counted as 1 for
the base scale by default, as reading the image counts it; FLIRT's base scale is then 0 and it never
finishes, so `reproduce` refuses; `-basescale 0` refused in both modes). Each named in both modes
(`upstream defect FSL-9n (flirt) corrected: ...`); the clean room follows in its part 2.

Refused in both modes (FLIRT's behaviour is undefined): **FSL-204** (`pointflirt` with two
coordinate files both empty or missing: FSL crashes after its `Could not open matrix file` lines;
`larmorx-nc: error: pointflirt: no points in the coordinate files (FSL-204)`, exit 1; the replica
since 2026-11-03, the clean room since 2026-11-05 after `-v`'s headings, as FSL prints
them), **FSL-241** (`midtrans` with transforms whose averages reach beyond double precision, a scale
of 1e200 or a translation of 1e300: FSL writes rows of NaN, or reports its failed decomposition as a
matrix-size error, and exits 0; the clean room refuses, exit 1, since 2026-11-05: rule B,
FSL's values not specified), FSL-21/FSL-66 (`-bins` giving no or negative
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

### glibc's maths functions (GLIBC-07)

FSL's FLIRT calls glibc 2.35's `sinf`, `cosf` and `atan2f` (the rotations' compositions and
decompositions), `exp` (the pyramid's and BBR's Gaussians), `cos` (the sinc window) and `logf` (the
mutual-information costs), which are not correctly rounded (`specs/flirt.md` §3, §5, §9.2, §12). The
clean room computes them correctly rounded by default and takes glibc's with
`upstream_defects="reproduce"` (`larmorx_core::math`'s `*_glibc` functions, proved bit for bit
against glibc 2.35; the user's decision of 2026-11-04: glibc's bits for `reproduce` only). A
registration is an iterative fit: one function value's last bit can move its matrix's last digits,
or where costs tie another grid point wins, and FLIRT cannot tell without registering twice, so
neither mode names GLIBC-07 there (nor applyxfm4D's resampling): the default's correctly rounded
functions can move a registration's last bits. avscale, makerot and `convert_xfm -fixscaleskew`
compute both choices and name GLIBC-07, in both modes, exactly where the other choice prints or
writes another result.

The replica takes glibc's `sinf`, `cosf`, `atan2f`, `exp` and `logf` in both modes (permissive
code, as FSL's build runs them), and since 2026-11-04 glibc's double `cos` with `reproduce` where
FSL calls it (`larmorx_core::math::cos_glibc`): the sinc kernels' Hanning and Blackman windows
(FLIRT's `-interp sinc`, applyxfm4D, mcflirt's `-sinc_final`; correctly rounded by default, named
nothing: the windows are stored as float and the two choices give the same kernels at every width
from 1 to 1001) and avscale's half transforms (`sqrtaff`; named, in both modes, where the two
choices print different reports, which the half transforms' float rotation makes rare). The
stage-4 cost's sinc table is the same with either `cos` (all 201 entries), so it serves both modes.

## Not yet

The coordinate programs' `-warp` (FNIRT's warp fields, refused until FSL's warp-field libraries are
ported); `flirt_average` and `epi_reg` (scripts of FSL's other programs: not ported).
`standard_space_roi`, which needs FSL's other programs too, is ported with BET: the replica's since
2026-11-04, the clean room's since 2026-11-05 ([fsl-bet.md](fsl-bet.md)). Known differences: with NaN or infinite input voxels in `reproduce` mode (FSL-14), the sign of
NaN results can differ from FSL's (`nan` against `-nan`; the values are NaN in both); FSL-91's and
FSL-92's `reproduce` runs (every cost equal) and the weighted `woods` registration of the
record part from FSL's paths (the catch-up's findings, above).

## History

- **The replica**, parts 1 to 3 (2026-11-01 and 02; its
  [PROVENANCE](../../crates-nc/larmorx-nc-flirt/PROVENANCE.md)): `flirt` and `convert_xfm` in part 1;
  `avscale`, `img2imgcoord`, `std2imgcoord`, `img2stdcoord`, `rmsdiff` and `makerot` in part 2
  (2026-11-01); `midtrans`, `pointflirt`, `applyxfm4D` and the scripts `pairreg`, `extracttxt` and
  `aff2rigid` in part 3.
- **The clean room**: `flirt` and `convert_xfm` in part 1 (2026-11-02); FLIRT's other programs and
  scripts, the verbose levels and `-debug`, and the spec writer's answers in part 2 (2026-11-02);
  the clean-room catch-up (2026-11-05), after which its record was made again from `a835ddd`; the
  weighted `normcorr` FSL's matrix as text since 2026-11-07.
