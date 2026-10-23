# Final report: A3 part 1: N4BiasFieldCorrection

Agent `a8834d9b49002e467`, 2026-10-11. The agent's report to the orchestrator, verbatim.

## A3 part 1 (N4BiasFieldCorrection): finished

All of it is committed on the worktree branch at `/home/karelo/VScode_projects/super_fmriprep/larmorx/.claude/worktrees/agent-a8834d9b49002e467`. Nothing is pushed, and main has not moved since f17a39d, so no merge was needed. The last commit passes every gate in the brief: Rust fmt, clippy and tests in both workspaces, `maturin develop --release`, pytest (296 passed), ruff and SPDX. Status: **`validated`**.

### What "bit-identical to ANTs with correctly rounded math" means
- **What it compares against:** the **unmodified ANTs 2.6.5 oracle binary** (`oracles/ants-2.6.5/bin/N4BiasFieldCorrection`), run with `ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS=1` and `LD_PRELOAD=oracles/core-math-preload/libcrmath.so`.
- **What the preload changes:** that library (built by the new `scripts/build_crmath_preload.sh`) replaces glibc's `expf`, `logf`, `exp`, `log`, `sin`, `cos` and `sincos` with CORE-MATH's correctly rounded versions when the program loads. ANTs' and ITK's own code is unchanged; only those C-library functions are substituted. `LD_DEBUG=bindings` confirmed the binding.
- **Results, standard tier** (51 cases: 45 compared, 54 outputs, 2D/3D/4D):
  - **Binary with the preload:** 54 of 54 outputs bit-identical; headers byte-identical.
  - **Unmodified binary, glibc maths, 1 thread:** 27 of 45 cases (30 of 54 outputs) bit-identical.
    - By category: fmriprep 4 of 11, options 15 of 22, inputs 8 of 12.
    - In the other 18 cases (24 outputs), the difference is smaller than ANTs' own difference between thread counts in 16. The two exceptions:
      - `fmriprep/boldref-coregistration`: 1.6× the band in max |diff|, 1.4× in 1−NCC;
      - `options/shrink-3`: 1.4× and 2.6×.
    - NCC is at least 0.9999999 on corrected images and 0.999996 on bias fields.
- **Cause of the remaining differences:** a scratch build of larmorx calling glibc's `expf`/`logf`/`exp` instead reproduced the unmodified binary bit for bit in all 13 smoke-tier cases (16 outputs) that differ.
- **Glibc measurements:** glibc 2.35 `logf` is wrong in the last bit for 416,908 floats; 10 of them are integers ≤ 65,536 (e.g. 1579, 3100), common in T1w data. `expf` is wrong for 170,648. I kept correctly rounded maths (rule 5).
- **Open question for the user** (in the decision log): an opt-in "glibc maths" mode would give bit-identity with the unmodified Linux binary. I did not build it.

### ANTs' thread band
N4's result depends on ANTs' thread count, because ITK's B-spline fit sums per-thread lattices. Measured on ds000005 at 2, 4 and 12 threads against 1:

| Command | Voxels changed | Max relative difference | 1−NCC |
|---|---|---|---|
| raw T1w, ANTs defaults | 92 % | up to 9.5e-4 | up to 1.4e-8 |
| brain-extraction N4s | 78–92 % | up to 1.2e-4 | up to 1.5e-10 |
| BOLD reference | 56 % | up to 3.5e-3 | up to 3.0e-8 |
| bias fields | — | — | up to 7e-6 |

larmorx always gives ANTs' 1-thread result, on any number of threads. The parity suite checks that 4 threads and 1 thread give the same bits. fMRIPrep runs the BOLD-reference and pre-template N4s with 1 thread; the brain-extraction N4s use `omp_nthreads` and sdcflows uses 8, so those depend on the machine.

### Two quirks reproduced
- **GCC 11 at `-O3` in the oracle build:** in 2D and 3D builds it keeps the fit's `r[0..1]` and the first two point coordinates in double, where the source rounds them to float. It does not do this in 4D. I found it from the disassembly and reproduce it with `gcc_double_axes`.
- **ITK's histogram min/max uses `else if`:** the first voxel can never become the minimum. This is covered by the `inputs/ramp` case.

Both are written up in `docs/findings/ants-n4.md`, with file:line references, the nipreps call sites and the nipype detail that `shrink_factor=2` for the BOLD reference is silently dropped.

### Parity per fMRIPrep command line (real data)
All 11 are bit-identical with the preload. Against the unmodified binary:

| Command line | Unmodified binary |
|---|---|
| sMRIPrep pre-template (ANTs defaults) | bit-identical |
| brain-extraction pass 1 | bit-identical |
| sdcflows magnitude, ds003012 and ds001600 | bit-identical |
| BOLD reference on ds000005 | differs, 1.6×/1.4× the band |
| BOLD reference on MNI res-02 | differs, 0.89× |
| final N4 on ds000005 | differs, 0.21× |
| final N4 on ds000210 | differs, 0.21× |
| `n4-only` | differs, 0.29× |
| boldref with mask | differs, 0.57× |
| T1w-preproc with mask | differs, 0.12× |

