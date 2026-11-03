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
