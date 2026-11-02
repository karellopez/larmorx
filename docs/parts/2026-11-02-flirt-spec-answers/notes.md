# FLIRT spec answers: relay notes

Worktree: <workspace>/larmorx/.claude/worktrees/agent-a1899e54855e47816
Branch: worktree-agent-a1899e54855e47816 (from main fb1de31). `cd` there first; every command runs
there; never touch the main checkout or the clean-room part 2's worktree.

## State (relay 3, 2026-11-02 ~17:00)
- **Sub-task 3 DONE but the smoke tier**: commits 193b27b (FSL-90..93 in the replica, defects, spec,
  five suite cases, restricted findings, investigations), 6af5ff9 (golden cases), 7e43e5c (golden.tsv
  re-recorded), 2044c75 (edge/background-overflow with a 2-voxel input: FSL-92 refused its 1-voxel
  input by default), 2c21ac6 (real/applyxfm-nan-border: FSL-22 in FLIRT's padding), 9b8e98d (the
  record from 2c21ac6: 302 cases, 209/209 compared bit-identical, 87 both, 6 expected; compare_records
  against the committed one: statuses equal, details only FSL-90's and FSL-92's fix passes).
  The first record run (from 7e43e5c) failed on edge/background-overflow: kept in
  `<scratch>/record-7e43e5c/`.
- Gates: Rust root, gpl OK; nc OK (golden recorded); ruff, spdx, pytest 665 passed.
- The parity smoke tier from 9b8e98d: PASSED (2653 passed; marker parity-smoke-9b8e98d...). crates-nc
  cargo test again: all pass (golden included). FSL's zero-voxsize run: timed out at 3600 s (never stored).
- **The part is DONE** (relay 3 stopped after its final report). HANDOFF.toml (9 ops, --check OK) and
  HANDOFF.md in `<scratch>`. Nothing running.

## State (relay 2 done, 2026-11-02)
- **Sub-task 1 (questions 1 to 5): DONE** (relay 1), commit d3b31ed.
- **Sub-task 2 (questions 6 to 20): DONE** (relay 2), commits c8ce906, 63321d9, b727a0c. Tree clean.
  All 15 answered and in the §14 table (rows 6 to 20), every precision answer proven by a model.
- **Next: sub-task 3** (the two refusals, FSL-90..95, the replica's switch if a refusal is decided,
  the handoff files, the part's final report). Relay 3 also decides the two new defect leads below.
- Nothing running. No record, no smoke tier, no golden run made (the replica is unchanged so far).

## What relay 2 did (for relay 3's final report)
- `specs/flirt.md` (every line marked "(spec writer, 2026-11-02)"): §3 "The composition exactly" and
  "The decomposition exactly" (q16, q6b); §4.1 "Neurological files" (q15, q9: the left-right swap's
  field of view (n_x − 1)·d_x in single precision); §4.2 the base scale before the tripling (q18);
  §7.2 the reset-only setscale keeps the centre (q8); §7.4 "The first pair" (q17); §8.1 steps 2, 3,
  4, 7, 8, 9 (q6: the search command's dof; the grids' tolerances' x₀; single-precision grids;
  the percentile's element; minima z, y, x; the minima composed from min(-dof,7) params then
  decomposed; -anglerep quaternion's minima read as quaternions; P's cost; the list rule); §8.2
  convergence's n_t and weights (q7) and "The tolerance unit of a line search" (q19: u = |1/τ|, τ
  the single sum of |û_i/t_i|; along an axis fl(1/fl(1/t)), not fl(t)); §8.5 (q15); §8.7 the
  warnings (q12); §9.2 the paddings (q20) and "The windowed sinc exactly" (q13); §9.3 the output
  order (q10), the edge rule before the blur and the spline exactly (q14), -applyisoxfm's header
  (q9); §12 "The 4 × 4 inverse exactly" (q11).
- **Per question** (right / incomplete / wrong, confidence, model): the §14 table rows 6 to 20 say
  it; in short: 6 (a,c,d,e) right, (b) incomplete (minima: matrix composed then decomposed);
  7, 8, 10, 12, 17, 18, 19, 20 right (high, source; 19 in the L1 sense); 9 right/incomplete (qform
  rounded to single before the conversion; model: 12 header fields); 11 right, no FMA effect
  (model: FSL's -0 text); 13 incomplete (model 40000/40000 voxels); 14 incomplete (model
  172800/172800 values); 15 found (single-precision FOV; model -13.74984741); 16 incomplete
  (composition (B·C)·Bᵀ + single decomposition; model: -fixscaleskew's 10 digits and a
  double-angle composition where the textbook rotation parts at the 8th digit).
- Models: `validation/investigations/flirt-spec-answers/matrices_model.py` (q9, 11, 15, 16; 5 of 5;
  variants double-fov, plain-rotation, divide), `resample_model.py` (q13; `--spline` q14; variants
  double-sums, kernel-order, divide-table); README updated with the numbers; log of the last run:
  `<scratch>/models_relay2.log`.
