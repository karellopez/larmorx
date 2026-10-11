<!-- Draft of an issue for github.com/nipreps/fmriprep. Status: draft, not yet filed. When filed, add the issue's URL here and in README.md. Register entry: PIPE-02 in docs/findings/upstream-defects.md. -->
# ResampleSeries corrupts random volumes when OpenBLAS runs more than one thread

## Summary

With `OMP_NUM_THREADS` greater than 1, the one-shot resampler (`ResampleSeries`) writes wrong
values into a few random volumes of the output series. The corrupted volumes change from run to
run, and nothing reports an error. The official container avoids this only because its
environment sets `OMP_NUM_THREADS=1`. fMRIPrep's own code does not set it. So installs outside
the container, and Apptainer/Singularity runs without `--cleanenv` on hosts where
`OMP_NUM_THREADS` is set, can write corrupted `desc-preproc_bold` files.

## Environment

- fMRIPrep 25.2.6, official image `nipreps/fmriprep@sha256:ccde6053b426c03111e0c1f18797e855b23491fb87c4dc4e9f46f7a9e232784e`,
  run with Apptainer 1.4.5.
- OpenBLAS 0.3.30 (pthreads build, from the image's pixi environment), Haswell kernels on the
  test CPU (Intel i7-8750H, 6 cores / 12 threads).
- Dataset: OpenNeuro ds000005, sub-01, run 01; `--fs-no-reconall`,
  `--output-spaces MNI152NLin2009cAsym:res-2 T1w func`, `--nprocs 8 --omp-nthreads 8`.

## Steps to reproduce

1. Run fMRIPrep with `--cleanenv`, as above, so `OMP_NUM_THREADS=1`. The outputs are correct.
2. Re-run the MNI resampling node of run 01
   (`bold_task-mixedgamblestask_run_01_wf/bold_std_wf/_in_tuple_MNI152NLin2009cAsym.res2/resample`,
   the `ResampleSeries` interface with `num_threads=8`). Use the node's own inputs, inside the
   same image, with `OMP_NUM_THREADS=8`.
3. Compare the output with the one from step 1.

## What happens

| `OMP_NUM_THREADS` | Compared with the step-1 output |
|---|---|
| 1 | bit-identical |
| 8, first run | 22,635 voxels wrong in 12 of 240 volumes (3, 6, 24, 80, ...), by up to 846; the data reach about 1,320 |
| 8, second run | 4,913 voxels wrong in 13 other volumes (1-7, 100, ...), by up to 785 |

Different volumes are hit on each run, which points to a race rather than a numerical
difference.

## Expected

The same output for any `OMP_NUM_THREADS`, or at most last-bit differences.

## Likely cause

`resample_series_async` resamples up to `num_threads` volumes at once in the default thread pool
(`fmriprep/utils/asynctools.py:5-8`, `fmriprep/interfaces/resampling.py:396-425`). Each volume
maps its coordinates through its head-motion matrix with `nibabel.affines.apply_affine`
(`resampling.py:292-297`), a matrix product numpy passes to OpenBLAS. When several Python
threads call the pthreads build of OpenBLAS at once while OpenBLAS also starts threads of its
own, the products come out wrong. We have not traced it inside OpenBLAS. Setting
`OMP_NUM_THREADS=1` makes every re-run correct.

## Possible fixes

- Limit BLAS to one thread inside the resampler, e.g. with `threadpoolctl.threadpool_limits(1)`
  around the per-volume work, or with a limit set in fMRIPrep's entry point instead of relying on
  the container's environment.
- Or compute the 4×4-by-N coordinate products without BLAS (e.g. an explicit multiply-add over
  the matrix columns), which also makes the result independent of the BLAS library and CPU
  kernel.
- Optionally, warn at start-up when `OMP_NUM_THREADS` (or the OpenBLAS/MKL equivalents) is above 1.

## Impact

- **Affected:** runs outside the official container, and Apptainer runs without `--cleanenv`
  where `OMP_NUM_THREADS` is set.
- **Not affected:** runs with the container's default environment, as the official images use
  by default.
- **Severity:** corrupted volumes in the final preprocessed BOLD series, with no error.
