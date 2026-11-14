# FreeSurfer 8.2.0's volume filters: behaviour spec (`fs.filter`)

Written by the replica author of queue items 3.1-42 to 3.1-44 as their spec writer
(`docs/porting-guide.md` §1): behaviour, maths, conventions, messages and the validation plan, from
FreeSurfer 8.2.0's source and the official build's outputs, never code or routines. A clean-room
implementer writes the clean room from this file, black-box runs of the official programs (and of
the probe `validation/probes/fs-filter/`, a black-box run of FreeSurfer's code),
`specs/freesurfer-mri.md` (the structure these functions work on), `specs/freesurfer-volume.md` (the
voxel functions they call) and the cited standards.

Status: **parts 1 to 4**. Part 1 (§1-8): the fast-marching distance maps `MRIextractDistanceMap`
and `MRIdistanceTransform`. Part 2 (§9-17): the Gaussian kernel, the one-dimensional and separable
convolutions, the matrix Gaussian smoothing, the window means and standard deviations, the pyramid
reductions, the Sobel derivatives, the divergence and Laplacian, the white-matter border
classification, the connected components and the global Sobel tables `sx`, `sy`. Part 3 (§17-18):
the frame functions (`fMRI*`, `MRIframe*`, `MRIvol*`, `MRIconjunct`, `ASLinterpMatrix`). Part 4
(§19-21): the volume clusters (`clust*`, `MRIremoveVolumeIslands`, `MRIremoveVolumeHoles`) and the
non-parametric white matter and cortex classification (`MRIreclassifyWMCtxNonPar`,
`MRIclassifyAmbiguousNonPar`). Not yet: 3.1-43's intensity normalisation, segments and flood fill
(`mrinorm.cpp`, `mrisegment.cpp`, `mriflood.cpp`, `rfa.cpp`'s `csf_labels`). The 2-D image reductions (`ImageConvolve1d`,
`ImageReduce`, `ImageReduce1d`) are specified with the images in `specs/freesurfer-io.md`.

## 1. Conventions

- Those of `specs/freesurfer-mri.md` §1 (x86-64 single-precision arithmetic, its NaNs: a NaN made
  from numbers is the default NaN `0xffc00000`, a NaN operand is passed on quieted, of two NaN
  operands the one named first below; messages).
- A voxel `(x, y, z)` of a volume of `w x h x d` is the `((z h + y) w + x)`-th voxel of frame 0;
  "the next voxel in memory" of `(x, y, z)` is the one after it in that order (the first voxel of
  the next row when `x = w - 1`).
- A value *truncates* or *rounds to an integer* as C's `(int)` and `(int)round(v)` do on x86-64:
  `round` halves away from zero; a NaN or a value at or beyond 2^31 in magnitude (after rounding)
  gives -2147483648 (FS-542 below: the default clamps to the range's end, NaN to 0, and notes it).
- `MRIgetVoxVal` as `specs/freesurfer-volume.md` gives it (indices outside the volume: its
  defects FS-52, FS-53).

## 2. `MRIextractDistanceMap(src, dst, label, max_distance, mode, mask)`

The distance of every voxel to the boundary of the *label voxels* (those of `src`, frame 0, whose
value rounds to `label`), measured on the voxel grid with unit spacing (the voxel sizes are not
used), by fast marching (Sethian 1996; the first-order upwind update of §4).

1. `max_distance` at or below 0 (not NaN) becomes `2 * max(w, h, d)` (the source's extent, as an
   integer converted to float).
2. Without `dst`: a new float volume of the source's extent, one frame, with the source's header
   (`MRIcopyHeader`). With `dst`: when its width, height or depth differs from the source's or it
   is not a float volume, standard error receives, for each differing extent, the two lines
   `ERROR : incompatible structure with mri_src:` and `mri_dst-><f>=<a> != mri_src-><f>=<b>` (`<f>`
   width, height or depth, `%d` numbers) and, for the type, `ERROR : incompatible structure with
   mri_dst:` and `mri_dst->type=<t> != MRI_FLOAT`; `dst` is returned unchanged.
3. Frame 0 of the result is set to 0. Then, by `mode`: 1 (outside) one outward pass; 2 (inside) one
   inward pass; 3 (both) the outward then the inward pass; 4 (both, unsigned) both passes and then
   every voxel of frame 0 replaced by its absolute value (the sign bit cleared, NaNs included).
   Another mode: nothing more (a map of zeros, without a message: FS-544; the default ends the
   program with `MRIextractDistanceMap: unknown mode <m> (1 outside, 2 inside, 3 both, 4 both
   unsigned)`, an error of FreeSurfer's library: `ERROR_BADPARM`).
4. The result is returned (the new volume, or `dst`). Frames after 0 are not touched.

The outward pass gives the voxels outside the label positive distances (the voxels on the
boundary 0.5, the label voxels keep their value), limit `L = max_distance`; the inward pass
negative distances inside (-0.5 on the boundary), limit `L = -max_distance`; each is §3 with `s =
+1` or `-1`. Every voxel the march does not reach holds `L`.

## 3. One pass (`s` = +1 outward, -1 inward; limit `L`)

Each voxel has a *state*: alive, trial, far or forbidden. All start far.

1. **Mask.** With `mask`: each voxel whose mask value (`MRIgetVoxVal(mask, x, y, z, 0)`, the
   mask's own extent: reads outside it as `MRIgetVoxVal` gives them) truncates to 0 is forbidden,
   and its value is set to `L` (FS-541: the official code sets it from a limit not yet set; the
   value is replaced in step 6 before anyone can read it, except through FS-539).
2. **Labels.** In the order `z`, then `y`, then `x` (x fastest): with `s = +1`, a voxel whose label
   (§1, rounding) is not `label` gets the value `L`, a label voxel becomes forbidden; with `s = -1`,
   a label voxel gets `L`, the others become forbidden.
3. **Boundary.** In the same order, for each voxel `p` and its three forward neighbours `q` (x + 1,
   then y + 1, then z + 1; a neighbour past the last index is `p` itself): when the labels of `p`
   and `q` differ and one of them is `label`, `q` is *made alive*; after the three, when any of them
   was, `p` is made alive. *Making alive* a voxel that is not forbidden sets its value to `s * 0.5`
   and, when it is far, makes it alive and appends it to the *alive list*.
4. **First trial points.** For each voxel of the alive list in order, its neighbours x - 1, x + 1,
   y - 1, y + 1, z - 1, z + 1 (those inside the volume), in that order: a far neighbour becomes
   trial, gets the value of §4 and is pushed on the queue (§5).
5. **March.** While the queue is not empty: let `t` be its top. When `s * value(t) >= s * L`, every
   voxel still in the queue becomes far with the value `L`, the queue is emptied and the pass ends.
   Otherwise `t` is popped and becomes alive; then each of its neighbours x - 1, x + 1, y - 1, y +
   1, z - 1, z + 1 (inside the volume), in that order: a far one becomes trial, gets the value of §4
   and is pushed; a trial one gets the value of §4 again **without being moved in the queue**; the
   others are left.
6. **End.** With `mask`: each voxel whose mask value truncates to 0 gets the value `L`.

## 4. The value of a trial point `(x, y, z)`

`g(n)` of a neighbour is `L` when it is far, else its value. On each axis, `a` is `s * g` of the one
neighbour when the point is at the axis's first index (the next one, x + 1) or its last (the
previous one), else the smaller of `s * g(next)` and `s * g(previous)` (`a < b ? a : b` with `a` the
next: a NaN comparison takes the previous). `A`, `B`, `C` are the three axes' (x, y, z) values put
in order by three compare-and-swaps: (A, B), then (B, C), then (A, B), each swapping when the first
is greater (`>`, false for a NaN). With `s * v` computed as a sign flip for `s = -1`, in single
precision, each operation's NaN as §1 with the operand written first taken first:

- `r = s * (A + 1)` (A first);
- `P = A*A + B*B` (for `s = -1`; `B*B + A*A` for `s = +1`), `S = (A + B) + C`;
- `D = S*S - ((C*C + P) - 1) * 3`; when `D >= 0`: `t = (sqrt(D) + S) / 3` (`sqrtss`); when
  `1e-6 + t >= C`, the value is `s * t`;
- otherwise `E = (A + B)*(A + B) - (c + c)` with `c = P - 1`; when `E >= 0`: `t = ((A + B) +
  sqrt(E)) * 0.5`; when `1e-6 + t >= B`, the value is `s * t`;
- otherwise `r`.

(The constants are floats: 1, 3, 0.5 and `1e-6f`.) A point on an axis of one voxel has no neighbour
on that axis: the official code reads one past the edge (FS-539: along x, the next voxel in memory,
its state and value as `g` gives them; FS-540: along y or z, and along x from the volume's last
voxel, memory outside the volume, so memory contents or a crash). The default takes `g = L` there
(no neighbour, as a far one); `reproduce` follows FS-539's next voxel and fills FS-540's with `L`.

## 5. The queue

A binary heap in an array, as libstdc++'s (GCC 8.4.0) `std::priority_queue` over a vector keeps it,
ordered by the comparison `less(a, b) = s * value(a) > s * value(b)` read from the volume's
**current** values each time it is made (so for `s = +1` the smallest value is on top). Because a
trial point's value can be lowered while it is queued (§3 step 5), the array is not always a heap:
the points leave in the order these exact steps give, which decides the distances.

- **Push `v`:** append `v` at index `n` (the old size); then, with `hole = n`, while `hole > 0` and
  `less(heap[(hole - 1) / 2], v)`: move the parent into the hole and let the hole be the parent.
  Put `v` in the hole.
- **Pop** (size `n`): when `n > 1`: let `v` be the last element (index `n - 1`) and put the top
  there; with `len = n - 1`, `hole = 0`, `child = 0`: while `child < (len - 1) / 2`: `child = 2
  (child + 1)`; when `less(heap[child], heap[child - 1])`, `child = child - 1`; move `heap[child]`
  into the hole and let the hole be `child`. Then, when `len` is even and `child == (len - 2) / 2`:
  `child = 2 (child + 1)`, move `heap[child - 1]` into the hole, the hole is `child - 1`. Then sift
  `v` up from the hole as in a push, but stopping at index 0 (`hole > 0`). Finally drop the last
  element (the old top). Integer divisions truncate.
- **Top:** element 0.

## 6. `MRIdistanceTransform(src, dst, label, max_dist, mode, mask)`

1. Without `dst`: a new float volume of the source's extent with its header; with `dst`: every
   voxel of every frame of `dst` set to 0 (`MRIclear`).
2. `mode`: `DTRANS_MODE_SIGNED` (1) becomes §2's 3 (negative inside, positive outside),
   `DTRANS_MODE_UNSIGNED` (2) becomes 4, `DTRANS_MODE_OUTSIDE` (3) becomes 1; `DTRANS_MODE_INSIDE`
   (4) stays 4: the unsigned map on both sides (FS-543; by default 2, the distances inside, 0
   outside); any other value is passed on as it is (FS-544: §2's modes 1 to 4 then apply to it, and
   any other gives zeros; by default the program ends with `MRIdistanceTransform: unknown mode <m>
   (DTRANS_MODE_SIGNED 1, UNSIGNED 2, OUTSIDE 3, INSIDE 4)`).
3. A source that is not a float volume is changed to one first (`MRIchangeType(src, MRI_FLOAT, 0,
   1, 1)`: the values copied); §2 runs on it (or on `src`) into the volume of step 1.
4. The volume's `outside_val` becomes `max_dist` (as given, before §2's replacement of a value at or
   below 0), and every voxel of every frame is multiplied by the source's column size `xsize`
   (`MRIscalarMul`: in single precision, `MRIsetVoxVal`).
5. The volume is returned.

## 7. Validation plan

- **Probe (black box).** `validation/probes/fs-filter/` runs FreeSurfer's two functions case by
  case: `fsfilter <cases>` prints, for each case line, `same <w> <h> <d> <type> <frames>
  <outside_val> <xsize> <voxels>` (`same` 1 when the result is the given destination; voxels as
  their stored values, floats as their bits, `v*k` for k equal ones) and the text FreeSurfer
  printed. The cases (`check.py`, seeded): volumes of 1 to 32 voxels a side (a side of 1 often),
  label volumes of every integer type and floats (values rounding to the label or past it, NaN,
  infinities, beyond the `int` range), destinations of the right and wrong extents and types,
  masks of the right and other extents (bytes, floats with NaN and fractions), limits at or below
  0, small, large, NaN and infinite, every mode and unknown ones. `real.py` runs the pipeline's
  calls on three recorded heads (`aseg.mgz` labels 2, 3, 4, 17; `filled.mgz` 255; `wm.mgz` 110).
- **Expected.** Every line identical to FreeSurfer's under `reproduce` but those that read memory
  outside a volume (FS-540, FS-541, FS-53) or a volume without voxels (FS-58), which are counted;
  the default differs only where a defect it corrects is noted. Observed at seeds 7 (300 cases),
  13 and 17 (600 each): 0 different (observed); the recorded heads: §8.

## 8. Defects

| ID | Class | What | Default |
|---|---|---|---|
| FS-539 | a | an axis of one voxel along x: the next voxel in memory read as the neighbour | no neighbour (`L`) |
| FS-540 | a | an axis of one voxel along y or z, or past the last voxel: memory outside the volume | no neighbour (`L`) in both modes |
| FS-541 | a | with a mask, masked voxels set from a limit not yet set (read only through FS-539) | the pass's limit |
| FS-542 | c | label or mask values NaN or beyond the `int` range become -2147483648 | clamped, NaN 0 |
| FS-543 | c | `DTRANS_MODE_INSIDE` gives the unsigned map | the inside map |
| FS-544 | e | an unknown mode gives a map of zeros without a message | an error |
| FS-545 | c | a convolution of a volume other than uchar or float into a float one: the outside value before a row and the next row's voxels past it | the edge voxel |
| FS-546 | a | a convolution's destination smaller than the source, or a frame either volume lacks: other voxels or memory outside | an error, `NULL` |
| FS-547 | c | a convolution into uchar from float reads the float bytes; into short or int reads any source as shorts or ints | the source's values |
| FS-548 | a | the separable convolution's kernel volume not float: its memory read as floats | an error (both modes) |
| FS-549 | a | a kernel over 1,001 taps (or a classification window over 1,001 voxels) reads past the index tables | the edge voxel |
| FS-550 | a | the separable convolution of a volume without frames crashes | an error (both modes) |
| FS-551 | a | the matrix smoothing along an axis of 0 voxels writes outside a matrix | an error (both modes) |
| FS-552 | a | the masked smoothing with a target of another size crashes after the smoother's message | an error (both modes) |
| FS-553 | c | the reduction of one slice without a destination returns a volume of depth 0 | the reduced slice (depth 1) |
| FS-554 | c | the Laplacian of several frames takes every centre voxel from frame 0 | the frame's own |
| FS-555 | a | a Sobel destination smaller, without the frame, or (for a source not uchar) not float: other voxels or memory outside | an error, `NULL` |
| FS-556 | e | the divergence ignores the destination it is given | the destination filled and returned |
| FS-557 | c | the classification's values NaN or beyond the `int` range become -2147483648; 32-bit squares wrap | clamped (NaN 0); 64-bit squares |
| FS-558 | c | the classifier's gray likelihood uses the white variance; both likelihoods 0 give a NaN white probability (white) | kept in both modes (rule D: measured at `mri_segment`'s port) |
| FS-559 | a | the border labels of a uchar volume written into a destination not uchar or smaller | an error, `NULL` |
| FS-560 | a | the class statistics count negative or NaN intensities in a bin before the histogram | left out of the histogram (both modes) |
| FS-561 | a | the region growing with a label its volume stores as 0 never ends when two object voxels touch | an error (both modes) |
| FS-562 | a | the covariance or AR(1) map written directly as floats into a given output that is not float or is smaller | other voxels' bytes as FreeSurfer writes them; nothing outside (both modes) |
| FS-563 | c | the AR(1) of a voxel of variance 0 is its covariance divided by 0 | 0 |
| FS-564 | a | the AR(1) or the normalisation of a series whose covariance cannot be computed crashes after the covariance's message (unless every voxel is masked out) | an error (both modes) |
| FS-565 | a | a degrees-of-freedom count, or a subsampled frame count, NaN or outside the `int` range becomes -2147483648 | saturated, NaN 0; the frame count an error |
| FS-566 | a | `ASLinterpMatrix(1)` writes one element past the matrix | dropped (the matrix the same) |
| FS-567 | a | `fMRItoMatrix` into a given matrix with fewer rows or columns than needed writes into other elements and past it | an error, `NULL` |
| FS-568 | a | `fMRIfromMatrix` with a mask of no voxel divides by 0 (a crash) | an error (both modes) |
| FS-569 | a | `fMRIsubSample` whose frames never reach the stop (a negative step, or the frame number past the `int` range) reads without end (a crash) | an error (both modes) |
| FS-570 | a | cluster members' coordinates never computed (no transform) are memory contents; pruning by distance reads them | 0, noted (both modes) |
| FS-571 | a | cluster coordinates with a transform that is not 4 x 4 crash after the product's message | an error (both modes) |
| FS-572 | c | the class histograms end at the smaller maximum; the upper ambiguity threshold is searched from the first bin | kept in both modes (rule D: measured at `mri_segment`'s port) |
| FS-573 | a | an ambiguity threshold search that finds no bin reads one bin outside the histogram | the edge bin (both modes) |
| FS-574 | c | the masks' `NdilWM`/`NdilCtx` dilations each restart from the undilated mask (one voxel) | kept in both modes (rule D) |
| FS-575 | a | a dilation count of 0 or less crashes | an error (both modes) |

## 9. The Gaussian kernel and the one-dimensional convolutions

**`MRIgaussian1d(sigma, max_len)`** returns a float volume of `len` x 1 x 1:

- `len = nint(8 sigma) + 1` (the product in single precision; `nint` as `specs/freesurfer-core.md`
  gives it, FS-12), made odd by adding 1 when even; then `max_len` when it is above 0 and below
  `len` (which may make it even); then at least 1. `half = len / 2` (integer division) is taken
  before that last step.
- `len` 1: the single weight 1.
- Otherwise, for `x = 0 .. len - 1`, `f = x - half` (as a float) and `a = |f|`: when `a <= 2 sigma`,
  the weight is the double exponential `exp(q)` of `q = (-f · f) / (sigma · 2 sigma)` (all single
  precision, `q` widened), rounded to float; when `2 sigma < a <= 4 sigma`, it is
  `c · (float)pow(4 - a / sigma, 4)` with `a / sigma` and the difference in double, `pow` of double,
  and `c = 1 / (16 (float)(e²))` in single precision (`e²` the double product of `M_E` by itself);
  else 0. The weights are summed in single precision in order and each divided by the sum. `exp`
  and `pow` are glibc's (GLIBC-07 with the default's correctly rounded ones).

**`MRIconvolve1d(src, dst, k, len, axis, src_frame, dst_frame)`**: the convolution of frame
`src_frame` of `src` with the `len` weights `k` along `axis` (`MRI_HEIGHT` 0, `MRI_WIDTH` 1,
`MRI_DEPTH` 2), into frame `dst_frame` of `dst`, over the source's extent:

1. When the source has one voxel along `axis`, the result is `MRIcopy(src, dst)` (every frame;
   `specs/freesurfer-mri.md`'s copy and its FS-163).
2. Without `dst`, a float volume of the source's extent and one frame (no header copied).
3. A destination of type uchar, short or int goes to `MRIconvolve1dByte`, `...Short`, `...Int`
   (below); another type but float: the error `MRIconvolve1d: unsupported dst pixel format <type>`
   and `NULL`.
4. An `axis` other than the three: the destination is returned as it is.
5. Each output voxel at `(x, y, z)` is `total`, starting at +0 and for `i = 0 .. len - 1` becoming
   `total + v_i · k[i]` (single precision; the product's first operand is the voxel, the sum's the
   running total: NaN bits), `v_i` the source's voxel at offset `i - len / 2` along the axis.
   A uchar or float source takes the nearest voxel of the axis past its ends; another type takes
   `MRIgetVoxVal` at the offset itself (the outside value before the start, the next row's or
   slice's voxels past the end: FS-545).
6. Returned: `dst` (or the new volume).

**`MRIconvolve1dByte`, `MRIconvolve1dShort`, `MRIconvolve1dInt`** (also called directly; without a
destination they make one of their type and the source's extent): a destination of another type
gives `MRIconvolve1dByte: unsupported dst pixel format <type>` (or `Short`, `Int`) and `NULL`. Byte
takes a uchar or float source (another: `MRIconvolve1d: unsupported pixel format <type>`, `NULL`)
and reads it as bytes (a float source's bytes, FS-547); Short and Int read any source as 2-byte and
4-byte integers from its memory (FS-547). The sums as above with the nearest voxel past the ends;
each stored as `nint(total)` in the type's low bits.

A destination smaller than the source, or a frame either volume lacks, makes the accesses land on
other voxels or outside the volumes (FS-546). A kernel of more than 1,001 taps reaches past the
volume's boundary index tables (FS-549).

**`MRIconvolveGaussian(src, dst, kernel)`**: the weights are the first `width` floats of `kernel`
(a float volume; FS-548). Without `dst`, `MRIclone(src)`; a `dst` of another type than the
source's ends the program (`MRIconvolveGaussian: Source and destination types differ!` on
standard error, exit 1); `src` as its own `dst` is smoothed into a clone that is copied back
(`MRIcopy`) at the end. A source without frames crashes (FS-550). For each frame `f`: frame `f`
is copied (`MRIcopyFrame`) into a one-frame volume `t` of the source's type (made at the first
frame), then `MRIconvolve1d(src, t, width, f, 0)`, `MRIconvolve1d(t, dst, height, 0, f)`,
`MRIconvolve1d(dst, t, depth, f, 0)`, and `t`'s frame 0 copied into frame `f` of `dst`. Then the
source's header is copied onto `dst` (`MRIcopyHeader`).

## 10. The matrix Gaussian smoothing

**`MRIgaussianSmoothNI(src, cstd, rstd, sstd, targ)`** (also in place, `targ = src`):

1. Without `targ`, a float volume of the source's extent and frames into which the source is
   copied (`MRIcopy`); a `targ` of another width, height, depth or frame count prints `ERROR:
   MRIgaussianSmoothNI: <width|height|depth|frames> dimension mismatch` on standard output and
   returns `NULL`; otherwise the source is copied into it unless it is the source.
2. With `Gdiag_no` above 0: `MRIgaussianSmoothNI(): <n> avail. processors, running in <t> threads`
   (here 1 thread) and, when a mixture weight is not 1, `MRIgaussianSmoothNI(): G2: <cw1> <cstd2>
   <rw1> <rstd2> <sw1> <sstd2>` (`%g`).
3. For each axis whose deviation is above 0 (in order columns, rows, slices): the matrix
   `G = GaussianMatrix(n, std / voxel size, 1)` (`specs/freesurfer-maths.md`; with a mixture weight
   `w1` not 1, `GaussianMatrix2(n, std / size, std2 / size, w1, 1)`; `std / size` in double rounded
   to float, `std2 / size` in float), `n` the axis' voxels; every line of voxels along the axis (for
   each other-axis place and frame) is read by `MRIgetVoxVal`, multiplied by `G` (`MatrixMultiply`)
   and written back by `MRIsetVoxVal`. The lines are taken for each row and slice (columns), each
   column and slice (rows), each column and row (slices). An axis of 0 voxels: FS-551. With
   `Gdiag_no` above 0, the lines' outer indices are printed (`<i> `, a newline every 10, one at the
   end; for the columns only with `DIAG_VERBOSE` too, which also prints `Smoothing rows`).
4. The axis' scale vector is the middle row of `G` (its 1-based row `n / 2`; the single value 1 for
   an axis of one voxel). The kernel's sum is `scale = Σ a_c b_r c_s` over the smoothed axes' entries
   (1 for an axis not smoothed), each product in double (`(a b) c`), added in x87 extended precision
   in the order column, row, slice; with `Gdiag_no` above 0 `MRIguassianSmoothNI(): scale = <Lg>`
   and `MRIguassianSmoothNI(): VMF = <Lg>, VRF = <Lg>` (the squares' sum and its inverse).
5. Every voxel becomes `MRIsetVoxVal(v / scale)`, the quotient of the voxel (as a double) by the
   extended-precision sum, rounded from extended precision to float.

**`MRIgaussianSmooth(src, std, norm, targ)`**: `norm` 0 prints `ERROR: MRIgaussianSmooth: norm must
be non-zero` and returns `NULL`; else `MRIgaussianSmoothNI(src, std, std, std, targ)`.

**`MRImaskedGaussianSmooth(src, mask, std, targ)`** (also in place): without a mask,
`MRIgaussianSmooth(src, std, 1, targ)`. Otherwise a copy of the source with every frame's voxels
zeroed where the mask (frame 0) is below 0.5 is smoothed into `targ`; the mask alone is smoothed
into a new volume; then each voxel of `targ` is 0 where the mask is below 0.5 and otherwise its value
divided (in double) by the smoothed mask's. A target the smoother refuses: FS-552.

## 11. The window statistics

**`MRImean(src, dst, wsize)`**: half-width `(wsize - 1) / 2` (C's division); for each frame and
voxel, the single-precision sum of the source's voxels in the window that lie in the volume, each
added as `voxel + sum` (NaN bits), divided by their count (a float); a voxel whose window holds none
is not written. Without `dst`, a float volume of the source's extent and frames with its header.

**`MRIstd(src, dst, mean, wsize)`** (half-width `wsize / 2`, frame 0): each voxel the square root of
`Σ (v - m)² / n` over the window's voxels in the volume, `m` the mean volume's voxel, the squares
added as `square + sum`, the root of the float quotient; 0 when `n` is 0. **`MRIstdNonzero`**: the
same over the window's voxels that are not zero (`|v| < FLT_EPSILON`, `<float.h>`'s), and voxels
whose mean is zero left as they are. Both: without `dst`, a float volume with the source's header;
a `dst` that is not float: `MRIstd: dst must be MRI_FLOAT` and `dst` returned unchanged.

## 12. The pyramid reductions

The reduction kernel is `{e, 0.25, 0.4, 0.25, e}`, `e = 0.25 - 0.4 / 2` in single precision.

**`MRIreduce1d(src, dst, k, len, axis)`**: each voxel `(x, y, z)` of `dst` (frame 0) is the sum
`total + v_i · k[i]` (as in §9) of the source's voxels (`MRIgetVoxVal`, frame 0) at `(2x, 2y, 2z)`
with the `axis` coordinate `2 c + i - (len - 1) / 2` cut to the axis' ends; `MRIsetVoxVal`. Without
`dst`, a volume of half the source's extent (C's division) and its type, `imnr1 = imnr0 + depth -
1`, the voxel sizes doubled (`MRIsetResolution`). **`MRIreduceSlice`**: the same within each slice
(the slice index kept; only the width and height axes act); without `dst`, a uchar volume of half
the width and height and the source's depth (sizes doubled in x and y).

