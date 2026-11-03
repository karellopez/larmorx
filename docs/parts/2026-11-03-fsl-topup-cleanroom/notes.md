# NOTES: topup clean-room original (relay notes)

Worktree: <workspace>/larmorx/.claude/worktrees/agent-a356d70e3aee0e469
(branch worktree-agent-a356d70e3aee0e469, from main 3a15db3; build cache copied from the main
checkout's target/)

## State after relay 6 (commits 9d5c031..1778d7f; worktree clean) — SUB-TASK 6 DONE: THE PART IS COMPLETE

Commits: 9d5c031 (the evidence of the original in evidence/fsl_topup.py: its figures-cache entries,
the per-case classification `cases-original`, its agreement, the speed chart, its trial and the
triage table; trial_triage.json; the helpers' README section "The site (relay 6)"), b4a9eac
(correlations with six digits), 01d68a4 (tables.py: `"fsl-topup": _bet`; the triage wording),
2425ade (default views as supersets), 1778d7f (record examples' default comparisons as before).
- Figures cache (`<workspace>/oracles/figures-cache/fsl-topup/`): `original-fmriprep-<run>` (2),
  `original-<case>` (4: opt-membrane-lm, at-lsr-y, at-jac-y, at-vb3d-y), `cases-original`; computed
  from 9d5c031 (clean), boxed 16 GiB (log `oracles/logs/topup-cleanroom-site-entries.log`).
- The original's case verdicts: 9 bit-identical, 2 within FSL's own thread band (ds001600,
  thr/nthr-4), 81 open (78 without a band; ds005040 beyond the band only in the field's
  correlation 0.999958 vs 0.999966; thr/fsl-num-threads and thr/openblas beyond FSL's near-zero
  band), 22 expected divergences, 66 rejected by both.
- Site clone `<scratch>/site`: branch `fsl-topup-cleanroom-page` (28c1ef6 on 171a8ae: the page,
  fsl-topup.json, blobs.sha256), blobs branch `fsl-topup-cleanroom-blobs` (79a73c8 = local
  evidence-blobs, 878 blobs; of the 851 before, one poster redrawn: ds005040's fsl4-fsl on the scale shared with
  the original's difference). Exported from 1778d7f. Build 0 problems; number check 748/748
  (live page before: 270/270); viewers 132/132; inventory after >= before on all 18 pages
  (`<scratch>/inv/before|after`); screenshots `<scratch>/shots/before|after` (1440/390,
  light/dark). The local server (port 8781) stopped at the end.
- HANDOFF.toml (18 ops, `--check` passes in the worktree with `--site <scratch>/site`) and
  HANDOFF.md (reasons, spec questions); generator `<scratch>/py/make_handoff.py`.
- Gates run in this relay: ruff check/format of the touched files, `tables --check`,
  `check_spdx.py`, `pytest validation/tests` (158 passed). The Rust is unchanged since relay 5b.

## State after relay 5b (commits 9721efc..028a5ce; worktree clean) — SUB-TASK 5 DONE

Commits: 9721efc (FSL-146, oblique mask, gradient FD test, first_step.py), e780a96 (FSL-146 on
mixed axes only; SCG weight; --debug order; thread_band.py), 0c0fd8c (7 permanent cases,
permanent.py line, expected divergence honoured, triage_counts.py), 72c5b93 (findings 32-36,
README), f9bedb0 (investigation scripts ruff-formatted), a49e214 (the record), d898df9 (golden),
028a5ce (API page, PROVENANCE).
- The probe: CG tolerance not the cause; FSL-146 found (finding 35; rule A; reproduce agrees
  with FSL; default the cost's gradient, unit test vs central differences). On the way: the
  oblique mask (32), --debug's images in FSL's order (33, a larmorx mistake), SCG's second
  gradient with the current point's λ' (34, a larmorx convention mistake: FSL's first SCG steps
  now reproduced). All conventions follow FSL in both modes.
- Triage (finding 36, `triage_counts.py`): 6,999 findings in 14 categories: 4,462 finding 14
  (movements); 895 FSL-146 (+14); 484 FSL-133; 429 quadratic count (fixed); 276 transposed
  acquisition (harness, fixed); 216 SCG weight (fixed in part) / ill-determined paths; 62
  config outputs (harness); 51 FSL-143; 47 OPEN quadratic regulariser Hessian at even k (10753);
  47 --trace -v (harness); 17 --debug attempt files; 8 OPEN lsr on lines dividing 2^32 (8089);
  4 integer outputs; 1 vb2D transients. Re-check samples: recheck3/4/5 (513 replays each).
- Permanent cases (validation/difftest/cases/fsl-topup-original/, 7, smoke tier): quadratic-count
  (drawn 11095), fsl145-short-axis (601 shrunk, expect divergence: finding 14), fsl146-mixed-axes
  (drawn 10280), scg-regulariser-weight (12747), acquisition-transposed (drawn 27),
  configuration-outputs (12), trace-verbose (1142): 6 pass + 1 expected divergence.
- Gates (from f9bedb0's code): gates-rust.sh all PASS; gates-python --no-build: all pass but the
  6 then-unrecorded golden cases (now recorded: CI-like golden 216 passed); ruff format fixed 39
  investigation scripts; tables --check pass; check_spdx pass.
- **Record** docs/validation/fsl-topup.md/.json from f9bedb0 (clean): 180 cases: 92 pass, 66
  both-error, 22 expected divergences, 0 failures (FSL: 161 served, 21 run: 19 aborting never
  stored, 2 --profile). fMRIPrep's: ds001600 within §12; ds005040 field p99 0.607 within FSL's
  --nthr=4 band 0.64.
- **Smoke tier** once from a49e214: PARITY-SMOKE exit 0 (2714 passed: 14 workers 271 s + serial
  764 s); marker parity-smoke-a49e2148...json. **record_golden** reused it: 6 topup/applytopup
  cases added, nothing else changed.
- Wheel: python/larmorx/_core.abi3.so from 72c5b93's Rust (= HEAD's; <scratch>/wheel5b);
  target/release/larmorx the same Rust.
- The benchmark (docs/benchmarks/fsl-topup.md) is relay 4's build: not re-run (FSL-146 and the
  SCG weight change no fMRIPrep path's speed measurably; informal).

## Sub-task 6 as planned by relay 5b (done by relay 6)
- The site clone, topup's page clean-room panels (FAST's clean-room panels the model), screenshots,
  inventories; HANDOFF.toml/HANDOFF.md from every "For the handoff" section below (relays 3b, 4,
  5, 5b) plus: tools.toml line(s) for fsl-topup (original), triage.toml append op (finding 36's
  categories), difftest.md row, golden.md lines (6 topup cases), next-steps (item 7, follow-ups
  incl. the replica's FSL-144/145/146 switches, the benchmarks README lines, replica applytopup 8x
  slower), the register rows FSL-144/145/146 and docs/api/upstream-defects.md lines; the spec
  questions; the final report of the part (BRIEF "The final report").

## State after relay 5 (commits f1288d8..00506c1; worktree clean) — SUB-TASK 5 HALF DONE (5a), NEXT: 5b

Relay 5 split sub-task 5 (the trial found more than a short triage): **5a done** (random testing,
its fixes, golden cases written, memory and thread tests); **5b next** (the trial's remaining
triage decision, permanent cases, the API page, the gates, then the record, the smoke tier and
record_golden from the final clean commit).

Done in relay 5 (commits):
- f1288d8: rule 5 in the topup module: `larmorx_core::math::{sin,cos,exp}` (movement.rs rotations,
  images.rs smoothing kernel) and a written-out integer power `ipow` (spline.rs; the bits of
  `__powidf2`, so Linux's outputs unchanged in the cases looked at); applytopup's field evaluation
  (`coefficient_field`, `image_field`) now inside the call's pool (`with_threads`; the thread test
  found rayon's global pool: 32 threads at OMP_NUM_THREADS=1); difftest descriptor
  `difftest/tools/fsl_topup_original.py` (`fsl-topup-original`: **the replica's draws**, drawn from
  the `fsl-topup:<seed>:<n>` stream, so seed `topup-trial` = the replica trial's cases, FSL served
  from the cache), registered in `tools/__init__.py`; the suite's `difftest_case` (parity/
  fsl_topup.py, as the replica suite's) and `LX_THREADS` (the worker's OMP_NUM_THREADS); the
  thread probe (`parity/thread_pools.py`): the original's topup (two levels, every output), topup
  membrane/float/linear/quadratic/scale/no-regrid, applytopup lsr/jac/vb2D/vb3D/vb4D (inputs
  `blipup`, `blipdn`; the lsr/vb source is the original's own quick topup made when the inputs
  are made) -> `pytest tests/parity/test_thread_pools.py -k "clean-room and (topup or
  applytopup)"`: 7 passed.
- 1d0b357: golden cases (tests/golden/registry.py, NOT RECORDED yet): inputs `blip_pair`,
  `blip_up`, `blip_down`, `blip_series` (16x16x8, 3 mm, a head moved +-1 voxel along y; seeded
  ints, no transcendental functions); cases `cli.fsl.topup` (covers cli:fsl topup; a config file,
  two levels LM with movements then SCG, every output, usage), `cli.fsl.topup.options` (membrane,
  quadratic, float, linear, no regrid, --acqp, --featout, -v; reproduce and default: FSL-117 and
  now FSL-145 too), `cli.fsl.applytopup` (covers cli:fsl applytopup; lsr, jac short names with
  trilinear/short/-v, FSL-132 reproduce and default on a 4D series, usage), `cli.fsl.applytopup.vb`
  (vb2D/3D/4D), `py.fsl.topup`, `py.fsl.applytopup` (covers the two functions);
  `REPLICA_ONLY_FUNCTIONS = {}`. Inputs given relative to the case's directory (`_in_dir` chdirs
  for the in-process call). Checked twice in dirs with a dot and a blank (`<scratch>/py/
  golden_try.py`): identical fingerprints, no directory text in any output, 0.01-0.04 s per case.
  Memory test `test_topup_original_needs_no_more_memory_than_fsl` (ORIGINAL_TOPUP_CASES =
  TOPUP_CASES): 4 passed (fMRIPrep's call 74 MiB vs FSL 87.6; lsr/jac 20 vs 14.2 + entry 13.8;
  vb3d 28 vs 22.5), FSL's peaks served from the cache.
- b7c0567: **FSL's quadratic count** `ceil((2n + k - 4)/(2k)) + 2` (spline.rs `Axis::ncoef`; the
  cubic rule kept for order 3), and **FSL-145** (new defect, rule A; `larmorx_core::defects::
  FSL_145`; `Axis::two_when_short`/`with_fsl145`, `Params::fsl145`, `topup::meets_fsl145`, the
  CLI's notice; applytopup accepts either count from a file): on an axis shorter than two knot
  spacings FSL lays 2 quadratic coefficients (its field forced to 0 from 1.5 k on); the default
  lays the general count (4), reproduce FSL's 2. Unit test of 20 FSL counts + 5 short axes. The
  suite: transposed acquisitions (`_acquisition`), configuration-named outputs (`effective_args`:
  FSL's `test_acqp` names `--fout=field_test --iout=iout_test`), `--trace` with `-v`
  (`_split_trace` takes stages out of the middle of FSL's cost lines), the quadratic counts in
  `field_of_coefficients`; **random cases' band** (`_random_band`: FSL with its read-out times
  times 1 + 2^-23, through the cache; `compare(band_name=...)`). Findings 29-31 in
  docs/findings/fsl-topup-original.md. `recheck.py` (validation/investigations/cleanroom-topup).
- 00506c1: `estmov_off.py`, README section "Random testing (relay 5)".
- Wheel `python/larmorx/_core.abi3.so` = b7c0567's Rust (replaced atomically: cp to .core.new +
  mv); `target/release/larmorx` = b7c0567's Rust. tests/python/test_defects.py +
  test_fsl_topup.py: 18 passed. cargo test -p larmorx-fsl --lib topup: 17 passed; clippy -p
  larmorx-fsl -p larmorx-core -p larmorx-cli --all-targets -D warnings: pass; cargo fmt: pass;
  ruff on every touched Python file: pass. NOT run: the workspace gates, golden recording, smoke,
  the record.

**The trial** (`<workspace>/oracles/difftest/2026-11-03-topup-original`, log
`oracles/logs/difftest-2026-11-03-topup-original.log`): seed `topup-trial`, `--budget 0.5 --jobs
12 --threads 1 --memory-gib 1.5 --max-cases 14400 --shrink-budget 180`, boxed 20 GiB,
LARMORX_DIFFTEST_CACHE_MODE=use, LARMORX_TOPUP_BIN = the build of f1288d8's Rust (its run.json
says 4117c2b-dirty: launched before the commit). **14,400 cases in 11 minutes: 7,401 agree, 6,999
differ** (by kind: applytopup 4,448 cases 12 % differ (jac 0 %, lsr 8 %, vb2D 63 %, vb3D 92 %, vb4D
87 %); topup 8,516, 61 %; fMRIPrep's call on drawn pairs 1,433, 89 %). Every FSL run of it served
from the cache except the shrinking's (stored). Peaks <= 0.67 GiB per worker.

Triage so far (re-checks: `recheck.py --per-group 25` and `--per-group 12 --seed 2`, 427 + 223
replays of the drawn cases with the fixes, in `<scratch>/r5/recheck1`, `recheck2`):
1. larmorx mistakes, fixed: the quadratic count (486 cases: `_fieldcoef` header) -> b7c0567; now
   every header agrees.
2. a defect of FSL: FSL-145 (rule A) -> b7c0567.
3. the suite's own mistakes, fixed: transposed acquisitions (277 exceptions), config-named outputs
   (127 "values identical"), --trace with -v (47 "Standard output"), quadratic field evaluation.
   After the fixes, the only exact failures left in the samples: "Files written" (16: --debug's
   attempt files, i.e. a different number of Levenberg-Marquardt attempts: path) and 2 vb3D "values
   identical (no brain)" (FSL's output all 0 with char/short: FSL-143's divergence).
4. metrics, cases that estimate the movements (5,769 findings): `estmov_off.py` (40 sampled, FSL's
   --estmov=0 variants run fresh through the cache: 40 new FSL runs, stored): with the movements
   off 27 agree, 13 differ -> the movements' path (finding 14: FSL's rotation derivatives) makes
   two thirds; the orchestrator already decided that finding 14 waits for the spec writer's
   answer (the record's expected divergences).
5. metrics, cases without movements (cubic: 462 findings) and the third left in 4: FSL's own band
   with its read-out times changed in their last float bit is SMALL (3693: 1.2e-4 for the
   corrected p99, the original 3.0; 243: 0.398 for the field p99): FSL's result is stable, the
   original's path differs. But the original reaches the same or a lower cost: in the differing
   cases with -v (`<scratch>/py/trial_costs.py`) the original's final cost <= FSL's + 0.1 % in 80 %
   (movements, 877 cases) and 78 % (no movements, 130 cases), median ratio 0.9997, p10 0.92, p90
   1.005. The cases looked at are ill-determined (3693: one level, LM, 2 iterations, `--lambda`
   left out so Reg = 0, 180 voxels at the subsampled level; costs FSL 2590.25 -> 262.168 ->
   204.458, the original 2590.25 -> 264.838 -> 203.021: the FIRST step already differs).
   **Open question for 5b (one probe, then decide):** is the first LM step's difference the
   solver's precision in poorly-determined directions (the original's PCG `pcg_damped` stops at a
   tolerance; FSL's solve may be exact)? Probe: a temporary build with a much tighter PCG tolerance
   (or a direct solve) on 3693 (`<scratch>/py/one.py <trial> 3693`): if the original's costs move
   to FSL's 262.168, it is the solver (decide: tighter tolerance where it pays, measured on the
   record's cases and the speed); if not, triage as "explained: the same objective reached
   (cost), the field differs where the data do not determine it (no regularisation, few voxels)"
   with these numbers, and say so in the record.
6. applytopup lsr/vb (171 + 375 findings): 77 % of the lsr ones on lines of 20 or 24 voxels (do not
   divide 2^32: FSL-133, not reproduced by the original, the record's expected divergence;
   `trial_costs.py` prints the counts); vb3D/vb4D also FSL-143's divergence (reproduced, the
   record's expected divergences); vb2D the record's at/vb2d-aniso kind (transients). 12 lsr
   findings on 16-voxel lines not yet looked at (5b: one look).
Scratch helpers (not committed, used for the triage): `<scratch>/py/{trial_stats,trial_split,
trial_findings,trial_exact,trial_g,trial_mag,trial_costs,coef_probe,quad_fit,one,golden_try,
golden_show}.py`; `<scratch>/r5/` (logs, quad_triples.txt, replays, one/<n>/ dirs). 5b: commit
the ones the record's text cites (trial_costs, coef_probe/quad_fit) into
validation/investigations/cleanroom-topup/ with README lines.

## Next: relay 5b (the rest of sub-task 5)
1. The probe of item 5 above (one temporary build; restore the code after; at most a few steps),
   then the decision; if the solver is the cause and a fix is cheap, change it and re-run
   `speed_identity.py`-style checks only on what the change can affect.
2. Permanent cases (`difftest --promote <finding> --id <name> --tier smoke --expect pass --status
   fixed` / `--status register --defect FSL-145 --rule A`) for: the quadratic count (e.g. finding
   601 or 18), FSL-145 (601: 16x16x12 at warpres 20, 2 mm: x short), the transposed acquisition
   (243; expect divergence? it differs on metrics: check its replay first), test_acqp's outputs
   (12), --trace with -v (1233). The suite must load them: `cases(tier)` in parity/fsl_topup.py
   adds `difftest.permanent.cases("fsl-topup-original", ...)` as parity/fsl_fast.py does (read
   it). Into `validation/difftest/cases/fsl-topup-original/` (mine).
3. triage.toml entries for the HANDOFF (append op; `[[difference]]` of `fsl-topup-original` with the
   trial's check names as `signature`; statuses: fixed (quadratic count), register (FSL-145),
   harness (the four suite fixes), explained/open (movements: finding 14, waiting for the spec
   answer; ill-determined paths: per the decision of item 1; FSL-133/FSL-143 for lsr/vb)).
4. docs/api/fsl-topup.md: the original's sections (it still says "to be written ... original raises
   NotImplementedError"): both programs' options, FSL-144, FSL-145, speed (link
   docs/benchmarks/fsl-topup.md), the record link.
5. Gates: `scripts/orchestration/gates-rust.sh <worktree> all`, `gates-python.sh <worktree>
   --no-build`, `python -m larmorx_validation tables --check` (tools.toml lines through the handoff).
6. From the final clean commit, once each: the record `fsl-topup` (`docs/validation/fsl-topup.md`;
   `lxv.sh <worktree> parity fsl-topup --tier standard ...` boxed, --jobs to match 14 threads;
   LARMORX_TOPUP_BIN or the wheel), the smoke tier (`python -P scripts/orchestration/
   parity_smoke.py --workers 14`), `scripts/record_golden.py` (reusing the smoke marker); then
   commit golden.json and the record.
NOTE for the record: the FSL-145 change can change standard-tier cases with `--splineorder=2` on
short axes (opt/splineorder-2 has 32 voxels at k 3: not short); and the maths change (correctly
rounded sin/cos/exp) can move last bits: relay 4's identity baseline no longer applies; the record
re-runs every case anyway.

## For the handoff (sub-task 6), from relay 5b
- FSL-146 register row (behaviour only): "topup with volumes whose phase encodings run along
  different axes (AP, PA, LR, RL): the cost's gradient leaves out of each axis' Jacobian term
  the volumes not encoded along that axis, so it is not the cost's gradient: the minimisers step
  in a wrong direction (a first Levenberg-Marquardt step can raise the cost) and stop where the
  false gradient vanishes | rule A (measured: the default's final cost <= FSL's in 86-89 % of
  the trial's differing four-volume cases) | default the cost's gradient, naming FSL-146;
  reproduce: FSL's"; docs/api/upstream-defects.md and the API page: "**FSL-146**: with volumes
  encoded along different axes the gradient is not the cost's: corrected (the cost's own)."
- Replica side follow-ups (D13, replicas share the corrected default): FSL-146's switch for the
  replica's topup (default the cost's gradient on mixed axes; reproduce FSL's), as FSL-145's.
- triage.toml `append` op: `[[difference]]` entries of tool `fsl-topup-original`, one per
  category of finding 36 (statuses: fixed: quadratic count (signature `header`), SCG weight
  (metrics); register: FSL-145, FSL-146; harness: transposed acquisition (an exception: no
  check), configuration-named outputs (`values identical`), `--trace` with `-v` (`Standard
  output`); explained: finding 14 (metrics: rotations/translations, fields), FSL-133, FSL-143,
  vb2D transients, integer outputs, `--debug` attempt files (`Files written`), ill-determined
  paths; open: quadratic regulariser Hessian at even k (10753), lsr on lines dividing 2^32
  (8 cases, 8089)); the counts and reproducers from findings 36 (`triage_counts.py`).
- Spec questions (relay 5b): §4.3 the oblique mask (finding 32); §4.6 `λ'` = the current point's
  (the last cost evaluated) inside SCG's curvature probe (finding 34); §4.6 the bending energy of
  quadratic B-splines where a knot falls on a voxel (finding 36); §5 `--debug`'s images in FSL's
  radiological order without qform/sform (finding 33); FSL-146's mechanism (finding 35).
- difftest.md's `fsl-topup-original` row: + "the permanent cases in
  validation/difftest/cases/fsl-topup-original/ (7)".
- permanent.py's SUITES line `"fsl-topup": ("fsl-topup-original", "original")` was added in the
  branch (harness registry, minimal).

## For the handoff (sub-task 6), from relay 5
- FSL-145 register row (behaviour only): "topup --splineorder=2: an axis shorter than two knot
  spacings gets two B-spline coefficients (every other axis ceil((2n+k-4)/(2k))+2): the field's
  support ends inside the axis, forced towards 0 at its far end | rule A | default lays the full
  count and names FSL-145; reproduce: FSL's two"; docs/api/upstream-defects.md and the API page's
  line: "**FSL-145**: `--splineorder=2` lays two coefficients on an axis shorter than two knot
  spacings, so the field is forced towards 0 at its far end: the full count by default."
- Replica side follow-up (D13: replicas share the corrected default): the replica's topup gets
  FSL-145's switch (default: the general quadratic count on short axes; reproduce: FSL's two).
- Spec questions: §4.2 the quadratic count (finding 29) and the 2-coefficient case (finding 30).
- difftest.md's tool table: a `fsl-topup-original` row (parity.fsl_topup's difftest_case; the
  replica's draws from the `fsl-topup` stream, so the replica trial's seed serves every FSL run;
  the trial's numbers).
- The trial's directory and numbers for the site's random-testing panel.

## State after relay 4 (commits cd331b1..4117c2b; worktree clean) — SUB-TASK 4 DONE, NEXT: SUB-TASK 5

Relay 4 (sub-task 4, speed, §11), **every output bit-identical to relay 3b's (e86add7)**:
`validation/investigations/cleanroom-topup/speed_identity.py` ran the original on all 173 cases
of the suite's standard tier (fMRIPrep's 2 + 97 topup + 74 applytopup) in both modes (346 runs;
FSL not run) and hashed every output, stdout and stderr: 0 differ against the baseline at 1, 7
and 14 threads (and the baseline itself was already thread-independent). So **every suite status
is unchanged** (relay 2/3b's: topup 45 pass, 11 expected, 41 both-error; fMRIPrep's 2 pass;
applytopup 38 pass, 11 expected, 25 both-error; 0 fail). The suite itself was NOT re-run.
- Code (crates/larmorx-fsl/src/topup): `spline.rs` Op1::apply_axis along y/z whole lines and
  planes (axpy, vectorised), along x 64 lines transposed per block (for_each_init buffers);
  `apply_axis_with` (output buffer given), `accumulate_z` (z pass added into a sum);
  `images.rs` `taps_both` (indices once, wrap only at the ends, both weight sets; `taps_of`
  and `interp` use it); `model.rs` the eval's voxels written in place per slice (`Slices`),
  the single-axis Line case with its own loop, `fields()` sharing the x/y passes of h and dh,
  the regulariser's x passes once per derivative order + accumulate_z, gradient/Hessian voxel
  weights in place (flat arrays), `Level::recycle(Eval)`, `Level::pool()`; new `pool.rs`
  (`Pool`: arrays kept at exact length, `zeros` (zeroed, parallel fill) / `take` (contents as
  they are: only where every value is written) / `give`, at most 48 kept, >= 32768 values);
  `stencil.rs` `pcg_damped` (the damped Hessian laid out by offset, `ByOffset`, transposed 8
  offsets at a time; product per line of rows), `solve` shared by `pcg`; Stencil::zeros_in,
  add_weighted_in (pool); `minimise.rs` LM uses pcg_damped and recycles rejected/replaced
  evals and Hessians; SCG reuses xp/xnew/diff and recycles ep/en/gplus/gold.
