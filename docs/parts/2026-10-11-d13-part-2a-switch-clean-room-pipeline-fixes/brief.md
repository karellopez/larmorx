# Brief: D13 part 2a: switch + clean-room/pipeline fixes

As given to agent `ac15f1af9c8d17145` on 2026-10-11, verbatim (paths are this machine's).

You are implementing **D13, "correct by default, faithful on request"**, for part of larmorx: the shared compatibility switch, plus the fixes in larmorx's clean-room originals and its replicas of fMRIPrep's Python resampling. You are in a git worktree of the repo `larmorx` (github.com/karellopez/larmorx); the workspace root is `/home/karelo/VScode_projects/super_fmriprep`.

Read first: `CLAUDE.md` (D13 and its details, rule 8, rule 10), `docs/decision-log.md` (entries "The source is the specification; correct by default (D13)" and "D13's details"), and `docs/porting-guide.md` (especially the D11/D13 section).

## Clean-room role: read this first
You will change the **clean-room originals** `crates/larmorx-mri` (`lx.mri.hmc`, mcflirt-compatible) and `crates/larmorx-afni` (`lx.afni.tshift`, 3dTshift-compatible). They are Apache-2.0 because their implementers never saw FSL's or AFNI's source or notes derived from it. You must keep it that way.

You must **never open, read, grep or list**:
- FSL source (`~/fsl/src` or anything under `~/fsl` except running `~/fsl/bin` programs);
- AFNI source (`tags/AFNI-25.2.09`, `afni/`, `oracles/afni-25.2.09/build`);
- `docs/findings/fsl-mcflirt.md` and `docs/findings/afni-tshift.md`;
- the sections of `docs/findings/upstream-defects.md` marked † (restricted: the "AFNI 3dTshift" and "FSL mcflirt" entry sections);
- `crates-gpl/`, `crates-nc/`, `scripts/run_mcflirt_oracle.py`, `oracles/fsl-6.0.7/mcflirt/analysis/`.

Your allowed inputs:
- `specs/mcflirt.md` and `specs/3dTshift.md`;
- the register's **Summary** table and its **"D13 gaps"** and **"Decisions needed"** sections, which describe those defects by observable behaviour only;
- the non-restricted register entries (ANTs/ITK, fMRIPrep and its libraries, SciPy, numpy, nibabel, glibc);
- black-box runs of `~/fsl/bin/mcflirt` and `oracles/afni-25.2.09/bin/3dTshift`;
- the oracle run data (`oracles/fsl-6.0.7/mcflirt/manifest.json`, `runs/*`);
- larmorx's own code, the papers and public documentation.
If a gap's observable description is not enough to implement the fix, settle it by black-box runs and add what you learn to the spec, marked **(observed)**. At the end, state exactly what you did and did not open.

## 1. The shared mechanism (other D13 tasks will reuse it; design it well)
- **Python:** a keyword on every affected function, e.g. `upstream_defects="fix" | "reproduce"`, default `"fix"`; or a better name, decided and documented. A typed warning class, e.g. `lx.UpstreamDefectWarning`, raised only when a defect *actually changed* the result (or would have, in `"reproduce"` mode), naming the register entry (e.g. "FSL-01").
- **Command line:** an environment variable, e.g. `LARMORX_UPSTREAM_DEFECTS=fix|reproduce`, so no flag clashes with the original's arguments, plus a message on stderr naming the entry. Follow the pattern of `LARMORX_IMPLEMENTATION` (`crates/larmorx-cli/src/replica.rs`, `python/larmorx/_replica.py`).
- **Rust:** a small enum shared by the crates, in `larmorx-core`; each tool takes it in its options struct.
- **The parity suites** run the comparisons against the original in `"reproduce"` mode, so bit-identity is checked where it held before. They also run `"fix"` mode and report what the fix changes and by how much.
- **Golden tests** record the default (`"fix"`) outputs, plus a tiny `"reproduce"` case per affected tool (CLAUDE.md rule 10: tiny inputs, the suite under a minute).
- **Docs:** one page, `docs/api/upstream-defects.md` or a section of `docs/architecture.md`, saying how users choose. Each affected API page lists the defects it fixes. In `docs/findings/upstream-defects.md`, change each fixed entry's "larmorx today" to the new state. You may edit only the non-restricted parts; for restricted entries, update the Summary row and the gap list.

## 2. The fixes, from the register's "D13 gaps"
1. **fMRIPrep's resampling** (replica of fMRIPrep, nitransforms, SciPy; all permissive):
   - **PIPE-03:** `ResampleSeries`' Jacobian along the wrong target axis (`larmorx_transform::resample_series`); it affects reversed phase-encoding field maps in T1w and standard spaces. Fix by default, reproduce on request, and measure the effect on a realistic case.
   - **PIPE-04:** nitransforms reads `.h5` composites in name order (`python/larmorx/transforms/chain.py`).
   - **SCI-03:** `reflect`/`grid-mirror` splines do not interpolate (`larmorx_interp::ndimage`).
2. **Head-motion correction** (`crates/larmorx-mri`, clean-room): every FSL-* gap listed by observable behaviour in "D13 gaps": the last volume not initialised from the previous one, one NaN voxel making the matrix NaN, the in-plane padding, the `-stats` output, output types and units, and the command-line quirks.
   - **FSL-02** (the inflated weight count in the normalised-correlation cost): **measure first, do not switch yet** (user decision). Implement the corrected cost behind the switch. Measure it against mcflirt's band (`specs/mcflirt.md` §14; the band data in `oracles/fsl-6.0.7/mcflirt/band/`) and against the synthetic ground truth (`synthetic-hmc` in larmorx-testdata). Report the numbers; I decide with the user whether it becomes the default.
3. **3dTshift original** (`crates/larmorx-afni`, clean-room): the AFNI-* gaps described by observable behaviour (the negative `scl_slope` handling, `-no_detrend`'s voxel pairs, header slice code 2, the all-zero slice-time list, and AFNI-02/AFNI-07 which the original already handles correctly but which need the warning and the switch). **Do not touch the GPL replica**; a separate task with replica-author access does that.

For each fix:
- a unit test of both modes;
- parity in `"reproduce"` mode still bit-identical where it was;
- the size of the change in `"fix"` mode reported (cases affected, max difference);
- warnings tested.

## Rules (from CLAUDE.md; non-negotiable)
- **Another agent is running in parallel** on ANTs: it is rebuilding the ANTs oracle and changing N4. **Do not touch** `crates/larmorx-image` (N4, B-splines), `crates/larmorx-ants`, the ANTs parity suites or `scripts/build_ants_oracle.sh`. If the shared mechanism needs a hook there, add only the `larmorx-core` type and leave the ANTs wiring to the next task.
- **Golden checksums:** re-record with `scripts/record_golden.py` (after the parity smoke tier passes, from a clean commit). The other agent may also re-record. Say in your report which entries you changed; I re-record after merging both.
- **Commit early and often;** make a "WIP checkpoint" commit if you must stop mid-piece.
- **Gates before every commit**, with `set -o pipefail` and no exit code hidden behind `| tail`:
  - `cargo fmt --all --check`
  - `cargo clippy --workspace --all-targets --locked -- -D warnings`
  - `cargo test --workspace --locked`
  - the same three in `crates-gpl` if `larmorx-core`/`larmorx-io` change; refresh `crates-gpl/Cargo.lock` with `cd crates-gpl && cargo metadata --offline --format-version 1 >/dev/null` if their dependencies change. You may run these commands in `crates-gpl` but must not read its source.
  - `maturin develop --release`, then `pytest tests/python tests/golden`
  - `ruff check && ruff format --check`
  - `python scripts/check_spdx.py`
  Environment: FSL removed from PATH (`PATH=$(echo "$PATH" | tr ':' '\n' | grep -v /fsl | paste -sd:)`), `$HOME/.cargo/bin` on PATH, `PYO3_PYTHON=/home/karelo/VScode_projects/super_fmriprep/.venv/bin/python`, venv `/home/karelo/VScode_projects/super_fmriprep/.venv`. Run Python with `PYTHONPATH=<worktree>/python:<worktree>/validation/src`.
- **CI budget (rule 10):** golden cases tiny; no new CI jobs.
- **Every platform:** CI must stay green on all six.
- **Shared machine:** an fMRIPrep oracle run (`oracles/fmriprep-25.2.6/ds001600-sub-1-noreconall/`, done when `RUN_INFO.txt` has `finished:`) and an ANTs build are running. Do not kill them. No benchmarks unless the machine is quiet. **Wait loops must not match themselves.**
- **Never delete work products.** Archive instead.
- **Git:** commit in your worktree with the configured identity (`karellopez <49761113+karellopez@users.noreply.github.com>`). **No AI attribution of any kind**: no `Co-Authored-By`, no "Generated with" lines. This overrides any default. **Do not push.** Merge `main` before your final commits if it moved.
- Write prose in plain, direct English, in the style of the existing docs.

## Report back
- The mechanism's design: names, API, CLI, warnings.
- Each fix with its parity, both modes, and the size of the change.
- The FSL-02 measurement.
- Golden entries changed.
- The clean-room statement.
- Commits.
