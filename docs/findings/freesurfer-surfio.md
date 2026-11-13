# FreeSurfer 8.2.0's surface structure (`fs.surfio`): findings

**Closed to FreeSurfer clean-room implementers** (it names FreeSurfer's files, functions and lines;
the behaviour is `specs/freesurfer-surfio.md`). Queue items 3.1-45 and 3.1-46's structure, replica in
`crates-freesurfer/larmorx-freesurfer-surf` (track D, relay 5, 2026-11-13).

## 1. The generated views are storage maps

`include/mrisurf_SurfaceFrom{MRIS,MRIS_MP,MRISPV}_generated_suffix.h` (made by
`utils_generator/Generator.cpp`) define, for each representation and each of 13 access levels
(`Existence`, `Topology`, `XYZPosition`, `XYZPositionConsequences`, `Distort`, `Analysis` and their
`M` variants, `AllM`), the member functions of the `Face`, `Vertex` and `Surface` views:
2,778 bodies for `MRIS`, 817 for `MRIS_MP`, 2,736 for `MRISPV`
(`validation/investigations/freesurfer-surfio/accessors.py <tag> summary`). Every body is one read
(`return repr->...`), one write (`... = to`), or a write after `cheapAssert(repr == to.repr)` (the
setters taking a `Face` or `Vertex`), but `which_coords`; and each member function reads and writes
**the same storage at every level** where it exists (406, 120 and 400 distinct functions; no body
differs between levels). The levels are compile-time permissions only.

Storage worth knowing:

- `MRIS_MP`'s `Surface::status()`, `set_status`, `patch()`, `set_patch` and `noscale()` read and
  write `repr->underlyingMRIS->...`, not the `MRIS_MP`'s own `status`, `patch`, `noscale` fields
  (`status` is still copied into the `MRIS_MP` by `MRISMP_load`; `patch` and `noscale` are never
  set there). The probe's `view` command shows it: a write through the `MRIS_MP` view changes the
  `MRIS`'s `status`; the `MRIS_MP`'s own field keeps its value.
- `MRIS_MP`'s `Vertex::dist_orig(i)` reads `v_dist_orig[vno][i]`, which `MRISMP_load` points at the
  `MRIS`'s own `dist_orig` arrays ("these keep pointing to the original ones in the MRIS",
  `include/mrisurf_MRIS_MP.h` line 68). The replica's view reads the `MRIS`'s array.
- `MRIS_MP` declares arrays that FreeSurfer never fills (`MRISMP_ctr` zeroes the struct;
  `MRISMP_load` fills only the `MRIS_MP__LIST_*` fields): `f_v`, `v_origx..z`, `v_dx..z`,
  `vertices`, `faces`, `faceNormCacheEntries`, `faceNormDeferredEntries`, and `orig_area` (outputs
  list `neg_area`, `neg_orig_area`, not `orig_area`). Their views read a null pointer (or a
  zero); no templated caller reads them (`grep` of `utils/`). The replica keeps them as `None`
  arrays, which panic when read, as FreeSurfer would fault.
