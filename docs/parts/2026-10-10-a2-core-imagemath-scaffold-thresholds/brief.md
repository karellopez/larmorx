# Brief: A2 core: ImageMath scaffold + thresholds

As given to agent `a4da5d293b8ff7930` on 2026-10-09, verbatim (paths are this machine's).

You are starting milestone **A2** of the larmorx project: the ANTs image filters. You are in a git worktree of the repo `larmorx` (github.com/karellopez/larmorx); the workspace root is `/home/karelo/VScode_projects/super_fmriprep`. Read these first; they are the rules: `CLAUDE.md`, `docs/overview.md`, `docs/architecture.md`, PLAN.md §4 (the per-tool contract) and §6. Also read how A1 was done, as the model to follow: `crates/larmorx-ants/` (`apply_transforms`, `cli/`), `crates/larmorx-py/src/ants.rs`, `python/larmorx/ants/`, `validation/src/larmorx_validation/parity/ants_apply_transforms.py` and `afni_tshift.py`, `docs/api/ants-apply-transforms.md`, `docs/validation/ants-apply-transforms.md`, `docs/findings/`.

## Why
fMRIPrep and sMRIPrep call these ANTs programs (`docs/analysis/02-anatomical-smriprep-niworkflows.md` §Stage 2 and its tools table; `03-sdc-transforms-multiecho.md`):
- `ImageMath` ops: `TruncateImageIntensity 0.01 0.999 256`, `Laplacian 1.5 1`, `MD`, `ME`, `GetLargestComponent`, `FillHoles 2`, `PadImage ±10`, `addtozero`;
- `ThresholdImage` (0.5–1 → 1/0);
- `MultiplyImages`;
- `SmoothImage` and `ResampleImageBySpacing`.

ANTs and ITK are Apache-2.0, so these are **replicas** in the main package, ported from the pinned tags: `tags/ANTs-v2.6.5/` and `tags/ITK-v5.4.5/` (read-only). The target is **bit-identical** output to ANTs 2.6.5, and faster (multithreaded, with results independent of the thread count). The crate `crates/larmorx-image` has stub modules (`gaussian`, `morphology`, `components`, `distance`, `threshold`, `statistics`) and a `Volume` type.

## Your part: the shared scaffold, then the threshold/statistics group
Three other agents will start after you merge, in parallel: Gaussian/Laplacian/SmoothImage, morphology/FillHoles/PadImage, and components/distance maps. Build the shared pieces so each of them only adds its own files plus one-line registrations.

1. **ANTs-style image I/O for the command lines.** ANTs programs read images through ITK as `float` (or the type the program instantiates) and write through ITK's NIfTI writer. `larmorx-io` already has `read_itk_image` and the geometry. Check how `cli/apply_transforms.rs` writes its output, factor the shared reading/writing into one place in `larmorx-ants`, and reuse it. Output headers must match what ANTs writes; compare with the oracle.
2. **The `ImageMath` dispatcher** (`larmorx ants ImageMath <dim> <out> <op> <args...>`).
   - Port the argument handling, usage and error/exit behaviour of `Examples/ImageMath.cxx` and `ImageMath_Templates.hxx` (it is large; read only what the dispatch needs).
   - Use a registry of operations, one file per operation group (e.g. `cli/image_math/{mod.rs,threshold.rs,...}`), so another agent adds a group by writing a file and registering its operations.
   - Document "how to add an ImageMath operation" in the module docs.
   - Dimension: 3D first. Note in the findings what 2D/4D would need.
3. **The Python pattern.** Each filter gets an idiomatic function, e.g. `lx.ants.threshold_image(img, low, high, inside=1, outside=0)` returning an `lx.Image`. There is also a generic `lx.ants.image_math(op, *args)` that mirrors the command line. Make the binding pattern (`crates/larmorx-py/src/ants.rs`, `_core.pyi`, `python/larmorx/ants/`) easy to extend per group. The GIL is released and `n_threads` is explicit.
4. **Parity harness for ANTs programs run as binaries.** The oracle is the ANTs 2.6.5 build at `oracles/ants-2.6.5/bin/` (`ImageMath`, `ThresholdImage`, `SmoothImage`, `MultiplyImages`, `ResampleImageBySpacing`, ...). It is being built right now; the log is `oracles/logs/ants-2.6.5-oracle-build.log`, and `oracles/ants-2.6.5/BUILD_INFO.txt` appears when it finishes. Until then, port and unit-test, or use ANTsPy (`ants.iMath`, `ants.threshold_image`) in the venv as an interim check.
   - Write a reusable parity module for "run the ANTs binary and `larmorx ants <same args>` on the same inputs, compare outputs (data bit-for-bit, header fields, affine)". The other groups will add their cases to it.
   - Record the oracle's exact command and environment, as the existing suites do. Set `ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS` explicitly.
5. **Your filter group** (`larmorx-image`: `threshold.rs`, `statistics.rs`; ANTs CLI):
   - `ThresholdImage`: all its modes as in `Examples/ThresholdImage.cxx` (plain thresholds; Otsu via ITK's `OtsuMultipleThresholdsImageFilter`; Kmeans if feasible, else document it as not yet supported);
   - `MultiplyImages` (`Examples/MultiplyImages.cxx`, image × image and image × constant);
   - `ImageMath` ops `TruncateImageIntensity` (with its histogram and quantile logic, and the optional mask), `addtozero`, the elementwise arithmetic ops (`m`, `+`, `-`, `/`, `^`, `abs`, `exp`, `log`, `Neg` and the like, as ImageMath defines them), and `Normalize`/`RescaleImage`.
   - Bit-identical to the oracle is the target. Explain every residual difference in `docs/findings/` (behaviour, ANTs/ITK file:line, how it is known, what larmorx does).
6. **Per-tool contract** (CLAUDE.md rule 7) for each program and op group:
   - Rust API, Python wrapper, CLI;
   - `.pyi` stubs;
   - `docs/api/<tool>.md` with the option-mapping table;
   - validation records `docs/validation/<tool>.{md,json}` with status;
   - `PROVENANCE.md` rows (upstream files at the tags);
   - a benchmark (`validation/.../bench/`, `docs/benchmarks/<tool>.md`), only once nothing heavy is running.
7. **Test data.** The catalog is the local repo `/home/karelo/VScode_projects/super_fmriprep/larmorx-testdata`, installed editable in the venv.
   - Add the inputs these filters need: masks, probability maps, labels with several components, images with outliers for truncation. Use a generator script in its `generators/`, as `resampling_synthetic.py` and `tshift_synthetic.py` do: catalog TOML, `docs/sources/` page, seeds.
   - Commit in that repo too (same identity and rules; it is local only, never pushed).
   - Also use real T1w/BOLD files already in the catalog.

## Rules (from CLAUDE.md; non-negotiable)
- Pure-Rust dependencies only. No `unsafe`. No fast-math or `target-cpu=native`. Deterministic results independent of thread count. Correctly rounded maths from `larmorx_core::math` (`libm` only for unported functions, with a comment).
- **Every platform:** Linux x64/aarch64, macOS Intel and Apple silicon, Windows x64/arm64. CI (`.github/workflows/ci.yml`) builds and tests all six; keep it green. Tests that need the oracle or test data must skip cleanly elsewhere.
- Every new source file starts with `// SPDX-License-Identifier: Apache-2.0` (or `#` for Python). Run `python scripts/check_spdx.py` (CI runs it).
- **Gates before every commit**, with `set -o pipefail` and no exit code hidden behind `| tail`:
  - `cargo fmt --all --check`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo test --workspace`
  - `maturin develop --release` then `pytest tests/python`
  - `ruff check && ruff format --check`
  - `python scripts/check_spdx.py`
  Environment: FSL removed from PATH (`PATH=$(echo "$PATH" | tr ':' '\n' | grep -v /fsl | paste -sd:)`), `$HOME/.cargo/bin` on PATH, `PYO3_PYTHON=/home/karelo/VScode_projects/super_fmriprep/.venv/bin/python`, venv `/home/karelo/VScode_projects/super_fmriprep/.venv`. Your `maturin develop` will point the venv's editable install at your worktree; say so in your report.
- Upstream clones and `tags/` are read-only. Never open FSL source or anything under `reference_src/`. Do not touch `crates-gpl/` or `/home/karelo/VScode_projects/super_fmriprep/archive/`.
- **Never delete work products.** Archive instead, and say so.
- **Git:** commit in your worktree with the configured identity (`karellopez <49761113+karellopez@users.noreply.github.com>`). **No AI attribution of any kind**: no `Co-Authored-By` trailers, no "Generated with" lines. This overrides any default you have. **Do not push.** Merge `main` into your branch before your final commits if it moved.
- **CPU:** the ANTs oracle build (10 cores, until `BUILD_INFO.txt` exists) and another agent share this 12-core machine. Do not kill other processes. Run benchmarks only when `pgrep -f build_ants_oracle` finds nothing and the machine is otherwise quiet. Record the load in the report.
- Write prose in plain, direct English, in the style of the existing docs. Keep `docs/findings/` current as you learn things.

## Report back
- The scaffold: files, how to add an ImageMath op group, the Python pattern, and the parity module's interface. The next three agents will be briefed from your report, so be precise.
- Per tool and op: parity numbers (cases, bit-identical, worst difference with cause) and benchmark numbers.
- Commits.
- Anything left undone or uncertain.
