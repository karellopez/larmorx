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
