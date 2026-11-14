# FreeSurfer 8.2.0's volume filters (`fs.filter`): findings

**Closed to FreeSurfer clean-room implementers** (it names FreeSurfer's files, functions and lines;
the behaviour is `specs/freesurfer-filter.md`). Queue item 3.1-42, replica in
`crates-freesurfer/larmorx-freesurfer-filter` (track D, relays 25 and 26, 2026-11-14).

## 1. The fast-marching distance maps (`utils/mri_fastmarching.cpp`, `include/fastmarching.h`)

`MRIextractDistanceMap` builds `FastMarching<+1>` and `FastMarching<-1>` objects (each a local of
its own `if` block, destroyed before the next is built). The class keeps the trial points in
`std::priority_queue<stCoord, std::vector<stCoord>, HeapCompare>`, whose comparison reads
`MRIFvox(mri, ...)` at call time. `_UpdatePoint` rewrites the value of a point already in the queue
(`st == eTrial`) without `push`/`pop`, so the vector stops being a heap; the order of `pop`s is then
libstdc++'s `__adjust_heap`/`__push_heap` on that vector. `larmorx-freesurfer-filter/src/fastmarch.rs`
translates `push_heap` (`__push_heap` with `topIndex` 0), `pop_heap` (`__pop_heap` then
`__adjust_heap`) and `pop_back` step for step (libstdc++ of GCC 8.4.0, `bits/stl_heap.h`, the
instantiations `std::__push_heap<..., _Iter_comp_val<FastMarching<1>::HeapCompare>>` and
`std::__adjust_heap<..., _Iter_comp_iter<...>>` in `mri_fastmarching.cpp.o`). `alive_list` (a
`std::list`) only feeds `InitTrialFromAlive`; the points `Run` appends are never read again (the
port does not keep them).

When the march reaches the limit, `Run` empties the queue by `pop`s, setting each top far at the
limit: the order is immaterial (every queued point ends far at the limit), so the port sets them in
array order.

## 2. `_UpdateValue`'s machine code (unit oracle's `mri_fastmarching.cpp.o`, GCC 8.4.0 `-O3`)

Single precision throughout (`sqrtss`: `::sqrt` resolves to the float overload through libstdc++'s
`<math.h>`). `sign * x` is folded: no operation for `+1`, `xorps` with the sign mask for `-1` (a NaN's
sign flips, no quieting). `MIN(sign*g(x+1), sign*g(x-1))` for `-1` is compiled as `g(x+1) >
g(x-1) ? -g(x+1) : -g(x-1)` (the same for every input). Operand orders (destination first):
`A + 1`; for `<1>` `B*B + A*A`, for `<-1>` `A*A + B*B`; `C*C + (that)`; `- 1`; `* 3` (`a*c` with
`a = 3`); `S*S - ...` where `S = (A + B) + C` (GCC squares `S`, not `b = -S`: a NaN's sign is not
flipped there); `sqrt(D) + S`; `/ 3`; `EPS + sol` (`1e-6f` first); the second trinome `(A+B)*(A+B) -
(c + c)` (`a*c` with `a = 2` as `c + c`), `(A + B) + sqrt(E)`, `* 0.5` (`/ a` with `a = 2`), `EPS + sol`;
for `<1>` its last test is a branch-free `cmpnless`/`andps` select of the same comparison. A
negative `delta` also calls `sqrtf` for `errno` (no effect on the value). The NaN orders matter only
with two NaN operands of different bits, which the maps produce only from a NaN limit; the probe's
NaN-limit cases agree.

## 3. Reads past an axis of one voxel (FS-539, FS-540)

`_UpdateValue` reads `x + 1` when `x == 0` before testing `x == width - 1`, so on an axis of one voxel
it reads index 1. Along x, `MRIvox(status, 1, y, z)` and `MRIFvox(mri, 1, y, z)` are the next row's
first element of the same chunk (`MRI::initSlices`: rows consecutive in one `calloc`), except after
the chunk's last voxel (FS-540: past the `calloc`). Along y or z, `slices[z][1]` or `slices[1]` reads
past the `calloc`'d pointer array of that slice (or of the slices): a pointer from heap memory,
usually NULL (a crash) or a stale one (FS-540; the probe's outputs on these cases are crashes, with a
few that happen to agree). The status volume is one frame, so its read decides first.

