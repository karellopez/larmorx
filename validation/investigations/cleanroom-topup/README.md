<!-- SPDX-License-Identifier: Apache-2.0 -->
# cleanroom-topup: black-box probes behind topup's clean-room original

The scripts the clean-room implementer of `larmorx fsl topup` used to read FSL 6.0.7.23's
behaviour from its outputs, `-v` lines and `--debug` files, and the numpy prototype the
conventions were checked with (`docs/findings/fsl-topup-original.md`). None of them reads FSL's
source or the replica's code. Every FSL run goes through the oracle cache (`use` mode), so a
re-run on another machine serves the stored outputs.

Environment: the validation package on `PYTHONPATH` (`<worktree>/python:<worktree>/validation/src`),
`TOPUP_RUNS` a directory for the runs (outside the repository), FSL never on `PATH`. Every run of
FSL or larmorx goes in a memory box (`scripts/orchestration/boxed.sh 8 ...`).

| Script | What it does |
|---|---|
| `fsl_case.py` | FSL's run of one case of the replica suite (`case <name> <dir>`), of fMRIPrep's call on an oracle run (`fmriprep <run> <dir>`) or of given arguments (`args <dir> -- ...`, `@name` the suite's derived inputs, `{imain}`/`{datain}` fMRIPrep's), through the cache; the streams in `STDOUT.txt`/`STDERR.txt`, the arguments in `ARGS.txt` |
| `probes.py` | the transition probes A to E (the half-resolution pair, two levels, `--debug=1 -v`) into `$TOPUP_RUNS/fsl-probe-<name>` |
| `err_cases.py` | FSL's streams for the suite's `err/` cases, compactly |
| `mine.py` | the original (`target/release/larmorx fsl topup`, `LARMORX_IMPLEMENTATION=original`) on an FSL run directory's inputs, with a comparison of `-v`'s lines and every output |
| `metrics.py` | §12's metrics between an FSL run and the original's on fMRIPrep's call (the field inside the brain, the corrected volumes, the Jacobians, the displacement fields) |
| `tp.py` | a numpy prototype of the estimation (one phase-encoding axis, no movements) |
| `proto1.py` | the first cost evaluation's corrected volumes against periodic, mirrored, clamped and zero boundaries (finding 1), the mask (finding 2) |
| `proto2.py` | the coefficients' layout from `_fieldcoef` and `--fout` (finding 3) |
| `proto3.py`, `proto8.py` | the gradient at a zero field and the Hessian's voxel weights against `TopupDebugGradient` and `TopupDebugABC` (findings 4 to 6) |
| `proto4.py` to `proto7e.py` | the regulariser's form against `-v`'s `Reg` and the regulariser's gradient (finding 7) |
| `proto9.py`, `proto9b.py`, `proto9c.py` | the first Levenberg-Marquardt step: the Hessian's factors and the conjugate-gradient solve (finding 8) |
| `t_probe.py`, `t_scg.py` | a Levenberg-Marquardt level and the scaled conjugate gradients from FSL's own start against `-v` (finding 9) |
| `t_iout.py` | the corrected volumes and their mean from FSL's coefficients (finding 10) |
| `t_trans.py` to `t_trans13.py` | the level transitions: sampling positions, intermediate knot spacings, refit penalties, energies (the transitions' section) |

Example: `TOPUP_RUNS=<scratch>/runs python -P fsl_case.py case debug/one-level $TOPUP_RUNS/fsl-debug1`
then `python -I proto3.py $TOPUP_RUNS/fsl-debug1`.
