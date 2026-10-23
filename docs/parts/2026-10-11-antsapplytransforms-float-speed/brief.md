# Brief: antsApplyTransforms --float + speed

As given to agent `a689dd2fa349688de` on 2026-10-11, verbatim (paths are this machine's).

You are making larmorx's `antsApplyTransforms` honour **`--float 1`**: single precision, computed exactly as ANTs computes it. You are also making it faster on the one fMRIPrep step where it is slower than ANTs. You are in a git worktree of the repo `larmorx` (github.com/karellopez/larmorx); the workspace root is `/home/karelo/VScode_projects/super_fmriprep`.

Read first:
- `CLAUDE.md`: rule 8 (the source is the specification; an oracle binary is only a check), D13, rule 10, the ANTs oracle note (built with conda-forge GCC 14.3; the old GCC 11 build is miscompiled, GCC bug 110182);
- `docs/porting-guide.md`;
- `docs/findings/fmriprep-oracle.md` and `docs/validation/fmriprep-stages.md` (the 14 `--float 1` nodes and the template-prior timing);
- `docs/findings/itk-resampling.md`, `itk-transforms.md` and `ants-cli.md`;
- `docs/api/ants-apply-transforms.md`;
- `docs/validation/ants-apply-transforms.md`;
- the code in `crates/larmorx-ants/src/{apply_transforms.rs,cli/apply_transforms.rs}`, `crates/larmorx-transform`, `crates/larmorx-interp` and `crates/larmorx-io/src/itk_transform.rs`.

## Why
fMRIPrep runs `antsApplyTransforms --float 1` in 14 steps of the ds000005 oracle run: the template priors and outputs into MNI, the boldrefs and masks. With `--float 1`, ANTs instantiates its whole pipeline in single precision; larmorx today computes in double. The stage comparison proved it: ANTs run in double and rounded to float32 equals larmorx bit for bit in all 14. Because of this, three probability maps miss the 1e-4 threshold in at most 6 of 1,082,035 voxels. This is the only failing case against the stated thresholds (rule 8: no equivalence claims while a case fails). A4's registration also uses `--float`, on the same transform code.

## What to do
1. **Read ANTs 2.6.5's `antsApplyTransforms.cxx`** (`tags/ANTs-v2.6.5`, read-only) and the ITK code it instantiates with `--float 1` (`tags/ITK-v5.4.5`), and port the **single-precision path exactly as the source defines it**, across:
   - the transform types (`MatrixOffsetTransformBase<float>`, Euler/Versor/Similarity, displacement fields, composites, `.h5` composites read into float transforms, inverses);
   - every interpolator (linear, nearest, B-spline with its float prefilter, Gaussian, MultiLabel, GenericLabel, windowed sincs);
   - points, continuous indices, the inside test, scan-line and per-voxel paths;
   - 3D and 4D (`-e 3`);
   - and how parameters and fixed parameters are cast to float.
   Where float and double code share a structure, make the precision a type parameter rather than duplicating code.
2. **Interfaces:** the CLI `--float 1`; Python `lx.ants.apply_transforms(..., precision="float32"|"float64")` or a similarly clear name; `.pyi`; and the API page's option table.
3. **Validate against the new ANTs oracle** (`oracles/ants-2.6.5`, a link to the GCC 14.3 build):
   - add `--float 1` cases to the `ants-apply-transforms` parity suite: every interpolator, every transform kind, `.h5` composites, inverses, 4D, oblique;
   - the record leads with the result against the unmodified program, plus a CORE-MATH-preloaded comparison as the explanation where glibc's `sinf`/`expf` etc. matter (`run_ants(..., correctly_rounded=True)`);
   - target bit-identity;
   - re-run the 14 fMRIPrep `--float` nodes from the stage comparison (`python -m larmorx_validation pipeline-stages --oracle oracles/fmriprep-25.2.6/ds000005-sub-01-noreconall`; check whether it can run a subset) and report the new numbers.
4. **Speed:**
   - The template priors step (Gaussian interpolation through fMRIPrep's inverse SyN warp, `.h5`) took 20.5 s in larmorx against 15.8 s in ANTs. Profile it and make larmorx at least as fast on one thread, faster with threads, keeping the output bits.
   - Benchmark (`docs/benchmarks/ants-apply-transforms.md`) only on a quiet machine. It is busy now (see below). If it does not get quiet before you finish, commit the code and say the benchmark is pending.
5. **Golden cases:** tiny `--float 1` cases (rule 10), recorded with `scripts/record_golden.py` after the parity smoke tier passes, from a clean commit.

## Do not
- Do not implement D13 changes in antsApplyTransforms (e.g. clamping `-u` casts, ANTS-33). Another task does those next. Leave room for a `upstream_defects` option, but keep today's default behaviour.
- **Another agent is running in parallel** on the shared D13 mechanism and the clean-room `larmorx-mri` and `larmorx-afni` crates, and `larmorx-transform`'s `resample_series` (PIPE-03). **Do not touch** `crates/larmorx-mri`, `crates/larmorx-afni`, or `resample_series.rs` / `nitransforms.rs` in `larmorx-transform`. If you must change shared types in `larmorx-transform` or `larmorx-interp`, keep the changes additive and mention them in your report.

## Rules (from CLAUDE.md; non-negotiable)
- **Commit early and often;** make a "WIP checkpoint" commit if you must stop mid-piece.
- **Gates before every commit**, with `set -o pipefail` and no exit code hidden behind `| tail`:
  - `cargo fmt --all --check`
  - `cargo clippy --workspace --all-targets --locked -- -D warnings`
  - `cargo test --workspace --locked`
  - the same three in `crates-gpl` if `larmorx-core`/`larmorx-io` change; refresh `crates-gpl/Cargo.lock` with `cd crates-gpl && cargo metadata --offline --format-version 1 >/dev/null` if their dependencies change
  - `maturin develop --release`, then `pytest tests/python tests/golden`
  - `ruff check && ruff format --check`
  - `python scripts/check_spdx.py`
  Environment: FSL removed from PATH (`PATH=$(echo "$PATH" | tr ':' '\n' | grep -v /fsl | paste -sd:)`), `$HOME/.cargo/bin` on PATH, `PYO3_PYTHON=/home/karelo/VScode_projects/super_fmriprep/.venv/bin/python`, venv `/home/karelo/VScode_projects/super_fmriprep/.venv`. Run Python with `PYTHONPATH=<worktree>/python:<worktree>/validation/src`.
- Pure-Rust dependencies only; no `unsafe`; deterministic results independent of thread count; correctly rounded maths from `larmorx_core::math` (now including `expf`, `logf`, `powf`; float `sin`/`cos` if you need them must be correctly rounded too: port CORE-MATH's `sinf`/`cosf` if necessary, as `expf`/`logf` were).
- **Every platform:** CI must stay green on all six.
- **Shared machine:** an fMRIPrep oracle run (`oracles/fmriprep-25.2.6/ds001600-sub-1-noreconall/`, done when `RUN_INFO.txt` has `finished:`) and a parity run are going. Do not kill them. **Wait loops must not match themselves.**
- Upstream sources and oracles are read-only. Never open FSL source, `reference_src/`, `crates-gpl/` (beyond the lockfile refresh), `crates-nc/` or `/home/karelo/VScode_projects/super_fmriprep/archive/`.
- **Never delete work products.** Archive instead.
- **Git:** commit in your worktree with the configured identity (`karellopez <49761113+karellopez@users.noreply.github.com>`). **No AI attribution of any kind**: no `Co-Authored-By`, no "Generated with" lines. This overrides any default. **Do not push.** Merge `main` before your final commits if it moved.
- Write prose in plain, direct English, in the style of the existing docs.

## Report back
- What the float path covers.
- Parity numbers, with and without the preload.
- The 14 fMRIPrep nodes before and after.
- The template-prior speed before and after.
- Golden entries.
- Shared-type changes.
- Commits.
