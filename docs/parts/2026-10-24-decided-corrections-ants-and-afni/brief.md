# Brief: Decided corrections: ANTs and AFNI

As given to agent `af2255d42fc4f41d1` on 2026-10-23, verbatim (paths are this machine's).

You are implementing decided corrections in larmorx's replicas (github.com/karellopez/larmorx; your worktree is a checkout of main). Read the repo's `CLAUDE.md` first (rules 1-10, "Agent working rules", "Working on this machine"), then `docs/decision-log.md` (2026-10-23: "Four rules for the originals' questionable behaviours; the 24 open questions"), the register entries you implement in `docs/findings/upstream-defects.md`, `docs/porting-guide.md` (§8, D13), `docs/api/upstream-defects.md` (how the switch works), `docs/figures.md`, `scripts/orchestration/README.md` (the helper scripts: use them), and the findings and API pages of the tools you touch.

**Your role: replica author** (you may read ANTs, ITK and AFNI source, pinned tags in `/home/karelo/VScode_projects/super_fmriprep/tags/`). Never edit a clean-room crate (`crates/larmorx-afni`, `crates/larmorx-mri`) or `specs/`. Validate against the GCC 14.3 ANTs oracle and AFNI 25.2.09 in `oracles/` only.

## Task: the corrections decided on 2026-10-23
Every one keeps `reproduce` giving the original exactly; parity in `reproduce` mode (bit-identical as before) plus a fix pass that shows what each correction changes; a notice where the default differs from the original; golden checksums re-recorded where defaults change.
1. **Rule A (correct it):**
   - ANTS-17: ImageMath `MO` with radius 0 (a dilation of zeros) and `ME` turning other labels into 1;
   - ANTS-22: ImageMath `PadImage` with a fractional pad (uneven growth and shift);
   - ANTS-23: ImageMath `ExtractContours` truncating label values;
   - ANTS-32: ResampleImageBySpacing's smoothing sigma (voxels used as millimetres) and the value given to points outside the image;
   - ANTS-44: DenoiseImage's bias field outside the mask left unsmoothed;
   - ANTS-116: antsAI `-b` comparing blob patches possibly reflected or turned (check the rotation's determinant and try the eigenvectors' sign combinations, keep the best correlation): show its effect on the T1w and its shifted copy (4 of 11 identical pairs correlate at 1 today) and on the rotated cases.
   For each, decide the methodically correct result and write the reasoning into the findings.
2. **Rule D (measure first, then rule A or C):**
   - ANTS-40: N4 keeps zero and negative voxels in its log image. Measure on fMRIPrep's real N4 calls without a mask (the oracle runs, `oracles/fmriprep-25.2.6/*/work`): how many voxels, how much the corrected field and image change (against N4's own thread variability), what the correction should be (excluding them from the fit, as the method intends). Then apply rule A (correct by default) if it is the methodically correct result, or rule C (keep and document) if correcting brings no gain or the effect is within ANTs' own variability; write the measurement and the reasoning.
   - ITK-06: ITK's NIfTI reader turns NaN and ±infinity into 0. Measure which larmorx tools and which real inputs it affects and what keeping NaN would do downstream; then rule A or C, with the reasoning. If the rules cannot settle it, say so in the report (the user decides only such cases).
3. **Rule B (stop with an error) in the AFNI GPL replica** (`crates-gpl/larmorx-gpl-afni`): AFNI-41 (3dAutomask `-nbhrs` outside 6..26) and AFNI-48 (3dUnifize `-T2` three or more times): a fatal error naming the entry by default, AFNI's behaviour with `reproduce`. (The specs already say so; the clean-room originals are written by another agent from them.)

