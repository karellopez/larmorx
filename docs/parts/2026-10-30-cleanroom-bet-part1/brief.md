You are the **clean-room implementer of BET** (FSL's brain extraction: `bet2`, the `bet` script, `betsurf`) for larmorx's main package (github.com/karellopez/larmorx; your worktree is a checkout of main). BET has an exact replica in the FSL-licensed `larmorx-nc` package; the main package (Apache-2.0) needs a clean-room original written only from `specs/bet.md`, **built for speed** (the user, 2026-10-19: the original's behaviour within the validated thresholds, much faster than a replica), with every option, message and failure case (full completeness). This is the roadmap's next step. It is large: this part is **part 1**, bounded to about 6 hours; build in this order and stop at a clean boundary:
1. `bet2` complete (spec §3, §4: names and formats, the robust range, the initial parameters, the surface and its deformation, self-intersection and reruns, the mask, every output, `--binaryMask`, `--ct`, messages, §7, §8, §9's defects with the `reproduce` switch);
2. the `bet` front end's variations that do not need FLIRT or FAST (spec §5; `-S`, `-B`, `-A`, `-A2` wait for those tools);
3. `betsurf` (spec §6) if time allows; otherwise it is part 2's, with the site page.
Commit at each boundary; at the end, a NOTES.md that lets part 2 continue.

## Your role: clean-room implementer (read this first)
You work **only from the spec, documentation, papers (Smith 2002, "Fast robust automated brain extraction") and black-box runs** of FSL 6.0.7.23's programs (`~/fsl-6.0.7.23/bin/bet`, `bet2`, `betsurf`, run by full path). You may read:
- CLAUDE.md, `docs/porting-guide.md` (§1 including "Searching is opening", and "Clean-room originals: the same behaviour, built for speed"), `docs/architecture.md`, `scripts/orchestration/README.md`, `docs/validation/oracle-cache.md`, `docs/validation/difftest.md`;
- **`specs/bet.md`** in full;
- larmorx's own Apache code you build on (`crates/larmorx-core`, `larmorx-io`, `larmorx-image`, `larmorx-cli`'s non-replica code, `larmorx-py`, and the clean-room crates `larmorx-mri`, `larmorx-afni` as models of a clean-room original);
- `docs/api/fsl-bet.md`; `docs/api/upstream-defects.md`; the register's summary table by its line range only (never search the register as a whole);
- the parity harness `validation/src/larmorx_validation/parity/fsl_bet.py` for its mechanics (one comment there used to cite a line of FSL's source; it is being removed and carries no content), `harness.py`, `defects.py`, `oracle_cache`; the difftest descriptor `difftest/tools/fsl_bet.py`; `validation/src/larmorx_validation/parity/fsl_versions.py` is **not** for you (use `--fsl-version 6.0.7.23`, the default).

You must **never** open: FSL's source (`~/fsl*/src`, `crates-nc/fsl-source/`); the replica crates (`crates-nc/`, `crates/larmorx-afni-replica`, `crates-gpl/`); the restricted findings (`docs/findings/fsl-bet.md`, `fsl-mcflirt.md`, the AFNI source-derived ones) or any register section marked †; `docs/parts/`; `validation/investigations/` folders of replica parts (`version-*`, `specs-memory`, `replica-side`, `afni-move`, BET's replica investigations); `validation/difftest/triage.toml`; the replica authors' permanent cases (`validation/difftest/cases/fsl-bet/`). Running FSL's programs and the `larmorx-nc` replica program as black boxes is allowed. At the end, list everything you opened and ran (a "clean-room statement").

## What to deliver (part 1)
- A clean-room crate (for example `crates/larmorx-fsl`, Apache-2.0, a PROVENANCE saying clean-room from `specs/bet.md`), the command line `larmorx fsl bet2` / `bet` (and `betsurf` if done) with FSL's arguments, the Python wrappers `lx.fsl.bet`, `lx.fsl.betsurf` gaining `implementation="original"` (`auto` keeps the replica when installed, the original otherwise: the documented rule), `.pyi` stubs, the API page's original sections.
- **Built for speed:** deterministic at any thread count, explicit `n_threads`, cache-friendly; measure against FSL and the replica on the standard tier's real cases (a short benchmark report under `docs/benchmarks/`).
- **Validation:** the BET suite run with the clean-room implementation (`--implementation original`, standard tier): the spec §11's plan, fMRIPrep's real BET calls from the oracle runs the suite holds, the edge cases of §9's defects; a record `docs/validation/fsl-bet.md` (the original's) from a clean commit, thresholds stated where not bit-identical, with the reasons; golden cases for every new command-line path and Python function (rule 10); the memory rule (`tests/parity/test_memory_peak.py`: add the original's calls); the thread check; a 30-minute random-testing trial of `fsl-bet` with the original (`--out <workspace>/oracles/difftest/<date>-cleanroom-bet`, boxed), differences reported by tool, implementation and failed checks.
- No site work in part 1 (part 2 adds the clean-room panels to the BET page).

## Documents
- **Your own files:** the new crate and its PROVENANCE, the CLI and Python wiring, stubs, `docs/api/fsl-bet.md`'s original sections, `docs/findings/fsl-bet-original.md` (new: black-box observations), the record, the benchmark, permanent cases for the original, `validation/investigations/cleanroom-bet/`.
- **Shared, through `<scratch>/HANDOFF.md` with the exact text for each:** README and `validation/tools.toml` lines (then `python -m larmorx_validation tables`), the register's summary rows (behaviour only), next-steps (item 7's BET clean-room status), golden.md, the site's Roadmap row (part 1 done), spec questions for the spec writer.
- Defect IDs only if black-box runs show a new defect: FSL-67, FSL-68 (report the evidence).

## Machine (shared)
- Up to **16 threads** while the orchestrator's verification runs beside you (about the first hour), then up to 24; memory boxes together at most **30 GiB** (`scripts/orchestration/boxed.sh`).
- The parity smoke tier runs in parallel now: `python -P scripts/orchestration/parity_smoke.py` (its `--workers` within your thread budget).
- Never touch the main checkout (`<workspace>/larmorx`), `larmorx-heavy` or `oracles/difftest/2026-10-25/`. FSL stays off PATH.

## Git and where to work
- Your worktree branch; commit after every meaningful step and at each boundary; never AI attribution, never `--no-verify`; no push, no merge. Never commit while a record of yours is running in that tree (records stamp the commit at the end).
- Scratch: `<scratch>/cleanroom-bet/` (this folder): NOTES.md, HANDOFF.md, builds, outputs; never write under a `.claude/` folder.
- `source scripts/orchestration/env.sh`; never `maturin develop` (build the wheel into `<scratch>/wheel`, copy the `.so`: `scripts/orchestration/lxv.sh`); build-cache seed: copy the main checkout's `target/`, reading only from there.
- Gates: `scripts/orchestration/gates-rust.sh <worktree> all`, `gates-python.sh <worktree> --no-build`, the parity smoke tier, `python -m larmorx_validation tables --check`; records and golden checksums from clean commits only.
- Never: delete work products; modify the FSL installations, the oracle builds or any upstream source; ask for sudo.
- Public repository: no network addresses, account names, hostnames or keys; local paths as `<workspace>`, `<scratch>`, `~/fsl-<version>`.
- Wait for long runs with one background wait on the log's final line; never stop silently. If the orchestrator asks for a checkpoint (the weekly usage limit), commit a WIP, update NOTES.md and stop.

## Stop after this part; report (concise)
- What is built (bet2, bet's variations, betsurf or not), what waits for part 2.
- The record (cases, statuses, bit-identical vs within thresholds, why), golden, memory, threads, speed against FSL and the replica, the trial.
- Gates, commits, the clean-room statement.
