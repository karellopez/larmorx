# Benchmarks: FAST (`lx.fsl.fast`, `larmorx fsl fast`)

FAST's clean-room original (`crates/larmorx-fsl`, module `fast`) against FSL 6.0.7.23's `fast`
(single-threaded) and the bit-exact replica (`larmorx-nc fsl fast`, FSL Licence), on fMRIPrep's
call (`fast -I 0 -N -p -g -S 1 <T1w brain>`) on the standard tier's three oracle runs. Wall time
of the whole command, **one run each (informal)**.

- **Generated:** 2026-11-03 on Linux x86_64 (the main machine: AMD Ryzen 9 7945HX, 16 cores / 32
  threads), with
  [`validation/investigations/cleanroom-fast/bench.py`](../../validation/investigations/cleanroom-fast/bench.py)
  (`--threads 1 14 --json`), larmorx at commit `f5b8c6b` (FAST's clean-room original, relay 3),
  inside a memory box. **The machine was shared**: another agent (topup's replica) worked beside
  this one (load average 1.5 of 32 threads before the runs), so the original was given at most 14
  threads and every time may be a little long; the runs: [fsl-fast.json](fsl-fast.json).
- **FSL's time is its stored run's:** FSL was not run again (the oracle cache serves every FSL
  run; CLAUDE.md, "never compute an original's result twice"). Its time is the one the cache
  recorded when it first ran the `fsl-fast-replica` suite's key on this machine, while that suite
  ran several cases at once, so FSL's times are likely longer than on a quiet machine (the spec,
  §10, measured about 25 s for ds000005's call, and the replica 17 s: the replica, timed here,
  is the steadier reference).
- **What is timed:** each program as its own process in FSL's clean environment (FSL off the
  `PATH`, `FSLOUTPUTTYPE=NIFTI_GZ`, OpenBLAS's Haswell kernels): reading the T1w brain, the
  segmentation, writing its twelve images (`_seg`, `_seg_0..2`, `_prob_0..2`, `_pve_0..2`,
  `_pveseg`, `_mixeltype`). The thread count is `OMP_NUM_THREADS`; FSL and the replica run on one.
- **The same result:** the replica's twelve images are FSL's byte for byte (checked by the
  script on each run); the original's are within its record's thresholds (`fsl-fast` suite,
  `specs/fast.md` §11), and **the same bits at any thread count**: every speed change of this
  relay leaves the original's outputs, standard output and error byte for byte as they were
  (13 command lines, one per path the changes touch, on 1, 3 and 14 threads, with
  [`threads_exact.py`](../../validation/investigations/cleanroom-fast/threads_exact.py)).

## fMRIPrep's call (wall time; in brackets the speed-up against FSL)

| Oracle run (mask voxels) | FSL | replica (1) | original (1) | original (14) |
|---|---|---|---|---|
| ds000005 (761,924; 1 × 1.33 × 1.33 mm) | 21.85 s | 15.25 s (1.4×) | 2.43 s (9.0×) | 0.39 s (56×) |
| ds000228, a 3.5-year-old (1,229,002; 1 mm) | 34.01 s | 23.30 s (1.5×) | 3.76 s (9.0×) | 0.73 s (47×) |
| ds005003, a glioma (1,643,850; 1.2 × 0.98 × 0.98 mm) | 59.61 s | 31.33 s (1.9×) | 5.21 s (11.4×) | 0.89 s (67×) |

Against the replica (the same machine and conditions): the original is 6.0 to 6.3 times faster
on one thread and 32 to 39 times on 14.

## Where the time goes, and why it is faster

fMRIPrep's call on ds005003 (the largest), each stage's time on 1 and 14 threads, before this
relay's speed work (commit `fa0a5a1`) and after it (`f5b8c6b`; profiled with temporary timers,
never committed):

| Stage | before, 1 thread | before, 14 | after, 1 | after, 14 |
|---|---|---|---|---|
| initialisation (percentiles, 19 iterations without the MRF) | 0.88 s | 0.31 s | 0.89 s | 0.12 s |
| the four MRF iterations (random start and five sweeps each) | 2.93 s | 2.86 s | 1.66 s | 0.19 s |
| partial volumes: the 3 + 3 × 101 likelihoods per voxel | 3.88 s | 0.29 s | 1.72 s | 0.12 s |
| partial volumes: the mixel ICM sweep | 0.14 s | 0.14 s | 0.14 s | 0.02 s |
| writing the twelve images | 0.77 s | 0.43 s | 0.73 s | 0.33 s |
| whole command | 8.86 s | 4.23 s | 5.31 s | 0.96 s |

