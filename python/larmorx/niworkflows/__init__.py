# SPDX-License-Identifier: Apache-2.0
"""niworkflows' interfaces under sdcflows' and fMRIPrep's field-map steps, ported from
niworkflows 1.15.4 (the same on these paths in 1.15.2, fMRIPrep 25.2.6's) with the numpy, SciPy
and scikit-image arithmetic under them.

- :func:`intensity_clip`: ``IntensityClip`` (``_advanced_clip``): the intensities clipped at
  the percentiles of the median-filtered image's positive values and scaled to 0-255.
- :func:`intra_modal_merge`: ``IntraModalMerge``: images of one modality merged into a series
  and averaged (with :func:`reorient` and nibabel's :func:`as_closest_canonical`).
"""

from larmorx.niworkflows.images import (
    IntraModalMergeResult,
    as_closest_canonical,
    intra_modal_merge,
    reorient,
)
from larmorx.niworkflows.nibabel import IntensityClipResult, intensity_clip

__all__ = [
    "IntensityClipResult",
    "IntraModalMergeResult",
    "as_closest_canonical",
    "intensity_clip",
    "intra_modal_merge",
    "reorient",
]
