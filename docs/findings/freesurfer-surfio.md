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
    `larmorx-freesurfer-surf`'s `metric.rs` kept its own copy with the official order until fs.core
    took it (track A's 99562005, merged in relay 11): both callers (`metric.rs`, `mp.rs`) now use
    fs.core's.
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
  A's `LTA` (fs.volume's 3.1-40, brought into `larmorx-freesurfer-mri`, ba29424f) replaced it in
  relay 11: `src/lta.rs` re-exports fs.mri's `Lta`, `LTAcopy` and `LTAfree` (FS-195..198 are
  track A's); the probe's `meta` side builds the arrays with `LTAalloc` as the official probe does;
  the seed-1 hash unchanged. `Mris` lost its Rust `Clone` (the real `LTA`, like C's structure, has
  no copy but `LTAcopy`; nothing cloned a surface).
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

## 12. Queue items 3.1-51 and 3.1-52: the metric properties' three computations (relay 10)

`MRIScomputeMetricProperties(MRIS*)` (`mrisurf_metricProperties.cpp` 1844-1881) reads three
environment variables at every call (no script of 8.2.0 sets any): `FS_FASTER_MP` sends the call to
`MRIScomputeMetricPropertiesFaster` (`mrisurf_metricProperties_faster.cpp`) and returns before the
call counter; otherwise `useOld = FREESURFER_OLD_...` and `useNew = FREESURFER_NEW_... || !useOld`.
The default (`useNew` alone) frees the distances (`MRISfreeDistsButNotOrig`), loads an `MRIS_MP`
(`MRISMP_load(&mp, mris)`, no outputs), computes on it (`MRIScomputeMetricProperties(MRIS_MP*)`,
`_fast.h` 1154) and unloads (`MRISMP_unload(mris, &mp, false)`). `useOld` alone runs
`MRIScomputeMetricPropertiesWkr` (`_slow.h` 711) on the surface; both set: both run and the unload
only compares (`check` true). `MRISMP_copy` and `MRISMP_load`'s `loadOutputs`/`dx` arguments have
one caller, `MRIScomputeSSE_asThoughGradientApplied` (`mrisurf_deform.cpp` 993-1031), which sets
`useNewBehaviour = false;` unconditionally: never reached in 8.2.0 (ported, not probed).

- **The unload never writes the faces' angles back (FS-120).** `MRISMP_unload`'s face loop handles
  `orig_area`, `orig_angle` and the normal as special cases (`ELTX`) and copies `MRIS_MP__LIST_F_OUT`
  by `ELT` only: of that list only `area` is an `ELT` (`normSet`, `norm`, `angle` are `ELTX`), so
  `face->angle` keeps its old value (observed: the probe's dump after `mp` shows the case's angles;
  `MRISMP_copy` does copy `f_angle`, "not so special"; the check mode compares no angle either).
  Readers of `face->angle` after a default computation: `mrisComputeAngleAreaTerms`-type SSE terms
  (`mrisurf_sseTerms.cpp` 2457, the templated 2890), `mrisurf_compute_dxyz.cpp` 572-587,
  `MRISwriteTriangleProperties` (`mrisurf_io.cpp` 266), `geodesics.cpp` 76, `mris_errors`. With every
  computation leaving the angles stale, `MRISstoreMetricProperties` copies them into `orig_angle`, so
  `deltaAngle(angle, orig_angle)` is 0 unless something else (the old or faster computation,
  `MRIScomputeTriangleProperties` called directly, a triangle file) set them: the angle terms of
  `mris_sphere` and `mris_register` see no change. First classed rule A (FreeSurfer's two other
  computations set the angles; the omission is a slip of the generated lists); **changed to rule D
  (the orchestrator, 2026-11-13)**: the stale angles change recon-all's `mris_sphere` and
  `mris_register` results, unmeasured and possibly relied on, so the default keeps FreeSurfer's
  behaviour (`fixed_by_default` false, as FS-35/FS-36) until those ports measure it; the correction is
  `SurfGlobals::fs120_write_face_angles` with `fix` (the angles copied, a notice when one changes);
  `reproduce` leaves them. The hash test's `fs120_the_face_angles_are_written_back_only_on_request`:
  seed 1's 120 `mprop` cases (of the 30-command set since relay 11), no FS-120 notice by default, 33
  noted with the request, 30 of them printing other angles (in the other 3 a later step recomputes
  them; with relay 10's 29-command set: 24 and 21).
- **What the copy never writes (FS-121).** `MRISMP_load` `malloc`s the `V_OUT` and `F_OUT` arrays
  and fills them only with `loadOutputs`. (1) `v_nx/ny/nz` are written by `MRISMP_computeNormals` only
  when a vertex's normal is found (`!count || mrisNormalize(snorm) > 0`); a vertex still pending
  after the fifth trial (NaN or infinite coordinates; or degenerate faces with coordinates so large
  that 0.001 moves round away) keeps the malloc's contents, which the unload copies into
  `v->nx..nz` (and `mrismp_OrientPlane` reads `v_nz`). The old computation leaves such a vertex's
  normal unchanged. (2) `mrismp_OrientPlane` sums `f_area` over all of a vertex's faces, ripped ones
  too, but `MRISMP_computeTriangleProperties` never writes a ripped face's `f_area`: memory contents
  in a plane's vertex area (observed: the official output changes from run to run, e.g. `4453a748`
  then `c1f5912d` for one area; in the check mode it prints `no:3  v->area  30.661219 !=
  25927592685962668213346764849152.000000`). The port starts the copy's normals and face areas as the
  surface's (what the old computation reads), in both modes, with the notice; the probe counts these
  cases, never compares them.
- **The comparison lines (FS-122).** `comparisonWkr` compares with `lhs == rhs`; the unload always
  runs `comparison(-1, 1, mris->N==mp->N)` for `MRIS_MP__LIST_MRIS_IN` and `comparison(vno, v->N,
  mp->v_N[vno])` for `MRIS_MP__LIST_V_IN` (`ripflag`, `whitex..z`, `pialx..z`, `wnx..z`), even when not
  checking: a NaN there prints `no:<vno>  v->whitex  nan != nan  during
  count_MRIScomputeMetricProperties_calls:<n>` (up to 11 per part, the part's counter reset for the
  vertices and the faces) and, once per process, `b <...>/utils/mrisurf_metricProperties_fast.h:335
  needed`; a NaN radius prints `no:-1  1  1.000000 != 0.000000`. The expression text is the macro
  argument as written (`#LHS`), so `cmpAnglesPerTriangle(f->orig_angle,mp->f_orig_angle[fno])`,
  `fNorm->orig_area`, `mris_dist[n]`. Rule A: equal bit patterns are equal; `reproduce` prints.
