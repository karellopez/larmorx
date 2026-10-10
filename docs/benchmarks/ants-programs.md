# Benchmarks: ANTs image programs (ImageMath, ThresholdImage, MultiplyImages)

larmorx against ANTs 2.6.5's own binaries on real images, as fMRIPrep uses these programs. Median of 5 runs after a warm-up.

- **Generated:** 2026-10-10 on Linux x86_64, with `python -m larmorx_validation bench ants-programs --repeats 5 --threads 1 4 0`
- **What is timed:** the whole command: reading the gzipped inputs, the operation, writing the output as uncompressed `.nii` (ITK compresses on one thread, which would dominate ANTs' time). ANTs runs as its own process; larmorx runs in-process through its console entry point. Both get the thread count through `ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS`.
- **Same result:** every larmorx output is compared with ANTs' (all values).
- **Machine load** before the run (1, 5, 15 min): 1.29, 1.31, 1.81; no other benchmark ran meanwhile.

## TruncateImageIntensity 0.01 0.999 256 (fMRIPrep's call), raw T1w ds000005 (160×192×192 int16)

| Tool | Threads | Median | Speed-up vs ANTs, 1 thread | Output vs ANTs |
|---|---|---|---|---|
| ANTs 2.6.5 | 1 | 388 ms | 1.0× |  |
| ANTs 2.6.5 | 12 | 175 ms | **2.2×** |  |
| larmorx | 1 | 234 ms | **1.7×** | bit-identical |
| larmorx | 4 | 96 ms | **4.0×** | bit-identical |
| larmorx | 12 | 83 ms | **4.7×** | bit-identical |

## TruncateImageIntensity 0.01 0.999 256, MNI152NLin2009cAsym 1 mm (193×229×193)

| Tool | Threads | Median | Speed-up vs ANTs, 1 thread | Output vs ANTs |
|---|---|---|---|---|
| ANTs 2.6.5 | 1 | 625 ms | 1.0× |  |
| ANTs 2.6.5 | 12 | 266 ms | **2.3×** |  |
| larmorx | 1 | 448 ms | **1.4×** | bit-identical |
| larmorx | 4 | 190 ms | **3.3×** | bit-identical |
| larmorx | 12 | 148 ms | **4.2×** | bit-identical |

## ThresholdImage 0.5 1, MNI brain probability map 1 mm

| Tool | Threads | Median | Speed-up vs ANTs, 1 thread | Output vs ANTs |
|---|---|---|---|---|
| ANTs 2.6.5 | 1 | 115 ms | 1.0× |  |
| ANTs 2.6.5 | 12 | 103 ms | **1.1×** |  |
| larmorx | 1 | 100 ms | **1.2×** | bit-identical |
| larmorx | 4 | 51 ms | **2.3×** | bit-identical |
| larmorx | 12 | 49 ms | **2.4×** | bit-identical |

## ThresholdImage Otsu 3 with a brain mask, fMRIPrep T1w

| Tool | Threads | Median | Speed-up vs ANTs, 1 thread | Output vs ANTs |
|---|---|---|---|---|
| ANTs 2.6.5 | 1 | 558 ms | 1.0× |  |
| ANTs 2.6.5 | 12 | 315 ms | **1.8×** |  |
| larmorx | 1 | 205 ms | **2.7×** | bit-identical |
| larmorx | 4 | 125 ms | **4.5×** | bit-identical |
| larmorx | 12 | 125 ms | **4.5×** | bit-identical |

## MultiplyImages T1w × brain mask (fMRIPrep derivatives)

| Tool | Threads | Median | Speed-up vs ANTs, 1 thread | Output vs ANTs |
|---|---|---|---|---|
| ANTs 2.6.5 | 1 | 157 ms | 1.0× |  |
| ANTs 2.6.5 | 12 | 158 ms | **1.0×** |  |
| larmorx | 1 | 128 ms | **1.2×** | bit-identical |
| larmorx | 4 | 77 ms | **2.0×** | bit-identical |
| larmorx | 12 | 62 ms | **2.5×** | bit-identical |

## ImageMath addtozero WM GM (probability maps)

| Tool | Threads | Median | Speed-up vs ANTs, 1 thread | Output vs ANTs |
|---|---|---|---|---|
| ANTs 2.6.5 | 1 | 204 ms | 1.0× |  |
| ANTs 2.6.5 | 12 | 200 ms | **1.0×** |  |
| larmorx | 1 | 75 ms | **2.7×** | bit-identical |
| larmorx | 4 | 44 ms | **4.6×** | bit-identical |
| larmorx | 12 | 40 ms | **5.1×** | bit-identical |

## ImageMath Normalize, fMRIPrep T1w

| Tool | Threads | Median | Speed-up vs ANTs, 1 thread | Output vs ANTs |
|---|---|---|---|---|
| ANTs 2.6.5 | 1 | 140 ms | 1.0× |  |
| ANTs 2.6.5 | 12 | 146 ms | **1.0×** |  |
| larmorx | 1 | 103 ms | **1.4×** | bit-identical |
| larmorx | 4 | 68 ms | **2.1×** | bit-identical |
| larmorx | 12 | 68 ms | **2.1×** | bit-identical |

## ImageMath RescaleImage 0 1, MNI 1 mm

| Tool | Threads | Median | Speed-up vs ANTs, 1 thread | Output vs ANTs |
|---|---|---|---|---|
| ANTs 2.6.5 | 1 | 185 ms | 1.0× |  |
| ANTs 2.6.5 | 12 | 167 ms | **1.1×** |  |
| larmorx | 1 | 145 ms | **1.3×** | bit-identical |
| larmorx | 4 | 102 ms | **1.8×** | bit-identical |
| larmorx | 12 | 97 ms | **1.9×** | bit-identical |

## ImageMath 4 m 0.5, fMRIPrep BOLD series (4D, 28 MB gzipped)

| Tool | Threads | Median | Speed-up vs ANTs, 1 thread | Output vs ANTs |
|---|---|---|---|---|
| ANTs 2.6.5 | 1 | 447 ms | 1.0× |  |
| ANTs 2.6.5 | 12 | 438 ms | **1.0×** |  |
| larmorx | 1 | 392 ms | **1.1×** | bit-identical |
| larmorx | 4 | 219 ms | **2.0×** | bit-identical |
| larmorx | 12 | 225 ms | **2.0×** | bit-identical |

## Notes

- **Where the time goes.** These programs do little arithmetic per voxel, so reading the gzipped inputs dominates: gzip decompression is sequential (zlib-rs in larmorx, zlib in ITK). For the 28 MB BOLD series it is about 200 ms of larmorx's 390 ms on one thread. The operations themselves run in parallel, and so does larmorx's conversion of the voxels to the program's pixel type.
- **ANTs' threads** help where ITK filters do the work (the label statistics behind `TruncateImageIntensity`, the Otsu labeller); ImageMath's own voxel loops are single-threaded.
- **Determinism.** larmorx's results do not depend on the thread count; the order-dependent steps of ANTs (running sums, the histogram range of `TruncateImageIntensity`, the carried value of `/`) run in image order.

## Environment

| Component | Version |
|---|---|
| larmorx | 0.0.1 (2960b32a61f4-dirty) |
| larmorx-testdata | ea0c2b73b3bb |
| Python | 3.12.10 |
| Platform | Linux x86_64 (Linux-6.8.0-124-generic-x86_64-with-glibc2.35) |
| CPU | Intel(R) Core(TM) i7-8750H CPU @ 2.20GHz, 12 logical CPUs |
| nibabel | 5.4.2 |
| numpy | 2.3.5 |
