# Brief: AFNI-23 fix + explicit Python settings

As given to agent `a3b361d527b599c1e` on 2026-10-12, verbatim (paths are this machine's).

You are making two small changes to larmorx's 3dTshift, in the **replica-author / spec-writer** role. You are in a git worktree of the repo `larmorx` (github.com/karellopez/larmorx); the workspace root is `/home/karelo/VScode_projects/super_fmriprep`.

Read first:
- `CLAUDE.md`: rule 1 (roles), rule 6 (Python: typed inputs, **no global config**), D13 and its details, rule 10, the merge checklist;
- `docs/decision-log.md`, the latest entries;
- `specs/3dTshift.md` §6–7.1;
- the register's AFNI entries, including AFNI-23;
- `docs/findings/afni-tshift.md`;
- `crates-gpl/` (the replica's settings table, `environ`);
- `python/larmorx/_replica.py` and `python/larmorx/afni/`;
- `docs/api/afni-tshift.md`.
You may read AFNI's source at `tags/AFNI-25.2.09`.

## Two user decisions (2026-10-12)
1. **AFNI-23 is fixed by default.** With `AFNI_NIFTI_SCALE=NO`, AFNI keeps an integer image's slope but silently drops its intercept, so the values are off by that offset.
   - larmorx's default keeps the values right. Find out from AFNI's source what `AFNI_NIFTI_SCALE=NO` is meant to do, and choose the correct result accordingly (e.g. apply both slope and intercept, or keep both in the header). Explain the choice.
   - A notice names AFNI-23 when AFNI would have dropped the intercept.
   - `LARMORX_UPSTREAM_DEFECTS=reproduce` gives AFNI's bytes.
   - Implement it in the **replica**, add a parity case or two, and write the **spec text** (behaviour only, **(observed)**) so the clean-room original can follow in its next task. Update the register entry (class c, decided).
2. **The Python API never reads the process environment** (rule 6). `lx.afni.tshift` today passes the environment's AFNI variables to the replica, and forces a few.
   - Instead, settings are explicit: a typed keyword such as `afni_settings: Mapping[str, str] | None = None`, or a small typed dataclass if that fits the existing result/option types better. The same call gives the same result whatever the shell has set.
   - Pass these explicitly to the replica, as `-D` options or a clean environment built from them; never inherit the caller's AFNI variables.
   - Keep the forced settings the wrapper needs (paths with spaces, plain messages), documented.
   - The **command line** (`larmorx afni 3dTshift`, `larmorx-gpl afni 3dTshift`) keeps reading the environment, as AFNI does.
   - Make the Python parameter work for the **original** too, as far as it supports settings today. Where the original does not yet support a variable (part 2b adds them), raise a clear `NotImplementedError` naming the variable, or document it; do not silently ignore it.
   - Tests: a variable set in `os.environ` has no effect on the Python call, and the explicit parameter does take effect. They must be tiny and run on all six platforms.

## Docs
- `docs/api/afni-tshift.md` (the Python parameter, and the environment rule for the CLI against Python);
- `.pyi`;
- the register;
- the restricted findings;
- the spec;
- `docs/porting-guide.md` §9.1 (the settings-table pattern: Python explicit, CLI from the environment);
- the decision log.
- Also fix `README.md`'s stale 3dTshift numbers (it still quotes 217 of 217 from before part 2a; use the current records).

## Rules (from CLAUDE.md; non-negotiable)
- **You are the only agent running.** Commit early and often; make a "WIP checkpoint" commit if you must stop mid-piece.
- **Licence hygiene:** no GPL code or GPL-derived text in Apache directories. The Python wrapper is Apache: it may know the replica's program name, arguments and settings, nothing more. Do not edit the clean-room original's Rust code (`crates/larmorx-afni`); part 2b does that.
- **Gates before every commit**, with `set -o pipefail` and no exit code hidden behind `| tail`:
  - both Rust workspaces: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked`;
  - `maturin develop --release`, then `pytest tests/python tests/golden`;
  - `ruff check && ruff format --check`;
  - `python scripts/check_spdx.py`.
  Environment: FSL removed from PATH (`PATH=$(echo "$PATH" | tr ':' '\n' | grep -v /fsl | paste -sd:)`), `$HOME/.cargo/bin` on PATH, `PYO3_PYTHON=/home/karelo/VScode_projects/super_fmriprep/.venv/bin/python`, venv `/home/karelo/VScode_projects/super_fmriprep/.venv`. Run Python with `PYTHONPATH=<worktree>/python:<worktree>/validation/src`.
- **Parity records** come from a clean commit, written to the scratchpad and then copied in, and lead with the result against unmodified AFNI.
- **Shared machine:** my parity re-runs of both 3dTshift suites may be going (`oracles/logs/verify-tshift-p2a.log`); do not kill them. **Wait loops must not match themselves.**
- **Do not run AFNI options that open a GUI or browser** (`-h_view`, `-h_web`, ...). Last time one opened gedit on the user's desktop.
- Never open FSL source, `reference_src/`, `crates-nc/` or `/home/karelo/VScode_projects/super_fmriprep/archive/`.
- **Never delete work products.** Archive instead.
- **Git:** commit in your worktree with the configured identity (`karellopez <49761113+karellopez@users.noreply.github.com>`). **No AI attribution of any kind**: no `Co-Authored-By`, no "Generated with" lines. This overrides any default. **Do not push.** Merge `main` before your final commits if it moved.
- Write prose in plain, direct English, in the style of the existing docs.

## Report back
- The AFNI-23 choice and its parity, both modes.
- The Python settings API and its tests.
- Docs.
- Commits.
