# Benchmarks: N4BiasFieldCorrection

larmorx against ANTs 2.6.5's own binaries on real images, as fMRIPrep uses these programs. Median of 3 runs after a warm-up.

- **Generated:** 2026-10-11 on Linux x86_64, with `python -m larmorx_validation bench ants-n4 --repeats 3 --threads 1 4 0`
- **What is timed:** the whole command: reading the gzipped inputs, the operation, writing the output as uncompressed `.nii` (ITK compresses on one thread, which would dominate ANTs' time). ANTs runs as its own process; larmorx runs in-process through its console entry point. Both get the thread count through `ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS`.
- **Same result:** every larmorx output is compared with ANTs' at one thread (all values).
- **Machine load** before the run (1, 5, 15 min): 0.50, 0.67, 1.63; no other benchmark ran meanwhile.

## fMRIPrep's BOLD reference: -b [200] -r -w premask, boldref ds000005 (64×64×34)

| Tool | Threads | Median | Speed-up vs ANTs, 1 thread | Output vs ANTs |
|---|---|---|---|---|
| ANTs 2.6.5 | 1 | 1.00 s | 1.0× |  |
| ANTs 2.6.5 | 12 | 556 ms | **1.8×** |  |
| larmorx | 1 | 135 ms | **7.4×** | 76748 values differ |
| larmorx | 4 | 88 ms | **11.4×** | 76748 values differ |
| larmorx | 12 | 99 ms | **10.1×** | 76748 values differ |

## fMRIPrep's BOLD reference, MNI152NLin2009cAsym boldref res-02 (97×115×97)

| Tool | Threads | Median | Speed-up vs ANTs, 1 thread | Output vs ANTs |
|---|---|---|---|---|
| ANTs 2.6.5 | 1 | 3.89 s | 1.0× |  |
| ANTs 2.6.5 | 12 | 1.24 s | **3.1×** |  |
| larmorx | 1 | 534 ms | **7.3×** | 468307 values differ |
| larmorx | 4 | 257 ms | **15.1×** | 468307 values differ |
| larmorx | 12 | 278 ms | **14.0×** | 468307 values differ |

## sMRIPrep before the template: ANTs' defaults, raw T1w ds000005 (160×192×192)

| Tool | Threads | Median | Speed-up vs ANTs, 1 thread | Output vs ANTs |
|---|---|---|---|---|
| ANTs 2.6.5 | 1 | 55.18 s | 1.0× |  |
| ANTs 2.6.5 | 12 | 10.73 s | **5.1×** |  |
| larmorx | 1 | 8.69 s | **6.4×** | bit-identical |
| larmorx | 4 | 3.18 s | **17.3×** | bit-identical |
| larmorx | 12 | 2.40 s | **23.0×** | bit-identical |

## sMRIPrep brain extraction, first N4: -b [200] -c [50x50x50x50,1e-7] -s 4, truncated T1w

| Tool | Threads | Median | Speed-up vs ANTs, 1 thread | Output vs ANTs |
|---|---|---|---|---|
| ANTs 2.6.5 | 1 | 57.92 s | 1.0× |  |
| ANTs 2.6.5 | 12 | 11.33 s | **5.1×** |  |
| larmorx | 1 | 8.59 s | **6.7×** | bit-identical |
| larmorx | 4 | 3.57 s | **16.2×** | bit-identical |
| larmorx | 12 | 2.69 s | **21.5×** | bit-identical |

## sMRIPrep brain extraction, final N4: -b [200] -c [50x50x50x50x50,1e-7] -s 4 -r -w WM, raw T1w

| Tool | Threads | Median | Speed-up vs ANTs, 1 thread | Output vs ANTs |
|---|---|---|---|---|
| ANTs 2.6.5 | 1 | 12.38 s | 1.0× |  |
| ANTs 2.6.5 | 12 | 4.33 s | **2.9×** |  |
| larmorx | 1 | 1.79 s | **6.9×** | 2037870 values differ |
| larmorx | 4 | 817 ms | **15.2×** | 2037870 values differ |
| larmorx | 12 | 690 ms | **17.9×** | 2037870 values differ |

## sdcflows' magnitude: -c [50x50x50x50x50,1e-7] -s 4, ds003012 magnitude1

| Tool | Threads | Median | Speed-up vs ANTs, 1 thread | Output vs ANTs |
|---|---|---|---|---|
| ANTs 2.6.5 | 1 | 1.96 s | 1.0× |  |
| ANTs 2.6.5 | 12 | 907 ms | **2.2×** |  |
| larmorx | 1 | 271 ms | **7.2×** | bit-identical |
| larmorx | 4 | 147 ms | **13.3×** | bit-identical |
| larmorx | 12 | 160 ms | **12.2×** | bit-identical |

## Notes

- **Same result.** ANTs' own result changes with its thread count (ITK's B-spline fit sums per-thread lattices); larmorx gives ANTs' one-thread result on any number of threads. Where the table says values differ from ANTs at one thread, the cause is glibc's `expf`/`logf`/`exp`, which are not always correctly rounded: the same ANTs binary with those functions replaced by CORE-MATH's through `LD_PRELOAD` gives larmorx's bits ([validation record](../validation/ants-n4.md)).
- **Where the time goes.** Each iteration sharpens the histogram (seven FFTs of 512 points), fits a B-spline to the residual at every voxel of the shrunk image, and evaluates the field on the shrunk grid; the default runs 200 iterations. ITK builds a point set and recomputes every point's span index and kernel weights at each fit. The points do not move within a level, so larmorx computes the weights once per level and reuses them in every iteration (the same arithmetic).
- **Threads.** larmorx splits the fit by blocks of control points, the evaluation by planes and the histogram mapping by voxels; the histogram and the FFTs are sequential. Reading the gzipped input and the full-resolution field and division at the end are a fixed cost.

- **This run.** The tree was dirty with documentation edits only; the code is that of commit 1f345c6. The machine was otherwise idle (load 0.50 before the run).

## Environment

| Component | Version |
|---|---|
| larmorx | 0.0.1 (1f345c61c340-dirty) |
| larmorx-testdata | 00b862132e9c |
| Python | 3.12.10 |
| Platform | Linux x86_64 (Linux-6.8.0-124-generic-x86_64-with-glibc2.35) |
| CPU | Intel(R) Core(TM) i7-8750H CPU @ 2.20GHz, 12 logical CPUs |
| numpy | 2.3.5 |
| nibabel | 5.4.2 |
