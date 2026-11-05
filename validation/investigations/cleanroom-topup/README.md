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

### applytopup (relay 3, findings 17 to 22)

| Script | What it shows |
|---|---|
| `at_case.py` | FSL's applytopup runs through the oracle cache (use mode): every `at/` case of the replica suite (`cases <outdir>`) or given arguments in a prepared directory (`args <outdir> <source> -- ...`) |
| `at_field.py` | the field and its derivatives of a `_fieldcoef` image on its grid (the helper of the others) |
| `at_jac_first_volume.py` | `jac-y`: FSL's first volume of the first image is trilinear whatever `--interp` says (FSL-132; p50 1e-7 against 0.016 for splines) |
| `at_jac.py` | the `jac` model on every `jac` case (sampling, movements about the field's centre, the mask, the Jacobian; finding 17) |
| `at_jac_pair.py` | `jac-pair`: the movement's sampling `R^T (x - c - t) + c` and the mask's axes (finding 17) |
| `at_jac_field.py`, `at_jac_field_scale.py` | a field image's Jacobian (mirrored spline, zero beyond the image) and FSL-134's scale by the field's voxel sizes (`probe134`; finding 18) |
| `at_lsr.py`, `at_lsr_where.py` | the least-squares restoration on every `lsr` case, and where it differs from FSL along the lines (findings 19, 21) |
| `wrap_probe.py`, `at_wrap_compare.py` | synthetic lines of 30 or 32 voxels with one bright voxel, FSL's restoration against the model (findings 19, 21) |
| `at_wrap_variants.py`, `at_wrap_fit.py` | FSL-133 hypotheses on the 30-voxel probes (the register's unsigned wrap, fitted wrap values): none reproduces FSL (finding 21) |
| `at_other_shape.py`, `at_other_shape2.py` | `jac-other-shape`: a field laid on a grid of another shape; no layout tried gives FSL's output (finding 22) |

Example: `python -P at_case.py cases <scratch>/at` (every case; FSL served by the cache), then
`python -P at_jac.py <scratch>/at/jac-pair` or `python -P at_lsr.py <scratch>/at/lsr-y`; the
probes: `python -P wrap_probe.py <dir> 2 0 2 6 30`, then `at_case.py args <dir> - --
--imain=pu.nii.gz,pd.nii.gz --inindex=1,2 --datain=acqw.txt --topup=tu --out=hifi`.

### applytopup's vb methods (relay 3b, findings 23 to 28)

Every script reads the probes from `VB_DIR` (a directory of your own, e.g. `<scratch>/vb`); FSL
runs only through the oracle cache (`at_case.py`, use mode), the replica as a black box
(`LARMORX_NC_PROGRAM`, the `larmorx-nc` program built from `crates-nc`).

| Script | What it shows |
|---|---|
| `vb_probe.py`, `vbprobe.sh` | synthetic probes (sizes, voxel sizes, constant or ramped fields, movements, 1 to 4 images, 4D) and FSL's runs of each method on them: `vbprobe.sh <name> <nx,ny,nz> <hz> "<vb_probe options>" <method>...` |
| `vbeff.sh`, `vbeff0.sh` | FSL's resampled inputs (zero-field `jac` runs: `(y0 + y_a) / 2`), which the models need on tiny images (finding 27) |
| `vbm.py`, `vbg.py` | the models: the stencils (`sten_w`, `stencil_Q`), the mean-field Gauss-Seidel VB (`gs_vb`, `vb_block`), the resampling matrices, FSL's inputs (`effective`) |
| `vbt.py` | the model against FSL's printed values and outputs: `vbt.py p4:vb3D q8:vb3D:count=two ...` (findings 23, 24) |
| `vb1.py` to `vb4.py`, `vb5.py`, `vb7.py`, `vb9.py` | the first update (the line prior, the traces), joint against Gauss-Seidel against Jacobi, the halved coupling and the distance-two term (findings 23, 24) |
| `vbfit.py`, `vbfit2.py`, `vb6.py` | the prior's stencil fitted to FSL's and the replica's converged outputs (finding 24) |
| `vbskip.py`, `vbskip2.py`, `vbskip3.py`, `vbext.py` | invalid voxels: no line skipped, periodic values, the output masked voxel by voxel (finding 25) |
| `vb8.py`, `vb10.py`, `vb11.py`, `vb12.py`, `vbinterp.py`, `vbl.py` | FSL's resampling of tiny and of 16-voxel images, and `lsr` on the probes (finding 27) |
| `vbmkeff.py`, `vbmine.sh`, `vbcmp.py` | the original on FSL's resampled inputs against FSL (finding 23) |
| `vbfixcmp.sh`, `vbfixcmp.py` | the original's default against the replica's default on the suite's `at/` cases (finding 26) |

### Speed (relay 4, `docs/benchmarks/fsl-topup.md`)

| Script | What it does |
|---|---|
| `speed_identity.py` | the original (`LARMORX_TOPUP_BIN`) on every case of the `fsl-topup` suite (its inputs as the suite prepares them; FSL is not run) in both modes at a given thread count, each output, stdout and stderr hashed: `speed_identity.py run <out.json> <threads> --jobs <n>`, then `speed_identity.py compare a.json b.json` (two builds or two thread counts, bit for bit) |
| `bench.py` | the benchmark: fMRIPrep's two topup calls and one applytopup `lsr` call with FSL (timed fresh through the oracle cache with a reason never to store: one thread and `--nthr=<all>`), the replica (`LARMORX_NC_BIN`) and the original on each `--threads`; `--json` for the report's data |

Example (in a memory box): `LARMORX_TOPUP_BIN=target/release/larmorx scripts/orchestration/boxed.sh 24
python -P speed_identity.py run <scratch>/t1.json 1 --jobs 14` (about 20 s), then the same at 7 and
14 threads (`--jobs 2`, `--jobs 1`) and `compare`.

## Random testing (relay 5)

| Script | What it does |
|---|---|
| `recheck.py` | a sample of each group of a trial's findings (kind, movements, quadratic splines, the first exact failure) replayed with `difftest --replay` on the drawn case (not the shrunk one), `--jobs` at a time, each in its own memory leaf; the verdicts per group |
| `estmov_off.py` | a sample of the trial's findings that estimate the movements, judged again with `--estmov=0` (FSL's variants run once through the cache): how many of the differences the movements' path makes |


## Random testing's triage (relay 5b)

Findings 32 to 36 of `docs/findings/fsl-topup-original.md`. Each runs FSL only through the oracle
cache (`LARMORX_DIFFTEST_CACHE_MODE=use`) and the original named by `LARMORX_TOPUP_BIN`; run them
inside `scripts/orchestration/boxed.sh`.

| Script | What it does |
|---|---|
| `first_step.py` | one finding's images (or subsets of its volumes, other acquisition rows with `FIRST_STEP_ACQ`, other options with `FIRST_STEP_ARGS`) run by FSL and the original with `--debug` and `-v`: the first gradient compared, the costs of every evaluation (`FIRST_STEP_MITER=own`), and with `FIRST_STEP_DEBUG=3` every debug image (Hessian weights, masks, fields). `LX_PROBE_*` variables reach the original (a temporary probe build's switches). FSL-146, the oblique mask, the SCG weight and the quadratic Hessian were found with it |
| `thread_band.py` | FSL at `--nthr=1` and `--nthr=4` and the original on findings: final costs and the corrected images' p99 (FSL's own thread band on random cases) |
| `triage_counts.py` | every finding of a trial in one category (its recipe and first failed check), with counts and a reproducer each (finding 36's table) |

`recheck.py` gained the group `mixed axes` (four volumes along y and x: FSL-146).

## The site (relay 6)

| File | What it shows |
|---|---|
| `trial_triage.json` | `triage_counts.py`'s output on the trial (`triage_counts.py <workspace>/oracles/difftest/2026-11-03-topup-original --json trial_triage.json`): the 6,999 differing cases' counts per category and the lowest finding of each; topup's page reads it for the triage table (`evidence/fsl_topup.py`, `trial_original`) |

The page's evidence: `python -m larmorx_validation evidence fsl-topup --site <site>`
(`evidence/fsl_topup.py`): the original's entries in the figures cache (`original-<call>`: the
original in reproduce mode at 14 threads and at one, its default mode at 14; `cases-original`:
every case of the record run again as the suite runs it, FSL served by the oracle cache, the
original's files compared with FSL's value by value, the record's metrics and FSL's own thread
band where the record measures it); `python -m larmorx_validation.evidence.fsl_topup original`
or `cases-original` computes them alone. Every page counted before and after with
`../cleanroom-bet/site_inventory.py count <site> <page>` and compared with its `compare`.

