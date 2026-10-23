# Brief: 3dTshift part 2b: clean-room original

As given to agent `ad98aa6636037d334` on 2026-10-12, verbatim (paths are this machine's).

You are completing larmorx's **clean-room 3dTshift original** (`crates/larmorx-afni`, Apache-2.0), part 2b. You are in a git worktree of the repo `larmorx` (github.com/karellopez/larmorx); the workspace root is `/home/karelo/VScode_projects/super_fmriprep`.

Read first:
- `CLAUDE.md` (rule 1 and the clean-room process; D13 and its details; rule 6; rule 8; rule 10; the merge checklist);
- `docs/porting-guide.md` (§1, §8, §9.1 on the settings table);
- `docs/api/upstream-defects.md`; `docs/api/afni-tshift.md`;
- **`specs/3dTshift.md`**: your main input, especially §6–6.5 and §7–7.1, added recently, and §8 and §9.

## Clean-room rules (why this code may be Apache-2.0)
You must **never open, read, grep or list**:
- AFNI's source: `tags/AFNI-25.2.09`, `afni/`, `oracles/afni-25.2.09/build`;
- `docs/findings/afni-tshift.md`;
- the sections of `docs/findings/upstream-defects.md` marked † (the AFNI 3dTshift and FSL mcflirt entry sections);
- the replica's source, `crates-gpl/` (you may run its program as a black box and run cargo commands there, but must not read its code);
- FSL source; `reference_src/`; `crates-nc/`; other tasks' `/tmp/claude-1000/` files.

Your allowed inputs:
- the spec;
- the register's Summary table, its "D13 gaps" and its non-restricted entries;
- black-box runs of `oracles/afni-25.2.09/bin/3dTshift`, and of the replica program `larmorx-gpl afni 3dTshift` (built in `crates-gpl/target/release/`);
- AFNI's public help text;
- public documentation of IEEE 754 and of the x87 80-bit extended format;
- larmorx's own Apache code.
**Never run AFNI options that open a GUI or browser** (`-h_view`, `-h_web`, …); one opened gedit on the user's desktop before.
If the spec is unclear, settle it by black-box runs and add what you learn to the spec, marked **(observed)**. At the end, state exactly what you did and did not open.

## What to implement, from the spec
1. **§6.1 geometry:**
   - AFNI-07 on oblique grids: in `reproduce`, keep the whole sform in metres as AFNI does;
   - AFNI-16: in `reproduce`, the plumb snapping with its exact single-precision test and cardinal matrix, header fields exact; by default keep the input's matrix and warn;
   - the extended-precision header arithmetic (AFNI-24, described in the spec) where it decides header bytes. Implement 80-bit extended arithmetic from the public format if needed; pure Rust, no `unsafe`.
2. **§6.2 the slice fields of a copy:** AFNI-08 and AFNI-17, with the switch.
3. **§6.3 output names:** AFNI-11 and AFNI-12, with the switch and the exact messages and exit codes.
4. **§6.4 AFNI-03's notice** when the output becomes a copy.
5. **§7 and §7.1:**
   - the global options `-overwrite`, `-Dname=value`, `-ok_1D_text`, `-skip_afnirc`, accepted anywhere as the spec describes;
   - the settings table, never modifying the process environment. The CLI reads the environment into the table, with `-D` winning; the Python API takes `afni_settings` explicitly and never reads the environment;
   - every variable in the spec's table, with its effects and messages; the "no effect" notice for the others;
   - `-overwrite` as a temporary file and then a rename, with Windows lock retries, as the spec describes;
   - AFNI-18 to AFNI-22 as the spec says.
6. **AFNI-23,** as the spec's §7.1 describes it, with the switch.
7. **The Python API:** remove the `NotImplementedError` that `lx.afni.tshift(..., implementation="original", afni_settings=...)` raises today for `AFNI_NIFTI_PRIORITY`, `AFNI_NIFTI_SCALE`, `AFNI_NIFTI_VIEW`, `AFNI_DEFAULT_STD_SPACE`, `AFNI_NIFTI_WRITE_TYPE` and `AFNI_NOREALPATH`, once each works.

## Targets
- **The original's parity** (`python -m larmorx_validation parity afni-tshift --tier standard`): the 71 expected divergences that exist because the spec's behaviours were missing must pass. In `reproduce` mode the original matches AFNI in data, as before, within its documented thresholds for the Fourier last bits. Headers and messages match exactly where the spec says AFNI's are deterministic.
- **The "D13 defaults: replica = original" check** agrees on every case, AFNI-08's copy included.
- **Records** come from a clean commit, written to the scratchpad and then copied in, and lead with the result against unmodified AFNI.
- **Golden:** tiny new cases for the original's new paths (rule 10), recorded with `scripts/record_golden.py` after the parity smoke tier passes, from a clean commit. Add tiny cross-platform tests for the original's `-overwrite` and `-D` on all six platforms, as the replica has.
- **Docs:**
  - `docs/api/afni-tshift.md` (the original's sections and counts);
  - the register's Summary rows and gap lines for the original (non-restricted parts only);
  - `crates/larmorx-afni/PROVENANCE.md` (clean-room inputs);
  - the decision log;
  - README and CLAUDE.md numbers for 3dTshift if they change.

## Rules (from CLAUDE.md; non-negotiable)
- **You are the only agent running.** Commit early and often; make a "WIP checkpoint" commit if you must stop mid-piece.
- **Gates before every commit**, with `set -o pipefail` and no exit code hidden behind `| tail`:
  - both Rust workspaces: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked`; the commands only in `crates-gpl`, no reading its source;
  - `maturin develop --release`, then `pytest tests/python tests/golden`;
  - `ruff check && ruff format --check`;
  - `python scripts/check_spdx.py`.
  Environment: FSL removed from PATH (`PATH=$(echo "$PATH" | tr ':' '\n' | grep -v /fsl | paste -sd:)`), `$HOME/.cargo/bin` on PATH, `PYO3_PYTHON=/home/karelo/VScode_projects/super_fmriprep/.venv/bin/python`, venv `/home/karelo/VScode_projects/super_fmriprep/.venv`. Run Python with `PYTHONPATH=<worktree>/python:<worktree>/validation/src`.
- Pure-Rust dependencies only; no `unsafe`; deterministic.
- **Shared machine:** my parity re-runs of both 3dTshift suites may be going (`oracles/logs/verify-tshift-afni23.log`); do not kill them. **Wait loops must not match themselves.**
- **Never delete work products.** Archive instead.
- **Git:** commit in your worktree with the configured identity (`karellopez <49761113+karellopez@users.noreply.github.com>`). **No AI attribution of any kind**: no `Co-Authored-By`, no "Generated with" lines. This overrides any default. **Do not push.** Merge `main` before your final commits if it moved.
- Write prose in plain, direct English, in the style of the existing docs.

## Report back
- What was implemented.
- The original's parity: expected divergences before and after, bit-identical counts, header and message agreement.
- The defaults check.
- Spec gaps settled by black-box runs.
- Golden and cross-platform tests.
- The clean-room statement.
- Commits.