- Benchmark `docs/benchmarks/fsl-topup.md` + `.json` (bench.py at 11f3cb0, load 5 of 32, the
  replica agent beside): fMRIPrep's call ds005040: FSL 32.63 s (1) / 12.32 s (--nthr=14),
  replica 29.69 s, original 2.83 s (1) / 0.80 s (14); ds001600: FSL 77.35 / 27.38, replica
  71.35, original 7.70 / 1.98; applytopup lsr (ds005040's pair, FSL's topup result): FSL 0.42 s,
  replica 3.47 s, original 0.13 / 0.04 s. Replica bit-identical to FSL's one-thread outputs on
  all three; the original's outputs the same bits at 1 and 14. Before (e86add7): ds005040
  7.3 s / 2.1 s, ds001600 18.2 s / 4.4 s. Peak memory ds001600: 187 -> 146 MB (1 thread),
  175 -> 159 MB (14); page faults 578k -> 123k.
- **FSL runs of this relay: 5 fresh timed runs** (bench.py, through oracle_cache.run with a
  `never` reason, not stored): the two topup calls at one thread and --nthr=14, the applytopup
  call. Nothing else ran FSL.
- Not done (in the report's last section): applytopup band solve (0.13 s, would change bits),
  fewer bytes per solver product (changes sums' order), SIMD dispatch (the crate forbids
  unsafe; would go to larmorx-core), fusing the regulariser's y/z passes (~3 of 7 ms per eval
  on 14 threads).