## The clean-room catch-up (2026-11-04, the spec answers of 2026-11-03)

Findings 37 to 44 of `docs/findings/fsl-topup-original.md`. FSL only through the oracle cache, the
original named by `LARMORX_TOPUP_BIN`; every run inside `scripts/orchestration/boxed.sh`.

| Script | What it does |
|---|---|
| `catchup_probes.py` | `final-single <dir>`: topup with `--splineorder=2` on 6 slices whose last level is subsampled, so the outputs' refit has a single coefficient along z (FSL: `Error occured while writing output from topup`, FSL-145); `general-count <dir> <coef dir>`: applytopup `--method=jac` on a coefficient file of the general quadratic count on a short axis (FSL aborts: `Unable to read file`) |
| `fsl201_spread.py` | one suite case with every read-out time changed by `k` units of its last float bit: FSL's runs (`fsl`), the original's `reproduce` runs (`lx`) against FSL's unperturbed run, and the original's default runs against its own unperturbed default (`fix`): FSL-201 makes FSL's path depend on its inputs' last bits (finding 37). `fsl201_spread.py <case> <new dir> 1,16,256 fsl,lx` |
| `lsr_speed.py` | applytopup `lsr` on ds005040's pair with FSL's topup result: the original's wall time at the given thread counts and its output against FSL's (0.13 s on one thread, 0.03 s on 14; FSL 0.42 s, `docs/benchmarks/fsl-topup.md`) |

`mine.py` takes the original's binary from `LARMORX_BIN` (default `target/release/larmorx`).

### The catch-up's random-testing trial (2026-11-05)

| Script | What it does |
|---|---|
| `trial_replay_vs_direct.py` | a finding of `fsl-topup-original` run as `difftest --replay` does (`judge_once`) and directly through the suite (`run_case`), on its drawn and its shrunk recipe: the replay runs the shrunk one (`trial_replay_vs_direct.py <finding .json>`, `LXT` the threads) |
| `trial_f6_probe.py` | finding topup-trial-00006's shrunk case (1 x 1 x 3 voxels, 3 volumes) on FSL and the original with `--iout` and `--featout`: the default, `--fwhm=0`, `--estmov=0`, both (`trial_f6_probe.py <dir with pair.nii.gz and acq.txt> <out>`) |
| `trial_f6_probe2.py` | the same pair tiled (x and y to 4 voxels, z to 24): which short axis makes the smoothed magnitude differ (confounded: FSL then estimates a field) |
