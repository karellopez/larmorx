# Orchestration scripts

How larmorx's work is checked before it reaches `main`, as scripts: the merge checklist of
`CLAUDE.md` ("Merging an agent's work") and the helpers the orchestrator uses to integrate an
agent's part. They make the checks reproducible on any machine that has the workspace
(`scripts/bootstrap_workspace.sh`), the oracles (`scripts/build_*_oracle.sh`) and the test data
(`larmorx-testdata`, github.com/karellopez/larmorx-testdata). Nothing here runs in CI.

`env.sh` sets the environment the others share: `LARMORX_WORKSPACE` (default: the parent of this
repository), `LARMORX_VENV` (default: `<workspace>/.venv`), cargo first on the path, FSL off it.

| Script | What it does |
|---|---|
| `gates-rust.sh <checkout> [root\|gpl\|both]` | fmt, clippy `-D warnings`, tests, in the root workspace and `crates-gpl` |
| `gates-python.sh <checkout> [--no-build]` | `maturin develop --release` (main checkout only), `pytest tests/python tests/golden`, ruff, SPDX |
| `build-gpl.sh <checkout>` | the `larmorx-gpl` replica program (release) |
| `lxv.sh <checkout> <args>` | `python -m larmorx_validation <args>` from that checkout |
| `verify-merge.sh <checkout> <out> [--suite "..."]... [--site <dir> --evidence <tool>...] [--golden] [--threads N] [--oracle-cache use\|refresh\|off]` | the checklist's steps 1-5 on a committed merge, one line per step in `<out>/main.log`; the parity runs use the oracle-output cache (below) |
| `compare_records.py <out>/records <suite>...` | the re-run records against the committed ones, case by case |
| `merge_golden.py` | a conflict in `tests/golden/golden.json` resolved case by case (then re-record) |
| `register_counts.py` | the defect register recounted from its table; every **decide** and **yes** listed |
| `handoff_blocks.py <HANDOFF.md>` | an agent's handoff: each text block with its instruction |
| `watch_ci.py <sha>`, `watch_pages.py <sha>` | wait for the six-platform CI run, or the site's deployment |

## Merging an agent's part

1. **Clean tree, merge without committing:** `git merge --no-ff --no-commit <branch>`; check
   `.git/MERGE_HEAD`. A golden conflict: `python -P scripts/orchestration/merge_golden.py`.
2. **Integrate the handoff** (`<agent scratch>/HANDOFF.md`, `handoff_blocks.py`) into the shared
   documents, and **harmonize** them: the register's table, sections, history and counts
   (`register_counts.py`: every **decide** listed under "Waiting for the user"), its API page,
   README, the validation index, golden.md, licensing, next-steps, CLAUDE.md "Where things are".
   Commit the merge.
3. **Verify the committed merge:** for example
   ```bash
   scripts/orchestration/verify-merge.sh . /tmp/verify --suite "ants-ai --jobs 6" \
       --site /tmp/site-check --evidence ants-ai --golden
   python -P scripts/orchestration/compare_records.py /tmp/verify/records ants-ai
   ```
   where `/tmp/site-check` is a clean checkout of the agent's site branch
   (`git -C ../larmorx_documentation worktree add /tmp/site-check <branch>`). Every status must
   match the agent's record; a re-export must change nothing but the exporter's fingerprint or
   links that the integrated documents now resolve.
   **The oracle-output cache** ([docs/validation/oracle-cache.md](../../docs/validation/oracle-cache.md))
   makes this re-run cheap: the agent's runs of the original programs (the same builds, inputs,
   arguments and settings) are served from `<workspace>/oracles/cache`, and only larmorx runs;
   each record's header says how many runs came from the cache. The runs the original varies
   (the clock, races, thread bands of programs that vary) still run. When an oracle was rebuilt,
   its key changed and its runs run again by themselves; to check the cache itself, re-run with
   `--oracle-cache refresh` (every cacheable run again, compared: a difference fails) or run
   `lxv.sh . oracle-cache verify --sample 20 --jobs 4`. `record_golden.py` (`--golden`) runs the
   smoke tier only when it has not passed for the same clean commit and build already: a passing
   `pytest tests/parity` leaves a marker in the cache, which it reuses.
4. **The site:** merge the site branch, integrate the handoff's site texts (status table,
   navigation, Validation page), re-export the summary (`lxv.sh . evidence summary --site
   ../larmorx_documentation`), build (`tools/build.py`: 0 problems), review screenshots.
5. **Publish:** check for AI attribution, push both, `watch_ci.py <sha>`, `watch_pages.py <sha>`;
   when green, delete merged worktrees' build caches except the newest.

## Agents' parts

Each part's brief, final report and handoff are kept in [docs/parts/](../../docs/parts/), so how a
tool was specified and what its agent found stays with the code. Tools that produced a finding or a
record are committed (`validation/investigations/`, `validation/probes/`), never left in a
temporary folder.
