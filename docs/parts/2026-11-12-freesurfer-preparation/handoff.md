# FreeSurfer's preparation (step 3.0): the handoff (relay 9, 2026-11-12)

Branch `worktree-agent-a2dbae2a2c45ae423` (the harness worktree `.claude/worktrees/agent-a2dbae2a2c45ae423`;
relays 1 to 7 already on main through the intermediate merge f9dece03 and relay 7's commits up to 7e504f9f;
relay 8 fast-forwarded to main a1a26763), final commit **`11212b1d`**, clean:
relay 8's three commits (22c522a1, df3d89ff, 7ada9813) and relay 9's. The branch's files and main's (to 0f310788) do not
overlap, so the merge is clean.

Two machine-readable files, both checked with `apply_handoff.py --check` on a fresh copy of main 0f310788 and
applied for real in a trial (main 0f310788 with the branch's files laid over it, `<scratch>/freesurfer-0/r9/trial`,
left there as a preview):

- **`HANDOFF.toml`**: 25 operations on 9 shared documents (below). In the trial: `tables --check` 0,
  `check_spdx.py` 0, the doc and audit tests (`test_docs_shell_blocks`, `test_upstream_watch`, `test_tables`,
  `test_cleanroom_{audit,verdicts,summary}`, `test_freesurfer_replay`) 287 passed, 1 skipped; no private name or
  path in the touched files.
- **`HANDOFF-audit.toml`**: 9 operations on `restrictions.py` and `test_cleanroom_audit.py`, the restricted paths
  as exact code (the orchestrator's: "the orchestrator puts them into restrictions.py with the tests at the
  merge"); in the trial ruff check and format clean, the audit's tests 217 passed, 1 skipped (19 new cases).

**Every text in HANDOFF.toml goes to a document a FreeSurfer clean-room implementer may read.** It holds the
layout, the decisions, and what the official programs show as black boxes (their compiler string, their
outputs at several thread counts, their own printed warnings, the files they open, the update archive's list);
the source-derived facts (which function, which line) stay in `docs/freesurfer/` (restricted). The mechanisms
behind the two black-box findings are in the cards only (below).

## What each operation does, and why

**docs/freesurfer-plan.md** (6):
1. "The version and the files" gains **the official build** (relays 2, 4, 5): one compiler, GCC 8.4.0, in every
   program's `.comment` (the 4dfp tools too: GNU Fortran, not Intel; relay 2's plan correction), ITK 4.13.2
   linked (TP-11), the reference BLAS/LAPACK; GCC 8.4.0 keeps N4's float roundings (relay 5's measurement:
   CLAUDE.md's GCC 11 note does not apply, relay 5's handoff note); the three fMRIPrep/sMRIPrep programs measured
   at 1/2/4 threads (relay 5; this also replaces relay 4's first-pass claim that mri_robust_register's reductions
   are thread-dependent for fMRIPrep, which was never written into a shared document); the update archive (D3).
2. 3.0 item 3: "Output: `docs/freesurfer/inventory.md`" named a file that never existed: the folder, its README
   as the index, done.
3. 3.0 item 4: done, with the proof's numbers (68 steps of the second pass, from relay 7's reports
   `<scratch>/r7/replay-b-*.json`: 66 reproduced, 1 where FreeSurfer varies, 1 with a varying message).
4. "The code's layout": the crates by name (W2, waves.md §4), the two borrowed pieces' replicas in the other
   companions (relay 8's handoff note; names as the orchestrator decided after relay 8), the permissive pieces
   (finding 16), the clean rooms' crates and who creates them (C3.1-00, finding 7), the plumbing (3.2-41).
5. The wave table from waves.md §1 (relay 6's note; relay 8's 3.6 wording with `mpr2mni305`), a column with the
   queue's sizes (waves.md §5), a "later" row, and the totals.
6. "Data files": the plan's "about 25 data files" with the 2019 GCA and `wmsa_new_eesmith.gca` was wrong (relay
   4's correction): 143 measured, by licence (data-files.toml: 125 FreeSurfer's, 8 MINC, 4 MNI-derived, 6
   Washington University), the classic stream's real atlases; D1, D2, D4 as the orchestrator decided them.

