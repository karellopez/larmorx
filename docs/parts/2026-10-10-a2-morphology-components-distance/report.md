# Final report: A2 morphology + components/distance

Agent `a40c451ffc40726e6`, 2026-10-10. The agent's report to the orchestrator, verbatim.

## A2 morphology and components groups: report

Both groups are done and committed in the worktree branch `worktree-agent-a40c451ffc40726e6`, with `main` merged in. Nothing was pushed, and no commit carries AI attribution. All gates passed before every commit:
- `cargo fmt --check`, `clippy -D warnings`, `cargo test --workspace`
- `maturin develop --release` and `pytest tests/python` (209 passed)
- `ruff`, `check_spdx`

**Overall parity.** The regenerated `docs/validation/ants-image-math.md` (standard tier) has 327 cases with no failures: 302 pass, 24 are rejected by both programs, 1 is an expected divergence. Of the 301 compared outputs, 296 are bit-identical, and all 301 have ANTs' exact header bytes.

### Group A: morphology (102 cases)
| Op | Cases | Compared / bit-identical | Notes |
|---|---|---|---|
| MD, ME, MO, MC | 38 | 37 / 37 | 1 both-error (radius -1, ANTs `bad_alloc`) |
| GD, GE, GO, GC | 30 | 30 / 28 | the 2 others are designed signed-zero cases |
| FillHoles | 17 | 16 / 16 | 1 expected divergence |
| PadImage | 17 | 14 / 14 | 3 both-error (ANTs crashes) |

All four groups were also run in 2D and 4D, and on real fMRIPrep masks and a raw T1w.

The three residual differences:
- **`GD`/`GC` signed zeros (0 ulp).** On an input made to hold `-0.0` next to `+0.0`, up to 820 of 1,680 voxels differ, and only in the sign of a zero result. ITK's moving histogram keeps both zeros under one `std::map` key, so its sign depends on the order the histogram was filled (and on ITK's thread split). larmorx returns `+0.0` for a zero maximum and `-0.0` for a zero minimum.
- **`FillHoles` with `holeparam ≤ 1` and a hole on the first or last slice.** ANTs reads outside the image's memory there, so its result is undefined. larmorx refuses with a message (the one expected divergence). sMRIPrep only calls `FillHoles 2`.

### Group B: components and distance maps (63 cases, 59 compared, all bit-identical)
- `GetLargestComponent`: 16 cases (ties, empty mask, every minimum-size edge, 2D, 4D, real masks and the WM probability map).
- `D` (Danielsson): 13 cases.
- `MaurerDistance`: 13 cases.
- Cheap neighbours added: `ExtractContours` (10), `ThresholdAtMean` (6), `ReplaceVoxelValue` (5).
- `ClusterThresholdVariate` was not ported: ANTs decides with a value read outside the image. It still answers "not supported yet".

### Benchmarks (`docs/benchmarks/ants-morphology.md`, load 0.54, larmorx vs ANTs on one thread)
The group A run from the earlier session was taken at load 2.33, so I re-measured everything together on a quiet machine. On a 1 mm brain mask:

| Op | larmorx (1 thread) | ANTs (1 thread) | larmorx (12 threads) |
|---|---|---|---|
| MD 2 | 130 ms | 248 ms | 40 ms |
| MD 5 | 134 ms | 287 ms | 40 ms |
| ME 2 | 279 ms | 589 ms | 71 ms |
| ME 10 | 278 ms | 956 ms | 72 ms |
| MC 4 | 414 ms | 1.08 s | 114 ms |
| FillHoles 2 | 68 ms | 2.91 s | 37 ms |
| PadImage 10 | 98 ms | 126 ms | 99 ms (I/O-bound) |
| GetLargestComponent | 55 ms | 240 ms | 29 ms |
| D (sequential in both) | 547 ms | 2.90 s | — |
| MaurerDistance | 213 ms | 518 ms | 58 ms (ANTs 115 ms on 12) |

On a raw T1w: `GD 2` takes 170 ms against 1.84 s, and `GO 2` 277 ms against 2.75 s. ANTs gains nothing from threads on the binary operations.

### How the main ones are implemented
- **Binary morphology** uses an exact integer distance transform. ITK's ball is exactly the offsets with `Σo² ≤ r(r+1)`, verified up to radius 65535.
- **Grayscale morphology** splits the ball into runs along x and reuses a running maximum per run width.
- **Danielsson** replays ITK's reflective sweeps in the same order on one thread, because its result depends on that order.
- **Maurer** reproduces ITK's float expressions exactly, in parallel over lines.

### Scaffold changes (additive only)
- `larmorx-image/lines.rs`: a generic `for_each_line` helper.
- `larmorx-py`: `gaussian::checked`, `resample::ants_input` and `output_with_spacing` are now `pub(crate)`.
- New ImageMath groups `morphology` and `components`; `ThresholdAtMean` and `ReplaceVoxelValue` went into the existing intensity group.
- New Python modules `lx.ants.morphology` and `lx.ants.components`, with stubs in `_core.pyi`.
- Bench suite `ants-morphology`; new generated inputs in `ants_inputs.py`.

New Python functions: `morphology`, `morphological_{dilate,erode,open,close}`, `grayscale_{dilate,erode,open,close}`, `fill_holes`, `pad_image`, `largest_component`, `distance_map`, `maurer_distance`, `extract_contours`, `threshold_at_mean`, `replace_voxel_value`.

### Commits
- `8252e61` morphology code
- `2fe3e9e` morphology docs and record
- `b479ddc` components code
- `57245be` ExtractContours
- `3fcd07d` components docs, record and benchmark
- `59fbc4c` merge of `main` (golden-test rule, CI `paths-ignore`)

### Staged test data
Six new seeded inputs: `hole-first-slice-uint8`, `holes-labels-4d`, `signed-zeros-float32`, `zeros-uint8`, `equal-components-uint8`, `sparse-points-uint8`. They are already defined in `validation/.../parity/ants_inputs.py`. I checked they rebuild identically across processes, and staged them in the testdata generator's format, with tags and SHA-256 of the uncompressed content, at `<scratchpad>/staged-testdata/ants_filters_synthetic_additions.py`. The scratchpad is session-only, but the repo copy is enough to redo it.

### Left of A2, and other gaps
- **Not done in A2:** the shrink filter (registration pyramids) and histogram matching (`HistogramMatchingImageFilter`, ImageMath `HistogramMatch`). Every ImageMath op the fMRIPrep, sMRIPrep and sdcflows code calls is now ported.
- **Golden cases:** none were written for the new ops, as you said a separate task will add them. Small seeded inputs like the ones above would fit them.
- **CLAUDE.md:** I did not touch it; its status line still needs "A2 morphology and components done".

### Where things are
- Findings: `docs/findings/ants-morphology.md`.
- Validation record: `docs/validation/ants-image-math.{md,json}`.
- Benchmark: `docs/benchmarks/ants-morphology.{md,json}`.
- API: `docs/api/ants-image-math.md`.
- Provenance rows: `crates/larmorx-image/PROVENANCE.md`, `crates/larmorx-ants/PROVENANCE.md`.
- Code: `crates/larmorx-image/src/{morphology,components,distance}.rs` and `crates/larmorx-ants/src/{image_math,cli/image_math}/{morphology,components}.rs`.
