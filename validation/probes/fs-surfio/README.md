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
`MRISscaleBrain`; then the state), 29 commands. `build.sh` also builds
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

Measured 2026-11-13 (relay 10, 29 commands): seed 7, 58,000 cases: 57,557 identical with `reproduce`,
443 counted, not compared (440 `mprop` cases where FreeSurfer reads memory it never wrote or does not
own: FS-121, FS-123, or the fast arctangent's table outside its range, FS-06: `check.py`'s `SKIP`; 3
`metric` cases of FS-114); `fix` differs only with notices (FS-01, FS-03, FS-12, FS-111, FS-113,
FS-115..FS-120, FS-122, FS-124) and in 4 `surf` and 70 `posn` cases (`fix_libm`); the port's outputs
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

```
validation/probes/fs-surfio/build.sh <workspace>
scripts/orchestration/boxed.sh 10 python -P validation/probes/fs-surfio/check.py --seed 7 --n 56000
```
