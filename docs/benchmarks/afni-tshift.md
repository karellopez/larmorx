# Benchmarks: 3dTshift

larmorx against AFNI 25.2.09's 3dTshift (single-threaded) on real BOLD runs, called as fMRIPrep calls it, with larmorx on 1, 12 threads. Median of 3 runs after a warm-up.

- **Generated:** 2026-10-10 on Linux x86_64, with `python -m larmorx_validation bench afni-tshift --repeats 3 --threads 1 0`
- **What is timed:** the whole command: reading the gzipped run, slice-timing correction (`-ignore 0 -tzero <t0> -TR <TR>s -tpattern @slice_timing.1D`, Fourier interpolation), writing the output (uncompressed `.nii`). AFNI runs as its own process; larmorx runs in-process through its console entry point. The Python API column (`lx.afni.tshift`) skips the file output.
- **Same result:** every larmorx output is compared with AFNI's (all values; the differences are within the parity thresholds, see `docs/validation/afni-tshift.md`).

## ds000210 sub-02 cuedSGT echo 1: 64×64×33, 260 volumes, int16

| Tool | Threads | Median | Speed-up vs AFNI | Output vs AFNI |
|---|---|---|---|---|
| AFNI 25.2.09 | 1 | 2.53 s | 1.0× |  |
| larmorx (CLI) | 1 | 1.27 s | **2.0×** | 6.8e-06 of values differ, max 1 |
| larmorx (Python, no file output) | 1 | 1.24 s | **2.0×** |  |
| larmorx (CLI) | 12 | 448 ms | **5.6×** | 6.8e-06 of values differ, max 1 |
| larmorx (Python, no file output) | 12 | 399 ms | **6.3×** |  |

## ds006010 sub-206 category: 96×96×69, 113 volumes, uint16 (read as float32)

| Tool | Threads | Median | Speed-up vs AFNI | Output vs AFNI |
|---|---|---|---|---|
| AFNI 25.2.09 | 1 | 7.25 s | 1.0× |  |
| larmorx (CLI) | 1 | 2.34 s | **3.1×** | 3.3e-01 of values differ, max 0.00195 |
| larmorx (Python, no file output) | 1 | 2.21 s | **3.3×** |  |
| larmorx (CLI) | 12 | 1.18 s | **6.2×** | 3.3e-01 of values differ, max 0.00195 |
| larmorx (Python, no file output) | 12 | 986 ms | **7.4×** |  |

## ds005454 sub-16 rest: 160×160×96, 280 volumes, int16 (multiband 4)

| Tool | Threads | Median | Speed-up vs AFNI | Output vs AFNI |
|---|---|---|---|---|
| AFNI 25.2.09 | 1 | 48.31 s | 1.0× |  |
| larmorx (CLI) | 1 | 24.32 s | **2.0×** | 2.6e-06 of values differ, max 1 |
| larmorx (Python, no file output) | 1 | 23.24 s | **2.1×** |  |
| larmorx (CLI) | 12 | 8.67 s | **5.6×** | 2.6e-06 of values differ, max 1 |
| larmorx (Python, no file output) | 12 | 8.23 s | **5.9×** |  |

## Notes

- **Where the time goes.** Reading the gzipped run is single-threaded (zlib-rs): with all threads it is about half of larmorx's time (measured separately: 0.18 s of 0.41 s for ds000210, 0.47 s of 0.97 s for ds006010, whose uint16 data are also converted to float32); with one thread the shift dominates.
- **Single-thread speed** comes from shifting two voxels with one complex FFT (their real and imaginary parts), a mixed-radix FFT planned once per run, and processing voxels in blocks so each series is gathered once.
- **Equal results.** The Fourier path computes the FFT in double precision where AFNI uses float32, so float32 outputs differ in the last bits and integer outputs occasionally round the other way (by 1).

## Environment

| Component | Version |
|---|---|
| larmorx | 0.0.1 (770872006940) |
| larmorx-testdata | 18c8309c46b6 |
| Python | 3.12.10 |
| Platform | Linux x86_64 (Linux-6.8.0-124-generic-x86_64-with-glibc2.35) |
| CPU | Intel(R) Core(TM) i7-8750H CPU @ 2.20GHz, 12 logical CPUs |
| numpy | 2.3.5 |
| nibabel | 5.4.2 |
