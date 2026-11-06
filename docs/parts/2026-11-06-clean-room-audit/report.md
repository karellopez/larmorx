## The clean-room audit part: final report (relays 1 to 4, 2026-11-06)

The last relay's report as returned to the orchestrator, condensed; the full texts are in `notes.md` and `handoff.md`
beside this file. The role: a validation and tooling agent (it may read anything, the transcripts included; it never
edits a clean-room crate or `specs/`). Branch of 14 commits on b8382b9, last ded424d.

### What was built
- **The scanner** (relay 1; `validation/src/larmorx_validation/cleanroom_audit/`, `python -m larmorx_validation
  cleanroom-audit scan`): every session transcript read (streaming), every agent's and the orchestrator's access log
  (Read, Grep, Glob, Bash commands parsed for paths, outputs for search hits, diffs and commit hashes), the restriction
  table R01 to R18 with the date each item began to apply. First run: 427 transcript files (5.33 GB; every archived one
  matching `SHA256SUMS`), 4 sessions from 2026-10-09 11:38 UTC, 179 agents (every launch with its transcript), 68,225
  tool calls, 230,608 accesses.
- **The verdicts** (relay 2; `cleanroom-audit verdicts`): each clean-room original's commits attributed to their
  makers (203 commits, all attributed: 199 by clean-room implementers, 62 agents in that role; 3 by the orchestrator:
  SPDX headers, a unit test's temporary path, an unused `libm` line removed at a merge; 1 by an integration agent: a
  9-line "implementation: original" message on 2026-10-20), each restricted access judged by its family and date;
  the declared slips compared with what the transcripts show.
- **Code similarity** (relay 3; `cleanroom-audit similarity`), figures only, never a threshold (the user): each
  clean-room original against its own upstream shares 0.0-0.4 % of its tokens in runs of 8 or more (3.3 % for
  larmorx-afni's shared code: NIfTI's public-domain quaternion method), against a floor of 0.0-0.9 % against unrelated
  upstreams; the replicas 0.3-8.5 % (runs up to 132 tokens); calibration: a Rust port of C 12-37 %, two Rust
  translations of one C source 64-100 %, independent implementations 0-7 %. Every run of 40 or more tokens read on
  both sides and explained (`similarity.EXPLAINED`); two written after the replica's near-identical code with no
  access recorded (FLIRT's introsort heap step, the strtod grammar for `inf` and `nan`): one published algorithm.
- **The public summary** (relay 4; `cleanroom-audit summary`): `docs/validation/clean-room-audit.{json,md}`, generated
  from the private dossier and the human judgements (`validation/cleanroom-audit/judgements.toml`), with a privacy
  check (no absolute paths, account or host names, session or call ids, e-mail addresses, transcript names).
- **The merge step** (relay 4): `scripts/orchestration/archive_transcripts.py` and `cleanroom-audit-merge.sh`
  (archive the part's transcripts with SHA-256, scan, verdicts, the gate: exit 1 when a clean-room original was edited
  after its writer saw restricted material of its family and no judgement covers it; `--summary` regenerates the page).

### The verdicts
Every one of the 16 clean-room originals (3dTshift, 3dAutomask, 3dUnifize, larmorx-afni's shared code, the head-motion
correction, BET, FLIRT and its programs, FAST, topup, fslmaths and fslstats, standard_space_roi, fslroi, larmorx-fsl's
shared code, cbrtf, sin/cos/sincos and tanh with glibc's bits): **clean**, no agent with access to the upstream's
source or the replica's code wrote its algorithms. Slips: 21 judged, of them 5 undeclared (D13 part 2a's 35 register
headings; one AFNI replica line calling larmorx's own memory check; catch-up relay 1's 3 AFNI replica lines, another
family; catch-up relay 3's one docstring line of a replica suite), none carrying an upstream routine; AFNI-26's
declaration under-counted by two lines. The replica author's csfft port sat in a clean-room crate's folder for 59 s
before any clean-room work (2026-10-09), quarantined by the orchestrator, never committed there.

### What the evidence cannot see
No sessions before 2026-10-09 11:38 UTC (the repository's first commit is 15 minutes older; no clean-room code then);
files programs open by themselves (cargo, pytest, scripts) only through their arguments and printed output; logs
written to files and never shown; names printed by `find`/`ls`; replica suites some briefs allowed, not judged line by
line; single-file searches compared with the file as it is today; 98 agents without a harness worktree, attributed by
the hashes they printed or their commit commands.

### Recommendations (in the porting guide through the handoff)
Name the replica program as an allowed black box; say which replica suite modules a clean-room brief may name; search
`crates/` with `--exclude-dir=larmorx-afni-replica`, excluding folders with `--exclude-dir`, never by filtering grep's
output.

### Merge notes (the orchestrator, 2026-11-06)
- Merged into main cf8f631 without conflict; HANDOFF.toml's 12 operations applied (CLAUDE.md, the orchestration,
  licensing and porting guides, next-steps, status, the orchestration README, tools.toml); CLAUDE.md's note on the
  transcripts worded for `cleanupPeriodDays` 36500 (set on both machines the same day); `tables`, `tables --check`
  (with `--site`).
- The new merge step run on this part (`cleanroom-audit-merge.sh . 2026-11-06-clean-room-audit --scratch audit-1
  --summary`): 4 agents, 0 missing; 16 originals, 203 commits; 0 blocking, 0 to judge; the page regenerated. Found at
  the merge: a part named as its `docs/parts` folder got its date twice in the archive's folder names; both scripts
  now keep a part's own date (a test added), the two folders of the first run renamed (`-first-merge-run`,
  `-merge`; relay 4's trial dossier `-trial`), the step run again.
