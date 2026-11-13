# FreeSurfer 8.2.0's surface structure (`fs.surfio`): findings

**Closed to FreeSurfer clean-room implementers** (it names FreeSurfer's files, functions and lines;
the behaviour is `specs/freesurfer-surfio.md`). Queue items 3.1-45 and 3.1-46's structure, replica in
`crates-freesurfer/larmorx-freesurfer-surf` (track D, relay 5, 2026-11-13); §7 the rest of 3.1-46
without a volume, §8 queue item 3.1-49's coordinate operations (relay 6).

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

## 7. The rest of 3.1-46 (relay 6): `mrisurf.cpp` and `mrisurf_base.cpp` without a volume

- **`mrisVertexHash`** (`utils/mrisurf.cpp` 113-201, 270-273) hashes `LIST_OF_VERTEX_TOPOLOGY_ELTS`'
  `ELTT` members (`vnum`, `v2num`, `v3num`, `vtotal`, `nsizeMax`, `nsizeCur`, `num`; `nsizeMaxClock`
  is `ELTX`, the pointers `ELTP`), then `v` (`mrisVertexVSize` ints), `f` and `n` (`num` each),
  `dist` and `dist_orig` (`mrisVertexVSize` floats). The second `ELTT` macro (lines 138-152, the
  `VERTEX` members) is defined and undefined **without** `LIST_OF_VERTEX_ELTS` between: no vertex
  scalar enters the hash. `showHashCalc` (a `static size_t` raised only by `mris_print_hash`'s
  `showHash`, which needs `++showHashCount == showHashLimit` with the limit 0) is 0 in every run, so
  the hash writes nothing. Its only reached caller is `utils/mrisurf_defect.cpp` (lines 289, 371:
  the topology fixer's vertex-state save, every `limit`-th one as `count` passes it, and the
  `cheapAssert(hash.hash == vs->hash.hash)` at restore). `fnv_init`/`fnv_add` are
  `include/fnv_hash.h`'s (queue item 3.1-18 puts them in larmorx-core; until then the crate has its
  own 8 lines).
