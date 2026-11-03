# spec-answers-3: the spec answers for topup, FAST and FLIRT (2026-11-03)

The spec writer's answers to the questions of three clean-room originals: topup's
(`specs/topup.md` §13), FAST's (`specs/fast.md`) and FLIRT's part 2 (`specs/flirt.md` §14). Each
answer that states a precision is proved by a model written from the spec's new text alone,
which reproduces what FSL 6.0.7.23 computed. FSL runs only through the oracle cache (`use`
mode); FSL off the `PATH`. Run everything inside `scripts/orchestration/boxed.sh` (8 GiB is
plenty; 12 for `measure_fsl201.py`), with `PYTHONPATH=python:validation/src` and `python -P`.

## topup

| File | What it shows | How to run |
|---|---|---|
| `rotation_model.py` | §4.3 and §4.6: the movement matrices as FSL builds them (single-precision angle, cosine and sine, the axis entry `q²`) and its movement derivatives (a forward difference of the sampling positions, steps 1e-4 mm and 1e-5 rad): the movement part of FSL's three gradients of the replica suite's case `debug/movements` (`TopupDebugGradient_*`), against the cost's exact derivative. `--variant double-matrix`: the same forward difference of matrices built in double precision gives the exact derivative (the rounding, not the step, is FSL-201's cause) | `rotation_model.py <FSL run of debug/movements>` (made by `cleanroom-topup/fsl_case.py case debug/movements <dir>`) |
| `refit_model.py` | §4.4: the refit between levels (the samples, the intermediate knot spacing, the refit when `--warpres` changes at the same spacing, the penalty of only the two wrap-around second differences) against FSL's level-2 start fields and coefficients of the clean-room implementer's probes D to H. `--variant interior`, `zero`, `none`: the other penalties part from FSL | `refit_model.py <runs dir>` (the probes made by `TOPUP_RUNS=<runs dir> cleanroom-topup/probes.py D E F G H`) |
| `lsr_model.py` | §7.4: applytopup's least-squares restoration with the matrices built row by row (each observed voxel's extent mapped back by FSL's walk; the mapping at a negative index read as FSL-133 reads it) against FSL's restorations of the probe lines. `--variant circular`: the circular read (misses lines of 30 voxels); `--variant overlap`: the matrices column by column with every folded part counted (misses folded lines) | `lsr_model.py <dir of probe directories>` |
| `fold_probe.py` | the probes of lines whose mapping folds: 16 voxels, the field rising 1 or 3 Hz per voxel (at 3 Hz the mapping steps back across the wrap-around) | `fold_probe.py <dir>/p_2_<y>_2 <y> <slope> <t> [n]`, then `cleanroom-topup/at_case.py args <dir>/p_2_<y>_2 - -- --imain=pu.nii.gz,pd.nii.gz --inindex=1,2 --datain=acqw.txt --topup=tu --out=hifi` |
| `bending_model.py` | §4.6: the regulariser's matrix `R` (a sum at integer voxel offsets over each basis function's open support; a quadratic's second derivative at an even spacing's inner knots the mean of its one-sided values, the outer knots not sampled) against FSL's regulariser Hessian of random testing's reproducer 10753, measured from two first Levenberg-Marquardt Hessians (`--debug=3`) at `--lambda=1e-4` and `1e-3`. `--variant mean`, `inner`, `outer`, `integral`: the other readings part from FSL | `bending_model.py run <dir>` (FSL through the cache), then `bending_model.py check <dir> [--variant ...]` |
| `quad_count.py` | §4.2: the quadratic B-splines' count (FSL-145: 2 coefficients when `k ≤ n < 2k`, 1 when `n < k`; the closed form otherwise) on FSL's `_fieldcoef` of reproducer 10753 at `--warpres=50` and `100`, and FSL's error when a level refits a single coefficient (`--subsamp=2`) | `quad_count.py <dir>` |
| `short_axis_model.py` | §4.5 and §9.4: the image spline's prefilter with its truncated periodic initial values (15 terms) against FSL's resampling at the grid points of small images (two images, a zero field, `--method=jac`: the first image trilinear by FSL-132, so FSL's resampling of the second is `2 · out − image 1`). `--variant exact`: the exact periodic coefficients (the samples) | `short_axis_model.py probe <dir> <nx,ny,nz>`, then `cleanroom-topup/at_case.py args <dir> - -- --imain=p0.nii.gz,p1.nii.gz --inindex=1,2 --datain=acqw.txt --topup=tu --out=hifi --method=jac`; `short_axis_model.py check <dir>...` |
| `other_shape_model.py` | §7.3, FSL-134: a coefficient field of the images' voxel count in another shape: the field laid in storage order for the displacement, its Jacobian computed on its own grid and written at the same indices (0 beyond), against FSL's `at/jac-other-shape`. `--variant laid-jacobian`, `evaluated`: the other layouts | `cleanroom-topup/at_case.py cases <dir> jac-other-shape`, then `other_shape_model.py <dir>/jac-other-shape [--variant ...]` |
| `lsr_8089.py` | §7.4 on reproducer 8089 itself (lines of 16 voxels whose mapping folds): FSL's topup source, FSL's applytopup output, the replica's `reproduce` run, FSL's moved images (`jac` with a zero field), and the model of the text's matrices on them. `--variant overlap`: the original's column-by-column matrices | `lsr_8089.py run <dir>`, then `lsr_8089.py check <dir> [--variant ...]` |
| `measure_fsl201.py` | FSL-201, rule D measured: topup with FSL's movement derivatives (the replica's `reproduce`, checked against FSL's run from the cache) against the exact derivatives (the replica's default): final cost, field, movements | `measure_fsl201.py <work dir> case <suite cases>`, `... fmriprep`, `... fmriprep-fsl-config`, `... finding <finding JSON>...`, `... args <name> -- <topup arguments>` |

