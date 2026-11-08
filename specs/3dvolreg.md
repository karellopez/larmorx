# Spec: `lx.afni.volreg` (behaviour of AFNI 3dvolreg 26.3.05)

(spec writer, 2026-11-07) The input for the clean-room **original** of larmorx's rigid-body
registration of a series to a base volume (`crates/larmorx-afni`, Apache-2.0), the program
fMRIPrep runs for its BOLD references (niworkflows' `RobustAverage`) and for sdcflows' PEPOLAR
reference averages. larmorx has two implementations (CLAUDE.md rule 1, `docs/licensing.md`):
- the bit-exact **replica** of AFNI's code, in the main package (`crates/larmorx-afni-replica`,
  Apache-2.0 with CC BY 4.0's attribution: AFNI 26.3.05 releases the Medical College of
  Wisconsin's code under CC BY 4.0), validated in `docs/validation/afni-volreg-replica.md`;
- the clean-room **original**, written from this file, AFNI's documentation (`3dvolreg -help`,
  `README.registration`), the papers (Cox and Jesmanowicz, *Real-time 3D image registration for
  functional MRI*, MRM 42:1014–1018, 1999) and black-box runs of the AFNI binary
  (`oracles/afni-26.3.05/bin/3dvolreg`, built by `scripts/build_afni_oracle.sh --add`), and
  built as a next-generation implementation: the same results, designed for speed (the user,
  2026-11-06; `docs/porting-guide.md`, "Clean-room originals: the same behaviour, built for
  speed").

**Whoever works on the original must not open** AFNI's source (`tags/AFNI-26.3.05/`,
`tags/AFNI-25.2.09/`, the `afni/` clone, `oracles/afni-*/build/`), the replica crate
(`crates/larmorx-afni-replica`; searches over `crates/` with
`--exclude-dir=larmorx-afni-replica`), the restricted findings (`docs/findings/afni-volreg.md`
and the others of CLAUDE.md rule 1), the replica's helpers that read AFNI's internals
(`validation/probes/afni-volreg-trace/`), the register's restricted sections, or `docs/parts/`.
The model of §15 (`validation/investigations/3dvolreg-spec/`) was written from this spec and
black-box runs only: it may be read and run.

This spec describes behaviour: what goes in, what comes out, and the arithmetic. Statements
marked **(observed)** were confirmed by running the AFNI binary; the rest come from AFNI's
documentation or the analysis of its behaviour. AFNI 25.2.09's 3dvolreg behaves as 26.3.05's on
every case of the replica's record (186 of 186, the version step); this spec is the newest
release's.

**Reused specs.** Reading and writing NIfTI datasets follows AFNI's rules as `specs/3dTshift.md`
§6 states them (values, scale factors, the geometry written, existing outputs and refused names,
§6.1–§6.3); AFNI's global options and environment variables as in its §7.1; the automask is
`specs/3dAutomask.md`'s (§3–§5) with the clip fraction this spec names; `-tshift` is 3dTshift's
default processing (§4, §5.1 of its spec); the glibc functions of `reproduce` are
`specs/glibc-dbl.md`'s (`sin`, `cos`, `sincos`) and `larmorx_core::math`'s `*_glibc` (`exp`,
`pow`). Where this spec says "as 3dTshift" it means that spec's section, nothing else.

**Defects (D13).** By default larmorx corrects AFNI's defects on this path and names each one
where it changed a result; `upstream_defects="reproduce"` (`LARMORX_UPSTREAM_DEFECTS=reproduce`)
gives AFNI's behaviour: AFNI-86 to AFNI-90, AFNI-92 to AFNI-99, and the AFNI tools' shared
AFNI-01 (on `-tshift`'s path), AFNI-11, AFNI-12, AFNI-18, AFNI-21, AFNI-26 and GLIBC-07. §12 gives
each with what the default and `reproduce` do; the statements of §3–§11 describe AFNI, that is
`reproduce`, unless they say otherwise.

---

## 1. Purpose

Each volume (sub-brick) of a 3D+time dataset is aligned to one base volume by a rigid-body
transformation (three rotations, three shifts). The method (help text, "Algorithm"): iterated
linearised weighted least squares, which makes each volume as much like the base as possible;
it is meant for **small motions**. Optionally a first pass on blurred volumes, started from a
coarse search over shifts and rotations, finds a starting point for larger ones (`-twopass`).

Outputs: the registered series (each volume resampled with the found transformation), the six
parameters per volume (`-1Dfile`, `-dfile`), the matrices (`-1Dmatrix_save`), the largest
displacement of the brain's edge voxels per volume and its change (`-maxdisp1D`), the per-voxel
displacements (`-savedisp`), and the `3drotate` commands that redo the resampling (`-rotcom`).

## 2. How fMRIPrep uses it

**RobustAverage** (niworkflows 1.14, `interfaces/images.py`; fMRIPrep 25.2.6's BOLD reference,
`init_raw_boldref_wf`, once per run and per 4D single-band reference):

    3dvolreg -Fourier -twopass -zpad 4 -prefix <in>_volreg.nii.gz \
             -1Dfile <in>_volreg.1D -1Dmatrix_save <in>_volreg.aff12.1D \
             -maxdisp1D <in>_volreg_md.1D <in>_sliced.nii.gz

Its input `<in>_sliced.nii.gz` is made from the BOLD run: the volumes of `t_mask` (the
non-steady-state volumes, at most 20 of them, or all), clipped to the 0.2 and 99.8 percentiles
(0 below when `nonnegative`), divided by the global signal's drift (the mean of each volume over
its voxels, divided by the largest such mean), clipped again, written as float32 with the
input's header (extensions removed). The base is volume 0 (the default). RobustAverage then takes
the median of the registered volumes (clipped to 0 below when `nonnegative`) as the reference.
**(observed)** The drift's mean sums float32 values in an order that numpy 2.3 changed: the
input must be made with fMRIPrep's own Python (numpy 2.2.6 in the fMRIPrep 25.2.6 image) to be
the file fMRIPrep gave 3dvolreg; with it, the median of AFNI's registered volumes is fMRIPrep's
own `gen_avg` output byte for byte on every run compared (§14.3).

**sdcflows' PEPOLAR estimator** (`fmap_preproc_wf/.../ref_average`) runs the same RobustAverage
call on its two topup-corrected reference volumes.

So fMRIPrep's default path is: Fourier interpolation for both the estimation and the output,
the two-pass scheme with its coarse rotation-and-shift search, zero padding of 4, the clipping
on, the edging and weights at their defaults, `-maxdisp` on.

## 3. Inputs, outputs and parameters

| Parameter | Option | Default |
|---|---|---|
| the series | `dataset` (last argument) or `-input`/`-source` | required; NIfTI-1/2 |
| base | `-base n` (a volume of the series) or `-base dset` (volume 0 of another dataset) | volume 0 |
| estimation method | `-heptic`, `-quintic`, `-cubic`, `-linear`, `-Fourier` | heptic |
| final (output) method | `-final NN\|linear\|cubic\|quintic\|heptic\|Fourier` | the estimation method |
| clip the output to the input's range | `-clipit` / `-noclip` | on |
| zero padding of every rotation | `-zpad n` | 4 |
| iterations | `-maxite m` | 23 |
| convergence: shifts | `-x_thresh x` (voxels) | 0.01 |
| convergence: rotations | `-rot_thresh r` (degrees) | 0.02 |
| derivative step | `-delta d` (voxels) | 0.70 |
| weight | `-weight 'wset[n]'`, `-wtinp` | the blurred base (§7.1) |
| edging (weight set to 0 near the faces) | `-edging e` or `-edging e%` | `AFNI_VOLREG_EDGING`, else 5 % |
| two passes | `-twopass`, `-twoblur b`, `-twodup`, `-sinit`, `-coarse del num`, `-coarserot`/`-nocoarserot` | off; b = 2.0; del = 10, num = 2; rotations searched |
| maximum displacement | `-maxdisp` / `-nomaxdisp`, `-maxdisp1D mm` | on |
| outputs | `-prefix`, `-float`, `-1Dfile`, `-dfile`, `-1Dmatrix_save`, `-rotcom`, `-savedisp`/`-SAVEDISP` | prefix `volreg` (AFNI's own format, refused by larmorx: §9) |
| slice timing first | `-tshift ii` | off |
| an extra transformation and grid | `-rotparent rset`, `-gridparent gset` | off |
| messages | `-verbose` (twice: more) | quiet |

The series is 3D+time (one volume is accepted only with an external base). Every volume is
registered independently to the base, from the same starting point, except that `-twopass`
starts each from its own first-pass result.

## 4. Coordinates and conventions

### 4.1 AFNI's grid of a NIfTI dataset

AFNI describes a dataset's grid by three **axes** (the voxel indices i, j, k, i fastest in
storage), each with:
- an **orientation**: the direction its index increases toward, one of R2L, L2R, P2A, A2P, I2S,
  S2I ("R2L": from right to left);
- a signed **step** Δ (mm): + for R2L, A2P, I2S, − for L2R, P2A, S2I; its size is the voxel size;
- an **origin** o: the coordinate of voxel 0 along the axis's direction, in AFNI's DICOM
  coordinates (x increases to the left, y to posterior, z to superior: NIfTI's x and y negated).

The coordinate of index n along an axis is `o + n·Δ` (float32), whatever the dataset's
obliquity: AFNI's grid is the **cardinal** grid of the voxel-to-world matrix, the obliquity is
ignored by every computation of this spec (it matters only for the header written, §10.1). From
the matrix (the sform when its code is positive, else the qform, else the voxel sizes on the
diagonal with orientation L2R, P2A, I2S and origin 0; `AFNI_NIFTI_PRIORITY` as in 3dTshift §7.1):
- the orientation of axis n: the signed permutation nearest to the matrix's rotation (the NIfTI
  reference library's orientation of the sform, or the nearest permutation of the qform's
  rotation: the one maximising the trace after orthonormalising the columns);
- the size of the step: the column's length (sform; computed in double, rounded to float32), or
  `pixdim[n]` (qform); metres and micrometres converted as 3dTshift §6.1 says;
- the origin: the matrix's offset in the row of the DICOM coordinate the axis runs along, with
  that coordinate's DICOM sign (negated for x and y).

**Handedness** h = +1 if the 3 × 3 matrix whose column n is the DICOM unit vector of axis n's
orientation (R2L +x, L2R −x, P2A −y, A2P +y, I2S +z, S2I −z) has a positive determinant, else −1.

**Axis codes.** For a letter L among R, L, A, P, I, S: `code(L) = +n` if axis n (1, 2, 3) runs
**from** L (L is the first letter of its orientation: R for R2L), `−n` if it runs **to** L.

