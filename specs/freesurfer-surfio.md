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
with their tags; **part 12** (queue item 3.1-48, §79-84): a surface file's type from its name, the
curvature and annotation file names, a surface written in the format its name says, the ASCII patch,
a surface file's vertex positions only. Later parts: reading surfaces through their format's identification (with the
GIFTI surfaces of 3.1-26), curvatures, patches,
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

## 79. A surface file's type from its name (`MRISfileNameType(fname)`)

A name the volume probe takes as a volume by its ending (`nii`, `mgz`, `mgh`, `img`, `bhdr`,
`nii.gz`, `_000.bfloat`, `_000.bshort`: `specs/freesurfer-io.md`'s `IDextensionFromName`, with its
FS-226) is type 9 (`MRIS_VOLUME_FILE`). Otherwise the type comes from the name's last part (after
its last `/`, cut at its last `#`, then at its last `@`): the text after an `@` still in it (only a
part that held two can have one), else after its last `.`, else nothing; in upper case: `ASC` 1
(ASCII triangles), `GEO` 2 (movie.byu), `TRI` or `ICO` 4 (icosahedron), `VTK` 5, `STL` 6, `GII` 7
(GIFTI), `MGH` or `MGZ` 3 (a volume in a surface's place: only names the volume test did not take,
such as `x.MGH`), `ANNOT` 8, `LABEL` 10, anything else 0 (the binary quadrangle or triangle file).
So the `@<type>` suffix the source's comments describe never forces a type: the part after the last
`@` is gone before the test (kept and documented, rule C: the readers open the name with its `@`
anyway). **FS-356:** a last part of 1,024 bytes or more overruns FreeSurfer's buffer (undefined:
observed crashes); larmorx takes it whole in both modes.

## 80. The curvature file names

`MRISwriteCurvature_getfilename(mris, sname, out)` returns a type and writes the chosen name into
`out`:
- A name without a `/`: in the folder of the surface's own name (everything before its last `/`, or
  `.` without one), or in `.` when `FS_POSIX` is set (any value): `<folder>/<sname>` when the name's
  first dot is its third byte and the two before it are `lh` or `rh`, else `<folder>/<hemi>.<sname>`,
  `<hemi>` `lh`, `rh`, `both` or `unknown` for the surface's hemisphere 0, 1, 3 or any other value.
- A name with a `/`: its folder (before its last `/`) and its last part (cut at its last `#` and
  `@`), joined with or without `<hemi>.` by the same test on the last part.
- Each built name longer than 1,023 bytes is cut to 1,023 with `MRISwriteCurvature_getfilename:
  Truncation on line <n>` on standard error, `<n>` 6720, 6726 (`FS_POSIX`, `/` added hemisphere
  and no hemisphere: 6720 and 6735; the surface's folder: 6726 and 6741; a name with a `/`: 6753
  with the hemisphere added, 6759 without) (**FS-126**: larmorx keeps the name whole; `reproduce`
  cuts it with the message). **FS-356** for a surface name, a folder or a last part of 1,024 bytes
  or more.
- The type: the volume probe's answer for the name (`mri_identify`), replaced by 35 (`ASCII_FILE`)
  when §79 gives type 1; then any type but 3 (MGH), 24 (NIfTI), 27 (GIFTI), 35 and 28 (VTK) becomes
  -1; for -1 the GIFTI rule for writers applies (§60's `__MRISapplyFSGIIwrite`: with `FS_GII=.gii`
  the name gains `.gii` and the type is 27, and so on, with its messages and exits).

`MRISreadCurvatureFile_getfilename(mris, sname, out)`:
- A name without a `/`: `./<sname>` when `FS_POSIX` is set (cut as above, line 6795); else in the
  surface's folder, `<folder>/<sname>` when the two bytes before the name's first dot are `lh` or
  `rh` (line 6804), else `<folder>/lh.<sname>` for a left surface (hemisphere 0) and
  `<folder>/rh.<sname>` for any other (line 6812). **FS-148:** a first dot at the name's first or
  second byte makes FreeSurfer compare one or two bytes before the name; larmorx takes them as no
  hemisphere letters in both modes (the hemisphere is added).
- A name with a `/`: as it is; 1,024 bytes or more end FreeSurfer with `*** buffer overflow
  detected ***: terminated` (signal 6) (**FS-126**: larmorx takes it whole; `reproduce` ends so,
  exit status 134).
