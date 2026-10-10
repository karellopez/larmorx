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
| SciPy | BSD-3-Clause | `v1.15.2` → `0f1fd4a7268b` | 1.15.2 (fMRIPrep's lockfile, `pixi.lock`) | `scipy.ndimage` spline interpolation (`map_coordinates`, `spline_filter`) in `larmorx_interp::ndimage`; the development venv runs 1.15.3, whose `ndimage` interpolation code is identical (checked 2026-10-10) |
| nitransforms | MIT | `25.1.0` → `c10f63d1a1f7` | 25.1.0 (fMRIPrep's lockfile) | coordinate mapping, transform chains and ITK readers under fMRIPrep's one-shot resampler (`larmorx_transform::nitransforms`, `larmorx.transforms`); identical to the installed wheel |
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
| nibabel | MIT | 5.4.2 (development venv); fMRIPrep's image has 5.3.2 | NIfTI reading/writing semantics (`larmorx-io`), parity oracle |
| NIfTI-1/2 standard | public domain | `nifti1.h` / `nifti2.h` | file format |

## Clean-room restricted (never read for implementation)
`reference_src/workbench/` (GPL-2.0-or-later) and `reference_src/MSM_HOCR/` (non-commercial) are kept outside the working tree by `scripts/bootstrap_workspace.sh`, for licence review only. FSL is not cloned.
