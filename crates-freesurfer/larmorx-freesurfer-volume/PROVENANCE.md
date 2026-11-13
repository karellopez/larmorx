# Provenance: larmorx-freesurfer-volume

**Replica: translated from FreeSurfer's source.** This crate holds `fs.volume` (3.1-35 to 3.1-41) and `fs.bspline` (wave 3.2) of the FreeSurfer
replica (`docs/freesurfer/waves.md` section 4): the `MRI` structure, voxels and frames, conforming, regions, voxel lists, resampling, transforms (LTA, `.xfm`, FSL matrices), the Talairach helpers, warp fields and CMA labels (`fs.volume`); EPFL's B-splines as Reuter's `mriBSpline.cpp` writes them, crediting EPFL (`fs.bspline`). It is distributed under FreeSurfer's
licence (`LicenseRef-FreeSurfer`, `../LICENSE`: the FreeSurfer Software License with its required
preface), as part of the `larmorx-freesurfer` package (`docs/licensing.md`).

The upstream is FreeSurfer 8.2.0 (GitHub's `v8.2.0` archive, SHA-256
`bb1699f7655ebf0f3790db6a78af9532ea7c832c97fe5f71bfd9f0e9526d7e78`; `docs/freesurfer-plan.md`),
built for the official binaries with GCC 8.4.0 (`docs/freesurfer/variation.md`).

## FreeSurfer files and functions translated

| FreeSurfer file | What | Item | Here |
|---|---|---|---|
| `utils/transform.cpp` 285-603 | `vg_print`, `getVolGeom`, `copyVolGeom`, `writeVolGeom`, `readVolGeom`, `vg_i_to_r`, `vg_r_to_i`, `TkrVox2RASfromVolGeom`, `vg_isEqual`, `vg_isNotEqualThresh` (taken first from 3.1-39 for the surface readers) | 3.1-39 | `vgeom.rs` |

No third-party code in these lines (the audit of `docs/freesurfer/third-party.md`). `readVolGeom`'s
`sscanf` calls are modelled on glibc 2.35's behaviour measured as a black box (the open
`validation/investigations/glibc-scanf-probe/`; the conversions themselves are
`larmorx-freesurfer-core`'s `cscan`), no glibc code.

## Changes from the originals

- `readVolGeom`: FS-285 to FS-288 (`docs/findings/freesurfer-volume.md` §1): the words are read whole
  (FS-285, FS-286; `reproduce` compares what the official build compares after FS-286's overflow);
  a line with no word is neither counted nor read by default (FS-287); before any word, no key in
  both modes (FS-288).
