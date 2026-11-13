# FreeSurfer 8.2.0's surface structure (`fs.surfio`): findings

**Closed to FreeSurfer clean-room implementers** (it names FreeSurfer's files, functions and lines;
the behaviour is `specs/freesurfer-surfio.md`). Queue items 3.1-45 and 3.1-46's structure, replica in
`crates-freesurfer/larmorx-freesurfer-surf` (track D, relay 5, 2026-11-13); §7 the rest of 3.1-46
without a volume, §8 queue item 3.1-49's coordinate operations (relay 6), §9 its parts b and c (relay
7), §10 queue item 3.1-50's part a (relay 8), §11 queue item 3.1-47's part a (relay 9).

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

## 9. Queue item 3.1-49, parts b and c: averaging, the sphere's centre, distances, areas, curvatures (relay 7)

- **The stub for `MRIScomputeMetricProperties`.** `MRISsmoothSurface` ends with
  `MRIScomputeMetricProperties(mris)` (queue items 3.1-51 and 3.1-52, not ported yet). The call in
  `mrisurf_metricProperties.cpp.o` is an `R_X86_64_PLT32` relocation against the global symbol, so
  the probe's second build, `fssurfio-mpstub` (`-DPROBE_MP_STUB`, linked with
  `-Wl,--allow-multiple-definition`, the probe's object first), resolves the library's own call to
  the probe's stub (it writes `MP` and computes nothing): the averaging is compared alone. The same
  binary runs nothing else differently (no other command reaches the function); `check.py` sends
  only `metric`'s cases to it.