- FSL runs added to `fsl_runs.py` (all through the oracle cache): quat, usesqform, isoxfm-setbg,
  sinc (served from the cache: the suite's), isoxfm-oblique (new, derived input
  `<workspace>/oracles/inputs/flirt-spec-answers/t1_oblique.nii`), fixscaleskew (cache),
  inverse-noz (new; computed twice: the first noz.mat was replaced by an exact 3-4-5 rotation, the
  first run's cache entry is unused), rotation (new). Outputs in `<scratch>/runs/<name>/`.
- `docs/findings/fsl-flirt.md`: section "The spec answers of 2026-11-02 (the search, the outputs and
  their precision, from the source)".

## Leads for relay 3 (found by relay 2; decide by rules A-D, IDs FSL-90..95)
- **`-anglerep quaternion`'s search** (rule A candidate, realistic: a documented option): the
  search's minima (Euler angles) are composed with the quaternion representation, so a minimum
  whose angles' squares sum above 1 loses its rotation (identity, the message printed) and the
  others are misread; FSL's result with -anglerep quaternion is then a worse search. §8.1 step 9
  states FSL's behaviour; if registered, the replica would compose the minima as Euler angles by
  default and keep FSL's with `reproduce`, + the spec's §11 entry + `larmorx_core::defects`
  (behaviour only) + a replica parity case (real/anglerep-quaternion's stderr already differs: the
  original printed 5 lines, FSL 4). Measure first (rule D?) whether the corrected search changes
  the result on real/anglerep-quaternion (the matrix vs -anglerep euler's).
- **A zero voxel size in a header** (not checked): the base scale reads the headers without the
  reading's zero-size repair, so a zero pixdim gives `m = 0 < 0.75`, `b = 0`; unrealistic (a broken
  header), probably rule B if FSL misbehaves; only if cheap.