- **The faster calculator (FS-123, FS-124, FS-125).** `new float[n]` leaves `face_areas`,
  `face_normals`, `face_angles` of ripped faces as memory contents (the face loop `continue`s), and
  the vertex loop sums every face of the vertex (no ripflag test), as `orientPlane`'s vertex loop
  does: memory contents in the vertex areas and normals next to a ripped face; `computeDimensions`
  reads `vertices[0..2]` of a surface with no vertex (`new float[0]`). The port fills 0 (the same
  sums as skipping) and gives an empty surface the default box, in both modes, with FS-123's notice.
  A vertex whose normal has no length is marked by `vertex_areas[vno] = -1`; after the fifth trial
  the loop ends without recomputing, so `transfer` writes area -1 and, for `origarea < 0`,
  `abs(-1)` (the `<cmath>` float `abs`: `andps` in the object) = 1, and the normal of no length
  (rule C, FS-124: an opt-in route; no value is right for a normal that cannot be computed).
  `reduction(+:total_area)` is OpenMP's own reduction (not ROMP's): the per-thread partial sums are
  combined in completion order, so the total's last bits depend on the threads (FS-125, never
  reproduced; the port gives the one-thread sum, faces in order). A vertex whose own face list lacks
  it reads `angles[3]` (the next face's first angle; past the array for the last face) with
  `FS_WEIGHTS_NORMS_BY_ANGLE`: the probe never generates such a list with the weights.
- **Reproducible sums.** The ROMP loops (`romp_for_begin.h`, `ROMP_Distributor_begin/end`,
  `romp_support.cpp` 459-501) sum each of at most 128 ranges from 0 and add the ranges' sums in order
  to the variable's start (`sum = sum + partial`, the accumulator first, as the object shows):
  `MRIScomputeTriangleProperties`' total area (both instances), `MRIScomputeAvgInterVertexDist`'s
  three sums (both, and the faster calculator's), the ellipsoid's and the faster plane's three area
  sums. The other parallel loops are independent per face or vertex (the retry list is sorted with
  `qsort`/`int_compare` of distinct vertex numbers: ascending), so one thread's results are every
  thread count's.
- **Operand orders read from the unit oracle's objects** (`mrisurf_metricProperties.cpp.o`,
  `mrisurf_metricProperties_faster.cpp.o`; `objdump -dr -C`):
  - `x *= -1.0f` is a sign flip (`xorps`) everywhere (face area, normal, angles, a plane's `nz`): a
    NaN's sign flips and it stays signalling. `neg_area += -area` is `neg_area - area` (`subss`); the
    faster `neg_area -= area` likewise.
  - The term comes first (`term + partial`) in the ROMP sums of the total area (triangle
    properties, both orientations, both instances, the faster face loop's OpenMP sum), the negative
    area and its original area (ellipsoid, faster plane and ellipsoid); `Sum2 += d*d` is `d*d +
    Sum2`, `Sum += d` and `N += 1` keep the accumulator first. The float sums of the plane
    orientation: `total = area + total`, `neg = neg - area`, `neg_orig = neg_orig + orig`; a
    vertex's area and normal sums keep the accumulator first, but the faster weighted sum is
    `fn*angle + acc` (the third product `angle*fn`).
  - `mrisFaceAreaNormal`/`mrismp_setFaceAreaNormal`/`mrisNormalFace`/`mrisTriangleArea`'s
    `-v12[1]*v20[2] + v20[1]*v12[2]` is `v20[1]*v12[2] - v12[1]*v20[2]` (no negation; likewise the
    third component), the second component as written.
  - `MRIScomputeTriangleProperties_extracted.h`: `V3_CROSS_PRODUCT`'s first product is `b.z*a.y`;
    `V3_LEN` of the area `sqrt(z² + (x² + y²))`, of `V3_NORMALIZE` `sqrt((y² + x²) + z²)`; the
    normalize's third product `n.z*s` in `MRIScomputeTriangleProperties` and `s*n.z` in
    `MRISMP_computeTriangleProperties` (the only difference of the two instances); `V3_DOT(a, b)`
    `a.z*b.z + (a.x*b.x + a.y*b.y)`; `VectorTripleProduct` out of line (fs.core's order).
  - `MRISMP_computeVertexDistancesWkr`'s plane formula stores `sqrt(zd² + (yd² + xd²))`, as the
    `MRIS` worker (relay 7); the sphere's formula calls `XYZApproxAngle_knownLength` out of line.
  - The faster calculator: the face normal `a x b` with the second component's second product `b2*a0`
    (also in the angles' `b x a`), its length `(x² + y²) + z²`; the corner's `vec_dot(c, fnorm)`
    `c2n2 + (c1n1 + c0n0)` and `vec_dot(a, b)` `(b1a1 + a0b0) + b2a2`; a vertex normal's length and the
    straight-line distance `z² + (x² + y²)`; `moveVertexRandomly`'s hash `f*(2r) - r` (the same
    value as `(HI-LO)*f + LO`) added as `random + pos`; `area /= 2.0` is `area * 0.5`.
- **Callers ported with the computation**: `MRISsmoothSurface`'s last step, `mrisDisturbVertices`,
  `MRISrecenter`, `MRISscaleBrainArea`, `MRISscaleVertexCoordinates`, `MRISscaleBrain` (its
  `cheapAssert(mris_src == mris_dst)`: one surface in the port).
- **Not ported here**: `mrisSampleMinimizationEnergy`, `mrisSampleNormalEnergy`,
  `mrisSampleSpringEnergy`, `mrisSampleParallelEnergy(AtVertex)` and `MRISsampleFaceCoordsCanonical`
  (`_slow.h` 734-1008: the surface hash `MHT` and `MRISsampleFaceCoords`, fs.surf); the rest of
  3.1-51's lines (`mrisurf_metricProperties.cpp` 9465-13617: gradients' averaging, second
  fundamental forms, curvature uses, radii, `MRISbinarizeVolume`, orientation marks, the surface
  checks, smoothing kernels, `MRISclone`, the statistics prints): later relays.

## 13. Queue item 3.1-47, part b: the small file readers and writers (relay 11)

`utils/mrisurf_io.cpp` 64-89 (`mris_readval_frame`, `MRISsetReadFrame`, `MRISgetReadFrame`),
99-211 (`MRISreadTriangleProperties`), 213-275 (`MRISwriteTriangleProperties`), 343-401
(`MRISwriteDists`), 535-584 (`MRISwriteAreaError`), 625-666 (`MRISwriteAngleError`), 668-719
(`MRISwriteCurvatureToWFile`), 2042-2077 (`MRISreadValuesBak`), 2079-2111 (`MRISreadImagValues`),
2113-2146 (`MRIScopyMarksToAnnotation`, `MRISmaxMarked`), 2625-2686 (`MRISwriteAscii`,
`MRISwriteNormalsAscii`), 2794-2853 (`MRISwriteVTK`, `MRISwriteCurvVTK`); `utils/fio.cpp`'s
`fread4`/`fwrite4` (the counts), `utils/utils.cpp` 788-808 (`FileNamePath`), `include/macros.h` 70
(`DEGREES`). Port: `larmorx-freesurfer-surf`'s `src/sfile.rs`; probe command `sfile`.

- **The counts of the triangle-property files (FS-127).** `MRISwriteTriangleProperties` writes
  `fwrite4(mris->nvertices, fp)` (`int fwrite4(int v, FILE *fp)`: the integer's bytes, swapped),
  `MRISreadTriangleProperties` reads `fread4(&f, fp); vnum = (int)f;` (`int fread4(float *v, FILE
  *fp)`: a float): the integer 10's bytes are the denormal 1.4e-44, `cvttss2si` gives 0, so the
  vertex check fails for every written file of a surface with vertices (observed: the probe's `wt`
  then `rt` of one case, `incompatible vertex number in file d/lh.triangle_area`). The only caller
  of the reader is under `#if AVERAGE_AREAS` in `mrisurf_integrate.cpp` 453-456 (0 in 8.2.0), and
  the writer has no caller: neither is reached by recon-all. Rule A (read the writer's integers);
  the conversions are `cvttss2si` (the object code: `cvttss2si 0x3c(%rsp),%r14d` and `%r15d` after
  each `fread4`), so `reproduce` converts through x86-64's `(int)` of the float (NaN and out of range
  -2147483648).
- **The face count is never checked (FS-133).** The reader loops `fno < fnum` over
  `mris->faces[fno]` and `setFaceOrigArea(mris, fno, f)` (the cache entry `fno`), past `nfaces` and
  past `max_faces`. Rule B (refuse a count other than the surface's); `reproduce` keeps writing the
  spare places (the arrays hold `max_faces`, zeroed above `nfaces` by `MRISreallocFaces`) and
  refuses beyond them (writing past them is heap corruption: never reproduced). The probe's
  generator keeps the float counts within the places (a guard would have to parse the file).
- **The angle loop's stale counter (FS-128).** The writer's second loop is `for (vno = 0; vno <
  mris->nfaces; vno++) { face = &mris->faces[fno]; ...}` with `fno == nfaces` from the first loop:
  `nfaces` copies of `faces[nfaces]`'s angles (observed: a spare place's 0.1/0.2/0.3 in the probe's
  first hand case; with `nfaces == max_faces` the bytes after the array, 0.577 three times in that
  build, memory). Rule A. The probe skips (`G`) the writer when `nf == maxf` (memory).
- **The hemisphere before the name (FS-129).** `cp = strrchr(name, '.'); strncpy(hemi, cp - 2,
  2)`: a dot at index 0 or 1 reads one or two bytes before the string. The probe passes every name
  with two NUL bytes before it (`name_tok`), which makes FreeSurfer's hemisphere empty, as
  `reproduce` takes it (observed: `./.triangle_area`). Rule B.
- **Names (FS-126, FS-134).** Measured on the probe (`fssurfio`, linked with the unit oracle's
  libutils, Ubuntu's gcc-8 with its default `_FORTIFY_SOURCE`): `MRISwriteAreaError`'s `strcpy(fname,
  name)` and `MRISwriteDists`'s `strcpy` branch abort at 1,024 bytes (`__strcpy_chk`: `*** buffer
  overflow detected ***: terminated`, signal 6), 1,023 pass; the `snprintf` branches print
  `<__FUNCTION__>: Truncation on line <__LINE__>` (`std::cerr`, the lines of the `cerr` statements:
  126, 174, 236, 254, 361, 369, 377, 384) and open the cut name; `FileNamePath(mris_fname, fpref)`
  (`utils.cpp` 792 `strcpy(pathName, fname)`, unchecked: the destination's size is unknown there)
  overruns `fpref[STRLEN]` for names of 1,024 bytes or more: at 1,100 bytes the function went on
  (the cut message, `could not open <first 1,023 bytes>`), at 2,100 bytes `*** stack smashing
  detected ***` at its return. `MRISwriteDists`' `FileNamePath(mris->fname, path)` cannot overrun:
  `MRIS::fname` is a `FixedSizeArray<char, STRLEN>`.
- **The angle error (FS-130, FS-131).** `error /= (float)(v->num * ANGLES_PER_TRIANGLE)` is inside
  the face loop (`divss` at 0xff, in the loop body); `i = DEGREES(error) * 100.0;` is `cvttsd2si`
  (0x127), after `mulsd` 360, `divsd` 2π, `mulsd` 100 (the error in the destination each time).
  Callers: `mris_errors`, `mris_flatten` (not recon-all).
- **The largest mark (FS-132).** `max_marked = mris->vertices[0].marked;` before the loop from 1
  (which skips ripped vertices); for `nvertices == 0` the array is `realloc(NULL, max_vertices *
  sizeof(VERTEX))`: with `max_vertices` 0 a 0-byte block (observed: 12336, memory), else a zeroed
  place (0). Caller: `mris_make_face_parcellation` (not recon-all).
- **The official build's operand orders** (`objdump -dr -C` of the unit oracle's
  `mrisurf_io.cpp.o`; `<scratch>/D/relay11/io.asm` in the session): the reader's `orig_area += f`
  is `movss orig_area,%xmm0; addss f,%xmm0` (the sum on the left); `v->origarea +=
  getFaceOrigArea(...)` is `addss 0x194(%r15),%xmm0` with the returned area in `%xmm0` (the face's
  area on the left); `MRISwriteAreaError`'s two sums keep the sum on the left (`addss 0xc(%rax),%xmm1`,
  `addss 0xc(%rbx),%xmm0`), then `subss`, `mulss` 100.0f, `cvtsi2ss` of `num`, `divss`, `nint(double)`;
  `MRISwriteCurvatureToWFile`'s `sum += f` keeps the sum on the left (`addsd %xmm0,%xmm5`) but `sum2
  += f * f` is `mulss %xmm1,%xmm1; cvtss2sd; addsd 0x10(%rsp),%xmm1` (the square on the left: seed
  7's one NaN-sign difference before the fix, `stdev = nan` against `-nan`); its `sqrt` is a call
  to the C library's (`R_X86_64_PLT32 sqrt`), made whether or not `DIAG_SHOW` prints the result:
  glibc 2.35's wrapper sets `EDOM` for a negative argument, and a later `ErrorPrintf` of the process
  prints it. The extremes are `minsd`/`maxsd` with the value in the destination: a NaN keeps the
  extreme, as the C comparisons.
- **Not ported here (wait for `mri_identify`, fs.io):** `MRISwriteValues` (its first step is
  `mri_identify(sname)`, which probes the file's contents for every format when the extension does
  not decide) and `MRISwriteAreaErrorToValFile` (its only output goes through `MRISwriteValues`).
  Found while reading it: its loop reads `getFaceNorm(mris, fno)` with `fno` the vertex's face
  **index**, not the face number (`vt->f[fno]`): the original areas of faces 0..num-1 of the
  surface, not the vertex's (for the register when it is ported).
- **Measured (relay 11).** Seed 7, `sfile` alone: 1,000 cases, every comparable one identical with
  `reproduce` (49 counted, not compared: FS-167's memory digits in the readers, FS-06's table
  arctangent in the writer's triangle properties); the default differs in 412, every one with a
  notice. The whole probe, seed 7 (60,000 cases of 30 commands): 59,497 identical with `reproduce`,
  503 counted, not compared (404 `mprop`, 99 `sfile`), the default different only with notices or the
  correctly rounded maths (5 `surf`, 77 `posn`), the port's lines identical under AArch64's NaN
  emulation in both modes (70,295 and 70,313 flips). Seed 1 (900 cases of 30 commands): 894
  identical, 6 `SKIP`; hash `d927ce4b...` passing in debug and release and under `NAN_TRAP_ARM=1`
  with `NAN_TRAP_DIR` set (1,925 and 13,214 NaNs flipped).

## 14. Queue item 3.1-48, part a: per-vertex value files and other formats (relay 12)

What `utils/mrisurf_io.cpp` lines 2861-6287 hold, what was ported (`src/curvio.rs`, `src/formats.rs`,
`src/scanf.rs`; spec part 9, §56-65) and what waits.

- **The probe reaches the file-local functions** (`static` in `mrisurf_io.cpp`: `__mrisreadcurvoldformat`
  has no symbol at all, inlined into its callers; `MRISreadCurvatureIntoArray`,
  `MRISreadNewCurvatureIntoArray`, `__MRISapplyFSGIIread`/`write` are local `t` symbols) through
  `validation/probes/fs-surfio/io_statics.cpp`, which includes `mrisurf_io.cpp` and is compiled with
  that file's own command from the unit oracle's `compile_commands.json` (track A's model,
  `fs-io/gifti_statics.cpp`); the probe then takes every `mrisurf_io.cpp` function from that object.
  Checked: the seed-1 hash of relay 11 (`d927ce4b...`, 900 cases, `sfile`'s 30 included) is unchanged
  with it.
- **FS-136** (`mrisReadAsciiCurvatureFile` line 788): `ErrorReturn(..., "%s could not open file %s.\n",
  mrisReadAsciiCurvatureFile, fname)` passes the function itself to `%s`: `printf` prints the bytes of
  its machine code up to the first 0 (`endbr64`, the six pushes, `mov %rdi,%rbx`, `mov %rsi,%rdi`, `sub
  $0x438,%rsp` up to its `00`), the same in the unit oracle's `mrisurf_io.cpp.o` (`objdump -d`) and in
  the probe's copy. Rule A: the name.
- **FS-138** (`MRISreadNewCurvatureIntoArray` line 5920): `ErrorReturn(ERROR_BADFILE, (ERROR_BADFILE,
  "MRISreadNewCurvatureIntoArray(%s): fopen failed"))`: the `%s` has no argument; `ErrorPrintf` prints
  the `errno` line, then `vsnprintf` reads a pointer from the register of the absent argument: a
  segmentation fault in the probe, every time (what it holds depends on the caller). Reached directly
  by `MRISreadNewCurvatureVector` with a missing file (`MRISreadCurvatureIntoArray` reopens a file it
  has just opened). Rule A; `reproduce` crashes after the `errno` line.
- **FS-137** (`mrisReadAsciiCurvatureFile`): `if (!cp) break;` (the source's own comment: `???should
  check if the vno match???`). Rule B.
- **FS-139, FS-140** (`MRISreadDecimation`): `int k, d, ndec;` — `d` is not initialised, `fscanf(fptr,
  "%d", &d)` leaves it when it fails (FS-140: the count; a stack slot of the frame), each later failed
  `fscanf` leaves the last value (the count for the first vertex); the binary branch's `char c =
  fgetc(fptr)` turns `EOF` into -1, not `'\0'`, so every missing byte is "decimated". Both errors
  return 0 through `ErrorReturn(0, ...)` with an extra argument `d` the format never prints (kept:
  rule C, an error FreeSurfer reports, 0 decimated).
- **FS-141, FS-142** (`MRISwriteGeo`, `MRISwriteICO`): Geo's `int vnos[300000]` is a stack array
  written only for unripped vertices and read for every unripped face's vertices (an unripped face
  of a ripped vertex reads a value never written; more than 300,000 vertices write past it); ICO
  numbers vertices with `actual_vno` but writes `face->v[n] + 1`. The probe skips Geo's case (`G`);
  ICO's is defined and compared.
- **FS-143, FS-144** (`MRISreadVTK` lines 2958-2966): `float x,y,z; fscanf(fp, "%f %f %f", &x, &y,
  &z);` inside the loop, the return value unchecked: the three are stack slots that keep the last
  values assigned (observed: every comparable case with a short vertex identical with the model
  "keep the number read before"); the first vertex's are memory contents (FS-144, skipped).
- **FS-145** (`MRISreadVTK` lines 3061-3076): `cp = fgetl(...)` after `POINT_DATA` and per field, then
  `sscanf(cp, ...)` without a test: `sscanf(NULL, ...)` faults (`CRASH 11` in the probe). **FS-146**:
  `if (fscanf(fp, "%f", &f))` is true for `EOF` (-1), so `f` stays `0.0`. **FS-147**:
  `mris->vertices_topology[vno].num++` on a `uchar` (and the same in the other readers of the
  surface, `mrisReadTriangleFile`, the quadrangle reader, `mrisReadAsciiFile`, `mrisReadGeoFile`:
  their ports will note FS-147 too; `MRISreadOverAlloc_new` then allocates `num` faces per vertex and
  writes one per face: a wrapped count writes past the allocation).
- **FS-148, FS-149** (`MRISbuildFileName_read` lines 5660-5708): `*(dot - 1)`, `*(dot - 2)` before the
  name; `sprintf(fname, "%s/%s%s", ...)` and `strcpy(fname, sname)` into the caller's `char *`
  (unchecked: `_FORTIFY_SOURCE` checks only buffers whose size the compiler sees). The same in
  `__MRISapplyFSGIIwrite`'s `sprintf(file_to_write, ...)`; its and `__MRISapplyFSGIIread`'s local
  `char gii[1024]` is checked (FS-126's abort). The names of `MRISreadCurvatureVector`,
  `MRISreadFloatFile`, `MRISreadNewCurvatureVector`, `MRISreadBinaryAreas` are FS-126's (`snprintf` with
  the `Truncation` line; `strcpy` into a local `char[STRLEN]`: checked, abort) and FS-134's
  (`FileNamePath` into a local `fpref[STRLEN]`, unchecked).
- **`scanf`'s `errno`** (`src/scanf.rs`, `validation/investigations/glibc-scanf-probe/scanf_errno.c`, open): the
  readers' error messages print `errno`, and glibc's `scanf` changes it at the end of the input in
  ways fs.core's `cscan::Call` did not model: a later read of the same call restores the `errno` of
  the first end (so `1e99`'s `ERANGE` is undone by a following directive), the white space before a
  conversion is skipped with `errno` 0, and `inf` looks one character on while `nan` and `infinity`
  do not. Found by the probe: a `nan` ending a curvature line left `errno` 0 in FreeSurfer's
  message; the first model (any conversion that ends the input counts) printed the earlier
  `ENOENT`. The model is the measured behaviour, no glibc code; fs.core's `Call` could take it (for
  track A).
- **The official build's operand orders** (`objdump -dr -C` of the unit oracle's `mrisurf_io.cpp.o`
  and `mrisurf_metricProperties.cpp.o`): `MatlabPlotFace`'s centre `cx += v1->x` is `cvtss2sd
  0x18(%r15),%xmm0; addsd 0x10(%rsp),%xmm0` (the corner on the left), `/ 3` is `divsd` with the sum on
  the left; the normal's end `cx + NormLen * snorm[k]` is `mulsd %xmm4,%xmm5` (`snorm` on the left)
  then `addsd %xmm7,%xmm5` (the product on the left); `MatlabPlotVertex` likewise (`nx * NormLen`, then
  `+ x`). The out-of-line `mrisNormalFace` (the one `MatlabPlotFace` calls; its copies inlined into
  `MRIScomputeNormals` are relay 10's) computes the cross product's second component as `v0[2] *
  v1[0] - v1[2] * v0[0]` (`mulss %xmm3,%xmm2; mulss %xmm1,%xmm4; subss %xmm4,%xmm2`), each product's
  operands the other way round from the source's `v1[0]*v0[2] - v0[0]*v1[2]`; its final normalisation
  sums the squares as `n2² + (n1² + n0²)` (the same value; a NaN sum never divides). Before these,
  13 of 6,000 NaN-heavy `cfile` cases differed (all in the plots); after, none.
- **Taken early from fs.surf** (queue items 3.4-02 and 3.4-03, the same crate): `MRISvalidVertices`
  (`mrisurf_topology.cpp` 306-316) and `mrisValidFaces` (1836-1846), the writers' counts
  (`topology::valid_vertices`, `valid_faces`).
- **What waits, and on which item** (3.1-48's other functions and 3.1-49's `mrisurf_io.cpp` part):
  - fs.io's format identification `mri_identify` (3.1-27: `IDextensionFromName`, `mri_identify`):
    `MRISfileNameType` and so `MRISread`, `MRISreadOverAlloc`, `MRISfastRead`, `MRISwrite`,
    `MRISwrite_new`, `MRISwriteVertexLocations`, `MRISreadVerticesOnly`, `MRISwritePatchAscii`;
    `MRISreadCurvatureFile_getfilename` and so `MRISreadCurvatureFile`, `MRISreadBinaryCurvature`,
    `MRISreadNewCurvatureFile`, `MRISreadMarked`; `MRISwriteValues` (and so `MRISwriteCropped`,
    `MRISwriteAreaErrorToValFile`); `MRISwriteCurvature_getfilename`/`MRISwriteCurvature` (and so
    `MRISwriteMarked`, `MRISwriteArea`); the annotation file names.
  - transform.cpp's `readVolGeom`, `writeVolGeom`, `vg_i_to_r`, `TkrVox2RASfromVolGeom` (3.1-39, track
    A; small: the text block of a surface file's volume geometry) and a stream `rewind`/`fseek` in
    fs.io's `CFile` (track A): `mrisReadTriangleFile`, `__MRISreadQuadrangleFile` (`rewind` for the
    oldest format), `MRISwriteTriangularSurface`, `__MRISwriteQuadrangleFile`,
    `__MRISwriteTriangularSurfaceTags`: the main surface reader and writers. `LTAreadEx` and
    `MRIreadHeader`, `getVolGeom` (3.1-39/3.1-28): `mrisReadTransform`.
  - fs.surf (wave 3.4, this crate): `mrisCompleteTopology` (3.4-03), `mrisCheckVertexFaceTopology`,
    `MRISsetNeighborhoodSizeAndDist`/`MRISresetNeighborhoodSize` (3.1-50's lines: mine), `ICOreadOverAlloc`
    (icosahedron.cpp, 3.4-01), `MRIScopyMRI`/`MRIcopyMRIS` (mrisurf_mri.cpp), `MRISPread` and
    `MRISfromParameterizationBarycentric` (mrisp.cpp), `SetSurfHopList` (`MatlabPlotVertexNbhd`):
    `MRISread`'s completion and `MRISreadParameterizationToSurface`, `MRISwriteField`.
  - fs.io's volume I/O (3.1-28: `MRIread`, `MRIwrite`): `__mrisreadcurvmri`, `MRISwriteField`, the group
    average area of `MRISread`. gifti.cpp (3.1-26: `mrisReadGIFTIfile`, `MRISwriteGIFTI`): the GIFTI
    branches. STL (`mrisurf_io_stl.cpp`, 3.1-49, mine): next.
- **Measured (relay 12).** `cfile` alone, seeds 7, 13 and 17: 2,000 cases each, every comparable one
  identical with `reproduce` (83, 89 and 81 counted, not compared). The whole probe, seed 7 (62,000
  cases of 31 commands): 61,334 identical with `reproduce`, 666 counted, not compared (455 `mprop`, 125
  `sfile`, 81 `cfile`, 5 `metric`), the default different only with notices or the correctly rounded
  maths (71 `posn`, 4 `surf`), the port's lines identical under AArch64's NaN emulation in both modes
  (67,516 and 67,552 flips). Seed 1 (930 cases): 919 identical, 11 `SKIP`; hash `17da0559...` passing
  in debug and release and under `NAN_TRAP_ARM=1` with `NAN_TRAP_DIR` set (2,897 and 12,690 flips).

## 15. Queue items 3.1-48 part b and 3.1-49's STL: the raw surface readers, STL (relay 13)

What was ported (`src/surfread.rs`, `src/stl.rs`; spec part 10, §66-72), the defects found and how
they were decided, from `utils/mrisurf_io.cpp` 3342-3518, 4341-4410, 5387-5492 and
`utils/mrisurf_io_stl.cpp`.

- **The probe reaches the file-local readers** through `io_statics.cpp`'s wrappers (relay 12's
  model): `mrisReadAsciiFile`, `mrisReadGeoFile`, `mrisReadGeoFilePositions`,
  `mrisReadTriangleFileVertexPositionsOnly`, `mrisReadTriangleFilePositions`, `quadCombine`.
  `mrisReadSTL` and `mrisWriteSTL` are external symbols of libutils, declared only in
  `mrisurf_io.cpp`; the probe declares them itself.
- **Unchecked `scanf` calls (FS-151, FS-152).** `mrisReadAsciiFile`: `sscanf(cp, "%d %d\n",
  &nvertices, &nfaces)` unchecked (uninitialised locals: FS-151); per vertex `float x,y,z;
  fscanf(fp, "%f  %f  %f  %d\n", &x, &y, &z, &rip)` unchecked, `rip` a function-scope `int` shared
  with the faces' `fscanf(fp, "%d\n", &rip)`; the face vertices `fscanf(fp, "%d ", &face->v[n])`
  straight into the face (0 from `MRISalloc`'s `bzero`). `mrisReadGeoFile` and
  `mrisReadGeoFilePositions`: `float x,y,z; fscanf(fp, "%e %e %e", ...)` unchecked. The loop
  variables are address-taken stack slots that keep their last values (observed, as `MRISreadVTK`'s,
  FS-143): `vertex 4 x 6` after `vertex 1 2 3` gives `4 2 3`. The first vertex's unread values are
  memory contents (FS-151; counted, not compared). `mrisReadAsciiSTL`: the `v` and `f` cases'
  `float x, y, z` share one set of slots in the official build (`objdump -dr -C` of the unit
  oracle's `mrisurf_io_stl.cpp.o`: both `sscanf` calls pass `0x98(%rsp)`, `0x9c(%rsp)`,
  `0xa0(%rsp)`; observed: `facet normal` with no numbers after `vertex 4 2 3` gets the normal
  `4 2 3`). Decided by rule B (a file whose numbers stop or hold something else makes no sense; the
  readers report success with made-up values): a clear error by default.
- **FS-150**: `cp = fgetl(...)` then `sscanf(cp, ...)` with `cp` `NULL` for a file with no line that
  is not blank or a comment (`sscanf(NULL, ...)`: `strlen(NULL)`, signal 11; observed `CRASH 11`).
  Rule B.
- **FS-153**: `vertices_per_face = nedges / nfaces;` (`int`): `nfaces` 0 (or -1 with `INT_MIN`
  edges) raises SIGFPE (observed `CRASH 8`). Rule B.
- **FS-154**: `mrisReadGeoFile`'s face loop `face->v[n]--; mris->vertices_topology[face->v[n]].num++`
  with no range check: 0 in the file (or a number that cannot be read, or a last vertex without its
  `-`: `fscanf(fp, "-%d\n", ...)` fails at the digit) gives -1, an index above the count writes past
  the array (the probe's small cases did not crash; what the write changes depends on the heap's
  layout). Rule B; counted, not compared.
- **FS-155**: `mrisReadGeoFilePositions` reads `mris->nvertices` coordinates whatever the file's
  counts (it reads them only for the edges-per-face test). `mrisReadTriangleFilePositions` compares
  both counts; rule B, as that reader does.
- **FS-156**: `mriss->vertices = calloc(nvertices, sizeof(SMALL_VERTEX)); if (!mriss->nvertices)`
  tests the count, not the pointer: a negative count's `calloc` fails (`ENOMEM`, observed in the
  next message's `errno` line) and the function returns the surface with `NULL` vertices and the
  negative count; a count of 0 gets the allocation message. Rule B: the message the test meant.
- **FS-147** (relay 12's) holds for `mrisReadAsciiFile` and `mrisReadGeoFile` too (`uchar num`);
  for the STL reader the wrapped count is also the size of the face lists `buildSurface` allocates
  (`calloc(vt->num, ...)`) and then fills with every face: **FS-157**, a heap overflow; rule B,
  counted, not compared.
- **FS-158** (`pointHash`): `int hx = 1000.0f * logf(abs(x) + 1.0f)` is the float `abs` (`andps`),
  `addss`, glibc's `logf` (PLT), `mulss` by 1000, `cvttss2si` (NaN and infinity give `INT_MIN`);
  `imul` by 2017, 1029, 1159 (wrapping), `xor`, then C's `%` by 2^20 (`cltd; shr $0xc; add; and
  $0xfffff; sub`: the remainder keeps the dividend's sign). Finite coordinates give at most
  `1000 * logf(FLT_MAX) = 88722`, products below 2^28: never negative. One or three `INT_MIN`
  terms make the hash negative, and the remainder negative unless its low 20 bits are 0:
  `hashTable[negative]` reads (and, for a new key, writes) the bytes before the vector's buffer.
  The chain never changes which vertex a point is (equality is tested on the coordinates), so the
  port uses a map and computes the index (glibc's `logf`, `larmorx_core::math::logf_glibc`) only for
  the notice. Rule A (the lookup corrected; the result where FreeSurfer is defined unchanged).
- **FS-159**: `buildSurface` reads `vertices[3*faceNo + k]` for every facet; a facet of fewer than
  three corners leaves fewer than `3 * nfaces` entries and the vector's value-initialised `nullptr`
  is dereferenced (`keyPtr->vertexNo`, observed `CRASH 11` with no output). Rule B.
- **FS-160**: `unsigned int fileSizeIfBinary = 84 + ntriangles * 50;` wraps; a file whose size
  matches a wrapped product is read as binary and `readFloat`'s `stream.read` fails past the end,
  leaving the uninitialised `float value` (memory). Rule B. Not generated by the probe (the counts
  involved, about 86 million triangles, would need gigabytes).
- **`mrisReadSTL`'s size test for short files** (no defect): `stlfile.read(header, 80)` and the
  count's `read` on a file of fewer than 84 bytes set `failbit`; `seekg(0, end)` then fails and
  `tellg` returns -1, so `fileSize` is 2^32 - 1, odd, never `84 + 50 n` (even): such files are
  always text, whatever the uninitialised `ntriangles` holds.
- **Not defects, noted**: `mrisReadAsciiSTL` never closes its `FILE` (and its `fopen` is unchecked:
  `mrisReadSTL` opened the file just before); `buildSurface`'s third `ErrorExit` passes `vt->n` (a
  pointer) to `%d`, reachable only when `calloc` of a byte count below 256 fails; the ASCII STL
  reader decides a line by its first character only (`vertices 1 2 3` is a vertex line whose
  numbers cannot be read: FS-152).
- **`mrisReadTriangleFile*`'s `fscanf(fp, "\n")`** after the creator's line also eats count bytes
  that are white space (a vertex count whose first big-endian byte is 9 to 13 or 32: 150,994,944 or
  more vertices); the port does the same through its `scanf` model. Not reachable with real surfaces.
- **Measured.** `sread`: seed 7, 1,000 cases (one ended by the official probe's 5-second alarm under
  the machine's load, identical when rerun alone) and seed 13, 2,000: every comparable case
  identical with `reproduce` (92 and 156 counted, not compared), the default different only with
  notices. `stl`: seed 7, 1,000 cases identical with `reproduce` (105 counted, not compared), the
  default different only with notices. Seed 1 (990 cases of 33 commands): 980 identical, 10 `SKIP`,
  hash `2543d270...` (`tests/fs_surfio_official.rs`), passing under `NAN_TRAP_ARM=1` with
  `NAN_TRAP_DIR` set in debug (7,746 flips) and release (17,227).
  The whole probe, seed 7 (66,000 cases of 33 commands): 65,018 identical with `reproduce`, 982
  counted, not compared (435 `mprop`, 198 `stl`, 151 `sread`, 115 `sfile`, 81 `cfile`, 2 `metric`),
  the default different only with notices or the correctly rounded maths (65 `posn`, 7 `surf`), the
  port's lines identical under AArch64's NaN emulation in both modes (74,230 and 74,272 flips).
- **What still waits** (as §14): `mri_identify` (track A's 3.1-27) for `MRISread`/`MRISwrite` and the
  `*_getfilename` functions (3.1-49's `mrisurf_io.cpp` 6690-6921); `readVolGeom` and friends (track
  C's 3.1-39) and `CFile`'s `rewind`/`fseek` (track A) for `mrisReadTriangleFile`,
  `__MRISreadQuadrangleFile`, `__MRISwriteQuadrangleFile`, `MRISwriteTriangularSurface`,
  `__MRISwriteTriangularSurfaceTags` (3.1-49's `mrisurf_io.cpp` 6314-6689).

## 16. Queue item 3.1-48: the binary triangle and quadrangle files (relay 14)

What was ported (`src/trifile.rs`; spec part 11, §73-77), the defects found and how they were
decided, from `utils/mrisurf_io.cpp` 5319-5385 (`MRISwriteTriangularSurface`), 5494-5636
(`mrisReadTriangleFile`), 6314-6489 (`__MRISreadQuadrangleFile`), 6491-6590
(`__MRISwriteQuadrangleFile`) and 6592-6686 (`__MRISwriteTriangularSurfaceTags`), on track C's
`readVolGeom`/`writeVolGeom` (merged at e6f8c154, with track A's CFile `rewind`).

- **The probe reaches them** through `io_statics.cpp`'s new wrappers (`mrisReadTriangleFile`,
  `__MRISreadQuadrangleFile`, `__MRISwriteQuadrangleFile`: file-local by their forward
  declarations at lines 41, 49-50); `MRISwriteTriangularSurface` is external. Command `tri`
  (fssurfio.cpp; tests/fs_surfio/tri.rs): the environment variables set per case (the creation
  time's replacement always, so the creator line is deterministic; `USER`/`LOGNAME` from the case,
  the probe's own environment has neither: `UNKNOWN`).
- **`SURF_TAG_DEBUG` is 0** (line 37): the space tags' debugging branches are compiled out; the
  readers skip tags 22-24 (`TAGskip`) and print nothing.
- **fs.io's `TAGreadStart` notes FS-49 at every end of file** (`tags::tag_read_start` calls
  `fio::fread_int`, whose short read notes FS-49 although `TAGreadStart` then returns 0 without
  using the number): every surface read would have been counted, not compared. `trifile.rs` has a
  local `tag_start` (4 bytes by `read_bytes`, 0 when the end-of-file flag is set) with the same
  results. Reported to track A (HANDOFF): the note should go where the number is used.
- **FS-346** (`len` -1): `calloc(0, 1)` gives a 24-byte usable block; the triangle reader writes
  `p[-1] = 0` (the chunk header's top size byte, 0 already) then `fread(p, 1, SIZE_MAX)`; the
  quadrangle reader reads first. More than 24 bytes left overflow the block: observed `free():
  corrupted unsorted chunks` and `malloc(): invalid next size (unsorted)` aborts (signal 6) at a
  later allocation. Rule B by default (an error); `reproduce` reads the rest (SKIP).
- **FS-347**: `if (mris->ncmds > MAX_CMDS)` tests after 1,000 lines stored, so the 1,001st goes to
  `cmdlines[1000]`, which is `ncmds` (4 bytes) and `group_avg_surface_area` (4 bytes) in the
  generated field list (`mrisurf_FACE_VERTEX_MRIS_generated.h` 522-524): `ncmds` becomes the
  pointer's low half (+1), the area its high half. Address-dependent (ASLR): both modes end with
  the message; not generated by the probe (1,001 tags).
- **FS-348**: the quadrangle reader's `calloc` result is not checked; `fread(NULL, ...)` faults when
  bytes are left, and `NULL[len] = 0` always: observed signal 11 in every such case.
- **FS-349**: `faces[fno]` is written before `cheapAssert(fno <= mris->max_faces)` (6443); with
  `nVFMultiplier` below 1 the faces past `max_faces` go past the array (observed: the assertion's
  message, no crash first, in the probe's few cases). The port stores none past the room.
- **FS-351, FS-352**: the quadrangle's vertex numbers are 3-byte (never negative) and not checked;
  `vertices_topology[v].num++` for `v` up to `max_vertices - 1` is in the array (defined: counted
  in `reproduce`), beyond it outside (observed crashes; SKIP).
- **FS-353**: the split loop `for (n = 0; n < 2; n++) { if (degenerate(va)) continue; ...; va = vb;
  }` skips the shift with `continue`. The writer's degenerate quadrangle `v0 v1 v1 v2` splits even
  into `v0 v1 v2` / `v1 v2 v1` (kept) and odd into `v0 v1 v1` / `v0 v1 v2` (lost). Rule A: the
  input is FreeSurfer's own; observed in the probe on every odd split of such a quadrangle (61 of
  1,000 cases at seed 13 noted it).
- **FS-354**: `fgets(line, 200, fp)` reads 199 bytes; the writer's line is `created by <USER> on
  <date>\n\n`, so a user name of about 165 bytes or more (or a long replacement time string) leaves
  the line's rest for `freadInt`: text bytes as counts (observed 1,970,632,053 vertices from `uuuu`),
  allocations the machine decides (the probe's box killed them: signal 9). Rule A (the default reads
  to the newline); applied also to `mrisReadTriangleFileVertexPositionsOnly` and
  `mrisReadTriangleFilePositions` (`surfread::triangle_counts`), which read the line whatever
  `TRIANGULARSURFACE_NOEXTRA_READ` says. The probe's seed-1 set makes no such line.
- **Allocations the machine decides** (as fs.io's FS-178): `calloc(len + 1)` of a command line and
  `TAGskip`'s `calloc(len)` fail on this machine above its memory, the port only beyond the largest
  object; the generator keeps odd tag lengths to the last tag (a length read from inside another
  tag's data is text), so every length is either small or beyond the largest object.
- **The space tags**: `vg_i_to_r` and `TkrVox2RASfromVolGeom` of a geometry with a zero voxel size
  are singular; `MatrixInverse` returns `NULL`, `MatrixMultiply(T, NULL)` prints its message and
  returns `NULL`, and the writer exits 1 after `[ERROR] ... couldn't obtain MatrixData` (observed
  identical, 40 cases at seed 17).

## 17. Queue item 3.1-48: the file names and the writer by name (relay 15)

What was ported (`src/names.rs`, `src/readwrite.rs`; spec part 12, §79-83), from
`utils/mrisurf_io.cpp` 2551-2616 (`MRISfileNameType`), 3276-3340 (`MRISwritePatchAscii`), 4279-4340
and 4414-4451 (`MRISwriteVertexLocations`, `MRISwrite`, `MRISwrite_new`), 5226-5317
(`MRISreadVerticesOnly`) and 6690-7000 (the four `_getfilename` functions), on track A's
`mri_identify` and `IDextensionFromName` (merged at cf5c23ba) and `TAGreadStart` without its FS-49
note at the end of a file (c117b9ea: `trifile.rs`'s local `tag_start` of §16 removed).

- **`MRISfileNameType`'s `@`** (lines 2570-2574): `FileNameOnly` already cut the name at its last `@`
  (`utils/utils.cpp` 428-431), so `strrchr(str, '@')` finds one only in a part that held two
  (`a@b@vtk` gives `b`); the comment's "forces the type of the file" never happens, and `*dot = 0`
  changes only the local copy. Rule C (kept, no ID): the readers open the name with its `@`, and no
  program of recon-all or fMRIPrep passes one.
- **`MRISreadCurvatureFile_getfilename`'s `strncmp(cp - 2, ...)`** (line 6801) reads two bytes
  before the name when its first dot is its first byte, one when it is its second (and always the
  first of them: unlike `MRISbuildFileName_read`'s test, `strncmp` starts at `cp - 2`): FS-148's
  text extended. The probe's names have two zero bytes before them (`name_tok`), so the official
  answer is "no hemisphere", as larmorx's in both modes.
- **The curvature writer's and reader's hemisphere tests differ** (lines 6715 and 6801): the writer
  takes a name as naming the hemisphere only when its first dot is its third byte (`lh.x`), the
  reader whenever the two bytes before the first dot are `lh` or `rh` (`xlh.curv`, `white.lh` not:
  first dot). Rule C (kept, no ID): only names with letters before `lh.` differ, which no program
  writes.
- **FS-355** (`MRISreadAnnotation_getfilename`, line 6934): `stricmp(fname_no_path, "lh")` compares
  the whole name, where the writer (line 6856) uses `strncmp(..., 2)`. A name such as
  `lh.aparc.annot` without a folder becomes `<folder>/../label/lh.lh.aparc.annot`, not found; the
  fallbacks then try `<that>.mgz` and the name in the program's folder, and otherwise the reader
  fails on `lh.lh.aparc.annot.annot`. Rule A: the default uses the writer's test (the first two
  letters, case matters); `reproduce` FreeSurfer's (with fs.core's `stricmp`, FS-11 for long names).
  A name of exactly `LH` (or `Lh`) is the one case where the default adds a hemisphere FreeSurfer
  does not.
- **FS-356**: `FileNameOnly` and `FileNamePath` copy into the callers' `char[STRLEN]` with an
  unchecked `strcpy` (`utils/utils.cpp` 416-418, 792; the destination is a pointer there, so
  `_FORTIFY_SOURCE` cannot check it): a part of 1,024 bytes or more writes past the buffer on the
  caller's stack. Observed: crashes (counted, not compared). larmorx takes the part whole in both
  modes (as FS-134 for the triangle-property files).
- **FS-126's checked copies**: `strcpy(fname, sname)` into a local `char[STRLEN]` (lines 6819,
  6896, 6953) and `strcat(fname, ".annot")` (lines 6908, 6978) are checked by the official build
  (`__strcpy_chk`, `__strcat_chk`): `*** buffer overflow detected ***: terminated`, signal 6, as
  FS-126's other copies; the `.mgz` fallback's `strcpy(fname, tmpstr)` (line 6964) likewise. The
  snprintf truncation messages carry the lines of the `std::cerr` statements (6720 ... 6969).
- **`MRISwrite`'s old behaviour** (`MRISwrite_old`, lines 4453-4573) sits behind `useOldBehaviour =
  false` with no way to set it: not reachable, not ported (likewise `MRISreadOverAlloc_old`).
  `chklc()` is the licence check (fs.core's `chklc`: no key needed); the probe sets
  `SURFER_SIDEDOOR` so the official build skips it.
- **GIFTI**: `MRISwrite_new`'s type 7 calls `MRISwriteGIFTI` (gifti.cpp, the surface part of queue
  item 3.1-26, this track's next piece); until it is ported the branch panics with a message. A
  binary surface name with `FS_GII=.gii` reaches it too (the name gains `.gii`), so the probe's
  `swr` command sets no `FS_GII` and writes no `.gii` name.
- **`MRISreadVerticesOnly`** never closes a quadrangle file (lines 5263-5314: no `fclose` on that
  path): a descriptor leak only; larmorx closes it. Its test `if (!mriss->nvertices)` (line 5285)
  checks the count, not the allocation: 0 vertices gives `could not allocate surface`. The old format
  (no magic number) is read as floats, as `__MRISreadQuadrangleFile` reads it (§16).

## 18. Queue item 3.1-48: `MRISread` (relay 16)

What was ported (`src/readwrite.rs`, `src/nbhd.rs`, `src/ico.rs`, `metric::average_radius`,
`coords::scanner2tkr` and `tkr2scanner`; spec part 13, §85-91): `utils/mrisurf_io.cpp`
3519-3791 (`MRISreadOverAlloc`, `MRISreadOverAlloc_new`), 4214-4277 (`MRISfastRead`, `MRISread`),
4617-4750 (`mrisReadTransform`, `MRISreadTransform`, up to its file test); fs.surf's pieces taken
early (decided 2026-11-13): `utils/mrisurf_topology.cpp` 33-176 (`shouldReportWkr`,
`mrisCheckVertexVertexTopologyWkr`), 360-410 (`mrisAddEdgeWkr`, `mrisAddEdge`), 818-875
(`MRIS_setNsizeCur`, `resizeVertexV`), 953-992 (`MRISresetNeighborhoodSize`), 1088-1488
(`MRISfindNeighborsAtVertex` with `_new`, `_newWkr` and `_old`), 1850-1883
(`mrisCountAttachedFaces`, `mrisCanAttachFaceToVertices`), 2093-2157 (`mrisAddFaceToVertex`,
`mrisAttachFaceWkr`, `setFaceAttachmentDeferred`, `mrisAttachFaceToEdges`,
`mrisAttachFaceToVertices`), 2522-2662 (`mrisCompleteTopology` with `_new` and `_old`);
`utils/mrisurf_metricProperties.cpp` 4249-4347 (`MRISsetNeighborhoodSize`,
`MRISsetNeighborhoodSizeAndDist` and their worker), 11485-11535 (`MRISaverageRadius`);
`utils/icosahedron.cpp` 1715-1845, 1866-1938 (`ICOtoMRIS`, `ICOtoScaledMRIS`, `ICOreadOverAlloc`,
`ICOread`, `read_icosahedron`); `utils/mrisutils.cpp` 2375-2418 (`MRIStkr2Scanner`,
`MRISscanner2Tkr`). `mrisCheckVertexFaceTopology` is `returnTrue()` (`include/mrisurf.h` 2199).

Waiting outside the track (placeholders that stop with a message): `LTAreadEx` and `MRIreadHeader`
(track C, fs.volume's 3.1-39 and 3.1-28) for `mrisReadTransform` when
`<surface folder>/../mri/transforms/talairach.xfm` exists; `MRIread` and `MRIScopyMRI` (track A)
when `<surface>.avg.area.mgh` is readable; `mrisReadGIFTIfile` on an existing file (this track's
gifti.cpp 213-2857). A GIFTI name that cannot be opened is ported (gifticlib's `** failed to open
GIFTI XML file '<file>'` and `mrisReadGIFTIdanum: ERROR reading gifti <file>`, both on standard
error): `MRISread` of any missing binary surface name reaches it (`__MRISapplyFSGIIread` makes the
name `<name>.gii`).

- **The old reader** (`MRISreadOverAlloc_old`, lines 3793-4212) sits behind `useOldBehaviour =
  false` (as `MRISwrite_old`, §17): not reachable, not ported.
- **An icosahedron name returns early** (lines 3559-3567): the surface `ICOreadOverAlloc` made is
  returned before the name, hemisphere, face-list rebuild, `mrisCompleteTopology`, box, distances,
  normals, Talairach transform, radius and stored positions: such a surface has an empty name,
  FreeSurfer's default hemisphere and no stored positions. Rule C (kept, no ID): FreeSurfer's
  programs read their `ic<n>.tri` files with `ICOread`, which does the same.
- **FS-359, the old format** (version 0, no magic number): `__MRISreadQuadrangleFile` keeps each
  vertex's face list from the file and adds the faces it reads to the same one-byte count (lines
  6376-6386 and 6437), and `MRISreadOverAlloc_new` rebuilds the lists only for `version < 0` (line
  3686): the place loop and `mrisCompleteTopology_old` then read `f[]` past its end (the old reader
  did the same). Rule A: by default the lists are rebuilt from the faces as for every other
  format; `reproduce` keeps the file's lists, the entries past them zeros (deterministic stand-in;
  counted, not compared).
- **FS-360**: after a reader whose one-byte face count wrapped (FS-147 reproduced), the rebuild's
  `calloc(num)` is too small and `f[num++] = fno` writes past it (lines 3686-3716). Modelled as a
  growing list (counted, not compared). By default FS-147 refuses such files first.
- **FS-351 then a segmentation fault**: a quadrangle face vertex at or above the vertex count
  (within the room) has no face list (`f` stays `NULL` past `nvertices`), so the rebuild faults;
  observed as `CRASH 11`. `reproduce` ends with exit status 139 (FS-351 noted); the default refuses
  the file in the quadrangle reader.
- **A surface without edges fails an assertion**: with no faces (or no vertex with a neighbour),
  `mrisComputeVertexDistances` makes no distance array, `dist_alloced_flags` stays 0 while
  `dist_nsize` becomes 3, and `MRISsetNeighborhoodSizeAndOptionallyDist`'s check (line 4260,
  `mris->dist_nsize == 0`) fails: observed for ASCII, quadrangle, triangle, STL and movie.byu files
  without faces. FS-03's rule (status 1 by default, the fault with `reproduce`); no new ID.
- **Ripped vertices** (the ASCII format's fourth number): the 3-hop search asserts `!v->ripflag`
  (line 1246) at the first ripped vertex in vertex order. Before it, FS-357: a vertex with a ripped
  vertex among its cached neighbours sets `nsize = ringLinks` but keeps `vnums[ring]` at the cached
  count (lines 1286-1296, 1314-1345), so the next ring scans `vlist` entries this call never wrote
  (the caller's stack array: the previous vertex's list in practice), and the cache keeps `vnum`
  while `v[]` is reallocated shorter. Rule A: by default the ring ends where it was filled;
  `reproduce` reads the caller's list as the previous vertex left it (zeros at first) and list
  entries past a reallocation as 0 (counted, not compared).
- **`mrisCompleteTopology_new`** (set by the environment variable of that name, read once): it
  attaches every face again with `mrisAttachFaceToVertices`, whose
  `cheapAssert(mrisCanAttachFaceToVertices(...))` (line 2111) fails as soon as an edge already has
  two faces: every closed surface fails; observed. A face with a repeated vertex fails line 1873
  first. Ported as written.
- **`MRISfindNeighborsAtVertex_old`** (set by the environment variable): prints `<file>:1426
  checking marks initially clear` once, checks every vertex's mark on every call (quadratic), and
  for `nlinks` beyond `mris->nsize` returns the rings without filling the cache, so
  `MRISresetNeighborhoodSize(mris, 3)` then fails `MRIS_setNsizeCur`'s assertion (line 821) on the
  first unripped vertex; observed. Its `nlinks == 3` case within the size sets `vt->vtotal` and
  returns 0 (line 1449: `vt->vtotal = vt->v3num` where `vtotal = ...` was meant), leaving marks
  set; FS-358 for its unchecked writes past the caller's 1,024 entries (not reached by the probe).
  With both variables set the two are compared (`Testing MRISfindNeighborsAtVertex`).
- **More than 1,024 vertices within 3 hops** (the probe's tree of 1,321 vertices): the new search
  prints `MRISfindNeighborsAtVertex: vno:0 has 1320 neighbours, but listCapacity is 1024`, searches
  again printing every vertex found, runs `mrisCheckVertexVertexTopologyWkr(..., true)` and fails
  `cheapAssert(false)` (line 1409); observed identical. The search's set (`tempForEachThread[0]`,
  a static per thread) is cleared only for the listed entries, so vertices found past the capacity
  stay marked into the second search: modelled (`SurfGlobals::find_nbrs_status`).
- **`MRISaverageRadius`'s operand order** (the unit oracle's object code): the box with
  `minsd`/`maxsd` (the same answers as the C comparisons, NaNs included), the centre
  `(lo + hi) * 0.5` (GCC's multiplication for `/ 2.0f`), each distance `sqrt((x·x + y·y) + z·z)`
  with `x·x` the destination, `radius += d`, then `radius / n`; 0 / 0 (the default NaN) without
  vertices.
- **The icosahedron reader** (`read_icosahedron`): FS-362 for its numbers: a vertex count or face
  count `sscanf` cannot read (the variable stays uninitialised; at the end of the file `line` still
  holds the line read before, so the face count may be a vertex line's first number), a vertex or
  face number outside the count (written outside `calloc`'s array), an unread or zero face vertex
  (`f->v[n] = -1`: `vertices_topology[-1]`). Rule B: refused by default
  (`read_icosahedron(<file>): ...`, then `ICOreadOverAlloc`'s `could not open file`); `reproduce`
  skips the write, or ends as a segmentation fault (status 139) for a face vertex beyond the room
  (counted, not compared). A negative count fails `calloc` (`read_ico: could not allocate vertex
  list`, with `errno` ENOMEM: `error: Cannot allocate memory` first; the face count's message is the
  same text). FS-361 for `ICOtoScaledMRIS`: the neighbour list is sized `vnum / 2` with `vnum`
  counted 2 per face, so a vertex with more neighbours than faces (any surface that is not closed,
  or one with a repeated face vertex) writes past it (line 1788); the one-byte `num` wraps past
  255 faces (line 1756). Rule A for the lists (they grow, in both modes, noted); a vertex of more
  than 255 faces is refused by default (`ICOread: vertex <v> is in more than 255 faces`).
- **`mrisReadTransform`**: `FileNamePath(mris_fname, fpref)` into `char fpref[300]` (line 4620):
  an unchecked `strcpy` of the whole name (`utils/utils.cpp` 792), so a surface name of 300 bytes
  or more writes past it: FS-356 extended (the name is taken whole in both modes; observed
  crashes counted, not compared). `FileExists` opens the file (a folder of that name counts).
- **`MRISread`'s statics** (`mrisCompleteTopology`'s `use_new`, `MRISfindNeighborsAtVertex`'s
  `use_new`/`use_old` and call count, `_old`'s first-call line, the search's set, `modVnum`'s and
  `mrisAddEdgeWkr`'s largest counts, the vertex-vertex check's two variables) live in
  `SurfGlobals`, so two reads in one process behave as FreeSurfer's (the probe's cases with two
  reads). `MRISsetNeighborhoodSizeAndOptionallyDist`'s `static int count, limit` never prints
  (`limit` is 0), `MRISfindNeighborsAtVertex`'s `interestingLaterTime` likewise.
- **Timeouts**: with `TRIANGULARSURFACE_NOEXTRA_READ` set, a triangle file that has its creator's
  line gives counts of billions read from its text; the official probe then runs past its 5-second
  alarm (FS-354's family, as `tri` found): `srd` does not set that variable.

## 19. Queue item 3.1-48's transform and 3.1-50 part b's first group (relay 17)

**`mrisReadTransform` (mrisurf_io.cpp 4618-4745) on track C's `LTAreadEx`.** recon-all's subjects
reach neither `MRIreadHeader` call: their `talairach.xfm`'s second line is `% avi2talxfm` (no volume
named: `mincGetVolInfo` gives two invalid geometries) and their surfaces carry a valid geometry (the
source centre is taken from it). The `fprintf` of the warning promotes the `float` centres (`%f`);
`FZERO` compares the `float` difference's absolute value (`andps`) with glibc's `FLT_EPSILON` as a
`float` (`comiss` with the object's constant 0x34000000, 2^-23: macros.h's 1e-5 is not used). `USE_AVERAGE305`'s centre literals are `double` constants stored into
`float`s. Found by `srd` seed 13: `mrisReadSTL`'s `std::ifstream` that cannot open leaves `errno`
(`ENOENT`), which a later `ErrorPrintf` prints (`No such file or directory`) first.

**3.1-50 part b, first group (`props.rs`).** Compiled operand orders (objdump of the unit oracle's
`mrisurf_metricProperties.cpp.o`; they decide which NaN's payload a result carries):
- Every `sqrt(SQR(a.x - b.x) + SQR(a.y - b.y) + SQR(a.z - b.z))` of this group (the vertex-spacing
  statistics' five cases, `MRIScomputeTotalVertexSpacingStats`, `MRISfindClosest*Vertex`,
  `FACES_commonEdgeLength_find`'s `V3_LEN`, `MRISprintTessellationStats`) is `dz² + (dx² + dy²)`,
  `sqrtss` (no call: the sum is never negative).
- `mrisCalculateFaceCentroid`: `(v1 + v2) * 0.5f`, then `(m + v0) * 0.5f` (the division by 2.0f
  folded into a product, the same value; `m` the destination of the second sum).
- `mrisComputeTangentPlanes`: `e1 = n x v` with the first product of x as `v.z n.y` (as fs.mp's
  triangle properties); `e2 = n x e1` with x as `e1.z n.y - e1.y n.z`; `V3_NORMALIZE` as
  `sqrt((y² + x²) + z²)`, then `x s`, `y s`, `s z`; `V3_LEN_IS_ZERO` as `(x² + y²) + z²` widened;
  `VectorLen` out of line (`(0 + x²) + y² + z²`); the `V3_LEN(v_e1) < 0.5` test as `z² + (x² + y²)`
  (only a `DiagBreak`).
- `MRIScomputeFaceAreaStats`: `area = (double)f->area * area_scale`; min and max as `minsd`/`maxsd`
  (the same as the comparisons, NaN keeping the old value); the vertex-spacing statistics'
  `dist *= dist_scale` with `dist_scale = 1.0f` folded away; their `min_dist` as `minsd`.
- The two variances' `sqrt` is glibc's call: a negative value (rounding) sets `errno` to `EDOM`,
  printed by a later error (`Numerical argument out of domain`, found by `mpb` seed 7).

**Defects.** FS-363: `mrisComputeOptimalPlane` (no caller in FreeSurfer 8.2.0) sums the unripped
positions into `xc`, `yc`, `zc`, then its second loop's initialiser sets them back to 0 and fills the
scatter matrix from every listed vertex, ripped ones too, and returns `xc = yc = zc = 0`.
FS-364: `MRIShistoThresholdCurvature` (mris_curvature's `-thresh`, which recon-all uses: `mris_curvature
-seed 1234 -thresh .999 -n -a 5 -w -distances 10 10 ?h.inflated`): `curv_scale = 999 / (max - min)`
is infinite when every unripped curvature is equal (or the range starts at 1000/-1000 with all
curvatures equal to the start), `curv_scale * 0` a NaN, `nint(NaN)` `INT_MIN` (FS-12): `counts[bin]++`
writes 8 GiB before the array (a segmentation fault, `CRASH 11` in the probe); likewise a NaN or
infinite curvature. Not a defect: `VERTICES_commonInFaces_find` with three common vertices writes
element 3 of its 3-vector (within it).

**Probe.** `mpb` (fssurfio.cpp `mpb_case`, tests/fs_surfio/mpb.rs): a surface read by `MRISread`;
`MRISread` leaves `orig_area` 0, so the face-area statistics' scale is 0 and every area 0 unless the
case sets it (half the cases do; the long-edge lines of `MRISprintTessellationStats` need it and the
meshes scaled by 12, one case in five).
