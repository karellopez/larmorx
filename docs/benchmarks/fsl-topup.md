# Benchmarks: topup and applytopup (`lx.fsl.topup`, `larmorx fsl topup`, `lx.fsl.applytopup`)

topup's clean-room original (`crates/larmorx-fsl`, module `topup`) against FSL 6.0.7.23's `topup`
and `applytopup` and the bit-exact replicas (`larmorx-nc fsl topup`, `fsl applytopup`, FSL
Licence), on fMRIPrep's topup call (sdcflows' `b02b0.cnf`, `specs/topup.md` §1) on its two oracle
runs, and on one applytopup least-squares restoration. Wall time of the whole command, **one run
each (informal)**.

- **Generated:** 2026-11-03 on Linux x86_64 (the main machine: AMD Ryzen 9 7945HX, 16 cores / 32
  threads), with
  [`validation/investigations/cleanroom-topup/bench.py`](../../validation/investigations/cleanroom-topup/bench.py)
  (`--threads 1 14 --fsl-threads 14 --json`), larmorx at commit `11f3cb0` (topup's clean-room
  original, relay 4), inside a 16 GiB memory box. **The machine was shared**: another agent
  (the `fslmaths` and `fslstats` replica) worked beside this one (load average 5.1 of 32 threads
  before the runs, 5.0 after), so the original was given at most 14 threads and every time may be
  a little long; the runs: [fsl-topup.json](fsl-topup.json).
- **FSL was run again for this report, five times** (fMRIPrep's two calls on one thread and with
  `--nthr=14`, and the applytopup call): a timed run is the one kind the oracle cache cannot
  serve, so each went through `oracle_cache.run` with a reason never to store it, and nothing was
  stored. FSL's `--nthr` default is 1 (its usage text), so its one-thread time is its default
  time. With `--nthr=14` FSL's result is not its one-thread result (FSL-116, `specs/topup.md`
  §9.3); the time is shown for comparison only.
- **What is timed:** each program as its own process in FSL's clean environment (FSL off the
  `PATH`, `FSLOUTPUTTYPE=NIFTI_GZ`, OpenBLAS's Haswell kernels): reading the inputs, the
  estimation, writing every output of the call (`--out`'s coefficients and movements, `--iout`
  and its mean, `--fout`, `--jacout`, `--rbmout`, `--dfout`, the log). The original's thread
  count is `OMP_NUM_THREADS`; the replica runs on one thread (it is FSL's one-thread result).
- **The same result:** the replica's outputs are FSL's one-thread outputs byte for byte (checked
  by the script on each run); the original's are within its record's thresholds (`fsl-topup`
  suite, `specs/topup.md` §12) and **the same bits at any thread count** (checked by the script on
  each run). Every speed change of this relay leaves the original's outputs, standard output and
  standard error byte for byte as they were before it (commit `e86add7`): every case of the
  `fsl-topup` suite's standard tier (173 cases, fMRIPrep's two calls included, in both modes: 346
  runs) at 1, 7 and 14 threads, with
  [`speed_identity.py`](../../validation/investigations/cleanroom-topup/speed_identity.py) (FSL
  is not run). So every suite status is what it was.

## fMRIPrep's topup call (wall time; in brackets the speed-up against FSL on one thread)

| Oracle run (grid) | FSL (1) | FSL (`--nthr=14`) | replica (1) | original (1) | original (14) |
|---|---|---|---|---|---|
| ds005040 (64 × 64 × 40 × 2, 3 × 3 × 3.3 mm) | 32.63 s | 12.32 s (2.6×) | 29.69 s (1.1×) | 2.83 s (11.5×) | 0.80 s (41×) |
| ds001600 (80 × 80 × 60 × 2, 3 mm) | 77.35 s | 27.38 s (2.8×) | 71.35 s (1.1×) | 7.70 s (10.0×) | 1.98 s (39×) |

Against the replica (the same machine and conditions): the original is 9.3 to 10.5 times faster
on one thread and 36 to 37 times on 14. Against FSL with 14 threads of its own: 4.4 to 4.6 times
faster on one thread, 14 to 15 times on 14.

## applytopup's least-squares restoration (`--method=lsr`, the default)

