# 3dvolreg spec writer, step 3 (2026-11-08): the clean-room implementer's questions after its records

Worktree: <workspace>/larmorx/.claude/worktrees/agent-ab0e98329b5a93c91
(branch worktree-agent-ab0e98329b5a93c91, reset to 17dc8e1 as briefed). State: see "Commits".

## Answers (one line each; all in specs/3dvolreg.md, marked "(spec writer, 2026-11-08, step 3)")
1. Volumes and median (§14.3): AFNI's own tie spread (AFNI's arithmetic, its choices replaced by 15 others) on fMRIPrep's
   call over 16 real runs reaches mean 3.4e-3 / NCC 0.99978 (nofeedback, 5.2 deg) and grows with rotation (<= 6.8e-4 per
   degree); new per-volume limit L_v = max(2e-3, 1e-3 theta_v) (theta_v = |roll|+|pitch|+|yaw|), NCC >= 1 - 50 L_v^2
   (worst use 0.68 / 0.16); the median held to the volumes' limit (mean <= L_med max + q, NCC >= 1 - 50 L_med^2; worst
   use 0.17), its largest voxel reported (AFNI's own ties reach 2.0e-2 on nofeedback and 1.5e-4..3.5e-4 on ds000228,
   ds005454, ds004958, ds000256-baseline, so the old 1e-4 could not hold).
2. -final NN: a nearest-neighbour rule: each differing voxel's value from an input voxel within two voxels of AFNI's
   source voxel, the input 0 outside its grid; 3,656,469 differing voxels of five cases (AFNI's variants and the
   original's sides), 0 against it (297 at two voxels: a one-voxel rule is not enough).
3. -SAVEDISP: §10.3's product order was right (any other association changes 25-70 % of values); what decides the few
   differing voxels is the line's double evaluation (a, b), now stated in §10.3: with it 0 of 4.8 M (s1) and 0 of 52.9 M
   (ds000005) values differ from AFNI; the textbook forms change 3,983 values on ds000005; the original departs at 233
   (s1) / 13,648 (ds000005) product values (savedisp.py --original).
4. Ill-conditioned fits: "The ties' spread": a validation switch LARMORX_VOLREG_TIES=random:<seed> (a seeded draw at every
   resampling), K = 12 seeded runs, each quantity within max(fixed, 3 x the case-wide spread) (9x for 1-NCC); measured 0
   failures on 103 converging trial findings (AFNI-arithmetic stand-in), the model's own: 0 of 208 (spread.py).
5. Tie re-examination with global sides: not acceptable: AFNI mixes sides per resampling (190 first / 144 other of 334
   ties on nofeedback); 3x the global sides' spread leaves 41 of 103 failing; replaced by the seeded spread (random
   testing) and the revised fixed limits (suite, fMRIPrep's calls).
6. Trial triage: 117 of 121 sampled findings within AFNI's own tie spread (4 at its edge), original side 0 == AFNI's
   first side within thresholds in 117/121 (4 runaways): ties, no missed behaviour. Runaways (any run beyond 15 deg /
   20 mm, or AFNI-97): structure only, exit status may differ by AFNI-97, numbers reported; "both runaway" not required
   (AFNI's own variants disagree: 6/18). Not converged: no own rule (all 20 within AFNI's spread). The 23: 8 SAVEDISP (the
   line's order), 7 volumes (1 revised fixed, 1 NN rule, 5 spread), 6 median (spread), 5 messages (3 pass now, 1 near tie
   -> defined, 1 coarse result -> reported), 1 vs replica (spread): all pass.

## Commits (branch worktree-agent-ab0e98329b5a93c91, on 17dc8e1)
- 4402541a specs/3dvolreg.md: the six answers; the model folder (model.py, thresholds.py, savedisp.py, spread.py, README)
- c6422e16 §15 and README: thresholds.py's results (21 cases x 6 tie choices: 0 failures in 126 runs)
State: DONE. Gates run on the folder: ruff check, ruff format --check, check_spdx: pass.

## Model folder (validation/investigations/3dvolreg-spec/): CHANGED
- model.py: TIES "random:<seed>"; Settings.wtinp; Cramer fallback for the 2x2 solve (LinAlgError on 2 findings).
- thresholds.py: 6 tie choices, revised limits (volume_limit, median from volumes, nn_unexplained), 6 new cases
  (s1-twopass-nn, s1-wtinp, ds000256-nofeedback/ds000228/ds004958/ds005454 fMRIPrep calls), --ties for splitting.
- savedisp.py (new), spread.py (new), README.
- Results: thresholds 21 cases x 6 tie choices: 0 failures (ds005454 split into 6 calls); savedisp 0 differences;
  spread 208 converging 0 failures.

## Scratch only (restricted, never committed)
tiesrc/ (a copy of the crates with a tie-choice hook in the replica's shear choice: VOLREG_TIEPOL afni/first/last/other/
index/rand/flipp/spec, VOLREG_TIELOG, VOLREG_TIESTAGES), tpr.py, tpr2.py, trial.py, sameside.py, spread.py (scratch),
summ.py, newlim.py, medfrac.py, labratio.py, nn2.py, the23.py, q3*.py, logrun.sh; runs/ cases/ trial/ outputs.
