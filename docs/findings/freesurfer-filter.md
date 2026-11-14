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
`MRIdilate` (1723), which no track has ported yet (track C takes them first, relay 27's notes).
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
