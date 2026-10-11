# Benchmarks: DenoiseImage

larmorx against ANTs 2.6.5's own binaries on real images, as fMRIPrep uses these programs. Median of 3 runs after a warm-up.

- **Generated:** 2026-10-11 on Linux x86_64, with `python -m larmorx_validation bench ants-denoise --repeats 3 --threads 1 4 0`
- **What is timed:** the whole command: reading the gzipped inputs, the operation, writing the output as uncompressed `.nii` (ITK compresses on one thread, which would dominate ANTs' time). ANTs runs as its own process; larmorx runs in-process through its console entry point. Both get the thread count through `ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS`.
- **Same result:** every larmorx output is compared with ANTs' at one thread (all values).
- **Machine load** before the run (1, 5, 15 min): 1.83, 3.93, 4.96; no other benchmark ran meanwhile.

## sMRIPrep: -n Rician -s 1, raw T1w ds000005 (160×192×192)

| Tool | Threads | Median | Speed-up vs ANTs, 1 thread | Output vs ANTs |
|---|---|---|---|---|
| ANTs 2.6.5 | 1 | 217.35 s | 1.0× |  |
| ANTs 2.6.5 | 12 | 45.19 s | **4.8×** |  |
| larmorx | 1 | 13.01 s | **16.7×** | 5522 values differ |
| larmorx | 4 | 4.02 s | **54.1×** | 5522 values differ |
| larmorx | 12 | 2.90 s | **75.1×** | 5522 values differ |

## sMRIPrep: -n Rician -s 1, T1w ds000210 (256×256×176)

| Tool | Threads | Median | Speed-up vs ANTs, 1 thread | Output vs ANTs |
|---|---|---|---|---|
| ANTs 2.6.5 | 1 | 213.44 s | 1.0× |  |
| ANTs 2.6.5 | 12 | 46.25 s | **4.6×** |  |
| larmorx | 1 | 16.34 s | **13.1×** | 5389 values differ |
| larmorx | 4 | 5.39 s | **39.6×** | 5389 values differ |
| larmorx | 12 | 3.82 s | **55.9×** | 5389 values differ |

## -n Rician, MNI152NLin2009cAsym T1w res-02 (97×115×97)

| Tool | Threads | Median | Speed-up vs ANTs, 1 thread | Output vs ANTs |
|---|---|---|---|---|
| ANTs 2.6.5 | 1 | 25.96 s | 1.0× |  |
| ANTs 2.6.5 | 12 | 5.69 s | **4.6×** |  |
| larmorx | 1 | 2.44 s | **10.7×** | 766 values differ |
| larmorx | 4 | 797 ms | **32.6×** | 766 values differ |
| larmorx | 12 | 550 ms | **47.2×** | 766 values differ |

## -n Gaussian (ANTs' default model), raw T1w ds000005

| Tool | Threads | Median | Speed-up vs ANTs, 1 thread | Output vs ANTs |
|---|---|---|---|---|
| ANTs 2.6.5 | 1 | 213.57 s | 1.0× |  |
| ANTs 2.6.5 | 12 | 45.43 s | **4.7×** |  |
| larmorx | 1 | 12.57 s | **17.0×** | 4865 values differ |
| larmorx | 4 | 3.68 s | **58.1×** | 4865 values differ |
| larmorx | 12 | 2.80 s | **76.3×** | 4865 values differ |

## -n Rician, BOLD reference ds000005 (64×64×34)

| Tool | Threads | Median | Speed-up vs ANTs, 1 thread | Output vs ANTs |
|---|---|---|---|---|
| ANTs 2.6.5 | 1 | 1.41 s | 1.0× |  |
| ANTs 2.6.5 | 12 | 282 ms | **5.0×** |  |
| larmorx | 1 | 206 ms | **6.8×** | 44 values differ |
| larmorx | 4 | 58 ms | **24.4×** | 44 values differ |
| larmorx | 12 | 49 ms | **29.1×** | 44 values differ |

## -n Rician, fieldmap magnitude ds003012 (64×64×36)

| Tool | Threads | Median | Speed-up vs ANTs, 1 thread | Output vs ANTs |
|---|---|---|---|---|
| ANTs 2.6.5 | 1 | 3.71 s | 1.0× |  |
| ANTs 2.6.5 | 12 | 801 ms | **4.6×** |  |
| larmorx | 1 | 345 ms | **10.7×** | 112 values differ |
| larmorx | 4 | 97 ms | **38.3×** | 112 values differ |
| larmorx | 12 | 75 ms | **49.6×** | 112 values differ |

## Notes

- **Same result.** ANTs' own result changes with its thread count (ITK's threads add to shared output voxels and overwrite a shared bias image without ordering; with 12 threads it also changes between runs); larmorx gives ANTs' one-thread result on any number of threads. Where the table says values differ from ANTs at one thread, the cause is glibc's `expf`/`exp`, which are not always correctly rounded: the same ANTs binary with those functions replaced by CORE-MATH's through `LD_PRELOAD` gives larmorx's bits ([validation record](../validation/ants-denoise.md)).
- **Where the time goes.** For each voxel, every neighbour in the 5×5×5 search box that passes the mean and variance tests gets a patch distance (27 squared differences), an `expf` and a weighted sum of its 27 patch values: about 490 million such neighbours in a 1 mm T1w. ITK reads every voxel through `GetPixel` with region tests; larmorx works on padded images and computes eight neighbouring centres side by side, keeping every sum in ITK's order.
- **Threads.** larmorx's centres run in parallel by rows; the output voxels are then summed in parallel, each in the single thread's order. Reading the gzipped input, the local mean and variance and the bias smoothing are a small fixed cost.

## Environment

| Component | Version |
|---|---|
| larmorx | 0.0.1 (77bb3c657378-dirty) |
| larmorx-testdata | 00b862132e9c |
| Python | 3.12.10 |
| Platform | Linux x86_64 (Linux-6.8.0-124-generic-x86_64-with-glibc2.35) |
| CPU | Intel(R) Core(TM) i7-8750H CPU @ 2.20GHz, 12 logical CPUs |
| numpy | 2.3.5 |
| nibabel | 5.4.2 |
