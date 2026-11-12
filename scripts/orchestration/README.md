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
| `gates-rust.sh <checkout> [root\|gpl\|nc\|freesurfer\|both\|all]` | fmt, clippy `-D warnings`, tests, in the root workspace, `crates-gpl`, `crates-nc` and `crates-freesurfer` (`both`: root and gpl; `all`: the four) |
| `gates-python.sh <checkout> [--no-build]` | `maturin develop --release` (main checkout only), `pytest tests/python tests/golden`, ruff, SPDX |
| `build-gpl.sh <checkout>` | the GPL workspace's verification tools (release; the AFNI replicas are in the main package since 2026-10-29) |
| `build-nc.sh <checkout>` | the `larmorx-nc` replica program (release) |
| `build-freesurfer.sh <checkout>` | the `larmorx-freesurfer` replica program (release) |
| `boxed.sh <GiB> <command>` | runs a heavy job in its own memory box (a systemd user scope, no swap): a runaway process is killed inside it and nothing else is touched; use it for every parity, random-testing, oracle, benchmark or merge-verification run |
| `lxv.sh <checkout> <args>` | `python -m larmorx_validation <args>` from that checkout |
| `verify-merge.sh <checkout> <out> [--suite "..."]... [--site <dir> --evidence <tool>...] [--golden] [--threads N] [--oracle-cache use\|refresh\|off] [--serial-parity] [--serial-suites]` | the checklist's steps 1-5 on a committed merge, one line per step in `<out>/main.log`, then `VERDICT PASSED` or `VERDICT FAILED: <the failed steps>` before `ALLDONE` (exit status 1 when any step failed: read the verdict, never only the log's tail); step 4 the smoke tier in parallel (`parity_smoke.py`, `--threads` its workers, 16 without), the `--suite`s side by side (`run-suites.sh`: their `--jobs` add up), STEP5 lines in the order given; the parity runs use the oracle-output cache (below) |
| `parity_smoke.py [--workers N] [--serial] [--junit-dir DIR] [-- <pytest args>]` | the parity smoke tier (`pytest tests/parity`) on N pytest-xdist workers (16), then the memory-peak and thread-pool files alone; the marker `record_golden.py` reuses; `--serial` one pytest run (run with `python -P`, `PYTHONPATH` set) |
| `run-suites.sh <checkout> <out> [--serial] [--main-log FILE] "<parity args>"...` | standard-tier records of several suites side by side (verify-merge's step 5), each in its own log |
| `compare_records.py <out>/records <suite>... [--against DIR]` | the re-run records against the committed ones, case by case (or against another re-run's) |
| `merge_golden.py` | a conflict in `tests/golden/golden.json` resolved case by case (then re-record) |
| `register_counts.py` | the defect register recounted from its table; every **decide** and **yes** listed |
| `handoff_blocks.py <HANDOFF.md>` | an agent's handoff: each text block with its instruction |
| `apply_handoff.py <HANDOFF.toml> [--site <dir>] [--check]` | an agent's machine-readable handoff applied to the shared documents in its order, all or nothing (`[[op]]` edit, insert, append, block; `site:` paths in `--site`); then `tables`, `register_counts.py` and a re-read |
| `cleanroom-audit-merge.sh <checkout> <part> [--agent ID]... [--notes F]... [--worktree NAME]... [--scratch NAME]... [--summary] [--similarity]` | the clean-room audit at a merge ([docs/validation/clean-room-audit.md](../../docs/validation/clean-room-audit.md)): the part's transcripts archived with their SHA-256 (`archive_transcripts.py`), the scan (boxed) and the verdicts over every transcript, then `cleanroom-audit gate`: exit 1 (stop the merge) when a clean-room original was edited after restricted material of its family was seen and `validation/cleanroom-audit/judgements.toml` does not judge it, or a commit of clean-room code has no maker; `--summary` regenerates the public page; dossier in `<workspace>/archive/clean-room-audit/<date>-<part>/` (`main.log`, ALLDONE) |
| `archive_transcripts.py --part <name> [--agent ID]... [--notes F]... [--worktree NAME]... [--scratch NAME]... [--dry-run]` | copies every transcript file Claude Code holds that the archive lacks (same path and content) into `<workspace>/archive/<date>-session-transcripts-<part>/main-machine/` with `SHA256SUMS`, `README.txt` and `part-<name>.json`; exit 1 when an agent of the part has no transcript (run with `python -P`) |
| `watch_ci.py <sha>`, `watch_pages.py <sha>` | wait for the six-platform CI run, or the site's deployment |
| `upstream_watch.py [--out DIR] [--offline] [--only NAME...]` | the upstream watch: the newest release of every upstream of `UPSTREAM.md` (git tags, PyPI, AFNI's `AFNI.version`, FSL's release manifest) and, for a newer one, which files of the crates' `PROVENANCE.md` it changed (licence-only changes marked), from `<workspace>/tags/` or `~/fsl-<version>/src`; Markdown and JSON in `<workspace>/oracles/logs/upstream-watch/`. `difftest-nightly.sh` runs it once a week |

## Merging an agent's part

1. **Clean tree, merge without committing:** `git merge --no-ff --no-commit <branch>`; check
   `.git/MERGE_HEAD`. A golden conflict: `python -P scripts/orchestration/merge_golden.py`.
2. **Integrate the handoff** (`<agent scratch>/HANDOFF.md`, `handoff_blocks.py`) into the shared
   documents, and **harmonize** them: the register's table, sections, history and counts
   (`register_counts.py`: every **decide** listed under "Waiting for the user"), its API page,
   README, the validation index, golden.md, licensing, next-steps, CLAUDE.md "Where things are" and `docs/status.md`.
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
5. **The clean-room audit:** `cleanroom-audit-merge.sh <checkout> <part> --scratch <folder>
   --summary` on the committed merge ([docs/orchestration.md](../../docs/orchestration.md), step
   5a): exit 1 stops the merge until each `STOP` line is reverted or judged in
   `validation/cleanroom-audit/judgements.toml`.
6. **Publish:** check for AI attribution, push both, `watch_ci.py <sha>`, `watch_pages.py <sha>`;
   when green, delete merged worktrees' build caches except the newest.

## Agents' parts

Each part's brief, final report and handoff are kept in [docs/parts/](../../docs/parts/), so how a
tool was specified and what its agent found stays with the code. Tools that produced a finding or a
record are committed (`validation/investigations/`, `validation/probes/`), never left in a
temporary folder.
