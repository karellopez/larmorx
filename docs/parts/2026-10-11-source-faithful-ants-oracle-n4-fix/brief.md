# Brief: Source-faithful ANTs oracle + N4 fix

As given to agent `a715e3d8d3821e4e8` on 2026-10-11, verbatim (paths are this machine's).

You are making larmorx's ANTs ports follow the **C++ source**, and making the ANTs oracle honour it too. You are in a git worktree of the repo `larmorx` (github.com/karellopez/larmorx); the workspace root is `/home/karelo/VScode_projects/super_fmriprep`. Read these first: `CLAUDE.md` (especially the new rule 8: "the source is the specification; an oracle binary is only a check", and D13), `docs/decision-log.md` (2026-10-11, "The source is the specification; correct by default"), `docs/findings/ants-n4.md` ("GCC 11's code generation on the oracle"), `docs/findings/fmriprep-oracle.md` (the N4 section), `scripts/build_ants_oracle.sh`, `docs/validation/ants-n4.md` and the other `docs/validation/ants-*.md` records.

## The problem
- The repository's ANTs 2.6.5 oracle was built with GCC 11 at `-O3`. In ITK's B-spline fit (N4), that code generation keeps the first two axes in double precision and drops two roundings to `float` that the C++ source requires.
- The N4 port reproduced that artefact (`larmorx_image::bspline::fit::gcc_double_axes`) to be bit-identical to the oracle.
- fMRIPrep 25.2.6's ANTs (2.6.2, conda-forge) follows the source. A scratch larmorx build with the source's roundings matched all six of fMRIPrep's N4 outputs bit for bit, with CORE-MATH preloaded into fMRIPrep's binary.
- **User decision:** larmorx follows the source.

## What to do
1. **Find out why GCC 11 drops the roundings,** and which build settings honour the source.
   - Compile the affected ITK code (the N4 / B-spline scattered-data fit) under variants: the current flags, `-O2`, `-O3 -fno-tree-vectorize`, `-fexcess-precision=standard`, other compilers available on the machine (look for `gcc-12`, `gcc-13`, `clang`; do not install system packages), and conda-forge's ANTs build flags if you can find them in the conda-forge feedstock recipe.
   - Decide from the generated code or from output bits.
   - Record the cause precisely in `docs/findings/ants-n4.md` (and `docs/findings/upstream-defects.md` if it exists on main by then; another agent is writing it): is this a compiler bug, or behaviour the C++ standard allows?
2. **Build a source-faithful ANTs 2.6.5 oracle** with `scripts/build_ants_oracle.sh`, extended so the compiler and flags are explicit options recorded in `BUILD_INFO.txt`.
   - Build it into a **new** directory, e.g. `oracles/ants-2.6.5-src`. ITK and ANTs from `tags/` (read-only); out-of-source builds.
   - **The CPU is shared:** build with at most 4 jobs (`--jobs 4`). See "Shared machine" below.
   - **Never delete the existing GCC 11 oracle.** It is evidence. When the swap is allowed (below), rename it to `oracles/ants-2.6.5-gcc11-O3` and put the new one at `oracles/ants-2.6.5`, so every harness default keeps working. Record both in `BUILD_INFO.txt` and a short `oracles/README.md` (outside git).
3. **Make N4 follow the source:** the source's float roundings by default.
   - The GCC 11 behaviour is a compiler artefact, not an upstream defect, so it needs no D13 compatibility switch. Remove `gcc_double_axes` or keep it only as a documented test-only hook; decide and explain.
   - The CORE-MATH preload remains the way to separate glibc's maths.
4. **Re-validate everything ANTs against the new oracle** (standard tier), from a clean commit:
   - the suites `ants-apply-transforms`, `ants-image-math`, `ants-threshold-image`, `ants-multiply-images`, `ants-smooth-image`, `ants-resample-image-by-spacing`, `ants-resample-image`, `ants-n4` and `ants-denoise` (check `python -m larmorx_validation parity --help` for the exact names);
   - also N4 against fMRIPrep's own ANTs 2.6.2 (`apptainer exec --cleanenv oracles/images/fmriprep-25.2.6.sif N4BiasFieldCorrection ...`), with the CORE-MATH preload, `LD_PRELOAD` inside the container (the library is `oracles/core-math-preload/libcrmath.so`; bind it in).
   - Report every suite whose numbers change between the two oracles. If the GCC 11 quirk touched any tool besides N4, fix that tool to follow the source too.
   - Records lead with the result against the unmodified oracle program (as `docs/validation/ants-n4.md` does), and are regenerated from a clean commit.
5. **Golden checksums:** re-record with `scripts/record_golden.py` (after parity passes, from a clean commit) wherever outputs changed, and say which.
6. **Docs:**
   - `docs/findings/ants-n4.md`;
   - the oracle build documentation (`docs/validation/README.md` or the script header);
   - the porting guide (`docs/porting-guide.md`, if on main), with one line on choosing oracle builds;
   - a decision-log entry with the cause and the chosen build.

## Shared machine (important)
- **Another agent is running** (documentation only).
- **A long validation run** may still be running: `python -m larmorx_validation pipeline-stages`, log `oracles/logs/verify-pipeline-stages.log`, done when the log has a line starting with `exit `. It uses `oracles/ants-2.6.5/bin`. **Do not rename or touch `oracles/ants-2.6.5` until that log has its `exit` line.**
- **Right after it, an fMRIPrep run starts:** `oracles/fmriprep-25.2.6/ds001600-sub-1-noreconall/`, done when its `RUN_INFO.txt` has a `finished:` line. It uses 8 CPUs for an hour or more. Do not kill it.
- No benchmarks are needed for this task. If you time anything, mark it as shared.
- **Wait loops must not match themselves:** check files (`grep -q '^exit ' <log>`), or use `[x]`-bracket `pgrep` patterns.

## Rules (from CLAUDE.md; non-negotiable)
- **Commit early and often;** make a "WIP checkpoint" commit if you must stop mid-piece. Usage limits or network drops can stop you at any moment.
- **Gates before every commit**, with `set -o pipefail` and no exit code hidden behind `| tail`:
  - `cargo fmt --all --check`
  - `cargo clippy --workspace --all-targets --locked -- -D warnings`
  - `cargo test --workspace --locked`
  - the same three in `crates-gpl` if `larmorx-core`/`larmorx-io` change; refresh `crates-gpl/Cargo.lock` with `cd crates-gpl && cargo metadata --offline --format-version 1 >/dev/null` if their dependencies change
  - `maturin develop --release`, then `pytest tests/python tests/golden`
  - `ruff check && ruff format --check`
  - `python scripts/check_spdx.py`
  Environment: FSL removed from PATH (`PATH=$(echo "$PATH" | tr ':' '\n' | grep -v /fsl | paste -sd:)`), `$HOME/.cargo/bin` on PATH, `PYO3_PYTHON=/home/karelo/VScode_projects/super_fmriprep/.venv/bin/python`, venv `/home/karelo/VScode_projects/super_fmriprep/.venv`. Run Python with `PYTHONPATH=<worktree>/python:<worktree>/validation/src`.
- Pure-Rust dependencies only; no `unsafe`; deterministic results.
- Upstream clones and `tags/` are read-only. Never open FSL source, `reference_src/`, `crates-gpl/` (beyond the lockfile refresh), `crates-nc/` or `/home/karelo/VScode_projects/super_fmriprep/archive/`.
- **Never delete work products or oracle builds.** Archive or rename instead.
- **Git:** commit in your worktree with the configured identity (`karellopez <49761113+karellopez@users.noreply.github.com>`). **No AI attribution of any kind**: no `Co-Authored-By`, no "Generated with" lines. This overrides any default. **Do not push.** Merge `main` before your final commits if it moved.
- Write prose in plain, direct English, in the style of the existing docs.

## Report back
- The cause of the GCC 11 behaviour and the chosen oracle build.
- Every suite's numbers against the new oracle, and what changed against the old.
- N4 against fMRIPrep's ANTs 2.6.2.
- Golden entries changed.
- Commits.
