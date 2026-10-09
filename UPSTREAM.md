# Upstream versions

Exact upstream releases that larmorx ports from or is validated against. Ported code is taken from the **release tags** below, never from the master/dev snapshots in [upstream.tsv](upstream.tsv) (those are reading references only). Each crate's `PROVENANCE.md` lists the files it was ported from.

Checked 2026-10-09 with `git ls-remote --tags` and fMRIPrep's lockfile (`fmriprep/pixi.lock` at `21a490fb`).

## Ported sources

| Project | Licence | Port from (tag → commit) | Version in fMRIPrep's image | Notes |
|---|---|---|---|---|
| ANTs | Apache-2.0 | `v2.6.x`: patch release to be decided (see below) | 2.6.2 (conda-forge) | latest 2.6.x is `v2.6.5` → `fdce4d2f84b6` |
| ITK | Apache-2.0 | `v5.4.7` → `4c49df934c31` (current choice, see below) | libitk 5.4.4 (conda-forge) | ANTs pins ITK `v5.4.3` (2.6.0–2.6.3) and `v5.4.5` (2.6.4–2.6.5) |
| AFNI | public domain (+ per-file exceptions) | `AFNI_25.2.09` → `b1e12b26dae2` | AFNI_25.2.09 | matches fMRIPrep |
| FreeSurfer | FreeSurfer Software License v1.0 | `v8.2.0` → `d932c45b7941` (pending D5) | 7.3.2 | fMRIPrep runs 7.3.2; D5 proposes porting 8.2.0 and validating against 7.4.x |

**Open question (ANTs/ITK patch level).** CLAUDE.md says "ITK v5.4.7 (pinned by ANTs 2.6)", but no ANTs 2.6.x release pins 5.4.7, and fMRIPrep's image runs ANTs 2.6.2 linked against ITK 5.4.4. The candidates:

| ANTs | ITK | Why |
|---|---|---|
| `v2.6.2` → `52bc0ab58810` | `v5.4.4` → `f98d5fac5e1d` | exactly what fMRIPrep runs: the closest match to the fMRIPrep oracles |
| `v2.6.5` → `fdce4d2f84b6` | `v5.4.5` → `f51594ad8819` | newest ANTs 2.6.x with the ITK it pins |

## Behaviour references (not ported line by line)

| Project | Licence | Version | Used for |
|---|---|---|---|
| nibabel | MIT | 5.4.2 (development venv); fMRIPrep's image has 5.3.2 | NIfTI reading/writing semantics (`larmorx-io`), parity oracle |
| NIfTI-1/2 standard | public domain | `nifti1.h` / `nifti2.h` | file format |

## Clean-room restricted (never read for implementation)
`reference_src/workbench/` (GPL-2.0-or-later) and `reference_src/MSM_HOCR/` (non-commercial) are kept outside the working tree by `scripts/bootstrap_workspace.sh`, for licence review only. FSL is not cloned.
