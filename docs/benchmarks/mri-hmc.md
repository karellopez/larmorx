# Benchmarks: head-motion correction (`lx.mri.hmc`)

larmorx's clean-room head-motion correction against FSL 6.0.7's mcflirt (single-threaded) on real BOLD runs, both with fMRIPrep's command, larmorx on 1, 4, 12 threads. larmorx: median of 3 runs after a warm-up.

- **Generated:** 2026-10-10 on Linux x86_64, with `python -m larmorx_validation bench mri-hmc --repeats 3 --threads 1 4 0`; load before the run: load average 3.11 6.09 6.53 (the machine idles at about 3; CPUs 95 % idle).
- **What is timed:** the whole command `-in <run> -out <out>.nii.gz -reffile <ref> -mats`: reading the gzipped run, the three-stage estimation, the corrected series (written gzipped) and the `.mat` files. mcflirt runs as its own process; larmorx in-process through its console entry point. The Python column (`lx.mri.hmc(..., resample=False)`) estimates only, which is all fMRIPrep uses.
- **Same result:** each larmorx run's matrices are compared with mcflirt's (RMS deviation, radius 80 mm; the parity record has the full comparison).

## ds000005 sub-01 run 1: 64×64×34, 240 volumes, int16

| Tool | Threads | Median | Speed-up vs mcflirt | Matrices vs mcflirt |
|---|---|---|---|---|
| mcflirt (FSL 6.0.7) | 1 | 30.67 s | 1.0× |  |
| larmorx (CLI) | 1 | 26.64 s | **1.2×** | RMS dev. median 0.0094 mm, 95th pct 0.0262 mm |
| larmorx (Python, matrices only) | 1 | 23.40 s | **1.3×** |  |
| larmorx (CLI) | 4 | 8.41 s | **3.6×** | RMS dev. median 0.0094 mm, 95th pct 0.0262 mm |
| larmorx (Python, matrices only) | 4 | 7.04 s | **4.4×** |  |
| larmorx (CLI) | 12 | 6.21 s | **4.9×** | RMS dev. median 0.0094 mm, 95th pct 0.0262 mm |
| larmorx (Python, matrices only) | 12 | 5.39 s | **5.7×** |  |

## ds006010 sub-206: 96×96×69, 113 volumes, uint16 (multiband)

| Tool | Threads | Median | Speed-up vs mcflirt | Matrices vs mcflirt |
|---|---|---|---|---|
| mcflirt (FSL 6.0.7) | 1 | 54.53 s | 1.0× |  |
| larmorx (CLI) | 1 | 23.58 s | **2.3×** | RMS dev. median 0.0073 mm, 95th pct 0.0142 mm |
| larmorx (Python, matrices only) | 1 | 12.24 s | **4.5×** |  |
| larmorx (CLI) | 4 | 7.66 s | **7.1×** | RMS dev. median 0.0073 mm, 95th pct 0.0142 mm |
| larmorx (Python, matrices only) | 4 | 4.12 s | **13.2×** |  |
| larmorx (CLI) | 12 | 5.79 s | **9.4×** | RMS dev. median 0.0073 mm, 95th pct 0.0142 mm |
| larmorx (Python, matrices only) | 12 | 3.32 s | **16.4×** |  |

## ds000258 sub-21262 echo 4: 64×64×30, 239 volumes, int16 (big-endian)

| Tool | Threads | Median | Speed-up vs mcflirt | Matrices vs mcflirt |
|---|---|---|---|---|
| mcflirt (FSL 6.0.7) | 1 | 39.15 s | 1.0× |  |
| larmorx (CLI) | 1 | 33.16 s | **1.2×** | RMS dev. median 0.0103 mm, 95th pct 0.0181 mm |
| larmorx (Python, matrices only) | 1 | 30.09 s | **1.3×** |  |
| larmorx (CLI) | 4 | 10.45 s | **3.7×** | RMS dev. median 0.0103 mm, 95th pct 0.0181 mm |
| larmorx (Python, matrices only) | 4 | 10.01 s | **3.9×** |  |
| larmorx (CLI) | 12 | 7.48 s | **5.2×** | RMS dev. median 0.0103 mm, 95th pct 0.0181 mm |
| larmorx (Python, matrices only) | 12 | 6.82 s | **5.7×** |  |

## Notes

- **Where the parallelism comes from.** The first stage (8 mm) is a chain: each volume starts from its predecessor's result, so its volumes run one after another and each cost evaluation samples its grid slices in parallel. The second and third stages (4 mm) start every volume from its own previous result, so the volumes run in parallel. Every evaluation gives the same bits on any number of threads.
- **Single-thread speed** is close to mcflirt's: the cost function follows mcflirt's float32 arithmetic point by point (that is what keeps the results within mcflirt's own variability), so the gain comes from the threads.

## Environment

| Component | Version |
|---|---|
| larmorx | 0.0.1 (fc392f579ce7-dirty) |
| larmorx-testdata | 28167dea9be5 |
| Python | 3.12.10 |
| Platform | Linux x86_64 (Linux-6.8.0-124-generic-x86_64-with-glibc2.35) |
| CPU | Intel(R) Core(TM) i7-8750H CPU @ 2.20GHz, 12 logical CPUs |
| numpy | 2.3.5 |
| nibabel | 5.4.2 |
