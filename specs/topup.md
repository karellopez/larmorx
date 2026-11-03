# Spec: `lx.fsl.topup` and `lx.fsl.applytopup` (behaviour of FSL 6.0.7.23's `topup` and `applytopup`: fsl-topup 2203.5)

The input for a clean-room **original** of larmorx's susceptibility-field estimation and
correction (to be written in the main package, Apache-2.0). FSL is distributed under the FSL
Licence (non-commercial use only), so larmorx has, or will have, two implementations (CLAUDE.md
rule 1, `docs/licensing.md`):
- a bit-exact replica of FSL's code, in the separate `larmorx-nc` package (done:
  `larmorx-nc fsl topup`, `larmorx-nc fsl applytopup`, `lx.fsl.topup`, `lx.fsl.applytopup`,
  `docs/validation/fsl-topup-replica.md`);
- the original, in the main package, written from this file, FSL's documentation, the papers and
  black-box runs of FSL's programs only (`~/fsl-6.0.7.23/bin/topup` and `applytopup`, run in a
  clean environment: FSL never on the `PATH`, `FSLDIR=~/fsl-6.0.7.23`, `FSLOUTPUTTYPE` set,
  `FSL_NUM_THREADS` unset, `OPENBLAS_CORETYPE=Haswell`).

**Whoever writes the original must not open** FSL's source (`~/fsl-6.0.7.23/src/`,
`crates-nc/fsl-source/`), the replica crates (`crates-nc/`), the findings written from the
source (`docs/findings/fsl-topup.md`, the sections of `docs/findings/upstream-defects.md` marked
restricted) or `docs/parts/` (porting guide §1).

This spec describes behaviour: what goes in, what comes out, the maths and the conventions that
decide the results. It holds no code. Statements marked **(observed)** were confirmed by running
FSL's programs (the replica's parity suite runs each of them against FSL, bit for bit); the rest
come from FSL's documentation, the papers and the analysis of the programs' behaviour. The
methods: J. L. R. Andersson, S. Skare, J. Ashburner, "How to correct susceptibility distortions
in spin-echo echo-planar images: application to diffusion tensor imaging", NeuroImage
20(2):870-888, 2003 (the model, the least-squares restoration); S. M. Smith et al., "Advances in
functional and structural MR image analysis and implementation as FSL", NeuroImage 23:S208-S219,
2004. FSL's user guide (the `topup` and `applytopup` pages, "TopupUsersGuide") describes the
options, the acquisition-parameter file and the configuration files.

**The original is built for speed** (user, 2026-10-19): the behaviour below within the
thresholds of §12, designed to be much faster than FSL (§11 lists where the time goes and what
can run in parallel deterministically). Bit-identity is not required of it; where a convention
below "decides the bits", following it keeps the original closest to FSL.

**Defects (D13).** Some behaviours below are defects that larmorx corrects by default and
reproduces with `upstream_defects="reproduce"` (`LARMORX_UPSTREAM_DEFECTS=reproduce`): FSL-116 to
FSL-135 and FSL-141 to FSL-143 (§10). Questionable behaviours kept as FSL's (rule C) are listed
in §10.2.

---

## 1. Purpose and use

`topup` estimates the off-resonance field (Hz) that displaces echo-planar images along their
phase encoding, from two or more images of the same object acquired with different phase
encodings (typically the same direction with opposite polarity: "blip up" and "blip down"). The
field is a 3D B-spline; optionally each image's rigid-body movement relative to the first is
estimated with it. The images, each displaced by the field along its own phase encoding (and
moved), with their intensities modulated by the displacement's Jacobian, should agree; topup
minimises their disagreement plus a smoothness penalty on the field, from coarse to fine.

`applytopup` corrects images with topup's result: each image by itself (Jacobian modulation,
`--method=jac`), or pairs acquired with opposite phase encodings together by a least-squares
restoration that recovers the signal compressed by the distortion (`--method=lsr`, the default;
`vb2D`, `vb3D`, `vb4D` are undocumented variants whose regularisation weight is inferred).

