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
| `include/cma.h` (whole); `utils/cma.cpp` 463-1098, 1541-1645, 2117-2160 | the label numbers and the `IS_*` macros (transcribed mechanically); `IsSubCorticalGray`, `SupraTentorialVolCorrection`, `CorticalGMVolCorrection`, `MRIasegContraLatLabel`, `MRIlrswapAseg`, `MRIlrswapSeg`, `MRIfixAsegWithRibbon`, `brainVolumeStatsFilename`, `CacheBrainVolumeStats`, `ReadCachedBrainVolumeStats` (its compute step the caller's), `MRIoneHotEncode`; libstdc++'s `>> double` (`num_get`, "C" locale) as these functions read the cache | 3.1-35 | `cma.rs` |

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
- The CMA labels and segmentation functions: FS-311 to FS-315 (`docs/findings/freesurfer-volume.md` §8):
  a voxel label that is NaN or beyond `int` saturates (FS-311), a thalamus voxel counts once (FS-312),
  `Right_vessel` counts as `Left_vessel` does (FS-313), rule A; a cache line without a number that
  ends in white space gives no number (FS-314, an error under `reproduce`); label lists read past
  their numbers an error (FS-315). `USE_AVERAGE305` comes from `ltaread::LtaIo::env` (the program's
  environment), not the process's.
- `src/resize.rs` (3.1-36; relay 16): `utils/mri.cpp` `MRIsetResolution`, `MRIvalueFill`, `MRIfillFG`,
  `MRIextractPlane`, `MRIfillPlane`, `MRIupsampleN`, `MRIupsample2`, `MRIupsampleNConserve`,
  `MRIdownsample2`, `MRIdownsampleN`, `MRImeanFrameThresh`, `MRIchangeType`, `MRISeqchangeType` and (from
  3.1-37) `MRIlimits`; `include/macros.h`'s `MAX`; the official build's operand orders from the unit
  oracle's `mri.cpp.o` (`docs/findings/freesurfer-volume.md` §9). Changes: FS-316 (the fill's seed clamped
  by its own axis), FS-318 (the frame's mean of the frame asked for), rule A; FS-317 (out-of-range
  conversions clamped, D13); FS-319 (an error, both modes); the fill keeps the bytes written where the
  destination holds no voxel and reads them back (FreeSurfer reads back the memory it wrote).
- `src/seq.rs` (3.1-36; relay 17): `utils/mri.cpp` `MRIinterpolateIntoVolume`, `MRIinterpolateIntoVolumeFrame`,
  `MRIsampleSeqVolume`, `MRIsampleSeqVolumeType`, `MRItrilinKernel`, `MRIsampleVolumeFrame_xyzInt_nRange_floats`,
  `MRIsampleVolumeFrameType_xyzInt_nRange_SAMPLE_NEAREST_floats` (compiled: `include/faster_variants.h`); operand
  orders from the unit oracle's `mri.cpp.o` (`docs/findings/freesurfer-volume.md` §10). Changes: FS-320 (sums clamped
  to the type's range), FS-321 (64-bit voxels added into whole), rule A.
- `src/deriv.rs` (3.1-36; relay 17): `utils/mri.cpp` `MRIsampleVolumeDerivativeScale`, `MRIsampleVolumeDerivative`,
  `MRIsampleVolumeGradient`, `MRIsampleVolumeGradientFrame`, `MRIneighborsOn`, `MRIneighborsOn3x3`,
  `MRIneighborsInWindow`, `MRIneighbors`, `MRIneighborsInRange`, `MRIneighbors3x3`, `MRIneighborsOff`,
  `MRIneighborsOff3x3`. Changes: FS-322 (a value never written taken as 0, both modes), FS-323 and FS-324 (errors,
  both modes).
- `src/xform.rs` (3.1-36, 3.1-37, 3.1-38; relay 17): `utils/mri.cpp` `MRIinverseLinearTransform`,
  `MRIapplyRASlinearTransform`, `MRIapplyRASlinearTransformInterp`, `MRIapplyRASinverseLinearTransform`,
  `MRIlinearTransform`, `MRIlinearTransformInterp`, `MRIgetResampleMatrix`, `MRIrasXformToVoxelXform`;
  `utils/mriset.cpp` `MRIsetValues`. Changes: FS-325 (a value never written taken as 0, both modes); cubic B-spline
  sampling is a port error until `fs.bspline` (wave 3.2) exists.
- `src/resize.rs`, relay 17: `MRIextractPlane` stores the float it read (no round trip through `double`, as the
  official build; the debug build quieted signalling NaNs before).
