# NOTES: spec answers for topup, FAST and FLIRT

WORKTREE: <workspace>/larmorx/.claude/worktrees/agent-aaf7b58fc18c485aa
(branch worktree-agent-aaf7b58fc18c485aa, from main d0c6e83; `cd` there first, every command there)

## FROM THE ORCHESTRATOR (keep this section in every rewrite)
- (2026-11-03, after relay 4) **Sub-task 5 also updates the site where the replicas' defaults changed**, so it never
  shows a default the code no longer gives: FSL-202 (FAST's single channel, fMRIPrep's three calls included) and
  FSL-201 (topup's estimated movements). In a clone of `<workspace>/larmorx_documentation` in `<scratch>/site` (from the
  site's `main` and `evidence-blobs` as they are when you start; branch `spec-answers-3-pages`; the no-reply identity
  first): find which panels of `/tools/fsl-fast/` and `/tools/fsl-topup/` show a replica's default output; re-export
  those tools' evidence from the final clean commit (`evidence/fsl_fast.py`, `evidence/fsl_topup.py`; the figures cache
  recomputes only what changed); add the explanation (what moved, why, relay 2b's numbers for FSL-202, relay 1a's for
  FSL-201, with the verdict the site's taxonomy gives). The site's checks: build (`tools/build.py`: 0 problems), number
  and viewer checks, inventory after >= before on every page, screenshots 1440 and 390, light and dark, before and
  after. If the pages show no default output of the replicas, say so and skip the re-export.
- The handoff names the clean-room follow-ups (FAST's original to §4.4 by default; topup's FSL-145 `reproduce` count
  and the FSL-146 trigger check; FLIRT's original's cases the answers close; `lx.mri.hmc`'s centre sums), the
  glibc-function question as a next-steps item for the orchestrator, and FSL-111's notice in the default only.
- (2026-11-03, after relay 3b) **Sub-task 4 starts by merging main** (1eac18c or later: the replicas' FSL-160 NaN bits
  on ARM and a Windows test, in `larmorx-nc-core`'s `filters.rs` and `volume.rs`; an orchestration script), so the
  records come from a tree that holds them. Then every switch the notes list: FSL-111 and FSL-204 in FLIRT's replica;
  FSL-144, FSL-145 (with relay 1b's 1-coefficient count and the single-coefficient refit's abort), FSL-146 and the
  FSL-201 texts in topup's; FSL-202 and FSL-203 in FAST's; the corrected defects texts (FSL-125, FSL-134, FSL-145).
  Records `fsl-flirt-replica`, `fsl-topup-replica`, `fsl-fast-replica` from the final clean commit, once each, side by
  side within 32 GiB; `compare_records` against main's: only the fix passes these switches change, and the new cases,
  each explained. Then the smoke tier once and `larmorx-nc`'s golden record if a case was added. Split 4a/4b if needed.
- **glibc's single-precision functions for the clean-room originals' `reproduce`** (relay 3b: `sinf`, `cosf`,
  `atan2f`) is the orchestrator's question for the user (moving code from a replica crate needs the user). In the
  handoff, write it as a next-steps follow-up only: which clean-room cases it would close.
- (2026-11-03, after relay 3) **`specs/mcflirt.md` §6.1 is yours too** (as spec writer, marked like the others):
  question 25's answer (mcflirt's centres of mass use the same blocked sums as FLIRT's) goes into it in relay 3b, with
  a model check on one mcflirt call if cheap; the handoff then names the follow-up for `lx.mri.hmc`'s clean-room
  original. FSL-204 as relay 3 decided (rule B: refused in both modes, after FSL's own lines).
- (2026-11-03, after relay 2b) **FSL-202 decided as rule A** on relay 2b's numbers (the corrected variance equal or
  within one voxel of the double-precision reference on 7 of 7 cases, its own band 0 where FSL's reaches hundreds of
  voxels). **Sub-task 4 therefore also covers FAST's replica**: FSL-202 (the corrected variance by default, named,
  `reproduce` FSL's) and FSL-203 (named, `reproduce` FSL's exit 255 and message), one parity case each, and the record
  `fsl-fast-replica` re-made from the final clean commit with the others (`fsl-flirt-replica`, `fsl-topup-replica`):
  `compare_records` may move only the fix passes these switches change, each explained. The handoff's next-steps item:
  FAST's clean-room original follows the new §4.4 by default in a later clean-room part.
- (2026-11-03, after relay 2) **FSL-202 is not decided yet: relay 2b measures whether the correction helps (rule D),
  before sub-task 3.** Relay 2 measured how far the exact variance moves the outputs (up to 16 times FSL's own last-bit
  band, fMRIPrep's ds000005 call included: `_seg` 54 voxels, `_mixeltype` 112). That shows the change is large, not that
  it helps, and D11 keeps larger changes opt-in unless they are corrections of a defect that make the result right.
  Measure, with the dev patch, on the same 7 cases (fMRIPrep's three FAST calls among them):
  1. **A reference for the intended maths:** FAST with the class statistics (mean and variance, in the log domain)
     computed in double throughout, everything else as FSL. Distance to it (the same metrics: `_seg` and `_mixeltype`
     voxels, `_pve` differences): FSL's against the corrected variant's.
  2. **Stability:** the corrected variant's own last-bit band (the same perturbation that gave FSL's band) against
     FSL's band.
  Then decide: **rule A** if the corrected variant is closer to the reference than FSL on most cases (fMRIPrep's calls
  included) and its band is no wider than FSL's: the default corrected, named, `reproduce` FSL's, and the register row
  names the fMRIPrep impact. **Rule C** otherwise: FSL's computation stays the default in both implementations, the
  register row documents the imprecision with the numbers, and no switch is needed. Report the numbers either way.
  Keep the measurement's scripts under `validation/investigations/spec-answers-3/` (the FSL-licensed patch stays in
  `crates-nc/`). Bounded: one relay.
  -> DONE in relay 2b (commit 20c0165): **rule A confirmed** (closer on 7 of 7, fMRIPrep's three calls included;
  band no wider on 7 of 7, in fact 0 label voxels). Numbers below ("relay 2b DONE") and in the README.
- FSL-203 (a NaN first voxel: FSL exits 255) as relay 2 decided (rule A); sub-task 4 completes the replica (named, and
  `reproduce` gives FSL's exit and message).
- (2026-11-03, after relay 1a) **FSL-201 is decided as relay 1a measured it** (rule D, then A): the exact rotation
  derivative by default in the replica (as the clean-room original already computes it), FSL's forward difference
  with `reproduce`; the four early-stopping runs that end 0.004 to 0.13 % higher are named in the register row's
  evidence. The refit's penalty and lsr's folding stay FSL's (rule C), as relay 1a decided.
- **Reproducer 8089 (lsr on 16-voxel lines): run it once in 1b** against the new spec text (the model, and the
  replica in `reproduce` as a black box through the cache), so that answer is not left at medium confidence.
  -> DONE in relay 1b (lsr_8089.py: model 3.2e-5 at p99 vs FSL; overlap reading 0.131 = the original's 0.1337;
  replica reproduce == FSL bit for bit). §13 row 4 now high confidence.
- **Sub-task 4 re-makes `fsl-topup-replica`** from its final clean commit: relay 1a's replica change (FSL-201's default)
  moves only the fix passes of cases with estimated movements; `compare_records` must show exactly those and nothing
  else, each explained.

## Summary for the next relays (relay 1 read these; re-read only what your sub-task needs)

### What a spec may contain (porting guide §1)
- Spec writer writes `specs/<tool>.md`: behaviour, maths, numeric conventions, command line, validation
  plan. NEVER code, routine/file/variable names of FSL's code, line references. Mark black-box-confirmed
  statements **(observed)**. Every new or changed line marked "(spec writer, 2026-11-0x)".
- FSL's printed messages may be quoted (they are output, e.g. `Topup::TopupScan::...` texts already in spec).
- Restricted (clean-room must not read): docs/findings/fsl-*.md without `-original`, docs/parts/, crates-nc/,
  register sections marked restricted. Our own restricted findings files: docs/findings/fsl-topup.md,
  fsl-fast.md, fsl-flirt.md (source-derived details, FSL routine names allowed there).

### The model part: docs/parts/2026-11-02-flirt-spec-answers/ (brief, report, handoff, handoff.toml)
- Answers go into each spec section + a table in the questions section ("The clean-room questions of
  2026-11-0x": question | section | observation right/incomplete/wrong). FLIRT's is §14 (rows 1-25 from
  2026-11-02 already there; add the 2026-11-0x table for part 2's 38).
- Models in validation/investigations/<topic>/ (README table: file | what it shows | how to run; "What the
  models gave" paragraph with every number). Ours: validation/investigations/spec-answers-3/.
  Model scripts there: fsl_runs.py (FSL runs through oracle_cache, `<runs dir> [name ...]`), *_model.py
  (written from the spec text only, reproducing FSL's numbers; `--variant` options show other readings part
  from FSL), replica_runs.py (replica both D13 modes).
- Defects decided: register row + detailed section via handoff, replica `reproduce` switch,
  larmorx_core::defects text (behaviour only), spec §10/§11 entry, one replica parity case per new path,
  golden case if new CLI path. Then replica standard tier once + compare_records + smoke tier once +
  larmorx-nc golden if case added.
- HANDOFF.toml format: scripts/orchestration/apply_handoff.py docstring; `--check` in worktree.

### topup (sub-task 1): sources of the questions
- docs/parts/2026-11-03-fsl-topup-cleanroom/handoff.md "Spec questions" (7 items + a long conventions list);
  report.md; docs/findings/fsl-topup-original.md (findings 1-36, numbered); docs/validation/fsl-topup.md
  (22 expected divergences: 9 finding 14 movements [opt/levels, opt/scg-movements, opt/four-volumes,
  opt/zero-readout, cfg/b02b0, b02b0_2, b02b0_4, b02b0_7T, difftest/fsl145-short-axis], 4 FSL-133 [at/lsr-long,
  vb2d/3d/4d-long], 3 FSL-143, vb2d-aniso transients, opt/lambda-negative FSL-126, 4 refused (warpres-neg
  FSL-124, vb-other-grid, jac-other-shape, lsr-other-grid FSL-134));
  validation/investigations/cleanroom-topup/ (README; probes.py A-H; wrap_probe.py, at_wrap_*.py (probe133);
  at_jac_field_scale.py, at_other_shape*.py (probe134); triage_counts.py, trial_triage.json; mov_*.py).
- Spec sections: §4.2 coefficients layout, §4.3 model/mask, §4.4 grids + refit, §4.5 interp, §4.6 cost +
  gradient, §4.7 minimisers, §6 outputs, §7.2-7.6 applytopup, §9 numerics, §10 defects, §12 validation.
  The spec has no questions section yet: add one ("§13 The clean-room questions of 2026-11-03").
- Restricted replica knowledge: docs/findings/fsl-topup.md (e.g. "inv(S) * inv(R) * S (the movement
  derivative) is Armadillo's redirect3: solve(S, inv(R)*S)"; FSL-133 = CircularArray i % _n with _n unsigned).

### FAST (sub-task 2)
- docs/parts/2026-11-03-cleanroom-fast/handoff.md lines 90-121 ("Spec questions", relays 1-5):
  R1 §4.4 variance second-moment quotient in double; §4.10 MRFWeightsTotal after each sweep; §4.8 ICM keeps type,
  initial 0; FSL-100 with -A; _restore 0 at NaN voxels; REGISTER CANDIDATE (rule D, measure): single-precision
  variance in the log domain. R2 §4.5 bias kernel taps double; §5.2 initial assignment products double summed
  single; §5.6 bias weights q/C(i,i) single, several channels no tissue volumes with --nopve; §3.1 (and BET §3.1)
  ANALYZE no scale field, negative voxel sizes radiological; §6.4 FLIRT -applyxfm pixdim[4] (FLIRT spec); §8.3
  how often "ERROR:: Empty mask image" printed; §9.1 FSL-99 -H nan lines, beta with several channels.
  R3 §10 exact parallel order of Gauss-Seidel sweeps (pipeline of planes: row (y,z) after plane z-1's row y+1);
  FLIRT §9.3 TR as FSL reads (|pixdim[4]|, 1 for TR 0), -verbose 2 "Resampled volume" TR for 3D.
  R4 §4.5 field outside mask (0-mean)*0 (NaN when mask mean NaN); §5.2 k-means line after inverses; §9.1
  FSL-101; §5.5/§5.3 how stats become NaN after NaN covariance (case 1840).
  R5 §8.6/§5.8 several channels: PV fraction search near ties (one 0.01 step expected?), §11 thresholds narrower
  than FSL's own band on small classes/images (ds005003 _mixeltype Dice 0.998).
- Evidence: docs/findings/fsl-fast-original.md, docs/validation/fsl-fast.md; restricted docs/findings/fsl-fast.md.
  FSL IDs for FAST: no range given in the brief for new FAST defects (topup's range FSL-201..210 is the part's).

### FLIRT part 2 (sub-task 3)
- docs/parts/2026-11-02-cleanroom-flirt-2/handoff.md lines 113-195: 38 numbered questions (3 superseded by 27,
  15 resolved by orchestrator, 29 answered by 30). Topics: makerot (1), avscale (2), img2imgcoord (4), stdin (5),
  number reading (6), -warp non-warp (7), matrix files (8), midtrans (9), pointflirt (10), applyxfm4D (11, 30),
  extracttxt (12), aff2rigid (13), pairreg schedules (14), -verbose lines (16, 17, 19, 28), -debug (18, 38),
  search (20, 24, 33), edge factors order (21), sinc last bits (22), leastsq sums (23), mcflirt CoM blocks (25),
  forms written (26, 27, 31, 32, 34), nearest outside (35), dgesv LU (36), singular inverse (37).
- Evidence: docs/findings/fsl-flirt-original.md, docs/validation/fsl-flirt.md; restricted docs/findings/fsl-flirt.md.
  Split 3a (1-20) / 3b (21-38) if needed.

### Sub-task 4 (replica switches) and 5 (handoff): see BRIEF.md. IDs FSL-201..210 (FSL-201 used).

## How to run things here (the sandbox refuses `source`, shell variables in paths and $() in commands)
- `<scratch>/lx <GiB> <command...>`: sets the environment (FSL off PATH, PYTHONPATH=worktree python +
  validation/src, OPENBLAS_CORETYPE=Haswell, LARMORX_ORACLE_CACHE_MODE=use) and runs the command inside
  scripts/orchestration/boxed.sh. Example: `cd <worktree>/validation/investigations/spec-answers-3 &&
  <scratch>/lx 8 python -P rotation_model.py <scratch>/runs/debug-movements`.
- Loops / several commands: write a script in <scratch> (see m201_more.sh, probe133.sh, foldprobes.sh,
  lsrcmp.sh) and run it by its absolute path. cargo: `cd <worktree>/crates-nc && PATH=$HOME/.cargo/bin:/usr/bin:/bin
  cargo build --release -j 14 --locked -p larmorx-nc-cli` (crates-nc/target seeded from the main checkout).
- FSL runs so far (all through the cache; stored): <scratch>/runs/debug-movements (case debug/movements),
  runs/fsl-probe-{D,E,F,G,H} (cleanroom-topup/probes.py with TOPUP_RUNS=<scratch>/runs), runs/probe133-{30,32},
  runs/probe-fold{1,3}; <scratch>/m201/ (measure_fsl201.py work dirs, summary-*.json).

## State
- relay 1 DONE (sub-task 1, part a). Commits 60667ad, acd1bbe, 842eb4d, d305c81 on branch
  worktree-agent-aaf7b58fc18c485aa. Checked: clippy -D warnings on larmorx-nc-topup clean; cargo fmt (both
  workspaces); ruff format/check and check_spdx on the new scripts clean. Nothing of relay 1 running.
  - specs/topup.md: §4.3 the movement matrix as FSL builds it (single-precision angle/cos/sin, axis entry q²;
    finding 13 confirmed; the --rbmout 5e-8 explained); §4.4 the refit (only the two wrap-around second
    differences, weight 0.005; refit on --warpres change even at the same spacing; intermediate spacing
    round-half-away(prev warpres / new voxel); level-1 and end refits; finding 11 right, finding 12 confirmed);
    §4.6 FSL's movement derivatives (forward difference of the sampling positions, steps 1e-4/1e-5, FSL's
    single-precision matrices) = FSL-201; §4.7 (the Hessian's movement blocks); §7.4 lsr's matrices row by row
    by FSL's walk, FSL-133's read w(k)=(k+2^32) mod n for k<0, folded lines kept (rule C); §10.1 FSL-201 row
    (+ FSL-133 row refined), §10.2 (refit penalty, folded lsr lines: rule C); header line names FSL-201;
    §13 "The clean-room questions of 2026-11-03" (rows 1, 2, 4, 5 answered; 3, 6, 7, 8, 9 "(to be answered)").
  - FSL-201 DECIDED rule A (measured: 22 runs with movements, lower or equal final cost in 18; 8/8 random
    cases with true movements lower 0.01-44 %; converged SCG -38 %; 4 higher by 0.004-0.13 % in runs cut short;
    fMRIPrep's two calls (sdcflows' config, estmov=0) unchanged). Numbers: validation/investigations/
    spec-answers-3/README.md.
  - REPLICA ALREADY CHANGED (for the measurement; sub-task 4 completes it): crates-nc/larmorx-nc-topup/src/scan.rs
    get_movement_derivative: default = exact derivative (exact_movement_derivative(), correctly rounded sin/cos),
    notice FSL-201 on every call (both modes); reproduce = FSL's forward difference. crates/larmorx-core/src/
    defects.rs: FSL_201 (behaviour text). NOT YET: parity case for FSL-201 (the record's movement cases trigger
    it; the suite's D13 fix pass will now see changed outputs on every case that estimates movements: check
    whether some case names FSL-201 with no visible change -> may need FSL-201 in the suite's SILENT list),
    larmorx-nc golden re-record (topup cases with movements change in the default), the register row via the
    handoff (check whether a test requires every defects.rs entry to be in the register before the merge).
  - docs/findings/fsl-topup.md: section "The spec answers of 2026-11-03" (routine names: GetMovementDerivative,
    make_rot's float theta, get_s_matrix, SubSample/SetWarpResolution, CircularArray/Find/GetK_Matrix).
  - validation/investigations/spec-answers-3/: README, rotation_model.py, refit_model.py, measure_fsl201.py,
    lsr_model.py, fold_probe.py.
- Register draft for FSL-201 (for sub-task 5's handoff; class c, rule A, tool topup, gap "-" once the replica
  is done; fMRIPrep impact: none (sdcflows' b02b0.cnf has --estmov=0); FSL's own b02b0.cnf estimates movements
  in levels 1-5, so FSL users' topup is affected): "with --estmov=1 the derivatives with respect to the
  rotations are forward differences (1e-5 rad) of rotation matrices built with a single-precision angle,
  cosine and sine and an axis entry slightly different from 1; the rounding divided by the step makes the
  gradient and the Gauss-Newton Hessian of the movements not the cost's (rotation entries 0.4 to 40 % off on
  real images): the minimisers end elsewhere than the cost's minimum". Measurements as above.

- relay 1b DONE (rest of sub-task 1). Commit 0221658 (on top of d305c81). ruff format/check + check_spdx clean on
  the new scripts. Nothing of relay 1b running. specs/topup.md §13: rows 1-9 ALL answered (no "(to be answered)").
  - New/changed spec text (all marked "(spec writer, 2026-11-03)"): header IDs FSL-141..146; §2.6 the acquisition
    reader's `TopupFileIO::` prefix; §2.7 `--estmov=100` when not given; §4.2 the layout rewritten (coef i at
    (i-1)k, not centred; cubic ceil((n+1)/k)+2; quadratic rule + closed form; FSL-145 = 2 when k<=n<2k and **1 when
    n<k** (new!); a refit of a single coefficient stops FSL "Mat::operator(): index out of bounds" while preparing);
    §4.3 displacement in the input's voxels; the mask rewritten (frame along the OTHER in-plane axis; oblique: no
    wrap, no frame but z; tol 1e-8); §4.4 negative FWHM = no smoothing (FSL-125); §4.5 periodic along every axis +
    the prefilter's truncated initial values (15 terms; |z|^N on axes < 15 voxels: finding 27 explained); §4.6 λ' =
    λ·SSD of the last cost evaluated (finding 34); E = cᵀRc with R translation-invariant, sampled at offsets in the
    open support, quadratic even-k inner knots the mean, outer knots not sampled (row 3); membrane per voxel; the
    2/(n(m-1)) factor; FSL-146 paragraph; §4.7 zero diagonal → solve fails, no cost, ×10 until 1e20 (finding 15);
    §5 debug Hess file format, debug images in FSL's order without forms, MaskedDiff grid, --trace -v; §6 last
    level without subsampling keeps its smoothing; §7.2 M⁻¹x; §7.3 jac precisely (mask, z=0, coefficient-field J,
    field-image J (mirrored prefilter start, zeros beyond, y term dx/fy: FSL-134), the other-shape layout (row 6));
    §7.4 8089 observed; §7.5 vb findings 23-25 confirmed; §9.4 prefilter note; §10.1 FSL-121 (empty line),
    FSL-125 (acts as none), FSL-134 (precise) rows corrected, rows FSL-144, FSL-145, FSL-146 ADDED (the spec lacked
    them); §13 rows 3, 4 (high now), 6, 7, 8, 9.
  - docs/findings/fsl-topup.md (restricted, mine): bullets added to "The spec answers of 2026-11-03" (bending
    energy helpers/kernel sampling, NCoef wrap + refit error, prefilter, FSL-134 other shape (no OOB write:
    zeropad extrapval), FSL-146 code, smaller confirmations).
  - validation/investigations/spec-answers-3/: bending_model.py, quad_count.py, short_axis_model.py,
    other_shape_model.py, lsr_8089.py + README rows and "What the models gave" numbers. Scratch helper
    `<scratch>/shortaxis.sh` runs the 7 short-axis probes (dims 8,8,4 1,8,1 4,4,4 2,6,3 8,8,8 12,14,10 16,16,16,
    seed 1). FSL runs new (stored in the cache): bend lambda-1e-4/1e-3 (runs/bend), quad_count's 4 (runs/qcount),
    7 short-axis jac runs (runs/short), 2 zero-field jac runs of 8089 (runs/r8089/moved0,1); from the cache:
    at/jac-other-shape (runs/at), 8089's topup source and applytopup.
- FOR SUB-TASK 4 (replica switches), new from relay 1b:
  - larmorx_core::defects texts (behaviour only) to correct: FSL_125 ("smooths with a kernel of that negative
    width" is WRONG: a negative --fwhm acts as NO smoothing, the kernel of non-positive width is the identity);
    FSL_145 (add: one coefficient when the axis is shorter than one spacing; a level that refits a single
    coefficient stops topup with an internal index error); FSL_134 (the other shape: laid as stored for the
    displacement, its Jacobian at the same indices, 0 beyond the field's grid).
  - FSL-145 in the replica: check the replica reproduces FSL's 1 coefficient for n<k and the refit error at
    --subsamp=2 (quad_count.py's 4 calls: run the replica with the same args; FSL's runs are in the cache), and
    the default's general count there.
- FOR SUB-TASK 5 (handoff), new from relay 1b:
  - register summary rows: FSL-125's behaviour text (acts as no smoothing; "results differ from the positive
    width's" stays true), FSL-145's (1 coefficient when n<k; refit error), FSL-134's (other shape: laid as stored
    + Jacobian at the same indices); the restricted section "### FSL-116 to FSL-135, FSL-141 to FSL-143" heading
    + text to cover FSL-144, FSL-145 (NCoef's int - unsigned wrap), FSL-146 (`if (_sm.HasBeta(i))` per scan in
    TopupCF::grad vs the manager's HasBeta() in hess), FSL-201 (GetMovementDerivative's forward difference of
    make_rot's float matrices): drafts in docs/findings/fsl-topup.md's spec-answers section.
  - next-steps / HANDOFF.md: which clean-room records/categories the topup answers let the original close: the
    47 quadratic-Hessian random cases (row 3) and case 7283 (short level, row 7); the 8 lsr cases (row 4: build K
    row by row by the walk); at/lsr-long and vb*-long (FSL-133, row 5); at/jac-other-shape (row 6: reproduce now
    reproducible, a new parity case possible for the original); finding 27's tiny-image vb probes and vb2d-aniso-
    like transients on small images (row 7); the 9 finding-14 cases + 4,462 random (FSL-201 decided: the
    original's default is already the exact derivative; reproduce needs §4.6's forward difference); the
    original's FSL-145 reproduce on n<k (1 coefficient) and its refit error.

- relay 2 DONE (sub-task 2, FAST's questions). Commits 75860a8 (tools), 74466ed (spec + findings + README)
  on top of 0221658. ruff format/check + check_spdx clean. Nothing of relay 2 running. The replica's source is
  unchanged (a measurement hook was applied, built, copied to <scratch>/fast/larmorx-nc-variance, reverted and the
  replica rebuilt clean: crates-nc/target/release/larmorx-nc has no hook).
  - specs/fast.md: header IDs + FSL-202/203; §3.1 ANALYZE (no scale field; mirrored only when the product of the
    stored voxel sizes is positive; 0 read as 1); §4.1.1 NaN -> 0 only through the rule's second branch (observed);
    §4.1.2 the extrema (first voxel seeds them, NaN never replaces; NaN first voxel -> empty mask, exit 255 = FSL-203);
    §4.4 second moment double + FSL-202 paragraph; §4.5 taps double, field outside (0-mean)*0; §4.8.6/7 ICM rules;
    §4.10 -nan printing, MRFWeightsTotal after the sweep; §5.2.2 products double summed single; §5.2.3 KMeans line
    after the inverses, not flushed; §5.3 a class without weight (the NaN mechanism, case 1840 observed, -N keeps
    MEAN 1); §5.6 weights single, R in double; no tissue volumes with --nopve; §5.8 near ties (one step; statistics'
    last bits decide); §6.1 FSLDIR only without -A; §6.4 the priors' TR; §7 "Standard output at an abort" (the list
    of flushing lines); §8.3 the Empty-mask count (4/class/statistics, 1/bias estimate, 1/class volumes; several
    channels none); §9.1 rows FSL-202, FSL-203 + FSL-99's -H nan lines exactly; §10 the exact parallel order
    (pipeline of planes, lag 2); §11 the band numbers + FSL-202's effect; NEW §12 the questions table (21 rows + the
    NaN-first row).
  - specs/flirt.md (FAST's FLIRT questions, rows 11 and 15 of FAST's §12): §4.1 ANALYZE + the TR as read; §9.3 the TR
    written (abs, 1 for a time axis with TR 0) and -verbose 2's "Resampled volume" lines. Sub-task 3 need not redo
    them; FLIRT's §14 table of 2026-11-03 may point to them.
  - docs/findings/fsl-fast.md (restricted, mine): "The spec answers of 2026-11-03" + "FSL-202 and FSL-203 (drafts for
    the register's restricted section)".
  - validation/investigations/spec-answers-3/: fast_probes.py, variance_measure.py, sweep_schedule_model.py,
    pv_ties_model.py + README "FAST" section with every number. The measurement hook (FSL-licensed) is at
    crates-nc/larmorx-nc-fast/dev-patches/variance_variant.patch (restricted location on purpose).
  - FSL runs new (stored in the cache): nan_noneg -B -b; nan_background -v; nan_first -v; t1_half -v -n 0;
    case 1840 with -N; FSL-CR band on ds000228 and on the verbose half case; FSL -v on the half case. From the cache:
    the other band runs, t1_nan, case 1840. Derived inputs: <workspace>/oracles/inputs/spec-answers-3/fast/.
  - DECIDED: **FSL-202** (one channel's variance: mu^2 = float square of the float mean, cancellation; rule D measured
    -> A): exact variance moves _mixeltype up to 16x FSL's band (ds000005 112 vs 7 voxels), 12x above 2^24 (346 vs
    29, pve p99.9 0.34 vs 0.01), within the band on 5 of 7. Default: mu = double quotient, mu*mu in double (variant
    "double"); named when a class's variance differs from FSL's (nearly every run); reproduce FSL's.
    **FSL-203** (a NaN first voxel: extrema NaN -> negative rule skipped; one channel's mask empty -> exit 255; rule A):
    default extrema over non-NaN values, mask as FSL-103's default; named when an input's first voxel is NaN.
    Both affect fMRIPrep? FSL-202 yes: fMRIPrep's call's default outputs change (ds000005 _mixeltype 112 voxels,
    _seg 54) -> say so to the orchestrator (register "fMRIPrep impact"); FSL-203 no (fMRIPrep's T1w brains have 0
    backgrounds).
- FOR SUB-TASK 4 (replica switches), new from relay 2:
  - FSL-202 in crates-nc/larmorx-nc-fast/src/mriseg.rs means_variances: default = the patch's "double" variant
    (mu = s_mp / s_p as f64; variance = (s_mpm / s_p2 - mu * mu) as f32), reproduce = as now; a notice FSL-202 when
    any class's variance differs from FSL's formula (compute both). Applies to every means_variances call (the
    initialisation, main loop, PV stage's intensity-domain statistics). NOT the multi-channel (no cancellation).
  - FSL-203 in crates-nc/larmorx-nc-fast/src/fast.rs clear_negative (both channel paths) and mriseg.rs create
    (the mask's max): default = extrema over non-NaN values (the mask is already "above 0 and finite" in fix mode),
    named when an input's first voxel (in FSL's radiological storage order) is NaN; reproduce = FSL's (empty mask ->
    exit 255). Check Segmentation::create's max_left_out logic with a NaN max (is_finite false: fine).
  - larmorx_core::defects: FSL_202, FSL_203 texts (behaviour only; see specs/fast.md §9.1 rows).
  - Parity cases in validation/src/larmorx_validation/parity/fsl_fast_replica.py: FSL-203's path (nan_first or
    nan_background input: the replica suite builds its own derived input, e.g. t1_nanfirst from t1_half); FSL-202
    is triggered by every single-channel case (the fix pass) -> it may need to be in the suite's SILENT list (named
    though some outputs can be identical, e.g. -n 1 or --nopve -N cases): check the fix-pass logic.
  - Golden: every single-channel FAST golden case's default output may change (FSL-202) -> re-record larmorx-nc's
    golden (nc golden registry) from the final clean commit; check whether golden cases run the default mode.
  - Records: fsl-fast-replica (standard tier) re-made from the final clean commit: compare_records must show only
    the fix passes changing (FSL-202 on single-channel cases, FSL-203's new case) and the new case.
- FOR SUB-TASK 5 (handoff), new from relay 2:
  - register rows FSL-202, FSL-203 (summary rows: behaviour only, from specs/fast.md §9.1; fMRIPrep impact: FSL-202
    changes the default outputs of fMRIPrep's call by about the measured numbers (reproduce unchanged), FSL-203 none)
    + the restricted section text (drafts in docs/findings/fsl-fast.md "FSL-202 and FSL-203").
  - specs/bet.md §3.1 (not mine): BET's "ANALYZE 7.5 inputs (observed): FSL works on them with x reversed" is
    incomplete: x reversed only when the product of the stored voxel sizes is positive; no scale field applied ->
    propose the text of specs/fast.md §3.1's paragraph for BET's spec (HANDOFF op on specs/bet.md, or the
    orchestrator's choice).
  - next-steps: FAST's clean-room follow-ups the answers open: (1) FSL-202 and FSL-203 in the original (default +
    reproduce); (2) §4.1.1: _restore NaN at NaN voxels without the rule's second branch (the original wrote 0
    always); (3) §7: an abort loses unflushed stdout lines (KMeans lines of earlier iterations; the PV heading with
    -n 0: err/n0-like cases with -v); (4) §5.3: case 1840 (the bias weights 0/NaN make the field NaN -> MEAN -nan);
    (5) §4.8.6/7, §4.5.3, §5.2.2, §5.6, §4.4, §4.5 already implemented by the original (confirmations); (6) §10's
    pipeline of planes for an exact parallel MRF/ICM sweep (speed); (7) §11: keep the band fallback (thresholds
    unchanged). FLIRT's clean-room: already reads the TR so (rows 11, 15); the ANALYZE reading too.
  - Which FAST record cases/random categories the answers close: case 1840 (open) -> closable; the 2 noise-free
    and rank-deficient cases stay (§8.6's kernels, not followed by design); multi/s5 stays (ill-conditioned);
    multi/s2-options' one-step partial volumes: expected (no change); singular-kmeans-line: the original should
    print no KMeans line at all before an abort (all lost), check its case.

- relay 2b DONE (FSL-202's measurement, the orchestrator's item). Commit 20c0165 on top of 74466ed. ruff format/check
  + check_spdx clean. Nothing of relay 2b running (box ended; the clean replica rebuilt: crates-nc/target/release/
  larmorx-nc has no hook, checked). The hooked binary: <scratch>/fast/larmorx-nc-reference; per-case JSON
  <scratch>/fast/ref/; logs <workspace>/oracles/logs/spec-answers-3-reference{.log,/}.
  - Method: validation/investigations/spec-answers-3/variance_reference.py; hooks crates-nc/larmorx-nc-fast/dev-patches/
    fsl202_reference.patch (superset of variance_variant.patch: LXNC_FAST_VARIANCE=double|twopass|reference,
    LXNC_CRMATH=1 = correctly rounded exp/log/expf/logf in the nc core's glibc_double/glibc_float). Sanity: replica
    with FSL's variance + LXNC_CRMATH == FSL under the CORE-MATH preload bit for bit on 7/7 (same perturbation).
    FSL and FSL-CR all served from the cache (no new FSL run).
  - Numbers, labels `_seg`+`_mixeltype` voxels: to the reference (double throughout), FSL / corrected; band FSL /
    corrected: ds000005 54+112 / 0+0, band 4+7 / 0; ds000228 66+89 / 0, band 122+113 / 0; ds005003 391+453 / 1+0,
    band 357+450 / 0; half -v 5+2 / 0, band 2+5 / 0; bias case 2+8 / 0, band 4+15 / 0; 2D 1+0 / 0, band 1+2 / 0;
    above 2^24 137+346 / 0, band 17+29 / 0. Partial volumes: FSL up to 1.0 from the reference, corrected within
    0.01 (0-92 voxels); corrected band 0.01 at 1-37 voxels (FSL's up to 1.0 at 14-7,126); the reference's band 0.
    -v stds FSL 226.856/109.413/38.6653, corrected = reference 226.863/109.422/38.6909.
  - **Finding: most of FSL's own last-bit band IS FSL-202's cancellation.** Written into specs/fast.md §4.4 (new
    paragraph), §9.1 row FSL-202 (behaviour: up to 844 label voxels from the exact statistics, fMRIPrep ds000005 166;
    default: measured 0/1 voxel, band 0), §11 (the band is `reproduce`'s allowance only; the default is compared with
    the replica's default, whose last-bit band is that narrow one), §12 row 6 (confidence: both scripts);
    docs/findings/fsl-fast.md (FSL-202 draft: the measurement, the hooks).
- FOR SUB-TASK 5 (handoff), new from relay 2b:
  - register row FSL-202: fMRIPrep impact = fMRIPrep's FAST call's default outputs change, toward the exactly
    computed statistics (ds000005 `_seg` 54 + `_mixeltype` 112 voxels, ds000228 66 + 89, ds005003 391 + 453;
    `reproduce` unchanged); evidence: variance_measure.py + variance_reference.py (closer 7/7, band 0 label voxels vs
    FSL's 3-807). The restricted section text: docs/findings/fsl-fast.md "FSL-202 and FSL-203".
  - next-steps, FAST's clean-room follow-up (1) FSL-202: its default record compares the default with the replica's
    default (FSL-202 corrected; narrow band), `reproduce` with FSL (FSL's band as now): specs/fast.md §11.
- FOR SUB-TASK 4, new from relay 2b: the replica suite's band/fix checks: with FSL-202's default the fix pass moves
  the single-channel outputs by about the "FSL to the reference" numbers above; the suite's band check (replica with
  correctly rounded maths) applies to `reproduce`.

- relay 3a DONE (questions 1-20) + 3b's first half (21, 25-28, 30-37). Commits 3d2478a, 5ea4b8a, d7dc8f2, 2412431,
  ad6610c, de83d14 on top of 20c0165. ruff format/check + check_spdx clean on flirt_models.py. Nothing running.
  - specs/flirt.md §14 table "The clean-room questions of 2026-11-03": rows 1, 2, 4-14, 16-21, 25-28, 30-37 answered;
    rows 22, 23, 24, 38 "(to be answered, relay 3b)". Answers in: §2.2 (the matrix-file reader exactly), §2.3 ("2 more
    during the free optimisation" was WRONG: same lowered level; "The levels exactly" list; -2D's missing sform lines =
    one-slice images lose their forms), §6.4 (edge factors order, negative weight -> 0), §6.5 (BBR without boundary
    voxels: the cost re-set before EVERY evaluation), §7.3 (short tolerance list), §8.1 step 9 (invalid quaternion
    start keeps only its SCALE, no translations: the spec's "translations and scale" was wrong; FSL-90 message at every
    invalid composition, evaluations included; P keeps the start composed once, the optimisation starts from its
    decomposition composed again), §8.2 (Erroneous dof counts; never in a subset), §9.2 (nearest neighbour outside),
    §9.3 (the forms written: completion of the reference copy's forms; -applyxfm's unset forms stay), §9.4 (-debug
    headers, setscale lines, BBR's five lines; post line "is 0" without boundary points; repeated at each evaluation),
    §11 FSL-204, §12 (vector solves = dgesv on the same LU; inv() fails only on an exact zero pivot: NaN matrices are
    inverted, FLIRT goes on), §15.1 (avscale exactly), §15.2 (scan, stdin, triples, -warp 3 volumes, V = S for every
    image without forms), §15.4 (makerot = §3's elementary rotation of the whole angle vector), §15.5 (midtrans: a
    non-finite determinant is NOT refused by FSL), §15.6 (pointflirt: nan reads 0; FSL-204), §15.7 (applyxfm4D: no
    blur, read errors, inverted first; signed char group FSL-111; FSL-79's x86-64 conversions), §15.8 (pairreg's
    schedules as behaviour; extracttxt; aff2rigid).
  - Models: validation/investigations/spec-answers-3/flirt_models.py (makerot 11/11 at 10 digits; avscale 16/16 probe
    blocks + 234/234 blocks of the 47 trial findings without a volume; --variant numpy-inverse parts in 11 of 250:
    only §12's inverse decides the check's signs of zero; readers 14/14 convert_xfm + 5/5 img2imgcoord; quat-messages
    8 = 4 starts + 4 evaluations at -dof 6, 4 = 4 + 0 at 12 dof), flirt_probes.txt, flirt_probes_trace.txt, README
    "FLIRT" section. Printouts (scratch): <scratch>/flirt/probes.out, trace.out, runs/dof6-trace (cached).
    Run: `cd <worktree> && <scratch>/lx 6 python -P validation/investigations/spec-answers-3/flirt_models.py
    <scratch>/flirt/probes.out --trace <scratch>/flirt/trace.out --findings <workspace>/oracles/difftest/
    2026-11-02-cleanroom-flirt-2/findings/fsl-flirt-original [--variant ...]`.
  - Restricted findings: docs/findings/fsl-flirt.md "The spec answers of 2026-11-03 (part 2's questions 1 to 20, from
    the source)" + the bullet for 21-37.
  - NEW DEFECT FSL-204 (rule B): pointflirt with no points (both coordinate files empty or missing) crashes FSL
    (signal 11) after its two "Could not open matrix file" lines. Spec: refused in both modes, FSL's lines first, then
    `pointflirt: no points in the coordinate files (FSL-204)`, exit 1. Next free ID: FSL-205.
- FOR SUB-TASK 4 (replica), new from relay 3: FSL-204 in crates-nc/larmorx-nc-flirt/src/pointflirt.rs procrustes():
  when n == 0 and y has 0 columns (both empty): default and reproduce both refuse (FSL's lines already printed by
  read_matrix) with the message, exit 1 (a crash is never reproduced); larmorx_core::defects FSL_204 text (behaviour:
  "pointflirt with two empty or missing coordinate files: FSL crashes (segmentation fault) after its messages; refused
  with a message, exit 1"); one replica parity case (fsl_flirt_replica: `pointflirt -i none1.txt -r none2.txt`: FSL's
  exit -11 vs the replica's refusal: the case must accept FSL's crash as "both-error"/expected, check how the suite
  treats a signal from FSL, e.g. FSL-75's pattern); golden case if the CLI path is new (an error path: probably not).
- FOR SUB-TASK 5 (handoff), new from relay 3:
  - register row FSL-204 (class a? crash; tool pointflirt; rule B; fMRIPrep impact none; larmorx: refused in both
    modes; gap -) + its restricted section text (docs/findings/fsl-flirt.md: two 0 x 0 matrices pass the count test,
    the later submatrix copies of the empty products crash).
  - specs/mcflirt.md §6.1 (question 25; not my file -> HANDOFF op): replace "sums over all voxels in double precision"
    with §8.1 step 3's rule of specs/flirt.md: `v − v_min` in single precision converted to double, double running
    sums over the voxels in z, y, x order flushed into the totals after every max(1000, trunc(√N)) + 1 voxels (N the
    voxel count); the denominator below 1e-5 in magnitude counts 1 with `WARNING::in calculating COG, total = 0.0` on
    standard error. Next-steps follow-up: `lx.mri.hmc`'s centre (clean-room crates/larmorx-mri) to follow it (measure:
    last bits of the centre, the .par/matrices' last digits).
  - next-steps: FLIRT clean-room follow-ups the answers open: (1) §2.3's free optimisations' level (if the original
    lowers by 4, fix); (2) §8.1 step 9: an invalid quaternion start keeps no translations (FSL-90 reproduce) and the
    message at every invalid composition (the original's "compose twice at dof 6" model to replace); (3) §15.1:
    avscale's check residual signs come from §12's inverse exactly (the 12th sampled finding); (4) §15.5: midtrans
    must not count a non-finite determinant as unreadable; (5) §15.6: FSL-204; nan in coordinate files read as 0;
    (6) §15.2: -warp: FSL's message and exit 1 for images of other than 3 volumes; (7) §12: in reproduce, inverses of
    NaN matrices go on (trial #2905, #704, #8361 closable); (8) §15.8: pairreg's schedules carried by the original
    (no FSLDIR; texts from FSL's -v echo); (9) §2.3/§9.4: -debug's BBR lines repeated at each evaluation without
    boundary points (#2424 likely), -2D's sform lines from one-slice images.

- relay 3b DONE (questions 22, 23, 24, 38; question 25's specs/mcflirt.md §6.1). Commits 5194f70 (spec + restricted
  findings + the replica's dev patch), 81d75d6 (models + README) on top of de83d14. ruff format/check + check_spdx clean.
  Nothing running; crates-nc/target/release/larmorx-nc rebuilt CLEAN (no LXNC_CRTRIG hook: strings checked). A standalone
  clean-room CLI was built for black-box runs: <worktree>/target/release/larmorx (git-ignored; reusable for sub-task 4's
  smoke tier? no: the smoke tier needs the wheel). specs/flirt.md §14 table: NO "(to be answered)" left.
  - Answers (all "(spec writer, 2026-11-03; question N of part 2 ...)"): §9.2 new paragraph "The table's sine is glibc's"
    (20 samples of the default table differ from correctly rounded; observed 32,052 = the clean-room count exactly);
    §15.7 spline = §9.3's exactly, double images read/filtered in double, value rounded to single; §6.2 "leastsq exactly"
    (three-level sums, (w·δ)·δ, (max−min)² fallback); §8.2 bullet "Equal and NaN costs" (strict comparisons; nothing moves;
    one sweep; the path decided by §3's compositions/decompositions: glibc's atan2f); §12 bullet "The single-precision
    functions that decide FLIRT's paths" (glibc sinf/cosf = Arm v19.11, atan2f = fdlibm; real/dof6's trace parting at the
    3,983rd evaluation = sinf/cosf, line 4,814 = atan2f); §9.4 two paragraphs (the levels' headers exactly: set forms only
    composed, every form of a neurological reference swapped back, unset qform's offset moves; global_refweight1
    pixdim[4..7] = 1; BBR without boundary points with -debug: NaN costs, nothing moves, -omat = the -init matrix, debug
    zeros, -out written); §14 rows 22, 23, 24, 38. specs/mcflirt.md §6.1: the blocked centre (both mcflirt centres).
  - Models: validation/investigations/spec-answers-3/flirt_models_3b.py (runs | original | check: sinc 840,862/840,862 with
    glibc sine, 808,810 with CR; applyxfm4D sinc 4 findings 11,806 voxels: glibc = FSL, CR = the original; spline 13,315/
    13,315 (original 8,138); leastsq 6/6 (one-level 0, double 3); ties 3,256/3,256 x 3 traces; debug 8/8 levels), README
    section "FLIRT (part 2's questions 22 to 25 and 38, relay 3b)", flirt_probes_3b.txt. Run dir (scratch): <scratch>/m3b
    (runs + original + check; log <workspace>/oracles/logs/spec-answers-3-models3b.log, MODELS-EXIT 0, every FSL run cached).
  - Dev hook (restricted location): crates-nc/larmorx-nc-flirt/dev-patches/crtrig_variant.patch (LXNC_CRTRIG=sin|atan|all);
    the hooked binary: <scratch>/bin/larmorx-nc-crtrig; its runs: <scratch>/q24/{empty-refweight,dof6}/replica-cr*.
  - Restricted findings: docs/findings/fsl-flirt.md "The spec answers of 2026-11-03 (part 2's questions 22 to 25 and 38,
    from the source)".
  - No new defect (FSL-205..210 still free). The k3 sign (a near-zero skew printed -0.000000 by FSL, 0.000000 by the
    original, line 66,125 of the FSL-91 trace) is the original's own composition/decomposition arithmetic (the hooked
    replica keeps FSL's sign with every variant): not found; for the clean-room part.
- FOR SUB-TASK 4: nothing new from relay 3b for the replicas (the FLIRT replica computes sinf/cosf/atan2f as glibc does).
- FOR SUB-TASK 5 (handoff), new from relay 3b:
  - next-steps, FLIRT clean-room follow-ups: (10) §9.2/§12: the original's `reproduce` (and the records' trace
    comparisons) need glibc's sinf/cosf (Arm's Optimized Routines v19.11, MIT) and atan2f (fdlibm's, Sun's permissive
    notice); the default keeps the correctly rounded functions (rule 5). How the clean-room crate gets them is the
    orchestrator's choice (the implementer must not read crates/larmorx-afni-replica, whose sincosf.rs is Arm's: e.g. move
    that module to larmorx-core, or port Arm's and fdlibm's own sources; the cbrtf/GLIBC-07 precedent); (11) §15.7:
    applyxfm4D's spline must follow §9.3's text (the original differs at 5,177 of 13,315 voxels of 5 trial findings, not
    explained by 20 readings tried: compare with flirt_models_3b.py's spline); (12) §6.2's leastsq sums
    (real/cost-leastsq); (13) §8.2: FSL-91/92's reproduce runs follow FSL's trace once atan2f is glibc's and the k3 sign
    (above) is found; (14) §9.4: the levels' unset forms of a neurological reference, global_refweight1's pixdim[4..7] = 1,
    BBR without boundary points (NaN costs move nothing: the original's NaN matrix and its stop at inv() are wrong);
    (15) lx.mri.hmc (clean-room crates/larmorx-mri): mcflirt's centres blocked as specs/mcflirt.md §6.1 now says (last bits
    of the centre; measure the .par/matrices' last digits).
  - Records/categories the answers let the FLIRT original close: real/interp-sinc (default: explained, the C library's
    last bit; reproduce: exact with glibc's sine); applyxfm4D float sinc findings (the same); real/cost-leastsq;
    real/weights-empty-refweight, -inweight, edge/one-voxel-input (with glibc's atan2f + the k3 arithmetic); the trace
    partings of real/dof6-like registrations (cost-normmi, weights-normcorr, weights-normmi, weights-woods: check with
    glibc's three functions first); the triage entries "From the same matrix, the windowed sinc and the spline..."
    (sinc part explained; spline part: the original's own departure) and "-debug's volumes on the trial's oblique and
    anisotropic images..." (114 findings: headers + pixdim + the BBR NaN path) -> handoff text for both triage entries.
  - specs/mcflirt.md §6.1 was edited directly as the orchestrator allowed (no HANDOFF op needed for it).

- relay 4 DONE (sub-task 4, whole: 4a switches + 4b records): merged main (7a3fc62, holds 1eac18c); commits b91a605 (switches, defects texts,
  parity cases, nc golden cases/notices, FSL-145 spacing-2 probe, API pages, restricted findings), e05cb7c (FSL-201 in
  topup's SILENT: opt/defaults, cfg/directory take no step), the fsl201_silent.py commit, 53796ce (two nc unit tests
  expect FSL-201/202 notices). New cases checked (newcases.sh): as designed. Topup check run (dryrun.sh, not a record):
  2 fails -> SILENT fix. Gates at 53796ce's content: root+gpl OK, python OK, tables OK, nc OK but its golden test
  (checksums to re-record: expected FSL-201/202/145 changes + 4 new cases). Records made by records.sh from clean 53796ce (log oracles/logs/spec-answers-3-records/).
  RECORDS DONE (clean 53796ce, <scratch>/rec/records; compare: status differs 0 in all three):
  - fsl-topup-replica 176 (106 pass, 67 both-error, 3 expected, 0 fail; was 173: 104/66/3): NEW opt/fsl145-short-axis
    (pass), opt/fsl145-one-coefficient (both-error: FSL's refit error reproduced, the default finishes),
    opt/fsl146-four-axes (pass); 37 cases' fix-pass counts 0 -> N, every one estimating movements (FSL-201: the
    option cases' default --estmov=1, debug/*, cfg/b02b0* configs; opt/four-volumes also FSL-146); at/jac-z's fix pass
    (8945, FSL-132) replaced by the FSL-144 refusal check (fix=False). applytopup's other cases unchanged.
  - fsl-fast-replica 87 (59 pass, 27 both-error, 1 expected; was 86: 59/26/1): NEW opt/nan-first (both-error, the
    default segments: FSL-203); 35 single-channel cases' fix-pass counts change (FSL-202), fMRIPrep's three calls
    included (2.06 M, 3.45 M, 4.65 M values); multi-channel cases unchanged.
  - fsl-flirt-replica 305 (211 pass, 87 both-error, 7 expected; was 302: 209/87/6): NEW programs/applyxfm4D-uint8,
    -uint8-3d-spline (pass, FSL-111 in the fix pass), programs/pointflirt-no-points (expected: FSL crashes, the
    replica refuses in both modes); no other case differs in any field.
  SMOKE TIER: passed once at 53796ce (2742 passed, 0 failed: 260 s on 14 workers + 1267 s serial memory/thread;
  marker <workspace>/oracles/cache/markers/parity-smoke-53796ce...json; log oracles/logs/spec-answers-3-smoke.log).
  Commits after it: 0971b20 (the three records into docs/validation/), c7ac893 (tables regenerated: README.md and
  docs/validation/README.md generated blocks only, from the records' counts; tables --check then OK), fabf9c2
  (larmorx-nc golden.tsv re-recorded from c7ac893: 4 new cases + the FSL-201/202/145 moves, nothing else); then the nc
  workspace's cargo test passes whole (33 ok), pytest tests/golden 216 passed. Tree clean at fabf9c2.
  Gates (at 53796ce's code): gates-rust root + gpl OK; nc fmt + clippy OK, tests OK after the golden re-record;
  gates-python --no-build OK; tables --check OK (after c7ac893). Main golden (tests/golden/golden.json) untouched
  (no main-package output changed: only defects texts).
  NOT DONE in sub-task 4: a parity case for applytopup reading a full-count coefficient file (needs a fix-mode topup
  output as input: covered by the nc golden case applytopup.jac-quadratic instead); FSL-111's notice in `reproduce`
  (the replica names FSL-111 only in the default, where it compares both readings).


- FOR SUB-TASK 5 (handoff), new from relay 4:
  - register rows (summary, behaviour only; the "larmorx" column now both implementations or the replica):
    FSL-111 "the replica too since 2026-11-03 (`programs/applyxfm4D-uint8`, `-uint8-3d-spline`)", gap "–" for the
    replica (clean-room already); FSL-144 "the replica refuses by default too (`at/jac-z`)", gap –; FSL-145 "the
    replica lays the general count too (`opt/fsl145-short-axis`, `opt/fsl145-one-coefficient`), applytopup reads
    either count by default; FSL's count precisely: 1 when n < k, 2 when k <= n < 2k only at k >= 3 (at k = 2 the
    general count; quad_count.py's spacing-2 probes)", gap –; FSL-146 "the replica too (`opt/fsl146-four-axes`)";
    FSL-125's text (acts as no smoothing); FSL-134's (other shape: laid as stored + Jacobian at the same indices);
    FSL-201, FSL-202, FSL-203, FSL-204 new rows (FSL-202/203: clean-room gap "original" = the clean-room follows in a
    later part; FSL-204 likewise for the original's pointflirt).
  - docs/api/upstream-defects.md: FSL-144/145/146 "(the original)" -> both implementations; FSL-201..204 entries.
  - golden.md: larmorx-nc's new golden cases (fast.fsl203, topup.fsl146, applytopup.jac-quadratic,
    applyxfm4D.uint8) and the re-recorded checksums (FSL-201 topup cases with movements + applytopup cascade,
    FSL-202 FAST single-channel, FSL-145 topup.float-membrane).
  - next-steps clean-room follow-ups from relay 4: topup's original `reproduce` of FSL-145 lays 2 where FSL lays 1
    (n < k) and 2 where FSL lays the general count (k = 2, n = 2 or 3): `Axis::short_quadratic` should be FSL's
    test `floor(n/k) k < floor(K/2)`; its reproduce also needs FSL's refit error for a single coefficient; its
    applytopup should read a coefficient file of either count by default. To check (not verified here): FSL-146's
    trigger in the original (`t * pe[a] != 0`) counts a volume with read-out time 0 as not displaced, while FSL's
    gradient leaves out only volumes whose encoding along the axis is 0 (a volume encoded along it with t = 0 keeps
    its term, beta 0): the original may name FSL-146, or reproduce it, where FSL's gradient is the cost's.
  - HANDOFF.md must say: c7ac893 regenerated the tables' generated blocks of README.md and docs/validation/README.md
    in the branch (python -m larmorx_validation tables, from the three records' counts), so tables --check passes;
    the orchestrator re-runs it at the merge.

- relay 5 DONE (sub-task 5, the last: the site, the handoff, the final report). Commits 3b65d4e (evidence: FAST's
  and topup's refresh_replica, the figures cache's merge_rerun/recompute, the FSL-144..146/201/202/203 reasons, topup's
  movements example opt/levels with its movement table), c50ba69 (topup's triage texts, the table vs the replica's
  default), 4c047cf (FLIRT's evidence names FSL-111), 5d896a0 (README: the re-export), e3ad16b (topup's output
  labels: --jacout's j_NN files, --featout's two). Tree clean at e3ad16b.
  Gates: gates-python --no-build exit 0 (608 passed, 12 skipped; ruff, SPDX); tables --check 0. No Rust changed.
  - SITE: clone <scratch>/site, branch spec-answers-3-pages = 4a70f2e (c030e59, 4a70f2e on site main a50a8ce);
    blobs branch spec-answers-3-blobs = d3ee6ba (994 blobs; = the clone's evidence-blobs). Exports from clean
    3b65d4e (FAST), 4c047cf (FLIRT), e3ad16b (topup); logs oracles/logs/spec-answers-3-export{,2,3,4}.log (all 0).
    refresh: FAST's 6 example entries' replica-fix rerun + named entry (87 cases) recomputed; topup's 7 (+ the new
    opt-levels and original-opt-levels entries computed; FSL from the oracle cache) + named (176). Build 0 problems
    (--main-repo the worktree); blobs verify 0 problems; number check FAST 1641/1641 (before 1316/1316), topup
    943/943 (before 748/748), FLIRT 319/319; viewer check FAST 520, topup 182, FLIRT 38, 0 failing; inventory
    after >= before on all 21 pages (site_inventory.py compare: 0 problems; <scratch>/inv/). Screenshots:
    <scratch>/shots/before, after (full pages 1440/390 light/dark; beyond ~16k px Chrome repeats the page: use
    the element shots), sel-before, sel-after (element shots: FAST's fMRIPrep example and texts, topup's membrane,
    movements, corrections, limits, random; FLIRT's corrections and limits). The before site: worktree
    <scratch>/site-before (a git worktree of the clone at main a50a8ce, old blobs) served on 8767, after on 8766;
    both servers stopped by relay 5.
    What moved: FAST fix rows d13 (FSL-202) on ds000005 54 seg + 112 mixeltype, ds000228 66+89, ds005003 392+453,
    bias 2+8, priors 250+678 (FSL-09 + FSL-202); multi-s3 unchanged. topup: membrane now names FSL-117 + FSL-201;
    new example movements (opt/levels): replica default vs original 3.7e-6 mm / 1.9e-7 rad, FSL 3.1e-4 mm / 8.4e-5
    rad away; fMRIPrep calls unchanged. FLIRT: replica record 305 (cases +3), FSL-111 named in its uint8 cases.
    Page texts: fsl-fast.md, fsl-topup.md (new section #movements), fsl-flirt.md.
  - HANDOFF: <scratch>/HANDOFF.toml (34 ops on 7 files: register rows FSL-111/125/134/144/145/146 corrected,
    FSL-201..204 new, the gap legend's "original" redefined, history paragraph, FSL-111's black-box section, the
    restricted section appended; api/upstream-defects.md; golden.md; next-steps (7 ops: FSL-111 and
    FSL-144..146 done, topup's/FAST's/FLIRT's questions answered with the clean-room follow-ups, FAST-1840, the
    glibc sinf/cosf/atan2f question, lx.mri.hmc's centres); specs/bet.md §3.1; triage.toml (7 entries);
    site:content/roadmap.md's completed row). `apply_handoff.py --check` (with --site <scratch>/site): all 34 ok.
    <scratch>/HANDOFF.md: the reasons, what each clean-room original can close.
  - Not done: FSL-111's notice in the replica's reproduce (by design: the default only); a parity case for
    applytopup reading a full-count coefficient file (covered by golden applytopup.jac-quadratic); the near-zero
    skew sign of FLIRT's FSL-91 trace (the original's own arithmetic, for its part); docs/figures.md does not yet
    describe refresh_replica (the investigation README does).

## PART COMPLETE (relay 5 was the last). The final report went to the orchestrator as relay 5's message.

## (done) sub-task 4's brief, kept for reference

- Switches: FSL-111 (FLIRT applyxfm4D uint8 as signed char: default as stored + notice, reproduce FSL's), FSL-144 (topup
  applytopup --method=jac with z PE: refused by default, rule B; reproduce FSL's), FSL-145 (topup --splineorder=2 short
  axis: general count by default; reproduce FSL's 2 coefficients, and 1 when n<k, plus the refit error), FSL-146 (topup
  volumes encoded along different axes: the cost's own gradient by default), FSL-201's remaining items (parity case,
  SILENT list check, register), FSL-202 and FSL-203 (FAST, the orchestrator's item above), FSL-204 (pointflirt without
  points: refused in both modes); larmorx_core::defects texts (behaviour only) incl. relay 1b's corrections (FSL_125,
  FSL_145, FSL_134); one replica parity case per new path; golden if a CLI path is new.
- Then from the final clean commit, once each: the replicas' standard tiers fsl-flirt-replica, fsl-topup-replica,
  fsl-fast-replica (compare_records: only the new cases and the fix passes the switches change, each explained), the
  parity smoke tier, larmorx-nc's golden record if a case was added; gates (gates-rust.sh all, gates-python.sh --no-build,
  tables --check). Split into 4a/4b if long (e.g. 4a code + cases, 4b records/smoke/golden).
- Then sub-task 5 (HANDOFF.toml/HANDOFF.md, the final report).
- Defect IDs used so far in this part: FSL-201 (topup), FSL-202, FSL-203 (FAST), FSL-204 (FLIRT); next free FSL-205 (to FSL-210).
