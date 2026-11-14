# Provenance: larmorx-freesurfer-filter

**Replica: translated from FreeSurfer's source.** This crate holds `fs.filter` (wave 3.1) of the FreeSurfer
replica (`docs/freesurfer/waves.md` section 4): volume filters and morphology, intensity normalisation (with Burkardt's spline), segmentation helpers, flood fill, connected components, fast marching and clusters. It is distributed under FreeSurfer's
licence (`LicenseRef-FreeSurfer`, `../LICENSE`: the FreeSurfer Software License with its required
preface), as part of the `larmorx-freesurfer` package (`docs/licensing.md`).

The upstream is FreeSurfer 8.2.0 (GitHub's `v8.2.0` archive, SHA-256
`bb1699f7655ebf0f3790db6a78af9532ea7c832c97fe5f71bfd9f0e9526d7e78`; `docs/freesurfer-plan.md`),
built for the official binaries with GCC 8.4.0 (`docs/freesurfer/variation.md`).

## FreeSurfer files and functions translated

The crate was created empty by queue item 3.1-01 (`docs/freesurfer/queue.md`); each item that
brings code adds its files and functions here, with the third-party code it found in them
(CLAUDE.md rule 3: each ported file is audited for third-party code first).

| Module | FreeSurfer 8.2.0 | Item, relay |
|---|---|---|
| `src/conv.rs` | `utils/mrifilter.cpp` (`MRIgaussian1d` 2368, `MRIconvolve1d` 3006, `MRIconvolve1dByte` 3321, `MRIconvolve1dShort` 3464, `MRIconvolve1dInt` 3550, `MRIconvolveGaussian` 2564, `MRIgaussianSmoothNI` 6517, `MRIgaussianSmooth` 2643, `MRImaskedGaussianSmooth` 2665, `MRImean` 2034, `MRIstd` 2429, `MRIstdNonzero` 2496, `MRImodifySampledHeader` 2743, the reduction kernel 2890, `MRIreduce` 2891, `MRIreduce2D` 2943, `MRIreduce1d` 3722, `MRIreduceSlice` 3852); `include/mri.h` (`smni_*` 2035, `MRI_HEIGHT`/`WIDTH`/`DEPTH` 1500) | 3.1-42 (and 3.1-43's line 6517), track D relay 26 |
| `src/sobel.rs` | `utils/mrifilter.cpp` (`MRIdivergence` 923, `MRIlaplacian` 947, `MRIsobelFrame` 1109, `MRIsobel` 1125, `MRIxSobel` 1192, `MRIxSobelForAllTypes` 1242, `MRIySobel` 1322, `MRIySobelForAllTypes` 1369, `MRIzSobel` 1442, `MRIzSobelForAllTypes` 1490) | 3.1-42, track D relay 26 |
| `src/classify.rs` | `utils/mrifilter.cpp` (`MRImarkBorderVoxels` 4515, `MRIborderClassifyVoxel` 4687, `MRIreclassify` 4824, `MRIcomputeClassStatistics` 5395); `include/mri.h` (`MRI_AMBIGUOUS`, `MRI_WHITE`, `MRI_NOT_WHITE`) | 3.1-42, track D relay 26 |
| `src/cc.rs` | `utils/connectcomp.cpp` (`xoff6`..`zoff26` 30-36, `RemoveHoles` 38, `GrassFire6` 180, `GrassFire18` 260, `GetLargestCC6` 340, `GetLargestCC18` 391; `myList*`/`myQueue*` 779-1335 as a `VecDeque`) | 3.1-42, track D relay 26 |
| `src/mem.rs` | the row-pointer voxel macros of `include/mri.h` (`MRIvox`, `MRIseq_vox`, `MRIFseq_vox`, `MRISseq_vox`, `MRIIseq_vox` 1477-1495) as an access model (in, another voxel, outside) | 3.1-42, track D relay 26 |
| `src/fastmarch.rs` | `utils/mri_fastmarching.cpp` (`MRIextractDistanceMap`, 25-116); `include/fastmarching.h` (`FastMarching<sign>`: constructor, destructor, `Init`, `SetLimit`, `AddTrialPoint`, `AddAlivePoint`, `InitTrialFromAlive`, `Run`, `_UpdatePoint`, `_GetValue`, `_UpdateValue`, `InitFromMRI`, `_AddAlivePoint`; `HeapCompare`); `utils/mri.cpp` (`MRIdistanceTransform`, 15492-15540) | 3.1-42, track D relay 25 |

Third-party code: none in these files (FreeSurfer's own code under FreeSurfer's licence: the
headers name Florent Segonne for `mri_fastmarching.cpp` and `connectcomp.cpp`, Bruce Fischl for
`mrifilter.cpp`; MGH's copyright). The trial queue is libstdc++'s `std::priority_queue` (GCC 8.4.0's `bits/stl_heap.h`:
`__push_heap`, `__adjust_heap`, `__pop_heap`; GPL-3.0 with the GCC Runtime Library Exception):
its steps are written here from the algorithm (a binary heap's sift-up and sift-down in that
order, `specs/freesurfer-filter.md` §5), not copied; the official build's machine code
(`mri_fastmarching.cpp.o` of the unit oracle) gave `_UpdateValue`'s operand orders.

`InitForOutsideMatch` and `MRISextractOutsideDistanceMap` (`utils/mris_fastmarching.cpp`, the
surface variant) are not here: they need `MRISbinarizeVolume` (3.1-51's leftovers).

## Changes from the originals

- The distance volume's frame 0 is read and written in its own bytes (no copy); the status volume
  is a byte array of the extent (FreeSurfer's `MRIalloc` of bytes, allocated the same way).
- The points `Run` appends to the alive list are not kept (nothing reads them).
- When the march reaches its limit the queue's points are set far at the limit in array order
  (FreeSurfer pops them one by one: the same result).
- FS-539 to FS-544 (`larmorx_core::defects`): the default corrects them; `reproduce` gives
  FreeSurfer's results, with FS-540's and FS-541's memory contents filled with the pass's limit.
- `mrifilter.cpp`'s OpenMP loops run in one thread's order (`MRIgaussianSmoothNI`'s kernel sum
  included); `exec_progress_callback` and `global_progress_range` (no output) are not kept, nor
  `MRIcomputeClassStatistics`' diagnostic `border.mgz` and Gaussian fit under `DIAG_SHOW` and
  `DIAG_VERBOSE`.
- The integer outputs and conversions go through `larmorx_freesurfer_core::utils::nint` (FS-12) and
  FS-557's truncation; the direct accesses through `src/mem.rs` (FS-546, FS-547, FS-555, FS-559).
- FS-545 to FS-561 (`larmorx_core::defects`): the default corrects them (FS-558 is kept in both
  modes until measured, rule D); `reproduce` gives FreeSurfer's results, but where FreeSurfer
  crashes, hangs or touches memory outside its volumes (an error, or the accesses dropped and read
  as 0).