- The wheel: `python/larmorx/_core.abi3.so` rebuilt from 01e7697's Rust (= HEAD's; maturin
  build into `<scratch>/wheel4`); tests/python/test_fsl_topup.py + test_defects.py: 18 passed
  (FSLDIR, LARMORX_NC_BIN unset).
- Gates run: cargo fmt, clippy -p larmorx-fsl -p larmorx-cli --all-targets -D warnings (pass),
  cargo test -p larmorx-fsl --release (92 passed), ruff check/format of the new scripts,
  check_spdx (pass). NOT run: workspace gates, golden, smoke, records (sub-task 5).
- Helpers: `speed_identity.py`, `bench.py` (README section "Speed"); scratch: `<scratch>/speed/`
  (the baseline binary `larmorx-base`, identity JSONs `base-t*.json`, `s1..s5-t*.json`, the
  micro-benchmark crate `mb/`, the temporary timers `instr/` (never committed), `orig.sh`).

## Relay 4's plan for relay 5 (kept for reference; see relay 5's state above)
- Random testing: descriptor `fsl-topup-original` (model: `difftest/tools/fsl_topup.py`,
  realistic cases only), one trial <= 30 min boxed with LARMORX_DIFFTEST_CACHE_MODE=use, triage,
  replays (<= 8 at a time, memory findings skipped).
