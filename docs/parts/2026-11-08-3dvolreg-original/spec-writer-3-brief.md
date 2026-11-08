You are the **spec writer** of 3dvolreg (github.com/karellopez/larmorx), step 3: the clean-room implementer's
questions after its records. Read CLAUDE.md first, then `docs/porting-guide.md` §1 (the spec writer: behaviour,
maths, conventions, command line and validation plan, never code or routines) and `specs/3dvolreg.md` as it stands.

## The situation
- 3dvolreg's replica is merged (main). Its clean-room original (`crates/larmorx-afni`'s `volreg`, a different
  agent that never read AFNI's source) has finished its tests and records in the branch whose head is **17dc8e1**
  (worktree `larmorx/.claude/worktrees/agent-a543551b26b82e03a`: never edit it; read it only as said below).
- Step 2 of the spec writer (commit 16d7e40, already in 17dc8e1 as 103bbba) answered eight questions and wrote
  §14.3's derived limits and the model's `thresholds.py`, `afni_run.py`, `afni89.py` and the `TIES` option.
- The orchestrator decided §16 (2026-11-07): the spec does **not** describe AFNI's own shear-decomposition
  arithmetic; the clean-room original is held to thresholds where the decomposition's exact ties decide AFNI's bits.
  That stays.

## Your worktree
The harness gave you a fresh worktree on its own branch. First: `git reset --hard 17dc8e1` (your branch only), so
your commits apply cleanly to the clean-room branch. Build the extension once: copy the clean-room worktree's
`target/` into yours to save the build (`cp -a`), then `maturin build --release -o <scratch>/wheel` and copy
`larmorx/_core.abi3.so` into your `python/larmorx/` (never `maturin develop`). `source scripts/orchestration/env.sh`.

## The questions (from the clean-room implementer's NOTES; the numbers are its own)
1. **§14.3's registered-volume (mean |diff| <= 2e-3 of the max) and median (1e-4) thresholds against real
   high-motion data.** fMRIPrep's median (niworkflows' RobustAverage) on ds000256's `nofeedback` run (a child's high
   motion; record case `fmriprep-ds000256-nofeedback`, the record's only failure): 2.31e-3 per volume and 2e-2 for
   the median with the original's default tie side; 3.05e-4 for the median with each volume's best side; ds000228
   2.4e-4; ds005454 1.07e-4; ds004958 1.37e-4 (one tie side meets them). The median's bound is stricter than what the
   volumes' own bound lets through. Measure AFNI's own spread under the ties on these runs (the tie choices as AFNI
   makes them, per resampling, against other choices: your restricted tools may compute AFNI's choices, in scratch
   only), and write limits the clean-room original can be held to without AFNI's choices, consistent between the
   volumes and the median (the median's limit following from the volumes'), with the measurements in the spec.
2. **`-final NN`** under the ties: 34,036 voxels moved on one case; a tie side gives AFNI's. A criterion that
   nearest-neighbour output can meet (e.g. the parameters within their limits and every differing voxel explained by
   a neighbour's value), measured.
3. **`-SAVEDISP`'s products** differ from §10.3's float32 order at a few voxels (second and third order): check §10.3's
   arithmetic order against AFNI; correct the spec where it is wrong, or state the limit.
4. **Ill-conditioned fits** (small, sharp phantoms in random testing): the ties move converged fits beyond the
   limits. A conditioning-aware limit stated from quantities a black-box implementer can compute, or another rule.
5. **The suite's tie re-examination**: on a failure the suite re-runs the original with each of its four tie sides
   (`LARMORX_VOLREG_TIES`, a validation-only switch), globally or volume by volume. Is that an acceptable reading of
   §14.3's "either side of every tie", given that AFNI mixes the sides per resampling? If not, what test is.
6. **The random-testing trial's triage** (`oracles/difftest/2026-11-08-volreg-original-trial`, 21,710 cases;
   a seeded sample of 1,200 re-judged with the final build in
   `<scratchpad>/volreg-cr-3/triage-trial.json`, by `validation/investigations/afni-volreg-original/triage.py`):
   ties 676, runaway fits 245, fits that never converge 80, parameters within the limits but another check failed 23,
   fixed 43, **converged but beyond the tie sides' spread 133** (open). State what the spec requires for runaway and
   never-converging fits (both sides runaway? the output's form only?); examine the 23 and a sample of at least 30 of
   the 133 with AFNI's own tie choices put into the model, and say for each kind whether it is the ties (then the limit
   of question 1 or 4 covers it) or a behaviour the spec misses (then describe that behaviour in the spec).

## What you may read and run
Anything a spec writer may (AFNI 26.3.05's source, the replica, `docs/findings/afni-volreg.md`, the replica's
investigations), plus the clean-room part's record `docs/validation/afni-volreg-original.{md,json}`,
`docs/findings/afni-volreg-original.md`, its suite's results and the trial's findings, and its NOTES
(`<scratchpad>/volreg-cr-1/NOTES.md`). Run the clean-room original, the replica and AFNI (through the oracle cache,
`use` mode) as you need.

## What you write (and nothing else)
- `specs/3dvolreg.md`: each answer marked "(spec writer, 2026-11-08, step 3)", behaviour and measured limits only:
  no AFNI code, routines, variable names or the shear formula.
- `validation/investigations/3dvolreg-spec/`: the model and its scripts (`thresholds.py` checking the new limits on
  its cases and on the real runs above, 0 failures), written from the spec and black-box runs only, so the
  clean-room implementer may run them; its README. Anything that needs AFNI's choices or source stays in your
  scratch, never committed.
- One or a few commits (spec and model folder only; the configured identity; no AI attribution; never
  `--no-verify`). Never edit `crates/` (above all `crates/larmorx-afni`), the clean-room's suite, evidence, difftest
  or docs files, nor any shared document.
- `<scratchpad>/volreg-spec-3/NOTES.md`: one line per answer, the commits, what the model folder gained.

## The machine
Every heavy job inside `scripts/orchestration/boxed.sh` (within **32 GiB and 14 threads** in all: another agent
works beside you); long runs in the background with logs in `oracles/logs/`, one background wait on the final
line; no helper that can find itself again. Scratch: `<scratchpad>/volreg-spec-3/` (`<scratchpad>` =
<scratch>);
never write under a `.claude/` folder (your worktree itself excepted). No push, no merge.

## The report (your final message, 5 to 10 lines)
Each answer in one line with its numbers, the commits, the model's checks, anything the orchestrator must decide.
