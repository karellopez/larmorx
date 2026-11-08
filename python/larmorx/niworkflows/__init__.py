# SPDX-License-Identifier: Apache-2.0
"""niworkflows' interfaces under sdcflows' and fMRIPrep's field-map steps, ported from
niworkflows 1.15.4 (the same on these paths in 1.15.2, fMRIPrep 25.2.6's) with the numpy, SciPy
and scikit-image arithmetic under them.

- :func:`intensity_clip`: ``IntensityClip`` (``_advanced_clip``): the intensities clipped at
  the percentiles of the median-filtered image's positive values and scaled to 0-255.
"""

from larmorx.niworkflows.nibabel import IntensityClipResult, intensity_clip

__all__ = ["IntensityClipResult", "intensity_clip"]
