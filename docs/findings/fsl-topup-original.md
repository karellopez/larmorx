<!-- SPDX-License-Identifier: Apache-2.0 -->
# topup: what black-box runs of FSL 6.0.7.23 show (the clean-room original)

Observations made while writing topup's clean-room original (`crates/larmorx-fsl/src/topup/`,
`larmorx fsl topup`) from `specs/topup.md`, the papers and black-box runs of FSL 6.0.7.23's
`topup` only: FSL's outputs, its `-v` lines and its `--debug` files (each cost evaluation's
corrected volumes, Jacobians, field, mask, gradients and the Hessian's voxel weights), compared
with numpy prototypes. Every FSL run went through the oracle cache (`use` mode); the probes and
the scripts are in [`validation/investigations/cleanroom-topup/`](../../validation/investigations/cleanroom-topup/README.md).
Nothing here comes from FSL's source. Where an observation adds to or differs from the spec, it
is a question for the spec writer (the part's handoff).

## Settled (reproduced to single precision or to `-v`'s six digits)

1. **The images' spline is periodic along every axis** (spec §4.5 says "periodic along the phase
   encoding"). The first cost evaluation's corrected volumes (`TopupDebugUnwarped..._iter00`,
   ds005040's pair, one level with subsampling 2) agree with a periodic cubic spline along x, y
   and z, for the regridding and the level's interpolation, to 1e-6 relative; a mirrored,
   clamped or zero boundary along x and z misses by up to 7 %.
2. **The mask** (§4.3): with the phase encoding along y, the first and last voxels along x and z
   are 0 and those along y are 1 (`TopupDebugMask`: 32 × 30 × 18 = 17280 = `-v`'s `n`). The spec's
   "first and last rows along the phase encoding" reads as along the *other* axis; the
   original's rule: a voxel is valid when, for every volume, its position along each axis the
   volume is not displaced along lies strictly inside the level's grid (`0 < p < n - 1`).
   With the phase encoding along x (y edges), along both axes, and with movements: to be
   checked (relay 2).
3. **The coefficients' layout** (§4.2): with knot spacing `k > 1`, coefficient `i` is centred at
   voxel `(i - 1) k` (`ceil((n + 1) / k) + 2` of them); with `k = 1`, at voxel `i` (`n` of them).
   FSL's `_fieldcoef` evaluated this way gives its `--fout` to 5e-6 Hz (64 × 64 × 40, `k` = 5, 5,
   4, and `k = 1`). The layout is not centred: for 64 voxels and `k = 5`, the last voxels lie
   partly outside the last coefficient's support (the basis sums to 0.9987 at voxel 61).
4. **The displacement at a subsampled level** is `h · t · p` voxels of the *input*, so
   `h · t · p / s` voxels of the level (with `s = 2`, FSL's gradient is the derivative of its
   cost with the displacement in the input's voxels; with the level's, it is twice FSL's).
5. **The gradient** is the exact derivative of the cost: `-2 / (n (m - 1)) Σ_i Σ_mask r_i ∂u_i/∂c`
   with `r_i = ū - u_i`, plus `λ' · 2 R c` (`λ' = λ · SSD`, not differentiated): FSL's
   `TopupDebugGradient` files agree to 1e-6 relative (m = 2; with four volumes the factor
   `1 / (m - 1)` against `2 / m` is to be checked, relay 2).
6. **The Gauss-Newton Hessian's voxel weights** (`TopupDebugABC`, six volumes: the upper triangle
   of the 3 × 3 products of the weights of `B`, `∂_x B`, `∂_y B`) are `Σ_i (c_ip - c̄_p)(c_iq - c̄_q)`
   with `c_i0 = J_i e_i ∂f_i`, `c_i,1+a = e_ia f_i`; FSL writes them unmasked, its Hessian uses
   them masked, times `2 / (n (m - 1))`.
7. **The regulariser** is `E = Σ` over the integer voxel positions covering every coefficient's
   support of `h_xx² + h_yy² + h_zz² + 2 (h_xy² + h_xz² + h_yz²)`, derivatives per mm — a sum,
   not the integral (`-v`'s `Reg = λ · SSD · E` to five digits at three iterations; the integral
   misses by 6 %). Its Hessian in the Levenberg-Marquardt system is `2 λ' R` (fitted factor
   0.99997).
8. **The Levenberg-Marquardt step** is the preconditioned conjugate-gradient solution (from zero,
   the Hessian's diagonal as preconditioner, stopping when `|r| / |b| < 1e-3`) of the damped
   system: FSL's field after the first step agrees with it to 6e-5 Hz, and with the exact
   solution only to 0.08 Hz. The first three Levenberg-Marquardt levels of fMRIPrep's call
   reproduce `-v`'s costs to five or six digits.
9. **The scaled conjugate gradients** take one cost and two gradients per accepted iteration
   (`--debug`'s numbering: a counter incremented by each gradient); the textbook algorithm
   (Møller 1993 in Bishop's formulation: `σ₀ = 0.01`, `λ₁ = 0.1`, ×4 below 0.25, ×½ above
   0.75, restarted every `N` successes, Polak-Ribière) reproduces FSL's cost sequence to five
   digits from FSL's own start (`probe scg`).
10. **The corrected volumes** (`--iout`) are the full-resolution predictions times `g / 100`
    (`g` the mean of the volumes' means), 0 outside the mask; the mean is their mean inside the
    mask and the first volume outside it (ds005040: 5e-6 relative from FSL's coefficients).
    `--jacout` is `1 + t p ∂h/∂y` unmasked; `--dfout` is `h t p · voxel size` (mm) unmasked;
    both are written without qform or sform (codes 0, `pixdim[0] = 1`). `_fieldcoef` has the
    knot spacings as its voxel sizes, intent 2016 with the field's voxel sizes, a qform of code 1
    with the identity rotation and the field's size as its offset, no sform.

11. **The level transitions** (probes A to H: the half-resolution pair, two levels,
    `--debug=1 -v`; D, F, G and H end with a level of `--miter=0` on the input's grid, so their
    `_fieldcoef` holds FSL's refitted coefficients exactly):
    - nothing changes the field when neither the subsampling nor `--warpres` changes (probe A:
      only the smoothing changed; the next level starts from the same field to 2e-6 Hz);
    - a change of subsampling samples the field at the new voxels' centres, `(step - 1) / 2 +
      j · step` old voxels, as the spec says, and refits it with the knot spacing the
      *previous level's* `--warpres` gives on the new grid (20 mm on 6 mm voxels: 3; probe H);
    - a change of `--warpres` then refits the field at the new knot spacing, **even when the
      spacing stays the same** (probes E and G: the field changes by 0.014 and 0.0014 Hz rms);
    - **each refit** (per axis, one axis after the other) minimises the squared error at the
      voxels plus `0.005²` times the squares of **only the two second differences of the
      coefficients that wrap around the ends**, `c[n-1] - 2 c[0] + c[1]` and `c[n-2] - 2 c[n-1]
      + c[0]`: with it the refits of probes D to H give FSL's level-2 start fields to single
      precision (rms 1e-6 Hz or less), and fMRIPrep's call on ds005040 follows `-v`'s costs to
      five or six digits through the first four levels. The interior second differences take no
      part (with them, or with the differences continued by zeros beyond the ends, as §4.4
      reads, the refits miss by 0.002 to 0.023 Hz rms). The penalty acts on the end
      coefficients, which the samples constrain weakly (the 1D basis' smallest singular value
      is about 0.003 at knot spacing 2), and couples the two ends; a field in its own space
      therefore changes where those coefficients reach the grid. Found by fitting the refit's
      per-axis matrices to probe G's coefficients (alternating least squares: separable to 1 %
      of the change; the deviation from the identity only in the columns of the two end
      coefficients at each end) and testing penalties on those rows.

12. **The outputs' predictions keep the last level's smoothing** when that level is not
    subsampled (§6 removes "the smoothing and subsampling" only after a subsampled last level):
    in `thr/openblas`'s case (one level, `--fwhm=4`, no subsampling) FSL's `--iout` agrees with
    the predictions from the smoothed images to 1.5e-5 relative at the 99th percentile, and
    differs from the unsmoothed ones by 2e-2. With a subsampled last level both are removed;
    fMRIPrep's calls end with a level of FWHM 0, where the two readings agree.

13. **The movements** (`debug/movements`: ds005040's pair, one Levenberg-Marquardt level at
    subsampling 2 with the movements, `--debug=1 -v`; `opt/four-volumes`: rotations up to
    0.05 rad):
    - the rotation is `R = Rx(rx) Ry(ry) Rz(rz)` with `Rx = [[1, 0, 0], [0, c, s], [0, -s, c]]`,
      `Ry = [[c, 0, -s], [0, 1, 0], [s, 0, c]]`, `Rz = [[c, s, 0], [-s, c, 0], [0, 0, 1]]`: the
      four-volume case's `--rbmout` matrices to 1.5e-7 (every other order of the three misses
      by 1e-3 or more);
    - `--rbmout`'s matrix is `[R | t + c - R c]` with `c = ((n - 1) voxel) / 2` of the input
      (to 1.4e-5 mm; the remaining 5e-8 on the matrix' diagonal is FSL's, unexplained);
    - a volume is sampled at `R^T (x - c - t) + c` (the inverse of that matrix) plus the
      field's displacement, the field taken at the unmoved voxel and the Jacobian unchanged by
      the movement, **`c` the centre of the image interpolated** (`(n_img - 1) v_img / 2` of
      the level's subsampled, regridded grid: the spec's "in their own grid"): FSL's corrected
      volumes at three evaluations agree to 2e-6 relative (the input's centre misses by 1.5e-3,
      sampling at `M x` instead of `M^-1 x` by 0.14);
    - five parameters for the reversed scan (`tx, tz, rx, ry, rz`: the gradient files hold
      4693 coefficients and 5 movement parameters), the movements carried unchanged from level
      to level and to the outputs;
    - the mask is unchanged by the movements in every run seen (`-v`'s `n` constant); the
      original marks a voxel invalid when a moved position leaves the level's grid along an
      axis the volume is not displaced along (identical for the movements seen).
    With the exact derivatives (below), the original follows `debug/movements`' `-v` to five
    digits and its movements to 1.4e-4 mm and 4e-5 rad.

14. **FSL's rotation derivatives are not its cost's.** Its translations' gradient entries agree
    with the exact derivative to 1e-7 relative, its rotations' do not: at the start of
    `debug/movements` (zero field, zero movement) `-298.41, 54.36, 14.39` against the exact
    `-297.19, 62.44, 16.06`, and the same at the next two iterations; on the half-resolution
    pair 0.6 to 26 % in each of five settings (with and without regridding, subsampling,
    smoothing), so none of those causes it. Single-voxel probes on the replica as a black box
    (a residual at one voxel of a linear image, radiological and neurological storage, every
    edge) give the exact lever arms `x - c` everywhere; with a curved image the rotations'
    entries differ again, and a translation whose exact derivative is 0 (no variation along z)
    gets -7e-7: FSL's movement derivatives look numerical (`--trace` names
    `TopupScan::GetMovementDerivative`), exact for linear images. Rejected: lever arms in the
    target's, the image's or the input's coordinates and centres, voxel units, per-axis scales,
    centre shifts, mirrored axes, masks, finite differences of the cost or of the predictions
    (forward and central, steps 1e-6 to 0.05) and of the images. The original keeps the exact
    derivative (§4.6); its Levenberg-Marquardt and scaled-conjugate-gradient paths with the
    movements therefore differ from FSL's where the movements are not converged (the cases
    `opt/four-volumes`, `opt/scg-movements`; the scripts `m2_model.py` to `m13_fd.py`).

15. **A free parameter ends a Levenberg-Marquardt level.** With `--lambda=0` and a coefficient
    whose support lies outside the mask (the default `--warpres=10` on the half-resolution
    pair: knot spacing 2, the last coefficient along x covers only the masked edge), FSL prints
    the first cost only and writes a zero field and zero movements (`opt/defaults`,
    `cfg/directory`, `cfg/long-line`'s defaults; the probe `I` with `-v`): the original ends
    the level without a step when the Hessian has a zero diagonal entry.

16. **Options observed:**
    - `--interp=linear`: trilinear interpolation of the regridding and of every level (the
      derivative the difference of the two neighbours), periodic: `-v` identical to six digits,
      the corrected volumes to 0.06 of 7.6e4 (probe `L`);
    - `--regmod=membrane_energy`: the cost's and gradient's energy measures the first
      derivatives per *voxel* (the bending energy's per mm): `-v`'s `Reg` to five digits with
      scaled conjugate gradients (probe `J`) and Levenberg-Marquardt with FSL's Hessian per mm
      (probe `K`, FSL-117 reproduced: the field to 1.7e-8 Hz);
    - `--splineorder=2`: the quadratic B-spline on the cubic layout (coefficient `i` centred
      at `(i - 1) k`, `ceil((n + 1) / k) + 2` of them for 32 and 20 voxels at spacing 3: the
      spec's count gives 11, FSL writes 13), intent 2017; FSL's `--fout` from its `_fieldcoef`
      to 2.5e-6 Hz; the original's field within 0.01 Hz;
    - the log's `--estmov` is `100` when the option is not given (a list of `1` when given);
    - FSL-118 at the first level is reported as `Error occurred when preparing to run topup`
      (probe `M`: `--subsamp=2,3`), at a later level as `... level <l> of topup` after the
      earlier levels ran (`err/subsamp-mixed`);
    - a z phase encoding fails after the log is written (`opt/z-phase-encoding`);
    - `--debug`'s and `--trace`'s outputs: the names of the debug files (level, iteration,
      attempt; finding 9) and their kinds per level (1: `Field`, `Unwarped`, `Jacobians`,
      `MovementParameters`, `Gradient_`; 2: `Mask`, `MaskedDiff`, `ABC`, `GradientWithReg_`; 3:
      `Hess`, sparse triplets by columns with the last diagonal entry always); FSL's `MaskedDiff`
      is on the regridded image's grid (33 x 33 x 21 for the 32 x 32 x 20 pair), the original's
      on the level's; `--trace` lists FSL's internal routines (2814 lines for two levels), the
      original its own stages.

17. **applytopup's sampling and mask** (`--method=jac`; FSL's outputs of the replica suite's
    `at/` cases, served by the oracle cache; `at_jac.py`, `at_jac_pair.py` and
    `at_jac_first_volume.py` in `validation/investigations/cleanroom-topup/`, the same model in `crates/larmorx-fsl/src/topup/apply.rs`): image `i`
    is sampled at `M⁻¹ x + h(x) t (px, py, 0)` voxels (`M` the movement of its `--inindex`
    row about the field's centre `(n - 1) · voxel / 2`, as topup's `--rbmout`), the
    interpolation periodic along every axis (cubic B-splines; trilinear with `--interp`), times
    `J = 1 + t (px ∂h/∂x + py ∂h/∂y)` per voxel from the coefficients' own derivatives, the
    images averaged volume by volume. A voxel is 0 unless every image's position lies in `[0,
    n - 1]` along every axis but the phase encoding's when the encoding is along x or y alone
    (`jac-pair`: the 1629 zeros exactly), along every axis when it is oblique (`jac-oblique`:
    the 1932 zeros exactly). A phase encoding along z moves nothing (`jac-z`: the output is the
    input, rule C: the spec's `d = h t (px dx, py dy, 0)`). Relative differences p50 1e-7 to
    4e-7, p99 below 4e-5 (single precision in FSL); every `jac` case of the suite passes.

18. **A field image's Jacobian** (`jac-field`, `jac-field-aniso`; `at_jac_field.py`): the cubic
    spline of `h` with mirrored ends (`c[-1] = c[1]`), its derivative evaluated at the voxels
    with the coefficients beyond the image taken as 0 along every axis (to 3e-6 in `J`; a
    periodic or mirrored evaluation is 0.1 off at the edges). FSL-134's Jacobian scaling uses
    the **field image's** voxel sizes: a field of 6 x 6 mm on an image of 6 x 4 mm is not
    affected (`jac-field-aniso`), a field image whose header says 6 x 4 mm gets its y term
    times 6/4 (probe `probe134`, a new FSL run through the cache, `at_jac_field_scale.py`: `J = 1 + t py (vx/vy) ∂h/∂y`
    to 4e-6, the unscaled one 0.13 off).

19. **lsr's matrices** (§7.4; `at_lsr.py`, the 32-voxel probes through the cache, `at_wrap_compare.py`): the
    matrices `K` of §7.4 with `φ` linear between the voxels and the line periodic, `S`'s rows
    as the spec gives them, `f = (KᵀK + 0.01 SᵀS)⁻¹ Kᵀ y`, give FSL's restorations of lines of
    32 voxels to 1e-4 at the 99th percentile (`lsr-y`, `-x`, `-4d`, `-three`, `-trilinear`,
    `-zero-field`, `short-options`) and a single 1000 at the first, the last or a middle voxel
    of a line to 4 decimals (`wrap_probe.py` with 32 voxels: no line couples to another). The
    moved images (§7.4's first step) are valid along the axes other than their phase
    encoding's; a line with an invalid voxel is 0 in **every** collection's output, so a voxel
    whose x line is skipped is 0 even where its y line was restored (`lsr-four`: FSL's output
    is 0 everywhere; with only the skipped line's own voxels zeroed, 160 voxels differ).

20. **Movements as matrices** (`probe_mat`: the `y` source's movements written as their
    `--rbmout` matrices, 4 columns and 8 rows): `lsr` gives FSL's output with the movement
    parameters to 1.2e-6 (relative), so a matrix `M` moves its image by sampling at `M⁻¹ x`,
    as the parameters do; the original's output agrees with FSL's to 1.2e-5 at the 99th
    percentile.

21. **FSL-133 is not reproduced.** For lines of 30 voxels (`lsr-crop`, the probes of
    `wrap_probe.py` with a single 1000 at voxel 0, 1, 15 or 29 of a 30-voxel line) FSL's
    restoration differs from the circular wrap at the line's ends only (16.7 of 650 next to the
    1000 at voxel 0, 19.7 at voxel 29, 0 at voxel 15), and for lines of 96 (`lsr-long`) at
    the first three voxels only. The register's description (the displacement just before a
    line's first voxel read from the middle of the line, `(2³² - 1) mod n`: voxel 15 of 30,
    63 of 96) was tried: reading that voxel's field for `φ(-1)` (and for the observed row of
    a wrapped contribution) does not give FSL's numbers (`at_wrap_variants.py`), nor does any single
    value of `φ(-1)` or `φ(n)` fitted to the probes (`at_wrap_fit.py`: 36 and 19 left). The
    original wraps circularly in both modes (exact where FSL's wrap is circular, lines of 32)
    and says so in `reproduce` mode; `lsr-long` stays outside §12 (p99 relative 0.015) as an
    expected divergence; `lsr-crop`'s differences lie outside the brain. Spec question:
    what exactly FSL reads at the wrap-around.

22. **A field of the images' voxel count in another shape** (`jac-other-shape`: the `y`
    coefficients, 32 x 32 x 20, on an image of 64 x 16 x 20): FSL writes 0 at every voxel with
    x ≥ 32 and, elsewhere, values that neither the field laid as stored onto the image's grid
    nor the field evaluated at the image's own voxels give (p50 0.015 to 0.047, `at_other_shape.py`,
    `at_other_shape2.py`). The original refuses it in both modes (FSL-134's default refuses other
    grids anyway); a field with another voxel count gives FSL's message in `reproduce` mode
    (`Attempted to copydata with non-matching sizes`). Spec question.

## What the remaining differences are

With the transitions settled, fMRIPrep's two calls differ from FSL as FSL differs from itself at
another thread count (FSL-116, the replica part's evidence: FSL at `--nthr=4` against one, from
the oracle cache):

| Call | Metric (§12) | Original − FSL | FSL `--nthr=4` − FSL | §12 |
|---|---|---|---|---|
| ds005040 | field, p99 \|d\| in the brain (Hz) | 0.61 | 0.64 | 0.5 |
| ds005040 | `--iout`, relative \|d\| p99 | 1.0e-2 | 1.0e-2 | 1e-3 |
| ds005040 | Jacobian, p99 \|d\| | 8.2e-3 | 8.4e-3 | 6.9e-3 |
| ds005040 | displacement, p99 \|d\| (mm) | 0.025 | 0.026 | 0.021 |
| ds005040 | final cost (relative) | 7.8e-3 | 8.7e-3 | 1e-3 |
| ds001600 | field, p99 \|d\| in the brain (Hz) | 0.022 | 0.030 | 0.5 |
| ds001600 | `--iout`, relative \|d\| p99 | 9.4e-4 | 1.6e-3 | 1e-3 |
| ds001600 | final cost (relative) | 5.4e-4 | 9.2e-4 | 1e-3 |

On ds005040 the scaled conjugate gradients of the full-resolution levels (60 iterations, almost
no regularisation at knot spacing 1) amplify any last-bit difference: the original follows
FSL's costs to six digits for the first five iterations of level 4, then drifts as FSL at four
threads does (the final cost of the original 61.39, of FSL 61.88 at one thread and 61.34 at
four). Each of its metrics lies inside FSL's own band. ds001600 meets §12's thresholds.
(The final costs of the runs without `-v` are recomputed from `--iout`, `final_ssd.py`: the
last level has no smoothing and no subsampling, so its predictions are `--iout` times `100 /
g`.)
