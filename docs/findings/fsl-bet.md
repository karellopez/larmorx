# Findings: FSL's BET (`bet`, `bet2`, `betsurf`) and the libraries it relies on (FSL 6.0.7.17; FSL 6.0.7.23 at the end)

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
  **Corrected 2026-10-29 (the FSL version trial, cases 661, 1297, 1308, 80):** a real centroid
  further outside does not make FSL crash as a rule. Its fill checks each neighbour's coordinate
  against the image along that axis only (`0<=x-1`, `xsize>x+1`, ...) and reaches a point by
  `value`'s linear index `(z*RowsY + y)*ColumnsX + x` (`basicptr`, int64): a point outside the
  image is then another voxel of the volume (the start `1 -4 0` before a 3 × 6 × 4 image reaches
  voxel `1 2 0` through its neighbour one slice up, and from there every voxel: FSL 6.0.7.17 and
  6.0.7.23 fill the whole image), or memory beyond the volume, which FSL reads and writes. The
  replica had treated any start more than one voxel outside as FSL's crash (no fill: 72 of 72
  values differed), and inside the `bet` script its `reproduce` had corrected the fill instead
  (it passed the fix flag to keep the script's failure for crashes: case 1297, 2,160 of 8,316
  values). `reproduce` now does FSL's fill point by point (`fill_by_index`: FSL's stack, its
  order, its per-axis bounds, its linear index; inside the volume exactly; beyond it no write, a
  place the fill wrote read back as its value and any other read as not 1), in bet2 alone and
  inside the script, and names FSL-43 with the number of reads beyond the volume; the NaN
  centroid keeps the crash class. Cases 661 and 1297 are then FSL's bytes in both versions. Where
  FSL's reads beyond the volume met something else (case 80 shrunk: `bet2 -c 22 19 8` on a 3 × 4
  × 8 ramp; 6.0.7.17 crashes, 6.0.7.23 writes 96 values unlike the replica's) its outputs depend
  on that memory: the suite judges them undefined (`parity/fsl_bet.py`, `reads_memory`), as the
  oracle cache never caches FSL-43's runs. The default (rule A) is unchanged; it is named where
  its mask differs from FSL's fill so computed. Permanent cases `bet2-fill-by-index`,
  `bet-fill-by-index`, `bet-robust-crash-filled`, `bet2-fill-memory-beyond`.
