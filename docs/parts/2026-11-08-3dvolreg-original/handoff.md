# Handoff: 3dvolreg's clean-room original (2026-11-08)

The part's six relays and two spec-writer steps, ready to merge. Shared-document changes are in
`HANDOFF.toml` beside this file (55 operations, 10 files); this file gives the reasons, how to apply
them, the expected conflicts and the audit's material.

## What to merge

- **larmorx:** branch `worktree-agent-a543551b26b82e03a` (worktree
  `.claude/worktrees/agent-a543551b26b82e03a`), head `e3fc0c7c`, from main `2b453c1`; 43 commits (the
  spec writer's 16d7e40 cherry-picked as 103bbba, and its 4402541a and c6422e16 fast-forwarded in).
- **Site:** the clone `<scratch>/site`, branch `volreg-original-pages` at `ab94b11c` (from main
  `e4b3757`: `content/tools/afni-volreg.md`, `static/evidence/afni-volreg.json`, `blobs.sha256`);
  `evidence-blobs` at `a4fa3b63` (1,236 blobs = 1,223 + 13 added, none removed or replaced; manifest
  SHA-256 `f8a73e3c4db1bb65`). Push `evidence-blobs` first, then `main`.
- The clone also holds the worktree `<scratch>/../volreg-cr-5/site-before` (main `e4b3757`, relay 5's
  "before" build): left in place for you to remove.

## How to apply (after the merge)

```
cd <merged larmorx checkout>
python -P scripts/orchestration/apply_handoff.py <scratch>/HANDOFF.toml --site <merged site>
python -m larmorx_validation tables --site <merged site>
python -P scripts/orchestration/register_counts.py
python -m larmorx_validation evidence summary --site <merged site>
(site) tools/build.py --main-repo <merged larmorx checkout>
```

Checked on copies of this branch and of the site branch (temporary git worktrees in
`<scratch>/../volreg-cr-6/`, removed afterwards; the generator `make_handoff.py`, which reads every
`old` text from the current files, and the site's build logs are kept there): every
operation applies (`--check`, and for real); `tables` rewrites README.md's 3dvolreg row, adds the
validation index's `afni-volreg-original` row and the site's 3dvolreg row, and `tables --check` then
passes; `register_counts.py`: 421 entries, no **decide**, **yes** or **original** left in the gap column (one rule 8);
`evidence summary` adds one entry to `summary.json`'s 3dvolreg tool, the clean-room original before
the replica (251 cases: 5 bit-identical, 180 within variability, 3 divergence, 63 both-reject;
`evidence.summary` already lists it since 314cb739); the site with every change built with 0 problems
(28 pages, 163 links into the main repository checked).

`validation/src/larmorx_validation/tables.py` gained `"afni-volreg-original": _bet` in this branch
(e3fc0c7c), so the tool's and the record's texts can say "fMRIPrep's {afni-volreg-original.fmriprep}
calls" (10: eight RobustAverage calls on six oracle runs, ds000256's two runs of the data panel
among them, and sdcflows' two PEPOLAR averages on two more).

## The operations and why

- **`validation/tools.toml`** (generated tables): the `afni-volreg` tool's `how` and `validation`
  name the clean-room original first (as PRELUDE's), `experimental`, with the record's numbers and the
  benchmark's speed (fMRIPrep's calls 2.8 to 5.2 times AFNI's speed and 1.5 to 1.9 times the
  replica's on one thread, 9 to 15 times AFNI's on 14); the replica's texts unchanged. A new
  `[[record]] afni-volreg-original` before the replica's (the originals' records come first, as
  `afni-automask`).
- **`docs/findings/upstream-defects.md`:** the summary's thirteen rows AFNI-86 to AFNI-99 (but
  AFNI-91): "the clean-room original the same since 2026-11-08 (record `afni-volreg-original`: its
  cases)", gap **original** -> "–" (AFNI-97: no suite case; its trial met it on AFNI's runaway first
  passes, and random testing accepts an exit status that differs only by AFNI-97, spec §14.3 "The ties' spread",
  point 4);
  the D13 gaps' 3dvolreg paragraph (the original done, AFNI-100 to AFNI-110 unused); the gap column's
  paragraph (no **original** left). Only the summary table and the D13 gaps' paragraph were read and
  edited: the "Restricted: AFNI 3dvolreg" section is untouched. The counts' line ("421 entries,
  recounted ... 2026-11-07") is unchanged (no new entry); recount at the merge as usual.
