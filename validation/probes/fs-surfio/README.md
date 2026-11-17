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

Relay 24 (queue item 3.1-51's leftovers, second group; the 45th command `mpg`: the edge and corner
tables, the face, edge and corner metrics, the triangle, edge and corner statistics, the edge
overlay, the two quality summaries, the curvature averaging and the middle of the grey matter;
`specs/freesurfer-surfio.md` §117): `mpg` at seeds 7, 13, 17 and 5 (888 cases each, from 40,000
cases of the whole probe): 0 different with `reproduce` beyond 3, 5, 5 and 5 lines whose only
differences are glibc's `acos` last bits (at most 2 units in the last place; `check.py` counts them
apart as `reproduce_differ_acos_last_bits`, GLIBC-07); 15 to 19 counted (fs.mri's FS-53, FS-121,
FS-06); `fix` different only with notices (FS-535 to FS-538) and those `acos` bits. Seed 7, 9,000
cases of the whole probe: 8,712 identical, 288 counted, 0 different. Seed 1, 1,350 cases: 1,309
identical, 41 `SKIP`, hash `6a44bcf748d45fc2be9fbcfb07335b04a355090e528bf8db73b40258f0e84b1c` (the
test).

Relay 32 of track D (the 46th command `fli`: `mrisurf_mri.cpp`'s surface-to-voxel maps and cached
conversions, `mriflood.cpp`'s shell, ribbon, bounding volume and interior; `specs/freesurfer-surfio.md`
§118-119, `specs/freesurfer-filter.md` §33-34): one or two surfaces read by `MRISread` (icosahedra,
grids, `srd`'s meshes scaled into 8 mm, the second often the first shrunk), each with a volume
geometry of the case, 1 to 3 volumes, then 1 to 4 operations (the four conversions, a map made,
used, loaded again and used, the shell, the ribbon, the bounding volume, the interior, the
bounding volume filled), each printing its coordinates' bits or the volume (extent, type, frames,
geometry, `ras_good_flag`, the voxels as runs `<bits>x<n>` and `E`); `MRISfillInterior`'s elapsed
time `<t>` on both sides. `fli` at seeds 7, 13 and 17 (100 cases each): 0 different with
`reproduce`; `fix` different only with notices (FS-604, FS-605, FS-608). Seed 7, 9,200 cases of the
whole probe: 8,920 identical, 280 counted, 0 different. Seed 1, 1,380 cases: 1,346 identical, 34
`SKIP`, hash `a20e3b9d9389f40f67a68a3b3a976aeee565c0181c5c131f76b84ae62e2f7098` (the test, from the
clean commit a95f764d), passing in debug and release, plain and under the AArch64 NaN emulation
(5,233 and 157,727 flips), and on the Windows laptop (`windows-cross-check`); the port's seed-7 `fli`
outputs (200 cases) identical with and without the emulation in both modes. The new case mix found
one difference before that commit: an error after a failed GIFTI read printed `No such file or
directory` where FreeSurfer prints nothing (glibc's `getpwuid` in `insertCommonMetaData` sets
`errno` to 0; fixed in a95f764d). `fssurfio`'s alarm is 900 seconds for `flr` lines (5 for the
rest).

Relay 33 of track D (the 47th command `bvl`: the rest of queue item 3.1-51, the signed distance volume,
the label masks, the laminar fractions and the outside distance map; `mpg` gained `mke`, the edge marks;
`specs/freesurfer-surfio.md` §120-121, `specs/freesurfer-filter.md` §35-36): a surface read by `MRISread`
with its white positions and pial positions (a second surface of the same mesh, or the white ones), a
volume geometry, values set from the vertex numbers, a few vertices marked, a label of vertex numbers,
1 or 2 volumes (bytes mostly; zeros, a pattern or a ball of one label about the centre), then 1 to 3
operations. `bvl` at seeds 7, 13 and 17 (200 cases each): 0 different with `reproduce` (16, 11 and 15
counted: FS-614, where FreeSurfer reads memory beyond the label volume and crashes); `fix` different only
with notices (FS-608 to FS-614). `mpg` with `mke` at seeds 7, 13 and 17 (200 cases each): 0 different
beyond one line per seed of glibc's `acos` last bits. Seed 7, 9,400 cases of the whole probe: 9,101
identical, 298 counted, 1 of `acos` last bits, 0 different. Seed 1, 1,410 cases: 1,371 identical, 39 `SKIP`,
hash `be9281e6ce75fecf6a59e4429cbadf28616bf09e2b30c94b2bcaf841006ca1fd` (the test, from the clean
commit ff190548), passing in debug (5.1 s) and release (0.6 s), plain and under the AArch64 NaN
emulation (58,453 and 334,654 flips), and on the Windows laptop; the port's seed-7 `bvl` and `mpg`
outputs (400 cases) identical with and without the emulation in both modes. Two findings on the way:
FreeSurfer's fast march takes its first root before testing it, so glibc's `sqrtf` of a negative
argument sets `errno` (`Numerical argument out of domain` in a later message; the filter crate's march
now does so), and the march's work volume is allocated with `MRIalloc` (its messages for an empty
extent).

