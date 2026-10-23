# Brief: A2 morphology + components/distance

As given to agent `a40c451ffc40726e6` on 2026-10-10, verbatim (paths are this machine's).

You are porting the last two groups of milestone A2 (the ANTs image filters) for the larmorx project: **morphology** and **components/distance maps**. You are in a git worktree of the repo `larmorx` (github.com/karellopez/larmorx); the workspace root is `/home/karelo/VScode_projects/super_fmriprep`. Read these first; they are the rules: `CLAUDE.md`, `docs/overview.md`, `docs/architecture.md`, PLAN.md §4 and §6.

## Context: A2 parts 1 and 2 are merged, and you build on their scaffold
Read before writing code:
- `crates/larmorx-ants/src/cli/image_math/mod.rs`: the ImageMath dispatcher and registry. Its module docs say how to add an op group: the Rust API in `larmorx-ants/src/image_math/<group>.rs` or ITK filters in `larmorx-image`; `cli/image_math/<group>.rs` with `OPERATIONS`; registration in `GROUPS`; parity cases; a Python wrapper. Ops ANTs has but larmorx lacks currently answer "not supported yet".
- `crates/larmorx-ants/src/image.rs`: `AntsImage<T>` (`derived`, `with_data`, `view()`), `Pixel` (incl. `from_f64_bounded`), `OutputImage`, `ImageStore`.
- `crates/larmorx-image`: `Volume`/`VolumeRef` (now D-dimensional), `gaussian`, `discrete_gaussian`, `median`, `statistics`, `threshold`, `intensity`, and the stubs `morphology.rs`, `components.rs`, `distance.rs`. Parallel work runs inside `larmorx_core::parallel::with_threads(n_threads, …)` with fixed chunks; results never depend on the thread count.
- The Python pattern: `crates/larmorx-py/src/ants_filters/`, `python/larmorx/ants/`, `_core.pyi`.
- The parity harness: `validation/src/larmorx_validation/parity/ants_programs.py` (`AntsCase`, placeholders, `make_suite`; oracle `oracles/ants-2.6.5/bin`), `ants_inputs.py`, `parity/ants_image_math/` (one module per group, listed in `GROUPS`), `bench/ants_programs.py`, `tests/parity/test_ants_programs.py`, `__main__.py`.
- Findings so far: `docs/findings/ants-image-programs.md`, `ants-gaussian-filters.md`, `ants-cli.md`, `itk-nifti.md`.

## Your groups
ANTs 2.6.5 and ITK 5.4.5 are Apache-2.0, so these are **replicas** in the main package. Port from the pinned tags `tags/ANTs-v2.6.5/` and `tags/ITK-v5.4.5/` (read-only). The target is **bit-identical** output to the oracle binaries in `oracles/ants-2.6.5/bin/`, and faster with threads. Results must be independent of the thread count.

Why: sMRIPrep's brain extraction (`docs/analysis/02-anatomical-smriprep-niworkflows.md`, "Atropos refinement" and the tools table) runs `ImageMath MD 2/4/5`, `ME 10/2/5`, `GetLargestComponent`, `FillHoles 2`, `PadImage ±10` and `MultiplyImages` on masks.

**Group A, morphology** (commit when done, before starting group B):
- ITK: `BinaryBallStructuringElement` (as ANTs builds it: radius, `SetRadiusIsParametric` if used), `BinaryErodeImageFilter`/`BinaryDilateImageFilter` (foreground/background values, boundary handling), `BinaryMorphologicalOpening/Closing`, and the grayscale erode/dilate/opening/closing that ImageMath's `G*` ops use.
- ImageMath ops: `MD`, `ME`, `MO`, `MC`, `GD`, `GE`, `GO`, `GC`, `FillHoles` (ANTs' own algorithm in `ImageMath_Templates.hxx`), `PadImage`, and other cheap neighbours in that family (list what you add).

**Group B, components and distance maps:**
- ITK: `ConnectedComponentImageFilter` (connectivity, label order), `RelabelComponentImageFilter` (ordering by size, ties), `DanielssonDistanceMapImageFilter`, `SignedMaurerDistanceMapImageFilter`, plus whatever else the ops below use.
- ImageMath ops: `GetLargestComponent`, `D` (distance), `MaurerDistance`, `ClusterThreshold` if cheap, the label/component ops fMRIPrep or the brain-extraction recipe need, and cheap neighbours (list them).

**Dimensions:** 3D first. Add 2D/4D where ITK's behaviour is clear (ITK's 4D neighbourhoods include time). Otherwise answer "not supported yet".

## The per-tool contract (CLAUDE.md rule 7)
- Rust API, Python wrappers (idiomatic functions such as `lx.ants.morphological_dilate(img, radius)`, `lx.ants.fill_holes(...)`, `lx.ants.largest_component(...)`, `lx.ants.distance_map(...)`, plus the generic `lx.ants.image_math`), CLI (through ImageMath), and `.pyi` stubs.
- `docs/api/ants-image-math.md` extended (option mapping).
- The validation record regenerated (`docs/validation/ants-image-math.{md,json}`).
- A benchmark on a quiet machine.
- `PROVENANCE.md` rows.
- Findings in `docs/findings/` (behaviour, ANTs/ITK file:line, how it is known, what larmorx does).
- Explain every residual difference.

## Rules (from CLAUDE.md; non-negotiable)
- **Commit early and often.** As soon as a coherent piece passes the gates, commit it. If you must stop mid-piece, make a "WIP checkpoint" commit on your branch. A session limit or network drop can stop you at any moment.
- Pure-Rust dependencies only. No `unsafe`. No fast-math or `target-cpu=native`. Deterministic results. Correctly rounded maths from `larmorx_core::math` (`libm` only for unported functions, with a comment).
- **Every platform:** Linux x64/aarch64, macOS Intel and Apple silicon, Windows x64/arm64. CI builds and tests all six; keep it green. Tests that need the oracle or test data must skip cleanly elsewhere.
- Every new source file starts with an SPDX header; run `python scripts/check_spdx.py`.
- **Gates before every commit**, with `set -o pipefail` and no exit code hidden behind `| tail` (a part-2 commit once slipped through that way):
  - `cargo fmt --all --check`
  - `cargo clippy --workspace --all-targets --locked -- -D warnings`
  - `cargo test --workspace --locked`
  - `maturin develop --release`, then `pytest tests/python`
  - `ruff check && ruff format --check`
  - `python scripts/check_spdx.py`
  Environment: FSL removed from PATH (`PATH=$(echo "$PATH" | tr ':' '\n' | grep -v /fsl | paste -sd:)`), `$HOME/.cargo/bin` on PATH, `PYO3_PYTHON=/home/karelo/VScode_projects/super_fmriprep/.venv/bin/python`, venv `/home/karelo/VScode_projects/super_fmriprep/.venv`. Run Python with `PYTHONPATH=<worktree>/python:<worktree>/validation/src`.
- **Dependency changes:** if you change the dependencies of `larmorx-core` or `larmorx-io`, refresh `crates-gpl/Cargo.lock` with `cd crates-gpl && cargo metadata --offline --format-version 1 >/dev/null`.
- **Test data:** you cannot write to `larmorx-testdata` from your worktree. Prefer the existing catalog (`synthetic-ants-filters` has masks, labels, components and holes; real T1w/BOLD) and `gen:` builders. If you need new files, stage a seeded generator change in your scratchpad, check it is deterministic, and list it in your report.
- Upstream clones and `tags/` are read-only. Never open FSL source, `reference_src/`, `crates-gpl/`, `crates-nc/` or `/home/karelo/VScode_projects/super_fmriprep/archive/`.
- **Never delete work products.** Archive instead.
- **Git:** commit in your worktree with the configured identity (`karellopez <49761113+karellopez@users.noreply.github.com>`). **No AI attribution of any kind**: no `Co-Authored-By`, no "Generated with" lines. This overrides any default. **Do not push.** Merge `main` before your final commits if it moved.
- **CPU:** you are the only agent running. Benchmark when `uptime` shows a quiet machine, and record the load.
- Write prose in plain, direct English, in the style of the existing docs.

## Report back
- Per op: parity (cases, bit-identical, worst difference with cause) and benchmarks.
- Scaffold changes, if any.
- Commits.
- Staged test data.
- What is left of A2.