## 4. The uninitialised limit (FS-541)

The constructor masks voxels with `MRIFvox(mri, x, y, z) = limit` before `SetLimit` (the member is
never initialised). For `<+1>` `InitFromMRI` rewrites the non-label voxels with the limit; the masked
label voxels keep the stale value until the destructor; for `<-1>` the masked non-label voxels do.
A trial point is never a 6-neighbour of a label voxel (`<+1>`) or of a non-label voxel (`<-1>`): those
are alive or forbidden, so the stale values are read only through FS-539's next-row read; the
destructor sets every masked voxel to the limit. No FreeSurfer 8.2.0 caller passes a mask (every
`MRIdistanceTransform` and `MRIextractDistanceMap` call in the tree passes `NULL`).

## 5. `MRIdistanceTransform` (`utils/mri.cpp` 15492-15540)

The mode mapping is an `if`/`else if` chain for `DTRANS_MODE_SIGNED`, `_UNSIGNED` and `_OUTSIDE`; the
comment beside the unused `inside = 2` shows `DTRANS_MODE_INSIDE` (4) was left to pass through, where
`MRIextractDistanceMap` reads it as `bothUnsigned` (FS-543, rule A: the input makes sense, the
result is not the inside map; no caller in the tree uses it). `mris_distance_to_label -mode <n>`
(`atoi`) reaches `MRIextractDistanceMap` with any value (FS-544, rule B). `MRIchangeType(src,
MRI_FLOAT, 0, 1, 1)` never returns `NULL` for a float destination (every source type is copied by
value; the types it would rescale print the forcing warning and copy).

## 6. The recorded heads

`validation/probes/fs-filter/real.py`: mri_fill's (`aseg.mgz`, label 2, 100, signed), the surface
placement's (`aseg.mgz` label 3; `filled.mgz` 255; signed, 100), mriset's (`wm.mgz` 110, unsigned,
default limit) and mris_distance_to_label's (`aseg.mgz` 17 outside 5, 4 inside 10) maps on three
heads: 18 maps, all 18 identical in both modes (none reaches a defect); 122 s for the 18 in the
official code, 94 s in the port (release build, one thread, reading the volumes included).

## 7. `mrifilter.cpp`'s convolutions, statistics and derivatives (relay 26)

- **`FLT_EPSILON`.** `mrifilter.cpp` (and `fmriutils.cpp`, `mrinorm.cpp`, `mriflood.cpp`,
  `mrisegment.cpp`, `rfa.cpp`, `volcluster.cpp`) see `<float.h>`'s `FLT_EPSILON` (1.19e-7): the
  unit oracle's preprocessed sources (`g++ -E -dM` with the probe's flags). `connectcomp.cpp`,
  `filter.cpp` and `diffuse.cpp` get `macros.h`'s 1e-5 (no `<float.h>` before it).
- **Operand orders** (the unit oracle's `mrifilter.cpp.o`, GCC 8.4.0 `-O3`; they decide only which
  of two NaNs survives): every convolution tap is `mulss k, v` then `addss p, total` (the voxel
  times the weight, the product added to the sum) in all ten OpenMP bodies of `MRIconvolve1d` and in
  `MRIconvolve1dByte`, `Short`, `Int`, `MRIreduce1d`, `MRIreduceSlice`; `MRImean`'s
  `val += MRIgetVoxVal(...)` is `addss sum, v` (the returned voxel is the destination); `MRIstd` and
  `MRIstdNonzero` add the square into the sum's register the same way. `MRIlaplacian` alternates:
  its first, third and fifth neighbours are added into the sum, the second, fourth and sixth take the
  sum into the neighbour (the spilled sum is the memory operand). `MRIxSobelForAllTypes` computes
  `2m` as `m + m` and its three column sums with three orders (`(2m + t) + b` before the loop,
  `(t + 2m) + b` for the second, `b + (2m + t)` in the loop), and its `/ eight` as `* 0.125f`;
  `MRIySobelForAllTypes` and `MRIzSobelForAllTypes` as `(2m + right) + left` (GCC commutes, never
  reassociates). `MRIsobel`'s magnitude is `z² + (x² + y²)`. `MRIdivergence` adds in double
  `(dx + dy) + dz`. `MRIgaussian1d`'s products are commuted but its operands cannot both be NaN.
