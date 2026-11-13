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
| `utils/mri.cpp` 1966-2450, 3128, 3863-4090, 4315-4360, 4696-4800, 4913-5555, 5597-5760 | `MRImatch`, `MRImatchDimensions`, `MRIlinearScale`, `MRIscalarMul`, `MRIscalarMulFrame`, `MRIlabelValRange`, `MRIvalRange`, `MRIvalRangeFrame`, `MRIvalRangeRegion`, `MRIcheckSize`, `MRIextract`, `MRIcopyFrames`, `MRIextractRegion`, `MRIextractInto`, `MRIclear`, `MRIthreshold`, `MRIupperthresholdAllFrames`, `MRIinvertContrast`, `MRIbinarize`, `MRIsubtract`, `MRIabs`, `MRIpos`, `MRIneg`, `MRIadd`, `MRImultiply`, `MRIscaleAndMultiply`, `MRIdivide`, `MRIclone`, `MRIcloneBySpace` (the row-pointer macros of `include/mri.h`: `MRIvox`, `MRIseq_vox`, `MRIFseq_vox`, `MRISseq_vox`, `MRIUSseq_vox`, `MRIIseq_vox`, `MRILseq_vox` with `machine.h`'s `long32`) | 3.1-35 | `arith.rs`, `rows.rs` |
| `utils/mri.cpp` 5557-5588 (`MRIaverage`), 9434-9501 (`MRIsampleVolumeFrameType`'s nearest path at whole coordinates, `arith.rs`), 56-61, 3281-3312, 3515-3741, 4371-4490, 6007-6044, 6156-6192, 9371-9426 (the nearest path), 10814-10832, 12829-12862, 15408-15485 | `MRIsetSampleVolumeDerivativeSign`, `MRIvoxelToTalairach`, `surfaceRASFromVoxel_`, `voxelFromSurfaceRAS_`, `surfaceRASFromRAS_`, `MRIRASToSurfaceRAS`, `MRIvoxelToSurfaceRAS`, `GetSurfaceRASToVoxelMatrix`, `MRIsurfaceRASToVoxel`, `MRIcenterOfMass`, `MRIprincipleComponents`, `MRIdump`, `MRIcompareHeaders`, `MRIsampleVolumeType` (`SAMPLE_NEAREST`), `MRIindexNotInVolume`, `MRIvolumeDeterminant` (with `stuff_four_by_four`), `MRIdircosToOrientationString`, `MRIsliceDirectionName` | 3.1-35 (the last four taken early from 3.1-36 to 3.1-38) | `coords.rs` |
| `utils/mriio.cpp` 263-288 | `getSliceDirection` (taken from fs.io for `MRIvoxelToTalairach`) | 3.1-25 | `coords.rs` |
| `packages/minc/minc_volume_io.c` 199-250 | `transform_point` (`homogenous_transform_point` with `w = 1`) in the official build's operand order (`coords::transform_point_fs`) | 3.1-14 | `coords.rs` |
| `utils/mri.cpp` 11600 | `MRIcopyFrame` (taken early for `MRIcopyFrames`) | 3.1-36 | `arith.rs` |
| `utils/mri.cpp` 14858 | `MRIcalcCRASforExtractedVolume` (taken early for `MRIextractInto`; `affine.h`'s `AffineMV` as the official build inlines it) | 3.1-37 | `arith.rs` |
| `utils/mri.cpp` 9033-9146, 9371-9501, 9688-9765, 10187-10207, 10254-10372, 10463-10581, 10582-10813 | `MRIsampleVolumeFrame`, `MRIsampleVolumeType`, `MRIsampleVolumeFrameType`, `MRIsampleVolume`, `MRIcubicCoeff`, `MRIcubicSampleVolume`, `MRIcubicSampleVolumeFrame`, `ham_sinc`, `MRIsincSampleVolume`, `MRIsincSampleVolumeFrame` (the official build's operand orders from the unit oracle's `mri.cpp.o`; `include/machine.h`'s `long32`) | 3.1-36 | `sample.rs` |
| `utils/mri.cpp` 6313-6349, 6472-6556, 6670-6735 | `MRItranslate`, `MRIrotateX_I`, `MRIrotateY_I`, `MRIrotateZ_I`, `MRIrotate_I` | 3.1-35 | `rotate.rs` |
| `utils/mri.cpp` 2754-3011 | `MRIfindApproximateSkullBoundingBox` (`MIN_DARK`, `MAX_LIGHT`) | 3.1-35 | `skull.rs` |
| `utils/transform.cpp` 1576-1621, 2737-3240, 3332-3365; `utils/utils.cpp` 284-304 | `TransformFileNameType`, `ltaReadRegisterDat`, `mincFindVolume`, `mincGetVolumeInfo`, `mincGetVolInfo`, `ltaMNIreadEx`, `ltaReadFileEx`, `LTAreadExType`, `LTAreadEx`, `ltaFSLread`; `fgetl`'s buffer as these functions read it | 3.1-39 | `ltaread.rs` |

No third-party code in these lines (the audit of `docs/freesurfer/third-party.md`). `readVolGeom`'s
`sscanf` calls are modelled on glibc 2.35's behaviour measured as a black box (the open
`validation/investigations/glibc-scanf-probe/`; the conversions themselves are
`larmorx-freesurfer-core`'s `cscan`), no glibc code.

## Changes from the originals

- `readVolGeom`: FS-285 to FS-288 (`docs/findings/freesurfer-volume.md` §1): the words are read whole
  (FS-285, FS-286; `reproduce` compares what the official build compares after FS-286's overflow);
  a line with no word is neither counted nor read by default (FS-287); before any word, no key in
  both modes (FS-288).
- The voxel arithmetic: FS-289 to FS-297 (`docs/findings/freesurfer-volume.md` §3): accesses outside
  a smaller volume (FS-289 other voxels, `reproduce` follows; FS-290 memory, never made); the byte-only
  functions on other types an error (FS-291); `MRIdivide`'s byte loop moves on (FS-292); a destination
  of another type kept (FS-293); a negative row length an error (FS-294); the new volume's start and
  end with the float voxel sizes (FS-295); a NaN stored as 0 (FS-296); 64-bit rows copied from their
  column (FS-297).
- The samplers, moves, head box and transform reading: FS-298 to FS-310 (`docs/findings/freesurfer-volume.md`
  §5-7): long volumes sampled whole (FS-298), cubic sums over 8 (FS-299), samples stored with the
  volume's conversion (FS-301), rule A; the cubic frame window's unread points 0 (FS-300), a sample
  never set 0 (FS-303), the `.lta` header values never set 0 (FS-308), a long name classified whole
  (FS-306), both modes; the crashes and hangs errors (FS-302, FS-304, FS-305, FS-307, FS-309, FS-310).
  `MRIrotateX_I` and the others do not allocate the clone FreeSurfer loses. `MRIreadHeader` and
  `regio_read_register` (fs.io) are the caller's (`ltaread::LtaIo`).
