# Provenance: larmorx-ndimage

**Ported** from SciPy 1.18.1 (the sdist in `<workspace>/tags/scipy-1.18.1`, BSD-3-Clause; the C
and Cython code of these functions is the same in SciPy 1.15.3, `tags/scipy-1.15.3`). The logic
is re-expressed in Rust; no code was copied verbatim. Each source file reproduces SciPy's notice
(the crate's root, `src/lib.rs`), which is also in NOTICE.

| larmorx | SciPy 1.18.1 | Licence |
|---|---|---|
| `neighbourhood::map_coordinate`, `Neighbourhood` (the boundary rules and the offsets of a footprint's elements) | `scipy/ndimage/src/ni_support.c` (`NI_InitFilterOffsets`, `NI_InitFilterIterator`) | BSD-3-Clause (Peter J. Verveer) |
| `filters::min_or_max_filter` | `src/ni_filters.c` (`NI_MinOrMaxFilter`, `CASE_MIN_OR_MAX_POINT`, `CASE_FILTER_OUT`, `CASE_FILTER_OUT_SAFE`) | BSD-3-Clause |
| `filters::min_or_max_filter1d`, `extend_line`, `harter` | `src/ni_filters.c` (`NI_MinOrMaxFilter1D`, Richard Harter's sliding minimum as SciPy adapts it), `src/ni_support.c` (`NI_ExtendLine`, `NI_InitLineBuffer`, `CASE_COPY_LINE_TO_DATA`) | BSD-3-Clause |
| `filters::rank_filter`, `ni_select` | `src/ni_filters.c` (`NI_RankFilter`, `CASE_RANK_POINT`, `NI_Select`) | BSD-3-Clause |
| `filters::rank_filter_1d`, `Mediator` | `src/_rank_filter_1d.cpp` (`Mediator`, `MediatorInsert` and its heap helpers, `_rank_filter`; Copyright (c) 2011 ashelly.myopenid.com, MIT, modified in 2024 by Gideon Genadi Kogan) | MIT within BSD-3-Clause |
| `morphology::binary_erosion`, `pass`, `propagate`, `center_is_true`, `dilation_arguments`, `generate_binary_structure`, `iterate_structure`, `MaskElement` | `src/ni_morphology.c` (`NI_BinaryErosion`, `CASE_NI_ERODE_POINT`, `NI_BinaryErosion2`, `CASE_ERODE_POINT2`), `_morphology.py` (`_binary_erosion`, `binary_dilation`, `_center_is_true`, `generate_binary_structure`, `iterate_structure`) | BSD-3-Clause |
| `measurements::label` | `src/_ni_label.pyx` (`_label`, `label_line_with_neighbor`, `mark_for_merge`: the same components and numbers, by a union of earlier neighbours in C order), `_measurements.py` (`label`) | BSD-3-Clause |
| `measurements::sum_labels_bincount` | `_measurements.py` (`_stats`'s `bincount` and `unique` paths, `sum_labels`) and numpy's `bincount` (sequential float64 sums) | BSD-3-Clause |
| `NdElement`, `cvtt_i32`, `cvtt_i64`, `cvtt_u64` | C's double-to-integer casts as SciPy's x86-64 build compiles them (measured: `validation/investigations/sdcflows-fieldmap-b/probe_int_casts.py`) | — |

The Python wrappers (`python/larmorx/_ndimage_filters.py`) port `_filters.py`
(`_min_or_max_filter`, `_rank_filter`, `minimum_filter1d`, `maximum_filter1d`, the `axes`,
origin and mode helpers), `_morphology.py` (`grey_*`, `binary_*`) and `_ni_support.py`
(`_normalize_sequence`, `_check_axes`, `_get_output`) with their messages.

**Deliberate differences** (`docs/api/ndimage-filters.md`, "Differences from SciPy"): where
SciPy reads or writes outside its arrays (binary morphology with an origin before its structure,
the `reflect` boundary two periods before a short axis, the 1-D rank filter's window running past
a short input) or loops for ever (a binary erosion until nothing changes that alternates),
larmorx gives the mode's own value or a clear error.
