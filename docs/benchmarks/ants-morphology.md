# Benchmarks: ANTs morphology and mask operations (ImageMath MD, ME, MC, GD, GO, FillHoles, PadImage)

larmorx against ANTs 2.6.5's own binaries on real images, as fMRIPrep uses these programs. Median of 5 runs after a warm-up.

- **Generated:** 2026-10-10 on Linux x86_64, with `python -m larmorx_validation bench ants-morphology --repeats 5 --threads 1 4 0`
- **What is timed:** the whole command: reading the gzipped inputs, the operation, writing the output as uncompressed `.nii` (ITK compresses on one thread, which would dominate ANTs' time). ANTs runs as its own process; larmorx runs in-process through its console entry point. Both get the thread count through `ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS`.
- **Same result:** every larmorx output is compared with ANTs' (all values).
- **Machine load** before the run (1, 5, 15 min): 2.33, 3.11, 2.62; no other benchmark ran meanwhile.

## ImageMath MD 2 (sMRIPrep), T1w brain mask ds000005 (160×192×192)

| Tool | Threads | Median | Speed-up vs ANTs, 1 thread | Output vs ANTs |
|---|---|---|---|---|
| ANTs 2.6.5 | 1 | 249 ms | 1.0× |  |
| ANTs 2.6.5 | 12 | 251 ms | **1.0×** |  |
| larmorx | 1 | 137 ms | **1.8×** | bit-identical |
| larmorx | 4 | 54 ms | **4.6×** | bit-identical |
| larmorx | 12 | 41 ms | **6.1×** | bit-identical |

## ImageMath MD 5 (sMRIPrep), T1w brain mask

| Tool | Threads | Median | Speed-up vs ANTs, 1 thread | Output vs ANTs |
|---|---|---|---|---|
| ANTs 2.6.5 | 1 | 275 ms | 1.0× |  |
| ANTs 2.6.5 | 12 | 280 ms | **1.0×** |  |
| larmorx | 1 | 141 ms | **1.9×** | bit-identical |
| larmorx | 4 | 53 ms | **5.2×** | bit-identical |
| larmorx | 12 | 42 ms | **6.5×** | bit-identical |

## ImageMath ME 2 (sMRIPrep), T1w brain mask

| Tool | Threads | Median | Speed-up vs ANTs, 1 thread | Output vs ANTs |
|---|---|---|---|---|
| ANTs 2.6.5 | 1 | 588 ms | 1.0× |  |
| ANTs 2.6.5 | 12 | 592 ms | **1.0×** |  |
| larmorx | 1 | 302 ms | **1.9×** | bit-identical |
| larmorx | 4 | 95 ms | **6.2×** | bit-identical |
| larmorx | 12 | 79 ms | **7.4×** | bit-identical |

## ImageMath ME 10 (sMRIPrep's CSF step), T1w brain mask

| Tool | Threads | Median | Speed-up vs ANTs, 1 thread | Output vs ANTs |
|---|---|---|---|---|
| ANTs 2.6.5 | 1 | 900 ms | 1.0× |  |
| ANTs 2.6.5 | 12 | 934 ms | **1.0×** |  |
| larmorx | 1 | 306 ms | **2.9×** | bit-identical |
| larmorx | 4 | 96 ms | **9.4×** | bit-identical |
| larmorx | 12 | 81 ms | **11.1×** | bit-identical |

## ImageMath MC 4, T1w brain mask

| Tool | Threads | Median | Speed-up vs ANTs, 1 thread | Output vs ANTs |
|---|---|---|---|---|
| ANTs 2.6.5 | 1 | 1.09 s | 1.0× |  |
| ANTs 2.6.5 | 12 | 1.06 s | **1.0×** |  |
| larmorx | 1 | 468 ms | **2.3×** | bit-identical |
| larmorx | 4 | 150 ms | **7.3×** | bit-identical |
| larmorx | 12 | 125 ms | **8.7×** | bit-identical |

## ImageMath FillHoles 2 (sMRIPrep), T1w brain mask

| Tool | Threads | Median | Speed-up vs ANTs, 1 thread | Output vs ANTs |
|---|---|---|---|---|
| ANTs 2.6.5 | 1 | 2.89 s | 1.0× |  |
| ANTs 2.6.5 | 12 | 2.85 s | **1.0×** |  |
| larmorx | 1 | 70 ms | **41.2×** | bit-identical |
| larmorx | 4 | 42 ms | **69.5×** | bit-identical |
| larmorx | 12 | 35 ms | **82.9×** | bit-identical |

## ImageMath PadImage 10 (sMRIPrep), T1w brain mask

| Tool | Threads | Median | Speed-up vs ANTs, 1 thread | Output vs ANTs |
|---|---|---|---|---|
| ANTs 2.6.5 | 1 | 128 ms | 1.0× |  |
| ANTs 2.6.5 | 12 | 129 ms | **1.0×** |  |
| larmorx | 1 | 98 ms | **1.3×** | bit-identical |
| larmorx | 4 | 97 ms | **1.3×** | bit-identical |
| larmorx | 12 | 96 ms | **1.3×** | bit-identical |

## ImageMath GD 2 (grayscale dilation), raw T1w (160×192×192 int16)

| Tool | Threads | Median | Speed-up vs ANTs, 1 thread | Output vs ANTs |
|---|---|---|---|---|
| ANTs 2.6.5 | 1 | 1.75 s | 1.0× |  |
| ANTs 2.6.5 | 12 | 488 ms | **3.6×** |  |
| larmorx | 1 | 171 ms | **10.2×** | bit-identical |
| larmorx | 4 | 96 ms | **18.2×** | bit-identical |
| larmorx | 12 | 91 ms | **19.3×** | bit-identical |

## ImageMath GO 2 (grayscale opening), raw T1w

| Tool | Threads | Median | Speed-up vs ANTs, 1 thread | Output vs ANTs |
|---|---|---|---|---|
| ANTs 2.6.5 | 1 | 2.69 s | 1.0× |  |
| ANTs 2.6.5 | 12 | 634 ms | **4.2×** |  |
| larmorx | 1 | 272 ms | **9.9×** | bit-identical |
| larmorx | 4 | 142 ms | **19.0×** | bit-identical |
| larmorx | 12 | 122 ms | **22.0×** | bit-identical |

## Notes

- **Binary morphology** (`MD`, `ME`, `MC`): ITK traces the object's border and paints the ball along it, so its time grows with the border and the ball (`ME 10` paints a ball of 4,945 voxels). larmorx computes the same voxel sets from an exact integer distance transform, whose cost does not depend on the radius, in parallel over lines.
- **FillHoles**: ANTs computes a Danielsson distance map (sequential) only to find the background, which larmorx takes directly; the connected components are run-length encoded in both.
- **Grayscale morphology** (`GD`, `GO`): ITK scans the ball or keeps a moving histogram per voxel; larmorx splits the ball into runs along x and reuses the running maximum of each run width for every row.
- **PadImage** is a copy: reading the gzipped input and writing the output dominate.

## Environment

| Component | Version |
|---|---|
| larmorx | 0.0.1 (8252e615467a-dirty) |
| larmorx-testdata | f32b1d60627c |
| Python | 3.12.10 |
| Platform | Linux x86_64 (Linux-6.8.0-124-generic-x86_64-with-glibc2.35) |
| CPU | Intel(R) Core(TM) i7-8750H CPU @ 2.20GHz, 12 logical CPUs |
| numpy | 2.3.5 |
| nibabel | 5.4.2 |