**docs/next-steps.md** (4): row 3's state (3.0 done, 3.1 in progress); row 6 gains FreeSurfer's permissive
pieces' clean rooms (relay 8's handoff note); the "Open, FreeSurfer" follow-up: the sMRIPrep run on the server
for relay 1's question 1, ITK 4.13.2 confirmed by third-party.md §10; a block "From FreeSurfer's preparation"
before t2smap's with relay 7b's open items as the orchestrator listed them (the networks' threads; the two thread
effects, worded without function names; the random sources, with relay 9's measurement; the classic stream's
later steps and the large steps; the streams against recon-all.log), the proof page's empty tables, the
reference runs' environment (relay 7's observation), new recordings and the audit's blob list, W6, and relay 7's
flaky cache test.

**docs/decision-log.md** (1, appended): "2026-11-12: FreeSurfer's preparation (the orchestrator's decisions
within the user's rules)": D1 to D4, GCC 8.4.0 (both toolchains), x87 as a foundation item, W1 to W6, the two
crate names, the Talairach scope (`mpr2mni305` included: relay 8's reading, accepted by the orchestrator after
relay 8), who creates the clean-room crates, the stable queue numbers, Q7 and the proposals below. The user's
own decisions of 2026-11-12 are already in the log; nothing of theirs is restated.

