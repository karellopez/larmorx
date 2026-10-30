# Spec: `lx.mri.hmc` (behaviour of FSL mcflirt, FSL 6.0.7.23; first written for 6.0.7.17)

The input for the clean-room **original** of larmorx's head-motion correction (crate
`larmorx-mri`, Apache-2.0, `lx.mri.hmc`, `larmorx mri hmc`). FSL is under the FSL Licence
(non-commercial). So larmorx has two implementations (CLAUDE.md rule 1, `docs/licensing.md`):
- the original, in the main package, written from this file, the published papers, FSL's
  documentation and black-box runs of the FSL binary only;
- later, a bit-exact replica of FSL's code, in `larmorx-nc` (`crates-nc/`).

**Whoever works on the original must not open:**
- FSL's source code: `~/fsl/src/` and any other copy of FSL's source (`fsl-mcflirt`,
  `fsl-flirt`, `fsl-newimage`, `fsl-miscmaths`, `fsl-armawrap`, `fsl-newnifti`, `fsl-utils`,
  ...), including FSL source files placed in `crates-nc/`;
- the replica crates (`crates-nc/`);
- `docs/findings/fsl-mcflirt.md`.

This spec describes behaviour: what goes in, what comes out, the maths and the numeric
conventions that decide the results. Statements marked **(observed)** were confirmed by
running the FSL binary (oracle runs in `<workspace>/oracles/fsl-6.0.7/mcflirt/`, made by
`scripts/run_mcflirt_oracle.py`, §14). The rest come from FSL's documentation and from
analysis of the program's behaviour.

**Version.** FSL 6.0.7.23 (Linux x86-64 conda build) since 2026-10-29: everything below holds for
it; the lines marked "(FSL 6.0.7.23)" say where it differs from FSL 6.0.7.17, the version this
spec was first written for (its packages in the table; 6.0.7.23's: fsl-mcflirt 2111.0,
fsl-newimage 2601.0, fsl-miscmaths 2412.6, fsl-newnifti 5.0.0, fsl-armawrap 0.7.0 (Armadillo
14.2.1), fsl-utils 2412.6, fsl-znzlib 2511.3, OpenBLAS 0.3.33). **(FSL 6.0.7.23)** The black-box
runs for it are in `<workspace>/oracles/fsl-6.0.7.23/mcflirt/` (the same runs, made by
`scripts/run_mcflirt_oracle.py --fsl-version 6.0.7.23`): 127 of 137 give 6.0.7.17's outputs byte
for byte; the 10 others change only as §9.3, §13 and the I/O lines below say. FSL 6.0.7.17's
packages:

| Package | Version (conda build) |
|---|---|
| fsl-mcflirt | 2111.0 (hb6de94e_6) |
| fsl-newimage | 2203.12 (h489b204_0) |
| fsl-miscmaths | 2203.2 (hb6de94e_5) |
| fsl-flirt | 2111.2 (hb6de94e_3) |
| fsl-newnifti | 4.1.0 (hdef71a4_5) |
| fsl-armawrap | 0.6.0 (hdef71a4_5) |
| fsl-utils | 2203.5 (hb6de94e_0) |
| fsl-znzlib | 2111.0 (hdef71a4_8) |

mcflirt calls itself "McFLIRT v 2.0" (with `-report`).

