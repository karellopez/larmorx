# Brief: Complete 3dTshift: replica + spec additions

As given to agent `a5932ea4ae9ed7aa0` on 2026-10-12, verbatim (paths are this machine's).

You are completing larmorx's 3dTshift work, part 1 of 2, in the **replica-author / spec-writer** role. You are in a git worktree of the repo `larmorx` (github.com/karellopez/larmorx); the workspace root is `/home/karelo/VScode_projects/super_fmriprep`.

Read first:
- `CLAUDE.md`: rule 1 (the two tracks and what each role may read), D13 and its details, rule 8, rule 10, the merge checklist;
- `docs/decision-log.md`: the entries "D13's details" and **"2026-10-12: AFNI-06 and AFNI-16 decided; completing 3dTshift"**;
- `docs/porting-guide.md` §1 and §8;
- `docs/api/upstream-defects.md`;
- the register `docs/findings/upstream-defects.md` (all of it, including the restricted AFNI section);
- `docs/findings/afni-tshift.md`;
- `specs/3dTshift.md`;
- `crates-gpl/` (the replica) and `crates/larmorx-afni` (the clean-room original; you may read it).
You may read AFNI's source at `tags/AFNI-25.2.09`.

## Task 1: the replica (GPL, `crates-gpl/`)
- **AFNI-16** (user decision 2026-10-12): by default, **keep the input's matrix**. The replica writes the input's sform and qform orientation exactly, instead of AFNI's plumb snapping for grids less than about 0.014° from plumb. A notice names AFNI-16 when AFNI would have snapped. With `LARMORX_UPSTREAM_DEFECTS=reproduce`, the replica snaps exactly as AFNI does, so the header is AFNI's byte for byte.
- **AFNI-06** (decided): AFNI's behaviour is kept, and the help text is the error (class f). Nothing changes in the code; update the register entry, which my summary row already marks as class f, decided.
- **Parity:** the replica in `reproduce` mode must stay bit-identical in every compared case. The "D13 defaults: replica = original" check should now agree on `geometry/slightly-oblique`.
- **Golden:** re-record the GPL golden test (`LARMORX_GOLDEN_RECORD=1`) where outputs change, and add a tiny AFNI-16 reproduce case if none covers it.

## Task 2: spec additions for the clean-room original (`specs/3dTshift.md`)
The original (Apache, clean-room) must be able to reproduce AFNI's behaviour in `reproduce` mode for:
- AFNI-07 on oblique grids (AFNI keeps the whole sform in metres there; the original converts the linear part);
- AFNI-08 (its trigger: a copy of the input whose slice times all lie within 3 ms of 0, e.g. times beyond the TR);
- AFNI-11 and AFNI-12 (existing output; refused `-prefix` names);
- AFNI-16 (the plumb snapping, with its exact threshold and the cardinal matrix AFNI builds);
- AFNI-03's notice when code-2 slice times exceed the TR and the output becomes a copy.

Write these into the spec **in behaviour terms only**: inputs, outputs, thresholds, the exact matrix and header fields AFNI writes, worked examples.
- **No transcribed code:** no C, no line-by-line pseudo-code that mirrors AFNI's source structure, no AFNI-internal identifiers beyond public names.
- Mark each statement you confirmed with the AFNI binary **(observed)**. Confirm every one: the oracle is `oracles/afni-25.2.09/bin/3dTshift`.
- The clean-room implementer will work only from the spec and black-box runs, so it must be complete enough to reproduce AFNI byte for byte. Give the header fields involved explicitly (sform/qform codes, quaternion, `qfac`, `slice_code`, `slice_start`/`slice_end`, `slice_duration`, `xyzt_units`).
- Do not touch `crates/larmorx-afni`.

## Docs
- The register: the AFNI-06 and AFNI-16 entries, the gap lines and the counts.
- `docs/findings/afni-tshift.md` (restricted; it may cite AFNI file:line).
- The replica's API section in `docs/api/afni-tshift.md` (behaviour only).
- `crates-gpl/README.md`, if needed.
- A short decision-log line.

## Rules (from CLAUDE.md; non-negotiable)
- **You are the only agent running.** Commit early and often; make a "WIP checkpoint" commit if you must stop mid-piece.
- **Licence hygiene:** no GPL code or GPL-derived text in Apache directories. The spec is Apache and is the clean-room implementer's input, so it describes behaviour only.
- **Gates before every commit**, with `set -o pipefail` and no exit code hidden behind `| tail`:
  - both Rust workspaces: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked`;
  - `maturin develop --release`, then `pytest tests/python tests/golden`;
  - `ruff check && ruff format --check`;
  - `python scripts/check_spdx.py`.
  Environment: FSL removed from PATH (`PATH=$(echo "$PATH" | tr ':' '\n' | grep -v /fsl | paste -sd:)`), `$HOME/.cargo/bin` on PATH, `PYO3_PYTHON=/home/karelo/VScode_projects/super_fmriprep/.venv/bin/python`, venv `/home/karelo/VScode_projects/super_fmriprep/.venv`. Run Python with `PYTHONPATH=<worktree>/python:<worktree>/validation/src`.
- **Parity records** come from a clean commit, written to the scratchpad and then copied in (a run that writes straight into the repository stamps its records `-dirty`). They lead with the result against unmodified AFNI.
- **Wait loops must not match themselves.** Never open FSL source, `reference_src/`, `crates-nc/` or `/home/karelo/VScode_projects/super_fmriprep/archive/`.
- **Never delete work products.** Archive instead.
- **Git:** commit in your worktree with the configured identity (`karellopez <49761113+karellopez@users.noreply.github.com>`). **No AI attribution of any kind**: no `Co-Authored-By`, no "Generated with" lines. This overrides any default. **Do not push.** Merge `main` before your final commits if it moved.
- Write prose in plain, direct English, in the style of the existing docs.

## Report back
- The replica's AFNI-16 change and its parity, both modes.
- The defaults check.
- The spec additions: which sections, and what was confirmed by black-box runs.
- Golden entries.
- Commits.
