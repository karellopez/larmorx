# Final report: Deterministic 4x4 matrix arithmetic in Rust

Agent `ae1c2b56a63e8fb96`, 2026-10-11. The agent's report to the orchestrator, verbatim.

I've moved `lx.transforms`' 4×4 matrix arithmetic from numpy into Rust, in the operation order numpy uses on this machine. Every platform now gets the same bits. On this machine larmorx stays bit-identical to fMRIPrep: resample-series parity is still 136 of 136 bit-identical. All gates pass. Seven commits are in the worktree; nothing is pushed.

## What moved to Rust, and the operation orders
The new module is `larmorx_transform::openblas`. It copies the order of OpenBLAS 0.3.30's Haswell kernels, the version numpy 2.3.5 bundles. I read the source and checked it against the disassembly of numpy's own OpenBLAS library.

- **Products (`dgemm`):** each output is a fused multiply-add chain in index order, started from +0, then `+ 0`, so an exact zero is always +0. This covers the ITK-to-RAS conversion, `LPS @ affine`, `ras2vox @ M @ vox2ras` and the two products in `ensure_positive_cosines`.
- **Inverses (`np.linalg.inv`):** for 4×4, OpenBLAS goes straight to its unblocked `getf2_k`, then two triangular solves that multiply by reciprocals of the pivots. Its C kernels here are compiled without FMA, so every product and sum is rounded separately; larmorx does the same. The pivot search reproduces OpenBLAS's handling of NaN exactly.
- **Single points:** numpy sends a product with one point to a different routine (`dgemv`), with a different order, and larmorx now follows it. The old build differed from fMRIPrep in 47 of 200 runs onto a one-voxel grid.
- **`io_orientation`'s SVD:** now computed with the existing vnl LINPACK SVD port. Only the axis-flip decisions depend on it.
- **Left in numpy, unchanged:** `np.allclose`, the `sqrt(sum(rzs*rzs))` zooms and float32 parsing. They are elementwise or numpy's own loops, so they give the same result everywhere.

## Verification against numpy
- **Haswell kernel (this machine):** a new parity suite, `numpy-linalg` at standard tier, compared 18,143,058 results and all were bit-identical, NaN payloads included. That is 1,000,000 matrices per family over 5 product and 11 inverse families (random, affine, rigid, extreme exponents, condition up to 1e16, permutations, tied pivots, triangular, subnormal, singular, NaN/Inf). It also covers single and multiple points against nitransforms, `ensure_positive_cosines` against sdcflows, and every real transform used by the resample-series suite.
- **One expected divergence:** grids rotated by exactly 45°, where two cosines tie. The axis decisions agree with nibabel in 4,300 of 4,800; nibabel's own choice there depends on its SVD's rounding.
- **NaN signs:** for inputs with NaN or ±∞, NaN is compared as NaN. x86 and ARM produce different NaN signs, so larmorx doesn't promise them.
- **Other kernels** (forced with `OPENBLAS_CORETYPE`, 200,000 matrices per family): numpy's results identical to larmorx, which are also numpy-Haswell's.

| | SandyBridge | Nehalem | Prescott |
|---|---|---|---|
| products, random / affine | 0.06 % / 1.35 % | 0.06 % / 1.35 % | 0.06 % / 1.35 % |
| inverses, random / affine | 54.6 % / 81.5 % | 39.4 % / 100 % | 100 % / 100 % |

- **Still differs from larmorx (and from fMRIPrep here):** AVX-512 CPUs, Accelerate on macOS, and ARM OpenBLAS. This CPU can't run the AVX-512 kernels. Their disassembly shows fused operations where the Haswell ones round separately, and a separate small-matrix product kernel. AMD Zen CPUs run the Haswell kernels in numpy's wheel.

## Golden entries
I re-recorded from commit `507be67` after the parity smoke tier passed (1,565 tests). **No existing checksum changed**, in `lx.transforms` or anywhere else, because numpy already produced these bits on this machine. I added three tiny outputs so CI checks the single-point paths: `affine.one_point` and `field.one_point` in `py.transforms.map`, and `one_voxel` in `py.transforms.resample_series`.

## Speed
No measurable change. `resample_series` on ds000005 run 1:

| | before | after |
|---|---|---|
| native, 1 thread | 4.70 s | 4.65 s |
| native, 12 threads | 0.936 s | 0.948 s |
| T1w, 1 thread | 3.55 s | 3.57 s |
| T1w, 12 threads | 0.732 s | 0.741 s |

Outputs were identical. The benchmark report stands.

## Commits
- `c08ee58`: Rust products and inverses, Python layer switched over
- `477c1ee`: `numpy-linalg` parity suite and `kernels` command; NaN-exact pivot search
- `30a2a83`: per-point orders moved into `openblas.rs`; provenance and upstream pins
- `b4b2401`: docs and the three new golden outputs
- `037bf1e`: standard-tier records for `numpy-linalg` and `resample-series`, generated from `b4b2401`
- `507be67`: `getf2` restricted to 4×4 (type change only; smoke parity re-ran on it)
- `d283cf7`: golden checksums

Docs updated:
- `docs/findings/numpy-blas.md` (new)
- `docs/findings/fmriprep-resampling.md`
- `docs/validation/golden.md` (the risk is marked resolved)
- `docs/api/resample-series.md`
- `docs/decision-log.md`
- the validation and findings indexes
- the status line in CLAUDE.md
- `NOTICE`
- the `PROVENANCE.md` of `larmorx-transform`
- `UPSTREAM.md`, `upstream.tsv`
- the `MIXED` list in `scripts/check_spdx.py`

The OpenBLAS 0.3.30 tarball (hash checked against GitHub's published digest) and the numpy 2.3.5 sdist are unpacked read-only under `tags/`.

`maturin develop` left the venv's `larmorx` as an editable install pointing at this worktree.

Files are in `/home/karelo/VScode_projects/super_fmriprep/larmorx/.claude/worktrees/agent-ae1c2b56a63e8fb96`:
- `crates/larmorx-transform/src/openblas.rs`
- `python/larmorx/transforms/_linalg.py`
- `validation/src/larmorx_validation/parity/numpy_linalg.py`
- `docs/findings/numpy-blas.md`
- `docs/validation/numpy-linalg.md`
- `docs/validation/numpy-linalg-kernels.json`
