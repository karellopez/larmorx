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
| `src/fastmarch.rs` | `utils/mri_fastmarching.cpp` (`MRIextractDistanceMap`, 25-116); `include/fastmarching.h` (`FastMarching<sign>`: constructor, destructor, `Init`, `SetLimit`, `AddTrialPoint`, `AddAlivePoint`, `InitTrialFromAlive`, `Run`, `_UpdatePoint`, `_GetValue`, `_UpdateValue`, `InitFromMRI`, `_AddAlivePoint`; `HeapCompare`); `utils/mri.cpp` (`MRIdistanceTransform`, 15492-15540) | 3.1-42, track D relay 25 |

Third-party code: none in these files (Florent Segonne's FreeSurfer code under FreeSurfer's
licence). The trial queue is libstdc++'s `std::priority_queue` (GCC 8.4.0's `bits/stl_heap.h`:
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