- `src/resample.rs` (3.1-37; relay 18): `utils/mri.cpp` `MRIresample`, `MRIresampleFill` (12072-12657), the limit
  macros (75-95); the trilinear sum's operand orders, the nearest path's bit copies and the 4-byte long stores from
  the unit oracle's `mri.cpp.o` (`docs/findings/freesurfer-volume.md` §11). Changes: FS-326 (every vote counted),
  FS-328 (64-bit voxels written whole), rule A; FS-327 (an error, both modes); cubic B-spline sampling is a port
  error until `fs.bspline` (wave 3.2) exists.
- `src/arith.rs`, relay 18 (3.1-38): `utils/mriset.cpp` `MRIcheckVolDims` (73-98), `MRIreplaceValues` (2263-2297),
  `MRIreplaceValuesUchar` (2338-2368). Change: FS-461 (every frame replaced), rule A. FreeSurfer's OpenMP loop over
  the slices writes each voxel once: run in order.
- `src/stats.rs` (3.1-37; relay 19): `utils/mri.cpp` `MRIprintStats`, `MRIstats` (12681-12828), `MRItype2str`
  (16535-16558), `MRIcropBoundingBox` (15191-15200), `MRIcalcCRASforSampledVolume` (14776-14842; `affine.h`'s
  `AffineMV` as in `MRIcalcCRASforExtractedVolume`). Changes: FS-329 (the 64-bit voxels read), rule A; FS-330 (0 for
  values never written), FS-462 (an error), both modes.
- `src/orient.rs` (3.1-37; relay 19): `utils/mri.cpp` `stuff_four_by_four` (12863-12908), `MRIvoxelXformToRasXform`
  (12992-13008), `MRIsetVoxelToRasXform` (13019-13075), `MRIgetVoxelToVoxelXform` (15202-15212),
  `MRIorientationStringToDircos` (15256-15319), `MRIcheckOrientationString` (15328-15398); `getenv` through the
  caller's `Environment`. Changes: FS-463 (NUL past the end), FS-464 (an error), both modes.
- `src/image.rs` (3.1-35's image part; relay 19): `utils/mri.cpp` `MRItoImageView` (6796-7106), `MRItoImage`
  (7115-7155), `ImageToMRI` (7157-7314), on track A's `larmorx_freesurfer_io::image`. Changes: FS-466 (each RGB frame's
  own pixels), rule A; FS-465 (only the pixels written and read), both modes.
- `src/labels.rs` (3.1-37; relay 19): `utils/mri.cpp` `MRIsmoothParcellation` (13124-13186), `MRIeraseBorderPlanes`
  (13188-13220), `MRImaxInNbhd` (13286-13304), `MRIcountNonzeroInNbhd`, `MRIareNonzeroInNbhd` (13329-13362),
  `MRIfindNearestNonzeroLocation` (13363-13401), `MRImeanInLabelMultispectral` .. `MRImaxInLabelInRegion` (14108-14246),
  `MRImakePositive` (14247-14269), `MRImarkLabelBorderVoxels` (14671-14718), `MRIcomputeLabelNbhd` (14720-14765),
  `MRIlabelsInNbhd`, `MRIlabelsInPlanarNbhd` (15807-15857), `MRIlabelBoundingBox` (16224-16250), `MRIcountThreshInNbhd`,
  `MRIfillBox` (16373-16404), `MRImaskZero` (16518-16533), `MRIclipBrightWM` (17744-17776); `WM_MIN_VAL`
  (`include/mri.h`), `DEFAULT_DESIRED_WHITE_MATTER_VALUE` (`include/mrinorm.h`). Changes: FS-468 (clamped bytes), rule
  A; FS-467 (a bin of its own), both modes; FS-12's rule for the `(int)` truncations.
- `src/labels.rs` (3.1-37; relay 20): `utils/mri.cpp` `MRIvoxelsInLabelWithPartialVolumeEffects` (14346-14583),
  its label windows kept as sorted maps of the labels met (the same reads and `float` sums as `MRIcomputeLabelNbhd`),
  operand orders from the official build (`volume += vox_vol`; `vox_vol * pv + volume`). Changes: FS-469 (an own label
  outside the limit takes its window mean), both modes; FS-470, FS-471 (crashes: errors; tables past the stack counted).
