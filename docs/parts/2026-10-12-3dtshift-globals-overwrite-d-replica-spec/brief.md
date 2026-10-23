# Brief: 3dTshift globals: -overwrite, -D (replica+spec)

As given to agent `a5cefece59c950414` on 2026-10-12, verbatim (paths are this machine's).

You are adding AFNI's **global command-line options** to larmorx's 3dTshift, in the **replica-author / spec-writer** role. You are in a git worktree of the repo `larmorx` (github.com/karellopez/larmorx); the workspace root is `/home/karelo/VScode_projects/super_fmriprep`.

Read first:
- `CLAUDE.md`: rule 1 (roles), D13 and its details, rule 8, rule 10, "Every platform from the start", the merge checklist;
- `docs/decision-log.md`, the latest entries;
- `docs/porting-guide.md`;
- `specs/3dTshift.md`, especially §6.1–6.5 and §7, just added;
- the register's AFNI section (`docs/findings/upstream-defects.md`; you may read the restricted parts) and `docs/findings/afni-tshift.md`;
- the replica in `crates-gpl/`;
- `crates/larmorx-cli/src/replica.rs` and `python/larmorx/_replica.py`.
You may read AFNI's source at `tags/AFNI-25.2.09`.

## The user's decision (2026-10-12)
Both larmorx 3dTshift implementations accept AFNI's global options **`-overwrite`** and **`-Dname=value`**, anywhere on the command line as AFNI does, working identically on Linux, macOS and Windows. Design:
- **`-overwrite`:**
  - write the output to a temporary file in the same directory, then rename it over the target (Rust's `std::fs::rename` replaces an existing file on all three systems);
  - on Windows, retry the rename briefly when another program holds the file (antivirus, viewers), then fail with a clear message, never leaving a partial file;
  - paths with spaces and non-ASCII names work everywhere.
  Interplay: AFNI-11 (an existing output) and its D13 handling.
- **`-Dname=value`:**
  - **never modify the process environment** (`std::env::set_var` is unsafe in multithreaded Rust);
  - parse each `-D` into an explicit **settings table** passed to the tool;
  - real environment variables the user set are read into the same table, so `-D` and the environment behave alike, with `-D` winning as in AFNI;
  - implement, explicitly, each AFNI environment variable that 3dTshift's code path actually consults: the program itself and the AFNI library routines it calls for reading, checking names and writing NIfTI (e.g. `AFNI_ALLOW_ARBITRARY_FILENAMES` for AFNI-12; find them all in the source and list them);
  - a variable AFNI would not consult on this path is accepted with a short notice that it has no effect.

## Your work
1. **The list:** every AFNI environment variable and global option on 3dTshift's path. For each: the default, the values, and the effect on 3dTshift's output, exit status and messages. Confirm each effect with the oracle binary (`oracles/afni-25.2.09/bin/3dTshift`), with the variable set in the environment and as `-D`.
2. **The replica:** implement `-overwrite`, `-Dname=value`, the settings table, the environment reading and every listed variable.
   - `reproduce` mode must match AFNI byte for byte, exit codes and messages included, for every combination you test.
   - Default (fix) mode keeps D13's corrections. Say how each variable interacts with them; e.g. AFNI-12's corrected default already accepts any name, so `AFNI_ALLOW_ARBITRARY_FILENAMES` changes nothing there.
3. **Spec additions** (`specs/3dTshift.md` §7 or a new section), **in behaviour terms only**, for the clean-room implementer of the original, who will implement the same from the spec:
   - the list of variables with their observed effects;
   - the parsing rules for global options (positions, `-D` forms, what AFNI does with malformed ones);
   - the cross-platform design above.
   No transcribed code, no AFNI-internal names beyond the public variable names. Mark each statement **(observed)** where confirmed with the binary.
4. **Parity:** add cases to the 3dTshift suite: `-overwrite` onto an existing output, `-D` and environment forms of each variable, malformed `-D`, global options in odd positions.
   - The replica must stay bit-identical in `reproduce` mode in every compared case.
   - The new cases are expected divergences for the original until part 2b.
   - Records come from a clean commit, written to the scratchpad and then copied in, and lead with the result against unmodified AFNI.
5. **Cross-platform tests in CI:** tiny tests (rule 10) proving `-overwrite` replaces an existing file, and that `-D` takes effect, on all six platforms, including Windows. The GPL golden test or the replica's CLI tests run in the `gpl` CI job; the Python dispatch in the `test` job. Keep them well under a second.
6. **Docs:**
   - the register (new entries if you find defects in the variables' handling);
   - `docs/findings/afni-tshift.md` (restricted);
   - the replica's API section (behaviour only);
   - `crates-gpl/README.md`;
   - the decision log;
   - the porting guide, with a short section on global options, environment variables and the settings-table pattern, since every AFNI and FSL port will meet this.

## Rules (from CLAUDE.md; non-negotiable)
- **You are the only agent running.** Commit early and often; make a "WIP checkpoint" commit if you must stop mid-piece.
- **Licence hygiene:** no GPL code or GPL-derived text in Apache directories; the spec describes behaviour only. Do not edit `crates/larmorx-afni` (the clean-room original); part 2b does that.
- **Gates before every commit**, with `set -o pipefail` and no exit code hidden behind `| tail`:
  - both Rust workspaces: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked`;
  - `maturin develop --release`, then `pytest tests/python tests/golden`;
  - `ruff check && ruff format --check`;
  - `python scripts/check_spdx.py`.
  Environment: FSL removed from PATH (`PATH=$(echo "$PATH" | tr ':' '\n' | grep -v /fsl | paste -sd:)`), `$HOME/.cargo/bin` on PATH, `PYO3_PYTHON=/home/karelo/VScode_projects/super_fmriprep/.venv/bin/python`, venv `/home/karelo/VScode_projects/super_fmriprep/.venv`. Run Python with `PYTHONPATH=<worktree>/python:<worktree>/validation/src`.
- Pure-Rust dependencies only; no `unsafe` (so no `set_var`); deterministic.
- **Shared machine:** my parity re-runs of both 3dTshift suites may be going (`oracles/logs/verify-tshift-p1.log`). Do not kill them. **Wait loops must not match themselves.**
- Never open FSL source, `reference_src/`, `crates-nc/` or `/home/karelo/VScode_projects/super_fmriprep/archive/`.
- **Never delete work products.** Archive instead.
- **Git:** commit in your worktree with the configured identity (`karellopez <49761113+karellopez@users.noreply.github.com>`). **No AI attribution of any kind**: no `Co-Authored-By`, no "Generated with" lines. This overrides any default. **Do not push.** Merge `main` before your final commits if it moved.
- Write prose in plain, direct English, in the style of the existing docs.

## Report back
- The variable list and what each does.
- The replica's parity, both modes.
- The cross-platform tests.
- The spec sections added.
- Commits.