- **GCC 8.4's operand orders** (`objdump -dr -C` of the unit oracle's object; probe: seed 13, 31 of
  2,000 NaN-heavy cases differed before, 0 after):
  - `MRIScenterSphere_old` (inlined into `MRIScenterSphere`): each inlined `estimateNRG` sums
    `(dy² + dx²) + dz²` (the `y` square first), each `estimateSquaredRadius` and `computeGradient`
    `(dx² + dy²) + dz²`; the gradient's terms are `dx*e`, `dy*e` and `e*dz`; the trial centre
    `x + epsilon*dx` is `dx*epsilon + x` (and the accepted centre the same values); `epsilon /= 2.0`
    is `epsilon * 0.5`; the final move `cx + scale*(x - x0)` is `(x - x0)*scale + cx`; the two
    rescalings onto radius 100 sum `(y² + x²) + z²` in float. `(xlo + xhi) / 2.0f` is `* 0.5`.
  - The smoothing step of `MRISaverageVertexPositionsWkr_part1` (inlined into `MRISsmoothSurface`):
    `p + scale*(x - p)` is `(x - p)*scale + p`.
  - **The averaging's undivided store** (inlined into `MRISaverageVertexPositions`, `scale == 1`,
    `includeCenter` true): for a ripped vertex or one with `vnum <= 0` GCC stores the vertex's own
    position without the division by the count 1 (it knows the count), so a signalling NaN stays
    signalling there; with neighbours (even all ripped) it divides, which quiets it.
  - `mrisComputeVertexDistancesWkr`'s plane formula (`._omp_fn.5`): the storing path sums
    `zd² + (yd² + xd²)`, the checking path `zd² + (xd² + yd²)`; the square root is `sqrtss`
    (`std::sqrt(float)`), with `sqrtf`'s call only on the negative-argument branch (errno). The
    sphere's formula (`._omp_fn.7`): `fabs(angle) * radius`.
  - `XYZApproxAngle_knownLength` (`matrix.cpp.o`): the dot product's third product is `z2*n.z`
    (`mulsd %xmm1,%xmm2`, the neighbour's coordinate in the destination), the first two `n.x*x2`,
    `n.y*y2`. fs.core's `xyz_approx_angle_known_length` takes `n.z*z2`: one probe case (seed 17: a
    NaN unit vector, from an infinite position, against a signalling NaN neighbour) differed.
    `larmorx-freesurfer-surf`'s `metric.rs` keeps its own copy with the official order; fs.core's
    owner is told (HANDOFF).
  - `MRISpercentDistanceError`: `dist_scale = sqrt(orig_area / total_area)` is computed in float
    (`sqrtss`, then widened: a different number from the double square root of the widened
    quotient: seed 13's one numeric difference, `40a2a871fbf4fb4f` against `40a2a871fde6e735`);
    `mean_error += fabs(...)` is `fabs(...) + mean_error` (the call's result in `xmm0`, the
    destination); the `pct += fabs(dist - odist) / odist` of the loop is dead code (`pct` is
    overwritten after the loop) and is not compiled; `FZERO` calls the library's `fabs`
    (`-fno-builtin`).
  - `mrisComputeArea`: `d1 = -v1[1]*v0[2] + v0[1]*v1[2]` is `v0[1]*v1[2] - v1[1]*v0[2]` (no negation:
    a NaN's sign differs from the C expression's), likewise `d3`; `d2`'s products are swapped
    (`v0[2]*v1[0] - v1[2]*v0[0]`); the dot product is `x*d1 + d2*y + d3*z`; `sqrtss`; `-sqrt` by
    `xorps`.
  - `MRISscaleCurvatures` tracks the old range with `minsd`/`maxsd` (the same as the C comparisons:
    a NaN curvature leaves the end); `MRISsmoothCurvatures` computes `vertex->H * 0.4f` and the norm
    `(vnum*0.25f + 0.4f) + (v2num - vnum)*0.05f` (equal to the C order: no NaN can enter);
    `MRISscaleDistances` is vectorised (`mulps`, the distances first).
  - With no unripped vertex, `MRISzeroMeanCurvature`, `MRISnormalizeCurvature` and
    `MRISnormalizeCurvatureVariance` compute the mean as `0.0 / 0.0` directly (`divsd %xmm2,%xmm2`):
    the default NaN, as the C gives.
- **FS-114** (rule B). `MRIScenterSphere_old` on a surface with no vertex: `estimateNRG` returns
  0, `estimateSquaredRadius` and `computeGradient` divide by 0 (`0 / 0`: NaN). The outer loop
  starts (`0 < 0 + 1`), the step search `while (NRG >= last_NRG)` keeps `NRG = 0 >= 0` true, and its
  exit test `epsilon * d < 1e-11` is false for ever (`d` is NaN). The official build spins until
  the probe's 5-second alarm (`CRASH 14`, seed 7: 3 cases). Only an empty surface does it: with any
  vertex, a NaN gradient makes the energy NaN, which ends the search. Callers:
  `MRIScorrectTopology` (`mrisurf_defect.cpp` 7873: `mris_fix_topology`, recon-all's topology
  fix, on the canonical sphere), `MRISrecenter` (`mrisurf_metricProperties.cpp` 1504; called by
  `mri_transform` only) and `mris_topo_fixer` (324); no recon-all step centres an empty surface.
  larmorx refuses it in both modes (`MRIScenterSphere: the surface has no vertex, so it has no
  centre`, code -4); a never-ending loop is never reproduced (D13).
- **`mrisComputeOriginalVertexDistancesIfNecessaryWkr`** computes the original distances and then,
  unless `FREESURFER_fix_missing_dist_orig` is set, zeroes them (the comment: "the old code did not
  compute this distance ... the default is to zero the values after computing them"). Kept (rule
  C: the default the pipelines ran with).
- **Probe-only notes.** `MRIS_Status` is an enum whose storage is an `unsigned int` (GCC's
  underlying type without negative enumerators): a case's -1 prints as 4294967295 from the probe;
  the port's side prints the field's bits the same way. `cheapAssert`'s text is the stringified
  test, whose internal blank runs the preprocessor folds to one blank (`mris->orig_area > 0.0`,
  where the source has two blanks).
- **Not ported yet** (each with its reason): `MRIScomputeMetricProperties` (the fast and slow
  headers, 3.1-51/52) and so `MRISsmoothSurface`'s last step, `MRISrecenter` (also the saved vertex
  positions, 3.1-50's `MRISsaveVertexPositions`/`MRISrestoreVertexPositions`) and
  `MRISscaleBrainArea` (also `MRISscaleBrain`, 3.1-51's lines); `MRIScomputeOrigArea`,
  `MRISsetOrigArea`, `MRISstoreMetricProperties`, `MRISrestoreMetricProperties` (the face-normal
  cache's `getFaceNorm`, `setFaceOrigArea`, `getFaceOrigArea`, 3.1-50's lines 7740-7800, whose
  deferred entries compute `computeDefectFaceNormal_calculate`); `MRISrestoreOldPositions`,
  `MRISstoreCurrentPositions` (the saved positions); `MRISltaMultiply`, `MRIStransform` (an `LTA`, a
  `TRANSFORM`: fs.volume); `MRISnonmaxSuppress` (`MRIStoParameterization`, `MRISPblur`: fs.surf);
  `mrisurf_io_stl.cpp` and `mrisurf_io.cpp` 6314-6921 (the readers' `MRISalloc` and file I/O, 3.1-47).
  `MRIScenterSphere_new` is a failed assertion, never called. Brought forward from 3.1-50:
  `mrisComputeCurvatureMinMax` (9113-9136); from `mrisurf_vals.cpp`: `MRISextractCurvatureVector`
  (677-685).

## 10. Queue item 3.1-50, part a: saved positions and normals, canonical angles, the face-normal cache (relay 8)

- **GCC 8.4's code keeps the C order everywhere here** (`objdump -dr -C` of the unit oracle's
  `mrisurf_metricProperties.cpp.o`): `computeDefectFaceNormal_calculate_template<SurfaceFromMRIS::
  XYZPositionM::Surface>` is one out-of-line function that `getFaceNorm`, `computeDefectFaceNormal`
  and `recomputeOrUndeferFaceNorms`' OpenMP body all call (no per-site inlining); its differences,
  `F_CROSS`' products and differences and the squared length are in the macro's order; `len / 2.0f`
  is `len * 0.5f`; the second try's `100.0 * (v3 - v2)` (a double product) is compiled as the
  float product `(v3 - v2) * 100.0f` (the same value: the exact product of a float and 100 fits in
  a double, so both round once); `sqrt` of the float sum is `sqrtss` with `sqrtf`'s call only on the
  negative branch. `MRISaverageCanonicalRadius`: the bounding box's `if (x > xhi) xhi = x` is
  `maxsd` with the coordinate as the destination (`(x > xhi) ? x : xhi`, so a NaN never enters and
  `-0` against `+0` keeps the old end, as in C), `(lo + hi) / 2.0f` is `* 0.5`, the radius sum
  `radius + sqrt(...)` with `radius` the destination, the count `n` incremented per vertex (so the
  division is by `nvertices`, and `0.0 / 0.0` without vertices: `pxor` both, the default NaN).
  `MRIScomputeCanonicalCoordinates`: `r * r` once before the loop; `theta + 2π` with `theta` the
  destination; `r*r - z*z` in float; `atan2f` called through the PLT (glibc's). `MRIScomputeFolding`:
  `((area * d) * d)` added to the sum, `d = k1 - k2` in double. `MRIScomputeOrigArea`: `orig_area +
  fNorm->orig_area`.
- **FS-115** (`MRISrestoreVertexPositions`, lines 3299-3343): the second `switch`, for spherical
  patches, has no `TARGET_VERTICES` and no `LAYERIV_VERTICES` case, so they fall to `default:` =
  `TMP_VERTICES` (`cx = tx`), while the first `switch` restored `x = targx` (or `l4x`). The only
  callers that restore the target set are `mris_make_surfaces`, `mris_place_surface`,
  `mrisurf_mri.cpp` (8816, 9559) and `mrisurf_timeStep.cpp` (2307), on white and pial surfaces,
  never spherical patches; no caller restores the layer-IV set. Rule A: the restored set (as every
  other named set); notice when a vertex's canonical triple changes by it. The first `switch`'s
  `default:` beside `case TMP_VERTICES:` (any unnamed kind restores the temporary set) is kept
  (rule C): `MRIScomputeSurfaceNormals` (6474) saves the current positions in the temporary set and
  then restores its argument's kind, so with `CURRENT_VERTICES` that `default:` gives back the
  current positions; pipelines may rely on it. (`mrisurf_io.cpp` 4290, which restores its
  argument's kind before writing a surface, would write the temporary set for `CURRENT_VERTICES`:
  its callers pass named sets.)
- **FS-116** (`getFaceOrigArea`, line 7747): `fprintf(stderr, "%s:%d NYI\n", __FILE__, __LINE__)`
  then `*(int*)(-1) = 0`, when the deferral byte has bit 2. The bit is set only by
  `mrisurf_deferSetFaceNorms` (called from `mrisurf_defect.cpp` 1874, for the topology fixer's
  defect surfaces, which `mrisurf_undeferSetFaceNorms` (3242) or `getFaceNorm` clear later); the
  callers of `getFaceOrigArea` (`MRISrestoreMetricProperties`, `mrisurf_io.cpp` 163's
  `MRISreadTriangleProperties`) do not meet a deferred face on the recorded paths. Rule A: the area
  computed as `getFaceNorm` would (`computeDefectFaceNormal_calculate`'s fourth output), stored
  with `setFaceOrigArea` (bit 2 cleared, bit 1 kept: computing the normal early could change it if
  the original coordinates move before its first read).
- **Kept and documented (rule C):** `MRISrestoreNormals(WHITE_VERTICES)` is an error though
  `MRISsaveNormals(WHITE_VERTICES)` works (the restore's `case WHITE_VERTICES:` is grouped with the
  unsupported ones), and both functions' message says `MRISsaveNormals` (a copied line; messages
  are kept). `MRISsaveVertexPositions`' `cheapAssert(false)` and the normals' `ErrorExit` are inside
  the vertex loop, so a surface without vertices accepts any kind.
- **`mrisurf_deferSetFaceNorms`' two paths** (`static` `use_parallel` from
  `FREESURFER_deferSetFaceNorms_parallel`, read once per process): the `memset(..., 3, nfaces)` and
  the OpenMP loop store the same bytes; `CHECK_DEFERED_NORMS` is not defined in the build, so its
  checks are not compiled. The port keeps one loop.
- **`MRISrestoreMetricProperties`** writes `v->dist[n] = v->dist_orig[n]` for `n < vsize` with no
  test of either pointer: a vertex in use whose distances were freed after
  `MRISstoreMetricProperties` (for example by `MRISsetXYZ`'s callers that free them) crashes. The
  port panics there (an invalid access), the probe's `G` keeps the cases inside the buffers.
- **The probe** (`posn`, `validation/probes/fs-surfio/`): seed 13, 10,000 cases identical with
  `reproduce` (602 FS-116 crashes, 206 assertions of `MRISsaveVertexPositions`, 766 fatal errors,
  878 `G`); with the default, 347 cases differ only in `theta`/`phi` (correctly rounded `atan2f`:
  on the grid of k/4 for k in [-400, 400], glibc 2.35's `atan2f` differs from the correctly rounded
  value in 101,400 of 641,601 argument pairs, 15.8 %), every other difference with a notice.

## 11. Queue item 3.1-47, part a: the surface's life, rip flags, arrays, column printing, edges (relay 9)

- **glibc's `realloc` to 0 bytes (FS-117).** glibc 2.35's `__libc_realloc` frees a non-NULL block
  given 0 bytes and returns NULL (`REALLOC_ZERO_BYTES_FREES`, 1 by default), while `realloc(NULL,
  0)` is `malloc(0)`, a valid pointer. `MRISctr`'s `bzero` leaves every array NULL, so a surface
  made with 0 vertices or 0 faces holds `malloc(0)` arrays; the next `MRISreallocFaces(mris, 0, *)`
  frees `faces` and fails `cheapAssert(mris->faces)` (line 584: observed, `CRASH 11`, also for
  `MRISreallocVerticesAndFaces(mris, n, 0)` on a surface made with no face). `MRISreallocVertices`
  keeps `max_vertices` at least the old room, so room 0 only follows room 0: the first call frees
  `vertices`, returns false (the counts untouched; `MRISoverAllocVerticesAndFaces` ignores the
  result); the second `malloc(0)`s it and frees `vertices_topology`, `dist_storage` and
  `dist_orig_storage` (no NULL check on those three); and so on (observed). Rule A: empty arrays
  kept; `reproduce` = `alloc::c_realloc`'s glibc model. Callers: `mrisutils.cpp` 3675/3783 (upsampled
  surfaces), `mris_topology.cpp` 534/735/1132 (the topology fixer's copies), `mrisurf_io.cpp` 2998
  (VTK, after `nfaces <= 0` is refused): every recon-all surface has faces, so no impact known.
- **`MRISreallocVertices`' zeroing** starts at the old `nvertices`, not the old `max_vertices`: a
  vertex dropped by `MRIStruncateNVertices` loses its fields at the next reallocation. The vertices'
  `dist`/`dist_orig` pointers are `dist_storage[vno]`'s buffers; `bzero` NULLs the pointers and the
  storage keeps the buffers (`alloc::zero_vertex` moves the valid buffer back to the storage, the
  model of §8). A negative `nvertices` (only through `MRISgrowNVertices`/`MRIStruncateNVertices`
  with a negative count, which no caller passes) makes `bzero` write before the array: out of range
  (a panic; the probe guards it). `notifyActiveRealmTreesChangedNFacesNVertices` loops over the
  active realm trees (fs.surf, not yet ported): no tree is ever registered, the port asserts it.
- **`MRISctr`'s `bzero` of `MRIS` zeroes its `VOL_GEOM`** (a C++ struct with member initialisers:
  `xsize = 1`, `x_r = -1`, ...): `MRISoverAlloc` uses `malloc`, so no constructor runs and a new
  surface's geometry is all zeros (`mris::zeroed_vol_geom`; observed through `meta`).
  `MRIScopyMetadata` uses `VOL_GEOM`'s `operator=` (fs.mri's `VolGeom::assign`, FS-59's case), copies
  `lta` with `LTAcopy(src, NULL)` and the matrices with `MatrixCopy(src, NULL)` (the target's old
  ones leaked), never `free_transform`.
- **`MRIS::lta` is a placeholder** (`src/lta.rs`, the orchestrator's decision of 2026-11-13): the
  surface code only copies (`LTAcopy`: `LTAalloc(num_xforms, NULL)`, then `type`, `fscale`,
  `subject`, each `LTcopy` of `xforms` and `inv_xforms`) and frees it (`LTAfree`, in `MRISdtr` only
  when `free_transform`); the probe's `meta` checks `num_xforms` and `type` after the copy. Track
  A's `LTA` (fs.volume's 3.1-40, brought into `larmorx-freesurfer-mri`) replaces the module.
- **`MRISdtr`** frees nothing observable: `CTABfree` only when `ct` is set (no message), the vertex
  and face destructors (`VERTEX_TOPOLOGYdtr`, `VERTEXdtr`, `FACEdtr`, `MRIS_EDGEdtr` (not `gradU`),
  `MRIS_CORNERdtr`) for the in-use places, `lta`/`SRASToTalSRAS_`/`TalSRASToSRAS_` only with
  `free_transform`, `cmdlines[i]` for `i < ncmds` (FS-112's 1,000 kept in both modes, so the
  overflowed count never reaches it), `dist_storage[vno]` for `vno < max_vertices`. Its only callers
  are `MRISfree` and `utils/test/topology_test.c`.
- **`cprints`' 16,384-byte static buffer (FS-119)**: `sprintf(pch_right, " [ %s ]\n", right)` is
  `__sprintf_chk(pch_right, 1, 16384, ...)` in the unit oracle's `mrisurf_base.cpp.o` (the
  official programs import `__sprintf_chk` too): 16,377 bytes print, 16,378 abort with `*** buffer
  overflow detected ***: terminated` before the left column (observed). Callers:
  `mris_curvature_stats.cpp` 449-671, 2422-2431 (surface, label and texture names: user input) and
  `mris_calc.cpp`; recon-all's names are short: no impact known. `cprintd`/`cprintf` cannot overflow.
- **`MRIS_vertexProgress_print` (FS-118)**: `fivePerc = 0.05 * totalVertices` (int) and `avertex %
  fivePerc` is an `idiv` (unit oracle object): SIGFPE for 1 to 19 vertices at the first call, after
  the message and ` [` (observed, `CRASH 8`). Callers: `mrisurf_metricProperties.cpp` 6279, 6330,
  6424 (`MRIScomputeSecondFundamentalFormDiscrete`'s helpers: mris_curvature_stats' discrete
  curvatures) and `mrisurf_topology.cpp` 3188 (`MRIS_facesAtVertices_reorder`); recon-all's
  surfaces have more than 100,000 vertices: no impact known.
- **`MRISallocExtraGradients`** with a negative count: the three `calloc((size_t)n, 4)` overflow and
  fail with `errno` ENOMEM, so `ErrorExit` prints `Cannot allocate memory` first (observed; the port
  sets `g.errno`).
- **`VECTOR_elementIndex_findNotEqual`** returns the 0-based `i` while its comment promises the
  1-based index; no caller in 8.2.0 (nor of `VECTOR_elementIndex_find`, `signum_eval`,
  `MRIScheckForNans`, `MRISget*Array`): kept as written (rule C: nothing can rely on a correction).
- **`edgesIntersect`**: `include/mrisurf_base.h` 268 defines `SPHERE_INTERSECTION` 1, so the
  sphere version and `edgesIntersectStable` are compiled (the "original version" with
  `mrisComputeCanonicalEdgeBasis` is not). In the stable version `(v1->cx + v2->cx) / 2.0` adds the
  two floats in float before widening, and `v2->cx - v1->cx` subtracts in float; the rest is
  double. Only comparisons leave the functions, so the NaN operand order is irrelevant; the probe's
  `edges` (quarter grids for the aligned branch, 1e-5 steps for the stable one) is identical.
- **Not ported here**: `vlst_alloc`, `vlst_free`, `vlst_enough_data`, `vlst_add_to_list`
  (`VOXEL_LIST`, `utils/voxlist.cpp`: fs.volume); `mrisDirectionTriangleIntersection` and
  `mrisAllNormalDirectionCurrentTriangleIntersections` (`MHTacqBucket`: fs.surf's surface hash;
  `triangle_ray_intersect`: `utils/tritri.cpp`); `slprints` (no caller in 8.2.0; the clock and
  `gethostname`, and a NULL `strtok` result for an empty or all-dots host name).
- **The probe's artefact** (not FreeSurfer's): an empty `std::vector`'s `data()` is NULL, which
  `MRISsetRegistrationSigmas` reads as "reset to the defaults"; the probe allocates one more place.
