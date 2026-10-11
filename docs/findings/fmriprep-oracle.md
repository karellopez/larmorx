# fMRIPrep 25.2.6 as it actually ran (the first oracle run)

What fMRIPrep 25.2.6 really did in the first oracle run, `ds000005-sub-01-noreconall` (ds000005
sub-01, `--fs-no-reconall`, `--nprocs 8 --omp-nthreads 8 --random-seed 0
--skull-strip-fixed-seed`; `scripts/run_fmriprep_oracle.sh`, decision log 2026-10-11), where it
differs from or adds to [the analysis](../analysis/01-fmriprep-bold-pipeline.md) of the
source. Everything here was read from the run's own records: nipype's working directory
(`work/.../command.txt`, `work/.../_report/report.rst`, below as `work/<node>`), the log
(`fmriprep.log`) and the image (`oracles/images/fmriprep-25.2.6.sif`, Apptainer). The
stage-by-stage comparison with larmorx is the record
[fmriprep-stages](../validation/fmriprep-stages.md); the harness that reads the working
directory is `python -m larmorx_validation pipeline-stages`.

Paths below are under `work/fmriprep_25_2_wf/sub_01_wf/`; `<run>` is
`bold_task_mixedgamblestask_run_01_wf` (runs 02 and 03 are the same).

## In short

- **fMRIPrep 25.2.6 ships ANTs 2.6.2, not 2.6.5**: `antsRegistration --version` in the image
  prints `2.6.2.dev1-gff61a3d` (conda-forge `ants` 2.6.2, build `h5f726e6_2`, on ITK 5.4.4).
  larmorx ports ANTs 2.6.5 on ITK 5.4.5, so "fMRIPrep's ANTs" and "the ANTs larmorx
  replicates" are different versions. FSL's `mcflirt` is the same release as FSL 6.0.7's
  (2111.0) on newer libraries; AFNI is 25.2.09, as the repository's oracle.
- **nipype hands ANTs its thread count as `NSLOTS`**, which ITK reads, not
  `ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS`. DenoiseImage, both brain-extraction N4s, antsAI,
  antsRegistration and Atropos ran with 8 threads; everything else with 1. So fMRIPrep's
  denoised T1w and its brain-extraction N4s are the 8-thread results, which ANTs does not
  reproduce at another thread count (DenoiseImage not even at the same one).
- **Every other step larmorx has is bit-identical to fMRIPrep as it ran**, as far as the
  steps' inputs and parameters go: the ImageMath, ThresholdImage and MultiplyImages steps, the
  one-shot resampling in all three spaces, and most antsApplyTransforms calls; see the
  record for the numbers.
- **Parameters differ from the analysis in two places**: the BOLD reference's antsAI uses
  `Affine[0.1]`, not `Rigid[0.1]`; and the BOLD reference's N4 runs with ANTs' default shrink
  factor (4), because `shrink_factor=2` is given to nipype's `Node`, not to the interface (as
  [02](../analysis/02-anatomical-smriprep-niworkflows.md) found, here confirmed).

## The tools inside the image

*Verified* (`apptainer exec` of the image; `conda-meta` of its pixi environment
`/app/.pixi/envs/fmriprep`):

| Tool | In fMRIPrep 25.2.6's image | The repository's oracle |
|---|---|---|
| ANTs | 2.6.2.dev1-gff61a3d, conda-forge `ants` 2.6.2 build `h5f726e6_2`, compiled 2025-07-31 | ANTs v2.6.5 (`oracles/ants-2.6.5`, `scripts/build_ants_oracle.sh`) |
| ITK | 5.4.4 (conda-forge `libitk` build `h8db0697_0`, shared libraries) | ITK v5.4.5, static, GCC 11.4 |
| FSL `mcflirt` | `fsl-mcflirt` 2111.0 build `h982b8fd_8`, `fsl-newimage` 2501.4, `fsl-miscmaths` 2412.4 | FSL 6.0.7.17: `fsl-mcflirt` 2111.0 build `hb6de94e_6`, `newimage` 2203.12, `miscmaths` 2203.2 |
| AFNI | 25.2.09 (`/usr/local/bin`) | 25.2.09 (`oracles/afni-25.2.09`) |
| Python stack | Python 3.12.11, nipype 1.12.0, niworkflows 1.15.2, smriprep 0.19.2, sdcflows 2.15.1, nitransforms 25.1.0, nibabel 5.3.2, numpy 2.2.6, SciPy 1.15.2 | (larmorx replicates nitransforms 25.1.0 and SciPy 1.15.2) |
| BLAS | OpenBLAS 0.3.30 (pthreads), Haswell kernels on this CPU (`threadpoolctl`) | larmorx reproduces the Haswell kernels' order |
| C library | glibc 2.35 (Ubuntu 22.04.5) | glibc 2.35 (the host, Ubuntu 22.04) |

So the same C library computes `exp` and `log` inside and outside the image, and fMRIPrep's
resampling code (`fmriprep/interfaces/resampling.py`, `fmriprep/utils/transforms.py`) is
byte-identical to the 26.0.0.dev source larmorx's `resample_series` replicates (*verified*:
the files from the image against the workspace's clone at `21a490fb`).