- **`MRIborderClassifyVoxel`** calls `expf` (C++'s float overload); the gray class's sum of squares
  and variance are never used (FS-558: the gray likelihood uses the white variance) and the official
  build drops their computation. `MRIreclassify`'s `MAX(pw, pg)` is branchless (`cmpless`, `andps`,
  `andnps`, `orps`): the second argument for equal or NaN values, as the macro.
- **FS-558 (rule D).** Using the gray variance changes `mri_segment`'s `wm.seg.mgz` wherever the two
  variances differ; whether that improves the white-matter segmentation is to be measured at
  `mri_segment`'s port (the recorded heads' `wm.seg.mgz` against the final `wm.mgz` edits and the
  surfaces), so both modes keep FreeSurfer's computation until then (`fixed_by_default` false).
- **`MRIconvolveGaussian`'s progress range** divides by the frame count before anything else
  (`(nstart - nend) / nframes`, integer): a volume without frames crashes (SIGFPE), FS-550.
- **`MRIreduce` of one slice**: `MRIreduce1d(mtmp1, mri_dst, kernel, KERNEL_SIZE, MRI_HEIGHT)` with
  `mri_dst` NULL allocates `sdepth / 2 = 0` slices (FS-553); the following `MRIsetResolution` and
  matrix scaling already treat depth 1 as a 2-D case, so the intent is a slice.
- **The typed convolutions** read through the code's pointer type: `MRIconvolve1dByte`'s float
  branch keeps a `BUFTYPE *` (`inBase_f`), so a float source's row is read byte by byte (its first
  `width` bytes); `MRIconvolve1dShort`/`Int` cast every source's rows (FS-547).
- **`MRIlaplacian`**'s centre is `MRIgetVoxVal(mri_src, x, y, z, 0)` in a loop over frames `f`
  (FS-554). **`MRIdivergence`** reassigns its parameter with `MRIalloc` at once (FS-556).
- **`MRIgaussianSmoothNI`'s kernel sum** is `long double` with an OpenMP `reduction(+)`: with several
  threads the partial sums are combined in the runtime's order (the source's comment says the long
  double was chosen to make thread counts agree in practice). The port adds in one thread's order.
  Its `%Lg` diagnostics (only with `Gdiag_no > 0`) are formatted from the sum rounded to double.

## 8. `connectcomp.cpp` (relay 26)

`GrassFire6`/`18`'s `myQueue` is a FIFO over a realloc'd array; the region, its size and bounds do
not depend on the order, so the port uses a `VecDeque`. With a label stored as 0 (a uchar label
volume and a label of -1, or 0 itself) `MRIsetVoxVal` marks nothing and two object voxels push each
other for ever (FS-561: the probe's official run ends by its 20-second alarm, the port's first try
grew past its 12 GiB box). `GetLargestCC6`/`18` scan `i` (rows) outermost, then `j` (columns), then
`k` (slices); ties keep the first region.

## 9. Waiting (relay 26)

`MRIreclassifyWMCtxNonPar` and `MRIclassifyAmbiguousNonPar` (`mrifilter.cpp` 4914, 5093) need
`mri2.cpp`'s `MRImatchSegs` (5687), `MRIsegStats` (3826), `HISTOseg` (5717) and `mriset.cpp`'s
`MRIdilate` (1723): track C ported them (its relay 22, merged at 01910d99); relay 27 ported the two
classifiers (§11).
`filter.cpp`'s image reductions (`ImageConvolve1d` 713, `ImageReduce` 928, `ImageReduce1d` 979) were
ported with fs.io's images (`larmorx-freesurfer-io/src/image.rs`: `ImageScaleDown` calls them and
fs.filter's crate sits above fs.io); nothing of them is left here.