Errors: 5 cases rejected by both ANTs and larmorx; `--help` passes.

### Benchmarks
Quiet machine, load 0.50 before the run; larmorx compared against ANTs at 1 thread (report: `docs/benchmarks/ants-n4.md`).

| Command | ANTs 1 thread | ANTs 12 threads | larmorx 1 thread | larmorx 4 threads | larmorx 12 threads |
|---|---|---|---|---|---|
| raw T1w, ANTs defaults | 55.2 s | 10.7 s | 8.69 s | 3.18 s | 2.40 s |
| brain-extraction pass 1 | 57.9 s | 11.3 s | 8.59 s | — | 2.69 s |
| final N4 | 12.4 s | 4.33 s | 1.79 s | — | 0.69 s |
| BOLD reference | 1.00 s | — | 135 ms | 88 ms | — |
| sdcflows magnitude | 1.96 s | — | 271 ms | 147 ms | — |

larmorx is 6.4–7.4× faster than ANTs on 1 thread, up to 23× with threads, and 4.2–6.3× faster than ANTs when both use 12 threads.

### Golden cases
- `cli.ants.N4BiasFieldCorrection`: mask, spline distance, two levels, bias field.
- `py.ants.n4`: six calls (defaults; weight with a mesh and quadratic splines; quintic splines; mask with rescaling and a threshold; 2D spline distance; 4D series).
- Recorded from 1414ba5 after the smoke parity of every suite passed (1609 tests). The golden suite now has 80 cases and 316 outputs and runs in 0.93 s; the N4 Python case takes 67 ms.

### Scaffold changes (APIs for A4 and later)
- **Shrink filter, for A4:**
  - `larmorx_image::shrink::{ShrinkPlan { output: ItkGrid, factors, offset }, shrink_plan(&ItkGrid, &[usize]) -> Result<ShrinkPlan>, shrink<T: Copy+Send+Sync>(data, &ItkGrid, &ShrinkPlan, n_threads)}`.
  - This is ITK's `ShrinkImageFilter` for any dimension and pixel type; the output grid can be computed without the data.
  - `larmorx_image::ItkGrid` carries ITK's start index and its index ↔ point rules (plus `round_half_integer_up` and `vnl_floor_f32`).
- **B-splines:** `larmorx_image::bspline::{Lattice, evaluate, refine, kernel, fit::{GridFit, GridFitPlan, fit_on_grid, gcc_double_axes}}`.
- **FFT:** `larmorx_image::vnl_fft::VnlFft1d`, vnl/Temperton FFT for powers of 2.
- **N4 filter:** `larmorx_image::n4::{N4Parameters, n4_bias_field}`.
- **Maths:** `larmorx_core::math::{expf, logf}` (CORE-MATH, bit-identical to the C on all 2^32 inputs) and `larmorx_core::vnl_svd_f32` (vnl's float LINPACK SVD).
- **Tool layers:** `larmorx_ants::n4_bias_field_correction`, the CLI, and `lx.ants.n4_bias_field_correction` returning `N4Result(corrected, bias_field, iterations)`. Python's `rescale_intensities` defaults to False, as in ANTsPy.
- **Validation harness:**
  - `run_ants(..., correctly_rounded=True)` and `crmath_preload()`;
  - the new suites `parity ants-n4` and `bench ants-n4`;
  - the benchmark harness now compares against ANTs at 1 thread.
- **Licensing:** NOTICE now covers VXL's float SVD and FFT and Temperton's GPFA, plus CORE-MATH's expf, logf and powf; the copy in `crates-gpl` is refreshed. PROVENANCE rows are added in the core, image and ants crates.
- **Test data:** no larmorx-testdata generator changes were needed (the suite has its own `gen:` builders).

### Commits
- a23c98b, 2d4988e, 24fa068, 8626cd8, 1414ba5: WIP checkpoints.
- 49a43d1: golden checksums.
- 1f345c6: validation record.
- 3e2b1be: benchmark and final docs.
- None carries AI attribution.

### What remains for DenoiseImage (A3 part 2)
- **Sources to port:** `Examples/DenoiseImage.cxx` (675 lines), and ITK's remote module AdaptiveDenoising (pinned at 24825c8d; about 1,400 lines: `AdaptiveNonLocalMeansDenoisingImageFilter`, `NonLocalPatchBasedImageFilter`, `VarianceImageFilter`).
- **Already in larmorx:**
  - `ShrinkImageFilter` (the new shrink API);
  - linear identity resampling (in larmorx-ants);
  - the CORE-MATH preload reference, which applies because the weights use `std::exp`.
- **Still to do:**
  - mean and variance filters;
  - the modified Bessel I0/I1 used for the Rician correction (check its implementation);
  - measure ANTs' thread band first: the patch-based accumulation may also depend on the thread count.

### Notes for you
- Four shell loops left over from earlier agents' afni-tshift benchmark waiters (PIDs 751489, 752721, 778555, 823847) keep polling forever, because their `pgrep -f` patterns match each other. That is why my own "wait until quiet" loop never started. I did not kill them; I ran the benchmark directly once the machine was quiet.
- I did not update the repo's `CLAUDE.md` status section; that is yours to do.