**docs/validation/oracle-cache.md** (3): relay 7's three texts (the `freesurfer` environment family, the
`freesurfer-replay` runner, FreeSurfer's files compared provenance aside), with one addition to the runner's
rules: a step FreeSurfer varies in is never cached (replay.py's VARIES).

**upstream.tsv** (1): `freesurfer-8.2.0`, GitHub's `v8.2.0` archive with the SHA-256 of
`tags/freesurfer-8.2.0-download/freesurfer-8.2.0.tar.gz` (bb1699f7..., the same as the plan's), licence
`FreeSurfer-1.0` (the `freesurfer` clone's own identifier), location `tags`. **Note:** the tag's marker
`tags/freesurfer-8.2.0/.upstream-sha256` holds `sha256sum`'s line (`<hash>  freesurfer-8.2.0.tar.gz`) where
`bootstrap_workspace.sh` writes and compares the bare hash: the bootstrap would fetch the archive again once.
Rewrite it as the bare hash (`tags/ITK-v4.13.2/.upstream-sha256` has the same form: track C's line).

**CLAUDE.md** (5): "Where things are" (the preparation complete); rule 1 names the two crates
(`crates-nc/larmorx-nc-tkregister2`, `crates-gpl/larmorx-gpl-gsl`) and, in the "Original" bullet, what a FreeSurfer
clean-room implementer never opens (the audit's FreeSurfer rules in words: today's and the proposals below; drop
`freesurfer-oracle/` if proposal 5 is declined); "Facts worth knowing", Sizes: the measured sizes (relay 2's
correction: code.md §3) and the queue.

**docs/porting-guide.md** (2): §1's list of what a clean-room implementer must not open gains `crates-freesurfer/`
and FreeSurfer's material (as CLAUDE.md). **If the documentation revision has reworded §1 by the time of this
merge**, the two `old` texts may no longer match: put the same list into its new wording.

**docs/licensing.md** (2): the packages paragraph names the two crates (and that no FreeSurfer program links
GSL's); the Talairach paragraph says which programs the clean room covers.

**docs/status.md** (1): a bullet "FreeSurfer 8.2.0's reference runs, inventory and replay tool" among what is in
place.

**Not in the handoff:** the README (no tool's status changes; the tool tables are generated), the register
(below), the site (no page for this part), `docs/validation/oracles.md` (track B's merge wrote the unit oracle's
section; relay 5's conda-forge GCC 8.4.0 is a measurement tool, recorded in the decision log's entry).

## The defect register: candidates, not entries

The brief gave the part no defect-ID range, so (the orchestrator's instruction) the findings go to their
programs' items: a new section "Findings for the defect register (rules A to D when ported)" in the cards
(`cards.py`'s `REGISTER_CANDIDATES`, 11 cards), each decided by the program's replica author with an ID of its
own range. The two black-box findings of relay 7, with the mechanism relay 9 read in the source (restricted):

- **`defect2seg`'s colour table varies** (cards `defect2seg`, `mri_label2vol`): `mri_label2vol --defects`
  (inside `defect2seg`, lines 101 and 125) gives `surface.defects.mgz` a colour table of random colours
  (`MRISdefects2Seg`, `utils/mrisurf_defect.cpp` 15443: `CTABunique` over `CTABrandom`, `colortab.cpp` 495 and
  552) from `randomNumber`'s clock seed (`utils.cpp` 117-148: no `setRandomSeed`, no `FREESURFER_SEED`); the
  program prints `mri_label2vol supposed to be reproducible but seed not set` (measured in every reference run:
  `clock_seeds.py`). Nondeterminism: never reproduced by default (D13): a fixed seed by default, the clock under
  `reproduce`. A clean room learns it from the warning and the replay (black box).
- **`mri_convert --conform` prints an unset field** (cards `mri_convert`, `mri_nu_correct.mni`): `valid   :
  <number>` from `vgprint(true)` on `MRIconformedTemplate`'s header (`mri_convert.cpp` 2927-2928), whose `valid`
  nothing sets: memory content, different in every run (measured in the recorded logs: rawavg to orig in both
  streams, and the classic stream's `mri_nu_correct.mni` `--like` call that relay 7 saw). A message only: a
  defined value by default, a warning where `reproduce` cannot give memory (D13).

The others in the cards: relay 5's (`mri_robust_template`'s centroid, the Gaussian smoothing's partitioned sums
under `mri_coreg`, `mri_brainvol_stats`' sums, recon-all 4220, `mri_normalize`'s spline end clamps), relay 1's
three script behaviours (`recon-all`, `mri_vsinus_seg`; the `T1PialSurf` one confirmed on the six v8 runs by
relay 9), and relay 9's `mri_edit_wm_with_aseg` clock seed (v8 only; its replayed steps reproduced: rule D,
measure on the other heads first).

## The restricted paths (proposals 1 to 5; the code in HANDOFF-audit.toml)

Checked against `restrictions.py` on main 0f310788. Already covered there: the clone and tags (R03), the unpacked
package's `bin`, `python`, `lib`, `etc`, ... (R03), the unit oracle's build tree (R03), the reference runs'
`scripts/recon-all.local-copy` and `scripts/recon-config.yaml` (R03), `crates-freesurfer/` and `larmorx-freesurfer*`
(R10), `docs/freesurfer/`, `docs/findings/freesurfer-*.md`, the inventory's and the unit oracle's tools, the replay
tool and `freesurfer_unit.py` (R11, run-only: Q7), the borrowed pieces' web sources (R19), other agents' scratch
(R18: the preparation's `<scratch>/freesurfer-0/`, with its call graphs and the GSL, Burkardt and EPFL downloads
in `r3/` and `r5/`, and the tracks' `<scratch>/freesurfer-1/`).

1. **The recordings' configuration blobs.** A recording keeps every file by SHA-256 in
   `record/blobs/<2 hex>/<sha256>`, so recon-all's `scripts/recon-config.yaml` sits there under a hash name.
   `recording_source.py` (new, inventory tools) lists them from the recordings' file maps: two in the 19
   recordings (12 reference, 6 classic one-thread, 1 stopped), `6930b943...` (classic) and `7d560ebd...` (v8); no
   other blob is a file of the package's source folders (60,284 files by the replay tool's manifest; one 1-byte
   newline matches empty Python files). Rule R03, family FreeSurfer:
   - `FREESURFER_CONFIG_BLOBS = frozenset({"6930b94343b8c1767ce6fdfa301cdb4fe381ee885ce4b20a99097e2184ca0ba6",
     "7d560ebd1430a9050bec9c3f71ed4e350f472304f418bb07aa7435fccc1a9ef7"})`
   - in `_classify_workspace`: `^oracles/freesurfer-[^/]+/(?:runs|failed)/[^/]+/record/blobs/[0-9a-f]{2}/(?P<sha>[0-9a-f]{64})$`
     with `sha` in the set -> `restricted("R03", "FreeSurfer", op, rel)`
   - tests: `(f"{WS}/oracles/freesurfer-8.2.0/runs/a-v8/record/blobs/7d/7d560ebd...", "read", "restricted", "R03")`,
     the classic blob with `"list"` -> `"restricted-names"`, and another blob -> `("oracle", None)`.
   A content rule was considered ("a blob whose recorded path is recon-config.yaml"): the audit classifies paths,
   so the set is listed instead, and `recording_source.py` re-run when recordings arrive (next-steps).
2. **recon-all's copies wherever a subject directory lives** (relay 7's replays' work folders,
   `oracles/inputs/freesurfer-replay/work/**` and `probe/**`, 72 copies today; a replay's `--dest` anywhere, a
   scratch folder included; a stopped run's `failed/<run>/`, which today's `runs/`-only pattern misses). An
   anchor, R03, family FreeSurfer: `/scripts/(recon-config\.yaml|recon-all\.local-copy)$` (after the
   `larmorx-freesurfer` anchors); and `(?:runs|failed)` in today's workspace pattern. Tests: the failed run's, the
   work folder's and a scratch `--dest`'s copies -> R03; the work folder's `mri/wm.mgz` -> `("oracle", None)`. A
   later clean-room `recon-all` (3.7) writing its own `scripts/recon-config.yaml` would match too: its brief names
   its output folders, or the audit's judgements file judges those reads.
3. **GSL's replica crate** (queue 3.1-53): `/crates-gpl/larmorx-gpl-gsl(/|$)` and `/larmorx[-_]gpl[-_]gsl(/|$)` ->
   R10, family **FreeSurfer**, **before** `/crates-gpl(/|$)` (family AFNI/tedana, which would not stop the RANLUX
   clean room's implementer, 3.1-05, nor any FreeSurfer clean room). Test: `crates-gpl/larmorx-gpl-gsl/src/ranlux.rs`
   -> R10, family FreeSurfer; `larmorx-gpl-tedana` stays tedana, `larmorx-gpl-cli` AFNI/tedana.
4. **tkregister2_cmdl's replica crate** (queue 3.5-26/27): `/crates-nc/larmorx-nc-tkregister2(/|$)` and
   `/larmorx[-_]nc[-_]tkregister2(/|$)` -> R10, family **FreeSurfer**, before `/crates-nc(/|$)` (FSL). Test:
   `crates-nc/larmorx-nc-tkregister2/src/main.rs` -> R10, FreeSurfer; `larmorx-nc-bet` stays FSL.
   With 3 and 4: the two workspaces' build outputs carry FreeSurfer's family too (`FSL/FreeSurfer`,
   `AFNI/tedana/FreeSurfer`), and those crates' metadata FreeSurfer's (no effect before the crates exist; drop
   that op if the orchestrator prefers to add it with the crates).
5. **The recorder** `validation/investigations/freesurfer-oracle/` (not listed today): R11, run-only, as the replay
   tool (its author, the orchestrator, had read the inventory; its README documents recon-all's steps).
   `docs/validation/freesurfer-replay.md` stays readable (black-box observations of the files; relay 7 checked,
   and relay 9 did not edit it). Tests: the recorder's README -> R11; the replay page -> `("repo", None)`.

Not proposed now: (6) for step 6, the permissive pieces' clean rooms will need those pieces' sources restricted
per piece (`tags/minc-1.5.1`, `tags/nifticlib-2.0.0`, `tags/ITK-v4.13.2`, ...; today permissive or workspace), as
the ANTs and ITK clean rooms will; (7) the unpacked package's `python/` rule also closes its bundled TensorFlow,
PyTorch, neurite and voxelmorph (permissive) to every FreeSurfer clean-room implementer: the inference core's
clean room (CP-13) reads those upstreams from their own sources. Also cosmetic: R10's title does not name
`crates-freesurfer`, R03's not the recordings' copies.

## For the orchestrator

- **The proof page's tables:** `docs/validation/freesurfer-replay.md` has its "The result" heading and the
  generator's marker but no tables. `proof_table.py <scratch>/r7/replay-b-*.json` prints them; relay 9 did not
  splice them, to keep a replica-side relay's hand off a page clean-room implementers read (the output is
  mechanical: the replay tool's black-box verdicts, private names masked). A next-steps follow-up says so.
- **The part's agents** (Claude Code's transcripts of the orchestrator's session, `subagents/`): relay 1
  a2dbae2a2c45ae423 (the worktree's), relay 2 afa7a50fd10f97701, relay 3 aaf5eda3ec64042cb, relay 4
  a1760a3d746b1b3b0, relay 5 a0600fee6c3f9b09b, relay 6 a79a40c9c3eae704c, relay 7 a552d0e6f1e089db6 (stopped by
  the user; its work accepted to 7e504f9f), relay 8 a2bb5317fd398aa4f, relay 9 a1d07ea63edaa7ce4; all
  replica-side. The coherence reviewer a2ed27ee7474cacfc read the part's notes (role "documentation").
- **Scratch material to keep or archive** (`<scratch>/freesurfer-0/`): the call graphs (`cg-*.json`, needed to
  regenerate waves and cards), `licences.json`, `cmake_targets3.json`, `r3/` to `r9/` (downloads with their
  SHA-256 in third-party.md §8, the strace jobs `r4/trace/` that cards.py reads, relay 7's replay reports and
  scripts, the trial); all closed (R18). Every tool behind a finding is in the repository.
- **ORCHESTRATOR-STATE.md** may drop the preparation's lines once merged; the reference runs' own lines stay.