## 10. `fmriutils.cpp`'s frame functions (relay 27)

Operand orders from the unit oracle's `fmriutils.cpp.o` (`relay27/fmriutils.asm`, GCC 8.4.0 `-O3`,
not vectorised: the loops call `MRIgetVoxVal`, or alias their row pointers):

- `fMRImatrixMultiply`: `movss pin; mulss val; addss (pout)`: `add(mul(v, w), out)`.
- `fMRIcovariance`: `v1` from the first call, `v2` from the second; `subsd m` from each; `mulsd
  %xmm1(v1 - m), %xmm0(v2 - m)`; `addsd %xmm0, %xmm2(sum)`: `sum + (v2 - m)(v1 - m)`; `divsd
  (double) DOFLag`. `DOF` is `cvtsi2ss nframes; subss DOFAdjust; cvttss2si` (single precision,
  x86-64's `INT_MIN` for NaN and out-of-range: FS-565; `INT_MIN - Lag` then wraps for `Lag >= 1`).
- `MRIframeMean`: `mulsd (w), v` (the value first), `addsd v, sum` (`sum + v`), `divsd nframes`;
  `MRIframeSum` likewise; `fMRIcumSum`: `addsd (sum), v` (`v + sum`, the other order).
- `MRIvolMax`/`MRIvolMin`: `maxsd`/`minsd` with the new value as the destination, the same as C's
  `if (max < v) max = v` for NaNs (a NaN in either keeps the running value).
- `MRIconjunct`: `fabs` is a call (`-fno-builtin`); `SIGN` is `surfcluster.h`'s / `mrisurf_base.h`'s
  `((x) > 0 ? 1.0 : -1.0)`, compared in single precision; the result `mulsd sign` after `min` (`min *
  sign`). A voxel of 0 gives -0.0 (kept: harmless, rule C; no defect ID).
- `MRIframeNorm`: `var * (float)(nframes - 1)` in single precision (`mulss`), `sqrtss` (with the
  library call for a negative argument, the same NaN), `DBL_MIN + sss` (`addsd` with `DBL_MIN` in the
  destination), `(v - mean) / den`.
- `fMRItemporalAR1`: the `voxvar == 0` test (`ucomiss`) stores 0 and then the quotient over it (no
  `continue`: FS-563, rule A).
- `matrix.cpp`'s `MatrixDemean(M, Mdm)`: `vsum + e`, `vsum / rows`, `(double) e - vmean`; it demeans
  `M` itself and returns `Mdm` (unchanged when it is another matrix; only the in-place call
  `MatrixDemean(M, M)` of `fMRIspatialCorMatrix` is reached: ported as `fmri::matrix_demean_in_place`,
  for track A's fs.core).
- `MRIframeNorm` frees the covariance at its end: with one frame and every voxel masked out the
  covariance is `NULL` and `MRIfree` prints its error (`MRIfree: null pointer`), seen on the probe
  (seed 17).
- `fMRIsubSample` with a negative step: `ceil` of a negative quotient gives a count of 0 or less, a
  negative count clones the input's frames (a mismatch), so only a count of 0 (`Stop - Start` below
  `|Delta|`) or a caller's volume of 0 frames reaches the endless loop (FS-569).
- `ASLinterpMatrix(1)`: element (1, 2) of a 1 x 1 matrix is the float after its number, inside the
  64-byte-rounded block of the new allocator (FS-566: nothing visible).

## 11. `volcluster.cpp` and the non-parametric classification (relay 27)

- `clustInitHitMap`'s `int maskval = MRIgetVoxVal(...)` truncates (`cvttss2si`): a mask value of 0.5
  is outside the mask, NaN and values beyond the `int` range (x86-64's `INT_MIN`) inside. The port
  tests `-1 < m < 1`, the same set, so no defect is needed.
- `clustAddMember` reallocs `x`, `y`, `z` without writing the new element; `clustCopyCluster` copies
  them; only `clustComputeXYZ`/`clustComputeTal` set them (FS-570). `clustGetClusters` never calls
  `clustPruneByDistance` (its `distthresh` is 0), so the reached programs never read them.
- `clustFreeClusterList` frees `(*list)[0]` (already freed and set to `NULL` by `clustFreeCluster`:
  `free(NULL)`) and never the list itself: a leak, no output.
- `clustGetClusters` sorts the size-pruned list with `qsort` of `clustCompareCluster`; the clusters'
  p-values are 0 (`calloc`), so the order is by size then `|maxval|`, ties in discovery order
  (glibc's merge sort: `larmorx_core::glibc_qsort::qsort` on indices).
- `VolClustFixMNI` is set only by `mri_volcluster` (not reached): `clustComputeTal` is
  `clustComputeXYZ` in every reached program (`FixMNITal` not ported).
- `MRIremoveVolumeIslands`' documentation says "binarizes mask > thresh"; the code keeps `|v| >=
  thresh` (`clustValueInRange` with `thsign` 0). Kept (rule C: `mri_binarize`'s `--remove-islands`
  relies on it); the spec states the code's behaviour.
- `MRIreclassifyWMCtxNonPar`/`MRIclassifyAmbiguousNonPar` (`mri_segment.cpp` 311-315, its `-aseg`
  path): the crossing search stores the signed `d` as its minimum (`min = d`, not `fabs(d)`). The
  difference `(1 - ctxCDF) - wmCDF` never increases along the bins (both CDFs are running sums of
  non-negative counts), so the bin found is the argmin of `|d|` either way (proved by cases: before
  the sign change both versions follow the decreasing positive values; at it the first negative
  value is taken by both or by neither; after it `|d|` only grows): no defect ID, no notice.
- `hbinmax = MIN(wmmax, ctxmax)` (the brighter white matter falls into the last bin, clamped by
  `HISTOseg`) and the upper-threshold loop starting at bin 0 against its comment ("from this
  minimum, look ahead"): FS-572, rule D (measured at `mri_segment`'s port).
- `for (n = 0; n < NdilWM; n++) wmmaskdil = MRIdilate(wmmask, wmmaskdil)`: each dilation reads the
  undilated mask (FS-574, rule D: `mri_segment`'s defaults are 2 and 2, so the documented "within
  NdilWM voxels" is one voxel in every recorded run). `NdilWM <= 0` leaves `wmmaskdil` `NULL`
  (FS-575).
- `MRIclassifyAmbiguousNonPar` writes `wm.pdf.dat` and `ctx.pdf.dat` (`HISTOwriteTxt`) into the
  program's current directory on every call: `mri_segment -aseg` leaves them in `mri/`. The port
  writes them where its caller says (`dir`; the program passes its current directory).
- Operand orders (`relay26/mrifilter.asm`): `MIN` as `minss` (`a < b ? a : b`), `d = (1.0 - ctx) -
  wm` (`subsd` twice), the ratio `(double) a / ((double) b + 1e-6)`; the reclassification compares
  `val > thresh` in single precision (`thresh` is a bin value, a float).

## 12. `mrisegment.cpp`, `mriflood.cpp` and `mrinorm.cpp`'s Voronoi diagram and soap bubble (relay 28)

- **The segment table's growth is safe.** `mriSegmentReallocateSegments` copies only the first
  `nsegments` slots, which would lose a live segment numbered at or above the count; but
  `mriSegmentNew` calls it only when the count equals the table's length, and the scan keeps the
  count equal to the number of live segments (a new segment adds one, a merge removes one), so
  every slot is live then. No defect (FS-576 unused).
- **FS-577.** `mri_labeled` is `MRI_SHORT`: `MRISvox(mri_labeled, x, y, z) = label` stores the low 16
  bits; a label of 32,768 or more reads back negative, so a later neighbour does not see it. Reached
  by a 64 x 64 x 17 checkerboard (34,816 single-voxel segments) under a slab: the slab joins only
  the singletons numbered below 32,768.
- **FS-578.** `MRImaxsegment` calls `MRIremoveSmallSegments` (which compacts, renumbering the kept
  segments) inside its scan and then only clears labels `>= nsegments`; the voxels of a kept
  segment that moved keep its old number, which is either cleared (its next voxels start a new
  segment) or another segment's. Reached when the table first grows past 8,800 slots (at 9,680
  segments) with a segment of at least 100 voxels: a pillar of 8 columns beside a 64 x 64 x 6
  checkerboard. The default rebuilds the map from the segments.
- **`MRImaxsegment`'s heap usage** is `getMemoryUsed()` (a `popen` of `grep -i vmdata
  /proc/<pid>/status | cut -f 2`): the port takes it as a function (the program passes
  `get_memory_used`, the probe a constant 1000, and `check.py` replaces the official number).
- **Speed without changing results.** `mriSegmentNew` searches the first empty slot from 0 and
  `MRIcompactSegments` searches the next live slot from `s + 1` for each empty slot: quadratic in
  the number of segments (17 s in a debug build for 35,000). The port searches from the first slot
  that may be empty (a merge lowers it) and resumes the compaction's search: the same slots.
- **`MRISfloodoutside`** reads `(int)MRIgetVoxVal` (`cvttss2si`); a voxel set to 1 reads back 1 in
  every type, so the port keeps the `int` values in an array and writes each filled voxel.
- **`distance`** compiles to `mulss` of each coordinate, `addss` of y² to x², then of z², and
  `sqrtss` (a call to `sqrtf` only below 0).
- **The Voronoi diagram, 16-bit:** `if (mri_src->type != MRI_SHORT || mri_dst->type != MRI_SHORT)`
  runs before `if (!mri_dst) mri_dst = MRIclone(...)`: a `NULL` destination crashes (FS-583, `CRASH
  11` in the probe). Its marks are read with `MRIvox` (bytes) of the work copy of the control volume,
  so an `int` control volume's bytes are read (FS-585). The float version reads them with
  `MRIgetVoxVal` into a `BUFTYPE` (`cvttss2si`, low byte), the byte version into an `int`.
- **The soap bubble, float:** the first phase's `fabs` is `std::fabs(float)` (`andps`); the passes'
  `fabs(mean / (3 * 3 * 3.0) - val)` is a double (a call to `fabs` under `-fno-builtin`). The work
  copy is made from the destination before the first phase, and each pass copies all of it back,
  so the first phase's values outside the pass's box are replaced by the work copy's (the source's)
  at the first pass: only the values inside the box survive (kept: FreeSurfer's behaviour, not a
  defect we can show changes a pipeline result). `#pragma omp parallel for` over the box's slices
  shares `max_change` without synchronisation (FS-584; the probe runs one thread). The box and
  `max_change` are not reset between frames, and `max_change` is not reset after the pass that
  stops a frame.
- **The soap bubble, 16-bit and other types:** `MRIvox(mri_ctrl, ...)` and `MRIseq_vox(mri_dst,
  ...)` are byte accesses whatever the volume's type (FS-585); within each row's first `width`
  bytes they stay inside the volume.

## 13. `mrinorm.cpp`'s normalisation without histograms (relay 29)

- **The file's state.** `num_control_points`, `xctrl`, `yctrl`, `zctrl` (set by
  `MRI3dUseFileControlPoints` and `MRI3dUseLabelControlPoints`), `control_volume_fname`,
  `bias_volume_fname` and the function statics (`MRInormAddFileControlPoints`' `first`, the
  diagnostic `pass` counters, `remove_gray_matter_control_points`' `ncalls`) are the port's
  `NormState`, one per program run (the probe runs each case in a fresh process: a fresh state).
- **FS-587 / FS-588.** Every control-volume access is `MRIvox` (a byte at `slices[z][y][x]`): a
  control volume of another type is read and written in its first bytes of each row; a smaller one
  outside it. The 16-bit spline normalisation writes `*pdst++ = (short)dval` through
  `&MRISvox(mri_dst, 0, y, z)`, whatever the destination's type. The port's direct accesses go
  through `mem.rs` (`Wrapped` as FreeSurfer with `reproduce`; by value by default; `Outside` reads 0
  and writes nowhere).
- **FS-589.** `whalf = ceil(WHALF / xsize)` (`ceilf`, `cvttss2si`), `nint(whalf_mm / xsize)`,
  `nint(max_dist / xsize)`: a voxel size of 0 gives `INT_MIN`, and `-whalf` wraps to `INT_MIN`, so
  the loop reads `xi[x + INT_MIN]`; above 500 the index tables (`MAX_INDEX` 500 each side) are left.
- **FS-590.** `MRInormCheckPeaks` reads `outputs[above - 7]`, `[above - 4]`, `[above - 1]` and
  starts there whatever `npeaks` is; `MRInormalize`'s arrays `inputs[80]`, `outputs[80]` are
  uninitialised locals, so in `mri_normalize` the entries past the peaks found are stack contents
  (FS-596 when the port meets its never-written marker, `NEVER_WRITTEN`). The backward pass reads
  `outputs[old_slice]` and `inputs[slice + 1]` from the start, so a start past the peaks decides
  which real peaks are dropped. Reached by the probe's `npk` (14 windows above, 9 peaks).
- **FS-591.** `MNI` holds `histograms[80]` and `regions[80]`; `MRInormInit` writes
  `regions[i]` for `i < above + below` unchecked; `MRInormCheckPeaks` reads `outputs[above - 7]`.
- **FS-592.** `BUFTYPE low_thresh, hi_thresh` in `MRInormGentlyFindControlPoints`: the object code
  stores `cvttsd2si` (7x7x7: double `wm - 1.5 below`, `1.5 above + wm`) and `cvttss2si` (5x5x5:
  float) results and compares `movzbl` of them with the truncated voxel values (`int`). With
  `mri_normalize`'s values (110, 25 or 12.5 above, 5 below) the thresholds stay inside a byte.
- **FS-594.** The removal loop `for (i ...) if (deleted[i]) { shift; count--; }` advances `i` past the
  element shifted into `i`.
- **FS-595 / FS-596.** `ErrorPrintf(ERROR_BADFILE, "!!!!! %ld control points rejected ...\n")` passes
  no number: `%ld` reads the first variadic register (`rdx`). In the official object code `rdx`
  holds, when the last point was skipped for its value, that value as `cvttss2si`'s 32 bits
  zero-extended (the `push rax` / `pop rdx` around the `printf`); after any other last point it is
  what `MRIindexNotInVolume`, `MRIgetVoxVal` or `printf` left (not reproduced: the port prints the
  count with FS-596; the probe masks the number then).
- **Operand orders** (object code): `MRIapplyBiasCorrectionSameGeometry` computes
  `divsd` of the target by the bias, then `mulsd` with the value as the second operand (the ratio's
  NaN wins); `MRI3dGentleNormalize`'s `(float)src * wm / (float)bias` in single precision, left to
  right; `MRIsplineNormalize`'s `(float)val * frac` (`mulss`, the value first) then `+ randomNumber`
  in double.
- **`MRIbuildBiasImage`** prints `performing soap bubble smoothing` but smooths with
  `MRIconvolveGaussian`; it ignores its `mri_bias` argument (a new clone each time).
- **`mri_normalize`'s `-g 1`** sets `mni.max_gradient` (`MRInormCheckPeaks`), not the gentle flag
  (`-gentle`).
- **Speed (release builds, one 256³ head):** the port's gentle normalisation takes 18 s (the
  search 1 s, the Voronoi diagram 5 s, the 65-tap Gaussian 11 s) where the official code averages
  15 s per volume including the read: the convolution (§10's `MRIconvolve1d`) is the part to speed
  up, without changing a bit.

## 14. `mrinorm.cpp`'s histogram steps (relay 30)

Closed (object code of the unit oracle's `mrinorm.cpp.o`, `relay27/mrinorm.asm`; FreeSurfer's source).
- **Operand orders.** `MRI3dNormalize`'s `src * (float)wm_target / (float)bias` is compiled as
  `wm_target * src` (the target in the destination register), then `/ bias`; `FZERO(bias)` with
  float.h's `FLT_EPSILON` (`|bias| < eps` via `andps` and `comiss`). In `MRInormFindControlPoints`
  `1.5 * intensity_above` is `above * 1.5` (`double`), the second search's `ceil(1.5 above + 5 pass)`
  is `5 pass + 1.5 above` and `ceil(1.5 below + 5 pass)` `1.5 below + 5 pass` (the first search's
  pass 0 is `0.0 + 1.5 below`, the same value); `(wm_val - gm_val) / 4` is `0.25 * (wm - gm)`;
  `hi_thresh = wm_target + intensity_above` is `above + wm_target`; `ceil(WHALF / xsize)` is the
  inlined `float` rounding. `remove_gray_matter_control_points` (inlined into `MRI3dNormalize`):
  `(val - val0)^2` in `float`, then added to the `double` sum (the square first); `(wm + gm) / 2` as
  `* 0.5`; the `sqrt(delta_std / n)` call is made and its result discarded (its argument is never
  negative, so no `errno`). `find_tissue_intensities`: `wm_val * .75` in `float` (exact), `gm_peak *
  .5` in `double` then `float`; `0.8 wm_peak`, `0.75 gm_peak`, `0.7 wm_peak` in `double`, truncated.
  `MAX` is macros.h's (`a >= b ? a : b`): a NaN keeps the second operand.
- **Rule C, kept and documented (no ID):** both searches' loops compare the count of the *last*
  pass with 10 (`pnctrl` is set, not added): a slow trickle of control points needs further passes;
  `remove_gray_matter_control_points` computes a white matter standard deviation from the inner
  control points and then overwrites it with `(wm - gm) / 10` (the DOF count is still printed);
  `MRInormFindControlPoints`' first two counts are overwritten unused; `scan_type` is passed down
  and never used. The `min_val`/`max_val` of the growth loops are never read.
- **FS-597.** `find_tissue_intensities` indexes `hwm->bins[wm_peak]` (`cltq`: -1 when the labelled
  histogram has no count, unreachable once a voxel is labelled), `hsmooth->bins[...]` with the
  white matter peak (-1 when the 40-bin region has no count), `gm_peak` (`HISTOfindBin` returns
  `(int)val - bins[0]` for a bin size of 1: negative or past the end), `gm_valley`
  (`HISTOfindPreviousValley` returns its start when it is 0 or below: very negative starts),
  `csf_peak` and the threshold bin. `bins` is its own `calloc` block: index -1 reads the upper half
  of glibc's chunk size field (0 for blocks under 4 GiB), other negative indices the chunk header or
  the block before, past the end the next chunk: memory contents. A start of -2^31 (`(int)` of a
  value below the range: `cvttss2si`) makes `HISTOfindPreviousPeak`'s `b0 - 1` wrap to 2^31 - 1 and
  read `counts[2^31 - 1]`: a crash. The probe counts these cases (`SKIP`).
- **FS-598.** The second search's loop has no bound. The first search must have succeeded; its
  window's low end is the second's (`floor(wm - x)` against `floor(wm - ceil(x))`, equal for a whole
  `wm_target`), its high end lower: every voxel of the first search's last pass qualifies in the
  second by the same pass, so the second ends unless the rounding of a target with a fraction
  moves part of that batch to an earlier pass. The port checks, after each pass that found fewer
  than 10, whether the widest window (the last pass whose `5 pass` fits an `int`; after it
  FreeSurfer's `5 pass` wraps and its windows only shrink) can still give 10 (a copy of the control
  volume, no message): if not, an error. Not reached by the probe's cases.
- **FS-599.** `pass++; if (pass > 10) ErrorReturn(...)` precedes the loop's `while (nctrl < 10)`:
  the eleventh pass's result is never looked at (rule A: kept by default).
- **FS-600** (split from FS-596): `MRInormalize`'s `inputs`/`outputs` are stack arrays never
  initialised; the peak check (FS-590) reads past the peaks found.
- **Results.** Seeds 7, 13 (100 `nrm`, `nfcp`, `n3d` cases each) and 17 (200 each): 0 different with
  `reproduce`; seed 13 with the default: no difference without a notice.