- Golden: `lx.fsl.topup` and `lx.fsl.applytopup` leave REPLICA_ONLY_FUNCTIONS in
  tests/golden/registry.py; quick configurations <= 32³; inputs relative to the case's dir; the
  known "No golden case covers: cli:fsl applytopup, cli:fsl topup".
- Memory test (tests/parity/test_memory_peak.py TOPUP_CASES: the original <= FSL + 10 % + 8
  MiB; the original's ds001600 peak is now 146-159 MB), thread test (parity/thread_pools.py:
  every parallel path; the pool's parallel fill and the solver's lines included).
- docs/api/fsl-topup.md's original sections (both programs, vb methods, FSL-144; speed: link
  the benchmark).
- Then from the final clean commit, once each: record `fsl-topup` (docs/validation/fsl-topup.md),
  the parity smoke tier, record_golden.py (reusing its marker); the workspace gates.

## Relay 3b's state (commits 3746f47..e86add7; sub-task 3 done) — kept for reference

Relay 3b (sub-task 3, the vb methods and FSL-144):
- `crates/larmorx-fsl/src/topup/vb.rs` (new): `--method=vb2D|vb3D|vb4D` (findings 23-27): per
  collection (x first) and block (vb2D: slice x volume, 50 iterations; vb3D: volume, 20; vb4D:
  all volumes, 20, no coupling across volumes), first estimate per line with the circulant
  line prior (1,-4,6,-4,1), then per iteration: print, φ (residual + Σ tr(KᵀK Σ_l)), λ (fᵀQf +
  Σ tr(Q_ll Σ_l)), Gauss-Seidel over the lines in storage order (Σ_l = inv(φKᵀK + λQ_ll), by
  Cholesky, Gauss-Jordan when not PD; factors in parallel batches of 64 lines, the sweep
  serial); Q = DᵀD, D = Σ_d (v_pe/v_d) Δ_d, truncated at the edges, periodic along the PE;
  FSL-143 reproduce halves the (±1 PE, ±1 slice) coupling; FSL-142 reproduce counts 2 per
  voxel (vb3D/vb4D); FSL-141 reproduce counts in i32; no line skipped (invalid voxels' periodic
  values used), output masked voxel by voxel (NOT lsr's whole lines). Stdout: NCollections,
  NScans (collection 1's), `slice = s` once per slice, the iteration lines. A singular line
  matrix is refused (`larmorx: error: applytopup: --method=...: the restoration's matrix of a
  line is singular ...`, exit 1; never seen).
- `apply.rs`: `moved_images` and `line_starts` factored out of `lsr` (shared with vb);
  `cholesky`/`cholesky_solve` pub(super).
- `cli/applytopup.rs`: vb accepted (NOT_YET const now unused but kept pub); the restorations
  share lsr's checks (FSL-134 other grid refused in both modes, FSL-135 without movements,
  matrices as movements); stdout written; notices FSL-141 (only when the 32-bit count differs),
  FSL-142 (vb3D/vb4D collection of != 2 images), FSL-143 (every vb3D/vb4D run); FSL-133's
  notice also for vb (same resampling matrices).
- **FSL-144** (orchestrator's decision; topup refuses pz != 0, §3.2 -> rule B): `jac` with any
  image whose PE has a z component: default refuses (exit 1, `... (FSL-144)`), reproduce gives
  FSL's output with a `reproduced` notice. `larmorx_core::defects::FSL_144` added. The suite's
  `at/jac-z`: FSL's output in reproduce + `FIX_REFUSES` (new dict in parity/fsl_topup.py: the
  default's refusal naming FSL-144): pass 9/9.
- Suite (parity/fsl_topup.py): the vb iteration lines' numbers compared as a metric check
  ("vb: the printed phi, lambda and their ratio", 4 significant digits = 5e-4), the text
  structure exact; EXPECTED_DIVERGENCES += at/vb2d-long, vb3d-long, vb4d-long (FSL-133, as
  lsr-long), at/vb3d-aniso, vb4d-four, vb3d-three (FSL-143 divergence amplifies), at/vb2d-aniso
  (vb2D transients in the top slices, 8e-4; finals 1e-5). **Final at/ run rec/r3h (all 74
  applytopup cases, standard tier): 38 pass, 25 both-error, 11 expected, 0 fail, 0 skipped**
  (the vb cases alone, rec/r3g: 8 pass, 2 both-error, 8 expected).
- Evidence for the default: the original's fix mode against the replica's fix mode (black box)
  on 12 at/vb cases: images p99 <= 1.8e-5, printed <= 1.2e-5 (vb3D/vb4D), <= 8.4e-4 (vb2D
  transients) (finding 26). Against FSL with FSL's own resampled inputs (tiny probes): exact
  to float32 (finding 23).
- Python: tests/python/test_fsl_topup.py: vb2D runs (stdout head, shape), FSL-144's refusal
  (22 passed with the new wheel, FSLDIR/LARMORX_NC_BIN unset). The wheel's `_core.abi3.so` in
  python/larmorx/ is built from e86add7's Rust (Rust unchanged after 58566b2).
- Gates run: cargo test -p larmorx-fsl -p larmorx-core -p larmorx-cli --lib (pass), clippy of
  those three --all-targets -D warnings (pass), cargo fmt, ruff check/format of the touched
  Python, check_spdx (pass). NOT run: workspace gates, golden, smoke (sub-task 5).
  **Known** (unchanged): tests/golden's "No golden case covers: cli:fsl applytopup, cli:fsl
  topup" (sub-task 5).
- Investigations: vb_probe.py + vb*.py/vb*.sh (README section "applytopup's vb methods"); they
  read probes from `VB_DIR` (this relay's: `<scratch>/vb`: probes p1..p18, q5, q8, q9, the
  replica's runs `vb/rep`, fix comparisons `vb/fix`, effective inputs `vb/effin`).

## For the handoff (sub-task 6), from relay 3b
- FSL-144: the register's summary row (behaviour only): "applytopup --method=jac with a phase
  encoding that has a z component (topup refuses such an acquisition) leaves the component out
  silently: an image encoded along z is written uncorrected | rule B | default refuses, naming
  FSL-144; reproduce: FSL's output"; docs/api/upstream-defects.md and docs/api/fsl-topup.md's
  defect list: "**FSL-144**: `--method=jac` with a phase encoding along z (which topup does not
  estimate) leaves the image uncorrected silently: refused."
- next-steps follow-up for the replica side (as FSL-111 was): the replica's `fsl applytopup`
  gets FSL-144's switch (default refuses jac with pz != 0, reproduce FSL's output), D13.
- The suite's new FIX_REFUSES dict and the vb metric check (for the record's text).

## For the handoff (sub-task 6), from relay 4
- The site's speed table: docs/benchmarks/fsl-topup.json (the rows above).
- For the replica side (observation, black box): the replica's applytopup `lsr` on ds005040's
  pair takes 3.47 s against FSL's 0.42 s (8 times slower; its outputs FSL's byte for byte).
- docs/benchmarks/README.md's index lists only mri-hmc among the FSL-family reports (no fsl-bet,
  fsl-flirt, fsl-fast, fsl-topup lines): the orchestrator may add them.

## Spec questions so far (for the handoff)
- §4.5: the spline is periodic along EVERY axis (observed), not only along the PE.
- §4.3: the mask's edge rule is along the axes OTHER than the PE (x for y-PE), plus z.
- §4.2: "centred on the grid" — observed coefficient i at (i-1)k (k>1), i (k=1); not centred.
- §4.4: the refits' penalty: `0.005²` times the squares of only the two second differences that
  wrap around the ends (`c[n-1]-2c[0]+c[1]`, `c[n-2]-2c[n-1]+c[0]`), not "the second
  differences" (findings 11; the spec should say which rows); the refit when warpres changes at
  the same k; the intermediate spacing = round(previous warpres / new voxel).
- §6: "if it was subsampled, the smoothing and subsampling are removed": confirm that a last
  level WITHOUT subsampling keeps its smoothing in --iout and the mean (observed, findings 12).
- §4.6/§4.7: the gradient factor for m > 2 (1/(m-1) vs 2/m) unverified; `Reg` = λ·SSD·E with
  E a sum at integer points (not the integral).
- §2.6: the datain reader's messages carry the prefix `TopupFileIO::` (spec omits it).
- §2.6/§10 (FSL-121): FSL's stream starts with an empty line before "basic_string: ...".

