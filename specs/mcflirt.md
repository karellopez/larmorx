# Spec: `lx.mri.hmc` (behaviour of FSL mcflirt, FSL 6.0.7)

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

**Version.** FSL 6.0.7.17 (Linux x86-64 conda build), packages:

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

### 9.4 Output values

The resampled series is float32. It is written in the input's storage type (§10.1),
converted by **truncation toward zero** (no rounding, no clamping) **(observed:** the int16
output equals the truncated float32 output of the same data, and is not the rounded one**)**.

## 10. Outputs

### 10.1 The corrected series

- **Name.** `<out>` with any image extension removed, plus the extension chosen by
  `FSLOUTPUTTYPE`: `NIFTI_GZ` → `.nii.gz`, `NIFTI` → `.nii`, `NIFTI2_GZ` → NIfTI-2 `.nii.gz`,
  and the pair types (`.hdr/.img`). Without `-out`, `<out>` = the input name without
  extension, plus `_mcf` **(observed)**. Unset `FSLOUTPUTTYPE` makes mcflirt fail (exit 1)
  after the estimation, after writing the `.mat` files **(observed)**.
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
  string (`2203.12-dirty 2024-02-01T16:17:47+00:00` here); intent fields and `aux_file` from
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
| `-bins n` | histogram bins (`atoi`) |
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
  `oracles/fsl-6.0.7/mcflirt/band/`: mcflirt on its own input with uniform noise of ±0.5,
  with the volumes in reverse order (separate reference only) and with the top slice cropped,
  compared with its unperturbed matrices. On fMRIPrep's command for the nine real runs the
  95th percentile of the RMS deviation is 0.015–0.10 mm (median 0.007–0.022 mm), and the FD
  correlation is as low as 0.79 (ds000005) where the subject barely moves. `-meanvol` varies
  more (ds000005: 0.054 mm; the synthetic series: 0.42 mm with noise), as do `-dof 12`
  (0.26 mm) and the 10 × 10 × 8 image (0.26 mm). Cropping a slice of a thin image changes
  the problem itself: mcflirt's matrices move by 7.5 mm (tiny image) and 99 mm (the series
  with a non-zero background).**)**
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
   described only in outline (appendix A); oracle runs exist for ds000005.
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
- `mutualinfo` (cost `−MI`) and `normmi` (cost `−(H1 + H2)/H12`): joint histogram of the
  reference bins and test bins (test values binned over the test volume's min–max), each
  sample spread over its bin and the neighbouring one with a linear "fuzzy" weight within
  half a bin of the bin edge, times the edge weight; entropies from probabilities
  `count / total grid points`, corrected to the overlap size by `H' = (n_grid/n_overlap)·H −
  log(n_grid/n_overlap)`.