**`MRIreduce(src, dst)`**: a width or height of 1 or less ends the program (`MRIreduce:
insufficient dimension (<w>, <h>, <d>)`). The source convolved along the width with the kernel
(§9, a new float volume); for a depth above 1, along the height too, then `MRIreduce1d` along the
depth; for one slice, `MRIreduce1d` along the height of the width's convolution (FS-553 without a
destination). Then the source's header, `MRIsetResolution(2 xsize, 2 ysize, zsize or 2 zsize)`, the
start and end coordinates the source's, and the voxel-to-RAS matrix the source's times
`diag(2, 2, 1 or 2, 1)` (`MRIsetVoxelToRasXform`, `specs/freesurfer-volume.md`).

**`MRIreduce2D(src, dst)`**: a width or height of 1 or less ends the program (`MRIreduce2D:
insufficient dimension (<w>, <h>)`). The width's convolution reduced along the height by
`MRIreduceSlice`, then the sampled header: the source's header, `thick`, `ps` and the voxel sizes
doubled (single precision), `scale` doubled (integer), the centre `MRIcalcCRASforSampledVolume`'s
(which fills the source's cached matrices when it has none), the cached matrices recomputed.

## 13. The Sobel derivatives, divergence and Laplacian

The x and y filters act in each slice on the rows 1 to `h - 2` and columns 1 to `w - 2`; the z
filter on the slices 1 to `d - 2`, every row, columns 1 to `w - 2`; other voxels of the destination
are not written, nor is anything for an extent below 3 along a filtered axis. Output into frame
`frame` of the destination (without one: a float volume of one frame with the source's header).

- **x**: at column `x`, `(right - left) / 8` with `left`, `right` the `[1 2 1]` sums `t + 2m + b`
  over the rows `y - 1, y, y + 1` at columns `x - 1`, `x + 1`.
- **y**: `(right + 2 middle + left) / 8` of the differences `b - t` (rows `y + 1` minus `y - 1`) at
  columns `x - 1, x, x + 1`. **z**: the same with slices `z + 1` minus `z - 1`.
- A uchar source (frame 0, read directly) is summed in integers; its destination must be float
  (else `MRIxSobel: unsupported src or dst type (<src> or <dst>)`, or y, z, and `NULL`). Another
  source is read by `MRIgetVoxVal` (frame 0) in single precision, `2m` as `m + m`, `/ 8` as
  `· 0.125`; NaN bits: x's first sum `(2m + t) + b`, its second `(t + 2m) + b`, the others
  `b + (2m + t)`; y's and z's `(2 middle + right) + left`. A destination smaller, without the
  frame, or not float for such a source: FS-555.
- **`MRIsobel(src, grad, mag)`**: x, y, z into frames 0, 1, 2 of `grad` (without one: a float volume
  of 3 frames with the source's header; a failing filter prints its message and the others run);
  with `mag`, its frame 0 the magnitude `sqrt(z² + (x² + y²))` in single precision, through direct
  reads and writes when both are float, else `MRIgetVoxVal`/`MRIsetVoxVal`. Returns `grad`.
- **`MRIsobelFrame(src, grad, mag, frame)`**: `MRIsobel` of frame `frame` copied (`MRIcopyFrame`)
  into a new one-frame volume.
- **`MRIdivergence(src, div)`**: a new float volume with the source's header (FS-556), each voxel the
  double sum `(x + y) + z` of `MRIsobel`'s three derivatives.
- **`MRIlaplacian(src, lap)`**: without a destination, a float clone of the source's extent and
  frames (`MRIcloneDifferentType`); each voxel of each frame `-6 c` plus its six neighbours (the
  nearest voxel past an edge) in the order x - 1, x + 1, y - 1, y + 1, z - 1, z + 1, single precision,
  `c` from frame 0 (FS-554); NaN bits: `c · -6`, then alternately `sum + v` and `v + sum`.

## 14. The white-matter border classification

Labels: `MRI_AMBIGUOUS` 128, `MRI_WHITE` 255, `MRI_NOT_WHITE` 1.

**`MRImarkBorderVoxels(src, dst)`**: each voxel `MRI_WHITE` where the source is white and a voxel
of its 3 x 3 x 3 neighbourhood (the nearest voxel past an edge) is `MRI_NOT_WHITE`, `MRI_NOT_WHITE`
where it is not white and one is white, else `MRI_AMBIGUOUS`. A uchar source is read and the
destination written directly (FS-559); another type by `MRIgetVoxVal`, the voxel rounded by `nint`
and its neighbours truncated (FS-557), written by `MRIsetVoxVal`. Without `dst`, `MRIclone(src)`.
With `Gdiag & DIAG_SHOW`: `border white:  <n %8d> voxels (<p %2.2f>%)` and `border gray    <n>
voxels (<p>%)` on standard error, `p = 100 n / (w h d)` in single precision.

**`MRIborderClassifyVoxel(src, labeled, wsize, x, y, z, &pw, &pg)`**: over the window of half-width
`(wsize - 1) / 2` (the nearest voxel past an edge; the centre left out), the source's values
truncated to integers (FS-557) where the labels are `MRI_NOT_WHITE` (count `ng`, sum `gm`) or
`MRI_WHITE` (count `nw`, sum `wm`, sum of 32-bit squares `ws`), single-precision sums. `wm /= nw`,
`wvar = ws / nw - wm²` (else both 1); `gm /= ng` (else 1). With `v` the centre's truncated value,
`pw = expf(-(v - wm)² / (2 wvar))`, `pg = expf(-(v - gm)² / (2 wvar))` (FS-558: the white variance
for both), `t = pg + pw`; `pg /= t` unless `|t| < FLT_EPSILON` (a NaN `t` divides), `pw /= t`
always. Returns `MRI_NOT_WHITE` when `pg > pw`, else `MRI_WHITE`. `expf` is glibc's (GLIBC-07).

**`MRIreclassify(src, labeled, dst, wm_low, gray_hi, wsize)`**: prints `MRIreclassify(): wm_low=<g>,
gray_hi=<g>, wsize=<d>`; the border labels of `labeled` (`MRImarkBorderVoxels`); without `dst`, a
copy of the labels (written, then lost: no output but the lines). For each slice
`DiagShowPctDone(z / (d - 1), 5)`; each voxel whose truncated intensity is strictly between `wm_low`
and `gray_hi` gets the classifier's label over the border labels with `wsize`, and when the larger of
`pw`, `pg` (the second when they are equal or one is NaN) is below 0.7, the sums of the
probabilities over the windows 5, 9, 13, 17 decide (`MRI_NOT_WHITE` when the gray sum is larger);
every voxel's label is written into `dst` (`labeled` itself allowed). Then `  <n %8d> voxels tested
(<p>%)`, `  <n> voxels changed (<p>%)`, `  <n> multi-scale searches  (<p>%)`. Returns 0.

**`MRIcomputeClassStatistics(T1, labeled, gray_low, gray_hi, ...)`**: histograms of `ceil(max) + 1`
bins of 1 from 0 (`max` the volume's largest value, `MRIvalRange`; the bins then set by `HISTOinit`
with 0, 0). Over the voxels whose label (truncated) is not `MRI_AMBIGUOUS`: border-white voxels at or
above `(gray_low + gray_hi) / 2` (single precision) and border-not-white voxels within `[gray_low,
gray_hi]` (all with a negative `gray_low`) are counted, summed and squared in double, their extremes
kept (from 100000 and -100000), and counted in bin `nint(v)` (FS-560). The means, `sqrt(Σv²/n -
mean²)`, the histograms' medians (`HISTOfindMedian`) printed as the modes: `CCS WM (<%2.1f>): <mean>
+- <std> [<min> --> <max>]` and `CCS GM (<mode>) : ...` (`%2.1f`). Returned: the means and
deviations (as floats).

## 15. The connected components

**`GrassFire6(vol, labels, label, pt, ...)`** labels `pt` with `label`, then grows the region of voxels
above 0 (`MRIgetVoxVal`, frame 0) connected by faces whose label is 0, labelling each with `label`;
returns the region's size and the bounds of its voxels. **`GrassFire18`**: faces and edges.
**`RemoveHoles(vol)`**: the background (voxels at or below 0; NaN is object) connected by faces to
the background voxels of the first row of the first slice is kept; every other voxel becomes 1.
**`GetLargestCC6(vol)`**, **`GetLargestCC18`**: scanning rows, then columns, then slices, each
unlabelled voxel above 0 seeds a new region (labels from 1); the first largest is kept and every
other voxel set to 0 (all of them when there is none).

**The global Sobel tables** (read by FreeSurfer's 2-D image code): `sy` = `{-0.25, -0.5, -0.25, 0,
0, 0, 0.25, 0.5, 0.25}` and `sx` = `{-0.25, 0, 0.25, -0.5, 0, 0.5, -0.25, 0, 0.25}` (floats, row by
row: `sx` is `sy` transposed).

## 16. Validation plan (part 2)

- **Probe (black box).** `fsfilter`'s commands `g1d`, `cv1`, `cvb`, `cvs`, `cvi`, `cvg`, `gsn`, `gs`,
  `mgs`, `mean`, `std`, `stdnz`, `red`, `red2`, `r1d`, `rsl`, `xsob`, `ysob`, `zsob`, `sob`, `sobf`,
  `div`, `lap`, `mbv`, `bcv`, `rcl`, `ccs`, `rmh`, `lcc6`, `lcc18`, `gf6`, `gf18` (the README lists
  their tokens) print the result volume with its header fields (sizes, direction cosines, centre,
  start and end, thickness, pixel size, scale, `imnr0`, `imnr1`) and any given destination. The
  cases: volumes of 1 to 10 voxels a side of every type, NaN-rich float volumes (several payloads),
  kernels with NaN weights, destinations of other types, extents and frames, invalid frames and
  axes, deviations 0, negative and NaN, label volumes with odd values.
- **Expected.** Every line identical under `reproduce` but those that touch memory outside a volume
  (FS-546, FS-548, FS-549, FS-550, FS-551, FS-552, FS-555, FS-559, FS-560, FS-561, FS-163, and FS-53, FS-58);
  the default differs only where a defect it corrects is noted. Observed at seeds 7 (1,200 cases),
  13 (1,400) and 17 (2,000): 0 different (observed).

## 17. The frame functions

A *series* is a volume of several frames; "at each voxel" means for every voxel of the input's
extent, in the order stated (`c` columns, `r` rows, `s` slices; "columns outermost" visits `c`, then
`r`, then `s`). Values are read and written with `MRIgetVoxVal`/`MRIsetVoxVal` (frames outside the
volume: FS-52, FS-53; integer outputs clamped and rounded as `MRIsetVoxVal` does) unless said
*direct* (a 4-byte float written at the voxel's place whatever the output's type and extent:
FS-562). "A new output" has the input's extent and header (`MRIcopyHeader`); messages go to standard
output; "fails" means the function prints the message and returns `NULL` (a given output unchanged).
Sums are `double` and add in the stated operand order (a NaN's bits: the first operand's).

- **`fMRImatrixMultiply(in, M, out)`**: fails with `ERROR: fMRImatrixMultiply: input dimension
  mismatch` when the input's frames are not `M`'s columns, `... input is not MRI_FLOAT` for a
  non-float input; a given output of another extent or of frames other than `M`'s rows fails with
  `... output dimension mismatch`, one not float with `... output is not MRI_FLOAT`. A new output is
  float with `M`'s rows as frames. The output is cleared, then for each output frame `o` and input
  frame `i` in turn each voxel becomes `(v_i * M[o+1][i+1]) + out` in single precision (the product
  first).
- **`fMRIcovariance(f, Lag, DOFAdjust, mask, covar)`**: a negative `DOFAdjust` means "remove the
  mean" with `DOFAdjust` = 1. `Lag` outside `[0, nframes)` fails with `ERROR: fMRIcovariance: Lag out
  of bound <Lag> ` (a blank before the newline). `DOF` = `nframes - DOFAdjust` in single precision,
  truncated to `int` (FS-565); `DOFLag = DOF - Lag`; `DOFLag <= 0` fails with `ERROR: fMRIcovariance:
  DOFLag = <DOFLag> <= 0 (DOF=<DOF>, Lag=<Lag>)`. A new output is float of one frame; a given one is
  written direct. Columns outermost: a voxel whose mask value is below 0.5 (NaN is not) gets 0;
  otherwise, with `m` the voxel's `MRIframeMean` (when removing the mean) or 0, `sum = sum + (v[f+Lag]
  - m)(v[f] - m)` over `f < nframes - Lag` (the later frame's difference first), and the voxel gets
  `sum / DOFLag` as a float.
- **`fMRItemporalAR1(f, DOFAdjust, mask, ar1)`**: the covariances at lags 0 and 1 (as above, their
  messages first), then a new float output of one frame (a given one written direct); columns
  outermost, masked voxels 0, others `cov1 / cov0` in double (FS-563: a variance of 0 gives 0;
  FS-564: a covariance that failed ends the program at the first voxel not masked out).
- **`fMRInskip(in, n, out)`**, **`fMRIndrop(in, n, out)`**, **`fMRIframe(in, frame, out)`**: fail
  with `ERROR: fMRInskip: nskip >= nframes` (`fMRIndrop: ndrop >= nframes`, `fMRIframe: frame >=
  nframes`) when `nframes <= n`. The output has `nframes - n` frames (one for `fMRIframe`) of the
  input's type (`fMRIframe`'s new output also takes the pulse parameters); a given one of another
  extent or frame count fails with `ERROR: <name>: output dimension mismatch`, of another type with
  `... structure type mismatch`. It is cleared, then output frame `k` takes input frame `k + n`
  (`fMRIndrop`: `k`; `fMRIframe`: `frame`), slices outermost. A negative `n` or `frame` reads frames
  before the volume (FS-53).
- **`MRIvolMax(in, out)`**, **`MRIvolMin`**: a new output of the input's type and one frame; a given
  one of another extent fails with `ERROR: MRIvolMax: dimension mismatch` (`MRIvolMin`). Columns
  outermost, the voxel's frame 0, replaced by a later frame only when strictly larger (smaller): a NaN
  never replaces, a NaN in frame 0 stays.
- **`MRIconjunct(in, out)`**: a new float output (a given one of another extent: `ERROR: MRIconjunct:
  dimension mismatch`). Columns outermost: from frame 0 and then every frame from 0 again, the value
  of the smallest magnitude (replaced only when strictly smaller); the voxel gets `|v| * sign` with
  `sign` = 1 for `v > 0` and -1 otherwise (so 0 gives -0; a NaN of frame 0 stays, its sign cleared).
- **`MRIvolMaxIndex(in, base, mask, out)`**: a new int output (a given one of another extent fails
  with `ERROR: MRIvolMax: dimension mismatch`). Columns outermost, each voxel first set to 0; where the
  mask is below 0.5 it stays 0; otherwise the first frame of the largest value (replaced only when
  strictly larger) plus `base`.
- **`MRIframeMean(vol, out, weights)`**: weights of a count other than 0 or the frames fail with
  `ERROR: MRIframeMean(): dim mismatch: weight=<n>, vol=<nframes>`. A new float output of one frame.
  Columns outermost, `sum = sum + v_f` (times `w_f` first, `v_f * w_f`, when weights are given), the
  voxel `sum / nframes`.
- **`MRIframeMedian(vol, out)`**: the median of the voxel's frames as `specs/freesurfer-core.md`'s
  `median` gives it. **`MRIframeSum(vol, out)`**: `sum = sum + v_f`.
- **`fMRIsubSample(f, Start, Delta, Stop, sub)`**: a negative `Stop` is the frame count; `Start >
  Stop` fails with `ERROR: fMRIsubSample(): Start > Stop`, `Delta == 0` with `... Delta == 0`. The
  count `n = ceil((Stop - Start) / Delta)` in double, truncated to `int` (FS-565); a new output is a
  float clone of the input's extent (`MRIcloneBySpace`) of `n` frames (a negative `n`: the input's
  count); `n` other than the output's frames fails with `ERROR: fMRIsubSample(): frame mismatch
  (<n>, <frames>)`. Frames `Start, Start + Delta, ...` below `Stop` go to output frames 0, 1, ...
  (columns outermost); FS-569 where they never reach `Stop`.
- **`fMRIextractFrames(mri, list)`**: an empty list fails with `ERROR: fMRIextractFrames():
  framelist is empty`, a frame `f >= nframes` with `ERROR: fMRIextractFrames(): f=<f> >= <nframes>`;
  otherwise a clone of the input's type with the listed frames in order and the pulse parameters.
- **`ASLinterpMatrix(ntp)`**: a real matrix of `ceil(ntp / 2)` rows and `ntp` columns of zeros, then
  row 1: +1 at column 1, -1 at column 2; when `ntp` is odd the last row `r` (if not row 1): +1 at
  column `2r - 1`, -1 at `2r - 2`; every other row `r`: -0.5 at `2r - 2`, +1 at `2r - 1`, -0.5 at
  `2r` (FS-566 for `ntp` = 1).
- **`fMRItoMatrix(f, M)`**: a new real matrix of `nframes` rows and `nvox` columns (the voxel count)
  or the given one; voxel `n`, counted slices outermost and columns fastest, puts frame `k` at
  element `(k + 1, n + 1)` (FS-567 for a given matrix too small; a larger one keeps its other
  elements).
- **`fMRIfromMatrix(M, f, mask)`**: with a mask of another extent (`MRIdimMismatch` without frames):
  `fMRIfromMatrix(): mask dimension mismatch`, return 1. `nvox` = the mask's voxels at 0.5 or more
  (NaN counts), or every voxel. Columns not `nvox`: not a multiple of it gives `ERROR:
  fMRIfromMatrix: dim mismatch nvox=<nvox>, mat <rows> <cols>`, return 1 (FS-568 for `nvox` = 0);
  otherwise `M` is reshaped to `cols / nvox` rows of `nvox` (its elements row by row). Rows other than
  the frames, or columns other than `nvox`: `ERROR: fMRIfromMatrix: dim mismatch`, return 1. Then the
  inverse of `fMRItoMatrix` over the selected voxels (slices outermost); return 0.
- **`fMRIspatialCorMatrix(f)`**: prints `fMRIspatialCorMatrix: creating matrix`, builds
  `fMRItoMatrix`, prints `... demeaing` and subtracts from each column its mean (the double sum of
  its elements from the first row divided by the rows; each element `(double) e - mean` stored as a
  float), prints `... normalizing` and divides each column by its length (`MatrixNormalizeCol`),
  prints `... creating matrix transpose`, `... multiplying` and forms `M' M` in double
  (`MatrixMultiplyD`), prints `... allocating SCM`, makes a float volume of the input's extent with
  one frame per voxel (no header copied), prints `... filling SCM` and fills it with
  `fMRIfromMatrix`.
- **`fMRIcumSum(in, mask, out)`**: a new float output with the input's frames; a given one of another
  extent or frame count fails with `ERROR: fMRIcumSum(): output dimension mismatch`. Slices
  outermost, voxels whose mask value is below 0.5 untouched; otherwise `sum = v_f + sum` (the value
  first) written to frame `f` after each frame.
- **`MRIframeNorm(src, mask, out)`**: a new float output with the input's frames; the frame mean,
  then `fMRIcovariance(src, 0, -1, mask)`. Columns outermost, masked voxels untouched; otherwise
  `den = DBL_MIN + (double) sqrtf(var * (float)(nframes - 1))` (the product and root in single
  precision) and frame `f` gets `((double) v_f - mean) / den`. At the end the mean and covariance are
  freed: a covariance that failed (with every voxel masked out) gives the error `MRIfree: null
  pointer` (FS-564 otherwise).

## 18. Validation plan (part 3)

- **Probe (black box).** `fsfilter`'s commands `fmm`, `fcov`, `far1`, `fskip`, `fdrop`, `ffrm`,
  `vmax`, `vmin`, `conj`, `vmaxi`, `fmean`, `fmed`, `fsum`, `fsub`, `fext`, `asl`, `f2m`, `m2f`, `scm`,
  `fcum`, `fnorm` (the README lists their tokens) print the result (volume with header fields, or
  matrix: rows, columns, elements' bits) and any given destination. The cases: series of 1 to 6
  frames of every type, NaN-rich float series, masks of uchar and float (values about 0.5, NaN,
  empty), destinations of other types, extents and frames, lags and adjustments out of range (NaN,
  1e10, negative), weights of the wrong count, negative and zero steps, frames outside the series,
  matrices too small and too large.
- **Expected.** Every line identical under `reproduce` but those that touch memory outside a volume
  or crash (FS-562, FS-564, FS-567, FS-568, FS-569, and FS-53, FS-58); the default differs only where
  a defect it corrects is noted. Observed at seeds 7 (420 frame cases), 13 (2,100) and 17 (4,200): 0
  different in either mode (observed).

## 19. The volume clusters

A *cluster* holds its members' column, row and slice in the order they were added, their
coordinates (x, y, z; computed only when a transform is given: FS-570), the index of its member
of the largest value (`maxmember`), that value (`maxval`), the voxel size (`voxsize`) and three
clusterwise p-values (0 unless a caller sets them).

- **In range** (`clustValueInRange(v, thmin, thmax, thsign)`): `v` taken as `|v|` for `thsign` 0, as
  `-v` for -1, as itself otherwise; in range when `v >= thmin` and, if `thmax > 0`, `v <= thmax`
  (single precision; NaN never in range).
- **The hit map** (`clustInitHitMap`): an int volume of the input's extent (no header copied);
  columns outermost, a voxel whose mask value (frame `maskframe`) truncates to 0 (`-1 < m < 1`; NaN
  does not) gets 1; otherwise a voxel of `frame` in range gets 0 and is a *hit* (in that order),
  every other voxel 1. With no hit the map stays all 0. With `Gdiag_no > 0` it first prints
  `clustInitHitMap: `, `   thmin  = <%f>`, `   thmax  = <%f>`, `   thsign = <d>`, `   maskframe = <d>`
  (one line each) and, with no hit, `no hits in hit map`; with `Gdiag_no > 1` `INFO:
  clustInitHitMap: found <n> hits`.
- **Growing** (`clustGrow(c, r, s, map, AllowDiag, npassesmax)`): the seed is the first member and
  marked 1 in the map; the voxel size is the map's `xsize * ysize * zsize` (single precision, in
  that order). Pass after pass, each member added since the previous pass (in order) adds its
  neighbours inside the map (column offset outermost, then row, then slice, each -1, 0, +1; only
  the 6 face neighbours unless `AllowDiag`) whose map value is 0, marking each 1, until a pass adds
  none or, when `npassesmax > 0`, after `npassesmax + 1` passes.
- **The largest member** (`clustMaxMember(vc, vol, frame, thsign)`): `maxval` starts at 0; each
  member's value `v` is compared as `v` (`thsign` 1), `|v|` (0) or `-v` (-1) (another `thsign` keeps
  the previous comparison value, from 0); when `|maxval|` is below it, `maxval = v` (the signed
  value) and `maxmember` the member; `maxmember` is otherwise left as it was (0 for a new cluster).
- **Finding the clusters** (`clustGetClusters(vol, frame, thmin, thmax, thsign, minsize, mask, XFM,
  AllowDiag)`): the hit map with mask frame 0; each hit whose map value is still 0 seeds a cluster
  (no pass limit), whose voxel size becomes the volume's `xsize * ysize * zsize`, whose largest
  member is found with `thsign`, and whose coordinates are computed when `XFM` is given (each
  member's `XFM * [c, r, s, 1]'` by `MatrixMultiply`, the first three elements; FS-571 for a
  transform that is not 4 x 4). Clusters whose `nmembers * voxsize` (single precision) is below
  `minsize` are dropped. The rest are sorted by C's `qsort` (glibc's merge sort: ties keep their
  order) on: the smaller clusterwise p-value first, then more members, then the larger `|maxval|`.
  With `Gdiag_no > 0`: `INFO: Found <n> voxels in threhold range`, `INFO: Found <n> clusters that
  meet threshold criteria`, `INFO: Found <n> clusters that meet size criteria`, `INFO: Found <n>
  final clusters`.
- **Pruning by distance** (`clustPruneByDistance(list, d)`): a cluster is dropped when another
  cluster's largest member lies closer than `d` (single precision: `sqrtf(dx² + dy² + dz²)`, added in
  that order) to its own and that other cluster's `maxval` is larger; the kept ones in order, or
  `NULL` when none is kept. **By size**: those whose `nmembers * voxsize >= threshold`.
- **To a volume** (`clustClusterList2Vol(list, tvol, frame, ValOption)`): a volume of `tvol`'s
  extent, type and header, with `tvol`'s frames when `ValOption` is not 0, else one frame; zero
  except at the members: with `ValOption` 1 their values of every frame, otherwise the cluster's
  number (from 1) in frame 0 (stored as the type stores it: clamped).
- **`MRIremoveVolumeIslands(mask, thresh, nKeep, out)`**: `nKeep` 0 fails with `ERROR:
  MRIremoveVolumeIslands(): nKeep==0`; a new output is of the mask's type and one frame (header and
  pulse parameters); a given one of another extent fails with `ERROR: MRIremoveVolumeIslands():
  dimension mismatch`. The clusters of frame 0 whose magnitude is at least `thresh` (as a float),
  faces only, no size limit; their numbers in a volume of the mask's type; prints
  `MRIremoveVolumeIslands() thresh=<%g>, nKeep=<d>, nclusters = <n>`; each voxel whose number is at
  most `nKeep` (0 outside clusters) keeps the mask's value, every other voxel 0. The mask may be its
  own output.
- **`MRIremoveVolumeHoles(mask, thresh, nKeep, FillVal, out)`**: prints `MRIremoveVolumeHoles()
  thresh=<%g>, nKeep=<d>` first; the same errors (`MRIremoveVolumeHoles()`); the voxels at or below
  `thresh` (NaN too) as 1 in an int volume, the others 0; its islands removed as above (at 0.5, with
  its own line); where that leaves 0 the voxel keeps the mask's value when above `thresh`, else
  `FillVal` (as a float); elsewhere 0.

## 20. The non-parametric white matter and cortex classification

White matter labels: 2, 41, 251-255; cortex: 3, 42. A class mask is `MRImatchSegs` with the value 1.
Class statistics are `MRIsegStats` of `norm` (frame 0) over a mask; printed `wmstats  n=<%6d>,
mean=<%g>, std=<%g>, min=<%g>, max=<%g>` and `ctxstats n=...`; a class with no voxel fails with
`ERROR: could not find any WM voxels` (`CTX voxels`). The histogram limits are the smaller of the
two minima and the smaller of the two maxima (single-precision comparisons; FS-572), printed
`hbminmin = <%g>, hbinmax = <%g>`; each class's histogram is `HISTOseg` with bins of 1 over them,
divided by its sum (`HISTOsumNorm`). The *crossing* is the first bin of the smallest `|(1 -
ctxCDF) - wmCDF|` (CDFs: running double sums of the PDFs stored as floats), the threshold that
bin's value.

- **`MRIreclassifyWMCtxNonPar(norm, seg, nitersmax, newseg)`**: prints `MRIreclassifyWMCtxNonPar():
  nitersmax = <d> ` (a blank before the newline); `newseg` being `seg` fails with `ERROR: cannot be
  done in-place`. Statistics and threshold from `seg` (`thresh = <%g>`). Then up to `nitersmax`
  passes: `newseg` becomes a copy of the current segmentation (`seg` in the first pass, `newseg`
  itself later); the class masks of the current segmentation; columns outermost, every class voxel
  with a face neighbour of the other class (neighbours read with `MRIgetVoxVal`; FS-52, FS-53 at the
  edges) is a border voxel; a border voxel whose `norm` value is above the threshold and whose label
  is 3 (42) becomes 2 (41), one at or below it labelled 2 (41) becomes 3 (42). Each pass prints
  `iter <%2d>/<%2d> nborder = <n>, nrecode = <n>`; a pass that recodes nothing ends them. Returns
  `newseg` (`NULL` when no pass ran and none was given).
- **`MRIclassifyAmbiguousNonPar(norm, seg, statseg, Qwm, Qctx, NdilWM, NdilCtx)`**: prints
  `MRIclassifyAmbiguousNonPar(): Qwm=<%g>, Qctx=<%g>, NdilWM=<d>, NdilCtx=<d> `; statistics and
  histograms from `statseg`; writes the two PDFs as text (`HISTOwriteTxt`) to `wm.pdf.dat` and
  `ctx.pdf.dat` in the current directory. `thresh` the crossing; `thresh_low`: from the crossing
  down, the first bin where `ctx / (wm + 1e-6) >= Qctx` (double); `thresh_hi`: from the first bin
  up, the first where `wm / (ctx + 1e-6) >= Qwm` (FS-572); FS-573 where none; printed `thresh =
  (<%g>,<%g>,<%g>)`. The class masks of `seg`, each dilated (`MRIdilate`; FS-574, FS-575). A new uchar
  volume of `seg`'s extent and header: 255 where `seg` is white matter, else 1; then 128 where a
  dilated mask holds the voxel and `thresh_low < norm < thresh_hi`. Prints `nhits = <n>, nrecode =
  <n>` (voxels in a dilated mask, voxels set to 128).

## 21. Validation plan (part 4)

- **Probe (black box).** `fsfilter`'s commands `clus` (`clustGetClusters` with every parameter,
  `Gdiag_no` 0 to 2, then `clustClusterList2Vol`), `cprd` (pruning by distance), `cgrow` (growing
  with a pass limit, then the hit map), `isl`, `hol` (new, given and in-place outputs), `cvr`
  (`clustValueInRange` of 8 values), `rwc`, `amb` (with the FNV-1a of the two text files). The
  cases: blob volumes of every type with NaNs, masks with values about the truncation's edges, 4 x 4
  transforms, segmentations with a background margin or labels on the faces, intensities by class
  (integer and fractional), one class missing, dilation counts 0 to 3.
- **Expected.** Every line identical under `reproduce` but those that touch memory outside a volume
  or a histogram, or crash (FS-53, FS-570, FS-571, FS-573, FS-575); the default identical but where a
  noted defect it corrects changed the result. Observed: clusters at seeds 7 (360 cases) and 13
  (2,400); classification at seeds 7 (120), 13 and 17 (1,200 each): 0 different in either mode
  (observed).
