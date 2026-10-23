# Final report: Source-faithful ANTs oracle + N4 fix

Agent `a715e3d8d3821e4e8`, 2026-10-11. The agent's report to the orchestrator, verbatim.

## What went wrong with the GCC 11 oracle
It is a **compiler bug, GCC bug 110182** ("TYPE_PRECISION on VECTOR_TYPE causes wrong-code"). The C++ standard does not allow this behaviour.
- **Mechanism:** GCC's SLP vectorizer turns the first two axes of ITK's B-spline fit into `vector(2) double` → `vector(2) float` → `vector(2) double` round trips. The value-numbering pass right after (`fre5`) then deletes them. The match.pd rule for "two conversions in a row" compares `TYPE_PRECISION`, which for a vector type is its lane count (2 = 2), not its element precision (32 vs 64).
- **Fix upstream:** GCC commit 3e12669a0eb (2023-06-09), released in 11.5, 12.4 and 13.2. I checked `gcc/match.pd` in each release: 11.4, 12.1–12.3 and 13.1 are still broken. Ubuntu 22.04's g++-11 (11.4) and gcc-12 (12.3) have the bug.
- **Why it is not allowed:** C++17 [expr]/13 says assignments and casts must still convert, even when expressions carry extra precision. x86-64 SSE uses no excess precision at all.
- **Evidence:**
  - GCC's own dumps of N4's translation unit: the round trips are present after `slp1` and gone after `fre5`.
  - A 50-line reproduction differs from the source's roundings in 31 of 120 values with g++-11 at -O3.
  - It gives 0 differences at -O0/-O1/-O2, with `-fno-tree-vectorize`, with `-fno-tree-slp-vectorize`, with gcc 8.4, and with conda-forge GCC 13.3 and 14.3.
  - gcc-12 12.3 (a C version of the reproduction) also miscompiles. clang is not on the machine.
- **N4 recompiled with GCC 11.4 under each flag set** (38 smoke cases, CORE-MATH preloaded, compared with larmorx following the source):

| Flags | Cases bit-identical |
|---|---|
| `-O2` | 38 of 38 |
| `-O3 -fno-tree-vectorize` | 38 of 38 |
| `-O3 -fno-tree-slp-vectorize` | 38 of 38 |
| oracle's own flags | 5 of 38 |
| `-O3 -fno-tree-loop-vectorize` | 5 of 38 |
| `-mtune=generic` | 5 of 38 |
| conda-forge's CXXFLAGS | 5 of 38 |

- `-fexcess-precision=standard` is "sorry, unimplemented" for C++ in GCC 11, and does not help in C.
- **Other builds follow the source:** fMRIPrep's ANTs 2.6.2 (N4 library built by conda-forge GCC 14.3.0, its ITK by 13.3.0) and ANTsPy 0.6.3 (GCC 10.2.1). ANTsPy's N4 is bit-identical to larmorx in the 5 of 5 cases I tried.

