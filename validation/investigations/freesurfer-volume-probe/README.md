# FreeSurfer 8.2.0's volume functions and B-splines, as a black box (fs.volume parts 1 to 8, fs.bspline)

The probe behind the **(observed)** marks and the validation plan (§69) of
[`specs/freesurfer-volume.md`](../../../specs/freesurfer-volume.md) for its parts 1 to 8 (§1 to §35, §37 to §39, and §54's changes of a transform array between the voxel, RAS-to-RAS and physical-voxel types:
the volume-geometry text block, its copies, matrices, comparison and printer; the voxel
arithmetic; coordinates, orientation, the Talairach point, centre of mass, principal components,
the running average, the header comparison, determinant and dump; the samplers, the moves, the
head's box and the transform reader; the CMA label tests and the segmentation functions; resolution,
planes, fills, resampling by whole factors and changes of type; the frame-range samplers, the trilinear
kernel and splatting, derivatives, gradients, neighbour counts, linear transforms; resampling onto a
template and replacing a value; statistics, orientation strings and voxel-to-RAS matrices; slices as 2-D
images; neighbourhoods and labels; the partial-volume count of a label, random and constant volumes, frames,
orders, fractions, difference lists and voxelwise operations), and of
[`specs/freesurfer-bspline.md`](../../../specs/freesurfer-bspline.md) (§9: the B-spline coefficients,
their sampling, the transforms through them and the factor-of-two pyramids). **Open to the clean-room
implementer of that spec**: the files here hold only calls of FreeSurfer's functions (and of the C
library's) by name, the reading and printing of their arguments, results and the structures'
fields, and a comparison loop; no FreeSurfer code, nothing written from its source. The implementer
runs the prebuilt probe (below) and the comparison loop; building the probe reads the unit oracle's
build tree, which is closed to FreeSurfer's clean-room implementers, so they never build it.

| File | What it is |
|---|---|
| `fs_volume_probe.cpp` | the probe: a batch of calls from a file, each in a child process of its own; its commands below |
| `build.sh` | builds it with the unit oracle's compiler (Ubuntu's GCC 8.4.0) and FreeSurfer's flags against `<workspace>/oracles/freesurfer-8.2.0-unit` (queue item 3.1-02): `build.sh <workspace> <output>` (the spec writer's or the orchestrator's step, not the implementer's) |
| `compare.py` | the comparison loop of the volume spec's §69 and the B-spline spec's §9: 29 groups of seeded cases and twelve of fixed cases (`vrules`, `vrules4` to `vrules13`, `vbsrules`; 15,435 calls), run on the probe (four times: plainly, plainly again in another process and in two guard runs, through the oracle-output cache) and on a candidate program that takes the same commands; `list`, `refs`, `compare` |
| `models.py` | statements of the spec checked as models of the probe's printed results: `fs291` (§9's byte walk of FS-291 under `reproduce`, a tensor volume's rows all at its first voxel: every `invc` and `sam` line of `vscale`, `vtwo` and `vrules`), `skull` (§20's walks: every `vskull` line), `cma` (§23's label tables: every `cmalab` line of `vcma`), `view` (§34's `MRItoImageView`: the `toimgv` lines of `vimage` and `vrules9` without a given image), `smooth` (§35's `MRIsmoothParcellation`: every `smpar` line on shorts), `nearest` (§35's `MRIfindNearestNonzeroLocation`: every `nearnz` line of `vnbhd`), `physvox` (§54's change from physical-voxel to RAS-to-RAS under `reproduce`: 60 seeded arrays, with the probe's own `vgi2r` and `vgr2i`), `ltatypes` (§54's six changes between the voxel, RAS-to-RAS and physical-voxel types, their validity checks and ends: every `ltachg` line of `vltapv` and `vrules10` and 180 seeded arrays, with the probe's own `vgi2r` and `vgr2i`) |

The prebuilt probe: `<workspace>/oracles/freesurfer-8.2.0-unit/probes/fs_volume_probe`; its guard
runs use the fs.maths probe's guard allocator, `guard_malloc.so` beside it
([`../freesurfer-maths-probe/`](../freesurfer-maths-probe/README.md)). Their builds (the larmorx
commit, the `build.sh`, the SHA-256s) are recorded in that folder's `BUILD_INFO.txt` for the
orchestrator's check; the implementer only runs them (every other access to the unit oracle's
folder, reading or hashing a file in it included, counts as opening FreeSurfer's build tree).

## Running it

```sh
fs_volume_probe batch <file>      # or - for standard input
```