- The type as the writer's, but 25 (a new-format curvature: the file starts with `0xffffff`) is
  kept too; for 25 and -1 the GIFTI rule for readers applies (§60's `__MRISapplyFSGIIread`).

## 81. The annotation file names

`MRISwriteAnnotation_getfilename(mris, sname, out)`:
- A name without a `/`: its last part (cut at its last `#` and `@`) in `<folder>/../label/`
  (`<folder>` the surface's, as in §80), or in `./` when `FS_POSIX` is set; after `lh.` (a left
  surface) or `rh.` (any other) unless the name starts with those two letters (case matters). Cut
  at 1,023 bytes with the truncation message (lines 6865 and 6872 without the hemisphere added,
  `FS_POSIX` first; 6882 and 6890 with it) (**FS-126**).
- A name with a `/`: as it is (FS-126's abort at 1,024 bytes, as in §80).
- Then the volume probe's answer for that name; unless 3, 24, 27 or 33 (an annotation), `.annot` is
  added (1,024 bytes or more after it: FS-126's abort) and the type is 33; for 33 the GIFTI rule for
  writers applies.

`MRISreadAnnotation_getfilename(mris, sname, out)` (the surface may be absent):
- A name without a `/` with a surface: its last part in `<folder>/../label/`, after `lh.`/`rh.` as
  for the writer (lines 6940 without, 6948 with; `FS_POSIX` plays no part here). **FS-355:**
  FreeSurfer adds the hemisphere unless the last part is exactly `lh` (left surface) or `rh` (any
  other), in any case: `lh.aparc.annot` becomes `label/lh.lh.aparc.annot`, a file the writer never
  wrote for that name (rule A: larmorx uses the writer's test, the name starting with the two
  letters, case matters; `reproduce` FreeSurfer's).
- A name with a `/`, or no surface: as it is (FS-126's abort at 1,024 bytes).
- When that file is not readable: `<name>.mgz` when it is readable (FS-126's abort if that name has
  1,024 bytes or more), else `sname` itself (relative to the program's folder) when it is readable
  (cut at 1,023 bytes with the message of line 6969).
- Then as the writer: the volume probe's answer, `.annot` and type 33 unless 3, 24, 27 or 33; for 33
  the GIFTI rule for readers applies.

## 82. A surface written in the format its name says (`MRISwrite(mris, name)`)

The name (1,024 bytes or more: FS-126's abort) gives a type (§79). Types 1, 4, 2, 6, 5 and 7 are
taken as they are; for any other the GIFTI rule for writers applies first (it may make the name a
`.gii` name, or end the program when the other form of the name exists: §60). Then the (possibly
changed) name's type decides: 1 the ASCII writer (§54's `MRISwriteAscii`), 5 the VTK writer
(§54's `MRISwriteVTK`), 2 the movie.byu writer (§63), 4 the icosahedron writer (§63), 6 the STL writer
(§71; the answer 1 when written, 0 when not), 7 the GIFTI writer (with queue item 3.1-26's surface
part); any other: the triangle file (§76) when the surface's type is 3 (a triangular surface, as the
triangle reader marks it), else the quadrangle file (§77). The answer is the writer's. (FreeSurfer
8.2.0 keeps an older behaviour behind a switch that is never on: not reachable.)

`MRISwriteVertexLocations(mris, fname, which)`: the surface's current coordinates are kept, the set
`which` is restored as the current one (§25's restore, with its rules), the surface is written
(`MRISwrite`), then the kept coordinates are put back as the current ones (the distances freed, as
§16's import does). Returns `MRISwrite`'s answer.

## 83. The ASCII patch and a surface file's vertex positions

`MRISwritePatchAscii(mris, fname)`: creates the file (`MRISwritePatchAscii: could not open file
<file>`, -1); writes `#!ascii version of patch <surface name>. The 1st index is not a vertex number`,
the numbers of unripped vertices and unripped faces (`<n> <m>`); on standard output
`nvertices=<n> (valid=<the number of valid vertices: not ripped, `MRISvalidVertices`>) nfaces=<m>`; per unripped vertex `<i>
vno=<vno>` (`<i>` = `vno + 1`, negated for a border vertex) and `<x>  <y>  <z>` (`%f`); per unripped
face its number and its three vertex numbers each followed by a space. Returns 0.

`MRISreadVerticesOnly(fname)`: names of type 1, 4 or 2 are refused (`MRISreadVerticesOnly: file type
<t> not supported`, -3, nothing returned); a file that cannot be opened: `MRISread(<file>): could
not open file` (-1). The first 3 bytes decide: `0xffffff` (shorts), `0xfffffd` (floats), `0xfffffe`
(the triangle file: §68's positions reader; when it fails, `mrisReadTriangleFile failed.` with its
code), anything else the old format (read again from the start: floats). Then the 3-byte vertex and
quadrangle counts (with Gdiag's show and verbose bits, `reading <n> vertices and <2q> faces.`;
`new surface file format` and `surfer: old surface file format` likewise for those formats); 0
vertices: `MRISreadVerticesOnly: could not allocate surface` (-2), nothing returned; else per vertex
three big-endian shorts divided by 100 (the division in double, the result as float) or three
big-endian floats; fs.io's rules at the end of the file hold (FS-167). FreeSurfer leaves the file
open (larmorx closes it; nothing a caller sees differs).

## 84. Validation plan, part 12

The probe's `fnam` command: each case in an empty work folder with `d`, `d/s` and `label`;
`FS_POSIX` one case in four, `FS_GII=.gii` one in four, `FS_GII=x` one in eight; up to 5 of 16
files (new-format curvatures, curvature, GIFTI and annotation files in `d`, `label` and the work
folder); the surface's hemisphere 0, 1, 2, 3, -1 or 7 and name (`d/lh.white`, `lh.white`, empty,
`d/`, `/x`, ...); 1 to 4 operations among the type test and the five name builders (the annotation
reader with and without a surface) on names made of a folder (none, `d/`, `label/`, a missing
folder, `./`, `d/s/../`), a prefix (`lh.`, `LH.`, `Lh`, `xlh.`, `.`, `h.`, `lh`, ...), a base and an
extension (each one of §79's, the volume endings, `@ASC`, `#3`, `mgz` without a dot, `@x@vtk`, ...),
one in 30 a name of 990 to 1,060 bytes; each answer and chosen name. Observed (relay 15): seeds 7
(1,000 cases), 13 and 17 (2,000 each): every comparable case identical with `reproduce` (14, 37 and
40 counted, not compared: FS-356), the default different only with notices.

The probe's `swr` command: an empty work folder with `d`; Gdiag's two bits;
`TRIANGULARSURFACE_NOEXTRA_WRITE` one in four, `USER` one in three, the creation time's replacement
and FreeSurfer's licence check's side door always; a surface of 1 to 6 vertices (none one in ten) and
0 to 6 faces, per vertex current, white and pial coordinates (as `tri`'s), ripped one in five, a
border one in four, faces ripped one in six, of type 0 or 3, a real-RAS flag, a volume geometry and
a name; 1 to 3 operations: the writer (three in six) on names of every §79 type but GIFTI and the
volume names (`x.mgz` is written as a surface file), the vertex-locations writer with sets 4, 9, 7,
2, 0, the patch writer, the vertex-positions reader on the files the case wrote or on generated
files (each quadrangle form, the old format, a triangle file; cut now and then); the other form of
a name (`<name>.gii`) there before a write one in ten. Then the surface's current coordinates and
every file in the folder.

## 85. Reading a surface (`MRISread(fname, dotkrRasConvert)`, `MRISfastRead(fname)`)

`MRISfastRead(fname)` is `MRISread(fname, true)`; `dotkrRasConvert` defaults to true.

1. A name of type 9 (a volume name, §79) is the fatal error `MRISread(): input <name> does not
   appear to be a surface (ext=<the text after the name's last dot>, MRIS_VOLUME_FILE)` (code -5).
2. The name is copied into a 1,024-byte buffer (FS-126's abort at 1,024 bytes or more).
3. For a type other than 1, 4, 2, 6, 5 and 7 the GIFTI rule for readers (§60) may change the name
   to `<name>.gii` and the type to GIFTI (in particular a binary surface name that cannot be read
   becomes `<name>.gii`).
4. The surface is read by its type (§86, no extra room). Nothing read: nothing returned.
5. The surface's `orig_xyzspace` becomes its `useRealRAS`. When `useRealRAS` is set and the
   conversion is asked for: a volume geometry that is not valid prints `ERROR: Surface <name>
   doesn't have valid volume geometry!` on standard output, frees the surface and returns nothing;
   a valid one moves the surface to tkregister space (§90).
6. The neighbourhoods to 3 hops with their distances (§88), then the neighbourhood in use reset to
   1 hop (§88). The surface is returned.

## 86. The reader by type (`MRISreadOverAlloc(fname, nVFMultiplier)`)

FreeSurfer 8.2.0 always takes its new reader (an older one sits behind a switch that is never on).
The licence check passes (the side door). The name's type (§79) chooses:

| type | reader | on failure |
|---|---|---|
| 1 (ASCII) | §66 | nothing returned |
| 4 (icosahedron) | §89, with room for `nVFMultiplier` times the counts, scale 1; **its surface is returned as it is**, none of the steps below | nothing returned |
| 2 (movie.byu) | §67 | nothing returned |
| 6 (STL) | §70 | nothing returned |
| 5 (VTK) | §62, no surface given | nothing returned |
| 7 (GIFTI) | GIFTI surfaces (queue item 3.1-26's surface part); a file that cannot be opened prints `** failed to open GIFTI XML file '<name>'` and `mrisReadGIFTIdanum: ERROR reading gifti <name>` (standard error) | nothing returned |
| 3 (`MGH`/`MGZ` without a volume ending) | the fatal error `ERROR: MRISread: cannot read surface data from file <name>!` (code -5) | |
| any other | by the file's first 3 bytes (below) | |

Any other type: the file is opened (`MRISread(<name>): could not open file`, -1, nothing returned).
Fewer than 3 bytes: `ERROR: reading <name>` and `Read 0 bytes, expected 1` on standard output,
nothing returned. `0xfffffe`: the triangle file (§74; its failure prints `mrisReadTriangleFile
failed.` with the reader's error code, nothing returned). `0xffffff` (with Gdiag's show and verbose
bits, `new surface file format` on standard output) and `0xfffffd`: the quadrangle file (§75).
Anything else: the old format (`surfer: old surface file format` likewise), read by §75 from the
start.

Then, for every reader but the icosahedron:

1. The surface's name is the name read; its **hemisphere** comes from the first byte of the name's
   last part (after its last `/`), in upper case: `R` right (1), `L` left (0), `B` both (3),
   anything else (an empty last part too) none (2).
2. **Face lists.** Every vertex's face list is made again, of the size the reader counted, and
   filled by going through the faces in order: each face appended to the lists of its three
   vertices in its vertex order. Not for the old format (FS-359: FreeSurfer keeps the file's lists,
   whose count is too large; by default larmorx rebuilds them as for every other format). FS-360:
   a count that wrapped around (FS-147 reproduced) gives lists too small for the faces. A face
   vertex at or above the vertex count (FS-351 reproduced) ends the program with a segmentation
   fault.
3. Every vertex's curvature 0, original area -1, border 0; for each face in its list, the vertex's
   place in that face (0, 1 or 2; the last place when the vertex is in a face twice; 0 when it is
   not in it).
4. The **neighbour lists** (§87).
5. The bounding box and centre (§16's `mrisComputeSurfaceDimensions`), the neighbour distances at
   the surface's number of hops (1: §20), the vertex normals (§40's computation on the surface).
6. The **Talairach transform**: with the name's folder `P` (the text before its last `/`, `.`
   without one; a name of 300 bytes or more: FS-356), when `P/../mri/transforms/talairach.xfm`
   can be opened (a folder of that name can), it is read by fs.volume's transform reader (`specs/
   freesurfer-volume.md`, reading transforms: an `.xfm` by its name; its messages as that reader
   gives them) and completed as §92 says; otherwise nothing.
7. `radius` the mean radius (§90); the metric properties (§38); the current coordinates stored as
   the temporary set (§25's `TMP_VERTICES`: `tx`, `ty`, `tz`).
8. When `<name>.avg.area.mgh` is readable (`Gdiag_no` >= 0 with verbose diagnostics first prints
   `Trying to read average area <file>`), it is read as a volume and copied as the vertices'
   `group_avg_area`, `group_avg_vtxarea_loaded` 1 (a volume that cannot be read prints `ERROR:
   reading in average area <file>` and returns nothing); otherwise `group_avg_vtxarea_loaded` 0
   (then `Average area loaded <flag>` with the same diagnostics).

## 87. The neighbour lists from the faces

By default (the environment variable `mrisCompleteTopology_new` unset; read at the first call):
for each vertex in order, its 1-hop count is cleared (§5's message rules hold for each change),
then for each face in its face list in order, with `p` the vertex's place in that face, the face's
vertex at place `p - 1` (2 for `p` 0) and then the one at place `p + 1` (0 for `p` 2) are appended
to the vertex's list when not in it yet. The list is exactly that long; the count in use is all of
it; the vertex's largest and current sizes are 1. With Gdiag's show and verbose bits, `finding
surface neighbors...` (no newline) on standard output first. Then for each face in order and each
of its places, the face must be in that vertex's face list, else the fatal error
`mrisFindNeighbors: <surface name>: face[<f>].v[<k>] = <v>, but face <f> not in vertex <v> face
list` (code -4). `avg_nbrs` is the float division of the sum of the counts in use over the unripped
vertices by their number (the default NaN without one).

With `mrisCompleteTopology_new` set: each face in order is attached to its three vertices: the
face and vertex numbers must lie in the surface (failed assertions
`<BUILD_ROOT>/utils/mrisurf_topology.cpp:2106 (0 <= fno) && (fno < mris->nfaces)`, `:2107` to
`:2109` `(0 <= vno0) && (vno0 < mris->nvertices)` with `vno1`, `vno2`); the three vertices must
differ (`:1873 vno0 != vno1 && vno0 != vno2 && vno1 != vno2`) and none of its three edges may
already belong to two faces of the first vertex's or second vertex's face list (`:2111
mrisCanAttachFaceToVertices(mris, vno0, vno1, vno2)`: every closed surface fails here); each
missing edge (the pairs 0-1, 1-2, 2-0) is added: each vertex gets the other at the end of its list
(sizes 1, count in use all of it; `adding edge <a> <--> <b>` on standard output with Gdiag's show
and verbose bits; a count above the largest so far, 20 at first, becomes the largest with the
debug message `vertex <v> has <n> immediate neighbours`). `avg_nbrs` as above, at least one vertex
counted.

## 88. Neighbourhood sizes and the 2- and 3-hop neighbours

A vertex keeps its neighbours **sorted by hops**: the first `vnum` 1 hop away, up to `v2num` 2 hops,
up to `v3num` 3 hops; its **largest size** (`nsizeMax`) says which of those counts are known, its
**current size** (`nsizeCur`) which are in use (`vtotal` the count of that size, 0 for size 0).

**Setting the current size** of a vertex to `n`: `n` above its largest is a failed assertion
(`<BUILD_ROOT>/utils/mrisurf_topology.cpp:821 nsize <= vt->nsizeMax`); `n` outside 0-3 fails `:828
false`.

**Resetting the surface's size** to `n`: for every unripped vertex, the current size `n` (1, 2 or
3); for any other `n`, the vertex's largest (which must be above 0, `:971 vt->nsizeMax > 0`, and the
same for every vertex, `:974 new_mris_nsize == vt->nsizeMax`; the surface's size becomes that
largest when `n` is negative). The surface's size becomes `n`; `avg_nbrs` as in §87.

**Finding a vertex's neighbours within `h` hops** (into a list of a given capacity, 1,024 here),
by default (the environment variables `MRISfindNeighborsAtVertex_new` and `_old`, read at the first
call, choose; neither set: the new search):

- The vertex must not be ripped (`:1246 !v->ripflag`), `h` at most 50 (`:1250 nlinks <=
  MRIS_MAX_NEIGHBORHOOD_LINKS`), and its largest size above 0 (`:1271 vt->nsizeMax > 0`).
- The rings it knows (1 hop, and 2 and 3 up to its largest size) are taken from its list in order,
  up to `h`: each unripped vertex listed and marked found (the centre is marked found first). A
  ripped vertex there makes that ring the last known one (it is searched again below); FS-357:
  FreeSurfer then searches the next ring from list entries it did not fill; by default the ring ends
  where it was filled.
- `h` must be at least 1 (`:1310 ringLinks >= 2`). Each further ring, up to `h`: for each vertex of
  the previous ring in list order, its 1-hop neighbours in their list order that are unripped and
  not yet found are found, in that order.
- Entries past the capacity are counted, not written. Found more than the capacity:
  `MRISfindNeighborsAtVertex: vno:<v> has <n> neighbours, but listCapacity is <capacity>` on
  standard output, the search done again showing each vertex found (`  vnoCandidate:<v> known to
  be in ring:<ring - 1>` for the known rings, `  vnoCandidate:<v> is neighbour of vno:<w> in
  ring:<index of w in the list>` for the others), the vertex-vertex check (below, always), then
  `:1409 false`. The vertices found past the capacity stay marked found for later searches in the
  same program (the set of found vertices is kept from call to call and cleared only for the
  listed vertices and the centre; it is cleared whole when it grows to a larger surface).
- The rings found are kept in the vertex when it knew fewer than `min(3, h)`: its list grows to the
  vertices within `min(3, h)` hops (its entries past the known rings replaced; its distances grown
  to that length when it has distances), `vnum` (when 1-hop was searched again), `v2num`, `v3num`,
  its largest size `min(3, h)`, its count in use that of its current size.
- With `h` below 4, each of the vertex's first neighbours (as many as were found) must be a vertex
  of the surface (else `<BUILD_ROOT>/utils/mrisurf_topology.cpp:1133 Bad
  mris->vertices_topology[<v>].v[<i>]` and `:1134 false`).

The old search (`MRISfindNeighborsAtVertex_old` set): 0 for a ripped vertex; at its first call
`<BUILD_ROOT>/utils/mrisurf_topology.cpp:1426 checking marks initially clear` on standard output;
every vertex's mark must be 0 at each call (`:1429 mris->vertices[n].marked == 0`); the vertex's
neighbours in use are listed and marked with their hops (by their place in its list), the vertex
marked -1. Within the surface's size: the count at `h` hops (`h` must not exceed the vertex's
current size, `:1440 nlinks <= vt->nsizeCur`; `h` 3 sets the vertex's count in use to `v3num` and
answers 0, leaving the marks; below 1: the fatal error `MRISfindNeighborsAtVertex: nlinks=<h>
invalid`, code -4). Beyond it: rings added from the listed vertices (all of them again for each
ring, in order, ripped ones skipped) by their 1-hop neighbours not yet marked. The answer's hops
are the marks, then cleared. It keeps nothing in the vertex. FS-358 past 1,024 entries. Both set:
both searches run (the new one into its own lists of 1,024), `<BUILD_ROOT>/utils/mrisurf_topology.cpp:1143Testing
MRISfindNeighborsAtVertex` once, then on any difference ` result_old:<a> != result_new:<b>` or
` vlist[<i>] old:<a> != new:<b>  or  hops[<i>] old:<c> != new:<d>` lines, `<BUILD_ROOT>/utils/mrisurf_topology.cpp:1105
laterTime:<calls> interestingLaterTime:0` and `:1162 false`; the old search's answer.

**The surface's size and distances to `n` hops** (`MRISsetNeighborhoodSizeAndDist(mris, n)`;
`MRISsetNeighborhoodSize` without forcing the distances): `n` must be 1, 2 or 3
(`<BUILD_ROOT>/utils/mrisurf_metricProperties.cpp:4276 1 <= nsize && nsize < 4`). Up to the
surface's largest size: the size reset to `n`. Beyond it: each vertex's neighbours within `n` hops
found and kept (above, capacity 1,024; vertices in order), the size reset to `n`, the surface's
largest size `n`. When the distances are known to fewer than `n` hops, the distances are computed
at the surface's size (§20; the original ones too when they are kept). Then: without distances
(`dist_alloced_flags` bit 1 clear) the distances' size must be 0 (`:4260 mris->dist_nsize ==
0`): **a surface without any edge fails here** (no vertex had distances to compute, yet the size
became `n`); with them at least `n` (`:4261`); `:4308 mris->nsize == nsize`, `:4345`, `:4346`
check the same.

**The vertex-vertex check** (`mrisCheckVertexVertexTopologyWkr`; run always only by the overflow
above): `FREESURFER_always_check_vv` (read at the first call; `<BUILD_ROOT>/utils/mrisurf_topology.cpp:52
FREESURFER_always_check_vv set`) makes it run always; otherwise only when its place's check is due
(§1's logic-problem thinning). Each problem is the logic problem `Bad vertex or face found` (at
the check's line; fixed, which fails `:38 false`, when `FREESURFER_crash_on_bad_topology` is set),
and its line is printed on standard output only when the problem was reported: `mris->nsize:<a> >
mris->max_nsize:<b>` (line 62); per vertex `dist or dist_orig non-null when !mris->dist_alloced`
(91); for unripped vertices `[vno1:<v>].nsizeCur:<c> != mris->nsize:<n> vSize:<s> vnum:<k>
vtotal:<t>` (103), `[vno1:<v>].nsizeCur:<c> exceeds nsizeMax:<m>` (111), `[vno1:<v>].vtotal:<t>
differs from expected:<e> for nsize:<c>, ripflag:0` (123), and per neighbour `[vno1:<v>].v[<i>] is
bad vno2:<w>` (133), `[vno1:<v>].v[<i>] not found in [vno2:<w>].v[*]` (142), `[vno1:<v>].v[<i>]:<w>
same as [vno1:<v>].v[<j>]:<w>` (151). With `FREESURFER_checkDist` (`... :163 checking dist[]
contents`) the distances are checked (§20).

## 89. The icosahedron file (`ICOread(fname)`, `ICOreadOverAlloc(fname, nVFMultiplier, scale)`)

**The file** (text; lines read 149 bytes at a time with comments and blank lines passed over, §59's
line reader): a line whose first integer is the vertex count; vertex lines of 4 numbers
(`number x y z`: the vertex number, from 1, a float truncated toward zero) or 6 (`x y z nx ny nz`:
the vertices in order), until a line of another number count or as many lines as vertices; a line
whose first integer is the face count (at the end of the file the last line read gives it: the line
reader's buffer); face lines of 4 integers (`number a b c`) or 3 (`a b c`: the faces in order), until
a line of another count or as many lines as faces. Unread vertices and faces stay 0. A file that
cannot be opened: `read_icosahedron: could not open <name>` (-1), then `ICOreadOverAlloc(<name>):
could not open file` (-1), nothing returned. A negative count is the fatal error `read_ico: could
not allocate vertex list` (code -2, `errno` ENOMEM: `Cannot allocate memory` printed first; the
face count gives the same text). FS-362 (a count that cannot be read, a vertex or face number outside
the count, a face vertex outside the vertices): refused by default (`read_icosahedron(<name>):
<what>`, -5), then `ICOreadOverAlloc`'s message.

**The surface**: each face's second and third vertices are swapped; a surface with room for
`nVFMultiplier` times the counts (§30), the vertices' coordinates `scale ⊗ x` (f) each; each face's
vertices from 0. Each vertex's 1-hop list: for each face in order, at each of its places in order,
the face's other two vertices in place order, each appended when not yet listed (FS-361: the list
FreeSurfer allocates holds as many entries as the vertex has faces, so an open surface writes past
it; larmorx's list holds them all). Each vertex's face list in face order with its places; sizes 1,
count in use the 1-hop count. Then the metric properties (§38), `radius` (§90), the surface's kind
3 (`MRIS_ICO_SURFACE`). A vertex of more than 255 faces: FS-361 (refused by default, `ICOread:
vertex <v> is in more than 255 faces`). (§5's messages hold for the counts.)

## 90. The mean radius, scanner and tkregister coordinates

**The mean radius** (`MRISaverageRadius`): the bounding box of every vertex's current coordinates
(ripped ones too), in double, from 10000 (low) and -10000 (high), a coordinate replacing the high
end when it is greater and the low end when it is smaller (NaNs never replace); the centre
`(lo ⊕ hi) ⊗ 0.5` (d) per axis; then the sum, in vertex order, of `sqrt((dx⊗dx ⊕ dy⊗dy) ⊕ dz⊗dz)` (d,
`dx = x ⊖ cx` with the coordinate converted to double first), divided by the vertex count (0 / 0,
the default NaN, without vertices). Where two NaNs meet, the left operand's is kept as written.

**Scanner to tkregister coordinates** (`MRISscanner2Tkr`): nothing when `useRealRAS` is 0;
otherwise every vertex's current coordinates multiplied (§16's matrix multiplication) by
`T × S⁻¹`, `S` the volume geometry's voxel-to-scanner matrix and `T` its tkregister matrix
(fs.volume's §§ for the geometry; FreeSurfer's float 4 × 4 products and inverse; an inverse that
cannot be made leaves no matrix and the multiplication ends the program); `useRealRAS` becomes 0.
**Tkregister to scanner** (`MRIStkr2Scanner`): nothing when `useRealRAS` is set; otherwise
`S × T⁻¹`, then `useRealRAS` 1.

## 91. Validation plan, part 13

The probe's `srd` command: each case in an empty work folder with `d`; Gdiag's two bits; each of
`mrisCompleteTopology_new`, `MRISfindNeighborsAtVertex_new`, `MRISfindNeighborsAtVertex_old`,
`FS_SKIP_TAGS`, `FS_GII=.gii`, `FREESURFER_always_check_vv` and `FS_FASTER_MP` set one case in 14
(the licence check's side door always); one or two input files named `<folder><hemisphere
letters>s<k><extension>` (the work folder or `d/`; `lh.`, `rh.`, `Lh.`, `R`, `b`, `B`, `x` or
nothing): mostly a mesh (a tetrahedron, an octahedron, an icosahedron now and then without a face, a
cube, a fan closed or open, a strip, two tetrahedra sharing a vertex, random faces of 1 to 8
vertices; scaled and moved, a coordinate replaced now and then by a special one, renumbered and
turned now and then) written in one format: the binary triangle file (with random tags, or a
real-RAS tag and a valid geometry, now and then), a quadrangle file of `a b c c` quadrangles (each
magic number and the old format), ASCII (`.asc`, a vertex ripped one in 40), movie.byu, STL (text or
binary), VTK, the icosahedron format (`.tri`, `.ico`, `.TRI`; 4- or 6-number vertex lines in order
or reversed, 3- or 4-number face lines, a face line short, a count off, a bad number now and then);
one in five another reader's random file (§65, §72, §78's generators); one in 300 a tree of 1,321
vertices whose centre has 1,320 within 3 hops; one in 40 the same file read by a name of 300 or more
(seed 1: 300; others now and then 1,040) bytes (`./` repeated); one or two reads (`MRISread` with or
without the conversion, `MRISfastRead`) of those names or, one in 30, of a name without a file (`.asc`,
`.tri`, `.stl`, `.geo`, `.vtk`, a volume name, `.MGH`). Each surface read is printed in full
(counts, kind, room, hemisphere, name, `useRealRAS`, `orig_xyzspace`, patch, status, sizes, the
distances' sizes and flags, `avg_nbrs`, `radius`, the group-average flag, the transform's presence
(with a transform, from relay 17, §93: its type, count, first transform's type, source and
destination geometries and matrix, and the two Talairach matrices), the volume geometry, box, centre, area totals; per vertex its coordinates, normal, area, original
area, curvature, temporary set, border, rip flag, mark, face list with places, the four counts and
two sizes, the neighbour list and both distance lists to its size in use; per face its vertices, rip
flag, area, angles and cached normal). Observed (relay 16): see the probe README.

## 92. The Talairach transform of a surface read

When the transform reader gives nothing, `mrisReadTransform: could not read xform file '<the
transform's name>'` is an error (`ERROR_NOFILE`; with `errno` set, its text first, as every error)
and the read goes on without a transform. A transform whose type is not RAS to RAS (an `.xfm` is
always RAS to RAS) ends the program: `the transform is not RAS-TO-RAS.  not supported.` (code -4).
Otherwise the surface keeps it and, with `D` = Gdiag's show and verbose bits both set, its first
transform's source and destination are completed:

- **Source not valid:** with the surface's own volume geometry valid, the source's centre
  (`c_r`, `c_a`, `c_s`) becomes the surface's (`D`: `INFO: found the orig volume info on <the
  geometry's file name> in the surface data.` on standard error); without it, when
  `P/../mri/orig` is a regular file (not a folder: the usual COR folder is not read) whose header
  can be read, the source and the surface's geometry both become that volume's (width, height,
  depth, voxel sizes, axes, centre, file name, valid; `D`: `INFO: found the orig volume (mri/orig)
  to get c_(ras) information for src` and `INFO: added info to the surface.`); else the source's
  centre is 0, 0, 0 (`D`: four lines `INFO: cannot find mri/orig volume to get c_(ras)
  information.`, `INFO: transform src volume information cannot be found. assume c_(ras) = 0`,
  `INFO: destination surface points may be shifted in the volume.`, `INFO: you should put the src
  info in the transform.`). Nothing else of the source changes (it stays not valid).
- **Source valid** and the surface's geometry valid: when one of the three centre coordinates
  differs by 1e-5 or more (the `float` difference, compared as a double: `fabs(d) < FLT_EPSILON`
  false, `FLT_EPSILON` the C library's 2^-23 here; a NaN difference counts as differing), two lines
  on standard error: `WARNING: the source volume info is not consistent between the info
  contained` and `WARNING: in the surface data (<s_r>,<s_a>,<s_s>) and that of the transform
  (<t_r>,<t_a>,<t_s>).` (each `%f`, the surface's first). Nothing changes.
- **Destination not valid** (`D`: `INFO: transform dst volume information cannot be found.`,
  `INFO: if the target is MNI average_305, then you can do 'setenv USE_AVERAGE305 true'`, `INFO:
  otherwise c_(ras) is set to 0. destination surface points may be shifted.`): with the
  environment variable `USE_AVERAGE305` set (to anything), its centre becomes (-0.0950, -16.5100,
  9.7500), each the `float` nearest the decimal's `double` (it stays not valid). (With that variable
  the transform reader itself has already given a destination average_305's whole geometry: see
  its spec.)

Then the surface's Talairach matrix is §64's `getSRASToTalSRAS` of the first transform (nothing
when it cannot be made), its inverse that matrix's inverse by fs.core's `MatrixInverse` (nothing
for a singular matrix or one with a non-finite element, with that function's messages), and the
surface records that it owns the transform (`free_transform` 1). The read's other steps (§86) do
not use the transform.

## 93. Validation plan, part 14

The probe's `srd` command (§91) from relay 17: each work folder also holds `mri/transforms`; one
case in three gives the subject a `talairach.xfm` (and then two of three surfaces are put in `d/`,
whose transform it is): one in 25 random bytes, otherwise MINC's text with its first line right (4
in 6) or not, 0 to 3 comment lines (FreeSurfer's `%Generated by ... src <a> dst <b>`, minctracc's
command line, MINC's GUI lines, `% avi2talxfm`, a blank line, `%`) naming volumes that do not
exist (a name containing `average_305` gives that brain's geometry), the `Transform_Type` lines or
not, and three rows of four numbers (one in 12 two rows; one in 20 a singular matrix; one number in
60 a word). The probe never sets `USE_AVERAGE305` (the transform reader of fs.volume reads it from
the process's environment) and never writes `mri/orig` as a file. Hand cases (the probe README): the
surfaces with a valid geometry of seed 7, each with eight transforms (a source of average_305, a
destination of it, both, neither, singular, NaN and infinite numbers, a wrong first line, two rows)
and Gdiag's bits off and on. Observed (relay 17): see the probe README.

## 94. Metric properties, part b, first group (queue item 3.1-50; relay 17)

Arithmetic in `float` unless said; "compiled as" gives the operand order where a NaN's payload
depends on it (the first operand's NaN wins in SSE; the reference outputs decide).

- **Distances.** Every Euclidean distance of this group between two positions `a` and `b` is
  `sqrt(dz² + (dx² + dy²))` with `d = a - b` component-wise (`a` the neighbour, the vertex, or the
  face's second common vertex as named below).
- **Common vertices of two faces** (`VERTICES_commonInFaces_find(fi, fj, v)`): the vertices of `fi`
  in its order that `fj` also has, as `float`s in a 3-vector initialised to -1; returns their number
  (0 to 3). **Common edge length** (`FACES_commonEdgeLength_find(mris, fi, fj)`): with exactly two
  common vertices `O` and `I` (the vertex numbers read back through the `float`s), the distance
  from `I` to `O`; any other count ends the program: `FACES_commonEdgeLength_find: No common edge
  found! <commonVertices> = <n>` and a newline, exit code -4 (status 252).
- **Tangent planes** (`mrisComputeTangentPlanes`): for every vertex (ripped ones too), with `n` its
  normal: `e1 = n x (ny, nz, nx)`, the cross product's x as `(b.z a.y) - (a.z b.y)` (`a = n`), y and
  z as written (`a.z b.x - a.x b.z`, `a.x b.y - a.y b.x`); when `sqrt(((0 + x²) + y²) + z²)` of `e1`
  is below 0.001, `e1 = n x (ny, -nz, nx)` instead. With verbose diagnostics and `|(x² + y²) + z²|`
  of `e1` below 1e-15 (as a `double`), `vertex <vno>: degenerate tangent plane` on standard error.
  `e2 = n x e1` with x as `(e1.z n.y) - (e1.y n.z)`, y and z as written. Each normalised: `len =
  sqrt((y² + x²) + z²)`; `s = 1` when `|len| < 2^-23`, else `1 / len`; then `(x s, y s, s z)`.
  Stored in `e1x..e1z`, `e2x..e2z`. Returns 0. **`MRISstoreTangentPlanes(mris, which)`**: each
  unripped vertex's `e1`, `e2` copied into `pe1`, `pe2` (`which` unused); returns 0.
- **Plane fit** (`mrisComputeOptimalPlane(mris, vertices, n, &nx, &ny, &nz, &cx, &cy, &cz)`): the
  unripped listed vertices' positions summed in `double`; fewer than 4 of them: the error
  `mrisComputeOptimalPlane: must specify at least 4 non-ripped verices` (`ERROR_BADPARM`, the
  outputs untouched) and -4. Otherwise a 3 x 3 `float` scatter matrix, each element accumulated as
  `(float)((double)element + product)`, the products `(x - xc)²`, `(y - yc)²`, `(z - zc)²` on the
  diagonal and `(x - xc)(y - yc)` into (2,1) and (1,2), `(x - xc)(z - zc)` into (3,1) and (1,3),
  `(y - yc)(z - zc)` into (3,2) and (2,3), in that order per vertex (`double` products of the `float`
  position minus the `double` centre); fs.core's `MatrixEigenSystem` of it; the third eigenvector
  column made unit length in `double` (`a / sqrt(a² + b² + c²)` ...) is the normal, the centre
  returned. **FS-363**: the original fits the plane about (0, 0, 0) with every listed vertex
  (ripped too) and returns the centre (0, 0, 0); by default the centre is the unripped vertices'
  mean (each sum divided by their number), the scatter taken over them only. Returns 0.
- **Face centroids** (`mrisCalculateFaceCentroid`, `mrisCalculateCanonicalFaceCentroid`; a third,
  original positions, unused): with the face's vertices `v0`, `v1`, `v2` in the positions named,
  `m = (p1 + p2) / 2`, then `(m + p0) / 2` per coordinate.
- **Face-area statistics** (`MRIScomputeFaceAreaStats(mris, &sigma, &min, &max)`): the metric
  properties first (§38); `s = orig_area / total_area` in `float` widened (1 for a patch); over the
  unripped faces `area = (double)face.area * s`: the mean (the `double` sum over the `double` count:
  0/0 a NaN for none), `sigma = sqrt(sum of squares / count - mean²)` (glibc's `sqrt`: a negative
  argument sets `errno` to `EDOM`), the smallest from 1000, the largest from -1 (a NaN area changes
  neither). `MRISread` leaves `orig_area` 0, so a surface just read has every scaled area 0.
- **Vertex-spacing statistics** (`MRIScomputeVertexSpacingStats(mris, &sigma, &min, &max, &vno,
  &vno2, which)`): the metric properties first; when not a patch, `orig_area / total_area` below 0
  sets `errno` to `EDOM` (its square root is computed and discarded); over each unripped vertex's
  1-hop neighbours (`vnum`, ripped neighbours included) the distance from the neighbour to the
  vertex in the positions `which` names: original (0), current (4), white (9), pial (7), target
  (10); any other value ends the program at the first neighbour: `MRIScomputeVertexSpacingStats:
  unsupported vertex set <which>` (`ERROR_BADPARM`, status 252). The `float` distance widened;
  mean, sigma, smallest (from 1000) as above; the largest from -1, each new largest recording the
  vertex and its neighbour (`vno`, `vno2`; written only when one exceeds -1). Returns the mean.
  **`MRIScomputeTotalVertexSpacingStats(...)`**: the same over every neighbour in use (`vtotal`),
  current positions.
- **Negative faces** (`MRIScountNegativeFaces`, `MRIScountNegativeTriangles`): the unripped faces
  whose area is below 0.
- **Tessellation statistics** (`MRISprintTessellationStats(mris, fp)`): the vertex-spacing
  statistics (current) then `vertex spacing <mean> +- <sigma> (<min>--><max>) (max @ vno <vno> -->
  <vno2>)`, the face-area statistics then `face area <mean> +- <sigma> (<min>--><max>)`, each
  number `%2.2f`, into `fp`; when the largest face area exceeds 20, for each 1-hop neighbour of
  vertex `vno` farther than 20 (the `float` distance from the neighbour), on standard output a tab
  and `<vno> --> <neighbour> = <d> mm` (`%2.1f`).
- **Closest vertices**: over the unripped vertices in order, the `float` distance from the vertex
  to the point; the first strictly below the best so far (from 10000). `MRISfindClosestVertex(mris,
  x, y, z, &dmin, which)`: current (4) or canonical (3) positions, any other value ends the
  program at the first unripped vertex (`MRISfindClosestVertex: unsupported vertex set <which>`,
  `ERROR_UNSUPPORTED`); returns the vertex (-1 for none) and gives the distance (10000 for none).
  `MRISfindClosestOriginalVertex`, `...CanonicalVertex`, `...WhiteVertex(mris, x, y, z)`: the
  vertex only.
- **Curvature edits**, each over the unripped vertices: `MRISsetCurvature(val)`; `MRIStrinarize
  Curvature(t)`: below `-t` -1, above `t` 1, else (NaN too) 0; `MRISscaleCurvature(scale)`: the
  mean `m` (`double` sum / count) then `(curv - m) * scale + m` in `double` stored as `float`;
  `MRISbinarizeCurvature(t, low, high, use_abs)`: the value (absolute with `use_abs`) below `t`
  gives `low`, else `-high` for a negative curvature and `high` otherwise; `MRISthresholdCurvature
  (t, use_abs)`: below `t` becomes 0; `MRISnormalizeField(distance_field, which_norm)`: nothing for
  -1; for a distance field or `which_norm` 2 the largest unripped curvature above 0 (from 0), and
  unless its magnitude is below 2^-23, `normalizing max <max> to be in [0 1]` (`%2.1f`) on standard
  output and each curvature divided by it; otherwise `MRISnormalizeCurvature(which_norm)` (the
  curvature normalisation of an earlier part of this spec).
- **`MRIShistoThresholdCurvature(mris, pct)`**: the unripped curvatures' range as `double`s (from
  1000 down and -1000 up); 1,000 bins; `scale = 999 / (max - min)`; bin size `1 / scale` (`float`);
  each unripped curvature counted at `nint(scale * (curv - min))` (fs.core's `nint`, FS-12 for a
  NaN); bin `b`'s value `b / scale + min` (`float`); `nint(scale * (0 - min))` also computed (its
  FS-12 notice only); the counts normalised to sum 1 (fs.core's `HISTOmakePDF`, only when their sum
  is above 0); with `DIAG_WRITE` and verbose diagnostics `curv.plt` (fs.core's `HISTOplot`) in the
  working folder; `z` the highest peak (fs.core's `HISTOfindHighestPeakInRegion` over every bin).
  `T` = the counts from `z` to the last bin summed in `double`; then from `z` up to the last but one
  bin, the first bin at which the running sum exceeds `pct * T` (or the last but one) gives the upper
  limit (its bin value). Likewise downwards: `T` = the counts from `z` down to bin 1, then the first
  bin from `z` down to bin 1 at which the running sum exceeds `pct * T`, or bin 0 when none does
  (bin 0's count never added), gives the lower limit. Each unripped curvature above the upper limit or below the lower one
  becomes it (`float`), counted once per side; `min_curv`, `max_curv` the resulting range (from
  10000 and -10000); with any changed, `<n> vertices thresholded to be in [<low> <high>]` (`%2.2f`)
  on standard error. **FS-364**: all unripped curvatures equal, or one not finite, gives a bin
  outside 0..999: the original crashes (segmentation fault); by default the sample is not counted
  (noted). **FS-365**: no count above 0 (no unripped vertex, or by default every sample refused):
  no peak; nothing is clipped (the range as measured).

## 95. Validation plan, part 15

The probe's `mpb` command (`validation/probes/fs-surfio/`): an empty work folder with `d`; Gdiag's
show, verbose and write bits each one in two; `FS_FASTER_MP` one in six; a binary triangle file
of `srd`'s meshes (one in five scaled by 12) read by `MRISread` as `d/lh.s0`; half the cases set
the surface's original area (a multiple of 4 up to 16,000 or any special value); per vertex an
override mask (ripped one in 8; original, white, pial, target and canonical positions each one in
3; curvature one in 2; normal one in 6; the values small multiples of 1/4 or, in heavy cases, any
float or NaN with payloads); per face ripped one in 8 and its area one in 4; then 1 to 6
operations: the common vertices or edge of a face and (mostly) a face sharing an edge with it, the
tangent planes, storing them, the plane fit of 0 to 8 listed vertices, a centroid, the face-area,
vertex-spacing (each position set and an unsupported one) and total statistics, the negative-face
counts, the tessellation statistics, the closest vertices (current, canonical, unsupported; and the
three others), and every curvature edit; each prints its result and, for the edits and the planes,
every vertex's curvature, `e1`, `e2`, `pe1`, `pe2` and the surface's curvature range. Observed
(relay 17): the probe README.

## 96. Metric properties, part b, second group (queue item 3.1-50; relay 18)

Arithmetic in `float` unless said; "compiled as" gives the operand order where a NaN's payload
depends on it (the first operand's NaN wins in SSE). `FZERO(x)` is `|x| < 2^-23` (for a `double`
too), `DZERO(x)` is `|x| < 1e-15`; a NaN is never zero by either.

- **Projection onto a triangle's plane** (`project_point_to_plane(point, V0, V1, V2, proj, pe1,
  pe2)`, all `double`): `e1 = V1 - V0`, `len = sqrt((x² + y²) + z²)`, `e1 = e1 / len` (a division
  per component); `DZERO(len)`: `point = V0 + V1`, `proj = (point + V2) / 3`, returns -1 (the
  basis outputs untouched). `e2 = V2 - V0`, made unit the same way; `DZERO`: the same with -2.
  `d = e1 . e2` (`(x x + y y) + z z`), `tmp = (e1.x d, e1.y d, d e1.z)`, `e2 = e2 - tmp`, its
  length; `DZERO`: -1 as above; else `e2 = e2 / len`. The basis is copied into `pe1`, `pe2` when
  given. `point = point - V0` (in place); `d1 = e1 . point` (as written), `d2` compiled as `(p.y
  e2.y + p.x e2.x) + p.z e2.z`; `proj = (e2 d2 + e1 d1) + V0` per component (`e1 d1` as `e1.k d1`,
  `e2 d2` as `e2.k d2`). Returns 0.
- **Barycentric coordinates** (`face_barycentric_coords(V0, V1, V2, cx, cy, cz, &l1, &l2, &l3)`,
  `double`): the projection of `(cx, cy, cz)`; a negative answer: a third each (`1.0 / 3`) and -3.
  With `proj` and the basis: `x = proj . e1`, `y = proj . e2` (as written); the corners' coordinates
  compiled as `x1 = e1.z V0.z + (e1.y V0.y + e1.x V0.x)`, `x2 = (e1.x V1.x + e1.y V1.y) + e1.z V1.z`,
  `x3 = (e1.y V2.y + e1.x V2.x) + e1.z V2.z`, `y1 = (e2.y V0.y + e2.x V0.x) + e2.z V0.z`, `y2 = V1.z
  e2.z + (e2.x V1.x + V1.y e2.y)`, `y3 = (V2.x e2.x + V2.y e2.y) + V2.z e2.z`; `det = (x1 - x3)(y2 -
  y3) - (y1 - y3)(x2 - x3)`; `DZERO(det)`: -2 with the coordinates of **FS-366** (the original gives
  0, 0, 0; the default a third each, noted). `l1 = ((x3 - x2)(y - y3) + (y2 - y3)(x - x3)) / det`,
  `l2 = ((y3 - y1)(x - x3) + (x1 - x3)(y - y3)) / det`, `l3 = (1 - l1) - l2`. Returns 1 when all
  three are strictly between 0 and 1; else, with `d = min(l, 1 - l)` (`l < 1 - l ? l : 1 - l`)
  narrowed to `float` for each, -1 when one `d` is below 0 and not `FZERO`, else 0. The coordinates
  are always written. The surface form `face_barycentric_coords(mris, fno, which, cx, cy, cz, ...)`
  takes the face's three corners in the coordinate set `which` (`MRISvertexCoord2XYZ_float`, its
  refusal of an unsupported set ending the program) widened to `double`.
- **Samples on a face.** `MRISsampleFaceCoords(mris, fno, x, y, z, which_coords, which_bary, &px,
  &py, &pz)`: the barycentric coordinates in the set `which_bary` (a third each when negative), the
  outputs set to 0, then per corner `p = (double)(coord) * l + (double)p` narrowed to `float`, for
  x, y and z of the set `which_coords`; returns the barycentric answer. `MRISsampleFace(mris, fno,
  which, x, y, z, val0, val1, val2)` (`double`): the barycentric coordinates in `which` whatever
  their answer (FS-366's applies), then `val = 0`, per corner `val + val_k l_k` (compiled as `val_k
  l_k`); each corner's coordinates of `which` are read once more (so an unsupported set ends the
  program). `MRISsampleFaceNormal(mris, fno, x, y, z, &px, &py, &pz)`: per corner the canonical
  distance `d`: each difference `(double)c - x` narrowed, `sqrt(dz² + (dx² + dy²))`; `dtotal` the
  sum (as `d + dtotal`) of those not below 0; outputs set to 0; per corner `w = 1 - (d + d) /
  dtotal` (`d` recomputed), skipped when below 0, else `px = nx w + px`, `py = ny w + py`, `pz = w nz
  + pz`; returns 0.
- **`MRISrmsTPHeight(mris)`** (`double`): 0 for status `MRIS_PLANE`. Otherwise the tangent planes
  are recomputed (§94), then over every unripped vertex with `vtotal > 0` and each of its first
  `vtotal` unripped neighbours: `x, y, z` the neighbour's current position minus the vertex's (in
  `float`, widened); `d = (x² + y²) + z²`, skipped when `FZERO(d)`; `dot = (x nx + y ny) + z nz`
  (the vertex's normal widened); the sum as `dot² / d + sum`; returns `sqrt(sum / count)` (a NaN,
  0/0, with no term).
- **Analytic distances.** For `which` `MRIS_SPHERE` (4) or `MRIS_PARAMETERIZED_SPHERE` (5): a
  circumference `c`, computed at the first unripped vertex where it is still `FZERO` (and kept from
  then on) as `(float)(sqrt(oz² + (ox² + oy²)) * 2 pi)` of that vertex's original position; the
  distance to a neighbour is `(float)((double)(angle c) / 2 pi)` with `angle` the `float` magnitude
  of fs.core's `Vector3Angle` of the two original positions (as 3-vectors) and the product in
  `float` as `angle c`. Any other `which`: the straight original-position distance `sqrt(zd² + (xd²
  + yd²))` (`d = v - vn`). **`MRIScomputeAnalyticDistanceError(mris, which, fp)`**: over the
  unripped vertices' first `vtotal` unripped neighbours (index `n`), `odist` as above; the sums (in
  `double`, each as `sum + term`) of `dist_orig[n] - odist` (its magnitude and itself; the
  difference in `float`), the same for `dist[n]`, and `odist` (its magnitude for a sphere); each
  mean over the count; `pct = mean_error / mean`, `pct_orig = mean_orig_error / mean`; prints `mean
  orig = %2.3f mm (%%%2.2f), final = %2.3f mm (%%%2.2f)` (mean orig error, `pct_orig 100`, mean
  error, `pct 100`) and `signed mean orig error = %2.3f, final mean error = %2.3f` on standard output;
  with `fp`, `FS_DISTURB_DISTANCES` read with `atof` (0 without it), the percent distance error (an
  earlier part of this spec; its assertions apply) and the line `%2.3f  %2.3f  %2.3f  %2.3f  %2.3f
  %2.3f  %2.3f` (two blanks between): `(valid 100) / nvertices` in `float`, the disturbance, `100
  pct_orig`, the mean orig error, `100 pct`, the mean error, the percent distance error. Returns
  `pct` (NaNs with no neighbour). **`MRISstoreAnalyticDistances(mris, which)`**: each unripped
  vertex's `dist_orig[n]` set to `odist` for its unripped neighbours; returns 0.
  **`MRISdisturbOriginalDistances(mris, max_pct)`**: each unripped vertex's first `vtotal`
  original distances `(float)((double)d * (r + 1))`, `r = randomNumber(-max_pct / 100, max_pct /
  100)` (fs.core's, one draw per distance, its seeding message on first use); returns 0.
- **`MRIScomputeHausdorffDistance(mris, mode)`** (`mode` unused; `double`): over each unripped
  vertex's first `vnum` unripped neighbours: when `vn.val v.val < 0` (in `float`), the zero's
  fraction from `v`: for `vn.val < 0` `1 - f(vn.val, v.val)`, else `f(v.val, vn.val)`, where `f(a,
  b) = (double)(-a / (b - a))` in `float`; the term `dist |v.val2| + (1 - dist) |vn.val2|` (the
  magnitudes in `float`, widened); when `v.val2 vn.val2 < 0`, the same with `val2` for the fraction
  and the term `|v.val| dist + |vn.val| (1 - dist)`; the mean of the terms (each added as `sum +
  term`), 0 without any.
- **Curvature ratio** (`MRISuseCurvatureRatio`): per unripped vertex `k1` the principal curvature
  of larger magnitude (`|k1| > |k2|`, else swapped); where `k2` is not `FZERO`, `curv = |k1 / k2|`
  and the extremes from 10000 (min) and -10000 (max) updated (`curv < min`, `curv > max`); then
  where `k2` is `FZERO`: `curv = 0` when `k1` is too, else the smallest ratio for `k1 < 0` and the
  largest otherwise. **Contrast** (`MRISuseCurvatureContrast`): `m` the smallest `k1` or `k2` of the
  unripped vertices (from 10000); per unripped vertex `k1 >= k2` (`k1 > k2`, else swapped), both less
  `m`, `curv = (k1 - k2) / (k1 + k2)`, extremes as above. Both set `min_curv`, `max_curv`.
- **Brute-force distance to a triangle** (`MinDistToTriangleBF(p1, p2, p3, ptest, pmin, dL)`,
  `double`): `dmin = 1e11`; `l1` from 0 while below 1 by `dL` (`l1 + dL`), within it `l2` the same;
  `l3 = 1 - (l2 + l1)`; skipped outside [0, 1]; `r = (p1 l1 + p2 l2) + p3 l3` for x and y, `l3 p3.z +
  (l1 p1.z + l2 p2.z)` for z; `d = sqrt(((pr.x² + 0) + pr.y²) + pr.z²)`, `pr = ptest - r`; a smaller
  `d` is kept with `pmin = r`; returns `dmin` (`pmin` untouched when nothing is below 1e11).
  **FS-367**: a step below 2^-52 (zero, negative) can loop for ever in the original; both modes
  refuse it (`MinDistToTriangleBF: step <dL> is too small` on standard error). `StuffVertexCoords`
  and `StuffFaceCoords` give a vertex's (a face corner's) current position widened.

## 97. Metric properties, part b, third group: normals, closest vertices, thickness, curvature bookkeeping

- **Face normals of a set** (`mrisOrigNormalFace`, `mrisWhiteNormalFace`, `mrisPialNormalFace`;
  `(mris, fno, n, norm)`): with corner `n` of the face, `n0` the corner before it (2 for 0) and
  `n1` the one after (0 for 2), in the set's positions: `v0 = P(n) - P(n0)`, `v1 = P(n1) - P(n)`,
  each made unit (`mrisNormalize`: `d = sqrt((x² + y²) + z²)`, divided when `d > 0`); `norm =
  (v0.y v1.z - v1.y v0.z, v0.z v1.x - v1.z v0.x, v0.x v1.y - v1.x v0.y)`, not normalised.
  **Vertex normals of a set** (`mrisComputeOrigNormal`, `...White...`, `...Pial...`; `(mris, vno,
  norm)`): `norm` set to 0, the face normals of the vertex's unripped faces (in its face order,
  with its corner index) added (`norm + face`); none: -4 (`ERROR_BADPARM`); else normalised with
  `d = sqrt(z² + (x² + y²))` (divided when `d > 0`) and 0.
- **`MRIScomputeNormal(mris, which, vno, &nx, &ny, &nz)`**: the same sum over unripped faces with
  the current set's face normal (`mrisNormalFace`, an earlier part of this spec) for `which` 4, and
  the set's above for 0 (original), 9 (white) and 7 (pial); any other `which` ends the program at
  the first unripped face: `MRIScomputeNormal: which = <which> not supported` (`ERROR_BADPARM`).
  None: -4, the outputs untouched; else normalised with `(x² + y²) + z²` and widened, 0.
- **`MRIScomputeSurfaceNormals(mris, which, navgs)`**: `MRISsaveVertexPositions(TMP_VERTICES)` and
  `MRISrestoreVertexPositions(which)` (an earlier part of this spec, with their refusals); each
  unripped vertex's normal of the set (`which` 9: `wn`, 7: `pn`, 0: `on`, any other: the current
  normal `n`) set to `MRIScomputeNormal`'s answer narrowed. **FS-369**: a vertex without an unripped
  face gets, in the original, the previous unripped vertex's answer (0 for the first); by default it
  keeps its normal (noted). Then `navgs` times: for each unripped vertex `t` = its normal, plus each
  unripped immediate neighbour's (`vnum`), each sum `(float)((double)t + (double)n)`; `len` the
  `float` `sqrt((x² + y²) + z²)` widened; unless `FZERO(len)` (a NaN divides), `t = (float)((double)t
  / len)`; kept in `tdx..tdz`; after all vertices each unripped vertex's normal set to it. At the end
  `MRISrestoreVertexPositions(which)` again: **FS-368** (rule D, kept by default): the surface is left
  at the set's positions, not the saved current ones; the correction (restoring the temporary set)
  is on request (a switch, noted). Returns 0.
- **Closest-vertex searches** (`MRISfindClosestOrigVertices(mris, nbhd)`,
  `MRISfindClosestPialVerticesCanonicalCoords(mris, nbhd)`, and both passes of the thickness): for a
  vertex `v`, the starting distance `sqrt(z² + (x² + y²))` of its own offset (named below), `v`
  marked 1 and listed; for `ns` from 1 to `nbhd`: for each listed vertex (in list order; the list
  grows during the ring and the ring walks the list as it was at its start) that is unripped and
  not marked below `ns - 1` (marked 0 counts as unmarked), each immediate neighbour (`vnum`) that is
  unripped and unmarked is listed and marked `ns`; then, with `d` its offset (below), skipped when
  `(d.x n.x + d.y n.y) + d.z n.z < 0` (`n` the vertex's normal) or when the neighbour's normal dotted
  with `n` is below 0; else `dist = sqrt((x² + y²) + z²)` of `d`; when `dist < min`, `min`, its ring
  and vertex are kept (with verbose diagnostics and the ring equal to `nbhd`: `<vno> --> <nb> =
  %2.3f` on standard output). At the end every listed unripped vertex's mark is set to 0. The
  orig search: offset from `v`'s original position to the neighbour's current one, current normals;
  the vertex's curvature set to the nearest vertex's number (as a `float`). The pial search: from
  `v`'s white position to the neighbour's pial one, white normals; curvature as above and `v`'s
  current position set to the nearest vertex's canonical one. Both then print `<count> vertices at
  <n> distance` for `n` from 0 to `nbhd` (the counts of the nearest rings). **FS-370**: the
  original keeps 100 counts and 100,000 listed vertices: a size of 100 or more, or more than 100,000
  listed, reads and writes outside them; both modes count and list all (noted).
- **Cortical thickness** (`MRISmeasureCorticalThickness(mris, nbhd, max_thick)`): pass 1 over all
  vertices (`<vno> of <nvertices> vertices processed` at every multiple of 25,000, 0 included):
  ripped: curvature 0; else the orig search with the vertex's own offset `x - origx` (and its
  Gdiag_no line `vno=%d A %3d %6d (%g,%g,%g) (%g,%g,%g) %g %g`: original position, current
  position, `dist`, the smallest so far); the distance above `max_thick` becomes it (counted as a
  white-to-gray truncation); stored in the curvature. Pass 2 (the same progress lines): unripped
  vertices only; the search from the neighbours' original positions to the vertex's current one
  (offset `x - vn.origx`, current normals; Gdiag_no line with `B` and the neighbour's original
  position); with verbose diagnostics and `|curv - dist| > 4`, `v %d, white->gray=%2.2f,
  gray->white=%2.2f`; truncated likewise (gray-to-white); curvature `(dist + curv) * 0.5`; Gdiag_no:
  `vno = %d, final measurment %g`. Then `thickness calculation complete, <w>:<g> truncations.` and
  the ring counts (both passes counted). Returns 0.
- **Curvature bookkeeping.** `mrisTrackTotalDistance(mris)` / `...New(mris)`: each unripped vertex's
  curvature `nc + curv` with `nc = (d.x nx + d.y ny) + d.z nz` of its step `d` (`dx..dz`; the new form
  `odx..odz`). `MRIStotalVariationDifference(mris)` (`double`): the sum of `(delta delta) area`,
  `delta = |k1 - k2|` in `float`, widened; after each term a sum that is not finite prints the
  error `curvature at vertex <vno> is not finite!` (fs.core's `ErrorPrintf`). `MRIStotalVariation`:
  the same with `delta = k1² + k2²` in `float`, the term `delta area`. `MRIScurvatureError(mris,
  Kd)`: the sum of `((k - Kd)²) area` for `k1` then `k2` of each unripped vertex, `double`.
  `MRISuseCurvatureDifference`: curvature `|k1 - k2|`; `...Max`: `k1` when `|k1| > |k2|`, else `k2`,
  and `kmin = %2.2f, kmax = %2.2f` on standard output; `...Min`: `k2` when `|k1| > |k2|`, else
  `k1`; `...Stretch`: the largest of 0 and `dist[n] s - dist_orig[n]` over the first `vtotal`
  neighbours, `s = sqrt(orig_area / total_area)` in `float` (glibc's `sqrtf`: `errno` `EDOM` for a
  negative ratio); all four keep the unripped extremes from 100000 (min) and -100000 (max) in
  `min_curv`, `max_curv`. `MRISuseNegCurvature`: 1 for an unripped vertex with a face of negative
  area, else 0; extremes 0 and 1.
- **Point in a spherical triangle** (`containsAnotherVertexOnSphere(mris, vno0, vno1, vno2, mode)`,
  `double` from the canonical positions): for each rotation of the triangle (`(0,1,2)`, `(1,2,0)`,
  `(2,0,1)`) as `(V0, V1, V2)`: `o = (float)(V1.c + V2.c) / 2`, `t = V2.c - V1.c` (in `float`),
  `n = o x t` (`F_CROSS`), `sign = (V0.c - o) . n`. The candidates: the immediate neighbours of
  `vno0` (other than `vno1`, `vno2`), of `vno1` (other than `vno0`, `vno2`) and of `vno2` (other than
  `vno0`, `vno1`), unmarked ones only, with `mode` only those whose `fixedval` is set; each marked
  as listed, all unmarked after. Returns 1 for the first candidate with `sign ((p - o) . n) >= 0`
  for all three edges, else 0. **FS-371**: the original keeps 1,000 candidates; more write past
  them; both modes list all (noted).

## 98. Validation plan, part 16

The probe's `mpc` command (`validation/probes/fs-surfio/`): an empty work folder with `d`; Gdiag's
bits; `FREESURFER_SEED` always (1 to 10^6) and `FS_DISTURB_DISTANCES` one in three (`3.5`, `-2`,
`1e1`, `abc`, `0x1p3`, `nan`, `12.75x`, `inf`); a binary triangle file of `srd`'s meshes read by
`MRISread`; the status set one in three (2, 4, 0, 5, 1); the original area one in two; per vertex
(after `MRISmakeDistOrig`) a mask: ripped one in 8; original, canonical, pial, white, current
positions, values, principal curvatures, normals, white normals, steps, old steps, area each one
in 2 to 5; mark (1 to 3) one in 12; `fixedval` one in 2; up to 8 original and current distances
(those past `vtotal` dropped); per face ripped one in 8 and its area one in 4; then 1 to 6
operations of this part (barycentric coordinates on a face, of any triangle with now and then two
equal or nearly equal corners, the projection; the samples; the RMS height; the analytic distances
for statuses 0, 2, 4, 5 with and without `fp`; storing and disturbing them; the zero-crossing
distance; ratio and contrast; the brute-force distance with steps 0.05 to 1, and one in ten a NaN,
infinity, 0, -0.25, 1e-20 or 2; the stuffed coordinates; the vertex normals of every set and an
unsupported one; the surface normals of sets 4, 9, 7, 0, 3 with 0 to 3 averages; the three searches
with sizes 0 to 4 (one in 40: 100, 101, -1) and `max_thick` up to 10; the curvature bookkeeping;
the spherical-triangle test on a face's corners or three random vertices, `mode` one in three);
each prints its result and the fields it changes (curvatures and extremes, normals of every set
with the positions, original distances, marks, `errno` after the curvature choices). A case in
which the original reads memory outside its arrays (FS-370, FS-371) or would loop (FS-367, ended by
the probe's alarm) is counted, not compared. Observed (relay 18): the probe README.

## 99. The discrete curvatures and the angle-weighted vertex normal (queue item 3.1-50; relay 19)

These are `mris_curvature`'s discrete curvatures (its `-discrete` option) and the topology fixer's
vertex normal. Every quantity is a `float` unless said; a `float` sum is accumulated in order, one
term at a time; "a NaN kept" means a NaN argument passes through the limiting unchanged. A
**cached face normal** is the one the triangle properties (§21) store for the face. The **arc
cosine of a `float`** (`acosf`) is glibc's with `reproduce` and the correctly rounded one by
default (§99.9); a NaN argument gives itself (quieted) in both.

**99.1 The face's place of a vertex** (`FACE_vertexIndex_find(face, vno)`): the **last** of the
face's three places (0 to 2) holding `vno`; -1 when none does. **The vertex off a shared edge**
(`FACE_vertexIndexAtMask_find(face, common)`), with `common` the three numbers §94's common-vertex
search gives for two faces (their common vertices in the first face's order, as `float`s, -1 for
the rest): the first of the face's vertex numbers that is not among them (compared as `float`s);
-1 when all are.

**99.2 The marks** (`MRISsetMarks(mris, m)`, `MRISclearMarks(mris)` = mark 0): every unripped
vertex's mark becomes `m` (stored as a 16-bit integer); ripped vertices keep theirs. Return 0.
`MRIScountAllMarked(mris)`: the number of vertices, ripped ones included, whose mark is above 0.
`MRISdilateMarked(mris, n)`: `n` times: every unripped vertex's `tx` becomes 0; then each unripped
vertex's `tx` becomes, as a `float`, the largest of its own mark and its 1-hop neighbours' marks
(ripped neighbours included); then each unripped vertex's mark becomes its `tx` (truncated). Return
0. (`tx` is left holding that value.)

**99.3 The faces in order around a vertex** (`MRIS_facesAtVertices_reorder(mris)`): when any
vertex, ripped ones included, has a mark above 0, the program ends with the failed assertion
`MRIScountAllMarked(apmris) == 0` (§3's assertion, its source line 3178 of
`utils/mrisurf_topology.cpp`). Otherwise a newline on standard error, then for every vertex
(ripped ones too) the progress bar (§47) with the message `Determining geometric order for vno
faces...` and the vertex's **walk**:
- A vertex without faces ends the program: on standard error two newlines, `FATAL ERROR
  encountered in function ''FACES_aroundVertex_reorder'.`, a newline, `Mesh structural error.
  Vertex <vno> has no faces.` and two newlines; exit status 1.
- The walk places the vertex's faces (its face list, places `0 .. num-1`) in a sequence: place 0
  first. Step `i` (from 0) takes the face last placed (the sequence's entry `i`) and looks at the
  places `(i + j) mod num` for `j = 0, 1, ...`: the first face that shares exactly two vertices
  with it (§94's common-vertex count; a face never shares exactly two with itself) and is not yet
  in the sequence is placed next; the walk ends when all are placed. A step that places nothing
  leaves the next step without a face: see FS-372.
- The program counts its walks; when `BEVIN_TESTING_dumpFacesAroundVertex` is set at the moment
  the count (before this walk) equals a limit (first 1; doubled each time it is used), the faces are
  dumped (below) — i.e. at the 2nd, 3rd, 5th, 9th, ... walk of the program while it is set.
- When not every face was placed: the dump, then the error message `FACES_aroundVertex_reorder:
  packed / faces mismatch; vertex = <vno>, faces = <num>, packed = <placed>` (§2's error report,
  code -4); the vertex is marked (1) and its lists are unchanged. Otherwise its face list becomes
  the faces in the walk's order and its face-place list (`n`) each face's place of the vertex
  (§99.1's search from the first place; 255 when absent).
- **The dump**, on standard output: `Dumping faces around vno:<vno>`, then per face of the vertex
  (list order) `  vi:<vno> is face:<fno> with vertices` (the vertex's number, as FreeSurfer
  prints it, not the face's place) and ` <v>` for each of its three vertices, then a newline.
Then the marks are dilated once (§99.2). Returns the last vertex's walk's result: 1, or -4 when it
failed.

**FS-372** (class a: out of bounds). In FreeSurfer, after a step that places nothing (a vertex on
the surface's border whose walk did not start at an end of its faces, two fans of faces meeting at
the vertex, a face sharing only one vertex with the rest), the next step starts from a sequence
entry that was never written and reads before the vertex's face list; what follows depends on
memory contents (garbage faces, writes past the sequence, a crash). larmorx stops the walk at that
step in both modes, so the vertex fails as above (dump, message, mark), and notes FS-372. Closed
surfaces (every vertex's faces one closed fan) are never affected.

**99.4 The curvature sign of two faces** (`FACES_Hcurvature_determineSign(mris, fo, fi)`): the
faces must share exactly two vertices, else the program ends: `FACES_Hcurvature_determineSign:
During call <c>, the passed faces do not share an edge` (code -4; `c` the number of this function's
earlier calls in the program that completed). With `O` and `I` the current positions of each face's
vertex off the shared edge (§99.1), `nO`, `nI` the faces' cached normals: `dN = |(O + nO) - (I +
nI)|` and `dA = |(O - nO) - (I - nI)|`, each component `(P + n) - (Q + m)` in `float` and the length
`sqrt(z² + (x² + y²))` in `float`; the result is +1 when `dN < dA`, else -1 (a NaN gives -1). The
completed-call count then increases. **FS-373** (class a): when every vertex of a face is on the
shared edge (a face that names one vertex twice), FreeSurfer uses the position of the vertex before
the surface's first (memory outside the vertices); larmorx ends the program in both modes with
`FACES_Hcurvature_determineSign: a face has no vertex off the shared edge (a vertex named twice)`
(code -4) and notes FS-373.

**99.5 The face angles at a vertex** (`VERTEX_faceAngles_determine(mris, vno, angles)`): with `a` =
the vertex's normal (`nx`, `ny`, `nz`) and `|a| = sqrt(az² + (ax² + ay²))`, for each face of the
vertex (list order), `f` its cached normal, `|f| = sqrt((fx² + fy²) + fz²)`: `errno` set to 0;
`r = ((fy ay + fx ax) + fz az) / (|f| |a|)`; `r` above 1 taken as 1, below -1 as -1 (a NaN kept);
the angle `acosf(r)` stored at the face's place. (FreeSurfer then checks `errno` and would print
`VERTEX_faceAngles_determine: acos error - angle set to zero for vertex = <vno>, face = <k>.` and
use 0; `acosf` of a number in [-1, 1] or a NaN never sets it.) Returns the face count. **Its
extremes** (`VERTEX_faceMinMaxAngles_determine(mris, vno, &imin, &min, &imax, &max)`): a vertex
without faces ends the program (`VERTEX_faceMinMaxAngles_determine: error with determining face
angles.`, code -4); otherwise `min` and `max` start at the first angle, and for each later place `k`
an angle below `min` (above `max`) replaces it and sets `imin = k` (`imax = k`); the indices are
written only then (the caller's values stay otherwise). Returns the face count.

**99.6 The angle between two faces** (`FACES_angleNormal_find(mris, fi, fj)`): `i`, `j` the faces'
cached normals, `|i| = sqrt(iz² + (ix² + iy²))`, `|j| = sqrt((jx² + jy²) + jz²)`; when either length,
as a `double`, is above 1.0001, the program ends: `FACES_angleNormal_find: face normal not unit
length -- Ni: <|i|>, Nj: <|j|>` (each `%f`) and a newline (code -4). Then the sign `s` (§99.4, which
may end the program), `r = ((ix jx + iy jy) + iz jz) / (|j| |i|)` limited as in §99.5, and the result
`acosf(r) · s` (`s` as a `float`).

**99.7 The discrete K and H** (`MRIS_discreteKH_compute(mris)`): for every vertex the progress bar
(`Determining KH curvatures...`). A marked vertex (mark not 0) gets `K = H = 0` and is skipped.
Otherwise, over its faces in list order with `F_k` the face at place `k` and `G_k` the next
(`k + 1`, the last with the first): the angle `θ_k` = §99.6 of (`F_k`, `G_k`); the length `e_k` of
their shared edge (§94's `FACES_commonEdgeLength_find`, which ends the program when they do not
share exactly two vertices); `A = A + area(F_k)`; `F_k`'s place of the vertex (§99.1; -1 ends the
program: `MRIS_discreteKH_compute:`, a newline, a tab and `angleIndex lookup failure for vertex
<vno>, face <k>`, code -4); `D = D + angle(F_k)[place]`; `S = θ_k e_k + S`. Then, in `double`, `K =
q · (2π - D)` with `q = 3 / A` computed in `float` and `H = S · (0.75 / A)`; the vertex keeps both
as `float`s. The first vertex (number 0), when unmarked, sets `Kmin = Kmax` to its `K` and `Hmin =
Hmax` to its `H` (else they start at 0); every unmarked vertex then raises `Kmax` (`Hmax`) when its
`double` value is above it, lowers `Kmin` (`Hmin`) when below (the stored `float`s); `Ktotal = (K ·
area) + Ktotal` in `double`, kept as a `float` (`area` the vertex's). The surface's `Kmin`, `Kmax`,
`Hmin`, `Hmax` and `Ktotal` are set at the end. Returns 0.

**99.8 The principal curvatures** (`MRIS_discretek1k2_compute(mris, signed)`): for every vertex
(ripped ones too) the progress bar (`Determining k1k2 curvatures...`); `d = H H - K` (a negative `d`
counted and taken as 0; a NaN kept), `s = sqrt(d)`, `A = H + s`, `B = H - s`; with `signed`, `k1 =
A ≥ B ? A : B` and `k2 = A ≤ B ? A : B`; without, the same comparisons of `|A|` and `|B|`; a NaN
makes both `B`. With a count above 0 at the end: `cprintd("deltaViolations", count)` (§47). Returns
0. **The second fundamental form, discrete** (`MRIScomputeSecondFundamentalFormDiscrete(mris,
signed)`): the marks cleared (§99.2), the triangle properties (§21), the faces in order (§99.3, its
result not used), §99.7 and §99.8; returns 0. **The obsolete H sign**
(`MRIS_Hcurvature_determineSign(mris)`, not in FreeSurfer's headers): per vertex the progress bar
(`Determining H sign for vertex faces...`) and the sum of §99.4 over its consecutive faces (the last
with the first; none for a vertex without faces); `undefval` 1 when the sum is 0 or more, else -1.
Returns 1.

**99.9 The arc cosine of a `float` by default** (`acosf` with correctly rounded maths): the correctly
rounded `double` arc cosine of the argument rounded to `float` (ties to even), except for the two
floats in [-1, 1] where that `double` lies exactly halfway between two floats: `0x328885a3` gives
`0x3fc90fdb` and `0x39826222` gives `0x3fc907b5` (the true values lie above the halfway points;
established on all 2,130,706,434 floats in [-1, 1]: `validation/investigations/acosf-double-rounding/`).
glibc's (`reproduce`) differs from it on 0.25 % of floats (`docs/findings/freesurfer-libm.md`).

**99.10 The angle-weighted vertex normal** (`computeVertexPseudoNormal(mris, vno, norm, verbose)`):
`norm` starts at 0, 0, 0; for each face of the vertex (list order), with `n0` the face-place list's
entry (`n`), `n1` the next place (`n0 = 2` gives 0) and `n2` the previous (`n0 = 0` gives 2), `a_k`
the face's vertex at place `k`:
- when `a_n0` is not `vno`, or `a_n1` or `a_n2` is, or `a_n1 = a_n2`, and `verbose` is 3 or more, on
  standard error (no newlines between): `error for vno in face <f>` when `a_n0 ≠ vno`, `error for vn1
  in face <f>` when `a_n1 = vno`, `error for vn2 in face <f>` when `a_n2 = vno`, `error for vn in face
  <f>` when `a_n2 = a_n1`, then `face <f> (<a_n0>,<a_n1>,<a_n2>) != (<vno>)` and a newline; with
  `verbose` 3 also `computeVertexPseudoNormal: SHOULD NOT HAPPEN` and a newline; with 4 the program
  ends with that message (`ERROR_BADPARM`). The face is used all the same.
- `v1`, `v2`: the original positions of `a_n1` and `a_n2` less the vertex's (`float`); `r = (((v1x v2x
  + v1y v2y) + v1z v2z) / |v1|) / |v2|` with `|v| = sqrt(vz² + (vx² + vy²))`; `α = r` above 1 → 1, `r`
  of 0 or below → 0 (`+0`), a NaN kept; `α = acosf(α)`; `norm_c = n_c α + norm_c` for each component,
  `n` the face's cached normal.
The result is not normalized.

## 100. Validation plan, part 17

The probe's `mpd` command (relay 19): an empty work folder with `d`; Gdiag's show and verbose bits;
`BEVIN_TESTING_dumpFacesAroundVertex` one case in six; a binary triangle file read by `MRISread`:
the icosahedron subdivided once (42 vertices, 80 faces; 2 cases in 3) or twice (162, 320; 1 in 12)
on a sphere of radius 1, 10, 50 or 100 (each coordinate moved by k/64 one time in 12), an open grid
of 3 to 6 by 3 to 6 vertices with heights k/16 (1 in 12), or one of `srd`'s small meshes (1 in 12);
per vertex a mask (ripped 1 in 40, a mark of -1 to 2 1 in 50, current position 1 in 25, normal 1 in
12, original position 1 in 10, `K` and `H` 1 in 8, area 1 in 12; in half the cases each four times
rarer), values as `mpc`'s (one case in five with NaNs and infinities one value in four); per face
(ripped 1 in 40, area, angles and cached normal 1 in 12 to 15; the normal's components k/16 mostly);
then 1 to 3 operations: the discrete second fundamental form (signed or not), the faces' order, KH,
k1k2 (signed or not), the sign and the angle of two faces (sharing an edge 4 in 5, else any two),
a vertex's face angles and their extremes, the obsolete H sign, the pseudo-normal (verbose 0, or 0 to
4 one in three), the face-place helpers, the dilation (0 to 2), clearing the marks. Each prints its
result and what it changes (`K`, `H`, `k1`, `k2`, marks, the extremes and `Ktotal`; the face and
place lists; `undefval`; `tx`). A case noting FS-372 or FS-373, or FS-76 (an MGH header without
voxels read for a transform's volume, §101), is counted, not compared. With the default, a case may
differ from FreeSurfer's only by the arc cosine (§99.9): the probe's `check.py` counts those apart
(`fix_libm`). Observed (relay 19): the probe README.

## 101. The transform's volumes, validated (relay 19)

§92's source geometry from `mri/orig` and the transform reader's source and destination volumes
(its spec) are read with the volume library's `MRIreadHeader` (any format it recognizes; fs.io's).
The probe's `srd` command (part 14) now writes, for a case with a transform, each of the volumes
`orig.mgz` (the name the v8 stream's `%Generated by ... src <a> dst <b>` line gives), `t1.mnc.mgz`
and `v.mnc.mgh` (minctracc's and the other comment lines need `.mnc` in a name) one time in four
missing, as an MGH file (version 1, or 2 one in 15; each dimension 1 to 4, or -1 or 0 one in 20; one
frame; `UCHAR`, `FLOAT`, `INT`, `SHORT` or 77; the RAS flag 1, 0 or -1; voxel sizes k/4, axes a
signed permutation or any, centre k/8, each value one in 25 NaN, infinite, 0, -0 or 1e30; the voxels;
one file in 12 cut), gzip-compressed one time in two for `.mgz`; and one case in three with a
transform a regular file `mri/orig` (an MGH file, or random bytes one in five). Half the comment
lines name those volumes. A message naming the work folder's absolute path (the volume reader's
direction-cosine message) is compared with the path written `<w>` on both sides. A regular
`mri/orig` without an extension is never recognized as an MGH volume (the format is found by the
name, and no content test of the volume library recognizes MGH), so its read gives `ERROR: could
not determine type of <name>` and the centre stays 0; FreeSurfer's subjects have `mri/orig` as a
folder, which is not read. Observed (relay 19): seeds 7, 13 and 17 (2,000 cases each): reproduce
identical on every compared case; 66 compared outputs carry a geometry read from a volume header.

## 102. Curvature choices and statistics (queue item 3.1-51; relay 20)

Every quantity is a `float` unless said; sums accumulate in order, one term at a time; "the first
`vnum` neighbours" is the vertex's 1-hop neighbour list. NaN operands follow §14's rule (the left
operand of each written expression gives its NaN); where the order of a sum's operands differs
from the formula's as written, the formula below gives the order.

**102.1 Smoothed normals** (`MRISsmoothSurfaceNormals(mris, navgs)`): `navgs` passes; in each, every
unripped vertex's `tdx, tdy, tdz` become `(n + n1 + ... + nk) / (k + 1)` (its own normal plus its
unripped first-`vnum` neighbours' normals, `k` their count, the count kept as a `float` incremented by
1 each time), then every unripped vertex's normal becomes `td / |td|` with `|td| = sqrtf((x x + y y) +
z z)`; when `|td|` is below `FLT_EPSILON` the normal becomes `td` unchanged. Return 0.
`mrisComputeBoundaryNormals(mris)`: nothing; 0.

**102.2 The mean curvature** (`MRIScomputeMeanCurvature(mris)`): the tangent planes are recomputed
(§94's `mrisComputeTangentPlanes`, ripped vertices too); for each unripped vertex with normal `n`
and tangents `e1`, `e2`, over its unripped first-`vnum` neighbours, with `d` the neighbour's position
minus the vertex's: `u = (e1x dx + e1y dy) + e1z dz`, `w` likewise with `e2`, `rsq = u u + w w`; a
neighbour with `rsq` below `FLT_EPSILON` is skipped; else `q = z / rsq` with `z = dz nz + (dy ny + dx
nx)`, `H = H + q` and the count increases; with `Gdiag` holding both `DIAG_SHOW` and `DIAG_VERBOSE` and
`|q| > 5`, `<vno> --> <neighbour>: curvature = %2.1f` (q) on standard output. The vertex's `H` is
`(H * 0.5) / count` (0 with no counted neighbour). The surface's `min_curv`, `max_curv` are the
smallest and largest `H` (starting at 10000 and -10000; a NaN never replaces). With `DIAG_SHOW`:
`mean curvature range [%2.3f --> %2.3f]` on standard output. Return 0.

**102.3 Curvature choices.** Each sets every unripped vertex's `curv`:
- `MRISuseGaussianCurvature`: `K`; extremes `Kmin`, `Kmax`. `MRISuseMeanCurvature`: `H`; extremes
  `Hmin`, `Hmax`. `MRISusePrincipalCurvature`: `k1`; extremes `Hmin`, `Hmax` (sic). Return 0.
- `MRISuseAreaErrors`: `(A - A0) / n` with `A` the sum of the areas of the vertex's faces (its face
  list, in order) and `A0` the sum of their cached original areas (§27; a deferred cache entry is
  computed first). **FS-374** (class c, rule A): FreeSurfer's `n` is 0, so each value is ±infinity
  or a NaN (0/0 the default NaN). `reproduce`: as FreeSurfer. Default: `n` the face count, 0 for a
  vertex without faces; noted when a value differs. Return 0.
- `MRISusePrincipalCurvatureFunction(mris, f)`: `curv = f(k1, k2)`; `MRISuseK1Curvature` and
  `MRISuseK2Curvature` are the same with `f` giving `k1` or `k2`. The extremes are kept with `curv <
  min` and `curv > max` over the unripped vertices in order. **FS-375** (class c, rule A):
  FreeSurfer starts both extremes at vertex 0's `curv` as it was before the call, so the range can
  hold a value no vertex has any more (and a NaN there stays). `reproduce`: as FreeSurfer. Default:
  both start at the first unripped vertex's new value (vertex 0's old one when every vertex is
  ripped); noted when the extremes differ. **FS-376** (class a, memory): on a surface without
  vertices FreeSurfer reads a vertex that does not exist; both modes set the extremes to 0, noted.
  Return 0.

**102.4 Curvature indices** (`MRIScomputeCurvatureIndices(mris, &ici, &fi)`, in `double`): over
unripped vertices, `ici = (K area) + ici` when `K > 0`, `fi = ((area |k1|) (|k1| - |k2|)) + fi`;
then each divided by `4 pi`. Without vertices both 0. Return 0.

**102.5 Largest radius** (`MRISmaxRadius(mris)`, `double`): the bounding box of every vertex's
current position (ripped too; low ends from 10000, high ends from -10000; a NaN never replaces) is
stored in the surface's `xlo, ylo, zlo, xhi, yhi, zhi` (as `float`); the centre `c = (lo + hi) *
0.5`; the result is the largest `sqrt((x² + y²) + z²)` over every vertex of its offset from `c` (0
without vertices; a NaN never replaces).

**102.6 Mean and spread of the curvature** (`MRIScomputeAverageCurvature(mris, psigma)`, `double`):
over unripped vertices `total_sq = total_sq + c²`, `total = total + c`, `n = n + 1`; `mean = total /
n`, `var = total_sq / n - mean²` (both 0 when `n` is 0); when `psigma` is given it receives
`sqrt(var)` (a negative `var`: the default NaN and `errno` `EDOM`). Returns `mean`.

**102.7 Vertices of negative faces.** `MRISmarkNegativeVertices(mris, mark)`: the three vertices of
every face whose area is below 0 get the mark (16 bits). `MRISripNegativeVertices(mris)`: such
faces and their three vertices are ripped. Return 0.

## 103. The second fundamental form (queue item 3.1-51; relay 20)

`MRIScomputeSecondFundamentalForm(mris)` is `MRIScomputeSecondFundamentalFormThresholded(mris,
-1)`. For a plane (`MRIS_PLANE`) nothing happens; return 0.

**103.1 The threshold.** With `pct_thresh` at or above 0: `vmean` is the mean neighbour spacing of
§94's `MRIScomputeTotalVertexSpacingStats` (it recomputes the metric properties first, §38), `m = vmean *
0.25`, `m = m < 1 ? m : 1` (a NaN gives 1), `thresh0 = m m`; otherwise `thresh0 = 0`. Then the tangent
planes are recomputed (§94). `Kmin = Hmin = 10000`, `Kmax = Hmax = -10000`, `Ktotal = 0` (`double`).
With `DIAG_WRITE` and `DIAG_VERBOSE` the file `curv.dat` is opened for writing.

**103.2 The fit at a vertex.** For each unripped vertex with at least one neighbour in use
(`vtotal`), with `n`, `e1`, `e2` its normal and tangents (values from the start of this vertex's
step): a `vtotal` x 3 matrix `U` and a `vtotal`-vector `z`, both zeros. Passes, each starting with
`kmin = 10000`, `kmax = -10000`, a row counter `r = 0` and `thresh` (`thresh0` at the first pass):
for each neighbour in use, in order, skipping ripped ones, with `y` its position minus the
vertex's (`float`): `u = (double)((e1x yx + e1y yy) + e1z yz)`, `v = (double)((yy e2y + yx e2x) + yz
e2z)`; row `r + 1` of `U` becomes `(float)(u u)`, `(float)((u + u) v)`, `(float)(v v)` and `z`'s
entry `r + 1` becomes `(nx yx + ny yy) + nz yz` — **written before the test below, so a rejected
neighbour's values sit in row `r + 1` until an accepted one overwrites them**; `rsq =
(float)(v v + u u)`; the neighbour is accepted when `rsq` is not below `FLT_EPSILON` and `(double)rsq
> thresh`: `k = z_entry / rsq` updates `kmax` (`k > kmax`) and `kmin` (`k < kmin`), and `r`
increases. After the pass `thresh = 0.25 thresh`; the passes end after the 102nd or when `r` is 4 or
more.

**103.3 The solve.** `Uᵀ` (§fs.maths `MatrixTranspose`), `A = Uᵀ U` (`MatrixMultiply`, `float`),
`cond = MatrixConditionNumber(A)`, `A⁻¹ = MatrixSVDInverse(A)`. When the inverse fails (singular):
the eigenvalues are 0, 0 and the eigenvector matrix the identity, and the vertex goes on to 103.4;
the ill-conditioned count increases. Otherwise `c = A⁻¹ (Uᵀ z)` (two `MatrixMultiply`), the Hessian
`Q = [[c1 + c1, c2 + c2], [c2 + c2, c3 + c3]]`. When `cond >= 500000` (a NaN is not): `k1 = kmax`,
`k2 = kmin`, `K = k1 k2`, `H = (k1 + k2) * 0.5`, and nothing else for this vertex. Otherwise
`MatrixEigenSystem(Q)` gives the eigenvalues and eigenvectors; when it fails, `MatrixSVDEigenValues(Q)`
gives `k1`, `k2` (its values 0 and 1), with `K` and `H` as above, nothing else for this vertex, and the
ill-conditioned count increases. **A matrix with a NaN or infinite entry:** see the fs.maths spec's
FS-48 (`MatrixConditionNumber` and `MatrixSVDInverse` refuse it; on some FreeSurfer hangs).

**103.4 The vertex's curvatures** (eigenvalues `k1`, `k2`, eigenvectors `a11 a12 / a21 a22`): `K = k1
k2`, `H = (k1 + k2) * 0.5`; with the file open, the line `<vno> %f %f %f %f` (`k1`, `k2`, `K`, `H`);
when the vertex is `Gdiag_no` and `DIAG_SHOW`: `v %d: k1=%2.3f, k2=%2.3f, K=%2.3f, H=%2.3f` on
standard output; the extremes (`K < Kmin`, `H < Hmin`, `K > Kmax`, `H > Hmax`); `Ktotal = ((double)k1
k2) area + Ktotal`; the area summed (`double`); the tangents: `e1x = e2x a21 + e1x a11`, `e1y = e1y
a11 + e2y a21`, `e1z = a21 e2z + a11 e1z`, `e2x = e1x a12 + e2x a22`, `e2y = e1y a12 + e2y a22`,
`e2z = a12 e1z + a22 e2z` (the old tangents' values).

**103.5 After the vertices.** The file closed. With `DIAG_SHOW` and `DIAG_VERBOSE`: `max H error=-1.000
at -1` and `total area = %2.3f`; with `DIAG_SHOW` and an ill-conditioned count above 0: `%d
ill-conditioned points` (standard output). With `pct_thresh` below 0: return 0.

**103.6 The outlier removal** (`pct_thresh` at or above 0), in `double`: over unripped vertices the
extremes of `k1` and of `k2` (from 1000 for the lows, -1000 for the highs; a NaN never replaces);
`scale = 999 / (max - min)` for each; two 1,000-bin histograms with `bin_size = (float)(1 / scale)`;
for each unripped vertex in order, `k1`'s bin `nint((k1 - min1) scale1)` counted (`1 + count`), then
`k2`'s. **FS-377** (class a, crash): a bin outside 0..999 (all values equal, so `scale` is infinite;
a value not finite) makes FreeSurfer write far outside the histogram (a segmentation fault).
`reproduce` ends the program as that crash (exit status 139); the default leaves that value out,
noted. Each bin's value `(float)(bin / scale + min)`. `nint((0 - min) scale)` is computed for each and
discarded (its notice for a NaN, fs.core's). Both histograms become PDFs (fs.core's `HISTOmakePDF` in
place); with `DIAG_WRITE` and `DIAG_VERBOSE` they are plotted to `k1.plt`, `k2.plt`. The peak of
each: `HISTOfindHighestPeakInRegion(h, 0, 1000)`. **FS-378** (class a, memory): without unripped
vertices there is no peak (-1) and FreeSurfer reads one place before the histogram; no vertex is
clipped, so both modes skip the read (noted) and the result is FreeSurfer's. The limits from a peak
`z`, for each histogram: `T` the sum of counts from `z` to the end, `t = T pct`; going up from `z`
(at most to bin 998), summing counts, the first bin where the sum exceeds `t` gives `max` = that bin's
value; likewise going down from `z` to bin 1 (stopping at 0) gives `min`. Then `Kmin = Hmin = 10000`,
`Kmax = Hmax = -10000`, and for each unripped vertex: `k1 > max1` → `max1`, `k2 > max2` → `max2`, `k1
< min1` → `min1`, `k2 < min2` → `min2` (each counted), `K` and `H` recomputed, the extremes updated as
in 103.4. When any was clipped: `%d vertices thresholded to be in k1 ~ [%2.2f %2.2f], k2 ~ [%2.2f
%2.2f]` (count, `min1`, `max1`, `min2`, `max2`) on standard error. Return 0.

**103.7 At one vertex** (`MRIScomputeSecondFundamentalFormAtVertex(mris, vno, vertices, vnum)`): 0
for a plane; -4 (`ERROR_BADPARM`) for a ripped vertex or `vnum` at or below 0. One pass of 103.2 over
the given vertices without a threshold: every unripped one's row taken (`r` increases for each), its
`k` kept for the extremes only when `rsq` is not below `FLT_EPSILON`. The solve as 103.3, except: a
singular `Uᵀ U` gives eigenvalues 0 with the eigenvector matrix **of the last call that computed
one** (the function keeps it between calls; zeros before the first) and goes on to the vertex's
curvatures; `cond >= 500000` sets `k1 = kmax`, `k2 = kmin` and `K`, `H` to the surface's `Kmin`, `Hmin`
when `kmin kmax < 0`, else `Kmax`, `Hmax`, and returns -4; a failed eigensystem gives the singular
values' curvatures and -4. The vertex's curvatures as 103.4 but `Ktotal = ((double)k2 k1) area +
Ktotal`, `e1z = a11 e1z + e2z a21`, `e2z = a12 e1z + e2z a22`; no file; with `DIAG_SHOW` and a
singular fit `1 ill-conditioned points`. Return 0.

## 104. The gradients' averaging (queue item 3.1-51; relay 20)

**104.1 `MRISaverageGradients(mris, num_avgs)`**: with `Gdiag_no` above 0 and `DIAG_VERBOSE`,
`MRISaverageGradients()` on standard output. Nothing more for `num_avgs` at or below 0. With
`Gdiag_no` at or above 0 (a vertex of the surface): `before averaging %d times dot = %2.2f ` (the dot
`(dx nx + dy ny) + dz nz` of that vertex's gradient and normal). The vertices that change are the
unripped ones with at least one unripped first-`vnum` neighbour (counted with repeats). `num_avgs`
passes: each such vertex's new gradient is `(d + d1 + ... + dk) * (1.0f / (k + 1))` (its own and its
unripped neighbours' gradients from the previous pass, summed in order; multiplied by the `float`
reciprocal, **not** divided); the other vertices keep theirs; `tdx`... are untouched. **FS-379**
(class a, memory): when a vertex's unripped neighbour has no unripped neighbour of its own (neighbour
lists that are not symmetric), FreeSurfer counts it but leaves it out of the vertex's list and reads
other vertices' neighbours (and past its list for the last); both modes average with that
neighbour's unchanged gradient, noted. With more than 150 passes, the first such call of the program
reads `FREESURFER_MRISaverageGradients_optimize_suppress` and, when it is set, writes
`FREESURFER_MRISaverageGradients_optimize_suppress so not optimizing` (standard output); the
numbers do not change. With `Gdiag_no` at or above 0: ` after dot = %2.2f D = (%2.2f, %2.2f, %2.2f),
N = (%2.1f, %2.1f, %2.1f)` (the dot `(nx dx + ny dy) + nz dz`, the gradient, the normal). Return 0.

**104.2 `MRISaverageGradientsFast(mris, num_avgs)`**: with `Gdiag_no` above 0 and `DIAG_VERBOSE`,
`MRISaverageGradientsFast()`. `num_avgs` passes in which each unripped vertex's gradient becomes `(d +
d1 + ... + dk) / (k + 1)` over its own and its unripped neighbours' previous gradients (a vertex
without unripped neighbours keeps its own); at the end each unripped vertex's `tdx, tdy, tdz` copy its
gradient. Return 0.

**104.3 `mrisAverageSignedGradients(mris, num_avgs)`**: nothing for `num_avgs` at or below 0. With
`Gdiag_no` at or above 0: `before averaging dot = %2.2f ` (as 104.1's). `num_avgs` passes: each
unripped vertex sums, in `double`, its own gradient and those of its unripped first-`vnum` neighbours
whose dot product with its own (`(v.dx n.dx + v.dy n.dy) + v.dz n.dz`, `float`) is not below 0 (a NaN
counts), divides by the count (as a `float`, then `double`) into `tdx, tdy, tdz` (`float`), then every
unripped vertex's gradient becomes its `td`. With `Gdiag_no` at or above 0: ` after dot = %2.2f
(%2.3f, %2.3f, %2.3f)`. Return 0.

## 105. Surface bookkeeping (queue item 3.1-51; relay 20)

**105.1 `MRISrescaleMetricProperties(surf)`**: 0 when `group_avg_surface_area` is 0. Else `scale =
sqrt((double)group_avg_surface_area / total_area)` (a negative ratio: NaN, `errno` `EDOM`), `scale =
%lf` on standard output, `s2 = scale scale`; each unripped vertex's area `(float)(area s2)` and its
distance to each unripped first-`vnum` neighbour `(float)(dist scale)`; `total_area = (float)(total_area
s2)`; `avg_vertex_dist = scale avg`, `std_vertex_dist = std scale`, `avg_vertex_area = s2 avg` (`double`).
Returns `scale`.

**105.2 `MRISnormalize(mris, dof, which)`**: `dof` at or below 0: `MRISnormalize: invalid dof %d` as
an error (§2), return -4. Else for each unripped vertex: `which` 2 (`VERTEX_AREA`): `origarea =
origarea / dof`; 3 (`VERTEX_CURV`): `curv = curv / dof`; anything else: `val = val / dof`; the result is
`m`; then `val2 = val2 / dof - m m` (`dof` as `float`). Return 0.

**105.3 `MRISupdateSurface(mris)`**: the metric properties (§38), then §103 with -1. Return 0.

**105.4 Orientation changes** (`MRISmarkOrientationChanges(mris)`; messages on standard error): every
unripped vertex's `curv` 0 (`MRISclearCurvature`, §22); then for each unripped vertex `v0`: `vertex %d
without neighbors ! ` + newline and `curv -5` when `vnum` is 0; `vertex %d without faces ! ` + newline
when it has no face; for each first-`vnum` neighbour `v1`: if ripped, `vertex %d connected to ripped
vertex %d !` + newline + a blank; the faces through the edge: for each other neighbour `v2 != v1` (in
order) whose triple `(v0, v1, v2)` is a face (a face of `v0` whose three vertices are all among the
three; the first such face in `v0`'s list), counted, the first one's face number `f1`, the latest
other one's `f2` (both kept across edges and vertices, 0 at the start). One face: `edge (%d<-->%d) has
one single face %d !` (`f1`), `curv -2`; none: `edge (%d<-->%d) has no face !`, `curv -3`; more than
two: `edge (%d<-->%d) has too many faces (at least : %d %d) !`, `curv -4`; two whose orientations agree
(the orientation of face `f` from `a` to `b`: 1 when its corners 0, 1 are `a, b`, or 1, 2 are `a, b`,
or `a` is at neither 0 nor 1 and corner 0 is `b`; else -1; the two orientations' product above 0):
`curv -1`, a newline then `defective orientation at vertex %d(%d) with faces %d and %d`, counted. At
the end a newline then `%2.3f %% of the vertices (%d vertices) exhibit an orientation change` with
`100 count / nvertices` (`double`; 0/0 the default NaN). Returns the count.

**105.5 `mrisCheckSurface(mris)`**: for each unripped face, for each of its corners whose vertex's
face list lacks the face: `mrisCheckSurface: %s: face[%d].v[%d] = %d, but face %d not in vertex %d face
list` (the surface's file name) on standard output. Return 0.

**105.6 `MRISsubsampleDist(mris, spacing)`** (`float`): every vertex's `d = 10000`, `val = 0`. Forward,
for each vertex `k`: for each first-`vnum` neighbour `m`, `d_m + 1 < d_k` → `d_k = d_m + 1`; then
`d_k >= spacing` → `d_k = 0`, `val = 1`, counted; then for each neighbour `d_m > d_k + 1` → `d_m = d_k +
1`. Backward, for each vertex from the last: for each neighbour both updates in that order. When
`spacing` is 2: for each vertex with `d > 0`: when at most two neighbours have `d == 0`, it becomes
`d = 0`, `val = 1`, `fixedval` 1, counted; and its neighbours' `d` lowered as above. Returns the count.

**105.7 `MRISextendedNeighbors(surf, targ, cur, thresh, list, dots, &n, max, type)`** (recursive,
depth first): returns 0 when `cur`'s `val2bak`, converted to an integer as x86-64 truncates (`INT_MIN`
for a NaN or out of range), equals `targ`, or when `cur` is ripped. At `cur == targ` the count `n`
becomes 0. With `type` 1: `p = |(double)((cy ty + tx cx) + tz cz)|` (`float` products and sums of
the two vertices' current positions), 0 when `p <= thresh` (a NaN goes on); otherwise `p =
|dx² + dy² + dz²|` (`double` squares of the `float` differences target minus current; `(x² + y²) +
z²`), 0 when `p >= thresh`. Returns 1 when `n >= max - 1`. Else `cur` and `p` are appended, `n`
increases, `cur`'s `val2bak = (float)targ`, and each first-`vnum` neighbour of `cur` is searched in
order; the first non-zero result is returned; else 0.

**105.8 `MRISunion(a, b)`**: when the volume geometries differ (fs.volume's `vg_isEqual`): `ERROR:
MRISunion() - VolGeom of both surface are not the same!!!` (standard output), no surface; when
`useRealRAS` differs: `ERROR:  MRISunion() - input surfaces are not in the same space!!!` (two
blanks), no surface. Else a new surface (`MRISalloc` of both counts) with `a`'s `useRealRAS` and
volume geometry, `a`'s then `b`'s vertex positions (only the positions, set as §14's `MRISsetXYZ`),
`a`'s faces then `b`'s with `b`'s vertex numbers plus `a`'s vertex count.

**105.9 `MRISclone(src)`**: a new surface (`MRISalloc`) receiving: type, status, `origxyz_status`,
`nsize`, `max_nsize`, `vtotalsMightBeTooBig`, `nsizeMaxClock`, hemisphere, centre, bounding box,
curvature extremes, total and original area, radius, average vertex area, average distance and its
spread, the transform and its two matrices (FreeSurfer shares them; a copy here), `free_transform` 0,
the three poles (the same vertex numbers), the volume geometry; per vertex: rip flag, current
position, normal, canonical position, curvature, face list and its places, `vnum` (§5's counter
with its maximum message), `v2num`, `v3num`, `nsizeMax`, `nsizeMaxClock`, `nsizeCur` (setting `vtotal`
as §88's `MRIS_setNsizeCur`), the neighbour list (its whole room), the distances and original
distances when the source has them, border, area, original area; every face whole (its normal
matrices copied). Every other field is the new surface's (zero).

**105.10 Vertex reports.** `MRISprintVertexStats(mris, vno, fp, which)` (standard output whatever
`fp`): nothing for `vno` below 0; `vertex %d spacing for %s surface` (`orig`, `current`, `white` for
those coordinate sets, else `unknown`); per first-`vnum` neighbour, a ripped one's `nbr %d = %d is
ripped`, else `\tvn %d: %d = %2.3f mm distant` with `sqrtf(dz² + (dx² + dy²))` of the `which`
positions' difference (§3's `MRISvertexCoord2XYZ_float`, which refuses some sets); `\tmean = %2.3f`
(the distances summed as `d + sum` in `double`, divided by the count; 0/0 a NaN). Return 0.
`MRISprintVertexInfo(fp, surf, vno)`: into `fp`: `vertexno %d`, `xyz  %7.4f %7.4f %7.4f`, `nxyz %7.4f
%7.4f %7.4f`, `vertex_area %lf`, `neighbors %d %d` (`vnum`, number of faces); then for `n` from 0 to
the number of faces (sic) `nbr %d  %5d %6.4lf %6.4lf %5d %6.4f`: `n`, the `n`-th entry of the neighbour
list, `sqrt((dx² + dy²) + dz²)` (`double` of the `float` differences vertex minus neighbour), the
neighbour's area, the `n`-th face and its area. **FS-380** (class a, memory): a vertex with more
faces than room in its neighbour list makes FreeSurfer read past the list; both modes stop at its
end (noted). Then, when the surface has edges, for each edge with the vertex at end 0 or 1: `Edge
%6d %d  %4d %4d %7.3lf %7.3lf` (edge, end, its two vertices, length, angle) on standard output. Return
0.

**105.11 Not yet specified here** (their items wait on other modules): `mrisFindMiddleOfGray`,
`mrisMarkIntersections`, `MRISprintSurfQualityStats`, `MRISprettyPrintSurfQualityStats`, the
volume users and the label users of `mrisurf_metricProperties.cpp` lines 11687-12041 and 12395-13180,
`MRISaverageGradientsFastCheck`.

## 106. Validation plan, part 18

The probe's `mpe` command (relay 20): an empty work folder with `d`; `Gdiag`'s show, verbose and
(one case in six) write bits; a binary triangle file read by `MRISread` (icosahedra subdivided once
or twice on a sphere of radius 1, 10, 50 or 100, open grids of 2 to 6 by 2 to 6, `srd`'s meshes);
the status (one case in four), `Kmin`, `Kmax`, `Hmin`, `Hmax` (one in three), `Gdiag_no` (one in five
a vertex) and `FREESURFER_MRISaverageGradients_optimize_suppress` (one in three) given before the
read; per vertex a mask (ripped, a mark, position, normal, curvature, `k1 k2`, `K H`, area, original
position, gradient, `val val2 origarea`), per face (ripped, area, cached original area, cached
normal), values as `mpd`'s (NaNs and infinities in one case in five); then 1 to 4 operations of
§102-105: smoothed normals (0 to 3), boundary normals, mean curvature, the curvature choices (three
functions for `MRISusePrincipalCurvatureFunction`: `k1 + 1`, `k2 * 0.5`, the larger), curvature
indices, largest radius, mean and spread (with or without the spread), negative faces' marks (-1 to
3) and rips, the second fundamental form (threshold -1 one in three, else 0.999, 0.99, 0.9, 0.5, 0, 1,
0.75 or a NaN), at one vertex (1 to 3 calls; the tangent planes first one in four; the neighbours of
the vertex's faces, none one in eight, random ones one in eight), the three averagings (-1 to 6
passes, 151 to 156 one in eight), rescaling (the group area 0, 100, 1000, 2.5, 50000 or -4), the
normalisation (`dof` -1 to 7, `which` 0 to 10), the update, orientation changes, the face-list
check, subsampling (spacing 0.5 to 4), extended neighbours (`val2bak` reset to -1 one in four; a
target; type 1 with thresholds 0 to 2500 or type 0 with 0.25 to 10000; at most 1 to 40), the union
with a clone (equal, `useRealRAS` flipped, the geometry changed), the clone, both vertex reports.
Each prints its result and what it changes; `DIAG_WRITE`'s files (`curv.dat`, `k1.plt`, `k2.plt`) by
length and FNV-1a hash. Counted, not compared: FS-376, FS-379, FS-380, and FS-48's matrices on which
FreeSurfer hangs (the probe's alarm ends it). FS-377's cases compare as FreeSurfer's crash.
Observed (relay 20): the probe README.