**Defects (D13).** Some behaviours below are defects that larmorx corrects by default and
reproduces with `upstream_defects="reproduce"` (`LARMORX_UPSTREAM_DEFECTS=reproduce`;
`docs/api/upstream-defects.md`): FSL-01 (the last volume's start, §5.4), FSL-03 (the in-plane
mode's padding, §12), FSL-05 to FSL-07 (the command line, §11), FSL-08 (`-stats`, §10.5),
FSL-09 (output types and conversion, §9.4, §10.1), FSL-10 (units, §10.1), FSL-13 (the `.mat`
directory, §10.2) and FSL-14 (NaN voxels, §12). FSL-02 (the count `N`, §6.4) is corrected only
on request (`normcorr_count="weights"`) until it is decided. This spec describes what mcflirt
does; the reproduced mode follows it. (spec writer, 2026-10-30) Refused by default, mcflirt's
behaviour with `reproduce` (rule B, the replica's decisions): FSL-20 (an uncreatable `.mat`
directory ends with status 0) and FSL-65 (an unopenable `.par` or `.rms` file), §10.6; FSL-21
(fewer than 8 bins with a histogram cost) and FSL-66 (negative `-bins`), §12.1; each section has
the exact messages, statuses and files of both modes, and §14.1 the cases.

**Importance tiers** used below: **tier 1** is what fMRIPrep runs (§2); **tier 2** is what
other pipelines commonly use (`-plots`, `-rmsrel`/`-rmsabs`, `-refvol`, `-meanvol`,
`-stages 4`, `-sinc_final`, `-spline_final`, `-dof`, `-stats`); **tier 3** is the rest.

---

## 1. Purpose

mcflirt registers every volume of a 4D series to one reference volume with a rigid-body
(6-parameter) transform, then resamples the series with those transforms. It returns the
corrected series and, on request, the transforms (`-mats`), the six motion parameters per
volume (`-plots`) and RMS displacements (`-rmsrel`, `-rmsabs`).

The registration is a fixed schedule of local searches (§5): first on a reference subsampled
to 8 mm, then to 4 mm, then 4 mm with a tighter tolerance. Each volume's search starts from
the previous volume's estimate (in the first stage) or from its own estimate of the previous
stage. The cost is a normalised correlation (§6), minimised one parameter at a time (§7).

## 2. How fMRIPrep uses it (tier 1)

`fmriprep/workflows/bold/hmc.py` runs nipype's `fsl.MCFLIRT(save_mats=True)` with
`ref_file` = the HMC reference (`hmc_boldref`, a robust average of the first volumes on the
BOLD grid). nipype adds nothing else; the command is exactly **(observed)**:

    mcflirt -in <bold.nii.gz> -out <cwd>/<bold stem>_mcf.nii.gz -reffile <boldref.nii.gz> -mats

with `FSLOUTPUTTYPE=NIFTI_GZ` (nipype's default). The input is the whole BOLD series
(dummy scans included).

- **Only the matrices are used:** `<out>.mat/MAT_0000 ... MAT_<N-1>` (§10.2). The corrected
  series is written but never read by fMRIPrep, and no `.par` or `.rms` file is requested.
  An implementation serving fMRIPrep may skip the final resampling (§9).
- **Conversion to ITK** (niworkflows `MCFLIRT2ITK`): each matrix is loaded with nitransforms'
  FSL reader, with the HMC reference as both "reference" and "moving" image, converted to a
  RAS world transform (§4.3) and written as one ITK transform per volume
  (`ITKLinearTransformArray`, LPS) in `mat2itk.txt`.
- **Confounds** (`fmriprep/interfaces/confounds.py`) are recomputed from those ITK transforms,
  not from mcflirt's own `.par`/`.rms`:
  - `FSLMotionParams`: back to FSL matrices (nitransforms), rotations from
    `scipy.Rotation.from_matrix(Rᵀ).as_euler("XYZ")` and translations
    `t − Rᵀc + c` with `c` = `zooms × scipy.ndimage.center_of_mass(boldref)` (note: no
    minimum subtracted, unlike mcflirt's own centre, §10.3);
  - `FSLRMSDeviation`: the RMS deviation of §10.4 between consecutive volumes, centre
    `0.5 (n − 1) × zooms`, radius 80 mm;
  - `FramewiseDisplacement`: Power's FD from those parameters, radius 50 mm.

  So everything fMRIPrep reports about motion depends only on the matrices, as written to
  text (6 decimals, §10.2).

## 3. Inputs and parameters

| Parameter | Option | Default | Tier |
|---|---|---|---|
| series | `-in` (or a bare argument) | required | 1 |
| output name | `-out`, `-o` | `<input without extension>_mcf` | 1 |
| reference image | `-reffile`, `-r` | none: a volume of the series | 1 |
| save matrices | `-mats` | off | 1 |
| reference volume index | `-refvol` | `N / 2` (integer division; 0-based) | 2 |
| register to the mean | `-meanvol` | off | 2 |
| number of stages | `-stages` | 3 | 2 |
| final interpolation | `-sinc_final`, `-spline_final`, `-nn_final` | trilinear | 2 |
| degrees of freedom | `-dof` | 6 | 2 |
| cost function | `-cost` | `normcorr` | 2 |
| motion parameters | `-plots` | off | 2 |
| RMS displacement | `-rmsrel`, `-rmsabs` | off | 2 |
| statistics images | `-stats` | off | 2 |
| edge-weighting width | `-smooth` | 1.0 mm | 3 |
| rotation tolerance divisor | `-rotation` | 1 | 3 |
| histogram bins | `-bins` | 256 | 3 |
| initial matrix | `-init` | none | 3 |
| in-plane mode | `-2d`, `-fov` | off; 20 mm | 3 |
| gradient images | `-gdt` | off | 3 |
| no previous-volume initialisation | `-fudge` | off | 3 |
| `-scaling`, `-hist`, `-v`, `-verbose`, `-report` | | see §11 | 3 |

The output format comes from the environment variable `FSLOUTPUTTYPE` (§10.1); mcflirt
fails without it.

## 4. Coordinates and conventions

### 4.1 Reading images

Images are NIfTI-1, NIfTI-2 or Analyze, optionally gzipped, any byte order. Everything is
converted to float32 on reading:
- stored value `v` → `v·slope + inter` in float32 when the header's `scl_slope` and
  `scl_inter` ask for scaling; a slope of (nearly) 0 means no scaling. Float32 data with slope
  1 and intercept 0 are kept as stored; NaN and infinities are kept (§12);
- `pixdim[1..3]` are the voxel sizes, as absolute values, with 0 replaced by 1. The voxel
  sizes come from `pixdim`, **not** from the lengths of the affine's columns;
- **left–right order.** If the image's affine has a positive determinant (the sform's if
  `sform_code > 0`, else the qform's if `qform_code > 0`), FSL calls it "neurological" and
  reverses the x axis of the data in memory (and adjusts both affines), so that every image
  is processed in "radiological" order. Images with negative determinant, with no affine, or
  whose sform and qform determinants disagree in sign are not flipped. Output images are
  flipped back, so the files keep their original storage order **(observed:** a series stored
  RAS with x-reversed data gives bit-identical matrices to the same images stored LAS, and so
  does a reference stored either way**)**;
- a 3D series is a series of one volume **(observed)**;
- a 4D `-reffile` is truncated to its first volume, with a warning on stderr ("An input
  intended to be a single 3D volume has multiple timepoints...") **(observed)**.

- **Formats and names (FSL 6.0.7.23).** A file is read by its content, whatever its name:
  gzip (first bytes `1f 8b`), zstd (`28 b5 2f fd`), bzip2 (`42 5a 68`), otherwise uncompressed; a
  file shorter than four bytes by its name's suffix (`.gz`, `.zst`, `.bz2`). A name without an
  extension is completed by trying, in this order, `.nii.gz`, `.nii`, `.hdr`, `.hdr.gz`,
  `.nii.zst`, `.hdr.zst`, `.nii.bz2`, `.hdr.bz2` (more than one match is an error, `Multiple
  possible filenames detected for basename: <name>`); `.img`, `.img.gz`, `.img.zst`, `.img.bz2`
  name the pair of the matching `.hdr` name. The data file of a pair is read in whatever format
  its own bytes say. **(observed:** a bzip2 file named `.nii` and a zstd file named `.nii.zst`
  read alike**)** FSL 6.0.7.17 read gzip and uncompressed files only.
- **Names holding `.img` (FSL 6.0.7.23, FSL-60).** FSL 6.0.7.23 opens every image under its name
  with the last occurrence of `.img` anywhere in it (a directory, or the file's own stem such as
  `sub.img2.nii.gz`) replaced by `.hdr`: it reads or writes another file, or fails with `Error:
  cant open file <the renamed name>` **(observed:** `-out a.img.d/out` writes
  `a.hdr.d/out.nii.gz` when that directory exists; `-in a.img.d/in.nii.gz` reads
  `a.hdr.d/in.nii.gz`; a pair written into `x.hdr.y.img.d/` aborts on `x.hdr.y.hdr.d/o5.hdr`**)**.
  larmorx opens the name given (default); `reproduce` renames as FSL 6.0.7.23 does, and both
  name FSL-60. FSL 6.0.7.17 opened the name given.

Obliquity is irrelevant: the estimation works in voxel coordinates. **(observed:** the same
array with an oblique affine gives bit-identical matrices.**)**

### 4.2 FSL millimetre coordinates and the `.mat` matrices

All transforms live in **FSL-mm coordinates**: for a voxel index `(i, j, k)` of the in-memory
(radiological) array, the FSL-mm position is `(i·dx, j·dy, k·dz)`. In terms of the file's
storage, for an image with a positive determinant `i_mem = nx − 1 − i_file`. Writing
`D = diag(dx, dy, dz, 1)` and `F` = the x flip (`x ↦ (nx − 1) − x` on the index, identity for
negative-determinant images), file voxel `v` has FSL-mm position `D·F·v`. The origin of the
NIfTI affine plays no part.

A matrix `M` written by mcflirt for volume `t` maps **FSL-mm coordinates of volume `t`** to
**FSL-mm coordinates of the reference**: the corrected volume is
`out(y) = in_t(M⁻¹ y)` for every reference position `y`. For the reference volume itself
(no `-reffile`), `M` is exactly the identity **(observed)**.

### 4.3 Relation to world (RAS) and ITK transforms

With `A_ref`, `A_in` the voxel-to-world affines (RAS mm) of the reference and of the input
series, and `P = D·F` built for each image as above:

- world transform moving a point of volume `t` onto the reference (the direction mcflirt's
  matrix points): `W = A_ref · P_ref⁻¹ · M · P_in · A_in⁻¹`;
- the transform ITK/ANTs/nitransforms store maps reference points to moving points:
  `T_ras = W⁻¹ = A_in · P_in⁻¹ · M⁻¹ · P_ref · A_ref⁻¹`, written in LPS as
  `L·T_ras·L` with `L = diag(−1, −1, 1, 1)`.

nitransforms builds `P` from the affine's column lengths and the affine's determinant, while
FSL uses `pixdim` and the sform/qform rule of §4.1. They agree whenever the header is
consistent.

### 4.4 Rigid parameters

The six parameters are `(rx, ry, rz, tx, ty, tz)`: three angles in **radians** and three
translations in **mm**. With a centre `c` (an FSL-mm point), they mean

    M(x) = R (x − c) + c + t,     R = Rx(rx) · Ry(ry) · Rz(rz),

    Rx(a) = [1 0 0; 0 cos a  sin a; 0 −sin a  cos a]
    Ry(b) = [cos b 0 −sin b; 0 1 0; sin b 0 cos b]
    Rz(g) = [cos g  sin g 0; −sin g  cos g 0; 0 0 1]

(rows separated by `;`). These are rotations by `−a`, `−b`, `−g` in the usual right-handed
sense. So `M = [R | c − R·c + t]`. **(observed:** decomposing the written matrices with this
convention reproduces the `.par` files to their printed precision.**)**

**Decomposition** of a rigid `M = [R | m]` about `c`:

    cy = sqrt(R11² + R12²)
    if cy ≥ 1e−4:  rx = atan2(R23/cy, R33/cy), ry = atan2(−R13, cy), rz = atan2(R12/cy, R11/cy)
    else:          rx = atan2(−R32, R22),      ry = atan2(−R13, 0),  rz = 0
    t = R·c + m − c

(1-based indices; `cy` taken non-negative). FSL computes these from float32 intermediates
(the cosines/sines and `cy`), so angles carry about 7 significant digits.

For `-dof` 7–12 the same matrix also carries scales and skews: `M` maps through
`R · K · S` (skew `K` upper unit-triangular with entries `k_xy, k_xz, k_yz`; scales `S`), each
factor kept about the centre `c`; with 7 dof one scale is used for all three axes **(observed:**
`-dof 7` gives three equal singular values**)**. Tier 3.

## 5. The schedule

### 5.1 The reference

- **`-reffile`:** that image (first volume if 4D). Every volume of the series is registered,
  in index order `0 … N−1`.
- **otherwise:** volume `r = N / 2` (integer division; `-refvol r` overrides; out-of-range
  values abort, §11). Volume `r` is not registered (its matrix is the identity). The others
  are visited forward `r+1, …, N−1`, then backward `r−1, …, 0` **(observed:** `-report`
  prints the visiting order**)**.
- **`-meanvol`:** two passes, §8.

### 5.2 Stages

| Stage | Reference grid (§5.3) | Tolerance factor `f` | Cost | Each volume starts from |
|---|---|---|---|---|
| 1 | 8 mm isotropic | 0.8 | normcorr, trilinear (§6) | identity, or the previous volume's stage-1 result (§5.4) |
| 2 | 4 mm isotropic | 0.8 | same | its own stage-1 result |
| 3 | the same 4 mm grid | 0.1 | same | its own stage-2 result |
| 4 (`-stages 4`) | the same 4 mm grid | 0.1 | normcorr with sinc interpolation (§6.5); other costs unchanged | its own stage-3 result |

`-stages 1` or `2` stop after that stage; stage 4 runs only with `-stages 4` (any value ≥ 4)
**(observed:** `-stages 4` changes the matrices, `-sinc_final` alone does not**)**. Each stage
visits all volumes (in the order of §5.1) before the next stage starts.

The parameter tolerances (the step scale of the line searches, §7) are

| Parameter | Base tolerance |
|---|---|
| rx, ry, rz | 0.005 rad / `rotation` (option `-rotation`, default 1) |
| tx, ty, tz | 0.2 mm |
| scales | 0.002 |
| skews | 0.001 |

multiplied by the stage's factor `f` (stored in float32: 0.8 and 0.1). So stage 1 uses
0.004 rad and 0.16 mm, stage 3 0.0005 rad and 0.02 mm **(observed** in the in-plane mode's
printout, and halved by `-rotation 2`**)**. Histogram costs use `256 / 8 = 32` bins in stage
1 and `256 / 4 = 64` bins after (`-bins` changes the 256; integer division).

### 5.3 The reference grids

The reference is resampled to an isotropic grid of spacing `s` (8 or 4 mm) **without any
smoothing**:
- step in reference voxels per axis: `step = s / d` (float32; `d` = that axis's voxel size);
- grid size per axis: `max(1, ⌊n / step⌋)`, with `n / step` computed in float32 (so for
  example 64 voxels of 3.125 mm give 25 at 8 mm and 50 at 4 mm);
- grid point `q` (0-based) lies at reference voxel position `q·step`, computed by adding
  `step` repeatedly in float32 from 0, independently per axis; the first grid point is
  voxel `(0, 0, 0)` of the reference, not the centre of the field of view;
- value: trilinear interpolation (§6.2) of the reference at that position, where neighbours
  outside the grid count as 0. (When a voxel is larger than `s`, the last grid point can
  fall up to one step beyond the last voxel and fades toward 0.);
- its voxel size is `s` in every axis, so grid point `q` has FSL-mm position `s·q`: the
  same frame as the reference's own FSL-mm coordinates (up to the float32 rounding of the
  accumulated steps).

The **test volume is never resampled or smoothed**: the cost compares the coarse reference
grid with the full-resolution volume, interpolated at the mapped positions (§6). This is the
only way the "8 mm" and "4 mm" stages are coarse.

### 5.4 Starting points and the previous-volume initialisation

- Every volume's stage-1 start is the identity, except that in stage 1 (and only there), once
  volume `i` is done, its result becomes the start of the next volume in the visiting order
  **unless that next volume is the last of the series (index `N−1`)**, which starts from the
  identity. So:
  - with `-reffile`: volume 0 starts from the identity, volumes 1 … N−2 from their
    predecessor, volume N−1 from the identity;
  - without: volumes `r+1` and `N−1` start from the identity, `r+2 … N−2` from their
    predecessor; volume `r−1` starts from the identity, `r−2 … 0` from their successor.

  **(observed:** six identical copies of one displaced volume registered with `-reffile` give
  bit-identical matrices for volumes 0 and 5 and different ones for 1–4; with `-fudge` all six
  are identical.**)**
- `-fudge` disables this initialisation: every volume starts stage 1 from the identity.
- Stages 2–4 start each volume from its own result of the previous stage.

The starting matrix enters the search as parameters: it is decomposed (§4.4) about the
current volume's centre `c` (§6.1); the search then moves those parameters, and each cost
evaluation recomposes the matrix from them. Only the first `dof` parameters are composed (6
for rigid), so a start matrix is projected onto the rigid family through its float32 Euler
angles. A start whose decomposition is not finite (the NaN result of a previous volume, §12)
is replaced by the identity.

## 6. The cost function

The cost compares the **reference grid** `G` (the 8 or 4 mm grid of §5.3, values `g`) with
the **test volume** `V` (the current volume at full resolution, values `v`, voxel sizes
`d = (dx, dy, dz)`, size `(nx, ny, nz)`). For a candidate matrix `M` (test FSL-mm → reference
FSL-mm):

### 6.1 Sampling

- The centre of rotation `c` is the **intensity-weighted centre of the test volume**, in
  FSL-mm: `c = d ⊙ Σ (v − v_min)·idx / Σ (v − v_min)`, with `v_min` the volume's minimum, sums
  over all voxels in double precision. **(observed** for the `.par` centre, which follows the
  same rule for the reference, §10.3: subtracting the minimum matters when the background is
  not 0.**)** If the denominator is below 1e−5 it is replaced by 1.
- The voxel-to-voxel map is `B = Dv⁻¹ · M⁻¹ · Dg` (`Dv = diag(d, 1)`, `Dg = diag(s, s, s, 1)`),
  computed in double, then each entry rounded to float32: `p = B·(x, y, z, 1)` is the test
  voxel position of reference grid point `(x, y, z)`.
- Grid points are visited slice by slice (z outer), row by row (y), then along x. For a row,
  the position at `x = 0` is `y·b12 + z·b13 + b14` (per component, float32, in that order);
  the first sample of the row adds `x_first·b11` to it; each next sample adds `b11` (and
  `b21`, `b31`) again, in float32.
- **A grid point contributes only if its test position lies inside the test volume**:
  `0 ≤ p_k ≤ n_k − 1.0001` for every axis `k` (bound in float32). Points outside contribute
  nothing; there is no padding. The admissible x range of each row is found analytically
  from the row's start and step, intersected with the grid, rounded inward to whole grid
  points, then re-checked by stepping through it with the same float32 additions: leading
  points that fail are dropped, and the row ends at the first later point that fails. The
  two end points of the range must also have `⌊p_k⌋ + 1 ≤ n_k − 1`.
- The reference grid is not masked: background points count.

### 6.2 Trilinear interpolation

At `p = (px, py, pz)`, `i = ⌊p⌋` (in the cost `p ≥ 0`, so truncation is the same), `f = p − i` in float32, with the eight
neighbours `v_abc` (`a, b, c` = offsets in x, y, z):

    e00 = (v100 − v000)·fx + v000     e01 = (v101 − v001)·fx + v001
    e10 = (v110 − v010)·fx + v010     e11 = (v111 − v011)·fx + v011
    h0  = (e10 − e00)·fy + e00        h1  = (e11 − e01)·fy + e01
    value = (h1 − h0)·fz + h0

in float32, in this order (x first, then y, then z). The same formula is used for the
reference grids (§5.3) and the final resampling (§9).

### 6.3 Edge weighting ("apodization")

Each contributing point gets a weight that falls linearly to 0 near the test volume's
boundary (Jenkinson et al. 2002's cost-function apodization). Per axis, with
`σ_k = smooth / d_k` (in voxels; `smooth` = `-smooth`, default 1 mm) and `b_k = n_k − 1.0001`:

    w_k = p_k / σ_k          if p_k < σ_k
    w_k = (b_k − p_k) / σ_k  else if b_k − p_k < σ_k
    w_k = 1                  otherwise

`w = w_x · w_y · w_z` (float32, in that order), clamped below at 0. Only one side per axis
can apply.

### 6.4 Normalised correlation as mcflirt computes it

With `x` = reference grid value, `y` = interpolated test value, and `w` the weight, form the
float32 sums

    Sx = Σ w·x,   Sy = Σ w·y,   Sxx = Σ (w·x)·x,   Syy = Σ (w·y)·y,   Sxy = Σ (w·x)·y

each accumulated in three levels: over a row (in x order), the row totals over a slice (in
y order), the slice totals over the grid (in z order); each level is a float32 running sum
started at 0.

**The count is not the sum of the weights.** It is a float32 running sum of running sums:
let `w_r` be the total weight of row `r` (rows numbered in visiting order over the whole
grid, every row of every slice, including rows with no contributing point), and

    C_r = w_1 + … + w_r            (running total of weights)
    A_r = C_1 + … + C_r            (running total of C)
    N   = Σ over slices z of A_{last row of slice z}

all as float32 running sums in visiting order. `N` is many orders of magnitude larger than
the number of contributing points (about `10^4` times larger on a 4 mm grid of a typical
BOLD volume).

Then, if `N > 2`:

    cov  = Sxy / (N − 1) − (Sx·Sy) / (N·N)
    varx = Sxx / (N − 1) − (Sx·Sx) / (N·N)
    vary = Syy / (N − 1) − (Sy·Sy) / (N·N)
    r    = cov / √varx / √vary    if varx > 0 and vary > 0, else 0

(the divisions by `N − 1` in double, the products and the second divisions in float32,
`cov`, `varx`, `vary` stored as float32), else `r = 0`. The cost is `1 − |r|` (an
anticorrelation scores as well as a correlation).

Because `N` is so large, the subtracted means are tiny and **`r` is in effect the uncentred
correlation** `Σwxy / √(Σwx² · Σwy²)` with a small correction, not Pearson's correlation.
**(observed:** a series that is a linear ramp along x (`1000 + 6x`) shifted along x is
realigned to within 0.14 mm by default, which a centred correlation cannot do, since shifting
a ramp only changes its mean; with `-smooth 0`, which uses the centred formula below, the
shifts are not found.**)** Reproduce `N` exactly: the correction is about `10⁻⁴` relative,
comparable to the cost differences the line search decides on near the optimum.

**`-smooth 0`** turns the weighting off (every contributing point has weight 1) and uses the
true count `n` of contributing points: `r = (Sxy/(n−1) − Sx·Sy/n²) / √(…) / √(…)`, all float32,
which is Pearson's correlation (tier 3).

### 6.5 Stage 4: sinc interpolation in the cost

Identical to §6.1–6.4 except that `y` is a windowed-sinc interpolation of the test volume:
- kernel `k(u) = sinc(u)·(0.5 + 0.5·cos(πu/3))` for `|u| ≤ 3`, else 0, with
  `sinc(u) = sin(πu)/(πu)` (and `1 − |u|` for `|u| < 1e−7`), tabulated at 201 points
  `u = (n − 100)/100·3`, `n = 0 … 200` (float32), and read by linear interpolation in the
  table (`n = ⌊u/3·100 + 100⌋`, 0 outside `0 ≤ n < 200`);
- the value at `p` is `Σ v·k(px − i)·k(py − j)·k(pz − l) / Σ k(…)k(…)k(…)` over the voxels
  `i = ⌊px⌋−3 … ⌊px⌋+3` (same in y, z) **that lie inside the volume**; float32 sums over z,
  then y, then x; the product of the three factors in the order x·y·z. If the weight sum is
  `≤ 1e−9` in magnitude the value is the volume's background value (§9.2).

## 7. The optimiser

### 7.1 Parameters and sweep

The search variable is the 12-vector `(rx, ry, rz, tx, ty, tz, sx, sy, sz, kxy, kxz, kyz)` of
§4.4, held in double, from the decomposition of the start matrix about `c`. Only the first
`dof` entries (6 by default) are searched, and the cost of a parameter vector is the cost of
the matrix composed from its first `max(dof, 6)` entries about `c`.

For each volume and stage, mcflirt performs **one sweep** of line searches: along
`rx, ry, rz, tx, ty, tz` in this order (then the scales and skews if `dof > 6`), each
starting where the previous one ended. There is no second sweep and no convergence test,
and Powell's direction-set update is not used. The cost at the start point is evaluated once
(before the first line search); each later line search reuses the best cost of the previous
one as its starting value (a starting value of exactly 0.0 is re-evaluated).

### 7.2 The line search

A line search along parameter `k` from the current point `p0`, with tolerance `u` (the
parameter's tolerance of §5.2, as float32), evaluates `φ(a) = cost(p0 + a·e_k)`. All step
positions `a` and cost values are float32. Throughout, `(a1, y1)`, `(am, ym)`, `(a2, y2)` is a
triple with `am` the best point so far.

**Bracketing.**
1. `am = 0` with `ym = φ(0)` (the starting value); `a1 = 10·u`, `y1 = φ(a1)`.
2. If `y1 < ym`, swap the two points (so `am = 10u`, `a1 = 0`).
3. `a2 = am + 1.6·(am − a1)`, `y2 = φ(a2)`. Let `δ = +1` if `am ≥ a1`, else `−1`.
4. While `ym > y2`:
   - `limit = am + 3.2·(a2 − am)`;
   - candidate `a` = the vertex of the parabola through the three points (§7.3); if there is
     none, or it lies behind `a1` (`(a − a1)·δ < 0`), or beyond `limit` (`(a − limit)·δ > 0`),
     use `a = am + 1.6·(a2 − a1)` instead; evaluate `y = φ(a)`;
   - if `a` lies strictly between `a1` and `am`: if `y < ym`, the bracket is
     `(a1, a, am)` (set `a2 = am`, `am = a`) and the loop ends; else `a1 = a`;
   - otherwise: if `y > ym`, the bracket is `(a1, am, a)` (set `a2 = a`) and the loop ends;
     else if `a` is nearer to `am` than `a2` is (`(a − a2)·δ < 0`), set `a1 = am`, `am = a`;
     else shift: `a1 = am`, `am = a2`, `a2 = a`.

**Refinement.** With `dmin = 0.1·u`, repeat at most 100 times while `|a2 − a1| / u > 1`:
1. candidate `a` = the parabola vertex through the triple if it exists and lies within
   `[min(a1, a2), max(a1, a2)]`; otherwise the golden point on the longer side:
   `a = 0.3819660·a_far + 0.6180340·am` with `a_far` the end (`a1` or `a2`) farther from `am`
   (ties go to `a1`);
2. let `ε = +1` if `a2 ≥ a1`, else `−1`; then, in this order:
   - if `|a − a1| < dmin`: `a = a1 + ε·dmin`;
   - if `|a − a2| < dmin`: `a = a2 − ε·dmin`;
   - if `|a − am| < dmin`: `a` = the golden point (computed as in step 1);
   - if `|am − a1| < 0.4·u`: `a = am + ε·0.5·u`;
   - if `|am − a2| < 0.4·u`: `a = am − ε·0.5·u`;
3. `y = φ(a)`; if `a` lies on the `a2` side of `am` (`(a − am)(a2 − am) > 0`), swap the
   roles of `(a1, y1)` and `(a2, y2)`;
4. if `y < ym`: `(a2, y2) = (am, ym)`, `(am, ym) = (a, y)`; else `(a1, y1) = (a, y)`.

The parameter moves by the final `am` (added to the double parameter vector) and the line
search returns `ym`.

### 7.3 Parabola vertex

Through `(a1, y1), (am, ym), (a2, y2)`, in float32:

    P = (am − a2)(ym − y1) − (am − a1)(ym − y2)
    Q = −(am² − a2²)(ym − y1) + (am² − a1²)(ym − y2)
    H = (am − a2)(a2 − a1)(a1 − am)

If `|H| > 1e−15` and `P/H < 0` the parabola opens downward: no vertex. Else if
`|P| > 1e−15` the vertex is `−Q / (2P)`; else none.

### 7.4 Consequences

- **Quantised results.** Trial points closer than half a tolerance to the best point are
  never evaluated, so a parameter whose optimum lies within a fraction of a tolerance of its
  starting value usually stays exactly where it started (in stage 3 the tolerance is 0.0005
  rad = 0.029° and 0.02 mm). Very still subjects show it **(observed:** in ds000005 run 1,
  fMRIPrep's command, whose motion stays within 0.12° and 0.12 mm, 100 of the 240 matrices
  have rx = ry = 0 exactly and 63 have tz = 0 exactly; none of the eight other real runs, with
  more motion, has such zeros**)**. A clean-room optimiser that converges further will differ
  from mcflirt by up to about a tolerance per parameter; this is the scale of the validation
  thresholds (§14).
- **Stage 3 works near the float32 noise of the cost.** Near the optimum, a 0.02 mm or
  0.0005 rad step changes the cost by roughly 10⁻⁶–10⁻⁷, while the float32 sums of §6.4
  carry relative rounding errors of about 10⁻⁷. Which of two nearby points wins can depend
  on the summation order, so an implementation that sums differently will take different
  steps in the last stage even with an otherwise identical algorithm.
- The estimate depends on the search path: start point, parameter order, centre `c`, the
  first trial at `+10u`. Reproduce them to stay close.

## 8. `-meanvol` (tier 2)

1. Stages 1–3 as above (with the reference of §5.1). **(observed:** with `-reffile`, the
   first pass registers to that file and the mean is formed on its grid: registering the
   first pass to volume `N/2` instead puts the mean of ds000005 off by up to 33 intensity
   units and the matrices by 0.17 mm.**)**
2. Every volume is resampled with its stage-3 matrix (trilinear, with the edge and
   background rules of §9) and the mean is formed: float32 sum in volume order, divided by
   `N` (float32). It is written as `<out>_mean_reg` (§10.1).
3. Stages 1–3 again with the mean as reference: all matrices reset to the identity, every
   volume registered in order `0 … N−1` (as with `-reffile`), with stage-1 initialisation.
4. Stage 4 if requested, then the final resampling.

The mean is also the reference for the `.par` centre and the RMS centre (§10.3–10.4).

## 9. Final resampling

### 9.1 Geometry

Each volume `t` is resampled with `M_t` (times the `-init` matrix, §11) onto the output grid:
the input's own grid, or with `-reffile` the reference's grid. For an output voxel `o`
(in-memory order), the source position is `Dsrc⁻¹ · (M_t·M_init)⁻¹ · Dout · o` (double,
rounded to float32 per entry). Output voxels are visited with z outermost, then x, then **y
innermost**: positions advance by adding the y column of that matrix in float32 from the
row start `x·b11 + z·b13 + b14`. (The cost function steps along x instead.)

With `-reffile`, the output grid must have the same number of voxels as the input volumes;
otherwise mcflirt aborts after the whole estimation (§12). With the same number but a
different shape or geometry, the data are written on the reference's grid under the input's
header (only the voxel data are copied into the series). fMRIPrep's reference is always on
the BOLD grid.

### 9.2 Values at the edges

- The input is treated as extended by one voxel on every side, the extension repeating the
  edge voxel (`index −1 → 0`, `index n → n − 1`, per axis); interpolation uses these values.
- Output voxels whose source position is outside `[−1, n]` on any axis are set to the
  **background value** of that input volume: sort the voxels of its outer shell two voxels
  thick (all voxels with an index within 2 of a face), take the element at index
  `⌊count / 10⌋` (the 10th percentile). **(observed:** on a series with a background near
  100, every voxel whose source falls outside the extended grid equals that value, 90.**)**
- NaN matrices (§12) put every output voxel of that volume at the background value
  **(observed)**.

### 9.3 Interpolation

| Option | Interpolation |
|---|---|
| default | trilinear (§6.2) |
| `-sinc_final` | windowed sinc: Blackman window `0.42 + 0.5·cos(πu/3) + 0.08·cos(2πu/3)`, `|u| ≤ 3`, times `sinc(u)`; tabulated at 1201 points over `[−3, 3]` and read by linear interpolation; 7×7×7 voxels, only voxels inside the grid, normalised by the weight sum; weight sum `≤ 1e−9` gives the extended-grid value at `⌊p⌋` |
| `-spline_final` | cubic B-spline; coefficients by the standard recursive prefilter (pole `√3 − 2`), the causal sweep initialised from a mirror-symmetric extension truncated at precision 1e−8; coefficients outside the grid repeat the edge coefficient; positions with `⌊p⌋ < −1` or `⌊p⌋ ≥ n` give the background value |
| `-nn_final` | nearest neighbour, rounding half away from zero |

The final interpolation does not change the matrices **(observed)**. Only one option
applies; the precedence is sinc, then nearest neighbour, then spline.

**(FSL 6.0.7.23, FSL-59)** FSL 6.0.7.23's mcflirt applies `-sinc_final` and `-nn_final` with
**trilinear** interpolation: its output with either option is byte for byte its default output,
`.par` and `.rms` included **(observed:** a synthetic series and ds000005 run 1, every output
identical to the default run's; `-spline_final` is still the spline**)**. The `.par` and `.rms`
files follow from the resampled reference volume (§10.3), so they change with it. larmorx
resamples with the interpolation asked for (default, FSL 6.0.7.17's result) and with
`reproduce` trilinearly, as FSL 6.0.7.23 does; both name FSL-59 where the two series differ.
**(FSL 6.0.7.23)** The spline's coefficients outside the grid: unchanged for mcflirt's final
resampling (it extends the edge coefficient); FSL's spline code now treats coefficients outside a
zero-padded grid as 0, which mcflirt never uses.

### 9.4 Output values

The resampled series is float32. It is written in the input's storage type (§10.1),
converted by **truncation toward zero** (no rounding, no clamping) **(observed:** the int16
output equals the truncated float32 output of the same data, and is not the rounded one**)**.
**(observed** by the clean-room implementer: values beyond the type's range wrap around, as
x86-64's conversion through a 32-bit integer does. A uint8 series with a sharp 0/255 edge,
resampled half a voxel off by `-init` with `-sinc_final` and `-spline_final`, gives 272.53 →
16, −17.53 → 239, −1.84 → 255, 256.84 → 0 and 280.62 → 24, where the float32 run of the same
data has the values on the left.**)**

## 10. Outputs

### 10.1 The corrected series

- **Name.** `<out>` with any image extension removed, plus the extension chosen by
  `FSLOUTPUTTYPE`: `NIFTI_GZ` → `.nii.gz`, `NIFTI` → `.nii`, `NIFTI2_GZ` → NIfTI-2 `.nii.gz`,
  and the pair types (`.hdr/.img`). Without `-out`, `<out>` = the input name without
  extension, plus `_mcf` **(observed)**. Unset `FSLOUTPUTTYPE` makes mcflirt fail (exit 1)
  after the estimation, after writing the `.mat` files **(observed)**. **(FSL 6.0.7.23)** The
  accepted values are `NIFTI`, `NIFTI_PAIR`, `NIFTI2`, `NIFTI2_PAIR`, `NIFTI_GZ`, `NIFTI_PAIR_GZ`, `NIFTI2_GZ`, `NIFTI2_PAIR_GZ`, `NIFTI_ZST`, `NIFTI_PAIR_ZST`, `NIFTI2_ZST`, `NIFTI2_PAIR_ZST`, `NIFTI_BZ2`, `NIFTI_PAIR_BZ2`, `NIFTI2_BZ2`, `NIFTI2_PAIR_BZ2`: the zstd types write `.nii.zst` (pairs
  `.hdr.zst`/`.img.zst`), the bzip2 types `.nii.bz2` (`.hdr.bz2`/`.img.bz2`); FSL 6.0.7.23 writes
  each piece of a zstd file (header, extensions, data) as a frame of its own, and bzip2 with
  blocks of 900 kB (only the decompressed bytes matter). An unknown value prints `ERROR::
  Unrecognised value (<value>) of environment variable FSLOUTPUTTYPE` and `Legal values are: NIFTI, NIFTI_PAIR, NIFTI2, NIFTI2_PAIR, NIFTI_GZ, NIFTI_PAIR_GZ, NIFTI2_GZ, NIFTI2_PAIR_GZ, NIFTI_ZST, NIFTI_PAIR_ZST, NIFTI2_ZST, NIFTI2_PAIR_ZST, NIFTI_BZ2, NIFTI_PAIR_BZ2, NIFTI2_BZ2, NIFTI2_PAIR_BZ2`
  (FSL 6.0.7.17 listed `NIFTI, NIFTI_PAIR, NIFTI_GZ, NIFTI_PAIR_GZ`) **(observed)**.
- **Type.**

  | Input | Output |
  |---|---|
  | uint8, int8 | uint8 (int8 values are not preserved) |
  | int16 | int16 |
  | int32, uint16 | int32 **(observed** for uint16**)** |
  | float32, uint32, int64, uint64 | float32 |
  | float64 | float64 **(observed)** |
  | any type with `scl_slope ≠ 1` or `scl_inter ≠ 0` (a slope of exactly 0 excepted; a NaN slope counts as ≠ 1) | float32 **(observed:** int16 with slope 0.5 and intercept 10**)**, except float64, which stays float64 |

- **Header** **(observed)**: dimensions, `pixdim` (including the TR), qform, sform and their
  codes as in the input (the stored affines are unchanged, whatever the orientation);
  `scl_slope = 1`, `scl_inter = 0`; `cal_min`/`cal_max` = the minimum and maximum of the
  float32 corrected series (before the type conversion); `descrip` = the FSL library's build
  string (`2203.12-dirty 2024-02-01T16:17:47+00:00` in FSL 6.0.7.17, `2601.0
  2025-12-08T15:20:57Z` in FSL 6.0.7.23 **(observed)**); intent fields and `aux_file` from
  the input; `dim_info`, `slice_code`, `slice_start`, `slice_end`, `slice_duration` set to 0;
  little-endian, also for big-endian input (ds000258). **(observed** by the clean-room
  implementer: a single corrected volume is written 3D (`dim[0] = 3`) with `pixdim[4]` kept;
  the qform quaternion is written back from FSL's own qform matrix, so its last float32 bits
  can change (up to ~1e−5 mm in the qform where its `w` is small, ds000258), and a stored
  quaternion whose `(b, c, d)` is longer than 1 comes back normalised (ds000122).**)**
- **`xyzt_units` is always mm and s**, while `pixdim[4]` is copied unchanged: a series
  whose TR was 2000 in milliseconds (ds003345, units 18) comes out with units 10 and a "TR"
  of 2000 s **(observed)**.

### 10.2 `-mats` (tier 1)

- Directory `<out>.mat`, where `<out>` is the `-out` string **exactly as given** (with
  `-out x_mcf.nii.gz` the directory is `x_mcf.nii.gz.mat`; with `-out plain` it is
  `plain.mat`) **(observed)**. If the directory exists, mcflirt tries `<out>.mat+`,
  `<out>.mat++`, … and writes into the first one it can create **(observed)**.
- One file per volume, `MAT_0000`, `MAT_0001`, … (`MAT_` plus the 0-based index, at least 4
  digits, zero-padded).
- Content **(observed)**: the 4 × 4 matrix of §4.2, one row per line, each number in fixed
  notation with **6 decimals**, each followed by one space, the line ending in `\n`:

      1.000000 0.000327 0.000724 -0.098268 \n

  Negative zero prints as `-0.000000`. The reference volume (no `-reffile`) gets the identity.
- `-rmsrel` and `-rmsabs` also create the directory; without `-mats` it then holds only the
  reference volume's identity matrix (nothing with `-reffile`). `-plots` alone creates no
  directory **(observed)**.

### 10.3 `-plots`: the `.par` file (tier 2)

`<out>.par` (again the raw `-out` string plus `.par`): one line per volume with the six
parameters `rx ry rz tx ty tz` (radians, mm) of §4.4, each followed by two spaces, then
`\n`; numbers printed like C++'s default stream format (6 significant digits, `%g`-like).
The centre is the **intensity-weighted centre of the reference**, as in §6.1 (minimum
subtracted), in the reference's FSL-mm, where the reference is the `-reffile` image, the
mean of `-meanvol`, or volume `r` of the corrected series. **(observed:** decomposing the
`.mat` files about this centre reproduces the `.par` within 1e−4, the rounding of the
6-decimal matrices; the centre without subtracting the minimum is wrong by up to 5 mm on a
series with a non-zero background.**)** The reference volume's line is `0  -0  0  0  0  0`
(signed zeros as printed by C++).

### 10.4 `-rmsrel`, `-rmsabs` (tier 2)

The RMS deviation between two affine matrices `M1`, `M2` over a sphere of radius `R = 80 mm`
centred at `x_c` (Jenkinson 1999):

    Δ = M1 · M2⁻¹ − I,   A = Δ[0:3, 0:3],   τ = Δ[0:3, 3] + A·x_c
    rms = sqrt( τ·τ + (R²/5)·trace(Aᵀ A) )

with `x_c = 0.5·(n − 1)·d` per axis, the centre of the reference's field of view (same
reference as §10.3). Files (`<out>` as given):
- `<out>_abs.rms`: `rms(I, M_t)` for every volume, one per line;
- `<out>_rel.rms`: `rms(M_{t−1}, M_t)` for `t = 1 … N−1` (N − 1 lines);
- `<out>_abs_mean.rms`, `<out>_rel_mean.rms`: their means (sum divided by N and N − 1).

Values printed with 6 significant digits. **(observed:** recomputed from the `.mat` files to
within 1e−4.**)** For rigid matrices the order of `M1, M2` does not matter.

### 10.5 `-stats` (tier 2)

`<out>_meanvol`, `<out>_variance`, `<out>_sigma` (the image extension of `FSLOUTPUTTYPE` is
appended to the raw `-out` string, e.g. `x_mcf.nii.gz_meanvol.nii.gz`) **(observed)**: the
temporal mean, variance (divided by `N − 1`) and standard deviation of the corrected float32
series, voxel by voxel, float32. **The first and last slices are left at 0.** Written as
float32. `<out>_mean_reg` of `-meanvol` is named the same way. (nipype expects
`x_mcf_mean_reg.nii.gz` with FSL ≥ 6 and so does not find these files.) **(observed** by the
clean-room implementer: these images are 3D and keep the input's `cal_min`/`cal_max`, which
are not recomputed; ds000005: 0 / 1353 as in the input, the synthetic series: 0 / 0.**)**

### 10.6 Outputs that cannot be written (FSL-20, FSL-65) (spec writer, 2026-10-30)

(spec writer, 2026-10-30) **The order of the writes.** After the estimation and the final
resampling, mcflirt writes, in this order: the `-stats` images (§10.5; `-meanvol`'s
`_mean_reg` image earlier still, §8); the two `refnum` lines on standard output (§11, `-report`);
then, when any of `-mats`, `-plots`, `-rmsrel`, `-rmsabs` is given: it opens `<out>.par` (with
`-plots`; an empty file appears at once), makes the `.mat` directory (§10.2; with `-mats`,
`-rmsrel` or `-rmsabs`), writes `<out>_rel.rms` and `<out>_rel_mean.rms`, then `<out>_abs.rms`
and `<out>_abs_mean.rms`, then, volume by volume from 0 to N − 1, the volume's `MAT_` file
followed by its `.par` line; last, ` Saving motion corrected time series... ` (with `-report`)
and the corrected series (§10.1). Every image write that fails (a directory missing or not
writable) ends the run with an uncaught exception, as for any image: `terminate called after
throwing an instance of 'NiftiIO::NiftiException'` and `  what():  Error: cant open file
<name>` on standard error, signal 6 (abort), nothing more written **(observed:** `-out
missing/o` alone: the series' name; `-out ro/o -mats`, `ro/` not writable: the same after the
message below**)**.

(spec writer, 2026-10-30) **The `.mat` directory (FSL-20).** Making `<out>.mat` (or the first
free `<out>.mat+`, `<out>.mat++`, ..., §10.2) ends one of three ways:
- it is made: the run goes on;
- permission denied (the parent directory is not writable): `Error! Could not create directory:
  <P><dir>. No write permission` on standard error, where `<dir>` is the directory's name as tried
  and `<P>` the `-in` string up to and including its last `/` (with no `/` in it, its first
  character: a misleading path, message only); the run goes on without any `MAT_` file (the
  `.rms` files are then not writable either, below), and the corrected series' write aborts as
  above **(observed:** `-out ro/o -mats`**)**;
- **any other failure** (FSL-20): a parent directory that does not exist (`-out missing/o`) or is
  a file (`-out afile/o`): `Error!: Unknown Log directory return value` (one line on standard
  error) and **exit status 0**, with nothing more written: no `MAT_` file, no `.rms` file, no
  `.par` line (with `-plots` its file could not be opened either: the same parent), no corrected
  series **(observed** with `-mats`, `-mats -plots`, `-rmsrel`, `-rmsabs -report` and
  `-mats -reffile`: status 0, the message, no file at all; with `-report` the progress lines and,
  without `-reffile`, the `refnum` lines come first**)**.

(spec writer, 2026-10-30) **The `.par` and `.rms` files (FSL-65).** They are opened as plain
text files:
- an `.rms` file that cannot be opened (its name is a directory, or its directory is not
  writable) is silently not written; everything else is written and the status is 0
  **(observed:** directories named `o_rel.rms` and `o_abs_mean.rms`: the other two `.rms` files,
  every `MAT_` file and the series written, status 0, no message**)**;
- a `.par` file that cannot be opened (`-plots`; its name is a directory, or its directory is
  missing or not writable): at volume 0's line mcflirt prints `error: unable to open output file!`
  on standard error and **exits with status 0**. Written by then: the `.mat` directory with
  `MAT_0000` only (with `-mats`; also when volume 0 is the reference, its identity), the `.rms`
  files; not written: every other `MAT_` file, every `.par` line, the corrected series
  **(observed:** a directory named `o.par`, with `-mats -plots -rmsrel`, `-plots` alone and
  `-mats -plots -refvol 0`**)**.

(spec writer, 2026-10-30) **What larmorx does** (the replica's decisions for FSL-20, rule B:
the run cannot honour its command and mcflirt reports success; the same rule for FSL-65):

| Case | Default (`fix`) | `reproduce` |
|---|---|---|
| FSL-20: the `.mat` directory cannot be made, for another reason than its permission | at the same point (after the estimation, before any output of this step): `Error: cannot create the directory <dir> for the matrices (FSL-20: mcflirt prints "Error!: Unknown Log directory return value" and exits with status 0)`, exit status **1**, nothing more written; a notice naming FSL-20 | mcflirt's message `Error!: Unknown Log directory return value`, exit status **0**, nothing more written; a notice naming FSL-20 |
| the `.mat` directory's permission denied | an error, exit 1, nothing more written (the message may name the real path) | mcflirt's message (its `<P>` prefix included), the run without matrices, then the series' write fails as an image write does: an error, exit 1 (a crash is never reproduced, D13; the replica prints mcflirt's two `terminate` lines first) |
| FSL-65: a `.par` file that cannot be opened | an error naming the file and FSL-65, exit 1 (before any output of this step: checked when the file is opened) | mcflirt's `error: unable to open output file!` after `MAT_0000` and the `.rms` files, exit status 0, nothing more written; a notice naming FSL-65 |
| FSL-65: an `.rms` file that cannot be opened | an error naming the file and FSL-65, exit 1 | that file not written, the rest as mcflirt; a notice naming FSL-65 |
| an image that cannot be written | an error naming the file, exit 1 | the same (the replica prints mcflirt's two `terminate` lines first) |

The default's refusal lines are `Error: <message>` on standard error; the notices are larmorx's
lines of `docs/api/upstream-defects.md` (`larmorx: upstream defect FSL-20 (mcflirt) corrected:
... [LARMORX_UPSTREAM_DEFECTS=reproduce gives mcflirt's result]`), after the run's other
messages. **(spec writer, 2026-10-30)** The replica does all of the FSL-20 rows today
(`larmorx-nc fsl mcflirt`, observed with the cases above); FSL-65 is new: the replica writes
everything it can and ignores an unopenable `.par` or `.rms` file in both modes, a follow-up of
the replica (the table is its target).

## 11. Command line (`larmorx mri hmc [options]`, accepting mcflirt's)

Arguments are processed left to right. Option names are matched exactly (case-sensitive,
whole word).

| Option | Behaviour |
|---|---|
| `-in f` | input series |
| a bare argument (not starting with `-`) | taken as the input, with a warning on stderr ("WARNING: change in option usage ...") |
| `-out f`, `-o f` | output name (§10) |
| `-reffile f`, `-r f` | reference image |
| `-refvol n` | reference index, read with C's `atoi` (`abc` → 0) |
| `-mats`, `-plots`, `-rmsrel`, `-rmsabs`, `-stats` | outputs (§10) |
| `-meanvol` | §8 |
| `-stages n` | `atoi`; ≥ 4 adds stage 4 |
| `-sinc_final`, `-spline_final`, `-nn_final` | final interpolation (§9.3) |
| `-dof n` | `atoi`, clamped to 6–12 with a message for values outside (tier 3) |
| `-cost c` | `mutualinfo`, `corratio`, `woods`, `normcorr`, `normmi`, `leastsquares`; anything else silently keeps `normcorr` **(observed)** (a message only if `-report` came earlier: `Unrecognised cost function type: X` and `Using the default (NormCorr)` on stderr **(observed)**) |
| `-bins n` | histogram bins (`atoi`); each stage uses `n / s` of them (integer division truncating toward zero, `s` the stage's scale: 8 in stage 1, 4 in stages 2–4), for every cost (the bins are prepared whatever the cost); fewer than 8 with a histogram cost and negative values: §12.1 (spec writer, 2026-10-30) |
| `-smooth x` | edge-weighting width in mm (`atof`); 0 switches the correlation to its centred, unweighted form (§6.4) |
| `-rotation x` | divides the rotation tolerances (`atof`) **(observed)** |
| `-scaling x` | accepted and ignored |
| `-fov n` | in-plane-mode threshold in mm, read with `atoi` (§12) |
| `-2d` | force the in-plane mode (§12) |
| `-init f` | a 4 × 4 matrix (FSL text format) applied **only in the final resampling**, before `M_t` (the source position uses `(M_t · M_init)⁻¹`); it does not affect the estimation or the written `.mat` files **(observed)** |
| `-gdt` | registers gradient-magnitude images of the test volumes (3 × 3 × 3 derivative masks) to the **unchanged** reference, and writes the reference as `grefvol_<out>`: with an absolute `-out` path that name is invalid and mcflirt aborts **(observed)** (tier 3; of little use) |
| `-fudge` | no previous-volume initialisation (§5.4) |
| `-report` | progress messages on stderr; also prints `refnum = r` and `Original_refvol = …` (the `-refvol` value as given, `-1` by default) on stdout at the end when there is no `-reffile`/`-meanvol` and at least one of `-mats`, `-plots`, `-rmsrel`, `-rmsabs` is given **(observed:** `-report -stages 1` alone prints nothing on stdout; adding any one of the four prints both lines**)** |
| `-v` | verbose level 5; `-verbose n` sets it (20 or more prints every evaluated matrix on stderr); `-hist` is accepted and ignored. **(observed:** each evaluation prints `Cost::affmat = `, the candidate matrix `M` in the `.mat` format and an empty line, in evaluation order; levels 5, 19 and 100 print nothing else, no cost values. These traces are how the clean-room implementation checked its search path evaluation by evaluation.**)** |
| `-help` | usage on stdout and exit 0, but only with at least one other argument: `mcflirt -help` alone exits 1 **(observed)** |

Errors **(observed)**:
- no input: "Input filename not found", usage on stdout, exit 2;
- fewer than two arguments (`mcflirt`, `mcflirt -help`): usage on stdout, exit 1;
- any option other than the value-less flags, placed last (known or not): "Lacking argument
  to option X", exit 255; an unknown option elsewhere: "Unrecognised option X", exit 255.
  `-edge`, which nipype's `use_contour` emits, is not an mcflirt option;
- `-refvol` ≥ N or ≤ −2: aborts ("Invalid t index"), signal 6, no outputs; `-refvol -1` is
  the default;
- `-dof` below 6 or above 12: "Erroneous dof 5 : using 6 instead" (or 12) on stderr for every
  volume and stage, then runs with 6 (or 12);
- `-stages 0` with `-reffile`: the reference is never read and mcflirt aborts at the
  resampling ("Attempted to use affine transform with no voxels in vout").

## 12. Edge cases

| Case | Behaviour |
|---|---|
| one volume, no `-reffile` (3D or 4D file) | nothing is registered; one identity `.mat`; the output equals the input **(observed)** |
| one volume, `-reffile` | that volume is registered **(observed)** |
| two volumes, no `-reffile` | reference is volume 1 |
| reference with a different voxel count | the estimation runs, then mcflirt aborts at the final resampling ("Attempted to copydata with non-matching sizes", signal 6); nothing is written, not even the `.mat` files **(observed)** |
| reference with the same voxel count but different shape or geometry | estimation in each image's own FSL-mm frame; corrected data on the reference's grid under the input's header (§9.1) |
| oblique images | no effect on the estimation (§4.1) **(observed)** |
| positive-determinant storage | x reversed in memory, so matrices refer to the reversed array (§4.2) **(observed)** |
| sform and qform with opposite determinant signs, or a zero determinant | not reversed |
| NaN or infinite voxels | one NaN voxel makes the volume's centre of mass (§6.1) and hence its matrix all NaN (written as `nan`), its `.par` line NaN, the `.rms` values involving it NaN, and its corrected volume the background value; other volumes are unaffected, also the one initialised from it (§5.4) **(observed)** |
| integer types | read and processed as float32; output truncated (§9.4) |
| thin slabs: a checked image has fewer than 3 slices, or its z extent (slices × z voxel size) is below `fov` (20 mm, `-fov`). The checked images are the 8 mm and 4 mm reference grids and every test volume; the 8 mm grid is checked first, so with the default `fov` any series less than 24 mm thick qualifies (its 8 mm grid has at most 2 slices). Also `-2d` | **in-plane mode** (below) **(observed** for a 4-slice, 16 mm slab, also with `-fov 10`**)** |
| very small images (e.g. 10 × 10 × 8 of 4 mm) | run normally; the 8 mm grid is 5 × 5 × 4 |
| truncated or unreadable input | "Failed to read volume ... short read, file may be truncated", abort (signal 6) **(observed**, ds003763 as published**)** |
| `-refvol -1` | the default reference `N / 2` **(observed)**; other negative values abort (§11) |

**In-plane mode** (tier 3). Once triggered it stays on for the rest of the run, for every
volume and stage:
- only `rz`, `tx`, `ty` are searched (in that order); `rx = ry = tz = 0` and the scales
  are 1 **(observed:** those columns of `.par` are exactly 0**)**;
- every reference grid and test volume is padded with one copy of its first slice before it
  and one copy of its last slice after it, and its z voxel size is set to 8 mm; in stage 1
  the reference grid is padded twice;
- the edge-weighting width becomes 0.1 mm;
- mcflirt prints `Params: rz : tx : ty` and `Tolerances: ...` lines on stderr for every
  volume, and "restricting optimization to R_z, T_x and T_y" with `-report`.

### 12.1 Bins no histogram can have (FSL-21, FSL-66) (spec writer, 2026-10-30)

(spec writer, 2026-10-30) Stage `k` has `B_k = n / s_k` bins (§11's `-bins` row; `n / 8` in
stage 1, `n / 4` in stages 2 to 4, truncated toward zero: `-bins 7` gives 0 and 1, `-bins -9`
gives −1 and −2). Each stage prepares its bins when it starts, for every cost function; the
histogram costs (`corratio`, `mutualinfo`, `normmi`, `woods`) use them.

**What mcflirt does** (FSL 6.0.7.23; the same in 6.0.7.17):
- **`B_k = 0` (FSL-21)**, i.e. `-bins` from −7 to 7 in stage 1, −3 to 3 in stages 2 to 4:
  - `normcorr`, `leastsquares`: no effect (they use no bins) **(observed:** `-bins 2` and `-bins
    -1`: the matrices of the default, status 0**)**;
  - `mutualinfo`, `normmi`: appendix B's rules with `B = 0` give every reference grid point bin
    0 and every sample centre bin `c = 0`, its upper neighbour `c⁺ = −1` (outside the histogram;
    its weight `w⁺` is exactly 0, so nothing changes) and its lower neighbour `c⁻ = 0`; with `f =
    0` (`u` is `±0`) each sample adds `g·0.5` twice to the single joint cell and to the single
    test cell, and `g` once to the single reference cell. The joint and test entropies are then
    equal, the corrected reference entropy is about 0, and the cost is rounding noise about a
    constant: the line searches end at arbitrary (deterministic) matrices, the same for every
    volume or tens of millimetres away **(observed:** `-bins 7`, `-bins 4 -stages 2`, `-bins 4`
    with three stages and `-plots`, `-bins 0`, `-bins abc` (read as 0): status 0, the matrices
    nothing like the default's**)**;
  - `woods`: every sample falls in the one bin (index 0, which here is also bin `B`), so the cost
    is that bin's term over the whole overlap: no error, a meaningless cost **(observed:** `-bins
    1`: status 0**)**;
  - `corratio`: after its sums, the correlation ratio folds the last bin (index `B`) into the one
    before (index `B − 1`); with `B = 0` that is outside its arrays, before their start, and the
    C library detects the damage when the arrays are released: `double free or corruption (out)`
    on standard error and signal 6 (abort), at the first cost evaluation in which any sample was
    counted, nothing written **(observed:** `-bins 2`, also with `-smooth 0`; with `-report` the
    message follows `Registering volumes ... [3]`, the first volume's index**)**.
- **`B_k < 0` (FSL-66)**, for **every** cost function, `normcorr` included, when stage `k` starts:
  - `B_k = −1` (`-bins` from −15 to −8, or from −7 to −4 with two stages or more): an integer
    division by zero while the stage prepares its tables: the process is killed by signal 8
    (`SIGFPE`), with no message of its own and nothing written **(observed:** `-bins -8`, `-bins
    -4 -stages 2`: exit by signal 8**)**;
  - `B_k ≤ −2` (`-bins` −16 or less): `terminate called after throwing an instance of
    'std::bad_array_new_length'` and `  what():  std::bad_array_new_length` on standard error,
    signal 6, nothing written **(observed:** `-bins -16` with `normcorr` and with
    `mutualinfo`**)**;
  - `-bins` from −3 to −1: `B_k = 0` in every stage, as above.
- With `-stages 0`, or a single volume and no `-reffile` (nothing is registered), no stage
  starts and `-bins` changes nothing **(observed:** `-bins 4 -stages 0` and a one-volume series
  with `mutualinfo`: status 0, identity matrices**)**.

**What larmorx does** (the replica's decisions of 2026-10-24 for FSL-21, rule B: a histogram needs
at least one bin, and mcflirt silently returns arbitrary matrices or corrupts its heap; the same
rule for FSL-66, whose crashes are never reproduced, D13 class (a)):

| Case | Default (`fix`) | `reproduce` |
|---|---|---|
| FSL-21: a histogram cost, `-stages` 1 or more (the default 3), and `-bins` from −7 to 7 (`n / 8` is 0) | refused **before any work** (before the input is registered; whatever the number of volumes or `-reffile`): `Error: -bins <n>: the histogram costs need at least 8 bins (the first stage uses -bins / 8 of them; mcflirt computes them with none, FSL-21)`, exit status 1, nothing written; a notice naming FSL-21 (`-bins <n> is refused with this cost: the first stage's histogram would have no bins`) | mcflirt's rules with `B_k = 0` (above): `mutualinfo`, `normmi`, `woods` run to their (arbitrary) matrices, status 0; `corratio` ends where mcflirt aborts: an error, exit status 1, nothing written (the replica prints `double free or corruption (out)` and then its own line saying mcflirt aborts there, signal 6); a notice naming FSL-21 is welcome (the replica prints none in this mode) |
| FSL-21 with `-stages 0` | runs (no stage, no bins needed) | the same |
| FSL-66: `-bins` negative with `-stages` 1 or more (with `normcorr` and `leastsquares` too); with a histogram cost the values −7 to −1 are FSL-21's row | refused before any work: an error naming `-bins <n>` (a negative number of bins) and FSL-66, exit status 1, nothing written; a notice naming FSL-66 | where mcflirt crashes (above: `B_k ≤ −1` in a stage that runs), an error saying how mcflirt ends (signal 8, or the `bad_array_new_length` abort with its two lines), exit status 1, nothing written; elsewhere (−3 to −1 with `normcorr`, `leastsquares`) as mcflirt |

**(spec writer, 2026-10-30)** The replica does the FSL-21 rows today; FSL-66 is new: the replica
and the clean-room original both run normally on negative `-bins` in both modes (they take the
stage's negative bins as none, or the original as one), a follow-up of the replica (the table is its target).

## 13. Numerics, operation order and determinism

| Quantity | Precision |
|---|---|
| image values, reference grids, interpolation | float32 |
| matrices, matrix inverses, parameter vector | double; matrices rounded to float32 entries before use in the sampling loops |
| sample positions | float32, incremental (§6.1, §9.1) |
| cost sums, count `N`, correlation | float32 (three-level sums, §6.4). **(observed,** by comparing the first parabola step of every volume of ds000005 run 1 with `-fudge -stages 1` against `-verbose 20` traces: square roots in double, sums in two levels instead of three, or `N` changed by one float32 ulp, each drop the agreement from 140 of 240 volumes to 1–42; `σ` other than `smooth/d` to 0**)** |
| line-search positions and cost values | float32 |
| centre of mass | double sums |
| Euler decomposition | float32 intermediates |
| rotation composition | the angle vector's norm is rounded to float32 before the rotation is built; the rotation matrix is therefore orthogonal only to about 1e−8. **(observed:** the translations of the evaluated matrices (`-verbose 20`) are reproduced only with each elementary rotation's cosine and sine rounded to float32, e.g. `1 − cos` off by 3.2e−8 at an angle of −0.00315; computing them in double reproduces 12 of 240 first parabola steps on ds000005, rounding them to float32 140 of 240**)** |
| `.mat` output | 6 decimals |
| **(FSL 6.0.7.23)** 4 × 4 inverses and solves | FSL 6.0.7.23's matrix library (Armadillo 14.2.1) inverts a general 4 × 4 matrix by an LU decomposition with partial pivoting (LAPACK `getrf` and `getri`, OpenBLAS 0.3.33), a diagonal one element by element and a triangular one by back substitution, and solves `inv(A)·B` by LU (`gesv`; for a symmetric `A`, such as a diagonal sampling matrix, by its symmetric indefinite factorisation); FSL 6.0.7.17's (Armadillo 10.5.1) used the closed-form cofactor inverse for every matrix up to 4 × 4. The difference is in the last bits: it changes no matrix, image or `.par` value in 127 of 137 recorded runs, but values computed from nearly equal matrices (the `.rms` of identical volumes, about 1e−15 mm) and the sign of NaN parameters (§12) differ **(observed:** 5 recorded runs; the same results under every OpenBLAS core this CPU runs, Haswell to Cooperlake**)** |

**Determinism.** mcflirt uses no random numbers and runs single-threaded (the FSL library
disables BLAS threading at start-up). Repeated runs give byte-identical outputs **(observed:**
three runs of fMRIPrep's command on ds000005 run 1: all 241 output files identical**)**.
Storage that leaves the in-memory float32 data unchanged (LAS vs x-reversed RAS, oblique
affines, int16 vs float32/float64/uint16 holding the same values, int16 with a scale factor)
gives bit-identical matrices **(observed)**.

**Speed (observed,** one core of a loaded 12-core x86-64 machine**):** about 30 s for a
64 × 64 × 34 × 240 run (ds000005), 60 s for 96 × 96 × 69 × 113 (ds006010), 75 s for
64 × 64 × 30 × 239 (ds000258); `-stages 4` multiplies the time by about 17 (531 s on
ds000005), `-sinc_final` adds about 100 s.

## 14. Validation plan for the original

**Oracle data** (`<workspace>/oracles/fsl-6.0.7/mcflirt/`, outside the repository; 137
cases; `manifest.json` lists every run, each run directory holds `run.json` with the exact
argv, environment, exit code, wall time and SHA-256 of inputs and outputs; `analysis/` holds
the scripts that check this spec's **(observed)** statements and their outputs):
- **real BOLD** from `larmorx-testdata`: ds000005 sub-01 run 1 with fMRIPrep's published
  boldref (fMRIPrep's exact command, its repeats, and a sweep of every option), and
  ds000117, ds000210 (echo 2, neurological storage), ds006010 (uint16, multiband),
  ds005040, ds000122, ds003345, ds006736, ds000258 (big-endian) with a median-of-10
  reference: fMRIPrep's command, plus `-plots -rmsrel -rmsabs`, plus mcflirt's defaults;
- **synthetic** series with known motion (`larmorx-testdata` `synthetic/hmc/`, generated by
  `generators/hmc_synthetic.py`, ground truth in JSON sidecars): small and large motion,
  identical copies, orientations, data types, NaNs, a ramp, a non-zero background, one and
  two volumes, a thin slab, a tiny image, and the error cases of §11–12.

**Metrics**, per run, comparing larmorx with mcflirt:
1. matrices: for each volume, the RMS deviation (§10.4, R = 80 mm, centre of the reference's
   field of view) between larmorx's and mcflirt's matrix, and the parameter differences
   (decomposed about the same centre) in mm and degrees;
2. FD (fMRIPrep's: Power, radius 50 mm, from the parameters) and fMRIPrep's `rmsd`: Pearson
   correlation of the series and the maximum absolute difference;
3. against the synthetic ground truth: the same RMS deviation, for larmorx and for mcflirt
   (larmorx must not be worse);
4. corrected images: maximum absolute difference and NCC, where the matrices agree.

**Thresholds** (PLAN.md §11.3, "Motion parameters"): median parameter difference ≤ 0.05 mm
and ≤ 0.05°; FD correlation r > 0.99; additionally the 95th percentile of the matrix RMS
deviation ≤ 0.1 mm. FD correlation is a weak test on very still subjects (ds000005: mean FD
0.07 mm, about three stage-3 tolerances), so report it with the FD range and use the RMS
deviation as the primary metric there.

**The variability band** to calibrate them:
- mcflirt is deterministic and blind to storage details (§13), so repeated runs give no
  spread. The band has to come from perturbations that leave the problem essentially
  unchanged: one-voxel crops of the field of view, ±0.5 LSB noise, reversing the volume
  order, a reference resampled by an identity transform. **(observed,** made by the clean-room
  implementer with `larmorx_validation.parity.mri_hmc_band`, outputs in
  `oracles/fsl-6.0.7/mcflirt/band/`: mcflirt on its own input with uniform noise of ±0.5
  (three realisations), with the volumes in reverse order (separate reference only) and with
  the top slice cropped, compared with its unperturbed matrices. On fMRIPrep's command for the
  nine real runs the 95th percentile of the RMS deviation is 0.013–0.10 mm (median
  0.008–0.022 mm) under noise and reverse order, and the FD correlation is as low as 0.79
  (ds000005) where the subject barely moves. `-meanvol` varies more (ds000005: 0.054 mm; the
  synthetic series: 0.27–0.42 mm with noise), as do `-dof 12` (0.10–0.26 mm with noise,
  1.2 mm in reverse order) and the 10 × 10 × 8 image (0.26–0.36 mm). The synthetic series
  with a non-zero background is unstable: one of the three noise realisations moves its
  matrices by 4.0 mm (95th percentile). Cropping a slice of a thin image changes the problem
  itself: mcflirt's matrices move by 7.5 mm (tiny image) and 99 mm (the series with a non-zero
  background).**)**
- Option changes measured on ds000005 (matrix RMS deviation from fMRIPrep's command, median /
  95th percentile, mm) **(observed)**: `-smooth 2` 0.009 / 0.023; `-rotation 2` 0.010 / 0.024;
  `-stages 2` 0.027 / 0.045; `-cost corratio` 0.028 / 0.050; `-stages 1` 0.045 / 0.062;
  `-fudge` 0.049 / 0.101; `-smooth 0` 0.072 / 0.117; `-stages 4` 0.145 / 0.195. `-scaling`,
  `-bins` (with normcorr), `-stats`, `-cost normcorr` and the final-interpolation options
  change nothing. `-cost woods` fails on BOLD data (deviations of about 90 mm).
- Another FSL version: fMRIPrep 21.0.1's published ds000005 confounds were made with FSL
  6.0.5.1's mcflirt. Against our 6.0.7.17 run with the default reference they differ by a
  near-constant offset (up to 0.16 mm and 0.06°) plus a per-volume scatter of 0.004–0.012
  (mm or degrees, standard deviation); with the published boldref as reference the scatter is
  up to 0.053 mm (tz). The HMC reference of that run was not published, so these numbers bound
  the version effect from above; they do not measure it.
- The line-search tolerances (§7.4) set the expected scale: about 0.02 mm and 0.03° per
  parameter in stage 3.
- Accuracy of mcflirt itself against the synthetic ground truth **(observed,**
  `synthetic/hmc/motion/small` with `-reffile`**)**: translation errors up to 0.14 mm,
  rotation errors up to 0.7° (about the antero-posterior axis), matrix RMS deviation up to
  0.69 mm (mean 0.35 mm). The phantom is smooth, so rotations are weakly constrained; larmorx
  must match mcflirt, not the truth, but report both.
- Only one FSL build (6.0.7.17, conda, x86-64) is installed here; a second build (FSL 6.0.5,
  or the FSL of fMRIPrep's current image) should be added for the band.

Bit-identity of the matrices is reported (counting the exact `0.000000` entries of §7.4),
not required.

### 14.1 The failure cases of §10.6 and §12.1 (spec writer, 2026-10-30)

The cases a clean-room implementer adds to the original's parity suite (`mri-hmc`, edge cases)
and its command-line tests, each in both modes, checking against mcflirt 6.0.7.23 run on the same
input: the exit status (a signal counts as "mcflirt crashed": larmorx's status is then 1), the
first line of standard error that is mcflirt's (larmorx's own lines, `Error: ...` and the
notices, set aside and checked separately: the defect each names), and the set of files written
(names; the matrices' values only where both run to the end, compared as the other cases are).
The black-box runs behind §10.6 and §12.1 are made by
`validation/investigations/specs-memory/mcflirt_fsl20_21.py` (a 24 × 24 × 16 × 5 phantom, its
cases named as below; `--larmorx` runs larmorx's command line instead, `compare_runs.py` sets the
runs side by side):

| Case | Arguments (with `-in` the series) | mcflirt | larmorx default | `reproduce` |
|---|---|---|---|---|
| `fsl20-mats` | `-out missing/o -mats -stages 1` | status 0, the message, no file | 1, FSL-20 | 0, the message, no file |
| `fsl20-mats-plots`, `fsl20-reffile`, `fsl20-file-parent` | likewise with `-plots`, with `-reffile`, `-out afile/o` | the same | the same | the same |
| `fsl20-rmsrel`, `fsl20-rmsabs-report` | `-rmsrel` or `-rmsabs -report` without `-mats` (FSL-13 also named) | status 0, the message | 1, FSL-20 | 0, the message |
| `fsl20-perm` | `-out ro/o -mats`, `ro/` read-only | the permission message, then signal 6 at the series | 1 | 1 |
| `fsl20-plots-only`, `fsl20-none` | `-plots` alone; no output option | `-plots`: FSL-65's message, status 0; none: signal 6 at the series | 1 | `-plots`: 0, FSL-65's message; none: 1 |
| `par-dir-plots-mats`, `par-dir-plots`, `par-dir-plots-mats-ref0` | a directory `o.par`; `-mats -plots -rmsrel`, `-plots`, `-mats -plots -refvol 0` | status 0, FSL-65's message, `MAT_0000` and the `.rms` files only | 1, FSL-65, nothing written | 0, the message, mcflirt's files |
| `rms-dir` | directories `o_rel.rms` and `o_abs_mean.rms`; `-mats -rmsrel -rmsabs` | status 0, those two files missing | 1, FSL-65 | 0, mcflirt's files |
| `bins7-mutualinfo`, `bins4-mutualinfo-s2`, `bins4-mutualinfo-s3-plots`, `bins0-normmi`, `bins1-woods`, `binsabc-mutualinfo` | the cost and `-bins` named | status 0, arbitrary matrices | 1, FSL-21, nothing written (`abc`: FSL-07's refusal first, status 255) | 0, matrices by §12.1's rules (close to mcflirt's only as far as the costs' rounding agrees: report, do not require) |
| `bins2-corratio`, `bins2-corratio-smooth0`, `bins2-corratio-report` | `-cost corratio -bins 2` | signal 6, `double free or corruption (out)`, nothing written | 1, FSL-21 | 1, nothing written |
| `bins8-corratio`, `bins8-mutualinfo-s3` | 8 bins | runs (one bin in stage 1) | runs | runs |
| `bins2-normcorr`, `bins2-leastsq`, `binsm1-normcorr` | `-bins 2` or `-1` with the costs that use no bins | runs, the default's matrices | `2`: runs; `-1`: 1, FSL-66 | runs |
| `bins4-mutualinfo-s0`, `bins4-mutualinfo-1vol` | no stage runs | runs (identities) | `-stages 0`: runs; one volume: 1, FSL-21 (refused before any work, whatever the volumes) | runs |
| `binsm4-normcorr-s2`, `binsm8-normcorr` | `-bins -4 -stages 2`, `-bins -8` | signal 8, nothing written | 1, FSL-66 | 1, nothing written |
| `binsm16-normcorr`, `binsm16-mutualinfo` | `-bins -16` | signal 6, `bad_array_new_length`, nothing written | 1, FSL-66 | 1, nothing written |

Also one golden case per new refusal (the error's exit status and that nothing is written), and
the Python API: `lx.mri.hmc(..., bins=4, cost="mutualinfo")` raises the same error by default.

## 15. Paper vs implementation

Jenkinson, Bannister, Brady & Smith (2002), *NeuroImage* 17:825–841, and its technical
report TR02MJ1 describe MCFLIRT. The implementation departs from or adds to it as follows.

| Paper | mcflirt 2111.0 |
|---|---|
| Multi-resolution: both images are blurred (Gaussian) and resampled to the scale | Only the reference is resampled, **without blurring**; the test volume is used at full resolution, unsmoothed (§5.3) |
| Normalised correlation (a correlation coefficient of the two images; the formula is in the paper's Table 1) | An effectively **uncentred** correlation, because of the count `N` of §6.4; Pearson's only with `-smooth 0` |
| Apodization: linear down-weighting near the edge of the overlap | As described, on the test volume's boundary only, width 1 mm by default (§6.3) |
| Local optimisation by Powell's method or N 1-D golden searches, tolerances proportional to scale | One sweep of six 1-D searches per stage (bracketing + parabolic/golden refinement), fixed tolerances per stage (0.8, 0.8, 0.1 times the base), no convergence loop (§7) |
| Schedule: 8 mm, 4 mm, 4 mm with lower tolerance; optional mean registration; optional sinc stage | Same; the sinc stage uses a Hanning-windowed sinc, half-width 3, as described; the final `-sinc_final` uses a Blackman window |
| Each image's low-resolution result initialises the next | Same, except for the last volume of the series (§5.4) |
| Reference: the middle image | Volume `N/2` (0-based; the later of the two middle volumes when N is even) |
| End slices: data padded by 2 slices when applying the transform | One replicated slice on each side (§9.2); background value beyond |
| Centre of transformation: centre of mass | Centre of mass of each test volume with its minimum subtracted (§6.1); `.par` uses the reference's |
| — | In-plane mode for thin slabs (§12), NaN handling (§12), truncation of integer outputs (§9.4) |

## 16. larmorx interfaces

- **Rust**, crate `larmorx-mri`: a function taking a 4D float32 series (with voxel sizes and
  storage order already resolved to FSL's in-memory convention) and a reference volume, and
  returning per-volume 4 × 4 matrices (FSL-mm, §4.2), with explicit `n_threads`. Volumes in
  stages 2–4 are independent and can run in parallel; stage 1 is a chain within each pass
  (§5.4) and is sequential unless `-fudge` semantics are chosen. A separate function
  resamples the series (§9).
- **Python:** `lx.mri.hmc(bold, *, reference=None, ref_index=None, mean=False, stages=3,
  final="trilinear", dof=6, cost="normcorr", smooth=1.0, n_threads=1, resample=True) ->
  HmcResult` with `matrices` (FSL), `world` (RAS, §4.3), `params` (§10.3), `rms_abs`,
  `rms_rel` and the corrected image (if `resample`). `implementation="auto"` as in
  `docs/licensing.md`.
- **CLI:** §11, writing the files of §10.
- **Docs:** `docs/api/mri-hmc.md`, with the option-mapping table.

## 17. Open questions

1. The exact numeric form of the correlation (§6.4) mixes float32 and double; whether a
   double-precision clean-room implementation stays within the thresholds is to be
   measured, not assumed. **(observed, partly settled by the clean-room implementer:** an
   implementation following §6 exactly, with float32 sines and cosines (§13), reproduces the
   first parabola step of 140 of 240 volumes of ds000005 (`-fudge -stages 1`) and of 20 of 20
   volumes of a synthetic series whose test volumes are constant; with test volumes that vary,
   50–70 % agree. The remaining steps differ by one or two float32 quanta of `r`, so some detail
   of how the interpolated test values enter the sums is still unknown. Variants that did not
   help: other orders of the trilinear steps, the interpolation in double, `(w·y)·x` or
   `w·(y·y)` forms of the products, positions computed directly or from a double row start,
   transposed or Gauss–Jordan inverses, float32 or perturbed centres, float32 trial points of
   other values.**)**
2. The variability band (§14) needs perturbation runs; none are made yet. **(Settled by
   the clean-room implementer for 20 runs, see §14; a second FSL build is still missing.)**
3. `-spline_final` boundary handling (§9.3) is described from analysis only; it is tier 2
   and should be confirmed by black-box comparison before larmorx claims it.
4. Histogram costs (`mutualinfo`, `normmi`, `corratio`, `woods`) and `leastsquares` are
   described only in outline (appendix A); oracle runs exist for ds000005. **(FSL 6.0.7.23)**
   `mutualinfo` and `normmi` are now described exactly (appendix B), for the clean-room
   original's `normmi` follow-up (its median temporal-SD ratio of 1.033 to mcflirt on 91 random
   phantoms).
5. A second FSL build for the band.
6. Whether larmorx should default to a better optimiser (D11) once the band is known: the
   uncentred correlation, the single sweep and the tolerance quantisation are mcflirt's, not
   necessities.

**Where exact agreement will be hard** (in decreasing order of effect):
1. the stage-1 chain (§5.4): an early difference propagates along the series;
2. stage 3 deciding on cost differences at the float32 noise level (§7.4), which depends on
   the three-level float32 sums and on `N` (§6.4);
3. the float32 sample positions and the row ranges (§6.1), which decide which points enter
   the sums;
4. the float32 Euler round trip between stages (§4.4, §5.4) and the slightly non-orthogonal
   rotations (§13);
5. the reference grids' float32 step accumulation and sizes (§5.3).

Write the `.mat` files in mcflirt's text format (§10.2): fMRIPrep reads them, and their
6-decimal rounding is part of what it sees.

## Appendix A. Other cost functions (tier 3)

All use the sampling, interpolation and edge weights of §6.1–6.3. Reference grid values are
binned once per stage: `bin = ⌊(g − g_min)·B/(g_max − g_min)⌋`, clamped to `0 … B−1`
(`B` = bins, §5.2; `g_max − g_min = 0` is replaced by 1).
- `leastsquares`: weighted mean of `(x − y)²` with the true weight sum (no `N` quirk);
  empty overlap gives `(max − min)²` over both images.
- `corratio`: `Σ_b n_b·var_b / (n·var)` over bins with `n_b > 2` (weighted counts, variances
  with `n_b − 1`), i.e. 1 − the correlation ratio of the test values given the reference bin;
  0 if the total variance is not positive.
- `woods`: `Σ_b n_b²·σ_b/μ_b / n` (unweighted; `σ_b/μ_b` replaced by `σ_b` when the bin mean
  is not positive); `1e10` for an empty overlap.
- `mutualinfo` (cost `−MI`) and `normmi` (cost `−(H1 + H2)/H12`), exactly in appendix B:
  joint histogram of the
  reference bins and test bins (test values binned over the test volume's min–max), each
  sample spread over its bin and the neighbouring one with a linear "fuzzy" weight within
  half a bin of the bin edge, times the edge weight; entropies from probabilities
  `count / total grid points`, corrected to the overlap size by `H' = (n_grid/n_overlap)·H −
  log(n_grid/n_overlap)`.

## Appendix B. `mutualinfo` and `normmi` exactly (FSL 6.0.7.23)

Both costs are unchanged from FSL 6.0.7.17; this appendix describes them completely (FSL
6.0.7.23). Every quantity is float32 unless said otherwise; sums are accumulated in the order
given.

**The pair.** At every stage (§5.2) the cost compares the reference grid of that stage (the
reference resampled to the stage's scale, §5.3: "ref", `n_x × n_y × n_z` points) with the volume
being registered at its own resolution ("test", after §12's in-plane padding where it applies).
Each evaluation maps every reference grid point into the test volume through the inverse of the
trial matrix, with the same positions, row ranges and bounds test as `normcorr` (§6.1), and
interpolates the test volume trilinearly there (§6.2).

**Bins.** `B = ⌊bins / s⌋` (integer division; (spec writer, 2026-10-30) truncating toward zero for a negative `bins`, §12.1) with `bins` from `-bins` (default 256) and `s` the
stage's scale in mm: with the default, 32 bins in stage 1 (8 mm) and 64 in stages 2 to 4 (4 mm).
- *Reference bins*, once per stage: `r_min`, `r_max` the minimum and maximum of the reference
  grid's values; when `r_max − r_min = 0`, `r_max` is increased by 1. `a1 = B/(r_max − r_min)`,
  `a0 = −r_min·B/(r_max − r_min)` (float32: `−r_min·B` first, then the division). Each grid
  point's bin is `trunc(v·a1 + a0)` (the product, then the sum, in float32; the conversion to an
  integer truncates toward zero, and a value out of the integer range or NaN gives the most
  negative integer), clamped to `0 … B−1`.
- *Test bins*, at every evaluation: `t_min`, `t_max` the minimum and maximum over every voxel of
  the test volume (not only those sampled); `b1 = B/(t_max − t_min)`, `b0 = −t_min·B/(t_max −
  t_min)` (float32; not guarded: a constant test volume divides by 0). For a sample `y`:
  `u = y·b1 + b0`; `c = trunc(u)` (toward zero, 64-bit); `c⁺ = c + 1`, `c⁻ = c − 1`; then, in
  this order: if `c ≥ B`, `c = B − 1` and `c⁺ = c`; if `c < 0`, `c = 0` and `c⁻ = c`; if
  `c⁺ ≥ B`, `c⁺ = B − 1`; if `c⁻ < 0`, `c⁻ = 0`.

**Fuzzy (partial-volume) binning.** The fraction `f = |u − trunc₃₂(u)|` (`trunc₃₂`: toward zero
through a 32-bit integer), so `f ∈ [0, 1)`, measured from the truncated value also for negative
`u`. With the fuzzy fraction `φ = 0.5` (fixed; mcflirt has no option for it):
- `f < φ`: centre weight `w_c = 0.5 + 0.5·(f/φ)`, `w⁻ = 1 − w_c`, `w⁺ = 0`;
- `f > 1 − φ`: `w_c = 0.5 + 0.5·((1 − f)/φ)`, `w⁺ = 1 − w_c`, `w⁻ = 0`;
- otherwise (`f = 0.5` exactly) `w_c = 1`, `w⁺ = w⁻ = 0`.
So each sample is shared between its own bin and the nearer neighbour, linearly, the share
reaching one half at the bin's edge; at the clamped ends the neighbour's share falls into the
same bin.

**Edge weight.** `g = 1`; with `σ_x = smooth/d_x`, `σ_y = smooth/d_y`, `σ_z = smooth/d_z` (`smooth`
from `-smooth`, default 1 mm; `d` the test volume's voxel sizes) and `x_b = n_x^test − 1.0001`
(likewise `y_b`, `z_b`, in float32), for each axis in turn (x, y, z) with `o` the sample's test
coordinate on it: if `o < σ`, `g ← g·(o/σ)`; else if `x_b − o < σ`, `g ← g·((x_b − o)/σ)`;
finally `g ← max(g, 0)`. With `-smooth 0` every `g` is 1 (no comparison holds). These costs
always take this weighted, fuzzy form, whatever `-smooth` (the fuzzy fraction is never 0).

**Histograms.** A joint histogram `J` of `(B+1) × (B+1)` cells and two marginals `M_ref`, `M_test`
of `B + 1` cells, all float32 and zeroed at each evaluation (the cells of index `B` stay 0). For
every sample, in the sampling order (z, then y, then x along each row), with `a` the reference
bin of its grid point: `J[a][c] += g·w_c`, `M_test[c] += g·w_c`, `J[a][c⁺] += g·w⁺`,
`M_test[c⁺] += g·w⁺`, `J[a][c⁻] += g·w⁻`, `M_test[c⁻] += g·w⁻` (each product `g·w` rounded,
then added), then `M_ref[a] += g`.

**Entropies.** `N = n_x·n_y·n_z` of the reference grid (as float32) — all grid points, sampled or
not. `H_joint = Σ −p·ln p` over the cells of `J` with a positive count, `p = count/N`, in index
order (reference bin major); `H_ref` likewise over `M_ref`; `H_test` over `M_test`, whose positive
counts also sum to `n_ov` (float32). `ln` is the single-precision logarithm (glibc's `logf`),
each term `−p·ln p` rounded to float32 and added. Then, when `n_ov > 0`: `ρ = N/n_ov` and each of
the three is replaced by `ρ·H − ln ρ` (the logarithm once, in float32), the correction of an
entropy measured over `N` points to the `n_ov` points that overlap. When `n_ov = 0`: `H_joint =
2·ln B`, `H_ref = H_test = ln B` (natural logarithms in double, stored in float32): the values of
a bad registration.

**The costs.** `mutualinfo`: `MI = H_ref + H_test − H_joint`, cost `−MI`. `normmi`:
`NMI = (H_ref + H_test)/H_joint`, or 0 when `|H_joint| < 1e−9`; cost `−NMI`. Both are minimised
by the line searches of §7, like every cost; neither is centred or normalised further (the
range `−2 … −1` of `−NMI` and its flatness near the optimum are what the line search sees). With
`-bins` below 8, stage 1 has `B = 0` bins (FSL-21, §12); (spec writer, 2026-10-30) what the rules above give with `B = 0`, and with a negative `B` (FSL-66), is in §12.1.