- The line search's unit `fl(1/fl(1/t))` (§8.2) is the same line search as mcflirt's
  (`specs/mcflirt.md` §7.2 says "u, the tolerance as float32"); mcflirt's fixed tolerances are
  unaffected at the scales it uses (relay 2's check: the defaults at 8, 4, 2, 1 agree); a sentence in
  mcflirt's §7.2 is optional.
- A zero BBR normal (relay 1's lead) stays as before.

## What relay 1 wrote (so relay 2 continues the same way)
- `specs/flirt.md`: §6.2 "`corratio` exactly" and "`woods` exactly" (new paragraphs after the MI
  paragraph); §6.5: the BBR normals' paragraph rewritten + a bullet list "The smoothing, the
  gradient and the normals exactly", the interpolation's per-neighbour zero, `local_abs` confirmed;
  §8.1 step 3: the centres of mass' precision (v − v_min in single, double block sums of
  max(1000, trunc(√N)) + 1 voxels); §12: the three-level sums hold for normcorr/labeldiff only, not
  corratio/woods/MI (one running sum per bin); no FMA in FLIRT's own arithmetic.
- **§14 has the table "The clean-room questions of 2026-11-02 (spec writer)"** with rows 1 to 5.
  Relays 2 and 3 ADD rows 6 to 20 and the refusals to the same table (columns: #, question,
  answered in, the observation right/incomplete/wrong, confidence and check).
- `specs/mcflirt.md` appendix A: corratio (n and var over the bins with n_b > 2 only; cost 1 when no
  such bin / no variance) and woods corrected (`n_b²σ_b/S_b`, `n_b²σ_b` when S_b ≤ 0) + **the hmc
  sentence** ("For `lx.mri.hmc`'s later part (tier 3): mcflirt's -cost corratio and woods are
  FLIRT's two costs unchanged ...").
- `docs/findings/fsl-flirt.md` (restricted): section "The spec answers of 2026-11-02 (the costs and
  BBR, from the source)", with the FSL routine names behind each answer. Relay 2 appends its own
  section the same way.
- `validation/investigations/flirt-spec-answers/`: README (table of files), `fsl_runs.py` (FSL runs
  through the oracle cache: add new named runs to its RUNS dict; `@name` = suite input,
  `@@name` = a derived input made by `derived()` in `<workspace>/oracles/inputs/flirt-spec-answers/`),
  `bbr_model.py`, `corratio_model.py` (has a working model of FLIRT's sampling, trilinear,
  edge weights, composition about the centre, the subset line search of mcflirt §7.2/7.3, and the
  centre of mass: reuse it for the search questions 6 to 8, 19), `corratio_costs.sch`.
- FSL runs so far (all via the cache, all now cached): bbrcost, bbrcost-noclamp, bbrcost-local_abs,
  bbrcost-global_abs, dof6-trace (-verbose 7, 5822 evaluation lines), dof6-debug (refvol_8 etc.,
  testvol), corratio-mc, woods-neg. Outputs in `<scratch>/runs/<name>/` (stdout.txt, stderr.txt,
  files). Re-running `fsl_runs.py <scratch>/runs <name>` serves them from the cache.

## How to run (the harness refuses `source`/`.` and compound commands with cd; use plain env)
    env OPENBLAS_CORETYPE=Haswell PYTHONPATH=python:validation/src LARMORX_WORKSPACE=<workspace> \
      scripts/orchestration/boxed.sh 8 <workspace>/.venv/bin/python -P \
      validation/investigations/flirt-spec-answers/<script>.py <scratch>/runs [...]
  `<scratch>` = <scratch>
  Lint: `.venv/bin/ruff check validation/investigations/flirt-spec-answers/` and `ruff format`
  (the investigations are linted; no Greek letters in Python strings: RUF001/002); SPDX:
  `.venv/bin/python scripts/check_spdx.py`. Commit with plain `git add <paths> && git commit -m ...`.

## What the next relays need (summaries of the documents relay 1 read)
- **Spec rules** (porting guide §1; brief): behaviour, maths, conventions, command line, validation
  plan only; **never code, routine names, file/variable names or line references** in `specs/`,
  `larmorx_core::defects`, `docs/api/`, next-steps, triage, the register's summary rows, the site.
  Mark every new or changed line "(spec writer, 2026-11-0x)"; "(observed)" where FSL as a black box
  confirmed it. Source names go only into the restricted `docs/findings/fsl-flirt.md`.
- **Never edit** `crates/larmorx-fsl`, `crates/larmorx-mri`, `crates/larmorx-afni`,
  `docs/findings/fsl-flirt-original.md`.
- **The questions with their evidence**: `docs/parts/2026-11-02-cleanroom-flirt-1/handoff.md`
  lines 202-258 ("Spec questions"); the implementer's observations in
  `docs/findings/fsl-flirt-original.md` (154 lines; "The search and the optimisation", "The pyramid
  and the outputs" are relay 2's material); their probes in `validation/investigations/cleanroom-flirt/`
  (`probe.py` runs a suite case with FSL + the original; `sinc_kernel.py` for question 13).
- **The 22 failing cases** (`docs/validation/fsl-flirt.md`): real/anglerep-quaternion and
  -quaternion-dof6 (an extra stderr line "Parameters do not form a valid axis - greater than unity"
  printed by the original only), real/dof6 and verbose/v (0.179 mm p95, band 0.155: the search's
  last bits — question 3 answered; the 4 mm stage's drift may also involve questions 6, 7, 19),
  real/cost-normmi (1.42 vs band 1.24), real/cost-woods (109 mm vs band 31: woods now exact, §6.2),
  real/weights-woods (60 mm, band 44; plus a missing "WARNING: Woods is not implemented with cost
  function weighting" line at line 3938 of FSL's stderr: the original stops printing it?),
  real/applyisoxfm-4d-blur-setbackground (question 14: 53831 of 172800 values, 0.41 of the range),
  real/out-only (NCC 0.998965 < 0.999), real/basescale-big (0.214 vs band 0.002), real/basescale-forced
  (0.132 vs 0.048), bbr/default, bbr/fmriprep-form, bbr/type-global_abs (0.107 vs 0.015: questions 4
  answered), bbr/type-local_abs (1.18 vs 0.90: question 5), bbr/fieldmap-mask (0.118 vs 0.006: the
  normals too, presumably), and others in the record (22 in all; its "Failures" section lists them).
- **The model part** (`docs/parts/2026-10-30-bet-spec-answers/`): its report gives each question,
  confidence, and what the model reproduced; its handoff gives exact texts for next-steps item 7 and
  the register. This part's final report (relay 3) follows the brief's "final report" list.
- **Handoff format** (`scripts/orchestration/apply_handoff.py` docstring): `[[op]]` with
  `do = "edit"` (file, old (exactly once), new), `"insert"` (file, after = start of a line, text),
  `"append"` (file, text), `"block"` (kind tool/record, id, text); check with
  `python -P scripts/orchestration/apply_handoff.py <scratch>/HANDOFF.toml --check` in the worktree.

## Leads for relays 2 and 3 (found by relay 1, not done)
- **A zero BBR normal** (an isolated boundary voxel with a perfectly symmetric smoothed
  neighbourhood) makes the points NaN and the cost NaN with FSL's warnings (stated in §6.5).
  Unrealistic on real segmentations; relay 3 may consider it with the refusals (rules A-D) or
  leave it documented.
- Question 6 (e) and 8 (the centres): §8.1 step 3 now says c_ref is the pair's reference level's
  centre and c_in the pair's input's (the model of relay 1 needed exactly that). Relay 2 confirms
  the rest of 6 and 8 in §8.1/§7.2.
- The trace model (`corratio_model.py`) stops after the first sweep of the first coarse point; to
  check question 7 (the convergence test's n_t = 4) extend it to the following sweeps (bound guess
  1 after the first sweep, §8.2) and compare more trace lines (FSL's 5822 are in
  `<scratch>/runs/dof6-trace/stdout.txt`).
- The rotations' composition: FSL takes θ = |angle| in single and its sine and cosine in single
  (restricted findings); the trace model agrees with either precision; question 16 may need it.
