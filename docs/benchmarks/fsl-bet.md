# Benchmarks: BET (`lx.fsl.bet`, `larmorx fsl bet2|bet`)

BET's clean-room original (`crates/larmorx-fsl`) against FSL 6.0.7.23's `bet2` and `bet`
(single-threaded) and the bit-exact replica (`larmorx-nc`, FSL Licence), on fMRIPrep's call and
on real T1-weighted images of the test data. Wall time of the whole command, the median of 3
runs.

- **Generated:** 2026-10-30 on Linux x86_64 (the main machine: AMD, 16 cores / 32 threads), with
  [`validation/investigations/cleanroom-bet/bench.py`](../../validation/investigations/cleanroom-bet/bench.py)
  (`--threads 1,24 --repeat 3`), larmorx at commit `cef5708`; load average before the runs 1.5
  (no other job running).
- **What is timed:** each program as its own process in a clean environment (FSL off the
  `PATH`, `FSLOUTPUTTYPE=NIFTI_GZ`): reading the gzipped input, the surface, the mask, writing
  the gzipped brain and mask. The thread count is `OMP_NUM_THREADS` (FSL's programs are
  single-threaded; the replica's BET runs on one thread whatever it is given).
- **Same result:** on these calls the original's outputs are FSL's bit for bit, but on the 7 T
  image, whose first pass self-intersects and reruns (the open question of the record:
  [validation/fsl-bet.md](../validation/fsl-bet.md)).

## fMRIPrep's call: `bet <boldref> <out> -f 0.20 -m` (ds000005, run 3: 64 × 64 × 34)

| Program | Threads | Median | Speed-up vs FSL |
|---|---|---|---|
| FSL 6.0.7.23 `bet` | 1 | 0.66 s | 1.0× |
| replica (`larmorx-nc fsl bet`) | 1 | 0.39 s | 1.7× |
| replica | 24 | 0.38 s | 1.7× |
| **original** (`larmorx fsl bet`) | 1 | 0.24 s | **2.8×** |
| **original** | 24 | 0.06 s | **11×** |

## `bet2 <T1w> <out> -m`

| Image | FSL | replica (1) | replica (24) | original (1) | original (24) | original vs FSL (1 / 24) |
|---|---|---|---|---|---|---|
| adult (ds000005: 160 × 192 × 192, 1 × 1.33 × 1.33 mm) | 1.26 s | 0.72 s | 0.72 s | 0.42 s | 0.16 s | 3.0× / 7.9× |
| child of 3.5 years (ds000228: 176 × 192 × 192, 1 mm) | 1.35 s | 0.80 s | 0.83 s | 0.45 s | 0.18 s | 3.0× / 7.5× |
| adult of 82 years (ds003592: 192 × 256 × 256, 1 mm) | 2.04 s | 1.03 s | 1.02 s | 0.68 s | 0.26 s | 3.0× / 7.8× |
| 7 T MP2RAGE (ds005454: 224 × 272 × 288, 0.8 mm; the first pass reruns) | 3.08 s | 1.78 s | 1.79 s | 1.19 s | 0.53 s | 2.6× / 5.8× |

## Where the time goes, and why it is faster

- **The deformation** (1000 iterations of 2562 vertices per pass): every vertex's step is computed
  from the previous iteration's positions only, so the vertices are updated in parallel, with the
  same result at any thread count; the triangle normals once per iteration, summed per vertex.
- **The self-intersection measure** finds the pairs closer than the mean neighbour distance in a
  grid of cells of that size instead of over all 3.3 million pairs, then sums them in index order.
- **The mask's fill** is a stack fill over a byte grid; the rasterisation draws each triangle once.
- **Images** are read and written once, without copies (the brain and the masks are the only
  full-size buffers besides the input; gzip in parallel blocks): with one thread most of the
  remaining time is reading and compressing.
- Memory: the original's peak is FSL's or below on every measured call
  (`tests/parity/test_memory_peak.py`: bet2 71 MiB against FSL's 72, `bet -R` 117 against 103 plus
  the interpreter's 13, `bet -F` on a 30-volume series 170 against fslmaths' 217).