- **FSL-63** (rule A, corrected; found by the FSL version trial, case 141, 2026-10-29): under
  `-R` the script rebuilds bet2's options as ``bet2opts=`echo $bet2opts | sed 's/-n//g'` ``
  (`bet:246`; `-B` likewise with `-s`, `bet:388`). bash's builtin `echo` takes the leading words
  made only of `-n`, `-e`, `-E` as its own options, so a leading `-e` (bet2's mesh: `bet <in>
  <out> -R -e`, where `-e` is the first bet2 option) is dropped and no `_mesh.vtk` is written
  (FSL 6.0.7.17 and 6.0.7.23 alike); after it, backslashes in the remaining options are read as
  `echo -e`'s escapes (`\c` ends the text). The default passes the options as given (the mesh
  written); `reproduce` runs them through bash's `echo` as FSL does (`bet.rs`, `echo`); both
  name FSL-63 where they differ. Permanent case `bet-robust-mesh-echo`.
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

## FSL 6.0.7.23 (the version update, 2026-10-29)

FSL 6.0.7.17 → 6.0.7.23: fsl-bet2 2111.8 → 2111.9 (`bet2.cpp`; the `bet` script, `betsurf.{h,cpp}`
unchanged), fsl-meshclass 2111.0 unchanged, fsl-avwutils 2209.3 → 2209.6 (the files `bet` runs
unchanged), fslpy 3.21.1 → 3.27.0, and the libraries of `findings/fsl-mcflirt.md`, "FSL 6.0.7.23"
(newimage 2601.0, NewNifti 5.0.0, znzlib 2511.3, utils 2412.6, armawrap 0.7.0). BET's replica
before the update, against 6.0.7.23 (`validation/investigations/version-fsl/`): 129 of 159
standard-tier cases differ, every one by the changes below.

| Change | Class | BET | The replica |
|---|---|---|---|
| `bet2.cpp`: an empty median list (`vm.empty()`) gives `tm = t98` ("Safe option here is to set the median to the robust max") | **FSL-41** fixed upstream | a binary or constant image no longer crashes there; a constant one still crashes later, in the mask's fill from the surface's NaN centre (FSL-43; `bet2_compare.sh`: exit 139 in both versions) | `tm = t98` in both modes, FSL-41 no longer named; the fix mode's refusal at FSL-43's fill now names FSL-43 (it was never reached before: FSL-41 refused first) |
| `bet2.cpp`: `--binaryMask <image>` ("Generate mesh from existing mask"), added between `--ct` and `-e` | new option | the surface is found on the mask (read first, as named, unchecked), §4.2–4.5 on it; then the input is read and `t2`, `t98`, `t` recomputed from it (`robustmin`, `robustmax`); the outputs from the input with the mask's brain mask | translated (`bet2.rs`); on the command line it needs `--binaryMask=<image>`, as every long option of utils' parser |
| `--binaryMask` with an input on another grid | defect, new (**FSL-61**, a) | the brain mask (the mask's grid) is indexed with the input's dimensions: reads and writes outside it | refused in both modes (rule B; what FSL does depends on memory: not reproduced; a documented gap where the input is smaller than the mask and FSL's reads stay inside it) |
| `--ct` with `--binaryMask` | defect, new (**FSL-62**, c) | `ctvol` is the mask (read first), so the brain image is the masked mask, and the surface is found on the converted mask | default: the brain image cut from the CT input; `reproduce`: FSL's; both name FSL-62 (rule A) |
| newimage's build string in every output's `descrip` | build metadata | `2601.0 2025-12-08T15:20:57Z` (was `2203.12-dirty 2024-02-01T16:17:47+00:00`) | `volume::BUILDSTRING` |
| utils 2412.6's usage ID | message | `Part of FSL (ID: 2412.6-dirty)` (was `Part of FSL (ID: "")`) in bet2's and betsurf's usage | `options.rs` |
| `FSLOUTPUTTYPE` (newimage 2601.0) | new values, message | the zstd and bzip2 types; the unknown value's list of 16 | as for mcflirt |
| fslpy 3.27.0 (`imtest`, `remove_ext`, `imglob`) | new extensions | `.nii.zst`, `.nii.bz2`, `.img.zst`, `.hdr.zst`, `.img.bz2`, `.hdr.bz2` in their lists (imtest and remove_ext keep MINC; imglob's own list has none); the pairs grouped alike | `avwutils.rs` (`FSLPY_EXTS`, `IMGLOB_EXTS`, `GROUPS`) |
| NewNifti 5.0.0's `replaceSuffix` (FSL-60), znzlib's formats | as for mcflirt | bet2's own name handling (FSL-40) aborts on a name holding `.img` before NewNifti is reached; betsurf and the script's tools meet FSL-60 | as for mcflirt |
| Armadillo 14.2.1's 4 × 4 inverses | as for mcflirt | BET inverts the sampling and swap matrices of `makeradiological` (a diagonal matrix's reciprocals; the swap matrix, upper triangular, by `dtrtri`), takes `left_right_order`'s determinants (a triangular sform's diagonal product, else LU), and solves `niftivox2newimagevox_mat` (`sampling_mat().i() * swapmat * sampling_mat()`, an image stored neurologically, by `dgesv`) in bet2's `-c` and in `fslstats -C` (bet's `-R`); no recorded output changed, but random testing found both (below) | **followed since 2026-10-29** (`lapack.rs`, `arma.rs`: `findings/fsl-mcflirt.md`, "Armadillo 14.2.1's LAPACK path"); both trial cases agree with FSL 6.0.7.23 |

### The version trial's BET differences, triaged (2026-10-29)

The trial (`oracles/difftest/2026-10-29-version-fsl-2`, against FSL 6.0.7.23) left six BET
replica differences; each replayed against both versions (`replay_bet_findings.sh`; against
6.0.7.17 the headers' build string always differs, the replica writing 6.0.7.23's):
- **51** (`bet2 -n`, NIFTI_PAIR; FSL crashes in FSL-43's NaN fill): the replica's filled run wrote
  nothing, which is what `-n` alone asks; the suite required an output. Harness: the check now
  asks for outputs only where the call does (`asks_outputs`). Permanent case
  `bet2-n-crash-nothing-asked`.
- **141** (`bet -R -e`): FSL-63, above; the replica now agrees with FSL 6.0.7.23 on the drawn case.
  Its shrunk form (a 1 × 1 × 2 image stored neurologically) meets one more change: FSL 6.0.7.23's
  `fslstats -C` prints the centre `-0.000000` where 6.0.7.17 prints `0.000000`, which `dc`
  then reads as a subtraction (`dc: stack empty`, twice): `fslstats.cc` and newimage's `cog` are
  unchanged, the centre goes through `niftivox2newimagevox_mat().i()` (Armadillo 14.2.1's
  inverse), and an image stored radiologically gives `0.000000` in both
  (`cog_sign_probe.py`). **Since 2026-10-29** the replica's `fslstats -C` solves as FSL
  6.0.7.23 does (`dgesv` of the voxel matrix, whose first pivot is −1: 0 / −1 is −0)
  and its `dc` reads a leading `-` as the subtraction GNU dc 1.4.1 makes of it (`bet.rs`,
  `dc::lexemes`): the shrunk case agrees (permanent case `bet-robust-cog-minus-zero`).
- **661** and **1297**: FSL-43's fill by the linear index, above (replica mistakes, fixed).
- **1308** (`bet -R -g 0.5 -e`): FSL's second bet2 crashes in a fill starting 71 slices above the
  image; the script's `ERR` trap prints `bet failed during command:...` after the crash, which
  the suite had counted as printed before it (harness, fixed: `_BET_FAILED`); `reproduce` goes
  on with FSL-43's fill (judged as a filled run).
- **80**: shrunk (`bet2 -c 22 19 8` on a 3 × 4 × 8 ramp), FSL-43's reads beyond the volume
  (undefined, above). Drawn (a 30 × 32 × 16 image of 1.2 × 0.8 × 1.2 mm stored neurologically,
  `-c 22 19 8 -m -w 0.5`): 18 of 15,360 voxels differ between FSL 6.0.7.17 (the replica's) and
  6.0.7.23; the versions agree without `-c`, and with `-c` on the same image stored
  radiologically (`bet2_versions.sh`): `-c`'s centre goes through
  `niftivox2newimagevox_mat()`'s inverse, whose last bits Armadillo 14.2.1 changes. **Since
  2026-10-29** the replica follows that solve: the drawn case agrees with FSL 6.0.7.23 (permanent
  case `bet2-centre-neurological-solve`).

## The clean-room original's open questions, answered from the source (spec writer, 2026-10-30)

Source-level notes behind the answers in `specs/bet.md` (lines marked "(spec writer,
2026-10-30)"); the behaviour is there, this section names where it comes from. Probes:
`validation/investigations/bet-spec-answers/` (Apache) and, for the copies of the replica, the
patch script in `<workspace>/archive/2026-10-30-bet-spec-answers/` (FSL-licensed snippets).

- **Reruns** (`bet2.cpp`, the `while (self_intersection)` loop): `m = moriginal` (`Mesh::operator=`,
  which rebuilds the triangles from `moriginal`'s list, so the neighbour lists are rebuilt by the
  `Triangle` constructor's remove-and-append); `l = 0`; `incfactor = pow(10, pass + 1)`, lowered for
  `i > .75 * nb_iter`; `step_of_computation(..., pass, incfactor, ...)` changes only
  `f2 *= increase_smoothing; f2 = Min(f2, 1.)` where `tmp = dv|n > 0`. A copy's neighbour and
  triangle lists equal the first pass's for all 2562 vertices (scratch probe on
  `Mesh::icosa(5)` and `copy()`), so the rerun is the spec's rule. The clean-room original's
  misses come from its arithmetic (ten choices, each enough to move `older-75y`'s first rerun:
  the specs' §4.5 and §8.1 list them with the measured totals); `step_of_computation`'s exact
  expressions are what §8.1 transcribes into maths (`rinv` from `sn|n`, `u = u1 + u2 + u3`,
  `local_normal`'s `normalize()` dividing, `medium_neighbours` dividing, `Triangle::normal` as
  `(v2 − v0) * (v1 − v0)`, the centre's single running sum in `adjust_initial_mesh`, `rmax = 10 *
  smootharg.value()` in float, the slab radius's `counter*image.ydim()*image.zdim()` in float,
  `image.xsize()*image.xdim()` in float, `gi += dscale`).
- **`real_self_intersection`** (`mesh.cpp`): the sweep over `Pt_special` sorted by `compPt` (x),
  `data[0]` counting each triangle's visits (tested and appended at 0, removed at 2), the
  coordinate test `Pt::operator==` (1e-8), the prefilter made moot by `centre *= 1/3`, and
  `Triangle::intersect` (`triangle.cpp`): the early exit on `B0`'s side, the two half-tests, the
  `n1|n3`, `n2|n3`, `n1|n2` signs. §4.5 of the spec states it as a predicate.
- **`find_skull`** (`bet2.cpp`): `volmesh(ci, cj, ck) == 0` reads 0 outside the volume (zero
  padding), the candidate ranges unclipped, three separately accumulated grids (`d` from 0, from
  `skull_start`, from `d_min + scale`), the in-volume tests on the voxel coordinates, the scan's
  `else d = d_max`. §4.7 of the spec.
- **`find_thresholds`** (`newimage.cc`): `fA = (max-min)/(double)HISTOGRAM_BINS`,
  `thresh2 = min + (T)((double)bottom_bin*fA)`; the narrowing's `(T)(min + ((double)bottom_bin/
  (double)HISTOGRAM_BINS)*(max-min))`; `imageHistogram`'s `fA`, `fB`. §4.2 of the spec.
- **The quaternion** (`legacyFunctions.cc`, `NewNifti.cc`): `getQForm()` passes the header's
  `qB, qC, qD` (doubles) to `nifti_quatern_to_mat44(float ...)`; `mat44` and `mat33` hold floats;
  `setQForm` keeps `nifti_mat44_to_quatern`'s doubles, which NIfTI-2 writes unrounded; the
  long double literals (`1.0l`, `0.5l`, `0.25l`, `2.0l`, `1.e-7l`) make those expressions x87
  extended. The same code as the NIfTI-1 reference library's (public domain) apart from the
  outputs' type. §8.2 of the spec, checked 18 of 18 (`quat_emul.py`).
- **Name resolution** (`generalio.cc`, `return_validimagefilename`): a name with a known extension
  whose file is missing falls back to the extension search; §3.1 of the spec (observed too).
