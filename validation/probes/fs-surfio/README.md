# fs-surfio: FreeSurfer 8.2.0's surface structure helpers against the port

**Closed to FreeSurfer clean-room implementers** (it names FreeSurfer's functions; running
`fssurfio` is a black-box run of FreeSurfer's code).

- `fssurfio.cpp` → `<workspace>/oracles/freesurfer-8.2.0-probes/fs-surfio/fssurfio` (`build.sh
  <workspace>`): FreeSurfer's surface helpers (`mrisurf_aaa.h`, `mrisurf.h`, `mrisurf_base.h`,
  `mrisurf_topology.h`, `mrisurf_sphere_interp.h`, `mrisurf_base.cpp`'s status texts, `modVnum`,
  `MRISvertexCoord2XYZ_float`/`_double`) and the `MRIS` and `MRIS_MP` views, linked with the unit
  oracle's libraries as the official programs are; the headers' `__FILE__` mapped to the official
  build's source folder (read from the unit oracle's `libutils.a`), so failed assertions print the
  official paths. One line per case: every case runs in a child process whose standard output and
  error go to a scratch file beside the cases; `<result> | <text>` with the text C-escaped, or
  `EXIT <status> | <text>`, `CRASH <signal> | <text>`. 15 commands: `coord`, `vnum`, `nbr`,
  `modvnum`, `norm`, `close`, `proj`, `sign`, `status`, `sortcmp`, `angles`, `split`, `vec3`,
  `view` (every metric accessor of an `MRIS` and of an `MRIS_MP` loaded on it by hand, read and
  written through both views), `fixarea`; since relay 6 (2026-11-13) also `addcmd`
  (`MRISaddCommandLine`), `vhash` (`mrisVertexHash`, `mrisVertexVSize`), `diff`
  (`MRISdiffSimple`), `temps` (`MRIS_acquireTemp` & co), `fpv` (`MRISmakeFloatPerVertex`),
  `names` (the integrator's file names, `NEG_AREA_K`) and `surf` (a surface from the case, 1 to 5
  of 34 operations: `mrisurf_metricProperties.cpp`'s coordinate operations, the distances' storage
  of `mrisurf_base.cpp`, `MRISreverseFaceOrder`; then the surface's whole state). 22 commands;
  since relay 7 `metric` (a surface from the case, 1 to 6 of 33 operations: the averaging and
  smoothing, `MRIScenterSphere`, the neighbour distances' computation and checks, the percent
  distance error, the areas, the triangle corners and coordinate sets, the curvature operations,
  the saved coordinates; an operation that would read or write past what the case allocated
  prints `G` and is skipped on both sides), 23 commands; since relay 8 `posn` (a surface from
  the case with its face-normal cache, 1 to 6 of 21 operations: saving and restoring the vertex
  positions and normals, the canonical angles and mean canonical radius, the folding, the
  face-normal cache's reads, writes, deferral and recomputation, the original area, storing and
  restoring the metric properties, the old and current positions; then the surface's whole state;
  storing or restoring the metric properties past a buffer prints `G`), 24 commands; since relay 9
  (queue item 3.1-47 part a) `life` (a surface made by `MRISalloc` or `MRISoverAlloc`, 1 to 8 of
  its reallocations, count changes, frozen count, vertex, neighbourhood, distance and face
  settings, rip-flag operations, arrays, extra gradients, NaN check and dumps; then its state and
  `MRISfree`), `meta` (`MRIScopyMetadata` into a new or a given surface, with an `LTAalloc`
  transform array and the Talairach matrices), `misc` (the curvature names, the registration
  sigmas, `cprints`/`cprintd`/`cprintf`, `MRIS_vertexProgress_print`, the vector searches,
  `signum_eval`, the two sort comparisons) and `edges` (`edgesIntersect`), 28 commands; since relay 10
(queue items 3.1-51 and 3.1-52) `mprop` (a surface with a mesh's topology from the case: each
vertex's faces and corners, its 1- to 3-hop rings, distances with their storage, the faces' cache;
the switches `FREESURFER_OLD_`/`NEW_MRIScomputeMetricProperties`, `FS_FASTER_MP`,
`FS_WEIGHTS_NORMS_BY_ANGLE`, `FREESURFER_fastApproxAtan2f_use_atanf2`, `FREESURFER_reportLogicErrors`,
`FREESURFER_SEED`; `UnitizeNormalFace`, `fix_vertex_area`; then 1 to 4 of `MRIScomputeMetricProperties`,
`MRIScomputeTriangleProperties`, `MRIScomputeNormals`, `MRIScomputeAvgInterVertexDist` (with and without
the deviation; `G` where a vertex would read distances it has not), `MRISupdateEllipsoidSurface`,
`mrisOrientSurface`, `mrisOrientPlane`, `mrisComputeSurfaceDimensions`, `MRISsmoothSurface`,
`mrisDisturbVertices`, `MRISrecenter`, `MRISscaleBrainArea`, `MRISscaleVertexCoordinates`,
`MRISscaleBrain`; then the state), 29 commands; since relay 11 (queue item 3.1-47 part b) `sfile` (each
case in an empty work folder `<cases>.d` with a subfolder `d`: its input files, a surface from
`MRISoverAlloc` with every face place and the face-normal cache, `FS_POSIX`, `MRIS_READVAL_FRAME`,
`Gdiag`, `fix_vertex_area`; then 1 to 3 of `MRISsetReadFrame`, `MRISgetReadFrame`,
`MRISreadTriangleProperties`, `MRISwriteTriangleProperties`, `MRISwriteDists`, `MRISwriteAreaError`,
`MRISwriteAngleError`, `MRISwriteCurvatureToWFile`, `MRISreadValuesBak`, `MRISreadImagValues`,
`MRIScopyMarksToAnnotation`, `MRISmaxMarked`, `MRISwriteAscii`, `MRISwriteNormalsAscii`, `MRISwriteVTK`,
`MRISwriteCurvVTK` on names relative to the folder, each passed with two NUL bytes before it; `G` where
FreeSurfer would read memory it does not own; then the state and every file in the folder with its
bytes; the port's side in `tests/fs_surfio/sfile.rs`, in a temporary folder), 30 commands; since relay 12 (queue item 3.1-48 part a) `cfile` (as `sfile`: an empty work folder, the input files, a surface from `MRISalloc`, `FS_POSIX`, `FS_GII`, `Gdiag`; then 1 to 3 of `mrisReadAsciiCurvatureFile`, `mrisWriteAsciiCurvatureFile`, `__mrisreadcurvoldformat`, `MRISreadCurvatureVector`, `MRISreadNewCurvatureVector`, `MRISreadCurvatureIntoArray`, `MRISreadNewCurvatureIntoArray`, `MRISreadFloatFile`, `MRISreadBinaryAreas`, `MRISbuildFileName_read`, `MRISwriteDecimation`, `MRISreadDecimation`, `__MRISapplyFSGIIread`, `__MRISapplyFSGIIwrite`, `MRISreadVTK` into a new or the case's surface, `MRISwriteGeo`, `MRISwriteICO`, `getSRASToTalSRAS`, `MatlabPlotFace`, `MatlabPlotVertex`, `UnitizeNormalFace`, a volume of the values given to the readers that take one; `G` where FreeSurfer would write past its caller's buffer or read a number it never wrote; then the state and the folder's files; the port's side in `tests/fs_surfio/cfile.rs`), 31 commands; since relay 13 (queue items 3.1-48 part b and 3.1-49's STL) `sread` (as `cfile`, with `FREESURFER_reportLogicErrors` set one case in two: `mrisReadAsciiFile`, `mrisReadGeoFile`, `mrisReadGeoFilePositions`, `mrisReadTriangleFileVertexPositionsOnly`, `mrisReadTriangleFilePositions` through `io_statics.cpp`'s wrappers, and `quadCombine`; a surface a reader returns printed in full; the port's side in `tests/fs_surfio/sread.rs`) and `stl` (`mrisReadSTL` on text and binary STL files, `mrisWriteSTL` on the case's surface; the port's side in `tests/fs_surfio/stl.rs`), 33 commands; since relay 14 (queue item 3.1-48) `tri` (as `stl`, with the environment from the case: `TRIANGULARSURFACE_NOEXTRA_READ` and `_WRITE` together, `FS_SKIP_TAGS`, `USER`, `LOGNAME`, and always `FREESURFER_REPLACEMENT_FOR_CREATION_TIME_STRING`: `mrisReadTriangleFile` and `__MRISreadQuadrangleFile` through `io_statics.cpp`'s wrappers on generated files or on the files a writer of the case wrote, `MRISwriteTriangularSurface` and `__MRISwriteQuadrangleFile` on the case's surface with its volume geometry, Talairach matrix, group-average area and command lines; a surface a reader returns printed in full; the port's side in `tests/fs_surfio/tri.rs`), 34 commands; since relay 15 (queue item 3.1-48) `fnam` (an empty work folder with `d`, `d/s` and `label`, `FS_POSIX` and `FS_GII` from the case, up to 5 curvature, GIFTI and annotation files, a surface from `MRISalloc` with a hemisphere and a name; then 1 to 4 of `MRISfileNameType`, `MRISwriteCurvature_getfilename`, `MRISreadCurvatureFile_getfilename`, `MRISwriteAnnotation_getfilename`, `MRISreadAnnotation_getfilename` with and without a surface, each answer and the chosen name; the port's side in `tests/fs_surfio/fnam.rs`) and `swr` (as `tri`, with `TRIANGULARSURFACE_NOEXTRA_WRITE`, `USER`, the creation time's replacement and `SURFER_SIDEDOOR` (FreeSurfer's licence check skipped, as the port needs no key): `MRISwrite` in every format but GIFTI, `MRISwriteVertexLocations`, `MRISwritePatchAscii`, `MRISreadVerticesOnly` on a surface with white and pial sets, rip flags and borders; then its current coordinates and the folder's files; the port's side in `tests/fs_surfio/swr.rs`), 36 commands; since relay 16 (queue item 3.1-48 with fs.surf's pieces taken early) `srd` (an empty work folder with `d`, `Gdiag`, the environment from the case: `mrisCompleteTopology_new`, `MRISfindNeighborsAtVertex_new` and `_old`, `FS_SKIP_TAGS`, `FS_GII=.gii`, `FREESURFER_always_check_vv`, `FS_FASTER_MP`, and always `SURFER_SIDEDOOR`; one or two input files, closed and open meshes in every format a name chooses (binary triangle with or without tags, each quadrangle form, ASCII, movie.byu, STL, VTK, the icosahedron format) or another reader's random file, now and then the same file by a name of 300 or more bytes; then one or two `MRISread` (with or without the conversion to tkregister space) or `MRISfastRead`, each surface printed in full; the port's side in `tests/fs_surfio/srd.rs`; since relay 17 every work folder also holds `mri/transforms`, a third of the cases a `talairach.xfm` there (MINC's text in many forms, or random bytes) and the transform printed with the surface), 37 commands; since relay 17 (queue item 3.1-50 part b) `mpb` (a binary triangle file of `srd`'s meshes read by `MRISread`, the original area set or not, per-vertex and per-face overrides, then 1 to 6 of the common vertices and edge of two faces, the tangent planes and their store, the plane fit, the face centroids, the face-area, vertex-spacing and total statistics, the negative-face counts, `MRISprintTessellationStats`, the closest vertices, the curvature edits with `MRIShistoThresholdCurvature`; the port's side in `tests/fs_surfio/mpb.rs`), 38 commands; since relay 18 (queue item 3.1-50 part b, second and third groups) `mpc` (as `mpb`, with `FREESURFER_SEED` and `FS_DISTURB_DISTANCES` from the case, the status set or not, `MRISmakeDistOrig` for every vertex, per-vertex overrides of the original, canonical, pial, white and current positions, values, principal curvatures, normals, white normals, steps, areas, marks, `fixedval` and up to 8 original and current distances, per-face rip flags and areas; then 1 to 6 of the barycentric coordinates (on a face, of any triangle) and `project_point_to_plane`, the face samples, `MRISrmsTPHeight`, the analytic distances, `MRIScomputeHausdorffDistance`, the curvature ratio and contrast, `MinDistToTriangleBF`, the stuffed coordinates, the vertex normals of each set, `MRIScomputeSurfaceNormals`, the closest-vertex searches, `MRISmeasureCorticalThickness`, the curvature bookkeeping and `containsAnotherVertexOnSphere`; the port's side in `tests/fs_surfio/mpc.rs`), 39 commands; since relay 19 (queue item 3.1-50 part b) `mpd` (icosahedra, grids and `srd`'s meshes read by `MRISread`, per-vertex and per-face overrides, `BEVIN_TESTING_dumpFacesAroundVertex`; then 1 to 3 of the discrete curvatures, the faces' order around the vertices, the curvature sign and angle of two faces, the face angles, the pseudo-normal, the face-place helpers and the marks; the port's side in `tests/fs_surfio/mpd.rs`), 40 commands; since relay 20 (queue item 3.1-51's rest) `mpe` (as `mpd`, with the status, the curvature extremes, `Gdiag_no` and `FREESURFER_MRISaverageGradients_optimize_suppress` given before the read and `Gdiag`'s write bit; per-vertex overrides of rip flags, marks, positions, normals, curvatures, `k1 k2`, `K H`, areas, original positions, gradients, `val val2 origarea`, per-face rip flags, areas, cached original areas and normals; then 1 to 4 of the smoothed normals, the mean curvature, the curvature choices, the curvature indices, `MRISmaxRadius`, the curvature's mean and spread, the negative faces' marks and rips, the second fundamental form (thresholded or not, its files by length and FNV-1a hash) and at one vertex, the three gradient averagings, the rescaling, `MRISnormalize`, `MRISupdateSurface`, the orientation check, `mrisCheckSurface`, `MRISsubsampleDist`, `MRISextendedNeighbors`, `MRISunion` with a clone, `MRISclone` and both vertex reports; the port's side in `tests/fs_surfio/mpe.rs`), 41 commands. Since relay 21 (queue item 3.1-26's surface part) `gii` (an empty work folder with `d`, `Gdiag`'s show and verbose bits, 1 or 2 generated GIFTI files (surfaces with metadata and coordinate systems, label tables, shape, label, vector, colour, matrix and statistics arrays, sparse storage, ASCII, Base64 and gzip-Base64 encodings, both index orders, broken forms; now and then random bytes), a surface kept from `MRISalloc`, a caller's colour table; then 1 to 3 `mrisReadGIFTIfile` with or without the kept surface, the overlays and the colour table, each result printed in full with the overlays; built with `packages/gifti` on the include path; the port's side in `tests/fs_surfio/gii.rs`; `srd` reads GIFTI surfaces too), 42 commands; `check.py` writes the official build's addresses in gifticlib's `** UPB: bad params (...)` message as the port's placeholders. Since relay 22 (queue item 3.1-26's writers and GIFTI volumes) `gwr` (an empty work folder with `d` and `mri/transforms`, `Gdiag`'s show and verbose bits, `FREESURFER_REPLACEMENT_FOR_CREATION_TIME_STRING` and `SURFER_SIDEDOOR`, 0 to 2 of `gii`'s GIFTI files and a `talairach.xfm` one case in three, a surface from `MRISalloc` with per-vertex coordinates, curvature, `stat` and annotation, faces, its name, subject, patch flag, real RAS, volume geometry, centre, group-average area, command lines, Talairach matrix and colour table, 1 to 3 volumes; then 1 to 4 of `MRISwriteGIFTI` (every intent kind), its volume form, `MRISwriteGIFTICombined`, `mriWriteGifti`, `MRISwrite` of a `.gii` name, `getShapeStatIntentCount`, `MRISreadGiftiAsMRI`; each written file printed with its `UserName` value written `U` and each `<Data>` payload Base64-decoded and inflated (`gii_normal`: zlib 1.2.11's compressed bytes here, zlib-rs's in the port, the values the same); the volume read printed in full; built with zlib's header and library; the port's side in `tests/fs_surfio/gwr.rs`), 43 commands; `check.py` also writes the official address in fs.io's `gifti_get_DA_value_2D` message (FS-214) as `(nil)`, the port's. Since relay 23 (queue item 3.1-51's leftovers) `mpf` (a binary triangle file of `mpe`'s surfaces read by `MRISread`, 0 to 3 vertices ripped, `FREESURFER_mriSurfaceRASToVoxel_both` and `_old` from the case, 1 to 3 volumes of one voxel per vertex (`MRI_FLOAT`, now and then `MRI_UCHAR`, `MRI_INT` or `MRI_SHORT`, now and then a voxel more or fewer); then 1 to 3 of `MRIScrsLUT`, `MRISar1`, `MRISarN`, `MRISsmoothKernel` (its hop lists kept between calls, its output new, a volume of the case written in place, or the input), `SetSurfHopList` and `mriSurfaceRASToVoxel` over `VoxelFromSRASmatrix` (unset, or a matrix of the case), each volume printed in full with the vertices' `undefval`; the port's side in `tests/fs_surfio/mpf.rs`), 44 commands; both sides write `MRISsmoothKernel`'s elapsed times and its closing value (column 1031 of its output: memory outside a small volume, FS-527) as `<t>` and `<v>` (`untime` in `fssurfio.cpp` and in the port's runner). `io_statics.cpp` builds `mrisurf_io.cpp` into both probes with that file's own compile command from the unit oracle's `compile_commands.json`, so the probe reaches its file-local functions (`__mrisreadcurvoldformat`, the curvature arrays, the `FS_GII` helpers); every `mrisurf_io.cpp` function then runs from that object (relay 11's seed-1 hash unchanged with it). `validation/investigations/glibc-scanf-probe/scanf_errno.c` (open) measures glibc's `sscanf`/`fscanf` return values and `errno` at the end of the input, the model of `src/scanf.rs`. `build.sh` also builds
  `fssurfio-mpstub`: the same probe with a stub `MRIScomputeMetricProperties` (it writes `MP`),
  linked first with `--allow-multiple-definition` so the library's own call (`MRISsmoothSurface`'s
  last step) reaches it; `check.py` runs `metric`'s cases through it.
- The port's side: `crates-freesurfer/larmorx-freesurfer-surf/examples/fs_surfio_probe.rs` (`cases
  <seed> <n>`, `run <cases> fix|reproduce`; the generator and the answers in
  `tests/fs_surfio/cases.rs`, shared with the hash test `tests/fs_surfio_official.rs`).
- `check.py --seed <s> --n <n> [--json out]`: the cases through FreeSurfer by the oracle-output
  cache at one thread, the port in both modes, the comparison (`reproduce` identical; `fix`
  different only with a notice), and the hash the test checks. Run it inside
  `scripts/orchestration/boxed.sh`.

Measured 2026-11-13 (relay 16, 37 commands): `srd` alone, seeds 7 (1,000 cases), 13 and 17 (2,000
each): every comparable case identical with `reproduce` (273, 526 and 535 counted, not compared: FS-121's
unwritten normals, the memory-dependent FS-357 to FS-362, the other readers' families; 1, 2 and 3 cases
past the official probe's 5-second alarm, all of FS-354's or FS-151's families), `fix` different only with
notices; 2,712 surfaces read and printed in full among them, every reader and both environment
switches reached (failed assertions where FreeSurfer's code asserts: a surface without edges,
`mrisCompleteTopology_new` on a closed surface, the old search). The whole probe at seed 7, 7,400 cases:
7,244 identical with `reproduce`, 156 counted, not compared, 0 different; `fix` different only with
notices (and 5 `posn` cases of correctly rounded maths). Seed 1, 1,110 cases: 1,078 identical, 32
`SKIP`, hash `31bb549125934b62c47bd5df2e01bbd2e6852c51cef2bac3fad5d4a51b90f1b1` (the test), passing in
debug and release, plain and under the AArch64 NaN emulation (3,518 and 21,790 flips); `srd`, `tri`,
`stl` and `sread` at seed 7 identical under the emulation in both modes. Logs:
`oracles/logs/fs-surfio-relay16-*`.

Measured 2026-11-13 (relay 17, 38 commands): `srd` with the transforms, seeds 7 (1,000 cases), 13 and 17
(2,000 each): 0 differ with `reproduce` after one fix (the STL reader's failed open now leaves `errno`);
46 of seed 7's first reads hold a transform; hand cases (the 27 seed-7 surfaces with a valid geometry
in `d/`, each with eight transforms, `Gdiag` off and on): 432 cases, 0 differ, 80 counted (FS-121),
every branch of the transform's completion reached. `mpb`, seeds 7 (1,000), 13 and 17 (2,000 each):
0 differ with `reproduce` (82, 159 and 152 counted, not compared: FS-121, FS-123 of the metric
properties), the default different only with notices; the official build's operand orders and its
`errno` (glibc's `sqrt`/`sqrtf` for a negative variance, the vertex-spacing statistics' discarded
scale, the metric properties' inter-vertex deviation) were taken from the object code and gdb.

Measured 2026-11-13 (relay 19, 40 commands): `mpd` (the discrete curvatures, the faces' order around
a vertex, the pseudo-normal), seeds 7 (1,000 cases), 13 and 17 (2,000 each): 0 differ with
`reproduce` (61, 104 and 113 counted, not compared: FS-372's walks around vertices whose faces are
not one fan, FS-121 of the read); the default differs from the official build only by the arc
cosine (`acosf`, correctly rounded by default: 84, 193 and 191 cases, `fix_libm`; with glibc's
`acosf` put into the default by hand, 0 of the 1,000 seed-7 cases differ). `srd` with the volumes
a transform names (`orig.mgz`, `t1.mnc.mgz`, `v.mnc.mgh`, MGH files) and a regular `mri/orig`:
seeds 7, 13 and 17 (2,000 each): 0 differ with `reproduce` (one seed-17 case counted: FS-76, an MGH
header without voxels, which the official build turns into a crash), 66 compared surfaces with a
geometry read from a volume header, 67 reads of `mri/orig` that cannot determine its type. The whole
probe at seed 7 (8,000 cases): 7,803 identical, 197 counted, 0 different (the default: only notices
and the libm cases, `mpd` 11 and `posn` 8); seed 1 (1,200 cases): 1,161 identical, 39 counted, hash
`fef1c552...`, passing in debug and release and under AArch64's NaN emulation (3,576 and 43,861 NaNs
flipped); the seed-7 `srd`, `mpd`, `mpc`, `tri` and `stl` outputs identical under the emulation but
for relay 18's two `mpc` `dd` cases (fs.core's `randomNumber`, reported). The work folder's absolute
path is written `<w>` in both probes' outputs (a volume reader's message names it).

Measured 2026-11-13 (relay 18, 39 commands): `mpc`, seeds 7 (1,000 cases), 13 and 17 (2,000 each): 0 differ
with `reproduce` (34, 59 and 62 counted, not compared: FS-121 of the read, FS-367's endless loops ended by
the probe's 5-s alarm, FS-370's arrays), the default different only with notices (FS-366, FS-369 and the
errors' FS-01/FS-03). The whole probe at seed 7 (7,800 cases): 7,606 identical, 194 counted, 0 different;
seed 1 (1,170 cases): 1,137 identical, 33 counted, hash `90fbb5e2...`, passing in debug and release and under
AArch64's NaN emulation. Under the emulation one seed-7 `mpc` case differs (`MRISdisturbOriginalDistances`
with an infinite percentage: fs.core's `randomNumber` makes `inf - inf` in plain arithmetic, so its NaN takes
the platform's default sign; reported to fs.core's track). The official build's operand orders were read from
`mrisurf_metricProperties.cpp.o` and `tritri.cpp.o`.
Seed 1: 1,140 cases, 1,116 identical, 24 counted, hash `209163ba486662e79d7931add4d23f34aec8757e0fc8d377a0386266e91e76d3`.

Measured 2026-11-13 (relay 15, 36 commands): `fnam` alone, seeds 7 (1,000 cases), 13 and 17 (2,000
each): every comparable case identical with `reproduce` (14, 37 and 40 counted, not compared: FS-356),
`fix` different only with notices; `swr` alone, seeds 7 (1,000), 13 and 17 (2,000 each): every
comparable case identical (9, 13 and 11 counted, not compared), `fix` different only with notices
(the first run found that `mrisWriteSTL`'s failed open leaves `errno` for later messages: fixed in the
port); `stl` seed 7 unchanged. The whole probe at seed 7, 7,200 cases: 7,090 identical with
`reproduce`, 110 counted, not compared, 0 different; `fix` different only with notices (and 7 `posn`
cases of correctly rounded maths). Seed 1, 1,080 cases: 1,064 identical, 16 `SKIP`, hash
`c2ea12117f4b024167aed9c765687f2ca1a6c4ee4155f6c5538f657b82d33eca` (the test), passing in debug and
release, plain and under the AArch64 NaN emulation (1,160 and 10,285 flips); `fnam`, `swr`, `tri`
and `stl` at seed 7 identical under the emulation in both modes. Logs:
`oracles/logs/fs-surfio-relay15-*`.

Measured 2026-11-13 (relay 14, 34 commands): `tri` alone, seeds 7 (1,000 cases), 13 (1,000) and 17
(2,000): every comparable case identical with `reproduce` (36, 54 and 80 counted, not compared), the
default different only with notices. After fs.core's `cscan::Scanf` and `fnv` replaced the crate's own
copies: seed 7, 13,600 cases: 13,408 identical with `reproduce`, 192 counted, not compared (74 `mprop`,
40 `stl`, 35 `sread`, 24 `sfile`, 12 `tri`, 7 `cfile`), 0 different; `fix` differs only with notices or
the correctly rounded maths (11 `posn`, 3 `surf`). Seed 1, 1,020 cases: 1,006 identical, 14 `SKIP`, hash
`2f2c8b35e113ed57bd95020d54b261cce294d85550d1894bc5805c06ed2072f1` (the test), passing in debug and
release with the emulation (1,779 and 17,035 flips); `tri` and `sread` seed 7 identical under the
emulation in both modes. Logs `oracles/logs/fs-surfio-relay14-seed1.log`, `-seed7-switch.log`,
`-nantrap.log`.

Measured 2026-11-13 (relay 13, 33 commands): seed 7, 66,000 cases: 65,018 identical with `reproduce`,
982 counted, not compared (435 `mprop`, 198 `stl`, 151 `sread`, 115 `sfile`, 81 `cfile`, 2 `metric`);
`fix` differs only with notices or the correctly rounded maths (65 `posn`, 7 `surf`); the port's outputs
identical under AArch64's NaN emulation in both modes (74,230 and 74,272 NaNs flipped). `sread` alone:
seed 7, 1,000 cases and seed 13, 2,000; `stl` alone: seed 7, 1,000: every comparable case identical with
`reproduce`, the default different only with notices. Seed 1, 990 cases: 980 identical, 10 `SKIP`, hash
`2543d2708fc953a9ec60e5dff31e85b7799e1a20285c2514b338e30f7b7abdaf` (the test), passing in debug and
release with the emulation (7,746 and 17,227 flips). Log `oracles/logs/fs-surfio-relay13-seed7.log`.

Measured 2026-11-13 (relay 12, 31 commands): seed 7, 62,000 cases: 61,334 identical with `reproduce`,
666 counted, not compared (455 `mprop`, 125 `sfile`, 81 `cfile`, 5 `metric`); `fix` differs only with
notices or the correctly rounded maths (71 `posn`, 4 `surf`); the port's outputs identical under
AArch64's NaN emulation in both modes (67,516 and 67,552 NaNs flipped). `cfile` alone, seeds 7, 13 and
17: 2,000 cases each, every comparable one identical (before the plots' operand orders were read from
the object code, 13 of the 6,000 differed, all in `MatlabPlotFace`/`MatlabPlotVertex`; before the
measured `scanf` end-of-input rule, 1 of seed 17's). Seed 1, 930 cases: 919 identical, 11 `SKIP`, hash
`17da05597cae13550e267517b2858b9932266b37ca7d64ab7894779a948bc7b6` (the test), passing under
`NAN_TRAP_ARM=1` with `NAN_TRAP_DIR` in debug and release (2,897 and 12,690 flips). Log
`oracles/logs/fs-surfio-relay12-seed7.log`.

Measured 2026-11-13 (relay 11, 30 commands): seed 7, 60,000 cases: 59,497 identical with `reproduce`,
503 counted, not compared (404 `mprop` cases of FS-121, FS-123, FS-06; 99 `sfile` cases of FS-167, the
readers' memory digits, and FS-06); `fix` differs only with notices (FS-126..FS-133 among them) and in 5
`surf` and 77 `posn` cases (`fix_libm`); the port's outputs byte-identical with and without AArch64's
NaN emulated (70,295 and 70,313 NaNs flipped), both modes (log
`<workspace>/oracles/logs/fs-surfio-relay11-seed7.log`). `sfile` alone, seed 7: 1,000 cases, every
comparable one identical (49 counted, not compared). Seed 1, 900 cases: 894 identical, 6 `SKIP`, hash
`d927ce4b6aebe9a1f899c92080b8c11b8d45a50b15208a50043e8cb464ca45ef`, with the emulation too (1,925 and
13,214 NaNs flipped), debug and release.

Measured 2026-11-13 (relay 10, 29 commands): seed 7, 58,000 cases: 57,557 identical with `reproduce`,
443 counted, not compared (440 `mprop` cases where FreeSurfer reads memory it never wrote or does not
own: FS-121, FS-123, or the fast arctangent's table outside its range, FS-06: `check.py`'s `SKIP`; 3
`metric` cases of FS-114); `fix` differs only with notices (FS-01, FS-03, FS-12, FS-111, FS-113,
FS-115..FS-120, FS-122, FS-124; FS-120 only until relay 11 made it kept by default, rule D: the
correction on request) and in 4 `surf` and 70 `posn` cases (`fix_libm`); the port's outputs
byte-identical with and without AArch64's NaN emulated (69,001 and 69,038 NaNs flipped), both modes
(log `<workspace>/oracles/logs/fs-surfio-relay10-seed7.log`). `mprop` alone, seeds 11, 13 and 17:
10,000 cases, every comparable one identical (1,729 counted, not compared); the default different
only with notices. Seed 1, 870 cases: 865 identical, 5 `SKIP`, hash
`16d6b5a0f4b53fe3a13a05708f5cdcd842a026a4ec1737698d1b7f9008125e3b` (the test, which hashes the
`SKIP` cases as `SKIP` as `check.py` does), with the emulation too, debug and release.

Measured 2026-11-13 (relay 9, 28 commands): seed 7, 56,000 cases: all identical with `reproduce`
(4,584 of them failed assertions, FS-03; 63 FS-117, 319 FS-118, 16 FS-119); `fix` differs only with
the notices of FS-01, FS-03, FS-12, FS-111, FS-113, FS-115, FS-116, FS-117, FS-118 and FS-119, in 4
`surf` cases that rotate and in 51 `posn` cases that compute the canonical angles (`fix_libm`); the
port's outputs byte-identical with and without AArch64's NaN emulated (3,904 and 3,926 NaNs
flipped), both modes (log `<workspace>/oracles/logs/fs-surfio-relay9-seed7.log`). Seed 1, 840
cases: all identical, hash `83e0ed801db07e93238b22842a6e01f0a8741c6b03241e10a390b6f89fda4096` (the
test), with the emulation too, debug and release. (`misc`'s first run differed on `ss 0 0` cases:
the probe's own empty `std::vector` gave `MRISsetRegistrationSigmas` a NULL list, which resets the
defaults; the probe now allocates one more place.)

Measured 2026-11-13 (relay 8, 24 commands): seed 7, 48,024 cases: 48,021 identical with
`reproduce`, 3 counted, not compared (FS-114, as below); `fix` differs only with the notices of
FS-01, FS-03, FS-12, FS-111, FS-113, FS-115 and FS-116, in 7 `surf` cases that rotate and in 76
`posn` cases that compute the canonical angles (the default's correctly rounded `atan2f`: `fix_libm`
counts both); the port's outputs byte-identical with and without AArch64's NaN emulated (32,380
NaNs flipped), both modes. Seed 13, 10,000 `posn` cases alone: all identical with `reproduce` (602
of them FS-116's deliberate crash, `CRASH 11`). Seed 1, 720 cases: all identical, hash
`15533dfff68f232f97a985423d16c9fc84d53dcc5b71b99be897d3e66856a825` (the test), with the emulation
too, debug and release. (A first seed-7 run with `--n 48000` overlapped a second one on the same
cases file, so the two `fssurfio` processes shared `<cases>.out`: its oracle-cache entry is
disputed and never served; never run two probes on one cases file at once.)

Measured 2026-11-13 (relay 7, 23 commands): seed 7, 46,000 cases: 45,997 identical with
`reproduce`, 3 counted, not compared (FS-114: FreeSurfer's `MRIScenterSphere` never returns on a
surface with no vertex, ended by the probe's 5-second alarm, `CRASH 14`, against larmorx's
refusal); `fix` differs only with the notices of FS-01, FS-03, FS-12, FS-111 and FS-113 and in 4
`surf` cases that rotate; the port's outputs byte-identical with and without AArch64's NaN
emulated (3,033 NaNs flipped), both modes. Seed 1, 690 cases: all identical, hash
`c46c6b80bd514c2b850e54fadeadae38f056c31954a9bed6a97d93ed8e04fefe` (the test), with the
emulation too, debug and release. Before the official operand orders were read from the object
code (`docs/findings/freesurfer-surfio.md` §9), 31 of 2,000 NaN-heavy `metric` cases of seed 13
and 1 of 10,000 of seed 17 differed; after, none.

Measured 2026-11-13 (relay 6, 22 commands): seed 7, 44,000 cases: all identical with `reproduce`;
`fix` differs only with the notices of FS-01, FS-03, FS-12, FS-111 and FS-113 and in 3 `surf` cases
that rotate (the default's correctly rounded `sinf`/`cosf`, `fix_libm`); the port's outputs
byte-identical with and without AArch64's NaN emulated (200 NaNs flipped). Seed 1, 660 cases: all
identical, hash `6803ed271fff9ae3fe95d6dc7ee1f86b34d4b362f52515c50d9a42cdcbebd4a6` (the test), with
the emulation too, debug and release. Seed 11: 4,000 `surf` cases identical (34 differed before the
NaN operand orders were read from the object code: `docs/findings/freesurfer-surfio.md` §8).
`addcmd` past 1,000 lines prints only the message (FreeSurfer then writes a pointer over the count);
`surf`'s generator keeps every distance buffer at least the vertex's size in use and never grows a
vertex with no buffer whose `vtotal` is positive (memory FreeSurfer never writes).

Measured 2026-11-13 (relay 5): seed 7, 30,000 cases: all identical with `reproduce`; `fix` differs only with
the notices of FS-01, FS-03 and FS-12. Seed 1, 450 cases: all identical, hash
`0cbc85857870611ee541f61cfe41040af6675957c02a27b57eb9825576f93358`, also with AArch64's NaN emulated
(`validation/probes/fsl-mcflirt-affine-trace/nan_trap.c`, `NAN_TRAP_ARM=1` with `NAN_TRAP_DIR`) in
debug and release; the 30,000 cases' port outputs are byte-identical with and without the emulation
in both modes. The scale of `vec3` is generated finite: where two NaNs meet in a header helper the
operand order is each caller's compiled code (`docs/findings/freesurfer-surfio.md` §5).

Measured 2026-11-13 (relay 22, 43 commands): `gwr` alone, seeds 7, 13 and 17 (1,000 cases each):
every comparable case identical with `reproduce` (FS-521's 221 cases, memory FreeSurfer never wrote,
identical too and counted, not compared, by `check.py`), `fix` different only with notices. The whole
probe at seed 7, 8,600 cases: 8,343 identical with `reproduce`, 257 counted, not compared, 0
different; `fix` different only with notices and correctly rounded maths (`mpd` 17, `posn` 6). Seed 1,
1,290 cases: 1,253 identical, 37 `SKIP`, hash
`6eee3fd8f2ff1151c36a81a3f748dcf290272db722225b2274f55572a304decd` (the test), passing in debug and
release, plain and under the AArch64 NaN emulation (1,886 and 59,460 flips); the port's seed-7
`gwr`, `gii` and `srd` outputs identical with and without the emulation in both modes.

Measured 2026-11-14 (relay 23, 44 commands): `mpf` alone, seeds 7, 13 and 17 (909 cases each):
every comparable case identical with `reproduce` (FS-534's cases, memory past a volume, counted, not
compared), `fix` different only with notices. The whole probe at seed 7, 8,800 cases: 8,528
identical with `reproduce`, 272 counted, not compared, 0 different; `fix` different only with notices
and correctly rounded maths (`mpd` 15, `posn` 6). Seed 1, 1,320 cases: 1,282 identical, 38 `SKIP`,
hash `d09e84accbd867aaf99fcb4d8d0516924b60bf0d7f3ae04d851b083b1009ad2f` (the test), passing in debug
and release, plain and under the AArch64 NaN emulation (2,344 and 92,611 flips); the port's seed-7
`mpf`, `gwr` and `gii` outputs identical with and without the emulation in both modes. The GIFTI
volume functions' move to `larmorx-freesurfer-io` (relay 23) changed no output: relay 22's seed-7
`gwr`, `gii` and `srd` outputs identical in both modes (2,790 cases).

```
validation/probes/fs-surfio/build.sh <workspace>
scripts/orchestration/boxed.sh 12 python -P validation/probes/fs-surfio/check.py --seed 7 --n 66000
```
