# Benchmarks: the field maps' part B (`lx.sdcflows`' masks, `lx.niworkflows`, `larmorx sdcflows sdcflows`)

The port of the rest of sdcflows 2.17.1's estimation (its masks, PEPOLAR's and SyN's Python steps,
the BIDS app), niworkflows 1.15.4's helpers and fMRIPrep 25.2.7's own field-map steps against the
originals themselves (in-process, in the newest Python environment, `oracles/python-newest`:
numpy 2.5.4, SciPy 1.18.1, scikit-image 0.26.0, nibabel 5.4.2). Wall time of the whole call
(reading, the work, writing every file).

- **Generated:** 2026-11-09 on Linux x86_64 (the main machine: AMD Ryzen 9 7945HX, 16 cores /
  32 threads), larmorx at commit `91fb9e73` (field maps part B, relay 28: the four calls; the
  random walker's rows and the BIDS app's runs at `8828f241`, the same code; the extension built
  from `2b07a2e7`'s source, the Rust of both), each part inside a memory box (32 GiB), with
  [`bench_estimation.py`](../../validation/investigations/sdcflows-fieldmap-b/bench_estimation.py)
  (its runs: [sdcflows-estimation.json](sdcflows-estimation.json)).
  `OPENBLAS_CORETYPE=Haswell` and `OPENBLAS_NUM_THREADS=1` (numpy's and SciPy's BLAS at one
  thread, as fMRIPrep's nodes run: `OMP_NUM_THREADS=1`). **The machine was quiet**: no other
  agent, nothing else running (one-minute load average 0.9 to 2.0 of 32 threads at the start of
  each part, the benchmark's own one-thread work included).
- **The same result:** every file of every larmorx run (`upstream_defects="reproduce"`) checked
  byte for byte (decompressed; a NIfTI file's four extension bytes after the header aside, as the
  memory test compares) against the original's run of the same call: identical in every row.

## The four memory calls

The calls of `tests/parity/test_memory_peak.py`'s
`test_fieldmap_steps_need_no_more_memory_than_the_originals` on fMRIPrep's files in the oracle
runs' working directories: sdcflows' `brain_masker` on ds002603's clipped magnitude (128 × 128 ×
128 once padded: sdcflows' largest Python step), niworkflows' `IntensityClip` (`_advanced_clip`,
`p_min=35`, `p_max=99.98`) on the merged magnitude before it, `RobustAverage`'s own steps
(`mc_method=None`: the median and the drift; the 3dvolreg it runs otherwise is a program of its
own, [afni-volreg](afni-volreg.md)) on ds004958's PEPOLAR series, and fMRIPrep's
`ReconstructFieldmap` of ds005454's coefficients onto its T1w grid through two transforms
(`bold_anat_wf.fmap_recon`). The originals have no thread option; larmorx at 1 and 14 threads
(`n_threads`). Each time the median of three runs after one untimed run of each implementation
(in brackets the speed-up against the original).

| Call | original | larmorx (1) | larmorx (14) | identical |
|---|---|---|---|---|
| `brain_masker`, ds002603 (sdcflows) | 8.04 s | 6.93 s (1.16×) | 2.47 s (3.25×) | yes |
| `IntensityClip`, ds002603 (niworkflows) | 1.40 s | 1.63 s (0.86×) | 0.19 s (7.42×) | yes |
| `RobustAverage` without HMC, ds004958 (niworkflows) | 0.20 s | 0.23 s (0.89×) | 0.18 s (1.16×) | yes |
| `ReconstructFieldmap`, ds005454 (fMRIPrep) | 0.89 s | 0.30 s (2.97×) | 0.14 s (6.36×) | yes |

**At one thread** the port is within 15 % of the originals on the two small niworkflows steps
(0.86× and 0.89×) and faster on `brain_masker` (1.16×) and `ReconstructFieldmap` (2.97×); **at
14 threads** 3.3 to 7.4 times faster on three of the four (`RobustAverage`'s fifth of a second
1.16×), with the same bits at any thread count (a follow-up for the clean-room original: the two
niworkflows steps at one thread). Memory, from the test (one thread, larmorx's peak against the
original's, relay 25 at `ab5ffe99`): `brain_masker` 431 MiB against 1,438, `IntensityClip` 84
against 143, `RobustAverage` 108 against 149, `ReconstructFieldmap` 230 against 1,438.

## A random walker that does not converge

`brain_masker` on random testing's case `sdcflows-estimation:fmapb-try1:492` (a 15 × 20 × 15
synthetic head of 2.5 × 0.8 × 1.0 mm voxels, values 0 to 3): the inverted image's grey closing is
0 everywhere, so sdcflows' `segtarget = 2 * closed / closed.max() - 1` is `0 / 0`, every weight of
the random walker a NaN, and SciPy's conjugate gradients never meet their test: they run to
`maxiter`, 2 × 162,120 iterations. One run each, one thread.

| Build | sdcflows | larmorx (1) | identical |
|---|---|---|---|
| relay 27's (`e88326b6`'s random walker) | 22.26 s | 210.49 s (0.11×) | yes |
| relay 28's (`2b07a2e7`) | 22.00 s | 0.053 s (416×) | yes |

**Where the time went** (relay 27's build, 40 samples of the running call by a debugger, `gdb
-batch`, one second apart): every sample in the conjugate gradients' slow paths, the products `A
@ p` (`matvec2`, 23 of 40) and SSE's NaN rule called out of line (`x86_nan::nan_f32`, 10 of 40),
the vector updates the rest: every element of every vector was a NaN, so each was computed, found
to be a NaN and computed again with a call per operation. **The change** (same bits): a NaN in the
residual stays (SSE's `r - y` with a NaN `r` is `r`), so the test never passes again, and a NaN of
the solution never changes (`x + y` is `x`): once every element of the solution is a NaN, the
iterations left give SciPy's result unchanged, so larmorx stops there with `info = maxiter` (its
warning, SciPy's); the slow paths' NaN rules are inlined, and a NaN sum of a row stops at its
first NaN (with the rules inlined alone, 95 s). Checked bit for bit, both D13 modes: the trial's
139 brain_masker cases replayed (every one of the 59 that timed out or took more than a minute,
80 others: all agree; the 8 timeouts of up to 2,592 s now take 73 to 199 s (14 replays at once), sdcflows' own
`maxiter` runs), the 13 permanent cases of the three tools, the golden case
`py.skimage.random_walker.nan` against scikit-image (both solvers, float32 and float64).

## The BIDS app: `sdcflows` against `larmorx sdcflows sdcflows`

sdcflows' own app (`sdcflows <bids> <out> participant --participant-label <sub> --nprocs 1
--omp-nthreads 1 --no-fmapless`, nipype's `Linear` plugin) against larmorx's on five of sdcflows
2.17.1's own runs (`oracles/sdcflows-2.17.1`, one of each estimator kind and the PEPOLAR runs),
through `check_app.py --sections run`, which compares every derivative file (names, uncompressed
and gzip bytes, sidecars), the `cli` logger's lines and the settings files: identical on every
run. sdcflows' programs run fresh, not from the oracle-output cache
(`LARMORX_ORACLE_CACHE_MODE=off`): ANTs' N4 at one thread with correctly rounded maths (the
oracle's build, the CORE-MATH preload), FSL 6.0.7.23's `prelude` and `topup`, AFNI's `3dvolreg`;
larmorx's with its replicas (`LARMORX_IMPLEMENTATION=replica`: topup's in `larmorx-nc`). One run
each, one thread, whole seconds as `check_app.py` prints them.

| Run (estimators) | sdcflows | larmorx (1) | identical |
|---|---|---|---|
| ds005752-sub-ON01016 (a direct field map, Hz) | 21 s | 4 s (5.3×) | yes |
| ds005454-sub-16 (a phase difference) | 50 s | 23 s (2.2×) | yes |
| ds002422-sub-02 (two phase differences, 1.5 T) | 27 s | 7 s (3.9×) | yes |
| ds005040-sub-002 (PEPOLAR: topup) | 103 s | 87 s (1.2×) | yes |
| ds006736-sub-004 (PEPOLAR: topup) | 79 s | 56 s (1.4×) | yes |

Each run's time includes every program the app runs (N4, PRELUDE, topup) and the B-spline fit
([sdcflows-fieldmap](sdcflows-fieldmap.md)); sdcflows' also includes nipype's own work at every
node (its folder, its hashed inputs, its pickled result), which larmorx's plain functions do not
do.
