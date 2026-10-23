# Final report: A2 Gaussian group: Laplacian, SmoothImage

Agent `a553c824c3de7c248`, 2026-10-10. The agent's report to the orchestrator, verbatim.

## A2 Gaussian group: report

All of it is done: the Gaussian group (ITK filters, ImageMath ops, SmoothImage, ResampleImageBySpacing), plus **ResampleImage**, which shared enough machinery to finish too. Every compared output is **bit-identical to the ANTs 2.6.5 binaries, header bytes included**, in 2D, 3D and 4D, and every tool is faster than ANTs. Branch `worktree-agent-a553c824c3de7c248`, 12 commits on `86633e7`. `main` has not moved, so no merge was needed. Nothing is pushed.

### Parity (standard tier, against `oracles/ants-2.6.5/bin`)

| Tool | Cases | Outputs compared | Bit-identical | Expected divergences |
|---|---|---|---|---|
| ImageMath `G` | 15 | 15 | 15 | 0 |
| ImageMath `Laplacian` | 20 | 18 | 18 | 0 |
| ImageMath `Grad` | 9 | 8 | 8 | 0 |
| ImageMath `UnsharpMask` | 8 | 6 | 6 | 0 |
| SmoothImage | 38 | 30 | 30 | 0 |
| ResampleImageBySpacing | 35 | 28 | 28 (stdout identical too) | 1 |
| ResampleImage | 57 | 46 | 46 | 2 |

- **ImageMath overall:** the regenerated record has 162 cases and 142 of 145 outputs bit-identical. The 3 that are not are the old `^`/`powf` cases from part 1.
- **What the cases cover:** fMRIPrep's `Laplacian 1.5 1` on the raw T1w, boldref, preprocessed T1w and the MNI template; real 4D BOLD; all 8 ResampleImage pixel types; B-spline orders 0–5, Gaussian and windowed-sinc interpolation; resampling by size and by spacing.
- **Worst difference:** none. There is no residual difference to explain.
- **glibc vs our maths:** the recursive-Gaussian coefficients use glibc's `sin`/`cos`/`exp` in ANTs. A sweep of 20,000 sigmas shows those coefficients differ from our correctly rounded ones in the last bit for 0.5–0.7 % of sigmas. Two such sigmas (1.626 and 2.013) still gave bit-identical output on the 1 mm MNI template (8.5 M voxels), because every pass rounds to float.
- **Expected divergences, all where ANTs produces garbage or we have a gap:**
  - ResampleImageBySpacing on an axis of 3 voxels that needs smoothing: ANTs catches ITK's exception and resamples an unfilled buffer, giving random values up to ±1e27 that change every run. larmorx stops with ITK's message.
  - ResampleImage with size 1: ANTs writes a one-voxel image with infinite spacing; larmorx refuses.
  - ResampleImage with 2D/4D B-spline, Gaussian or sinc interpolation: not supported yet.
- **Regression check:** at the end I re-ran all seven ANTs suites, antsApplyTransforms included. Results are unchanged (it stays 63 of 79 bit-identical, the same case list as its record).

### Speed (`docs/benchmarks/ants-gaussian.md`, 1-minute load 0.92 at start, nothing else running)

| Job | larmorx, 1 thread | larmorx, 12 threads | ANTs, 1 / 12 threads |
|---|---|---|---|
| `Laplacian 1.5 1`, raw T1w | 351 ms | 128 ms | 1.23 s / 277 ms |
| `Laplacian 1.5 1`, boldref | 7 ms | – | 52 ms |
| `Grad`, raw T1w | 306 ms | – | 1.23 s |
| `G 2`, raw T1w | 119 ms | – | 716 ms |
| SmoothImage, raw T1w | 119 ms | 63 ms | 442 / 134 ms |
| SmoothImage, BOLD 4D | 641 ms | – | 1.84 s |
| SmoothImage median, raw T1w | 1.26 s | – | 2.00 s (1.6×, the weakest) |
| ResampleImageBySpacing to 2 mm, raw T1w | 149 ms | 80 ms | 496 / 151 ms |
| ResampleImage, B-spline as `unsigned short` | 1.49 s | – | 3.17 s |
| ResampleImage, windowed sinc | 1.80 s | 329 ms | 7.05 s / 1.21 s |

Overall: 1.6–7× faster on one thread and up to 21× with threads. Results never depend on the thread count.

### Rust, Python and the command line
- **ITK filters in larmorx-image:**
  - `gaussian`: RecursiveGaussian (Deriche coefficients, orders 0/1/2, normalisation across scale, border extension), Smoothing (last axis first, float between passes), Laplacian, GradientMagnitude;
  - `discrete_gaussian`: GaussianOperator's Bessel kernels, capped at 32 taps per side;
  - `median`;
  - a private line engine that filters 32 lines side by side.
