# Provenance: larmorx-freesurfer-io

**Replica: translated from FreeSurfer's source.** This crate holds `fs.io` (3.1-25 to 3.1-32) and `fs.labels` (3.1-33, 3.1-34) of the FreeSurfer
replica (`docs/freesurfer/waves.md` section 4): the volume formats (`MRIread`/`MRIwrite`: MGH/MGZ, NIfTI, ANALYZE, the format probe, the other readers; FreeSurfer's tags, `.mat`, `.npy`, GIFTI volumes), FreeSurfer's changes to MINC's files, and colour tables, annotations, labels, `register.dat`, control points, point sets and CSV (`fs.labels`). It is distributed under FreeSurfer's
licence (`LicenseRef-FreeSurfer`, `../LICENSE`: the FreeSurfer Software License with its required
preface), as part of the `larmorx-freesurfer` package (`docs/licensing.md`).

The upstream is FreeSurfer 8.2.0 (GitHub's `v8.2.0` archive, SHA-256
`bb1699f7655ebf0f3790db6a78af9532ea7c832c97fe5f71bfd9f0e9526d7e78`; `docs/freesurfer-plan.md`),
built for the official binaries with GCC 8.4.0 (`docs/freesurfer/variation.md`).

## FreeSurfer files and functions translated

None yet: the crate was created empty by queue item 3.1-01 (`docs/freesurfer/queue.md`). Each
item that brings code adds its files and functions here, with the third-party code it found in
them (CLAUDE.md rule 3: each ported file is audited for third-party code first).

## Changes from the originals

None yet.
