# Benchmarks: 3dTshift

larmorx against AFNI 25.2.09's 3dTshift (single-threaded) on real BOLD runs, called as fMRIPrep calls it, with larmorx on 1, 12 threads. Median of 3 runs after a warm-up.

- **Generated:** 2026-10-10 on Linux x86_64, with `python -m larmorx_validation bench afni-tshift --repeats 3 --threads 1 0`
- **What is timed:** the whole command: reading the gzipped run, slice-timing correction (`-ignore 0 -tzero <t0> -TR <TR>s -tpattern @slice_timing.1D`, Fourier interpolation), writing the output (uncompressed `.nii`). AFNI runs as its own process; larmorx runs in-process through its console entry point. The Python API column (`lx.afni.tshift`) skips the file output.
- **Same result:** every larmorx output is compared with AFNI's (all values; the differences are within the parity thresholds, see `docs/validation/afni-tshift.md`).

## ds000210 sub-02 cuedSGT echo 1: 64×64×33, 260 volumes, int16

| Tool | Threads | Median | Speed-up vs AFNI | Output vs AFNI |
|---|---|---|---|---|
| AFNI 25.2.09 | 1 | 2.53 s | 1.0× |  |
| larmorx (CLI) | 1 | 1.23 s | **2.1×** | 6.8e-06 of values differ, max 1 |
| larmorx (Python, no file output) | 1 | 1.28 s | **2.0×** |  |
| larmorx (CLI) | 12 | 430 ms | **5.9×** | 6.8e-06 of values differ, max 1 |
| larmorx (Python, no file output) | 12 | 385 ms | **6.6×** |  |

## ds006010 sub-206 category: 96×96×69, 113 volumes, uint16 (read as float32)

| Tool | Threads | Median | Speed-up vs AFNI | Output vs AFNI |
|---|---|---|---|---|
| AFNI 25.2.09 | 1 | 6.68 s | 1.0× |  |
| larmorx (CLI) | 1 | 2.46 s | **2.7×** | 3.3e-01 of values differ, max 0.00195 |
| larmorx (Python, no file output) | 1 | 2.28 s | **2.9×** |  |
| larmorx (CLI) | 12 | 1.13 s | **5.9×** | 3.3e-01 of values differ, max 0.00195 |
| larmorx (Python, no file output) | 12 | 999 ms | **6.7×** |  |

## ds005454 sub-16 rest: 160×160×96, 280 volumes, int16 (multiband 4)

| Tool | Threads | Median | Speed-up vs AFNI | Output vs AFNI |
|---|---|---|---|---|
| AFNI 25.2.09 | 1 | 52.95 s | 1.0× |  |
| larmorx (CLI) | 1 | 23.09 s | **2.3×** | 2.6e-06 of values differ, max 1 |
| larmorx (Python, no file output) | 1 | 22.31 s | **2.4×** |  |
| larmorx (CLI) | 12 | 8.41 s | **6.3×** | 2.6e-06 of values differ, max 1 |
| larmorx (Python, no file output) | 12 | 7.66 s | **6.9×** |  |

## The replica: `larmorx-gpl afni 3dTshift`

The GPL-3.0-or-later replica (`crates-gpl/`, a translation of AFNI's source) on the same runs, as a separate process with the same arguments. It keeps AFNI's arithmetic (float32 FFT, one voxel pair at a time) and runs the pairs of each slice in parallel. Its output is compared with AFNI's on every value (`docs/validation/afni-tshift-replica.md`).

### ds000210 sub-02 cuedSGT echo 1: 64×64×33, 260 volumes, int16

| Tool | Threads | Median | Speed-up vs AFNI | Output vs AFNI |
|---|---|---|---|---|
| AFNI 25.2.09 | 1 | 2.53 s | 1.0× |  |
| larmorx-gpl (replica, CLI) | 1 | 1.53 s | **1.7×** | bit-identical |
| larmorx-gpl (replica, CLI) | 12 | 701 ms | **3.6×** | bit-identical |

### ds006010 sub-206 category: 96×96×69, 113 volumes, uint16 (read as float32)

| Tool | Threads | Median | Speed-up vs AFNI | Output vs AFNI |
|---|---|---|---|---|
| AFNI 25.2.09 | 1 | 6.68 s | 1.0× |  |
| larmorx-gpl (replica, CLI) | 1 | 3.03 s | **2.2×** | bit-identical |
| larmorx-gpl (replica, CLI) | 12 | 1.48 s | **4.5×** | bit-identical |

### ds005454 sub-16 rest: 160×160×96, 280 volumes, int16 (multiband 4)

| Tool | Threads | Median | Speed-up vs AFNI | Output vs AFNI |
|---|---|---|---|---|
| AFNI 25.2.09 | 1 | 52.95 s | 1.0× |  |
| larmorx-gpl (replica, CLI) | 1 | 29.87 s | **1.8×** | bit-identical |
| larmorx-gpl (replica, CLI) | 12 | 15.57 s | **3.4×** | bit-identical |

## Notes

- **Conditions of this run** (added by hand, 2026-10-10). The machine was otherwise idle (other
  processes sampled every 5 s) except for a system backup (timeshift: `rsync`, then `rm`) for
  about 30 s at the start of the ds005454 jobs: AFNI's three ds005454 runs took 49.7, 53.9 and
  53.0 s. Elsewhere a run differs from its median by at most 14 % (the Python API on ds006010
  with 12 threads), mostly by less than 4 % (`afni-tshift.json` has every run). Two earlier
  runs the same night, on a busier machine, are not reported.
- **The replica** does AFNI's arithmetic step by step, a voxel pair at a time, so it is slower
  than the clean-room original; it is bit-identical to AFNI on all three runs.

- **Where the time goes.** Reading the gzipped run is single-threaded (zlib-rs): with all threads it is about half of larmorx's time (measured separately: 0.18 s of 0.41 s for ds000210, 0.47 s of 0.97 s for ds006010, whose uint16 data are also converted to float32); with one thread the shift dominates.
- **Single-thread speed** comes from shifting two voxels with one complex FFT (their real and imaginary parts), a mixed-radix FFT planned once per run, and processing voxels in blocks so each series is gathered once.
- **Equal results.** The Fourier path computes the FFT in double precision where AFNI uses float32, so float32 outputs differ in the last bits and integer outputs occasionally round the other way (by 1).

## Environment

| Component | Version |
|---|---|
| larmorx | 0.0.1 (3b3f69caabcb-dirty) |
| larmorx-testdata | ea0c2b73b3bb |
| Python | 3.12.10 |
| Platform | Linux x86_64 (Linux-6.8.0-124-generic-x86_64-with-glibc2.35) |
| CPU | Intel(R) Core(TM) i7-8750H CPU @ 2.20GHz, 12 logical CPUs |
| numpy | 2.3.5 |
| nibabel | 5.4.2 |