The **conversion matrix** P (dataset axes to DICOM order) has, in column n, ±1 in the row of the
DICOM coordinate of axis n: +1 for R2L, A2P, I2S, −1 for L2R, P2A, S2I. A vector u in dataset
axis order (mm along each axis's direction) is `P·u` in DICOM order; a matrix M acting in DICOM
order acts as `Pᵀ·M·P` in dataset order.

**The centre of a dataset** (DICOM, float32): with `a = o + 0`, `b = o + (n − 1)·Δ` per axis
(each in float32: `(n−1)·Δ` a float32 product), both converted to DICOM order by P, the centre is
`0.5f·(a + b)` (float32). The **axis centre** used for displacements (§8.10) is instead
`o + 0.5·(n − 1)·Δ` evaluated in double and rounded to float32, per axis, in dataset order.

### 4.2 The voxel grid of a rotation

All resampling is done in index space about the **grid centre** `c_n = (N_n − 1)/2` of the
(padded) volume, `N_n` voxels along axis n. A transformation is given in mm along the dataset
axes, `x ↦ Q·x + d` (Q a 3 × 3 rotation, d a shift), about the centre; it is converted to index
units by `d'_n = d_n / δ_n` and `Q'_{mn} = Q_{mn}·δ_n/δ_m` (for m ≠ n; the diagonal unchanged),
with `δ_n = |Δ_n|` (1 where 0), all in double. The resampled volume `v'` satisfies
`v'(Q'·(p − c) + c + d') = v(p)`: the **content moves** by the transformation (it is not a
pull-back).

### 4.3 Elementary rotations

For an angle θ (radians) and an axis a ∈ {0, 1, 2}, with c = cos θ, s = sin θ (double):

    E_0(θ) = [[1, 0, 0], [0, c, s], [0, −s, c]]
    E_1(θ) = [[c, 0, −s], [0, 1, 0], [s, 0, c]]
    E_2(θ) = [[c, s, 0], [−s, c, 0], [0, 0, 1]]

(the usual right-handed rotation by −θ). The **rotation of three angles** about axes a1, a2, a3
is `R(a1, θ1; a2, θ2; a3, θ3) = E_{a3}(θ3)·E_{a2}(θ2)·E_{a1}(θ1)`, the products in double in
that order (first `E_{a2}·E_{a1}`, then `E_{a3}·(…)`), each entry of a product
`A_i0·B_0j + A_i1·B_1j + A_i2·B_2j` summed left to right.

With `reproduce`, the sine and cosine are glibc 2.35's (`specs/glibc-dbl.md`); the pair for the
**third** angle comes from glibc's combined `sincos`, the first two from separate `sin` and
`cos` (AFNI's build merged only that pair; glibc's `sincos` can differ from its `sin` in the last
bit). By default they are correctly rounded (`larmorx_core::math`). This is GLIBC-07, never
named here (an iterative fit, the user's decision of 2026-11-04).

### 4.4 The six parameters

**Internal parameters** (what the fit estimates): three angles θ1, θ2, θ3 (degrees) about the
dataset axes `a1 = |code(I)| − 1`, `a2 = |code(R)| − 1`, `a3 = |code(A)| − 1` (roll about the
I–S axis, pitch about R–L, yaw about A–P), and three shifts δx, δy, δz (mm) along the dataset
axes i, j, k. The transformation they stand for moves the content of the volume (§4.2) by

    Q = R(a1, −θ1·π/180; a2, −θ2·π/180; a3, −θ3·π/180),   d = (δx, δy, δz),

the angle in radians rounded to float32 before its negation (π/180 in double, the product of the
float32 degrees and π/180 in double, then rounded).

**Reported parameters** (`-1Dfile`, `-dfile`, `-rotcom`, the matrices):
- `roll = θ1·σ1`, `pitch = θ2·σ2`, `yaw = θ3·σ3`, with `σ1 = sign(code(I))·h`, `σ2 =
  sign(code(R))·h`, `σ3 = sign(code(A))·h` (degrees; a sign change of a zero gives −0);
- `dS`, `dL`, `dP` (mm): for each dataset axis n with internal shift δ_n: R2L gives `dL = δ_n`,
  L2R `dL = −δ_n`, P2A `dP = −δ_n`, A2P `dP = δ_n`, I2S `dS = δ_n`, S2I `dS = −δ_n` (again −0 for
  a negated zero).

So each angle is counter-clockwise about its axis looking from the positive end, in DICOM
coordinates (help: "degrees CCW"), and the shifts are along S, L and P. The help's reading: the
parameters bring the volume back into alignment with the base; resampling the input with
`3drotate -rotate <roll>I <pitch>R <yaw>A -ashift <dS>S <dL>L <dP>P` (shifts after the
rotation, about the grid centre) gives the registered volume.

**(observed)** The base volume's parameters are 0, so with an axis whose sign is negative they
are printed `-0.0000`: a dataset stored RAS (orientation L2R, P2A, I2S) gives
`  0.0000  -0.0000  -0.0000   0.0000  -0.0000  -0.0000` for the base's `-1Dfile` line; LPS-stored
(R2L, A2P, I2S) gives all `0.0000`.

### 4.5 The matrices

For each volume, from the reported parameters (float32):

- **R** = `R(2, −roll·π/180; 0, −pitch·π/180; 1, −yaw·π/180)` in DICOM order (x, y, z = 0, 1,
  2), the angles in double (`−(π/180)·roll` with π/180 in double, not rounded to float32), with
  §4.3's sine and cosine (here too the third pair from `sincos` with `reproduce`);
- **t** = `(dL, dP, dS)` (float32);
- the **stored matrix** (AFNI's `VOLREG_MATVEC` attribute, which larmorx does not write, §10.1):
  the 12 float32 numbers `R00 R01 R02 t0 R10 R11 R12 t1 R20 R21 R22 t2` (R rounded to float32).

**`-1Dmatrix_save`** (base to input, DICOM, the convention of 3dAllineate): with `cb`, `ci` the
centres (§4.1) of the base dataset and of the input (equal for an internal base):
1. `w = R·ci` computed in double (R double, ci float32, each entry `R_i0·ci_0 + R_i1·ci_1 +
   R_i2·ci_2` left to right) and **rounded to float32**;
2. `v_i = t_i + (cb_i − w_i)` in **float32** (the difference first, then the sum);
3. the inverse of `(R, v)`: `R⁻¹` by cofactors (below) in double, and `u = −(R⁻¹·v)` (the
   product in double, left to right as in 1., then negated);
4. one line: `R⁻¹_00 R⁻¹_01 R⁻¹_02 u_0 R⁻¹_10 … u_2`, each printed with C's `%13.6g`, separated by
   one blank, after a first line `# 3dvolreg matrices (DICOM-to-DICOM, row-by-row):`.

The inverse: `det = A00·A11·A22 − A00·A12·A21 − A10·A01·A22 + A10·A02·A21 + A20·A01·A12 −
A20·A02·A11` (left to right), `r = 1/det`, and

    inv00 = (A11·A22 − A12·A21)·r     inv01 = (−A01·A22 + A02·A21)·r    inv02 = (A01·A12 − A02·A11)·r
    inv10 = (−A10·A22 + A12·A20)·r    inv11 = (A00·A22 − A02·A20)·r     inv12 = (−A00·A12 + A02·A10)·r
    inv20 = (A10·A21 − A11·A20)·r     inv21 = (−A00·A21 + A01·A20)·r    inv22 = (A00·A11 − A01·A10)·r

(the unary minus applies to the first product). The signs of zeros follow this arithmetic:
**(observed)** the base volume's line is `1 0 0 -0 0 1 0 -0 0 0 1 -0` (in `%13.6g` fields): its
translations are −0.

## 5. Rotating and shifting a volume

Every resampling of this program (the derivative images, each iteration, the output, the coarse
search, `-rotparent`) applies one rigid transformation to a float32 volume by **four shears**,
each a set of one-dimensional shifts of the volume's rows, after an optional 180° flip. The
order of the shears and the interpolation of each row decide the result; §11 says what an
implementation may change.

### 5.1 The transformation

Two ways in:
- **from angles** (the fit, the output): angles θ1, θ2, θ3 (radians, float32, about the axes of
  §4.4) and a shift d (mm, float32): `Q = R(a1, −θ1; a2, −θ2; a3, −θ3)` (the negation exact; the
  float32 angles converted to double), `d` as given (the shift is applied after the rotation).
  If all three angles are exactly 0, they are replaced by `θ1 = 1e-6`, `θ2 = 1.1e-6`, `θ3 =
  0.9e-6` radians (float32 constants) first: **a pure shift is always done as a tiny rotation**;
- **from a matrix** (the coarse search, `-rotparent`): a 3 × 3 matrix M and a shift d (double,
  dataset axes, mm). Q is the orthogonal matrix nearest to M: `Q = U·Vᵀ` from the singular value
  decomposition `M = U·D·Vᵀ` (in double; its last bits depend on the SVD's arithmetic). No
  nudge.

The volume must have at least 2 voxels along every axis; otherwise it is returned unchanged.

### 5.2 The 180° flip

If the trace of Q is below 1: let `t` be the axis of the largest diagonal entry (the first in
order 0, 1, 2 on ties) and `p < q` the other two. If `Q_pp + Q_qq < −0.02`, the volume's content
is first flipped about the grid centre along p and q (index `i ↦ N − 1 − i` on both axes), and
Q is replaced by `Q·F` (F = diag with −1 at p and q, 1 at t; product in double). d is unchanged.

### 5.3 The four shears

After the flip, Q and d are converted to index units (§4.2): `Q'`, `d'`. The decomposition is
found for each of the six axis orders `(o1, o2, o3)` = (0, 1, 2), (0, 2, 1), (1, 0, 2), (1, 2, 0),
(2, 0, 1), (2, 1, 0), in this order:

1. Permute: `q_{mn} = Q'_{o_m o_n}`, `e_m = d'_{o_m}`.
2. Find the shears `X1`, `Y`, `Z`, `X2` (in the permuted frame) with

       q = X2·Z·Y·X1,   X1 = [[1, b1, c1], [0, 1, 0], [0, 0, 1]],
       Y = [[1, 0, 0], [ay, f, cy], [0, 0, 1]],   Z = [[1, 0, 0], [0, 1, 0], [az, bz, f]],
       X2 = [[f, b2, c2], [0, 1, 0], [0, 0, 1]],

   and shifts: none for X1, `sy` for Y, `sz` for Z, `sx` for X2, so that applying X1, then Y
   plus `sy`, then Z plus `sz`, then X2 plus `sx`, gives `p ↦ q·p + e`. With `D = det q` (q's
   determinant), the unique solution is

       f  = D^(1/3)            ay = q10                 b1 = (q11 − f)/q10
       az = (q11·q20 − q10·q21)/f                      bz = (q20 − az)/q10
       c1 = (q22 − f − bz·q12)/az                       cy = q12 − q10·c1
       [b2, c2] solves  q10·b2 + q20·c2 = q00 − f,   q11·b2 + q21·c2 = q01 − f·b1
       sy = e1      sz = e2 − bz·e1      sx = e0 − b2·e1 − c2·e2

   (for a rotation `f = 1`). The decomposition **fails** (this order is not a candidate) when, in
   double arithmetic, `D = 0`, `q10 = 0` or `q11·q20 − q10·q21 = 0`. AFNI evaluates an
   algebraically identical closed form (a general solution in which `f` comes out of a cube root
   taken as `pow(·, 0.333333333333333)` of a product containing `D²`, so `f` differs from 1 by
   about 1e-14); its double bits are its own (see "Numerics" below).
3. Its **norm** is the largest of `|b1|, |c1|, |ay|, |cy|, |az|, |bz|` (the first three shears'
   coefficients only; X2's are not counted).
4. Map back: shear m acts along axis `o_{axis(m)}` with its coefficients moved to the original
   axes (`coefficient of permuted axis n` → `axis o_n`).

The order with the **smallest norm** is used (the first on ties; a failed order has norm 1e38).

**Shortcut.** Before step 1: if `trace(Q') ≥ 2.99999` and the sum of the six off-diagonal
`|Q'_{mn}|` divided by the trace is below 1e-6, the shears are pure shifts: X1 none, then a shift
of `d'_1` along axis 1, `d'_2` along axis 2, `d'_0` along axis 0 (in that order), all
coefficients 0.

**Retry.** If all six orders fail: `Q' ← Pᵀ·(Q'·P)` with
`P = R(0, 1.09e-6; 1, 1.22e-6; 2, 1.37e-6)` (§4.3, double), and the six orders are tried again.
If they fail again, AFNI prints `*** THD_rota_vol: can't compute shear transformation!` and the
volume is left as it was. A rotation about **one** axis only (two angles exactly 0, as for every
rotation derivative image of §7.1) fails in all six orders (its exact zeros make `q10` or the determinant term 0) and always takes the retry.

**Numerics (spec writer's measurements, 2026-11-07 and 2026-11-08).** Two things depend on how
the decomposition is evaluated.
1. *The tiny coefficients* of the retry (about 1e-6 to 1e-8, e.g. `c1` and `c2` nearly
   cancelling) differ between AFNI's closed form and the stable formulas above; both reconstruct
   `Q'` to about 1e-8, and the difference changes only last bits.
2. *Exact ties choose the order*, and this is what decides the results. When two axes have the
   same voxel size (as in every EPI with square in-plane voxels), orders come in pairs whose
   norms are **equal in exact arithmetic**: an entry of a rotation equals its own cofactor
   where its row's and its column's voxel sizes are equal, and in each pair the largest
   coefficient of one order is an entry of `Q'` (its `ay = q10`) while the other's is the matching
   cofactor (its `az`). For a rotation mainly in the plane of axes 0 and 1 (equal sizes) the
   orders (0, 1, 2) and (1, 2, 0) tie, and so do (0, 2, 1) and (1, 0, 2); for a rotation about
   one axis only (the rotation derivative images, after the retry) the four orders that shear in
   its plane tie. The computed norms then differ only by rounding (about 1e-16 relative,
   measured), so the evaluation's last bits choose the order, at **every** resampling of such a
   grid: the derivative images, each iteration of the fit, the output. Both choices are exact
   decompositions of the same transformation, but they shift the rows along different axes
   (x–y–x against y–x–y shears for a rotation about z) and so interpolate differently.

Measured with the model of §15 taking either side of every tie, against AFNI: on a 40 × 44 × 24
series with 3 × 3 × 3.5 mm voxels the roll derivative image's extremes move by 0.4 % to 1.4 %
with heptic (−24.80 against −24.69 for one choice) and 5.5 % with linear, the first fit's
coefficients by up to 4e-3 (heptic) and 4e-2 (linear) relative; through the iterations the
parameters move by at most 0.0005° with Fourier (fMRIPrep's call: 0.0004°), 0.0044° heptic,
0.0068° quintic, 0.013° cubic and **0.051° with `-linear`** (a roll of 2.4° in the series'
3 × 3 mm plane; 0.0098° on another phantom, 0.0035° and 0.0012° on fMRIPrep's inputs of ds003592
and ds000005), and single voxels of the registered volumes at their edges by up to 0.6 % of the
largest value even where the parameters agree. Linear interpolation's smoothing depends on the
fraction of each row's shift, so the axes it shifts along matter far more for it than for the
higher orders. **With AFNI's choice of order and the stable formulas' coefficients** (a scratch
check of the spec writer, with AFNI's evaluation of the norms: not kept), the model gives AFNI's
`-1Dfile` lines exactly on that `-linear` case: the order is what decides, not the coefficients.
AFNI's choice comes from its own rounding: in exact arithmetic on the same double `Q'` the tied
norms differ by less than that rounding and agree with AFNI's choice in only 75 % of the
derivative images measured, so no rule of this spec reproduces it. An implementation that does
not evaluate the norms as AFNI does cannot be bit-identical, and §14.3's thresholds allow for
either side of every tie. Every other statement of this spec was checked bit for bit by the model
of §15 with the decomposition's double values made AFNI's: then it reproduces AFNI's outputs
exactly (§15).

**AFNI's own spread under the ties** (spec writer, 2026-11-08, step 3). Measured with AFNI's
arithmetic and AFNI's choice at every resampling replaced by another choice (the first or the last
of the tied orders everywhere, the one after AFNI's, or a seeded draw at every resampling: 15
variants per case; a scratch measurement of the spec writer, not kept): with AFNI's own choices
it gives AFNI's outputs bit for bit, so the variants show how far AFNI's result moves when only
the ties are decided otherwise, the most a clean-room implementation can be expected to agree.
- **AFNI mixes the sides, resampling by resampling.** fMRIPrep's call on ds000256's high-motion
  run (`nofeedback`, 64 × 64 × 32 voxels of 4 mm: all three axes tie) meets 334 ties: 12 in the
  derivative images, 71 in the fits' iterations, 11 in the 20 output resamplings and 240 in the
  coarse search's rotations; AFNI takes the first of the tied orders in 190 of them and another
  in 144, both in every stage. No single side, taken everywhere, reproduces it.
- **Real data** (fMRIPrep's call on 16 runs: seven of §14.2's and the data panel's nine): the parameters move
  by at most 0.48 of their thresholds (ds000256 `nofeedback`, rotations to 4.7°; 0.06 or less on
  the others), the registered volumes' mean |diff| by up to 3.4e-3 of their largest value and NCC
  down to 0.99978 (`nofeedback`'s volume with 5.2° of rotation), fMRIPrep's median by up to 2.0e-2
  at single voxels (mean 9.0e-4). The volumes' differences grow with each volume's rotation: at
  most 6.8e-4 of the largest value per degree of `|roll| + |pitch| + |yaw|` (§14.3).
- **Random testing's phantoms** (small grids with sharp edges, larger motions): the fits can be
  very ill-conditioned with respect to the ties (the roll nearly collinear with the shifts on a
  small grid): over the variants, AFNI's parameters move by a median of 25 thresholds and up to
  256 in the sampled findings that converge beyond the thresholds, up to 782 in those that stop
  at their iteration limit, and whether a
  fit runs away (beyond 15° or 20 mm), and whether AFNI-97's crash happens on the way, changes
  with the choices too. §14.3's "The ties' spread" says how such cases are judged.
- **With the same choice the arithmetic agrees.** The clean-room original taking the first tied
  order everywhere (`LARMORX_VOLREG_TIES=0`, §14.3) against AFNI's arithmetic taking it too: every
  parameter within its threshold in 117 of 121 sampled random-testing findings (all 4 others
  runaways of hundreds of degrees): the remaining differences are the ties, not a behaviour this
  spec misses.

### 5.4 Zero padding (`-zpad n`)

If n > 0, the volume is padded with n zero voxels on both sides of every axis before the flip and
the shears, and the padding is removed afterwards (the grid centre is the same). The padded size
`N_n + 2n` is what §5.5's rows have. `-zpad 0` pads nothing. The padding is the same for every
resampling of the run (also the coarse search's and the derivative images'). The help's
"`AFNI_ROTA_ZPAD` can set a nonzero default" is wrong: the variable is never read (rule C, §12).

### 5.5 Applying a shear

A shear along axis x with coefficients `(a, b)` on the other two axes `(y, z)` (in index order:
for an x-shear `y = j`, `z = k`; for a y-shear `x = i`, `z = k`; for a z-shear `x = i`, `y = j`)
and shift `s` shifts the row through `(y, z)` by

    a0 = a·(y − cy) + b·(z − cz) + s

in **float32** (a, b, s rounded to float32; `cy = 0.5f·(Ny − 1)` float32; `(y − cy)` an int minus
a float; each product and sum in float32, left to right). The shear is **skipped** if
`st = |a|·cy' + |b|·cz' + |s|` (computed in double from the float32 values, rounded to float32;
`cy'`, `cz'` the half-extents `0.5·(N − 1)` of the two other axes) is below 1e-3.

Rows are processed **in pairs** (rows `2m` and `2m + 1` along the first of the two other axes,
for each value of the second): the pair's shifts are `a0` of row `2m` and `a1 = a0 + a` (float32)
for row `2m + 1`, **not** row `2m + 1`'s own formula (the two differ in the last bit; this decides
the result bit for bit, for every method). When the
count of rows along that axis is odd, the last row is shifted alone, with a "second shift" that
matters only to the Fourier method's large-shift rule (§5.6): for an x-shear it is `a0 + a` of
that row; for y- and z-shears it is the second shift of the previous pair in the same plane (0 if
there was none). Pairing changes no value for the polynomial methods (each row is shifted
independently); for Fourier both rows of a pair share one complex FFT (§5.6), which changes the
last bits.

### 5.6 Shifting a row

A row `f[0..n)` (float32) is shifted by `α` (float32): the output is `f(i − α)` interpolated, with
**zeros outside** `0..n−1`. Let `β = −α`, `ia = trunc(β)` minus 1 when `β < 0` (so an exact
negative integer gives `ia = β − 1`), `x = β − ia` (float32; in [0, 1]). If `ia ≤ −n` or `ia ≥ n`
the row becomes all zeros. Otherwise output `i` uses the samples `f[i + ia + k]`:

| Method | samples k | weights (x the float32 fraction) |
|---|---|---|
| NN | 0 | 1 (the sample at `i + ia`: the floor, not the nearest; an exact shift of +1 takes `f[i−2]`) |
| linear | 0, 1 | `w0 = 1 − x`, `w1 = x` (the first computed in double, rounded) |
| cubic | −1 … 2 | `x(1−x)(x−2)·0.1666667`, `(x+1)(x−1)(x−2)·0.5`, `x(x+1)(2−x)·0.5`, `x(x+1)(x−1)·0.1666667` |
| quintic | −2 … 3 | `x(x²−1)(2−x)(x−3)·0.008333333`, `x(x²−4)(x−1)(x−3)·0.041666667`, `(x²−4)(x²−1)(3−x)·0.083333333`, `x(x²−4)(x+1)(x−3)·0.083333333`, `x(x²−1)(x+2)(3−x)·0.041666667`, `x(x²−1)(x²−4)·0.008333333` |
| heptic | −3 … 4 | `x(x²−1)(x²−4)(x−3)(4−x)·0.0001984126984`, `x(x²−1)(x−2)(x²−9)(x−4)·0.001388888889`, `x(x−1)(x²−4)(x²−9)(4−x)·0.004166666667`, `(x²−1)(x²−4)(x²−9)(x−4)·0.006944444444`, `x(x+1)(x²−4)(x²−9)(4−x)·0.006944444444`, `x(x²−1)(x+2)(x²−9)(x−4)·0.004166666667`, `x(x²−1)(x²−4)(x+3)(4−x)·0.001388888889`, `x(x²−1)(x²−4)(x²−9)·0.0001984126984` |

**The weights' arithmetic:** `x²` is the float32 product `x·x`; every other operation is in
double, left to right exactly as written (the leading factor, then each parenthesis, the constant
last); the weight is then rounded to float32. (This also settles `specs/3dTshift.md` §5.2's open
point: its "unresolved" last-bit differences of the quintic and heptic weights come from `x²`
being a float32 product.)

**The sums.** Where every sample used lies inside the row (`i + ia + kmin ≥ 0` and
`i + ia + kmax ≤ n − 1`), the output is the float32 sum of float32 products; elsewhere the
products and the sum are in double (float32 weights, the samples as doubles, zeros outside) and
rounded at the end. The order of the terms: increasing k for linear, cubic and quintic; for
heptic k = −2, −1, 0, 1, 2, 3, then −3, then 4.

**Fourier** (`-Fourier`; both rows of a pair at once):
1. **Large shifts.** If both shifts of the pair are beyond the row (`α < −n` or `α > n`, for each
   of the two), both rows become zeros (for a lone row, see AFNI-97, §12). Otherwise:
2. **Length.** `L` = the smallest number ≥ `trunc(n + 0.5·st)` (n the padded row length, st the
   shear's extent of §5.5, the sum in double) of the form `2^p·3^q·5^r` with p ≥ 1 and q, r ∈
   {0, 1}.
3. **Transform.** The two rows as one complex row (`f + i·g`; `g = 0` for a lone row),
   zero-padded to L, forward DFT `X_k = Σ_m x_m·e^(−2πi·mk/L)` (float32 arithmetic, AFNI's own
   FFT).
4. **Separate** the two spectra (each twice its true size): `F_0 = (2·Re X_0, 0)`,
   `G_0 = (2·Im X_0, 0)`; for `1 ≤ k < L/2`: `F_k = (Re X_k + Re X_{L−k}, Im X_k − Im X_{L−k})`,
   `G_k = (Im X_k + Im X_{L−k}, −Re X_k + Re X_{L−k})`; `F_{L/2} = (2·Re X_{L/2}, 0)`,
   `G_{L/2} = (2·Im X_{L/2}, 0)` (float32).
5. **Phase.** `Δk = 2π/L` (double, rounded to float32); `φ = −α·Δk` (float32) for each row;
   `e = (cos φ, sin φ)` (double functions of the float32 φ, rounded to float32; with `reproduce`
   glibc's `sincos`); with `p_0 = (1, 0)`, for k = 1 … L/2: `p_k = e·p_{k−1}`, then
   `F_k ← p_k·F_k` (complex products in float32: `(ar·br − ai·bi, ar·bi + ai·br)`, the
   accumulated factor first). Then `Im F_{L/2} = 0`.
6. **Rejoin** and inverse: `X_0 = (Re F_0, Re G_0)`; for `1 ≤ k < L/2`:
   `X_k = (Re F_k − Im G_k, Im F_k + Re G_k)`, `X_{L−k} = (Re F_k + Im G_k, −Im F_k + Re G_k)`;
   `X_{L/2} = (Re F_{L/2}, Re G_{L/2})`; the unnormalised inverse DFT (`e^(+2πi·mk/L)`); then
   `f_m = (0.5/L)·Re x_m`, `g_m = (0.5/L)·Im x_m` (the factor in float32) for m < n.

The result is a circular shift within the padded length L (content shifted past the row's end
enters the padding, and beyond L wraps around).

### 5.7 What a rotation costs, and where the bits come from

Each resampling is one flip (rarely), four passes over all rows of the padded volume, and nothing
else. The decomposition (double), the row shifts (float32 as above) and, for Fourier, the FFT
decide the bits. The polynomial methods are exactly reproducible from this section; Fourier's
last bits depend on the FFT's arithmetic (§11).

## 6. Blurs

### 6.1 The FIR Gaussian

A Gaussian blur of standard deviation σ (mm) along an axis of step δ (mm) is a symmetric FIR
filter of half-width m (taps `w_0 … w_m`) applied to every row along that axis, in place, x then
y then z.
- **Half-width:** `m = ceil(F·σ/δ)` (double), with `F = 2.5`; `AFNI_BLUR_FIRFAC` sets F (a value
  ≤ 0 keeps 2.5 for the program's main blur, a value below 2 keeps 2.5 for the coarse search's
  blur, §6.3). `m < 1` becomes 1.
- **Taps:** with `u = δ/σ` (float32 quotient): for each `i = 0 … m`,
  `s_i = Σ_{j=−11}^{11} c_j·exp(−0.5·x_ij²)`, `x_ij = fl32(i·u) + j·u/22` (the first term the
  float32 product of the integer and u, the rest in double; `u/22` is `u/(2.0·11)` in double),
  `c_j = 0.5` for `j = ±11`, else 1; the sums in double, in increasing j; `w_i = s_i` rounded to
  float32. The normaliser `N` is a float32 accumulated as `N = fl32(N + s_0)`, then
  `N = fl32(N + 2·s_i)` for i ≥ 1 (each sum in double, rounded); then `r = 1/N` in float32 and
  `w_i ← w_i·r` (float32). With `AFNI_BLUR_FIROLD` yes: `w_0 = 1`, `w_i = exp(−0.5·x²)` with
  `x = fl32(i·u)`, `N = 1 + Σ 2·w_i` (float32 additions), normalised the same way.
- **A row is blurred only if its length is at least m + 1**; a shorter axis is left unblurred
  (also for the main weight's m = 8 on a volume of fewer than 9 slices).
- **Row ends:** the row `r[0..n)` is extended by m values on each side. **At the start the
  extension mirrors about the first sample** (`r[−q] = r[q]`); **at the end it mirrors about
  the point half a sample past the last** (`r[n−1+q] = r[n−q]`: `r[n] = r[n−1]`), for the x and y
  axes; the z axis mirrors about both end samples (`r[n−1+q] = r[n−1−q]`). When the volume is at
  least 512 voxels wide (nx ≥ 512), the y blur instead extends by one mirrored sample at each end
  (`r[−1] = r[1]`, `r[n] = r[n−2]`) and zeros beyond. These are AFNI's (rule C, measured: §12).
- **Sums (float32):** for `m ≤ 7`: `w_m·(r[i−m] + r[i+m]) + w_{m−1}·(…) + … + w_1·(r[i−1] +
  r[i+1]) + w_0·r[i]`, evaluated left to right (outermost pair first, the centre last); for
  `m > 7`: `w_0·r[i]`, then `+ w_q·(r[i−q] + r[i+q])` for `q = 1 … m`. Each pair is summed before
  its product.

### 6.2 The FFT Gaussian

An axis whose half-width exceeds 35 is blurred in the frequency domain instead (in this
program's blurs every axis has the same `σ/δ`, so all three or none): length `L = ` §5.6's form ≥ `n + trunc(3σ/δ)`, the rows taken two at a time as
the real and imaginary parts of one complex row (a lone last row with zero imaginary part),
zero-padded, forward DFT, each bin `k` (and `L − k`) multiplied by
`g_k = (1/L)·exp(−σ²·(2πk/(L·δ))²/2)` for `k = 0 … L/2` (float32 factors; the exponent's argument
in float32: `σ²·0.5` and `k·dk` with `dk = 2π/(L·δ)` float32, the exponential in double), inverse
DFT, the first n values kept; when any axis was blurred this way, every value is finally
**clipped to the range of the volume before the blur**. (Reached only by `-twoblur` above 14.)

### 6.3 Where the blurs are used

| Use | σ | δ | method |
|---|---|---|---|
| the default weight (§7.1) and `-wtinp`'s weight | 3·δ_n per axis (the product in double, rounded to float32) | the voxel size (mm) | FIR (m = 8) |
| `-twopass`: the base, each volume, the first-pass weight (§8.5) | `twoblur` | 1 (voxel units) | FIR for twoblur ≤ 14, else FFT |
| the coarse rotation search (§8.6): its weight, and the subsampled search's base and volume | 1 | 1 | FIR only, `m` also capped at `n/2` (integer division) per axis; an axis of 1 voxel is skipped |

**(observed)** The first blur of a run prints, once, on standard error:
`++ edt_blur: sigx <σ>, dx <δ>, fwhm <σ/0.4246609>, sfac <F>, firx <m>` (`%g` for the numbers,
the x axis's values; e.g. `++ edt_blur: sigx 9, dx 3, fwhm 21.1934, sfac 2.5, firx 8` for 3 mm
voxels without `-twopass`, `++ edt_blur: sigx 2, dx 1, fwhm 4.70964, sfac 2.5, firx 5` with it).
The coarse search's blur never prints it.

## 7. The registration of one volume

### 7.1 The basis (the base, its weight and its derivative images)

Set up once per pass from a float32 base volume `b` (voxel sizes `δ` = the input's) and an
optional weight volume.

**The weight w:**
1. *Default* (no weight given): `|b|`, blurred by §6.3's first line, then every value below
   `0.025·max` set to 0 (the maximum as a double, the product in double, the threshold rounded
   to float32; a value equal to it stays).
2. *Given* (`-weight`, or the first pass's weight of §8.5): used as given: no absolute value, no
   blur, no clip (negative weights included).
3. *`-wtinp`*: the input's volume 0 (scaled to float32), processed as the default (absolute
   value, blur, 2.5 % clip).
4. A given weight whose grid differs from the base's prints `*** WARNING: in
   mri_3dalign_setup, weight image mismatch!` and the default is used (unreachable from the
   command line, which checks it before).

**Edging** (only for the default weight, and the first pass's weight of §8.5): with the edge
widths `ex, ey, ez` (§8.3), every voxel within `ez` planes of either z face, `ex` of either x
face and `ey` of either y face gets weight 0. (`-weight` and `-wtinp` get no edging.)

**Trimming** (always on: `-wtrim` only prints its notice): the box of the weight:
1. the voxels with a non-zero weight; the largest 6-connected cluster of them; one peel and
   restore with 18 neighbours and threshold 17 (`specs/3dAutomask.md` §5.3 with `p = 1`, edge
   voxels duplicated); the largest cluster again (§5.2's rule);
2. the box = the first and last planes along each axis holding a voxel of that cluster;
3. if the cluster is empty, or the box is the single voxel (0, 0, 0) (all its upper bounds 0),
   AFNI prints ` + skipping empty trim, {xyz}{a,b} = xa,xb, ya,yb, za,zb` (the bounds; 0s for an
   all-zero weight) and nothing is trimmed;
4. else, with `T = 100·box volume/volume` (in float32), the box is used if `T < 90`; otherwise
   AFNI prints ` + skipping use of trim - too little savings` (always, not only when verbose)
   and nothing is trimmed.

When trimmed, the base, the six derivative images and the weight are cut to the box, and every
volume to be fitted is cut to it after each resampling: **the fit uses only the voxels in the
box**, so a non-zero weight outside the largest cluster's box is ignored.

If every weight is 0, AFNI prints `** error: weights are all zero` and continues (the least
squares then fail, §7.3).

**Derivative images** (with the method of the pass, §8.7 and §8.5, all on the untrimmed base,
then trimmed), for the six parameters in the order θ1, θ2, θ3, δx, δy, δz:
- rotations: `ε = 2·dfac/(nx + ny + nz)` radians (double, rounded to float32; `dfac` = `-delta`'s
  value, ×`twoblur` in the first pass), `p` = the base rotated by `+ε` about that parameter's axis
  (the other angles 0, no shift; §5.1 "from angles"), `q` = rotated by `−ε`;
  `D = κ·(q − p)` with `κ = sinit·0.5·(π/180)/ε` (double, rounded to float32), the difference and
  the product in float32;
- shifts along axis n: `ε = dfac·δ_n` (float32), `p` = the base shifted by `+ε` (all angles 0, so
  §5.1's nudge makes it a tiny rotation too), `q` by `−ε`, `κ = sinit·0.5/ε` (float32), `D = κ·(q −
  p)`.

`sinit` is 1, or the scale of §8.5 once set (it then stays for the rest of the run, the final
pass included). So `D_r` is minus the derivative of the rotated base with respect to the angle
in degrees (times sinit), `D_s` minus the derivative with respect to the shift in mm.

**The least squares** have seven reference vectors: `b` and `D_1 … D_6` (trimmed, as vectors over
the box's voxels in storage order) and the weight w:
1. **Check:** a reference vector that is all zeros is refused: AFNI prints
   `** ERROR: lsqfit :: <k> vector<s> are all 0 for data fitting` (`vector` for k = 1), then
   `** ERROR: mri_startup_lsqfit: bad call to startup_lsqfit!` (with a bell, `\a`, before the
   line end) and the basis cannot fit (§7.3).
2. **Normal matrix:** `C_ij = Σ_v r_i[v]·r_j[v]·w[v]` for `i ≤ j` (the product `(r_i·r_j)·w` in
   float32, the sum in double, in storage order), symmetric.
3. **Cholesky:** for row `i = 0 … 6`: `L_ij = (C_ij − Σ_{k<j} L_ik·L_jk)/L_jj` for j < i, then
   `s = C_ii − Σ_{k<i} L_ik²` (double, k increasing); if `s ≤ 0`, AFNI prints `** ERROR: Choleski
   factorization failure in startup_lsqfit [row=<i> sum=<s %g>]` and then the "bad call" line of
   1.; else `L_ii = √s`.
4. **The reference vectors are then multiplied by the weight**, voxel by voxel, in float32
   (`r_i ← r_i·w`): the right-hand sides use these.

**Fitting a volume** `y` (float32, trimmed): `ρ_i = Σ_v r_i[v]·y[v]` (the float32 product of the
weighted reference and the voxel, summed in double in storage order), forward and backward
substitution with L in double, the seven coefficients rounded to float32: `(c0, c1 … c6)`, the
scale `c0` and the six parameter increments (degrees and mm).

### 7.2 Registering one volume

Given the basis, an input volume `y` (float32; its voxel sizes `δ`), the thresholds `x_t`
(voxels) and `φ_t` (degrees), the iteration limit `M`, and optional initial values:
1. The shift thresholds in mm: `t_n = δ_n·x_t` (float32; `x_t` itself where `δ_n` is 0).
2. **Start.** With non-zero initial values (any of the six ≠ 0): the parameters `(θ, δ)` = the
   initial values, and iterate at least once. Otherwise: the **first fit** = the least squares
   of the unresampled `y` (trimmed); the parameters = its six coefficients; iterate if any
   `10·|c_k|` exceeds its threshold (`10·|c_{1..3}| > φ_t`, `10·|c_{4..6}| > t_n`; double
   comparisons). Note that without initial values the first fit is accepted as the result when
   all six are below a tenth of their thresholds.
3. **Iterate:** resample `y` by the current parameters (§5.1 "from angles", the angles
   `θ·(π/180)` rounded to float32; the method of the pass), trim, fit: the increments
   `(c1 … c6)` are **added** to the parameters (float32); continue while the number of
   iterations is below M and any `|increment|` exceeds its threshold (`|c_{1..3}| > φ_t`,
   `|c_{4..6}| > t_n`). So at most M resamplings and fits after the start.
4. **Result:** the angles `θ·(π/180)` (float32, radians) and shifts (float32 mm).
5. **Output** (the final pass, unless `-rotparent`): `y` resampled by the result with the final
   method (§3); if the clipping is on and the final method is cubic, quintic, heptic or Fourier
   (not linear or NN), every value is clipped to `[min y, max y]` (the volume's own range, as
   doubles compared with each float32).

The scale `c0` is never used (only printed with `-verbose -verbose`).

### 7.3 Failures of the basis

- **A volume that is not 3D** (one slice, `nz = 1`): the basis cannot be built: AFNI prints
  `\n*** mri_3dalign_setup: cannot use nD images!\a` and 3dvolreg `** Can't initialize base image
  for alignment` (final pass) or `** Can't initialize first pass alignment algorithm` (first
  pass), exit status 1. With `-twopass` and the coarse rotation search, each of its 44 rotations
  first prints `\n*** THD_rota3D_matvec: non-3D image input!` (and its sums cover no voxel).
- **A basis whose least squares failed** (§7.1 step 1: a reference vector that is 0 at every
  voxel of the fit's box; or step 3: a pivot `s ≤ 0`): the setup's messages are printed, and
  AFNI dies at the first volume it fits ("Fatal Signal 11 (SIGSEGV)", its crash banner, exit
  status 1): AFNI-89 (§12).
- **Which inputs reach it** (spec writer, 2026-11-08; **(observed)**,
  `validation/investigations/3dvolreg-spec/afni89.py`). Whether the least squares fail is decided
  by the arithmetic of §5–§7 alone (exact zeros, a non-positive pivot), so an implementation that
  follows it meets the failure on the same inputs; there is no simpler criterion, because an
  exact zero depends on the float32 rounding of the interpolation (below).
  - **Every weight in the box 0** (an all-zero `-weight`, with or without `-twopass`):
    ` + skipping empty trim, {xyz}{a,b} = 0,0, 0,0, 0,0`, `** error: weights are all zero`,
    `** ERROR: Choleski factorization failure in startup_lsqfit [row=0 sum=0]`, the "bad call"
    line, the crash.
  - **With `-twopass`, a base that does not vary along an axis** (each row along it one value),
    for the first pass's basis: the two images of its shift derivative along that axis (the
    blurred base shifted by `±twoblur·dfac` voxels, 1.4 by default, by linear interpolation,
    §7.1) differ from the blurred base only in the `twoblur·dfac` voxels (rounded up: 2) nearest
    each face of the axis, where the padding's zeros come in, so the derivative image (their
    scaled difference) is 0 elsewhere if the interpolation gives the blurred base's value back
    exactly (float32, §5.6). When the edging along that axis (§8.3) is at least that wide (with
    the default 5 %, `trunc(0.05·N + 0.5) ≥ 2`: N ≥ 30 voxels) and the value comes back exactly
    at every other voxel of the box, the image is 0 over the box: `** ERROR: lsqfit :: <k> vector<s> are
    all 0 for data fitting`, one per such axis, then the "bad call" line and the crash. A
    constant 40 × 44 × 24 series (value 100) gives `2 vectors` (x and y; its 24 slices have an
    edging of 1), 30 × 30 × 30 gives 3, 30 × 29 × 29 and 29 × 29 × 30 give 1; so do the
    values 37.3 and 7·10⁵ (2) and 1 (5: its rotation images are 0 over the box as well), the
    40 × 44 × 24 series whose lower half of slices is 100 and upper half 0 (2), `-cubic` (the
    first pass is always linear) and `-delta 0.3` (shifts of 0.6 voxel reach one plane: 3).
  - **Not reached**, AFNI ends with exit status 0 and parameters that fit only the few voxels
    near the faces (meaningless): when every axis's edging is narrower than the shifts' reach
    (a constant 29 × 29 × 29 series: largest parameter 0.0016; 20 × 22 × 12: 569 mm;
    40 × 44 × 24 with `-edging 1`: 117; with `-twoblur 3`, shifts of 2.1 voxels: 8·10⁶), and
    when the interpolation does not give the value back exactly everywhere (the constants 1000
    and 123.456 over 40 × 44 × 24: 4·10⁵ and 7·10⁵; a base constant along x but varying along y
    and z: 4·10⁵).
  - **Without `-twopass`** a constant series registers (the default heptic's shifts of 0.7 voxel
    reach 4 planes, more than the 5 % edging; every parameter 0).

### 7.4 Volumes that are not registered

- The base volume of an internal base (`-base n`) is not registered: its parameters are 0 and its
  output is the base itself (as float32, then stored as §10.1 says).
- A volume with fewer than 66 non-zero voxels (as float32) is not registered: AFNI prints
  `*+ WARNING: skipped registration for #<k> because image is empty-ish`, its parameters are 0
  and its output is the volume unchanged (`-twopass`: its first-pass parameters are 0 too, without
  the warning).

## 8. The program

After the command line (§9), in this order.

### 8.1 The output's grid, the output name, `-rotparent`

1. The output dataset is a copy of the input's header. With `-gridparent` (§8.9) it may be
   zero-padded along the slice axis.
2. `-prefix NULL`: `*+ WARNING: No output dataset will be calculated`; nothing is resampled for
   output (the parameters are still computed and written). Otherwise, if `AFNI_DECONFLICT`
   starts with `N` and the output exists: `** FATAL ERROR: Output file <name> already exists --
   cannot continue!` (and a blank line), exit status 1, before any work (else an existing output
   is found only when writing: 3dTshift §6.3, AFNI-11).
3. `-rotparent`: the extra transformation (§8.9).

### 8.2 Reading the input, `-tshift`, the base

- With `-verbose`: `++ Time shifting input dataset <name>` (with `-tshift`) or `++ Reading input
  dataset <name>`, `<name>` as AFNI shows a dataset's file (its real path, unless
  `AFNI_NOREALPATH` is yes or the file is a symbolic link; a qform or sform of code 2 adds the
  directory, `./` when there is none: §9.3).
- **`-tshift ii`** (when its checks of §9.2 passed): the input is slice-time corrected **in
  place** before anything reads it (the base, the `-maxdisp` mask, `-wtinp`'s weight and both
  passes use the corrected data): 3dTshift's default processing (`specs/3dTshift.md` §4 with the
  default restore, §5.1 Fourier; the FFT length from the whole series' `nt + 4`) with
  `-ignore ii`, the time origin the mean of the slice offsets (their float32 sum in slice order
  divided by nz, in float32), the TR and offsets in the dataset's time units. It fails (and the
  series stays unshifted, with `++ WARNING: some error during -tshift operation!` on standard
  error) when the offsets are not all within `[0, TR]` or are all equal, or the TR is 0. On
  success the output has no slice offsets and a time origin of 0. AFNI-01 (a negative brick
  factor) applies on this path as in 3dTshift.
- **The base**: an internal base is volume n of the (shifted) input, scaled to float32
  (`factor·x` in float32 when the volume's factor is neither 0 nor 1, negative factors included);
  an external base is volume 0 of the `-base` dataset, scaled likewise, read during the scan
  (§9.1). If it has fewer than 100 non-zero voxels: `** FATAL ERROR: 3dvolreg fails: base image
  has <k> nonzero voxel<s> (< 100)` (`voxel` for k = 1), exit status 1.
- Each input volume is likewise scaled to float32 when it is used.

### 8.3 Edging

From `-edging e` (or `AFNI_VOLREG_EDGING`, or 5 %): as a percentage `p`: `e_n = trunc(0.01·p·N_n
+ 0.5)` (double); as a voxel count `e`: `e_n = trunc(min(0.25·N_n, e))` (double), per axis
(N_n the input's size). With `-verbose`: `++ Edging: x=<ex> y=<ey> z=<ez>`.

### 8.4 The `-maxdisp` mask (on by default)

With `-verbose`: `++ Creating mask for -maxdisp`. The automask of the (shifted) input:
`specs/3dAutomask.md` §3–§5 (the average of the absolute values over every volume, with their
brick factors as 3dAutomask's §3 applies them) with **clip fraction 0.333**, no dilation or
erosion. With `-verbose`: ` + Automask has <k> voxels`. Its **edge voxels** are kept: a mask
voxel is an edge voxel if it lies on a face of the grid or one of its six face neighbours is
outside the mask. With `-verbose`: ` + <k> voxels left in -maxdisp mask after erosion`. Their
positions are `(o + i·Δ)` per axis (float32, dataset order, §4.1). If no voxel is left, nothing
of `-maxdisp` is computed or printed (and `-maxdisp1D`'s files are not written). (`*+ WARNING:
Can't create -maxdisp mask?!` if the mask cannot be made at all.)

The 0.333 clip fraction then **stays set for the rest of the run**: the coarse rotation search's
automask (§8.6) uses it too, and 0.5 with `-nomaxdisp` (AFNI-92, §12).

### 8.5 `-twopass`: the first pass

With `-verbose`: `++ Start of first pass alignment on all sub-bricks`.

1. `B1` = the base, blurred by `twoblur` (§6.3, voxel units).
2. The fit's settings for this pass: thresholds `twoblur·x_t` and `twoblur·φ_t`, derivative step
   `twoblur·dfac` (float32 products), the iteration limit M (§9.1: `-twopass` raises an M below
   10 to 66), the **linear** method, no output.
3. **The first pass's weight** (unless `-weight` was given, in which case the basis is `B1` with
   that weight):
   - With `-verbose`: `++ Computing first pass weight as sum of base and brick`.
   - `V0` = the input's volume 0, scaled to float32, blurred by `twoblur`.
   - The coarse search of `V0` against `B1` (§8.6) gives a rotation `(r, p, y)` and a shift `(sx,
     sy, sz)` (voxels); without a coarse search (`-coarse` with del ≤ 0 or num ≤ 0) all are 0.
     With `-verbose`: `++ Getting best coarse rot+shift [0]:` and the search's marks, then
     ` <r> <p> <y> : <sx> <sy> <sz>` (`%g`, `%d`); with `-nocoarserot` `++ Getting best coarse
     shift [0]:` and ` <sx> <sy> <sz>`. The angles are then multiplied by the signs `σ` of §4.4
     (they come out of the search in the reported convention).
   - The box `|sx| ≤ i < N_x − |sx|` (likewise y, z).
   - **Scale** (unless `-sinit`): `f` = the float32 sum over the box (storage order) of
     `V0[i − sx, j − sy, k − sz]`, `g` = that of `B1[i, j, k]`; if `g > 0`: `s = f/g` (float32);
     if `0.005 < s < 2000`, `sinit = s` (§7.1) and, with `-verbose` and `s < 200`, `++ Scale init =
     <s %g>`; with `s ≥ 200` always `++ Warning: Scale init = <s> is large. Check output.`;
     otherwise always `-- Large scale difference between datasets.\n   Scale init = <s>\n
     3dvolreg might not converge.` (no final line end).
   - `W1` = `B1` plus, inside the box only, the shifted `V0` (`W1[i,j,k] = B1[i,j,k] + V0[i−sx,
     j−sy, k−sz]`, float32); blurred by `twoblur` (with `-verbose`: `++ Blurring first pass
     weight`); every value below `0.025·max` set to 0. The coarse rotation is **not** applied
     here.
   - The basis is `B1` with the weight `W1`, edged (§7.1) as a default weight is.
   This weight is used also when the base is volume 0 itself (fMRIPrep's case).
4. If the basis cannot be built: `** Can't initialize first pass alignment algorithm`, exit 1.
5. For each volume k (skipping the base volume and the empty-ish ones of §7.4, whose first-pass
   parameters are 0): scaled to float32, blurred by `twoblur`; for k > 0 (and for k = 0 when no
   first-pass weight was computed) the coarse search of this volume against `B1` (as above,
   with its verbose lines `[k]`); the initial values `(r, p, y)·σ` (degrees) and `(sx·δx,
   sy·δy, sz·δz)` (mm, float32 products); registered (§7.2); the result kept in degrees (the
   float32 radians times `180/π` in double, rounded to float32) and mm. **(observed)** With `-verbose` at the end:
   `++ CPU time for first pass=<t> s` (larmorx prints 0).
6. Every later fit uses these as its initial values (§8.7).

### 8.6 The coarse searches (first pass only)

**The coarse step.** Before the run, when `-twopass` is given with `del > 0` and `num > 0`:
`m = trunc(0.1·min(nx, ny, nz) + 0.499)` (double), at least 1; if `del > m`: `++ Coarse del was
<del>, replaced with <m>` (on standard error, during the option checks) and `del = m`.

**Shift search** of a volume `V` against a base `B` with an optional weight `W` (float32
volumes, the same grid): the candidates are the shifts `(dx, dy, dz)` with each component in
`−del·num … del·num` in steps of del and `dx² + dy² + dz² ≤ (del·num)²` (integers), in the order
dz outer, dy, dx inner, from the most negative. For each, over the voxels `(i, j, k)` with
`e ≤ i < N_x − e` (likewise j, k), `e = del·(num + 1)` — **the same region for every shift** —
and, with a weight, only where `W > 0`, the float32 sums (storage order, sequential)
`Sbb += W·b·b`, `Svv += W·v·v`, `Sbv += W·b·v` (`b = B[i,j,k]`, `v = V[i − dx, j − dy, k − dz]`,
products left to right, in float32; without a weight the plain products), and the score
`Sbb − Sbv·Sbv/Svv` (float32; `Sbb` alone if `Svv` is not positive): the residual of `B` against
the best multiple of the shifted `V`. The best is the first candidate whose score is **strictly
below** the best so far, starting from **no shift with the score `Σ B²` over the whole volume**
(float32, sequential, unweighted). (AFNI-93: a region with no voxel scores 0.)

**Shift-only search** (`-nocoarserot`): the shift search of `V` against `B` without a weight.

**Rotation and shift search** (the default):
1. **Subsampling.** If the grid has **120 voxels or more along every axis**: `B` and `V` are each
   copied, blurred by §6.3's third line (σ = 1 voxel), and subsampled: `B'[i,j,k] = B[2i, 2j,
   2k]` on a grid of `trunc((N + 1)/2)` per axis (voxel sizes doubled); del is halved (integer
   division) for the search and the shift found is doubled at the end. With `-verbose`, `x`.
   (A del of 1 halves to 0: AFNI-94, §12.)
2. **The weight.** With `-verbose`, `w`. `W = |B'|` (the subsampled or the full base), blurred
   with σ = 1 voxel (FIR only), divided by its maximum (`W·(1/max)`, the reciprocal in float32),
   values below 0.05 set to 0, then the automask of `W` (`specs/3dAutomask.md` §4–§5 on the one
   image, with the clip fraction in force: 0.333 when `-maxdisp` is on, §8.4) sets every voxel
   outside the mask to 0. With `-verbose`: `[<100·masked/total %.1f>%]`.
3. **The 45 rotations** (the NN method, §5.6): for roll index `a = −1, 0, 1` (outer), pitch
   `b = −2 … 2`, yaw `c = −1, 0, 1` (inner): `(r, p, y) = (9a, 9b, 9c)` degrees in the reported
   convention. The rotation `(0, 0, 0)` uses `V'` itself; any other, `V'` resampled "from a
   matrix" (§5.1, nearest orthogonal matrix, no shift) by `R(a1, −(π/180)·σ1·r; a2,
   −(π/180)·σ2·p; a3, −(π/180)·σ3·y)` (§4.4's axes and signs; the angle products in double). Each
   gets its shift search against `B'` with the weight `W`; with `-verbose` a `*` when its score
   is strictly below the best so far, else `.`. The best rotation is the first with the smallest
   score (starting from 1e38).
4. The rotation and shift found (the shift doubled when subsampled).

**(observed)** `-verbose`'s line for one volume:
`++ Getting best coarse rot+shift [3]:w[29.2%]**.**..*.............*....................... 0 0 -9 : 0 0 0`.

### 8.7 The final pass

1. The fit's settings: thresholds `x_t`, `φ_t`, step `dfac`, limit M, the estimation method, and
   the final method for the output (§3).
2. With `-wtinp` (and no `-weight`): the weight is the input's volume 0 (§7.1 item 3).
3. The basis: the base with its weight (§7.1). With `-verbose`: `++ Initializing alignment base`.
   If it cannot be built: `** Can't initialize base image for alignment`, exit 1.
4. For each volume k (with `-verbose`: `++ Starting final pass on <n> sub-bricks: ` once, then
   `<k>` before and `.` after the fit, `.` after storing): §7.4's skips; else the initial values
   are its first-pass parameters (with `-twopass`; else none), registered (§7.2), resampled for
   output (unless `-rotparent`, §8.9, or `-prefix NULL`).
5. The internal parameters are converted to the reported ones (§4.4); the radians to degrees as
   the float32 radians times `180/π` in double, rounded to float32 (also for §8.5's first-pass
   results).
6. **rmsold, rmsnew** (`-dfile`): `√(Σ (b − y)²/N)` over all voxels, the input volume `y` and
   its registered volume, the base `b`; the sum in float32 (sequential), the quotient in float32,
   the root in double, rounded to float32; both 0 with `-rotparent`; for the base volume its
   output is the base, so rmsnew = 0.
7. With `-verbose`: `\n++ CPU time for realignment=<t> s` and, for more than one volume,
   `  [=<t/n> s/sub-brick]`, a line end (larmorx prints 0), then
   `++ Min : roll=%+.3f  pitch=%+.3f  yaw=%+.3f  dS=%+.3f  dL=%+.3f  dP=%+.3f`, the same with
   `Mean:` and `Max :`, over the volumes (the minimum and maximum starting from volume 0, the
   mean as a float32 sum divided by n).

### 8.8 Matrices and maximum displacements

For each volume in order: the matrices of §4.5; `-1Dmatrix_save`'s line; `-savedisp`'s
displacements (§10.3); and, when the `-maxdisp` mask has voxels:
- `R_d = Pᵀ·R·P`, `t_d = Pᵀ·t` (dataset order, double; R double, t the float32 `(dL, dP, dS)`),
  the axis centre `c` (§4.1);
- for each edge voxel position `x` (float32, converted to double; in mask order):
  `q = R_d·(x − c) + t_d + c` and the displacement vector `u = x − q` (double, in this order:
  `x − c`, the product, `+ t_d`, `+ c`, then `x − q`); its length `|u|` (the square root of the
  sum of squares in double) rounded to float32; `dmax` = the largest (starting from 0);
- the **delta**: `|u_prev − u|` (rounded likewise), `u_prev` the same voxel's vector of the
  previous volume (`u` itself for volume 0, so 0); `emax` = the largest;
- the run's largest `dmax` and `emax` with the first volume reaching them (strictly larger).

**AFNI-99.** The positions `x` are AFNI's dataset coordinates (DICOM-valued along each axis),
but `P` carries the axes' signs: for an axis stored L2R, P2A or S2I, `R_d` and `t_d` act on the
position mirrored along that axis, so the same data stored RAS and LPS give different maximum
displacements. **(observed)** A 40 × 44 × 24 series stored with orientation R2L, A2P, I2S gives
1.672, 3.343, 5.014, 6.693, 8.363 mm for its volumes 1–5; the same array stored L2R, P2A, I2S
(so the same motion turned 180° about z, which moves no point farther) gives 1.718, 3.436, 5.150,
6.874, 8.589 mm, which this section's formulas reproduce exactly; with `P` replaced by the
permutation of the axes (`|P|`, no signs) they give 1.672, 3.344, 5.014, 6.693, 8.363, as the
first storage does.
larmorx's default uses `|P|` for the matrix and the shift here and in §10.3 (rule A); `reproduce`
uses `P` as AFNI does. fMRIPrep writes the file (`-maxdisp1D`) but uses only the registered
volumes.

With `-verbose`: `++ Max displacements (mm) for each sub-brick:` before, then ` %.2f(%.2f)` per
volume, a line end. Always: `++ Max displacement in automask = %.2f (mm) at sub-brick %d` and
`++ Max delta displ  in automask = %.2f (mm) at sub-brick %d` (two blanks after `displ`).

### 8.9 `-rotparent` and `-gridparent`

`-rotparent rset` applies, after each volume's registration, the first 3dvolreg transformation
stored in `rset` (an output of AFNI's own 3dvolreg: its `VOLREG_MATVEC_000000`,
`VOLREG_CENTER_OLD` and `VOLREG_CENTER_BASE` attributes, in a NIfTI file's AFNI extension;
larmorx writes no such extension, so only AFNI's outputs can serve), resampling each input volume
once:
1. `R`, `t` from the 12 numbers of `VOLREG_MATVEC_000000` (§4.5's order, float32 to double). If
   `Σ |RᵀR − I|` (the nine entries, double; the sum rounded to float32) exceeds 0.01: `** -rotparent
   matrix not orthogonal!`, exit 1.
2. The centres (DICOM, double from float32): `E2` = the centre (§4.1) of the **input's** grid
   padded as `-gridparent` requires (below), before step 4 moves its origin: the input's origin
   with, along the slice axis, `o − npad_neg·Δ` (a float32 product and difference) and
   `npad_neg + nz + npad_pos` slices; without padding (no `-gridparent`, or one with as many
   slices) the input's own centre. The gridparent's origin is **not** used for `E2` (spec writer,
   2026-11-08, confirmed from the analysis; the reading the clean-room implementer's black-box
   runs against AFNI found). `E1` = the gridparent's centre (or `E2`), `S2` =
   `VOLREG_CENTER_OLD`, `S1` = `VOLREG_CENTER_BASE` (their first three numbers).
   `t ← t + (R·(E2 − S2) − (E1 − S1))`.
3. To dataset order: `R ← Pᵀ·R·P`, `t ← Pᵀ·t` (the output's P).
4. With `-gridparent`, the output's origin becomes the gridparent's (and the slice timing's
   origin with it).
5. Only for a dataset **with slice offsets** (a NIfTI header that gives slice timing,
   3dTshift §3.2; after `-gridparent`'s padding, the padded offsets): they move by
   `ndz = rint(t_z/|Δz|)` slices (double, ties to even; `t_z` the dataset-order z component):
   offset `k` becomes the input's offset `k − ndz` (0 outside); when `ndz ≠ 0` and with
   `-verbose`: `++ adjusting time-offsets by <ndz> slices`. A dataset without slice offsets
   prints nothing here, whatever `ndz` (spec writer, 2026-11-08, confirmed from the analysis; as
   the clean-room implementer's black-box runs found).
6. Each volume is registered as usual (§7.2) but not resampled; its reported parameters are
   printed with `%.4f` and read back (each value rounded to 4 decimals, as float32), turned into
   the transformation `T1` = rotation of §4.4 in dataset axes with the shift **after** the
   rotation (`-ashift`), composed with the rotparent's `T2 = (R, t)`: `T = T2∘T1` (`R2·R1`,
   `R2·t1 + t2`, double).
7. The volume, zero-padded along the slice axis as `-gridparent` requires (below), is resampled
   "from a matrix" (§5.1) by T with the final method, and clipped (when the clipping is on and
   the final method is cubic, quintic, heptic or Fourier) to the padded volume's range (its added
   zeros included). rmsold and rmsnew are 0.

`-gridparent gset` (only with `-rotparent`; else `++ WARNING: -gridparent means nothing without
-rotparent!` and ignored): `gset` must have the input's step sizes (within 1 % each:
`|Δ_g − Δ| ≤ 0.01·|Δ_g|`) and orientation (else `** Fatal Error:\n** -gridparent dataset and
input dataset don't\n** match in grid spacing and/or orientation!`), the same nx and ny (else
`** Fatal Error:\n** -gridparent and input datasets\n** don't match in x,y dimensions!`) and at
least as many slices (else `** Fatal Error:\n** -gridparent has fewer slices than input
dataset!`), each exit 1. With more slices, the output gets `n1 = trunc(Δn/2)` slices added before
and `n2 = Δn − n1` after along the slice axis's direction (`-R`/`-A`/`-I` side first), so
`npad_neg = n1`, `npad_pos = n2` for a slice axis whose step is positive (R2L, A2P, I2S) and the
reverse for a negative one; with `-verbose`: `++ Zero padding to match -gridparent: -<I> <n1>
-<S> <n2>` (the letters of the slice axis: R L, A P or I S, two blanks before the second);
the padded output grid is plumb (its matrix the cardinal one of its axes, an oblique input's
tilt discarded) and its slice offsets padded with zeros. Without padding, an oblique input's
matrix stays and only its origin moves to the gridparent's.

Also: `++ WARNING: Combining -rotparent and -twopass isn't recommended!` (both given),
`++ WARNING: -gridparent overrides -twodup!` (both given; `-twodup` is then off).

### 8.10 `-twodup`

With `-twopass`, an external base and no `-rotparent`: the written output's origin becomes the
base dataset's (and the slice timing's origin with it), after the matrices, displacements and
`-savedisp` datasets were computed (they use the input's grid). Otherwise ignored, silently.

## 9. The command line (`larmorx afni 3dvolreg [options] dataset`)

No argument, or a first argument starting with `-help`: the usage text on standard output, exit
status 0 (larmorx prints its own text; AFNI's help is documentation). Otherwise AFNI's global
options are taken out (3dTshift §7.1), then `-@` as the **last** argument is replaced by the words
read from standard input (split at blanks, tabs, line ends, carriage returns, form feeds and
vertical tabs; global options read this way are unknown options; when nothing or only blanks are
read the `-@` stays and the scan reports it).

### 9.1 The option scan

Options come before the dataset, and the scan stops at the first argument that does not start
with `-`. Each argument is tested **in this order**, by exact name or by its first letters
(`n` letters: the argument must start with that many letters of the name, so `-pref`, `-prefixed`
and `-pre` are `-prefix`):

| # | Option | Match | Value(s) and effect |
|---|---|---|---|
| 1 | `-input`, `-source` | exact | the dataset (opened now); a second one: `** FATAL ERROR: Can't have 2 '<opt>' options!` |
| 2 | `-maxdisp`, `-nomaxdisp` | exact | on / off |
| 3 | `-maxdisp1D mm` | exact | `-maxdisp` on, the file mm and the delta file (§10.2) |
| 4 | `-savedisp sss`, `-SAVEDISP sss` | exact | §10.3 |
| 5 | `-1Dmatrix_save ff` | exact | §10.2; a second one: `** FATAL ERROR: Can't have multiple -1Dmatrix_save options!` |
| 6 | `-sinit` | exact | the scale initialisation **off** |
| 7 | `-params m x r d` | 4 (`-par`) | M = `m` (C `strtol`, base 10), `x_t`, `φ_t`, `dfac` (C `strtod`, float32), any values |
| 8 | `-maxite m` | 4 (`-max`) | M = m if m > 0 (so `-maxd`, which is not `-maxdisp`, is `-maxite`) |
| 9 | `-x_thresh x` | 4 (`-x_t`) | `x_t` if > 0 |
| 10 | `-rot_thresh r` | 6 (`-rot_t`) | `φ_t` if > 0 |
| 11 | `-delta d` | 4 (`-del`) | `dfac` if > 0 |
| 12 | `-final mode` | 4 (`-fin`) | `cubic`, `quintic`, `heptic`, `Fourier`, `NN`, `linear` (exact, case-sensitive); else `** Illegal mode after -final`, exit 1 |
| 13 | `-rotcom` | exact | §10.2 |
| 14 | `-twopass` | exact | on; an M below 10 becomes 66 (the help's "resets to 66" is wrong for the default 23: rule C) |
| 15 | `-twoblur b` | exact | b (`strtod`, float32); below 2.0: `** ERROR: value after -twoblur is < 2.0`, exit 1 |
| 16 | `-verbose` | 5 (`-verb`) | one level more |
| 17 | `-clipit` | 4 (`-cli`) | `++ Notice: -clipit is now the default` on standard error; on |
| 18 | `-noclip` | 4 (`-noc`) | off — **this also catches `-nocoarserot`** (AFNI-86) |
| 19 | `-zpad n` | 5 (`-zpad`) | `trunc(strtod(n))`; if a `-zpad` other than 4 was set before: `++ WARNING: second -zpad option!`; negative: `** ERROR: Can't use -zpad <n>`, exit 1 |
| 20 | `-Fourier`, `-fourier` | 4 | estimation method Fourier |
| 21 | `-linear`, `-Linear` | 4 | linear |
| 22 | `-cubic`, `-Cubic` | 4 | cubic |
| 23 | `-quintic`, `-Quintic` | 4 | quintic |
| 24 | `-heptic`, `-Heptic` | 4 | heptic |
| 25 | `-prefix p` | 4 (`-pre`) | the output (§10.1) |
| 26 | `-dfile f` | 4 (`-dfi`) | §10.2 |
| 27 | `-1Dfile f` | 4 (`-1Df`) | §10.2 |
| 28 | `-base b` | 4 (`-bas`) | below |
| 29 | `-coarse del num` | exact | two `strtol` values, any sign |
| 30 | `-coarserot` | exact | `++ -coarserot is now the default`; on |
| 31 | `-nocoarserot` | exact | off (never reached in AFNI: #18 matches first) |
| 32 | `-wtrim` | exact | `++ Notice: -wtrim is now always enabled` (standard error) |
| 33 | `-wtinp` | exact | §8.7 |
| 34 | `-edging e` | exact | `strtod`; negative: `** Illegal value after -edging`; followed by `%`: a percentage, above 25: `** Illegal percentage after -edging` (exit 1); else a voxel count |
| 35 | `-weight 'wset[n]'` | 4 (`-wei`) | below |
| 36 | `-twodup` | exact | §8.10 |
| 37 | `-tshift ii` | exact | `trunc(strtod(ii))`; negative: `** -tshift parameter is negative!`, exit 1 |
| 38 | `-rotparent rset` | 7 (`-rotpar`) | below |
| 39 | `-gridparent gset` | 7 (`-gridpa`) | below |
| 40 | `-float` | 6 (`-float`) | output as float32 |
| — | anything else starting with `-` | | `** Unknown option: <arg>` followed by a bell (`\a`) and a line end, exit 1 |

Numbers are read as C's `strtol` (leading blanks, a sign, digits; 0 when none) and `strtod`
(rounded to float32 where the value is a float); trailing text is ignored except where stated.

**`-base b`:** a second `-base` after one that set a dataset or an index above 0: `** Can't have
two -base arguments`, exit 1 (after `-base 0` a second one is accepted: AFNI-95). If `b` is
entirely an integer (`strtol` consumes it all): negative: `** FATAL ERROR: Illegal negative
number after -base`; else the base index. Otherwise `b` is a dataset, opened now (`** FATAL
ERROR: Couldn't open -base dataset <b>`); with `-verbose` given before: `++ Reading in base
dataset <name>` (§8.2's name); more than one volume: `*+ WARNING: -base dataset <b> has more than
1 sub-brick`; its volume 0 is the base, its centre and origin are kept.

**`-weight 'wset[n]'`:** a second: `** Can't have two -weight options`. The argument must contain
`[` not at its start (else `** Illegal weight dataset after -weight`); the name is the part before
it, the index read by `sscanf("%d")` after it (`-1` when none: `** Illegal sub-brick selector
after -weight`); the dataset (`** Can't open weight dataset <name>`); the index below its number of
volumes (else `** Illegal sub-brick selector for dataset <name>`); with `-verbose` before:
`++ Reading in weight dataset <name>`; that volume scaled to float32 is the weight. Each exit 1.

**`-rotparent rset`:** a second: `** Can't use -2 -rotparent options!`; the dataset (`** Can't
open -rotparent dataset!` and a blank line); its `VOLREG_MATVEC_000000` attribute with at least 12
numbers (`** -rotparent dataset doesn't have VOLREG attributes!?`). **`-gridparent gset`:** a
second: `** Can't use -2 -gridparent options!`; the dataset (`** Can't open -gridparent dataset!`
and a blank line). Each exit 1.

The datasets of the scan (`-input`, `-base`, `-weight`, `-rotparent`, `-gridparent`) are opened
where the scan meets them, so their reading messages (the obliquity report, the type conversion
notice, §9.3) come in the scan's order, before later options' messages.

### 9.2 After the scan, in this order

1. `-twodup` is dropped unless `-twopass` and an external base were given.
2. `-gridparent` without `-rotparent`: the warning of §8.9, dropped. `-rotparent` with `-twopass`:
   its warning. `-gridparent` with `-twodup`: its warning, `-twodup` dropped.
3. Without `-edging`: `AFNI_VOLREG_EDGING`, parsed as `-edging`'s value, with the messages
   `** Illegal value in AFNI_VOLREG_EDGING` and `** Illegal percentage in AFNI_VOLREG_EDGING`
   (exit 1); unset: 5 %.
4. The dataset: without `-input`, exactly one argument must remain: none: `** FATAL ERROR: Too few
   arguments!?  Last=<the last argument>`; more: `** FATAL ERROR: Too many arguments?!
   Dataset=<the first remaining>?`; it is opened. With `-input` and arguments left: `*+ WARNING:
   Skipping argument(s) after last option? '<the first>'`.
5. If any volume has a brick factor (neither 0 nor 1) and `-float` was not given: `*+ WARNING:
   Input dataset has scale factors ==> output will be stored as floats!` and the output is float32.
6. If the dataset cannot be opened (without `-input`): `** Can't open dataset <arg>`, exit 1.
7. The centres of the base and the input (§4.1); with an external base, `-verbose` and centres
   more than 0.01 mm apart (float32 length): `++ centers of base and input datasets are %.2f mm
   apart`.
8. `-tshift`'s checks (in order, each a warning on standard error that turns `-tshift` off,
   except the last): one volume: `++ WARNING: -tshift used on a 1 brick dataset!`; fewer slice
   offsets than slices: `++ WARNING: -tshift used on a dataset with no time-offsets!`;
   `ii > nvals − 5`: `++ WARNING: -tshift ignore is too large for this dataset!`; an internal base
   index below ii: `++ WARNING: base brick is prior to -tshift ignore point.` (`-tshift` stays on).
9. An internal base index ≥ the number of volumes: `** Dataset <arg> doesn't have base brick index
   = <n>`, exit 1 (`<arg>` is the argument after the options: with `-input` there is none and AFNI
   prints `(null)`: AFNI-98).
10. An external base whose grid differs: each of `** Input <in> and base <base> don't have same
    dimensions!\n   Input: nx=%d  ny=%d  nz=%d\n   Base:  nx=%d  ny=%d  nz=%d`, `... same grid
    spacing!\n   Input: dx=%6.3f  dy=%6.3f  dz=%6.3f\n   Base:  dx=...` (the signed steps; a step
    differs when the difference exceeds 1 % of the first dataset's), `... same orientation!\n
    Input: <o> <o> <o>\n   Base:  <o> <o> <o> ` (orientations as `R-L`, `L-R`, `P-A`, `A-P`, `I-S`,
    `S-I`; a blank at the end of the base's line) that applies, then `** FATAL ERROR: perhaps you
    could make your datasets match?`, exit 1. The names are the datasets' files with `./` added
    when they have no directory.
11. A `-weight` volume of another size: `** Dataset <arg> doesn't conform to dimensions of weight
    brick\n    Weight  has nx = %d  ny = %d  nz = %d\n    Dataset has nx = %d  ny = %d  nz = %d`,
    exit 1 (`<arg>` as in 9).
12. An internal base and one volume: `** FATAL ERROR: You can't register a 1 brick dataset to
    itself!`.
13. The coarse step (§8.6).
14. `-weight` with `-wtinp`: `++ Input weight file overrides -wtinp option!`, `-wtinp` off.

### 9.3 Messages

On standard error unless stated; **(observed)** in the order of §8 and §9. The prefixes are
AFNI's: `++ ` information, ` + ` continued information, `*+ WARNING: ` warnings, `** ERROR: `
errors, `** FATAL ERROR: ` fatal errors (followed by `** Program compile date = <date>` in AFNI,
which larmorx does not print). AFNI colours the labels of warnings and errors (AFNI-21, 3dTshift
§7.1): `reproduce` writes AFNI's, the default plain ones.
- At the start: AFNI's banner `++ 3dvolreg: AFNI version=...` (larmorx prints none) and
  `++ Authored by: RW Cox` (not with `AFNI_QUIET_STARTUP` yes).
- Reading NIfTI datasets: as 3dTshift §6/§7.1 (the "NO spatial transform" warning, the obliquity
  report). Two run-wide rules: the obliquity report's long warning (the paragraph starting
  `  If you are performing spatial transformations on an oblique dset`) is printed for the
  run's first oblique dataset only (its one-line `Oblique dataset: ... degrees from plumb` for each
  of the first 20); the notice that a NIfTI type is converted to float (`** AFNI converts
  NIFTI_datatype=...`) for the run's first such dataset only, unless `AFNI_NIFTI_TYPE_WARN` is yes.
- The first blur's `++ edt_blur: ...` line (§6.3), always.
- §7.1's trimming lines when they apply, always.
- §8's `-maxdisp` lines, always; the coarse-step line with `-twopass`.
- The output file messages (§10).
- With `-verbose` (larmorx adds its own first lines: which implementation runs and the D13 mode):
  §8's verbose lines; with a byte or scaled input no more.
- With `-verbose -verbose`, in addition, from the basis and the fits (the numbers `%g` unless
  stated): `  initializing weight`, `:` after its blur (the `edt_blur` line may fall between);
  `  processing weight` for `-wtinp`; the trimming line ` + wtrim: [xa..xb]x[ya..yb]x[za..zb] = <k>
  voxels kept, out of <n> (<T %.1f>%)`; `  weight range: min=<g>  max=<g>`; ` # zero weights=<k> out
  of <n> (<%.1f>%)`; `  mri_3dalign: using <trunc(10·voxels·4/2²⁰)> Mbytes of workspace`; for each
  of `d/d(th1)`, `d/d(th2)`, `d/d(th3)`, `d/dx`, `d/dy`, `d/dz`: `  initializing <name>[delta=<ε>]`,
  `:` after each of the two resamplings, `  <name> range: min=<g>  max=<g>`; `  initializing least
  squares`; for each fit: `\nFirst fit: ` and the seven coefficients each `%13.6g` separated by
  blanks (the initial values with scale 1 when given), per iteration `Delta fit: ` (the increment's
  seven) and `Total fit: ` (the increment's scale and the six totals), and `Iteration complete at
  <k> steps`.
  **Over the box** (spec writer, 2026-11-08, confirmed from the analysis; **(observed)** by the
  clean-room implementer): every number of these setup lines after ` + wtrim:` is taken over the
  fit's box (§7.1 "Trimming"), the whole volume when nothing is trimmed: `weight range` is the
  trimmed weight's minimum and maximum, ` # zero weights=<k> out of <n>` counts the box's voxels
  (n is the box's size), the workspace's `voxels` are the box's, and each `<name> range` is the
  range of that derivative image **after** it was cut to the box, not of the whole volume.
- **(observed)** A whole quiet run of fMRIPrep's call on a 40 × 44 × 24 × 6 series:

      ++ Authored by: RW Cox
      ++ Coarse del was 10, replaced with 2
      ++ edt_blur: sigx 2, dx 1, fwhm 4.70964, sfac 2.5, firx 5
      ++ Max displacement in automask = 8.36 (mm) at sub-brick 5
      ++ Max delta displ  in automask = 1.69 (mm) at sub-brick 2

The names AFNI shows: a NIfTI dataset's file as given (in warnings), or its real path (the
`-verbose` "Reading" lines and "Wrote dataset to disk in"), with `./` added for a qform or sform
of code 2; the mismatch messages add `./` to a name without a directory.

### 9.4 Exit statuses and failures

Exit status 0 on success, also when an output cannot be written (`** ERROR: Cannot open '<f>' for
output` for the text files, with ` :(` after it for `-1Dmatrix_save` and `-maxdisp1D`; AFNI-11 and
AFNI-12 for datasets), 1 for every error of §7.3, §8 and §9, and where AFNI crashes (AFNI-88,
AFNI-89, AFNI-90, AFNI-97: §12). larmorx also stops with status 1 where AFNI would write AFNI's own
format (a `-prefix` or `-savedisp` without `.nii`/`.nii.gz`, the default prefix `volreg`
included), names a sub-brick selector (`dset[...]`), or reads AFNI's `.HEAD/.BRIK` format (all the
larmorx AFNI tools' rule); with `-verbose` its errors name the reason.

### 9.5 Environment variables and global options

AFNI's global options (`-overwrite`, `-Dname=value`, `-skip_afnirc`, `-ok_1D_text`) and the
variables the dataset reader and writer consult are as in 3dTshift §7.1. 3dvolreg's own:
`AFNI_VOLREG_EDGING` (§9.2), `AFNI_BLUR_FIRFAC` and `AFNI_BLUR_FIROLD` (§6.1),
`AFNI_DECONFLICT` (§8.1; and when writing, 3dTshift §6.3), `AFNI_NIFTI_TYPE_WARN` (§9.3),
`AFNI_QUIET_STARTUP`. `AFNI_ROTA_ZPAD` is never read (§5.4). `AFNI_BLUR_INTS_AS_OLD` has no
effect (it concerns integer volumes; every blur here is of float32 volumes). OpenMP settings have
no effect: AFNI's 3dvolreg is single-threaded on every path of this spec, and larmorx's results
must not depend on `n_threads`.

## 10. Outputs

### 10.1 The registered dataset (`-prefix`)

- One output volume per input volume, on the output grid (the input's, or §8.9's padded one).
- **Storage type:** float32 with `-float` or when the input has brick factors (§9.2 item 5);
  otherwise each volume's own: float32 as is; int16 rounded (ties to even) after clamping to
  `[−32767, 32767]`; uint8: AFNI-87 (§12). No brick factors (`scl_slope` 0).
- **Header:** as 3dTshift §6 writes a dataset that keeps its input's grid (geometry §6.1, slice
  fields §6.2): the input's TR and time origin and slice timing (no slice offsets and time origin
  0 after `-tshift`; §8.9's moved or padded offsets with `-rotparent`/`-gridparent`); the padded
  grid of §8.9 is written plumb; `-twodup` moves the origin (§8.10). NIfTI-1, little-endian.
- AFNI also writes an AFNI extension with the history and the `VOLREG_*` attributes
  (`VOLREG_ROTCOM_NUM`, `VOLREG_ROTCOM_<k>` with the `-rotate ... -ashift ...` text of each volume,
  `VOLREG_MATVEC_<k>` with §4.5's 12 numbers, `VOLREG_CENTER_OLD` (the output grid's centre),
  `VOLREG_CENTER_BASE`, the input's, base's, rotparent's and gridparent's names and ID codes);
  larmorx writes no extension (every larmorx AFNI tool), so its outputs cannot be `-rotparent`s.
- Existing outputs and refused names: 3dTshift §6.3 (AFNI-11, AFNI-12, AFNI-26), except that
  3dvolreg does **not** check `-prefix` during the option scan: a name AFNI refuses is found only
  when writing (**(observed)** `** ERROR: Illegal filename for NIfTI output: ...`, nothing
  written, exit status 0).
- With `-verbose`: `++ Wrote dataset to disk in <name>`.

### 10.2 Text outputs

| Option | File | Format (C `printf`) |
|---|---|---|
| `-1Dfile f` | f | per volume `%8.4f %8.4f %8.4f %8.4f %8.4f %8.4f\n`: roll pitch yaw dS dL dP |
| `-dfile f` | f | per volume `%4d %8.4f %8.4f %8.4f %8.4f %8.4f %8.4f  %11.4g %11.4g\n`: index, the six, rmsold, rmsnew (two blanks before rmsold) |
| `-1Dmatrix_save ff` | `ff` if it ends in `.1D`, else `ff.aff12.1D` | §4.5 |
| `-maxdisp1D mm` | mm, and its delta file | `# <command line>\n`, `# max displacement (mm) for each volume\n` (the delta file: `# max delta displ (mm) for each volume\n`), then ` %.3f\n` per volume |
| `-rotcom` | standard output | `\n3drotate fragment<s>:\n\n` (`s` for more than one volume), per volume `3drotate <-NN\|-linear\|-cubic\|-Fourier\|-quintic\|-heptic> <-clipit\|-noclip>[ -zpad <n>] -rotate %.4fI %.4fR %.4fA -ashift %.4fS %.4fL %.4fP\n` (the final method; `-zpad` when n ≠ 0), then `\n` |

- **The delta file's name:** `_delt` inserted before the name's extension when it ends with one of
  `.HEAD .BRIK .BRIK.gz .mnc .mri .svl .1D .1D.dset .1D.do .txt .3D .nii .nii.gz .nia .hdr .img
  .mpg .mpeg .MPG .MPEG .niml .niml.dset .niml.do .gii .gii.dset .niml.tract .jpg .jpeg .png
  .heic` (`md.1D` → `md_delt.1D`), else appended (`md` → `md_delt`). `-maxdisp1D -` writes the
  displacements to standard output and the deltas to a file named `-_delt` (rule C).
- **The command line** of `-maxdisp1D`'s first line: `3dvolreg` and the arguments (after the
  global options were taken out and `-@` expanded), each preceded by a blank; an argument AFNI's
  file-name rule (3dTshift §6.3) refuses is written in single quotes with its blanks, control
  characters and non-ASCII bytes replaced by blanks; empty arguments left out.
- **Existing files** are overwritten: `-1Dfile` and `-dfile` with `** Warning: overwriting file
  <f>` (standard error), `-maxdisp1D`'s with `*+ WARNING: Overwriting file <f>`, `-1Dmatrix_save`
  silently. `-1Dmatrix_save`'s name must pass the file-name rule at the scan (`** FATAL ERROR:
  badly formed filename: -1Dmatrix_save '<ff>'`).
- **Order of writing:** `-1Dmatrix_save` (its first line before the matrices are computed, its
  lines as they are), the two `Max displacement` lines and `-maxdisp1D`'s files, the dataset,
  `-savedisp`'s datasets, `-dfile`, `-1Dfile`, `-rotcom`, then (input with brick factors, an
  output written) `*+ WARNING: Input dataset has scale factors ==> output is stored as floats!`.

### 10.3 `-savedisp sss`, `-SAVEDISP sss`

The name must pass the file-name rule and, ending in `.nii` (`.nii.gz`), be at least 5 (8) bytes
long (else `** FATAL ERROR: -savedisp '<sss>' is invalid output dataset name`, with the option as
given); `sss` = stem + suffix (`.nii`, `.nii.gz`, or none: AFNI's own format, refused by larmorx).

Three float32 datasets `<stem>_DX<suffix>`, `_DY`, `_DZ`, on the output grid with the output's
header (no brick factors), one volume per input volume, also with `-prefix NULL`: at voxel
`(i, j, k)` of the output grid, with `x = (o + i·Δ)` per axis taken on the **input's** grid
(float32), `c` the output grid's axis centre (§4.1), `R_d`, `t_d` as §8.8 (the output's P;
by default `|P|`, AFNI-99):
`u = x − (R_d·(x − c) + t_d + c)` (double; the same operation order), and the datasets hold
`u_0`, `u_1`, `u_2` (dataset axes 0, 1, 2; mm along each axis's direction) rounded to float32.
Written after the registered dataset, in the order X, Y, Z (with `-verbose` each `++ Wrote
dataset to disk in <name>`).

`-SAVEDISP` normalises each of the three before writing, voxel by voxel over its volumes: the
linear trend removed (3dTshift §4 step 2's fit over all volumes; skipped with fewer than 3), then
every value multiplied by `1/max |value|` (float32; left alone when the maximum is 0); and then
writes 16 more, in this order: `_DXX, _DYY, _DZZ, _DXY, _DXZ, _DYZ, _DXXX, _DYYY, _DZZZ, _DXXY,
_DXXZ, _DXYY, _DXZZ, _DYYZ, _DYZZ, _DXYZ`, each voxel `P_a(X)·P_b(Y)·P_c(Z)` of the normalised
three (a, b, c the letters' counts; `P_0 = 1`, `P_1(x) = x`, `P_2(x) = 1.5·x·x − 0.5`,
`P_3(x) = (2.5·x·x − 1.5)·x`, all float32 left to right, the three factors multiplied left to
right), normalised the same way. (The help lists only the three datasets; `-SAVEDISP` is
undocumented.) The values normalised are exactly the float32 values `-savedisp` writes for the
same arguments (they are normalised in place, after they were computed as above), and the 16
products are made from the three normalised datasets; how both are validated: §14.3,
"`-SAVEDISP`" (spec writer, 2026-11-08: the normalisation magnifies small differences of the raw
values).

**The arithmetic, exactly** (spec writer, 2026-11-08, step 3; **(observed)**: with it, the
nineteen datasets made from AFNI's own `-savedisp` values are AFNI's `-SAVEDISP` datasets bit
for bit, 0 of 4,815,360 values differing on the suite's `s1` and 0 of 52,920,320 on fMRIPrep's
ds000005 input, `validation/investigations/3dvolreg-spec/savedisp.py`). For one voxel's series
`v_0 … v_{n−1}` (float32; n the number of volumes, at least 3):
1. **The line.** `S0 = v_0 + v_1 + … + v_{n−1}` and `S1 = (v_1·1) + (v_2·2) + … + (v_{n−1}·(n−1))`
   in double, each sum accumulated in that order (S0 starting from `v_0`, S1 from 0), each
   product `v_i·i` a float32 product; `N = n` and `Q = n·n` (an integer product) as doubles; then,
   each operation in double in the order the parentheses give,
   `a = ((2/(N+1))·(1/N))·(((2·(N·S0)) − 3·S1) − S0)` and
   `b = ((−6/(Q−1))·(1/N))·(((−S0) − 2·S1) + N·S0)`, each rounded to float32. This order is part
   of the behaviour: a or b is decided by the double's last bits now and then, and the
   normalisation's division by a small largest value turns that one float32 step into a visible
   difference (the textbook forms `(2(2n−1)·S0 − 6·S1)/(n(n+1))` and `(12·S1 − 6(n−1)·S0)/(n(n²−1))`,
   equal in exact arithmetic, change 3,983 of ds000005's 8,355,840 normalised `_DX`, `_DY`, `_DZ`
   values; none of `s1`'s, whose 6 volumes rarely meet such a case).
2. **Detrended:** `w_i = v_i − (a + b·i)` in float32 (`b·i` first, then `a +`, then the
   subtraction; for i = 0 the same, `b·0 = 0`).
3. **Normalised:** `m = max_i |w_i|` (float32); when `m ≠ 0`, `r = 1/m` (float32) and every value
   `w_i·r` (float32); when `m = 0`, the values stay as they are.
4. **The products**, from the three normalised series `X`, `Y`, `Z` of the voxel (the values
   `_DX`, `_DY`, `_DZ` hold): `P_2(x) = ((1.5·x)·x) − 0.5`, `P_3(x) = (((2.5·x)·x) − 1.5)·x`
   (float32, the constants float32), `P_1(x) = x`, `P_0 = 1`, and the value
   `(P_a(X)·P_b(Y))·P_c(Z)` (float32); each product series is then detrended and normalised by
   steps 1 to 3. Any other association changes many values, not a few (on `s1`: `1.5·(x·x)`
   40 % to 58 % of each product's values, the polynomials in double 46 % to 70 %, `X·(Y·Z)` for
   `_DXYZ` 25 %); a difference at a few voxels comes from steps 1 to 3 (the original of
   2026-11-08 differs from this arithmetic applied to its own `_DX`, `_DY`, `_DZ` at 233 of `s1`'s
   4,055,040 product values and 13,648 of ds000005's: `savedisp.py --original`).

## 11. A faster implementation: what must stay, what may change

**Must stay** (it decides the results):
- the conventions of §4 (axes, signs, the parameters' and matrices' definitions, negative zeros);
- the resampling of §5: the flip rule, the four-shear decomposition and the choice among the six
  orders by §5.3's norm, the voxel scaling, the row-by-row interpolation with each method's
  kernel, zeros outside the row, the zero padding, the Fourier method's padded length and its
  circular shift, the nudge of a zero rotation, the skip of a shear below 1e-3;
- the blurs' kernels, half-widths, mirrors and the cases they skip (§6);
- the basis (weights, edging, the trimming box, the derivative images' steps and scale, the
  seven-vector weighted least squares), the start, increment and stopping rules (§7);
- the two-pass scheme, the coarse searches' candidates, region, scores, the strict "better than"
  and their order, the subsampling rule, the automasks (§8);
- the storage conversions, the file formats, the messages, the failures (§9, §10, §12).

**May change** (how it is computed), as long as the result is deterministic and independent of
the thread count:
- **Parallelism:** the volumes of each pass are independent once the basis exists (the first
  pass's weight uses volume 0 before the others): they may run in parallel; within a resampling
  the rows of one shear are independent; the 45 coarse rotations and their shift scores are
  independent (keep the order for the "first strictly better" rule); the dot products may be
  split into a fixed number of blocks summed in a fixed order.
- **Arithmetic:** double where AFNI uses float32, other summation orders (pairwise, blocked), a
  different FFT (any algorithm for the same length L), SIMD, a transposed layout for the y and z
  shears. Each changes last bits, which the thresholds of §14.2 cover; the polynomial methods can
  be bit-exact if the orders of §5.6 and §7.1 are kept, and it is worth keeping them (fMRIPrep's
  path is Fourier, the default is heptic).
- **Fusion:** e.g. the fit's seven products computed while the last shear writes its rows; the
  shift search's 33 scores in one sweep; the basis built once per pass.
- **Memory:** at most what AFNI holds: AFNI keeps one copy of the input, frees each volume once it
  is registered and stored, and needs the basis (base, weight, six derivative images: about 8
  volumes, trimmed) and a few padded work volumes (about 10 volumes in all; the help's "40 bytes
  per input voxel"). Measured **(observed)** for fMRIPrep's call on 64 × 64 × 40 × 20 float32
  voxels: AFNI 27 MiB, the replica 29 MiB. No full-size copies AFNI does not hold, no cache that
  raises the peak (CLAUDE.md, "Memory similar to the original's").
- **Speed:** the replica takes 2.9 s on fMRIPrep's ds000005 call (64 × 64 × 34 × 20) against AFNI's
  9.0 s, 41 s on ds005454 (160 × 160 × 96 × 20) against 93 s, one thread each
  (`docs/api/afni-volreg.md`, informal). The first pass's coarse search (45 nearest-neighbour
  rotations per volume and their shift scores) and the Fourier resamplings dominate.

## 12. Defects and what larmorx does by default

Each is corrected by default and named by an `UpstreamDefectWarning` (`*+ WARNING: upstream
defect AFNI-<n> ...` on the command line, after the run's other messages) where the correction
changed something; `reproduce` gives AFNI's behaviour. Rules A–D of the decision log
(2026-10-23).

| ID | Rule | AFNI (`reproduce`) | larmorx's default | Named |
|---|---|---|---|---|
| AFNI-86 | A | `-nocoarserot` is read as `-noclip` (its first 4 letters, §9.1 #18): the clipping turns off, the coarse rotation search stays on | `-nocoarserot` turns the coarse rotation search off, as the help says | whenever `-nocoarserot` is given |
| AFNI-87 | A | a uint8 dataset's registered volumes are stretched, each by its own range: `trunc(s·(x − min))`, `s = 255.9/(max − min)` (double; `max = min + 1` when `max ≤ min`; `x − min` in float32), stored as uint8 | rounded (ties to even) and clamped to 0..255 | when the two differ |
| AFNI-88 | B | an option that needs a value, given last, reads past the arguments: a crash (`Fatal Signal 11`), except `-input`/`-source` (`** FATAL ERROR: Can't open input dataset '(null)'`), `-rotparent`/`-gridparent` (`** Can't open -rotparent dataset!` and a blank line) and `-1Dmatrix_save` (`** FATAL ERROR: no argument after '-1Dmatrix_save'!`) | an error naming the option, exit 1, in both modes (AFNI's own message where it has one) | in both modes |
| AFNI-89 | B | a basis whose least squares failed (§7.1: a reference vector 0 over the whole box, a failed Cholesky; §7.3 lists the inputs measured: an all-zero weight, and with `-twopass` a base that does not vary along an axis of 30 voxels or more, for some of its values) prints its errors and crashes at the first volume fitted | AFNI's messages, then an error, exit 1, in both modes | in both modes |
| AFNI-90 | A | a `-prefix`, `-dfile` or `-1Dfile` value, or `-weight`'s dataset name, of 256 bytes or more aborts the program at that option (`*** buffer overflow detected ***: terminated` and the crash banner, exit 1); `-maxdisp1D`'s name is cut to 997 bytes | kept whole | when a name is that long |
| AFNI-92 | A | the coarse rotation search's automask uses the clip fraction left by `-maxdisp` (0.333), so `-nomaxdisp` changes it to 0.5 | 0.333 always | with `-nomaxdisp`, `-twopass` and the coarse rotation search |
| AFNI-93 | A | a shift search whose region has no voxel (a grid of at most `2·del·(num+1)` voxels along an axis) scores every candidate 0: the first shift (`dz = −del·num`) wins, and in the rotation search the first rotation (−9°, −18°, −9°) | such candidates are not taken: no shift, no rotation | when such a region is met |
| AFNI-94 | B | `-twopass` with a coarse step of 1 (after §8.6's adjustment) on a grid of 120 or more voxels along every axis: the subsampled search's step halves to 0 and AFNI never ends | an error before the work, in both modes | in both modes |
| AFNI-95 | B | `-base 0` followed by another `-base` is accepted (the last wins) | `** Can't have two -base arguments`, exit 1 | when it happens |
| AFNI-96 | B | a `-rotparent` without `VOLREG_CENTER_OLD` or `VOLREG_CENTER_BASE`, or with fewer than 3 numbers in one, is read through a null pointer or past the attribute | an error, exit 1, in both modes | in both modes |
| AFNI-97 | A | Fourier: a lone last row (§5.5) whose shift and second shift are both beyond the row (rotations far beyond a head's motion, e.g. a runaway first pass) is cleared through a null pointer: a crash | the row cleared alone, as a pair is | when it happens |
| AFNI-99 | A | `-maxdisp` and `-savedisp` convert the matrix with the axes' signs but not the positions: for an axis stored L2R, P2A or S2I the displacements are those of the mirrored position (§8.8; the same data stored RAS and LPS: 1.718 against 1.672 mm) | the permutation without signs: the displacement of the voxel itself | when a value the run prints or writes differs: the maximum displacements as the two messages (two decimals and the sub-brick), `-verbose`'s list (two decimals) and the `-maxdisp1D` files (three decimals) show them, `-savedisp`'s values as stored |
| AFNI-98 | A | with `-input`, the messages "Dataset %s doesn't have base brick index" and "doesn't conform to dimensions of weight brick" print the argument after the options: `(null)` when there is none (or a stray argument) | the dataset's name | when they differ |

Shared with the other AFNI tools (3dTshift's spec has them): AFNI-01 (a negative brick factor on
`-tshift`'s path), AFNI-11, AFNI-12, AFNI-18, AFNI-21, AFNI-26, and GLIBC-07 (§4.3: correctly
rounded `sin`, `cos`, `exp`, `pow` by default, glibc 2.35's with `reproduce`, never named for this
iterative program).

**Kept in both modes and documented (rule C, no ID):**
- the blur's row-end mirrors (§6.1): measured (rule D): mirroring every row about both end samples
  on every axis moves the parameters by at most 1.5e-5° and 1.1e-5 mm on synthetic series (5e-7°
  and 2.4e-7 mm on fMRIPrep's inputs), the maximum displacements by 7e-6 mm and the registered
  volumes by 1.4e-6 of their largest value: no gain;
- the first-pass weight built from the base and volume 0 even when the base is volume 0 (§8.5):
  fMRIPrep's path relies on it, and the first pass only seeds the second;
- the nearest-neighbour shift takes the floor of the position (§5.6), and an exact negative
  integer position one sample lower (reached only by exact integer shifts);
- the help's "`-twopass` resets `-maxite` to 66" (only an M below 10 is raised) and
  "`AFNI_ROTA_ZPAD`" (never read);
- `-maxdisp1D -` writing its deltas to a file named `-_delt`.

## 13. larmorx interfaces

- **Rust**, crate `larmorx-afni` (the original): a function that registers a series held in
  AFNI's storage types (uint8 / int16 / float32 volumes with their brick factors) on its grid,
  with the parameters of §3 as a typed struct, explicit `n_threads` and the D13 mode, returning the
  registered volumes, the internal and reported parameters, the matrices, rmsold/rmsnew, the
  maximum displacements and the `-savedisp` volumes; the messages as a list (so the command line
  can print them in AFNI's order). The reader and writer are the crate's AFNI NIfTI ones
  (3dTshift §6).
- **Python:** `lx.afni.volreg(image, *, base=0, interpolation="heptic", final=None, clip=True,
  zpad=4, twopass=False, twoblur=2.0, coarse=(10, 2), coarse_rotation=True, sinit=True,
  weight=None, weight_index=0, weight_input=False, edging=None, max_iterations=23,
  x_threshold=0.01, rot_threshold=0.02, delta=0.70, tshift=None, float_output=False, maxdisp=True,
  save_displacements=False, rotparent=None, gridparent=None, twodup=False, n_threads=1,
  implementation="auto", upstream_defects="fix", afni_settings=None) -> VolregResult` (the
  replica's signature, `docs/api/afni-volreg.md`; `implementation="original"` runs the original,
  `"auto"` the replica while it is installed, as for 3dTshift) with `registered`, `parameters`
  (n × 6, reported), `matrices` (n × 12, `-1Dmatrix_save`'s), `rms`, `max_displacement`,
  `max_delta_displacement`, `displacements`; `lx.afni.fmriprep_volreg_options()` gives fMRIPrep's
  arguments. It never reads the process environment (`afni_settings` instead, 3dTshift §8).
- **CLI:** §9, `larmorx afni 3dvolreg` (`LARMORX_IMPLEMENTATION=original` chooses the original).
- **Docs:** `docs/api/afni-volreg.md` gains the original's columns (status, speed, differences).

## 14. Validation plan for the original

### 14.1 The suite

The parity suite `afni-volreg` (`validation/src/larmorx_validation/parity/afni_volreg.py`,
`tests/parity/test_afni_volreg_replica.py`; the record `docs/validation/afni-volreg-replica.md`)
runs AFNI 26.3.05's `3dvolreg` through the oracle-output cache (one thread; its 380 runs are
cached) and larmorx with the same arguments, each in its own directory, with a clean environment
(`HOME` empty, AFNI's variables cleared, `AFNI_DONT_LOGFILE` yes); it compares exit statuses,
messages byte for byte (AFNI's banner and compile date, larmorx's own lines and the CPU times set
aside), every text output byte for byte and every NIfTI output (data and header). The original
joins it as a second implementation (`IMPLEMENTATIONS`), with its own record
`afni-volreg-original` (standard tier) from a clean commit. Its 187 standard cases, by category:
base (7), datatypes (10: int16, uint8, scaled, float64, uint16, ...), defects (3), the random
trial's permanent cases (6), divergences (2), edge cases (23: one volume with an external base, no
motion, large rotations, a constant series, one and four slices, 120³ for the subsampled search,
odd sizes, obliquity, ...), environment (6), errors (22), fMRIPrep's calls (8, §14.2), global
options (8), interpolation (21: every method and final method, clipping, padding), messages (10:
`-verbose`, `-verbose -verbose`), outputs (10), `-rotparent` (20), `-tshift` (8), `-twodup` (4),
`-twopass` (11), weights (8). The inputs are derived into `oracles/inputs/afni-volreg/` (synthetic
moving series of plain arithmetic, their variants, `-rotparent` datasets made by AFNI's own
3dvolreg, the oracle runs' inputs of §14.2). Each case runs in `reproduce` against AFNI, then the
default: a defect must be named exactly where the two modes differ.

### 14.2 fMRIPrep's calls

RobustAverage's call (§2) on the `_sliced` input of each oracle run's node
`work/fmriprep_25_2_wf/<subject>_wf/<bold>_wf/bold_fit_wf/hmc_boldref_wf/gen_avg` (PEPOLAR:
`fmap_preproc_wf/wf_auto_<n>/ref_average`), re-made from the node's recorded inputs (`in_file`,
`t_mask`, `nonnegative`, its `_report/report.rst`) by niworkflows' own lines run with fMRIPrep
25.2.6's Python in its image (the suite's `SLICED_SCRIPT`), then the median of the registered
volumes compared with the node's own `*_average.nii.gz`:

| Label | Oracle run | What it adds |
|---|---|---|
| ds000005-run-01 | ds000005-sub-01-noreconall | single-band, int16 stored, 64 × 64 × 34, 20 volumes (smoke tier) |
| ds000005-run-03 | ds000005-sub-01-noreconall | two volumes |
| ds005454 | ds005454-sub-16-noreconall | multiband, 160 × 160 × 96 (not subsampled: nz < 120), oblique |
| ds003592-echo-1 | ds003592-sub-280-noreconall | multi-echo (echo 1), oblique, 64 × 64 × 43 |
| ds004958 | ds004958-sub-fsm64ps-noreconall | multiband 8, uint16 stored, 104 × 104 × 72, 3 volumes, 24° oblique |
| ds002603-mb4 | ds002603-sub-01-noreconall | Philips multiband 4, float32 stored |
| pepolar-ds005040 | ds005040-sub-002-noreconall | sdcflows' PEPOLAR reference average: two topup-corrected volumes, float32 |
| pepolar-ds001600 | ds001600-sub-1-noreconall | the same: two volumes, 80 × 80 × 60 |

**(observed)** The replica is bit-identical with AFNI on all eight, and its medians are fMRIPrep's
own outputs byte for byte; fMRIPrep's own AFNI (in its image) gives the oracle's outputs bit for
bit on all eight. Seven more runs are in the full tier (ds000228, ds000256, ds002422, ds004884,
ds005003, ds005040's BOLD, ds005752's reverse run).

### 14.3 Thresholds (the original against AFNI, `reproduce`; and its default against the replica's default)

(spec writer, 2026-11-08: revised from the measurements of §5.3, "Numerics". A clean-room
original differs from AFNI because the decomposition's exact ties may be decided the other way,
at every resampling; the thresholds allow for either side of every tie and nothing more.
`validation/investigations/3dvolreg-spec/thresholds.py` holds every requirement below against
AFNI for the model of §15 taking the smallest computed norm, the first and the last side of
every tie, on 15 cases: the 40 × 44 × 24 × 6 series `s1` of §14.1 with each estimation method,
`-twopass`, `-linear -twopass` and fMRIPrep's call, the model's own phantom with heptic and
`-linear`, and fMRIPrep's inputs re-made from ds000005 and ds003592 with heptic, `-linear` and
fMRIPrep's call: **0 failures** in 45 runs. The earlier fixed thresholds for the matrices, 1e-5,
and the maximum displacements, 0.01 mm, were stricter than the parameters they follow from: they
failed respectively every volume whose angles differ by more than about 0.0006° and the cubic
and linear runs, with every parameter within its threshold.)

(spec writer, 2026-11-08, step 3: measured against AFNI's own spread under the ties on real
high-motion data and random testing's findings, §5.3. The registered volumes' limit now grows
with each volume's rotation, fMRIPrep's median is held to the limit its volumes imply, `-final
NN` has a nearest-neighbour rule, and fits that are ill-conditioned with respect to the ties, or
run away, are judged by "The ties' spread" below. `thresholds.py` holds the revised requirements
on 21 cases with six tie choices each, fMRIPrep's calls on ds000256 `nofeedback`, ds000228,
ds004958 and ds005454 included; `spread.py` holds the ties' spread on random testing's findings:
§15.)

| Quantity | Requirement |
|---|---|
| exit status | the same |
| messages | byte for byte, except: the CPU times; the numbers that follow from the parameters (the two `Max displacement` messages and `-verbose`'s ` %.2f(%.2f)` list: within the derived bounds below plus 0.01, the printed sub-brick the same unless it is a near tie, reported: (spec writer, 2026-11-08, step 3) in AFNI's run, the value at the original's printed sub-brick is within the sum of the two sub-bricks' derived bounds plus 0.01 (the two-decimal printing) of AFNI's printed largest, taken from AFNI's `-maxdisp1D` values when written, else from the `-verbose` list, else the near tie is assumed and reported); `-verbose -verbose` (next row) |
| `-verbose -verbose` | the lines that come before any resampling exactly (` + wtrim: ...`, `weight range`, ` # zero weights`, the workspace: §9.3); each derivative image's range within 10 % of the larger magnitude (the tie choices move them by up to 1.4 % with heptic, 5.5 % with linear); the fit lines (`First fit`, `Delta fit`, `Total fit`, `Iteration complete at <k> steps`) the same text with their numbers masked where both runs iterate as often; their numbers, and a different count, reported, not required (the first fit's coefficients move by up to 4e-3 relative with heptic, 4e-2 with linear) |
| coarse search results (`-verbose`'s coarse lines) | the same rotation and shift for every volume on §14.1's named cases and §14.2's calls; elsewhere a difference is reported, not required (spec writer, 2026-11-08, step 3): the search's 44 nearest-neighbour rotations meet the ties too (240 ties on ds000256 `nofeedback`), and on random testing's phantoms AFNI's own result changes with them (measured: AFNI `0 9 -9 : 0 0 0`, its tie variants `-9 18 -9 : 0 -2 0` and `0 -0 -9 : 0 0 0`); the coarse result only starts the first pass, and the final parameters are judged by the rows below |
| parameters (`-1Dfile`, `-dfile`, `-rotcom`) | per volume `|Δ angle| ≤ φ_t + k·|angle|` and `|Δ shift| ≤ x_t·δ_n + k·|shift|` (δ_n the voxel size along the shift's axis; `angle` and `shift` AFNI's), and the median of all `|Δ|` ≤ m (degrees or mm): **k = 0.005 and m = 0.001** for the estimation methods heptic, quintic, cubic and Fourier (measured: within `φ_t` and `x_t·δ_n` alone, at most 0.013° and 0.0035 mm, cubic on `s1`); **k = 0.03 and m = 0.005 for `-linear`** (measured: up to 0.051° at a roll of 2.4°, which needs k = 0.015, and a median of 0.0031); the estimation method decides, not `-final`, and `-twopass`'s linear first pass does not count (its result only starts the final pass) |
| `-dfile`'s rmsold, rmsnew | rmsold identical (it compares the input with the base); rmsnew within `rms(y_AFNI − y)` (the two registered volumes' difference: the triangle inequality) `+ 1e-3·rmsnew` (the float32 sums and the `%11.4g` printing), for float32 outputs, `+ 1` for int16, taken from a run that writes the registered dataset; otherwise reported (measured: rmsnew moves by up to 11 % with `-linear`, 4.5 % cubic, 1.7 % heptic, 0.2 % Fourier on `s1`) |
| matrices (`-1Dmatrix_save`) | derived (below): rotation entries within `α_v + 1e-6`; translations within `τ_v + α_v·(|t| + |c_b|) + 1e-6·(|t| + |c_b| + |c_i|) + 1e-5·|u|` |
| maximum displacements (`-maxdisp1D`, the messages) | derived: each volume's within `α_v·r + τ_v + 0.001`, each delta within the sum of its volume's and the previous volume's bounds |
| `-savedisp` | derived: each voxel's three values within `α_v·|x − c| + τ_v + 1e-6·(1 + |u|)` |
| `-SAVEDISP` | in two steps, below: its normalisation and its 16 products exactly, on AFNI's own raw displacements; a whole run's 19 datasets by names, order, count, headers and types exactly, and by value only where the normalisation is well conditioned |
| registered volumes, float32 | (spec writer, 2026-11-08, step 3: the limit grows with each volume's rotation) for volume v, with `θ_v = |roll| + |pitch| + |yaw|` of AFNI's `-1Dfile` line (degrees) and `L_v = max(2e-3, 1e-3·θ_v)`: mean `|diff|` ≤ `L_v`·max `|AFNI_v|` (the volume's own largest value) and NCC ≥ `1 − 50·L_v²` (up to 2°: 2e-3 and 0.9998, as before), whether or not its parameters agree (the output's own resampling meets the ties); the largest single difference reported, not required. Measured: AFNI's own spread under the ties (§5.3) on fMRIPrep's call over 16 real runs reaches 0.68 of `L_v` and 0.16 of the NCC's allowance (ds000256 `nofeedback`: mean 3.4e-3 at 5.2°, 2.3e-3 at 4.2°; the earlier fixed 2e-3 and 0.9998 failed four of its volumes), the clean-room original the same; earlier measurements: NCC ≥ 0.99994 and mean ≤ 1.1e-3 with `-linear` on `s1`; single voxels at the volume's edges up to 2.9e-2 of the largest value with `-linear`, 1.7e-2 cubic, 5.9e-3 heptic on ds000005, 5.2e-2 Fourier on `nofeedback`, also where every parameter agrees within 1e-4 |
| registered volumes, int16 / uint8 | the same measures on the stored values; the fraction of voxels that differ by more than 1 reported |
| registered volumes, `-final NN` | (spec writer, 2026-11-08, step 3) the nearest-neighbour rule instead of the measures above: at every voxel where the original's output and AFNI's differ, the original's value is the input's value at a voxel within two voxels along each axis of the input voxel whose value AFNI's output holds there, the input taken as 0 outside its grid (so a 0 against a value is accepted when that value's voxel lies within one voxel of a face of the input grid: the edge of the rotated field of view); checked where the input's values are distinct (the suite's float32 series), elsewhere the share of differing voxels reported; the parameters as above. Each of the four shears rounds its rows' shifts down to whole voxels, so a tie decided the other way moves whole rows by one voxel, and two shears along the same axis by up to two: most voxels can differ (measured: AFNI's own tie variants change up to 81 % of a volume's voxels on `s1` with `-twopass -Fourier -final NN`, NCC 0.9917; over 3,656,469 differing voxels of five float32 cases (the two suites' `s1` series with `-twopass -Fourier`, heptic and `-linear -noclip`, and a 48 × 48 × 36 isotropic series), AFNI's tie variants (9 to 11 per case) and the original's 5 runs alike, none against the rule: 3,533,151 from a source voxel one voxel away, 198 two voxels away, 108,961 a 0 against a value from a face, 99 a 0 against a value one voxel inside it, 14,060 not checkable; a rule of one voxel would leave the 297 at two) |
| fMRIPrep's medians | (spec writer, 2026-11-08, step 3: the median's limit follows from the volumes') on §14.2's calls and the data panel's runs, against fMRIPrep's own `gen_avg` / `ref_average` output: mean `|diff|` ≤ `L_med`·max + q and NCC ≥ `1 − 50·L_med²`, with `L_med` the largest `L_v` of the run's volumes and q the stored median's step (its scale factor for an int16 output, 0 for float32); the largest difference and the share of voxels beyond 1e-3 of the maximum reported. At each voxel the median moves by at most the largest of its volumes' differences there (the median of the same number of values is 1-Lipschitz in each), so it cannot be held to more than the volumes; measured over AFNI's own tie variants on the 16 runs: at most 0.17 of `L_med` (ds000256 `nofeedback`: mean 9.0e-4, the largest single voxel 2.0e-2, 29 % of its voxels beyond 1e-3; on the runs with less motion the largest voxel 1.2e-6 to 3.5e-4, so the earlier 1e-4 on the largest voxel failed ds000228, ds005454, ds004958 and ds000256's baseline by AFNI's own ties) |
| headers, types, dimensions, geometry, time fields | identical |

Bit-identity is reported per category, not required. PLAN.md §11.3's motion-parameter criterion
(median ≤ 0.05 mm and 0.05°) is met by construction.

**Derived quantities** (spec writer, 2026-11-08). The matrices, the maximum displacements and
`-savedisp`'s displacements are functions of a volume's six reported parameters (§4.5, §8.8,
§10.3), so their thresholds follow from that volume's own parameter differences, which already
carry every tolerance. For volume v, with `Δθ_k` (roll, pitch, yaw; degrees) and `Δs_k` (dS, dL,
dP; mm) the differences of the two runs' `-1Dfile` lines (printed with four decimals, so the
float32 values differ by at most `|Δ| + 0.0001` each):
- `α_v = (π/180)·Σ_k (|Δθ_k| + 0.0001)` (radians) bounds the angle of the rotation between the
  two volumes' rotations (the angle of a product of rotations is at most the sum of their
  angles), and so every entry of the difference of the two rotation matrices and of their
  inverses (an entry is at most the difference's 2-norm, `2·sin(angle/2)`);
- `τ_v = √(Σ_k (|Δs_k| + 0.0001)²)` (mm) bounds the length of the difference of the shifts;
- so a point at distance ρ from the centre of rotation moves by at most `α_v·ρ + τ_v` between the
  two runs. `-savedisp`'s ρ is the voxel's distance `|x − c|` from the axis centre (§10.3); the
  maximum displacement's `r` is half the grid's diagonal, `0.5·√(Σ_n ((N_n − 1)·δ_n)²)`, which no
  edge voxel of the mask exceeds;
- `-1Dmatrix_save`'s translation is `−R⁻¹·(t + c_b) + c_i` (§4.5), so it moves by at most
  `τ_v + α_v·|t + c_b|`, with `|t|` the length of AFNI's `(dL, dP, dS)` and `c_b`, `c_i` the base's
  and the input's centres (§4.1);
- the small terms cover the roundings: `1e-6` the rotation entries' `%13.6g`, `1e-6·(|t| + |c_b| +
  |c_i|)` §4.5's float32 steps and `1e-5·|u|` the translations' `%13.6g` (|u| the larger printed
  value), 0.001 the two `%.3f` of `-maxdisp1D`, `1e-6·(1 + |u|)` `-savedisp`'s float32 storage.

With every parameter within its threshold, `α_v ≤ (π/180)·Σ_k (φ_t + k·|θ_k| + 0.0001)`: for the
default `φ_t = 0.02°` and small angles the rotation entries are within about 1.05e-3. This is the
threshold that follows from the parameter thresholds; the per-volume form above is tighter and
is the one checked (measured differences of the rotation entries: 5e-8 to 9e-4).

**`-SAVEDISP`.** Its normalisation (§10.3) divides each voxel's detrended series by its largest
absolute value m, so a difference e of the raw displacements becomes about e/m. For a motion close
to linear in time m is the size of the parameters' own small differences from that line, and the
normalised values turn differences far below the thresholds into any value between −1 and 1
(measured: on `s1`, whose volumes move by equal steps, the normalised displacements differ by up
to 2.0 and no voxel is well conditioned; on ds000005's 20 volumes 49 % of the voxels are with
heptic, 24 % with `-linear`). They are therefore validated in two steps:
1. **The arithmetic, exactly.** AFNI's `-savedisp` run with the same arguments gives the raw
   displacements that `-SAVEDISP` normalises (the same float32 values: AFNI is deterministic and
   normalises them in place before writing). The original's normalisation and its 16 products,
   applied to those raw datasets, must give AFNI's 19 `-SAVEDISP` datasets bit for bit (spec
   writer, 2026-11-08, step 3: §10.3's arithmetic, its double order for the line included, does,
   on every value of `s1` and ds000005; `savedisp.py` checks it, and with `--original` lists where
   an implementation's products depart from it).
2. **A whole run.** Names, order, count, headers and types exactly; the raw displacements
   (`-savedisp`) at the derived threshold above; the three normalised displacements `_DX`, `_DY`,
   `_DZ` by value only at the voxels where the normalisation is well conditioned: with `τ` the
   voxel's largest `-savedisp` bound over the volumes and `m` the largest absolute value of AFNI's
   detrended raw series, voxels with `m > 30·τ`, whose normalised values must agree within
   `6·τ/(m − 3·τ)` (the detrending is a projection that at most triples a difference, the
   division at most doubles it). The other voxels and the 16 products are reported (largest
   difference, correlation), not required.

**The ties' spread** (spec writer, 2026-11-08, step 3: the rule for ill-conditioned fits, where
the requirements above cannot hold because AFNI's own result moves beyond them with the ties,
§5.3 "AFNI's own spread under the ties"). The requirements above (the fixed ones) hold on
§14.1's named cases, on §14.2's calls and the data panel's runs, without any other rule. In
random testing (§14.4), whose small phantoms with sharp edges and larger motions make fits
ill-conditioned with respect to the ties, a case that fails them is judged by the original's own
spread under the ties instead, which an implementation measures from its own runs:
1. **The switch.** The original provides a validation-only setting that decides each tie by a
   seeded draw, independently at every resampling (every derivative image, every iteration, the
   output, every coarse rotation): `LARMORX_VOLREG_TIES=random:<seed>`, a uniform choice among
   the orders whose norms are within a relative 1e-12 of the smallest, from a generator seeded
   with `<seed>` and started afresh by every run (the model's `TIES = "random:<seed>"`, §15).
   Its global sides (`first`, `last`, an index) remain, but do not measure the spread (point 6).
2. **The spread.** K = 12 runs with seeds 1 to 12, each with the case's arguments in `reproduce`.
   For each quantity, the spread is the largest difference, over the 12 runs and over the case's
   volumes, between the original's default run and a seeded run, in the requirement's own units:
   each parameter's |difference| divided by its threshold (§14.3's per-volume thresholds, from
   AFNI's values), the median of all |differences| (degrees or mm), each registered volume's mean
   |diff| divided by AFNI's largest value of that volume, and 1 − NCC.
3. **The requirement.** Each quantity of the original against AFNI within the larger of its fixed
   requirement and 3 times its spread (9 times for 1 − NCC). The quantities derived from the
   parameters (matrices, maximum displacements, `-savedisp`, the numbers in messages) keep their
   derived bounds, which follow each volume's own parameter differences and so need no spread.
   Measured (the spread taken over AFNI's arithmetic with 12 seeded draws as a stand-in for the
   original's, the spec writer's scratch measurement): on the parameters and the registered
   volumes, 0 failures in 103 converging findings of a seeded sample of the trial (`oracles/difftest/2026-11-08-volreg-original-trial`) for the
   original's default as the centre, and also for AFNI's arithmetic with the first side
   everywhere as the centre; with 2 times the spread 1 failure; with each volume's own spread
   instead of the case's, 1 to 2 failures at 3 times. The model of §15 holds the rule with its
   own 12 seeded runs (`spread.py`): the result is in §15.
4. **Runaways.** A case in which AFNI's run or any of the original's runs (its default and its 12
   seeded runs) has a volume beyond 15° or 20 mm, or ends with AFNI-97 (AFNI's crash, or the
   original's AFNI-97 error with `reproduce`), is outside the small motions 3dvolreg is made for
   (§1), and the ties decide what happens in it: whether a fit runs away at all, which volume, and
   whether AFNI-97's condition is met on the way (measured over AFNI's own tie variants: in 6 of
   18 sampled runaways some variants stay below 15° and 20 mm; in a finding whose AFNI run stays
   at 10.7° all 15 variants go beyond 16°; in 7 findings where AFNI and the original ended with
   different exit statuses, AFNI-97's condition came in 2 to 12 of AFNI's 16 variants, AFNI's own
   run included or not). Required: when both end with exit status 0, the files written, their names, headers,
   types and dimensions, and the messages' text with their numbers masked; an exit status that
   differs only by AFNI-97 (AFNI's crash, or the original's AFNI-97 stop) accepted; the
   parameters, volumes and derived numbers reported, not required. Not required either: that
   both runs away (AFNI's own choices make that a coin toss).
5. **Fits that stop at their iteration limit** need no rule of their own: the ties' spread covers
   them (measured: all 20 sampled such findings within AFNI's own tie spread; such a fit oscillates
   between two points with increments that do not shrink, and the ties choose where it stops).
6. **The suite's re-examination with the global sides** (each of the four sides everywhere, or each
   volume taken from the side closest to AFNI's) is not a reading of "either side of every tie":
   AFNI decides each resampling its own way (§5.3: 190 and 144 of 334 ties on one run), so a
   global side is one point of the spread, not its extent; measured on the same 103 findings, 3
   times the spread of the original's four global sides still leaves 41 failing on the parameters
   alone (AFNI's result up to 1,356 times that spread away), against 0 with the seeded spread.
   On §14.1's and §14.2's cases the revised requirements need no re-examination (they hold for
   every tie choice measured, §15; the suite's record of 2026-11-08 failed, or declared expected
   divergences, only where they were revised: the volumes and median of high-motion runs and
   `-final NN`); in random testing the seeded spread above replaces it.
   The per-volume composite of global sides should not be used to pass a case.

**AFNI's own variability.** AFNI is deterministic and single-threaded: repeated runs, AFNI 25.2.09
and fMRIPrep's own AFNI give the same bits **(observed)**, so there is no natural spread. A band
for the final validation phase comes from perturbations that leave the problem unchanged (±0.5 LSB
noise on int16 inputs, a one-voxel crop, the base resampled by an identity, the volume order
reversed), as for mcflirt (`specs/mcflirt.md` §14).

### 14.4 Random testing, golden cases, memory

- **Random testing:** the descriptor `afni-volreg` (`validation/src/larmorx_validation/difftest/
  tools/afni_volreg.py`: random moving series and every option) gains the original as an
  implementation; one trial (`difftest --tools afni-volreg --budget 0.5`, boxed) before the gates;
  its differences triaged into `validation/difftest/triage.toml` (the replica's trial of
  2026-11-06: 39,751 cases, all agreeing after AFNI-97's correction). The replica's permanent cases
  are not read by the clean-room implementer (porting guide §1). (spec writer, 2026-11-08, step
  3) The original's cases are judged by §14.3's fixed requirements first and, where they fail, by
  "The ties' spread" (its 12 seeded runs, runaways by its point 4); a case beyond both is a
  finding to triage. What the trial's open verdicts were (a seeded sample of 1,200 of its 10,325
  differences, judged again by the clean-room part's `triage.py`), measured against AFNI's own tie
  variants on 123 of them (all 23 "parameters within their bounds but another check failed", 45
  of the 133 "converged and beyond", 20 "not converged", 20 "runaway", 15 "within the ties'
  spread"): 117 of the 121 comparable ones lie within AFNI's own spread (the original's distance
  from AFNI no larger than the farthest of AFNI's 15 variants; the 4 others at its edge, 6.90
  against 6.89 thresholds, 31 against 28, 218 against 211, 24.34 against 24.32), and the original
  taking the first side everywhere agrees with AFNI's arithmetic taking it too in 117 of 121 (all 4
  others runaways): the ties, not a missed behaviour. The 23, one by one: 8 fail only
  `-SAVEDISP`'s products (not the ties: §10.3's double order for the line, now stated); 7 the
  registered volumes (1 passes the revised fixed limit, 1 is a `-final NN` case within the
  nearest-neighbour rule, 51 differing voxels all from a neighbouring source voxel, 5 lie within
  the seeded spread); 6 the parameters' median (all 6 within the seeded spread: ill-conditioned
  phantoms whose parameters spread over up to 36 thresholds); 5 messages (3 pass with the final
  build, 1 is the sub-brick of a near tie, now defined in §14.3's messages row, 1 the coarse
  search's result, now reported, not required); 1 the parameters against the replica's default
  (1.05 thresholds, where the seeded spread is 0.98 thresholds). With the revised requirements every
  one of the 23 passes. (Measured with AFNI's arithmetic and its own tie variants: the spec writer's
  scratch tools, not kept.)
- **Golden:** at least one case per code path of the original (heptic, Fourier with `-twopass`,
  `-zpad`, the clipping off, int16, `-savedisp`), inputs ≤ 32³ × 4 of plain arithmetic, in
  `crates/larmorx-cli/tests/` beside the replica's.
- **Memory:** `tests/parity/test_memory_peak.py`'s `3dvolreg` call (fMRIPrep's call on a 64 × 64 ×
  40 × 20 float32 series) with the original: within 10 % of AFNI's peak plus 8 MiB.
- **Speed:** against AFNI and the replica on fMRIPrep's calls, at 1 and n threads (`validation/
  investigations/afni-volreg/speed.py` measures both programs from the command line).

## 15. The model (`validation/investigations/3dvolreg-spec/`)

`model.py` implements §4–§8 and §10.2 in numpy, statement by statement, from this spec only (the
automasks are larmorx's clean-room `lx.afni.automask(..., implementation="original")`, the
decomposition §5.3's stable formulas, the FFT numpy's); `check.py` runs AFNI 26.3.05's 3dvolreg as
a black box and the model on the same input and compares the `-1Dfile` lines, the
`-1Dmatrix_save` lines, the `-maxdisp1D` values and the registered volumes (`README.md` there:
how to run it). Not modelled: `-tshift`, `-rotparent`/`-gridparent`, `-savedisp`, `-weight`,
the FFT blur, the messages (`-wtinp` is modelled since step 3). The clean-room implementer may read and run it.

**Results (2026-11-07, 16 cases):** a synthetic moving series (40 × 44 × 24 × 5, float32, LPS
storage) with the default heptic method, `-linear`, `-cubic`, `-quintic`, `-Fourier`, `-final NN`,
`-noclip -zpad 0`, `-base 2`, `-twopass`, fMRIPrep's call, `-twopass -nocoarserot` (AFNI-86's
reading) and `-twopass -coarse 2 1` (the shift-only search); the same series stored RAS as int16
(default and fMRIPrep's call); and fMRIPrep's real input re-made from ds000005 run 1 (64 × 64 × 34
× 20, int16 with slope and intercept) with the default and with fMRIPrep's call.
- **With its own decomposition** (what a clean-room implementation of this spec gets): every
  parameter within 0.0012 (degrees or mm) of AFNI's (0.0098° with `-linear`, the roll at 2.4°;
  fMRIPrep's call at most 0.0002), 16 of 20 `-1Dfile` lines identical on ds000005 with fMRIPrep's
  call, the maximum displacements within 0.01 mm, the registered volumes within 2.7e-5 of the
  largest value (2.1e-4 with `-linear`; 0.6 % at one edge voxel of ds000005 with heptic).
- **With the decomposition's double values made AFNI's** (the spec writer's check of every other
  statement, §5.3 "Numerics"): **bit-identical** to AFNI in all 12 cases without Fourier (every
  `-1Dfile` and `-1Dmatrix_save` line, the maximum displacements and every voxel of the
  registered volumes), the two-pass cases with their coarse rotation search included; the four
  Fourier cases identical in every `-1Dfile` line and maximum displacement, their volumes within
  1.2e-6 (numpy's FFT, not AFNI's) and their int16 volumes within ±1 in 22 of 211,200 voxels.

**Added 2026-11-08** (spec writer; the implementer's questions on §14.3, §7.3, §8.9, §9.3 and
§10.3):
- `model.py`'s `TIES` takes either side of §5.3's exact ties (`"smallest"`, the spec's rule on
  the model's own evaluation, as before; `"first"`, `"last"`: the first or the last order within
  a relative 1e-12 of the smallest norm).
- `thresholds.py` runs AFNI through the oracle cache (`afni_run.py`) and the model with each of
  the three on 15 cases (§14.3's list) and holds every requirement of §14.3 against them,
  `-savedisp` and `-SAVEDISP` included (the model's displacements computed by §10.3's formula
  from its parameters): 0 failures in 45 runs. It also prints what the earlier fixed thresholds
  would have failed (the matrices' 1e-5 in every case but fMRIPrep's calls, the maximum
  displacements' 0.01 mm with `-cubic` and `-linear`).
- `afni89.py` runs AFNI on the inputs of §7.3 (constant series of several sizes and values, a
  half-constant one, a base constant along one axis, an all-zero weight, with and without
  `-twopass`, `-edging`, `-twoblur`, `-delta`) and prints its errors or the parameters it ends
  with.

**Added 2026-11-08, step 3** (spec writer; the implementer's questions after its records: §14.3's
volume and median limits on real high-motion data, `-final NN`, `-SAVEDISP`'s products,
ill-conditioned fits, the tie re-examination, the trial's open verdicts):
- `model.py`: `TIES = "random:<seed>"` (a seeded draw among the tied orders at every resampling,
  the generator started afresh by each run); `Settings.wtinp` (`-wtinp`, §8.7 step 2); the 2 × 2
  system of §5.3 solved by Cramer's rule where the elimination's pivot rounds to exactly 0 (a
  coarse rotation's exact sines and cosines; it stopped the model on 2 random-testing findings).
- `thresholds.py`: six tie choices per case (the three before and `random:1` to `random:3`), the
  revised requirements of §14.3 (each volume's limit `L_v` from its own rotation, fMRIPrep's
  median held to the volumes' limit, the nearest-neighbour rule for `-final NN`), and six more
  cases: `s1` with `-twopass -Fourier -final NN` and with `-wtinp`, and fMRIPrep's call on the
  data panel's runs with the most motion and the most ties (ds000256 `nofeedback`, ds000228,
  ds004958, ds005454, their inputs re-made by the clean-room suite in fMRIPrep's image).
  **0 failures in 126 runs** (21 cases × 6 tie choices). On `nofeedback` the model's volumes use
  up to 0.68 of `L_v` like AFNI's own variants, its median's mean 1.2e-4 to 8.7e-4 against
  `L_med` = 5.2e-3 (its largest voxel 1.4e-2 to 2.0e-2); on ds005454 the median's largest voxel
  is 1.1e-4 to 1.9e-4 (the earlier 1e-4 would fail every tie choice); `-final NN`: 55,946 to
  93,145 differing voxels per tie choice, none against the nearest-neighbour rule.
- `savedisp.py`: §10.3's arithmetic applied to AFNI's own `-savedisp` values against AFNI's
  `-SAVEDISP` datasets: 0 of 4,815,360 values differ on `s1`, 0 of 52,920,320 on ds000005 (the
  textbook forms of the line's coefficients: 3,983 normalised values differ on ds000005);
  `--original` compares an implementation's products with §10.3 applied to its own three
  normalised datasets (the original of 2026-11-08: 233 values on `s1`, 13,648 on ds000005).
- `spread.py`: §14.3's "The ties' spread" on random testing's findings: their inputs made again
  from the recipes, the command lines read into the model's settings (§9.1's scan; findings with
  `-tshift`, `-weight`, an external `-base`, `-float` or a `-twoblur` above 14 left out), AFNI
  through the oracle cache and the model 13 times (the spec's rule as the centre, 12 seeded
  draws). On the trial of 2026-11-08, the spec writer's sample of 123 findings and a seeded sample
  of 300 (`--sample 300 --seed 7`): 208 converging findings, every one within the rule (24 by the
  fixed requirements, 184 by the seeded spread: the model, too, is beyond the fixed requirements
  on most of these phantoms), 52 runaways, 1 input AFNI refuses, 162 with options the model does
  not implement; 0 failures.

## 16. Open points

- **Bit-identity needs AFNI's evaluation of the shear decomposition's norms** (§5.3,
  "Numerics"): the order AFNI chooses where two orders tie exactly decides every resampling of a
  grid with two equal voxel sizes, and AFNI's closed form is generated code, not a mathematical
  definition; this spec does not give it, and no rule found reproduces its choice (measured: in
  the derivative images AFNI's choice follows a fixed pattern, but taking that pattern there alone
  brings the results no closer; in the fit's iterations the choice follows the last bits of each
  matrix). The original is
  validated by the thresholds of §14.3. (For the orchestrator: if the user wants the original
  bit-identical on the polynomial methods, the spec could describe that evaluation's order of
  operations; the spec writer's recommendation is not to: the replica gives AFNI's bits, and the
  difference is within the thresholds that follow from the convergence thresholds, `-linear`'s
  wider ones included.)
- For Fourier the FFT's last bits are AFNI's own (§5.6) and not specified.
- (spec writer, 2026-11-08, step 3) The decision on bit-identity above stands (the orchestrator,
  2026-11-07): the requirements of §14.3 allow for AFNI's own spread under the ties, measured,
  and nothing more. The line's double order stated in §10.3 for `-SAVEDISP` is the same fit as
  `specs/3dTshift.md` §4 step 2's, whose text leaves that order open; for 3dTshift a different
  order can change an output value by one float32 step now and then (nothing magnifies it there),
  which its thresholds allow, so that spec is not changed here (for the orchestrator: it could
  gain the same sentence if 3dTshift's original is ever held to bit-identity).
