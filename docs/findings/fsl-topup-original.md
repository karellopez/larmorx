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

23. **The vb methods' iterations** (§7.5; probes `p1` to `q9` of `vb_probe.py`, tiny images of
    1 x 8 x 1 to 8 x 8 x 4 voxels through the cache, FSL's own resampled inputs taken from
    zero-field `jac` runs, `vbeff.sh`; the models `vbm.py`, `vbg.py`, compared by `vbt.py`): per collection and block, the first estimate per line
    `f_l = (Σ_a K_aᵀK_a + P)⁻¹ Σ_a K_aᵀ y_a` with `P` the circulant `(1, -4, 6, -4, 1)` (the
    line periodic) and `Σ_l` that inverse (`φ = λ = 1`, printed first); then each iteration
    prints `λ/φ, φ, λ`, sets `φ = (1e-10 + N_φ/2) / (1e-10 + (Σ|y_a - K_a f|² + Σ_l tr(K_lᵀK_l
    Σ_l)) / 2)`, then `λ = (1e-10 + N_λ/2) / (1e-10 + (fᵀQf + Σ_l tr(Q_ll Σ_l)) / 2)` (the new
    `φ` is not used by `λ`), then updates the lines one after the other in storage order
    (Gauss-Seidel: `Σ_l = (φ K_lᵀK_l + λ Q_ll)⁻¹`, `f_l = Σ_l (φ K_lᵀy_l - λ Σ_{m≠l} Q_lm
    f_m)`, each line using the lines already updated). A joint solve or a Jacobi sweep is
    0.4 to 8 % off (`vb5.py`). Blocks: `vb2D` each slice of each volume (slices outer, the
    `slice = s` line once per slice, then per volume 50 iterations and the final line);
    `vb3D` each volume (20); `vb4D` every volume at once (20; no coupling between volumes,
    `p6`). `N_λ` the block's voxels; `N_φ` every image's data points for `vb2D`, two per voxel
    for `vb3D` and `vb4D` whatever the collection holds (`q8`, three images: FSL-142); integer
    halves. Two collections: x's first, each with its own iterations, the restorations added
    (`q9`). With FSL's resampled inputs the model gives FSL's printed values to their six
    digits and its outputs to 1e-7 (single precision), and so does the original
    (`vbmkeff.py` and `vbmine.sh`: 18 probes, printed values equal or in the last digit, outputs
    1.2e-7).

24. **The vb prior** (`vbfit.py`, `vbfit2.py`: the stencil fitted to converged outputs, `Q f = (φ/λ) Σ_a
    K_aᵀ (y_a - K_a f)`): `Q = DᵀD`, `D = Σ_d w_d Δ_d` the second differences along the slice's
    axes (`vb2D`: centre 20, axial -8, diagonal 2, distance two 1 for unit weights) or the
    volume's (`vb3D`, `vb4D`: 42, -12, 2, 1), truncated at the image's edges (rows centred
    outside the image too: the centre is 20 or 42 at every voxel) and periodic along the phase
    encoding; `w_d = v_pe / v_d` (voxels of 2 x 3 x 4 mm: weights 1.5, 1, 0.75 along y, `p10`;
    1, 2/3, 1/2 along x, `p12`: every coefficient to 4 digits). FSL-143: the coupling of a voxel
    with the voxels one step along the phase encoding in the next and previous slices is `w_pe
    w_z`, half of `DᵀD`'s `2 w_pe w_z` (`p3`, `p4`: 1 instead of 2 along y; `q5`: along x), the
    rest as `DᵀD` (`z2` = 1, `p4`); the precision is then indefinite (its symbol is negative at
    zero frequency), `λ` turns negative in the first iterations on real images and `φ` and `λ`
    fall towards 1e-23 to 1e-206. The replica's default (the `larmorx-nc` program as a black box) is `DᵀD` itself
    (fitted: 2), which converges; the original's default is the same.