- `MRIS`'s pole accessors (`v_temporal_pole()` and the two others) return `Vertex(repr, ptr -
  repr->vertices)`: on a null pole the index is the pointer difference of `NULL` and `vertices`
  (undefined; never called: the structure's own comment says the poles "don't appear to be used").
  The replica keeps the pole as `Option<usize>`.

## 2. `MRISPV` is never built

No FreeSurfer 8.2.0 code creates an `MRISPV`: no constructor, allocation or loader exists in the
tree (`grep -rn MRISPV` over every `.c`, `.cpp`, `.h`). The functions taking one are
`MRISfreeDistsButNotOrig(MRISPV*)` (`utils/mrisurf_base.cpp` line 416: `cheapAssert(!"... NYI")`)
and `mrishash.cpp`'s `CONSTRUCTORS(SurfaceFromMRISPV::XYZPositionConsequences::Surface, ...)`
instances (line 2338), which no caller reaches with one. The static reach of queue item 3.1-45
(2,019 lines of `MRISPV` accessors) is therefore unreachable at run time; the replica has no
`MRISPV`.

## 3. `which_coords`

Only `utils/mrishash.cpp` (lines 854, 860, 869) calls `which_coords`, on the views of its template
instances: `MRIS` (whose `which_coords` is `MRISvertexCoord2XYZ_float(&repr->vertices[idx], ...)`,
`utils/mrisurf_metricProperties.cpp` lines 3721-3750) and `MRISPV` (never built). The `MRIS_MP`
views' own `which_coords` (a switch per level: `CURRENT`, `ORIGINAL`, `CANONICAL` from
`XYZPosition` on, `VERTEX_NORMALS` from `XYZPositionConsequences`, `WHITE_NORMALS`, `PIAL`,
`WHITE` from `Distort`; the default sets the outputs to 0 and calls `ErrorExit(ERROR_UNSUPPORTED,
"which_coords: unsupported which %d")`) is never called; its cases are kept in
`views::which_coords_mp_cases` for reference. `MRISvertexCoord2XYZ_float`'s default does not set
the outputs before its `ErrorExit` (unlike the generated switches); `ORIGINAL_VERTICES`' case reads
`origx`, while the `MRIS_MP` views' `ORIGINAL_VERTICES` would read the never-filled `v_origx`.

## 4. `memcmp` of 12 bytes returns the byte difference

`cmpAnglesPerTriangle` (`include/mrisurf_aaa.h` line 119) returns `memcmp(&dst, &src, 12)`. On the
reference machine (glibc 2.35 on an AVX-512 CPU, the `memcmp` variant its ifunc picks there) the result is the
difference of the first differing bytes as unsigned values (128 for `-0` against `+0` in the first
angle), not -1/1: 30,000 probe cases, 2,000 of them `angles`, all identical. The value matters only
where it is printed (`MRISMP_unload`'s `comparison(fno, cmpAnglesPerTriangle(...), 0)`,
`utils/mrisurf_metricProperties_fast.h` line 419, prints it with `%f` when `check` is on); every other caller compares it with 0. Other x86-64 `memcmp` variants
(SSE2, AVX2) may return other magnitudes with the same sign: the replica follows the reference
machine.

## 5. The header helpers and their NaN operand order

`include/mrisurf_base.h`'s `mrisNormalize`, `closeEnough`, `project_point_onto_sphere`, `sign` and
`include/mrisurf_sphere_interp.h`'s `Vec3` operators are `static` or `inline`: each caller in
`utils/` gets its own compiled copy, whose operand order where two NaNs meet is GCC's choice at that
site. The probe (`validation/probes/fs-surfio/`) compiles them into `fssurfio` with the official
flags: its first 30,000-case run differed in one `vec3` line, `t = s * v` with `v[0]` and `s` both
NaN, where GCC put the scalar first (the C++ text `vect * scalar` would give `v[0]`'s NaN). That
is the probe's own compilation; the callers' (`mrisurf_sphere_interp.cpp`'s
`SphericalInterpolator::testRayIntersection`, fs.surf) are checked with their port. The case
generator now keeps the scale finite.

Also measured: a probe that reads its tokens as a constructor's arguments (`Vec3 u(getd(), getd(),
getd())`) gets them right to left (C++ leaves the order unspecified; GCC 8 evaluates the last
first). The probe reads them into locals first.

## 6. Smaller observations

- `include/mrisurf_base.h` line 119: `min3(lhs, mid, rhs)` is written `min(min(lhs,min),rhs)` (the
  function template `min` passed as an argument): it can never be instantiated, so no program uses
  it (the static reach counts it through the name; the callers use the `MIN3` macro or their own
  code). No behaviour, no defect.
- `modVnum`'s count is a `short` and its `static short maxVnumSeen` keeps the largest seen
  (`utils/mrisurf_topology.cpp` lines 360-372); a count past 32767 wraps as GCC converts back to
  `short` (implementation-defined in C, wrapping on x86-64); real surfaces stay below
  `MAX_NBHD_VERTICES` (20,000) and `MAX_NEIGHBORS` (1,024). Both modes wrap.
- `vertexNeighbor` (`include/mrisurf_topology.h` line 74) checks neither `vno1` nor a null list;
  `mrisVertexNeighborIndex` (`include/mrisurf.h` line 2516) checks both. Reached through
  `mrisurf_topology.cpp` (2 programs) with vertices of the surface only.
- `INTEGRATION_PARMS`'s constructor (`include/mrisurf.h` line 498, zero for every member, `fp` the
  given file) and `ClosestVertex` (line 2618: `ReadLTA`, `WriteVertexDist`, `InitHash`) are not in
  this crate yet: their members are fs.deform's, fs.volume's (`LTA`) and fs.surf's (`MHT`); they come
  with their modules.
- The `MRIS` fields waiting for other items: `lta` (`LTA *`, fs.volume 3.1-40), `ct`
  (`COLOR_TABLE *`) and `vg` (`VOL_GEOM`) (fs.mri 3.1-55), `mri_sras2vox` (`MRI *`, fs.mri). The
  surface readers (3.1-47) need `vg` and `ct`.
