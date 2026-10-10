# Benchmarks: one-shot BOLD resampling

larmorx's `lx.transforms.resample_series` against fMRIPrep's one-shot resampler (`fmriprep/interfaces/resampling.py`, the code `ResampleSeries` runs) on real BOLD runs, on 1, 4, 12 threads. Median of 3 runs after a warm-up (fMRIPrep's slowest jobs once, without a warm-up).

- **Generated:** 2026-10-10 on Linux x86_64, with `python -m larmorx_validation bench resample-series --repeats 3 --threads 1 4 0`; load average before the run: 3.24 3.24 2.55.
- **What is timed:** the resampling as `ResampleSeries._run_interface` does it, without the file I/O: loading the transform files, reorienting the source (distortion correction), mapping the target grid, and resampling every volume (head motion, voxel-shift map, cubic B-spline interpolation in `grid-constant` mode, Jacobian). Both start from the same in-memory run and field map.
- **Inputs:** seeded head motion (a random walk of about 1° and 1 mm) and, where marked, a seeded smooth field map (±120 Hz) on the target grid, PE `j`, readout 31.2 ms; fMRIPrep 21.0.1's own boldref→T1w affine and T1w→MNI warp for ds000005.
- **Same result:** every larmorx output is compared with fMRIPrep's, value by value.

## ds000005 run 1 → boldref (native): 64×64×34, 240 volumes; motion + field map

| Tool | Threads | Median | Speed-up vs fMRIPrep (same threads) | vs fMRIPrep on 1 thread | Output vs fMRIPrep |
|---|---|---|---|---|---|
| fMRIPrep | 1 | 21.15 s | 1.0× | 1.0× |  |
| larmorx | 1 | 4.57 s | **4.6×** | 4.6× | bit-identical |
| fMRIPrep | 4 | 7.66 s | 1.0× | 2.8× |  |
| larmorx | 4 | 1.41 s | **5.4×** | 15.0× | bit-identical |
| fMRIPrep | 12 | 5.25 s | 1.0× | 4.0× |  |
| larmorx | 12 | 932 ms | **5.6×** | 22.7× | bit-identical |

## ds000005 run 1 → T1w grid: motion + boldref→T1w affine + field map

| Tool | Threads | Median | Speed-up vs fMRIPrep (same threads) | vs fMRIPrep on 1 thread | Output vs fMRIPrep |
|---|---|---|---|---|---|
| fMRIPrep | 1 | 14.39 s | 1.0× | 1.0× |  |
| larmorx | 1 | 3.52 s | **4.1×** | 4.1× | bit-identical |
| fMRIPrep | 4 | 4.73 s | 1.0× | 3.0× |  |
| larmorx | 4 | 1.08 s | **4.4×** | 13.3× | bit-identical |
| fMRIPrep | 12 | 3.50 s | 1.0× | 4.1× |  |
| larmorx | 12 | 732 ms | **4.8×** | 19.7× | bit-identical |

## ds000005 run 1 → MNI152NLin2009cAsym res-2 (97×115×97): motion + affine + .h5 warp

| Tool | Threads | Median | Speed-up vs fMRIPrep (same threads) | vs fMRIPrep on 1 thread | Output vs fMRIPrep |
|---|---|---|---|---|---|
| fMRIPrep | 1 | 155.01 s | 1.0× | 1.0× |  |
| larmorx | 1 | 29.90 s | **5.2×** | 5.2× | bit-identical |
| fMRIPrep | 4 | 60.80 s | 1.0× | 2.5× |  |
| larmorx | 4 | 9.36 s | **6.5×** | 16.6× | bit-identical |
| fMRIPrep | 12 | 42.53 s | 1.0× | 3.6× |  |
| larmorx | 12 | 6.88 s | **6.2×** | 22.5× | bit-identical |

## ds006010 sub-206 → native: 96×96×69, 113 volumes (uint16); motion + field map

| Tool | Threads | Median | Speed-up vs fMRIPrep (same threads) | vs fMRIPrep on 1 thread | Output vs fMRIPrep |
|---|---|---|---|---|---|
| fMRIPrep | 1 | 46.20 s | 1.0× | 1.0× |  |
| larmorx | 1 | 9.15 s | **5.1×** | 5.1× | bit-identical |
| fMRIPrep | 4 | 18.79 s | 1.0× | 2.5× |  |
| larmorx | 4 | 2.66 s | **7.1×** | 17.4× | bit-identical |
| fMRIPrep | 12 | 12.96 s | 1.0× | 3.6× |  |
| larmorx | 12 | 1.88 s | **6.9×** | 24.6× | bit-identical |

## ds005454 sub-16 rest → native: 160×160×96, 280 volumes (multiband 4); motion + field map

| Tool | Threads | Median | Speed-up vs fMRIPrep (same threads) | vs fMRIPrep on 1 thread | Output vs fMRIPrep |
|---|---|---|---|---|---|
| fMRIPrep | 1 | 409.25 s | 1.0× | 1.0× |  |
| larmorx | 1 | 86.44 s | **4.7×** | 4.7× | bit-identical |
| fMRIPrep | 4 | 162.56 s | 1.0× | 2.5× |  |
| larmorx | 4 | 26.27 s | **6.2×** | 15.6× | bit-identical |
| fMRIPrep | 12 | 108.71 s | 1.0× | 3.8× |  |
| larmorx | 12 | 16.68 s | **6.5×** | 24.5× | bit-identical |

## Notes

- **Where larmorx saves time.** fMRIPrep maps the target grid once per run, but then, per volume, copies the coordinates, applies the head-motion affine to all of them, adds the voxel-shift map, prefilters the volume (`map_coordinates` builds a float64 spline filter each call) and recomputes the Jacobian; each step allocates full-size arrays. larmorx computes the voxel-shift map and Jacobian once per run and moves each voxel's coordinates in registers inside the interpolation loop; the prefilter runs once per volume, line by line over contiguous rows.
- **Exactness costs speed.** Each output value keeps SciPy's 64-tap summation order and numpy's fused multiply-adds for the head-motion product (a call into the C library's `fma` on x86-64 builds without FMA instructions). A separable evaluation would be several times faster but changes the last bits.
- **Threads.** fMRIPrep resamples up to `nthreads` volumes at once (SciPy releases the GIL). larmorx gives whole volumes to threads when there are more volumes than threads, so at most `n_threads` prefiltered volumes are in memory, and otherwise splits each volume's voxels. The machine has 6 cores and 12 hardware threads.

## Environment

| Component | Version |
|---|---|
| larmorx | 0.0.1 (46225a79652d) |
| larmorx-testdata | 28167dea9be5 |
| Python | 3.12.10 |
| Platform | Linux x86_64 (Linux-6.8.0-124-generic-x86_64-with-glibc2.35) |
| CPU | Intel(R) Core(TM) i7-8750H CPU @ 2.20GHz, 12 logical CPUs |
| numpy | 2.3.5 |
| fmriprep | 26.0.0.dev1+g21a490fb8 |
| nitransforms | 25.1.0 |
| scipy | 1.15.3 |
| nibabel | 5.4.2 |
