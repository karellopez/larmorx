# Provenance: larmorx-freesurfer-surf

**Replica: translated from FreeSurfer's source.** This crate holds `fs.surfio` (wave 3.1), `fs.surf` (wave 3.4), `fs.mars` (later) of the FreeSurfer
replica (`docs/freesurfer/waves.md` section 4): the surface structure and its I/O, with the metric properties every read computes (`fs.surfio`); the rest of the surface library: topology, values, volume-surface sampling, projections, the surface hash, realms, icosahedra, spherical maps, surface clusters, gcsa and the surface-gradient code (`fs.surf`). It is distributed under FreeSurfer's
licence (`LicenseRef-FreeSurfer`, `../LICENSE`: the FreeSurfer Software License with its required
preface), as part of the `larmorx-freesurfer` package (`docs/licensing.md`).

The upstream is FreeSurfer 8.2.0 (GitHub's `v8.2.0` archive, SHA-256
`bb1699f7655ebf0f3790db6a78af9532ea7c832c97fe5f71bfd9f0e9526d7e78`; `docs/freesurfer-plan.md`),
built for the official binaries with GCC 8.4.0 (`docs/freesurfer/variation.md`).

## FreeSurfer files and functions translated

| Rust module | FreeSurfer file | Functions and data |
|---|---|---|
| `aaa` | `include/mrisurf_aaa.h`; `include/base.h` 139-141; `utils/mrisurf_base.cpp` 23-77, 122-133; `utils/mrisurf.cpp` 21-22; `include/mrisurf_base.h` 299-301 | `MAX_NEIGHBORS`, `MRIS_MAX_NEIGHBORHOOD_LINKS`, `MAX_SURFACES`, the coordinate-system and surface-name constants, `VERTICES_PER_FACE`, `ANGLES_PER_TRIANGLE`, `MAX_CMDS`, `NEW_VERSION_MAGIC_NUMBER`, `VERTEX_SULCAL`, `WHICH_FACE_SPLIT` (with fs.core's `nint`), `vertices_per_face_t`, `angles_per_triangle_t`, `copyAnglesPerTriangle`, `cmpAnglesPerTriangle` (glibc's `memcmp` result as observed: the findings §4), `MRIS_XYZ`, `MRIS_HASH`, `MRIS_Status` (as an `int`, with its `MRIS_Status_ELTS` table), `MRIS_Status_DistanceFormula`, `MRIS_Status_text`, `MRIS_Status_distanceFormula`, `areCompatible`, `checkOrigXYZCompatibleWkr`, `mris_sort_compare_float`, `MRIS_AREA_LABEL`, `FaceNormCacheEntry`, `FaceNormDeferredEntry`, `MRI_EDGE`, `MRI_CORNER`, `STRIP`, `FloatXYZ`; the globals `UnitizeNormalFace`, `RmsValErrorRecord`, `fix_vertex_area` with `MRISsetFixVertexAreaValue`/`MRISgetFixVertexAreaValue`, `mrisurf_activeRealmTreesSize` with `hasActiveRealmTrees`, as `SurfGlobals` |
| `mris` | `include/mrisurf_FACE_VERTEX_MRIS_generated.h` 7-262 | `FACE` (`face_type_`), `VERTEX_TOPOLOGY`, `VERTEX` (`vertex_type_`), `MRIS`: every field but `lta`, `ct`, `vg` and `mri_sras2vox` (their types are fs.mri's and fs.volume's: added with the surface readers, 3.1-47); C's pointers as the module explains |
| `mris_mp` | `include/mrisurf_MRIS_MPPropertiesInVectors.h`; `include/mrisurf_MRIS_MP.h` 13-118 | `MRIS_MP`: the arrays and values (the input arrays shared by copies as `Arc`, replacing `in_src`/`in_ref_count`); the pointers into the `MRIS` (`underlyingMRIS`, `vertices_topology`, `vertices`, `faces`, `v_dist_orig`) read through the `MRIS` a view pairs it with |
| `views` | `include/mrisurf_SurfaceFromMRIS_MP_generated_*.h` (the `AllM` level's accessors; every level's are the same storage: `validation/investigations/freesurfer-surfio/accessors.py`), `include/mrisurf_SurfaceFromMRIS_generated_*.h` (the same accessors on `MRIS`), `include/mrisurf_SurfaceFromMRIS_MP_generated_suffix.h` lines 27-1510 (`which_coords`' cases), `include/mrisurf.h` 1025-1041; `utils/mrisurf_metricProperties.cpp` 3721-3758 | `MetricSurface` (the `MRIS_MP` views' 114 accessors but the face normal and the face-normal caches) for `Mris` and `MpView`; `which_coords_mp_cases`; the selector constants `ORIGINAL_VERTICES` ... `WHITE_NORMALS`; `MRISvertexCoord2XYZ_float`, `MRISvertexCoord2XYZ_double` (the `MRIS` views' `which_coords`) |
| `topology` | `include/mrisurf.h` 2198-2200, 2501-2530; `include/mrisurf_topology.h` 74-84; `utils/mrisurf_topology.cpp` 360-372, 907-918 | `returnTrue` (`mrisCheckVertexFaceTopology`, `mrisCheckVertexVertexTopology`), `setVnum`, `clearVnum`, `vnumAdd`, `addVnum`, `VERTEXvnum`, `mrisVertexNeighborIndex`, `mrisVerticesAreNeighbors`, `vertexNeighbor`, `modVnum` (its `static short maxVnumSeen` in `SurfGlobals`), `mrisVertexVSize` |
| `base` | `include/mrisurf_base.h` 112, 156-168, 179-208; `include/mrisurf_sphere_interp.h` 13-56 | `sign`, `mrisNormalize`, `closeEnough`, `project_point_onto_sphere` (`FZERO` from `include/macros.h`), `Vec3` with `+`, `-`, `*` (inner product and scale), `operator*(double, Vec3)`, `dot`, `cross` |
| `mrisurf` | `utils/mrisurf.cpp` 87-108, 113-201, 270-273, 924-1007; `include/fnv_hash.h` 22-31; `include/mrisurf.h` 1579-1593 | `MRISaddCommandLine` (FS-112), `vertix_n_hash_add` (one surface, no difference file) as `mrisVertexHash`, `fnv_init`, `fnv_add` (larmorx-core's copy of queue item 3.1-18 replaces them when it exists), `MRISdiffSimple`, `SMALL_VERTEX`, `SMALL_SURFACE`, `MRISSfree` |
| `surfbase` | `utils/mrisurf_base.cpp` 80-118, 133-157, 505-512, 937-938; `include/mrisurf.h` 117-121; `include/mrisurf_base.h` 305 | `MRIS_TempAssigned`, `MRIS_acquireTemp`, `MRIS_checkAcquiredTemp`, `MRIS_releaseTemp`, `MRISacquireNverticesFrozen`, `MRISreleaseNverticesFrozen`, `MRISmakeFloatPerVertex`, `MRISfreeFloatPerVertex`, `MRISsetInflatedFileName` (FS-111), `MRISsetSulcFileName`, `MRISsetOriginalFileName`; the globals `NEG_AREA_K`, `mrisurf_surface_names`, `curvature_names` (in `SurfGlobals`) |

Not translated, on purpose (`docs/findings/freesurfer-surfio.md`):

- `MRISPV` (`include/mrisurf_MRIS_PropertiesInVectors.h`) and its views
  (`include/mrisurf_SurfaceFromMRISPV_generated_*.h`): no FreeSurfer 8.2.0 code ever creates one, so
  none of their code can run.
- The `MRIS` views' remaining accessors: each is one field of `Mris`, `Vertex`, `Face` or
  `VertexTopology` (the table tool checks that every accessor reads one storage at every level).
- `MRIS_MP`'s own `which_coords` (never called: its cases are kept as `which_coords_mp_cases`).
- `min3` (`include/mrisurf_base.h` 119: cannot be instantiated), `square`, `square3`, `norm3`,
  `max`, `min`, `max3` (unused templates), `MRISBaseCtr`, `MRISBaseConstCtr`, `MRISBaseToConst`
  (unreached).
- `mris_hash_init`, `mris_hash_add`, `mris_hash_print`, `mris_print_hash`, `mris_print_diff`
  (`utils/mrisurf.cpp` 229-317): reached only behind `debugNonDeterminism` (a constant 0) or from
  `utils/test/`.
- `MRISripMidline`, `MRIcomputeLabelNormal`, `MRIScopyCoords` (`utils/mrisurf.cpp` 358-922): with
  fs.mri's `MRI`, `LABEL` and `VOL_GEOM` (queue item 3.1-55); `insertActiveRealmTree`,
  `removeActiveRealmTree` (`utils/mrisurf_base.cpp` 159-195): with fs.surf's `RealmTree`; the
  `gMRISexternal*` hooks (`utils/mrisurf.cpp` 57-62): with fs.deform's `INTEGRATION_PARMS`.
- `INTEGRATION_PARMS` and `ClosestVertex` (`include/mrisurf.h` 299-541, 2618-2669): with the modules
  their members belong to (fs.deform, fs.volume's `LTA`, fs.surf's `MHT`).

Third-party code in these files: none (`docs/freesurfer/third-party.md` §6: the generated headers
and `mrisurf_sphere_interp.h` are MGH's, under FreeSurfer's licence).

## Changes from the originals

- C's pointers are Rust values (`Option<Vec<T>>` for an array a function allocates, `None` for
  `NULL`; vertex numbers for pointers to vertices; `Arc` for the `MRIS_MP` arrays its copies share;
  `Opaque` for `void *`): reading a `NULL` array panics where FreeSurfer reads through a null
  pointer.
- A write through an `MRIS_MP` copy into one of its shared input arrays changes that copy only
  (FreeSurfer's copies share it; no FreeSurfer code writes one).
- The process-wide variables are fields of `SurfGlobals`, one value per run.
- Failed assertions end the call with FS-03's exit (`larmorx-freesurfer-core::diag::assert_failed`).
- `MRISmakeFloatPerVertex`'s array is 0 where FreeSurfer's `malloc` leaves it undefined.
- FS-111 (an inflated name of 1,022 bytes or more is taken whole; `reproduce`: the official
  build's abort message and exit status 134) and FS-112 (a command line past 1,000 is dropped in
  both modes, the original's count becoming a memory address) (`docs/findings/freesurfer-surfio.md` §7).
