# The address-space limit of the original programs

The tools behind the limit of `validation/src/larmorx_validation/address_space.py` (2026-10-28,
[machine-move-proof.md](../../../docs/findings/machine-move-proof.md), "Fixed"): every original
program the harness runs, and larmorx's in-process runs of the ANTs image programs, run under
`RLIMIT_AS` = 64 GiB (`LARMORX_ADDRESS_SPACE_LIMIT`), so an impossible allocation is refused alike
on every machine. The limit has to sit between the largest real call's virtual size and the
smallest allocation the records expect to be refused. Run from the repository's checkout with the
environment of `scripts/orchestration/env.sh`, in a memory box (`scripts/orchestration/boxed.sh`).

| Tool | What it measures | How to run it |
|---|---|---|
| `vm_peak.py <out> [--jobs N] [--keys KEY...]` | the largest real calls of the original programs, re-run from the oracle cache (a copy: the workspace's cache is not written) under the limit: each run's largest single-process `VmPeak` (every process of the run sampled every 20 ms), and whether it gives its stored result | `boxed.sh 26 python -P vm_peak.py /tmp/vm --jobs 3` (about 15 minutes) |
| `refused_sizes.py <out>` | the allocation each refusal case asks for: the original's run traced with `strace` (`mmap`, `mremap`, `brk`), the largest request refused with `ENOMEM` | `boxed.sh 12 python -P refused_sizes.py /tmp/refused` (a minute) |
| `harness_peak.py <out> <suite> <tier> <case>` | the harness's own `VmPeak` while larmorx runs in-process on a suite's cases (its address space counts toward the limit there) | `boxed.sh 24 python -P harness_peak.py /tmp/h ants-image-math panel m/bold-mb8` |

## The measurements (2026-10-28, the main machine, limit 64 GiB)

The largest real calls, each the same as its stored result under the limit (`vm_peak.py`):

| Original run | Largest process `VmPeak` |
|---|---|
| mcflirt, fMRIPrep's call on the 600-volume multiband-8 BOLD series (ds004958) | 9.40 GiB |
| antsApplyTransforms, the same series to T1w (4 threads) | 6.65 GiB |
| ImageMath `m` on the same series (4 threads) | 3.56 GiB |
| 3dTshift, the 96-slice multiband BOLD series (OpenMP at 32 threads, and unset) | 2.57 GiB |
| antsRegistration, sMRIPrep's T1w to MNI on the panel (read from the 14 running ones) | 2.65 GiB |
| N4BiasFieldCorrection, sMRIPrep's brain extraction (ds005454) | 2.03 GiB |
| fMRIPrep's `bet` through Apptainer (the 7 T T1w; Apptainer's starter) | 1.84 GiB |
| MultiplyImages, SmoothImage on a 4D Philips multiband series | 1.57, 1.32 GiB |
| Atropos (the child's T1w and T2w), betsurf with a T2, 3dUnifize, ResampleImage (7 T), bet, DenoiseImage, antsAI, antsAffineInitializer | 0.25 to 1.46 GiB |

The allocations the records expect to be refused (`refused_sizes.py`):

| Case | Refused request |
|---|---|
| ImageMath `binary-morphology/MD-radius-negative` | 96.0 GiB |
| ResampleImage `difftest/allocation-refused` | 109.2 GiB |
| ResampleImageBySpacing `difftest/allocation-refused` | 110.9 GiB |
| N4BiasFieldCorrection `difftest/allocation-refused` | 3.2·10¹⁶ bytes (beyond any address space) |

64 GiB is 6.8 times the largest real call and 32 GiB below the smallest refusal; the old laptop's
RAM plus swap (94 GiB), which decided the records, lies between the two as well.