- **The MRF sweeps and the mixel ICM in parallel with the serial result.** Both are Gauss-Seidel
  sweeps in storage order (`specs/fast.md` §4.6, §4.8.7, §10): each voxel reads its 18
  neighbours, the earlier ones already updated. The earlier neighbours of a voxel in row `(y, z)`
  lie in the rows `(y-1..y+1, z-1)` and `(y-1, z)`, its later ones in `(y+1, z)` and
  `(y-1..y+1, z+1)`. So the planes run as a pipeline (`Grid::gauss_seidel`): as many tasks as
  threads take the planes in increasing order, each plane's rows in order, and row `(y, z)` starts
  once plane `z-1` has finished row `y+1`. Every voxel then reads exactly the values the serial
  sweep gives it, at any thread count; no red-black or Jacobi order (which would change the
  posteriors and need §11's thresholds) is used. The posteriors are shared as atomics (plain
  loads and stores; the row counters' release and acquire order them), so the crate stays free
  of `unsafe`. A unit test compares the pipeline with the serial sweep bit for bit on 2, 5 and 9
  threads. The four iterations: 2.86 s to 0.19 s on 14 threads.
- **The MRF update itself** (one thread): its per-voxel scratch on the stack for 2 to 4 classes
  (the same additions in the same order): 0.73 s to 0.41 s per iteration.
- **The partial volumes' likelihoods**: per voxel, 3 pure-class and 3 × 101 mixed-type terms, each
  a double `exp`. The terms of 64 voxels are computed at once and their `exp` by
  `larmorx_core::math::exp_in_place`, new in this relay: the correctly rounded `exp`'s fast path
  four values at a time without branches (vectorised with AVX2 and FMA through the core's audited
  runtime dispatch; the portable build elsewhere), and `exp` itself for any value outside its fast
  path or failing its rounding test, so every result has `exp`'s bits (tested on both builds
  against `exp`: 1.2 M and 0.6 M values, every boundary, the accurate path's database). Each
  voxel's sum keeps its fraction order. 3.88 s to 1.72 s on one thread; 0.29 s to 0.12 s on 14.
  Several channels' likelihoods and partial volumes use the same batches.
- **The statistics**: FSL's blocked sums (§8.3: partial sums of `N + 1` values added in order) run
  block by block in parallel, each block in order and the blocks' sums added in block order: the
  same bits whatever the thread count. The percentiles' sort runs in parallel with a unique order
  (values that compare equal ordered by their bits).
- **Several channels** (`-S n`): the MRF sweeps on the same pipeline, the class statistics'
  independent sums (`Σ q` in single precision, `Σ q y` and the covariances in double, each still
  over the voxels in order) one task each, the bias fields' per-voxel weights and residuals in
  parallel. `-S 2 -B -b -g -p` on the MNI T1w and T2w at 2 mm: 2.20 s to 1.53 s on one thread,
  1.10 s to 0.36 s on 14.
- **Writing**: the twelve images are 452 MB uncompressed; writing them uncompressed takes as long
  (0.49 s on 14 threads), so the stage is bound by the files themselves, not by compression (which
  already runs in parallel). Expanding the mask's values to the image runs row by row in
  parallel.

What was measured and left out:
- **Skipping negligible terms** of the partial-volume sums (a term below a quarter of the sum's
  last unit cannot change it, decidable without the `exp`): exact, but on ds005003 only 9,691 of
  498 million terms qualify (the classes' Gaussians overlap widely), and the test cost more than
  it saved.
- **Four voxels' sums interleaved** in scalar code: 4 % (the `exp` itself is the limit), replaced
  by the batched `exp`.
- **A batched `expf`** for the initialisation's single-precision likelihoods (19 × 3 per voxel,
  about 0.2 s of the 0.89 s on one thread) and the MRF's three `exp` per voxel (a batch of three
  inside the serial sweep: about 6 % of the sweeps): not done.

Memory: the original's peak on ds005003 is 211 MiB on one thread and 216 MiB on 14
(`/usr/bin/time`; before the speed work 210 and 223 MiB): the posteriors' atomic view reuses
their storage, and the batches are 64 voxels. The memory test against FSL's peak is sub-task 4's
(`tests/parity/test_memory_peak.py`).
