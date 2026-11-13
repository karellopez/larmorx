# Provenance: larmorx-freesurfer-mri

**Replica: translated from FreeSurfer's source.** This crate holds `fs.mri` (queue item 3.1-55) of
the FreeSurfer replica (`docs/freesurfer/waves.md` section 4): the `MRI` structure below the readers
that build it (the orchestrator's decision on 3.1-25, 2026-11-13). It is distributed under
FreeSurfer's licence (`LicenseRef-FreeSurfer`, `../LICENSE`: the FreeSurfer Software License with
its required preface), as part of the `larmorx-freesurfer` package (`docs/licensing.md`).

The upstream is FreeSurfer 8.2.0 (GitHub's `v8.2.0` archive, SHA-256
`bb1699f7655ebf0f3790db6a78af9532ea7c832c97fe5f71bfd9f0e9526d7e78`; `docs/freesurfer-plan.md`),
built for the official binaries with GCC 8.4.0 (`docs/freesurfer/variation.md`).

## FreeSurfer files and functions translated (3.1-55, piece 1 of 2)

| FreeSurfer file | What | Here |
|---|---|---|
| `include/mri.h` | `VOL_GEOM` (fields, initialisers, copy constructor, copy assignment), `MRI` (fields and initialisers), `MRI::Shape`, `MRI_FRAME`, `MRI_REGION`, the type, intent and version constants, `MRI_COMPLEX_REAL`/`IMAG` | `vol_geom.rs`, `mri.rs`, `voxel.rs` |
| `utils/mri.cpp` 74-99 | the clipping constants (`UCHAR_MIN`, `SHORT_MIN`, `SHORT_MAX`; the others from `<limits.h>`, checked with the unit oracle's preprocessor) | `voxel.rs` |
| `utils/mri.cpp` 108-535 | `MRI::Shape::Shape`, both `MRI::MRI`, `initGCAMorphLabel`, `initSlices`, `initIndices`, `~MRI` (Rust's drop), `intentName`, `intentCode`, `MRIfree`, `MRIalloc`, `MRIallocSequence`, `MRIallocHeader` | `mri.rs` |
| `utils/mri.cpp` 589-745 | `MRIxfmCRS2XYZ`, `MRImatrixOfDirectionCosines`, `MRImatrixOfVoxelSizes`, `MRImatrixOfTranslations`, `extract_i_to_r`, `extract_r_to_i`, `MRIsetVox2RASFromMatrix` | `vol_geom.rs` |
| `utils/mri.cpp` 1383-1430 | `MRIp0ToCRAS` | `vol_geom.rs` |
| `utils/mri.cpp` 1470-1880 | `MRIsizeof`, `MRIgetVoxDx`/`Dy`/`Dz`, `MRIgetVoxVal`, `MRIsetVoxVal` (with the `*seq_vox` macros of `mri.h` and `long32` of `machine.h`) | `mri.rs`, `voxel.rs` |
| `utils/mri.cpp` 3836 | `MRIreInitCache` | `vol_geom.rs` |
| `utils/mri.cpp` 6198, 13221, 16256, 16279 | `MRIcopyHeader`, `MRIcopyPulseParameters`, `MRIcopyVolGeomToMRI`, `MRIcopyVolGeomFromMRI` | `mri.rs` |
| `utils/transform.cpp` 365, 381 | `MRIallocFromVolGeom`, `useVolGeomToMRI` | `mri.rs` |
| `include/colortab.h`, `utils/colortab.cpp` 264-350 | `COLOR_TABLE`, `COLOR_TABLE_ENTRY`, `CTABfree`, `CTABdeepCopy` (the structure an `MRI` carries; the rest of the colour tables is `fs.labels`) | `ctab.rs` |

No third-party code in these lines (the audit of `docs/freesurfer/third-party.md`). MINC's
`General_transform`, which an `MRI` holds, is the main package's MINC 1.5.1 port
(`larmorx_io::xfm::GeneralTransform`, track C's 3.1-14); FreeSurfer's own copy of volume_io
(`packages/minc/minc_volume_io.c`) changes three functions an `MRI` uses, translated in
`mri.rs`: `get_linear_transform_ptr`, `get_inverse_linear_transform_ptr` (a transform that is not
linear: their name on standard error and `exit(1)`, where MINC reports an internal error and
returns `NULL`) and `copy_general_transform` (only linear transforms and concatenations, any other
kind `copy_and_invert_transform bad kind` and `exit(1)`).

## FreeSurfer files and functions translated (3.1-55, piece 2 of 2)

| FreeSurfer file | What | Here |
|---|---|---|
| `include/mri.h` 213-218 | `VOL_GEOM`'s `get_Vox2RAS`, `get_RAS2Vox`, `get_Vox2TkregRAS`, `get_TkregRAS2Vox`, `get_RAS2TkregRAS`, `get_TkregRAS2RAS` | `tkreg.rs` |
| `include/mri.h` 329-660 | `VOL_GEOM::vgprint`, `geomprint`, `operator==`, `isNotEqualThresh`, `checkgeom`, `checkvox2ras`, `diffvox2ras`, `shearless_components`, `toMatrix` (`FZEROTHR` of `macros.h`) | `vol_geom.rs` |
| `include/mri.h` 700-701 | `MRI::Shape`'s `operator==`, `operator!=` | `mri.rs` |
| `include/transform.h` 74 | `vg_isEqual_Threshold` | `larmorx-freesurfer-core`'s `Globals` |
| `utils/mri.cpp` 835-1380 | `MRIxfmCRS2XYZtkreg`, `MRIxfmCRS2XYZfsl`, `MRItkReg2Native`, `MRItkRegMtx`, `MRItkRegMtxFromVox2Vox`, `MRIvoxToVoxFromTkRegMtx`, `MRIfixTkReg`, `MRIfsl2TkReg`, `MRItkreg2FSL`, `MtxCRS1toCRS0` | `tkreg.rs` |
| `utils/mri.cpp` 1437 | `MRIhfs2Sphinx` | `tkreg.rs` |
| `utils/mri.cpp` 1884-1960 | `MRIinterpCode`, `MRIinterpString`, `MRIprecisionCode`, `MRIprecisionString` (`SAMPLE_*` of `mri.h`) | `mri.rs` |
| `utils/mri.cpp` 3423, 3491, 3784 | `MRIvoxelToWorld` (with `affine.h`'s `AffineMV` as the official build inlines it), `MRIworldToVoxelIndex`, `MRIworldToVoxel` | `vol_geom.rs` |
| `utils/mri.cpp` 3634 | `RASFromSurfaceRAS_` | `tkreg.rs` |
| `utils/mri.cpp` 5783 | `MRIcopy` | `copy.rs` |
| `utils/transform.cpp` 567, 580 | `VGtkreg2RAS`, `VGras2tkreg` | `tkreg.rs` |
| `packages/minc/minc_volume_io.c` 397-430, 716 | FreeSurfer's `get_linear_transform_ptr`, `get_inverse_linear_transform_ptr`, `copy_general_transform` (FreeSurfer's changes to MINC 1.5.1's volume_io; the transform itself is `larmorx_io::xfm`) | `mri.rs` |

## Changes from the originals

- The voxels are one contiguous buffer (FreeSurfer's `chunk`); its fallback of separate slices,
  taken only when that allocation fails, is not kept (such a volume ends with the fallback's
  message). Row and slice pointers are computed, not stored.
- C++'s inheritance (`MRI : VOL_GEOM`) is a field `vg` with `Deref`; pointers into the transform
  (`linear_transform`) are the matrices they point at; `ischunked`, `valid`, `mean`, `roi.x/y/z`
  that FreeSurfer leaves unset are 0.
- D13 (`larmorx_core::defects`): FS-52 to FS-60 and FS-161 to FS-166, each corrected by default
  with `reproduce` giving FreeSurfer's result where it is defined (findings:
  `docs/findings/freesurfer-mri.md`).
- `MRIcopy`'s `mri_src == mri_dst` (return at once) has no counterpart: a volume cannot be passed as
  both. `VGras2tkreg`'s ignored output argument is left out (no caller passes one).
- Products FreeSurfer writes into one of their own inputs (`MatrixMultiply(D, R, D)`,
  `MatrixInverse(D, D)`) are computed from a copy of that input, as `MatrixMultiply` and
  `MatrixInverse` copy it.
