You are the **clean-room implementer** of a `cbrtf` with glibc 2.35's bits, for 3dUnifize `-EPI`'s `reproduce` mode, and you make one small change to the memory test (github.com/karellopez/larmorx; your worktree is a checkout of main). The user decided on 2026-10-30 (decision log, "Three decisions by the user"): **the default keeps the correctly rounded `cbrtf`** (`larmorx_core::math::cbrtf`, CORE-MATH, the more accurate), **and `reproduce` uses a clean-room `cbrtf` with glibc 2.35's bits**, so `reproduce` gives AFNI's result. Aim for 2 to 3 hours; stop at the end of the part.

## Your role: clean-room implementer (read this first)
You work **only from the spec, documentation and black-box runs** (CLAUDE.md rule 1; `docs/porting-guide.md` §1, including "Searching is opening"). You may read:
- CLAUDE.md, `docs/porting-guide.md` (§1 and "Clean-room originals: the same behaviour, built for speed"), `docs/architecture.md`, `scripts/orchestration/README.md`;
- **`specs/cbrtf.md`** (the method as mathematics, its constants, every rounding, the special values, the validation plan) and `specs/3dUnifize.md` (its `-EPI` radius and §9, by line range, as you need);
- the block hashes of glibc's results, `validation/investigations/specs-memory/cbrtf-glibc-2.35-x86_64-blocks.json` (that one file only: black-box output);
- larmorx's own Apache code: `crates/larmorx-core` (its `math` module), the clean-room crate `crates/larmorx-afni` (3dUnifize), `crates/larmorx-py`'s AFNI wrappers; `tests/parity/test_memory_peak.py`; the harness mechanics of `validation/src/larmorx_validation/parity/afni_unifize.py`;
- the clean-room findings `docs/findings/afni-masks-original.md`; `docs/api/afni-unifize.md`; the register's summary table by its line range only (never search the register as a whole);
- in `docs/next-steps.md`: the follow-ups "3dUnifize `-EPI`'s `cbrtf`: decided by the user" and "The Python entry point's own memory: decided by the user" only.

You must **never** open: glibc's or AFNI's source; the replica crates (`crates/larmorx-afni-replica`, `crates-gpl/` with its tools, `crates-nc/`); the restricted findings or any register section marked †; `docs/parts/`; the rest of `validation/investigations/specs-memory/`, and `version-*/`, `afni-move/`; `validation/difftest/triage.toml`; the replica authors' permanent cases. Running the system's `cbrtf` (glibc 2.35, through ctypes or a tiny program that calls it) and the original programs as black boxes is allowed. At the end, list everything you opened and ran (a "clean-room statement").

## Tasks
1. **The function:** write `cbrtf` with glibc 2.35's bits from `specs/cbrtf.md`, as a function of `larmorx_core::math` (for example `cbrtf_glibc`), with a short note in the code that it is a clean-room implementation from the spec (Apache-2.0). Prove it on **all 2^32 float inputs** against the system's `cbrtf` run as a black box: the block hashes of the spec's validation plan, and an input-by-input comparison against the system's `cbrtf` (commit the verifier under `validation/investigations/cleanroom-cbrtf/` or `tools/`, and its result). NaN payloads as the spec says. Keep it fast and deterministic on every platform (rule 5: no FMA contraction unless the spec says so; golden cases for it).
2. **The clean-room 3dUnifize:** `-EPI`'s radius uses `larmorx_core::math::cbrtf` by default (unchanged) and the new function with `reproduce`. **By default, name the difference** where glibc's last bit would have changed the result (the ball of neighbours, or the printed radius): a D13 notice naming **GLIBC-05** (a new register entry, class d: glibc 2.35's `cbrtf` is not correctly rounded; the orchestrator writes the register's row from your handoff), with the radius each would give. Add GLIBC-05 to `larmorx_core::defects`. Cases in the afni-unifize suite: voxel volumes where the two radii change the ball (for example 11.97 mm³: radius 8 with glibc's, 7.9999995 correctly rounded), in both modes, against AFNI 26.3.05 (`reproduce` must give AFNI's bytes; the default its own, with the notice), and ordinary voxel sizes (no notice).
3. **The memory test** (`tests/parity/test_memory_peak.py`; the user's decision): a call through `python -m larmorx` is allowed 1.10 × the original's peak **plus the peak of `python -m larmorx --version`** measured the same way (Python's start-up and larmorx's core), in place of the fixed 8 MiB for those calls; the binary's calls keep the 8 MiB rule. Remove 3dAutomask's strict expected failure if it now passes; say the numbers.
4. Records and gates: the afni-unifize standard record re-made from a clean commit (compare_records against main's: only the new cases); golden cases for the new function and for each new path, recorded from a clean commit; `pytest tests/parity` green.

## Documents
- **Your own files:** `larmorx_core::math` (the new function), the clean-room crate, the API page `docs/api/afni-unifize.md`, `docs/findings/afni-masks-original.md`, the record, the test, `validation/investigations/cleanroom-cbrtf/` (README and verifier).
- **Shared, through `<scratch>/HANDOFF.md` with the exact text for each:** the register's summary row GLIBC-05 (behaviour only), `docs/api/upstream-defects.md`, next-steps (the two follow-ups: the cbrtf part's clean-room half done, the replica's half left for a replica author; the memory test done), golden.md, the site's Roadmap row ("A cube root with the C library's digits, for reproduce": the clean-room half done).
- The replica's 3dUnifize is **not** yours: a replica author wires the new function into it afterwards (do not open it).

## Machine (shared)
- Up to **24** threads; memory boxes together at most **36 GiB** (`scripts/orchestration/boxed.sh`; the exhaustive check included).
- Never touch the main checkout (`<workspace>/larmorx`), `larmorx-heavy` or `oracles/difftest/2026-10-25/`.

## Git and where to work
- Your worktree branch; commit after every meaningful step; never AI attribution, never `--no-verify`; no push, no merge.
- Scratch: `<scratch>/cleanroom-cbrtf/` (this folder): NOTES.md, HANDOFF.md, builds, outputs; never write under a `.claude/` folder.
- `source scripts/orchestration/env.sh`; never `maturin develop` (build the wheel into `<scratch>/wheel`, copy the `.so`: `scripts/orchestration/lxv.sh`); build-cache seed: copy the main checkout's `target/`, reading only from there.
- Gates: `scripts/orchestration/gates-rust.sh <worktree> all`, `gates-python.sh <worktree> --no-build`, `pytest tests/parity`, `python -m larmorx_validation tables --check`; records and golden checksums from clean commits only.
- Never: delete work products; modify the oracle builds or any upstream source; ask for sudo.
- Public repository: no network addresses, account names, hostnames or keys; local paths as `<workspace>` and `<scratch>`.
- Wait for long runs with one background wait on the log's final line; never stop silently.

## Stop after this part; report (concise)
- The function: the exhaustive proof's numbers (blocks, inputs, NaNs), its speed.
- The clean-room 3dUnifize: the default's notice, `reproduce` against AFNI, the cases; the record (compare_records), golden.
- The memory test: the allowance and the numbers; the expected failure removed or not.
- Gates, commits, the clean-room statement.