ds005040's pair (the merged image's two volumes as `up` and `dn`) with FSL's topup result of
fMRIPrep's call above (`--imain=up,dn --inindex=1,2 --datain=<encfile> --topup=<FSL's result>
--out=hifi`): 64 × 40 lines of 64 voxels along the phase encoding.

| Call | FSL (1) | replica (1) | original (1) | original (14) |
|---|---|---|---|---|
| applytopup `lsr`, ds005040 | 0.42 s | 3.47 s (0.12×) | 0.13 s (3.2×) | 0.04 s (10×) |

The replica is slower than FSL here (its dense per-line LU and inverse, FSL's arithmetic,
`specs/topup.md` §9.4); the original solves each line's normal equations by a Cholesky
factorisation, the lines in parallel.

## Where the time goes, and why it is faster

fMRIPrep's call on ds001600 (the larger), each stage's time on 1 and 14 threads, before this
relay's speed work (commit `e86add7`) and after it (`11f3cb0`; profiled with temporary timers,
never committed; one run each, the machine shared):

| Stage | before, 1 thread | before, 14 | after, 1 | after, 14 |
|---|---|---|---|---|
| 142 cost evaluations (the predictions, the field and its derivative, the regulariser) | 10.03 s | 2.51 s | 2.88 s | 0.69 s |
| 139 gradients | 2.21 s | 0.67 s | 1.18 s | 0.29 s |
| 15 Gauss-Newton Hessians (the subsampled levels) | 0.65 s | 0.16 s | 0.54 s | 0.13 s |
| 15 Levenberg-Marquardt solves (the damped copy and the conjugate gradients) | 4.11 s | 0.79 s | 2.44 s | 0.29 s |
| the levels' images, the refits, the scaling and regridding | 0.50 s | 0.07 s | 0.27 s | 0.06 s |
| the outputs (full resolution, writing) | 0.43 s | 0.10 s | 0.38 s | 0.09 s |
| whole command | 18.15 s | 4.43 s | 7.81 s | 1.72 s |

The four scaled-conjugate-gradient levels at full resolution take most evaluations and gradients
(two of each per iteration); the three Levenberg-Marquardt levels at half resolution take the
Hessians and the solves. The page faults of the whole command (first touches of new memory, from
`/usr/bin/time`, runs of their own): 578,000 before and 123,000 after on one thread, 146,000 and
125,000 on 14.

- **The one-axis operators** (the field and its derivatives from the coefficients, the
  gradient's projections, the regulariser, the refits, the smoothing: every separable pass,
  `Op1::apply_axis`): along y and z a row's weights are the same for every x, so each weight is
  applied to a whole line or plane at once (contiguous, vectorised by the compiler); along x the
  lines are taken 64 at a time, transposed into a small buffer, and treated as along y. Every
  value is still its row's sum in slot order, the zero weights left out, so every result has the
  same bits. The regulariser's `R c` (six terms of three passes each) computes the x passes once
  per derivative order and adds each term's z pass to the sum directly; the field and its
  derivative share their x and y passes. The regulariser: 30 ms to about 7 ms per evaluation on
  one thread at knot spacing 1.
- **The cost's voxels** are written in place, slice by slice, into the evaluation's arrays (no
  intermediate rows); the spline's indices are computed once for the value and the derivative
  (`taps_both`: the indices wrapped only at the image's ends, the weights of the same
  expressions); the single-axis case (fMRIPrep's: no movements, one phase-encoding axis) has its
  own loop with its invariants taken out.
- **The gradient's and the Hessian's voxel weights** are written in place (one flat array of
  weights per voxel instead of a small array each).
- **The Levenberg-Marquardt solve**: the damped Hessian is laid out by offset (`data[d · n + k]`:
  row `k`'s entry at offset `d`, the stencil transposed eight offsets, one cache line, at a time),
  so the conjugate gradients' products run over whole lines of rows (vectorised), each row's sum
  in the stencil's order (`dz`, `dy`, `dx` ascending). The entries a row skipped are outside the
  grid or zero, and a zero entry adds `±0` to a sum that is never `-0`, so every product has the
  same bits. It replaces the damped copy of the stencil, so the memory is the same.
- **Memory reused within a level** (`topup::pool::Pool`): the evaluations, gradients, the
  regulariser's passes, the Hessians and the solves give their large arrays back, and the next
  ones take them at their exact length (zeroed where they are added to; the evaluation's arrays,
  every value of which is written, as they are) instead of new memory from the system, whose
  first touch costs a page fault per 4 KiB page; on many threads the faults queue in the kernel.
  The scaled conjugate gradients keep their trial points and differences in reused vectors (the
  same element-by-element arithmetic). The peak memory is lower (ds001600's call: 187 MB before
  and 146 MB after on one thread, 175 MB and 159 MB on 14).
- **Threads:** every parallel loop gives each value to one task (a voxel, a line, a plane, a row
  of the stencil) and every sum has a fixed order (the dot products in fixed blocks summed in
  order, the cost and the gradient's sums per slice, then in order), so the results do not
  depend on the thread count (checked at 1, 7 and 14 threads, above).

## What was not done (`specs/topup.md` §11), and why

- **A banded or periodic-banded solve of applytopup's lines**: the restoration takes 0.13 s on one
  thread (the images' input and output most of it); a band solver would change the factorisation's
  rounding, and the record's applytopup cases would have to be measured again.
- **Fewer bytes per solver product** (a matrix-free product from the voxel weights, the stencil's
  symmetry, single-precision storage): each changes the order or precision of the sums, so the
  steps and the fields would change within §12's thresholds but not keep their bits. On one thread
  the products are bound by memory bandwidth (26 MB per product at the subsampled levels), so these
  are the remaining large gain (the solves are a third of the one-thread time).
- **SIMD with runtime dispatch** (AVX2 copies of the passes): the crate forbids `unsafe`, and the
  vectorised loops above already run with the baseline SSE2 at full width; a dispatched copy
  would belong in `larmorx-core`'s audited dispatch (as `exp_in_place` for FAST).
- **The regulariser's y and z passes fused per strip of x columns** (each pass now reads and writes
  a 3 MB array at knot spacing 1): about 3 ms of an evaluation's 7 ms on 14 threads; not taken.
