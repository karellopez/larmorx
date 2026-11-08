# Benchmarks: sdcflows' field-map estimation (`lx.sdcflows`)

The port of sdcflows 2.17.1's field-map estimation (`crates/larmorx-sdcflows`, `lx.sdcflows`)
against sdcflows itself (its nipype interfaces, in-process, with scikit-learn 1.9.1, SciPy
1.18.1, numpy 2.5.4 and nibabel 5.4.2: the newest Python environment, `oracles/python-newest`),
on fMRIPrep 25.2.6's calls of the oracle runs. Wall time of the whole interface (reading, the
fit or the unwarping, writing every file).

- **Generated:** 2026-11-08 on Linux x86_64 (the main machine: AMD Ryzen 9 7945HX, 16 cores /
  32 threads), larmorx at commit `2ed5b887` (field maps part A, relay 8), inside a memory box
  (24 GiB), with
  [`bench_sdcflows.py`](../../validation/investigations/sdcflows-fieldmap/bench_sdcflows.py)
  (`--repeats 3 --volumes 60 --threads 1,14 --low-memory`: the median of three runs, larmorx after
  one untimed warm-up; its runs: [sdcflows-fieldmap.json](sdcflows-fieldmap.json)).
  `OPENBLAS_CORETYPE=Haswell` and `OPENBLAS_NUM_THREADS=1` (numpy's and SciPy's BLAS at one
  thread, as fMRIPrep's nodes ran: `OMP_NUM_THREADS=1`). **The machine was shared**: another
  agent worked beside this one (load average 2 to 3 of 32 threads), so the times may be a little
  long and the 14-thread times are noisy.
- **The same result:** every file of every larmorx run (`upstream_defects="reproduce"`) checked
  byte for byte (decompressed) against sdcflows' run of the same call: identical in every row.

## BSplineApprox: fMRIPrep's seven `bs_filter` calls

fMRIPrep's parameters (spacing 16 × 16 × 10 mm, median, ridge alpha 1e-4, extrapolation); the
field maps and masks of the oracle runs' working directories. BSplineApprox has no thread option
(scikit-learn's sparse conjugate gradients at one BLAS thread). larmorx at 1 and 14 threads with
its default (the stored collocation matrix at one thread, the weights evaluated at every product
with several), and with `low_memory=True` at one thread (in brackets the speed-up against
sdcflows).

| Call (field-map grid) | sdcflows | larmorx (1) | larmorx (14) | `low_memory` (1) | identical |
|---|---|---|---|---|---|
| ds005454-sub-16/wf_gre16 (112 × 112 × 70) | 16.72 s | 15.26 s (1.10×) | 2.15 s (7.8×) | 23.70 s (0.71×) | yes |
| ds002422-sub-02/wf_auto_00000 (64 × 64 × 24) | 1.96 s | 1.73 s (1.14×) | 0.30 s (6.5×) | 2.72 s (0.72×) | yes |
| ds002422-sub-02/wf_auto_00001 (64 × 64 × 36) | 3.44 s | 3.17 s (1.09×) | 0.50 s (6.9×) | 4.94 s (0.70×) | yes |
| ds001600-sub-1/wf_auto_00000 (64 × 64 × 44) | 2.83 s | 3.29 s (0.86×)¹ | 0.39 s (7.3×) | 3.97 s (0.71×) | yes |
| ds001600-sub-1/wf_auto_00001 (64 × 64 × 44) | 2.97 s | 3.43 s (0.87×)¹ | 0.45 s (6.6×) | 4.16 s (0.71×) | yes |
| ds001600-sub-1/wf_auto_00002 (64 × 64 × 44) | 2.88 s | 3.36 s (0.86×)¹ | 0.40 s (7.2×) | 4.03 s (0.71×) | yes |
| ds002603-sub-01/wf_auto_00000 (128 × 128 × 128) | 27.48 s | 24.87 s (1.10×) | 4.04 s (6.8×) | 45.53 s (0.60×) | yes |

**At one thread the port is 1.09 to 1.14 times faster than scikit-learn** (the stored
collocation matrix: 8 bytes per weight, read once per product; the adjoint scatters the rows in
order, as scipy's `csc_matvec` on `X.T` does), **at 14 threads 6.5 to 7.8 times faster**
(the weights evaluated in parallel at every product), with the same bits at any thread count;
`low_memory=True` (the weights evaluated at every product also at one thread: memory of the
order of the voxels and knots) is 0.60 to 0.72 times sdcflows' speed at one thread.

¹ **The stored matrix's one-thread speed depends on what the process ran before.** The same
code at one thread took 2.50 to 2.70 s on ds001600's three calls in the previous run of this
benchmark (commit `2f6d6744`) and 2.52 to 2.61 s in a fresh process, but 3.3 s here, and 5.5 to
5.7 s after the benchmark's first three calls in the same process, sdcflows' runs included
([`bench_fit_paths.py`](../../validation/investigations/sdcflows-fieldmap/bench_fit_paths.py)
`process`, reproduced, at a load of 2 of 32 threads); after the same larmorx calls without
sdcflows' it stays at 2.55 to 2.59 s, and fixing glibc's mmap threshold
(`GLIBC_TUNABLES=glibc.malloc.mmap_threshold=65536`) does not change it (5.7 s). The weights
evaluated at every product (`low_memory`) vary by less than 1 % in every run, and the bits never
change. Each of fMRIPrep's nodes runs in a process of its own, where the port is the faster one;
the cause (the stored matrix's memory-bound products after sdcflows, nipype, scikit-learn and
numpy have run in the process) is an open follow-up, before any pipeline runs these fits in a
long-lived process.

**Why not the stored matrix at every thread count.** At commit `2f6d6744` the fit read the
stored matrix also with several threads (its transpose for the adjoint: 16 bytes per weight in
all): at 14 threads it was only 2.3 to 3.1 times faster than sdcflows (ds005454 7.40 s,
ds002603 10.95 s: [sdcflows-fieldmap-stored-every-thread-count.json](sdcflows-fieldmap-stored-every-thread-count.json)),
where evaluating the weights in parallel takes 2.15 s and 4.04 s: the stored matrix's products
are bound by memory bandwidth and its transpose is built serially
([`bench_fit_paths.py`](../../validation/investigations/sdcflows-fieldmap/bench_fit_paths.py)
`paths`: ds005454 7.63 s against 2.32 s, ds002422 0.62 s against 0.30 s). Since `3c9c10af`
the matrix is stored at one thread only. Memory (`tests/parity/test_memory_peak.py`, one
thread): 83 MiB against sdcflows' 366 on ds002422's call, 468 against 1,473 on ds005454's.

## ApplyCoeffsField: ds005454's field on its BOLD series

ds005454-sub-16's coefficients (`bs_filter`), its field-map registration
(`fmapreg_wf/coregister`, `fmap2data_xfm`), readout time 0.0429303 s along `j-`, the Jacobian,
cubic interpolation, on the first 60 volumes of its BOLD series (160 × 160 × 96, int16): the whole
280-volume series would need about 50 GiB in sdcflows (3.7 GiB for 20 volumes, the memory test),
beyond this part's memory box. sdcflows' `num_threads` (its asyncio semaphore over the volumes)
and larmorx's `n_threads` alike.

| Call | threads | sdcflows | larmorx | identical |
|---|---|---|---|---|
| ds005454-sub-16, 60 volumes (160 × 160 × 96 × 60) | 1 | 31.83 s | 18.68 s (1.7×) | yes |
| | 14 | 11.29 s | 6.56 s (1.7×) | yes |

The unwarping is SciPy's `map_coordinates` (cubic, with its prefilter) per volume in both; the
port's replica (`larmorx-interp`, the resampler's) runs it with the shift, the Jacobian and the
writing in Rust, one volume at a time (829 MiB against sdcflows' 3,741 MiB on 20 volumes).
