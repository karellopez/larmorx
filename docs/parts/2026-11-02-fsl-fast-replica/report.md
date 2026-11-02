## FAST replica: final report (relays 1 to 5, 2026-11-02)

The last relay could not save its `REPORT.md` (the harness refused the write); this is its report as
returned to the orchestrator, verbatim but for paths.

**Branch:** `worktree-agent-ac6eb2fb8c3621cf1`, from main 0bed438: 17 commits, f747b28 to da6ce74 (relay 5:
584017e, 7051a1c, ef91a76, 190cbac, da6ce74). **Site:** clone `<scratch>/site`, branch `fsl-fast-page`
97101ab; local `evidence-blobs` 6977211 with 689 blobs (the 592 earlier ones, none removed).

### What was built
- `larmorx-nc fsl fast`: every option, message, exit status and failure case, one or 2 to 8 channels,
  FSL's tissue priors registered by FLIRT's replica in the same process; `-S 9` and more refused in both
  modes (the orchestrator's decision).
- `larmorx-nc-core` gained code but changed none: glibc `expf`/`log`, `rand`, FSL's masked sums and
  percentiles, LAPACK up to 8 x 8.
- `lx.fsl.fast` and `larmorx fsl fast` (replica only), the API page, `specs/fast.md` (behaviour only),
  `docs/findings/fsl-fast.md` (restricted), PROVENANCE, the difftest descriptor, the evidence module
  `evidence/fsl_fast.py`, the site page.

### Numbers
- Record `fsl-fast-replica` (from clean bb068f1): 86 cases; 59 of 59 compared bit-identical, fMRIPrep's
  call on ds000005, ds000228 and ds005003 included (fMRIPrep's own partial-volume maps also equal, 4 of 4);
  26 rejected by both alike; 1 expected divergence (FSL-104); 0 fail.
- The core changed: mcflirt 237/237, BET 178/178, FLIRT 302/302 identical case by case to the committed
  records (`compare_records.py`), re-made into `<scratch>/records`, not committed.
- Golden: `larmorx-nc` 11 cases and 124 outputs added (now 79 cases, 440 outputs); every other checksum
  unchanged; no main-package checksum due.
- Memory: the replica peaks at 538, 117 and 127 MiB against FSL's 707, 139 and 187 MiB. Threads: the same
  results at any thread count.
- Trial: 30 minutes, 17,497 realistic cases; 16,768 agree, 725 rejected by both, 4 differed, all FSL-101,
  fixed in `reproduce` and kept as 3 permanent cases.
- New defects FSL-96 to FSL-104 (FSL-105 to 110 unused):

| Entry | What FSL does | Decision |
|---|---|---|
| FSL-96, FSL-97 | saves memory it never wrote | defined value in both modes, named |
| FSL-98, FSL-103 | wrong class statistics / wrong mask | rule A, corrected by default |
| FSL-99 | uses meaningless option values | rule B, refused by default |
| FSL-100, FSL-101 | aborts or crashes | default mode segments |
| FSL-102 | runs FLIRT through the shell, names unquoted | names passed whole |
| FSL-104 | reads past smaller channels | refused |

- FSL-98 hits an ordinary call: `fast -S 2 T1w T2w` changes 2,345,041 values by default.

### What ran
- The record ran FSL 86 times: 64 from the oracle cache, 22 run (10 never cached). The trial and replays
  used the cache in `use` mode; the page's 6 FSL runs all from the cache.
- One exception, outside the cache: relay 3's command-line probe (`validation/probes/fsl-fast-cli/probe.sh`)
  ran FSL directly, about 40 short calls, boxed; documented in its README and in the handoff §5.
- The smoke tier once (2,660 passed), each record once, the trial once. Relay 5's boxes: 8 GiB, one at a time.

### Gates
Relay 4: crates-nc fmt, clippy and tests, then pytest (672 passed). Relay 5 changed only Python: ruff,
format, SPDX and `tables --check` pass; pytest validation/tests, tests/python and tests/golden 672 passed,
12 skipped. Site: build 0 problems (127 links into the worktree checked); number check 136/136, viewer
check 195/195 over 6 examples; inventory after >= before on every page; screenshots 1440 and 390, light and
dark.

### The page (23.5 MB, budget 40 MB)
Six examples: fMRIPrep's call on ds000005 as a volume; the bias field; the priors (FSL-09's correction the
one non-zero difference); three channels; ds000228 and ds005003 as full-resolution slices (ds005003's
through the tumour). Each lists every file FAST writes; the tissue volumes compared with fMRIPrep's own
maps. The case explorer: 86 cases with verdicts and reasons.

### Handoff
`HANDOFF.toml`: 23 operations (`--check` passed on its base): the register (rows, counts, history,
restricted section), the users' defect table, triage, difftest.md, golden.md, next-steps, README,
status.md, porting-guide §1, the site's `site.yml` and roadmap. `HANDOFF.md`: the reasons, the core
change, the probe, notes for FAST's clean-room implementer. For the orchestrator in CLAUDE.md: add
`fsl-fast.md` to rule 1's list of source-derived findings; update "Next".

### Not done
The full tier (all 14 runs), the data panel and long random testing (final validation phase); `-S 9` and
more (refused, as decided); the rule-D measurements of the multi-channel initialisation and bias field;
with 5 to 8 channels, NaN results can carry other payload bits than FSL's; no benchmark report; the
clean-room original is next.

### Merge notes (the orchestrator, 2026-11-02)
- Five relay agents; each report read before the next relay was launched. Scans of the branch: 17
  commits, all by the no-reply identity, no AI attribution; no home path, account name or hostname added.
- Merged after FLIRT's clean-room part 2 (main ffc7c3f): seven conflicts, each where both parts added to
  one list (`larmorx_core::defects`, the dispatch's replica-only list and its tests, `lx.fsl`'s
  docstring, the golden registry's `REPLICA_ONLY_FUNCTIONS`, the memory tests, the README's generated
  table); resolved by keeping both: `fast` is now the one FSL program with its replica only (a test
  checks that `implementation="original"` says so).
- The handoff (`handoff.toml` here is the applied copy): four operations re-anchored on what FLIRT's
  merge had changed (the register's counts paragraph and class (c) count, FLIRT's summary in the
  site's `site.yml`, the roadmap's FAST row, now row 1); counts 313 (a 69, c 142, e 53), no **decide**
  entry. By hand: rule 1's list of source-derived findings and the porting guide's list gain
  `fsl-fast.md`; the register's restricted FSL section renamed "Restricted: FSL BET, FLIRT and FAST"
  (it held FLIRT's and FAST's sections already), its header note updated; `docs/status.md` and
  CLAUDE.md's "Next".
- The site's blob manifest conflicted (each part added its blobs): the union, 703 blobs (592 + 14 + 97),
  verified by `tools/blobs.py verify`; the `evidence-blobs` commit made from it.
- Relay 3's command-line probe ran FSL outside the oracle cache (about 40 short calls, boxed): reported
  at relay 3's end, the rule repeated in relay 4's and 5's prompts; none since.
- The agent's 55 scratch scripts not in the branch (run wrappers and probes; its findings' tools are
  committed under `validation/investigations/fsl-fast/` and `validation/probes/fsl-fast-cli/`) archived
  in `<workspace>/archive/2026-11-02-fsl-fast-replica-scratch-scripts/`.
- FAST's `-S 9` and more: refused in both modes (the orchestrator's decision; next-steps follow-up).