- **Unreached hashing.** `mris_hash_init`, `mris_hash_add`, `mris_hash_print`, `mris_print_hash`,
  `mris_print_diff` (lines 229-317): every caller in `utils/` is behind `debugNonDeterminism`
  (`include/mrisurf_base.h` line 27: `static const int debugNonDeterminism = 0`), `if (0)`
  (`mrisurf_integrate.cpp` 2888, 2890) or in `utils/test/topology_test.c`. `mris_print_hash` would
  also print a pointer (`%p` of `avg_vertex_dist`'s bits), not reproducible. Not translated.
- **FS-111.** `MRISsetInflatedFileName` (`utils/mrisurf_base.cpp` 133-143) does `sprintf(fname,
  "%s.H", inflated_name)` into `char fname[STRLEN]` (`STRLEN` = `STR_LEN` = 1,024,
  `include/const.h` 51-52). The official build compiles it to `__sprintf_chk(fname, 1, 0x400, ...)`
  (`objdump` of the unit oracle's `mrisurf_base.cpp.o`: `mov $0x400,%edx; mov $0x1,%esi`):
  `_FORTIFY_SOURCE` level 2, so 1,022 bytes or more abort in `__chk_fail` (glibc 2.35 writes `***
  buffer overflow detected ***: terminated` to standard error, then `abort`); the probe shows 1,021
  bytes set the names and 1,022 abort (signal 6). `mrisurf_surface_names[0]` is already set when it
  aborts (the store precedes the call) but the process ends. Reached from `mris_register -inflated
  <name>` (`mris_register.cpp` 913, 503); recon-all does not pass `-inflated`.
- **FS-112.** `MRISaddCommandLine` (`utils/mrisurf.cpp` 98-108): `fs::error()` only prints (its
  destructor writes and logs, `include/log.h` 56-61; no exit), then `int i = mris->ncmds++;
  mris->cmdlines[i] = calloc(...)`. `MRIS_cmdlines_t` is `FixedSizeArray<char *, MAX_CMDS>`
  (`include/mrisurf_aaa.h` 55; `include/base.h` 144-156: no bounds check), and `ncmds` is the next
  member (`include/mrisurf_FACE_VERTEX_MRIS_generated.h` 252-254): at 1,000 the new pointer's 8 bytes
  overwrite `ncmds` and `group_avg_surface_area`. The probe prints only the message for such cases.
  Callers: every program that writes a surface; `gifti.cpp` 702 (one per `NUM_TAG_CMDLINE` line of a
  GIFTI file); the surface readers have their own `ncmds > MAX_CMDS` checks (`mrisurf_io.cpp` 4049:
  off by one, 3.1-48's).
- **Unreached, translated anyway** (small, in the item): `MRIS_acquireTemp`, `MRIS_checkAcquiredTemp`,
  `MRIS_releaseTemp`, `MRISmakeFloatPerVertex`, `MRISfreeFloatPerVertex` (no caller in 8.2.0);
  `MRISdiffSimple` (`mris_diff` only). `MRISsetSulcFileName` is reached from `mris_register`'s,
  `mris_left_right_register`'s and `mris_congeal`'s options, `MRISsetOriginalFileName` from
  `mris_register` on every run (`mris_register.cpp` 500).
- **`MRISdiffSimple`'s NaN.** `rmssum/nhits` with no vertex compared is `0.0/0.0`, SSE's default NaN
  with the sign set: `%g` prints `-nan` (AArch64's default NaN would print `nan`: the replica uses
  fs.core's `sse` helpers for every operation that can make or pass a NaN).
- **Waiting for fs.mri:** `MRISripMidline` (358-807), `MRIcomputeLabelNormal` (809-878),
  `MRIScopyCoords` (888-922, it copies `vg`); the realm-tree list (`mrisurf_base.cpp` 159-195) with
  fs.surf's `RealmTree`.

## 8. Queue item 3.1-49, part a: the coordinate operations and the distances' storage (relay 6)

- **GCC 8.4's operand orders.** Where two NaNs meet, x86's SSE returns the first source operand's
  (the destination register). GCC treats `+` and `*` as commutative and the official build's code
  differs from the C text in many places; read from the unit oracle's `mrisurf_metricProperties.cpp.o`
  (`objdump -d -C`, the same compiler and flags as the official binaries) and confirmed by the
  probe (seed 11: 34 of 4,000 `surf` cases differed before, 0 after):
  - `MRIStranslate`: the box and centre are two `addps` of four lanes with the shifts as the
    destination (`[dy dz dx dy] + [ylo zlo xhi yhi]`, `[dx dy dz dx] + [xctr yctr zctr xlo]`), `zhi`
    a scalar `zhi + dz`; the vertices `x + dx`.
  - `MRISscale`: `zhi` is `scale * zhi` (`mulsd %xmm1,%xmm0` with the scale in `xmm0`), the other
    eight `mulpd` with the fields first; the `if (scale < 0)` block (FS-113) is compiled away (it
    stores each low end back over itself).
  - `MRISscaleThenTranslate`: `ylo`, `zlo` are `d + s*e` (`mulpd %xmm0,%xmm1` with `[sy sz]` in
    `xmm1`, then `addpd %xmm1,%xmm4` with `[dy dz]` in `xmm4`), the rest `e*s + d`; its three
    `if (s < 0)` swaps are compiled away too.
  - `MRISanisotropicScale`: the high ends are `x0 + sx*(xhi - x0)`, the low ends `(xlo - x0)*sx +
    x0`; `0.5f * (float)((double)xlo + (double)xhi)` is computed as `(xhi' + xlo') * 0.5f` in
    float (exact: the double sum of two floats rounds to the float sum).
  - `MRISrotate` (template, `._omp_fn.0`): the twelve products as `cacb = ca*cb, cacg = ca*cg,
    cacgsb = cacg*sb, sasg = sg*sa, cgsa = cg*sa, casbsg = (sb*ca)*sg, cbsa = cb*sa, cgsasb =
    sb*cgsa, casg = ca*sg, sasbsg = (sa*sb)*sg, cbcg = cg*cb, cbsg = sg*cb`; the per-vertex `xp =
    (K1*z + cacb*x) + K2*y`, `yp = (K3*z - x*cbsa) + K4*y` (the source's `-x * cbsa + ...` becomes a
    subtraction, which changes a NaN's sign), `zp = (z*cbcg + x*sb) + y*cbsg`; the sines and
    cosines are three `sincosf` calls (libstdc++'s `std::sin(float)` is `__builtin_sinf`, which
    `-fno-builtin` does not stop, and GCC pairs it with `cosf`), made before
    `MRISfreeDistsButNotOrig`.
  - `MRISblendXYZandTXYZ`: `x*a + tx*b`; `MRISblendXYZandNXYZ`: `nx*a + x`;
    `MRIStranslate_along_vertex_dxdydz` (`._omp_fn.3`): `dx*dt + x` in double.
  - `MRISavgVetexRadius`: `zz + (xx + yy)` in float with `sqrtss` (the `float` overload of `sqrt`),
    `(sum2/N - avg*avg) * N / (N - 1)`; `MRISsphericalCoords`: `d2 = xx + yy`, `r = sqrt(zz + d2)`.
  - `MRIScenterCOG2` negates after rounding to float (`cvtsd2ss` then `xorps`): the same bits as
    rounding the negated double.
- **glibc's `atan2` with NaNs** returns `x + y` (1,331 probe cases of NaN, signalling-NaN, infinite
  and zero coordinates; the replica adds them in that order). Its last bit differs from CORE-MATH's
  on about 0.09 % of random arguments (`docs/findings/freesurfer-libm.md` §3), but 300,000 random
  spherical coordinates rounded to float were identical.
- **The distances' storage model.** FreeSurfer keeps `mris->dist_storage[vno]` and points
  `v->dist` at it (or `NULL`); the replica holds the buffer in `v.dist` while valid and in
  `dist_storage[vno]` while not, never both (`crates-freesurfer/larmorx-freesurfer-surf/src/dist.rs`).
  The probe sets FreeSurfer's two pointers equal wherever a case's distances are valid, as
  `changeDistOrDistOrig` and `mrisSetDist` leave them. `realloc`'s new memory is undefined:
  `growDistOrDistOrig` zeroes from `vtotal` (`dist`) or the old capacity (`dist_orig`) to the new
  capacity, so a buffer shorter than `vtotal`, or none at all, leaves undefined values below
  `vtotal` (the probe printed heap contents for such cases, e.g. `e861ace0 00007086`); the replica
  gives 0 there and the probe's generator avoids those states (FreeSurfer's callers grow the
  distances of vertices whose buffer covers `vtotal`). `MRISmakeFloatPerVertex`'s `malloc` is the
  same matter (no caller).
- **`MRISsetOriginalXYZfromXYZ`** tells the active realm trees about vertex `nvertices` (the loop
  variable after the loop), one past the end; harmless with no realm tree (this module's callers),
  to be looked at with fs.surf's realm trees.
- **FS-113.** `MRISscale` (`mrisurf_metricProperties.cpp` 408-412) and `MRISscaleThenTranslate`
  (499-501) mean to swap a negatively scaled axis's ends but write the low end back over itself:
  `float xlo = mris->xlo; mris->xlo = mris->xhi; mris->xlo = xlo;`. The box stays upside down until
  the next `mrisComputeSurfaceDimensions`. Callers: `mris_convert`'s scale option (the user's
  factor, `mris_convert.cpp` 441); `mri_watershed` (recon-all's skull strip, `mri_watershed.cpp`
  6503) and `mri_brain_volume` (2473) scale the icosahedron by radii `MIN(xmax - xCOG, xCOG -
  xmin)`, negative only when the centre of gravity lies outside the brain's box. Rule A: larmorx
  swaps; `reproduce` keeps the original's.
- **Unreached:** the `MRIS_MP` overloads of `MRISrotate` and `MRIStranslate_along_vertex_dxdydz`
  fail `MRISfreeDistsButNotOrig(MRIS_MP*)`'s assertion; the only `MRIS_MP` caller
  (`mrisurf_deform.cpp` 1023, `MRIScomputeSSE_asThoughGradientApplied`) is behind `useNewBehaviour`,
  set to `false` unconditionally (line 1006).
- **Waiting:** `MRIStalairachTransform` (needs `MRISclone` and the `LTA`), `MRISrotate(src, NULL,
  ...)` and `MRISprojectOntoTranslatedSphere` (`MRISclone`, 3.1-51's lines; the latter also
  `MRISupdateEllipsoidSurface`), `mrisDisturbVertices` (`MRIScomputeMetricProperties`).