- **`docs/api/upstream-defects.md`:** the nine 3dvolreg rows say "both implementations"; the
  replica's paragraph says the original has the same switch and defaults (record's fix pass).
- **`docs/validation/golden.md`:** 246 cases and 1,863 outputs; the recording of 2026-11-08 (6 cases,
  86 outputs, about 0.1 s together: `pytest tests/golden -k volreg` 0.21 s for 7 tests); the coverage
  table (79 command-line cases, `lx.afni` 7); 3dvolreg in "Tools with a replica and an original"; the
  replica's paragraph in the past tense; a new paragraph "3dvolreg, the clean-room original".
- **`docs/next-steps.md`:** the order's item (5) done; a new first item of "Follow-ups, when they fit"
  (the three open spec questions of relay 4, the final phase's work, speed, the random-testing
  harness's leftovers); the replica part's items for the original, the site's panels and the long
  output names marked done; "Waiting for the user"'s §16 entry gains one sentence (the original built
  on that recommendation; the entry stays there for the user).
- **`docs/status.md`:** the AFNI row and its records; the "Next:" sentence (3dvolreg's clean-room
  original done; next the field maps).
- **`docs/decision-log.md`:** a proposed entry "2026-11-07 and 08" recording the orchestrator's §16
  decision (unless the user objects), the spec writer's step-3 limits and switches, and the choice not
  to run a spec-writer step for relay 4's three questions. Edit or drop it as you see fit: these were
  your decisions, written from NOTES.md's "FROM THE ORCHESTRATOR" sections.
- **Site `data/status.yml`** (the planned AFNI line: 3dvolreg no longer planned), **`site.yml`** (the
  3dvolreg card's summary), **`content/roadmap.md`** (last updated 2026-11-08; the "Under way" row 1
  removed and the rest renumbered 1 to 5, each in its own small edit; a "Completed" row for
  2026-11-08).

## Expected conflicts with the field maps' part A (its own worktree)

Both branches touch shared registries; keep both sides: `tests/golden/registry.py` and `golden.json`
(re-record or merge with `merge_golden.py`), `validation/difftest/triage.toml` (both appended at the
end), `validation/src/larmorx_validation/__main__.py` (suite registrations),
`difftest/tools/__init__.py` and `difftest/permanent.py` (SUITES), `evidence/__init__.py` (TOOLS),
`tables.py` (METRICS), `tests/parity/test_memory_peak.py`, `python/larmorx/_core.pyi` (this branch's
`crates/larmorx-py/src/afni.rs` registers `afni_volreg_cli` and `afni_volreg_savedisp` in its own
`register`; `lib.rs` is unchanged here). Shared documents: if part A's handoff is applied first, the roadmap's
renumbering edits ("| 2 | **Field maps** |" and the four after it) and status.md's "Next: 3dvolreg's
clean-room original,\nfield maps," may no longer match: apply this handoff first, or adjust them.

## For the merge's verification

This branch changed shared clean-room code of `crates/larmorx-afni` besides the `volreg` module:
`output.rs` (temporary names for output names of 228 to 255 bytes: 3dTshift, 3dAutomask and
3dUnifize too), `fft.rs` (batched transforms, new functions; the single transform unchanged),
`cfmt.rs` (formats), `cli/mod.rs` (the tool list); and `crates/larmorx-cli/src/replica.rs` (3dvolreg
`original: true`). The parity smoke tier passed on 88ecca2 (2,853) and the golden checksums of every
other case are unchanged; suggested suites at the merge: `afni-volreg-original`, `afni-volreg
--implementation replica`, `afni-tshift`, `afni-automask`, `afni-unifize` (their records should stay
identical), evidence `afni-volreg` and `summary`, `--golden`. The record `afni-volreg-original` is
from 88ecca2 (f90bf8f3); later commits change no crate code (Python evidence and docs, the tables'
metric).

