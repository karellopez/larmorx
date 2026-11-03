# topup replica: investigations

Tools behind the findings of the topup replica (`crates-nc/larmorx-nc-topup`, FSL 6.0.7.23's
`topup`). The parity suite is `fsl-topup-replica`
(`validation/src/larmorx_validation/parity/fsl_topup_replica.py`); its derived inputs are made
once in `<workspace>/oracles/inputs/fsl-topup-replica/` (the suite's `inputs()`).

## `membrane_hessian.py`: FSL-117, measured (rule D)

With `--regmod=membrane_energy`, topup's cost and gradient measure the membrane energy with the
field's first derivatives in voxels, while the Hessian Levenberg-Marquardt solves with divides
each derivative by the voxel size (mm). The script runs the replica on the same cost both ways
(`reproduce`: FSL's Hessian, bit for bit FSL's run; `fix`: the Hessian in the cost's own units)
and prints the final cost of each (the last `Cost =` of `-v`):

    scripts/orchestration/boxed.sh 8 python -P validation/investigations/fsl-topup/membrane_hessian.py \
        crates-nc/target/release/larmorx-nc <workspace>/oracles/inputs/fsl-topup-replica <scratch>/memhess

Result on 2026-11-03 (ds005040's pair from fMRIPrep's working directory, half and full
resolution):

| case | FSL's Hessian (mm): final cost | the cost's units: final cost | change |
|---|---|---|---|
| half-1level | 162.486 (13 evaluations) | 162.482 (8) | -0.00 % |
| half-movements | 161.265 (13) | 158.592 (11) | -1.66 % |
| half-strong (lambda 0.5) | 162.572 (12) | 162.572 (27) | +0.00 % |
| half-levels (three levels) | 349.979 (40) | 348.266 (60) | -0.49 % |
| pair-1level | 191.566 (13) | 191.564 (27) | -0.00 % |

The consistent Hessian never ends at a higher cost and ends lower where the movements or
several levels are estimated: rule A, corrected by default (the minimised cost itself is
unchanged); `reproduce` gives FSL's steps.

## `vb_prior.py`: FSL-143, measured (rule A)

applytopup's undocumented `--method=vb3D` and `vb4D` regularise the restored lines with a
smoothness prior built from blocks of `AᵀA`, `A` the 3D second difference. The block that couples
a voxel with the voxels one step along the phase encoding in the neighbouring slices holds `sf₁`
where `AᵀA` holds `2 sf₁` (the same coupling within the slice holds `2 sf₀`, as it should). The
script (1) assembles the prior's precision matrix from its blocks on a small grid with FSL's block
and with `AᵀA`'s, and prints the smallest eigenvalues; (2) runs the replica on the suite's
`at/vb3d-y` and `at/vb4d-y` inputs in `reproduce` mode (FSL's applytopup bit for bit: the suite's
cases pass) and in the default `fix` mode, and prints the restored image's range:

    PYTHONPATH=<worktree>/validation/src scripts/orchestration/boxed.sh 8 python -P \
        validation/investigations/fsl-topup/vb_prior.py crates-nc/target/release/larmorx-nc <scratch>/vbprior

Result on 2026-11-03:

| | FSL's block (`reproduce`) | `AᵀA`'s (`fix`) |
|---|---|---|
| smallest eigenvalue, 8 × 6 × 5 grid, sf = (1, 0.909) | -2.739 (5 of 240 negative) | +0.4027 |
| sf = (1.5, 0.909) | -2.533 (4 negative) | +0.6095 |
| sf = (1, 1) | -3.005 (4 negative) | +0.4499 |
| `at/vb3d-y` restored image (input: 0 to 1.09e5, mean 2.18e4) | -3.85e10 to 3.53e10, mean 3.39e5 | -1.68e4 to 1.18e5, mean 2.13e4 |
| `at/vb4d-y` | the same | the same |

FSL's prior is not positive definite, and the variational-Bayes iterations diverge to values five
orders of magnitude beyond the data; with `AᵀA`'s block the prior is positive definite and the
restoration stays in the data's range (as `--method=vb2D`'s, whose prior is right). Rule A:
corrected by default; `reproduce` gives FSL's values.
