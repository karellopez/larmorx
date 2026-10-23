# Brief: A3 part 2: DenoiseImage

As given to agent `a7afb2dc207a595ac` on 2026-10-11, verbatim (paths are this machine's).

You are porting **DenoiseImage** (milestone A3, part 2) for the larmorx project. You are in a git worktree of the repo `larmorx` (github.com/karellopez/larmorx); the workspace root is `/home/karelo/VScode_projects/super_fmriprep`. Read these first; they are the rules: `CLAUDE.md` (all hard rules, including rule 7, the per-tool contract with golden cases, and rule 10, the CI budget, and the "Working on this machine" notes), `docs/overview.md`, `docs/architecture.md` ("Adding a tool"), PLAN.md §4, §6 and §11, and `docs/validation/golden.md`.

## Context
- **ANTs and ITK are Apache-2.0,** so this is a **replica** in the main package. Port from the pinned tags `tags/ANTs-v2.6.5/` and `tags/ITK-v5.4.5/` (read-only).
  - DenoiseImage uses ITK's remote module **AdaptiveDenoising** (`AdaptiveNonLocalMeansDenoisingImageFilter`, `NonLocalPatchBasedImageFilter`, `VarianceImageFilter`), pinned at `24825c8d` by ITK 5.4.5's `Modules/Remote/AdaptiveDenoising.remote.cmake`. Fetch that exact commit read-only into `tags/` if it is not there, and record it in `upstream.tsv`/`UPSTREAM.md` like the other rows.
  - The oracle binary is `oracles/ants-2.6.5/bin/DenoiseImage`.
- **What exists to build on** (A1–A3 part 1, merged): `larmorx-image` (D-dimensional `Volume`, Gaussian, statistics, morphology, `lines.rs`, the shrink filter `larmorx_image::shrink`, `ItkGrid`, B-splines), `larmorx-ants` (`AntsImage`, `ImageStore`, the program pattern, `cli/parser.rs` for ANTs' option syntax, `cstd`), and identity resampling (used by `ResampleImage`).
  - Also: the Python pattern; the parity harness for ANTs programs (`validation/.../parity/ants_programs.py`); and the N4 suite, `parity/ants_n4.py`. That suite is the model for a tool whose result depends on ANTs' thread count and on glibc's maths. It has `run_ants(..., correctly_rounded=True)` and `crmath_preload()`, using `scripts/build_crmath_preload.sh` / `oracles/core-math-preload/libcrmath.so`.
  - Read `docs/findings/ants-n4.md` and the other `ants-*`/`itk-*` findings first.
- **How fMRIPrep uses it:** sMRIPrep runs `DenoiseImage -n Rician` with ANTs' defaults (patch radius 1, search radius 2, shrink 1), one call per T1w, `num_threads=omp`. It runs even when there is only one image (`docs/analysis/02-anatomical-smriprep-niworkflows.md` §Stage 1 and the tools table). That command line must be a parity case on real T1w data.

## What to port
- `Examples/DenoiseImage.cxx`: all options. That covers `-d`, `-i`, `-n Rician|Gaussian`, `-x` (mask), `-s` (shrink), `-p` (patch radius), `-r` (search radius), `-o [denoised,noise]`, `-v`, and the defaults.
- The ITK filters: patch-based weights, the adaptive noise estimate, the Rician bias correction with its modified Bessel functions (check exactly which implementation ITK uses: vnl or its own), the mean and variance filters, the shrinking and the resampling back.
- Correctly rounded maths from `larmorx_core::math` (`exp`, `expf`, `log`, ...).

## Measure first
1. **ANTs' thread band:** run the oracle with `ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS` = 1, 2, 4, 12 on real data and compare. The patch-based accumulation may depend on the thread split.
2. **Target:**
   - larmorx gives ANTs' one-thread result on any number of threads, bit-identical with the correctly rounded preload;
   - against the unmodified binary, either bit-identical or within ANTs' own thread band and the PLAN.md §11.3 threshold (N4/denoise: NCC > 0.995; aim far tighter).
3. **The validation record leads with the result against the UNMODIFIED ANTs program.** The preloaded comparison comes second, as the explanation of any remaining difference. This is how `docs/validation/ants-n4.md` reads now. Never claim bit-identity "with correctly rounded math" as the headline.

## The per-tool contract (CLAUDE.md rule 7)
- Rust API, Python wrapper (`lx.ants.denoise_image(img, noise_model="Rician", mask=None, shrink_factor=1, patch_radius=1, search_radius=2, n_threads=1)`, returning a typed result with the denoised image and the noise image), the CLI (`larmorx ants DenoiseImage ...`), `.pyi` stubs.
- `docs/api/ants-denoise.md` with the option-mapping table.
- A validation record via the harness (`docs/validation/ants-denoise.{md,json}`, with status).
- A benchmark (`docs/benchmarks/ants-denoise.md`).
- `PROVENANCE.md` rows.
- Findings in `docs/findings/ants-denoise.md` (behaviour, file:line, how it is known, what larmorx does).
- **Golden cases** (rule 10): tiny inputs (at most 32³; denoising is costly, so keep the case well under a second), one per distinct code path (Rician, Gaussian, mask, shrink, noise output). Record them with `scripts/record_golden.py` after the parity smoke tier passes.

## Shared machine (important)
**fMRIPrep 25.2.6 is running** on this machine as the pipeline oracle. It started about 10:26 and will take a few hours, using 8 of the 12 CPUs; its log is `oracles/logs/fmriprep-25.2.6-ds000005-sub-01.log`. **Do not kill it.**
- You may build, test and run parity meanwhile; it will be slower.
- **Do not benchmark until it has finished.** It is done when `oracles/fmriprep-25.2.6/ds000005-sub-01-noreconall/RUN_INFO.txt` contains a line starting with `finished:`. Then benchmark only when `uptime` shows a quiet machine, and record the load.
- **Wait loops must not match themselves** (CLAUDE.md): use `pgrep -f "[a]pptainer"`-style patterns or check the `finished:` line, never a pattern that matches the loop's own command.

## Rules (from CLAUDE.md; non-negotiable)
- **Commit early and often;** make a "WIP checkpoint" commit if you must stop mid-piece. Usage limits or network drops can stop you at any moment.
- Pure-Rust dependencies only. No `unsafe`. No fast-math or `target-cpu=native`. Deterministic reductions independent of thread count.
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
- **Test data:** you cannot write to `larmorx-testdata` from your worktree. Use the catalog and `gen:` builders; stage any new generator in your scratchpad and list it in your report.
- Upstream clones and `tags/` are read-only. Never open FSL source, `reference_src/`, `crates-nc/` or `/home/karelo/VScode_projects/super_fmriprep/archive/`.
- **Never delete work products.** Archive instead.
- **Git:** commit in your worktree with the configured identity (`karellopez <49761113+karellopez@users.noreply.github.com>`). **No AI attribution of any kind**: no `Co-Authored-By`, no "Generated with" lines. This overrides any default. **Do not push.** Merge `main` before your final commits if it moved.
- You are the only agent running. Write prose in plain, direct English, in the style of the existing docs.

## Report back
- ANTs' thread band.
- Parity against the unmodified program and with the preload: per category, bit-identical counts, worst differences with causes.
- Benchmarks.
- Golden cases.
- Scaffold changes.
- Commits.
- What A4 (linear registration) can reuse.