Each line of the file is one call: `[@<seconds>] [+gdiag=<hex>] <command> <arguments>`, run as the
fs.mri probe runs its lines ([`../freesurfer-mri-probe/README.md`](../freesurfer-mri-probe/README.md),
"Running it": a child process each, an alarm of 2 seconds unless `@` sets another, the line
`<result> | <standard output> <standard error>`, `EXIT <status>`, `CRASH <signal>`, `TIMEOUT`). From the
build of relay 8 (commit `9319981f`) each child orients its standard error before the call, as a program
that has written to it (the C library's `perror` then leaves `errno` alone: the spec's §21). From the build of
relay 14, two more prefixes before the command: `+gdiagno=<n>` sets `Gdiag_no` to n and points the diagnostic
stream `Gstdout` at standard output (as a program's diagnostic set-up does), `+env=<NAME>=<value>` sets an
environment variable for that call (`+env=FSLOUTPUTTYPE=NIFTI_GZ`; `+env=NAME=` sets it empty; the comparison
loop's runs start from an environment of `PATH`, `LC_ALL=C` and `OMP_NUM_THREADS=1` only). The
commands that write or read a file use `vg.out` and `vg.in` in the working directory (`ltaread` and
`ltareads` their own names, `bvc` and `bvr` the folder `subj`).

## Encodings

Integers, floats, doubles, strings, **M** (a matrix), **G** (a geometry), **V** (a volume's shape),
the fill seed, **CT** (a colour table), **X** (a MINC transform) and the printed volume are the
fs.mri probe's (its README, "Encodings"). Added here:

| Kind | Form |
|---|---|
| W, a working volume | V; a fill: a seed (the fs.mri probe's fill seed: byte i is (seed + 101 i) mod 256; -1 leaves the voxels as allocated, zeros), `t<seed>` (for a float volume: float i of the buffer, frames included, is the table's entry (7 i + seed) mod 34; for another type the seed's bytes) or `i<a>,<b>,...` (the buffer's 32-bit words from the list in order, repeated: integers that a float cannot hold); the geometry's fields but `valid`, the extent and the name: `xsize ysize zsize ras_good_flag x_r x_a x_s y_r y_a y_s z_r z_a z_s c_r c_a c_s s_r s_a s_s`; the header fields `imnr0 imnr1` (integers) `xstart xend ystart yend zstart zend thick ps tr te ti` (floats) `flip_angle` (a double) `dof` (an integer) `fov location` (floats) `ptype`; the cache: 0 the cached matrices as the allocation left them, 1 recomputed from the fields (`MRIreInitCache`), 2 none; the registration matrix (M); the colour table (CT) |
| the table | the floats `00000000 80000000 3f000000 bf000000 3f800000 bf800000 40200000 437f8000 437e8000 43800000 bfc00000 501502f9 d01502f9 7f7fffff ff7fffff 00000001 7f800000 ff800000 7fc00000 ffc00000 7fc12345 7f800001 40500000 42c80000 1e3ce508 477fff80 46ffff00 c7000080 4f000000 4f800000 41200000 c1200000 3dcccccd 40490fdb` (entry 0 first): zeros, halves, ones, 2.5, 255.5, 254.5, 256, -1.5, ±1e10, the largest floats, the smallest subnormal, infinities, NaNs (both signs, a payload, a signalling one), 3.25, 100, 1e-20, 65535.5, 32767.5, -32768.5, 2^31, 2^32, ±10, 0.1, π |
| D, a destination | `new` (none: the function makes one), `self` (the first source), `other` (the second source), or a W |
| R, a region | `null`, or `x y z dx dy dz` |
| a volume (printed) | `null`, or the fs.mri probe's header (its "volume (printed)" without the index tables and frames), the colour table (CT, each entry with its count), the voxels |
| a result (printed) | `self`, `other` or `given` when the returned volume is that one, then the volume; then, when a given destination is not the result, the destination's state |
| unset | an output the call may leave is 7.0 before the call (`40e00000`, `401c000000000000`) |
| P, a painting | n, then n times `x y z value` (integers and a float): frame 0's voxels set with `MRIsetVoxVal` after the volume is made |
| I, an image | `null`, or `<rows> <cols> <format> <frames>` (`ImageAlloc`: the format's code, 0 `PFBYTE`, 1 `PFSHORT`, 2 `PFINT`, 3 `PFFLOAT`, 4 `PFCOMPLEX`, 6 `PFDOUBLE`, 35 `PFRGB`, 36 `PFRGBZ`), `<xsize> <ysize>` (floats), a fill (a seed: byte i of the pixels is (seed + 101 i) mod 256; -1 as allocated, zeros; `t<seed>`: float i is the table's entry (7 i + seed) mod 34), then `<orows> <ocols> <rows> <cols> <num_frame>` written over the header after the fill (-1 keeps the allocation's). Printed: `orows ocols rows cols format num_frame xsize ysize`, then the pixels (a string of the buffer's bytes as allocated, `null` without a buffer). A result is `given` (the given image itself, still marked as the probe's), `new` or `null` |

## Commands

The spec's sections in brackets.

| Command | Calls | Prints |
|---|---|---|
| `vgwrite G` | `writeVolGeom` to `vg.out` [§2] | the file's bytes (a string) |
| `vgread <string> G` | the string written to `vg.in`; `readVolGeom` of it opened for reading into the geometry G, `errno` set to 0 first [§3] | the geometry (printed G), `ftell`, end-of-file and error flags (1/0), `errno`, the next 64 bytes of the stream (a string) |
| `vgprint G` | `vg_print` [§4] | nothing (standard error) |
| `getvg G G`, `copyvg G G` | `getVolGeom`, `copyVolGeom` (the first into the second) | the second (G) |
| `vgi2r G`, `vgr2i G`, `tkrv2r G` | `vg_i_to_r`, `vg_r_to_i`, `TkrVox2RASfromVolGeom` | M |
| `vgeq G G d` | `vg_isNotEqualThresh` (the threshold), then `vg_isEqual` | two integers |
| `clone W D` | `MRIclone` [§7] | the result |
| `clonesp W i i` | `MRIcloneBySpace` (type, frames) | the volume |
| `clear W` | `MRIclear` | its value, the volume |
| `match W W` | `MRImatch`, `MRImatchDimensions` | two integers |
| `chsize W W\|null i i i` | `MRIcheckSize` (the width, height, depth) | its value |
| `cpframe W D i i` | `MRIcopyFrame` (source frame, destination frame) | the result |
| `cpframes W D i i i` | `MRIcopyFrames` (first, last, first destination frame) | the result |
| `vrange W`, `vrangef W i`, `vrangereg W R` | `MRIvalRange`, `MRIvalRangeFrame`, `MRIvalRangeRegion` [§8] | its value, the minimum, the maximum (floats) |
| `lrange W W i` | `MRIlabelValRange` (the labelled volume, the label) | its value, the minimum, the maximum |
| `linscale W D f f i` | `MRIlinearScale` (scale, offset, only non-zero) [§9] | the result |
| `smul W D f`, `smulf W D f i` | `MRIscalarMul`, `MRIscalarMulFrame` (the frame) | the result |
| `thresh`, `uthresh`, `invc` `W D f` | `MRIthreshold`, `MRIupperthresholdAllFrames`, `MRIinvertContrast` | the result |
| `binz W D f f f` | `MRIbinarize` (threshold, low, high) | the result |
| `abs`, `pos`, `neg` `W D` | `MRIabs`, `MRIpos`, `MRIneg` | the result |
| `mul2`, `sub2`, `add2`, `div2` `W W D` | `MRImultiply`, `MRIsubtract`, `MRIadd`, `MRIdivide` [§10] | the result |
| `sam W f W D` | `MRIscaleAndMultiply` | the result |
| `extract W D i×6` | `MRIextract` (x0 y0 z0 dx dy dz) [§11] | the result |
| `extinto W D i×9` | `MRIextractInto` (and x1 y1 z1) | the result |
| `extreg W D R` | `MRIextractRegion` | the result |
| `cras W W i×6` | `MRIcalcCRASforExtractedVolume` (x0 y0 z0 x1 y1 z1) | the three doubles, then the source's geometry (printed) |
| `sras W k` | k 0 `surfaceRASFromVoxel_`, 1 `voxelFromSurfaceRAS_`, 2 `GetSurfaceRASToVoxelMatrix`, 3 `surfaceRASFromRAS_` of the volume [§13] | M, then the volume's geometry (printed) |
| `spt W k d d d` | k 0 `MRIRASToSurfaceRAS`, 1 `MRIvoxelToSurfaceRAS`, 2 `MRIsurfaceRASToVoxel` | its value, the three doubles, the geometry (printed) |
| `ostr G` | `MRIdircosToOrientationString` into 8 bytes of `Z`, `MRIsliceDirectionName`, `getSliceDirection` [§14] | its value, the first 5 bytes (a string), the name, the direction |
| `vtal W X d d d` | `MRIvoxelToTalairach` of the volume with the MINC transform X | its value, the three doubles, the geometry (printed) |
| `inidx W d d d` | `MRIindexNotInVolume` [§15] | its value |
| `snear W d d d d` | `MRIsampleVolumeType(..., SAMPLE_NEAREST)` with the volume's `outside_val` set to the last double | its value, the sample |
| `com W i` | `MRIcenterOfMass` (the threshold, a byte) | its value, the three means |
| `pca W i` | `MRIprincipleComponents` into a new 3 x 3 matrix | its value, the vectors (M), the three values (floats), the means |
| `avg W i D` | `MRIaverage` (dof) [§15b] | the result |
| `cmph W\|null W\|null\|self` | `MRIcompareHeaders` [§16] | its value |
| `vdet W` | `MRIvolumeDeterminant` | a float |
| `dump W` | `MRIdump` to standard output | its value |
| `dsign d` | `MRIsetSampleVolumeDerivativeSign` | the double returned |
| `samp k W d n (d d d i i)×n` | with the volume's `outside_val` set to the first double, n points `x y z frame aux`, each with the sampler k: `tri` `MRIsampleVolume`, `typ` `MRIsampleVolumeType` (aux the interpolation code), `frm` `MRIsampleVolumeFrame`, `fty` `MRIsampleVolumeFrameType` (aux the code), `cub` `MRIcubicSampleVolume`, `cubf` `MRIcubicSampleVolumeFrame`, `sinc` `MRIsincSampleVolume` (aux the half-width), `sincf` `MRIsincSampleVolumeFrame` (aux the half-width) [§18] | per point its value and the sample (a double, unset 7.0) |
| `coef n (d i)×n`, `hsinc n (d d)×n` | `MRIcubicCoeff(x, i)`, `ham_sinc(x, fullwidth)` | the doubles |
| `trans W D d d d` | `MRItranslate` (dx dy dz) [§19] | the result |
| `rot W D M M` | `MRIrotate_I` (the rotation matrix, the centre: a matrix or `null`) | the result |
| `rotx`, `roty`, `rotz` `W D f` | `MRIrotateX_I`, `MRIrotateY_I`, `MRIrotateZ_I` (the angle) | the result |
| `skull W i n (i×6 f)×n` | n boxes `x0 y0 z0 x1 y1 z1 value` written into frame 0 (`MRIsetVoxVal`, every voxel from the first corner to the second), then `MRIfindApproximateSkullBoundingBox` (the threshold) into a box set to -7 [§20] | its value, the box's six integers |
| `tftype n x×n` | `TransformFileNameType` of each name [§21] | the integers |
| `ltaread x x\|nofile i` | the second string written to a file named by the first in the working directory (`nofile`: no file, any old one removed), `USE_AVERAGE305` unset (0), set to `true` (1) or set empty (2), `errno` set to 0, `LTAreadEx` of the name, the file removed | `errno`, then the array (the fs.mri probe's LTA, or `null`) |
| `ltareads n (x x\|nofile)×n i` | as `ltaread`, n files read one after the other in one process (`USE_AVERAGE305` as the last integer says) | per file `errno` and the array |
| `cmalab n (i f)×n` | per pair: §23's label tests (`IS_FUTURE_WMSA`, `IS_LH_LAYER`, `IS_CC`, `IS_RH_LAYER`, `IS_LAYER`, `IS_UNKNOWN`, `IS_BRAIN`, `IS_WM`, `IS_CEREBRAL_WM`, `IS_HYPO`, `IS_WMSA`, `IS_WMH`, `IS_THALAMUS`, `IS_GM`, `IS_VENTRAL_DC`, `IS_CEREBELLAR_WM`, `IS_CEREBELLAR_GM`, `IS_CEREBELLUM`, `IS_CHOROID`, `IS_HIPPO`, `IS_AMYGDALA`, `IS_MTL`, `IS_CORTEX`, `IS_LAT_VENT`, `IS_CSF`, `IS_INF_LAT_VENT`, `IS_VENTRICLE`, `IS_CAUDATE`, `IS_PUTAMEN`, `IS_CLAUSTRUM`, `IS_ACCUMBENS`, `IS_PALLIDUM`, `LABEL_WITH_NO_TOPOLOGY_CONSTRAINT`, `IS_FIMBRIA`, `IS_GRAY_MATTER`, `IS_WHITE_MATTER`, `IS_FLUID`, `IS_LH_CLASS`, `IS_RH_CLASS`, `IS_GRAY_CLASS`, `IS_WHITE_CLASS`, `IS_CSF_CLASS`, then `IS_CLASS` with the classes 0, 1, 2, 7) of the integer, of the float and of the float as a double; `IsSubCorticalGray` and `MRIasegContraLatLabel` of the integer [§23] | three strings of 46 digits 0/1, then the two integers |
| `cmavol W P W P i` | `SupraTentorialVolCorrection`, `CorticalGMVolCorrection` (the segmentation, the ribbon, `hemi`) | two doubles |
| `lrswap W P` | `MRIlrswapAseg` | the volume |
| `fixrib W P W P D` | `MRIfixAsegWithRibbon` (the segmentation, the ribbon, the destination) | the result |
| `lrseg W P M M` | `MRIlrswapSeg` (the segmentation, the `lh` and `rh` lists) | the volume or `null` |
| `ohe W P n i×n i` | `MRIoneHotEncode` (the segmentation, the list, `force1`) | the volume |
| `bvc i d×16` | the folder `subj/stats` in the working directory made (1) or not (0), `CacheBrainVolumeStats` of the 16 values for `subj` in `.` | the file's bytes (a string), or `null` |
| `bvr i x` | the folder made (1, the string written to `subj/stats/brainvol.stats`) or not (0), `ReadCachedBrainVolumeStats` of `subj` in `.` | the count, then the doubles |
| `setres W f f f` | `MRIsetResolution` (x, y, z) [§25] | its value, the volume |
| `vfill W f` | `MRIvalueFill` | its value, the volume |
| `fillfg W D i i i i i` | `MRIfillFG` (the seed x y z, the threshold, the fill value) into `npix` set to -7 | `npix`, the result |
| `explane W D i i` | `MRIextractPlane` (the orientation, the plane) | the result |
| `fillplane W W i i i` | `MRIfillPlane` (the mask, the destination, the orientation, the plane, the fill value) | the result |
| `ups k W D i` | k 0 `MRIupsampleN`, 1 `MRIupsample2` (the integer unused), 2 `MRIupsampleNConserve` (N) | the result |
| `dns2 W D` | `MRIdownsample2` | the result |
| `dnsn W D f f f i` | `MRIdownsampleN` (Fc Fr Fs, KeepType) | the result |
| `mft W i f` | `MRImeanFrameThresh` (the frame, the threshold) | the double |
| `lim W` | `MRIlimits` into min and max set to 7.0 | its value, the two floats |
| `chtype k W i f f i i` | k 0 `MRIchangeType`, 1 `MRISeqchangeType` (the type, `f_low`, `f_high`, `no_scale`), the environment's `FS_FORCE_BIN_CHECK` set to `1` (last integer 1) or unset (0) | the result (`self` when it is the source), then the source when it is not |
| `splat W n (k d d d i d)×n` | per point (x y z, the frame, the value): k 0 `MRIinterpolateIntoVolume` (the frame unused), 1 `MRIinterpolateIntoVolumeFrame` [§27] | per point its value, then the volume |
| `sseq W d n (d d d i i i)×n` | with the volume's `outside_val` set to the double, per point x y z, the first and last frame and the code: -1 `MRIsampleSeqVolume`, else `MRIsampleSeqVolumeType` with that code, into an array of floats set to 7.0 (8 unset slots before index 0 and after the last frame) | per point its value and the floats at the frames first to last |
| `sxyz W d n (i i i i i i)×n` | with `outside_val` set, per point x y z, the frames begin and end (excluded) and k: 0 `MRIsampleVolumeFrameType_xyzInt_nRange_SAMPLE_NEAREST_floats`, 1 `MRIsampleVolumeFrame_xyzInt_nRange_floats` (the array as above) | per point its value and the `end - begin` floats |
| `tkern W n (d d d i)×n` | `MRItrilinKernel` at each point, into an array of 8 doubles set to 7.0 (last integer 0) or none (1) | `given` or `new`, then the 8 doubles |
| `deriv W d n (i d d d d d d d)×n` | `MRIsetSampleVolumeDerivativeSign` of the first double, then per point k 0 `MRIsampleVolumeDerivative`, 1 `MRIsampleVolumeDerivativeScale` (x y z, dx dy dz, sigma) into a magnitude set to 7.0 [§28] | per point its value and the magnitude |
| `grad W n (i d d d i)×n` | k 0 `MRIsampleVolumeGradient`, 1 `MRIsampleVolumeGradientFrame` (x y z, the frame) into three doubles set to 7.0 | per point its value and the three doubles |
| `nbrs W n (i i i i i i f f)×n` | per point the count k (0 `MRIneighborsOn`, 1 `MRIneighborsOn3x3`, 2 `MRIneighborsInWindow`, 3 `MRIneighbors`, 4 `MRIneighborsInRange`, 5 `MRIneighbors3x3`, 6 `MRIneighborsOff`, 7 `MRIneighborsOff3x3`), x y z, then p and q (the minimum or value p; for 2 the window size p and the value q; for 4 the frame p) and the floats low and high (4 only) | the counts |
| `setvals W f` | `MRIsetValues` [§29] | its value, the volume |
| `rasvox W W\|null M` | `MRIrasXformToVoxelXform` (the source, the destination, the RAS matrix or `null`, no voxel matrix given) | M |
| `lxf k W d D M i` | with the source's `outside_val` set to the double, k 0 `MRIlinearTransformInterp`, 1 `MRIlinearTransform`, 2 `MRIinverseLinearTransform`, 3 `MRIapplyRASlinearTransformInterp`, 4 `MRIapplyRASlinearTransform`, 5 `MRIapplyRASinverseLinearTransform` (the destination, the matrix or `null`, the method for 0 and 3) | the result |
| `rsmat W W` | `MRIgetResampleMatrix` (the source, the template; the diagnostic bits through `+gdiag=`) | M |
| `rsfill W W i f i` | `MRIresampleFill` (the source, the template, the type, the fill), or `MRIresample` when the last integer is 1 [§31] | the result |
| `repval W D k ...` | k 1: `MRIreplaceValuesUchar` with two integers (as bytes); k 0: `MRIreplaceValues` with two floats | the result |
| `stats W`, `pstats W` | `MRIstats` into outputs set to 7.0 (the count -7), resp. `MRIprintStats` to standard output [§33] | its value, then for `stats` min, max (floats), the count, mean, std (floats) |
| `type2s n i×n` | `MRItype2str` of each integer | the strings |
| `cropbb W i×6` | `MRIcropBoundingBox` of the box `x y z dx dy dz` | its value, the box's six integers |
| `crass W W` | `MRIcalcCRASforSampledVolume` (the source, the sampled volume) into doubles set to 7.0 | the three doubles, then the source's geometry (printed: its cached matrices kept or made) |
| `chkostr x`, `o2dc W x` | the string (its bytes, NUL included, in an allocation of its own length and a terminator), `MRIcheckOrientationString`, resp. `MRIorientationStringToDircos` of the volume | the returned text (a string) or `null`; resp. its value and the nine direction cosines (floats) |
| `s44 k [i i] f×16` | `stuff_four_by_four` into k 0 no matrix, 1 a real 4 x 4, 2 a real one of the two sizes, 3 a complex 4 x 4 | its value, then the matrix (M; a complex one as its 32 floats of storage) |
| `vx2rx W W\|null M M\|null`, `v2v W W` | `MRIvoxelXformToRasXform` (source, destination or none, the voxel matrix, the RAS matrix given or none), resp. `MRIgetVoxelToVoxelXform` (source, destination) | the matrix (`given` when it is the given one) |
| `setv2r W M x\|unset` | `FS_SetVoxToRasXform_Change_VoxSize` set to the string or unset, then `MRIsetVoxelToRasXform` of the volume with the matrix | its value, then the volume's geometry (printed) |
| `bsp k W i [W]` | `MRItoBSpline` of the volume with the degree, the B-spline k 0 none, 1 `MRIallocBSpline` of the volume's extent and frames, 2 one whose coefficients are the second volume, 3 one without coefficients [fs.bspline §3] | `given` or `new`, the degree, the source type, `srcneg`, the coefficients (the volume printed in full: header, index tables, frames, voxels) |
| `bsamp i W d n (d d d i)×n` | with the volume's `outside_val` set to the double, `MRItoBSpline` with the degree, then n points `x y z frame` sampled by `MRIsampleBSpline` [§4.1] | per point its value and the sample (a double, unset 7.0) |
| `bsraw i i W d n (d d d i)×n` | a B-spline from `MRIallocBSpline` of the volume's extent and frames with the degree and `srcneg` given (no `MRItoBSpline`), the source type the volume's, its coefficients the volume's floats (a float volume; else zeros), their `outside_val` the double; n points sampled as `bsamp` [§4.1, FS-964] | as `bsamp` |
| `bsseq i W d n (d d d i i)×n` | as `bsamp`, each point `x y z first last` sampled by `MRIsampleSeqBSpline` into floats set to 7.0 (first at least 0) [§4.2] | per point its value, then the floats of frames first to last |
| `bslxf i W D M`, `bsras i W D M` | `MRItoBSpline` with the degree, then `MRIlinearTransformBSpline` (voxel to voxel), resp. `MRIapplyRASlinearTransformBSpline` (RAS to RAS), into the destination with the matrix or `null` [§5.1, §5.2] | the result |
| `bslta i W D LTA i` | `USE_AVERAGE305` unset (0), set to `true` (1) or set empty (2), `MRItoBSpline` with the degree, then `LTAtransformBSpline` into the destination with the array [§5.3] | the result |
| `bsdown W D`, `bsup W D` | `MRIdownsample2BSpline`, resp. `MRIupsample2BSpline` [§6.3, §6.4] | the result |
| `bsfree k` | `MRIfreeBSpline` of k 0 none, 1 a B-spline without coefficients, 2 one of 1 x 1 x 1 [§2] | its value, then `none` or `kept` |
| `toimgv W P I i i i` | `MRItoImageView` of the painted volume into the image (the slice, the view, the frame) [§34] | the result (I) |
| `toimg W P I i` | `MRItoImage` (the slice) | the result (I) |
| `img2mri I` | `ImageToMRI` | the image's `xsize`, `ysize` after the call, then the volume |
| `smpar W P i` | `MRIsmoothParcellation` (the count) [§35] | the result |
| `erasebp W i` | `MRIeraseBorderPlanes` (the border) | its value, the volume |
| `nbhd W P n (k i i i i i f)×n` | per point k 0 `MRImaxInNbhd` (wsize x y z frame), 1 `MRIcountNonzeroInNbhd`, 2 `MRIcountThreshInNbhd` (the threshold, the float), 3 `MRIareNonzeroInNbhd` | the integers |
| `nearnz W P n (i d d d i)×n` | per point `MRIfindNearestNonzeroLocation` (wsize, xr yr zr; the place asked for: 1, or not: 0) into a place set to -7 | per point the float and the place's three integers |
| `lmeans k W W P i i i i i i` | the source, the label volume painted, then label, x0 y0 z0 whalf and whether the spread is asked for: k 0 `MRImeanInLabelMultispectral`, 1 `MRImeanInLabel`, 2 `MRImeanAndStdInLabel`, 3 `MRImeanInLabelInRegion`, 4 `MRImaxInLabelInRegion` | k 0 the vector (M); 1, 4 the double; 2, 3 the double and the spread (unset 7.0) |
| `mkpos W D` | `MRImakePositive` | the result |
| `mlbv W P D i i i` | `MRImarkLabelBorderVoxels` (label, mark, six) | the result |
| `lnbhd W P W\|null i i i i i i` | `MRIcomputeLabelNbhd` of the painted label volume and the values (x y z, whalf, max_labels, the means given: 1, or none: 0) into arrays of exactly max_labels entries set to -7 and 7.0 | its value, the counts, then the means when given |
| `linbhd W P n (k i i i i i i)×n` | per point k 0 `MRIlabelsInNbhd` (x y z whalf label), 1 `MRIlabelsInPlanarNbhd` (and the plane) | the counts |
| `lbbox W P i` | `MRIlabelBoundingBox` (the label) into a box set to -7 | its value, the box's six integers |
| `fillbox W R f` | `MRIfillBox` (the box, the value) | its value, the volume |
| `mskz W W P D` | `MRImaskZero` (the source, the painted mask, the destination) | the result |
| `clipwm W W P` | `MRIclipBrightWM` (the T1 volume, the painted white-matter volume) | its value, the T1 volume |
| `ltachg LTA i` | `LTAchangeType` (the type) [§54] | `self` when it returns the array, then the array (LTA) |
| `ltainv k LTA [LTA]` | `LTAinvert` of the array into k 0 none, 1 the second array, 2 the array itself [§54] | `null`, `self`, `given` or `new`, then the result (LTA or `null`), the given array when it is not the result (k 1), the first array when it is not the result (k 0, 1) |
| `ltacat n LTA×n i`, `ltacat2 LTA LTA i` | `LTAconcat` of the n arrays, resp. `LTAconcat2` of the two, with the reduction (1) or not (0) | the result (LTA or `null`), then each given array as the call left it |
| `ltared LTA` | `LTAreduce` | the result (LTA or `null`), then the array |
| `ltasrc LTA W` | `LTAmriIsSource`, then `LTAmriIsTarget` of the volume | the two integers |
| `ltaprt LTA` | `LTAprint` to standard output | its value |
| `pvlab W P W\|null P i i W\|null W\|null W\|null P` | the label volume painted, the values (painted) or none, label, maxlabels, the mixing and neighbour-label volumes or none, the border volume (painted) or none: `MRIvoxelsInLabelWithPartialVolumeEffects` [§37] | the float, then the mixing and the neighbour volumes (`null` when none) |
| `randn i i i i f f i W\|null` | `srand48` of the seed, then `MRIrandn` (columns, rows, slices, frames, avg, stddev) into the given volume or none [§38] | the result (`given` when it is the given one) |
| `const i i i i f W\|null` | `MRIconst` (columns, rows, slices, frames, the value) | the result |
| `nseq W P i f`, `nfr W P k f` | `MRInormalizeSequence` (the target); k 0 `MRInormalizeFrames`, 1 `MRInormalizeFramesMean`, 2 `MRInormalizeFramesFirst` (the integer and the float are read and not used where the call takes none) | its value, the volume |
| `sort W P W\|null P D` | `MRIsort` (the mask painted, or none) | the result |
| `revsl W P M\|null M\|null i D` | the b-value and b-vector tables and the b-vector space set on the volume, then `MRIreverseSliceOrder` | the result, then its two tables (M) and b-vector space |
| `cdtype W i` | `MRIcloneDifferentType` (the type) | the volume |
| `ordidx W P i`, `vperm W P i` | `MRIorderIndices`; `setRandomSeed` of the integer, then `MRIcomputeVoxelPermutation`, into three `short` arrays of one entry per voxel set to -7 | its value, then the x, the y and the z entries |
| `nonmax W P D f i` | `MRInonMaxSuppress` (thresh, dir) | the result |
| `xrpad W P R i` | `MRIextractRegionAndPad` (the region or `null`, pad; no destination) | the volume |
| `rms W P W` | `MRIrms` into the second | the second volume |
| `vfrac W M W P W` | `MRIcomputeVolumeFractions` (the source, the vox2vox matrix, the segmentation painted, the fractions) | its value, the fractions |
| `cdt W P W P D` | `MRIcombineDistanceTransforms` | the result |
| `diffv W P W P W\|null P d` | `MRIdiff2Vect` (the mask painted or none, the threshold) | `null`, or the row count and each row's five doubles |
| `applyv W P W\|null P n (d×5)×n D` | `MRIapplyDiffVect` of the n rows (the mask painted or none) into D | the count applied (unset -7), then the result |
| `tiv x x\|nofile i d i` | `USE_AVERAGE305` as in `ltaread`; any old file of the first string's name removed, the second string written to it (`nofile`: none), then `MRIestimateTIV` (the scale; the determinant asked for: 1, or not: 0), the file removed after a call that returns. Unlike `ltaread`, `errno` is not set to 0 first: when no old file was there, its removal leaves `ENOENT` (2), which the transform reader's error lines then print first (`error: No such file or directory`); a call that ends the program leaves its file in the working directory, so a later line of the batch that names it removes it without setting `errno` | the double, the determinant (unset 7.0) |
| `dimmis W W i` | `MRIdimMismatch` (frames compared: 1) | its value |
| `fpow W P n f×n` | `mri_framepower` with an array of exactly n powers [§39] | its value, the volume |
| `mbin W P D f x\|null i` | `mri_binarize` (thresh, the tail or none, invert) | the count (unset -7), the result |
| `reshape W P i i i i` | `mri_reshape` (columns, rows, slices, frames) | the volume, then the source's frame count |
| `msq k W P W\|null P D` | k 0 `MRIsquare`, 1 `MRIsquareRoot` (the mask painted or none), 2 `MRIsqrt` (the mask read and not used) | the result |
| `mmax W P W P D` | `MRImax` | the result |
| `mconst W P d i D` | 1 `MRImultiplyConst`, 0 `MRIaddConst` (the constant) | the result |
| `fbin W P d W\|null P` | `MRIframeBinarize` (thresh; the mask painted, or none) | the result (`given` when it is the given mask) |
| `vote W P W\|null P D i` | `MRIvote` (the mask painted or none; exclude0) | the result, then the source (sorted in place) |
| `crop W P i×6` | `MRIcrop` (c1 r1 s1 c2 r2 s2) | the volume |
| `revs W P D` | `MRIreverseSlices` | the result |
| `cutend W P i` | `MRIcutEndSlices` (n) | the volume |
| `segids W P i i` | the frame, then 1 `MRIsegIdListNot0` or 0 `MRIsegIdList` | the count, then the list |
| `hcos W P d D` | `MRIhalfCosBias` (alpha) | the result |

## The models

```sh
PYTHONPATH=validation/src scripts/orchestration/boxed.sh 4 python -P \
  validation/investigations/freesurfer-volume-probe/models.py fs291 \
  --probe <workspace>/oracles/freesurfer-8.2.0-unit/probes/fs_volume_probe --scratch <scratch>/models
```

(`fs291`; `skull` and `cma` the same way.) Prints how many lines match the model (by the types met: `uchar` only, a tensor, other types), how
many it leaves out (FreeSurfer crashed or read memory it does not own) and how many differ; exit
status 1 when one differs.

## The comparison loop

```sh
python -P compare.py list --scratch <scratch>/volume
PYTHONPATH=validation/src scripts/orchestration/boxed.sh 8 python -P \
  validation/investigations/freesurfer-volume-probe/compare.py refs \
  --probe <workspace>/oracles/freesurfer-8.2.0-unit/probes/fs_volume_probe --scratch <scratch>/volume
PYTHONPATH=validation/src scripts/orchestration/boxed.sh 8 python -P \
  validation/investigations/freesurfer-volume-probe/compare.py compare \
  --probe <workspace>/oracles/freesurfer-8.2.0-unit/probes/fs_volume_probe \
  --candidate <the candidate> --scratch <scratch>/volume --mode reproduce   # then --mode fix
```

The candidate, the guard runs, the memory lines and what may differ are as in the fs.mri probe's
loop (its README, "The comparison loop"), without its `FS-264` batches: a difference is allowed on a
line where FreeSurfer did not finish (`TIMEOUT`, `CRASH`) and the candidate did, on a **memory**
line, in `--mode fix` on a line where the candidate noted a defect, and in both modes on a line where
the candidate noted a defect whose value in FreeSurfer is memory on the stack it never wrote, on which
the probe's four runs agree only because the program's own earlier steps leave the same bytes there:
FS-300 in `vsample` and `vresamp` (the cubic frame sampler's window points outside the volume), FS-322 in
`vderiv` (a gradient's sample never written) and FS-325 in `vlinear` (a linear transform's samples before
the first that succeeds), FS-469 in `vpvlab` (a label's mean read outside the table, from the stack
beside it, which every run fills the same way) and FS-198, FS-504 and FS-508 in `vlta54` and `vrules14` (a freed
matrix, or a transform the array does not hold, read: the same bytes in every run), and in both modes on the
`bvr` line without a cache that FreeSurfer ends at its licence check (`EXIT 255`, `FREESURFER_HOME is
not defined`: LARMORX performs no licence check). Under `reproduce` a line where FreeSurfer aborted
with the C++ runtime's lines (`CRASH 6`, standard error starting `terminate called after throwing`:
FS-314) and that is not a memory line must end the same in the candidate (`EXIT 134` or `CRASH 6`,
the same standard output and error); the loop prints how many such aborts it checked. Every call gets 10 seconds
(`@10`) but `pca`'s (2: FreeSurfer's eigensystem never returns on some covariances) and the two
`ltaread` calls of FS-307 (3: FreeSurfer never returns). The groups:
`vgblock` (§2-3: 60 writes; 500 reads of generated blocks: well-formed, with lines missing,
repeated, reordered or blank, lines of white space or other keys, first words of 63 to 200 bytes,
odd second words, numbers in glibc's other forms, lines longer than 1,039 bytes, NUL bytes, CRLF
endings, no final newline, data after the block, an empty file), `vgcopy` (§4), `vmake` (§7),
`vrange` (§8), `vscale` (§9), `vtwo` (§10), `vextract` (§11), `vcoords` (§13-14), `vstats` (§15,
§15b), `vheader` (§16), and the fixed cases of `vrules` (§6-15: FS-291 with tensor volumes, the
clear of a bitmap and of volumes without voxels, `MRIlinearScale` with `only_nonzero`, sums and
differences of types without direct access, the frame copies' messages, `MRIvalRangeRegion` without
a region, `MRIsampleVolumeType`'s order of checks), and part 4's `vsample` (§18: the eight samplers at
points inside, at and past the edges, whole and within `FLT_EPSILON` of whole, NaN, infinite and
beyond the int range, frames past the volume's, interpolation codes 0 to 7 and -1, half-widths 0 to
6, 70 and -1; the cubic weights and the sinc kernel), `vmove` (§19: translations, rotations by
matrices (rotations, random, singular, NaN, of other sizes) about given or default centres, rotations
about an axis by NaN, infinite and ordinary angles), `vskull` (§20: volumes of 4 to 64 voxels a side,
with a bright head and dark margins of 6 to 15 voxels, a dark inside, painted boxes, thresholds -5 to
300) and `vltaread` (§21: names with `.`, `@`, `#`, `nii.gz` and 500-byte names classified; `.lta`
texts with comments, odd separators and numbers, 0 to 3 transforms, matrices of other sizes, the
geometry blocks, subject and intensity lines, cut files; `.xfm` texts in the information lines'
forms, cut; `.fslmat`, `register.dat` texts; `identity.nofile`; missing files; `USE_AVERAGE305`
unset, `true` and empty), the fixed cases of `vrules4` (relay 8, §18-21: the cubic weights outside 0
to 3, the sinc kernel's NaNs, `MRIrotate_I` with matrices and centres of other sizes, the `.xfm`
reader's buffer at the end of the file, its three forms of information lines, several `.xfm` files
in one process for `minctracc`'s count, the FSL, `register.dat` and `identity.nofile` readers' fields,
`TransformFileNameType`'s `#` and `@` rules) and part 5's `vcma` (§23: every label from -3 to 1005
and the ranges above 1000 as an integer, a float and a double; segmentations and ribbons of up to 4
x 4 x 4 voxels of six types with painted labels, NaN and odd values, `hemi` 0 to 3; the swaps, the
ribbon fix, the one-hot encoding; the statistics cache's lines in their forms), and the fixed cases of
`vrules5` (relay 9, §21 and §23: `.xfm` texts whose line 2 is neither an information line nor a
`Transform_Type` line, `%Generated by` lines with words holding both `src` and `dst`; label lists of 0
columns (FS-315), of no rows and ordinary ones, on three segmentation types; cache lines without a
number ending in each kind of white space or none, FS-314's two exceptions), and part 6a's `vres` (§25:
voxel sizes with NaNs, infinities, 0 and negatives; fills from the float table; flood fills of mostly
byte volumes from seeds -1 to the extent, thresholds and fill values outside 0 to 255; planes of every
orientation, unknown ones, planes -1 to 5; up- and downsampling of every type by N -1 to 3 and factors
0.5 to 4, -1 and NaN (no factor 0: §26), into no destination, one of the result's extent, of the
source's shape, of another type, or the source; the frame mean with frames -1 to 3; the limits; changes
of every type to every type with `f_low`, `f_high` 0 to 1, -0.5 and NaN, `no_scale` 0 to 2, with and
without `FS_FORCE_BIN_CHECK`), the fixed cases of `vrules6` (relay 10, §25: the clean room's questions
on part 6a: `MRIchangeType`'s histogram ends, `MRIdownsample2` into the source, `MRIfillFG` from seeds
past the depth and the height, `MRIextractPlane` into the source), part 6b's `vframes` (§27: splatting
into every type with values with NaNs, infinities, 1e10, 70000, frames past the volume's on lines of
their own; the frame-range samplers with codes -1 to 7, frame ranges empty, inside and past; the
trilinear kernel), `vderiv` (§28: the derivatives with sigmas 0, tiny, negative, ±1e300, NaN, FS-324's
two on lines of their own; the gradients; the eight neighbour counts up to three voxels outside) and
`vlinear` (§29: `MRIsetValues`, `MRIrasXformToVoxelXform`, the six linear transform functions with
methods 0 to 4 and 7, `MRIgetResampleMatrix`), and part 6c's `vresamp` (§31: resampling onto templates
on the source's grid scaled and moved, or random ones, types 0 to 4, 6, 7, the vote of 8 x 8 x 8 sources;
`MRIreplaceValues` and `MRIreplaceValuesUchar`), the fixed cases of `vrules7` (relay 11, §15 and §31:
`MRIindexNotInVolume` at rows whose rounding as a double and as a float differ; the vote of 513 x 3 x 1 and
511 x 3 x 1 byte sources of 100 to 130 frames onto one 512 mm voxel, 202 to 262 votes a label: FS-326),
`vstat7` (§33: the statistics and their print of every type, headers only, special floats; the type
names; box crops with starts and extents up to the `int` range; the centre of a sampled volume with the
diagnostic bits; orientation strings of 2 to 5 characters, valid ones in both cases and invalid
characters, NUL and 0x80; the 4 x 4 filler into none, 4 x 4, other sizes, complex; the voxel-to-RAS
and voxel-to-voxel matrices; `MRIsetVoxelToRasXform` with scaled, odd and NaN matrices and the variable
unset, a number, a word, empty), the fixed cases of `vrules8` (§33: statistics of one voxel, a NaN first
voxel, 64-bit integers, types not read, no voxels; orientation strings of 0 to 4 characters, every case,
NUL and 0x80; centres with factors that do not divide and of 0), `vbsvol` (§29, §31: the linear transforms with method 5 and `MRIresampleFill` with type 5, the cubic
B-spline), part 7b's `vimage` (§34: `MRItoImageView` of volumes with direction cosines that are mostly
signed permutations, every slice direction and view, slices negative, inside and past, frames -1 to one
past, no, a fitting or another given image; `MRItoImage` of bytes, ints, floats and other types into no or
a given image of every format and size; `ImageToMRI` of every pixel format, 0 to 3 frames, sizes 0, tiny
and NaN, headers that count other rows, columns or frames) and `vnbhd` (§35: the parcellation smoothing
on shorts with small labels and on other types, counts 0 to 27; the border planes; the window counts and
maxima with window sizes -2 to 7 and points up to two voxels outside; the nearest non-zero voxel at points
that round inside; the label means with label volumes of the source's or another extent; the positive
shift; the label borders; the label counts of a window, in it and in a plane; the label's box; boxes
filled across the edges; the masks; the clipping of bright white matter), §54's `vltapv` (transform arrays
changed from physical-voxel to RAS-to-RAS or to the same type, 0 to 2 transforms, geometries valid in most
cases, voxel sizes 0 and NaN, matrices of every kind, inverses without a matrix; fs.bspline's
`LTAtransformBSpline` with such arrays), the fixed cases of `vrules9` (§34, §35: the image view of 32-bit
integers that a float cannot hold, RGB images of 1 to 3 frames, headers that count more than their pixels,
the formats `ImageToMRI` refuses, a given image too small or too narrow for `MRItoImage`, negative labels
in the smoothing, bright white matter of values outside 0 to 255, windows more than 500 voxels outside and
the nearest non-zero voxel at points outside, on lines of their own, planes not supported), the fixed cases of
`vrules10` (§54: `LTAchangeType`'s six changes between the voxel, RAS-to-RAS and physical-voxel types with two
transforms and with none, singular geometries as source and destination, voxel sizes 0 and NaN, the validity
checks, the targets the three types do not support, arrays of other types, a diagnostic level), the fixed
cases of `vrules11` (§35: the label sums' NaN order, volumes of chosen NaN payloads), part 8's `vpvlab` (§37:
partial-volume counts of label volumes in blocks, intensity volumes of chosen values, border volumes given),
`vfr8` (§38: random and constant volumes, the normalisations, sorts, slice reversals, orders and permutations,
non-maximum suppression, padded extractions, root mean squares, volume fractions, distance-transform combinations,
difference lists, TIV estimates), `vmri2` (§39: frame powers, binarisations, reshapes, squares and roots, maxima,
constants, frame binarisations, votes, crops, slice reversals, end cuts, label lists, half-cosine weightings) and
the fixed cases of `vrules12` (§37 to §39: the NaN orders of the normalisations', the root mean square's and the
random draw's sums, the zero test, the integer stores past their range, the vote's runs, the defects' cases on
lines of their own) and of `vrules13` (§37: FS-471's negative label limits off and on the border), and fs.bspline's `vbspline` (the coefficients of volumes of the six numeric types, degrees 2
to 9 and 0, 1, 10, -1, given B-splines with fitting, other or no coefficients; sampling at points inside,
at and past the edges, NaN and infinite, frames -1 to one past; the frame-range sampler; coefficients
sampled as given with degrees -2 to 12; the linear, RAS and transform-array transforms; the pyramids into
new, fitting, other and source destinations) and `vbsrules` (lines of 80 and 20 samples for every degree,
degree 6 on 5 samples, NaN payloads, -0 only, a step of bytes, one-voxel axes, FS-962, FS-963, FS-964's
degrees 10 and 11, `MRIfreeBSpline`'s three cases, destinations of another extent): volumes of up to 4 x 4 x 4 voxels (5 for
`vextract` and `vres`, 6 for `vcoords`) and 0 to 3 frames of every type (the types without a voxel size
header-only), headers only too, destinations new, of the same shape, of another shape or a source,
cached matrices as allocated, recomputed or dropped.

## Results (the reference machine: glibc 2.35, Linux 6.8, x86-64)

- Relay 6 (2026-11-16): `refs`, seed 7, the prebuilt probe from commit `77392db5`: the hashes of
  the spec's §69.2, the same through the oracle-output cache cold, warm and without it; the probe
  as its own candidate: no unexplained difference in either mode.
- Relay 7 (2026-11-17): the fixed group `vrules` (84 calls), the same hash from the cache and
  without it, the other groups' hashes unchanged, the probe as its own candidate identical in both
  modes (log `oracles/logs/fs-volume-probe-r7-refs-2026-11-17.log`); `models.py fs291`: 127 lines as
  the model, none different, 50 memory or crash.
- Relay 7, part 4: the prebuilt probe rebuilt from commit `420314bb` (part 4's commands; its build
  in `BUILD_INFO.txt`, the same bytes twice): every earlier group's hash unchanged; part 4's groups
  `vsample`, `vmove`, `vskull`, `vltaread` (1,309 calls) the same through the cache cold, warm and
  without it; the probe as its own candidate: no unexplained difference in either mode;
  `models.py skull`: 160 of 160 lines as the model (log
  `oracles/logs/fs-volume-probe-r7-final-2026-11-17.log`).
- Relay 8 (2026-11-17): the prebuilt probe rebuilt from commit `9319981f` (part 5's commands,
  `ltareads`, standard error oriented before each call; its build in `BUILD_INFO.txt`, the same bytes
  twice; the 420314bb build kept in `<workspace>/archive/2026-11-17-fs-volume-probe-420314bb/`): every
  earlier group's hash unchanged but `vltaread`'s, whose only changed lines are its six missing
  `register.dat` cases (`errno` 2 and `No such file or directory` instead of 22 and `Invalid argument`,
  the 420314bb build's lines compared one by one); the new groups `vrules4` (94 calls) and `vcma`
  (377) the same cold, warm and without the cache (`vcma` twice without it); the probe as its own
  candidate: no unexplained difference in either mode; `models.py fs291` 127, `skull` 160 and `cma`
  95 lines as their models (logs `oracles/logs/fs-volume-probe-r8-final-2026-11-17.log`,
  `fs-volume-probe-r8-vcma-2026-11-17.log`). The loop lets a `vsample` line noting FS-300 differ in
  both modes (its window points FreeSurfer never reads).
- Relay 9 (2026-11-18): the fixed group `vrules5` (46 calls; the prebuilt probe from 9319981f
  unchanged) the same cold, warm and twice without the cache, every other hash unchanged; the probe as
  its own candidate identical in both modes (`vcma`, `vrules5`); two stand-in candidates (the probe's
  lines with FreeSurfer's aborts as `EXIT 134`, then as `EXIT 1`) tested the abort check: none and 9
  unexplained lines under `reproduce`, none with `fix` (log
  `oracles/logs/fs-volume-probe-r9-vrules5-2026-11-18.log`).
- Relay 9, part 6a: the prebuilt probe rebuilt from commit `53b6d7dc` (part 6a's commands; its build in
  `BUILD_INFO.txt`, the same bytes twice; the 9319981f build kept in
  `<workspace>/archive/2026-11-18-fs-volume-probe-9319981f/`): every earlier group's hash unchanged; the
  new group `vres` (830 calls) the same cold, warm and twice without the cache; the probe as its own
  candidate: no unexplained difference in either mode; `models.py fs291` 127, `skull` 160, `cma` 95
  lines as their models (logs `oracles/logs/fs-volume-probe-r9-final-2026-11-18.log`,
  `fs-volume-probe-r9-vres-2026-11-18.log`). The downsampler's cases use positive factors only: a
  negative or NaN factor makes FreeSurfer allocate index arrays of a size whose failure differs between
  processes (the hash would not be stable), and a factor of 0 would make the default ask for a volume
  2^31 - 1 voxels wide (spec §26).
- Relay 10 (2026-11-18): the prebuilt probe rebuilt from commit `20578b55` (part 6b's and 6c's commands;
  its build in `BUILD_INFO.txt`, the same bytes twice; the 53b6d7dc build kept in
  `<workspace>/archive/2026-11-18-fs-volume-probe-53b6d7dc/`): every earlier group's hash unchanged; the
  new groups `vrules6` (16 calls), `vframes` (404), `vderiv` (382), `vlinear` (360) and `vresamp` (272) the
  same cold, warm and twice without the cache; the probe as its own candidate: no unexplained
  difference in either mode; `models.py fs291` 127, `skull` 160, `cma` 95 lines as their models (logs
  `oracles/logs/fs-volume-probe-r10-final-2026-11-18.log` and, for `vresamp` once its sources mostly had a
  good RAS (an oblique source without one is refused before any sampling),
  `fs-volume-probe-r10-vresamp-2026-11-18.log`). Points that read memory the program does not
  own (frames past the volume's, `MRIneighborsOff` at a row or slice outside) and FS-324's two sigmas
  are on lines of their own: a crash or a time-out hides every point of its line.
- Relay 11 (2026-11-18): the fixed group `vrules7` (12 calls; the prebuilt probe from 20578b55) the same
  cold, warm and twice without the cache (log `oracles/logs/fs-volume-probe-r11-vrules7-2026-11-18.log`);
  then the prebuilt probe rebuilt from commit `d4b27c43` (fs.bspline's commands) and from `5d35ae1f` (part
  7a's commands; each build in `BUILD_INFO.txt`, the same bytes twice; the 20578b55 and d4b27c43 builds
  kept in `<workspace>/archive/2026-11-18-fs-volume-probe-20578b55/` and `-d4b27c43/`): every earlier
  group's hash unchanged; the new groups `vbspline` (410 calls), `vbsrules` (71), `vbsvol` (70), `vstat7`
  (481) and `vrules8` (32) the same cold, warm and twice without the cache; the probe as its own
  candidate: no unexplained difference in either mode; `models.py fs291` 127, `skull` 160, `cma` 95 lines
  as their models (logs `oracles/logs/fs-volume-probe-r11-final-2026-11-18.log`,
  `fs-volume-probe-r11-final2-2026-11-18.log`). The loop's both-modes allowance now names FS-300 in
  `vsample` and `vresamp`, FS-322 in `vderiv` and FS-325 in `vlinear`. A negative B-spline degree on a
  volume of one voxel (FS-964) is left out: its indices are stack contents whose detection changed in one
  of four reference runs.
- Relay 12 (2026-11-20): the prebuilt probe rebuilt from commit `0b5c7bb6` (part 7b's commands, `ltachg`,
  the image encoding and the integer fill; its build in `BUILD_INFO.txt`, the same bytes twice; the
  5d35ae1f build kept in `<workspace>/archive/2026-11-20-fs-volume-probe-5d35ae1f/`): every earlier group's
  hash unchanged; the new groups `vimage` (290 calls), `vnbhd` (441), `vltapv` (135) and `vrules9` (34) the
  same cold, warm and twice without the cache; the probe as its own candidate: no unexplained difference
  in either mode; `models.py fs291` 127, `skull` 160, `cma` 95 lines as their models (log
  `oracles/logs/fs-volume-probe-r12-final-2026-11-20.log`), `view` 76, `smooth` 22, `nearest` 40 and
  `physvox` 60 as theirs, none different (log `oracles/logs/fs-volume-probe-r12-models-2026-11-20.log`).
  Label volumes are zeros with small labels painted (a seeded fill would make nearly every point's own
  voxel non-zero); the nearest non-zero voxel's points round inside the volume (its direct read of the
  point itself outside is on lines of their own, `vrules9`).
- Relay 13 (2026-11-22): the prebuilt probe from `0b5c7bb6` unchanged; the new fixed group `vrules10` (65
  calls: §54's `LTAchangeType` between the voxel, RAS-to-RAS and physical-voxel types) the same cold, warm and
  twice without the cache, the probe as its own candidate identical in both modes (log
  `oracles/logs/fs-volume-probe-r13-vrules10-2026-11-22.log`); `models.py ltatypes`: 282 changed arrays as the
  model bit for bit, 19 ends of the program as the model, 17 at the inverses' fill, none different, from the
  cache and without it (log `oracles/logs/fs-volume-probe-r13-ltatypes-2026-11-22.log`). The new fixed group
  `vrules11` (28 calls: §35's NaN order of the label sums) the same cold, warm and twice without the cache,
  identical in both modes as its own candidate (log `oracles/logs/fs-volume-probe-r13-vrules11-2026-11-22.log`).
- Relay 13 (2026-11-22, later): the prebuilt probe rebuilt from commit `1536bbbb` (part 8's commands; its build
  in `BUILD_INFO.txt`, the same bytes twice; the 0b5c7bb6 build kept in
  `<workspace>/archive/2026-11-22-fs-volume-probe-0b5c7bb6/`): every earlier group's hash unchanged; the new groups
  `vpvlab` (90 calls), `vfr8` (490), `vmri2` (390) and `vrules12` (112) the same cold, warm and twice without the
  cache; the probe as its own candidate: no unexplained difference in either mode; every model as before, none
  different (log `oracles/logs/fs-volume-probe-r13-final-2026-11-22.log`). Part 8's generators keep every volume's
  cached matrices (FS-252 would end the line; `vrules12` holds it once).
- Relay 14 (2026-11-22): the prebuilt probe from `1536bbbb` unchanged; the new fixed group `vrules13` (13 calls: §37's
  FS-471 with negative label limits) the same cold, warm and twice without the cache, the probe as its own candidate
  identical in both modes (log `oracles/logs/fs-volume-probe-r14-vrules13-2026-11-22.log`). The loop lets a `vpvlab`
  line noting FS-469 differ in both modes (a label's mean read beside the table, the same in all four runs).