fMRIPrep (through sdcflows' PEPOLAR workflow) calls `topup --config=b02b0.cnf
--datain=<encfile> --imain=<merged b0s> --out=<base> --iout=<corrected> --fout=<field>
--jacout=jac --logout=<log> --rbmout=xfm --dfout=warpfield` with sdcflows' own `b02b0.cnf`
(seven levels, §2.5) and uses the field, not applytopup.

## 2. topup's command line

### 2.1 Syntax

Every option is a long option with its value after `=` (`--imain=pair.nii.gz`); `-v` and `-h`
also have their short forms. Options may come in any order; an option given twice keeps the last
value. Per-level options take a comma-separated list (`--warpres=20,16,14`), read piece by piece
as C's `atoi`/`atof` read them (a piece that is not a number reads as 0). A word that is not an
option is an error (§2.6).

### 2.2 Options

In the order of the usage text (the per-level options are marked L; §2.4):

| Option | Value | Default | Meaning |
|---|---|---|---|
| `--imain` | image | (compulsory) | the 4D image of the volumes, one per acquisition |
| `--datain` | file | | the acquisition parameters (§3.2) |
| `--acqp` | file | | the same, by its other name (exactly one of the two must be given) |
| `--out` | base name | `--imain`'s path and name before its first `.` | the outputs' base name: `<out>_fieldcoef`, `<out>_movpar.txt` (§6) |
| `--fout` | image name | | the field in Hz on the input's grid |
| `--iout` | image name | | the corrected volumes (4D), and their mean (§6) |
| `--featout` | base name | | the field in rad/s and the corrected mean, for FEAT (§6) |
| `--logout` | file | `--imain`'s path and name + `.topup_log` | the log of the settings (§2.7) |
| `--warpres` (L) | mm | 10 | the B-spline knot spacing (§4.2) |
| `--subsamp` (L) | integer | 1 | the subsampling factor (§4.4) |
| `--fwhm` (L) | mm | 8 | the Gaussian smoothing of the images (§4.4) |
| `--config` | file | | a configuration file of options (§2.5) |
| `--miter` (L) | integer | 5 | the iterations of the minimiser (§4.7) |
| `--lambda` (L) | float | 0 | the regularisation weight (§4.6) |
| `--ssqlambda` | 0/1 | 1 | the weight multiplied by the current sum of squared differences (§4.6) |
| `--regmod` | `bending_energy`, `membrane_energy` | `bending_energy` | the regulariser (§4.6) |
| `--estmov` (L) | 0/1 | 1 | estimate the movements (§4.3) |
| `--minmet` (L) | 0 (Levenberg-Marquardt), 1 (scaled conjugate gradients) | 0 | the minimiser (§4.7) |
| `--splineorder` | 2 or 3 | 3 | quadratic or cubic B-splines |
| `--numprec` | `double`, `float` | `double` | the Hessian's precision (§4.7) |
| `--interp` | `linear`, `spline` | `spline` | the images' interpolation (§4.5) |
| `--scale` | 0/1 | 0 | each volume scaled to a mean of 100 by itself instead of all by one factor (§4.1) |
| `--regrid` | 0/1 | 1 | work on a slightly finer grid (§4.4) |
| `--nthr` | integer | 1 | threads (§9.3) |
| `-h`, `--help` | flag | | the usage text |
| `-v`, `--verbose` | flag | | the cost of every evaluation on standard output (§5) |

Hidden options (not in the usage text): `--dfout` (base name: each volume's displacement field),
`--rbmout` (base name: each volume's rigid-body matrix), `--jacout` (base name: each volume's
Jacobian determinant) (§6); `--debug` (0 to 3: debug files of every cost evaluation, §5),
`--trace` (flag: the name of every internal stage entered and left, on standard output),
`--profile` (flag: `<out>.topup_profile_topup.cpp_main.txt`, one line `Entry: <name> Duration:
<seconds>` per stage: "Begins topup run" and "Begins topup level <l>") **(observed)**.

The default `--lambda` 0 means no regularisation; FSL's documentation's "default depending on
--ssqlambda and --regmod" refers to its shipped configurations, which set it **(observed:** no
`--lambda` and no configuration runs unregularised**)**.

### 2.3 The usage text

Printed to standard error with exit status 1 for no arguments, `-h`/`--help` (also with the
compulsory option set), a missing `--imain`, and before every option or setting error (§2.6),
exactly (the first line is empty; `-h,--help` is listed twice, as FSL lists it) **(observed)**:

```

Part of FSL (ID: 2412.6-dirty)
topup

Usage: 
topup --imain=<some 4D image> --datain=<text file> --config=<text file with parameters> --out=my_topup_results


Compulsory arguments (You MUST set one or more of):
	--imain		name of 4D file with images

Optional arguments (You may optionally specify one or more of):
	--datain	name of text file with PE directions/times
	--acqp		alternative way to specify text file with PE directions/times
	--out		base-name of output files (spline coefficients (Hz) and movement parameters)
	--fout		name of image file with field (Hz)
	--iout		name of 4D image file with unwarped images
	--featout		base-name of output for export to FEAT
	--logout	Name of log-file
	--warpres	(approximate) resolution (in mm) of warp basis for the different sub-sampling levels, default 10
	--subsamp	sub-sampling scheme, default 1
	--fwhm		FWHM (in mm) of gaussian smoothing kernel, default 8
	--config	Name of config file specifying command line arguments
	--miter		Max # of non-linear iterations, default 5
	--lambda	Weight of regularisation, default depending on --ssqlambda and --regmod switches. See user documetation.
	--ssqlambda	If set (=1), lambda is weighted by current ssq, default 1
	--regmod	Model for regularisation of warp-field [membrane_energy bending_energy], default bending_energy
	--estmov	Estimate movements if set, default 1 (true)
	--minmet	Minimisation method 0=Levenberg-Marquardt, 1=Scaled Conjugate Gradient, default 0 (LM)
	--splineorder	Order of spline, 2->Qadratic spline, 3->Cubic spline. Default=3
	--numprec	Precision for representing Hessian, double or float. Default double
	--interp	Image interpolation model, linear or spline. Default spline
	--scale		If set (=1), the images are individually scaled to a common mean, default 0 (false)
	--regrid		If set (=1), the calculations are done in a different grid, default 1 (true)
	--nthr		Number of threads to use (cannot be greater than numbers of hardware cores), default 1
	-h,--help	display help info
	-v,--verbose	Print diagonostic information while running
	-h,--help	display help info


```

(`2412.6-dirty` is FSL 6.0.7.23's utilities' version string; the spelling mistakes are FSL's.)

### 2.4 Levels

topup runs one estimation per **level**, coarse to fine. The number of levels is the longest of
the lists `--subsamp`, `--warpres`, `--fwhm`, `--miter`, `--lambda`, `--estmov`, `--minmet`; a
list of one value applies it to every level; a list of any other length is an error (`Mismatch
between --<option> and other parameters `, §2.6) **(observed)**.

### 2.5 Configuration files

`--config=<name>` reads more options from a text file: each line is `--option=value` (one word),
a line starting with `#` is a comment, blank lines are skipped; the text after a line's first
word is ignored. **An option on the command line wins over the same option in the file**,
whatever their order **(observed)**. The file is looked for as named; else, if the name has no
extension, with `.cnf` added; else, if the name has no directory part, in
`$FSLDIR/etc/flirtsch/` (as named, then with `.cnf`). A directory of that name counts as found
(and gives no option). Not found: `Cannot find config-file <name>` on standard error, exit 1
(FSL-121 when FSLDIR is unset, §10). An unknown option in the file: the usage text and its
message, exit 1. FSL ships `b02b0.cnf` (nine levels, the movements estimated in the first five),
`b02b0_1.cnf`, `b02b0_2.cnf`, `b02b0_4.cnf` (for images whose sizes are divisible by 1, 2 or 4:
the subsampling of each), `b02b0_7T.cnf` and `test_acqp.cnf` **(observed:** each runs, named
without its path and extension**)**.

sdcflows' `b02b0.cnf` (fMRIPrep's call; MD5 025014fc26cf49e82840f6971d35cd11), seven levels:

