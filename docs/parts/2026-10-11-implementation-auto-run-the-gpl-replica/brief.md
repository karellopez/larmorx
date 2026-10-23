# Brief: implementation="auto": run the GPL replica

As given to agent `a72e1622ee1fcbc61` on 2026-10-11, verbatim (paths are this machine's).

You are completing larmorx's two-track design for licence-restricted tools: **`implementation="auto"`**. With it, larmorx runs the exact GPL replica when it is installed, and the Apache clean-room original otherwise. 3dTshift is the first tool with both. You are in a git worktree of the repo `larmorx` (github.com/karellopez/larmorx); the workspace root is `/home/karelo/VScode_projects/super_fmriprep`. Read these first: `CLAUDE.md`, **`docs/licensing.md`** (sections "Packages" and "Choosing at run time"), `docs/overview.md`, `docs/architecture.md`, `docs/api/afni-tshift.md`, `crates-gpl/README.md`, and the code in `python/larmorx/afni/`, `crates/larmorx-cli/`, `crates/larmorx-afni/src/cli/` and `crates-gpl/larmorx-gpl-cli/`.

## Design decisions (made; implement them)
1. **The licences stay separate through process boundaries.**
   - The Apache package `larmorx` never imports, loads or links GPL code, in Rust or in Python.
   - It runs the replica as a **separate program**, the `larmorx-gpl` binary, exchanging NIfTI files in a temporary directory, exactly as `docs/licensing.md` already says for the CLI. Use this for Python too.
   - Record this decision in `docs/licensing.md` and the decision log: in-process loading of a GPL extension would be a combined work, so it is not done.
2. **Packaging the replica.**
   - A binary-only wheel `larmorx-gpl` (maturin `bindings = "bin"`), with its own `pyproject.toml` under `crates-gpl/` (e.g. `crates-gpl/larmorx-gpl-cli/pyproject.toml`).
   - Licence metadata `GPL-3.0-or-later`; the wheel carries `crates-gpl/LICENSE` and the upstream notices the GPL and LGPL parts require.
   - The main `pyproject.toml` gets an extra, `exact = ["larmorx-gpl"]`. Neither package is on PyPI yet; say so in the docs.
   - Verify locally: build both wheels, install them into a throwaway venv, and check that `pip install larmorx-gpl` puts a `larmorx-gpl` executable on the venv's scripts path.
3. **Finding the replica.** Look in this order:
   - an explicit environment variable (e.g. `LARMORX_GPL_BIN`, already used by the parity suite);
   - then the `larmorx-gpl` executable in the current Python environment's scripts directory (`sysconfig`);
   - then `PATH`.
   Do it once per call; no global state that changes behaviour silently.
4. **Python API:** `lx.afni.tshift(..., implementation="auto")`, with `"auto"`, `"replica"` or `"original"`.
   - `"auto"` uses the replica if found, else the original. `"replica"` raises a clear error, with the install command, if it is missing.
   - If the replica is found but fails, raise. Never fall back silently, because the two give different last bits.
   - The result records which implementation ran and the replica's version (e.g. a field on the result or in the image's metadata, consistent with the existing result types).
   - Typed, documented, `.pyi` updated.
5. **Command line:** `larmorx afni 3dTshift <AFNI args>` runs the replica as a separate process when it is found, and the original otherwise. That is what `docs/licensing.md` says.
   - An environment variable `LARMORX_IMPLEMENTATION=original|replica|auto` overrides, so no flag clashes with AFNI's own arguments.
   - `-verb` or the verbose output names the implementation that ran.
   - Exit codes and output files must be exactly what the chosen implementation produces.
6. **Generic, not 3dTshift-only.** Put the selection and process-running logic in one small shared module (Python and Rust), with a registry mapping tool to replica program and subcommand. Future replicas (FSL in `crates-nc/`, Workbench) then add one line. Document "how to add a replica" in `docs/architecture.md`.

## Verification
- **Parity through the Python path:** run the 3dTshift suite with `implementation="replica"` and with `"original"`. Extend the suite if needed, keeping it Apache: it calls the GPL binary only as a process. The replica must stay 217 of 217 bit-identical; the original keeps its numbers.
- **Tests that run in CI without the GPL package:**
  - the selection logic, with a stub replica executable created by the test, written to work on Windows too (e.g. a tiny Python script launched through `sys.executable`, or a `.cmd` on Windows);
  - the error messages;
  - that `"auto"` falls back to the original when nothing is found.
  Keep them tiny (CLAUDE.md rule 10). Golden coverage must still pass; add a case only if a new public function appears.
- **No CI changes** unless essential. Do not add a wheel-building job for `larmorx-gpl`; that belongs to the release workflow, later. Document the local build commands instead.

## Rules (from CLAUDE.md; non-negotiable)
- **Commit early and often**; make a "WIP checkpoint" commit if you must stop mid-piece. Usage limits or network drops can stop you at any moment.
- **Gates before every commit**, with `set -o pipefail` and no exit code hidden behind `| tail`:
  - `cargo fmt --all --check`
  - `cargo clippy --workspace --all-targets --locked -- -D warnings`
  - `cargo test --workspace --locked`
  - the same three in `crates-gpl`
  - `maturin develop --release`, then `pytest tests/python tests/golden`
  - `ruff check && ruff format --check`
  - `python scripts/check_spdx.py`
  Environment: FSL removed from PATH (`PATH=$(echo "$PATH" | tr ':' '\n' | grep -v /fsl | paste -sd:)`), `$HOME/.cargo/bin` on PATH, `PYO3_PYTHON=/home/karelo/VScode_projects/super_fmriprep/.venv/bin/python`, venv `/home/karelo/VScode_projects/super_fmriprep/.venv`. Run Python with `PYTHONPATH=<worktree>/python:<worktree>/validation/src`.
- Pure-Rust dependencies only; no `unsafe`; `spawn` only, no `shell=True`; UTF-8 I/O; paths with spaces must work.
- **Licence hygiene:**
  - no GPL code or GPL-derived text in the Apache directories;
  - SPDX headers per directory (`scripts/check_spdx.py`);
  - the Apache side may know the replica's program name and arguments, nothing more.
- If you change the dependencies of `larmorx-core`/`larmorx-io`, refresh `crates-gpl/Cargo.lock` (`cd crates-gpl && cargo metadata --offline --format-version 1 >/dev/null`).
- Upstream sources and clones are read-only. Never open FSL source or `reference_src/`, or `/home/karelo/VScode_projects/super_fmriprep/archive/`.
- **Never delete work products.** Archive instead.
- **Git:** commit in your worktree with the configured identity (`karellopez <49761113+karellopez@users.noreply.github.com>`). **No AI attribution of any kind**: no `Co-Authored-By`, no "Generated with" lines. This overrides any default. **Do not push.**
- You are the only agent running. Write prose in plain, direct English, in the style of the existing docs.

## Report back
- The design as built.
- The packaging and its local verification.
- Parity through both paths.
- The tests.
- Docs changed.
- Commits.
- Anything left for the release workflow.
