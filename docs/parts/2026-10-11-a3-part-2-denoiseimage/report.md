# Final report: A3 part 2: DenoiseImage

Agent `a7afb2dc207a595ac`, 2026-10-11. The agent's report to the orchestrator, verbatim.

## DenoiseImage (A3 part 2): done, `validated`

DenoiseImage is ported in full: the Rust API, the CLI, `lx.ants.denoise_image`, the `.pyi` stub, the API page, findings, a validation record, a benchmark, provenance and golden cases. Every gate passes. The branch is `worktree-agent-a7afb2dc207a595ac`, with main merged in and nothing pushed.

On ds000005's T1w, with sMRIPrep's exact command (`-i T1w -n Rician -o out -s 1`), larmorx is bit-identical to ANTs 2.6.5 with CORE-MATH preloaded. Against the unmodified binary, 5,522 of 5,898,240 voxels differ, by at most 2.0e-4. It takes 13.0 s on one thread against 217 s for ANTs.

### ANTs' thread band
The thread dependence is much larger than N4's. The filter writes its output, count and Rician bias image across patch neighbourhoods with no ordering between threads. ITK's pool also makes 4 work units per thread when there is more than one.

Measured on ds000005's T1w (values up to 1,850), each compared with the one-thread result:

| Threads | Voxels that differ | Max \|diff\| | 1 − NCC | Same result run to run? |
|---|---|---|---|---|
| 2 | 21 % | 96.8 | 1.4e-5 | yes |
| 4 | 49 % | 271 | 5.9e-5 | yes |
| 8 | 82 % | 271 | 1.1e-4 | no: two runs differ in 47,635 voxels |
| 12 | 85 % | 271 | 1.6e-4 | no: one run differs in 149,102 voxels |

- **One thread** is deterministic.
- **fMRIPrep's own output is affected.** Its denoised T1w (25.2.6 run, 8 threads through `NSLOTS`) differs from ANTs at one thread in 4.9M voxels, and from ANTs at 8 threads in 163k.
- **larmorx** gives the one-thread result on any thread count.

### Parity (standard tier: 51 cases, 42 pass, 9 rejected by both, 0 fail)
- **Unmodified ANTs, one thread:** 7 of 40 compared cases (9 of 52 outputs) are bit-identical.
  - The rest differ through glibc's `expf`/`exp` only, in a few voxels by a few float units.
  - The worst cases:
    - ds000210's T1w: 5,389 voxels, max 2.1e-3.
    - The outliers phantom (values up to 1e6): max 1.2e-2.
  - 1 − NCC is at most 2.2e-13, and every difference is at most 2.2e-4 of ANTs' own thread band.
- **With CORE-MATH preloaded:** 52 of 52 outputs are bit-identical, in 2D, 3D and 4D.

Two ITK details that had to be reproduced:
- **Float-coordinate resampling.** DenoiseImage declares `ResampleImageFilter<…, float>` for the noise, so points and continuous indices are rounded to float. In 4D the interpolator's weights are float too.
- **ITK's SSE2 `floor`.** It returns 0, not −1, for floats in [−2⁻²⁶, 0). This caused the last 7 differing voxels in 4D.

### Benchmarks (quiet machine after fMRIPrep finished, `docs/benchmarks/ants-denoise.md`)
- 10.7–17× faster than ANTs on one thread.
- 29–76× faster with 12 threads, and 5.8–16× faster than ANTs when both use 12.
- sMRIPrep's T1w: 13.0 s on one thread and 2.9 s on 12, against 217 s and 45 s for ANTs.
- The speed comes from padded images, 8 centres computed side by side, two patch distances summed per pass, and the weighted sums kept in registers. Every sum stays in ITK's order.

### Golden cases (recorded from fbbcf24; smoke parity passed, 1656 tests)
- `cli.ants.DenoiseImage` covers Rician with a mask and the noise output, and Gaussian with `-s 2` and per-axis radii.
- `py.ants.denoise_image` covers Rician, Gaussian with a mask, shrink 2, 2D and 4D with shrink 2.
- The suite now has 82 cases and 334 outputs. Both new cases together take about 0.08 s.

### Scaffold changes
- **New modules:**
  - `larmorx_image::adaptive_denoising` (the filter).
  - `larmorx_image::neighborhood` (ITK's box mean and the module's box variance).
- **Resampling:** `larmorx_ants::resample` gains `Coordinates::{Double, Float}` and `resample_identity_with`. The existing double 4D path now uses ITK's floor (`grid::itk_floor_f64`); parity and golden outputs are unchanged.
- **Shared internals:** the Bessel functions and N4's `congruent` are now `pub(crate)`.
- **Parity harness:** the thread-band comparison moved from the N4 suite into a shared `parity/ants_threaded.py`, which both suites now use. N4's smoke tier still passes.
- **Upstream pin:** an `ITKAdaptiveDenoising` row in `upstream.tsv`/`UPSTREAM.md`, fetched read-only into `tags/ITKAdaptiveDenoising`. It is identical to the copy the oracle build put in ITK's tree. `NOTICE` lists it.
- **Test inputs:** the six new input builders live in the parity suite, not in larmorx-testdata: `denoise-mask-shrunk-2`, `-larger`, `-smaller`, `-other-grid`, `-values` and `denoise-slice-mask`.
- **Oracle runs:** in `oracles/ants-2.6.5/denoise-thread-band/`, with logs in `oracles/logs/`.

### Commits
43c718f, 724d986, 257b3e3, 8a9cc3b, ababc25 (WIP checkpoints), fbbcf24 (merge of main), d8b2a62 (golden checksums), 77bb3c6 (docs), 0c7c7a4 (benchmark), 80818f0 (validation record and status docs). None has AI attribution.

### What A4 can reuse
- `shrink_plan`/`shrink`, the discrete Gaussian, and the box mean and variance.
- Resampling with float or double coordinates, and ITK's float and double `floor`.
- The deterministic scatter-to-gather pattern: compute in parallel, then sum each output in ITK's order. It applies to any ITK filter where threads add into shared voxels.
- The padded-image approach with centres computed side by side, which fits neighbourhood metrics such as CC.
- `parity/ants_threaded.py` for tools whose result depends on the thread count, and the CORE-MATH preload as the exact reference.

### Files
- Rust: `crates/larmorx-image/src/adaptive_denoising.rs`, `crates/larmorx-image/src/neighborhood.rs`, `crates/larmorx-ants/src/denoise_image.rs`, `crates/larmorx-ants/src/cli/denoise_image.rs`, `crates/larmorx-py/src/ants_filters/denoise.rs`
- Python: `python/larmorx/ants/denoise.py`
- Validation: `validation/src/larmorx_validation/parity/ants_denoise.py`, `validation/src/larmorx_validation/parity/ants_threaded.py`, `validation/src/larmorx_validation/bench/ants_denoise.py`
- Docs: `docs/api/ants-denoise.md`, `docs/findings/ants-denoise.md`, `docs/validation/ants-denoise.md`, `docs/benchmarks/ants-denoise.md`

All paths are under `/home/karelo/VScode_projects/super_fmriprep/larmorx/.claude/worktrees/agent-a7afb2dc207a595ac/`.