25. **vb's invalid voxels and its mask** (`p14`, `p16`: 8 x 8 x 8, the second image moved by half
    a voxel along x or z): no line is skipped; the moved images' values at the voxels outside
    the movements' mask enter the restoration, interpolated periodically (`vbskip3.py`, `vbext.py`: the
    periodic spline 4e-5 off in the printed values, mirrored, reflected, nearest or zero
    coefficients 0.2 to 3 off); the output is 0 at every invalid voxel, voxel by voxel
    (`at/vb2d-y`: 62 lines partly 0), not by whole lines as `lsr`'s.

26. **vb against FSL and the replica** (the suite's 18 `at/vb*` cases, real images of 32 x 32 x
    20): 8 pass, 2 both-error (FSL's own messages), 8 expected divergences: the three `*-long`
    (FSL-133, lines of 96), `vb3d-aniso`, `vb4d-four` and `vb3d-three` (FSL-143's divergence
    amplifies the moved images' last differences: printed values up to 2 % apart at 1e-23 to
    1e-206, `vb3d-three`'s image 1.4e-3 at p99 in slices 4 to 6; `vb4d-four`'s output is
    infinite at every brain voxel in FSL and in the original's `reproduce` run, the ratio of two
    underflowing precisions), `vb2d-aniso` (the printed values of the top slices part in the 4th
    digit during the iterations, 8e-4, and meet again: final values within 1e-5, image 1e-5).
    The original's default against the replica's default (`vbfixcmp.sh`, `vbfixcmp.py`: 12 cases): images within
    1.8e-5 at p99 (mean 1e-6), printed values within 1.2e-5 (`vb3D`, `vb4D`) and 8.4e-4
    (`vb2D`'s transients), so the 4-digit differences of `vb2d-aniso` are the iterations'
    sensitivity, not the model.

27. **The moved images** (`p17`, `p18`: 16 x 16 x 16 voxels moved by a translation, or by
    rotations of 0.05, 0.03 and 0.04 rad, `vbinterp.py`; `vb2d-aniso`'s images with a zero field):
    FSL's resampling is the periodic cubic B-spline to single precision (3.6e-4 and 8.5e-4 of
    values about 500; 2e-6 of the largest values of `vb2d-aniso`'s). On tiny images it is not:
    an image with an axis of 4 voxels or fewer is not interpolated at its own grid points (up
    to 1 % off on `p4`, 8 x 8 x 4; 2e-5 at the ends of 8-voxel lines, `p1`); no periodic,
    mirrored or reflected prefilter and evaluation explains it (`vb10.py`). Spec question; it
    does not reach images of 16 voxels or more per axis.

28. **FSL-144** (the orchestrator's decision, rule B): `jac` with a phase encoding along z
    leaves the image uncorrected (`jac-z`: the output is the input), and topup refuses such an
    acquisition (§3.2: `third element of pevec must be zero`), so no field for it exists: the
    default refuses (exit 1, naming FSL-144), `reproduce` gives FSL's output (the suite's
    `at/jac-z`: FSL's output in `reproduce` mode, the default's refusal).

29. **The quadratic B-splines' count** (random testing's trial of 2026-11-03: 486 cases with
    `--splineorder=2` whose `_fieldcoef` had another grid than FSL's; FSL's `_fieldcoef` of 45
    distinct `(n, k)` axes from the oracle cache, `coef_probe.py`-style listing in the trial's
    triage, and the layout fitted to FSL's `--fout` where the case wrote one): FSL lays
    `ceil((2n + k - 4) / (2k)) + 2` quadratic coefficients on an axis of `n` voxels at knot
    spacing `k > 1` (35 of 35 axes with `n >= 2k`; the cubic rule, `ceil((n + 1) / k) + 2`,
    which finding 16 had taken for both orders, agrees only where the two coincide, as for
    `opt/splineorder-2`'s 32 voxels at spacing 3), coefficient `i` centred at `(i - 1) k` as for
    the cubic (FSL's `--fout` reproduced to 1e-7 Hz from its `_fieldcoef` on 12 axes). Spec
    question (§4.2 gives one count for both orders).

30. **FSL-145: two quadratic coefficients on a short axis.** On every axis shorter than two knot
    spacings (`n < 2k`: 10 of the 45 axes, e.g. 10 slices at spacing 6, 16 voxels at spacing
    10) FSL lays 2 quadratic coefficients, centred at `-k` and `0`, where the rule above gives 4
    (`n = 2k` gives 5: no case between). Their support ends at `1.5 k`, inside the axis: FSL's
    field is forced towards 0 at the axis' far end (in FSL's own voxel order: the stored first
    voxel of an x axis it reads flipped), exactly 0 from `1.5 k` on, whatever the images show
    (trial cases 601 and 377 with `--fout`: FSL's |field| per voxel along the short axis falls
    from 10.5 Hz to 0.0, from 4754 Hz to 6.9; the original's default stays at 8.6 and 119 Hz).
    The input makes sense and the result is wrong (rule A): the default lays the axis with the
    count of every other size and names FSL-145 (`larmorx_core::defects::FSL_145`), `reproduce`
    lays FSL's two (bit-for-bit the same layout; `with_fsl145`). applytopup reads either count
    from a `_fieldcoef` file (the file's count decides). A topup run names FSL-145 when one of
    its levels' grids or its output's (`topup::meets_fsl145`) has such an axis.

31. **Random testing's trial** (2026-11-03, `oracles/difftest/2026-11-03-topup-original`: seed
    `topup-trial`, the replica trial's cases drawn again, so every FSL run was served from the
    oracle cache; 14,400 cases in 11 minutes, 12 jobs at one thread, 1.5 GiB per case; the
    build of `f1288d8`'s Rust): 7,401 agree, 6,999 differ. What it found besides the quadratic
    count and FSL-145, and what was done (`recheck.py` replays a sample of each group with the
    fixes: `<scratch>`'s re-check of 427 findings):
    - the suite's own mistakes, fixed: the transposed acquisition file (four rows) read as rows
      (277 cases ended in an exception); outputs named by a configuration file (FSL's
      `test_acqp` names `--fout=field_test` and `--iout=iout_test`) compared as exact files (127
      cases); `--trace` with `-v`, whose stages FSL prints inside a cost line (47 cases); the
      field of a quadratic `_fieldcoef` with the new counts;
    - every exact difference left is a consequence of the minimisations' paths: `--debug`'s
      attempt files (the number of Levenberg-Marquardt attempts at a level, 25 cases);
    - the metrics: in cases that estimate the movements, 70 to 98 % differ (fMRIPrep's call on
      the drawn pairs 89 %), with or without true movements, by medians of 4 to 13 times the
      thresholds: the movement derivatives of finding 14 (the original's are the cost's exact
      derivatives, FSL's are not), which the record's expected divergences already name; in
      cases without movements, 16 to 23 % differ (cubic splines). Their FSL band (FSL's run with
      its read-out times changed in their last float bit, `parity/fsl_topup.py`'s
      `_random_band`) is small (e.g. case 3693: 1.2e-4 for the corrected images' p99 against the
      original's 3.0): FSL's own result does not move, the original's path differs. In the
      cases looked at the problem is ill-determined (no regularisation, `Reg = 0`, with
      `--lambda` left out, or a few hundred voxels at the subsampled level) and the original
      ends at the same or a lower cost (3693: 203.02 against FSL's 204.46): the field differs
      where the objective does not determine it. Relay 5b's probes settled them (findings 32
      to 36, and the triage below).

32. **The mask of an oblique phase encoding** (`first_step.py` with `FIRST_STEP_ACQ`: trial
    case 3693's images with the rows `0.6 0.8 0` / `-0.6 -0.8 0`, and mixed with `0 1 0`; FSL's
    `--debug=3` masks): for a volume displaced along more than one axis no axis wraps; a voxel
    whose sample leaves the level image (`[0, n - 1]` in the level image's voxels) along a
    displaced axis is invalid. The rule reproduces FSL's mask after the first step voxel for
    voxel (190 of 190; the original had kept every voxel, 288, and so a cost 4 % lower at FSL's
    own field: 627.905 against 654.757); since then the oblique probes' costs agree to 6
    digits. A convention the spec leaves open (§4.3 names one phase-encoding axis): followed in
    both modes. Spec question.

33. **`--debug`'s images are in FSL's order.** FSL writes them as it holds them (radiological),
    with no qform or sform: for an input stored neurologically (a positive determinant, trial
    case 12747) its debug fields are mirrored along x against the input's storage, and the
    original flipped them back (its debug fields equalled FSL's mirrored to 1e-6). Now written
    unflipped (12747: 1.1e-6 of 6.6 Hz). The suite compares only the debug files' names, so
    the trial did not see it; a larmorx mistake, fixed.

34. **The scaled conjugate gradients' probe gradient** (trial cases 12747, 395, 2724, 12118 with
    `--debug=1 -v`; a probe build that printed the original's `γ`, `κ`, `μ`, `β`, `δ`, `α`):
    the first step of an SCG level whose start has `Reg > 0` was 1.43 times FSL's (12747: the
    same direction to 3e-7; the curvature probes `g(x + σd) - g(x)` equal to 1e-3), because the
    original's gradient at `x + σd` took `λ' = λ·SSD` of `x + σd`. FSL evaluates no cost there,
    so its `λ'` stays the current point's (the last cost evaluated): with that weight the four
    cases follow FSL's costs to 6 digits (12747's level-2 step 5696.38 = FSL's; before 6862.3).
    A convention the spec leaves open (§4.6: "SSD the current first term"): followed in both
    modes. In the trial's re-check sample its no-movement cubic group went from 3 to 10 of 20
    agreeing. Spec question.

35. **FSL-146: volumes encoded along different axes** (trial case 3693: four volumes along y,
    -y, x, -x; `first_step.py` on its volume subsets; temporary builds that wrote the
    original's per-volume gradient pieces, fitted to FSL's first gradient by least squares). A
    y pair or an x pair alone gives FSL's first gradient (5e-6); one y and one x volume gives a
    gradient 0.78 times as long, and FSL's own first Levenberg-Marquardt attempt then raises the
    cost (1604.65 to 4048.2) while the Gauss-Newton Hessian (FSL's `--debug=3` weights equal
    the original's to 4e-4) predicts a fall. FSL's gradient is reproduced to the float
    precision of its debug files (1e-4 of 17 to 28, all subsets, also with oblique phase
    encodings) by: per axis `a`, the Jacobian term summed over the volumes displaced along `a`
    of `(ū - u_i)(β_ia - β̄_a)` (`β_ia = t_i p_ia f_i`, `β̄_a = (1/m) Σ_k β_ka`): the volumes
    not displaced along `a` leave out their `(ū - u_j)(-∂ū/∂c)`, which the cost's own gradient
    has. With it, the original's reproduce runs follow FSL's costs to 6 digits (3693: 204.456
    against 204.458; its rejected 4048.27). The input makes sense (AP, PA, LR, RL acquisitions)
    and the result is wrong: the minimisers step along a direction that is not the cost's
    gradient and stop where it vanishes (rule A): the default keeps the cost's gradient (a unit
    test checks it against central differences of the cost) and names FSL-146 whenever some
    axis has displaced and undisplaced volumes; `reproduce` gives FSL's (on those axes only:
    elsewhere its term is 0 but for rounding). Measured on the trial's differing four-volume
    cases: the default's final cost is lower than or equal to FSL's in 86 % (with movements,
    median 1.6 % lower) and 89 % (without). fMRIPrep's calls (one axis) are not concerned.

36. **Random testing's trial: the triage** (`triage_counts.py`: every one of the 6,999
    findings in one category by its recipe and first failed check; `recheck.py --per-group 20
    --seed 3` replayed 513 of them with each fix; `thread_band.py` measured FSL at `--nthr=4`):

    | Findings | Category | Reproducer |
    |---|---|---|
    | 4,462 | explained: finding 14 (FSL's movement derivatives; the record's expected divergences) | 6 |
    | 895 | FSL-146 (rule A; the reproduce runs agree: 17 of 20 without movements in the sample), and with movements finding 14 | 10280 |
    | 484 | explained: FSL-133 (applytopup's lines whose length does not divide 2^32) | 5 |
    | 429 | larmorx mistake, fixed: the quadratic B-splines' count (finding 29) | 11095 |
    | 276 | the suite's mistake, fixed: the transposed acquisition file | 27 |
    | 216 | partly the SCG weight (finding 34, fixed), the rest explained: the path of ill-determined fits (FSL's own band: at `--nthr=4` 0 on three of four, 5.3e-4 on 12118 against the original's 2.2e-3; cases just past a threshold, 2724: 0.11 Hz for 0.1) | 12118 |
    | 62 | the suite's mistake, fixed: outputs named by a configuration file | 12 |
    | 51 | explained: FSL-143 (vb3D/vb4D's indefinite prior, reproduced) | 275 |
    | 47 | open: quadratic splines' regulariser Hessian at an even knot spacing | 10753 |
    | 47 | the suite's mistake, fixed: `--trace` with `-v` | 1142 |
    | 17 | explained: `--debug`'s attempt files (the minimisation's path) | 2150 |
    | 8 | open: applytopup `lsr` on lines that divide 2^32 (p99 1.4e-3 to 0.13) | 8089 |
    | 4 | explained: integer outputs (`char`, `short`: values a rounding apart land on neighbouring integers) | 7321 |
    | 1 | explained: vb2D's transients (the record's `at/vb2d-aniso`) | 13346 |

    The open quadratic item: with `--splineorder=2` and an even knot spacing the knots fall on
    voxels, where a quadratic B-spline's second derivative jumps; the Levenberg-Marquardt
    Hessian's regulariser part differs from FSL's (`--debug=3`, case 10753: the data part
    equal to 2e-7, the regulariser's entries up to 3.4 % apart) while the first gradient and
    cost agree. Four knot conventions were tried in a probe build (the outer piece, the inner,
    the left or right limit, their average) and the exact integral and the image's voxels only:
    the average comes closest (Hessian 4.8e-6 apart instead of 1.4e-5) but none is FSL's. Spec
    question (§4.6: how is the bending energy of quadratic B-splines evaluated where a knot
    falls on a voxel?). Case 7283 (`--regrid=0`, a level of 4 x 4 x 4 voxels) differs from the
    first cost on, as finding 27's short axes do.

## The clean-room catch-up (2026-11-04): the spec answers of 2026-11-03

The original brought up to `specs/topup.md`'s answers (§13's table, lines marked "spec writer,
2026-11-03"): every FSL run through the oracle cache (`use` mode); the probes in
[`validation/investigations/cleanroom-topup/`](../../validation/investigations/cleanroom-topup/README.md)
(`catchup_probes.py`, `fsl201_spread.py`). Nothing here comes from FSL's source or the replica.

37. **FSL-201's `reproduce`** (§4.3, §4.6): the movements sampled through FSL's matrices
    (single-precision angle, glibc's `cosf` and `sinf` of it, the axis entry `q²`, each elementary
    rotation about the level image's centre with its product `(n − 1) v` in single precision,
    `M⁻¹` in double precision) and their derivatives FSL's forward differences of the sampling
    positions (steps 1e-4 mm and 1e-5 rad, the sum with the image's derivative rounded to single
    precision, divided by the step and rounded again, times the Jacobian); `--rbmout` writes FSL's
    matrices about the regridded image's centre. The first gradients agree with FSL's
    `TopupDebugGradient` files: `debug/movements` −298.408089, 54.363663, 14.391515 against FSL's
    −298.408083, 54.363534, 14.391763 (2e-5; the exact derivatives 0.4 to 15 % off), and all 17
    movement entries of the four-volume case's to 1e-6. The default (the exact derivatives) names
    FSL-201 on every run that estimates the movements; `reproduce` names it reproduced.
    **FSL's path is not determined beyond its inputs' last bits** (`fsl201_spread.py`): a
    rotation's gradient entry moves by about 2 % of its lever-weighted sum when the rounding of its
    angle to single precision flips by one unit (the four-volume case's second gradient: three of
    nine rotation entries 40 to 70 apart from FSL's, the other six within 1e-4, where the
    movements after the first step agree to 5e-10 rad). Changing every read-out time by 1, 16 and
    256 units of its last float bit moves FSL's own corrected images by (p99 relative, inside the
    brain) 0.00084 to 0.0021 (`opt/int16`, `opt/interp-linear`, `opt/levels`), 0.0017 to 0.0054
    (`cfg/b02b0_4`, `cfg/b02b0_7T`) and 0.021 to 0.047 (`opt/four-volumes`, 1 to 1024 bits); the
    original's `reproduce` runs with the same changes lie as far from FSL's unperturbed run
    (2.1e-5 to 0.0037; `opt/int16`'s k = 1 and `opt/levels`' k = 16 within 2e-5 of FSL's: the
    same path), except `cfg/b02b0_7T` (0.0079 to 0.0105, its `-v` costs following FSL's to 4 or 5
    digits level by level, the final within 3e-4) and `opt/four-volumes` (0.043 to 0.057). The
    default moves by 1e-7 to 1.4e-5 under the same changes (`cfg/b02b0_7T`): proportional to the
    change, as an objective's minimum should. The record lists the six as expected divergences
    with these numbers; `opt/scg-movements`, `opt/zero-readout`, `cfg/b02b0`, `cfg/b02b0_2`,
    `opt/defaults` and `difftest/fsl145-short-axis` pass. The suite takes FSL's own last-bit band
    (its run with the read-out times changed by one bit) for every case whose `reproduce` run
    names FSL-201.

38. **The mask follows the encoding, and the moved volumes keep the frame** (§4.3, spec writer
    2026-11-03): (a) a volume with a read-out time of 0 was framed as if encoded along no axis
    (the frame decided by the displacement `t p`, not by the encoding `p`): `opt/zero-readout`'s
    `-v` counted `n = 16200` where FSL's is 17280; with the encoding deciding, its costs follow
    FSL's to 5 digits and its movements to 2e-5 (the case passes, an expected divergence before).
    The same choice now decides FSL-146's trigger and `reproduce` (§4.6: FSL leaves out of an
    axis' Jacobian term only the volumes whose encoding along it is 0, whatever their read-out
    time: brief item 8; the case named FSL-146 where FSL's gradient is the cost's). (b) a moved
    volume's validity checked the moved position strictly inside the level's target grid and set
    no frame, so its mask grew where FSL's shrinks (`opt/four-volumes`: 16740 after the first step
    against FSL's 16200, then 16128 and 15947); now the frame by the voxel's index always and the
    sampling position within `[0, n − 1]` of the image interpolated, to 1e-8, along every axis but
    the one the image wraps along: the second cost equals FSL's to 6 digits.

39. **FSL-145's counts and its error** (§4.2): `reproduce` lays FSL's count, 1 coefficient when
    `n < k`, 2 when `k ≤ n < 2k` at `k ≥ 3`, the general count at `k = 2` from `n = 2` on (FSL's
    test: no coefficient added while `floor(n / k) k < floor(K / 2)`), and stops as FSL does where a
    refit has a single coefficient along an axis: at level 1 when it is subsampled (`Error occurred
    when preparing to run topup`), at a later level (`... level <l> of topup`), and at the outputs'
    refit after a subsampled last level, which a black-box run showed: `Error occured while
    writing output from topup` / `Exception thrown with message: Mat::operator(): index out of
    bounds`, exit 1, only the log written (`catchup_probes.py final-single`: 6 slices, `--warpres=44
    --subsamp=2`). `opt/fsl145-one-coefficient` is rejected by both alike. applytopup: the default
    reads a coefficient file of either count (naming FSL-145 when it reads the general count on a
    short axis), `reproduce` only FSL's: FSL aborts on the other (`terminate called after throwing
    an instance of 'TOPUP::TopupFileIOException'` / `what(): TopupFileIO::TopupFileReader::
    TopupFileReader: Unable to read file tu`, signal 6; `catchup_probes.py general-count` on the
    original's default `_fieldcoef` of `opt/fsl145-short-axis`).

40. **The regulariser's sampling of quadratic B-splines** (§4.6): each basis function sampled at
    the offsets inside its open support, the second derivative at an even spacing's inner knots
    the mean of its one-sided values (`-1 / (2 k²)`), the outer knots not sampled (both modes):
    random testing's reproducer 10753 agrees (the open item of finding 36).

41. **The spline prefilter's truncated start** (§4.5, §9.4; both modes): the recursive prefilter
    with initial sums of `min(N, 15)` terms and no normalisation, periodic for the images and
    mirrored for a field image's Jacobian (§7.3): reproducer 7283 (`--regrid=0`, a level of 4 x 4 x
    4 voxels) agrees; for `N ≥ 15` the coefficients are the exact ones to 1e-9.

42. **applytopup `lsr`'s matrices row by row** (§7.4; both modes): each observed voxel's extent
    mapped back by FSL's walk, the columns wrapping circularly: reproducer 8089 (lines of 16 voxels
    whose mapping folds) agrees. **FSL-133's `reproduce`**: the mapping at an index `k < 0` takes
    the field of voxel `(k + 2³²) mod n`: `at/lsr-long`, `at/vb2d-long`, `at/vb3d-long`,
    `at/vb4d-long` and reproducer 5 pass (expected divergences before); both modes name FSL-133
    where a restored line read the wrap-around and the two reads take different field values.

43. **FSL-134's field of another shape** (§7.3, `reproduce`): a coefficient field laid onto the
    images' grid in storage order for the displacement, its Jacobian from its own grid written at
    the same indices and 0 beyond: `at/jac-other-shape` passes (refused in both modes before; the
    default still refuses other grids); a field image of another shape is laid the same way, its
    Jacobian's spline derivatives evaluated at the images' voxels with zeros beyond its own grid
    (no case reaches it). The coefficient field's Jacobian takes §7.3's scales, the image's voxel
    size over the field's (1 on its own grid: no case changes); a field image's `reproduce` scales
    are FSL's, `dx / fx` and `dx / fy` (`dx` the image's x voxel size, `fx`, `fy` the field image's),
    the default's 1 (the field indexed as the image is: the Jacobian of the mapping applied).

44. **Random testing's reproducers of the spec answers, replayed** (`difftest --replay`, FSL from
    the cache): 10753, 8089, 7283, 5, 10280, 2724, 1142, 13346, 7321, 11095, 12 agree; 6, 12118
    and 3693 differ by the minimisations' paths of findings 31 and 37, 275 by FSL-143's divergence
    (finding 26), 2150 by `--debug`'s attempt files (the path). Two replays (6 and 27) report
    differences that the same cases run directly through the suite (`run_case`) do not show (6:
    `feat_fieldmap_mag`'s p99; 27: `jac_N: the same grid`): a difference of the replay's
    environment, to understand before the next trial (sub-task 6).

## The catch-up's random-testing trial (relay 8, 2026-11-05)

45. **The replay question of finding 44, answered.** `difftest --replay` runs a finding's
    *shrunk* recipe (`doc["shrunk"]`); the direct runs of finding 44 took its *drawn* one. On the
    same recipe the replay (`runner.judge_once`) and the suite (`run_case`) agree
    (`trial_replay_vs_direct.py`): both drawn cases agree, both shrunk ones differed. The shrunk
    case of 27 (1 x 1 x 1 voxel, `--jacout`) was the suite's mistake: nibabel reads the
    one-voxel Jacobian as shape `(1,)` (NIfTI's `dim[0]` drops trailing axes of one voxel) and the
    suite compared it with the mask's `(1, 1, 1)`, "another grid" on identical files; the suite
    measures such images on the mask's grid now (`validation/tests/test_fsl_topup_measures.py`).
    The shrunk case of 6 (1 x 1 x 3 voxels of 1 mm, three volumes, the default FWHM of 8 mm: a
    19-tap kernel on a 3-voxel axis) is the original's: `--featout`'s magnitude (here `--iout`'s
    mean of the first volume's smoothed prediction: every voxel lies outside the common mask)
    3.0 to 3.1 % above FSL's, identical to 1 ulp with `--fwhm=0` (`trial_f6_probe.py`): the
    smoothing of §4.4 on an axis shorter than its kernel, a question for the spec writer.

46. **`--miter` counts every iteration of the scaled conjugate gradients, and FSL-242** (observed
    with `-v`, `trial_fsl_verbose.py`). A level of SCG evaluates its start and exactly `--miter`
    steps, the rejected ones too: `topup-trial-11566`'s second level (three iterations) evaluates
    four costs in FSL, the first step rejected (311236 above 284996), where the original counted
    accepted steps and evaluated five. And a level whose last iteration is rejected ends at that
    rejected, worse point: `topup-trial-06854`'s second level (one iteration) rejects its only step
    (4120.88 above 1193.2) and FSL's third level starts from it (4164.96), not from 1193.2.
    The original counts every iteration in both modes (FSL's design, rule C); ending at a rejected
    point is FSL-242 (rule A, new): `reproduce` ends there, the default at the lowest cost the level
    found, the notice named on every run where that happens (`topup::minimise`'s unit test
    `every_scg_iteration_counts_and_a_rejected_last_one_is_fsl242`; permanent cases
    `scg-every-iteration-counts` and `scg-rejected-last-step`). With it both findings agree; of
    1,351 trial findings with an SCG level or an FSL configuration, 61 now agree
    (`trial_recheck.py`); the record keeps every status and detail.

47. **The trial** (`oracles/difftest/2026-11-05-catchup-topup`, seed `topup-trial`, 30 minutes,
    14 jobs at one thread, 1.5 GiB a case, the oracle cache in `use` mode: 17,535 cases, 13,300
    agree, 4,223 differ, 12 notices; of the 2026-11-03 trial's 6,999 differences drawn again,
    3,695 agree now). By category (`trial_categories.py`, the failed metrics against FSL's own band:
    its run with the read-out times changed in their last float bit): 3,213 estimate the
    movements and lie within 10 times that band (median 2.2): FSL-201's path, which FSL's own
    runs with 16 and 256 last bits changed spread as far on sampled findings (`trial_spread.py`);
    608 with the movements beyond 10 times the band, of which 179 run away (translations tens of mm
    from FSL's; 159 with an SCG level, which FSL warns against; in both modes; 98 already in the
    2026-11-03 trial; on `topup-trial-04946`, with `--debug`, the first gradients agree to 1e-7 and
    the field part of the second, at `x + σd`, to 9e-5, but its rotation entries part by 1.5 %:
    FSL-201's forward differences jump by about 2 % when an angle's single-precision rounding flips,
    which the scaled conjugate gradients divide by their small `σ`, so their curvature and step
    differ from the first iteration: FSL-201 with SCG, FSL's own end a rotation of 0.69 rad); 236 FSL-143 (applytopup vb3D, vb4D); 88 without movements within 10
    times the band; 24 cubic cases without movements beyond it (2 were finding 46's, 6 of 10
    sampled within FSL's wider spread, 07557 and 10619 open); 28 quadratic cases without movements,
    deterministic (FSL's band 1e-7 to 1e-5), 22 of them among the 109 cases with an axis shorter
    than two knot spacings (5 of the 145 others; `trial_quadratic.py`): §4.6's bending energy on
    FSL-145's layout, a question for the spec writer; 25 `--debug` attempt files (the path); 1 the
    suite's (a NIfTI-2 quaternion a float rounding above 1, read with nibabel's float tolerance
    now). The triage: `validation/difftest/triage.toml`.

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