- §4.3/§4.6 (relay 2): the movement model: R = Rx(rx) Ry(ry) Rz(rz) with the signs of findings
  13; the sampling `R^T (x - c - t) + c` with `c` the centre of the image interpolated (each
  level's subsampled regridded grid); the field at the unmoved voxel; `--rbmout` about the
  input's centre. The spec should state these.
- §4.6 (relay 2): FSL's rotation derivatives (gradient and Hessian) are not the exact
  derivatives of its cost (findings 14: exact for linear images, 0.4-26 % off on real ones; a
  zero-derivative translation gets -7e-7): what does FSL compute? The original uses the exact
  derivative; 9 suite cases with unconverged movements differ beyond §12 because of it.
- §4.7 (relay 2): with a zero diagonal entry (λ = 0, a coefficient outside the mask) FSL's
  Levenberg-Marquardt ends the level without a step (findings 15): the spec should say so.
- §4.2 (relay 2): the quadratic layout is the cubic one (13 coefficients for 32 voxels at
  spacing 3, not the spec's 11).
- §4.6 (relay 2): the membrane energy's cost measures derivatives per voxel (FSL-117's text
  says so; §4.6 says "in mm" for both).
- §2.6/§10 FSL-125 (relay 2): a negative --fwhm (-8) acts as no smoothing (the first cost equals
  --fwhm=0's), not "a kernel of that negative width".
- §2.7 (relay 2): the log's `--estmov` is `100` when not given.
- §5/FSL-118 (relay 2): at the first level FSL says "Error occurred when preparing to run topup".
- §5 (relay 2): `--trace`'s stage names are FSL's internals; the original prints its own (the
  suite compares the other lines); `--debug`'s MaskedDiff is on the regridded image's grid.

- §7.3 (relay 3): d's z component is 0: a z phase encoding leaves `jac`'s images uncorrected
  silently (jac-z: output = input). Candidate defect (rule B: FSL-144?) for the orchestrator: the
  replica keeps FSL's behaviour in both modes, so the original does too (no new ID used).
- §7.2/§7.3 (relay 3): the mask: along every axis but the PE's when the PE is along x or y
  alone, along every axis for an oblique PE (jac-oblique); the spec should say so.
- §7.3/FSL-134 (relay 3): the field image's Jacobian scale uses the field image's own voxel
  sizes (vx/vy of the field: probe134), not the images'.
- §7.4 (relay 3): "a line with any voxel outside the mask ... written 0 in every collection's
  output": confirmed (lsr-four); the spec could add "so the voxel is 0 in the sum".
- §7.2 (relay 3): four-column movements sample at M⁻¹x (probe_mat), as the six parameters do.
- FSL-133 (relay 3, findings 21): what exactly FSL reads at the wrap-around of lines whose
  length does not divide 2^32 (the probes: one bright voxel at 0, 1, 15, 29 of a 30-voxel line;
  FSL's restoration differs from the circular one at the line's ends only; the register's
  "(2^32-1) mod n" field read does not reproduce it).
- FSL-134 (relay 3, findings 22): how FSL lays a field of the images' voxel count but another
  shape (jac-other-shape: 0 for x >= 32 of 64, other values unexplained).

- §7.5 (relay 3b, findings 23-25): the spec should state: the first estimate's line prior
  `(1,-4,6,-4,1)` circulant with φ = λ = 1 and its Σ in the first traces; the order print, φ,
  λ (λ uses the old f and Σ, not the new φ), then the lines Gauss-Seidel in storage order with
  per-line Σ_l; the traces tr(KᵀK Σ_l), tr(Q_ll Σ_l); Q = DᵀD with D = Σ_d (v_pe/v_d) Δ_d over
  the slice's or volume's axes, truncated at the edges (rows centred outside the image), periodic
  along the PE; vb4D without coupling between volumes; `slice = s` once per slice, then per
  volume; collections x first; NScans the first collection's count; invalid voxels' periodic
  values enter (no line skipped); the output masked voxel by voxel (unlike lsr's lines).
- §10 FSL-143 (relay 3b): "half the weight of the in-slice coupling" is exactly the (±1 along
  the PE, ±1 slice) entries at w_pe·w_z instead of 2·w_pe·w_z; every other entry as DᵀD.
- §7.4/§9.4 (relay 3b, finding 27): FSL's resampling of images with an axis of <= 8 voxels is
  not interpolating at the grid points (up to 1 % off for an axis of 4 voxels, 2e-5 at the
  ends of 8-voxel lines; exact for 16 voxels): what does FSL's spline do on short axes?
- FSL-144 (relay 3b): decided by the orchestrator (rule B), recorded in finding 28.

- §4.2 (relay 5, findings 29-30): the quadratic B-splines' count is `ceil((2n + k - 4)/(2k)) + 2`
  (not the cubic's), centred at `(i - 1) k` as the cubic's; an axis shorter than two spacings
  gets FSL's 2 (FSL-145, rule A).
- §5/§2 (relay 5): `--trace` with `-v`: FSL prints a stage's "Entering ..." inside a cost line.

## Clean-room statement (relay 1)
Opened: CLAUDE.md, BRIEF.md, specs/topup.md (whole); larmorx's own Apache code: crates/larmorx-fsl
(Cargo.toml, lib.rs, cli/{mod,common,options (part),fast (parts),fmt,bet2 (read/resolve)}.rs,
bet/{image (part),mod (Vol)}.rs, fast/mod.rs, flirt/mat.rs (parse_matrix_text)),
crates/larmorx-cli/src/{replica.rs, lib.rs} (parts), crates/larmorx-io nifti header (field list),
crates/larmorx-core defects.rs/parallel.rs (greps), crates/larmorx-py/src/fsl.rs (grep);
python/larmorx/fsl/{topup.py, fast.py (parts)}, python/larmorx/_replica.py (select);
validation: parity/fsl_topup_replica.py (lines 1-460, 576-780, the err cases' list),
parity/fsl_flirt_replica.py (fsldir, _environment, oracle_run, run_replica, compare head),
oracle_cache (grep); docs/validation/fsl-topup-replica.md (head + grep "thread");
scripts/orchestration/{env.sh, lxv.sh, boxed.sh, README (grep)}; tests/python/test_fsl_topup.py
(grep lines); pyproject.toml (ruff grep); sdcflows' b02b0.cnf and FSL's etc/flirtsch/b02b0.cnf
(configuration data; `ls etc/flirtsch | grep b02b0`); `grep -l "ruff: noqa"` inside
validation/investigations/cleanroom-fast and cleanroom-flirt (file names only).
Never opened: FSL's source, crates-nc/, replica crates, restricted findings, † sections,
docs/parts/, replica investigations/probes/triage/permanent cases, replica git history.
Ran: FSL only through the oracle cache (use mode) via the scripts above: served from the cache:
debug/one-level, the 39 err/ cases, fMRIPrep's two calls; NEW (stored): probe scg, probes A, B,
C, D, E, and fMRIPrep's ds005040 call with -v (7 new FSL runs, each seconds to ~35 s, boxed 8
GiB). The original: target/release/larmorx in boxes of 8-12 GiB (one at a time). Nothing left
running.

## Clean-room statement (relay 1b)
Opened: BRIEF.md, NOTES.md; specs/topup.md (the heading list, §4, §6, §9, §11, §12);
docs/validation/fsl-topup-replica.md (head, thread rows, grep); docs/figures.md (grep "cache");
validation/src/larmorx_validation/evidence/fsl_topup.py (lines 40-240, grep); the figures cache's
topup entries (`<workspace>/oracles/figures-cache/fsl-topup/`: listing, the fMRIPrep entries' FSL
outputs at one and four threads, read as data); the oracle cache's findings.jsonl (its last line:
an AFNI container record, by mistake of a tail); parity/fsl_fast.py (whole), parity/
fsl_topup_replica.py (lines 1-205, 300-668, 1270-1320, 1955-2063, its def list), parity/harness.py
(def list, Check), parity/defects.py (27-140), validation/src/larmorx_validation/__main__.py
(lines 40-50); larmorx's own code: python/larmorx/fsl/topup.py (whole), fast.py (parts),
_replica.py (100-236, ORIGINAL), crates/larmorx-py/src/fsl.rs (grep), crates/larmorx-fsl/src/
cli/{mod.rs (grep), topup.rs (parts), common.rs and fast.rs (grep of refusal formats)},
topup/{mod.rs, spline.rs} (whole), crates/larmorx-cli/src (grep LARMORX_IMPLEMENTATION),
crates/larmorx-fsl/PROVENANCE.md (grep); tests/python/test_fsl_topup.py (1-130),
test_fsl_fast.py (96-140); scripts/orchestration/{boxed.sh head, lxv.sh}; my own scratch
scripts and runs. Imported without opening: fsl_topup_replica's re-exports (_environment,
Completed, fsldir, workspace).
Never opened: FSL's source, crates-nc/, replica crates, restricted findings, † sections,
docs/parts/, replica investigations/probes/triage/permanent cases, replica git history,
parity/fsl_flirt_replica.py, parity/fsl_mcflirt_replica.py.
Ran FSL (all through the oracle cache, use mode): NEW: probes F, G, H (3 runs, seconds each,
boxed 8 GiB); FSL `--nthr=4` on fMRIPrep's two calls twice (first suite run: 2 fresh runs, as
the cache's policy runs above one thread until 3 identical runs; later served); the 2 never-cached
`--profile` cases at each of two suite runs (4 runs, seconds). Everything else served from the
cache (about 110 FSL runs served). The original: target/release/larmorx and the wheel in boxes
of 8-24 GiB, one box at a time. Nothing left running.

## Clean-room statement (relay 2)
Opened: BRIEF.md, NOTES.md; specs/topup.md (§2 to §6, §8 to §12, the heading list); larmorx's own
code in the worktree: crates/larmorx-fsl/src/topup/{mod,model,minimise,stencil,spline,images}.rs
(whole), cli/topup.rs (whole), cli/mod.rs (module list, Settings), bet/image.rs (Output, Values,
write: grep), crates/larmorx-core/src/defects.rs (the FSL-103..134 rows, lines 495-525, and
Notice::new), crates/larmorx-fsl/PROVENANCE.md (the topup section); python/larmorx/_replica.py
(grep LARMORX_NC_BIN), python/larmorx/fsl/topup.py and tests/python/test_fsl_topup.py (grep);
validation: parity/fsl_topup.py (whole, mine), parity/fsl_topup_replica.py (def list, lines
205-360 and 668-1330: the inputs, Run, the debug/option/config/error/thread case lists),
parity/defects.py (add_fix_check), parity/harness.py (Check, Outcome, CheckList, run_one);
docs/findings/fsl-topup-original.md (mine); validation/investigations/cleanroom-topup/
(README.md, fsl_case.py, mine.py head, tp.py lines 1-210: mine); scripts/orchestration/
{boxed.sh head, lxv.sh head}; FSL's and the replica's outputs as data in scratch.
Also: `ls -la` of the one file `<workspace>/larmorx/crates-nc/target/release/larmorx-nc` in the
main checkout (to see whether the replica program exists; nothing else listed or opened there);
then the replica program built from this worktree's crates-nc by `cargo build --release --bin
larmorx-nc` into `<scratch>/nc-target` (sources never opened; only the build log's last line
read).
Never opened: FSL's source, crates-nc/ sources, replica crates, restricted findings, † sections,
docs/parts/, replica investigations/probes/triage/permanent cases, replica git history, the
oracle cache's own files, parity/fsl_flirt_replica.py.
Ran FSL (through the oracle cache, use mode, boxed 4-8 GiB): NEW: probes I (defaults -v), J, K
(membrane SCG/LM -v), L (trilinear -v), M (FSL-118 at level 1), N, O (oblique and b02b0_7T with
--estmov=0), P (--fwhm=-8 --miter=0 -v), Q (--lambda=-0.005 -v): 9 runs, seconds each; the
never-cached --profile cases at each suite run (2 runs x 4 runs). Served from the cache:
debug/movements, debug/hessian, debug/trace, debug/profile, opt/levels, opt/four-volumes,
opt/x-phase-encoding, opt/defaults, opt/splineorder-2 and the suite's ~100 FSL runs at each of
four suite runs (rec/r2a..r2d), fMRIPrep's two calls and their --nthr=4 band (rec/r2e).
The replica (black box, `<scratch>/nc-target`): ~40 runs on synthetic 16x16x8 ramps and curved
images (mov_replica_probe.py) and 5 on the half-resolution pair (mov_vs_replica.py), seconds
each. The original: target/release/larmorx in boxes of 6-24 GiB (the suite at --jobs 2-3, 4
threads per case: at most 12 threads). Nothing left running.

## Clean-room statement (relay 3)
Opened: BRIEF.md, NOTES.md; specs/topup.md (the heading list, §3, §4.1-4.5, §6, §7 to §12);
larmorx's own code in the worktree: crates/larmorx-fsl/src/topup/{mod.rs, movement.rs (1-152),
images.rs (1-200, 280-460), model.rs (140-560), spline.rs (70-125, 300-325)} (mine),
cli/topup.rs (1-60, 176-1451: mine), cli/mod.rs (whole), cli/common.rs (grep: read_rows,
abort, image_exception, print_notices), cli/bet2.rs (grep: resolve_image, FSLOUTPUTTYPE_UNSET),
bet/image.rs (grep: Input, read, Values, write, output_file), bet/mod.rs (Vol),
flirt/mod.rs (OutType), flirt/engine.rs (2240-2340, 2515-2560: output types and the
conversion), crates/larmorx-core/src/defects.rs (515-535, 600-700: FSL-124..143 rows, Notice),
crates/larmorx-mri/src/hmc/image.rs (490-560: convert_values), crates/larmorx-cli/src/
{replica.rs (240-275, 700-720), lib.rs (405-460)}, crates/larmorx-py/src/fsl.rs (19-44),
crates/larmorx-fsl/PROVENANCE.md (the topup section); python/larmorx/fsl/{topup.py (1-30,
155-180, 280-443), __init__.py (15-30)}, tests/python/test_fsl_topup.py (120-200 and greps),
tests/golden/registry.py (1000-1020, grep topup), validation: parity/fsl_topup.py (whole, mine),
parity/fsl_topup_replica.py (205-460, 576-617, 1321-1955: the inputs, Run, _prepare, the at/
cases), validation/src/larmorx_validation/__main__.py (378-418), docs/findings/
fsl-topup-original.md (mine), validation/investigations/cleanroom-topup/{fsl_case.py head,
README.md tail} (mine), scripts/orchestration/{env.sh, lxv.sh head}; FSL's and the
original's outputs as data in scratch.
Never opened: FSL's source, crates-nc/ (sources and its target), replica crates, restricted
findings, † sections, docs/parts/, replica investigations/probes/triage/permanent cases, replica
git history, the oracle cache's own files, the main checkout, parity/fsl_flirt_replica.py.
Ran FSL (applytopup only, through the oracle cache, use mode, boxed 4-16 GiB): served from the
cache: the 72 at/ cases (at_case.py cases) and the suite's at/ runs (three suite runs: r3a, r3b,
r3c; 19 aborting runs per suite run are never stored and ran again, under a second each); NEW (stored): the probes probe133 (4 runs: 30-voxel lines), probe32 (2 runs), probe134 (1),
probe_mat (1): 8 new FSL runs, under a second each. The original: target/release/larmorx and
the wheel, boxed 4-16 GiB (the suite at --jobs 3-4, 4 threads per case). The replica was not
run. Nothing left running.

## Clean-room statement (relay 3b)
Opened: BRIEF.md, NOTES.md; specs/topup.md (the heading list, §7 to §12); larmorx's own code in
the worktree: crates/larmorx-fsl/src/topup/{apply.rs (whole), spline.rs (100-200), mod.rs (the
module list), vb.rs (mine, new)}, cli/applytopup.rs (1-60, 200-260, 380-760: mine), cli/fmt.rs
(1-40), crates/larmorx-core/src/{parallel.rs (with_threads), defects.rs (505-545)},
crates/larmorx-fsl/PROVENANCE.md (the topup section, grep), python/larmorx/fsl/topup.py (grep
stdout), tests/python/test_fsl_topup.py (140-200), tests/python/test_defects.py (grep);
docs/api/fsl-topup.md (grep: lines 31, 95-165); validation: parity/fsl_topup.py (mine: 1-30,
86-147, 200-302, 780-1000, greps), parity/fsl_topup_replica.py (1555-1680, the vb and jac-z
case lines, the acq_z line: greps), parity/defects.py (Change, add_fix_check), __main__.py
(378-420); docs/findings/fsl-topup-original.md (mine); validation/investigations/cleanroom-topup/
(README, wrap_probe.py, at_case.py: mine); scripts/orchestration/{env.sh, lxv.sh, boxed.sh
head}; FSL's, the original's and the replica's outputs as data in scratch.
Never opened: FSL's source, crates-nc/ sources (the replica program `<scratch>/nc-target/release/
larmorx-nc` run as a black box only), replica crates, restricted findings, † sections,
docs/parts/, replica investigations/probes/triage/permanent cases, replica git history, the
oracle cache's own files, the main checkout, parity/fsl_flirt_replica.py.
Ran FSL (applytopup only, through the oracle cache, use mode, boxed 4-16 GiB): NEW (stored):
the probes p1..p18, q5, q8, q9 (vb2D/vb3D/vb4D/lsr/jac on 1x8x1 to 16x16x16 images: about 60
runs, under a second each) and their zero-field jac runs for FSL's resampled inputs (vbeff.sh,
vbeff0.sh: about 15 runs) and aniso_eff (1); a few first attempts with broken arguments
(p5/p8/p9: usage, exit 1, kept in scratch). Served from the cache: the suite's at/ runs (runs
r3d..r3h: the 18 vb cases five times, all 74 at/ cases once; FSL's aborting runs re-run at each,
under a second). The replica (black box): fix and reproduce runs on p4, p10, p12, p14 and fix
runs on 12 at/vb cases (seconds each, boxed 8 GiB). The original: target/release/larmorx and the
wheel, boxed 4-16 GiB (the suite at --jobs 3-4). Nothing left running (the one running scope,
run-rf3920b5..., is the orchestrator's known orphan from 2026-11-02, not mine).

## Clean-room statement (relay 4)
Opened: BRIEF.md, NOTES.md; specs/topup.md (the heading list, §1, §9, §11, §12); larmorx's own
code in the worktree: crates/larmorx-fsl/src/topup/{mod.rs, model.rs, stencil.rs, minimise.rs,
images.rs (whole), spline.rs (1-325, 405-506)}, cli/topup.rs (27, 495-600, 900-930, 1051-1111,
greps), cli/topup_debug.rs (grep of fn names), cli/mod.rs and cli/applytopup.rs (greps: threads),
crates/larmorx-core/src/{parallel.rs (fn list), math/mod.rs (55-80, 160-190: the dispatch
pattern)}, crates/larmorx-fsl/src/lib.rs (grep forbid), crates/larmorx-fsl/PROVENANCE.md (the
topup section), Cargo.toml (grep profile), rust-toolchain.toml; validation: parity/fsl_topup.py
(170-210, 899-1070, greps), parity/fsl_topup_replica.py (def list, 91-130, 395-420, 1435-1462,
1955-1990), parity/harness.py (the Case class), tests/parity/test_memory_peak.py (grep topup),
docs/validation/fsl-topup-replica.md (grep memory), validation/investigations/cleanroom-topup/
{README.md, fsl_case.py, mine.py (1-80)} (mine), validation/investigations/cleanroom-fast/
bench.py and docs/benchmarks/fsl-fast.md/.json (the model), docs/benchmarks/README.md (grep),
scripts/orchestration/{env.sh, boxed.sh head, lxv.sh head}; objdump of the worktree's own
target/release/larmorx (floor's lowering); FSL's, the original's and the replica's outputs as
data in scratch.
Never opened: FSL's source, crates-nc/ sources (the replica program `<scratch>/nc-target/release/
larmorx-nc` run as a black box only), replica crates, restricted findings, † sections,
docs/parts/, replica investigations/probes/triage/permanent cases, replica git history, the
oracle cache's own files, the main checkout, parity/fsl_flirt_replica.py (its run_replica used
through fsl_topup_replica's re-export without opening).
Ran FSL: 5 fresh timed runs (bench.py, boxed 16 GiB, through oracle_cache.run with a `never`
reason, not stored): fMRIPrep's two topup calls at one thread and at --nthr=14, one applytopup
lsr call. Nothing served from the cache (the identity harness prepares the suite's inputs only;
the derived inputs already existed). The replica (black box): the three bench calls. The
original: target/release/larmorx (baseline copy and builds) in boxes of 4-24 GiB, at most one
box at a time besides the bench (the identity harness at --jobs 14/2/1 x 1/7/14 threads), the
micro-benchmark `<scratch>/speed/mb` (synthetic 80x80x60 images). Stopped myself: one hung run
of my own instrumented build (a stderr lock deadlock in a temporary timer) via TaskStop. Nothing
left running.

## Clean-room statement (relay 5)
Opened: BRIEF.md, NOTES.md; docs/validation/difftest.md (whole); larmorx's own code in the
worktree: validation/src/larmorx_validation/difftest/tools/{fsl_topup.py, fsl_fast_original.py,
__init__.py} (whole), difftest/core.py (1-200), difftest/inputs.py (1-90), difftest/cli.py (32-122,
200-270), difftest/runner.py (greps: threads, explore), difftest/gen.py (Draw), difftest/tools/
_ants.py (prefix_of, case_id, describe); parity/fsl_topup.py (mine: whole, in parts),
parity/fsl_topup_replica.py (85-135, 296-420, 433-450, 1985-2063: oracle_run, Run, _prepare,
_oracle, difftest_case, run_case), parity/thread_pools.py (whole, in parts), tests/parity/
test_thread_pools.py, tests/parity/test_memory_peak.py (1-30, 85-130, 425-575), tests/golden/
registry.py (the helpers, inputs, Context, FAST's and FLIRT's cases, the replica-only list),
tests/golden/test_golden.py (60-97), scripts/record_golden.py (argument list), python/larmorx/fsl/
topup.py (47-110, 160-355, 371-396), crates/larmorx-fsl/src/topup/{movement.rs (15-35), images.rs
(220-235), spline.rs (100-200, 480-500, 545-590), apply.rs (1-140: greps), mod.rs (58-150,
190-330)}, cli/topup.rs (greps; 515-535, 862-905, 1115-1180), cli/applytopup.rs (90, 210-235,
395-480, 520-600, 700-750, 875-900), crates/larmorx-core/src/{defects.rs (530-550), parallel.rs
(26-60), math/mod.rs (25-160)}, docs/api/fsl-topup.md (1-33, 120-162), docs/findings/
fsl-topup-original.md (mine: 140-328), validation/investigations/cleanroom-topup/{README.md (head),
final_ssd.py (head)} (mine), pyproject.toml (grep filterwarnings), scripts/orchestration/{lxv.sh,
env.sh} (heads). The listing of `validation/src/larmorx_validation/difftest/tools/` and `.../
difftest/` (the harness's own package, which the brief names; no replica part's investigations,
probes, triage or permanent cases listed). Data read: the trial's own results and findings (the
original's and FSL's outputs as data), FSL's outputs of the trial's cases served from the cache
in scratch dirs.
Never opened: FSL's source, crates-nc/ (sources and target), replica crates, restricted findings,
† sections, docs/parts/, replica investigations/probes/triage/permanent cases (validation/difftest/
triage.toml and cases/ not opened or listed), validation/investigations/difftest/, replica git
history, the oracle cache's own files, the main checkout, parity/fsl_flirt_replica.py,
oracles/difftest/2026-11-03-topup-replica (the replica trial's directory: only its seed, from
difftest.md, was used).
Ran FSL (only through the oracle cache, use mode): the trial's 14,400 cases (every FSL run served:
the replica trial's; the shrinking's runs new, stored: a few hundred small runs, seconds each);
the re-checks (650 replays of drawn cases: FSL served; their band runs new, stored: up to one per
replay whose metrics failed); `estmov_off.py` (40 new FSL runs, stored); the memory test (FSL's
peaks served). The original: target/release/larmorx and the wheel, boxed (the trial 20 GiB, the
re-checks 14 GiB, the others 4-12 GiB; never more than 26 GiB of my boxes at once). The replica
was not run. Nothing left running.

## Clean-room statement (relay 5b)
Opened: BRIEF.md, NOTES.md; specs/topup.md (§3.2, §4.5-§4.7: lines 255-272, 340-390, 395-410);
larmorx's own code in the worktree: crates/larmorx-fsl/src/topup/{minimise.rs (1-230),
model.rs (15-60, 171-240, 300-430, 511-522, 556-740, 785-960), mod.rs (40-100, 225-295),
spline.rs (67-97, 121-230, 531-560), stencil.rs (greps), images.rs (greps: prepare,
level_images, LevelImages)}, cli/topup.rs (855-935), cli/topup_debug.rs (88-132, fn list),
crates/larmorx-core/src/defects.rs (530-538); python/larmorx/fsl/topup.py (10-16, 394-400,
greps), python/larmorx/_replica.py (14-30); validation: parity/fsl_topup.py (mine: 176-213,
762-800, 880-935, 968-1010, 1147-1200), parity/fsl_topup_replica.py (119-122, 306-340: Run),
parity/fsl_fast.py (532-544), parity/harness.py (greps: Case), difftest/permanent.py (whole:
the harness's registry; one line added), difftest/tools/fsl_topup.py (515-560),
difftest/tools/fsl_topup_original.py (mine, whole); docs/validation/difftest.md (greps and
222-245); docs/api/fsl-topup.md (whole); docs/findings/fsl-topup-original.md (mine:
300-379); docs/validation/fsl-fast.md (head: the record's format);
validation/investigations/cleanroom-topup/ (mine: recheck.py, fsl_case.py, README, listing);
scripts/orchestration/{boxed.sh, lxv.sh, gates-python.sh, parity_smoke.py} (heads). Data: the
trial's results and findings (oracles/difftest/2026-11-03-topup-original), FSL's and the
original's outputs of probes in <scratch>/r5b.
**One slip:** a `grep -rn "FSL-145\|FSL_145"` over the worktree (excluding target/ and
crates-nc/): it printed only lines of my own files (docs/findings/fsl-topup-original.md,
parity/fsl_topup.py, crates/larmorx-fsl/src/topup/{apply,spline,mod}.rs, cli/topup.rs,
larmorx-core defects.rs); no restricted file matched or was printed.
Never opened: FSL's source, crates-nc/ (sources; its target built by gates-rust.sh and the
replica program `<scratch>/nc-target/release/larmorx-nc` run only as a black box by the smoke
tier), replica crates, restricted findings, † sections, docs/parts/, replica
investigations/probes/triage.toml/permanent cases, replica git history, the oracle cache's own
files, the main checkout, parity/fsl_flirt_replica.py.
Ran FSL (only through the oracle cache, use mode): the probes of first_step.py (3693's volume
subsets, oblique acquisitions, --debug=3, --lambda=0; the 4 nm-* and 4 r5-* cases; 10753's
knot tests): new runs stored (a few dozen, seconds each); thread_band.py (4 FSL --nthr=4
runs, stored after the cache's repeats); recheck.py three times (513 replays each, FSL served,
band runs stored); the permanent cases' suite runs; the record (161 served, 21 run: 19 aborting
runs never stored, 2 --profile). The original: probe builds (temporary code, restored each
time: CG tolerance, gradient dumps, FSL-146 hypothesis, SCG probes, knot conventions) and
target/release/larmorx, boxed 4-16 GiB, one box at a time besides the smoke tier. Stopped
myself: one hung probe run (eprintln deadlock in a temporary build) via TaskStop.

## Clean-room statement (relay 6)
Opened: BRIEF.md, NOTES.md; docs/figures.md (whole); larmorx's own code in the worktree:
validation/src/larmorx_validation/evidence/{fsl_topup.py (whole: mine), fsl_fast.py (def list,
340-670, 860-1300, 1600-1909), common.py (120-535), __init__.py (40-140), sizes.py (20-48),
core.py (agreement), credits.py (attach), difftest.py (60-126)}, figures/cache.py (1-300),
difftest/permanent.py (def list), tables.py (def list, 60-80, 170-215), parity/fsl_topup.py (mine:
1-215, 366-392, 469-1245), parity/fsl_topup_replica.py (def list, 85-200, the Run class,
395-450, 1955-2031), validation/tools.toml (1-30, 160-200, 405-440); docs/validation/fsl-topup.md
and .json (mine), docs/findings/fsl-topup-original.md (mine: headings, 329-470),
docs/api/fsl-topup.md (mine), docs/benchmarks/{fsl-topup.md/.json (mine), README.md, the heads
(12 lines) of fsl-bet.md, fsl-flirt.md, fsl-fast.md}, the register by its range only (lines 41
to 390: the header, the `| FSL-13x/14x` rows, FSL-103, FSL-104, FSL-111), docs/api/
upstream-defects.md (FSL-111, 116-160, the note), docs/next-steps.md (38-48, 460-520, 590-606,
a grep of "topup"), docs/validation/golden.md (71-186 parts), docs/validation/difftest.md
(125-128, 230-330), docs/status.md (a grep of "topup"), tests/golden/registry.py (topup's cases),
tests/golden/golden.json (keys), tests/parity/test_memory_peak.py (TOPUP_CASES),
scripts/orchestration/{apply_handoff.py (docstring, 40-130), README (grep), env.sh, lxv.sh,
boxed.sh}; validation/investigations/cleanroom-topup/ (mine), the listings and README heads of
cleanroom-fast and cleanroom-flirt; my trial's data (summary, findings, the permanent cases'
`found`). The site clone: README, CLAUDE.md, content/tools/fsl-topup.md and fsl-fast.md,
site.yml, content/roadmap.md, data/status.yml and verdicts.yml (parts), tools/{build.py args,
evidence.py speed, shoot.mjs head, blobs.py docstring}, static/css/evidence.css (grep).
**Outside the reading list (not restricted):** a filtered listing of
validation/investigations/cleanroom-bet/ (`ls | grep site|inventory|shot`: printed only
`site_inventory.py`) and runs of that script (its docstring printed by a `--help` run; the
inventory tool FLIRT's clean-room site relay used); the heads of the BET and FLIRT benchmark
reports (for the README op the orchestrator asked for); docs/status.md (a grep).
Never opened: FSL's source, crates-nc/, replica crates, restricted findings, † sections,
docs/parts/, replica investigations/probes/triage.toml/permanent cases, replica git history, the
oracle cache's own files, the main checkout, parity/fsl_flirt_replica.py (imported only).
Ran FSL only through the oracle cache (use mode): `cases-original` re-ran each passing case of
the record as the suite runs it (FSL's runs served; the --profile case's never-cached run fresh;
the multi-threaded runs as the cache's policy serves them); no new probes. The original:
the wheel (python -m larmorx), boxed 16 GiB, one box at a time. The replica was not run (its
outputs are the replica entry's). Headless Chrome on the local build and on the live site.
Nothing left running.

## FROM THE ORCHESTRATOR (keep this section in every rewrite)
- (2026-11-03) **For sub-task 6 (relay 6), from relay 5b's report.**
  - **Searching:** never search the worktree, `docs/` or `validation/` recursively (relay 5b's `grep -rn` for
    "FSL-145" over the worktree, recorded): `docs/findings/` and `docs/parts/` hold restricted files. Search only
    your own files, named by path, and the register only by its line range.
  - **The site clone** comes from the site's `main` as it is now (171a8ae, topup's replica page in place), and its
    `evidence-blobs` from 72d1f2d (851 blobs). New blobs only add to it; nothing the page shows today is removed or
    reduced. The inventory must stay at or above before on every page.
  - **The two open categories** (47 cases of the quadratic regulariser's Hessian at an even knot spacing, reproducer
    10753; 8 cases of applytopup `lsr` on lines whose length divides 2^32, reproducer 8089) go into the spec
    questions with their reproducers and numbers. On the page they appear as open, with their counts, as the
    record and the triage name them.
  - **FSL-146** goes on the page beside FSL-117's example if a fMRIPrep-like call reaches it; otherwise as a line of
    the trial's table. Its replica switch is one of the next-steps follow-ups (with FSL-144 and FSL-145).
  - **The final report** lists, beside the brief's items, every clean-room slip of the part (relays 1, 1b, 2, 5b),
    each with what it showed, so the merge notes can record them.
- (2026-11-03) **The trial's 6,999 differing cases: triage, not tolerance.** Every difference goes into one
  category of the rule in `docs/validation/difftest.md` (the triage entries through the handoff's `append` op; never
  open `triage.toml`), with its count and one reproducer each.
  - **A larmorx mistake:** fixed, with a permanent case.
  - **A defect of FSL:** registered by the rules A-D, as FSL-145 was.
  - **Explained:** a named cause with its evidence, in these classes:
    - the movement derivatives of finding 14 (waiting for the spec writer's answer);
    - FSL-133 and FSL-143;
    - a different minimum of an ill-determined fit, shown by the cost ratio and by FSL's own stability;
    - another cause, if one is found.
  - **Open:** report it.
- **Probe the first Levenberg-Marquardt step (the CG tolerance) first, as planned.** If the original's step
  differs from FSL's by a convention the spec leaves open, follow FSL's where it is measurable: one convention,
  not a per-case allowance.
- **Thresholds stay §12's**, or the measured band where wider. The trial's numbers in the record and the report are
  counts per category, not a pass rate on a loosened metric.
- (2026-11-03) **Sub-task 5's heavy runs.** The parity smoke tier runs with `--workers 14`
  (`scripts/orchestration/parity_smoke.py --workers 14`), because the replica agent beside you may run its own.
  - Your boxes together stay at most 32 GiB.
  - The random-testing trial uses `LARMORX_DIFFTEST_CACHE_MODE=use`, at most 30 minutes, boxed.
  - From the final clean commit, once each: the record `fsl-topup`, the smoke tier, `record_golden.py` (reusing its
    marker).
- **For the handoff:** the replica's applytopup is 8 times slower than FSL (your benchmark), and
  `docs/benchmarks/README.md` lacks the fsl-* lines. Both go in as next-steps follow-ups, the README lines as an op.
- (2026-11-03) **applytopup `--method=jac` with a z phase encoding**, which leaves the image uncorrected
  silently (relay 3a). Decided by the rules A-D, as **FSL-144**:
  - If topup accepts z phase encodings and estimates a field for them (the spec's `--datain` rules; check with one
    black-box run if the spec is silent): **rule A**. jac corrects along z by default, as along x and y, and names
    the difference; `reproduce` gives FSL's uncorrected output.
  - If topup refuses them: **rule B**. jac refuses with a clear message by default; `reproduce` gives FSL's output.
  - Add FSL-144's `larmorx_core::defects` entry, one suite case, the register row (behaviour only) and the API
    page's line through the handoff.
  - The replica must get the same switch (D13: replicas share the corrected default). Write a next-steps follow-up
    in the handoff for the replica side, as FSL-111 was.
- **FSL-133 and FSL-134's "laid as stored"** (not reproduced): keep them as spec questions, with your probes'
  evidence, for the spec writer's answers part.
- (2026-11-03) **The rotation derivatives (finding 14)** go to the spec questions with their evidence (the
  probes, the 0.4 to 26 % numbers, the 10 cases). Whether FSL's behaviour is a defect or a deliberate approximation
  can only be settled from the source: the orchestrator will arrange a spec writer's answer, so spend no more steps
  on it. Keep the 10 cases as expected divergences, each with its reason in the record.
- **The main checkout is out of bounds**, a listing included (relay 2 listed its `crates-nc/target`). Use only this
  worktree's own builds (`<scratch>/nc-target` for the replica as a black box).
- (2026-11-03) The oracle cache's own files (`findings.jsonl` and its entries) are not on the reading
  list: read the cache only through `oracle_cache.run` and the suite's outputs. Relay 1b tailed `findings.jsonl`
  once (recorded).
- The refit's wrap-around penalty (relay 1b's finding) is a spec question: keep it, with its probes F, G and H, in
  the handoff's spec questions.
- (2026-11-03) **Windows paths.** `docs/api/fsl-topup.md` now says how a configuration's `.cnf` and
  applytopup's `--topup` base name are found on Windows: a backslash separates directories as `/` does, and Linux and
  macOS are unchanged. The original behaves the same. Its tests must pass under a temporary folder whose name holds
  a dot (Windows: `...\.tmpXXXX\...`).
- (2026-11-03) **ds005040 outside the thresholds.**
  - First look for the cause: the refit between levels, §4.4. Use black-box probes through the oracle cache.
  - The allowance is as §12 says. The band of a last-bit change (FSL at `--nthr=2` and `4`; the replica with
    correctly rounded `exp`/`log`) is the original's allowance only where it is wider than §12's numbers. Report
    it in the record beside the fixed thresholds; never loosen a threshold without that measured band.
    - FSL's thread band is already measured by the replica part, so do not re-measure it. Its page's evidence and
      `docs/validation/fsl-topup-replica.md` hold the numbers; the oracle cache serves those FSL runs.
    - Compare in the same brain mask §12 defines.
  - If the refit stays unknown after 1b's probes, write the exact spec question, the observations, and what the
    spec would need to say into NOTES.md, and go on. The orchestrator may arrange an answer from the spec writer
    (as with FLIRT's spec answers).
- **Stay within the brief's reading list.** Relay 1 opened `parity/fsl_flirt_replica.py`, which the list does not
  name; do not open it again. For the suite's mechanics, use `parity/fsl_topup_replica.py`, `parity/fsl_fast.py`,
  `parity/harness.py` and `oracle_cache/`. A file outside the list needs the orchestrator's word first; ask in your
  report.