| Level | 1 | 2 | 3 | 4 | 5 | 6 | 7 |
|---|---|---|---|---|---|---|---|
| `--warpres` (mm) | 14 | 12 | 10 | 6 | 4 | 4 | 4 |
| `--subsamp` | 2 | 2 | 2 | 1 | 1 | 1 | 1 |
| `--fwhm` (mm) | 6 | 4 | 3 | 2 | 1 | 0 | 0 |
| `--miter` | 5 | 5 | 5 | 10 | 10 | 20 | 20 |
| `--lambda` | 0.001 | 0.0001 | 5e-06 | 5e-07 | 5e-08 | 5e-10 | 1e-11 |
| `--minmet` | LM | LM | LM | SCG | SCG | SCG | SCG |

with `--ssqlambda=1 --regmod=bending_energy --estmov=0 --splineorder=3 --numprec=double
--interp=spline --scale=1 --regrid=1` (read the file itself for the exact values).

### 2.6 Messages of the command line and the settings (standard error, exit status 1)

- An unknown option: the usage text, an empty line, `<option>: Option doesn't exist!`; a value
  that does not read: `<option>: Couldn't set_value! valstr="<value>"!`; a missing value:
  `<option>: Missing non-optional argument!` **(observed)**.
- The checks of the settings, each printed as the usage text, an empty line and
  `Topup::topup_clp::topup_clp: <text>` **(observed)**, in this order: the `--imain` header
  cannot be read (the image library's own lines: `Failed to read volume <name>` and `Error : No
  image files match: <name>`, without the usage); both `--datain` and `--acqp` (`Only one of
  --datain and --acqp can be set.`); neither (`Exactly one of --datain and --acqp must be
  set.`); the acquisition file cannot be read or has neither 4 columns nor 4 rows
  (`TopupDatafileReader:: error reading file <name>`, from the matrix reader's own message
  lines); a phase-encoding vector whose length differs from 1 by more than 0.01
  (`TopupDatafileReader:: phase-encode vectors must be unity length`); fewer or more rows than
  volumes (`Mismatch between <imain> and <datain>`, FSL-120); a read-out time outside
  0.01-0.2 s, except one row whose time is below 1e-6 (the last such row is excused: `Unrealistic
  read-out time, should be in range 0.01-0.2 s`); the per-level lists (§2.4); a subsampling below
  1 or not dividing the image's three sizes (`Subsampling levels incompatible with image
  data.`); a `--minmet` other than 0 or 1 (`Unrecognized minimization method code.`); an unknown
  `--numprec`, `--regmod` or `--interp` (`Unrecognized value <v> as argument to --<option>.`); a
  `--splineorder` other than 2 or 3 (`Only quadratic or cubic splines allowed.`).
- A warning on standard output, the run continuing: `topup_clp::topup_clp: WARNING, it is
  generally NOT a good idea to include movements when using the Scaled Conjugate Gradient
  method.` for each level that estimates the movements with SCG **(observed)**.
- `--nthr`: 1 is one thread; another value is used when it is at most the number of hardware
  threads, else that number; 0 means FSL's default thread count (`FSL_NUM_THREADS`, else 1); a
  negative value the hardware count **(observed)**. The results depend on it (FSL-116, §9.3).
- `--debug` above 3 or negative: refused when the estimation is prepared (`Error occurred when
  preparing to run topup` + `Exception thrown with message: Topup::TopupCF::SetDebug: Debug level
  out of range`); a negative `--splineorder` is the spline-order message above **(observed)**.

### 2.7 The log file

Written after the settings are checked, before the estimation: for each option, a comment line
`# <its help text>` and the line `--<option>=<value>` (lists comma-separated, numbers as C++
prints them with 6 significant digits), in this order: `--imain`, `--datain`, `--acqp`, `--out`
(even when not set: an empty value), `--fout`, `--iout`, `--featout` (only when set), then
`--warpres`, `--subsamp`, `--fwhm`, `--miter`, `--lambda`, `--ssqlambda`, `--estmov`, `--minmet`,
`--regmod`, `--splineorder`, `--numprec`, `--interp`, `--scale`, `--regrid`, `--nthr` (the values
in force: the command line's, the configuration's, or the defaults) **(observed)**. The help
texts are the usage text's, each keeping its leading tab. A log that cannot be written is skipped
silently (rule C, §10.2).

## 3. Inputs

### 3.1 The images

`--imain` is read as FSL's image layer reads every image (`specs/bet.md` §3.1 and §3.2: the
name with or without its extension, the formats and compressions, the data converted to 32-bit
float with `scl_slope`/`scl_inter` applied, the radiological storage mirrored along x when read
and back when written). A 3D image is one volume. The voxel sizes are the absolute header sizes.
Every computation uses the stored axes (after the mirroring): the phase-encoding vectors refer to
them.

### 3.2 The acquisition parameters (`--datain`, `--acqp`)

A text matrix: one row per volume of `--imain`, four numbers: the phase-encoding vector `(px,
py, pz)` in the image's voxel axes and the total read-out time `t` in seconds (the time between
the centres of the first and last echoes). A file of four rows and as many columns as volumes is
read transposed. `pz` must be 0 (else `Error occurred when preparing to run topup` / `Exception
thrown with message: Topup::TopupScan::TopupScan: third element of pevec must be zero`, exit 1)
**(observed)**. The vector's length must be 1 within 0.01 (§2.6); its components are used as
given (an oblique `0.6 0.8 0` is two components) **(observed)**. A displacement of `f` Hz at a
voxel moves volume `i`'s signal by `f · t_i · p_i` voxels along each axis (`· voxel size` in mm).

## 4. The estimation

### 4.1 Intensity scaling

Each volume's mean `m_i` over every voxel (FSL's image sum: double-precision partial sums over
fixed blocks, as `specs/fast.md` §8.3 describes, divided by the voxel count) and their mean `g`
are computed;
then every volume is multiplied by `100 / g` (default), or volume `i` by `100 / m_i` with
`--scale=1` **(observed)**. The outputs that are images of intensities (`--iout`, `--featout`'s
magnitude) are multiplied back by `g / 100` (§6).

