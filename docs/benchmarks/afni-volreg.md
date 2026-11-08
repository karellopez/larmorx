# Benchmarks: 3dvolreg (`lx.afni.volreg`, `larmorx afni 3dvolreg`)

3dvolreg's clean-room original (`crates/larmorx-afni`, modules `volreg` and `cli::volreg`) against
AFNI 26.3.05's `3dvolreg` and the bit-exact replica (`crates/larmorx-afni-replica`, a black box for
the original's agent), on fMRIPrep 25.2.6's RobustAverage call (`-Fourier -twopass -zpad 4
-prefix o.nii.gz -1Dfile -1Dmatrix_save -maxdisp1D`) on the `_sliced` inputs the parity suite
re-makes from the oracle runs in fMRIPrep's image (`oracles/inputs/afni-volreg-original/fmriprep/`).
Wall time of the whole command (reading, both passes, writing) and its peak resident memory.

- **Generated:** 2026-11-08 on Linux x86_64 (the main machine: AMD Ryzen 9 7945HX, 16 cores / 32
  threads), larmorx at commit `22d7884` (the original's part, relay 3), inside memory boxes, with
  [`perf.py`](../../validation/investigations/afni-volreg-original/perf.py) (`--afni-fresh
  --repeat 3`: the median of three runs, AFNI timed afresh through the oracle cache with the cache
  off for its runs; ds005454 one AFNI and replica run, three of the original). The standalone
  `larmorx` binary (`target/release/larmorx`), `LARMORX_IMPLEMENTATION=original` or `replica`.
- **The machine was shared**: another agent's random testing ran beside this one (load average 23
  to 26 of 32 threads), so every time is long (AFNI took 118 s on ds005454 here against 76 s in a
  quieter measurement the same day) and the many-thread times are the most affected; the ratios
  between the programs measured together are the information.
- **The same result:** every run's outputs are checked by the parity suite `afni-volreg-original`
  (its record [afni-volreg-original](../validation/afni-volreg-original.md)): the original's at
  1, 2, 5 and 14 threads byte for byte the same.
- AFNI's 3dvolreg and the replica run on one thread (3dvolreg has no OpenMP loop on this path).

## fMRIPrep's RobustAverage calls (wall time; in brackets the speed-up against AFNI)

| Input (voxels × volumes) | AFNI | replica (1) | original (1) | original (4) | original (14) |
|---|---|---|---|---|---|
| ds000005 run 1 (64 × 64 × 34 × 20) | 9.03 s | 3.39 s (2.7×) | 1.75 s (5.2×) | 0.76 s (12×) | 0.61 s (15×) |
| ds003592 echo 1 (64 × 64 × 43 × 20) | 11.10 s | 3.69 s (3.0×) | 2.47 s (4.5×) | 1.10 s (10×) | 0.82 s (14×) |
| ds004958 (104 × 104 × 72 × 3, uint16) | 6.35 s | 3.39 s (1.9×) | 1.96 s (3.2×) | 0.76 s (8.4×) | 0.60 s (11×) |
| ds005040 PEPOLAR average (64 × 64 × 40 × 2) | 1.17 s | 0.45 s (2.6×) | 0.25 s (4.7×) | 0.14 s (8.4×) | 0.13 s (9.0×) |
| ds005454 (160 × 160 × 96 × 20, multiband) | 118.2 s | 72.8 s (1.6×) | 42.1 s (2.8×) | 15.4 s (7.7×) | 10.5 s (11×) |

On one thread the original is 1.5 to 1.9 times the replica's speed; with threads its work inside
each volume (the shears' rows and planes, the coarse search's rotations and scores, the blurs, the
derivative images) runs in parallel, deterministically.

**The quieter measurement** (relay 2 of the part, 2026-11-08, an earlier build of the same design,
the other agent's work beside it lighter; `perf.py`, one run each, AFNI's time from its first run):

| Input | AFNI | replica (1 / 14) | original (1 / 14) |
|---|---|---|---|
| ds000005 run 1 | 7.26 s | 2.39 / 2.26 s | 1.31 / 0.36 s |
| ds003592 echo 1 | 8.62 s | 2.95 / 2.69 s | 1.80 / 0.48 s |
| ds004958 | 4.46 s | 2.08 / 1.97 s | 1.24 / 0.30 s |
| ds005454 | 75.6 s | 40.45 / 39.54 s | 23.79 / 4.69 s |

## Peak memory (MiB; the rule: at most 1.1 × AFNI's peak + 8 MiB, CLAUDE.md "Memory similar to the original's")

| Input | AFNI | allowance | replica (1) | original (1) | original (4) | original (14) |
|---|---|---|---|---|---|---|
| ds000005 run 1 | 23.1 | 33.4 | 27.7 | 25.8 | 30.7 | 38.3 |
| ds003592 echo 1 | 28.1 | 39.0 | 32.2 | 30.9 | 39.8 | 46.5 |
| ds004958 | 54.2 | 67.6 | 65.1 | 60.9 | 60.7 | 66.2 |
| ds005040 PEPOLAR | 15.5 | 25.1 | 17.5 | 16.8 | 17.2 | 20.7 |
| ds005454 | 311.3 | 350.4 | 338.3 | 332.3 | 339.5 | 345.8 |

At one thread the original is within the rule on every call (the parity smoke tier's
`test_volreg_original_needs_no_more_memory_than_afni` holds it for fMRIPrep's call on a 64 × 64 × 40
× 20 series and on ds005454, in both modes). With threads the extra memory is about 1 MiB per
thread (each thread's plane and batch buffers, the allocator's arenas): no full-size copy per
thread, the volumes registered one after another.

## Where the time goes (ds005454, one thread)

From relay 2's profile (uncompressed output, 19.1 s then): the first pass's coarse search (44
nearest-neighbour rotations per volume on its support box, 7.3 s, and their shift scores, 2.2 s:
each candidate's float32 sum kept sequential, as the spec's "first strictly better" rule needs the
same scores), the final pass's Fourier resamplings (6.7 s: a batched double-precision FFT four row
pairs at a time), the first pass's linear fits and blurs (2 s), and reading and writing.