## Threads

**nipype passes `num_threads` to ANTs as `NSLOTS`** (`nipype/interfaces/ants/base.py:21-27`,
`PREFERED_ITKv4_THREAD_LIMIT_VARIABLE`), and the environment of every node has
`OMP_NUM_THREADS=1` and `MKL_NUM_THREADS=1` (fMRIPrep's config). ITK reads `NSLOTS`, then
`ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS`, the last one set winning
(`itkMultiThreaderBase.cxx:266-330`). *Read; verified* in the `_report/report.rst` of each node
(section "Environment"):

| Node | Program | `NSLOTS` | fMRIPrep's time (its run) |
|---|---|---|---|
| `anat_fit_wf/anat_template_wf/denoise` | DenoiseImage | **8** | 53.9 s |
| `anat_fit_wf/brain_extraction_wf/inu_n4`, `atropos_wf/inu_n4_final` | N4BiasFieldCorrection | **8** | 14.4 s, 9.5 s |
| `brain_extraction_wf/init_aff`, `<run>/.../init_aff` | antsAI | 8 | 18.4 s, 33 s |
| `brain_extraction_wf/norm`, `register_template_wf/.../registration` | antsRegistration (SyN) | 8 | 520 s, 2118 s |
| `atropos_wf/01_atropos` | Atropos | 8 | 12.0 s |
| `<run>/bold_fit_wf/enhance_and_skullstrip_bold_wf/n4_correct` | N4BiasFieldCorrection | **1** (`n_procs=1`) | 1.0 s |
| every ImageMath, ThresholdImage, MultiplyImages, antsApplyTransforms | | 1 (nipype's default) | |
| `<run>/.../boldref_bold`, `bold_anat_wf/resample`, `bold_std_wf/.../resample` | ResampleSeries (Python) | `num_threads` 8 | 12.5 s, 7.6 s, 72 s |
| `<run>/.../mri_coreg` | mri_coreg | `--threads 8` | 14-17 s |

- **The 8-thread results are fMRIPrep's results.** DenoiseImage and N4 depend on ANTs' thread
  count ([ants-denoise.md](ants-denoise.md), [ants-n4.md](ants-n4.md)), so fMRIPrep's
  denoised T1w and both brain-extraction N4s carry the 8-thread arithmetic; at another
  `--omp-nthreads`, or on a machine with fewer cores, they differ. larmorx gives ANTs'
  one-thread result on any number of threads. *Verified*: the record's chain for these
  stages.
- **`mcflirt` and `3dvolreg` are single-threaded; `OMP_NUM_THREADS=1` keeps AFNI's
  `3dUnifize` and `3dAutomask` on one thread too.**
- *larmorx:* its `larmorx ants` command lines read the thread count as ITK does, `NSLOTS`
  included (fixed 2026-10-11, `larmorx_ants::cli::itk_threads_from`); before, they read only
  `ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS`, so under nipype they would have used every CPU (the
  results are the same either way). `larmorx mri hmc` and `larmorx afni` read
  `OMP_NUM_THREADS`, which fMRIPrep sets to 1.
- *Correction (2026-10-11):* [ants-n4.md](ants-n4.md) said nipype's `num_threads` "becomes
  `ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS`"; it becomes `NSLOTS`, with the same effect.

## Seeds

- **`--random-seed 0` sets `ANTS_RANDOM_SEED=50495`** in every node's environment. fMRIPrep
  seeds Python's `random` with the master seed and draws ANTs' seed from it
  (`fmriprep/config.py:690-719`: `random.seed(0)`, then `random.randint(1, 65536)`); 0 is
  taken as a seed, not as "none". *Read; verified* (the reports' "Environment").
- **Atropos ran with `--use-random-seed 0`** (`atropos_wf/01_atropos/command.txt`), because of
  `--skull-strip-fixed-seed`. antsRegistration and antsAI take `ANTS_RANDOM_SEED` from the
  environment (Mattes' "Regular" sampling is jittered with it).

## Parameters nipype passes or drops

*Read* from each node's `command.txt` (the exact command nipype ran):

- **DenoiseImage**: `DenoiseImage -i <T1w> -n Rician -o <out> -s 1` (`anat_template_wf/denoise/
  mapflow/_denoise0`): no `-d`, no `-x`; it runs on the single T1w (there is no template to
  build with one image, and the pre-template N4 does not run).
- **N4 for the BOLD reference** (`<run>/bold_fit_wf/enhance_and_skullstrip_bold_wf/
  n4_correct`): `--bspline-fitting [ 200 ] -d 3 ... -r --weight-image <MNI prior in boldref>`,
  **no `--shrink-factor`**, so ANTs' default 4, not niworkflows' intended 2
  (`niworkflows/func/util.py:378-383` gives `shrink_factor=2` to `pe.Node`); `-r` without a
  mask. *Verified.*
- **Brain-extraction N4s**: `-s 4 -b [200] -c [50x50x50x50,1e-7]` on the truncated T1w, and
  `-c [50x50x50x50x50,1e-7] -r -w <WM weight> -o [corrected,bias]` on the **denoised, not
  truncated** T1w, as [02](../analysis/02-anatomical-smriprep-niworkflows.md) says.
- **antsAI for the BOLD reference**: `-m Mattes[<MNI boldref>,<HMC boldref>,32,Regular,0.2]
  -t Affine[0.1] -s [20,0.12] -g [40.0,0x40x40] -p 0 -c [10,1e-06,10] -x <MNI brain mask>`.
  [01](../analysis/01-fmriprep-bold-pipeline.md) says `Rigid[0.1]`; the code says
  `transform=('Affine', 0.1)` (`niworkflows/func/util.py:427-438`, niworkflows 1.15.2).
- **antsAI for brain extraction**: `Mattes[...,32,Regular,0.25] -t Affine[0.1] -s [15,0.1]
  -g [40.0,0x40x40] -p 0 -c [10,1e-06,10] -v 1`, as 02 says.
- **The BOLD reference's affine registration to MNI stops after 25 of its 200 iterations**
  (`<run>/.../norm`: `Affine[0.1]`, Mattes 64 bins, `Random 0.2`, convergence `[200, 1e-09,
  10]`, shrink 2, σ 2 mm): fMRIPrep's node took 0.4 s. *Verified* by re-running its
  `command.txt` in the image with `--verbose 1`: the conjugate-gradient line-search optimizer
  ANTs uses for linear stages (`itkantsRegistrationHelper.hxx:1363-1375`) ends after iteration
  25 with a convergence value of 1.9e-6, above the 1e-9 threshold. To be understood when A4
  ports it.
- **The MNI normalisation** is `t1w-mni_registration_precise_000.json` (`flavor: precise`,
  `explicit_masking: True`, `float: True`; the images are masked before the call and no
  masks are passed): Rigid and Affine with Mattes 56 bins `Regular 0.25`, SyN `[0.1,3,0]` with
  CC 4, winsorize `[0.005, 0.995]`, histogram matching, composite output. It took 2118 s on 8
  threads (shared machine), the longest node of the run.
- **No dummy volumes were found**, so the HMC reference averages volumes 20 to 39 (`t_mask`
  in `<run>/bold_fit_wf/hmc_boldref_wf/gen_avg`), aligned by `3dvolreg -Fourier -twopass
  -zpad 4` inside niworkflows' `RobustAverage`; that call is not a nipype node, so it has no
  `command.txt`.
- **No slice timing**: ds000005 has no `SliceTiming`, so there is no `bold_stc_wf`; and no
  `PhaseEncodingDirection` either ("Cannot determine world direction of phase encoding.
  Orientation: LAS; PE dir: None", three times in `fmriprep.log`).
- **`fsl2itk` warns "Multithreading is deprecated. Remove the num_threads input."** three times
  (`fmriprep.log`): fMRIPrep still passes `num_threads` to niworkflows' `MCFLIRT2ITK`.

## Headers and data types nipype rewrites

- **antsApplyTransforms' outputs are rewritten by niworkflows** (`FixHeaderApplyTransforms`,
  `niworkflows/interfaces/fixes.py:56-74`): after ANTs writes, nibabel copies the reference's
  sform and qform onto the output and sets `descrip` to "xform matrices modified by
  FixHeaderApplyTransforms (niworkflows v1.15.2)". The data are ANTs'. So a node's output
  header is not ANTs' header; the record compares data and affines.
- **N4 outputs get the input's header back** (nipype's `copy_header=True` for the BOLD
  reference); their `descrip` is nibabel's.
- **Masks in T1w and MNI space are written as float64 by antsApplyTransforms** (`--float 0`,
  `MultiLabel`), 8 bytes per voxel in the working directory; the DataSink writes the
  derivatives as `uint8`. Its log line says otherwise ("Changing
  .../space-T1w_desc-brain_mask.nii.gz dtype from float64 to float64"), and says "int16 to
  int16" for `desc-hmc_boldref`, which is written as float32. *Verified* (nibabel on the
  derivatives).
- **The derivative `desc-preproc_bold` files are the resampling nodes' outputs unchanged**
  (float32, same values; the record compares larmorx with both).
- **The T1w-space grid** is the T1w's field of view cropped to the brain mask, at the BOLD's
  voxel size (`bold_anat_wf/gen_ref`, `keep_native`): 50 × 54 × 39 voxels for run 01, sform and
  qform code 2.

## Reproducibility run to run

Each step was re-run in fMRIPrep's image with fMRIPrep's thread count (the record's
"fMRIPrep's tool re-run" column). The programs that ran single-threaded reproduce fMRIPrep's
outputs bit for bit, and so do `mcflirt` (all 240 matrices of each run, also FSL 6.0.7's
`mcflirt` on this machine, despite its older libraries) and `ResampleSeries`.
**DenoiseImage at 8 threads does not reproduce itself**; see the record for the N4s.