### 4.2 The field

The field `h(x)` (Hz) is a tensor-product B-spline of order `--splineorder` (cubic by default)
on the working grid (§4.4), with knot spacing `k_d = round(warpres / voxel size_d)` voxels along
each axis (1 when that rounds to 0), the voxel sizes those of the level's target grid (below).
Its coefficients cover the grid with the knots placed as FSL's spline fields place them, centred
on the grid: for `n` voxels and spacing `k`, `n` coefficients when `k = 1`, else `ceil((n + 1) /
k) + 2` for cubic splines and, for quadratic ones, `floor(n / k) + 1` increased until the last
basis function reaches the last voxel **(observed:** the `_fieldcoef` sizes of every suite
case**)**. The field starts at 0. At each change of
grid or knot spacing the current field is sampled on the new grid and refitted (§4.4).

### 4.3 The model of one volume

For volume `i` at a level, with its phase encoding `p_i`, read-out time `t_i`, rigid-body
movement `M_i` (six parameters: three translations in mm and three rotations in radians, as an
Euler matrix about the centre of the field of view, `((n-1)·voxel size)/2` per axis; volume 1's
is always the identity) and the field `h`:
- the displacement field `d_i(x) = h(x) · t_i · (px_i · dx, py_i · dy, 0)` mm;
- the predicted undistorted volume `u_i(x) = J_i(x) · f_i(M_i · x + d_i(x))`: the smoothed,
  subsampled input (§4.4) interpolated (§4.5) at the moved and displaced position, times the
  Jacobian determinant `J_i` of the displacement (`1 + ∂d/∂y`-like: the determinant of `I +
  ∇d_i` in voxel units; along one axis `1 + t_i p_i ∂h/∂axis` computed from the spline's
  derivative, with the same field scaled; along both, the three-component determinant);
- a validity mask: 1 where the interpolation stayed inside the image along the axes other than
  the phase encoding (the phase-encoding axis wraps: the image is periodic along it), 0 else;
  the first and last slices (z) are always 0, and the first and last rows along the phase
  encoding are 0 when the encoding is along one axis.

The movements are estimated (`--estmov=1`) for every volume after the first, six parameters
each; when the phase encodings use only one axis, the first volume whose encoding has the
opposite sign of the first one gets five parameters (the translation along the phase encoding
is not estimable and stays 0) **(observed)**. Movements of the subsampled images are applied in
their own grid.

### 4.4 Grids: regridding, subsampling, smoothing

- **Regridding** (`--regrid=1`): the input is resampled once onto a grid of `n + s_max` voxels
  per axis (`s_max` the largest subsampling of all levels) spanning the same field of view: voxel
  size `((n - 1) · dx - 1e-6) / (n + s_max - 1)`, voxel `j` sampled at input position `j · (new
  size / dx)` by the level-1 interpolation (§4.5) **(observed)**. With `--regrid=0` the input
  grid is used. (FSL-118: subsamplings whose largest does not divide the others.)
- **Subsampling** `s`: the regridded image's `s × s × s` blocks averaged (the sum in single
  precision, in x-fastest order, divided by `s³`); the image sizes must be divisible by `s`.
- **The target grid** of a level is the *original* image size divided by `s`, with voxel sizes
  `s ·` the original's: the field, the predicted volumes and the cost live on it, while the
  interpolation reads the subsampled regridded image (positions scaled between the two)
  **(observed)**.
- **Smoothing**: each subsampled image is convolved with a Gaussian of standard deviation
  `σ = fwhm / sqrt(8 ln 2)` mm, separably along x, then y, then z: along each axis `σ_v = σ /
  voxel size` voxels, a kernel of `2r + 1` taps, `r = 2 · trunc(σ_v - 0.001) + 3`, the values
  `exp(-j² / (2 σ_v²))` (double `exp`, each rounded to single precision, summed in single
  precision) scaled by the inverse of their sum; each output voxel the single-precision sum, in
  kernel order, of the double products; voxels beyond the grid taken periodically (the image
  wraps along every axis); FWHM 0 is no smoothing.
- At the start of each level after the first, in this order: the subsampling changes (the field
  is sampled at the centres of the new grid's voxels, `(step - 1)/2 + j · step` in the old
  grid's voxels, `step = new s / old s`, and refitted with the *previous* level's knot spacing),
  the smoothing changes, the knot spacing changes (the field sampled at the voxels and refitted),
  the movements fixed or estimated, the weight set **(observed)**.
- **Refitting** a field to a sampled volume: per axis, the coefficients minimising the squared
  error plus `0.005²` times the squared second differences of the coefficients (a small
  curvature regularisation), applied one axis at a time.

### 4.5 Interpolation

`--interp=spline` (default): cubic B-spline interpolation of the image (its coefficients from
the smoothed image with periodic extrapolation along the phase encoding), the partial derivatives
along x, y and z from the same spline; `--interp=linear`: trilinear interpolation and its
derivatives. Outside the image the phase-encoding axis wraps periodically; the other axes mark
the voxel invalid (§4.3).

### 4.6 The cost

With `m` volumes, `n` the number of voxels where every volume's mask is 1, the mean predicted
volume `ū = (1/m) Σ u_i` and the regulariser `E(h)`:

`C = (1 / (n (m - 1))) Σ_i Σ_masked (ū - u_i)² + λ' · E(h)`

where `λ' = λ · SSD` with `--ssqlambda=1` (SSD the current first term) and `λ' = λ` otherwise.
`E` is the bending energy (the integral of the squared second derivatives, all six of them, in
mm) or, with `--regmod=membrane_energy`, the membrane energy (squared first derivatives).
Movements are not regularised. The gradient with respect to the field's coefficients is the
chain rule through the interpolated derivatives along the phase encoding and the Jacobian (the
image derivative times `t_i p_i`, plus the intensity times the derivative of the Jacobian with
respect to the coefficients), with respect to the movements through the moved positions; all in
double precision from single-precision images.

### 4.7 The minimisers

