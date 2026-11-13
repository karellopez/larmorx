# FreeSurfer 8.2.0's surface structure and its I/O: behaviour spec (`fs.surfio`)

Written by the replica author of queue items 3.1-45 to 3.1-52 as their spec writer
(`docs/porting-guide.md` §1): behaviour, conventions, messages and the validation plan, from
FreeSurfer 8.2.0's source and the official code's output, never code or routines. A clean-room
implementer writes the clean room from this file, black-box runs of the official programs and the
cited standards only.

Status: **part 1** (queue items 3.1-45 and the record of 3.1-46): what a surface holds, the
coordinate sets and their selector, the kinds of surface, the neighbour counts, the small geometric
helpers every surface computation shares; **part 2** (the rest of 3.1-46 that needs no volume,
§8-13): command lines, the neighbourhood checksum, the simple surface comparison, borrowed marks,
per-vertex scratch arrays and the integrator's file names; **part 3** (queue item 3.1-49's
first part, §14-16): the neighbour distances' storage and the coordinate operations; **part 4**
(queue item 3.1-49's parts b and c, §18-24): averaging and smoothing positions, centring a sphere,
computing and checking the neighbour distances, the areas, triangle corners and coordinate sets,
the curvature operations and saved coordinates; **part 5** (queue item 3.1-50's part a,
§25-29): the saved vertex positions and normals, the canonical angles, the face-normal cache, the
original area and storing and restoring the metric properties; **part 6** (queue item 3.1-47's
part a, §30-37): a surface's room, counts and life, the metadata copy, the dumps, rip flags and
arrays, the curvature names and registration scales, the extra gradients and NaN check, the column
printing and progress bar, the vector searches and sort comparisons, whether two edges cross;
**part 7** (queue items 3.1-51 and 3.1-52, §38-46): the metric properties every read computes;
**part 8** (queue item 3.1-47's part b, §47-55): the small file readers and writers (the read
frame, the triangle-property files, the distance, area-error, angle-error and curvature files, the
backup and imaginary value readers, the marks, the ASCII and VTK formats); **part 9** (queue item
3.1-48's part a, §56-65): one `scanf` call, the curvature, float, area and decimation files, file
names and `FS_GII`, the VTK reader, the movie.byu and icosahedron writers, the Talairach matrix,
the MATLAB plots; **part 10** (queue items 3.1-48's part b and 3.1-49's STL, §66-72): the raw ASCII,
movie.byu and triangle-file readers, the quadrangle of two triangles, STL read and written; **part
11** (queue item 3.1-48, §73-78): the binary triangle and quadrangle surface files, read and written,
with their tags. Later parts: reading and writing surfaces through their format's identification, curvatures, patches,
overlays and value files (3.1-47's part c, 3.1-48's rest); the midline marking, the label normal and
the coordinate copy (they need the volume structure).

## 1. Conventions

As `specs/freesurfer-maths.md` §1 (the official build's arithmetic: x86-64, SSE2, no fused
operations, each operation rounded to its type; conversions to integers; the C library's functions)
and `specs/freesurfer-core.md` §2-3 (messages, fatal errors with their exit statuses, failed
assertions: `ASSERTION FAILED: <file>:<line> <test>` on both streams, then ` break
<BUILD_ROOT>/utils/diag.cpp:85` on standard output and a segmentation fault, FS-03: default exit
status 1, `reproduce` 139). `<BUILD_ROOT>` is
`/home/rd521/space_freesurfer/build/docker_ubuntu22/trunk/rd521-8.2.0`.

- **NaN bits.** Where two NaNs meet in one operation, which one the result carries is decided by
  the official build's instruction order, which for the helpers of §6 differs from one place of use
  to another: it is specified with the computations that use them. §6 gives the formulas' order,
  which holds wherever one operand only is a NaN; a NaN made from numbers is the default NaN with
  the sign bit set (`0xffc00000`, `0xfff8000000000000`).

## 2. The surface record

A surface has `nvertices` vertices and `nfaces` triangles, both `int`, numbered from 0. A new
surface starts with every number 0, every list absent and every name empty.

**A vertex** holds (all `float` but where said):

| Group | Quantities |
|---|---|
| coordinate sets (§3) | current `x y z`; original `origx origy origz`; canonical (spherical) `cx cy cz`; temporary `tx ty tz`; second temporary `t2x t2y t2z`; target `targx targy targz`; pial `pialx pialy pialz`; white `whitex whitey whitez`; layer IV `l4x l4y l4z`; inflated `infx infy infz`; flattened `fx fy fz` |
| normals | current `nx ny nz`; pial `pnx pny pnz`; white `wnx wny wnz`; original `onx ony onz` |
| motion | the current step `dx dy dz`, the previous one `odx ody odz` (momentum), a temporary gradient `tdx tdy tdz` |
| exact coordinates | `px qx py qy pz qz` (`int`: rational coordinates) |
| tangent frames | `e1x..z`, `e2x..z`, `pe1x..z`, `pe2x..z` |
| values | `curv`, `curvbak`, `val`, `imag_val`, `val2`, `valbak`, `val2bak`, `stat`, `nc`, `fieldsign`, `fsmask`, `d`, `mean`, `mean_imag`, `std_error`, `theta`, `phi`, `area`, `origarea`, `group_avg_area`, `K` (Gaussian curvature), `H` (mean curvature), `k1`, `k2` (principal curvatures) |
| integers | `undefval`, `old_undefval`, `fixedval`, `annotation`, `fno` (a face it is in), `cropped` (`int`); `flags` (`unsigned int`, bit 1 = sulcal); `marked`, `marked2`, `marked3` (`short`) |
| flags | `oripflag`, `origripflag`, `neg` (1: the normal is inverted), `border`, `ripflag` (1: the vertex no longer exists) (signed 8-bit) |
| distances | the distances to its neighbours in the current coordinates and in the original ones (as many as its neighbourhood holds), with the number of places allocated for each |

**Its neighbourhood**: the faces around it (`num`, an 8-bit count) with, for each, the vertex's
position (0, 1 or 2) in that face; the neighbouring vertices, sorted by hops, with the counts of
those at 1 hop (`vnum`), at 1 or 2 hops (`v2num`), at 1 to 3 hops (`v3num`) and in use (`vtotal`),
all signed 16-bit; an edge state per neighbour; the neighbourhood size the counts were filled for.
A neighbour list can be absent (a damaged surface).

**A face**: its three vertex numbers, counter-clockwise seen from outside; its area; its three
angles and their original values; `ripflag`, `oripflag` (8-bit); `marked` (`int`); its normal as a
3 x 1 double matrix and that normal's gradients with respect to its three vertices.

**The surface**: the bounding box (`xlo ylo zlo xhi yhi zhi`) and centre (`xctr yctr zctr`), the
centre of a spherical expansion (`x0 y0 z0`), totals (`total_area`, `orig_area`, `neg_area`,
`neg_orig_area`; `avg_vertex_area`, `avg_vertex_dist`, `std_vertex_dist` in double), curvature
ranges (`max_curv`, `min_curv`, `Hmin`, `Hmax`, `Kmin`, `Kmax`, `Ktotal` in double), the kind of
surface and the kind its original coordinates came from (§4), `patch` (1: a patch), `radius`
(double, for a sphere), ellipsoid parameters `a b c`, rotations `alpha beta gamma` and their last
steps, the file it was read from and the subject's name (each at most 1,024 bytes), up to 1,000
command lines, named area labels (a name, a centroid, a number), the coordinate space it was read
in, whether its coordinates are scanner coordinates, the geometry of the volume it was made from,
a colour table, a transform and the matrices to and from Talairach space, and its edges and corners
when computed (an edge: its two ends and two opposite vertices, two faces, the corners, length,
the dot product and angle of the two face normals, a cost, the unit vector, the two faces' areas;
a corner: its three vertices, face, two edges and their directions, dot product, angle, cost).

## 3. The coordinate sets and their selector

A selector picks one triple of a vertex:

| Selector | Triple |
|---|---|
| 0 original | `origx origy origz` |
| 2 temporary | `tx ty tz` |
| 3 canonical | `cx cy cz` |
| 4 current | `x y z` |
| 5 inflated | `infx infy infz` |
| 6 flattened | `fx fy fz` |
| 7 pial | `pialx pialy pialz` |
| 8 second temporary | `t2x t2y t2z` |
| 9 white | `whitex whitey whitez` |
| 12 current normals | `nx ny nz` |
| 13 pial normals | `pnx pny pnz` |
| 14 white normals | `wnx wny wnz` |

Any other selector (1, 10 target, 11 layer IV, negative, above 14) is a fatal error,
`MRISvertexCoord2XYZ_float: unsupported which <n>` with code -3 (exit status 253; **FS-01**'s NUL
byte before the newline with `reproduce`). The same triple is given as `float`s or widened to
`double`s.

## 4. The kinds of surface

| Value | Name | Distance formula |
|---|---|---|
| 0 | `MRIS_SURFACE` | 0 |
| 1 | `MRIS_PATCH` (also `MRIS_CUT`) | 0 |
| 2 | `MRIS_PLANE` | 0 |
| 3 | `MRIS_ELLIPSOID` | 0 |
| 4 | `MRIS_SPHERE` | 1 |
| 5 | `MRIS_PARAMETERIZED_SPHERE` | 1 |
| 6 | `MRIS_RIGID_BODY` | 0 |
| 7 | `MRIS_SPHERICAL_PATCH` | 0 |
| 8 | `MRIS_UNORIENTED_SPHERE` | 0 |
| 9 | `MRIS_PIAL_SURFACE` | 0 |

The kind is an `int`; asking the name or the distance formula of any other value is a failed
assertion, `<BUILD_ROOT>/utils/mrisurf_base.cpp:50 false` (name) or `...:63 false` (formula).
Two kinds are **compatible** when their formulas are equal (the first kind's checked first). The
compatibility check of the original coordinates' kind against the surface's writes nothing when
compatible, else the debug message `using incompatible <name 1> and <name 2>` (shown only with the
verbose diagnostic bit), and goes on.

**Other small rules:**

- Comparing two angle triples compares their 12 bytes as `memcmp` does (each float's bits, little
  end first; so `-0` and `+0` differ and identical NaNs are equal): 0 when equal, else the first
  differing byte of the first triple minus that of the second, as unsigned values (for example 128
  for `-0` against `+0` in the first angle) (observed: glibc 2.35 on x86-64).
- Comparing two floats for sorting gives 1 when the first is greater, -1 when it is smaller, else 0
  (NaN compares 0 with everything).
- The repeatable face-split choice of two vertex numbers `a`, `b`: FreeSurfer's rounding `nint`
  (`specs/freesurfer-core.md` §11) of `sqrt(1.9 a) + sqrt(3.5 b)` in double; a negative number makes
  the sum NaN and the rounding is **FS-12**'s (default 0, `reproduce` -2147483648).
- The "fix vertex area" switch: 1 at start; setting it returns the new value.

## 5. The neighbour counts

- **Setting the 1-hop count** of a vertex to `n`, **clearing** it (to 0), **adding** `a` and
  getting the count before (default `a` = +1), **adding** `a` and getting the count after (default
  `a` = -1). The count is signed 16-bit: a sum outside its range wraps (32767 + 1 gives -32768).
  The program keeps the largest count seen (30 at its start); a count above it becomes the largest
  and writes the debug message `modVnum: vertex <vertex number> has <count> immediate neighbours`
  (verbose only).
- **The count at `i` hops** (1, 2 or 3): `vnum`, `v2num`, `v3num`; any other `i` is a failed
  assertion, `<BUILD_ROOT>/include/mrisurf.h:2512 false`.
- **The position of vertex `b` among the 1-hop neighbours of `a`**: the first index below `vnum`
  where the list holds `b`, else -1; -1 when the vertex has no neighbour list. `a` outside
  `[0, nvertices)` is a failed assertion, `<BUILD_ROOT>/include/mrisurf.h:2517 0 <= vno1 && vno1 <
  mris->nvertices`. **"Are neighbours"** is that position not being negative.
- **The plain neighbour test** (1 or 0, `b` among the first `vnum` of `a`'s list) checks neither
  `a` nor a missing list (FreeSurfer reads memory it does not own there; no caller is known to
  pass such a vertex).
- The two topology self-checks FreeSurfer's library calls are switched off in 8.2.0: they always
  answer true.

## 6. Geometric helpers

- **Sign** of a float or a double: 1.0 when above 0, else -1.0 (0 and NaN included), as a double.
- **Normalising** a float triple: `d = sqrt((v0² + v1²) + v2²)` in float; when `d > 0` each
  component is divided by `d` (in float); `d` is returned.
- **Close enough** (floats `a`, `b`): with `m` the larger of `|a|`, `|b|` (`|b|` when they are
  equal or a NaN is involved), 1 when `m < 1e-6`, or when `|a - b|` (in float) `< m · 0.01` (in
  double); else 0.
- **Projecting a point onto a sphere** of radius `r` centred at 0, in float: `dist =
  sqrt((x² + y²) + z²)`; `d = 0` when `|dist| < 2^-23`, else `1 - r / dist`; the point is
  `(x - d x, y - d y, z - d z)`; the function returns 0 with it.
- **Triples of doubles**: sum and difference component by component; the inner product `(u0 v0 +
  u1 v1) + u2 v2` (double), also given rounded to float; a triple times a number, component by
  component (the number second); the cross product `(u1 v2 - u2 v1, u2 v0 - u0 v2, u0 v1 - u1 v0)`.

## 7. Validation plan, part 1

- **Against the official code, function by function** (`validation/probes/fs-surfio/`): `fssurfio`
  runs FreeSurfer's own functions from the unit oracle's build (queue item 3.1-02) on seeded cases of
  15 commands: every selector of §3 on vertices of special values (both float and double results,
  and the surface views' selector at three access levels), the counts at hops of §5 with
  invalid hops, the neighbour position and tests with absent lists, invalid vertices and short
  lists, sequences of the count operations of §5 (with the verbose bit on and off, starting near
  the 16-bit limits), the helpers of §6 with NaNs, infinities, zeros and subnormals, the kinds of §4
  (valid and invalid, verbose on and off), the angle and float comparisons, the face-split choice
  (negative numbers included), the "fix vertex area" switch, and every read and write of the
  per-vertex, per-face and per-surface quantities the metric computations use, on a surface and on
  FreeSurfer's array-per-quantity copy of it. Measured 2026-11-13: 30,000 cases (seed 7), all
  identical with `reproduce`; the default differs only with the notices of FS-01 (the NUL byte),
  FS-03 (failed assertions end with status 1) and FS-12 (the face split of negative numbers). The
  SHA-256 of the official outputs of seed 1 (450 cases) is checked by the port's test on every
  platform, also with AArch64's default NaN emulated.
- **The replays** of the first programs that use these functions (every program that reads a
  surface, from `mris_info` and `mris_convert` on), with `reproduce`, on the recorded heads (the
  programs' items).

## 8. Command lines

A surface keeps up to 1,000 command lines (the programs that made it), in order. **Adding one**
stores its bytes before the first NUL as the next line and returns 0. When the surface already
holds 1,000 (or more), the line is not stored: standard error gets `error: can't add cmd to surface
since max cmds (<count>) has been reached`, and the count stays (**FS-112**: FreeSurfer stores the
line past the end of its list, over the count of lines, which then holds a memory address; larmorx
keeps 1,000 lines in both modes, with FS-112's notice).

## 9. The neighbourhood checksum

A vertex's checksum is a 64-bit unsigned number: start from `2166136261` (`0x811c9dc5`); for each
byte `b` of the sequence below, `h = (h XOR b) · 16777619`, modulo 2^64 (FNV-1a's step with the
32-bit prime, kept in 64 bits). The sequence, little-endian:

1. the 1-hop, 1-2-hop and 1-3-hop neighbour counts and the count in use (`vnum`, `v2num`, `v3num`,
   `vtotal`: 2 bytes each, signed), then the neighbourhood size filled, the current size and the
   number of faces (1 byte each); the clock copy (`nsizeMaxClock`) does not enter;
2. when the vertex has a neighbour list: its first `s` entries (4 bytes each), where `s`, the size
   in use, is the count at the filled size's hops (`vnum`, `v2num`, `v3num` for size 1, 2, 3; 0 for
   any other size) or `vtotal` when that is larger;
3. when the vertex has a face list: its `num` face numbers (4 bytes each), then the `num`
   positions of the vertex in those faces (1 byte each);
4. when the vertex has distances: the first `s` (4-byte floats, their bits); then likewise its
   original distances.

No coordinate, value, flag or mark of the vertex enters the checksum. FreeSurfer's topology fixer
saves it with a vertex's state and checks it when the state is restored (a mismatch is a failed
assertion). Lists shorter than what the sequence reads are outside the behaviour (FreeSurfer reads
memory it does not own).

## 10. Comparing two surfaces

The simple comparison of surfaces 1 and 2 with a tolerance count `ndiffmin`, a distance threshold
and a verbosity writes to standard output (numbers of `%g` form are C's `%g`) and returns:

1. `Entering MRISdiffSimple(): ndiffmin=<n>, rmsthresh=<%g>, verbosity=<n>`;
2. different vertex counts: `Surfaces differ in the number of vertices <n1> <n2>`, returns 1;
   different face counts: `Surfaces differ in the number of faces <n1> <n2>`, returns 2;
3. the vertices whose 1-hop counts differ, counted (with verbosity > 0 each writes `<k> Surfaces
   differ in number of neighbors at vertex <vno>   <n1> <n2>`, `k` the count before it, three
   blanks before the counts); more than `ndiffmin`: `Surfaces differ in number of neighbors at a
   vertex ndiff=<k>`, returns 3;
4. the vertices whose rip flags differ, counted; more than `ndiffmin`: `Surfaces differ in ripflags
   ndiff=<k>`, returns 4;
5. for each vertex unripped in both: the distance `sqrt((dx·dx + dy·dy) + dz·dz)` in double, each
   difference taken in float (surface 1's coordinate minus surface 2's) and widened; it is stored
   as surface 1's `val` (rounded to float); the vertex numbered `Gdiag_no` writes `vtxno <vno>
   [<x1>,<y1>,<z1>] [<x2>,<y2>,<z2>] <distance>` (two blanks after the number); the largest
   distance (a strictly larger one replaces it, so NaN never does; starting at 0) and its vertex
   (0 at start) are kept, and the distances summed in vertex order; a distance above the threshold
   counts (with verbosity > 0: `<k> Surfaces differ in xyz at vertex <vno>  rms=<d> (<x1>,<y1>,<z1>)
   (<x2>,<y2>,<z2>)`);
6. more than `ndiffmin` counted: `Surfaces differ in vertex xyz ndiff=<k>, rmsmax = <max> at <vno>,
   rmsmean = <sum / number of vertices compared>` (0 compared gives C's NaN, printed `-nan`),
   returns 4;
7. else, when the largest distance is above 0, `rmsmax = <max>`; then `surfaces do not differ`;
   returns 0.

## 11. Borrowed marks, per-vertex scratch arrays, a frozen vertex count

- A surface records which of two vertex marks (`marked` = 0, `marked2` = 1; 2 is the list's end)
  a function has **borrowed**, as bits of an integer (bit `1 << n`). Borrowing returns 123 and
  sets the bit; borrowing one already set is a failed assertion,
  `<BUILD_ROOT>/utils/mrisurf_base.cpp:83 !(bits & *tc)`. **Checking** a borrow (the 123 is not
  looked at) asserts the bit is set (`...:92 (bits & *tc)`); **returning** it asserts the same
  (`...:99 (bits & *tc)`) and clears the bit.
- **A per-vertex scratch array** of `nvertices` floats raises the surface's frozen-vertex counter
  by one (while it is above 0 the vertex count may not change); FreeSurfer leaves the values
  undefined (larmorx gives 0); a negative vertex count gives no array (the allocation fails).
  **Freeing** it empties the caller's reference and lowers the counter by one. The counter is a
  plain `int` (no check: it may go negative).

## 12. The integrator's file names

Three surface names (initially `inflated`, `smoothwm`, `smoothwm`) and three curvature file names
(`inflated.H`, `sulc`, none) tell the surface integrator what to read (its own part).

- **Setting the inflated name** to `n` makes the first surface name `n` and the first curvature
  name `n.H`; returns 0. A name of 1,022 bytes or more (before a NUL) ends the official program
  with `*** buffer overflow detected ***: terminated` on standard error and signal 6 (exit status
  134) (**FS-111**; observed: 1,021 bytes work, 1,022 abort). larmorx takes any length, with
  FS-111's notice; `reproduce` gives the original's end.
- **Setting the sulcal name** makes the second curvature name; **setting the original name** makes
  the second and third surface names; both return 0. Names are bytes before a NUL.
- The negative-area weight of the surface integrator starts at 10.0 (a double).

## 13. Validation plan, part 2

The probe of §7 gains six commands (21 in all): command lines (counts from 0 to 1,003, lines with
NULs and high bytes), the checksum with its size in use (every filled size 0 to 3, 4 and 255,
negative counts, absent lists, lists longer than read, random values outside the neighbourhood),
the comparison (equal and different vertex and face counts, differing 1-hop counts and rip flags,
NaN and infinite coordinates, `Gdiag_no` on and off, negative `ndiffmin`, verbosity 0 and 1), the
borrowed marks (every order of borrow, check and return over the three values, from any starting
bits), the scratch arrays (negative, zero and positive vertex counts, frozen counters from -2) and
the file names (sequences of the three settings, NULs, high bytes, inflated names of 1,019 to 2,100
bytes). Measured 2026-11-13: seed 1, 630 cases, all identical with `reproduce` (the port's test
checks their SHA-256 on every platform); the default differs only with the notices of FS-01, FS-03,
FS-12 and FS-111 (FS-112's cases give the same output in both modes).

## 14. Notation for the coordinate operations

`a ⊕ b`, `a ⊖ b`, `a ⊗ b` are one IEEE operation in the type named (float or double), with the
official build's NaN rule: a NaN made from numbers is the default NaN with the sign set; where both
operands are NaNs the result is the **left** one's, quieted (§1). The formulas below are written in
the order that rule needs; for numbers any order gives the same result. "(f)" is a float
operation, "(d)" a double one; a float widened to double or a double rounded to float keeps a NaN's
sign and the top of its payload.

## 15. The neighbour distances' storage

Each vertex has distances to its neighbours in the current coordinates (`dist`, with a capacity)
and in the original ones (`dist_orig`, with its own capacity). The surface remembers, as bit 1 and
bit 2 of `dist_alloced_flags`, whether some vertex's distances (original distances) are in use.
When distances are **freed** they become absent with capacity 0, but their buffer is kept for the
next use.

- **Freeing the distances** sets `dist_nsize` to 0; when bit 1 is set, every vertex's distances
  become absent and the bit is cleared (bit 2 and the original distances likewise for **freeing
  the original distances**, which leaves `dist_orig_nsize`).
- **Making** a vertex's distances (original distances): room for the vertex's size in use (§9's
  `s`), all zero.
- **Growing** them to a minimum capacity `c`: room for the larger of `s` and `c`; the values below
  the vertex's `vtotal` (for original distances: below their old capacity) are kept, the rest zero.
- In both, when the capacity is already enough nothing is reallocated (the values from the old
  size on are still zeroed). When it is not, the kept buffer is resized to exactly the new
  capacity, keeping the values it holds up to the shorter length; making a buffer for a vertex
  whose distances were absent sets the bit. A buffer that never existed holds no values: growing
  it keeps undefined values in FreeSurfer (larmorx gives 0; findings).
- **Taking the buffer** for a new set of distances of capacity `c`: the kept buffer (when the
  distances are absent; it then leaves the vertex) or a new one, resized to `c`.
- **Setting** a vertex's distances to a buffer with a new capacity sets bit 1, frees the kept
  buffer and makes the given one the distances; a new capacity below the present one is a failed
  assertion, `<BUILD_ROOT>/utils/mrisurf_base.cpp:334 newCapacity >= v->dist_capacity`.
- The metric-properties copy's freeing was never written: a failed assertion,
  `<BUILD_ROOT>/utils/mrisurf_base.cpp:414 !"MRISfreeDistsButNotOrig(MRIS_MP* mris) NYI"`, so its
  rotation and its step along the motion vectors always end that way.

## 16. The coordinate operations

"Every vertex" includes ripped ones unless said. The **bounding box** of a set of points: from
`lo = 10000`, `hi = -10000` (floats), each coordinate above `hi` replaces it and below `lo`
replaces it (so NaNs never enter); the centre is `0.5 ⊗ (lo ⊕ hi)` (f) (a double sum of the two
widened, rounded to float, gives the same number). **Computing the surface's dimensions** stores
the current coordinates' box (`xlo ylo zlo`, `xhi yhi zhi`) and centre (`xctr yctr zctr`).

The **logic problem** of setting coordinates: when bit 1 (for current coordinates; bit 2 for the
original ones) of `dist_alloced_flags` is set, FreeSurfer's logic-problem report of
`specs/freesurfer-core.md` (shown with `FREESURFER_reportLogicErrors`, thinned per place) with the
message `dist not freed when setXYZ called.  Add call to MRISfreeDistsButNotOrig(MRIS*)`
(`dist_orig not freed when setOriginalXYZ called.  Add call to MRISfreeDistOrigs(MRIS*)`), the
function name `<unknown>` and the caller's file and line; the coordinates are set anyway.

| Operation | Behaviour |
|---|---|
| set a vertex's current (original) coordinates | the logic problem; a vertex outside `[0, nvertices)` is a failed assertion `<BUILD_ROOT>/utils/mrisurf_metricProperties.cpp:65 (0 <= vno) && (vno < mris->nvertices)` (`:180` for original coordinates) |
| export | three arrays of the current x, y, z; a negative vertex count is a failed assertion `...:80 0 == posix_memalign((void**)ppx, 64, n*sizeof(float))` |
| import from three arrays | the logic problem (line 131), the coordinates, the box and centre |
| copy from another surface | the logic problem (line 151); another vertex count is a failed assertion `...:154 nvertices == mris_from->nvertices`; the coordinates, the box and centre |
| original from current | frees the original distances, `origxyz_status = status`, every vertex's original coordinates become its current ones |
| mean radius | `d = sqrt((z⊗z) ⊕ ((x⊗x) ⊕ (y⊗y)))` (f; a negative argument cannot occur, NaN passes), `sum ⊕= d`, `sum2 ⊕= d⊗d` (d) over every vertex; mean `sum ⊘ N`; with a deviation asked, `sqrt((((sum2 ⊘ N) ⊖ (mean⊗mean)) ⊗ N) ⊘ (N - 1))` (d; `N` the vertex count: 0 gives NaN, 1 a division by 0) |
| import from three arrays of a kind | kind 4 (current) frees the distances and sets the current coordinates; any other kind is the fatal error `MRISimportVertexCoords: which <n> not supported` (code -3) at the first vertex, nothing with no vertices; returns 0 |
| reverse a set | for every unripped vertex of the current (4), canonical (3) or original (0) set, the sign of x (direction 0 and any other value), y (1) or z (2) flips (original coordinates through their setter, line 328); any other set is the fatal error `MRISreverseCoords: unsupported which_vertices <n>` (code -3) at the first unripped vertex; direction 0 with the face flag reverses the faces; returns 0. The plain **reverse** does the same on the current set |
| reverse the faces | every face's first and third vertices swap; for each of its three vertices, the vertex's position in the face (the first match) is written into its position list wherever its face list (first `num`) names this face; a vertex not in the face is a failed assertion `<BUILD_ROOT>/utils/mrisurf_topology.cpp:1929 n < VERTICES_PER_FACE` |
| scale by `s` (double) | frees the distances; each current coordinate `x ⊗ s` (d, stored as float); the box's ends and centre likewise but `zhi`, which is `s ⊗ zhi` (d). With `s < 0` the box stays upside down in FreeSurfer (**FS-113**; larmorx swaps each axis's ends, `reproduce` does not); returns 0 |
| flip a patch | every `x` negated; `xlo = -xhi`, `xhi = -xlo` (the old values), `xctr = -xctr`; returns 0 |
| translate by `d` (floats) | every current coordinate `x ⊕ dx` (f); the box and centre `dx ⊕ xlo`, ... for all but `zhi`, which is `zhi ⊕ dz`; the distances are not freed; returns 0 |
| scale then translate (`s`, `d` doubles) | frees the distances; each coordinate `(x ⊗ s) ⊕ d` (d, stored as float); the box and centre likewise (each end and the centre `(e ⊗ s) ⊕ d`, but `ylo`, `zlo`: `d ⊕ (s ⊗ e)`); a negative factor leaves that axis's box upside down (FS-113, as for the scale) |
| anisotropic scale (`sx sy sz` floats) | computes the dimensions and frees the distances; when all three are 1 nothing else; else, around the centre `c`, each coordinate `((x ⊖ c) ⊗ s) ⊕ c` (f); the low ends the same, the high ends `c ⊕ (s ⊗ (hi ⊖ c))`; each axis's two results stored in order (the first as `lo` when it is below the second, else swapped: NaN swaps); the new centre `(hi' ⊕ lo') ⊗ 0.5` (f) of the two results before ordering; returns 0 |
| rotate by `α, β, γ` (radians, floats) | frees the distances; `sa, ca` the float sine and cosine of α (likewise `sb cb`, `sg cg`; glibc's with `reproduce`); with `cacb = ca⊗cb`, `cacg = ca⊗cg`, `cacgsb = cacg⊗sb`, `sasg = sg⊗sa`, `cgsa = cg⊗sa`, `casbsg = (sb⊗ca)⊗sg`, `cbsa = cb⊗sa`, `cgsasb = sb⊗cgsa`, `casg = ca⊗sg`, `sasbsg = (sa⊗sb)⊗sg`, `cbcg = cg⊗cb`, `cbsg = sg⊗cb`, `k1 = (-cacgsb) ⊖ sasg`, `k2 = cgsa ⊖ casbsg`, `k3 = cgsasb ⊖ casg`, `k4 = cacg ⊕ sasbsg` (all f), each vertex becomes `x' = ((k1⊗z) ⊕ (cacb⊗x)) ⊕ (k2⊗y)`, `y' = ((k3⊗z) ⊖ (x⊗cbsa)) ⊕ (k4⊗y)`, `z' = ((z⊗cbcg) ⊕ (x⊗sb)) ⊕ (y⊗cbsg)` (the rotation Z(α)·Y(β)·X(γ)) |
| multiply by a matrix `M` | frees the distances; each vertex's `(x, y, z, 1)` times `M` with FreeSurfer's float matrix product (`specs/freesurfer-maths.md`); the first three rows are the new coordinates; when `M` is not 4 x 4 the product prints its error for every vertex and the vertex becomes `(0, 0, 0)`; an absent `M` is the product's fatal error; returns 0 |
| spherical coordinates | frees the distances; with `x, y, z` widened, `d2 = (x⊗x) ⊕ (y⊗y)`, `(x, y, z)` become `(atan2(y, x), atan2(z, sqrt(d2)), sqrt((z⊗z) ⊕ d2))` (d, stored as float); `atan2` with a NaN argument gives `x ⊕ y` (observed); returns 0 |
| move the origin to `p` | computes the dimensions, translates by `px ⊖ xctr`, ... (f), then the centre is exactly `p` |
| centre | moves the origin to `(0, 0, 0)` |
| centre of gravity | the sums `s ⊕= x` (d) over every vertex, each divided by the vertex count (d); **centring on it** translates by minus the centre rounded to float and returns the centre |
| blend with the temporary set (`a`, `b` floats) | frees the distances; `x = (x ⊗ a) ⊕ (tx ⊗ b)` (f) |
| blend with the normals (`a`) | frees the distances; `x = (nx ⊗ a) ⊕ x` (f) |
| step along the motion (`dt` double) | frees the distances; every unripped vertex `x = (dx ⊗ dt) ⊕ x` (d, from the float step and coordinate; stored as float) |

## 17. Validation plan, part 3

The probe's `surf` command: a surface of 0 to 5 vertices and up to 3 faces from the case (every
coordinate set the operations use, rip flags, neighbour counts and face lists, distances in every
state: absent, valid, freed with a kept buffer, with capacities below their buffers), its box,
`dist_alloced_flags`, `dist_nsize` and kinds, then 1 to 5 of the operations of §15-16 (special
floats: NaNs of both signs with payloads, signalling ones, infinities, zeros, subnormals; negative,
unit and NaN factors; matrices of 4 x 4, 3 x 4, 4 x 3 and none; the logic problem's report on and
off), then the whole state. Measured 2026-11-13: seed 11, 4,000 such cases identical with
`reproduce`; 1,331 cases of NaN and special coordinates for the spherical coordinates; 300,000
random spherical coordinates identical (glibc 2.35's `atan2` differs from the correctly rounded
one in the last bit of about 0.09 % of random arguments, measured; none of those reaches a float
result here, so a correctly rounded `atan2` suffices).

## 18. Averaging and smoothing positions

Notation as §14. `s` a vertex's 1-hop neighbours: the first `vnum` entries of its neighbour list;
"unripped" neighbours are those whose rip flag is 0.

- **Averaging the positions `n` times** (`n` an int; none for `n <= 0`): the distances (not the
  original ones) are freed (§15), then each round computes, for every vertex from the previous
  round's positions, `x' = (x ⊕ Σ xn) ⊘ c` (f), the sum in neighbour order (`x ⊕ xn1 ⊕ xn2 ...`,
  left to right), over its unripped neighbours, `c` their count plus 1 as a float. A ripped
  vertex, or one with `vnum <= 0`, keeps its position **without** the division (a signalling NaN
  stays signalling; with neighbours that are all ripped the division by 1 is made and quiets it).
  The rounds' results then replace the current coordinates as the import of §16 does (the box and
  centre; no logic problem, since the distances were freed). Returns 0.
- **Smoothing `n` times with step `t`** (`t` a float, cut to 1 when `t > 1`): first the logic
  problem `MRISsmoothSurface was calling MRIScomputeMetricProperties too soon` (FreeSurfer's
  report of `specs/freesurfer-core.md`, function `MRISsmoothSurface`, at
  `<BUILD_ROOT>/utils/mrisurf_metricProperties.cpp:1172`; the variable
  `FREESURFER_fix_MRISsmoothSurface1` names the fix and changes nothing else); the distances freed;
  each round, for every vertex, `m = (0 ⊕ Σ xn) ⊘ c` (f) over its unripped neighbours (none for a
  ripped vertex; `c` their count, so `0 ⊘ 0`, the default NaN, without any) and then, when `t` is
  exactly 1, `x' = m`, else `x' = ((m ⊖ x) ⊗ t) ⊕ x` (f; a NaN `t` takes this branch); the rounds'
  results imported as above; then the metric properties are computed (queue items 3.1-51 and
  3.1-52).

## 19. Centring a sphere

The current coordinates are moved and scaled so the surface's centre is the origin and its radius
100, in double unless marked (f):

1. `Finding true center and radius of Spherical Surface...` on standard error (no newline).
2. A surface with **no vertex** never returns in FreeSurfer (its energy is 0 and its gradient
   `0 ⊘ 0`, so the step search of 5 never ends): **FS-114**; larmorx refuses it, in both modes, with
   the fatal error `MRIScenterSphere: the surface has no vertex, so it has no centre` (code -4,
   FreeSurfer's fatal-error form) after the message of 1.
3. The bounding box over every vertex (from -10000 and 10000, each widened coordinate above or
   below replacing the end; NaNs never enter); the start `c = (lo ⊕ hi) ⊗ 0.5` per axis.
4. With `dx = x - cx` widened (d), the **squared radius** `R²(c) = (Σ ((dx⊗dx ⊕ dy⊗dy) ⊕ dz⊗dz)) ⊘ N`
   (each term added to the running sum, the sum on the left), and the **energy**
   `E(c, R²) = Σ ((((dy⊗dy) ⊕ (dx⊗dx)) ⊕ (dz⊗dz)) ⊖ R²)²` (note: `dy²` first), over every vertex,
   `N` the vertex count as a double.
5. `R² = R²(c)`, `E = E(c, R²)`. If `E(0, 10000) < E`: `c = 0`, `R² = 10000`, `E = E(0, 10000)`.
   If `E(0, R²(0)) < E`: `c = 0`, `R² = R²(0)`, `E = E(0, R²)`.
6. `last = E ⊕ 1`; while `E < last` (at most 100 rounds; the 101st stops before doing anything):
   `last = E`; the **gradient** `g = (Σ (dx ⊗ e), Σ (dy ⊗ e), Σ (e ⊗ dz)) ⊘ N` with
   `e = ((dx⊗dx ⊕ dy⊗dy) ⊕ dz⊗dz) ⊖ R²` (sums as in 4); `d = sqrt((gx⊗gx ⊕ gy⊗gy) ⊕ gz⊗gz)`; when
   `d > 1`, `g = g ⊘ d`. Then with `ε = 2`, while `E >= last` (a NaN energy ends it): `ε = ε ⊗ 0.5`, the trial
   centre `c' = (g ⊗ ε) ⊕ c` per axis, `E = E(c', R²(c'))`, `d` recomputed from `g` as above, and
   the repeat ends at once when `ε ⊗ d < 1e-11`. After the repeat: when `E < last`, `c = c'` and `R² = R²(c)`; else `E = E(c, R²)`.
7. `k = sqrt(10000 ⊘ R²)`; every vertex `x = ((x ⊖ cx) ⊗ k) ⊕ 0` (stored as float).
8. Twice for every vertex: `r² = ((y⊗y) ⊕ (x⊗x)) ⊕ (z⊗z)` (f, widened), `k = 100 ⊘ sqrt(r²)`,
   `x = x ⊗ k` (d, stored as float), likewise `y`, `z`.
9. `done\nSurface centered at (0,0,0) with radius 100.0 in <n> iterations\n` on standard error, `n`
   the rounds started in 6 (101 when the limit stopped them).

## 20. The neighbour distances

A vertex's **count in use** for distances is `vtotal`; when checking, it is its count at the
stored number of hops (`vnum`, `v2num`, `v3num` for 1, 2, 3). The **formula** comes from the kind of
surface (§4): the plane formula, or the sphere's for the kinds that use it; the current
coordinates use `status`, the original ones `origxyz_status`; a kind outside the list is §4's
failed assertion (`<BUILD_ROOT>/utils/mrisurf_base.cpp:63 false`).

**Computing** the distances (current: into `dist`; original: into `dist_orig`) at `h` hops:
`h <= 0` is a failed assertion `<BUILD_ROOT>/utils/mrisComputeVertexDistancesWkr_extracted.h:15 0 <
new_dist_nsize`, `h > 3` one at `:16 new_dist_nsize <= 3`. For every unripped vertex the distances
are **made** (§15: room for the size in use, all zero), then for each of its first count-in-use
neighbours (ripped ones too):

- **plane formula:** with `xd = x ⊖ xn` (f), `d = sqrt((zd⊗zd) ⊕ ((yd⊗yd) ⊕ (xd⊗xd)))` (f,
  correctly rounded; `sqrt` of a negative cannot occur, of a NaN passes it);
- **sphere's formula:** 0 when the neighbour is ripped or its length (below) is below
  `FLT_EPSILON`; else `|angle| ⊗ r` (f), `r` the vertex's own length, `angle` the approximate
  angle between the vertex's unit vector and the neighbour's position (`specs/freesurfer-maths.md`'s
  approximate angle with a known length; its dot product's third term is `z2 ⊗ n.z`, the
  neighbour's coordinate first). A vertex's **length** and **unit vector** are the normalised load
  of its position (`specs/freesurfer-maths.md`: `sqrt((x⊗x ⊕ y⊗y) ⊕ z⊗z)` in double rounded to
  float; the unit vector `(x, y, z) ⊗ (1 ⊘ len)` (f), or `(1, 0, 0)` for length 0); the vertex's own
  length may be 0 or not finite (its distances are then whatever the product gives).

Then, when no unripped vertex has a non-zero x coordinate (counted over every unripped vertex;
when checking with the plane formula, only over those whose distances exist), the logic problem
`all xyz inputs zero - cant fix` with no fixing variable (file
`<BUILD_ROOT>/utils/mrisComputeVertexDistancesWkr_extracted.h`, line 232, function
`mrisComputeVertexDistancesWkr`, or `mrisComputeOriginalVertexDistancesWkr` for the original
ones). Computing returns 0 and sets `dist_nsize` (`dist_orig_nsize`) to the surface's `nsize`,
the number of hops used.

**Checking** the stored distances: true at once when `dist_nsize` (`dist_orig_nsize`) is 0; else
the same computation at that many hops (its assertions too), only for vertices whose distances
exist, nothing made or stored: true when every computed distance equals the stored one (a NaN never
equals), false otherwise. In the plane formula the checking sum is
`(zd⊗zd) ⊕ ((xd⊗xd) ⊕ (yd⊗yd))` (the computing one adds `yd²` first): only NaN bits can differ.

**Computing the original distances if necessary:** nothing when bit 2 of `dist_alloced_flags` is
set; else the logic problem `dist_orig not already computed` (line 1628, function
`mrisComputeOriginalVertexDistancesIfNecessaryWkr`, fixed by `FREESURFER_fix_missing_dist_orig`),
the original distances computed, and, unless that variable is set, every vertex's original
distances set to 0 for its whole capacity (FreeSurfer's earlier behaviour, kept on purpose).

**Clearing the original distances:** every unripped vertex's first `vtotal` original distances 0
(a vertex without them, or with fewer, is outside the behaviour). **Scaling the distances** by `k`
(float): every unripped vertex's distances in use (the size of §9) `d ⊗ k` (f). Both return 0.

**The percent distance error:** for a patch the scale is 1; otherwise a total area or original
area not above 0 is a failed assertion (`...mrisurf_metricProperties.cpp:1682 mris->total_area >
0.0`, `:1683 mris->orig_area > 0.0`) and the scale is `sqrt(orig_area ⊘ total_area)` (f, then
widened). For every unripped vertex and each of its first `vtotal` neighbours, `dist = d ⊗ scale`
(d; 0 without distances), `odist` the original distance widened (0 without them); the sums
`Σdist ⊕= dist`, `Σodist ⊕= odist`, `Σerr = |dist ⊖ odist| ⊕ Σerr` (the new term on the left) and
the count of pairs. With `DIAG_VERBOSE` set, the debug messages `MRISpercentDistanceError called
when all dist zero` when `Σdist == 0` and `MRISpercentDistanceError called when all dist_orig zero`
when `Σodist == 0`. With a count of 0 nothing is divided (the count becomes 1); else `Σodist` and
`Σerr` are divided by the count; the result is 100000 when `|Σodist| < FLT_EPSILON`, else
`100 ⊗ (Σerr ⊘ Σodist)`.

## 21. Areas, triangle corners and coordinate sets

- **Clearing the original area** sets every unripped vertex's `origarea` to 0 (returns 0); the
  variant with `val2` clears both.
- **A face's area at corner `n`** (0, 1, 2; `n0` the previous corner, `n1` the next, cyclically):
  with `p` the corner's current position, `v0 = p ⊖ p0`, `v1 = p1 ⊖ p` (f), the cross product
  `d1 = (v0y⊗v1z) ⊖ (v1y⊗v0z)`, `d2 = (v0z⊗v1x) ⊖ (v1z⊗v0x)`, `d3 = (v0x⊗v1y) ⊖ (v1x⊗v0y)`, the
  length `L = sqrt((d1⊗d1 ⊕ d2⊗d2) ⊕ d3⊗d3)` (f) and `dot = ((px⊗d1) ⊕ (d2⊗py)) ⊕ (d3⊗pz)` (f);
  the area is `-L` (the sign bit flipped, also of a NaN) when `dot < 0`, else `L`.
- **Zeroing negative areas:** every unripped vertex's and face's area below 0 becomes 0 (NaN
  stays); returns 0.
- **A face's three corners** as doubles: the original coordinates; or the white set (9), the pial
  set (7), or the current set for any other kind. Returns 0.
- **Extracting a coordinate set** into three float arrays: current (4), target (10), white (9),
  layer IV (11), pial (7), inflated (5), flattened (6), canonical (3), original (0), the second
  temporary set (8) or the temporary set (2), every vertex; any other kind is the fatal error
  `MRISextractVertexCoords: which <n> not supported` (code -3) at the first vertex (nothing without
  vertices). Returns 0.

## 22. The curvature operations

The surface's curvature **range** (`max_curv`, `min_curv`): from the first unripped vertex's `curv`
both, then each `curv` above the maximum or below the minimum replacing it (NaN never enters
after the first); unchanged with no unripped vertex. Below, `mean` is the unripped vertices' mean
curvature, `Σ ⊕= curv` (d) divided by their count (`0 ⊘ 0` without any); every result is stored as
float; each operation returns 0.

| Operation | Behaviour |
|---|---|
| clear | every unripped vertex's `curv` 0 (the variant with `val2` clears both) |
| rectify | every vertex's `curv` (ripped too) its absolute value (sign bit cleared), then the range |
| scale to `[a, b]` (floats) | over the unripped vertices, the old range from -100000 and 100000 (doubles: each `curv` above or below replacing it), `k = (b ⊖ a) (f, widened) ⊘ (old max ⊖ old min)`, each `curv = ((curv ⊖ mean) ⊗ k) ⊕ mean`; the range is not updated |
| smooth the mean curvature `n` times | each round, for every unripped vertex, `d = H' ⊘ w` (d) with `H' = (H ⊗ 0.4 (f), widened) ⊕ Σ(0.25 ⊗ Hn)` over the first `vnum` neighbours then `⊕ Σ(0.05 ⊗ Hn)` from there up to `v2num` (0.05 being `0.25 - 0.4/2` in float; products in double), `w = (0.4 + vnum·0.25) + (v2num - vnum)·0.05` (f, the counts as floats); then every unripped vertex's `H` becomes its `d` |
| zero mean | every unripped `curv = curv ⊖ mean`; with `DIAG_SHOW` and `DIAG_VERBOSE`, `curvature mean = %2.3f` on standard output first; then the range |
| normalise, median (1) | every vertex's `curv` (ripped too) sorted by the C library's `qsort` with the float comparison (`specs/glibc-qsort.md`; a NaN compares equal), the median the element at `nvertices / 2` (widened); `std = sqrt(Σ(curv ⊖ median)² ⊘ count)` over the unripped; `curvature median = %2.3f, std = %2.3f` on standard output; every unripped `curv = (curv ⊖ median) ⊘ std`; then the range. A surface with no vertex is outside the behaviour (FreeSurfer reads one past its empty copy) |
| normalise, none (-1) | nothing |
| normalise, any other value (the mean, 0) | `std = sqrt(Σ(curv ⊖ mean)² ⊘ count)`; `curvature mean = %2.3f, std = %2.3f` on standard output; every unripped `curv = (curv ⊖ mean) ⊘ std`; then the range |
| normalise the variance | as the mean's, but `curv = ((curv ⊖ mean) ⊘ std) ⊕ mean`, and the message only with `DIAG_SHOW` and `DIAG_VERBOSE` |
| count compressed edges closer than `m` (double) | for every unripped vertex and each unripped 1-hop neighbour, `dx = xn ⊖ x` (f, widened), `sqrt((dx⊗dx ⊕ dy⊗dy) ⊕ dz⊗dz) < m`; returns the count (each pair counted from both ends) |
| store the mean curvature | every unripped vertex's `H` becomes its `curv` |

The messages' numbers are `printf`'s `%2.3f` of the doubles (`nan`, `-nan`, `inf`, `-inf` as glibc
writes them).

## 23. Saved coordinates

**Saving** the current coordinates keeps the vertex count and every vertex's `x, y, z`.
**Loading** them back sets each vertex's coordinates through the setter of §16 (its logic problem,
at the setter's place `<BUILD_ROOT>/utils/mrisurf_metricProperties.cpp:3148`, once per vertex as
the report's thinning allows); a saved count other than the surface's is a failed assertion
`...:3145 nvertices == mris->nvertices`. **Popping** loads and then frees them.

## 24. Validation plan, part 4

The probe's `metric` command (run on the probe built with a stub metric-properties step that
writes `MP`, so the smoothing's averaging is compared alone): a surface of 0 to 6 vertices and up
to 4 faces from the case (every coordinate set, `curv`, `H`, `val2`, `d`, areas, rip flags, the
neighbour counts with a neighbour list of their largest length, distances in every state as §17,
the face areas and rip flags), the kinds (valid and outside the list), `dist_alloced_flags`, the
three hop counts (0 to 3, -1, 4), patch, total and original areas, the curvature range, the logic
problem's report on or off, the two fixing variables, `DIAG_SHOW|DIAG_VERBOSE` on or off; one case
in three NaN-heavy (a NaN of either sign with payloads, signalling ones, or an infinity in one value
of four); then 1 to 6 of the operations of §18-23; then the whole state. An operation whose reads
or writes would leave what the case allocated (clearing or scaling distances shorter than the
count in use, checking or the percent error over distances shorter than the count read) prints
`G` and is skipped, on both sides. Measured 2026-11-13: seeds 13 and 17, 2,000 and 10,000 such
cases identical with `reproduce` but FS-114's (FreeSurfer's never return: the probe's 5-second
alarm, `CRASH 14`, against larmorx's refusal; counted, not compared); the probe's whole-run numbers
are in `validation/probes/fs-surfio/README.md`.

## 25. Saved vertex positions and normals

**Saving the current positions** into a set (the kinds of §3, with 10 target and 11 layer IV):
- original (0): the original distances are freed (§15's "freeing the original distances"), the
  original coordinates' kind becomes the surface's kind, and every vertex's `origx, origy, origz`
  its `x, y, z`;
- temporary (2), canonical (3), inflated (5), flattened (6), pial (7), second temporary (8),
  white (9), target (`targx targy targz`, 10), layer IV (`l4x l4y l4z`, 11): every vertex's triple
  becomes its `x, y, z` (bits copied, ripped vertices too);
- any other kind (1, 4 current, 12 and above, negative): a failed assertion
  `<BUILD_ROOT>/utils/mrisurf_metricProperties.cpp:3230 false` when the surface has a vertex (§1;
  nothing happens without one).

After saving into the canonical set the canonical angles are computed (§26). Returns 0.

**Restoring positions** from a set: with the original set (0) first the check that the surface's
kind and the original coordinates' kind use the same distance formula (§4: when not, the debug
message `using incompatible <kind> and <kind>`, shown with `DIAG_VERBOSE`; a kind outside the list
is §4's fatal error); then `dist_nsize` becomes 0 and every vertex's `x, y, z` (ripped too)
becomes the set's triple: target, white, layer IV, pial, inflated, flattened, canonical, original,
second temporary, and the **temporary set for any other kind** (current 4, 1, 12, negative...).
On a **spherical patch** (kind 7) every unripped vertex's canonical triple then becomes the same
triple (white, pial, inflated, flattened, original, second temporary, temporary or any other: the
temporary set; the canonical set: unchanged), except for the target and layer-IV sets:

- **FS-115** (corrected by default, rule A): for the target and layer-IV sets the original
  surface sets the canonical triple to the **temporary** set (they are missing from the second
  list), so `cx, cy, cz` differ from the restored `x, y, z`. larmorx sets them to the restored
  set, as for every other named set, and notes FS-115 when a vertex's canonical triple differs by
  it; `reproduce` gives the temporary set.

Then the bounding box and centre are recomputed from the current coordinates (§16's
`mrisComputeSurfaceDimensions`). Returns 0. Restoring the old positions is restoring the temporary
set; storing the current positions is saving into it.

**Saving the normals** (`nx, ny, nz`) into the temporary set (2), the second temporary set (8),
the pial normals (`pnx pny pnz`, 7) or the white normals (`wnx wny wnz`, 9); **restoring** them
from the temporary, second temporary or pial normals. Any other kind, and restoring from the white
normals (9), is the fatal error `MRISsaveNormals: unsupported which <n>` (`ERROR_BADPARM`, code -2,
exit status 254; FS-01's NUL with `reproduce`; restoring names the saving function too) at the
first vertex, nothing without vertices. Return 0.

## 26. Canonical angles, the mean canonical radius, folding

**The mean canonical radius**: the canonical coordinates' bounding box in double (from 10000 and
-10000; each coordinate above the high end or below the low end replacing it, so a NaN never
enters), its centre `c = (lo ⊕ hi) ⊗ 0.5` (d), and the mean over every vertex (ripped too) of
`sqrt((dx⊗dx ⊕ dy⊗dy) ⊕ dz⊗dz)` with `dx = cx ⊖ c_x` (d), summed `r ⊕= …` (d) and divided by the
vertex count (d): `0 ⊘ 0`, the default NaN, without a vertex.

**The canonical angles**: the surface's radius becomes the mean canonical radius, then
`nint(radius)` (§1 of `specs/freesurfer-core.md`: FS-12 for NaN and values beyond `int`) as a
float `r`, and `radius` that float widened. For every vertex (ripped too): `theta = atan2f(cy,
cx)` and, when `theta < 0`, `theta = 2π ⊕ theta` in double (π as a double) rounded to float;
`d = (r⊗r) ⊖ (cz⊗cz)` (f), 0 when below 0 (a NaN stays), `phi = atan2f(sqrt(d), cz)`. The
official build's `atan2f(y, x)` is glibc 2.35's (`larmorx_core::math::atan2f_glibc` gives its
bits); larmorx's default takes the correctly rounded value instead (the double `atan2` rounded to
float: class (d) of the porting guide, no notice; on the probe's arguments of §29 about one call in
six differs), `reproduce` glibc's. Of NaN arguments both give the NaN of `x ⊕ y` (f), `x` being
the second argument (`cx`, or `cz`). Returns 0.

**Folding**: `Σ ⊕= (area ⊗ (k1 ⊖ k2)) ⊗ (k1 ⊖ k2)` (d, each float widened) over the unripped
vertices, from 0.

## 27. The face-normal cache

Each face has a cache entry (`nx, ny, nz`, `orig_area`, floats) and a deferral byte: bit 1 "the
normal is to be computed", bit 2 "the original area is to be computed" (any other bits count as
"deferred" for reading the normal only).

**A face's normal and original area from its corners' original coordinates** (`v1, v2, v3` the
face's three vertices in order; every operation (f)): `a = v2 ⊖ v1`, `b = v3 ⊖ v1` (each
coordinate), `n = a × b` with `nx = (ay⊗bz) ⊖ (by⊗az)`, `ny = (az⊗bx) ⊖ (bz⊗ax)`, `nz = (ax⊗by) ⊖
(bx⊗ay)`, `len = sqrt((nx⊗nx ⊕ ny⊗ny) ⊕ nz⊗nz)`. When `|len| < FLT_EPSILON` (a NaN is not):
`a = (v3 ⊖ v2) ⊗ 100`, `b = (v1 ⊖ v2) ⊗ 100` (the product of the float difference and 100, one
rounding), `n = a × b` as before, `len` as before, and when that is also below `FLT_EPSILON` the
length is taken as 1 (`n` kept as it is). The result is `n ⊘ len` (each coordinate) and `len ⊘ 2`
for the original area. A vertex number outside the surface is outside the behaviour (FreeSurfer
reads out of its array).

The operations:

| Operation | Behaviour |
|---|---|
| set the normal | the entry's `nx, ny, nz`; bit 1 cleared |
| set the original area | the entry's `orig_area`; bit 2 cleared |
| compute the face's normal | both computed from the original coordinates and set (both bits cleared) |
| read the normal (the entry) | when the byte is nonzero: the normal computed and stored, the original area too when bit 2 is set (otherwise the stored area kept), the byte 0; then the entry |
| read the original area | the stored area, when bit 2 is clear; see FS-116 |
| defer every face | every face's byte becomes 3 (with or without the variable `FREESURFER_deferSetFaceNorms_parallel`, read once per process: the same bytes) |
| recompute every face | every face's normal and area computed and stored, every byte 0 |
| undefer | every face with a nonzero byte gets both computed and stored (whatever its bits), byte 0 |

- **FS-116** (corrected by default, rule A): reading the original area of a face whose bit 2 is
  set prints `<BUILD_ROOT>/utils/mrisurf_metricProperties.cpp:7747 NYI` and a newline on standard
  error and ends the program with a deliberate segmentation fault (signal 11). larmorx computes the
  area as the cache would (half the length above), stores it, clears bit 2 (bit 1 stays) and
  returns it, noting FS-116; `reproduce` prints the message and ends with exit status 139.

## 28. The original area, storing and restoring the metric properties

- **The original area** of the surface: the sum (f, from 0, in face order) of the unripped faces'
  cached original areas, each read through "read the normal" of §27 (so a deferred face is computed
  first, and its normal too). **Setting the original area** stores it in the surface's
  `orig_area`.
- **Storing the metric properties**: for every unripped vertex, `origarea` becomes `area`; when it
  has distances (§15: in use), its original distances are made when it has none (§15's "making the
  original distances": zeroed, the size in use) and the first `vsize` of them (§5's size in use)
  become the distances' (bits copied); then for every unripped face, the cached original area
  becomes the face's `area` (bit 2 cleared) and its original angles its angles; the surface's
  `orig_area` becomes `total_area`. Returns 0. Reading or writing past a distance buffer's
  capacity is outside the behaviour.
- **Restoring the metric properties**: for every unripped vertex, `area` becomes `origarea` and the
  first `vsize` distances the original distances' (a vertex in use without both buffers is outside
  the behaviour: FreeSurfer writes through a null pointer); then for every unripped face, `area`
  becomes the cached original area ("read the original area": FS-116 when deferred) and its angles
  its original angles; the surface's `total_area` becomes `orig_area`. Returns 0.

## 29. Validation plan, part 5

The probe's `posn` command: a surface of 0 to 6 vertices and up to 4 faces from the case (every
coordinate set and the normals, `k1`, `k2`, `area`, `origarea`, `theta`, `phi`, rip flags, the
neighbour counts, distances in every state as §17, each face's corners (one face in six with two
equal corners), area, angles, original angles, rip flag, cache entry and deferral byte (0 to 3, 4,
6, -1)), the kinds (the spherical patch in one case of three, the rest as §24), `dist_alloced_flags`,
the hop counts, the radius (any double), total and original areas; one case in three NaN-heavy as
§24, one in four with original coordinates in steps of 2^-14 (so that a face's first cross product
is shorter than `FLT_EPSILON` and its second is not); then 1 to 6 of §25-28's operations (the kinds
valid mostly, 1, 4, 12 and -1 now and then); then the whole state (every coordinate set, the
angles, areas, distances, each face's area, angles, cache entry and byte, the radius, the areas,
the kinds, the hop counts, the bounding box). Storing or restoring the metric properties where it
would read or write past a buffer prints `G` and is skipped on both sides. Measured 2026-11-13:
seed 13, 10,000 `posn` cases identical with `reproduce` (602 of them FS-116's crash, 206 the
assertion of §25, 766 fatal errors); the default differs only where a notice says why (FS-01,
FS-03, FS-12, FS-115, FS-116) or in the angles of §26 (347 cases, correctly rounded `atan2f`);
seed 7, the probe's whole run of 48,024 cases (2,001 `posn`), the same; the seed-1 hash of every
command's 30 cases is in the probe's README.

## 30. A surface's room, counts and life

A surface has **room** for `max_vertices` vertices and `max_faces` faces and uses the first
`nvertices` and `nfaces` of them (all `int`). The vertex room holds, per place, a vertex (§2), its
neighbourhood, and the two kept distance buffers of §15; the face room holds, per place, a face
and its face-normal cache entry and deferral byte (§27). Every place a surface gains is zero in
every field (no list, no buffer, numbers 0); a place that leaves the room is gone.

**Making a surface** with room `max_vertices`, `max_faces` and counts `nvertices`, `nfaces`: every
field of the surface zero (every list absent, every name empty, the volume geometry's every
number 0 — its voxel sizes and direction cosines too, not the 1 and -1 a fresh geometry has —
no colour table, no transform, no matrices), then "changing the room of both" below with those four
numbers, then the neighbourhood size and its maximum (`nsize`, `max_nsize`) 1. The plain form gives
room for exactly the counts.

**Changing the vertex room** to `m` with `n` in use: `m` is raised to the present room when below
it (the vertex room never shrinks). Then, in this order, failed assertions (§1) at
`<BUILD_ROOT>/utils/mrisurf_base.cpp` with these lines and tests: 521 `nvertices >= 0` (`n`
negative), 522 `max_vertices >= nvertices` (`m < n`), 523 `!mris->nverticesFrozen` (the frozen
count below is not 0). The room becomes `m`: the places from the **old count in use** (not the old
room) up to `m` are zeroed — vertices that had been dropped from use by lowering the count lose
their fields here — except that a zeroed vertex's distance buffers stay kept (§15: absent, with
the buffer reused by the next making or growing); the kept-buffer places past the old room are
empty. Then `nvertices` = `n`, `max_vertices` = `m`, and the result is "done" (1).
The **zero-room rule** (FS-117): the official program's arrays behave as the system's memory
reallocation does, for which an array that exists, given zero places, ceases to exist, while an
array that does not exist, given zero places, comes to exist empty. So, at room 0 (only possible
when the room already was 0): if the vertex array exists, it ceases to exist and the result is
"failed" (0) with the counts and room unchanged (nothing else changes); if it does not exist, it
comes to exist and the other three per-place arrays (neighbourhoods, both kept-buffer lists),
which exist, cease to exist; the result is "done". Every surface made has every array (a surface
made with no vertex has empty ones), so the first change to room 0 fails, the next succeeds, and
so on alternately.
- FS-117 (corrected by default, rule A): larmorx keeps empty arrays (the result "done", every
  array existing), with FS-117's notice whenever an existing array would have ceased to exist;
  `reproduce` gives the alternation.

**Changing the face room** to `m` with `n` in use: failed assertions at lines 579 `nfaces >= 0`
and 580 `max_faces >= nfaces`. The face room becomes **exactly** `m` (it can shrink: the faces past
`m` are dropped); the places from the old count in use up to `m` are zeroed (the face, its cache
entry and byte); `nfaces` = `n`, `max_faces` = `m`; the result is "done". The zero-room rule: the
three per-place face arrays always exist on a surface made (with no face too), so a change to room
0 makes the face array cease to exist and fails the assertion at line 584 `mris->faces` (FS-117:
any change of a surface's face room to 0 ends the program). (Lines 588 `mris->faceNormCacheEntries`
and 592 `mris->faceNormDeferredEntries` would follow for the other two arrays.) larmorx's default
keeps empty arrays and goes on, with FS-117's notice; `reproduce` gives the assertion.
"Allocating the faces" with `n` is the change to room `n` with `n` in use.

**Changing the room of both** (`max_vertices`, `max_faces`, `nvertices`, `nfaces`): failed
assertions at lines 634 `nvertices >= 0` and 635 `nfaces >= 0` first; each room raised to its
count when not above it; then the vertex room's change (its result ignored) and the face room's.
The plain form ("reallocating to counts") gives the counts as the rooms.

**The counts alone**: growing the vertex count to `n` (assertion 559 `nvertices <=
mris->max_vertices`); truncating it, and recording vertices as removed (both: assertion `mris->nvertices
>= nvertices`, at 565 and 571); growing the face count (613 `nfaces <= mris->max_faces`); truncating
it and recording faces as removed (619 and 625 `mris->nfaces >= nfaces`). Each sets the count;
negative counts pass these assertions, and a later change of room on a surface with a negative
count is outside the behaviour (the official program zeroes memory before its arrays). **The frozen
count**: acquiring adds 1, releasing subtracts 1 (no check).

**Copying the metadata** from a source into a target (a new surface made with the source's counts
when no target is given): the kind of surface made (`type`), its kind (`status`), the hemisphere;
the volume geometry (assigned as fs.mri's geometry assignment does, `specs/freesurfer-mri.md`);
when the source has a transform array, a copy of it (as many transforms, its type, scale, subject
and transforms) replaces the target's; each of the two Talairach matrices the source has replaces
the target's with a copy; whatever the source lacks, the target keeps. `free_transform` is not
copied. The result is the target.

**Destroying** a surface prints nothing: its lists, buffers, colour table, command lines and
matrices go; its transform array and Talairach matrices only when `free_transform` is set
(otherwise they are someone else's). Freeing a surface destroys it and leaves "no surface".

**The dumps** (to a given stream; each `%f` is C's six-decimal form of the float widened to double):
- a vertex `v`: `vno:<v> ripflag:<r> marked:<m> nsizeCur:<c> nsizeMax:<x> vnum:<a> v2num:<b>
  v3num:<c3> vtotal:<t> nsizeCur:<c> nsizeMax:<x> x:%f y:%f z:%f`, then for each `i` below the
  vertex's size in use (§5's `s`) ` [<i>] v:<neighbour> dist:%f dist_orig:%f` (`-1.000000` for
  absent distances), a newline, then for each face around it ` [<i>] f:<face> n:<position>`, a
  newline (`ripflag` as a signed number; a neighbour list shorter than `s` is outside the
  behaviour);
- a face `f`: `fno:<f> ripflag:<r> marked:<m>` and a newline, ` [0] v:<a> [1] v:<b> [2] v:<c>` and
  a newline;
- the shape: `mrisDumpShape {`, `nvertices:<n> nfaces:<n> max_nsize:<n> nsize:<n>`, the first ten
  vertices' and the first ten faces' dumps, `} // mrisDumpShape`, each on its line.

## 31. Rip flags and the arrays

- **Exporting** the faces' (vertices') rip flags: a new list of the in-use faces' (vertices')
  `ripflag`; a negative count gives no list, a count of 0 an empty one. **Importing** sets each
  in-use face's (vertex's) `ripflag` from a list of at least that many.
- **Counting** the ripped vertices: the in-use vertices whose `ripflag` is not 0. **Storing** the
  rip flags copies each in-use vertex's and face's `ripflag` into `oripflag`; **restoring** copies
  back; **unripping** sets every in-use `ripflag` to 0. Each returns 0 (but the count).
- **The arrays** (new lists of `3 × count` numbers): the in-use vertices' `x, y, z`; their normals
  `nx, ny, nz`; the in-use faces' three vertex numbers; the faces' cached normals, each read as
  "read the normal" of §27 (a deferred face's normal computed first). A negative count is outside
  the behaviour (the official program throws `std::bad_array_new_length`).

## 32. The curvature names and the registration's scales

Process-wide: three surface names (`inflated`, `smoothwm`, `smoothwm`) and three curvature names
(`inflated.H`, `sulc`, none; §12 changes them too).
- **Setting the curvature name** `k` (0 to 2) to a copy of a name (up to its first NUL) returns 0;
  `k` above 2 prints `ERROR: MRISsetCurvatureName() nth=<k> > 2` and a newline on standard output
  and returns 1; a negative `k` is outside the behaviour (FreeSurfer's callers give 0, 1 and 2).
- **Printing the curvature names** on standard output (whatever stream is given): for `k` 0 to 2,
  `<k> <curvature name>` when it exists, else `<k> <surface name> (computed)` when that exists,
  else nothing; each with a newline. Returns 0.
- **The registration's blurring scales**: four floats (4, 2, 1, 0.5) and their count, a `double`
  (4). Setting them to a caller's list and count makes the list the scales (the official program
  keeps the caller's list itself, so later changes the caller makes to it show; FreeSurfer's
  callers make none) and the count, widened, the count; setting "none" restores the four defaults.
  Returns 0.

## 33. Extra gradients and the NaN check

- **Allocating the extra gradients**: when the surface has none, three lists of one float per
  in-use vertex, all 0 (empty with no vertex); with a negative vertex count the allocation fails:
  `error: Cannot allocate memory`, `error: MRISallocExtraGradients: could allocate gradient
  vectors` (with FS-01's NUL before the newline under `reproduce`) and the end of the program
  (`ERROR_NO_MEMORY`, exit status 254). Returns 0.
- **The NaN check**: for each in-use vertex, `odx`, `ody`, `odz`, `dx`, `dy`, `dz` in that order;
  the first NaN (exponent all ones, fraction not 0) is a failed assertion
  `<BUILD_ROOT>/utils/mrisurf_base.cpp:1587 !devIsnan(f)`.

## 34. Column printing and the progress bar

All on standard error, flushed; widths count bytes; texts end at their first NUL.
- **A status line** (`left`, `right` texts): `left` right-aligned in 50 bytes (printed only when
  not empty), then ` [ <right> ]` and a newline right-aligned in 30 bytes (only when `right` is not
  empty).
  - FS-119 (corrected by default, rule A): the official program first writes ` [ <right> ]` and the
    newline into a 16,384-byte buffer the official build checks: a `right` of 16,378 bytes or more
    prints only `*** buffer overflow detected ***: terminated` and a newline, and ends with signal 6.
    larmorx prints the line whole, noting FS-119; `reproduce` gives the abort (exit status 134).
- **A number line** (an integer, or a float as `%f`): `left` right-aligned in 50 bytes (a single
  blank right-aligned when `left` is empty), then ` [ <number> ]` and a newline right-aligned in 30.
- **The progress bar** at vertex `v` of a loop over the surface's `N` vertices: `p` = the integer
  part of `0.05 × N` (in double); at `v` = 0 the message right-aligned in 50 bytes (when there is
  one) and ` [`; then a `#` when `v mod p` (C's remainder) is `p - 1`; then, at `v = N - 1`, `] `
  and, with a message, `[ ok ]` and a newline. Returns 1.
  - FS-118 (corrected by default, rule A): with `p` 0 (fewer than 20 vertices) the official program
    divides by zero and ends with signal 8 at its first call, after what `v` = 0 prints. larmorx
    prints no `#` then, noting FS-118; `reproduce` ends with exit status 136.

## 35. Vector searches, the sign, the sort comparisons

- **Searching a column vector** (a float matrix of `r` rows) for a value `t`: "first not equal"
  returns, as a signed 16-bit number, the 0-based index `i` of the first row whose element `(i + 1,
  1)` is not equal to `t` (a NaN is never equal), 0 when every element equals it (so a first-row
  difference and none both give 0, although FreeSurfer's comment promises the 1-based index); "first
  equal" returns the 1-based row of the first element equal to `t`, 0 when none.
- **The sign** of a float: -1 when it is below 0, else 1 (NaN: 1).
- **Comparing two vertices for sorting** (a vertex number and `x y z`): by `y`, then `x`, then
  `z`: -1 when the first's is below, 1 when above, and 1 when every pair is neither (never 0).
  **Comparing two faces** (a face number and three vertex numbers): by the first, second, third
  vertex number likewise, 1 for equal faces.

## 36. Whether two edges cross on the sphere

Two edges (vertex number pairs `(p1, p2)`, `(q1, q2)`) of a surface, on the canonical coordinates
`cx, cy, cz` (each float widened to double; every operation (d) unless marked (f)). The result is 0
or 1.

- When the edges share a vertex number (either end with either end): 0.
- `u0 = p1`, `u1 = p2`, `u2 = q1`, `u3 = q2`; `n0 = u0 × u1`, `n1 = u2 × u3` with `(a × b)_0 =
  a1⊗b2 ⊖ b1⊗a2`, `(a × b)_1 = a2⊗b0 ⊖ b2⊗a0`, `(a × b)_2 = a0⊗b1 ⊖ b0⊗a1`; the dot product `a·b =
  (a0⊗b0 ⊕ a1⊗b1) ⊕ a2⊗b2`; `a0 = u0·n1`, `a1 = u1·n1`, `a2 = u2·n0`, `a3 = u3·n0`.
- When any of `|a0|`, `|a1|`, `|a2|`, `|a3|` is below `1e-7` (a double), the **stable form**
  decides instead: `og0 = (p1 ⊕ p2) ⊘ 2` with the sum (f) of the two floats, then widened, divided
  (d); `n0 = og0 × (p2 ⊖ p1)` with the difference (f), widened; likewise `og1`, `n1` from `q1, q2`;
  `u0 = p1 ⊖ og1`, `u1 = p2 ⊖ og1`, `u2 = q1 ⊖ og0`, `u3 = q2 ⊖ og0` (d); `a0..a3` as before; then
  the decision below with these values.
- **The decision**: `a = a0⊗a1`, `b = a2⊗a3`. When `a > 0`: 0. When `a < 0`: 0 when `b > 0`, else
  1. Otherwise (0 or NaN): 0 when `b > 0`, 1 when `b < 0`; otherwise `u = ((u0 ⊕ u1) ⊕ u2) ⊕ u3`
  (each coordinate), `n2 = u × n0`, `c_k = u_k·n2` for `k` 0 to 3, and the result is whether
  `max(c2, c3) >= min(c0, c1)` and `min(c2, c3) <= max(c0, c1)`, with `min(x, y)` = `x < y ? x :
  y` and `max(x, y)` = `x > y ? x : y` (so a NaN picks the second argument). Only comparisons leave
  the computation: a NaN's bits never matter.

## 37. Validation plan, part 6

The probe's commands `life`, `meta`, `misc` and `edges` (each a black-box run case by case):
- `life`: a surface made by the plain or the room form (counts 0 to 6 and 0 to 4, rooms to 8 and
  6, a negative count one case in 40), then 1 to 8 of: changing the vertex room, the face room
  (room 0 one time in 15), both, both by counts, the six count operations, acquiring and releasing
  the frozen count, setting a vertex's coordinates, motion and normals (any floats, specials
  included), rip flag and mark, its neighbourhood lists, making its distances (filled with `i + 1 +
  10 v`), setting a face, its cache entry and byte, exporting, importing (flags `(7i + s) mod 3 - 1`
  for faces, `(5i + s) mod 3 - 1` for vertices), counting, storing, restoring, unripping, the three
  plain arrays, the face-normal array, the extra gradients, the NaN check, the three dumps; the
  generator follows the counts so that about one operation in 15 breaks an assertion; then the
  state (the counts, rooms, frozen count, the ring sizes, `dist_alloced_flags`, which arrays exist,
  every vertex place's coordinates, flags, neighbourhood counts and distances, every face place's
  corners, flags, cache entry and byte) and the free. An operation that would read or write past what
  the surface holds prints `G` and is skipped.
- `meta`: a source (counts, kinds, hemisphere, `free_transform`, every geometry number, a name of
  up to 20 printable bytes, 0 to 3 transforms of a type, the two matrices or not) copied into a new
  surface or into a given one (made the same way); the result's state.
- `misc`: 1 to 6 of: setting a curvature name (`k` 0 to 4), printing them, setting the scales (or
  the defaults) and printing the count and the scales, a status line (texts of 0 to 60 bytes with
  NULs, backslashes and high bytes now and then, a `right` of 16,376 to 17,000 bytes one time in
  40), the two number lines, the progress bar over a range of vertices of a surface of 0 to 120 (one
  in three below 20), the two searches (0 to 6 rows, half of them the value), the sign, both sort
  comparisons both ways (with equal coordinates and numbers often).
- `edges`: 4 to 6 vertices whose canonical coordinates are on a grid of quarters (zeros and
  collinear points: the decision's last branch), in steps of 1e-5 (the stable form) or any float;
  1 to 6 pairs of edges (shared vertices often).

Measured 2026-11-13 (relay 9): seed 17, 1,000 cases of each command: identical with `reproduce`;
seed 19, 1,000 `life` cases: 170 failed assertions (FS-03), 40 with FS-117, 387 with a skipped
operation; 1,000 `misc` cases: 165 FS-118 ends, 7 FS-119 ends; the default differs only with those
notices. The probe's whole run and the seed-1 hash: the probe's README.

## 38. Computing the metric properties: the three routes

**Computing the metric properties** of a surface (its normals, areas, angles, bounding box,
neighbour distances and their mean) is chosen by the environment at every call (no FreeSurfer
script sets these variables):

- `FS_FASTER_MP` set: the **faster computation** (§44); nothing is counted.
- Otherwise the call is **counted** (a process-wide count from 1, printed by §43's lines), then:
  - neither `FREESURFER_OLD_MRIScomputeMetricProperties` nor
    `FREESURFER_NEW_MRIScomputeMetricProperties` set, or only the second: the **default
    computation**: the distances (not the original ones) are freed (§15), a **copy** of the
    surface's values is taken (§39), the computation runs on the copy (§40-42) and the copy is
    **written back** (§43);
  - only the first set: the **old computation** on the surface itself (§40-42 with the surface's
    own fields, §42's old form);
  - both set: the copy is taken first, the copy's computation and then the old one run, and the copy
    is **compared** with the surface instead of written back (§43): the surface keeps the old
    computation's results.

Returns 0. Each computation in order: the vertex normals and areas (§40, which may move degenerate
vertices), the bounding box (§16's, from 10000/-10000), the distances at `nsize` hops (§20's
computing; `dist_nsize` becomes `nsize`; the copy's distances are made in the copy, §39), the faces'
areas, normals and angles and the vertices' areas from them (§41), `avg_vertex_area = total_area ⊘
nvertices` (f, the count converted to float; `nvertices` 0 gives an infinity or a NaN), the mean
neighbour distance and its deviation (§41), the orientation (§42); last, for a parameterized sphere
(5), a rigid body (6) or a sphere (4), `total_area = ((π ⊗ radius) ⊗ radius) ⊗ 4` (d, rounded to
float; π the double nearest).

## 39. The copy

The copy holds: the kind, the original coordinates' kind, the counts, `nsize`, `radius`,
`dist_nsize`; each face's three vertex numbers, rip flag, cached original area (read through §27's
"read the normal": a deferred face is computed then) and original angles, and a "normal set" flag
(cleared); each vertex's rip flag, size in use (§5), white and pial coordinates, white normal,
current coordinates, border flag, canonical coordinates, curvature, original area and face number,
and empty distances (capacity 0). Its normals and face areas start as the surface's (FS-121 below),
its vertex areas, a plane's vertex signs and the faces' normals and angles as 0.

The copy's distances: **making** a vertex's distances in the copy gives it room for the larger of
its capacity and its size in use: the buffer it has when large enough; else that buffer grown (its
values kept); else the surface's stored buffer for that vertex when the surface's distances of that
vertex are free (§15: the values it holds kept up to the shorter length, the surface's storage then
empty), else a new buffer; the new capacity is the larger value. Values past those written are
outside the behaviour.

## 40. The vertex normals and areas

**Faces first:** each ripped face makes its three vertices border vertices (`border` 1); each other
face with at least one unripped vertex gets as its normal (the copy's; the surface's cached normal
in the old computation, §27's "set the normal") the cross product of `a = p2 ⊖ p0` and `b = p1 ⊖ p2`
(f, `p0 p1 p2` its corners in order): `((a.y ⊗ b.z) ⊖ (b.y ⊗ a.z), (b.x ⊗ a.z) ⊖ (a.x ⊗ b.z),
(a.x ⊗ b.y) ⊖ (b.x ⊗ a.y))`.

**Then the vertices,** in up to five rounds (round `t` = 0 to 4) over a list that starts as every
unripped vertex in order. For a vertex, over its faces in its list order (`f[n]`, the vertex at
corner `c = n[n]` of face `f[n]`), skipping ripped faces, with `q0`, `q1`, `q` the corners `c - 1`,
`c + 1`, `c` (cyclically): its **corner normal**, the cross product above of `a = q ⊖ q0` and `b = q1
⊖ q` each first made unit length (§6's normalisation: `len = sqrt((x⊗x ⊕ y⊗y) ⊕ z⊗z)` (f), divided
when `len > 0`), and that product made unit length too while `UnitizeNormalFace` is set (1, the
default); and its **corner area**, half the length of the same cross product of the unnormalised
`a` and `b`: `sqrt((d1⊗d1 ⊕ d2⊗d2) ⊕ d3⊗d3) ⊘ 2` (f). The vertex's normal is the sum of its corner
normals (`s ⊕= n` per component, from 0) and its area the sum of its corner areas (f, from 0). When
the vertex has no unripped face, or the normal's length (§6, with the normalisation applied) is
`> 0`: the area becomes `area ⊘ 3` (with `fix_vertex_area`, the default; else `⊘ 2`; in double,
rounded to float), the original area becomes it too when it is negative, and the normal is stored
(unit length; zero for no face). Otherwise the vertex is kept for the next round.

After a round with vertices kept, in ascending vertex order: with `DIAG_VERBOSE` set and the kind a
spherical patch, a parameterized sphere or a sphere, `vertex <vno>: degenerate normal` on standard
error; the vertex's `x`, then `y`, plus `h(t, vno, k)` with `k` a counter from 0 for that vertex
(f); then, unless the kind is a plane (2) or a patch (1), its `z`, and for each of its 1-hop
neighbours (with `DIAG_SHOW` and `DIAG_VERBOSE` set, `   k=<vno %5d> <n> nbr = <nbr %5d> / <vnum>`
on standard output) the neighbour's `x`, `y`, `z` plus the next three `h`, and its unripped faces'
normals recomputed as above. The next round retries only these vertices (not their neighbours). A
vertex still kept after the fifth round keeps the normal it had (the default computation: FS-121).

`h(i, k, c)`: the FNV-1a hash of the 12 bytes of the three unsigned 32-bit integers `i`, `k`, `c`
(little-endian; 64-bit arithmetic: from `0x811c9dc5`, per byte `hash = (hash xor byte) × 16777619
mod 2^64`), `f = (hash mod 2^32) / 4294967295` (d), the move `(0.002 × f) − 0.001` rounded to float
(d; only numbers are involved).

## 41. Faces' areas, normals and angles; the mean distance

For each unripped face (`p0 p1 p2` its corners), with `a = p1 ⊖ p0`, `b = p2 ⊖ p0` (f):
- `n = ((b.z ⊗ a.y) ⊖ (a.z ⊗ b.y), (a.z ⊗ b.x) ⊖ (a.x ⊗ b.z), (a.x ⊗ b.y) ⊖ (a.y ⊗ b.x))`;
- the **area** `sqrt((n.z⊗n.z) ⊕ ((n.x⊗n.x) ⊕ (n.y⊗n.y))) ⊗ 0.5` (f);
- the **normal** `n ⊗ s` per component with `L = sqrt(((n.y⊗n.y) ⊕ (n.x⊗n.x)) ⊕ (n.z⊗n.z))`, `s = 1`
  when `|L| < FLT_EPSILON` else `1 ⊘ L`; its third component `n.z ⊗ s` in the old computation and `s
  ⊗ n.z` in the copy's (the first two `n.x ⊗ s`, `n.y ⊗ s`); stored as the face's normal (the copy's;
  the cached one in the old computation);
- the **angle at corner `k`** (0, 1, 2), with `o` the corner, `u` and `w` the corners `(p2, p1)`,
  `(p0, p2)`, `(p1, p0)` for `k` = 0, 1, 2, `A = u ⊖ o`, `B = w ⊖ o` (f): `fast_atan2(T, D)` with
  `T` the triple product `(B x A) . normal` as `specs/freesurfer-maths.md` defines it for three
  3-vectors (its `v1 = B`, `v2 = A`, `v3 = normal`) and `D = (A.z ⊗ B.z) ⊕ ((A.x ⊗ B.x) ⊕ (A.y ⊗ B.y))`
  (f); `fast_atan2` is the table arctangent of `specs/freesurfer-core.md` (the C library's `atan2f`
  with `FREESURFER_fastApproxAtan2f_use_atanf2` set; its table read with a NaN or two infinities is
  FS-06).

The **total area** is the reproducible sum (below) of the unripped faces' areas in double (each
term added as `area ⊕ partial`), rounded to float. Each unripped vertex's **area** is the float sum
(from 0, `acc ⊕ area`) of its unripped faces' areas, then `⊘ 3` (or `⊘ 2`) as §40.

**The mean neighbour distance**: over each unripped vertex and each of its first `vnum` neighbours
that is unripped, its distance `d` widened, the reproducible sums `S ⊕= d`, `S2 = (d ⊗ d) ⊕ S2`,
`N ⊕= 1` (d); `avg = S ⊘ N`, `std = sqrt(((S2 ⊘ N) ⊖ (avg ⊗ avg)) ⊗ N ⊘ (N ⊖ 1))` (d); the surface's
`avg_vertex_dist` and `std_vertex_dist` (no pair: 0/0, a NaN).

**Reproducible sums**: the index range `0..count` is cut into `m = min(count, 128)` consecutive
ranges, the `i`-th (from 0) holding `(remaining) / (m - i)` indices (integer division, `remaining`
the indices not yet given); each range is summed from 0 in index order, then the ranges' sums are
added in order to the variable's start value (0), the accumulator on the left. The result does not
depend on the number of threads.

## 42. The orientation

For a rigid body (6), parameterized sphere (5), sphere (4), ellipsoid (3) or spherical patch (7)
(never an unoriented sphere, 8, nor any other kind): each unripped face whose normal points inwards,
`(((x0 ⊕ x1) ⊕ x2) ⊗ n.x ⊕ ((y0 ⊕ y1) ⊕ y2) ⊗ n.y) ⊕ ((z0 ⊕ z1) ⊕ z2) ⊗ n.z < 0` (f), gets its area,
normal and three angles negated (the sign bit flipped, also of a NaN, which stays signalling); then
the reproducible sums (d, each term on the left: `term ⊕ partial`) over the unripped faces: of the
non-negative areas into `total_area`, of the negated negative ones (the area negated as a float,
then widened) into `neg_area`, of their cached original areas into `neg_orig_area` (each rounded
to float).

For a plane (2): each unripped face whose normal's z is negative gets its area, normal and angles
negated (sign flips); the float sums from 0 over the unripped faces: `total_area = area ⊕ total`
for a non-negative area, else `neg_area = neg_area ⊖ area` and `neg_orig_area = neg_orig_area ⊕
orig_area` (the cached original area). Then each unripped vertex: a negative normal z is negated
(sign flip) and `neg` becomes 1, else `neg` 0; its area becomes the float sum (`acc ⊕ area`) of the
areas of **all** its faces, ripped ones included, then `⊘ 3` (or `⊘ 2`). A ripped face's area is the
surface's (the copy's is the one it started with, FS-121).

The **old computation's** forms of the face, normal and orientation steps are the same on the
surface's fields and cached normals (each read through §27's "read the normal").

## 43. Writing the copy back, and the comparison lines

**Writing back** (not checking): `avg_vertex_dist`; a comparison (below) of `1` with whether each
of the kind, the original coordinates' kind, the two counts, `nsize`, `radius` and the topology are
equal (`no:-1  1  1.000000 != 0.000000 ...` when not: a NaN radius, FS-122); `dist_nsize`, `xlo`,
`xhi`, `ylo`, `yhi`, `zlo`, `zhi`, `xctr`, `yctr`, `zctr`, `total_area`, `avg_vertex_area`,
`std_vertex_dist`, `neg_orig_area`, `neg_area`. Then each vertex not ripped (in the copy and the
surface): a plane's sign; its distances moved into the surface with the copy's capacity (§15's
"setting the distances": a capacity below the surface's is the failed assertion
`<BUILD_ROOT>/utils/mrisurf_base.cpp:334 newCapacity >= v->dist_capacity`); comparisons of `v->ripflag`,
`v->whitex` ... `v->whitez`, `v->pialx` ... `v->pialz`, `v->wnx` ... `v->wnz` with the copy's (equal
unless a NaN: FS-122); `x y z border cx cy cz curv origarea fno area nx ny nz` copied. Each face not
ripped: its cached original area set (bit 2 cleared) and original angles set from the copy; its
cached normal set when the copy set one; its `area` copied. **The faces' angles are not copied**
(FS-120): they keep what the surface held.

**Comparing** (both routes ran): the faces' vertex numbers (`0` against the difference of the first
differing bytes of the two triples, as `memcmp`), `mris->avg_vertex_dist`, the inputs as above;
`mris->dist_nsize`, `mris->xlo` ... `mris->neg_area` against the copy's; per vertex `v->neg`, the
first `vtotal` distances (`mris_dist[n]`; `vtotal` above the surface's capacity is the failed
assertion `<BUILD_ROOT>/utils/mrisurf_metricProperties_fast.h:390 vtotal <= v->dist_capacity`), the
inputs, then `v->x` ... `v->nz`; per face `fNorm->orig_area`,
`cmpAnglesPerTriangle(f->orig_angle,mp->f_orig_angle[fno])` (the byte difference of §4 against 0),
`fNorm->nx`, `fNorm->ny`, `fNorm->nz` (when the copy set a normal), `f->ripflag`, `f->area`; nothing
is written.

**A comparison** of `lhs` and `rhs` (doubles) prints, when `lhs != rhs`, on standard output `no:<no>
<expr>  <lhs> != <rhs>  during count_MRIScomputeMetricProperties_calls:<calls>` (two spaces around
the expression and before `during`; `%f` numbers; `<no>` the vertex or face number or -1), at most
11 per part (a count of the differences, the vertices' and the faces' parts each from 0); the first
line of the process is followed by `b <BUILD_ROOT>/utils/mrisurf_metricProperties_fast.h:335
needed`.

- **FS-120** (kept by default, rule D; the orchestrator's decision of 2026-11-13): the default
  computation leaves the faces' angles stale, and larmorx's default does the same until the effect
  on `mris_sphere`'s and `mris_register`'s results is measured with those programs' ports. The
  correction is on request: with it and `fix`, the copy's angles are written back (as the other two
  computations set them), with a notice when an angle changes; `reproduce` always leaves them.
  Validation: the default and `reproduce` give FreeSurfer's angles; with the request, a later
  read of the angles (a dump right after the computation) shows the copy's.
- **FS-121** (both modes): what the copy never computes is memory contents in FreeSurfer: the normal
  of a vertex kept after the fifth round (copied into the surface) and a ripped face's area in a
  plane's vertex areas. larmorx gives such a vertex the normal it had and such a face the surface's
  area, in both modes, with a notice; these values are outside what can be compared.
- **FS-122** (corrected by default, rule A): a comparison of two NaNs with the same bits prints
  nothing in larmorx; `reproduce` prints FreeSurfer's line.

## 44. The faster computation (`FS_FASTER_MP`)

The vertices' positions and rip flags and the faces are taken from the surface; every vertex's
distances (ripped too) are made (§15: zeros of the size in use); each ripped face makes its vertices
border vertices. `FS_WEIGHTS_NORMS_BY_ANGLE` set: the vertex normals are weighted by the corner
angles. The fix value is 3 with `fix_vertex_area` set, else 2.

**Normals, areas, angles,** in up to five rounds (`t` = 0 to 4): the total (d) from 0; every vertex
area and normal 0; each unripped face (`p0 p1 p2`): `a = p1 ⊖ p0`, `b = p2 ⊖ p0`, `n = ((a.y ⊗ b.z)
⊖ (a.z ⊗ b.y), (a.z ⊗ b.x) ⊖ (b.z ⊗ a.x), (a.x ⊗ b.y) ⊖ (a.y ⊗ b.x))`, `L = sqrt(((n.x⊗n.x) ⊕
(n.y⊗n.y)) ⊕ (n.z⊗n.z))`, the normal `n ⊘ L` per component when `L > 0` (else `n`), the area `L ⊗
0.5`, the total `area ⊕ total` (d); the angle at corner `k` with `(A, B)` = `(p2 ⊖ p0, p1 ⊖ p0)`,
`(p0 ⊖ p1, p2 ⊖ p1)`, `(p1 ⊖ p2, p0 ⊖ p2)`: `c = B x A` in the form of `n` (with `B` first),
`fast_atan2((c.z ⊗ m.z) ⊕ ((c.y ⊗ m.y) ⊕ (c.x ⊗ m.x)), ((B.y ⊗ A.y) ⊕ (A.x ⊗ B.x)) ⊕ (B.z ⊗ A.z))`
(`m` the face's unit normal). `total_area` becomes the total (rounded). Each unripped vertex: over
all its faces (ripped too), area `acc ⊕ face area`; normal `acc ⊕ face normal` per component, or
with the weights `(fn.x ⊗ w) ⊕ acc`, `(fn.y ⊗ w) ⊕ acc`, `(w ⊗ fn.z) ⊕ acc` (`w` the face's angle at
the vertex's corner); area `⊘` the fix value (f); `len = sqrt((z⊗z) ⊕ ((x⊗x) ⊕ (y⊗y)))`: when `len >
0` the normal `⊘ len`, else the vertex is **bad**: its area becomes -1. With no bad vertex the
round loop ends. Otherwise, in vertex order, each bad vertex moves (below) with a counter from 0, then
each of its 1-hop neighbours with the same counter going on; the vertices are marked as moved and
the next round runs (after the fifth round the loop ends without recomputing: FS-124 below).
**Moving** vertex `v`: `x ⊕= h(t, v, k)`, `y ⊕=` the next, and `z ⊕=` the next for a surface other
than a plane or a patch, else `z ⊕= 0` (`+0`, which turns `-0` into `+0`).

**The bounding box** from the first vertex's position (low and high ends), then every other vertex
as §16's; the centre `0.5f ⊗ (float)((double)lo ⊕ (double)hi)`.

**The distances** of each unripped vertex to its first `vtotal` neighbours (no assertion on the
kind): a sphere (4) or parameterized sphere (5): §20's sphere formula (the zero count of §20 and its
logic problem are not part of this computation); any other kind: `sqrt((zd⊗zd) ⊕ ((xd⊗xd) ⊕
(yd⊗yd)))` (f). **The mean distance** and its deviation as §41.

`avg_vertex_area` as §38; **the orientation** for the kinds of §42's first paragraph: a face points
inwards when `((((x0 ⊕ x1) ⊕ x2) ⊗ n.x) ⊕ (((y0 ⊕ y1) ⊕ y2) ⊗ n.y)) ⊕ (((z0 ⊕ z1) ⊕ z2) ⊗ n.z) < 0`;
for a plane (2), when the normal's z is negative; such a face's area, normal and angles are negated
(sign flips); the reproducible sums over the unripped faces: `total_area = area ⊕ partial` for a
non-negative area, else `neg_area = neg_area ⊖ area` (the accumulator first) and `neg_orig_area =
orig_area ⊕ partial` (the cached original area); a plane's unripped vertices then as §42 (the
normal's z sign-flipped with `neg`; the area the float sum over all its faces `⊘` the fix value).
The sphere's area of §38.

**Written back:** each unripped vertex's position (when moved), normal and area, and its original
area `|area|` when negative; each unripped face's cached normal, angles and area; `dist_nsize`
becomes `nsize`.

- **FS-123** (both modes): FreeSurfer adds memory contents for a ripped face to its vertices' areas
  and normals, and takes the bounding box of a surface with no vertex from memory it does not own;
  larmorx counts a ripped face with area and normal 0 (the sums as without it) and gives an empty
  surface §16's box (10000 low, -10000 high), with a notice.
- **FS-124** (kept, rule C): a vertex still bad after the fifth round has area -1 (its original area
  1 when it was negative) and a normal of no length.
- **FS-125**: with several threads FreeSurfer's total area of the face loop depends on the order the
  threads finish; larmorx sums the faces in order (one thread's value).

## 45. Callers that compute the metric properties

- **Smoothing a surface** `niter` times by `step`: §18's smoothing, then the metric properties.
- **Disturbing the vertices** by `amount`: the distances freed; each vertex's `x`, then `y`, plus
  `randomNumber(-amount, amount)` (`specs/freesurfer-core.md`'s generator; `FREESURFER_SEED`), in
  double rounded to float; then the metric properties.
- **Recentring** with the sets `move` and `target`: the metric properties; `radius = sqrt(total_area
  ⊘ 4π)` (d); the current positions saved into `move` and restored from `target` (§25); §19's centring;
  `rescaling canonical coordinates to use r=<radius %2.1f>` on standard output; each unripped
  vertex's current coordinates `radius ⊗ (x ⊘ r)` (d, rounded), `r` the length (d) of its `target`
  coordinates; `t = Σ (target ⊖ move)` (d) over the unripped vertices divided by their count; each
  unripped vertex's canonical coordinates `c ⊕ t` (d, rounded), or, for a target other than the
  canonical set, the fatal error `MRISrecenter: movable vertices <move> not supported` (code -3) at
  the first unripped vertex; the current positions restored from `move`; the metric properties.
- **Scaling to the original area**: the surface scaled about its box's centre (§16's anisotropic
  scaling, the same factor on each axis) by `sqrt(orig_area ⊘ (total_area ⊕ neg_area))` (f); the
  metric properties. Returns 0. **Scaling the coordinates** by `s`: §16's scaling. **Scaling the
  brain** by `s`: §16's anisotropic scaling by `s` on each axis.

## 46. Validation plan, part 7

The probe's `mprop` command: a surface of 0 to 10 vertices (none one case in 20) with a mesh of up
to `2 nv` random faces (three distinct corners, a repeated one in one face in ten), each vertex's
faces and corners from the faces (one vertex in 15 lists one more face at a random corner, never
with the weights), its 1- to 3-hop rings in order as neighbours (`vnum`, `v2num`, `v3num`
cumulative, `vtotal` one of them, `nsizeMax` not above it), the positions on a rough sphere (1/16
steps; one vertex in eight at another vertex's place; specials now and then), the original
coordinates mostly the current ones, the other sets, normals, curvature and areas any (the original
area -1 one case in three), border, sign, face number and rip flags; the distances in every state of
§17 (a buffer at least the size in use when there is one); each face's rip flag, area, angles,
original angles, cache entry and byte; the kinds (all ten, an invalid one in 25), `nsize` 1 to 3
(0 or 4 one in 30), `dist_alloced_flags`, `dist_nsize`, `radius`, `total_area`, `avg_vertex_dist`;
the switches (old alone one case in 12, old and new one in 12, the faster one in 6 with the weights
half the time, new alone rarely; the table arctangent's `atan2f` one in 15, one in 3 for the
NaN-heavy third of the cases; `FREESURFER_reportLogicErrors` one in 4), `DIAG_SHOW`+`DIAG_VERBOSE`
one in 6, `UnitizeNormalFace` 0 one in 4, `fix_vertex_area` 0 one in 3; then 1 to 4 of: computing
the metric properties (most often), the triangle properties, the normals, the mean distance (with
and without its deviation; skipped with `G` where a vertex would read distances it does not have),
the ellipsoid's and the surface's and the plane's orientation, the bounding box, and §45's callers;
then the state (each vertex's position, normal, area, original area, canonical x, curvature,
border, sign, face number and distances; each face's area, angles, original angles, cache entry and
byte; the box, the areas, the mean distance and deviation, `dist_nsize`, `dist_alloced_flags`).
Cases where larmorx notes FS-121, FS-123 or FS-06 (FreeSurfer reads memory it never wrote or does
not own) are counted, not compared.

# Part 8: the small file readers and writers (queue item 3.1-47 part b)

Every function here takes a file name relative to the program's working directory (or absolute).
A file that cannot be opened sets the C library's `errno` (Linux's numbers), which the error message
that follows prints first (`specs/freesurfer-core.md`'s errors: `error: <strerror text>` then
`error: <message>`, the message ending in a NUL byte and a newline, FS-01); `errno` stays set
afterwards, so a later error of the same process prints it again. Numbers in binary files are
big-endian: a **2-byte number** is an `int` clamped to [-32767, 32767] then written as a `short`;
a **3-byte number** the low 24 bits of an `int` (-1 is `ff ff ff`), read back as an unsigned
24-bit number; a **4-byte integer** an `int`; a **float** its four bytes. A number a file ends
before is read as specified in `specs/freesurfer-io.md` (FS-167: the missing bytes are memory
contents in FreeSurfer). `<n>` in a message is `%d`; `%f` six decimals; the text formats use the C
library's `printf` (glibc 2.35: correctly rounded decimals, `nan`, `-nan`, `inf`, `-inf`).

**Names of 1,024 bytes or more (FS-126, FS-134).** FreeSurfer builds or copies some names into
1,024-byte buffers. Where it **copies** a whole given name (the area-error file's name; the
distance file's name when it holds a `/`), a name of 1,024 bytes or more ends the program with
`*** buffer overflow detected ***: terminated` on standard error and signal 6. Where it **builds**
a name from parts (the triangle-property files, the distance file without a `/`), a result of
1,024 bytes or more is cut to its first 1,023 bytes, after the message `<function>: Truncation on
line <line>` and a newline on standard error (the functions and lines below), and the cut name is
opened. Where it takes the **folder** of a surface name of 1,024 bytes or more (the
triangle-property files), it writes past a 1,024-byte buffer: what follows depends on the build
(observed: the cut name of the whole result, and for names of about 2,000 bytes `*** stack smashing
detected ***: terminated` and signal 6 when the function returns). larmorx takes every name whole
(FS-126, FS-134 noted); with `reproduce` it aborts and cuts as FreeSurfer does (the folder's
overrun: the cut of the whole result, no abort).

## 47. The frame `MRISreadValues` reads

A process-wide frame number, -1 at the start. **Setting** it stores the number. **Getting** it:
the stored number when it is 0 or more; else the environment variable `MRIS_READVAL_FRAME` read as
`sscanf("%d")` (glibc's: leading white space skipped, an optional sign, digits; a number beyond the
`int` range is the C library's `long` result cut to 32 bits (`99999999999` gives `1215752191`,
`errno` `ERANGE`); no digits leave 0); 0 when the variable is unset.

## 48. The triangle-property files

The **hemisphere** of a surface name is the two bytes before its last `.` (a name without a dot:
`ERROR_BADPARM` (-4) after the error `<function>(<name>): could not scan hemisphere from fname`,
the writer's text `could not scanhemisphere from fname`, without the space). **FS-129** (rule B):
when the last dot is the name's first or second byte FreeSurfer reads the hemisphere from the
memory before the name; larmorx refuses such a name with the same error; `reproduce` takes the
bytes before the name as 0 (an empty hemisphere: `<folder>/.triangle_area`). The **folder** is the
name up to its last `/` (`/name` gives the empty folder, a name without `/` gives `.`). The two
files are `<folder>/<hemi>.triangle_area` and `<folder>/<hemi>.triangle_angle` (cut at 1,023 bytes
with `MRISreadTriangleProperties: Truncation on line 126` and `... line 174`, or
`MRISwriteTriangleProperties: Truncation on line 236` and `... line 254`, FS-126).

**Writing** (`MRISwriteTriangleProperties`): first the triangle properties are computed (§41:
every face's area, normal and angles, the vertices' areas, the total area), then the name is
checked, then the area file is created (`wb`; failing: `ERROR_NO_FILE` (-1) after
`MRISwriteTriangleProperties: could not open <file>`): the vertex count and the face count (4-byte integers), each face's area (floats); then
the angle file likewise: the two counts, then three floats per face. **FS-128** (rule A):
FreeSurfer writes, for every face, the angles of the place after the last face: a spare face place
of the surface when it has more places than faces (whatever angles that place holds), memory past
the faces otherwise; larmorx writes each face's own three angles; `reproduce` writes the spare
place's (0 when there is none, noted). A surface without faces writes the counts only. Returns 0.

**Reading** (`MRISreadTriangleProperties`): with `DIAG_SHOW` and `DIAG_VERBOSE` both set,
`reading triangle files...` (no newline) on standard output first. The name is checked; the area
file opened for reading; when it cannot be, `\nno precomputed triangle areas and angles -
computing...\n` on standard output and the result 1 (nothing changed). The two counts: **FS-127**
(rule A): FreeSurfer reads each as a float and converts it to an integer (towards 0; NaN and values
beyond the `int` range give -2147483648), so the counts its writer writes (integers) read as 0 (a
denormal float) and every file of a surface with vertices is refused; larmorx reads them as the
integers the writer writes; `reproduce` as floats. A vertex count other than the surface's:
`ERROR_NOFILE` (-1) after `MRISreadTriangleProperties: incompatible vertex number in file <file>`
(the file closed). **FS-133** (rule B): FreeSurfer then reads as many faces as the file's count,
into the surface's spare face places and past its arrays (memory corruption) when the count exceeds
its faces; larmorx refuses a face count other than the surface's with `ERROR_NOFILE` after
`MRISreadTriangleProperties: incompatible face number in file <file>`; `reproduce` reads up to the
surface's face places (0 or a negative count reads none) and refuses only a count beyond them.
Then the surface's original area is 0 and, per face, a float read (`freadFloat`: an error at the
end of the file, FS-167) is set as the face's cached original area (§27, bit 2 cleared) and added:
`orig_area ⊕ a` (f). Then each of the file's vertices (the vertex count) gets the original area
`0`, then for each of its faces in order `a_face ⊕ origarea` (f; the face's area on the **left**),
the face's cached original area read as §27 says (FS-116 for a deferred one). With `DIAG_SHOW` and
`DIAG_VERBOSE`: `total area = <orig_area %2.0f>.` and a newline. Then the angle file: missing gives
1 (the areas kept); the counts and checks as for the area file; three floats per face into its
original angles. Returns 0. FreeSurfer never closes the two files when it succeeds (no output
depends on it).

## 49. The distance file (`MRISwriteDists`)

The file: the name itself when it holds a `/` (a whole copy, FS-126's abort at 1,024 bytes);
otherwise beside the surface's own file (`<folder>/<name>`, the folder of the surface's name as
§48's), with the hemisphere's prefix `lh.` (the surface's hemisphere 0) or `rh.` (any other) when
the name has no `.`; with the environment variable `FS_POSIX` set (to anything) `./` replaces the
folder. Built names are cut at 1,023 bytes with `MRISwriteDists: Truncation on line <line>`: line
361 (`FS_POSIX`, no dot), 369 (no dot), 377 (`FS_POSIX`, a dot), 384 (a dot). The file (`wb`;
failing: `ERROR_NOFILE` after `MRISwriteDists: could not open <file>`): the vertex count and the
face count (3-byte numbers), then each vertex's `d` as a 2-byte number of `nint(d ⊗ 100.0)` (d)
(`specs/freesurfer-core.md`'s `nint`: FS-12 beyond the `int` range). Returns 0.

## 50. The area-error and angle-error files

**Area error** (`MRISwriteAreaError`): the name copied whole (FS-126's abort at 1,024 bytes); with
`DIAG_SHOW` and `DIAG_VERBOSE`, `writing area error file <file>...` before and `done.` and a
newline after, on standard output; the file (`wb`; failing: `ERROR_NOFILE` after
`MRISwriteAreaError: could not open <file>`): the two counts (3-byte numbers), then per vertex a
2-byte number of `nint(((area ⊖ orig) ⊗ 100) ⊘ num)` (f, then `nint` of the float as a double),
`area` the float sum (from 0, `area ⊕ face_area`) of its faces' areas and `orig` the float sum
(`orig ⊕ cached`) of their cached original areas read as §27's "read the normal" (a deferred face
computed and stored), in the vertex's face order, `num` its face count (as a float; 0 makes `0 ⊘
0`, a NaN, and `nint`'s FS-12). Returns 0.

**Angle error** (`MRISwriteAngleError`): with `DIAG_SHOW` and `DIAG_VERBOSE`, `writing angular
error file <file>...` and `done.`; the file as above (`MRISwriteAngleError: could not open
<file>`), the two counts, then per vertex a 2-byte number: for each of its faces in order, for each
of the three angles, `e ⊕ |δ|` (f), `δ` the difference of the face's angle and original angle
folded once by 2π (`specs/freesurfer-core.md`'s `deltaAngle`), `|·|` clearing the sign bit;
**FS-130** (rule A): FreeSurfer divides the running sum by `3 num` (as a float) after **every**
face (`e ⊘ (3 num)` (f)), so a vertex of more than one face gets less than the mean; larmorx divides
once, after the last face (a vertex without faces: 0); `reproduce` after every face. The error in
hundredths of a degree: `((e ⊗ 360) ⊘ 2π) ⊗ 100` (d, `e` widened, 2π the double nearest), converted
towards 0. **FS-131** (rule A, D13's conversions): a NaN or a value beyond the `int` range converts
to -2147483648 in FreeSurfer (then clamped to -32767); larmorx clamps (NaN 0, beyond the range the
nearest end, then the 2-byte clamp); `reproduce` gives -2147483648. Returns 0.

## 51. The curvature as a w file (`MRISwriteCurvatureToWFile`)

With `DIAG_SHOW`: `writing out surface values to <file>.` and a newline on standard output. The
file (`wb`); when it cannot be created the program ends: the error `Can't create file <file>`
followed by a newline (then FS-01's NUL and newline), exit status 255. Its contents: a 2-byte 0,
the vertex count (3 bytes), then per vertex its number (3 bytes) and its curvature (a float). A
curvature that is not finite: the error `MRISwriteCurvatureToWFile(<file>): val at vertex <k> is
notfinite` (no space before `finite`), and the value written anyway. Over the vertices in order,
`sum ⊕ c` (d, `c` widened), `(c ⊗ c) ⊕ sum2` (the square in float, widened, on the **left**), and
the extremes from max -1000 and min 1000 (`c > max`, `c < min`, compared in double: a NaN changes
neither). Then `mean = sum ⊘ n` (d), `sd = sqrt((sum2 ⊘ n) ⊖ (mean ⊗ mean))` (d), `n` the vertex
count (0 gives NaNs); the square root is the C library's: a negative argument (rounding) gives the
default NaN and sets `errno` to `EDOM` (33) whether or not anything is printed (so a later error
of the process prints `error: Numerical argument out of domain` first). With `DIAG_SHOW`: `avg =
<mean>, stdev = <sd>, min = <min>, max = <max>` (each `%2.3f`) and a newline. Returns 0.

## 52. The backup and imaginary value readers

`MRISreadValuesBak` and `MRISreadImagValues` read a w file into each vertex's backup value or
imaginary value. The file opened for reading (failing: `ERROR_NOFILE` after `<function>: File
<file> not found` and a newline, then FS-01's NUL and newline); a 2-byte number (read, unused);
every vertex's value 0; a 3-byte count; then per entry a 3-byte vertex number and a float
(`freadFloat`: FS-167 at the end of the file); a number at or beyond the vertex count prints
`<function>: vertex index out of range: <k> f=<value %f>` and a newline on standard output (the
value dropped), any other sets that vertex's value (a later entry wins). Returns 0.

## 53. The marks

**Marks to annotations**: each unripped vertex's annotation becomes its mark (the number); returns
0. **The largest mark**: the largest mark of the unripped vertices. **FS-132** (rule A): FreeSurfer
starts from vertex 0's mark whatever its rip flag (so a ripped vertex 0's mark counts), and for a
surface without vertices reads vertex 0's place (a spare place's mark, 0 for a new one; memory past
an empty array when it has no place); larmorx takes the unripped vertices only, 0 when there is
none; `reproduce` starts from vertex 0's place (0 when the surface has no vertex place, noted).

## 54. The ASCII and VTK formats

**ASCII** (`MRISwriteAscii`, opened `w`; failing: `ERROR_NOFILE` after `MRISwriteAscii: could not
open file <file>`): `#!ascii version of <surface's name>` and a newline; `<vertices> <faces>`; per
vertex `<x>  <y>  <z>  <rip>` (`%f`, two spaces, the rip flag as a number); per face `<v0> <v1> <v2>
<rip>` (each followed by a space but the last, then a newline). **Normals as ASCII**
(`MRISwriteNormalsAscii`, `MRISwriteNormalsAscii: could not open file <file>`): the same with the
first line `#!ascii version of <name> (vertices are surface normals)` and each vertex's normal in
place of its coordinates. **VTK** (`MRISwriteVTK`, opened `w`, `MRISwriteVTK: could not open file
<file>`): `# vtk DataFile Version 1.0`, `vtk output`, `ASCII`, `DATASET POLYDATA`, `POINTS <n>
float`, per vertex `<x>  <y>  <z>` (`%.9f`, two spaces), `POLYGONS <faces> <4 faces>`, per face `3
<v0> <v1> <v2> ` (a space after each number) and a newline. **The curvature into a VTK file**
(`MRISwriteCurvVTK`, opened for appending `a`; failing: `ERROR_NOFILE` after `MRISwriteScalarVTK:
could not open file <file>`): `POINT_DATA <n>`, `FIELD FieldData 1`, `curv 1 <n> double`, then one
`%.9f` curvature per line. Each returns 0.

## 55. Validation plan, part 8

The probe's `sfile` command (`validation/probes/fs-surfio/`): each case in an empty work folder
with a subfolder `d`, its input files written first, then a surface of `nv` 0 to 7 vertices with
`max` 0 to 3 more vertex places, `nf` 0 to 6 faces when `nv ≥ 3` with 0 to 2 more face places (every
place's three distinct corners, rip flag one in eight, area, angles, original angles, cache entry
and deferral byte, nonzero one in five), each vertex's coordinates, normal, `d`, curvature, value,
backup and imaginary values, areas (specials and NaN payloads, more in a third of the cases), mark,
annotation, rip flag (one in six) and its faces and corners from the faces; the hemisphere 0 to 2;
the surface's name (in the folder, in `d`, in a folder that does not exist, empty); `FS_POSIX` one
case in four, `MRIS_READVAL_FRAME` one in four (`3`, `-2`, `x`, ` 12`, `99999999999`, empty, `7x`,
`0`), `DIAG_SHOW` and `DIAG_VERBOSE` each one in two, `fix_vertex_area` 0 one in four; then 1 to 3
of the operations of §47-54 on names in the folder, in `d`, in a missing folder, with and without a
dot, with the dot first or second (FS-129), and, for seeds other than 1, now and then of 1,015 to
1,032 bytes (FS-126, FS-134). The readers' input files: triangle-property files with the writer's
integer counts, float counts (the surface's face count, one less, one more within its places, -1,
NaN, other bits whose float count stays within the places), mixed counts, the face values after
them, now and then cut; w files with 0 to 8 entries (vertex numbers within, at and beyond the
surface, `ff ff ff`), the count now and then beyond the entries, now and then cut; an existing VTK
file to append to. Then the state: each vertex's backup and imaginary values, area, original area,
normal's x and z and annotation; each face's area, angles, original angles, cached original area and
byte; the surface's original and total areas; then every file in the folder (its name relative to
it, sorted, and its bytes). An operation is skipped with `G` where FreeSurfer would read or write
memory it does not own: the writer of triangle properties when the surface has no spare face place
(FS-128), either triangle-property function with a name of 1,024 bytes or more (FS-134), the largest
mark of a surface with no vertex place (FS-132). Cases where larmorx notes FS-167 or FS-06 (memory
contents in FreeSurfer) are counted, not compared. Observed (relay 11): seed 7, 1,000 `sfile` cases
of 30,000: every comparable case identical with `reproduce` (49 counted, not compared); the default
differs only in cases with notices; with the whole probe (seed 7, 60,000 cases, 2,000 `sfile`): every
comparable case identical.

# Part 9 (relay 12, queue item 3.1-48 part a): per-vertex value files and other formats

The functions below open files by the name they are given (relative to the working directory);
a failed open leaves the C library's `errno` (`ENOENT` 2 for a missing file or folder), which the
error message that follows prints first (§46's `ErrorPrintf` form: `error: <strerror>` then
`error: <message>`). `ERROR_NOFILE` is -1, `ERROR_NO_MEMORY` -2, `ERROR_BADPARM` -4,
`ERROR_BADFILE` -5, `NO_ERROR` 0. "Verbose" below means `Gdiag` has both `DIAG_SHOW` and
`DIAG_VERBOSE`. Numbers in binary files are big-endian; a 3-byte number is unsigned; a 2-byte one is
signed. A number a binary file ends before is fs.io's (FS-167 for `fread2` and `freadFloat`, which
also prints `freadFloat: fread failed`; FS-49 for `freadInt`): its missing bytes are memory contents
in FreeSurfer.

## 56. One `scanf` call (the text readers' conversions)

The text readers convert with C's `sscanf` and `fscanf` as glibc 2.35 does (observed:
`validation/investigations/glibc-scanf-probe/`, open): the format's white space skips any amount of white
space (none included); another character must match the next input character; `%d` reads a `long`
and keeps its low 32 bits (`ERANGE` only beyond the 64-bit range); `%f` and `%e` read a `float`
as fs.core's conversion (correctly rounded; `ERANGE` on overflow and on a tiny inexact result;
`inf`, `infinity`, `nan` with a sign; `nan(...)` leaves the parenthesis unread); `%s` reads the
characters before white space (at most the width); `%*...` converts the same characters and
assigns nothing, `ERANGE` included. A call returns the number of items it assigned, or -1 when its
input ends before it assigned any; a matching failure returns the count so far. **`errno` at the
end of the input** (observed): the first time a call meets the end it remembers `errno` as it is
then, and every later attempt of the same call to read a character (a conversion's item, a format
character that must match, the white space a format ends with) sets `errno` back to that value; an
end met while a conversion skips the white space before its item is remembered as 0. A conversion
meets the end only by looking past its last character: a number does, `inf` does (it may go on as
`infinity`), a complete `nan` or `infinity` does not. So, with `errno` 5 before the call:
`sscanf("", "%d")` leaves 0, `sscanf("", "x%d")` 5, `sscanf("1e99", "%f %d")` 5 (the overflow
forgotten), `sscanf("1e99 ", "%f %d")` 0, `sscanf("1e99 x", "%f %d")` 34, `sscanf("nan", "%f %d")`
0, `sscanf("inf", "%f %d")` 5; `fscanf(fp, "\n")` at the end of a file returns 0 and leaves 5.
Each `fscanf` call starts afresh (a second `%f` at the end of a file returns -1 with `errno` 0).

**Lines** (`fgetl(line, n, fp)`): the next line of at most `n - 1` bytes (`fgets`; a longer line
gives its next `n - 1` bytes at the next call) that, after its leading white space, is neither
empty nor starts with `#`; every `#` in it ends it; its newline is removed. The readers scan the
line from its first character that is not white space; an error message that quotes the line
quotes it with its leading white space.

## 57. The curvature as text

**Reading** (`mrisReadAsciiCurvatureFile(mris, fname, curvmri)`): a file that cannot be opened
gives `ERROR_BADFILE` after the message `<name> could not open file <fname>.` and a newline, where
**FS-136** (rule A): FreeSurfer prints 25 bytes of binary data in place of the name (observed:
`f3 0f 1e fa 41 57 41 56 41 55 41 54 55 53 48 89 fb 48 89 f7 48 81 ec 38 04`); larmorx prints
`mrisReadAsciiCurvatureFile`, `reproduce` the 25 bytes. Then, for each vertex in order that is not
ripped, a line (`fgetl`, 100) scanned as `%*d %*f %*f %*f %f\n`: the fifth number becomes the
vertex's curvature and, when a volume of the values is given, its voxel `(vno, 0, 0, 0)` (fs.mri's
`MRIsetVoxVal`, the volume's type's conversion). A line without a fifth number gives
`ERROR_BADFILE` after `mrisReadAsciiCurvatureFile(<fname>): could not scan curvature from line
'<line>'` (the vertices before keep what they got). **FS-137** (rule B): when the file has no more
lines FreeSurfer stops and returns `NO_ERROR`, the remaining vertices keeping their curvatures (as
`reproduce`); larmorx returns `ERROR_BADFILE` after `mrisReadAsciiCurvatureFile(<fname>): the file
ends before vertex <vno>`. Returns `NO_ERROR`.

**Writing** (`mrisWriteAsciiCurvatureFile(mris, fname)`): per vertex that is not ripped, `%3.3d
%2.5f %2.5f %2.5f %2.5f` and a newline of its number, `x`, `y`, `z` and curvature. A file that
cannot be created ends the program (`mrisWriteAsciiCurvatureFile() could not open output file
<fname>.` and a newline; exit status 251, `ERROR_BADFILE`'s). Returns `NO_ERROR`.

## 58. The old binary curvature and the curvature arrays

**Old format** (`__mrisreadcurvoldformat(mris, fname, curvmri)`): verbose: `reading curvature file
(old format) ...` (no newline) first. A file that cannot be opened: `ERROR_NOFILE` after
`__mrisreadoldcurvaturefile(): could not open <fname>`. The vertex count (3 bytes), the face count
(3 bytes); a vertex count other than the surface's: `ERROR_NOFILE` after
`__mrisreadoldcurvaturefile(): incompatible vertex number in file <fname>`. Then per vertex a
2-byte number `i`, the curvature `(float)(i / 100.0)` (in `double`), into the vertex and the given
volume; the surface's largest and smallest curvature from them (the first value both, then each
larger or smaller one; 10,000 and -10,000 for a surface without vertices). Verbose: `done.
min=%2.3f max=%2.3f` and a newline. Returns `NO_ERROR`.

**Arrays** (`MRISreadCurvatureIntoArray(name, size, &out)`): verbose: `reading curvature file...`.
A file that cannot be opened: `ERROR_BADFILE` after `MRISreadCurvatureVectorIntoArray(<name>):
fopen failed`. The first 3 bytes 16,777,215 (the new format's mark): the file is read by the new
reader below. Else they are the vertex count: another than `size` returns `ERROR_BADFILE` without a
message; then the face count (3 bytes) and per vertex a 2-byte number in hundredths as above; the
array has `size` values. Returns `NO_ERROR` and the array. **New format**
(`MRISreadNewCurvatureIntoArray`): verbose: `reading curvature file...`; a file that cannot be
opened: `ERROR_BADFILE` after `MRISreadNewCurvatureIntoArray(<name>): fopen failed`, where
**FS-138** (rule A): FreeSurfer reads a name it is never given and ends with a segmentation fault
(signal 11) after the `errno` line (as `reproduce` ends). The first 3 bytes other than the mark:
the old reader above. Then the vertex count, the face count and the values per vertex as 4-byte
integers: another vertex count than `size` gives `ERROR_BADFILE` after
`MRISreadNewCurvatureIntoArray(<name>): number of vertices (<n>) doesn't match what was expected
(<size>)`; a count of values per vertex other than 1 gives `ERROR_BADFILE` after
`MRISreadNewCurvatureIntoArray(<name>): vals_per_vertex was not 1`; a negative `size` the file
matches cannot be allocated: the program ends (`Cannot allocate memory`, then
`MRISreadNewCurvatureVector(<name>): calloc failed`, exit status 254). Then a float per vertex.
Returns `NO_ERROR` and the array.

**Vectors** (`MRISreadCurvatureVector(mris, sname)`, `MRISreadNewCurvatureVector`): the file is
`sname` itself when it holds a `/` (a name of 1,024 bytes or more: FS-126's abort); else in the
folder of the surface's name (its part before the last `/`, or `.`), `<folder>/<sname>` when
`sname` has a dot, `<folder>/<lh|rh>.<sname>` without one (`lh` for the left hemisphere, else `rh`;
cut at 1,023 bytes with `<function>: Truncation on line <n>`: lines 5030 and 5038, and 5882 and
5889); then the array reader above with the surface's vertex count (old reader first for
`MRISreadCurvatureVector`, new reader first for the other); `NULL` on any error.

## 59. The float and area files

**Float file** (`MRISreadFloatFile(mris, sname)`): the file as the vectors' (Truncation lines 5129
and 5136); verbose: `reading float file...`. A file that cannot be opened: `ERROR_NOFILE` after
`MRISreadFloatFile: could not open <file>`. The vertex and face counts (4-byte integers); another
vertex count: `ERROR_NOFILE` after `MRISreadFloatFile: incompatible # of vertices in file <file>`;
another face count: `... incompatible # of faces file <file>`. Then a float per vertex into its
value (`val`). Returns `NO_ERROR`.

**Area file** (`MRISreadBinaryAreas(mris, mris_fname)`): verbose: `reading area file...`; the file
`<folder of mris_fname>/<lh|rh>.area` (FS-134 for a name of 1,024 bytes or more; Truncation line
5199). Missing: `ERROR_BADPARM` after `MRISreadBinaryAreas: no area file <file>` and a newline. The
vertex and face counts (3 bytes); another vertex count: `ERROR_NOFILE` after `MRISreadBinaryAreas:
incompatible vertex number in file <file>`. Then a float per vertex into its original area.
Returns `NO_ERROR`.

## 60. The names of files to read, and FS_GII

**`MRISbuildFileName_read(mris, sname, fname)`**: a name with a `/` is the file. Else, with the
surface's folder as above: when the two characters before the name's **first** dot are `lh` or `rh`
(`?h`, the `h` right before the dot and an `l` or `r` before it), `./<sname>` with `FS_POSIX` set
(cut at 1,023 bytes, Truncation line 5675), else `<folder>/<sname>` (line 5682); otherwise the
hemisphere's prefix (`lh.` left, `rh.` right, `both.` both, nothing else) goes before the name:
`./<prefix><sname>` with `FS_POSIX` (line 5696), else `<folder>/<prefix><sname>`. **FS-148**: a
first dot at the name's start, or second after an `h`, makes FreeSurfer read the byte(s) before the
name; larmorx takes them as not those letters in both modes (noted). **FS-149**: the legacy names
without a hemisphere and the names with a `/` are not bounded (FreeSurfer writes a name of 1,024
bytes or more past its caller's buffer); larmorx returns them whole in both modes (noted). Returns
`NO_ERROR`.

**`__MRISapplyFSGIIread(file_to_read, fname, filetype)`**: `<fname>.gii` of 1,024 bytes or more
aborts the official build (FS-126: `*** buffer overflow detected ***: terminated`, signal 6; larmorx
takes it whole, `reproduce` aborts). When both `fname` and `<fname>.gii` can be opened for reading:
`[ERROR] both <fname> and <fname>.gii exist. Check FS_GII setting, remove unused files` and the
program ends with status 1. With `FS_GII` exactly `.gii`: when `<fname>.gii` can be opened,
`file_to_read` becomes it, `filetype` 27 (`GIFTI_FILE`) and `[INFO] read, FS_GII set, read as
<fname>.gii (<fname>)`; else `[INFO] read, FS_GII set, cannot find <fname>.gii, trying <fname>
...`. Without it, when `fname` cannot be opened: `file_to_read` `<fname>.gii`, `filetype` 27,
`[INFO] read, cannot find <fname>, trying <fname>.gii ...`. Each test of a file sets `errno` when
it fails. The messages on standard output with their newlines.

**`__MRISapplyFSGIIwrite(file_to_write, fname, filetype)`**: with `FS_GII` `.gii`: `file_to_write`
`<fname>.gii` (FS-149), `filetype` 27, `[WARN] write, FS_GII set, write as <fname>.gii (<fname>)`;
then when `fname` can be opened: `[ERROR] attemp to write GIFTI <fname>.gii, its regular form
<fname> exists` and exit status 1. Without it: (FS-126's abort for `<fname>.gii` of 1,024 bytes or
more) when `<fname>.gii` can be opened, `[ERROR] attemp to write <fname>, its GIFTI form
<fname>.gii exists` and exit status 1.

## 61. The decimation files

**Writing** (`MRISwriteDecimation(mris, fname)`): a 0 byte, the vertex count (4 bytes), then per
vertex a byte, 1 when its `d` is 0 (either zero), else 0. Cannot be created: `ERROR_BADFILE` after
`MRISwriteDecimation: could not create <fname>`. Returns `NO_ERROR`.

**Reading** (`MRISreadDecimation(mris, fname)`): every vertex's `undefval` becomes 1 and `fixedval`
0; a file that cannot be opened: `ERROR_BADFILE` (-5) after `MRISreadDecimation: could not create
<fname>`. The first byte `#`: a text file: a word (`fscanf("%*s")`), the count (`%d`), then a
number per vertex (each its own `fscanf("%d")`); else binary: the count (4 bytes), then a byte per
vertex. A count other than the vertex count returns 0 after `<program>: decimation file <fname> has
wrong # of vertices` and a newline. Each vertex whose number or byte is not 0 gets `d` 0,
`fixedval` 1 and `undefval` 0, and is counted; the count is returned. **FS-139** (rule B):
FreeSurfer takes a byte the file ends before as not 0 (decimated) and a number it cannot read as the
last one read (the count for the first) and reports the count (as `reproduce`); larmorx returns 0
after `MRISreadDecimation: decimation file <fname> ends before vertex <k>`. **FS-140**: a text
file whose count cannot be read: FreeSurfer compares memory contents with the vertex count; larmorx
reports the wrong count (`reproduce` takes the count as 0).

## 62. The VTK reader

**`MRISreadVTK(mris, fname, curvmri)`**, into a new surface (`mris` `NULL`) or into `mris`. Every
error returns `NULL` after a message with `ERROR_NOFILE`: the file cannot be opened (`MRISreadVTK:
could not open file <fname>` and a newline). Lines (`fgetl`, 1,024) are read until three of them
start with one of `VTK OUTPUT`, `vtk output`, `surface file`, `SURFACE FILE`, `ASCII`, `ascii`,
`DATASET POLYDATA`, `dataset polydata` (each line counts once); fewer: `MRISreadVTK: error reading
file <fname>` and a newline. The next line: none gives `MRISreadVTK: error parsing POINTS from file
<fname>`; scanned as `POINTS %d float\n`, and when that returns 0 as `points %d float\n`; both 0:
`... error reading file <fname>, no POINTS field`; a count (0 when none was read: a scan that
returned -1 counts as a match) not above 0: `... error reading file <fname>, invalid
nvertices=<n>`. A new surface is `MRISalloc(n, 0)` of type 3 (`MRIS_TRIANGULAR_SURFACE`); a given
one must have `n` vertices (`... error reading file <fname>, mismatched nvertices=<m>/<n>`). Its
distances are freed (§15). Then per vertex `fscanf("%f %f %f")`: a new surface's vertex gets them
(`MRISsetXYZ`); a given one's coordinates are kept. **FS-143** (rule B): a call that does not
assign three numbers leaves FreeSurfer with the numbers read before for the missing ones (those of
the vertex before; **FS-144**: memory contents for the first vertex) and it goes on; larmorx
returns `NULL` after `MRISreadVTK: error reading the coordinates of vertex <vno> in file <fname>`;
`reproduce` goes on with the earlier numbers (0 for FS-144's). The next line: none, or neither
`POLYGONS %d %d\n` nor `polygons %d %d\n` matching: `... error parsing POLYGONS in file <fname>`
(no newline); a face count not above 0: `... invalid nfaces=<n>`; the second number not four
times the first (in 32 bits): `... invalid POLYGON data`. A new surface allocates the faces (a
failure ends the program: `MRISreadVTK(<n>, 80): could not allocate faces`); a given one must have
as many (`... mismatched nfaces=<m>/<n>`). Per face `fscanf("%d ")` (a count it cannot read keeps
the one before, 0 at first) must give 3 (`... facepoints != 3`); then three `fscanf("%d")` (-1 when
not read) within `0 .. n-1` (`MRISreadVTK: invalid vertex num in <fname>, vno = <v>`): a new
surface's face gets them and each vertex's face count grows by one, where **FS-147** (rule B): the
count is one byte and wraps after 255 in FreeSurfer (as `reproduce`); larmorx refuses a vertex of
more than 255 faces (`... error reading file <fname>, vertex <v> is in more than 255 faces`); a
given surface's face must hold the same number (`... mismatched faces=<a>/<v>`). The next line,
when there is one, scanned as `POINT_DATA %d\n` or `point_data %d\n` (no match ends the reading):
a count other than `n` gives `... invalid nvertices=<k> in POINT_DATA field, expected <n>
vertices`; then a line scanned as `FIELD %*s %d\n` or `field %*s %d\n` (no match ends the reading),
the number of fields; per field a line `%s %d %d %*s\n` (the name, a number, the count: a count
not read keeps the one before) whose count must be `n` (`... invalid nvertices=<k> in <name>
field, expected <n> vertices`), then per vertex `fscanf("%f")`: 0 numbers assigned: `MRISreadVTK:
error parsing <name> FIELD in file <fname>`; the value (0 when the call returned -1: **FS-146**,
rule B: larmorx gives that error instead, `reproduce` takes 0) becomes the vertex's `curv` and
`val` and the given volume's voxel; a field whose name starts with `curv` or `CURV` sets the
surface's largest and smallest curvature from its values. **FS-145** (rule B): a file that ends
where the `FIELD` line or a field's line should be makes FreeSurfer end with a segmentation fault
(signal 11; `reproduce` ends so); larmorx returns `NULL` after `... error reading file <fname>, it
ends after its POINT_DATA line` (or `inside its FIELD lines`). Returns the surface.

## 63. The movie.byu and icosahedron writers

**`MRISwriteGeo(mris, fname)`**: cannot be created: `ERROR_NOFILE` after `MRISwriteGeo: could not
open file <fname>`. Lines: `    1 <V> <F> <3F>` and `    1 <F>` (V and F the vertices and faces that
are not ripped); the unripped vertices' `x y z` as `%12.5e %12.5e %12.5e`, two vertices to a line
(a space between them, a newline after the second; a last odd one ends its line); per unripped
face, with each vertex's number among the unripped ones from 1: `<first> <third> -<second>` and a
newline. **FS-141** (rule B): an unripped face with a ripped vertex makes FreeSurfer write a number
from memory contents; larmorx returns `ERROR_BADPARM` without writing, after `MRISwriteGeo(<fname>):
face <fno> has the ripped vertex <vno>`; `reproduce` writes 1 for it. (FreeSurfer's numbers are
held in 300,000 places: a larger surface writes past them; larmorx holds every vertex in both
modes.) Returns `NO_ERROR`.

**`MRISwriteICO(mris, fname)`**: cannot be created: `ERROR_NOFILE` after `MRISwriteICO: could not
open file <fname>`. `%8d` and a newline of V; per unripped vertex `%8d %8.4f %8.4f %8.4f` of its
number from 1 among them and `x y z`; `%8d` of F; per unripped face `%8d %8d %8d %8d` of its number
from 1 among them and its first, third and second vertices. **FS-142** (rule A): FreeSurfer writes
each face's vertices as their numbers on the surface plus one, which name other vertices once a
vertex before them is ripped; larmorx writes the numbers it gave them (`reproduce`: FreeSurfer's).
FS-141 likewise: larmorx refuses an unripped face with a ripped vertex (`MRISwriteICO(<fname>):
face <fno> has the ripped vertex <vno>`, `ERROR_BADPARM`). Returns `NO_ERROR`.

## 64. The surface-to-Talairach matrix and the MATLAB plots

**`getSRASToTalSRAS(lt)`**: two 4 x 4 identities, the first with the source geometry's centre
`(c_r, c_a, c_s)` as its translation column, the second with the destination's centre negated
(`-c`: a NaN's sign flips); the result `second · (m_L · first)` with fs.core's matrix product
(`float`).

**`MatlabPlotVertex(fp, surf, vno, color, len)`**: `% Vertex <vno>` and a newline on standard
output; into `fp`: `plot3([x],[y],[z],'<color>*')`, `h=text(x,y,z,'V<vno>');`,
`set(h,'fontsize',7,'color','<color>');` (each with a newline, the numbers `%6.4f` of the `double`
of the coordinate); when `|len| > 0` (not for a NaN): `hold on;`, `plot3([x,X],[y,Y],[z,Z],'k')`,
`hold off;`, where `X = (nx · len) + x` in `double` (the normal's component first, then the
product first: a NaN's sign and payload follow that order). Returns 0. **`MatlabPlotFace(fp, surf,
fno, color, len)`**: `% Face <fno>` on standard output; into `fp`: `plot3(` then for each corner
`n` and the next `m` (cyclically) `[xn,xm],[yn,ym],[zn,zm],'<color>' ` followed by `,` except after
the third, then `);` and a newline; the centre `c = ((((0 + x0) ...` summed in `double` with each
corner's coordinate as the **left** operand (`x_k + sum`), divided by 3; `h=text(cx,cy,cz,'F<fno>');`
and the `set(...)` line; when `|len| > 0`: `hold on;`, `plot3([cx],[cy],[cz],'<color>*',` and
`[cx,X],[cy,Y],[cz,Z],'k'` and `);`, `hold off;` (each with its newline), where the normal is
§41's corner normal at the face's first corner (the edges `p2 - p0` and `p1 - p2` with `p0` the
third corner, `p1` the second, `p2` the first, each made unit length; the cross product's second
component computed as `v0[2] v1[0] - v1[2] v0[0]`; made unit length when `UnitizeNormalFace` is
set) and `X = (n · len) + c` as for a vertex. Returns 0.

## 65. Validation plan, part 9

The probe's `cfile` command (`validation/probes/fs-surfio/`): as `sfile` (§55), each case in an empty
work folder with `d`; `FS_POSIX` one case in four, `FS_GII` one in three (`.gii` twice as often as
`gii` or empty), `DIAG_SHOW`/`DIAG_VERBOSE` each one in two; a surface of 0 to 7 vertices and, with
3 or more, 0 to 6 faces (three distinct corners, rip flag one in eight), each vertex's coordinates,
normal, `d`, curvature, value and original area (specials and NaN payloads, more in a third of the
cases), rip flag (one in six), `undefval`, `fixedval`; the hemisphere 0 to 3; the surface's name;
its largest and smallest curvature. Then 1 to 3 of the operations of §57-64 (and `UnitizeNormalFace`
set to 0 or 1) on names in the folder, in `d`, in a missing folder, with and without dots, with the
dot first, and (seeds other than 1) of about 1,024 bytes. The input files: text curvature files
(numbers in decimal, fixed and exponent forms, `nan`, `inf`, `1e40`, `0x1p3`, `1e`, `x`;
comments, blank lines, leading tabs, lines of more than 99 bytes, too few lines, lines of fewer
than five numbers), old and new binary curvature files (a count near the surface's, a second value
per vertex, cut), float and area files (likewise), decimation files (binary and text, other counts,
too few values, a lone `#`, a word that is no number), VTK files (the header lines in either case,
one missing; `POINTS` variants: no count, a negative or zero count, `points`, `double`, one more;
the coordinates now and then one short or not a number; `POLYGONS` variants; faces of 4 points or
a vertex out of range; `POINT_DATA` (or `CELL_DATA`), `FIELD` with 0 to 2 fields named `curv`,
`CURVATURE`, `thickness`, `val`, counts one off, values cut; files of 255 to 257 faces on one vertex);
the files `name` and `name.gii` for `FS_GII`. Then the state: each vertex's coordinates, normal,
`d`, curvature, value, original area, `undefval`, `fixedval`; the largest and smallest curvature;
every file in the folder with its bytes. `G` (skipped on both sides): `MRISbuildFileName_read` with
a name and surface name of about 1,016 bytes or more together (FS-149), `__MRISapplyFSGIIwrite`
with `FS_GII` set and a name of 1,020 bytes or more (FS-149), `MRISwriteGeo` with an unripped face
of a ripped vertex (FS-141). Cases noted with FS-49, FS-140, FS-144 or FS-167 (memory contents in
FreeSurfer) are counted, not compared. Observed (relay 12): seeds 7, 13 and 17, 2,000 `cfile` cases
each: every comparable case identical with `reproduce` (253 counted, not compared).

# Part 10 (relay 13, queue items 3.1-48 part b and 3.1-49's STL): the raw surface readers, STL

Part 9's conventions hold (the error codes, `errno` before a message, one `scanf` call §56, `fgetl`
lines). The readers below build a raw surface (`MRISalloc(nvertices, nfaces)`, §31): its counts,
coordinates, rip flags, faces and (STL) neighbour lists; completing it (the topology, the metric
properties) is the caller's work (`MRISreadOverAlloc`, later). A "free-format number" is one
`fscanf` conversion of the file's next item, any white space (newlines included) before it.
**Unreadable values (FS-151, FS-152)**: where a reader's call cannot convert a number (the file ends,
or holds something else there), FreeSurfer keeps the value its variable had: the one the same
variable got from the call before (FS-152), or, where no call assigned it yet, memory contents
(FS-151). larmorx refuses such a file by default with the message given below (an error, the
reader's failure result); `reproduce` keeps the values read before (FS-152) and uses 0 where
FreeSurfer has memory contents (FS-151), with the notices.

## 66. The ASCII surface (`mrisReadAsciiFile(fname)`)

The file: a first line (`fgetl` of up to 1,023 bytes, §56) `<nvertices> <nfaces>` (two `%d`), then
per vertex four free-format numbers `x y z rip` (`%f %f %f %d`), per face four `a b c rip`. A file
that cannot be opened: `MRISreadAsciiFile: could not open file <file>` (`ERROR_NOFILE`, `NULL`). A
file with no line (empty, or only blank and comment lines): **FS-150**, FreeSurfer ends with a
segmentation fault (signal 11); larmorx: `mrisReadAsciiFile(<file>): the file holds no line`
(`ERROR_BADFILE`, `NULL`); `reproduce` exits with status 139. Counts that cannot be read: FS-151
(`mrisReadAsciiFile(<file>): could not read the counts`). The surface: type
`MRIS_TRIANGULAR_SURFACE` (3); a negative count fails `MRISalloc`'s check (FS-03, §31). Each vertex:
its coordinates set as `MRISsetXYZ` does (§14); its rip flag the `int` read, kept in one signed byte
(`256` gives 0, `255` gives -1); any nonzero rip flag makes the surface a patch (`patch` 1, status
`MRIS_PLANE`, 2). The variables of the three coordinates and the rip flag keep their values from
vertex to vertex, and the rip flag's from the last vertex into the faces: a vertex whose numbers
cannot be read takes the previous vertex's (FS-152; the first vertex's: FS-151; message `... could
not read vertex <v>`). Each face: three `%d ` calls into its vertices (a vertex number that cannot
be read stays 0, as `MRISalloc` gave it: FS-152, `... could not read face <f>`); a vertex number
below 0 or not below the vertex count ends the program: `<program>: face <f> vertex <n>: <v> --
out of range!!! Should be in [0 <nvertices - 1>]` (`ERROR_BADPARM`, exit status 252); each corner
adds 1 to its vertex's face count in one byte (**FS-147**: past 255 it wraps; larmorx refuses,
`... vertex <v> is in more than 255 faces`; `reproduce` wraps); then its rip flag (`%d\n`, kept from
the call before when it cannot be read: FS-152; never read before: FS-151), in one signed byte.
Returns the surface.

## 67. The movie.byu surface and its positions

**The header** (both readers): a first line (`fgetl` of up to 99 bytes) read as `%*d %d %d %d`
(the parts, unused; the vertex, face and edge counts). No line: FS-150 (signal 11; larmorx
`mrisReadGeoFile(<file>): the file holds no line`). Fewer than three counts: `mrisReadGeoFile(<file>):
could not scan dimensions from '<line>'` (the line from its first character that is not white
space; `ERROR_BADFILE`). A face count of 0, or -1 with an edge count of -2^31: **FS-153**, FreeSurfer
ends with an arithmetic exception (signal 8, the integer division); larmorx
`mrisReadGeoFile(<file>): unsupported vertices/face: <e> edges for <f> faces.`, `reproduce` exits
with status 136. Edges divided by faces (C's integer division) other than 3:
`mrisReadGeoFile(<file>): unsupported vertices/face <n>.` (`ERROR_BADFILE`). Then a second line
(`fgetl` of up to 199 bytes: the face count again, unused). Opening failures: `mrisReadGeoFile:
could not open file <file>` (`ERROR_NOFILE`).

**The coordinates**: per vertex three free-format numbers (`%e %e %e`; a vertex of an odd number
also skips the white space after it: `fscanf(fp, "\n")`); the variables keep their values from
vertex to vertex (FS-152, FS-151; larmorx `... could not read vertex <v>`, the message's reader name
`mrisReadGeoFile` or `mrisReadGeoFilePositions`).

**`mrisReadGeoFile(fname)`**: a new surface of the header's counts, type `MRIS_GEO_TRIANGLE_FILE`
(2), the coordinates as above, then per face `%d ` twice and `-%d\n` (the last vertex written
negative, its `-` a format character that must match), each number minus 1 (the file counts from
1); a number that cannot be read stays 0 before the subtraction (FS-152). A face vertex that is not
one of the surface's (below 0 or not below the vertex count after the subtraction, including an
unreadable one and a last vertex not written negative): **FS-154**, FreeSurfer counts the face in
memory outside the surface's arrays (a crash, or corrupted memory later); larmorx
`mrisReadGeoFile(<file>): face <f> vertex <n>: <v> is not a vertex of the surface`, `NULL`;
`reproduce` keeps the number and counts nothing. Each valid corner adds 1 to its vertex's face
count (FS-147 as §66). Then the face's second and third vertices are swapped. The rip flags stay 0.
Returns the surface.

**`mrisReadGeoFilePositions(mris, fname)`**: the header, then the coordinates of `mris`'s own
vertex count (not the file's), set as `MRISsetXYZ` does (the surface's distances not freed: §14's
logic message when they are marked allocated). **FS-155**: FreeSurfer does not compare the file's
counts with the surface's; larmorx refuses a file of other counts:
`mrisReadGeoFilePositions(<file>): the file holds <n> vertices and <m> faces, the surface <n'> and
<m'>` (`ERROR_BADFILE`); `reproduce` reads as FreeSurfer. Returns `NO_ERROR`, `ERROR_NOFILE` or
`ERROR_BADFILE`.

## 68. The binary triangle file's positions

Both readers open the file (`mrisReadTriangleFile(<file>): could not open file`, `ERROR_NOFILE`),
read 3 bytes (the magic number, not checked), one line (`fgets` of up to 199 bytes), skip white
space (`fscanf(fp, "\n")`: also bytes of the counts that happen to be white space), then the vertex
and face counts (`freadInt`, big-endian; FS-49 at the file's end). Verbose: `surface <file>: <n>
vertices and <m> faces.` on standard output. Coordinates are big-endian floats (`freadFloat`,
FS-167 at the end).

**`mrisReadTriangleFileVertexPositionsOnly(fname)`**: a vertex count of 0: `MRISreadVerticesOnly:
could not allocate surface` (`ERROR_NO_MEMORY`, `NULL`). A negative count: **FS-156**, its
allocation fails (`errno` `ENOMEM`, 12) and FreeSurfer returns a surface of that count with no
vertices; larmorx gives the message above (`Cannot allocate memory` first), `reproduce` returns that
surface. Otherwise per vertex three floats; after each vertex's three, a coordinate whose size is
beyond 10000, or that is not finite, ends the program: `<program>: vertex <v> x coordinate
<value>!` (`y`, `z`; `%f`; `ERROR_BADFILE`, exit status 251), the first such coordinate in x, y, z
order. Returns the positions (the small surface of §11's `SMALL_SURFACE`).

**`mrisReadTriangleFilePositions(mris, fname)`**: counts other than the surface's: `mrisReadTriangleFile
opened <file> okay but surface doesn't match <surface's file name>.  nvertices:<n> !=
mris->nvertices:<n'> || nfaces:<m> != mris->nfaces:<m'>` and its newline (`ERROR_BADPARM`; the file
stays open). Verbose message; the surface's distances freed (`MRISfreeDistsButNotOrig`, §8); then per
vertex three floats set as `MRISsetXYZ` does. Returns `NO_ERROR`.

## 69. Two triangles' quadrangle (`quadCombine(quad, vA, vB)`)

Goes through `vA`'s three vertices in order; each is appended to `quad` (when four are already
there: returns false); after it, the vertices of `vB` are searched in order for one equal to it:
the first equal one whose predecessor in `vB` (cyclically: the one before position 0 is position 2)
is `vA`'s next vertex (cyclically) appends `vB`'s vertex after it (cyclically; when four are
already there: returns false); an equal one whose predecessor is not `vA`'s next vertex ends the
search. Returns whether exactly four were appended. `quad`'s places are written as they are
appended, also when it returns false (the others keep what they held). So two triangles sharing
one edge in opposite directions give `vA`'s vertices with `vB`'s third inserted after the shared
edge (observed: `0 1 2` and `2 1 4` give true and `0 1 4 2`; `0 1 2` and `2 1 0`, two shared
edges, false and `0 2 1 0`; `0 1 2` and `3 4 5` false and `0 1 2`, the fourth place untouched).

## 70. STL surfaces, read (`mrisReadSTL(filename)`)

A file that cannot be opened: `NULL`, no message. **Binary or text**: the file is binary when its
size equals `84 + 50 n`, computed in 32 unsigned bits, for the `n` its bytes 80 to 83 give
(little-endian); a file of fewer than 84 bytes is never binary (observed: its size reads as
2^32 - 1). **FS-160**: a count whose product with 50 wraps around 2^32 to the file's size makes
FreeSurfer read the triangles past the file's end (memory contents); larmorx refuses it: `error:
STL file <file>: <n> triangles do not fit in its <size> bytes`, `NULL`; `reproduce` reads them as 0.

**Binary**: after the 80-byte header (unused) and the count, per triangle the normal and three
corners (12 little-endian floats) and 2 bytes (unused).

**Text**: lines (`fgetl` of up to 1,023 bytes) told apart by their first character that is not
white space: `v`: `vertex %f %f %f` (`sscanf`), a corner; `f`: `facet normal %f %f %f`, a new facet
with this normal; `s`: a solid, the second one an error: `error: STL file <file> contains multiple
solids!`, `NULL`; `e`, `o`: passed over (any line starting so); any other: `error: ASCII STL file
<file> contains unrecognized line: <line>` (the line with its leading white space, cut at a `#`),
`NULL`. The three numbers of the vertex and facet lines go into the same three variables: a number
a line cannot read keeps the one the line before (vertex or facet) gave (FS-152; never given: FS-151;
larmorx `error: ASCII STL file <file>: could not read the numbers of line <line>`, `NULL`).

**The vertices**: two corners are one vertex when their three coordinates compare equal as floats
(`-0` equals `+0`; a NaN equals nothing, so every corner with a NaN is a vertex of its own); vertices
are numbered in the order of their first corner and keep that corner's coordinates. A corner
before the first facet, or a fourth corner of a facet, ends the program: `error: STL file does not
appear to represent a triangular mesh` (exit status 1). **FS-158**: FreeSurfer finds earlier corners
through a table indexed by a hash of the coordinates that is negative for most corners with one or
three NaN or infinite coordinates (observed: the index is `h % 2^20` with
`h = H(x)·2017 ^ H(y)·1029 ^ H(z)·1159` in 32-bit wrapping arithmetic, `H(c) = (int)(1000 ·
logf(|c| + 1))` in float, glibc's `logf`, a NaN or out-of-range float converting to -2^31; the
index only decides where a corner is looked up, never which vertex it is), and then reads memory
before the table (a crash or a wrong vertex); larmorx looks such corners up as any other, noted.
**FS-159**: a facet of fewer than three corners: FreeSurfer ends with a segmentation fault (signal
11) once the file is read; larmorx `error: STL file <file>: facet <k> has fewer than three
vertices` (the first such facet, from 0), `NULL`; `reproduce` exits with status 139.

**The surface**: `MRISalloc(vertices, facets)`, type `MRIS_TRIANGULAR_SURFACE` (3); each face's
cached normal is its facet's (`setFaceNorm`: not deferred), its vertices the corners' vertices,
each vertex's coordinates its first corner's (set as `MRISsetXYZ` does). Then for each face in
order and each corner: the vertex's face count `num` plus 1 (one byte; **FS-157**: a vertex of more
than 255 faces makes FreeSurfer write its face lists past their allocation; larmorx `error: STL
file <file>: vertex <v> is in more than 255 faces`, `NULL`; `reproduce` gives the whole lists and
the wrapped count) and its neighbour count `vnum` plus 2 (`modVnum`, §11: the debug message when it
passes the largest seen). Each vertex's neighbour list gets `vnum` places and `vnum` becomes 0;
then for each face in order and each corner, the face's other two vertices in order are appended
to the corner's vertex's list unless already in it (a vertex can be its own neighbour, when a facet
names it twice). Each vertex's face list `f` and positions `n` get `num` places; `num` becomes 0,
`vtotal` becomes `vnum`; then for each face in order and each corner `k`, the corner's vertex
appends the face number to `f` and `k` to `n`. The rip flags stay 0; nothing else is computed.

## 71. STL surfaces, written (`mrisWriteSTL(mris, filename)`)

A binary STL file: 80 bytes (`freesurfer mesh` and zeros), the face count (32-bit little-endian),
per face its cached normal (`getFaceNorm`, §28: a deferred one computed first) and its three
vertices' coordinates (little-endian floats), then the two bytes `0` (0x30) and 0. Returns true;
false, with no message, when the file cannot be created.

## 72. Validation plan, part 10

The probe's `sread` and `stl` commands (`validation/probes/fs-surfio/`): as `cfile` (§65), each case
in an empty work folder with `d`. `sread`: `FREESURFER_reportLogicErrors` set one case in two;
`DIAG_SHOW`/`DIAG_VERBOSE` each one in two; a case surface of 0 to 6 vertices (coordinates, rip
flags one in six) and, with 3 or more, 0 to 4 faces, its distances' flags and size 0 to 3; 1 to 3
operations: the ASCII reader on files of 0 to 6 vertices (a comment line or blank lines first one
in three; the header `x y`, one count, or a negative count now and then; coordinates in decimal,
fixed, exponent and special forms, `nan`, `inf`, `1e40`, `x`; rip flags 0, 1, 2, -1, 255, 256, 512,
1000; a face vertex out of range one face in fifteen; 255 to 257 faces on one vertex one file in
25; a file with no line one in 30; cut by 1 to 6 bytes one in nine); the movie.byu reader and the
positions reader on files of the case's counts or others (edges of 4 per face or one too many, a
face count of 0, a header of two numbers, vertex numbers 0, the count plus one or -2, a last vertex
not negative, cut files, files with no line); the two triangle-file readers on files with a creator
line of 210 bytes now and then, or without the blank line, or with extra white space; counts of the
case's or others, -1 or -7 or 0; coordinates beyond 10000 or not finite now and then; cut files;
`quadCombine` on triangles of vertices 0 to 5, half of them sharing an edge reversed. `stl`: Gdiag
as above; 1 to 3 operations, the reader (four in five) on text or binary files of 0 to 5 facets of
points from a pool of 3 to 6 (their zeros made `-0` one point in ten; a NaN or infinity one in 40),
fans of 16 to 20 facets on one point one file in twelve (a sixth of them 256), text files with
comments, numbers that cannot be read, facets of 2 or 4 corners, a corner before the first facet, a
second solid, an unrecognized line, `SOLID`; binary files whose header starts with `solid` one in
three and one byte too long one in 15; and the writer on a case surface of 0 to 6 vertices and 0 to 4
faces with cached normals (deferred one in five). Then the case surface's state and every file in
the folder. Cases noted with FS-151, FS-154, FS-157, FS-158 or FS-160 (and fs.io's FS-49, FS-167)
are counted, not compared. Observed (relay 13): `sread` seed 7, 1,000 cases and seed 13, 2,000;
`stl` seed 7, 1,000: every comparable case identical with `reproduce` (92, 156 and 105 counted, not
compared), the default different only with notices.

# Part 11 (relay 14, queue item 3.1-48): the binary triangle and quadrangle surface files

Part 9's and 10's conventions hold. Numbers in these files are big-endian: 3-byte unsigned numbers
(`fread3`/`fwrite3`: a negative number written as its low 24 bits), 2-byte signed `short`s, 4-byte
`int`s and floats, 8-byte lengths; fs.io's reading rules at the end of a file hold (FS-49, FS-167:
the missing bytes of a number the file ends in are memory contents in FreeSurfer). The tags are
fs.io's (`specs/freesurfer-io.md` §11-12); the volume geometry's text block is fs.volume's
(`readVolGeom`, `writeVolGeom`: eight lines `valid = <v>  # volume info valid` (or `invalid` for
0), `filename = <name>`, `volume = <w> <h> <d>`, `voxelsize = <x> <y> <z>`, `xras   = ...`,
`yras   = ...`, `zras   = ...`, `cras   = ...`, the numbers as `%.15e` of the floats). A surface
read is raw: its counts, coordinates, faces, flags and tags; completing it (topology, metric
properties) is the caller's work (`MRISreadOverAlloc`, later).

## 73. The surface readers' tags

After the faces, unless `FS_SKIP_TAGS` is set, both readers read tags until `TAGreadStart` returns
0 (the end of the file, also inside a tag number: then the number is not used):
- 32 `GROUP_AVG_SURFACE_AREA`: one float (whatever the tag's length says) into the surface's
  group-average area; the quadrangle reader then prints `reading group avg surface area <a / 100,
  %2.0f> cm^2 from file`.
- 20 `OLD_SURF_GEOM` (no length): the volume-geometry block into the surface's geometry.
- 2 `OLD_USEREALRAS` (no length): an `int` (`freadIntEx`) into the real-RAS flag; 0 when the file
  ends in it.
- 3 `CMDLINE`: the next command line. When the surface holds more than 1,000 already: `<who>(<file>):
  too many commands (<n>) in file`, exit status 254 (`ERROR_NOMEMORY`; `<who>` `MRISread` for the
  triangle reader, `__MRISreadQuadrangleFile` for the quadrangle reader). Otherwise a zeroed block of
  `len + 1` bytes, `len` bytes read into it (the file's bytes as far as it holds them), the line its
  bytes before the first NUL; the count grows by one. The triangle reader, when the block cannot be
  allocated (a length below -1, or 2^63 - 1): `MRISread(<file>): could not allocate <len> byte
  cmdline`, exit status 254, `Cannot allocate memory` first (FS-177: FreeSurfer prints the length's
  low 32 bits as a signed number; larmorx the length, `reproduce` the low bits).
  - **FS-346** (a length of -1, both readers): FreeSurfer reads the rest of the file into a block of
    no bytes (memory corruption when more than a few bytes follow: observed aborts later). larmorx
    ends the program by default (`<who>(<file>): a command line of length -1`, exit status 251);
    `reproduce` takes the rest of the file as the line (its bytes before a NUL; counted, not
    compared).
  - **FS-347** (both readers): the 1,001st command line goes past FreeSurfer's list of 1,000, over the
    surface's command-line count and group-average area. larmorx ends the program in both modes
    with the message above for 1,000 (`... too many commands (1000) in file`): FreeSurfer's values
    there are a memory address.
  - **FS-348** (the quadrangle reader): it does not check the allocation: a length that cannot be
    allocated ends FreeSurfer with a segmentation fault (observed: signal 11). larmorx gives the
    triangle reader's message by default; `reproduce` ends with status 139.
- 22 `SURF_DATASPACE`, 23 `SURF_MATRIXDATA`, 24 `SURF_TRANSFORMEDSPACE` and every other tag: skipped
  (`TAGskip`: its messages and FS-177/FS-178 hold).

## 74. The triangle file, read (`mrisReadTriangleFile(fname, nVFMultiplier)`)

Opens the file (`mrisReadTriangleFile(<file>): could not open file`, `ERROR_NOFILE`, `NULL`); reads
3 bytes (the magic number, not checked); unless `TRIANGULARSURFACE_NOEXTRA_READ` is set, one line
(`fgets` of up to 199 bytes) and the white space after it (`fscanf(fp, "\n")`: also bytes of the
counts that happen to be white space); the vertex and face counts (`int`s). Either count negative:
on standard error `<BUILD_ROOT>/utils/mrisurf_io.cpp:5520 <file> freadInt returned nvertices:<n> `
and `<BUILD_ROOT>/utils/mrisurf_io.cpp:5521 <file> freadInt returned nfaces:<m> ` (each with a space
before its newline), then exit status 1. Verbose (`DIAG_SHOW` with verbose diagnostics): `surface
<file>: <n> vertices and <m> faces.` A new surface with room for `(int)(nVFMultiplier * n)` vertices
and `(int)(nVFMultiplier * m)` faces (never fewer than the counts; FS-02 for a product outside the
`int` range), `n` and `m` in use, type `MRIS_TRIANGULAR_SURFACE` (3). Per vertex three floats, set
as `MRISsetXYZ` does, its face count 0; then a coordinate whose size is beyond 10000, or that is not
finite, ends the program: `<program>: vertex <v> x coordinate <value>!` (`y`, `z`; `%f`;
`ERROR_BADFILE`, exit status 251), the first in x, y, z order. Per face three `int`s; a vertex
number below 0 or not below the vertex count ends the program: `f[<f>]->v[<k>] = <v> - out of
range!` and its newline (exit status 251); then each of the face's three vertices counts the face
(one byte: FS-147, `mrisReadTriangleFile(<file>): vertex <v> is in more than 255 faces`, `NULL`, by
default). The real-RAS flag 0; then the tags (§73). Returns the surface.

**FS-354**: a first line of more than 199 bytes (a long user name or creation date): FreeSurfer
reads its remaining bytes as the counts (text: counts of many millions, whose allocations the
machine's memory decides). larmorx reads the line to its newline by default (with the note);
`reproduce` reads as FreeSurfer (counted, not compared). The same holds for §68's two readers.

## 75. The quadrangle file, read (`__MRISreadQuadrangleFile(fname, nVFMultiplier)`)

Opens the file (`__MRISreadQuadrangleFile(<file>): could not open file`, `ERROR_NOFILE`, `NULL`);
the magic number (3 bytes): `0xffffff` (quadrangles, coordinates in hundredths), `0xfffffd`
(quadrangles, float coordinates), anything else the old format (the file is read again from its
start). The vertex count and the quadrangle count (3 bytes each). A vertex count of 0: `ERROR:
MRISread: file '<file>' has 0 vertices!` + newline + `Probably trying to use a scalar data file as a
surface!` + newline (`ERROR_BADFILE`, exit status 251). More than four quadrangles per vertex:
`nquads=<q>,  nvertices=<n>` on standard error, then `ERROR: MRISread: file '<file>' has many more
faces than vertices!` + newline + the same second line (exit status 251). Verbose: `reading <n>
vertices and <2q> faces.` A new surface with room for `(int)(nVFMultiplier * n)` vertices and
`(int)(nVFMultiplier * 2 * q)` faces, `n` vertices and no faces in use, type
`MRIS_BINARY_QUADRANGLE_FILE` (0). Per vertex: `0xffffff`: three `short`s, each divided by 100.0 (a
`double`) and set as a float; otherwise three floats; the old format then a face count (1 byte) and
that many face numbers (3 bytes each) as the vertex's face list (its face count set to the file's;
other formats 0).

Per quadrangle four vertex numbers (3 bytes each, so never negative) `q0 q1 q2 q3`; `which =
nint(sqrt(1.9 * q0) + sqrt(3.5 * q1))` (doubles, §3's face split); even: the triangles `q0 q1 q3`
then `q2 q3 q1`; odd: `q0 q1 q2` then `q0 q2 q3`. Each triangle with no two equal vertices becomes
the next face (its vertices stored first, then each counts the face: FS-147 as above); a triangle
with two equal vertices is not made. **FS-353**: FreeSurfer's loop, after a first triangle with two
equal vertices, tries the same first triangle again: the second triangle is never made. The
quadrangle writer (§77) stores a lone triangle `v0 v1 v2` as `v0 v1 v1 v2`: read back with an odd
split, the triangle is lost. larmorx makes the second triangle by default (noted); `reproduce` drops
it (noted).
- **FS-351**: a vertex number at or above the vertex count but within the surface's room is
  accepted by FreeSurfer (counted in the room's vertex). larmorx refuses the file by default
  (`__MRISreadQuadrangleFile(<file>): face <f> vertex <v> out of range`, `ERROR_BADFILE`, `NULL`);
  `reproduce` accepts it.
- **FS-352**: a vertex number beyond the room: FreeSurfer counts the face in memory outside its
  arrays (observed: crashes). larmorx refuses it by default as above; `reproduce` stores the face
  and does not count it (counted, not compared).
- **FS-349**: more faces than the room (a multiplier below 1): FreeSurfer stores them past its face
  array, then fails `cheapAssert(fno <= mris->max_faces)` (`<BUILD_ROOT>/utils/mrisurf_io.cpp:6443`,
  FS-03). larmorx stores none past the room and fails the same check (both modes; noted).

Then the face count set (`MRISgrowNFaces`), the real-RAS flag 0, the tags (§73). Returns the surface.
The split choice means a quadrangle file read back may divide a quadrangle along its other diagonal
than the two triangles the writer combined (FreeSurfer's design, kept).

## 76. The triangle file, written (`MRISwriteTriangularSurface(mris, fname)`)

The user: `USER`, else `LOGNAME`, else `UNKNOWN`. With `TRIANGULARSURFACE_NOEXTRA_WRITE` set,
`[INFO]: NO TRIANGULARSURFACE EXTRA - created by line & cmds` on standard output. The date:
`currentDateTime()` (`FREESURFER_REPLACEMENT_FOR_CREATION_TIME_STRING` when set, also when empty;
else the local time as `%c`). Verbose: `writing surface file <file>, created by <user> on <date>.`
The file is created (`fopen` `w`; failing: `MRISwriteTriangularSurface(<file>): can't create file`
and its newline, `ERROR_BADFILE`, returned). Written: `0xfffffe` (3 bytes); unless
`TRIANGULARSURFACE_NOEXTRA_WRITE`, `created by <user> on <date>` and two newlines; the vertex and
face counts (`int`s); each vertex's x, y, z (floats); each face's three vertex numbers (`int`s); the
tag 2 and the real-RAS flag (`int`s); the tag 20 and the volume-geometry block; when the
group-average area's size is at least `FLT_EPSILON`, `writing group avg surface area <a / 100, %2.0f>
cm^2 into surface file` on standard output and the tag 32 with length 4 and the area (float);
unless `TRIANGULARSURFACE_NOEXTRA_WRITE`, each command line as the tag 3 with its length including
its NUL and its bytes with the NUL (**FS-350**: a command line the surface does not hold, below its
count: FreeSurfer crashes (signal 11) with what its stream wrote so far; larmorx skips it by
default, `reproduce` ends with status 139); then the space tags:

The space tags (`__MRISwriteTriangularSurfaceTags`): S = the geometry's scanner voxel-to-RAS
matrix (`vg_i_to_r`), T = its tkregister voxel-to-RAS matrix (`TkrVox2RASfromVolGeom`), inverses by
`MatrixInverse`, products by `MatrixMultiply` (their messages and failures as fs.core's).
- Real-RAS flag not 0, geometry valid: data space `NIFTI_XFORM_SCANNER_ANAT`, matrix `T * S^-1`,
  transformed space `NIFTI_XFORM_UNKNOWN`. Geometry not valid: the identity, both
  `NIFTI_XFORM_SCANNER_ANAT`.
- Real-RAS flag 0, geometry valid: `NIFTI_XFORM_UNKNOWN`, `S * T^-1`, `NIFTI_XFORM_SCANNER_ANAT`.
  Not valid: with a Talairach matrix of 4 x 4 (`SRASToTalSRAS_`) a copy of it, `NIFTI_XFORM_UNKNOWN`
  and `NIFTI_XFORM_TALAIRACH`; else the identity, both `NIFTI_XFORM_UNKNOWN`.
- A matrix that cannot be made (an inverse fails: a geometry of a zero voxel size) where the two
  spaces differ: `[ERROR] __MRISwriteTriangularSurfaceTags(): couldn't obtain MatrixData` on standard
  output, exit status 1 (the file as the stream wrote it).
Written: tag 22 with the data space's text (its length, no NUL), the matrix tag 23 (`TAGwriteMatrix`'s
text), tag 24 with the transformed space's text. The file is closed; returns `NO_ERROR`.

## 77. The quadrangle file, written (`__MRISwriteQuadrangleFile(mris, fname)`)

The file is created (`__MRISwriteQuadrangleFile(<file>): can't create file` and its newline,
`ERROR_BADFILE`). Written: `0xfffffd` and the vertex count (3 bytes each); the faces go in order:
face k and face k + 1 that `quadCombine` (§69) combines become one quadrangle (k advances by two),
else face k alone is the quadrangle `v0 v1 v1 v2`; their number (3 bytes); each vertex's x, y, z
(floats); each quadrangle's four vertex numbers (3 bytes each); the tag 2 and the real-RAS flag; the
tag 20 and the geometry block; the group-average area as in §76; every command line as in §76
(FS-350; `TRIANGULARSURFACE_NOEXTRA_WRITE` does not apply here). Returns `NO_ERROR`.

## 78. Validation plan, part 11

The probe's `tri` command (`validation/probes/fs-surfio/`): as `stl` (§72), each case in an empty
work folder with `d`; Gdiag's two bits each one in two; `TRIANGULARSURFACE_NOEXTRA_READ` and
`TRIANGULARSURFACE_NOEXTRA_WRITE` together one case in six (a file read without the creator line it
was written with gives counts from text), `FS_SKIP_TAGS` one in eight, `USER` one in two, `LOGNAME`
one in three (a name of 1 to 12 printable bytes), the creation time's replacement always (a date,
empty, or `x y`); 1 to 3 operations: the triangle reader on generated files (the creator line, one
in 40 of more than 199 bytes (not with seed 1), one in 13 without its blank line; counts of 0 to 6
vertices and 0 to 6 faces, now and then negative; coordinates mostly sixteenths, now and then
special or beyond 10000; faces in strips of two that share an edge reversed, a vertex out of range
one face in 25), the quadrangle reader (each magic number or none; 0 vertices or too many
quadrangles one file in 25; shorts or floats; the old format's per-vertex face lists; quadrangles
with a repeated middle vertex three in ten, a vertex at or past the count one in ten), both on the
files a writer of the case wrote half the time, with multipliers 1, 1.1, 1.5, 2 (0.5 one in 40); the
tags in any order (real-RAS flag, geometry blocks with a line now and then missing, group-average
area, command lines with lengths exact, 0, -1, below -1, 2^63 - 1, and as the last tag shorter or
longer, the space tags, unknown tags, a partial tag number last); cut files one in nine; the two
writers on a surface of 0 to 6 vertices and 0 to 6 faces with a real-RAS flag of 0, 1 or 2, a volume
geometry (invalid one in four; voxel sizes of 0 one in 30; axis-aligned or random direction
cosines), a Talairach matrix of 4 x 4, 3 x 4, 4 x 3 or 1 x 1 or none, a group-average area of 0 (half
the time), a whole number of quarters, a value below `FLT_EPSILON`, NaN or infinity, 0 to 3 command
lines and a count one too large one in 30. Then every file in the folder. Cases noted with FS-346,
FS-352 or FS-354 (and fs.io's FS-49, FS-167) are counted, not compared. Observed (relay 14): seeds 7
(1,000 cases), 13 (1,000) and 17 (2,000): every comparable case identical with `reproduce` (36, 54 and
80 counted, not compared), the default different only with notices.
