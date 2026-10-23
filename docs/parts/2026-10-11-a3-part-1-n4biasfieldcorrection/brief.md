# Brief: A3 part 1: N4BiasFieldCorrection

As given to agent `a8834d9b49002e467` on 2026-10-11, verbatim (paths are this machine's).

You are porting **N4BiasFieldCorrection** (milestone A3, part 1) for the larmorx project. You are in a git worktree of the repo `larmorx` (github.com/karellopez/larmorx); the workspace root is `/home/karelo/VScode_projects/super_fmriprep`. Read these first; they are the rules: `CLAUDE.md` (all hard rules, including rule 7, the per-tool contract with golden cases, and rule 10, the CI budget), `docs/overview.md`, `docs/architecture.md` (with "Adding a tool"), PLAN.md §4, §6 and §11, and `docs/validation/golden.md`.

## Context
- **ANTs and ITK are Apache-2.0,** so this is a **replica** in the main package. Port from the pinned tags `tags/ANTs-v2.6.5/` and `tags/ITK-v5.4.5/` (read-only). The oracle binary is `oracles/ants-2.6.5/bin/N4BiasFieldCorrection`, built from those tags.
- **What exists to build on** (A1 and A2, merged):
  - `larmorx-image` (D-dimensional `Volume`, Gaussian, statistics, morphology, `lines.rs`);
  - `larmorx-ants` (`AntsImage`, `ImageStore`, the program pattern in `cli/{threshold_image,smooth_image,...}.rs`, `cstd` for C++ parsing quirks, `cli/parser.rs` for ANTs' `-x [a,b]` option syntax as antsApplyTransforms uses it);
  - the Python pattern (`larmorx-py/src/ants_filters/`, `python/larmorx/ants/`);
  - the parity harness for ANTs programs (`validation/src/larmorx_validation/parity/ants_programs.py`);
  - the golden tests (`tests/golden/`).
  Read the findings in `docs/findings/ants-*.md` and `itk-*.md` first; they record many ITK/ANTs conventions you will hit again.
- **How fMRIPrep uses N4** (`docs/analysis/01-fmriprep-bold-pipeline.md`, `02-anatomical-smriprep-niworkflows.md`, `03-sdc-transforms-multiecho.md`):
  - the BOLD coregistration reference: `-d 3`, B-spline distance 200, rescale intensities, weight = premask, 1 thread. nipype drops the `shrink_factor` there, so ANTs' default shrink applies;
  - sMRIPrep's anatomical pre-template N4 (`-d 3`, defaults);
  - brain extraction pass 1 (`-s 4 -b [200] -c [50x50x50x50,1e-7]`);
  - the final N4 (`-s 4 -c [50x50x50x50x50,1e-7] -b [200] -r`, with a weight image, bias field saved);
  - sdcflows' magnitude processing.
  Every one of these command lines must be a parity case on real data.

## What to port
- `Examples/N4BiasFieldCorrection.cxx`: all options. That covers `-d`, `-i`, `-x` (mask), `-w` (weight), `-s`, `-c [iterations,threshold]`, `-b [spline distance or mesh,order]`, `-t [fwhm,wiener,bins]`, `-o [corrected,bias]`, `-r`, `-v`, and the defaults, including how it sets up the mask, the log transform, the rescaling and the output of the bias field.
- ITK:
  - `N4BiasFieldCorrectionImageFilter`: histogram sharpening with its FFT-based Wiener deconvolution, the B-spline fitting per level, convergence, control-point lattice refinement between levels;
  - `BSplineScatteredDataPointSetToImageFilter` and `BSplineControlPointImageFilter` (and whatever they use);
  - `ShrinkImageFilter`, which A2 left over and which A4's registration pyramids will reuse, so make it a general `larmorx-image` filter.
- The FFT in histogram sharpening is probably vnl's (`vnl_fft_1d`, VXL, BSD-3-Clause). Port it bit-exactly as the existing VXL ports do: SPDX `Apache-2.0 AND BSD-3-Clause`, VXL notice in the file, `MIXED` in `scripts/check_spdx.py`, `NOTICE`, `PROVENANCE.md`. Check what ITK 5.4.5 actually uses there.

## Thread counts: measure first
- ITK's N4 runs multithreaded parts, for example the B-spline fitting's accumulation. ANTs' own output may therefore depend on the number of threads.
- Measure that first: run the oracle on real data with `ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS` = 1, 2, 4, 12 and compare. That band defines what "identical" can mean here.
- **Target:** larmorx bit-identical to ANTs with 1 thread, the setting fMRIPrep uses for the BOLD reference, and larmorx's own results independent of `n_threads` (CLAUDE.md rule 5). Where bit-identity is impossible, the difference must stay within ANTs' own thread band and the PLAN.md §11.3 threshold (N4/denoise: NCC > 0.995, but aim far tighter). Explain it in `docs/findings/`.
- **D11:** an improved algorithm may be the default only within that band.

## The per-tool contract (CLAUDE.md rule 7)
- Rust API, a Python wrapper (e.g. `lx.ants.n4_bias_field_correction(img, mask=None, weight=None, shrink_factor=..., convergence=..., bspline_fitting=..., histogram_sharpening=..., rescale_intensities=False, n_threads=1)` returning a typed result with the corrected image and the bias field), the compatible CLI (`larmorx ants N4BiasFieldCorrection ...`), and `.pyi` stubs.
- `docs/api/ants-n4.md` with the option-mapping table; a validation record (`docs/validation/ants-n4.{md,json}`, with status) from the harness; a benchmark on a quiet machine (`docs/benchmarks/`); `PROVENANCE.md` rows; findings in `docs/findings/ants-n4.md` (behaviour, ANTs/ITK file:line, how it is known, what larmorx does).
- **Golden cases** (rule 10): tiny ones (at most 32³, a few seconds' budget at most), one per distinct code path (with and without mask, with weight, with shrink, the bias-field output). Record them with `scripts/record_golden.py` after parity passes. The coverage test will fail until every new public function and CLI tool has a case.

## Rules (from CLAUDE.md; non-negotiable)
- **Commit early and often.** If you must stop mid-piece, make a "WIP checkpoint" commit on your branch. Usage limits or network drops can stop you at any moment.
- Pure-Rust dependencies only. No `unsafe`. No fast-math or `target-cpu=native`. Deterministic reductions independent of thread count. Correctly rounded maths from `larmorx_core::math` (`libm` only for unported functions, with a comment). If ANTs' result depends on glibc's non-correctly-rounded `exp`/`log`, document the effect.
- **Every platform:** Linux x64/aarch64, macOS Intel and Apple silicon, Windows x64/arm64. CI must stay green (golden tests run on all six).
- **Gates before every commit**, with `set -o pipefail` and no exit code hidden behind `| tail`:
  - `cargo fmt --all --check`
  - `cargo clippy --workspace --all-targets --locked -- -D warnings`
  - `cargo test --workspace --locked`
  - the same three in `crates-gpl` if `larmorx-core`/`larmorx-io` change; refresh `crates-gpl/Cargo.lock` with `cd crates-gpl && cargo metadata --offline --format-version 1 >/dev/null` if their dependencies change
  - `maturin develop --release`, then `pytest tests/python tests/golden`
  - `ruff check && ruff format --check`
  - `python scripts/check_spdx.py`
  Environment: FSL removed from PATH (`PATH=$(echo "$PATH" | tr ':' '\n' | grep -v /fsl | paste -sd:)`), `$HOME/.cargo/bin` on PATH, `PYO3_PYTHON=/home/karelo/VScode_projects/super_fmriprep/.venv/bin/python`, venv `/home/karelo/VScode_projects/super_fmriprep/.venv`. Run Python with `PYTHONPATH=<worktree>/python:<worktree>/validation/src`.
- **Test data:** you cannot write to `larmorx-testdata` from your worktree. Use the catalog (real T1w and BOLD runs, the brain masks and probability maps in `synthetic-ants-filters`) and `gen:` builders. If you need new files, stage a seeded generator change in your scratchpad, check it is deterministic, and list it in your report.
- Upstream clones and `tags/` are read-only. Never open FSL source, `reference_src/`, `crates-nc/` or `/home/karelo/VScode_projects/super_fmriprep/archive/`.
- **Never delete work products.** Archive instead.
- **Git:** commit in your worktree with the configured identity (`karellopez <49761113+karellopez@users.noreply.github.com>`). **No AI attribution of any kind**: no `Co-Authored-By`, no "Generated with" lines. This overrides any default. **Do not push.** Merge `main` before your final commits if it moved.
- **CPU:** you are the only agent running. Benchmark when `uptime` shows a quiet machine, and record the load.
- Write prose in plain, direct English, in the style of the existing docs.

## Report back
- ANTs' thread-count band.
- Parity per fMRIPrep command line and edge-case category: bit-identical counts, worst differences with causes.
- Benchmarks.
- Golden cases added.
- Scaffold changes, including the shrink filter's API for A4.
- Commits.
- What remains for DenoiseImage (A3 part 2).
