# fslmaths and fslstats: benchmarks

The clean-room originals of `fslmaths` and `fslstats` (`larmorx fsl fslmaths` and `larmorx fsl
fslstats` with `LARMORX_IMPLEMENTATION=original`, crate `larmorx-fsl`) against FSL 6.0.7.23's
programs and the replicas (`larmorx-nc`, run by larmorx's dispatch as a separate program), on
fMRIPrep's calls and on the heavy operations of
[`specs/fslmaths.md`](../../specs/fslmaths.md) §20 and §21 and
[`specs/fslstats.md`](../../specs/fslstats.md) §9 and §10.

**Informal**: one run of each, on the main machine (AMD, 16 cores and 32 threads, 124 GiB,
Ubuntu 22.04) while another agent worked beside it; 2026-11-03, larmorx at the commit of this
report (the speed work of relay 5 of the originals' part). FSL and the replica are
single-threaded; the original ran with `OMP_NUM_THREADS` 1 and 14. Times are wall-clock seconds
of the whole program (reading and writing `.nii.gz` included); memory is the peak resident set
(MiB). Made by [`validation/investigations/cleanroom-maths/bench.py`](../../validation/investigations/cleanroom-maths/bench.py)
(below, "Reproducing").

## Inputs

fMRIPrep 25.2.6's ds005040 run (`oracles/fmriprep-25.2.6/ds005040-sub-002-noreconall`): the BOLD
series in T1w space (50 × 62 × 44 × 63, float32, 3 × 3 × 3.3 mm), the preprocessed T1w (191 × 288
× 288, float32, 0.85 × 0.83 × 0.83 mm), the two masks of the BOLD reference's `combine_masks`
call, and the goodvoxels chain's inputs as the replica suite makes them (the cortical ribbon a
stand-in: the segmentation's grey matter on the BOLD grid, as the run has no FreeSurfer ribbon).

## Results

| Case | FSL s (MiB) | replica s (MiB) | original, 1 thread | original, 14 threads | Checks |
|---|---|---|---|---|---|
| combine_masks: `<mask> -mul <mask>` | 0.01 (16) | 0.00 (11) | 0.00 (11) | 0.01 (11) | 1 = 14 threads; identical to FSL's |
| goodvoxels: its 14 fslmaths and 4 fslstats calls in order | 1.70 (79) | 0.73 (38) | 0.38 (41) | 0.37 (40) | 1 = 14 threads; 34 of 34 images identical to FSL's |
| BOLD `-Tmean` | 0.26 (79) | 0.12 (38) | 0.12 (40) | 0.10 (40) | 1 = 14 threads; identical |
| BOLD `-Tstd` (63 volumes) | 0.30 (79) | 0.16 (37) | 0.14 (41) | 0.17 (40) | 1 = 14 threads; identical |
| BOLD `-Tmedian` | 0.49 (79) | 0.29 (38) | 0.27 (41) | 0.11 (40) | 1 = 14 threads; identical |
| BOLD `-s 5` (goodvoxels' smoothing, on the series) | 2.79 (80) | 1.71 (70) | 0.85 (42) | 0.21 (88) | 1 = 14 threads; identical |
| BOLD `-fmedian` | 5.32 (80) | 4.19 (70) | 2.22 (42) | 0.39 (81) | 1 = 14 threads; identical |
| BOLD `-bptf 25 2` | 1.82 (80) | 5.26 (70) | 0.79 (48) | 0.26 (90) | 1 = 14 threads; identical |
| BOLD `-kernel boxv 13 -fmeanu` (FSL's FFT path) | 28.73 (113) | 16.12 (95) | 2.98 (42) | 0.43 (89) | 1 = 14 threads; max \|d\| 2.8e-6 of FSL's largest value |
| T1w `-s 5` | 11.15 (196) | 8.62 (186) | 2.28 (128) | 0.35 (127) | 1 = 14 threads; identical since relay 6's kernel correction (below; before: max \|d\| 2.8e-7 of FSL's largest value, 98.6 % within 4 ulp) |
| T1w `-dilD` | 3.80 (195) | 1.72 (185) | 1.05 (128) | 0.24 (128) | 1 = 14 threads; identical |
| T1w `-kernel sphere 6 -fmean` (FSL's FFT path) | 227.71 (4063) | 195.74 (3159) | 5.41 (188) | 0.64 (187) | 1 = 14 threads; max \|d\| 8.4e-6 of FSL's largest value |
| BET's `fslstats <T1w> -l 0.001 -C` | 0.58 (315) | 0.34 (245) | 0.45 (278) | 0.29 (277) | 1 = 14 threads; 1 of 3 numbers differs, 6.1e-8 relative (findings 21) |
| `fslstats -t <BOLD> -M -S -r` | 0.23 (47) | 0.23 (37) | 0.22 (41) | 0.23 (43) | 1 = 14 threads; 54 of 252 numbers differ (`-S`'s last digits), at most 3.0e-10 relative |

"Identical": the images' data and headers are FSL's bit for bit (`descrip` aside). The
differences are within the specs' thresholds (§21 of `specs/fslmaths.md`: the sums' largest
difference at most 1e-5 of FSL's largest value; §10 of `specs/fslstats.md`: 1e-6 relative):
- on FSL's FFT path (§10.2), FSL's FFT rounds relative to the image's largest values, where the
  original sums directly (the suite's expected divergences, findings 9 and 14 of
  [`docs/findings/fsl-maths-original.md`](../findings/fsl-maths-original.md));
- the T1w's `-s 5` (49- and 51-tap Gaussians at 0.85 and 0.83 mm) differed from FSL's in 30 % of
  the voxels at relay 5 (at most 2.8e-7 of the largest value, 98.6 % within 4 units in the last
  place, below §21's 99 %): the Gaussian's exponent took each term as `(i² d) d` where FSL takes
  `i² (d d)`, which differs at voxel sizes not exact in binary (findings 23). Corrected in relay 6:
  the original's output is FSL's bit for bit (re-run at one thread, 2.26 s, 127 MiB; the other
  timings are relay 5's, the change touches only the kernel's 49 values);
- fslstats' unmasked `-S` on a volume of many nonzero voxels (`Σ v²` of single-precision
  squares) differs from FSL's in the printed last digits (3e-10 relative; findings 23).

## What the numbers show

- **fMRIPrep's calls**: combine_masks takes milliseconds in every implementation; the
  goodvoxels chain takes 0.38 s on one thread against FSL's 1.70 s (4.5 times faster) and half
  the memory, its 34 images identical to FSL's. Its cost is now reading and writing the series:
  `-Tstd` and `-Tmean` are mostly gzip.
- **Filters**: the separable `-s 5` is 3.3 (BOLD) and 4.9 (T1w) times faster than FSL on one
  thread, 13 and 32 times on 14; `-fmedian` and `-dilD` 2.4 and 3.6 times on one thread, 14 and
  16 times on 14 threads; `-bptf` 2.3 times on one thread (it was slower than FSL before relay 5: 2.78 s),
  7 times on 14.
- **Large kernels (the orchestrator's question: an FFT of the original's own?)**: no. On FSL's
  FFT path the original's direct sum, a row of voxels at a time with the kernel's zeros left out
  of a finite volume, is 9.6 times faster than FSL's FFT on one thread for `boxv 13` on the
  series (2.98 s against 28.73 s) and 42 times faster for `sphere 6` on the T1w (5.41 s against
  227.71 s; 356 times on 14 threads), with 37 % and 5 % of FSL's memory there (FSL's FFT holds
  volumes of twice the image's size per axis: 4 GiB for the T1w). FSL's FFT costs the same for
  every kernel past its switch (the padded size depends on the image); the direct sum grows with
  the kernel's nonzero taps: `sphere 6` on the T1w is about 1,500 taps per voxel, so the direct
  sum on one thread would reach FSL's 228 s only near 60,000 taps (a sphere of about 20 mm on a
  0.83 mm grid), and on 14 threads beyond 500,000 taps. The direct sum also keeps the results
  FSL's direct sum gives (bit for bit where FSL sums directly; FSL-191 and FSL-192 corrected by
  default), which an FFT would not.
- **fslstats**: reading the image is most of its time (BET's call 0.45 s on one thread against
  FSL's 0.58, the 63-volume series 0.22 against 0.23); its memory is below FSL's (41 MiB against
  47 on the series: before relay 5 the stored values and their single-precision copy were both
  held, 71 MiB, above the rule's 60).
- **The replica**: a faithful translation, faster than FSL on most calls (Rust's code
  generation), slower on `-bptf` (5.26 s), with FSL's FFT on large kernels.
- **Threads**: every output of the original at 14 threads is the 1-thread output bit for bit
  (images decompressed, printed text), as the unit tests check for each fast path at 1, 3 and 4
  threads.
- **Memory** (the rule: at most 10 % above FSL's peak plus 8 MiB, `tests/parity/test_memory_peak.py`
  measures it at one thread): on one thread the original holds about half FSL's memory on the
  series (one copy of the image: the input's values moved into the computation, not copied, and
  the output's moved to the writer) and two thirds on the T1w. On 14 threads writing a `.nii.gz`
  adds about 3 MiB per worker (larmorx-io's parallel gzip holds two 256 KiB blocks of input and
  their output per worker): the BOLD series' 4D outputs reach 81 to 90 MiB against FSL's 80
  (within its allowance of 96), and 106 MiB on all 32 threads (above it). That is the shared
  writer's design (a follow-up for larmorx-io: one block per worker would halve it); the same
  calls written as `.nii` hold 40 MiB at any thread count.

## What makes it fast (the original's own design, `specs/fslmaths.md` §20)

- **Separable filter** (`-s`, `-fmean` after a box or Gaussian kernel): each axis a row of x at a
  time, every voxel's taps added in their order (the spec's single-precision sums), the weights'
  sum computed once per position along the axis; the rows in parallel.
- **Direct sum** (`-fmean`, `-fmeanu` with other kernels, at every kernel size): a row of x at a
  time, the kernel's rows and taps in their order; the taps outside the image left out, and the
  kernel's zeros too when the volume is finite (a sum that starts at +0 never changes by a ±0
  product, so the bits are the same); the rows in parallel.
- **`-bptf`**: the windows' sums that do not depend on the voxel (`Σ w Δ`, `Σ w Δ²`, `Σ w`, the
  denominators) once per time point; the voxels' own sums for blocks of 64 voxels at once, in the
  series' order; the series written back in place, a wave of 32,768 voxels at a time.
- **NaN on every platform**: the fast loops use the plain arithmetic, and every voxel whose
  result is NaN is computed again by x86-64's rules (`maths::num`), so the NaN bits are those of
  x86-64 on every platform (the golden cases compare them byte for byte).
- **One copy of the image**: the input's values moved where they already have the computing
  type (a neurological image mirrored in place); the output's moved to the writer where writing
  converts nothing; the convolutions in place, volume by volume; `-fmean`'s convolution of ones
  computed on one volume for a whole series (the same for every volume).
- The reductions (`-T*`), the morphological filters and fslstats' passes were already parallel
  per column, voxel or block (relays 1 to 4); the loops auto-vectorise for the baseline targets
  (SSE2 on x86-64, NEON on AArch64). Runtime dispatch to AVX2 or AVX-512 is not used: the crate
  forbids `unsafe` code, which calling a `#[target_feature]` function needs (below).

## Not done

- Runtime SIMD dispatch (AVX2/AVX-512). Measured once with a whole build for x86-64-v3 (AVX2,
  never a release setting: rule 5) on one thread, writing `.nii`: `boxv 13 -fmeanu` on the series
  2.13 s against 2.69 s, `sphere 6 -fmean` on the T1w 3.76 against 5.03 s, `-s 5` on the T1w 1.28
  against 1.63 s (1.26 to 1.34 times), `-bptf` unchanged (0.40 s). A follow-up: an audited
  `unsafe` module of dispatched copies of the row loops (the crate is `#![forbid(unsafe_code)]`,
  and calling a `#[target_feature]` function needs `unsafe`), as `larmorx_core::math` does for
  FMA; the bits stay the same (no fused operations, the same order).
- Fused chains (one pass for a run of voxelwise operations), a selection instead of a sort for
  the medians, TFCE by a union-find, van Herk/Gil-Werman for box maxima (§20): the measured calls
  do not need them (reading and writing dominate the voxelwise chains).
- A quiet machine's repeated runs (the final validation phase's benchmarks).

## Reproducing

```
scripts/orchestration/boxed.sh 24 python -P validation/investigations/cleanroom-maths/bench.py \
    <out> <worktree>/target/release/larmorx --nc <larmorx-nc> --impl fsl,replica,orig1,orig14
python -P validation/investigations/cleanroom-maths/bench.py <out> x --table
```

FSL runs from `~/fsl-6.0.7.23` (off `PATH`, timed directly: benchmarks are the oracle-output
cache's exception); this report's FSL runs are one run each from a first pass (`--fsl-from`),
the fslstats calls' from the final one.
