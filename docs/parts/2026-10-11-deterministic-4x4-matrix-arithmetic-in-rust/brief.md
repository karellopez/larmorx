# Brief: Deterministic 4x4 matrix arithmetic in Rust

As given to agent `ae1c2b56a63e8fb96` on 2026-10-10, verbatim (paths are this machine's).

You are making larmorx's `lx.transforms` give **the same bits on every platform**, by moving its 4×4 matrix arithmetic from numpy into Rust with a fixed operation order. You are in a git worktree of the repo `larmorx` (github.com/karellopez/larmorx); the workspace root is `/home/karelo/VScode_projects/super_fmriprep`. Read these first: `CLAUDE.md` (rules 5, 8 and 10), `docs/validation/golden.md` (its section on the numpy-matrix risk and the options), `docs/findings/fmriprep-resampling.md`, `docs/api/resample-series.md`, `docs/decision-log.md` (the 2026-10-10 resampler and golden-test entries), and the code in `python/larmorx/transforms/` and `crates/larmorx-transform/src/{nitransforms,resample_series}.rs`.

## What CI showed (run 38095330520, golden tests on six platforms)
Everything is bit-identical across all six platforms except four `lx.transforms` cases:

| Case | Difference |
|---|---|
| `py.transforms.resample_series` (`out.float64`) | 36–112 ulp, up to 8e-13 |
| `py.transforms.resample_series.sdc` (`out`) | 21 ulp, 1.9e-14 |
| `py.transforms.files` (`load_transforms`, `composite.affine`, `load_itk_linear.mat`) | 1 ulp |
| `py.transforms.ensure_positive_cosines` (`affine`) | 1 ulp |

- The float32 outputs are identical.
- Linux x64 (the reference, this machine's Haswell-class OpenBLAS kernels) differs from Linux aarch64, Windows arm64, macOS (Accelerate) and, in this run, Windows x64. Windows x64 passed in the previous run, so the runner's CPU changes numpy's kernel choice.
- So numpy's 4×4 products and inverses, and therefore fMRIPrep's own last bits, depend on the BLAS library and the CPU.

## The decision (made; implement it)
Do the 4×4 matrix arithmetic in Rust with a fixed operation order, reproducing what numpy does **on this machine** (OpenBLAS, Haswell-class kernels with fused multiply-add). Then:
- every platform and CPU gives the same bits, and the golden tests pass everywhere;
- larmorx stays **bit-identical to fMRIPrep on this validation machine**, so the resample-series parity stays 136 of 136 bit-identical.

That means:
1. **Products** (4×4 by 4×4, 4×4 by vector, and any other numpy matrix op on the path): fused multiply-add chains in the order OpenBLAS's Haswell/Zen kernel uses. The golden-test agent matched 300 of 300 random products this way. Prove it on millions of random matrices, plus every real transform the parity suite uses.
2. **Inverses** (`numpy.linalg.inv`, i.e. LAPACK `dgesv` through OpenBLAS): port what OpenBLAS actually executes for n = 4 (`dgetrf` and `dgetrs` with its own small-matrix paths and kernels) so the bits match.
   - OpenBLAS is BSD-3-Clause: a permissive replica in the main package. SPDX `Apache-2.0 AND BSD-3-Clause`, notice in the file and in `NOTICE`, the file listed in `MIXED` in `scripts/check_spdx.py`, and a `PROVENANCE.md` row with the OpenBLAS version numpy bundles here (`numpy.show_config()`).
   - Read OpenBLAS's source from a pinned release tarball you download into `/home/karelo/VScode_projects/super_fmriprep/tags/`, recorded in `upstream.tsv`/`UPSTREAM.md` like the other rows. Find out which kernel `numpy` uses here (`OPENBLAS_VERBOSE=2`, `threadpoolctl`).
3. **Any other numpy linear algebra on that path** (determinants, `allclose` decisions, SVD/polar steps in `ensure_positive_cosines`, ITK-to-RAS conversions): either already exact in IEEE arithmetic, or moved to Rust with a fixed order. Document each in the findings.
4. **Verification:**
   - a test against numpy with `OPENBLAS_CORETYPE=Haswell` (or the kernel this machine uses) on many random and edge-case matrices: near-singular, permutations, large and small exponents;
   - the resample-series parity suite at standard tier, still 136 of 136 bit-identical (`python -m larmorx_validation parity resample-series --tier standard --out docs/validation`);
   - a short note on which numpy kernels still differ from larmorx (e.g. the SSE2 or AVX-512 kernels, Accelerate). fMRIPrep's own results differ there too.
5. **Re-record the golden checksums** with `scripts/record_golden.py` once the parity smoke tier passes (it checks the tree is committed and records the commit). Only the `lx.transforms` entries should change; say if anything else does.
6. **Docs:**
   - update `docs/validation/golden.md` (the risk is resolved);
   - update `docs/findings/fmriprep-resampling.md`, or add `docs/findings/numpy-blas.md`, with the measurement: how often numpy's kernels differ from each other, and what larmorx reproduces;
   - update `docs/api/resample-series.md` ("The 4×4 matrices come from numpy" is no longer true);
   - add a decision-log entry.
   Keep the benchmark numbers honest: if the change affects speed measurably, note it.

## Rules (from CLAUDE.md; non-negotiable)
- **Commit early and often**; make a "WIP checkpoint" commit if you must stop mid-piece. Usage limits or network drops can stop you at any moment.
- Pure-Rust dependencies only. No `unsafe`. No fast-math or `target-cpu=native`. `f64::mul_add` is a correctly rounded FMA on every target, so it reproduces hardware FMA bit for bit.
- **Gates before every commit**, with `set -o pipefail` and no exit code hidden behind `| tail`:
  - `cargo fmt --all --check`
  - `cargo clippy --workspace --all-targets --locked -- -D warnings`
  - `cargo test --workspace --locked`
  - the same three in `crates-gpl`, if `larmorx-core`/`larmorx-io` change; refresh `crates-gpl/Cargo.lock` with `cd crates-gpl && cargo metadata --offline --format-version 1 >/dev/null` if their dependencies change
  - `maturin develop --release`, then `pytest tests/python tests/golden`
  - `ruff check && ruff format --check`
  - `python scripts/check_spdx.py`
  Environment: FSL removed from PATH (`PATH=$(echo "$PATH" | tr ':' '\n' | grep -v /fsl | paste -sd:)`), `$HOME/.cargo/bin` on PATH, `PYO3_PYTHON=/home/karelo/VScode_projects/super_fmriprep/.venv/bin/python`, venv `/home/karelo/VScode_projects/super_fmriprep/.venv`. Run Python with `PYTHONPATH=<worktree>/python:<worktree>/validation/src`.
- **CI budget (rule 10):** keep any new golden cases tiny. Heavy verification (millions of matrices) belongs in a local script or a test marked to skip in CI, not in CI.
- Upstream sources are read-only. Never open FSL source, `reference_src/`, or `/home/karelo/VScode_projects/super_fmriprep/archive/`.
- **Never delete work products.** Archive instead.
- **Git:** commit in your worktree with the configured identity (`karellopez <49761113+karellopez@users.noreply.github.com>`). **No AI attribution of any kind**: no `Co-Authored-By`, no "Generated with" lines. This overrides any default. **Do not push**; I push, and CI then checks the six platforms.
- You are the only agent running. Write prose in plain, direct English, in the style of the existing docs.

## Report back
- What moved to Rust, and the operation orders.
- The verification numbers against numpy, by kernel.
- Resample-series parity.
- Which golden entries changed.
- Speed effect.
- Commits.
