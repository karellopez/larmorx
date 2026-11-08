# Benchmarks: sdcflows' field-map estimation (`lx.sdcflows`)

The port of sdcflows 2.17.1's field-map estimation (`crates/larmorx-sdcflows`, `lx.sdcflows`)
against sdcflows itself (its nipype interfaces, in-process, with scikit-learn 1.9.1, SciPy
1.18.1, numpy 2.5.4 and nibabel 5.4.2: the newest Python environment, `oracles/python-newest`),
on fMRIPrep 25.2.6's calls of the oracle runs. Wall time of the whole interface (reading, the
fit or the unwarping, writing every file).

- **Generated:** 2026-11-08 on Linux x86_64 (the main machine: AMD Ryzen 9 7945HX, 16 cores /
  32 threads), larmorx at commit `c397728e` (field maps part A, relay 6), inside a memory box
  (24 GiB), with
  [`bench_sdcflows.py`](../../validation/investigations/sdcflows-fieldmap/bench_sdcflows.py)
  (`--repeats 3 --volumes 60 --threads 1,14`: the median of three runs, larmorx after one
  untimed warm-up; its runs: [sdcflows-fieldmap.json](sdcflows-fieldmap.json)).
  `OPENBLAS_CORETYPE=Haswell` and `OPENBLAS_NUM_THREADS=1` (numpy's and SciPy's BLAS at one
  thread, as fMRIPrep's nodes ran: `OMP_NUM_THREADS=1`). **The machine was shared**: another
  agent (3dvolreg's clean-room original) worked beside this one (load average about 6 of 32
  threads), so every time may be a little long and the 14-thread times are noisy.
- **The same result:** every file of every larmorx run (`upstream_defects="reproduce"`) checked
  byte for byte (decompressed) against sdcflows' run of the same call: identical in every row.

## BSplineApprox: fMRIPrep's seven `bs_filter` calls

fMRIPrep's parameters (spacing 16 × 16 × 10 mm, median, ridge alpha 1e-4, extrapolation); the
field maps and masks of the oracle runs' working directories. BSplineApprox has no thread option
(scikit-learn's sparse conjugate gradients at one BLAS thread); larmorx at 1 and 14 threads (in
brackets the speed-up against sdcflows).

| Call (field-map grid) | sdcflows | larmorx (1) | larmorx (14) | identical |
|---|---|---|---|---|
| ds005454-sub-16/wf_gre16 (112 × 112 × 70) | 17.62 s | 24.72 s (0.71×) | 2.60 s (6.8×) | yes |
| ds002422-sub-02/wf_auto_00000 (64 × 64 × 24) | 2.02 s | 2.96 s (0.68×) | 0.38 s (5.3×) | yes |
| ds002422-sub-02/wf_auto_00001 (64 × 64 × 36) | 3.74 s | 5.03 s (0.74×) | 0.62 s (6.0×) | yes |
| ds001600-sub-1/wf_auto_00000 (64 × 64 × 44) | 3.01 s | 4.14 s (0.73×) | 0.50 s (6.0×) | yes |
| ds001600-sub-1/wf_auto_00001 (64 × 64 × 44) | 3.19 s | 4.28 s (0.75×) | 0.54 s (6.0×) | yes |
| ds001600-sub-1/wf_auto_00002 (64 × 64 × 44) | 3.02 s | 4.19 s (0.72×) | 0.52 s (5.8×) | yes |
| ds002603-sub-01/wf_auto_00000 (128 × 128 × 128) | 29.78 s | 48.12 s (0.62×) | 4.65 s (6.4×) | yes |

**At one thread the port is 1.3 to 1.6 times slower than scikit-learn.** The port applies the
collocation matrix on the fly from its per-axis B-spline weights (the summation order of
SciPy's stored CSR matrix, its explicit zeros included), which costs more per conjugate-gradient
iteration than scikit-learn's product with the stored matrix; that design is what keeps its
memory low (`tests/parity/test_memory_peak.py`: 70 MiB against sdcflows' 1,474 MiB on
ds005454's call, 40 against 367 on ds002422's). The memory rule (1.1 times the original's peak
plus 8 MiB) would allow a stored matrix (8 bytes per entry: about 450 MB for ds005454's call,
below sdcflows' peak), which would give the same bits faster at one thread: an option for the
next step or for the clean-room original. At 14 threads the port is 5 to 7 times faster
(deterministic: the same bits at any thread count).

## ApplyCoeffsField: ds005454's field on its BOLD series

ds005454-sub-16's coefficients (`bs_filter`), its field-map registration
(`fmapreg_wf/coregister`, `fmap2data_xfm`), readout time 0.0429303 s along `j-`, the Jacobian,
cubic interpolation, on the first 60 volumes of its BOLD series (160 × 160 × 96, int16): the whole
280-volume series would need about 50 GiB in sdcflows (3.7 GiB for 20 volumes, the memory test),
beyond this part's memory box. sdcflows' `num_threads` (its asyncio semaphore over the volumes)
and larmorx's `n_threads` alike.

| Call | threads | sdcflows | larmorx | identical |
|---|---|---|---|---|
| ds005454-sub-16, 60 volumes (160 × 160 × 96 × 60) | 1 | 33.54 s | 19.71 s (1.7×) | yes |
| | 14 | 12.34 s | 7.27 s (1.7×) | yes |

The unwarping is SciPy's `map_coordinates` (cubic, with its prefilter) per volume in both; the
port's replica (`larmorx-interp`, the resampler's) runs it with the shift, the Jacobian and the
writing in Rust, one volume at a time (829 MiB against sdcflows' 3,741 MiB on 20 volumes).
