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
first part, §14-16): the neighbour distances' storage and the coordinate operations. Later parts: reading and writing
surfaces, curvatures, patches and overlays (3.1-47, 3.1-48), the metric properties every read
computes (3.1-49 to 3.1-52); the midline marking, the label normal and the coordinate copy (they
need the volume structure).

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
