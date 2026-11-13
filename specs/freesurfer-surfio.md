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
original area and storing and restoring the metric properties. Later parts: reading and writing
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
