# HANDOFF: the clean-room additions (FSL-20, FSL-21, FSL-65, FSL-66 in `lx.mri.hmc`; AFNI-75 in 3dUnifize's original), 2026-10-30

Branch: the worktree `agent-a91cbd5bb2b9e5d0e` (from main d2453b3). The exact texts for the shared
documents follow; each block says where it goes. No new defect IDs (FSL-65 and FSL-66 were added
to `larmorx_core::defects`, the code registry, from the register's summary rows).

## 1. `docs/findings/upstream-defects.md`: the summary rows (behaviour only)

Replace the five rows' "larmorx today" and "Gap" cells (the other cells unchanged):

- **FSL-20** — larmorx today: `the replica: an error by default (rule B, 2026-10-24), mcflirt's exit 0 with reproduce; the clean-room original the same since 2026-10-30 (specs/mcflirt.md §10.6: by default every text output opened before anything is written, an uncreatable .mat directory an error naming FSL-20; reproduce mcflirt's message, exit 0, nothing more written)` — Gap: `–`
- **FSL-21** — larmorx today: `the replica: refused by default (rule B, 2026-10-24); reproduce gives mcflirt's matrices and, for corratio, its abort; the clean-room original since 2026-10-30: refused by default before any work (CLI and Python API, whatever the series; -stages 0 runs), reproduce runs appendix B's costs with no bins (FSL-21 named) and ends with an error where corratio aborts (specs/mcflirt.md §12.1)` — Gap: `–`
- **FSL-65** — larmorx today: `the clean-room original since 2026-10-30 (specs/mcflirt.md §10.6): an error by default (exit 1, the file and FSL-65 named, nothing written), mcflirt's behaviour with reproduce (an unopenable .par: its message after MAT_0000 and the .rms files, exit 0; an unopenable .rms: not written); the replica writes everything it can and ignores the failure in both modes (a follow-up)` — Gap: `the replica`
- **FSL-66** — larmorx today: `the clean-room original since 2026-10-30 (specs/mcflirt.md §12.1): refused by default before any work (exit 1, every cost, -stages 1 or more), with reproduce an error saying how mcflirt ends where it crashes (its own lines first: none for SIGFPE, the two std::bad_array_new_length lines), mcflirt's run elsewhere (-3 to -1 with normcorr, leastsquares); the replica runs normally in both modes (a follow-up)` — Gap: `the replica`
- **AFNI-75** — larmorx today: `the method kept; by default both implementations compute the ball's memory first (AFNI's method: (4/3·π·r³ + 1)·(24 + 4·w) bytes) and stop at once, naming AFNI-75 with the GiB needed, the radius, the voxels and the likely cause (micrometre units), the same words in both (the replica since 2026-10-25, the clean-room original since 2026-10-30, specs/3dUnifize.md §9.1); reproduce attempts it as AFNI does (the original keeps only the ball's lines that reach the grid: the same values, little memory)` — Gap: `–`

The "D13 gaps" list: remove "AFNI-75 | the clean-room original of 3dUnifize: the same guard ..." (line ~567) and the
"FSL-20 and FSL-21 in the clean-room original" part of the "Open:" sentence (line ~539-541); keep "FSL-65 and
FSL-66 in the replica" as open. Recount (`register_counts.py`).

## 2. `docs/api/upstream-defects.md`

Rows 117-120 and 105, replace with:

```
| AFNI-75 | 3dUnifize `-EPI` | `larmorx afni 3dUnifize -EPI` (both implementations) | the ball of neighbours is sized from the voxel size, unbounded: beyond any machine's memory on micrometre voxels | the same computation; by default both implementations stop before building the ball when it exceeds the limit (a limit given to the computation, `LARMORX_MEMORY_LIMIT`, above 4 GiB the machine's memory), naming AFNI-75, the GiB needed and the radius (the same words in both); `reproduce` attempts it |
| FSL-20 | mcflirt | `larmorx fsl mcflirt` (the replica), `larmorx mri hmc` | when the `.mat` directory cannot be created for another reason than its permissions, mcflirt prints "Error!: Unknown Log directory return value" and exits with status 0, writing nothing more | an error (exit 1), nothing written |
| FSL-21 | mcflirt | `larmorx fsl mcflirt` (the replica), `larmorx mri hmc`, `lx.mri.hmc` | with a histogram cost and `-bins` below 8, the first stage's histogram has no bins: arbitrary matrices, and `corratio` aborts | an error before any work (exit 1) |
| FSL-65 | mcflirt | `larmorx mri hmc` (the replica: to do) | an output text file (`.par`, `.rms`) that cannot be opened: mcflirt exits with status 0 after part of its outputs (`.par`), or silently skips the file (`.rms`) | an error (exit 1), nothing written; `reproduce` mcflirt's behaviour |
| FSL-66 | mcflirt | `larmorx mri hmc`, `lx.mri.hmc` (the replica: to do) | a negative `-bins` crashes mcflirt (signal 8 or 6) for every cost | refused before any work (exit 1); `reproduce` an error where mcflirt crashes |
```

## 3. `docs/next-steps.md`

- Line ~265 ("mcflirt's FSL-20, FSL-21, FSL-65 and FSL-66 in the clean-room original: ready for its
  implementer ..."): replace the item with
  `- **mcflirt's FSL-20, FSL-21, FSL-65 and FSL-66 in the clean-room original: done** (2026-10-30, from specs/mcflirt.md §10.6, §12.1; the 33 cases of §14.1 in the mri-hmc suite, failures/<case>, mcflirt live; golden cli.mri.hmc.unwritable and cli.mri.hmc.bins; docs/findings/mri-hmc-original.md, 2026-10-30).`
- Line ~324 (under "Random testing's open items": "AFNI-75's guard is in the replica of 3dUnifize only: the clean-room original's is ready for its implementer ..."): replace with
  `- AFNI-75's guard: in both implementations of 3dUnifize (the clean-room original's since 2026-10-30, specs/3dUnifize.md §9.1; the same words as the replica's; the §10 cases in the afni-unifize suite, guard/epi-<voxels>, and the permanent case difftest/epi-ball-beyond-any-machine-original; golden cli.afni.3dUnifize.guard).`
- Keep "The mcflirt replica's FSL-65 and FSL-66" as it is (the replica's follow-up).

## 4. `validation/difftest/triage.toml` (by tool, implementation, failed checks)

The trial: `python -m larmorx_validation difftest --tools mri-hmc,afni-unifize --budget 0.5 --jobs 6 --seed 20261030 --out <workspace>/oracles/difftest/2026-10-30-cleanroom-additions` (boxed, 16 GiB; larmorx `6f64291`; 4,179 cases; report.md there). Entries to add (the file's own fields; the agent did not open it):

- tool `mri-hmc`, implementation `original`, failed checks `out_mcf.nii.gz.mat: matrix RMS deviation, Nth percentile` (1,105), `out_mcf.nii.gz_rel.rms: values` (972), `out_mcf.nii.gz_abs.rms: values` (914), `out_mcf.nii.gz: voxel values` (743), `_rel_mean.rms`, `_abs_mean.rms`, FD correlation, `.par` (227 combinations, 1,528 of 2,411 cases): status `explained`. The several-minima class of the nights of 2026-10-25 and 2026-10-29 (small phantoms moved by large rotations; the cost has several minima, the two optimisers end in different ones): as well aligned as mcflirt's, the corrected series' temporal standard deviation larmorx / mcflirt median 1.000 (5-95 %: 0.929-1.047; lower 312, within 1 % 973, higher 243), every option alike; `validation/investigations/cleanroom-fixes/hmc_alignment.py`, results `validation/investigations/cleanroom-additions/results/hmc-alignment-trial-2026-10-30.jsonl`; smallest reproducer `mri-hmc:20261030:8`. The trial's draws (64 to 256 bins, outputs in existing directories) meet none of FSL-20, -21, -65, -66.
- tool `mri-hmc`, implementation `original`, failed check `D13 notices in both modes` (verdict notice: FSL-01, FSL-09, FSL-59 named by default only), case `mri-hmc:20261030:1778` (`-cost mutualinfo -bins 128 -nn_final -stages 1 -plots -rmsabs -stats` on a thin slab): status `open`, the class of `mri-hmc:20261029:45` (the default's FSL-03 correction changes the first stage's path, so the notices of the later corrections differ between the modes); not changed by this part (its code paths are untouched).
- tool `afni-unifize`, implementations `original` and `replica`: no difference (1,757 agree of 1,768); 9 cases over the case's memory limit (`-EPI` on tiny voxels, 1.9 GiB to 2.4e11 GiB predicted) where both defaults stop with AFNI-75's error (verdict `guard` for both, the original since `GUARDED` names it); 1 notice case (AFNI-45 named without a visible change, `afni-unifize:20261030:1415`, both implementations alike; not this part's); 1 undefined case.

## 5. `docs/validation/golden.md`

Add to the cases' list (the mri and AFNI sections):

- `cli.mri.hmc.unwritable`: outputs mcflirt cannot write (`specs/mcflirt.md` §10.6): `-mats` in a directory that does not exist (FSL-20) and a directory named like the `.par` file (FSL-65), both modes: the exit statuses (1, 0, 1, 0), the defects named, nothing written by the default, mcflirt's `MAT_0000` and `.rms` files reproduced.
- `cli.mri.hmc.bins`: bins no histogram can have (§12.1): `mutualinfo -bins 4` (FSL-21) and `-bins -16` (FSL-66) refused (exit 1, nothing written), `-bins -16` reproduced (exit 1), and `mutualinfo -bins 7 -stages 1` reproduced (its matrices: appendix B's zero-bin histograms).
- `cli.afni.3dUnifize.guard`: AFNI-75's guard (`specs/3dUnifize.md` §9.1): `-EPI` on a series in micrometres with `LARMORX_MEMORY_LIMIT=4G`: exit 1, AFNI-75 named, nothing written.

The golden suite's helpers `cli_status` (the exit status and the defects named of a call that may be refused) and `files_under` are new in `tests/golden/registry.py`.

## 6. The site's Roadmap (`content/roadmap.md`)

The row "Two specified clean-room additions": status **done** (2026-10-30), with: "FSL-20, FSL-21, FSL-65 and FSL-66 in the clean-room head-motion correction; AFNI-75's guard in the clean-room 3dUnifize (both from the specs of 2026-10-30)". Move it to "Completed" with its date. (No site work by this part; the orchestrator re-exports the pages at the merge.)

## 7. Notes for the merge

- The records `docs/validation/mri-hmc.md` and `afni-unifize.md` were re-made from a clean commit (7c9df9d, standard tier: `mri-hmc` 169 cases, 145 pass, 20 rejected by both, 3 expected, 1 fail, `real/ds003345/default`, the same failure as main's record; the 33 new `failures/` cases: 24 pass, 9 rejected by both, i.e. mcflirt ends by a signal and larmorx exits 1 in both modes. `afni-unifize` 124 cases, 103 pass, 19 rejected by both, 2 expected, none failing; the 7 new cases pass). compare_records: status differs 0 in both suites, details differ 0 (afni-unifize) and 1 (mri-hmc: a case's timing and reason text of `difftest/phantom-several-minima`). The mri-hmc record now names the recording `oracles/fsl-6.0.7.23/mcflirt-haswell/` (the harness's choice, `fsl_versions`; main's record said `.../mcflirt/`): every recorded case has the same status. The generated tables (README, validation index) were regenerated (counts only: 124 cases for 3dUnifize; 1 of 146 compared cases for mri-hmc); the index's mri-hmc row still says "FSL 6.0.7.17 ... until a clean-room agent aligns it" from `validation/tools.toml` (stale since 2026-10-29: the orchestrator's text); compare_records against main's: only the new cases.
- The replica suites are unchanged: the AFNI-75 guard cases are the original's only (`afni_unifize._guard_scenarios`), and the harness's `Scenario` gained `guard_text` (checked only where given).
- Two earlier trial attempts are kept, renamed, and say nothing about larmorx: `oracles/difftest/2026-10-30-cleanroom-additions-aborted-so-replaced` (log `...-2026-10-30-aborted.log`: the worktree's module was rebuilt under it, exceptions) and `oracles/difftest/2026-10-30-cleanroom-additions-exported-tree-wrong-afni` (log `...-2026-10-30-exported-tree.log`: run from a `git archive` copy outside the workspace, where the harness finds no workspace and runs `~/abin/3dUnifize` instead of the AFNI 26.3.05 oracle: 376 header-byte differences for both implementations alike; its mri-hmc part used the right FSL). The trial reported here ran from the worktree (`--out oracles/difftest/2026-10-30-cleanroom-additions`).
- `difftest/runner.py`: `GUARDED["afni-unifize"]` now names the original as well as the replica (its default checks the ball's memory before allocating), so a case predicted beyond the memory limit runs the original's guard check (verdict `guard`) instead of being queued as heavy (`memory`).
