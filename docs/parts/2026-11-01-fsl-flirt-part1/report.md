# FLIRT replica, part 1: report (2026-11-01)

Branch `worktree-agent-a49385b701ec80acb` from main 7f6c43b, 16 commits to 1d2671f. The tree is clean and
nothing is running. No commit carries AI attribution; the guard was never bypassed. HANDOFF:
`<scratch>/fsl-flirt/HANDOFF.md` (sections 1 to 11); NOTES: `<scratch>/fsl-flirt/NOTES.md` (the resume point
and the part 2 list).

## What was built
- **Crate `crates-nc/larmorx-nc-flirt`:** `flirt` and `convert_xfm`, translated from FSL 6.0.7.23 (fsl-flirt
  2111.4).
  - Every cost function, including the weighted ones, labeldiff and BBR (its types, slope,
    coordinates/normals, field map).
  - The search, the pyramid, every schedule command, the default 3D/2D schedules and FSL's shipped schedules.
  - `-applyxfm`, `-applyisoxfm`, every output type, FSL's messages and exit statuses.
- **Refused with a message until part 2:** `-anglerep quaternion`, `alignpaxes`, parameter-subset masks other
  than FLIRT's own and the shipped schedules', and `-debug` with BBR.
- **Interfaces:** `larmorx-nc fsl flirt|convert_xfm`, and `larmorx fsl flirt|convert_xfm` through the replica
  dispatch (refused without larmorx-nc); `lx.fsl.flirt` and `lx.fsl.convert_xfm`; `docs/api/fsl-flirt.md`, the
  crate's README and PROVENANCE, FSL's source copied under `crates-nc/fsl-source/fsl-flirt/`.
- **Spec for the clean-room original:** `specs/flirt.md`.
- **Core additions:** weighted costs, labeldiff, unsmoothed entropies, the BBR module, FLIRT's filters and
  Powell's optimiser. Two later core fixes: the convolution reads beyond the image edges by the volume's
  extrapolation; BBR's 0/0 gives SSE's NaN.
- **mcflirt and BET after the final core change:** compare_records from 9272446 shows mcflirt 236/236 and BET
  178/178 identical to their committed records.

## Validation
- **Record `docs/validation/fsl-flirt-replica.md`** (standard tier, from clean commit 6913b97): 141 cases, 107
  bit-identical, 34 rejected by both programs alike, 0 failures. fMRIPrep's BBR calls on ds005040, ds003592 and
  ds000228 pass, and their matrices are identical as text to fMRIPrep's own.
- **Golden:** 10 crates-nc cases (`flirt.*`, `convert_xfm.*`), recorded from clean commits; the earlier
  checksums unchanged. `lx.fsl.flirt` and `lx.fsl.convert_xfm` registered as replica-only.
- **Memory (one thread):** fMRIPrep's BBR call FSL 452 MiB, replica 395; a 12-dof call FSL 259, replica 247.
- **Threads:** the FLIRT programs pass the thread-pool test, re-run with the final core.
- **Speed (informal, one thread):** default 12-dof FSL 4.6 s, replica 3.1 s; BBR FSL 46.6 s, replica 56.8 s.
- **New parity smoke test:** `tests/parity/test_fsl_flirt_replica.py`, 7 cases, all pass.

## Random testing
- **Descriptor:** `difftest/tools/fsl_flirt.py`. The trial (`<workspace>/oracles/difftest/2026-11-01-fsl-flirt`):
  3945 cases, 3009 agree, 928 differ, 8 over the memory limit.
- **Two causes fixed, each with a unit test and a permanent case:** `-applyxfm` blurred its input with zero
  outside the image (FLIRT sets the extrapolation before blurring; permanent case `applyxfm-blur-extraslice`,
  plus two real 4D cases in the suite); BBR with no boundary points printed `nan` where FSL prints `-nan` (the
  compiler had folded 0/0 into a positive NaN; permanent case `bbr-no-boundary-points-nan`).
- **After the fixes, 577 of the 936 findings agree.** The other 359 are in `triage.toml` as `unclear`, for part
  2: 281 where FSL corrupts its heap (aborts or segfaults) on small or constant images while the replica
  finishes (the cause not traced; once it is, the replica should refuse these inputs in both modes); 66
  matrices differ (64 with voxels below 0.5 mm or above 5 mm); 8 where the default `fix` mode changes the
  result without naming a defect (FSL-14 on infinite voxels; `-setbackground` with spline); single cases: one
  extra "Erroneous dof 3" message, one more heap corruption with `-bins 16`, 2 runs above 3 GiB.

## D13 defects (only IDs from the given range)
| ID | What | Decision |
|---|---|---|
| FSL-67 | BBR normals divided by the reference level's voxel sizes | rule D: kept until it is measured (part 2) |
| FSL-68 | errors after the command line exit with status 0 | rule B: exit 1 by default, `reproduce` exits 0 |
| FSL-69 | the image pair holds references across `setscale` | rule C: kept |
| FSL-70 to FSL-73 | undefined behaviour (bad matrix name, bbrstep below 1, unparsable `read`, `-pedir` without a field map) | rule B: refused in both modes |
| FSL-74 | usage text only | message kept as FSL's |

FLIRT also meets the existing FSL-09, 11, 14, 21, 56 and 66. No new entry needs a decision from the user.

## Gates
All pass: crates-nc fmt, clippy and test, golden included; root and crates-gpl gates (passed earlier; `crates/`
unchanged since); gates-python: 649 passed; ruff and SPDX; `tables --check`; parity smoke tier: 2615 passed.
The smoke tier's marker names 320e4ec, but the run started at 435d03f: the marker takes `HEAD` when the run
ends (HANDOFF §11 suggests a fix to `parity_smoke.py`).

## Not done in part 1 (part 2)
- The trial's open findings, then a second trial.
- The site page and the full tier (fMRIPrep's 22 calls).
- Quaternions, `alignpaxes`, the general pseudo-inverse of parameter subsets, the `-debug` volumes, FSL-67's
  measurement, suite cases for the hang defects (they need oracle timeouts).
- The other programs: avscale, makerot, midtrans, rmsdiff, img2imgcoord, img2stdcoord, std2imgcoord,
  applyxfm4D, pointflirt.
- The clean-room original (another agent, from `specs/flirt.md`).

**Dependencies of FSL's scripts:** epi_reg (FAST, fslmaths, fugue/applywarp, convert_xfm, flirt);
standard_space_roi (flirt, fslmaths, fslroi); aff2rigid (avscale); flirt_average (flirt, fslmaths,
convert_xfm); pairreg (flirt with its pairreg schedules, bet).

## Merge note (orchestrator, 2026-11-01)
- The parity smoke tier's marker now refuses a run whose commit or build moved before its end
  (`tests/parity/conftest.py`, `head_and_build`; `scripts/orchestration/parity_smoke.py`), as HANDOFF §11
  suggested. The golden checksums recorded from 320e4ec are checked again by verify-merge's crates-nc
  golden test on the merged tree.
- The register's counts table recounted (292 entries; one **yes**, FSL-67, rule D; no **decide**).
- The site page was not part of part 1: part 2 makes it first (the per-part rule: each part's site page from
  its tests).
