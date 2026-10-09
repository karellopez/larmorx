# Benchmarks: antsApplyTransforms

larmorx against antsApplyTransforms (ANTs 2.6.5 on ITK 5.4.5, through ANTsPy) on real fMRIPrep resampling jobs (ds000005, sub-01), with 1, 4, 12 threads. Median of 3 runs after a warm-up.

- **Generated:** 2026-10-09 on Linux x86_64, with `python -m larmorx_validation bench ants-apply-transforms --repeats 3 --threads 1 4 0`
- **What is timed:** the whole command: reading the image and the transforms, resampling, writing the output (uncompressed `.nii`, float64 unless `-u`). Both run in-process with the same arguments; ITK's thread count is set per worker process. The Python API column skips the file output.
- **Same result:** every larmorx output below is compared with ANTs' (bit-identical, or the largest relative difference).

## boldref → T1w 1 mm, Linear

Output 160×192×192.

| Threads | ANTs | larmorx CLI | Speed-up | larmorx Python | Output vs ANTs |
|---|---|---|---|---|---|
| 1 | 289 ms | 180 ms | **1.6×** | 144 ms | bit-identical |
| 4 | 189 ms | 79 ms | **2.4×** | 42 ms | bit-identical |
| 12 | 183 ms | 66 ms | **2.8×** | 27 ms | bit-identical |

## boldref → T1w 1 mm, Lanczos

Output 160×192×192.

| Threads | ANTs | larmorx CLI | Speed-up | larmorx Python | Output vs ANTs |
|---|---|---|---|---|---|
| 1 | 11.62 s | 1.75 s | **6.6×** | 1.62 s | ≤ 4e-16 rel. |
| 4 | 3.18 s | 567 ms | **5.6×** | 528 ms | ≤ 4e-16 rel. |
| 12 | 1.92 s | 423 ms | **4.5×** | 398 ms | ≤ 4e-16 rel. |

## T1w → MNI 2 mm (.h5 affine + warp), Linear

Output 97×115×97.

| Threads | ANTs | larmorx CLI | Speed-up | larmorx Python | Output vs ANTs |
|---|---|---|---|---|---|
| 1 | 850 ms | 743 ms | **1.1×** | 732 ms | bit-identical |
| 4 | 760 ms | 646 ms | **1.2×** | 620 ms | bit-identical |
| 12 | 740 ms | 619 ms | **1.2×** | 601 ms | bit-identical |

## T1w → MNI 2 mm (.h5 affine + warp), Lanczos

Output 97×115×97.

| Threads | ANTs | larmorx CLI | Speed-up | larmorx Python | Output vs ANTs |
|---|---|---|---|---|---|
| 1 | 4.25 s | 1.36 s | **3.1×** | 1.38 s | ≤ 3e-16 rel. |
| 4 | 1.75 s | 824 ms | **2.1×** | 758 ms | ≤ 3e-16 rel. |
| 12 | 1.26 s | 783 ms | **1.6×** | 747 ms | ≤ 3e-16 rel. |

## aseg → boldref, GenericLabel

Output 64×64×34.

| Threads | ANTs | larmorx CLI | Speed-up | larmorx Python | Output vs ANTs |
|---|---|---|---|---|---|
| 1 | 288 ms | 24 ms | **12.0×** | 26 ms | bit-identical |
| 4 | 203 ms | 10 ms | **19.7×** | 11 ms | bit-identical |
| 12 | 199 ms | 9 ms | **21.4×** | 9 ms | bit-identical |

## BOLD 240 volumes → T1w grid, Lanczos (-e 3)

Output 49×52×37×240.

| Threads | ANTs | larmorx CLI | Speed-up | larmorx Python | Output vs ANTs |
|---|---|---|---|---|---|
| 1 | 80.23 s | 14.80 s | **5.4×** | 12.42 s | ≤ 6e-16 rel. |
| 4 | 21.88 s | 4.49 s | **4.9×** | 3.94 s | ≤ 6e-16 rel. |
| 12 | 13.65 s | 3.49 s | **3.9×** | 3.15 s | ≤ 6e-16 rel. |

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

## Environment

| Component | Version |
|---|---|
| larmorx | 0.0.1 (7466702e9d4c-dirty: the uncommitted changes were to `docs/findings/` only) |
| larmorx-testdata | 535104904d50 |
| Python | 3.12.10 |
| Platform | Linux x86_64 (Linux-6.8.0-124-generic-x86_64-with-glibc2.35) |
| CPU | Intel(R) Core(TM) i7-8750H CPU @ 2.20GHz, 12 logical CPUs |
| numpy | 2.3.5 |
| antspyx | 0.6.3 |
| nibabel | 5.4.2 |
| h5py | 3.16.0 |