## The clean-room audit (step 5a)

The part's agents (archive their transcripts):

| Agent | Role | Id |
|---|---|---|
| relay 1 | clean-room implementer | a543551b26b82e03a |
| relay 2 | clean-room implementer | a99eecbb30997199b |
| spec writer, step 2 | spec writer (not an implementer; read AFNI's source) | ae4835153236dfbf2 |
| relay 3 | clean-room implementer | a938e0ae66722c27b |
| spec writer, step 3 | spec writer (not an implementer; read AFNI's source) | ab0e98329b5a93c91 |
| relay 4 | clean-room implementer | a406152e7bc33e465 |
| relay 5 | clean-room implementer | a7d6c92c90c4edce3 |
| relay 6 | clean-room implementer (this handoff) | ad8d9af0a170fa8c4 |

The spec writers' commits to `specs/3dvolreg.md` and `validation/investigations/3dvolreg-spec/` were
brought in by the orchestrator (cherry-pick 103bbba; fast-forward to c6422e16); no spec writer edited
the clean-room crate.

Declared items the gate may raise (NOTES.md's "Clean-room statements" has every relay's reads):
- relay 1: a `grep` of `crates/larmorx-core/src/defects.rs` for AFNI_86..99 printed their one-line
  descriptions (written by the replica part); two name AFNI routines (AFNI-87 "mri_to_byte", AFNI-97
  "fft_shift2"); nothing beyond `specs/3dvolreg.md` §12 taken from them.
- relays 3, 4, 5: `validation/difftest/triage.toml` read for their own block only (relay 3: keys and
  statuses through `tomllib`; relays 4 and 5: the block from line 3931); the brief allowed appending
  one's own entries; relay 4's `git diff -U0` hunk header printed one line fragment of the entry
  before the block (a GLIBC-07 entry of another tool: "explanation = "14 cases: GLIBC-07 named where
  the correctly rounded arctangent a...").
- relay 3: the spec writer's model scripts run with `--help` (they started their own cases; stopped by
  a timeout): run, never read.
- relay 5: `cleanroom_audit/restrictions.py`'s `classify` read (to confirm the site is not
  restricted); a draft import line naming `larmorx_validation.evidence.afni_tshift.afni_version`
  (never run; the file never opened).
- relay 6: this session's own transcript metadata read to find its id (`subagents/` listing, names
  only; `agent-ad8d9af0a170fa8c4.meta.json` and the first 600 bytes of its transcript: its own prompt)
  and the field maps relay's `agent-a3f6fcb919202dd47.meta.json` (its description line only:
  "Field maps A, relay 8 (records)"); `cleanroom_audit/verdicts.py`'s `TOOL_NAMES`, `tool_of` and
  `BRIEF_FILES` lines (to check that 3dvolreg's module and this part's folders are classified right).
- Never opened by any relay: AFNI's source, `crates/larmorx-afni-replica`, `crates-nc` and
  `crates-gpl` sources, `docs/findings/afni-volreg.md` or any restricted finding, the register's
  restricted sections, `docs/parts/`, `validation/probes/`, `validation/investigations/afni-volreg/`,
  the replica's parity, evidence and random-testing modules (the evidence module only called), the
  spec model's code.

## Left for the orchestrator (not changed by this handoff)

- The site's `data/status.yml` planned FSL line still lists "PRELUDE's clean-room original" as planned
  (stale since 2026-11-07), and the register's D13 paragraph "PRELUDE's clean-room original is the
  next part": not this part's text.
- `/tmp` holds 43,366 `lx-difftest-inputs_*` folders (1.7 GB), random testing's generated inputs
  from every night and triage (this part's and earlier ones): not removed (a follow-up for the
  harness, in next-steps).
- Logs: `oracles/logs/volreg-original-*` (this part's suites, smoke tiers, benchmark, triage, gates).
