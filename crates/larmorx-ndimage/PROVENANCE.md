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
| `watershed::watershed_ift`, `watershed_ift_run`, `Queues`, `WatershedEvents` | `src/ni_measure.c` (`NI_WatershedIFT`, `NI_WatershedElement`, `WS_GET_INDEX`, `CASE_WINDEX1`-`3`: the per-level doubly linked queues, the markers' order, the `int` offsets and labels, the propagation; byte-identical in SciPy 1.15.3), `_measurements.py` (`watershed_ift`); the corrected run (neighbours inside the array, an element always taken out of its queue, 64-bit labels) is larmorx's | BSD-3-Clause |
| `measurements::sum_labels_bincount` | `_measurements.py` (`_stats`'s `bincount` and `unique` paths, `sum_labels`) and numpy's `bincount` (sequential float64 sums) | BSD-3-Clause |
| `NdElement`, `cvtt_i32`, `cvtt_i64`, `cvtt_u64` | C's double-to-integer casts as SciPy's x86-64 build compiles them (measured: `validation/investigations/sdcflows-fieldmap-b/probe_int_casts.py`) | — |

`center_of_mass` is Python (`python/larmorx/_ndimage_filters.py`): `_measurements.py`
(`center_of_mass`, `_stats`' `single_group`; the same in SciPy 1.15.3) with numpy's sums of
contiguous float arrays in a chosen release's order (`larmorx_core::numpy::reduce`).
The fast paths of `filters::min_or_max_filter` (`extreme_plain`, `extreme_plain_3d`,
`lines_plain`, relay 15) are larmorx's: the same values as `NI_MinOrMaxFilter`'s where no NaN can
arise.

The Python wrappers (`python/larmorx/_ndimage_filters.py`) port `_filters.py`
(`_min_or_max_filter`, `_rank_filter`, `minimum_filter1d`, `maximum_filter1d`, the `axes`,
origin and mode helpers), `_morphology.py` (`grey_*`, `binary_*`) and `_ni_support.py`
(`_normalize_sequence`, `_check_axes`, `_get_output`) with their messages.

**Deliberate differences** (`docs/api/ndimage-filters.md`, "Differences from SciPy"): where
SciPy reads or writes outside its arrays (binary morphology with an origin before its structure,
the `reflect` boundary two periods before a short axis, the 1-D rank filter's window running past
a short input) or loops for ever (a binary erosion until nothing changes that alternates),
larmorx gives the mode's own value or a clear error. `watershed_ift`'s defects (neighbours across
the edges, an element left alone in its queue, 32-bit markers) are corrected by default with
SciPy's labels on request (`upstream_defects`).

## scikit-image's `ball`, `threshold_otsu` and `random_walker` (`src/skimage/`)

**Ported** from scikit-image 0.26.0 (BSD-3-Clause, `tags/scikit-image-0.26.0`, sdist SHA-256
`f5f970ab...99fa`; the same arithmetic in 0.25.2, fMRIPrep 25.2.6's), with SciPy 1.18.1's and
1.15.3's sparse arithmetic under the random walker. Each file carries scikit-image's notice.

| Rust | Upstream | Licence |
|---|---|---|
| `skimage::ball` | `src/skimage/morphology/footprints.py` (`ball`, no decomposition) and numpy's `mgrid` with a complex step (`numpy/lib/_index_tricks_impl.py`, `nd_grid`) | BSD-3-Clause |
| `skimage::otsu` (`threshold_otsu`, `threshold_otsu_int`, `threshold_otsu_hist`) | `src/skimage/filters/thresholding.py` (`threshold_otsu`, `_validate_image_histogram`), `src/skimage/exposure/exposure.py` (`histogram`, `_histogram`, `_bincount_histogram`, `_bincount_histogram_centers`, `_offset_array`) | BSD-3-Clause |
| `skimage::random_walker` (`preprocess`, `random_walker`, `RwOutput::full_prob`, `labels`) | `src/skimage/segmentation/random_walker_segmentation.py` (`_make_graph_edges_3d`, `_compute_weights_3d`, `_build_laplacian`, `_build_linear_system`, `_solve_linear_system`'s `cg` and `cg_j`, `_preprocess`, `random_walker`); SciPy's `_compressed.py` (`_setdiag`, `_minor_index_fancy`, `diagonal`), `_coo.py` (`_setdiag`, `_coo_to_compressed`), `_base.py` (`sum`), `sparsetools/csr.h` (`csr_matvecs`, `csc_matvecs`, `csr_matmat`, `csr_diagonal`, `coo_tocsr`), `_dia.py` (`dia_matvec`), `sparse/linalg/_isolve/iterative.py` (`cg`, `_get_atol_rtol`) and `utils.py` (`make_system`), SciPy 1.18.1 and 1.15.3 (the rows' order: measured, `check_skimage.py`) | BSD-3-Clause |

The Python wrapper (`python/larmorx/skimage.py`) ports `random_walker`'s argument checks,
`channel_as_last_axis`, `img_as_float` (`src/skimage/util/dtype.py`, `_convert`'s integer and
boolean branches), the early returns and scikit-image's warnings. Not ported: `mode='bf'`
(SciPy's `spsolve`), pyamg's `cg_mg` (scikit-image's own fallback to `cg_j` without pyamg is),
`ball(decomposition='sequence')`.

`match_histograms` (relay 14 of field maps part B, `python/larmorx/skimage.py` only, numpy
underneath): scikit-image 0.26.0's `src/skimage/exposure/histogram_matching.py`
(`_match_cumulative_cdf`, `match_histograms`; byte-identical in 0.25.2) and
`src/skimage/_shared/utils.py` (`channel_as_last_axis`, `_supported_float_type`'s table);
SKI-02.

**The D13 switch** (`upstream_defects`): SKI-01, a masked Laplacian numbered by the edges' ends;
SKI-02, `match_histograms`' `bincount` of a reference it refuses (`docs/api/skimage.md`).