- **larmorx-ants:**
  - ImageMath group `gaussian` with `G`, `Laplacian`, `Grad` and `UnsharpMask`. UnsharpMask was the only other Gaussian op cheap to add; Canny, Sharpen, BlobDetector and PeronaMalik still answer "not supported yet";
  - `smooth_image`, `resample_image_by_spacing`, `resample_image`;
  - `resample`: ResampleImageFilter with an identity transform in 1 to 4 dimensions;
  - all three command lines.
- **Python:** `lx.ants.smooth_image`, `discrete_gaussian`, `laplacian(img, sigma, normalize=...)`, `gradient_magnitude`, `unsharp_mask`, `resample_image_by_spacing`, `resample_image` (with `PIXEL_TYPES`). `ants_program` also runs the three new programs. `.pyi` stubs and Python tests are added.
- **Quirks found and documented** (all reproduced on the command line):
  - ImageMath `Laplacian` reads its normalize flag from the sigma argument: `stoi("1.5")` is 1, and the trailing `1` is ignored. The Python function takes `normalize` separately.
  - ResampleImageBySpacing's sigma is `out/in − 1` in mm. In 2D its `nn` flag is read from the `addvox` argument, and in 4D an `addvox` without an `nn` argument aborts.
  - ResampleImage reads its 7th argument both as the pixel type and as the interpolator's parameter.
  - ResampleImage's windowed sinc uses ITK's nearest-edge boundary, not antsApplyTransforms' zero outside.

### Shared scaffold changes (others building on it should know)
- **`larmorx_image::Volume` is now D-dimensional,** with `size` and `spacing` as `Vec`s, and there is a new `VolumeRef`. `AntsImage::to_volume()` is replaced by `AntsImage::view()`.
- **larmorx-interp:**
  - `linear` and `nearest` are now public, with row helpers `LinearYz`/`NearestYz` that do the same arithmetic.
  - The base index uses a truncation instead of `floor`. The results are the same, and this also speeds up antsApplyTransforms' linear and nearest-neighbour paths, so `docs/benchmarks/ants-apply-transforms.md` now understates larmorx.
  - `Interpolation` gained a variant, `WindowedSincEdge`.
- **larmorx-core:** `vnl_inverse(m, n)` for any n × n matrix; it equals `vnl_inverse3` bit for bit in 3D.
- **larmorx-ants:**
  - `Pixel::from_f64_bounded` (ITK's bounds-checked cast) is now required of every pixel type.
  - New pixel types and `OutputImage` variants `i8`/`i16`/`u16`/`u32`.
  - `cstd` gains `convert_vector_f32`/`convert_vector_f64`, `stof` and `from_string_f64`.
- **Python:** in-memory image tuples may carry a 4th element, the extra-axis spacing. In-memory 4D images now pass the header's repetition time to the ANTs programs; before, they got spacing 1. `FloatInput` gained `spacing`.
- **Benchmark harness:** `run_suite()` plus a title and notes for `render()`.
- **Correction:** the part-1 benchmark label gave the raw T1w as 176×256×256; it is 160×192×192.

### Docs
- API pages: `ants-smooth-image.md`, `ants-resample-image-by-spacing.md`, `ants-resample-image.md`; `ants-image-math.md` extended.
- Validation records, plus the README tables.
- Benchmark `ants-gaussian`.
- Findings in `docs/findings/ants-gaussian-filters.md`, linked from `itk-resampling.md` and `ants-image-programs.md`.
- PROVENANCE rows in four crates.
- `architecture.md` and `overview.md`.

### Commits
278bcd2, 2f95678, 48d09e6, dd060a3, 54d3282, 8f4f9a0, 26a8a65, 6ca5f61, e71528c, 3095940, 60616f0, 9ab97ec.

One gate slip: `6ca5f61` was first committed after the gate failed on clippy, because `| tail` hid the exit code. I fixed the lint and amended the commit straight away, before any later commit; every gate passed on the final tree.

### Left undone
- ResampleImage's B-spline, Gaussian and windowed-sinc interpolation in 2D and 4D.
- The median filter orders `-0.0`/`+0.0` and NaN deterministically, where ITK leaves it to chance. This is documented; NIfTI input never contains NaN.
- **Test data:** two new generated inputs, `thin-3-slices` and `tiny-4`, still have to go into larmorx-testdata. I couldn't write there, so they are staged in `testdata-staging/` in my scratchpad: a patched generator and a README with the commands. Both are deterministic and identical to the parity harness's own builders.
- Re-running the antsApplyTransforms benchmark; it is now faster than its page says.
- The CLAUDE.md status line is left for you to update.

Files are in `/home/karelo/VScode_projects/super_fmriprep/larmorx/.claude/worktrees/agent-a553c824c3de7c248`; the staging is in `/tmp/claude-1000/-home-karelo-VScode-projects-super-fmriprep/d96e838f-0213-4978-9a72-dac8eae4b541/scratchpad/`:
- docs/validation/ants-smooth-image.md
- docs/validation/ants-resample-image-by-spacing.md
- docs/validation/ants-resample-image.md
- docs/validation/ants-image-math.md
- docs/benchmarks/ants-gaussian.md
- docs/findings/ants-gaussian-filters.md
- testdata-staging/ (in the scratchpad)
