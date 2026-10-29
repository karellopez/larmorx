# Upstream versions

Exact upstream releases that larmorx ports from or is validated against. Ported code is taken from the **release tags** below, never from the master/dev snapshots in [upstream.tsv](upstream.tsv) (those are reading references only). Each crate's `PROVENANCE.md` lists the files it was ported from.

Checked 2026-10-09 with `git ls-remote --tags` and fMRIPrep's lockfile (`fmriprep/pixi.lock` at `21a490fb`).

**Policy from 2026-10-27 (user):** the newest upstream releases, brought in by planned update parts. On 2026-10-27 ANTs 2.6.5 and FreeSurfer 8.2.0 were the newest; AFNI moved to 26.3.05 on 2026-10-29 (part 1 of the version update); FSL moved to 6.0.7.23 on 2026-10-29 (part 2: the replicas; the clean-room head-motion correction stays validated against 6.0.7.17 until a clean-room agent aligns it with FSL-59's behaviour) (`docs/decision-log.md`, 2026-10-27). The tables below name the versions the current records were made with.

## Ported sources

| Project | Licence | Port from (tag → commit) | Version in fMRIPrep's image | Notes |
|---|---|---|---|---|
| ANTs | Apache-2.0 | `v2.6.5` → `fdce4d2f84b6` | 2.6.2 (conda-forge) | newest 2.6.x |
| ITK | Apache-2.0 | `v5.4.5` → `f51594ad8819` | libitk 5.4.4 (conda-forge) | the version ANTs 2.6.5's build pins (2.6.0–2.6.3 pin `v5.4.3`) |
| ITK remote module AdaptiveDenoising | Apache-2.0 | commit `24825c8d2469` (`tags/ITKAdaptiveDenoising`) | – | the commit ITK v5.4.5 fetches (`Modules/Remote/AdaptiveDenoising.remote.cmake`) when ANTs' build enables `Module_AdaptiveDenoising`; `DenoiseImage`'s adaptive non-local means, local variance and patch-based base filter (`larmorx_image::adaptive_denoising`, `neighborhood`). The ANTs oracle's ITK build fetched the same commit into `tags/ITK-v5.4.5/Modules/Remote/AdaptiveDenoising` (identical files) |
| AFNI | public domain (NIH) + CC BY 4.0 (code copyrighted by the Medical College of Wisconsin; GPL-2 up to 25.2.09) | `AFNI_26.3.05` → `07c1dcfb5fdc` (since 2026-10-29; before: `AFNI_25.2.09` → `b1e12b26dae2`, kept in `tags/AFNI-25.2.09` and `oracles/afni-25.2.09`) | AFNI_25.2.09 | the GPL replicas (`crates-gpl/`, GPL-3.0-or-later) follow 26.3.05; the clean-room originals follow 26.3.05 too since their alignment the same day (AFNI-03 and AFNI-05 as 26.3.05 in both modes; records against 26.3.05). The MCW files' relicensing (CC BY 4.0) is inventoried in `docs/findings/afni-relicensing-26.md` (the move into the main package decided by the user on 2026-10-29, next-steps item 3c). The tags are also the sources of the AFNI oracles (`scripts/build_afni_oracle.sh`) |
| FreeSurfer | FreeSurfer Software License v1.0 | `v8.2.0` → `d932c45b7941` (pending D5) | 7.3.2 | fMRIPrep runs 7.3.2; D5 proposes porting 8.2.0 and validating against 7.4.x |
| CORE-MATH | MIT | `master` → `040ee482a8ca` (no releases; 2026-10-09) | – (fMRIPrep's tools use glibc's libm) | correctly rounded `exp`, `log`, `sin`, `cos`, `asin`, `acos`, `atan2`, `cbrt`, `powf`, `expf`, `logf`, `sinf`, `cosf`, `tanhf`, `cbrtf` in `larmorx_core::math` (user decision 2026-10-09; `tanhf`, `cbrtf` 2026-10-23 for 3dUnifize) |
| GNU C Library | LGPL-2.1-or-later | `glibc-2.35` release tarball, SHA-256 `5123732f6b67` (tag `glibc-2.35` → `f94f6d8a3572`) | – (AFNI 25.2.09 runs with Ubuntu 22.04's glibc 2.35) | `sinf`/`cosf` (x86-64 FMA build), ported in `crates-gpl/larmorx-gpl-afni` so that the 3dTshift replica's weighted sinc matches AFNI on every platform; LGPL-2.1 §3 allows the GPL-3 package to include it |
| SciPy | BSD-3-Clause | `v1.15.2` → `0f1fd4a7268b` | 1.15.2 (fMRIPrep's lockfile, `pixi.lock`) | `scipy.ndimage` spline interpolation (`map_coordinates`, `spline_filter`) in `larmorx_interp::ndimage`; the development venv runs 1.15.3, whose `ndimage` interpolation code is identical (checked 2026-10-10) |
| nitransforms | MIT | `25.1.0` → `c10f63d1a1f7` | 25.1.0 (fMRIPrep's lockfile) | coordinate mapping, transform chains and ITK readers under fMRIPrep's one-shot resampler (`larmorx_transform::nitransforms`, `larmorx.transforms`); identical to the installed wheel |
| OpenBLAS | BSD-3-Clause | `v0.3.30` release tarball, SHA-256 `27342cff5186` (tag `v0.3.30` → `993fad6aebbc`) | – (fMRIPrep's numpy calls the OpenBLAS its wheel bundles; numpy 2.3.5's is scipy-openblas64 0.3.30) | the operation order of the Haswell kernels behind numpy's 4×4 `@`/`dot` (`dgemm`, `dgemv`) and `np.linalg.inv` (`dgesv`: `getf2_k`, `getrs`, the triangular solves, `idamax`), reproduced in `larmorx_transform::openblas` so that `lx.transforms` gives the same bits on every platform (docs/findings/numpy-blas.md) |
| fMRIPrep | Apache-2.0 | `master` → `21a490fb89ac` (the workspace clone; the venv runs it as 26.0.0.dev1+g21a490fb8) | – | the one-shot BOLD resampler (`fmriprep/interfaces/resampling.py`, `utils/transforms.py`) in `larmorx_transform::resample_series` and `larmorx.transforms.resample_series` |

**ANTs/ITK patch level (decided 2026-10-09: ANTs v2.6.5 + ITK v5.4.5).** CLAUDE.md first said "ITK v5.4.7 (pinned by ANTs 2.6)", but no ANTs 2.6.x release pins 5.4.7, and fMRIPrep's image runs ANTs 2.6.2 linked against ITK 5.4.4. The candidates were:

| ANTs | ITK | Why |
|---|---|---|
| `v2.6.2` → `52bc0ab58810` | `v5.4.4` → `f98d5fac5e1d` | exactly what fMRIPrep runs: the closest match to the fMRIPrep oracles |
| `v2.6.5` → `fdce4d2f84b6` | `v5.4.5` → `f51594ad8819` | newest ANTs 2.6.x with the ITK it pins (**chosen**) |

Validation against fMRIPrep's outputs must allow for the patch-level difference (ANTs 2.6.2 + ITK 5.4.4 in fMRIPrep's image); the ANTsPy oracle is checked for the version it embeds.

## Behaviour references (not ported line by line)

| Project | Licence | Version | Used for |
|---|---|---|---|
| numpy | BSD-3-Clause | 2.3.5 (development venv; sdist SHA-256 `784db1dcdab5`) | which BLAS and LAPACK routine each 4×4 operation calls and with which memory layout (`numpy/_core/src/umath/matmul.c.src`, `_core/src/common/cblasfuncs.c`, `linalg/umath_linalg.cpp`); read, not translated |
| nibabel | MIT | 5.4.2 (development venv); fMRIPrep's image has 5.3.2 | NIfTI reading/writing semantics (`larmorx-io`), parity oracle |
| NIfTI-1/2 standard | public domain | `nifti1.h` / `nifti2.h` | file format |

## FSL (oracle, spec source, the `larmorx-nc` replicas)

FSL is not cloned. **Since 2026-10-29 the replicas follow FSL 6.0.7.23** (`~/fsl-6.0.7.23`, FSL's official installation; table below); the clean-room head-motion correction follows 6.0.7.23 too since its alignment the same day (record against 6.0.7.23); 6.0.7.17 stays in `~/fsl` for the step between the two and the earlier records. The oracle and the source read for `specs/mcflirt.md` are the FSL 6.0.7.17
conda installation in `~/fsl` (Linux x86-64), whose packages ship their source in
`~/fsl/src/<package>/`. Versions from `~/fsl/conda-meta/*.json` and each package's
`metadata_conda_debug.yaml` (checked 2026-10-10):

| Package | Licence | Version (tag on git.fmrib.ox.ac.uk) | Conda build | Package SHA-256 | Used for |
|---|---|---|---|---|---|
| fsl-mcflirt | FSL Licence | `2111.0` | `hb6de94e_6` | `7f4bb0e9e0e3…71a44c203` | mcflirt (`lx.mri.hmc`) |
| fsl-newimage | FSL Licence | `2203.12` | `h489b204_0` | `4839b9d79fa2…a2dca6da` | cost functions, resampling, NIfTI I/O |
| fsl-miscmaths | FSL Licence | `2203.2` | `hb6de94e_5` | `6d19870685ff…fd9538d1` | optimiser, Euler maths, kernels, splines |
| fsl-flirt | FSL Licence | `2111.2` | `hb6de94e_3` | `adcb70044a0f…88e9d7578` | (flirt: same cost library) |
| fsl-newnifti | FSL Licence | `4.1.0` | `hdef71a4_5` | `2a114eda0336…0fa2bf20` | NIfTI header reading |
| fsl-utils | FSL Licence | `2203.5` | `hb6de94e_0` | `99d3cc616a95…627c32f2` | start-up (threading) |
| fsl-znzlib | FSL Licence | `2111.0` | `hdef71a4_8` | `630acf92423d…eef379e0` | gzip I/O |
| fsl-armawrap | Apache-2.0 | `0.6.0` | `hdef71a4_5` | `40804c88739f…6292c69c` | matrix printing (`.mat` format) |

FSL 6.0.7.23 (`~/fsl-6.0.7.23/conda-meta/`, checked 2026-10-29), the source of the replicas since
2026-10-29 (`crates-nc/fsl-source/MANIFEST.tsv`):

| Package | Licence | Version | Conda build | Package SHA-256 | Used for |
|---|---|---|---|---|---|
| fsl-mcflirt | FSL Licence | `2111.0` | `h465c9ab_11` | `dadac722951f…5c72926d8` | mcflirt (unchanged source, rebuilt) |
| fsl-newimage | FSL Licence | `2601.0` | `h541c7df_0` | `7e27e07cc37d…1f4ac3c99` | cost functions, resampling, NIfTI I/O |
| fsl-miscmaths | FSL Licence | `2412.6` | `hd7ab901_3` | `36b7a485161d…7f5f6f449` | optimiser, Euler maths, kernels, splines |
| fsl-flirt | FSL Licence | `2111.4` | `h28ab0de_3` | `f12fcd76e08d…28f9bc809` | (flirt: not ported yet) |
| fsl-newnifti | FSL Licence | `5.0.0` | `h56e42e0_0` | `b6b5305dfed3…b8f2442ad` | NIfTI I/O (FSL-60) |
| fsl-utils | FSL Licence | `2412.6` | `hdf63e7b_1` | `b6616878217f…01c93b9e0` | start-up, usage ID |
| fsl-znzlib | FSL Licence | `2511.3` | `hcc786c9_101_zlib` | `9722b02e530a…5ded44334` | gzip, zstd and bzip2 I/O |
| fsl-armawrap | Apache-2.0 | `0.7.0` (Armadillo 14.2.1) | `h2bc3f7f_3` | `ff91e76239af…a4c853148` | matrix printing; 4 × 4 inverses through LAPACK (not followed yet) |
| fsl-bet2 | FSL Licence | `2111.9` | `hcdb67c3_3` | `6ee4925e2b98…97365e40a` | bet2, betsurf, the `bet` script |
| fsl-meshclass | FSL Licence | `2111.0` | `h8c873e0_12` | `c5e71e8f99e6…8079a54f1` | BET's mesh |
| fsl-avwutils | FSL Licence | `2209.6` | `h6b316f4_0` | `a8ce7e16c92b…09d7cd9db` | `fslstats`, `fslmaths` in `bet` |
| fslpy | Apache-2.0 | `3.27.0` | `pyhd8ed1ab_0` | `637448124c0b…9bf7e1776` | `imtest`, `remove_ext`, `imglob` |
| libopenblas | BSD-3-Clause | `0.3.33` | `pthreads_h94d23a6_0` | `3d9aa85648e5…ccedc8903` | Armadillo 14.2.1's LAPACK |

Full SHA-256 values are in `oracles/fsl-6.0.7/mcflirt/manifest.json`. fMRIPrep's published
ds000005 derivatives (21.0.1) were made with FSL 6.0.5.1; fMRIPrep's current image installs FSL
from conda as well. FSL source and notes derived from it are forbidden to clean-room
implementers (`docs/licensing.md`).

## Clean-room restricted (never read for implementation)
`reference_src/workbench/` (GPL-2.0-or-later) and `reference_src/MSM_HOCR/` (non-commercial) are kept outside the working tree by `scripts/bootstrap_workspace.sh`, for licence review only. FSL is not cloned.
