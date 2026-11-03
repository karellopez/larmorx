## topup's replica: final report (relays 1 to 11, 2026-11-02 to 2026-11-03)

The last relay's report as returned to the orchestrator (subagents cannot write report files), verbatim but
for paths.

The part is complete (sub-tasks 1 to 7). Worktree clean at 3b21e31; main not merged again (main is at
e9cc529); nothing of mine running (no boxes open, local servers stopped; only part 1's orphaned box
remains).

### What was built
- `larmorx-nc fsl topup` and `fsl applytopup` (crate `crates-nc/larmorx-nc-topup`, FSL 6.0.7.23, fsl-topup
  2203.5): every option, message, exit status and failure case of both programs (topup's levels, LM and
  SCG in double and float precision, bending and membrane energy, the movements, `--regrid`, `--scale`,
  `--splineorder 2|3`, linear and spline interpolation, FSL's configurations by name and its own,
  `--datain`/`--acqp`, `--debug`, `--trace`, `--profile`, `--nthr`, every output; applytopup's `jac`, `lsr`
  and the undocumented `vb2D`, `vb3D` and `vb4D`, `--interp`, `--datatype`, `--inindex`, lists of
  `--imain`).
- `lx.fsl.topup`, `lx.fsl.applytopup`, `larmorx fsl topup|applytopup` (replica only until the clean-room
  original); `specs/topup.md` (about 700 lines); `docs/api/fsl-topup.md`, `docs/findings/fsl-topup.md`
  (restricted), PROVENANCE.
- `larmorx-nc-core` gained FSL's libraries, each checked against FSL's own (probes in
  `validation/probes/`: millions of values, 0 differ): fsl-basisfield's spline fields with bending- and
  membrane-energy regularisers; SpMat, BFMatrix, CG/BiCG, the nonlin LM/SCG optimisers, SplineField::Set;
  the splinterpolator with derivatives, the interpolation partials, warpfns; OpenBLAS's blocked LU, ddot,
  dsymv; LAPACK's dsytrf/dsytrs, sytri, getri, trsv/trsm; the profiler; configuration lines of any
  length. One existing change: `dgemm` now sums as OpenBLAS's four partial sums; every other change
  additive.

### Record, golden, memory, threads, trial
- **Record `fsl-topup-replica`** (standard tier, from 24cf764 as 5a7a856): 173 cases: 104 pass bit for bit
  (fMRIPrep's calls on ds005040 and ds001600 included: 15 of 15 files each, and fMRIPrep's own outputs the
  same, 9 of 9), 66 rejected by both alike, 3 expected divergences (FSL-124, FSL-134: refused in both
  modes), 0 fail.
- **The four other replica records re-made** (the `dgemm` change): `compare_records` identical case by case
  with equal summaries: mcflirt (237 cases), BET replica (178), FLIRT (302), FAST (86).
- **Golden:** `larmorx-nc` 7 topup/applytopup cases plus 1 refusal, 54 outputs (now 86 cases, 494 outputs);
  no earlier checksum moved. The main package's `record_golden.py`: no checksum changed (208 cases). The
  topup logs held the test's temporary path: relative input paths (5c26745), re-recorded (d9fed9c), the
  check passes twice.
- **Memory:** both modes pass: fMRIPrep's call 93.7 MiB against FSL's 87.6 (within the allowance);
  applytopup lsr/jac/vb3d 13.2 to 13.5 MiB against FSL's 14.2 to 22.5.
- **Threads:** the thread test passes for topup and applytopup.
- **Random-testing trial:** 14,460 realistic cases in 27 minutes (12 jobs, 1.5 GiB per case, FSL through the
  cache in `use` mode): every one bit-identical; nothing to triage or replay.

### New defects (FSL-116 to FSL-135, FSL-141 to FSL-143; 136 to 140 unused)
- Class b, never reproduced: FSL-116, FSL's thread dependence (the replica gives the one-thread result at
  any thread count). On fMRIPrep's ds005040 call at `--nthr=4` FSL's field moves by up to 26.1 Hz, 3.19 Hz
  inside the brain mask; fMRIPrep's own runs equal the one-thread result, so fMRIPrep is unaffected.
- Rule A (corrected by default): FSL-117 (measured first, rule D), 118, 119, 122, 132, 133, 142, 143
  (measured). Rule B (refused by default): FSL-121, 123, 125, 126, 135. Rules A and B: FSL-134. Refused in
  both modes: FSL-124 (FSL exhausts the memory). Message only: FSL-120. Overflow: FSL-141. Library defects
  topup and applytopup never reach: FSL-127 to FSL-131.
- FSL-132 has the same cause as mcflirt's FSL-59; no other FSL replica meets it.

### What was run
- The record: FSL served from the oracle cache 152 times and run 21 times (19 SIGABRT aborts, never
  stored, and 2 `--profile` runs). During development the standard tier ran about 3 times (relay 6) and
  the `at/` cases about 6 times (relay 7), FSL served from the cache except the aborts, `--profile` and
  the first thread runs.
- The page (relay 11): every FSL one-thread run served; FSL at `--nthr=4` on the two fMRIPrep calls ran
  twice each (4 new runs through the cache, which serves such runs only after 3 identical ones); no new
  cache finding.
