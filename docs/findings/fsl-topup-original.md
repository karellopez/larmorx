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

## Partly settled: the level transitions

Probes A to E (the half-resolution pair, two levels, `--debug=1 -v`) show:

- nothing changes the field when neither the subsampling nor `--warpres` changes (probe A:
  only the smoothing changed; the next level starts from the same field to 2e-6 Hz);
- **a change of `--warpres` refits the field even when the knot spacing stays** (probe E,
  warpres 20 to 22 at 12 mm voxels, both spacing 2: the field changes by 0.014 Hz rms and its
  energy from 20.8 to 23.7);
- a change of knot spacing on the same grid (probe B, 2 to 1) is reproduced by the refit of
  §4.4 with the second differences continued by zeros beyond the coefficients (0.002 Hz rms,
  energy 59.78 against FSL's 59.80); without the continuation (only the interior second
  differences) the edge coefficients are freer (0.0026 Hz, and much larger energies at the
  grid changes below);
- **a change of subsampling** (2 to 1) samples the field at the new voxels' centres, `(step -
  1) / 2 + j · step` old voxels, as the spec says, and refits it with the knot spacing the
  *previous level's `--warpres`* gives on the new grid (20 mm on 6 mm voxels: 3), then with the
  new level's: 0.015 Hz rms from FSL's next start (probe scg); any other intermediate spacing
  misses by 0.2 Hz or more.

Not settled: the refits' exact regularisation. The remaining differences (0.015 Hz rms in-grid,
the energy 258 against FSL's 268 in probe scg; 0.014 Hz in probe E) sit in the coefficients at
the grid's ends, which the samples constrain weakly. On fMRIPrep's call they make the
full-resolution levels start from a slightly different field (SSD 118.1 against FSL's 118.7 at
level 4), which the 60 scaled-conjugate-gradient iterations of the fine levels carry to the end:
the field's 99th percentile of the absolute difference inside the brain is 0.55 to 0.61 Hz (the
threshold 0.5) and the corrected volumes' relative difference 1e-2 at the 99th percentile (the
threshold 1e-3; their mean 2e-3), dominated by the Jacobian's (the field's derivative at knot
spacing 1, almost unregularised). Spec question: the refits' penalty (rows beyond the ends, its
weight) and the samples they use.
