# Benchmarks: ANTs image programs (ImageMath, ThresholdImage, MultiplyImages)

larmorx against ANTs 2.6.5's own binaries on real images, as fMRIPrep uses these programs. Median of 5 runs after a warm-up.

- **Generated:** 2026-10-10 on Linux x86_64, with `python -m larmorx_validation bench ants-programs --repeats 5 --threads 1 4 0`
- **What is timed:** the whole command: reading the gzipped inputs, the operation, writing the output as uncompressed `.nii` (ITK compresses on one thread, which would dominate ANTs' time). ANTs runs as its own process; larmorx runs in-process through its console entry point. Both get the thread count through `ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS`.
- **Same result:** every larmorx output is compared with ANTs' (all values).

## TruncateImageIntensity 0.01 0.999 256 (fMRIPrep's call), raw T1w ds000005 (176×256×256 int16)

| Tool | Threads | Median | Speed-up vs ANTs, 1 thread | Output vs ANTs |
|---|---|---|---|---|
| ANTs 2.6.5 | 1 | 402 ms | 1.0× |  |
| ANTs 2.6.5 | 12 | 181 ms | **2.2×** |  |
| larmorx | 1 | 224 ms | **1.8×** | bit-identical |
| larmorx | 4 | 128 ms | **3.2×** | bit-identical |
| larmorx | 12 | 109 ms | **3.7×** | bit-identical |

## TruncateImageIntensity 0.01 0.999 256, MNI152NLin2009cAsym 1 mm (193×229×193)

| Tool | Threads | Median | Speed-up vs ANTs, 1 thread | Output vs ANTs |
|---|---|---|---|---|
| ANTs 2.6.5 | 1 | 654 ms | 1.0× |  |
| ANTs 2.6.5 | 12 | 294 ms | **2.2×** |  |
| larmorx | 1 | 499 ms | **1.3×** | bit-identical |
| larmorx | 4 | 245 ms | **2.7×** | bit-identical |
| larmorx | 12 | 175 ms | **3.7×** | bit-identical |

## ThresholdImage 0.5 1, MNI brain probability map 1 mm

| Tool | Threads | Median | Speed-up vs ANTs, 1 thread | Output vs ANTs |
|---|---|---|---|---|
| ANTs 2.6.5 | 1 | 119 ms | 1.0× |  |
| ANTs 2.6.5 | 12 | 105 ms | **1.1×** |  |
| larmorx | 1 | 72 ms | **1.6×** | bit-identical |
| larmorx | 4 | 79 ms | **1.5×** | bit-identical |
| larmorx | 12 | 52 ms | **2.3×** | bit-identical |

## ThresholdImage Otsu 3 with a brain mask, fMRIPrep T1w

| Tool | Threads | Median | Speed-up vs ANTs, 1 thread | Output vs ANTs |
|---|---|---|---|---|
| ANTs 2.6.5 | 1 | 542 ms | 1.0× |  |
| ANTs 2.6.5 | 12 | 349 ms | **1.6×** |  |
| larmorx | 1 | 220 ms | **2.5×** | bit-identical |
| larmorx | 4 | 171 ms | **3.2×** | bit-identical |
| larmorx | 12 | 164 ms | **3.3×** | bit-identical |

## MultiplyImages T1w × brain mask (fMRIPrep derivatives)

| Tool | Threads | Median | Speed-up vs ANTs, 1 thread | Output vs ANTs |
|---|---|---|---|---|
| ANTs 2.6.5 | 1 | 191 ms | 1.0× |  |
| ANTs 2.6.5 | 12 | 173 ms | **1.1×** |  |
| larmorx | 1 | 152 ms | **1.3×** | bit-identical |
| larmorx | 4 | 114 ms | **1.7×** | bit-identical |
| larmorx | 12 | 110 ms | **1.7×** | bit-identical |

## ImageMath addtozero WM GM (probability maps)

| Tool | Threads | Median | Speed-up vs ANTs, 1 thread | Output vs ANTs |
|---|---|---|---|---|
| ANTs 2.6.5 | 1 | 234 ms | 1.0× |  |
| ANTs 2.6.5 | 12 | 218 ms | **1.1×** |  |
| larmorx | 1 | 100 ms | **2.3×** | bit-identical |
| larmorx | 4 | 87 ms | **2.7×** | bit-identical |
| larmorx | 12 | 74 ms | **3.2×** | bit-identical |

## ImageMath Normalize, fMRIPrep T1w

| Tool | Threads | Median | Speed-up vs ANTs, 1 thread | Output vs ANTs |
|---|---|---|---|---|
| ANTs 2.6.5 | 1 | 143 ms | 1.0× |  |
| ANTs 2.6.5 | 12 | 142 ms | **1.0×** |  |
| larmorx | 1 | 85 ms | **1.7×** | bit-identical |
| larmorx | 4 | 70 ms | **2.0×** | bit-identical |
| larmorx | 12 | 71 ms | **2.0×** | bit-identical |

## ImageMath RescaleImage 0 1, MNI 1 mm

| Tool | Threads | Median | Speed-up vs ANTs, 1 thread | Output vs ANTs |
|---|---|---|---|---|
| ANTs 2.6.5 | 1 | 187 ms | 1.0× |  |
| ANTs 2.6.5 | 12 | 176 ms | **1.1×** |  |
| larmorx | 1 | 146 ms | **1.3×** | bit-identical |
| larmorx | 4 | 107 ms | **1.7×** | bit-identical |
| larmorx | 12 | 129 ms | **1.4×** | bit-identical |

## ImageMath 4 m 0.5, fMRIPrep BOLD series (4D, 28 MB gzipped)

| Tool | Threads | Median | Speed-up vs ANTs, 1 thread | Output vs ANTs |
|---|---|---|---|---|
| ANTs 2.6.5 | 1 | 513 ms | 1.0× |  |
| ANTs 2.6.5 | 12 | 478 ms | **1.1×** |  |
| larmorx | 1 | 530 ms | **1.0×** | bit-identical |
| larmorx | 4 | 501 ms | **1.0×** | bit-identical |
| larmorx | 12 | 358 ms | **1.4×** | bit-identical |

## Environment

| Component | Version |
|---|---|
| larmorx | 0.0.1 (17a3d2c14f30-dirty) |
| larmorx-testdata | 5434909ddf09 |
| Python | 3.12.10 |
| Platform | Linux x86_64 (Linux-6.8.0-124-generic-x86_64-with-glibc2.35) |
| CPU | Intel(R) Core(TM) i7-8750H CPU @ 2.20GHz, 12 logical CPUs |
| numpy | 2.3.5 |
| nibabel | 5.4.2 |
