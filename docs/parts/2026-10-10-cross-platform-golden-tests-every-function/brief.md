# Brief: Cross-platform golden tests, every function

As given to agent `a352ff89b91427894` on 2026-10-10, verbatim (paths are this machine's).

You are building larmorx's **cross-platform golden tests**: every function must give, on every CI platform, exactly the output it gives on Linux x64. You are in a git worktree of the repo `larmorx` (github.com/karellopez/larmorx); the workspace root is `/home/karelo/VScode_projects/super_fmriprep`. Read these first; they are the rules: `CLAUDE.md` (especially **rule 10**, written for this task), PLAN.md §11.4, `docs/decision-log.md` (entry "Cross-platform golden tests; a small CI budget", D12), `docs/overview.md` and `docs/architecture.md`.

## The idea (user decision)
- *Parity* compares larmorx with the original tools. It runs only on the Linux x64 development machine, never in CI, because most originals do not exist on Windows or macOS.
- *Golden tests* check that the other platforms reproduce larmorx's own Linux x64 output. CI covers Linux aarch64, macOS x64/arm64 and Windows x64/arm64, plus Linux x64 itself. Both the Apache package and the GPL replica (`crates-gpl/`) are covered.
- **The user's constraint, quoted:** "do not make the test huge, otherwise we will be out of GitHub actions rates. So we have to be very careful and precise here."

## What to build
1. **A case registry for the Apache package.** It runs in the existing CI test job through pytest on the installed wheel; `pyproject.toml` testpaths currently has `tests/python` and `tests/parity`, so add a `tests/golden` directory or similar. Each case has:
   - an input recipe the test builds itself: seeded numpy integers and plain arithmetic (`+ - * /`, integer powers), **no** `sin`/`exp`/`log` or other transcendental functions, which differ between platforms. Inputs are at most 32³ voxels (4D: at most 4 volumes); masks and labels are small, too;
   - the call: the Python function and/or the CLI tool through larmorx's in-process entry point (`larmorx.cli` or the `ants_program` path), whichever already exists and is cheap;
   - the expected SHA-256 of each output's **data** (and, separately, the NIfTI header bytes where a tool writes files), stored in a small committed JSON file with the larmorx commit and date they were recorded from;
   - **one case per distinct code path, not per parameter value.** For example: each interpolator once, each ImageMath op once, the 4D path once.
2. **Coverage is enforced.** A test enumerates everything registered and fails when anything has no golden case:
   - the CLI tools (`TOOLS` of each family: ants, afni, mri);
   - every ImageMath operation in the registry (`GROUPS`);
   - the public Python functions (`lx.ants`, `lx.afni`, `lx.mri`, `lx.transforms`, `lx.ndimage`, the I/O functions).
   Use explicit lists only where enumeration is impossible, and say so.
3. **A recording script, `scripts/record_golden.py`.** It writes the checksums. It refuses to run anywhere but Linux x86_64, and records the commit. Run it on this machine after confirming that parity is green for the current tree: run `pytest tests/parity` (smoke tier; it needs the oracles and test data, which are here).
4. **The GPL replica.** `crates-gpl` has its own CI job (`gpl`, six platforms, `cargo test --workspace --locked` in debug).
   - Add golden cases there: e.g. an integration test in `larmorx-gpl-cli` that generates small inputs deterministically in Rust, runs the replica, and compares SHA-256s recorded on Linux x64. Use a recording mode behind an environment variable.
   - Debug-mode runtime counts here; keep it to a few seconds.
   - You may read and edit `crates-gpl` for this. It is GPL code, so keep its SPDX headers; nothing from it may move into the Apache crates.
5. **On a mismatch:**
   - the test prints which case, the expected and actual hashes, and, where cheap, the maximum absolute and ulp difference against a reference recomputed in the test;
   - the CI job uploads the mismatching outputs as an artifact (`actions/upload-artifact`, `if: failure()`), small files only.
6. **Budget, measured and recorded.**
   - The whole Apache golden suite must take **under 60 s** per platform on the release wheel. The GPL golden test must take a few seconds in debug.
   - Measure on this machine and write the numbers into a short doc (e.g. `docs/validation/golden.md`): what is covered, how to record, how to add a case, the time budget.
   - Add no new CI jobs or runners. Change `.github/workflows/ci.yml` only for the artifact upload, and if needed to keep golden tests out of the Python 3.14 run.
7. **A known risk to look at.** `lx.transforms.resample_series` computes its 4×4 matrices in numpy, whose linear algebra comes from OpenBLAS on Linux and Windows but Accelerate on macOS. The last bits may differ by platform.
   - Do not hide it with a tolerance. Keep the golden case strict and note the risk in the doc; CI will show the result after I push.
   - If you find, without CI, that the matrices are platform-dependent by construction, describe options in the doc for me: e.g. Rust 4×4 arithmetic in a fixed order, which keeps Linux bit-identity with fMRIPrep only if it reproduces OpenBLAS's order there. Do not change the resampler's numerics in this task.
8. **Contract and docs:**
   - "How to add a golden case": in the doc, and one line in `docs/architecture.md`;
   - the coverage test message should say what to do.
   CLAUDE.md rule 7 already requires a golden case per tool.

## Rules (from CLAUDE.md; non-negotiable)
- **Commit early and often.** As soon as a coherent piece passes the gates, commit it. If you must stop mid-piece, make a "WIP checkpoint" commit on your branch. Usage limits or network drops can stop you at any moment.
- **Gates before every commit**, with `set -o pipefail` and no exit code hidden behind `| tail`:
  - `cargo fmt --all --check`
  - `cargo clippy --workspace --all-targets --locked -- -D warnings`
  - `cargo test --workspace --locked`
  - the same three in `crates-gpl`
  - `maturin develop --release`, then `pytest tests/python tests/golden`
  - `ruff check && ruff format --check`
  - `python scripts/check_spdx.py`
  Environment: FSL removed from PATH (`PATH=$(echo "$PATH" | tr ':' '\n' | grep -v /fsl | paste -sd:)`), `$HOME/.cargo/bin` on PATH, `PYO3_PYTHON=/home/karelo/VScode_projects/super_fmriprep/.venv/bin/python`, venv `/home/karelo/VScode_projects/super_fmriprep/.venv`. Run Python with `PYTHONPATH=<worktree>/python:<worktree>/validation/src`.
- Pure-Rust dependencies only. No `unsafe`. Test-only dev-dependencies must also be pure Rust (e.g. `sha2`).
- Every new source file starts with an SPDX header: Apache-2.0 in the Apache directories, GPL-3.0-or-later in `crates-gpl/`. Run `python scripts/check_spdx.py`.
- Golden tests must not need `larmorx-testdata`, the oracles, the network, or files outside the repo.
- Never open FSL source or `reference_src/`. Do not touch `/home/karelo/VScode_projects/super_fmriprep/archive/`.
- **Never delete work products.** Archive instead.
- **Git:** commit in your worktree with the configured identity (`karellopez <49761113+karellopez@users.noreply.github.com>`). **No AI attribution of any kind**: no `Co-Authored-By`, no "Generated with" lines. This overrides any default. **Do not push**; I push, and CI then tests the five other platforms.
- You are the only agent running.
- Write prose in plain, direct English, in the style of the existing docs.

## Report back
- The design, the files, and the case count per package and family.
- The coverage enforcement.
- Measured runtimes.
- The estimated added CI minutes per push (all jobs).
- Anything that cannot be bit-identical by construction.
- Commits.