- `src/frames.rs` (3.1-37; relay 20): `utils/mri.cpp` `MRIrandn` (13718-13751), `MRIconst` (13926-13999),
  `MRInormalizeSequence` (14005-14043), `MRInormalizeFrames`, `MRInormalizeFramesMean`, `MRInormalizeFramesFirst`
  (15945-16020), `MRIsort`, `CompareDoubles` (16076-16134; the C library's `qsort` through
  `larmorx_core::glibc_qsort`), `MRIreverseSliceOrder` (15547-15586), `MRIcloneDifferentType` (16406-16416). Changes:
  FS-472 (clamped narrow stores), rule A; FS-473 (b-table entries past the rows left 0), both modes.
- `src/order.rs` (3.1-37; relay 20): `utils/mri.cpp` `compare_sort_mri`, `MRIorderIndices` (16441-16480),
  `MRIcomputeVoxelPermutation` (16482-16516, fs.core's `randomNumber`). Changes: FS-474 (whole coordinates, clamped
  values), rule A.
- `src/deriv.rs` (3.1-37; relay 20): `utils/mri.cpp` `MRInonMaxSuppress` (15641-15724).
- `src/resize.rs` (3.1-37; relay 20): `utils/mri.cpp` `MRIextractRegionAndPad` (15726-15769).
- `src/combine.rs` (3.1-37; relay 20): `utils/mri.cpp` `MRIrms` (16643-16675; the official build's operand order in
  the last frame), `MRIcomputeVolumeFractions` (16690-16774), `MRIcombineDistanceTransforms` (17386-17412),
  `MRIapplyDiffVect`, `MRIdiff2Vect` (17899-17996), `MRIestimateTIV` (15892-15911); `utils/mri2.cpp` `MRIdimMismatch`
  (1346-1353). Changes: FS-476, FS-477, FS-478, rule A; FS-475 (label 0 left out), both modes; FS-479 (an error).
- `src/voxops.rs` (3.1-37; relay 20): `utils/mri2.cpp` `mri_framepower` (417-466; `sqrtf`, `powf`), `mri_binarize`
  (476-546), `mri_reshape` (653-734), `MRIsqrt`, `MRImax` (1781-1835), `MRImultiplyConst`, `MRIaddConst` (1866-1912),
  `MRIframeBinarize` (1922-1958), `MRIvote` (2063-2149), `MRIsquare`, `MRIsquareRoot` (2401-2466), `MRIcrop`
  (2643-2708), `MRIreverseSlices` (2748-2799), `MRIcutEndSlices` (2801-2838), `MRIsegIdListNot0`, `MRIsegIdList`
  (2873-2928), `MRIhalfCosBias` (3077-3100); operand orders from the official build's object `mri2.cpp.o` (the unit
  oracle's `libutils.a`). Changes: FS-480, FS-481, rule A.
- `src/vol2vol.rs` (3.1-37; relay 21): `utils/mri2.cpp` `MRIvol2Vol` (877-1051), `MRIvol2VolTLKernel` (1233-1302),
  `MRImaskAndUpsample` (1304-1344), `MRIvol2VolDelta` (2580-2641), `MRIvol2VolVSM` (3102-3266); `utils/transform.cpp`
  `TransformRegDat2LTA` (4417-4452, 3.1-40 taken early); operand orders from the official `mri2.cpp.o`. Changes:
  FS-482 (0 for a shift never written, both modes); cubic B-splines and `LTAchangeType` port errors until ported.
- `src/pca.rs` (3.1-37; relay 21): `utils/mri2.cpp` `MRIfdr2vwth` (1402-1574), `MRIcovarianceMatrix` (1576-1641),
  `MRIpca` (1643-1744), `PrintPCAStats`, `WritePCAStats` (1746-1779); `utils/sig.cpp` `fdr2vwth`, `doublecompar`
  (53-110). Changes: FS-483 (rule A), FS-484 (an error); `log10` glibc's in both modes.
- `src/region.rs` (3.1-38; relay 21): `utils/region.cpp` `REGIONalloc` (49-57), `REGIONclear`, `REGIONcopy` (116-133),
  `REGIONgetBoundingBoxM` (299-334), `REGIONgetBoundingBox` (344-379), `REGIONgetBoundingBoxEqOdd` (387-492),
  `REGIONprint` (495-499); `FZERO`'s 1e-5 of this file.
- `src/mrivoxel.rs` (3.1-38; relay 21): `utils/mrivoxel.cpp` `MRIvoxelDx`, `MRIvoxelDy`, `MRIvoxelDz` (55-148),
  `MRIvoxelMean` (174-205), `MRIvoxelDirection` (316-384), `MRIvoxelGradientDir2ndDerivative` (393-433); operand
  orders from `mrivoxel.cpp.o`. Changes: FS-485 (the window not cut by default), FS-486 (0 for a sample never
  written, both modes).
