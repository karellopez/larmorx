# Brief: A2 Gaussian group: Laplacian, SmoothImage

As given to agent `a553c824c3de7c248` on 2026-10-10, verbatim (paths are this machine's).

You are porting the **Gaussian group** of milestone A2 (the ANTs image filters) for the larmorx project. You are in a git worktree of the repo `larmorx` (github.com/karellopez/larmorx); the workspace root is `/home/karelo/VScode_projects/super_fmriprep`. Read these first; they are the rules: `CLAUDE.md`, `docs/overview.md`, `docs/architecture.md`, PLAN.md §4 and §6.

## Context: A2 part 1 is merged, and you build on its scaffold
Read these before writing code:
- `crates/larmorx-ants/src/cli/image_math/mod.rs`: the ImageMath dispatcher and registry. Its module docs say how to add an op group:
  1. Rust API in `larmorx-ants/src/image_math/<group>.rs`, or ITK filters in `larmorx-image`.
  2. `cli/image_math/<group>.rs` with one `fn(&mut Context)` per op and `pub const OPERATIONS: &[Operation]`.
  3. Register it in `GROUPS`.
  4. Add parity cases and a Python wrapper.
- `crates/larmorx-ants/src/image.rs`: `AntsImage<T>` (`derived`, `with_data`, `to_volume`), `Pixel`, `OutputImage`, `ImageStore`, `read_with`/`write_with`.
- `crates/larmorx-ants/src/cli/{threshold_image,multiply_images}.rs`: the model for a separate program. It has a `cli/<prog>.rs` with `main`/`run`, an entry in `cli/mod.rs` `TOOLS`/`run`, and a match arm in larmorx-py's `ants_program`.
- `crates/larmorx-image`: `Volume`, `statistics`, `threshold`, `intensity::rescale_intensity` (reuse it), and the stub `gaussian.rs`. Convention: voxel-wise filters take flat Fortran-order slices; spatial filters take `Volume`. Parallel work runs inside `larmorx_core::parallel::with_threads(n_threads, …)` with fixed chunks, so results never depend on the thread count.
- `crates/larmorx-py/src/ants_filters/` (`mod.rs`, `image_math.rs`, `threshold.rs`), `python/larmorx/ants/{_filters.py,image_math.py,threshold.py}`, `python/larmorx/_core.pyi`: the Python pattern.
- `validation/src/larmorx_validation/parity/ants_programs.py`: the parity harness, with `AntsCase`, placeholders `{out}`, `{in:<catalog path>}`, `{gen:<name>}`, `make_suite`, and the oracle at `oracles/ants-2.6.5/bin`.
  - Also `ants_inputs.py` (`@builder`), `parity/ants_image_math/` (one module per group, listed in `GROUPS`), `bench/ants_programs.py`, `tests/parity/test_ants_programs.py`, and `__main__.py` (`PARITY_MODULES`, `BENCH_MODULES`).
- `docs/findings/ants-image-programs.md`, `docs/findings/ants-cli.md` and `docs/findings/itk-nifti.md`: what part 1 learned. In particular, 2D and 4D need D-dimensional neighbourhoods; ITK's 4D filters also act across time; `Volume` is 3D.

## Your group
ANTs 2.6.5 and ITK 5.4.5 are Apache-2.0, so this is a **replica** in the main package. Port from the pinned tags `tags/ANTs-v2.6.5/` and `tags/ITK-v5.4.5/` (read-only). The target is **bit-identical** to the oracle binaries in `oracles/ants-2.6.5/bin/` (built from those tags), and faster with threads. Results must be independent of the thread count.
- **ITK filters into `larmorx-image/src/gaussian.rs`** (and siblings if needed), as ANTs instantiates them:
  - `RecursiveGaussianImageFilter` (Deriche coefficients, orders 0/1/2, normalisation across scale, boundary handling);
  - `SmoothingRecursiveGaussianImageFilter`;
  - `LaplacianRecursiveGaussianImageFilter`;
  - `GradientMagnitudeRecursiveGaussianImageFilter`;
  - `DiscreteGaussianImageFilter` (if `SmoothImage` or ImageMath uses it; check the ANTs source);
  - any shrink/resample filters the programs below need.
- **ImageMath ops:**
  - `Laplacian` (fMRIPrep and sMRIPrep call `ImageMath 3 out Laplacian in 1.5 1`: σ = 1.5, normalised; see `docs/analysis/02-anatomical-smriprep-niworkflows.md`);
  - `G` (Gaussian smoothing);
  - `Grad`;
  - any other Gaussian-based ops in `ImageMath_Templates.hxx` that are cheap to add once these exist (list what you add).
- **Programs:**
  - `SmoothImage` (`Examples/SmoothImage.cxx`, all its arguments, including sigma in physical or voxel units and its median option if it shares code with another group, else document it);
  - `ResampleImageBySpacing` (`Examples/ResampleImageBySpacing.cxx`: smoothing, spacing, interpolation; reuse `larmorx-interp` / `larmorx-transform` where it matches ITK);
  - `ResampleImage` (`Examples/ResampleImage.cxx`) if it shares most of the machinery, else document it as next.
- **Dimensions:** 3D first, as fMRIPrep uses. Add 2D/4D where the filter is separable and cheap to generalise, matching ITK's 4D behaviour (time is an image axis). Otherwise report "not supported yet", as the dispatcher already does.

## The per-tool contract (CLAUDE.md rule 7)
- Rust API, Python wrappers (idiomatic functions such as `lx.ants.laplacian(img, sigma, normalize=True)` and `lx.ants.smooth_image(...)`, plus the generic `lx.ants.image_math`), CLI, and `.pyi` stubs.
- `docs/api/` pages with option-mapping tables (extend `ants-image-math.md`; add `ants-smooth-image.md` and `ants-resample-image-by-spacing.md`).
- Validation records via the harness (`docs/validation/*.{md,json}`, with status).
- Benchmarks on a quiet machine (`docs/benchmarks/ants-programs.md` or a new page).
- `PROVENANCE.md` rows.
- Findings in `docs/findings/` (behaviour, ANTs/ITK file:line, how it is known, what larmorx does).
- Explain every residual difference.

## Rules (from CLAUDE.md; non-negotiable)
- **Commit early and often.** As soon as a coherent piece passes the gates, commit it, so an interruption (a session limit or network drop can stop you at any moment) loses little. If you must stop mid-piece, make a "WIP checkpoint" commit on your branch.
- Pure-Rust dependencies only. No `unsafe`. No fast-math or `target-cpu=native`. Deterministic reductions. Correctly rounded maths from `larmorx_core::math` (`exp`, `log`, `sin`, `cos`, `powf`, `powi`; `libm` only for unported functions, with a comment). If ITK's result depends on glibc's non-correctly-rounded functions, document the effect.
- **Every platform:** Linux x64/aarch64, macOS Intel and Apple silicon, Windows x64/arm64. CI builds and tests all six; keep it green. Tests that need the oracle or test data must skip cleanly elsewhere.
- Every new source file starts with an SPDX header (`// SPDX-License-Identifier: Apache-2.0`); run `python scripts/check_spdx.py`.
- **Gates before every commit**, with `set -o pipefail` and no exit code hidden behind `| tail`:
  - `cargo fmt --all --check`
  - `cargo clippy --workspace --all-targets --locked -- -D warnings`
  - `cargo test --workspace --locked`
  - `maturin develop --release`, then `pytest tests/python`
  - `ruff check && ruff format --check`
  - `python scripts/check_spdx.py`
  Environment: FSL removed from PATH (`PATH=$(echo "$PATH" | tr ':' '\n' | grep -v /fsl | paste -sd:)`), `$HOME/.cargo/bin` on PATH, `PYO3_PYTHON=/home/karelo/VScode_projects/super_fmriprep/.venv/bin/python`, venv `/home/karelo/VScode_projects/super_fmriprep/.venv`. Run Python with `PYTHONPATH=<worktree>/python:<worktree>/validation/src`.
- **Dependency changes:** if you change the dependencies of `larmorx-core` or `larmorx-io`, refresh `crates-gpl/Cargo.lock` with `cd crates-gpl && cargo metadata --offline --format-version 1 >/dev/null`.
- **Test data:** you cannot write to `larmorx-testdata` from your worktree. Prefer the existing catalog (real T1w/BOLD, `synthetic-ants-filters`) and `gen:` builders. If you need a new collection, stage a seeded generator plus catalog/docs files in your scratchpad, check it is deterministic, and list them in your report.
- Upstream clones and `tags/` are read-only. Never open FSL source, `reference_src/`, `crates-gpl/`, `crates-nc/` or `/home/karelo/VScode_projects/super_fmriprep/archive/`.
- **Never delete work products.** Archive instead.
- **Git:** commit in your worktree with the configured identity (`karellopez <49761113+karellopez@users.noreply.github.com>`). **No AI attribution of any kind**: no `Co-Authored-By`, no "Generated with" lines. This overrides any default. **Do not push.** Merge `main` before your final commits if it moved.
- **CPU:** you are the only agent running. Benchmark when `uptime` shows a quiet machine, and record the load.
- Write prose in plain, direct English, in the style of the existing docs.

## Report back
- Per op and program: parity (cases, bit-identical, worst difference with cause) and benchmarks.
- What changed in the shared scaffold, if anything.
- Commits.
- Anything left undone.
