# Benchmarks: ANTs Gaussian filters and resampling (ImageMath Laplacian, G, Grad; SmoothImage; ResampleImageBySpacing; ResampleImage)

larmorx against ANTs 2.6.5's own binaries on real images, as fMRIPrep uses these programs. Median of 5 runs after a warm-up.

- **Generated:** 2026-10-10 on Linux x86_64, with `python -m larmorx_validation bench ants-gaussian --repeats 5 --threads 1 4 0`
- **What is timed:** the whole command: reading the gzipped inputs, the operation, writing the output as uncompressed `.nii` (ITK compresses on one thread, which would dominate ANTs' time). ANTs runs as its own process; larmorx runs in-process through its console entry point. Both get the thread count through `ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS`.
- **Same result:** every larmorx output is compared with ANTs' (all values).
- **Machine load** before the run (1, 5, 15 min): 0.92, 2.26, 2.69; no other benchmark ran meanwhile.

## ImageMath Laplacian 1.5 1 (fMRIPrep's call), raw T1w ds000005 (160×192×192 int16)

| Tool | Threads | Median | Speed-up vs ANTs, 1 thread | Output vs ANTs |
|---|---|---|---|---|
| ANTs 2.6.5 | 1 | 1.23 s | 1.0× |  |
| ANTs 2.6.5 | 12 | 277 ms | **4.5×** |  |
| larmorx | 1 | 351 ms | **3.5×** | bit-identical |
| larmorx | 4 | 146 ms | **8.5×** | bit-identical |
| larmorx | 12 | 128 ms | **9.6×** | bit-identical |

## ImageMath Laplacian 1.5 1 (fMRIPrep's call), boldref ds000005 (64×64×34)

| Tool | Threads | Median | Speed-up vs ANTs, 1 thread | Output vs ANTs |
|---|---|---|---|---|
| ANTs 2.6.5 | 1 | 52 ms | 1.0× |  |
| ANTs 2.6.5 | 12 | 31 ms | **1.7×** |  |
| larmorx | 1 | 7 ms | **7.1×** | bit-identical |
| larmorx | 4 | 4 ms | **13.2×** | bit-identical |
| larmorx | 12 | 3 ms | **15.0×** | bit-identical |

## ImageMath Laplacian 1.5 1, MNI152NLin2009cAsym 1 mm (193×229×193)

| Tool | Threads | Median | Speed-up vs ANTs, 1 thread | Output vs ANTs |
|---|---|---|---|---|
| ANTs 2.6.5 | 1 | 1.64 s | 1.0× |  |
| ANTs 2.6.5 | 12 | 383 ms | **4.3×** |  |
| larmorx | 1 | 523 ms | **3.1×** | bit-identical |
| larmorx | 4 | 237 ms | **6.9×** | bit-identical |
| larmorx | 12 | 218 ms | **7.5×** | bit-identical |

## ImageMath Grad 1, raw T1w

| Tool | Threads | Median | Speed-up vs ANTs, 1 thread | Output vs ANTs |
|---|---|---|---|---|
| ANTs 2.6.5 | 1 | 1.23 s | 1.0× |  |
| ANTs 2.6.5 | 12 | 253 ms | **4.8×** |  |
| larmorx | 1 | 306 ms | **4.0×** | bit-identical |
| larmorx | 4 | 147 ms | **8.3×** | bit-identical |
| larmorx | 12 | 130 ms | **9.4×** | bit-identical |

## ImageMath G 2 (discrete Gaussian, sigma 2 mm), raw T1w

| Tool | Threads | Median | Speed-up vs ANTs, 1 thread | Output vs ANTs |
|---|---|---|---|---|
| ANTs 2.6.5 | 1 | 716 ms | 1.0× |  |
| ANTs 2.6.5 | 12 | 232 ms | **3.1×** |  |
| larmorx | 1 | 119 ms | **6.0×** | bit-identical |
| larmorx | 4 | 85 ms | **8.4×** | bit-identical |
| larmorx | 12 | 62 ms | **11.6×** | bit-identical |

## SmoothImage 3 sigma 1 voxel, raw T1w

| Tool | Threads | Median | Speed-up vs ANTs, 1 thread | Output vs ANTs |
|---|---|---|---|---|
| ANTs 2.6.5 | 1 | 442 ms | 1.0× |  |
| ANTs 2.6.5 | 12 | 134 ms | **3.3×** |  |
| larmorx | 1 | 119 ms | **3.7×** | bit-identical |
| larmorx | 4 | 79 ms | **5.6×** | bit-identical |
| larmorx | 12 | 63 ms | **7.0×** | bit-identical |

## SmoothImage 3 median radius 1, raw T1w

| Tool | Threads | Median | Speed-up vs ANTs, 1 thread | Output vs ANTs |
|---|---|---|---|---|
| ANTs 2.6.5 | 1 | 2.00 s | 1.0× |  |
| ANTs 2.6.5 | 12 | 361 ms | **5.5×** |  |
| larmorx | 1 | 1.26 s | **1.6×** | bit-identical |
| larmorx | 4 | 381 ms | **5.2×** | bit-identical |
| larmorx | 12 | 259 ms | **7.7×** | bit-identical |

## SmoothImage 4 sigma 1 voxel, fMRIPrep BOLD series (4D, 28 MB gzipped)

| Tool | Threads | Median | Speed-up vs ANTs, 1 thread | Output vs ANTs |
|---|---|---|---|---|
| ANTs 2.6.5 | 1 | 1.84 s | 1.0× |  |
| ANTs 2.6.5 | 12 | 524 ms | **3.5×** |  |
| larmorx | 1 | 641 ms | **2.9×** | bit-identical |
| larmorx | 4 | 376 ms | **4.9×** | bit-identical |
| larmorx | 12 | 337 ms | **5.4×** | bit-identical |

## ResampleImageBySpacing 3 to 2 mm (smoothed, linear), raw T1w

| Tool | Threads | Median | Speed-up vs ANTs, 1 thread | Output vs ANTs |
|---|---|---|---|---|
| ANTs 2.6.5 | 1 | 496 ms | 1.0× |  |
| ANTs 2.6.5 | 12 | 151 ms | **3.3×** |  |
| larmorx | 1 | 149 ms | **3.3×** | bit-identical |
| larmorx | 4 | 87 ms | **5.7×** | bit-identical |
| larmorx | 12 | 80 ms | **6.2×** | bit-identical |

## ResampleImageBySpacing 3 to 3 mm, MNI 1 mm

| Tool | Threads | Median | Speed-up vs ANTs, 1 thread | Output vs ANTs |
|---|---|---|---|---|
| ANTs 2.6.5 | 1 | 629 ms | 1.0× |  |
| ANTs 2.6.5 | 12 | 193 ms | **3.3×** |  |
| larmorx | 1 | 252 ms | **2.5×** | bit-identical |
| larmorx | 4 | 161 ms | **3.9×** | bit-identical |
| larmorx | 12 | 148 ms | **4.2×** | bit-identical |

## ResampleImage 3 to 2 mm (linear), MNI 1 mm

| Tool | Threads | Median | Speed-up vs ANTs, 1 thread | Output vs ANTs |
|---|---|---|---|---|
| ANTs 2.6.5 | 1 | 431 ms | 1.0× |  |
| ANTs 2.6.5 | 12 | 434 ms | **1.0×** |  |
| larmorx | 1 | 91 ms | **4.7×** | bit-identical |
| larmorx | 4 | 76 ms | **5.7×** | bit-identical |
| larmorx | 12 | 70 ms | **6.2×** | bit-identical |

## ResampleImage 3 to 1 mm, B-spline order 3 as unsigned short, raw T1w

| Tool | Threads | Median | Speed-up vs ANTs, 1 thread | Output vs ANTs |
|---|---|---|---|---|
| ANTs 2.6.5 | 1 | 3.17 s | 1.0× |  |
| ANTs 2.6.5 | 12 | 947 ms | **3.3×** |  |
| larmorx | 1 | 1.49 s | **2.1×** | bit-identical |
| larmorx | 4 | 482 ms | **6.6×** | bit-identical |
| larmorx | 12 | 334 ms | **9.5×** | bit-identical |

## ResampleImage 3 to 1.5 mm, windowed sinc (Hamming), boldref

| Tool | Threads | Median | Speed-up vs ANTs, 1 thread | Output vs ANTs |
|---|---|---|---|---|
| ANTs 2.6.5 | 1 | 7.05 s | 1.0× |  |
| ANTs 2.6.5 | 12 | 1.21 s | **5.8×** |  |
| larmorx | 1 | 1.80 s | **3.9×** | bit-identical |
| larmorx | 4 | 472 ms | **14.9×** | bit-identical |
| larmorx | 12 | 329 ms | **21.4×** | bit-identical |

## Notes

- **Where the time goes.** Reading the gzipped input is sequential in both programs (zlib-rs in larmorx, zlib in ITK): about 50 ms of larmorx's time for the raw T1w, which is why 4 and 12 threads differ little for the faster filters. The recursive filters cost a few multiply-adds per voxel and pass; the Laplacian and the gradient magnitude make nine passes over a 3D image, `SmoothImage` three.
- **How larmorx runs the passes.** Every pass filters 32 neighbouring lines together, laid out so that the 32 recursions advance side by side on contiguous values (lines along the first axis are transposed into that layout). Each line's arithmetic is exactly ITK's, so the result does not depend on the thread count or on the grouping. The resampler computes the part of the linear interpolation that is constant along an output row once per row when the row runs along an input axis (axis-aligned images).
- **Threads.** ITK splits every pass into regions per thread, so ANTs gains 3–5× from 12 threads. larmorx gains less from many threads on these images: reading the input stays sequential, and a pass over a 6-million-voxel image is short.

## Environment

| Component | Version |
|---|---|
| larmorx | 0.0.1 (30959407505d-dirty) |
| larmorx-testdata | 28167dea9be5 |
| Python | 3.12.10 |
| Platform | Linux x86_64 (Linux-6.8.0-124-generic-x86_64-with-glibc2.35) |
| CPU | Intel(R) Core(TM) i7-8750H CPU @ 2.20GHz, 12 logical CPUs |
| numpy | 2.3.5 |
| nibabel | 5.4.2 |
