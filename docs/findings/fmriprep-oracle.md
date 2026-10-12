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
- **fMRIPrep's one-shot resampler corrupts random volumes unless OpenBLAS runs one thread**,
  which only the image's `ENV OMP_NUM_THREADS=1` ensures (below). The oracle run is not
  affected.
- **fMRIPrep's ANTs computes N4 as the C++ source says; the repository's first ANTs oracle
  (GCC 11.4) did not, because of a GCC bug.** larmorx now follows the source and, with
  correctly rounded maths, is bit-identical to fMRIPrep's N4 binary in all 45 cases of the N4
  parity suite and in all six N4 outputs of this run (below); the oracle is rebuilt by a fixed
  compiler.
- **75 of the 92 nodes larmorx can run are bit-identical to fMRIPrep as it ran** (record
  regenerated 2026-10-12; 61 on 2026-10-11), on the steps' own inputs and parameters: every
  ImageMath, ThresholdImage and MultiplyImages step, the one-shot resampling in all three
  spaces, and every antsApplyTransforms call but the Gaussian ones of A06 and F10 (glibc's
  `exp`), the 14 with `--float 1` included (larmorx computes them in single precision as ANTs
  does since 2026-10-11). The other 17 are explained by the thread count and glibc's maths
  alone: with the GCC 14.3 oracle, the ANTs build no longer separates anything (the record).
  The field-map runs are below.
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
| ITK | 5.4.4 (conda-forge `libitk` build `h8db0697_0`, shared libraries; GCC 13.3, ANTs' N4 library GCC 14.3) | ITK v5.4.5, static, conda-forge GCC 14.3 (GCC 11.4 until 2026-10-11) |
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
`OMP_NUM_THREADS=1` and `MKL_NUM_THREADS=1` (from the image, `fmriprep/Dockerfile:90-91`, not
from fMRIPrep's code). ITK reads `NSLOTS`, then
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

## fMRIPrep's one-shot resampler is only correct with single-threaded OpenBLAS

**With `OMP_NUM_THREADS` above 1, fMRIPrep's `ResampleSeries` corrupts random volumes.** It
resamples the volumes concurrently (`asyncio` tasks run in the default thread pool,
`fmriprep/utils/asynctools.py:5-8`, at most `num_threads` = 8 at once,
`fmriprep/interfaces/resampling.py:396-425`), and each volume maps its coordinates through its
head-motion matrix with `nibabel.affines.apply_affine` (`resampling.py:292-297`),
a matrix product numpy hands to OpenBLAS. The image's OpenBLAS is the pthreads build
(0.3.30); called from several Python threads at once while it runs threads of its own, its
results go wrong. *Verified* on run 01's MNI resampling (`<run>/bold_std_wf/
_in_tuple_MNI152NLin2009cAsym.res2/resample`), re-run in fMRIPrep's image with the node's
inputs:

| `OMP_NUM_THREADS` | Against fMRIPrep's own output |
|---|---|
| 1 (as fMRIPrep ran) | bit-identical |
| 8, first run | 22,635 voxels wrong in 12 of 240 volumes (3, 6, 24, 80, ...), by up to 846 (the data reach 1,320) |
| 8, second run | 4,913 voxels wrong in 13 other volumes (1-7, 100, ...), by up to 785 |

- Only the image's environment prevents it: `ENV OMP_NUM_THREADS=1` (`fmriprep/Dockerfile:90-91`;
  in the Apptainer image `/.singularity.d/env/10-docker2singularity.sh:30` sets
  `OMP_NUM_THREADS=${OMP_NUM_THREADS:-1}`). fMRIPrep's Python code does not set it. So the
  corruption reaches fMRIPrep's derivatives when fMRIPrep runs outside its image (a pip or
  pixi install) or in Apptainer without `--cleanenv` on a machine where `OMP_NUM_THREADS` is set,
  and nothing reports it. The oracle run used `--cleanenv` (`OMP_NUM_THREADS=1` in every node's
  report), so its outputs are not affected.
- *larmorx:* `lx.transforms.resample_series` does these products in Rust, in a fixed order,
  and its result does not depend on any thread count; it is bit-identical to fMRIPrep's output
  in all nine resamplings of the oracle run. The record's re-run of fMRIPrep's resampler sets
  `OMP_NUM_THREADS=1`, as fMRIPrep's image does.

## fMRIPrep's ANTs computes N4 as the C++ source says; the first repository oracle did not

**fMRIPrep's N4 binary (ANTs 2.6.2, conda-forge) and the repository's first ANTs 2.6.5 oracle
(GCC 11.4, `-O3`) gave different N4 results on the same input at one thread, even with
correctly rounded maths in both, and the whole difference was the oracle's: GCC 11.4
miscompiles two float roundings of ITK's B-spline fit (GCC bug 110182,
[ants-n4.md](ants-n4.md), "GCC 11 miscompiles the fit").** larmorx reproduced that build at
first; since 2026-10-11 it follows the source, and the oracle is rebuilt by conda-forge's GCC
14.3 (the compiler of fMRIPrep's ANTs).

- *Verified* on the BOLD reference's N4 of run 03 (`<run>/.../n4_correct`, one thread):
  fMRIPrep's binary in its image with CORE-MATH preloaded against the GCC 11 oracle with
  CORE-MATH preloaded: 137,675 of 139,264 voxels differ, by at most 0.152 (1 − NCC 7.8e-10);
  as built (glibc's maths in both), 138,068 voxels, at most 1.44 (1 − NCC 7e-8).
- *Verified:* larmorx with the source's roundings is **bit-identical to fMRIPrep's binary
  with CORE-MATH preloaded (one thread) in all six N4 outputs of the run**: the three BOLD
  references, the brain extraction's first N4 and its final N4 (corrected image and bias
  field). (First found with a scratch build; it is larmorx's default now.)
- *Validated* (2026-10-11) on every case of the N4 parity suite (standard tier), with
  fMRIPrep's binary at one thread in place of the oracle: **45 of 45 cases (54 of 54 outputs)
  bit-identical to larmorx with CORE-MATH preloaded**; as built, 29 of 45 (35 of 54), every
  remaining difference the same, case by case, as the new oracle's as built (glibc's `expf`,
  `logf`, `exp`; [ants-n4](../validation/ants-n4.md)). The binary ran through Apptainer
  wrappers given to the suite's cases as `LARMORX_ANTS_BIN`, with the suite's
  `ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS` passed in and `libcrmath.so` bound into the container
  and preloaded there.
- So the "version" link of the record's chain for N4 was not a change between ANTs 2.6.2 and
  2.6.5: it was one compiler's bug. The [stage record](../validation/fmriprep-stages.md)
  predates the change (its N4 rows compare the old larmorx and the GCC 11 oracle); it is to be
  regenerated.
- DenoiseImage has no such difference: on every case of the DenoiseImage suite (standard
  tier), fMRIPrep's binary at one thread is bit-identical to larmorx in 40 of 40 cases (52 of
  52 outputs) with CORE-MATH and in 7 of 40 as built, the same numbers as the oracle's
  ([ants-denoise](../validation/ants-denoise.md)). *Validated* (2026-10-11).

## Reproducibility run to run

Each step was re-run in fMRIPrep's image with fMRIPrep's thread count (the record's
"fMRIPrep's tool re-run" column). *Verified:*

- Every program that ran single-threaded reproduces fMRIPrep's outputs bit for bit, and so do
  the two brain-extraction N4s at 8 threads (ITK's per-thread lattices are summed in a fixed
  order, so N4 depends on the thread count but not on the run).
- `mcflirt` reproduces all 240 matrices of each run, and so does FSL 6.0.7's `mcflirt` on
  this machine, despite its older `newimage` and `miscmaths`.
- `ResampleSeries` reproduces its output with `OMP_NUM_THREADS=1` (and does not with more,
  above).
- **DenoiseImage at 8 threads does not reproduce itself**: two re-runs differ from fMRIPrep's
  output in 163,367 and 346,866 of 5,898,240 voxels, by up to 72.6 (1 − NCC 1.1e-6 and
  1.9e-6). At one thread,
  fMRIPrep's binary and the ANTs 2.6.5 oracle agree bit for bit; fMRIPrep's 8-thread result
  differs from that in 4,900,198 voxels, by up to 271 (1 − NCC 1.1e-4), which is what separates
  larmorx (ANTs' one-thread result) from fMRIPrep's denoised T1w.

## The field-map runs (2026-10-12)

Three more oracle runs with the same image and options (`scripts/run_fmriprep_oracle.sh`,
`--fs-no-reconall`), compared stage by stage with the same harness:
[ds005040](../validation/fmriprep-stages-ds005040.md) sub-002 (one T1w; one BOLD run of 63
volumes, 3×3×3.3 mm, slice timing; a PEPOLAR pair estimated with `topup`),
[ds005454](../validation/fmriprep-stages-ds005454.md) sub-16 (two T1w, so the anatomical
template with `mri_robust_template` and the pre-template N4; one oblique BOLD run of 280
volumes at 1.4 mm, multiband with repeated slice times; a `phasediff` field map, unwrapped by
`prelude`) and [ds005752](../validation/fmriprep-stages-ds005752.md) sub-ON01016 (one T1w; BOLD
runs of 200 and 10 volumes with slice timing; a direct `fieldmap` with its magnitude).
Paths below are under each run's `work/fmriprep_25_2_wf/sub_<label>_wf/`. *Verified* from the
working directories, the logs and the stage comparisons:

- **ds005752's field map is not used.** Neither the field map's sidecar (`IntendedFor`,
  `B0FieldIdentifier`) nor the BOLD runs' (`B0FieldSource`) say which BOLD it corrects.
  sdcflows finds the estimator (`sdcflows/utils/wrangler.py:389-411`, 2.15.1), but fMRIPrep
  keeps only estimators a BOLD run maps to (`fmriprep/workflows/base.py:995-1021`,
  `get_estimator` at 1034-1050) and drops it; the log says only "No fieldmap correction -
  skipping Stage 3". So no oracle run covers the direct-`fieldmap` path yet; a copy of ds005752
  with `IntendedFor` (or `B0FieldIdentifier`/`B0FieldSource`) added would.
- **The Jacobian is applied for PEPOLAR field maps only** (`fmriprep/workflows/base.py:853-858`):
  ds005040's `ResampleSeries` nodes have `jacobian : True`, ds005454's (`phasediff`) `False`.
- **Slice timing** runs as `bold_native_wf/bold_stc_wf/slice_timing_correction`
  (`fmriprep/workflows/bold/stc.py:121-131`): `3dTshift -ignore 0 -prefix <name>_tshift.nii.gz
  -tpattern @slice_timing.1D -TR <TR>s -tzero <t0> <BOLD>` with `OMP_NUM_THREADS=1`, `t0` the
  middle of the slice times rounded to 3 decimals (`--slice-time-ref` 0.5,
  `fmriprep/workflows/bold/stc.py:107`), then niworkflows' `CopyXForm` restores the input's
  header. nipype's
  `TShift` writes `slice_timing.1D` in the node directory from the `slice_timing` input (the
  times joined by tabs as Python prints them, reversed for a negative slice-encoding
  direction; `nipype/interfaces/afni/preprocess.py:3290-3298`, 1.12.0) and links the input
  there (`copyfile=False`); both are removed after the run, so only `command.txt`, the report
  and the hash file `_0x<hash>.json` (which keeps the input's original path) remain. AFNI keeps
  the int16 input's type, scaled by a factor of its own when the shifted values need it
  (ds005040: `scl_slope` 47.67; ds005454 and ds005752: unscaled), and writes its history
  extension.
- **The 3dTshift replica reproduces all four slice-timing outputs bit for bit** (data and
  header, `vox_offset` aside: larmorx writes no history extension), as does the repository's
  AFNI 25.2.09 build; fMRIPrep's AFNI reproduces itself. The clean-room original differs by one
  stored unit in 189 of 10,321,920 values (ds005040), 1,808 of 688,128,000 (ds005454), 86 of
  39,321,600 and 1 of 1,966,080 (ds005752), within [afni-tshift.md](../validation/afni-tshift.md)'s
  thresholds. None of AFNI's defects is met: larmorx's default gives the same files.
- **nipype re-quantizes the field-map reference's N4 output** (PIPE-16): sdcflows clips the
  reference to int16 (`IntensityClip`, `niworkflows/interfaces/nibabel.py:667-723`) and runs N4
  with `copy_header=True`; nipype's N4 interface then writes the corrected image in the
  input's type (`_copy_header_map`, `keep_dtype=False`), so fMRIPrep's node output is N4's
  float32 result rounded to int16 with a scale factor (ds005040: slope 0.0032, inter 105.19).
  Re-run in fMRIPrep's image at the node's 8 threads (serially and six at once), N4 gives the
  same bits every time, and after nipype's own `copy_header` exactly fMRIPrep's output.
- **fMRIPrep's `space-T1w` grid is at the BOLD's voxel size** in all runs (`gen_ref`,
  `keep_native`), so PIPE-03 matters there only through the angle between the BOLD's and the
  T1w's axes: little for ds005040 (axial BOLD), much for ds005454 (its phase-encoding axis 22.5°
  from the scanner's anterior axis).
- **Head motion.** larmorx's matrices stay within 0.003 mm / 0.005° (median) of mcflirt's in
  every run. In ds005752's 200-volume run (largest FD 0.56 mm) the FD correlation is 0.974,
  below PLAN.md's 0.99 but within mcflirt's own band on that run (0.961 to 0.976 under ±0.5
  noise and reversed volumes, measured by the harness); ds005040 (largest FD 0.17 mm) gives
  0.77, where the correlation is not required (mcflirt's testdata band for the dataset 0.82).
