# 3dvolreg spec writer, step 2 (2026-11-08): the clean-room implementer's eight questions

Worktree: <workspace>/larmorx/.claude/worktrees/agent-ae4835153236dfbf2
(branch worktree-agent-ae4835153236dfbf2, from main 2b453c1). State: DONE.

## Answers (one line each, behaviour terms; all written into specs/3dvolreg.md)
1. Matrix threshold (§14.3): the matrices, maximum displacements and -savedisp are now derived per volume from that volume's own -1Dfile differences: with alpha = (pi/180)*sum(|d angle|+0.0001) and tau = length of (|d shift|+0.0001), rotation entries within alpha + 1e-6, translations within tau + alpha*(|t|+|c_base|) + small rounding terms, max displacement within alpha*r + tau + 0.001 (r = half the grid diagonal), -savedisp within alpha*|x-c| + tau; with the parameters at their thresholds this is about 1.05e-3 for rotation entries (the old 1e-5, and the old 0.01 mm for max displacements, were stricter than the parameters they follow from).
2. -linear (§5.3, §14.3): it is the decomposition, no behaviour is missing: with two equal voxel sizes, pairs of shear orders have exactly equal norms, so rounding picks the order at every resampling (x-y-x vs y-x-y shears), and linear interpolation is very sensitive to that choice (with AFNI's order choice the model matches AFNI's -1Dfile exactly); thresholds for -linear: per volume |d| <= threshold + 0.03*|value| and median <= 0.005 (measured worst 0.051 deg, median 0.0031); other methods keep 0.005 and 0.001.
3. AFNI-89 (§7.3): reached exactly when the least squares fail in the spec's own arithmetic: an all-zero weight (any), or with -twopass a base that does not vary along an axis of 30+ voxels (edging >= 2 covers the 1.4-voxel shift's reach) when linear interpolation returns its value exactly in float32 (100, 37.3, 1, 7e5 do; 1000, 123.456 and a base constant along one axis only do not, and then AFNI exits 0 with meaningless parameters, as the 20x22x12 series does).
4. §9.3: confirmed: every -verbose -verbose setup number after the wtrim line (weight range, zero-weight count, workspace size, each derivative image's range) is over the trimmed box.
5. §8.9: confirmed: E2 is the centre of the input grid padded as -gridparent requires (slice-axis origin moved by -npad_neg*step, float32), before the origin moves to the gridparent's.
6. §8.9 step 5: confirmed: time offsets move, and "adjusting time-offsets" prints (with -verbose, when ndz != 0), only for a dataset with slice offsets.
7. -SAVEDISP (§10.3, §14.3): validated in two steps: (1) the normalisation and the 16 products applied to AFNI's own -savedisp raw values must give AFNI's 19 datasets bit for bit; (2) a whole run by names, order, count, headers and types exactly, raw displacements at the derived threshold, and the three normalised datasets only at well-conditioned voxels (m > 30*tau, within 6*tau/(m-3*tau)); the rest reported.
8. Nothing else under the notes' spec questions; from the same cause the spec also now states: -verbose -verbose fit numbers reported (setup lines exact, derivative ranges within 10 %), -dfile rmsold exact and rmsnew within rms(volume difference) + 1e-3, registered volumes NCC >= 0.9998 and mean |diff| <= 2e-3 of the max (single edge voxels reported), message numbers within the derived bound + 0.01.

## Commits
- 16d7e401 specs/3dvolreg.md: the clean-room implementer's eight questions answered (spec writer, 2026-11-08)

## Model folder (validation/investigations/3dvolreg-spec/): CHANGED
- model.py: TIES option (either side of the exact ties; default unchanged), afni_run.py (AFNI through the
  oracle cache), thresholds.py (every §14.3 requirement, 15 cases x 3 tie choices: 0 failures),
  afni89.py (the AFNI-89 probes), README updated. All written from the spec and black-box runs only.
- Not committed (scratch only, restricted): afni_decomp.py (AFNI's generated shear formula, used to
  show that the order choice alone decides).

## Notes for the orchestrator
- Bring specs/3dvolreg.md and the whole model folder into the clean-room branch before relay 3.
- The model's runs need python/larmorx/_core.abi3.so (copied from the main checkout's for this step).
