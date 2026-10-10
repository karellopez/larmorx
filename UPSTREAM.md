# Upstream versions

Exact upstream releases that larmorx ports from or is validated against. Ported code is taken from the **release tags** below, never from the master/dev snapshots in [upstream.tsv](upstream.tsv) (those are reading references only). Each crate's `PROVENANCE.md` lists the files it was ported from.

Checked 2026-10-09 with `git ls-remote --tags` and fMRIPrep's lockfile (`fmriprep/pixi.lock` at `21a490fb`).

## Ported sources

| Project | Licence | Port from (tag → commit) | Version in fMRIPrep's image | Notes |
|---|---|---|---|---|
| ANTs | Apache-2.0 | `v2.6.5` → `fdce4d2f84b6` | 2.6.2 (conda-forge) | newest 2.6.x |
| ITK | Apache-2.0 | `v5.4.5` → `f51594ad8819` | libitk 5.4.4 (conda-forge) | the version ANTs 2.6.5's build pins (2.6.0–2.6.3 pin `v5.4.3`) |
| AFNI | public domain (NIH) + GPL-2 (code copyrighted by the Medical College of Wisconsin) | `AFNI_25.2.09` → `b1e12b26dae2` | AFNI_25.2.09 | matches fMRIPrep. MCW-copyrighted (GPL-2) files are read and re-implemented originally, never translated (CLAUDE.md rule 1); the tag is also the source of the AFNI oracle (`scripts/build_afni_oracle.sh`) |
| FreeSurfer | FreeSurfer Software License v1.0 | `v8.2.0` → `d932c45b7941` (pending D5) | 7.3.2 | fMRIPrep runs 7.3.2; D5 proposes porting 8.2.0 and validating against 7.4.x |
| CORE-MATH | MIT | `master` → `040ee482a8ca` (no releases; 2026-10-09) | – (fMRIPrep's tools use glibc's libm) | correctly rounded `exp`, `log`, `sin`, `cos` in `larmorx_core::math` (user decision 2026-10-09) |

**ANTs/ITK patch level (decided 2026-10-09: ANTs v2.6.5 + ITK v5.4.5).** CLAUDE.md first said "ITK v5.4.7 (pinned by ANTs 2.6)", but no ANTs 2.6.x release pins 5.4.7, and fMRIPrep's image runs ANTs 2.6.2 linked against ITK 5.4.4. The candidates were:

| ANTs | ITK | Why |
|---|---|---|
| `v2.6.2` → `52bc0ab58810` | `v5.4.4` → `f98d5fac5e1d` | exactly what fMRIPrep runs: the closest match to the fMRIPrep oracles |
| `v2.6.5` → `fdce4d2f84b6` | `v5.4.5` → `f51594ad8819` | newest ANTs 2.6.x with the ITK it pins (**chosen**) |

Validation against fMRIPrep's outputs must allow for the patch-level difference (ANTs 2.6.2 + ITK 5.4.4 in fMRIPrep's image); the ANTsPy oracle is checked for the version it embeds.

## Behaviour references (not ported line by line)

| Project | Licence | Version | Used for |
|---|---|---|---|
| nibabel | MIT | 5.4.2 (development venv); fMRIPrep's image has 5.3.2 | NIfTI reading/writing semantics (`larmorx-io`), parity oracle |
| NIfTI-1/2 standard | public domain | `nifti1.h` / `nifti2.h` | file format |

## FSL (oracle, spec source, future `larmorx-nc` replica)

FSL is not cloned. The oracle and the source read for `specs/mcflirt.md` are the FSL 6.0.7.17
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

Full SHA-256 values are in `oracles/fsl-6.0.7/mcflirt/manifest.json`. fMRIPrep's published
ds000005 derivatives (21.0.1) were made with FSL 6.0.5.1; fMRIPrep's current image installs FSL
from conda as well. FSL source and notes derived from it are forbidden to clean-room
implementers (`docs/licensing.md`).

## Clean-room restricted (never read for implementation)
`reference_src/workbench/` (GPL-2.0-or-later) and `reference_src/MSM_HOCR/` (non-commercial) are kept outside the working tree by `scripts/bootstrap_workspace.sh`, for licence review only. FSL is not cloned.
