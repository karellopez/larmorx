# Benchmarks: BET (`lx.fsl.bet`, `lx.fsl.betsurf`, `larmorx fsl bet|bet2|betsurf`)

BET's clean-room original (`crates/larmorx-fsl`) against FSL 6.0.7.23's `bet`, `bet2` and
`betsurf` (single-threaded) and the bit-exact replica (`larmorx-nc`, FSL Licence), on fMRIPrep's
call, on real T1-weighted images of the test data, and betsurf on real heads with and without a
T2. Wall time of the whole command, the median of 3 runs.

- **Generated:** 2026-10-31 on Linux x86_64 (the main machine: AMD Ryzen 9 7945HX, 16 cores / 32
  threads), with
  [`validation/investigations/cleanroom-bet/bench.py`](../../validation/investigations/cleanroom-bet/bench.py)
  (`--threads 1,24 --repeat 3 --json`), larmorx at commit `e45d805` (BET's part 2b: betsurf's
  original; the program's code is that of `21762e8`); load average before the runs 2.2 (no other
  job running). The runs, with their environment: [fsl-bet.json](fsl-bet.json).
- **What is timed:** each program as its own process in a clean environment (FSL off the
  `PATH`, `FSLOUTPUTTYPE=NIFTI_GZ`): reading the gzipped inputs, the computation, writing the
  gzipped outputs. The thread count is `OMP_NUM_THREADS` (FSL's programs are single-threaded; the
  replica's bet2 runs on one thread whatever it is given, its betsurf smooths in parallel).
- **The same result:** bet2's and bet's outputs on these calls are FSL's bit for bit, in both
  implementations; betsurf's are the replica's bit for bit, and the original's within its record's
  thresholds ([validation/fsl-bet.md](../validation/fsl-bet.md)).

## fMRIPrep's call: `bet <boldref> <out> -f 0.20 -m` (ds000005, run 3: 64 × 64 × 34)

| Program | Threads | Median | Speed-up vs FSL |
|---|---|---|---|
| FSL 6.0.7.23 `bet` | 1 | 0.67 s | 1.0× |
| replica (`larmorx-nc fsl bet`) | 1 | 0.39 s | 1.7× |
| replica | 24 | 0.38 s | 1.8× |
| **original** (`larmorx fsl bet`) | 1 | 0.24 s | **2.8×** |
| **original** | 24 | 0.06 s | **11.6×** |

## `bet2 <T1w> <out> -m`

| Image | FSL | replica (1) | replica (24) | original (1) | original (24) | original vs FSL (1 / 24) |
|---|---|---|---|---|---|---|
| adult (ds000005: 160 × 192 × 192, 1 × 1.33 × 1.33 mm) | 1.25 s | 0.72 s | 0.72 s | 0.49 s | 0.17 s | 2.5× / 7.3× |
| child of 3.5 years (ds000228: 176 × 192 × 192, 1 mm) | 1.32 s | 0.82 s | 0.82 s | 0.53 s | 0.18 s | 2.5× / 7.4× |
| adult of 82 years (ds003592: 192 × 256 × 256, 1 mm) | 2.02 s | 1.03 s | 1.04 s | 0.70 s | 0.28 s | 2.9× / 7.2× |
| 7 T MP2RAGE (ds005454: 224 × 272 × 288, 0.8 mm; the first pass reruns) | 3.06 s | 1.75 s | 1.76 s | 1.21 s | 0.47 s | 2.5× / 6.5× |

## `betsurf`: `--t1only -m <t1> <mesh> <matrix> <out>` and `-m <t1> <t2> <mesh> <matrix> <out>`

bet2's mesh of each T1 and FLIRT's matrix to MNI152 (and the T2 registered to the T1) are the
inputs the parity suite derived with FSL (`oracles/inputs/fsl-bet/`).

| Head | FSL | replica (1) | replica (24) | original (1) | original (24) | original vs FSL (1 / 24) |
|---|---|---|---|---|---|---|
| T1 only: adult (ds000005) | 6.51 s | 3.91 s | 2.58 s | 2.14 s | 0.61 s | 3.0× / 10.7× |
| T1 only: adult of 82 years (ds003592) | 11.76 s | 6.37 s | 3.75 s | 3.28 s | 0.92 s | 3.6× / 12.8× |
| T1 and T2: child of 7 years (ds000256) | 18.42 s | 9.86 s | 5.81 s | 5.32 s | 1.36 s | 3.5× / 13.5× |
| T1 and T2: glioma (ds005003) | 12.23 s | 6.83 s | 4.39 s | 3.89 s | 0.98 s | 3.1× / 12.5× |

## Where the time goes, and why it is faster

- **bet2's deformation** (1000 iterations of 2562 vertices per pass): every vertex's step is
  computed from the previous iteration's positions only, so the vertices are updated in parallel,
  with the same result at any thread count; the triangle normals once per iteration, summed per
  vertex.
- **The self-intersection measure** finds the pairs closer than the mean neighbour distance in a
  grid of cells of that size instead of over all 3.3 million pairs, then sums them in index order.
- **The mask's fill** is a stack fill over a byte grid; the rasterisation draws each triangle once.
- **betsurf's profiles** are cast in parallel, each vertex's estimates into its own list, counted
  into the three counting images in vertex order (small integers: exact in any order); the
  counting images are smoothed one axis at a time, each axis's lines in parallel, in two buffers;
  the three surfaces are deformed with every vertex's step computed in parallel.
- **Images** are read and written once, without copies (gzip in parallel blocks): with one thread
  most of bet2's remaining time is reading and compressing.
- Memory: the original's peak is within the project's rule on every measured call
  (`tests/parity/test_memory_peak.py`: bet2 71 MiB against FSL's 72, `bet -R` 117 against 103
  plus the interpreter's 13, `bet -F` on a 30-volume series 170 against fslmaths' 217, betsurf 144
  against FSL's 175).
