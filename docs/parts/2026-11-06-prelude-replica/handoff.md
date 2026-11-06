# PRELUDE's replica part (2026-11-06): the handoff

The exact changes are in `HANDOFF.toml` beside this file (24 operations, 11 files: 7 of the main
repository, 3 of the site, `validation/tools.toml`). `python -P scripts/orchestration/apply_handoff.py
<scratch>/HANDOFF.toml --site <scratch>/site --check` passes **against main's checkout** (fd3df7b,
after the clean-room audit's merge) and the site clone (`prelude-pages`). Against the part's own
worktree one operation does not apply ([13], next-steps' order paragraph): its text came with the
audit's merge, so apply the handoff after merging the branch into main. Afterwards: `python -m
larmorx_validation tables --site <site>` (the README's tool table, the validation index and the
site's status block: prelude's new rows and topup's replica counts, 177 and 107), `register_counts.py`
(13 new rows), `python -m larmorx_validation evidence summary --site <site>` (prelude's row:
`evidence/summary.py`'s PAGES gained `/tools/fsl-prelude/` in the branch).

Branch `worktree-agent-a9212aaf7ebf67263` (worktree `larmorx/.claude/worktrees/agent-a9212aaf7ebf67263`,
from main 43f0738), 27 commits, the last `b2a352b8` (or later: NOTES.md has the final one).
Site: branch `prelude-pages` of `<scratch>/site` (from site main aca371e): `0334005` (prelude's page,
`content/tools/fsl-prelude.md`, `static/evidence/fsl-prelude.json`, the manifest) and `384bb72`
(topup's page after its replica's record was made again: 177 cases, FSL-273); `evidence-blobs`
`067d4e5` (1209 blobs: the 1133 before and prelude's 76; the manifest's SHA-256 c191a17069cb8c2a).
Push `evidence-blobs` (force) before `main`.

The site's checks: build 0 problems (156 links into the worktree); prelude's page: number check 90 of
90, viewers 30 of 30, no failed example, 6.99 MB (budget 40 MB); topup's page after its update: 1014
of 1014, 182 of 182; the site 332 MB. Inventory (`validation/investigations/difftest/site_inventory.py`,
`<scratch>/inventory.md` with the shared files previewed, `<scratch>/inventory-topup.md` after
topup's update): every page after >= before, prelude's page new, the home page, tools index and
roadmap each gained a row. Two pages are marked "NO" by the tool, neither a loss: the Validation page,
because the summary's tools are addressed by index and prelude is inserted at index 18 (compared by
page every earlier tool's summary entry is the same; the manifest gained 76 lines and lost none),
and topup's page, whose four shown counts grew with its re-made record (176 -> 177 cases, 106 -> 107
bit-identical; a row and 188 words added, nothing removed). Screenshots: `<scratch>/shots/after/`
(prelude's page: top, ds005454, FSL-272, ramps, truth, cases at 1440 and 390, light and dark) and
`<scratch>/shots/{before,after}/` (home's tools table, tools index, roadmap, Validation summary, both
widths and themes).

## Why each operation

- **The register, 13 rows (FSL-261 to FSL-273), all †:** found from FSL's source (the mechanism);
  FSL-272 and FSL-273 also measured black-box by the spec writer. Classes: (a) where FSL reads memory
  it does not own (FSL-263, FSL-265), (c, a) where a NaN reaches a sort (FSL-268, FSL-269), (e) for
  `--end` before `--start` accepted silently (FSL-270, as FSL-135), (f) for the messages (FSL-267),
  (c) otherwise. Gap **original**: the replica corrects by default, PRELUDE's clean-room original
  (the next part) still to be written from `specs/prelude.md` §9; FSL-273: "original (coefficient
  fields)", the topup original already does it for field images. FSL-270's case is `err/empty-range`
  (the findings said `opt/series-start-end`: corrected in the branch, b2a352b).
- **The register's replica paragraph** (after BET's): what PRELUDE's replica corrects, refuses and
  keeps, as the other replicas' paragraphs do; FSL-273 named there as the same part's finding.
- **The restricted section** (appended; its heading gains "prelude"): the source mechanisms, the
  cases, unit tests and probes. No link points at the old heading's anchor (checked).
- **The user-facing table** (`docs/api/upstream-defects.md`): one row per correction or refusal
  (FSL-267, kept, has none), after FSL-242.
- **CLAUDE.md rule 1 and the porting guide §1:** `docs/findings/fsl-prelude.md` is source-derived
  (closed to clean-room implementers). The replica author's probes `validation/investigations/fsl-prelude/`
  are closed too; `validation/investigations/prelude-spec/` and `spec-answers-4/` are open (FSL runs,
  generated inputs, the spec's model): the porting guide lists no investigation folders, so these
  go into the clean-room audit's restriction table (below).
- **`validation/tools.toml`:** prelude's `[[tool]]` (after fslstats) and `[[record]]` (after
  fslstats' replica; `tables.py`'s METRICS has `fsl-prelude-replica` since relay 2). The status
  table says "68 of 68 compared cases bit-identical" while the site's page counts 67 bit-identical and
  1 expected divergence: the record's complex128 case (FSL-265) passes with the values FSL takes
  from memory it never wrote reported, not compared; both texts say so. topup's `how` names FSL-273.
  topup's replica counts (177, 107) come from its record re-made by relay 5: only `tables` to run.
- **`docs/validation/golden.md`:** prelude's 15 cases and 4 failures, recorded by relay 2 from fb86dc8
  (8fc37df), 69 rows, every other checksum unchanged.
- **`docs/status.md`:** prelude among the replicas; the catch-up (2026-11-05) and PRELUDE's replica
  in the FSL paragraph, its "Next" brought to PRELUDE's clean-room original and 3dvolreg.
- **`docs/next-steps.md`:** the order paragraph's item (2) done; the gathered spec questions marked
  ANSWERED (topup §7.3; FLIRT §6.4 normcorr and §3/§8.2 zero signs; mcflirt's leastsquares; fslmaths
  §10.4, §19, §7) and the four groups still open (BET, FAST, topup's `--splineorder=2` and 1 × 1 × 3
  smoothing, standard_space_roi's headers; FLIRT's other items untouched). No source detail in
  next-steps (clean-room readers): the replica author's pointers for BET's `-c`/`-R` and
  standard_space_roi's headers are only in this part's notes (`docs/parts/`, closed). A new
  follow-ups block: PRELUDE's clean-room original, the clean-room follow-ups from the answers (FLIRT's
  normcorr per point, also `lx.mri.hmc`; topup's coefficient-field Jacobian, FSL-273; fslmaths' FFT
  twiddles with glibc's `sincos` in `reproduce`; hmc's leastsquares), two harness items and one site item.
- **The site:** `site.yml` (prelude after fslstats), `data/status.yml` (the FSL planned line, which
  still named the finished catch-up; the generated block from `tables --site`), `content/roadmap.md`
  (Last updated, step 1's status, a Completed row), `static/evidence/summary.json` (re-export).
  The preview (applied in the clone's working tree, then restored): `<scratch>/site-shared-preview.diff`.

## For the orchestrator (not in the operations)

- **The clean-room audit's restriction table** (main's `validation/src/larmorx_validation/cleanroom_audit/`,
  not in this branch, which predates the audit's merge), for PRELUDE's clean-room part:
  `restrictions.py` `SOURCE_FINDINGS["fsl-prelude.md"] = ("FSL", <its first commit, 94f20305's date>,
  <the merge commit that lists it>)`; `verdicts.py` `FSL_TOOLS` += `(r"(^|/)prelude(/|\.rs$)", "prelude")`,
  `TOOL_NAMES[("FSL", "prelude")] = "prelude"`, `BRIEF_FILES` += `fsl-prelude` (the replica's probes;
  not `prelude-spec`, open); `similarity.py` += `(r"^fsl-fugue/", "prelude")` for `crates-nc/fsl-source`
  and `(r"^larmorx-nc-fugue/", "prelude")` for the replica crate.
- **topup's site page** was stale after relay 5's record (176 → 177 cases, FSL-273): re-exported in
  this part (`python -m larmorx_validation.evidence.fsl_topup refresh "<why>"`, then `evidence
  fsl-topup` from b2a352b; site commit 384bb72; no image changed). `summary.json`'s topup row follows
  at the orchestrator's `evidence summary`.
- **Gates** (relay 6, b2a352b; Rust unchanged since relay 5's passing gates at d61cf50c):
  gates-python --no-build: 827 passed, 12 skipped, 1 failed, `validation/tests/test_tables.py` (the
  generated tables: README.md's and docs/validation/README.md's topup rows, one line each, the
  re-made record's counts; `tables` at the merge); CI's environment 660 passed; ruff, ruff format,
  check_spdx, `fsl_source.py --check` (150 files) pass.
- CLAUDE.md's "Where things are" ("next ... PRELUDE's replica and clean-room original") and
  `ORCHESTRATOR-STATE.md`: the orchestrator's own text.
- `docs/parts/<the part>/`: BRIEF.md, NOTES.md (with the six relays' agent ids, for the audit's
  archive), HANDOFF-relay5.md, this file, HANDOFF.toml and the final report.
- The part's folder name in the next-steps text: written as "the part's notes, `docs/parts/`" (no
  folder name assumed).

## Tell the user (through the orchestrator)

- PRELUDE (FSL's phase unwrapping, which fMRIPrep runs for every phase-difference field map) has its
  exact replica: every file FSL writes, bit for bit, in every compared case, fMRIPrep's seven calls on
  four oracle runs included (fMRIPrep's own outputs the same), and 50,702 of 50,711 random cases (the
  nine others the random generator's own mistake, fixed).
- Twelve defects of FSL's prelude found (FSL-261 to FSL-272), eleven corrected by default or refused,
  FSL's behaviour on request. The one that can matter for real data: a phase map whose first voxel is
  NaN is unwrapped whole cycles wrong in places (FSL-272; fMRIPrep's own calls are safe). The spec
  writer found one more in applytopup (FSL-273), corrected in the topup replica.
- The behavioural specification for PRELUDE's clean-room original is written and checked by a model
  of its own text against FSL; most of the questions other clean-room originals were waiting for are
  answered, four groups remain open.
