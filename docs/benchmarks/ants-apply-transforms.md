# Benchmarks: antsApplyTransforms

larmorx against antsApplyTransforms (ANTs 2.6.5 on ITK 5.4.5, through ANTsPy) on real fMRIPrep resampling jobs (ds000005, sub-01), with 1, 4, 12 threads. Median of 3 runs after a warm-up.

- **Generated:** 2026-10-10 on Linux x86_64, with `python -m larmorx_validation bench ants-apply-transforms --repeats 3 --threads 1 4 0`
- **What is timed:** the whole command: reading the image and the transforms, resampling, writing the output (uncompressed `.nii`, float64 unless `-u`). Both run in-process with the same arguments; ITK's thread count is set per worker process. The Python API column skips the file output.
- **Same result:** every larmorx output below is compared with ANTs' (bit-identical, or the largest relative difference).

## boldref → T1w 1 mm, Linear

Output 160×192×192.

| Threads | ANTs | larmorx CLI | Speed-up | larmorx Python | Output vs ANTs |
|---|---|---|---|---|---|
| 1 | 258 ms | 137 ms | **1.9×** | 95 ms | bit-identical |
| 4 | 187 ms | 75 ms | **2.5×** | 33 ms | bit-identical |
| 12 | 180 ms | 61 ms | **3.0×** | 21 ms | bit-identical |

## boldref → T1w 1 mm, Lanczos

Output 160×192×192.

| Threads | ANTs | larmorx CLI | Speed-up | larmorx Python | Output vs ANTs |
|---|---|---|---|---|---|
| 1 | 11.55 s | 2.64 s | **4.4×** | 2.47 s | ≤ 4e-16 rel. |
| 4 | 3.31 s | 742 ms | **4.5×** | 689 ms | ≤ 4e-16 rel. |
| 12 | 1.87 s | 536 ms | **3.5×** | 490 ms | ≤ 4e-16 rel. |

## T1w → MNI 2 mm (.h5 affine + warp), Linear

Output 97×115×97.

| Threads | ANTs | larmorx CLI | Speed-up | larmorx Python | Output vs ANTs |
|---|---|---|---|---|---|
| 1 | 832 ms | 683 ms | **1.2×** | 678 ms | bit-identical |
| 4 | 744 ms | 626 ms | **1.2×** | 619 ms | bit-identical |
| 12 | 730 ms | 616 ms | **1.2×** | 614 ms | bit-identical |

## T1w → MNI 2 mm (.h5 affine + warp), Lanczos

Output 97×115×97.

| Threads | ANTs | larmorx CLI | Speed-up | larmorx Python | Output vs ANTs |
|---|---|---|---|---|---|
| 1 | 4.21 s | 1.60 s | **2.6×** | 1.62 s | ≤ 2e-16 rel. |
| 4 | 1.73 s | 875 ms | **2.0×** | 881 ms | ≤ 2e-16 rel. |
| 12 | 1.25 s | 791 ms | **1.6×** | 768 ms | ≤ 2e-16 rel. |

## aseg → boldref, GenericLabel

Output 64×64×34.

| Threads | ANTs | larmorx CLI | Speed-up | larmorx Python | Output vs ANTs |
|---|---|---|---|---|---|
| 1 | 290 ms | 20 ms | **14.6×** | 19 ms | bit-identical |
| 4 | 203 ms | 11 ms | **17.8×** | 10 ms | bit-identical |
| 12 | 196 ms | 10 ms | **20.2×** | 8 ms | bit-identical |

## BOLD 240 volumes → T1w grid, Lanczos (-e 3)

Output 49×52×37×240.

| Threads | ANTs | larmorx CLI | Speed-up | larmorx Python | Output vs ANTs |
|---|---|---|---|---|---|
| 1 | 80.16 s | 19.71 s | **4.1×** | 20.76 s | ≤ 5e-16 rel. |
| 4 | 22.18 s | 5.73 s | **3.9×** | 5.82 s | ≤ 5e-16 rel. |
| 12 | 13.19 s | 4.21 s | **3.1×** | 4.06 s | ≤ 5e-16 rel. |

## Notes

- **Where the time goes.**
  - Linear interpolation onto a 1 mm grid is dominated by reading the gzipped input and
    writing the 47 MB output. ANTs hardly speeds up with more threads there.
  - The `.h5` jobs are dominated by reading fMRIPrep's 90 MB gzip-compressed warp: h5py
    for larmorx, ITK's HDF5 reader for ANTs, both decompressing in one thread.
  - A pure-Rust reader for ITK's `.h5` layout with parallel decompression is planned
    (decision log, 2026-10-09).
- **Single-thread speed** comes from computing each output row's indices once (ITK's
  scan-line method) and reusing them for every volume of a series. It also comes from
  evaluating GenericLabel only for the labels around each point; ITK evaluates every label
  of the image.
- **Equal results.** Where outputs are not bit-identical, the windowed-sinc weights go through
  the platform's `sin`/`cos`. See `docs/findings/platform-math.md`.
- **Changes since the 2026-10-09 report.**
  - **Linear, nearest-neighbour and label jobs are faster:** the interpolators now find the
    base index by truncation instead of `floor`, with the same results (A2 part 2).
  - **Lanczos jobs are about 30 % slower than before** (T1w → MNI: 4.4× instead of 6.6×
    faster than ANTs on one thread; the 240-volume BOLD run: 4.1× instead of 5.4×). The
    windowed-sinc weights now use larmorx's correctly rounded `sin`/`cos` (decided
    2026-10-09, CLAUDE.md rule 5), which cost more per call than glibc's. That buys the same
    bits on every platform; the old numbers used the `libm` crate.

## Environment

| Component | Version |
|---|---|
| larmorx | 0.0.1 (c1b28cee0967) |
| larmorx-testdata | f32b1d60627c |
| Python | 3.12.10 |
| Platform | Linux x86_64 (Linux-6.8.0-124-generic-x86_64-with-glibc2.35) |
| CPU | Intel(R) Core(TM) i7-8750H CPU @ 2.20GHz, 12 logical CPUs |
| numpy | 2.3.5 |
| antspyx | 0.6.3 |
| nibabel | 5.4.2 |
| h5py | 3.16.0 |