- Once each: the smoke tier (from 5a7a856: 2,703 passed), the topup record, each of the four replica
  records. Golden: `larmorx-nc`'s checksums recorded twice (a0e6ccf, then d9fed9c for the three logs);
  the main package's once, reusing the smoke tier's marker.
- Not re-run: the smoke tier for 79dbdf0..d9fed9c (golden files and golden.rs's arguments only) and for
  relay 11's 400a6bf and 3b21e31 (the evidence module and the API page only).

### Gates
- Relay 10: `gates-rust.sh all` (root and crates-gpl pass; crates-nc passes but for golden's 54 "not
  recorded" lines, then recorded and passing); `gates-python.sh --no-build` 709 passed; `tables --check`.
- Relay 11: ruff check, ruff format --check (531 files), check_spdx, `tables --check`; the evidence tests
  (60) and every evidence module imports.

### Commits
- larmorx: relay 1 4a4dd9e; relay 2 ba5b5bc, 90d6d3f, ce380a8; relay 3 f1e7fd3, fc82822; relay 4 7d61b37;
  relay 5 ed5451f, 6a1130c; relay 6 53aa746, 843ddd0, 5ebdd29; relay 7 ba1c8df, 2573c77; relay 8 c221e3d,
  d86a451, cf4e92b; relay 9 a7ab9f6, 26945c0, 21a5f8a; relay 10 2c24a7c (merge of main 8220300), 30e7838,
  6cfbc11, 24cf764, 5a7a856, 79dbdf0, a0e6ccf, 5c26745, d9fed9c; relay 11 400a6bf (the evidence module
  `evidence/fsl_topup.py`, its TOOLS and summary PAGES entries, the API page's record numbers), 3b21e31.
- Site (clone `<scratch>/site`, branch `fsl-topup-page` from main 4223a61): 8d9e280
  (`content/tools/fsl-topup.md`, `static/evidence/fsl-topup.json`, `blobs.sha256`); `evidence-blobs`
  72d1f2d (851 blobs: 780 + 71 new, none removed). Exported from 3b21e31 (clean); the first cache entries
  stamped `-dirty` were recomputed from the clean commit (the old ones archived in
  `<workspace>/archive/2026-11-03-topup-figures-cache-dirty/`).
- The page: six worked examples (fMRIPrep's two calls: the pair before and after, the field, the Jacobians
  and displacements by FSL at 1 and 4 threads and the replica in both modes and at 4 threads, FSL's own
  thread band as the yardstick; FSL-117 on `opt/membrane-lm`; applytopup's `lsr`, `jac` and `vb3D` with
  FSL-132 and FSL-143), the trial's table, the case explorer with all 173 cases. Build 0 problems; number
  check 270/270; viewer check 72/72; no example's images "could not be loaded"; page 6.8 MB (budget 40),
  site 298.0 MB; inventory after >= before on every page (the tool flags the Validation page only because
  topup's entry shifts the summary's indices; by page every earlier entry is identical); screenshots
  1440/390, light/dark, before and after.

### Handoff
`HANDOFF.toml` (28 operations on 13 files; `--check` passes in the worktree and the site clone) and
`HANDOFF.md` (the reasons and what to tell the user), kept here as `handoff.toml` and `handoff.md`.

### Not done
- topup's clean-room original: the next part, from `specs/topup.md`.
- The full tier, the data panel's other PEPOLAR runs and long random testing (the final validation phase).
- applytopup's restorations of lines with fewer than 8 or more than 256 voxels, and band systems needing
  LAPACK's blocked band factorisation: refused in both modes (a follow-up).
- FSL's results above one thread: not reproduced (FSL-116); an upstream report is a candidate for the user.
- No benchmark report.

### Merge notes (the orchestrator, 2026-11-03)
- Eleven relay agents; each report read before the next relay was launched, the orchestrator's items in
  NOTES.md ("FROM THE ORCHESTRATOR"): relay 2's `dgemm` change checked by re-made mcflirt, BET, FLIRT and
  FAST replica records (identical); LAPACK's symmetric solver before sub-task 3 ended; the thread findings
  decided by the D13 rules (one-thread result); the random-testing runner's opt-in cache mode for the trial;
  the defect IDs FSL-141 to FSL-150 after FSL-135 (136 to 140 went to FAST's clean-room part); applytopup's
  undocumented `vb` methods within full completeness (sub-task 4b); FSL-132 against the earlier replicas
  (only mcflirt's FSL-59).
- Scans: 31 commits, all by the no-reply identity, no AI attribution, no home path, account name or
  hostname added; no clean-room crate touched; no shared document edited in the branch. The site branch:
  one commit, the same identity, no home path.
- The merge with main e9cc529 (FAST's golden FSLDIR fix and the CI-like gate) needed no resolution.
- The 28 handoff operations applied with `apply_handoff.py`, golden.md's correction of the main package's
  stale count included; the register recounted (336 entries: a 76, b 13, c 151, d 23, e 58, f 13,
  explained 2; no **decide**, no **yes**) and its counts paragraph rewritten (eight entries with a second
  class: FSL-134 and FSL-135 added); tables, the site's status and summary regenerated (71 new blobs copied
  into the site checkout, 851 verified); CLAUDE.md's "Next".
- The agents' 26 scratch scripts not in the branch archived in
  `<workspace>/archive/2026-11-03-fsl-topup-replica-scratch-scripts/`.