`real.py` (relay 32): `flr` lines on three recorded heads (`ds000005-sub-01`, `ds000228-sub-pixar008`,
`ds002422-sub-02`, the classic recipe), both hemispheres: the white and pial interiors in a copy of
`aseg.mgz` (`MRISfillInterior(surface, 0, volume)`, mri_surf2volseg's call), the ribbon between
them in `orig.mgz` (`MRISribbon`, mris_volmask's), the white surface's interior in its bounding
volume at 0.5 mm (mris_place_surface's T2 and FLAIR steps): 24 of 24 identical in both modes, no
notice (observed). The 24 calls: FreeSurfer 55 s (59 s at relay 33's fresh run); the port 138 s at
9df8cbcb's parent, 87 s after the flood's breadth-first fill and the other speed changes, 38 s after
relay 33's (928fb0e2: `MRISfillInterior` and `MRISshell` through dense buffers, the ribbon's byte steps;
24 of 24 identical in both modes again). Per call (relay 33, one head): the interior 1.26 s
(FreeSurfer 1.40 s), the ribbon 3.72 s (4.33 s), the bounding volume filled 1.41 s (2.01 s); the rest
of the port's time is mostly the surface reads (0.72 s each: 0.47 s the file, topology and metric
properties, 0.25 s the neighbourhoods of three hops with their distances).

Relay 35 of track D (the 48th command `crv`: queue items 3.1-47/48's curvature files by name,
`MRISwriteCurvature` and `MRISreadCurvatureFile` with `__mrisreadcurvmri`; `specs/freesurfer-surfio.md`
§122-123): an empty work folder with `d` and `d/s`, the environment (`FS_POSIX`, `FS_GII`,
`MRIS_READVAL_FRAME`, `FS_LEAVE_NANS`, the creation time), the read frame now and then, 0 to 3 input
files (MGH, MGZ and NIfTI-1 volumes of about one voxel per vertex, text, VTK, GIFTI, the old and new
binary formats), a surface from `MRISalloc`, then 1 to 4 writes and reads (a read takes the name
just written one time in two, else an input's), each status, the volume of values and the
overlays, then the vertices, the counts and every file in the folder (a `.gii` one as
`gii_normal`). `crv` at seed 7 (400 cases) and seeds 13 and 17 (1,000 each): 0 different with
`reproduce` (15, 29 and 32 counted: fs.io's FS-167 and FS-49, fs.mri's FS-53, the GIFTI reader's
memory defects), `fix` different only with notices. Two findings on the way
(`docs/findings/freesurfer-surfio.md` §28): the GIFTI writers kept a signalling NaN's bits where the
official build's value setter is inlined (the shape and statistics arrays, their volume forms,
`mriWriteGifti`) and quiet it for the coordinates (`cvtss2sd`): fixed in fs.io's
`gifti_set_da_value_2d_f32` and `gifti_write.rs` (`gwr`'s values gained signalling NaNs: seeds 7 and
13, 400 cases each, 0 different); and the binary value readers now leave a failed read's `errno` (a
folder read as a file: `Is a directory`). The port's generator makes no random volume headers: a
random MGH header asks `MRIalloc` for 449,657,401 frames, which the official build refuses and the
port's fs.mri allocates (§28, for fs.mri's owner). Seed 1, 1,440 cases: 1,394 identical, 46 `SKIP`,
hash `60c0fd6aedcaf440b976e919450952585e9039708bd865e255dd4cf902ca8eab` (the test, from the clean
commit f3ca2f1c; seed 1's writes take no name Windows changes: a trailing dot, a `*`). `real.py --curv` (`flr crv`): the three heads' recorded `curv`, `sulc`,
`thickness`, `area`, `curv.pial`, `jacobian_white`, `volume` and `w-g.pct.mgh` of both hemispheres
read into the white surface, written in the new binary format, MGZ, compressed NIfTI, GIFTI and VTK
and read back (the MGZ and new-format files): 48 of 48 identical in both modes, no notice; every
read and write `NO_ERROR`; the 42 new-format files the port writes are the recorded files byte for
byte (the MGH overlay `w-g.pct.mgh` is the eighth kind: its new-format copy is another format);
FreeSurfer 33.5 s, the port 49 s (observed).

Relay 36 of track D (the 49th to 51st commands; `specs/freesurfer-surfio.md` §124-125; findings §29):
`pvw` (the patch, `MRISwriteIntoVolume`, the value readers `MRISreadValues` and
`MRISreadValuesIntoArray`, the three-vector volumes: normals, white normals, principal directions,
and `MRISreadNormals`), `vps` (another file's positions: `MRISreadVertexPositions` and the white,
pial, canonical and original-property readers, on a surface read from a triangle or quadrangle file
of a mesh, its other files of the same mesh moved in every format, a group-average area beside it
now and then) and `ann` (`MRISwriteAnnotation` and `MRISreadAnnotation`: annotation files, MGH
segmentations, GIFTI; a surface's and a caller's colour tables). The runner marks `NP` (a format the
port does not read yet: DICOM, which CTN takes some w files and patches for) and `FS-625u` (a
caller's colour table FreeSurfer has freed, used again); both, FS-191, FS-622 and FS-623 are counted,
not compared. Seeds 7, 13, 17: `pvw` 1,000 cases each, `vps` 1,000 then 500 each, `ann` 1,000 each: 0
different in both modes (`fix`'s `atan2f` last bits in `vps`'s canonical coordinates apart:
check.py's `fix_libm`). Seed 1, 1,530 cases: 1,480 identical, one `mpg` line hashed as the port's
(`acos` last bits), 49 `SKIP`, hash `ae997cbd7a99cc74ba69b29e2381ee1b84f9f111e519c27a1abcb7882bc4f4b6`
(the test, from the clean commit 16bade30; seed 1's `gwr` writes take no folder that does not exist:
Windows resolves `nodir/..` by the name). `real.py --pvw`: 30 of 30 identical in both modes;
`--vps`: 6 of 6 with `reproduce` (the default's `atan2f` changes `theta` and `phi`); `--ann`: 24 of
24 in both modes, every written annotation file the recorded one byte for byte (observed).

```
validation/probes/fs-surfio/build.sh <workspace>
scripts/orchestration/boxed.sh 12 python -P validation/probes/fs-surfio/check.py --seed 7 --n 66000
scripts/orchestration/boxed.sh 24 python -P validation/probes/fs-surfio/real.py
scripts/orchestration/boxed.sh 24 python -P validation/probes/fs-surfio/real.py --curv
scripts/orchestration/boxed.sh 24 python -P validation/probes/fs-surfio/real.py --pvw
scripts/orchestration/boxed.sh 24 python -P validation/probes/fs-surfio/real.py --vps
scripts/orchestration/boxed.sh 24 python -P validation/probes/fs-surfio/real.py --ann
```

Measured 2026-11-16 (relay 37 of track D, 52 commands): `mpx` (group (e) of queue items 3.1-49/50:
`MRIStalairachTransform` in place and into a clone, the seven vertex-to-voxel conversions,
`mrisComputeNormalDotDistribution`, `mrisComputePrincipalCurvatureDistributions` (a volume FreeSurfer
would read outside written `G` on both sides), `mrisSampleAshburnerTriangleEnergy` (the probe calls it
with a `calloc`ed `INTEGRATION_PARMS`); with `Gdiag`'s write and verbose bits the folder's files by
name, length and FNV-1a; the port's side in `tests/fs_surfio/mpx.rs`), seeds 7 (300 cases), 13 and 17
(1,000 each): 0 different with `reproduce` (9, 15 and 13 counted: FS-121 and FS-06 of the read), `fix`
different only with notices (FS-626, FS-628, FS-629, the read's FS-01; FS-627 is kept in both modes).
`flr mpx` (`real.py --mpx`: three heads, both hemispheres: the two distributions on `?h.white` with
`?h.sphere`'s canonical coordinates, Ashburner's energy at 64 vertices of `?h.sphere`, `?h.white` moved
to Talairach space with the head's `talairach.xfm`): 6 of 6 identical with `reproduce`; `fix` differs
only in the energies' last bits (correctly rounded `logf`; `check.py` counts an `mpx` case with `ash`
as a libm case). Seed 1, 1,560 cases: 1,518 identical, 42 `SKIP`, hash
`bf4fe88a9440b98a1060b3fd332b829f52347515ca86557a5d7a63b1ebca2d21`, from the clean commit 5c59b521;
the hash test and `props11_defaults` pass in debug and release, plain and under the AArch64 NaN
emulation (67,867 and 376,617 flips), and on the Windows laptop; seed 7's `mpx`, `vps` and `ann`
outputs (1,500) identical under the emulation in both modes.

Measured 2026-11-16 (relay 37 of track D, 53 commands): `lbl` (label.cpp's surface-label functions:
`LabelMark`, `LabelMark2`, `LabelUnmark`, `LabelMarkSurface`, `LabelArea`, `LabelFillMarked`,
`LabelErode`, `LabelDilate`, `MRISlabel2Mask`, `MRISlabelInvert`, on a label from `LabelAlloc` with
the case's points, index table, surface and template handles; an operation FreeSurfer would read or
write outside the surface's vertices or the label's places for written `G` on both sides; the port's
side in `tests/fs_surfio/lbl.rs`), seeds 7 (300 cases), 13 and 17 (1,000 each): 0 different with
`reproduce`, `fix` different only with notices (FS-632, the error messages' FS-01). `flr lbl`
(`real.py --lbl`: three heads, both hemispheres, four labels each on `?h.white`): 24 of 24 identical
in both modes.

Measured 2026-11-16 (relay 37 of track D, 54 commands): `wvl` (`MRISwriteValues` and
`MRISwriteAreaErrorToValFile` to w-file and volume names, `MRISstoreMetricProperties`, `MRIcopyMRIS`
of 19 field names into new and given volumes; the folder's files by their bytes; the port's side in
`tests/fs_surfio/wvl.rs`), seeds 7 (300 cases), 13 and 17 (1,000 each): 0 different with `reproduce`,
`fix` different only with notices (FS-633, FS-01); one seed-17 case of FS-634 (a vertex listing more
faces than the surface has: FreeSurfer reads its face-normal cache past the faces and crashed) counted,
not compared (`check.py`'s `SKIP`).

Measured 2026-11-16 (relay 38 of track D, 55 commands): `scl` (`surfcluster.cpp` whole: a surface
read by `MRISread` with every vertex's value, the group-average areas and flags, preset cluster
numbers, `Gdiag_no`, `FS_CLUSTER_USE_AVG_VERTEX_AREA`, `USE_FAST_SURF_SMOOTHER`, `FREESURFER_SEED`
1234; 1 to 5 of the cluster map, growth, measures, summaries, table, sort, renumberings, maxima, hand
p-values, pruning, annotation, growth by distance and point set, on the case's current list; after
each the vertices' numbers, values, annotations, the list and `errno`; `G` where FreeSurfer reads or
writes outside its arrays; the port's side in `tests/fs_surfio/scl.rs`), seeds 7, 13 (500 cases each)
and 17 (1,000): 0 different with `reproduce`, `fix` different only with notices (FS-906's point sets
in a missing folder, the messages' FS-01). The first runs found `glibc`'s `acos` EDOM outside [-1, 1]
(a vertex farther from the centre than the seed) and the colour table's clock seed (FS-04, hence
`FREESURFER_SEED`). `lbl` gained `LabelRemoveIslandsSurf` (`ri`), `LabelRemoveHolesSurf` (`rh`),
`LabelRemoveHolesAndIslandsSurf` (`rhi`) with some vertices' `stat` set: seeds 7, 13 (500 each) and 17
(1,000): 0 different (238 of FS-902's crashes on empty labels and full inversions compared as
crashes). `flr scl` (`real.py --scl`: three heads, both hemispheres, four labels each on `?h.white`:
the three clean-ups, the clusters of `|x| >= 20 mm` with a transform, the clusterwise-p values, the
old summary, a growth by distance): 24 of 24 identical in both modes (FreeSurfer 13.1 s, the port
21.9 s). Seed 1 (1,650 cases): 1,593 identical, 57 counted, hash `bb6a3c15...`.

Measured 2026-11-16 (relay 38 of track D, 56 commands): `vcp` (`mrisurf_vals.cpp`'s copies between
vertex fields and vectors, and `MRISwriteD`, `MRISwriteMarked`, `MRISwriteArea`, `MRISreadMarked`, each
named by its FreeSurfer function, on every field of every vertex; the folder's files by their bytes;
the port's side in `tests/fs_surfio/vcp.rs`), seeds 7, 13 (500 cases each) and 17 (1,000): 0 different
in both modes. `flr war` (`mris_place_surface --area-map` of each recorded white and pial surface):
the twelve files are recon-all's `?h.area` and `?h.area.pial` byte for byte, in both builds
(`real.py --scl`, 36 of 36 identical in both modes). Seed 1 (1,680 cases): 1,628 identical, 52
counted, hash `98549f42...`.

Measured 2026-11-16 (relay 39 of track D, 57 commands): `mht` (fs.surf's surface hash table:
the face and vertex tables with every bucket in the box of the vertices' voxels, every face's
intersection test, a vertex's moved test, its faces removed and added, the self-intersection test,
`mrisMarkIntersections` without and with hole filling, `MRISfillHoles` of a field or a volume,
`MRISmarkEdge` with hole filling; folded grids, crumpled icosahedra, crossing sheets, a 150 mm
triangle (FS-911), a triangle over 11,250 faces (FS-912), surfaces beyond the table (FS-910),
coordinates that are not finite (FS-909, FS-913); the port's side in `tests/fs_surfio/mht.rs`), seeds
7, 13 (500 cases each) and 17 (1,000): 0 different with `reproduce`, `fix` different only with
notices; FS-913's cases (the official build stopped by the 5-second alarm, the port refusing them)
counted. A vertex moved far from a far surface made a 1,000 mm sliver that FreeSurfer's quadratic
voxel-list search could not finish in 5 seconds (the port: 0.45 s): `mv` moves by up to 3 mm since.
`flr mki` (`real.py --mki`: three heads, both hemispheres, `orig.nofix`, `orig`, `white`, `pial`,
`inflated`, without and with hole filling): 56 of 60 identical in both modes; the other 4 are
`orig.nofix` with hole filling, where the official build crashes (FS-901: the one cluster of the
whole surface passes its stack) and the port grows it in full. Faces found intersecting: 0 on every
`orig.nofix`, 0 to 60 on `orig` (ds002422's left 60, right 30), 0 or 27 on `white`, 0 or 22 on
`pial`, 1 to 159 on `inflated`. `fs_mht_timing <surface>` (the port's example) times the read, the
face table, every face's test and `mrisMarkIntersections`: on ds000005's `lh.white` 0.82, 0.25,
0.88 and 1.04 s.

Measured 2026-11-16 (relay 40 of track D, 58 commands): `vls` (`mrisurf_vals.cpp`'s marks,
settings, rip flags, face tests, averages, medians, Gaussian, mode and soap-bubble filters and the
long-range sampling: `MRISsampleDistances` and `MRISsampleAtEachDistance` with `FREESURFER_SEED`,
`Gdiag_no`, the heartbeat bit and a `VDIAG` pair from the case, the files they write; grids, srd's
meshes and icosahedra, now and then at 2 hops; the port's side in `tests/fs_surfio/vls.rs`), seeds 7,
13 (500 cases each) and 17 (1,000): 0 different with `reproduce`, `fix` different only with notices
(FS-03's failed assertions, FS-01's `ErrorExit` messages, FS-12, FS-972, FS-974 to FS-977). The
official build's results include its signal 8 on small surfaces with the heartbeat bit (FS-974) and
its signal 11 writing a ripped `Gdiag_no`'s log (FS-977), both reproduced. A label point outside the
surface (FS-917) is written `G`; `MRISmodeFilterZeroVals` runs only in cases without special values
(FS-973's NaN makes the official filter print for ever).

Measured 2026-11-17 (relay 41 of track D, 59 commands): `vlr` (`mrisurf_vals.cpp`'s functions of
one program each: the rip dilation and erosion with the faces' flags, `MRISripZeros`,
`MRISripUnknown`, the vector registration's value setters, `MRISmodeFilterAnnotations` with the
program's annotation table (none, a table file where FreeSurfer looks for its default one, or a
colour table set first) and the surface's colour table, `MRISdilateVertexToSum` with a measure, the
areas or no surface, `MRISdilateConfined`, the accumulations, `MRISclearWhichAndVal2`,
`MRISmarkRandomVertices` with `FREESURFER_SEED`, `MRISannotation_to_label`; the port's side in
`tests/fs_surfio/vlr.rs`), seeds 7, 13 (500 cases each) and 17 (1,000): 0 different with
`reproduce`, `fix` different only with notices (FS-978, FS-981 to FS-983, FS-03, FS-01, FS-12). The
official build's results include its signal 11 for a value setter on a vertex without values
(FS-981) and for a disk of no hops (FS-983 through FS-530), both reproduced, and FS-979's writes past
its one-entry tables, which never crashed it. `G` where FreeSurfer reads memory it never wrote: a
field outside a vertex's values (FS-980), a surface without a colour table (FS-984), an index without
an annotation (FS-985). `flr maf` and `flr dvs` (`real.py --vlr`: three heads, both hemispheres;
`mris_ca_label`'s filter of four recorded annotations, `mri_binarize --dilate-vertex` disks of 5, 20
and 50 mm around vertices 0, 1,000 and 50,000): 78 of 78 identical with `reproduce`; the default
finds the 18 disks of 50 mm FreeSurfer reports as not possible (FS-982: its count over the passes
passes the vertex count at pass 45 or 46). Official 53 s, port 71-75 s (the reads are track A's).

Measured 2026-11-17 (relay 42 of track D, 60 commands): `top` (the rest of `mrisurf_topology.cpp`:
`edgeExists`, `mrisCountValidLinks`, `mrisAddEdge`, `mrisRemoveEdge`, `mrisRemoveVertexLink`,
`MRISremoveTriangleLinks` on surfaces typed as quadrangle files, `mrisCountTotalNeighbors`,
`MRIScomputeEulerNumber`, `MRIStopologicalDefectIndex`, `mrisStoreVtotalInV3num`,
`mrisVertexReplacingNeighbors`, `mrisForgetNeighborhoods`, `MRIScountTotalNeighbors`,
`MRISgetNeighborsBeginEnd`, `MRISisSurfaceValid`, `vertexInFace`, `mrisRemoveLink`, `mrisRemoveFace`,
`MRISrenumberRemovingRippedFacesAndVertices` (after `freedists`, `MRISfreeDistsButNotOrig` and
`MRISfreeDistOrigs`), `MRISremoveRipped`, `MRISevertSurface`, `MRISgetSpatOrderedNeighbors`, with
`MRISsetRipInFacesWithRippedVertices`, `MRIS_setNsizeCur` and rips between them, at 1 to 3 hops with or
without distances; the port's side in `tests/fs_surfio/top.rs`), seeds 7, 13 (500 cases each) and 17
(1,000): 0 different with `reproduce`, `fix` different only with notices (FS-986 to FS-990, FS-993,
FS-995, FS-357, FS-03, FS-01). `G` (both sides) for a vertex or face a renumbering removed; after a
renumbering that wrote the vertex number -1 (FS-995) both sides print the state and `STOP`. Counted,
not compared: FS-991 (the surface check of a vertex without neighbours), FS-992 (a link removal whose
copy runs over the memory around the list: the official build goes on with corrupted lists), FS-993.
`flr top` (`real.py --top`: three heads, both hemispheres, `orig.premesh`, `orig.nofix`, `orig` and
`white` with `cortex.label`: the counts, `MRISisSurfaceValid`, `LabelRipRestOfSurface`'s rip and
removal, the renumbering, the eversion): 24 of 24 identical with `reproduce`; the six `orig.premesh`
Euler counts equal recon-all's `mris_euler_number` lines. Official 23 s, port 40-42 s (the reads are
track A's).

Also relay 42, 61 commands: `sbl` (the four soap bubbles of `mrisurf_deform.cpp` with rips, marks of
0 to 3 and -1, offset current, original and target positions (NaN in heavy cases), `Gdiag`'s show and
verbose bits, `Gdiag_no` (now and then beyond the surface), the distances released or the marks set
again between them; the port's side in `tests/fs_surfio/sbl.rs`), seeds 7, 13 (500 cases each) and 17
(1,000): 0 different in both modes; counted, not compared: FS-996 (a diagnostic vertex beyond the
surface, read by FreeSurfer). `flr sbl` (`real.py --sbl`: three heads, both hemispheres, `white` and
`orig`, `cortex.label`'s vertices free, 25 passes of the plain and the weighted bubble): 12 of 12
identical in both modes. Official 17 s, port 31-33 s.

Relay 43 of track D, 62 commands: `mcl` (the closest searches of the surface hash table: face and
vertex tables of crumpled grids, small meshes, icosahedra and crossing sheets, now and then beyond the
table (FS-999), at resolutions 0.25 to 10 mm (0, -1, NaN in heavy cases); `MHTfindClosestVertexGeneric`,
`MHTfindClosestVertexNoXYZ`, `MHTfindClosestSetVertexNo`, `MHTfindVnoOfClosestVertexInTable`,
`MHTfindClosestVertexInTable`, `MHTfindClosestVertexNo2`, `MHTfindClosestVertex2`,
`MHTfindClosestVertexSet2` (a second surface: the file read again, its vertices moved),
`MHTfindClosestFaceGeneric`, `MHTfindClosestFaceNoGeneric`, `MHTBruteForceClosestFace`, every vertex of
the second surface searched, vertices moved or ripped after the table was made; the port's side in
`tests/fs_surfio/mcl.rs`). A face search with no limit on its shells is asked for only where a face
is found early (FreeSurfer looks at every voxel up to the distance otherwise: the alarm). Seeds 7, 13
(500 cases each) and 17 (1,000): 0 different in both modes; counted, not compared: FS-576 (a search of
the other kind of table reading a number beyond the surface: memory in FreeSurfer), FS-121. `flr mcl`
(`real.py --mcl`: three heads, both hemispheres: `mri_surf2volseg`'s vertex table of `white` at 16 mm
and `MHTfindClosestVertexNoXYZ` of every `pial` vertex and of each moved by 2 mm; `mri_surf2surf`'s
table of `sphere.reg` at 16 mm and `MHTfindClosestVertexNo2` and `MHTfindClosestVertexSet2` of every
vertex of the other hemisphere's `sphere.reg`; `mri_relabel_hypointensities`' table at 1 mm and
`MHTfindClosestVertexInTable`; `mrisProjectOntoSurface`'s face table of `white` and its two face
searches of every `pial` vertex): 6 of 6 identical in both modes (every `pial` vertex finds a
`white` vertex within 16 mm, 7 % within 1 mm; 94 % of them need the second face search). Official
201 s, port 239-243 s. `fs_mhtfind_timing <white> <pial> <sphere.reg> <other sphere.reg>` (the port's
example) times each part (ds000005 `lh`: the reads 2.6 s, the 16 mm vertex searches 6.9 s, the
spheres' 1.6 s, the 1 mm table 0.07 s, the projected face searches 15.7 s, the second ones 0.7 s).

Also relay 43: `lbl` gains `LabelToFlat` (`tf`), `LabelRipRestOfSurface` (`rr`),
`LabelRipRestOfSurfaceWithThreshold` (`rrt`) and `LabelRip` (`rp`), each rip followed by top's dump of
the surface; seeds 7, 13 (500 cases each) and 17 (1,000): 0 different with `reproduce`, `fix` different
only with notices (FS-986, FS-357, FS-902, FS-632, FS-01, FS-03). `flr lrr` (`real.py --lrr`: three
heads, both hemispheres, `white` with `cortex.label`, `BA1_exvivo.label`, `BA44_exvivo.thresh.label`,
`MT_exvivo.label`: `LabelRipRestOfSurface` with every odd point's vertex cleared, the threshold at 0.5,
`LabelRip`): 24 of 24 identical with `reproduce` (the default differs in all 24 by FS-986, as relay 42
found for every recorded cortex label). Official 33 s, port 53 s (the reads are track A's).

`stack.py` (relay 38 of track D) measures FS-901, the depth at which the official cluster growth
overflows its 8 MiB stack, with `flr sgd` (every value 1, one `sclustMapSurfClusters`; the port also
prints its deepest path): on flat grids (strips of 2 x 20,000 to 2 x 80,000, squares of 100 x 1,000
to 400 x 400 vertices, 0.1 mm apart) the official build completes a growth 103,500 levels deep and
crashes from 104,000; one cluster of each recorded head's whole white surface (119,164 to 148,828
vertices) goes 71,053 to 87,414 deep and completes. The port grows every one in full.

`lc.py <object.o> .LC<n> ...` (relay 37 of track D) prints an object file's local constants as
`float` and `double` (the values the disassembly's `.LC` loads name: findings §30's FZERO bound, the
distributions' limits, Ashburner's `SMALL`).
