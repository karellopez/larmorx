# Brief: D13 part 1: ANTs/ITK defects

As given to agent `a448db6a134de1e65` on 2026-10-11, verbatim (paths are this machine's).

You are implementing **D13 part 1**: "correct by default, faithful on request" for larmorx's ANTs and ITK ports. You are in a git worktree of the repo `larmorx` (github.com/karellopez/larmorx); the workspace root is `/home/karelo/VScode_projects/super_fmriprep`.

Read first:
- `CLAUDE.md`: D13 and its details; rule 8 (the source is the specification); rule 10 (CI budget); the merge checklist and "Work in progress and queue";
- `docs/decision-log.md`: the entries "The source is the specification; correct by default (D13)", "D13's details", "FSL-02 stays opt-in";
- `docs/porting-guide.md` §8;
- **`docs/api/upstream-defects.md`**: the mechanism already built (`larmorx_core::defects`, `UpstreamDefects::{Fix, Reproduce}`, `Notice`, Python `upstream_defects=` and `lx.UpstreamDefectWarning`, CLI `LARMORX_UPSTREAM_DEFECTS`). See how part 2a wired it into `larmorx-mri`, `larmorx-afni` and `resample_series`, and reuse that pattern exactly;
- **`docs/findings/upstream-defects.md`**: the register. Its "D13 gaps" section lists every ANTs/ITK entry with the larmorx code path it touches.

ANTs and ITK are Apache-2.0 and these are replicas, so you may read `tags/ANTs-v2.6.5` and `tags/ITK-v5.4.5` (read-only).

## The work
Every ANTs/ITK entry of class (a), (c) or (e) in the register's "D13 gaps": `ANTS-*` and `ITK-*`, plus SciPy's SCI-02 (the undefined float-to-integer cast). Among them:
- the ImageMath op defects: `/`, `total`/`mean`, `vtotal`, `Neg`, `Normalize`, `TruncateImageIntensity`'s range, `G`'s NaN, `GetLargestComponent`'s empty result, FillHoles out of bounds;
- Otsu with a mask; N4's histogram range and FFT size; DenoiseImage's Rician NaN; ITK's floor of tiny negatives; extra dimensions losing their direction; float labels stopping at 2^24;
- the command-line quirks: ImageMath's exit status after a failure, MultiplyImages with a missing file, N4's options and unreadable mask, `Laplacian`'s flag and the sigma count, short file names, operand parsing, ThresholdImage's mode names, ResampleImage's seventh argument (ANTS-30), ResampleImageBySpacing's arguments, antsApplyTransforms' `-u` casts and ignored interpolator parameters (ANTS-33);
- and the rest listed there.

Take each from the register; do not trust this summary over it.

**The user's decisions apply** (the decision log, "D13's details"):
- **Undefined float-to-integer conversions** (`-u` casts, ANTS-06, SCI-02): **clamp to the integer range and warn** by default; `reproduce` gives x86-64's wraparound exactly as today.
- **Failure and garbage cases get a switch too.** Where the original's output is deterministic, `reproduce` gives it exactly. Where it depends on memory contents (unfilled buffers, out-of-bounds reads), `reproduce` gives everything defined (exit code, files, headers), fills the undefined part deterministically, and warns that it does.
- **Questionable behaviours (marked `c?`) stay faithful and documented;** do not change their defaults.
- **ResampleImage's corrected command line (ANTS-30):** design it, document the design in the API page, and keep `reproduce` exactly as ANTs parses it.

## For each entry
- The default fixes it; `reproduce` (Python `upstream_defects="reproduce"`, CLI `LARMORX_UPSTREAM_DEFECTS=reproduce`) gives the original's result bit for bit.
- A notice or warning names the entry when the defect would have changed the result.
- Unit tests of both modes and of the warning.
- Parity: the ANTs suites compare in `reproduce` mode and must stay exactly as before: the same bit-identical counts against the **current oracle** (`oracles/ants-2.6.5`, the GCC 14.3 build; never the old `ants-2.6.5-gcc11-O3`). A fix pass reports what the default changes (cases, max difference), as part 2a did.
- Records lead with the result against the unmodified original. Regenerate them from a clean commit.
- Update the register entry's "larmorx today" and its gap line, the API pages' D13 sections, and the decision log.

## Golden tests
Re-record with `scripts/record_golden.py` (after the smoke parity passes, from a clean commit). Add tiny `*.reproduce` cases where a default changed (rule 10: tiny inputs, the suite under a minute). Say which entries changed.

## Rules (from CLAUDE.md; non-negotiable)
- **You are the only agent running.** Commit early and often; make a "WIP checkpoint" commit if you must stop mid-piece.
- **Gates before every commit**, with `set -o pipefail` and no exit code hidden behind `| tail`:
  - `cargo fmt --all --check`
  - `cargo clippy --workspace --all-targets --locked -- -D warnings`
  - `cargo test --workspace --locked`
  - the same three in `crates-gpl` if `larmorx-core`/`larmorx-io` change; refresh `crates-gpl/Cargo.lock` with `cd crates-gpl && cargo metadata --offline --format-version 1 >/dev/null` if their dependencies change
  - `maturin develop --release`, then `pytest tests/python tests/golden`
  - `ruff check && ruff format --check`
  - `python scripts/check_spdx.py`
  Environment: FSL removed from PATH (`PATH=$(echo "$PATH" | tr ':' '\n' | grep -v /fsl | paste -sd:)`), `$HOME/.cargo/bin` on PATH, `PYO3_PYTHON=/home/karelo/VScode_projects/super_fmriprep/.venv/bin/python`, venv `/home/karelo/VScode_projects/super_fmriprep/.venv`. Run Python with `PYTHONPATH=<worktree>/python:<worktree>/validation/src`.
- Pure-Rust dependencies only; no `unsafe`; deterministic; correctly rounded maths.
- **Every platform:** CI must stay green on all six.
- **Shared machine:** fMRIPrep oracle runs are going (`oracles/fmriprep-25.2.6/`; chain log `oracles/logs/fmriprep-25.2.6-sdc-chain.log`), and my parity re-run of antsApplyTransforms may still be running. Do not kill them. No benchmarks needed. **Wait loops must not match themselves.**
- Never open FSL source, AFNI source, `reference_src/`, `crates-gpl/` source (beyond the lockfile refresh), `crates-nc/` or `/home/karelo/VScode_projects/super_fmriprep/archive/`.
- **Never delete work products.** Archive instead.
- **Git:** commit in your worktree with the configured identity (`karellopez <49761113+karellopez@users.noreply.github.com>`). **No AI attribution of any kind**: no `Co-Authored-By`, no "Generated with" lines. This overrides any default. **Do not push.** Merge `main` before your final commits if it moved.
- Write prose in plain, direct English, in the style of the existing docs.

## Report back
- Each entry: fixed, its default change measured, or left (with the reason).
- Parity per suite in `reproduce` mode (unchanged?) and the fix pass.
- The ResampleImage design.
- Golden entries.
- Commits.
