## The AFNI replicas move into the main package (next-steps item 3c, 2026-10-29 and 30): report

The three replicas (3dTshift, 3dAutomask, 3dUnifize) are `crates/larmorx-afni-replica` and run
inside larmorx; the gates, the records and the golden tests pass. glibc's `cbrtf` changes 3dUnifize
`-EPI` for some voxel sizes, so that decision goes to the user. Branch
`worktree-agent-ac71aae39a4bfa297`, 14 commits from 57ab243 to e55138c; site branch `afni-move`
(d6c1d24); the handoff is `handoff.md` here. No defect IDs used.

### 1. The C library's maths
- **`sinf`/`cosf`:** translated afresh from Arm Optimized Routines v19.11 (MIT) into
  `src/sincosf.rs`, both of glibc's x86-64 builds (FMA, SSE2); `crates-gpl/tools/sincosf-verify`
  checked all 2^32 inputs of each function and build: 0 differ from the earlier glibc translation,
  from glibc 2.35's FMA build and from its SSE2 build (forced with `GLIBC_TUNABLES`); the two
  builds differ from each other on 12 and 22 inputs, so the check sees which one runs.
- **`tanhf`/`expm1f`:** `fdlibm.rs` (Sun's notice); `tanhf` 0 of 2^32 inputs differ from glibc.
- **`cbrtf` (rule D): its last bit matters.** `crates-gpl/tools/cbrtf-measure` covered every
  non-negative float32 voxel volume:

| | all volumes | from 0.001 mm³ | 1 to 100 mm³ |
|---|---|---|---|
| ball of neighbours differs | 1,449,010 | 8,794 | 80 |
| printed radius differs | 65,145,681 | 552 | 43 |

  The two cube roots are always at most 1 ulp apart (e.g. at 11.97 mm³ glibc gives radius 8,
  2,109 offsets; the correctly rounded one 7.9999995, 2,103). The replica uses CORE-MATH's `cbrtf`
  for now and warns, with a line ending `(larmorx)`, wherever 1 ulp could change the ball or the
  message (917,292,437 volumes in all, 2,953 in 1 to 100 mm³; every volume that really changes is
  among them, 0 missed). Recommended to the user: a clean-room `cbrtf` with glibc's bits (a spec
  of the method, written by an agent that never read glibc, proved on all 2^32 inputs; a small
  part); alternatives: a table of the 9,346 changed volumes from 0.001 mm³, or keeping the warning.

### 2. The move
- Headers per file: `Apache-2.0 AND CC-BY-4.0` for the 30 MCW-derived files (CC BY 4.0's
  attribution in each header), `Apache-2.0` for the NIH-derived files and larmorx's own (AFNI's
  request to cite kept), `Apache-2.0 AND MIT` for Arm's, `Apache-2.0 AND SunPro` for fdlibm's. The
  attribution also in `PROVENANCE.md` and the root `NOTICE`; `LICENSES/` holds CC BY 4.0's legal
  code and Arm's MIT text, shipped in the wheel. `scripts/check_spdx.py` allows CC BY 4.0 only in
  this crate and requires it where a header names MCW code. The code written for `larmorx-gpl` is
  relicensed to Apache-2.0 by its author.
- In-process: the command line runs the replica through `larmorx_cli::replica`
  (`Location::Builtin`); Python calls `_core.replica_run_builtin` without the GIL, with an explicit
  environment table and working directory (never the process environment). `auto` keeps its rule
  and now always runs the replica. The `larmorx-gpl` program, its wheel and the `[exact]` extra
  are removed (git history keeps them).
- `crates-gpl/`: no package code; README, LICENSE and NOTICE (reserved for Connectome
  Workbench) and the GPL verification tools (`glibc-reference`, `sincosf-verify`,
  `floatmath-verify`, `cbrtf-measure`).
- CI: the six `gpl` jobs stay (they test `crates-nc`); their `crates-gpl` steps are removed (4.0
  min per run); the replica's tests join the `test` jobs; net saving about 1 to 3 min per run.

### 3. Validation
- Records: the three replica records re-made in-process from the clean b6c7066; compare_records
  against main's: 0 status and 0 detail changes in 315 + 125 + 116 cases (a first set stamped
  `-dirty` was redone).
- Golden: 40 cases and 90 outputs moved to `crates/larmorx-cli/tests/afni_replica_golden.rs`, the
  checksums unchanged, passing through the in-process replica.
- Threads: 77 passed. Parity smoke tier: 2,584 passed, 3 strict expected failures (memory).
- Memory: the move adds nothing (within 1.2 MiB of the old program). Two calls exceed the rule, as
  before the move, now strict expected failures: the replica's 3dUnifize (1.5 × AFNI with `-T2`,
  2.8 × with `-EPI`), and 3dAutomask on a small image through `python -m larmorx` (Python and numpy
  add about 13 MiB; the replica alone 8.2 MiB against AFNI's 7.5).
- Random-testing trial (30 min, from 6ee7765): 13,971 cases, no difference (only known notices,
  AFNI-20's identifier, 14 `-EPI` heavy cases); two earlier attempts set aside, not deleted.
- Speed (original time ÷ replica time, 1 / 8 threads): 3dTshift 0.95 / 0.72, 3dAutomask 0.54 /
  0.44, 3dUnifize 0.47 / 0.27: the clean-room originals are faster.

### 4. Site
The three AFNI pages and the Validation page say the replica is in `pip install larmorx`
(Apache-2.0 with CC BY 4.0's attribution, in-process); evidence re-exported from the cache;
inventory: no loss (the changed evidence files only renamed the replica, reworded two reasons and
updated provenance); number check 174, 156, 199, 361; build 0 problems.

### 5. Gates
Rust (three workspaces), Python 624 passed, ruff, SPDX, `tables --check`; parity smoke 2,584
passed and 3 strict expected failures.

### For the user
1. The `cbrtf` decision (recommended: a clean-room `cbrtf` with glibc's bits).
2. Whether `auto` should keep choosing the slower replica (the speed table).
3. The memory follow-ups (the replica's 3dUnifize memory; a lazy numpy import for the Python
   command line).

### At the merge (orchestrator)
The handoff integrated: CLAUDE.md (rule 1's tracks and exception, rule 2, the AFNI row, Next, the
repo map), licensing (rewritten for AFNI, at the file's actual wording), overview, README,
UPSTREAM.md, the porting guide (the table and the clean-room's forbidden list), next-steps (3c done,
follow-ups, two items for the user), the decision log, golden.md, `validation/tools.toml` and the
generated tables.
