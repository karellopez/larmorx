# Benchmarks: ANTs morphology, components and distance maps (ImageMath MD, ME, MC, GD, GO, FillHoles, PadImage, GetLargestComponent, D, MaurerDistance)

larmorx against ANTs 2.6.5's own binaries on real images, as fMRIPrep uses these programs. Median of 5 runs after a warm-up.

- **Generated:** 2026-10-11 on Linux x86_64, with `python -m larmorx_validation bench ants-morphology --repeats 5 --threads 1 4 0`
- **What is timed:** the whole command: reading the gzipped inputs, the operation, writing the output as uncompressed `.nii` (ITK compresses on one thread, which would dominate ANTs' time). ANTs runs as its own process; larmorx runs in-process through its console entry point. Both get the thread count through `ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS`.
- **Same result:** every larmorx output is compared with ANTs' (all values).
- **Machine load** before the run (1, 5, 15 min): 0.54, 2.24, 2.28; no other benchmark ran meanwhile.

## ImageMath MD 2 (sMRIPrep), T1w brain mask ds000005 (160×192×192)

| Tool | Threads | Median | Speed-up vs ANTs, 1 thread | Output vs ANTs |
|---|---|---|---|---|
| ANTs 2.6.5 | 1 | 248 ms | 1.0× |  |
| ANTs 2.6.5 | 12 | 254 ms | **1.0×** |  |
| larmorx | 1 | 130 ms | **1.9×** | bit-identical |
| larmorx | 4 | 50 ms | **5.0×** | bit-identical |
| larmorx | 12 | 40 ms | **6.2×** | bit-identical |

## ImageMath MD 5 (sMRIPrep), T1w brain mask

| Tool | Threads | Median | Speed-up vs ANTs, 1 thread | Output vs ANTs |
|---|---|---|---|---|
| ANTs 2.6.5 | 1 | 287 ms | 1.0× |  |
| ANTs 2.6.5 | 12 | 273 ms | **1.1×** |  |
| larmorx | 1 | 134 ms | **2.1×** | bit-identical |
| larmorx | 4 | 51 ms | **5.7×** | bit-identical |
| larmorx | 12 | 40 ms | **7.2×** | bit-identical |

## ImageMath ME 2 (sMRIPrep), T1w brain mask

| Tool | Threads | Median | Speed-up vs ANTs, 1 thread | Output vs ANTs |
|---|---|---|---|---|
| ANTs 2.6.5 | 1 | 589 ms | 1.0× |  |
| ANTs 2.6.5 | 12 | 585 ms | **1.0×** |  |
| larmorx | 1 | 279 ms | **2.1×** | bit-identical |
| larmorx | 4 | 85 ms | **6.9×** | bit-identical |
| larmorx | 12 | 71 ms | **8.3×** | bit-identical |

## ImageMath ME 10 (sMRIPrep's CSF step), T1w brain mask

| Tool | Threads | Median | Speed-up vs ANTs, 1 thread | Output vs ANTs |
|---|---|---|---|---|
| ANTs 2.6.5 | 1 | 956 ms | 1.0× |  |
| ANTs 2.6.5 | 12 | 921 ms | **1.0×** |  |
| larmorx | 1 | 278 ms | **3.4×** | bit-identical |
| larmorx | 4 | 90 ms | **10.7×** | bit-identical |
| larmorx | 12 | 72 ms | **13.3×** | bit-identical |

## ImageMath MC 4, T1w brain mask

| Tool | Threads | Median | Speed-up vs ANTs, 1 thread | Output vs ANTs |
|---|---|---|---|---|
| ANTs 2.6.5 | 1 | 1.08 s | 1.0× |  |
| ANTs 2.6.5 | 12 | 1.06 s | **1.0×** |  |
| larmorx | 1 | 414 ms | **2.6×** | bit-identical |
| larmorx | 4 | 137 ms | **7.9×** | bit-identical |
| larmorx | 12 | 114 ms | **9.5×** | bit-identical |

## ImageMath FillHoles 2 (sMRIPrep), T1w brain mask

| Tool | Threads | Median | Speed-up vs ANTs, 1 thread | Output vs ANTs |
|---|---|---|---|---|
| ANTs 2.6.5 | 1 | 2.91 s | 1.0× |  |
| ANTs 2.6.5 | 12 | 2.88 s | **1.0×** |  |
| larmorx | 1 | 68 ms | **42.8×** | bit-identical |
| larmorx | 4 | 44 ms | **66.6×** | bit-identical |
| larmorx | 12 | 37 ms | **77.8×** | bit-identical |

## ImageMath PadImage 10 (sMRIPrep), T1w brain mask

| Tool | Threads | Median | Speed-up vs ANTs, 1 thread | Output vs ANTs |
|---|---|---|---|---|
| ANTs 2.6.5 | 1 | 126 ms | 1.0× |  |
| ANTs 2.6.5 | 12 | 125 ms | **1.0×** |  |
| larmorx | 1 | 98 ms | **1.3×** | bit-identical |
| larmorx | 4 | 99 ms | **1.3×** | bit-identical |
| larmorx | 12 | 99 ms | **1.3×** | bit-identical |

## ImageMath GD 2 (grayscale dilation), raw T1w (160×192×192 int16)

| Tool | Threads | Median | Speed-up vs ANTs, 1 thread | Output vs ANTs |
|---|---|---|---|---|
| ANTs 2.6.5 | 1 | 1.84 s | 1.0× |  |
| ANTs 2.6.5 | 12 | 475 ms | **3.9×** |  |
| larmorx | 1 | 170 ms | **10.9×** | bit-identical |
| larmorx | 4 | 97 ms | **19.0×** | bit-identical |
| larmorx | 12 | 95 ms | **19.5×** | bit-identical |

## ImageMath GO 2 (grayscale opening), raw T1w

| Tool | Threads | Median | Speed-up vs ANTs, 1 thread | Output vs ANTs |
|---|---|---|---|---|
| ANTs 2.6.5 | 1 | 2.75 s | 1.0× |  |
| ANTs 2.6.5 | 12 | 619 ms | **4.4×** |  |
| larmorx | 1 | 277 ms | **9.9×** | bit-identical |
| larmorx | 4 | 143 ms | **19.3×** | bit-identical |
| larmorx | 12 | 127 ms | **21.6×** | bit-identical |

## ImageMath GetLargestComponent (sMRIPrep), T1w brain mask

| Tool | Threads | Median | Speed-up vs ANTs, 1 thread | Output vs ANTs |
|---|---|---|---|---|
| ANTs 2.6.5 | 1 | 240 ms | 1.0× |  |
| ANTs 2.6.5 | 12 | 180 ms | **1.3×** |  |
| larmorx | 1 | 55 ms | **4.4×** | bit-identical |
| larmorx | 4 | 34 ms | **7.0×** | bit-identical |
| larmorx | 12 | 29 ms | **8.4×** | bit-identical |

## ImageMath GetLargestComponent (sMRIPrep, on the WM class), WM probability map

| Tool | Threads | Median | Speed-up vs ANTs, 1 thread | Output vs ANTs |
|---|---|---|---|---|
| ANTs 2.6.5 | 1 | 272 ms | 1.0× |  |
| ANTs 2.6.5 | 12 | 213 ms | **1.3×** |  |
| larmorx | 1 | 62 ms | **4.4×** | bit-identical |
| larmorx | 4 | 41 ms | **6.7×** | bit-identical |
| larmorx | 12 | 35 ms | **7.7×** | bit-identical |

## ImageMath D (Danielsson distance map), T1w brain mask

| Tool | Threads | Median | Speed-up vs ANTs, 1 thread | Output vs ANTs |
|---|---|---|---|---|
| ANTs 2.6.5 | 1 | 2.90 s | 1.0× |  |
| ANTs 2.6.5 | 12 | 2.92 s | **1.0×** |  |
| larmorx | 1 | 547 ms | **5.3×** | bit-identical |
| larmorx | 4 | 548 ms | **5.3×** | bit-identical |
| larmorx | 12 | 600 ms | **4.8×** | bit-identical |

## ImageMath D, BOLD brain mask (64×64×34)

| Tool | Threads | Median | Speed-up vs ANTs, 1 thread | Output vs ANTs |
|---|---|---|---|---|
| ANTs 2.6.5 | 1 | 78 ms | 1.0× |  |
| ANTs 2.6.5 | 12 | 76 ms | **1.0×** |  |
| larmorx | 1 | 11 ms | **7.0×** | bit-identical |
| larmorx | 4 | 11 ms | **7.0×** | bit-identical |
| larmorx | 12 | 11 ms | **7.1×** | bit-identical |

## ImageMath MaurerDistance, T1w brain mask

| Tool | Threads | Median | Speed-up vs ANTs, 1 thread | Output vs ANTs |
|---|---|---|---|---|
| ANTs 2.6.5 | 1 | 518 ms | 1.0× |  |
| ANTs 2.6.5 | 12 | 115 ms | **4.5×** |  |
| larmorx | 1 | 213 ms | **2.4×** | bit-identical |
| larmorx | 4 | 72 ms | **7.2×** | bit-identical |
| larmorx | 12 | 58 ms | **8.9×** | bit-identical |

## Notes

- **Binary morphology** (`MD`, `ME`, `MC`): ITK traces the object's border and paints the ball along it, so its time grows with the border and the ball (`ME 10` paints a ball of 4,945 voxels). larmorx computes the same voxel sets from an exact integer distance transform, whose cost does not depend on the radius, in parallel over lines.
- **FillHoles**: ANTs computes a Danielsson distance map (sequential) only to find the background, which larmorx takes directly; the connected components are run-length encoded in both.
- **Grayscale morphology** (`GD`, `GO`): ITK scans the ball or keeps a moving histogram per voxel; larmorx splits the ball into runs along x and reuses the running maximum of each run width for every row.
- **PadImage** is a copy: reading the gzipped input and writing the output dominate.
- **GetLargestComponent**: run-length connected components in both; larmorx extracts the runs in parallel and links them in one pass.
- **D** (Danielsson): the propagation is order-dependent, so larmorx replays ITK's sweeps in the same order on one thread too; it is faster because it walks the image with flat indices and keeps each voxel's squared length instead of recomputing it.
- **MaurerDistance**: separable passes in both, threaded over lines; ITK computes the inner contour with run-length encoding, larmorx row by row.

## Environment

| Component | Version |
|---|---|
| larmorx | 0.0.1 (57245bee1dee-dirty) |
| larmorx-testdata | f32b1d60627c |
| Python | 3.12.10 |
| Platform | Linux x86_64 (Linux-6.8.0-124-generic-x86_64-with-glibc2.35) |
| CPU | Intel(R) Core(TM) i7-8750H CPU @ 2.20GHz, 12 logical CPUs |
| numpy | 2.3.5 |
| nibabel | 5.4.2 |
