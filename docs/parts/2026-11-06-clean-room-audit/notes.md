Worktree: <workspace>/larmorx/.claude/worktrees/agent-a60f2409192e31991
(branch worktree-agent-a60f2409192e31991, fast-forwarded to main b8382b99 at the start of relay 1)

# NOTES: the clean-room audit part (relay notes)

Brief: `<scratch>/BRIEF.md` (this folder). Relays: one sub-task each, then stop.

## State

- Relay 1 (2026-11-06): sub-task 1 DONE, commits 2ed33dab (WIP) and 4b929e37.
- Relay 2 (2026-11-06): sub-task 2 DONE, commits 518f79c6, 8befc3b3 (WIP), e243cdf0, e2aeaa99 on
  `worktree-agent-a60f2409192e31991`. Gates: ruff check, ruff format --check, check_spdx.py,
  `pytest validation/tests` (287 passed), `tables --check`.
- Relay 3 (2026-11-06): sub-task 3 DONE, commits fef9f389, f9de8154, 95ea64a8, 5210da8f,
  fd5f8ffe on the same branch. Gates: ruff check, ruff format --check, check_spdx.py,
  `pytest validation/tests` (329 passed), `tables --check`. Also begun for sub-task 4: the
  generated public similarity section (`similarity.public_markdown`, `similarity-public.md`).
