# Benchmarks

Speed of larmorx against the tools it replaces, on real data from `larmorx-testdata` (files tagged `benchmark` plus typical inputs). Each report records the machine, the versions and the method (median of repeated runs after a warm-up).

| Report | Compared with |
|---|---|
| [nifti-io](nifti-io.md) | nibabel, SimpleITK |
| [ants-apply-transforms](ants-apply-transforms.md) | antsApplyTransforms (ANTs 2.6.5 via ANTsPy) |
| [afni-tshift](afni-tshift.md) | 3dTshift (AFNI 25.2.09 binary); also the GPL replica `larmorx-gpl` |
| [resample-series](resample-series.md) | fMRIPrep's one-shot resampler (`resample_image`) |
| [ants-programs](ants-programs.md) | ImageMath, ThresholdImage, MultiplyImages (ANTs 2.6.5 binaries) |
| [ants-gaussian](ants-gaussian.md) | ImageMath `Laplacian`, `G`, `Grad`; SmoothImage; ResampleImageBySpacing; ResampleImage (ANTs 2.6.5 binaries) |
| [ants-morphology](ants-morphology.md) | ImageMath `MD`, `ME`, `MC`, `GD`, `GO`, `FillHoles`, `PadImage`, `GetLargestComponent`, `D`, `MaurerDistance` (ANTs 2.6.5 binaries) |
| [ants-n4](ants-n4.md) | N4BiasFieldCorrection (ANTs 2.6.5 binary) on fMRIPrep's, sMRIPrep's and sdcflows' command lines |
| [ants-denoise](ants-denoise.md) | DenoiseImage (ANTs 2.6.5 binary) on sMRIPrep's command line and the same options on other images |
| [mri-hmc](mri-hmc.md) | mcflirt (FSL 6.0.7.17 binary) |
| [fsl-bet](fsl-bet.md) | bet, bet2 and betsurf (FSL 6.0.7.23) and BET's replica, against BET's clean-room original at 1 and 24 threads (`validation/investigations/cleanroom-bet/bench.py`) |
| [fsl-flirt](fsl-flirt.md) | flirt (FSL 6.0.7.23) and FLIRT's replica, against FLIRT's clean-room original at 1 and 14 threads, informal (`validation/investigations/cleanroom-flirt/bench.py`) |
| [fsl-fast](fsl-fast.md) | fast (FSL 6.0.7.23) and FAST's replica, against FAST's clean-room original at 1 and 14 threads, informal (`validation/investigations/cleanroom-fast/bench.py`) |
| [fsl-topup](fsl-topup.md) | topup and applytopup (FSL 6.0.7.23, at one thread and `--nthr=14`) and their replicas, against topup's clean-room original at 1 and 14 threads, informal (`validation/investigations/cleanroom-topup/bench.py`) |
| [fsl-maths.md](fsl-maths.md) | fslmaths' and fslstats' clean-room originals against FSL 6.0.7.23 and the replicas (2026-11-03; FSL's FFT path re-measured 2026-11-04 with the original's double sums): fMRIPrep's combine_masks and goodvoxels chain (4.5 times FSL's speed, half its memory), the heavy operations on a real BOLD series and T1w (1.8 to 28 times on one thread, up to 268 on 14; FSL's FFT path the largest), BET's fslstats call |
| [ants-ai](ants-ai.md) | antsAI and antsAffineInitializer (ANTs 2.6.5 binaries with CORE-MATH): time, CPU time and peak memory, larmorx in both D13 modes at 1 and 8 threads |
| [sdcflows-estimation](sdcflows-estimation.md) | sdcflows 2.17.1's masks and BIDS app, niworkflows 1.15.4's helpers and fMRIPrep 25.2.7's `ReconstructFieldmap` (in-process, the newest Python environment): the four memory calls at 1 and 14 threads, a random walker that does not converge (before and after relay 28's change), the app's whole runs on five of sdcflows' own runs with its programs run fresh (`validation/investigations/sdcflows-fieldmap-b/bench_estimation.py`) |
| [confounds](confounds.md) | nipype 1.12.0's confound algorithms, niworkflows 1.15.4's and fMRIPrep 25.2.7's confound steps (in-process, the newest Python environment): every Python node of fMRIPrep's confounds workflow on three oracle runs, larmorx at 1 and 8 threads, every result identical; the whole `bold_confounds` against fMRIPrep's recorded node durations; numpy's `svd`/`lstsq` and SciPy's `gesvd` on CompCor's largest matrices; where the time goes (`validation/investigations/confounds/bench.py`) |

Reproduce a report (build larmorx in release mode first):

```bash
maturin develop --release
python -m larmorx_validation bench nifti-io --tier full --out docs/benchmarks
python -m larmorx_validation bench ants-apply-transforms --repeats 3 --threads 1 4 0 --out docs/benchmarks
python -m larmorx_validation bench afni-tshift --repeats 3 --threads 1 0 --out docs/benchmarks
python -m larmorx_validation bench resample-series --repeats 3 --threads 1 4 0 --out docs/benchmarks
python -m larmorx_validation bench ants-programs --repeats 5 --threads 1 4 0 --out docs/benchmarks
python -m larmorx_validation bench ants-gaussian --repeats 5 --threads 1 4 0 --out docs/benchmarks
python -m larmorx_validation bench ants-morphology --repeats 5 --threads 1 4 0 --out docs/benchmarks
python -m larmorx_validation bench ants-n4 --repeats 3 --threads 1 4 0 --out docs/benchmarks
python -m larmorx_validation bench ants-denoise --repeats 3 --threads 1 4 0 --out docs/benchmarks
python -m larmorx_validation bench mri-hmc --repeats 3 --threads 1 4 0 --out docs/benchmarks   # needs FSL
python -m larmorx_validation bench ants-ai --repeats 3 --threads 1 8 --out docs/benchmarks   # in a memory box
```
