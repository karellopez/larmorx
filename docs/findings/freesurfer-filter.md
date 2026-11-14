# FreeSurfer 8.2.0's volume filters (`fs.filter`): findings

**Closed to FreeSurfer clean-room implementers** (it names FreeSurfer's files, functions and lines;
the behaviour is `specs/freesurfer-filter.md`). Queue item 3.1-42, replica in
`crates-freesurfer/larmorx-freesurfer-filter` (track D, relay 25, 2026-11-14).

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
