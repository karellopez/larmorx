# Benchmarks: ANTs Gaussian filters (ImageMath Laplacian, G, Grad; SmoothImage; ResampleImageBySpacing)

larmorx against ANTs 2.6.5's own binaries on real images, as fMRIPrep uses these programs. Median of 5 runs after a warm-up.

- **Generated:** 2026-10-10 on Linux x86_64, with `python -m larmorx_validation bench ants-gaussian --repeats 5 --threads 1 4 0`
- **What is timed:** the whole command: reading the gzipped inputs, the operation, writing the output as uncompressed `.nii` (ITK compresses on one thread, which would dominate ANTs' time). ANTs runs as its own process; larmorx runs in-process through its console entry point. Both get the thread count through `ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS`.
- **Same result:** every larmorx output is compared with ANTs' (all values).
- **Machine load** before the run (1, 5, 15 min): 0.99, 2.80, 3.12; no other benchmark ran meanwhile.

## ImageMath Laplacian 1.5 1 (fMRIPrep's call), raw T1w ds000005 (160×192×192 int16)

| Tool | Threads | Median | Speed-up vs ANTs, 1 thread | Output vs ANTs |
|---|---|---|---|---|
| ANTs 2.6.5 | 1 | 1.23 s | 1.0× |  |
| ANTs 2.6.5 | 12 | 269 ms | **4.6×** |  |
| larmorx | 1 | 350 ms | **3.5×** | bit-identical |
| larmorx | 4 | 159 ms | **7.8×** | bit-identical |
| larmorx | 12 | 131 ms | **9.4×** | bit-identical |

## ImageMath Laplacian 1.5 1 (fMRIPrep's call), boldref ds000005 (64×64×34)

| Tool | Threads | Median | Speed-up vs ANTs, 1 thread | Output vs ANTs |
|---|---|---|---|---|
| ANTs 2.6.5 | 1 | 48 ms | 1.0× |  |
| ANTs 2.6.5 | 12 | 29 ms | **1.6×** |  |
| larmorx | 1 | 7 ms | **6.5×** | bit-identical |
| larmorx | 4 | 4 ms | **13.2×** | bit-identical |
| larmorx | 12 | 3 ms | **13.7×** | bit-identical |

## ImageMath Laplacian 1.5 1, MNI152NLin2009cAsym 1 mm (193×229×193)

| Tool | Threads | Median | Speed-up vs ANTs, 1 thread | Output vs ANTs |
|---|---|---|---|---|
| ANTs 2.6.5 | 1 | 1.61 s | 1.0× |  |
| ANTs 2.6.5 | 12 | 384 ms | **4.2×** |  |
| larmorx | 1 | 510 ms | **3.2×** | bit-identical |
| larmorx | 4 | 228 ms | **7.1×** | bit-identical |
| larmorx | 12 | 212 ms | **7.6×** | bit-identical |

## ImageMath Grad 1, raw T1w

| Tool | Threads | Median | Speed-up vs ANTs, 1 thread | Output vs ANTs |
|---|---|---|---|---|
| ANTs 2.6.5 | 1 | 1.20 s | 1.0× |  |
| ANTs 2.6.5 | 12 | 266 ms | **4.5×** |  |
| larmorx | 1 | 309 ms | **3.9×** | bit-identical |
| larmorx | 4 | 146 ms | **8.2×** | bit-identical |
| larmorx | 12 | 126 ms | **9.6×** | bit-identical |

## ImageMath G 2 (discrete Gaussian, sigma 2 mm), raw T1w

| Tool | Threads | Median | Speed-up vs ANTs, 1 thread | Output vs ANTs |
|---|---|---|---|---|
| ANTs 2.6.5 | 1 | 738 ms | 1.0× |  |
| ANTs 2.6.5 | 12 | 223 ms | **3.3×** |  |
| larmorx | 1 | 125 ms | **5.9×** | bit-identical |
| larmorx | 4 | 67 ms | **11.1×** | bit-identical |
| larmorx | 12 | 67 ms | **11.1×** | bit-identical |

## SmoothImage 3 sigma 1 voxel, raw T1w

| Tool | Threads | Median | Speed-up vs ANTs, 1 thread | Output vs ANTs |
|---|---|---|---|---|
| ANTs 2.6.5 | 1 | 450 ms | 1.0× |  |
| ANTs 2.6.5 | 12 | 139 ms | **3.2×** |  |
| larmorx | 1 | 117 ms | **3.8×** | bit-identical |
| larmorx | 4 | 67 ms | **6.8×** | bit-identical |
| larmorx | 12 | 64 ms | **7.1×** | bit-identical |

## SmoothImage 3 median radius 1, raw T1w

| Tool | Threads | Median | Speed-up vs ANTs, 1 thread | Output vs ANTs |
|---|---|---|---|---|
| ANTs 2.6.5 | 1 | 1.92 s | 1.0× |  |
| ANTs 2.6.5 | 12 | 365 ms | **5.3×** |  |
| larmorx | 1 | 1.27 s | **1.5×** | bit-identical |
| larmorx | 4 | 373 ms | **5.1×** | bit-identical |
| larmorx | 12 | 256 ms | **7.5×** | bit-identical |

## SmoothImage 4 sigma 1 voxel, fMRIPrep BOLD series (4D, 28 MB gzipped)

| Tool | Threads | Median | Speed-up vs ANTs, 1 thread | Output vs ANTs |
|---|---|---|---|---|
| ANTs 2.6.5 | 1 | 1.82 s | 1.0× |  |
| ANTs 2.6.5 | 12 | 512 ms | **3.5×** |  |
| larmorx | 1 | 636 ms | **2.9×** | bit-identical |
| larmorx | 4 | 369 ms | **4.9×** | bit-identical |
| larmorx | 12 | 336 ms | **5.4×** | bit-identical |

## ResampleImageBySpacing 3 to 2 mm (smoothed, linear), raw T1w

| Tool | Threads | Median | Speed-up vs ANTs, 1 thread | Output vs ANTs |
|---|---|---|---|---|
| ANTs 2.6.5 | 1 | 490 ms | 1.0× |  |
| ANTs 2.6.5 | 12 | 129 ms | **3.8×** |  |
| larmorx | 1 | 148 ms | **3.3×** | bit-identical |
| larmorx | 4 | 87 ms | **5.7×** | bit-identical |
| larmorx | 12 | 88 ms | **5.6×** | bit-identical |

## ResampleImageBySpacing 3 to 3 mm, MNI 1 mm

| Tool | Threads | Median | Speed-up vs ANTs, 1 thread | Output vs ANTs |
|---|---|---|---|---|
| ANTs 2.6.5 | 1 | 613 ms | 1.0× |  |
| ANTs 2.6.5 | 12 | 194 ms | **3.2×** |  |
| larmorx | 1 | 243 ms | **2.5×** | bit-identical |
| larmorx | 4 | 156 ms | **3.9×** | bit-identical |
| larmorx | 12 | 148 ms | **4.1×** | bit-identical |

## Notes

- **Where the time goes.** Reading the gzipped input is sequential in both programs (zlib-rs in larmorx, zlib in ITK): about 50 ms of larmorx's time for the raw T1w, which is why 4 and 12 threads differ little for the faster filters. The recursive filters cost a few multiply-adds per voxel and pass; the Laplacian and the gradient magnitude make nine passes over a 3D image, `SmoothImage` three.
- **How larmorx runs the passes.** Every pass filters 32 neighbouring lines together, laid out so that the 32 recursions advance side by side on contiguous values (lines along the first axis are transposed into that layout). Each line's arithmetic is exactly ITK's, so the result does not depend on the thread count or on the grouping. The resampler computes the part of the linear interpolation that is constant along an output row once per row when the row runs along an input axis (axis-aligned images).
- **Threads.** ITK splits every pass into regions per thread, so ANTs gains 3–5× from 12 threads. larmorx gains less from many threads on these images: reading the input stays sequential, and a pass over a 6-million-voxel image is short.

## Environment

| Component | Version |
|---|---|
| larmorx | 0.0.1 (54d3282e3bf5-dirty) |
| larmorx-testdata | 28167dea9be5 |
| Python | 3.12.10 |
| Platform | Linux x86_64 (Linux-6.8.0-124-generic-x86_64-with-glibc2.35) |
| CPU | Intel(R) Core(TM) i7-8750H CPU @ 2.20GHz, 12 logical CPUs |
| numpy | 2.3.5 |
| nibabel | 5.4.2 |
