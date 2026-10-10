# Upstream versions

Exact upstream releases that larmorx ports from or is validated against. Ported code is taken from the **release tags** below, never from the master/dev snapshots in [upstream.tsv](upstream.tsv) (those are reading references only). Each crate's `PROVENANCE.md` lists the files it was ported from.

Checked 2026-10-09 with `git ls-remote --tags` and fMRIPrep's lockfile (`fmriprep/pixi.lock` at `21a490fb`).

## Ported sources

| Project | Licence | Port from (tag → commit) | Version in fMRIPrep's image | Notes |
|---|---|---|---|---|
| ANTs | Apache-2.0 | `v2.6.5` → `fdce4d2f84b6` | 2.6.2 (conda-forge) | newest 2.6.x |
| ITK | Apache-2.0 | `v5.4.5` → `f51594ad8819` | libitk 5.4.4 (conda-forge) | the version ANTs 2.6.5's build pins (2.6.0–2.6.3 pin `v5.4.3`) |
| AFNI | public domain (NIH) + GPL-2 (code copyrighted by the Medical College of Wisconsin) | `AFNI_25.2.09` → `b1e12b26dae2` | AFNI_25.2.09 | matches fMRIPrep. MCW-copyrighted (GPL-2-or-later) files are translated only in the GPL replica package (`crates-gpl/`, GPL-3.0-or-later); the Apache clean-room originals never use them (CLAUDE.md rule 1, `docs/licensing.md`). The tag is also the source of the AFNI oracle (`scripts/build_afni_oracle.sh`) |
| FreeSurfer | FreeSurfer Software License v1.0 | `v8.2.0` → `d932c45b7941` (pending D5) | 7.3.2 | fMRIPrep runs 7.3.2; D5 proposes porting 8.2.0 and validating against 7.4.x |
| CORE-MATH | MIT | `master` → `040ee482a8ca` (no releases; 2026-10-09) | – (fMRIPrep's tools use glibc's libm) | correctly rounded `exp`, `log`, `sin`, `cos` in `larmorx_core::math` (user decision 2026-10-09) |
| GNU C Library | LGPL-2.1-or-later | `glibc-2.35` release tarball, SHA-256 `5123732f6b67` (tag `glibc-2.35` → `f94f6d8a3572`) | – (AFNI 25.2.09 runs with Ubuntu 22.04's glibc 2.35) | `sinf`/`cosf` (x86-64 FMA build), ported in `crates-gpl/larmorx-gpl-afni` so that the 3dTshift replica's weighted sinc matches AFNI on every platform; LGPL-2.1 §3 allows the GPL-3 package to include it |

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
