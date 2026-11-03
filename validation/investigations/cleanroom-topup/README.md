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
| `probes.py` | the transition probes A to H (the half-resolution pair, two levels, `--debug=1 -v`; D, F, G and H end with a level of `--miter=0` at full resolution, so their `_fieldcoef` holds FSL's refitted coefficients exactly) into `$TOPUP_RUNS/fsl-probe-<name>` |
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
| `t_trans14.py` | a rejected hypothesis: the refits solved approximately by conjugate gradients (3D) change the field more than FSL's do |
| `t_trans15.py`, `t_trans16.py` | probes E and D: where FSL's refit changes the coefficients (the weakly sampled end coefficients; the 1D basis' smallest singular values, about 0.003 at knot spacing 2) |
| `t_trans17.py`, `t_trans18.py` | probes G and F (one refit each, FSL's exact coefficients): 18 penalty models (coefficients' or field's differences, zero-continued, periodic, interior, joint 3D) none of which gives FSL's change; the change in the normal matrix' eigenbasis |
| `t_trans19.py` | probe G: the refit is separable (per-axis matrices fitted by alternating least squares explain FSL's change to 1 %), and its deviation from the identity sits in the columns of the two end coefficients at each end, coupling the ends |
| `t_trans20.py` to `t_trans23.py` | the fitted per-axis matrices against wrap-around models: the penalty `0.005²` times the squares of the two second differences that wrap around the ends matches (finding 11) |
| `t_trans24.py` | that penalty against probes D to H: FSL's level-2 start fields to single precision (the zero-continued penalty misses by 0.002 to 0.023 Hz rms) |
| `final_ssd.py` | the last level's SSD recomputed from `--iout` (it equals `-v`'s last SSD): the final cost of runs without `-v`, FSL's at `--nthr=4` included |

| `mov_euler.py`, `mov_order.py` | the rigid-body matrix: the Euler order and signs, the centre of `--rbmout`'s matrices (`debug/movements`, `opt/four-volumes`; finding 13) |
| `mov_model.py` | the movement model against FSL's corrected volumes of `debug/movements` (the sampling `R^T (x - c - t) + c`, the image's centre; finding 13) |
| `mov_grad.py`, `mov_grad_at.py`, `mov_fit.py`, `mov_fit_lever.py`, `mov_brute.py`, `mov_mask.py`, `mov_fd.py`, `mov_fd_images.py`, `mov_trilinear.py` | the movement part of FSL's gradients against the exact derivative and the rejected hypotheses (lever arms, centres, units, masks, finite differences; finding 14) |
| `mov_replica_probe.py` | the replica (`LARMORX_NC_BIN`, a black box) on synthetic images with a residual at one voxel: FSL's lever arms (`SX=-1` radiological storage, `Q=1` curved images, `V0` the voxels, `MINE` the original for comparison; finding 14) |
| `mov_vs_replica.py` | the original against the replica: the first gradient's movement entries in five settings of regridding, subsampling and smoothing (finding 14) |
| `quad_layout.py` | the quadratic B-spline layout from FSL's `_fieldcoef` and `--fout` (`opt/splineorder-2`; finding 16) |

The probes of relay 2 (FSL through the cache with `fsl_case.py args`, then `mine.py`): `I` the
defaults with `-v` (finding 15), `J` and `K` the membrane energy with scaled conjugate gradients
and Levenberg-Marquardt, `L` trilinear interpolation, `M` FSL-118 at the first level, `N` and
`O` the oblique and `b02b0_7T` cases without the movements, `P` a negative `--fwhm`, `Q` a
negative `--lambda` (finding 16); `mine.py` takes `LXD=reproduce` for the original's mode.

Example: `TOPUP_RUNS=<scratch>/runs python -P fsl_case.py case debug/one-level $TOPUP_RUNS/fsl-debug1`
then `python -I proto3.py $TOPUP_RUNS/fsl-debug1`.
