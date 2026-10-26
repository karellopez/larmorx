# Findings: FSL 6.0.7.17's BET (`bet`, `bet2`, `betsurf`) and the libraries it relies on

**Restricted** (CLAUDE.md rule 1): written from FSL's source while translating the replica
(`crates-nc/larmorx-nc-bet`, with FSL's libraries in `crates-nc/larmorx-nc-core` since BET's
merge, 2026-10-25). Whoever writes the clean-room original of BET must not read this
file; the behaviour it needs is in `specs/bet.md`. References are `file:line` in the FSL 6.0.7.17
conda packages' source (`~/fsl/src/<package>/`, copied unmodified to `crates-nc/fsl-source/`):
fsl-bet2 2111.8, fsl-meshclass 2111.0, fsl-newimage 2203.12, fsl-newnifti 4.1.0, fsl-miscmaths
2203.2, fsl-utils 2203.5, fsl-znzlib 2111.0, fsl-avwutils 2209.3, fslpy 3.21.1.

## The oracle

- FSL 6.0.7.17 from FSL's conda installer (`~/fsl`), built by conda-forge's GCC 13.2.0 with
  conda-forge's flags for x86-64 (`-march=nocona`: SSE3, no FMA, no AVX), so every `double` and
  `float` operation of FSL's own code is one IEEE operation in source order: the machine code of
  `bet2`, `betsurf`, `libfsl-newimage.so`, `libfsl-meshclass.so` and `libfsl-NewNifti.so` holds no
  `vfmadd`, and x87 instructions only where the source asks for `long double` (108 in
  NewNifti's legacy quaternion routines, one in each program). The source is the
  specification (rule 8) and the binary agrees with it: no code-generation departure was found,
  so no source-built oracle is needed (unlike ANTs, `docs/findings/ants-float.md`).
- The C library is the system's, glibc 2.35 (Ubuntu 22.04). The functions FSL's code calls that
  shape BET's results are not correctly rounded:
  - `pow` (`bet2.cpp:351`, the radius `pow(x, 1/3)` and `pow(x, 0.5)`; `bet2.cpp:933`,
    `pow(f, 0.275)`; `bet2.cpp:1000`, `pow(10, pass + 1)`) is `__pow_fma` (glibc picks it by ifunc
    on a CPU with FMA and AVX2): about 0.09 % of BET's calls differ from the correctly rounded
    value in the last bit;
  - `exp` (`newimagefns.cc:701`, the Gaussian kernel of `smooth`) is `__exp_fma`;
  - `tanh` (`bet2.cpp:483`, `betsurf.cpp:286`, 2.5 million calls per bet2 run) is the generic
    fdlibm `__tanh` (glibc has no CPU variant of it);
  - NewNifti's quaternion routines (`legacyFunctions.cc`, `mat44_to_quatern`,
    `quatern_to_mat44`, `mat33_polar`) are written in `long double`: on x86-64, x87 extended
    precision, every intermediate rounded to 64 bits and then stored.
  `crates-nc/larmorx-nc-core/tools/libm-verify` compares the replica's ports with this machine's
  glibc and x87 unit: 0 of 4 million `pow` calls (BET's arguments and the whole range), 0 of 4
  million `exp`, 0 of 3 million `tanh` and 0 of 5 million x87 operations differ (2026-10-24).
- FSL is deterministic: single-threaded, no random numbers; the parity record's runs of FSL again
  are bit-identical.
- fMRIPrep 25.2.6's image carries the same fsl-bet2 2111.8 and fsl-meshclass 2111.0 with newer
  libraries (fsl-newimage 2501.4, fsl-miscmaths 2412.4, fsl-utils 2412.5, fslpy 3.24.0, GCC 15.1's
  libstdc++), on the same glibc 2.35: the record compares its outputs with the oracle's.

## The defects (D13), with their rule

Each is in the register (`docs/findings/upstream-defects.md`); the switch is
`LARMORX_UPSTREAM_DEFECTS` / `upstream_defects` (`larmorx_core::defects`).

- **FSL-40** (rule A, corrected): `bet2.cpp:871-875` removes `.hdr` with
  `name.erase(name.find(".hdr"), 4)` wherever it stands, and for a name containing `.img` erases at
  `find(".hdr")` again (a typo for `.img`): a directory `x.hdr/` is read as `x/`; a name with `.img`
  and no `.hdr` throws `std::out_of_range` (`basic_string::erase`), uncaught: FSL aborts. The
  default removes one trailing extension; `reproduce` erases as FSL does and, where FSL aborts,
  refuses with FSL's message.
- **FSL-41** (crash; refused by default, filled in `reproduce`): `bet2.cpp:427-438`: `tm` is
  `*max_element(vm.begin(), vm.begin() + med)` after `nth_element(..., vm.begin() + med - 1, ...)`.
  With no voxel strictly between `t2` and `t98` inside the initial sphere (a constant image, a
  binary image), `vm` is empty and `max_element` of an empty range is dereferenced: a null read,
  FSL segfaults (`nth_element` returns at once on the empty range). With one voxel `med` is 0:
  `nth_element` with `nth` one before the range only runs its insertion sort on the single
  element, and `max_element` of the empty range `[begin, begin)` returns `vm.begin()`:
  `tm = vm[0]`, defined; the replica reproduces that. The default refuses the empty case (rule B: a constant or
  binary image has no brain intensities); `reproduce` keeps every value FSL defines and takes
  `tm = 0` for the one it dereferences from nowhere (D13's failure cases: the defined parts
  reproduced, the rest filled deterministically, with a warning naming FSL-41). On the parity case
  `bet2/constant` the surface then collapses and its centroid is not a number, where FSL-43's fill
  is skipped in `reproduce` (below): the run writes its outputs, the same bytes every time.
- **FSL-42** (rule B, refused by default): bet2 checks only `f < 0 || f > 1` and `g < -1 || g > 1`
  (`bet2.cpp:845-856`): NaN passes; `-r` negative, NaN or infinite, `-w` of 0 or less, a
  non-finite `-c` and `--parameter_d1/d2` are used as given (the deformation then computes with
  NaN or divides by 0). The default refuses, naming the value; `reproduce` computes with it.
- **FSL-43** (rule A, corrected): `make_mask_from_mesh` (`bet2.cpp:262-290`,
  `betsurf.cpp:177-215`) writes `mask.value(x, y, z)` at the truncated centroid without a bounds
  check (`value` has none) and then reads its neighbours the same way: a centroid outside the
  image writes before (or after) the volume's buffer and reads there. It happens on real inputs: on
  a single slice the deformed surface's centroid lies below it (the parity case `defects/2d`:
  voxel `47 55 -1`), and FSL survives the write (it lands in the heap just before the buffer) and
  fills the slice from the start's one neighbour inside the image. The default starts the fill at
  the nearest voxel inside the image; `reproduce` does what FSL defines (no write outside; the
  start's neighbours inside the image are examined, those outside count as not to be filled) and
  warns that FSL writes and reads outside the volume there. On `defects/2d` both give FSL's
  outputs, bit for bit. Further than one voxel outside, FSL's write lands far from the buffer and
  it crashes (a NaN centroid, as FSL-42's NaN options give, truncates to `INT_MIN`: the parity
  cases `defects/nan-f` and `defects/zero-w` segfault in FSL): the default fills from the nearest
  voxel when the centroid is a real position and refuses otherwise; `reproduce` does not do the
  write FSL crashes on and fills nothing (the mask holds the drawn surface only), and warns (D13's
  failure cases, since BET's merge).
- **FSL-44** (rule A, corrected): the `bet` script expands `$1`, `$2`, `$IN`, `$OUT` unquoted
  throughout (`bet:155-160` and every command): a name with a blank becomes several words (the
  input is "not valid"; an output name writes other files), and `while [ _$1 != _ ]`
  (`bet:177`) ends the options at an empty argument. The default treats each argument as one
  word; `reproduce` splits as bash does.
- **FSL-45** (rule A, corrected): `-Z` crops its outputs with `for f in \`imglob ${OUT}*\``
  (`bet:329`), `imglob` called without `${FSLDIR}/bin`: with FSL off the PATH (fMRIPrep's
  image puts it on, a clean shell does not) bash prints `line 329: imglob: command not found`, the
  loop runs on nothing and the outputs stay padded; with it on the PATH every image whose name
  starts with `${OUT}` is cropped, other images included. The default crops exactly its own
  outputs; `reproduce` looks for `imglob` on the PATH and does what FSL's script would (the
  replica reimplements fslpy's `imglob` itself).
- **FSL-46** (rule A, corrected): `/bin/rm -f ${OUT}_tmp*` (`bet:405`) removes files the run did
  not create. The default removes only its own temporary files.
- **FSL-47** (rule A, corrected): `t1only_main` fills the scalp mask with
  `make_mask_from_mesh(output1, m)` (`betsurf.cpp:948`): `m` is the outer skull mesh, still in
  the padded working grid, not the scalp mesh `m2` (the T1+T2 `main` uses `m2`,
  `betsurf.cpp:1737`). The fill starts at `m`'s centroid shifted by the padding: inside the scalp
  almost always, so the mask rarely changes; the default fills from `m2` and names FSL-47 only
  when the masks differ.
- **FSL-48** (rule B, refused by default): betsurf samples the T2 at the T1's voxel coordinates
  (`co_ext`, `special_extract`) without checking its grid. The default refuses a T2 whose
  dimensions or voxel sizes differ from the T1's.
- **FSL-49** (rule C, kept): `co_ext` (`betsurf.cpp:1075-1081`): when the T2's scalp lies beyond
  the T1's by more than 5 mm, FSL validates the T1's scalp against a T2 search and keeps it
  (`else outskin = outskin;`, commented "very suspicious" in FSL's own source). The symmetric
  branch replaces the T1's scalp by a re-search; this one keeps the nearer estimate after
  validating it, which is sensible. Kept and documented: possibly deliberate, no evidence that
  another choice is better.
- **FSL-50** (rule A, corrected): `co_ext` calls `exit(-1)` (`betsurf.cpp:1138`) when
  `minabs == -500`: one profile whose search range is empty (the outer skull search starting
  beyond the scalp) ends the program with status 255 and no message, before any output is
  written. The line sets `output = false` first: the profile is to be dropped. The default drops
  it (only its scalp estimate counts) and names FSL-50; `reproduce` exits 255.
- **FSL-51** (garbage, corrected): `Profile` (`profile.cpp`) dereferences `v.end()` in its forward
  searches (`set_lroi`, `next_point_over`, `next_point_under`, `value`), and `min`, `max`,
  `begin` read `v[lroi]` with `lroi == v.size()` when a region of interest is empty at the end of
  the profile. The vector's capacity is larger than its size (powers of two), so these reads are of
  uninitialised heap memory, not a crash. Where the read cannot change the result (`set_lroi`'s
  loop: the counter is the same whatever is read) the replica does nothing special; where it can
  (`next_point_*` past the end with the region ending earlier; an empty region's `min`, `max` or
  `begin`), the replica reads "nothing there" (+∞, NaN) and marks the profile: the default drops
  it, `reproduce` keeps the result of that reading and warns that FSL's depends on memory
  contents.
- **FSL-52** (crash; refused by default, filled in `reproduce`): `Mesh::load_*`
  (`mesh.cpp:423-612`) reads with `>>` and
  never checks the stream: a number that does not read leaves the variable at 0 (C++11) and every
  later one never set (in practice the previous iteration's stack values); a face naming a vertex
  index outside the mesh reads `_points[n]` out of range (a wild pointer). An empty mesh (`c`, or a
  file of 0 vertices) divides by 0 for the centroid and FSL-43 follows. FSL 6.0.7.17 segfaults
  on each of the parity cases (`defects/mesh-unread`, `mesh-missing-vertex`, `mesh-truncated`;
  after writing `out_inskull_mesh.vtk` from the never-set values in the first and the last). The
  default refuses with the reason; `reproduce` keeps the values C++ stores (0 for a number that
  does not read), takes 0 for every value the stream never sets, leaves out the faces that name
  a missing vertex, lists what it filled in the FSL-52 warning, and goes on (the centroid of a
  surface of zeros is not a number: FSL-43's skipped fill). FSL's own VTK files carry no point
  data: the trailing reads fail and leave each
  point's value never set, but betsurf never uses values (`Mesh::operator=` does not copy them,
  `stream_mesh` type 3 does not write them), so the replica ignores that section.
- **FSL-53** (rule B, refused by default): betsurf reads the 16 numbers of the matrix with
  `f >> M.m11 >> ...` (`betsurf.cpp:593`, `1381`) and no check; `trMatrix M` is not initialised.
  The default refuses a file with fewer than 12 readable numbers (`special_case` uses 12);
  `reproduce` uses 0 for the first unread one (as C++11's `num_get` stores) and for the rest
  (never set in FSL) and warns.
- **FSL-54** (rule A, corrected): `main` calls `t1only_main(...)` and then `exit(0)`
  (`betsurf.cpp:1347-1350`), dropping its return value: `--t1only` exits 0 after "too few
  arguments" or "unable to open". The default exits with the failure's status (255). A failure
  that ends the program inside FSL's libraries is not dropped: an unknown `FSLOUTPUTTYPE` makes
  `FslGetEnvOutputType` call `exit(1)` itself, as the replica does in both modes (random testing,
  2026-10-26).
- **FSL-55** (rule B, refused by default): betsurf's `nth_element` over the T1's values in the
  brain (`betsurf.cpp:629`, `1420`) has no strict weak order when a value is NaN: libstdc++'s
  unguarded partitions can then run outside the vector. The default refuses; `reproduce` runs
  libstdc++'s algorithm (translated) with its loops stopped at the vector's ends and warns.
- **FSL-58** (class a, found by random testing 2026-10-26): with no voxel of the T1 inside the
  brain surface (a surface outside the image, a tiny image), `brain_hist` is empty
  (`betsurf.cpp:621-630`, and `1411-1421` in `t1only_main`): `e98` is 0, `nth_element` gets
  `begin() - 1` and `thr = brain_hist[-1]` reads before the vector's start: FSL crashes (SIGSEGV on
  a one-voxel constant image) or uses memory it never wrote. The default refuses, naming the
  entry; `reproduce` takes that percentile as 0 (D13's failure case) and goes on.
- **FSL-56** (rule A, corrected): NewNifti's `vox_offset` (`NewNifti.cc:890`) adds
  `extensions[0].extensionSize()` once per extension instead of each extension's size: with two
  or more extensions of different sizes (kept with `FSL_LOAD_NIFTI_EXTENSIONS`) the header points
  the data into the extensions or past a gap, so readers read the wrong bytes. The default writes
  the correct offset.
- **FSL-57** (rule C, kept): `standard_step_of_computation` sets `f2 = 0` where a mask applies
  (`betsurf.cpp:314`, `330`) after `u2 = f2 * sn * addsmooth` was computed: a dead store. The
  intent was probably to switch the smoothness term off inside the previous surface; doing so
  would change every skull and scalp surface. Kept: the method as published and validated runs
  with it, and there is no reference to measure a change against.

## Other facts the replica follows

- **newimage's out-of-bounds writes** (`newimage.h:485`): a non-const `operator()(x, y, z)`
  outside the volume returns a reference to the volume's `extrapval` member, which zero padding
  resets to 0 at every read: betsurf's and bet2's drawing and counting outside the grid write
  nowhere that matters.
- **The flood fills** push a voxel each time it is seen with the fill value and set it when it
  is popped; marking when pushed fills the same set.
- **`real_self_intersection`'s prefilter** (`mesh.cpp:1080`): `centre *= 1/3` is an integer
  division (0), so every triangle's "centre" is the origin and the bounding-sphere test passes
  for every pair; the exact triangle test decides. The result is the same as with a correct
  prefilter (it only prunes); FSL is just slower (bet2 `-e`).
- **The meshes' list orders** (`triangle.cpp:75-96`, `mesh.cpp:85-130`, `823-897`): a triangle's
  constructor removes and re-appends its vertices in each other's neighbour lists; the copy
  constructor copies values and not `data`, `operator=` copies `data` and not values; retessellate
  numbers new vertices `nvertices() + count` and deletes the old triangles last. The sums over a
  vertex's triangles and neighbours follow these orders.
- **`real_self_intersection` sorts the vertices by x** with `std::sort` (libstdc++'s introsort):
  equal keys end up in the order that algorithm decides, which the sweep depends on; the replica
  translates the algorithm (`larmorx-nc-core/src/stdsort.rs`).
- **betsurf's CSF image** (`co_ext`, `special_extract`, `csfvolume`) is computed and never
  written: the replica leaves it out (its `Profile::value` calls cannot exit: every position asked
  for is a sample's).
- **betsurf's working-grid masks** start from `copyconvert(write0, ...)` (`betsurf.cpp:740`, `827`,
  `1531`, `1618`): the smoothed inner-skull counting image truncated to short, so values of 1 or
  more stay in the mask as walls and as mask values above 1 (the outer skull's mask is built from
  `write0` too, not `write1`). Rarely any voxel reaches 1 after the 3 mm smoothing. Translated as
  is.
- **bet2's one-argument help** (`bet2.cpp:811-818`): with exactly one argument, any argument that
  contains `-h` anywhere (`my-head.nii`) prints the usage and exits 0. Translated.
- **The `bet` script's `$0`** is the script's absolute path in FSL's messages
  (`<FSLDIR>/bin/bet failed during command:...`); the replica prints `bet`. The parity record
  reads FSL's path as `bet`.
- **`fslstats -C`** (`fslstats.cc`, `calc_cog`) sums in chunks of `nlim` voxels and converts the
  centre from newimage's voxels to NIfTI's with `niftivox2newimagevox_mat().i()` (a 4 x 4 solve):
  translated, so `bet -R`'s centres are FSL's to the printed digit.
- **`dc`** (bet's arithmetic) truncates every division at its scale register (0) and keeps the
  operands' decimals in sums and products: the replica's `dc` does the same for the expressions
  the script uses.

## Random testing (BET's merge, 2026-10-26)

BET's descriptor (`validation/src/larmorx_validation/difftest/tools/fsl_bet.py`) draws bet2's
options, bet's variations and betsurf's (with a closed surface it makes) on heads of up to 32
voxels per axis of every stored type, through BET's own parity runner. Four trial runs (seeds
`bettrial1` to `bettrial4`; 34, 324, an interrupted one and 201 cases) found eight kinds of
difference (two of them in the suite's own comparison), each fixed, seven kept as permanent cases
(`validation/difftest/cases/fsl-bet/`, `triage.toml`):

- `fslstats -C` prints newimage's `WARNING::in calculating COG, total = 0.0` itself
  (`newimage.cc:1586`, `cerr`): the replica had dropped it (an empty brain under `bet -R`).
- Inside `bet`, where FSL's bet2 crashes (FSL-41), the script's ERR trap prints its failure and
  exits 1: FSL's defined outcome, which `reproduce` now gives (it had filled and gone on).
- (The suite's own) a filled run's messages: FSL's buffered standard output is lost at its crash
  (`-v` on a constant image), so the suite compares what FSL printed as the start of the
  replica's.
- **FSL-58**: betsurf with no voxel of the T1 inside the brain surface (above); the replica had
  panicked.
- `fslmaths -mas` (bet `-F`) computes in double and writes double for a double input
  (`fslmaths.cc:1414-1440`, newimage's `dtype` of the file); the replica wrote float.
- FSL-43's correction was named where it changed nothing: now compared with FSL's own fill (a
  start one voxel outside) or with no fill (further out, where FSL crashes), and named only where
  an output shows the mask (not under `-n` without `-m`).
- betsurf `--t1only` with an unknown `FSLOUTPUTTYPE`: `FslGetEnvOutputType` calls `exit(1)`
  itself, so FSL-54's dropped status does not apply.
- (The suite's own) a filled run that then meets a failure FSL never reaches is judged as filled.

## NaN and infinite voxels (BET's merge, 2026-10-25)

FSL's Linux build computes with SSE: a NaN made from numbers (`0 * inf`, `inf - inf`, `0 / 0`)
has the sign set (printed `-nan` in a VTK mesh), AArch64's has it clear, and when two NaNs meet
the operand order decides which one passes on (porting guide, "NaN bits are part of the output").
BET's NaN matrix (`validation/probes/fsl-bet-nan`) runs a head with NaN voxels (quiet, negative,
with a payload), infinite voxels and NaN voxels in the 2-voxel border through bet2, `bet -R` and
`betsurf --t1only`, in both modes, by FSL, a release and an unoptimised debug build, and the
release build with AArch64's default NaN emulated (`libnan_trap.so`).

- **Found on its first run:** `brain-payload-neginf` (a NaN with a payload and a -inf voxel),
  `bet2 -m -s -o -e` in `reproduce`: the mesh printed `-nan` on x86-64 and `nan` with ARM's NaN.
  The -inf voxel made the robust range's `max - min` infinite, `0 * inf` in the histogram's bin
  position (`newimage`'s `find_thresholds`), and a NaN centre of gravity (`0 / 0` in bet2's
  initial mesh) then reached every vertex. FSL itself crashes in its fill there (FSL-43's crash
  class), so `reproduce` writes what FSL defines and the fill is skipped.
- **Fixed** by computing, in the source's order, with the core's SSE rules (`sse::S32`, `S64`):
  `find_thresholds`, meshclass's point and vector arithmetic, bet2's initial mesh and
  deformation step, betsurf's deformation step. The profile searches compare only (a NaN
  threshold compares false on every platform) and need nothing.
- **Result** (the final run, 2026-10-25): 60 cases, 0 differ: every output identical
  across the release build, the debug build and the release build with ARM's NaN, and identical
  to FSL's wherever FSL finishes (FSL crashed in 18; those are compared among the
  replica's builds). BET's golden files and every parity case were unchanged by the fix.

## Pending (not replicated yet)

`bet -S` (standard_space_roi, FLIRT, fslmaths' morphology, fslstats), `-B` (standard_space_roi,
FAST), `-A` and `-A2` (FLIRT, then betsurf): the replica refuses them with exit 1, naming the
missing FSL program. They become possible as FLIRT and FAST are replicated (docs/next-steps.md,
the FSL item). `robustfov` (in fsl-bet2) is not translated either.
