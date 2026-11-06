# HANDOFF: the clean-room audit part (2026-11-06)

Branch `worktree-agent-a60f2409192e31991` (worktree `larmorx/.claude/worktrees/agent-a60f2409192e31991`),
from main b8382b99; main has moved to cf8f6311 since (decision log and next-steps only: no conflict
with this branch, and every operation of HANDOFF.toml was checked on main's tree plus this branch's
files, `tables` included).

The exact changes are in `HANDOFF.toml` beside this file (12 operations, 8 files;
`python -P scripts/orchestration/apply_handoff.py <scratch>/HANDOFF.toml --check`: all apply).
Afterwards run `python -m larmorx_validation tables` (the validation index gets the audit's row).

## What the branch adds (no shared document edited)

- `validation/src/larmorx_validation/cleanroom_audit/` (relays 1 to 4): `scan` (every transcript
  into every agent's access log), `verdicts` (per clean-room original), `similarity` (extra
  evidence), `summary` (the public page) and `gate` (the merge step's question), as
  `python -m larmorx_validation cleanroom-audit <command>`; tests in `validation/tests/`
  (`test_cleanroom_audit.py`, `_verdicts.py`, `_similarity.py`, `_summary.py`,
  `test_archive_transcripts.py`).
- `validation/cleanroom-audit/judgements.toml`: the human judgement of every item the audit raised
  (21 calls, 6 edits by other roles, the 16 verdicts, the limits, the recommendations).
- `docs/validation/clean-room-audit.json` and `.md`: the public facts and the page generated from
  them (`cleanroom-audit summary --check`; a test checks that page and facts agree, and that neither
  holds an absolute path, an account or host name, a session or call id).
- `scripts/orchestration/cleanroom-audit-merge.sh` and `archive_transcripts.py` (and their two
  rows in `scripts/orchestration/README.md`'s table).

## Each shared document, and why

1. **`docs/orchestration.md`**: a new step **5a**, the clean-room audit, between the parity re-run
   (5) and the documents (6), and "steps 1-5 and 5a" in the checklist's first line. Why there: it
   needs the committed merge (its history and judgements) and must pass before anything is
   pushed; it takes about half a minute (archive 2 s, scan 16 s boxed, verdicts 6 s, gate 1 s).
   The STOP and JUDGE lines' handling is spelled out, so the orchestrator's own edits of
   clean-room code are caught too (the gate counts the orchestrator as a role that has seen
   restricted material: every edit of clean-room code it makes must be judged).
2. **`scripts/orchestration/README.md`**, "Merging an agent's part": the same step as item 5,
   "Publish" becomes 6.
3. **`docs/licensing.md`**, "Process rules": a bullet "The proof" pointing to the audit's page
   (what it shows, that it runs at every merge, that the transcripts stay private).
4. **`docs/porting-guide.md` §1**: after "Searching is opening", (a) the two search habits behind
   four slips (two of them undeclared, one under-counted) (`--exclude-dir=larmorx-afni-replica`
   over `crates/`; exclude folders with `--exclude-dir`, not by filtering grep's output), and (b) a
   paragraph "The clean-room audit": what it proves, that it runs at every merge, that a slip is
   declared in the part's report and compared with what the audit finds, and that similarity is
   extra evidence only.
5. **`docs/next-steps.md`**: item (1) of "Then, before FreeSurfer" marked done with its numbers,
   and a follow-up entry (the porting guide's black-box wording, the briefs' grep habit, the
   similarity at merges that change clean-room code and for new families, other machines'
   transcripts, `verdicts.tool_of` for each new clean-room crate or module).
6. **`CLAUDE.md`**: "Where things are" (the audit done, the next items unchanged), **the merge
   rule for the orchestrator** ("The clean-room audit at every merge", after "The orchestrator's
   procedures" in "Working on this machine"), and a repo-map row for the audit's code and
   judgements.
7. **`docs/status.md`**, "In place": one bullet for the audit.
8. **`validation/tools.toml`**: a `[[record]]` "clean-room-audit" (status `reference`) after
   "difftest", so the validation index links the page (then `tables`).

## Proposed, not in HANDOFF.toml (the orchestrator or the user decides)

- **The replica program as an allowed black box.** Every FSL clean-room part ran `larmorx-nc` (the
  replica's compiled program) to compare with; the audit classifies it as "the replica program run
  as a black box" (no file shown), allowed in substance, but no rule names it. Proposed text for the
  porting guide's §1 table, the clean-room implementer's "May read" cell, after "black-box runs of
  the upstream binary": ", and of the replica's compiled program (`larmorx-nc`, the AFNI replica
  through `LARMORX_IMPLEMENTATION=replica`): its outputs and messages, never its source or its
  build's diagnostics". And: "A clean-room brief may name the replica's parity suite for its cases
  and harness (the FLIRT, FAST, topup and fslmaths parts did); the replica's evidence modules,
  investigations and permanent cases stay closed." Rule A-D does not settle a rule of conduct;
  my recommendation: adopt both (they describe what the parts did, and the audit found nothing of
  the upstream through them).
- **A decision-log entry** (if wanted): "2026-11-06, the clean-room audit's result: every
  clean-room original clean; 21 slips of implementers (titles of defect entries, behaviour the
  specs gave, another family's lines, test-harness lines), 5 undeclared; 6 edits of clean-room code
  by other roles (the orchestrator's licence headers, a test path, a merge's package line and the
  quarantine of 2026-10-09; the replica author's 59-second copy before the crate existed; the
  integration agent's 9 lines of plumbing); the audit runs at every merge from now on."
- **The part's folder** `docs/parts/2026-11-06-clean-room-audit/`: the brief (`<scratch>/BRIEF.md`),
  this handoff, the final report and the notes (`<scratch>/NOTES.md`; they name private paths only
  as `<scratch>` and `<workspace>`, except relay 1's run commands: check before copying).

## At the merge

- Run step 5a itself on the merge: `scripts/orchestration/cleanroom-audit-merge.sh . 2026-11-06-clean-room-audit --scratch audit-1 --worktree agent-a60f2409192e31991 --summary`
  (it archives this part's own transcripts and the orchestrator's session, and regenerates the
  page from the newest evidence; commit the two `docs/validation/clean-room-audit.*` files).
  A trial of it ran on this branch (2026-11-06 18:09 UTC): 24 files (121 MB) archived into
  `<workspace>/archive/2026-11-06-session-transcripts-clean-room-audit/`, 4 agents of the part
  found, 0 missing; the scan read 443 files (189 transcripts, 185 agents, 69,006 calls) in 16 s;
  the gate: 16 originals, 203 commits, 0 blocking, 0 to judge; dossier
  `<workspace>/archive/clean-room-audit/2026-11-06-clean-room-audit/`. The page committed on the
  branch is made from the reference dossier `<workspace>/archive/clean-room-audit/2026-11-06/`
  (the one relays 2 and 3 judged); the merge run's `--summary` replaces it with the newer one.
- The PRELUDE replica part merges beside this one: it touches no clean-room code, so the gate is
  unaffected; its agents' transcripts are archived by the next run of step 5a.
- The relays' ad hoc helper scripts (inspection only; every record was made by committed code) are
  kept, private, in `<workspace>/archive/clean-room-audit/2026-11-06/helpers/` (they hold local
  paths, so not in the repository).