## Chosen oracle build
- ITK 5.4.5 and ANTs 2.6.5, built by **conda-forge GCC 14.3.0** (the compiler of fMRIPrep's ANTs).
  - It lives in a user-level conda environment, `oracles/toolchains/gcc-14.3`; no system packages were installed.
  - Same ITK/ANTs flags as before (`-O3 -march=corei7`), static libstdc++. Built with 4 jobs in 35 minutes.
- `scripts/build_ants_oracle.sh` now:
  - takes `--cc/--cxx/--cflags/--cxxflags/--ldflags/--workspace`;
  - records the compiler, the flags each build actually used and a probe result in `BUILD_INFO.txt`;
  - refuses a compiler that fails the bug probe (`--allow-miscompile` overrides);
  - builds only the programs used. It also fixes a misspelt option: `-DSUPERBUILD_ANTS` does not exist, so the old oracle had been a superbuild; it now passes `ANTS_SUPERBUILD=OFF`.
- **Swap:** the old build was renamed to `oracles/ants-2.6.5-gcc11-O3`. `oracles/ants-2.6.5` is now a symlink to `oracles/ants-2.6.5-src`, so its build tree keeps its paths.
  - Both `BUILD_INFO.txt` files have notes on the swap. `oracles/README.md` (outside git) describes every oracle directory.
  - The investigation's scripts, outputs and N4 variant binaries are kept in `oracles/ants-2.6.5-gcc11-O3/gcc-bug-110182/`.
- The AFNI oracle and the CORE-MATH preload are built by GCC 11.4 at -O2, where GCC 11 does not vectorize, so the bug cannot affect them.

## N4 code change
- N4 now follows the source: `r` and the point coordinates are rounded to float on every axis. This is a compiler artefact, so there is no D13 switch.
- I kept `gcc_double_axes`, renamed `gcc11_double_axes`, as a documented hook for comparisons only. It is 0 by default and in every caller. It can reproduce GCC 11 builds (Ubuntu 22.04's compilers make them) and keeps the old records checkable.

## Re-validation (standard tier, records from clean commit 38b2a0c97ecf)
Every suite passes.

| Suite | Result |
|---|---|
| ants-apply-transforms | 79 pass, 4 both-error; 63 of 79 bit-identical |
| ants-threshold-image | 35 pass, 5 both-error, 1 expected divergence |
| ants-multiply-images | 12 pass, 3 both-error |
| ants-smooth-image | 31 pass, 7 both-error |
| ants-resample-image-by-spacing | 29 pass, 5 both-error, 1 expected divergence |
| ants-resample-image | 47 pass, 8 both-error, 2 expected divergences |
| ants-image-math | 302 pass, 24 both-error, 1 expected divergence |
| ants-n4 | 46 pass, 5 both-error |
| ants-denoise | 42 pass, 9 both-error |

- **antsApplyTransforms now runs the oracle binary** instead of ANTsPy in-process (ANTsPy is the fallback); the 63 of 79 is the same as with ANTsPy.
- **Only N4's numbers changed:**
  - as built: 29 of 45 cases (35 of 54 outputs) bit-identical, up from 27 of 45 (30 of 54);
  - with CORE-MATH: 45 of 45 (54 of 54).
  - Two cases exceed ANTs' own thread band: sMRIPrep's pre-template N4 (1.3×) and the shrink-3 phantom (1.7× max |diff|, 4.2× in 1−NCC).
  - NCC is at least 0.9999999 for corrected images and 0.9999998 for bias fields.
- Every other record differs only in the reference line, pointer addresses and crash or libstdc++ error texts. DenoiseImage's band ratios moved, as expected at ≥8 threads.
- **Old build against new build, case by case:** byte-identical outputs and headers in every case of every other suite except one ResampleImageBySpacing case that resamples uninitialised memory. N4: only 6 of 45 identical.
- **No other tool needed fixing.** A GCC-dump scan of about 780 units of the old build (the 9 ported programs, antsUtilities and 26 ITK libraries) finds the same folded round trips only in code no ported tool uses:
  - `CenteredTransformInitializer` for float Euler transforms (antsRegistration `--initial-moving-transform` with `--float 1`);
  - `ImageRegistrationMethodv4::SetMetricSamplePoints` in every 2D/3D antsRegistration instantiation;
  - the B-spline and velocity-field transforms;
  - ImageMath's KinematicTensor and metric operations.
  - **A4/A5 (registration) must be validated against the new build only.**

## Against fMRIPrep's own ANTs 2.6.2
I ran its binary through Apptainer wrappers (CORE-MATH bound in and preloaded), one thread, on every standard-tier case:
- **N4: 45 of 45 cases (54 of 54 outputs) bit-identical to larmorx with CORE-MATH.** As built, 29 of 45; every remaining difference matches the new oracle's, case by case.
- **DenoiseImage:** 40 of 40 with CORE-MATH, 7 of 40 as built.

## Golden checksums
Re-recorded from f58d32a after the smoke parity passed (1,660 tests) against the new oracle:
- `cli.ants.N4BiasFieldCorrection`: the corrected image and bias field data changed; headers unchanged.
- `py.ants.n4`: 15 of 18 outputs changed; the 4D case's outputs did not.
- The other 80 cases are unchanged.

## Docs updated
`docs/findings/ants-n4.md` (GCC section rewritten), `fmriprep-oracle.md`, `upstream-defects.md` (ANTS-41), `ants-denoise.md` (renamed path), `docs/validation/README.md` (how the oracle is built), `docs/porting-guide.md` (choose the oracle's compiler), `docs/api/ants-n4.md`, `docs/overview.md`, the findings index, the decision log (new entry; the earlier "compiler is part of the reference" bullet marked superseded), and the stage-harness wording.

## Commits
On branch `worktree-agent-a715e3d8d3821e4e8`; main merged at 10847f5 and has not moved since. Not pushed; no attribution lines.
- bc5754c, b7c14a3, 38b2a0c: WIP checkpoints (the code change and script, the harness, docs). They had the 2 N4 golden failures, as expected until re-recording.
- f58d32a: the 9 records plus docs.
- e16106a: golden.json plus the decision-log golden summary.
- d8cb349: findings wording.
- **All gates pass at HEAD.**

## Left for you
- CLAUDE.md still says "27 of 45" for A3 part 1; it should be 29 of 45 (35 of 54 outputs). I did not edit CLAUDE.md.
- `docs/validation/fmriprep-stages.md` predates the change (its N4 rows use the old larmorx and the GCC 11 oracle) and should be regenerated. The findings say so.
- `docs/benchmarks/ants-n4.md` was timed against the GCC 11 binary and was not re-run.