Per level, from the previous level's result (the field's coefficients and the movements), for at
most `--miter` successful iterations:
- **Levenberg-Marquardt** (`--minmet=0`): the Gauss-Newton Hessian `2/(n(m-1)) Σ JᵀJ` (the first
  derivatives of the residuals only) plus `λ'` times the regulariser's Hessian; sparse (the
  coefficients' overlaps), with the movement block dense, in double or, with `--numprec=float`,
  single precision; the damping multiplies the diagonal by `(1 + λ_LM)` (λ_LM starts at 0.1,
  divided by 10 after a step that lowers the cost, multiplied by 10 after one that does not,
  the step retried with the same Hessian; the level ends when `λ_LM` exceeds 1e20); the step
  solves `H δ = -g` by conjugate gradients with the Hessian's diagonal as preconditioner (at most
  500 iterations, relative tolerance 1e-3); it ends early when two successive costs agree
  within `2|a - b| ≤ 1e-8 (|a| + |b| + ε)`.
- **Scaled conjugate gradients** (`--minmet=1`): Møller's method (σ = 0.01, λ starting at 0.1,
  multiplied by 4 when the comparison ratio is below 0.25 and halved above 0.75, restarted every
  number-of-parameters iterations, Polak-Ribière's β), on the gradient only; it ends early when
  `max |g_j| · max(|p_j|, 1) / max(C, 1) < 1e-8`.
An iteration counts when its step is accepted. `--miter=0` runs no iteration. The movements are
part of the parameter vector where estimated. The minimiser's failure stops topup with `Error
occurred when running level <l> of topup` (level 1: `first level`) and the exception's text,
exit 1.

## 5. Messages while running

- `-v`: one line per cost evaluation on standard output, `SSD = <s>\tn = <n>\tReg = <r>\tCost =
  <c>` (6 significant digits), and `***Going to next resolution level***` before each level
  after the first **(observed)**.
- `--trace`: `Entering <stage>` / `Leaving <stage>` lines on standard output **(observed;** the
  replica reproduces FSL's list**)**.
- `--debug=1..3`: after every cost evaluation the files `TopupDebugUnwarped`,
  `TopupDebugJacobians`, `TopupDebugField`, `TopupDebugMovementParameters...txt` (level 2 adds
  `TopupDebugMask`, `TopupDebugMaskedDiff`), named with the level, iteration and attempt; the
  gradients (`TopupDebugGradient_...txt`, with and without the regulariser) and, at level 3,
  the Hessian's weight images **(observed)**.
- Errors after the parse: `Error occurred when reading --imain file <name>` / `Exception thrown
  with message: <text>`; `Error occurred when preparing to run topup`; `Error occurred when
  preparing to run level <l> of topup`; `Error occured while writing output from topup` (FSL's
  spelling); each with `Exception thrown with message: <text>` and exit 1 **(observed)**. An
  error escaping these: `TOPUP::: Topup failed with message <text>` on standard output, exit 1.
- An output type FSL does not know (`FSLOUTPUTTYPE`): the image library's message and exit 1
  (`specs/bet.md` §3.1).

## 6. topup's outputs

After the last level, if it was subsampled, the smoothing and subsampling are removed (the
field kept) so the outputs are on the full grid. Then, in this order:

| File | When | Content |
|---|---|---|
| `<out>_fieldcoef` | always | the coefficients (Hz) as an image of the coefficients' grid, float32: voxel sizes the knot spacings (voxels), intent code 2016 (cubic) or 2017 (quadratic) with the intent parameters the field's voxel sizes (mm), a scanner-anatomical qform of the identity with the translation the field's size in voxels (x, y, z), no sform **(observed)** |
| `<out>_movpar.txt` | always | one row per volume: the three translations (mm) and three rotations (rad), each printed with 10 significant digits followed by two blanks **(observed)** |
| `--fout` | set | the field (Hz) on the input's grid, the input's header, intent code 2018 (FSL-119: the intent parameters), display range 0 0 |
| `--iout` | set | the predicted undistorted volumes `u_i` (§4.3) at full resolution, times `g / 100`, 4D, the input's header |
| `<iout>_mean` | `--iout` | their mean over the voxels inside the common mask (outside it, the first volume) times `g / 100`; the name is `--iout`'s value as given plus `_mean`, so `--iout=x.nii.gz` writes `x.nii.gz_mean.nii.gz` (rule C) **(observed)** |
| `<featout>_fieldmap_rads`, `<featout>_fieldmap_mag` | `--featout` | the field times `2 · 3.14159265358979323846` (rad/s), the mean corrected image |
| `<dfout>_01`, `_02`, ... | `--dfout` | each volume's displacement field (mm; 3 volumes x, y, z; z always 0) |
| `<rbmout>_01.mat`, ... | `--rbmout` | each volume's rigid-body matrix (4 × 4, 10 significant digits, two blanks after each number); a file that cannot be opened prints `Could not open file <name> for writing` and the run goes on (exit 0) **(observed)** |
| `<jacout>_01`, ... | `--jacout` | each volume's Jacobian determinant `J_i` |

Images are written in `FSLOUTPUTTYPE`'s type, float32, with FSL's image library's `descrip`.
A movement-parameter file that cannot be written is an error (§5).

## 7. applytopup

### 7.1 Command line

Options with their values after `=` (long) or as the next word (short). Usage text (standard
error, exit 1: no arguments, `-h`, a missing compulsory option, and before an option error)
**(observed)**:

```

Part of FSL (ID: 2412.6-dirty)
applytopup (Version 1.0)
Copyright(c) 2009, University of Oxford (Jesper Andersson)

Usage: 
applytopup -in=topdn,botup --topup=mytu --inindex=1,2 --out=hifi
applytopup -in=topdn --topup=mytu --inindex=1 --method=jac --interp=spline --out=hifi


Compulsory arguments (You MUST set one or more of):
	-i,--imain	comma separated list of names of input image (to be corrected)
	-a,--datain	name of text file with PE directions/times
	-x,--inindex	comma separated list of indicies into --datain of the input image (to be corrected)
	-t,--topup	name of field/movements (from topup)
	-o,--out	basename for output (warped) image