## Validation
Extend the parity suites (`ants-image-math`, `ants-resample-image-by-spacing`, `ants-denoise`, `ants-n4`, `ants-ai`, `afni-automask`, `afni-unifize`) with cases for each entry, synthetic (phantoms where the correct result is known) and real (the oracle runs); records regenerated from clean commits, the standard tier only (the full tier once at the end if a correction touches its cases). Unit tests for every corrected path; golden cases re-recorded (`scripts/record_golden.py`, clean commit).

## Documentation, both repositories
- Your own files: the API pages (their D13 tables), the findings files of the tools, the records. Shared documents (the register's rows and entries, `docs/api/upstream-defects.md`, README, the validation index, golden.md, next-steps, the decision log, CLAUDE.md, the site's status, navigation, Validation page, summary) go through `<scratch>/HANDOFF.md` with the exact text; the orchestrator integrates it.
- The site (approved evidence form): each corrected tool's page shows its correction as a deliberate correction (verdict and reason, its measured effect, the original's result one switch away): ImageMath and the filters, DenoiseImage, N4 (the ANTS-40 measurement and decision), antsAI (ANTS-116 before and after), 3dAutomask and 3dUnifize (the two refusals in their case explorers). Number check, re-export check, your own review of screenshots (desktop 1440 and phone 390, light and dark) in `<scratch>/shots/`.

## Shared documents and IDs
New defects found on the way: only ANTS-130 to ANTS-149, ITK-80 to ITK-89, AFNI-60 to AFNI-69, each decided by rules A-D (decision log) and named in the report.

## Git and where to work
- larmorx: your worktree branch. The site: `git clone /home/karelo/VScode_projects/super_fmriprep/larmorx_documentation <scratch>/site`, set its `user.name karellopez` and `user.email 49761113+karellopez@users.noreply.github.com` before the first commit, branch `small-fixes-evidence`; squash at the end. Do not push or merge.
- Scratch: `/tmp/claude-1000/-home-karelo-VScode-projects-super-fmriprep/d96e838f-0213-4978-9a72-dac8eae4b541/scratchpad/small-fixes/` (NOTES.md, HANDOFF.md, clones, builds, outputs); never write under any `.claude/` folder. Commit every tool behind a measurement or finding under `validation/investigations/<topic>/` with a README.

## Working rules
- **You run in parallel with another agent** (the clean-room originals of 3dAutomask and 3dUnifize, which must never see AFNI's source or the replica: do not write anything about AFNI's internals into files it may read, i.e. keep such text in the restricted findings and the register's † sections, through the handoff). At most 6 threads (`cargo -j 6`, parity `--jobs 3`, `n_threads` ≤ 6).
- Environment: `source scripts/orchestration/env.sh`; never `maturin develop` in your worktree: `maturin build --release -o <scratch>/wheel`, copy `larmorx/_core.abi3.so` into `python/larmorx/`, run Python with `scripts/orchestration/lxv.sh` or `PYTHONPATH=<worktree>/python:<worktree>/validation/src`. Copy `target/` and `crates-gpl/target/` from the build-cache seed `/home/karelo/VScode_projects/super_fmriprep/larmorx/.claude/worktrees/agent-aa903a90387159f4f/`.
- Gates: `scripts/orchestration/gates-rust.sh <worktree> both`, `gates-python.sh <worktree> --no-build`, `pytest tests/parity`; records and golden from clean commits only.
- The user watches the weekly usage limit: commit after every meaningful step; keep `<scratch>/NOTES.md` current; wait for long runs with one background wait and end your turn until it notifies you; never stop silently.
- Never add `Co-Authored-By`, "Generated with" or any AI attribution. Never delete work products; never modify the upstream clones; never ask for sudo. No formal benchmarks.

## Stop after this part; report (concise)
Per entry: the correction (or rule C with the reason), its measured effect, the parity numbers (reproduce bit-identical; the fix pass), real-data results; ANTS-40 and ITK-06's measurements and decisions; new defects and their rule; golden; the site pages; gates; commits; `<scratch>/HANDOFF.md`; anything that needed approval or that the rules could not settle.
