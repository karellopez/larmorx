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