Optional arguments (You may optionally specify one or more of):
	-m,--method	Use jacobian modulation (jac) or least-squares resampling (lsr), default=lsr.
	-n,--interp	interpolation method {trilinear,spline}, default=spline
	-d,--datatype	Force output data type [preserve char short int float double].
	-v,--verbose	switch on diagnostic messages
	-h,--help	display this message


```

A missing compulsory option prints FSL's `COMPULSORY` block before the usage; option errors as
topup's (§2.6). `--invert` is not an option ("Option doesn't exist"); `-v` is accepted and has no
effect (rule C). `--inindex` pieces are read as C's `atoi` (`1x` is 1, `a` is 0).

### 7.2 Inputs and checks

Every check below is an uncaught error in FSL: its text on standard error after `terminate
called after throwing an instance of '<type>'` / `  what():  <text>`, and FSL aborts (signal 6);
larmorx prints the same lines, then `larmorx-nc: applytopup aborts here (signal 6); the replica
exits with status 1`, exit 1 **(observed)**. In order:
- `--interp` other than `trilinear` or `spline`: `ApplyTopup::Unknown interpolation type <v>`;
- the number of `--imain` names differs from `--inindex`'s: `ApplyTopup::Mismatched --in and
  --inindex lists`;
- `--datatype` unknown: `ApplyTopup::Unknown data type <v>` (`preserve` takes the first image's
  type, read before any image);
- each image read as topup's (§3.1), 3D or 4D; an image of another size than the first:
  `ApplyTopup::Mismatched input files <name> and <first>`;
- the acquisition file read as topup's (§3.2); an index 0 or beyond its rows: `ApplyTopup::Invalid
  entry in --inindex list`;
- `--topup`: a base name **without extension** (a `.` after the last `/` is refused:
  `TopupFileReader::TopupFileReader: Filename must be given without extension, fname=<name>`);
  `<name>_fieldcoef` (an image with intent 2016 or 2017, §6) with `<name>_movpar.txt` is a topup
  result; else `<name>` itself is a field image in Hz (intent 2018, or any other image: a field
  map); neither: `Unable to read file <name>`. A missing `_movpar.txt` prints `Could not open
  matrix file <name>_movpar.txt` and gives no movements (identity); a movement file of six
  columns gives each row's matrix about the field's centre, one of four columns and 4k rows gives
  the matrices themselves (used by `lsr` and the `vb` methods, not by `jac`) **(observed)**. The
  coefficients' field size comes from the qform's translation and its voxel sizes from the
  intent parameters, the knot spacing from the voxel sizes rounded.

### 7.3 `--method=jac`

For each image `i` (its acquisition row `r_i`): the field on the image's grid (a coefficient
field evaluated; a field image as stored, FSL-134 for other grids), the displacement `d = h ·
t · (px · dx, py · dy, 0)` mm, the image moved by its movement matrix (identity without
movements or for a field image) and interpolated at `x + d` (`--interp`, cubic splines of order
3 by default; periodic along its phase encoding as in §4.5), each volume of a 4D image alike;
times the Jacobian determinant of `d` (from the spline field's derivatives; for a field image,
from the cubic-spline derivatives of the displacement images, zero outside the image: FSL-134); the corrected images averaged
over the inputs (volume by volume) **(observed)**.

### 7.4 `--method=lsr` (least-squares restoration)

First each image is moved by its movement matrix onto the first's space (interpolated with
`--interp`); FSL-132 concerns the first volume of the first image. The images are divided into
**collections** by the axis of their phase encoding (x or y; a vector with both, or neither:
`ApplyTopup::Invalid phase-encode vector for least-squares restoration`); each collection must
hold at least two different products `p · t` along its axis (else `ApplyTopup::Invalid
combination of phase-encode vectors for least-squares restoration`). For each collection, each
slice and each line along its axis (a line with any voxel outside the movements' mask is
skipped and written 0 in every collection's output):
- the line's field values `h_j` (Hz) give each image `a` a mapping `φ_a(j) = j + h_j · p_a t_a`
  (voxels) of undistorted voxel `j` to its distorted position, linear between voxels and
  periodic (the line wraps; FSL-133);
- `K_a(i, j)` = the length (in undistorted voxels) of the part of voxel `j`'s extent `[j - 1/2,
  j + 1/2]` that `φ_a` maps into observed voxel `i`'s extent `[i - 1/2, i + 1/2]`: the
  intensity-conserving resampling matrix of Andersson et al. (2003); `K` stacks the collection's
  `K_a`;
- `S` the second-difference matrix of the line (rows `(-1, 2, -1)`, the first and last rows
  `(1, -1)` and `(-1, 1)`);
- the restored line `f = (KᵀK + 0.01 SᵀS)⁻¹ Kᵀ y`, `y` the collection's observed lines stacked,
  each volume of 4D images alike;
- the restored lines of the collections are **added** (two collections, x and y, give their sum).
The output is masked by the movements' mask (§7.6).

### 7.5 `--method=vb2D`, `vb3D`, `vb4D` (undocumented)

As `lsr` (movements, collections, `K`), with the restoration's regularisation weight and the
noise precision inferred by variational Bayes: the noise precision `φ` and the prior precision
`λ` with Gamma priors of shape `1e-10` and scale `1e10`; the image prior a second-difference
smoothness across lines: within a slice (`vb2D`: `φ` and `λ` per slice and volume, 50
iterations), across slices too (`vb3D`: per volume, 20 iterations), over every volume at once
(`vb4D`: one `φ` and `λ`, 20 iterations). The first estimate uses the line prior `(6, -4, 1)`
(the second differences' square); the updates alternate `φ`, `λ` and the images. Printed on
standard output **(observed)**: `NCollections = <n>`, `NScans = <n>` (collection 1's count),
then `vb2D`: `slice = <s>` and per iteration `relative lambda = <x>, phi = <φ>, lambda = <λ>`
(lower-case `relative`), then `slice = <s>,   relative lambda = <x>`; `vb3D`/`vb4D`: per
iteration `Relative lambda = <x>, phi = <φ>, lambda = <λ>`, `Updating phi`, `Updating lambda`,
`Updating image` (`vb3D`) or `Updating images` (`vb4D`); numbers with 6 significant digits. The
defects FSL-141 to FSL-143 (§10) concern these methods. A method name other than the five
(FSL-135) and a `lsr`/`vb` run without movements (FSL-135) are refused by default.

### 7.6 The output

`--out`: the first image's header (4D with the images' volume count), display range 0 0, every
voxel outside the mask (the product of every resampling's validity: all images, all volumes) 0;
float32, or `--datatype`'s type (values rounded as the FSL replicas round integer outputs:
FSL-09 of `docs/api/upstream-defects.md`) **(observed)**.

## 8. Messages and exit statuses (summary)

| Status | When |
|---|---|
| 0 | the outputs were written (also when a `--rbmout` or log file could not be written) |
| 1 | topup: every error of §2.6 and §5; larmorx's refusals (§10); applytopup: the usage, option errors, larmorx's refusals and FSL's aborts (larmorx exits 1 where FSL aborts with signal 6) |

## 9. Numeric conventions that decide the bits

### 9.1 Precision

Images are single precision; the cost's sums, the field's coefficients, the gradients and the
movements double precision; the Hessian double or single (`--numprec`). The interpolation and its
derivatives in single precision from double-precision spline weights. topup's matrix arithmetic
(the refits' small systems, the movements' matrices, the Levenberg-Marquardt solver's dense
parts) follows Armadillo 14.2.1 with OpenBLAS 0.3.33 on one thread (FSL's programs force
OpenBLAS to one thread unless `FSL_SKIP_GLOBAL` is set); the conjugate-gradient solver's sparse
products and dot products are serial sums in storage order.

### 9.2 Sums

The intensity means (§4.1) are double sums over the volume in storage order. The cost and the
number of mask voxels are accumulated in storage order (x fastest), volume by volume.

### 9.3 Threads

FSL's `--nthr` (and `FSL_NUM_THREADS`, which sets the thread count of the sparse matrices topup
builds) change the order of the sparse matrix-vector products' partial sums, so FSL's results
depend on them (FSL-116, class b): `--nthr=4` moved the field by up to 0.032 Hz and the corrected
images by up to 88 (of about 20,000) on the half-resolution pair **(observed)**. The reference
is FSL's one-thread result; larmorx gives it at any thread count.

### 9.4 applytopup

The restorations' matrices are dense per line (`KᵀK` is not banded in general: the line wraps),
solved by LU factorisation with the explicit inverse applied to `Kᵀy` (FSL's arithmetic: OpenBLAS
0.3.33's LU with partial pivoting and Armadillo's band and triangular shortcuts where the matrix
is banded, e.g. a zero field). The vb methods' traces and solves follow Armadillo's evaluation of
`inv(A) · B` as a linear solve.

## 10. Defects (D13): what the default does

### 10.1 The register's entries

| ID | Behaviour of FSL | Default (`fix`) | `reproduce` |
|---|---|---|---|
| FSL-116 | results depend on `--nthr` and `FSL_NUM_THREADS` (§9.3) | FSL's one-thread result at any count | (no switch: the one-thread result) |
| FSL-117 | with `--regmod=membrane_energy`, Levenberg-Marquardt's Hessian of the membrane energy is in other units (mm) than its cost and gradient (voxels) | the Hessian in the cost's units; named on every LM run with the membrane energy and λ ≠ 0 (final cost 0 to 1.66 % lower, measured) | FSL's Hessian |
| FSL-118 | `--regrid=1` with subsamplings whose largest does not divide the others (3 and 2): `TopupScan::SubSample: invalid subsampling factor` | the grid grows by the subsamplings' least common multiple (identical wherever FSL runs) | FSL's error |
| FSL-119 | `--fout`/`--featout` field maps get the input's intent parameters shifted by one place (0, p1, p2) | (p1, p2, p3); named when they differ | FSL's |
| FSL-120 | `--acqp` with a wrong row count: `Mismatch between <imain> and ` (the empty `--datain`) | names the `--acqp` file | FSL's text |
| FSL-121 | FSLDIR unset and a `--config` name not found locally: `basic_string: construction from null is not valid`, exit 1 | `larmorx-nc: error: topup: --config=<name>: no such file here, and FSLDIR is unset ... (FSL-121)`, exit 1 | FSL's text |
| FSL-122 | a configuration line whose text after its first word exceeds 1023 characters ends the reading (the rest of the file ignored silently) | lines of any length; named when a setting followed | FSL's |
| FSL-123 | a negative `--miter` runs no iteration at that level, silently | refused | FSL's |
| FSL-124 | a negative `--warpres` whose ratio to the voxel size rounds below 0 gives a knot spacing of about 4.29e9 voxels: memory exhaustion | refused (both modes refuse where FSL's spacing wraps) | refused where it wraps |
| FSL-125 | a negative `--fwhm` smooths with a negative-width kernel | refused | FSL's |
| FSL-126 | a negative `--lambda` makes the cost negative, the field large | refused | FSL's |
| FSL-132 | applytopup: the first volume of the first image is resampled trilinearly whatever `--interp` says (the default spline) | every volume with the method asked for; named | that volume trilinear |
| FSL-133 | applytopup `lsr`/`vb`: a line's wrap-around reads the wrong voxel for line lengths that do not divide 2³² (30, 96, 100, ...) | the circular wrap; named when a read differs | FSL's |
| FSL-134 | applytopup with a field image: `jac` scales the y displacement by the x voxel size; a field on another grid than the images is not refused (laid as stored, or read past its lines with `lsr`) | the y voxel size; other grids refused | FSL's (`lsr` with another grid refused in both modes) |
| FSL-135 | applytopup: an unknown `--method` writes nothing and exits 0; `lsr`/`vb` without movements abort (`TopupFileReader::MoveMatrix: Index i out of bound`) | clear refusals | FSL's |
| FSL-141 | `vb*`: the Gamma shapes' counts in 32-bit integers wrap beyond 2³¹ values | 64-bit counts | the wrapped values |
| FSL-142 | `vb3D`/`vb4D` count `2 m` data points per line whatever the collection holds (`vb2D` counts every image) | the collection's count; named | FSL's |
| FSL-143 | `vb3D`/`vb4D`'s prior couples neighbouring slices with half the weight of the in-slice coupling: the precision is indefinite and the iterations diverge (an output of ±3.9e10 for an input of 0 to 1.1e5) | the consistent coupling; named on every `vb3D`/`vb4D` run | FSL's |

FSL-127 to FSL-131 are defects of FSL's libraries that no path of topup or applytopup reaches
(register only). Each correction that changed a result is named on standard error
(`larmorx-nc: upstream defect FSL-<n> (topup) corrected: ...`) and in Python as an
`UpstreamDefectWarning`.

### 10.2 Kept as FSL's (rule C, not registered)

`--iout=x.nii.gz`'s mean named `x.nii.gz_mean`; a log file that cannot be written skipped
silently; `--rbmout` into a missing directory prints its message and goes on; two volumes with
the same phase encoding run (the field set by the regularisation alone); `--miter=0`; `--nthr`'s
rules (§2.6); z phase encoding refused by FSL's own message; applytopup's `-v` without effect,
`--invert` unknown, `--inindex`'s `atoi`, `jac` without movements for a field image, the vb
methods' printed collection-1 count, the first VB estimate's line prior, `vb2D`'s lower-case
`relative`; the integer halving of the vb counts (rule C: 0.5 in thousands).

### 10.3 Not replicated

Every option of topup is replicated. The replica of applytopup refuses, in both modes, the
least-squares and vb restorations of lines of fewer than 8 or more than 256 voxels along the
phase encoding, and band systems whose shape needs LAPACK's blocked band factorisation (a field
exactly zero over long stretches), with `larmorx-nc: ... not replicated` messages: FSL's
arithmetic there follows OpenBLAS's and Armadillo's paths for other sizes, which the replica does
not reproduce yet. The original need not refuse them: its thresholds do not need FSL's bits.

## 11. Built for speed

Where FSL spends its time (fMRIPrep's call on ds005040, 64 × 64 × 40 × 2, about 33 s with FSL and
30 s with the replica on one thread): at every cost evaluation, each volume's interpolation with
its three partial derivatives (spline) and the Jacobian at every voxel; at every
Levenberg-Marquardt iteration, the Gauss-Newton Hessian (the products of the basis functions'
overlaps weighted by the images' derivatives: the largest part at the fine levels) and its
conjugate-gradient solve; the refits at each grid change; at the end, the outputs at full
resolution.

What can run in parallel deterministically:
- the volumes' predictions, Jacobians and masks (per volume, per slice);
- the Hessian's assembly (per coefficient column: each column's entries are sums over the
  voxels its basis function covers; keep each sum's order fixed, or use a reproducible tree);
- the cost and gradient sums: fixed blocks summed in a fixed order (the thresholds of §12 allow
  any fixed order; FSL's own threaded run is not a reference: FSL-116);
- applytopup: every line is independent (lines in parallel, each solved alone); the band
  structure of `KᵀK` (nonzero only near the diagonal and in the wrap-around corners) admits a
  solve in `O(n)` per line instead of a dense inverse.
The minimisers are serial in their iterations.

## 12. Validation plan

Against FSL 6.0.7.23 (`~/fsl-6.0.7.23/bin/topup`, `applytopup`), through `python -m
larmorx_validation parity` (the replica's suite, `fsl-topup-replica`,
`docs/validation/fsl-topup-replica.md`, is the model; the original's suite runs the same cases
with `implementation="original"` and the oracle cache's stored FSL runs):
- **Cases**: fMRIPrep's calls on its oracle runs (ds005040, a Philips 3 T pair along y,
  64 × 64 × 40 of 3 × 3 × 3.3 mm; ds001600, 80 × 80 × 60 of 3 mm); every option of §2.2 on the
  half-resolution pair (precision, interpolation, regularisers, spline orders, scaling,
  regridding, minimisers per level, movements, FSL's shipped configurations, x and y phase
  encodings, both axes in four volumes, oblique vectors, a zero read-out time); every message and
  failure of §2.6, §5 and §7.2; applytopup's methods on topup's own outputs (lsr, jac, the vb
  methods; lines of 30, 32 and 96 voxels; 4D images; every `--datatype`; field images); the
  defects of §10 in both modes; then the data panel's PEPOLAR runs in the final validation phase.
- **Metrics**, each output against FSL's:
  - exit status, messages and file names: identical (verbose numbers to 4 significant digits);
  - the field (`--fout`, Hz) inside the brain (the corrected mean above 10 % of its 99th
    percentile): the 99th percentile of the absolute difference ≤ 0.5 Hz and the mean ≤ 0.1 Hz;
    the correlation with FSL's ≥ 0.999;
  - the coefficients: the field they define, as above (the coefficients themselves may differ
    where the basis is flat);
  - the movements: translations within 0.05 mm, rotations within 0.001 rad;
  - `--iout`, the mean and applytopup's outputs: the relative difference inside the mask ≤ 1e-3
    at the 99th percentile, the mean of the corrected pair's absolute difference ≤ 1e-3 of its
    mean intensity;
  - the Jacobians and displacement fields: the same bounds as the field, scaled by `t · voxel
    size`;
  - the final cost (`-v`'s last line): within 0.1 %.
  The thresholds are first checked against the band of a last-bit change: the replica with
  correctly rounded `exp` and `log` (instead of glibc's) and FSL at `--nthr=2` and `4` (its own
  thread variability, FSL-116) on the standard tier; the band measured there is the original's
  allowance where it is wider than the numbers above, reported in its record.
- **FSL's own variability**: topup at `--nthr=1` is deterministic (the same input gives the same
  bits run after run); at other thread counts it varies (§9.3): that spread is reported.
- **Speed**: against FSL and the replica on fMRIPrep's two calls (single thread and all cores),
  reported, not gated.