What the models gave (2026-11-03):

- `rotation_model.py`: FSL's first gradient (zero field, zero movement) `tx 13.532148, tz
  10.445746, rx −298.408083, ry 54.363534, rz 14.391763`; the model `13.532152, 10.445743,
  −298.408060, 54.363647, 14.391454` (rotations within 2.1e-5 relative); the exact derivative
  `−297.186642, 62.435591, 16.064168` (0.4, 15 and 12 % off). The next two gradients: the model's
  rotation entries within 1.5e-4 of FSL's, the exact ones 0.25 to 39 % off. `double-matrix`
  gives the exact derivative to 1e-4.
- `refit_model.py` (the wrap-around penalty): FSL's level-2 start fields to 2.4e-7 to 6.7e-7 Hz
  rms (largest 1.1e-5 Hz) on probes D to H, its coefficients to 1.8e-4 to 5.9e-3 (of 51 to 198:
  the end coefficients the float samples determine weakly); `interior` 1.1e-3 to 1.8e-2 Hz rms,
  `zero` 2.0e-3 to 2.3e-2, `none` 1.4e-3 to 2.6e-2.
- `measure_fsl201.py` (2026-11-03; the replica built from this branch at 60667ad; FSL's runs
  from the cache, new ones stored: fMRIPrep's inputs with FSL's `b02b0.cnf`, the three
  converged runs; `reproduce`'s movement file equal to FSL's byte for byte in all 22 runs). The
  final cost, FSL's → the exact derivatives' (relative):
  - fMRIPrep's two calls (sdcflows' `b02b0.cnf`, no movements): every output identical,
    FSL-201 not named;
  - the record's cases: `debug/movements` 81.8932 → 81.8935 (equal to `-v`'s digits),
    `opt/levels` −0.015 %, `opt/scg-movements` −1.51 % (movements 0.25 mm, 0.0048 rad apart),
    `opt/four-volumes` −0.093 %, `opt/zero-readout` equal, `cfg/b02b0` and `cfg/b02b0_2`
    +0.026 %, `cfg/b02b0_4` +0.004 % (one iteration per level), `cfg/b02b0_7T` −0.051 %;
  - random-testing cases with movements between the volumes (trial
    `2026-11-03-topup-original`, findings 6, 8, 25, 40, 46, 51, 70, 97): −0.29, −0.16, −0.081,
    −0.010, −0.13, −12.3, −0.19, −44.3 % (8 of 8 lower; finding 97's rotations 0.35 rad apart);
  - fMRIPrep's inputs with FSL's own `b02b0.cnf` (movements in its first five levels):
    ds005040 −0.55 %, ds001600 +0.13 % (movements 7e-5 mm and 2e-3 mm apart);
  - converged, one level: 30 Levenberg-Marquardt iterations on the half-resolution pair equal,
    on the four-volume image −0.015 % (0.10 mm, 0.0022 rad apart); 60 scaled-conjugate-gradient
    iterations on the half-resolution pair −38.4 % (FSL's stalls: the field 14.8 Hz apart at
    the 99th percentile).
  In all, lower or equal in 18 of 22 runs; higher by 0.004 to 0.13 % in 4 runs cut short.
  Decided: rule A (`specs/topup.md` §10.1).
- `lsr_model.py` (2026-11-03; FSL's runs new, stored in the cache: probe133's lines of 30 and 32
  voxels with the bright voxel at 0, 1, 15, 29 (`cleanroom-topup/wrap_probe.py <dir> 2 <y> 2 6
  <n>`), and `fold_probe.py`'s lines of 16 voxels, slope 3 Hz with the bright voxel at 0, 8, 15
  and slope 1 at 0, 8, read-out time 0.045 s): FSL's reading 7.4e-6 to 3.0e-5 from FSL on all 13
  lines (of 620 to 936); `circular` 3.1, 16.7 and 19.7 on the 30-voxel lines (exact elsewhere);
  `overlap` 111 to 383 on the three folded lines (and the 30-voxel lines' misses).
- `bending_model.py` (relay 1b, 2026-11-03; FSL's two runs new, stored in the cache): FSL's
  regulariser `R` from its two Hessians (5,054 entries, translation-invariant to 6 digits; the
  first cost's SSD 1009.54 in both runs): the text's sampling to 8.6e-7 relative (p50), 2.65e-5
  (largest), least-squares scale 0.9999991; the mean at both knots 4.8e-2 (p50, scale 0.986),
  the outer piece 1.4e-2, the inner piece 0.20, the exact integrals 0.17. A knot spacing of 2
  along z instead of 3 misses by 0.19.
- `quad_count.py` (FSL's runs new, stored): the closed form `ceil((2n + k − 4) / (2k)) + 2`
  equals the rule for every `n ≥ 2k` (k 2 to 29, n below 300); FSL's `_fieldcoef` 2 x 1 x 2 on
  24 x 16 x 14 voxels at spacings 20, 20, 13 and 1 x 1 x 1 at 40, 40, 25 (`--subsamp=1`), the
  rule's; at `--subsamp=2` both stop with `Error occurred when preparing to run topup` /
  `Exception thrown with message: Mat::operator(): index out of bounds`.
- `short_axis_model.py` (FSL's seven runs new, stored): FSL's resampling at the grid points
  departs from the samples by up to 5.68 (8 x 8 x 4), 0.0122 (1 x 8 x 1, at the line's ends),
  13.4 (4 x 4 x 4), 65.1 (2 x 6 x 3), 0.0742 (8 x 8 x 8), 2.6e-3 (12 x 14 x 10), 2.4e-4
  (16 x 16 x 16, the single-precision rounding) of values up to 999; the model differs from
  FSL's by at most 2.8e-5 to 9.1e-5 on all seven (single precision); the exact coefficients by
  the departures above.
- `other_shape_model.py` (FSL's run from the cache): the text's layout 1.1e-5 relative at the
  99th percentile (8.1e-8 p50; largest 0.0996 of 1.13e5), FSL's 10,784 zeros exactly; the
  Jacobian laid as stored too 0.21 (p99), the field evaluated at the image's voxels 1.18.
- `lsr_8089.py` (FSL's topup and applytopup runs from the cache; the two zero-field `jac` runs
  new, stored; the replica built from this branch): the model against FSL 5.6e-7 (p50), 3.2e-5
  (p99), 8.3e-3 (largest relative, at small values), the 2,464 zeros the same; `circular` the
  same (16 divides 2³²); `overlap` 0.131 at the 99th percentile (the original's trial value
  0.1337); the replica's `reproduce` output equals FSL's (largest difference 0).

## FAST

`specs/fast.md` §12 lists the questions and where each is answered (relay 2, 2026-11-03).

| File | What it shows | How to run |
|---|---|---|
| `fast_probes.py` | FSL's runs behind §4.1 (NaN voxels and the negative-value rule; a NaN first voxel, FSL-203), §7 (standard output at an abort) and §5.3 (a class without weight, case 1840), each against the prediction of a model of the new text (`model_extrema`, `model_restore`, `model_mask_empty`, the expected `-v` lines) | `fast_probes.py <out> [nan-noneg nan-neg nan-background nan-first n0-verbose nan-stats] [--finding <workspace>/oracles/difftest/2026-11-03-cleanroom-fast/findings/fsl-fast-original/cleanroom-fast-01840-original.json]` (derived inputs in `<workspace>/oracles/inputs/spec-answers-3/fast/`) |
| `variance_measure.py` | FSL-202, rule D measured: the single-channel class variance as FSL computes it against the exact one (the FAST replica with a measurement hook: `double`, `twopass`), beside FSL's own band (FSL with correctly rounded maths), on seven cases; JSON of every metric | `LARMORX_NC_BIN=<the hooked larmorx-nc> variance_measure.py <out.json> [case ...]` (12 GiB box); the hook is FSL-licensed replica code: `crates-nc/larmorx-nc-fast/dev-patches/variance_variant.patch` (a clean-room implementer does not open it), applied in a checkout, built, the binary copied out, the patch reverted |
| `variance_reference.py` | FSL-202, rule D's second measurement (relay 2b): the distance of FSL's outputs and of the corrected variance's (`double`) to a reference that computes the class statistics in double throughout (double products and sums, the variance about the double mean in two passes, each rounded once to single; everything else as FSL), each taken four ways (either run with the C library's `exp`/`log` or with correctly rounded ones, against the reference with either); and each computation's own band (the C library's `exp`, `log`, `expf`, `logf` against correctly rounded ones, the perturbation of FSL's band). The replica with correctly rounded maths and FSL's variance must equal FSL under the CORE-MATH preload bit for bit (the sanity row) | `LARMORX_NC_BIN=<the hooked larmorx-nc> variance_reference.py run <out dir> <case>` (the seven cases side by side, one thread each, 24 GiB box), then `variance_reference.py summary <out dir>`; the hooks (`LXNC_FAST_VARIANCE=double\|twopass\|reference`, `LXNC_CRMATH=1`) are FSL-licensed replica code: `crates-nc/larmorx-nc-fast/dev-patches/fsl202_reference.patch` (a superset of `variance_variant.patch`; a clean-room implementer does not open it), applied in a checkout, built, the binary copied out, the patch reverted |
| `sweep_schedule_model.py` | §10: a pipeline of planes (plane `z` two rows behind plane `z − 1`, one step's rows at once) gives the in-place sweep's bits; a lag of one row does not | `python -I sweep_schedule_model.py` |
| `pv_ties_model.py` | §5.8 and §4.8.8: the fraction search's near ties: how many voxels move, and by how many steps, when the class statistics change in their last bits or the energy is evaluated another way | `python -I pv_ties_model.py` (4 GiB box) |

What the models gave (2026-11-03):

- `fast_probes.py` (FSL's runs new, stored in the cache, but `nan-neg` and `nan-stats`: from it):
  `nan-noneg` (five NaN voxels, no negative value): FSL's `_restore` NaN at the five, `_bias` 1;
  `nan-neg` (the replica suite's `t1_nan`: the same voxels, one value −0.00024): `_restore` 0,
  `_bias` 1; the model predicts all ten. `nan-background` (every background voxel NaN) and
  `nan-first` (only FSL's first voxel NaN): exit 255, `-v` prints `1 0`, `2 0`, `3 0` and
  `Exception: Not enough classes detected to init KMeans`; the model: extrema NaN, mask empty.
  `n0-verbose` (`-v -n 0`): exit 134 (`Mixeltype exceeds allowed types`), standard output's 84
  lines end with ` BETA 0.1`; the partial-volume heading written before the abort is missing, as
  §7 predicts. `nan-stats` (case 1840): every class `MEAN 1` in the first main-loop iteration,
  `MEAN -nan` from the second; `nan-stats-N` (the same with `-N`): `MEAN 1` to the end; both as
  §5.3 predicts.
- `variance_measure.py` (FSL and FSL with correctly rounded maths from the cache, but
  ds000228's and the verbose case's new; the hooked replica from this branch at 0221658): the
  smallest per-class Dice of `_mixeltype` against FSL (voxels differing), band → exact variance:
  ds000005 0.99998 (7) → 0.99864 (112); ds000228 0.99976 (113) → 0.99985 (89); ds005003 0.99800
  (450) → 0.99623 (453); the half-resolution T1w 0.99988 (5) → 0.99993 (2); the bias-field case
  0.99970 (15) → 0.99988 (8); the 2D slice 0.99761 (2) → 1 (0); values above 2^24 0.99886 (29)
  → 0.99317 (346), there the partial volumes' 99.9th percentile 0.01 → 0.22 to 0.34. `double`
  and `twopass` give the same images on six cases, within two voxels on ds005003. `-v` on the
  half-resolution T1w, the last class line's standard deviations, FSL → exact: 226.856 →
  226.863, 109.413 → 109.422, 38.6653 → 38.6909 (the band: 226.843, 109.42, 38.6587).
  Decided: rule A (FSL-202, `specs/fast.md` §4.4, §9.1).
- `variance_reference.py` (relay 2b; FSL and FSL with correctly rounded maths from the cache,
  every run served; the hooked replica from this branch at 74466ed, one thread, `reproduce`).
  Sanity: the replica with FSL's variance and correctly rounded maths equals FSL under the
  CORE-MATH preload bit for bit on 7 of 7 cases (every label and partial volume). Label voxels
  that differ (`_seg` + `_mixeltype`) from the reference (the class statistics computed in
  double throughout), FSL as built (FSL with correctly rounded maths) → the corrected variance
  (`double`, either maths); then each one's own band (the C library's maths against correctly
  rounded ones), FSL → corrected:

  | case | FSL to the reference | corrected to the reference | band FSL | band corrected |
  |---|---|---|---|---|
  | ds000005 (fMRIPrep's call) | 54 + 112 (52 + 117) | 0 + 0 | 4 + 7 | 0 + 0 |
  | ds000228 (fMRIPrep's call) | 66 + 89 (56 + 46) | 0 + 0 | 122 + 113 | 0 + 0 |
  | ds005003 (fMRIPrep's call) | 391 + 453 (84 + 103) | 1 + 0 | 357 + 450 | 0 + 0 |
  | the half-resolution T1w, `-v` | 5 + 2 (7 + 7) | 0 + 0 | 2 + 5 | 0 + 0 |
  | the bias-field case | 2 + 8 (2 + 7) | 0 + 0 | 4 + 15 | 0 + 0 |
  | the 2D slice | 1 + 0 (0 + 2) | 0 + 0 | 1 + 2 | 0 + 0 |
  | values above 2^24 | 137 + 346 (148 + 341) | 0 + 0 | 17 + 29 | 0 + 0 |

  Partial volumes: FSL's largest difference from the reference 0.01 to 1.0 (99.9th percentile
  0.34 above 2^24, at most 0.01 elsewhere; 4 to 5,179 voxels differ); the corrected variance's
  0.01 at 0 to 92 voxels; its band 0.01 at 1 to 37 voxels (FSL's band up to 1.0 at 14 to 7,126
  voxels); the reference's band 0 on every output of every case. `-v` on the half-resolution
  T1w, the last class line's standard deviations: FSL 226.856, 109.413, 38.6653; corrected and
  reference both 226.863, 109.422, 38.6909. Closer to the reference on 7 of 7 (fMRIPrep's three
  calls included), band no wider on 7 of 7: rule A confirmed (FSL-202). Most of FSL's own
  last-bit band is FSL-202's cancellation.
- `sweep_schedule_model.py`: on six random grids (4 to 8 voxels a side, 80 % mask) the pipeline
  of planes equals the serial sweep bit for bit on 6 of 6; a lag of one row on 0 of 6 (72 to 141
  voxels differ).
- `pv_ties_model.py` (200,000 mixed voxels each): two channels, the statistics changed by 1e-7,
  1e-6, 1e-5: 22, 143, 1,387 voxels move, every one by one step; the energy through the explicit
  2 x 2 inverse instead of a solve: 0 move. One channel: 22, 375, 3,361, every one by one step.

## FLIRT (part 2's questions 1 to 20, relay 3a)

| File | What it shows | How to run |
|---|---|---|
| `flirt_probes.txt` | FSL 6.0.7.23's black-box runs behind questions 1, 2, 6 to 10 (`makerot -o`, `avscale --inverteddies`/`--allparams`, `convert_xfm -omat` of odd matrix files, `img2imgcoord` on odd coordinate files, `-warp` of a 4-volume image, `midtrans` with non-finite determinants, `pointflirt` with `nan` and with missing files), one per line in the format of `cleanroom-flirt/programs2_fsl.py` | `cleanroom-flirt/programs2_fsl.py flirt_probes.txt > <probes printout>` (FSL through the cache) |
| `flirt_probes_trace.txt` | question 20: `-anglerep quaternion` registrations of the suite's pair at `-verbose 7`, with `-dof 6` and with 12 dof | `cleanroom-flirt/programs2_fsl.py flirt_probes_trace.txt > <trace printout>` |
| `flirt_models.py` | the models from `specs/flirt.md`'s new text: `makerot` (§15.4, §3's elementary rotation of the whole angle vector), `avscale` (§15.1: §3's decomposition and composition about the origin, `((M⁻¹·R)·K)·S` with §12's inverse and products), the readers (§2.2's matrix files, §15.2's triples), FSL-90's message count (§8.1 step 9: one per composition of a vector part outside the unit ball) | `flirt_models.py <probes printout> [--trace <trace printout>] [--findings <trial findings dir>]` (`--findings`: the part-2 trial's avscale findings without a volume, FSL served from the cache) |

What the models gave (2026-11-03; FSL's runs new, stored in the cache, but for the trial's avscale
findings, served from it): `makerot` 11 of 11 matrices FSL's to ten significant digits; `avscale`
16 of 16 printed blocks of the 3 probes and 234 of 234 of the 47 trial findings without a volume
(the check matrix's `-0.000000` residuals included; the 35 findings with a volume, whose centre is
the volume's centre of gravity, not modelled); the readers 14 of 14 `convert_xfm -omat` files or
errors and 5 of 5 `img2imgcoord` coordinate lists; FSL-90's messages: with `-dof 6` FSL prints 8,
the model 4 invalid starts + 4 evaluations whose vector part lies outside the unit ball = 8; with
12 dof 4 = 4 + 0 (the implementer's "composed twice at dof 6" fits the first count only by
chance). Black-box answers without a model: `-warp` of a 4-volume image prints `An error occured
while reading file`, exit 1, after the verbose lines; `midtrans` of a matrix whose determinant
overflows (`1e200` scales) uses it (NaN rows, exit 0), a singular one is `Could not read matrix`;
`pointflirt` gives the same bytes for `1 2 nan` and `1 2 0`, and two missing files crash it
(signal 11: FSL-204).
