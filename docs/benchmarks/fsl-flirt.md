# Benchmarks: FLIRT (`lx.fsl.flirt`, `larmorx fsl flirt`)

FLIRT's clean-room original (`crates/larmorx-fsl`) against FSL 6.0.7.23's `flirt`
(single-threaded) and the bit-exact replica (`larmorx-nc`, FSL Licence), on the fsl-flirt suite's
standard tier's real cases. Wall time of the whole command, **one run each (informal)**.

- **Generated:** 2026-11-02 on Linux x86_64 (the main machine: AMD Ryzen 9 7945HX, 16 cores / 32
  threads), with
  [`validation/investigations/cleanroom-flirt/bench.py`](../../validation/investigations/cleanroom-flirt/bench.py)
  (`--threads 1,14 --json`), larmorx at commit `04f5462` (part 2, relay 3), inside a memory box.
  **The machine was shared**: another agent's parity runs used part of it (load average before the
  runs 5.6 of 32 threads), so the original was given at most 14 threads and every time may be a
  little long; the runs, with their environment: [fsl-flirt.json](fsl-flirt.json).
- **What is timed:** each program as its own process in FSL's clean environment (FSL off the
  `PATH`, `FSLOUTPUTTYPE=NIFTI_GZ`, OpenBLAS's Haswell kernels): reading the inputs, the
  registration, writing the matrix (and image). The thread count is `OMP_NUM_THREADS`; FSL and
  the replica run on one thread.
- **The same result:** the replica's matrices are FSL's bit for bit; the original's are within its
  record's thresholds ([validation/fsl-flirt.md](../validation/fsl-flirt.md)), and the same bits
  at any thread count: the speed changes of this part were checked to leave every result of the
  suite's 134 `flirt` cases byte for byte as it was, at 1 and 7 threads (the 11 boundary-based
  cases also at 13), with
  [`same_bits.py`](../../validation/investigations/cleanroom-flirt/same_bits.py).

## The standard tier's real cases (wall time; in brackets the speed-up against FSL)

The original on 1 and 14 threads; the replica and FSL on one. fMRIPrep's call is its
`flt_bbr` node (`-cost bbr`, `bbr.sch`, the full-size T1w, 6 dof) on the tier's three oracle
runs; the others register ds005040's 3 mm BOLD reference to its T1w at half resolution (the
suite's images; the field-map case is the suite's `bbr/fieldmap-mask`).

| Case | FSL | replica (1) | original (1) | original (14) |
|---|---|---|---|---|
| fMRIPrep's BBR call, ds005040 | 46.70 s | 56.86 s (0.8×) | 38.75 s (1.2×) | 3.76 s (12.4×) |
| fMRIPrep's BBR call, ds003592 | 35.44 s | 43.00 s (0.8×) | 28.00 s (1.3×) | 2.71 s (13.1×) |
| fMRIPrep's BBR call, ds000228 | 18.34 s | 22.64 s (0.8×) | 14.67 s (1.2×) | 1.35 s (13.6×) |
| default 12 dof (BOLD reference to T1w) | 0.97 s | 1.31 s (0.7×) | 1.10 s (0.9×) | 0.30 s (3.2×) |
| 6 dof | 0.72 s | 0.78 s (0.9×) | 0.69 s (1.0×) | 0.20 s (3.6×) |
| BBR with a field map and its mask (-pedir -2) | 255.62 s | 269.25 s (0.9×) | 80.82 s (3.2×) | 7.04 s (36.3×) |
| -applyxfm (trilinear) | 0.22 s | 0.21 s (1.1×) | 0.12 s (1.9×) | 0.04 s (5.6×) |

## Where the time goes, and why it is faster

- **The boundary-based cost** (fMRIPrep's call: `bbr.sch` at 1 mm, its evaluations of the white
  matter's boundary pairs): each pair's term (two trilinear samples of the input, the `tanh`
  contrast) is independent, so the pairs are computed in parallel and summed in their order
  (double precision, the same sum at any thread count). One thread 1.2 to 1.3 times FSL's speed,
  14 threads 12 to 14 times.
- **The correlation ratio** (the default cost, every other registration): the reference level's
  points sampled slice by slice. Its bins' sums are single-precision running sums in the
  sampling order (FSL's arithmetic); in parallel the slices are sampled at once, each slice's
  points sorted by bin keeping their order, and each bin's sums then run over the slices in
  order, the bins at once: the same additions in the same order, so the same bits. On one thread
  the sequential pass (no sorting); the search's grid points and minima run in parallel. One
  thread is at FSL's speed on these small registrations (the same sampling and the same
  sequential running sums); 14 threads 3 to 4 times: the optimisations after the search are line
  searches, one evaluation after the other, each only 43,000 to 350,000 points.
- **The field map's search** (BBR with `-fieldmap`, `-pedir`): a boundary point outside the field
  map's mask looks for the nearest point of the mask along the phase-encoding direction, up to
  the map's largest extent in mm, one interpolation per step; the original skips the steps a
  distance map of the mask shows cannot reach it (the same point found): on one thread from more
  than 8 minutes before this part to 81 s (FSL 256 s), the points in parallel on more.
- **Not yet:** SIMD sampling (the trilinear interpolation of 8 or 16 points at once, with runtime
  dispatch, the same bits) and a lower-overhead parallel pass for the correlation ratio's
  evaluations of a few tens of thousands of points: the next speed steps for the one-thread and
  the small registrations.