- Relay 4 (2026-11-06): sub-tasks 4, 5 and 6 DONE, commits f276b629, 35db607b, ded424d2 on the
  same branch. Gates: ruff check, ruff format --check, check_spdx.py, `pytest validation/tests`
  (375 passed), `tables --check`. HANDOFF.toml (12 ops, `apply_handoff.py --check`: all apply, also
  on main cf8f6311's tree) and HANDOFF.md in this folder. **The part is complete: no relay 5.**

## Relay 4: done (the public summary, the merge step, the handoff)

**Sub-task 4:** `validation/src/larmorx_validation/cleanroom_audit/summary.py`:
`cleanroom-audit summary --dossier <dir>` extracts the public facts (`extract`: scan + verdicts +
a similarity record + `validation/cleanroom-audit/judgements.toml`, the human judgements of
judgements.md as data: 21 accesses by (agent id, record line), 6 edits by (agent, commit), the 16
verdicts' words, the limits, the recommendations) into `docs/validation/clean-room-audit.json`,
and `render` makes `docs/validation/clean-room-audit.md` from that JSON alone (`--check` compares;
`privacy_problems` refuses absolute paths, account/host names, session and call ids, e-mails,
`.jsonl`). The similarity section is `similarity.public_markdown` of a pruned record
(`prune_similarity`, asserted to lose nothing). `--verbatim <file>` writes every item's call and
output verbatim (private: `archive/clean-room-audit/2026-11-06/verbatim.md`, 21 calls). Tests
`validation/tests/test_cleanroom_summary.py` (42): page = render(facts), no private strings, every
committed item judged, the judgements file, extraction again from the private dossier (skipped
without it), privacy patterns, groups/kinds/scopes, a synthetic dossier judged and unjudged, the
refusal of a private description, the gate, the CLI exit codes. The committed page is made from
the reference dossier `archive/clean-room-audit/2026-11-06/` (scan-run6).

**Sub-task 5:** `scripts/orchestration/archive_transcripts.py` (copies every live transcript file
the archive lacks by path and content into `archive/<date>-session-transcripts-<part>/main-machine/`
with SHA256SUMS, README.txt, part-<part>.json; the part's agents by --agent, --notes, --worktree,
--scratch (first prompt); exit 1 when one is missing; a non-prefix earlier copy is kept as
`.before-<sha>`), `scripts/orchestration/cleanroom-audit-merge.sh <checkout> <part> [selectors]
[--summary] [--similarity]` (archive, boxed scan, verdicts, gate; exit 1 = stop; on a stop the
private facts/page/verbatim in the dossier); `cleanroom-audit gate` (blocking: unjudged implementer
findings, unjudged other-role edits after restricted views (the orchestrator always), unattributed
commits; to judge: unjudged slips). Tests `validation/tests/test_archive_transcripts.py` (4). Trial
run on this part (18:09 UTC): 24 files (121 MB) archived into
`archive/2026-11-06-session-transcripts-clean-room-audit/`, dossier
`archive/clean-room-audit/2026-11-06-clean-room-audit/`, gate 0 blocking 0 to judge; with no
judgements: 13 blocking, 21 to judge (exit 1), as expected.

**Sub-task 6:** HANDOFF.toml / HANDOFF.md here; the final report went to the orchestrator.
Relays' helper scripts copied (private) to `archive/clean-room-audit/2026-11-06/helpers/`.

## Relay 3: done (code similarity, extra evidence only)

**The tool:** `validation/src/larmorx_validation/cleanroom_audit/similarity.py`, run as
`cleanroom-audit similarity --out <private dir> [--repo] [--workspace] [--fsl-src] [--wide]
[--jobs 12]` (wired in `cli.py`); tests `validation/tests/test_cleanroom_similarity.py` (42: the
three lexers (Rust nested comments, raw strings, chars vs lifetimes, attributes; C preprocessor
with `#define` bodies kept; scripts), `language`, `drop_tests` (`#[cfg(test)]`, `#[test]`),
`select_fns`, a C function and its Rust translation giving the same names stream, the shape
stream's chains and literal lists, casts and unary `&`/`*` dropped, C conditions, `++`/`--` and
counting loops, the measures on synthetic token arrays (coverage, containment, Jaccard, the
longest run with its files and lines, never across files, beyond the ladder, empty), floors,
the AFNI PROVENANCE table's file -> tool rule, the FSL tables, AFNI modules, a whole run on a
synthetic plan, the public section and public names).

**The method** (the module's docstring has it in full): every file in two token streams:
`names` (identifiers kept by name, lower case without underscores; primitive types, `self.`,
`this->`, `std::` dropped) and `shape` (identifier chains one `I`); both: comments, braces,
condition parentheses, Rust's `let mut pub fn`, attributes, lifetimes, `as` casts, `?`, macro `!`,
`in`; C's qualifiers, primitive and named casts, functional casts, preprocessor lines (defines'
bodies kept), unary `&`/`*` dropped; C counting loops as Rust's `for i a..b`, `x++` as `x += 1`;
literal lists one literal. Rust test modules and `tests/` dirs out. Per pair: identifiers shared
(4+ characters), coverage (share of tokens in a run of >= n tokens also in the reference; names
n = 8, 16; shape n = 16, 32), containment/Jaccard (names), the longest common run (exact,
checked token by token; 64-bit polynomial hashes otherwise). **Why two streams:** the first
design (identifiers as `I` only) was too coarse: unrelated Rust code scored as high as the
replicas (n = 12 coverage 50-80 % against other families) and ports did not separate; the names
stream separates. The sets: clean-room by `verdicts.tool_of`; FSL upstream `crates-nc/fsl-source`
and replicas `crates-nc/larmorx-nc-*` by `FSL_UPSTREAM_TOOLS` / `FSL_REPLICA_TOOLS`; AFNI upstream
`tags/AFNI-26.3.05/src` files the replica's PROVENANCE table names (a file named by one tool's
modules is that tool's, else shared), replica `crates/larmorx-afni-replica` by module; GNU
(`GNU_TOOLS`): glibc 2.35 files, "replicas" = `glibc-reference`'s cbrtf (+ frexpf, ldexpf), the
spec writer's model `glibc-dbl-model`, the FSL replica's `fdlibm.rs`; the wide trees: all C/C++
of AFNI 26.3.05's `src/` and FSL 6.0.7.23's `$FSLDIR/src` (`--wide`). Calibration (`CALIBRATION`):
ports (CORE-MATH cbrtf, sinf/cosf; Arm's sincosf vs glibc's copy), two Rust translations of one
C (sincosf; tanhf/expm1f), independent implementations (CORE-MATH vs glibc cbrtf and sinf in C;
larmorx's CORE-MATH ports vs the clean-room cbrtf_glibc and the Arm translation in Rust); plus
the matrices' off-diagonals (other tools of the family) and the other families.

**The record:** `<workspace>/archive/clean-room-audit/2026-11-06/similarity/` (`similarity.json`,
`similarity.md` private: files and lines; `similarity-public.md` the generated public section,
public names only), made from clean commit fef9f389 with `--wide` (log
`oracles/logs/cleanroom-audit-relay3-similarity.log`; boxed 24 GiB, 12 readers). Earlier trial
runs: `<scratch>/sim1` (identifiers-only design), `sim2`, `sim3` (names stream); the first
record (fef9f389, without the whole trees' floor) is kept in
`archive/clean-room-audit/2026-11-06/similarity-first-run-fef9f389/`. The final record: from clean
commit 95ea64a8 (log `oracles/logs/cleanroom-audit-relay3-similarity-final.log`), its reports
rewritten at fd5f8ffe by `similarity --out <dir> --report-only` (the explanations of 5210da8f;
measuring unchanged). Rerun: `scripts/orchestration/boxed.sh 24 bash <scratch>/work3/run-audit.sh
similarity --out <dir> --wide --jobs 12` (about 15 minutes with `--wide`, 1 without).
Helpers in `<scratch>/work3/` (not committed): `run-audit.sh`, `figs.py <json>` (the compact
figures above), `perfile.py` (per-file coverage of chosen tools), `check_writers.py <scan>` (did
a clean-room file's writers access given replica files), `try_norm.py`, `NOTES.relay2.md`.

**The figures** (names stream; coverage = share of tokens in runs of 8+ tokens also in the
reference; "diag" the tool against its own counterpart, "off" the median / max of the family's
other tools, i.e. the floor; run = longest common run in tokens):

| original | clean-room vs upstream: diag (off) run | replica vs upstream: diag (off) run | clean-room vs replica: diag (off) run |
|---|---|---|---|
| 3dAutomask | 0.4 % (0.1/0.7) 13 | 7.1 % (0.3/0.5) 49 | 10.7 % (9.0/11.1) 29 |
| afni-shared | 3.3 % (0.0/0.1) 38 | 6.3 % (0.0/0.2) 132 | 34.2 % (4.3/5.7) 334 |
| 3dTshift | 0.0 % (0.0/0.0) 7 | 8.5 % (0.2/0.2) 54 | 15.3 % (9.7/12.8) 45 |
| 3dUnifize | 0.0 % (0.1/0.1) 7 | 5.2 % (0.2/1.0) 35 | 14.1 % (7.9/9.2) 58 |
| fsl-shared | 0.0 % (0.0/0.0) 7 | 6.9 % (0.2/0.6) 89 | 24.3 % (5.7/13.4) 87 |
| BET | 0.3 % (0.0/0.9) 30 | 7.7 % (0.2/0.9) 92 | 9.9 % (5.3/13.7) 37 |
| FLIRT | 0.2 % (0.1/0.9) 13 | 1.8 % (0.3/0.8) 35 | 15.1 % (6.3/15.1) 36 |
| topup | 0.2 % (0.0/0.4) 10 | 7.9 % (0.2/2.6) 69 | 11.4 % (6.2/15.3) 25 |
| FAST | 0.0 % (0.0/0.2) 8 | 3.9 % (0.2/0.8) 21 | 9.4 % (8.3/13.7) 35 |
| fslmaths/fslstats | 0.2 % (0.1/0.4) 13 | 2.2 % (0.1/0.9) 96 | 8.2 % (6.8/13.0) 33 |
| fslroi | 0.0 % (0.0/0.4) 5 | 0.3 % (0.2/1.0) 13 | 3.2 % (8.6/11.9) 14 |
| standard_space_roi | 0.0 % (0.0/0.1) 3 | 0.0 % (0.0/0.8) 3 | 9.9 % (6.4/20.1) 33 |
| hmc (mcflirt) | 0.1 % (0.1/0.9) 10 | 1.2 % (0.3/0.8) 17 | 7.1 % (4.9/13.7) 52 |
| cbrtf_glibc | 0.0 % 5 | 11.2 % 13 | 9.7 % 12 |
| sincos_glibc | 5.0 % 45 | 4.3 % 47 | 33.4 % 47 (the spec writer's model) |
| tanh_glibc | 43.2 % 44 | 56.9 % 52 | 70.1 % 100 (a declared fdlibm port) |

Calibration (shared names; coverage 8 / 16; run): ports of permissive C into Rust: CORE-MATH
cbrtf 33 %; 36.9 / 26.9 %; 81 — sinf/cosf 15 %; 12.6 / 8.4 %; 64 — Arm's sincosf vs glibc's copy
32 %; 9.7 / 0 %; 15. Two Rust translations of one C: sincosf 51 %; 63.7 / 36.6 %; 110 —
tanhf/expm1f 100 %; 100 / 100 %; 723. Independent: C vs C (CORE-MATH vs glibc cbrtf, sinf) 0 %;
0 / 0 %; 5; Rust vs Rust (larmorx's CORE-MATH ports vs the clean-room cbrtf_glibc and the Arm
translation) 27-42 %; 2.1-7.3 / 0 %; 10-11. Whole trees (final record, 778 s): clean-room vs every C/C++ file of AFNI 26.3.05's `src/` (2,364
files, 7.6 M tokens) 0.9-5.9 %, runs 10-38, against the floor (the other families' clean-room
code against the same tree) median 2.7 %, max 5.5 %, runs up to 65, and the AFNI replicas
7.9-9.7 %; vs FSL 6.0.7.23's `$FSLDIR/src` (2,389 files, 3.5 M tokens) 0.6-3.6 %, runs 9-46
(BET's quatern.rs: NIfTI), floor median 2.6 %, max 16.7 %, runs up to 38, the FSL replicas
2.0-11.6 %. The public section lists 15 runs of 40+ tokens, every one explained (0 "not yet
explained").

**Reading:** across languages, each clean-room original shares with its own upstream no more
than with the other tools' upstream (diag ≈ off), while each replica stands clearly above its
floor (the translations); in Rust against Rust, every clean-room original is close to its
family's other replicas (shared larmorx idioms, the same command-line conventions) except
afni-shared (34 %: NIfTI's public-domain quaternion method in geometry.rs, the CLI files), and
far below two translations of one source (64-100 %). **Every run of 40+ tokens was read on both
sides** and its nature is in `similarity.EXPLAINED` (public section): NIfTI's public-domain
quaternion (geometry.rs, bet/quatern.rs) and orientation (automask/program.rs) methods; the
specs' option rules (3dTshift CLI, older than the replica's) and sorting network (unifize
median7); larmorx's own notices (hmc CLI, older than the replica's); bash word splitting
(ssroi); fdlibm's declared port (tanh); glibc-dbl's operations (sincos). **Two to note:**
`flirt/schedule.rs` (libstdc++'s introsort heap step, 48 tokens; clean-room 2026-11-02, replica
`stdsort.rs` 2026-10-24) and `cli/options.rs` (strtod's inf/nan(...) grammar, 87 tokens;
clean-room 2026-10-30, replica `miscmaths.rs` 2026-10-24): near-identical form, the clean-room
written AFTER the replica; relay 2's verdicts record no access by their writers to the replica:
one published algorithm (SGI STL / C standard) written in the same terms, by the same model.
Relay 4 should word these carefully in the public summary (no claim beyond "no access
recorded").

## Relay 2: done (the verdicts)

**The tool:** `validation/src/larmorx_validation/cleanroom_audit/verdicts.py`, run as
`cleanroom-audit verdicts --scan <scan dir> --out <private dir> [--repo <larmorx checkout>]`
(wired in `cli.py`); tests `validation/tests/test_cleanroom_verdicts.py` (43: tool_of, role_of,
attribution by printed hash / subject / time / a hash in the command's own output, the register's
restricted-only lines and an output's headings and body lines, judge's levels and families, R18
and R13 by writer, verdict's statuses, git exclusion pathspecs). It does: the clean-room code by
tool (`tool_of`: larmorx-afni by 3dTshift/3dAutomask/3dUnifize/shared, larmorx-mri = the
head-motion correction (FSL family), larmorx-fsl by bet/flirt/fast/topup/fslmaths/
standard_space_roi/fslroi/shared, larmorx-core's cbrtf_glibc, sincos_glibc(+tables), tanh_glibc
(GNU)); every commit of every ref changing that code (203) attributed to its maker (`attribute`:
193 by the printed hash + a commit command within 10 min, 10 by the command's subject; 0
unattributed); roles (`role_of`: description words, then the brief's "You are"/"Your role" sentences,
then the brief file the prompt names and the agent read first; `ROLE_BY_DESCRIPTION` settles 4 by
hand); each restricted access of each toucher judged against the tool's family and date (`judge`:
levels content / possible / scope / names / build / mention / replica-run / build-cache / metadata
/ clean-writer; R18 and R13 by the writers' roles and the part's kind); the register's
restricted-only lines over all 87 versions compared with what each read or grep printed (headings
vs body lines); `implementer_slips` (all 62 implementers, any family); `brief_dependent` (R15
permanent cases by their writer, the triage's replica entries, R16 replica suites where the brief
forbade them (the catch-up), replica parts' and spec writers' investigations from 2026-10-29 with
`BRIEF_ALLOWED` exceptions); greps of one restricted file checked against the file's lines;
`merge_edits` (`--cc`); the orchestrator's edits; `diff_nature` of other roles' commits;
`write_markdown` (the private dossier). Fix in relay 1's reader: git exclusion pathspecs
(`':!crates/larmorx-afni-replica'`) were read as accesses (`accesses._pathspecs`).

**The run:** a new scan `run6` (relay 1's reader with the pathspec fix; 431 files, 181 agents, 68,457
calls; log `oracles/logs/cleanroom-audit-relay2-run6.log`), then `verdicts` (6 s). **Private output:**
`<workspace>/archive/clean-room-audit/2026-11-06/` with `scan-run6/` (the scan used),
`verdicts/verdicts.json`, `verdicts/verdicts.md` (machine-made dossier: per tool the agents, roles,
touches, commits, findings with session, record line and time; every implementer's slip; the
brief-forbidden reads; the orchestrator's edits; every commit's maker) and
`verdicts/judgements.md` (**the human judgement of every item, read it first**).

**The verdicts (summary):** for every clean-room original (3dTshift, 3dAutomask, 3dUnifize,
larmorx-afni's shared code, the head-motion correction, BET, FLIRT and its programs, FAST, topup,
fslmaths/fslstats, standard_space_roi, fslroi, larmorx-fsl's shared code, cbrtf_glibc,
sincos_glibc, tanh_glibc): **no agent that had read the upstream's source or a replica's code
edited its algorithms.** 199 of 203 commits by clean-room implementers; the other 4: the
orchestrator (SPDX headers in 9 files, 10-10; a unit test's temp path, 10-12; a merge's Cargo.toml
`libm` line, 10-30) and the integration agent `implementation="auto"` (10-11: a 9-line
"implementation: original" verbose message after reading the GPL replica's CLI; the roles rule
"never edits a clean-room crate" came 10-20). The csfft replica port sat 59 s in the not-yet-created
crate folder before the orchestrator quarantined it (10-09, before any clean-room work).
Implementers' content-level exposures (12 calls): declared: AFNI-26 (9 GPL-replica lines + 1
register body line; declaration omits 2 `&defects::AFNI_26` lines), AFNI-75/FSL-20..66 (6 register
body lines; a permanent case), FLIRT 1 (AFNI replica env names), 3dTshift 2b / FLIRT 2 relay 1 /
BET 2b († headings), catch-up relays 1 (spec-answers-3 README), 3, 6, 7, 8 (replica suite lines, the
triage tail). **Undeclared:** D13 part 2a (35 † headings by `grep -n "^#"`, 2026-10-11T15:10Z, ten minutes after the
restricted sections were committed and named); Clean-room additions (1 line of the AFNI replica's unifize.rs:
`larmorx_core::alloc::check_plan(needed)`); catch-up relay 1 (3 lines of the AFNI replica:
`TshiftParams {`, other family); catch-up relay 3 (1 docstring line of fsl_topup_replica.py). None
carries routine content. The 20 clean-room briefs in docs/parts hold no restricted register line or
source-derived finding line.

**Uncertain (also in judgements.md):** programs' own reads (cargo, pytest, scripts) are seen only
through arguments and outputs; outputs sent to log files are not in the transcripts; find/ls output
names not parsed; replica suites the briefs allowed (FLIRT, FAST, topup, fslmaths parts) are not
judged line by line; roles are read from words (every toucher checked by hand); single-file greps
compared with the file on main only; the root commit predates the first record by 15 min (no
clean-room code then); topup's brief allowed only its own replica suite (relay 1 read FLIRT's:
declared by topup, counted as allowed by the tool's per-part rule).

Helpers in `<scratch>/work2/` (not committed): gitfacts.py, attrib*.py, commitcmds.py, index.py,
touchers.py, candidates.py, cr_content.py, show.py, rec.py (`rec.py <transcript> <line> [chars]`:
a call and its result), vsum.py, regversions.py; `NOTES.relay1.md` (this file before relay 2).

## Relay 1: done

The tool: `validation/src/larmorx_validation/cleanroom_audit/` (`restrictions.py` the table and
classification, `shell.py` the Bash reader, `accesses.py` one call's accesses and commits,
`transcripts.py` discovery and streaming parse, `cli.py` `scan`), wired as
`python -m larmorx_validation cleanroom-audit scan --out <private dir> [--root ...] [--jobs 12]`
(in `validation/src/larmorx_validation/__main__.py`, a subparser after `tables`; the PRELUDE agent
may touch `PARITY_MODULES` in the same file: different lines). Why `validation/` and not
`scripts/orchestration/`: it is evidence like `tables` and `difftest`, tested by `pytest
validation/tests`; the merge step (sub-task 5) will be a thin `scripts/orchestration/` wrapper.
Tests: `validation/tests/test_cleanroom_audit.py` (78: each tool kind, 26 Bash forms, cd /
pushd / subshell / git -C, a slip with its dates, compaction, two roots merged with the archive's
SHA256SUMS, a persisted output, refused calls, printed and quiet commits, other agents' scratch,
launches linked, the table's dates, 28 classifications, no account name in shown locations).

Helpers in `<scratch>/work/` (not committed; relay 5 decides): `run-audit.sh` (runs the CLI from
this worktree: the harness refuses `PYTHONPATH=... python` chains), `run-tests.sh`,
`run-validation.sh`, `inspect_run.py <run dir> unresolved|cleanroom|rule R..|agent <id> [R..]|
noworktree|compaction` (quick looks at a scan), `slowcalls.py` (times each call's parse; found the
backtracking regex), `grep_calls.py`. NOTE: the harness refuses any Bash command whose text names
`git` in a complex form (heredocs, `python -c` with "git" inside): write the script with Write, run it.

**The run** (run5, the reference: `<scratch>/run5/`, 12 MB, private; earlier runs 1-4 are the
refinement steps; log `oracles/logs/cleanroom-audit-relay1-run5.log`):
`scripts/orchestration/boxed.sh 24 bash <scratch>/work/run-audit.sh scan --out <scratch>/run5
--jobs 12`: 12 s, under 24 GiB (it peaked far below).
- 427 transcript files read (5.33 GB): the archive's 244 (main machine 181, old laptop 63; all 244
  match `SHA256SUMS`, 0 differ) and the live directory's 183; 183 distinct transcripts.
- **4 sessions** (orchestrator files): d96e838f 2026-10-09 11:38Z to 10-26 09:36Z, 7214b6a0 10-26
  09:36Z to 11-01 17:56Z, b322ad6c 10-28 06:17Z to now, 6ad71057 (2 s on 10-31, empty). Continuous
  from the first record (2026-10-09 11:38Z) on.
- **179 agents** (175 launched by the orchestrator, 4 by agents: spawnDepth 2, `parentAgentId`);
  every agent's launch call found, every launch's transcript found (0 and 0).
- **68,225 tool calls** (Bash 56,499, Write 5,096, Edit 3,814, Read 1,863, Agent 181, Monitor 139,
  WebFetch 20, ...). 2,245 commits printed (by `[branch hash]` or a `git log --oneline` after a
  quiet commit, flagged `inferred`).
- **230,608 accesses, 49,284 distinct paths**: read 37,671, search 37,502 (scopes), search-hit
  26,271 (lines shown), search-match 4,031 (names), list 10,529, stat 7,679, script-path 14,386,
  mention 29,092, exec 27,106, build 3,831, copy 2,511, diff-shown 396, diagnostic 1,046, web 657,
  write 19,787, edit 6,406, delete 1,569, move 138.
- Restricted (refused calls excluded), by rule: R01 6,237, R02 1,622, R03 157, R04 8, R05 5,
  R06 272, R07 3, R10 13,038, R11 729, R13 759, R14 637, R17 141, R18 454 (most by replica authors,
  spec writers, validation agents and the orchestrator; not a verdict yet). Of these after the
  rule named them: R01 6,233, R02 1,425, R03 157, R10 12,840, R11 515, R13 735, R17 74, R18 342.
  Other categories: replica-build-output 1,237, replica-metadata 803, register 1,738,
  findings-unlisted 29, difftest-cases 291, run 1,122, cleanroom 9,563, spec 2,130.

**Parsing gaps** (all in `summary.json` `gaps` / `gap_examples`):
- 0 unreadable lines; 0 results without their call; 0 calls only in the API-request copies
  (`api-request-blob`): the assistant records hold every call.
- 11 calls without a result over all copies (4 distinct: interrupted runs at a transcript's end,
  one of them this relay's own call in flight).
- 64 compactions; every pre-compaction record is still in its file. 2 boundaries (the two copies
  of agent a888f8be0d1f2c1af, "BET clean-room part 2b") name a parent record that is in no file,
  but its 2,773 earlier records are present: nothing lost. No transcript starts with a
  continuation summary.
- 1,389 refused calls (the harness's worktree isolation, `<tool_use_error>`, permissions): their
  574 restricted accesses are flagged `refused` and not counted (nothing ran).
- 5,927 paths with a variable the command did not set (`$S`, `$OUT`, `$W`... set in an earlier
  call, or script parameters `$1`): flagged `unresolved`; the anchors (crates-nc, fsl-source,
  docs/findings/<file>, ...) still classify them by their visible part.
- 96 agents without a harness worktree in their meta: 14 early ones (2026-10-09 to 10-26, among
  them the first "Clean-room lx.mri.hmc from spec" (2026-10-10, 48 calls; its second agent of the
  same name has a worktree), the CORE-MATH and csfft ports, data searches: they worked where
  their brief said, the main checkout or a clone) and 82 relay agents from 2026-11-02 on (they work
  in the worktree their NOTES name, with `cd`); relay 2 maps them to branches by the worktree in
  their paths (the `checkout` field) and by their printed commits.
- Not modelled (documented limits): globs are not expanded (a glob is classified by its literal
  part and, under docs/findings, by the R11 names it can match); a program's own file reads
  (`python script.py` without the script's text in the transcript, `cargo test`) are seen only
  through the paths in its arguments (`mention`) and its output; `find`/`ls` outputs are not
  parsed for names (the Glob and Grep tools' are); Python scripts the agent wrote are scanned for
  path literals (`script-path`: assumed opened).
- Sessions before 2026-10-09 11:38Z: none in either root. Git history before then: relay 2 checks.

## For relay 4 (sub-task 4: the outputs)

- **The public summary** `docs/validation/clean-room-audit.md` (generated; a new module, e.g.
  `cleanroom_audit/summary.py` + `cleanroom-audit summary --verdicts <dir> --similarity <dir>
  --out docs/validation/clean-room-audit.md`): per clean-room original from `verdicts.json`
  (`tools`: period, implementers and roles, accesses by category, restricted accesses with their
  nature in words, verdict, slips declared and found; the human judgements of
  `verdicts/judgements.md` are the words to use) and the similarity section =
  `similarity.public_markdown(json.load(similarity.json))` (no path beyond repository-relative:
  upstream files are shown as "AFNI 26.3.05 src/...", "FSL 6.0.7.23 ...", "glibc 2.35 ...";
  check every line for `/home`, `/tmp`, the account name, session ids). How to re-run.
- **The long runs not yet explained** in `similarity.EXPLAINED` (the public section prints "not
  yet explained (to read)"): read both sides of each (allowed: validation role), add an entry
  with its nature, regenerate the section (no rerun: `public_markdown` reads the JSON). The
  ones explained so far are in the module (NIfTI's quaternion method twice, tanh's fdlibm port,
  sincos and the spec writer's model).
- **The private dossier's manifest** (SHA-256 of every transcript used, matching `SHA256SUMS`:
  `scan-run6/manifest.json` has them) and each restricted access verbatim with its record.
- Then relay 5 (sub-tasks 5 and 6), as planned below.

## For relay 3 (sub-task 3: similarity, figures only) (done; kept for the record)

- Pairs: each clean-room original (by tool, `verdicts.tool_of`'s files) against its upstream source
  (FSL: `crates-nc/fsl-source/` (6.0.7.23) or `~/fsl-*/src`; AFNI: `<workspace>/tags/AFNI-26.3.05`
  (or 25.2.09) `src/`; glibc: none in the workspace: skip or say so) and its replica
  (`crates-nc/larmorx-nc-*`, `crates/larmorx-afni-replica`). Relay 3 reads source by code only (it may:
  validation role), never into committed files beyond figures.
- Measure: token n-grams (n = 5..8) after normalising identifiers and literals (Rust and C/C++
  lexed simply: identifiers -> ID, numbers -> NUM, strings -> STR; comments dropped), Jaccard and
  containment per file pair and per tool; longest common token run. Report figures; no threshold.
- Inputs for the per-tool file lists: `verdicts.json` (`tools`, the touched files), or `tool_of`
  over `git ls-files crates/larmorx-{afni,mri,fsl}`.
- Then relay 4 (sub-task 4: the public summary `docs/validation/clean-room-audit.md` generated from
  `verdicts.json` + similarity, the dossier's manifest with SHA-256, remaining tests) and relay 5
  (sub-tasks 5 and 6: the merge step: archive the part's transcripts, `scan` on them, `verdicts`,
  stop on a `review` status with findings not in the judged list; HANDOFF.toml/.md; final report).
  For the merge step: a judged-findings list (e.g. `archive/clean-room-audit/<date>/verdicts/
  judged.json` of (agent, call) pairs) lets a re-run say "no new finding".

## For relay 2 (heads-up from relay 1's checks, not verdicts)

- Output: `run5/agents.json` (per agent: meta with description/worktreePath/worktreeBranch/
  toolUseId/parentAgentId, `brief` = its first prompt, `own_scratch`, `launched_by`, commits
  count), `run5/access/<session>__<agent>.jsonl.gz` (one line per access: ts, file, line, call,
  tool, op, via, detail, path, area, rel, checkout, show, category, rule, family, in_force, named,
  refused, unresolved, lines/hits, writers for R18, part for R13, the full `command` for restricted
  ones), `run5/commits.jsonl`, `run5/scratch_tasks.json` (scratch folder -> [agent, first write]),
  `run5/manifest.json`.
- The categories to weigh: `restricted` (content shown or copied) vs `restricted-names` (names),
  `restricted-build`/`-exec` (compiling or running the replica), `replica-build-output`,
  `replica-metadata`, `restricted-mention`; `search` is a scope (content only through
  `search-hit` lines of the same call).
- R18 (other scratch) and R13 (docs/parts) need the writer's role: e.g. FLIRT clean-room part 2's
  relays read part 1's (clean-room) scratch notes; BET clean-room parts 2a/2b read docs/parts (their
  own part folder?). R12 (the register) needs the line ranges per version (1,738 accesses).
- Real-looking hits seen while checking the parser: FLIRT clean-room part 1 (a083655161a925b75)
  and the clean-room catch-up relay 1 (abec7fbbff23f3191) printed lines of
  `crates/larmorx-afni-replica` by `grep -rn ... crates/` (AFNI family: not FLIRT's, but against
  "searching is opening"); "AFNI-26 in clean-room original" (a0313b...) 4 hits in R10; "Clean-room
  additions FSL and AFNI-75" 1 R10 hit. Running the replica binary (`larmorx-nc`) as a black box is
  `replica-build-output` with op `exec` (topup relay 3b, catch-up relay 2): decide whether allowed.
- R15/R16 (replica cases, brief-named modules) are not in the table's paths yet: from the briefs.
- The register (R12): accesses carry `lines` (Read ranges, `sed -n a,bp`, `head`, `tail`) and `hits`
  (grep -n line numbers); a grep without -n leaves only the content: relay 2 compares output lines
  with the restricted sections at that version (the output is not kept: re-read the call from the
  transcript by `file`/`line`).

## The restriction table (derived from the rules; written into `restrictions.py`)

Dates: when the restriction began to apply. Two dates where they differ: the date the rules first
named the item (main's first-parent history of CLAUDE.md / porting-guide.md / licensing.md) and the
date its content became restricted by nature (the file's first commit, any branch). The tool reports
an access before the naming date as "restricted (before the rule named it)": a read of
source-derived content is a contamination whatever the rule said that day.

Strict clean-room for originals: decision log 2026-10-09 ("Replica and original for every restricted
upstream", superseding the same day's "read, never translate"); CLAUDE.md rule 1 from 8d332534
(2026-10-09 23:26). Families: FSL, AFNI, FreeSurfer, Workbench, MSM, GNU (glibc, libstdc++), tedana;
plus "all" (restricted for every clean-room original).

| id | what | where (logical anchors) | family | since |
|---|---|---|---|---|
| R01 | FSL's source | `crates-nc/fsl-source/` (from 2026-10-24); `<FSLDIR>/src/`, `<FSLDIR>/include/`, `<FSLDIR>/pkgs/`; reading (not running) `<FSLDIR>/bin/*` scripts and `<FSLDIR>/lib/python*/site-packages/fsl*`; `oracles/fsl-*/` source builds; `archive/*fsl-source*`; git.fmrib.ox.ac.uk | FSL | 2026-10-09 |
| R02 | AFNI's source (MCW and NIH alike; the clean-room originals stay clean after the CC BY 4.0 move of 2026-10-29) | `<ws>/afni/` (not `afni/doc/`), `tags/AFNI-*`, `oracles/afni-*/build/`, reading `oracles/afni-*/bin/*` scripts, `archive/*afni-replica*`; github.com/afni/afni source views | AFNI | 2026-10-09 |
| R03 | FreeSurfer's source | `<ws>/freesurfer/`, `tags/freesurfer*`, reading `/usr/local/freesurfer/` scripts and python | FreeSurfer | 2026-10-09 (rule 3; clean-room originals planned 2026-10-26) |
| R04 | Workbench's source | `reference_src/workbench/` | Workbench | 2026-10-09 |
| R05 | MSM_HOCR's source | `reference_src/MSM_HOCR/` | MSM | 2026-10-09 |
| R06 | glibc's and libstdc++'s source (LGPL / GPL) | `tags/glibc-*`, `/usr/include/c++/`, sourceware.org / ftp.gnu.org glibc | GNU | 2026-10-30 (`specs/cbrtf.md`, the first glibc behaviour spec, a9c584fb; rule text "never copied or translated" 2026-11-04, 8a11aa02) |
| R07 | tedana's source (LGPL-2.1) | `<ws>/tedana/` | tedana | 2026-10-09 (licensing.md: original from the published equations; t2smap's clean-room planned 2026-11-06) |
| R10 | the replica crates | `crates-nc/` (FSL; created 2026-10-24), `crates-gpl/` (AFNI MCW replicas until 2026-10-29, then the glibc tools, Workbench; created 2026-10-10), `crates/larmorx-afni-replica/` (AFNI; created 2026-10-29 23:28, named 2026-10-30 01:29, 05893577); `larmorx-nc-*`, `larmorx-gpl-*` anywhere (wheels, site-packages) | FSL / AFNI+GNU+Workbench / AFNI | 2026-10-09 (rule 1) |
| R11 | source-derived findings | `docs/findings/afni-tshift.md` (named 2026-10-10 01:07, 222a15bc; file 2026-10-09 20:20), `fsl-mcflirt.md` (2026-10-11 17:00, 9cd70709; file 2026-10-10), `afni-automask.md`, `afni-unifize.md` (2026-10-24 03:16, 24588722; files 2026-10-23 10:23), `afni-licence-routines.md` (2026-10-24 07:11, bd2ca21d), `fsl-bet.md` (2026-10-26 05:13, 95321461; file 2026-10-24 19:01), `fsl-flirt.md` (2026-11-01 14:59, f489dd31; file 12:37), `fsl-fast.md` (2026-11-02 23:10, d704c32d; file 20:25), `fsl-topup.md` (2026-11-03 07:38, 29fba40a; file 05:19), `fsl-maths.md` (2026-11-03 16:40, 8d5f8f10; file 08:53). Any other `docs/findings/(fsl|afni)-*.md` not `*-original.md`: "unlisted, review" | per file | per file |
| R12 | the register's restricted sections | `docs/findings/upstream-defects.md`, the `## Restricted: ...` sections (line ranges at the version the agent had; checked by line numbers and by content of the output) | AFNI / FSL per section | 2026-10-11 17:00 (9cd70709); searching the whole file named a violation 2026-10-30 (d526f107) |
| R13 | other parts' briefs, reports, handoffs | `docs/parts/` (an agent's own part folder excepted) | all | 2026-10-23 19:28 (c6ff930b; porting guide 2026-10-24) |
| R14 | probes (source-derived) | `validation/probes/` (family by folder: `fsl-*`, `bet-options`, `standard-space-roi` FSL; `glibc-double-maths`, `libstdcxx-sort` GNU; `ants-ai-blobs`, `nlohmann-to-chars` permissive) | per folder | the audit brief 2026-11-06; content from 2026-10-23 (d901a1a0) |
| R15 | replica authors' permanent cases (a clean-room part's own named in its brief) | `validation/difftest/cases/<replica tool>`: to settle in relay 2 from the briefs | per tool | 2026-10-30 (d526f107) |
| R16 | replicas' suite and evidence modules where a brief says so | from the briefs (relay 2) | per tool | per brief |
| R17 | session transcripts (they hold every upstream source an agent read) | `~/.claude/projects/`, `archive/*session-transcripts*` | all | 2026-10-09 (by content; no rule names them) |
| R18 | other agents' scratch notes and archived scratch | `<scratch root>/<other task>/` (not the agent's own), `archive/*scratch*` | all | 2026-10-23 (as R13); before: logged |

Allowed, logged by category: black-box runs (`<FSLDIR>/bin/<program>`, the AFNI, ANTs and FreeSurfer
oracle binaries, Apptainer fMRIPrep, `-help`); upstream documentation (`<FSLDIR>/doc`, `man`,
`afni/doc/`, the upstreams' documentation sites); permissive sources (ANTs, ITK, nibabel, SciPy, numpy,
OpenBLAS, CORE-MATH, nitransforms, fMRIPrep and its libraries, Arm's routines); `specs/`; the clean-room
crates; oracle outputs and data.

## Plan (sub-tasks 1 to 6 over relays)

- **Relay 1: sub-task 1** the tool's core: discovery (archive + live, identical copies merged by SHA-256,
  differing copies merged by tool-use id), streaming parse, per agent and orchestrator the access log
  (Read/Grep/Glob/LS/Write/Edit/MultiEdit/NotebookEdit/WebFetch, Bash parsed: quotes, heredocs,
  `$(...)`, variables, `for` lists, `cd`/`pushd`/subshells, `git -C`, the verbs' option grammars,
  redirections), output parsing (grep hits, `diff --git` headers, rustc `-->` diagnostics, commit
  hashes printed by `git commit`), persisted tool results, compaction boundaries, the restriction
  table with dates; tests on synthetic transcripts; one boxed run; counts and gaps here.
- **Relay 2: sub-task 2** the verdicts: git (the three repositories): the commits touching each
  clean-room original (crates `larmorx-afni`, `-mri`, `-fsl`, `larmorx-core`'s `*_glibc` from specs,
  any other found by SPDX headers / PROVENANCE), by worktree branch and by printed hash -> agent;
  roles from the briefs (first message, `docs/parts/<part>/brief.md`); R15/R16 from the briefs; the
  register's line ranges per version (R12) checked against the logged outputs; per tool and agent the
  restricted accesses before or during the work; declared slips (NOTES' "Clean-room statements",
  `docs/parts/*/notes.md`) against found ones; the orchestrator's edits to clean-room crates (git:
  first-parent non-merge commits and merge results differing from both parents).
- **Relay 3: sub-task 3** similarity (token n-grams after normalising identifiers and literals; longest
  common token runs) per clean-room original against its upstream source and its replica; figures only.
- **Relay 4: sub-task 4** the outputs: the private dossier (`<workspace>/archive/clean-room-audit/<date>/`:
  access logs, restricted accesses verbatim with session/line/time, manifest with SHA-256 matching
  `SHA256SUMS`), the generated public summary `docs/validation/clean-room-audit.md`, the remaining tests.
- **Relay 5: sub-tasks 5 and 6** the merge step (`scripts/orchestration/`: archive the part's
  transcripts with SHA-256, run the audit, stop the merge on an edit of a clean-room crate by an agent
  with a restricted access for that tool), HANDOFF.toml / HANDOFF.md, the part's final report.
