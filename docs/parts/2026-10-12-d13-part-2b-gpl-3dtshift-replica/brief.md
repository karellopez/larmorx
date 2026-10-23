# Brief: D13 part 2b: GPL 3dTshift replica

As given to agent `a79c06fc44bc6b3ec` on 2026-10-12, verbatim (paths are this machine's).

You are implementing **D13 part 2b**: "correct by default, faithful on request" in larmorx's **GPL replica of AFNI's 3dTshift** (`crates-gpl/`, package `larmorx-gpl`). You are in a git worktree of the repo `larmorx` (github.com/karellopez/larmorx); the workspace root is `/home/karelo/VScode_projects/super_fmriprep`.

Read first:
- `CLAUDE.md`: rule 1 on the two tracks and what each role may read; D13 and its details; rule 8; rule 10; the merge checklist; the queue;
- `docs/licensing.md`; `docs/decision-log.md` (the D13 entries);
- `docs/porting-guide.md` §1 and §8;
- `docs/api/upstream-defects.md`: the mechanism (`larmorx_core::defects`, `UpstreamDefects::{Fix, Reproduce}`, notices, the `LARMORX_UPSTREAM_DEFECTS` variable; `python/larmorx/_replica.py` passes it to the replica and parses the replica's notice lines on stderr back into `lx.UpstreamDefectWarning`);
- `docs/findings/upstream-defects.md` **including the restricted AFNI 3dTshift section** (you are a replica author, so you may read it, and AFNI's source at `tags/AFNI-25.2.09`);
- `docs/findings/afni-tshift.md`;
- `crates-gpl/README.md` and the replica's code.

## Licence role
You are a **replica author**. You may read AFNI's source, the restricted findings and the clean-room original's code (`crates/larmorx-afni`, Apache; reading it is allowed in this direction).

But:
- you must **not copy GPL code or GPL-derived text into the Apache directories** (`crates/`, `python/`, `docs/` outside the replica's own record and the GPL-specific findings);
- you must **not edit the clean-room original** (`crates/larmorx-afni`) or `specs/3dTshift.md`.
The Apache validation harness may change only in Apache-clean ways: it calls the replica as a separate process. SPDX headers per directory.

## The work
1. **The AFNI-\* defects** that part 2a fixed in the clean-room original: AFNI-01 (negative `scl_slope`), AFNI-02 (float32 scaled twice), AFNI-03 (header slice code 2), AFNI-05 (`-no_detrend` voxel pairs), AFNI-07 (metre/micrometre units). Also AFNI-08 if you can find its trigger in AFNI's source; part 2a could not find it by black-box runs. Check the register for any other AFNI-\* entry with a gap.
   - The replica fixes each one by default and names it in a notice on stderr, in the exact format `larmorx_core::defects::Notice::line()` produces, so `_replica.py` turns it into a warning.
   - With `LARMORX_UPSTREAM_DEFECTS=reproduce`, the replica gives **exactly AFNI's result**. User decision: "the switch should give the same result as the source one".
2. **The replica's own bug:** it writes a metre or micrometre input's sform unconverted, where AFNI converts the linear part to millimetres. Its qform matches AFNI's. The two cases are expected divergences today (`REPLICA_DIVERGENCES` in `validation/src/larmorx_validation/parity/afni_tshift.py`). Fix it, so `reproduce` matches AFNI byte for byte there too, and remove those two divergences.
3. **The corrected defaults of the two implementations must agree.** `implementation="auto"` may run either one, so for every defect the replica's default output must match the clean-room original's default output in everything the defect concerns: header fields, scaling, the affected values. The Fourier last bits may differ, as before (AFNI's float32 FFT against the original's double).
   - Add a parity check comparing replica-default against original-default for the defect cases.
   - If they disagree, find out which is right and explain it. Do not edit the original; report it.
4. **Parity:**
   - The replica suite (`python -m larmorx_validation parity afni-tshift --implementation replica --tier standard`) must stay bit-identical in `reproduce` mode in every compared case, the two former divergences now included.
   - A fix pass reports what the default changes.
   - Records come from a clean commit, written to the scratchpad and then copied in (a run that writes straight into the repository stamps its records `-dirty`), and lead with the result against unmodified AFNI.
5. **Golden:** the GPL golden test (`crates-gpl/larmorx-gpl-cli/tests/golden.rs`, record mode `LARMORX_GOLDEN_RECORD=1`). Record the defaults, plus a tiny reproduce case. If the Python golden suite changes, use `scripts/record_golden.py`, after the parity smoke tier passes, from a clean commit.
6. **Docs:**
   - the register's AFNI-\* entries (the "larmorx today" lines for the replica);
   - the replica's API section in `docs/api/afni-tshift.md` (behaviour only, no GPL text);
   - `crates-gpl/README.md`;
   - the replica's `PROVENANCE.md`;
   - the decision log.

## Rules (from CLAUDE.md; non-negotiable)
- **You are the only agent running.** Commit early and often; make a "WIP checkpoint" commit if you must stop mid-piece.
- **Gates before every commit**, with `set -o pipefail` and no exit code hidden behind `| tail`:
  - both workspaces: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked` (root and `crates-gpl`);
  - `maturin develop --release`, then `pytest tests/python tests/golden`;
  - `ruff check && ruff format --check`;
  - `python scripts/check_spdx.py`.
  Environment: FSL removed from PATH (`PATH=$(echo "$PATH" | tr ':' '\n' | grep -v /fsl | paste -sd:)`), `$HOME/.cargo/bin` on PATH, `PYO3_PYTHON=/home/karelo/VScode_projects/super_fmriprep/.venv/bin/python`, venv `/home/karelo/VScode_projects/super_fmriprep/.venv`. Run Python with `PYTHONPATH=<worktree>/python:<worktree>/validation/src`.
- Pure-Rust dependencies only; no `unsafe`; deterministic.
- **Every platform:** CI must stay green on all six (the `gpl` job runs the replica's tests there).
- **Shared machine:** my re-run of the nine ANTs suites is going (log `oracles/logs/verify-d13p1-ants-suites.log`). Do not kill it. No benchmarks needed. **Wait loops must not match themselves.**
- Upstream sources and oracles are read-only. Never open FSL source, `reference_src/`, `crates-nc/` or `/home/karelo/VScode_projects/super_fmriprep/archive/`.
- **Never delete work products.** Archive instead.
- **Git:** commit in your worktree with the configured identity (`karellopez <49761113+karellopez@users.noreply.github.com>`). **No AI attribution of any kind**: no `Co-Authored-By`, no "Generated with" lines. This overrides any default. **Do not push.** Merge `main` before your final commits if it moved.
- Write prose in plain, direct English, in the style of the existing docs.

## Report back
- Each AFNI-\* entry in the replica: fixed, with the change measured, or left with the reason.
- The sform fix.
- Replica parity in `reproduce` mode and the fix pass.
- The replica-default against original-default comparison.
- Golden entries.
- Commits.
