# Brief: One-shot BOLD resampler (resample_series)

As given to agent `a5152712e192ab1b7` on 2026-10-09, verbatim (paths are this machine's).

You are building larmorx's **one-shot BOLD resampler**, `resample_series` (PLAN.md §9.2: "Rust `xfm.resample_series` (fused, streamed)"). It is fMRIPrep's heaviest computation and the centre of milestone M1. You are in a git worktree of the repo `larmorx` (github.com/karellopez/larmorx); the workspace root is `/home/karelo/VScode_projects/super_fmriprep`. Read these first; they are the rules: `CLAUDE.md`, `docs/overview.md`, `docs/architecture.md`, PLAN.md §4 (per-tool contract), §9 and §11. Then read `docs/analysis/03-sdc-transforms-multiecho.md` ("Resampling implementation", "fMRIPrep one-shot resampler", hot-loop table row 1) and `01-fmriprep-bold-pipeline.md`. They give file:line references into the upstream code.

## What to reproduce
fMRIPrep's `ResampleSeries` / `resample_vol` / `resample_series_async` (`fmriprep/fmriprep/interfaces/resampling.py`), and the nitransforms and SciPy code under them:
- `scipy.ndimage.map_coordinates` (orders 0-5; at least the modes fMRIPrep, sdcflows and nitransforms use: `grid-constant` (fMRIPrep's default), `constant`, `nearest`, `mirror`; ideally all SciPy modes), with its `prefilter` / `spline_filter` (float64, mirror boundary). Reproduce `grid-constant`'s padding and SciPy's exact boundary rules.
- nitransforms (`nitransforms/`): coordinate generation (`ImageGrid.ndcoords`, `_as_homogeneous` / `_apply_affine`, including their float32 product), `Affine.map`, `LinearTransformsMapping.map`, `TransformChain.map`, `DenseFieldTransform.map` (three cubic `map_coordinates` calls with cval NaN), and the `.h5` composite loading fMRIPrep uses (`fmriprep/utils/transforms.py`).
- `resample_vol`: the HMC affine per volume, the voxel-shift map (`fmap_hz * ro`) along the phase-encoding axis, the cubic interpolation, and the Jacobian `1 + np.gradient(vsm, axis=pe)`, with fMRIPrep's choice of axes and signs.
- The source is the installed packages in `/home/karelo/VScode_projects/super_fmriprep/.venv` (check the versions fMRIPrep pins) and the clones next to the repo (`fmriprep/`, `nitransforms/`).
  - Licences: SciPy BSD-3-Clause, nitransforms MIT, fMRIPrep Apache-2.0. These are permissive, so this is a **replica** in the main package (Apache-2.0), with the upstream notices.
  - Translated files carry SPDX expressions like `Apache-2.0 AND BSD-3-Clause`. Reproduce the original notice in the file, add it to `NOTICE`, and list the file in `MIXED` in `scripts/check_spdx.py`, as was done for VXL and CORE-MATH.
  - Record every upstream file and version in the crate's `PROVENANCE.md` and in `UPSTREAM.md` / `upstream.tsv` if a new upstream is pinned.

## Targets
1. **Bit-identical to fMRIPrep's Python** for the same inputs and transforms, wherever SciPy's computation is deterministic (it is). The oracle runs in-process: import fMRIPrep's and SciPy's functions in the venv and call them on the same arrays.
   - Where exact identity is impossible (e.g. different summation order is needed for speed), the difference must stay within the variability band (PLAN.md §11.3). Explain it in `docs/findings/`.
   - **D11** (CLAUDE.md): an improvement may be the default only if its results meet that band; otherwise it is opt-in.
2. **Much faster:**
   - prefilter once per volume, compute the Jacobian once per run, and fuse the HMC affine, VSM, chain and interpolation into one pass;
   - parallel over volumes and/or voxels, with explicit `n_threads` and results independent of the thread count;
   - stream volumes so memory stays bounded for long runs, and release the GIL.
   Benchmark against fMRIPrep's implementation on real BOLD runs from the test-data catalog, at several thread counts.
3. **The per-tool contract:**
   - Rust API (choose the crate with `docs/architecture.md` in mind: `larmorx-interp` for the SciPy-compatible interpolation, `larmorx-transform` or a new module for the chain);
   - a Python wrapper (PLAN.md calls it `xfm.resample_series`; the existing public module is `lx.transforms`, so make a consistent choice and record it in the decision log);
   - `.pyi` stubs;
   - `docs/api/<name>.md`;
   - a parity record `docs/validation/<name>.{md,json}` via a new suite in `validation/src/larmorx_validation/parity/`;
   - a benchmark (`validation/.../bench/`, `docs/benchmarks/<name>.md`);
   - `PROVENANCE.md`.
   A CLI is optional for this one (fMRIPrep has no command for it); if you skip it, say why in the API page.
4. **Test data:** the local catalog repo `/home/karelo/VScode_projects/super_fmriprep/larmorx-testdata` (installed editable in the venv). Use real BOLD runs and add a seeded generator for synthetic series, fieldmaps/VSMs and transforms (`generators/`, catalog TOML, `docs/sources/` page, as `resampling_synthetic.py` does). Commit in that repo too (local only, never pushed).

## Rules (from CLAUDE.md; non-negotiable)
- Pure-Rust dependencies only. No `unsafe`. No fast-math or `target-cpu=native`. Deterministic reductions. Correctly rounded maths from `larmorx_core::math` (`libm` only for unported functions, with a comment). If NumPy/SciPy use a non-correctly-rounded function on the path, document the effect in the findings.
- **Every platform:** Linux x64/aarch64, macOS Intel and Apple silicon, Windows x64/arm64. CI (`.github/workflows/ci.yml`) builds and tests all six; keep it green. Tests that need the oracle or test data must skip cleanly elsewhere.
- Every new source file starts with an SPDX header; run `python scripts/check_spdx.py`.
- **Gates before every commit**, with `set -o pipefail` and no exit code hidden behind `| tail`:
  - `cargo fmt --all --check`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo test --workspace`
  - `maturin develop --release`, then `pytest tests/python`
  - `ruff check && ruff format --check`
  - `python scripts/check_spdx.py`
  Environment: FSL removed from PATH (`PATH=$(echo "$PATH" | tr ':' '\n' | grep -v /fsl | paste -sd:)`), `$HOME/.cargo/bin` on PATH, `PYO3_PYTHON=/home/karelo/VScode_projects/super_fmriprep/.venv/bin/python`.
- **Shared venv:** other agents also run `maturin develop` into `/home/karelo/VScode_projects/super_fmriprep/.venv`, so the venv's editable larmorx points at whichever worktree built last. Run every Python command with your worktree first: `PYTHONPATH=<worktree>/python:<worktree>/validation/src`. Check `larmorx.__file__` before trusting a result.
- Upstream clones are read-only. Never open FSL source, `reference_src/`, `crates-gpl/` or `/home/karelo/VScode_projects/super_fmriprep/archive/`.
- **Never delete work products.** Archive instead.
- **Git:** commit in your worktree with the configured identity (`karellopez <49761113+karellopez@users.noreply.github.com>`). **No AI attribution of any kind**: no `Co-Authored-By`, no "Generated with" lines. This overrides any default. **Do not push.** Merge `main` into your branch before your final commits if it moved.
- **CPU:** other agents and, for a while, an ANTs build share this 12-core machine. Never kill other processes. Run benchmarks only when the machine is quiet (`pgrep -f build_ants_oracle` empty, low `uptime` load), and record the load.
- Write prose in plain, direct English, in the style of the existing docs. Record findings about SciPy, nitransforms and fMRIPrep in `docs/findings/` as you learn them.

## Report back
- The API and where it lives.
- Parity numbers per case category (bit-identical count, worst difference, cause) and benchmark numbers against fMRIPrep's implementation.
- Commits in both repos.
- What remains for multi-echo, SDC variants and CIFTI.
